# 251 - Provider-independent From-Blank Creation Golden Gate v1 方案

> 状态：已完成并归档  
> 确认日期：2026-07-16  
> 上游结论：`审查目录/其它AI审查目录/52-P0-0.5-v7-三引擎A通道正式汇总对比.md`  
> 继承方案：`250-AI-Primary-ProjectProduction-Dual-Path-v1方案.md`

## 1. 决策

采用方案 C：新增一个深的 `C01FromBlankCreationGate` Module，用一个 Interface 完成真实空目录预检、正式 CreateProject、一次目标授权、provider-independent imported Codex Candidate、逐候选验证与 receipt、Save/reopen/Preview、Windows Export、真实窗口与统一计时。

```text
run_from_blank(C01FromBlankCreationRequest) -> C01FromBlankCreationReport
```

它不重写 250，也不增加第二套 ProjectPatch/SourcePatch/AssetImport 真相。内部继续调用 `ProjectCandidateEntry` 和现有 C-01 owner。

## 2. 方案比较

| 方案 | 做法 | 判断 |
|---|---|---|
| A：直接重跑旧 `c01-golden-gate` | 人工先建空项目，再运行现有 construction mode | 不足。无法证明目标根起初不存在，也没有一次目标授权、imported JSON 入口和活动/等待时间 |
| B：只写外层脚本 | PowerShell 创建目录并拼接多个命令 | 不足。关键不变量散落在调用者，报告不能成为引擎侧权威证据 |
| C：深 Creation Gate | 一个请求隐藏 preflight、创建、import、candidate、preview/export 和报告 | 采用。Interface 小，复杂度集中，可由 CLI、测试和后续 Editor consumer 复用 |

## 3. 强制不变量

```text
project_root 在 Gate 开始时不得存在；禁止清空或复用已有项目。
candidate_store、evidence、external_export 必须与项目根、Engine SDK 根互不包含。
不得读取旧 D1-D10 candidate/evidence；fresh=10，reused=0。
项目创建必须走正式 Editor CreateProject command。
每个 envelope 必须以 sourceKind=imported_codex 经 strict JSON imported entry 重新解析。
用户只确认一次 FeatureSpec/目标摘要；内部仍保留 10 个 validation/approval/receipt/digest。
Gate 执行期间 Engine SDK source snapshot 前后必须一致。
失败保留 first blocker、已完成阶段、repair count 和下一动作；不得回退为 validation_only。
```

## 4. 报告合同

新增 `c01-from-blank-creation-report.v1`，至少记录：

```text
status / entryMode=creation_mode / providerMode=provider_independent_imported_codex
featureSpecDigest / goalApprovalDigest / manualConfirmationCount
projectRootExistedBefore=false / formalProjectCreate=passed
freshCandidateCount=10 / reusedCandidateCount=0
initialEmptyProjectDigest / finalProjectDigest
firstPlayableMs / totalWallClockMs / automationActiveMs / externalWaitMs
repairCount / firstBlocker
engineSourceSnapshot before/after
nested C01 candidate/preview/export evidence
```

`firstPlayableMs` 在真实 Preview 纹理、HUD 和 8/8 transition 通过后停止；`externalWaitMs` 统计编译、Player 子进程、打包和外部验证的阻塞区间；`automationActiveMs = totalWallClockMs - externalWaitMs`。用户等待和自动化工作不得混称。

## 5. 性能可比口径

扩展正式 Windowed Player performance summary：

```text
total CPU frame
update
render submit
present/acquire wait
```

每项记录 observed samples、mean、P95、P99。总帧仍保留以兼容现有报告；Phase breakdown 是新增证据，不能倒推或伪造 Unity/UE 历史数据。

## 6. 真实 Gate 根

```text
project=<LOCAL_TEST_ROOT>\AiFirstGame-FromBlank-251-r2
candidate_store=<run-root>\251-r2\candidates
evidence=<run-root>\251-r2\evidence
external_export=<LOCAL_TEST_ROOT>\Exports\AiFirstGame-FromBlank-251-r2
engine_sdk=<repository-root>\rust
```

R1 在 43.5 秒处因未传播 `CARGO_TARGET_DIR` 导致 Player artifact build root 落入 Engine SDK，并被隔离合同正确拒绝。R1 项目、10 个候选与失败 evidence 全部冻结；R2 必须使用新的四个输出根，并通过 `prior_attempt_report` 把 R1 first blocker 与 repair count 绑定进最终报告。旧 `<LOCAL_TEST_ROOT>\AiFirstGame`、旧 250-F evidence、Unity A 和 UE A 全部只读。

## 7. 验收

```text
硬门槛：同一连续窗口 2 小时内完成。
竞争目标：30 分钟内首次可玩，60 分钟内 Windows 外部交付。
Preview / Export / external no-arg / screenshot / texture / HUD / 8/8 transitions 全通过。
120 warmup + 600 performance samples，并输出 phase breakdown。
一次人工目标确认；用户不管理内部十个 candidate。
Engine SDK source snapshot unchanged。
定向、受影响域、default workspace、all-features workspace 最终通过。
```

## 8. 明确不做

```text
不修改或删除旧 C-01、Unity、UE 项目和证据。
不使用内置 Provider，不发送源码或项目内容到网络 Provider。
不新增多 payload 原子事务，不绕过逐候选 validation/receipt/rollback。
不把 C-01 玩法语义移入 Engine Core。
不以 validation_only、预置项目、复制项目或 mock/headless 截图冒充 creation mode。
不开始三引擎 B 通道。
```

## 9. 方案自审

```text
用户确认：通过。用户在 52 号终局建议后明确要求直接开始本 Gate。
优先级：通过。240 原队列全部关闭；本项是用户指定的新 P0，不跳过未完成项。
深 Module：通过。调用者只理解一个 request/report；preflight、创建、候选、验证、导出和度量隐藏在实现内。
既有真相复用：通过。ProjectCandidateEntry、ProjectLauncher、Preview、Export、RuntimePackage 均不复制。
零基础用户：通过。一次目标确认，不暴露 Cargo、JSON、Candidate 顺序或内部 approval。
公平性：通过。真实不存在的项目根开始，旧候选复用为硬失败。
范围：通过。只补 52 指出的 from-blank 证明和性能口径，不重开 Renderer/ECS/AUI。
```

结论：251 方案可生成、自审并激活唯一施工文档。
