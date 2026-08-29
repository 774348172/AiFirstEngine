# M5 Native Input System v1 方案

## 1. 系统定义

M5 是 Native Input System v1。

它不是临时的 Runtime Window Input Pump，也不是只为某个项目补键盘事件。

M5 的长期目标是建立完整输入系统骨架：

```text
Platform Window Event
  -> PlatformInputBackend
  -> RawInputEvent
  -> InputDeviceState
  -> InputMappingContext / InputMappingAsset
  -> InputActionResolver
  -> ActionSnapshot
  -> EngineFrameInput
  -> EngineHostLoop / ProjectLogicRunner
  -> Trace / Report / Replay
```

第一版只落地 Windows 键盘和鼠标：

```text
Keyboard KeyDown / KeyUp / Repeat
Mouse Move / ButtonDown / ButtonUp / Wheel
FocusLost release all pressed keyboard and mouse buttons
```

第一版不实现：

```text
Gamepad
Touch
IME text input
Runtime rebinding UI
Multi-player input ownership
Complex trigger graph
Full platform abstraction for every OS
```

但第一版必须按完整输入系统边界施工，不能把 Windows / winit 类型泄漏到项目逻辑、EngineHostLoop 或 ProjectLogicRunner。

## 2. 已有规则继承

本方案继承：

```text
40-Input-System路线.md
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
98-Input-Mapping-Asset-C-min方案.md
134-M4-Native-Player-WindowHost-RuntimePresentIntegration-v1方案.md
```

已确认的规则继续有效：

```text
项目逻辑只读 ActionSnapshot。
项目逻辑不能读 WindowEvent / RawInputEvent / RuntimeInputFrame。
Golden Scenario / Replay 默认记录 ActionSnapshot。
Input Mapping 属于项目资源。
EngineHostLoop 每帧接收 EngineFrameInput。
```

M5 只补齐真实 Native Player Window 的输入链路，并把输入核心收敛为长期可扩展的 engine_input 边界。

## 3. 参考引擎源码结论

### Unity

Unity 在 PlayerLoop 早期处理输入。

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\PlayerLoop\PlayerLoop.bindings.cs
EarlyUpdate.ProcessMouseInWindow
EarlyUpdate.UpdateInputManager
PreUpdate.NewInputUpdate
PreUpdate.SendMouseEvents
```

结论：

```text
输入在 gameplay update 前被引擎处理成稳定状态。
脚本层不直接处理 OS window message。
```

### Unreal Engine

UE 分成平台窗口、Slate/UI、PlayerInput/EnhancedInput 多层。

源码参考：

```text
Engine\Source\Runtime\ApplicationCore\Private\Windows\WindowsApplication.cpp
FWindowsApplication::ProcessMessage
MessageHandler->OnKeyDown / OnKeyUp / OnMouseDown / OnMouseUp

Engine\Source\Runtime\Slate\Private\Framework\Application\SlateApplication.cpp
FSlateApplication::OnKeyDown / OnMouseDown

Engine\Source\Runtime\Engine\Private\UserInterface\PlayerInput.cpp
UPlayerInput::InputKey
UPlayerInput::ProcessInputStack

Engine\Plugins\EnhancedInput\Source\EnhancedInput
UInputAction
UInputMappingContext
UEnhancedPlayerInput
```

结论：

```text
平台事件不直接进入项目逻辑。
Action / Mapping / Trigger 是独立层。
玩家逻辑读取已经处理后的输入结果。
```

### Bevy

Bevy 的 winit 层把 WindowEvent 转成 Bevy input events，input crate 再维护状态资源。

源码参考：

```text
crates\bevy_winit\src\state.rs
WindowEvent::KeyboardInput / MouseInput / CursorMoved

crates\bevy_input\src\keyboard.rs
KeyboardInput -> ButtonInput<KeyCode>

crates\bevy_input\src\mouse.rs
MouseButtonInput / MouseMotion / MouseWheel
```

结论：

```text
winit 只属于平台接入层。
核心输入状态是引擎类型。
```

### Godot

Godot 由 DisplayServer / Window 产生 InputEvent，再由 Viewport 和 InputMap 分发与映射。

源码参考：

```text
scene\main\window.cpp
Window::_window_input

scene\main\viewport.cpp
Viewport::push_input

core\input\input_map.cpp
InputMap::event_is_action

core\input\input.cpp
Input::is_action_pressed / action_press
```

结论：

```text
Raw InputEvent 和 ActionMap 分开。
项目层可用 action 查询，而不是直接绑定平台消息。
```

## 4. 方案对比

| 方案 | 做法 | 优点 | 缺点 | 结论 |
|---|---|---|---|---|
| A | 项目逻辑直接读 winit / OS 事件 | 最快 | 平台泄漏，AI 难查，Replay 难统一 | 不选 |
| B | 只在 runtime_player_winit 里补一个输入泵 | 改动小 | 容易成为临时结构，后续会返工 | 不选 |
| C | 建立完整 Native Input System，v1 只实现 Windows 键鼠 | 长期边界正确，AI 友好，复杂项目可扩展 | 第一版改动更大 | 选择 |
| D | 完整复制 UE Enhanced Input / Unity Input System | 能力强 | 第一版复杂度过高，规则膨胀 | 不选 |

最终选择：

```text
方案 C。
完整输入系统架构一次定好。
第一版只实现 Windows keyboard / mouse backend。
```

## 5. 我们的系统边界

### engine_input

长期输入核心 crate。

职责：

```text
RawInputEvent 数据模型
InputDeviceState
InputMappingAsset / InputMappingContext
InputActionResolver
ActionSnapshot / InputTraceSummary
Input diagnostics
Headless scripted input helper
```

不允许：

```text
依赖 winit
依赖 editor
依赖 runtime_player_winit
依赖项目 gameplay 语义
```

### runtime_player_winit

平台窗口接入层。

职责：

```text
winit WindowEvent -> RawInputEvent
Native player 每帧 drain input
调用 engine_input resolver
把 ActionSnapshot 写入 EngineFrameInput
把 input summary 写入 NativeWindowHostReport
```

不允许：

```text
实现自己的 Action 规则
实现项目 gameplay 输入逻辑
让 ProjectLogicRunner 读取 winit 类型
```

### engine_runtime

运行时主循环层。

职责：

```text
EngineFrameInput 接收 ActionSnapshot
FrameLoop / ProjectLogicRunner 读取 ActionSnapshot
RuntimeTrace 记录 InputTraceSummary
```

M5 允许 engine_runtime 通过 re-export 兼容旧路径，但长期真相层应迁移到 engine_input。

## 6. 核心数据结构

### RawInputEvent

```text
RawInputEvent
  frame_id
  window_id
  device_kind
  event_kind
  device_path
  value
  is_repeat
```

第一版事件：

```text
KeyboardKeyDown
KeyboardKeyUp
MouseMove
MouseButtonDown
MouseButtonUp
MouseWheel
FocusLost
```

### InputDeviceState

记录当前稳定设备状态：

```text
pressed_keys
pressed_mouse_buttons
pointer_position
mouse_wheel_delta
focus
```

`FocusLost` 必须释放所有 pressed key / button，避免窗口失焦后输入卡住。

### RuntimeInputFrame

RuntimeInputFrame 仍保留为 resolver 的每帧输入容器。

M5 后它由 `InputDeviceState + RawInputEvent` 派生，第一版保持兼容现有结构。

### ActionSnapshot

项目逻辑唯一可读输入真相：

```text
frame_id
actions
```

ActionSnapshot 不包含 Windows / winit 信息。

## 7. 每帧执行流程

```text
NativeWindowHost receives winit events
  -> WinitWindowsInputBackend converts to RawInputEvent
  -> RawInputEvent appended to pending input queue

On frame tick:
  -> InputDeviceState.apply_events(pending events)
  -> InputDeviceState.to_runtime_input_frame(frame_id, window_id)
  -> InputActionResolver.resolve(frame, mapping)
  -> EngineFrameInput.with_action_snapshot(snapshot)
  -> EngineFrameInput.with_input_trace_summary(summary)
  -> EngineHostLoop.tick(input, world)
  -> Render / Present
```

Headless scripted input 也必须最终生成 ActionSnapshot，不能生成另一套 gameplay 输入。

## 8. 第一版 Input Mapping 范围

第一版支持 device path：

```text
keyboard/<KeyName>
mouse/Left
mouse/Right
mouse/Middle
mouse/Position
mouse/Wheel
```

第一版支持 action value：

```text
Button
Axis2
Pointer
Axis1
```

其中 Axis1 只用于 mouse wheel 或后续扩展。

第一版 trigger：

```text
Pressed
Released
Down
Hold 占位兼容
Tap 占位兼容
```

复杂 Trigger Graph 不在 M5 v1 做。

## 9. Report / Trace 规则

NativeWindowHostReport 增加 input summary：

```text
inputStatus
input
  backend
  platform
  focused
  rawEventCount
  runtimeEventCount
  resolvedActionCount
  lastActionIds
  pressedKeyCount
  pressedMouseButtonCount
  pointerPosition
```

运行时性能规则：

```text
默认只记录摘要。
不在普通 runtime report 中保存完整 raw event 流。
Debug / Replay 模式未来可选择记录完整输入。
```

## 10. 测试规则

M5 必须通过以下最小测试：

```text
1. KeyDown Space -> action.fire pressed
2. KeyUp Space -> action.fire released
3. Hold D over multiple frames -> action.move x = 1
4. MouseMove -> action.pointer position
5. MouseButtonDown Left -> action.fire pressed
6. MouseWheel -> action.scroll Axis1
7. FocusLost -> pressed key and mouse button released
8. Headless path and window path resolve to same ActionSnapshot shape
9. EngineHostLoop receives ActionSnapshot before ProjectLogicRunner
10. NativeWindowHostReport records input summary
```

## 11. AI 友好规则

AI 查输入问题时固定按以下顺序：

```text
RawInputEvent 是否生成正确
InputDeviceState 是否更新正确
RuntimeInputFrame 是否派生正确
InputMappingAsset 是否匹配 device_path
ActionSnapshot 是否包含目标 action
EngineFrameInput 是否带入 EngineHostLoop
ProjectLogicRunner 是否读取 ActionSnapshot
Trace / Report 是否记录摘要
```

任何新输入能力都必须落在这些层里，不允许新增旁路。

## 12. M5 v1 验收结论

M5 v1 完成后，引擎应具备：

```text
Windows native player window 可以响应键盘和鼠标。
真实窗口输入可以驱动 Runtime Action。
项目逻辑继续只读 ActionSnapshot。
输入系统具备后续扩展 gamepad / touch / multi-platform 的长期边界。
```

