# 135 当前可自动化施工文档：M5 Native Input System v1

## 1. 施工目标

按照 `135-M5-Native-Input-System-v1方案.md` 实现 Native Input System v1。

本阶段选择方案 C：

```text
完整输入系统架构一次定好。
第一版只支持 Windows 键盘和鼠标输入。
```

施工不能引入项目 gameplay 专用概念。

## 2. 必须保持的边界

```text
engine_input 是输入核心。
runtime_player_winit 是平台窗口接入层。
engine_runtime 接收 ActionSnapshot，不读 winit。
ProjectLogicRunner 只读 ActionSnapshot。
```

不允许：

```text
项目逻辑读取 winit::WindowEvent。
ProjectLogicRunner 读取 RawInputEvent。
runtime_player_winit 自己实现 Action 规则。
为了打飞机游戏增加 enemy / bullet / weapon 等输入专用规则。
```

## 3. 自动化任务

### A1 新增 engine_input crate

新增：

```text
rust/crates/engine_input
```

实现：

```text
RawInputEvent
RawInputEventKind
InputDeviceKind
RawInputValue
InputDeviceState
InputDeviceStateReport
```

将现有输入类型迁入或 re-export：

```text
ActionSnapshot
InputActionState
ActionValue
ActionPhase
InputTraceSummary
RuntimeInputFrame
RuntimeInputEvent
RuntimePointerButton
InputMappingAsset
InputResolver
```

第一版可以从 engine_runtime 原模块迁移代码到 engine_input，然后 engine_runtime 保留兼容 re-export。

完成后测试：

```text
cargo test -p engine_input
```

### A2 实现 InputDeviceState -> RuntimeInputFrame

实现：

```text
InputDeviceState::apply_raw_event
InputDeviceState::apply_raw_events
InputDeviceState::to_runtime_input_frame
```

覆盖：

```text
KeyDown
KeyUp
KeyRepeat
MouseMove
MouseButtonDown
MouseButtonUp
MouseWheel
FocusLost
```

完成后测试：

```text
cargo test -p engine_input input_device_state
```

### A3 扩展 InputResolver 支持稳定状态

目标：

```text
held key over multiple frames can resolve axis/button
mouse wheel can resolve Axis1
released event can resolve Released trigger
```

如果需要保留旧事件解析行为，必须兼容旧测试。

完成后测试：

```text
cargo test -p engine_input input_mapping
cargo test -p engine_runtime input
```

### A4 engine_runtime 接入 engine_input

修改：

```text
engine_runtime/Cargo.toml
engine_runtime/src/lib.rs
engine_runtime/src/input_action.rs
engine_runtime/src/input_mapping.rs
```

目标：

```text
engine_runtime 通过 re-export 保持旧路径可用。
EngineHostLoop / FrameLoop / ProjectLogicRunner 继续编译。
```

完成后测试：

```text
cargo test -p engine_runtime
```

### A5 runtime_player_winit 接入 Windows 键鼠输入

新增或修改：

```text
runtime_player_winit::WinitWindowsInputBackend
runtime_player_winit::NativeInputSummary
NativeWindowHostReport.input_status
NativeWindowHostReport.input
```

实现：

```text
winit WindowEvent::KeyboardInput -> RawInputEvent
winit WindowEvent::CursorMoved -> RawInputEvent
winit WindowEvent::MouseInput -> RawInputEvent
winit WindowEvent::MouseWheel -> RawInputEvent
winit WindowEvent::Focused(false) -> FocusLost
```

每帧：

```text
drain pending raw input
update InputDeviceState
resolve ActionSnapshot
with_action_snapshot
with_input_trace_summary
host.tick
```

完成后测试：

```text
cargo test -p runtime_player_winit
cargo check -p runtime_player_winit --features real-window
```

### A6 Headless 与真实窗口输入统一测试

实现 headless 层可测输入，不要求真实 OS 交互测试。

测试目标：

```text
scripted RawInputEvent -> ActionSnapshot
window backend converted RawInputEvent -> same ActionSnapshot shape
NativeWindowHostReport records input summary
```

完成后测试：

```text
cargo test -p runtime_player_winit native_input
```

## 4. 阶段性检测

每完成一个任务必须执行对应测试。

如果失败：

```text
先修复当前任务。
不进入下一任务。
不降低方案 C 的系统边界。
```

## 5. 整体回归

全部完成后执行：

```text
cargo fmt --check
cargo test -p engine_input
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo check -p runtime_player_winit --features real-window
cargo test -p editor_window_winit
cargo test -p runtime_cli
```

## 6. 完成记录

施工完成后新增：

```text
框架设计/引擎总体架构/阶段完成记录/2026-07-01-M5-Native-Input-System-v1/00-总览.md
```

并更新：

```text
49-当前状态与下一步入口.md
00-文档地图.md
施工文档/README.md
```

## 7. 自检清单

施工文档自检：

```text
是否选择方案 C：是。
是否只支持 Windows 键鼠 v1：是。
是否保留长期完整输入架构：是。
是否避免项目 gameplay 专用规则：是。
是否要求阶段测试和整体测试：是。
```

