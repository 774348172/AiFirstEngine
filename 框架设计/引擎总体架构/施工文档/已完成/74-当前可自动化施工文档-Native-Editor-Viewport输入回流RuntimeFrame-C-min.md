# 当前可自动化施工文档：Native Editor Viewport 输入回流 Runtime Frame C-min

本文档是 74 号方案的施工文档。

正式规则见：

```text
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
40-Input-System路线.md
47-Native-Editor-Host-BC路线.md
71-Runtime-Viewport-Rendering-System方案.md
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
```

## 目标

实现 Native Editor Viewport 输入进入 Runtime Action / Game Frame 的最小闭环：

```text
EditorInputEvent
  -> UI HitTest
  -> ViewportHost
  -> ViewportInputGateway
  -> ViewportInputRoute
  -> RuntimeInputFrame
  -> InputResolver
  -> ActionSnapshot
  -> EngineHostLoop / FrameLoop
  -> ProjectLogicRunner
```

施工完成后，必须能用 headless 测试证明：

```text
UI 输入不会进入 Runtime。
SceneView 输入只进入编辑器工具 / 相机命令。
GameView focused 输入能生成 ActionSnapshot。
ActionSnapshot 能进入 Runtime Frame。
ProjectLogicRunner 能读取当前帧 ActionSnapshot。
RuntimeTrace 能记录输入路由和 action 摘要。
```

## 非目标

第一版不做：

```text
完整手柄输入。
完整触摸输入。
复杂 rebinding UI。
复杂 Scene Gizmo。
多 viewport 多窗口输入归属。
输入延迟补偿。
网络输入预测。
平台输入法细节。
真实 OS window 依赖测试。
```

## 当前基础

已完成：

```text
Input System MVP。
Native Editor Host / ViewportHost headless gate。
Runtime Viewport Rendering System D-min。
ProjectLogicRunner / LogicExecutor / Rust ECS 接入 v1。
EngineHostLoop / FrameLoop。
RuntimeTrace。
```

可参考完成文档：

```text
施工文档/已完成/41-当前可自动化施工文档-Input-System-MVP.md
施工文档/已完成/66-当前可自动化施工文档-Native-Editor-RealWindow-ViewportSurface-C-min.md
施工文档/已完成/71-当前可自动化施工文档-Runtime-Viewport-Rendering-System-D-min.md
施工文档/已完成/73-当前可自动化施工文档-ProjectLogicRunner-LogicExecutor-RustECS接入-v1.md
```

## 自动化任务 A1：定义 Viewport 输入路由数据结构

新增或扩展 editor/native 侧数据结构：

```text
EditorInputEvent
ViewportInputGateway
ViewportInputRoute
ViewportInputRouteKind
RuntimeInputFrame
RuntimeInputEvent
```

第一版 route kind：

```text
UiConsumed
SceneCameraCommand
EditorToolCommand
RuntimeInputFrame
Ignored
```

第一版 RuntimeInputEvent：

```text
PointerDown
PointerMove
PointerUp
KeyDown
KeyUp
```

验收：

```text
数据结构可序列化 / 可 debug。
route 必须带 viewport_id / viewport_kind / focused / hovered / input_event_kind / reason。
不引入真实窗口依赖。
```

最小测试：

```text
construct_viewport_input_route_is_serializable
runtime_input_frame_keeps_events_in_order
```

## 自动化任务 A2：实现 UI HitTest -> ViewportHost -> ViewportInputGateway

实现纯函数或小型 service：

```text
route_editor_input(event, ui_hit_result, viewport_host_state) -> ViewportInputRoute
```

规则：

```text
UI hit 优先返回 UiConsumed。
SceneView focused / hovered 默认返回 SceneCameraCommand 或 EditorToolCommand。
GameView focused 返回 RuntimeInputFrame。
GameView not focused 返回 Ignored(reason = viewport_not_focused)。
没有命中 viewport 返回 Ignored(reason = no_viewport_hit)。
```

验收：

```text
ViewportHost 不执行项目逻辑。
ViewportInputGateway 不调用 Runtime tick。
ViewportInputGateway 不解析 InputAction。
```

最小测试：

```text
ui_hit_consumes_pointer_down
scene_view_pointer_drag_routes_to_scene_camera_command
game_view_focused_key_down_routes_to_runtime_input_frame
game_view_unfocused_key_down_is_ignored
```

## 自动化任务 A3：实现 RuntimeInputFrame -> ActionSnapshot

接入已有 Input System MVP：

```text
RuntimeInputFrame
  -> InputResolver
  -> ActionSnapshot
```

第一版只支持：

```text
Button Action
Axis2 Action
Pointer Action
```

示例 mapping：

```text
Space -> action.fire
PointerPosition -> action.pointer
WASD -> action.move
```

验收：

```text
项目逻辑仍只读 ActionSnapshot。
不允许项目逻辑读取 RuntimeInputFrame。
ActionSnapshot 可用于 Golden Scenario / Replay。
```

最小测试：

```text
space_key_generates_fire_action_snapshot
pointer_move_generates_pointer_action_snapshot
wasd_generates_move_axis2_action_snapshot
```

## 自动化任务 A4：ActionSnapshot 接入 EngineHostLoop / FrameLoop

扩展 runtime tick 输入：

```text
EngineHostLoop.tick_runtime_frame(action_snapshot?)
FrameLoop.tick_runtime_frame(world, action_snapshot?)
LogicContext.action_snapshot
```

规则：

```text
ActionSnapshot 必须在 ProjectLogicRunner 前可读。
ProjectLogicRunner 只读当前帧 ActionSnapshot。
RenderExtract 只读取 ProjectLogicRunner 修改 ECS 后的 dirty 结果。
兼容旧 tick 路径时，可以 action_snapshot = None。
```

验收：

```text
旧测试不破坏。
无输入时 Runtime 行为保持稳定。
有输入时 ProjectLogicRunner 能读取 action。
```

最小测试：

```text
runtime_frame_accepts_none_action_snapshot
runtime_frame_exposes_action_snapshot_to_project_logic
fire_action_rule_writes_transform_or_marker_component
```

## 自动化任务 A5：Trace / Report 接入

Runtime Trace 增加输入摘要：

```text
frame_id
viewport_id
viewport_kind
route_kind
route_reason
action_count
action_ids
project_logic_rule_count
```

Editor Trace 或 gate report 增加：

```text
editor_frame_id
input_event_kind
ui_hit
viewport_hit
focused_viewport_id
route_kind
reason
```

验收：

```text
能查出输入为什么没进 Runtime。
能查出输入生成了哪些 Action。
能查出 Action 进入了哪一帧。
```

最小测试：

```text
trace_records_ui_consumed_route
trace_records_runtime_route_and_action_ids
trace_records_ignored_unfocused_game_view
```

## 自动化任务 A6：端到端 headless 场景测试

新增一个 headless scenario：

```text
focused GameView
KeyDown Space
Input Mapping: Space -> action.fire
ProjectLogicRunner rule: action.fire pressed -> move entity x += 1
FrameLoop tick
RenderExtract
RuntimeRenderer output summary
```

验收：

```text
ActionSnapshot 包含 action.fire。
ProjectLogicRunner 执行对应规则。
ECS Transform 产生变化。
DirtyRecord 被 RenderExtract 消费。
RuntimeTrace 记录输入和规则执行。
```

最小测试：

```text
game_view_space_key_drives_project_logic_and_render_extract
```

## 自动化任务 A7：文档同步

施工完成后必须：

```text
1. 新增 阶段完成记录/2026-xx-xx-Native-Editor-Viewport输入回流RuntimeFrame-C-min/00-总览.md
2. 把本施工文档从 施工文档/当前 移到 施工文档/已完成。
3. 更新 49-当前状态与下一步入口.md。
4. 更新 54-当前可自动化施工总入口.md。
5. 更新 18-长期主义实现路径.md。
6. 更新 README.md。
7. 更新 00-文档地图.md。
8. 更新 施工文档/README.md。
9. 更新 阶段完成记录/README.md。
```

## 测试命令

施工过程中至少运行：

```powershell
cargo test -p engine_runtime input
cargo test -p engine_runtime frame_loop
cargo test -p engine_runtime project_logic
cargo test -p engine_runtime
```

如果涉及 editor/native crate，额外运行对应 crate 测试：

```powershell
cargo test -p editor_core
cargo test -p editor_native
```

实际 crate 名以当前 workspace 为准。

## 完成标准

```text
1. UI / SceneView / GameView 输入归属测试通过。
2. RuntimeInputFrame -> ActionSnapshot 测试通过。
3. ActionSnapshot -> ProjectLogicRunner 测试通过。
4. 输入驱动 ECS 写入并进入 RenderExtract 的端到端测试通过。
5. RuntimeTrace 能记录 route / action / frame。
6. cargo test 相关测试通过。
7. 文档同步完成。
```
