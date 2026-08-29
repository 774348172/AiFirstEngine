# 248-Native Editor UI Interaction Reachability / Layout Convergence v1 方案

> 状态：248-A/B/C 已全部施工完成并归档；247 P0-1 已完成。
> 方案日期：2026-07-12。  
> 选题来源：`247-人工从空项目创建复杂打飞机并导出Windows可玩-系统讨论优先级.md` P0-1。  
> 正式选择：Editor-only Retained Widget Tree，替换旧的面板内手写 DrawList / HitRegion 路线，不建立并行 UI 框架。  
> 并行边界：不修改 245/246，不取得施工授权；未来施工文档必须排在既有施工队列之后，除非用户显式重新排序。
> 自审修订：补齐 retained reconcile 生命周期、UI 状态唯一归属、UI-local action 边界、Taffy 布局内核决策、真实像素与 DPI 权威证据，以及三份串行施工包规则。

## 1. 这个系统解决什么问题

本系统治理的是引擎编辑器自身 UI，不是项目侧 AUI。

它要保证：

```text
用户看得到的按钮真的能点击；
按钮画在哪里，点击区域就在哪里；
隐藏、裁剪或滚出区域的控件不会继续接收输入；
窗口缩放和 DPI 改变后，文字、布局和命中仍然一致；
Dock、Tab、Scroll、Focus、Menu、Modal 不互相穿透；
控件不能操作时，用户能看到 Disabled / Busy / Failed 原因；
AI 和测试可以通过稳定 Widget ID 找到控件、检查状态并回放真实点击。
```

它不是视觉换肤，也不以颜色、圆角和动画为目标。它只收敛会阻断人工创建复杂打飞机项目的编辑器 UI 可达性、布局、焦点和反馈。

## 2. 为什么不能继续只补 HitRegion

当前正式链路已经存在：

```text
EditorUiModel
  -> SelfUiRenderer::build_draw_list
  -> UiDrawList + 手写 HitRegion
  -> hit_test
  -> EditorInputRouter
  -> UiCommand
  -> EditorSession
```

但当前实现同时存在以下结构问题：

### 2.1 两套布局事实

```text
editor_window_winit::DockLayoutManager
  -> 拥有 PanelRegistry 和固定 Dock 结构

editor_ui_renderer::layout_for
  -> 实际计算渲染使用的另一套 panel rect
```

`NativeEditorApplication` 持有 `DockLayoutManager`，但 `SelfUiRenderer` 没有使用它生成实际界面。这与 121 已确定的“EditorMainFrame 引用 DockLayoutManager 生成可见面板布局”不一致。

### 2.2 绘制和命中分别手写

面板代码先写：

```text
DrawCommand::Rect
DrawCommand::Text
```

然后再手写：

```text
HitRegion { rect, target, enabled, command_id, ... }
```

两者没有共同的控件对象和共同的计算几何，任何局部修改都可能只改视觉、不改点击，或者只改点击、不改视觉。

### 2.3 缺少父子、裁剪和交互层级

当前 HitRegion 是平面数组，主要依赖逆序查找。它不能自然表达：

```text
父级 ScrollView 的有效裁剪；
Modal 对下层控件的阻断；
Menu / Popup 的覆盖范围；
Dock Stack 里非活动 Tab 的不可见性；
Pointer Capture；
控件到 Panel 的事件路径。
```

### 2.4 已有可见假控件

当前顶部菜单文字和底部 Project / Console / RuntimeTrace 标签页被绘制出来，但没有对应可操作控件合同。Hierarchy 没有通用滚动；Inspector 超出区域后直接停止生成，后续字段不可达。

### 2.5 DPI 合同不完整

真实窗口当前主要把 winit 的物理像素位置直接传入 EditorInputEvent，渲染布局也使用 surface 物理宽高。缺少统一的：

```text
physical window coordinates
  <-> logical editor coordinates
  <-> physical GPU pixels
```

这无法证明 Windows 100%、150%、200% DPI 下视觉和命中一致。

### 2.6 反馈主要停留在报告

`EditorCommandFeedback`、`reason_disabled` 和 `interaction_feedback` 已存在，但并未形成所有控件统一可见的 Tooltip / Status feedback。用户仍可能只看到“点了没反应”。

## 3. 成熟编辑器源码对标

### 3.1 Unreal Editor：Retained Slate Widget Tree

本地源码：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/SlateCore/Public/Widgets/SWidget.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/SlateCore/Private/Input/HittestGrid.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Slate/Private/Framework/Application/SlateApplication.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Slate/Private/Framework/Docking/SDockingTabStack.cpp
```

核心链路：

```text
SWidget Tree
  -> ArrangeChildren(FGeometry)
  -> OnPaint
  -> FHittestGrid
  -> FWidgetPath
  -> Preview / Target / Bubble event
```

采用点：Widget 的同一份 Geometry 同时服务布局、绘制、命中、导航和事件路径；Dock Tab 本身也是 Widget Tree 的一部分。

不照搬点：第一版不复制 Slate 全部属性绑定、声明宏、完整 Invalidation Root 和多用户输入系统。

### 3.2 Godot Editor：Retained Control / Container Tree

本地源码：

```text
<GODOT_SOURCE>/godot/editor/editor_node.cpp
<GODOT_SOURCE>/godot/editor/docks/editor_dock_manager.cpp
<GODOT_SOURCE>/godot/scene/gui/control.cpp
<GODOT_SOURCE>/godot/scene/main/viewport.cpp
```

Godot 编辑器直接用 `Control`、`Container`、`DockSplitContainer`、`DockTabContainer`、`Button` 等长期对象组成编辑器树。

命中时统一考虑：

```text
visible in tree
parent transform
clip contents
child order
mouse_filter
has_point
focus mode
```

采用点：编辑器 Shell、Dock 和普通控件使用同一结构规则；裁剪、焦点、Tooltip 和命中不由每个业务面板重新实现。

### 3.3 Unity Editor：Retained Shell + UI Toolkit / IMGUI Hybrid

本地源码：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/EditorWindow.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/GUIView.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/HostView.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Modules/UIElements/Core/VisualElement.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Modules/UIElements/Core/Panel.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Modules/UIElements/Core/IMGUIContainer.cs
```

Unity 不是纯 retained：

```text
EditorWindow / GUIView / HostView
  -> rootVisualElement
      -> UI Toolkit VisualElement Tree
      -> IMGUIContainer
          -> OnGUI / GUILayout
```

采用点：现代编辑器窗口优先使用 `rootVisualElement`；UI Toolkit 用同一 `VisualElement.layout/worldBound` 支撑布局、Pick、Focus 和事件分发。

不采用点：不保留一套长期手写 IMGUI 路线作为正式并行入口。Unity 的 Hybrid 主要是历史兼容成本，不应成为新引擎主动制造的双轨债务。

### 3.4 O3DE Editor：Qt Retained Widget Tree

官方源码参考：

```text
https://github.com/o3de/o3de/blob/development/Code/Editor/MainWindow.cpp
```

O3DE 以 Qt `QMainWindow`、`QDockWidget`、`QWidget` 组织编辑器。Qt 控件对象、父子关系、布局、焦点和事件长期存在。

采用点：复杂编辑器通常需要稳定的窗口、Dock、控件和焦点对象图，而不是让每个面板独立维护坐标表。

### 3.5 对标结论

并非所有成熟编辑器的每个控件都是纯 retained；Unity IMGUI 和部分 Dear ImGui 编辑器证明 Immediate UI 也能工作。

但成熟路线无论 retained 或 immediate，都必须统一：

```text
stable control identity
layout geometry
clip stack
focus / pointer capture
z-order / modal scope
draw and hit order
DPI transform
scroll visibility
```

当前项目的“面板内分别 push DrawCommand 和 HitRegion”没有形成上述任一成熟框架。考虑长期复杂原生编辑器目标，正式选择 Editor-only Retained Widget Tree。

## 4. 方案比较与正式选择

### 4.1 方案 A：逐面板修补

继续查找错位、遮挡和缺失 HitRegion，逐个修复。

优点是改动小；缺点是结构性漂移仍然存在，后续每个新控件都可能再次产生相同问题。不采用。

### 4.2 方案 B-min+：Immediate-style UiBuilder

保留 DrawList 主线，用统一 Button/Field/Tab helper 同时生成绘制和命中。

它可以满足短期 P0，但复杂 Dock、Modal、Popup、Focus Path、Pointer Capture 和长期 AI 结构检查仍需要不断给平面输出增加元数据。作为迁移技巧可以使用，不能作为最终 UI 真相。

### 4.3 方案 C-min：Editor-only Retained Widget Tree

正式采用。

```text
EditorUiModel
  -> EditorWidgetTree
  -> Layout / Clip / Pick / Focus Path
  -> UiDrawList
  -> WGPU Renderer
```

这不是在旧 UI 前面再叠加一层。终态规则是：

```text
面板不得手写最终 HitRegion。
面板不得维护独立 computed rect 真相。
layout_for() 与 DockLayoutManager 的重复布局必须收敛。
UiDrawList 只保留为渲染 DTO。
HitRegion 只允许由 WidgetTree 派生，用于迁移兼容、测试和报告。
EditorWidgetTree 是编辑器 UI 结构、计算几何、裁剪和拾取的唯一真相。
```

## 5. 真相层与职责边界

### 5.1 EditorUiModel：业务显示状态

继续负责：

```text
项目、Scene、Hierarchy、Inspector、Asset、Build、AI 等显示数据；
CommandAvailability；
Disabled / Busy / Failed 原因；
业务选择和运行状态的只读投影。
```

禁止把业务数据复制进 WidgetTree。Widget 不持有 Project、Scene、Prefab、AUI 或 RuntimePackage 的第二份业务真相。

### 5.2 EditorWidgetTree：UI 结构与计算几何

负责：

```text
Widget 父子关系和稳定 WidgetId；
布局约束与 computed geometry；
effective clip；
可见性和 z-order；
控件角色、Tooltip、Focusable、Pointer behavior；
CommandBinding；
Scroll、expanded 等节点本地 UI 状态；
Pick 和 WidgetPath。
```

状态唯一归属固定为：

```text
DockLayoutManager
  -> 持久 Dock placement、split ratio、active dock tab。

EditorWidgetTree node state
  -> scroll offset、tree/list expanded、popup open 等节点本地 UI 状态。

EditorFocusInputSystem
  -> hover、pressed、keyboard focus、pointer capture 等瞬时输入状态。

EditorUiModel / EditorSession
  -> 业务选择、字段草稿、项目数据、命令可用性和业务结果。
```

同一状态不得同时保存在两处。Widget snapshot 可以投影上述状态，但不能成为另一份可写真相。

### 5.3 EditorFocusInputSystem：引用 WidgetId 的交互状态

保留现有应用级输入状态职责，但收敛为稳定 WidgetId 引用：

```text
hovered_widget_id
pressed_widget_id
keyboard_focus_widget_id
pointer_capture_widget_id
active_panel_widget_id
```

不得同时保留另一套按 panel 特判的焦点真相。Asset Browser、AI Prompt、Inspector Field 等特殊路径最终都必须从 Widget role / state / event path 解析。

### 5.4 UiDrawList：纯渲染输出

继续作为 `editor_wgpu_renderer` 的稳定输入：

```text
Rect
Text
ImageTextureSlot
ViewportTextureSlot
Clip commands / effective clip metadata
```

它不拥有 Widget 生命周期、业务命令或焦点状态。

### 5.5 UiCommand：唯一业务动作入口

Widget 不保存任意业务闭包。可操作 Widget 只声明结构化绑定：

```text
command_id
payload hint / target reference
enabled
reason_disabled
source domain
```

事件路由解析绑定后仍进入现有 `EditorInputRouter -> UiCommand -> EditorSession`，不能绕过 Command Framework 直接修改业务状态。

### 5.6 UI-local Action：只修改 Widget/Dock 状态

Tab、Scroll、Popup 和 Splitter 不应为了改变纯 UI 状态伪造项目业务命令。Widget activation 明确分为：

```text
EditorWidgetAction
  ActivateTab
  ScrollBy / ScrollTo
  ToggleExpanded
  OpenPopup / ClosePopup
  ResizeSplit
  RequestFocus / ReleaseFocus

EditorCommandBinding
  -> 进入 EditorInputRouter -> UiCommand -> EditorSession
```

`EditorWidgetAction` 只能修改第 5.2 节明确列出的 UI-local 状态，不进入项目 Undo，不得创建、修改或删除 Scene、Prefab、AUI、Rule、Asset、Build Profile 等业务对象。任何业务副作用都必须使用 `EditorCommandBinding`。

## 6. C-min 核心数据模型

第一版只建立满足复杂编辑器主流程的最小类型，不复制完整 Web DOM、Slate 或 UI Toolkit。

```text
EditorWidgetTree
  root_id
  nodes
  revision
  layout_revision
  composition_revision

EditorWidgetNode
  widget_id
  semantic_path
  role
  parent_id
  children
  layout_style
  computed_geometry
  visibility
  visual
  interaction

EditorWidgetGeometry
  logical_rect
  effective_clip_rect
  z_order

EditorWidgetInteraction
  activation: EditorWidgetAction | EditorCommandBinding | None
  enabled
  reason_disabled
  focusable
  tooltip
  pointer_behavior
```

### 6.1 最小 Widget Role

```text
Root
Panel
Split
Stack
TabBar / Tab
ScrollView
Button
Toggle
Label
TextField / ValueField
List / Tree / Row
Menu / MenuItem
Overlay / ModalBarrier / Dialog
Image
ViewportSlot
Separator / Spacer
```

Role 是结构化语义，不等于每种 Role 都建立一个大型 Rust class 层次。实现可以使用数据 enum + focused behavior，避免过度面向对象化。

### 6.2 稳定 Widget ID

Widget ID 必须语义稳定、可预测、可序列化：

```text
toolbar.play
hierarchy.entity:<entity_id>
inspector.field:<field_id>
asset_browser.entry:<stable_asset_key>
bottom_tabs.console
build_export.command:<command_id>
```

禁止使用每帧自增 index、内存地址或临时 ECS entity 作为跨帧身份。

重复 ID、孤儿节点、循环父子关系、失效 parent 和失效 command binding 必须在构建时产生结构化诊断。

### 6.3 布局约束

C-min 至少支持：

```text
horizontal / vertical flow
fixed / min / max size
flex grow / shrink
padding / gap
split ratio
absolute overlay anchor
overflow visible / clip / scroll
visibility collapsed / hidden / visible
```

不做完整 CSS。布局字段保持小而明确。

C-min 正式选择成熟 Rust `taffy` 作为 block/flex/absolute/min-max/gap 等通用布局计算内核；Dock split ratio、Overlay stacking、scroll offset 和 effective clip 由 Editor Widget adapter 在其上实现。Bevy UI 已使用 Taffy 作为正式布局内核，本方案不重新手写通用 Flexbox。

施工 Gate A 必须先完成：

```text
Taffy 当前稳定版本、license、feature 和 MSRV 核对；
最小依赖 feature，只启用 C-min 实际需要的布局能力；
logical f32 输入和确定性输出验证；
文本 measure callback、absolute overlay、overflow/scroll adapter spike；
与 Cargo.lock、246 dependency policy 和构建预算的兼容检查。
```

如果 spike 证明 Taffy 无法满足核心合同，必须暂停并回填 248 方案，不得在施工中静默改成自研完整布局引擎。

### 6.4 Retained Compose / Reconcile 生命周期

Retained 不能只表示“有一个树类型”。动态 Hierarchy、Inspector、Asset 和 Workflow 必须通过稳定 ID 进行确定性 reconcile：

```text
EditorUiModel revision / Dock revision
  -> Panel widget declaration
  -> reconcile(previous tree, declared tree)
      reuse same WidgetId node and UI-local state
      create new WidgetId node
      remove stale node
  -> validate tree invariants
  -> layout dirty subtrees
  -> extract / pick
```

规则：

```text
相同 semantic WidgetId 且 role 兼容：复用节点和合法 UI-local state。
role 不兼容：显式 replace，并产生 debug/trace evidence。
节点移除：同步清理 focus、capture、hover、pressed 和孤立 popup。
动态列表 reorder：按 stable item key 复用，禁止按行号复用错误状态。
重复 ID：本次 compose 失败并显示诊断，不允许 last-write-wins。
业务 Model 未变化时，pointer move 不重新 compose 全树。
```

C-min 不引入 React 式通用 Virtual DOM 或任意 diff 插件 API；只实现 EditorWidgetNode 的确定性 keyed reconcile。

## 7. 唯一布局与 Dock 合同

`DockLayoutManager` 不再自己维护一套最终 PanelRect，同时 `layout_for()` 再计算另一套 rect。

正式关系：

```text
DockLayout persisted configuration
  -> Split / Stack / Tab Widget nodes
  -> WidgetTree layout
  -> computed geometry
```

规则：

```text
DockLayoutManager 负责可保存的 Dock 配置和 Panel placement。
EditorWidgetTree 负责把配置计算成唯一 geometry。
PanelRegistry 负责 panel_id 到 Widget subtree factory 的注册。
非活动 Dock Tab 保留结构和状态，但 visibility=collapsed，不绘制、不命中。
窗口 Resize 只使受影响布局 dirty，不建立第二套固定坐标表。
```

C-min 必须让固定 Dock、Resize 和真实 Tab 可用；自由拖拽 Dock、floating multi-window 可以保留为后续能力，但数据模型必须允许扩展。

## 8. 绘制、裁剪与拾取

### 8.1 单一 Geometry

同一个 `EditorWidgetGeometry` 必须同时用于：

```text
Draw extraction
effective clipping
pointer hit testing
focus navigation bounds
tooltip anchor
test screenshot annotation
```

禁止渲染器和 InputRouter 各自重新计算 rect。

### 8.2 裁剪

每个节点的 effective clip 为自身 clip 与祖先 clip 的交集。超出 effective clip 的部分：

```text
不绘制；
不命中；
不出现在 visible reachability 清单；
仍可作为 ScrollView 的不可见内容保留布局信息。
```

WGPU 输出需要明确 scissor/clip 边界，不能只在 CPU hit test 裁剪而继续把文本和图形画出父容器。

### 8.3 Pick 与事件路径

```text
physical pointer
  -> logical coordinates
  -> top-most visible widget pick
  -> WidgetPath(root ... target)
  -> minimal capture / target / bubble handling
  -> CommandBinding resolution
```

C-min 不需要复制完整 DOM event API，但必须支持：

```text
ancestor ScrollView 消费 wheel；
Button target 消费 click；
ModalBarrier 阻断下层；
pointer capture 支持拖拽和 splitter resize；
disabled target 返回可见原因而不是穿透到下层控件。
```

### 8.4 迁移期 HitRegion 兼容

为控制施工风险，第一阶段可以从 WidgetTree 自动导出 flat `HitRegion` 给现有 InputRouter 和历史测试：

```text
WidgetTree geometry / interaction
  -> derived HitRegionSnapshot
```

约束：

```text
Panel 代码禁止直接 push HitRegion。
derived HitRegion 不能反向成为布局真相。
同一 Widget 的视觉 rect、clip 后 hit rect 和 command metadata 必须自动一致。
终态输入优先使用 WidgetPath；HitRegion 保留为兼容 DTO 或报告证据。
```

这属于迁移 Adapter，不是第二套 UI 框架。

## 9. DPI 与坐标合同

编辑器内部统一使用 logical UI coordinates：

```text
winit physical position / physical window size
  -> divide by scale factor
  -> logical WidgetTree layout and pick
  -> multiply by scale factor
  -> physical WGPU viewport / scissor / glyph rasterization
```

规则：

```text
scale_factor 使用 f64，禁止继续把 1.5 round 成整数 2。
scale factor 只能在窗口输入边界和 GPU 输出边界转换。
Panel 和 Widget 不得自行读取或猜测系统 DPI。
字体、边框、Tooltip、drag threshold 和 min target size 使用 logical units。
ScaleFactorChanged 必须触发布局、glyph 和 surface 的一致更新。
```

当前 `NativeEditorWindowConfig.dpi_scale` 和部分窗口报告使用整数，无法准确表达 150%。施工必须先升级坐标/DPI 合同，再迁移 Widget geometry；禁止通过把 150% 当 100% 或 200% 让测试表面通过。

## 10. 焦点、滚动、Menu 与 Modal

### 10.1 Focus

```text
点击 focusable Widget -> keyboard focus。
Tab/Shift+Tab -> 在可见 focusable Widget 顺序中移动。
隐藏、删除或 collapse focused Widget -> 使用确定性 fallback。
Modal 打开 -> focus 被限制在 modal subtree。
窗口失焦 -> 清理 pressed/capture，保留或按策略释放 keyboard focus。
```

### 10.2 Scroll

Hierarchy、Inspector、Workflow、Asset Browser、Report、Console 等长内容必须使用同一 ScrollView 合同。Scroll offset 是纯 UI 状态，以稳定 WidgetId 保存；内容变化后执行合法范围 clamp。

禁止继续通过 `.take(n)` 或超界 `break` 冒充滚动。

### 10.3 Tab

Tab 必须是可命中的 Widget：

```text
active tab -> subtree visible and interactive
inactive tab -> subtree collapsed
tab click -> editor UI state command / action
```

禁止继续绘制 Project / Console / RuntimeTrace 标签但同时把所有面板叠在同一底部区域。

### 10.4 Menu / Popup / Modal

使用 Overlay subtree：

```text
Popup/Menu 在普通内容之上命中。
点击外部可以关闭 Popup，但不能误触下层业务控件。
ModalBarrier 覆盖可交互窗口范围并阻断下层。
Disabled MenuItem 显示原因且不能穿透。
```

### 10.5 Text Editing / IME 边界

248 不新建第二套字段草稿和文本提交系统：

```text
Widget TextField
  -> 提供 geometry、focus、caret/selection visual 和 TextInput/IME event target

现有 PropertyEditBuffer / AI prompt draft / Asset search state
  -> 继续拥有实际 draft text、commit、cancel 和 validation
```

Widget 节点只引用稳定 field ID 和显示快照；不能复制一份长期 draft text。IME preedit 属于瞬时编辑会话状态，可以由现有文本输入/编辑会话持有，但 commit 后必须进入对应的正式 draft owner。P0-1 验证输入框可见、可聚焦、命中和不泄漏输入；复杂文本编辑语义继续继承 216 已有合同。

## 11. 人类反馈合同

所有可操作 Widget 必须具有清晰状态：

```text
normal
hovered
pressed
focused
disabled + reason
busy + progress/status
failed + reason + next action when available
```

第一版至少实现：

```text
图标按钮 Tooltip；
disabled hover/click 显示原因；
最近命令结果进入可见 Status feedback；
busy 控件不能重复触发冲突命令；
failed 状态不会只存在于 report。
```

自然语言反馈来自结构化诊断，不替代 command_id、widget_id、状态和错误码。

## 12. AI 适配合同

Retained Widget Tree 必须比当前平面 DrawList 更容易被 AI 检查，而不是只方便渲染。

验证模式可以输出：

```text
EditorWidgetTreeSnapshot
  widget_id
  semantic_path
  role
  parent_id
  visible
  logical_rect
  effective_clip_rect
  enabled
  command_id
  reason_disabled
  focused / hovered / pressed
  tooltip
```

AI 和自动化测试可以执行：

```text
find widget by stable id / role / command
assert visible and reachable
resolve center in logical coordinates
convert to physical coordinates
inject real-window pointer event
assert command / model / tree revision changed
```

禁止让测试只按截图猜按钮位置，也禁止只验证“command 存在”而不验证真实 UI 可达。

## 13. Runtime 与性能边界

本系统只进入 Native Editor：

```text
不链接 Runtime Player；
不进入导出 Windows Game.exe；
不改变项目 AUI Runtime；
不增加游戏帧开销和发布包体。
```

Editor 性能规则：

```text
Widget identity 和结构长期存在。
Model revision、layout、style、paint、interaction 使用分域 dirty 标记。
Pointer move 只执行 pick/hover 必要更新，不重建业务 Model。
Scroll/hover/pressed 优先局部 invalidation。
Reachability report、完整 tree snapshot 和像素审查只在验证档开启。
正常 Editor 不每帧序列化 WidgetTree 或生成重型报告。
```

C-min 可以先实现确定性全树 layout，再通过基准决定是否增加空间索引或更细 invalidation；禁止在没有性能证据时先复制 Slate 的全部优化复杂度。

Retained reconcile 的最低性能证据必须区分：

```text
compose/reconcile time
layout time
draw extract time
pick time
node reuse/create/remove counts
dirty subtree count
```

不得只报告总 frame time 后无法定位退化来源。

## 14. 第一版迁移范围

为避免终态双轨，当前所有可见 Native Editor 面板都必须迁移到 WidgetTree authoring：

```text
Project Launcher
Main Menu / Toolbar
Hierarchy
Game / Scene Viewport shell
Inspector
Bottom Tabs
Asset / Project Browser
Workspace / Workflow Rail
Input Mapping Authoring
Build / Export
AI Panel
Console
Runtime Trace / Report entry surface
```

迁移可以分 Gate 完成，但正式完成前不得留下“新面板走 WidgetTree、旧面板继续任意手写 HitRegion”的长期公开入口。

### 14.1 代码归属建议

施工前根据 246 代码治理结果复核最终文件，但职责方向固定为：

```text
editor_ui_model
  -> 业务显示 Model，不依赖 WidgetTree renderer internals。

editor_ui_renderer
  -> 第一版直接承载 Widget data、composer/reconcile、layout adapter、clip、extract、pick。

editor_window_winit
  -> window/DPI boundary、application lifecycle、focus/input state、command dispatch。

editor_wgpu_renderer
  -> 只消费 UiDrawList 和 clip/scissor 输出。
```

不得建立 `editor_ui_renderer -> editor_window_winit` 反向依赖。

C-min 第一版不新增 `editor_widget_tree` crate。先在现有 `editor_ui_renderer` 内按模块形成深边界，避免为了类型名称增加 crate 层。只有 246 的确定性依赖/规模证据证明独立 crate 能形成真实复用和依赖收敛时，才允许在后续方案中拆出；施工不得自行改架构。

## 15. 与现有系统的关系

### 15.1 继承

```text
121 Native Editor Application Shell
127 Interaction Feedback / Command Availability
150 AI-first Editor Command Framework
171 Native Editor Real Interaction Validation Gate
173 Real Window Interaction Smoke / Screenshot Gate
177 Editor Visual Regression / Golden Image Gate
```

### 15.2 修正

```text
121 的 DockLayoutManager 从“存在但不驱动渲染”修正为真实布局配置入口。
127 的 reason_disabled 从 report/局部视觉修正为 Widget 统一反馈。
171 的按 HitRegion 中心回放升级为按 WidgetId -> geometry -> physical click 回放。
177 的结构化 DrawList baseline 增加 WidgetTree reachability 和真实像素证据。
```

### 15.3 不替代

```text
不替代 EditorUiModel。
不替代 EditorCommandSystem / UiCommand。
不替代 EditorSession transaction。
不替代项目 AUI Document / AUI Runtime。
不替代 P0-3 的真实命令上下文收敛。
```

P0-1 只保证控件本身可见、可达、可路由；某个业务命令缺 path、selection 或 document context 时，仍由 P0-3 治理。

## 16. 验证档位与 Report

新增或升级的 UI 可达性证据只在 Editor/Test 路径运行：

```text
Off
  正常 Editor，只执行必要 layout/pick/render。

Summary
  输出不可达控件数、重复 ID、越界、裁剪、缺命令、缺反馈摘要。

Trace
  输出完整 WidgetPath、geometry、event route、command resolution 和截图标注。
```

正式 Runtime 没有该 Report 档位。

建议报告：

```text
EditorUiReachabilityReport v1
  schema_version
  scenario_id
  viewport / dpi
  widget_count
  visible_interactive_count
  reachable_count
  duplicate_id_count
  clipped_interactive_count
  missing_command_count
  missing_disabled_reason_count
  draw_hit_geometry_mismatch_count
  focus_route_status
  diagnostics
  screenshot evidence: kind / width / height / rgba_hash / artifact_path
```

Report Panel 只注册并展示报告，不形成新的 UI 真相或新的报告总线。

`metadata-only` screenshot 只能证明报告管线存在，不能满足 P0-1 的像素验收。权威结果必须标记 `kind=actual_rgba_readback` 或等价真实 framebuffer/window capture，并提供非伪造的 RGBA hash 与 artifact；缺少真实像素时状态只能是 `not_evaluated/environment_blocked`，不能记为 passed。

## 17. 验收矩阵

### 17.1 结构测试

```text
Widget ID 唯一且稳定。
父子关系无循环、无孤儿。
每个 visible interactive Widget 有合法 command binding 或明确的 UI-local action。
disabled Widget 有 reason。
inactive/collapsed/clip-out Widget 不可命中。
Draw extraction 和 pick 使用同一 geometry。
Modal/Menu 不穿透。
Scroll offset clamp 正确。
focused Widget 删除或隐藏后 fallback 正确。
```

### 17.2 尺寸矩阵

```text
1280x720
1600x900
1920x1080
```

每个尺寸必须验证关键控件无重叠、无窗口外命中、可通过 Scroll/Tab 访问。

### 17.3 Windows DPI 矩阵

```text
100%
150%
200%
```

每档验证：

```text
logical layout
physical rendering
physical pointer -> logical pick
glyph and control alignment
tooltip anchor
drag threshold
```

DPI 采用两层证据：

```text
默认确定性 Gate
  -> 注入 scale_factor=1.0 / 1.5 / 2.0
  -> 验证 logical/physical round-trip、layout、clip、pick 和 glyph plan

Windows 权威 Gate
  -> 真实 Per-Monitor DPI Aware window
  -> 记录 OS 报告的实际 f64 scale factor
  -> 真实 pointer event + actual pixel capture
```

测试机不具备目标 DPI 环境时必须明确 `environment_blocked`，不能用整数 headless `dpi_scale` 或截图 metadata 冒充真实 150%/200% 通过。

### 17.4 真实窗口交互

不能只调用 Widget handler。至少覆盖：

```text
真实 winit window；
真实 surface present；
按 Widget geometry 转换出的物理坐标点击；
wheel scroll；
Tab focus；
Popup / Modal open-close；
Resize / DPI change；
结构化 event route / command / model evidence；
actual framebuffer/window pixel screenshot，而不是 metadata-only placeholder。
```

### 17.5 P0-1 主流程面板

```text
Project Launcher
Workspace / Workflow Rail
Hierarchy / Scene / Inspector
Asset Browser
Prefab / Rule / Input / AUI Authoring entry
AI Panel
GameView / Play Controls
Build / Export
Report Panel
```

每个面板至少有一条成功操作、一条 disabled/busy/failed 反馈、一条 Resize/DPI/Scroll 或 Tab 相关验证。

## 18. 串行施工包与 Gate 建议

248 的范围包括 retained core、全部现有面板迁移和真实窗口验收。自审判定它不适合压成一份不可中断的巨型施工文档；未来必须生成三份同属 248 的串行施工包，每份单独自审、单独进入 `待执行/`，并始终遵守 `当前/` 最多一份施工文档：

```text
248-A Retained Core / Single Geometry
  -> Widget schema、keyed reconcile、Taffy、DPI、clip、extract、pick、derived HitRegion

248-B Native Editor Panel Migration / Cutover
  -> Dock、Tab、Scroll、Focus、Modal 和全部当前可见面板迁移
  -> 删除 panel 手写 HitRegion 与重复 layout_for 真相

248-C Real-window Reachability / DPI / Pixel Acceptance
  -> Report、尺寸矩阵、DPI 矩阵、真实点击、actual pixel screenshot、最终回归
```

规则：

```text
三份施工包必须按 A -> B -> C 激活，不能并行修改同一 UI 主线。
A 完成不代表 P0-1 完成；B/C 未完成时 248 仍是 partial。
A 的迁移 Adapter 不得取得新增业务面板的长期扩展权。
B 完成前禁止把 WidgetTree 宣称为唯一终态真相。
C 通过后才允许更新 247 的 P0-1 完成状态。
任何施工包激活前都必须按当时代码和 246 结果重做 baseline/self-review。
```

方案级 Gate 顺序为：

```text
Gate A：baseline 与 WidgetTree schema / invariant
Gate B：logical DPI、layout、clip、extract、pick 核心
Gate C：Dock / Tab / Scroll / Focus / Modal 核心行为
Gate D：Launcher / Toolbar / Hierarchy / Inspector / Viewport 迁移
Gate E：Asset / Workflow / Input / Build / AI / Report 等面板迁移
Gate F：移除 layout_for 重复真相和 panel 手写 HitRegion 入口
Gate G：EditorUiReachabilityReport 与结构回归
Gate H：尺寸、DPI、真实窗口点击和像素 screenshot 矩阵
Gate I：受影响域与最终权威回归、文档同步和归档
```

每个 Gate 先跑定向测试，再跑受影响 crate；最终候选冻结后运行一次权威整体回归。不得在 245/246 或其它施工仍占用 `施工文档/当前/` 时开始本方案施工。

## 19. 风险与控制

### 19.1 新树变成第二份业务状态

控制：Widget 只持有 UI 结构、计算几何和短生命周期交互状态；业务值始终来自 EditorUiModel，业务修改始终走 UiCommand。

### 19.2 形成新旧双轨

控制：迁移期 HitRegion 必须由 WidgetTree 派生；禁止新增 panel 手写 HitRegion；正式完成前迁移全部当前可见面板并删除重复布局入口。

### 19.3 C-min 膨胀成完整浏览器布局引擎

控制：只实现编辑器当前需要的 Role 和布局约束；不做 CSS selector、Web DOM、动画系统、任意脚本回调和项目 Runtime UI。

### 19.4 一次迁移范围过大

控制：拆成 248-A/B/C 三份串行施工包；每包保持可编译、可测试；兼容 Adapter 仅用于迁移，不取得长期扩展权。

### 19.5 Retained 状态失效

控制：稳定 WidgetId、Model revision、layout revision 和结构 invariant；节点移除时清理 focus/capture/scroll state；禁止依赖裸 index 或地址。

### 19.6 Editor 性能退化

控制：Editor-only；分域 dirty；报告按档位关闭；建立 layout/pick/extract benchmark 和节点规模证据后再决定优化，不预先堆叠复杂缓存。

## 20. 第一版明确不做

```text
不修改项目 AUI Runtime 或 AUI Document 真相。
不把 Editor Widget 变成 Runtime ECS Entity。
不做完整 CSS / Web DOM。
不做通用插件 Widget ABI。
不做完整无障碍平台桥，但保留 role/focus/label 语义。
不做动画与视觉换肤工程。
不一次实现 Slate 全部 Invalidation 和导航规则。
不要求第一版完成自由 Floating 多窗口。
不通过 AI 绕过真实人工点击验收。
不在本方案阶段修改代码或施工队列。
```

## 21. 完成标准

只有以下全部成立，P0-1 才能标记施工完成：

```text
EditorWidgetTree 成为唯一 UI 结构、computed geometry、clip 和 pick 真相。
当前可见 Native Editor 面板全部从 WidgetTree 生成。
Panel 代码不再手写最终 HitRegion。
UiDrawList 只作为渲染 DTO。
DockLayoutManager 和 layout_for 不再形成重复布局。
底部 Tab、Menu、Scroll、Focus、Modal 真实可用且不穿透。
disabled/busy/failed 原因对人可见。
1280x720、1600x900、1920x1080 全通过。
Windows 100%、150%、200% DPI 全通过。
真实窗口点击 replay 和像素 screenshot 通过。
AI 可通过稳定 WidgetId/Path 定位控件和失败原因。
Runtime Player、项目 AUI 和 Windows 导出性能不受影响。
全部 Gate、受影响域回归和最终权威回归通过。
```

## 22. 方案自审结果

### 22.1 是否真的替换旧路线

通过。Panel 禁止手写最终 HitRegion；`UiDrawList` 只作为渲染 DTO；迁移期 HitRegion 只能由 WidgetTree 派生。248-B 必须删除重复 `layout_for` 和长期手写入口。

### 22.2 是否存在重复状态真相

修订后通过。Dock 持久状态、节点本地状态、瞬时输入状态和业务状态已明确分属 DockLayoutManager、WidgetTree、FocusInputSystem、EditorUiModel/Session；snapshot 只读投影。

### 22.3 Retained 生命周期是否闭合

初稿缺失，已修正。新增 keyed compose/reconcile 合同，明确节点复用、动态 reorder、role replace、删除清理和 duplicate ID fail closed。

### 22.4 UI-local Action 是否会污染业务命令

初稿边界不清，已修正。Tab/Scroll/Popup/Splitter 使用受限 `EditorWidgetAction`；任何项目或 authoring 副作用仍必须进入 UiCommand/EditorSession。

### 22.5 是否需要新 crate

不需要。C-min 第一版留在 `editor_ui_renderer` 内建立深模块，避免新增 crate 层；只有 246 后续提供真实依赖/规模证据时才另案拆分。

### 22.6 布局内核是否悬空

初稿悬空，已修正。正式选择 Taffy 作为通用布局内核，并要求 Gate A 做版本、license、feature、MSRV、文本 measure 和确定性 spike；失败时回填方案，禁止静默手写完整 Flexbox。

### 22.7 文本编辑状态是否重复

修订后通过。Widget 只负责 geometry/focus/caret visual/event target；PropertyEditBuffer、AI prompt、Asset search 等现有 owner 继续持有 draft 和 commit/cancel。

### 22.8 DPI 与像素证据是否真实

初稿不足，已修正。默认 Gate 注入 1.0/1.5/2.0 做确定性坐标验证；Windows 权威 Gate 使用真实 f64 scale factor、真实 pointer event 和 actual RGBA capture。metadata-only 不得冒充通过。

### 22.9 C-min 范围是否过大

单份施工文档过大，已修正为 248-A/B/C 三份串行施工包。它们共享一个正式方案、不能并行施工，只有 C 权威验收完成后 P0-1 才完成。

### 22.10 是否影响 Runtime、AUI 或当前施工队列

不影响。全部能力属于 Editor；不链接 Runtime Player，不修改项目 AUI。当前只修订 248 方案，不触碰 245/246、代码或施工队列。

### 22.11 自审结论

```text
方案规格：通过。
架构边界：通过，已消除状态归属和双轨歧义。
AI 适配：通过，WidgetId/Path/Role/Action/Command/Evidence 均结构化。
复杂项目适配：通过，具备 Dock/Tab/Scroll/Focus/Modal/Menu 扩展主线。
效率边界：通过，Editor-only、dirty/reconcile、报告分档，并要求分阶段性能证据。
施工可行性：通过，但必须按 248-A -> 248-B -> 248-C 串行实施。
```

248 正式方案通过自审。下一步可以生成三份串行施工文档并分别自审，但在既有施工槽位和队列授权不满足前只能进入 `待执行/`，不得施工。

## 23. 结论

248 正式选择：

```text
方案 C-min：Editor-only Retained Widget Tree

EditorUiModel
  -> EditorWidgetTree
  -> single layout / clip / pick / focus path
  -> UiDrawList
  -> WGPU
```

它以替换方式收敛当前手写 DrawList / HitRegion 路线，不新增第二套业务状态，不进入 Runtime，不影响项目 AUI。第一版只建设复杂原生编辑器必需的 retained core，并迁移当前全部可见编辑器面板；后续复杂控件和 Dock 能力在同一 WidgetTree 上扩展。
