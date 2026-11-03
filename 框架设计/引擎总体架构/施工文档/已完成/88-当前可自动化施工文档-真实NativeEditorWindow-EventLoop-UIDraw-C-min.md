# 88-当前可自动化施工文档：真实 Native Editor Window / EventLoop / UI Draw C-min

## 目标

按 `88-真实NativeEditorWindow-EventLoop-UIDraw-C-min方案.md` 实现第一版真实 Native Editor Window。

本轮目标不是做完整编辑器 UI，而是让 `editor_host` 从 skeleton/readiness report 进入真实窗口 C-min：

```text
editor_host --real-window
  -> winit EventLoop
  -> OS Window
  -> wgpu Surface
  -> EditorSession
  -> EditorUiModel
  -> SelfUiRenderer DrawList
  -> WgpuUiRendererCmin
  -> Present
```

## 当前已有基础

必须复用：

```text
rust/crates/editor_host/src/main.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_window_winit/src/windowed_runtime_present.rs
```

不要重写：

```text
EditorSession
EditorUiModel
UiDrawList
EditorInputRouter
HeadlessWindowBackend
HeadlessSurfaceBackend
WindowedRuntimePresentReport
```

## 本轮边界

必须实现：

```text
RealNativeEditorWindowReport。
HeadlessNativeEditorWindowApp C-min。
WgpuUiRendererCmin 或等价最小 UI GPU renderer。
real-window feature 下的 winit ApplicationHandler。
wgpu Instance / Surface / Device / Queue 初始化。
surface configure / resize / acquire / present。
DrawList 中 panel / rect / toolbar / viewport shell 的最小绘制。
鼠标点击 -> EditorInputEvent -> hit_test -> EditorInputRouter -> UiCommand -> EditorSession。
CloseRequested 正常退出。
editor_host --real-window 显式入口。
```

明确不做：

```text
完整字体系统。
复杂文本渲染。
Dock layout。
菜单栏。
复杂 Inspector 输入。
真实 Gizmo。
多窗口。
多 viewport。
完整 Scene Editing 面板体验。
完整 GPU UI renderer。
```

## 施工原则

```text
Editor Core 不依赖 winit / wgpu。
Window backend 不直接修改业务状态。
UI renderer 只消费 UiDrawList。
UiDrawList 不成为业务状态真相。
所有真实窗口错误必须进入 report。
真实窗口必须显式开启，不进入默认 cargo test --workspace。
headless tests 必须覆盖同一条数据链路。
```

## A1：新增 RealNativeEditorWindowReport

修改优先级：

```text
rust/crates/editor_window_winit/src/lib.rs
```

新增结构：

```text
RealNativeEditorWindowReport
RealNativeEditorWindowDiagnostic
RealNativeEditorWindowDiagnosticSeverity
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
diagnostics
```

测试：

```text
real_native_editor_window_report_serializes
real_native_editor_window_report_marks_environment_blocked
```

运行：

```powershell
cargo test -p editor_window_winit real_native_editor_window_report
```

## A2：HeadlessNativeEditorWindowApp C-min

修改：

```text
rust/crates/editor_window_winit/src/lib.rs
```

实现纯数据 headless app loop：

```text
HeadlessNativeEditorWindowApp
  new(config)
  frame(&EditorUiModel)
  resize(width, height)
  click(x, y, draw_list)
  close()
  report()
```

规则：

```text
headless app 不创建真实窗口。
headless app 使用同一套 EditorUiModel / SelfUiRenderer / UiDrawList。
headless click 必须走 EditorInputRouter。
```

测试：

```text
headless_native_editor_window_app_builds_frame_report
headless_native_editor_window_app_resize_updates_surface_state
headless_native_editor_window_app_click_routes_to_ui_command
headless_native_editor_window_app_close_sets_report_state
```

运行：

```powershell
cargo test -p editor_window_winit headless_native_editor_window_app
```

## A3：WgpuUiRendererCmin 数据边界

修改：

```text
rust/crates/editor_window_winit/src/lib.rs
```

或新增：

```text
rust/crates/editor_window_winit/src/ui_gpu_renderer.rs
```

第一版 renderer 可以先分两层：

```text
UiGpuDrawPlan
  input: UiDrawList
  output: rect draw count / clear color / surface size

WgpuUiRendererCmin
  real-window feature 下消费 UiGpuDrawPlan 并提交到 surface
```

headless 测试先验证 draw plan，不依赖真实 GPU。

测试：

```text
ui_gpu_draw_plan_counts_panel_and_viewport_rects
ui_gpu_draw_plan_is_stable_for_same_ui_model
ui_gpu_draw_plan_rejects_empty_surface
```

运行：

```powershell
cargo test -p editor_window_winit ui_gpu_draw_plan
```

## A4：real-window feature 下接 winit ApplicationHandler

修改：

```text
rust/crates/editor_window_winit/src/lib.rs
```

或新增：

```text
rust/crates/editor_window_winit/src/real_native_editor_window.rs
```

实现：

```text
RealNativeEditorApp
  holds EditorSession
  holds EditorInputRouter
  holds optional wgpu state
  creates window on resumed
  handles WindowEvent::Resized
  handles WindowEvent::CloseRequested
  handles WindowEvent::MouseInput / CursorMoved
  handles RedrawRequested
```

规则：

```text
ApplicationHandler 只做窗口和事件。
业务修改必须通过 UiCommand -> EditorSession。
```

编译门禁：

```powershell
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

如果本机被 Windows 应用程序控制策略阻止 DLL：

```text
记录 environment_blocked。
不把它记为架构失败。
headless tests 仍必须通过。
```

## A5：wgpu Surface / Present C-min

修改：

```text
rust/crates/editor_window_winit/src/real_native_editor_window.rs
```

实现：

```text
create wgpu Instance
create Surface from Window
request Adapter
request Device / Queue
configure Surface
on resize reconfigure Surface
on redraw acquire current texture
clear background
draw minimal UI rects
present
```

第一版绘制：

```text
background clear
panel rect
toolbar button rect
viewport shell rect
```

不要求：

```text
text
anti-alias
rounded corners
icons
complex clipping
```

测试：

```text
real_window_feature_can_build_native_editor_app
```

运行：

```powershell
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

## A6：editor_host --real-window 入口

修改：

```text
rust/crates/editor_host/src/main.rs
rust/crates/editor_host/Cargo.toml
```

实现：

```text
默认 cargo run -p editor_host：
  保持当前 readiness report，不打开窗口。

cargo run -p editor_host -- --real-window：
  调用 real native editor window app。
```

如果 feature 未开启：

```text
输出明确 diagnostic：
real_window_feature_not_enabled
```

测试：

```text
editor_host_default_still_prints_readiness
editor_host_real_window_arg_reports_missing_feature_without_feature
```

运行：

```powershell
cargo test -p editor_host real_window
```

## A7：端到端 headless gate

新增测试：

```text
native_editor_window_headless_frame_draws_ui_model
native_editor_window_headless_click_toolbar_routes_command
native_editor_window_headless_resize_then_frame_presents_report
native_editor_window_headless_close_exits_cleanly
```

运行：

```powershell
cargo test -p editor_window_winit native_editor_window_headless
cargo test -p editor_host native_editor_window
```

## A8：本机真实窗口 smoke

新增 ignored smoke：

```text
real_native_editor_window_smoke
```

运行：

```powershell
cargo test -p editor_window_winit real_native_editor_window_smoke --features real-window,real-wgpu-surface -- --ignored
```

允许情况：

```text
成功打开窗口并 present 一帧。
或因为环境阻止返回 environment_blocked diagnostic。
```

不允许：

```text
panic。
无结构化错误。
阻塞默认 workspace 测试。
```

## A9：验证命令

施工完成必须运行：

```powershell
cargo test -p editor_window_winit real_native_editor_window_report
cargo test -p editor_window_winit headless_native_editor_window_app
cargo test -p editor_window_winit ui_gpu_draw_plan
cargo test -p editor_host real_window
cargo test -p editor_window_winit native_editor_window_headless
cargo test -p editor_host native_editor_window
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test --workspace
```

feature-gated 检查：

```powershell
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
cargo check -p editor_host --features real-window,real-wgpu-surface
```

如果 feature-gated 检查因 OS policy 失败，完成记录必须写清：

```text
失败原因。
是否为 environment_blocked。
headless tests 是否全部通过。
```

## A10：完成后文档同步

完成后必须：

```text
1. 将本文档移动到 施工文档/已完成。
2. 新增 阶段完成记录/2026-06-28-真实NativeEditorWindow-EventLoop-UIDraw-C-min/00-总览.md。
3. 更新 87-当前实现状态总览-2026-06-28.md。
4. 更新 49-当前状态与下一步入口.md。
5. 更新 54-当前可自动化施工总入口.md。
6. 更新 18-长期主义实现路径.md。
7. 更新 00-文档地图.md。
8. 更新 施工文档/README.md。
9. 更新 阶段完成记录/README.md。
10. 在 88-真实NativeEditorWindow-EventLoop-UIDraw-C-min方案.md 追加实施完成记录。
```

## 完成判定

满足以下条件才算完成：

```text
RealNativeEditorWindowReport 已实现并可序列化。
headless native editor window app 能跑 frame / resize / click / close。
UiDrawList 能生成 GPU draw plan。
real-window feature 下能编译真实窗口 app，或输出明确 environment_blocked。
editor_host 有 --real-window 入口。
默认 editor_host 行为不破坏。
真实窗口 smoke 不进入默认 workspace test。
headless tests 通过。
cargo test --workspace 通过。
```

## 明确不算完成的情况

```text
只新增方案文档，未实现代码。
只打开空窗口，没有接 EditorUiModel / DrawList。
UI renderer 直接持有业务状态。
窗口事件直接修改 EditorSession 内部字段，绕过 UiCommand。
真实窗口失败只 panic 或 stderr，没有 report diagnostic。
默认 cargo test --workspace 依赖真实窗口。
```

