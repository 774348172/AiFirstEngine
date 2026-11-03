# 299 Windows Player AUI Clean-Frame Cache Minimal Repair v1

状态：已完成并归档；用户于 2026-08-18 授权按最小修复直接施工。

## 目标与边界

把 294 已在普通 Editor GameView 验证的 AUI clean-frame present cache 接入 Windows 导出
Player 的现有帧循环，避免 binding 可见值未变化时重复执行 document resolve、layout、draw list
与 glyph plan。

唯一产品源码 owner：

```text
rust/crates/runtime_player_winit/src/lib.rs
```

不新增 schema、migration、公共 API、缓存层或诊断家族；不修改 Tower 玩法、项目配置、帧调度、
字体、FPS UI、输入反馈或 renderer 架构；不运行 Local CI，不替换 production Editor/Player，
不重建真实 Tower Preview cache。

## Gate A：单 owner 修复与定向验证

1. 增加 owner test：相同可见 binding 值连续两帧只 rebuild 一次并命中一次 cache；值变化后
   rebuild/revision 各增加一次。旧实现必须无法满足该测试。
2. 在 `runtime_player_winit` 内增加 session-local cache，active binding paths 只计算一次；每帧仍由
   producer 生成 active-path snapshot，值相同则以 `take/store` 所有权转移复用上一份 present。
3. 将 real-window persistent session、headless gate 与兼容 window loop 三个现有调用点接入同一 helper。
4. 运行 owner test、`runtime_player_winit` affected tests、real-window check 与 fmt check。
5. 使用 source-linked/run-owned Player 做一次 120-180 帧有界复测，证明 clean frame 不再 rebuild，
   并对比 2026-08-18 基线 `update.meanMs=59.13`；不以 60 FPS 作为本 Gate 的扩大验收。

失败停止条件：owner test 或 affected check 失败时只修本 owner；需要改公共 schema、Tower 或生产态时
立即停止并另行取得授权。

## 自审

- 反事实最小：缺口是 294 cache 未进入 Player owner，不需要重新设计 AUI。
- 修改面限制为一个 Rust owner 加施工/完成入口文档。
- 验证贴近 owner，性能复测有界；没有完整 E2E、发布资格化或 production promotion。
- 当前施工槽此前为空，编号 299 未占用；可直接执行 Gate A。

## 完成结果

- 旧基线 owner test 因缺少 `NativeAuiPresentCache` 确定性编译失败；修复后 1/1 passed。
- real-window persistent session、headless gate 与兼容 window loop 已共用一个 session-local cache；
  30 帧真实 Tower 临时计数为 `rebuild=2 / hit=28 / revision=2`，计数诊断随后已从源码移除。
- `cargo check -p runtime_player_winit --features real-window` 与 scoped fmt check passed。
- crate suite 为 25 passed / 3 failed；三项失败均在进入本 owner 前由既有测试 fixture 把 `rules`
  写入错误的 `runtimeModule` JSON 层级导致 package parse 失败，本轮未扩大修复。
- source-linked Tower 180 帧报告证明 present 不再每帧 rebuild，但 `update.meanMs` 仍为约
  `59.18-61.24`，未改善旧 `59.13` 基线。10 帧临时分段计时定位为 cache 判断前的
  768-path ABI snapshot production 约 `50-59ms/frame`；AUI input 约 `2ms`、host tick 约
  `0.7-1.8ms`。临时计时已删除。

299 只完成 Windows Player present cache 接线。要继续消除卡顿，必须另立 producer 前置
dirty/revision 合同；不得在本 repair 内加入轮询节流、Tower 特判或公共 schema 扩张。
