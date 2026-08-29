# 112-Native Editor Text Rendering C-min 方案

> 状态：已被 `113-Native-Editor-FontSystem-v1方案.md` 取代为正式文本渲染路径。本文档中的内置 ASCII 点阵字体只保留为系统字体加载失败时的 fallback，不再作为 Native Editor 文本质量的长期方向。

## 问题

当前 Native Editor real-window 已经能把 `UiDrawList` 画到 WGPU Surface，但 `DrawCommand::Text` 仍然被计入 `skipped_text_count`，不参与绘制。

结果是：

```text
窗口可见
panel / viewport placeholder 可见
按钮和面板几何可见
文字不可见
```

这会阻碍下一步编辑器最小闭环验证，因为人和 AI 都无法从界面直接判断 Toolbar、Hierarchy、Inspector、Console 的状态。

## 其他引擎对比

### Unreal Engine

UE Slate 使用 Slate Font Cache / Font Atlas / Shaped Text / Slate DrawElements。

可借鉴：

```text
Text 是 UI renderer 的基础能力，不应该由窗口层直接画。
文本最终仍然变成 draw elements / atlas / GPU draw。
```

不照搬：

```text
完整字体缓存。
复杂 shaping。
多语言 fallback。
富文本。
```

### Unity

Unity 编辑器文本由 IMGUI / UI Toolkit / native backend 处理，运行时常见 Text / TextMeshPro。

可借鉴：

```text
编辑器第一可用性强依赖文本。
UI 控件体系不应该直接暴露底层 glyph 绘制细节。
```

不照搬：

```text
IMGUI 文本布局历史模式。
TextMeshPro SDF 完整管线。
```

### Bevy

Bevy UI 文本通常走 font asset、glyph atlas、text pipeline。

可借鉴：

```text
长期应走字体资源 + atlas + text pipeline。
```

不照搬：

```text
第一版不引入完整 asset/font pipeline。
```

### Godot

Godot Control / CanvasItem 文本最终由 Font / TextServer / RenderingServer 绘制。

可借鉴：

```text
文本是 UI renderer 能力，不是项目逻辑能力。
复杂文字处理属于独立 TextServer / Font System。
```

不照搬：

```text
完整 TextServer。
国际化 shaping。
```

## 方案选择

采用 C-min：

```text
DrawCommand::Text
  -> BuiltinDebugFont
  -> small rect glyphs
  -> merge into UiGpuDrawPlan drawable rects
  -> existing WGPU rectangle pipeline
```

也就是：第一版不做真实字体 atlas，而是用内置 ASCII 5x7 bitmap 字体，把每个字符拆成小矩形绘制。

## 为什么适合我们

AI 友好：

```text
Text 仍然来自 DrawCommand::Text。
Report 能说明 text_command_count / rendered_glyph_count / unsupported_glyph_count。
AI 不需要理解字体资源管线，也能判断文字是否被绘制。
```

复杂项目可维护：

```text
BuiltinDebugFont 只作为 Editor C-min 可读性门禁。
后续真实 Font System 可以替换 glyph provider，不推翻 DrawCommand::Text。
```

简单：

```text
不引入 fontdue / cosmic-text / rustybuzz。
不引入字体资产。
不引入 atlas packing。
不新增 shader。
复用现有 rect pipeline。
```

效率：

```text
C-min 文本由大量小矩形组成，只适合编辑器早期调试。
它不是最终高性能文本方案。
```

## 边界规则

负责：

```text
editor_wgpu_renderer 负责把 DrawCommand::Text 转成可绘制几何。
Text C-min 只支持 ASCII 32..126。
不支持字符时绘制 '?' 或计入 unsupported_glyph_count。
```

禁止：

```text
窗口层直接绘制文字。
Editor Core 关心 glyph / font atlas。
为了 C-min 添加完整 Font Asset / Font Importer。
为了 C-min 添加复杂文本布局规则。
```

## 第一版字段

`UiGpuDrawPlan` 新增：

```text
text_command_count
rendered_glyph_count
unsupported_glyph_count
```

`RealUiPresentReport` 新增：

```text
text_command_count
rendered_glyph_count
unsupported_glyph_count
```

保留：

```text
skipped_text_count
```

规则：

```text
当 Text C-min 启用后，ASCII 文本不再计入 skipped_text_count。
不支持字符计入 unsupported_glyph_count。
如果整条 Text 因 rect 太小无法绘制，计入 skipped_text_count。
```

## 完成标准

```text
Native Editor 窗口能看到 Toolbar / Panel title / Console 的 ASCII 文本。
cargo test -p editor_wgpu_renderer 通过。
cargo test -p editor_wgpu_renderer --features real-wgpu 通过。
cargo test -p editor_window_winit 通过。
cargo check -p editor_host --features real-window 通过。
```

## 后续真实 Font System

后续如果进入正式 Font System，应新开方案，不在本 C-min 内扩展：

```text
Font Asset / Importer
Glyph Atlas
Text shaping
Fallback font
Unicode / CJK
SDF / MSDF
Text clipping / wrapping
```
