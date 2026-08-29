# 99-Runtime Prefab Spawn / Despawn C-min 方案

## 1. 问题是什么

`70-Scene-Prefab-Entity-Runtime实例化方案.md` 已经确认并实现了底层路线：

```text
RuntimePrefabData
  -> RuntimeInstanceLoader
  -> RuntimePrefabInstance
  -> SourceEntityId -> RuntimeEntityId map
  -> World / ECS
```

但项目规则现在还缺少正式运行时调用链：

```text
Project Rule
  -> LogicContext
  -> CommandBuffer
  -> FrameLoop apply point
  -> RuntimeInstanceLoader
  -> World / ECS
  -> Trace / Report
```

如果没有这一层，项目规则要生成运行时对象时只能手写 `SpawnEntity` 和组件数据，无法自然复用 Prefab。这样会让 AI 生成规则变复杂，也会让复杂项目后期维护变差。

本系统只讨论引擎底座能力，不讨论 Bullet / Enemy / Explosion 等项目玩法概念。

## 2. 其他引擎怎么做

### Unity

Unity 用户心智是：

```text
Object.Instantiate(prefab)
Object.Destroy(instance)
```

特点：

```text
API 很简单。
Prefab asset 和 prefab instance 区分清楚。
底层实例化、生命周期、引用修复大量隐藏在 native 层。
适合用户，但对 AI 调试不够透明。
```

### Unreal Engine

UE 运行时生成对象主要是：

```text
UWorld::SpawnActor
UGameplayStatics::BeginDeferredActorSpawnFromClass
UGameplayStatics::FinishSpawningActor
AActor::Destroy
```

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\World.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Actor.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\GameplayStatics.cpp
```

特点：

```text
能力很强。
支持 deferred spawn。
生命周期和组件注册较重。
适合大型项目，但第一版照搬会过度复杂。
```

### Bevy

Bevy ECS 心智更接近：

```text
Commands.spawn(...)
SceneSpawner / DynamicScene
InstanceId
despawn_recursive
EntityMap / MapEntities
```

特点：

```text
适合 ECS。
实例化需要保存 entity map。
despawn 可以按 instance / root entity 清理。
AI 能读懂实例到实体的映射。
```

### Godot

Godot 用户心智是：

```text
PackedScene.instantiate()
Node.add_child()
queue_free()
```

特点：

```text
接口简单。
树结构心智直接。
复杂引用修复和大型项目分阶段实例化能力弱于 UE。
```

## 3. 推荐方案

采用：

```text
Unity-like API 心智
+ Bevy-like EntityMap 实现
+ UE-like 最小分阶段 report
```

第一版 C-min 只开放项目规则需要的最小通用能力：

```text
LogicContext.request_instantiate_prefab(prefab_ref, parent_entity, target_scene_instance)
LogicContext.request_despawn_prefab_instance(instance_id)
LogicContext.request_despawn_entity(entity_id)
FrameLoop / ProjectLogicRunner 在固定 apply point 统一应用命令
RuntimeInstanceLoader 负责真实实例化 / 销毁
RuntimeTrace 记录 enqueue 和 apply
```

## 4. 标准结构

### 4.1 GameplayCommand 扩展

```text
GameplayCommand
  SpawnEntity
  DespawnEntity
  AddComponent
  RemoveComponent
  SetParent
  InstantiatePrefab
  DespawnPrefabInstance
```

`InstantiatePrefab`：

```text
prefab_ref: RuntimeAssetRef
parent_entity: Option<SourceEntityId>
target_scene_instance: Option<RuntimeInstanceId>
```

`DespawnPrefabInstance`：

```text
instance_id: RuntimeInstanceId
```

### 4.2 GameplayCommandApplyRecord 扩展

第一版保持外层统一，减少 trace 复杂度：

```text
GameplayCommandApplyRecord
  command_id
  operation
  entity_id
  result
  error_code
  instance_id
  prefab_ref_id
  created_entity_count
```

规则：

```text
AI / Trace 默认只读 operation / result / entity_id / instance_id / error_code。
Prefab 详细 source_to_runtime_entity map 仍保留在 RuntimeInstanceLoader report 中。
```

### 4.3 LogicContext API

项目规则只允许通过 `LogicContext` 发请求：

```text
request_instantiate_prefab(prefab_ref, parent_entity, target_scene_instance) -> GameplayCommandId
request_despawn_prefab_instance(instance_id) -> GameplayCommandId
request_despawn_entity(entity_id) -> GameplayCommandId
```

规则：

```text
LogicContext 不暴露 RuntimeInstanceLoader。
项目规则不能在规则执行中立即拿到 root_entity_id。
项目规则只拿到 command_id / request_id。
真实 root_entity_id 在 apply 后进入 Trace / Report。
```

原因：

```text
保持规则执行期间 World 写入顺序简单。
避免同一帧规则直接依赖刚实例化实体导致隐式时序。
如果项目需要后续引用，项目规则应在下一帧通过 query / marker / component 找到实例。
```

## 5. 执行流程

### 5.1 Instantiate Prefab

```text
Project Rule:
  ctx.request_instantiate_prefab(prefab_ref, parent_entity, target_scene_instance)

ProjectLogicRunner:
  collect pending commands

Apply point:
  apply_gameplay_commands_with_runtime(...)
    -> RuntimeInstanceLoader.instantiate_prefab_from_package(...)
    -> World / ECS
    -> GameplayCommandApplyRecord
    -> RuntimeTrace
```

### 5.2 Despawn Prefab Instance

```text
Project Rule:
  ctx.request_despawn_prefab_instance(instance_id)

Apply point:
  RuntimeInstanceLoader.despawn_prefab_instance(instance_id, world)
  release owned asset handles
  remove owned entities
  write trace
```

### 5.3 Despawn Entity

`DespawnEntity` 保留现有 CommandBuffer 路线。

规则：

```text
DespawnEntity 只删除单个 Entity。
DespawnPrefabInstance 删除整个 prefab instance 拥有的 Entity 集合。
项目规则自己选择语义。
```

## 6. 边界

第一版不做：

```text
Prefab override
Prefab variant
运行时编辑 prefab asset
异步分帧 instantiate
复杂依赖加载策略
deferred construction callback
项目玩法专用 API，例如 SpawnBullet / SpawnEnemy
```

第一版必须保留：

```text
RuntimePrefabInstanceId
root_entity_id report
source_to_runtime_entity map
instantiate trace
despawn trace
headless tests
```

## 7. 为什么适合我们

AI 友好：

```text
AI 生成项目规则时只需要生成 request_instantiate_prefab / request_despawn_prefab_instance。
Trace 能说明请求来自哪条规则、在哪个 apply point 生效、结果是什么。
```

复杂项目可维护：

```text
Prefab 内部 EntityRef remap、资源 handle、despawn 清理由 RuntimeInstanceLoader 管。
项目规则不需要手写一整棵 Entity。
```

简单度：

```text
不增加玩法概念。
不增加复杂生命周期。
不让项目规则直接访问底层 loader。
```

效率：

```text
所有实例化统一在 command apply point 执行。
第一版同步执行，后续可在同一命令结构下升级为分帧或异步。
```

