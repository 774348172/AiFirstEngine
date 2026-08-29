# 214-AUI Complex Controls / Modal-Focus-Scroll Productization v1 方案

## 1. 这个系统是干什么的

一句话：

```text
把 213 已经接通的 AUI click / drag-drop 输入链路，继续推进到复杂项目 UI 面板可用：Modal 不穿透输入、焦点能被弹窗锁住、Scroll/List 能滚动消费输入、Scene 中 AUI 点选闭环更真实。
```

它不是重做 AUI，也不是新增一套 UI 脚本层。

当前 AUI 已经具备：

```text
AUI Document
  -> RuntimePackage
  -> ProjectUiStateSnapshot
  -> Binding Resolve
  -> AuiLayout / AuiDrawList
  -> AuiCompositionFrame
  -> RuntimeRenderer UI composition pass
  -> Present
  -> AUI click / drag-drop C-min
  -> consumed pointer input filter
```

214 补的是下一段复杂项目 UI 必须具备的交互基础：

```text
Modal stage
  -> blocks outside pointer / wheel / key leakage
  -> traps focus inside modal subtree

ScrollView / List C-min
  -> consumes MouseWheel / drag scroll
  -> produces scroll offset report

Scene Unified AUI Authoring
  -> AUI proxy hit-test closure
  -> SelectAuiNode evidence
```

简单说：213 让 HUD 可以点和拖；214 让暂停菜单、设置面板、装备列表、关卡列表这类真实游戏 UI 不再只是“能画出来”，而是能被稳定操作。

## 2. 在其它引擎里对标什么

### 2.1 Unity UGUI

对标：

```text
EventSystem / StandaloneInputModule
  -> pointer / submit / navigation input
ScrollRect
  -> scroll wheel / drag scroll / content offset
InputField
  -> text focus / edit / submit
Canvas + GraphicRaycaster
  -> UI hit test before gameplay
```

官方参考：

```text
https://docs.unity3d.com/Packages/com.unity.ugui@1.0/manual/script-ScrollRect.html
https://docs.unity3d.com/Packages/com.unity.ugui@1.0/manual/script-InputField.html
https://docs.unity3d.com/Packages/com.unity.ugui@2.0/manual/index.html
https://docs.unity3d.com/6000.1/Documentation/Manual/UIE-faq-event-and-input-system.html
```

本机源码参考：

```text
<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\EventSystem\InputModules\StandaloneInputModule.cs
  Process / ProcessMouseEvent / ProcessMousePress / ProcessMove / ProcessDrag

<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\ScrollRect.cs
  OnInitializePotentialDrag / OnBeginDrag / OnDrag / OnEndDrag / OnScroll

<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\InputField.cs
  OnSelect / OnDeselect / OnUpdateSelected / ProcessEvent / OnSubmit / OnPointerClick
```

可学习点：

```text
ScrollRect 是 UI framework 状态机，不是项目脚本自己读鼠标滚轮。
InputField 是复杂文本编辑控件，需要 focus / selection / submit / IME 等完整机制。
UI EventSystem 应该先于 gameplay input 消费对应输入。
```

不照搬：

```text
不照搬 GameObject / RectTransform 作为 AUI 真相。
不照搬完整 EventSystem / Selectable / Navigation 大体系。
本轮不做完整 InputField / IME / Mask / inertia / nested scroll。
```

### 2.2 Unreal Slate / UMG / CommonUI

对标：

```text
FSlateApplication
  -> hit test / RoutePointerDownEvent / ProcessReply
FReply
  -> Handled / focus / capture / navigation intent
SScrollBox
  -> OnMouseWheel / SetScrollOffset
SEditableText
  -> SupportsKeyboardFocus / OnTextChanged / OnTextCommitted
CommonActivatableWidget / CommonUI Action Router
  -> modal / active stack / input routing
```

官方参考：

```text
https://dev.epicgames.com/documentation/unreal-engine/slate-user-interface-programming-framework-for-unreal-engine
https://dev.epicgames.com/documentation/unreal-engine/understanding-the-slate-ui-architecture-in-unreal-engine
https://dev.epicgames.com/documentation/unreal-engine/creating-user-interfaces-with-umg-and-slate-in-unreal-engine
```

本机源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Private\Framework\Application\SlateApplication.cpp
  ProcessReply / SetUserFocus / SetKeyboardFocus

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Public\Widgets\Layout\SScrollBox.h
  GetScrollOffset / SetScrollOffset / ScrollToStart / ScrollToEnd / OnMouseWheel

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Private\Widgets\Layout\SScrollBox.cpp
  SetScrollOffset / Tick / OnMouseWheel

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Public\Widgets\Input\SEditableText.h
  SupportsKeyboardFocus / SetText / GetText / OnTextChanged / OnTextCommitted
```

可学习点：

```text
输入处理结果要有明确 reply / handled / focus 语义。
Modal / focus 不是单个按钮属性，而是 UI runtime 的 active stack / focus state。
ScrollBox 的滚动位置是 runtime state，不应该写回 UI document 真相。
```

不照搬：

```text
不新增 Slate 式运行时 Widget 对象层。
不做完整 tunnel / bubble routing。
不做完整 CommonUI active tree / multi-platform action router。
```

### 2.3 Bevy UI / input_focus

对标：

```text
Interaction
  -> Pressed / Hovered / None
FocusPolicy
  -> block / pass
InputFocus
  -> focused entity / FocusedInput dispatch
```

官方参考：

```text
https://docs.rs/bevy/latest/bevy/input_focus/index.html
https://docs.rs/bevy/latest/bevy/input_focus/struct.InputFocus.html
https://docs.rs/bevy/latest/bevy/input_focus/struct.FocusedInput.html
https://docs.rs/bevy/latest/bevy/input_focus/tab_navigation/index.html
```

本机源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_ui\src\focus.rs
  Interaction / FocusPolicy / ui_focus_system

<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_input_focus\src\lib.rs
  InputFocus / FocusedInput / FocusChangeEvents
```

可学习点：

```text
focus 可以是明确 runtime resource / state，而不是隐式散落在控件里。
UI hit / focus / input dispatch 可以通过结构化状态和事件报告。
```

不照搬：

```text
AUI Node 不变成 ECS Entity。
不把 Bevy ECS UI tree 当成 AUI 真相。
```

## 3. 本项目当前基线

### 3.1 已完成能力

已完成系统：

```text
190 AUI RuntimePackage Document Hydration / Binding / Present
199 ProjectUiStateSnapshot Producer
204 AUI Document Authoring
208 Runtime Text Glyph Present
209 AUI Scene Unified Authoring
210 RuntimeRenderer Multi-stage UI Composition Pass
211 AUI Prefab / Template Reuse
213 AUI Runtime Interaction / Input Consumption / Action Dispatch
```

当前代码基线：

```text
rust/crates/engine_runtime/src/aui.rs
  AuiNodeKind 已有 Panel / Image / Text / Button / ProgressBar / Toggle / Slider / List / ScrollView / InputField / Custom。
  AuiNode 已有 interactable / consume_input / draggable / drop_target。
  AuiInteractionSystem 已支持 pointer hit test / click / drag-drop C-min。
  AuiInteractionConfig 当前只有 drag_threshold_px。
  AuiInteractionEventKind 当前只有 PointerDown / PointerUp / PointerMove。
  AuiCommandKind 当前已有 PointerDown / PointerUp / PointerMove / Hover / Click / DragStart / DragMove / Drop / DragCancel。

rust/crates/engine_input/src/input_mapping.rs
  RuntimeInputEvent 已有 PointerDown / PointerMove / PointerUp / PointerHeld / MouseWheel / KeyDown / KeyUp / KeyHeld。
  RuntimeInputFrame 已能 filter consumed pointer events。
  注意：当前 filter 仍是 pointer-only；214 必须扩展为 filter_consumed_events，支持 MouseWheel / KeyDown / KeyUp / KeyHeld 的 consumed 过滤证据。

rust/crates/runtime_player_winit/src/lib.rs
  runtime loop 已按 213 调整为 AUI present/layout -> interaction -> filtered input -> InputResolver。
  real-window 已能把 winit MouseWheel 转为 runtime input delta。

rust/crates/editor_input/src/lib.rs
  HitTarget::AuiSceneNode 已能优先路由为 UiCommandPayload::SelectAuiNode。

rust/crates/editor_core/src/services/aui_service.rs
  AUI authoring service 能 decode ScrollView / InputField node kind，但控件行为未产品化。
```

### 3.2 当前真实缺口

```text
Modal 当前只是 rendering composition stage。
Modal 全屏 input blocking / focus trap 仍是 213 deferred flag。
ScrollView / List 只有 enum / authoring kind，runtime 没有 scroll state、scroll offset、wheel consumption、drag scroll。
InputField 只有 enum / authoring kind，本轮仍不做。
Scene AUI hit-test 已有 HitTarget / SelectAuiNode route，但仍需要把 209 authoring proxy + current pointer + report 作为闭合证据。
```

### 3.3 必须遵守的边界

来自 `195` / `199` / `209` / `210` / `211` / `213`：

```text
UI 系统由 Rust AUI Runtime Framework 写。
UI 界面结构由 AUI Document 写。
UI 显示数据由 ProjectUiStateSnapshot 提供。
复杂 UI 交互机制由 Rust AUI Runtime Framework 写。
IR 不实现 drag / focus / scroll / IME。
AUI Node 不变成 Runtime ECS Entity。
Scroll / focus / modal runtime state 不写回 AUI Document。
复杂控件视觉组合继续使用 AUI subtree，不把一个 AuiNode 做成 GameObject Component 容器。
```

## 4. 可选方案

### 方案 A：Modal / Focus Closure C-min

做：

```text
Modal 全屏 input blocking。
Modal focus trap。
Scene AUI hit-test closure。
```

不做：

```text
ScrollView / List。
InputField / IME。
keyboard / gamepad navigation。
```

优点：

```text
范围最小。
直接清掉 213 的 Modal / focus deferred。
```

缺点：

```text
复杂打飞机的装备列表、关卡列表、设置列表仍不能真实滚动。
做完后 UI 依旧只适合按钮和简单面板。
```

结论：

```text
不采用为本轮正式方案。
```

### 方案 B：Complex Controls B-min

做：

```text
Modal input blocking。
Modal focus trap C-min。
ScrollView / List C-min：wheel scroll + drag scroll + scroll offset report。
Scene AUI hit-test closure：AUI proxy hit -> SelectAuiNode -> Inspector/report 证据。
Native/player/e2e report 输出 modal / focus / scroll / editor-hit-test evidence。
```

不做：

```text
完整 InputField / IME。
完整 keyboard navigation / gamepad navigation。
multi pointer / touch。
nested scroll。
inertia / elastic scroll。
virtualized list。
Mask / Clip / Scrollbar rendering。
rich text / CJK shaping 扩展。
通用 UI dirty / cache / batching。
项目装备交易执行逻辑。
```

优点：

```text
覆盖复杂打飞机近期最需要的真实 UI 面板能力。
不新增架构层，只扩 AuiInteractionState / AuiInteractionConfig / AuiInteractionReport。
AI 可以通过结构化 report 判断输入是否被 UI 正确消费。
保留后续完整控件体系接口。
```

缺点：

```text
不是完整 UI 框架。
ScrollView 第一版没有视觉裁剪和滚动条渲染，主要证明交互和状态链路。
Focus trap 第一版只解决 Modal 内 Tab/Escape/KeyDown 归属，不做完整空间导航。
```

结论：

```text
采用。
```

### 方案 C：完整复杂控件系统

做：

```text
完整 modal stack。
完整 focus manager / keyboard navigation / gamepad navigation。
ScrollView inertia / nested scroll / scrollbar / mask / clip / virtualization。
InputField cursor / selection / clipboard / IME / submit / cancel。
多平台输入和可访问性。
```

优点：

```text
长期最接近 UGUI / Slate / CommonUI。
```

缺点：

```text
范围过大，会把本轮从复杂打飞机需要的 UI 能力扩成完整 UI framework。
会同时碰 input / text / font / layout / clipping / renderer / editor 多条主线。
风险是重新走向“功能太多 -> 加层太多 -> 难维护”。
```

结论：

```text
不采用为本轮施工方案，只保留为长期路线。
```

## 5. 正式推荐方案：B-min

正式命名：

```text
AUI Complex Controls / Modal-Focus-Scroll Productization v1
```

采用：

```text
方案 B：Complex Controls B-min
```

核心原则：

```text
不新增运行时架构层。
不新增 AUI Designer。
不把 AUI Node 变成 ECS Entity。
不让 IR 处理 UI state machine。
不把 ScrollView / Modal state 写回 AUI Document。
所有新增能力必须有 schema / report / deterministic e2e evidence。
```

## 6. B-min 具体设计

### 6.1 Modal input blocking

当前：

```text
Modal 是 AuiCompositionStage 的 rendering stage。
213 只保证 Modal hit priority 或 report deferred，不做全屏阻挡。
```

214 做：

```text
AuiInteractionConfig:
  modal_blocks_pointer_outside: bool = true
  modal_blocks_wheel_outside: bool = true
  modal_blocks_keyboard: bool = true

AuiInteractionState:
  active_modal_root: Option<String>

AuiInteractionSystem:
  如果当前 document 有 visible Modal canvas：
    pointer 命中 Modal subtree -> 正常产生 command/action。
    pointer 未命中 Modal subtree -> consumed=true，但不产生业务 action。
    MouseWheel 未命中可滚动 Modal ScrollView -> consumed=true，不进入 gameplay。
    KeyDown / KeyHeld / KeyUp -> 如果 focus 在 Modal 内，consumed=true。
```

输入过滤实现规则：

```text
必须把 engine_input 的 filter_consumed_pointer_events 扩展为 filter_consumed_events。
consumed 证据不能只按 pointer 类型过滤，必须能按事件 kind 区分 pointer / wheel / key。
AUI interaction report 需要输出 consumed_event_count_by_kind，证明 wheel/key 没有继续流入 gameplay InputResolver。
必须新增 key_event_info 或等价路径，让 KeyDown / KeyUp / KeyHeld 进入 AUI interaction；Tab/Escape 不能继续被 pointer_event_info 忽略。
```

第一版 Modal root 判断：

```text
按 AuiCanvas.composition_stage == Modal 找 root_node。
如果多个 Modal canvas，按 layer / sorting_order / tree_order 取 topmost。
```

不做：

```text
多 Modal stack / push-pop API。
动画 opening / closing 状态。
背景 dim overlay 自动生成。
```

### 6.2 Focus trap C-min

当前：

```text
AUI runtime 没有 focus state。
engine_input 已有 KeyDown / KeyUp / KeyHeld。
editor_window_winit 有 editor focus 系统，但那是 editor UI，不是 runtime AUI。
```

214 做：

```text
AuiFocusState:
  focused_node: Option<String>
  focus_scope_root: Option<String>
  focus_reason: Pointer | Keyboard | ModalOpen | Cleared

AuiInteractionState:
  focus: AuiFocusState

规则：
  PointerDown 命中 interactable AUI node -> focused_node = node_id。
  Modal active 时 focus_scope_root = topmost modal canvas 的 root_node id。
  Modal 不再 active 时清空 focus_scope_root，必要时清空不在有效 scope 内的 focused_node。
  Tab / Shift+Tab 只在 focus_scope_root subtree 内循环。
  Escape 在 Modal active 时产生 AuiCommandKind::Cancel / action event close intent，但不直接关闭项目弹窗。
  Tab / Escape 必须来自 RuntimeInputEvent::KeyDown 经 key_event_info 进入 AUI，不允许只在 gameplay input resolver 中处理。
```

本轮新增命令建议：

```text
AuiCommandKind::Focus
AuiCommandKind::Blur
AuiCommandKind::Cancel

AuiActionEvent::Focus
AuiActionEvent::Blur
AuiActionEvent::Cancel
```

说明：

```text
Cancel 只是 UI 意图，是否关闭暂停菜单由 Project Logic / RuleSlot 处理。
Focus / Blur 主要用于 report 和后续样式状态，不要求本轮做 focused visual style。
```

不做：

```text
方向键空间导航。
gamepad navigation。
文本输入焦点编辑。
```

### 6.3 ScrollView / List C-min

当前：

```text
AuiNodeKind 已有 ScrollView / List，但 layout / interaction 未做滚动状态。
RuntimeInputEvent 已有 MouseWheel。
```

214 做：

```text
AuiScrollState:
  node_id: String
  offset_y: f32
  max_offset_y: f32
  last_delta_y: f32

AuiInteractionState:
  scroll_offsets: BTreeMap<String, AuiScrollState>
  active_scroll_capture: Option<String>

AuiInteractionConfig:
  wheel_scroll_px_per_delta: f32 = 48.0
  drag_scroll_threshold_px: f32 = 4.0

AuiInteractionSystem:
  MouseWheel over ScrollView/List -> update offset_y, consumed=true。
  PointerDown on ScrollView/List -> may capture scroll。
  PointerMove beyond threshold -> drag scroll, consumed=true。
  PointerUp -> release scroll capture。
```

Layout / draw C-min 规则：

```text
本轮可以先不做真实 Mask / Clip。
Scroll offset 必须在 layout 阶段影响 ScrollView/List subtree 的 child computed rect y 偏移，保证 report 能证明内容位置变化。
本轮不把 draw item y offset 当主路径，也不把 scroll_offset_applied=false 当成功路径。
如果无法安全移动真实 child rect，Gate C 失败并输出诊断；不能假装通过。
```

推荐第一版：

```text
在 AuiLayoutEngine 或 AuiInteraction-applied layout helper 中接收 scroll_offsets。
layout_node 遇到 ScrollView/List 时，把对应 offset_y 应用到子树 computed rect。
layout report 至少输出 scroll_offset_applied、scroll_applied_node_count、clipped_node_count。
clipped_node_count 需要基于 offset 后子节点是否超出 viewport 给出真实证据，不能继续永远为 0。
不新增 renderer clip pass。
不新增 Scrollbar 渲染。
```

不做：

```text
nested scroll。
inertia / elastic。
virtualized list。
scrollbar visual。
mask / clip。
```

### 6.4 Scene AUI hit-test closure

当前：

```text
209 已把 AUI 进入 Scene 统一 authoring。
editor_input 已有 HitTarget::AuiSceneNode -> UiCommandPayload::SelectAuiNode 路由。
213 report 仍保留 editor_hit_test_deferred_to_209。
```

214 做：

```text
AuiSceneUnifiedAuthoringReport:
  pointer_source: scene_view
  hit_test_status: HitAuiNode / HitSceneEntity / Miss
  hit_document_path
  hit_node_id
  routed_command_kind = SelectAuiNode
  inspector_target_kind = AuiNode
```

规则：

```text
不新增 AUI 专用编辑模式。
不新增独立 AUI Designer。
Scene View 的 2D 仍只是视图模式。
点击 AUI proxy 优先于 Viewport 背景；未命中 AUI 时保留 Scene entity / viewport route。
```

### 6.5 Report

新增或扩展：

```text
AuiComplexControlsProductizationReport:
  schema_version
  status
  modal_blocking_status
  focus_trap_status
  scroll_status
  scene_hit_test_status
  consumed_pointer_count
  consumed_wheel_count
  consumed_keyboard_count
  consumed_event_count_by_kind
  focus_change_count
  scroll_offset_change_count
  scroll_offset_applied
  scroll_applied_node_count
  clipped_node_count
  gameplay_input_filtered_count
  control_style_deferred
  slider_toggle_binding_target_deferred
  authoring_action_payload_deferred
  diagnostics
  next_actions
```

runtime player summary 增加：

```text
aui_modal_blocking_status
aui_focus_trap_status
aui_scroll_status
aui_scroll_offset_count
aui_consumed_wheel_count
aui_consumed_keyboard_count
aui_consumed_event_count_by_kind
aui_scroll_offset_applied
aui_scroll_applied_node_count
aui_clipped_node_count
```

project_e2e_gate 新增：

```text
complex-shooter-aui-complex-controls-productization-report.json
```

报告必须证明：

```text
点击 Modal 外背景不会触发 gameplay fire。
Modal active 时 wheel / key 不泄漏到 gameplay。
ScrollView/List wheel 后 offset_y 发生变化。
ScrollView/List layout 后 child computed rect 发生 offset 后的位置变化。
drag scroll 后 offset_y 发生变化，并进入 layout/report 证据。
Scene AUI hit route 能选中 AuiNode。
```

## 7. AI 和用户如何理解

用户心智：

```text
我在 Scene 里编辑 UI。
Modal 是弹窗层。
ScrollView/List 是可滚动区域。
Panel / Image / Text 可以继续作为 AUI subtree 组合复杂界面。
Button 当前保持单节点 + label；复杂控件、装备格、组合面板用 subtree 组合。
点击按钮后触发 action，业务是否执行由项目逻辑处理。
```

AI 心智：

```text
AUI Document 修改结构和字段。
AuiInteractionState 负责运行时 focus / scroll / capture。
ProjectUiStateSnapshot 负责显示数据。
AuiAction 只表示 UI 意图。
Project Logic / Transaction RuleSlot 处理业务后果。
Report 负责证明输入是否消费、焦点是否锁定、滚动是否生效。
```

禁止 AI 做：

```text
不要把 UI focus / scroll state 写进 AUI Document。
不要让 IR 读鼠标、读键盘、算 hit test 或更新 scroll offset。
不要在 engine_runtime 写 Player / Weapon / Equipment 等项目语义。
不要为 InputField / IME 偷偷铺完整文本编辑系统。
```

## 8. 施工 Gate 草案

正式施工文档生成时，建议按以下 Gate 拆：

### Gate A：schema / state / command 补齐

目标：

```text
新增 AuiFocusState / AuiScrollState。
扩展 AuiInteractionConfig。
扩展 AuiInteractionState。
新增 Focus / Blur / Cancel / Scroll command/action 语义。
新增 report struct。
扩展 filter_consumed_pointer_events -> filter_consumed_events，支持 pointer / wheel / key consumed 过滤。
新增 key_event_info 或等价路径，让 KeyDown / KeyUp / KeyHeld 可进入 AUI interaction。
本轮不扩 AuiStyle 专属控件样式，report 标记 control_style_deferred=true。
本轮不扩 SliderValue / ToggleState binding target；ScrollView offset 是 runtime transient state，不作为 AUI Document binding target。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_focus
cargo test -p engine_runtime aui_scroll
```

### Gate B：Modal blocking + focus trap

目标：

```text
Modal canvas topmost detection。
Modal 外 pointer / wheel / key consumption。
Modal active 时 focus_scope_root = topmost modal canvas root_node id；Modal 关闭时清空。
Tab 在 Modal scope 内循环。
Escape 生成 Cancel command/action，不直接关闭项目 UI。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_modal
cargo test -p engine_runtime aui_focus
```

### Gate C：ScrollView / List C-min

目标：

```text
MouseWheel over ScrollView/List 改变 offset_y。
drag scroll 改变 offset_y。
scroll offset 进入 layout，真实改变 ScrollView/List 子树 computed rect。
report 输出 scroll_offset_applied=true、scroll_applied_node_count、clipped_node_count。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_scroll
```

### Gate D：runtime_player_winit 接线与 summary

目标：

```text
runtime loop 使用同一个 AuiInteractionState 保存 focus / scroll。
MouseWheel / KeyDown / Pointer 经过 AUI interaction 后再进入 filtered InputResolver。
NativeAuiPresentSummary 增加 modal/focus/scroll evidence。
```

测试：

```powershell
cd rust
cargo test -p runtime_player_winit aui
cargo test -p runtime_player_winit aui_interaction
```

### Gate E：Scene AUI hit-test closure

目标：

```text
AUI proxy hit -> HitTarget::AuiSceneNode -> SelectAuiNode -> Inspector / report。
project_e2e_gate 能输出 Scene AUI hit-test closure evidence。
```

测试：

```powershell
cd rust
cargo test -p editor_input aui_scene
cargo test -p editor_core aui_scene_authoring
```

### Gate F：complex shooter e2e report

目标：

```text
新增 complex-shooter-aui-complex-controls-productization-report.json。
覆盖 modal blocking、focus trap、wheel scroll、drag scroll、Scene AUI hit-test。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate aui_complex_controls
cargo test -p project_e2e_gate
```

## 9. 验收标准

必须通过：

```text
Modal active 时点击弹窗外，gameplay fire 不触发。
Modal active 时 MouseWheel 不泄漏到 gameplay scroll action。
Modal active 时 KeyDown 不泄漏到 gameplay action，除非明确配置 pass-through。
filter_consumed_events 能用 report 证明 pointer / wheel / key 分 kind 被过滤。
Tab 只在 Modal subtree 内改变 focus。
Escape 只生成 AUI Cancel action，不直接改项目状态。
ScrollView/List wheel 后 offset_y 改变并进入 report。
ScrollView/List wheel/drag 后 child computed rect 发生 layout offset，并进入 report。
drag scroll 后 offset_y 改变；不能用“未支持诊断”替代本轮成功。
Scene View 点击 AUI proxy 能生成 SelectAuiNode 并进入 selection / inspector evidence。
```

允许 deferred：

```text
InputField / IME。
完整 keyboard navigation / gamepad navigation。
nested scroll。
scrollbar visual。
Mask / Clip。
virtualized list。
dirty / cache / batch 优化。
full modal stack / transition animation。
control-specific AuiStyle 扩展。
Slider / Toggle binding target。
authoring action payload channel。
```

不允许：

```text
用 debug overlay 假装 AUI ScrollView。
用项目语义污染 AUI core。
让 UI 事件直接修改 ECS。
让 IR 实现 focus / scroll / modal state machine。
```

## 10. 自审

### 10.0 是否吸收 33 号审查

```text
已吸收。
filter 从 pointer-only 明确升级为 filter_consumed_events。
KeyDown/KeyUp/KeyHeld 通过 key_event_info 或等价路径进入 AUI interaction。
Scroll offset 固定走 layout computed rect 路径，不把 draw offset 或 diagnostic fallback 当成功路径。
Button 措辞修正为单节点 + label；复杂控件和装备格才使用 subtree 组合。
AuiStyle 专属控件样式、Slider/Toggle binding target、authoring action payload channel 均明确 deferred 并进入 report。
```

### 10.1 是否增加了新层

```text
没有。
本方案只扩展现有 Rust AUI Runtime Core 的 state / interaction / report。
```

没有新增：

```text
Logic Ownership Router。
Architecture Guard。
独立 AUI Designer。
运行时 Widget 对象层。
UI 脚本系统。
```

### 10.2 是否过度扩张

```text
没有。
本轮只做 Modal / Focus / Scroll/List / Scene hit-test closure。
InputField / IME / rich text / mask / virtualized list / full navigation 都明确 deferred。
```

### 10.3 是否符合复杂打飞机目标

```text
符合。
复杂打飞机需要暂停菜单、设置面板、武器/装备/关卡列表，这些比完整 InputField/IME 更靠前。
```

### 10.4 是否符合 AI-first

```text
符合。
所有新增行为都有 schema / state / report / deterministic test。
AI 可以通过 report 判断输入是否被 UI 消费、焦点是否锁定、滚动是否发生。
```

### 10.5 是否和 195 / 199 / 209 / 210 / 211 / 213 冲突

```text
不冲突。
214 是 213 deferred flags 的后续收敛。
AUI Document 仍是结构真相。
ProjectUiStateSnapshot 仍是显示数据入口。
Scroll / focus / modal state 是 runtime transient state。
Scene authoring 仍按 209，不新增 Designer。
复杂控件仍按 211 使用 subtree。
```

## 11. 结论

```text
采用方案 B：Complex Controls B-min。
下一步如要施工，应先根据本方案生成自动化施工文档，并在施工文档中把 Gate A-F、测试命令、文档同步和归档规则写清楚。
```
