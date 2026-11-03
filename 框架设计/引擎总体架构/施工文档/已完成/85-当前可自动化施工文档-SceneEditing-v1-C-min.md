# 85-当前可自动化施工文档：Scene Editing v1 C-min

## 目标

按照 `85-Scene-Editing-v1-C-min方案.md` 实现第一版 Scene Editing v1 C-min。

目标不是做完整 Unity / UE 场景编辑器，也不是做复杂 Gizmo，而是把编辑器场景编辑能力收敛成一套长期可维护的最小闭环：

```text
SceneView / Hierarchy / Inspector / AI / Test
  -> SceneEditRequest
  -> SceneEditCommand
  -> SceneEditTransaction
  -> EditorSceneDocument
  -> SceneDirtyState
  -> PreviewWorldSync full rebuild
  -> SaveScene
```

## 当前基础

必须复用：

```text
70-Scene-Prefab-Entity-Runtime实例化方案.md
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
85-Scene-Editing-v1-C-min方案.md

editor_core:
  EditorSession
  CommandTransaction / CommandResult
  HierarchyModel / InspectorModel / ViewportModel / ConsoleModel

engine_runtime:
  scene_loader::load_scene_into_world
  runtime_package::RuntimePackage
  world::World
  components / ids / math

editor_ui_model:
  UiCommand / UiCommandPayload
  HierarchyModel / InspectorModel / ToolbarModel / ConsoleModel
```

## 第一版边界

本轮必须实现：

```text
EditorSceneDocument
EditorSceneEntity
SceneSelection
SceneEditRequest
SceneEditCommand
SceneEditTransaction
SceneEditTransactionReport
SceneDirtyState
SceneSavePipeline
PreviewWorldSync full rebuild
EditorSession 基础接入
headless 单元测试
```

本轮不实现：

```text
真实 Viewport Gizmo。
复杂鼠标拾取。
复杂多选批量编辑。
Prefab Mode。
嵌套 Prefab override。
Terrain / Light / NavMesh 工具。
复杂吸附 / 对齐。
Asset Browser drag into Scene。
多人协作锁。
复杂 Scene diff / merge。
完整 Unity/UE EditorMode 插件系统。
```

## 施工原则

```text
EditorSceneDocument 是编辑真相。
Runtime World 不是编辑真相。
PreviewWorld 是预览结果，可以全量重建。
所有 Scene 源数据修改必须走 SceneEditCommand。
所有 SceneEditCommand 必须进入 SceneEditTransaction。
SceneView / Hierarchy / Inspector / AI 不直接修改 EditorSceneDocument。
AI 只生成 SceneEditCommand，不直接写 Scene 文件或 Runtime ECS。
SaveScene 必须走 SceneSavePipeline。
Save 成功 dirty=false，失败 dirty 保持 true。
```

## 建议代码位置

优先新增：

```text
rust/crates/editor_core/src/scene_editing.rs
```

需要调整：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_input/src/lib.rs
```

第一版尽量少碰：

```text
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_ui_backend_egui/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
```

除非为了暴露最小命令入口，否则不要先做真实 UI 控件。

## A1：新增 EditorSceneDocument 数据结构

新增模块：

```text
editor_core::scene_editing
```

新增常量：

```text
SCENE_EDIT_TRANSACTION_REPORT_SCHEMA_VERSION = "scene-edit-transaction-report.v1"
PREVIEW_WORLD_SYNC_REPORT_SCHEMA_VERSION = "preview-world-sync-report.v1"
EDITOR_SCENE_DOCUMENT_SCHEMA_VERSION = "editor-scene-document.v1"
```

新增类型：

```text
EditorSceneDocument
  scene_id
  scene_path
  schema_version
  entities
  selected_entity_ids
  revision
  dirty_state

EditorSceneEntity
  entity_id
  name
  enabled
  parent_id optional
  sibling_order
  transform
  components

EditorTransform
  local_position
  local_rotation
  local_scale

EditorSceneComponent
  component_type
  fields serde_json::Value

SceneDirtyState
  dirty
  revision
  last_transaction_id optional
```

第一版规则：

```text
每个 Entity 必须有 Transform。
Entity ID 使用稳定 AuthoringEntityId 字符串。
RuntimeEntityId 不写入 EditorSceneDocument。
组件字段第一版用 serde_json::Value 承载。
```

测试：

```text
editor_scene_document_loads_minimal_scene_file
editor_scene_document_serializes_stably
editor_scene_document_requires_transform_for_each_entity
editor_scene_document_tracks_dirty_revision
```

运行：

```powershell
cargo test -p editor_core editor_scene_document
```

## A2：实现 SceneSelection

新增：

```text
SceneSelection
  selected_entity_ids
  primary_entity_id optional
```

第一版规则：

```text
只实现单选行为。
数据结构保留 Vec，给后续多选扩展。
Selection 属于 Editor，不属于 Runtime。
Selection 第一版可以不进 Undo，但必须可 trace。
```

测试：

```text
scene_selection_selects_single_entity
scene_selection_clears_missing_entity
scene_selection_does_not_mark_scene_dirty
```

运行：

```powershell
cargo test -p editor_core scene_selection
```

## A3：实现 SceneEditCommand / SceneEditRequest

新增：

```text
SceneEditRequest
  request_id
  source
  target_scene_id
  payload

SceneEditRequestSource
  SceneView
  Hierarchy
  Inspector
  Toolbar
  Ai
  Test

SceneEditCommand
  SelectEntity
  CreateEntity
  DeleteEntity
  DuplicateEntity
  ReparentEntity
  SetTransform
  SetComponentField
  SaveScene
  Undo
  Redo
```

命令字段按 85 方案：

```text
CreateEntity:
  parent_id optional
  name
  components
  local_transform
  sibling_order optional

DeleteEntity:
  entity_id
  delete_children true

ReparentEntity:
  entity_id
  new_parent_id optional
  sibling_order optional
  keep_world_transform bool

SetTransform:
  entity_id
  local_position optional
  local_rotation optional
  local_scale optional

SetComponentField:
  entity_id
  component_type
  field_path
  value

SaveScene:
  scene_id
  path optional
```

测试：

```text
scene_edit_command_serializes_for_ai_patch
scene_edit_request_records_source
scene_edit_command_set_transform_only_uses_local_transform
scene_edit_command_save_scene_keeps_path_optional
```

运行：

```powershell
cargo test -p editor_core scene_edit_command
```

## A4：实现 SceneEditTransaction

新增：

```text
SceneEditTransaction
  transaction_id
  request_id
  command
  status
  read_set
  write_set
  before_summary
  after_summary
  diagnostics
  undo_record optional

SceneEditTransactionStatus
  Pending
  Committed
  Rejected
  Failed

SceneEditDiagnostic
  severity
  code
  layer
  message
  path optional
  entity_id optional
```

第一版执行规则：

```text
SelectEntity:
  更新 SceneSelection。
  不标记 dirty。

CreateEntity:
  验证 parent 是否存在。
  新增 Entity。
  dirty=true。
  生成 undo_record。

DeleteEntity:
  验证 entity 存在。
  默认删除子树。
  如果外部 EntityRef 指向它，第一版默认 rejected。
  dirty=true。
  生成 undo_record。

ReparentEntity:
  验证 entity 存在。
  验证 new_parent 存在或为空。
  禁止形成循环。
  dirty=true。
  生成 undo_record。

SetTransform:
  验证 entity 存在。
  只修改 local transform。
  dirty=true。
  生成 undo_record。

SetComponentField:
  验证 entity / component / field_path。
  第一版 field_path 支持单层字段名。
  dirty=true。
  生成 undo_record。
```

测试：

```text
scene_edit_transaction_create_entity_marks_dirty
scene_edit_transaction_select_entity_does_not_mark_dirty
scene_edit_transaction_set_transform_records_write_set
scene_edit_transaction_rejects_missing_entity
scene_edit_transaction_rejects_cyclic_reparent
scene_edit_transaction_delete_entity_removes_child_subtree
scene_edit_transaction_set_component_field_updates_json_field
```

运行：

```powershell
cargo test -p editor_core scene_edit_transaction
```

## A5：实现 Undo / Redo 最小能力

新增：

```text
SceneUndoRecord
  transaction_id
  command_kind
  before_document_snapshot
  after_document_snapshot

SceneUndoStack
  undo_stack
  redo_stack
```

第一版规则：

```text
Undo / Redo 用文档快照实现，简单可靠。
只记录修改 EditorSceneDocument 的命令。
SelectEntity 第一版不进入 Undo。
SaveScene 不进入 Undo。
Undo 后 dirty=true。
Redo 后 dirty=true。
```

测试：

```text
scene_undo_restores_created_entity
scene_redo_restores_created_entity
scene_undo_restores_transform
scene_undo_does_not_record_selection_only
```

运行：

```powershell
cargo test -p editor_core scene_undo
```

## A6：实现 PreviewWorldSync full rebuild

新增：

```text
PreviewWorldSyncReport
  schema_version
  scene_id
  sync_mode = full_rebuild
  entity_count
  component_count
  diagnostics

PreviewWorldSync::full_rebuild(document) -> World + PreviewWorldSyncReport
```

第一版规则：

```text
每次修改后允许全量重建 PreviewWorld。
PreviewWorldSync 不修改 dirty。
PreviewWorldSync 不写 Scene 文件。
PreviewWorldSync 失败时 Transaction 可以 Failed 或 CommittedWithDiagnostic；第一版推荐 Failed，避免 UI 预览和文档不一致。
```

测试：

```text
preview_world_sync_full_rebuild_creates_world_entities
preview_world_sync_reports_entity_and_component_count
preview_world_sync_does_not_clear_dirty
preview_world_sync_rebuild_after_transform_change_updates_world
```

运行：

```powershell
cargo test -p editor_core preview_world_sync
```

## A7：实现 SceneSavePipeline

新增：

```text
SceneSavePipeline
SceneSaveReport
  scene_id
  path
  status
  diagnostics
  dirty_after
```

第一版规则：

```text
保存同步执行。
保存路径必须在项目目录内。
保存不能写 Runtime Package 产物目录。
保存前验证 EditorSceneDocument。
保存成功 dirty=false。
保存失败 dirty=true。
```

测试：

```text
scene_save_pipeline_writes_scene_file
scene_save_pipeline_rejects_path_outside_project
scene_save_pipeline_rejects_runtime_package_output_path
scene_save_pipeline_keeps_dirty_on_failure
scene_save_pipeline_clears_dirty_on_success
scene_save_pipeline_reload_matches_saved_document
```

运行：

```powershell
cargo test -p editor_core scene_save_pipeline
```

## A8：EditorSession 基础接入

改造：

```text
EditorSession 新增 editor_scene_document optional。
EditorSession 新增 preview_world optional 或复用现有 world 作为 preview world。
OpenRuntimePackage 不等于 OpenEditableScene。
第一版可以通过测试命令打开 SceneDocument。
```

需要新增或扩展命令：

```text
UiCommandPayload::OpenSceneDocument { path }
UiCommandPayload::SceneEdit { request_json 或 typed payload }
UiCommandPayload::SaveScene
UiCommandPayload::UndoSceneEdit
UiCommandPayload::RedoSceneEdit
```

如果不想立刻污染 `editor_ui_model`，允许先在 `editor_core` 暴露测试 API：

```text
EditorSession::open_scene_document_for_test(path)
EditorSession::execute_scene_edit_for_test(command)
```

但完成判定要求：

```text
Hierarchy / Inspector / Viewport 至少能从 EditorSceneDocument / PreviewWorld 构建。
Console 能看到 SceneEditTransaction 成功 / 失败摘要。
```

测试：

```text
editor_session_open_scene_document_builds_hierarchy
editor_session_scene_edit_create_entity_refreshes_hierarchy
editor_session_scene_edit_set_transform_refreshes_viewport
editor_session_scene_edit_failure_creates_console_error
editor_session_save_scene_clears_dirty
```

运行：

```powershell
cargo test -p editor_core editor_session_scene_edit
```

## A9：UI Model 最小接入

第一版最小要求：

```text
HierarchyModel 从 EditorSceneDocument 或 PreviewWorld 构建。
InspectorModel 对选中 Entity 显示 Transform 和组件字段。
ViewportModel 从 PreviewWorld 构建 renderable summary。
ConsoleModel 显示 SceneEditTransaction diagnostics。
```

暂缓：

```text
真实 Inspector 输入控件。
真实 SceneView Gizmo。
真实 Hierarchy 拖拽重排。
真实 Asset drag into Scene。
```

测试：

```text
scene_edit_ui_model_shows_created_entity
scene_edit_ui_model_shows_selected_entity_transform
scene_edit_ui_model_shows_component_json_field
```

运行：

```powershell
cargo test -p editor_core scene_edit_ui_model
```

## A10：Headless smoke 与 workspace 测试

必须运行：

```powershell
cargo test -p editor_core editor_scene_document
cargo test -p editor_core scene_selection
cargo test -p editor_core scene_edit_command
cargo test -p editor_core scene_edit_transaction
cargo test -p editor_core scene_undo
cargo test -p editor_core preview_world_sync
cargo test -p editor_core scene_save_pipeline
cargo test -p editor_core editor_session_scene_edit
cargo test -p editor_core scene_edit_ui_model
cargo test --workspace
```

如果 `cargo test --workspace` 因环境或无关历史问题失败，必须记录失败原因和已通过的更小测试命令。

## A11：完成后文档同步

施工完成后必须：

```text
1. 将本文档移动到 施工文档/已完成/。
2. 新增 阶段完成记录/2026-06-28-SceneEditing-v1-C-min/00-总览.md。
3. 更新 49-当前状态与下一步入口.md。
4. 更新 54-当前可自动化施工总入口.md。
5. 更新 18-长期主义实现路径.md。
6. 更新 00-文档地图.md。
7. 更新 施工文档/README.md。
8. 更新 阶段完成记录/README.md。
9. 在 85-Scene-Editing-v1-C-min方案.md 追加实施完成补充。
```

阶段记录必须包含：

```text
实现的模块。
当前能编辑的 Scene 能力。
没有实现的能力。
测试命令和结果。
下一步建议。
```

## 完成判定

满足以下条件才算 85 C-min 完成：

```text
EditorSceneDocument 已存在并可从 scene file 加载。
SceneEditCommand / SceneEditTransaction 已存在并可序列化。
CreateEntity / DeleteEntity / ReparentEntity / SetTransform / SetComponentField 可执行。
Undo / Redo 最小快照能力可用。
SceneDirtyState 正确标记 dirty / clean。
PreviewWorldSync full rebuild 可生成 World。
SceneSavePipeline 可保存并重新加载一致。
EditorSession 能打开可编辑 Scene，并在编辑后刷新 Hierarchy / Inspector / Viewport。
Console 能看到 SceneEditTransaction 成功 / 失败摘要。
最小单元测试通过。
```

## 明确不算完成的情况

```text
只写类型，没有 Transaction 执行。
直接修改 Runtime World，当作 Scene 编辑真相。
AI / UI 可以绕过 SceneEditTransaction 写 Scene 文件。
没有 DirtyState。
没有 Undo / Redo。
没有 PreviewWorldSync。
没有 SaveScene。
没有测试。
没有文档同步。
```
