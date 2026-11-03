# 111-当前可自动化施工文档：Native Editor Real UI Present 方案B

## 目标

按照 `111-Native-Editor-Real-UI-Present-方案B.md`，把当前 Native Editor real-window 从“只创建窗口并统计 DrawList”推进到：

```text
SelfUiRenderer DrawList
  -> editor_wgpu_renderer
  -> WGPU Surface
  -> Present
```

本施工只做 C-min，不扩展完整 UI framework。

## 施工边界

必须做：

```text
新增 editor_wgpu_renderer crate。
迁出 UiGpuDrawPlan。
新增 RealUiPresentReport / HeadlessUiGpuRenderer。
editor_window_winit 使用 editor_wgpu_renderer。
真实窗口 RedrawRequested 不再伪造 presented。
Rect / ViewportTextureSlot placeholder 能形成真实 GPU draw path。
Text skipped 计数。
```

不做：

```text
真实字体渲染。
图片纹理。
真实 viewport texture sampling。
复杂 clipping / batching。
Dock / Panel 新功能。
Editor Core 新业务能力。
RuntimeRenderer 合并。
```

## 具体步骤

### 1. 新增 crate

新增：

```text
rust/crates/editor_wgpu_renderer
```

更新：

```text
rust/Cargo.toml
```

依赖：

```text
editor_ui_renderer
serde
wgpu optional
bytemuck optional
pollster optional
```

feature：

```text
real-wgpu = ["dep:wgpu", "dep:bytemuck", "dep:pollster"]
```

### 2. 数据结构

新增：

```text
UiGpuDrawPlan
UiGpuDrawableRect
RealUiPresentReport
RealUiPresentDiagnostic
HeadlessUiGpuRenderer
```

规则：

```text
UiGpuDrawPlan 只从 UiDrawList 派生。
RealUiPresentReport 只记录 present 层结果。
```

### 3. Headless 测试

测试：

```text
draw_plan_counts_rect_text_viewport
draw_plan_rejects_empty_surface
headless_renderer_reports_skipped_text
headless_renderer_treats_viewport_slot_as_placeholder_rect
report_is_serializable
```

### 4. 接入 editor_window_winit

修改：

```text
rust/crates/editor_window_winit/Cargo.toml
rust/crates/editor_window_winit/src/lib.rs
```

要求：

```text
删除本 crate 内 UiGpuDrawPlan 定义。
HeadlessNativeEditorWindowApp 使用 HeadlessUiGpuRenderer。
RealNativeEditorWindowReport 根据 RealUiPresentReport 更新 draw/present 字段。
real-window feature 透传 editor_wgpu_renderer/real-wgpu。
```

### 5. 真实 WGPU 路径

在 `editor_wgpu_renderer` 中实现：

```text
RealWgpuUiRenderer
```

C-min 能力：

```text
初始化 instance / surface / adapter / device / queue。
configure surface。
resize reconfigure。
present draw_list。
clear background。
绘制 Rect / ViewportTextureSlot placeholder。
Text skipped。
```

如果本机策略阻止真实窗口或 wgpu 编译，必须输出明确 diagnostic。

### 6. 验证命令

运行：

```powershell
cargo test -p editor_wgpu_renderer
cargo test -p editor_window_winit native_editor_window_headless
cargo test -p editor_host real_window
cargo test --workspace
```

可选 smoke：

```powershell
cargo check -p editor_window_winit --features real-window
cargo run -p editor_host --features real-window -- --real-window
```

## 完成记录要求

施工完成后：

```text
把本文档移动到 施工文档/已完成/。
新增 阶段完成记录/2026-06-30-Native-Editor-Real-UI-Present-方案B/00-总览.md。
更新 00-文档地图.md。
更新 47 / 88 文档中的当前规则引用。
```

