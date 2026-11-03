# 158-当前可自动化施工文档-Codebase Hygiene Stage F3 EditorSession command service 拆分

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
49-当前状态与下一步入口.md
阶段完成记录/2026-07-03-Codebase-Hygiene-StageF2-EditorSession-service拆分/00-总览.md
```

本阶段执行：

```text
Codebase Architecture Hygiene Gate v1 - Stage F3
```

目标：

```text
继续拆 editor_core/src/session.rs。
保持 EditorSession public facade。
保持 execute_command / execute_ui_command_direct 作为统一命令入口。
把 project / runtime / scene command method 迁入 services/*。
不改变 UiCommandPayload 行为。
不新增编辑器功能。
```

## 2. 当前基线

Stage F2 后：

```text
editor_core/src/session.rs: 1914 lines
editor_core/src/ui_model_composer.rs: 1280 lines
editor_core/src/transaction.rs: 64 lines
editor_core/src/services/project_service.rs: 12 lines
editor_core/src/services/runtime_service.rs: 28 lines
editor_core/src/services/scene_service.rs: 93 lines
```

`session.rs` 当前仍承载：

```text
execute_ui_command_direct dispatcher
project command methods
input mapping command methods
runtime package / tick / trace command methods
scene edit / selection / save command methods
play command methods
build command methods
AI command methods
transaction finish / diagnostics helper
```

## 3. 本轮目标结构

本轮只迁移以下 command method：

```text
services/project_service.rs:
  create_project
  open_project
  select_recent_project
  refresh_recent_projects
  select_project_browser_entry
  open_project_browser_entry
  create_default_input_mapping
  edit_input_mapping
  validate_input_mapping
  set_workspace_view_mode
  open_scene_document_for_launcher

services/runtime_service.rs:
  open_runtime_package
  reload_runtime_package
  tick_one_frame
  reset_runtime
  select_trace_entry

services/scene_service.rs:
  open_scene_document
  execute_scene_edit
  place_asset_into_scene
  save_scene_document
  select_entity
```

仍留在 `session.rs`：

```text
EditorSession struct / Default
new / execute_command / execute_ui_command_direct
test helper public methods
scene_dirty / active_project_session / project_launcher_state / recent project load/save wrappers
begin_transaction / reject_unknown_editor_command / finish_transaction
play command methods
build command methods
AI command methods
clear_console
push_info / push_error / push_play_session_report / make_diagnostic
```

## 4. 本轮不做什么

```text
不把 execute_ui_command_direct dispatcher 拆走。
不做 trait 抽象。
不迁 build/play/AI command methods。
不改变 public API。
不改变 transaction schema。
不改 tests 语义。
不引入 gameplay 概念。
```

原因：

```text
F3 是 command service 迁移第一轮。
project/runtime/scene 是当前最清晰的 domain 边界。
build/play/AI 依赖 project/runtime/diagnostic/AI panel 状态较多，留到 F4 更稳。
```

## 5. 分阶段施工

### F3-0：施工前基线

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageF3-EditorSession-command-service拆分\before.json
cargo test -p editor_core
```

### F3-1：迁移 project command service

移动到：

```text
rust/crates/editor_core/src/services/project_service.rs
```

验证：

```powershell
cargo test -p editor_core tests::project_open_builds_project_browser_entries
cargo test -p editor_core tests::project_browser_open_scene_document_refreshes_hierarchy
cargo test -p editor_core tests::editor_session_input_mapping_creates_edits_validates_and_updates_workspace
cargo test -p editor_core
```

### F3-2：迁移 runtime command service

移动到：

```text
rust/crates/editor_core/src/services/runtime_service.rs
```

验证：

```powershell
cargo test -p editor_core tests::open_runtime_package_builds_hierarchy_model
cargo test -p editor_core tests::tick_one_frame_updates_viewport_and_trace_model
cargo test -p editor_core tests::editor_session_play_runs_headless_session_when_package_loaded
cargo test -p editor_core
```

### F3-3：迁移 scene command service

移动到：

```text
rust/crates/editor_core/src/services/scene_service.rs
```

验证：

```powershell
cargo test -p editor_core tests::editor_session_execute_command_scene_opens_scene_document
cargo test -p editor_core tests::editor_session_execute_command_scene_sets_transform
cargo test -p editor_core tests::editor_session_place_asset_into_scene_creates_selected_renderable_entity
cargo test -p editor_core tests::editor_session_save_scene_clears_dirty
cargo test -p editor_core
```

### F3-4：最终回归与归档

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageF3-EditorSession-command-service拆分\after.json
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p code_hygiene
```

完成后：

```text
生成 阶段完成记录/2026-07-03-Codebase-Hygiene-StageF3-EditorSession-command-service拆分/00-总览.md
移动本施工文档到 施工文档/已完成/
更新 00-文档地图 / 49 / 54 / 施工文档 README / 阶段完成记录 README / 149 问题队列
```

## 6. 验收标准

```text
project/runtime/scene command method 不再堆在 session.rs。
EditorSession public facade 不变。
execute_command / execute_ui_command_direct 行为不变。
editor_core / editor_window_winit / editor_host / code_hygiene 回归通过。
session.rs 行数继续下降。
```

## 7. 施工文档自审

```text
Specification fit:
  满足用户要求：Stage F3 先生成施工文档，然后直接施工。

Rule fit:
  延续 151、F1、F2，不重新讨论已确认系统；不新增 gameplay API。

Textual consistency:
  本文明确只迁 project/runtime/scene command service，不宣称完成全部 EditorSession service 化。

Design fit:
  符合长期主义：把命令执行职责按 domain 归口，EditorSession 保持外部 facade。

Implementation feasibility:
  Rust 允许 services 模块中继续 `impl EditorSession`；F2 已把字段设为 pub(crate)，具备迁移条件。

Practical reasonableness:
  分三组迁移、每组测试；build/play/AI 留到后续，降低回归定位风险。
```

结论：

```text
本施工文档通过自审，可以开始 Stage F3 施工。
```
