# 254-R2 - Production Conformance / Candidate Lifecycle / Real Codex Outcome Acceptance v1 方案（历史）

> 状态：`superseded_by_254_scope_correction`；2026-07-22 用户确认 254 不负责证明精确引擎版本通过真实 AI 验收，本方案整体退出当前规范与施工主线，仅保留历史证据
> 历史终态：R2-FC6 已精确执行一次并以 `source_snapshot.file_set_mismatch` terminal failed；没有 candidateId，cleanup 完整，不得重试或改写为成功
> 日期：2026-07-21
> 当前规范：`254-AI-Tool-Gateway-Codex-Adapter-v1方案.md` 第 30 节
> 历史关系：曾替代 `254-R1-Real-Codex-Client-Integration-User-Acceptance-Gate-v1方案.md`，现与 R1 一并作为历史诊断证据
> 性质：历史架构与失败证据说明；不是当前正式方案、施工文档、施工授权、可恢复入口或重跑授权

## 0. 当前历史定位（最高优先级）

本节覆盖本文其余所有历史措辞。254 当前只负责向 AI 提供好用、自由、可审计的引擎工具；不负责生产精确引擎 candidate，也不负责通过真实 Codex outcome acceptance 证明某个精确引擎版本。

因此，本文曾定义的以下公共生命周期已经退出 254：

```text
ProductionCandidateModule::produce
CodexOutcomeEvaluationModule::evaluate
candidateId / activationId / attemptId production lifecycle
release candidate preparation/conformance/seal/reopen
real-session attempt observation、terminal seal 与 outcome validation
```

本文第 1-18 节继续保留 B+C、bounded goal-level Grant、Gitlink、immutable snapshot / writable workspace、failure artifact 和历史自审上下文，目的是解释已经发生的设计与失败，不表示这些合同仍应实现。任何“必须补齐”“后续执行”“当前公共 Module”或“等待 remediation”语句都只能按历史事实阅读。

### 0.1 与 254 Core 的分离

必须保留的是 `project.mutate.candidate`、`ProjectCandidateEntry`、mutation receipt 与 rollback handle：它们描述 AI 对用户项目的一次受控修改，属于 254 Tool Kernel 的可审计工具能力。

必须退役的是 `ProductionCandidateModule`：它描述精确引擎发布候选的 source snapshot、release build、conformance、seal 与 reopen。二者虽然都曾使用 candidate 一词，但身份、所有者和产品价值不同，不得因本方案退役而删除项目 mutation Candidate，也不得再把引擎发布 candidate 接回 AI 工具流程。

### 0.2 废弃源码合同

后续获得独立施工授权后，R2 lifecycle 中只服务退役职责的源码与测试必须使用 `git mv` 归档到：

```text
legacy/rust/254-r2-lifecycle/
  README.md
  production-candidate/
  outcome-evaluation/
  shared-lifecycle/
  tests/
```

该目录是不可编译、不可依赖、不可发布的历史源码快照：

1. 不加入 Cargo workspace，不提供可构建 `Cargo.toml`、`build.rs`、feature 或兼容入口。
2. active Rust、Cargo dependency、production binary、Adapter、测试与生成清单对 `legacy/**` 必须为零依赖。
3. 只被 R2 lifecycle 使用的完整文件和测试一起归档；混合文件先按真实 consumer 拆分。具有独立 active consumer 的 Core 能力保留在其真实所有者中，不复制两份 Implementation。
4. 不保留旧 CLI 参数、公共 re-export、默认 production composition、active shim 或专用 legacy error 来维持退役 Interface。
5. archive `README.md` 必须记录原始路径、归档 commit、退出原因、R2-FC6 最后终态、相关方案、错误归档和完成记录，并明确不得作为施工入口。
6. Git history 与该目录共同承担追溯责任；历史代码无需继续编译或通过旧测试。

### 0.3 当前授权边界

当前已授权并完成 254 正式范围修订、本方案历史定位，以及独立退役施工文档的编写、自审和激活前复核。R2 lifecycle retirement 已完成 R2-LR0 至 R2-LR6，当前执行 R2-LR7；授权只覆盖解除 active consumer、归档 archive-only 源码和 Core 回归。不得继续 `ProductionCandidateModule` remediation，不得创建第二个 Fresh Candidate、activationId、attemptId 或真实 Codex acceptance。

## 1. 决策摘要

采用简化的 B+C 组合：

```text
B：先用 Production Conformance 确定性证明每个正式工具与生命周期 Module 的合同。
C：再用真实 Codex 对开放用户目标做 outcome acceptance，验证结果与安全不变量。
```

同时采用：

```text
bounded goal-level Grant
candidateId / activationId / attemptId 分离
seal 前 preparation 可重复、candidate 确定性失败终止、attempt inconclusive 可重做
source snapshot 对 Git mode 感知；本地 exact clean gitlink 展开为自包含 material closure
immutable source snapshot 与 writable conformance workspace 是 Module 内部两种不可混淆的 capability role
```

总原则：

```text
AI owns the cross-tool plan.
Each tool owns its internal deterministic workflow.
Grant constrains risk, not implementation.
Acceptance validates outcomes and safety invariants, not a trace recipe.
```

## 2. 问题定义

254 Core 的目标是让外部 AI 通过正式、可审计、项目绑定的工具自由完成复杂工程任务。R1 的多次失败并不说明 AI 无法使用这些工具，而是说明验收架构把四类不同问题压进了一次不可重试运行：

```text
生产包是否等价且可启动。
候选是否被正确、不可变地封存。
本机配置是否正确激活且可恢复。
真实 AI 是否能自主完成用户目标。
```

R1 又把最后一项写成固定调用菜谱。于是合法额外读取、动态重规划、不同工具顺序或与目标无关能力的省略都会成为“协议违约”；candidate manifest 同时吸收 plan、config 和未来执行事实，产生循环绑定；一次审批 TTL 或外部中断也被误当成 candidate 失败。

R2 必须解除这些耦合，同时保留真正必要的生产真实性、安全性和证据完整性。

## 3. 目标与非目标

### 3.1 目标

```text
证明真实发布内容与正式工具合同一致。
建立无环、可审计、可回滚的 candidate/activation/attempt 生命周期。
让用户按目标、风险与预算授权，而不是批准未来工具顺序。
让真实 Codex 自主规划并按证据动态调整。
用结果、不变量和实际证据验证开放目标。
明确 inconclusive 与 rejected，避免把人工或外部中断误判为产品失败。
```

### 3.2 非目标

```text
不设计统一 Agent Planner 或固定产品 Runner。
不要求 AI 使用某组工具、固定次数、固定参数或固定总顺序。
不开放 shell、任意项目根外写入或反射式内部 API。
不重写 Tool Kernel、RuntimePackage、AUI、Scene、AssetDB 或 Build Graph ownership。
不在本方案中生成施工文档、修改代码或授权任何真实 Gate。
```

## 4. 备选方案与选择

### 4.1 方案 A：继续修补 R1 frozen plan

优点是复用现有 freezer、action validator 和 acceptance artifact。缺点是验收继续测“是否照菜谱执行”，无法证明 AI 能面对开放需求；每增加工具或合法分支都会扩张计划 schema 和负向矩阵。否决。

### 4.2 方案 B：只做工具级 Production Conformance

优点是确定性强、故障定位清晰。缺点是自动化客户端无法证明真实 Codex 的 discovery、typed MCP 理解、动态组合和人工授权体验。单独使用不足。

### 4.3 方案 C：只做真实 Codex 开放目标验收

优点是接近真实用户价值。缺点是失败时难以区分生产包、工具合同、配置、客户端行为或外部中断，仍会形成高成本重试循环。单独使用不足。

### 4.4 选择：简化 B+C

先由 B 关闭工具与生命周期的确定性合同，再由 C 只验证真实客户端完成开放目标。两层共享 identity、receipt、evidence verifier 和安全不变量，但不共享固定调用计划。这样既保留真实验收，也将故障归属缩小到可诊断边界。

## 5. 保留的 254 Core

R2 不推翻 254。下列架构继续有效：

```text
Editor 托管 Gateway Core，MCP/CLI 为外置薄 Adapter。
AiToolContractRegistry -> project-aware Catalog -> typed MCP projection。
Tool Kernel Core 是 Grant、mutation、receipt、rollback、operation 的唯一语义真相。
session/project binding、read generation、digest drift 与显式 reconnect。
广泛观察 + 领域深工具 + ProjectPatch/Controlled SourcePatch/Asset Import 逃生通道。
exact presented-frame Preview evidence、视觉诊断、Build/Delivery。
AI 跨工具规划和动态重规划。
```

R1 已经修正且仍有效的 typed MCP、direct-input ownership、Read/mutation 分离、approval lifecycle、exact-frame evidence 和 reconnect 合同继续保留。

## 6. 规划权与严格性边界

### 6.1 AI 拥有工具外计划

AI 根据用户目标和当前证据决定：

```text
先观察什么。
使用哪个领域工具或通用逃生通道。
是否以及何时 mutation。
遇到诊断或 digest 变化后如何改计划。
目标是否需要 Preview、视觉诊断、Build 或 Delivery。
完成策略是保留修改还是恢复初始状态。
```

Catalog、tool result 和 diagnostics 可以返回事实、能力、前置条件及可恢复错误，但不得返回要求客户端执行的全局 `next_action`。

### 6.2 工具内部允许严格工作流

单个工具或生命周期 Module 可以内部拥有 deterministic stages、deadline、原子发布、process ownership、故障注入、rollback 和 cleanup。严格步骤封装在 Module 内，由 Module 负责执行和证明；调用者不应学习或逐阶段编排这些内部 seam。

### 6.3 只保留因果依赖

跨工具只能验证真实因果关系：

```text
mutation 必须发生在覆盖该 side effect 的 Grant active 之后。
receipt 必须引用实际 operation、project、前后 digest 与 Grant lineage。
Delivery verification 必须引用真实 Build artifact。
rollback 必须引用真实 mutation receipt。
evidence consumer 必须引用匹配 project/digest/operation ownership 的 evidence。
```

不得把一个特定目标的合理路径升级为所有目标必须遵循的总顺序。

## 7. bounded goal-level Grant

### 7.1 GoalBinding

```text
goalId
userVisibleOutcome
projectIdentity
initialProjectDigest
completionPolicy
```

`completionPolicy` 明确成功时是保留目标状态、交付 artifact，还是在验证后恢复初始状态；它不描述工具步骤。

### 7.2 RiskEnvelope

```text
riskClass: ProjectOwnedLowRisk | higher explicit class
optional allowed/denied paths or object scopes
mutation budget
time budget
cost budget
deleteAllowed
dependencyChangeAllowed
networkAllowed
```

### 7.3 授权语义

用户看到并批准的是目标、影响范围、风险标志和预算。一次 bounded Grant 可以覆盖完成同一目标所需的多次低风险 mutation；read 不消费 mutation budget，默认 mutation budget 不得退化为 1。AI 改变实现方案但仍处于包络内时无需重复批准；目标、scope、风险标志或预算扩张时必须申请新 Grant。

Grant 的 forward mutation authority 可以过期或撤销，但已经发生的 mutation receipt 必须保留 rollback authority，避免 TTL 到期后无法清理。Gateway Core 拥有 request、decision、active、revoke、expire、project-switch cleanup 和 stale-decision rejection；UI 只投影 typed request。

### 7.4 Approval digest

approval digest 必须覆盖规范化的 `GoalBinding + RiskEnvelope + project/session binding`，不得只覆盖某次 candidate payload、第一步 mutation 或固定 action plan。Grant receipt 记录用户决定、批准摘要、authority scope、预算初值和时间边界。

## 8. 身份模型

### 8.1 package identity

package identity 描述经过 preparation/conformance 的不可变发布内容，包括 source/build/tool contracts/fixture compatibility 所需摘要。它不包含本机 activation config、未来 attempt 或 AI plan。

### 8.2 candidateId

candidateId 可以在 `ProductionCandidateModule::produce` 的 staging 内部预分配，以便形成最终目录名和待发布 manifest；但只有 atomic publish 与 reopen 成功后才生效、可观察并返回给 caller。seal 前的 preparation 可以重复，失败必须清理 staging；未发布的预分配值不可引用、不可复用，也不构成一个 candidate。candidate sealed 后不可修改；任何需要改变 package 或 candidate-bound contract 的修订都创建新 candidate。

### 8.3 activationId

每次把 candidate 激活到本机真实 Codex/Editor 环境均创建新 activationId。activation 通过 compare-and-replace 修改精确配置，记录 before/after digest、lease、process/session binding 与 rollback receipt。activation/config receipt 不进入 candidate identity。

### 8.4 attemptId

每次真实 Codex outcome acceptance 开始均创建新 attemptId，绑定 candidateId、activationId、client identity、project identity、initial digest、goalId 和 Grant lifecycle。一个 attempt 的 transcript、receipt 或 evidence 不得与另一个 attempt 拼接。

## 9. 公共 Module 与 Interface

公共生命周期只保留两个目标级深 Module：

```text
ProductionCandidateModule::produce(request)
  -> CandidateDisposition

CodexOutcomeEvaluationModule::evaluate(candidate, goal, riskEnvelope)
  -> EvaluationOperation
```

公共 caller 只表达“生产 verified candidate”或“评价真实 Codex 的开放目标结果”，不编排 source snapshot、prepare、conformance、seal、activate、begin、finalize 或 rollback。`EvaluationOperation` 只提供既有长 operation 的 observe/cancel/terminal disposition；批准决定通过既有 Gateway/UI authority 输入，不新增 `advance`、内部 stage selector 或 `next_action`。

### 9.1 ProductionCandidateModule

负责从明确的 source selection 生产、验证并原子 seal 一个 candidate。Module 自己拥有 trusted source snapshot producer，不信任 caller 提供的 path-only source root，也不把排除规则外包给 caller。snapshot receipt 必须绑定规范 path、byte length、content digest、tracked/untracked 和 producer-selected material ownership，并证明 Git-ignored/generated/external evidence 已由 producer 排除。`materialOwnership` 表示“由 snapshot producer 选入本次不可变 source closure”，不得把 engine source、fixture 或施工工具一律误标为 project material；更细的 source role 如需加入，仍由 producer 推导，不新增 caller 排除表。内部继续完成 release package、fixture/RuntimeModule compatibility、source contract regression、sealed release headless Gateway smoke、failure artifact、cleanup、candidate registry seal/reopen。只有所有 immutable 输入闭合、所有 writable execution 与 snapshot 隔离、最终 reopen 成功后才返回 sealed candidate；staging 内预分配的 candidateId 此时才生效。

request 可以携带目标平台、声明式 source selection、外部 work/output resource envelope 与总 deadline，但不允许 caller 指定内部 stage order、逐阶段 retry、文件排除菜谱、cargo 子命令或手工拼 manifest。内部 receipt 可以详细记录事实阶段，但不是 caller 或 AI 的操作计划。

#### 9.1.1 Immutable snapshot / writable conformance workspace 合同

`ProductionCandidateModule` 的 Implementation 必须显式区分以下 crate-private semantic roles：

```text
ImmutableSourceSnapshot
  canonical root + exact receipt + manifest digest + reopen authority
  只承担 candidate source identity，不提供 project write capability

DisposableConformanceWorkspace
  owned work root 下由 Module 物化的可写执行空间
  绑定 snapshot manifest、source-relative selection、initial file set/bytes/tree digest
  只承担 build/test/Editor 运行与证据收集，不进入 source identity
```

二者不是新的公共 Module、caller stage 或 request 字段。caller 不提供 copy mode、generated path allowlist、exclude list、fixture rewrite、stage order 或 cleanup recipe。materialization、delta classification、resource limit、cleanup 与 receipt 都属于 `ProductionCandidateModule` 的 Implementation。

必须满足：

1. snapshot root 与 workspace root 规范化后互斥、互不嵌套；workspace 内不得存在指回 snapshot/source 的 symlink、junction、reparse point 或 writable hard-link alias。允许使用的 copy-on-write primitive 必须能证明写时不会改变 snapshot bytes，否则使用独立复制。
2. 任何可能创建 `Library`、cache、journal、Build、Temp 或其它生成状态的 Editor/fixture action，只能获得 workspace 中的 project root。released Editor/MCP headless smoke 禁止原地打开 snapshot fixture。
3. release/source consumer 只有在全部 writable outputs 已重定向到 owned external root、输入写入被拒绝或阶段结束后 exact reopen 成立时，才可直接读取 snapshot；不能证明时必须改用 workspace。
4. workspace materialization receipt 至少绑定 snapshot manifest digest、normalized source-relative root、materialization kind/version、entry count、byte count、initial tree digest、canonical snapshot/workspace roots 与 disjointness proof。
5. conformance 期间的实际 file delta 必须由 producer 按引擎自己的 authored/generated-state 真相分类。允许的 generated state 可以留在 workspace 到证据封存；任何 authored material missing/extra/content drift 都 fail-closed。不得把 `Library/**` 或其它目录变成 caller-owned exclude recipe。
6. candidate/package identity 绑定 immutable snapshot、release inventory 与 conformance report，不绑定 disposable workspace 的 journal/cache/generated bytes。conformance evidence 可以绑定 bounded delta summary 与 workspace receipt digest。
7. seal 前必须重新验证原 source 未漂移、snapshot exact reopen、workspace evidence reopen 和 owned workspace cleanup；任一失败都产生没有生效 candidateId 的 durable failed disposition。

#### 9.1.2 Git source-material kind 合同

source selection 的 Git 真相不能只读取 path。producer 必须读取 index stage/mode/object id，并至少区分：

```text
100644 / 100755：regular tracked material。
160000：gitlink material。
120000：当前版本不支持的 symlink material，typed fail-fast。
其它 mode：typed fail-fast，不得退化为目录递归或普通文件读取。
```

`ProductionCandidateRequest` 与 `ProductionSourceSelection` 不新增 `submodulePaths`、`excludeGitlinks`、`fetchPolicy`、Git 命令、resolver 顺序或递归步骤。Git material 分类、解析、资源上限、copy、receipt、reopen 和 drift verification 全部属于 `ProductionCandidateModule` 的 Implementation。这样保持外部 Interface 的 depth 与 locality，不把 Git 菜谱转嫁给 caller 或 AI。

mode `160000` 采用用户确认的 **OID-authoritative、local-only、clean gitlink expansion**：

1. 父 index 的 gitlink path 与 expected commit OID 是声明身份。
2. path 必须是 selected workspace 内的 contained directory，不能是 symlink、junction 或 reparse point。
3. path 必须是精确的嵌套 Git repository root；嵌套 `HEAD` 必须等于 expected OID，且 expected OID 必须在本地 object database 中存在并为 commit。
4. 嵌套 repository 必须 clean：无 staged、tracked modified/deleted、untracked 或 conflict。当前合同不把 nested dirty/untracked 隐式吸收到顶层 `includeUntrackedProjectMaterial`。
5. producer 从 expected commit tree/object database 枚举 material，按 Git mode 解析并展开到 snapshot mount path；不递归扫描工作目录，不复制 `.git`，不执行 hook、external filter 或用户命令。
6. `.gitmodules` 有唯一合法同路径声明时记录 declaration digest；缺失但 exact local checkout 可证明时允许并记录 `declaration=absent`；声明损坏、重复或路径冲突时 fail-fast。
7. 禁止 `clone`、`fetch`、`submodule update`、LFS pull、credential helper 或任何网络恢复。material 未在本地闭合时 preparation failed，不创建 candidateId。
8. 递归 gitlink 只有能按同一合同闭合时才允许；material graph 必须无环，并受 depth/node/file/byte/deadline 上限约束。

当前合同不扩展为通用 Resolver Registry，不新增 LFS-like pointer hydration，也不允许 opaque-reference-only snapshot。opaque gitlink 只记录 OID 而不包含构建字节，不能满足 candidate source closure 的自包含要求；完整 LFS/远程 source resolver 属于未来另行讨论的能力。

#### 9.1.3 Gitlink receipt 与 reopen

source snapshot receipt writer 升级后，gitlink root 至少绑定：

```text
mount path
parent index mode = 160000
expected commit OID
resolved repository root identity
resolved HEAD OID
worktree state = clean
gitmodules declaration present/absent 与 declaration digest
resolution kind = local_exact_checkout
resolver/version
expanded tree digest、entry count 与 byte count
```

每个展开 entry 至少绑定 normalized path、Git mode、blob OID、byte length、SHA-256、material ownership、owning gitlink path 与 owning commit OID。manifest digest 覆盖 regular tracked/untracked material、gitlink root、全部展开 entries、声明状态与资源策略。snapshot 不包含 `.git`；reopen 只依赖 snapshot root 与 receipt，不访问原 workspace。seal 前再次验证 snapshot exact file set/bytes、父 gitlink OID、nested HEAD/clean state、commit tree 与 source 未漂移。

失败不得继续折叠为 `source_snapshot.material_invalid`，至少区分：

```text
source_snapshot.gitlink_checkout_missing
source_snapshot.gitlink_repository_invalid
source_snapshot.gitlink_head_mismatch
source_snapshot.gitlink_dirty
source_snapshot.gitlink_object_missing
source_snapshot.gitlink_declaration_conflict
source_snapshot.gitlink_tree_unsupported
source_snapshot.gitlink_path_collision
source_snapshot.gitlink_drift
```

`CandidateDisposition` 只有两类终态：成功时返回已 reopen 验证的 sealed candidate；失败时返回位于 staging 之外、可由共享 verifier reopen 的 durable failure artifact，且没有生效 candidateId。failure artifact 至少保留失败责任、bounded child exit reason/exit code、受限 stdout/stderr、kill/wait/read error、source/package lineage、snapshot/workspace role 与 cleanup 结果；不得只保留 child result digest，不得先删除唯一失败证据，也不得把 `stagingClean` 或 `childProcessesClean` 硬编码为成功。file-set/content mismatch 必须在 cleanup 前记录 bounded missing/extra/changed path、总计数、截断状态和对应 snapshot/workspace root role；通用 `file_set_mismatch` 只能作为顶层分类，不能丢失可定位差异。staging absence 通过精确路径复查；process tree cleanup 通过 bounded process primitive 的 ownership/bind/terminate/wait/release/reader-join receipt 证明。Windows 权威实现是 Job Object/handle，不使用运行结束后的裸 PID 枚举。cleanup digest 生成失败必须 fail-closed。

#### 9.1.4 内部 composition seam

公共 `produce(request)` Interface 不增加参数或步骤，但 Implementation 必须提供 crate-private 的 dependency/composition seam，使同一 orchestration 可以装配 production Adapter 与 deterministic test Adapter。这个 seam 至少覆盖 snapshot producer/reopen、workspace materializer、release producer、conformance executor、registry seal/reopen 和 cleanup evidence；只有一个 production Adapter 时不为其建立无变化价值的独立公共 port。

测试不得只用 closure 替换整个 `produce` body 后断言 facade 被调用。至少要通过同一个 `produce` orchestration 证明：会写入的 conformance Adapter 只能取得 workspace、写入后 snapshot 仍 exact reopen、unexpected authored drift fail-closed、workspace cleanup 可 reopen、成功 seal 与失败 disposition 均保持身份语义。released Editor/MCP 的真实 binary composition 另由第 10.4 节 exact preflight 证明，不把昂贵 subprocess 塞入所有快速测试。

### 9.2 CodexOutcomeEvaluationModule

负责使用一个 sealed candidate 评价真实 Codex 是否在 GoalBinding/RiskEnvelope 内完成开放目标。内部完成 target inventory、candidate reopen、activation compare-and-replace、lease、production process/session binding、goal-level Grant、attempt、实际 transcript/receipt/evidence 收集、outcome invariant validation、rollback 与 cleanup。真实 Codex 已经是 Gateway 的外部 client，不由引擎 shell/CLI 启动；production composition 必须使用内部 real-session observer Adapter，把同一已连接 Gateway session 的实际 operation、receipt、Grant、project generation 与 terminal cleanup 投影成 attempt evidence。deterministic executor Adapter 只用于 Module Interface 测试，不是 production wiring。每次实际 activation/attempt 仍创建新的 activationId/attemptId；candidate bytes 不变，证据不得跨 attempt 拼接。

用户批准仍只批准目标、项目、风险与预算，通过既有 Gateway/UI authority 输入；Module 自己等待并消费决定。调用者不执行 `activate -> begin -> finalize -> rollback`，不调用通用 `advance`，也不接收内部 next step。`evaluate` 每次只启动一个目标级 operation；返回的 `EvaluationOperation` 仅允许 observe/cancel，并最终产生 passed/rejected/inconclusive disposition 与 durable cleanup evidence。cancel 是请求 Module 收敛当前 operation，不是跳阶段；任何终态都必须完成或诚实记录 activation rollback、process/session release 和项目 final-state 检查。

### 9.3 内部责任 Module 与身份

原四类责任继续保留，但降为公共目标级 Module 的内部 seam：

```text
CandidatePreparationModule：snapshot、release、conformance、failure/cleanup。
CandidateRegistryModule：prepared identity、atomic seal、marker、reopen。
ActivationLeaseModule：config compare-and-replace、lease、process/session、rollback。
RealCodexAcceptanceModule：attempt binding、evidence/outcome validation、disposition。
```

`ReleasePackageModule` 和 `ProductionConformanceModule` 是 CandidatePreparation 的更内层依赖；`GoalGrantAuthorityModule` 是 Tool Kernel Core 的深化。内部 Module 可以有各自 receipt、failure injection 与测试 adapter，但不得公开为 caller 必须学习和排序的 lifecycle Interface。

candidateId、activationId、attemptId 是三类事实身份，不是三个 caller action。CandidateRegistry 可以在 staging 内预分配 candidateId，但只有 publish/reopen 成功后该身份才生效；ActivationLease 与 RealCodexAcceptance 只在 evaluate 内部实际发生对应事务时创建新 activationId/attemptId。它们的 lineage 与重试语义保持第 8、12、13 节不变。

## 10. Production Conformance

Conformance 分为三层，证据名称与完成声明必须反映真实执行路径：

### 10.1 Source contract regression

以 trusted source snapshot 为身份真相运行确定性测试。只读且写入被隔离的 consumer 可以直接读取 snapshot；任何可能写 source/project tree 的 consumer 必须在 `DisposableConformanceWorkspace` 中运行。每个 layer 结束后都必须重新验证 snapshot exact reopen，并逐项验证：

```text
Registry/Catalog/typed MCP schema 唯一来源与 canonical decode。
session/project/read generation/reconnect/drift。
Goal Grant authorization、budget、revoke、expiry 与 rollback authority。
每个 mutation 工具的 operation/receipt/digest/rollback。
Preview exact presented-frame ownership、PNG/metadata digest。
visual diagnosis evidence lineage。
Build artifact 与 Delivery verification lineage。
async start/observe/cancel/crash/restart 的真实生命周期。
negative cases、tamper、cross-session/cross-project/cross-attempt rejection。
```

### 10.2 Sealed release headless smoke

只从 release package inventory 启动正式 Editor/MCP binary。fixture 的 immutable identity 来自 snapshot，但启动前必须由 Module 物化为绑定该 identity 的 writable conformance project；Editor 禁止打开 snapshot 内的 fixture root。验证 workspace materialization receipt、精确 project root、discovery、session/project binding、typed MCP `status/catalog/inspect/search`、正式二进制 digest、workspace delta、snapshot unchanged、workspace cleanup、退出码和 bounded process ownership。`--gateway-process-preflight` 属于 headless smoke，不能命名为 actual production Editor conformance，也不证明 Native window、mutation、Preview、visual diagnosis 或 Build/Delivery 已在发布二进制路径执行。

### 10.3 Real Editor / Codex outcome acceptance

真实 Native Editor window、真实 Gateway client/session、用户批准和目标级 side effect/evidence 只在第 11 节的 Real Codex Outcome Acceptance 中证明。这里按 AI 实际使用的能力验证 mutation、Preview、visual、Build/Delivery 与 rollback lineage，不要求固定全集。

前两层可以使用确定性测试客户端或 fixture，但 artifact 必须标明 producer kind 和 layer，不能声称真实 Codex outcome 已通过。source contract 或 release headless smoke 未通过时不得 seal candidate；真实 acceptance 不重复承担 source 单元回归职责。

### 10.4 Exact production composition coverage

Production Candidate 的进入顺序固定为验证纪律，而不是 caller 菜谱：

```text
快速组合红/绿测试：
  通过 produce orchestration 的内部 test Adapter 写 workspace，证明 snapshot/workspace capability 隔离。

受影响域回归：
  覆盖 source snapshot、workspace materialization、release、conformance、failure、cleanup 与 owner Adapter。

exact production-composition preflight：
  从 fresh exact source 构建 release inventory；真实 released Editor/MCP 打开 writable workspace；
  证明 generated state 只出现在 workspace、snapshot exact reopen、workspace cleanup 与 evidence reopen。

最终权威 workspace regression：
  只在上述 exact preflight 全绿后执行一次，证明共享域没有回归。
```

name filter、test count、default/all-features workspace pass、只测 snapshot extra-file rejection、只测 Editor open success，均不能单独声明 exact composition 已覆盖。被 `#[ignore]`、local-only 或 construction-only 跳过的测试必须在 coverage inventory 中显式列出；未按合同执行就只能记录 skipped，不能进入 passed summary。最终 Fresh Candidate production operation 仍只按单独用户授权执行，不能用它调试内部 composition。

## 11. Real Codex Outcome Acceptance

### 11.1 输入

真实验收输入是用户可理解的开放 goal、completion policy 和 risk envelope，不是工具调用列表。例如可以要求“修复指定可见问题并保留修复”，或“产生可交付构建并验证”。目标应足以判断结果，但不能偷偷编码实现步骤。

### 11.2 AI 行为

真实 Codex 使用当前 typed MCP discovery 和工具结果自主完成任务。它可以做任意数量的合法 read，在 Grant 范围内进行多次 mutation，选择或跳过 Preview、视觉诊断、Build、Delivery，并根据失败诊断重新规划。任何具体能力一旦被实际使用，其 receipt/evidence 必须满足该能力的 conformance contract。

### 11.3 finalize 不变量

```text
BindingInvariant：client/candidate/activation/project/attempt/goal 精确一致。
OutcomeInvariant：userVisibleOutcome 满足 completionPolicy。
AuthorizationInvariant：全部 side effects 均在有效 Grant 与预算内。
EvidenceInvariant：实际使用的 evidence ownership、lineage、digest 有效。
CausalityInvariant：实际依赖关系成立，不验证无关工具总顺序。
FinalStateInvariant：最终 digest、artifact 或 rollback/cleanup 满足 completionPolicy。
IsolationInvariant：不跨 attempt 拼接，不污染 project root 外状态。
```

### 11.4 条件证据

只有实际目标或 AI 路径使用能力时才要求对应证据：

| 实际行为 | 必需证据 |
|---|---|
| mutation | Grant、operation、mutation receipt、前后 digest |
| Preview | exact presented-frame PNG、metadata、frame/operation ownership |
| visual diagnosis | issue/locate/explain/owner evidence 的实际 lineage |
| Build | build operation 与 artifact digest |
| Delivery | 引用真实 Build artifact 的 delivery receipt |
| rollback | 引用真实 mutation receipt 的恢复 digest |

不存在“每个 goal 都必须走完所有行”的要求。

## 12. disposition 与重试合同

### 12.1 Preparation / conformance

seal 前可在清理完成后重复。失败不创建 candidateId。相同输入重做是 preparation retry，不是 candidate retry。

### 12.2 Candidate

sealed candidate 的确定性 acceptance failure 为 terminal rejected：包括 tool-contract、safety、goal、evidence、identity、final-state 或 cleanup failure。不得修改或重试该 candidate 来掩盖失败；修复产品后重新 prepare/seal 新 candidate。

attempt inconclusive 不否定 candidate，因为它没有证明产品错误。

### 12.3 Activation

activation 失败必须按本次 receipt rollback；可以创建新 activationId 再试。旧 activation 的 config、session 或 receipt 不得复用为新 activation 的身份。

### 12.4 Attempt

```text
passed：目标和全部不变量成立。
inconclusive：user decline、approval timeout、外部中断，且 cleanup/rollback 已完成。
rejected：tool-contract、安全、目标、证据、身份、最终状态或 cleanup 失败。
```

inconclusive 后可以在仍有效的 sealed candidate 上创建新 activationId（如需要）与新 attemptId。证据从零开始，不得跨 attempt 合并。cleanup 失败本身是 rejected，不得归入 inconclusive。

## 13. artifact 与 verifier

### 13.1 Candidate artifact

只包含 immutable package、tool-contract/catalog、fixture compatibility、production conformance summary 和 producer lineage。不包含 activation config、future goal、Grant、attempt plan 或 transcript。

### 13.2 Activation receipt

包含 candidateId、activationId、target inventory、config compare-and-replace、before/after digest、lease/session/process ownership、rollback material 和 cleanup status。

### 13.3 Acceptance artifact

包含 goal/Grant、attempt binding、实际事件集合、实际 receipts/evidence、最终状态和 disposition。事件按事实记录；schema 不含 required action allowlist、exact count 或固定 total order。

### 13.4 Shared verifier

共享 verifier 验证 canonical encoding、digest、identity reference、ownership、causal edges、budget accounting、final state 和 tamper rejection。它不能把未发生且与目标无关的工具调用当作缺失证据，也不能因额外合法 read 或合法重规划拒绝 attempt。

### 13.5 Preparation failure artifact

seal 前失败 artifact 与 candidate artifact 分离，保存于 staging 生命周期之外，并绑定 preparation operation、source snapshot、conformance workspace receipt、package/conformance 子结果和 cleanup measurement。共享 verifier 必须能在 staging 已清理后 reopen 并验证该 artifact；artifact 中的 bounded stdout/stderr 必须有明确截断元数据，process exit、timeout、kill、wait、ownership release 和 reader failure 不得折叠成单一通用 diagnostic。snapshot/workspace file-set 或 content mismatch 必须保存 bounded missing/extra/changed path、总数、截断状态和 root role，不能只保存 `source_snapshot.file_set_mismatch`。裸 PID 只能作为诊断字段，不是 cleanup authority。

## 14. 迁移方案

### 14.1 保留

```text
typed MCP projection 与 canonical direct-input decode。
session access/read generation/approval lifecycle/reconnect。
exact-frame Preview、visual diagnosis、Build/Delivery evidence contract。
production process ownership、deadline、atomic publish、rollback/cleanup 技术。
R1/R5/R6 测试中可复用的底层 verifier 与 failure injection。
```

### 14.2 替代或删除默认控制权

```text
GateFAcceptancePlan 固定 action allowlist/count/order。
Candidate freezer 的 15-stage caller-visible orchestration。
candidate manifest 对 plan/config/step 的身份绑定。
一次 approval/一次 mutation 的默认预算。
把 approval timeout 归类为 candidate failure。
legacy ProjectIntentWorkflow/candidate_plan_steps/AdvanceRun 作为默认 AI 路径。
```

这些类型若仍被历史 artifact parser 需要，可以保留只读兼容解析，但不得出现在 R2 当前执行路径。

### 14.3 施工拆分建议

后续施工文档应按可独立验证的责任拆分，而不是重建一次性 Gate 菜谱：

```text
R2-A：goal-level Grant 与多 mutation budget。
R2-B：candidate/activation/attempt schema、无环 identity 与 shared verifier。
R2-C：两个公共目标级 Module、四类内部责任及 production conformance 收敛。
R2-D：outcome acceptance/disposition/artifact validator。
R2-E：迁移、负向测试、default/all-features 与真实客户端授权前审计。
```

该拆分只是方案对施工文档的责任建议，不是当前施工命令；正式施工文档仍需单独生成、审查和激活。

## 15. 风险与控制

### 15.1 Goal 太模糊

控制：`userVisibleOutcome` 与 `completionPolicy` 必须可验证；不能以模糊目标换取任意 side effect。若结果不可判定，begin 前拒绝并返回需要补充的目标事实，不生成 attempt。

### 15.2 Grant 太宽

控制：风险包络有明确 project、scope、budget 和高风险 flags；默认仍是 project-owned low risk。bounded 不等于 unlimited，扩权必须重新批准。

### 15.3 Outcome validator 偷偷恢复菜谱

控制：schema 与测试明确禁止 required tool list/count/order；用两条不同合法计划完成同一 goal 的等价测试，额外合法 read 和动态重规划必须通过。

### 15.4 Conformance 与真实验收重复

控制：source contract regression 证明实现合同，sealed release headless smoke 证明正式二进制最小路径，Real Codex Acceptance 证明真实客户端完成开放目标。第三层不重复逐工具全集；只有实际使用能力才消费其证据。任何 layer 的名称和 artifact producer kind 都不得越级声明。

### 15.5 生命周期仍然复杂

控制：公共 Interface 只保留 ProductionCandidate 与 CodexOutcomeEvaluation 两个目标级深 Module；四类责任放在内部 Module，不暴露 production stages。package/candidate/activation/attempt 事实继续分离，其中 candidateId、activationId、attemptId 三类事务身份不得混淆；身份分离不再被误写为 caller 必须执行的步骤。

### 15.6 Source material 类型被 path-only inventory 抹平

控制：producer 从 Git index mode/object id 建立 material closure，不把 `git ls-files` 的 path 集合假定为 regular files。regular blob、gitlink、symlink/unsupported mode 有不同 typed contract。gitlink 只在 exact local clean material 可证明时展开；缺失 material 不联网恢复、不作为 opaque reference 越过 seal。该规则留在 Module 内部，不新增 caller policy。

### 15.7 Immutable snapshot 被当作 writable project

控制：snapshot 与 conformance workspace 使用不同的 crate-private capability/value role；released Editor、project workflow 或任何可能生成 journal/cache/Library/Build/Temp 的 action 只能获得 workspace project root。workspace materialization receipt 绑定 snapshot identity，workspace delta 不进入 candidate source identity；seal 前重新验证 snapshot exact reopen、原 source unchanged、workspace evidence 与 cleanup。禁止通过弱化 exact reopen、把 `Library/**` 加进 caller exclude list或忽略额外文件来绕过失败。

### 15.8 验证设施遮蔽真实 composition

控制：先有穿过 `produce` orchestration 的快速组合测试，再有真实 released Editor/MCP exact composition preflight，最后才运行一次昂贵 workspace authoritative。PowerShell 或其它 construction Runner 不再承担产品 ownership 真相；name-filtered/default/all-features 通过不能替代 skipped/ignored exact-composition evidence。每个 stage 保存独立 command、exit、duration 与 bounded log digest。

## 16. 验证门槛

后续施工必须至少证明：

```text
同一开放 goal 可由两种不同合法工具计划通过。
额外合法 read、不同 read 顺序和 evidence-driven replan 不影响通过。
未使用 Preview/Build/Delivery 的目标不会因缺少这些证据失败。
使用某能力时，tampered/cross-project/cross-attempt evidence 必须失败。
同一 Grant 支持预算内多 mutation；越界 mutation 请求新批准。
approval timeout -> inconclusive；cleanup failure -> rejected。
seal 前预分配 candidateId 不生效、不可观察；每次 activation/attempt 都有新 ID。
activation rollback 不改变 candidate bytes；attempt evidence 不可拼接。
bounded process receipt 证明 ownership/bind/terminate/wait/release/reader join；裸 PID 不作为 cleanup authority。
cleanup digest 生成失败 fail-closed，不生成全零占位 digest。
headless gateway smoke 不冒充 Native Editor window 或真实 Codex outcome。
production `CodexOutcomeEvaluationModule` 只能由 Gateway real-session observer Adapter 组成；test executor 不算 production wiring。
legacy fixed-plan path 不再是 typed MCP 默认入口。
mode `160000` 不进入 regular-file copy loop；exact local clean gitlink 可展开并形成 self-contained receipt。
gitlink checkout missing/head mismatch/dirty/object missing/declaration conflict/path collision/drift 均 typed fail-fast。
gitlink expansion 不复制 `.git`、不联网、不运行 hook/filter/credential helper，receipt reopen 不依赖原仓库。
snapshot 与 workspace canonical root 互斥、无 writable alias；Editor 只能打开 workspace project root。
workspace receipt 绑定 snapshot manifest、source-relative root、initial file set/bytes/tree digest 与 materialization kind。
conformance generated state 只进入 bounded workspace delta evidence；authored material drift fail-closed。
快速测试通过同一个 produce orchestration 证明 conformance write 不改变 snapshot，并证明 workspace cleanup。
released Editor/MCP exact composition preflight 证明 journal/cache 只写入 workspace，随后 snapshot exact reopen。
coverage inventory 显式列出 skipped/ignored test；未执行的 exact composition 不进入 passed summary。
file-set/content mismatch failure artifact 保存 bounded missing/extra/changed path、总数、截断状态和 root role。
```

该历史执行链已经终止。不得再以完成施工、自动化 conformance、生产 inventory 或取得单次授权为由恢复真实 Codex acceptance；如未来需要发布质量体系，必须在 254 之外重新讨论独立方案。

## 17. 历史方案自审

> 本节记录 R2 仍作为正式方案时的自审结论。第 17.1-17.11 节不覆盖第 0 节的退役决定。

### 17.1 是否过度复杂

通过。身份模型表面增加了明确名词，但它们对应原本已经存在却混在一起的发布、配置和运行事实。公共 lifecycle 从 R1 的多阶段 freezer/plan validator 收敛为 ProductionCandidate 与 CodexOutcomeEvaluation 两个目标级深 Module；prepare/seal/activate/begin/finalize/rollback 均为内部责任，调用者不再承担生命周期编排。

### 17.2 是否限制 AI 能力

通过。删除固定工具集合、次数、exact input、总顺序和 `next_action`；允许多 mutation、合法额外读取和动态重规划。限制只作用于项目、风险、预算和证据真实性，不规定实现方法。

### 17.3 是否满足“工具内可有步骤，工具外不能要求 AI 有菜谱”

通过。Production stages、事务、deadline 和 cleanup 归各 Module 内部；AI 只组合能力。跨工具 validator 只检查实际因果关系和结果不变量，不检查预设 trace。

### 17.4 是否保留安全与可审计性

通过。goal-level Grant、side-effect receipts、digest、ownership、budget、final-state 和 cleanup 仍 fail-closed；rollback authority 不因 forward Grant 到期而丢失。

### 17.5 是否能定位失败

旧结论已被 R2-FC6 推翻：当时 failure artifact 只给出通用 `source_snapshot.file_set_mismatch`，owned staging cleanup 后不能直接读取 exact extra path，导致定位成本过高。修订后通过：Preparation、seal、activation 和 attempt 继续有独立身份与 disposition；snapshot/workspace role、bounded missing/extra/changed paths、总数和截断状态在 cleanup 前进入 durable failure artifact，conformance 仍先隔离工具合同错误，真实 acceptance 不承担生产包单元回归职责。

### 17.6 是否会重现循环绑定

通过。candidate 只包含 immutable preparation/conformance 事实；activation config 和 attempt plan/transcript 均在 candidate 之后产生，不进入 candidate identity。

### 17.7 是否诚实处理 R1 历史

通过。R1 不改写为成功，所有历史 candidate 和失败 artifact 保留原终态；只撤销其当前规范和施工效力。

### 17.8 是否已经授权施工

历史 remediation、Gateway real-session observer Adapter 与 Native Editor production composition 授权均已消费。R2-O0 至 R2-O7、clean exact-commit preflight 60/60 与 default/all-features authoritative matrix 已通过并归档；Fresh Candidate 第一段唯一授权已由 R2-FC6 消耗并以 `source_snapshot.file_set_mismatch` terminal failed，没有 candidateId 且 cleanup 完整。当前没有任何 R2 lifecycle 代码施工或重跑授权；退役施工文档已编写并自审，唯一后续动作是在用户单独授权后做激活前复核，把活动实现稳妥移出 Core。R2-N6/N7/N7V/N7W/N8、R2-G、FC6/FC7、Real Evaluation、activation 与真实 Codex attempt 均不得复活或复用。

### 17.9 Gitlink 合同是否过度复杂或限制 AI

通过。外部仍只有 `ProductionCandidateModule::produce(request)`，caller 不提供 submodule 步骤、排除表或 fetch policy。新增复杂性只对应 Git 已存在的 mode/OID/material closure 事实，并集中在 source snapshot Implementation。当前只实现 local exact clean expansion，不建立通用 Resolver Registry、LFS 或网络恢复；它约束 production freeze 的可证明性，不限制 AI 的跨工具规划与项目创作能力。

### 17.10 两段目标级授权是否重新引入 caller 菜谱

不引入。第一段只授权“生产一个可 reopen 验证的 fresh sealed candidate”，第二段只授权“评价真实 Codex 是否完成开放目标”；它们对应 immutable candidate production 与真实环境 side effect 两个不同风险域。第一段内部的 snapshot、release、conformance、seal、reopen 和 cleanup 仍由 `ProductionCandidateModule` 自主完成，caller 只提交声明式 request。任何薄 CLI/host Adapter 都只能严格解码 request、调用 Module 一次并原子写出 disposition，不得公开内部 stage selector、retry、排除表或 next action。

### 17.11 两种内部 capability role 是否增加公共复杂度或限制 AI

不增加。`ImmutableSourceSnapshot` 与 `DisposableConformanceWorkspace` 只编码 Module Implementation 已经存在但此前被普通 `Path` 抹平的 mutability/ownership 事实；它们不进入 public request，不新增 caller step，也不规定 AI 的跨工具计划。复杂性从外部隐式约束和昂贵失败中收回 Module 内部，提升 locality 与可测试性。

## 18. 历史结论与当前处置

历史结论曾采用 Production Conformance 与 Real Codex Outcome Acceptance 分层，并把 `ProductionCandidateModule::produce` 与 `CodexOutcomeEvaluationModule::evaluate` 设计成两个目标级深 Module。该设计解决的是精确引擎候选、激活和真实 AI 结果验收，不增加 AI 对用户项目的工具能力，因此已被 254 当前范围纠正整体替代。

R1 与 R2 的方案、candidate、attempt、failure artifact、错误归档和完成记录全部保持原终态，继续解释为什么固定菜谱、精确版本验收和 lifecycle 复杂度不应进入 AI 工具 Core。它们不再提供公共 Interface、施工入口或重试边界。

当前状态：`superseded_by_254_scope_correction / R2-FC6 terminal failed retained / lifecycle retirement R2-LR7 in progress`。当前施工只解除 active consumer、移除公共导出和 production composition，并按第 0.2 节把无 active consumer 的 lifecycle 源码归档到 `legacy/rust/254-r2-lifecycle/`。不得恢复 candidate/activation/attempt 或真实 AI acceptance。历史失败索引见 `254-R2-历史失败归档-2026-07-22.md`。
