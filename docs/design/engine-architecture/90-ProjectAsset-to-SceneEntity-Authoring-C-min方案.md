# 90-ProjectAsset-to-SceneEntity Authoring C-min 方案

## 问题是什么

89 已经跑通了一个真实可编辑小项目闭环：

```text
open default scene
  -> hierarchy / inspector
  -> transform edit
  -> dirty
  -> save
  -> undo / redo
  -> play
  -> Console / Trace / EditableProjectLoopReport
```

但现在编辑器只能编辑已有 Entity。用户还不能像 Unity / UE / Godot 那样，把 ProjectDock / Project Library 里的资源放进 Scene，生成可保存、可预览、可运行的场景对象。

90 解决的是：

```text
Project asset
  -> Add / Drop into Scene
  -> Create Entity
  -> Attach Component / AssetRef
  -> Inspector can edit
  -> SaveScene
  -> Play can instantiate / render
```

这不是重新定义资源系统，也不是重新定义 Scene 数据模型。15 号文档已经确认过正式规则：

```text
用户从 Project Library 拖资源到 Hierarchy / Scene 时，不是把资源本身放进 Scene。
引擎会创建一个 Entity，并把资源通过 Component / AssetRef 挂到 Entity 上。
Scene 只保存 Entity / Component / AssetRef / Prefab Instance / Overrides。
```

90 的职责是把这条规则变成 Editor Core 可执行命令和 headless 可验证闭环。

## 其它引擎怎么做

### Unity

Unity 的用户体验是：

```text
Project Window asset
  -> drag into Hierarchy / Scene View
  -> create GameObject
  -> attach component
```

典型结果：

```text
Sprite / Texture -> GameObject + SpriteRenderer
Model / Prefab -> GameObject / Prefab Instance + MeshRenderer / Animator
Audio -> GameObject + AudioSource
```

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Commands\GOCreationCommands.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector\GameObjectInspector.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\DragAndDrop.bindings.cs
```

观察到的关键点：

```text
Unity 有明确的 Create / Place 路径。
拖拽只是一种 UI 输入方式，真正修改 Scene 的是创建 GameObject、放置 Transform、Undo 注册、Prefab 实例化等编辑事务。
```

### Unreal Engine

UE 的用户体验是：

```text
Content Browser asset
  -> drag into Level Viewport
  -> ActorFactory chooses Actor type
  -> spawn Actor / attach Component
```

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\EditorEngine.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\EditorViewportClient.cpp
```

关键实现点：

```text
UE 用 ActorFactory 作为资源到 Actor 的转换边界。
Viewport drop 和 Content Browser selected asset 最终都会走工厂/编辑器事务，而不是由 UI 直接改 Level。
```

### Godot

Godot 的用户体验是：

```text
FileSystem / Resource
  -> drop into 2D / 3D viewport or Scene tree
  -> instantiate PackedScene or create suitable Node
```

源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\editor\scene\canvas_item_editor_plugin.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\scene\3d\node_3d_editor_viewport.cpp
```

关键实现点：

```text
Godot 明确区分 can_drop_data / drop_data。
PackedScene 会实例化为 Node。
Texture / Mesh 等资源会按编辑器上下文生成或绑定到对应 Node。
```

## 方案对比

### 方案 A：UI 直接生成 SceneEditCommand

```text
ProjectDock / Viewport
  -> 根据 asset_type 自己拼 CreateEntity / SetComponentField
```

优点：

```text
第一版最快。
层级最少。
```

缺点：

```text
资源到 Entity 的默认规则散落在 UI。
AI 修改时容易不知道同一类资源应该生成什么组件。
后期 ProjectDock、Viewport、AI、测试会各写一套转换逻辑。
```

不推荐。

### 方案 B：新增 AssetPlacementResolver，只返回 SceneEditCommand

```text
UiCommandPayload::PlaceAssetIntoScene
  -> EditorSession
  -> AssetPlacementResolver
  -> SceneEditCommand
  -> SceneEditTransaction
```

优点：

```text
资源到 Entity 的规则集中。
UI / AI / Test 统一走同一条命令路径。
不绕过 EditorSceneDocument。
不增加 Runtime 规则。
第一版实现量适中。
```

缺点：

```text
会新增一个很小的 resolver 层。
第一版需要定义支持哪些 asset_type。
```

推荐。

### 方案 C：完整 Unity / UE 式 Placement / Factory 系统

```text
AssetPlacementRegistry
  -> typed factory
  -> placement preview
  -> snapping / surface placement
  -> multi asset drop
  -> prefab nested override
```

优点：

```text
长期最强。
最接近 UE ActorFactory。
```

缺点：

```text
第一版过重。
会拖慢最小可编辑游戏闭环。
容易把 Gizmo、Picking、Prefab Mode、Asset Importer 复杂度提前引进来。
```

暂不采用完整 C，只保留长期方向。

## 推荐方案

采用 **方案 B：AssetPlacementResolver C-min**。

核心链路：

```text
ProjectDock / Viewport / AI / Test
  -> UiCommandPayload::PlaceAssetIntoScene
  -> EditorSession
  -> AssetPlacementResolver
  -> SceneEditCommand::CreateEntity / SetComponentField
  -> SceneEditTransaction
  -> EditorSceneDocument
  -> PreviewWorldSync
  -> SaveScene
  -> PlaySession
```

关键原则：

```text
ProjectDock 不直接写 Scene。
Viewport 不直接写 Scene。
AI 不直接写 Scene 文件。
AssetPlacementResolver 不读真实文件系统。
AssetPlacementResolver 只读取已验证的 AssetRegistry / AssetPipelineState 派生数据。
资源不会被复制进 Scene。
Scene 只保存 Entity / Component / AssetRef / Prefab Instance / Overrides。
```

## C-min 第一版边界

第一版只支持三类资源：

```text
mesh / model:
  Entity + Transform + EditorMesh.asset_ref

texture / sprite:
  Entity + Transform + EditorMesh.asset_ref 或 SpriteRenderer 动态组件

prefab:
  Entity + Transform + PrefabInstance 动态组件
```

考虑当前代码已经存在：

```text
EditorSceneEntity.mesh
EditorMesh.asset_ref
EditorMesh.material_ref
RuntimeMesh.asset_ref
RuntimeMesh.material_ref
PreviewWorldSync -> Renderable
RuntimeInstanceLoader -> Renderable
```

所以 C-min 优先采用 `EditorMesh.asset_ref` 承载可渲染资源，避免第一版新增复杂 SpriteRenderer / MeshRenderer 类型树。

第一版不做：

```text
真实鼠标拖拽。
Viewport picking / surface placement。
snapping。
多资源批量 drop。
材质自动匹配。
模型子资源选择。
Prefab Mode。
Nested Prefab override。
资源导入器重跑。
真实文件系统扫描。
```

## 数据结构建议

### UiCommandPayload

新增：

```text
PlaceAssetIntoScene {
  asset_id: String,
  asset_type: String,
  asset_guid: Option<String>,
  target_parent_id: Option<String>,
  local_position: Option<Vec3>,
  placement_mode: AssetPlacementMode
}
```

第一版 `placement_mode` 只支持：

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

### AssetPlacementResolver

输入：

```text
AssetPlacementRequest
  asset_ref
  target_parent_id
  local_transform
  placement_mode
```

输出：

```text
AssetPlacementPlan
  scene_commands: Vec<SceneEditCommand>
  selected_entity_id: Option<String>
  diagnostics: Vec<AssetPlacementDiagnostic>
```

C-min 规则：

```text
mesh/model/texture/sprite -> 一个 CreateEntity 命令，entity.mesh.asset_ref 写入该 asset。
prefab -> 一个 CreateEntity 命令，components 中写入 PrefabInstance { source: AssetRef }。
不支持的 asset_type -> 返回 diagnostic，不修改 Scene。
```

### SceneEditCommand

长期可以新增：

```text
CreateEntityFromAsset { ... }
```

但 C-min 不建议直接这样做。原因是：

```text
SceneEditCommand 应保持对 Scene 修改的通用表达。
资源到 Entity 的翻译属于 AssetPlacementResolver。
最终落地仍然是 CreateEntity / SetComponentField。
```

这样 AI 和测试能同时看到：

```text
用户意图：PlaceAssetIntoScene
编辑结果：CreateEntity + Component / AssetRef
```

Trace 更容易解释。

## AI 友好规则

AI 生成资源入场操作时，不应该直接拼 Scene JSON。

AI 应生成：

```text
UiCommandPayload::PlaceAssetIntoScene
```

或者生成更高层的自然语言计划：

```text
把 player_ship.mesh 放进当前 Scene，位置在原点。
```

再由 EditorSession 转成统一命令。

报告必须包含：

```text
source_asset_id
source_asset_type
created_entity_id
created_component_types
scene_transaction_id
diagnostics
```

这样用户说“为什么我拖图片进来没显示”，AI 能从 report 追到：

```text
资源是否存在
asset_type 是否支持
是否创建 Entity
是否写入 AssetRef
是否 PreviewWorldSync 生成 Renderable
Runtime 是否能解析 AssetRef
```

## 与现有系统关系

不替代：

```text
25-Asset-DB-Importer-MVP.md
63 Asset Pipeline ProjectDock / Console UI 接入
85-Scene-Editing-v1-C-min方案.md
86-真实UI命令接入SceneEditing-C-min方案.md
89-真实可编辑小项目闭环-C-min方案.md
```

依赖：

```text
AssetRegistry / AssetPipelineState 提供 asset identity。
EditorSession 提供统一入口。
SceneEditTransaction 保证 dirty / undo / redo。
PreviewWorldSync 生成编辑器预览 World。
SceneSavePipeline 保存 Scene。
Runtime Package / RuntimeInstanceLoader 在 Play 后验证 AssetRef。
```

## 最小测试用例

### 用例 1：mesh asset 放入场景

输入：

```text
PlaceAssetIntoScene(asset_id="player_ship_mesh", asset_type="mesh")
```

期望：

```text
Scene 新增 Entity: player_ship_mesh
Entity 有 Transform
Entity.mesh.asset_ref.id == "player_ship_mesh"
Scene dirty == true
Hierarchy 出现新 Entity
Inspector 显示 Renderable / AssetRef
Undo 删除该 Entity
Redo 恢复该 Entity
```

### 用例 2：prefab asset 放入场景

输入：

```text
PlaceAssetIntoScene(asset_id="enemy_prefab", asset_type="prefab")
```

期望：

```text
Scene 新增 Entity: enemy_prefab
Entity 有 Transform
Entity.components 包含 PrefabInstance
PrefabInstance.source.id == "enemy_prefab"
Scene dirty == true
```

### 用例 3：不支持的 asset_type

输入：

```text
PlaceAssetIntoScene(asset_id="readme", asset_type="text")
```

期望：

```text
不修改 Scene
Scene dirty 不变化
Console / Report 产生 unsupported_asset_type diagnostic
AI 可读 suggested_fix
```

### 用例 4：保存并 Play

输入：

```text
PlaceAssetIntoScene(mesh)
SaveScene
Play
```

期望：

```text
保存后的 Scene 文件包含 Entity + AssetRef
PlaySession 可以加载 Runtime Package
Runtime Trace 能看到对应 Entity / Renderable
```

## 为什么适合我们

按项目优先级判断：

```text
AI 友好：
  资源入场统一用 PlaceAssetIntoScene，AI 不需要直接写 Scene JSON。

复杂项目能力：
  资源到 Entity 的规则集中在 resolver，后期可扩展 asset_type / placement mode / prefab 规则。

后期可维护：
  UI、AI、Test 不各自实现一套转换逻辑。

简单：
  C-min 不引入完整 Factory 注册体系，不提前做真实拖拽和 Gizmo。

效率：
  这是编辑期操作，不在 Runtime 高频路径；Runtime 仍只看到普通 Scene / Entity / Component / AssetRef。
```

## 结论

90 第一版采用：

```text
AssetPlacementResolver C-min
UiCommandPayload::PlaceAssetIntoScene
AssetPlacementPlan -> SceneEditCommand
SceneEditTransaction 负责真正写 Scene
```

它对齐 Unity 的 Project -> Scene / Hierarchy 体验，借鉴 UE ActorFactory 的集中转换边界，也保留 Godot 对 PackedScene / Resource 入场的清晰分流，但第一版只实现最小可编辑游戏项目需要的资源入场能力。

## 下一步

如果确认本方案，下一步生成：

```text
施工文档/当前/90-当前可自动化施工文档-ProjectAsset-to-SceneEntity-Authoring-C-min.md
```

施工范围应限制在：

```text
editor_ui_model: 新增 PlaceAssetIntoScene command payload
editor_core: 新增 AssetPlacementResolver / report / EditorSession 接入
editor_input 或 editor_host: 增加 headless command route 测试
editor_core: 增加 mesh / prefab / unsupported asset_type / save-play 闭环测试
文档归档和阶段完成记录
```
