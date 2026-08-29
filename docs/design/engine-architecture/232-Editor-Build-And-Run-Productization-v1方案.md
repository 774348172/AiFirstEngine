# 232-Editor Build And Run Productization v1 方案

> 状态：正式方案，用户已确认采用 `B-min`。  
> 校准日期：2026-07-09。  
> 所属路线：`227` 的 `P1-1 Editor Build And Run Productization v1`。  
> 前置：`129 Editor Build / Export Workspace v1`、`217 Editor Play / RuntimePackage Preview Productization v1`、`231 Exported Windows Playable Golden Gate v1` 已完成。  
> 本文只生成方案，不允许直接施工；施工前仍需审查/自审、施工文档、分 Gate 测试。

## 0. 用户确认结论

本系统确认采用：

```text
B-min:
  Editor-facing Build And Run command
  + DesktopExportPipeline
  + exported Game.exe process launch
  + structured EditorBuildAndRunReport
  + Report Panel entry
```

一句话含义：

```text
用户在编辑器里点击 Build And Run，
编辑器先复用正式 DesktopExportPipeline 生成 Windows dev package，
成功后从导出目录启动 staged Game.exe，
并把 export / launch / process / report path / diagnostics 汇总成 AI 可审查报告。
```

核心红线：

```text
不新建第二套 Build Pipeline。
不把 Build And Run 混成 Editor Play。
不让 Runtime 读取项目源目录或 EditorSession 内存。
不把 231 golden gate 变成每次用户点击 Build And Run 的默认重型流程。
不从 cargo target 直接启动 runtime_cli 假装是导出 Game.exe。
```

## 1. 这个系统是干什么的

它解决的是用户体验问题：

```text
现在自动化 gate 已能证明导出的 Game.exe 可玩，
但用户还缺一个编辑器里的明确入口：
  点一次 Build And Run
  -> 生成 Windows 包
  -> 启动导出的 Game.exe
  -> 失败时知道卡在 Build、Package、Launch 还是 Runtime。
```

它在本引擎主线中的位置：

```text
Editor Build Export Panel
  -> BuildAndRunDesktopPackage command
  -> EditorBuildAndRunService
  -> DesktopExportPipeline::export
  -> Build/Windows/dev/Game.exe
  -> launch exported Game.exe from package dir
  -> EditorBuildAndRunReport
  -> Console / Report Panel / AI context
```

它不是：

```text
不是 Editor Play。
不是 GameView in-editor Play。
不是 P0 golden gate。
不是 release installer/signing/store package。
不是新的 RuntimePackage 装配器。
```

## 2. 为什么现在做它

`227` 的 P0 已经完成：

```text
228 真实贴图进入 RuntimePackage / GPU texture present。
229 复杂打飞机项目规则真实运行。
230 HUD state 来自真实 runtime World / Project Rule 状态。
231 导出的 Game.exe 已通过 P0 golden gate 验证。
```

当前剩余体验缺口：

```text
129 已有 Export / Output / Report，
但没有 Build And Run。

231 能在自动化测试里启动导出的 Game.exe，
但它不是用户在编辑器里点击的产品化入口。
```

因此下一步应该把：

```text
自动化可证明
```

推进到：

```text
用户可操作、AI 可诊断、Report Panel 可查看。
```

## 3. 其它引擎源码参考

### 3.1 Unity

本机源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPlayerWindow.cs
  BuildPlayerAndRun()
  BuildPlayerAndRunInternal(bool askForBuildLocation)
  CallBuildMethods(askForBuildLocation, BuildOptions.AutoRunPlayer | BuildOptions.StrictMode)

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPlayerWindowBuildMethods.cs
  CallBuildMethods(...)
  m_Building
  BuildPlayerOptions
  DefaultBuildMethods.BuildPlayer(options)

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPipeline\Settings\BuildOptions.bindings.cs
  AutoRunPlayer
```

官方参考：

```text
https://docs.unity3d.com/6000.0/Documentation/ScriptReference/BuildPipeline.BuildPlayer.html
```

Unity 的关键做法：

```text
Build And Run 是 Build Pipeline 的一条分支。
Build And Run 通过 AutoRunPlayer 表达“构建完成后启动 Player”。
BuildPlayerWindow 负责 UI / location / profile，真正构建进入 BuildPipeline。
CallBuildMethods 用 m_Building 保证一次只执行一个 build。
BuildReport 是构建结果的正式证据。
```

可学习点：

```text
用户心智简单：Build / Build And Run。
Build And Run 不等于 Play Mode。
UI 不自己拼底层构建步骤。
一次只允许一个 build/run 操作。
```

不照搬：

```text
不复制 Unity 的 native Player build 内部黑盒。
不引入 Unity 式 Domain Reload / Script Compile / Build Profile 全套复杂度。
不把 Build And Run 做成隐藏状态很多、AI 难审查的黑盒按钮。
```

### 3.2 Unreal Engine

本机源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\Scripts\RunProjectCommand.Automation.cs
  Run(ProjectParams Params)
  RunInternal(...)
  SetupClientParams(...)
  RunClient(...)

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\AutomationUtils\ProcessUtils.cs
  ProcessUtils.Run(...)
  ProcessManager.CreateProcess(...)
```

官方参考：

```text
https://dev.epicgames.com/documentation/unreal-engine/unreal-automation-tool-overview-for-unreal-engine
```

UE 的关键做法：

```text
BuildCookRun 把 Cook / Stage / Package / Deploy / Run 组织成一条平台流程。
Run 阶段从 staged output 找 executable，再通过平台抽象启动。
ProcessUtils.Run 负责进程存在性、命令行、环境变量、stdout/stderr、exit code。
日志与进程结果是自动化和诊断的重要证据。
```

可学习点：

```text
Stage 目录是发布运行边界。
Run 必须启动 staged executable，不应启动源工程里的临时二进制。
进程启动要有结构化 process report。
```

不照搬：

```text
不复制 UAT / UBT / BuildGraph / 多平台矩阵的大型框架。
不把第一版 Build And Run 做成完整发布平台。
不把 UE 的巨量命令行参数暴露给用户或 AI。
```

### 3.3 Godot

本项目已有源码参考：

```text
框架设计/Godot源码参考/10-Build-Module-Platform源码参考.md
框架设计/Godot源码参考/11-EditorRun-GameView-PlaySession源码参考.md
```

官方参考：

```text
https://docs.godotengine.org/en/latest/tutorials/export/exporting_projects.html
```

Godot 的关键做法：

```text
Export 负责输出可玩的构建产物。
Editor Run / GameView 是另一条编辑器运行链路。
进程运行由 EditorRun 管理，GameView 只是显示/调试消费者。
```

可学习点：

```text
Build/Export 和 Editor Run 要保持边界清楚。
GameView 不承担构建编排。
运行进程、显示窗口、调试入口可以逐步扩展，但第一版先把启动链路做真。
```

## 4. 本项目当前基线

### 4.1 已完成能力

`129 Editor Build / Export Workspace v1` 已完成：

```text
BuildExportModel
ExportDesktopPackage
OpenBuildOutput
OpenBuildReport
DesktopExportPipeline::export
last_desktop_export_report
Report Panel build.export provider
```

当前代码入口：

```text
rust/crates/editor_core/src/services/build_service.rs
  EditorSession::export_desktop_package
  EditorSession::open_build_output
  EditorSession::open_build_report

rust/crates/editor_ui_model/src/command.rs
  UiCommandPayload::ExportDesktopPackage
  UiCommandPayload::OpenBuildOutput
  UiCommandPayload::OpenBuildReport

rust/crates/editor_ui_model/src/build_export.rs
  BuildExportModel
  BuildExportCommand
  BuildExportReportSummary

rust/crates/editor_core/src/ui_model_composer.rs
  build_export_model

rust/crates/editor_core/src/report_panel.rs
  BuildExportReportProvider
```

`217 Editor Play / RuntimePackage Preview Productization v1` 已完成：

```text
Play 使用 Preview RuntimePackage cache。
Play 不默认执行 DesktopExportPipeline。
Build And Run 被明确留在 Build Pipeline 分支。
```

`231 Exported Windows Playable Golden Gate v1` 已完成：

```text
project_e2e_gate 可导出 complex shooter sample。
runtime_cli::verify_exported_player_process 可启动 staged Game.exe。
报告可汇总 package/process/texture/gameplay/HUD/AUI evidence。
```

相关代码：

```text
rust/crates/runtime_cli/src/exported_player_verification.rs
  verify_exported_player_process
  ExportedPlayerProcessVerificationReport

rust/crates/project_e2e_gate/src/exported_windows_playable_golden.rs
  ExportedWindowsPlayableGoldenGateReport
```

### 4.2 当前缺口

```text
没有 UiCommandPayload::BuildAndRunDesktopPackage。
BuildExportModel 里没有 Build And Run 按钮/命令。
EditorSession 没有 Build And Run service。
Export 成功后不会从 package dir 启动 staged Game.exe。
没有 EditorBuildAndRunReport。
Report Panel 没有 build_and_run entry。
自动化没有验证“编辑器命令 -> export -> staged exe launch/report”的产品化链路。
```

## 5. 备选方案

### 5.1 方案 A：Export 后提示用户手动打开目录

内容：

```text
保持 129。
Export 成功后 Console 输出 package dir。
用户自己打开目录并双击 Game.exe。
```

优点：

```text
改动最小。
不涉及进程管理。
```

缺点：

```text
不是 Build And Run。
AI 无法确认 exe 是否启动成功。
用户体验仍停留在“自己去找产物”。
```

结论：

```text
不采用。
```

### 5.2 方案 B-min：Editor Build And Run Command

内容：

```text
新增 BuildAndRunDesktopPackage command。
先调用 DesktopExportPipeline::export。
export 成功后从 report.package_dir 启动 staged Game.exe。
写 EditorBuildAndRunReport。
Report Panel 增加 Build And Run entry。
自动化测试复用 bounded headless verification。
```

优点：

```text
最贴近 Unity Build And Run 心智。
复用现有 DesktopExportPipeline，不造第二套 pipeline。
复用 231 exported process contract，不造第二套 exe 验证器。
AI 能通过 report 精确定位 Build / Stage / Launch / Runtime 失败。
复杂项目可维护，后续可自然扩展 build queue、cancel、stdout panel。
```

缺点：

```text
第一版还不是完整 build queue / run session manager。
用户 windowed run 的进程生命周期只做最小启动记录，不做完整调试器。
```

结论：

```text
采用。
```

### 5.3 方案 C：完整 Build Queue + Run Session Manager

内容：

```text
Build profiles。
后台构建队列。
取消/重试。
Run last build / run last good。
stdout/stderr live panel。
进程 stop/restart 管理。
多平台 build target。
```

优点：

```text
长期完整。
```

缺点：

```text
当前 P1-1 过大。
会把“编辑器一键构建并启动”拖成完整发布系统工程。
容易和后续 Release Package / Build Queue / Debugger 系统边界混淆。
```

结论：

```text
作为后续扩展，不进入 v1。
```

## 6. 正式采用：B-min

正式命名：

```text
Editor Build And Run Productization v1
```

核心链路：

```text
Build Export panel
  -> BuildAndRunDesktopPackage { profile_id: "windows-dev" }
  -> EditorBuildAndRunService
  -> DesktopExportPipeline::export
  -> DesktopExportReport
  -> if export success:
       launch Build/Windows/dev/Game.exe from package dir
  -> EditorBuildAndRunReport
  -> Console / Report Panel
```

自动化验证链路：

```text
EditorSession command
  -> DesktopExportPipeline::export
  -> runtime_cli::verify_exported_player_process
  -> EditorBuildAndRunReport links ExportedPlayerProcessVerificationReport
```

用户窗口启动与自动化验证分离：

```text
用户点击 Build And Run：
  目标是启动导出的 Game.exe。
  第一版可以只记录 process_started / pid / package_dir / command_line。
  不默认等待用户关闭窗口。

自动化测试：
  使用 bounded headless verification。
  等待 child process 退出并解析 report。
```

## 7. 核心规则

### 7.1 Build And Run 不等于 Play

规则：

```text
Play:
  高频编辑验证。
  走 Preview RuntimePackage cache。
  不 stage desktop package。
  不复制/启动导出 Game.exe。

Build And Run:
  发布产物验证/用户试运行。
  走 DesktopExportPipeline。
  生成 Build/Windows/dev package。
  启动 staged Game.exe。
```

禁止：

```text
点击 Play 默认执行 Build And Run。
点击 Build And Run 使用 editor preview cache 替代 DesktopExportPipeline。
把 GameView in-process runtime 冒充导出的 Game.exe。
```

### 7.2 只启动导出目录内的 Game.exe

规则：

```text
Build And Run 只能启动 DesktopExportReport.player_executable 指向的 staged executable。
默认工作目录必须是 DesktopExportReport.package_dir。
RuntimePackage path 必须来自 package_dir/data/runtime_package。
```

禁止：

```text
直接启动 rust/target/debug/ai_engine_runtime_cli.exe。
直接启动 runtime_player_winit 内部 helper。
直接从项目源目录读取 RuntimePackage。
```

原因：

```text
用户要验证的是导出产物，不是开发环境里的临时二进制。
AI 诊断也必须能回答“这次运行的是哪个 package”。
```

### 7.3 一次只允许一个 Build And Run

参考 Unity `m_Building`，v1 规则：

```text
如果已有 build/export/build-and-run 正在执行，新的 Build And Run 返回 rejected。
如果已有 launched player 仍在运行，v1 不强制 kill；只在 report 中提示 previous_player_process_may_still_be_running。
完整 Stop Built Player / process lifecycle manager 延后。
```

### 7.4 Export 失败不启动

规则：

```text
DesktopExportReport.status != success 时：
  Build And Run 必须停止在 export_failed。
  不尝试启动旧的 Game.exe。
  不自动运行 last good。
```

后续可扩展：

```text
Run Last Successful Build
```

但不进入 v1。

### 7.5 Report 分档

遵守项目 report 规则：

```text
Editor report:
  Off / Summary / Trace。

默认用户点击：
  写 Summary。
  Console 显示简短状态、package dir、report path。

自动化 / gate：
  可写 Trace。
  可链接 ExportedPlayerProcessVerificationReport。
```

Runtime 热路径默认不因此常驻写重 JSON。

## 8. 新增/扩展结构

### 8.1 UiCommandPayload

新增：

```text
BuildAndRunDesktopPackage {
  profile_id: Option<String>
}
```

第一版只支持：

```text
None
"windows-dev"
```

unsupported profile：

```text
editor.build_and_run.unsupported_profile
```

### 8.2 BuildExportModel

在 `BuildExportModel.commands` 中增加：

```text
command_id: build_and_run_desktop_package
label: Build & Run
enabled:
  project_open && no_build_running
```

没有项目时：

```text
disabled reason = Open a project first.
```

### 8.3 EditorBuildAndRunRequest

```text
EditorBuildAndRunRequest
  schema_version
  project_root
  profile_id
  target
  run_mode
  report_level
  timeout_ms
```

run_mode：

```text
UserWindowed:
  用户命令默认。
  启动 Game.exe 后不等待完整退出。

HeadlessVerification:
  自动化测试默认。
  复用 verify_exported_player_process。
  等待 child report 并返回 pass/fail。
```

### 8.4 EditorBuildAndRunReport

schema：

```text
editor-build-and-run-report.v1
```

字段：

```text
schema_version
status
project_root
profile_id
target
run_mode

desktop_export:
  status
  package_dir
  game_exe_path
  runtime_package_dir
  desktop_export_report_path
  diagnostic_count

launch:
  attempted
  started
  process_id
  working_dir
  executable_path
  args
  start_error

verification:
  attempted
  status
  exported_player_process_verification_report_path
  child_report_path
  process_exit_code
  child_player_exit_code

duration:
  export_duration_ms
  launch_duration_ms
  total_duration_ms

diagnostics:
  severity
  code
  domain
  stage
  path
  message
  next_action

artifacts:
  desktop_export_report
  process_verification_report
  child_player_report
```

status：

```text
not_started
export_failed
launch_failed
launched
verification_passed
verification_failed
environment_blocked
```

用户窗口启动成功：

```text
status = launched
```

自动化 headless verification 成功：

```text
status = verification_passed
```

### 8.5 Console 文案

成功：

```text
Build And Run launched: <package_dir>
Report: <editor-build-and-run-report.json>
```

Export 失败：

```text
Build failed before launch. Read desktop export report: <path>
```

Launch 失败：

```text
Build succeeded but Game.exe failed to launch. Read Build And Run report: <path>
```

## 9. 错误分类

必须结构化：

```text
editor.build_and_run.no_project
editor.build_and_run.unsupported_profile
editor.build_and_run.export_failed
editor.build_and_run.game_exe_missing
editor.build_and_run.launch_spawn_failed
editor.build_and_run.process_exited_immediately
editor.build_and_run.verification_failed
editor.build_and_run.report_write_failed
editor.build_and_run.previous_player_process_may_still_be_running
```

每个 diagnostic 必须包含：

```text
domain
stage
path
message
next_action
```

## 10. 与已有系统关系

### 10.1 与 129 的关系

```text
129 是 Build Export 面板和 Export 命令。
232 在 129 上增加 Build And Run 命令和报告。
不推翻 129。
```

新增后的 Build 面板：

```text
Export
Build & Run
Output
Report
```

### 10.2 与 217 的关系

```text
217 是 Editor Play / Preview RuntimePackage。
232 是 Desktop Build / Export / Run。
两者入口、频率、产物、报告都不同。
```

不能做：

```text
为了 Build And Run 快，把 217 preview cache 当成正式 Windows package。
```

### 10.3 与 231 的关系

```text
231 是 P0 golden gate。
232 是用户产品入口。
```

复用方式：

```text
232 的自动化测试可以复用 231 的 exported process contract。
232 的用户点击默认不跑完整 P0 golden 汇总。
```

原因：

```text
用户 Build And Run 需要快、明确、可操作。
Golden Gate 需要强验收、汇总证据、适合 CI / gate。
```

## 11. 复杂打飞机项目例子

### 11.1 用户正常 Build And Run

```text
打开 samples/complex_shooter_project。
修改 Scene / AUI / Rule 后保存。
点击 Build & Run。
Editor 调用 DesktopExportPipeline::export。
成功后启动 Build/Windows/dev/Game.exe。
Console 显示 package dir 与 report path。
Report Panel 出现 Latest Build And Run。
```

### 11.2 Export 失败

```text
例如 AUI binding path 或 Rule asset 结构错误。
DesktopExportPipeline 返回 failed。
Build And Run 不启动 Game.exe。
EditorBuildAndRunReport.status = export_failed。
next_action 指向 desktop-export-report.json 的首个 error。
```

### 11.3 Game.exe 缺失

```text
DesktopExportReport.player_executable_status = not_available。
Build And Run report:
  status = launch_failed
  diagnostic = editor.build_and_run.game_exe_missing
  next_action = Build runtime_cli / runtime player executable before Build And Run.
```

### 11.4 自动化验证

```text
project_e2e_gate 构造 EditorSession。
执行 BuildAndRunDesktopPackage in HeadlessVerification mode。
验证：
  DesktopExportReport success。
  verify_exported_player_process passed。
  EditorBuildAndRunReport links verification report。
```

## 12. 非目标

本轮不做：

```text
完整异步 build queue。
取消/暂停/重试 build。
Run Last Good。
Stop Built Player。
stdout/stderr live panel。
完整外部进程 session manager。
多平台 Build And Run。
installer / signing / store package。
release metadata / icon / final layout polish。
真实窗口 pixel golden。
自动打开系统文件管理器。
```

这些可以后续拆成：

```text
Editor Build Queue / Cancel / Progress v1
Editor External Player Process Session v1
Release Package Polish / Metadata / Icon / Layout v1
```

## 13. 预期施工 Gate

后续生成施工文档时，建议拆成：

### Gate A：命令与 UI model

目标：

```text
新增 UiCommandPayload::BuildAndRunDesktopPackage。
EditorCommandRegistry 注册命令。
BuildExportModel 增加 Build & Run command。
editor_input / editor_window_winit command mapping 接入。
```

测试：

```powershell
cd rust
cargo test -p editor_ui_model build_export
cargo test -p editor_input build_export
cargo test -p editor_core editor_command_registry
```

### Gate B：report schema / service

目标：

```text
新增 EditorBuildAndRunRequest / EditorBuildAndRunReport。
新增 EditorBuildAndRunService 或 build_service 内部窄接口。
export failed / game exe missing / launch failed fixture 覆盖。
```

测试：

```powershell
cd rust
cargo test -p editor_core build_and_run
```

### Gate C：Export + launched process

目标：

```text
BuildAndRunDesktopPackage 调用 DesktopExportPipeline。
成功后从 package_dir 启动 staged Game.exe。
用户模式记录 pid / working_dir / args，不等待完整退出。
```

测试：

```powershell
cd rust
cargo test -p editor_core build_service
```

### Gate D：headless verification e2e

目标：

```text
自动化模式复用 verify_exported_player_process。
project_e2e_gate 验证 complex shooter sample 可通过 Editor command build-and-run。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate editor_build_and_run
```

### Gate E：Report Panel / Console

目标：

```text
Report Panel 注册 build.and_run provider。
Console 输出成功/失败简短摘要和 report path。
AI context 可看到 last_build_and_run_report。
```

测试：

```powershell
cd rust
cargo test -p editor_core report_panel
cargo test -p editor_core ui_model
```

### Gate F：整体回归

目标：

```text
确认不破坏 129 / 217 / 231。
```

测试：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_core build
cargo test -p runtime_cli exported_player_process_verification
cargo test -p project_e2e_gate exported_windows_playable_golden
cargo test -p project_e2e_gate editor_build_and_run
```

## 14. 验收标准

必须满足：

```text
项目打开后 Build 面板显示 Build & Run。
无项目时 Build & Run disabled，并说明 Open a project first。
Build & Run 只支持 windows-dev。
Build & Run 先执行 DesktopExportPipeline::export。
Export failed 时不启动 Game.exe。
Export success 后从 package_dir 启动 staged Game.exe。
用户模式能输出 process_started / pid 或 launch_failed。
自动化模式能复用 verify_exported_player_process 并解析 child report。
EditorBuildAndRunReport 链接 DesktopExportReport。
Report Panel 能看到 Latest Build And Run。
Play 路径不调用 DesktopExportPipeline。
231 golden gate 不被每次用户 Build And Run 默认执行。
```

不允许用以下方式冒充完成：

```text
Build And Run 只等于 Export。
启动 rust/target/debug/ai_engine_runtime_cli.exe，而不是导出目录 Game.exe。
Export failed 后静默运行旧包。
用户点击 Build And Run 时强制跑完整 P0 golden gate。
Runtime 读取项目源目录或 EditorSession 内存。
Report 只写 Console 文本，没有结构化 schema。
```

## 15. 对用户和 AI 的心智

用户心智：

```text
Build And Run 会做一个 Windows dev 包，并启动它。
失败时我能看到是构建失败、exe 缺失，还是启动失败。
Play 是快速预览；Build And Run 是导出产物试运行。
```

AI 心智：

```text
要判断 Build And Run 是否真的成功，读 EditorBuildAndRunReport。
要定位构建错误，读 DesktopExportReport。
要定位启动/进程错误，读 launch / verification 字段。
不要猜 Console 文本。
不要绕过 DesktopExportPipeline。
不要把 PreviewPackage 当成导出包。
```

## 16. 自审

### 16.1 是否符合 227 优先级

```text
符合。
P0 已完成，当前进入 P1-1。
本方案直接服务“用户在编辑器里自由编辑复杂打飞机并 Windows 打包运行”的体验。
```

### 16.2 是否增加过多结构

```text
没有。
只新增一个 Editor 命令、一个 service/report，以及 Report Panel provider。
不新增新的 runtime 层、不新增 build pipeline、不新增 run session manager。
```

### 16.3 是否和 Play 混淆

```text
没有。
方案明确 Play 继续走 217 Preview RuntimePackage。
Build And Run 走 DesktopExportPipeline 和 staged Game.exe。
```

### 16.4 是否和 231 重复

```text
不重复。
231 是 heavy golden gate。
232 是用户命令产品化。
自动化测试可以复用 231 的 exported process contract，但用户点击默认不跑完整 golden 汇总。
```

### 16.5 是否适合复杂项目和 AI

```text
适合。
复杂项目需要知道失败发生在 Build / Package / Launch / Runtime 哪一层。
EditorBuildAndRunReport 能把这些层分开，让 AI 后续 patch 不靠猜。
```

### 16.6 是否控制效率

```text
是。
用户 Build And Run 只做必要 export + launch。
不默认跑完整 P0 golden。
不默认开启重 Trace。
```

## 17. 结论

`Editor Build And Run Productization v1` 的正确 v1 形态是：

```text
在 129 Build Export 面板上增加 Build & Run；
复用 DesktopExportPipeline 生成 Windows dev package；
只启动导出目录中的 Game.exe；
用 EditorBuildAndRunReport 汇总 export / launch / process / diagnostics；
自动化测试复用 231 exported process contract；
不把 Build And Run 和 Play / Golden Gate / Release Package 混成一个大系统。
```

这个系统完成后，复杂打飞机从“自动化能证明导出包可玩”进一步变成“用户在编辑器里可以一键导出并启动 Windows 可玩包”。
