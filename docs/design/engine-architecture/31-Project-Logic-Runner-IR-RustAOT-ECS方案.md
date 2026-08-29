# Project Logic Runner / IR / Rust AOT / ECS 方案

## 当前修正：195 / 196 优先

本文保留为 ProjectLogicRunner 与 Rust AOT 接入的早期技术骨架。按 `195` / `196` 的当前规则：

```text
IR 不再作为项目逻辑总真相层。
Canonical Rule IR 只作为 Contract-bound RuleSlot 的内部规范语义和构建输入。
复杂 gameplay 流程、复杂算法、复杂 UI 工作流默认进入 Rust Project Module / Rust Framework。
ProjectLogicRunner 只能执行已通过规则资产管线验证的受限规则片段。
```

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

本文档定义项目逻辑如何正式进入 Rust Runtime、FrameLoop 和 Rust ECS。

## 问题定义

当前 Rust Runtime 已经具备：

```text
EngineHostLoop
  -> FrameLoop
  -> RenderExtract
  -> RenderCommand
  -> RenderSceneState
```

但项目逻辑还没有正式接入 Rust ECS。TypeScript 侧的 IR Interpreter 只作为原型和迁移参考，不能继续膨胀成第二套正式 Runtime。

本系统要解决的问题是：

```text
AI 生成的项目规则如何进入 Rust Runtime。
IR Interpreter 和 Rust AOT 如何共享同一套语义。
项目逻辑如何读写 ECS，同时不把底层 storage 暴露给项目层。
FrameLoop 在哪里调用项目逻辑。
Trace 如何从运行结果回溯到 rule_id / requirement / source map。
```

## 其它引擎参考

### Unreal Engine

UE 的项目逻辑主要进入：

```text
World
  -> TickTaskManager
  -> Actor Tick / Component Tick
  -> Blueprint / Native C++ logic
```

参考源码：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/TickTaskManager.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/Actor.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/Components/ActorComponent.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Classes/Engine/EngineBaseTypes.h
```

UE 的特点：

```text
TickFunction 是项目逻辑进入引擎帧循环的正式入口。
Actor / Component 生命周期清晰。
TickGroup / prerequisite 可表达复杂顺序。
Blueprint 最终仍挂入 UObject / Actor / Component 生命周期体系。
复杂度强，但对非程序用户和 AI 不够直观。
```

### Unity

Unity 的项目逻辑主要进入：

```text
PlayerLoop
  -> FixedUpdate.ScriptRunBehaviourFixedUpdate
  -> Update.ScriptRunBehaviourUpdate
  -> PreLateUpdate.ScriptRunBehaviourLateUpdate
  -> C# MonoBehaviour / IL2CPP
```

参考源码：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Runtime/Export/PlayerLoop/PlayerLoop.bindings.cs
```

Unity 的特点：

```text
用户心智简单：FixedUpdate / Update / LateUpdate。
底层 PlayerLoop 很复杂，但普通用户不需要面对。
C# 逻辑与 GameObject / Component 生命周期绑定。
AI 可以生成 C#，但后期复杂项目的规则追踪、验证、回滚和热更边界不够结构化。
```

### Bevy

Bevy 的项目逻辑主要进入：

```text
App / Schedule
  -> System
  -> World
```

参考源码：

```text
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_ecs/src/schedule
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_ecs/src/system
```

Bevy 的特点：

```text
Rust ECS System 直接读写 World。
Schedule 可以根据 system 访问关系做顺序和并行调度。
性能和 ECS 结构优秀。
但普通用户和 AI 需要理解 system / query / access / schedule 等底层概念。
```

### Godot

Godot 的项目逻辑主要进入：

```text
SceneTree
  -> Node lifecycle
  -> _ready / _process / _physics_process
  -> signals
```

Godot 的特点：

```text
Node 生命周期清晰。
脚本挂在节点上，易理解。
不是 ECS 路线，大型数据驱动项目的统一查询和批处理能力弱于 ECS。
```

## 可选方案对比

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| A：Unity 式直接生成代码 | AI 直接生成 C# / Rust 脚本挂生命周期 | 简单，性能好，用户熟悉 | AI 可控性弱，规则验证弱，后期 Bug 回溯和热更边界弱 |
| B：Bevy 式纯 Rust ECS System | AI 生成 Rust System，进入 Schedule | 性能强，Runtime 结构直接 | AI 直接维护 Rust 风险高，编译反馈重，热更弱 |
| C：RuleSlot 规则资产 + ProjectLogicRunner + Rust AOT 执行路径 | AI 改 Gameplay Rule Asset / RuleSlot；Runtime 通过统一 Runner 调 Rust AOT 或受限验证执行路径 | AI 友好，可验证，可追踪，可热更，性能有 AOT 出口 | 多一层规则资产管线，但边界可控 |
| D：只跑 IR Interpreter | 所有项目逻辑解释执行 | 热更和调试最简单 | 复杂项目性能风险大 |

正式选择：

```text
采用方案 C-min。
```

C-min 的意思是保留长期正确边界，但第一版只实现最小必要结构，不引入完整自动调度器和复杂依赖推导。

## 正式架构

项目逻辑进入 FrameLoop 的正式链路：

```text
EngineHostLoop
  -> FrameLoop
    -> Input ActionSnapshot
    -> ProjectLogicRunner
      -> LogicExecutor
        -> IrInterpreterExecutor
        -> RustAotExecutor
      -> ECS World read/write
      -> LogicTrace
    -> DeferredCommandApply
    -> RenderExtract
    -> RenderCommand
    -> RenderSceneState
```

核心规则：

```text
ProjectLogicRunner 是 Runtime 调项目逻辑的唯一入口。
Gameplay Rule Asset / Contract-bound RuleSlot 是用户和 AI 面向的规则资产边界。
Canonical Rule IR 是受限 RuleSlot 的内部规范语义和构建输入。
Rust AOT 是默认执行路径；解释执行只用于受限验证、诊断或未来热更覆盖，不作为 C-min 主线。
TypeScript Runtime 不作为正式项目逻辑 Runtime 保留。
项目逻辑不直接访问底层 ECS storage。
项目逻辑通过受控 WorldReadApi / WorldWriteApi 读写 Component。
```

## ProjectLogicRunner

ProjectLogicRunner 负责：

```text
读取 RuleExecutionPlan。
按 FrameLoop 阶段执行项目规则。
为规则构造 LogicContext。
选择 LogicExecutor。
执行规则并写回 ECS。
记录 LogicTrace。
把规则产生的结构变化请求提交到 Deferred Command。
```

ProjectLogicRunner 不负责：

```text
自动理解业务依赖。
自动修复规则顺序。
直接生成 RenderCommand。
直接加载资源。
直接执行平台 API。
```

## RuleExecutionPlan

第一版不实现复杂自动 scheduler，只使用显式执行表：

```text
RuleExecutionPlan:
  fixed_update[]
  frame_update[]
  event_handler[]
```

规则：

```text
业务顺序由项目规则显式表达。
引擎不根据业务语义自动调整顺序。
引擎只负责内存安全、Component 存在性、结构变化安全点和确定性执行。
如果两个规则都写 Health，底层不判断谁的业务优先级更高。
扣血、反弹、金身、名刀等业务顺序属于项目层规则，不属于 ECS 底层规则。
```

这样做的原因：

```text
避免把业务依赖隐藏到 scheduler 里。
避免 AI 需要猜复杂底层调度规则。
避免后期 Bug 变成不可解释的隐式顺序问题。
```

## LogicExecutor

LogicExecutor 是执行后端接口。

标准形态：

```text
LogicExecutor:
  run(rule_id, LogicContext) -> LogicResult
```

第一版保留两个后端：

```text
IrInterpreterExecutor:
  用于编辑器、验证、热更覆盖、Debug。

RustAotExecutor:
  用于发布版、高频规则、高性能路径。
```

后端选择规则：

```text
如果 rule_id 有 hotfix IR，使用 IrInterpreterExecutor。
否则发布版默认使用 RustAotExecutor。
编辑器验证和 AI Patch 验证可以强制使用 IrInterpreterExecutor。
```

## Rust AOT 规则

Rust AOT 不是项目逻辑真相。

正式规则：

```text
Rust AOT 必须由 Canonical Rule IR 确定性生成。
AI 默认不直接手写 Rust AOT 代码。
Rust AOT 函数必须实现稳定 contract。
Rust AOT 行为必须与 IR Interpreter 等价。
```

建议 contract：

```rust
fn run(ctx: &mut LogicContext) -> LogicResult
```

Rust AOT 不允许：

```text
绕过 LogicContext 访问 ECS raw storage。
直接调用 RenderCommand / RHI / OS API。
直接修改 Asset Runtime 缓存。
直接创建或销毁 Entity。
```

结构变化必须通过：

```text
RuntimeSpawnRequest
RuntimeDespawnRequest
AddComponentRequest
RemoveComponentRequest
DeferredCommand
```

## ECS 读写边界

项目规则允许读写 ECS Component，但只能通过受控 API。

允许：

```text
WorldReadApi.read_component(entity, ComponentType)
WorldWriteApi.write_component_field(entity, ComponentType, field, value)
WorldWriteApi.patch_component(entity, ComponentType, patch)
```

不允许：

```text
直接访问 archetype / table / sparse storage。
直接持有 Component 指针。
直接跨线程修改 World。
直接在规则中修改 RenderSceneState。
```

引擎负责：

```text
Entity 是否存在。
Component 是否存在。
字段类型是否匹配。
写入是否发生在允许阶段。
结构变化是否进入安全 Apply Point。
render-facing component 写入后 dirty 标记。
```

项目负责：

```text
业务顺序。
扣血流程。
Buff 叠加规则。
反弹 / 免伤 / 护盾 / 死亡判定先后。
多个规则同时影响同一数据时的玩法语义。
```

## Trace 规则

每次规则执行必须能回溯：

```text
frame_index
phase
rule_id
executor_kind
sourceMap.featureId
sourceMap.requirementIds
entity_id
component_type
field
before / after
events
commands
errors
```

Trace 的目标不是每帧保存完整世界，而是保存规则执行证据。

AI 查 Bug 时应能回答：

```text
哪条规则改了这个字段。
这条规则来自哪个需求。
它在第几帧、哪个阶段执行。
它使用的是 IR Interpreter 还是 Rust AOT。
它发出了哪些 Event / Command。
```

## 热更规则

热更只影响 IR 覆盖规则。

正式规则：

```text
热更包下载、校验、mount 后，必须在安全 Apply Point 生效。
热更不能在规则执行中途替换当前调用。
已有调用栈继续使用旧版本。
下一次规则调用按 RuleRegistry 选择新版本。
State / Lifecycle Rule 正在运行的实例默认不被中途替换。
```

安全 Apply Point：

```text
FrameBegin
SceneLoad
SafePause
```

## 测试要求

第一版必须有最小测试：

```text
RuleExecutionPlan 按 fixed_update / frame_update / event_handler 顺序执行。
ProjectLogicRunner 能执行一条移动规则并写 Transform。
Transform 写入后 RenderExtract 能看到 dirty 并产生 RenderCommand。
LogicTrace 能记录 rule_id -> entity -> component -> field。
同一条规则在 IrInterpreterExecutor 和 RustAotExecutor 下结果一致。
热更 IR 覆盖只在 Apply Point 后生效。
```

## 当前项目接入点

当前代码中正式接入点应为：

```text
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/render_extract.rs
```

TypeScript 原型只作为参考：

```text
src/ir/canonicalRuleIr.ts
src/ir/ruleInterpreter.ts
src/runtime-backends/typescript/TypeScriptRuntimeBackend.ts
```

后续施工不应继续扩大 TypeScript Runtime。

## 最终结论

```text
Gameplay Rule Asset / Contract-bound RuleSlot 是用户和 AI 面向的规则资产边界。
Canonical Rule IR 是受限 RuleSlot 的内部规范语义和构建输入。
ProjectLogicRunner 是 Runtime 调项目逻辑的唯一入口。
LogicExecutor 是执行后端抽象。
Rust AOT 用于正式发布和默认运行路径。
解释执行只用于受限验证、诊断或未来热更覆盖。
ECS World 是运行时状态真相。
项目规则可通过受控 API 读写 Component。
业务顺序由项目规则负责，引擎不隐藏业务调度。
Trace 必须能从运行结果回溯到 rule_id / requirement / source map。
```
