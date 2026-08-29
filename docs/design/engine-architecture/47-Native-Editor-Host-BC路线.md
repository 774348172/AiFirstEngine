# Native Editor Host C-lite 与自研 UI Renderer 路线

> 当前真实 UI present 规则见 `111-Native-Editor-Real-UI-Present-方案B.md`：窗口层负责 OS Window / EventLoop / Surface lifecycle，`editor_wgpu_renderer` 负责 `UiDrawList -> GPU commands -> present`。

本文档确认编辑器主线采用 C-lite 长期方案。

核心结论：

```text
Native Rust Editor Host 是主线。
Rust Editor Core 是编辑器业务真相。
UI Schema / Command / Layout Model 自研。
winit + wgpu + 自研 UI renderer 是正式 UI Backend v1。
egui / eframe 不进入正式窗口主线。
editor_ui_backend_egui 仅保留为 headless compatibility test adapter。
Electron / React 只作为 legacy transition shell。
后续 Dock / Inspector / Graph / Viewport Overlay 在自研 UI renderer 上逐步补齐。
```

## 为什么从 B-C 调整为 C-lite

旧 B-C 路线的问题：

```text
第一阶段接 egui / eframe，第二阶段再替换自研 UI renderer，会形成两次真实窗口路线。
编辑器越早绑定 egui widget / runtime state，后面迁移成本越高。
AI 修改 UI 时不能面对一堆手写控件回调和临时 UI 状态。
```

纯 C 路线的问题：

```text
第一阶段需要同时自研窗口、输入、布局、绘制、文本、Dock、Inspector、Runtime 接入。
变量太多，出了问题难以判断是 Runtime、Editor Core 还是 UI Framework。
即使 AI 能加速写代码，交互验证、IME、高 DPI、拖拽、性能和跨平台细节仍然需要时间。
```

C-lite 路线的判断：

```text
第一阶段就用 winit + wgpu + 自研 UI renderer 承载真实窗口底座。
第一阶段不追求完整 UI 框架，只实现 Rect / Text / Panel / Click / Selection / Viewport Shell。
真正长期资产是 Rust Editor Core、UI Schema / Command / Layout Model 和自研 UI renderer。
功能慢慢加，但底座不再走临时 egui / eframe。
```

## 总体结构

```text
Native Rust Editor Host
  -> Rust Editor Core
  -> UI Schema / UI Tree / Command / Layout Model
  -> UI Backend Interface
      -> winit + wgpu + self UI renderer backend v1
      -> optional compatibility / test backend
  -> engine_runtime crate
  -> Runtime Package / viewport texture / RenderFrameReport / RuntimeTrace / FrameHash
```

crate 建议：

```text
rust/
  crates/
    engine_runtime/
    editor_core/
    editor_host/
    editor_ui_model/
    editor_ui_renderer/
    editor_input/
    editor_window_winit/
    editor_ui_backend_egui/  # headless compatibility test adapter only
```

第一版可以先合并部分 crate，但边界必须按上面的方向设计。

## Rust Editor Core

Editor Core 负责编辑器业务真相。

第一版包含：

```text
Project Session
Runtime Package Session
Selection Model
Command Registry
Panel Layout Model
Inspector Model
Console Model
Runtime Trace View Model
Viewport View Model
```

Editor Core v1 边界已确认：

```text
第一版只管理 Runtime Package。
第一版不直接管理完整 Project。
第一版不接 Asset DB / Build Graph / AI Patch。
第一版 Inspector 只读。
```

原因：

```text
Native Host MVP 的首要目标是验证 Native Host + Rust Runtime direct call 主链路。
Runtime Package 已经是经过验证的运行时输入边界。
如果第一版直接接完整 Project / Asset / Patch / Build，会让变量过多，问题定位困难。
```

Editor Core v1 负责：

```text
EditorSession
RuntimePackageSession
RuntimeController
SelectionState
CommandRegistry
DiagnosticsStore
ConsoleStore
RuntimeTraceStore
PanelLayoutState
ViewModelBuilder
```

Editor Core v1 命令：

```text
open_runtime_package
reload_runtime_package
select_entity
tick_one_frame
play
pause
step_frame
reset_runtime
clear_console
select_trace_entry
```

Editor Core v1 数据流：

```text
UI Backend
  -> UiCommand
  -> Editor Core
  -> engine_runtime crate
  -> viewport texture / RenderFrameReport / RuntimeTrace / FrameHash
  -> Editor Core ViewModel
  -> UI Backend 渲染
```

不负责：

```text
winit 窗口事件细节。
self UI renderer 绘制细节。
wgpu 绘制细节。
操作系统窗口。
React / Electron legacy 状态。
完整 Project 编辑。
字段写回。
Asset DB。
Build Graph。
AI Patch。
Undo / Redo。
真实资源导入。
真实 Native RHI。
```

规则：

```text
Editor Core 不依赖 egui / eframe。
Editor Core 不依赖 winit。
Editor Core 不依赖 wgpu。
Editor Core 不依赖 Electron。
Editor Core 不依赖 DOM。
Editor Core 不直接持有 GPU backend。
Editor Core 只输出 UI Model / Command / Diagnostics。
```

## 当前落地状态：A7-A9 默认自动化闭环

当前代码已经落地以下 Native Editor 主线 crate：

```text
editor_ui_renderer：
  Self UI Renderer 的第一版数据层。
  输入 EditorUiModel。
  输出 UiDrawList / DrawCommand / HitRegion。
  不持有业务状态。

editor_input：
  Editor Input Router 的第一版数据层。
  输入 EditorInputEvent + UiDrawList。
  输出 UiCommand。
  不执行业务命令。

editor_window_winit：
  Native Window 的第一版 skeleton。
  默认测试路径固定 window attributes 和 wgpu surface plan。
  real-window feature 才编译真实 winit / wgpu 依赖。
```

当前验证：

```powershell
npm.cmd run test:nativeeditor
```

真实窗口 feature 门禁：

```powershell
npm.cmd run test:nativeeditor:realwindow
```

当前边界：

```text
Self UI Renderer 尚未接真实 GPU 绘制。
editor_window_winit 尚未打开真实 OS window。
真实 winit / wgpu surface 初始化等待 GUI 工具链门禁。
Editor Core 仍保持不依赖 winit / wgpu / renderer。
```

## UI Schema / UI Model

UI Model 是 AI 和 UI Backend 之间的稳定协议。

UI Model v1 采用：

```text
Typed ViewModel + Command Intent
```

含义：

```text
Editor Core 输出类型化 ViewModel。
UI Backend 只渲染 ViewModel。
所有交互都变成 UiCommand。
UiCommand 是 UI Backend 唯一交互出口。
```

整体结构：

```text
EditorUiModel
  frame
  active_runtime_package
  panels
  toolbar
  hierarchy
  inspector
  viewport
  console
  runtime_trace
  diagnostics
```

### PanelLayoutModel

第一版固定布局，但必须模型化：

```text
PanelLayoutModel
  layout_id
  mode = Fixed
  regions[]

PanelRegion
  region_id
  panel_ids[]
  active_panel_id?
```

固定区域：

```text
top: toolbar
left: hierarchy
center: viewport
right: inspector
bottom: console / runtime_trace
```

### ToolbarModel

```text
ToolbarModel
  commands[]
  runtime_state

ToolbarCommand
  command_id
  label
  enabled
  reason_disabled?
```

命令：

```text
open_runtime_package
reload_runtime_package
play
pause
step_frame
tick_one_frame
reset_runtime
```

### HierarchyModel

```text
HierarchyModel
  scene_id?
  roots[]
  selected_entity_id?

HierarchyNode
  entity_id
  label
  alive
  children[]
```

交互只允许：

```text
select_entity(entity_id)
```

### InspectorModel

第一版只读：

```text
InspectorModel
  selected_entity_id?
  title
  sections[]
  readonly = true

InspectorSection
  section_id
  title
  fields[]

InspectorField
  field_id
  label
  value
  value_type
  path
  readonly = true
```

第一版值类型：

```text
String
Bool
Number
Vec3
AssetRef
EntityRef
Json
```

字段来源：

```text
Transform
Renderable
Hierarchy
Metadata
```

### ViewportModel

第一版不是 3D 渲染，只展示 Runtime 输出摘要：

```text
ViewportModel
  scene_id?
  frame
  frame_hash?
  renderable_count
  selected_entity?
  renderables[]

RenderableSummary
  entity_id
  mesh_ref?
  material_ref?
  local_position
  visible
```

### ConsoleModel

```text
ConsoleModel
  entries[]
  unread_error_count
  unread_warning_count

ConsoleEntry
  entry_id
  level
  source
  message
  frame?
  timestamp_ms?
```

来源：

```text
Editor
Runtime
Command
Package
```

### RuntimeTraceModel

```text
RuntimeTraceModel
  frame
  entries[]
  selected_entry_id?

TraceEntryView
  entry_id
  frame
  phase
  system_id
  message
  entity_id?
  level
```

交互：

```text
select_trace_entry(entry_id)
```

如果 trace entry 带 `entity_id`：

```text
Editor Core 同步 select_entity(entity_id)。
```

### UiCommand

```text
UiCommand
  command_id
  payload
```

第一版 payload：

```text
OpenRuntimePackage { path }
ReloadRuntimePackage
SelectEntity { entity_id }
TickOneFrame
Play
Pause
StepFrame
ResetRuntime
ClearConsole
SelectTraceEntry { entry_id }
```

### Transaction-first Command System

UiCommand 执行规则采用方案 C：

```text
Transaction-first Command System
```

原因：

```text
Unity / Unreal / Godot 的成熟编辑器都不是 UI 直接改数据。
编辑器操作需要统一命令、事务、诊断、回滚和审计入口。
本引擎后续还要接 AI Patch、字段编辑、Project 修改、Asset 操作和 Undo / Redo。
如果第一版只返回 bool / string 或临时 CommandResult，后续必然重写。
```

执行链路：

```text
UiCommand
  -> CommandRequest
  -> CommandTransaction
  -> Validate
  -> Execute
  -> Commit / Reject
  -> CommandResult
  -> Diagnostics / Console / ViewModel rebuild
```

CommandTransaction v1：

```text
CommandTransaction
  transaction_id
  request_id
  command_id
  source
  payload
  status
  read_set[]
  write_set[]
  diagnostics[]
  state_changes[]
  undo_policy
```

status：

```text
pending
validated
committed
rejected
failed
```

undo_policy v1：

```text
none
snapshot_ready
future_undoable
```

CommandResult v1：

```text
CommandResult
  transaction_id
  request_id
  command_id
  status
  diagnostics[]
  console_entries[]
  state_changes[]
  ui_model_revision
```

EditorDiagnostic v1：

```text
EditorDiagnostic
  severity
  code
  message
  source
  command_id?
  request_id?
  path?
  entity_id?
  trace_entry_id?
  suggested_action?
```

StateChangeSummary v1：

```text
StateChangeSummary
  kind
  path
  before_summary?
  after_summary?
```

第一版命令 undo_policy 建议：

```text
select_entity -> none
select_trace_entry -> none
tick_one_frame -> none
clear_console -> snapshot_ready
open_runtime_package -> future_undoable
reload_runtime_package -> future_undoable
reset_runtime -> future_undoable
play / pause / step_frame -> none
```

第一版硬规则：

```text
UiCommand 是 UI Backend 唯一交互出口。
Editor Core 是唯一命令执行者。
所有 UiCommand 必须创建 CommandTransaction。
命令失败必须返回结构化 diagnostics。
命令失败必须进入 Console 或 DiagnosticsStore。
第一版不允许 partial_success。
第一版不做完整 Undo / Redo 栈。
第一版不做复杂 Project 写回。
StateChangeSummary 只做摘要，不存完整反向补丁。
```

目标：

```text
AI 可以理解当前有哪些面板、字段和命令。
Editor Core 可以生成 UI Model。
self UI renderer 只负责渲染 UI Model。
headless compatibility backend 也只能消费同一份 UI Model。
```

禁止：

```text
把业务状态存在 UI widget local state 中。
让 UI callback 直接修改 Runtime Package / Project。
让 AI 生成 UI renderer 代码来完成编辑器业务功能。
```

测试闭环：

```text
初始状态生成 Hierarchy。
select_entity 命令更新 Inspector。
Inspector 保持 readonly。
tick_one_frame 命令更新 Viewport frame / frameHash。
RuntimeTrace 记录 tick。
UI Backend 不直接调用 engine_runtime。
```

## UI Backend v1：winit + wgpu + 自研 UI renderer

winit + wgpu + 自研 UI renderer 的定位：

```text
Native Editor Host v1 的正式窗口与 UI 绘制底座。
用于验证窗口、面板、Inspector、Trace、Console、Viewport shell。
不是完整 Unity / Unreal 级 UI Framework，功能按模块逐步补齐。
```

## Real-window Gate 历史方案 B：窗口壳 + 事件协议 + DrawList 渲染后端

方案 B 已被后续 C-min 取代，不再作为当前 real-window / viewport gate 主路线。

它仍保留为历史对比：如果只做窗口壳、事件协议和 DrawList 渲染后端，施工风险更小，但不能在第一版验证真实 Editor Viewport 闭环。

核心判断：

```text
真实窗口不能直接承载业务逻辑。
真实窗口不能绕过 Editor Core。
真实窗口只负责 OS window、surface、event loop 和 present。
输入路由只负责从 HitRegion 生成 UiCommand。
Renderer 只负责把 UiDrawList 变成 GPU 绘制。
Editor Core 仍是业务真相和命令执行者。
```

标准链路：

```text
editor_host
  -> editor_window_winit
      -> winit EventLoop / Window
      -> WindowEvent 转 EditorInputEvent
      -> wgpu surface / device / queue / swapchain
  -> editor_input
      -> HitRegion 命中
      -> UiCommand
  -> editor_core
      -> 执行 UiCommand
      -> 生成 CommandTransaction / Diagnostics / StateChangeSummary
      -> 输出 EditorUiModel
  -> editor_ui_renderer
      -> EditorUiModel -> UiDrawList
  -> editor_wgpu_renderer
      -> UiDrawList -> GPU commands -> present
```

分层规则：

```text
Window 不执行业务。
Window 不直接修改 Editor Session。
Window 不直接修改 Runtime Package。
Input 不修改项目。
Input 不调用 Editor Core。
Renderer 不调用 Editor Core。
Renderer 不调用 engine_runtime。
Renderer 不保存业务状态。
Editor Core 不依赖 winit / wgpu / egui / Electron。
UiModel 是编辑器显示真相。
UiCommand 是用户操作入口。
DrawList 是 UI 绘制真相。
CommandTransaction 是业务执行证据。
```

第一版只做：

```text
创建真实 OS window。
窗口 resize。
窗口 close。
鼠标点击。
Toolbar / Hierarchy 命中。
DrawList 绘制 rect / text。
wgpu clear + 最小 UI 绘制。
```

第一版不做：

```text
Dock 拖拽。
复杂文本编辑。
菜单系统。
快捷键系统。
真实 Scene 3D viewport。
Gizmo。
多窗口。
插件 UI。
```

和市场方案关系：

```text
更接近 Unreal Slate 的长期方向：自研编辑器 UI 底座。
不同点是上层不直接暴露 Slate 式 C++ Widget，而是暴露 AI 友好的 UiModel / UiCommand / DrawList 数据协议。
比 Unity / Godot 的编辑器 UI 路线更重，但更适合 AI 生成、验证、回放和调试。
```

## Native Editor real-window / Viewport Surface Gate

本 gate 的目标不是“先做一个窗口”，而是确认编辑器从 headless / mock / Electron transition 进入真实 Native Editor Host 主线时，窗口、UI、Viewport、Runtime Renderer 的边界。

参考成熟引擎：

```text
Unreal：
  FSlateApplication / SWindow
    -> Slate UI
    -> SViewport
        -> FSceneViewport
            -> ViewportClient / Scene rendering

Unity：
  ContainerWindow / HostView / EditorWindow
    -> SceneView / GameView
        -> Camera / Render Pipeline

Bevy：
  WindowPlugin
    -> Window entity
  WinitPlugin
    -> winit event loop
  RawHandleWrapper
    -> renderer 创建 surface
  bevy_render
    -> WindowSurfaces / wgpu device / queue / present
```

本项目采用接近 Unreal 的编辑器结构，但保留 AI-native 的 Schema / Trace / Command 层：

```text
NativeEditorHost
  -> EditorWindowBackend
      -> OS Window / EventLoop
      -> Surface lifecycle
  -> EditorCore
      -> UI Schema / Layout / Command / Transaction
  -> EditorUiRenderer
      -> Toolbar / Hierarchy / Inspector / Console / Panel chrome
  -> ViewportHost
      -> Scene / Game Viewport 区域、尺寸、焦点、输入路由
  -> RuntimeRenderer
      -> 世界内容渲染到 Viewport render target / surface
```

边界规则：

```text
EditorWindowBackend 只负责 OS window、event loop、surface lifecycle、present。
EditorCore 只负责编辑器状态、命令、事务、diagnostics。
EditorUiRenderer 只负责编辑器 UI。
ViewportHost 只负责 Viewport 作为编辑器区域的注册、尺寸、焦点和输入归属。
RuntimeRenderer 只负责 Scene / Game 世界内容渲染。

Editor UI Renderer 不画 3D world。
Runtime Renderer 不画 Hierarchy / Inspector / Console / Toolbar。
WindowBackend 不执行业务命令。
ViewportHost 不保存项目真相。
```

当前确认采用方案 C-min：

```text
第一版就进入真实 Editor Viewport 闭环。
但每个能力只做最小可验证形态，不做完整 Unity / Unreal 级编辑器功能。
```

C-min 和市场方案的关系：

```text
比 Unreal / Unity 的完整编辑器 viewport 小很多，只保留它们的结构骨架。
比 Bevy 的 window + renderer 路线多一层 EditorCore / ViewportHost，因为我们需要编辑器语义和 AI 查错。
比纯窗口壳方案更长期主义，因为第一版就验证 UI / Viewport / RuntimeRenderer 的真实组合点。
```

第一版 gate 必须做到：

```text
打开真实 winit OS window。
创建 wgpu instance / adapter / device / queue / surface。
支持 resize / dpi scale / close / redraw。
Editor UI Renderer 能画最小固定 panel layout。
ViewportHost 能注册一个 Scene Viewport rect，并维护 viewport size / focus / output_kind。
Runtime Renderer 输出 clear color / test triangle / test texture 到 Scene Viewport。
输入事件进入 Editor Input Router，再分发给 UI 或 Viewport。
SceneCamera 使用最小 editor camera state，只支持固定视角或最小 orbit/pan 占位。
SelectionOutline 使用占位 draw pass 或 report 字段表示，不实现复杂拾取。
Gizmo 只允许 placeholder / disabled state，不实现完整 transform gizmo。
生成 RealWindowGateReport，记录 window / surface / device / frame / resize / present 状态。
```

Headless 测试硬规则：

```text
C-min 中所有能力都必须能 headless 测试。
真实 OS window / real wgpu surface 不能成为唯一验证方式。
每个 real backend 都必须有 headless / mock / noop backend 等价入口。
自动化测试默认跑 headless backend。
real-window feature 只作为本机 smoke gate 和平台兼容 gate。
```

必须支持 headless 测试的能力：

```text
EditorWindowBackend：
  使用 HeadlessWindowBackend 模拟 create / resize / dpi / close / redraw。

Surface lifecycle：
  使用 HeadlessSurfaceBackend 模拟 create / configure / acquire / present / surface lost。

EditorInputEvent：
  使用纯数据事件序列测试 winit event -> EditorInputEvent -> UI / Viewport routing。

ViewportHost：
  使用纯数据测试 viewport rect / focus / output_kind / resize。

EditorUiRenderer：
  使用 DrawList snapshot 测试 panel shell / viewport border / hit region。

RuntimeRenderer：
  使用 HeadlessRuntimeRenderer 输出 test frame descriptor / frame hash，不依赖真实 GPU。

SceneCamera：
  使用纯数据测试 camera state 更新，不依赖真实鼠标或窗口。

SelectionOutline：
  使用 report / draw-pass descriptor 测试占位状态，不依赖真实拾取。

Gizmo：
  使用 disabled / placeholder state 测试，不实现真实 transform gizmo。

RealWindowGateReport：
  headless 和 real-window 都必须输出同结构 report。
```

测试门禁：

```text
CI / 默认自动化：
  必须通过 headless C-min gate。

本机 real-window：
  可以手动或显式 feature 跑 smoke gate。
  失败时不能阻塞普通 headless CI，但必须输出 RealWindowGateReport。

禁止：
  任何 C-min 模块只有真实窗口路径、没有 headless 测试路径。
  任何业务逻辑只能通过真实鼠标点击验证。
  任何 renderer 状态只能通过肉眼看窗口验证。
```

第一版 gate 不做：

```text
完整 docking。
多窗口。
真实 RDG。
复杂材质。
完整 gizmo。
Asset previewer。
平台原生菜单。
D3D12 / Vulkan / Metal native backend。
复杂鼠标拾取。
复杂场景编辑。
完整 Scene 工具体系。
```

RealWindowGateReport 第一版最小字段：

```text
window:
  created
  size
  dpi_scale
  close_requested

surface:
  created
  configured
  format
  present_mode
  last_resize

gpu:
  adapter_name
  backend
  device_created
  queue_created

frame:
  frame_index
  redraw_requested
  acquired_surface_texture
  presented
  error_code

viewport:
  viewport_id
  rect
  focused
  output_kind
  camera_state
  selection_outline_state
  gizmo_state

diagnostics:
  severity
  message
  source_stage
```

这个 report 只用于 gate / diagnostics / AI 查错，不作为运行时热路径必备数据。

Viewport 合成规则：

```text
Editor UI Renderer 画 Scene 面板外壳和控件。
Runtime Renderer 画 Scene / Game 世界内容。
Scene Viewport 把 Runtime Renderer 的 render target / texture 嵌入 Editor UI。
Editor-only grid / gizmo / selection outline / debug draw 属于 Runtime Renderer 的 EditorViewport pass 或 Editor Viewport Overlay，不属于普通面板控件。
```

禁止：

```text
self UI renderer 直接画游戏世界。
Runtime Renderer 直接画 Hierarchy / Inspector / Console / Toolbar。
Scene View 维护第二套独立世界渲染管线。
```

允许：

```text
用 winit 管理窗口和输入事件。
用 wgpu 承载 UI 绘制和后续 viewport surface。
用自研 UI renderer 渲染 Panel / Tree / Inspector / Console 的最小控件。
为测试实现最小交互。
```

不允许：

```text
业务逻辑写死在 UI widget 里。
Command 绕过 Editor Core。
Selection 绕过 Selection Model。
Panel 布局只存在 renderer runtime state。
Runtime tick 由 UI widget 直接驱动。
```

## Native Editor Viewport 输入回流规则

Viewport 输入归属采用接近 Unreal / Unity 的结构：

```text
WindowEvent
  -> EditorInputEvent
  -> UI HitTest
  -> ViewportHost / ViewportInputGateway
  -> ViewportInputRoute
      -> EditorToolCommand
      -> SceneCameraCommand
      -> RuntimeInputFrame
  -> InputResolver
  -> ActionSnapshot
  -> EngineHostLoop tick
```

参考引擎：

```text
Unreal:
  FSceneViewport 接收 Slate 输入。
  FSceneViewport 调用 ViewportClient->InputKey / InputAxis。
  UGameViewportClient 再路由到 PlayerController。

Unity:
  GameView.OnGUI 把 Event.current 转成 GameView input event。
  SceneView 优先处理编辑器相机、Handle、Gizmo、Selection。

Bevy:
  WindowEvent 先转成引擎输入资源 / PointerInput。
  项目系统读取处理后的输入状态，而不是读平台窗口事件。
```

本项目规则：

```text
UI 永远优先吃输入。
Scene View 默认输入归编辑器工具。
Game View / Play Mode 获得焦点后才把输入送进 Runtime Action。
ViewportHost 只维护 viewport 区域、焦点和输入归属。
ViewportHost 不执行项目逻辑。
EditorInputRouter 不直接调用 engine_runtime。
项目逻辑只读 ActionSnapshot。
Runtime Trace 记录输入路由摘要和 ActionSnapshot 摘要。
```

第一版只支持：

```text
PointerDown / PointerMove / PointerUp
KeyDown / KeyUp
UI or Viewport routing
SceneView editor tool route
GameView runtime action route
ActionSnapshot -> EngineHostLoop
headless 测试
```

## Native Host 与 Rust Runtime

第一版主链路：

```text
Native Editor Host
  -> Editor Core
  -> Runtime Package Loader
  -> engine_runtime
  -> tick
  -> viewport texture / RenderFrameReport / RuntimeTrace / FrameHash
  -> Editor Core View Model
  -> self UI renderer / wgpu 展示
```

规则：

```text
Native Host 第一版直接调用 engine_runtime crate。
不围绕 Electron sidecar 设计。
不通过 TypeScript Runtime。
不读取 React / Electron 内存 Project Object。
第一版输入使用 Runtime Package。
```

## Electron / React Legacy Shell

Electron / React 定位：

```text
legacy transition shell
```

允许：

```text
继续维护已有功能。
修复现有 bug。
作为 Native Host 完成前的可用编辑器。
使用现有 Build / Report / Patch / Console 面板。
```

不允许：

```text
新增底层主线能力。
新增 Electron-only Runtime 接口。
把 Runtime Preview 主线绑回 Electron。
让 React 组件成为新 Editor Core。
```

## 第一阶段目标

时间目标：

```text
3-5 周形成 Native Editor MVP。
```

第一版面板范围已确认：

```text
Toolbar
Hierarchy
Inspector
Viewport Shell
Console
RuntimeTrace
```

第一版布局采用固定布局，不做复杂 Dock：

```text
Toolbar
------------------------------------------------
Hierarchy | Viewport Shell | Inspector
------------------------------------------------
Console / RuntimeTrace
```

第一阶段必须看到：

```text
Native Host 可启动。
可选择 / 加载 Runtime Package。
可调用 Rust Runtime tick。
可显示 Hierarchy。
可显示 Inspector。
可显示 Console。
可显示 RuntimeTrace。
可显示 Viewport shell。
可显示 RenderFrameReport 摘要。
```

### Toolbar v1

职责：

```text
Open Runtime Package
Reload
Play / Pause
Step Frame
Tick 1 Frame
Reset Runtime
```

第一版不做：

```text
菜单系统
快捷键系统
复杂工具模式
AI 命令入口
Build 按钮
```

### Hierarchy v1

职责：

```text
显示当前 runtime scene entity 树。
显示 entityId / name / parent / children / alive state。
点击选择 Entity。
选中后同步 Inspector。
```

第一版不做：

```text
创建 / 删除 Entity
拖拽改变层级
Prefab 实例化
搜索过滤
右键菜单
```

### Inspector v1

职责：

```text
显示选中 Entity 的只读数据。
显示 Transform / Renderable / Hierarchy / 基础 metadata。
```

第一版不做：

```text
字段编辑
添加 / 删除 Component
复杂字段控件
Patch Plan 修改
```

原因：

```text
第一版先验证 Native Host 能正确读取 Runtime 输出，不急着写回项目。
```

### Viewport Shell v1

职责：

```text
显示 sceneId
显示 frame
显示 frameHash
显示 renderable count
显示 selected entity summary
显示 renderables table / simple 2D placeholder
```

第一版不做：

```text
真实 3D 渲染
相机控制
Transform gizmo
选中描边
材质预览
```

### Console v1

职责：

```text
显示 Editor diagnostics
显示 Runtime diagnostics
显示 Command result
显示 Package load result
```

第一版不做：

```text
AI 对话
Patch Review
Build Report
复杂过滤
```

### RuntimeTrace v1

职责：

```text
显示 frame
显示 phase
显示 systemId
显示 message
显示 entityCount
显示 diagnostic level
点击 trace 可以定位相关 Entity，如果 trace 带 entityId。
```

第一版不做：

```text
复杂火焰图
Profiler
Replay 对比
IR source map 详细展开
```

### 第一版成功标准

```text
Native Editor Host 可以启动。
Toolbar 可以打开 Runtime Package。
Hierarchy 可以显示 scene entity。
点击 Entity 后 Inspector 更新。
Toolbar 可以 tick runtime。
Viewport Shell 可以显示 frame / frameHash / renderable count。
RuntimeTrace 可以显示本帧 trace。
Console 可以显示加载、tick、错误日志。
整个流程不依赖 Electron。
```

第一阶段不做：

```text
AI Panel
Project / Asset Browser
Build Report Panel
完整 Dock 系统。
完整 Graph Editor。
字段编辑。
真实 GPU 3D viewport。
真实 Asset Import UI。
真实 Build Graph UI 全量迁移。
菜单 / 快捷键。
Electron 删除。
```

## 后续替换路线

替换顺序：

```text
1. 自研 Panel Layout / Dock Model
2. 自研 Inspector Field Widgets
3. 自研 Tree / Asset Browser
4. 自研 Runtime Trace / Profiler View
5. 自研 Graph Editor
6. 自研 Viewport Overlay
7. 自研完整 UI Backend
```

替换原则：

```text
先稳定 UI Model，再替换渲染 Backend。
替换 Backend 不改变 Editor Core。
替换 Backend 不改变 Command / Selection / Patch / Validation。
```

## 成功标准

```text
Native Host 能独立于 Electron 启动。
Native Host 能加载 Runtime Package。
Native Host 能调用 Rust Runtime。
Native Host 能展示 Runtime 输出。
UI 业务状态存在 Editor Core / UI Model，不存在 self UI renderer / egui 细节中。
Electron 不再是新能力主线。
```
