# 159-当前可自动化施工文档-Codebase Hygiene Stage G scene_editing domain 拆分

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
49-当前状态与下一步入口.md
阶段完成记录/2026-07-03-Codebase-Hygiene-StageF3-EditorSession-command-service拆分/00-总览.md
```

本阶段只治理：

```text
rust/crates/editor_core/src/scene_editing.rs
```

当前基线：

```text
scene_editing.rs: 2404 lines
```

目标：

```text
把 Scene Editing 从单一大文件拆为清晰 domain 模块。
保持 public API、序列化结构、测试语义不变。
不新增编辑器功能，不改变 SceneEditCommand 行为。
```

## 2. 目标结构

建议目标结构：

```text
editor_core/src/scene_editing/
  mod.rs
  document.rs
  entity.rs
  transform.rs
  command.rs
  transaction.rs
  undo.rs
  diagnostics.rs
  preview_world_sync.rs
  collider_debug.rs
  save_pipeline.rs
  json_path.rs
  tests.rs
```

`editor_core/src/scene_editing.rs` 最终可以二选一：

```text
方案 A：保留为 thin facade，内部 mod scene_editing/*。
方案 B：移动为 scene_editing/mod.rs，lib.rs 改为 pub mod scene_editing。
```

推荐第一轮采用方案 A，降低路径迁移风险。

## 3. 分阶段施工

### G-0：施工前基线

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageG-scene_editing-domain拆分\before.json
cargo test -p editor_core scene_editing
cargo test -p editor_core
```

### G-1：拆 document / entity / transform

迁移：

```text
EditorSceneDocument
EditorSceneEntity
EditorTransform
EditorVec3
EditorMesh
EditorAssetRef
EditorSceneComponent
SceneDirtyState
```

目标文件：

```text
scene_editing/document.rs
scene_editing/entity.rs
scene_editing/transform.rs
```

验证：

```powershell
cargo test -p editor_core scene_editing::tests::editor_scene_document_loads_minimal_scene_file
cargo test -p editor_core scene_editing::tests::editor_scene_document_serializes_stably
cargo test -p editor_core scene_editing::tests::editor_scene_document_requires_transform_for_each_entity
cargo test -p editor_core scene_editing::tests::editor_scene_document_tracks_dirty_revision
```

### G-2：拆 command / transaction / undo / diagnostics

迁移：

```text
SceneEditRequest
SceneEditRequestSource
SceneEditCommand
SceneEditTransaction
SceneEditTransactionStatus
SceneEditTransactionReport
SceneUndoRecord
SceneUndoStack
SceneEditDiagnostic
SceneEditDiagnosticSeverity
```

目标文件：

```text
scene_editing/command.rs
scene_editing/transaction.rs
scene_editing/undo.rs
scene_editing/diagnostics.rs
```

验证：

```powershell
cargo test -p editor_core scene_editing::tests::scene_edit_command_serializes_for_ai_patch
cargo test -p editor_core scene_editing::tests::scene_edit_transaction_create_entity_marks_dirty
cargo test -p editor_core scene_editing::tests::scene_edit_transaction_set_transform_records_write_set
cargo test -p editor_core scene_editing::tests::scene_undo_restores_transform
cargo test -p editor_core scene_editing::tests::scene_redo_restores_created_entity
```

### G-3：拆 preview world sync / collider debug / json path

迁移：

```text
PreviewWorldSync
PreviewWorldSyncReport
ColliderDebugDrawList
ColliderDebugDrawItem
ColliderDebugShape
ColliderDebugDiagnostic
editor_component_to_collider2d
set_json_object_path
is_supported_component_field_path
```

目标文件：

```text
scene_editing/preview_world_sync.rs
scene_editing/collider_debug.rs
scene_editing/json_path.rs
```

验证：

```powershell
cargo test -p editor_core scene_editing::tests::preview_world_sync_full_rebuild_creates_world_entities
cargo test -p editor_core scene_editing::tests::preview_world_sync_rebuild_after_transform_change_updates_world
cargo test -p editor_core scene_editing::tests::collider_debug_draw_list_builds_selected_aabb_and_circle
cargo test -p editor_core scene_editing::tests::scene_edit_transaction_set_collider2d_nested_field_updates_json_field
```

### G-4：拆 save pipeline

迁移：

```text
SceneSavePipeline
SceneSaveStatus
SceneSaveReport
normalize_path
```

目标文件：

```text
scene_editing/save_pipeline.rs
```

验证：

```powershell
cargo test -p editor_core scene_editing::tests::scene_save_pipeline_writes_scene_file
cargo test -p editor_core scene_editing::tests::scene_save_pipeline_clears_dirty_on_success
cargo test -p editor_core scene_editing::tests::scene_save_pipeline_rejects_path_outside_project
cargo test -p editor_core scene_editing::tests::scene_save_pipeline_reload_matches_saved_document
```

### G-5：测试迁移与整体回归

允许把 `scene_editing.rs` 内部测试移入：

```text
scene_editing/tests.rs
```

验证：

```powershell
cargo fmt
cargo fmt --check
cargo test -p editor_core scene_editing
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p code_hygiene
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageG-scene_editing-domain拆分\after.json
```

## 4. 不做什么

```text
不改变 Scene 文件 JSON schema。
不改变 SceneEditCommand payload。
不新增 gameplay 概念。
不顺手改 PreviewWorldSync 行为。
不为了降行数制造无意义 wrapper。
```

## 5. 验收标准

```text
scene_editing.rs 不再是 2000+ 行大文件。
Scene Editing public API 对外兼容。
editor_core / editor_window_winit / editor_host / code_hygiene 回归通过。
阶段完成记录写入 before/after hygiene 数据。
施工完成后本施工文档移动到 施工文档/已完成/。
```

## 6. 施工文档自审

```text
Specification fit:
  满足“只生成施工文档，不施工”，并对应当前最大热点 scene_editing.rs。
Rule fit:
  遵守 151 hygiene gate，不新增功能、不改变行为、不引入项目侧 gameplay API。
Textual consistency:
  阶段、目标文件、验证命令一一对应。
Design fit:
  Scene Editing 是编辑器 authoring 基础能力，拆分后更适合 AI 定位和长期维护。
Implementation feasibility:
  现有 scene_editing.rs 内部边界清晰，适合按 document/transaction/save/sync/debug 分段迁移。
Practical reasonableness:
  每段都有小测试，失败定位明确，不做一次性大爆破。
```
