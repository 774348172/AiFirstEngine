# 237-Release Package Polish / Metadata / Icon / Layout v1 方案

> 状态：已施工完成（用户确认的 B-min+；Gate A-F、整体回归、完成记录与归档均已完成）。  
> 路线优先级：`227` 的 `P2-2`。  
> 前置系统：`236 Save / Reload / Rebuild Consistency Gate v1` 已完成并归档。  
> 目标：把已可玩的 Windows dev export 收敛为具有真实产品身份、可移植目录、可机器验证入口和稳定 Launcher 升级边界的 Windows portable release package。  
> 用户确认：采用 B-min+，并补充 `entrypoint` / 文件角色合同，禁止把根目录入口永久绑定为 Runtime 本体。

## 1. 这个系统是干什么的

当前导出是开发产物：

```text
Game.exe
data/runtime_package/
reports/
package-manifest.json
```

237 把它升级为可直接分发的 Windows 目录：

```text
<ProductExecutable>.exe
package-manifest.json
data/runtime_package/
```

系统负责四件事：

```text
Metadata
  产品名、公司、描述、显示版本、Windows 四段版本和版权信息。

Icon
  从项目 AssetRef 解析应用图标，生成多尺寸 Windows icon resource，并写入入口 exe。

Layout
  生成只含发布 payload 的 portable directory；manifest 只保存 package-relative path。

Entrypoint
  用户双击入口 exe 可直接进入游戏；Build & Run、验证器和 AI report 从 manifest 解析入口，不再硬编码 Game.exe。
```

它大致对标：

```text
Unity PlayerSettings + BuildPipeline platform postprocessor
Unreal GeneralProjectSettings + Windows Stage/Bootstrap resource update
Godot Windows export preset + TemplateModifier
```

它不是 installer、签名系统、商店上传系统，也不改变 RuntimePackage 作为发布运行输入真相的地位。

## 2. 为什么现在必须做

复杂打飞机主线已经完成真实纹理、真实玩法、真实 HUD、导出 playable golden、Build & Run、规则/输入/资产可视化编辑和 236 一致性闭环。

当前仍有真实发布缺口：

```text
exe 固定名为 Game.exe。
exe 的 FileVersion / ProductName / ProductVersion / CompanyName 为空。
BuildProfile v1 没有发布身份、架构或 icon AssetRef。
package-manifest.json 保存本机绝对路径，不可移植。
reports 位于实际分发目录内。
exported player verifier 固定寻找 Game.exe。
runtime_cli 无参数启动返回 missing arguments，用户双击 exe 不能进入游戏。
```

因此 P2-2 不是视觉装饰，而是发布合同缺口。

## 3. 当前代码基线

### 3.1 BuildProfile

当前入口：

```text
rust/crates/editor_core/src/project_runtime_package_assembler.rs
  BuildProfile

samples/complex_shooter_project/BuildProfiles/windows.dev.json
```

`BuildProfile v1` 只有：

```text
profile
target
runtimePackageMode
frameLimit
headlessSurfaceGate
realWindowSmoke
```

`project.aife.json.projectName` 已存在，但 authoring project name 不能自动承担完整 release identity。

### 3.2 DesktopExportPipeline

当前入口：

```text
rust/crates/editor_core/src/desktop_export.rs
  DesktopExportRequest::windows_dev
  DesktopExportPipeline::export
  DesktopPackageManifest
  DesktopExportReport
  stage_player_executable
```

当前结构：

```text
package root = Build/Windows/dev
runtime package = data/runtime_package
reports = package root/reports
entry executable = Game.exe
player source = rust/target/debug/ai_engine_runtime_cli.exe
```

当前 package manifest 的 `packageDir`、`runtimePackageDir`、`reportsDir` 和 `playerExecutable` 都是本机绝对路径。

### 3.3 运行入口与验证器

当前入口：

```text
rust/crates/runtime_cli/src/lib.rs
  RuntimeCliArgs::parse
  resolve_native_player_paths

rust/crates/runtime_cli/src/exported_player_verification.rs
  resolve_verification_paths
```

现状：

```text
runtime_cli 没有参数时直接返回 missing arguments。
run-native-player 未指定 package 时优先读取 <exe_dir>/data/runtime_package。
默认 report 写入 <exe_dir>/reports/windowed-player-run-report.json。
exported verifier 固定读取 <package>/Game.exe。
```

正式发布运行时默认应为 report Off，不能要求发布目录可写，也不能靠编辑器补参数才能启动。

### 3.4 继承 236

236 已完成：

```text
RuntimePackage canonical digest
package-relative path containment
single-writer staging/publish
rollback/recovery
published package formal loader verification
```

237 继承 236 的最终合同，不复制 RuntimePackage assembler、content hash 或 runtime payload publisher。

237 只负责 RuntimePackage 外层 Windows release directory：

```text
verified RuntimePackage
+ stamped entrypoint executable
+ outer release manifest
-> release directory staging
-> release verification
-> release directory publish
```

## 4. 成熟引擎与 Windows 参考

### 4.1 Unity

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\PlayerSettings.bindings.cs
  PlayerSettings.companyName
  PlayerSettings.productName
  PlayerSettings.bundleVersion
  SetIcons / SetIconsForPlatform

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPlayerWindowBuildMethods.cs
  GetBuildPlayerOptions
  Paths.MakeValidFileName(PlayerSettings.productName)

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPipeline\PostprocessBuildPlayer.cs
  Postprocess(... companyName, productName, ...)
```

可学习：产品身份是结构化 project/build input，平台 postprocessor 负责生成最终产物。  
不照搬：不建立全局隐式 PlayerSettings；本项目继续以版本化 BuildProfile、AssetRef 和 report 为真相。

### 4.2 Unreal Engine

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\EngineSettings\Classes\GeneralProjectSettings.h
  CompanyName
  CopyrightNotice
  Description
  ProjectName
  ProjectVersion
  ProjectDisplayedTitle

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\Win\WinPlatform.Automation.cs
  GetFilesToDeployOrStage
  StageBootstrapExecutable
  ModuleResourceUpdate.SetIcons
  bootstrap executable target/args resource
```

可学习：发布入口、Runtime executable 和 staged payload 可以分离；资源写入发生在 staging 后、签名前。  
不照搬：本轮不实现多架构 Bootstrap、prerequisite installer 或第二进程。

### 4.3 Godot

```text
https://github.com/godotengine/godot/blob/master/platform/windows/export/export_plugin.cpp
  EditorExportPlatformWindows::_process_icon
  application/icon
  application/file_version
  application/product_version
  application/company_name
  application/product_name
  TemplateModifier::modify

https://github.com/godotengine/godot/blob/master/platform/windows/export/template_modifier.cpp
  FixedFileInfo
  VersionInfo
  GroupIcon
  PE resource directory rebuild
```

可学习：Windows metadata 属于 export preset；版本写入前验证为四段整数；源图标生成 16/32/48/64/128/256 多尺寸资源。  
不照搬：不在本项目手写完整 PE parser/resource directory builder。

### 4.4 Windows 官方合同

```text
https://learn.microsoft.com/en-us/windows/win32/menurc/versioninfo-resource
https://learn.microsoft.com/en-us/windows/win32/menurc/icon-resource
https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-updateresourcew
```

关键约束：

```text
FILEVERSION / PRODUCTVERSION 是四个 16-bit 整数。
StringFileInfo 承载 CompanyName / ProductName / FileDescription 等字符串。
ICON/GROUP_ICON 可以包含多个尺寸。
资源修改必须保持有效 PE 和已有无关资源。
```

## 5. 候选方案与正式选择

### 5.1 方案 A：Manifest / Layout Only

只重命名 exe、改相对 manifest、移出 reports，不写 PE metadata/icon。

优点：施工快。  
缺点：Explorer 仍没有真实图标和版本属性，不满足系统目标。  
结论：不采用。

### 5.2 方案 B-min+：Direct Runtime Entrypoint + Release Contract

```text
BuildProfile v2 定义 application metadata。
项目 icon 使用 AssetRef。
复制 release runtime template 到 staging root。
入口命名为 application.executableName.exe。
写入 Windows VERSIONINFO 和多尺寸 icon。
release manifest 定义 entrypoint、runtimePackage 和 file roles。
验证器从 manifest 解析 entrypoint，不硬编码 Game.exe。
无参数启动在有效 release package 中进入 native player。
验证通过后发布 portable directory。
```

优点：不增加第二进程，形成最小真实发布闭环，并保留 Launcher 升级边界。  
缺点：入口 exe 当前仍是 Runtime 本体。  
结论：正式采用。

### 5.3 方案 C：Branded Bootstrap Launcher + Hidden Runtime

```text
根目录品牌 Launcher 作为 entrypoint。
通用 Runtime 位于 bin/。
Launcher 显式传 RuntimePackage 路径，等待子进程并透传 exit code。
```

优点：适合多架构、依赖安装、签名和 installer。  
缺点：增加第二进程、错误透传、生命周期和签名对象。  
结论：deferred；B-min+ 的 manifest 必须允许后续切换，但本轮不实现 Launcher。

## 6. 正式架构链

```text
ProjectManifest
+ BuildProfile v2
+ application icon AssetRef
  -> ReleasePackagePlan
  -> ProjectRuntimePackageAssembler
  -> RuntimePackageBuilder / 236 safe publish
  -> verified RuntimePackage
  -> release directory staging
       copy runtime release template
       stamp PE metadata/icon
       copy runtime payload inventory
       write relative release manifest
  -> ReleasePackageVerifier
       schema/path/inventory/hash
       PE resource readback
       outer staging manifest entrypoint process launch
       RuntimePackage formal load
  -> outer release directory publish
  -> ReleasePackageReport
  -> Report Panel build.release_package
```

禁止：

```text
绕过 ProjectRuntimePackageAssembler 扫项目目录。
从编辑器内存对象直接拼最终 payload。
把 Game.exe 固定名称写入 release 新逻辑。
把 entrypoint 必然等同 Runtime executable 写入消费者。
在 release manifest 中写绝对 project/output path。
```

实际施工确认的所有权与发布顺序：

```text
generic Runtime asset payload bytes 必须进入 RuntimePackageSourceAsset / assembly input digest，
并由 RuntimePackageBuilder 写入；DesktopExport / release outer layer 禁止在 Builder 后补拷贝 Runtime payload。

ReleasePackageVerifier 直接验证 outer staging；schema/path/hash、PE readback、正式 RuntimePackage load
和 manifest-driven 真实子进程全部通过后才允许 atomic publish 到 final。
```

## 7. BuildProfile v2

release profile 示例：

```json
{
  "schemaVersion": "build-profile.v2",
  "profile": "release",
  "target": "windows",
  "architecture": "x86_64",
  "runtimePackageMode": "debug-readable",
  "frameLimit": 6,
  "headlessSurfaceGate": true,
  "realWindowSmoke": "optional",
  "application": {
    "displayName": "Complex Shooter",
    "executableName": "ComplexShooter",
    "companyName": "Example Studio",
    "fileDescription": "Complex Shooter",
    "displayVersion": "1.0.0",
    "windowsFileVersion": [1, 0, 0, 0],
    "windowsProductVersion": [1, 0, 0, 0],
    "copyright": "Copyright Example Studio",
    "icon": {
      "assetId": "asset-app-icon"
    }
  },
  "release": {
    "layout": "portable-directory-v1",
    "includeReports": false,
    "includeDebugSymbols": false
  }
}
```

`runtimePackageMode` 本轮仍诚实沿用当前可加载格式；237 不扩展为单文件 cooked/archive 系统。

### 7.1 兼容规则

```text
windows.dev.json 继续支持 build-profile.v1。
release package 只接受 build-profile.v2 的完整 application/release block。
不得静默把不完整 v1 profile 当 release profile。
projectName 只可作为创建 profile 时 displayName 的初始默认值。
```

### 7.2 executableName

```text
显式配置，不从 displayName 每次重新推导。
仅允许稳定 Windows 文件名字符集。
禁止绝对路径、分隔符、.、..、尾随点/空格和保留设备名。
最终文件名为 <executableName>.exe。
大小写折叠后不得与 package 内其它路径冲突。
```

### 7.3 版本

```text
displayVersion 是用户可见字符串。
windowsFileVersion / windowsProductVersion 必须各有四段。
每段范围 0..65535。
不从任意 semver 字符串隐式猜四段 Windows 版本。
```

### 7.4 Icon AssetRef

```text
icon 必须通过 AssetRef/assetId 解析。
release profile 禁止长期 raw path-only icon。
源图至少支持 PNG 或 ICO；PNG 应为正方形且不小于 256x256。
生成 16/32/48/64/128/256 尺寸资源。
缺失、悬空、解码失败或尺寸不合格时 release build 失败，不静默使用 engine 默认 icon。
```

## 8. Release package manifest

schema：

```text
release-package-manifest.v1
```

示例：

```json
{
  "schemaVersion": "release-package-manifest.v1",
  "application": {
    "displayName": "Complex Shooter",
    "displayVersion": "1.0.0",
    "companyName": "Example Studio"
  },
  "target": {
    "platform": "windows",
    "architecture": "x86_64",
    "profile": "release"
  },
  "launch": {
    "userFrameLimit": null
  },
  "entrypoint": "ComplexShooter.exe",
  "runtimePackage": "data/runtime_package",
  "runtimeContentHash": "sha256:...",
  "releasePayloadHash": "sha256:...",
  "files": [
    {
      "path": "ComplexShooter.exe",
      "size": 123456,
      "sha256": "sha256:...",
      "roles": ["entrypoint", "runtime"]
    }
  ]
}
```

规则：

```text
所有 path 是 package-relative `/` path。
entrypoint 必须引用 files 中具有 entrypoint role 的唯一文件。
runtimePackage 必须位于 package root 内。
roles 是版本化字符串集合；v1 至少支持 entrypoint/runtime/launcher/runtime-payload。
同一个文件在 B-min+ 可以同时拥有 entrypoint 和 runtime role。
package-manifest.json 自身不进入 files/releasePayloadHash，避免自引用 hash。
releasePayloadHash 对按 path 排序后的 payload path + file SHA-256 做长度分帧 canonical hash。
manifest 不保存 packageDir、projectRoot、reportsDir 等绝对路径。
launch.userFrameLimit=null 表示用户双击后由窗口关闭事件结束；它不得回退为 CLI 默认 1 帧。
自动化 verifier 显式传 --frames，测试帧数不覆盖用户 launch policy。
```

### 8.1 后续切换方案 C

B-min+：

```json
{
  "entrypoint": "ComplexShooter.exe",
  "files": [
    { "path": "ComplexShooter.exe", "roles": ["entrypoint", "runtime"] }
  ]
}
```

方案 C：

```json
{
  "entrypoint": "ComplexShooter.exe",
  "files": [
    { "path": "ComplexShooter.exe", "roles": ["entrypoint", "launcher"] },
    { "path": "bin/ai_engine_runtime.exe", "roles": ["runtime"] }
  ]
}
```

切换时不改变：

```text
RuntimePackage schema/contentHash
ProjectRuntimePackageAssembler
玩法、ECS、渲染、AUI 和资产链
Build & Run / verifier 读取 entrypoint 的规则
PE metadata/icon stamper 输入合同
```

方案 C 的 Launcher 必须等待 Runtime child、透传 exit code，并显式传 package-relative RuntimePackage 路径。

## 9. Windows executable resource stamping

建议新增窄模块：

```text
rust/crates/editor_core/src/windows_executable_resources.rs
```

职责：

```text
输入 copied executable + validated metadata + resolved icon bytes。
使用固定版本、通过依赖审计的 Rust PE resource library 修改资源。
写入 VERSIONINFO、StringFileInfo、ICON/GROUP_ICON。
保留已有 manifest 和无关资源。
输出结构化 result/diagnostics。
```

约束：

```text
施工 Gate 先评估并固定 editpe 一类能重建完整 resource directory 的 Rust library。
不得手写通用 PE parser。
不得依赖用户机器额外安装 rcedit.exe、rc.exe 或 Visual Studio。
只修改 staging copy，禁止修改 rust/target 下的 engine template。
未来签名必须发生在 resource stamping 之后。
```

PE readback 必须证明：

```text
ProductName
CompanyName
FileDescription
ProductVersion / FileVersion string
fixed file/product four段版本
GROUP_ICON 和预期 icon sizes
```

## 10. Portable release layout

```text
Build/Windows/x86_64/release/ComplexShooter/
  ComplexShooter.exe
  package-manifest.json
  data/
    runtime_package/
      manifest.json
      assets/
      aui/
      cooked/
      fonts/
      input/
      prefabs/
      rules/
      scenes/
      schema/
```

规则：

```text
发布 payload 不包含 editor report、trace、project source 或 cache。
RuntimePackage 内非运行必需 reports/** 在 release copy 中排除。
includeReports=false 是 v1 release 强制值。
includeDebugSymbols=false 是 v1 默认值；符号发布另开系统。
v1 不自动生成 ZIP；portable directory 是本轮发布产物。
```

Editor/AI 报告保存在：

```text
<project>/.aife/reports/release-package/latest.json
```

Trace 可以记录绝对 staging/output path；Summary 和发布 manifest 不暴露本机路径。

## 11. Entrypoint 启动合同

### 11.1 用户双击

B-min+ 的入口是 Runtime 本体，但必须支持：

```text
no args
+ current exe parent 下存在有效 release-package-manifest.v1
+ manifest.entrypoint 指向当前 exe
+ manifest.runtimePackage 通过 containment 校验
-> packaged release mode
-> native windowed player
```

packaged release mode 读取 `launch.userFrameLimit`；`null` 映射为无限用户会话，自动化测试必须显式传有限 `--frames`，不得让 CLI 默认 1 帧造成双击即退出。

否则保持 CLI 开发行为并输出 usage，不把任意 cwd 猜成 release package。

### 11.2 Report 默认值

```text
用户双击 packaged release：Runtime report Off，不写发布目录。
Editor Build & Run / test：显式传 --report，允许 Summary/Trace。
正式 Runtime 不因 report 写入失败而影响用户运行。
```

### 11.3 Editor / verifier

```text
Build & Run 从 DesktopExportReport/ReleasePackageManifest 获取 entrypoint。
exported verifier 从 manifest 获取 entrypoint/runtimePackage。
release verifier 不得拼接 Game.exe。
验证器显式传 --package <manifest.runtimePackage>，不依赖 exe_dir 猜测。
working directory 固定为 package root。
```

旧 `desktop-package-manifest.v1 + Game.exe` 只保留为明确 legacy/dev 分支；`release-package-manifest.v1` 不回退猜文件名。

## 12. ReleasePackageReport

schema：

```text
release-package-report.v1
```

建议字段：

```text
schemaVersion
status
reportLevel
projectId
profile
target
architecture
applicationSummary
runtimePackageSummary
entrypointSummary
resourceStampSummary
layoutSummary
payloadHash
verificationSummary
diagnostics[]
nextAction
```

diagnostic 至少区分：

```text
release_profile_missing
release_profile_schema_unsupported
release_identity_invalid
release_executable_name_invalid
release_version_invalid
release_icon_asset_missing
release_icon_decode_failed
release_player_template_missing
release_resource_stamp_failed
release_resource_readback_mismatch
release_manifest_invalid
release_path_escape
release_path_collision
release_entrypoint_missing
release_entrypoint_launch_failed
release_runtime_package_load_failed
release_payload_hash_mismatch
release_publish_busy
release_publish_failed
release_publish_rollback_failed
```

分档：

```text
Off
  不自动执行 release build，不生成 JSON。

Summary
  Report Panel 显示最近状态、产品名、版本、入口、payload hash 和首个 next action。

Trace
  记录 profile resolution、AssetRef、staging、resource readback、file inventory、process 和 publish evidence。
```

Report Panel provider：

```text
build.release_package
```

## 13. 编辑器产品面

Build 面板新增 release profile 产品面，底层真相仍是 BuildProfile v2：

```text
Display Name
Executable Name
Company Name
File Description
Display Version
Windows File Version
Windows Product Version
Application Icon Asset Picker
Architecture
Output Layout Preview
Build Release Package
```

规则：

```text
Icon 通过 235 Asset Browser Picker 选择 AssetRef。
字段编辑走既有 command/service/undo/save 路径。
UI 不直接拼 package 或修改 exe。
保存成功后才清 dirty。
Build Release Package 不等于 Build & Run；它默认执行发布验证，但不为用户常驻启动游戏。
```

## 14. 验证策略

### 14.1 Headless deterministic

```text
BuildProfile v2 schema/default/invalid matrix。
Windows 文件名和四段版本边界。
AssetRef resolve 和 icon 尺寸生成。
manifest 全相对路径、唯一 entrypoint、file role 和 canonical payload hash。
相同输入两次 releasePayloadHash 一致。
有效 metadata/icon mutation 改变对应 resource/hash。
report/path/timestamp mutation 不改变 payload hash。
旧 stale payload 在 safe publish 后消失。
```

### 14.2 PE fixture

```text
在 owned temp executable copy 上 stamp。
readback metadata 与 BuildProfile 一致。
icon group/尺寸存在。
已有 manifest/unrelated resource 未丢失。
source engine template hash 不变。
```

### 14.3 Windows process gate

```text
最终 manifest entrypoint 存在。
无参数启动可进入 packaged native player。
显式 headless-gate 启动 exit 0。
RuntimePackage 通过正式 loader。
process cwd 是 package root。
用户模式不要求 report 文件。
默认 release build 在 outer staging 上完成上述 process gate；失败不得 publish final。
```

真实 Explorer icon cache 视觉检查作为 local-only smoke；PE resource readback 是默认强制门禁。

## 15. 预期涉及模块

生成施工文档前必须按 236 完成后的代码复扫，预计涉及：

```text
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/desktop_export.rs
rust/crates/editor_core/src/windows_executable_resources.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/ui_model_composer.rs

rust/crates/editor_ui_model/src/build_export.rs
rust/crates/editor_ui_renderer/src/panels/build_export.rs

rust/crates/runtime_cli/src/lib.rs
rust/crates/runtime_cli/src/exported_player_verification.rs

rust/crates/project_e2e_gate/src/release_package.rs
rust/crates/project_e2e_gate/src/lib.rs

samples/complex_shooter_project/BuildProfiles/windows.release.json
samples/complex_shooter_project/Assets/Branding/<icon asset>
```

不新增：

```text
第二套 ProjectRuntimePackageAssembler
通用跨平台 Packaging Framework
Runtime 常驻 Report 层
Bootstrap Launcher crate
Installer/Signing/Store service
```

## 16. 推荐施工 Gate

### Gate A：BuildProfile v2 / Release Plan

```text
BuildProfile v1 dev compatibility。
BuildProfile v2 release schema。
application/release validation。
executable/version/path collision tests。
```

### Gate B：Icon Asset / PE Resource Stamping

```text
依赖审计与固定版本。
AssetRef resolve。
multi-size icon generation。
VERSIONINFO/icon stamp + readback fixture tests。
```

### Gate C：Release Manifest / Layout / Safe Publish

```text
release-package-manifest.v1。
entrypoint/file roles/runtimePackage relative path。
payload inventory/hash。
outer staging/validation/publish/rollback。
```

### Gate D：No-arg Packaged Entrypoint / Manifest-driven Verifier

```text
packaged no-arg native player mode。
Runtime report Off default。
verifier removes release Game.exe hardcode。
legacy dev fallback remains explicit。
```

### Gate E：Editor / Report Panel / Complex Shooter E2E

```text
release profile editor surface。
Build Release Package command/service。
build.release_package provider。
complex shooter release package e2e。
```

### Gate F：Regression / Docs / Archive

```text
editor_core / runtime_cli / engine_runtime / editor_window_winit / project_e2e_gate 回归。
真实 Windows process smoke。
阶段完成记录和入口同步。
施工文档归档。
227 P2-2 标为完成。
```

## 17. 本轮明确不做

```text
Bootstrap Launcher / 第二进程。
ZIP、installer、MSIX、Steam/Store package。
代码签名、证书管理、timestamp server。
自动更新、patch installer、delta package。
资源加密、反篡改、DRM。
PDB/symbol server/crash upload。
通用跨平台 release abstraction。
改变 RuntimePackage 内部格式为单文件 cooked archive。
```

## 18. 风险与控制

### 风险 1：为了未来 C 过度设计

控制：只定义 `entrypoint` 和 file roles，不实现 Launcher interface、进程代理框架或多 Runtime router。

### 风险 2：PE 修改破坏 executable

控制：使用成熟 library、只修改 staging copy、保留原资源、readback + process launch 双验证。

### 风险 3：把 release identity 塞进 ProjectManifest

控制：ProjectManifest 表达 authoring project；平台发布身份进入 BuildProfile v2。

### 风险 4：manifest 泄露本机路径

控制：发布 schema 只接受 package-relative path；绝对路径只允许出现在 Trace report。

### 风险 5：双击启动依赖可写目录

控制：packaged user mode report Off；不在 package root 写报告、日志或 cache。

### 风险 6：与 236 重复发布逻辑

控制：复用 236 已完成的 path/hash/publish 合同；237 只增加外层 release directory 编排。

### 风险 7：未来签名顺序错误

控制：合同固定为 assemble -> stamp -> verify -> sign（deferred）-> publish；签名后禁止再改资源。

### 风险 8：Windows process image 短暂占用阻塞目录替换

控制：共享 `atomic_directory_publish` 只对 Windows 5/32/33 的 rename 失败重试，间隔 20ms、总上限 5 秒；其它错误立即失败，single-writer、rollback、fault injection 和 hard bound 合同不变。

## 19. 方案自审

### 19.1 是否符合用户确认

是。正式采用 B-min+，并补充：

```text
manifest-driven entrypoint
file roles
禁止 Game.exe 固定假设
禁止 entrypoint == runtime 的永久假设
未来 Launcher 等待 child 并透传 exit code
```

### 19.2 是否保持 RuntimePackage 真相

是。RuntimePackage 仍由 ProjectRuntimePackageAssembler -> RuntimePackageBuilder 产生；237 只装配外层发布目录。

### 19.3 是否继承 236

是。237 继承 236 已完成的 canonical digest、path containment、safe staging/publish 和 formal loader verification，不重复实现 RuntimePackage 真相。

### 19.4 是否 AI-first

是。BuildProfile、manifest、file roles、hash、diagnostics 和 report 都是版本化 schema，可生成、可审查、可复现。

### 19.5 是否能平滑升级 C

是。方案 C 只把一个 `[entrypoint,runtime]` 文件拆成 `[entrypoint,launcher]` 与 `[runtime]` 两个文件；消费者仍启动 manifest.entrypoint，RuntimePackage 链不变。

### 19.6 是否扩大为完整发行平台

否。installer、签名、商店、ZIP、自动更新、符号服务和资源归档格式均 deferred。

### 19.7 是否解决真实双击入口

是。release package 中无参数启动进入 native player；缺少有效 release manifest 的普通 CLI 环境仍输出 usage。

### 19.8 是否满足 report 规则

是。用户 Runtime 默认 Off；Editor report 分 Off/Summary/Trace；发布 manifest 与 Summary 不暴露本机绝对路径。

### 19.9 是否可以生成施工文档

已完成。施工文档已按 Gate A-F 执行、记录并归档到 `施工文档/已完成/237-当前可自动化施工文档-Release-Package-Polish-Metadata-Icon-Layout-v1.md`。

## 20. 结论

正式采用：

```text
B-min+：BuildProfile v2
       + AssetRef icon
       + stamped direct Runtime entrypoint
       + portable relative layout
       + manifest-driven entrypoint/file roles
       + no-arg packaged launch
       + ReleasePackageReport
```

施工结果：

```text
Gate A-F 与整体回归通过
-> 阶段完成记录已生成
-> 入口与 227 P2-2 状态已同步
-> 当前施工文档已归档
```

方案 C 保留为后续升级方向，但本轮不支付第二进程成本。

### 20.1 实际施工回填（2026-07-11）

```text
ReleasePackageBuilder 默认在 outer staging 中完成 manifest-driven headless process verification，验证失败不发布 final。
generic Runtime assets 的 bytes/digest 已收回 RuntimePackageBuildInput -> RuntimePackageBuilder，DesktopExport 不再做 post-builder asset copy。
共享 atomic_directory_publish 对 Windows transient rename errors 5/32/33 使用 20ms interval、5s hard bound retry。
Build 与 Asset Browser 的底部产品面改为独立稳定列，发布命令和 Asset Browser 行/打开命令均可真实命中。
editpe 固定为 0.2.3、BSD-2-Clause、default-features=false、features=[std]；PE template 只在 staging copy 修改。
```

下一入口是 238 Real LLM Provider / Minimal Repair Loop v1 的方案审查与施工文档闭环；237 不并行启动第二份施工。

## 21. 参考

```text
227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md
236-Save-Reload-Rebuild-Consistency-Gate-v1方案.md
232-Editor-Build-And-Run-Productization-v1方案.md
231-Exported-Windows-Playable-Golden-Gate-v1方案.md
128-Playable-Windows-Export-Vertical-Slice-v1方案.md
07-Build-Export-Pipeline.md

rust/crates/editor_core/src/desktop_export.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/runtime_cli/src/lib.rs
rust/crates/runtime_cli/src/exported_player_verification.rs

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\PlayerSettings.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPlayerWindowBuildMethods.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPipeline\PostprocessBuildPlayer.cs

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\EngineSettings\Classes\GeneralProjectSettings.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\Win\WinPlatform.Automation.cs

https://github.com/godotengine/godot/blob/master/platform/windows/export/export_plugin.cpp
https://github.com/godotengine/godot/blob/master/platform/windows/export/template_modifier.cpp
https://learn.microsoft.com/en-us/windows/win32/menurc/versioninfo-resource
https://learn.microsoft.com/en-us/windows/win32/menurc/icon-resource
https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-updateresourcew
```
