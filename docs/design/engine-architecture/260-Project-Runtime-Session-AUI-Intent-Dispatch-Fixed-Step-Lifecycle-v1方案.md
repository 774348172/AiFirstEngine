# 260-Project Runtime Session / AUI Intent Dispatch / Fixed-Step Lifecycle v1 方案

## 1. 文档状态

```text
系统编号：260
方案版本：v1
建立日期：2026-07-30
选题来源：塔防项目 P0-5 真实 AUI 可玩闭环暴露的公共引擎能力缺口
讨论结论：引擎方案 E3 已由用户确认
当前状态：已完成施工并归档
施工状态：Gate A-F 已完成
施工文档：施工文档/已完成/260-当前可自动化施工文档-Project-Runtime-Session-AUI-Intent-Dispatch-Fixed-Step-Lifecycle-v1.md
完成记录：阶段完成记录/2026-07-30-Project-Runtime-Session-AUI-Intent-Dispatch-Fixed-Step-Lifecycle-v1/00-总览.md
施工授权：已结束
```

本文档只定义通用引擎能力；施工结果以对应已完成施工文档与阶段完成记录为准。260
完成不自动授权继续塔防 P0-5 项目施工。

塔防项目只是第一个需求方。引擎 API、report、diagnostic、测试 fixture 和第二项目 Gate
不得出现征兵、部署、合成、出售、扩格、军略、波次或其它塔防专用语义。

## 2. 一句话目的

让构建期静态链接的项目 RuntimeModule 拥有一个按运行实例创建、可持有项目状态的窄型
`ProjectRuntimeSession`，使 AUI 已生成的业务意图能够 exactly-once 进入项目写侧，并让
项目状态在引擎 FixedUpdate 节点确定性推进。

## 3. 已确认的产品决定

采用讨论中的 E3，但收敛为窄型会话钩子，不建立万能 Controller：

```text
ProjectRuntimeSession 负责：
  AUI action batch 的项目写侧接收
  每运行实例的项目 Rust 状态生命周期
  FixedUpdate 项目状态推进
  通过只读 World API / deferred mutation batch 更新项目投影
  结构化 result / diagnostic / report

ProjectRuntimeSession 不负责：
  AUI hit test、layout、binding 或 render
  ProjectUiStateSnapshot 生产
  ProjectLogicRunner 的规则排序与执行
  Physics2D、RenderProjection 或 Scene hydration
  项目资产热更新系统
  任意项目玩法语义
```

260 是 213 和 242 之间缺失写侧接缝的补齐：

```text
213：Runtime input -> AUI interaction -> AuiAction
242：RuntimePackage + statically linked ProjectRuntimeModule -> BoundProjectRuntime
260：BoundProjectRuntime 内的 stateful session 接收 AuiAction 并参与 FixedUpdate
```

260 不推翻 195、199、213、220 或 242。

## 4. 当前实现证据与缺口

### 4.1 已有能力

当前 `engine_runtime::aui` 已有：

```text
AuiActionRef
AuiActionEvent
AuiAction {
  action_id
  node_id
  event
  payload
}
AuiInteractionResult.actions
AuiInteractionSystem
```

当前 Player 和 Editor GameView 已能：

```text
RuntimeInputFrame
  -> AuiInteractionSystem
  -> consumed pointer filtering
  -> AuiInteractionResult
  -> EngineFrameInput::with_aui_interaction
```

当前 242 已有：

```text
ProjectRuntimeModule
ProjectRuntimeRegistration
ProjectRuntimeBootstrap
BoundProjectRuntime
ProjectLogicRunner
ProjectUiStateSnapshotProducer
default InputMappingAsset
```

### 4.2 断开的链路

`EngineFrameInput.aui_interaction` 当前只被保存，没有进入 runtime frame 的项目执行路径：

```text
EngineHostLoop::tick_internal
  -> 只把 action_snapshot / input_trace / delta / runtime_context 传入 FrameLoop
  -> aui_interaction 没有传给 ProjectLogicRunner
  -> aui_interaction 没有传给其它项目 callback
```

`LogicContext` 当前只提供：

```text
action_snapshot()
action_pressed()
collision_pairs()
WorldWriteApi
GameplayCommandBuffer
```

它没有 AUI action accessor。

`ProjectRuntimeRegistration` 当前只提供：

```text
register_rust_aot_rule
set_ui_state_producer_factory
```

`BoundProjectRuntime` 当前只持有：

```text
ProjectLogicRunner
ProjectUiStateSnapshotProducer
default InputMappingAsset
bind receipt
```

因此项目没有公共入口可以同时做到：

```text
接收本帧 AUI actions
持有一份运行实例级项目状态
把 action 翻译为正式项目命令
在 FixedUpdate 继续推进同一份状态
在 Play Stop / Player exit 时释放该状态
```

### 4.3 213 的明确 deferred

213 完成了：

```text
hit test
pointer consumption
click / drag / drop action mapping
interaction trace
runtime player / Editor GameView 输入接入
```

213 明确没有实现具体项目业务交易，并把 AuiAction 的响应留给 Project Logic、
RuleSlot 或 Project Module。当前代码没有实现这条公开接缝，所以 260 是 deferred gap
closure，不是重复实现 213。

## 5. 成熟引擎源码对照

### 5.1 Unity UGUI

参考：

```text
<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\Button.cs
```

关键链路：

```text
IPointerClickHandler
  -> Button.Press()
  -> ButtonClickedEvent
  -> onClick.Invoke()
```

可学习点：

```text
UI 控件负责把指针输入收敛为语义事件。
项目订阅语义事件并决定业务行为。
```

不可照搬点：

```text
不把 UnityEvent 反射、MonoBehaviour 或项目 C# callback 模型引入本引擎。
不让每个 AUI node 直接保存任意代码函数。
```

### 5.2 Unreal UMG / Slate

参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\UMG\Public\Components\Button.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\UMG\Private\Components\Button.cpp
```

关键链路：

```text
SButton::OnClicked
  -> UButton::SlateHandleClicked
  -> UButton::OnClicked.Broadcast()
```

可学习点：

```text
底层 UI 事件与上层项目 delegate 分层。
项目业务不会进入 Slate/UMG renderer。
```

不可照搬点：

```text
不引入 UObject delegate hierarchy、Blueprint VM 或大型 Subsystem tree。
```

### 5.3 Godot Control

参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\gui\control.cpp
<GODOT_SOURCE>\godot-master\godot-master\scene\gui\base_button.cpp
```

关键链路：

```text
Control::_call_gui_input
  -> gui_input signal / _gui_input
  -> BaseButton::_pressed
  -> pressed signal
```

可学习点：

```text
原始 GUI 输入先由控件解释，再产生稳定信号。
项目代码只处理语义信号。
```

不可照搬点：

```text
不引入任意节点脚本、动态方法查找或通用 Signal bus。
```

### 5.4 Bevy UI

参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_ui\src\focus.rs
<BEVY_SOURCE>\bevy-main\bevy-main\examples\ui\ui_drag_and_drop.rs
<BEVY_SOURCE>\bevy-main\bevy-main\examples\ui\widgets\button.rs
```

关键链路：

```text
Pointer<Click> / Pointer<DragDrop> / Interaction
  -> Observer 或 Query<Changed<Interaction>>
  -> 项目 System
```

可学习点：

```text
事件数据保持通用，项目 System 负责解释。
状态更新仍经过 ECS/System 边界。
```

不可照搬点：

```text
不要求当前 AUI 全量迁移成 ECS UI，不扩大 260 为输入/事件框架重写。
```

### 5.5 本项目结论

成熟引擎共同证明：

```text
UI interaction owner 只产生通用语义事件。
项目 runtime owner 处理业务意图。
Renderer 和 UI binding 不理解项目命令。
```

260 采用同一分层，但用静态链接的项目 RuntimeModule 和结构化 session interface 保持
本引擎的 schema-first、deterministic、reportable 边界。

## 6. 方案比较与最终选择

### 6.1 E1：把 AuiAction 注入 LogicContext

优点：

```text
改动最小。
简单规则可直接读取 action。
```

拒绝原因：

```text
同一 action 可能被多个 RulePhase 重复读取。
payload 解析会分散到普通规则。
无法自然持有跨帧 MatchRuntime 类项目状态。
exactly-once、拒绝和统一 report 边界不深。
```

### 6.2 E2：只增加 ProjectUiActionHandler

优点：

```text
读侧 producer 与写侧 handler 分离。
action batch 可一次性分发。
```

拒绝原因：

```text
只解决输入入口，没有解决同一项目状态的 FixedUpdate 生命周期。
handler、项目状态和 UI producer 可能被迫通过全局或共享锁拼接。
无法完整支撑状态化项目 runtime。
```

### 6.3 E3：窄型 ProjectRuntimeSession

最终选择：

```text
一个 session 同时拥有 AUI intent ingress 和 FixedUpdate lifecycle。
ProjectLogicRunner 继续拥有普通 Rule execution。
ProjectUiStateSnapshotProducer 继续拥有只读 UI read model。
World / command buffer 是 session 与其它 runtime domain 的受控交换边界。
```

E3 比 E2 多解决生命周期，但不把 UI producer、规则执行、物理和渲染吸收到 session，
因此不是万能 Controller。

## 7. 正式架构

```text
ProjectRuntimePackageAssembler
  -> RuntimePackage

ProjectRuntimeModule::install
  -> register Rust AOT rules
  -> register ProjectUiStateSnapshotProducer factory
  -> register ProjectRuntimeSession factory

ProjectRuntimeBootstrap::bind
  -> verify module descriptor
  -> create ProjectLogicRunner
  -> create ProjectRuntimeSession
  -> create ProjectUiStateSnapshotProducer
  -> bind default InputMappingAsset
  -> BoundProjectRuntime

Runtime frame
  -> AUI interaction creates ordered action batch
  -> ProjectRuntimeSession handles batch once
  -> ProjectRuntimeSession fixed update
  -> ProjectLogicRunner phases
  -> Physics / Projection / RenderExtract

Next UI present
  -> ProjectUiStateSnapshotProducer reads World
  -> AUI binding / layout / render
```

## 8. 公共接口草案

名称可以在施工前因 Rust 可见性或现有 module ownership 做窄调整，但语义不得变化。

### 8.1 Session trait

```rust
pub trait ProjectRuntimeSession {
    fn session_id(&self) -> &str;

    fn handle_aui_actions(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
        batch: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput;

    fn fixed_update(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
    ) -> ProjectRuntimeSessionOutput;
}
```

v1 不增加：

```text
update
late_update
post_physics
render
save_snapshot
network_tick
editor callback
```

若未来确有项目要求，必须基于真实需求单独讨论，不得提前把 trait 扩成完整生命周期镜像。

### 8.2 Factory

```rust
pub type ProjectRuntimeSessionFactory =
    for<'a> fn(ProjectRuntimeSessionCreateContext<'a>)
        -> Result<Box<dyn ProjectRuntimeSession>, ProjectRuntimeError>;
```

create context 至少提供：

```text
RuntimePackage 只读引用
project/module identity
report mode
```

factory 不获得裸 renderer、window、filesystem、network 或 Editor state。

### 8.3 Registration

`ProjectRuntimeRegistration` 新增一个明确的注册入口：

```rust
set_runtime_session_factory(factory)
```

规则：

```text
每个 ProjectRuntimeModule 必须注册且只能注册一个 session factory。
无状态项目必须显式注册 engine/project no-op session。
重复注册 fail closed。
缺失注册在 ProjectRuntimeBootstrap::bind 阶段失败，不延迟到首帧。
```

不使用隐式全局默认值，以便 RuntimePackage、linked module 和实际 runtime composition
保持可审计。

### 8.4 Bound runtime

`BoundProjectRuntime` 增加：

```text
project_runtime_session
session identity in bind receipt
```

session 必须跟随实际运行实例：

```text
Exported Player instance
Headless Player instance
Editor in-process GameView Play instance
Editor Step instance
```

它不能按进程、project path 或 module id 存入 global static。

## 9. Session context 边界

`ProjectRuntimeSessionContext` 只提供完成项目 runtime 推进所需的受控能力：

```text
frame_index
TimeContext
RuntimePackage 只读访问
WorldReadApi
ProjectRuntimeMutationBuffer
report mode
```

`ProjectRuntimeMutationBuffer` 是 callback 局部、由 host 拥有的 deferred batch，至少承载：

```text
component replacement
component field write
transform write
GameplayCommandBuffer structural commands
```

正式提交顺序：

```text
session callback 构造 result 和 mutation batch
  -> Rejected / Unhandled / Faulted：丢弃 batch
  -> Applied：host 对全部 target/component/structural command 做 preflight
  -> preflight passed：commit batch
  -> commit failed：runtime 进入 terminal project-session fault
```

Session public interface 不直接获得当前即时写入的 `WorldWriteApi`。普通
`ProjectLogicRunner` 是否继续使用 `WorldWriteApi` 不属于 260 的修改范围。

明确禁止：

```text
&mut World 裸引用作为长期公开合同
WorldWriteApi 即时写入接口
裸 entity index / archetype pointer
RenderSceneState / RenderProxy / GPU handle
Physics2DWorld 内部对象
EditorSession / window handle
filesystem / socket / arbitrary process
```

所有结构变化继续走 deferred command / runtime spawn / runtime despawn 正式边界。普通
component/field/transform 投影写入也必须先进入 session mutation batch，不能在 callback
中提前修改 World。

session 不得跨 callback 保存 context、World 引用或 entity 裸地址。

## 10. 帧序合同

### 10.1 Runtime advancing mode

正式顺序：

```text
FrameBegin
  -> Input/AUI result 已由 consumer 准备
  -> ProjectRuntimeSession::handle_aui_actions（最多一次）
  -> apply session action output
  -> ProjectRuntimeSession::fixed_update（每个正式 fixed step 一次）
  -> apply session fixed output
  -> ProjectLogicRunner::FixedUpdate
  -> ProjectLogicRunner::Update
  -> Physics2D sync / pair build
  -> ProjectLogicRunner::PostPhysics
  -> RenderExtract
  -> FrameEnd
```

260 服从当前 FrameLoop 的固定步模型。若未来 RuntimeTime 支持一个 host frame 内 0..N 个
fixed step，session 必须与 `ProjectLogicRunner::FixedUpdate` 使用同一个 fixed-step 计数，
不得自己建立第二个 accumulator。

### 10.2 非 advancing mode

`EditorPause` 等 `runtime_advanced=false` 模式：

```text
不调用 handle_aui_actions。
不调用 fixed_update。
不缓存 action batch 到恢复后的未来帧。
若收到 batch，report 标记 discarded_non_advancing_mode。
```

Editor 自己的 Resume/Step 命令属于 Editor command，不伪装成项目 AUI action。

### 10.3 EditorStep

一次 EditorStep：

```text
action batch 最多分发一次
session fixed_update 一次
ProjectLogicRunner FixedUpdate 一次
```

不得在 Step 后恢复时重放同一 batch。

## 11. AUI action batch 合同

### 11.1 输入结构

v1 复用现有：

```text
action_id
node_id
event
payload
```

260 不增加任何项目字段。

### 11.2 顺序

```text
AuiInteractionResult.actions 的 vector index 是当前 batch 的稳定顺序。
session 必须按原顺序遍历。
引擎不得按 action_id、node_id 或 event 重排。
```

项目命令之间的业务依赖继续由项目自己的 command revision、显式因果关系、固定阶段和
稳定 sequence 判断；引擎不猜测 A 必须在 B 前还是 B 后。

### 11.3 Exactly-once

```text
每个 advancing host frame 的 batch 最多交给 session 一次。
下一帧不得自动携带上一帧 actions。
Rejected 不重试。
Unhandled 不重试。
Session fault 不重放。
```

260 不增加幂等结果重放系统。

### 11.4 每 action 交易边界

默认采用按 action 的有序独立交易：

```text
action[0] 成功后提交。
action[1] 看到 action[0] 已提交后的项目状态。
action[1] 被拒绝不回滚 action[0]。
后续 action 是否继续由 session fault policy 决定。
```

每个正式项目命令内部仍必须满足项目自己的原子性和零写入失败合同。

不把整个 UI batch 强制设为单一原子事务，因为一个 input frame 可能同时含有相互独立的
focus、drag 和 click 意图。

### 11.5 Payload

`payload` 在 engine core 中保持 opaque：

```text
Engine 不解释项目 JSON 字段。
Summary 不输出原始 payload。
Trace 默认只输出 presence / byte length / digest。
项目负责 schema、revision、target 和范围校验。
```

260 不补 AUI authoring 的动态 payload 生成能力。若后续真实项目证明仅靠 action_id、
node_id 和现有 drag/drop payload 无法表达需求，应另开 AUI authoring 方案，不塞入
ProjectRuntimeSession。

## 12. 项目状态生命周期

### 12.1 创建

session 在 `ProjectRuntimeBootstrap::bind` 成功过程中创建：

```text
module descriptor 已匹配
RuntimePackage 已加载
rule registry 已验证
session factory 成功
UI producer factory 成功
default input 已绑定
```

任一步失败，`BoundProjectRuntime` 不可见，runtime 不进入首帧。

### 12.2 运行

session 可以持有项目 Rust Framework 状态，例如：

```text
match/session state
deterministic RNG streams
project command revision
project transaction services
cached project adapters
```

这些只是能力类别，不进入 engine schema。

### 12.3 Restart

项目的“重新开始”是项目 action：

```text
AuiAction
  -> session validates terminal/restart policy
  -> project creates fresh internal match state
  -> resets project revision/RNG according to project contract
  -> updates World projection
```

Restart 不重新绑定 RuntimeModule，不重建 EngineHostLoop，也不复用上一局未完成 action batch。

### 12.4 销毁

```text
Play Stop
Player exit
Editor GameView instance replacement
bootstrap failure rollback
```

都必须释放该实例 session。Drop 不执行跨项目写文件或网络副作用。

### 12.5 Snapshot/resume

260 不新增 session snapshot、save game、rollback snapshot 或跨进程恢复合同。

已有项目内部可序列化状态不等于引擎已拥有通用 session snapshot 能力。后续如需通用化，
必须单独方案定义 schema version、identity、migration 和失败语义。

## 13. RuleStatement 与 Rust 分工

260 继承 195 的边界：

| 逻辑类型 | Owner | 热更新方向 |
|---|---|---|
| 数值、条件、Modifier、受限触发 | Gameplay Rule Asset / RuleStatement | 在稳定 Contract 内可热更新 |
| 固定 Contract 内的受限业务编排 | Transaction RuleSlot / IR | 在引擎支持范围内可热更新 |
| AUI action 到正式项目命令的适配 | ProjectRuntimeSession | 项目 Rust，需要重建 |
| 命令协议、事务不变量、确定性状态机 | Project Rust Framework | 项目 Rust，需要重建 |
| AUI binding read model | ProjectUiStateSnapshotProducer | 项目 producer |
| hit test、input consumption、render | Engine AUI | 引擎 |

Session 不能成为“因为入口方便，所以所有规则都写 Rust”的借口。

项目实现应保持：

```text
Session 只负责入口、生命周期和复杂不变量。
简单规则继续由 RuleStatement 数据驱动。
Session 调用项目已有的正式 command/transaction API。
Session 不复制一套玩法规则。
```

260 本身不承诺新增在线 Rust hot reload、dylib replacement、Lua、Blueprint 或完整 IR
Interpreter。

## 14. World Projection 与 UI read model

正式数据流：

```text
ProjectRuntimeSession authoritative state
  -> project-owned controlled World projection
  -> Scene / presentation components
  -> ProjectUiStateSnapshotProducer reads World
  -> ProjectUiStateSnapshot
  -> AUI Binding
```

规则：

```text
ProjectUiStateSnapshotProducer 保持只读。
AUI Binding 保持只读 ProjectUiStateSnapshot。
Renderer 不读取 session 或 project command。
Session 不直接写 AUI Document runtime value。
Session 不直接生成 AuiOverlayFrame。
```

260 保留 213 的 `snapshot_frame_lag=1` 现实：

```text
当前帧 hit test 使用当前已 present 的 UI snapshot。
session 提交的新状态在下一次 UI snapshot production 中可见。
260 不为消除一帧 UI read-model 延迟而每帧生产两次 snapshot。
```

World presentation 可以按现有 RenderProjection 在同一 runtime frame 提取；AUI read model
仍按下一次 producer 调用更新。

## 15. Output、diagnostic 与 report

### 15.1 Session output

每次 callback 返回结构化结果，至少表达：

```text
session_id
stage：aui_action_dispatch / fixed_update
status：applied / no_op / rejected / faulted
handled_action_count
unhandled_action_count
rejected_action_count
staged_world_write_count
committed_world_write_count
deferred_command_count
diagnostics
```

不要用 panic、日志字符串或 renderer 状态作为正式结果。

### 15.2 Action disposition

Trace 可按 action 记录：

```text
batch_index
action_id
node_id
event
disposition：handled / unhandled / rejected / faulted
project_command_id：optional
diagnostic_codes
payload_digest：optional
```

项目 command payload、完整项目状态和原始 UI payload 不进入默认 report。

### 15.3 Report 分档

```text
Off：
  只保留运行所需 compact status，不构造长字符串、完整 action trace 或 JSON。

Summary：
  frame/session/stage/counters/status/diagnostic codes。

Trace：
  每 action disposition、稳定顺序、command correlation 和 payload digest。
```

正式 runtime 默认 Off 或 compact Summary；Editor report panel、测试和显式诊断可以使用
Summary/Trace。

### 15.4 EngineFrameOutput

`EngineFrameOutput` 应暴露可选 compact session report，供：

```text
runtime_player_winit report
Editor GameView report
headless e2e
project gate
```

consumer 不得重新解析 RuntimeTrace 猜测 session 是否执行。

## 16. 失败语义

### 16.1 Bootstrap failure

以下情况在首帧前 fail closed：

```text
缺 session factory
重复 session factory
factory error
空 session id
module/session identity 不一致
```

### 16.2 Action rejection

项目可因以下通用类别拒绝 action：

```text
unknown action
unsupported event
invalid payload
invalid project phase
stale project revision
project invariant violation
```

Engine 只保存结构化 diagnostic，不解释业务。

### 16.3 Session fault

`faulted` 表示 session 本轮不能继续可靠推进：

```text
本 callback 未提交的 command/write 丢弃。
已完成的前序独立 action 不自动回滚。
当前 action 不重放。
当前 runtime instance 标记 project session fault。
后续是否停止 runtime 由正式 host fail-closed policy 决定并写入 report。
```

如果项目内部权威状态已成功推进，但 host 在 mutation batch preflight/commit 阶段发现
投影错误，则 runtime 必须进入 terminal `project_session_projection_commit_failed`：

```text
不继续下一 phase 或下一帧。
不重放 action。
不声称自动回滚项目内部状态。
保留 project command result、mutation diagnostic 和 frame identity。
```

项目 session 应在生成 batch 前验证投影目标；host preflight 是最终安全边界，不是项目
业务回滚机制。

项目代码不得让 panic 穿过正式 session boundary。施工方案必须决定使用返回值约束、
panic containment 或两者组合，并为选定做法提供测试。

### 16.4 Non-advancing discard

暂停帧收到项目 action 时：

```text
不进入 session。
不排队。
report 记录 discarded_non_advancing_mode。
```

## 17. Consumer 装配

### 17.1 Exported Player

headless 与真实窗口 Player 必须从同一个 `BoundProjectRuntime` 获得：

```text
ProjectLogicRunner
ProjectRuntimeSession
ProjectUiStateSnapshotProducer
InputMappingAsset
bind receipt
```

不得为 headless 另造直接调用 session 的旁路。

### 17.2 Editor in-process GameView

Editor GameView 必须使用同一 session factory 和帧序：

```text
Play 创建新 session。
Pause 不推进 session。
Step 推进一步。
Stop drop session。
再次 Play 创建新 session。
```

Editor authoring state不与 runtime session 共享可变对象。

### 17.3 Project e2e

headless e2e 通过正式 EngineHostLoop 输入 action batch，不直接调用项目 session。

允许 unit test 直接测试项目 session，但不能把 unit test 当成引擎装配完成证据。

### 17.4 第二项目兼容

242 的第二项目必须：

```text
注册显式 no-op session 或一个语义不同的最小 session。
继续通过 bind、headless、Editor GameView 和 exported Player 相关 Gate。
证明 260 没有引入塔防专用依赖。
```

## 18. Bind receipt 与兼容性

`ProjectRuntimeBindReceipt` 增加：

```text
session_id
session_status
```

如改变 public interface，RuntimeModule interface version 必须按 242 合同升级；旧 package 与
新 module、或新 package 与旧 module 的交叉组合必须在 bootstrap fail closed。

迁移对象至少包括：

```text
EmptyProjectRuntimeModule
complex shooter project module
switch puzzle second-project module
runtime_player_winit
Editor GameView play composition
project_e2e_gate fixtures
tower defense project module（只在引擎施工完成后）
```

不保留一个会静默吞掉 action 的隐式兼容 adapter。无 AUI 行为的项目通过显式 no-op session
表达意图。

## 19. 明确不做

260 不做：

```text
塔防玩法逻辑或塔防 schema
AUI Document、layout、style、renderer 重写
动态列表/repeater 或动态 action payload authoring
通用 EventBus / SignalBus
Logic Ownership Router / Architecture Guard runtime layer
Lua / WASM / Blueprint VM
Rust dylib hot replace
网络同步、rollback netcode、预测
save game / session snapshot
项目 UI 同帧双 snapshot production
Editor authoring mutation
任意 renderer / physics 直写
```

## 20. 施工影响面预估

正式施工文档生成前必须重新核对，当前预估 owner：

```text
engine_runtime::project_runtime_module
  session trait/factory/registration/bind receipt/no-op session

engine_runtime::engine_host_loop
  action batch exactly-once dispatch
  session report forwarding

engine_runtime::frame_loop
  fixed-step session hook 与确定顺序

engine_runtime::logic_executor / world API
  仅复用或提取受控 write context；不把 AUI action 注入 LogicContext

runtime_player_winit
  headless/windowed BoundProjectRuntime composition

editor_core::editor_gameview_play
  Editor Play/Pause/Step/Stop session lifecycle

project_e2e_gate
  dispatch、ordering、fault、second-project、consumer equivalence

sample project RuntimeModules
  explicit session registration
```

塔防项目文件不属于 260 引擎施工文档。引擎 Gate 完成并归档后，塔防 P0-5 才能重新进入
项目方案/施工流程。

## 21. Gate 意图

本节只定义未来施工文档必须覆盖的结果，不是可直接执行的施工步骤。

### Gate A：Registration / lifecycle

```text
显式 session factory 成功绑定。
缺失/重复/错误 factory fail closed。
每个 runtime instance 获得不同 session。
Play Stop / Player exit 释放 session。
bind receipt 包含 session identity。
```

### Gate B：AUI exactly-once

```text
单 click action 调用一次。
下一空帧不重放。
Rejected/Unhandled 不重试。
多个 action 保持 vector 顺序。
非 advancing frame 不调用并明确 discard。
```

### Gate C：FixedUpdate

```text
每个正式 fixed step 调用 session 一次。
Session fixed update 与 ProjectLogicRunner FixedUpdate 顺序稳定。
EditorStep 精确推进一次。
Pause 不推进。
```

### Gate D：World / transaction safety

```text
Session 只能使用 WorldReadApi / deferred mutation batch。
Rejected/Unhandled/Faulted output 不提交 batch。
Applied batch 必须完整 preflight 后提交。
投影提交失败必须终止 session，不允许带漂移继续运行。
结构变化不绕过 command buffer。
无裸 ECS/renderer/physics 指针泄漏。
```

### Gate E：Report

```text
Off/Summary/Trace 分档成立。
Summary 不输出原始 payload。
Trace 可证明 action 顺序、disposition 和 command correlation。
EngineFrameOutput 有正式 compact report。
```

### Gate F：Consumer equivalence

```text
headless Player
windowed/exported Player
Editor GameView Play
Editor Pause/Step/Stop
project e2e
```

使用同一公共 session chain。

### Gate G：第二项目与回归

```text
第二项目显式 no-op/minimal session。
Core/Player 无塔防专用概念。
现有 AUI interaction、ProjectRuntimeModule、UI producer、规则执行和输入 Gate 不回归。
```

### Gate H：第一个真实项目消费

260 引擎施工完成后的独立项目 Gate 才验证：

```text
AUI action
  -> project session
  -> project command
  -> project state
  -> World projection
  -> 下一帧 UI snapshot
```

该 Gate 可以使用塔防项目，但其项目实现和证据不能写进 260 引擎施工范围。

## 22. 风险与控制

### 22.1 Session 变成万能层

控制：

```text
v1 只有 handle_aui_actions 和 fixed_update。
不加入 renderer、physics、filesystem、network 或 Editor callback。
规则与 UI producer 继续由既有 owner 持有。
```

### 22.2 项目状态与 World 双真相漂移

控制：

```text
项目 Rust state 是项目业务权威。
World 是受控 presentation/projection。
每次成功 action/fixed update 后由同一项目 adapter 更新投影。
测试断言 command result、World projection 和 UI snapshot 的对应关系。
```

260 不规定每个项目的内部 state schema，但项目方案必须声明权威 owner。

### 22.3 action 重放或跨 phase 重复

控制：

```text
action 不进入普通 LogicContext。
EngineHostLoop 单一 dispatch owner。
batch 不缓存到下一帧。
report 记录 frame/batch/action disposition。
```

### 22.4 为热更新把复杂状态机塞进 IR

控制：

```text
Session 和项目 Rust Framework 保持复杂不变量。
RuleStatement 只在固定 Contract 内表达受限规则。
260 不扩张 IR 词汇。
```

### 22.5 破坏 242 的通用 RuntimeModule

控制：

```text
session 通过 ProjectRuntimeRegistration 注册。
RuntimePackage/module identity 继续由 242 验证。
第二项目 Gate 必须通过。
Core 禁止项目专用词和依赖。
```

## 23. 塔防项目后续边界

260 施工并归档后，塔防 P0-5 方案 C 可以在项目侧实现：

```text
TowerDefenseRuntimeSession（项目名只存在项目侧）
  owns MatchRuntime
  maps AUI action -> existing MatchCommand
  calls MatchRuntime::apply
  calls MatchRuntime::advance on fixed update
  writes project-owned World projection

TowerDefenseUiStateProducer
  reads World projection
  produces active binding paths
```

项目侧仍须单独讨论：

```text
AUI Document 和固定 node/action id
Scene presentation entity/component
MatchRuntime -> World projection schema
UI command feedback
终局 restart
Editor Play 验收
```

260 的确认不自动确认这些塔防项目细节，也不授权修改塔防项目。

## 24. 方案自审

### 24.1 是否修改了已确认方案的核心边界

否。

```text
195：AUI action 进入 Project Logic / Project Rule，保持。
199：ProjectUiStateSnapshotProducer 只读，保持。
213：AUI interaction/action mapping owner，保持。
220：Editor GameView input/AUI routed dispatch，保持。
242：ProjectRuntimeModule 静态链接与 bootstrap identity，保持。
```

260 只增加 242 尚未提供的状态化项目运行实例和 AUI 写侧入口。

### 24.2 是否建立新的 Logic Ownership Router

否。

`ProjectRuntimeSession` 是 `ProjectRuntimeModule` 的一个实例化部件，不负责选择哪个系统拥有
逻辑，也不在运行时路由任意 domain。接口只有两个固定 callback。

### 24.3 是否把项目玩法写进 Engine Core

否。

公共合同只有 action、session、time、World read/deferred mutation、command、diagnostic 和 report 等通用
概念。塔防只作为后续独立 consumer。

### 24.4 是否破坏热更新目标

否。

方案明确要求简单数值、条件和受限 Modifier 留在 RuleStatement/RuleSlot。Session 只承接
复杂状态生命周期、命令协议和确定性事务；Rust 变化仍需重建。

### 24.5 是否保证 exactly-once 和确定性

是。

```text
EngineHostLoop 是唯一 dispatch owner。
vector index 是 batch 稳定顺序。
每 action 独立有序提交。
Rejected/Unhandled/Faulted 不重试。
非 advancing frame 不排队。
```

### 24.6 是否遗漏 runtime consumer

没有。

方案覆盖 exported/headless/windowed Player、Editor Play/Pause/Step/Stop、project e2e 和第二
项目兼容。

### 24.7 是否过早扩大范围

没有。

动态 payload authoring、snapshot/resume、网络、通用 EventBus、完整生命周期 callback 和
同帧双 UI snapshot 均明确 deferred。

### 24.8 方案审查结论

```text
结论：通过
必须修改正式方案：无
施工约束：第 8-18、20-22 节必须写入未来施工文档 Gate 和自审
当前可施工：否
原因：尚未生成施工文档，且用户尚未授权施工
```

## 25. 下一步

下一步只能是：

```text
260 独立引擎施工文档已生成、自审并进入 待执行/
  -> 用户明确授权开始施工
  -> 激活前复核
  -> 移动到 当前/ 并同步 54
  -> 才开始修改引擎代码
```

在 260 引擎施工完成、Gate 通过、完成记录生成并归档前：

```text
塔防 P0-5 方案 C 保持已选方向。
塔防项目不得用私有引擎改动或全局静态状态绕过缺口。
不得把 headless 直接调用 MatchRuntime 伪装成 AUI 可玩闭环。
```
