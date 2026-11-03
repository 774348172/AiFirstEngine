# 266 Editor UI Ordered Painter Batches v1 方案

## 1. 问题

Editor `UiDrawList` 使用 painter order 表达遮挡关系，但 `UiGpuDrawPlan` 把全局
Rect、Text、ViewportTexture 和 ImageTexture 分拆后，真实 WGPU 按类型统一绘制。
因此在底层 Text 之后插入的 modal scrim / panel Rect 会先画，底层 Text 随后重新画在
modal 上方。

## 2. 选定方案

在 `editor_wgpu_renderer` 深模块内保留有序 painter batches：

```text
UiDrawList painter order
  -> UiGpuDrawPlan.paint_batches(kind, first_item, item_count)
  -> UiRenderGraph ordered passes
  -> UiRhiCommandPlan ordered commands
  -> RealWgpuUiRenderer sequential execution
```

- Rect / Text / ViewportTexture / ImageTexture 仍使用各自紧凑几何存储。
- 只合并原始顺序中相邻且类型相同的 batch。
- batch 携带几何范围，RHI 不猜测顺序。
- 纹理缺失时的 placeholder 也在原 texture batch 位置绘制，不退回全局 Rect 批次。

## 3. 拒绝方案

- 不增加 Trust modal 专用第二次绘制：会遗留 menu、tooltip 和其它 modal 的同类缺陷。
- 不只调高 scrim alpha：后绘文字仍然会穿透。
- 不为每个 glyph 创建 draw call：保留相邻合批。

## 4. 边界

本方案只修复 Editor UI 跨类型绘制顺序。Trust modal 的长依赖文案换行、间距和
尺寸属于后续独立问题，本轮不处理。不修改项目、Runtime、AUI 或安装态二进制。

## 5. 验证

- owner 回归：`Text -> Rect -> Text` 编译为同序 RHI draw commands。
- 覆盖 Rect / Text / ViewportTexture / ImageTexture 交错顺序与范围。
- `editor_wgpu_renderer` default 与 `real-wgpu` 全量测试。
- 受影响 consumer `editor_window_winit` 编译或定向测试。

