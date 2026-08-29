# 245-Reproducible Toolchain / CI / Lint Budget Gate v1 方案

> 状态：已完成；方案 A 本地 exact-commit CI 已取得权威通过证据并归档。
> 建立日期：2026-07-12。  
> 选题来源：`240-5.6审查剩余问题讨论与施工优先级.md` Priority 5 / CQ-08。  
> 审查输入：`审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md`、`01-2026-07-11-新增功能增量代码质量审查报告.md`。  
> 用户确认：采用方案 C，并于 2026-07-12 确认方案 A 本地 Git 修订：本地 exact-commit CI 是 authoritative Adapter，GitHub Actions 为可选 Adapter。  
> 前置状态：244/CQ-06 已完成施工、双 workspace 回归与归档；245 已完成并关闭 CQ-08。
> 目标：让开发机与 CI 使用同一工具链、同一验证入口和同一 lint 债务合同；允许已登记历史债务暂时存在，但任何新增、替换、失效或无审计 suppression 都必须 fail closed。

## 1. 这个系统解决什么问题

当前项目有大量真实测试，但工程质量仍依赖执行者记住正确命令和本机环境：

```text
没有 rust-toolchain.toml
  -> 不同机器可能使用不同 rustc / Cargo / rustfmt / Clippy

没有统一 Quality Gate
  -> fmt、default、all-features、Clippy、hygiene 由人工分别执行

没有 CI carrier
  -> 本地 Git commit 后没有自动、可追溯、隔离执行的集成门禁

Clippy 已有历史 warning
  -> 立即全局 -D warnings 会把 CQ-08 扩大成一次性清债
  -> 只比较 warning 总数又允许“删一条旧 warning，换一条新 warning”
```

目标链路：

```text
committed rust-toolchain.toml
  -> QualityGateRunner::verify
  -> fmt / default workspace / all-features workspace
  -> structured Clippy JSON
  -> fingerprinted debt reconciliation
  -> existing hygiene evidence
  -> quality-gate-report.v1
  -> local caller / thin CI Adapter
  -> fail closed on any contract violation
```

完成后，“本地通过”和“CI 通过”不再是两套近似流程，而是同一个深 Module 的两个 caller。

## 2. 当前代码与工程基线

### 2.1 已确认事实

```text
当前本机 rustc：1.96.0 (ac68faa20 2026-05-25)
当前本机 cargo：1.96.0 (30a34c682 2026-05-25)
当前 active toolchain：stable-x86_64-pc-windows-msvc
仓库：没有 rust-toolchain.toml / rust-toolchain
施工前仓库：没有 .github/workflows 或其它 CI workflow
rust/Cargo.toml：没有 [workspace.lints]
workspace members：没有统一 [lints] workspace = true
仓库定位：本地 Git 项目；没有 Git remote 是预期部署形态，不是 CI 缺口
```

`rust/crates/code_hygiene` 已提供：

```text
code_hygiene.report.v1
Rust 文件数量与总行数
over_1000 / over_2000 / over_4000
largest_files / hotspots / recommendations
```

但它目前只生成 recommendation，不按绝对上限、相对恶化或 exception 失败。这个缺口属于后续 CQ-07，不在 245 中重写。

### 2.2 5.6 审查快照不能直接当当前 ledger

5.6 增量审查曾记录：

```text
cargo clippy --workspace --all-targets --all-features：220 条 warning
cargo clippy ... -- -D warnings：失败
```

该数字对应 2026-07-11 当时的工作树。239、241、242、243、244 后续已修改大量 Rust 文件，因此：

```text
220 只作为“必须建立 lint budget”的历史证据。
220 不能写成 245 的当前允许数量。
245 ledger 必须在施工 Gate A 基于 244 完成后的稳定工作树重新采集。
```

## 3. 与 240 队列的边界

245 只关闭 CQ-08：

```text
固定 Rust toolchain 与组件。
建立 workspace lint policy。
建立本地与 CI 共用的唯一验证入口。
冻结并指纹化现有 Clippy/rustc warning，禁止新增或置换。
建立结构化 Quality Gate report。
建立薄 CI Adapter 和本地 exact-commit 执行证据合同。
```

245 不关闭 CQ-07：

```text
不制定文件行数绝对上限。
不建立 Module decomposition 数量预算。
不要求本轮拆分 aui.rs、session.rs、report_panel.rs 等热点。
不把 code_hygiene recommendation 改成规模阻断规则。
不要求一次性清零全部历史 Clippy warning。
```

CQ-07 后续继承 151 和 245：245 提供可复现 runner、CI carrier、exception schema 与报告容器；CQ-07 只增加 hygiene budget，不再另造 CI 系统。

## 4. 外部审查结论分类

### 4.1 必须修改

| 审查结论 | 245 处理 |
|---|---|
| 没有固定 Rust toolchain | 新增精确 `rust-toolchain.toml`，固定 1.96.0、rustfmt、clippy、minimal profile |
| 没有统一 workspace lint policy | 根 workspace 定义 policy，所有 member 显式继承 |
| 没有 CI | 建立薄 CI Adapter，调用仓库内唯一 runner |
| 严格 Clippy 失败 | 先建立逐条 debt ledger；new warning fail closed，历史 warning 渐进归零 |
| warning 数量持续增长 | 不按总数放行，按 warning identity 对账，阻止替换式增长 |

### 4.2 施工约束

```text
default workspace 与 all-features workspace 都必须进入阻断矩阵。
本地与 CI 不得复制两套命令列表。
Clippy 必须使用 Cargo JSON，不解析人类可读终端文本。
ledger 更新不得由普通 verify 自动写回。
CI Adapter 不得持有项目 secret 或发布权限。
完成判定必须区分普通 dirty verify 与隔离 local commit run 已观察。
```

### 4.3 已由历史施工吸收

```text
CQ-02 all-features 失败已关闭；245 只把已通过矩阵固化为永久 Gate。
239/241/242/243/244 已恢复 default/all-features workspace；245 不重复其功能施工。
151 已建立 code_hygiene.report.v1；245 只调用，不重写其统计语义。
```

### 4.4 不适用

```text
CQ-07 文件规模与 Module 拆分预算不适用于 245。
CQ-01/CQ-03/CQ-04/CQ-05/CQ-06 与 INC-01/02/03 已关闭，不机械扩入。
Provider、Agent、installer、签名、发布权限和游戏功能与 CQ-08 无关。
```

## 5. 成熟实现参考

### 5.1 Rustup toolchain file

Rustup 官方 `Overrides / The toolchain file` 明确：

```text
rust-toolchain.toml 可提交版本控制。
[toolchain] 可声明 channel、components、targets、profile。
目录内 rustup proxy 会选择该 toolchain。
```

本项目采用：

```toml
[toolchain]
channel = "1.96.0"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

不采用 nightly，也不把 target 列表提前扩大到当前未验证平台。

### 5.2 Cargo workspace lints

Cargo 官方 workspace 文档明确：

```text
[workspace.lints] 在根 workspace 定义 lint 配置。
member 通过 [lints] workspace = true 继承。
lint level 可用 allow / warn / deny / forbid 与 priority 表达。
```

245 使用它统一策略，但初始 policy 必须根据 Gate A 实测建立，不能为了形式上严格而让全部历史 warning 立即变成编译错误。

### 5.3 Clippy CI

Clippy 官方建议：

```text
CI 使用 -D warnings 阻止 warning 通过。
Clippy 与项目编译使用相同 toolchain。
```

本项目接受最终目标，但采用迁移合同：

```text
未登记 warning 的效果等价于 -D warnings。
已登记历史 warning 暂时进入 debt ledger。
ledger 清零后切换为原生全局 -D warnings。
```

### 5.4 Bevy 源码做法

参考：

```text
bevy/.github/workflows/ci.yml
bevy/tools/ci/src/main.rs
```

Bevy workflow 使用 `cargo run -p ci -- test/lints/compile/doc`，把真实命令放在仓库内 runner；workflow 只做 checkout、工具链、依赖、缓存、矩阵和超时编排。

可学习点：

```text
本地和 CI 调同一个 runner。
RUSTFLAGS=-D warnings 作为成熟终态。
workflow/action 固定版本或 commit。
job 有 timeout 与 concurrency policy。
```

不照搬点：

```text
不复制 Bevy 的 nightly、Miri、no_std、WASM、doc、typos 全矩阵。
不把大型引擎的全部平台成本一次引入 245。
当前主产品是 Windows，第一版 authoritative full Gate 以 Windows 为准。
```

### 5.5 GitHub Actions 官方 Rust workflow

GitHub 官方建议 workflow 使用与本地相同的 Cargo build/test 命令，并可按 OS、`Cargo.lock` 和工具链建立缓存。

245 把 GitHub Actions 作为第一版 CI Adapter，但不把 GitHub YAML 当质量规则真相。若未来迁移 CI host，只替换 Adapter，不改 `QualityGateRunner` interface 和 ledger/report schema。

## 6. 方案比较与正式选择

### 6.1 方案 A：立即全局严格清零

```text
rust-toolchain.toml
workspace lints
cargo clippy ... -- -D warnings
所有现有 warning 一次修完
```

不采用。它表面简单，但会把 CQ-08 扩大成跨 workspace 的历史清债和 Module 重构，破坏 240 对 CQ-08/CQ-07 的拆分。

### 6.2 方案 B：warning 总数冻结

```text
记录 warning_count
actual_count <= baseline_count 即通过
```

不采用。它允许删除一个旧 warning 后引入一个完全不同的新 warning，也允许不同 lint category 互相置换，无法证明“没有新增”。

### 6.3 方案 C：固定工具链 + 深 Runner + 指纹债务账本 + 薄 CI Adapter

正式采用。

```text
精确工具链保证诊断集合可复现。
QualityGateRunner 形成唯一验证 interface。
Cargo JSON 形成结构化 lint evidence。
fingerprint ledger 区分 known / new / resolved / stale。
thin CI Adapter 只调用相同 runner。
```

它既不放任 warning 增长，也不要求第一轮把 CQ-07 债务全部清零。

## 7. 深 QualityGateRunner Module

### 7.1 外部 interface

核心 interface 保持小：

```rust
pub struct QualityGateRunner<E: QualityCommandExecutor> { /* private */ }

impl<E: QualityCommandExecutor> QualityGateRunner<E> {
    pub fn verify(
        &self,
        request: QualityGateRequest,
    ) -> QualityGateReport;
}
```

正式 CLI 只有一个阻断入口：

```powershell
cargo run -p quality_gate -- verify
```

`QualityGateRequest` 只包含：

```text
workspace_root
report_mode：Summary | Trace
report_output（可选，必须位于 target/ 或 CI artifact staging）
```

caller 不传入命令列表、warning limit、lint level 或 ledger override。所有合同来自版本控制内配置。

### 7.2 implementation 隐藏内容

```text
toolchain identity 验证
command plan 与 timeout
Cargo child process lifecycle
Cargo JSON line parsing
diagnostic normalization 与 SHA-256 fingerprint
ledger/suppression reconciliation
known/new/resolved/stale 分类
default/all-features matrix
hygiene evidence 聚合
report schema 与 exit decision
```

删除该 Module 会让命令、超时、解析、budget 和报告重新散回 PowerShell、workflow 和人工说明，因此它提供真实 leverage 与 locality。

### 7.3 command seam 与 Adapter

```rust
pub trait QualityCommandExecutor {
    fn execute(&self, spec: &QualityCommandSpec) -> QualityCommandOutcome;
}
```

Adapter：

```text
SystemQualityCommandExecutor：production，本地与 CI 共用。
ScriptedQualityCommandExecutor：test，注入 exit、stdout/stderr JSON、timeout 和 malformed output。
```

该 seam 只在 `quality_gate` crate 内部可见，不扩散到 engine/runtime/editor。

## 8. 固定工具链合同

### 8.1 文件真相

根目录新增：

```text
rust/rust-toolchain.toml
```

正式值：

```toml
[toolchain]
channel = "1.96.0"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

`Cargo.lock` 继续作为依赖解析真相，阻断命令统一使用 `--locked`。

### 8.2 identity 验证

Runner 在执行矩阵前采集：

```text
rustc -Vv
cargo -V
cargo clippy -V
rustfmt -V
host triple
```

报告至少保存版本、commit hash、host 和匹配结果。版本或组件缺失时立即 fail closed，不继续产生不可比较的 lint ledger 结果。

### 8.3 升级流程

升级工具链必须是显式工程变更：

```text
修改 rust-toolchain.toml
生成新旧 toolchain diagnostic diff report
分类新增/消失/变化 lint
提交 ledger migration candidate
default/all-features/CI 全部通过
人工确认后落盘
```

普通 verify 不自动升级，也不自动接受新 lint。

## 9. Workspace Lint Policy

根 `rust/Cargo.toml` 增加 `[workspace.lints]`，全部 workspace member 增加：

```toml
[lints]
workspace = true
```

初始 policy 原则：

```text
明确声明 rust 与 clippy 基础 lint group。
已经零债务且属于 correctness 的规则可 deny。
存在历史债务的规则保持 warn，由 ledger 阻断 new warning。
禁止 package 私自降低 workspace lint level。
禁止用 crate-wide allow 绕过 ledger。
```

Gate A 必须先用固定 1.96.0 扫描真实诊断，再确定具体 lint table。正式施工文档必须列出每个 deny/warn group 和 baseline 证据，不允许凭方案文本猜测当前可 deny 的 lint。

## 10. Lint Debt Ledger

### 10.1 schema

新增版本控制文件：

```text
rust/quality/lint-debt-ledger.v1.json
schema_version = lint-debt-ledger.v1
```

逻辑结构：

```json
{
  "schema_version": "lint-debt-ledger.v1",
  "toolchain": "1.96.0",
  "entries": [
    {
      "id": "lint-debt-0001",
      "fingerprint": "sha256:...",
      "lint_code": "clippy::too_many_arguments",
      "relative_path": "crates/example/src/lib.rs",
      "anchor_hash": "sha256:...",
      "allowed_occurrences": 1,
      "origin": "post-244-baseline",
      "reason": "legacy debt; decomposition deferred to CQ-07",
      "owner": "engine-maintainers",
      "review_by": "2026-10-01"
    }
  ],
  "source_suppressions": []
}
```

`review_by` 到期默认失败，必须删除、续期并给新理由，或转入 CQ-07 施工。没有永久、无 owner、无原因的 exception。

### 10.2 fingerprint canonical tuple

Clippy 使用：

```powershell
cargo clippy --workspace --all-targets --all-features --locked --message-format=json
```

只读取 Cargo JSON 中：

```text
reason = compiler-message
message.level = warning
message.code.code
primary span file_name
primary span text
diagnostic message
```

fingerprint 输入：

```text
tool = rustc | clippy
lint_code
workspace-relative normalized path
normalized diagnostic message
normalized primary source anchor
```

明确排除：

```text
绝对路径
行号/列号
ANSI color
编译耗时
target 临时目录
机器用户名
```

这样普通上下移动不会破坏 identity，而 lint 类型、文件、消息或源 anchor 的实质变化会成为新 diagnostic。

同一 fingerprint 多次出现时按 `allowed_occurrences` 对账；实际数量更大仍算 new debt。

### 10.3 reconciliation

每次 verify 分类：

```text
known：fingerprint 与 occurrence 均在 ledger 内。
new：没有 ledger entry，或 occurrence 超额；Gate 失败。
resolved：ledger entry 当前不再出现；Gate 失败并要求收缩 ledger。
stale：路径不存在、anchor 不存在、toolchain 不匹配或 review_by 到期；Gate 失败。
changed：同路径/同 lint 但 identity 改变；作为 resolved + new，不隐式迁移。
```

要求 resolved 也失败，是为了避免 ledger 永久膨胀并掩盖以后重新引入同类问题。

### 10.4 baseline 更新不是普通 verify

允许单独命令：

```powershell
cargo run -p quality_gate -- propose-ledger
```

它只能在 `target/quality-gate/candidate/` 生成候选 ledger 与 diff report：

```text
added diagnostics
removed diagnostics
changed diagnostics
new suppressions
expired entries
```

不得直接覆盖版本控制内 ledger。AI/开发者必须审查候选并通过正常 patch 落盘。

## 11. Source Suppression Contract

仅对 emitted warning 做 ledger 不足以阻止新增：

```rust
#[allow(clippy::some_lint)]
```

会让 diagnostic 根本不出现。因此 Runner 必须扫描 workspace Rust source 的 lint attributes：

```text
allow
expect
cfg_attr(..., allow/expect(...))
crate-level lint attribute
```

解析必须使用 Rust syntax parser，不允许 regex 作为正式真相。

每个允许的 suppression 必须在 `source_suppressions` 登记：

```text
stable id
lint code/group
relative path
source anchor hash
reason
owner
review_by
```

未登记、过期、路径失效、anchor 失效或 occurrence 增加均失败。Gate A 对现有少量 `#[allow(dead_code)]`、`#[allow(clippy::too_many_arguments)]` 做一次 inventory，不自动判定它们合理。

## 12. 唯一验证矩阵

`QualityGateRunner::verify` 固定按以下阶段执行：

| Stage | 命令/行为 | 阻断 |
|---|---|---|
| Toolchain | rustc/cargo/rustfmt/clippy identity | 是 |
| Lock | Cargo.lock 存在，后续统一 `--locked` | 是 |
| Format | `cargo fmt --all -- --check` | 是 |
| Lint policy | workspace inheritance 与 package override audit | 是 |
| Suppression | source attribute inventory vs ledger | 是 |
| Clippy | workspace/all-targets/all-features Cargo JSON | 是 |
| Debt | known/new/resolved/stale reconciliation | 是 |
| Default tests | `cargo test --workspace --locked` | 是 |
| All-features tests | `cargo test --workspace --all-features --locked` | 是 |
| Hygiene evidence | 复用 `code_hygiene.report.v1` | 命令失败阻断；规模 recommendation 暂不阻断 |
| Report | `quality-gate-report.v1` 完整、可序列化 | 是 |

不加入真实公网 provider、真实人工窗口、GPU screenshot 或环境相关 ignored smoke；这些继续保持既有 local-only/optional 合同。

## 13. Quality Gate Report

### 13.1 schema

```text
quality-gate-report.v1
```

字段至少包括：

```text
schema_version
gate_id
started_at / duration_ms
workspace_digest
toolchain expected/actual/matched
host_os / host_triple
lockfile_digest
stages[]：id、command_id、status、exit_code、duration_ms、timeout、next_action
lint：observed/known/new/resolved/stale/suppression counts
lint_items[]：fingerprint、lint_code、relative_path、ledger_id、classification
test_matrix：default/all_features status
hygiene_summary：schema/version/counts/recommendation_count
ci：adapter、execution_scope、run_id、commit_sha、execution_status
diagnostics[]
passed
```

报告不得保存整个源文件、绝对用户路径、环境变量、secret、完整 stdout/stderr。Trace artifact 可保存 bounded、sanitized command diagnostics；Summary 只保存计数与 next action。

### 13.2 workspace digest

为防止把旧 CI run 冒充当前代码，report 必须绑定：

```text
Git commit SHA（有 Git commit 时）
Cargo.lock SHA-256
rust-toolchain.toml SHA-256
workspace Cargo manifest set SHA-256
lint ledger SHA-256
```

脏工作树本地验证可运行，但报告必须显示 `workspace_state=dirty`，不能作为“本地 CI 已验证该 commit”的证据。只有隔离 detached worktree 中 `workspace_state=clean`、commit 完全匹配且 Gate 通过，才形成 authoritative CI evidence。

## 14. CI Adapter

### 14.1 Authoritative Local Git Adapter

正式 interface：

```powershell
cargo run -p quality_gate --locked -- local-ci --commit HEAD
```

`LocalCiRunner::run` implementation 必须隐藏：

```text
确认调用仓库 worktree clean。
把请求 revision resolve 为本地 commit SHA，并要求等于当前 HEAD。
在系统临时目录创建 detached Git worktree。
从该 commit 的 rust workspace 调用唯一 QualityGateRunner verify interface。
Local CI 把 Cargo build target 放在 source workspace 的 ignored `target/quality-gate/local-ci-build/`；detached worktree 不承载深层 build cache。
QualityGateRunner 向 Cargo child 传播标准 `CARGO_TARGET_DIR`，workspace test 使用其下独立 `workspace-tests-target`；嵌套 Project Player build 与 executable resolution 必须遵守同一 target。
要求隔离 report workspace_state=clean、commit_sha 一致、passed=true。
把 quality-gate-report.v1 与 local-ci-run-report.v1 复制到原 workspace target/quality-gate/local-ci/<run_id>/。
记录 report SHA-256、run_id、commit_sha、cleanup status。
无论成功失败都有限时移除临时 worktree；不得留下 child 或 worktree registration。
```

本地 CI 不复制 fmt/test/clippy 命令，不修改 source、ledger、index、branch 或 commit。普通 `verify` 与 `local-ci` 的差别只在执行载体和 commit evidence。

### 14.2 可选 GitHub Adapter

预期新增：

```text
.github/workflows/rust-quality.yml
```

workflow 只负责：

```text
checkout，persist-credentials=false
最小 contents:read permission
使用 committed rust-toolchain.toml
按 Cargo.lock/toolchain/OS 建 cache key
Windows authoritative runner
job timeout
concurrency cancel-in-progress
调用 cargo run -p quality_gate -- verify
上传 quality-gate-report.v1 artifact
把 Summary 写入 CI job summary
```

workflow 不复制 fmt/test/clippy 命令，不拥有 lint baseline，不自动更新 ledger，不需要 repository secret。

GitHub Actions 继续作为第二个可选 Adapter，证明同一 interface 可跨 carrier；本地 Git 项目不要求配置 remote，也不要求观察 GitHub run。workflow configured 只表示可选 Adapter 合同有效，不参与 CQ-08 完成判定。

### 14.3 CI evidence 诚实状态

```text
普通开发验证：adapter=developer-worktree，execution_scope=dirty_or_clean_worktree，execution_status=not_authoritative。
本地正式 CI：adapter=local-git-worktree，execution_scope=local_commit，execution_status=observed_commit_passed|observed_commit_failed。
可选 GitHub：adapter=github-actions，execution_scope=remote_commit，execution_status=observed_commit_passed|observed_commit_failed。
```

CQ-08 以本地正式 CI 的 commit-bound artifact 为完成证据；无 remote 不再构成阻塞。

### 14.4 后续 CI 迁移

如果未来不是 GitHub：

```text
保留 rust-toolchain.toml
保留 QualityGateRunner
保留 ledger/report schema
只替换 carrier Adapter
```

不新增第二套验证命令。

## 15. 性能与并发约束

```text
Clippy、default tests、all-features tests 是完整 CI profile，不进入 Editor/runtime 热路径。
本轮不新增常驻 daemon、文件 watcher 或 Editor Report Panel provider。
CI 使用 concurrency key 取消同 branch 旧 run。
每个 stage 有独立 timeout，总 job 有上限。
缓存只是加速，不参与通过真相；cache miss 必须仍能完成。
Runner 不并发启动两个会争用同一 Cargo target lock 的 workspace Cargo 命令。
```

本地运行允许用户显式启动；不在每次 Editor save 后自动跑全 workspace。

## 16. Fail-closed 合同

以下任一情况必须失败：

```text
toolchain/version/component 不匹配
Cargo.lock 会被修改或 --locked 失败
任一 workspace member 未继承 lint policy
package 私下降 lint level
Clippy JSON malformed、truncated 或 command timeout
出现 new warning 或 occurrence 超额
出现 resolved/stale/expired ledger entry
出现未登记 source suppression
default 或 all-features workspace 失败
report 无法生成或 schema 不完整
CI artifact 与 commit/digest 不匹配
```

禁止以下“通过”方式：

```text
只比较 warning 总数
把 stderr 文本 grep 成计数
在 workflow 加 continue-on-error
普通 verify 自动接受当前 warning
新增 crate-level allow(warnings)
跳过 all-features
CI 失败但本地通过就标记 CQ-08 已关闭
```

## 17. 预计文件范围

正式施工预计只涉及工程治理层：

```text
rust/rust-toolchain.toml
rust/Cargo.toml
rust/*/Cargo.toml workspace member lint inheritance
rust/crates/quality_gate/Cargo.toml
rust/crates/quality_gate/src/lib.rs
rust/crates/quality_gate/src/main.rs
rust/crates/quality_gate/src/{toolchain,command_plan,cargo_json,lint_ledger,suppression,report}.rs
rust/crates/quality_gate/tests/*
rust/quality/lint-debt-ledger.v1.json
.github/workflows/rust-quality.yml
245 施工文档、完成记录和入口摘要
```

不修改：

```text
engine_runtime gameplay/runtime behavior
editor_core authoring behavior
RuntimePackage schema
ProjectPatch/AUI/Renderer/Player 业务逻辑
244 方案、施工归档或完成记录
```

若施工发现必须修改业务代码才能建立 ledger，默认记录历史 debt，不在 245 中顺手重构；只有 correctness lint 证明真实 bug 时，先回填方案/施工文档再决定是否扩大。

## 18. 高层 Gate 计划

本节固定方案验收顺序，不替代后续唯一施工文档。

### Gate A：Post-244 Baseline / Toolchain / Workspace Policy

```text
确认 244 已归档且工作树基线稳定。
固定 1.96.0 + rustfmt + clippy。
采集当前 Cargo JSON lint 与 suppression inventory。
建立 workspace lint inheritance。
生成初始 ledger candidate，人工审查后落盘。
```

### Gate B：Deep QualityGateRunner / Structured Report

```text
建立 verify interface、System/Scripted Adapter。
建立 command plan、timeout、sanitized outcome。
建立 quality-gate-report.v1。
验证 malformed output、timeout、non-zero、report failure 均 fail closed。
```

### Gate C：Fingerprint Ledger / Suppression Guard

```text
解析真实 Cargo JSON compiler-message。
验证 path/line move 稳定性与 source semantic change 敏感性。
覆盖 known/new/resolved/stale/changed/occurrence matrix。
覆盖 allow/expect/cfg_attr inventory 与 expiry。
propose-ledger 只生成 candidate，不自动 apply。
```

### Gate D：Canonical Matrix / Thin CI Adapter

```text
Runner 串行执行 fmt、lint、default、all-features、hygiene evidence。
LocalCiRunner 只在隔离 local commit worktree 调用 verify；GitHub workflow 仍只调用 verify。
clean source、timeout、cleanup、commit match、artifact 合同固定。
非默认 CARGO_TARGET_DIR、嵌套 Project Player build/executable match 与 Windows 无深 target worktree cleanup 合同固定。
本地与 CI report schema/digest 一致。
```

### Gate E：Real Gate / Regression / Activation Evidence

```text
固定 toolchain clean environment 验证。
default/all-features workspace 全部通过。
注入新 warning、替换 warning、删除旧 warning、增加 suppression 均被阻断。
提交本地集成 commit，取得 `local-ci --commit HEAD` run 与 artifact。
生成完成记录、入口同步和施工归档。
```

## 19. 测试矩阵要求

后续施工文档至少覆盖：

| 场景 | 预期 |
|---|---|
| exact toolchain | pass |
| wrong rustc/clippy | fail before lint reconciliation |
| known warning unchanged | pass lint budget |
| new warning | fail |
| old warning removed | fail with prune-ledger action |
| old warning replaced by new warning，总数不变 | fail |
| same warning line number moved | identity remains stable |
| source anchor semantic change | changed = resolved + new |
| occurrence 1 -> 2 | fail |
| expired ledger entry | fail |
| unregistered `#[allow]` | fail |
| registered suppression unchanged | pass |
| malformed Cargo JSON | fail |
| Clippy timeout/non-zero | fail |
| default workspace fail | fail |
| all-features fail | fail |
| hygiene recommendation exists | report evidence；CQ-08 不按规模失败 |
| local CI source worktree dirty | fail before detached worktree creation |
| local CI requested commit != current HEAD | fail evidence validation |
| local CI report commit mismatch | fail evidence validation |
| local CI worktree cleanup failure | fail and preserve actionable diagnostic |

所有测试使用临时 workspace 或 `ScriptedQualityCommandExecutor`；不得为了测 fail path 修改真实 workspace ledger。

## 20. 完成判定

CQ-08 只有同时满足以下条件才可关闭：

```text
rust-toolchain.toml 固定并在 clean environment 生效。
所有 workspace member 继承统一 lint policy。
QualityGateRunner 成为本地/CI 唯一入口。
初始 ledger 基于 244 后真实工作树并经过人工审查。
new/resolved/stale/suppression negative matrix 全通过。
default 与 all-features workspace 通过。
quality-gate-report.v1 可审查且不泄漏绝对路径/secret。
LocalCiRunner 与可选 GitHub thin Adapter 已配置。
本地 exact-commit CI 在隔离 clean worktree 运行并通过，artifact digest 一致。
完成记录、240/49/54/README 同步和施工文档归档完成。
```

## 21. 回滚与演进

```text
Runner/ledger/report 都是工程治理层，不进入 Runtime/Player 产物。
LocalCiRunner 回滚不影响游戏运行，但会失去 authoritative 本地门禁，因此必须显式记录；GitHub workflow 回滚只移除可选 carrier。
工具链升级通过 ledger migration，不覆盖旧 evidence。
ledger 清零后删除兼容路径，直接使用原生 -D warnings。
CQ-07 在同一 report/runner 上增加 hygiene budget，不新增 quality_gate_v2 平行系统。
```

## 22. 方案自审

### 22.1 是否符合用户确认的方案 C

通过。正式方案包含 pinned toolchain、深 `QualityGateRunner`、Cargo JSON warning fingerprint、debt/suppression ledger、统一矩阵、结构化 report 和薄 CI Adapter，没有缩成 warning 总数比较。

### 22.2 是否干扰 244

不干扰。244 已在本方案落盘前完成施工与归档；245 不修改 244 方案、施工归档或完成记录。后续 245 施工严格受唯一施工文档约束，不回开或扩写 244。

### 22.3 是否错误使用 220 作为当前基线

没有。220 明确只是 5.6 历史快照。正式 ledger 必须在施工 Gate A 用固定 1.96.0 对 post-244 工作树重新生成并审查。

### 22.4 是否形成深 Module

通过。caller 只学习 `verify(request) -> report`；命令计划、Cargo JSON、fingerprint、ledger、timeout、matrix 和 report 都在 implementation 内。System/Scripted 两个 Adapter 使 command seam 真实且可测试。

### 22.5 是否可能被总数置换绕过

已阻断。Gate 按指纹与 occurrence 对账；同总数的新 warning 会形成 resolved + new 并失败。

### 22.6 是否可能被 allow 绕过

已阻断。Rust source suppression 独立 inventory，未登记或过期 suppression 失败；ledger 不只依赖 emitted diagnostics。

### 22.7 是否吞并 CQ-07

没有。245 只执行现有 hygiene report 并保存 evidence，不建立文件规模阻断预算、不拆热点 Module。CQ-07 继续是 240 Priority 6。

### 22.8 CI 语义是否诚实

通过。普通 developer verify 不等于 commit-bound CI。只有 local-ci 在隔离 clean worktree 对当前 HEAD 执行并产出匹配 artifact，才能记录 observed_commit_passed。

### 22.9 是否把 GitHub 绑定进核心

没有。LocalCiRunner 是本地 Git authoritative Adapter，GitHub Actions 是可选第二 Adapter；工具链、runner、ledger 和 report 都是仓库内平台无关真相。

### 22.10 外部审查是否完整处理

通过。固定工具链、CI、workspace lint、default/all-features、Clippy budget 和 hygiene evidence 均有明确处理；已关闭 CQ 项和 CQ-07 重构预算按吸收/不适用分类，没有机械扩入。

### 22.11 是否可以生成施工文档

设计范围、interface、fingerprint、ledger、suppression、matrix、report、CI 激活和 fail-closed 合同已固定。唯一 245 施工文档已生成并完成自审，后续施工只能按该文档 Gate A-E 串行推进。

方案审查结论：`通过；245 正式方案可以进入后续施工文档流程，但不得从本方案直接开工。`

## 23. 正式结论

CQ-08 正式采用方案 C：

```text
Pinned Rust 1.96.0 Toolchain
  + Workspace Lint Policy
  + Deep QualityGateRunner
  + Fingerprinted Lint Debt Ledger
  + Source Suppression Contract
  + quality-gate-report.v1
  + Thin CI Adapter
```

它的核心不是“把 CI YAML 写出来”，而是建立一个可复现、可审查、不能靠总数或 `allow` 绕过的质量真相。历史债务可以有期限地存在，新债务从 Gate 启用起不再允许进入。

## 24. 下一步

245 讨论完成后，按 `240` 下一讨论项是 Priority 6：

```text
CQ-07 Code Hygiene Regression Blocking / Module Decomposition Budget v2
```

最终 authoritative run 为 `local-eefa5d26955c-1783844809`，绑定 commit `eefa5d26955ca76bdacd37970ac8ff9939348cfd`；`quality-gate-report.v1` 为 11/11 stages passed、`execution_status=observed_commit_passed`，digest 为 `sha256:d39fb262a195b611cf37e10c6138611decc9d20228655f1e33f3f737b5854593`，cleanup 为 `removed`。CQ-08 已关闭；不要求 Git remote。

## 25. 参考

```text
框架设计/引擎总体架构/240-5.6审查剩余问题讨论与施工优先级.md
框架设计/引擎总体架构/151-Codebase-Architecture-Hygiene-Gate-v1方案.md
框架设计/引擎总体架构/244-Diagnostics-first-Scene-Hydration-World-Mutation-Safety-v1方案.md
框架设计/引擎总体架构/阶段完成记录/2026-07-12-Diagnostics-first-Scene-Hydration-World-Mutation-Safety-v1/00-总览.md
审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md
审查目录/5.6审查目录/01-2026-07-11-新增功能增量代码质量审查报告.md

rust/Cargo.toml
rust/crates/code_hygiene/src/lib.rs
rust/crates/code_hygiene/src/main.rs
rust/crates/code_hygiene/tests/hygiene_report.rs

https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file
https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section
https://doc.rust-lang.org/cargo/reference/workspaces.html#the-lints-table
https://doc.rust-lang.org/clippy/continuous_integration/index.html
https://docs.github.com/en/actions/tutorials/build-and-test-code/rust
https://github.com/bevyengine/bevy/blob/main/.github/workflows/ci.yml
https://github.com/bevyengine/bevy/blob/main/tools/ci/src/main.rs
```
