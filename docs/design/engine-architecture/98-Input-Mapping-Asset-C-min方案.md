# 98-Input Mapping Asset C-min 方案

本文档定义第一版项目输入映射资源规则。

本规则承接：

```text
40-Input-System路线.md
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
93-复杂打飞机验证所需引擎侧缺失能力清单.md
```

## 1. 问题是什么

当前 Runtime 已经有：

```text
ActionSnapshot
InputActionState
InputTraceSummary
EngineHostLoop 接收 ActionSnapshot
ProjectLogicRunner 只读 ActionSnapshot
Native Editor Viewport 输入回流 RuntimeFrame
```

但还缺一个正式项目资源：

```text
Input Mapping Asset
```

也就是说，现在窗口层可以临时把 `Space -> action.fire` 解析出来，但这个映射还没有成为项目资产，也没有清晰的导入、验证、运行时读取边界。

## 2. 其他引擎怎么做

### Unity

Unity 旧输入系统使用：

```text
Input.GetKey
Input.GetButton
Input.GetAxis
```

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\InputLegacy\Input.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\PlayerLoop\PlayerLoop.bindings.cs
```

Unity 新 Input System 的方向是：

```text
Input Action
Action Map
Binding
Processor
Interaction
Device abstraction
```

结论：

```text
Unity 的长期方向是输入资产化和 Action-first。
旧的 KeyCode 直读简单，但不适合复杂项目和 AI 修改。
```

### Unreal Engine

UE 旧输入系统使用：

```text
UInputSettings
ActionMapping
AxisMapping
InputComponent binding
```

UE Enhanced Input 使用：

```text
UInputAction
UInputMappingContext
UInputModifier
UInputTrigger
EnhancedInputSubsystem
```

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\EnhancedInput
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Classes\Components\InputComponent.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\Runtime\OpenXR\Source\OpenXRInput
```

结论：

```text
UE 的长期方向也是 Action + Mapping Context + Modifier + Trigger。
能力很强，但第一版照搬会让规则过多。
```

### Godot

Godot 使用：

```text
InputMap
InputEventKey
InputEventJoypadButton
InputEventJoypadMotion
is_action_pressed
```

本地源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\core\input
<GODOT_SOURCE>\godot-master\godot-master\editor\settings\action_map_editor.cpp
<GODOT_SOURCE>\godot-master\godot-master\main\main.cpp
```

结论：

```text
Godot 的 InputMap 简单清晰，适合第一版心智。
但复杂上下文、Processor、Trigger preset 需要额外设计。
```

### Bevy

Bevy 内置偏底层：

```text
ButtonInput<KeyCode>
ButtonInput<MouseButton>
KeyboardInput
MouseButtonInput
GamepadEvent
```

本地源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_input
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_winit
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_gilrs
```

结论：

```text
Bevy 简单灵活，但项目容易直接依赖设备输入。
本项目不能让项目逻辑默认读 ButtonInput / KeyCode。
```

## 3. 正式方案

采用：

```text
Input Mapping Asset C-min
```

核心规则：

```text
Input Mapping 是项目资源。
Raw Device Event 是引擎输入层数据。
InputResolver 读取 InputMappingAsset，把 RuntimeInputFrame 转成 ActionSnapshot。
项目逻辑和 AI 只读 ActionSnapshot。
项目逻辑禁止直接读 KeyCode / MouseButton / TouchEvent。
第一版只开放 preset，不开放自定义 Trigger / Processor 脚本。
```

## 4. 标准结构

### 4.1 InputMappingAsset

```text
InputMappingAsset
  schema_version
  asset_id
  actions
  contexts
  bindings
  platform_overrides
```

### 4.2 InputActionDefinition

```text
InputActionDefinition
  id
  value_type
```

`value_type` 第一版只支持：

```text
Button
Axis1
Axis2
Pointer
```

`Gesture` 留作后续，不进入第一版实现。

### 4.3 InputContextDefinition

```text
InputContextDefinition
  id
  priority
  consume_input
  enabled_by_default
```

规则：

```text
priority 数值越高优先级越高。
consume_input = true 时，该 context 触发后低优先级 context 不再消费同一输入事件。
第一版只做确定性排序，不做运行时复杂 context stack。
```

### 4.4 InputBindingDefinition

```text
InputBindingDefinition
  context_id
  action_id
  device_path
  processor
  trigger
```

`device_path` 示例：

```text
keyboard/Space
keyboard/W
keyboard/A
keyboard/S
keyboard/D
mouse/Left
mouse/Position
gamepad/South
gamepad/LeftStick
```

### 4.5 Processor preset

第一版只支持：

```text
none
deadzone
normalize
scale
invert
```

规则：

```text
Processor 只修改输入值，不决定是否触发。
第一版参数保持少量固定字段。
不支持项目自定义 Processor 脚本。
```

### 4.6 Trigger preset

第一版只支持：

```text
down
pressed
released
hold
tap
```

规则：

```text
Trigger 只决定 Action phase。
第一版不做 doubleTap / chord / releaseAfterHold。
这些复杂输入后续以 preset 方式扩展，不开放脚本。
```

### 4.7 Platform override

第一版结构保留，但只做最小验证：

```text
PlatformInputOverride
  platform
  binding_overrides
```

第一版可先支持：

```text
desktop
```

后续再扩展：

```text
windows
macos
linux
ios
android
web
console
```

## 5. Runtime 执行流程

```text
OS / Editor Window input
  -> RuntimeInputFrame
  -> InputMappingAsset
  -> InputResolver
  -> ActionSnapshot
  -> EngineHostLoop.tick_runtime_frame
  -> ProjectLogicRunner
```

边界：

```text
ViewportHost 只判断输入归属。
InputResolver 才解析项目 Input Mapping。
EngineHostLoop 只接收 ActionSnapshot。
ProjectLogicRunner 不理解设备输入。
```

## 6. AI 友好规则

AI 生成输入时，应该生成：

```text
action.move: Axis2
  keyboard/WASD
  gamepad/LeftStick

action.fire: Button
  keyboard/Space
  mouse/Left
  gamepad/South
```

AI 不应该生成：

```text
项目逻辑直接读 keyboard/W
项目逻辑直接读 mouse/Left
项目逻辑直接读 platform raw event
自定义 Trigger 脚本
隐藏 coroutine 输入逻辑
```

## 7. 第一版不做

```text
完整触摸手势
复杂 rebinding UI
自定义 Trigger 脚本
自定义 Processor 脚本
多人本地输入
输入延迟补偿
网络输入预测
平台输入法细节
```

## 8. 第一版测试要求

必须覆盖：

```text
InputMappingAsset 可以序列化 / 反序列化。
keyboard/Space 可以触发 action.fire。
keyboard/WASD 可以生成 action.move Axis2。
mouse/Position 可以生成 pointer action。
context priority 决定解析顺序。
consume_input 可以阻止低优先级 context。
unknown action / context / device path 会产生诊断。
ActionSnapshot 只包含最终 action，不暴露 raw device event。
```

## 9. 结论

```text
本项目采用 Unity / UE 的 Action-first 长期方向。
第一版复杂度靠 Godot-like 清晰 InputMap 心智收敛。
Bevy 的 raw input/resource 模型只作为底层采集参考，不暴露给项目逻辑。
Input Mapping Asset 是项目资源。
InputResolver 是引擎底座。
Project Logic 只读 ActionSnapshot。
```
