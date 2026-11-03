# 254-R2 当前可自动化施工文档 - Fresh Production Candidate v1

> 文档状态：`r2_fc6_terminal_failed_contract_revised_new_construction_document_pending`
>
> 激活日期：2026-07-22
>
> 用户决策：采用两段目标级授权方案 B；仅授权第一段 Fresh Candidate。
>
> 当前施工状态：R2-FC0 至 R2-FC5 已完成；R2-FC6 已精确执行一次并以 `source_snapshot.file_set_mismatch` terminal failed，owner/module invocation count 均为 1、retry count 为 0、candidateId 不存在且 cleanup 完整。后验审查已确认 immutable snapshot 被 released Editor 当作 writable project root；用户已确认方案 2 并完成正式合同修订。本施工文档保持 terminal failed，不能作为 remediation 施工入口；FC7 未授权且不得执行。
>
> 硬边界：不得创建 activationId/attemptId，不得调用 `CodexOutcomeEvaluationModule::evaluate`，不得连接或启动真实 Codex，不得执行旧 R2-N8/R2-G、F-A/F-B/G 或三引擎 B 通道。

## 1. 施工目标

在不增加第三个公共生命周期 Interface、不暴露内部 production stages 的前提下：

1. 为现有 `ProductionCandidateModule::produce(request)` 增加一个 production-owned 薄 Adapter；
2. Adapter 只严格解码一个声明式 `ProductionCandidateRequest`、调用 Module 一次，并原子写出 canonical `CandidateDisposition`；
3. 在全新 clean exact-commit source 上完成分层验证；
4. 只执行一个真实 Fresh Candidate production operation；
5. 成功时得到可由 `CandidateRegistryModule::reopen` 验证的 sealed candidate，失败时得到 durable failure artifact 且没有 candidateId；
6. 完成后停止在 `Real Evaluation authorization pending`。

本阶段不评价 AI，不修改项目，不使用 Gateway session，也不消费 goal-level Grant。

## 2. 当前证据与实现缺口

已成立：

```text
ProductionCandidateModule::production().produce 已具有 source snapshot、release、conformance、seal/reopen 与 cleanup 实现，但 R2-FC6 证明其内部 mutability/ownership composition 不符合修订后的正式合同。
OID-authoritative、local-only、clean gitlink expansion 已实现并通过回归。
observer/composition R2-O0 至 R2-O7 与 authoritative default/all-features 已通过。
两个公共目标级 Module 的 schema 与边界已冻结。
```

本施工开始时的缺口是 production owner 缺失；R2-FC1 已补齐该 owner。R2-FC6 后确认的新缺口是：

```text
snapshot/samples/complex_shooter_project
-> released editor_host --gateway-process-preflight 直接打开该路径
-> ProjectIntentWorkflow 写入 Library/ProjectIntent/journal.json
-> SourceSnapshotProducer::reopen 发现 extra file
-> source_snapshot.file_set_mismatch
```

正式方案现要求 `ImmutableSourceSnapshot` 与 `DisposableConformanceWorkspace` 为两个 crate-private capability role；released Editor 只能打开 workspace project root。当前代码尚未按新合同施工。本轮全部历史错误、证据 digest 与 R2-G evidence gap 已归档到 `../../254-R2-历史失败归档-2026-07-22.md`。

## 3. 架构边界

允许新增的薄 Adapter：

```text
editor_host --production-candidate-request <absolute-json>
            --production-candidate-result <fresh-absolute-json>
```

名称可在红测试后按现有 argument parser 习惯微调，但语义必须保持：

```text
strict JSON decode -> request.verify -> ProductionCandidateModule::production().produce exactly once
-> independently reopen terminal artifact -> atomic canonical result write -> deterministic exit
```

Adapter 不得接受或暴露：

```text
stage selector / stage order / next action
retry count / per-stage retry
submodule path / exclude list / fetch policy
cargo command / manifest fragments
candidateId 预分配
activation/evaluation/Grant/attempt 字段
```

内部 snapshot、release、conformance、seal、reopen、failure persistence 与 cleanup 继续完全归 `ProductionCandidateModule` 所有。

## 4. 施工基线与工作树

文档激活基线：

```text
branch: codex/254-r2-source-material-consolidation
commit: 21606d0f7f48ce5f2307f38e2211cc6145e028e6
tree: 8ca5c21cb24a0982d5657a41ce2cd5241ec2a358
gitlink mode: 160000
gitlink OID: e332e0ba6d5c1b7772de57ef69804eebe28ea415
```

该 consolidation 工作树保留 sample `Build/**` tracked drift 与 `Library/**` untracked 生成物，不得清理、暂存或作为 clean source。代码施工必须在仓库外新建专用 clean worktree/branch；nested `AiFirstEngine` 必须按 gitlink OID 本地展开且 clean。production source identity 只能在实现与回归冻结后确定，不能把本激活基线冒充最终 candidate source。

## 5. 预计涉及文件

```text
rust/crates/editor_host/src/main.rs
rust/crates/editor_host/tests/gateway_process.rs
rust/crates/ai_tool_gateway/src/production_candidate.rs（仅当独立 reopen/result helper 确需下沉）
rust/crates/ai_tool_gateway/tests/r2_goal_modules_public_schema.rs
```

按实际需要可增加同域测试文件，但不得修改 `CodexOutcomeEvaluationModule`、activation、real-session observer 或项目 mutation 代码。

## 6. 分 Gate 施工

### R2-FC0：隔离基线与红测试

1. 建立全新 clean outer/nested worktree、独立 `CARGO_TARGET_DIR`、TEMP 与 artifact root。
2. 记录 exact commit/tree/gitlink、source/staged/nested clean 状态。
3. 新增红测试证明：严格 request、未知字段拒绝、路径/资源包络验证、result 路径必须 fresh、Module 调用精确一次、sealed/failed 两终态、失败没有 candidateId、禁止 evaluation 字段。
4. 证明旧 `--candidate-freeze-preflight` 继续 fail-closed，旧 R2-N8 不成为入口。

定向命令：

```text
cargo test -p editor_host --test gateway_process production_candidate -- --nocapture
cargo test -p ai_tool_gateway --test r2_goal_modules_public_schema -- --nocapture
```

预期成本：小于 5 分钟。红测试必须因 owner 缺失而失败；若其它基线失败，先停止诊断，不进入 R2-FC1。

R2-FC0 于 2026-07-22 在独立 outer/nested worktree、`CARGO_TARGET_DIR`、TEMP 与 evidence root 中完成。最小修改前基线为旧 freezer fail-closed 1/1 通过、公共 Module schema 3/3 通过；新增红测试后，`editor_host` production candidate 定向矩阵 0/3 通过，三个失败均收敛到尚未识别 `--production-candidate-request` 的同一个 owner 缺口，公共 Module schema 4/4 继续通过。未修改 production 实现，未创建 candidate、activationId 或 attemptId，未调用真实 Codex。机器证据见 `254-R2-R2-FC0-baseline-red-test-report-v1.json`。

### R2-FC1：薄 production owner 实现

实现 strict request/result Adapter，并满足：

```text
request bytes 与 SHA-256 进入 result lineage；
只构造 ProductionCandidateModule::production()；
只调用一次 produce；
sealed 时再次 reopen candidate root，核对 candidateId/manifest/package digest；
failed 时 reopen durable failure artifact，核对 operation/digest/cleanup；
result 使用 fresh sibling temp + atomic replace；
输出已存在、写入失败或独立 reopen 失败均 fail-closed；
日志有界，不输出 credential 或 source 内容；
退出码与 terminal disposition 一致。
```

不得新增 caller-visible production stage enum、公共 runner Module 或固定步骤清单。

R2-FC1 于 2026-07-22 完成。`editor_host` 只新增 request/result 两个参数，具体 strict decode、canonical request lineage、一次 Module 调用、sealed/failed terminal reopen、fresh sibling temp 与 Windows write-through atomic publish 均收敛在 crate-private `production_candidate_owner` Adapter Module；公共 lifecycle 仍只有既有两个目标级 Module。FC0 累计 owner 测试 3/3 通过，公共目标 Module schema 4/4 通过。测试只生成并清理 invalid-source failed disposition，没有启动 FC6 真实 Fresh Candidate production operation，也没有创建 activationId/attemptId 或连接真实 Codex。机器证据见 `254-R2-R2-FC1-production-owner-report-v1.json`。

### R2-FC2：定向与负向矩阵

```text
cargo test -p editor_host --test gateway_process -- --nocapture
cargo test -p editor_host --bin editor_host -- --nocapture
cargo test -p ai_tool_gateway --test r2_goal_modules_public_schema -- --nocapture
cargo test -p ai_tool_gateway production_candidate -- --nocapture
```

负向矩阵至少覆盖：未知字段、相对路径、source/resource overlap、work/output overlap、已有 result、invalid source durable failure、result tamper/reopen failure、重复参数、evaluation/activation 字段注入。

R2-FC2 于 2026-07-22 完成。补齐相对 result、重复 result、缺少配对参数、mode conflict、source/work 与 work/output overlap、cleanup receipt、临时 result 清理和 durable failure artifact tamper/reopen 负向覆盖；四组规定测试最终分别为 7/7、18/18、4/4 和 production-candidate 相关 3/3 通过。首次完整矩阵仅因测试错误地要求整个 `workRoot` 不存在而失败；正式合同要求 cleanup receipt 指向的 owned staging root 消失，修正测试断言后累计失败用例与完整矩阵均通过，production 实现未变。未启动真实 Fresh Candidate production operation，未创建 candidateId/activationId/attemptId，未调用 evaluation 或真实 Codex。机器证据见 `254-R2-R2-FC2-directed-negative-matrix-report-v1.json`。

### R2-FC3：受影响域回归

进入条件：R2-FC2 全绿。

```text
cargo test -p ai_tool_gateway
cargo test -p editor_host
cargo test -p editor_window_winit
cargo test -p project_e2e_gate
cargo fmt --all -- --check
```

新增 warning/suppression 必须先运行 inventory 并解释；不得用 `allow` 隐藏 owner 缺口。

R2-FC3 于 2026-07-22 完成。首次 `ai_tool_gateway` 默认并行回归两次分别在 604 秒和 184 秒超时；单线程 lib 96/96、排除 named-pipe 90/90、排除 discovery 93/93、named-pipe 单组 6/6 的诊断矩阵将问题收敛为 Windows named-pipe accept rearm shutdown race：server 越过 shutdown 检查但尚未创建下一 pipe instance 时，单次 wake 会过早失败并让 `join()` 无限等待。修复为五秒有界的重复 cancel/wake，并增加 64 轮 shutdown race 回归。之后五组规定命令全部通过：`ai_tool_gateway` lib 97/97、integration 11/11、doctest 8/8；`editor_host` 26/26；`editor_window_winit` 120/120；`project_e2e_gate` 65/65（1 个真实窗口 local gate 按既有合同 ignored）；格式检查通过。未新增 warning/suppression，测试生成的 sample `Library/` 已清理。未启动真实 Fresh Candidate production operation、evaluation 或真实 Codex。机器证据见 `254-R2-R2-FC3-affected-domain-regression-report-v1.json`。

### R2-FC4：环境等价预检与代码冻结

1. 形成包含实现与测试的 exact commit。
2. 从该 commit 创建新的 clean outer/nested worktree；不得复用 R2-O7 target、TEMP、artifact 或 sample-mutated worktree。
3. 使用 `H:` 上互不重叠的 source、target、TEMP、preflight artifact 根。
4. 一次性覆盖 owner 累计失败集、source/gitlink、release、conformance、failure persistence、cleanup 和 editor host 参数矩阵。
5. 预检通过后冻结代码；除完成记录/入口同步外，任何代码、manifest、lockfile、test 或 ledger 变化都使预检失效。

R2-FC4 于 2026-07-22 完成。包含实现与测试的冻结 commit=`35e88d3f3a6e888d8db3b79606ea161f7de07ff8`、tree=`2f2e6d2063e1fcefbf38cb3bd64aec5e0e259feb`、gitlink=`e332e0ba6d5c1b7772de57ef69804eebe28ea415`。在 `H:` 上互不重叠且全新的 outer/nested source、`CARGO_TARGET_DIR`、TEMP 与 evidence 根中，64-stage 累计预检全部 exit 0，耗时 919.830 秒；覆盖 O7 的 60-stage 累计失败集以及 FC owner 的完整参数矩阵、binary、公共 schema 与 production domain。`preflight-result.json` SHA-256=`fdec240706d723eed9ee0e536e1f1f2aad22d4b99d7620c28c92c554a6f002cf`。运行后仅出现已分类的 sample Build/Library 生成物，恢复/清理后 outer working tree、staged、nested 均 clean，且没有 owned orphan process。前两组 fresh root 分别因外层 10 秒执行上限和 PowerShell stderr 捕获方式中断，均未产生测试失败、未复用 partial target；第三组修正 runner 后从 fresh roots 完整通过。未启动 R2-FC5、真实 Fresh Candidate production operation、evaluation 或真实 Codex。机器证据见 `254-R2-R2-FC4-environment-equivalence-code-freeze-report-v1.json`。

### R2-FC5：最终权威回归

进入条件：R2-FC4 全绿且 exact commit 未漂移。

在另一个 fresh exact outer/nested worktree 与全新 target/TEMP/artifact 根顺序执行一次：

```text
cargo test --workspace
cargo test --workspace --all-features
```

只有 default exit 0 才允许启动 all-features。任何失败都停止本轮，不启动 production operation；修复后必须回到 R2-FC4，不在权威矩阵中碰运气。

R2-FC5 于 2026-07-22 完成。在另一组 fresh exact outer/nested worktree 与全新 `CARGO_TARGET_DIR`、TEMP、artifact 根中执行唯一一次权威 runner：`cargo test --workspace` 先以 1615 passed / 0 failed / 8 ignored、exit 0 通过，耗时 901.965 秒；仅随后启动 `cargo test --workspace --all-features`，以 1667 / 0 / 17、exit 0 通过，耗时 862.324 秒。总耗时 1764.517 秒；default/all-features 日志 SHA-256 分别为 `678852b3377e267f79507ce7f00bc621fcea52d7d55d9735946976dacb11a5fa` 与 `a3e85d6337a167e99316346d8d5bc1bda7eb90f810ddab2de3170d2cd5c32aba`，`authoritative-result.json` SHA-256=`b3677c69fec4c59d2caf3452799ffcfbb441c30346881918e28731bfdc901cf2`。运行后已恢复/清理 sample Build/Library 生成物，outer working tree、staged、nested 均 clean，owned orphan process=0。未启动 R2-FC6、production candidate、evaluation 或真实 Codex。机器证据见 `254-R2-R2-FC5-final-authoritative-regression-report-v1.json`。

### R2-FC6：一次 Fresh Candidate production operation

进入条件：R2-FC5 全绿、exact commit/tree/gitlink 与 producer binary digest 已冻结、production source outer/nested clean、request/result/work/output roots 均为全新绝对路径且位于 source 外。

声明式 request 只包含正式 schema 字段：

```text
schemaVersion
sourceSelection { schemaVersion, workspaceRoot, includeUntrackedProjectMaterial }
targetPlatform
resources { schemaVersion, workRoot, outputRoot, maxArtifactBytes }
totalDeadlineMs
```

执行纪律：

```text
production owner 只启动一次；
ProductionCandidateModule::produce 调用次数必须为 1；
不得先跑 durable failure probe；
不得运行旧 R2-N6/N8 ignored test；
不得在失败后修改 request 或更换 root 再试；
sealed 或 failed terminal disposition 出现后立即进入证据冻结并停止。
```

资源上限在执行前写入 request 并冻结：总 deadline 不超过 7,200,000 ms，artifact budget 不超过 34,359,738,368 bytes。外层进程 deadline 必须覆盖 Module deadline与 result/cleanup 余量，不得以历史平均耗时作为 kill deadline。

R2-FC6 于 2026-07-22 精确执行一次并 terminal failed。冻结 producer binary SHA-256=`8cc00edcd99aa5e2f64a95d4aef37a3e902045c4dc8b44f12e408b330e4edb12`，request SHA-256=`a8918e21d631d11a46550cf068fd6e3dcea31e9e14e0f4218d0f0025ce416ca7`；owner invocation count=1、Module invocation count=1、retry count=0，进程 exit code=2，耗时 703.103 秒。终态 diagnostic=`source_snapshot.file_set_mismatch`，没有 candidateId；11 个记录的受控子进程均 completed 且 exit code=0。durable failure artifact canonical digest=`sha256:6a0f31cbf512f3db4220b92d4451447119ee48a55039ab3f750c08e8dc081695`，cleanup measurement digest=`sha256:d020d79e7d39df1182e0fac48bb289cebd59134e8e6a568f7cdae6469ef45d08`；owned staging 已删除、owned orphan process=0、输出根只有该 failure artifact，production source outer/staged/nested 保持冻结且 clean。当前只确认终端 diagnostic，不在本报告中把尚未诊断的实现原因写成事实。机器证据见 `254-R2-R2-FC6-production-operation-terminal-report-v1.json`。

上述 terminal report 保留执行当时的“root cause not diagnosed”事实；后验只读审查随后确认根因是 released Editor 打开 snapshot fixture 后写入 project journal。该后验结论记录在 `../../254-R2-历史失败归档-2026-07-22.md`，不回写或伪造原 terminal artifact。

### R2-FC7：证据冻结、归档与停止

当前状态：未授权、未执行。R2-FC6 terminal failure 已消耗本次唯一 Fresh Candidate production authorization；不得以本节为由继续 FC7、补跑或归档为 completed。

成功证据至少包含：

```text
producer exact commit/tree/gitlink 与 clean receipts
producer binary SHA-256
request path/SHA-256 与 canonical fields
owner invocation count = 1 / module invocation count = 1 / retry count = 0
candidateId、candidate root、manifest/package identity digest
source snapshot receipt、release inventory、conformance summary
seal marker、final independent reopen
work root cleanup 与 source unchanged
activationIdCreated=false / attemptIdCreated=false
```

失败证据至少包含 durable failure ref/artifact digest、diagnostic、bounded process results、cleanup measurement、`candidateIdCreated=false`。证据无效、cleanup 不完整或 owner 无法证明一次调用时，本阶段终态为 rejected，不得补跑。

完成后生成阶段完成记录、同步入口并归档本施工文档。无论成功或失败都立即停止；Real Evaluation 仍为 `authorization pending`。

## 7. 高风险清单

```text
薄 Adapter 意外恢复 R1 freezer 或 caller stage 菜谱；
ignored preflight candidate 被误标为 production candidate；
脏 source/sample 生成漂移进入 snapshot；
gitlink OID/HEAD/clean/material closure 漂移；
owner 或 Module 被调用超过一次；
失败产生 candidateId 或删除唯一失败证据；
result 写入成功但独立 reopen 失败；
work/output/result root 重叠或复用；
权威回归失败后仍启动 production；
production 成功后自动进入 activation/evaluation。
```

## 8. 禁止事项

```text
不得执行旧 R2-N6/N7/N7V/N7W/N8/R2-G。
不得复用历史 candidate、preflight candidate、activation、attempt 或 evidence。
不得创建 activationId、attemptId 或 goal-level Grant。
不得调用 CodexOutcomeEvaluationModule::evaluate。
不得启动 Native Editor production session 或真实 Codex。
不得执行 F-A/F-B/G 或三引擎 B 通道。
不得清理或暂存 consolidation 工作树的 sample Build/Library 漂移。
```

## 9. 自审

- 与 254-R2 正式方案一致：公共 Interface 仍只有两个目标级 Module；本阶段只使用第一个。
- 没有扩大到 Real Evaluation、项目 mutation、Preview、Build/Delivery outcome 或真实客户端。
- 薄 Adapter 只提供 transport/ownership，不暴露内部 stages，不限制 AI 的工具外规划。
- 旧 R2-N8 明确保持历史未执行状态，不被重命名、重跑或复用。
- 验证分为定向、受影响域、环境等价预检和最终权威回归；production 只在全部前置证据通过后执行一次。
- 失败边界明确：任何 production terminal failure、证据无效或 cleanup failure 都停止，不自动重试。
- source、target、TEMP、artifact、work/output/result root 的隔离与容量合同已写明。
- 当前施工文档唯一，待执行队列为空；R2-FC6 唯一授权已消耗且失败，FC7 与第二段 Real Evaluation 均未授权。
- 用户已确认保留两个公共 Module，并修订 `ProductionCandidateModule` 内部 immutable snapshot / writable conformance workspace 合同；该确认不授权修改代码或运行第二次 candidate。

自审结论：R2-FC6 已执行一次并 terminal failed，证据与 cleanup 已冻结，未发生重试、candidate 创建或下游执行。失败原因已归档，正式内部合同已按方案 2 修订；本文件不再是可执行 remediation 计划。下一步必须单独编写、自审并激活新的 `ProductionCandidateModule` 内部合同 remediation 施工文档，取得新代码施工授权后才能修改实现；第二次 Fresh Candidate 仍需后续独立授权。
