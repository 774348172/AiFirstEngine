# 160-当前可自动化施工文档-Codebase Hygiene Stage H editor_ui_model domain 拆分

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
阶段完成记录/2026-07-03-Codebase-Hygiene-StageF3-EditorSession-command-service拆分/00-总览.md
```

本阶段治理：

```text
rust/crates/editor_ui_model/src/lib.rs
```

当前基线：

```text
editor_ui_model/src/lib.rs: 2289 lines
```

目标：

```text
把 UI model 纯数据结构按领域拆分。
保持 public type、serde schema、command payload 语义不变。
不引入 editor_core 依赖。
```

## 2. 目标结构

```text
editor_ui_model/src/lib.rs
editor_ui_model/src/model.rs
editor_ui_model/src/command.rs
editor_ui_model/src/workspace.rs
editor_ui_model/src/project_browser.rs
editor_ui_model/src/asset_browser.rs
editor_ui_model/src/input_mapping.rs
editor_ui_model/src/launcher.rs
editor_ui_model/src/build_export.rs
editor_ui_model/src/layout.rs
editor_ui_model/src/toolbar.rs
editor_ui_model/src/hierarchy.rs
editor_ui_model/src/inspector.rs
editor_ui_model/src/viewport.rs
editor_ui_model/src/console.rs
editor_ui_model/src/runtime_trace.rs
editor_ui_model/src/ai_panel.rs
editor_ui_model/src/diagnostics.rs
editor_ui_model/src/tests.rs
```

`lib.rs` 最终只做：

```text
mod 声明
pub use 重导出
少量 crate 级文档
```

## 3. 分阶段施工

### H-0：施工前基线

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageH-editor_ui_model-domain拆分\before.json
cargo test -p editor_ui_model
cargo test -p editor_core
```

### H-1：拆 root model / diagnostics / command

迁移：

```text
EditorUiModel
EditorCommandFeedback
EditorCommandFeedbackStatus
UiCommand
UiCommandPayload
UiCommandSource
EditorDiagnostic
DiagnosticSeverity
DiagnosticSource
command_id helper
```

目标文件：

```text
model.rs
command.rs
diagnostics.rs
```

验证：

```powershell
cargo test -p editor_ui_model command
cargo test -p editor_ui_model diagnostic
cargo test -p editor_core editor_command_registry
```

### H-2：拆 workspace / launcher / project browser

迁移：

```text
ProjectAuthoringWorkspaceModel
WorkspaceDomainKind
WorkspaceDomainSummary
WorkspaceDocumentSummary
WorkspaceSelectionSummary
ProjectLauncherModel
RecentProjectEntry
ProjectBrowserModel
ProjectBrowserEntry
```

目标文件：

```text
workspace.rs
launcher.rs
project_browser.rs
```

验证：

```powershell
cargo test -p editor_ui_model workspace
cargo test -p editor_core project_authoring_workspace
cargo test -p editor_core project_open_builds_project_browser_entries
```

### H-3：拆 asset / input / build

迁移：

```text
AssetBrowserModel
AssetBrowserEntry
AssetKind
AssetQuery
AssetSelection
AssetDragPayload
InputMappingAuthoringModel
InputMappingActionSummary
InputMappingBindingSummary
BuildExportModel
BuildExportReportSummary
```

目标文件：

```text
asset_browser.rs
input_mapping.rs
build_export.rs
```

验证：

```powershell
cargo test -p editor_ui_model asset
cargo test -p editor_ui_model input
cargo test -p editor_core editor_session_input_mapping_creates_edits_validates_and_updates_workspace
cargo test -p editor_core build_export_desktop_package_updates_report_model
```

### H-4：拆 panels / toolbar / hierarchy / inspector / viewport

迁移：

```text
PanelLayoutModel
PanelRegion
ToolbarModel
ToolbarCommand
HierarchyModel
HierarchyNode
InspectorModel
InspectorSection
InspectorField
InspectorValue
ViewportModel
EntitySummary
RenderableSummary
ColliderOverlayModel
```

目标文件：

```text
layout.rs
toolbar.rs
hierarchy.rs
inspector.rs
viewport.rs
```

验证：

```powershell
cargo test -p editor_ui_model panel
cargo test -p editor_ui_model inspector
cargo test -p editor_core editor_session_builds_schema_driven_property_tree_for_selected_entity
cargo test -p editor_window_winit native_editor_application_layout_computes_stable_panel_rects
```

### H-5：拆 console / runtime trace / AI panel / tests

迁移：

```text
ConsoleModel
ConsoleEntry
RuntimeTraceModel
RuntimeTraceEntry
AiPanelModel
AiPanelMessage
AiProposedCommand
```

目标文件：

```text
console.rs
runtime_trace.rs
ai_panel.rs
tests.rs
```

验证：

```powershell
cargo test -p editor_ui_model
cargo test -p editor_core tick_one_frame_updates_viewport_and_trace_model
cargo test -p editor_core editor_session_ai_prompt_proposes_command
```

### H-6：整体回归

```powershell
cargo fmt
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p code_hygiene
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageH-editor_ui_model-domain拆分\after.json
```

## 4. 不做什么

```text
不改 JSON 字段名。
不改 UiCommandPayload 变体。
不引入 editor_core 依赖。
不新增 UI 功能。
不移动 renderer 代码。
```

## 5. 验收标准

```text
editor_ui_model/src/lib.rs 收敛为 thin facade。
editor_ui_model 不再是 2000+ 行单文件 crate。
所有下游 crate 编译和测试通过。
serde roundtrip 测试保持通过。
```

## 6. 施工文档自审

```text
Specification fit:
  覆盖 F3 后明确剩余热点 editor_ui_model。
Rule fit:
  遵守 UI model 只承载数据真相，不引入业务执行。
Textual consistency:
  拆分目标与 151 的 editor_ui_model 目标结构一致。
Design fit:
  提升 AI 检索和复杂编辑器长期维护能力。
Implementation feasibility:
  lib.rs 内类型按 UI 领域天然分区，适合机械迁移和重导出。
Practical reasonableness:
  每组迁移都有 editor_ui_model 与下游 editor_core/editor_window_winit 验证。
```
