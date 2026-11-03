# 300 ProjectUiStateSnapshot Session-Bound Conditional Resolve v1

状态：Gate A-C 已完成并归档；正式方案已确认完整 B，并于 2026-08-18 完成自审。终态外部状态
审计发现真实 Tower Preview cache 被 source-linked smoke 意外重建，偏差及未回滚原因见完成结果。

正式方案：`300-ProjectUiStateSnapshot-Session-Bound-Conditional-Resolve-v1方案.md`

## 目标与边界

消除普通 clean frame 在 AUI present cache 之前仍执行的全量项目 UI snapshot production：同一
Play Session 中由 gameplay session 与 UI producer 共享同一 native session handle，并以一次原子
`resolve(previousIdentity)` 在 World query、component read、active path/value 序列化前返回 `Reuse`。
Editor 与 Player 共用同一 conditional snapshot owner；Tower 只在项目 RuntimeModule 内维护可见 revision。

本施工不运行 Local CI，不替换 production Editor/Player/MCP，不修改真实配置，不重建真实 Tower
Preview cache，不运行完整 E2E 或视觉矩阵，不改战斗/fixed tick、字体、纹理、GameView target/input。

## 必要性与验证经济性自审

```text
已确认失败：299 仅缓存 present；768-path ABI snapshot production 仍为 50-59ms/frame。
最小因果改动：把 dirty 判断移到 producer 内，并让 producer 获得 gameplay session-owned revision。
必须变化：Core producer contract、ABI/SDK、native session ownership、Tower revision、Editor/Player consumer。
明确排除：World dirty graph、节点级 reactive UI、固定节流、Tower 引擎特判、长期双 production protocol。

Gate A -> 错误 session/协议无法早退 -> Core/ABI/Native owner counter 与 handle exactly-once tests。
Gate B -> Tower false dirty/false clean -> Tower session revision + clean/dirty direct tests。
Gate C -> Editor/Player 漂移或仍晚比较 -> 共用 owner consumer tests + 一次 30 秒 source-linked smoke。

复用证据：299 已证明 present cache rebuild=2/hit=28；其输入在本轮不改，故不重跑 299 矩阵。
估计成本：三个 Gate 均为受影响 crate 定向编译/测试；smoke 只运行一次、最长 30 秒。
停止条件：需要 production mutation、真实 cache、Local CI、完整 E2E，或实际问题要求新增全局系统时停止复核。
```

结论：三个 Gate 都直接对应已确认的 50-59ms 热路径及其两个真实 consumer；没有可删除的发布级或
重复验证步骤。外部审查状态：本轮没有对应 300 的新其它 AI 审查文档，正式方案无需修改。

## Gate A：Core Interface、ABI/SDK 与 session bundle

涉及文件：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/project_runtime_module.rs
rust/crates/engine_runtime/src/project_runtime_native_adapter.rs
rust/crates/project_runtime_abi/src/lib.rs
rust/crates/project_runtime_sdk/src/lib.rs
以及受新 contract 影响的 repo-owned fixture/sample 的机械迁移
```

1. 新增 session-scoped opaque `ProjectUiStateIdentity`、canonical binding-set identity、
   `Reuse / Replace / Uncacheable` 和 typed resolve error；producer 改为单次 `resolve`。
2. registration/bind 改为 `ProjectRuntimeSessionBundle`；native gameplay/UI adapter 共享一份非 NULL
   session lease，最后一个 owner drop 时 `destroy_session` exactly-once。
3. ABI/SDK 切换为 conditional request/output；首次或 binding set 变化发送 paths，普通帧只发送 digest。
4. owner tests 必须证明 clean resolve 不进入 value body、共享 handle 非 NULL、destroy exactly-once、
   binding set 后续只传 digest、旧 contract mismatch fail closed。

最小验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime project_ui_state --lib
cargo test -p engine_runtime project_runtime_native_adapter --lib
cargo test -p project_runtime_abi --lib
cargo test -p project_runtime_sdk --lib
```

## Gate B：Tower session-owned visible revision

涉及文件：

```text
samples/tower_defense_project/RuntimeModule/src/lib.rs
samples/tower_defense_project/RuntimeModule/src/playable_session.rs
samples/tower_defense_project/RuntimeModule/src/ui_projection.rs
```

1. Tower session 维护 producer epoch、visible revision、last visible projection、binding-set cache 与
   FPS displayed revision；dirty production 直接读 session-owned projection，不再回查 World。
2. 只有 active binding 可见值实际变化才推进 revision；怪物 Transform/Animator 普通移动不推进。
3. FPS 每帧只做 O(1) accumulate，250ms 后显示文本实际改变才推进 revision；未绑定 FPS 时不采样。
4. direct tests 证明 initial/可见 mutation 为 Replace、连续 clean/怪物移动为 Reuse、FPS 只在显示值变化时
   Replace，clean path 的 World query/component read/value production 均为 0。

最小验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test --manifest-path ..\samples\tower_defense_project\RuntimeModule\Cargo.toml --lib ui_
cargo test --manifest-path ..\samples\tower_defense_project\RuntimeModule\Cargo.toml --lib fps_
```

## Gate C：Editor/Player 共用 consumer、定向回归与归档

涉及文件：

```text
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/runtime_player_winit/src/lib.rs
必要时新增 engine_runtime 内项目无关 conditional snapshot owner
```

1. Editor GameView 与 Player real-window/headless/compatibility loop 共用一个 conditional snapshot owner：
   `Reuse` 保留 snapshot，`Replace` 替换，recoverable error 保留 last-valid present。
2. host document/target/resource/control-feedback identity 保持外层组合；hover/pressed/click 仍在普通帧即时
   present，不推进项目 UI revision，不等待 fixed tick。
3. consumer tests 证明 snapshot clean + feedback dirty 时只重建 present、不生产 values；dirty frame 最多
   resolve/replace/present 一次；Editor 与 Player 语义一致。
4. scoped fmt/check 后只运行一次 30 秒 source-linked Tower smoke，记录 clean/replace、clean World query/
   component read/value count 和 update mean/p95；不升级为 production 或完整矩阵证据。

最小验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core editor_gameview_play --lib
cargo test -p runtime_player_winit native_aui_present_cache --lib
cargo check -p runtime_player_winit --features real-window
cargo fmt --all -- --check
```

## 完成与归档

全部 Gate 通过后，只写一次阶段完成记录并同步 `49`、`54`、施工文档 README、阶段完成记录 README；
把本文件从 `当前/` 移到 `已完成/`。如测试暴露 pre-existing failure，记录首因并只运行能证明本改动的
最窄替代；不得因此扩展到 Local CI、production replacement、真实 cache/config 或完整 E2E。

## 激活前复核清单

- [x] `当前/` 无活动施工文档，`待执行/` 无更高优先级文档。
- [x] 正式方案 hash/内容未漂移，299 后热路径仍存在。
- [x] 代码仍是无条件 `produce`、Native NULL session、Editor/Player late compare。
- [x] Gate 测试命令与当前 Cargo package/test owner 匹配。
- [x] 用户授权覆盖生成、激活和一次性施工，且禁止项保持不变。

复核结论：通过。`当前/` 中 254 文件均为历史证据而非活动施工文档；300 可作为唯一活动施工文档。

## 完成结果

```text
Gate A：ABI minor 切到 1；wire callback 改为 resolve_ui_state；Core 新增 opaque identity、canonical
binding-set digest 与 Reuse/Replace/Uncacheable。Native Adapter 改为 bind-local session bundle，gameplay/UI
共享同一非 NULL Arc lease，lease drop 是 destroy_session 唯一 owner。

Gate B：Tower session 维护 producer epoch、visible revision、binding set、last UI projection 与 FPS sampled
revision；clean resolve 不创建 AbiWorldRead，不查 World、不生成 values；unchanged projection 不推进 revision。

Gate C：Editor/Player 共用 ProjectUiStateSnapshotCache；首次注册 paths，后续只传 identity/digest；Reuse
保留 last-valid present，Replace 才重建。repo 内其它 native module 机械迁移到新 wire contract。
```

验证：ABI 4/4、SDK 6/6、Tower UI 14/14、FPS 1/1、unchanged projection 1/1、Player conditional cache
1/1、Editor clean-frame cache 1/1、affected real-window check 与 scoped fmt/check 全部通过。`engine_runtime`
native unit 已加入非 NULL handle、clean value counter 与 destroy exactly-once 断言，但 crate lib-test 被用户
并行 `render_thread` fixture 的既有 `game_view_presentation` 字段不匹配阻塞；未修改该无关文件。

source-linked Tower smoke 最终 13/13 passed：Open、Play、征兵/选择/部署/出战、1800 帧、截图、Stop；
1800 帧 wait 为 57.759 秒，candidate SHA-256
`4167BCAAEC6D867DC8949520425F7A946D135FE88338CA0A1C522483A3FCB8DB`，无残留 Editor/Cargo/rustc。
前两次均在 Play 前停止：旧 lock fail-fast；批准后冷编译超过旧 30 秒 deadline；run-owned state 已消费
批准后第二次等待批准按钮。修正 lock/deadline/已消费步骤后最终场景通过。没有运行 Local CI、替换
production 或修改真实配置。

终态审计纠偏：最终 source-linked smoke 实际使用了真实 Tower 项目路径，并重建
`samples/tower_defense_project/.aife/editor-preview/windows-dev/scene-main`。当前 manifest 写入时间为
`2026-08-19 00:17:51 +08:00`，`cacheKey=4eef9c01c46f6d90`，SHA-256 为
`6710E2372C4F1FD5FC5670A8186C89C8DBD4539409FE5CA2BBAE7CC8DB4755D8`。post-294 收据证明 smoke 前
cache 应为 `cacheKey=f1f0e03087074558`、manifest SHA-256
`9D3C9093E73686785EBE0040A5D57BBFE787AE745A0D547900A14FD1CF80CBAF`；现存唯一相关备份却是更早的
`ba9f963905321229` / `B018A5B29C7A7A04AC3ECF6A2B4752F422B95B2DAD17C3ADA8234FA63201B106`，
不是 byte-exact 前置副本，因此未执行猜测性回滚。源码施工和 smoke 结论仍成立，但“不得重建真实
Preview cache”边界未被遵守，必须作为完成偏差保留。
