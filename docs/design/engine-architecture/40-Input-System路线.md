# Input System 路线

本文档定义 Rust Runtime 与编辑器共同使用的输入系统路线。

核心结论：

```text
Input 系统采用 Action-first。
Action 不是单一 Axis。
Action 必须支持 Button / Axis1 / Axis2 / Pointer / Gesture。
Input Mapping 属于项目层。
Raw Device Event 属于引擎层。
必须有 Input Processor。
必须有 Input Trigger / Activation Rule。
项目逻辑和 AI 只读最终 InputAction / ActionSnapshot。
Golden Scenario / Replay 记录 ActionSnapshot 和触发阶段。
```

## 问题说明

游戏输入不是简单的键盘按键。

长期必须支持：

```text
键盘
鼠标
手柄
触摸
移动端虚拟摇杆
UI 点击
双击
长按
蓄力
组合键
按住释放
多上下文输入
Replay 输入
Golden Scenario 输入
AI 自动测试输入
```

如果项目逻辑直接依赖底层输入：

```text
KeyCode.W
MouseButton.Left
TouchEvent
GamepadButton.A
```

会导致：

```text
AI 难以修改。
多平台映射复杂。
移动端适配困难。
Replay / Golden Scenario 不稳定。
项目逻辑和设备强耦合。
```

因此项目逻辑必须读 InputAction，而不是读 Raw Device Event。

## 市场方案对比

### Unreal Enhanced Input

核心：

```text
Input Action
Input Mapping Context
Input Modifier
Input Trigger
```

优点：

```text
Action-first。
Mapping Context 适合 UI / Gameplay / Vehicle 等模式切换。
Modifier / Trigger 能表达复杂触发条件。
适合复杂项目。
```

缺点：

```text
概念较多。
对非程序用户理解成本较高。
AI 如果直接生成底层配置，容易变复杂。
```

本项目借鉴：

```text
Input Action
Mapping Context
Modifier -> Processor
Trigger -> Activation Rule
```

本项目不照搬：

```text
不把复杂 Trigger API 直接暴露给普通用户。
第一版只做常用 preset。
```

### Unity Input System

核心：

```text
Input Action
Action Map
Binding
Processor
Interaction
Device abstraction
```

优点：

```text
跨平台设备抽象成熟。
Action Map / Binding 资产化。
Processor / Interaction 能表达复杂输入。
```

缺点：

```text
配置较复杂。
调试心智成本较高。
对 AI 生成来说需要更强 schema 约束。
```

本项目借鉴：

```text
Action Map / Binding / Device abstraction
Processor / Interaction 思想
```

本项目不照搬：

```text
不直接复制 Unity 资产格式。
不把输入配置做成难以 AI 审查的大对象。
```

### Godot InputMap

核心：

```text
项目逻辑读 action。
action 可以绑定多个输入。
```

优点：

```text
简单。
直观。
适合中小项目。
```

缺点：

```text
复杂上下文、Trigger、Gesture 能力需要扩展。
```

本项目借鉴：

```text
项目逻辑只读 action。
自然语言用户容易理解。
```

本项目扩展：

```text
增加 Context / Processor / Trigger / Gesture / ActionSnapshot。
```

### winit

核心：

```text
Rust 跨平台窗口和 Raw Input Event。
```

优点：

```text
适合作为 Rust Runtime 的底层输入事件来源。
跨平台窗口事件生态成熟。
```

缺点：

```text
它不是游戏输入系统。
只提供 raw event，不提供 Action / Mapping / Trigger。
```

本项目借鉴：

```text
底层 keyboard / mouse / window event 采集。
```

### Bevy Input

Bevy 的输入系统对本项目的参考点是：

```text
winit raw event 进入引擎。
PreUpdate / input system 把 raw event 处理成 ButtonInput、Touches 等运行时状态。
项目 system 通常读取处理后的输入资源，而不是直接处理平台事件。
```

本项目继续采用更 AI 友好的 Action-first：

```text
Bevy 的 raw-to-state 处理可以参考。
项目逻辑不直接读取 ButtonInput / Touches。
项目逻辑和 AI 只读取 InputAction / ActionSnapshot。
Trigger / Processor 使用 preset，避免把复杂输入脚本暴露给普通用户。
```

本项目不允许：

```text
项目逻辑直接依赖 winit event。
AI 直接生成 winit 输入处理代码。
```

## 正式架构

输入流：

```text
Raw Input Event
  -> Input Device Layer
  -> Input Processor
  -> Input Trigger / Activation Rule
  -> Input Mapping / Context
  -> InputAction
  -> ActionSnapshot
  -> Runtime / IR / Project Rule
```

### Raw Input Event

引擎层负责。

来源：

```text
keyboard
mouse
gamepad
touch
window focus
platform-specific input
```

Raw Input Event 不进入项目逻辑。

### Input Processor

负责修改输入值。

第一版 preset：

```text
deadzone
normalize
scale
invert
clamp
```

示例：

```text
left_stick -> deadzone(0.15) -> normalize -> move
```

### Input Trigger / Activation Rule

负责判断 Action 是否触发。

第一版 preset：

```text
press
hold
release
tap
doubleTap
threshold
chord
```

示例：

```text
fire:
  trigger: press

heavy_attack:
  trigger: hold(duration=0.5)

dash:
  trigger: doubleTap(maxInterval=0.25)

charge_release:
  trigger: releaseAfterHold(minDuration=0.6)
```

用户和 AI 看到的推荐叫法：

```text
触发条件
Activation Rule
```

底层文档可以叫：

```text
Input Trigger
```

### Input Mapping / Context

Input Mapping 属于项目层。

它定义：

```text
哪个设备输入绑定到哪个 Action。
当前上下文是否启用。
上下文优先级。
是否消费输入。
```

典型 Context：

```text
gameplay
ui
vehicle
dialogue
build_mode
debug
```

示例：

```text
gameplay:
  Keyboard.WASD -> move
  Mouse.Left -> fire
  Shift -> dash

ui:
  Mouse.Left -> ui_click consume=true
```

如果 UI context 消费输入，gameplay context 不能同时触发 fire。

### InputAction

项目逻辑和 AI 只读 InputAction。

Action 类型：

```text
Button
Axis1
Axis2
Pointer
Gesture
```

Action 状态：

```text
started
performed
canceled
pressed
justPressed
released
heldDuration
value
sources
```

示例：

```text
move: Axis2
aim: Axis2
fire: Button
dash: Button
select: Pointer
zoom: Gesture
ui_click: Pointer
```

### ActionSnapshot

ActionSnapshot 是每帧输入结果。

用于：

```text
Runtime tick
Replay
Golden Scenario Test
AI Patch validation
debug trace
```

ActionSnapshot 记录：

```text
frame
activeContexts
actions[]
action.value
action.phase
action.sources
action.heldDuration
```

## AI 规则

AI 不生成底层输入处理代码。

AI 可以生成：

```text
InputAction
InputBinding
InputContext
InputTrigger preset
InputProcessor preset
Golden Scenario ActionSnapshot
```

AI 应该和用户确认：

```text
动作名称是什么。
默认绑定是什么。
移动端怎么触发。
手柄怎么触发。
是否需要长按、双击、组合键。
UI 是否抢占该输入。
```

示例：

```text
用户：给玩家加冲刺技能。
AI：
  需要新增 dash Action。
  PC 默认绑定 Shift。
  手柄默认绑定 B。
  移动端默认双击虚拟摇杆方向。
  触发条件为 press 或 doubleTap。
```

## Golden Scenario / Replay 规则

Golden Scenario 和 Replay 默认记录 ActionSnapshot。

不默认记录：

```text
Keyboard.W
Mouse.Left
TouchEvent raw details
```

原因：

```text
ActionSnapshot 更稳定。
跨平台一致。
AI 更容易生成。
测试不依赖具体设备。
```

如果需要调试底层设备输入，可以额外记录 Raw Input Trace，但它不是项目逻辑标准输入。

## 第一版 MVP

第一版实现：

```text
InputAction schema
InputBinding schema
InputContext schema
InputProcessor preset schema
InputTrigger / ActivationRule preset schema
ActionSnapshot
Keyboard / Mouse raw event adapter
Button / Axis1 / Axis2 / Pointer / Gesture 类型
press / hold / release / tap / doubleTap / threshold / chord
deadzone / normalize / scale / invert / clamp
Context priority / consume
RuntimeTrace input summary
Golden Scenario ActionSnapshot fixture
```

第一版不做：

```text
完整手柄设备接入
完整触摸系统接入
复杂 rebinding UI
输入延迟补偿
网络输入预测
多人本地输入
平台输入法细节
复杂自定义 Trigger 脚本
```

## 当前确认后的结论

```text
Input 系统采用 Action-first。
必须有 Processor。
必须有 Trigger / Activation Rule。
Action 支持 Button / Axis1 / Axis2 / Pointer / Gesture。
Input Mapping 属于项目层。
Raw Device Event 属于引擎层。
项目逻辑和 AI 不直接面对 KeyCode / MouseButton / TouchEvent。
Golden Scenario / Replay 记录 ActionSnapshot。
第一版做 keyboard / mouse adapter，手柄和触摸完整接入后续扩展。
```

## Native Editor Viewport 输入回流 Runtime Frame 规则

本规则定义编辑器 Viewport 里的输入如何进入 Runtime Frame。

核心结论：

```text
WindowEvent
  -> EditorInputEvent
  -> UI HitTest
  -> ViewportInputGateway
  -> ViewportInputRoute
      -> EditorToolCommand
      -> SceneCameraCommand
      -> RuntimeInputFrame
  -> InputResolver
  -> ActionSnapshot
  -> EngineHostLoop tick
```

市场引擎参考：

```text
Unreal：
  FSceneViewport 接收 Slate 输入事件。
  FSceneViewport 调用 ViewportClient->InputKey / InputAxis。
  UGameViewportClient 再路由到 Console / Override / PlayerController。
  参考源码：
    Engine/Source/Runtime/Engine/Private/Slate/SceneViewport.cpp
    Engine/Source/Runtime/Engine/Private/GameViewportClient.cpp

Unity：
  GameView.OnGUI 读取 Event.current。
  GameView 把鼠标坐标换算到 GameView 内部坐标。
  GameView 调用 EditorGUIUtility.QueueGameViewInputEvent。
  SceneView 则优先处理编辑器相机、Handle、Gizmo、Selection。
  参考源码：
    Editor/Mono/GameView/GameView.cs
    Editor/Mono/SceneView/SceneView.cs

Bevy：
  WindowEvent 进入输入系统。
  PointerInputPlugin 把 WindowEvent 转成 PointerInput。
  项目系统读取处理后的输入资源 / 消息，而不是直接绑定平台窗口事件。
  参考源码：
    crates/bevy_picking/src/input.rs

Godot：
  InputEvent 进入 Viewport。
  项目侧通常通过 InputMap action / _input / _unhandled_input 处理。
```

本项目采用：

```text
UE-like ViewportClient 边界
  + Unity-like GameView / SceneView 输入归属
  + Bevy-like 数据化输入事件
  + 本项目 ActionSnapshot 规则
```

硬规则：

```text
1. UI 永远优先吃输入。
2. Scene View 默认输入归编辑器工具，不直接进入项目逻辑。
3. Scene View 输入可生成 EditorToolCommand / SceneCameraCommand / SelectionCommand。
4. Game View 或 Play Mode 获得焦点后，输入才进入 RuntimeInputFrame。
5. 项目逻辑只读 ActionSnapshot，不读 EditorInputEvent / WindowEvent。
6. Runtime Trace 必须记录输入路由摘要、ActionSnapshot 摘要和进入的 runtime frame。
7. Golden Scenario / Replay 继续记录 ActionSnapshot，不记录平台 Raw Event 作为默认真相。
8. ViewportHost 只判断 viewport 区域、焦点、输入归属，不执行项目逻辑。
9. InputResolver 负责把 RuntimeInputFrame 转成 ActionSnapshot。
10. EngineHostLoop 只接收每帧输入结果，不直接依赖 winit / EditorInputEvent。
```

第一版只做：

```text
PointerDown / PointerMove / PointerUp
KeyDown / KeyUp
UI or Viewport 路由
SceneView editor tool route
GameView runtime action route
ActionSnapshot 进入 EngineHostLoop
headless 测试
```

第一版不做：

```text
完整手柄输入
完整触摸输入
复杂 rebinding UI
复杂 Scene Gizmo
多 viewport 多窗口输入归属
输入延迟补偿
网络输入预测
平台输入法细节
```

AI 规则：

```text
AI 不生成底层 WindowEvent / EditorInputEvent 处理代码。
AI 可以生成 InputAction / Binding / Context / Processor / Trigger preset。
AI 可以生成 Golden Scenario ActionSnapshot。
AI 查 Bug 时优先看 ViewportInputRoute、ActionSnapshot、RuntimeTrace。
```
