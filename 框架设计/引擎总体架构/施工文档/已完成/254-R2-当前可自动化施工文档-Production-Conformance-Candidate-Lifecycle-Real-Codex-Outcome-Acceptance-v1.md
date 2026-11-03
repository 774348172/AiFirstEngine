# 254-R2 当前施工文档：Production 证据合同收敛与实现修复 v3

> 正式方案：`254-R2-Production-Conformance-Candidate-Lifecycle-Real-Codex-Outcome-Acceptance-v1方案.md`
>
> 当前授权：只修订 254 Core/254-R2/当前施工文档和已确认的实现偏差，并运行定向与受影响域测试。禁止执行任何 fresh preflight、authoritative Gate、production candidate、activation、真实 Codex evaluation、F-A/F-B/G 或三引擎 B 通道。
>
> 当前状态：`r2_contract_and_implementation_remediation_completed_archived`

## 1. 目标

保留两个公共目标级深 Module：

```text
ProductionCandidateModule::produce(request)
CodexOutcomeEvaluationModule::evaluate(candidate, goal, riskEnvelope)
```

本轮只修复以下偏差：

1. candidateId 的 staging 预分配与 publish/reopen 后生效语义不一致。
2. cleanup receipt 通过 error 字段推断进程已回收，并在 digest 失败时写全零值。
3. headless release Gateway smoke 被写成 actual production Editor conformance。
4. PowerShell Runner 形成第二套裸 PID ownership Implementation。
5. source snapshot 把所有 source 都标成 project material。
6. `CodexOutcomeEvaluationModule` 只有 deterministic executor，尚无 Gateway real-session observer production wiring，但施工记录曾声明 production runtime 已完成。
7. legacy R1 被 R2 authoritative `--all-features` 矩阵无差别带回。

本轮不补真实 Codex observer Adapter。该 Adapter 需要单独的 schema、Gateway event/receipt inventory 与施工文档；不得用 shell/CLI launcher 或公开 `start/collect/cleanup` 菜谱 Interface 临时代替。

## 2. 不变量

- caller 仍只学习两个目标级 Interface；内部 preparation/seal/activation/attempt 不公开。
- AI 自主选择工具、次数、顺序和合法重规划；validator 只检查实际因果关系和结果不变量。
- candidate/package/activation/attempt 身份继续分离。
- user approval 继续由 Native Editor/Gateway authority 持有。
- 历史 R2-G、R2-N6/N7/N7V/N7VR/N7W artifact 保留原终态，不改写为 passed。
- 任何 cleanup 证据都不能依赖 post-hoc 裸 PID 枚举。
- 本轮测试不能运行 ignored fresh harness 或生产环境 side effect。

## 3. 历史终态

| 历史阶段 | 终态 | 当前含义 |
|---|---|---|
| R2-N0..N5 | 已执行 | 证明两个公共 Interface 与内部 deterministic engine 基线，不证明 production real-session wiring |
| R2-N6 | failed | Gitlink source-material contract 缺失；禁止重跑 |
| R2-N7 | failed | remediation 后验证未闭合；禁止重跑 |
| R2-N7V | failed | PowerShell Runner 未取得可靠 exit code；禁止重跑 |
| R2-N7VR | passed | 只证明当时 Runner fixture；不证明产品或 N7 passed |
| R2-N7W | failed | 裸 PID residual identity unproven；禁止重跑 |
| R2-N8 | 未执行 | 原条件未成立；旧 PowerShell Runner 已退役 |
| 历史 R2-G | failed | invalid snapshot/evidence lost；禁止重跑或复用 |

上述内容只作历史索引，不是可执行步骤。详细证据继续由当前目录中的冻结 JSON report 保存。

## 4. 当前施工范围

### 4.1 正式合同

修订：

- `254-AI-Tool-Gateway-Codex-Adapter-v1方案.md` 第 30 节。
- `254-R2-Production-Conformance-Candidate-Lifecycle-Real-Codex-Outcome-Acceptance-v1方案.md`。

合同必须明确：

- candidateId 可在 staging 预分配，但 publish/reopen 前不生效。
- conformance 分为 source contract regression、sealed release headless smoke、real Editor/Codex acceptance。
- Windows child tree 使用 Job Object/handle receipt；直接 child 使用持有的 Child handle/wait receipt；PID 仅作诊断。
- cleanup digest 失败 fail-closed。
- deterministic executor 只证明 evaluation engine，不是 production wiring。

### 4.2 bounded process 与 failure receipt

涉及：

```text
rust/crates/runtime_cli/src/bounded_child_process.rs
rust/crates/runtime_cli/src/lib.rs
rust/crates/ai_tool_gateway/src/production_failure.rs
rust/crates/ai_tool_gateway/src/production_candidate.rs
```

实现要求：

- `BoundedChildProcessResult` 记录 ownership kind、group creation、root bind、termination request、root wait、group release、reader join。
- cleanup 只消费上述 receipt，不从 `kill_error == None` 推导成功。
- spawn failure 与 bind/start failure 分开保留真实 process identity/cleanup 事实。
- failure artifact schema 升版并绑定 ownership evidence digest。
- 不写全零 cleanup digest。

### 4.3 conformance 命名与真实路径

涉及：

```text
rust/crates/ai_tool_gateway/src/production_conformance.rs
```

实现要求：

- `production_editor_path` 改为 `release_gateway_headless_smoke`。
- report schema 升版，旧 report 不得被新 verifier 当成同一合同。
- smoke 只执行正式 release Editor/MCP binary 的 read-only typed MCP 路径。
- smoke 必须有直接 child handle/wait 或 bounded ownership receipt。
- 不声明真实 window、mutation、Preview、visual、Build/Delivery 已通过。

### 4.4 source ownership

涉及：

```text
rust/crates/ai_tool_gateway/src/source_snapshot.rs
```

实现要求：

- 新 receipt 使用 `producer_selected_source`，不把 engine/fixture/tool 一律称为 project material。
- v1/v2 receipt 继续只读验证，不能被静默重写。
- tracked/untracked、Git mode/OID、gitlink lineage 继续独立记录。

### 4.5 旧 Runner 退役

涉及：

```text
tools/r2_n7v_validation_runner.ps1
tools/r2_n7v_process_control.ps1
tools/tests/r2_n7v_process_control_regression.ps1
tools/r2_n7w_prepare.ps1
tools/r2_n8_fresh_source.ps1
tools/r2_n8_preflight_runner.ps1
```

全部遗留 PowerShell 入口只保留 fail-fast retired guard。历史 report 不删除；任何未来 fresh preflight 必须使用 Rust production harness、普通进程退出码与 `runtime_cli` ownership receipt，并先获得新授权。

### 4.6 Production evaluation wiring 的诚实状态

现有 `codex_outcome_runtime.rs` 和 deterministic `TestExecutor` 保留为内部 evaluation engine 测试。当前不得新增以下伪修复：

```text
public CodexAttemptExecutor lifecycle recipe
shell/CLI spawned Codex
caller supplied transcript/finalization artifact
test Adapter renamed production Adapter
```

本轮完成后状态必须仍为：

```text
evaluation engine implemented
Gateway real-session observer Adapter pending
production constructor/composition pending
real Codex outcome acceptance not executed
```

下一份独立施工文档必须先定义 observer 如何从同一 Gateway session 收集 operation、receipt、Grant、project generation、evidence ownership 与 terminal cleanup，再实现 production composition。

## 5. 验证

### 5.1 定向测试

```powershell
cd G:\gameEngin\rust
cargo test -p runtime_cli bounded_child_process
cargo test -p ai_tool_gateway production_failure
cargo test -p ai_tool_gateway source_snapshot
cargo test -p ai_tool_gateway production_conformance
cargo test -p ai_tool_gateway codex_outcome
```

不得添加 `--ignored`，不得设置任何 `AIFE_R2_N8_*` 或 production activation 环境变量。

### 5.2 受影响域

```powershell
cd G:\gameEngin\rust
cargo test -p runtime_cli
cargo test -p ai_tool_gateway
cargo check -p ai_tool_gateway --all-targets
cargo fmt --all -- --check
```

`ai_tool_gateway --all-features` 不属于 R2 当前权威矩阵，因为它会启用 `legacy-r1`。legacy compatibility 后续单独运行和报告；不得用其失败/通过替代 R2 当前合同。

### 5.3 明确不运行

```text
workspace default/all-features authoritative regression
R2-N6/N7/N7V/N7W
R2-N8 ignored fresh preflight
R2-G
ProductionCandidateModule::produce on production source
CodexOutcomeEvaluationModule::evaluate on real environment
F-A/F-B/G
three-engine B channel
```

## 6. 完成条件

- 正式方案与实现使用同一身份、conformance 和 cleanup 语义。
- cleanup tests 证明 incomplete ownership receipt 不能晋升 clean。
- source v3 receipt 与 v1/v2 compatibility tests 通过。
- conformance report 只出现 `release_gateway_headless_smoke`。
- 全部旧 N7V/N7W/N8 PowerShell 执行入口无裸 PID 枚举或 `Stop-Process`。
- 文档不再声明 production evaluation wiring 已完成。
- 定向与受影响域测试通过。
- 入口文档同步为 remediation 完成、real-session observer/new freeze authorization pending。

## 7. 自审

### 7.1 Module depth

通过。公共 Interface 未增加；ownership、conformance layers 与 source classification 都留在 `ProductionCandidateModule` Implementation。真实 client/session observer 也必须是 `CodexOutcomeEvaluationModule` 的内部 Adapter。

### 7.2 是否形成工具外菜谱

没有。本文档步骤只约束施工和测试，不是 AI runtime protocol。真实 Codex 仍按 goal 和工具结果自主规划。

### 7.3 是否增加重复流程

没有。旧 PowerShell process Implementation 被退役，权威事实回到已有 Rust bounded-process primitive。没有新增公共 Module、Resolver Registry 或 Gate lifecycle。

### 7.4 是否诚实反映完成度

是。N3 只证明内部 evaluation engine；production Gateway real-session observer 和 composition root 明确 pending。headless smoke 也不再冒充真实 Native Editor acceptance。

### 7.5 授权边界

本轮授权在文档、实现偏差和非 ignored 测试完成后消费并停止。任何 fresh 或 production 行为都需要新的明确授权与新的施工文档。

## 8. 当前执行记录

状态：`r2_contract_and_implementation_remediation_completed`。

完成内容：

- 254 Core 第 30 节、254-R2 正式方案与本施工文档已统一为两个公共目标级深 Module。
- candidate publish identity、三层 conformance、producer-selected source、process ownership receipt 与 fail-closed cleanup digest 已按正式合同修复。
- headless release Gateway smoke 不再冒充真实 Native Editor/Codex acceptance。
- deterministic evaluation engine 明确不等于 production wiring；Gateway real-session observer Adapter 与 production composition 仍 pending。
- 旧 N7V/N7VR/N7W/N8 PowerShell 执行入口已全部退役为 fail-fast guard。
- 修复 v2 conformance required-ID 清单未按 canonical order 排列、导致合法 report 被 verifier 自拒绝的问题，并增加顺序回归测试。

验证结果：

```text
cargo test -p runtime_cli --test bounded_child_process: 7 passed
cargo test -p ai_tool_gateway production_failure: 2 passed
cargo test -p ai_tool_gateway source_snapshot: 17 passed
cargo test -p ai_tool_gateway production_conformance: covered by final gateway suite; 2 matching tests passed
cargo test -p ai_tool_gateway codex_outcome: 6 passed
cargo test -p runtime_cli: 34 passed across all test binaries
cargo test -p ai_tool_gateway: 103 passed across all test binaries; 3 ignored and not run
cargo check -p ai_tool_gateway --all-targets: passed
cargo fmt --all -- --check: passed
```

保留 warning：`engine_runtime` 3 条既有 dead-code warning；`ai_tool_gateway` 的 pending activation/evaluation/acceptance production composition 仍产生 dead-code warning。未 suppression，因为它们如实显示 real-session wiring 尚未完成。

明确未运行：所有 `--ignored`、workspace authoritative/default/all-features、`ai_tool_gateway --all-features`、R2-N6/N7/N7V/N7W/N8、R2-G、production candidate/activation/真实 Codex evaluation、F-A/F-B/G 与三引擎 B 通道。未创建 candidateId、activationId 或 attemptId。

下一步唯一入口：单独编写并自审 Gateway real-session observer Adapter 与 production composition 施工文档。状态为 `new freeze authorization pending`；在新授权前不得执行 fresh freeze、R2-N8 或 R2-G。
