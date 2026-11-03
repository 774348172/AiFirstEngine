# 266 Editor UI Ordered Painter Batches v1 施工文档

```text
状态：Gate A-C 已完成，2026-08-03 归档
红测试：[DrawRectBatch, DrawTextBatch]
修复后：[DrawTextBatch, DrawRectBatch, DrawTextBatch]
```

## 授权与范围

用户已授权按 266 方案修改。只修改 `editor_wgpu_renderer` 的 draw plan / render graph /
real WGPU consumption 及回归测试。不修 Trust modal 布局，不运行 Local CI，不替换
production / 安装态二进制，不修改真实用户配置。

## Gate A - 合同与红测试

- 新增 ordered paint batch 合同的 owner 回归。
- 测试必须在当前全局按类型重排实现上失败。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_wgpu_renderer ordered_painter_batches_preserve_cross_kind_occlusion -- --nocapture
```

## Gate B - 实现

- `UiGpuDrawPlan` 生成有序、相邻合并的 paint batches。
- RenderGraph / RHI 保留 batch 顺序与范围。
- Real WGPU 按 RHI command 顺序提交，缺失纹理 placeholder 保留原位。

## Gate C - 受影响验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_wgpu_renderer
cargo test -p editor_wgpu_renderer --features real-wgpu
cargo test -p editor_window_winit --no-run
```

## 自审

- 与 266 方案一致，没有引入 modal 专用特例。
- 验证直接覆盖用户的穿透症状和共享 consumer。
- 不扩大到文字换行、production replacement 或 Local CI。
- 当前施工槽原为空，本文档是唯一当前施工文档。
