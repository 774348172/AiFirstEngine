# Platform Build Profile MVP

本文记录 Phase 15 移动端与多平台发布基础的第一版实现。

## 定位

Platform Build Profile 不是实际打包器。

它负责在 Build Graph 上方定义：

```text
Platform Profile
  -> Build Profile
  -> Platform Build Plan
  -> Fallback Report
  -> Package Manifest
```

目标是先把多平台发布的决策层做成结构化、可验证、可解释的数据。

真实 Windows / Android / iOS / Web 打包器后续必须执行这套结构，而不是绕过它。

## 当前实现

新增代码：

```text
src/build/platformBuildProfile.ts
scripts/platform-build-profile.cjs
scripts/test-platform-build-profile.cjs
scripts/test-platform-build-profile-script.cjs
scripts/build-graph.cjs
scripts/export-game-web.cjs
scripts/export-game-android.cjs
scripts/android-build-gate.cjs
scripts/export-game-ios.cjs
scripts/ios-build-gate.cjs
scripts/test-web-export.cjs
scripts/test-android-export-skeleton.cjs
scripts/test-android-build-gate.cjs
scripts/test-ios-export-skeleton.cjs
scripts/test-ios-build-gate.cjs
```

新增命令：

```powershell
npm.cmd run test:platformbuild
npm.cmd run test:platformbuildscript
npm.cmd run test:webexport
npm.cmd run test:androidexport
npm.cmd run test:androidbuildgate
npm.cmd run test:iosexport
npm.cmd run test:iosbuildgate
```

## Platform Profile v1

结构：

```text
schemaVersion: platform-profile.v1
platform
displayName
runtime
defaultPackageKind
capability
```

当前内置平台：

```text
windows
android
ios
web
```

当前 capability：

```text
realGpuBackend
nativeRuntime
hotUpdate
dynamicNativeCode
filesystemAccess
packageKinds[]
maxTextureSize
supportedAssetFormats[]
supportedRuleTargets[]
```

规则：

```text
Platform Profile 描述平台事实。
AI 和用户不应该通过自然语言随意改平台事实。
如果平台能力变化，应作为引擎版本升级或平台后端升级处理。
```

## Build Profile v1

结构：

```text
schemaVersion: build-profile.v1
id
platform
mode
quality
packageKind
assetCookFormat
ruleTarget
includeDebugSymbols
allowFallback
createdBy
```

规则：

```text
Build Profile 是用户 / AI 可以配置的发布意图。
Build Profile 不直接执行打包。
Build Profile 必须经过 Platform Profile 解析。
非法配置不能静默通过。
allowFallback=true 时，引擎可以降级并生成 warning。
allowFallback=false 时，引擎必须生成 error。
```

## Platform Build Plan v1

结构：

```text
schemaVersion: platform-build-plan.v1
id
projectName
projectVersion
platformProfile
buildProfile
stages[]
fallbacks[]
errors[]
warnings[]
```

当前标准阶段：

```text
validate-project
resolve-build-profile
build-asset-database
plan-bundles
cook-assets
build-bundles
compile-runtime
compile-rules
package-platform
write-package-manifest
```

Web 当前不包含 compile-rules，因为第一版 Web 使用 web-ir / ir-interpreter 路线。

规则：

```text
Platform Build Plan 是给引擎执行的发布计划。
AI 可以生成 Build Profile，但不能直接手写最终打包步骤。
Build Graph 仍然负责 Asset DB / Bundle Plan / Bundle Build。
Platform Build Plan 负责把平台、资源 cook、规则目标、包类型统一起来。
```

## Fallback Report v1

当前 fallback 覆盖：

```text
packageKind
assetCookFormat
ruleTarget
quality
```

规则：

```text
Fallback 必须结构化记录 requested / resolved / severity / reason。
Fallback 是 AI 向用户解释“为什么发布结果变了”的证据。
Fallback 不能藏在 console log。
Fallback severity=error 时，Platform Build Plan validation 必须失败。
```

## Package Manifest v1

结构：

```text
schemaVersion: package-manifest.v1
packageId
projectName
projectVersion
platform
buildProfileId
packageKind
runtime
ruleTarget
assetCookFormat
bundleManifest
generatedAt
fallbacks[]
```

规则：

```text
Package Manifest 是最终平台包的解释索引。
它必须记录使用了哪个 Build Profile、哪个 ruleTarget、哪个 assetCookFormat。
它必须保留 fallback 记录。
后续真实打包器必须写出同一 Package Manifest。
```

## 当前边界

暂不做：

```text
真实 APK / AAB / IPA 输出
真实 Rust runtime 编译
真实 Android Gradle 编译
真实完整 Xcode 工程生成 / xcodebuild archive
真实签名 / 证书 / store upload
真实平台资源压缩
编辑器 UI
```

第一版已建立多平台发布决策层的可测数据闭环，并已让 Windows Build Graph、Web Export、Android Export Skeleton、iOS Export Skeleton 写出 `platform-build-plan.json` 和 `package-manifest.json`。

## Windows Build Graph 接入

当前 Windows 导出会额外写出：

```text
platform-build-plan.json
package-manifest.json
```

并在 `build-manifest.json` 中记录：

```text
platform
buildProfile
packageManifest
packageKind
runtime
ruleTarget
assetCookFormat
fallbacks[]
```

规则：

```text
Windows 导出也必须执行 Platform Build Plan。
Windows 当前 packageKind=exe-portable。
Windows 当前 ruleTarget=rust-aot。
Windows 当前 assetCookFormat=desktop-raw。
Build Graph 仍然负责 Asset DB / Bundle Plan / Bundle Build。
Package Manifest 负责解释最终平台包使用了什么发布配置。
```

## Web Export 接入

当前 Web 导出会输出静态目录：

```text
index.html
ai-first-runtime.js
player.js
game-project.json
asset-database.json
asset-report.json
bundle-plan.json
bundle-manifest.json
platform-build-plan.json
package-manifest.json
bundles/
three/
```

并在 `build-manifest.json` 中记录：

```text
platform: web
packageKind: web-static
runtime: web
ruleTarget: web-ir
assetCookFormat: basisu-web
```

规则：

```text
Web Export 也必须执行 Platform Build Plan。
Web Export 当前 packageKind=web-static。
Web Export 当前 ruleTarget=web-ir。
Web Export 当前 assetCookFormat=basisu-web。
Web Export 不复制 Electron main.cjs / preload.cjs。
Web Export 复用 Runtime Asset Loader / Bundle Manifest，不另建一套资源加载规则。
```

## Android Export Skeleton 接入

当前 Android 导出会输出工程骨架：

```text
android-project/
  settings.gradle
  build.gradle
  gradle.properties
  app/build.gradle
  app/src/main/AndroidManifest.xml
  app/src/main/java/<package>/MainActivity.kt
  app/src/main/res/values/styles.xml
  app/src/main/assets/ai-first/
    ai-first-runtime.js
    game-project.json
    asset-database.json
    asset-report.json
    bundle-plan.json
    bundle-manifest.json
    platform-build-plan.json
    package-manifest.json
    bundles/
```

并在 `build-manifest.json` 中记录：

```text
platform: android
packageKind: aab
runtime: native-rust
ruleTarget: rust-aot
assetCookFormat: astc-mobile
android.gradleBuildStatus: not-run
android.skeletonOnly: true
```

规则：

```text
Android Export Skeleton 必须执行 Platform Build Plan。
Android Export Skeleton 当前 packageKind=aab。
Android Export Skeleton 当前 ruleTarget=rust-aot。
Android Export Skeleton 当前 assetCookFormat=astc-mobile。
Android Export Skeleton 不伪造 APK / AAB。
Android Export Skeleton 必须明确记录 gradleBuildStatus=not-run。
后续 Gradle / NDK / Rust Runtime 接入必须消费 app/src/main/assets/ai-first/package-manifest.json。
```

## Android Build Gate 接入

当前 Android Build Gate 会输出：

```text
android-build-gate-report.json
```

结构：

```text
schemaVersion: android-build-gate-report.v1
androidProjectDir
status: ready | blocked
canRunGradle
gradleBuildStatus: not-run
checks[]
blockingIssues[]
warnings[]
nextActions[]
generatedAt
```

当前检查：

```text
settings.gradle
root build.gradle
app build.gradle
AndroidManifest.xml
ai-first package-manifest.json
ai-first build-manifest.json
ANDROID_HOME / ANDROID_SDK_ROOT
ANDROID_NDK_HOME / NDK_HOME
Gradle wrapper
Android Gradle Plugin declaration
Kotlin Android Plugin declaration
namespace / applicationId / compileSdk / minSdk
```

规则：

```text
Android Build Gate 只判断是否具备真实 Android 编译前置条件。
Android Build Gate 不运行 Gradle。
Android Build Gate 不生成 APK / AAB。
Android Build Gate 必须输出结构化 blockingIssues / warnings / nextActions。
缺少 Android SDK 是 blocking。
缺少 NDK 或 Gradle wrapper 在当前阶段是 warning，后续进入真实 Rust Native 编译时可升级为 blocking。
```

## iOS Export Skeleton 接入

当前 iOS 导出会输出工程骨架：

```text
ios-project/
  <AppName>.xcodeproj/project.pbxproj
  <AppName>/Info.plist
  <AppName>/AppDelegate.swift
  <AppName>/ViewController.swift
  <AppName>/Resources/README-ai-first.md
  <AppName>/Resources/ai-first/
    ai-first-runtime.js
    game-project.json
    asset-database.json
    asset-report.json
    bundle-plan.json
    bundle-manifest.json
    platform-build-plan.json
    package-manifest.json
    bundles/
```

并在 `build-manifest.json` 中记录：

```text
platform: ios
packageKind: ipa
runtime: native-rust
ruleTarget: rust-aot
assetCookFormat: astc-mobile
ios.xcodeBuildStatus: not-run
ios.skeletonOnly: true
```

规则：

```text
iOS Export Skeleton 必须执行 Platform Build Plan。
iOS Export Skeleton 当前 packageKind=ipa。
iOS Export Skeleton 当前 ruleTarget=rust-aot。
iOS Export Skeleton 当前 assetCookFormat=astc-mobile。
iOS Export Skeleton 不伪造 IPA。
iOS Export Skeleton 必须明确记录 xcodeBuildStatus=not-run。
后续 Xcode / Rust Runtime / Signing 接入必须消费 Resources/ai-first/package-manifest.json。
```

## iOS Build Gate 接入

当前 iOS Build Gate 会输出：

```text
ios-build-gate-report.json
```

结构：

```text
schemaVersion: ios-build-gate-report.v1
iosProjectDir
status: ready | blocked
canRunXcodebuild
xcodeBuildStatus: not-run
checks[]
blockingIssues[]
warnings[]
nextActions[]
generatedAt
```

当前检查：

```text
host platform is macOS
.xcodeproj
project.pbxproj
Info.plist
ai-first package-manifest.json
ai-first build-manifest.json
xcodebuild
Apple development team
```

规则：

```text
iOS Build Gate 只判断是否具备真实 iOS 编译前置条件。
iOS Build Gate 不运行 xcodebuild。
iOS Build Gate 不生成 IPA。
iOS Build Gate 必须输出结构化 blockingIssues / warnings / nextActions。
非 macOS host 是 blocking。
缺少 xcodebuild 是 blocking。
缺少 Apple Development Team 在当前阶段是 warning，后续进入签名 / archive export 时可升级为 blocking。
```

## 当前测试覆盖

当前测试覆盖：

```text
default windows / android / ios / web platform profiles
default build profile resolution
windows / android / ios native-rust rule target
web web-ir rule target
native platforms include compile-rules stage
web skips compile-rules stage
package manifest records platform/build profile/runtime/rule target/asset cook format
unsupported web package kind falls back to web-static
unsupported web asset cook falls back to basisu-web
unsupported web rule target falls back to web-ir
web high quality falls back to medium
strict iOS invalid profile fails validation when allowFallback=false
script-side platform build profile matches the same schema
Windows Build Graph writes platform-build-plan.json
Windows Build Graph writes package-manifest.json
build-manifest.json references platform build plan and package manifest
Web Export writes static runtime directory
Web Export writes platform-build-plan.json
Web Export writes package-manifest.json
Web Export omits Electron main/preload files
Web build-manifest.json records web-ir and basisu-web
Android Export Skeleton writes Gradle project skeleton
Android Export Skeleton writes ai-first assets
Android Export Skeleton writes platform-build-plan.json
Android Export Skeleton writes package-manifest.json
Android Export Skeleton records gradleBuildStatus=not-run
Android Build Gate produces ready report with mock SDK/NDK
Android Build Gate produces blocked report without Android SDK
Android Build Gate records nextActions
iOS Export Skeleton writes Xcode project skeleton
iOS Export Skeleton writes ai-first resources
iOS Export Skeleton writes platform-build-plan.json
iOS Export Skeleton writes package-manifest.json
iOS Export Skeleton records xcodeBuildStatus=not-run
iOS Build Gate produces ready report with mock xcodebuild
iOS Build Gate produces blocked report without macOS/xcodebuild
iOS Build Gate records nextActions
```

回归命令：

```powershell
npm.cmd run test:platformbuild
npm.cmd run test:platformbuildscript
npm.cmd run test:buildgraph
npm.cmd run test:webexport
npm.cmd run test:androidexport
npm.cmd run test:androidbuildgate
npm.cmd run test:iosexport
npm.cmd run test:iosbuildgate
npm.cmd run build
```
