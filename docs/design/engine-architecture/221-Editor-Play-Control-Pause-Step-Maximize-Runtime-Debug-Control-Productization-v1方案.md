# 221-Editor Play Control / Pause-Step-Maximize / Runtime Debug Control Productization v1 方案

## 1. 一句话说明

本系统把已经能在 Editor GameView 中运行、显示和接收输入的 runtime 实例，补齐成真正可调试的 Play 控制面：

```text
Play
  -> 使用 217 Preview RuntimePackage 真相
  -> 启动 218 EditorRuntimePlayInstance
  -> 219 显示真实 GameView GPU texture
  -> 220 接收 GameView 输入
  -> 221 支持 Pause / Resume / StepFrame / Stop / Maximize on Play
```

它对标 Unity 的 Game View `Play / Pause / Step / Play Maximized`，也对标 UE PIE 的 PlaySession state machine 和 Godot GameView debugger 的 pause / next frame 控制。

本系统不是新建一个 Debugger 层，也不是给 Runtime 增加一套新主循环；它只在现有 `PlaySessionController + EditorRuntimePlayInstance + Editor GameView` 链路上补齐运行控制状态和可审查报告。

## 2. 背景与问题

当前 217-220 已完成的链路：

```text
217-Editor Play / RuntimePackage Preview:
  Play 前准备 Preview RuntimePackage cache，避免每次 Play 都全量构建。

218-Editor In-process GameView Play Runner:
  Editor 进程内创建 EditorRuntimePlayInstance，并从 RuntimePackage load / hydrate / tick。

219-Editor GameView Full GPU Texture Sharing:
  Runtime RHI plan 渲染到 Editor shared texture，再由 Editor UI renderer 采样到 GameView。

220-Editor GameView Input / Focus / AUI RoutedDispatch:
  GameView-local RuntimeInputFrame 进入 active runtime，AUI consumed 后剩余输入进入 gameplay InputResolver。
```

但现在 Play 体验仍有明显缺口：

```text
Pause 当前基本是 Stop 语义，会销毁 EditorRuntimePlayInstance。
StepFrame 当前走旧 runtime_package tick_one_frame，不是 active GameView runtime 的单帧推进。
GameView 有运行画面和输入，但用户不能暂停后观察复杂打飞机中的子弹、敌人、HUD 和输入响应。
Maximize on Play 还只是 Unity-like 目标，没有进入 Editor layout / view model。
```

对复杂打飞机项目来说，这一块非常直接：

```text
运行中暂停，检查玩家 / 子弹 / 敌人 / HUD 状态。
暂停后单帧推进，观察碰撞、生成、输入、AUI 点击是否按预期发生。
Play 时最大化 GameView，让用户像 Unity 一样专注验证运行效果。
```

## 3. 成熟引擎源码参考

### 3.1 Unity

Unity 对用户暴露的是少量稳定 Play 控制：

```text
EditorApplication.isPlaying
EditorApplication.isPaused
EditorApplication.Step()
GameView maximizeOnPlay / Play Maximized
```

源码 / 文档参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\EditorApplication.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\EditorApplication.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\PlayModeView\PlayModeView.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\GameView\GameView.cs
框架设计/Unity源码参考/EditorPlayMode-GameView-BuildAndRun源码参考.md
https://docs.unity3d.com/Manual/GameView.html
https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/GameView/GameView.cs
```

可学习点：

```text
GameView 是 Play 输出、焦点、尺寸和最大化视图，不是 PlaySession 编排核心。
Pause / Step 是 PlayMode 控制，而不是普通 UI 暂停按钮。
用户心智要少：Play、Pause、Step、Stop、Maximize 足够。
```

不可照搬点：

```text
Unity 的 PlayMode 依赖 C++ engine/editor 深度一体化，内部细节很多不可见。
Domain Reload / Scene Reload / Script Recompile During Play 不进入本轮。
```

### 3.2 Unreal Engine

UE 的 PIE 不是 UI 按钮直接创建 World，而是：

```text
FRequestPlaySessionParams
  -> GUnrealEd->RequestPlaySession(...)
  -> 下一帧 StartQueuedPlaySessionRequest()
  -> StartPlayInEditorSession / StartPlayInNewProcessSession
  -> RequestEndPlayMap / EndPlayMap
```

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Public\PlayInEditorDataTypes.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\PlayLevel.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\PlayLevelNewProcess.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\Kismet2\DebuggerCommands.cpp
框架设计/UE源码参考/EditorPlaySession-PIE-Standalone源码参考.md
```

可学习点：

```text
UI 只发 PlaySessionRequest / StopRequest / debug command。
Request 和运行期 SessionState 分开。
Start / Stop / Pause / Step 应在稳定同步点处理，避免 UI 回调中直接破坏 runtime 状态。
自动化测试应直接调用 Session API，不依赖真实 UI 点击。
```

不可照搬点：

```text
UE 的 PIE World 复制、GWorld 切换、Blueprint debugger references 对本项目当前阶段过重。
多人 PIE、VR Preview、网络客户端、多窗口不进入本轮。
```

### 3.3 Godot

Godot 默认以外部进程运行项目，但 GameView / debugger 可作为运行会话消费者：

```text
EditorRunBar
  -> EditorRun
  -> Runtime process
  -> EditorDebuggerNode / GameView / EmbeddedProcess
  -> pause / next_frame / speed / screenshot 等调试消息
```

源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\editor\run\editor_run_bar.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\run\editor_run.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\run\game_view_plugin.cpp
框架设计/Godot源码参考/11-EditorRun-GameView-PlaySession源码参考.md
```

可学习点：

```text
RunBar / PlaySessionController 负责会话编排。
GameView 只订阅运行状态并提供显示 / 调试控制。
Pause / next frame 是运行会话控制命令，不是编辑器 UI 本地状态。
```

### 3.4 Bevy

Bevy 没有完整传统编辑器 PlaySession，但 Runner 模型值得参考：

```text
App
  -> runner
  -> ScheduleRunner / WinitRunner
```

源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app\src\app.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app\src\schedule_runner.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_winit\src\state.rs
框架设计/Bevy源码参考/14-AppRunner-Winit-ScheduleRunner-RunSession源码参考.md
```

可学习点：

```text
Runtime 内容和 Runner 分离。
Windowed runner 与 headless runner 可以运行同一个 app / package。
Pause / Step 应该控制 runner 是否推进 update，而不是让项目逻辑自己猜测编辑器状态。
```

## 4. 本项目当前代码基线

当前关键代码：

```text
rust/crates/editor_core/src/editor_gameview_play.rs
  EditorRuntimePlayInstance
  tick_next_descriptor_frame
  tick_next_descriptor_frame_with_runtime_input
  apply_gpu_present_result
  stop

rust/crates/editor_core/src/services/play_service.rs
  start_play_session
  stop_play_session

rust/crates/editor_core/src/session.rs
  UiCommandPayload::Play -> start_play_session
  UiCommandPayload::Pause -> stop_play_session
  UiCommandPayload::StepFrame -> tick_one_frame
  tick_active_game_view_runtime_descriptor_frame
  tick_active_game_view_runtime_descriptor_frame_with_input

rust/crates/editor_core/src/services/runtime_service.rs
  tick_one_frame

rust/crates/editor_core/src/editor_command_registry.rs
  play / pause / step_frame / tick_one_frame command descriptor

rust/crates/editor_window_winit/src/command_system.rs
  play / pause / step_frame / tick_one_frame command registration
```

当前问题：

```text
Pause 命令和 Stop 混在一起。
StepFrame 没有优先控制 active EditorRuntimePlayInstance。
EditorRuntimePlayInstance 有 state 字段，但没有产品化为可观察、可控制的 PlayControlState。
GameViewPresentReport 可以证明帧推进和输入，但还不能证明 paused / resumed / stepped / maximized。
```

已有规则基础：

```text
17-Runtime-FrameLoop.md 已定义 Editor Pause / Editor Step：
  Pause: EditorFrame + last runtime snapshot present，不推进 RuntimeFrame。
  Step: RuntimeFrame once，再 RenderExtract / RenderFrame / Present。

217-220 已明确 Pause / Step / Maximize on Play deferred，221 正是收敛这个 deferred。
```

## 5. 方案选型

### 5.1 方案 A：Pause / Step A-min

内容：

```text
Pause 不再 Stop。
StepFrame 推进 active EditorRuntimePlayInstance 一帧。
Resume 通过 Play 命令恢复。
不做 Maximize on Play。
```

优点：

```text
最快。
改动最少。
```

问题：

```text
用户体验仍不像 Unity。
复杂打飞机验证时 GameView 仍可能太小，不方便观察。
Stop 语义仍需要补清楚。
```

结论：

```text
可作为极小切片，但不推荐作为正式 221。
```

### 5.2 方案 B：Unity-like B-min，当前采用

内容：

```text
新增或产品化 EditorPlayControlState：
  Idle
  Preparing
  Running
  Paused
  Stepping
  Stopping
  Stopped
  Failed

Play:
  无 active runtime 时启动 Preview RuntimePackage + EditorRuntimePlayInstance。
  Paused 时作为 Resume。

Pause:
  active runtime 从 Running 进入 Paused。
  保留 EditorRuntimePlayInstance、World、AUI interaction state、last frame、last RHI plan。
  不再销毁 runtime instance。

StepFrame:
  Paused 时推进 active GameView runtime exactly one frame。
  Step 完成后回到 Paused。
  Running 时可返回结构化 diagnostic，提示先 Pause。

Stop:
  作为独立 StopPlaySession / Stop 命令语义。
  销毁 EditorRuntimePlayInstance，写 stopped report。
  兼容旧路径，但新语义中 Pause 不再承担 Stop。

Maximize on Play:
  只改变 Editor layout / GameView view model。
  不改变 RuntimePackage、RuntimeRenderer、RHI、AUI、InputResolver。
  Stop 后按策略恢复上一视图布局。
```

优点：

```text
用户心智接近 Unity。
直接改善复杂打飞机调试体验。
AI 能通过结构化 command / state / report 判断当前是否在 Running / Paused / Stepping。
不新增架构层，只补现有 PlaySession / GameView state。
```

风险：

```text
需要梳理现有 Pause=Stop 兼容行为。
真实 winit RedrawRequested 的自动 tick 必须尊重 Paused，不然暂停会被下一帧悄悄推进。
Maximize on Play 不能变成 OS fullscreen 或新 GameView 窗口，本轮只做 editor view model / layout intent。
```

结论：

```text
采用方案 B-min。
```

### 5.3 方案 C：Full Runtime Debug Control

内容：

```text
time scale
slow motion
frame timeline
breakpoint
input recording / replay
runtime state inspector
multi-instance debug
remote process debugger
```

优点：

```text
长期调试能力强。
```

问题：

```text
范围过大。
容易把 Play 控制、trace replay、debugger protocol、inspector、hot reload 安全点混在一起。
会拖慢复杂打飞机主线。
```

结论：

```text
作为长期方向 deferred，不进入 221。
```

## 6. 最终方案：B-min

221 的正式目标：

```text
Editor GameView Play Control B-min
  = Pause / Resume / StepFrame / Stop / Maximize on Play
  + compact report
  + deterministic tests
  + 不新增 Debugger 大系统
```

核心原则：

```text
Pause 不是 Stop。
StepFrame 推进 active GameView runtime，不走旧 RuntimePackage debug tick。
Maximize on Play 是 Editor 视图状态，不是 Runtime 能力。
RuntimePackage 仍是 Play 真相。
GameView 仍是运行输出和输入面，不负责构建和会话编排。
Runtime 默认 Off 或 compact result；Editor 默认可提供 Summary 给 Report Panel，不让 runtime 热路径承担 Trace 成本。
```

## 7. 数据与状态设计

### 7.1 EditorPlayControlState

建议产品化为可序列化 / 可报告枚举：

```text
EditorPlayControlState:
  Idle
  Preparing
  Running
  Paused
  Stepping
  Stopping
  Stopped
  Failed
```

说明：

```text
Idle:
  没有 active Play session。

Preparing:
  正在准备 Preview RuntimePackage / queued start。

Running:
  active EditorRuntimePlayInstance 可以随 GameView redraw / input tick 推进 runtime。

Paused:
  active EditorRuntimePlayInstance 保留，但自动 tick 被门控；GameView 显示 last runtime frame。

Stepping:
  transient 状态，只在 StepFrame 命令处理中出现；推进一帧后回到 Paused。

Stopping:
  正在执行 stop request。

Stopped:
  已停止并释放 active runtime instance。

Failed:
  启动或控制失败。
```

这不是新的架构层，只是现有 PlaySession / EditorRuntimePlayInstance 的显式状态。

### 7.2 EditorRuntimePlayInstance 控制 API

建议 API：

```text
pause() -> GameViewPresentReport
resume() -> GameViewPresentReport
step_next_frame(input: Option<RuntimeInputFrame>) -> GameViewPresentReport
stop() -> GameViewPresentReport
control_state() -> EditorPlayControlState
```

tick 门控：

```text
Running:
  tick_next_descriptor_frame / tick_next_descriptor_frame_with_runtime_input 正常推进 EngineHostLoop。

Paused:
  普通 tick request 不推进 EngineHostLoop。
  返回 last_frame + paused_last_frame_reused diagnostic / summary。

Stepping:
  只允许 StepFrame 命令推进一次 EngineHostLoop。
  完成后恢复 Paused。
```

### 7.3 EditorSession 命令语义

命令语义收敛：

```text
Play:
  Idle / Stopped / Failed:
    start_play_session。
  Paused:
    resume_active_gameview_play_session。
  Running:
    返回 already_running summary，或按 UI toggle 策略请求 Stop；本轮建议不要隐式 Stop。

Pause:
  Running:
    pause_active_gameview_play_session。
  Paused:
    返回 already_paused summary。
  Idle:
    返回 no_active_play_session diagnostic。

StepFrame:
  Paused:
    step_active_gameview_play_session_one_frame。
  Running:
    返回 pause_before_step diagnostic。
  Idle:
    如果存在旧 RuntimePackage debug world，可继续保留 tick_one_frame debug path；但 GameView Play 优先。

Stop:
  Running / Paused:
    stop_play_session。
  Idle:
    返回 already_stopped summary。
```

兼容策略：

```text
现有 Pause=Stop 行为必须废弃为旧语义。
如果 UI 当前没有 Stop command，221 必须补 StopPlaySession / stop_play_session command descriptor。
不要继续让 Pause 销毁 EditorRuntimePlayInstance。
StepFrame 必须在 report 中写明 target_runtime_domain：
  active_gameview_runtime
  opened_runtime_package_debug
  none
不能在 active GameView runtime 存在时静默落到旧 tick_one_frame debug path。
```

### 7.4 Maximize on Play

Maximize on Play 只属于 Editor view model：

```text
EditorGameViewLayoutState:
  maximize_on_play: bool
  is_game_view_maximized: bool
  restore_workspace_region: Option<String>
  reason: play_started | user_toggle | stop_restore
```

规则：

```text
Play started + maximize_on_play=true:
  GameView 占据主要 workspace 区域。

Stop:
  如果本次是 Play 自动最大化，则恢复上一个 workspace layout。

用户手动切换最大化:
  记录 reason=user_toggle，不要被 Stop 强行覆盖。

Headless / tests:
  不需要真实 OS window maximize，只要 ViewModel / report 能证明 layout intent。
```

命令入口：

```text
221 必须提供 SetGameViewMaximizeOnPlay / ToggleGameViewMaximizeOnPlay 等价命令，或在已有设置命令中显式暴露 maximize_on_play 字段。
不能只在内部默认开启，必须让用户和 AI 都能读写该偏好。
```

本轮不做：

```text
OS fullscreen。
外部 GameView 窗口。
多显示器 GameView。
多 GameView simultaneous play。
```

## 8. Report / Trace 档位

必须遵守当前 skill 规则：所有 report / trace 区分 runtime 和 editor，并分 Off / Summary / Trace。

### 8.1 Runtime 档位

```text
RuntimeReportMode::Off:
  默认。不写完整 trace，不分配长 JSON，不记录完整 frame timeline。

RuntimeReportMode::Summary:
  只返回 compact result，例如 control_state、frame_count、last_frame_hash。

RuntimeReportMode::Trace:
  只用于 gate / debug / 用户显式诊断。可输出 play control transition trace。
```

### 8.2 Editor 档位

```text
EditorReportMode::Summary:
  Report Panel 可展示当前 Play control summary。

EditorReportMode::Trace:
  自动化 gate / AI 审查 / 用户显式诊断时开启。
  输出 PlayControlTransitionReport。
```

### 8.3 建议报告字段

建议扩展或新增 report：

```text
EditorPlayControlReport:
  schema_version
  session_id
  control_state_before
  control_state_after
  command
  frame_count_before
  frame_count_after
  runtime_advanced
  paused_last_frame_reused
  step_count
  target_runtime_domain
  maximize_on_play
  is_game_view_maximized
  stop_released_runtime_instance
  report_mode
  diagnostics
  next_actions
```

也可以先扩展 `GameViewPresentReport`：

```text
control_state
control_command
runtime_advanced
paused_last_frame_reused
maximize_status
```

推荐施工取舍：

```text
B-min 可以优先扩展 GameViewPresentReport，避免新增过多 report 类型。
如果扩展后字段过杂，再提取 EditorPlayControlReport。
```

## 9. AI 适配性

221 必须让 AI 能稳定判断：

```text
现在有没有 active runtime instance。
当前是 Running 还是 Paused。
Pause 是否真的没有推进 frame_count。
StepFrame 是否只推进了一帧。
Stop 是否释放了 runtime instance。
Maximize on Play 是否只是 editor layout 状态，不影响 runtime。
```

AI patch / report 需要结构化证据：

```text
before_state / after_state
before_frame_count / after_frame_count
runtime_advanced: true | false
reason / diagnostic_code
next_action
```

避免：

```text
AI 通过自然语言 console 猜测状态。
Pause 命令既可能 Pause 又可能 Stop。
StepFrame 有时 tick debug runtime，有时 tick GameView runtime，但报告不说明。
Maximize on Play 被误认为 RuntimeRenderer / GPU resource 改动。
```

## 10. 复杂项目适配性

对复杂打飞机：

```text
暂停时可以检查 HUD、输入、子弹生成、敌人波次、碰撞结果。
单帧推进可验证 bullet spawn / movement / hit / score update 是否按帧发生。
Maximize on Play 可让用户在编辑器内更舒服地反复验证。
Report 可让 AI 复盘“为什么点击后没有射击”：是 Paused、AUI consumed、还是 InputResolver 没产生 action。
```

对后续复杂项目：

```text
自走棋可在战斗回合中暂停检查棋子状态和 UI。
复杂装备 UI 可在 Paused 状态下验证 drag/drop 是否只改变 UI transient state 或是否触发项目交易。
多系统项目可以通过 StepFrame 缩小 bug 发生帧范围。
```

## 11. 效率边界

运行效率：

```text
Running:
  与 220 当前 tick 路径一致。

Paused:
  不推进 EngineHostLoop，不重新做 gameplay update。
  可以复用 last runtime frame / last RHI plan / last GameView texture。

StepFrame:
  每次只推进一帧，不启动新 RuntimePackage，不重建 PreviewPackage。
```

编辑器效率：

```text
Maximize on Play 只影响 layout / view model，不触发 RuntimePackage rebuild。
Pause / Resume 不应重新 hydrate world。
Stop 才释放 active runtime instance。
```

Report 成本：

```text
Runtime 默认 Off 或仅返回功能必需 compact result；Editor / Report Panel 可默认展示 Summary。
Trace 只在 gate / debug / 用户显式诊断开启。
```

## 12. Deferred 边界

不进入 221 B-min：

```text
time scale / slow motion。
breakpoint / watch expression。
frame timeline UI。
input recording / replay。
runtime state inspector。
hot reload safe pause 完整流程。
multi-instance play。
external process pause protocol。
remote device play control。
OS fullscreen / detached GameView window。
multi-display GameView。
```

注意：

```text
17-Runtime-FrameLoop.md 中的 SafePause 可作为长期热更新安全点概念。
221 B-min 只做 Editor Play pause control，不声明完整热更新 SafePause 已实现。
```

## 13. 对 38-220 审查文档的判断

用户指定的审查文档：

```text
其它AI审查目录/38-220-Editor-GameView-Input-Focus-AUI-HitCandidate-RoutedDispatch方案审查.md
```

审查对象是 220，不是 221。判断如下：

```text
适用性:
  该审查是 220 的前置系统审查，不是 221 的直接方案审查。

已由历史施工吸收:
  220 已完成并归档，完成记录明确：
    复用 AuiInteractionSystem::process_with_state。
    不新增平行 AUI router。
    AUI consumed 后通过 RuntimeInputFrame::filter_consumed_events 过滤。
    input_bridge_status 从 deferred 变为真实状态。
    WorldPickCollector / EditorOverlayCollector / wheel/text/IME/gamepad bridge / 完整 HitTraceReport 仍 deferred。

对 221 的施工约束:
  221 不重新打开 220 的 HitCandidate/RoutedDispatch 实现。
  221 必须继承 220 的输入链路，只控制 active EditorRuntimePlayInstance 是否推进 tick。
  221 report 继续遵守 runtime/editor 分档，runtime 默认 Off 或 compact result，Trace 只用于 gate/debug/显式诊断。
  221 测试继续优先 headless deterministic gate，真实 OS window / GPU smoke 只能 optional 或 ignored local-only。

不适用项:
  220 审查中的坐标转换、AUI hit-test、AUI consumed filter 等问题已属于 220 完成范围，不作为 221 再施工内容。
```

因此 38-220 审查文档不要求推翻或大改 221 方案，只要求 221 施工文档显式继承上述约束。

## 14. 施工建议 Gate

后续进入施工时建议拆成以下 Gate：

```text
Gate A: 文档审查与现状锁定
  读取其它 AI 审查文档。
  确认 217-220 当前实现边界。
  读取 38-220 审查并声明：它已由 220 施工吸收，221 只继承输入链路/report/headless 约束。

Gate B: PlayControlState / report 字段
  产品化 EditorPlayControlState。
  扩展 GameViewPresentReport 或新增 EditorPlayControlReport。
  单测覆盖状态序列化 / summary。

Gate C: EditorRuntimePlayInstance pause / resume / step
  Pause 不推进 frame_count。
  StepFrame exactly +1 frame。
  Resume 后继续正常 tick。

Gate D: EditorSession command 语义
  Play 在 Paused 时 Resume。
  Pause 不再 Stop。
  StepFrame 优先控制 active GameView runtime。
  新增 StopPlaySession / Stop command descriptor，并让 Pause 不再复用 stop_play_session。
  StepFrame report 必须写明 target_runtime_domain。

Gate E: Maximize on Play view model
  新增 GameView layout state。
  新增 SetGameViewMaximizeOnPlay / ToggleGameViewMaximizeOnPlay 等价命令或设置字段。
  Play 时根据 maximize_on_play 更新 view model。
  Stop 后按策略恢复。

Gate F: Report Panel / project_e2e_gate
  Report Panel 展示 PlayControl summary。
  project_e2e_gate 验证 Play -> Pause -> Step -> Resume -> Stop。
  验证 paused 不推进 frame，step 只推进一帧。

Gate G: 整体回归与文档同步
  cargo fmt --check。
  editor_core / editor_window_winit / project_e2e_gate 相关测试。
  更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
```

## 15. 自审

本方案满足当前项目规则：

```text
没有新增 Runtime 架构层。
没有把 GameView 变成 PlaySession 编排核心。
没有改变 RuntimePackage 真相。
没有把 Maximize on Play 混入 RuntimeRenderer / RHI。
没有让 Pause 继续伪装成 Stop。
没有默认打开 runtime 热路径 trace。
没有让 StepFrame 在 GameView runtime 和旧 debug runtime 之间静默切换。
保留 Unity-like 用户心智，同时给 AI 留出结构化 state / report。
```

当前结论：

```text
221 采用方案 B-min：
  Editor Play Control / Pause-Step-Maximize / Runtime Debug Control Productization v1

它是 218 A4 的产品化收敛：
  217 解决 PreviewPackage。
  218 解决 in-process GameView runtime。
  219 解决 GPU texture present。
  220 解决 GameView input。
  221 解决 Play control。
```
