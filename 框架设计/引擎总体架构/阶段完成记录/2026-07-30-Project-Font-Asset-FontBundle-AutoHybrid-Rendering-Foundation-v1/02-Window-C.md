# 261 Window C / Gate C 完成记录

日期：2026-07-30

状态：Window C / Gate C 已完成

证据级别：dirty-worktree development evidence

## 1. 完成范围

新增 `cooked-font-bundle.v2`、page/glyph 合同、RuntimeFontBundle manifest、RuntimePackage
source、严格 Loader 与原子 registry。每个 page 冻结 render mode、format、width/height、
byte_len、SHA-256 和 payload path；bundle digest 通过 page descriptor 中的 hash 覆盖全部
page bytes。Loader 拒绝半包、损坏 hash、错误 byte length/format/order、重复 glyph key、
无效 rect、page budget overflow 和 legacy quality 伪装。

`ProjectFontBundleBuilder` 从 Window B 的 Hinting R8 variants 做稳定排序、去重和 bitmap
multi-page packing。RuntimePackage Builder 写入 v2 metadata、全部 page payload 和 manifest，
正式 `load_runtime_package` 已能加载完整 v2 bundle。

正式项目字体 cook 只进入 v2 bundle；无正式字体资产的旧项目继续使用 208 v1 adapter。
legacy load report 固定 `legacy_mode=true`、`quality_gate_eligible=false`，不会通过 261
Font Quality Gate。

## 2. Gate C 验证

```text
cargo test -p engine_runtime cooked_font_bundle --offline -- --nocapture
结果：3 passed；0 failed

cargo test -p engine_runtime runtime_font_bundle_loader --offline -- --nocapture
结果：1 passed；0 failed

cargo test -p editor_core project_font_bundle_builder --offline -- --nocapture
结果：2 passed；0 failed

cargo test -p editor_core project_font_bundle_determinism --offline -- --nocapture
结果：1 passed；0 failed

cargo test -p engine_runtime runtime_package_builder_writes_and_loads_cooked_font_atlas --offline -- --nocapture
结果：1 passed；0 failed

cargo test -p editor_core project_runtime_package_assembler --offline -- --nocapture
结果：10 passed；0 failed
```

首次 Builder 命令在增量编译阶段超过 60 秒工具时限，没有测试失败；相同非外部状态命令在
编译完成后重跑通过。

## 3. 边界与下一步

Window C 只完成 bitmap multi-page；未声称 MSDF production recipe、AutoHybrid、AUI v2
registry selection、GPU texture array 或 hot reload generation 完成。未运行 Local CI、
production replacement、安装态和真实 GPU/窗口验证。

按连续授权进入 Window D / Gate D。
