# 226-PrefabInstance RuntimePackage Bake / Authoring Prefab Instance Expansion v1 方案

## 1. 系统定义

本系统正式命名为：

```text
PrefabInstance RuntimePackage Bake / Authoring Prefab Instance Expansion v1
```

选择方案：

```text
方案 C-min：构建期 bake Scene 中预放的 PrefabInstance，RuntimePackage 仍保留 PrefabAsset 给运行时动态 spawn 使用。
```

一句话说明：

```text
用户在 Scene 里放的是 PrefabInstance；Play / Preview / Export 时，ProjectRuntimePackageAssembler 必须把它解析成真正 RuntimeScene entities。
```

它解决的是：

```text
PrefabAsset + Scene PrefabInstance + PrefabOverride
  -> RuntimePackageBuildInput
  -> flattened RuntimeScene entities
  -> Windows Player / Editor GameView 真实运行
```

它不是新的 gameplay 系统，不引入：

```text
Player / Enemy / Bullet / Score / Wave 等打飞机专用 API
新的 Runtime Prefab Authoring 解释器
新的 Scene 导出桥
新的 Runtime 热路径 report 常驻系统
```

## 2. 为什么现在需要它

225 已完成：

```text
复杂打飞机 sample 已有 3 个 PrefabAsset。
Main.scene.json 已出现真实 engine.prefab_instance。
Prefab / Rule authoring asset completeness report 不再固定 partial。
```

但 225 完成记录明确留下断点：

```text
当前 ProjectRuntimePackageAssembler 尚未展开 authoring Scene 里的 engine.prefab_instance。
sample 的 entity-enemy-a 仍保留 SpriteRenderer2D / project.linearMotion 本地运行时组件，避免 gameplay 回归。
完整 PrefabInstance -> RuntimePackage bake / expansion 仍 deferred。
```

如果不做本系统，复杂打飞机会出现一个很危险的假完成：

```text
编辑器里看起来有 PrefabInstance
Prefab authoring report 也能看到 PrefabInstance
但导出的 RuntimePackage 并不是由 PrefabAsset + Override 真正生成运行对象
```

这会让用户和 AI 都误判：

```text
改 PrefabAsset 不一定影响导出结果。
Scene 中 PrefabInstance 可能只是一个 authoring 标记。
Bug 排查时无法判断问题来自 PrefabAsset、Override、Scene placeholder，还是 RuntimePackage assembly。
```

所以本系统是 225 之后最直接的落地缺口。

### 2.1 外部审查前置复核

2026-07-09 读取：

```text
<internal-review-root>\01-226-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion方案审查.md
```

审查对象与本文一致，审查结论为：

```text
方案方向正确，建议采纳 C-min。
施工前需要复核 225 闭环状态，并补充 bake 实现细则。
```

对 225 闭环状态的当前仓库复核：

```text
施工文档/当前/ 当前为空。
225 施工文档已存在于 施工文档/已完成/。
阶段完成记录/2026-07-08-Project-Authoring-Asset-Completeness-Prefab-Rule-Assetization-Gate-v1/00-总览.md 存在。
49 / 54 当前入口已记录 225 完成和 PrefabInstance RuntimePackage Bake 后续缺口。
```

因此审查中关于“225 闭环未正式关闭”的判断在当前仓库状态下已由历史同步吸收，不再阻塞本文方案；但生成 226 施工文档前仍必须再次确认：

```text
施工文档/当前/ 为空。
225 阶段完成记录存在。
49 / 54 没有回退到旧状态。
```

## 3. 其它引擎对标

### 3.1 Unity

参考：

```text
官方文档：
https://docs.unity3d.com/Manual/PrefabInstanceOverrides.html

源码参考：
https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/Prefabs/PrefabUtility.cs
本地参考：
框架设计/Unity源码参考/Scene-Prefab-GameObject-Instantiation源码参考.md
```

Unity 的心智是：

```text
Prefab asset 是资源。
Prefab instance 是 Scene 中的实例和 override。
PrefabUtility.InstantiatePrefab(asset, scene/parent) 会在 Scene 中产生 GameObject tree。
Build / Play 最终运行的是已解析的 GameObject / Component 对象。
```

可以学习：

```text
Prefab asset 与 prefab instance 必须区分。
Prefab instance 可以放到指定 Scene / parent。
Instance override 是一等编辑概念。
```

不照搬：

```text
不照搬 Unity native 黑盒 instantiate。
不照搬完整 Prefab Variant / nested prefab / missing script 历史复杂度。
不把 MonoBehaviour 生命周期引入本项目。
```

### 3.2 UE

参考：

```text
官方文档：
https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Engine/GameFramework/AActor

本地参考：
框架设计/UE源码参考/Scene-Level-Actor-Instantiation源码参考.md
```

UE 的核心链路是：

```text
Map / Level Package 已加载
  -> UWorld::AddToWorld(ULevel)
  -> IncrementalUpdateComponents
  -> ULevel::RouteActorInitialize
  -> Actor RegisterAllComponents / BeginPlay
  -> Level visible
```

可以学习：

```text
场景对象进入 Runtime World 必须分阶段。
Component 注册、引用解析、激活、渲染资源初始化不能混成不可诊断黑盒。
复杂场景后期要预留增量化和阶段报告。
```

不照搬：

```text
不引入 UObject / UClass / Actor / Component 反射体系。
不引入 Construction Script 全套生命周期。
不做 World Partition / streaming level 复杂度。
```

### 3.3 Godot

参考：

```text
官方文档：
https://docs.godotengine.org/en/stable/classes/class_packedscene.html

源码参考：
https://github.com/godotengine/godot/blob/master/scene/resources/packed_scene.cpp
本地参考：
框架设计/Godot源码参考/04-Object-Node-PackedScene源码参考.md
```

Godot 的心智是：

```text
.tscn/.scn
  -> PackedScene
  -> SceneState
  -> instantiate()
  -> Node tree
```

可以学习：

```text
场景文件 / prefab 资源不等于运行时对象。
PackedScene 保存 authoring data，instantiate 创建运行时对象。
运行时身份和 source ownership 需要清楚。
```

不照搬：

```text
不把 Node / ObjectDB / ClassDB 作为本项目核心对象模型。
不让用户面对弱类型 NodePath / Variant 黑盒。
```

## 4. 当前项目基线

### 4.1 已有能力

当前已有：

```text
rust/crates/editor_core/src/prefab_workflow.rs
  PrefabAsset
  PrefabInstance
  PrefabOverride
  ResolvedPrefabView::resolve(...)

rust/crates/editor_core/src/project_runtime_package_assembler.rs
  ProjectRuntimePackageAssembler
  collect_prefabs(...)
  editor_scene_to_runtime(...)

rust/crates/engine_runtime/src/runtime_package_builder.rs
  authoring-prefab-asset.v1 -> runtime-prefab.v1

rust/crates/engine_runtime/src/runtime_instance_loader.rs
  RuntimeInstanceLoader::instantiate_prefab_from_package(...)
  RuntimePrefabInstance
  PrefabInstantiateReport
```

已有运行时动态 prefab spawn：

```text
Rule / GameplayCommand
  -> instantiate_prefab
  -> RuntimeInstanceLoader::instantiate_prefab_from_package(...)
```

### 4.2 当前断点

当前 `ProjectRuntimePackageAssembler::assemble(...)` 的顺序是：

```text
read project manifest
read default scene
input.scenes.push(editor_scene_to_runtime(scene))
input.prefabs = collect_prefabs(...)
```

而 `editor_scene_to_runtime(...)` 当前只是：

```text
scene.entities
  .iter()
  .map(editor_entity_to_runtime)
  .collect()
```

这意味着：

```text
engine.prefab_instance 会作为普通 component 进入 RuntimeScene。
PrefabAsset 不会被查找。
PrefabOverride 不会被应用。
Prefab root / child entity 不会被展开。
Scene 中的 PrefabInstance 还不是真正 RuntimePackage truth。
```

### 4.3 sample 当前状态

`samples/complex_shooter_project/Scenes/Main.scene.json` 中：

```text
entity-enemy-a
  SpriteRenderer2D
  project.linearMotion
  engine.prefab_instance -> prefab-enemy-scout
```

这是一种过渡状态：

```text
engine.prefab_instance 是 225 新增的 authoring truth evidence。
SpriteRenderer2D / project.linearMotion 是为了本轮之前不让 runtime 退化而保留的本地运行时组件。
```

本系统施工时应把它收敛成：

```text
PrefabAsset enemy_scout 包含敌机运行所需组件。
Scene entity-enemy-a 只保留 PrefabInstance + Scene placement / override。
RuntimePackage scene 由 bake 后的 RuntimeEntity 承载运行数据。
```

## 5. 可选方案

### 5.1 方案 A：Authoring-time 展开成普通 Scene Entity

做法：

```text
用户把 prefab 放入 Scene 时，直接复制 PrefabAsset entities 到 Scene。
Scene 里长期保存普通 Entity。
```

优点：

```text
RuntimePackage assembly 最简单。
不需要 bake 阶段解析。
```

缺点：

```text
Prefab 链接容易丢失。
后续修改 PrefabAsset 不会稳定影响实例。
AI 难判断对象来自 prefab 还是复制体。
和 203 / 225 已建立的 PrefabAsset + PrefabInstance + Override 心智冲突。
```

结论：

```text
不推荐。
```

### 5.2 方案 B：Runtime load 时解释 PrefabInstance

做法：

```text
RuntimePackage scene 保留 engine.prefab_instance component。
Runtime load scene 时查找 package.prefabs，再动态展开。
```

优点：

```text
Scene 文件更接近 authoring 原貌。
运行时可以统一复用 instantiate_prefab 路径。
```

缺点：

```text
Runtime 热路径要理解 authoring-only component。
RuntimePackage scene 不再是简单运行真相。
Play / Export 的问题更晚暴露。
Report / deterministic gate 更难定位是 build 失败还是 runtime load 失败。
```

结论：

```text
可作为未来高级动态场景加载能力参考，但不适合复杂打飞机当前落地。
```

### 5.3 方案 C：Build-time bake + Runtime dynamic spawn 保留

做法：

```text
Scene 中预放的 PrefabInstance：
  ProjectRuntimePackageAssembler 构建 RuntimePackage 时展开成 RuntimeScene entities。

Rule / gameplay 运行中动态生成的 Prefab：
  保留现有 RuntimeInstanceLoader::instantiate_prefab_from_package(...)。

RuntimePackage：
  Scene 是已 bake 的 runtime truth。
  Prefabs 仍保留为 runtime-prefab.v1，供动态 spawn 使用。
```

优点：

```text
AI 适配性最好：bake report 可以明确说明每个实例如何展开。
复杂项目可维护：PrefabAsset / PrefabInstance / RuntimeScene 边界清楚。
运行效率好：正式 runtime load 不需要解释 authoring prefab_instance。
和 189 / 203 / 217 / 225 主线一致：ProjectRuntimePackageAssembler 是唯一项目装配入口，Editor Play 也走 RuntimePackage 真相。
```

缺点：

```text
ProjectRuntimePackageAssembler 需要多一个 prefab bake 阶段。
施工时要处理 deterministic id remap、root transform、override diagnostics。
```

结论：

```text
采用方案 C-min。
```

## 6. 方案 C-min 详细规则

### 6.1 真相层规则

Authoring truth：

```text
Prefabs/*.prefab.json
  PrefabAsset 真相。

Scenes/*.scene.json
  普通 Entity 真相。
  engine.prefab_instance 表示 Scene 中的 PrefabInstance 真相。
  PrefabInstance overrides 表示该实例相对 PrefabAsset 的修改。
```

Build truth：

```text
ProjectRuntimePackageAssembler
  是项目目录进入 RuntimePackageBuildInput 的唯一入口。
  负责把 Scene 中的 PrefabInstance bake 成 RuntimeScene entities。
```

Runtime truth：

```text
RuntimePackage scenes/*.json
  包含已 bake 的 RuntimeEntity。
  不应依赖 authoring-only engine.prefab_instance 才能运行。

RuntimePackage prefabs/*.json
  保留 runtime-prefab.v1。
  只给运行中动态 instantiate_prefab 使用。
```

### 6.2 Bake 输入

输入：

```text
EditorSceneDocument
PrefabBakeCatalog / authoring_prefab_by_id
Project root
Asset collector
```

注意：当前 `collect_prefabs(...)` 的真实返回类型是：

```text
Vec<RuntimePackageSourcePrefab> {
  prefab_id,
  document: serde_json::Value
}
```

它不是 `ResolvedPrefabView::resolve(asset, instance)` 可直接使用的 `PrefabAsset map`。

C-min 需要在 `ProjectRuntimePackageAssembler` 内构建一个 bake catalog：

```text
PrefabBakeCatalog
  runtime_source_prefabs: Vec<RuntimePackageSourcePrefab>
  authoring_prefab_by_id: BTreeMap<String, PrefabAsset>
  diagnostics: Vec<ProjectRuntimePackageAssemblyDiagnostic>
```

构建规则：

```text
collect_prefabs(project_root, assets, diagnostics)
  -> 保留原始 RuntimePackageSourcePrefab 给 input.prefabs
  -> 对 schemaVersion=authoring-prefab-asset.v1 的 document 反序列化为 PrefabAsset
  -> authoring_prefab_by_id[prefab_id] = PrefabAsset
```

如果 Scene PrefabInstance 引用的 prefab 只存在 runtime-prefab.v1、缺少 authoring PrefabAsset：

```text
C-min 不做反向还原。
assembly 输出 Error:
  scene_prefab_instance_requires_authoring_prefab_asset
suggested_fix:
  Keep scene-placed PrefabInstance backed by Prefabs/*.prefab.json authoring-prefab-asset.v1.
```

施工时需要调整 assembly 顺序：

```text
先 collect_prefabs 得到 runtime_source_prefabs
再 build_prefab_bake_catalog(runtime_source_prefabs)
再 editor_scene_to_runtime_with_prefab_bake(scene, authoring_prefab_by_id, ...)
最后 input.prefabs = runtime_source_prefabs
```

避免：

```text
Scene bake 时还没有 PrefabAsset 信息。
误把 Vec<RuntimePackageSourcePrefab> 当成 PrefabAsset map。
另建临时 prefab 导出桥。
```

### 6.3 Bake 流程

每个 Scene Entity：

```text
if entity 没有 engine.prefab_instance:
  走现有 editor_entity_to_runtime(...)

if entity 有 engine.prefab_instance:
  PrefabInstance::from_scene_entity(entity)
  找 PrefabAsset
  ResolvedPrefabView::resolve(asset, instance)
  应用 Scene root placement
  remap source entity ids
  转成 RuntimeEntity[]
  写 PrefabRuntimeBakeReport instance entry
```

Override 与 remap 的顺序必须固定：

```text
PrefabOverride.targetSourceEntityId 始终指向 PrefabAsset 内的 sourceEntityId。
ResolvedPrefabView::resolve(...) 先按 prefab source id 应用 override。
只有在生成 RuntimeEntity 阶段，才把 sourceEntityId remap 成 runtime entity id。
```

禁止：

```text
施工时把 PrefabOverride.targetSourceEntityId 改写成 runtime entity id。
施工时先 remap 再应用 override。
```

原因：

```text
同一个 PrefabAsset 会被多个 Scene PrefabInstance 复用。
Override 只有绑定 prefab source id，才能跨实例稳定复用和审查。
runtime entity id 是 bake 产物，不是 authoring truth。
```

### 6.4 ID remap 规则

C-min 使用 deterministic remap：

```text
Prefab root sourceEntityId
  -> Scene placeholder entity_id

Prefab child sourceEntityId
  -> {scene_placeholder_entity_id}__{source_entity_id}
```

示例：

```text
entity-enemy-a + prefab root entity-enemy-scout-root
  -> runtime entity id = entity-enemy-a

child muzzle
  -> runtime entity id = entity-enemy-a__muzzle
```

父子关系：

```text
Prefab root parent_id
  -> Scene placeholder parent_id

Prefab child parent_source_entity_id
  -> remapped parent runtime entity id
```

基础字段继承规则：

```text
Root RuntimeEntity:
  id = Scene placeholder entity_id
  name = Scene placeholder name
  parent_id = Scene placeholder parent_id
  sibling_order = Scene placeholder sibling_order
  enabled = Scene placeholder enabled && resolved prefab root enabled
  transform = Scene placeholder transform

Child RuntimeEntity:
  id = {scene_placeholder_entity_id}__{prefab_child_source_entity_id}
  name = resolved prefab child name
  parent_id = remapped parent runtime entity id
  sibling_order = resolved prefab child sibling_order
  enabled = resolved prefab child enabled
  transform = resolved prefab child transform
```

当前代码基线中：

```text
PrefabEntity 已有 sibling_order / enabled。
ResolvedPrefabEntity 当前只保留 source_entity_id / name / parent_source_entity_id / transform / components。
```

因此施工时必须二选一：

```text
优先：扩展 ResolvedPrefabEntity，保留 sibling_order / enabled。
或：bake 阶段按 source_entity_id 从 PrefabAsset.entity(...) 取回 sibling_order / enabled。
```

禁止：

```text
展开 RuntimeEntity 时把 sibling_order / enabled 临时写死为默认值。
```

`__` 是 C-min reserved separator：

```text
Prefab sourceEntityId 不得包含 "__"。
如果 sourceEntityId 包含 "__"，assembly 输出 Error:
  prefab_source_entity_id_contains_reserved_separator
suggested_fix:
  Rename prefab source entity id or use a later escaped-id remap scheme.
```

好处：

```text
运行时 Inspector / Report / E2E gate 可以稳定追踪。
AI 能从 runtime entity id 反查 source instance。
不会因 HashMap 顺序导致导出不稳定。
```

### 6.5 Root transform / placement 规则

PrefabAsset 根实体提供模板结构。

Scene placeholder entity 提供实例摆放：

```text
root RuntimeEntity.transform = Scene placeholder transform
```

Prefab child transform：

```text
保持 PrefabAsset / PrefabOverride resolve 后的 transform。
```

如果 PrefabInstance overrides 中同时修改 root `engine.transform`，C-min 规则是：

```text
Scene placeholder transform 优先。
root engine.transform override 产生 warning：
  root_transform_override_shadowed_by_scene_placement
建议用户把根摆放写在 Scene Entity transform，把 prefab 内部节点偏移写在 child transform override。
```

原因：

```text
用户在 Scene 里拖动 PrefabInstance，心智上就是移动实例根。
不能让 PrefabAsset root 原点或 root transform override 把 Scene 摆放吃掉。
```

### 6.6 Component 规则

PrefabInstance placeholder 上的 `engine.prefab_instance` 是 authoring-only：

```text
Bake 后不进入 RuntimeScene components。
```

PrefabInstance placeholder 上的其它 runtime component 在 C-min 中不作为正常长期真相：

```text
如果存在 SpriteRenderer2D / project.* / Collider2D 等本地 runtime component：
  bake report 输出 warning:
    prefab_instance_local_runtime_component_shadowed
  suggested_fix:
    move component into PrefabAsset or express as PrefabOverride
```

C-min 施工目标应迁移 complex shooter sample：

```text
enemy_scout.prefab.json 补齐运行所需 SpriteRenderer2D / project.linearMotion。
entity-enemy-a 不再依赖本地重复 runtime component 才能正常运行。
```

sample 敌机一致性规则：

```text
entity-enemy-a 当前已有 engine.prefab_instance，应作为首个 bake 验收对象。
entity-enemy-b 当前仍是普通复制实体；C-min 推荐一并迁移为 prefab-enemy-scout 的 Scene PrefabInstance。
```

如果施工选择暂时保留 `entity-enemy-b` 为普通实体，必须在 `PrefabRuntimeBakeReport` 或 project_e2e_gate report 中解释：

```text
why_entity_enemy_b_left_as_plain_scene_entity
```

默认推荐：

```text
两个 Enemy Scout 都成为 prefab-enemy-scout 的 Scene PrefabInstance。
这样 sample 不会出现同类敌机一个走 prefab、一个走复制实体的长期混乱状态。
```

不做：

```text
不默认把 placeholder local runtime components 合并进 prefab bake 结果。
不创建“PrefabInstance + local component 混合运行真相”。
```

### 6.7 Override 规则

继续复用：

```text
ResolvedPrefabView::resolve(asset, instance)
```

支持：

```text
component field override
engine.transform localPosition / localRotation / localScale override
```

失败诊断：

```text
missing_prefab_asset
scene_prefab_instance_requires_authoring_prefab_asset
invalid_prefab_ref
missing_source_entity
invalid_override_field
prefab_source_entity_id_contains_reserved_separator
runtime_expand_failed
root_transform_override_shadowed_by_scene_placement
prefab_instance_local_runtime_component_shadowed
```

严重性：

```text
缺 PrefabAsset / override 无法解析 -> Error，RuntimePackage assembly failed。
Scene PrefabInstance 只找到 runtime-prefab.v1、找不到 authoring PrefabAsset -> Error。
Prefab sourceEntityId 包含 "__" reserved separator -> Error。
placeholder local runtime component -> Warning，施工期间先迁移 sample，后续可升级为 gate warning。
root transform override 被 Scene placement 覆盖 -> Warning。
```

## 7. Report 设计

新增或扩展：

```text
PrefabRuntimeBakeReport
```

它是 build/editor/gate evidence，不进入正式 runtime 热路径。

Schema 建议：

```json
{
  "schemaVersion": "prefab-runtime-bake-report.v1",
  "status": "success|partial|failed",
  "reportMode": "summary|trace",
  "projectRoot": "...",
  "sceneId": "scene-main",
  "prefabAssetCount": 3,
  "scenePrefabInstanceCount": 1,
  "bakedInstanceCount": 1,
  "bakedEntityCount": 1,
  "instances": [
    {
      "sceneEntityId": "entity-enemy-a",
      "instanceId": "prefab-instance-enemy-scout-a",
      "prefabId": "prefab-enemy-scout",
      "rootSourceEntityId": "entity-enemy-scout-root",
      "rootRuntimeEntityId": "entity-enemy-a",
      "emittedEntityIds": ["entity-enemy-a"],
      "appliedOverrideCount": 1,
      "ignoredAuthoringComponentTypes": ["engine.prefab_instance"],
      "localRuntimeComponentWarnings": [],
      "diagnostics": []
    }
  ],
  "diagnostics": []
}
```

Report 分档：

```text
Summary:
  默认用于 ProjectRuntimePackageAssemblyReport / Report Panel / project_e2e_gate。
  只包含计数、实例摘要、错误和 next action。

Trace:
  只在 gate/debug/用户显式诊断时开启。
  可包含 source_to_runtime_entity_remap、完整 override list、组件类型展开细节。

Runtime:
  正式 runtime 默认 Off。
  不在每帧或加载热路径生成长 JSON。
```

## 8. 与现有系统关系

### 8.1 与 203 Prefab Authoring 的关系

203 已完成：

```text
PrefabAsset / PrefabInstance / PrefabOverride authoring workflow。
PrefabStageModel 是 editor-only，不进入 RuntimePackage。
PrefabAuthoringReport 区分 prefab_assets_count 与 prefab_instances_count。
```

226 接续 203：

```text
不重做 Prefab editor。
只让已保存的 PrefabAsset + Scene PrefabInstance 真正进入 RuntimePackage bake。
```

### 8.2 与 189 RuntimePackage Assembly 的关系

189 定义：

```text
ProjectRuntimePackageAssembler 是项目目录进入 RuntimePackageBuildInput 的唯一正式装配入口。
```

226 必须在这个入口内完成，不新增：

```text
ScenePrefabExportBridge
PrefabRuntimeBakeStandaloneExporter
DesktopExportPipeline 专属 prefab hack
```

### 8.3 与 217 / 218 Editor Play 的关系

Editor Play 当前走：

```text
saved project
  -> ProjectRuntimePackageAssembler
  -> RuntimePackage preview cache
  -> EditorRuntimePlayInstance
```

因此 226 完成后：

```text
Editor Play 和 Windows Export 会同时获得 PrefabInstance bake 能力。
不需要给 GameView 单独做一套 PrefabInstance 解释。
```

### 8.4 与 RuntimeInstanceLoader 的关系

RuntimeInstanceLoader 继续负责：

```text
运行时动态 instantiate_prefab。
运行时 despawn prefab instance。
RuntimePrefabInstance ownership / source_to_runtime mapping。
```

226 不把 RuntimeInstanceLoader 变成 authoring Scene placeholder 解释器。

## 9. 对复杂打飞机的直接价值

完成后，复杂打飞机可以把敌人/子弹/爆炸等对象收敛为：

```text
Prefabs/enemy_scout.prefab.json
Prefabs/player_bullet.prefab.json
Prefabs/explosion_effect.prefab.json

Scenes/Main.scene.json
  entity-enemy-a:
    engine.prefab_instance -> prefab-enemy-scout
    transform -> 关卡摆放
    overrides -> 本实例速度 / 血量 / 初始状态差异

Rules/*.rule.json / runtime rule manifest
  instantiate_prefab -> prefab-player-bullet
```

用户体验更接近 Unity：

```text
改 PrefabAsset -> 所有未覆盖实例生效。
改 Scene 中某个实例 -> 写 PrefabOverride。
点击 Play / Export -> RuntimePackage 使用真实展开结果。
```

AI 排查问题也更清楚：

```text
PrefabAsset 缺组件
Override 写错字段
Scene placement 被覆盖
RuntimePackage bake 没有展开
动态 spawn prefab 缺 runtime asset
```

这些会出现在不同 report / diagnostic 中，不需要猜日志。

## 10. 不做范围

C-min 不做：

```text
Nested Prefab。
Prefab Variant。
Prefab unpack / replace full workflow。
跨 prefab entity reference 的完整重写系统。
异步 / 增量 bake job。
Runtime load 解释 authoring-only prefab_instance。
完整 prefab dependency graph 可视化。
把 placeholder 本地 runtime component 合并成长期语义。
```

这些留给后续：

```text
Prefab Variant / Nested Prefab Productization v1
Prefab Dependency / Impact Analysis v1
Runtime Scene Streaming / Incremental Instantiate v1
```

## 11. 测试门禁建议

单元测试：

```text
cargo test -p editor_core prefab -- --nocapture
cargo test -p editor_core project_runtime_package_assembler -- --nocapture
```

重点覆盖：

```text
PrefabBakeCatalog parses authoring-prefab-asset.v1 into PrefabAsset map。
Scene PrefabInstance referencing only runtime-prefab.v1 fails with scene_prefab_instance_requires_authoring_prefab_asset。
scene prefab_instance expands into runtime scene entities。
root prefab source entity id remaps to scene placeholder entity id。
child prefab source entity id uses deterministic id prefix。
prefab sourceEntityId containing "__" fails with reserved separator diagnostic。
scene placeholder transform wins as root placement。
root RuntimeEntity sibling_order / enabled follow Scene placeholder + resolved root rules。
child RuntimeEntity sibling_order / enabled follow resolved prefab child rules。
PrefabOverride applies to component field。
PrefabOverride resolves by prefab sourceEntityId before runtime id remap。
invalid override fails assembly with diagnostic。
missing prefab asset fails assembly with diagnostic。
engine.prefab_instance is not emitted as runtime component after bake。
placeholder local runtime component emits warning。
```

RuntimePackage / runtime 测试：

```text
cargo test -p engine_runtime runtime_package_builder -- --nocapture
cargo test -p engine_runtime runtime_instance_loader -- --nocapture
```

重点覆盖：

```text
runtime-prefab.v1 仍写入 package，动态 instantiate_prefab 不退化。
Scene baked entities 与 runtime prefab assets 可以同时存在。
```

E2E gate：

```text
cargo test -p project_e2e_gate prefab_runtime_bake -- --nocapture
cargo test -p project_e2e_gate -- --nocapture
```

复杂打飞机验收：

```text
sample Scene 至少一个真实 engine.prefab_instance。
RuntimePackage scene 中不再需要 authoring-only engine.prefab_instance 才能运行。
entity-enemy-a baked from prefab-enemy-scout。
PrefabRuntimeBakeReport bakedInstanceCount >= 1。
complex shooter export/headless player 仍通过。
entity-enemy-a 和 entity-enemy-b 默认都收敛为 prefab-enemy-scout 的 Scene PrefabInstance；如保留 enemy-b 普通实体，report 必须给出 why_entity_enemy_b_left_as_plain_scene_entity。
```

整体回归：

```text
cd rust
cargo fmt --check
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p project_e2e_gate
```

## 12. 方案自审

### 12.1 AI 适配性

通过：

```text
PrefabAsset / PrefabInstance / Override 都是结构化数据。
Bake report 明确 prefabId / instanceId / sourceEntityId / runtimeEntityId / diagnostics。
失败原因有 code / source path / suggested fix。
```

风险：

```text
如果只把 PrefabInstance 静默展开，没有 report，AI 仍很难排查。
```

处理：

```text
226 必须把 PrefabRuntimeBakeReport 纳入 ProjectRuntimePackageAssemblyReport 或 project_e2e_gate artifact。
```

### 12.2 复杂项目适配性

通过：

```text
Scene 预放对象和运行时动态生成对象分开处理。
PrefabAsset 仍是模板真相。
RuntimePackage Scene 仍是运行真相。
```

风险：

```text
placeholder local runtime component 如果长期保留，会让 PrefabInstance 语义混乱。
```

处理：

```text
C-min 不把它作为长期合法运行真相。
施工时迁移 complex shooter sample 的 enemy_scout prefab，减少重复组件。
```

### 12.3 效率

通过：

```text
构建期展开，正式 runtime load 更简单。
Editor Play preview cache 已存在，bake 结果可参与 dirty domain cache。
动态 spawn 继续用已有 RuntimeInstanceLoader。
```

风险：

```text
大量 PrefabInstance 后构建期 bake 成本会上升。
```

处理：

```text
C-min 同步执行即可。
未来可在 RuntimePackage preview cache / dirty domain / incremental bake 中优化。
```

### 12.4 外部审查吸收记录

已读取审查文档：

```text
<internal-review-root>\01-226-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion方案审查.md
```

审查对象：

```text
226-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion-v1方案.md
```

适用性判断：

```text
审查对象、系统编号和本文一致。
审查结论可直接用于优化本文。
```

必须修改，已写入本文：

```text
1. collect_prefabs 返回 Vec<RuntimePackageSourcePrefab>，不是 PrefabAsset map。
   已在 §6.2 改为 PrefabBakeCatalog / authoring_prefab_by_id 规则。

2. override 必须先按 prefab sourceEntityId resolve，再做 runtime id remap。
   已在 §6.3 写入固定时序和禁止项。

3. sibling_order / enabled 继承规则缺失。
   已在 §6.4 写入 root / child 规则，并说明当前 ResolvedPrefabEntity 需扩展或回查 PrefabEntity。

4. child id remap 使用 "__" 存在碰撞风险。
   已在 §6.4 把 "__" 定义为 C-min reserved separator，并新增错误诊断。

5. entity-enemy-b 样例处置不明确。
   已在 §6.6 写入默认一并迁移为 prefab-enemy-scout Scene PrefabInstance。
```

施工约束，后续施工文档必须继承：

```text
1. 生成 226 施工文档前复核 施工文档/当前/ 为空。
2. 复核 225 阶段完成记录存在，49 / 54 没有回退旧状态。
3. Gate 测试必须覆盖 PrefabBakeCatalog、override-before-remap、sibling/enabled、reserved separator、enemy-b 一致性。
4. PrefabRuntimeBakeReport 必须进入 project_e2e_gate artifact 或 ProjectRuntimePackageAssemblyReport evidence。
```

已由当前仓库状态吸收：

```text
审查提出 225 闭环状态未正式关闭。
当前复核结果显示：施工文档/当前/ 为空，225 已在 施工文档/已完成/，225 阶段完成记录存在，49 / 54 已记录 225 完成和 226 后续缺口。
因此该项不再作为方案阻塞，但保留为施工前复核约束。
```

不适用：

```text
审查未提出需要推翻 C-min 或改选 A/B 的结论。
审查未要求新增 runtime authoring interpreter、嵌套 prefab、variant 或完整增量 bake。
```

## 13. 最终结论

采用：

```text
方案 C-min：Build-time bake Scene PrefabInstance + keep runtime dynamic prefab spawn。
```

核心规则：

```text
用户/AI 编辑 PrefabAsset 和 Scene PrefabInstance。
ProjectRuntimePackageAssembler 在构建 RuntimePackage 时展开 Scene 中预放 PrefabInstance。
RuntimePackage Scene 保存可直接运行的 RuntimeEntity。
RuntimePackage PrefabAsset 继续供 Rule 动态 instantiate_prefab。
所有展开、覆盖、跳过和失败都通过 PrefabRuntimeBakeReport 解释。
```

下一步如果进入施工，应生成：

```text
施工文档/当前/226-当前可自动化施工文档-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion-v1.md
```
