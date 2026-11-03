# 112-当前可自动化施工文档：Native Editor Text Rendering C-min

## 目标

按照 `112-Native-Editor-Text-Rendering-C-min方案.md`，让 Native Editor real-window 第一版能显示基础 ASCII 文本。

施工目标：

```text
DrawCommand::Text
  -> BuiltinDebugFont
  -> rect glyphs
  -> existing WGPU rect pipeline
```

## 边界

必须做：

```text
在 editor_wgpu_renderer 中新增 BuiltinDebugFont。
UiGpuDrawPlan 支持 Text 转 drawable rects。
RealUiPresentReport 增加 text/glyph 统计。
Headless tests 覆盖 ASCII text、unsupported glyph、too-small rect。
real-wgpu feature tests 覆盖 text 生成 vertices。
```

不做：

```text
真实字体文件。
字体 atlas。
中文渲染。
复杂排版。
文本换行。
富文本。
clipping。
```

## 施工步骤

### 1. 数据结构

修改：

```text
rust/crates/editor_wgpu_renderer/src/lib.rs
```

新增：

```text
BuiltinDebugFont
GlyphBitmap
TextRenderStats
```

扩展：

```text
UiGpuDrawableRectSource::TextGlyph
UiGpuDrawPlan.text_command_count
UiGpuDrawPlan.rendered_glyph_count
UiGpuDrawPlan.unsupported_glyph_count
RealUiPresentReport.text_command_count
RealUiPresentReport.rendered_glyph_count
RealUiPresentReport.unsupported_glyph_count
```

### 2. 字符绘制规则

C-min 规则：

```text
只支持 ASCII 32..126。
每个 glyph 使用 5x7 bitmap。
字符宽度 = size * 0.6。
字符高度 = size。
像素块大小根据 Text rect 和 size 缩放。
超出 rect 宽度时停止绘制，不换行。
不支持字符用 '?' fallback，且 unsupported_glyph_count += 1。
```

### 3. 测试

新增 / 修改测试：

```text
draw_plan_renders_ascii_text_as_glyph_rects
draw_plan_falls_back_for_unsupported_text
draw_plan_skips_text_when_rect_too_small
headless_renderer_reports_rendered_glyphs
vertices_from_plan_includes_text_glyph_rects
```

### 4. 验证命令

运行：

```powershell
cargo fmt
cargo test -p editor_wgpu_renderer
cargo test -p editor_wgpu_renderer --features real-wgpu
cargo test -p editor_window_winit
cargo check -p editor_host --features real-window
```

可选手动 smoke：

```powershell
cargo run -p editor_host --features real-window -- --real-window
```

## 完成记录

完成后：

```text
移动本文档到 施工文档/已完成/。
新增 阶段完成记录/2026-06-30-Native-Editor-Text-Rendering-C-min/00-总览.md。
更新 00-文档地图.md / README.md / 111 文档。
```

