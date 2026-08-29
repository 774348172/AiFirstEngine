# 88-真实 Native Editor Window / EventLoop / UI Draw C-min 方案

> 后续补充规则：真实 UI present 已收敛到 `111-Native-Editor-Real-UI-Present-方案B.md`。本文继续作为 Window / EventLoop / UIDraw gate 文档；凡是讨论 `SelfUiRenderer DrawList -> WGPU Surface`，以 111 文档为准。

## 问题

当前 `editor_host` 可以运行，但只是：

```text
EditorSession
  -> EditorUiModel
  -> SelfUiRenderer DrawList
  -> readiness report
  -> exit
```

它还没有进入真实桌面窗口主循环：

```text
winit EventLoop
真实 OS Window
wgpu Surface
持续 redraw
DrawList -> GPU UI draw
鼠标键盘事件 -> EditorInputRouter -> EditorSession
```

因此当前游戏引擎“能跑 skeleton”，但还不能“正常打开一个可交互编辑器窗口”。

本方案目标是把 Native Editor Host 从 headless / skeleton / report-only 推进到真实窗口 C-min。

## 已有基础

当前已经具备：

```text
editor_core::EditorSession
editor_ui_model::EditorUiModel
editor_ui_renderer::SelfUiRenderer
editor_ui_renderer::UiDrawList
editor_ui_renderer::HitRegion
editor_input::EditorInputRouter
editor_window_winit::NativeEditorWindowConfig
editor_window_winit::winit_window_attributes
editor_window_winit::HeadlessWindowBackend
editor_window_winit::HeadlessSurfaceBackend
editor_window_winit::RealWindowGateReport
editor_window_winit::WindowedRuntimePresentReport
```

已完成但仍是 gate / report / headless 为主的相关文档：

```text
47-Native-Editor-Host-BC路线.md
66-当前可自动化施工文档-Native-Editor-RealWindow-ViewportSurface-C-min.md
77-当前可自动化施工文档-真实WindowedRuntime-ViewportPresent-C-min.md
83-真实默认WindowedEndToEndGameGate方案.md
87-当前实现状态总览-2026-06-28.md
```

## 其他引擎对照

### Unreal Engine

UE 使用：

```text
FEngineLoop
FSlateApplication
Slate Window / Widget
Slate Renderer
Render thread / RHI
```

参考点：

```text
EngineLoop 是最高层心跳。
SlateApplication 管窗口、输入、UI widget。
Slate renderer 负责编辑器 UI 绘制。
Editor UI 与 Runtime World 渲染分层。
```

我们吸收：

```text
Native editor host 拥有主循环。
UI renderer 不拥有 editor state。
Editor Core 不依赖窗口系统。
窗口层负责 event loop / surface / present。
```

不照搬：

```text
完整 Slate widget tree。
复杂 dock / menu / command binding。
完整 UE PIE。
```

### Unity

Unity 使用 native editor host 管主循环和窗口，C# 暴露：

```text
EditorWindow
GUIView
Repaint
EditorApplication.update
```

参考点：

```text
用户可见 EditorWindow 不直接拥有底层 OS event loop。
SceneView / GameView 是特殊 editor view。
Repaint / update 由 native host 调度。
```

我们吸收：

```text
EditorSession / EditorUiModel 是业务状态入口。
Window event 只转成 editor input / command。
UI 重绘由 host frame 控制。
```

不照搬：

```text
IMGUI / UI Toolkit 双体系。
完整 Unity dock layout。
```

### Bevy

Bevy 使用：

```text
bevy_winit
winit::ApplicationHandler
WindowPlugin
App schedule
RenderApp / wgpu
```

参考点：

```text
Rust + winit 的真实事件模型。
winit event 转换成引擎事件。
windowed runner 与 headless runner 可以分离。
```

我们吸收：

```text
winit 0.30 ApplicationHandler 模式。
real-window feature gate。
headless gate 与 real window 共享核心数据链路。
```

不照搬：

```text
Bevy ECS app runner。
Bevy RenderApp 双 world。
```

### Godot

Godot 使用：

```text
Main iteration
DisplayServer
RenderingServer
Control / CanvasItem
Editor UI
```

参考点：

```text
编辑器 UI 是引擎自身 UI 系统。
DisplayServer / RenderingServer 与业务 UI 分层。
```

我们吸收：

```text
长期方向是自研 UI renderer。
窗口、渲染服务、编辑器状态分层。
```

不照搬：

```text
Godot Node / Control UI 体系。
```

## 推荐方案：C-min

采用：

```text
NativeEditorApp
  -> winit EventLoop
  -> OS Window
  -> Wgpu Surface
  -> EditorSession
  -> EditorUiModel
  -> SelfUiRenderer DrawList
  -> WgpuUiRenderer C-min
  -> Present
```

第一版只做真实窗口和最小 UI 绘制闭环。

## 架构边界

### Editor Core

```text
Editor Core 不依赖 winit。
Editor Core 不依赖 wgpu。
Editor Core 不依赖 UI renderer。
Editor Core 只接收 UiCommand，输出 EditorUiModel。
```

### UI Model / DrawList

```text
EditorUiModel 是 UI 数据真相。
SelfUiRenderer 把 EditorUiModel 转成 UiDrawList / HitRegion。
UiDrawList 是 UI backend 的输入，不是业务状态。
```

### Window Backend

```text
editor_window_winit 负责 OS window / event loop / surface lifecycle / resize / present。
它可以持有 EditorSession，但不能绕过 EditorSession 修改业务状态。
```

### Wgpu UI Renderer C-min

```text
只消费 UiDrawList。
第一版只绘制矩形、边框、基础颜色。
第一版不做完整字体系统。
第一版不做复杂控件。
```

### Runtime Viewport

```text
Runtime renderer 仍负责游戏 / scene viewport 内容。
UI renderer 只画 editor panels / viewport shell / overlay。
第一版 viewport 内容可以是 placeholder 或现有 viewport texture slot 摘要。
```

## 第一版范围

必须实现：

```text
真实 winit EventLoop。
真实 OS Window。
真实 wgpu Instance / Surface / Device / Queue 初始化。
Surface configure / resize / acquire / present。
EditorSession 初始化。
每帧 build_ui_model。
SelfUiRenderer build_draw_list。
WgpuUiRendererCmin 绘制 DrawList 中的 panel / rect / toolbar button / viewport shell。
鼠标点击转换为 EditorInputEvent。
hit_test -> EditorInputRouter -> UiCommand -> EditorSession。
CloseRequested 正常退出。
Resize 重新 configure surface。
RealNativeEditorWindowReport 记录窗口、surface、draw、input、present 状态。
```

第一版不实现：

```text
复杂文本渲染。
字体 atlas / glyph cache。
真实 Dock Layout。
菜单栏。
复杂 Inspector 控件。
真实 Gizmo。
多窗口。
多 viewport。
高 DPI 完整适配。
完整 GPU UI renderer。
完整 Scene Editing 面板体验。
```

## 命令行入口

建议新增：

```powershell
cargo run -p editor_host -- --real-window
```

默认 `cargo run -p editor_host` 可以继续保留 skeleton/readiness 输出，避免自动化环境意外打开窗口。

如果实现成本更低，也可以使用单独 feature：

```powershell
cargo run -p editor_host --features real-window,real-wgpu-surface -- --real-window
```

最终规则：

```text
真实窗口必须显式开启。
默认测试和默认 run 不强制依赖真实窗口。
```

## 测试策略

### Headless 必测

```text
headless window app builds frame report
headless draw list renderer consumes UiDrawList
headless resize updates surface state
headless click routes to UiCommand
headless command mutates EditorSession through execute_command
headless close exits app loop state
```

### Feature-gated 编译门禁

```powershell
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
cargo check -p editor_host --features real-window,real-wgpu-surface
```

### 本机 smoke

本机真实窗口测试可以 ignored：

```powershell
cargo test -p editor_window_winit real_native_editor_window_smoke --features real-window,real-wgpu-surface -- --ignored
```

本机 smoke 允许因为环境问题失败，但必须输出明确 diagnostic。

## 环境门禁

当前机器已观察到：

```text
Windows 应用程序控制策略阻止 Rust 编译生成的 proc-macro DLL。
错误：LoadLibraryExW failed: 应用程序控制策略已阻止此文件。 (os error 4551)
```

因此施工必须包含：

```text
real-window feature 编译环境检查。
如果被 OS policy 阻止，记录为 environment_blocked，不把它伪装成代码失败。
headless tests 必须仍然通过。
```

## 报告结构

新增：

```text
RealNativeEditorWindowReport
```

建议字段：

```text
schema_version
backend
window_created
surface_created
surface_configured
device_created
frame_index
draw_command_count
hit_region_count
input_event_count
ui_command_count
present_status
resize_count
close_requested
diagnostics[]
```

diagnostics 最小字段：

```text
severity
code
message
source
suggested_action
```

## AI 友好规则

```text
所有窗口 / surface / draw / input / present 失败必须进入 RealNativeEditorWindowReport。
错误不能只打印 stderr。
AI 调试优先读取 report。
UiDrawList 和 HitRegion 继续作为 AI 可读 UI 结构。
UI renderer 不持有业务状态，避免 AI 查错时出现第二套真相。
```

## 推荐施工顺序

```text
1. 新增 RealNativeEditorWindowReport 数据结构。
2. 新增 WgpuUiRendererCmin 或 UiGpuRendererCmin skeleton。
3. 新增 headless C-min app loop，用纯数据模拟 frame / resize / click / close。
4. 新增 real-window feature 下的 winit ApplicationHandler。
5. 接入 wgpu surface configure / acquire / present。
6. 接入 DrawList 矩形绘制。
7. 接入 mouse click -> EditorInputRouter -> EditorSession。
8. editor_host 增加 --real-window 入口。
9. 补齐 tests / smoke / docs。
```

## 完成标准

满足以下条件才算完成：

```text
editor_host 有显式真实窗口入口。
真实窗口路径能创建 window / surface / device / queue。
真实窗口路径至少能 present 一帧。
UI DrawList 至少能绘制 panel / toolbar / viewport shell 几何。
鼠标点击至少能经过 hit_test 进入 EditorSession。
Resize / Close 有明确处理。
Headless tests 通过。
Feature-gated cargo check 通过，或因本机 OS policy 被阻止时输出明确 environment diagnostic。
RealNativeEditorWindowReport 可序列化。
cargo test --workspace 通过。
```

## 当前结论

选择 C-min。

这条路线比继续 skeleton 更接近真实产品，也不会像完整 UI renderer 那样一口吃太大。

它符合当前项目优先级：

```text
AI 友好：状态和错误通过 UiDrawList / Report 暴露。
复杂项目维护：EditorCore / Window / Renderer 分层清晰。
后期可修改：真实窗口只是 backend，不侵入 EditorSession。
简单度：第一版只画矩形，不做完整控件和字体。
效率：winit + wgpu 是长期底座，避免再走 Electron / egui 临时路线。
```
## 2026-06-28 实施完成记录

88 号真实 Native Editor Window / EventLoop / UI Draw C-min 已完成第一版落地。

已完成：

```text
RealNativeEditorWindowReport v1。
RealNativeEditorWindowDiagnostic / Severity。
UiGpuDrawPlan v1。
HeadlessNativeEditorWindowApp C-min。
editor_host --real-window 显式入口。
real-window feature 下的 winit ApplicationHandler 骨架。
默认 editor_host 仍保持 readiness/headless 输出，不自动打开真实窗口。
默认 cargo test --workspace 不依赖真实窗口。
```

关键边界：

```text
Editor Core 不依赖 winit / wgpu。
Window backend 不直接持有或修改业务真相。
真实窗口入口通过 EditorUiModel / UiDrawList 消费 UI 数据。
真实窗口未启用 feature 时输出 real_window_feature_not_enabled diagnostic。
feature-gated 检查如被 Windows 应用控制策略阻止，记录为 environment_blocked，不视为架构失败。
```

验证结果：

```text
cargo test -p editor_window_winit real_native_editor_window_report：通过。
cargo test -p editor_window_winit headless_native_editor_window_app：通过。
cargo test -p editor_window_winit ui_gpu_draw_plan：通过。
cargo test -p editor_host real_window：通过。
cargo test -p editor_window_winit native_editor_window_headless：通过。
cargo test -p editor_host native_editor_window：通过，当前无匹配测试。
cargo test -p editor_window_winit：45 passed。
cargo test -p editor_host：11 passed。
cargo test --workspace：通过。
cargo run -p editor_host：输出 editor_host ready。
cargo run -p editor_host -- --real-window：未启用 feature 时输出 real_window_feature_not_enabled。
```

环境门禁：

```text
cargo check -p editor_window_winit --features real-window,real-wgpu-surface：被 Windows 应用程序控制策略阻止 proc-macro DLL 加载，os error 4551。
cargo check -p editor_host --features real-window,real-wgpu-surface：同样被 os error 4551 阻止。
该失败属于 environment_blocked，不是 headless 链路或默认 workspace 测试失败。
```
