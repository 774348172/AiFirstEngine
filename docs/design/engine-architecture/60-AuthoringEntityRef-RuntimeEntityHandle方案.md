# AuthoringEntityRef / RuntimeEntityHandle 方案

本文档定义 Scene / Prefab / Runtime Package 中的 Entity 引用如何保存，以及运行时如何修复成可访问的 Entity Handle。

## 1. 问题是什么

项目中会出现大量 Entity 引用：

```text
WeaponComponent.muzzle -> player_muzzle
ProjectileComponent.owner -> player
ProjectileComponent.target -> enemy_01
SkillState.lockTarget -> boss
CameraFollow.target -> player
```

关键矛盾：

```text
Scene / Prefab 文件需要稳定、可序列化、可被 AI 修改。
Runtime ECS 需要高效、可验证、能防止旧引用误命中新 Entity。
```

因此不能把 RuntimeEntityId 直接写入文件。

## 2. 其它引擎怎么做

### Unity

参考文档：

```text
../Unity源码参考/ObjectReference-PPtr-GlobalObjectId.md
```

Unity 的思路：

```text
编辑器对象引用通过 SerializedProperty.objectReferenceValue 读写。
底层用 PPtr / EntityId / GlobalObjectId 等稳定引用结构。
可序列化 PPtr 可表现为 guid + fileID + type。
ExposedReference 用 exposedName + resolver 做上下文解析。
```

可学习：

```text
文件里保存可重建引用，不保存当前内存对象。
Inspector 通过统一入口校验引用。
Missing Reference 是可显示、可诊断状态。
```

不照搬：

```text
不照搬 fake null。
不让用户面对 guid + fileID + type。
不把所有引用合成 Unity ObjectReference 黑箱。
```

### Unreal Engine

参考文档：

```text
../UE源码参考/ObjectReference-Weak-SoftObjectPath.md
```

UE 的思路：

```text
TWeakObjectPtr / FWeakObjectPtr = ObjectIndex + ObjectSerialNumber。
FSoftObjectPath = 可延迟加载的路径引用。
Deferred Spawn 缓存使用 TWeakObjectPtr，并清理失效对象。
```

可学习：

```text
运行时弱引用不保活对象。
槽位复用必须有 generation / serial 防误命中。
可序列化引用和运行时引用必须分层。
```

不照搬：

```text
不引入 UObject / GC / UPROPERTY 体系。
不让 Entity 生命周期由强引用自动保活。
```

### Bevy

参考文档：

```text
../Bevy源码参考/12-Scene-EntityRef-MapEntities.md
```

Bevy 的思路：

```text
Entity 只在当前 World 有效。
Scene / World 写入目标 World 时先创建所有目标 Entity。
EntityHashMap 保存 source Entity -> target Entity。
MapEntities / ReflectComponent.apply_or_insert_mapped 修复 Component 内 Entity 字段。
```

可学习：

```text
先分配所有 Entity，再修复引用。
Component 中的 Entity 字段由统一机制 remap。
```

不照搬：

```text
不要求 AI / 用户写 MapEntities trait。
不把引用修复隐藏在 Rust 派生细节里。
```

## 3. 方案选择

### 方案 A：文件直接保存 RuntimeEntityId

```text
Scene / Prefab 中直接保存 runtimeEntityId(index, generation)。
```

优点：

```text
实现最简单。
运行时无需 fixup。
```

缺点：

```text
Scene 每次加载都会分配新 RuntimeEntityId，文件中的 id 立即失效。
Prefab 多实例无法区分不同实例内的同名引用。
AI 修改文件时无法稳定理解引用来源。
槽位复用后风险极高。
```

结论：

```text
不采用。
```

### 方案 B：只有字符串路径引用

```text
所有 EntityRef 都保存 scene path / hierarchy path。
```

优点：

```text
用户可读。
AI 容易猜。
```

缺点：

```text
重命名 / 移动层级容易断。
同名节点和 Prefab 多实例容易歧义。
运行时查找成本高。
```

结论：

```text
只作为 debugName / fallback，不作为主引用。
```

### 方案 C：AuthoringEntityRef + RuntimeEntityHandle

```text
Scene / Prefab / Runtime Package 保存 AuthoringEntityRef。
Runtime 加载 / Spawn 后生成 RuntimeEntityHandle。
```

优点：

```text
AI 可读、可修改、可验证。
Scene / Prefab 文件稳定。
Runtime 防止旧引用误命中新对象。
适合复杂项目和 Prefab 多实例。
和 Unity / UE / Bevy 的成熟经验一致。
```

缺点：

```text
需要 SceneInstantiator 做 fixup。
需要 Component Schema 标记 EntityRef 字段。
需要 diagnostics 记录 missing reference。
```

结论：

```text
采用 C-min。
```

## 4. 正式结构

### AuthoringEntityRef

`AuthoringEntityRef` 只存在于编辑器数据、Scene、Prefab、Runtime Package 中。

```text
AuthoringEntityRef:
  kind
  sourceEntityId?
  prefabLocalId?
  sceneId?
  path?
  expectedComponent?
  allowMissing?
  debugName?
```

字段含义：

```text
kind:
  scene_local
  prefab_local
  owner
  parent
  self

sourceEntityId:
  Scene 内稳定 Entity ID。

prefabLocalId:
  Prefab 内稳定局部 Entity ID。

sceneId:
  只用于显式跨 Scene 引用。第一版默认不开放隐式跨 Scene 引用。

path:
  调试 / 迁移 / 人类可读 fallback，不作为主键。

expectedComponent:
  可选校验，例如 target 必须有 Health。

allowMissing:
  true 时允许 Missing Reference，但必须产生 diagnostics。

debugName:
  给 Inspector / Trace / AI 报告显示。
```

第一版允许的 kind：

```text
scene_local
prefab_local
self
parent
owner
```

第一版不允许：

```text
implicit_cross_scene
runtime_query_ref
path_only_required_ref
auto_rebind_ref
strong_keep_alive_ref
```

### RuntimeEntityHandle

`RuntimeEntityHandle` 只存在 Runtime 内存、事件队列、延迟请求、Trace / Diagnostics 中。

```text
RuntimeEntityHandle:
  runtimeEntityId:
    index
    generation
  sceneInstanceId?
  sourceEntityId?
  issuedFrame?
  debugName?
```

规则：

```text
RuntimeEntityHandle 不写入 Scene / Prefab / Runtime Package。
RuntimeEntityHandle 不保活 Entity。
RuntimeEntityHandle 使用前必须 resolve。
generation 不匹配时不能命中新 Entity。
```

## 5. Component Schema 如何声明 EntityRef

Component Schema 必须标记哪些字段是 EntityRef。

最小字段：

```text
type:
  EntityRef

required:
  true / false

scope:
  self
  parent
  owner
  scene_local
  prefab_local
  runtime

expected:
  anyOf ComponentTag / ComponentType

allowMissing:
  true / false

display:
  pickerLabel / debugName 可选
```

示例：

```yaml
ProjectileComponent:
  fields:
    owner:
      type: EntityRef
      required: true
      scope: runtime
      expected:
        anyOf: [PlayerTag]

    target:
      type: EntityRef
      required: false
      scope: runtime
      expected:
        anyOf: [Health]

    damage:
      type: number
```

作用：

```text
编辑器 Inspector 知道这个字段要显示 Entity Picker。
AI Patch 知道不能填 runtimeEntityId。
SceneInstantiator 知道加载时要 fixup。
Validation 知道 expectedComponent / allowMissing 怎么检查。
RuntimeTrace 知道 missing ref 怎么解释。
```

规则：

```text
type=EntityRef 表示该字段需要 Entity Picker / AI 引用校验 / Runtime fixup。
scope 限定引用来源，不做复杂查询。
expected 只做组件存在性校验，不表达业务条件。
required=false 允许空引用，但不等于允许坏引用。
allowMissing=true 允许 Missing Reference 留在数据中，但必须产生 diagnostics。
display 只影响编辑器展示，不影响 Runtime 语义。
```

禁止：

```text
在 Component Schema 中写复杂查询 DSL。
用 expected 表达阵营、距离、血量、AI 状态等业务条件。
让 EntityRef 自动强引用保活 Entity。
让 EntityRef 自动跨 Scene 搜索。
让旧 EntityRef 自动重绑定到新 Entity。
用户 / AI 直接填写 runtimeEntityId 或 RuntimeEntityHandle。
```

## 6. Scene / Prefab 加载流程

正式流程：

```text
1. 读取 Scene / Prefab / Runtime Package。
2. 收集所有 sourceEntityId / prefabLocalId。
3. 分配所有 RuntimeEntityId。
4. 建立 AuthoringRefKey -> RuntimeEntityHandle 映射。
5. 写入 Transform / Hierarchy / 普通 Component。
6. 扫描 Component Schema 中的 EntityRef 字段。
7. 把 AuthoringEntityRef 修复为 RuntimeEntityHandle。
8. 对 missing / generation / scene 状态输出 diagnostics。
9. 提交 EntitySpawned / SceneLoaded Trace。
```

关键规则：

```text
必须先分配全部 Entity，再写 Component EntityRef。
Prefab 多实例必须生成不同 RuntimeEntityHandle。
Prefab 内 prefab_local 引用只解析到当前实例内部。
Scene local 引用只解析到当前 SceneInstance。
```

## 7. Runtime Spawn 规则

RuntimeSpawnRequest 生成对象时：

```text
Prefab 中的 prefab_local AuthoringEntityRef
  -> 当前 spawn instance 内 RuntimeEntityHandle

parent / owner / self
  -> Runtime Spawn System 根据上下文填充

外部传入 target
  -> 必须已经是 RuntimeEntityHandle
```

运行时新产生的引用：

```text
事件、延迟请求、Projectile.target、HitEvent.target 使用 RuntimeEntityHandle。
它们不回写到 AuthoringEntityRef。
```

## 8. Despawn / Scene Unload 后的引用行为

```text
Entity despawn 后，对应 RuntimeEntityHandle 失效。
Scene unload 后，属于该 SceneInstance 的 RuntimeEntityHandle 统一失效。
旧 handle 不会自动重绑定。
旧 handle 不会阻止 Entity 被销毁。
Runtime 记录有界 tombstone diagnostics。
```

resolve 失败码：

```text
entity_not_found
generation_mismatch
pending_despawn
scene_unloaded
missing_authoring_ref
expected_component_missing
```

## 9. 打飞机小游戏测试

测试流程：

```text
Scene:
  player
  player_muzzle
  enemy_01

WeaponComponent:
  muzzleRef = AuthoringEntityRef(scene_local, player_muzzle)

Runtime:
  SceneInstantiator fixup muzzleRef -> RuntimeEntityHandle(player_muzzle)
  Player fire -> RuntimeSpawnRequest(Bullet)
  ProjectileComponent.owner = RuntimeEntityHandle(player)
  ProjectileComponent.muzzle = RuntimeEntityHandle(player_muzzle)
  ProjectileComponent.target = RuntimeEntityHandle(enemy_01)
  Bullet hit enemy_01
  RuntimeDespawnRequest(enemy_01)
  enemy_01 old handle invalid
  slot reused by enemy_02
  old enemy_01 handle generation_mismatch，不会命中 enemy_02
```

测试结论：

```text
muzzle authoring ref fixup 成功。
Projectile owner / target 可 resolve。
Enemy despawn 后旧 target 失效。
槽位复用后旧 target 不会误命中新 Enemy。
tombstone 能保留 sourceEntityId / sceneInstanceId / despawnReason。
```

说明：

```text
该方案能覆盖最小射击游戏的 Scene 引用、Prefab/Spawn 引用、运行时目标引用、Despawn 失效、槽位复用防误命中。
```

## 10. 最终规则

```text
Scene / Prefab / Runtime Package:
  保存 AuthoringEntityRef。

Runtime:
  使用 RuntimeEntityHandle。

Component Schema:
  标记 EntityRef 字段。

SceneInstantiator:
  先分配全部 RuntimeEntityId。
  再批量 fixup EntityRef 字段。

Runtime:
  RuntimeEntityHandle 使用前 resolve。

Despawn:
  handle 失效。
  generation 推进。
  写 tombstone diagnostics。
```

禁止：

```text
Scene / Prefab / Runtime Package 保存 runtimeEntityId。
用户 / AI 直接填写 RuntimeEntityHandle。
EntityRef 只靠 hierarchy path。
旧引用自动重绑定。
EntityRef 自动强引用保活 Entity。
```

## 11. 为什么适合本项目

AI 友好：

```text
AuthoringEntityRef 是结构化、可读、可验证数据。
AI 修改 Scene / Prefab 时不需要理解 runtime slot / generation。
missing reference 可以直接定位到字段和 sourceEntityId。
```

复杂项目友好：

```text
Prefab 多实例、Scene 多实例、Runtime Spawn、Despawn 都有统一引用模型。
不靠散落在各系统里的特殊修复逻辑。
```

维护友好：

```text
引用规则集中在 Schema + SceneInstantiator + Runtime resolve。
不是每个 Component / System 自己发明引用生命周期。
```

简单度：

```text
只保留两层：AuthoringEntityRef 和 RuntimeEntityHandle。
不引入 UObject 式万能对象引用。
不引入复杂所有权图和强引用保活。
```
