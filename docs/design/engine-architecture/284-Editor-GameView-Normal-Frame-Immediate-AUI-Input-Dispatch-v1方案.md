# 284 Editor GameView Normal-Frame Immediate AUI Input Dispatch v1 方案

## 1. 文档状态

```text
系统编号：284
方案版本：v1
建立日期：2026-08-13
问题来源：普通 Production Editor GameView 中 AUI 按钮反馈与业务响应明显滞后
选定方案：普通帧即时 AUI 输入阶段
用户确认：已确认
当前状态：Gate A-C 已完成，施工文档和阶段完成记录已归档
施工授权：已按 Window A / Gate A-B 与 Window B / Gate C 两次独立授权完成
```

本文档只固化已确认的正式方案，不构成代码修改、测试、production Editor 更新、缓存重建或
真实配置修改授权。

## 2. 一句话目的

让普通 Editor GameView 在输入到达后的本轮普通主循环中完成 AUI 命中、按钮瞬态反馈和
业务 `AuiActionDispatch`，再由本轮既有 redraw/render/present 显示结果；战斗、物理、怪物移动和
Animator2D fixed progression 仍只由 fixed tick 推进。

## 3. 已确认问题

当前普通 Editor 的 GameView 输入路径为：

```text
EditorInputEvent
  -> NativeEditorApplication::try_route_production_game_view_input
  -> ViewportInputGateway::route_editor_input
  -> EditorSession::tick_active_game_view_runtime_descriptor_frame_with_input
  -> EditorRuntimePlayInstance::tick_next_descriptor_frame_with_runtime_input
  -> build AUI present/layout
  -> AUI interaction / feedback / InputResolver
  -> EngineHostLoop::tick
  -> ProjectRuntimeSession AUI action dispatch
  -> fixed update / project rules / physics / render extraction
  -> Animator2D observation / GameView frame publication
```

确认的因果缺陷不是 AUI action 配置错误，也不是 Tower fixed tick 低于目标帧率，而是：

```text
每个 GameView pointer 输入都同步触发一个完整 runtime/GameView tick。
```

因此一次 PointerDown、PointerUp 或 PointerMove 可能承担与完整游戏帧接近的成本。输入消息如果与
普通 16 ms tick、最多 8 次 catch-up 或复杂 AUI/render extraction 相遇，按钮反馈和业务回调都会被
完整帧成本拖延。

283 已移除普通 GameView 每帧 report 写盘热路径，证明运行时本身可稳定推进，但没有改变上述
“输入即完整 tick”的阶段耦合，因此 284 是独立的最小因果修复。

## 4. “当前帧”的正式定义

本方案中的“当前帧处理”不是修改已经呈现在显示器上的旧像素，也不是在 Windows 消息回调里直接
提交一张独立 GPU 帧。

正式定义为：

```text
OS 输入事件进入 Native Editor 主线程
  -> 本轮普通主循环的 Input/AUI 阶段立即消费
  -> 本轮普通主循环后续执行 redraw/render/present
  -> 最近一次显示器 Present 可见
```

必须区分：

```text
不等待下一次 fixed tick：是
不等待下一次 ordinary render/present：物理上不可能
为输入单独创建 GPU present：否
```

60 Hz 显示设备仍可能产生 `0..16.7 ms` 的扫描/Present 等待。这是显示设备边界，不得被误报为
引擎 fixed-tick 延迟。

## 5. 成熟引擎依据

### 5.1 Unity UGUI

本地源码：

```text
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/EventSystem/EventSystem.cs
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UI/Core/Selectable.cs
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UI/Core/Button.cs
```

关键顺序：

```text
EventSystem.Update
  -> currentInputModule.Process
  -> Selectable.OnPointerDown / OnPointerUp
  -> Button.OnPointerClick
  -> Button.Press
  -> onClick.Invoke
  -> Canvas pre-render rebuild / render
```

按钮状态和业务回调属于普通 Update，不等待 `FixedUpdate`。

### 5.2 Unreal Slate

本地源码：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Slate/Private/Framework/Application/SlateApplication.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Slate/Private/Widgets/Input/SButton.cpp
```

关键顺序：

```text
ProcessMouseButtonDownEvent / ProcessMouseButtonUpEvent
  -> RoutePointerDownEvent / RoutePointerUpEvent
  -> SButton::OnMouseButtonDown / OnMouseButtonUp
  -> Press / Release
  -> ExecuteOnClick
  -> Slate TickAndDrawWidgets
```

`SButton` 在输入路由中立即更新 pressed 状态并按 click method 执行回调，显示由随后 Slate draw 完成。

### 5.3 Godot Control

参考：

```text
https://docs.godotengine.org/en/stable/tutorials/inputs/inputevent.html
<GODOT_SOURCE>/godot-master/godot-master/scene/gui/base_button.cpp
```

关键顺序：

```text
Viewport input
  -> BaseButton::gui_input / on_action_event
  -> pressed/button_down/button_up signal
  -> queue_redraw
  -> subsequent draw
```

状态与 signal 在输入事件处理中完成，`queue_redraw` 请求最近一次绘制，不等待 physics process。

### 5.4 Bevy UI

参考：

```text
https://docs.rs/bevy/latest/bevy/input/struct.InputPlugin.html
https://docs.rs/bevy/latest/bevy/ui/enum.Interaction.html
```

输入、UI interaction 和 UI layout/extraction 进入普通 app schedule，而不是把 UI click 绑定到
fixed schedule。

### 5.5 对本引擎的结论

成熟引擎共同采用：

```text
普通输入阶段解释 pointer
  -> 立即更新控件瞬态
  -> 普通 update/event 阶段执行 UI 业务回调
  -> 同一轮后续 render/present

fixed step
  -> 只负责需要固定时间语义的模拟
```

284 吸收该阶段划分，但继续保留本引擎结构化 `AuiInteractionResult`、项目 Rust session、deferred
mutation 和可审查 report，不引入 UnityEvent、UObject delegate、Godot 动态 signal 或全量 ECS UI。

## 6. 正式方案

### 6.1 普通帧总顺序

```text
Native Editor ordinary frame
  1. Platform input collect / normalize
  2. Viewport routing and target-space coordinate resolve
  3. Immediate AUI interaction
     - hit test
     - hover / capture / focus
     - PointerDown -> Pressed
     - PointerUp -> Released / Click / Activated
     - consumed input filtering
  4. Immediate project AUI action dispatch
     - ordered AuiAction batch
     - ProjectRuntimeSession::handle_aui_actions
     - existing deferred mutation preflight + commit
     - exactly once
  5. Refresh action-affected project observation / AUI binding
  6. AUI feedback advance and draw extraction
  7. Existing Editor redraw / RuntimeRenderer composition / Present

Independent fixed tick
  - ProjectRuntimeSession::fixed_update
  - ProjectLogicRunner FixedUpdate
  - physics
  - combat simulation
  - enemy movement
  - Animator2D fixed progression
```

普通帧 AUI 输入阶段不得调用完整 `EngineHostLoop::tick` 或
`FrameLoop::tick_runtime_frame_with_project_session_and_delta`。

它也不得伪装成 `EngineHostMode::EditorPause` 或其它 `runtime_advanced=false` host frame。260 对
非推进 host frame 中 action 的 `discarded_non_advancing_mode` 规则继续有效；284 增加的是 host frame
之外、由 active runtime instance owner 串行调用的离散 action stage。

### 6.2 PointerDown

```text
PointerDown
  -> 使用最近一次已发布且身份匹配的 AUI document/layout/presentation 命中
  -> 更新 AuiInteractionState.pressed_node / capture
  -> 生成 Pressed visual snapshot
  -> 标记 AUI draw dirty
  -> request_redraw
```

PointerDown 不等待 fixed tick，也不运行 ProjectRuntimeSession fixed update、物理或 world render
extraction。

### 6.3 PointerUp / Click

```text
PointerUp
  -> 使用同一 capture/session identity 完成 release
  -> inside + eligible 时生成 Click command 和 AuiAction
  -> 立即执行 AuiActionDispatch
  -> 提交现有 deferred mutation
  -> 产生 Activated/Released visual snapshot
  -> 刷新必要 observation/binding
  -> request_redraw
```

普通 AUI 业务 action 不排队等待下一个 fixed tick。Rejected、Unhandled 和 Faulted 继续沿用 260
语义，不自动重试。

### 6.4 业务 action 与连续模拟的边界

以下类型属于即时 action dispatch：

```text
选择、招募、购买、出售、解锁
开始回合/提交准备状态
项目内 pause/resume toggle
菜单、确认、取消、重开意图
其它由 AuiAction 表达的离散项目命令
```

以下类型仍只属于 fixed tick：

```text
战斗时间推进
怪物路径移动
碰撞与物理
持续伤害、射击间隔、波次计时
Animator2D fixed progression
依赖固定时间步的规则
```

离散 action 可以在普通帧改变 authoritative project state；由它启动的连续模拟从最近一次后续 fixed
tick 开始。

### 6.5 两种暂停不得混淆

```text
Editor Play Pause
  Editor 控制层状态，暂停 runtime fixed progression。

Project AUI pause action
  项目业务 action，例如 Tower 的 td.toggle-pause。
```

项目 AUI pause/resume 必须由普通帧即时 action dispatch 消费，否则 resume action 可能被已经暂停的
fixed progression 阻断。Editor Pause 状态下是否允许项目 GameView AUI action，继续服从现有 Editor
Play 控制合同；284 不把 Editor toolbar Pause 改造成项目 action。

### 6.6 未被 AUI 消费的 gameplay input

`RuntimeInputFrame` 中未被 AUI 消费的事件不能因为移除同步完整 tick 而丢失：

```text
ordinary input stage
  -> AUI interaction 得到 consumed indices
  -> AUI actions 立即 dispatch
  -> filtered gameplay input 进入 active play instance 的有界 pending input
  -> 最近一次正常 runtime tick 由既有 InputResolver 消费一次
```

该 pending input 只保存标准化的非 AUI gameplay input，不保存或重放已经即时提交的 `AuiAction`。
它必须：

```text
保持事件顺序和 frame/device identity
对连续 PointerMove 使用现有输入语义做有界合并，不形成无界队列
在 Stop、session replacement、focus lost/cancel 时清理
在下一正常 runtime tick exactly once drain
```

如果输入仅包含已被 AUI 消费的按钮事件，pending gameplay input 为空。

## 7. Owner 与接口边界

### 7.1 `editor_window_winit`

负责：

```text
接收和规范化平台输入
走既有 ViewportInputGateway
触发普通帧 immediate AUI route
根据结果 request_redraw
```

不得：

```text
在平台 adapter 中解释项目 action
直接修改 ECS/world
直接调用 ProjectRuntimeSession
为每次 pointer event 创建独立 GPU present
```

### 7.2 `editor_core::EditorSession`

新增或收敛一个窄型普通帧入口，概念语义为：

```rust
route_active_game_view_aui_input(runtime_input_frame)
```

它只委托 active `EditorRuntimePlayInstance` 的 immediate AUI path，并同步最新 GameView UI
publication/report；不再把输入入口映射为 `tick_next_descriptor_frame_with_runtime_input`。

最终 Rust 名称可在施工前按现有可见性做窄调整，但行为边界不得变化。

### 7.3 `EditorRuntimePlayInstance`

拥有：

```text
最近一次稳定 AUI resolved document/layout/presentation cache
AuiInteractionState
AuiControlFeedbackState
通过 EngineHostLoop 绑定的 active ProjectRuntimeSession
World 与 project observation state
last GameView world/render publication
filtered gameplay input 的有界 pending state
```

普通帧入口复用这些现有 owner，不建立第二套 AUI runtime 或第二个 project session。

### 7.4 `engine_runtime`

应从当前完整 FrameLoop 中复用或窄提取现有 `AuiActionDispatch` owner，使其能够：

```text
执行 ProjectRuntimeSession::handle_aui_actions
复用现有 mutation prepare/commit
复用 panic/fault/rejected/unhandled 语义
可选执行 action 后 observation refresh
不 advance RuntimeTime
不调用 fixed_update
不运行 ProjectLogicRunner/physics/render extraction
```

不得复制一套 mutation commit 实现。若现有 helper 的可见性不足，只移动/提升该 helper，不重写协议。

该 stage 使用 active runtime instance 当前已提交的 runtime frame index 与只读 `TimeContext` 快照作为
action context，不调用 `RuntimeTime::advance_frame` 或 `advance_fixed_step`。为了区分同一 runtime frame
index 内可能发生的多个普通输入事件，exactly-once 由 Editor 主线程输入顺序和事件 identity 保证，不得
伪造额外 gameplay frame。

如果项目 action 产生 `Animator2DCommand`，即时 stage 必须把离散命令 exactly once 交给 active
`Animator2DModule` 的既有 pending-command owner；不得丢弃，也不得留到下一次重新执行 action。这里的
`apply` 只安装 bool/trigger 等离散命令，不调用 Animator2D `tick`，不推进 clip time，不执行 fixed
progression 或 observation scan。命令的时间推进效果仍从后续 fixed tick 开始。

### 7.5 Tower 项目

Tower 的 action id、AUI document 和 `TowerDefenseRuntimeSession::handle_aui_actions` 保持项目所有权。
284 不把 `td.*`、回合、招募、出售、暂停或怪物概念写入引擎 Core。

Tower 无需为 284 修改 action schema；项目侧只作为真实 consumer 验证普通按钮业务 action 已在 fixed tick
前提交。

## 8. 缓存与失效合同

即时 hit test 只能使用与当前 active Play instance 同身份的稳定缓存：

```text
session_id
document_id / document revision
GameView target extent
scale policy
presentation/surface generation
```

出现以下状态时必须 fail closed，不得对过期布局执行 action：

```text
尚无成功 AUI present
Play instance 已替换或 Stop
target extent / scale policy 已改变但布局未重建
document re-hydration 后 identity 不匹配
surface generation 失效
node hidden/removed/disabled
```

失效结果应清理 pressed/capture，保留输入未路由或明确 consumed 的既有规则，并请求正常 frame 重建；
不得用同步完整 GameView tick 作为 fallback。

## 9. Observation、Binding 与 Present

### 9.1 即时 UI 结果

action 修改项目 session state 或提交 World mutation 后，只刷新使本轮 UI 正确所需的读侧：

```text
active observation contract
active binding paths
dirty/cached ProjectUiStateSnapshot
AUI feedback overrides
AUI draw/composition publication
```

不得在每次 pointer move 时全量生成全项目 UI state。无 action 且只有 hover/pressed 变化时，应只更新
interaction/feedback 与 AUI draw。

### 9.2 World 画面

普通帧 immediate AUI path 复用最近一次已发布的 world/GameView render result。只有下一个正常 runtime
frame 才重新进行 world render extraction。

如果 action 提交了影响 world presentation 的 mutation：

```text
业务状态立即提交；
AUI 可在本轮反映；
world sprite/physics presentation 在最近一次正常 runtime frame 同步。
```

284 不增加“仅 world 重提取”或输入专用 render graph。

### 9.3 AUI-only presentation 身份

当前 `GameViewRuntimeFrame` 和 Editor publication 的 last-good 复用判断主要依赖：

```text
session_id
runtime frame_index
runtime frame_hash
target width / height
```

普通帧即时 AUI path 不推进 runtime frame，因此只更新 AUI draw 后，上述 gameplay/runtime 身份仍可能完全
相同。如果 publication 继续把它判定为 last-good reuse，`request_redraw` 只会再次显示旧合成纹理，Pressed、
Released、Activated 和 action 后 binding 虽已更新，却不会在屏幕上兑现。

284 因此要求在既有 GameView publication 边界补一个最小的 ordinary-presentation content identity：

```text
runtime frame_index / frame_hash
  -> 只标识 gameplay/world runtime frame，AUI-only 更新不得修改或伪造它们

ordinary presentation revision/content identity
  -> 当 AUI layout/draw/feedback/binding 的可见合成内容改变时更新
  -> 当 world 与 AUI 内容都未改变时保持稳定
```

publication/reuse 合同为：

```text
相同 world identity + 相同 AUI presentation identity
  -> 继续复用 last-good，不提交新的 runtime composition work

相同 world identity + 新 AUI presentation identity
  -> 复用最近 world render result
  -> 经既有 ordinary redraw / RuntimeRenderer composition 路径重新合成 UI
  -> 发布新的可见 presentation

新 runtime frame identity
  -> 沿用既有完整 runtime publication
```

这不是输入回调内独立 GPU Present：输入阶段只更新状态、publication content identity 并
`request_redraw`，实际 GPU composition/Present 仍由既有普通 redraw 生命周期执行。它也不是伪造 gameplay
frame、world render extraction 或 RenderGraph 重构。

施工前复核应优先寻找现有可复用的内部 publication generation/hash；只有现有字段确实无法区分 AUI-only
变化时，才在最窄的 GameView frame/publication identity 上增加字段。具体字段名和承载位置留给施工文档按
当前 consumer 基线决定，不自动升级 AUI document、项目 action、RuntimePackage 或整套 report schema。

### 9.4 Feedback 时间

277 的 Hover/Pressed/Released/Activated 属于 presentation-time 状态，不使用 gameplay time scale，也不
依赖 fixed tick。PointerDown/Up 的离散状态在本轮立即生效；短过渡由后续普通 presentation frame 使用
既有 unscaled UI delta 推进。

不新增第二套按钮动画系统，不复用 Animator2D，也不使用输入回调中的阻塞 sleep。

## 10. Exactly-once、顺序与故障

### 10.1 Exactly-once

```text
每个规范化 input event 只进入一次 AuiInteractionSystem。
每个生成的 AuiAction 只进入一次 ProjectRuntimeSession action dispatch。
普通 fixed tick 不再携带已经即时提交的 action。
Rejected / Unhandled / Faulted 不重试。
```

### 10.2 顺序

同一 `RuntimeInputFrame` 内沿用 `AuiInteractionResult.actions` 的 vector 顺序。多个 input event 按
Native Editor 主线程观察顺序串行处理，不为 284 引入并行 action executor。

### 10.3 故障

ProjectRuntimeSession action dispatch 的 terminal fault 继续使该 runtime session fail closed。不得为了
保持按钮视觉“流畅”而吞掉项目 fault 或继续执行后续 action。

按钮 released/capture cleanup 仍需完成，避免 Editor 留下 stuck pressed 状态。

## 11. Report 与性能合同

284 不新增公共 schema version 或新的常驻 report family。优先复用：

```text
AuiInteractionResult
Aui control feedback result/report
ProjectRuntimeSessionStageReport(AuiActionDispatch)
GameViewPresentReport / compact input route facts
```

普通热路径要求：

```text
report level Off/Summary 时不写盘
不序列化完整 document/layout/world
不因 pointer move 生成长 action trace
不运行完整 GameView frame 只为了更新 report
```

Trace 只在测试或显式诊断开启。

建议保留的最小可审查事实：

```text
input route kind
input event count / consumed count
pressed or activated node identity（Summary 可压缩）
action count / action ids
action dispatch status
runtime_advanced=false
fixed_update_count=0
redraw_requested
```

如现有 report 无法表达其中某项，施工文档必须先证明它是验收所需，再做最小字段调整；不得仅为“完整”
升级整套 schema。

第 9.3 节所需的 publication content identity 属于正确发布所必需的内部内容身份，不等于新增 report
family。若它必须穿过现有序列化 `GameViewRuntimeFrame`，只允许对该窄合同做兼容性评估和最小演进；不得
借此版本化无关 schema。

## 12. 明确非目标

284 不实施：

```text
独立输入线程
输入回调内独立 GPU Present
RuntimeRenderer 或 RenderGraph 重构
新的 UI renderer primitive
新的 AUI document/schema/migration
新的项目 action schema
通用 event bus / signal system
完整 FrameLoop 拆分
catch-up policy 重写
physics / combat / Animator2D 改为 variable update
Tower 插值或移动算法调整
production Editor 替换
Tower Preview cache 重建
Local CI 或完整视觉矩阵
```

如果 284 完成后普通帧仍因最多 8 个 catch-up tick 长时间占用主线程，再以新的测量证据讨论 catch-up
调度；它不是 284 的前置条件。

## 13. 最小改动面预期

正式施工文档生成前需按当前代码复核，预期 owner 范围为：

```text
rust/crates/editor_window_winit/src/application.rs
  输入入口从完整 tick 改为普通帧 immediate AUI route

rust/crates/editor_core/src/session.rs
  active GameView 普通帧 AUI 委托与 publication 同步

rust/crates/editor_core/src/editor_gameview_play.rs
  stable AUI present cache 与 immediate interaction/feedback/action path

rust/crates/editor_window_winit/src/editor_frame_publication.rs
以及必要时 rust/crates/editor_window_winit/src/real_window.rs
  让 last-good reuse 区分 runtime frame identity 与 AUI-only presentation identity

rust/crates/engine_runtime/src/project_runtime_session.rs
或 rust/crates/engine_runtime/src/frame_loop.rs
  窄复用现有 action dispatch + mutation commit owner
```

测试只放在能捕获本缺陷的 owner/consumer seam。不得因为修改跨 crate 就机械运行 production replacement、
完整 workspace、Local CI、完整 Tower 视觉矩阵或 282 qualification。

## 14. 验收合同

### 14.1 Owner 行为

1. PointerDown 输入后、任何 fixed update 发生前，目标按钮已进入 Pressed feedback state。
2. PointerUp inside 后、任何 fixed update 发生前，Click/Activated 与对应 `AuiActionDispatch` 已执行。
3. 即时 path 的 `runtime_advanced=false`，RuntimeTime、fixed-step count、物理、战斗 tick、敌人位置和
   Animator2D fixed progression 均不变化。
4. action mutation 使用既有 prepare/commit，Rejected/Unhandled/Faulted/exactly-once 语义不变。
5. action 后 observation/binding 可在本轮 AUI publication 看到新值；纯 hover/pressed 不做全量 observation。
6. stale layout/presentation/session identity fail closed，不回退到同步完整 GameView tick。
7. 未被 AUI 消费的 gameplay input 在最近一次正常 runtime tick 消费一次，不丢失、不重复、不无界积压。
8. AUI-only 可见变化不增加 runtime `frame_index`、不改变 world `frame_hash`，但会产生新的 ordinary
   presentation publication；相同 world+AUI 内容的普通 redraw 仍复用 last-good。
9. action 产生的 Animator2D 离散命令被既有 module 接收一次，但即时阶段不调用 Animator2D `tick` 或推进
   clip time。

### 14.2 Consumer 行为

至少用一个通用 fixture 证明：

```text
PointerDown -> pressed before fixed
PointerUp -> one action before fixed
ordinary redraw publication updated
same runtime frame identity publishes changed AUI composition once
unchanged world+AUI redraw reuses last-good
next fixed tick does not replay the action
unconsumed gameplay input remains available to the next normal runtime tick
```

Tower consumer 只需证明一个真实按钮，例如 `td.start-round` 或 `td.recruit`：

```text
click 后 project action state 已提交
同一即时阶段 fixed tick 未增加
随后 fixed tick 才开始连续战斗推进
```

### 14.3 性能行为

建立一个可红的定向计时/调用计数 seam，至少证明单次 pointer 输入不再进入：

```text
EngineHostLoop::tick full frame
ProjectRuntimeSession::fixed_update
ProjectLogicRunner fixed phases
physics
world render extraction
Animator2D fixed progression/observation scan
```

性能验收以调用边界和相对耗时为主，不写死依赖机器负载的绝对毫秒值。真实普通 Editor smoke 只在后续
施工授权明确包含 production composition 时执行。

## 15. 与既有方案的关系

```text
213：继续拥有 AUI interaction、input consumption、AuiAction mapping。
220：继续拥有 Editor GameView focus、target-space input routing。
260：继续拥有 ProjectRuntimeSession AUI action dispatch、deferred mutation、exactly-once。
273：继续拥有 GameView target/presentation/AUI coordinate consistency。
277：继续拥有当帧 Pressed/Activated feedback 和 presentation-time profile。
280：继续拥有普通 Editor project-level GameView target。
283：继续保留 report hotpath 不逐帧写盘的性能修复。
284：只补普通 Editor GameView 没有按普通帧兑现上述能力的阶段耦合缺口。
```

284 不推翻 260 的 fixed-step lifecycle。它只把 260 中 `handle_aui_actions` 从“必须附着于完整 advancing
frame”收敛为一个可在普通帧执行的离散 action stage；`fixed_update` 和连续模拟仍保持原合同。

## 16. 方案自审

### 16.1 需求一致性

```text
按钮反馈不等待 fixed tick：满足。
业务 action 不等待 fixed tick：满足。
战斗/物理/怪物移动留在 fixed tick：满足。
不做输入回调内独立 GPU Present：满足。
不重构整个 renderer：满足。
```

### 16.2 架构边界

```text
AUI interaction、feedback、action、binding、renderer owner 未混合。
Tower 玩法语义保持项目侧。
项目 mutation 继续 deferred preflight/commit。
fixed simulation 与 ordinary UI event 明确分离。
```

### 16.3 过量施工审计

```text
确认失败：pointer 输入同步触发完整 GameView/runtime tick，导致按钮反馈和 action 被完整帧成本拖延。
最小因果修复：输入入口改走普通帧 immediate AUI interaction + action dispatch，不调用完整 tick。
必须涉及：Editor 输入入口、active GameView instance、现有 action-dispatch helper 可复用性，以及让
          AUI-only 变化不被 last-good 错误吞掉的最小 publication identity 修正。
能够先红后绿的证据：pointer 输入前后 fixed/runtime/render phase 调用计数与 action/feedback 状态。
明确延期：catch-up 重写、renderer 重构、独立 Present、插值、公共 schema/report 扩张。
```

没有理由新增 AUI action queue、公共 schema version、migration、事务协议、第二套 feedback owner、
第二套 mutation commit、独立 GPU present 或完整 FrameLoop 架构重写。为保留未消费 gameplay input 所需的
active-instance 私有有界 pending state 不得扩成公共队列协议。

### 16.4 风险

主要风险只有五个：

1. 复用过期 layout/presentation 导致错误命中；由第 8 节 identity fail-closed 约束。
2. 即时 action 与下一 fixed tick 重放；由 exactly-once 测试和输入不再附着完整 tick 约束。
3. action 后 UI observation 未刷新，业务已执行但文本晚一帧；由 dirty active-binding refresh 验收约束。
4. AUI 状态已变但 publication 仍复用旧纹理；由独立 ordinary-presentation identity 和 AUI-only republish
   测试约束。
5. 即时 action 产生的 Animator2DCommand 被丢弃或提前推进动画时间；由 command exactly-once 与
   no-Animator2D-tick 验收约束。

以上风险都能在 owner seam 定向验证，不构成扩大施工范围的理由。

## 17. 施工结果

284 已完成并归档：普通 GameView input 进入 normal-frame immediate AUI stage，未消费 gameplay input
在下一 normal tick 消费一次；AUI 可见 composition 使用独立 presentation identity，same-runtime-frame
AUI-only 更新不会被 last-good reuse 吞掉。Tower `td.recruit` consumer 已证明 action-before-fixed 与 no-replay。

完成记录：

```text
阶段完成记录/2026-08-13-Editor-GameView-Normal-Frame-Immediate-AUI-Input-Dispatch-v1/00-总览.md
```

本次只形成 source-level 证据；未更新 production Editor，未运行 Local CI、真实配置、Tower Preview cache
或完整视觉矩阵。
