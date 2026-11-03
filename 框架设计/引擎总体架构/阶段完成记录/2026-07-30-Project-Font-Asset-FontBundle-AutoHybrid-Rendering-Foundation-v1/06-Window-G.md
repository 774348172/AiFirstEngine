# 261 Window G / Gate G 完成记录

日期：2026-07-30

状态：Window G / Gate G 已完成

证据级别：dirty-worktree development evidence

## 1. 完成范围

项目字体 cook dependency digest 已从完整文本哈希收敛为 recipe/profile/assets/source font hash、
规范 codepoint 集合与实际相邻 kerning pair 集合。文本重排但 glyph/pair 集合不变时复用 bundle；
新增 codepoint 或 pair 时精确失效。

新增 `RuntimeFontGenerationManager`：新 generation 必须 page metadata/payload 全部 prepare 后，
才能在显式 frame boundary 原子成为 active；prepare/reject 失败保持旧 generation。旧 generation
按 `frame_index + max_frames_in_flight` 进入退休队列，只有 completed frame 越过该边界才可回收。

FontBundle page GPU handle 已编码 generation。真实 wgpu backend 可让新旧两代 texture arrays
并存，并只按旧上传 receipt 显式释放旧代；UiProjection 使用 glyph plan 的 atlas generation
选择对应 handle。Preview、Play、Export 下游 RuntimePackage 独立产物验证为同一 bundle digest，
且产物中没有 TTF/OTF/TTC 源字体。

## 2. Gate G 验证

```text
cargo test -p editor_core project_font_hot_reload --offline -- --nocapture
结果：1 passed，0 failed

cargo test -p engine_runtime runtime_font_generation --offline -- --nocapture
结果：2 passed，0 failed

cargo test -p engine_runtime font_generation_gpu_retirement --features real-wgpu --offline -- --nocapture
结果：1 passed，0 failed；本机真实 adapter 已执行

cargo test -p engine_runtime font_preview_play_export_parity --offline -- --nocapture
结果：1 passed，0 failed

cargo test -p runtime_player_winit font_generation_gpu_retirement --features real-window --no-run --offline
结果：真实 Player consumer feature 编译通过

cargo fmt --check
结果：通过
```

首个 hot reload 测试错误使用了 fixture 不包含的 `B`，触发正确的 `RequiredGlyphMissing`；
改用 fixture 已资格验证的 `A/1/2` 后通过。parity 测试首轮缺少 scene 引用的 texture AssetRef，
Builder 正确报 `MissingAssetRef`；补齐既有 minimal asset 合同后原样重跑通过。

## 3. 边界与下一步

Window G 不运行 Local CI、production replacement、安装态或真实窗口。按连续授权进入 Window H，
但 H 中 production composition、真实 Windows/GPU screenshot、exported Player 视觉验收及
clean exact-commit 仍需独立授权，当前只能执行自动化 negative/consumer/compatibility 范围。
