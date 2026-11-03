# 245-当前可自动化施工文档-Reproducible Toolchain / CI / Lint Budget Gate v1

> 状态：已完成；方案 A 本地 exact-commit CI 权威证据通过，CQ-08 已关闭，本文已归档。
> 正式方案：`../../245-Reproducible-Toolchain-CI-Lint-Budget-Gate-v1方案.md`。  
> 方案选择：用户已确认方案 C，并确认方案 A 本地 Git 修订；LocalCiRunner 为 authoritative Adapter，GitHub Actions 为可选 Adapter。  
> 前置：244/CQ-06 已完成、回归并归档；生成本文前 `施工文档/当前/` 为空。  
> 执行规则：严格按 Gate A -> B -> C -> D -> E 串行执行；当前 Gate 失败先修当前 Gate，不跨 Gate 堆积问题。

## 1. 施工目标

关闭 CQ-08：

```text
固定 rustc/Cargo/rustfmt/Clippy 版本和组件。
所有 workspace member 继承统一 lint policy。
本地和 CI 只调用一个 QualityGateRunner verify interface。
基于 post-244 工作树建立真实 lint debt ledger，不使用历史 220 作为当前数量。
逐条识别 known/new/resolved/stale/changed warning，禁止总数置换。
登记并阻断 unmanaged allow/expect/cfg_attr suppression。
运行 fmt、default workspace、all-features workspace、Clippy 与 hygiene evidence。
生成 quality-gate-report.v1。
建立有限时、可追溯、绑定本地 commit 的 LocalCiRunner；保留 GitHub Actions 可选 Adapter。
```

完成必须证明：

```text
错误工具链、缺组件、lockfile 漂移 fail closed。
new warning、warning replacement、occurrence 增长、stale/resolved/expired ledger fail closed。
未登记 source suppression fail closed。
普通 verify 不会修改 committed ledger。
本地/CI 使用同一 command plan 与 report schema。
default/all-features workspace 最终通过。
普通 developer verify 与 local exact-commit CI observed 分开记录。
```

## 2. 施工范围

预期新增：

```text
rust/rust-toolchain.toml
rust/crates/quality_gate/Cargo.toml
rust/crates/quality_gate/src/lib.rs
rust/crates/quality_gate/src/main.rs
rust/crates/quality_gate/src/toolchain.rs
rust/crates/quality_gate/src/command.rs
rust/crates/quality_gate/src/cargo_json.rs
rust/crates/quality_gate/src/lint_ledger.rs
rust/crates/quality_gate/src/suppression.rs
rust/crates/quality_gate/src/report.rs
rust/crates/quality_gate/src/local_ci.rs
rust/crates/quality_gate/tests/*
rust/quality/lint-debt-ledger.v1.json
.github/workflows/rust-quality.yml
```

预期修改：

```text
rust/Cargo.toml
rust workspace member Cargo.toml：统一 [lints] workspace = true
240、00、49、54、施工文档/README、阶段完成记录/README
245 正式方案状态
```

明确不修改：

```text
244 方案、归档施工文档与完成记录。
Runtime/Editor/Player 业务行为与 schema。
code_hygiene 的文件规模阻断规则；CQ-07 继续 deferred。
项目玩法、AUI、Renderer、ProjectPatch、LLM 功能。
```

## 3. 外部审查落地

### 必须修改

```text
无固定 toolchain -> Gate A。
无 workspace lint policy -> Gate A。
无 CI / 统一入口 -> Gate B/D。
strict Clippy 失败且 warning 增长 -> Gate A/C 的 fingerprint ledger。
```

### 施工约束

```text
default/all-features 都阻断 -> Gate D/E。
Clippy 用 Cargo JSON，不解析终端文本 -> Gate C。
普通 verify 不更新 ledger -> Gate C negative test。
LocalCiRunner 不复制 command plan、有限时 cleanup、无 source/index/branch mutation -> Gate D。
本地 exact-commit detached worktree run 单独证明 -> Gate E。
```

### 历史吸收

```text
CQ-02 已关闭，只继承 all-features Gate。
151 已有 code_hygiene.report.v1，只聚合 evidence。
239-244 的正确性施工不重复实现。
```

### 不适用

```text
CQ-07 文件规模和 Module decomposition budget 不扩入。
其它已关闭 CQ/INC 不扩入。
Provider/Agent/installer/signing/game feature 不适用。
```

读取两份 5.6 审查后，无需修改 245 正式方案；缺口已全部分配到 Gate。

## 4. 固定合同

### 4.1 工具链

```toml
[toolchain]
channel = "1.96.0"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

### 4.2 唯一入口

```powershell
cargo run -p quality_gate -- verify
```

### 4.3 schema

```text
lint-debt-ledger.v1
quality-gate-report.v1
```

### 4.4 fingerprint

```text
tool + lint_code + workspace-relative path
  + normalized diagnostic message
  + normalized primary source anchor
```

排除绝对路径、行列号、ANSI、target 路径和机器信息。

### 4.5 分类

```text
known：ledger 内且 occurrence 未超额。
new：未知或 occurrence 超额，失败。
resolved：ledger 中但不再出现，失败并要求 prune。
stale：路径/anchor/toolchain/review_by 失效，失败。
changed：resolved + new，失败。
```

## 5. Gate A - Post-244 Baseline / Toolchain / Workspace Policy

### 5.1 任务

1. 确认 244 完成记录存在、`施工文档/当前/` 只有本文。
2. 新增 `rust/rust-toolchain.toml`，固定 1.96.0、rustfmt、clippy。
3. 采集 `rustc -Vv`、`cargo -V`、`cargo clippy -V`、`rustfmt -V`。
4. 用固定工具链执行 Clippy Cargo JSON，得到 post-244 真实 diagnostic set。
5. 结构化 inventory 全 workspace `allow/expect/cfg_attr` suppression。
6. 根 workspace 建立 lint policy；所有 member 显式继承。
7. 生成并人工核对初始 `lint-debt-ledger.v1.json`。
8. 记录 historical 220 只作审查证据，不写成当前 baseline count。

### 5.2 policy 约束

```text
零债务 correctness lint 可 deny。
有历史债务的 lint 保持 warn，由 ledger 阻断 new。
package 不得私下降 level。
不新增 crate-wide allow(warnings)。
```

### 5.3 测试命令

```powershell
rustc -Vv
cargo -V
cargo clippy -V
rustfmt -V
cargo metadata --locked --format-version 1
cargo clippy --workspace --all-targets --all-features --locked --message-format=json
cargo fmt --all -- --check
```

### 5.4 通过条件

```text
固定工具链真实生效。
Cargo.lock 不漂移。
所有 member 继承 workspace lints。
baseline 来自当前 post-244 源码。
ledger 无绝对路径，entries/suppressions 可解析且 owner/reason/review_by 完整。
```

## 6. Gate B - Deep QualityGateRunner / Report

### 6.1 任务

1. 新增 `quality_gate` workspace crate。
2. 建立 `QualityGateRunner::verify(request) -> QualityGateReport`。
3. 建立 internal `QualityCommandExecutor` seam。
4. 实现 `SystemQualityCommandExecutor` 与 `ScriptedQualityCommandExecutor`。
5. child stdout/stderr 并发 drain，stage timeout 后 kill + wait；禁止 pipe deadlock/detach。
6. command plan、working directory、environment allowlist、bounded diagnostics 全部集中。
7. 实现 `quality-gate-report.v1` Summary/Trace 序列化。

### 6.2 报告字段

```text
toolchain expected/actual/matched
host/triple、lockfile/toolchain/manifest/ledger digest
stages[] status/exit/duration/timeout/next_action
lint known/new/resolved/stale/suppression counts/items
default/all_features status
hygiene summary
ci adapter/execution_scope/run/commit/execution_status
diagnostics/passed
```

### 6.3 定向测试

```powershell
cargo test -p quality_gate command
cargo test -p quality_gate report
cargo test -p quality_gate toolchain
```

覆盖：success、non-zero、timeout、kill/wait、malformed output、bounded stdout/stderr、schema roundtrip、绝对路径 sanitization。

## 7. Gate C - Cargo JSON / Fingerprint Ledger / Suppression

### 7.1 任务

1. 只解析 `reason=compiler-message && level=warning`。
2. canonicalize workspace-relative path/message/primary anchor。
3. SHA-256 fingerprint 与 `allowed_occurrences` 对账。
4. 实现 known/new/resolved/stale/changed 分类。
5. 用 Rust syntax parser 扫描 allow/expect/cfg_attr suppression。
6. suppression 与 ledger source_suppressions 对账。
7. `propose-ledger` 只写 `target/quality-gate/candidate/`。
8. verify 运行前后校验 committed ledger digest 不变。

### 7.2 negative matrix

```text
new warning -> fail
warning replacement with same total -> fail
occurrence 1 -> 2 -> fail
old warning removed -> resolved/fail
path/anchor/review_by stale -> fail
line number move only -> stable
semantic anchor change -> changed/fail
unregistered allow/expect -> fail
registered suppression unchanged -> pass
malformed/truncated Cargo JSON -> fail
```

### 7.3 定向测试

```powershell
cargo test -p quality_gate cargo_json
cargo test -p quality_gate lint_ledger
cargo test -p quality_gate suppression
cargo test -p quality_gate --test lint_budget_matrix
```

## 8. Gate D - Canonical Matrix / Thin CI Adapter

### 8.1 verify command plan

```text
toolchain identity
Cargo.lock/manifest/ledger digest
cargo fmt --all -- --check
lint inheritance/suppression audit
cargo clippy --workspace --all-targets --all-features --locked --message-format=json
cargo test --workspace --locked
cargo test --workspace --all-features --locked
cargo run -p code_hygiene -- --root crates
quality-gate-report.v1
```

Cargo workspace commands串行执行，不制造 target lock 竞争。

### 8.2 Local Git Authoritative Adapter

```text
local-ci --commit HEAD 只接受 clean source worktree。
revision resolve 后必须等于当前 HEAD。
在系统临时目录创建 detached worktree，不复用 dirty source tree。
isolated worktree 只调用 cargo run -p quality_gate --locked -- verify --report-mode summary。
通过环境合同写入 adapter=local-git-worktree、execution_scope=local_commit、run_id、commit_sha。
校验 workspace_state=clean、commit/report 一致、passed=true。
复制 quality-gate-report.v1 与 local-ci-run-report.v1 到 target/quality-gate/local-ci/<run_id>/。
成功、失败、timeout 都执行 git worktree remove --force；cleanup 失败则 local CI 失败。
```

### 8.3 可选 GitHub Adapter

```text
contents: read
checkout persist-credentials=false
Windows authoritative runner
committed rust-toolchain.toml
cache key = OS + toolchain + Cargo.lock
job timeout
branch concurrency cancel-in-progress
只调用 cargo run -p quality_gate -- verify
上传 report artifact / job summary
无 secret、无 write permission、无 publish/deploy
```

### 8.4 测试

```powershell
cargo test -p quality_gate matrix
cargo test -p quality_gate ci_contract
cargo test -p quality_gate local_ci
cargo run -p quality_gate -- verify --report-mode summary
```

普通 report 可显示 `workspace_state=dirty`，但必须是 `execution_status=not_authoritative`。local-ci report 必须显示 clean、exact commit 和 observed_commit_passed。

## 9. Gate E - Real Gate / Regression / Activation / Archive

### 9.1 本地真实 Gate

```powershell
cargo run -p quality_gate -- verify --report-mode trace
cargo fmt --all -- --check
cargo test -p quality_gate
cargo test --workspace --locked
cargo test --workspace --all-features --locked
```

### 9.2 mutation evidence

使用临时 workspace/fixture，不修改真实 ledger：

```text
new/resolved/replaced/occurrence/suppression/toolchain/lockfile/JSON/timeout
```

每一项必须证明 report `passed=false`、稳定 diagnostic code 和 next_action。

### 9.3 Local CI activation evidence

```text
adapter=local-git-worktree
execution_scope=local_commit
execution_status=observed_commit_passed
run_id / commit_sha / quality report digest
source_worktree_state=clean
isolated_worktree_state=clean
cleanup_status=removed
```

正式命令：

```powershell
cargo run -p quality_gate --locked -- local-ci --commit HEAD
```

### 9.4 完成后文档动作

只有本地 Gate 与 exact-commit Local CI 都通过后：

```text
生成 阶段完成记录/2026-07-12-Reproducible-Toolchain-CI-Lint-Budget-Gate-v1/00-总览.md
更新 245 状态为已完成。
更新 240：CQ-08 已关闭、施工状态已完成，下一讨论 CQ-07。
更新 00-文档地图、00-阅读顺序、49、54。
更新 施工文档/README、阶段完成记录/README。
本文从 当前/ 移到 已完成/。
```

## 10. Diagnostic Codes

至少固定：

```text
quality_gate.toolchain_mismatch
quality_gate.component_missing
quality_gate.lockfile_changed
quality_gate.workspace_lint_not_inherited
quality_gate.lint_level_lowered
quality_gate.command_failed
quality_gate.command_timed_out
quality_gate.cargo_json_malformed
quality_gate.lint_new
quality_gate.lint_occurrence_exceeded
quality_gate.lint_resolved_prune_required
quality_gate.lint_entry_stale
quality_gate.lint_entry_expired
quality_gate.suppression_unregistered
quality_gate.suppression_stale
quality_gate.ledger_mutated_by_verify
quality_gate.default_workspace_failed
quality_gate.all_features_workspace_failed
quality_gate.report_write_failed
quality_gate.ci_not_observed
quality_gate.ci_commit_mismatch
quality_gate.local_ci_source_dirty
quality_gate.local_ci_commit_mismatch
quality_gate.local_ci_worktree_failed
quality_gate.local_ci_report_missing
quality_gate.local_ci_report_invalid
quality_gate.local_ci_cleanup_failed
```

## 11. 安全、隐私与输出

```text
不保存环境变量值、token、secret。
不保存绝对用户路径。
stdout/stderr bounded；Trace 只保存 sanitized 摘要。
LocalCiRunner 不修改 source/index/branch/commit；GitHub workflow 不请求写权限。
report 写入 target/ 或 CI artifact staging，不写项目资产目录。
ledger/propose 输出通过 Project source patch review，不自动覆盖。
```

## 12. 效率约束

```text
Quality Gate 不进入 Editor/runtime 热路径。
不新增 daemon/watcher。
CI cache 不参与通过真相。
完整矩阵只由显式 verify/CI 触发。
Cargo stage 串行，避免相互锁 target。
每 stage 和总 job 有 deadline。
```

## 13. 实施停止条件

遇到以下情况先回填 245/本文再继续：

```text
1.96.0 无法构建当前 workspace。
Cargo JSON 缺少形成稳定 identity 所需字段。
syn 无法可靠解析实际 suppression form。
LocalCiRunner 或 GitHub workflow 必须复制 command plan 才能工作。
建立 ledger 必须大规模修改业务代码。
需要把 CQ-07 文件规模门禁提前引入。
```

## 14. 施工文档自审

### 14.1 与 245 正式方案一致

通过。Gate A-E 完整覆盖 pinned toolchain、workspace lints、深 Runner、command seam、Cargo JSON、fingerprint ledger、suppression、canonical matrix、report、Local CI 和可选 GitHub Adapter。

### 14.2 外部审查是否完整处理

通过。两份 5.6 审查的 toolchain/CI/lint/default/all-features/hygiene 要求均进入 Gate；审查只要求 CI 与实际 commit evidence，不要求 remote。原远端硬门槛与用户的本地 Git 部署形态冲突，245 正式方案已先行修订；CQ-07 仍按不适用处理。

### 14.3 是否使用过时 220 baseline

没有。Gate A 强制对 post-244 工作树重新采集；220 只作历史严重度证据。

### 14.4 是否吞并 CQ-07

没有。code_hygiene 只作为 evidence 执行，文件规模 recommendation 暂不阻断；不拆业务大文件。

### 14.5 是否可能靠 warning 总数通过

已阻断。Gate C 要求 identity + occurrence，对替换 warning 判定 resolved + new。

### 14.6 是否可能靠 allow 绕过

已阻断。Gate A/C 结构化 inventory allow/expect/cfg_attr，未登记 suppression 失败。

### 14.7 是否形成深 Module

通过。developer caller 只有 verify request/report；authoritative caller 只有 local-ci request/run report。Git revision、detached worktree、child verify、artifact copy、digest 与 cleanup 都藏在 LocalCiRunner implementation；System/Scripted 两个 command Adapter 继续支撑内部 seam 与测试。

### 14.8 测试是否覆盖每个 Gate

通过。Gate A 有真实基线/toolchain/metadata/fmt；B 有 command/report/toolchain；C 有完整 lint/suppression negative matrix；D 新增 source dirty、revision mismatch、report mismatch、cleanup failure 合同；E 有双 workspace 与本地 exact-commit activation evidence。

### 14.9 CI 语义是否诚实

通过。普通 developer verify 与 local exact-commit observed/pass 分开。本地 Git 是正式部署形态，无 remote 不再是阻塞；dirty report 仍不能冒充 CI。

### 14.10 是否干扰 244 或其它用户改动

不干扰。244 已归档，本文禁止修改其代码和文档。工作树已有大量用户施工成果，所有改动只在 245 工程治理范围内增量进行，不 reset/revert。

### 14.11 是否可以直接施工

可以。本文仍是唯一当前施工文档；用户已确认方案 A，正式方案先于本文完成修订，Gate A-C 既有证据保持有效，新增范围只在 245 工程治理层。结论：`通过，直接恢复 Gate D LocalCiRunner；失败先修 Gate D，不提前提交或归档。`

## 15. 施工执行记录

### 15.1 Gate A - 已完成

```text
rustup versioned selector：1.96.0-x86_64-pc-windows-msvc 已真实安装。
rustc：1.96.0 (ac68faa20 2026-05-25)。
cargo：1.96.0 (30a34c682 2026-05-25)。
clippy：0.1.96 (ac68faa20c 2026-05-25)。
rustfmt：1.9.0-stable (ac68faa20c 2026-05-25)。
workspace packages：20；lint inheritance：20/20。
post-244 baseline：312 warning occurrences / 139 fingerprint identities。
source suppression：8 occurrences / 7 identities。
ledger：0 absolute paths、0 embedded newlines、0 missing audit fields。
```

历史 `220` 未作为当前 baseline；初始 PowerShell 候选因 canonical newline 和文本搬运问题被废弃，最终 committed ledger 由 Rust `propose-ledger` 生成并与候选 SHA-256 完全一致。

### 15.2 Gate B - 已完成

```text
QualityGateRunner::verify(request) -> QualityGateReport 已落地。
SystemQualityCommandExecutor / ScriptedQualityCommandExecutor 两个 Adapter 已落地。
child stdout/stderr 并发 drain、16 MiB bounded head+tail capture、timeout kill+wait 已测试。
quality-gate-report.v1 已绑定 commit、Cargo.lock、toolchain、manifest set 和 ledger digest。
Summary/Trace 不保存 secret；失败 Trace 脱敏 workspace/home/temp 路径。
失败 Summary 在最多 6 行 / 1024 字符预算内同时保留 stdout 测试失败与 stderr Cargo 上下文，不再因 stderr 尾部挤占而丢失具体失败测试名。
```

Windows 自托管 Runner 运行 workspace tests 时曾触发正在运行的 `quality_gate.exe` 文件锁；已通过两个测试 stage 共用隔离 `CARGO_TARGET_DIR=target/quality-gate/workspace-tests-target` 修复，正式 Cargo 命令矩阵保持不变。

### 15.3 Gate C - 已完成

```text
Cargo JSON 只接受 compiler-message warning。
fingerprint 忽略行号和 slash style，保留 lint/path/message/source anchor 语义。
known/new/resolved/stale/occurrence exceeded 全部 fail-closed 对账。
syn 扫描 allow/expect/cfg_attr，覆盖 rust workspace 与 samples RuntimeModule 源码。
propose-ledger 只写 target/quality-gate/candidate/，测试证明 committed ledger 不变。
```

负向矩阵已覆盖 new、same-total replacement、occurrence growth、resolved、expired、anchor changed、malformed JSON、unregistered suppression、timeout/reap、report roundtrip/path sanitization。

### 15.4 Gate D - 已完成

```text
.github/workflows/rust-quality.yml 已配置。
permissions：contents: read。
checkout：persist-credentials=false。
runner：windows-latest；job timeout=90 minutes；concurrency cancel-in-progress=true。
workflow 只调用 cargo run -p quality_gate --locked -- verify。
workflow 不复制 fmt/clippy/test，不使用 secret/write/publish/deploy。
report summary 与 artifact upload 已配置。
```

`ci_contract` 测试已证明薄 Adapter 合同。

方案 A 修订已完成：

```text
LocalCiRunner interface 只接收 workspace_root + revision，并返回 local-ci-run-report.v1。
clean source 检查、revision/HEAD commit match、detached worktree、唯一 verify child command 已实现。
isolated report 必须同时匹配 clean、passed、adapter、adapter_configured、execution_scope、run_id 与 commit SHA。
quality report copy、SHA-256 digest、bounded worktree cleanup 已实现；cleanup failure fail closed。
Local CI Cargo build target 位于 source workspace ignored target；workspace test 与嵌套 Project Player build/executable resolution 共享标准 CARGO_TARGET_DIR，detached worktree 不承载深 build cache。
source dirty、revision mismatch、report mismatch、cleanup failure、canonical child command 均有测试。
```

Gate D 定向验证：

```text
cargo fmt --all -- --check：通过。
cargo test -p quality_gate：32 unit + 1 ci_contract + 4 lint_budget_matrix，全部通过。
cargo clippy -p quality_gate --all-targets --all-features --locked -- -D warnings：通过。
cargo run -p quality_gate --locked -- local-ci --commit HEAD（dirty source）：按预期失败。
诊断码：quality_gate.local_ci_source_dirty。
source_workspace_state=dirty；isolated_workspace_state=not_created；cleanup_status=not_required。
git worktree list 只有主工作树，无临时 worktree registration。
```

### 15.5 Gate E - developer verify 与 Local CI activation 已完成

最终 `target/quality-gate/quality-gate-report.v1.json`：

```text
schema_version=quality-gate-report.v1
passed=true
workspace_state=dirty
workspace_commit_sha=7cf2f6465b00a90cd1d4c228d2198d75cf74e723
stages=11 passed / 0 failed
lint=312 observed / 139 known / 0 new / 0 resolved / 0 stale
suppression=8 observed / 0 new
default_workspace_passed=true
all_features_workspace_passed=true
hygiene_evidence_generated=true
adapter=developer-worktree
execution_status=not_authoritative
diagnostics=0
```

独立回归：

```text
cargo fmt --all -- --check：通过。
cargo test -p quality_gate：32 unit + 1 ci_contract + 4 lint_budget_matrix，全部通过。
cargo test -p project_e2e_gate --lib：50 tests，全部通过；并行压力运行 3 轮、150/150 通过。
cargo test -p editor_core --lib：395 passed / 1 ignored；header cancellation 定向压力 5/5 通过。
cargo test --workspace --locked：通过。
cargo test --workspace --all-features --locked：通过。
cargo clippy -p quality_gate --all-targets --all-features --locked -- -D warnings：通过。
```

整体回归曾暴露两处 243 测试夹具时序竞态，均未修改生产 Runtime/Editor 行为：

```text
RepairSecondRequestStall 原先固定等待 1 秒，并行负载下会在 cancel 前自行返回；现改为第二请求到达后等待客户端断开，并有 2 秒失败清理上限。
llm_transport_cancellation_interrupts_real_header_wait 原先启动 40ms 后猜测连接已建立，cancel 先于 connect 时 server accept + join 可永久阻塞；现改为 accept handshake 后取消，并给 accept 2 秒 deadline。
```

方案 A Gate E 重跑又暴露两处测试 fixture 并发隔离缺口，修复仍未修改生产 Runtime/Editor 行为：

```text
editor_core RuntimePackage fixture 原先直接位于系统临时目录，所有并行 fixture 的 GameView report 都落到同一个 Temp/reports/editor-gameview-present-report.json；现改为每个 fixture 使用唯一拥有根，并新增报告根隔离回归测试。
editor_window_winit project launcher fixture 原先只用 Windows 系统时间纳秒值生成目录；64 路并发压力可在第 3 轮复现 63/64 唯一路径。现加入 PID + 进程内 AtomicU64 sequence，并新增 64 路并发唯一性测试；修复后 50 轮、4500 tests 全通过。
```

修复后真实 Gate：

```text
Trace：11/11 stages passed；lint 312/139/0 new/0 resolved/0 stale；suppression 8/0 new；passed=true。
Summary：11/11 stages passed；lint_items=[]；所有 stage trace=null；无绝对 workspace/user path；passed=true。
default_workspace_passed=true；all_features_workspace_passed=true；hygiene_evidence_generated=true；diagnostics=0。
candidate 与 committed ledger SHA-256 同为 5bb144159c0e74447acc405d4e7203840b2bf2b0c54ee2b3bb6bc65b7e886bd1；五类 diff 均为 0。
```

原“必须取得远端 provider run”的判断已由用户确认本地 Git 部署形态后推翻；5.6 审查只要求自动化 CI 与实际 commit evidence，并未要求 remote。LocalCiRunner 与 dirty-source fail-closed evidence 已完成；剩余条件是完整回归、提交当前完整本地施工快照、运行 `local-ci --commit HEAD` 并核对 artifact。

首次 exact-commit Local CI（commit `3333d62e36abd640efed7259f918f7d4bd79c7e5`）按合同失败并保留证据：

```text
source_workspace_state=clean；isolated_workspace_state=clean；commit/run_id/adapter 均匹配。
default/all-features 均在 6 个 Project Player / export / release 测试失败。
根因：QualityGate workspace tests 使用非默认 CARGO_TARGET_DIR，但 Project Player executable path 硬编码 rust/target/debug；主工作树旧二进制掩盖，clean worktree 首次构建必现。
cleanup_status=failed：Git registration 已移除，但 worktree 内深层 Cargo target 物理删除返回 255；残留目录已在绝对 TEMP 路径校验后删除。
修复：统一 workspace target/executable 解析，传播 CARGO_TARGET_DIR；Local CI build cache 移到 source ignored target，使 detached worktree cleanup 不再承担深 target 删除。
定向验证：非默认 CARGO_TARGET_DIR 下 Project Player build + descriptor inspect 通过；quality_gate 32 unit + 1 ci_contract + 4 lint matrix 通过。
```

后续 exact-commit run 继续按合同 fail closed，并暴露只在 clean/full-matrix 环境可见的测试合同缺口：

```text
release_package.rs 与 pe_contract.rs 的 Project Player 路径复制了 rust/target/debug，未继承 CARGO_TARGET_DIR；已统一到 ProjectPlayerArtifact::debug_executable_path。
ReleasePackage 内部测试在首次构建 Player 前截取源 executable 基线；已改为先 ensure_built，再验证打包和 PE stamp 不反向修改源 Player。
quality_gate workspace target 单测读取 ambient CARGO_TARGET_DIR，嵌套 workspace test 时形成环境依赖；已改为显式 resolver 参数矩阵。
GitHub workflow contract 只接受 LF，clean checkout 的 CRLF 触发假失败；已先规范化 CRLF/LF，再执行相同只读、薄 Adapter 断言。
```

最终 authoritative run：

```text
run_id=local-eefa5d26955c-1783844809
commit_sha=eefa5d26955ca76bdacd37970ac8ff9939348cfd
source_workspace_state=clean
isolated_workspace_state=clean
quality_gate stages=11 passed / 0 failed
adapter=local-git-worktree
execution_scope=local_commit
execution_status=observed_commit_passed
quality_gate_report_digest=sha256:d39fb262a195b611cf37e10c6138611decc9d20228655f1e33f3f737b5854593
cleanup_status=removed
diagnostics=0
passed=true
```

artifact 已保存到主工作区 ignored 路径 `rust/target/quality-gate/local-ci/local-eefa5d26955c-1783844809/`；完成记录固定 commit、run id 与 digest，不依赖该缓存目录长期存在。

施工状态结论：`Gate A-E、developer 回归、exact-commit Local CI、完成记录和入口同步均已完成；CQ-08 关闭，本文归档到 施工文档/已完成/。`
