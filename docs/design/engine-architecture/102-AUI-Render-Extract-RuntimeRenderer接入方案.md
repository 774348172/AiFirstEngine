# 102-AUI Render Extract / RuntimeRenderer 接入方案

## 当前归属说明：UiProjection

本文档中的 `AuiRendererBridge / AUI Render Extract`，从 `110-World-Projection-Adapter统一跨域同步规则.md` 起统一归属为：

```text
UiProjection
```

正确链路：

```text
AUI Tree / Layout / DrawList
  -> UiProjectionAdapter
  -> UI Render Commands / AuiOverlayFrame
  -> RuntimeRenderer UI pass
```

`AuiRendererBridge` 是历史实现名，不再作为新增架构概念扩展。

## 1. 问题是什么

`101 AUI C-min` 已经实现：

```text
AuiDocument
  -> AuiLayoutEngine
  -> AuiDrawList
  -> AuiRenderReport
```

但它仍然是 headless 产物，还没有进入 RuntimeRenderer，也不能进入 RenderGraph / Viewport Present。

本系统解决：

```text
AuiDrawList 如何进入 RuntimeRenderer。
AUI 是否直接生成 RenderCommand。
AUI 是否变成 RenderProxy。
UI pass 在 RuntimeRenderer 中的位置。
AI 如何通过 AuiDrawList / Report 理解 UI 渲染结果。
```

## 2. 其他引擎怎么做

### 2.1 Unreal UMG / Slate

UE 路线：

```text
UMG Widget Tree
  -> Slate Widget
  -> Layout / Prepass
  -> OnPaint
  -> Slate Draw Elements
  -> Slate Renderer
  -> RHI
```

关键点：

```text
UI 不直接操作 RHI。
Widget 先生成 DrawElement。
Renderer 再消费 DrawElement。
```

我们学习：

```text
AuiDrawList 类似 Slate Draw Elements。
AuiRendererBridge 类似 Slate Renderer 前的转换层。
RuntimeRenderer 负责把 UI Draw Items 放入 render graph。
```

### 2.2 Unity UGUI

Unity 路线：

```text
Canvas / Graphic / Image / Text
  -> CanvasRenderer
  -> UI batch
  -> Overlay / Camera render
```

优点：

```text
Canvas / Image / Text 用户心智成熟。
Overlay UI 默认在世界渲染后。
```

风险：

```text
Canvas rebuild 容易黑箱。
复杂 layout / rebuild 性能不透明。
```

我们吸收 Overlay / Canvas 心智，不照搬 Canvas rebuild 黑箱。

补充规则见 `211-AUI-Prefab-Template-Reuse-Productization-v1方案.md`：复杂 AUI 控件在 authoring 层用 AUI Document subtree 组合表达；到本系统时只消费基础绘制结果 `DrawRect / DrawImage / DrawText`，不把 Button、装备格、商店项等复杂控件直接做成新的 RenderCommand 或 RenderProxy。

### 2.3 Bevy UI

Bevy 路线：

```text
UI Node / Style
  -> Layout
  -> Extract
  -> Render phase
```

我们吸收：

```text
数据驱动。
layout 和 render extract 分离。
headless 可测试。
```

### 2.4 Godot Control

Godot 路线：

```text
Control Tree
  -> CanvasItem draw
  -> RenderingServer
```

我们吸收：

```text
Control/UI 独立于世界 Sprite。
UI 有自己的 Canvas / Layer 规则。
```

## 3. 可选方案对比

### 方案 A：AUI 直接生成 RenderCommand

优点：

```text
路径短。
第一版实现快。
```

缺点：

```text
AUI 过早绑定底层 renderer。
AI 需要理解 RenderCommand 细节。
后期 Text / Clip / Mask / Batch 扩展时容易污染 RuntimeRenderer。
```

### 方案 B：AuiDrawList -> AuiRendererBridge -> RuntimeRenderer UI Draw Items

优点：

```text
最接近 UE Slate DrawElement 路线。
AUI Core 保持独立。
Renderer 只消费 UI draw items。
AI 可以读 AuiDrawList / AuiRenderReport，不需要读底层 RHI。
后续 Text atlas / Clip / Batch 可以在 bridge 或 UI pass 内扩展。
```

缺点：

```text
多一个 bridge 层。
第一版需要定义 UI draw item 和 UI pass。
```

### 方案 C：AUI 变成 RenderProxy

优点：

```text
复用 RenderSceneState / RenderProxy。
和现有世界渲染路径统一。
```

缺点：

```text
UI 和 World/Sprite 容易混在一起。
Canvas / Layer / HitTest / Text 语义会被 RenderProxy 稀释。
后期复杂 UI 维护风险高。
```

## 4. 推荐方案

选择方案 B：

```text
AuiDocument
  -> AuiLayoutEngine
  -> AuiDrawList
  -> AuiRendererBridge
  -> AuiOverlayFrame
  -> RuntimeRenderer UI Render Pass
  -> RenderGraph
```

## 5. 标准结构

```text
AuiOverlayFrame
  frame_index
  draw_items
  report

AuiOverlayDrawItem
  item_id
  node_id
  item_kind
  rect
  color
  asset_id
  text
  font_size
  sort_key

AuiOverlaySortKey
  canvas_layer
  canvas_sorting_order
  tree_order
```

第一版 `item_kind`：

```text
Rect
Image
Text
```

历史 C-min 文档中曾把第一版文本项称为 `TextPlaceholder`；在 `208-Runtime-Text-Glyph-Present-AUI-Text-Rendering-Productization-v1方案.md` 之后，应按 `Text + AuiTextGlyphPlan` 理解。

第一版 RuntimeRenderer 增加：

```text
RuntimeRendererInput.aui_overlay: Option<&AuiOverlayFrame>
RenderPassKind::DrawUiOverlay
RenderPassCommand::DrawUiOverlay
```

`DrawUiOverlay` 第一版只作为 headless / RenderGraph 级 UI pass 描述：

```text
target
item_count
text_count
image_count
debug_label
```

真实 GPU UI batching、字体 atlas、文本 shaping 后续再做。

## 6. Pass 顺序

第一版固定：

```text
Clear Main
World / Sprite draw passes
AUI Overlay pass
Present Main
```

规则：

```text
ScreenOverlay AUI 默认在 World / Sprite 之后。
AUI 不参与 Sprite2D sorting。
AUI 不进入 RenderProxy。
RuntimeRenderer 只接收 AuiOverlayFrame，不直接读取 AuiDocument。
```

## 7. AI 友好规则

AI 默认读：

```text
AuiDrawList
AuiOverlayFrame
AuiRenderReport
RuntimeRenderFrameReport
RenderGraph pass list
```

AI 不默认读：

```text
RHI command details
GPU buffer
font atlas page
shader variant
batch key
```

## 8. 第一版边界

第一版做：

```text
AuiRendererBridge
AuiOverlayFrame
AuiOverlayDrawItem
RuntimeRendererInput.aui_overlay
DrawUiOverlay pass
headless tests
```

第一版不做：

```text
真实 GPU UI 渲染。
真实字体 atlas。
复杂 Text shaping。
Clip / Mask。
UI batching。
Button state。
HitTest。
WorldSpace UI。
```

## 9. 结论

本方案最接近 UE Slate 路线：

```text
UE Slate DrawElement ≈ AuiDrawList / AuiOverlayDrawItem
UE Slate Renderer ≈ AuiRendererBridge + RuntimeRenderer UI pass
```

它保留 AUI 的 AI 友好结构，又不把 UI 过早绑定到底层 RHI。
