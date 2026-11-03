# 246-当前可自动化施工文档-Code Hygiene Semantic Architecture Review Gate v2

> 状态：`已完成 / Gate A-G 全部通过 / CQ-07 已关闭`。
> 正式方案：`../../246-Code-Hygiene-Semantic-Architecture-Review-Gate-v2方案.md`。
> 选题：`240` Priority 6 / CQ-07。
> 前置：151 Stage A-K 与 245/CQ-08 已完成并归档；激活前 `施工文档/当前/` 为空。
> 激活记录：2026-07-12 已按用户明确指令重新核对 clean exact-commit 基线并移入 `当前/`。
> 施工记录：主体实现 `be2ac8b6`；Provider 兼容 `85550cb`；canonical bundle `130f951`；最终 exact commit `3d6db9a7c859aea6c95c0b32f24f6b42c4ad5911`。
> 完成证据：真实 `gpt-5.5` Provider artifact、EngineStrict QualityGate v2 与 authoritative Local CI 全部通过。

## 1. 施工目标

把现有只报告规模的 `code_hygiene.report.v1` 升级为可持续治理系统：

```text
architecture-policy.v1
  -> EngineStrict / ProjectAdvisory scope
  -> deterministic inventory / dependency / facade / size risk
  -> historical debt + review coverage + touched-scope reconciliation
  -> trusted AI semantic review artifact
  -> local evidence/provenance validation
  -> review_required / disposition contract
  -> QualityGateRunner
  -> quality-gate-report.v2
```

完成后必须证明：

```text
引擎侧新结构回归和未处置 verified high/critical finding 会阻断。
项目侧 AI finding 默认只建议，不阻断 Build/Play/Export。
项目源码不会在未授权时发送给外部 Provider。
旧引擎代码完成确定性全量建账，AI review coverage 可分批且不伪装完整。
未触碰旧债不阻断无关变更；触碰后不得恶化；已解决债务不能回退。
public CI 不持有 Provider secret；Merge 只信 exact-commit trusted artifact。
quality_gate 不依赖 editor_core，不进入 Runtime/Player。
245 的 v1 artifact 保持可读取；246 新产物诚实使用 v2。
```

## 2. 施工范围

### 2.1 预计修改

```text
rust/Cargo.toml
rust/Cargo.lock（只允许由正式依赖变化生成）

rust/crates/code_hygiene/Cargo.toml
rust/crates/code_hygiene/src/lib.rs
rust/crates/code_hygiene/src/main.rs
rust/crates/code_hygiene/tests/hygiene_report.rs

rust/crates/quality_gate/Cargo.toml
rust/crates/quality_gate/src/lib.rs
rust/crates/quality_gate/src/main.rs
rust/crates/quality_gate/src/runner.rs
rust/crates/quality_gate/src/report.rs
rust/crates/quality_gate/src/workspace.rs
rust/crates/quality_gate/src/architecture_policy.rs（新增）
rust/crates/quality_gate/src/architecture_inventory.rs（新增）
rust/crates/quality_gate/src/architecture_debt.rs（新增）
rust/crates/quality_gate/src/change_scope.rs（新增）
rust/crates/quality_gate/src/architecture_review.rs（新增）
rust/crates/quality_gate/src/architecture_artifact.rs（新增）
rust/crates/quality_gate/tests/architecture_policy.rs（新增）
rust/crates/quality_gate/tests/architecture_gate_matrix.rs（新增）
rust/crates/quality_gate/tests/report_compatibility.rs（新增）

rust/quality/architecture-policy.v1.toml（新增）
rust/quality/architecture-debt-ledger.v1.json（新增）
rust/quality/architecture-review-coverage.v1.json（新增）

rust/crates/ai_transport/*（候选新增；仅在 Gate E 证明提取能复用 238/243 且不产生反向依赖时落地）
rust/crates/architecture_review/*（新增 tooling crate）

rust/crates/editor_core/Cargo.toml
rust/crates/editor_core/src/project_patch/llm_http.rs
rust/crates/editor_core/src/project_patch/llm_transport.rs
rust/crates/editor_core/src/project_patch/llm_request_controller.rs
rust/crates/editor_core/src/project_patch/llm_source.rs
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/report_panel.rs

.github/workflows/rust-quality.yml
框架设计/引擎总体架构/246-Code-Hygiene-Semantic-Architecture-Review-Gate-v2方案.md
框架设计/引擎总体架构/240-5.6审查剩余问题讨论与施工优先级.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

实际新增模块名可以在激活前复核时小幅调整，但必须保持以下依赖方向：

```text
quality_gate -> pure policy/inventory/artifact verification
architecture_review tooling -> quality_gate contracts + generic AI transport
editor_core -> generic AI transport

禁止：quality_gate -> editor_core
禁止：Runtime/Player -> quality_gate/architecture_review/AI transport
```

### 2.2 明确不修改

```text
不拆分 aui.rs、runtime_player_winit/src/lib.rs、report_panel.rs 等具体热点实现。
不修改玩法、AUI Runtime、ECS、渲染、物理或 RuntimePackage 语义。
不把 ProjectAdvisory 改成默认阻断。
不把真实 Provider secret 放进 fork/untrusted PR。
不自动批准 AI finding、exception、baseline 或 policy 放宽。
不把 AI 原始 response、完整源码或 credential 写进 report/artifact。
不重开或修改已完成归档的 245。
```

## 3. 激活前复核

本文从 `待执行/` 移入 `当前/` 前必须逐项确认：

```text
245 已取得本地 exact-commit CI artifact、完成记录并归档。
当前/ 为空，54 没有其它执行入口。
246 正式方案仍为当前真相，没有新外部审查推翻设计。
quality_gate/report/provider 代码基线与本文预计文件一致。
238/243 的 LLM transport/controller API 没有发生未吸收漂移。
Rust workspace、toolchain、Cargo.lock 和 lint ledger 处于可验证状态。
真实 Provider 的受信执行环境、授权方式和 Gate G 完成条件可说明。
每个 Gate 的测试命令仍存在且不会与其它进程争抢同一 target。
```

除真实 Provider 配置外，任一项不满足：保留在 `待执行/`，先修订本文并重新自审；禁止原样激活。用户已明确确认本地工作流允许先激活、后在 Gate G 前补齐 Provider，但不得降低最终完成合同。

### 3.1 本次激活前复核结果

```text
通过：245 exact-commit Local CI、完成记录和归档均存在。
通过：激活前 `当前/` 为空，240/54 的下一项是 246/CQ-07。
通过：quality_gate v1、code_hygiene v1、LLM transport/controller、Report Panel 和 LocalCiRunner 基线存在。
通过：在 detached worktree 对 exact commit `139548df191fc5cc527f8e02e367d2d1d7aee41b` 复核，worktree clean；`cargo metadata --locked --format-version 1 --no-deps` 成功解析 20 个 workspace package。
通过：Rust 1.96.0、Cargo.lock、lint ledger 与 13 项引用基线文件均存在；临时 worktree 已清理。
通过：本方案允许本地 trusted reviewer，不要求 Git remote 或远端 required check。
待满足：当前进程没有 AI_ENGINE_LLM_API_KEY、AI_ENGINE_LLM_BASE_URL、AI_ENGINE_LLM_MODEL，也没有已生成的 exact-commit trusted architecture review artifact。
结论：246 已激活并完成全部本地可执行施工；Provider 缺口仍阻止 Gate G 外部激活与 CQ-07 完成，不得用 scripted provider 降低完成合同。
```

Gate G 完成前置：

```text
1. 在受信本地环境配置 Provider credential、HTTPS/loopback base URL 和 model；不把值写入文档、Git 或 report。
2. 明确 EngineStrict 发送范围、数据类别、预算、撤销和过期规则。
3. 在候选冻结后从 exact HEAD 建立专用 clean worktree，确认 247 或其它未提交内容不进入施工提交。
4. 运行环境等价预检与最终权威回归，生成并核对 exact-commit trusted artifact。
```

### 3.2 三层验证与成本预算

| 层级 | 内容 | 预计成本 | 进入条件 | 失败后动作 |
|---|---|---:|---|---|
| 定向测试 | 当前 Gate 的 schema/parser/matrix/artifact/transport 测试 | warm cache 下秒级到约 2 分钟 | 当前 Gate 代码可编译 | 精确复现，加入累计失败集，不跑完整 workspace |
| 受影响域回归 | `code_hygiene`、`quality_gate`、`architecture_review`、Editor LLM/report、`project_e2e_gate` | 约 3-8 分钟，cold cache 可更长 | 当前 Gate 定向测试全绿 | 修复后一次性重跑累计失败集和受影响域 |
| 最终权威回归 | clean exact commit、真实受信 Provider artifact、default/all-features、Local CI | 当前基线约 8-12 分钟加 Provider 时间 | Gate A-G 预检全绿且候选冻结 | 从 artifact 定位首个失败；环境等价收敛全部已知失败后才重跑 |

成本是计划值，不是超时或通过条件。最终权威回归默认只运行一次；定向测试和受影响域回归不能冒充完成证据。

### 3.3 环境等价与高风险清单

```text
专用 clean worktree / exact base-head commit / Rust 1.96.0 / Cargo.lock。
显式 CARGO_TARGET_DIR、target/quality-gate artifact 根、Windows CRLF/LF 兼容和子进程环境传播。
245 quality-gate-report.v1 reader 与 LocalCiRunner exact-commit/cleanup 合同。
quality_gate 不得依赖 editor_core；Runtime/Player 不得链接治理或 AI transport。
LLM credential lease、HTTPS/loopback、body/token/cost/timeout/rate budget、cancel/join 和 redaction。
scripted/recorded/prompt-injection/eval fixtures，以及一次真实 Provider artifact。
policy/ledger/coverage candidate 不得修改 committed 真相；rename/move/touched-debt identity 必须稳定。
```

同类缺口出现第二个 consumer 时先全仓搜索并收敛共享 interface。最终候选冻结后，代码、test、manifest、lockfile、policy、ledger 或 coverage 任一变化都必须重新跑环境等价预检。

## 4. Gate A - Policy / Scope / Deterministic Inventory

### 4.1 任务

1. 定义 `architecture-policy.v1` serde schema、严格 parser 和 diagnostics。
2. 建立 `EngineStrict`、`ProjectAdvisory`、显式 `ProjectStrict` profile。
3. 声明 engine/project include、generated/vendor/build exclusions、domain/owner、allowed dependency、facade-only、review authority。
4. 使用 Cargo metadata 建 crate dependency graph；使用 `syn` 建 Rust file/symbol/impl/import inventory。
5. Cargo manifests、build scripts、workflow、engine schema/policy 进入 EngineStrict 入口 inventory。
6. policy diff 必须输出影响范围；放宽规则不能静默通过。
7. 禁止 ad hoc regex 作为 Rust symbol 或依赖真相。

### 4.2 Gate A 测试

```powershell
cargo test -p quality_gate architecture_policy
cargo test -p quality_gate architecture_inventory
cargo test -p quality_gate --test architecture_policy
cargo clippy -p quality_gate --all-targets --all-features --locked -- -D warnings
```

### 4.3 Gate A 验收

```text
未知 schema/profile/domain/rule ID fail closed。
路径重叠、owner 缺失、循环或禁止依赖有结构化 diagnostics。
generated/vendor exclusion 只能来自 committed policy。
EngineStrict 和 ProjectAdvisory 对同一 finding 的 decision 不同但证据相同。
policy 放宽产生可审查 diff，不自动写回。
```

## 5. Gate B - code_hygiene v2 / Historical Debt / Coverage

### 5.1 任务

1. `code_hygiene.report.v2` 输出逐文件 fingerprint、风险带、domain、owner、baseline delta 和 changed classification。
2. 保留 v1 读取兼容；v2 writer 不伪装成 v1。
3. 建立 `architecture-debt-ledger.v1` 和 `architecture-review-coverage.v1` schema。
4. 首次对 EngineStrict scope 完成确定性 inventory，形成 candidate ledger/coverage。
5. candidate 只能写 `target/quality-gate/candidate/`，人工审查后才能成为 committed ledger。
6. ledger/coverage 支持 active/resolved/coverage_pending，包含 owner、reason、review_by、plan 和 evidence digest。
7. 过期、失效、自动放宽、resolved 回退、新问题伪装历史债务均 fail closed。

### 5.2 Gate B 测试

```powershell
cargo test -p code_hygiene
cargo test -p quality_gate architecture_debt
cargo test -p quality_gate architecture_coverage
cargo run -p code_hygiene --locked -- --root crates
```

### 5.3 Gate B 验收

```text
单纯超过 1,000 行只产生风险，不单独证明失败。
历史 >2,000/>4,000 热点被建账但不会全局阻断无关修改。
candidate 生成前后 committed ledger digest 不变。
coverage_pending 不会被报告成 reviewed/passed。
改进后的 baseline 不能回退到更差历史值。
```

## 6. Gate C - Canonical Change Scope / Touched Debt Reconciliation

### 6.1 任务

1. 定义 `base_commit/head_commit/dirty_patch_digest` 和 accepted merge-base policy。
2. 基于 AST/content/dependency fingerprint 建 changed symbol 和 dependency neighborhood。
3. 支持 rename/move continuity，line number 不进入 identity。
4. 纯格式、注释拼写和内容不变移动可分类为 non-semantic；不确定时升级 review_required。
5. 实现 historical finding before/after reconciliation。
6. 未触碰未到期旧债不阻断无关 domain；触碰后新增职责、扩大依赖或风险提高会阻断。
7. 到期债务阻断 subject/domain 功能增长，但允许明确 remediation change。

### 6.2 Gate C 测试

```powershell
cargo test -p quality_gate change_scope
cargo test -p quality_gate architecture_debt
cargo test -p quality_gate --test architecture_gate_matrix touched_debt
cargo test -p quality_gate --test architecture_gate_matrix rename_move
```

### 6.3 Gate C mutation matrix

```text
unrelated domain change -> pass with historical debt summary
local bug fix without growth -> pass, review_by unchanged
new mixed responsibility in hotspot -> fail
dependency expansion -> fail
real responsibility extraction -> pass and lower baseline
rename/move same subject -> finding remains
resolved debt reintroduced -> fail as new
unknown base/ambiguous change scope -> fail closed/review_required
```

## 7. Gate D - Review Contract / Local Evidence / Artifact Verifier

### 7.1 任务

1. 定义 strict `ArchitectureReviewRequest/Outcome/Finding/Artifact` Rust schema。
2. finding 限制 path、symbol、issue type、severity、confidence、rule IDs、coverage 和 evidence digest。
3. 实现 candidate -> evidence_verified -> review_required -> disposition 状态机。
4. 本地复算 path/symbol/AST/dependency/rule/context；幻觉证据不能成为 high/critical blocker。
5. artifact 绑定 base/head/patch、workspace、policy、model、prompt、schema、context、coverage digest。
6. 实现 artifact freshness、provenance、size、redaction 和 disposition verifier。
7. 建立 scripted provider、recorded fixture 和 prompt-injection fixture。
8. QualityGateRunner 此 Gate 只验证 artifact，仍不调用网络 Provider。

### 7.2 Gate D 测试

```powershell
cargo test -p quality_gate architecture_review
cargo test -p quality_gate architecture_artifact
cargo test -p quality_gate --test architecture_gate_matrix ai_evidence
cargo test -p quality_gate --test architecture_gate_matrix artifact_provenance
```

### 7.3 Gate D negative matrix

```text
missing/unknown rule ID -> provider_invalid_evidence
missing symbol/path -> provider_invalid_evidence
fabricated dependency edge -> provider_invalid_evidence
commit/context digest mismatch -> fail
stale/expired artifact -> fail
high candidate without local verification -> cannot block as confirmed finding
verified high without disposition -> review_required/fail Merge
rejected without reviewer/reason -> fail
source comment prompt injection -> no policy/tool override
raw credential/absolute path/full response in artifact -> fail redaction
```

## 8. Gate E - Shared AI Transport / Trusted Architecture Review Tool

### 8.1 前置设计检查

先确认 238/243 transport/controller 中哪些能力是真正项目无关的：credential lease、strict HTTP body limit、timeout、cancel/join、retry/rate classification。只有提取后能同时减少重复并保持 public API/行为，才新增 `ai_transport` crate；否则使用独立 command Adapter，不允许 `quality_gate -> editor_core`。

### 8.2 任务

1. 建立独立 `architecture_review` tooling crate/CLI，读取 committed policy 和 canonical context。
2. 复用经 Gate E 证明的 generic transport；不得复制一套无 cancel/join/limit 的简化 HTTP。
3. 外部 Provider 使用 strict structured output、request/response/token/cost/timeout/rate budgets。
4. cache key 包含 commit/content/policy/model/prompt/schema/context digest。
5. 生成 artifact 到 `target/quality-gate/`，不修改 source ledger/policy。
6. public CI 保持无 secret；trusted protected job/runner 才能调用真实 Provider。
7. fork/untrusted PR 不得到 credential，也不能扩张 context path。
8. ProjectAdvisory 外部 Provider 默认 Off，增加项目/provider/scope/data-class 授权、预览、撤销和过期合同。
9. Editor Report Panel 只显示 Summary/Trace 和 next action，不保存完整源码/response。

### 8.3 Gate E 测试

```powershell
cargo test -p architecture_review
cargo test -p architecture_review --test loopback_provider
cargo test -p editor_core project_patch::llm_request_controller
cargo test -p editor_core project_patch::llm_transport
cargo test -p editor_core report_panel
cargo test -p project_e2e_gate --lib
```

如落地 `ai_transport`：

```powershell
cargo test -p ai_transport
cargo clippy -p ai_transport -p architecture_review --all-targets --all-features --locked -- -D warnings
```

### 8.4 Gate E 验收

```text
quality_gate 依赖图中不存在 editor_core/reqwest/tokio。
Editor 原有 238/243 provider、cancel/join、credential tests 语义不退化。
ProjectAdvisory 未授权不发请求、不阻断 Build/Play/Export。
撤销授权后停止新请求；旧 artifact 不能越权复用。
loopback 覆盖 success/refusal/timeout/cancel/rate-limit/oversize/invalid schema。
```

## 9. Gate F - QualityGate v2 / CI / Report / Evaluation

### 9.1 任务

1. `QualityGateRequest` 增加 profile、base/head/patch 和 artifact expectation，不破坏 Fast 默认入口。
2. `QualityGateReport` 新 writer 使用 `quality-gate-report.v2`，保留 v1 reader。
3. v2 汇总 policy、inventory、debt、coverage、changed scope、AI artifact、finding disposition 和 final decision。
4. Summary 不保存 finding 明细源码；Trace 只保存脱敏 anchor/diagnostic。
5. workflow 保持薄：public job 运行 deterministic Fast；trusted job 产出 artifact；Merge job 验证 artifact。
6. 为 Report Panel 注册 Quality/Architecture Summary provider，不新增顶级报告真相。
7. 建立 architecture eval corpus：wrong-domain、mixed responsibility、fake wrapper、cohesive large module、rename/move、prompt injection、hallucinated evidence、partial/timeout/budget。
8. Provider/model/prompt/policy 升级输出 blocking precision、finding stability、coverage 对比；不允许只凭一次真实模型结果改 baseline。

### 9.2 Gate F 测试

```powershell
cargo test -p quality_gate
cargo test -p quality_gate --test report_compatibility
cargo test -p quality_gate --test ci_contract
cargo test -p quality_gate --test architecture_gate_matrix
cargo test -p architecture_review architecture_eval
cargo test -p editor_core report_panel
```

### 9.3 Gate F 验收

```text
245 v1 fixture 可读取；246 writer 只写 v2。
public workflow 无 secret/write/publish permission。
fork PR 不执行持密钥 trusted step。
unknown required v2 semantics fail closed。
cohesive large module fixture 不因行数机械失败。
fake wrapper/mixed-domain fixture 被稳定发现。
```

## 10. Gate G - Baseline Bootstrap / Full Regression / External Activation

### 10.1 初始基线

1. 对当前 EngineStrict scope 生成确定性 inventory candidate。
2. 人工核对 domain/owner/exclusion 和 historical debt candidate。
3. 按风险生成第一批 AI review；未覆盖部分写 `coverage_pending`。
4. 不要求 246 内拆完热点，但每个 active debt 必须有 owner/reason/review_by/plan。
5. 记录 >1,000/>2,000/>4,000 热点及真实行数，不复用讨论期近似值。
6. candidate 与 committed diff 必须单独审查；verify 不能自动覆盖。

### 10.2 整体回归

```powershell
cargo fmt --all -- --check
cargo test -p code_hygiene
cargo test -p quality_gate
cargo test -p architecture_review
cargo test -p editor_core --lib
cargo test -p project_e2e_gate --lib
cargo test --workspace --locked
cargo test --workspace --all-features --locked
cargo clippy -p code_hygiene -p quality_gate -p architecture_review --all-targets --all-features --locked -- -D warnings
```

如 Gate E 新增 `ai_transport`，整体回归和 Clippy 必须包含该 crate。

### 10.3 真实受信 Provider 激活

在受信环境显式执行一次真实 Provider review，必须验证：

```text
exact base/head commit
policy/model/prompt/schema/context digest
credential/redaction contract
artifact provenance/freshness
local evidence verification
finding disposition
quality-gate-report.v2 final decision
```

本项目不要求 Git remote 或远端 required-check。没有受信 Provider credential，或没有绑定 exact local base/head commit 的 artifact 时：本地实现可以记录 Gate A-F 通过，但 CQ-07 不得标记完全激活/完成，也不得伪造真实 AI evidence。

### 10.4 2026-07-12 当前施工结果

```text
Gate A-F：通过。
Gate G 本地部分：通过；default workspace、all-features workspace、Clippy、历史 artifact 兼容、受影响域回归与 exact-commit Local CI 均已执行。
实现提交：be2ac8b6（Implement semantic architecture review gate）。
首轮权威 Local CI：在 be2ac8b6 上失败；source_suppression 正确发现 architecture_review/src/main.rs 中未登记的 #[allow(dead_code)]。同轮 all-features workspace 出现瞬时失败，随后在相同 clean exact-commit 条件下独立复跑通过。
修复提交：3e76b022（Remove unregistered architecture review suppression）。
第二轮权威 Local CI：在 exact commit 3e76b022c60188dded66bb7d55f63e0bc20dbcf1 上通过，source_workspace_state=clean，isolated_workspace_state=clean，diagnostics=0，cleanup=removed。
Local CI artifact：target/quality-gate/local-ci/local-3e76b022c601-1783856153/quality-gate-report.v2.json。
Artifact digest：sha256:eec9e26e5d82dc7eda41990a867384b8e3d84104217d371724a1e2c8f0fa6112。
真实 Provider：未执行；当前环境未配置 AI_ENGINE_LLM_API_KEY、AI_ENGINE_LLM_BASE_URL、AI_ENGINE_LLM_MODEL。
```

以上是 Provider 授权前的中间状态；该阻塞已由 10.5 的真实 Provider、EngineStrict 和 Local CI 证据关闭。

### 10.5 2026-07-12 最终 Gate G 结果

```text
Provider：user-supplied-provider / gpt-5.5 / trusted_provider / complete。
Provider endpoint：https://sub2api.zhouzelong.cn/v1/；credential 仅注入单次进程，未写入 source、artifact 或文档。
Canonical batch：按风险选择 crates/engine_runtime/src/aui.rs 与 crates/editor_core/src/asset_browser.rs；其余 subject 保持 coverage_pending。
Artifact exact base：9e8923a31163bc113aef1f888ec31484280f9371。
Artifact exact head：3d6db9a7c859aea6c95c0b32f24f6b42c4ad5911。
Provider artifact digest：sha256:c47fe43cee67b88020928f3d2a12bafec6bc0c180efdcca1b5e4c8d3fc12bab3。
EngineStrict：12/12 stages passed，artifact_status=verified，final_decision=passed，diagnostics=0，duration_ms=1183843。
EngineStrict report digest：sha256:b00b9e0ec99857afd6e0b427499eaa8406f4eae1da90cb5dc1c1c9c613b49c9c。
Local CI run_id：local-3d6db9a7c859-1783860976。
Local CI：source_workspace_state=clean，isolated_workspace_state=clean，cleanup_status=removed，diagnostics=0，passed=true，duration_ms=898896。
Local CI quality report digest：sha256:3e222c30d28f0d47e7ca7707192cf1a2d23078912b52da8647857f6911f4e031。
Local CI run report digest：sha256:977e86512dc8542569c54128289f7610c903a38fb148fad5ab9fc8bd82286a45。
```

Gate G 与 CQ-07 已完成，可以生成完成记录并归档本文。

## 11. Report 与诊断要求

所有失败至少包含：

```text
code
profile
source path / domain / subject
stage
observed evidence
rule ID
classification/disposition
next action
```

报告档位：

```text
Runtime：Off，不链接治理代码。
Editor/CLI Summary：计数、状态、最高风险、next action。
Trace：测试/显式诊断使用，仍需路径和源码脱敏。
```

禁止把 Provider raw response、完整 source context、API key、绝对用户路径或未授权项目文件写入 artifact/report。

## 12. 文档同步与归档

全部 Gate 和真实受信 Provider review 完成后：

1. 更新 246 正式方案状态。
2. 更新 240 CQ-07 为施工完成。
3. 更新 49、54、施工文档 README、阶段完成记录 README。
4. 生成 `阶段完成记录/2026-xx-xx-Code-Hygiene-Semantic-Architecture-Review-Gate-v2/00-总览.md`。
5. 把本文从 `施工文档/当前/` 移到 `施工文档/已完成/`。
6. 为每个需要真实拆分的热点另开独立方案/施工文档，不在完成记录中声称已拆分。

如果施工后只完成 Gate A-F、真实受信 Provider review 仍阻塞：本文继续留在 `当前/`，状态写清外部阻塞，不生成完成记录、不归档。Provider 不得在 Gate G 中被降级或伪造。

## 13. 回滚策略

```text
policy/ledger/coverage：按 schema artifact 回滚，不覆盖用户无关改动。
report v2：保留 v1 reader；回滚 writer 时不得破坏 245 artifact。
transport 提取：如破坏 238/243 生命周期，先恢复原 consumer 路径并保留新增 contract 测试。
workflow：回滚 Adapter 不得回滚仓库内 policy/validator 真相。
baseline：verify 永不自动修改 committed 文件，因此失败时删除 candidate 即可。
```

禁止使用 `git reset --hard`、`git checkout --` 或删除用户工作树改动作为回滚方式。

## 14. 施工文档自审

### 14.1 是否与 246 正式方案一致

通过。Gate A-G 覆盖 EngineStrict/ProjectAdvisory、policy、旧债务、触碰式治理、可信 AI artifact、本地 evidence、授权、报告 v2、评测和真实激活。

### 14.2 是否干扰 245

不干扰。245 已完成并归档；本文是 `当前/` 唯一施工文档，已完成本地施工，也不重开、修改或替代 245。

### 14.3 是否形成第二执行入口

没有。本文已在激活前复核通过后成为 `当前/` 唯一施工文档，54 和目录 README 指向同一入口。

### 14.4 是否范围过大

范围较大但保持单一治理链路。具体热点拆分明确排除；Gate 按 policy -> debt -> change scope -> AI evidence -> provider -> report -> activation 串行，禁止跨 Gate 扩张。

### 14.5 是否错误影响项目侧

没有。ProjectAdvisory 只建议，外部 Provider 默认 Off；未授权不会发送源码，也不会阻断 Build/Play/Export。

### 14.6 是否增加 Runtime 开销

没有。依赖方向和验收明确禁止 Runtime/Player 链接 quality_gate、architecture_review 或 AI transport。

### 14.7 AI 是否能凭感觉阻断

不能。Gate D 要求 strict schema、本地 path/symbol/dependency/rule/context 复算和 reviewer disposition；幻觉证据不能成为 verified blocker。

### 14.8 是否会造成 CI secret 泄漏

已控制。public Fast CI 无 secret；真实 Provider 只在受保护环境运行，fork/untrusted PR 不得到 credential。

### 14.9 是否覆盖旧代码而不冻结全仓库

覆盖。全量确定性 inventory + 分批 AI coverage；未触碰未到期旧债不阻断无关变更，触碰时不得恶化，到期按 subject/domain 限制增长。

### 14.10 测试是否覆盖每个 Gate

通过。每 Gate 有定向测试、negative/mutation matrix；Gate G 有 default/all-features workspace、Clippy、历史 artifact 兼容和真实外部激活证据。

### 14.11 当前能否施工

已完成。Gate A-F、真实受信 Provider review、artifact 本地验证、EngineStrict 和 authoritative exact-commit Local CI 全部通过。

## 15. 当前结论

```text
施工文档：已生成。
施工文档自审：修订后通过。
队列状态：已完成 / 待归档。
施工授权：已取得并执行完毕。
归档施工：246，已移入已完成目录。
完成条件缺口：无。
下一动作：生成阶段完成记录、同步入口并归档本文。
```
