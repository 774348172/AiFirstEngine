# 86-当前可自动化施工文档：真实 UI 命令接入 Scene Editing C-min

## 目标

按 `86-真实UI命令接入SceneEditing-C-min方案.md` 实现第一版真实 UI 命令接入。

本轮目标不是做完整 Scene 编辑器，也不是做真实 Gizmo，而是把已经完成的 85 Scene Editing 内核接入真实 `UiCommandPayload -> EditorSession::execute_command` 路径：

```text
Toolbar / Hierarchy / Inspector / Viewport / AI
  -> UiCommandPayload
  -> EditorSession
  -> SceneEditCommand
  -> SceneEditTransaction
  -> EditorSceneDocument
  -> PreviewWorldSync
  -> EditorUiModel refresh
  -> Console / Diagnostic
```

## 前置基础

必须复用：

```text
85-Scene-Editing-v1-C-min方案.md
86-真实UI命令接入SceneEditing-C-min方案.md

rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_host/src/lib.rs
```

已经存在的 85 能力：

```text
EditorSceneDocument
SceneEditCommand
SceneEditTransaction
SceneDirtyState
SceneUndoStack
PreviewWorldSync full rebuild
SceneSavePipeline
EditorSession headless Scene Editing API
```

## 本轮边界

本轮必须实现：

```text
扩展 editor_ui_model::UiCommandPayload。
EditorSession::execute_command 接入真实 Scene UI 命令。
OpenSceneDocument / SelectSceneEntity / CreateSceneEntity / DeleteSceneEntity。
SetSceneTransform / SetSceneComponentField。
SaveSceneDocument / UndoSceneEdit / RedoSceneEdit。
EditorInput / EditorHost 最小命令路由接入。
Toolbar / Hierarchy / Inspector / Viewport 的 headless 测试覆盖。
Console diagnostic 和 UI model refresh 测试覆盖。
```

本轮不实现：

```text
真实 Viewport Gizmo。
复杂 picking。
复杂 Inspector schema renderer。
Prefab Mode。
Asset Browser drag into Scene。
复杂 reparent drag-drop。
Duplicate UI command。
多选批量编辑。
```

## 施工原则

```text
UI 层只表达用户意图，不直接写 EditorSceneDocument。
EditorSession 是真实 UI 进入 Scene Editing 的唯一入口。
所有 Scene 源数据修改仍然走 SceneEditTransaction。
Toolbar 不直接保存 Scene 文件，保存必须走 SceneSavePipeline。
Hierarchy 不直接改 Entity 树。
Inspector 不直接写 Transform 或组件字段。
Viewport 第一版只接选择，不接真实 Gizmo。
AI 仍然只生成 SceneEditCommand，但必须由 EditorSession 执行。
```

## 建议修改文件

优先修改：

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_host/src/lib.rs
```

尽量不修改：

```text
rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_ui_backend_egui/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
```

除非测试表明必须调整 85 内核，否则不要改 `scene_editing.rs` 的核心规则。

## A1：扩展 UiCommandPayload

修改：

```text
rust/crates/editor_ui_model/src/lib.rs
```

新增命令：

```text
UiCommandPayload::OpenSceneDocument { path: String }
UiCommandPayload::SelectSceneEntity { entity_id: String }
UiCommandPayload::CreateSceneEntity { parent_id: Option<String>, name: String }
UiCommandPayload::DeleteSceneEntity { entity_id: String }
UiCommandPayload::SetSceneTransform {
  entity_id: String,
  local_position: Option<Vec3>,
  local_rotation: Option<Vec3>,
  local_scale: Option<Vec3>
}
UiCommandPayload::SetSceneComponentField {
  entity_id: String,
  component_type: String,
  field_path: String,
  value: serde_json::Value
}
UiCommandPayload::SaveSceneDocument { path: Option<String> }
UiCommandPayload::UndoSceneEdit
UiCommandPayload::RedoSceneEdit
```

规则：

```text
UiCommandPayload 只表达 UI 意图。
UiCommandPayload 不直接包含 TransactionReport。
UiCommandPayload 不直接修改 EditorSceneDocument。
```

测试：

```text
ui_command_payload_serializes_open_scene_document
ui_command_payload_serializes_set_scene_transform
ui_command_payload_serializes_set_scene_component_field
ui_command_payload_scene_edit_commands_are_ai_readable
```

运行：

```powershell
cargo test -p editor_ui_model ui_command_payload
```

## A2：EditorSession::execute_command 接入 Scene UI 命令

修改：

```text
rust/crates/editor_core/src/lib.rs
```

将以下 `UiCommandPayload` 映射到已有 85 内核：

```text
OpenSceneDocument
  -> open_scene_document

SelectSceneEntity
  -> SceneEditCommand::SelectEntity

CreateSceneEntity
  -> SceneEditCommand::CreateEntity

DeleteSceneEntity
  -> SceneEditCommand::DeleteEntity

SetSceneTransform
  -> SceneEditCommand::SetTransform

SetSceneComponentField
  -> SceneEditCommand::SetComponentField

SaveSceneDocument
  -> save_scene_document

UndoSceneEdit
  -> SceneEditCommand::Undo

RedoSceneEdit
  -> SceneEditCommand::Redo
```

规则：

```text
真实 UI 命令和 headless/test API 必须走同一套内部实现。
不要保留两条不同的 Scene Editing 执行逻辑。
旧 runtime package 命令保持兼容。
OpenRuntimePackage 不等于 OpenSceneDocument。
SaveSceneDocument 不能绕过 SceneSavePipeline。
```

测试：

```text
editor_session_execute_command_opens_scene_document
editor_session_execute_command_selects_scene_entity
editor_session_execute_command_creates_scene_entity
editor_session_execute_command_deletes_scene_entity
editor_session_execute_command_sets_scene_transform
editor_session_execute_command_sets_component_field
editor_session_execute_command_saves_scene_document
editor_session_execute_command_undo_redo_scene_edit
editor_session_execute_command_rejects_scene_edit_without_open_scene
```

运行：

```powershell
cargo test -p editor_core editor_session_execute_command_scene
```

## A3：收敛 headless/test API

修改：

```text
rust/crates/editor_core/src/lib.rs
```

当前 85 中存在：

```text
open_scene_document_for_test
execute_scene_edit_for_test
save_scene_document_for_test
```

本轮规则：

```text
这些 test API 可以保留，但内部必须调用真实 UiCommandPayload 路径，或与 execute_command 共用同一个私有实现。
不允许 test API 成为第二套正式 Scene Editing 入口。
```

测试：

```text
scene_edit_test_api_matches_ui_command_path
scene_save_test_api_matches_ui_command_path
```

运行：

```powershell
cargo test -p editor_core scene_edit_test_api
```

## A4：EditorInput 最小路由接入

修改：

```text
rust/crates/editor_input/src/lib.rs
```

第一版最小路由：

```text
Hierarchy hit entity
  -> UiCommandPayload::SelectSceneEntity

Toolbar save_scene
  -> UiCommandPayload::SaveSceneDocument

Toolbar undo_scene_edit
  -> UiCommandPayload::UndoSceneEdit

Toolbar redo_scene_edit
  -> UiCommandPayload::RedoSceneEdit

Viewport selected_entity hit
  -> UiCommandPayload::SelectSceneEntity
```

如果现有 hit-test / router 没有对应区域，允许只新增 headless route helper，不做真实 UI 坐标扩展。

测试：

```text
router_maps_hierarchy_hit_to_select_scene_entity
router_maps_toolbar_save_to_save_scene_document
router_maps_toolbar_undo_to_undo_scene_edit
router_maps_toolbar_redo_to_redo_scene_edit
router_maps_viewport_pick_to_select_scene_entity
```

运行：

```powershell
cargo test -p editor_input scene_edit
```

## A5：EditorHost 最小端到端接入

修改：

```text
rust/crates/editor_host/src/lib.rs
```

目标：

```text
EditorHost 能通过真实 UiCommandPayload 打开 Scene。
EditorHost 能通过真实 UiCommandPayload 选择 Entity。
EditorHost 能通过真实 UiCommandPayload 修改 Transform。
EditorHost 能通过真实 UiCommandPayload 保存 Scene。
```

规则：

```text
EditorHost 不直接写 EditorSceneDocument。
EditorHost 不直接调用 SceneEditTransaction。
EditorHost 只把 UiCommand 交给 EditorSession。
```

测试：

```text
editor_host_routes_open_scene_document_to_editor_core
editor_host_routes_hierarchy_select_to_scene_editing
editor_host_routes_inspector_transform_to_scene_editing
editor_host_routes_save_scene_to_scene_save_pipeline
```

运行：

```powershell
cargo test -p editor_host scene_edit
```

## A6：Inspector 最小字段提交规则

修改优先级：

```text
优先通过 editor_ui_model / editor_input 测试字段命令。
不急着做真实渲染控件。
```

第一版字段规则：

```text
Vec3 单轴变化也进入 SetSceneTransform。
数字输入 Enter / blur 提交。
Checkbox / Toggle 点击即提交。
组件 JSON 字段第一版只支持单层 field_path。
非法字段由 EditorCore 拒绝，Console 显示 diagnostic。
```

测试：

```text
inspector_vec3_x_change_emits_set_scene_transform
inspector_component_field_change_emits_set_scene_component_field
inspector_invalid_component_field_reports_console_error
```

运行：

```powershell
cargo test -p editor_input inspector
cargo test -p editor_core inspector_scene_edit
```

## A7：Toolbar / Hierarchy / Inspector UI Model 刷新测试

修改：

```text
rust/crates/editor_core/src/lib.rs
```

测试：

```text
ui_model_after_open_scene_document_has_scene_hierarchy
ui_model_after_create_scene_entity_refreshes_hierarchy
ui_model_after_select_scene_entity_refreshes_inspector
ui_model_after_set_scene_transform_refreshes_viewport
ui_model_after_save_scene_document_clears_dirty
ui_model_after_failed_scene_edit_adds_console_error
```

运行：

```powershell
cargo test -p editor_core ui_model_after_scene
```

## A8：兼容旧 Runtime Package 命令

必须保证以下旧测试继续通过：

```text
open_runtime_package_builds_hierarchy_model
select_entity_updates_readonly_inspector
tick_one_frame_updates_viewport_and_trace_model
editor_session_play_runs_headless_session_when_package_loaded
```

新增测试：

```text
open_scene_document_does_not_break_runtime_package_open
open_runtime_package_does_not_mutate_editor_scene_document
```

运行：

```powershell
cargo test -p editor_core open_runtime_package
cargo test -p editor_core editor_session_play
```

## A9：AI 入口保持 SceneEditCommand

本轮不实现完整 AI 面板，但必须保留规则：

```text
AI 可以生成 SceneEditCommand。
AI 生成的 SceneEditCommand 必须由 EditorSession 执行。
AI 不直接发 UiCommandPayload，除非它是在模拟 UI 行为。
AI 不直接写 EditorSceneDocument。
```

测试：

```text
ai_scene_edit_command_uses_editor_session_transaction_path
ai_scene_edit_command_failure_reports_console_diagnostic
```

运行：

```powershell
cargo test -p editor_core ai_scene_edit_command
```

## A10：Headless smoke 与 workspace 测试

必须运行：

```powershell
cargo test -p editor_ui_model ui_command_payload
cargo test -p editor_core editor_session_execute_command_scene
cargo test -p editor_core scene_edit_test_api
cargo test -p editor_input scene_edit
cargo test -p editor_host scene_edit
cargo test -p editor_core ui_model_after_scene
cargo test -p editor_core open_runtime_package
cargo test -p editor_core editor_session_play
cargo test -p editor_core ai_scene_edit_command
cargo test -p editor_core
cargo test --workspace
```

如果 `cargo test --workspace` 因历史无关问题失败，必须记录失败原因和已通过的更小测试命令。

## A11：完成后文档同步

施工完成后必须：

```text
1. 将本文档移动到 施工文档/已完成。
2. 新增 阶段完成记录/2026-06-28-真实UI命令接入SceneEditing-C-min/00-总览.md。
3. 更新 49-当前状态与下一步入口.md。
4. 更新 54-当前可自动化施工总入口.md。
5. 更新 18-长期主义实现路径.md。
6. 更新 00-文档地图.md。
7. 更新 施工文档/README.md。
8. 更新 阶段完成记录/README.md。
9. 在 86-真实UI命令接入SceneEditing-C-min方案.md 追加实施完成补充。
```

阶段记录必须包含：

```text
实现的模块。
真实 UI 命令已接入哪些 Scene Editing 能力。
没有实现的能力。
测试命令和结果。
下一步建议。
```

## 完成判定

满足以下条件才算 86 C-min 完成：

```text
editor_ui_model::UiCommandPayload 已包含 Scene Editing UI 命令。
EditorSession::execute_command 已能处理 Scene Editing UI 命令。
OpenSceneDocument / Select / Create / Delete / SetTransform / SetComponentField / Save / Undo / Redo 可通过 UiCommand 执行。
headless/test API 不再形成第二套正式路径。
EditorInput 至少能把 Hierarchy / Toolbar / Viewport 的最小事件映射到 Scene Editing UiCommand。
EditorHost 至少能把 Scene Editing UiCommand 交给 EditorSession。
Hierarchy / Inspector / Viewport UI model 会在 Scene Editing 后刷新。
Console 能显示 Scene Editing 成功 / 失败摘要。
旧 Runtime Package / PlaySession 测试不被破坏。
最小单元测试和 workspace 测试通过或记录合理失败原因。
文档同步完成。
```

## 明确不算完成的情况

```text
只新增 UiCommandPayload 类型，没有接入 EditorSession。
UI 命令直接写 EditorSceneDocument。
Toolbar 直接调用文件写入而绕过 SceneSavePipeline。
Hierarchy / Inspector 自己维护第二套 Scene 编辑状态。
Scene edit test API 成为第二套正式执行入口。
没有 Console diagnostic。
没有 UI model 刷新测试。
没有兼容旧 Runtime Package 命令测试。
没有文档同步。
```
