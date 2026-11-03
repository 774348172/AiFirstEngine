# 261 Window E / Gate E 完成记录

日期：2026-07-30

状态：Window E / Gate E 已完成

证据级别：dirty-worktree development evidence

## 1. 完成范围

AUI text style 新增 per-node FontBundle/family/style/weight/raster policy。样式从 AUI Document
进入 DrawText 与 Overlay，不进入项目 gameplay 语义或 GPU handle。

`RuntimeFontRegistry` 在已原子加载的 v2 bundle 上按 bundle/family、exact style、nearest
weight、render mode 和 nearest pixel size 解析 glyph；fallback 可见且不会把真实中文替换成
`?` 后仍报告 exact。FontBundle 保存基础 kerning，AUI v2 glyph planning 在定位下一个 glyph
前应用 adjustment。

v2 glyph plan 输出 page index、Bitmap/MSDF render mode、UV、generation 和 compact counts；
Player、Editor GameView 与项目 E2E consumer 已切换为优先使用 v2 FontBundle，legacy atlas
仅在无 v2 bundle 时生效。

## 2. Gate E 验证

```text
cargo test -p engine_runtime runtime_font_registry --offline -- --nocapture
结果：1 passed；0 failed

cargo test -p engine_runtime aui_font_style --offline -- --nocapture
结果：1 passed；0 failed

cargo test -p engine_runtime aui_text_glyph_plan --offline -- --nocapture
结果：1 passed；0 failed

cargo test -p engine_runtime aui_text_kerning --offline -- --nocapture
结果：1 passed；0 failed

cargo test -p runtime_player_winit --no-run --offline
结果：consumer 编译通过
```

首次 `aui_text_kerning` 过滤得到 0 tests，未计为 Gate；补充精确 owner 测试后重跑为
1 passed。两次 consumer 编译命令曾被 60 秒工具时限终止，增量编译完成后独立重跑通过。

## 3. 边界与下一步

尚未完成 GPU texture arrays、Bitmap/MSDF shader pipeline、frame-boundary generation swap
或真实视觉矩阵。未运行 Local CI、production replacement、安装态和真实窗口验证。

按连续授权进入 Window F / Gate F。
