# Scene / Entity / Component / Prefab 数据模型

## 当前归属说明：Projection 术语

本文中如果出现以下历史名称：

```text
RenderExtract
RenderAssetBridge / Render Asset Bridge
Physics2DBridge
RuntimeScene Hydration
AuiRenderExtract / AuiRendererBridge
SpriteRenderer2D ECS-to-RenderProxy Bridge
```

统一按 `110-World-Projection-Adapter统一跨域同步规则.md` 理解为：

```text
RenderProjection
AssetProjection
Physics2DProjection
HydrationProjection
UiProjection
RenderProjectionAdapter<SpriteRenderer2D>
```

这些名称可以作为历史实现名保留，但不再作为新增架构概念扩展。后续新增类型只新增对应 `ProjectionAdapter`，不新增独立 Bridge。

本文档定义编辑器对象模型与运行时 ECS 的边界。

## 基础确认

编辑器层统一使用：

```text
Entity
Component
Scene
Prefab
```

## Audio Component v1 implementation rule

Current minimal implementation:

```text
AudioComponent
  clipRef: AssetRef<audio>
  volume: number 0..1
  loop: boolean
  spatial: boolean
  autoplay: boolean
```

Boundary:

```text
AudioComponent is pure data.
It does not implement play / stop / update logic.
Asset DB derives audio dependencies from clipRef.
Runtime audio extraction exposes audio sources for runtime backends. 旧 RenderSnapshot 说法只属于过渡 MVP。
Audio Runtime Backend performs the platform-specific loading / playback.
Project rules decide high-level playback timing beyond autoplay.
```

Scene 文件保存：

```text
Entity 树
```

而不是保存 ECS 初始快照。

运行时加载时：

```text
Scene Entity Tree
  -> Resolve Prefab
  -> Apply Overrides
  -> Validate Component Schema
  -> Build Spawn Plan
  -> Spawn ECS Entities
  -> Attach Component Storage
  -> Bind IR Rule / Behavior
```

用户看到的是：

```text
Scene 里有 Entity
Entity 上有 Component
Prefab 是可复用 Entity 模板
```

引擎内部执行的是：

```text
ECS World
```

## Entity 与 Transform 规则

正式规则：

```text
Entity 本体只是轻量 ID。
Transform 是引擎内置 Component。
Scene / Prefab / Hierarchy 中可见的 Entity 默认必须有 Transform。
纯逻辑 Entity / 数据 Entity 不强制拥有 Transform。
```

Entity 本体不保存复杂关系。  
它只代表一个对象身份，基础信息包括：

```text
EntityId
Generation / Version
所属 Scene
Alive 状态
```

父子层级不由 Entity 本体维护，而由引擎内置的 Hierarchy / Transform 系统管理。

TransformComponent 负责表达空间数据：

```text
localPosition
localRotation
localScale
worldPosition / worldRotation / worldScale，可由运行时缓存
localMatrix / worldMatrix，可由运行时缓存
dirtyFlag
```

Hierarchy System 负责表达场景树关系：

```text
parent
children
siblingOrder
sceneRoot
addChild / removeChild / moveEntity
destroySubtree
```

实现上可以把 Hierarchy Storage、Transform Storage、Dirty Queue 和 World Matrix Cache 拆开优化。  
但对用户和 AI 来说，它表现为：

```text
Entity 有 Transform
Entity 可以挂到另一个 Entity 下面
子 Entity 的世界坐标受父 Entity 影响
```

Scene Entity 必须有 Transform 的原因：

```text
Scene 显示需要 Transform
Prefab 展开需要 Transform
渲染 / 物理 / 动画 / 音频空间化需要 Transform
编辑器选择、拖拽、对齐和层级组织需要 Transform
```

纯逻辑 Entity 不强制 Transform。  
例如：

```text
BattleRuleController
RoundState
ShopState
QuestTracker
InventoryContainer
NetworkSession
```

这些对象可以只是数据容器或逻辑状态，不需要位置、旋转、缩放。

Entity 不负责维护：

```text
自己引用了哪些 Asset
哪些 Asset 引用了自己
自己引用了哪些 Entity
哪些 Entity 引用了自己
自己被哪些 System 使用
自己属于哪些玩法规则
```

这些关系分别由以下系统管理：

```text
资源引用 -> Component AssetRef + Asset Graph
运行时 Entity 引用 -> Component EntityRef + Runtime Reference Index
玩法语义 -> Schema / DSL / Blueprint
系统调度 -> ECS Scheduler / System Graph
```

最终原则：

```text
Entity 保持轻量。
Transform 是 Scene Entity 的内置必备组件。
Hierarchy / Transform 是引擎底层能力。
项目语义不进入 Entity 本体。
```

## Component 纯数据规则

正式规则：

```text
Component 是 Schema 定义的纯数据。
Component 不允许包含项目业务逻辑。
Component 不允许定义 Start / Update / OnDamage 等行为方法。
所有行为必须进入 System。
项目行为进入 Blueprint / DSL / IR Rule。
引擎行为进入 Native System。
```

Component 只描述状态、配置和引用。  
例如：

```text
HealthComponent
  hp
  maxHp
  shield
  invincible

SpriteRendererComponent
  sprite: AssetRef
  color
  layer
  visible

AudioSourceComponent
  clip: AssetRef
  volume
  loop
  spatial
```

不允许：

```text
HealthComponent.takeDamage(amount)
HealthComponent.onDeath()
SpriteRendererComponent.render()
AudioSourceComponent.play()
```

对应行为应该进入：

```text
DamageSystem / CombatRule IR
DeathSystem / RewardRule IR
RenderSystem
AudioSystem
```

Component 可以分为：

```text
Data Component：项目数据，例如 Health / Inventory / Team / SkillState
Tag Component：无字段标记，例如 PlayerTag / DeadTag / BossTag
Reference Component：保存 AssetRef / EntityRef，例如 Target / Owner / SpriteRenderer
Built-in Component：引擎内置组件，例如 Transform / Camera / SpriteRenderer / AudioSource
Runtime-only Component：运行时缓存，例如 WorldMatrixCache / RenderHandle
BehaviorBinding Component：把 Entity 绑定到 DSL / IR 行为规则
```

有些 Component 会触发可见效果，但它们仍然是数据组件。  
例如 `SpriteRendererComponent` 只是描述要显示哪张图，真正渲染由 `RenderSystem` 执行；`AudioSourceComponent` 只是描述要播放哪个音频，真正播放由 `AudioSystem` 执行。

采用纯数据 Component 的原因：

```text
Schema 可验证
Inspector 可稳定展示
AI 修改更安全
Diff 和 Patch Plan 更清晰
IR 读写边界清楚
ECS 存储更高效
Bug 可以追踪到具体 System / DSL Rule
```

最终原则：

```text
Component 描述“是什么状态”。
System 执行“发生什么行为”。
AI 默认生成和修改 Component Schema、Feature Folder 内的 Project Assets、Gameplay Rule Asset / RuleSlot，而不是把逻辑塞进 Component。
```

其中 Project Rule System 的运行方式以逻辑系统文档为准：

```text
Project Rule System = Rust Project Module / Rust Framework + Contract-bound RuleSlot
```

发布时可以把受限 RuleSlot 的 Canonical Rule IR 编译成 Rust AOT；解释执行只作为受限验证、诊断或未来热更覆盖路径，不作为默认项目逻辑主线。

## ECS 与上层项目模型映射

正式规则：

```text
E 来自 Scene / Prefab / Runtime Spawn Request。
C 来自 Component Schema + Component Data。
S 来自 Native Module + System / Domain Blueprint + IR Rule。
```

底层 Rust ECS 的 Entity / Component / System 不是用户直接手写的底层结构，而是由上层项目模型声明后落地生成或注册。

### Entity 映射

底层：

```text
ECS Entity = Rust Runtime 中的运行时对象 ID
```

上层来源：

```text
Scene Entity
Prefab Entity / Prefab Instance
Runtime Spawn Request
```

Scene / Prefab 中声明的 Entity 在运行时被 Spawn 成 Rust ECS Entity。  
Entity 本体保持轻量，复杂关系由 Component、Hierarchy / Transform、Asset Graph 和 Runtime Reference Index 管理。

## RuntimeSpawnRequest 结构

RuntimeSpawnRequest 是运行时生成对象的领域请求。  
它不是 DSL，不是用户手写 API，也不是统一 RuntimeCommand。

典型来源：

```text
Project Rule / State Rule
技能规则
子弹 / 掉落物 / 临时特效 / 召唤物
测试场景
```

最小结构：

```text
RuntimeSpawnRequest:
  requestId
  source
  prefab
  owner
  parent
  transform
  componentOverrides
  spawnMode
  lifetime
  failurePolicy
  diagnostics
```

字段含义：

```text
source:
  发起来源，例如 ruleId / skillId / systemId / aiPatchId。

prefab:
  要生成的 Prefab / Entity Template / Runtime Template。

owner:
  SceneOwned(sceneInstanceId) / RuntimeOwned(ownerSceneInstanceId) / Persistent。
  默认不填时归属当前 active SceneInstance。

parent:
  可选 parent Entity。
  如果指定 parent，默认继承 parent 的 EntityOwner。

transform:
  localPosition / localRotation / localScale。
  如果没有 parent，可按 world transform 写入。

componentOverrides:
  只允许覆盖 prefab 暴露的 Component 初始字段。
  不允许临时塞未知 Component。

spawnMode:
  immediate / deferred。
  第一版默认 deferred，在 Runtime 安全 apply point 执行。

lifetime:
  untilDestroyed / sceneUnload / duration。
  第一版只要求 untilDestroyed 和 sceneUnload。

failurePolicy:
  failRequest / skip / fallbackPrefab。

diagnostics:
  trace_id / reason / sourceMap / errorCode。
```

第一版支持：

```text
Prefab spawn。
parent。
ownerSceneInstanceId。
transform。
componentOverrides。
deferred apply。
diagnostics。
```

第一版不支持：

```text
任意脚本回调。
复杂构造流程。
绕过 Prefab 直接拼任意 Component 图。
AI 默认创建 Persistent。
跨线程立即写 ECS。
```

执行结果：

```text
RuntimeSpawnRequest
  -> Runtime Spawn System
  -> 分配 runtimeEntityId
  -> 写 Transform
  -> 写 Component Data
  -> 建立 Hierarchy
  -> 写 EntityOwner
  -> 发 EntitySpawned
```

## RuntimeDespawnRequest 结构

RuntimeDespawnRequest 是引擎层销毁对象的领域请求。  
它只定义 Runtime 如何安全销毁 Entity，不定义项目侧什么时候销毁。

最小结构：

```text
RuntimeDespawnRequest:
  requestId
  target
  mode
  reason
  source
  diagnostics
```

字段含义：

```text
target:
  runtimeEntityId
  或 SceneInstanceId
  或 RuntimeSpawnInstanceId

mode:
  entity_only
  with_children
  scene_owned_group

reason:
  explicit_destroy
  scene_unload
  runtime_shutdown
  replace_instance
  invalid_spawn_cleanup

source:
  systemId / sceneLifecyclePlanId / runtimeSystem / trace source

diagnostics:
  trace_id / sourceEntityId / sceneInstanceId / errorCode
```

执行流程：

```text
RuntimeDespawnRequest
  -> Despawn Queue
  -> Runtime Despawn System
  -> 检查 Entity 是否存在
  -> 检查是否已 pending_despawn
  -> 标记 pending_despawn
  -> 根据 mode 收集 Entity 集合
  -> 发 EntityDespawning
  -> 移除 Hierarchy / Transform / Component
  -> 清理 EntityRef / sourceEntityId -> runtimeEntityId 映射
  -> 发 EntityDespawned
  -> 写 RuntimeTrace / Diagnostics
```

引擎层规则：

```text
默认 deferred despawn。
不支持项目侧立即删除 ECS Entity。
重复 despawn 不报 fatal，记录 warning / ignored。
销毁不存在 Entity 不崩溃，返回 not_found diagnostics。
Scene unload 批量销毁走同一套 Runtime Despawn System。
Persistent Entity 只有 explicit_destroy / runtime_shutdown 可以销毁。
资源 release 不在 Runtime Despawn System 里做。
```

### Component 映射

底层：

```text
ECS Component = Rust Runtime 中挂在 Entity 上的运行时数据
```

上层来源：

```text
Component Schema = Component 类型定义
Scene / Prefab Component Data = Component 初始值
Runtime Component Data = 运行时产生或修改的数据
Built-in Component = 引擎内置组件
```

Schema 定义字段和类型，Rust Runtime 注册或生成对应 Component Storage。  
Scene / Prefab / Spawn Request 提供初始 Component Data。

### System 映射

底层：

```text
ECS System = Rust Runtime 中被调度执行的系统
```

上层来源分两类：

```text
Native System:
  来自 Engine Core / Native Module
  由 Rust 实现
  例如 TransformSystem / RenderSystem / PhysicsSystem / AudioSystem

Project Rule System:
  来自 System / Domain Blueprint
  由 Rust Domain Runtime 调度
  具体规则来自 IR Rule
  发布版可执行 Rust AOT
  热更 / 编辑器 / 验证可执行 IR Interpreter
```

Blueprint 定义系统结构、Command、Event、Rule Slot、读写边界和集成契约。  
IR Rule 定义具体规则如何计算。  
Rust Runtime 负责查询 Component、构造规则输入、执行规则、验证输出、写回 ECS、发出 Event / Command。

### 总结

```text
上层 Scene / Prefab 生成 Entity。
上层 Schema / Component Data 生成 Component。
上层 Native Module / Blueprint / IR Rule 生成或驱动 System。
```

## Project Library 拖拽资源规则

正式规则：

```text
用户从 Project Library 拖资源到 Hierarchy / Scene 时，不是把资源本身放进 Scene。
引擎会创建一个 Entity，并把资源通过 Component / AssetRef 挂到 Entity 上。
```

也就是说：

```text
资源仍然属于 Project Library
Scene 只保存 Entity / Component / AssetRef / Prefab Instance / Overrides
```

这对齐 Unity 的使用体验：

```text
拖 Sprite -> 创建 Entity + SpriteRenderer
拖 Model -> 创建 Entity + MeshRenderer / Animator
拖 Audio -> 创建 Entity + AudioSource
拖 Prefab -> 创建 Prefab Instance
```

本引擎对应为：

```text
拖资源 -> 创建 Entity
资源 -> 挂到 Component 的 AssetRef 字段
```

## 示例

拖入 Sprite：

```text
enemy_medium.png
```

生成：

```text
Entity: enemy_medium
  Transform
  SpriteRenderer
    sprite = AssetRef(enemy_medium.png)
```

拖入 3D Model：

```text
boss_model.glb
```

生成：

```text
Entity: boss_model
  Transform
  MeshRenderer
    mesh = AssetRef(boss_model.mesh)
    materials = AssetRef(...)
  Animator optional
  Collider optional
```

拖入 Audio：

```text
explosion.wav
```

生成：

```text
Entity: explosion
  Transform
  AudioSource
    clip = AssetRef(explosion.wav)
```

拖入 Prefab：

```text
Enemy.prefab
```

生成：

```text
Prefab Instance:
  source = AssetRef(Enemy.prefab)
  overrides = {}
```

拖入 DSL / Behavior：

```text
enemy_ai.behavior
```

如果目标是 Entity：

```text
Entity:
  BehaviorBinding
    behavior = AssetRef(enemy_ai.behavior)
```

## Entity Creation Resolver

拖入资源时由 Entity Creation Resolver 决定默认行为：

```text
Project Library Resource
  -> Drag into Hierarchy / Scene
  -> Entity Creation Resolver
  -> 根据资源类型选择默认 Component
  -> 创建 Entity 或修改目标 Entity
  -> 添加 Transform
  -> 添加对应 Renderer / Source / Binding Component
  -> 写入 AssetRef
  -> 更新 Scene
  -> 更新 Asset Graph
```

默认映射：

```text
Sprite / Texture:
  Entity + Transform + SpriteRenderer

3D Model:
  Entity + Transform + MeshRenderer + optional Animator + Collider

Audio:
  Entity + Transform + AudioSource

VFX:
  Entity + Transform + VfxEmitter

Material:
  如果拖到 Entity -> 替换 Renderer Material
  如果拖到空场景 -> 创建预览 Entity

Prefab:
  创建 Prefab Instance

Scene:
  打开 / Additive Load / Set Active Scene

DSL / Behavior:
  如果拖到 Entity -> 添加 BehaviorBinding Component

UI Asset:
  如果拖到 Canvas / UI Root -> 创建 UI Entity
```

## Scene 保存内容

Scene 不直接保存资源内容。

Scene 保存：

```text
scene_id
scene_settings
root_entities
entity_tree
entity_components
prefab_instances
asset_refs
overrides
```

资源内容仍然在：

```text
Project Library
```

引用关系由：

```text
Asset Graph
```

记录。

## Scene / Prefab 运行时实例化规则

Scene / Prefab 从 Runtime Package 进入运行时后，直接实例化到 Rust ECS。

正式规则：

```text
Scene / Prefab Runtime 实例化直接进入 Rust ECS。
不创建 GameObject / Actor 中间层。
SceneInstantiator 负责 EntityId 分配、Component 写入、Hierarchy 建立、Prefab 展开、Override 应用、EntityRef 修复。
Transform 是每个 Scene / Prefab Entity 的必备 Component。
Prefab 是 Entity 模板，不是 Runtime 特殊对象。
Scene 实例化不强制同步加载所有资源，只校验 AssetRef 可解析。
资源 preload / release 时机由项目侧 Loading Rule 控制。
RenderExtract 后续从 ECS 读取 render-facing components，生成 RenderCommand。
```

SceneInstantiator 最小流程：

```text
读取 Runtime Package manifest
查找 scene_table 中的目标 Scene
校验 scene schemaVersion
创建 SceneInstanceId
为每个 source entity 分配 runtime EntityId
建立 sourceEntityId -> runtimeEntityId 映射
写入必备 Transform Component
写入普通 Component Data
建立 Parent / Children 层级关系
解析 Prefab Instance
应用 Prefab Overrides
修复 EntityRef / AssetRef
提交到 ECS World
发出 SceneLoaded / SceneActivated 事件
```

运行时 ID 规则：

```text
sourceEntityId = 编辑器 / 文档中的稳定 Entity ID。
runtimeEntityId = Rust ECS 中的运行时 Entity ID。
二者不能混用。
```

Runtime EntityRef / Handle 规则：

```text
runtimeEntityId 必须是 index + generation。
RuntimeEntityHandle 是运行时保存 Entity 引用的安全形式。
RuntimeEntityHandle 至少包含 runtimeEntityId，可附带 sceneInstanceId / sourceEntityId / issuedFrame / debugName。
Component 中保存运行时 Entity 引用时，保存 RuntimeEntityHandle，不保存裸 index。
Scene / Prefab / Runtime Package 中保存稳定 sourceEntityId，不保存 runtimeEntityId。
SceneInstantiator 加载时负责把 sourceEntityId 修复为 RuntimeEntityHandle。
```

Component Schema EntityRef 规则：

```text
Component Schema 必须显式标记 EntityRef 字段。
Scene / Prefab / Runtime Package 中的 EntityRef 字段保存 AuthoringEntityRef。
Runtime 加载后，SceneInstantiator 按 Component Schema 批量 fixup 为 RuntimeEntityHandle。
expected 只做组件存在性校验，不表达业务条件。
scope 只限定引用来源，不做复杂查询。
```

示例：

```text
ProjectileComponent.target:
  type = EntityRef
  required = false
  scope = runtime
  expected.anyOf = [Health]
```

禁止：

```text
Scene / Prefab 保存 RuntimeEntityHandle。
EntityRef 字段只靠 hierarchy path。
EntityRef 自动跨 Scene 搜索。
EntityRef 自动强引用保活 Entity。
EntityRef 自动重绑定旧引用。
```

Handle resolve 规则：

```text
Runtime 使用 Entity 前必须验证 index / generation / pending_despawn / sceneInstance 状态。
generation 不匹配代表旧引用，不能重绑定到新 Entity。
pending_despawn 的 Entity 不再允许作为新逻辑目标。
Scene unload 后，属于该 SceneInstance 的 RuntimeEntityHandle 统一失效。
```

失效结果必须可诊断：

```text
entity_not_found
generation_mismatch
pending_despawn
scene_unloaded
```

Despawn 后 Runtime 保留有界 tombstone diagnostics：

```text
runtimeEntityId
sourceEntityId
sceneInstanceId
despawnFrame
despawnReason
lastKnownName
```

`sourceEntityId -> runtimeEntityId` 映射用于：

```text
EntityRef 修复
Prefab Override 定位
Trace / Debug 回查
AI 定位 Bug
Scene Unload 时按 SceneInstance 回收 Entity
```

Prefab 实例化规则：

```text
Prefab 保存可复用 Entity Tree 和 Component 初始值。
Prefab Instance 保存 source AssetRef 和 overrides。
运行时实例化 Prefab 时，先展开 Prefab Entity Tree，再应用 overrides，再挂接到 Scene Hierarchy。
展开后的结果是普通 ECS Entity / Component，不保留特殊 Runtime Object。
```

资源规则：

```text
Scene 实例化阶段不负责把所有 AssetRef 同步加载为真实资源。
Scene 实例化阶段只校验 AssetRef 能通过 RuntimeAssetIndex 解析。
需要提前加载哪些资源、何时 release、跨场景是否常驻，由项目侧 Loading Rule / Scene Lifecycle 决定。
```

最小 diagnostics：

```json
{
  "sceneId": "main_scene",
  "stage": "instantiate_entity | apply_component | prefab_override | entity_ref | asset_ref",
  "entitySourceId": "enemy_001",
  "component": "SpriteRenderer",
  "field": "texture",
  "errorCode": "missing_asset_ref",
  "message": "AssetRef cannot be resolved by RuntimeAssetIndex.",
  "sourceMap": {
    "scene": "main.scene",
    "prefab": "enemy.prefab"
  }
}
```

## Scene 生命周期与 Entity Ownership

Scene 实例化后，Runtime 必须能知道每个 Entity 的所有权。  
这是 Scene unload、Additive Scene、运行时 Spawn、AI 查错和资源释放边界的基础。

正式规则：

```text
SceneInstanceId 表示一次 Scene 加载产生的运行时实例。
SceneInstantiator 创建 SceneInstanceId，并记录该实例生成的所有 runtimeEntityId。
每个 Runtime Entity 必须有 EntityOwner。
Scene unload 只销毁 Entity，不直接决定资源 release。
资源 release 仍由项目侧 Loading Rule / Scene Lifecycle 决定。
```

EntityOwner 最小分类：

```text
SceneOwned(sceneInstanceId):
  由 Scene / Prefab 实例化产生。
  随所属 SceneInstance unload 一起销毁。

RuntimeOwned(ownerSceneInstanceId optional):
  运行时 Spawn 产生。
  默认归属当前 active SceneInstance。
  如果 Spawn 指定 parent，则继承 parent 的 EntityOwner。
  如果 ownerSceneInstanceId 存在，所属 SceneInstance unload 时一起销毁。

Persistent:
  跨 Scene 常驻。
  不随普通 Scene unload 销毁。
  必须显式声明，普通 AI 生成对象默认不能创建 Persistent。
  必须由项目侧显式 destroy，或由 Runtime shutdown 统一清理。
  AI 如果认为需要 Persistent，必须生成 reason，并进入验证 / 询问流程。
```

Runtime Spawn 归属规则：

```text
默认 Runtime Spawn -> 当前 active SceneInstance。
指定 parent -> 继承 parent 的 EntityOwner。
指定 ownerSceneInstanceId -> 归属指定 SceneInstance。
跨场景常驻 -> 必须显式声明 Persistent，并通过 Validation。
```

Scene 生命周期事件最小集合：

```text
SceneLoadRequested
SceneLoaded
SceneActivated
SceneUnloadRequested
SceneDeactivated
SceneUnloaded
EntitySpawned
EntityDespawned
```

Scene unload 最小流程：

```text
SceneUnloadRequested
  -> 标记 SceneInstance unloading
  -> 停止该 SceneInstance 新规则写入
  -> 发 SceneDeactivated
  -> 收集 SceneOwned Entity
  -> 收集 ownerSceneInstanceId 指向该 Scene 的 RuntimeOwned Entity
  -> 发 EntityDespawned
  -> 从 ECS 删除 Entity / Component
  -> 清理 sourceEntityId -> runtimeEntityId 映射
  -> 写入 RuntimeEntity tombstone diagnostics
  -> 发 SceneUnloaded
  -> 通知 Loading Rule 可执行资源 release 策略
```

Additive Scene 规则：

```text
同一个 Runtime World 可以同时存在多个 SceneInstance。
每个 SceneInstance 拥有独立 SceneInstanceId。
Active Scene 只决定默认 Runtime Spawn 归属，不代表只有它会 Tick / Render。
跨 Scene EntityRef 必须显式声明，不能靠运行时临时 EntityId 隐式引用。
```

## SceneLifecyclePlan 结构

SceneLifecyclePlan 是 Project Rule / State Rule 生成的结构化 Runtime 请求。  
它不是通用脚本，不是小型编程语言，也不是新的规则系统。

设计原则：

```text
Load 和 Activate 分开。
Unload 和 Release 分开。
Scene 生命周期只编排场景实例，不直接实现资源加载底层能力。
复杂条件和项目玩法逻辑仍由 Project Rule / State Rule 表达。
SceneLifecyclePlan 只描述“请求哪些生命周期动作”。
触发和复杂条件属于 Project Rule / State Rule。
Validation 只做结构和引用检查。
Runtime 只做状态保护和 diagnostics。
```

最小结构：

```text
SceneLifecyclePlan:
  id
  preload
  load_scene
  activate
  unload
  release
  fallback
  diagnostics
```

字段含义：

```text
preload:
  需要提前加载的 AssetSet / AssetRef。

load_scene:
  要加载的 SceneRef。
  加载模式为 single / additive。
  只表示加载请求，不表示立刻激活。

activate:
  是否设为 active Scene。
  是否等待资源加载完成。
  是否允许加载完成后延迟激活。

unload:
  要卸载的 SceneInstance / SceneRef。
  卸载只销毁 Entity / Component。

release:
  要释放的 AssetSet / handle / scope。
  release 由 AssetRuntime 执行。

fallback:
  加载失败、资源缺失、激活失败时的降级策略。

diagnostics:
  AI / Trace / Report 需要记录的 planId、sceneId、assetSet、reason、errorCode。
```

示例：

```yaml
id: enter_battle_scene
preload:
  assetSets:
    - battle_core
load_scene:
  scene: battle_scene
  mode: single
activate:
  setActive: true
  waitForPreload: true
  allowDelayedActivation: true
unload:
  scenes:
    - main_menu
release:
  assetSets:
    - main_menu_only
fallback:
  onLoadFailed: return_previous_scene
diagnostics:
  trace: true
```

Project Rule / State Rule 负责决定什么时候提交该 Plan：

```text
enter_state(Battle)
  -> submit SceneLifecyclePlan(enter_battle_scene)
```

第一版不允许：

```text
任意循环。
任意脚本回调。
逐 Entity 操作。
直接调用底层 IO / decode / GPU upload。
直接创建 Persistent Entity。
```

校验边界：

```text
不建立 SceneLifecyclePlan 专用 Validation DSL。
只使用现有 Validation 做结构和引用检查：
  scene 是否存在
  AssetSet / AssetRef 是否存在
  load mode 是否合法
  release scope 是否可解析
  fallback 是否可识别
```

运行时保护：

```text
scene 未加载不能 activate。
scene 正在 unload 不能重复 unload。
资源仍被引用时 release 记录 retained，不强行释放。
Persistent Entity 持有资源时 release 失败或降级为 retained。
```

## AI 修改规则

AI 修改 Scene / Entity 时，也必须遵守同样模型。

参考 Bevy Scene / Reflect / patch 思路，AI 修改 Scene 的默认单位应该是字段级 patch，而不是整文件重写：

```text
修改目标必须有稳定 entityId / component / field path。
Prefab Instance 默认保留 source，只写 overrides。
引用其它 Entity 时使用稳定 EntityRef / scene-local name，不依赖运行时临时 Entity index。
Scene patch 必须能被 Validation 单独审查和回滚。
不采用 Bevy BSN / macro 风格作为项目源数据，因为它偏程序员表达，不适合 AI 和普通用户长期维护。
```

用户说：

```text
把这个敌人的图换成更有压迫感的新版本
```

AI 应该：

```text
生成新 sprite
更新 SpriteRenderer.sprite AssetRef
保留 Entity / Transform / Collider / Behavior
刷新 Asset Graph
```

不应该：

```text
重建整个 Entity
把图片嵌进 Scene
破坏 Prefab Instance 关系
```

用户说：

```text
给这个敌人加爆炸音效
```

AI 应该：

```text
添加 AudioSource Component
clip = AssetRef(explosion.wav)
设置 play_on_death binding
```

## 最终规则

```text
Scene 是 Entity 树。
资源不直接成为 Scene 内容。
资源通过 Component 上的 AssetRef 被 Entity 使用。
从 Project Library 拖资源到 Hierarchy / Scene，会创建 Entity 或修改目标 Entity。
拖入逻辑由 Entity Creation Resolver 根据资源类型决定。
Scene 保存 Entity / Component / AssetRef / Prefab Instance / Overrides。
资源内容仍然由 Project Library 管理。
引用关系由 Asset Graph 管理。
```
