# 254-R2 当前施工文档：Gateway Real-Session Observer / Production Composition v1

> 正式方案：`254-R2-Production-Conformance-Candidate-Lifecycle-Real-Codex-Outcome-Acceptance-v1方案.md`
>
> 文档状态：`activated_construction_authorized`
>
> 队列状态：当前唯一施工文档。激活前复核已于 2026-07-21 通过，用户已单独授权 observer/composition 实现及非 production 测试，从 R2-O0 开始施工。
>
> 当前授权：R2-O0 至 R2-O7 的 observer/composition 代码修改及非 production 测试。仍不授权 fresh freeze、R2-N8、R2-G、production candidate、真实环境 activation、真实 Codex evaluation、F-A/F-B/G、三引擎 B 通道，亦不得重跑 R2-N6/N7/N7V/N7W。

### 激活前复核记录

- 正式方案第 9 节及 production conformance 合同未漂移，仍要求两个公共目标级 Module；observer/claim/journal/cursor 仅为 Implementation 内部 seam。
- 当前代码缺口与本文第 2 节一致：Gateway 尚无 attempt claim/journal，Native Editor 尚未组合 production evaluation Module。
- 工具链为 Rust/Cargo 1.96.0，激活时 HEAD 为 `9f5adc73ca02a5e3fed3f38c62bb9a420e6bd5de`。
- 工作树含用户既有修改且 `ai_tool_gateway` 为未跟踪源码；本施工使用作用域 source manifest/hash 固定基线，不清理、不回退、不覆盖范围外改动。
- fresh/production 授权继续独立，本文激活不构成 candidate/freezer/Gate 授权。

## 1. 施工目标

补齐 `CodexOutcomeEvaluationModule::evaluate(candidate, goal, riskEnvelope)` 的 production Implementation，使它能够观察同一 Editor-hosted Gateway 中真实连接的 MCP session，消费该 session 的实际 operation、Grant、receipt、project generation 与 evidence ownership，并由 Native Editor production composition root 持有。

本施工不新增第三个公共 Module，不改变两个公共目标级 Interface：

```text
ProductionCandidateModule::produce(request)
CodexOutcomeEvaluationModule::evaluate(candidate, goal, riskEnvelope)
```

实现完成只证明 production wiring 与非伪造观察链已经存在；不等于真实 Codex outcome acceptance 已执行或通过，也不自动授权新的 candidate/freezer/Gate。

## 2. 当前代码基线

已经存在：

- `CodexOutcomeEvaluationModule::evaluate` 与 opaque `EvaluationOperation::observe/cancel`。
- `CodexOutcomeRuntime` 的 activation、attempt、approval wait、terminal artifact、rollback/cleanup 骨架。
- `GatewayCore` 的 session/project binding、read generation、goal-level Grant、operation ownership 和 owner-thread dispatcher。
- Native Editor 持有 `GatewayCore`、`GatewayOwnerThreadClient/Dispatcher` 与 `EditorGatewayHost`。
- deterministic `TestExecutor` 可以测试 evaluation engine。

尚未存在：

- 只在 active evaluation 期间开启的 Gateway attempt observation journal。
- 将真实 Gateway facts 投影为 `AcceptanceEvent` 的 production Adapter。
- 对一个 eligible MCP session 的排他 attempt claim、cursor、overflow 和 terminal seal。
- production completion verifier 与 owner-thread 原子终态封存。
- externally-owned MCP/Codex session 的诚实 cleanup ownership schema。
- Native Editor 对 production `CodexOutcomeEvaluationModule` 的 composition 与 shutdown join。

当前 `CodexAttemptExecutor::start/collect/cleanup` 及 `ExternalCleanupEvidence.process_released` 只适合 deterministic executor 模型，不能直接改名为 production wiring。

## 3. 深 Module 与内部 seam

### 3.1 外部 Interface 不变

公共 caller 仍然只调用：

```rust
CodexOutcomeEvaluationModule::evaluate(candidate, goal, risk_envelope)
```

返回值仍只允许：

```text
EvaluationOperation::observe()
EvaluationOperation::cancel()
terminal disposition + durable evidence
```

禁止公开 `attach/start/collect/finalize/cleanup/advance`，禁止让 caller 提交 transcript、operation list、required tool list、stage selector 或内部 attempt identity。

### 3.2 production Adapter 是 Implementation 内部 seam

内部使用一个 real-session observer seam，并至少有两个 Adapter：

```text
GatewayRealSessionObserverAdapter   production
InMemorySessionObserverAdapter      deterministic tests
```

Adapter 可以在 Implementation 内部返回 opaque observation lease，以表达“先绑定 exact session、批准后观察”的真实时序；lease 不能进入 crate public Interface，行为测试仍以 `CodexOutcomeEvaluationModule` 为主要测试面。

### 3.3 不建立 completion 菜谱

不新增 `goal.complete`、`finalize_attempt` 或固定最后一步工具。AI 可以用任意合法工具顺序、额外 read 和动态重规划完成目标。

production evaluator 只在以下事实同时成立时尝试终态封存：

1. 当前 attempt claim 下所有已接受 operation 均已 terminal。
2. 最后一次 side effect 之后存在真实 final inspect。
3. project digest 与 read generation 在验证窗口内稳定。
4. `GoalBinding.completionPolicy` 的实际结果 verifier 通过。
5. owner thread 在同一原子操作中重新核对上述 expected digest/generation、封存 journal 并撤销 forward Grant。

任一事实未成立时继续观察；deadline 到期按真实证据返回 inconclusive/rejected，不伪造 outcome。原子封存后不允许该 attempt 再产生 mutation；合法计划不按 action count/order 验证。

## 4. 证据与 ownership 合同

### 4.1 observation journal

journal 只在 active attempt claim 期间记录结构化事实：

```text
attemptId
clientSessionId / MCP adapter instance identity
project identity / canonical root digest
monotonic sequence
requestId / invocationId / operationId
toolId / operation state
grant digest / receipt digest
before and after project digest
read generation
typed output kind
produced and consumed evidence refs
diagnostic code
event digest / previous event digest
```

规则：

- 事实由 owner-thread `GatewayCore` 在 dispatch/operation completion/close 时产生，不读取 caller transcript，不从日志文本反推。
- output 到 `AcceptanceEventKind` 的映射必须按 typed `AiToolInvocationPayload/AiToolOutput` 结构化匹配；禁止按 tool name substring、stdout 或 JSON 字符串猜测。
- journal 必须有总 event/byte 上限。overflow 立即 fail-closed 并产生 terminal diagnostic，不能丢旧事件后继续声称 evidence 完整。
- 没有 active claim 时不保留完整 Trace；正常 Gateway 热路径只承担有界的分支检查。
- claim 与 session/project/attempt 一对一；跨 session、跨 project、跨 attempt、旧 cursor 和重复 claim 全部拒绝。

### 4.2 externally-owned session

真实 Codex 及其 MCP stdio 进程由外部 client 所有，production evaluator 不启动、不 kill、不枚举裸 PID，也不把它们写成自己的 child process。

cleanup v2 至少区分：

```text
observationClaimReleased
forwardGrantRevoked
activationRollbackCompleted
ownerThreadTerminalSealCompleted
projectFinalStateVerified
externalSessionOwnership = externally_owned_observed
externalSessionConnectionClosed = observed fact, not cleanup requirement
```

`process_released=true` 不得继续代表 production cleanup。evaluation 负责释放自己拥有的 claim、Grant 和 activation；是否关闭外部 MCP connection 只能记录实际事实，不能伪造 ownership。

### 4.3 session selection

production Adapter 只能绑定：

- 当前 Editor Gateway 中 active、project/root 精确匹配的 MCP session；
- activation 之后观察到且尚未被其它 attempt claim 的 adapter instance；
- session read generation 与 goal initial digest 可建立因果关系；
- identity 不歧义的唯一 eligible session。

无 session 时有界等待；多 session 歧义、project drift、reconnect、session replacement 或 identity 缺失时 fail-closed。不得“任选第一个 MCP session”，不得把 Test client 标成真实 Codex。

## 5. 涉及文件

预计主要修改：

```text
rust/crates/ai_tool_gateway/src/core.rs
rust/crates/ai_tool_gateway/src/protocol.rs
rust/crates/ai_tool_gateway/src/codex_outcome_runtime.rs
rust/crates/ai_tool_gateway/src/codex_outcome_evaluation.rs
rust/crates/ai_tool_gateway/src/real_codex_acceptance.rs
rust/crates/ai_tool_gateway/src/lifecycle_identity.rs
rust/crates/ai_tool_gateway/src/lib.rs
rust/crates/ai_tool_gateway/src/real_session_observer.rs          new internal file if justified
rust/crates/ai_tool_gateway/tests/r2_goal_modules_public_schema.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/tests/mod.rs
rust/crates/editor_window_winit/src/tests/gateway_reconnect.rs
rust/crates/editor_host/tests/gateway_process.rs
```

按实现实际需要可增加同域测试文件，但不得修改 candidate producer、恢复 PowerShell Runner、加入 Codex launcher 或扩大到 R2-N8/R2-G。

## 6. 分 Gate 施工

### R2-O0：激活前基线与红测试

目标：先证明 production composition 缺失，并冻结当前 public Interface。

任务：

- 确认 remediation 文档已归档、本文件已从 `待执行/` 移到 `当前/`。
- 在隔离 clean worktree 或等价无混合变更基线中记录 exact commit、Rust/Cargo、Windows、target/output root 与已有 warning。
- 增加红测试：production constructor 不存在；Gateway 没有 claim/journal；Test Adapter 不能冒充 production；外部 session cleanup 不能用 process ownership 表示。
- 保留 compile-fail 测试，证明 stage-level lifecycle 未进入 public Interface。

定向测试：

```powershell
cd G:\gameEngin\rust
cargo test -p ai_tool_gateway --test r2_goal_modules_public_schema
cargo test -p ai_tool_gateway codex_outcome
```

通过条件：红测试只因上述缺口失败；若发现正式方案缺少必须的新公共概念，停止施工并先修正式方案。

恢复合同补充（2026-07-22）：本轮 O0 实际只建立了 dirty-worktree scoped source manifest，没有建立可 checkout 的 exact commit；它能检测直接文件漂移，但不满足“等价无混合变更基线”。2026-07-21 的激活复核因此只对代码/方案范围成立，对第 12 节第 4 项的 source-material 判断不成立。不得继续用 scoped manifest、历史 candidate snapshot 或当前脏工作树冒充 exact-commit 基线。

### R2-O1：Gateway attempt claim 与 journal

目标：让 owner thread 成为真实 observation facts 的唯一 producer。

任务：

- 增加 internal claim/batch/seal schema 与 canonical digest verifier。
- 在 connect/dispatch/accepted/terminal/cancel/close/project-drift 处记录有界事件。
- 保存 operation 的 exact session/grant/typed output lineage，terminal 后仍可由该 attempt cursor 消费，直到 claim seal/release。
- 实现 duplicate claim、cross-session、cross-project、cursor rollback、overflow、disconnect 与 reconnect rejection。
- 没有 active claim 时不生成完整 attempt Trace。

定向测试：

```powershell
cd G:\gameEngin\rust
cargo test -p ai_tool_gateway gateway_attempt_observation
cargo test -p ai_tool_gateway gateway_session_operation_ownership
cargo test -p ai_tool_gateway reconnect
```

### R2-O2：Real-session observer Adapter

目标：把 journal 的实际 typed facts 投影为 outcome evidence。

任务：

- 新增 crate-private `GatewayRealSessionObserverAdapter` 与 deterministic in-memory Adapter。
- 只绑定唯一 eligible MCP adapter instance；有界等待，不启动 Codex/MCP 进程。
- 结构化投影 Read、Mutation、Preview、VisualEvidence、Build、Delivery、Rollback、Diagnostic、FinalInspect。
- 校验 produced/consumed refs、Grant digest、receipt digest、operation ownership、project digest/read generation。
- journal overflow、缺 receipt、tamper、跨 attempt evidence、session replacement 全部 fail-closed。
- 两条不同合法计划完成同一 goal 均通过；额外合法 read 和重规划不被拒绝。

定向测试：

```powershell
cd G:\gameEngin\rust
cargo test -p ai_tool_gateway real_session_observer
cargo test -p ai_tool_gateway real_codex_acceptance
```

### R2-O3：Evaluation runtime 深化

目标：删除 deterministic executor 冒充 production 的可能，并保持一个目标级 Interface。

任务：

- 用 internal observer seam 替代 `CodexAttemptExecutor::start/collect/cleanup` 的 shallow lifecycle 语义。
- activation、attempt、approval、observation、completion verification、terminal seal、rollback 和 cleanup 继续由 `CodexOutcomeRuntime` 隐藏。
- 将 cleanup artifact 升级为 externally-owned session 语义；旧 schema 只读验证或明确拒绝，不能静默重写。
- cancel、approval decline/expiry、session disconnect、owner timeout、panic/lock poison 与 artifact persistence failure 都必须 single-terminal、join 并留下 durable evidence。
- terminal success 前由 owner-thread compare-and-seal 原子撤销 forward Grant；rollback authority 不依赖 forward Grant 仍有效。

定向测试：

```powershell
cd G:\gameEngin\rust
cargo test -p ai_tool_gateway codex_outcome_runtime
cargo test -p ai_tool_gateway production_evaluation_cleanup
cargo test -p ai_tool_gateway lifecycle_identity
```

### R2-O4：Native Editor production composition

目标：在真实 Editor-hosted Gateway composition root 中构造 production Module，但不执行真实 Codex acceptance。

任务：

- `NativeEditorApplication` 持有与当前 project/Gateway host 同生命周期的 production `CodexOutcomeEvaluationModule` 或其 owner。
- project open/change/close 时按 exact project identity/root 重建或撤销 composition；旧 module 不得跨项目存活。
- window shutdown 必须请求所有 evaluation operation 收敛、bounded join、释放 claim、撤销 Grant 并回滚 activation。
- production constructor 固定使用 `GatewayRealSessionObserverAdapter`；测试 Adapter 只能通过 `#[cfg(test)]` 路径注入。
- 暴露的仍是目标级 Module/operation handle，不公开 observer、claim、cursor 或内部 stage。

定向测试：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_window_winit production_evaluation_composition
cargo test -p editor_window_winit gateway_reconnect
cargo test -p editor_host --test gateway_process
```

### R2-O5：因果、并发与负向矩阵

目标：关闭真实 session 观察最容易被误判或伪造的路径。

必须覆盖：

```text
zero / one / multiple eligible sessions
session reconnect before and during attempt
project switch and external project digest drift
operation accepted before claim or after terminal seal
concurrent read and mutation completion ordering
duplicate/replayed operation or receipt
cross-session / cross-project / cross-attempt evidence
journal overflow and cursor rollback
Grant expiry during operation
cancel during approval / operation / final seal
final inspect before last side effect
stable digest with stale read generation
cleanup partial failure
two distinct legal plans for the same goal
extra legal reads and dynamic replanning
```

测试不能要求固定 tool list、count 或 total order，只验证身份、因果、预算、最终状态和 cleanup。

定向测试：

```powershell
cd G:\gameEngin\rust
cargo test -p ai_tool_gateway outcome_observation_negative
cargo test -p ai_tool_gateway outcome_plan_equivalence
cargo test -p editor_window_winit evaluation_shutdown
```

### R2-O6：受影响域回归

进入条件：O0-O5 全绿，累计失败集为空。

```powershell
cd G:\gameEngin\rust
cargo fmt --all -- --check
cargo check -p ai_tool_gateway --all-targets
cargo test -p ai_tool_gateway
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p runtime_cli
```

失败动作：先将新失败加入累计失败集，定向修复并重跑 O0-O5 与本 Gate；不得直接进入完整 workspace。

### R2-O7：环境等价预检与最终权威回归

进入条件：受影响域全绿；代码、manifest、lockfile、测试与施工文档施工段冻结；一次性 source-material consolidation 已形成新的本地 exact commit；该 commit 已在仓库外 clean worktree 中证明 source inventory 完整、工作树 clean，且无并行混合变更。

#### R2-O7-R：一次性 source-material consolidation 恢复合同

该恢复只修复本地 Git 工程真相，不新增引擎 Module、公共 Interface、production lifecycle 或 AI 工具外步骤，也不构成 fresh freeze、candidate、activation、attempt 或 R2-G 授权。

1. 先冻结当前主工作树的只读 path/hash/status inventory；不得清理、回退、stash、覆盖或暂存主工作树。
2. 使用独立临时 index 构造候选 tree：父树为当前 HEAD；只从当前工作树纳入经审计的正式源码、测试、Cargo manifest/lockfile、正式方案、施工文档与阶段记录。
3. 本次允许纳入根固定为 `README.md`、`rust/Cargo.toml`、`rust/Cargo.lock`、`rust/crates/**`、`框架设计/引擎总体架构/**`。新增路径必须先更新本合同并重新自审，禁止 `git add -A`。
4. 固定排除 `target/**`、`samples/**/Build/**`、`samples/**/Library/**`、当前 sample runtime 状态、`审查目录/**`、`tools/**`、历史 candidate/evidence 与任何外部输出根。排除只影响 consolidation tree，不删除或修改主工作树文件。
5. `AiFirstEngine` 保留父 index 的 mode `160000` 与 OID；必须重新证明嵌套 HEAD 等于父 OID、嵌套 repository clean。不得 fetch、clone、submodule update 或改写 gitlink。
6. 2026-07-17 的 254 frozen snapshot/source manifest 只作取证锚点，不能复用为 candidate，不能把其中 `mixed_worktree=true`、`exact_commit_ci=false` 的结果升级为当前权威证据。
7. 候选 tree 必须生成 include/exclude inventory、tree OID、parent OID 与 commit OID；`README.md`、Rust/Cargo 源码、manifest、lockfile 与测试路径的 `git diff --cached --check` 必须通过。正式 Markdown 文档中既有的双空格 hard-break warning 必须单独盘点并确认只属于 Markdown 换行语义；其它 whitespace error 仍 fail-closed，不得为过检机械改写历史文档。随后创建专用本地 `codex/254-r2-source-material-consolidation` 分支与单一 consolidation commit，不更新当前分支引用。
8. 从该 commit 创建仓库外 clean worktree；要求 `git status --porcelain` 为空、Gateway crate 已进入 Git index、Cargo workspace metadata 可解析、格式与受影响域预检通过。任一项失败即停在恢复阶段，修订 include/exclude inventory 后产生新的 commit；不得进入 workspace 权威矩阵。
9. 只有上述证据全部成立，才恢复本 Gate 的环境等价预检并运行一次 default/all-features 权威矩阵。consolidation 不是 O2-O6 重跑授权，也不能触发任何 production 行为。

环境等价预检必须记录：

```text
fresh clean exact-commit worktree
Windows toolchain and line endings
isolated CARGO_TARGET_DIR and artifact root
Gateway discovery root / port / named-pipe isolation
no production Codex config mutation
no real candidate / activation / attempt
no ignored real-acceptance test
```

预检一次通过后运行一次最终权威回归：

```powershell
cd <clean-worktree>\rust
cargo test --workspace
cargo test --workspace --all-features
```

如果 legacy-r1 feature 仍被矩阵带回，必须把 R2 public-schema/production-composition 结果单独列出；不能通过 suppression、删除 legacy tests 或把 test Adapter 改名 production 来过 Gate。

本 Gate 证明代码与 composition，不启动真实 Codex、不生成 production candidate，也不执行 R2-G。

## 7. 高风险测试清单

- `CodexOutcomeEvaluationModule` public Interface 无增长。
- owner-thread journal 完整性、上限和 event digest chain。
- operation/receipt/grant/project generation 的同 session lineage。
- externally-owned session cleanup 语义。
- owner-thread terminal seal 与 mutation dispatch 的竞态。
- project switch、Editor shutdown 与 worker join。
- reconnect 不转移 operation/attempt ownership。
- deterministic Adapter 无法进入 production constructor。
- 两种合法计划等价，不引入工具外菜谱。
- `legacy-r1` feature 不污染 R2 production composition。

## 8. 明确禁止

施工期间也禁止：

- shell/CLI 启动 Codex；
- parent PID、窗口标题或进程名冒充 Codex identity；
- caller 提交 transcript、receipt list 或 `outcomeAchieved`；
- 公开 observer/claim/cursor/stage Interface；
- 新增固定 action allowlist/count/order；
- post-hoc 裸 PID cleanup；
- 复用历史 candidate、activation、attempt 或 R2-G artifact；
- 运行 ignored real-Codex acceptance；
- 执行 R2-N8、R2-G、F-A/F-B/G 或三引擎 B 通道。

## 9. 完成条件

- production `CodexOutcomeEvaluationModule` 只能由真实 Gateway observer Adapter 组成。
- deterministic Adapter 只在测试配置可达。
- Native Editor composition 与 project/Gateway lifecycle 同步。
- actual Gateway facts 可形成可 reopen、可校验、不可跨 attempt 拼接的 outcome artifact。
- completion 由结果不变量和原子 terminal seal 判定，不要求 AI 执行菜谱。
- cleanup 诚实表达 externally-owned session，不伪造 process ownership。
- 定向、受影响域、环境等价预检和一次最终权威回归通过。
- 未执行真实 Codex evaluation，状态仍为 `real Codex outcome acceptance pending / new freeze authorization pending`。
- 完成后生成阶段完成记录，归档本施工文档，并同步 README、00、49、54、240、施工文档 README 与阶段完成记录 README。

## 10. 失败收敛

- 任一 Gate 失败先停在当前 Gate，不跨 Gate 堆积。
- 发现正式方案缺少真实外部 identity、terminal ownership 或 public Interface 决策时，停止代码施工，先修 254-R2 正式方案并重新自审本文件。
- owner journal 丢事件、overflow、terminal seal 竞态、cleanup 不完整或 artifact digest 失败一律 fail-closed。
- 权威回归失败先用结构化 artifact 定位并加入累计失败集；环境等价预检全绿前不得重跑权威回归。
- 不得用真实 Codex run 调试 production composition。

## 11. 施工文档自审

### 11.1 与正式方案一致性

通过。只补 254-R2 第 9.2 节已经要求的 Gateway real-session observer Adapter 与 production composition；不修改 candidate/activation/attempt 重试边界，不执行真实 acceptance。

### 11.2 Module depth

通过。公共 caller 仍只学习 `evaluate(candidate, goal, riskEnvelope)` 和 opaque operation。journal、claim、cursor、session selection、approval wait、completion verification、seal 和 cleanup 都在 Implementation 内。production 与 in-memory 两个 Adapter 使内部 seam 真实存在，但 seam 不泄漏到公共 Interface。

### 11.3 是否形成工具外菜谱

没有。施工测试验证两种合法计划、额外 read 与动态重规划；没有 required tool list/count/order，也不新增完成工具。严格顺序只存在于 owner-thread 原子 seal 和 cleanup 等工具内部安全步骤。

### 11.4 是否诚实处理外部 client ownership

通过。真实 Codex/MCP 不由引擎启动或拥有；cleanup 只证明 claim、Grant、activation 和 terminal seal 的释放/回滚。外部 connection close 只是观察事实。

### 11.5 是否会把 deterministic test 冒充 production

不会。O0 红测试、O4 production constructor 与 public-schema test 共同阻止 Test Adapter 进入 production composition。Editor/MCP process smoke 只证明 wiring，不命名为真实 Codex acceptance。

### 11.6 并发与证据完整性

通过。journal 有 monotonic sequence、digest chain、上限和 overflow fail-closed；terminal 由 owner thread compare-and-seal，避免 final validation 与新 mutation 之间的竞态。

### 11.7 验证层次

通过。文档分为定向测试、受影响域、环境等价预检和最终权威回归；高风险矩阵覆盖共享 Interface、跨 crate composition、历史 reconnect/cleanup 错误和环境隔离。

### 11.8 授权边界

通过。本文件已完成激活前复核并位于 `当前/`。代码授权只覆盖 observer/composition 实现及非 production 测试；没有创建 candidate/activation/attempt，也没有消费 fresh/production 授权。

### 11.9 O0/O7 恢复合同自审

通过。恢复合同只把已经要求的 exact-commit 前置条件改为可机械验证的 Git tree/commit/worktree 证据，没有修改两个公共目标级 Module，也没有增加 caller 或 AI 必须学习的流程。固定 allow roots 与 exclude roots 只约束一次性仓库整理，避免把截图、sample 生成物、外部审查材料和历史 Runner 混入 source truth；主工作树零扰动与专用分支保证用户改动可保留。whitespace 检查对代码/manifest/test 保持严格，对正式 Markdown 只豁免已盘点的语义 hard-break，不会用格式清理扩大历史文档改动。历史 frozen snapshot 仅作取证，明确禁止复用为 candidate。恢复成功后仍只执行原 R2-O7 的一次权威矩阵，因此没有扩大 production 授权或绕过 Gate。

## 12. 激活前复核

激活时必须重新确认：

1. remediation 施工文档已归档，`当前/` 为空。
2. 254 Core 第 30 节与 254-R2 第 9/11/14 节没有漂移。
3. `CodexOutcomeRuntime`、`GatewayCore`、Native Editor composition 基线与第 2/5 节相符。
4. 当前工作树可隔离出 clean exact-commit 基线。
5. O0 红测试仍准确，测试名称可由实际代码命名调整但语义不可缩水。
6. 用户明确授权“实现 real-session observer 与 production composition”；该授权不包括 fresh freeze 或真实 Codex evaluation。

复核已于 2026-07-21 对代码/方案范围通过，本文档已移动到 `施工文档/当前/` 并同步入口；2026-07-22 复查确认第 4 项当时仅由 scoped manifest 支撑、结论不足，现已由 R2-O7-R 恢复合同纠正。恢复合同自审通过后，必须先完成专用本地 consolidation commit 与 clean-worktree 证明，才能恢复 R2-O7。

## 13. 施工记录

### 13.1 R2-O0 completed

- 冻结 `254-R2-R2-O0-scoped-source-manifest-v1.txt`；基线 HEAD=`9f5adc73ca02a5e3fed3f38c62bb9a420e6bd5de`，Rust/Cargo 1.96.0。
- public schema 基线 3/3 通过，`codex_outcome` 基线 6/6 通过。
- `r2_production_observer_contract` 四项红测按预期失败，精确确认 claim/journal、observer Adapter、production composition、externally-owned cleanup 四个缺口；没有方案级新公共概念缺失。

### 13.2 R2-O1 completed

- Gateway owner thread 已拥有 MCP-only 排他 attempt claim、bounded journal、monotonic cursor、event digest chain 与 seal/release。
- journal 记录 bounded Gateway reply、terminal operation snapshot 与 session close typed facts；无 active claim 时不生成 attempt journal。
- Test session 冒充 production、duplicate claim、cursor rollback、overflow 与 incomplete seal 均 fail-closed。
- 定向测试 `cargo test -p ai_tool_gateway gateway_attempt_observation` 2/2 通过；`cargo check -p ai_tool_gateway --all-targets` 通过。

当时下一 Gate：R2-O2。后续状态见 13.3 至 13.8；fresh/production 授权仍未授予。

### 13.3 R2-O2 completed

- 新增 crate-private `GatewayRealSessionObserverAdapter`，只通过 owner-thread claim/read/seal 命令消费唯一 eligible MCP session 的 typed Gateway facts；不启动、不 kill、不关闭 externally-owned Codex/MCP。
- typed output 投影覆盖 Read、Mutation、Rollback、Preview、VisualEvidence、Build、Delivery、Diagnostic 与 FinalInspect；batch 校验 attempt、cursor、sequence、digest chain 与 journal head。
- duplicate/replayed operation、cross-project result、tampered event、错误 attempt/head/sequence 均 fail-closed；mutation/rollback receipt 校验 operation、Grant、project 与 canonical digest lineage。
- `real_session_observer` 3/3、`real_codex_acceptance` 定向回归通过；两种合法计划、额外 read 与动态重规划不依赖固定工具顺序。

### 13.4 R2-O3 completed

- `CodexAttemptExecutor` shallow lifecycle 已由 internal `CodexOutcomeObserver` seam 替代；activation、attempt、approval、observation、terminal artifact、rollback 与 cleanup 仍隐藏在 `CodexOutcomeRuntime`。
- cleanup schema 升级为 externally-owned session v2，不再声称 `process_released`；claim seal 可为拒绝、取消与错误路径提供 fail-closed owner terminal persistence。
- runtime 在 reopen candidate 后从 immutable release inventory 绑定 activation target digest，避免 composition 预先猜测 candidate inventory。
- `codex_outcome` 6/6、runtime/cleanup/lifecycle 定向回归通过。

### 13.5 R2-O4 completed

- `EditorGatewayHost::production_evaluation_module` 固定装配 `GatewayRealSessionObserverAdapter` 与 production runtime，没有 Test Adapter 注入路径。
- `NativeEditorApplication` 持有 `CodexOutcomeEvaluationModule`；project switch/close 时先撤销旧 composition，再重建 exact project Gateway host/module；外部只暴露目标级 Module 能力状态，不公开 observer/claim/cursor/stage。
- `production_evaluation_composition` 1/1、`gateway_reconnect` 1/1、`editor_host --test gateway_process` 2/2、`r2_production_observer_contract` 4/4 通过。

### 13.6 R2-O5 completed

- 负向矩阵覆盖 zero/multiple eligible sessions、reconnect ownership、project switch/drift、terminal 后 dispatch、duplicate/replay、cross-project/attempt、overflow、cursor rollback、cancel、cleanup partial failure与 final inspect ordering。
- `outcome_observation_negative` 2/2、`outcome_plan_equivalence` 1/1、`evaluation_shutdown` 1/1 通过。
- 测试只验证身份、因果、预算、最终状态和 cleanup；没有固定 tool list/count/order。

### 13.7 R2-O6 completed

- `cargo fmt --all -- --check` 通过。
- `cargo check -p ai_tool_gateway --all-targets` 通过。
- `cargo test -p ai_tool_gateway`：96 passed / 0 failed / 3 ignored；ignored 均为既有 construction-only fresh environment preflight，不是真实 acceptance。
- `cargo test -p editor_window_winit`：120/0；`editor_host`：21/0；`runtime_cli`：34/0。
- 回归中定位并修复 `activation_lease_compare_and_replace_failure_keeps_config_unchanged` 的并行 fixture 隔离缺口：Windows `SystemTime` 纳秒值不足以保证唯一 temp root，现叠加进程内 `AtomicU64` 序号；该定向测试连续 5 次通过，随后 `ai_tool_gateway` 及全部受影响下游回归再次全绿。

### 13.8 R2-O7 recovery and authoritative attempt 1 failed

- HEAD=`9f5adc73ca02a5e3fed3f38c62bb9a420e6bd5de`；当前有 138 个 tracked 修改、221 个 untracked 文件，且 `rust/crates/ai_tool_gateway` 的 tracked 文件数为 0。
- 无法形成不混入用户既有修改、同时完整包含本轮实现的 fresh clean exact-commit worktree。按 Gate 合同未运行 `cargo test --workspace` 或 `cargo test --workspace --all-features`，未创建 commit，未清理或回退用户改动。
- 证据见 `254-R2-R2-O7-environment-equivalence-preflight-report-v1.md`。
- 2026-07-22 按 R2-O7-R 从临时 index 创建本地 consolidation 分支；首个 commit `f7382d13ae838606709046e981c4ea46c224e197`、tree `011f91e0b9a2ad67d26feeed6eb2a78accc6a6cf`，父 commit 为上述 HEAD。commit 只改变 243 个 allow-root 路径，Gateway 39 个文件已进入 index，排除根没有进入 diff；主工作树保持原分支、原 HEAD、0 staged。
- 该 commit 的仓库外 clean worktree、Cargo metadata、格式、Gateway all-targets 与 O6 四个受影响 crate 回归均通过。第一次 O7 authoritative default matrix 于 129.293 秒 fail-fast：`editor_core/src/controlled_source_patch.rs` 的测试 fixture 初始化 `BoundedChildProcessResult` 时缺少新增 `ownership` 字段；all-features 未启动。
- 全仓 initializer 审计又发现 `project_e2e_gate/src/critical_correctness_safety.rs` 同类 fixture。两个测试 fixture 均补充默认 ownership evidence；`cargo test -p editor_core --lib`（543 tests，既有 local-only ignored 保留）与 `cargo test -p project_e2e_gate` 通过。测试生成的 sample Build/Library 漂移不进入修复 commit。
- 两处 schema migration 修复已形成 exact commit `3b359826f61eb9b0a47d0db30112e01a36086e20`。该 commit 的新 clean worktree 扩大累计预检通过 `cargo fmt --all -- --check`、Cargo metadata、workspace default/all-features all-targets check，但在受影响域回归中捕获两个依赖调度时序的并发测试：background request 用 `<50ms` 墙钟阈值判断异步返回，cancel test 则可能在 transport 真正进入阻塞前取消。两者均为测试同步缺口，不是 production controller 失败；因此第二次 O7 authoritative 矩阵尚未启动。
- fake HTTP server 新增仅供测试使用的 event gate：测试先确定 transport 已到达 server，再分别释放响应或执行 cancel；删除墙钟阈值和未建立 happens-before 的 abort 假设。`llm_background_request_returns_before_blocked_http_and_pumps_proposal` 与 `llm_cancel_drops_late_provider_result_and_duplicate_submit_is_rejected` 已各连续运行 5 次并全部通过；production controller 未修改。
- event-gated 修复形成 exact commit `113f29cdf865254fb3f7fcb929ca17e2a06ec906`、tree `517ab815fcc9dbf3049c4ad938ad427c5418068f`。全新 clean worktree 的扩大累计 preflight 在 888.3 秒内全部通过；preflight 后仅有 sample Build/Library 生成物漂移，sample 外 tracked/untracked 与 staged 均为 0。`AiFirstEngine` 由既有本地对象物化为 detached nested worktree，index gitlink、nested HEAD 均为 `e332e0ba6d5c1b7772de57ef69804eebe28ea415` 且 nested repository clean；没有 fetch、clone、submodule update 或 gitlink 改写。
- 第二次 O7 authoritative default matrix 在独立 worktree/target/TEMP/artifact 根执行并以 exit 101 失败，`editor_core` 汇总为 540 passed / 1 failed / 2 ignored；失败项为 `llm_request_controller_task_panic_becomes_failed_joined`，all-features 未启动。default log SHA-256=`1ab1323481eb222a4191dd11cba9da7dad9a5ee5ee6c7f8e99c345c01f6471b4`。失败 fixture 的 worker 立即 panic，但测试只以约 1 秒轮询窗口等待 worker 首次调度；在 full-workspace 并发负载下窗口可能先耗尽，热环境定向 baseline 10/10 通过，证明这是测试同步缺口而非 production controller 丢失 terminal event。
- panic fixture 现使用 test-only channel gate 建立 `worker entered transport -> test release -> intentional panic -> controller join` 的 happens-before；production controller 未修改。修复后该定向测试实际连续运行 10 次并全部通过。该修复必须先形成新 exact commit，并在新 clean worktree 完成累计失败集与受影响域预检，全部通过后才允许再次运行 authoritative 矩阵。
- 上述修复形成 exact commit `7cf6edf1713fe916116b2c03c2b4edd4ccc94a8c`、tree `e1b85a7a2a8f644747aaa55894956cac567012a3`；全新 clean worktree 的累计 preflight 在 926.6 秒内全部通过，包含 background 5/5、cancel 5/5、panic 10/10、workspace default/all-features all-targets check、`editor_core`、`project_e2e_gate` 与 O6 四 crate。preflight 后 sample 外 source/staged/nested 均 clean。
- 第三次 O7 authoritative default matrix 在独立 worktree/target/TEMP/artifact 根执行 717.243 秒后仍由同一 panic fixture 以 exit 101 失败，`editor_core` 汇总 540 passed / 1 failed / 2 ignored；all-features 未启动。default log SHA-256=`07048fdc5eb3c31da27be136978acd5efe0c0902af813216ade9c69e299b4542`。channel gate 已证明 worker 确实进入 transport 并执行 intentional panic；剩余延迟来自 `RUST_BACKTRACE=1` 下 panic hook/backtrace 完成前 Tokio task 尚未成为 finished，原测试因此错误地把预期 panic 日志生成速度纳入约 1 秒 controller 合同。
- test-only panic fixture 改用 `std::panic::resume_unwind`：仍产生 Tokio panicked `JoinError`，但不调用全局 panic hook，不再让 backtrace 输出成本决定 controller terminal 观察。event gate 保留，production controller 与公共 Interface 均不修改。该 fixture 已在 `RUST_BACKTRACE=1` 下实际连续执行 20 次并全部通过。该修复必须重新冻结 exact commit 并通过累计 preflight 后，才允许下一次 authoritative。
- hook-free fixture 形成 exact commit `d7a252f2a3ed20b7993faa04daeeed88669b241d`、tree `123f074c3cd614887b0ba8b31e22d105bb4bb8ba`。全新 clean preflight 的 fmt、workspace default/all-features all-targets check、background 5/5、cancel 5/5、hook-free panic 20/20、`editor_core` 与 `project_e2e_gate` 均通过，但随后 `cargo test -p ai_tool_gateway` 以 exit 101 停止；下游三个 O6 crate 未启动，authoritative 未创建。
- 该 preflight runner 没有为每个 stage 单独保留完整日志，聚合输出又被截断，因此无法恢复具体 Gateway 失败测试；不得凭摘要猜修复。相同 exact commit/target 的 Gateway diagnostic 随即 96/0/3 ignored 全绿，之后连续重复 5 次也全部通过，只能证明该失败是间歇性的，不能把本轮 preflight 升级为通过。下一轮 preflight 必须为每个 stage 写独立日志和 exit evidence；若失败直接从对应日志定位。
- 诊断记录形成 exact commit `c54c448664c5e2e9daf6d1e9ee23d8df106abb98`、tree `abb699342971e4fdcedc446ef0f4d8daf564460a`。逐 stage 日志化的全新 clean preflight 在 912.2 秒内 40 个 stage 全部通过；每个 stage 均记录 command、exit、duration 与 log SHA-256，无输出的 fmt stage 使用标准空文件 SHA-256。preflight 后 sample 外 source/staged/nested 均 clean。
- 第四次 O7 authoritative default matrix 在独立 worktree/target/TEMP/artifact 根执行 502.172 秒后 exit 101；失败项为 `codex_outcome_decline_timeout_and_cancel_are_cleaned_inconclusive`，Gateway 汇总 95 passed / 1 failed / 3 ignored，all-features 未启动。default log SHA-256=`6b1c871705180e6786269cd5365128268ad7ebe75054b483cbeaef5e522f7cc6`。这也确认上一轮未留日志的 Gateway 间歇性失败属于同一测试。
- Gateway runtime 的 production owner roundtrip 已使用有界 `recv_timeout`；失败只来自 test harness：spawn evaluation worker 后没有 readiness，立即用 `yield_now()` 忙轮询 2 秒。在 Windows full-workspace 负载下，worker 可能尚未开始或被 busy loop 争抢 CPU，harness 便以 `TryRecvError::Empty` 失败。test-only harness 现先通过 channel 确认 worker started，再 pump dispatcher；空轮询改为 1ms sleep，integration deadline 为 5 秒。production runtime 与公共 Module 不修改。
- 修复后的 Gateway harness 目标测试在 `RUST_BACKTRACE=1` 下实际连续执行 20 次并全部通过，证明 readiness 与调度边界不再依赖偶然 worker 启动时序。
- 同一修复工作树的完整 `cargo test -p ai_tool_gateway` 全绿：96 passed / 0 failed / 3 ignored，integration tests 与 doc tests 同步通过。该结果仅作为定向与受影响域验证；仍必须形成新 exact commit，并从全新 clean worktree 完成累计 preflight 后，才允许再次运行 authoritative matrix。
- Gateway harness 修复形成 exact commit `ece07f8546c46a565f88c56c8a0f950b9d50dcfa`、tree `196a5a8c397c6b159f96c8499d36a258b401dc2a`。全新逐 stage clean preflight 的 metadata、fmt、workspace default/all-features all-targets check、background 5/5、cancel 5/5、hook-free panic 20/20、Gateway harness 20/20 与完整 `editor_core` 均通过，但第 56 个 stage `cargo test -p project_e2e_gate` 在链接期因 `G:` 空间耗尽而 exit 101；后续四个 stage 与 authoritative matrix 均未启动。
- 失败日志 SHA-256=`a1b3a1d6456d4b266f74efe94be2dffc7074f11263e5eddb54b315045fab60fd`，同时包含 linker PDB `LNK1318 LIMIT`、`LNK1108` 与 rustc `no space on device`，证明这是隔离 target 容量不足而非 source/test assertion 失败。56-stage 证据已保留至 `H:\gameEngin-target\254-r2-o7-ece07f8-preflight-failed-nospace`，失败 outer/nested worktree 已注销；下一轮必须在有充足容量的 `H:` 上创建全新 outer/nested worktree、target、TEMP 与 artifact 根，不得复用部分构建 target。
- 环境失败记录形成 doc-only exact commit `795de2471d7264b6b0d89ff1908acc7c530cb517`、tree `5d27ffcc9eac85e17cbc92250aede6f593557b02`。其 `H:` fresh outer worktree 与 nested `AiFirstEngine` worktree 分别固定到上述 commit 和 gitlink OID `e332e0ba6d5c1b7772de57ef69804eebe28ea415`，两层 source/staged/nested 在预检前均 clean。
- `H:\gameEngin-target\254-r2-o7-795de24-preflight` 的 60-stage clean preflight 全绿，耗时 889.427 秒：metadata/fmt、workspace default/all-features all-targets check、background 5/5、cancel 5/5、hook-free panic 20/20、Gateway harness 20/20、`editor_core`、`project_e2e_gate`、`ai_tool_gateway`、`editor_window_winit`、`editor_host` 与 `runtime_cli` 全部 exit 0。`preflight-result.json` SHA-256=`c34a132e2adcb7f24d9dd5d9e834d512d23e29a7412445a45463cc4b92e3ff83`；预检后 unexpected source drift=0、staged=0、nested clean，仅有 sample Build/Library 生成漂移。
- `H:\gameEngin-target\254-r2-o7-795de24-authoritative` 使用另一个 fresh exact outer/nested worktree 与全新 target/TEMP/artifact 根执行唯一一次权威矩阵。`cargo test --workspace` exit 0，1608 passed / 0 failed / 8 ignored，耗时 828.794 秒，log SHA-256=`52c1a89d687787a29ecf1c37791e0d93fa37929a88618b51b4ba4527b0cc2f19`；随后且仅随后执行 `cargo test --workspace --all-features`，exit 0，1660 passed / 0 failed / 17 ignored，耗时 1106.507 秒，log SHA-256=`9d7c9b2642179920f5f56e20ec0922edcd33997d3c5bf988ef80ebfc85d792cf`。`authoritative-result.json` status=`passed`、总耗时 1935.528 秒、SHA-256=`5e19d062d71ca13956498f6fb8c1d5b200869d54bd3bef81c7ea5a105b8536dc`；运行后 staged=0、nested clean，仅有 sample Build/Library 生成漂移。
- R2-O7 完成期间未执行 production candidate、activation、真实 Codex evaluation、R2-N8、R2-G、F-A/F-B/G 或三引擎 B 通道。四次 authoritative default 失败与一次 storage preflight 失败均按 first-failure fail-closed，all-features 只在最终 default exit 0 后执行一次；没有重试任何 production attempt。

当前状态：R2-O0 至 R2-O7 全部完成；Gateway real-session observer Adapter 与 Native Editor production composition 已通过累计定向、受影响域、60-stage clean preflight 和唯一成功的 default/all-features authoritative matrix。本施工文档可以生成阶段完成记录并归档。fresh/production 仍为 `new freeze authorization pending`；R2-N8、R2-G、production candidate、activation 与真实 Codex evaluation 均未授权、未执行。
