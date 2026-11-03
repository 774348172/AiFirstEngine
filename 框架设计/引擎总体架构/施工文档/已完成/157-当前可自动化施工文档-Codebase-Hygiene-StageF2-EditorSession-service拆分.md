# 157-当前可自动化施工文档-Codebase Hygiene Stage F2 EditorSession service 拆分

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
49-当前状态与下一步入口.md
阶段完成记录/2026-07-02-Codebase-Hygiene-StageF-editor_core-EditorSession-service拆分/00-总览.md
```

本阶段执行：

```text
Codebase Architecture Hygiene Gate v1 - Stage F2
```

Stage F2 的目标是：

```text
继续拆 editor_core/src/session.rs。
保持 EditorSession 作为 public facade。
把 transaction 类型、UI model composition、project/runtime/scene 纯 helper 从 session.rs 中迁出。
不改变 EditorSession public API。
不改变 UiCommandPayload -> EditorSession 执行行为。
不新增编辑器功能。
```

## 2. 当前基线

Stage F1 后的热点：

```text
rust/crates/editor_core/src/session.rs: 3361 lines
rust/crates/editor_core/src/scene_editing.rs: 2404 lines
rust/crates/editor_ui_model/src/lib.rs: 2289 lines
rust/crates/editor_core/src/tests.rs: 1845 lines
rust/crates/editor_core/src/inspector_details.rs: 1644 lines
```

`session.rs` 当前承载：

```text
CommandStatus / UndoPolicy / StateChangeSummary / CommandTransaction / CommandResult
EditorSession state
EditorSession command dispatch
project / input mapping / runtime / scene / play / build / AI command methods
UI model composer methods
diagnostic helper methods
path / vec3 / diagnostics / report helper functions
```

## 3. 本轮目标结构

新增或调整：

```text
rust/crates/editor_core/src/transaction.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/services/mod.rs
rust/crates/editor_core/src/services/project_service.rs
rust/crates/editor_core/src/services/runtime_service.rs
rust/crates/editor_core/src/services/scene_service.rs
```

目标职责：

```text
transaction.rs:
  CommandStatus / UndoPolicy / StateChangeSummary / CommandTransaction / CommandResult
  command_for_test

ui_model_composer.rs:
  impl EditorSession 中只负责 build_ui_model 与各 panel / workspace / inspector / viewport model composition 的方法
  UI-only helper，例如 toolbar_command / inspector_field / viewport_from_snapshot / collider overlay conversion

services/project_service.rs:
  project-relative path 和 input mapping path 判断等 project-side pure helper

services/runtime_service.rs:
  Runtime diagnostics -> EditorDiagnostic 等 runtime pure helper

services/scene_service.rs:
  scene edit source / scene diagnostics / asset placement diagnostics / vec3 conversion 等 scene pure helper
```

`session.rs` 保留：

```text
EditorSession state
execute_command / execute_ui_command_direct
project/runtime/scene/play/build/AI command method body
begin_transaction / finish_transaction
push_info / push_error / make_diagnostic
```

## 4. 本轮不做什么

```text
不把 execute_ui_command_direct 拆成 dispatcher trait。
不把 project/runtime/scene 命令方法整体迁出。
不改变 transaction schema。
不改变 EditorSession public API。
不改变 UI model 输出字段。
不改变测试断言。
不引入项目侧 gameplay 概念。
不做 engine_runtime domain grouping。
```

原因：

```text
F2 是 EditorSession service 拆分第一轮。
如果本轮直接移动所有 command method，会同时改变太多私有调用边界，测试失败时定位成本会变高。
先迁出类型、UI composition 和纯 helper，可以降低 session.rs 体量，同时给后续 F3 的 service method 迁移建立模块落点。
```

## 5. 分阶段施工

### F2-0：施工前基线

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageF2-EditorSession-service拆分\before.json
cargo test -p editor_core
```

### F2-1：拆 transaction.rs

移动：

```text
CommandStatus
UndoPolicy
StateChangeSummary
CommandTransaction
CommandResult
command_for_test
```

要求：

```text
crate root 继续 pub use 这些类型。
外部调用路径不变。
```

验证：

```powershell
cargo test -p editor_core tests::clear_console_is_snapshot_ready_transaction
cargo test -p editor_core tests::scene_edit_test_api_matches_ui_command_path
cargo test -p editor_core
```

### F2-2：拆 ui_model_composer.rs

移动：

```text
build_ui_model
build_toolbar_model
build_project_launcher_model
build_project_browser_model
build_asset_browser_model
build_input_mapping_authoring_model
build_project_authoring_workspace_model
workspace domain composer
build_export_model
build_hierarchy_model
build_inspector_model
build_viewport_model
build_collider_overlay_model
build_console_model
build_runtime_trace_model
build_ai_panel_model
last_trace
selected_entity_summary
UI-only helper functions
```

要求：

```text
EditorSession::build_ui_model public API 不变。
UI model JSON/schema 行为不变。
```

验证：

```powershell
cargo test -p editor_core tests::editor_session_creates_empty_ui_model
cargo test -p editor_core tests::ui_model_after_scene_open_has_scene_hierarchy
cargo test -p editor_core tests::project_authoring_workspace_model_summarizes_open_project_domains
cargo test -p editor_core
```

### F2-3：拆 services pure helper

移动：

```text
normalize_project_relative_path -> services/project_service.rs
is_input_mapping_relative_path -> services/project_service.rs
runtime_diagnostics_to_editor -> services/runtime_service.rs
ui_vec3_to_editor / editor_vec3_to_ui / to_ui_vec3 -> services/scene_service.rs
scene_edit_source_for_ui_source -> services/scene_service.rs
scene_diagnostics_to_editor -> services/scene_service.rs
asset_placement_diagnostics_to_editor -> services/scene_service.rs
```

要求：

```text
本轮只迁移 pure helper，不迁移 command method。
services 目录先作为后续 F3 的落点，不提前抽象 trait。
```

验证：

```powershell
cargo test -p editor_core tests::editor_session_execute_command_scene_sets_transform
cargo test -p editor_core tests::editor_session_place_asset_into_scene_creates_selected_renderable_entity
cargo test -p editor_core tests::tick_without_runtime_package_returns_structured_diagnostic
cargo test -p editor_core
```

### F2-4：最终回归与归档

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageF2-EditorSession-service拆分\after.json
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p code_hygiene
```

完成后：

```text
生成 阶段完成记录/2026-07-03-Codebase-Hygiene-StageF2-EditorSession-service拆分/00-总览.md
移动本施工文档到 施工文档/已完成/
更新 00-文档地图 / 49 / 54 / 施工文档 README / 阶段完成记录 README / 149 问题队列
```

## 6. 验收标准

```text
editor_core/src/session.rs 行数显著下降。
transaction.rs 成为 transaction 类型唯一承载点。
ui_model_composer.rs 成为 UI model composition 的主要承载点。
services/project_service.rs / runtime_service.rs / scene_service.rs 存在并承载对应 pure helper。
EditorSession public API 不变。
editor_core / editor_window_winit / editor_host / code_hygiene 回归通过。
```

## 7. 施工文档自审

```text
Specification fit:
  满足用户要求：先生成 Stage F2 施工文档，审查后直接施工，施工完成后测试。

Rule fit:
  继承 151 和 Stage F1 完成记录，不重新讨论已确认系统，不新增项目侧规则。

Textual consistency:
  本文明确 F2 只迁移 transaction、UI composer 和 pure helper，不宣称完成最终 service 化。

Design fit:
  符合长期主义：先建立稳定模块落点，降低 session.rs 体量，为 F3 迁移 command service 做准备。

Implementation feasibility:
  Rust 允许在独立模块中继续 `impl EditorSession`；现有字段为 pub(crate)，迁移 UI composer 可行。

Practical reasonableness:
  每个阶段有定向测试和整体回归；不一次性移动所有 command method，避免行为回归难定位。
```

结论：

```text
本施工文档通过自审，可以开始 Stage F2 施工。
```
