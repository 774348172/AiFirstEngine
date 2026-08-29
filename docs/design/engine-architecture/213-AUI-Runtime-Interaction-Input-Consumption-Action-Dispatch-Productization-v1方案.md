# 213-AUI Runtime Interaction / Input Consumption / Action Dispatch Productization v1 方案

## 1. 这个系统是干什么的

一句话：

```text
把已经存在的 AUI hit test / command / trace core 接到真实运行时输入链路上，让导出后的游戏里 AUI 按钮和 drag/drop 能真正消费输入、生成 AuiAction，并把证据写进 report。
```

它解决的不是 AUI 显示问题。AUI 显示链路已经由 190 / 199 / 208 / 209 / 210 / 211 推进到：

```text
AUI Document
  -> RuntimePackage
  -> ProjectUiStateSnapshot
  -> Binding Resolve
  -> AuiLayout / AuiDrawList
  -> AuiCompositionFrame
  -> RuntimeRenderer UI composition pass
  -> Present
```

本系统补的是下一段：

```text
RuntimeInputFrame
  -> AUI hit test / interaction
  -> AuiCommand / AuiAction / drag-drop payload
  -> consumed pointer input 不再进入 gameplay InputResolver
  -> AUI interaction report / native player report / e2e report
```

简单说：HUD 已经能显示，现在要让 HUD 可以被玩家点、拖、放，并且不会点 UI 的同时触发开火。

## 2. 其它引擎对标

### 2.1 Unity UGUI

对标：

```text
StandaloneInputModule
  -> ProcessMouseEvent
  -> ProcessMousePress
  -> ExecuteEvents
  -> Button.OnPointerClick / ScrollRect.OnBeginDrag / OnDrag / OnEndDrag
```

源码参考：

```text
<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\EventSystem\InputModules\StandaloneInputModule.cs
  Process / ProcessMouseEvent / ProcessMousePress

<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\Button.cs
  OnPointerClick

<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\ScrollRect.cs
  OnInitializePotentialDrag / OnBeginDrag / OnDrag / OnEndDrag / OnScroll
```

可学习点：

```text
输入先经过 UI EventSystem。
按钮 click 和 drag 是 UI framework 机制，不是 gameplay 自己扫鼠标。
控件接收 PointerEventData，业务层只关心点击或拖放结果。
```

不照搬：

```text
不照搬完整 EventSystem / StandaloneInputModule / Selectable 状态机。
不在第一版做 ScrollRect、InputField、IME、完整拖拽视觉反馈。
```

### 2.2 Unreal Slate / UMG

对标：

```text
FSlateApplication
  -> LocateWindowUnderMouse / WidgetPath
  -> RoutePointerDownEvent / RoutePointerUpEvent
  -> SWidget::OnMouseButtonDown
  -> FReply::Handled / Unhandled
  -> ProcessReply
```

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Private\Framework\Application\SlateApplication.cpp
  ProcessMouseButtonDownEvent
  RoutePointerDownEvent
  ProcessReply

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Public\Widgets\Input\SButton.h
  OnMouseButtonDown
```

可学习点：

```text
Widget 返回明确 reply。
Handled 后输入不继续泄漏。
drag / capture / focus 是 reply 的后续扩展，不必第一版全做。
```

不照搬：

```text
不新增 Slate 式运行时 Widget 对象层。
不做完整 tunnel / bubble routing。
不做完整 focus manager / navigation。
```

### 2.3 Godot Control UI

对标：

```text
Viewport::_gui_input_event
  -> gui_find_control
  -> Control::_call_gui_input
  -> mouse_filter STOP / PASS / IGNORE
  -> set_input_as_handled
```

源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\main\viewport.cpp
  _gui_input_event
  gui_find_control
  set_input_as_handled

<GODOT_SOURCE>\godot-master\godot-master\scene\gui\control.cpp
  _call_gui_input
  gui_input
  mouse_filter

<GODOT_SOURCE>\godot-master\godot-master\scene\gui\base_button.cpp
  BaseButton::gui_input
```

可学习点：

```text
STOP / PASS / IGNORE 很适合 AI 和用户理解。
UI 命中与输入消费必须可解释。
```

不照搬：

```text
不把 AUI Node 改成 Godot Control 对象。
不新增脚本式 _gui_input 给用户写。
```

### 2.4 Bevy Picking

对标：

```text
PointerLocation
  -> PointerHits
  -> HoverMap
  -> Pointer<Click / DragStart / Drag / DragDrop>
```

源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_picking\src\backend.rs
  PointerHits

<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_picking\src\hover.rs
  HoverMap / PreviousHoverMap

<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_picking\src\events.rs
  pointer_events
  Click / DragStart / Drag / DragDrop
```

可学习点：

```text
Hit production 和 event production 分开。
Click / drag/drop 可以是高层 pointer event。
多 backend 长期可扩展，但第一版可只做 AUI ScreenOverlay / Modal。
```

不照搬：

```text
不引入 Bevy ECS event pipeline。
不把 AUI Node 变成 ECS Entity。
```

## 3. 本项目当前基线

已存在：

```text
rust/crates/engine_runtime/src/aui.rs
  AuiInteractionEventKind: PointerDown / PointerUp / PointerMove
  AuiHitTestResult
  AuiCommandKind: PointerDown / PointerUp / PointerMove / Hover / Click
  AuiCommand
  AuiInteractionTrace
  AuiInteractionResult
  AuiInteractionSystem::hit_test
  AuiInteractionSystem::process
  AuiActionMapper

rust/crates/runtime_player_winit/src/lib.rs
  resolve_native_input_frame
  InputDeviceState -> RuntimeInputFrame
  InputResolver::resolve
```

当前断点：

```text
runtime_player_winit 仍然先把 RuntimeInputFrame 直接交给 InputResolver。
AuiInteractionSystem 没有接入真实 native player frame。
AUI consumed pointer input 没有从 gameplay input 中剔除。
AuiAction 没有进入 runtime player / e2e report。
DragStart / DragMove / Drop 还没有正式 command / action / trace。
AuiCommand.payload 当前没有真实 payload。
```

旧 103 已经完成 AUI Interaction C-min core；213 不是从零重做，而是做运行时产品化和 drag/drop C-min 扩展。

## 4. 方案对比

### 方案 A：只保留当前 core

做法：

```text
继续只在 engine_runtime::aui 单元测试里证明 hit test / click。
不接 runtime_player_winit。
```

优点：

```text
无施工风险。
```

缺点：

```text
导出后的游戏里 UI 点击仍不能证明有效。
UI 输入仍可能泄漏到 gameplay。
不能支撑复杂打飞机的暂停按钮、确认按钮，也不能支撑复杂装备 UI drag/drop。
```

结论：

```text
不选。
```

### 方案 B-min+：运行时接线 + drag/drop C-min

做法：

```text
RuntimeInputFrame
  -> AUI present/layout 使用当前 frame 可用 snapshot
  -> AuiInteractionSystem::process
  -> AuiActionMapper
  -> filter_consumed_pointer_events
  -> InputResolver::resolve(filtered_frame)
  -> EngineFrameInput / native report 携带 AUI interaction evidence
```

支持：

```text
Click
Hover
PointerDown / PointerUp / PointerMove
DragStart
DragMove
Drop
DragCancel / DragEnd report evidence
single primary pointer
pointer capture for active drag
drop target hit test
source_node / target_node payload
input consumed filtering
structured report
```

暂不支持：

```text
scroll
keyboard navigation
gamepad navigation
text input / IME
multi pointer drag
world-space UI raycast
bubble / tunnel event propagation
复杂拖拽视觉 ghost
项目业务交易执行
```

结论：

```text
选择。
```

### 方案 C：完整 UI interaction framework

做法：

```text
一次补齐 focus、capture、navigation、scroll、IME、InputField、ScrollView、drag/drop、bubble/tunnel、screen flow。
```

优点：

```text
长期能力完整。
```

缺点：

```text
范围过大。
AI 可审查面太宽。
当前复杂打飞机主线不需要一次补齐。
容易新增多层结构，违背当前文档收敛方向。
```

结论：

```text
后续长期方向，不作为本轮。
```

## 5. 正式选择

本轮采用：

```text
方案 B-min+：Runtime Productized AUI Interaction + DragDrop C-min
```

命名：

```text
AUI Runtime Interaction / Input Consumption / Action Dispatch Productization v1
```

定位：

```text
它是 AUI Runtime Core 的运行时输入产品化系统。
它不新增用户心智层。
它不让 IR 实现 drag/drop 状态机。
它不把具体玩法写进 engine core。
```

## 5.1 32 号审查采纳结论

`其它AI审查目录/32-213-AUI-Runtime-Interaction-Input-Consumption-Action-Dispatch方案审查.md` 的结论为：213 方向正确，可以进入施工，但施工前必须补清以下规则。本方案采纳：

```text
Gate C 拆为 C1 / C2 / C3：
  C1 = RuntimeInputFrame::filter_consumed_pointer_events 克隆重建 filtered frame。
  C2 = runtime loop 从 input -> present 调整为 present/layout -> interaction -> filtered input -> InputResolver。
  C3 = EngineFrameInput.aui_interaction 与 native/player/e2e report 字段位。

AuiNode 显式新增：
  draggable: bool = false
  drop_target: bool = false

Runtime drag/drop payload 由 AuiInteractionSystem 运行时生成，不读取 authoring payload。
editor_core::services::aui_service 中 SetAuiActionRef.payload 当前仍是 deferred，本轮只在 report 中标注 authoring_action_payload_deferred。

Modal 第一版只保证 hit priority 高于 ScreenOverlay。
Modal 全屏输入阻挡 / focus trap deferred，并在 report 中标注 modal_input_blocking_deferred。

Interaction 第一版使用当前可用 AUI present snapshot；接受 snapshot_frame_lag = 1，并在 report 中显式输出。

drag_threshold_px 必须是配置/default 字段，不写成隐藏 magic constant。
只有 primary/left pointer 可以触发 drag。
DragStart 之后 click 与 drag 互斥。
drag source 松到空白区生成 DragCancel。
```

## 6. 正式运行顺序

第一版运行顺序固定为：

```text
Raw OS input
  -> InputDeviceState
  -> RuntimeInputFrame
  -> AUI present/layout for hit test
  -> AuiInteractionSystem::process
  -> AuiInteractionResult
  -> RuntimeInputFrame::filter_consumed_pointer_events
  -> InputResolver::resolve(filtered_frame)
  -> EngineFrameInput {
       action_snapshot,
       input_trace_summary,
       aui_overlay,
       aui_composition,
       aui_interaction
    }
  -> Project Logic / Runtime frame
  -> Render / Report
```

规则：

```text
AUI pointer input 优先于 gameplay InputMapping。
只有命中 interactable 且 consume_input=true 的节点才能消费 pointer input。
active drag 期间，captured pointer 的 move/up 必须继续被 AUI 消费。
未命中 AUI、或命中 non-interactable、或 consume_input=false 的输入必须继续进入 gameplay InputResolver。
AUI action 是业务级 UI 意图；AuiCommand 是指针级事件。
runtime_player_winit 当前 input->present 的实现必须调整为 present/layout->interaction->filter->input。
present 和 hit test 使用同一份 AUI present/layout 结果，避免同帧重复 layout。
filter 采用克隆重建 RuntimeInputFrame 的简单路径，不侵入 InputResolver。
filter 只移除被 AUI 消费的 pointer event，不移除 keyboard、wheel 或未消费 pointer。
```

## 7. Click / Button 范围

保留旧 103 的 click 规则：

```text
PointerDown 命中同一个 interactable node。
PointerUp 仍命中同一个 node。
该 node 有 Click action_ref。
生成 AuiCommand::Click。
再映射为 AuiAction { action_id, node_id, event=Click }。
```

第一版可证明：

```text
点击暂停 / 继续 / 确认按钮时生成 AuiAction。
点击 UI 按钮时 mouse/left 不再触发 gameplay fire。
```

不要求本轮实现具体暂停业务流程；项目侧如何响应 `ui.pause` 仍由 Project Logic / RuleSlot / Project Module 决定。

## 8. Drag/drop C-min 范围

### 8.1 新增最小概念

新增通用 UI 机制字段，不能出现装备、棋子、背包等项目语义：

```text
AuiNode:
  draggable: bool = false
  drop_target: bool = false
```

施工固定采用显式字段，不采用 `interactable + action_refs` 推导。旧 AUI document 缺字段时通过 serde default 读取为 `false`，保持 `aui-document.v1` 兼容，不为本轮单独升级 v2。

新增 command kind：

```text
AuiCommandKind:
  DragStart
  DragMove
  Drop
  DragCancel
```

新增 action event：

```text
AuiActionEvent:
  Click
  DragStart
  DragMove
  Drop
```

### 8.2 Drag 状态机

第一版只支持一个 primary pointer drag：

```text
PointerDown on draggable node
  -> capture source_node
  -> consumed if source.consume_input

PointerMove while captured
  -> if movement >= drag_threshold_px:
       emit DragStart once
  -> emit DragMove after started
  -> consumed

PointerUp while captured
  -> hit test current pointer
  -> if target is drop_target:
       emit Drop(source_node, target_node)
     else:
       emit DragCancel
  -> release capture
  -> consumed
```

默认阈值：

```text
drag_threshold_px = 4
配置位置：AUI interaction config/default 字段；代码可用 default helper，但不能把 4px 写成无法报告的隐藏 magic constant。
```

规则：

```text
只有 RuntimePointerButton::Primary / left pointer 可以触发 drag。
right / middle pointer 可以产生普通 pointer hit trace，但不触发 drag。
DragStart / DragMove / Drop 都是通用 UI 事件。
DragStart 之后不再生成 Click；drag 与 click 互斥。
source_node / target_node 是 AUI node id，不是项目对象 id。
项目对象身份由项目侧通过 node id、binding path 或 ProjectUiStateSnapshot 查询。
drag/drop 状态机归 Rust AUI Core，不进入 IR。
drop 后是否能装备、能购买、能放置，归项目 Rust Module / Transaction RuleSlot。
```

### 8.3 Payload C-min

第一版 payload 使用结构化 JSON 字符串，避免扩大 AuiAction 类型改动：

```text
AuiDragDropPayload:
  schema_version: "aui-drag-drop-payload.v1"
  source_node: string
  target_node: Option<string>
  start_pointer: { x, y }
  current_pointer: { x, y }
  delta: { x, y }
  drag_phase: "start" | "move" | "drop" | "cancel"
```

不允许：

```text
payload 直接塞 ECS entity pointer。
payload 直接塞 renderer handle。
payload 出现具体玩法字段，如 equipment_id / chess_piece_id / inventory_index。
```

payload 来源规则：

```text
runtime drag/drop payload 由 AuiInteractionSystem 根据 source_node / target_node / pointer 运行时生成。
authoring 侧 SetAuiActionRef.payload 当前仍 deferred，不作为本轮 runtime payload 输入。
report 必须标注 authoring_action_payload_deferred=true。
```

如果项目需要装备 id：

```text
项目侧通过 source_node 对应的 binding / snapshot path 查询。
或后续单独增加 Project UI Hot Logic Module / Transaction RuleSlot 映射。
```

## 9. Report 规则

新增或扩展 report 字段：

```text
AuiInteractionProductizationReport:
  schema_version
  frame
  document_id
  drag_threshold_px
  snapshot_frame_lag
  authoring_action_payload_deferred
  modal_input_blocking_deferred
  editor_hit_test_deferred_to_209
  input_event_count
  filtered_input_event_count
  consumed_pointer_event_count
  command_count
  action_count
  click_action_count
  drag_start_count
  drag_move_count
  drop_count
  drag_cancel_count
  active_drag_source
  traces[]
  diagnostics[]
```

Trace 字段：

```text
AuiInteractionTrace:
  frame
  event_kind
  pointer
  hit_node
  captured_node
  drop_target
  consumed
  reason
  command_count
  action_count
```

Native player summary 增加：

```text
aui_interaction_status
aui_input_consumed
aui_action_count
aui_drop_count
gameplay_input_filtered
snapshot_frame_lag
```

状态判断：

```text
text / draw / present 成功但 aui interaction 未运行:
  next_action = aui_runtime_interaction_productization

AUI consumed pointer input 但 filtered_frame 仍触发 gameplay fire:
  diagnostic = aui_input.consumed_pointer_leaked_to_gameplay

Drop command 没有 target_node:
  diagnostic = aui_drag.drop_without_target

Modal stage 有可见 interactable 节点但没有全屏阻挡:
  deferred = modal_input_blocking_deferred

editor 侧 hit-test 仍未闭合:
  deferred = editor_hit_test_deferred_to_209
```

## 10. 与现有规则关系

与 103：

```text
103 是 AUI Interaction core C-min。
213 是 runtime player 产品化 + drag/drop C-min。
```

与 195 / 196：

```text
符合。drag/drop 状态机属于 Rust AUI Core，不进入 IR。
IR / RuleSlot 只处理 can_drop / can_equip / equip_requested 这类受限业务规则。
```

与 199：

```text
ProjectUiStateSnapshot 继续只作为 UI read model。
AUI interaction 不直接读 ECS。
项目侧可用 snapshot / node id / action id 理解 UI intent。
```

与 201：

```text
AUI Core drag/drop/layout/hit test 不承诺热更。
布局、action id、binding path 可通过 AUI Document / RuntimePackage 更新。
业务可热更规则仍归 RuleSlot / Transaction RuleSlot。
```

与 210：

```text
Modal / ScreenOverlay / BeforeWorld 的渲染 stage 已有。
本轮 interaction 第一版只要求 ScreenOverlay / Modal 命中，Modal hit priority 高于 ScreenOverlay。
Modal 全屏输入阻挡 / focus trap 不在本轮，必须在 report 标注 modal_input_blocking_deferred。
BeforeWorld / WorldSpace raycast 不作为本轮验收。
```

与 211：

```text
AUI subtree template 可以复用复杂按钮 / 装备格。
实例化后的真实 AUI nodes 可以带 draggable / drop_target / action_refs。
Runtime 不读取 template asset，只读取展开后的 AUI Document nodes。
```

## 11. 复杂打飞机与复杂装备 UI 判断

对复杂打飞机：

```text
本系统让 HUD 按钮可点击、可消费输入、可生成 UI action report。
可以验证暂停按钮 / 继续按钮 / 确认按钮不会触发开火。
```

对复杂装备 UI / 自走棋 UI：

```text
本系统只解决通用 drag/drop 机制。
它可以生成 Drop(source_node, target_node)。
它不负责执行装备交易。
装备槽位校验、扣钱、替换装备、回滚诊断仍归项目 Rust Module / Transaction RuleSlot。
```

这能避免把 UI 框架写成玩法系统，同时也让后续装备 UI 有真实输入底座。

## 12. 可施工 Gate 建议

Gate A：交互 schema 补齐

```text
扩展 AuiCommandKind / AuiActionEvent / payload / report 数据结构。
为 AuiNode 显式增加 draggable / drop_target，serde default=false，保持旧 document 兼容。
新增 AUI interaction config/default，暴露 drag_threshold_px=4。
测试：cargo test -p engine_runtime aui_interaction
```

Gate B：Drag/drop C-min core

```text
实现 primary pointer capture、drag threshold、DragStart / DragMove / Drop / DragCancel。
Drop payload 包含 source_node / target_node / pointer delta。
断言只有 primary pointer 触发 drag，DragStart 后不生成 Click，拖到空白区生成 DragCancel。
测试：cargo test -p engine_runtime aui_drag
```

Gate C1：RuntimeInputFrame filter

```text
新增 RuntimeInputFrame::filter_consumed_pointer_events 或等价 helper。
采用 clone/rebuild filtered frame，不侵入 InputResolver。
只过滤 AUI consumed pointer event。
测试：cargo test -p engine_input aui_input_filter
```

Gate C2：Runtime player 顺序调整

```text
runtime_player_winit 从 input -> present 调整为 present/layout -> interaction -> filter -> InputResolver。
present 和 hit test 复用同一份 AUI present/layout 输出。
未消费输入继续进入 gameplay InputResolver。
测试：cargo test -p runtime_player_winit aui_interaction
```

Gate C3：EngineFrameInput 字段位

```text
EngineFrameInput 增加 aui_interaction。
EngineHostLoop 透传该字段给 runtime trace / report 可读位置。
NativeWindowHostReport 增加 AUI interaction summary。
测试：cargo test -p engine_runtime engine_host_loop aui_interaction
```

Gate D：Report / evidence

```text
NativeWindowHostReport 增加 AUI interaction summary。
report 输出 authoring_action_payload_deferred / modal_input_blocking_deferred / editor_hit_test_deferred_to_209 / snapshot_frame_lag=1。
project_e2e_gate 增加 complex-shooter-aui-runtime-interaction-productization-report.json。
测试：cargo test -p project_e2e_gate aui_interaction
```

Gate E：复杂打飞机 fixture

```text
构造 HUD click fixture：点击 UI 按钮不触发 action.fire。
构造 drag/drop fixture：source_node -> target_node 生成 Drop action 和 payload。
构造 drag cancel fixture：source_node -> 空白区生成 DragCancel。
测试：cargo test -p project_e2e_gate
```

Gate F：文档同步

```text
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
施工完成后归档施工文档。
测试：cargo fmt --check
```

建议完整测试：

```powershell
cargo fmt --check
cargo test -p engine_runtime aui
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate
```

## 13. 验收标准

必须证明：

```text
runtime_player_winit 真实输入帧会先经过 AUI interaction。
点击可交互 AUI Button 生成 AuiCommand::Click 和 AuiAction。
AUI consumed pointer input 不再触发 gameplay fire。
drag source pointer move 超过阈值后生成 DragStart / DragMove。
pointer up 到 drop target 后生成 Drop。
Drop payload 有 source_node / target_node / pointer delta。
所有交互都有 trace / report / next_action。
复杂打飞机 e2e report 能看到 aui_interaction_status=success。
```

允许保留：

```text
无 scroll。
无 focus navigation。
无 IME / InputField。
无 gamepad navigation。
无 world-space UI raycast。
无多指针 drag。
无复杂拖拽视觉 ghost。
无项目装备交易执行。
```

不允许：

```text
点击 UI 后 gameplay fire 仍被触发却报告 success。
drag/drop payload 出现项目专用语义。
让 IR 实现 drag/drop 状态机。
让 RuntimeRenderer 读取 input / action / binding path。
把 AUI Node 改成 Runtime ECS Entity。
```

## 14. 自审

是否仍是 B-min：

```text
是。只补 runtime 接线、input consumption、click action dispatch、drag/drop C-min。
没有扩展到 scroll、focus、IME、navigation、完整控件库。
```

是否满足用户要求：

```text
满足。用户选择 B-min，并要求把 drag/drop 加上；本方案采用 B-min+，明确把 DragStart / DragMove / Drop 纳入第一版。
```

是否适合 AI：

```text
适合。AI 可读 AuiInteractionTrace、AuiAction payload、filtered input report，不需要猜窗口事件或 renderer 状态。
```

是否适合复杂项目：

```text
适合。drag/drop 机制通用，项目业务交易仍留在 Project Module / RuleSlot，不污染 engine core。
```

是否与当前规则冲突：

```text
不冲突。195 / 196 / 200 / 201 均明确 drag/drop 是 Rust AUI Core 机制，不能进入 IR。
```

## 15. 最终结论

正式采用：

```text
213-AUI Runtime Interaction / Input Consumption / Action Dispatch Productization v1
方案 B-min+：Runtime Productized AUI Interaction + DragDrop C-min
```

本系统完成后，AUI 不再只是“能显示的 HUD”，而是能在导出运行时中处理 click 和 drag/drop，并能用结构化 report 证明 UI 输入没有泄漏到 gameplay 输入。
