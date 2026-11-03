# 当前可自动化施工文档：ProjectLogicRunner / LogicExecutor / Rust ECS 接入 v1

状态：已确认，可以直接施工。

本文档只定义“怎么做”和“怎么验收”。长期规则以对应框架文档为准。

## 规则来源

```text
../../31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
../../05-逻辑系统边界-DSL-IR-RustAOT-ECS.md
../../17-Runtime-FrameLoop.md
../../21-Runtime-Core-Boundary.md
../../39-Rust-Runtime-ECS路线.md
../../40-Input-System路线.md
../../50-RenderCommand-RenderSceneState方案.md
../../51-RenderDirtyTracker-RenderExtract-RenderCommand闭环方案.md
../../54-当前可自动化施工总入口.md
```

## 问题定义

当前 Rust Runtime 已有：

```text
EngineHostLoop
  -> FrameLoop
  -> World / ECS
  -> DirtyRecord
  -> RenderExtract
  -> RenderCommand
  -> RenderSceneState
  -> RuntimeRenderer / MinimalRenderer
```

但项目逻辑还没有正式进入 Rust Runtime。当前 `FrameLoop` 只记录：

```text
FixedUpdate
Update
LateUpdate
```

这些只是 trace phase，不会真正执行用户项目规则。

本阶段要补上：

```text
FrameLoop
  -> ProjectLogicRunner
  -> LogicExecutor
  -> controlled WorldReadApi / WorldWriteApi
  -> ECS Component write
  -> DirtyRecord
  -> RenderExtract
```

## 参考引擎对照

### Unity

Unity 的用户逻辑通过 `PlayerLoop` 进入：

```text
FixedUpdate.ScriptRunBehaviourFixedUpdate
Update.ScriptRunBehaviourUpdate
PreLateUpdate.ScriptRunBehaviourLateUpdate
MonoBehaviour / IL2CPP
```

优点：

```text
用户心智简单。
Update / FixedUpdate / LateUpdate 足够直观。
```

缺点：

```text
项目逻辑真实依赖和修改来源不够结构化。
AI 很难稳定追踪一条规则来自哪个自然语言需求。
```

### Unreal Engine

UE 的用户逻辑通过 `World / TickTaskManager / Actor Tick / Component Tick / Blueprint / Native C++` 进入。

优点：

```text
生命周期和 Tick 入口正式。
复杂项目能力强。
```

缺点：

```text
TickGroup / prerequisite / Actor lifecycle 复杂。
对 AI 和轻程序用户不够直接。
```

### Bevy

Bevy 的用户逻辑通过：

```text
App / Schedule / System / World
```

优点：

```text
Rust ECS 路线清晰。
System 读写 World 的性能和并行潜力强。
```

缺点：

```text
普通用户和 AI 需要理解 schedule / query / access / system order。
底层调度规则容易变成隐藏复杂度。
```

### Godot

Godot 的用户逻辑通过：

```text
SceneTree / Node / _ready / _process / _physics_process / signal
```

优点：

```text
生命周期入口简单。
脚本挂在节点上，用户好理解。
```

缺点：

```text
不是 ECS 数据路线。
大型数据驱动项目的统一批处理和规则验证能力弱于 ECS。
```

### 本项目选择

采用：

```text
Unity-like 简单帧入口
+ UE-like 正式 runtime 入口边界
+ Bevy-like Rust ECS 数据底座
+ AI-first Rule / Trace / SourceMap
```

不采用：

```text
Bevy 式把完整 schedule 复杂度暴露给项目层。
UE 式把 TickGroup / prerequisite 作为第一版用户规则。
Unity 式直接让 AI 生成普通 C# 行为脚本。
Godot 式 Node script 作为底层 runtime 架构。
```

## 施工目标

实现 Project Logic 正式进入 Rust Runtime 的最小闭环：

```text
RuleExecutionPlan
  -> ProjectLogicRunner
  -> LogicExecutor
  -> WorldReadApi / WorldWriteApi
  -> World Transform write
  -> DirtyRecord
  -> RenderExtract sees dirty Transform
  -> RuntimeTrace records rule execution
```

第一版用一个最小移动规则验证：

```text
entity-player.local_position.x += velocity_x * delta_time
```

## 非目标

本阶段不做：

```text
IR -> Rust AOT codegen。
完整 Rust IR Interpreter。
真实热更 mount / rule override。
复杂业务依赖自动推导。
完整多线程 job scheduler。
完整 ActionSnapshot 输入回流。
真实脚本语言。
TypeScript Runtime 继续扩张。
```

重要边界：

```text
TypeScript IR Interpreter 只作为历史 prototype 和语义参考。
本阶段不能把 TypeScript Runtime 接回正式 runtime 主线。
RustAotExecutor 第一版可以用手写 generated-like Rust function 验证 contract，但不能把手写函数当成最终用户脚本路线。
```

## 推荐文件

优先新增：

```text
rust/crates/engine_runtime/src/project_logic.rs
rust/crates/engine_runtime/src/logic_executor.rs
rust/crates/engine_runtime/src/world_api.rs
```

优先修改：

```text
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/runtime_trace.rs
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/lib.rs
```

可选新增测试脚本：

```text
scripts/test-rust-project-logic-runner.cjs
```

如果只用 Rust 单元测试即可覆盖，本阶段可以不新增 Node 脚本。

## 数据结构要求

### RuleExecutionPlan

第一版结构：

```text
RuleExecutionPlan:
  fixed_update: RuleCall[]
  frame_update: RuleCall[]
  event_handler: RuleCall[]
```

`RuleCall`：

```text
rule_id
executor_hint
enabled
```

规则：

```text
执行顺序只按数组顺序。
引擎不根据业务语义自动重排。
引擎只做存在性、类型、安全写入和 trace。
```

### ProjectLogicRunner

职责：

```text
持有 RuleExecutionPlan。
在 FrameLoop 的固定位置执行 fixed_update / frame_update。
为每条规则创建 LogicContext。
调用 LogicExecutor。
把 LogicResult 写入 RuntimeTrace。
```

不负责：

```text
自动理解业务依赖。
自动决定扣血、反弹、金身、死亡等业务顺序。
直接生成 RenderCommand。
直接访问 ECS raw storage。
```

### LogicExecutor

第一版接口：

```text
LogicExecutor:
  run(rule_id, LogicContext) -> LogicResult
```

执行后端：

```text
RustAotExecutor:
  第一版使用手写 generated-like Rust function registry。

IrInterpreterExecutor:
  第一版只保留类型和错误路径。
  不实现完整解释器。
  不回退到 TypeScript Runtime。
```

这样做的原因：

```text
先固定正式 Rust Runtime contract。
避免为了赶进度把 TypeScript Runtime 扩成第二套正式引擎。
给后续 IR Interpreter / Rust AOT codegen 留稳定接入口。
```

### LogicContext

第一版字段：

```text
frame_index
delta_time
phase
world_read
world_write
trace_writer
```

第一版可以先不接真实输入，只保留扩展点：

```text
action_snapshot optional
```

### WorldReadApi / WorldWriteApi

允许：

```text
read_transform(entity_id)
write_transform(entity_id, transform)
write_transform_local_position(entity_id, local_position)
```

禁止：

```text
暴露 archetype table。
暴露 raw component pointer。
暴露 mutable World 给项目规则。
项目规则直接 mark dirty。
项目规则直接写 RenderSceneState。
```

dirty 规则：

```text
WorldWriteApi 写 Transform 后，由 World 内部 setter 标记 DirtyType::Transform。
项目规则不手写 dirty record。
```

## FrameLoop 接入规则

`tick_runtime_frame` 的第一版顺序：

```text
FrameBegin
FixedUpdate trace
ProjectLogicRunner.fixed_update
Update trace
ProjectLogicRunner.frame_update
LateUpdate trace
RenderExtract
RenderCommand Apply
FrameEnd
```

规则：

```text
ProjectLogicRunner 必须在 RenderExtract 之前执行。
项目逻辑只能通过 WorldWriteApi 修改 ECS Component。
RenderExtract 只根据 DirtyRecord / Render-facing Component 生成 RenderCommand。
```

兼容要求：

```text
空 RuleExecutionPlan 时，现有 FrameLoop 测试结果不能改变。
现有 tick(world: &World) compatibility path 可以保持只读，不强制接 ProjectLogicRunner。
```

## RuntimeTrace 要求

每条规则至少记录：

```text
frame_index
phase
rule_id
executor_kind
status
entity_id optional
component_type optional
field optional
before optional
after optional
error optional
```

第一版 trace 字段保持少而精，不保存完整世界快照。

AI 查问题时至少要能回答：

```text
第几帧执行了哪条规则。
规则用的是 RustAotExecutor 还是 IrInterpreterExecutor。
规则改了哪个 Entity 的哪个 Component / field。
规则是否失败，失败原因是什么。
```

## 最小测试用例

必须新增或覆盖：

```text
RuleExecutionPlan 按 fixed_update / frame_update 数组顺序执行。
ProjectLogicRunner 能执行一条移动规则并写 Transform.local_position。
Transform 写入后 World 产生 DirtyType::Transform。
RenderExtract 能在同一帧看到该 dirty 并产生 RenderCommand。
RuntimeTrace 能记录 rule_id / executor_kind / entity_id / component / field。
空 RuleExecutionPlan 不改变现有 FrameLoop 行为。
IrInterpreterExecutor 未实现时返回结构化 unsupported diagnostic，不回退 TypeScript Runtime。
```

建议测试名：

```text
project_logic_runner_executes_frame_update_rule
project_logic_write_transform_marks_dirty
runtime_frame_runs_logic_before_render_extract
project_logic_trace_records_rule_write
empty_project_logic_plan_preserves_existing_frame_output
ir_interpreter_executor_is_structured_unsupported_in_v1
```

## 验证命令

最低验证：

```powershell
cargo test -p engine_runtime project_logic
cargo test -p engine_runtime frame_loop
cargo test -p engine_runtime render_extract
```

完整 Rust runtime 回归：

```powershell
cargo test -p engine_runtime
```

如果新增 Node smoke：

```powershell
node scripts\test-rust-project-logic-runner.cjs
```

不要求本阶段跑：

```text
真实 wgpu surface。
真实 Rust runtime process spawn。
IR -> Rust AOT codegen。
TypeScript Runtime equivalence test。
```

## 完成标准

本阶段完成后必须满足：

```text
ProjectLogicRunner 成为 Rust Runtime 调项目逻辑的正式入口。
FrameLoop 在 RenderExtract 前执行项目逻辑。
项目逻辑通过受控 World API 写 ECS。
Transform 写入能进入 DirtyRecord，并被 RenderExtract 消费。
RuntimeTrace 能追到 rule_id 和写入字段。
TypeScript Runtime 没有新增正式能力。
```

## 完成后文档更新

施工完成后必须：

```text
新增 阶段完成记录/2026-xx-xx-ProjectLogicRunner-LogicExecutor-RustECS接入-v1/00-总览.md
将本文档移动到 施工文档/已完成/
更新 ../../49-当前状态与下一步入口.md
更新 ../../54-当前可自动化施工总入口.md
更新 ../../18-长期主义实现路径.md
更新 ../README.md
如实现结果修正规则，回填 ../../31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
```

## 下一阶段候选

本阶段完成后，优先讨论或施工：

```text
Native Editor Viewport 输入 -> Runtime Action -> ProjectLogicRunner。
真实 Rust runtime CLI / process spawn。
Rust IR Interpreter v1 或 IR -> Rust AOT codegen 的正式边界。
```
