# 269 Project Open + Editor Play Latency Convergence v1 施工文档

状态：Gate A-E 已完成，2026-08-04 归档。方案已由用户明确确认。

## 范围与禁止项

Owner：`editor_core` preview/source policy、Play service、composition artifact；
consumer：`editor_window_winit` background operation、`editor_ui_model` / `editor_ui_renderer` progress。

禁止修改 Tower Defense 内容；禁止 production replacement、真实配置、Local CI；禁止删除项目内
`RuntimeModule/target` 或 `Tests/**/target`。

## Gate A - Preview 指纹

- 为含 Cargo `target` 的真实结构 fixture 增加 red test；
- 统一 source policy、流式摘要与 cancellation；
- 验证 `cargo test -p editor_core editor_preview_package_ -- --nocapture`。

## Gate B - Async Play

- 将 preparation 拆为 worker prepare 与 UI-thread commit；
- 增加 typed progress、busy/repeat protection、cancel/join 和 project identity recheck；
- 验证 `editor_window_winit` async Play owner tests 及 `editor_ui_model` / renderer 定向测试。

## Gate C - GameView Player 边界

- in-process GameView 跳过 Project Player artifact；
- Export / standalone consumer 保持现有 build；
- 验证 `editor_core` Play service、project player/export consumer tests。

## Gate D - Composition 编译缓存

- exact artifact seal 与 compilation compatibility identity 分离；
- application-owned shared Cargo target root，不进入 project 或 staging；
- 验证 cache path containment、identity mutation matrix、failed build cleanup 不删除共享 cache。

## Gate E - 回归与闭环

- `cargo fmt --all -- --check`；
- `cargo test -p editor_core`；
- `cargo test -p editor_ui_model`；
- `cargo test -p editor_ui_renderer`；
- `cargo test -p editor_window_winit -- --test-threads=1`；
- 用 Tower 捕获报告/只读目录证明旧 6.24 GiB target 不再是输入；必要时使用 run-owned state root
  做真实 cached composition timing，不修改 production state。

完成后生成阶段完成记录，更新 49/54/文档地图/施工索引并移入 `已完成/`。若任一 Gate 失败，先保留
首个可定位诊断，不跨 Gate 堆积。
