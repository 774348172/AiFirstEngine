# 216-AUI Runtime Navigation / Screen Flow / Text Entry Productization v1 方案

采用方案：

```text
C-full-guarded
```

## 1. 这个系统是干什么的

一句话：

```text
让 AUI 菜单和输入框从“能点、能滚、能显示”推进到“能用键盘/手柄完整操作，能进入/退出界面，能编辑文本并处理 IME 的第一版可产品化闭环”。
```

它补齐 213 / 214 / 215 之后复杂 UI 仍缺的四件事：

```text
Submit / Cancel：
  焦点按钮、菜单项、输入框可以被 Enter / Space / Gamepad A 提交，被 Esc / Gamepad B 取消。

Screen Flow：
  暂停菜单、设置菜单、装备菜单、确认弹窗有 runtime screen stack / back stack / default focus / focus restore。

Gamepad Navigation：
  键盘方向键和手柄方向输入归一成 UI navigation intent，不让项目逻辑自己猜按键。

Text Entry：
  InputField 不再只是 AuiNodeKind enum，而是有 edit mode、draft text、caret/selection C-min、TextChanged / TextSubmitted / TextCancelled 和 IME composition schema。
```

它在成熟引擎里的大致对标：

```text
Unity UGUI：
  EventSystem / StandaloneInputModule / Selectable Navigation / Button submit / InputField。

Unreal：
  Slate focus/input reply + UMG widgets + CommonUI ActivatableWidget / Action Router / Widget Stack。

Godot：
  Control focus navigation / ui_accept / ui_cancel / LineEdit / TextEdit。
```

它在本引擎主线中的作用：

```text
AUI Document 仍是 UI 结构真相。
AUI Runtime Core 负责 navigation / screen stack / text editing state machine。
ProjectUiStateSnapshot 仍负责显示数据输入。
AUI action 仍只是业务级 UI 意图，不直接改 ECS / Project State。
IR 不处理 UI 输入状态机。
AUI Node 不变成 Runtime ECS Entity。
```

## 2. 为什么现在需要它

215 已完成：

```text
RectClip。
vertical scrollbar。
keyboard navigation B-min。
focus visible auto-scroll。
```

但 215 明确 deferred：

```text
InputField / IME。
完整 gamepad navigation。
Submit action / screen flow。
virtualized list。
nested scroll。
inertia。
```

复杂打飞机项目马上会遇到这些真实场景：

```text
开始菜单：键盘/手柄选择 Start / Options / Exit。
暂停菜单：Esc / B 打开或返回，Resume / Restart / Settings 可以 Submit。
设置菜单：slider / toggle / dropdown 需要焦点和 submit/cancel 语义。
装备菜单：方向键/手柄在装备格之间移动，Submit 选择，Cancel 返回。
命名/存档/调试输入：InputField 需要文本输入和 IME 的基础链路。
```

如果没有 216，复杂 UI 会卡在：

```text
鼠标能点，但键盘/手柄不能完整操作。
Cancel 只是一个零散 action，不知道该关闭输入框、弹窗还是返回上一个 screen。
InputField 只是节点类型，没有真正的文本编辑 runtime。
IME 没有 schema，后续中文输入会被临时补丁污染 input / AUI / editor 多条链路。
```

## 3. 外部源码参考

### 3.1 Unity UGUI

本机源码参考：

```text
<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\EventSystem\InputModules\StandaloneInputModule.cs
  Process
  SendMoveEventToSelectedObject
  SendSubmitEventToSelectedObject
  submitButton / cancelButton

<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\Selectable.cs
  navigation / FindSelectable

<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\InputField.cs
  compositionString
  OnUpdateSelected
  Append
  SendOnSubmit
  OnDeselect
```

可学习点：

```text
UI input 需要统一模块先处理 pointer / move / submit，再把 handled 结果留给 gameplay input。
Submit / Cancel 是 UI framework 基础语义，不应该由每个按钮项目脚本重新实现。
InputField 是 focus + edit buffer + caret + selection + composition + submit 的状态机，不是普通 Text 节点加一个 action。
```

不照搬：

```text
不照搬 GameObject / RectTransform / Component 作为 AUI 真相。
不照搬完整 Unity EventSystem / Selectable 大体系。
不把 InputField 的所有平台细节一次性复制进 v1。
```

### 3.2 Unreal Slate / UMG / CommonUI

本机源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\Runtime\CommonUI\Source\CommonUI\Public\CommonActivatableWidget.h
  UCommonActivatableWidget
  ActivateWidget
  DeactivateWidget
  NativeOnHandleBackAction

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\Runtime\CommonUI\Source\CommonUI\Public\Widgets\CommonActivatableWidgetContainer.h
  UCommonActivatableWidgetStack
  UCommonActivatableWidgetQueue

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\Runtime\CommonUI\Source\CommonUI\Public\Input\CommonUIActionRouterBase.h
  GetLeafmostActivatableWidget
  SetActiveRoot
  RegisterWidgetBindings

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\Runtime\CommonUI\Source\CommonUI\Private\Input\UIActionRouterTypes.cpp
  FActivatableTreeNode
  FActivatableTreeRoot
  UpdateLeafmostActiveNode
  CacheFocusRestorationTarget
```

可学习点：

```text
复杂游戏 UI 需要 active screen / leafmost active node / back action / focus restore 的统一 runtime 语义。
手柄和键盘不应该直接散落到项目脚本里；先归一成 UI action / navigation intent。
Screen stack 和 focus restore 是运行时状态，不是写回 UI 资源的结构修改。
```

不照搬：

```text
不新增 CommonUI 式 Activatable Widget 对象层。
不新增完整 Action Router / Action Domain / Input Config 体系。
不把 AUI Node 注册成运行时 Widget 实例树。
```

### 3.3 Godot

本机源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\gui\control.*
  Control focus / gui input / focus neighbor。

<GODOT_SOURCE>\godot-master\godot-master\scene\gui\line_edit.cpp
  LineEdit edit / unedit / is_editing。
  has_ime_text / cancel_ime / apply_ime。
  set_text / get_text / caret / selection / insert_text_at_caret / delete_text。

<GODOT_SOURCE>\godot-master\godot-master\scene\gui\text_edit.*
  TextEdit 多行文本编辑和 caret/selection。
```

可学习点：

```text
focus navigation、ui_accept/ui_cancel 和 LineEdit/TextEdit 都是 GUI runtime 内核能力。
IME 必须从一开始有明确 API，即使 v1 只做 preedit / commit / cancel 的基础证据。
```

不照搬：

```text
不照搬 Godot Node + script 模型。
不把 UI 控件做成可随意脚本覆写的对象树。
不在本轮实现完整 TextEdit / 多行 / bidi / clipboard / candidate window。
```

## 4. 本项目当前基线

相关已完成系统：

```text
190 AUI RuntimePackage Document Hydration / Binding / Present
199 AUI ProjectUiStateSnapshot Producer
204 AUI Document Authoring
208 Runtime Text Glyph Present
209 AUI Scene Unified Authoring
210 RuntimeRenderer Multi-stage UI Composition Pass
211 AUI Prefab / Template Reuse
213 AUI Runtime Interaction / Input Consumption / Action Dispatch
214 AUI Complex Controls / Modal-Focus-Scroll
215 AUI RectClip / Scrollbar / Keyboard Navigation
```

当前代码基线：

```text
rust/crates/engine_runtime/src/aui.rs
  AuiNodeKind 已有 InputField。
  AuiNavigationMode / AuiFocusState / AuiInteractionState 已存在。
  AuiCommandKind / AuiActionEvent 已有 Cancel / Scroll / Focus / Blur。
  当前没有 Submit command/action。
  当前没有 screen stack / active screen / focus restore stack。
  当前没有 InputField edit state / draft text / caret / selection / composition state。

rust/crates/engine_input/src/input_mapping.rs
  RuntimeInputEvent 已有 Pointer / MouseWheel / KeyDown / KeyUp / KeyHeld。
  filter_consumed_events 已存在。
  当前没有 TextInput / IME preedit / IME commit / composition cancel 事件。

rust/crates/editor_core/src/property_editing.rs
  已有 TextCompositionState，可作为编辑器侧 IME 状态参考。
  但 runtime AUI 不能直接复用 editor property editing 的状态真相。
```

当前真实缺口：

```text
Submit 没有成为 AUI command/action。
Cancel 还没有和 screen back / text edit cancel 形成统一优先级。
Gamepad 输入没有归一成 AUI navigation intent。
Screen stack / Back stack / default focus / focus restore 没有 runtime 产品化。
InputField 没有运行时 edit buffer。
IME 没有 runtime input schema 和 report。
```

## 4.1 吸收 34 号审查后的新增前置

`其它AI审查目录/34-216-AUI-Runtime-Navigation-ScreenFlow-TextEntry方案审查.md` 判断本方案方向正确，但指出进入施工前必须吸收以下前置：

```text
高严重度：
  modifiers 链路断链：
    RuntimeInputFrame.modifiers 字段存在，AUI 也读取 Shift，但 device_state / winit 没有真实填充。
    216 施工必须补 WindowEvent::ModifiersChanged -> RuntimeInputFrame.modifiers。

  winit TextInput / IME 事件无来源：
    RuntimeInputEvent 即使新增 TextInput / Ime*，也必须由 runtime_player_winit 把 Character / IME 事件转进来。
    key_name 不能把文本输入当普通 KeyDown 并强制大写。

中严重度：
  Screen flow 实现路径必须收敛为 canvas runtime visible，不走 ProjectUiStateSnapshot 切结构。
  AuiBindingTarget 必须新增 InputFieldText，完成 snapshot -> InputField 显示回流。
  authoring 必须能编辑本轮新增字段，否则 AI/用户不能稳定配置 screen metadata / InputField 属性。

低严重度：
  gamepad v1 可以先做 normalized intent schema 和 synthetic/headless 输入，不要求完整设备生态。
  focusable v1 推荐显式字段；旧文档可用 interactable 推导，并在 report 标注。
  IME 平台覆盖必须进入 report，不假装全平台完整。
```

因此，本轮施工文档必须把这些内容列入 Gate，不允许只做 engine_runtime headless enum 扩展。

## 5. 可选方案

### 方案 A：拆成两个系统分别做

内容：

```text
先单独做 Screen Flow / Submit-Cancel / Gamepad Navigation。
后续再单独做 InputField / Text Editing / IME。
```

优点：

```text
单次施工范围小。
Submit/Cancel 和菜单可先落地。
```

缺点：

```text
Cancel 优先级会被拆裂：同一个 Esc / B 既可能退出输入框，也可能关闭弹窗或返回 screen。
焦点、输入模式、文本编辑模式会重复设计。
下一轮做 InputField 时容易推翻 Screen Flow 的输入归属。
```

结论：

```text
不采用。它看起来小，但会把同一套输入状态机拆成两次不一致的设计。
```

### 方案 B：B-min，只做菜单导航

内容：

```text
Submit / Cancel。
Screen stack。
default focus / focus restore。
keyboard + gamepad navigation intent。
InputField / IME 继续 deferred。
```

优点：

```text
能让暂停菜单、开始菜单、装备菜单先可用。
风险低于完整文本编辑。
```

缺点：

```text
InputField 仍只是 enum。
后续中文输入、命名输入、调试输入仍会出现临时补丁压力。
Cancel 语义仍缺少 text-editing mode 这条关键分支。
```

结论：

```text
可作为保底，但不是本轮采用方案。
```

### 方案 C：C-full

内容：

```text
Submit / Cancel。
Screen stack / Back stack。
default focus / focus restore。
完整 keyboard + gamepad navigation。
Action prompt。
InputField edit mode。
caret / selection。
IME。
clipboard。
multi-line。
rich text。
candidate window。
accessibility。
transition animation。
```

优点：

```text
最接近成熟商业 UI framework。
```

缺点：

```text
范围过大。
会把当前 AUI runtime 推向完整 CommonUI + TextEdit 克隆。
测试面过宽，容易影响 213-215 已完成链路稳定性。
```

结论：

```text
不采用裸 C-full。
```

## 6. 正式采用：C-full-guarded

正式命名：

```text
AUI Runtime Navigation / Screen Flow / Text Entry Productization v1
```

采用：

```text
方案 C-full-guarded。
```

它的含义：

```text
用一个统一系统边界处理 navigation / screen flow / submit-cancel / text entry。
能力范围比 B-min 更完整，避免后续推翻输入状态机。
但每个高风险能力都设 C-min 护栏，不做完整 CommonUI / InputField / IME 克隆。
```

本轮必须做：

```text
Submit / Cancel 优先级和 action。
Screen stack / active screen / back stack C-min。
default focus / focus restore。
Keyboard + gamepad normalized UI intent。
Action prompt report C-min。
modifiers 真实链路。
canvas runtime visible。
InputFieldText binding target。
authoring 新字段入口 C-min。
InputField edit mode。
draft text / caret / selection C-min。
backspace / delete / left / right / home / end。
TextChanged / TextSubmitted / TextCancelled。
IME composition event schema + preedit / commit / cancel 基础证据。
winit TextInput / IME event 转换。
runtime_player_winit + project_e2e_gate evidence。
```

本轮明确不做：

```text
rich text。
复杂 IME candidate window。
accessibility。
screen transition animation。
完整 clipboard。
完整 multi-line editor。
商业级 CommonUI action bar。
dirty/cache/batch 优化。
完整 touch / mobile virtual keyboard。
完整多人本地用户输入路由。
```

## 7. 核心设计

### 7.1 输入归一：AUI UI Intent

新增或扩展：

```text
AuiUiIntentKind：
  MoveUp
  MoveDown
  MoveLeft
  MoveRight
  FocusNext
  FocusPrevious
  Submit
  Cancel
  TextInput
  TextCompositionStart
  TextCompositionUpdate
  TextCompositionCommit
  TextCompositionCancel
  TextEditCommand

AuiTextEditCommand：
  MoveCaretLeft
  MoveCaretRight
  MoveCaretHome
  MoveCaretEnd
  Backspace
  Delete
  SelectLeft
  SelectRight
  SelectAll
```

规则：

```text
RuntimeInputEvent 先归一成 AuiUiIntent，再进入 AUI interaction。
Keyboard / Gamepad 不直接触发项目 gameplay action；AUI consumed 后再 filter_consumed_events。
Submit / Cancel 是 AUI framework 级 intent，项目侧只接收业务 action。
方向键 / dpad / left stick C-min 统一为 Move intent。
```

优先级：

```text
TextEditing mode 优先处理文本编辑键。
Modal / active screen 优先处理 screen-local navigation。
普通 Navigation mode 处理 focus move / submit / cancel。
未被 AUI 消费的输入才进入 gameplay InputResolver。
```

### 7.2 输入模式：Navigation / TextEditing / ModalBlocking

新增或扩展：

```text
AuiInputMode：
  Navigation
  TextEditing { node_id }
  ModalBlocking { modal_root }
```

说明：

```text
ModalBlocking 不新增架构层，只是 AuiInteractionState 内的当前输入上下文。
TextEditing 是 InputField 进入编辑时的 runtime transient state。
Navigation 是默认菜单/按钮/列表焦点模式。
```

切换规则：

```text
PointerDown / Submit 命中 InputField：
  进入 TextEditing。
  初始化 draft_text / caret / selection。

TextSubmitted：
  退出 TextEditing。
  生成 AUI action，不直接写 AUI Document。

TextCancelled：
  恢复 original_text。
  退出 TextEditing。

Screen push / modal open：
  进入对应 active screen / modal context。
  设置 default focus。

Screen pop / modal close：
  restore previous focus。
```

### 7.3 Submit / Cancel

新增或扩展：

```text
AuiCommandKind::Submit
AuiActionEvent::Submit
AuiActionEvent::TextSubmitted
AuiActionEvent::TextChanged
AuiActionEvent::TextCancelled
```

Submit 规则：

```text
TextEditing mode：
  Enter / Gamepad A 根据 input_field.submit_behavior 提交文本或插入换行。
  v1 默认为 single-line submit。

Navigation mode：
  focused interactable node 生成 Submit command。
  如果 node 有 action_id，生成对应 AuiActionEvent::Submit。
  如果 node 是 Button，可复用 click action_id，但 report 必须区分 source=submit。
```

Cancel 规则：

```text
TextEditing mode：
  Esc / Gamepad B 先生成 TextCancelled，不弹 screen。

Modal active：
  Cancel 生成 screen/modal back intent，不直接关闭项目 UI。

Screen stack 非空：
  Cancel 生成 PopScreen intent 或 Back action。

无 AUI 可消费：
  输入继续进入 gameplay。
```

禁止：

```text
Submit / Cancel 直接修改 ECS。
Submit / Cancel 直接关闭 AUI Document 中的节点。
Submit / Cancel 由 IR 读取原始键盘或手柄状态实现。
```

### 7.4 Screen Flow / Back Stack

新增或扩展：

```text
AuiScreenStackState：
  active_stack: Vec<AuiScreenStackEntry>
  last_popped_screen_id: Option<String>
  focus_restore_count: usize

AuiScreenStackEntry：
  screen_id
  document_path
  canvas_id
  root_node_id
  default_focus_node_id
  previous_focus_node_id
  modal
  can_cancel
```

Document 侧允许声明：

```text
AuiCanvas / root node metadata：
  screen_id
  default_focus_node_id
  cancel_action_id
  submit_action_id
```

规则：

```text
声明 metadata 是资源结构，不是 runtime screen stack。
active_stack 是 runtime transient state，不写回 AUI Document。
PushScreen / PopScreen / ReplaceScreen 是 AUI runtime command 或 project action intent。
真正打开哪个项目界面，本轮统一走 canvas runtime visible：
  AUI Document 可声明 canvas 初始 visible / screen metadata。
  AuiInteractionState 保存 canvas visibility override。
  layout / present / hit-test 根据有效 visible 过滤 canvas。
  PushScreen 使目标 canvas runtime visible=true。
  PopScreen 使 top canvas runtime visible=false，并恢复 previous focus。
不走 ProjectUiStateSnapshot 切 UI 结构；ProjectUiStateSnapshot 只负责显示数据。
```

v1 只做：

```text
单用户 screen stack。
default focus。
pop 后 focus restore。
Cancel -> top screen back intent。
report 证明 stack push/pop/restore。
```

不做：

```text
screen transition animation。
route graph editor。
multi-player local user focus。
CommonUI Action Domain。
```

### 7.5 Gamepad Navigation

新增或扩展：

```text
AuiNavigationInputSource：
  Keyboard
  Gamepad

AuiGamepadIntent：
  MoveUp / MoveDown / MoveLeft / MoveRight
  Submit
  Cancel
```

规则：

```text
InputMapping 负责把具体设备输入转成 runtime input。
AUI Runtime 只消费归一后的 UI intent，不绑定某个手柄厂商按键。
dpad / left stick C-min 必须有 debounce / repeat guard，避免一帧跳多格。
Move intent 复用 215 的 AuiNavigationMode / AuiNavigationRef / focus-visible auto-scroll。
```

Action prompt C-min：

```text
本轮只输出 report / summary：
  current_input_source
  focused_node_id
  available_ui_actions
  submit_label
  cancel_label

不做商业级屏幕底部按钮提示条。
```

### 7.6 InputField Text Editing

新增或扩展：

```text
AuiInputFieldState：
  node_id
  original_text
  draft_text
  caret_index
  selection_anchor
  selection_focus
  composition: Option<AuiTextCompositionState>
  dirty

AuiTextCompositionState：
  preedit_text
  cursor_start
  cursor_end
  active
```

规则：

```text
draft_text 是 runtime transient state，不写回 AUI Document。
输入框显示文本优先使用 draft_text + composition preedit。
TextChanged 只是 action/report，不直接更新 ProjectUiStateSnapshot。
TextSubmitted 后由 Project Logic 决定是否写项目状态，再由 ProjectUiStateSnapshot 回流显示。
InputField 显示回流使用 AuiBindingTarget::InputFieldText。
```

本轮支持：

```text
single-line InputField。
caret left / right / home / end。
backspace / delete。
selection C-min：select all / shift-left / shift-right 或等价最小选择范围。
TextChanged / TextSubmitted / TextCancelled。
max_length / read_only / placeholder C-min。
```

本轮不做：

```text
multi-line。
rich text。
clipboard 完整系统。
password masking。
undo / redo。
bidi / grapheme cluster 完整编辑。
复杂输入法候选窗位置。
```

字符索引规则：

```text
外部 report 不暴露 UTF-8 byte index。
v1 内部可以先以 char index 做 C-min。
遇到无法安全编辑的复杂 unicode cluster，必须输出 diagnostic，不允许静默破坏字符串。
```

### 7.7 IME schema

扩展 RuntimeInputEvent：

```text
TextInput { text }
ImePreedit { text, cursor_start, cursor_end }
ImeCommit { text }
ImeCancel
```

规则：

```text
winit / platform event 只负责转换为 RuntimeInputEvent。
AUI Runtime 只在 TextEditing mode 消费 IME event。
ImePreedit 更新 composition preedit，不提交 draft_text。
ImeCommit 写入 draft_text 并生成 TextChanged。
ImeCancel 清空 composition。
runtime_player_winit 必须接入 Character / IME event source。
IME platform coverage 必须进入 report。
```

验收不要求：

```text
真实候选窗 UI。
不同输入法完整兼容。
mobile virtual keyboard。
复杂 CJK shaping 重排。
```

但必须有：

```text
runtime input schema。
headless test。
runtime_player_winit summary evidence。
project_e2e_gate report evidence。
```

### 7.8 Runtime 链路

目标链路：

```text
RuntimeInputFrame
  -> AuiUiIntent normalize
  -> AuiInteractionSystem
  -> AuiInputMode / AuiScreenStackState / AuiInputFieldState
  -> AuiCommand / AuiActionEvent
  -> consumed event indices
  -> filter_consumed_events
  -> gameplay InputResolver
  -> Project Logic handles business action
  -> ProjectUiStateSnapshot updates visible data
  -> AUI Binding / Layout / Draw / Present
```

关键边界：

```text
AUI Runtime 可以保存 focus / screen stack / text draft / composition。
AUI Document 只保存结构、默认 focus、screen metadata、action_id、binding path。
ProjectUiStateSnapshot 只提供显示数据，不成为 text edit buffer。
Renderer 只显示 AUI resolved frame，不处理 Submit / IME / screen stack。
```

## 8. 复杂打飞机项目例子

### 8.1 暂停菜单

```text
Esc / Gamepad Start：
  Project Logic 请求 PushScreen(pause_menu)。

AUI Runtime：
  active_stack push pause_menu。
  focus = resume_button。

方向键 / dpad：
  在 Resume / Settings / Quit 间移动。

Enter / A：
  生成 Submit action。

Esc / B：
  生成 Cancel / PopScreen intent。
```

### 8.2 装备菜单

```text
装备格是 AUI subtree，不是 Scene Entity。
Grid focus 由 215 navigation + 216 gamepad intent 驱动。
Submit 选择装备格，生成 action_id=equipment.select_slot。
Cancel 返回上一个 screen。
真正换装、校验槽位、扣资源由 Project Logic / RuleSlot 处理。
```

### 8.3 名称输入框

```text
InputField 获得 Submit 或 PointerDown 后进入 TextEditing。
用户输入 draft_text。
IME preedit 只显示预编辑，不提交。
Enter 提交 TextSubmitted。
Esc 取消 TextCancelled，恢复 original_text。
Project Logic 决定是否接受文本，并通过 ProjectUiStateSnapshot 更新显示。
```

## 9. AI-first 报告

新增报告：

```text
aui-runtime-navigation-screenflow-textentry-productization-report.v1
complex-shooter-aui-runtime-navigation-screenflow-textentry-productization-report.v1
```

核心字段：

```text
schema_version
status
input_mode_before
input_mode_after
normalized_ui_intent_count
keyboard_intent_count
gamepad_intent_count
submit_count
cancel_count
screen_stack_push_count
screen_stack_pop_count
active_screen_id
default_focus_applied_count
focus_restore_count
text_edit_session_count
text_changed_count
text_submitted_count
text_cancelled_count
caret_move_count
selection_change_count
ime_preedit_count
ime_commit_count
ime_cancel_count
ime_platform_coverage
consumed_event_count_by_kind
gameplay_input_filtered_count
action_prompt_reported
focusable_derived_from_interactable
deferred_flags
diagnostics
next_actions
```

必须保留 deferred flags：

```text
rich_text_deferred=true
ime_candidate_window_deferred=true
accessibility_deferred=true
screen_transition_animation_deferred=true
clipboard_full_deferred=true
multi_line_text_edit_deferred=true
common_ui_action_bar_deferred=true
dirty_cache_batch_deferred=true
touch_virtual_keyboard_deferred=true
multi_user_input_deferred=true
```

报告必须能回答：

```text
这次输入是不是被 AUI 消费了？
当前处于 Navigation 还是 TextEditing？
Submit 到底触发了哪个 node / action？
Cancel 是取消输入框、关闭 modal，还是返回 screen？
screen stack 是否真的 push/pop？
focus 是否按 default / restore 移动？
文本 draft 是否改变？
IME preedit / commit 是否进入 AUI Runtime？
未实现的复杂能力是不是明确 deferred？
```

## 10. 拟施工 Gate

正式施工文档生成时，建议按以下 Gate 拆：

### Gate A：schema / intent / report

目标：

```text
新增 AuiUiIntent / AuiInputMode / AuiScreenStackState / AuiInputFieldState schema。
扩展 RuntimeInputEvent TextInput / IME events。
扩展 AuiCommandKind / AuiActionEvent Submit / TextChanged / TextSubmitted / TextCancelled。
扩展 RuntimeInputFrame modifiers 真实链路。
扩展 AuiBindingTarget::InputFieldText。
新增或确认 AuiNode focusable 字段；旧数据兼容时可从 interactable 推导。
扩展 AUI authoring schema_path：
  navigation / focusable / placeholder / maxLength / readOnly / submitBehavior
  screenId / defaultFocusNodeId / cancelActionId / submitActionId / canvasVisible。
新增 productization report struct。
```

测试：

```powershell
cd rust
cargo test -p engine_input text_input
cargo test -p engine_runtime aui_navigation_screenflow_textentry_schema
```

### Gate B：Submit / Cancel priority

目标：

```text
Submit 触发 focused interactable node。
Cancel 按 TextEditing -> Modal -> ScreenStack -> gameplay pass-through 顺序处理。
consumed_event_count_by_kind 可证明 key/gamepad 被过滤。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_submit_cancel
```

### Gate C：Screen stack / focus restore

目标：

```text
PushScreen / PopScreen / ReplaceScreen C-min。
Screen visible 切换统一走 canvas runtime visible，不写回 AUI Document。
default focus。
pop 后 restore previous focus。
Cancel 生成 back intent。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_screen_flow
```

### Gate D：Gamepad navigation intent

目标：

```text
Keyboard / gamepad 输入归一为 AuiUiIntent。
gamepad v1 先支持 schema + synthetic/headless gamepad input event；真实设备生态 deferred。
dpad / stick C-min repeat guard。
Move intent 复用 215 focus navigation 和 focus-visible auto-scroll。
Action prompt report C-min。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_gamepad_navigation
```

### Gate E：InputField edit state

目标：

```text
InputField 进入/退出 TextEditing。
draft_text / original_text / caret / selection C-min。
backspace / delete / left / right / home / end。
TextChanged / TextSubmitted / TextCancelled。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_input_field_text_edit
```

### Gate F：IME schema and evidence

目标：

```text
TextInput / ImePreedit / ImeCommit / ImeCancel 进入 RuntimeInputEvent。
runtime_player_winit 把 Character / IME event 转为 RuntimeInputEvent，不污染普通 KeyDown。
AUI Runtime 在 TextEditing mode 消费。
report 输出 preedit / commit / cancel 证据。
report 输出 ime_platform_coverage。
```

测试：

```powershell
cd rust
cargo test -p engine_input ime
cargo test -p engine_runtime aui_ime
```

### Gate G：runtime_player_winit / project_e2e_gate

目标：

```text
NativeAuiPresentSummary 输出 navigation / screen flow / text entry evidence。
project_e2e_gate 输出 complex shooter report。
```

测试：

```powershell
cd rust
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate aui_runtime_navigation_screenflow_textentry
cargo test -p project_e2e_gate
```

## 11. 验收标准

必须满足：

```text
Enter / Space / Gamepad A 能对 focused Button/MenuItem 生成 Submit action。
Esc / Gamepad B 在 TextEditing 时先取消文本编辑，不误关 screen。
Esc / Gamepad B 在 screen stack 中生成 back intent，并输出 stack pop evidence。
default focus 在 screen push 后生效。
focus restore 在 screen pop 后生效。
gamepad move intent 能移动焦点，并复用 215 的可见性滚动。
InputField 能进入 edit mode。
draft_text 会随 TextInput / key edit command 改变。
caret / selection C-min 有 report 证据。
TextSubmitted / TextCancelled 明确区分。
IME preedit 不提交 draft_text，IME commit 才写入 draft_text。
所有被 AUI 消费的输入不会继续进入 gameplay InputResolver。
```

不允许用以下方式冒充完成：

```text
只新增 Submit enum，但没有 focused node 行为。
只把 Esc 映射成 Cancel action，但没有优先级和 screen/text mode 证据。
只把 InputField 当普通 Text 改字符串，没有 edit buffer / caret / report。
只接收 TextInput，不处理 IME preedit / commit / cancel schema。
把 screen stack 写进 AUI Document。
让 IR 读取键盘/手柄来实现 UI 状态机。
```

## 12. AI 和用户如何理解

用户心智：

```text
我在 AUI Document / Scene 统一 authoring 里做菜单和输入框。
按钮、装备格、设置项可以被鼠标、键盘、手柄操作。
一个 screen 打开后会自动聚焦默认按钮。
返回键会按直觉先退出输入，再关弹窗，再返回上级菜单。
输入框里的临时文字是运行时草稿，提交后项目逻辑决定是否保存。
```

AI 心智：

```text
改 UI 结构：改 AUI Document。
改菜单默认焦点和 action_id：改 AUI metadata / node fields。
改输入/导航 runtime 行为：改 Rust AUI Runtime。
改提交后的业务后果：改 Project Logic / RuleSlot。
验证行为：看 productization report / e2e report。
```

禁止 AI 做：

```text
不要把 focus / screen stack / text draft 写回 AUI Document。
不要新增 Logic Ownership Router / Architecture Guard。
不要把 AUI Node 改成 Runtime ECS Entity。
不要用 IR 实现 UI event loop / IME / focus navigation。
不要在 engine_runtime 写 Player / Weapon / Equipment 等项目语义。
```

## 13. 自审

### 13.1 是否增加新架构层

```text
没有。
本方案只扩展现有 Rust AUI Runtime Core、engine_input 和 report。
```

没有新增：

```text
独立 AUI Designer。
运行时 Widget 对象层。
CommonUI Action Router 克隆。
Logic Ownership Router。
UI 脚本系统。
```

### 13.2 是否范围太大

```text
C-full-guarded 的范围确实大于 B-min，但它把同一套输入状态机一次性定清楚。
风险通过 Gate 和明确 deferred flags 控制。
```

最容易膨胀的部分已压住：

```text
IME 只做 schema + preedit/commit/cancel 证据。
Text edit 只做 single-line C-min。
Action prompt 只做 report C-min。
Screen flow 只做 stack/default/restore，不做动画和 route graph。
Gamepad 只做 normalized intent，不做完整设备生态。
```

### 13.3 是否符合复杂打飞机目标

```text
符合。
开始菜单、暂停菜单、设置菜单、装备菜单、确认弹窗、命名输入都需要这条链路。
```

### 13.4 是否符合 AI-first

```text
符合。
所有新增 runtime 状态都有 schema / report / deterministic gate。
AI 可以通过 report 判断焦点、screen、submit/cancel、text draft、IME 是否真实工作。
```

### 13.5 是否和 195 / 199 / 209 / 210 / 211 / 213 / 214 / 215 冲突

```text
不冲突。
AUI Document 仍是结构真相。
ProjectUiStateSnapshot 仍是显示数据入口。
RuntimeRenderer 仍只消费 frame，不处理 UI input。
AUI Node 仍不是 Runtime ECS Entity。
复杂 UI runtime 机制仍在 Rust AUI Runtime。
IR 仍不处理 hit-test / focus / drag / scroll / IME / screen flow。
```

## 14. 结论

```text
采用方案 C-full-guarded。
下一步如果进入施工，应先根据本文档生成自动化施工文档，并在施工文档中严格拆 Gate A-G、自审施工范围、边施工边测试。
```
