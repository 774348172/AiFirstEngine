# 98-当前可自动化施工文档-Input Mapping Asset C-min

## 1. 施工目标

按照 `98-Input-Mapping-Asset-C-min方案.md` 实现第一版 Input Mapping Asset。

目标是把当前临时存在于窗口层的输入映射能力，上移为 Runtime 可复用的项目输入映射底座：

```text
InputMappingAsset
InputActionDefinition
InputContextDefinition
InputBindingDefinition
InputResolver
InputMappingDiagnostics
RuntimeInputFrame / RuntimeInputEvent
```

## 2. 硬边界

```text
不做自定义 Trigger 脚本。
不做自定义 Processor 脚本。
不做复杂 rebinding UI。
不做完整触摸手势。
不做多人本地输入。
不做网络输入预测。
不让 ProjectLogicRunner 直接读 KeyCode / MouseButton。
不把 Input Mapping 放在 editor_window_winit 里继续膨胀。
```

## 3. 施工文件

优先新增 / 修改：

```text
rust/crates/engine_runtime/src/input_mapping.rs
rust/crates/engine_runtime/src/input_action.rs
rust/crates/engine_runtime/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
```

说明：

```text
engine_runtime 负责 InputMappingAsset / InputResolver。
editor_window_winit 只负责把 EditorInputEvent 转成 RuntimeInputFrame。
ActionSnapshot 仍然在 engine_runtime::input_action。
```

## 4. 施工步骤

### Step 1: 新增 Runtime 输入映射模块

实现：

```text
InputActionValueType
InputProcessorPreset
InputTriggerPreset
InputActionDefinition
InputContextDefinition
InputBindingDefinition
PlatformInputOverride
InputMappingAsset
InputMappingDiagnostic
InputMappingReport
```

最小测试：

```text
input_mapping_asset_serializes
input_mapping_asset_validates_action_and_context_refs
input_mapping_rejects_unknown_device_path
```

### Step 2: 迁移 RuntimeInputFrame / RuntimeInputEvent

实现：

```text
RuntimeInputFrame
RuntimeInputEvent
RuntimePointerButton
```

从窗口层迁移到 engine_runtime，使 Runtime 可独立测试输入解析。

最小测试：

```text
runtime_input_frame_keeps_events_in_order
runtime_input_event_reports_kind
```

### Step 3: 实现 InputResolver

实现：

```text
InputResolver::resolve(frame, mapping) -> InputResolveResult
InputResolveResult.action_snapshot
InputResolveResult.diagnostics
```

第一版支持：

```text
Button
Axis2
Pointer
trigger: down / pressed / released
processor: none / normalize / invert / scale / deadzone 的最小行为
context priority
consume_input
```

最小测试：

```text
space_key_generates_fire_action_snapshot
wasd_generates_move_axis2_action_snapshot
pointer_move_generates_pointer_action_snapshot
context_priority_resolves_before_lower_priority
consume_input_blocks_lower_priority_binding
resolve_result_reports_unknown_action
```

### Step 4: 更新 editor_window_winit

实现：

```text
删除窗口层临时 InputMapping / InputResolver 定义。
改为使用 engine_runtime::input_mapping。
RuntimeInputFrame / RuntimeInputEvent 使用 engine_runtime 类型。
Editor PointerButton 转成 RuntimePointerButton。
```

最小测试：

```text
editor_window_runtime_input_route_still_serializes
game_view_space_key_still_drives_project_logic_and_render_extract
trace_records_runtime_route_and_action_ids
```

## 5. 验证命令

```text
cargo fmt --check
cargo test -p engine_runtime input_mapping
cargo test -p engine_runtime input_action
cargo test -p editor_window_winit input
cargo test -p editor_window_winit
cargo test --workspace
```

## 6. 完成标准

```text
InputMappingAsset 成为 engine_runtime 模块。
InputResolver 不再只存在于 editor_window_winit。
窗口层只保留输入路由和 RuntimeInputFrame 创建。
项目逻辑仍然只读 ActionSnapshot。
新增能力有最小测试。
cargo fmt --check 通过。
cargo test --workspace 通过，或明确记录环境阻塞。
```

## 7. 完成后归档

完成施工后移动到：

```text
框架设计/引擎总体架构/施工文档/已完成/
```

并更新：

```text
54-当前可自动化施工总入口.md
施工文档/README.md
阶段完成记录/2026-06-29-Input-Mapping-Asset-C-min/00-总览.md
```
