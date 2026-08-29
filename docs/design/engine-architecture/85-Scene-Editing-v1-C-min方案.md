# 85-Scene Editing v1 C-min 方案

## 定位

Scene Editing v1 是编辑器的场景编辑系统。

它解决的问题不是 Runtime 怎么运行游戏，而是：

```text
用户 / AI 如何在编辑器里修改 Scene 源数据。
编辑器如何选择 Entity。
编辑器如何移动 / 旋转 / 缩放 Entity。
编辑器如何创建 / 删除 / 复制 / 改父子关系。
Inspector 如何修改组件字段。
修改如何进入 Undo / Redo。
修改如何标记 Scene dirty。
修改如何同步到 PreviewWorld。
Scene 如何保存回项目文件。
```

本方案采用 C-min：

```text
长期架构按 Unity / UE 级别的编辑器场景编辑分层设计。
第一版只实现最小可用能力。
```

它不是临时 SceneDocument，也不是完整 UE EditorMode。

## 已有边界

本方案不重新讨论以下系统：

```text
70-Scene-Prefab-Entity-Runtime实例化方案.md
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
84-Editor-Play-Run-Session-System方案.md
```

已有规则：

```text
70 负责 Scene / Prefab / Entity 如何实例化到 Runtime ECS。
74 负责 SceneView / GameView 的输入归属。
84 负责 Editor Play / Run Session。
```

本方案新增的是：

```text
Authoring Scene 编辑真相。
SceneEditCommand。
SceneEditTransaction。
Undo / Redo。
Dirty / Save。
PreviewWorldSync。
```

## 参考 Unity

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SceneView\SceneView.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Tools\BuiltinTools.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Tools\EditorToolManager.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\EditorSceneManager.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SceneHierarchy.cs
```

Unity 的核心做法：

```text
SceneView 负责世界视图、选择、Handle / Gizmo。
Selection 是编辑器全局选择状态。
Tools / TransformTool / Handles 负责移动、旋转、缩放。
Inspector 通过 SerializedObject 修改对象字段。
Undo 记录编辑动作。
EditorSceneManager 负责 New / Open / Save / MarkDirty。
```

Unity 值得学习：

```text
用户心智简单。
SceneView / Hierarchy / Inspector 的协作清晰。
选中对象后即可用 Gizmo 和 Inspector 修改。
Scene dirty / Save 是正式流程。
```

Unity 不直接照搬：

```text
不照搬 IMGUI 历史结构。
不照搬 UnityObject / SerializedObject 黑箱。
不让项目逻辑直接依赖编辑器对象。
```

## 参考 UE

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\LevelEditor\Public\SLevelViewport.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\LevelEditorViewport.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\EditorActor.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\EditorServer.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\AssetSelection.cpp
```

UE 的核心做法：

```text
SLevelViewport / FLevelEditorViewportClient 处理编辑器视口。
EditorMode / Widget 负责编辑工具。
GEditor->AddActor / SelectActor / edactDeleteSelected 修改关卡对象。
FScopedTransaction + Modify() 负责 Undo / Redo。
MarkPackageDirty 标记关卡或资源需要保存。
AssetSelection / ActorFactory 负责资源拖入场景生成 Actor。
```

UE 值得学习：

```text
所有编辑动作都有 Transaction。
场景修改、选择修改、资源放置、删除都有明确编辑器入口。
大型项目里工具能力强，Undo / Dirty / Save 边界稳定。
```

UE 不直接照搬：

```text
不第一版实现完整 EditorMode 插件系统。
不照搬 UObject / Actor / Component 反射体系。
不第一版实现完整 ActorFactory / PlacementMode。
不把规则复杂度提前推给用户或 AI。
```

## 方案选择

### 方案 A：直接修改 Runtime World

```text
SceneView / Inspector
  -> 直接写 Runtime ECS World
```

优点：

```text
最快。
代码少。
```

缺点：

```text
编辑数据和运行数据混在一起。
保存困难。
Undo / Redo 困难。
AI 修改容易越权。
后期复杂项目会难以维护。
```

结论：

```text
不采用。
```

### 方案 B：简单 SceneDocument + Command + Transaction

```text
SceneEditCommand
  -> SceneDocument
  -> PreviewWorld
```

优点：

```text
比 A 清晰。
第一版容易实现。
```

缺点：

```text
如果没有 Tool / Selection / Dirty / Save / PreviewSync 的长期边界，后续仍会迁移。
容易变成临时编辑器模型。
```

结论：

```text
不作为最终方案。
```

### 方案 C：完整 Unity / UE 式编辑器场景系统

```text
完整 SceneView Tool Framework
完整 Selection System
完整 Gizmo / Handle
完整 Transaction
完整 Scene Save Pipeline
完整 Prefab Mode
完整 Placement / DragDrop / MultiEdit
```

优点：

```text
长期最强。
```

缺点：

```text
第一版范围过大。
会拖慢最小游戏编辑闭环。
```

结论：

```text
不做完整 C。
```

### 方案 C-min：长期架构骨架 + 第一版最小能力

```text
SceneView / Hierarchy / Inspector / AI
  -> SceneEditRequest
  -> ActiveSceneTool
  -> SceneEditCommand
  -> SceneEditTransaction
  -> EditorSceneDocument
  -> DirtyState
  -> PreviewWorldSync
  -> SaveScene
```

优点：

```text
从第一版就保持长期架构方向。
不会把编辑器改成临时脚手架。
AI 只需要生成结构化 SceneEditCommand。
用户心智接近 Unity。
事务 / Dirty / Save 边界接近 UE。
第一版功能范围可控。
```

缺点：

```text
比方案 B 多几个正式边界。
需要先实现最小 Transaction 和 PreviewWorldSync。
```

结论：

```text
采用方案 C-min。
```

## 正式架构

### 总链路

```text
SceneView / Hierarchy / Inspector / AI
  -> SceneEditRequest
  -> ActiveSceneTool
  -> SceneEditCommand
  -> SceneEditTransaction
  -> EditorSceneDocument
  -> SceneDirtyState
  -> PreviewWorldSync
  -> Editor UI Model refresh
  -> SaveScene
```

### 数据真相

```text
EditorSceneDocument 是编辑器场景编辑真相。
Runtime World 不是编辑真相。
PreviewWorld 是 EditorSceneDocument 的预览结果。
Runtime Package 是运行输入，不是编辑输入。
Scene 文件是最终持久化数据。
```

### 与 Runtime 的关系

```text
EditorSceneDocument
  -> PreviewWorldSync
  -> PreviewWorld
  -> SceneView / Viewport preview
```

运行时：

```text
Scene 文件 / Runtime Package
  -> RuntimeInstanceLoader
  -> Runtime ECS World
```

规则：

```text
编辑器不能直接把 Runtime ECS World 当作 Scene 源数据修改。
Scene Editing 修改 EditorSceneDocument。
PreviewWorld 可以被重建或增量同步。
PlaySession 使用 Runtime Package / Runtime World，不直接使用未保存的临时 PreviewWorld。
```

## 核心模块

### EditorSceneDocument

职责：

```text
持有当前打开 Scene 的源数据。
保存 Entity 树。
保存 Entity 的组件数据。
保存 AssetRef / PrefabRef / EntityRef。
保存 dirty 状态相关版本号。
提供读接口给 Hierarchy / Inspector / SceneView。
```

不负责：

```text
不执行项目逻辑。
不执行 Runtime tick。
不直接发 RenderCommand。
不处理窗口输入。
```

第一版字段：

```text
scene_id
scene_path
schema_version
entities[]
selected_entity_ids[]
revision
dirty
```

### EditorSceneEntity

第一版字段：

```text
entity_id
name
enabled
parent_id optional
sibling_order
transform
components[]
```

规则：

```text
每个 Entity 必须有 Transform。
Transform 是组件，但作为编辑器必备组件显示。
Entity ID 使用稳定 AuthoringEntityId。
RuntimeEntityId 不写入 Scene 文件。
```

### SceneSelection

职责：

```text
记录当前选中的 AuthoringEntityId 列表。
同步 Hierarchy / Inspector / SceneView 高亮。
```

规则：

```text
选择状态属于 Editor，不属于 Runtime。
选择修改可以进入 Transaction，也可以作为轻量编辑状态记录。
第一版支持单选，保留多选结构。
```

### ActiveSceneTool

第一版工具：

```text
SelectTool
MoveTool
RotateTool
ScaleTool
CreateEntityTool
```

规则：

```text
SceneView 输入先经过 74 号 ViewportInputGateway。
SceneView 输入不会直接进入 Runtime。
SceneView 输入生成 EditorToolCommand。
EditorToolCommand 再生成 SceneEditRequest / SceneEditCommand。
```

第一版不做完整 Gizmo：

```text
Move / Rotate / Scale 可以先由 Inspector numeric input 或 headless command 测试。
Viewport Gizmo 可在后续系统实现。
```

### SceneEditRequest

来源：

```text
SceneView
Hierarchy
Inspector
Toolbar
AI
Test
```

第一版字段：

```text
request_id
source
target_scene_id
payload
```

### SceneEditCommand

第一版命令：

```text
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

规则：

```text
所有会修改 Scene 源数据的操作都必须变成 SceneEditCommand。
AI 只能生成 SceneEditCommand，不直接写 Scene 文件。
UI 只能发 SceneEditRequest，不直接改 EditorSceneDocument。
```

### SceneEditTransaction

职责：

```text
验证命令。
记录 before / after summary。
应用 EditorSceneDocument 修改。
维护 Undo / Redo。
生成 diagnostics。
标记 dirty。
触发 PreviewWorldSync。
```

第一版字段：

```text
transaction_id
request_id
command
status
read_set
write_set
before_summary
after_summary
diagnostics[]
undo_record optional
```

规则：

```text
Create / Delete / Duplicate / Reparent / SetTransform / SetComponentField 必须产生 Transaction。
SaveScene 可以产生 Transaction，但保存本身不进入 Undo。
Selection 第一版可以不进入 Undo，但必须可 trace。
```

### SceneDirtyState

职责：

```text
记录 Scene 是否有未保存修改。
记录最后一次修改 transaction_id。
记录 revision。
```

规则：

```text
任何修改 EditorSceneDocument 的 Transaction 都必须 dirty=true。
SaveScene 成功后 dirty=false。
PreviewWorldSync 不改变 dirty。
Runtime tick 不改变 dirty。
```

### SceneSavePipeline

职责：

```text
把 EditorSceneDocument 写回 Scene 文件。
写入前做结构验证。
写入后更新 dirty=false。
输出 SaveSceneReport。
```

规则：

```text
第一版保存同步执行。
保存失败必须保留 dirty=true。
保存路径必须在项目目录内。
保存不能写 Runtime Package 产物目录。
```

### PreviewWorldSync

职责：

```text
把 EditorSceneDocument 同步为 PreviewWorld。
供 SceneView / Hierarchy / Inspector / Viewport preview 使用。
```

第一版策略：

```text
小场景可以全量重建 PreviewWorld。
命令执行后生成 PreviewWorldSyncReport。
后续再做增量同步。
```

规则：

```text
PreviewWorldSync 是编辑器预览能力，不是 Runtime 正式运行能力。
PreviewWorld 可以丢弃重建。
EditorSceneDocument 才是编辑真相。
```

## 与 UI 面板关系

### SceneView

职责：

```text
显示 PreviewWorld。
显示选中高亮。
显示编辑工具可视化。
把输入转成 EditorToolCommand。
```

不负责：

```text
不直接修改 EditorSceneDocument。
不保存 Scene。
不直接修改 Runtime World。
```

### Hierarchy

职责：

```text
显示 EditorSceneDocument 的 Entity 树。
发出 Select / Reparent / Delete / Duplicate 请求。
```

### Inspector

职责：

```text
显示选中 Entity 的组件字段。
字段修改生成 SetComponentField 或 SetTransform。
```

### AI

职责：

```text
把自然语言转换成 SceneEditCommand。
解释 Transaction / Diagnostic / PreviewWorldSyncReport。
```

AI 禁止：

```text
禁止直接写 Scene 文件。
禁止直接写 Runtime ECS World。
禁止绕过 SceneEditTransaction。
```

## 第一版范围

第一版必须做：

```text
EditorSceneDocument load from scene file。
SceneSelection 单选。
SceneEditCommand 基础结构。
SceneEditTransaction 基础结构。
CreateEntity。
DeleteEntity。
SetTransform。
SetComponentField。
ReparentEntity。
SaveScene。
Undo / Redo 最小能力。
DirtyState。
PreviewWorldSync 全量重建。
Console / Diagnostic 反馈。
headless 单元测试。
```

第一版可以暂缓：

```text
真实 Viewport Gizmo。
复杂鼠标拾取。
多选批量编辑。
Prefab Mode。
嵌套 Prefab override。
Terrain / Light / NavMesh 工具。
复杂吸附 / 对齐。
拖拽资源进 Scene 自动生成 Entity。
多人协作锁。
复杂 Scene diff / merge。
```

## 命令标准结构

### CreateEntity

```text
CreateEntity:
  parent_id optional
  name
  components
  local_transform
  sibling_order optional
```

结果：

```text
新增 AuthoringEntityId。
写入 EditorSceneDocument。
dirty=true。
PreviewWorldSync。
Hierarchy 刷新。
```

### DeleteEntity

```text
DeleteEntity:
  entity_id
  delete_children: true
```

规则：

```text
第一版删除 Entity 时删除子树。
如果有外部 EntityRef 指向它，生成 diagnostic。
第一版可以阻止删除，也可以允许删除并清空引用；默认阻止。
```

### ReparentEntity

```text
ReparentEntity:
  entity_id
  new_parent_id optional
  sibling_order optional
  keep_world_transform: bool
```

第一版规则：

```text
默认 keep_world_transform=false。
禁止把 Entity 设为自己的子级。
禁止形成循环父子关系。
```

### SetTransform

```text
SetTransform:
  entity_id
  local_position optional
  local_rotation optional
  local_scale optional
```

规则：

```text
只写 local Transform。
world Transform 由 Transform 系统计算，不写入 Scene 文件。
```

### SetComponentField

```text
SetComponentField:
  entity_id
  component_type
  field_path
  value
```

规则：

```text
字段必须由 Component Schema 验证。
非法字段不写入。
类型不匹配不写入。
```

### SaveScene

```text
SaveScene:
  scene_id
  path optional
```

规则：

```text
保存前验证 EditorSceneDocument。
保存成功 dirty=false。
保存失败 dirty 保持 true。
```

## Trace / Report

SceneEditTransactionReport 第一版字段：

```text
schema_version = scene-edit-transaction-report.v1
transaction_id
request_id
command_kind
status
target_scene_id
affected_entity_ids[]
read_set[]
write_set[]
diagnostics[]
dirty_after
preview_sync_status
```

PreviewWorldSyncReport 第一版字段：

```text
schema_version = preview-world-sync-report.v1
scene_id
sync_mode = full_rebuild
entity_count
component_count
diagnostics[]
```

AI 查 Bug 优先看：

```text
SceneEditTransactionReport
PreviewWorldSyncReport
EditorSceneDocument revision
Console diagnostics
```

## 最小测试用例

### 测试 1：创建飞机 Entity

输入：

```text
CreateEntity name=PlayerPlane
components: Transform, Renderable
```

期望：

```text
EditorSceneDocument 新增 Entity。
Hierarchy 出现 PlayerPlane。
dirty=true。
PreviewWorldSync 成功。
Undo 后 Entity 消失。
Redo 后 Entity 恢复。
```

### 测试 2：移动飞机

输入：

```text
SelectEntity PlayerPlane
SetTransform local_position=(10, 0, 0)
```

期望：

```text
EditorSceneDocument 中 Transform 修改。
PreviewWorld 中飞机位置更新。
dirty=true。
Transaction write_set 包含 transform.local_position。
```

### 测试 3：Inspector 修改组件字段

输入：

```text
SetComponentField entity=PlayerPlane component=Health field=max_hp value=100
```

期望：

```text
Component Schema 验证通过。
字段写入。
Inspector 刷新。
Undo 可恢复。
```

### 测试 4：非法 Reparent

输入：

```text
ReparentEntity parent=Child child=Parent
```

期望：

```text
检测到循环父子关系。
Transaction rejected。
EditorSceneDocument 不变化。
dirty 不变化。
Console 输出 diagnostic。
```

### 测试 5：保存 Scene

输入：

```text
SaveScene
```

期望：

```text
Scene 文件写回项目目录。
dirty=false。
再次加载 Scene 后数据一致。
```

## 与其它引擎对比

| 项目 | Unity | UE | 我们 |
|---|---|---|---|
| 用户体验 | SceneView + Inspector + Hierarchy | LevelViewport + Details + Outliner | Unity-like 简洁体验 |
| 编辑工具 | Tools / Handles / EditorToolManager | EditorMode / Widget | ActiveSceneTool C-min |
| 修改入口 | SerializedObject / Undo | FScopedTransaction / Modify | SceneEditCommand / SceneEditTransaction |
| 场景保存 | EditorSceneManager.SaveScene | MarkPackageDirty / SavePackage | SceneSavePipeline |
| 选择状态 | Selection | GEditor Selection | SceneSelection |
| Runtime 隔离 | Edit Mode / Play Mode 分离 | Editor World / PIE World 分离 | EditorSceneDocument / PreviewWorld / RuntimeWorld 分离 |
| AI 友好 | 弱 | 弱 | 强，命令和报告结构化 |
| 第一版复杂度 | 成熟系统 | 成熟系统 | C-min，可控 |

## 正式规则

```text
1. Scene Editing v1 采用 C-min。
2. EditorSceneDocument 是编辑真相。
3. Runtime World 不是编辑真相。
4. PreviewWorld 是预览结果，可重建。
5. 所有修改 Scene 源数据的操作必须走 SceneEditCommand。
6. 所有 SceneEditCommand 必须进入 SceneEditTransaction。
7. Transaction 负责验证、应用、Undo/Redo、Dirty、Diagnostic。
8. SceneView / Hierarchy / Inspector / AI 不能直接修改 EditorSceneDocument。
9. AI 只能生成 SceneEditCommand，不能直接写 Scene 文件或 Runtime ECS。
10. Scene 保存必须走 SceneSavePipeline。
11. Save 成功 dirty=false，失败 dirty 保持 true。
12. 第一版 PreviewWorldSync 允许全量重建。
13. 第一版不实现完整 Gizmo，但保留 ActiveSceneTool 边界。
14. 第一版不实现完整 Prefab Mode、复杂多选、拖拽资源入场景。
```

## 后续施工边界建议

下一步如果生成施工文档，范围应是：

```text
EditorSceneDocument。
SceneEditCommand。
SceneEditTransaction。
SceneSelection。
DirtyState。
SceneSavePipeline。
PreviewWorldSync full rebuild。
EditorSession 接入基础 Scene Editing command。
最小 headless 测试。
```

不要在第一版施工中做：

```text
真实 Gizmo。
复杂鼠标拾取。
Prefab Mode。
Asset Browser drag into Scene。
复杂 Inspector 编辑器。
```

## 2026-06-28 ʵʩ��ɲ��䣺85 Scene Editing v1 C-min

```text
ʩ���ĵ�����ɲ��鵵��ʩ���ĵ�/�����/85-��ǰ���Զ���ʩ���ĵ�-SceneEditing-v1-C-min.md
�׶���ɼ�¼���׶���ɼ�¼/2026-06-28-SceneEditing-v1-C-min/00-����.md

������أ�
rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/lib.rs

�����������
EditorSceneDocument / EditorSceneEntity / SceneSelection / SceneEditRequest / SceneEditCommand / SceneEditTransaction / SceneEditTransactionReport / SceneDirtyState / SceneUndoStack / PreviewWorldSync full rebuild / SceneSavePipeline / EditorSession headless Scene Editing ���� / UI model ��Сˢ�� / Console diagnostic ��С������

��֤��
cargo test -p editor_core��59 passed
cargo test --workspace��passed

���������·�������ʩ���ĵ���������ʵ UI ������롢Inspector ��С�ɱ༭�ֶΡ�Hierarchy ��С�༭������Viewport picking / Gizmo C-min��
```
