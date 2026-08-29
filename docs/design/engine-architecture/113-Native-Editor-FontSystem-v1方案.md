# 113-Native Editor FontSystem v1 方案

## 问题

`112-Native-Editor-Text-Rendering-C-min` 用内置 ASCII 点阵字体让编辑器可读，但视觉质量很差，不适合作为后续编辑器基础。

现在需要进入正式 FontSystem v1：

```text
DrawCommand::Text
  -> FontSystem
  -> Glyph Rasterizer
  -> Glyph Atlas
  -> WGPU textured text pipeline
```

## 其他引擎对比

### Unreal Engine

UE Slate 文本链路大致是：

```text
FSlateDrawElement::MakeText
  -> FSlateTextElement
  -> FSlateFontServices
  -> FSlateFontCache
  -> FSlateTextureAtlas
  -> SlateRenderer / RHI
```

本地源码参考：

```text
Engine/Source/Runtime/SlateCore/Public/Rendering/SlateRenderer.h
Engine/Source/Runtime/SlateCore/Public/Rendering/ElementBatcher.h
Engine/Source/Runtime/SlateCore/Public/Rendering/DrawElementTypes.h
Engine/Source/Runtime/SlateCore/Public/Fonts/FontCache.h
Engine/Source/Runtime/SlateCore/Private/Textures/TextureAtlas.cpp
```

可借鉴：

```text
Font services 独立于窗口。
Text draw element 不直接变成业务状态。
Glyph cache / atlas 由 renderer 侧管理。
```

不照搬：

```text
完整 FreeType / HarfBuzz shaping。
GameThread / RenderThread 双 FontCache。
SDF / MSDF。
复杂富文本。
```

### Unity

Unity 编辑器文本由 native editor backend / IMGUI / UI Toolkit 支撑；运行时高质量文本通常走 TextCore / TextMeshPro。

可借鉴：

```text
文本质量是编辑器可用性的基础。
字体资源、glyph cache、atlas 是渲染层基础设施。
```

不照搬：

```text
TMP 完整 SDF 管线。
UI Toolkit 全控件体系。
```

### Bevy

Bevy UI 文本常见链路是：

```text
Font Asset
  -> glyph extraction
  -> glyph atlas
  -> render pipeline
```

可借鉴：

```text
Rust 生态里字体 rasterizer / atlas / text pipeline 应拆层。
```

不照搬：

```text
Bevy ECS render extract。
完整 TextLayout pipeline。
```

### Godot

Godot 使用 Font / TextServer / RenderingServer 处理文字。

可借鉴：

```text
复杂文字处理应独立成 TextServer / FontSystem。
```

不照搬：

```text
完整 TextServer 和国际化 shaping。
```

## 方案选择

采用 FontSystem v1-min：

```text
editor_wgpu_renderer
  -> EditorFontSystem
      -> load system font bytes
      -> ab_glyph rasterize glyph
      -> CpuGlyphAtlas
      -> WgpuFontAtlas texture
      -> TextVertex textured quads
```

第一版使用 `ab_glyph`：

```text
优点：轻、Rust 生态成熟、能直接 rasterize TTF/OTF。
缺点：不做复杂 shaping，不是完整 CJK/emoji 文本方案。
```

## 边界

负责：

```text
Editor FontSystem v1 负责 Native Editor UI 的基础文字质量。
editor_wgpu_renderer 持有 GPU font atlas。
DrawCommand::Text 仍是外部稳定输入。
```

不负责：

```text
项目运行时 UI 字体资产系统。
AUI 全量字体系统。
复杂 shaping。
中文 fallback。
字体导入器。
富文本。
输入法。
文本编辑器。
```

## 字体选择

第一版按平台查找系统字体：

```text
Windows:
  <WINDOWS_FONTS>\segoeui.ttf
  <WINDOWS_FONTS>\arial.ttf

fallback:
  如果系统字体不可用，回退到 112 的 BuiltinDebugFont。
```

规则：

```text
字体路径只作为 editor host 本机工具链输入。
不把系统字体打进项目包。
不让项目逻辑依赖系统字体。
```

## 数据结构

```text
EditorFontSystem
  font_source
  glyph_cache
  cpu_atlas

GlyphKey
  char
  px_size

GlyphAtlasEntry
  uv_rect
  pixel_rect
  advance
  bearing

TextGpuDrawPlan
  glyph_quads[]
  missing_glyph_count
  atlas_revision
```

Report 新增：

```text
font_backend
font_loaded
font_source
glyph_atlas_width
glyph_atlas_height
glyph_cache_count
missing_glyph_count
text_pipeline
```

## 第一版限制

```text
只做单字体。
只做单 atlas page。
只做横排 LTR。
只做 ASCII / Latin 基础可读。
中文和 emoji fallback 为 '?'。
不做换行。
不做 kerning。
不做 subpixel positioning。
不做 atlas eviction。
atlas 满时 report error，后续再扩展 multi-page。
```

## 为什么适合我们

AI 友好：

```text
Text 仍然来自 DrawCommand::Text。
Report 能说明字体是否加载、glyph 是否 missing、atlas 是否满。
AI 可以通过结构化 report 判断为什么文字不好看或没显示。
```

复杂项目可维护：

```text
FontSystem 是独立基础设施，不污染 Editor Core。
后续 AUI / Runtime UI 可复用同类 FontSystem 思路。
```

简单：

```text
不直接上 glyphon / cosmic-text / HarfBuzz。
不做多语言复杂排版。
先把编辑器文字质量拉到可用。
```

效率：

```text
比 5x7 rect glyph 少大量顶点。
使用 atlas texture sampling，接近成熟引擎第一层真实路径。
```

## 完成标准

```text
Native Editor Toolbar / Panel title / Inspector / Console 文本明显改善。
cargo test -p editor_wgpu_renderer 通过。
cargo test -p editor_wgpu_renderer --features real-wgpu 通过。
cargo test -p editor_window_winit 通过。
cargo check -p editor_host --features real-window 通过。
```

