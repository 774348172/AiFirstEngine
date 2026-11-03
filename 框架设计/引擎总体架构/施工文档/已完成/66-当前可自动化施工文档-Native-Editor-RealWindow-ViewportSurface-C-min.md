# 当前可自动化施工文档：Native Editor RealWindow / ViewportSurface C-min

> 状态：已确认可施工。本文档只定义本轮自动化施工范围，不继续扩展 Native Editor / Renderer 长期架构规则。长期规则以 `37-Editor-Core与可迁移UI路线.md`、`47-Native-Editor-Host-BC路线.md`、`17-Runtime-FrameLoop.md` 和 `50-RenderCommand-RenderSceneState方案.md` 为准。

## 1. 施工目标

本轮目标是把已经确认的 Native Editor real-window / Viewport Surface C-min 规则落成可自动化验证的最小闭环：

```text
NativeEditorHost
  -> EditorWindowBackend
  -> EditorCore
  -> EditorUiRenderer
  -> ViewportHost
  -> RuntimeRenderer
  -> RealWindowGateReport
```

核心结果：

```text
默认自动化路径必须 headless 可测。
真实窗口路径必须有明确接口和 feature gate。
ViewportHost 成为编辑器 UI 和 Runtime Renderer 的真实组合点。
RuntimeRenderer 能输出最小 test frame descriptor / frame hash。
Editor UI 能生成最小 panel layout / viewport border / hit region DrawList。
输入事件能从窗口事件模型转换为 EditorInputEvent，并路由到 UI 或 Viewport。
RealWindowGateReport 能解释 window / surface / gpu / frame / viewport / diagnostics 状态。
```

本轮不是实现完整 Unity / Unreal 级 Scene Viewport，也不是实现真实 RDG / RHI / 多窗口 / 完整 gizmo。

## 2. 参考引擎对比

| 引擎 | 对应机制 | 可借鉴点 | 本轮取舍 |
|---|---|---|---|
| Unreal | `FSlateApplication` + `SWindow` + `SViewport` + `FSceneViewport` + `ViewportClient` | UI viewport 和引擎 viewport/render target 分离；窗口、输入、viewport、渲染边界清楚 | 学结构骨架，不复制 Slate Widget / UObject / 完整 EditorViewportClient |
| Unity | `EditorWindow` + `SceneView` / `GameView` + Camera / Render Pipeline | SceneView 是编辑器窗口里的世界视图，带 camera state、overlay、repaint | 学 Scene/Game 视图体验，不复制 IMGUI / UITK 内部历史结构 |
| Bevy | `WindowPlugin` + `WinitPlugin` + `RawHandleWrapper` + `WindowSurfaces` / wgpu renderer | Window / raw handle / surface / renderer 分层简单 | 学 winit/wgpu 分层和 headless runner 思路，不把 Window 直接变成编辑器业务真相 |
| Godot | Editor main window + viewport dock + editor camera / gizmo | Viewport 是编辑器面板中的编辑对象 | 学面板内 viewport，不采用 Godot 节点编辑器路线 |

本项目路线：

```text
UE-like Viewport 边界
+ Unity-like Scene View 体验方向
+ Bevy-like winit / surface / renderer 分层
+ AI-first headless report / schema / command / trace
```

## 3. 硬规则

### 3.1 所有能力必须 headless 可测

```text
C-min 中所有能力都必须能 headless 测试。
真实 OS window / real wgpu surface 不能成为唯一验证方式。
每个 real backend 都必须有 headless / mock / noop backend 等价入口。
自动化测试默认跑 headless backend。
real-window feature 只作为本机 smoke gate 和平台兼容 gate。
```

### 3.2 分层边界

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

### 3.3 真实窗口 feature

```text
默认测试和默认 build 不依赖真实 OS window。
真实 winit / wgpu surface 代码必须放在显式 feature 或独立 crate 边界后。
real-window smoke gate 可以失败但必须输出 RealWindowGateReport。
普通 headless CI 不因为 GUI 工具链缺失而失败。
```

## 4. 本轮边界

### 本轮做

```text
新增 C-min window / surface / viewport / renderer 的纯数据接口和 headless backend。
新增 HeadlessWindowBackend，模拟 create / resize / dpi / close / redraw。
新增 HeadlessSurfaceBackend，模拟 create / configure / acquire / present / surface lost。
新增 ViewportHost v1，支持一个 Scene Viewport rect、focus、output_kind、resize。
新增 EditorInputEvent v1，支持鼠标移动、点击、滚轮、键盘、resize、redraw、close 的纯数据表示。
新增输入路由：window event model -> EditorInputEvent -> UI / Viewport target。
新增 EditorUiRenderer C-min DrawList：固定 panel layout、viewport border、hit region。
新增 HeadlessRuntimeRenderer，输出 clear / test triangle / test texture 的 frame descriptor 和 frame hash。
新增 SceneCameraState v1，支持固定视角或最小 orbit / pan 占位数据。
新增 SelectionOutlineState v1，占位 draw-pass descriptor 或 report 字段。
新增 GizmoState v1，只支持 disabled / placeholder。
新增 RealWindowGateReport v1。
新增最小 headless 测试命令。
更新 49 / 54 / 18 / README / 00 入口状态。
```

### 本轮不做

```text
完整 docking。
多窗口。
真实 RDG。
真实 RHI / GPU Resource。
复杂材质。
完整 gizmo。
复杂鼠标拾取。
复杂场景编辑。
完整 Scene 工具体系。
Asset previewer。
平台原生菜单。
D3D12 / Vulkan / Metal native backend。
真实 Render Thread。
```

## 5. 建议代码落点

实际落点以当前代码结构为准。若目录不存在，按最小新增方式建立。

```text
src/editor/native-host/
  realWindowGateTypes.ts
  headlessWindowBackend.ts
  headlessSurfaceBackend.ts
  viewportHost.ts
  editorInputEvent.ts
  editorInputRouter.ts
  editorUiRendererCmin.ts
  headlessRuntimeRenderer.ts
  realWindowGateReport.ts

scripts/
  test-native-editor-real-window-cmin.cjs
```

如果当前 Rust native editor crate 已具备对应模块，应优先在 Rust crate 内实现；如果当前自动化链路仍以 TypeScript/Node 测试驱动，则先实现纯数据 TypeScript gate，但不能把 TypeScript runtime 扩展成第二套正式引擎。

## 6. 数据结构要求

### 6.1 EditorWindowBackend

```text
WindowConfig:
  title
  width
  height
  dpi_scale

WindowState:
  created
  size
  dpi_scale
  close_requested
  redraw_requested
```

HeadlessWindowBackend 必须支持：

```text
create_window(config)
resize(width, height, dpi_scale)
request_redraw()
close()
snapshot()
```

### 6.2 SurfaceBackend

```text
SurfaceState:
  created
  configured
  format
  present_mode
  width
  height
  acquired_frame
  presented_frame
  last_error
```

HeadlessSurfaceBackend 必须支持：

```text
create_surface()
configure(width, height, format, present_mode)
acquire()
present()
lose_surface(error)
snapshot()
```

### 6.3 ViewportHost

```text
ViewportState:
  viewport_id
  kind: scene | game
  rect
  focused
  output_kind
  camera_state
  selection_outline_state
  gizmo_state
```

只允许一个 Scene Viewport。多 viewport 和多窗口不在本轮。

### 6.4 EditorInputEvent

```text
EditorInputEvent:
  event_id
  kind
  position
  button
  key
  delta
  modifiers
  target_hint
```

本轮只需要支持：

```text
mouse_move
mouse_down
mouse_up
mouse_wheel
key_down
key_up
resize
redraw
close_requested
```

### 6.5 DrawList / HitRegion

```text
UiDrawList:
  frame_id
  commands[]
  hit_regions[]

DrawCommand:
  rect
  color
  text
  border

HitRegion:
  id
  rect
  target_kind: ui | viewport
  command_hint
```

### 6.6 HeadlessRuntimeRenderer

```text
RuntimeFrameDescriptor:
  frame_id
  viewport_id
  output_kind
  clear_color
  test_geometry_kind
  camera_state
  selection_outline_state
  gizmo_state
  frame_hash
```

### 6.7 RealWindowGateReport

```text
RealWindowGateReport:
  window
  surface
  gpu
  frame
  viewport
  input
  draw_list
  runtime_frame
  diagnostics
```

Report 必须可 JSON 序列化，字段必须稳定，供 AI 和人类查错。

## 7. 施工步骤

### A0：建立 C-min 类型层

任务：

```text
新增 realWindowGateTypes。
定义 WindowConfig / WindowState / SurfaceState / ViewportState / EditorInputEvent / UiDrawList / RuntimeFrameDescriptor / RealWindowGateReport。
所有类型保持纯数据。
```

测试：

```text
构造完整 C-min state。
JSON stringify / parse 后保持关键字段。
缺省字段有稳定默认值。
```

### A1：HeadlessWindowBackend / HeadlessSurfaceBackend

任务：

```text
实现 headless window backend。
实现 headless surface backend。
模拟 resize / dpi / close / redraw / acquire / present / surface lost。
```

测试：

```text
create -> resize -> request_redraw -> close。
create surface -> configure -> acquire -> present。
surface lost 能写入 error 并进入 report。
```

### A2：ViewportHost v1

任务：

```text
实现 register scene viewport。
实现 resize viewport。
实现 focus viewport。
保存 camera / selection / gizmo 占位状态。
```

测试：

```text
注册一个 Scene Viewport。
resize 后 rect 更新。
focus 切换后 focused 正确。
禁止注册第二个 Scene Viewport。
```

### A3：EditorInputEvent / Router

任务：

```text
定义 EditorInputEvent。
实现 route_input。
根据 hit region 判断 UI target 或 viewport target。
resize / redraw / close 进入 window target。
```

测试：

```text
点击 toolbar hit region -> ui command hint。
点击 viewport rect -> viewport input。
resize event -> window resize。
close_requested -> window close。
```

### A4：EditorUiRenderer C-min

任务：

```text
根据固定 layout 生成 UiDrawList。
包含 Toolbar / Hierarchy / Inspector / Console / Scene Viewport border。
生成 hit regions。
不直接调用 RuntimeRenderer。
```

测试：

```text
draw list 包含固定 panel shell。
viewport border rect 与 ViewportHost rect 一致。
hit regions 不重叠或按明确顺序命中。
```

### A5：HeadlessRuntimeRenderer

任务：

```text
根据 ViewportState 输出 RuntimeFrameDescriptor。
支持 clear / test triangle / test texture 三种 output_kind。
输出稳定 frame_hash。
不读取 Editor UI 状态。
```

测试：

```text
同样输入得到同样 frame_hash。
camera_state 改变时 frame_hash 改变。
selection_outline_state / gizmo_state 能进入 descriptor。
```

### A6：RealWindowGateReport v1

任务：

```text
聚合 window / surface / viewport / input / draw_list / runtime_frame / diagnostics。
所有异常以 diagnostics 表达。
```

测试：

```text
正常 headless frame 输出 report。
surface lost 输出 error report。
viewport 缺失输出 error report。
report 可 JSON 序列化。
```

### A7：端到端 headless C-min scenario

任务：

```text
构造一帧 C-min editor scenario：
create window
create/configure surface
register scene viewport
render UI draw list
route click to viewport
render runtime test frame
present surface
produce RealWindowGateReport
```

测试：

```text
scenario 通过。
report.window.created = true。
report.surface.presented_frame > 0。
report.viewport.focused = true。
report.runtime_frame.frame_hash 稳定。
diagnostics 无 error。
```

### A8：package / cargo 测试入口

任务：

```text
新增最小测试命令。
命令名建议：test:native-editor-cmin。
如果 Rust crate 已接入，则增加对应 cargo test。
```

测试：

```text
npm run test:native-editor-cmin
npm run build
```

如有 Rust：

```text
cargo test -p <native-editor-crate> cmin
```

## 8. 真实窗口 feature 边界

本轮可以只准备接口，不强制打开真实窗口。

允许：

```text
新增 real-window feature skeleton。
新增 WinitWindowBackend 类型壳。
新增 WgpuSurfaceBackend 类型壳。
新增 smoke gate 命令占位。
```

不要求：

```text
默认 CI 运行真实窗口。
默认 build 编译真实 winit / wgpu。
自动化测试依赖 GPU。
```

如果真实窗口在本机已经具备条件，可以额外实现：

```text
显式 feature 打开 winit window。
绘制 clear color。
输出 RealWindowGateReport。
```

但这不能替代 A0-A8 headless 测试。

## 9. 验收标准

必须全部满足：

```text
施工文档仍在 施工文档/当前，未完成前不移动。
所有新增能力有 headless 测试。
不新增必须依赖真实 OS window 的默认测试。
不新增必须依赖真实 GPU 的默认测试。
ViewportHost 不保存项目真相。
EditorUiRenderer 不画 3D world。
RuntimeRenderer 不画编辑器面板。
RealWindowGateReport 可 JSON 序列化。
npm run test:native-editor-cmin 通过。
npm run build 通过。
```

## 10. 完成后文档同步

施工完成后必须：

```text
把本文档移动到 施工文档/已完成。
新增 阶段完成记录/2026-xx-xx-Native-Editor-RealWindow-ViewportSurface-C-min/00-总览.md。
更新 49-当前状态与下一步入口.md。
更新 54-当前可自动化施工总入口.md。
更新 18-长期主义实现路径.md。
更新 README.md。
更新 00-阅读顺序.md。
更新 施工文档/README.md。
如果施工中产生长期规则，回填 37 / 47 / 17 / 50 等对应框架文档。
```

## 11. 风险控制

```text
如果真实 winit / wgpu 工具链失败，不阻塞 headless C-min。
如果当前代码结构不适合 Rust 直接落地，可以先做纯数据 TypeScript gate，但必须保持 TypeScript runtime 冻结原则。
如果某个能力需要复杂 UI / Renderer 才能实现，降级为 descriptor / report 占位，不扩展本轮范围。
如果新增模块和旧 Electron / React shell 有冲突，优先保持新模块纯数据，不强接 UI。
```

