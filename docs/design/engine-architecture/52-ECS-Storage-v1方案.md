# ECS Storage v1 方案

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

本文档定义 Rust Native Runtime 第一版 ECS Storage 的正式方案。

当前结论已经更新：

```text
第一版 ECS Storage 不再采用“每种 Component 一个简单 Dense Vec，后续再升级 Archetype”的路线。
第一版底层就按最小 Archetype Table 架构设计。
SparseSet 作为 Tag / 稀疏组件 / 高频增删组件的辅助存储。
AI 和项目层不直接面对 Archetype / Table / SparseSet。
```

它只讨论 ECS 的数据存储地基，不讨论完整 System Scheduler、IR 执行、物理、渲染后端或编辑器 UI。  
System 调度、Deferred Command、Change Detection、RenderExtract 会在后续文档继续细化。

## 1. ECS Storage v1 是什么

ECS Storage v1 是 Runtime 内部保存游戏世界数据的第一版底层结构。

通俗说：

```text
Entity = 一个运行时对象的稳定身份
Component = 这个对象身上的纯数据
Storage = 引擎把这些 Component 存在哪里、怎么查、怎么改、怎么删
World = Entity 和 Component Storage 组成的运行时世界
```

例如一个敌人：

```text
Entity: enemy_001
Component:
  Transform
  Health
  Renderable
  EnemyTag
```

ECS Storage v1 要解决的是：

```text
enemy_001 是否还活着？
enemy_001 有哪些 Component？
Health 数据存在什么地方？
系统怎么快速找到所有有 Health 的 Entity？
修改 Health 时怎么记录 trace / dirty？
删除 Entity 时怎么安全移除所有 Component？
添加 / 删除 Component 时 Entity 怎么在底层存储中移动？
```

它不是用户直接面对的编辑器模型。  
用户和 AI 仍然面对 Scene / Entity / Component Schema / Rule / Patch Plan。

## 2. 设计目标

优先级：

```text
1. AI 友好
2. 适配复杂项目
3. 后期可维护、可修改
4. 高效率
5. 规则尽量简单
```

第一版目标不是追求极限性能，而是建立长期不会推翻的底层边界：

```text
World / Entity / Component / Query 概念稳定。
底层从第一版开始就是 Archetype / Table-ready。
Schema 到 Runtime Component 的映射稳定。
写入入口统一。
Trace / Dirty / FrameHash 可接入。
后续可以优化 chunk、bitset、query cache，但不改上层模型。
```

## 3. Storage / Table / Archetype 的区别

```text
Storage = Component 数据存储系统的总称。
Table = 按列连续保存一组 Entity 的 Component 数据。
Archetype = 按 Entity 拥有哪些 Component 进行分组。
```

关系：

```text
ECS Storage
  -> Archetype: Transform + Health
      -> Table: Entity[] / Transform[] / Health[]
  -> Archetype: Transform + Health + Renderable
      -> Table: Entity[] / Transform[] / Health[] / Renderable[]
  -> SparseSet: Tag / 稀疏组件 / 高频增删组件
```

Archetype 负责回答：

```text
哪些 Entity 拥有同一组 Component？
```

Table 负责回答：

```text
这些 Entity 的 Component 数据如何连续存放？
```

Storage 是整套系统。

## 4. 市场方案参考

### Unity GameObject

Unity 传统主线是 GameObject + Component。它对用户非常友好，但底层不是现代高性能 ECS 存储模型。

可借鉴：

```text
Entity / Component 心智简单。
拖资源到场景生成对象的工作流成熟。
Inspector 对用户友好。
```

不照搬：

```text
不以对象指针和脚本组件作为 Runtime 数据底层。
不把用户脚本对象作为高性能数据查询核心。
```

### Unity DOTS / Entities

Unity DOTS 使用 archetype / chunk 路线。同一组 Component 的 Entity 会进入相同 archetype，并按 chunk 批量存储。

可借鉴：

```text
同类 Component 连续存储。
批量查询和并行执行。
数据布局影响性能。
Component 组合变化会导致 Entity 迁移到新 archetype。
```

不照搬：

```text
第一版不暴露 DOTS 式复杂用户心智。
第一版不做完整 chunk allocator / Burst / Jobs 体系。
```

### Unreal Actor / Component

UE 普通 Gameplay 主线是 Actor / Component / UObject，不是纯 ECS。

可借鉴：

```text
稳定对象身份。
编辑器和反射能力强。
组件化工作流成熟。
```

不照搬：

```text
不引入 UObject 式庞大对象体系。
不把 Actor 生命周期作为 AI-first Runtime 的底层真相。
```

### Unreal MassEntity

MassEntity 使用 Entity / Fragment / Archetype / Chunk 思路，适合 crowd、traffic、大规模模拟。

可借鉴：

```text
Fragment 类似 Component 数据。
Archetype / Chunk 适合大规模批量处理。
Processor 类似系统执行。
```

不照搬：

```text
不照搬 UE Mass 的完整框架和工具链依赖。
不让普通项目用户直接面对 Mass 风格底层概念。
```

### Bevy ECS

Bevy ECS 是 Rust 生态中最接近本项目的参考。它支持 table storage、sparse set storage、archetype、change detection、schedule executor。

可借鉴：

```text
Entity 带 generation，避免旧 id 误用。
Component 可选择 Table 或 SparseSet storage。
Archetype / Table 支持高效多组件查询。
Query access 用于读写冲突分析。
Change tick 用于变化检测。
Deferred Commands 用于结构变化安全提交。
```

不照搬：

```text
不暴露 bevy_ecs API 给项目层。
不要求 AI 生成 Bevy system。
不把 SystemSet / before / after / ambiguous_with 变成用户主要心智。
```

### Flecs

Flecs 是成熟 ECS 框架，强调 cache-friendly archetype / SoA 存储。

可借鉴：

```text
Archetype / SoA 是成熟 ECS 的常见路线。
查询和数据布局绑定很深。
大规模 Entity 处理需要从底层存储就考虑性能。
```

不照搬：

```text
不直接引入 Flecs C API 或其用户模型。
不把完整 query DSL 暴露给 AI。
```

### Godot

Godot 是 Node / SceneTree 路线，不是 ECS。

可借鉴：

```text
Scene / Node 心智简单。
树状编辑器体验清晰。
```

不照搬：

```text
Runtime 底层不采用 NodeTree 作为高性能数据查询模型。
```

## 5. 推荐方案

第一版采用：

```text
Generational RuntimeEntityId
+ SourceEntityId 映射
+ ComponentTypeId / ComponentRegistry
+ ArchetypeId / ArchetypeSignature
+ ArchetypeTable
+ EntityLocation
+ SparseSet 辅助存储
+ World Write API
+ Dirty / Trace hook
```

含义：

```text
RuntimeEntityId 使用 index + generation，避免删除后旧 id 误指向新对象。
SourceEntityId 保留 Scene / Prefab / Runtime Package 中的稳定 id。
ComponentTypeId 来自 Built-in Registry / Component Schema。
ArchetypeSignature 是一组 ComponentTypeId 的稳定排序集合。
ArchetypeTable 按列保存同一 Archetype 下的 Entity 和 Component 数据。
EntityLocation 记录 Entity 当前在哪个 ArchetypeTable 的哪一行。
SparseSet 用于 Tag / 稀疏组件 / 高频增删组件。
所有写入必须经过 World Write API，方便记录 dirty、trace 和 frame hash。
```

## 6. 核心数据结构

### RuntimeEntityId

```rust
struct RuntimeEntityId {
    index: u32,
    generation: u32,
}
```

规则：

```text
index 指向 entity slot。
generation 防止旧 RuntimeEntityId 复用后误命中。
RuntimeEntityId 是 Runtime 内部 id。
Scene / Prefab / Runtime Package 使用 SourceEntityId。
Trace / Report 必须能从 RuntimeEntityId 回查 SourceEntityId。
```

### RuntimeEntityHandle / EntityRef

RuntimeEntityId 只表达底层身份，RuntimeEntityHandle 表达“可以被跨系统保存和诊断的运行时引用”。

```rust
struct RuntimeEntityHandle {
    id: RuntimeEntityId,
    scene_instance_id: Option<SceneInstanceId>,
    source_id: Option<SourceEntityId>,
    issued_frame: Option<u64>,
    debug_name: Option<String>,
}
```

规则：

```text
World 内部查询可以直接使用 RuntimeEntityId。
事件、延迟请求、Component EntityRef、Trace、Diagnostics、AI Debug 默认使用 RuntimeEntityHandle。
RuntimeEntityHandle 访问 Entity 前必须 resolve。
Runtime Package 不保存 RuntimeEntityHandle，只保存 SourceEntityId。
SceneInstantiator / Runtime Spawn System 负责创建 RuntimeEntityHandle。
```

resolve 最小结果：

```text
Ok(EntityAccess)
Err(entity_not_found)
Err(generation_mismatch)
Err(pending_despawn)
Err(scene_unloaded)
```

不允许：

```text
用 index 单独作为 EntityRef。
旧 RuntimeEntityId 在 generation 不匹配时自动命中新 Entity。
项目规则绕过 World API 直接持有 Archetype row。
```

### SourceEntityId

```rust
struct SourceEntityId(String);
```

规则：

```text
SourceEntityId 来自 Runtime Package / Scene / Prefab。
SourceEntityId 用于 AI、Trace、Inspector、Golden Scenario。
RuntimeEntityId 用于底层执行。
两者必须建立双向映射。
```

### EntitySlot

```rust
struct EntitySlot {
    generation: u32,
    alive: bool,
    pending_despawn: bool,
    source_id: Option<SourceEntityId>,
    scene_instance_id: Option<SceneInstanceId>,
    location: Option<EntityLocation>,
}
```

EntitySlot 规则：

```text
alive=false 表示该 slot 当前没有有效 Entity。
pending_despawn=true 表示该 Entity 已进入销毁流程，不能作为新逻辑目标。
generation 每次 despawn / reuse 前后必须推进，防止旧引用误命中新对象。
source_id / scene_instance_id 用于 Trace / Diagnostics / AI Debug 回查。
```

### EntityLocation

```rust
struct EntityLocation {
    archetype_id: ArchetypeId,
    row: usize,
}
```

它用于快速找到 Entity 当前的表和行。

### ComponentTypeId

```rust
struct ComponentTypeId(String);
```

来源：

```text
Built-in Component:
  engine.transform
  engine.renderable
  engine.hierarchy
  engine.name

Project Component:
  project.health
  project.inventory
  project.equipment
```

规则：

```text
ComponentTypeId 必须稳定。
ComponentTypeId 来自 Component Schema / Built-in Registry。
不能来自 Rust TypeName 的临时字符串。
```

### ArchetypeSignature

```rust
struct ArchetypeSignature {
    component_types: Vec<ComponentTypeId>,
}
```

规则：

```text
component_types 必须稳定排序。
Transform + Health 与 Health + Transform 必须得到同一个 signature。
Tag / SparseSet-only Component 不一定进入 Table signature。
```

### ArchetypeTable

推荐结构：

```rust
struct ArchetypeTable {
    id: ArchetypeId,
    signature: ArchetypeSignature,
    entities: Vec<RuntimeEntityId>,
    columns: HashMap<ComponentTypeId, ComponentColumn>,
}
```

ComponentColumn 第一版可以是显式枚举：

```rust
enum ComponentColumn {
    Transform(Vec<Transform>),
    Renderable(Vec<Renderable>),
    Hierarchy(Vec<Hierarchy>),
    Name(Vec<Name>),
    Dynamic(Vec<RuntimeValue>),
}
```

规则：

```text
同一个 ArchetypeTable 中，每个 column 的长度必须等于 entities.len()。
row 是 Entity 在该 table 内的行号。
查询 Transform + Renderable 时，只扫描包含这两个 ComponentTypeId 的 ArchetypeTable。
```

### SparseSet

SparseSet 用于：

```text
Tag Component。
很少出现的 Component。
频繁添加 / 删除、不适合频繁迁移 Archetype 的 Component。
Runtime-only marker。
```

第一版可以先只设计接口，不急着实现复杂 SparseSet 优化。  
但 ComponentRegistry 必须能标记 storage_kind：

```text
Table
SparseSet
```

## 7. Add / Remove Component 如何执行

### AddComponent

```text
1. 找到 Entity 当前 EntityLocation。
2. 读取当前 ArchetypeSignature。
3. 加入新的 ComponentTypeId，得到目标 ArchetypeSignature。
4. 找到或创建目标 ArchetypeTable。
5. 把共有 Component 从旧 row 复制 / move 到新 table。
6. 写入新 Component。
7. 从旧 table swap_remove 旧 row。
8. 修正被 swap entity 的 EntityLocation。
9. 更新当前 Entity 的 EntityLocation。
10. 记录 Trace / Dirty / write_seq。
```

### RemoveComponent

```text
1. 找到 Entity 当前 EntityLocation。
2. 从当前 ArchetypeSignature 移除 ComponentTypeId。
3. 找到或创建目标 ArchetypeTable。
4. 把剩余 Component move 到新 table。
5. 从旧 table swap_remove 旧 row。
6. 修正被 swap entity 的 EntityLocation。
7. 更新当前 Entity 的 EntityLocation。
8. 记录 Trace / Dirty / write_seq。
```

这一步比简单 Storage 复杂，但它是长期性能和查询模型的核心。  
第一版必须把流程跑通，后续再优化复制成本。

## 8. Built-in Component 与 Project Component

第一版必须支持两类 Component：

```text
Built-in Component
Project Component
```

Built-in Component 由引擎提供 Rust 类型：

```text
Transform
Hierarchy
Renderable
Name
```

Project Component 由项目 Schema 定义：

```text
Health
Inventory
Equipment
SkillState
QuestState
```

规则：

```text
Built-in Component 可以使用强类型 Rust struct。
Project Component 第一版可以使用 RuntimeValue / SchemaValue 存储在 Dynamic column。
高频 Project Component 后续可由 Rust AOT 生成 typed column。
外层 World API 不因内部 typed / dynamic 差异改变。
```

这样做的原因：

```text
第一版保证 AI / Schema / Runtime Package 能跑通。
避免一开始就要求所有项目组件都完成 Rust codegen。
为后续性能优化保留通道。
```

## 9. Query v1

第一版只支持最小查询：

```text
query one component
query two components
query entity by id
query children / hierarchy
query changed component
```

示例：

```text
Query<Transform>
Query<Transform, Renderable>
Query<Health>
```

执行方式：

```text
根据 Query 的 ComponentTypeId 集合筛选匹配 ArchetypeTable。
在匹配 table 中按 row 连续遍历。
从对应 columns 取出 Component 数据。
```

不做：

```text
复杂 filter DSL。
任意 join optimizer。
用户自写 Rust query。
跨线程裸引用泄露。
完整 query cache。
```

正式规则：

```text
System 不能直接拿到 ArchetypeTable 的可变裸引用。
System 通过 World Query / Write API 访问数据。
Query 的 reads / writes 必须可被调度器记录。
```

## 10. 写入入口

所有 Component 写入必须经过统一入口：

```text
World::write_component(entity, component_type, patch)
World::write_field(entity, component_type, field_path, value)
World::add_component(entity, component_type, value)
World::remove_component(entity, component_type)
```

原因：

```text
统一记录 RuntimeTrace。
统一标记 dirty。
统一更新 FrameHash 输入。
统一做 Schema validate。
统一做读写安全检查。
统一维护 EntityLocation / ArchetypeTable row。
```

禁止：

```text
项目规则直接改 ArchetypeTable.columns。
项目规则直接改 ComponentColumn Vec。
AI 生成绕过 World API 的 Rust 代码。
RenderExtract 直接修改 Gameplay ECS。
Editor 直接修改 Runtime World。
```

## 11. Dirty / Change Detection v1

第一版使用简单 ChangeVersion：

```rust
struct ComponentChange {
    entity: RuntimeEntityId,
    source_entity: Option<SourceEntityId>,
    component_type: ComponentTypeId,
    field_path: Option<String>,
    frame: u64,
    write_seq: u64,
}
```

规则：

```text
每次写入产生 write_seq。
每帧可以按 component_type 查询 changed entities。
Render-facing Component 的变化进入 RenderDirtyTracker。
Inspector 可以读取 dirty 摘要。
RenderFrameReport 可以读取变化摘要。
```

第一版不做完整 Bevy change tick 兼容，只学习思想。  
后续如果需要更高性能，可以把 ChangeVersion 替换成 tick / bitset / sparse changed list。

## 12. Scene / Runtime Package 加载

加载流程：

```text
Runtime Package Scene
  -> allocate RuntimeEntityId
  -> 建立 SourceEntityId 到 RuntimeEntityId 映射
  -> 收集 Transform / Hierarchy / Built-in Component
  -> 根据 Component Schema 收集 Project Component
  -> 计算 ArchetypeSignature
  -> 插入对应 ArchetypeTable
  -> 验证 AssetRef / EntityRef
  -> 输出 LoadTrace
```

规则：

```text
Runtime Package 不直接保存 RuntimeEntityId(index,generation)。
Runtime Package 保存稳定 SourceEntityId。
Runtime 加载时分配 RuntimeEntityId。
Trace / Report 必须能从 RuntimeEntityId 回查 SourceEntityId。
```

## 13. 删除与复用

Despawn 规则：

```text
EntitySlot.pending_despawn = true。
EntitySlot.alive = false。
generation 增加。
根据 EntityLocation 从 ArchetypeTable swap_remove 当前 row。
修正被 swap entity 的 EntityLocation。
清理 SourceEntityId 映射。
从 parent children 中移除。
子 Entity 的处理策略由命令决定：recursive despawn 或 detach。
产生 Trace / Dirty。
写入有界 tombstone diagnostics。
```

禁止：

```text
立即复用同一 generation。
删除 Entity 后让旧 RuntimeEntityId 继续查到新对象。
删除 Entity 时留下 ArchetypeTable 悬挂 row。
```

tombstone diagnostics 最小字段：

```text
runtimeEntityId
sourceEntityId?
sceneInstanceId?
despawnFrame
despawnReason
lastKnownName?
```

这只用于 Debug / Trace / AI 查错，不参与正式逻辑，不做无限历史保存。

## 14. 多线程边界

Storage v1 必须支持未来并行调度，但第一版不暴露复杂锁模型。

正式规则：

```text
Runtime / Game Thread 是 World 结构变化 owner。
同 phase 内无冲突 System 可并行读写不同 ComponentTypeId。
结构变化进入 Deferred Commands，在安全点统一 apply。
worker_count=1 也必须走同一调度和写入路径。
```

第一版实现可以保守：

```text
先用调度器保证同一时间没有两个 writer 写同一 ComponentTypeId。
Storage 内部不靠全局大锁解决业务并发。
跨线程不泄露长期可变引用。
```

## 15. AI 友好性

AI 不需要理解 ArchetypeTable、row migration、chunk、SparseSet 的实现细节。

AI 看到的是：

```text
Entity 有哪些 Component。
Component 字段是什么。
哪个规则改了哪个字段。
这次修改产生了哪些 Trace。
哪些 Render-facing 数据变 dirty。
哪个 Golden Scenario 失败。
```

因此 Storage v1 必须输出可解释信息：

```text
source entity id
runtime entity id
component type
field path
old value / new value in debug mode
write reason
rule id / system id
frame / write_seq
```

Release 可以裁剪 old/new 大对象，但不能破坏 Runtime 正确性。

## 16. 不进入 v1 的内容

```text
完整 chunk allocator。
完整 query optimizer。
完整 query cache。
完整 reflection editor。
用户手写 Rust ECS system API。
项目层直接访问 storage。
跨 world replication。
network rollback storage。
物理引擎集成。
GPU driven ECS。
```

这些不是不要，而是不放进 Storage v1。

## 17. 最小测试用例

Storage v1 必须至少有以下测试：

```text
spawn entity creates alive RuntimeEntityId
despawn invalidates old RuntimeEntityId generation
source entity id maps to runtime id and back
entity with Transform creates Transform archetype
entity with Transform + Renderable creates matching archetype
query Transform + Renderable scans matching archetype table
add Renderable migrates entity to new archetype
remove Renderable migrates entity to smaller archetype
swap_remove updates moved entity location
write Transform records ComponentChange
despawn removes table row and clears source mapping
worker_count=1 uses same write API
Render-facing write marks dirty summary
```

Golden Scenario：

```text
加载一个打飞机小场景。
玩家、子弹、敌机分别生成 RuntimeEntity。
子弹命中敌机时写 Health。
Health 写入产生 Trace。
Renderable / Transform 写入产生 RenderDirty。
FrameHash 稳定。
```

## 18. 正式结论

ECS Storage v1 采用：

```text
Generational RuntimeEntityId
SourceEntityId mapping
ComponentTypeId / ComponentRegistry
ArchetypeSignature
ArchetypeTable
EntityLocation
SparseSet auxiliary storage
World Write API
ChangeVersion / Dirty hook
```

这套方案的核心价值：

```text
底层方向从第一版开始对齐成熟 ECS 路线。
避免后期从简单 Dense Vec 迁移到 Archetype 时重做 Query / Dirty / Scheduler / RenderExtract。
外层概念稳定，AI 不面对底层 ECS API。
复杂项目需要的批量查询、trace、dirty、schema、render extract 都能接入。
第一版只做最小 Archetype Table，不做完整 chunk / optimizer，控制复杂度。
```
