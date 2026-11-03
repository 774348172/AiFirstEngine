# 90-当前可自动化施工文档：ProjectAsset-to-SceneEntity Authoring C-min

## 目标

按照 `90-ProjectAsset-to-SceneEntity-Authoring-C-min方案.md`，把 ProjectDock / Project Library 中的资源放入 Scene，形成可保存、可撤销、可预览、可运行的最小编辑闭环。

目标链路：

```text
Project asset
  -> UiCommandPayload::PlaceAssetIntoScene
  -> EditorSession
  -> AssetPlacementResolver
  -> SceneEditCommand
  -> SceneEditTransaction
  -> EditorSceneDocument
  -> PreviewWorldSync
  -> SaveScene
  -> Play
  -> AssetPlacementReport / Console / Trace
```

本轮不是做完整拖拽系统，而是先把“资源生成场景 Entity”的核心规则落成可测试的 Editor Core 能力。

## 已有基础

必须复用：

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/editable_project_loop.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_host/src/main.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
```

已有规则：

```text
EditorSceneDocument 是编辑真相。
Runtime World 不是编辑真相。
UI / AI / Test 不直接写 Scene 文件。
UI 行为必须先转 UiCommandPayload。
EditorSession 是 Editor Core 的统一入口。
Scene 修改必须走 SceneEditTransaction。
Save 必须走 SceneSavePipeline。
Play / Run 必须走 PlaySessionController。
资源不直接进入 Scene。
Scene 只保存 Entity / Component / AssetRef / Prefab Instance / Overrides。
```

关键参考文档：

```text
15-Scene-Entity-Component-Prefab数据模型.md
85-Scene-Editing-v1-C-min方案.md
86-真实UI命令接入SceneEditing-C-min方案.md
89-真实可编辑小项目闭环-C-min方案.md
90-ProjectAsset-to-SceneEntity-Authoring-C-min方案.md
```

## 明确不做

本轮不实现：

```text
真实鼠标拖拽。
Viewport picking / surface placement。
Gizmo。
snapping。
多资源批量 drop。
材质自动匹配。
模型子资源选择。
Prefab Mode。
Nested Prefab override。
资源导入器重跑。
真实文件系统扫描。
完整 ProjectDock 文件浏览。
完整 Inspector schema renderer。
真实窗口 smoke 作为默认测试。
```

## A1：扩展 UiCommandPayload

修改：

```text
rust/crates/editor_ui_model/src/lib.rs
```

新增：

```text
UiCommandPayload::PlaceAssetIntoScene {
  asset_id: String,
  asset_type: String,
  asset_guid: Option<String>,
  target_parent_id: Option<String>,
  local_position: Option<Vec3>,
  placement_mode: AssetPlacementMode,
}
```

新增：

```text
AssetPlacementMode
```

C-min 只支持：

```text
WorldOrigin
UnderSelectedOrRoot
```

不支持：

```text
SurfaceHit
SceneCameraCenter
GridSnap
```

测试：

```text
cargo test -p editor_ui_model place_asset_into_scene
```

最低断言：

```text
PlaceAssetIntoScene 可序列化为 AI 可读 JSON。
placement_mode 使用稳定 camelCase 或明确枚举字符串。
asset_id / asset_type / target_parent_id / local_position 字段稳定。
```

## A2：新增 AssetPlacementResolver

优先新增：

```text
rust/crates/editor_core/src/asset_placement.rs
```

新增结构：

```text
ASSET_PLACEMENT_REPORT_SCHEMA_VERSION
AssetPlacementMode
AssetPlacementRequest
AssetPlacementPlan
AssetPlacementReport
AssetPlacementDiagnostic
AssetPlacementDiagnosticSeverity
AssetPlacementResolver
```

输入：

```text
asset_id
asset_type
asset_guid
target_parent_id
local_position
placement_mode
```

输出：

```text
scene_commands: Vec<SceneEditCommand>
selected_entity_id: Option<String>
created_entity_ids: Vec<String>
created_component_types: Vec<String>
diagnostics
```

规则：

```text
Resolver 不写 Scene。
Resolver 不读真实文件系统。
Resolver 不直接访问 Runtime World。
Resolver 只把资源放置意图翻译成 SceneEditCommand。
```

C-min 支持资源类型：

```text
mesh
model
texture
sprite
prefab
```

映射规则：

```text
mesh / model:
  CreateEntity
  entity.mesh.asset_ref = EditorAssetRef { id: asset_id, type: asset_type }

texture / sprite:
  CreateEntity
  entity.mesh.asset_ref = EditorAssetRef { id: asset_id, type: asset_type }

prefab:
  CreateEntity
  entity.components += PrefabInstance { source: { id, type, guid } }
```

说明：

```text
texture / sprite 第一版也走 EditorMesh.asset_ref，是为了复用当前 Renderable / PreviewWorldSync / RuntimeMesh 链路。
长期可替换为 SpriteRenderer typed component，但本轮不引入。
```

测试：

```text
cargo test -p editor_core asset_placement
```

最低断言：

```text
mesh 生成 CreateEntity + EditorMesh.asset_ref。
sprite / texture 生成 CreateEntity + EditorMesh.asset_ref。
prefab 生成 CreateEntity + PrefabInstance 动态组件。
unsupported asset_type 返回 diagnostic 且不生成 scene_commands。
entity id 由当前 EditorSceneDocument 的 next_entity_id 规则生成，避免冲突。
```

## A3：SceneEditCommand 支持携带 mesh

当前 `SceneEditCommand::CreateEntity` 已支持：

```text
components: Vec<EditorSceneComponent>
local_transform: EditorTransform
```

但如当前结构不能直接带 `EditorMesh`，需要二选一：

### 优先方案

给 `CreateEntity` 增加可选字段：

```text
mesh: Option<EditorMesh>
```

原因：

```text
mesh 是当前 EditorSceneEntity 的正式字段。
PreviewWorldSync 和 Runtime 现有链路已经识别 mesh。
比把 mesh 塞进 dynamic component 更少弯路。
```

兼容规则：

```text
#[serde(default)]
mesh: Option<EditorMesh>
```

已有测试如果只构造旧 CreateEntity，需要补 `mesh: None`。

### 备选方案

先通过 `components` 写 `RenderableBinding` 动态组件。

不推荐，因为当前 PreviewWorldSync 不会自动把这个动态组件变成 Renderable。

测试：

```text
cargo test -p editor_core scene_edit_transaction_create_entity
cargo test -p editor_core preview_world_sync
```

最低断言：

```text
CreateEntity 可以创建带 mesh 的 EditorSceneEntity。
PreviewWorldSync 能把带 mesh 的 Entity 转为 Renderable。
旧的不带 mesh 的 CreateEntity 行为不变。
```

## A4：EditorSession 接入 PlaceAssetIntoScene

修改：

```text
rust/crates/editor_core/src/lib.rs
```

新增命令处理：

```text
UiCommandPayload::PlaceAssetIntoScene
  -> require open EditorSceneDocument
  -> AssetPlacementResolver::resolve(...)
  -> execute generated SceneEditCommand(s)
  -> select created entity if any
  -> refresh ui model
  -> push Console summary / diagnostics
```

规则：

```text
EditorSession 是唯一入口。
PlaceAssetIntoScene 不绕过 SceneEditTransaction。
如果 Resolver 返回多个 SceneEditCommand，本轮允许逐条执行，但 Report 必须保留同一个 placement request id。
如果任一命令失败，返回 structured diagnostic。
```

测试：

```text
cargo test -p editor_core editor_session_place_asset_into_scene
```

最低断言：

```text
未打开 Scene 时返回 diagnostic。
mesh asset 放入 Scene 后 dirty == true。
mesh asset 放入 Scene 后 Hierarchy 出现新 Entity。
mesh asset 放入 Scene 后 Inspector 选中新 Entity 并显示 AssetRef。
unsupported asset_type 不修改 Scene，并进入 Console error / warning。
```

## A5：UI / Input / Host 最小路由测试

如当前 `editor_input` 或 `editor_host` 已有命令路由模式，补最小 headless 测试。

修改候选：

```text
rust/crates/editor_input/src/lib.rs
rust/crates/editor_host/src/main.rs
```

目标：

```text
ProjectDock asset action / test command
  -> UiCommandPayload::PlaceAssetIntoScene
  -> EditorSession
```

本轮不做真实 drag/drop，只做可测试命令入口：

```text
Add selected asset to scene
```

测试：

```text
cargo test -p editor_input place_asset_into_scene
cargo test -p editor_host place_asset_into_scene
```

如果某个 crate 暂无对应 UI model，不强行扩张；以 editor_core 的 headless 测试为准。

## A6：保存与 Play 闭环

扩展或新增 headless 闭环测试：

```text
open default scene
  -> PlaceAssetIntoScene(mesh)
  -> SaveScene
  -> Undo
  -> Redo
  -> Play
  -> report
```

可新增：

```text
run_asset_authoring_loop_headless()
AssetAuthoringLoopReport
```

或扩展 89 的 `EditableProjectLoopReport`，但推荐新增小报告，避免 89 报告无限膨胀。

报告字段：

```text
schema_version
asset_id
asset_type
opened_scene
created_entity_id
created_component_types
dirty_before_save
dirty_after_save
undo_applied
redo_applied
play_finished
console_entry_count
runtime_trace_entry_count
diagnostics
```

测试：

```text
cargo test -p editor_core asset_authoring_loop
cargo test -p editor_host asset_authoring_loop
```

最低断言：

```text
report 可序列化。
report 包含 asset_authoring_loop schema version。
place -> save -> undo -> redo -> play 全链路通过。
```

## A7：文档和索引收尾

施工完成后必须：

```text
1. cargo fmt
2. cargo fmt --check
3. cargo test -p editor_ui_model place_asset_into_scene
4. cargo test -p editor_core asset_placement
5. cargo test -p editor_core editor_session_place_asset_into_scene
6. cargo test -p editor_core asset_authoring_loop
7. cargo test -p editor_host place_asset_into_scene 或 asset_authoring_loop
8. cargo test -p editor_core
9. cargo test -p editor_host
10. cargo test --workspace
```

如某个 filter 没有匹配测试，必须在阶段记录中说明：

```text
该 filter 当前无匹配测试，不视为失败。
```

完成后归档：

```text
施工文档/当前/90-当前可自动化施工文档-ProjectAsset-to-SceneEntity-Authoring-C-min.md
  -> 施工文档/已完成/90-当前可自动化施工文档-ProjectAsset-to-SceneEntity-Authoring-C-min.md
```

新增阶段记录：

```text
阶段完成记录/2026-06-28-ProjectAsset-to-SceneEntity-Authoring-C-min/00-总览.md
```

更新：

```text
90-ProjectAsset-to-SceneEntity-Authoring-C-min方案.md
87-当前实现状态总览-2026-06-28.md
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
18-长期主义实现路径.md
00-文档地图.md
README.md
施工文档/README.md
阶段完成记录/README.md
```

## 最小验收标准

本轮完成后，必须能证明：

```text
1. AI / UI / Test 可以用同一个 PlaceAssetIntoScene 命令把资源放入 Scene。
2. 资源入场不会绕过 EditorSceneDocument / SceneEditTransaction。
3. mesh / sprite / texture / prefab 至少能生成稳定 Entity 数据。
4. unsupported asset_type 不会污染 Scene。
5. 新建 Entity 可以 dirty / save / undo / redo。
6. 保存后的 Scene 可以进入 Play 闭环。
7. Report 能帮助 AI 解释资源是否成功生成 Entity。
```

## 当前结论

90 施工是 89 之后最小可编辑游戏项目闭环的下一步。完成后，编辑器将从“能改已有 Scene”推进到“能从资源搭 Scene”，这才接近 Unity / UE / Godot 的基础编辑体验。
