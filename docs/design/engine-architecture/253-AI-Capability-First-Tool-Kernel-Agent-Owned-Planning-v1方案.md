# 253 - AI Capability-First Tool Kernel / Agent-Owned Planning v1 方案

> 状态：正式方案已确认并完成自审，尚未生成施工文档，尚未施工  
> 确认日期：2026-07-17  
> 用户选择：方案 C，能力优先工具内核 + AI 自主规划 + 有限授权 + 可选长期记忆  
> 上游实现：`250-AI-Primary-ProjectProduction-Dual-Path-v1方案.md`、`251-Provider-independent-From-Blank-Creation-Golden-Gate-v1方案.md`、`252-Editor-Goal-level-Iterative-Project-Production-Workflow-v1方案.md`  
> 竞争证据：`审查目录/其它AI审查目录/53-P0-0.5-v8-三引擎B通道已有项目持续修改对比协议.md`、`<LOCAL_TEST_ROOT>\Evidence\P0-0.5-v8\b8\三引擎B通道正式汇总对比.md`  
> 文档性质：架构方案，不是施工文档、施工授权或 B 通道重跑授权

## 1. 决策

正式采用方案 C：引擎不再用统一的硬编码菜谱规定 AI 完成开放式游戏需求的步骤，而是向 AI 提供稳定、可发现、可组合、可验证、可回滚的工程工具。AI 根据当前目标、项目事实和每一步反馈自主选择工具，并可随时丢弃旧计划、重新规划。

```text
用户负责：目标、产品取舍、可接受风险和授权范围。
AI 负责：理解、规划、拆分、工具选择、结果判断和动态重规划。
引擎负责：稳定工具、项目事实、安全约束、验证、receipt、rollback 和结构化 diagnostics。
```

核心规则：

```text
AI 决定下一步做什么。
引擎保证这一步怎样安全、可靠地完成。
Plan 可以丢弃和重写。
真实授权、项目修改、验证结果和回滚结果必须持久保存。
```

这不是取消流程和安全控制，而是重新分配流程所有权：

- 开放式任务的跨工具顺序由 AI 拥有；
- 单个确定性工程工具内部仍可封装严格流程；
- 用户授权约束“允许做什么”，不冻结“必须按什么顺序做”；
- 引擎不内置一个通用 Agent Planner，也不把某种模型的思考链写入 Core。

## 2. 为什么必须改变 252 的执行架构

252 v2 正确解决了自由表达、零散需求、Park、Reopen、Bug Diagnosis 和跨会话记忆，但把这些记忆对象与项目执行编排绑定得过深。当前代码证据包括：

```text
ChangePreparationRequest / ChangeSetProposal 强制携带 candidatePlanSteps[]。
authorize() 根据冻结步骤生成 ProjectProductionRun.stepSnapshots[]。
AdvanceRun 每次只找出并执行一个 Pending step。
Previewing 仍持有整个项目 mutation lane。
RecoverRun 只把整个 Run 粗粒度重置为 Approved。
complete_preview() 固定发送通用 UiCommandPayload::Play。
```

对应源码：

```text
rust/crates/editor_core/src/project_intent_workflow/model.rs
rust/crates/editor_core/src/project_intent_workflow/mod.rs
rust/crates/editor_core/src/project_intent_workflow/execution.rs
```

B 通道暴露的不是一个孤立 Preview Bug，而是执行所有权错误：通用 Workflow 认为自己知道完整执行顺序和完成方式，结果在项目实际要求 `project.c01.runtime` 时重新进入 `engine.empty.runtime`。真实 project-owned Preview 已通过，但 Workflow 无法自然完成，最终本引擎在 7284.991 秒时触发两小时硬截止，B7 未启动；Unity 用 2582.229 秒通过，UE 用 1742.503 秒通过。

因此，仅修复 `UiCommandPayload::Play` 或把多个 `AdvanceRun` 合并成一个大 Runner，只会让同一问题更隐蔽：

- 新任务一旦需要不同顺序，Runner 继续膨胀；
- 中途发现新事实时，冻结计划与真实需求冲突；
- 修复一个特殊项目会把项目语义泄漏进通用编排；
- 长 Run 把修改、Preview、等待、恢复和交付绑成一个故障域；
- AI 被迫适配引擎菜谱，而不是直接使用引擎能力。

结论：252 的记忆层可以保留，执行编排层应被替换，不应继续在现有 `ProjectProductionRun` 状态机上修补。

## 3. Unity + Codex、UE + Codex 为什么更快

### 3.1 Unity

Codex 通常直接在正式项目表面工作：读取和修改项目 C#、通过 Editor API / `AssetDatabase` 操作序列化对象、运行 Unity Test Framework、进入 Play，再用 `BuildPipeline.BuildPlayer` 生成 `BuildReport`。

Unity 的 `BuildPipeline.BuildPlayer` 是一个深 Module：调用方只提出构建请求，内部负责构建 Player。它没有规定 Codex 在实现护盾、修 Bug 或修改 HUD 时必须先后调用哪些编辑工具。

本项目对应真实入口：

```text
<LOCAL_TEST_ROOT>\Unity\unityTest\Assets\C01\Editor\C01ProjectBuilder.cs
```

### 3.2 UE

Codex 通常使用项目 C++ / Config、Editor API / Python / Commandlet、UObject / UMG、Automation Test、UBT 和 UAT。`BuildCookRun` 内部封装 Build、Cook、Stage、Package、Archive、Deploy、Run 等确定性交付阶段，但 Codex 自己决定何时改源码、何时验证、何时调用交付工具。

本机正式源码依据：

```text
<UNREAL_LAUNCHER_REFERENCE>\UE_5.8\Engine\Source\Programs\AutomationTool\Scripts\BuildCookRun.Automation.cs
DoBuildCookRun -> Project.Build -> Cook -> Stage -> Package -> Archive -> Deploy -> Run
```

### 3.3 真正差异

Unity 和 UE 并不是“没有流程”，而是把流程放在合适的位置：

```text
开放式需求：Codex 自己规划。
确定性工程动作：引擎工具内部执行。
每一步结果：编译器、Editor、测试、Preview、Build/Cook 日志反馈。
失败：Codex 根据新证据改计划。
```

本引擎此前多了一层强制的通用执行菜谱。它重复了 AI 的规划职责，却不能拥有项目的全部语义，所以增加了 materialize、AdvanceRun、状态转换、恢复和 Preview completion 成本。

## 4. 本引擎在成熟状态下应有的优势

方案 C 不是复制 Unity/UE 的文件操作方式。成熟后的目标优势是：

| 维度 | Unity / UE + Codex 的常见状态 | 本引擎 + Codex 的目标优势 |
|---|---|---|
| 工具发现 | 文档、源码、命令和项目约定混合 | machine-readable Tool Catalog |
| 修改安全 | 主要依赖版本控制、项目约定和引擎校验 | 每次 mutation 都有 CapabilityGrant、digest、receipt、rollback |
| 跨域修改 | C#/C++、Editor API、资源格式、构建工具各自组合 | 统一 Invocation / Result / Diagnostic 合同，底层仍保留领域工具 |
| 零基础用户 | 常需理解脚本、组件、构建设置和错误日志 | 用户只表达目标、选择取舍和批准风险；AI 读取结构化事实 |
| 失败恢复 | 由 Codex 和项目脚本自行拼接 | 工具原生提供 checkpoint、operation status、cancellation 和 rollback |
| 验证 | 不同子系统输出不同格式 | changed domains、recommended validation 和 evidence 可机器读取 |
| 审计 | 可由项目额外建设 | 默认记录授权、修改、验证、费用和回滚，不记录秘密和模型思考链 |

这些优势不是因为本引擎“AI First”这个名字自动成立。只有工具足够深、响应足够快、覆盖正式项目能力、diagnostics 足够明确，并且 AI 不必绕过它们才能完成项目时，优势才真实存在。否则 Unity/UE 的成熟工具面仍然更强。

## 5. 角色和真相分层

### 5.1 用户

用户提供：

- 想达到的可见结果；
- 当前取舍和优先级；
- 可接受的修改域、风险、时间和外部费用；
- 对删除、依赖变更、网络访问、发布等高风险动作的明确授权。

用户不需要提供完整技术步骤，也不需要把需求整理成一次性、无歧义的规格。

### 5.2 AI

AI 拥有当前任务的工作计划。它可以：

- 先检查项目，也可以先问一个真正影响结果的问题；
- 把任务拆分、合并或重新排序；
- 根据 diagnostics 改变实现方案；
- 在不超出授权的情况下增加必要的定向验证；
- 放弃未执行的计划，不需要迁移一个全局状态机；
- 在需要新风险或新语义时停止并请求新的用户决定。

AI 的隐藏推理、临时 TODO 和完整计划不是工程真相，不要求引擎持久化。

### 5.3 引擎

引擎拥有：

- Tool Catalog 与 schema；
- 当前项目 identity、digest、AssetDB、Scene/Prefab/AUI/Rule/Build Profile 等事实；
- CapabilityGrant 校验；
- 项目写入 containment 和实际 mutation；
- validation、Preview、Build/Export；
- operation、receipt、rollback 和 diagnostics。

### 5.4 真相优先级

```text
项目正式源对象 / RuntimePackage / Build Profile
  > 有效 CapabilityGrant
  > 已提交 mutation receipt / rollback receipt
  > validation / Preview / build evidence
  > IntentEvent / WorkItem / Diagnosis 记忆
  > AI 当前 Plan
  > 聊天摘要
```

Plan 可以被删除，不得用 Plan 推断某个修改已经发生。只有 receipt 和当前项目 digest 能证明修改事实。

## 6. 核心 Module 与 Seam

新增概念 Module：`AI Capability Tool Kernel`。名称表示职责，不提前规定 crate 或文件名。

它的外部 Seam 保持小而深：

```text
catalog(CatalogRequest) -> ToolCatalog
inspect(InspectRequest) -> InspectResult
execute(ToolInvocation, CapabilityGrant) -> ToolResult
observe(OperationId) -> OperationSnapshot
cancel(OperationId, CapabilityGrant) -> CancellationReceipt
```

解释：

- `catalog`：发现当前项目、平台和授权条件下可用的工具；
- `inspect`：读取项目事实、operation 状态和可授权范围，不修改项目；
- `execute`：统一执行同步或异步工具；
- `observe`：读取长操作的结构化进度和阶段结果；
- `cancel`：请求有界取消并返回实际取消结果。

顶层 Interface 不为每个资源类型增加一个方法，也不暴露 Candidate 内部状态机。具体工具通过 `ToolDescriptor + ToolInvocation` 扩展。这样新增材质、动画、导航或平台构建工具时不扩大 Kernel Interface。

Kernel Implementation 可以有多个内部 Module，但它们不是 AI 必须理解的外部 Interface。

## 7. Tool Catalog

### 7.1 ToolDescriptor 最低合同

```text
toolId
schemaVersion
summary
inputSchema
outputSchema
sideEffects
requiredCapabilities
changedDomains
costClass
expectedDurationClass
supportsDryRun
supportsCancellation
supportsRollback
diagnosticCodes
idempotencyClass
preconditions
progressEventSchema
completionEvidence
```

必须满足：

- schema 可机器读取，字段含义稳定；
- 明确只读、写项目、写生成目录、启动进程、联网和产生费用等副作用；
- 错误返回稳定 code、message、evidence 和 nextActions，不要求 AI 猜日志；
- duration / cost 是等级和预算提示，不伪造精确时间；
- 工具版本变化可被调用方检测，不静默改变输入语义。

### 7.2 工具分类

首批正式分类：

```text
inspect/search
  project binding、inventory、schema、references、diagnostics、source/asset search

mutate/import
  ProjectPatch、Controlled SourcePatch、formal asset import、Scene/Prefab/AUI/Rule/Build Profile edit

validate/test
  schema validation、domain validation、compile、targeted test、affected-domain regression

preview/build
  project-owned Preview、RuntimePackage、Build/Export、external delivery verification

checkpoint/diff/rollback
  checkpoint、project diff、receipt lookup、impact analysis、rollback
```

分类用于发现和授权，不规定调用顺序。

### 7.3 深工具而不是浅命令包装

一个正式 mutation 工具应在内部完成必要的：

```text
输入与项目 binding 校验
SafeProjectPath / ProjectWriteScope 校验
Candidate prepare / validate / apply
原子写入或明确的补偿边界
before/after digest
changed object/domain inventory
receipt 与 rollback handle
```

AI 不应被迫分别调用 `prepare -> validate -> apply -> write receipt` 才能完成一个最小修改，也不应绕过工具直接写文件。`dryRun` 可返回候选 diff 和风险；正式 `execute` 在有效授权下完成整个可靠动作。

确定性长流程可以封装成深工具，例如：

```text
project.preview
project.build_export
project.delivery_verify
quality.authoritative_regression
```

它们只封装自身确定性职责，不决定一个开放式任务何时必须调用它们。

## 8. CapabilityGrant 有限授权模型

### 8.1 目标

授权回答“AI 在什么范围内可以做什么”，不回答“AI 必须按哪些步骤做”。

最低合同：

```text
grantId
schemaVersion
projectIdentity
userVisibleOutcomeDigest
initialBaseDigest / scopeDigests
allowedDomains
allowedMutationKinds
allowDelete
allowDependencyChange
allowNetwork
externalCostBudget
timeBudget
maxMutationCount
expiresAt
issuedBy
grantDigest
```

Grant 明确禁止保存固定 `candidatePlanSteps[]`、工具顺序或模型思考链。

`CapabilityGrant` 必须来自用户对当前可见结果和风险范围的批准。Kernel 可据此为每次实际 mutation 派生一次性 `ProjectCandidateApproval`：它仍严格绑定 candidate digest、validation digest、project lineage 和 Grant digest，但不是一次新的用户询问。若 Candidate 的结果、domain 或风险超出 Grant，派生失败并请求新的用户决定，不能自行扩大授权。

### 8.2 授权继承链

固定 base digest 不能在第一次合法修改后让同一授权失效。因此 Kernel 维护授权 lineage：

```text
initialBaseDigest
  -> mutation receipt.afterDigest
  -> next mutation receipt.afterDigest
  -> ...
```

只有由同一有效 Grant 下、已提交 receipt 导出的 digest 才能继续该 lineage。外部修改、未知写入或 receipt 缺失导致 drift 时 fail-closed，AI 必须重新检查并决定是恢复、接受新 base 还是请求新授权。

### 8.3 授权等级

```text
ReadGrant：项目内只读检查、搜索和状态观察。
ScopedMutationGrant：限定项目、域、修改种类、时间、次数和费用的修改。
ElevatedGrant：删除、依赖变更、网络费用、发布、Engine Core 或其它高风险动作。
```

同一个语义稳定的任务通常只需要一次 `ScopedMutationGrant`。以下情况必须重新请求决定或授权：

- 用户可见结果发生实质变化；
- 需要进入未授权 domain；
- 需要删除、增加依赖、联网付费或发布；
- 成本、时间或 mutation 次数超过预算；
- 项目发生无法归因于当前 receipt lineage 的 drift；
- 工具发现授权范围不足且没有等价低风险方案。

普通 repair、工具顺序变化、增加定向测试或替换未执行计划，不需要重复批准。

### 8.4 短 mutation lane

同一项目的真实 Apply 默认串行，但 mutation lane 只覆盖实际提交窗口，不覆盖整个 AI 任务、编译、Preview、用户等待或结果观察。只读检查和隔离验证可在已证明资源隔离时并行。

这直接取代 252 中 `Previewing` 仍持有 mutation lane 的设计。

## 9. ToolResult、Operation 与 Receipt

### 9.1 ToolResult

同步工具直接返回结果；长工具返回 `OperationId`。统一结果至少包含：

```text
status
toolId / toolVersion
operationId
facts
diagnostics[]
suggestedNextActions[]
changedDomains[]
receiptRef
evidenceRefs[]
timing
externalCost
```

`suggestedNextActions` 是工具建议，不是强制状态转换。AI 可以选用其它满足授权和验收的动作。

### 9.2 Operation

长操作必须：

- 按阶段发布 started/progress/completed/failed/cancelled；
- 可在进程或 Editor 重启后 `observe`；
- 区分“取消请求已接收”“子进程已终止”“无法中断但结果将被丢弃”；
- 保存有界 stdout/stderr、结构化 diagnostics 和 artifact refs；
- 不依赖调用方持续轮询某个 `AdvanceRun` 才能继续内部确定性工作。

### 9.3 MutationReceipt

```text
receiptId
operationId
toolId / toolVersion
grantDigest
projectIdentity
beforeProjectDigest
afterProjectDigest
changedPathsOrObjects[]
changedDomains[]
candidateDigest / validationDigest
rollbackHandle
status
diagnostics[]
timing
externalCost
```

Receipt 记录已经发生的事实，不记录完整 prompt、模型隐藏推理或 API Key。

### 9.4 Rollback

Rollback 是工具能力，不是“把项目想象成回到原状态”。它必须返回独立 `RollbackReceipt`，说明：

- 回滚了哪些对象；
- 当前 project digest；
- 哪些外部副作用不可逆；
- 是否存在后续验证要求；
- 是否因为 drift 而拒绝回滚。

## 10. AI 自主规划与动态重规划

以下是允许的反馈循环，不是引擎硬编码状态机：

```text
用户目标与 Grant
  -> AI catalog / inspect
  -> AI 选择一个工具
  -> Kernel execute
  -> ToolResult / diagnostics / receipt
  -> AI 判断目标是否满足
       -> 未满足：改变计划或继续调用工具
       -> 需要新取舍/权限：询问用户
       -> 满足：运行适当验收并报告结果
```

关键性质：

- AI 可以只做一次很小的修改，也可以跨多个 domain；
- 工具失败不会自动把整个任务恢复到第一步；
- 新证据可以推翻旧假设；
- 已提交修改由 receipt 管理，未执行计划直接丢弃；
- AI 可以在一个 Bug 修复中多次复现、诊断和验证，不需要预先知道次数；
- 引擎不要求所有任务最终收敛为同一种 Run state sequence。

## 11. IntentJournal、WorkItem 与 Diagnosis 的新职责

### 11.1 保留

保留以下价值：

- 不可变用户原话和附件来源；
- 可纠正的长期理解；
- Park、Resume、Reopen、Merge、Split 和关系；
- Bug symptom、evidence、hypothesis 和 root cause 历史；
- 跨会话恢复项目背景；
- 将 receipt、validation 和交付证据关联回用户目标。

### 11.2 降级

它们不再是所有工具调用的强制前门：

```text
简单只读问题可以直接 inspect。
已明确的小修改可以在有效 Grant 下直接调用工具。
WorkItem 不要求先变成 candidatePlanSteps[]。
Journal 不拥有 mutation lane。
Diagnosis 不规定 AI 必须经过固定假设数量。
```

### 11.3 自动记忆与用户负担

AI 或 Editor 可以在后台把重要目标、决定、receipt 和结果写入 Journal。零基础用户不需要维护 issue tracker，也不需要为了让引擎继续工作而手工整理 WorkItem。

长期记忆可以缺失或重建；缺失记忆不能伪造授权，也不能改变项目事实。

## 12. 对 250、251、252 的处理

### 12.1 250 保留为能力底座

继续保留并收敛为 mutation 工具内部 Implementation：

```text
ProjectReadiness
CandidateProjectRevision
Controlled SourcePatch
Formal Asset Import
ProjectCandidateEntry::inspect_project_binding
ProjectCandidateEntry::prepare / prepare_with_source_file
ProjectCandidateEntry::validate
ProjectCandidateEntry::apply
ProjectCandidateEntry::rollback
SafeProjectPath / ProjectWriteScope
receipt / digest / diagnostics
```

这些能力是方案 C 的安全优势，不应删除。底层 `ProjectCandidateApproval` 由有效 CapabilityGrant 为单个已验证 Candidate 派生，保留精确绑定，不把一次语义授权重新拆成多次人工批准。

### 12.2 251 保留为权威能力 Gate

Provider-independent From-Blank Creation Golden Gate 继续证明工具可以从空项目到 Preview、Export、外部运行。它不再证明所有任务都必须经过固定 ProjectProductionRun。

### 12.3 252 部分保留、部分被 253 取代

保留：

```text
IntentEvent
WorkItem
Diagnosis
跨会话 Journal / Snapshot
用户语义与 receipt/evidence 关联
```

被 253 取代为开放式任务默认路径：

```text
ProjectIntentWorkflow 作为唯一项目修改前门
强制提前冻结完整 candidatePlanSteps[]
ProjectProductionRun 规定跨工具执行顺序
外部反复 AdvanceRun
Previewing 长期占用 mutation lane
RecoverRun 粗粒度恢复整个 Run
通用 Workflow 决定项目 Preview completion
```

历史 252 施工和测试证据保持真实，不回写成“从未实现”。253 是后续架构优先级更高的正式解释。

## 13. Project Binding 与 project-owned Preview

所有 Preview、test、Build/Export 工具必须从当前项目 binding 解析真实执行入口：

```text
project identity
RuntimeModule identity
RuntimePackage assembler
Preview player / adapter
Build profile
delivery entry
```

禁止由通用 Workflow 写死 `engine.empty.runtime` 或使用无法表达 project-owned runtime 的通用 Play payload。

`project.preview` 自己负责：

- 校验 project binding 和当前 digest；
- 装配或复用正确 RuntimePackage；
- 启动项目拥有的 RuntimeModule / Player；
- 返回可观察 operation 和 Preview evidence；
- 关闭后释放资源；
- 不持有 mutation lane。

Preview 失败只表示该工具失败。AI 根据已提交 mutation receipt、diagnostics 和目标决定修复、回滚、换验证方式或请求用户决定。

## 14. 快速迭代与里程碑交付分离

方案 C 明确区分两个节奏：

### 14.1 Interactive Loop

用于日常修改：

```text
小范围 inspect
最小 mutation
定向 validation/test
project-owned Preview
```

目标是快速反馈。它不默认运行完整 workspace、完整 Windows Export 或全量权威 Gate。

### 14.2 Milestone Delivery

用于用户明确要求交付、发布或阶段验收：

```text
候选冻结
受影响域回归
环境等价预检
一次 authoritative regression
Build/Export
项目外交付与无参数运行验收
```

`quality.authoritative_regression` 和 `project.build_export` 可以是深工具。AI 决定何时达到里程碑条件，引擎保证工具内部按正式纪律执行。

## 15. 多任务、复杂任务、需求变化和 Bug Reopen

### 15.1 多任务

- 每个任务可以有独立 AI plan 和可选 WorkItem；
- 只读 inspection 在资源隔离成立时可并行；
- 同一项目 Apply 通过短 mutation lane 串行；
- receipt lineage 防止两个任务基于旧 digest 静默覆盖；
- 未获授权的任务不能借用另一个任务的 Grant。

### 15.2 复杂任务

复杂度由 AI 逐步消化，不通过预先生成巨大步骤数组消化。AI 可以先建立最小纵切，再根据实际 Preview 和测试增加工具调用。

### 15.3 需求变化

用户继续表达时：

- 只是补充背景：更新记忆，当前 Grant 可继续；
- 改变用户可见结果但仍未修改：AI 丢弃旧 Plan，重新整理；
- 改变已授权结果：停止后续 mutation，请求新授权；
- 增加无关未来想法：Park，不阻塞当前任务；
- 改变已提交结果：基于当前 digest 形成新 mutation，不篡改旧 receipt。

### 15.4 Bug Reopen

Bug 再次出现时，恢复历史 symptom、evidence、receipt 和回归结果，但重新读取当前项目事实。旧 root cause 只是先验，不得冒充当前诊断结论。

## 16. 安全、失败、停止和恢复

### 16.1 Fail-closed

以下情况拒绝 mutation：

- Grant 无效、过期或不覆盖工具；
- project identity、scope 或 lineage drift；
- 工具输入 schema 不合法；
- SafeProjectPath / ProjectWriteScope 不成立；
- 删除、依赖、网络或费用权限不足；
- validation 未通过；
- mutation lane 被真实 Apply 占用；
- receipt 无法可靠写入。

### 16.2 失败不是统一回退

工具失败返回局部事实：什么没发生、什么已经发生、当前 digest、是否可重试、是否可回滚。AI 决定下一步。禁止把所有失败粗暴地重置为某个 `Approved` 状态。

### 16.3 停止

- 用户可以取消当前 operation 或撤销未使用 Grant；
- 到达 time/cost budget 后不启动新 operation；
- 已提交 mutation 不因 AI 会话停止而消失；
- 后台 operation 必须可 observe，并按 243 生命周期规则清理 worker 和凭据；
- 不可中断的外部进程必须如实报告，不伪装已取消。

### 16.4 恢复

恢复依赖当前项目 digest、operation snapshot、receipt 和 Grant，而不是依赖旧聊天窗口仍存在。若 Grant 已过期，可以只读恢复事实，再请求新的最小授权。

## 17. CLI、MCP 与 Editor Adapter

Tool Kernel 只定义一个正式 Interface。不同入口通过 Adapter 使用同一 Tool Catalog：

```text
Native Editor Adapter：面向零基础用户的自然语言、审批、进度和结果 UI。
CLI Adapter：本地自动化、测试和可复现脚本。
MCP Adapter：Codex 或其它 Agent 的结构化工具发现和调用。
Test Adapter：确定性 fake/in-memory project 与 fault injection。
```

Adapter 不复制授权、项目写入、Candidate、receipt 或 validation 逻辑。Editor、CLI 和 MCP 对同一 invocation 必须产生等价的 Kernel 语义。

只有两个真实 Adapter 出现时才建立具体内部 Seam；不得为假设中的 Provider 或 UI 预造大量 pass-through 层。

## 18. Provider-independent 约束

- Tool Catalog、CapabilityGrant、Invocation、Result、Receipt 和 Diagnostic 不包含特定模型字段；
- 引擎不依赖 Provider 输出隐藏推理；
- Provider 只生成或选择结构化 invocation，不获得额外文件权限；
- imported Codex、本地模型、远程模型和无模型手动调用共享同一 Kernel；
- Provider 失败不破坏已提交 receipt、项目状态或长期记忆；
- API Key 只存在于既有受控 credential lease，不进入 Journal、Grant 或 report；
- 更换 Provider 不要求迁移项目工程真相。

## 19. 性能与报告分档

### 19.1 性能原则

- Catalog 和轻量 inspect 不触发完整编译；
- changed domains 驱动定向 validation，不默认扫描全 workspace；
- mutation 工具复用稳定进程、缓存和已验证项目索引；
- Preview、Build 和 authoritative regression 分开；
- 冷缓存、cache hit/miss 和外部等待单独报告；
- 失败先跑累积定向失败集，不每次重跑完整权威 Gate；
- 长 operation 发布阶段事件，不允许数小时黑盒等待。

### 19.2 报告分档

继续遵守全局 `Off / Summary / Trace`：

```text
Runtime：默认 Off 或功能必需 compact result。
Editor 日常：Summary。
测试、Gate、debug 或用户显式诊断：Trace。
```

Tool Result 默认返回有界 Summary；完整命令、逐阶段日志和 candidate trace 通过 evidence ref 按需读取，不把大 JSON 常驻热路径。

## 20. 分阶段施工边界

本节只定义未来施工拆分，不是施工文档或开工授权。

### Phase 1：Capability Tool Kernel 合同与 Inventory

- 盘点现有 inspect、Candidate、validation、Preview、Build、rollback 能力；
- 定义 ToolDescriptor、Invocation、Result、Operation、Receipt schema；
- 建立最小 Kernel Interface 和确定性 fake；
- 不迁移项目功能，不删除 252。

### Phase 2：CapabilityGrant 与短 mutation lane

- 有限授权、receipt lineage、drift、budget 和 elevated capability；
- 把 lane 收缩到实际 Apply；
- 建立 revoke、expiry、cancel 和 reopen 合同。

### Phase 3：现有能力工具化

- 将 250 的 ProjectCandidateEntry 三类 lowering 收进正式 mutation tools；
- 建立 inspect/search、validate/test、checkpoint/diff/rollback 工具；
- 禁止 CLI/MCP/Editor 复制实现。

### Phase 4：Project-owned Preview / Build / Delivery

- 修复并工具化项目 binding 解析；
- Preview 直接进入正确 RuntimeModule / Player；
- Build/Export、项目外交付和无参数运行成为可观察深工具；
- 不再由 ProjectIntentWorkflow 决定 completion。

### Phase 5：Agent Adapter 与自主反馈闭环

- MCP/CLI/Editor Adapter 读取 Catalog 并调用 Kernel；
- AI 可以根据 ToolResult 自主重规划；
- 不实现通用硬编码 Agent Planner；
- 不要求外部 `AdvanceRun` 推动长工具内部阶段。

### Phase 6：252 执行层降级与兼容迁移

- IntentJournal / WorkItem / Diagnosis 保留为可选记忆；
- 移除其对 mutation、Preview 和工具顺序的所有权；
- 历史 Journal 可读取，旧 active Run 通过明确 migration/close receipt 收口；
- 不让两套执行真相长期并存。

### Phase 7：竞争 Gate

- 从空项目、已有项目持续修改、Bug Reopen、Save/reopen、Preview 和 Windows delivery；
- provider-independent、CLI/MCP/Editor 等价；
- 授权、drift、取消、rollback 和失败注入；
- 重跑相同 B 通道 cohort 前先完成本引擎单通道预检。

未来施工文档必须逐 Phase 写定向测试、受影响域回归和最终权威回归，并重新核对当时代码基线。本文不预先指定代码文件和命令。

## 21. 验收标准

### 21.1 架构

- 开放式任务没有统一强制 `candidatePlanSteps[]`；
- AI 可以在反馈后改变工具顺序而不迁移全局 Run；
- Kernel 外部 Interface 不随工具种类线性膨胀；
- 规划器不进入 Engine Core；
- IntentJournal/WorkItem 缺失不阻止已授权的小型工具任务；
- 当前项目事实、授权、receipt 和验证之间没有第二套真相。

### 21.2 授权与安全

- 一个语义稳定任务可以在一次 ScopedMutationGrant 下完成多次必要 mutation；
- 顺序变化和定向 repair 不重复询问用户；
- 删除、依赖、网络费用、跨域和语义变化必须升级授权；
- 每次 mutation 都有 before/after digest、changed domains、receipt 和明确 rollback 能力；
- 未知 drift、越界路径和 receipt 写入失败均 fail-closed；
- mutation lane 不跨 Preview、Build 或用户等待。

### 21.3 工具和恢复

- AI 可通过 Catalog 理解输入、输出、副作用、权限、耗时和诊断；
- 长 operation 可 observe、cancel、重启后恢复；
- project-owned Preview 解析正确 RuntimeModule，不回落到 empty runtime；
- Tool failure 返回局部事实和 nextActions，不要求解析非结构化长日志；
- CLI、MCP、Editor 使用同一 Kernel 语义。

### 21.4 产品

- 零基础用户只需要表达目标、纠正结果和批准真实风险；
- 零散需求、Park、跨会话和 Bug Reopen 仍可用；
- 简单任务不被迫建立完整 WorkItem/ChangeSet/ProductionRun；
- 复杂任务可以经历任意次数的检查、修改和 repair；
- 同一 B 通道本引擎必须完成 B7 和 Windows 外部交付，不得再因 Workflow completion 停止；
- 在重跑正式 cohort 前，本引擎预检应明显低于两小时上限，并分离框架开销、编译、测试、Preview 和交付耗时。

## 22. 明确不做

本方案不做：

- 在 Engine Core 中实现一个通用 Agent Planner；
- 保存模型隐藏思考链或要求 Provider 暴露 chain-of-thought；
- 用另一个更大的统一 Production Runner 取代 252；
- 让 AI 获得无边界项目写入、删除、联网或费用权限；
- 取消 Candidate、validation、approval、receipt、digest、rollback 或 SafeProjectPath；
- 把所有领域能力压成一个任意 shell 工具；
- 把 Unity/UE 的 C#/C++、二进制资产格式或编辑器内部结构直接照搬进本引擎；
- 在本方案中生成施工文档、修改代码、启动 Gate 或重跑 B 通道；
- 同时重写渲染、ECS、AUI、AssetDB、RuntimePackage 或 Build Graph。

## 23. 风险与约束

| 风险 | 约束 |
|---|---|
| AI 自主规划后乱调用工具 | Catalog 明确副作用；Grant 约束能力；mutation 前 validation；所有结果可观察 |
| 一次授权过宽 | project/domain/kind/time/cost/count/expiry 多维限制；高风险独立 ElevatedGrant |
| 一次授权过窄导致频繁询问 | 授权用户可见结果和风险范围，不冻结文件清单与步骤顺序 |
| 工具粒度过细，AI 仍被迫编菜谱 | mutation、Preview、Build 等做成深工具，隐藏确定性 Implementation |
| 工具粒度过大，失败难定位 | Operation 阶段事件、局部 diagnostics、receipt 和 evidence refs |
| 两个 Agent 相互覆盖 | 短 Apply lane + digest lineage + drift fail-closed |
| AI 只看建议形成新硬流程 | suggestedNextActions 只是建议，Kernel 不强制跨工具 transition |
| 旧 252 与新 Kernel 双写 | Phase 6 明确迁移并取消执行所有权，不长期维护双真相 |
| Tool Catalog 变成浅 wrapper 大全 | 删除测试：删掉 Module 后复杂度应重新散落；否则不值得新增工具 |
| MCP/CLI/Editor 行为漂移 | Adapter 只转换协议；合同测试通过同一 Kernel Interface |
| 自主循环无休止 | Grant time/cost/count budget、operation timeout、用户 cancel 和完成验收 |
| 安全治理拖慢交付 | receipt/validation 内置于工具，日常 Summary；权威 Gate 只在里程碑执行 |

## 24. 外部审查与竞争证据吸收

### 24.1 已吸收

从 53 号协议与 B8 汇总吸收：

- 本引擎完整治理不能抵消 B7 未启动和 Windows 交付缺失；
- 真实缺口是 Workflow 与 project-owned Preview/执行入口不一致；
- harness 冷编译、proposal repair、payload boundary repair 和重复编排造成可避免成本；
- Unity/UE 通过项目自然表面、成熟工具和反馈闭环完成任务；
- Build/Export 的确定性阶段应封装，但开放式需求不应硬编码；
- 最终评价先判硬 Gate，不用主观治理加分抵消交付失败。

### 24.2 对 B8 原建议的修正

B8 建议“建立单一、可恢复、有阶段进度的 authoritative production runner”。本方案只采纳其中“单一权威回归、可恢复、阶段进度”作为里程碑深工具，不采纳把它扩展为所有项目修改的统一 Runner。

原因：

```text
authoritative regression / Build/Export 是确定性工程动作，适合深工具。
实现任意玩法、修 Bug、调整 UI 是开放式推理，必须由 AI 动态规划。
```

### 24.3 无需吸收

- 不把 B 通道冻结的护盾事件流写进产品流程；它只用于公平测试；
- 不把 Unity/UE 的弱审计复制为本引擎默认；安全治理保留但下沉；
- 不根据一次 B 通道时间直接设定某个语言、渲染或 ECS 重写目标；证据不支持该结论。

## 25. 方案自审

### 25.1 是否真的把规划权交给 AI

通过。Kernel 没有跨工具状态机；Catalog 只描述能力；ToolResult 的 nextActions 只是建议；Plan 不进入工程真相。未来施工若重新要求完整 `candidatePlanSteps[]` 或以固定 Run 控制所有任务，即违反本方案。

### 25.2 是否保留必要安全

通过。ProjectCandidateEntry、validation、approval、SafeProjectPath、digest、receipt、rollback 和 drift 检查全部保留，并通过 CapabilityGrant 收敛为工具执行条件。方案 C 不是“让 AI 直接随便改文件”。

### 25.3 WorkItem 是否仍是 Gate

否。WorkItem 只负责长期组织、来源和跨会话记忆。简单 inspect、小修改和工具恢复不要求先创建 WorkItem；语义和风险授权由 Grant 负责。

### 25.4 是否偷偷新增通用 Agent Planner

否。AI 规划发生在 Codex/Agent 会话；Engine Core 只提供工具和事实。MCP/CLI/Editor 是 Adapter，不拥有计划。

### 25.5 Module 是否足够深

通过。Kernel 只有五个外部操作；新增工具不扩大顶层 Interface。Candidate 链、安全写入、receipt 和 rollback 藏在 mutation 工具 Implementation 内。Build/Export 只封装确定性复杂度。

### 25.6 是否能处理复杂任务和反复修改

通过设计。未执行 Plan 可随时重写，已执行事实由 receipt lineage 管理；用户变化只在改变语义或权限时使 Grant 失效；Bug Reopen 读取历史但重新验证当前事实。

### 25.7 是否尊重当前工程边界

通过。Rust Native Runtime、RuntimePackage、ProjectRuntimeModule、AssetDB、AUI、Projection、Build Graph 等正式所有权不变；253 只重构 AI 与这些能力之间的 authoring / automation Seam。

### 25.8 是否越权施工

否。本文没有生成施工文档，没有修改 Rust 代码，没有运行 Gate，没有激活 `施工文档/当前/`，没有重跑三引擎 cohort。当前与待执行施工槽位保持为空。

## 26. 最终决定

```text
正式采用方案 C。

新的默认架构：AI Capability Tool Kernel + Agent-Owned Planning + CapabilityGrant + Receipt Lineage。

252 的 IntentJournal / WorkItem / Diagnosis 保留为可选长期记忆；
252 的 ProjectProductionRun / candidatePlanSteps / AdvanceRun 不再作为开放式任务默认执行真相。

250 的 Candidate、安全写入、validation、receipt、rollback 继续作为工具内部底座；
251 的 from-blank Gate 继续作为能力验收；
project-owned Preview、Build/Export 和 authoritative regression 建设为独立深工具。

下一步若用户要求施工，必须先依据本文生成并自审新的施工文档；
在此之前不得改代码、激活施工或重跑 B 通道。
```
