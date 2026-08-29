# 220-Editor GameView Input / Focus / AUI HitCandidate RoutedDispatch Productization v1 方案

## 1. 一句话说明

本系统让 Editor GameView 里的 Play 真正可操作：

```text
用户在 Editor GameView 中移动鼠标 / 点击 / 拖拽 / 按键
  -> 输入进入 RuntimePackage 对应的 runtime 实例
  -> 输入先进入命中 / 路由；220-min 只有 AUI domain 会消费
  -> 未被 AUI 消费的输入继续进入 gameplay InputResolver
```

它对标 Unity 的 `EventSystem + Raycaster + ExecuteEvents`，也对标 UE 的 `SlateApplication + WidgetPath + FReply`，但本引擎不照搬 GameObject / Slate Widget 对象模型，而是以 `RuntimeInputFrame`、`AuiLayout / AuiDrawList`、结构化 compact result 和测试期开启的 report 作为真相。

## 2. 背景与问题

219 已完成 Editor GameView 的长期主线：同进程 shared GPU texture sharing，让 Editor GameView 可以显示 runtime 画面。

但 219 明确留下缺口：

```text
GameView Input / Focus / AUI Interaction Bridge
```

当前 runtime_player_winit 已经有：

```text
RuntimeInputFrame
  -> AUI Interaction
  -> filter consumed events
  -> InputResolver
```

但 Editor GameView 还缺：

1. Editor window 输入如何只在 GameView focus / hover / capture 时进入 runtime。
2. Window 坐标如何转换成 GameView-local runtime 坐标。
3. AUI 点击如何按 layout / draw order 命中，而不是粗暴“UI 优先抢输入”。
4. AUI consumed 之后，剩余输入如何继续进入 gameplay。
5. 测试时如何生成可审查证据，但正式 runtime 默认不承担 report 成本。

注意：220-min 不是新建完整输入框架。220-min 的目标是把已有 runtime AUI interaction 链路接入 Editor GameView，并为后续 B+ 的统一候选 / 路由模型预留命名和 report 边界。

本方案的 lineage：

```text
217 = Editor Play 使用 Preview RuntimePackage 真相。
218 = In-process Editor GameView Play Runner，A1，input_bridge_status=deferred。
219 = Full GPU Texture Sharing，A2，只解决画面 present。
220 = 218 A3：GameView Input / Focus / AUI Interaction 落地。
```

## 3. Unity 源码参考

Unity UGUI 的点击不是直接发给 Button，而是先做 Raycast / HitTest：

```text
PointerInputModule
  -> EventSystem.RaycastAll
  -> BaseRaycaster 列表
      -> GraphicRaycaster
      -> PhysicsRaycaster
      -> Physics2DRaycaster
  -> FindFirstRaycast
  -> pointerCurrentRaycast.gameObject
  -> ExecuteEvents.ExecuteHierarchy / GetEventHandler
```

关键源码：

```text
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/EventSystem/InputModules/PointerInputModule.cs
  PointerInputModule.GetTouchPointerEventData / GetMousePointerEventData
  eventSystem.RaycastAll(pointerData, m_RaycastResultCache)
  FindFirstRaycast(m_RaycastResultCache)

<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/EventSystem/EventSystem.cs
  EventSystem.RaycastAll
  RaycasterManager.GetRaycasters()
  module.Raycast(eventData, raycastResults)
  raycastResults.Sort(s_RaycastComparer)

<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UI/Core/GraphicRaycaster.cs
  GraphicRaycaster.Raycast
  Graphic.raycastTarget
  canvasRenderer.cull
  RectangleContainsScreenPoint
  graphic.Raycast(pointerPosition, eventCamera)
  depth sort

<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/EventSystem/InputModules/StandaloneInputModule.cs
  ProcessMousePress
  pointerCurrentRaycast.gameObject
  ExecuteEvents.ExecuteHierarchy(pointerDownHandler)
  GetEventHandler<IPointerClickHandler>
  GetEventHandler<IDragHandler>
```

可学习点：

1. 输入事件必须先进入命中候选列表，而不是直接变成 UI action。
2. UI 和世界对象可以来自不同 Raycaster，再统一排序。
3. 控件是否响应由 handler / raycastTarget / filter 决定。

不可照搬点：

1. Unity 绑定 GameObject / Component / Camera / Raycaster，和本项目 AUI Document 为真相的模型不同。
2. Unity 的很多状态隐含在 EventSystem / GameObject hierarchy 中，不利于 AI 审查和结构化报告。

## 4. UE 源码参考

UE 普通屏幕 UI 不是 Unity 式 Raycaster 名字，但本质是 screen point -> widget path -> routed event：

```text
SlateApplication
  -> LocateWindowUnderMouse / LocateWidgetInWindow
  -> FWidgetPath
  -> RoutePointerDownEvent
  -> Tunnel / Bubble
  -> FReply::Handled / Unhandled
```

世界空间 Widget 则通过 trace 命中 WidgetComponent：

```text
WidgetInteractionComponent
  -> PerformTrace
  -> LineTraceMultiByChannel
  -> GetHitWidgetPath
  -> PressPointerKey / ReleasePointerKey
  -> Slate pointer event
```

gameplay 输入则进入 GameViewportClient：

```text
UGameViewportClient::InputKey / InputAxis
  -> ViewportConsole / override
  -> PlayerController->InputKey
```

关键源码：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Slate/Private/Framework/Application/SlateApplication.cpp
  LocateWindowUnderMouse
  LocateWidgetInWindow
  ProcessMouseButtonDownEvent
  RoutePointerDownEvent
  ProcessKeyDownEvent

<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/UMG/Private/Components/WidgetInteractionComponent.cpp
  PerformTrace
  DetermineWidgetUnderPointer
  PressPointerKey
  ReleasePointerKey
  SendKeyChar

<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/GameViewportClient.cpp
  InputKey
  InputAxis
  PlayerController->InputKey
```

可学习点：

1. WidgetPath 路由比“命中一个节点就结束”更适合复杂 UI。
2. `Handled / Unhandled` 是输入消费的核心契约。
3. focus / capture / drag 都应该由 dispatch reply 明确表达。

不可照搬点：

1. Slate / UMG 是庞大 C++ Widget 框架，本项目不应复制其对象体系。
2. UE 的 screen UI 和 world widget 入口不同，本项目应先满足 AUI screen/HUD，再预留 world pick。

## 5. 本项目当前基线

已具备：

```text
engine_input:
  RuntimeInputFrame
  RuntimeInputEvent
  filter_consumed_events
  InputResolver

engine_runtime::aui:
  AuiInteractionSystem
  AuiInteractionResult
  pointer / key / wheel / gamepad / text / IME C-min
  consumed_event_indices
  commands

runtime_player_winit:
  raw input
    -> RuntimeInputFrame
    -> AUI interaction
    -> filtered input
    -> InputResolver

editor_window_winit:
  ViewportInputGateway 雏形
  EditorInputEvent -> RuntimeInputFrame C-min

editor_core::editor_gameview_play:
  input_bridge_status 仍为 deferred
```

主要缺口：

1. Editor GameView 未正式接入 runtime input bridge。
2. 坐标转换仍需要产品化为 GameView-local runtime 坐标。
3. Editor GameView Play 尚未复用 runtime_player_winit 的 AUI interaction -> filter -> InputResolver 链路。
4. report/trace 需要分档，不能默认进入 runtime 热路径。

## 6. 可选方案

### 方案 A：RuntimeInputFrame 直通 gameplay

```text
EditorInputEvent
  -> GameView focus
  -> RuntimeInputFrame
  -> InputResolver
```

优点：

1. 最快。
2. 施工最小。

缺点：

1. HUD / 菜单 / 装备界面不能正确消费点击。
2. 点击按钮可能同时触发 gameplay fire。
3. 不符合 Unity / UE 的真实输入模型。

结论：不选。

### 方案 B：AUI 先消费，再 gameplay fallback

```text
RuntimeInputFrame
  -> AUI interaction
  -> filter consumed
  -> InputResolver
```

优点：

1. 与当前 runtime_player_winit 基线贴近。
2. 能满足复杂打飞机的 HUD / 菜单 / 装备 UI 基本交互。

缺点：

1. 如果只描述为“AUI 先消费”，容易误解成 UI 永远优先抢输入。
2. 未明确 hit candidate、visual order、routed reply。
3. 后续世界单位点击、UI 在世界前后混排时容易返工。

结论：作为 B+ 的 C-min 基础保留，但不作为最终架构描述。

### 方案 C：全量统一 UI / World / Editor Overlay Pointer Raycast

```text
Input
  -> AUI candidate
  -> World candidate
  -> Editor overlay candidate
  -> unified sort
  -> full routed dispatch
```

优点：

1. 长期最完整。
2. 自走棋 / RTS / 世界空间 UI / 场景物体选择都能纳入。

缺点：

1. 一次做太大。
2. 需要 world pick、camera ray、physics、selectable/clickable contract、cross-domain visual order 全部成熟。
3. 会拖慢复杂打飞机主线落地。

结论：作为 deferred 长期目标，不作为 220-min。

## 7. 最终选择：方案 B+

正式选择：

```text
方案 B+：Render-order Aware Hit Candidate + Routed Dispatch，Report Guarded
```

核心链路：

```text
OS / winit input
  -> EditorInputEvent
  -> Editor GameView focus / hover / capture gate
  -> GameView-local coordinate transform
  -> RuntimeInputFrame
  -> HitCandidate collect
      -> AUI HitCollector
      -> World PickCollector deferred
  -> visual-order aware sort
  -> TargetPath
  -> Routed Dispatch
      -> Consumed / PassThrough
      -> CapturePointer / ReleasePointer
      -> SetFocus / ClearFocus
      -> StartDrag / Drop / CancelDrag
      -> AuiCommand
  -> filter consumed events
  -> InputResolver / gameplay fallback
```

220-min 的实际落地映射：

```text
HitCandidate collect / TargetPath / Routed Dispatch
  在 220-min 中不新造完整 router。
  AUI domain 先复用已有 AuiInteractionSystem::process_with_state。
  AuiInteractionResult.consumed_event_indices / commands / focus / drag state
    作为本轮 compact dispatch result。

World PickCollector / EditorOverlayCollector / 完整 TargetPath bubble/tunnel
  全部 deferred。
```

也就是说，B+ 是长期架构语言；220-min 是“Editor GameView 输入桥接 + 现有 AUI interaction 产品化接入”。施工时不得为了实现文档里的长期术语而新增一套平行 UI router。

### 7.1 为什么比 Unity / UE 更适合本项目

相比 Unity：

1. Unity 的 Raycaster 思想保留，但不绑定 GameObject / Component。
2. 命中候选来自 AUI Document resolve 后的 layout/drawlist，和本项目 UI 真相一致。
3. consumed reason 可测试期结构化输出，AI 能查。

相比 UE：

1. UE 的 WidgetPath / FReply 思想保留，但不复制 Slate 对象体系。
2. routed dispatch 只保留本项目需要的 reply contract。
3. 正式 runtime 默认不生成完整 trace，避免性能负担。

## 8. Report / Trace 分档规则

本方案必须遵守全项目新增规则：

```text
Report / Trace 必须区分 runtime 和 editor，并支持档位。
正式 runtime 默认不开启重 report。
测试 / gate / debug 才允许完整 trace。
```

### 8.1 Runtime 档位

```text
RuntimeReportMode::Off
  默认。
  不新增完整 HitTraceReport JSON。
  不新增长字符串 trace。
  不保存完整 hit candidates。
  不写文件。
  只返回功能必需的 compact result。

RuntimeReportMode::Summary
  轻量统计。
  可用于 Editor Play 状态栏 / Report Panel 摘要。
  示例：input_bridge_status / consumed_count / fallback_count / focused_viewport。

RuntimeReportMode::Trace
  测试和诊断专用。
  输出完整 HitTraceReport。
  默认不得在 exported runtime 或普通 Editor Play 中开启。
```

兼容现状：

```text
已有 AuiInteractionProductizationReport / NativeInputSummary / InputTraceSummary
  不在 220 中强行重构。
  220 的硬规则是：新增的 GameView HitTraceReport / editor input bridge trace 必须受 Off / Summary / Trace 控制。
  普通 Editor Play 不得因为 220 新增每帧完整 candidate trace 或文件写入。
```

与既有渲染 report 档位的关系：

```text
engine_runtime::render_command::RenderFrameReportLevel
  现有枚举是 Off / Stats / Summary / Evidence。
  它属于 render frame report 档位，不在 220-min 中替换或回填。

220 的 Off / Summary / Trace
  只约束 GameView input bridge / HitTraceReport / editor input evidence。
  Trace 档位不是 engine_runtime::runtime_trace::RuntimeTrace 时间线。
  后续如果要统一全项目 report 档位，应单独开 report-level convergence 方案，不塞进 220。
```

### 8.2 Editor 档位

```text
EditorReportMode::Off
  普通交互不写报告。

EditorReportMode::Summary
  Editor Play 面板显示轻量状态。
  可进入 Report Panel 的摘要 provider。

EditorReportMode::Trace
  自动化 gate / 用户显式诊断 / AI 审查时开启。
  输出 GameView input bridge、focus、坐标转换、hit / dispatch / fallback 证据。
```

### 8.3 Compact Result 与 Trace Report 的区别

正式 runtime 热路径只保留功能必需结果：

```text
InputDispatchCompactResult
  consumed_event_indices / consumed_mask
  command_count
  commands
  focus_change
  pointer_capture_change
  drag_state_change
  fallback_to_gameplay_count
```

测试 / 诊断才生成完整证据：

```text
HitTraceReport
  raw_input_event
  editor_route
  game_view_rect
  local_position
  runtime_input_frame
  hit_candidates
  sort_key
  selected_target_path
  dispatch_reply
  consumed_reason
  gameplay_fallback_reason
  diagnostics
```

## 9. 220-min 施工范围

本方案第一阶段只做复杂打飞机需要的真实闭环：

1. GameView focus / hover / capture gate。
2. EditorInputEvent -> GameView-local RuntimeInputFrame。
   - 必须从 window position 转换为 GameView rect local position。
   - 再转换为 runtime texture / surface coordinate。
   - 转换必须与 219 的 Editor GameView present rect / viewport texture descriptor / DPI scale 对齐。
   - 推荐公式：
     `window_point -> game_view_rect_local -> normalized_viewport_uv -> runtime_texture_pixel`。
   - DPI scaling / viewport scaling / letterbox 如果当前不能完整处理，必须输出 Summary diagnostic，不能静默使用 window 坐标。
3. Editor GameView Play 接入 AUI interaction。
4. AUI HitCandidate 来自已有 AuiLayout / AuiDrawList / AuiInteractionSystem。
5. AUI consumed 后过滤 RuntimeInputFrame。
6. 剩余输入进入 InputResolver，驱动 gameplay action snapshot。
7. `input_bridge_status` 从 `deferred` 变为真实状态。
8. Report mode 默认 Off 或 Summary；gate 才启 Trace。
9. 输入类型范围：
   - 本轮必须支持 pointer move / down / up 与 keyboard down / up。
   - mouse wheel / text input / IME / gamepad 的 Editor GameView bridge 作为可选 Gate；如果不做，必须在 deferred_flags / Summary diagnostic 中明确。
   - runtime 侧已有这些 RuntimeInputEvent 类型不等于 Editor GameView bridge 已经完成。
10. 自动化测试证明：
   - 点击 AUI button 不触发 gameplay fire。
   - 点击 AUI 外区域可触发 gameplay fire。
   - 拖拽 AUI slot 不泄漏到 gameplay。
   - GameView 未 focus 时 keyboard 不进 runtime。
   - 坐标转换命中正确。
   - 上述逻辑测试默认走 headless deterministic gate：构造 EditorInputEvent / RuntimeInputFrame / AUI fixture 断言 consumed / fallback / focus / coordinate transform。
   - 真实 winit window / OS input smoke 只能作为 optional 或 ignored local-only，不默认阻塞 CI。

## 10. Deferred 边界

以下不进入 220-min：

1. World PickCollector。
2. Camera ray -> physics -> ECS selectable/clickable contract。
3. UI 和 world object 的全量跨域 visual order sorting。
4. 多指针 / 触摸 gesture full。
5. mouse wheel / text input / IME / gamepad 的 Editor GameView bridge，如果 220-min 施工未显式纳入 Gate。
6. Editor overlay 与 runtime AUI 的统一输入仲裁。
7. 完整 accessibility input routing。

但 220-min 的数据结构必须预留：

```text
HitCandidateDomain:
  Aui
  WorldDeferred
  EditorOverlayDeferred

HitCandidateSortKey:
  composition_stage
  layer_order
  z_order
  draw_order
  depth_or_distance
```

## 11. 验收标准

完成后应能证明：

1. Editor GameView 中的 RuntimePackage 画面可被输入操作。
2. AUI HUD / 菜单 / 装备 UI 点击、拖拽、键盘焦点能正确消费输入。
3. 未被 AUI 消费的输入继续进入 gameplay。
4. 普通 runtime 默认不生成完整 HitTraceReport。
5. 自动化 gate 可开启 Trace 并输出可审查证据。
6. Report Panel 只接 Summary / Trace 的产物，不强迫 runtime 热路径写 report。
7. Pointer 坐标必须证明已从 window 坐标转换到 GameView-local runtime 坐标；不能再直接把 window 坐标塞入 RuntimeInputFrame。
8. 方案没有新增平行 AUI router；220-min 复用 AuiInteractionSystem，完整 B+ candidate/router 留给后续。

## 12. 当前结论

220 应采用：

```text
B+：Render-order Aware Hit Candidate + Routed Dispatch，Report Guarded
```

第一阶段做 B+-min：

```text
Editor GameView Input Bridge
  + GameView-local coordinate transform
  + AUI HitCandidate / Routed Dispatch
  + consumed filter
  + gameplay fallback
  + report mode Off/Summary/Trace
```

World PickCollector 和全量 UI/world 统一排序保留为后续系统，不阻塞复杂打飞机项目继续落地。
