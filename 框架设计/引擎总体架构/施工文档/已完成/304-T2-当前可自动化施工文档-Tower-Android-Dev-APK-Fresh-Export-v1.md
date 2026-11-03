# 304-T2 当前可自动化施工文档：Tower Android Dev APK Fresh Export v1

## 状态与授权

```text
父方案：304 Android Native Player Dev APK Vertical Slice B v1
前置：304 源码 Gate A-C、304-T1 工具链安装与锁定均已完成
状态：已激活，施工中
授权：直接执行 304-T2 Tower Android dev APK fresh export
```

本 Gate 只在 fresh run-owned Tower 副本执行一次真实 Android dev export，并校验生成的 APK、签名、
ARM64 native library、RuntimePackage、module digest 与 package manifest。不得运行 ADB、Local CI，不得修改
引擎源码、真实 Tower 配置/cache 或替换 production Editor/Player/MCP。

## 必要性与验证经济性

```text
requested claim：已锁定工具链能够把当前 Tower 输入真实导出为可审查 Android dev APK。
minimum change：无源码改动；fresh Tower 副本 + 一次现有 exporter 执行 + 产物只读校验。
excluded：ADB、模拟器、真机交互、release/AAB、多 ABI、production 更新、完整回归。
Gate T2-A：lock/fresh-root 边界 -> 路径和输入身份只读检查。
Gate T2-B：真实 export -> 唯一一次 export report 与 APK。
Gate T2-C：产物合同 -> apksigner、ZIP entry、manifest/hash/digest 一致性。
reused evidence：304 owner tests 与 304-T1 toolchain identity；源码/锁不变时不重跑。
stop condition：preflight 或 export 失败即保留首因并停止，不重复 export；不自动进入 repair/ADB。
```

## Gate T2-A：fresh 输入与锁定环境

创建唯一 run-owned evidence root 和临时 Tower 副本。副本保留当前项目输入，排除 `.aife`、`Library`、
`Build`、`Tests` 与 `RuntimeModule/target` 缓存/测试产物；临时项目位于 `samples` 直接子目录，以保持项目 RuntimeModule 的
`../../../rust` 正式相对依赖。记录项目输入摘要、exporter SHA-256 和 toolchain lock SHA-256。

## Gate T2-B：唯一真实 export

在 dot-source `G:\AifeToolchains\android-304-v1\activate.ps1` 的同一进程中调用一次：

```text
G:\gameEngin\rust\target\debug\export_android_dev.exe <fresh-tower-root>
```

捕获 stdout/stderr、退出码和结构化 report。失败时不得自动重试；成功时复制最终 APK、package manifest、
export report 与必要 staging readback 到 evidence root。

## Gate T2-C：只读产物校验与清理

校验 report success、APK SHA-256、debug signing certificate、`lib/arm64-v8a/libmain.so`、GameActivity manifest、
RuntimePackage asset manifest、project/module/profile/SDK/NDK/ABI 字段及 native/runtime digest 一致性。验证完成后
只清理已证明属于本 Gate 的临时 Tower 副本，保留外部 evidence root。

## 激活前复核与自审

2026-08-20 复核：当前/ 没有其它施工文档，待执行队列为空；304-T1 lock 的 hash 与 verification report
一致，consumer probe ready；真实 Tower output/staging 不存在。现有 exporter 和 Tower profile 已由 304 owner
evidence 覆盖，本轮只验证新的工具链/Android artifact composition，不重复源码测试。文档没有扩大到 repair、
ADB、Local CI、真实项目或 production。结论：通过，允许施工。

## Gate T2-A / T2-B 实际结果

2026-08-20，run `304-T2-20260820-090835` 的 269 个 fresh 输入文件与当前 Tower 源逐文件 SHA-256
一致，RuntimeModule 相对依赖解析到正式 `G:\gameEngin\rust`；Gate T2-A 通过。

唯一一次 exporter attempt 在 1.05 秒内以 `environmentBlocked / notBuilt / notRun` 结束，未创建 output 或
staging。五项 preflight 中 JDK、SDK/build-tools、NDK 和 Rust target 通过，只有 `gradle-bootstrap=false`。
只读诊断确认锁定 Gradle 8.11.1 的 Windows 入口为 `gradle.bat`，而
`android_export.rs:496` 使用 `Command::new("gradle")`；PowerShell/cmd 可执行该 `.bat`，Rust probe 未命中，
属于引擎 owner 的 Windows command resolution 缺陷，不是工具链或 Tower 缺失。

本施工文档明确排除了引擎源码修改，并规定 export 失败不得自动重试，因此 Gate T2-B 保持 terminal failed，
Gate T2-C 未运行。fresh root 与 evidence 保留；真实 Tower 未修改。恢复条件是单独授权 one-owner source repair，
修复 Windows `gradle.bat` 探测/调用后再取得一次新的 export attempt 权限。

## Repair R1：Windows Gradle Launcher Resolution

2026-08-20 用户已单独授权一次引擎最小修复。允许范围仅为：

```text
owner：rust/crates/editor_core/src/android_export.rs
change：Windows preflight 与后续 build command 使用 gradle.bat；非 Windows 继续使用 gradle
test：host launcher name 合同 + 激活锁定工具链时的真实 Command invocation
consumer：重建 development export_android_dev.exe
excluded：新的 APK export attempt、ADB、Local CI、production 更新、其它 exporter/command runner 重构
```

该 repair 不新增 schema、shim、通用进程抽象或兼容层。repair 通过只证明修复已进入开发态 exporter，不恢复
T2-B，也不授权第二次 export；新 attempt 必须另行授权。

### Repair R1 结果

```text
source：Windows 使用 gradle.bat；非 Windows 使用 gradle
fmt：cargo fmt --package editor_core -- --check passed
targeted test：1 passed / 0 failed / 706 filtered out
真实 locked gradle.bat Command invocation：passed
development exporter build：passed
old exporter SHA-256：5a756345...710197
new exporter SHA-256：e712524a...398244
```

构建仅出现三条既有 `engine_runtime` dead-code warning。未运行新的 export attempt、ADB、Local CI 或 production
更新。Repair R1 完成；304-T2 仍等待用户单独授权新的 fresh export attempt。

## Gate T2-B Attempt 2 授权

2026-08-20 用户已直接授权下一步。attempt 2 精确绑定：

```text
fresh root：G:\gameEngin\samples\__304_t2_20260820_090835
input：269/269 files，与 attempt 1 input manifest SHA-256 一致，零 output/staging
exporter SHA-256：e712524aedc26405d70c77453e03da27df037c8b2a60e9c88ac198ed85398244
toolchain lock SHA-256：c4dd5bef413c71bde25c7c5bade5cbf5cafb2f7142de774636b2d8f699bab007
scope：唯一 attempt 2 + 成功后的 Gate T2-C artifact verification
excluded：自动 attempt 3、ADB、Local CI、production、真实 Tower 修改
```

attempt 2 失败则保留首因并停止，不自动修复或重跑。

### Gate T2-B Attempt 2 结果

attempt 2 的 toolchain preflight 五项全部通过，`gradleCommand=gradle.bat`，证明 Repair R1 已进入真实 exporter。
随后 RuntimePackage assembly 在 native/Gradle 前失败，状态为 `failed / notBuilt / notRun`，只创建了空 staging，
没有 APK/output。

首因是施工 scenario 把整个 `Library` 误分类为缓存。Tower 项目 FontBundle 的正式 consumer
`project_font_cook.rs` 明确从 `Library/AssetPipeline/asset-database.json` 解析 font source assetRef，缺失时产生
`FontAssetDatabaseMissing`。真实 Tower 的 AssetPipeline 三份 identity 文件存在且未复制到 fresh root。该失败
不需要引擎或 Tower 源码修复；最小 scenario correction 是只复制 byte-exact
`asset-database.json / asset-graph.json / asset-registry.json`，继续排除其它 Library/cache。

attempt 2 terminal failed；attempt 3 未授权。fresh root、空 staging 与 evidence 保留，真实 Tower 未修改。

## Gate T2-B Attempt 3 与剩余闭环授权

2026-08-20 用户已授权一次性完成整个 304-T2 剩余闭环：

```text
attempt 3 fresh root：G:\gameEngin\samples\__304_t2_attempt3_20260820_110738
input：当前 Tower 源 + byte-exact Library/AssetPipeline 三份 formal identity
exporter SHA-256：e712524aedc26405d70c77453e03da27df037c8b2a60e9c88ac198ed85398244
toolchain lock SHA-256：c4dd5bef413c71bde25c7c5bade5cbf5cafb2f7142de774636b2d8f699bab007
authorized：唯一 attempt 3、Gate T2-C、artifact/evidence retention、run-owned cleanup、完成记录与归档
excluded：attempt 4、ADB、Local CI、production、真实 Tower 修改、额外引擎/Tower source repair
```

attempt 3 失败则保留首因并停止；成功则连续完成 T2-C、cleanup 和文档闭环。

### Gate T2-B Attempt 3 结果

attempt 3 的五项工具链 preflight 全部通过，并已包含 Tower 正式 AssetPipeline identity。exporter 在
RuntimePackage assembly 阶段以 `android_export.runtime_package_build_failed` terminal failed；未进入 native
或 Gradle build，未生成 APK。

首因是 Android exporter 的正式 request target 为 `android-arm64-dev`，而
`engine_runtime::runtime_package_builder::validate_request` 仍只允许 `dev-desktop`。这是
RuntimePackage owner 与已完成 304 Android consumer 之间的单一 target 合同缺口；工具链、Tower 输入和
fresh scenario 均不是本次失败 owner。

## Repair R2 与 Gate T2-B Attempt 4 授权

2026-08-20 用户重新授权一次性完成 304-T2 剩余闭环。授权精确收敛为：

```text
owner：rust/crates/engine_runtime/src/runtime_package_builder.rs
minimum repair：正式允许 dev-desktop 与 android-arm64-dev；其它 target 继续 UnsupportedTarget fail-closed
test：一个 owner 定向测试同时证明 Android accepted 与 unknown rejected
consumer：重建 development export_android_dev.exe
attempt 4：使用新的 corrected fresh root，当前 Tower byte-exact 输入 + 三份正式 AssetPipeline identity
success closure：Gate T2-C APK/signing/ARM64/GameActivity/RuntimePackage/manifest/hash 校验、证据保留、run-owned cleanup、完成记录与归档
excluded：ADB、Local CI、production Editor/Player/MCP 更新、真实 Tower 配置/cache/source 修改、通用 Build Target schema 或 runner 重构
```

attempt 4 仍为唯一新的真实 export。若 repair 的定向测试、export 或产物合同失败，保留首因并按真实结果
结束，不循环重试或扩大 repair。

### Gate T2-B Attempt 4 结果与 Repair R3

attempt 4 已通过 toolchain、profile、RuntimePackage 与 launcher，证明 Repair R2 和 corrected fresh input
均生效；随后 `android-activity` 的 C++ build script 因未收到 NDK compiler 环境而 terminal failed：

```text
CXX_aarch64_linux_android = None
failed to find tool aarch64-linux-android-clang++ / clang++.exe
```

锁定 NDK 已包含与 linker 同目录、同 API level 的 `aarch64-linux-android26-clang++.cmd`。用户的
“一次性完成整个 304-T2 剩余闭环”授权继续覆盖同一 Android exporter owner 内的最小接线：Cargo child
process 显式获得 target-specific `CC_aarch64_linux_android` 与 `CXX_aarch64_linux_android`，且启动前
fail-closed 校验 C++ compiler。增加一个纯 owner 定向测试；重建 exporter 后只执行一次新的 fresh
attempt 5。其余排除项保持不变，禁止演变为通用 command runner、工具链重装或多平台 target schema。

### Gate T2-B Attempt 5 结果与 Repair R4

attempt 5 日志确认 `CXX_aarch64_linux_android=Some(...android26-clang++.cmd)`，C++ compiler 缺口已关闭；
同一个 `cc-rs` build 随后明确因 `AR_aarch64_linux_android=None` 而找不到
`aarch64-linux-android-ar`。锁定 NDK 已包含同目录 `llvm-ar.exe`。剩余最小 repair 只把该正式 archiver
以 target-specific `AR_aarch64_linux_android` 传给同一 Cargo child，并把存在性纳入已有 tool binding
定向测试。重建 exporter 后执行一次新的 fresh attempt 6；其它授权边界不变。

### Gate T2-B Attempt 6 结果与 Repair R5

attempt 6 已成功生成 ARM64 `libmain.so`，证明 RuntimePackage、launcher、CC/CXX/AR 与 native build 均已
闭环；随后 exporter 在 Gradle wrapper 阶段达到 30 分钟边界且没有 terminal JSON。线程转储、staging 与
独立 `--no-daemon wrapper --stacktrace` 诊断共同定位到默认 distribution URL 的公网 HEAD 请求：
`https://services.gradle.org/distributions/gradle-8.11.1-bin.zip` 连接超时。锁定工具链已经持有下载哈希
通过的 `downloads/gradle-8.11.1-bin.zip`；使用对应 `file:///` URL 的 run-owned staging 诊断在 6.7 秒
内通过并生成 wrapper。

Repair R5 只让 exporter 从 `AIFE_ANDROID_TOOLCHAIN_LOCK` 同根解析该已锁定本地 archive，并以
`--no-daemon wrapper --gradle-distribution-url file:///...` 生成 wrapper；archive 缺失时 fail-closed。
不增加网络 fallback、下载器或新 schema。定向测试与 exporter 重建通过后，执行一次新的 fresh attempt 7，
成功则进入 T2-C 与归档；其它排除项保持不变。

### Gate T2-B Attempt 7 结果与 Repair R6

attempt 7 已通过 `rustNative`，本地 Gradle distribution 完整消费成功；Gradle 在
`:app:checkDebugAarMetadata` 以确定性诊断失败：generated app 引用了
`androidx.games:games-activity:3.0.5`，但未生成 `android.useAndroidX=true`。Repair R6 只在生成 Android
project 时写入标准 `gradle.properties`，并扩展现有 generated-project owner 测试；重建 exporter 后执行
一次新的 fresh attempt 8。

同次 Gradle 在锁定 SDK 下自动安装了此前 lock 未列出的 `platform-tools 37.0.1`。该外部漂移不修改
`toolchain-lock.json`、不作为 304-T2 成功证据，并在最终记录中单列；ADB 仍不运行。

## Gate T2-B Attempt 8 / Gate T2-C 最终结果

attempt 8 使用 272 个与真实 Tower 源逐文件 SHA-256 一致的输入、toolchain lock
`c4dd5bef...bab007` 和 exporter `98170859...f35e26`，在 272.217 秒内通过全部 8 个 stage：

```text
toolchain -> profile -> runtimePackage -> launcher -> rustNative -> gradle -> apkVerify -> publish
status=success
packageStatus=success
deviceQualification=notRun
```

Gate T2-C 通过。APK 为 34,738,700 bytes / SHA-256 `3c824fc4...1761ff`；APK Signature v2 与单一
debug signer 通过，证书 SHA-256 为 `a8d94950...3d8729`。APK 只含
`lib/arm64-v8a/libmain.so`；GameActivity、portrait、SDK 35 / min 26、`android.app.lib_name=main` 均由
aapt2 真实 readback 证明。67 个 RuntimePackage 文件逐项 size/hash 通过，package/module/profile/NDK、
720x1280 contain target 与跨 manifest digest 均一致。

首次 Gate T2-C verifier 因 PowerShell 单引号内错误转义 `\"main\"` 产生一项 harness false negative；修正
只读断言后同一 APK 的 14/14 checks passed，未重建或重导出。最终 verification SHA-256 为
`c981b458...1af02d`。

APK、package manifest、RuntimePackage manifests、export report、apksigner 与 aapt2 readback 已保留在：

```text
G:\AifeAndroidRuns\304-T2-20260820-090835\artifacts\attempt-8
G:\AifeAndroidRuns\304-T2-20260820-090835\evidence\304-t2-final-result.json
```

7 个 `samples/__304_t2_*` run-owned 临时项目根已删除；没有 Cargo/Gradle/Java/exporter 残留。真实 Tower、
production Editor/Player/MCP 和持久配置均未修改。未运行 ADB、Local CI 或 production 更新。

外部偏差：attempt 7 的 AGP 自动安装 `platform-tools 37.0.1` 到锁定 SDK，但原
`toolchain-lock.json` 保持 byte-unchanged；该新增组件不计入 304-T1 原 lock 证明，证据见
`platform-tools-external-drift.json`。304-T2 已完成，可以归档。
