# 232-当前可自动化施工文档-Editor Build And Run Productization v1

> 状态：当前唯一施工文档。  
> 来源方案：`232-Editor-Build-And-Run-Productization-v1方案.md`。  
> 采用方案：`B-min`。  
> 施工日期：2026-07-09。  
> 施工完成前，不开启其它施工文档。

## 1. 施工目标

把编辑器 Build 面板从：

```text
Export / Output / Report
```

推进到：

```text
Export / Build & Run / Output / Report
```

并建立真实链路：

```text
BuildAndRunDesktopPackage
  -> DesktopExportPipeline::export
  -> staged Build/Windows/dev/Game.exe
  -> user launch or headless verification
  -> EditorBuildAndRunReport
  -> Console / Report Panel / project_e2e_gate
```

## 2. 范围边界

本轮必须做：

```text
UiCommandPayload::BuildAndRunDesktopPackage。
BuildExportModel 增加 Build & Run 命令。
editor_input / editor_window_winit command id 映射。
EditorBuildAndRunReport schema。
EditorSession Build And Run service。
Report Panel build.and_run provider。
project_e2e_gate editor_build_and_run 验证。
分 Gate 测试和整体回归。
```

本轮不做：

```text
完整异步 build queue。
取消/暂停/重试 build。
Run Last Good。
Stop Built Player。
stdout/stderr live panel。
完整外部进程 session manager。
多平台 Build And Run。
Release Package polish / installer / signing / store package。
真实窗口 pixel golden。
自动打开系统文件管理器。
```

## 3. 涉及文件

预计修改：

```text
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/build_export.rs
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_ui_model/src/tests.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/command_system.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/build_service_tests.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/project_e2e_gate/src/editor_build_and_run.rs
```

可能需要修改：

```text
rust/crates/editor_ui_renderer/src/tests.rs
rust/crates/editor_ui_renderer/src/*
rust/crates/editor_input/src/*
```

文档同步：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-09-Editor-Build-And-Run-Productization-v1/00-总览.md
```

## 4. Gate A：命令与 UI model

目标：

```text
新增 UiCommandPayload::BuildAndRunDesktopPackage { profile_id }。
BuildExportModel 增加 Build & Run command。
command id 统一为 build_and_run_desktop_package。
Workspace domain / command summary / editor input / winit command mapping 接入。
```

测试：

```powershell
cd rust
cargo test -p editor_ui_model build_export
cargo test -p editor_input build_export
cargo test -p editor_core editor_command_registry
```

完成标准：

```text
项目打开后 Build 面板模型含 Build & Run。
无项目时 Build & Run disabled，reason 为 Open a project first.
命令 roundtrip / input router / command registry 均能识别。
```

## 5. Gate B：report schema / service

目标：

```text
新增 EditorBuildAndRunReport 及 nested summary。
新增 last_build_and_run_report。
新增 build_and_run_desktop_package service。
支持 unsupported profile / no project / export failed / game exe missing。
```

测试：

```powershell
cd rust
cargo test -p editor_core build_and_run
```

完成标准：

```text
Export failed 不启动 Game.exe。
Game.exe 缺失时 status=launch_failed。
Report 写入 Build/Windows/dev/reports/editor-build-and-run-report.json。
```

## 6. Gate C：Export + user launch

目标：

```text
BuildAndRunDesktopPackage 先执行 DesktopExportPipeline。
成功后从 DesktopExportReport.player_executable 启动 staged Game.exe。
工作目录为 DesktopExportReport.package_dir。
用户模式记录 attempted / started / pid / executable_path / working_dir。
```

测试：

```powershell
cd rust
cargo test -p editor_core build_service
```

完成标准：

```text
默认用户命令不启动 target/debug runtime_cli。
默认用户命令不跑完整 231 golden。
如果 spawn 成功，status=launched；spawn 失败，status=launch_failed。
```

## 7. Gate D：headless verification e2e

目标：

```text
为测试提供 HeadlessVerification 路径。
复用 runtime_cli::verify_exported_player_process。
project_e2e_gate 新增 editor_build_and_run report/test。
```

测试：

```powershell
cd rust
cargo test -p runtime_cli exported_player_process_verification
cargo test -p project_e2e_gate editor_build_and_run
```

完成标准：

```text
complex shooter sample 经 EditorSession command 导出并启动 staged Game.exe。
EditorBuildAndRunReport 链接 ExportedPlayerProcessVerificationReport。
验证报告 status=verification_passed。
```

## 8. Gate E：Report Panel / Console

目标：

```text
Report Panel 注册 build.and_run provider。
Console 输出成功/失败摘要与 report path。
BuildExportReportSummary 或 BuildExportModel 可显示最近 Build And Run 状态。
```

测试：

```powershell
cd rust
cargo test -p editor_core report_panel
cargo test -p editor_core ui_model
```

完成标准：

```text
Report Panel 能看到 Latest Build And Run。
Console 不只输出裸文本，结构化 report 是事实来源。
```

## 9. Gate F：整体回归

目标：

```text
确认 129 / 217 / 231 未被破坏。
确认 Play 不调用 DesktopExportPipeline。
确认 Build And Run 不默认跑完整 231 golden。
```

测试：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_core build
cargo test -p runtime_cli exported_player_process_verification
cargo test -p project_e2e_gate exported_windows_playable_golden
cargo test -p project_e2e_gate editor_build_and_run
cargo test -p project_e2e_gate
```

## 10. 文档同步与归档

全部 Gate 通过后：

```text
生成阶段完成记录：
  阶段完成记录/2026-07-09-Editor-Build-And-Run-Productization-v1/00-总览.md

更新入口：
  49-当前状态与下一步入口.md
  54-当前可自动化施工总入口.md
  施工文档/README.md
  阶段完成记录/README.md

归档施工文档：
  从 施工文档/当前/
  移动到 施工文档/已完成/
```

归档后：

```text
施工文档/当前/ 必须为空。
下一步仍按 227 选择 P1 后续系统。
```

## 11. 施工自审

```text
是否读取正式方案：
  是，已读取 232 方案全文。

是否读取外部审查文档：
  本轮用户未提供 232 对应外部审查文档；无需读取。若施工中出现外部审查文档，必须暂停并按 skill 流程吸收。

是否保持当前唯一施工文档：
  是，生成前确认 施工文档/当前/ 为空。

是否扩大范围：
  否。本轮不做 build queue、Stop Built Player、Run Last Good、release polish、完整进程管理器。

是否和 232 冲突：
  否。Build And Run 走 DesktopExportPipeline 和 staged Game.exe；Play 继续走 217 Preview RuntimePackage；231 golden 不作为用户点击默认流程。

是否有分 Gate 测试：
  是。Gate A-F 均列出测试命令，完成后必须跑整体回归。

是否能证明完成：
  是。以 EditorBuildAndRunReport、Report Panel entry、project_e2e_gate editor_build_and_run 和 runtime_cli process verification 为证据。
```
