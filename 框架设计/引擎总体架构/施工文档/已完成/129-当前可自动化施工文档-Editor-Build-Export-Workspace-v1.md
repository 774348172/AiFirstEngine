# 129-当前可自动化施工文档-Editor Build / Export Workspace v1

## 施工目标

把 `128` 已完成的 `DesktopExportPipeline` 接入 Native Editor 工作区，形成可点击、可报告、可测试的 Build / Export 面板。

第一版只做：

```text
windows-dev profile
ExportDesktopPackage
OpenBuildOutput
OpenBuildReport
BuildExportModel
SelfUiRenderer Build Export panel
```

## 涉及文件

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/authoring_workspace.rs
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
```

## Gate 1：UI Model

任务：

```text
新增 BuildExportModel / BuildProfileSummary / BuildExportReportSummary / BuildExportCommand。
EditorUiModel 增加 build_export 字段。
UiCommandPayload 增加 ExportDesktopPackage / OpenBuildOutput / OpenBuildReport。
```

测试：

```powershell
cargo test -p editor_ui_model
```

## Gate 2：Input Router

任务：

```text
Build Export panel 按钮使用统一 ToolbarCommand hit target。
EditorInputRouter 把 export_desktop_package / open_build_output / open_build_report 转成 UiCommandPayload。
```

测试：

```powershell
cargo test -p editor_input
```

## Gate 3：EditorSession 接入 DesktopExportPipeline

任务：

```text
EditorSession 保存 last_desktop_export_report。
build_ui_model 暴露 BuildExportModel。
ExportDesktopPackage 调用 DesktopExportPipeline::export。
未打开项目时返回 rejected diagnostic。
导出结果写入 console summary。
```

测试：

```powershell
cargo test -p editor_core
```

## Gate 4：Renderer 面板与 HitRegion

任务：

```text
SelfUiRenderer 增加 Build Export panel。
显示 profile、status、diagnostics、player exit reason。
提供 Export / Output / Report hit regions。
```

测试：

```powershell
cargo test -p editor_ui_renderer
```

## Gate 5：Window Shell 命令注册

任务：

```text
editor_window_winit command_id_for_shell_payload 支持新增 payload。
标准 command registry 注册 export / open output / open report。
```

测试：

```powershell
cargo test -p editor_window_winit
```

## 完成后回归

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_input
cargo test -p editor_core
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit
cargo test -p editor_host
```

## 状态

已完成。完成记录见：

```text
阶段完成记录/2026-07-01-Editor-Build-Export-Workspace-v1/00-总览.md
```
