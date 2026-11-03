# 113-当前可自动化施工文档：Native Editor FontSystem v1

## 目标

按照 `113-Native-Editor-FontSystem-v1方案.md`，替换 112 的点阵字视觉效果，完成第一版真实字体渲染：

```text
DrawCommand::Text
  -> ab_glyph rasterize
  -> CPU glyph atlas
  -> WGPU atlas texture
  -> textured glyph quads
```

## 施工范围

必须做：

```text
editor_wgpu_renderer 增加 ab_glyph 依赖。
实现 EditorFontSystem / CpuGlyphAtlas。
实现 TexturedTextVertex / text pipeline。
real-wgpu 路径使用 glyph atlas texture 绘制文本。
headless 路径能生成真实 glyph layout / report。
保留 BuiltinDebugFont 作为 fallback。
```

不做：

```text
中文 shaping。
emoji。
多字体 fallback。
字体 asset/importer。
换行。
富文本。
atlas eviction。
```

## 步骤

### 1. Cargo 依赖

修改：

```text
rust/crates/editor_wgpu_renderer/Cargo.toml
```

新增：

```text
ab_glyph = "0.2"
```

### 2. FontSystem 数据结构

新增：

```text
EditorFontSystem
EditorFontSource
GlyphKey
GlyphAtlasEntry
CpuGlyphAtlas
TextGlyphQuad
TextLayoutReport
```

### 3. Headless layout

测试：

```text
font_system_loads_system_font_or_reports_fallback
font_system_lays_out_ascii_text
font_system_reports_missing_glyph
font_system_atlas_reuses_cached_glyph
```

### 4. WGPU text pipeline

实现：

```text
glyph atlas texture upload
text bind group
text render pipeline
TexturedTextVertex
draw text quads after rect pass
```

### 5. fallback

规则：

```text
系统字体加载失败时，继续使用 112 BuiltinDebugFont。
report.font_backend = builtin_debug_font。
```

### 6. 验证

运行：

```powershell
cargo fmt
cargo test -p editor_wgpu_renderer
cargo test -p editor_wgpu_renderer --features real-wgpu
cargo test -p editor_window_winit
cargo check -p editor_host --features real-window
```

可选 smoke：

```powershell
cargo run -p editor_host --features real-window -- --real-window
```

## 完成记录

完成后：

```text
移动本文档到 施工文档/已完成/。
新增 阶段完成记录/2026-06-30-Native-Editor-FontSystem-v1/00-总览.md。
更新 README / 00-文档地图 / 112 文档。
```

