# 254 当前施工文档：R2 Lifecycle Retirement / Core Simplification v1

> 状态：已完成并归档，R2 lifecycle retired / 254 Core simplified
> 编写日期：2026-07-22
> 当前规范：`254-AI-Tool-Gateway-Codex-Adapter-v1方案.md` 第 30 节
> 历史输入：`254-R2-Production-Conformance-Candidate-Lifecycle-Real-Codex-Outcome-Acceptance-v1方案.md`、`254-R2-历史失败归档-2026-07-22.md`
> 当前授权：执行 R2-LR0 至 R2-LR9 的 lifecycle retirement / Core simplification；不授权任何 candidate、activation、attempt、真实 AI acceptance 或历史 Gate

## 1. 施工目标

本施工只完成一件事：把不属于 254 唯一职责的 engine release candidate / real AI outcome acceptance lifecycle 从 active production composition、公共 Interface、Cargo 构建面和活跃测试面中退役，使 254 Core 回到“向 AI 提供好用、自由、可审计的引擎工具”。

完成后必须同时成立：

```text
ProductionCandidateModule::produce 不再是 active/public Interface。
CodexOutcomeEvaluationModule::evaluate 不再是 active/public Interface。
editor_host 不再提供 production-candidate owner/mode。
Native Editor 不再默认组合 outcome evaluation runtime。
Gateway 不再 claim evaluation attempt、terminal seal 或因 attempt terminal 拒绝合法后续调用。
只服务退役 lifecycle 的源码和测试已用 git mv 归档，active source 对 legacy/** 零依赖。
project.mutate.candidate、project.rollback.candidate、Grant、operation、receipt、digest、rollback、Preview、diagnostics 与 Build/Delivery 保持可用。
AI 继续在工具外自由规划、选工具、分支和动态重规划。
```

## 2. 明确不施工的内容

本施工不做：

```text
不修复或重写 ProductionCandidateModule。
不生产第二个 Fresh Candidate，不执行 R2-FC7。
不创建 candidateId、activationId、attemptId 或 real-session acceptance artifact。
不执行 Real Evaluation、F-A、F-B、Gate G 或三引擎 B 通道。
不恢复 R1 freezer、Gate F validator、旧 R2-N8/R2-G 或任何历史 Runner。
不实现 capability-aware Catalog；RuntimeModule/platform/availability-aware Catalog 是后续独立方案。
不新增 RetirementManager、compatibility Module、feature-gated legacy shim 或隐藏 lifecycle Runner。
不改变 ProjectPatch、Controlled SourcePatch、Asset Import 或项目级 Candidate 的语义。
不借退役名义重构无关 Runtime、Editor UI、AUI、Build Graph 或项目逻辑。
```

历史 R2-FC6 的 `source_snapshot.file_set_mismatch` 保持 terminal failed；本施工既不重试，也不把退役解释成该历史运行成功。

## 3. 不变量与名称分离

### 3.1 必须保留的 active Core

```text
Editor-hosted Gateway Core 与 typed MCP / Tool Catalog
project/session binding、read generation、digest drift、reconnect
CapabilityGrant、operation ownership、receipt lineage、rollback authority、journal/evidence/cleanup
inspect/search/read/references/source/evidence
领域深工具
ProjectPatch / Controlled SourcePatch / Formal Asset Import
project.mutate.candidate / project.rollback.candidate
Preview、runtime/UI diagnostics、Build/Delivery
```

### 3.2 必须退役的 production lifecycle

```text
engine source snapshot -> release build -> conformance -> seal/reopen
production candidate registry/identity
activation config/lease/rollback lifecycle
acceptance attempt identity、real-session observation、terminal seal、outcome validation
旧 freezer / Gate F 中仅服务精确引擎版本验收的 active build/test surface
```

### 3.3 两类 Candidate 禁止混淆

`project.mutate.candidate` 是 AI 对用户项目的一次受控修改，属于 active Core；`ProductionCandidateModule` 是精确引擎发布候选生产器，属于退役 lifecycle。文件名、测试名或历史上共享 `candidate` 一词不能作为删除依据，必须按真实 consumer 和行为归类。

## 4. 施工原则

1. 按依赖方向施工：先 inventory 和红测试，后解除 consumer，再归档无 consumer 文件，最后清 Cargo/依赖。
2. 退役范围按行为与 consumer 决定，不按文件名猜测。每个目标文件必须归入 `active Core`、`archive-only` 或 `mixed` 三类之一。
3. `archive-only` 文件与其专属测试用 `git mv` 一起归档；`mixed` 文件先拆出仍有真实 consumer 的 Core 行为，只归档 lifecycle 部分。
4. 具有独立 active consumer 的能力留在真实所有者中，不复制两份 Implementation。
5. 不保留 pass-through shim。删除退役 Module 后，复杂度应消失，而不是转移到 caller 或换一个名字继续存在。
6. 测试通过 active Interface 验证保留能力；不为了继续编译历史代码而扩大 active Interface。

## 5. 当前代码基线与待分类清单

编写时确认的高置信 consumer：

```text
rust/crates/editor_host/src/main.rs
  -> production-candidate 参数与 owner dispatch
rust/crates/editor_host/src/production_candidate_owner.rs
  -> ProductionCandidateModule::production/produce
rust/crates/editor_window_winit/src/application.rs
  -> 默认 CodexOutcomeEvaluationModule composition
rust/crates/ai_tool_gateway/src/editor_host.rs
  -> evaluation runtime composition
rust/crates/ai_tool_gateway/src/core.rs
  -> attempt claim/journal/terminal seal/late-dispatch rejection
rust/crates/ai_tool_gateway/src/lib.rs
  -> lifecycle module declarations与公共 re-export
```

激活后 R2-LR0 必须逐一分类、记录 consumer 和最终动作的文件至少包括：

```text
rust/crates/ai_tool_gateway/src/production_candidate.rs
rust/crates/ai_tool_gateway/src/codex_outcome_evaluation.rs
rust/crates/ai_tool_gateway/src/codex_outcome_runtime.rs
rust/crates/ai_tool_gateway/src/real_session_observer.rs
rust/crates/ai_tool_gateway/src/activation_lease.rs
rust/crates/ai_tool_gateway/src/candidate_preparation.rs
rust/crates/ai_tool_gateway/src/candidate_registry.rs
rust/crates/ai_tool_gateway/src/production_conformance.rs
rust/crates/ai_tool_gateway/src/production_failure.rs
rust/crates/ai_tool_gateway/src/lifecycle_identity.rs
rust/crates/ai_tool_gateway/src/release_package.rs
rust/crates/ai_tool_gateway/src/source_snapshot.rs
rust/crates/ai_tool_gateway/src/acceptance_artifact.rs
rust/crates/ai_tool_gateway/src/candidate_freeze_production.rs
rust/crates/ai_tool_gateway/src/candidate_freezer.rs
rust/crates/ai_tool_gateway/src/frozen_candidate.rs
rust/crates/ai_tool_gateway/src/gate_f_acceptance.rs
rust/crates/ai_tool_gateway/src/codex_config.rs
rust/crates/ai_tool_gateway/src/bin/ai_engine_gateway_codex_config.rs
rust/crates/editor_host/src/candidate_freeze_preflight.rs
rust/crates/ai_tool_gateway/tests/r2_goal_modules_public_schema.rs
rust/crates/ai_tool_gateway/tests/r2_production_observer_contract.rs
rust/crates/ai_tool_gateway/tests/candidate_freezer.rs
rust/crates/ai_tool_gateway/tests/gate_f_acceptance.rs
rust/crates/editor_host/tests/gateway_process.rs
```

`codex_config.rs`、`lifecycle_identity.rs`、`release_package.rs`、`source_snapshot.rs` 等不得预判为整文件 archive-only；如果仍服务 typed MCP 安装、普通 Build/Delivery、通用 digest 或其它 active consumer，必须保留在其真实所有者中。`gateway_process.rs` 是 mixed 测试文件：生产候选用例退出，但真实 Editor Gateway process smoke 必须保留。

## 6. 废弃源码合同

归档根固定为：

```text
legacy/rust/254-r2-lifecycle/
  README.md
  production-candidate/
  outcome-evaluation/
  shared-lifecycle/
  tests/
```

合同：

1. 归档必须使用 `git mv`，保留原始历史；不允许复制后再留下 active 重复实现。
2. 目录不加入 Cargo workspace，不含可构建 `Cargo.toml`、`build.rs`、feature、shim 或发布清单。
3. active Rust、Cargo dependency、binary、Adapter、测试和生成清单不得 import、include、运行或复制 `legacy/**`。
4. `README.md` 记录每个归档文件的原路径、归档 commit、分类理由、退役原因、R2-FC6 最后终态、正式方案、错误归档与完成记录。
5. R1 遗留文件只有在 R2-LR0 证明其唯一职责也是同一已退役的精确版本验收时，才归入 `shared-lifecycle/`；不能因为 feature 名为 `legacy-r1` 就跳过 consumer 审计。
6. 历史源码不要求继续编译或通过旧测试；Git history 与只读归档共同承担追溯。

## 7. 分 Gate 施工

### R2-LR0：激活前基线与 consumer/source inventory

进入条件：

```text
本文件已通过激活前复核并从 待执行/ 移到 当前/。
54 与施工文档 README 已同步为唯一当前施工。
使用仓库外 fresh clean worktree、exact commit 与隔离 CARGO_TARGET_DIR/TEMP/artifact root。
正式方案第 30 节和 R2 历史定位没有漂移。
```

动作：

- 记录 Git commit/tree、outer/nested worktree clean 状态、Rust/Cargo 版本、磁盘空间和 target/temp/artifact 根。
- 全仓搜索每个 lifecycle type/module/CLI/feature 的声明、re-export、construction、test 和 binary consumer。
- 生成结构化 inventory，逐项标记 `active Core`、`archive-only` 或 `mixed`，并写明证据与拟议动作。
- 建立保留能力 baseline；任何 baseline 失败先收敛，不进入 R2-LR1。

命令：

```powershell
git status --short
git rev-parse HEAD
git rev-parse 'HEAD^{tree}'
cargo metadata --format-version 1 --no-deps --locked
rg -n "ProductionCandidateModule|CodexOutcomeEvaluationModule|production_candidate|codex_outcome|attempt_claim|terminal_seal|legacy-r1" rust/crates
cargo test -p ai_tool_gateway gateway_session_operation_ownership
cargo test -p ai_tool_gateway gateway_approval_lifecycle
cargo test -p ai_tool_gateway adapter_equivalence
cargo test -p editor_host --test gateway_process real_editor_gateway_process_preflight_catalog_and_cleanup
```

预计成本：定向 baseline 低到中；inventory 为静态扫描。失败动作：停止修改，记录已有失败或正式范围缺口，不得用删除测试绕过。

#### R2-LR0 执行记录（2026-07-22）

```text
结果：passed
source commit：88b7ca47075311a34ab9e9e4ce5d70ed5f981447
source tree：28e6a20bf0cbfe53edf9e8aad30691fa7452225d
outer source：clean（独立 baseline worktree）
nested gitlink：AiFirstEngine=e332e0ba6d5c1b7772de57ef69804eebe28ea415，clean
toolchain：cargo 1.96.0 / rustc 1.96.0 / x86_64-pc-windows-msvc
baseline worktree：H:\gameEngin-worktrees\254-r2-lr0-88b7ca4
target root：H:\gameEngin-target\254-r2-lr0-88b7ca4
temp root：H:\gameEngin-temp\254-r2-lr0-88b7ca4
H: free：约 625 GiB（执行记录时）
```

保留能力 baseline：

| 命令 | 结果 |
|---|---|
| `cargo metadata --format-version 1 --no-deps --locked` | passed |
| `cargo test -p ai_tool_gateway gateway_session_operation_ownership` | 1 passed |
| `cargo test -p ai_tool_gateway gateway_approval_lifecycle` | 2 passed |
| `cargo test -p ai_tool_gateway adapter_equivalence` | 1 passed |
| `cargo test -p editor_host --test gateway_process real_editor_gateway_process_preflight_catalog_and_cleanup` | 1 passed |

consumer/source inventory：

| 分类 | 文件/职责 | consumer 证据与动作 |
|---|---|---|
| active Core | `ai_tool_gateway/src/codex_config.rs`、`src/bin/ai_engine_gateway_codex_config.rs` | 独立 CLI 仍安装、替换和回滚 typed MCP 配置；保留，不随 activation 退役 |
| active Core | Tool Kernel、typed MCP、protocol/discovery/named-pipe/remote adapter、普通 Gateway session/operation/receipt | 直接服务 AI authoring 工具；保持 active |
| archive-only | `production_candidate.rs`、`production_failure.rs`、`candidate_preparation.rs`、`candidate_registry.rs`、`production_conformance.rs`、`lifecycle_identity.rs`、`release_package.rs`、`source_snapshot.rs` | consumer 全部闭合在 release-candidate producer/seal/reopen 链；与普通 `project.build_export` / `project.delivery_verify` 无关，归入 `production-candidate/` |
| archive-only | `activation_lease.rs`、`real_codex_acceptance.rs` | 只服务 candidate activation 与 acceptance，归入 `shared-lifecycle/` |
| archive-only | `codex_outcome_evaluation.rs`、`codex_outcome_runtime.rs`、`real_session_observer.rs` | 只服务真实 Codex attempt/outcome evaluation，归入 `outcome-evaluation/` |
| archive-only | `acceptance_artifact.rs`、`candidate_freeze_production.rs`、`candidate_freezer.rs`、`frozen_candidate.rs`、`gate_f_acceptance.rs` | `legacy-r1` 精确版本 Gate/冻结职责无 active consumer，归入 `shared-lifecycle/` |
| archive-only | `editor_host/src/production_candidate_owner.rs`、`editor_host/src/candidate_freeze_preflight.rs` | 只由待退役 CLI mode 调用，归入 `production-candidate/` 与 `shared-lifecycle/` |
| archive-only | `ai_tool_gateway/tests/candidate_freezer.rs`、`gate_f_acceptance.rs`、`r2_goal_modules_public_schema.rs`、`r2_production_observer_contract.rs` | 只验证待退役 public surface/lifecycle，归入 `tests/` |
| mixed | `ai_tool_gateway/src/core.rs` | 删除 attempt claim/journal/read/seal/terminal late-dispatch；保留普通 session、Grant、operation、receipt、observe/cancel/reconnect |
| mixed | `ai_tool_gateway/src/editor_host.rs` | 删除 production evaluation composition；保留 discovery publication、named-pipe host 与 shutdown |
| mixed | `ai_tool_gateway/src/lib.rs`、`Cargo.toml` | 删除 lifecycle modules/re-export、`legacy-r1` 与专属 test targets；保留 `codex_config` 和全部 Gateway public surface |
| mixed | `editor_window_winit/src/application.rs`、`src/tests/native_app.rs` | 删除 evaluation field/composition 与 CandidateFreeze helper/tests；保留 Gateway host、project switch/rebind/reconnect |
| mixed | `editor_window_winit/src/real_window.rs` | 删除只被旧 preflight 使用的 exact-freeze readback helper；保留正式 Preview/readback/render path |
| mixed | `editor_host/src/main.rs`、`tests/gateway_process.rs` | 删除 production-candidate/candidate-freeze CLI 与专属测试；保留普通 Editor 和真实 Gateway process smoke |

判定：所有待移动文件均已证明唯一 consumer 属于已退役 lifecycle；未发现需要 active code 依赖 archive 的例外。R2-LR0 完成，可以进入 R2-LR1。

### R2-LR1：退役合同红测试

动作：

- 新增/改造 active Core scope contract，先证明当前代码仍暴露退役公共 surface、旧 CLI/feature/composition 或 attempt terminal 行为，因此测试为红。
- 同时固定保留合同：typed MCP catalog、正常 inspect 后继续调用、项目 Candidate mutation/rollback、operation receipt lineage 必须保持。
- 红测试不得 import 未来 `legacy/**`，不得以旧 lifecycle 能继续运行作为期望。

最低断言：

```text
active lib.rs/Cargo metadata 不声明或 re-export 退役 Module/feature/test target。
editor_host 不接受 production-candidate 或 candidate-freeze-preflight 参数。
Native Editor application 不持有 production evaluation composition。
普通 MCP session 在 inspect 后仍可继续合法 dispatch；不存在 evaluation terminal seal 拒绝。
active source、test、manifest 对 legacy/rust/254-r2-lifecycle 为零引用。
```

命令：

```powershell
cargo test -p ai_tool_gateway --test core_scope_contract
cargo test -p editor_host --test gateway_process retired_production_lifecycle
```

预计成本：低。进入 R2-LR2 前必须保留真实红证据；若测试错误地要求删除项目 Candidate，先修测试设计。

#### R2-LR1 执行记录（2026-07-22）

```text
结果：passed
core_scope_contract red：1 passed / 1 failed；失败为 active lib.rs 仍含 production_candidate
editor_host red：0 passed / 1 failed；失败为 --candidate-freeze-preflight 仍返回 legacy lifecycle diagnostic，而非 unknown_argument
红证据排除了编译、环境和保留工具目录失败；core_catalog_retains_ai_authoring_tools 同轮通过。
```

### R2-LR2：解除 active consumer 与公共 surface

按调用方向处理：

1. 删除 `editor_host` production-candidate/candidate-freeze-preflight mode、owner dispatch 和专属参数校验。
2. 删除 Native Editor 默认 `CodexOutcomeEvaluationModule` 字段、绑定和 shutdown composition；保留 Gateway publication、project switch/rebind 与 reconnect。
3. 从 Gateway Core 删除 evaluation attempt claim/read/complete/seal command、journal 和 terminal late-dispatch rejection；保留普通 operation journal、session ownership、cancel/observe 和 receipt 同步。
4. 删除 lifecycle public re-export、module declaration 和只服务退役路径的 production constructor。
5. 更新 mixed tests：删除旧 lifecycle 用例，保留并强化正常 Gateway/typed MCP/项目 mutation 用例。

定向命令：

```powershell
cargo test -p ai_tool_gateway --test core_scope_contract
cargo test -p ai_tool_gateway gateway_session_operation_ownership
cargo test -p ai_tool_gateway --test explicit_reconnect
cargo test -p editor_host --test gateway_process
cargo test -p editor_window_winit gateway_host_lifecycle
cargo test -p editor_window_winit gateway_reconnect
```

预计成本：中。若移除 attempt 代码影响通用 operation/receipt，必须在 Core 内按真实所有权拆分，禁止删除审计能力或保留 lifecycle shim。

### R2-LR3：使用 git mv 归档 archive-only 源码与测试

动作：

- 按 R2-LR0 inventory 将 `archive-only` 文件和专属测试移动到固定 archive layout。
- 对 `mixed` 文件先完成 active/archive 拆分；禁止整文件移动后从 active code 引用 legacy。
- 编写 archive `README.md` 并验证原路径、目标路径和 `git status` rename evidence。
- R1-only feature/test target 若已证明只是同一退役职责，和其实现一起退出 active Cargo。

门禁：

```powershell
git status --short
Test-Path legacy/rust/254-r2-lifecycle/README.md
rg -n "legacy[/\\]rust[/\\]254-r2-lifecycle" rust --glob '*.rs' --glob 'Cargo.toml'
Get-ChildItem -LiteralPath legacy/rust/254-r2-lifecycle -Recurse -File | Where-Object { $_.Name -in @('Cargo.toml', 'build.rs') }
```

期望：`rg` 对 active Rust/Cargo 零命中；结构化文件检查无输出，不存在可构建 archive manifest。预计成本：低到中。

### R2-LR4：清理 mixed code、Cargo 与依赖

动作：

- 清理 `lib.rs`、Cargo feature/test target、unused dependency、专属 binary 和残留协议类型。
- 只有 `cargo tree -i` 与源码 consumer 共同证明无 active 使用时才移除依赖。
- 保留 typed MCP config 安装/回滚所需的 `codex_config` 行为；若其只服务 activation，则随 lifecycle 归档。该结论必须写入 inventory，不凭名称决定。
- 清理 diagnostic 文案、CLI usage 和测试 fixture 中把 R2 当作当前入口的内容。

命令：

```powershell
cargo metadata --format-version 1 --no-deps --locked
cargo check -p ai_tool_gateway --all-targets --locked
cargo check -p editor_host --all-targets --locked
cargo check -p editor_window_winit --all-targets --locked
rg -n "ProductionCandidateModule|CodexOutcomeEvaluationModule|candidateId|activationId|attemptId|legacy-r1" rust/crates --glob '*.rs' --glob 'Cargo.toml'
```

最后一个搜索只允许项目级 Candidate 的明确上下文和必要历史 test label；所有命中必须逐项写入 allowlist rationale，不能把搜索非零直接当作通过。

#### R2-LR2 至 R2-LR4 执行记录（2026-07-22）

```text
结果：passed
active consumer：production CLI、Native Editor evaluation composition、Gateway attempt journal/seal/late-dispatch、lifecycle public exports 已移除
archive：24 个 archive-only 源码/测试通过 git mv 进入 legacy/rust/254-r2-lifecycle
archive guard：README 存在；无 Cargo.toml/build.rs；active Rust/Cargo 对 archive 零依赖
Cargo：legacy-r1、candidate_freezer/gate_f test targets 与 ai_tool_gateway -> runtime_cli 专属依赖已移除
保留：codex_config CLI、Gateway host/rebind/reconnect、项目 Candidate/rollback、Preview/diagnostics/Build/Delivery 均保持 active
定向结果：core_scope_contract 3 passed；retired CLI 1 passed；session ownership 1 passed；explicit reconnect 1 passed；gateway_process 2 passed；Native host/reconnect 各 1 passed
all-targets：ai_tool_gateway、editor_host、editor_window_winit passed
```

搜索 allowlist：

- `editor_core` 的 `candidateId` 是项目级 `project.mutate.candidate` / rollback schema，必须保留。
- `core_scope_contract.rs` 与 `gateway_process.rs` 中的 lifecycle 名称只用于证明 active public surface/CLI 不可用。
- active Rust/Cargo 中没有 `ProductionCandidateModule`、`CodexOutcomeEvaluationModule`、`activationId`、`attemptId` 或 `legacy-r1` 实现命中。

### R2-LR5：定向与负向验证

必须覆盖：

```text
typed MCP 列表仍是 tool-specific schema，不暴露 raw execute。
正常 session 的 catalog/inspect/search/execute/observe/cancel/reconnect 仍成立。
项目 Candidate mutation 仍只消费合法 ProjectOwnedLowRisk Grant。
rollback 仍使用 receipt-bound authority，并恢复项目 digest。
Preview、runtime/UI diagnostics、Build/Delivery tool registration 不丢失。
旧 production lifecycle public type、CLI arg、feature 和 composition 不可用。
普通 session 不会因历史 attempt terminal 状态拒绝合法调用。
legacy 目录不可构建、不可依赖、不可发布。
```

命令：

```powershell
cargo test -p ai_tool_gateway --test core_scope_contract --locked
cargo test -p ai_tool_gateway mcp_projection_tool_list --locked
cargo test -p ai_tool_gateway gateway_approval_lifecycle --locked
cargo test -p ai_tool_gateway gateway_expired_forward_grant_preserves_receipt_bound_rollback_authority --locked
cargo test -p ai_tool_gateway gateway_concurrent_writer_allows_only_first_same_digest_candidate --locked
cargo test -p ai_tool_gateway gateway_session_operation_ownership --locked
cargo test -p ai_tool_gateway adapter_equivalence --locked
cargo test -p ai_tool_gateway mcp_projection_evidence_read_exposes_scope_and_preview_consumer_contract --locked
cargo test -p ai_tool_gateway --test explicit_reconnect --locked
cargo test -p editor_host --test gateway_process --locked
cargo test -p editor_window_winit gateway_host_lifecycle --locked
cargo test -p editor_window_winit gateway_reconnect --locked
cargo test -p project_e2e_gate gateway_build_delivery_exports_once_then_verifies_existing_package --locked
```

预计成本：中。任何保留能力失败都视为本 Gate 失败，不允许用“退役范围大”豁免。

#### R2-LR5 执行记录（2026-07-22）

```text
结果：passed
13 组定向/负向命令全部通过。
覆盖：typed MCP tool-specific schema、approval lifecycle、receipt-bound rollback、并发 Candidate writer、session operation ownership、Adapter 等价、Preview evidence consumer、explicit reconnect、真实 Editor Gateway process、Native Editor host/reconnect、Build/Delivery E2E。
退役 surface、旧 CLI/feature/composition 不可用；保留的项目 Candidate、rollback、Preview、diagnostics 与 Build/Delivery 均可用。
```

### R2-LR6：受影响域回归

命令：

```powershell
cargo fmt --all -- --check
cargo test -p ai_tool_gateway --locked
cargo test -p editor_host --locked
cargo test -p editor_window_winit --locked
cargo test -p project_e2e_gate --locked
```

预计成本：中到高。进入条件：R2-LR5 全绿。失败动作：把新失败加入累计失败集，在同一隔离环境修复并重跑 R2-LR5 + 失败 crate，不启动 workspace 权威回归。

#### R2-LR6 执行记录（2026-07-22）

```text
结果：passed
cargo test -p ai_tool_gateway --locked：passed
cargo test -p editor_host --locked：passed
cargo test -p editor_window_winit --locked：passed
cargo test -p project_e2e_gate --locked：passed
已知 warning：原有 engine_runtime dead-code 与 feature-dependent Editor helper warning；不构成本次失败。
```

### R2-LR7：环境等价预检与代码冻结

在仓库外 fresh exact-commit worktree 执行，使用新的隔离 `CARGO_TARGET_DIR`、`TEMP/TMP` 和 artifact root。必须确认：

```text
outer 与 nested Git material 固定且 clean。
Cargo.lock 未漂移，metadata 不含 legacy source/feature/test target。
active source 对 legacy/** 零依赖。
archive 无 buildable manifest、active shim 或重复 Implementation。
R2-LR5 全矩阵和 R2-LR6 全受影响域一次性通过。
测试后除已声明 sample Build/Library 生成物外无 source/staged 漂移。
```

预检通过后冻结代码、Cargo、lockfile、测试和 archive inventory。除完成记录与入口同步外，任何上述内容变化都必须重新执行 R2-LR7。

预计成本：高。失败动作：只回到定向/受影响域收敛，不得直接重跑最终权威回归。

#### R2-LR7 首轮失败与收敛记录（2026-07-22）

```text
作废 exact commit：fdfcb21b270f5711ca8b993f9c6467c2265f1773
作废 exact tree：7cfaf61d21ec9d46abbb0ca46129d7bbe82480b5
authoritative R2-LR8：未启动
```

两次仓库外 runner 启动缺口先后暴露为空 `cargo fmt` 日志的 null 解析和 Windows PowerShell 5 把 Cargo 正常 stderr 进度提升为终止异常；二者均在测试得出结果前失败，只修改仓库外 runner，并分别换用新的 fresh worktree/target/temp/artifact 根。

第三个 fresh 根取得首个真实失败：完整 R2-LR5 全绿，R2-LR6 的 `ai_tool_gateway` 51 passed、`editor_host` 21 passed，`editor_window_winit` 为 117 passed / 1 failed，随后 fail-fast，未运行 `project_e2e_gate` 全 crate。失败测试为 `dialog::tests::native_dialog_rejects_invalid_initial_directory_without_fallback`；刚创建的 `file.txt` 被观察为 missing，预期 diagnostic `initial_directory_not_directory` 变为 `initial_directory_missing`。

根因是既有 test-only `unique_temp_root` 只用 PID 与墙钟 tick 作为路径身份，同 tick 并发调用可以生成同一路径并互相 cleanup。同 crate 的 `unique_project_launcher_temp_dir` 已使用 AtomicU64 序号避免该竞争。本次补充相同 tick 的确定性红测试，确认旧 helper 生成相同路径；随后加入进程内原子序号，红测试转绿，`cargo test -p editor_window_winit --locked` 为 119 passed / 0 failed。

由于测试源码已变化，`fdfcb21` 不再满足冻结合同。必须形成新的 exact commit，并从新的 fresh 根完整重跑 R2-LR7；不得复用本轮通过阶段，也不得进入 R2-LR8。

#### R2-LR7 fresh exact-commit 重跑记录（2026-07-22）

```text
结果：passed，19/19 stages passed
冻结 commit：5d7379db3262a456e2f0f889e096d38a6e3d64b4
冻结 tree：624b8a655ab3cd2d6f289139fed05313a43d8760
nested gitlink/index/HEAD：e332e0ba6d5c1b7772de57ef69804eebe28ea415，clean
Cargo.lock SHA-256：d8349ee962cf9ff458f6f48c0a061e208a61a72b704472e86cf911d274871b8d
fresh source：H:\gameEngin-worktrees\254-r2-lr7-5d7379d-d
结果：完整 R2-LR5、四个受影响 crate、metadata/archive/source guard 全部通过
测试后状态：outer 只有允许的 samples/complex_shooter_project/Library/；staged/nested clean
耗时：624.927 秒
report：H:\gameEngin-artifacts\254-r2-lr7-5d7379d-d\r2-lr7-report.json
report SHA-256：94d0ddce4f0d66433b355f9f96e2606332ea387ada0294f093dcb9b40bd4be00
```

`5d7379d` 是唯一进入 R2-LR8 的冻结代码基线；旧 commit `fdfcb21` 保持作废，不得用于后续证据。

### R2-LR8：一次最终权威回归

仅在 R2-LR7 对同一 exact commit 全绿后执行一次 authoritative Runner；default 失败则不启动 all-features。

```powershell
cargo test --workspace --locked
cargo test --workspace --all-features --locked
```

Runner 必须记录 exact commit/tree、target/temp/artifact root、每 stage 起止/exit code/耗时、日志 SHA-256、测试计数、ignored 列表、source/staged/nested clean 和 cleanup。预检不能冒充权威回归，定向测试不能替代完整矩阵。

#### R2-LR8 唯一权威运行与裁定记录（2026-07-22）

```text
裁定结果：passed
authoritative run number：1；没有重跑，也不授权重跑
exact commit/tree：5d7379db3262a456e2f0f889e096d38a6e3d64b4 / 624b8a655ab3cd2d6f289139fed05313a43d8760
default：1544 passed / 0 failed / 5 ignored，exit code 0，954.187 秒
default log SHA-256：d9ac71a739f9390eb9d4e9041e3b2621ccc9eb612084232e6a008ea870bf160a
all-features：1558 passed / 0 failed / 14 ignored，exit code 0，792.097 秒
all-features log SHA-256：afcee92ce6813bd0c42551409ba93d275dbfb8fee669c4d9f4cf3a34348bcb2c
总耗时：1746.432 秒
Cargo.lock：前后均为 d8349ee962cf9ff458f6f48c0a061e208a61a72b704472e86cf911d274871b8d
archive buildable manifest：0；active archive reference：0；staged/nested clean
cleanup：source/nested worktree、isolated target、temp root 已移除；artifact root 保留
```

原始 runner report 保持 `status=failed`，不修改原始证据。该状态来自最终 guard 的实现范围比本节前述 LR7/LR8 合同更窄：它只允许未跟踪 `Library/`，却把完整测试合法重写的 tracked sample `Build/**` fixture 判成 unexpected。独立分类覆盖 29 条 delta，全部属于施工合同已声明的 `samples/complex_shooter_project/Build/**` 或 `Library/**`；没有 source/staged/nested/lockfile 漂移。因此两个权威矩阵和全部施工不变量通过，最终 guard 判定无效，R2-LR8 由独立 adjudication 收敛，不需要也不允许第二次权威运行。

证据：

```text
raw report：H:\gameEngin-artifacts\254-r2-lr8-5d7379d-authoritative\r2-lr8-authoritative-report.json
raw report SHA-256：c41833833b50d965e23706fbcc0a2ba4f2696b3cfc992bdaebc91aebe2a6e918
generated-output classification SHA-256：1b5353f5e4acd818474f4af56a6841f6d79c05beface1efee2f905ecc145daf3
cleanup receipt SHA-256：938b291141d1457bf9975519335d9dc1171156e26664fd3e4b016c297d4be0e3
adjudication SHA-256：7d68c4f35418c8317cd00492cea512d7cd6c1d8d891d4f038930478ea7d18f4f
```

### R2-LR9：完成记录、入口同步与施工归档

仅当 R2-LR0 至 R2-LR8 全部通过：

1. 生成 `阶段完成记录/2026-07-xx-R2-Lifecycle-Retirement-Core-Simplification-v1/00-总览.md`。
2. 记录 inventory、归档 commit、rename evidence、三层验证、authoritative run 次数/耗时、失败重跑原因、cleanup 和遗留风险。
3. 同步 `README.md`、总体架构 `README.md`、`00-文档地图.md`、`00-阅读顺序.md`、`49`、`54`、`240`、施工文档 README 与阶段完成记录 README。
4. 将本文件从 `施工文档/当前/` 移到 `施工文档/已完成/`。
5. 状态只能写为“R2 lifecycle retired / 254 Core simplified”；不得写成 R2-FC6、Real Evaluation 或真实 AI acceptance passed。

#### R2-LR9 执行记录（2026-07-22）

```text
结果：passed
阶段完成记录：阶段完成记录/2026-07-22-R2-Lifecycle-Retirement-Core-Simplification-v1/00-总览.md
入口同步：README、总体架构 README、00 文档地图/阅读顺序、49、54、240、施工文档 README、阶段完成记录 README
正式状态同步：254-AI-Tool-Gateway-Codex-Adapter-v1方案.md 当前施工状态
施工归档：施工文档/已完成/254-当前可自动化施工文档-R2-Lifecycle-Retirement-Core-Simplification-v1.md
当前施工：无
待执行施工：无
终态：R2 lifecycle retired / 254 Core simplified
```

## 8. 高风险验证清单

```text
[x] ProductionCandidateModule/CodexOutcomeEvaluationModule 公共 surface 真正消失，无改名 shim。
[x] editor_host 旧 production/freezer 参数真正不可用，普通 Editor Gateway 入口仍可用。
[x] Native Editor project switch/reconnect/publication 未被 composition 删除破坏。
[x] Gateway operation journal、receipt sync、cancel/observe、disconnect ownership 保留。
[x] 普通 session 无 attempt terminal late-dispatch rejection。
[x] project.mutate.candidate 和 rollback 没有被 release candidate 退役误删。
[x] ProjectOwnedLowRisk approval、digest drift、read generation 与 rollback authority 保留。
[x] Preview/diagnostics/Build/Delivery catalog surface 保留。
[x] mixed 文件按 consumer 拆分，无 active/archive 双实现。
[x] active Rust/Cargo/tests/manifests 对 legacy/** 零依赖。
[x] archive 无 Cargo/build/feature/compatibility 入口。
[x] legacy-r1 feature/test target 若退出，已有 R1 历史证据仍由 Git/归档可追溯。
[x] default/all-features 使用同一冻结 exact commit；最终权威 Runner 只执行一次，原始 guard 误判由独立证据裁定。
```

## 9. 失败收敛

- inventory 不能证明某文件 archive-only：保留在 active，记录 mixed/unknown，并停止该文件移动；不得猜测。
- 发现正式方案仍要求 active lifecycle：停止施工，先修正式方案并重新自审本文档。
- 删除 lifecycle 导致项目 Candidate、receipt、rollback、Preview 或 Build/Delivery 失败：视为实现偏离，回到当前 Gate 修复，不得缩小测试。
- 同类 consumer 第二次出现：全仓搜索并统一清理，禁止逐文件碰运气。
- 环境等价预检失败：修复后重跑累计失败集和受影响域；不得启动 R2-LR8。
- 权威回归失败：保留结构化 artifact，以首个真实失败为准；只有新的 R2-LR7 全绿后才可由用户另行决定是否授权第二次权威运行。
- 任一失败不授权 Candidate、activation、attempt、F-A/F-B/G 或三引擎 B 通道。

## 10. 施工文档自审

### 10.1 与正式方案一致性

通过。施工目标逐项对应 254 第 30.5-30.7 节：解除 consumer、移除公共 Interface、归档 archive-only 源码、清理 Cargo，并保留 Tool Kernel 与项目级 Candidate。R2 正式方案已是历史输入，不被当作当前规范执行。

### 10.2 是否扩大范围

未扩大。计划只删除 exact engine release candidate / real AI acceptance lifecycle 及其无独立 consumer 的支撑代码；capability-aware Catalog、其它工具增强、R1/R2 重试和外部引擎验证明确排除。R1 遗留只在证明其属于同一退役职责后才退出 active build surface。

### 10.3 是否限制 AI

不会。删除的是工具外验收状态机和 terminal attempt 约束；AI 的工具选择、顺序、分支和动态重规划不变。保留的 typed MCP、广泛观察、领域深工具和通用 mutation 逃生通道仍是 254 的主要 Interface。

### 10.4 是否形成新的菜谱或浅 Module

不会。施工没有新增公共 Module、Runner 或 next-step Interface。Gate 是一次性代码迁移的工程顺序，不是 AI 使用引擎时必须遵循的运行菜谱。按 deletion test，退役 Module 删除后其复杂度应消失，而不是散到 callers。

### 10.5 是否误删项目 Candidate 或审计

有明确防线。第 3 节区分两类 Candidate；R2-LR1、LR5、LR6 和高风险清单持续验证项目 mutation、Grant、operation、receipt、digest、rollback 与 evidence。任何回归都阻塞施工完成。

### 10.6 归档合同是否可执行

可执行。固定目录、`git mv`、README 字段、零 active dependency、无 Cargo/feature/shim 和 mixed-file 先拆分都已写成 Gate 与命令；没有要求历史代码继续编译。

### 10.7 验证层次与成本

符合仓库纪律。R2-LR1/LR5 是定向和负向测试，R2-LR6 是受影响域，R2-LR7 是 clean exact-commit 环境等价预检，R2-LR8 才是一次最终 default/all-features 权威回归。失败先在便宜层收敛，不把完整 workspace 当调试循环。

### 10.8 当前基线未知是否被掩盖

没有。文档只记录高置信 consumer，明确要求激活后对 lifecycle-looking 文件逐一分类；尤其不预判 `codex_config`、`lifecycle_identity`、`release_package`、`source_snapshot` 的独立 consumer。未知分类不能通过猜测移动。

### 10.9 授权边界

清楚。原始自审时本文件位于 `待执行/`；该历史判断已由第 11.1 节的激活前复核和用户施工授权覆盖。当前只授权 R2-LR0 至 R2-LR9 的 lifecycle retirement / Core simplification，仍不授权任何 candidate、activation、attempt、真实 AI acceptance 或历史 Gate。

自审结论：

```text
passed
本施工文档与 254 当前正式范围一致，范围闭合、依赖顺序合理、保留能力有回归门禁、归档合同可验证。
原始自审允许文档进入待执行队列；第 11.1 节随后记录了独立激活前复核与用户代码施工授权。
```

## 11. 激活前复核条件

激活前必须全部确认：

```text
1. 254 第 30 节仍是当前规范，R2 仍为 superseded historical。
2. ProductionCandidateModule remediation 仍为 cancelled，没有新方案恢复 release candidate acceptance。
3. 施工文档/当前/ 没有其它非 README 施工文档；历史机器报告不算施工入口。
4. 本文件仍是正式优先级最高的待执行文档。
5. 文档改动已进入明确 exact commit；专用仓库外 clean worktree 可复现该 commit。
6. outer/nested source、Cargo.lock、工具链、磁盘、target/temp/artifact root 满足环境等价要求。
7. R2-FC6 历史 artifact/digest 可读，但没有 candidate/activation/attempt 复用。
8. 用户单独授权代码施工，并明确允许把本文件移动到 当前/。
```

任一项不成立，本文档留在 `待执行/`，修订并重新自审；不得原样激活。

### 11.1 2026-07-22 激活前复核记录

```text
结果：passed
正式范围：254 第 30 节仍为当前规范；254-R2 为 superseded historical；remediation 未恢复。
文档槽位：当前目录无其它有效施工文档；历史机器报告不计为施工入口。
文档基线 commit：88b7ca47075311a34ab9e9e4ce5d70ed5f981447
文档基线 tree：28e6a20bf0cbfe53edf9e8aad30691fa7452225d
nested gitlink/index/HEAD：e332e0ba6d5c1b7772de57ef69804eebe28ea415，clean。
工具链：cargo/rustc 1.96.0 x86_64-pc-windows-msvc。
环境：H: 可用空间约 679 GB；target/temp/artifact 使用 H: 下独立根。
历史证据：错误归档与 I:\EngineTest\Evidence 可读；没有复用 candidate/activation/attempt。
授权：用户明确要求复核后直接开始施工。
```

本文档已从 `待执行/` 移入 `当前/` 并同步入口；R2-LR0 从该 exact 文档基线开始。
