# 140-M7 Prefab Workflow / Reusable Authoring Object System v1 方案

## 1. 本文解决什么

本文定义 `M7 Prefab Workflow`：

```text
Prefab 可复用对象编辑工作流 v1
```

它不是重新讨论已经完成的运行时实例化能力。已有规则继续有效：

```text
15-Scene-Entity-Component-Prefab数据模型.md
70-Scene-Prefab-Entity-Runtime实例化方案.md
99-Runtime-Prefab-Spawn-Despawn-C-min方案.md
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
```

本文补齐的是编辑器和构建侧的产品工作流：

```text
PrefabAsset 创建 / 编辑 / 保存
Scene 中放置 PrefabInstance
PrefabInstance Override 编辑
Prefab Asset 与 Prefab Instance 的引用关系
RuntimePackageBuilder 展开 Prefab
Project Rule 按 PrefabRef 实例化
Report / Trace / Test 能说明 Prefab 链路问题
```

没有这个系统，复杂项目会退化成大量复制粘贴 Entity 和组件数据。AI 也会被迫重复生成同类对象，而不是复用稳定模板。

本文只定义引擎底座能力，不允许把以下项目侧概念做成引擎 API：

```text
Player
Enemy
Bullet
Health
Damage
Score
Wave
Weapon
Boss
Drop
```

这些必须由项目侧通过 `Project Schema / Project Rule / Prefab / Asset / AUI` 组合表达。

## 2. 其它引擎对应模块

### 2.1 Unity

Unity 对应的是：

```text
Prefab Asset
Prefab Instance
Prefab Overrides
Apply / Revert
Prefab Stage / Prefab Mode
PrefabUtility
```

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\HierarchyEditor\ScriptBindings\HierarchyGameObjectHandler.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\HierarchyEditor\Managed\HierarchyWindow.cs
```

可借鉴：

```text
Prefab Asset 和 Scene Instance 明确分离。
Scene 中实例默认保留 Prefab 关系，而不是复制成普通对象。
Instance 修改以 Override 形式记录。
需要防止循环嵌套。
```

不照搬：

```text
第一版不做完整 Prefab Mode。
第一版不做完整 Nested Prefab。
第一版不做 Variant。
第一版不做 Unity 那套大量隐藏 native 行为。
```

### 2.2 Unreal Engine

UE 没有 Unity 同名 Prefab，但对应心智是：

```text
Blueprint Class / Actor Class Defaults
Placed Actor Instance
Actor Component Defaults
SpawnActor
Deferred Spawn
```

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Actor.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\World.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\GameplayStatics.cpp
```

可借鉴：

```text
模板对象与实例对象分离。
运行时生成走统一 Spawn 流程。
大型项目需要清晰的实例生命周期和组件初始化阶段。
```

不照搬：

```text
UE Blueprint/Class Default Object 体系太重。
第一版不把 Prefab 做成完整脚本类系统。
第一版不引入复杂 Actor 生命周期。
```

### 2.3 Godot

Godot 对应的是：

```text
PackedScene
SceneState
PackedScene.instantiate()
Node scene_instance_state / inherited_state
```

源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\resources\packed_scene.cpp
<GODOT_SOURCE>\godot-master\godot-master\scene\main\node.cpp
<GODOT_SOURCE>\godot-master\godot-master\scene\property_utils.cpp
```

可借鉴：

```text
Scene 本身可以作为可复用模板。
实例化心智简单。
节点树和资源状态能被序列化。
```

不照搬：

```text
我们底层是 ECS，不直接采用 Node 树运行模型。
第一版不做完整 Scene 继承系统。
```

### 2.4 Bevy

Bevy 对应的是：

```text
DynamicWorld
WorldAsset
SceneSpawner
InstanceId
EntityMap / MapEntities
```

源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_world_serialization\src\world_asset_spawner.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_world_serialization\src\dynamic_world.rs
```

可借鉴：

```text
实例化时保留 InstanceId。
source entity 到 runtime entity 的映射必须显式存在。
despawn 可以按 instance 清理。
Entity 引用需要 remap。
```

不照搬：

```text
Bevy 没有 Unity-like 编辑器 Prefab 产品流。
我们需要让 AI 和用户能在编辑器里直接看懂 PrefabAsset / PrefabInstance / Override。
```

## 3. 可选方案

### 3.1 方案 A：只保留 Runtime Prefab

```text
Project Rule
  -> Runtime InstantiatePrefab
```

编辑器里不建立完整 Prefab 工作流，Scene 继续以普通 Entity 为主。

优点：

```text
最快。
代码改动最少。
```

缺点：

```text
复杂项目复用能力弱。
AI 会重复生成对象结构。
Scene 中无法表达“这是某个 Prefab 的实例”。
Prefab 资产修改后无法统一影响实例。
后续一定返工。
```

不推荐。

### 3.2 方案 B：Unity-like Prefab Asset + Instance + Overrides B-min

```text
PrefabAsset
  -> root entity tree
  -> component values
  -> asset refs

Scene
  -> normal entities
  -> prefab instances

PrefabInstance
  -> prefab_ref
  -> parent
  -> overrides
```

优点：

```text
用户心智接近 Unity。
AI 能清楚区分模板、实例和覆盖。
能支撑复杂项目复用。
能衔接已有 RuntimePrefabData / RuntimeInstanceLoader。
第一版复杂度可控。
```

缺点：

```text
需要改 Editor Authoring / Inspector / Asset Browser / RuntimePackageBuilder。
需要定义最小 override 规则。
```

推荐。

### 3.3 方案 C：完整 Unity / UE 级 Prefab / Blueprint

第一版直接支持：

```text
Nested Prefab
Prefab Variant
Prefab Mode
Apply / Revert / Unpack 全套 UI
Prefab 继承
复杂循环检测
批量迁移
```

优点：

```text
长期能力最强。
```

缺点：

```text
第一版过重。
会拖慢复杂打飞机主线。
容易让规则膨胀。
会把我们再次带回细节无限讨论。
```

暂不推荐。

## 4. 推荐方案

采用：

```text
方案 B：Unity-like Prefab Asset + Instance + Overrides B-min
+ Bevy-like EntityMap / InstanceId 实现
+ UE-like 生命周期 report
```

第一版目标：

```text
Prefab 是一等 Authoring Asset。
Scene 可以放置 PrefabInstance。
Inspector 编辑 PrefabInstance 时默认产生 Override。
RuntimePackageBuilder 能把 PrefabInstance 展开成 RuntimeSceneData / RuntimePrefabData。
Project Rule 能通过 PrefabRef 实例化。
Trace / Report 能定位 Prefab 缺失、Override 失败、循环引用、展开失败。
```

第一版明确不做：

```text
Prefab Variant。
完整 Nested Prefab。
完整 Prefab Mode。
复杂 Apply / Revert UI。
Prefab 继承。
项目玩法专用对象类型。
```

## 5. 标准数据结构

### 5.1 PrefabAsset

`PrefabAsset` 是 authoring truth。

```text
PrefabAsset:
  prefab_id
  name
  source_path
  root_entity_id
  entities: PrefabEntity[]
  metadata
```

```text
PrefabEntity:
  source_entity_id
  name
  parent_source_entity_id
  components: ComponentValueMap
  asset_refs: AssetRef[]
```

规则：

```text
PrefabAsset 保存完整模板数据。
PrefabAsset 不保存某个 Scene 的 instance 状态。
PrefabAsset 可以被 Asset Browser、Inspector、RuntimePackageBuilder 读取。
```

### 5.2 PrefabInstance

`PrefabInstance` 是 Scene 中对 Prefab 的引用和覆盖。

```text
PrefabInstance:
  instance_id
  prefab_ref
  scene_parent_entity_id
  instance_root_entity_id
  overrides: PrefabOverride[]
```

```text
PrefabOverride:
  target_source_entity_id
  component_type
  field_path
  value: ComponentValue
```

规则：

```text
Scene 默认不复制 PrefabAsset 的完整实体树。
Scene 只保存 prefab_ref 和 overrides。
PrefabInstance 展示时可以通过 resolved view 显示完整结构。
```

### 5.3 ResolvedPrefabView

`ResolvedPrefabView` 是编辑器和构建器用于查看展开结果的只读视图。

```text
ResolvedPrefabView:
  prefab_ref
  instance_id
  resolved_entities
  applied_overrides
  diagnostics
```

规则：

```text
ResolvedPrefabView 不是新的真相层。
它可以缓存，但缓存失效后必须能从 PrefabAsset + PrefabInstance 重建。
AI / Inspector / Report 默认读 ResolvedPrefabView。
保存时仍写 PrefabAsset 或 PrefabInstance overrides。
```

## 6. 编辑器工作流

### 6.1 创建 Prefab

```text
Scene selected entity tree
  -> Create Prefab
  -> PrefabAsset
  -> Asset Browser lists prefab
```

规则：

```text
第一版支持从选中的 Entity Tree 创建 PrefabAsset。
创建后 Scene 中可以选择保留普通 Entity，也可以替换为 PrefabInstance。
默认推荐替换为 PrefabInstance，但必须由命令显式执行。
```

### 6.2 放置 Prefab

```text
Asset Browser select Prefab
  -> Place In Scene
  -> PrefabInstance
  -> Hierarchy shows instance root
```

规则：

```text
放置 Prefab 不直接复制成普通实体。
Hierarchy 必须能标记该对象来自 PrefabInstance。
Inspector 必须能看到 prefab_ref 和 overrides。
```

### 6.3 编辑 Prefab Instance

```text
Inspector field edit
  -> PrefabOverride
  -> Scene dirty
  -> ResolvedPrefabView update
```

规则：

```text
编辑 instance 默认写 override。
不允许无提示直接修改 PrefabAsset。
如果字段不属于 prefab 源实体，返回 structured diagnostic。
```

### 6.4 编辑 Prefab Asset

第一版只做最小能力：

```text
打开 PrefabAsset。
编辑 PrefabAsset 自身组件字段。
保存 PrefabAsset。
Scene 中引用该 Prefab 的实例在 reload / rebuild 时重新 resolve。
```

不做完整 Prefab Mode，但需要保持数据边界清楚。

### 6.5 Apply / Revert

第一版只保留数据能力和最小命令，不要求复杂 UI：

```text
RevertOverride(instance_id, override_path)
ApplyOverrideToPrefab(instance_id, override_path)
```

规则：

```text
Revert 删除 PrefabOverride。
Apply 把 override 写回 PrefabAsset，并删除对应 instance override。
第一版可以只在测试和命令层支持。
```

## 7. Build / RuntimePackage 流程

```text
Project Document
  -> PrefabAsset
  -> Scene PrefabInstance
  -> RuntimePackageBuilder
  -> Resolve PrefabAsset
  -> Apply Overrides
  -> RuntimeSceneData / RuntimePrefabData
  -> RuntimeInstanceLoader
  -> ECS World
```

规则：

```text
RuntimePackageBuilder 是 authoring prefab 到 runtime prefab 的唯一生产入口。
RuntimePackage 中必须保留 enough metadata 供 report 定位 source prefab / instance / override。
Runtime 不读取 editor-only UI 状态。
Runtime 可以通过 RuntimePrefabData 实例化项目规则请求的 PrefabRef。
```

## 8. Project Rule 接入

项目规则只能通过通用底座表达：

```text
InstantiatePrefab(prefab_ref, parent_entity, target_scene_instance)
DespawnPrefabInstance(instance_id)
DespawnEntity(entity_id)
```

规则：

```text
Project Rule 不直接依赖 PrefabAsset 编辑器结构。
Project Rule 不知道 Player / Enemy / Bullet。
Project Rule 只知道 PrefabRef 和通用命令。
```

## 9. Diagnostics / Report

第一版最小报告：

```text
PrefabWorkflowReport:
  prefab_assets_count
  prefab_instances_count
  overrides_count
  resolved_instances_count
  failed_instances_count
  diagnostics[]
```

```text
PrefabDiagnostic:
  severity
  code
  prefab_ref
  instance_id
  source_entity_id
  field_path
  message
```

标准错误码：

```text
missing_prefab_asset
invalid_prefab_ref
missing_source_entity
invalid_override_field
cyclic_prefab_reference
resolve_failed
apply_override_failed
revert_override_failed
runtime_expand_failed
```

规则：

```text
Report 是 AI / 用户查问题入口。
Report 不保存完整 Scene dump。
Report 必须能回指 prefab_ref / instance_id / field_path。
```

## 10. 测试门禁

第一版必须有以下测试：

```text
PrefabAsset can be created from entity tree.
PrefabInstance can reference PrefabAsset.
Inspector edit on instance creates override.
ResolvedPrefabView applies override correctly.
Revert removes override.
Apply writes back to PrefabAsset.
RuntimePackageBuilder resolves PrefabInstance into runtime data.
Project Rule InstantiatePrefab can instantiate runtime prefab.
Missing prefab_ref produces diagnostic.
Cyclic prefab reference is rejected.
Save / reload preserves prefab_ref and overrides.
```

## 11. 与其它系统的边界

### 11.1 与 Scene Editing

Scene Editing 负责：

```text
选中对象。
创建实体。
删除实体。
移动层级。
触发创建 Prefab / 放置 Prefab 命令。
```

Prefab Workflow 负责：

```text
PrefabAsset / PrefabInstance / Override 的语义。
```

### 11.2 与 Inspector

Inspector 负责：

```text
展示字段。
提交字段编辑命令。
显示 override 状态。
```

Prefab Workflow 负责：

```text
判断字段编辑写入 PrefabAsset 还是 PrefabInstance Override。
```

### 11.3 与 Asset Browser

Asset Browser 负责：

```text
列出 PrefabAsset。
选择 PrefabAsset。
触发放置到 Scene。
```

Prefab Workflow 负责：

```text
PrefabAsset 的 authoring 数据和 resolved 语义。
```

### 11.4 与 RuntimePackageBuilder

RuntimePackageBuilder 负责：

```text
把 authoring prefab 展开为 runtime 数据。
```

Prefab Workflow 负责：

```text
提供稳定、可验证的 authoring prefab 输入。
```

## 12. 方案自审

### 12.1 Specification fit

本文满足 M7 缺失能力：补齐 Prefab 编辑器产品工作流，并衔接 RuntimePackage 和项目规则实例化。

### 12.2 Rule fit

本文遵守现有规则：

```text
引擎只提供底座能力。
不加入项目玩法 API。
不重复讨论已完成 Runtime Prefab Spawn / Despawn。
Prefab 接入 RuntimePackageBuilder，而不是绕过构建链路。
```

### 12.3 Textual consistency

本文使用统一术语：

```text
PrefabAsset 是真相层。
PrefabInstance 是 Scene 引用和覆盖。
ResolvedPrefabView 是只读展开视图，不是真相层。
RuntimePackageBuilder 是 authoring 到 runtime 的唯一生产入口。
```

不存在同时把 cache / view 当真相层的问题。

### 12.4 Design fit

该方案符合项目优先级：

```text
AI 友好：对象模板、实例、覆盖清楚可读。
复杂项目：支持对象复用和统一更新。
可维护：不把复制后的实体散落到 Scene。
简单：第一版不做 Variant / Nested / 完整 Prefab Mode。
效率：运行时仍消费 RuntimePrefabData，不读取编辑器结构。
```

### 12.5 Implementation feasibility

当前已有：

```text
RuntimePrefabData
RuntimeInstanceLoader
GameplayCommand::InstantiatePrefab
RuntimePackageBuilder 基础
Editor Authoring Workspace
Inspector Field Editing 基础
Asset Browser 基础
```

因此 M7 可以在现有架构上施工，不需要推翻底层。

### 12.6 Practical reasonableness

B-min 版本覆盖复杂打飞机需要的复用对象能力，同时避免一次性实现完整 Unity Prefab。它可测试、可诊断、可分阶段施工。

结论：

```text
方案通过自审，可以作为 M7 正式规则。
```
