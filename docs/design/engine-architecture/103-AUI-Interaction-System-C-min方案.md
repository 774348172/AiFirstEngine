# 103-AUI Interaction System C-min方案

## 当前归属说明：AUI Interaction 与 UiProjection

AUI 输入交互属于 AUI Runtime Core，不属于 Projection；但它依赖 AUI 渲染结果中的 hit-test / layout 数据。AUI 渲染进入 RuntimeRenderer 的同步统一归属为：

```text
UiProjection
```

本文档中历史出现的 `AuiRendererBridge` 按 `110-World-Projection-Adapter统一跨域同步规则.md` 解释为 UiProjection 的早期实现名。

## 1. 问题定位

`102 AUI Render Extract / RuntimeRenderer接入` 已经让 AUI 可以完成：

```text
AuiDocument
  -> AuiLayoutEngine
  -> AuiDrawList
  -> AuiRendererBridge
  -> AuiOverlayFrame
  -> RuntimeRenderer UI Render Pass
```

但 AUI 目前还不能处理运行时输入。
如果不补这一层，玩家点击 UI 按钮时，输入会继续进入 Gameplay InputMapping，造成典型问题：

```text
点击暂停按钮
  -> UI 应该响应
  -> 但鼠标左键也触发了开火 / 攻击 / 选择世界对象
```

所以本系统要解决的不是单独的 HitTest 小细节，而是完整但最小的：

```text
AUI Interaction System C-min
```

它负责：

```text
Raw Input
  -> AUI HitTest
  -> AUI Event Dispatch
  -> AUI Command / Consumed Result
  -> 未消费输入继续进入 InputMapping / Gameplay
```

## 2. 其他引擎参考

### 2.1 Unity UI Toolkit / UGUI

Unity UI Toolkit 使用 `DefaultEventSystem` 处理运行时 UI 输入。
源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\UIElements\Core\GameObjects\DefaultEventSystem.cs
```

核心流程：

```text
DefaultEventSystem.Update
  -> Raycaster.Update
  -> ProcessInputForUIEvents
  -> SendPositionBasedEvent / SendRayBasedEvent
  -> FindTargetAtPosition / FindTargetAtRay
  -> targetPanel.visualTree.SendEvent
```

特点：

```text
先找最上层 Panel。
命中 UI 后派发 PointerEvent。
支持 focus、pointer capture、panel enter/leave、world-space raycast。
事件可以停止传播。
```

我们吸收：

```text
UI 输入优先于 Gameplay 输入。
命中结果需要知道 target node / panel。
事件处理结果必须能决定是否消费输入。
```

我们第一版不照搬：

```text
完整 focus controller。
pointer capture。
world-space panel raycast。
复杂事件传播阶段。
```

### 2.2 Unreal Slate / UMG

UE Slate 由 `FSlateApplication` 统一处理窗口和 Widget 输入。
源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Private\Framework\Application\SlateApplication.cpp
```

核心流程：

```text
ProcessMouseButtonDownEvent
  -> LocateWindowUnderMouse
  -> FWidgetPath
  -> RoutePointerDownEvent
  -> Widget::OnMouseButtonDown
  -> FReply::Handled / Unhandled
  -> ProcessReply
```

特点：

```text
HitTest 得到 WidgetPath。
Widget 返回 FReply。
FReply 同时表达 handled、focus、capture、navigation 等结果。
Handled 后输入不继续向下泄漏。
```

我们吸收：

```text
AUI 事件返回一个明确的 InteractionResult。
InteractionResult 必须包含 consumed。
长期可以扩展 focus / capture，但 C-min 不实现。
```

### 2.3 Godot Control UI

Godot 由 Viewport 处理 Control 输入。
源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\main\viewport.cpp
```

核心流程：

```text
Viewport::push_input
  -> _gui_input_event
  -> gui_find_control
  -> Control::gui_input
  -> mouse_filter: STOP / PASS / IGNORE
```

特点：

```text
Control 树简单直观。
mouse_filter 决定是否阻止事件继续传播。
STOP / PASS / IGNORE 非常清晰。
```

我们吸收：

```text
AUI 节点需要简单、可解释的交互开关。
第一版只需要 visible / interactable / consume_input。
```

### 2.4 Bevy Picking

Bevy 把 picking 做成后端管线。
源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_picking\src\backend.rs
```

核心流程：

```text
PointerLocation
  -> Picking Backend
  -> PointerHits
  -> 合并排序
  -> hover / click / drag events
```

特点：

```text
Picking backend 只负责产生命中结果。
多个 backend 可以合并。
命中结果和事件消费分离。
```

我们吸收：

```text
HitTest 和 InteractionResult 分开。
AUI 第一版只做 ScreenOverlay backend，长期可扩展 world-space UI / 3D picking。
```

## 3. 方案对比

### 方案 A：只做裸 HitTest

```text
AuiHitTester.hit_test(x, y) -> node_id
```

优点：

```text
最简单。
实现快。
```

缺点：

```text
不能表达 consumed。
不能生成 UI Command。
不能解释输入为什么没有进入 Gameplay。
后期还是要补 Event / Result / Trace，容易返工。
```

结论：不选。

### 方案 B：完整 Unity / UE 式事件系统

```text
HitTest
  -> Capture
  -> Focus
  -> Bubble / Tunnel
  -> Navigation
  -> IME
  -> DragDrop
```

优点：

```text
长期能力最强。
接近成熟引擎。
```

缺点：

```text
第一版过重。
规则太多，AI 需要判断太多隐式状态。
当前最小游戏闭环不需要。
```

结论：长期方向参考，但 C-min 不选。

### 方案 C：AUI Interaction System C-min

```text
RuntimeInputFrame
  -> AuiInteractionSystem
  -> AuiHitTestResult
  -> AuiInteractionEvent
  -> AuiInteractionResult
      consumed
      commands
      trace
  -> if !consumed: InputResolver / Gameplay ActionSnapshot
```

优点：

```text
比裸 HitTest 完整。
比完整事件系统简单。
AI 可解释。
能直接解决 UI 输入与 Gameplay 输入冲突。
长期可以向 Unity / UE 的 focus、capture、navigation 扩展。
```

缺点：

```text
第一版不支持复杂控件行为。
第一版不支持键盘 / 手柄导航。
第一版不支持 drag / scroll / IME。
```

结论：选择方案 C。

## 4. 正式规则

### 4.1 系统定位

```text
AUI Interaction System 是 AUI Runtime Core 的输入交互层。
```

它是引擎底座能力，不包含项目玩法语义。
禁止出现：

```text
enemy
bullet
health
score
ammo
skill
inventory
quest
```

允许出现：

```text
node
pointer
event
hit
command
consumed
trace
```

### 4.2 第一版输入顺序

```text
RuntimeInputFrame
  -> AuiInteractionSystem::process
  -> AuiInteractionResult
  -> if result.consumed:
       stop UI-consumed pointer input
     else:
       InputResolver::resolve
```

规则：

```text
AUI 输入优先于 Gameplay 输入。
AUI 只能消费命中可交互 UI 节点的 pointer 输入。
未命中 UI 的输入必须继续进入 InputMapping。
AUI 消费输入必须生成 trace。
```

### 4.3 C-min 支持范围

第一版支持：

```text
PointerDown
PointerUp
PointerMove
Click
Hover
Topmost node hit test
visible 过滤
interactable 过滤
consume_input
AuiCommand
AuiInteractionTrace
```

第一版不支持：

```text
keyboard navigation
gamepad navigation
drag / drop
scroll
text input / IME
pointer capture
bubble / tunnel 双阶段传播
world-space UI raycast
```

### 4.4 HitTest 规则

输入：

```text
AuiDocument
AuiLayoutResult
pointer x/y
```

输出：

```text
AuiHitTestResult
  pointer
  hit_node
  consumed
  reason
```

命中规则：

```text
只检查 visible = true 的节点。
只允许 interactable = true 的节点成为可消费目标。
按 tree_order 从大到小检查，后绘制的节点优先命中。
rect.contains(x, y) 为 true 才算命中。
```

### 4.5 AuiCommand 规则

第一版命令：

```text
AuiCommand
  command_id
  source_node
  command_kind
  payload
```

第一版 command_kind：

```text
Click
Hover
PointerDown
PointerUp
```

规则：

```text
AuiCommand 只是 UI 层输出。
项目侧如何响应 AuiCommand 由 Project Rule / Project Logic 决定。
引擎不提供按钮业务语义。
```

### 4.6 Trace / Report 规则

最小字段：

```text
AuiInteractionTrace
  frame
  event_kind
  pointer
  hit_node
  consumed
  reason
  command_count
```

规则：

```text
AI 默认读 AuiInteractionTrace，不读底层窗口事件。
Trace 字段必须保持 UI 通用，不出现项目玩法语义。
```

## 5. 为什么适合我们

### AI 友好

AI 只需要判断：

```text
这个节点是否 visible？
这个节点是否 interactable？
pointer 是否落在 rect 内？
是否生成 AuiCommand？
是否 consumed？
```

没有复杂隐式传播阶段，调试路径短。

### 复杂项目可维护

长期可扩展：

```text
Focus
Navigation
DragDrop
Scroll
IME
WorldSpace UI
```

但第一版不让这些能力污染主链路。

### 简单度

比 Unity / UE 完整事件系统少很多层。
比裸 HitTest 多了必要的 command / consumed / trace。

### 效率

第一版 ScreenOverlay UI 节点数量通常可控，倒序扫描 computed_nodes 足够。
长期如果 UI 节点很多，再引入空间索引或 canvas-level hit region cache。

## 6. 后续施工入口

施工文档：

```text
施工文档/当前/103-当前可自动化施工文档-AUI-Interaction-System-C-min.md
```
