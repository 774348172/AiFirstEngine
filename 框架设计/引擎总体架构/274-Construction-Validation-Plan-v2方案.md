# 274-Construction Validation Plan v2 方案

## 1. 状态与结论

讨论结论：用户确认方案 C。

方案状态：正式方案已确认；v2-A Gate A-E 已于 2026-08-06 完成并归档。

施工状态：v2-A 已完成；v2-B、v2-C、v2-D 未授权、未施工。

方案性质：用户明确指定的工程验证治理插入，不重开 240 中已经关闭的 CQ / INC 项，
不改变 240 的既有优先级与完成状态。

正式选择：在现有 `quality_gate` crate 内新增深
`ConstructionValidationModule`，与 `QualityGateRunner`、`LocalCiRunner` 并列；
默认命令行保持简单，高成本或涉及外部状态的执行必须先取得可审查计划，再携带显式授权执行。

外部 interface 固定为：

```rust
prepare(request) -> PrepareReport
execute(plan_ref, authorization) -> ExecutionReport
```

第一施工范围只允许实现 v2-A：`prepare` 计划生成与静态验证。v2-A 不执行命令、
不启动进程、不写外部配置、不创建 Local CI worktree，也不以任何形式隐式进入 `execute`。

## 2. 三个不可变限制

以下限制由用户确认，后续施工文档、实现和迁移不得弱化：

1. 复用现有 `quality_gate`。不得创建 `quality_gate_v2`、第二套质量 Runner、平行报告真相或绕过
   `QualityGateRunner` / `LocalCiRunner` 的新验证栈。
2. 不创建通用 workflow engine。不得引入任意 DAG、通用任务语言、插件式流程编排器、
   YAML workflow 解释器或面向产品功能的工作流平台。
3. 第一实施范围只做 plan-only。v2-A 只建立 catalog、affected closure、去重、成本估算、
   evidence reuse 判定和结构化 `PrepareReport`；不得执行计划。

如果未来需求与任一限制冲突，必须重新回到正式方案讨论并取得用户确认，不能在施工中自行扩大。

## 3. 这个 Module 解决什么问题

当前施工验证规则已经具备正确原则，但验证选择仍主要由施工文档和执行者手工展开：

- owner test、consumer test、affected regression、freeze、release / activation evidence 容易被写成
  多轮固定菜谱，而不是同一 proof obligation 的最小覆盖；
- filtered test、crate suite、workspace suite 与 Local CI 之间缺少机器可读的 `subsumes` 关系，
  同一配置下可能重复执行同一证据；
- source、test harness、fixture、binary、composition、external configuration 的身份没有统一进入
  计划判定，导致无关变化也可能触发全量重跑；
- 高成本 Gate 的 timeout、fail-fast、证据落盘和 cleanup reserve 依赖文档逐次描述，难以稳定复用；
- 最近真实 Editor / Tower 验证暴露的失败通常需要先收敛 owner-level reproduction，再重跑受影响
  consumer；现有规则能要求这样做，但缺少一个可审查的计划产物来阻止先跑大矩阵。

因此，问题不是“测试太多”，而是缺少一个把 claim、影响闭包、suite 覆盖、证据身份和成本统一
收敛为执行前计划的深 Module。

## 4. 目标

1. 把施工验证从手工测试清单提升为 claim-driven、affected-driven 的结构化计划。
2. 用一个小 interface 隐藏 owner / consumer closure、suite 去重、证据复用、成本和 fail-fast 排序。
3. 保持已有验证 producer 为真相；本 Module 只选择、解释和编排，不重写各测试系统。
4. 让 AI、施工文档和人工审查者在执行高成本操作前看到同一份 machine-readable plan。
5. 让失败停在最接近 owner 的红色证据，并只更新被修复所失效的资格证据。
6. 让外部状态、production composition、真实 Editor、安装态二进制和 Local CI 始终受显式授权控制。

## 5. 非目标

- 不修改 Runtime、Editor、AUI、Projection、项目玩法或 AI Tool public interface。
- 不取代 Cargo、测试 harness、`QualityGateRunner`、`LocalCiRunner` 或真实 Gate producer。
- 不自动推断业务需求是否实现正确；claim 仍必须由正式方案 / 施工文档声明。
- 不把 owner、consumer、closure 固化为三轮物理测试。
- 不把所有历史失败永久追加成公共验证步骤。
- 不为普通 development test 强制 clean exact commit、fresh root 或 production binary seal。
- 不在 v2-A 执行任何命令，也不提供隐藏执行开关。

## 6. 当前代码基线与复用点

当前 `rust/crates/quality_gate` 已经提供：

```text
QualityGateRunner::verify(QualityGateRequest) -> QualityGateReport
LocalCiRunner::run(LocalCiRequest) -> LocalCiRunReport
QualityCommandExecutor
SystemQualityCommandExecutor
ScriptedQualityCommandExecutor
ChangeScopeRequest / ChangeScope
```

`QualityGateRunner` 已隐藏固定工具链、lint ledger、suppression、architecture evidence、
stage report、fail-fast 与 timeout；`LocalCiRunner` 已隐藏 exact-commit worktree、隔离执行、
report 和 cleanup。`change_scope` 已提供 schema-first 的 source change classification。

274 不复制这些 implementation。它在更上层回答“为当前 claim 应选择哪些既有 verifier、
哪些证据仍有效、按什么顺序运行”，最终执行仍委托给 catalog 指向的现有 producer / adapter。

## 7. 成熟工具参考

### 7.1 Nx affected

Nx 通过 base / head 变化与 project graph 计算 affected projects，再只运行受影响任务。

可学习：先求影响闭包，再选择任务；影响范围是计划输入，不是测试命令中的隐式约定。

不照搬：本项目的 verifier 不只有 project task，还包括真实 Editor、binary、配置事务和人工授权，
不能退化为 JavaScript monorepo task graph。

### 7.2 Gradle Build Cache

Gradle 以 task inputs、实现和环境决定输出是否可复用。

可学习：证据复用必须绑定 consumed identities，不能只以测试名或最后通过时间判断。

不照搬：验证 evidence 不是普通 build output；ReleaseActivation 还需要授权、composition 与外部状态
资格，cache hit 不能自动等价为真实激活通过。

### 7.3 Bazel iteration guidance

Bazel 强调缩小 targets、复用未变化工作并优化开发迭代路径。

可学习：owner red-capable verifier 应先于昂贵 closure；高成本矩阵不应充当调试循环。

不照搬：274 不引入 Bazel action graph 或重建系统。

### 7.4 Cargo test

Cargo 已提供 package、target、feature、filter 等真实执行选择器。

可学习：catalog 应引用现有 Cargo command producer，并显式记录 profile、features、platform 和 filter。

不照搬：不能仅凭 Cargo filter 推断 suite 的语义覆盖，`subsumes` 必须由 repository-owned catalog
明确声明并由测试验证。

## 8. 方案比较与冻结决定

### 8.1 方案 A：单一 `run(mode)`

表面调用最少，但 planning、授权和 execution 混在一次调用内。审查者无法在副作用发生前冻结计划，
也难以证明 v2-A 绝不会执行。

结论：不采用。

### 8.2 方案 B：完全 registry-driven prepare / execute

扩展性强，但第一版必须先建设完整 registry、executor、历史数据库和 adapter 生态，前置成本过高，
容易滑向通用 workflow engine。

结论：不采用。

### 8.3 方案 C：深 Module + 简单默认 CLI

`ConstructionValidationModule` 用小 interface 生成计划；普通低成本路径可由简单 CLI 使用默认策略，
高成本路径必须显式 `prepare` 后再 `execute(plan_ref, authorization)`。内部 catalog 保持项目内、
typed、有限表达力。

结论：正式采用。它同时满足审查性、深度、渐进实施和三个不可变限制。

## 9. Module、Seam 与 Adapter

### 9.1 外部 seam

外部 seam 位于 `quality_gate` crate：

```rust
pub struct ConstructionValidationModule<C, E> {
    catalog: C,
    evidence_store: E,
    // private implementation
}

impl<C, E> ConstructionValidationModule<C, E> {
    pub fn prepare(&self, request: PrepareRequest) -> PrepareReport;
    pub fn execute(
        &self,
        plan_ref: PlanRef,
        authorization: ExecutionAuthorization,
    ) -> ExecutionReport;
}
```

这是概念 interface，具体 Rust 类型可在施工文档中收敛，但不得增加 caller 必须手工完成的
closure、去重、缓存或排序步骤。

### 9.2 深 implementation 隐藏内容

- owner、direct consumer、transitive affected closure；
- claim 到 proof obligation，再到 verifier 的映射；
- suite `subsumes`、same-environment duplicate elimination；
- evidence identity、reuse、partial invalidation；
- cost class、historical duration、timeout 与 cleanup reserve；
- fail-fast stage ordering 和 failure cutoff；
- run-owned process / state ownership requirements；
- plan digest、authorization scope 和 structured diagnostics。

删除该 Module 后，上述复杂度会重新散落到每份施工文档和每个调用者，因此它通过 deletion test。

### 9.3 内部 adapters

只有存在真实变化点时才建立 adapter：

```text
ValidationCatalogSource       committed catalog / in-memory test catalog
EvidenceStore                 filesystem evidence index / in-memory fake
ValidationExecutor            existing quality gate and command adapters
Clock                         system clock / deterministic test clock
```

v2-A 只需要前两项及确定性 clock；`ValidationExecutor` 在 v2-C 前不得进入 production wiring。

## 10. Claim 模型

```rust
pub enum ValidationClaim {
    Development,
    Integration,
    Freeze,
    ReleaseActivation,
}
```

### Development

证明 owner 行为可红、修复后转绿，并覆盖共享 interface 的直接受影响 consumer。允许 dirty-worktree
身份；结果是开发反馈，不是 release evidence。

### Integration

证明跨 crate、进程、fixture、generated contract 或平台 seam 的 affected closure。只有 claim 明确绑定
commit 时才要求 clean exact commit。

### Freeze

证明候选 source / harness / artifact identities 被冻结且所需矩阵完成。Freeze 不是自动 activation，
也不隐含 production replacement。

### ReleaseActivation

证明实际 production composition、installed binary、真实 Editor / OS / GPU、真实配置或声明矩阵。
凡涉及外部状态或一次性尝试都需要显式用户授权。

claim level 是 proof strength，不是把低等级全部再跑一遍。高等级计划可以通过 `subsumes` 或有效
evidence 复用满足较低 proof obligation。

## 11. PrepareRequest

建议 schema：

```json
{
  "schemaVersion": "construction-validation.prepare-request.v2",
  "requestId": "...",
  "claim": "Integration",
  "change": {
    "baseCommit": "optional exact sha",
    "headCommit": "optional exact sha",
    "dirtyPatchDigest": "optional sha256",
    "changedSubjects": []
  },
  "declaredOwners": [],
  "declaredConsumers": [],
  "requiredCapabilities": [],
  "environment": {
    "platform": "windows-x86_64",
    "profile": "...",
    "features": [],
    "composition": "source|isolated|production|installed"
  },
  "authorizationCeiling": {
    "localCi": false,
    "realEditor": false,
    "productionReplacement": false,
    "realConfigurationMutation": false
  },
  "timeBudgetSeconds": 10800
}
```

`authorizationCeiling` 只是规划上限，不是 execution authorization。它允许 planner 排除当前明确
禁止的 verifier，但不能授权任何副作用。

## 12. Validation Catalog

Catalog 是 repository-owned typed data，不是通用 workflow DSL。每个 entry 只能描述一个既有 verifier：

```json
{
  "schemaVersion": "construction-validation.catalog.v1",
  "verifiers": [{
    "id": "quality_gate.engine_strict",
    "ownerDomains": ["quality_gate"],
    "consumerDomains": ["rust_workspace"],
    "proves": ["integration.rust_workspace"],
    "commandProducer": "quality_gate.verify",
    "profile": "engine-strict",
    "features": "declared",
    "platform": "windows-x86_64",
    "subsumes": [],
    "costClass": "high",
    "defaultTimeoutSeconds": 5400,
    "externalEffects": ["owned_processes", "owned_artifact_root"],
    "authorization": []
  }]
}
```

有限表达力要求：

- `commandProducer` 只能引用代码内已注册的 producer id，不能保存任意 shell；
- `subsumes` 只在相同 inputs、platform、profile、features、composition 下生效；
- catalog 不表达条件循环、变量赋值、动态脚本、任意依赖 DAG 或 retry program；
- catalog 变更本身属于 test / verification harness identity，必须经过 owner test 和审查。

## 13. PrepareReport 与 PlanRef

```json
{
  "schemaVersion": "construction-validation.prepare-report.v2",
  "requestId": "...",
  "status": "ready|ineligible|needs_authorization|invalid",
  "planRef": {
    "planId": "...",
    "planDigest": "sha256:...",
    "catalogDigest": "sha256:..."
  },
  "claim": "Integration",
  "affectedClosure": [],
  "proofObligations": [],
  "stages": [],
  "reusedEvidence": [],
  "eliminatedDuplicates": [],
  "estimatedDurationSeconds": 0,
  "cleanupReserveSeconds": 0,
  "requiredAuthorization": [],
  "omissions": [],
  "diagnostics": []
}
```

每个 stage 至少记录 verifier id、proof obligations、consumed identities、environment identity、
cost、timeout、fail-fast predecessor、external effects 和 evidence output。`planDigest` 覆盖 canonical
request、catalog、selected verifier、ordering、identities 和 authorization requirements。

`PlanRef` 是 immutable content reference；catalog、request、identity 或授权需求变化后旧 plan 必须
fail closed，不能静默重算后继续执行。

## 14. Evidence Identity 与失效

至少分开跟踪：

```text
ProductSourceIdentity
VerificationHarnessIdentity
GeneratedContractFixtureIdentity
ProductionBinaryIdentity
LaunchCompositionIdentity
ExternalConfigurationIdentity
PlatformToolchainIdentity
```

Evidence record 包含 producer id、consumed identity digests、environment、claim strength、结果、
时间、report digest 和 cleanup / rollback outcome。

复用规则：

1. verifier 声明的全部 consumed identities 必须完全匹配；
2. platform、profile、features、composition 必须满足同一性；
3. 原 evidence 必须成功完成并保有可验证 report；
4. ReleaseActivation evidence 的授权和外部状态 lineage 不可迁移给另一安装态或配置；
5. test-only 变化只失效消费 harness identity 的 evidence，不自动失效未消费它的 binary receipt；
6. source pass 不得替代不同 binary / composition 的 production evidence。

## 15. Planning Algorithm

`prepare` 必须确定性执行以下逻辑：

1. 校验 request、claim、时间预算和 authorization ceiling。
2. 解析 source change scope，结合 catalog owner / consumer edge 求 affected closure。
3. 将 claim 展开为 proof obligations，而不是固定 suite 名单。
4. 为每个 obligation 选择能够证明它的最小 verifier 集合。
5. 按同一 environment identity 应用 `subsumes`，删除重复 stage 并记录理由。
6. 查询 evidence store；只复用全部 consumed identities 匹配的 evidence。
7. 对剩余 stage 使用 cost class、历史 p95 与 default timeout 估算时间。
8. 将 red-capable owner verifier、廉价 contract verifier排在昂贵 integration / real-world stage 前。
9. 为有副作用 stage 加入 evidence-write 与 cleanup reserve，并声明 exclusive state ownership。
10. 若总预算、授权上限或资格不满足，返回 `ineligible` / `needs_authorization`，不得删减必需 proof
    obligation 后伪装为 ready。
11. canonicalize plan，生成 digest 与 `PlanRef`，只写 run-owned plan artifact。

## 16. Execution Algorithm

本节定义未来 v2-C / v2-D 合同，不属于 v2-A 施工范围。

1. 读取 `PlanRef` 指向的 immutable plan 并验证 digest、catalog digest 和当前 identities。
2. 将每个 required authorization 与 `ExecutionAuthorization` 精确匹配；范围不足则 fail closed。
3. 为每个有副作用 stage 建立唯一 run root、process ownership 和 cleanup contract。
4. 按 plan 顺序委托既有 producer；不重新实现 `QualityGateRunner` 或 `LocalCiRunner`。
5. 首个使 claim 不再可能成立的 upstream failure 后停止昂贵后续 stage。
6. 只有计划明确标记 distinct evidence collection 时才允许失败后继续。
7. 每阶段先保留 actionable failure evidence，再 cleanup owned state。
8. 输出 observed、reused、skipped、failed、not-authorized 和 cleanup 状态。
9. 修复后必须重新 `prepare`；不得就地编辑旧 plan 或继续使用失效授权。

## 17. Authorization 与外部状态安全

```rust
pub struct ExecutionAuthorization {
    pub plan_digest: String,
    pub allowed_effects: BTreeSet<AuthorizedEffect>,
    pub expires_at: Option<SystemTime>,
    pub user_authorization_ref: String,
}
```

至少区分：

```text
LocalCi
RealEditorSession
RealOsWindowOrGpu
ProductionBinaryReplacement
InstalledBinaryMutation
RealConfigurationMutation
OneShotExternalAcceptance
OwnedRecursiveCleanup
```

授权必须绑定 plan digest，不能仅以“运行全部测试”泛化。v2-A 即使收到 authorization 也不得执行；
这是版本能力缺失，不是运行时开关。

涉及递归 cleanup 时继续遵守 `game-engine-construction` 的 strict-child、literal path、reparse-point、
owned descendant、evidence preservation 和结果记录规则。Module 只计划和校验 ownership，不扩大删除权限。

## 18. ExecutionReport

未来 schema 至少包含：

```json
{
  "schemaVersion": "construction-validation.execution-report.v2",
  "planRef": {},
  "status": "passed|failed|ineligible|stopped_by_time_limit|cleanup_incomplete",
  "sourceIdentity": {},
  "authorizationRef": "...",
  "stages": [],
  "observedEvidence": [],
  "reusedEvidence": [],
  "firstFailure": null,
  "omittedStages": [],
  "externalState": [],
  "cleanup": [],
  "durationSeconds": 0,
  "diagnostics": []
}
```

报告不能只保存 test name 和 exit code。失败 stage 必须链接首个 assertion / diagnostic、stdout / stderr
artifact、fixture、environment 和 next action。

## 19. 分阶段实施

### v2-A：Plan-only Foundation

唯一允许的第一施工范围：

- 在现有 `quality_gate` crate 新增 `ConstructionValidationModule::prepare`；
- 建立 typed request、catalog、plan、report、diagnostic schema；
- 实现 affected closure、claim mapping、`subsumes` 去重、evidence validity 判定和成本估算；
- 使用 in-memory / fixture evidence store 验证确定性；
- CLI 如需暴露，只允许 `quality_gate validation-plan ...` 之类的只读计划入口；
- 产物只允许写入明确的 run-owned `target/quality-gate/validation-plans/<run-id>/`；
- 不接入 command executor，不调用 `verify` / `local-ci`，不启动任何进程。

### v2-B：Evidence Index

在 v2-A 稳定后，增加 repository-owned evidence index、report digest 校验、partial invalidation 和
historical duration。仍可保持 plan-only；是否单独施工由后续施工文档决定。

### v2-C：Bounded Execution

引入有限 `ValidationExecutor` adapter，只允许调用 catalog 已注册的现有 producer。低成本 development /
integration 路径先接入；必须有显式 `execute` 调用，禁止 prepare 自动执行。

### v2-D：High-cost / External-state Qualification

最后接入 Local CI、真实 Editor、production / installed binary 与真实配置类 verifier，落实 plan-bound
authorization、fresh role root、binary / composition identity、事务 rollback 和 cleanup report。

每阶段都必须重新生成独立施工文档并由用户激活；前一阶段完成不自动授权下一阶段。

## 20. 从当前施工 Skill / 文档迁移

迁移采用“结构化替换”，不做双轨永久叠加：

1. v2-A 前，`game-engine-construction` 继续是施工验证唯一规则真相。
2. v2-A 后，施工文档可先声明 `PrepareRequest`，附 `PrepareReport`，人工仍执行命令。
3. 当 catalog 覆盖某类选择逻辑后，从施工模板删除对应的手工 owner / consumer / duplicate 计算说明，
   只保留 claim、特殊风险、授权边界和正式验收要求。
4. v2-C 后，低成本执行可委托 Module；未迁移 verifier 仍按现有施工规则运行并在 report 中标记 manual。
5. v2-D 后，高成本执行也只能在 plan-bound authorization 下进入现有 producer。

迁移完成的衡量不是新增多少规则，而是每引入一项结构化治理，就删除一项重复手工菜谱。

## 21. v2-A 测试与验收

未来施工文档至少覆盖：

### Module owner

- 同一 request + catalog + evidence identities 生成字节稳定的 canonical plan digest；
- changed owner 能闭包到直接和传递 consumer；无关 domain 不进入计划；
- Development / Integration / Freeze / ReleaseActivation 产生正确 proof obligations；
- suite A 被同环境 suite B `subsumes` 时只保留 B，并记录 eliminated reason；
- platform、feature、profile 或 composition 不同则不得错误去重；
- harness-only、source-only、binary-only 变化只失效实际消费该 identity 的 evidence；
- 缺失 catalog entry、环形 `subsumes`、未知 producer、超预算和授权上限不足均 fail closed；
- owner-level red-capable stage 排在高成本 consumer / matrix 前；
- cleanup reserve 计入总预算。

### Plan-only negative proof

- production wiring 中不存在 `QualityCommandExecutor` 依赖；
- scripted executor 记录为零调用；
- prepare 前后无 child process、无 Local CI worktree、无 production / installed mutation；
- 即使 request 携带所有 authorization，v2-A 仍只输出 plan；
- CLI 不存在 `--execute`、`--apply` 或等价隐藏路径。

### Existing quality_gate consumers

- 现有 `verify`、`local-ci`、`propose-ledger` 行为和 report schema 不变；
- `QualityGateRunner` 与 `LocalCiRunner` 现有 owner tests / affected tests 通过；
- 不新增第二份 lint、architecture 或 Local CI truth。

### 文档验收

- 施工文档明确只覆盖 v2-A；
- 三个不可变限制逐条映射到 Gate 和 negative test；
- 未经用户另行授权，不运行 Local CI、不替换 production / installed binary、不修改真实配置。

## 22. 预计文件范围

v2-A 预计只涉及：

```text
rust/crates/quality_gate/src/lib.rs
rust/crates/quality_gate/src/main.rs
rust/crates/quality_gate/src/construction_validation.rs
rust/crates/quality_gate/src/validation_catalog.rs
rust/crates/quality_gate/src/validation_evidence.rs
rust/crates/quality_gate/tests/*
rust/quality/construction-validation-catalog.v1.json（若施工复核确认该位置）
框架设计/引擎总体架构/施工文档/...
```

具体拆分和 catalog 路径由未来施工文档按当前代码基线复核。不得因此修改 Runtime、Editor、AUI、
AI Tool 或 sample project。

## 23. 风险与控制

### 风险 1：变成第二个 QualityGateRunner

控制：Module 只选择 verifier 和管理 evidence；实际 quality stages 继续由现有 Runner 生产。

### 风险 2：变成通用 workflow engine

控制：catalog 不接受任意 shell、循环、变量或动态 DAG；producer id 必须代码注册。

### 风险 3：计划正确但施工文档继续重复旧菜谱

控制：迁移遵循 replace-don't-layer；每个结构化能力落地时删除对应手工步骤。

### 风险 4：错误复用过期证据

控制：按 consumed identity 分项绑定；未知或缺失 identity 一律不可复用。

### 风险 5：`subsumes` 掩盖不同配置

控制：只有 environment identity 完全相容才去重；差异形成独立 evidence obligation。

### 风险 6：prepare 偷跑验证

控制：v2-A production 类型不依赖 executor，配合零进程 / 零副作用 negative test。

### 风险 7：高成本授权被泛化

控制：authorization 绑定 immutable plan digest 与 effect set；任何漂移必须重新 prepare。

## 24. 方案自审

### 24.1 是否符合用户选择的方案 C

通过。使用深 Module 和小 `prepare / execute` interface；默认 CLI 可保持简单，高风险执行仍可审查。

### 24.2 是否坚持三个限制

通过。复用 `quality_gate`；catalog 是有限 verifier registry，不是通用 workflow engine；v2-A 明确
plan-only，并设置零 executor / 零进程 / 零副作用验收。

### 24.3 是否为深 Module

通过。closure、claim mapping、去重、evidence invalidation、成本和排序均隐藏在小 interface 后；
caller 不需拼装算法。

### 24.4 是否误改 Runtime / Editor / AI Tool seam

没有。外部 seam 位于 `quality_gate` 治理层，产品运行链路和 AI-facing Tool interface 不变。

### 24.5 是否重复现有 Runner

没有。现有 Runner 是 verifier producer；新 Module 是 validation planner / governed executor，二者职责不同。

### 24.6 是否把 owner / consumer 变成固定三轮测试

没有。它们是 affected closure 与 proof obligations；物理 suite 通过最小覆盖与 `subsumes` 选择。

### 24.7 是否保持外部状态安全

通过。高成本 effects 必须 plan-bound authorization；v2-A 无执行能力。

### 24.8 是否改变 240 队列或激活施工

没有。本方案是用户明确指定的流程治理插入；240 已关闭项保持原状态。v2-A 归档后施工槽为空。

### 24.9 是否可以直接施工

v2-A 已完成并归档到 `施工文档/已完成/`，Gate A-E 与最终 affected closure 全部通过。后续如需进入
v2-B，必须单独生成、自审、激活施工文档并取得用户授权；v2-A 完成不自动授权后续阶段。

## 25. 正式结论

Construction Validation Plan v2 采用方案 C：

```text
existing quality_gate
  ├─ QualityGateRunner                 existing verifier producer
  ├─ LocalCiRunner                     existing exact-commit adapter
  └─ ConstructionValidationModule      new governance module
       ├─ prepare(request) -> PrepareReport
       └─ execute(plan_ref, authorization) -> ExecutionReport
```

首轮仅施工 v2-A：建立可审查、确定性、无副作用的验证计划。只有后续独立方案 / 施工授权才能逐步进入
evidence index、bounded execution 和高成本 qualification。

## 26. 参考

- Nx affected：<https://nx.dev/ci/features/affected>
- Gradle Build Cache：<https://docs.gradle.org/current/userguide/build_cache.html>
- Bazel iteration speed：<https://bazel.build/advanced/performance/iteration-speed>
- Cargo test：<https://doc.rust-lang.org/cargo/commands/cargo-test.html>
- `245-Reproducible-Toolchain-CI-Lint-Budget-Gate-v1方案.md`
- `240-5.6审查剩余问题讨论与施工优先级.md`
- `rust/crates/quality_gate/src/lib.rs`
- `rust/crates/quality_gate/src/runner.rs`
- `rust/crates/quality_gate/src/local_ci.rs`
- `rust/crates/quality_gate/src/change_scope.rs`
- `.agents/skills/game-engine-construction/SKILL.md`
