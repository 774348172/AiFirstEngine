# 261 Window D / Gate D 完成记录

日期：2026-07-30

状态：Window D / Gate D 已完成

证据级别：dirty-worktree development evidence

## 1. 完成范围

`ProjectFontCookModule` 使用 Gate A 已资格化的 fdsm backend，为真实 glyph outline 生成固定
edge-color seed、em size、pixel range、error correction、sign correction 和 u8 量化的
确定性 MSDF RGBA8 variant。Bitmap 与 MSDF 从同一解析结果取得 normalized advance。

`ProjectFontBundleBuilder` 新增 MSDF multi-page packing、RGBA8 page hash/budget/digest，并与
Bitmap page 使用同一 bundle/glyph schema；正式输出不存在普通单通道 SDF。

AutoHybrid 决策按 authored size × DPI × node scale 得到 physical px：大于 32 physical px、
连续缩放、粗描边或 bitmap scale 超出 `[0.875, 1.125]` 时选择 MSDF，否则选择 Hinting R8；
每次决策返回稳定 reason。

## 2. Gate D 验证

```text
cargo test -p editor_core project_font_msdf --offline -- --nocapture
结果：2 passed；0 failed

cargo test -p editor_core project_font_auto_hybrid --offline -- --nocapture
结果：1 passed；0 failed

cargo test -p editor_core project_font_variant_metrics --offline -- --nocapture
结果：1 passed；0 failed

cargo test -p editor_core project_font_bundle_determinism --offline -- --nocapture
结果：1 passed；0 failed
```

## 3. 边界与下一步

未完成 AUI style/face selection、runtime glyph planning、GPU texture arrays/pipelines 或 hot
reload。未运行 Local CI、production replacement、安装态和真实 GPU/窗口验证。

按连续授权进入 Window E / Gate E。
