# 252 - Editor Intent / WorkItem / ChangeSet Iterative Project Production Workflow v2 方案

> 状态：用户已确认第二种结构；正式方案已重新生成并自审；252 v2 施工文档已生成、自审并激活，施工中  
> 初次确认日期：2026-07-16  
> v2 重构确认日期：2026-07-16  
> 稳定文件名说明：为保持既有 `252` 引用有效，文件名继续保留 `Goal-level-Iterative-Project-Production-Workflow-v1`；正文与未来实现真相以本 v2 为准  
> 用户选择：`IntentEvent + WorkItem + ChangeSetProposal + ProjectProductionRun`  
> 上游方案：`250-AI-Primary-ProjectProduction-Dual-Path-v1方案.md`  
> 上游 Gate：`251-Provider-independent-From-Blank-Creation-Golden-Gate-v1方案.md`  
> 产品对象：没有编程基础和只有少量编程基础的游戏创作爱好者

## 1. 决策

正式采用第二种结构，新增一个通用、深的 `ProjectIntentWorkflow` Module：

```text
Unrestricted User Expression
  -> immutable IntentEvent
  -> independent WorkItem
  -> optional Diagnosis / Evidence Loop
  -> scoped ChangeSetProposal
  -> one mutation approval
  -> existing ProjectProductionRun
  -> Candidate validation / apply / receipt
  -> Preview / Play / Feedback / Reopen
```

核心原则：

```text
不限制用户如何表达。
不要求用户先形成完整、连续、无矛盾的需求脉络。
不让一个未想清楚的问题阻塞另一个已经明确的修改。
捕获、整理、查询、只读扫描、复现和诊断不等于项目修改批准。
只有准备真正修改项目的 ChangeSetProposal 才进入批准边界。
批准约束引擎允许修改什么，不约束用户允许说什么。
```

`FeatureSpecRevision` 不再是所有需求、Bug、反馈和实验的强制入口。它被替换为可选、派生的 `ProjectGoalSnapshot`，只用于表达某个时点的项目目标，不拥有修改授权，也不阻塞独立 WorkItem。

## 2. 为什么必须推翻原 v1 前门

原 v1 已经解决了“一次 prompt 后黑盒完成”和“逐 Candidate 询问”问题，但仍然把真实创作过程压入一个全局 Feature Spec：

```text
所有影响实现的 open question 都要先解决。
所有 user decision 都要在计划批准前解决。
任意 spec / plan 变化都会使整个批准失效。
所有需求、Bug 和试玩反馈都进入同一种 incremental_change Run。
全局状态仍然是 Drafting -> SpecReady -> Planning -> PlanReview 的线性流程。
```

这不符合人的实际工作方式：

```text
用户可能今天提出玩法想法，下周才决定是否采用。
用户可能只说“这里看起来不对”，暂时没有复现步骤。
一个 Bug 可能经历多次证据补充、错误假设、修复和重新打开。
用户可能同时有一个明确小修改和三个尚未想清楚的大方向。
对话可能跨天、跨任务、被打断，并且没有一条完整叙事线。
```

因此 v2 不再要求“先把整个需求说清楚，再允许开始”。它只要求“准备修改的这一小批内容已经足够明确，可以审阅和验证”。

## 3. Unity + Codex、UE + Codex 的实际处理方式

### 3.1 Unity + Codex

Unity 不要求用户把需求整理成引擎内 Feature Spec。实际流程是：

```text
用户随时提出需求、Bug、截图或试玩反馈
  -> Codex 检查项目 C#、Scene、Prefab、serialized asset 和 Console
  -> 通过 Editor API / AssetDatabase / Test Framework / Play Mode 复现
  -> 修改项目层代码、资源或设置
  -> script compile / domain reload / Play / BuildPipeline 验证
  -> 用户继续给出下一条修改
```

本项目 Unity A 通道真实经历了多次 build、Scene repair、Preview 和 exported runtime 尝试，但修复始终位于正式项目表面，不需要修改 Unity。

Unity 的优势：表达自由、局部修改进入快、项目层工具成熟。

Unity 的缺口：需求历史、Bug 原因、用户决定和验收标准通常分散在对话、Git、外部 issue 系统和日志里；Unity 默认不提供 Candidate、批准绑定、project digest 和 apply receipt。

### 3.2 UE + Codex

UE 同样不强制需求表达流程：

```text
用户随时提出需求、Bug、截图或试玩反馈
  -> Codex 检查 C++ / Blueprint / UObject / Config / Output Log
  -> PIE / Automation / Functional Test / Message Log 复现和诊断
  -> 修改项目 C++、Blueprint、Python、资源或 Config
  -> UBT 编译 / PIE / Cook / Stage 验证
  -> 用户继续给出下一条修改
```

UE 的优势：诊断、测试、内容生产和大型项目工具成熟。

UE 的缺口：C++、UObject、Shader、Cook、Target 和插件环境使迭代更重；需求与诊断历史仍主要依赖外部管理，默认也没有针对 AI 修改的原生批准和 receipt 链。

### 3.3 对本方案的直接约束

本引擎不能通过要求用户先整理需求来换取审计能力。正确的差异化是：

```text
用户表达摩擦 <= Unity/UE + Codex
项目修改约束与证据 > Unity/UE 默认项目流程
跨会话的意图、Bug、决定和验证连续性 > 仅依赖对话历史
```

Unity/UE 的成熟诊断工具应当学习；它们对需求表达不设前置 Gate 的做法也应当学习。不能照搬的是“让对话、Git 和日志自行承担全部长期工程记忆”。

### 3.4 本方案采用的实测与源码证据

```text
审查目录/其它AI审查目录/52-P0-0.5-v7-三引擎A通道正式汇总对比.md
  - Unity / UE A 通道实际项目生产与多轮 repair
  - 三引擎 validation / rollback / audit 对比

<LOCAL_TEST_ROOT>/Unity/unityTest/Assets/C01/Editor/C01ProjectBuilder.cs
  - AssetDatabase / Scene / Prefab / serialized validation / BuildPipeline 项目层闭环

<LOCAL_TEST_ROOT>/Unity/unityTest/Library/PackageCache/com.unity.test-framework@1405238725ab/
  - TestRunnerApi、EditMode/PlayMode、LogAssert 和 command-line test

<UNREAL_LAUNCHER_REFERENCE>/UE_5.8/Engine/Source/Developer/OutputLog/
  - FOutputLogModule、日志过滤和 Console

<UNREAL_LAUNCHER_REFERENCE>/UE_5.8/Engine/Source/Runtime/Core/Public/Misc/AutomationTest.h
  - FAutomationTestFramework

<UNREAL_LAUNCHER_REFERENCE>/UE_5.8/Engine/Source/Developer/AutomationController/
<UNREAL_LAUNCHER_REFERENCE>/UE_5.8/Engine/Source/Developer/FunctionalTesting/
  - Automation report、Message Log、Functional Test 和 screenshot evidence
```

## 4. 方案比较

| 方案 | 做法 | 结论 |
|---|---|---|
| A：放宽原 FeatureSpec Gate | 允许局部 open question，补 Park/Resume | 不采用。全局 FeatureSpec 仍是瓶颈，Bug 与实验仍被压入同一种对象 |
| B：IntentEvent + WorkItem + ChangeSetProposal + ProductionRun | 自由捕获、独立工作项、可选诊断、局部批准、复用现有执行真相 | 采用。兼顾低摩擦和原生审计 |
| C：立即实现完整项目知识图谱 | 任意实体/关系/推理/长期知识库 | 暂不采用。当前会扩大 schema、查询和 UI，先用固定 WorkItem 关系建立可演进底座 |

## 5. Module 与 Seam

### 5.1 外部 Interface

普通 Editor、Launcher 和 AI Panel 只依赖一个 `ProjectIntentWorkflow` Interface：

```text
capture(IntentCaptureInput) -> IntentCaptureReceipt
observe(ProjectIntentQuery) -> ProjectIntentSnapshot
prepare_change(ChangePreparationRequest) -> ChangePreparationResult
authorize(ChangeSetApproval) -> ProjectProductionRun
dispatch(ProjectIntentWorkflowCommand) -> ProjectIntentWorkflowSnapshot
```

Interface 行为合同：

```text
capture 只在本地追加来源事件并快速返回 receipt，不同步等待 Provider、编译或 Preview。
observe 无项目修改副作用，可读取由 journal checkpoint 派生的当前 snapshot。
prepare_change 返回完整 ChangeSetProposal 或结构化 blockers，不返回半有效 proposal。
authorize 原子校验 proposal/base/approval binding；失败时不创建具有写权限的 Run。
dispatch 的长诊断、验证、编译和 Preview 由后台 worker 驱动，不能阻塞 UI thread。
同一 command id 重放必须幂等或返回已有 terminal receipt。
```

`ProjectIntentWorkflowCommand` 只承载运行期生命周期命令：

```text
AdvanceRun
CancelRun
ResolveDecision
RecoverRun
ParkWorkItem
ResumeWorkItem
ReopenWorkItem
MergeWorkItems
SplitWorkItem
```

调用方不需要理解事件压缩、WorkItem 归一化、关系维护、proposal materialization、Candidate chain、receipt recovery 或 Provider 差异。这些复杂性由 Module Implementation 隐藏。

### 5.2 Module 深度

删除 `ProjectIntentWorkflow` 后，以下复杂性会重新散落到 Launcher、AI Panel、EditorSession、Report Panel 和各类 worker 中：

```text
跨会话事件持久化
WorkItem 拆分、合并、去重和重开
Bug 证据与诊断状态
局部 readiness 与非阻塞 open question
ChangeSet 批准绑定和失效规则
Candidate 执行、暂停、取消和恢复的聚合状态
```

因此它是有实际 Leverage 和 Locality 的深 Module，不是现有 `ProjectCandidateEntry` 的透传包装。

### 5.3 内部复用

252 只负责编排，不建立第二套 lowering、Apply、Build 或 Runtime 真相：

```text
ProjectIntentWorkflow
  -> ProjectIntentJournal
  -> WorkItemIndex
  -> optional ProjectDiagnosisSession
  -> ChangeSetProposal materialization
  -> formal CreateProject command
  -> ProjectCandidateEntry
       -> ProjectPatch lowering
       -> ControlledSourcePatch lowering
       -> AssetImport lowering
  -> ProjectReadiness
  -> Preview / RuntimePackage
  -> Report Registry
```

现有 `ProjectCandidateEntry::prepare/validate/apply/rollback` 仍是每个 Candidate 的唯一正式入口。252 不直接写 Scene、Prefab、AUI、Rule、RuntimeModule、AssetDB 或 Build 输出。

### 5.4 内部 Adapter

只有真实存在多个实现的 seam 才建立 Adapter：

```text
IntentNormalizationSource
  - ImportedCodexNormalization
  - BuiltInProviderNormalization

ChangePlanSource
  - ImportedCodexChangePlan
  - BuiltInProviderChangePlan（能力满足时）
  - FuturePlannerChangePlan

EvidenceSource
  - UserAttachedEvidence
  - LocalProjectDiagnosticEvidence
```

Adapter 只能提交 schema-valid normalization、evidence 或 plan source。它不能批准、Apply、伪造验证结果、直接写项目或把自己的推断覆盖成用户事实。

## 6. 工程真相对象

### 6.1 IntentEvent

`IntentEvent` 是用户表达和项目观察的不可变来源记录：

```text
schemaVersion
eventId
projectIdentity?
occurredAt
sourceKind = user_message | screenshot | log | test_result |
             editor_observation | imported_context | system_event
sourceIdentity
contentRef?
sanitizedSummary
attachmentRefs[]
relatedEventIds[]
privacyClass
contentDigest
```

规则：

```text
任何表达都可以先捕获，不要求完整、无矛盾或可执行。
事件只追加，不原地改写；用户纠正通过新事件表达。
原始聊天可按隐私策略只保存在本地或只保留引用；sanitized summary 不替代原始来源身份。
IntentEvent 不进入 RuntimePackage，不进入游戏运行时热路径。
捕获事件不产生项目写入授权。
```

`IntentEvent` 是来源证据，不等于 AI 已正确理解需求。用户明确表达和附件是来源；`WorkItem.latestUnderstanding` 是可纠正的派生解释。

### 6.2 ProjectIntentSnapshot

`ProjectIntentSnapshot` 是从 journal checkpoint 派生的只读观察面：

```text
schemaVersion
checkpointId
journalRevision
journalDigest
workItemSummaries[]
activeDiagnosisSummaries[]
activeProposalSummary?
activeRunSummary?
pendingNormalizationEventIds[]
diagnostics[]
```

Snapshot 可以丢弃并从 journal 重建。调用方不能直接编辑 snapshot 来绕过 WorkItem revision 和事件记录。

### 6.3 WorkItem

`WorkItem` 是 AI/Editor 从一个或多个 IntentEvent 归一化得到的独立工作对象：

```text
schemaVersion
workItemId
kind = idea | requirement | change | bug | question | feedback | experiment
title
userVisibleOutcome
sourceEventIds[]
status
priority
scopeHints[]
constraints[]
acceptanceCriteria[]
openQuestions[]
evidenceRefs[]
relationshipRefs[]
latestUnderstanding
explicitlyDeferred[]
revision
workItemDigest
```

固定关系只支持当前真实需要的集合：

```text
depends_on
blocks
related_to
duplicates
supersedes
caused_by
evidence_for
```

v1 不实现任意知识图谱查询语言。

关键规则：

```text
每个 WorkItem 独立判断 readiness。
一个 WorkItem 的 open question 只阻塞它自己或显式 depends_on 它的项。
未选入本次 ChangeSet 的 WorkItem 不影响批准。
WorkItem 可以 Park、Resume、Merge、Split、Cancel 和 Reopen，且必须保留来源 lineage。
用户可以直接编辑 AI 的 latestUnderstanding；修正产生新 revision，不删除来源事件。
```

### 6.4 ProjectGoalSnapshot

`ProjectGoalSnapshot` 是可选的项目目标视图，不是强制前门：

```text
schemaVersion
snapshotId
projectIdentity?
includedWorkItemRevisions[]
goals[]
gameplaySummary[]
visualSummary[]
uiSummary[]
platformTargets[]
constraints[]
explicitlyDeferred[]
snapshotDigest
```

它可以帮助用户理解“这个项目目前想做什么”，也可以作为 from-blank 首次规划上下文。但它是从已选 WorkItem 派生的快照：

```text
ProjectGoalSnapshot 变化不会自动使所有 ChangeSet 失效。
只有被某个 ChangeSet 显式绑定的目标变化才影响该 ChangeSet。
项目允许长期存在未决、互相冲突或仅供探索的 WorkItem。
```

### 6.5 ProjectDiagnosisSession

Bug、性能问题或“看起来不对”可以先进入诊断，不要求立刻提出修复：

```text
schemaVersion
diagnosisId
workItemId
baseProjectDigest
state = needs_evidence | reproducing | investigating |
        cause_confirmed | inconclusive | fix_scope_ready | closed
reproductionAttempts[]
observations[]
hypotheses[]
confirmedCause?
evidenceRefs[]
proposedFixScope[]
diagnosisDigest
```

无需项目修改批准即可执行：

```text
读取项目和报告
读取 Console / Output Log / structured diagnostics
运行已有测试、Preview 或复现步骤
在隔离 evidence/output 根生成诊断产物
比较已有 artifact
```

仍需独立权限或批准：

```text
修改项目源代码、资源、配置或依赖
为了诊断而向项目插入 instrumentation
产生未授权外部网络行为、费用或敏感上下文发送
删除、替换或覆盖用户内容
```

诊断结论不必一次正确。错误假设被新证据否定时保留历史，并继续同一个 Bug WorkItem。

### 6.6 ChangeSetProposal

只有准备真正修改项目的 WorkItem 才进入 `ChangeSetProposal`：

```text
schemaVersion
proposalId
targetKind = new_project | existing_project
targetProjectIdentity?
projectCreateSpec?
expectedBaseProjectDigest?
selectedWorkItemRevisions[]
userVisibleOutcomes[]
explicitExclusions[]
candidatePlanSteps[]
acceptanceChecks[]
estimatedExternalWaits[]
externalCosts[]
risks[]
requiredDecisions[]
repairPolicy
proposalDigest
```

批准前只要求解决与本 proposal 直接相关、会改变实现或风险的决定。项目中其它 Idea、Question、Bug 或 parked WorkItem 不阻塞它。

每个 `candidatePlanSteps[]` 是已审阅修改的计划项，不提前伪造最终 Candidate receipt：

```text
stepId
dependsOn[]
payloadKind = asset_import | project_patch | controlled_source_patch
payloadSourceDigest
materializationPolicy
validationProfile
expectedChangedDomains[]
userVisibleOutcome
failurePolicy
```

Candidate envelope 在执行该 step 时绑定当时真实的 `project_id`、`project_digest` 和 context hash。

### 6.7 ChangeSetApproval

```text
schemaVersion
approvalId
approvedBy
proposalDigest
selectedWorkItemDigests[]
targetIdentity
expectedBaseProjectDigest?
approvedRiskClasses[]
approvedExternalCosts[]
approvedRepairPolicy
approvedAt
approvalDigest
```

这是每个修改批次唯一具有项目修改权限的用户批准。内部 Candidate approval 只有在以下条件全部成立时才能派生：

```text
Candidate 是已批准 proposal step 的精确 materialization 或允许的同语义 repair。
当前 project digest 满足该 step 的顺序绑定。
Candidate validation passed。
changed domains、风险、依赖、外部费用和用户可见结果没有越过批准范围。
```

只使当前批准失效的变化：

```text
被选 WorkItem 的用户可见语义变化。
proposal step、changed domains、风险、依赖、费用或验收标准变化。
base project digest 无法按批准顺序绑定。
repair 超出 approvedRepairPolicy。
```

不会使当前批准失效的变化：

```text
未选 WorkItem 的新增、补充、Park、Resume 或讨论。
与当前 proposal 无关的未来 Idea 或 Question。
只追加诊断证据且不改变当前修复范围。
确定性 schema normalization、路径/格式修复和批准 validation profile 内的重试。
```

### 6.8 ProjectProductionRun

```text
schemaVersion
runId
runKind = from_blank | scoped_change
proposalId
changeSetApprovalDigest
targetProjectIdentity
baseProjectDigest
currentProjectDigest?
state
activeStepId?
stepSnapshots[]
appliedReceipts[]
linkedWorkItemRevisions[]
decisionRequests[]
recoveryOptions[]
previewEvidence?
diagnostics[]
timing
```

Run 是 Editor/Report 可观察的聚合状态，不替代各 Candidate 的 validation、approval 和 receipt 证据。

同一项目默认只允许一个具有写权限的 active `ProjectProductionRun`。其它 WorkItem 可以继续捕获、讨论和只读诊断，但不能绕过当前 mutation lane 并发 Apply。

## 7. 状态模型

### 7.1 WorkItem 生命周期

WorkItem 不是全局线性状态机，每个项独立流转：

```text
Captured
  -> Triaging
  -> NeedsClarification | NeedsEvidence | Ready | Parked
  -> Proposed
  -> InProgress
  -> Verifying
  -> Done
```

`Proposed` 只表示 WorkItem 被某个 active ChangeSetProposal 选择。批准状态属于 `ChangeSetApproval`，不复制成 WorkItem 真相；对应 Run 正式开始后，WorkItem 才进入 `InProgress`。

通用非线性转换：

```text
任意非终态 -> Parked -> Triaging | Ready
任意非终态 -> Blocked -> 原状态或 Parked
Captured / Triaging / Ready / Parked -> Cancelled
Done -> Reopened -> Triaging | NeedsEvidence | Ready
任意可编辑状态 -> Merged | Split，保留 lineage
```

### 7.2 Bug Profile

Bug 使用更明确的诊断进度：

```text
Captured
  -> NeedsEvidence
  -> Reproducing
  -> Investigating
  -> CauseConfirmed | Inconclusive
  -> FixScopeReady
  -> Proposed
  -> Fixing
  -> RegressionVerifying
  -> Done | Reopened
```

如果 Bug 已有确定复现和根因，可以跳过不必要阶段；跳过理由进入事件和诊断记录。不能为了填满状态机强迫用户重复提供信息。

### 7.3 ProductionRun 生命周期

```text
ProposalReady
  -> ApprovalReview
  -> Approved
  -> CreatingProject          (from_blank only)
  -> Executing
  -> Previewing
  -> Completed
```

非成功分支：

```text
ProposalReady / ApprovalReview -> Cancelled
Executing / Previewing -> Cancelling -> Cancelled | RecoverableFailure
Executing -> AwaitingUserDecision -> Executing | ApprovalReview | Cancelled
Executing / Previewing -> RecoverableFailure -> Recovering -> Executing | RolledBack | Failed
相关 project digest drift -> Stale -> ProposalReady
```

状态转换必须写结构化事件；UI 不根据日志字符串猜当前阶段。

## 8. 自由表达、局部推进与跨会话连续性

### 8.1 任意表达

以下输入都合法，不要求用户先分类：

```text
“以后可能想加联机，但我还没想好。”
“先把按钮改成红色。”
“昨天试玩时敌人偶尔不生成，我不知道怎么复现。”
“这个画面感觉不够有力量。”
一张截图、一段日志、一次崩溃、一次测试失败。
```

系统可以归一化为多个 WorkItem，但必须把“AI 理解”与“用户原话/证据”区分开。

### 8.2 局部 readiness

示例：

```text
W-01 联机想法              -> Parked
W-02 按钮改红色            -> Ready
W-03 敌人偶尔不生成 Bug    -> NeedsEvidence
```

用户可以立即为 W-02 准备和批准 ChangeSet。W-01 和 W-03 不阻塞它。

### 8.3 执行中继续说

执行某个 ChangeSet 时，用户仍可继续表达：

```text
与当前 proposal 无关 -> 捕获为新事件/WorkItem，当前 Run 继续。
只是补充证据且不改当前修复范围 -> 追加 evidence，当前 Run 可继续。
改变当前选中 WorkItem 的用户可见结果 -> 当前 Run 安全暂停并重新 prepare proposal。
要求立刻停止 -> 协作式取消，保留已 Apply receipt。
```

不能因为“用户又说了一句话”就让整个 Run 自动 stale；只有与当前批准绑定内容发生实质冲突时才 stale。

### 8.4 跨天、跨任务恢复

`ProjectIntentJournal` 追加结构化事件：

```text
IntentCaptured
WorkItemCreated / Revised / Parked / Resumed / Reopened
EvidenceAttached
DiagnosisUpdated
ProposalPrepared / Approved / Invalidated
RunLinked / Completed / Recovered
VerificationCompleted
```

`ProjectIntentSnapshot` 是可重建 checkpoint，不是另一份手工维护真相。重新打开项目时可以恢复 WorkItem、诊断、proposal 和 run 关系，不依赖回放完整聊天窗口。

为控制体积：

```text
长对话正文可以只保留本地引用和 digest。
结构化 event、用户决定、evidence identity、receipt link 必须持久化。
旧 snapshot 可以 compact，但不可破坏 event lineage 和审计 digest。
附件使用受控引用，不复制进入 RuntimePackage。
```

Journal 只属于 Editor authoring 元数据，不是新的全引擎事件总线：

```text
已有项目：持久化到受 ProjectWriteScope 保护的 project-owned editor metadata；具体路径在施工文档冻结。
从空白创建：先写 Launcher 本地 pre-project draft store，绑定 draft id、canonical target path 和 ProjectCreateSpec digest。
批准前：目标项目根必须仍不存在，pre-project draft 不写入目标目录。
正式 CreateProject 后：记录 create receipt 和 initial project digest，再把 draft journal 受控接管为项目 editor metadata。
创建失败或取消：保留或删除本地 draft 由用户决定，不伪造项目已经存在。
Build/Export：Intent、WorkItem、Diagnosis 和 journal 一律不进入 RuntimePackage 或交付包。
```

## 9. 用户侧流程

### 9.1 从空白创建

```text
Launcher: Create with AI
  -> 项目名称与位置
  -> 用户用任意方式描述游戏
  -> AI 将当前理解整理为独立 WorkItem
  -> 未想清楚的内容可以 Park
  -> 选择“先做出第一个可玩版本”的 ready WorkItem
  -> 查看本次 ChangeSet 摘要、排除项、风险和验收
  -> 一次修改批准
  -> 聚合进度
  -> 自动 Preview
  -> 继续聊天、试玩、修改、Park 或 Reopen
```

不要求从空白创建前解决所有未来玩法、画面和平台问题。

### 9.2 已有项目继续修改

```text
用户随时描述一个或多个变化
  -> 系统追加 IntentEvent
  -> 更新或新建 WorkItem
  -> 明确项可以立即准备 ChangeSet
  -> 一次批准
  -> validate / apply / Preview
  -> Done、继续调整或 Reopen
```

### 9.3 Bug 与断续修复

```text
用户：“敌人偶尔不出现”
  -> 捕获 Bug WorkItem
  -> 读取日志/测试/项目状态
  -> 无法复现：NeedsEvidence，用户可先做其它事情
  -> 后续用户补截图或再次出现
  -> Resume diagnosis
  -> 确认原因和局部 fix scope
  -> 生成 ChangeSetProposal
  -> 一次批准
  -> 修复 + regression verification
  -> Done；若再次出现则 Reopen 原 WorkItem
```

### 9.4 默认 UI 语言

普通用户只看到产品语言：

```text
我记录了三个事项，其中一个现在可以处理。
“以后加联机”已暂存，不影响当前修改。
敌人生成问题还缺少复现证据，你可以继续制作其它内容。
本次准备修改：按钮颜色和对应视觉验收。
修复已通过原问题复现和相关回归。
```

Candidate id、Cargo、raw JSON、编译器输出和完整 receipt 放在高级证据视图。

## 10. 批准、自动修复与用户决定

### 10.1 无需修改批准

```text
捕获用户表达
整理、拆分、合并或 Park WorkItem
生成 ProjectGoalSnapshot
只读 Context Scan
运行已有测试和复现
读取日志、报告和 artifact
在隔离根生成诊断证据
```

外部 Provider 调用、敏感上下文发送和费用仍遵守独立的 Provider 授权规则，不能借“只读诊断”绕过。

### 10.2 需要修改批准

```text
创建正式项目
修改项目资产、Scene、Prefab、AUI、Rule、Input、Build 配置
修改 project-owned source 或依赖
删除、替换或覆盖用户已有内容
发布、导出到新的外部目标或产生额外费用
```

### 10.3 批准后可自动继续

```text
确定性 schema normalization
计划内且不改变用户可见语义的路径/格式修复
同一 changed domains、风险和依赖范围内的诊断驱动 repair
已批准 validation profile 中的重试
```

每次 repair 都进入 Trace，并证明没有改变 selected WorkItem outcome、验收和风险。

### 10.4 必须暂停

```text
选中 WorkItem 的玩法或视觉结果出现多个合理选择
删除或替换 proposal 未声明的用户内容
新增 project-owned source 权限、依赖、外部费用或网络行为
changed domains、目标平台、交付范围或性能预算变化
无法保持已批准验收标准
```

暂停只影响当前 proposal/run，不冻结整个项目的意图捕获和其它 WorkItem 整理。

## 11. 取消、恢复与重新打开

### 11.1 取消

```text
停止产生新的 mutation work
向当前 worker 发出 cancellation
等待 join / credential release
保留已 Apply receipts
清理未 Apply candidate staging
输出 cancellation receipt 和当前 project digest
关联 WorkItem 回到 Ready、Parked 或 NeedsDecision
```

继续复用 243 的 transport cancellation/join 纪律。

### 11.2 恢复

```text
尚未 Apply：丢弃当前 Candidate，项目不变。
已 Apply 且仍是精确 latest chain：允许按 receipt 逆序恢复本 Run 已应用步骤。
存在手动修改或相关 digest drift：禁止自动覆盖，转为 Stale 并重新 prepare proposal。
Preview 失败但源修改有效：保留修改，建立 Preview/Build repair WorkItem 或恢复选择。
```

恢复选择：

```text
RetryCurrentStep
ReprepareFromCurrentProject
RollbackRunReceipts
KeepCurrentProjectAndStop
ParkAffectedWorkItems
```

### 11.3 Reopen

完成项出现回归时，不新建一个失去历史的孤立 Bug：

```text
Done WorkItem
  -> Reopened event
  -> 新 evidence / reproduction
  -> 关联上一次 ChangeSet、receipt 和 regression report
  -> 新 proposal / approval / verification
```

## 12. Provider-independent 合同

v2 必须支持 imported Codex normalization 和 plan；内置 Provider 不是唯一入口。

```text
Editor 导出 sanitized ProjectIntentContext
Codex 提交 schema-valid IntentNormalizationProposal 或 ChangePlanSource
Editor 校验 source/schema/capability/base/risk/lineage
归一化结果更新 WorkItem，但不获得修改权限
用户选择 ready WorkItem 并审阅 ChangeSetProposal
用户一次批准
Engine materialize -> Candidate Entry -> validate/apply
```

导入内容不得携带 API Key、不得声明“验证已通过”、不得伪造用户事实、不得引用 Engine Core 写入路径。来源失败不等于 intent journal 或项目生产系统失效；可以更换来源继续同一批 WorkItem。

## 13. UI 架构

不把方案做成强制 issue tracker，也不把全部状态堆进 `AiPanelModel`：

```text
ProjectLauncherModel
  - Create with AI
  - pre-project intent capture

ProjectIntentModel
  - continuous conversation
  - lightweight current understanding
  - active / parked / needs-evidence WorkItem view
  - merge / split / park / resume / reopen

ProjectChangeReviewModel
  - selected WorkItem outcomes
  - exclusions / risks / decisions / acceptance
  - one mutation approval

ProjectProductionModel
  - progress / decision / cancellation / recovery
  - Preview and verification result

AiPanelModel
  - conversation input and messages
  - does not own journal, WorkItem, proposal or run truth
```

零基础用户不需要填写 issue 表单、选择 WorkItem kind 或管理关系图。AI 默认整理，用户只需纠正理解、选择现在做什么、暂存什么，以及批准具体修改。

`editor_ui_model` 只保存可序列化 view model；`editor_ui_renderer` 只渲染和发命令；`editor_window_winit` 只负责 OS dialog/input/worker pump；`editor_core` 持有 workflow、验证和生命周期真相。

## 14. 报告与可观察性

遵循 Off / Summary / Trace：

```text
Off
  稳定帧不生成完整 journal/proposal/run 报告。

Summary
  active/parked/needs-evidence 数量、当前修改、等待原因、最新结果。

Trace
  event/work-item/proposal/approval digest、diagnosis evidence、
  candidate validation/receipt、timing、diagnostics、recovery evidence。
```

Runtime 不读取 IntentEvent、WorkItem、Diagnosis 或 ProductionRun。

计时至少区分：

```text
captureActiveMs
clarificationWaitMs
diagnosisActiveMs
evidenceWaitMs
planningActiveMs
approvalWaitMs
validationExternalWaitMs
applyActiveMs
previewExternalWaitMs
timeToFirstPlayableMs
totalChangeSetWallClockMs
```

## 15. 与 250 / 251 的关系

```text
250
  提供 CandidateProjectRevision、ControlledSourcePatch、AssetImport、
  Provider-independent Candidate Entry 和双 lowering 真相。

251
  提供固定 C-01 的真实 creation-mode 权威证据和时间/像素/交付 Gate。

252 v2
  提供自由表达、长期 WorkItem、诊断、局部 ChangeSet 批准，
  并把 ready 内容交给既有 ProductionRun/Candidate 执行真相。
```

251 不被删除或降级。252 施工完成时必须增加真实 Native Editor 用户流程 Gate，并继续运行 251。固定 C-01 payload builder 只能作为测试 fixture。

## 16. 分阶段施工边界

本节只冻结未来施工边界，不构成施工授权。

### Phase 1：Intent Journal 与 WorkItem 纯模型

```text
IntentEvent
ProjectIntentJournal / checkpoint / digest
pre-project draft store / CreateProject 后受控接管
WorkItem / revision / fixed relationships
Park / Resume / Reopen / Merge / Split
局部 readiness 与否定矩阵
ProjectGoalSnapshot 派生
```

### Phase 2：ChangeSetProposal 与批准语义

```text
selected WorkItem binding
proposal digest / exclusions / acceptance / risks
局部 required decision
ChangeSetApproval
unrelated WorkItem 不使批准失效
selected semantic change 使批准 fail-closed
```

### Phase 3：ProductionRun 执行器接入

```text
formal CreateProject
proposal step materialization
ProjectCandidateEntry 调度
ChangeSet approval -> internal Candidate approval
base digest / stale / receipt chain
one active mutation lane per project
```

### Phase 4：Diagnosis / Evidence Loop

```text
Bug profile
read-only diagnostic capability
evidence attachment
reproduction / hypothesis / confirmed cause
diagnostic instrumentation 必须转 ChangeSet
regression verification / Reopen
```

### Phase 5：Provider-independent Adapter

```text
sanitized ProjectIntentContext
Imported Codex normalization
Imported Codex ChangePlanSource
strict source import
来源推断与用户事实分离
```

### Phase 6：Native Editor 产品入口

```text
Launcher Create with AI
continuous conversation
lightweight WorkItem view
park/resume/reopen
ChangeSet review + one approval
progress / decision / recovery / Preview
```

### Phase 7：真实 Golden Gate

```text
真实 Native Editor 从空白 C-01
不少于十次零散、修正和互相无关的表达
一个 parked 大需求不阻塞一个 ready 小修改
一个无法复现 Bug 跨会话补证据后继续
一个 ChangeSet 批准派生多个内部 Candidate
执行中新增无关 WorkItem 不使 Run stale
选中 WorkItem 语义变化会安全暂停并重新批准
Preview、Save/reopen、Windows Export、真实窗口、8/8、性能
251 regression
default/all-features workspace regression
```

## 17. 验收标准

v2 至少证明：

```text
用户可以输入不完整、矛盾、跨天和无法立即执行的内容，捕获不失败。
IntentEvent 不可变，WorkItem 修正保留来源 lineage。
从空白创建批准前不写目标项目根；CreateProject 后 journal 接管绑定正式 receipt/digest。
至少支持 idea / requirement / change / bug / question / feedback / experiment。
一个 WorkItem 的 open question 不阻塞无依赖的 ready WorkItem。
Park / Resume / Reopen / Merge / Split 跨保存重开保持一致。
Bug 可以在不批准项目修改的情况下读取、复现和收集证据。
诊断需要修改项目时必须生成 ChangeSetProposal。
ChangeSet 只绑定 selected WorkItem revisions、base、风险、费用和验收。
无关 WorkItem 变化不使批准失效；selected semantic change 必须使其失效。
一个 ChangeSet 批准可受控派生多个 Candidate approval。
至少覆盖 AssetImport、ProjectPatch、ControlledSourcePatch 三类 step。
用户不读取或编辑 JSON/Rust/Cargo 即可到达 Preview。
手动修改造成相关 base drift 时 fail-closed 并 reprepare，不覆盖修改。
失败可以 retry/reprepare/rollback/keep-current/park。
完成 Bug 再次出现时 Reopen 并链接旧 receipt 与验证证据。
Report 可以从用户摘要下钻到 event/work-item/diagnosis/proposal/candidate/receipt。
```

## 18. 明确不做

```text
不在本方案阶段直接施工。
不把 UI 做成强制 issue tracker 或需求表单。
不要求用户先构造完整 Feature Spec。
不要求解决项目中所有 open question 后才能修改局部内容。
不实现任意知识图谱、通用查询语言或自动产品决策。
不把聊天记录直接当成项目修改真相。
不让 AI normalization 覆盖用户原始表达和证据。
不要求用户逐个批准内部 Candidate。
不伪造跨 Candidate 全事务原子性。
不新建第二套 ProjectPatch/SourcePatch/AssetImport apply 或 rollback。
不允许 plan source 直接修改项目、Engine Core 或 RuntimePackage 派生产物。
不把 Provider Registry、模型选择和费用策略混入 workflow 核心。
不进入三引擎 B 通道。
```

## 19. 当前代码基线

```text
rust/crates/editor_ui_model/src/launcher.rs
  已有 Open/Create Project launcher model，无 intent workflow。

rust/crates/editor_ui_renderer/src/panels/launcher.rs
  已有真实 Launcher 绘制与 hit target，可承接新的正式入口。

rust/crates/editor_window_winit/src/application.rs
  已有 project folder dialog、command dispatch、worker pump；长诊断和生产不能阻塞 UI thread。

rust/crates/editor_ui_model/src/ai_panel.rs
rust/crates/editor_core/src/services/ai_service.rs
  已有 prompt、proposal、LLM cancellation 和单 ProjectPatch candidate 路径，
  不拥有 journal、WorkItem、Diagnosis、ChangeSet 或跨 payload Run。

rust/crates/editor_core/src/project_candidate_entry.rs
  已有统一 imported/provider candidate prepare/validate/apply/rollback 真相，必须复用。

rust/crates/project_e2e_gate/src/c01_from_blank_creation.rs
  已有固定 C-01 from-blank 权威 Gate，只作为测试参考和回归 consumer。
```

当前代码没有 252 v1 实现，因此本次方案重构不产生旧 workflow 数据迁移或兼容负担。

## 20. 风险与约束

| 风险 | 约束 |
|---|---|
| 把 workflow 做成新的巨型 AI Panel | 独立深 Module；UI 只消费 snapshot、发 command |
| 把 WorkItem 做成强制 issue tracker | 默认由 AI 整理；用户只纠正、选择、Park 和批准 |
| AI 错误归一化用户意图 | IntentEvent 不可变；WorkItem 可纠正；始终保留 source lineage |
| WorkItem 无限碎片化 | 支持 merge/split/duplicates/supersedes，保留关系和 digest |
| 一个模糊问题阻塞全项目 | readiness 和 open question 只作用于 WorkItem/显式依赖 |
| 用户继续聊天导致 Run 频繁失效 | 只检查 selected WorkItem 和 proposal binding，忽略无关事件 |
| 只读诊断偷偷修改项目 | capability 分类；instrumentation 和 dependency 变化必须转 ChangeSet |
| 批准被解释为无限授权 | 精确绑定 proposal、selected revisions、base、risk、cost、repair policy |
| repair 偷换用户语义 | 限制 changed domains/risk/dependency/outcome，全部写 Trace |
| 跨会话 journal 无限膨胀 | checkpoint/compaction/attachment reference；保留关键 lineage 和 digest |
| 原始对话或证据泄露 | privacyClass、local reference、sanitized export、RuntimePackage 排除 |
| 完整知识图谱过度设计 | v1 只支持固定关系集合，不引入通用图查询层 |
| 多个 Run 并发覆盖 | 同项目一个 active mutation lane；project digest 和 receipt 顺序绑定 |
| 测试 fixture 进入产品 | C-01 builder 留在 Gate；core 只认识通用 schema 和 workflow |

## 21. 方案自审

```text
用户确认：通过。用户明确确认采用第二种结构并重新生成方案。
表达自由：通过。任意输入先捕获，不要求完整 Feature Spec 或全局 open question 清零。
局部推进：通过。WorkItem 独立 readiness；无关 parked/unclear 项不阻塞 selected ChangeSet。
Bug 断续生命周期：通过。Evidence、Diagnosis、Park/Resume、Regression 和 Reopen 为一等对象。
跨会话连续性：通过。append-only journal + derived checkpoint，不依赖完整聊天回放。
from-blank 存储：通过。pre-project draft store 不写目标根，CreateProject 后按 receipt/digest 受控接管。
批准边界：通过。只在项目 mutation seam 批准；只读诊断与内容修改明确分开。
Module 深度：通过。小 Interface 隐藏 journal、WorkItem、diagnosis、proposal、run 和 recovery。
既有真相复用：通过。CreateProject、ProjectCandidateEntry、ProjectReadiness、Preview、RuntimePackage 和 Report 均不复制。
Provider-independent：通过。imported Codex 与 built-in provider 都只是 normalization/plan Adapter。
目标用户：通过。默认流程不要求填写 issue、编辑 JSON/Rust/Cargo 或理解 Candidate。
范围控制：通过。不进入完整知识图谱、通用 Agent Planner、Provider Registry 或三引擎 B 通道。
施工授权：通过。用户已明确要求生成、自审施工文档并按规则开始施工；唯一当前施工文档已激活。
```

最终决定：正式采用方案 B v2。`ProjectIntentWorkflow` 以 `IntentEvent -> WorkItem -> ChangeSetProposal -> ProjectProductionRun` 为唯一新前门；`ProjectGoalSnapshot` 只作可选派生视图。用户施工授权已取得，252 v2 当前施工文档已完成自审与激活，后续严格按其 Gate 推进。
