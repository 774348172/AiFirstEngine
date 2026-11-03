# 261 Window F / Gate F 完成记录

日期：2026-07-30

状态：Window F / Gate F 已完成

证据级别：dirty-worktree development evidence

## 1. 完成范围

`UiProjection` 已按 `(FontBundleRenderMode, page_index)` 生成稳定字体 batch，并把
render mode、page、UV、颜色、texture handle 与顶点传给 RenderGraph/RHI。正式 v2
FontBundle 使用每页稳定 handle；legacy atlas 保持单纹理兼容路径。

真实 wgpu backend 已建立 Bitmap R8 coverage 与 MSDF RGBA8 median/smoothstep 两条 text
pipeline。Bitmap 与 MSDF page 分别上传为 `R8Unorm` 和 `Rgba8Unorm` 的二维 texture array，
每个 layer 通过 page handle 绑定；Player `real-window` consumer 优先上传 v2 FontBundle，
没有 v2 时才回退 legacy alpha atlas。

## 2. Gate F 验证

```text
cargo test -p engine_runtime ui_projection_font --offline -- --nocapture
结果：1 passed，0 failed

cargo test -p engine_runtime font_texture_array --features real-wgpu --offline -- --nocapture
结果：1 passed，0 failed；本机真实 adapter 已执行

cargo test -p engine_runtime aui_text_bitmap_pipeline --features real-wgpu --offline -- --nocapture
结果：1 passed，0 failed；局部非空 coverage 输出

cargo test -p engine_runtime aui_text_msdf_pipeline --features real-wgpu --offline -- --nocapture
结果：1 passed，0 failed；局部非空 median/smoothstep 输出

cargo test -p runtime_player_winit font_texture_array --features real-window --no-run --offline
结果：真实 Player consumer feature 编译通过
```

施工文档原建议命令把 `real-wgpu` 写在 `runtime_player_winit` 上，但该 crate 的真实 feature
名为 `real-window`，并由它传递启用 `engine_runtime/real-wgpu`。GPU owner 测试因此放在
`engine_runtime`，Player 运行相应 consumer 编译 Gate；施工文档已同步修正。

首次真实 wgpu 测试编译因测试 helper 闭包类型推断失败，补充显式类型后出现一个闭包结构体
字面量语法错误；两处均为测试代码问题。修复后原 Gate 原样重跑全绿。

## 3. 边界与下一步

尚未完成 generation 精确失效、frame-boundary 原子切换、失败保旧、GPU generation retirement
或 Preview/Play/Export digest parity。未运行 Local CI、production replacement、安装态变更或
真实窗口视觉验收。按连续授权进入 Window G / Gate G。
