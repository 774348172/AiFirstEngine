# 254-R2 R2-O7 环境等价预检报告 v1

日期：2026-07-22

结论：`blocked_before_authoritative_regression`

## 已满足

- Windows 本机 Rust/Cargo 1.96.0。
- R2-O0 至 R2-O5 定向测试通过。
- R2-O6 受影响域回归通过。
- 未修改 production Codex config。
- 未创建 production candidate、activation 或 attempt。
- 未启动真实 Codex，未执行 R2-N8、R2-G、F-A/F-B/G 或三引擎 B 通道。
- 3 个 ignored test 均为既有 construction-only fresh R2-N6/R2-N8/R2-G environment preflight，不是真实 acceptance 测试。

## 阻塞事实

```text
HEAD = 9f5adc73ca02a5e3fed3f38c62bb9a420e6bd5de
branch = codex/248-editor-widget-tree
tracked modified files = 138
untracked files = 221
tracked files under rust/crates/ai_tool_gateway = 0
```

本轮实现依赖整个未跟踪的 `rust/crates/ai_tool_gateway`、多个未跟踪的 `editor_core` 文件，以及含用户既有修改的 `editor_window_winit/src/application.rs`。当前状态无法证明一个不混入用户改动、同时完整包含 R2-O2 至 O6 实现的 exact commit。

## 禁止降级

- 未运行 `cargo test --workspace`。
- 未运行 `cargo test --workspace --all-features`。
- 未在当前脏工作树把受影响域回归冒充最终权威回归。
- 未创建 commit、临时 index commit 或混合 patch bundle。
- 未清理、回退、暂存或覆盖用户改动。

## 解除条件

先由仓库 owner 明确整理并提交当前 254/R2 source-material 边界，使本轮代码、manifest、lockfile、测试和施工记录落到一个可复现 exact commit。之后从该 commit 建立 fresh clean worktree，配置隔离的 `CARGO_TARGET_DIR`、artifact root 与 Gateway discovery/pipe identity，再单独恢复 R2-O7 preflight。不得重跑 R2-O2 至 O6 来替代该边界整理。
