# Native Editor Viewport 输入回流 Runtime Frame 方案

## 当前归属说明：Projection 术语

本文中如果出现以下历史名称：

```text
RenderExtract
RenderAssetBridge / Render Asset Bridge
Physics2DBridge
RuntimeScene Hydration
AuiRenderExtract / AuiRendererBridge
SpriteRenderer2D ECS-to-RenderProxy Bridge
```

统一按 `110-World-Projection-Adapter统一跨域同步规则.md` 理解为：

```text
RenderProjection
AssetProjection
Physics2DProjection
HydrationProjection
UiProjection
RenderProjectionAdapter<SpriteRenderer2D>
```

这些名称可以作为历史实现名保留，但不再作为新增架构概念扩展。后续新增类型只新增对应 `ProjectionAdapter`，不新增独立 Bridge。

本文档定义 Native Editor Viewport 中的输入如何进入 Runtime Action / Game Frame。

本规则不重新讨论 Input System 总路线。Input Action / Binding / Processor / Trigger / ActionSnapshot 的总规则见：

```text
40-Input-System路线.md
47-Native-Editor-Host-BC路线.md
```

## 问题是什么

当前已经具备：

```text
Native Editor Host
ViewportHost
Runtime Viewport texture / frame summary
Input System MVP
EngineHostLoop
ProjectLogicRunner
Rust ECS
```

但还缺一条正式闭环：

```text
用户在 Native Editor Viewport 中按键 / 鼠标操作
  -> 判断输入属于 UI、Scene View 编辑器工具，还是 Game View Runtime
  -> 生成 Runtime 可读的 ActionSnapshot
  -> ActionSnapshot 随当前帧进入 EngineHostLoop
  -> ProjectLogicRunner 在项目逻辑中读取输入
  -> ECS / RenderCommand / RuntimeRenderer 进入下一帧
```

这个系统的核心不是“如何支持每一种设备”，而是输入归属边界：

```text
UI 是否吃掉输入？
Scene View 输入是否只控制编辑器工具？
Game View 什么时候把输入交给 Runtime？
项目逻辑读取什么格式？
AI 和 Trace 如何定位输入相关 Bug？
```

## 其它引擎怎么做

### Unreal Engine

UE 的路线是 ViewportClient 边界。

```text
Slate 输入事件
  -> FSceneViewport
  -> FViewportClient::InputKey / InputAxis
  -> UGameViewportClient
  -> PlayerController / PlayerInput
```

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Slate\SceneViewport.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\GameViewportClient.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\PlayerController.cpp
```

关键点：

```text
Viewport 不直接等于项目逻辑。
Viewport 先交给 ViewportClient。
GameViewportClient 再决定是否交给 PlayerController。
Editor viewport 和 game viewport 有不同输入归属。
```

### Unity

Unity 的路线是 GameView / SceneView 分离。

```text
EditorWindow / IMGUI Event
  -> GameView.OnGUI / SceneView.OnGUI
  -> GameView 进入 Play Mode 输入
  -> SceneView 优先处理相机、Handle、Gizmo、Selection
  -> PlayerLoop 执行项目逻辑
```

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\GameView\GameView.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SceneView\SceneView.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\PlayerLoop\PlayerLoop.bindings.cs
```

关键点：

```text
Game View 和 Scene View 的输入语义不同。
Scene View 默认服务编辑器工具。
Game View / Play Mode 焦点决定输入是否进入 Runtime。
```

### Bevy

Bevy 的路线是数据化输入。

```text
winit WindowEvent
  -> Bevy window / input events
  -> ButtonInput / MouseMotion / MouseWheel 等资源
  -> PreUpdate 准备输入
  -> Update 中项目系统读取输入状态
```

源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_winit\src\state.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_input\src\lib.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app\src\main_schedule.rs
```

关键点：

```text
项目系统不直接读平台窗口事件。
Input 在项目逻辑前准备好。
测试可以通过数据化事件和输入资源完成。
```

## 方案对比

### 方案 A：EditorInputEvent 直接调用 Runtime tick

```text
WindowEvent -> EditorInputEvent -> Runtime tick
```

优点：

```text
实现最短。
第一版能很快看到输入影响画面。
```

缺点：

```text
UI、Scene View、Game View 边界混乱。
项目逻辑会被编辑器事件污染。
AI 查 Bug 时难判断输入到底在哪一层被消费。
后期多窗口、多 Viewport、Replay、Golden Scenario 都会补债。
```

结论：

```text
不采用。
```

### 方案 B：完全做 UE / Unity 级完整输入管线

```text
完整设备层
完整焦点系统
完整多窗口多 Viewport
完整 Gamepad / Touch / IME
完整 Rebinding UI
完整 Scene Tool / Gizmo
```

优点：

```text
长期能力最强。
接近成熟引擎。
```

缺点：

```text
第一版范围过大。
会拖慢最小游戏循环。
大量设备细节和 UI 细节暂时不是主线。
```

结论：

```text
不作为第一版。
```

### 方案 C：ViewportInputGateway C-min

```text
WindowEvent
  -> EditorInputEvent
  -> UI HitTest
  -> ViewportHost
  -> ViewportInputGateway
  -> ViewportInputRoute
      -> UiConsumed
      -> EditorToolCommand
      -> SceneCameraCommand
      -> RuntimeInputFrame
  -> InputResolver
  -> ActionSnapshot
  -> EngineHostLoop.tick_runtime_frame
```

优点：

```text
保留 UE 的 ViewportClient 边界。
保留 Unity 的 Scene View / Game View 输入归属。
保留 Bevy 的数据化输入和 headless 测试能力。
项目逻辑只读 ActionSnapshot，对 AI 最清楚。
第一版范围足够小，可以直接施工。
```

缺点：

```text
比直接调用 Runtime 多一层 ViewportInputGateway / Route。
多窗口、多 Viewport、复杂设备仍需后续扩展。
```

结论：

```text
采用方案 C-min。
```

## 本项目正式规则

### 总链路

```text
WindowEvent
  -> EditorInputEvent
  -> UI HitTest
  -> ViewportHost
  -> ViewportInputGateway
  -> ViewportInputRoute
      -> UiConsumed
      -> EditorToolCommand
      -> SceneCameraCommand
      -> RuntimeInputFrame
  -> InputResolver
  -> ActionSnapshot
  -> EngineHostLoop.tick_runtime_frame
  -> ProjectLogicRunner
  -> ECS
  -> RenderExtract
  -> RuntimeRenderer
```

### 输入归属规则

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

### ViewportHost 职责

ViewportHost 只保存编辑器 viewport 的外壳状态：

```text
viewport_id
viewport_kind = SceneView | GameView
rect
focused
hovered
output_kind
latest_runtime_frame_summary
```

ViewportHost 不负责：

```text
不执行项目逻辑。
不解析 InputAction。
不修改 ECS。
不直接调用 ProjectLogicRunner。
不保存项目真相。
```

### ViewportInputGateway 职责

ViewportInputGateway 是 Editor Input 与 Runtime Input 的边界。

它负责：

```text
接收 EditorInputEvent。
查询 UI HitTest 结果。
查询 ViewportHost 的 viewport 区域和焦点。
判断输入归属。
输出 ViewportInputRoute。
```

它不负责：

```text
不执行 Runtime tick。
不执行项目逻辑。
不解析项目 Input Binding。
不写 ECS。
```

### ViewportInputRoute

第一版 route 类型：

```text
UiConsumed
SceneCameraCommand
EditorToolCommand
RuntimeInputFrame
Ignored
```

第一版 route 必须记录：

```text
route_kind
viewport_id
viewport_kind
focused
hovered
input_event_kind
reason
```

### RuntimeInputFrame

RuntimeInputFrame 是进入 InputResolver 前的运行时输入帧。

第一版字段：

```text
frame_id
viewport_id
events[]
modifiers
pointer_position
```

第一版 event 类型：

```text
PointerDown
PointerMove
PointerUp
KeyDown
KeyUp
```

### InputResolver

InputResolver 负责：

```text
RuntimeInputFrame
  -> Input Mapping
  -> Processor
  -> Trigger / Activation Rule
  -> ActionSnapshot
```

第一版只支持：

```text
Button Action
Axis2 Action
Pointer Action
```

项目逻辑读取：

```text
ActionSnapshot
```

项目逻辑禁止读取：

```text
WindowEvent
EditorInputEvent
ViewportInputRoute
RuntimeInputFrame
Raw KeyCode
Raw MouseButton
```

## 与 EngineHostLoop 的关系

EngineHostLoop 每帧接收一个可选 ActionSnapshot：

```text
EngineHostLoop.tick_editor_frame
  -> EditorInputRouter
  -> ViewportInputGateway
  -> RuntimeInputFrame
  -> InputResolver
  -> ActionSnapshot
  -> EngineHostLoop.tick_runtime_frame(action_snapshot)
```

Runtime Frame 内部顺序：

```text
BeginFrame
InputSnapshotReady
FixedUpdate
ProjectLogicRunner.fixed_update
Update
ProjectLogicRunner.frame_update
LateUpdate
RenderExtract
RuntimeRenderer
EndFrame
```

规则：

```text
ActionSnapshot 必须在 ProjectLogicRunner 前准备好。
ProjectLogicRunner 只读当前帧 ActionSnapshot。
RenderExtract 只读取 ProjectLogicRunner 修改后的 ECS dirty 结果。
```

## Trace / Report

第一版必须能查清楚三类问题：

```text
输入为什么没有进 Runtime？
输入进入 Runtime 后生成了什么 Action？
Action 进入哪一帧？
```

Runtime Trace 最小字段：

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

Editor Trace 最小字段：

```text
editor_frame_id
input_event_kind
ui_hit
viewport_hit
focused_viewport_id
route_kind
reason
```

## 第一版范围

第一版只做：

```text
PointerDown / PointerMove / PointerUp
KeyDown / KeyUp
UI or Viewport routing
SceneView editor tool route
GameView runtime action route
ActionSnapshot -> EngineHostLoop
Runtime Trace 输入摘要
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
真实 OS window 依赖测试
```

## AI 友好规则

AI 不生成：

```text
底层 WindowEvent 处理代码。
EditorInputEvent 到 Runtime 的直连代码。
绕过 ActionSnapshot 的项目逻辑输入读取。
```

AI 可以生成：

```text
InputAction
Binding
Context
Processor
Trigger preset
Golden Scenario ActionSnapshot
输入相关测试用例
```

AI 查 Bug 优先看：

```text
ViewportInputRoute
ActionSnapshot
RuntimeTrace
FrameHash
ProjectLogicRunner rule trace
```

## 最小测试场景

### 测试 1：UI 吃掉输入

输入：

```text
PointerDown 落在 Toolbar button 上。
```

期望：

```text
ViewportInputRoute = UiConsumed
不生成 RuntimeInputFrame
不生成 ActionSnapshot
Runtime frame 不因该输入产生项目动作
```

### 测试 2：Scene View 控制编辑器相机

输入：

```text
PointerMove / MouseDrag 落在 focused SceneView。
```

期望：

```text
ViewportInputRoute = SceneCameraCommand
不生成 RuntimeInputFrame
不生成 ActionSnapshot
不调用 ProjectLogicRunner 输入逻辑
```

### 测试 3：Game View 输入进入 Runtime

输入：

```text
GameView focused
KeyDown Space
```

Input Mapping：

```text
Space -> action.fire
```

期望：

```text
ViewportInputRoute = RuntimeInputFrame
InputResolver 生成 ActionSnapshot(action.fire pressed)
EngineHostLoop tick_runtime_frame 读取 ActionSnapshot
ProjectLogicRunner 能读到 action.fire
RuntimeTrace 记录 route + action + frame_id
```

### 测试 4：无焦点输入不进入 Runtime

输入：

```text
GameView 未 focused
KeyDown Space
```

期望：

```text
ViewportInputRoute = Ignored
reason = viewport_not_focused
不生成 ActionSnapshot
```

## 结论

本项目采用 ViewportInputGateway C-min：

```text
UE-like ViewportClient 边界
+ Unity-like SceneView / GameView 输入归属
+ Bevy-like 数据化输入
+ 本项目 ActionSnapshot / Trace / Golden Scenario 规则
```

这个方案比直接调用 Runtime 多一层 route，但这层是必要边界：

```text
它让 UI / Editor Tool / Runtime Action 分清楚。
它让 AI 和用户能查清楚输入到底在哪里被消费。
它避免项目逻辑依赖编辑器事件。
它能 headless 测试，不被真实窗口和真实 GPU 阻塞。
```
