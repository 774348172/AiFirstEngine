# Rust Runtime ECS 路线

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

本文档确认 Rust Native Runtime 的 ECS 实现路线。

核心结论：

```text
Rust Runtime ECS 走自研外壳。
第一版使用最小 Archetype Table Storage。
不直接采用 hecs。
不直接采用 bevy_ecs。
不照搬 Unity DOTS。
不照搬 Unreal MassEntity。
hecs / bevy_ecs / DOTS / Mass 只作为设计参考。
AI 和项目层不直接面对底层 ECS API。
第一版即按并行调度架构设计。
测试环境可使用 worker_count=1，但不能存在单线程专用 ECS 路径。
```

## 为什么需要确认 ECS 路线

Rust Runtime MVP 的核心是 ECS。

ECS 决定：

```text
Entity 怎么创建和销毁。
Component 怎么存储。
System 怎么查询和执行。
FrameLoop 怎么调度。
项目规则怎么读写数据。
AI 生成的规则怎么落地。
RuntimeTrace 和 FrameHash 怎么产生。
后续并行调度怎么扩展。
```

如果 ECS 路线过早绑定第三方 API，会导致：

```text
AI 被迫理解第三方 ECS API。
项目层代码依赖第三方 ECS 概念。
后续替换底层存储困难。
编辑器 Schema / Inspector / Trace 需要跟着第三方模型变化。
```

因此 ECS 必须先定义引擎自己的外壳和概念。

## 路线对比

### hecs

hecs 是 Rust 生态中的轻量 ECS library。

特点：

```text
小。
简单。
侵入低。
更像 ECS World / Storage 库。
不强制完整引擎调度框架。
```

优点：

```text
容易嵌入自研 runtime。
学习和替换成本较低。
不会强迫引擎接受完整框架。
```

缺点：

```text
调度系统需要自研。
并行策略需要自研。
反射 / Inspector / Schema 映射需要自研。
AI 可解释层需要自研。
```

结论：

```text
hecs 可以作为最小存储实现的参考。
当前不直接采用 hecs 作为正式依赖和项目 API。
```

### bevy_ecs

bevy_ecs 是 Bevy 引擎的 ECS crate，也可独立使用。

特点：

```text
功能完整。
系统调度成熟。
并行能力强。
生态活跃。
带有明显 Bevy 设计哲学。
```

优点：

```text
可以快速获得成熟 ECS 能力。
查询、资源、调度、change detection 等能力较完整。
Rust 生态资料较多。
```

缺点：

```text
容易把 Bevy 的 API 和世界观带进本引擎。
AI 可能被迫生成 bevy_ecs 风格代码。
后续替换底层会困难。
引擎自己的 Schema / IR / Command / Trace 边界容易被第三方模型污染。
```

结论：

```text
bevy_ecs 可以作为调度、查询、change detection 的设计参考。
当前不直接采用 bevy_ecs 作为正式依赖和项目 API。
```

Bevy 源码分析后的补充规则：

```text
Archetype / Table / SparseSet 的存储取舍可以参考。
Query access graph 用于判断 System 读写冲突。
Commands / Deferred apply 用于结构变化安全提交。
Change tick / changed filter 用于增量检测。
Schedule executor 用于无冲突 System 并行。
RenderApp / Extract 用于 Main World 到 Render World 的边界。
```

但本项目不能把 Bevy 的自由调度模型暴露给用户和 AI：

```text
不让用户直接面对 SystemSet / before / after / ambiguous_with。
不要求 AI 生成 Bevy 风格 Rust system 函数。
不把 Reflect 当成项目 Schema 真相。
不把 Bevy Plugin / App 模型变成本项目插件主线。
```

正式方向保持：

```text
外层是本项目自己的 Entity / Component / System / FrameLoop / Trace 概念。
底层可以参考 Bevy 的存储、调度、change detection 和 extract 实现。
AI 只面对 Schema / Rule / Patch / Report，不面对 bevy_ecs API。
```

### Unity DOTS / Entities

Unity DOTS / Entities 是高性能 data-oriented ECS 路线。

特点：

```text
面向高性能和大规模模拟。
结合 Burst / Jobs / Physics / Graphics。
适合大量实体和硬件受限平台。
```

优点：

```text
性能路线成熟。
数据导向明确。
大规模模拟能力强。
与 Unity 平台工具链结合深。
```

缺点：

```text
复杂。
学习成本高。
和传统 GameObject / MonoBehaviour 体系并存导致心智负担重。
对 AI 生成和普通用户并不天然友好。
```

结论：

```text
学习 DOTS 的数据导向、批处理、并行、缓存友好思想。
不照搬 DOTS 的用户暴露模型。
```

### Unreal MassEntity

Unreal 的主流玩法仍然是 Actor / Component / UObject / Gameplay Framework。

MassEntity 是 UE 的 data-oriented 大规模实体框架。

特点：

```text
Entity / Fragment / Processor。
适合 crowd、traffic、大规模模拟。
和 UE 自己的反射、Gameplay、工具链深度结合。
```

优点：

```text
大规模实体模拟能力强。
工程体系成熟。
和 UE 工具链、AI、Gameplay 系统结合。
```

缺点：

```text
不是 UE 普通玩法的唯一核心模型。
依赖 UE 自身庞大框架。
学习成本不低。
不适合直接照搬到 AI-first 引擎。
```

结论：

```text
学习 Mass 的 Fragment / Processor / 大规模实体分片思想。
不照搬 UE Mass 的框架和暴露方式。
```

## 我们的正式路线

采用：

```text
自研 ECS 外壳 + 最小 Archetype Table Storage
```

含义：

```text
Entity / Component / Query / Command / System / Schedule 是本引擎自己的概念。
AI 和项目层只面对 Schema / IR / Command / System Contract。
Rust Runtime 内部实现 ECS。
第一版底层直接采用最小 Archetype Table，而不是先做简单 Dense Vec / HashMap 再迁移。
SparseSet 只作为 Tag / 稀疏组件 / 高频增删组件的辅助存储。
底层后续可以优化 chunk / query cache / sparse storage，但外壳和上层规则不变。
```

## ECS 外壳必须稳定

对上层暴露的稳定概念：

```text
EntityId
ComponentTypeId
ComponentStorage
ArchetypeTable
EntityLocation
Query
CommandQueue
RuntimeSystem
Schedule
RuntimeTrace
FrameHash
```

AI 和项目层只允许通过这些稳定概念间接影响 ECS：

```text
Component Schema
Scene Entity Data
Prefab Data
Canonical Rule IR
System Contract
Command
Patch Plan
```

禁止：

```text
AI 生成 hecs API 代码。
AI 生成 bevy_ecs API 代码。
项目层直接依赖第三方 ECS crate。
第三方 ECS 类型进入 Project Schema。
第三方 ECS 类型进入 IR。
第三方 ECS 调度语义成为项目规则真相。
```

## 第一版 ECS MVP

第一版只做最小正式内核。

必须包含：

```text
EntityId
Entity allocator
Entity alive / dead 状态
RuntimeEntityId / SourceEntityId
ComponentTypeId / ComponentRegistry
ArchetypeSignature / ArchetypeTable
EntitySlot / EntityLocation
Transform component
Renderable component
Hierarchy parent / children
spawn command
destroy command
query by component type
PhaseScheduler fixed phases
RuntimeTrace
FrameHash
Golden Scenario Test
```

第一版不做：

```text
完整 TaskGraph
archetype 极致优化 / chunk optimizer
完整反射系统
插件 ECS API
用户手写 Rust System
完整编辑器 ECS Inspector
复杂物理集成
复杂资源实例化
```

## 第一版并行调度规则

第一版 ECS 不能再按单线程路线设计。

原因：

```text
单线程路线会反过来影响 ECS API、System Contract、Trace 和 FrameLoop。
复杂项目需要从一开始验证读写冲突、阶段同步和确定性输出。
AI 生成系统时需要依赖稳定的 reads / writes 契约，而不是后期补调度器。
```

第一版 Schedule 必须具备：

```text
system reads / writes 声明
读写冲突检测
按 phase 分批调度
同批无冲突 system 可并行
command queue flush point
trace begin / end
frame hash collection
deterministic merge order
```

业务顺序仍由项目层定义。

测试规则：

```text
Golden Scenario 可以 worker_count=1。
worker_count=1 必须走并行调度器的同一代码路径。
禁止为测试创建绕过调度器的单线程 ECS 执行器。
```

引擎只负责：

```text
读写安全
结构变化安全
命令提交安全
trace 可观察性
```

## ECS 与最小 Job System 的关系

Rust Runtime ECS 第一版必须接入 Minimal Job System，但不实现完整通用任务图。

对应关系：

```text
ECS Schedule
  -> PhaseScheduler 的 Runtime 阶段输入

RuntimeSystem
  -> Job 的可执行内容

reads / writes
  -> PhaseScheduler 判断同批 system 是否可并行的安全信息

CommandQueue flush point
  -> PhaseScheduler 的同步点

RuntimeTrace / FrameHash
  -> Job begin / end 与 phase begin / end 的可观察输出
```

正式规则：

```text
ECS 业务顺序不依赖隐式线程调度。
ECS 并行只解决读写安全和执行效率。
项目规则中的先后关系必须在 Project Rule / Schedule Contract 中显式表达。
AI 不生成底层线程任务，只生成项目阶段、规则和依赖声明。
```

第一版并行边界：

```text
同 phase 内 reads / writes 不冲突的 system 可以并行。
冲突 system 串行。
结构变化统一进入 CommandQueue，在 flush point 提交。
Render-facing Component 的 dirty 在 ECS Write API 中产生。
RenderExtract 由独立 RenderExtractScheduler 处理。
```

与 UE / Unity 的取舍：

```text
UE:
  有成熟 TaskGraph 和大量系统级并行点。
  优点是性能和工程能力强，缺点是内部复杂度很高。

Unity:
  用户侧有 PlayerLoop 和 Job System / DOTS。
  优点是用户心智相对清晰，缺点是传统 GameObject 与 DOTS 并存增加复杂度。

本项目:
  ECS 从第一版进入多线程架构。
  但只实现固定阶段内的最小并行调度。
  保持 AI 面向 Schema / Rule / Trace，而不是面向底层任务图。
```

## 后续性能演进路线

后续按阶段优化，不破坏外壳。

### 阶段 1：最小 Archetype Table

目标：

```text
正确性
可测试
AI 可解释
Golden Scenario 通过
EntityLocation 稳定
Query / Dirty / Trace 能接入同一套 World Write API
```

### 阶段 2：Query / Change Detection

目标：

```text
提高同类 Component 查询性能。
减少 cache miss。
为批量系统执行做准备。
只处理变化 Component。
支持 Inspector dirty state。
支持 RenderExtract 增量提取，并生成 RenderCommand / RenderFrameReport。
```

### 阶段 3：System Access Declaration

目标：

```text
每个 System 声明 reads / writes。
用于调度分析、Trace、AI 解释。
```

注意：

```text
reads / writes 是引擎安全和诊断信息。
不是项目业务正确性的唯一来源。
业务正确性仍由项目层规则和 Golden Scenario 验证。
```

### 阶段 4：Parallel Schedule

目标：

```text
根据 reads / writes 自动并行。
冲突系统串行。
结构变化延迟提交。
```

### 阶段 5：底层优化或借鉴

目标：

```text
在不改变 ECS 外壳的前提下，继续借鉴 hecs / bevy_ecs / DOTS / Mass 的实现思想。
必要时优化 chunk、query cache、sparse storage、change tick。
```

规则：

```text
上层 Schema / IR / Command / System Contract 不变。
AI 不感知底层替换。
项目数据不感知底层替换。
```

## 与 AI-first 的关系

本引擎的 ECS 不只追求底层跑分。

更重要的是：

```text
AI 能理解 ECS 结构。
AI 能生成 Component Schema。
AI 能生成 System Contract。
AI 能解释某个 System 为什么读写某些 Component。
AI 能通过 RuntimeTrace 和 Golden Scenario Test 找 Bug。
用户不需要直接面对复杂 ECS API。
```

因此 ECS 的上层模型必须比 hecs / bevy_ecs / DOTS / Mass 更适合 AI：

```text
Schema-first
Command-first
Trace-first
Validation-first
Golden Scenario-first
```

## 与项目规则边界

本文档只定义 ECS 实现路线。

业务规则边界继续遵守：

```text
16-ECS写入与项目规则边界.md
```

核心规则：

```text
ECS Runtime Framework = Engine Layer
Component Schema = Project Layer
Project System / Rule = Project Layer
业务顺序 = Project Layer
读写安全 = Engine Layer
并行调度安全 = Engine Layer
结构变化安全 = Engine Layer
业务正确性 = Project Layer
```

## 当前确认后的结论

```text
不直接用 hecs。
不直接用 bevy_ecs。
不照搬 Unity DOTS。
不照搬 Unreal Mass。
Rust Runtime ECS 走自研外壳。
第一版用最小 Archetype Table Storage。
第一版按并行调度架构设计。
AI 和项目层只能面对 Schema / IR / Command / System Contract。
吸收 hecs 的简洁、bevy_ecs 的调度、Unity DOTS 的数据导向、Unreal Mass 的大规模实体分片思想。
```
