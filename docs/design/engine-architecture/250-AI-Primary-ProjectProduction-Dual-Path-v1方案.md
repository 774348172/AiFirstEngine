# 250-AI-Primary ProjectProduction Dual Path v1 方案

> 状态：用户已确认，正式方案已自审  
> 确认日期：2026-07-14  
> 用户选择：方案 C，建立单一深 `ProjectProduction Module`  
> 产品对象：没有编程基础和只有少量编程基础的游戏创作爱好者  
> 首个验证任务：C-01 2D Combat Arena Vertical Slice v1

## 1. 结论

项目生产层采用一个深 Module、两个 lowering 通道和一个候选版本真相：

```text
User Intent / Imported Codex Candidate / Editor Authoring
  -> ProjectProduction Module
     -> FeatureSpec / ChangeRequest
     -> ProjectPatch                 (标准、结构化工程修改)
     -> Controlled Project SourcePatch (复杂项目 Rust 逻辑)
     -> CandidateProjectRevision
     -> Validation / Review / Apply / Rollback
     -> Preview / Export Evidence
```

Provider 只是 `ProjectProduction Module` 内部的 Adapter，不再是项目生产 Interface。
内置 Provider、外部 Codex 和人工可视化编辑必须进入同一候选、验证、审阅、应用和证据链。

## 2. 当前问题

当前各子系统分别存在，但不能从空项目稳定生产一个复杂可玩项目：

```text
ProjectLauncher 创建的空项目声明 engine.empty.runtime，
但 manifest 同时填写 RuntimeModule/Cargo.toml，目录中却没有该文件。

ProjectPatch 能修改 Scene / Input / Asset / Prefab / AUI / Rule / Build，
但没有受控项目源码通道，复杂逻辑无法从空项目正式产生。

真实 Provider 失败会阻断候选生成；外部 Codex 即使能生成源码，
当前也没有正式 imported source candidate 入口。

复杂打飞机样例能够运行，只证明预置项目 Module 可运行，
不证明零基础用户能从空项目生产相同能力。
```

这里的根因不是 Renderer、ECS、AUI 或 Build 需要推倒重来，而是项目生产行为分散在多个浅 Interface 中，没有一个 owner 对完整结果负责。

## 3. Module 与 Interface

### 3.1 外部 Interface

第一版长期 Interface 保持为四类行为：

```text
inspect(ProjectRoot) -> ProjectReadinessReport
propose(ChangeRequest | ImportedCandidate) -> CandidateProjectRevision
validate(CandidateRevisionId) -> ProjectValidationReport
apply(ApprovedRevisionId) -> ProjectApplyReceipt
rollback(AppliedRevisionId) -> ProjectRollbackReceipt
```

Preview 和 Export 是 validation/evidence 的正式阶段，不要求调用者理解内部 ProjectPatch、Cargo、RuntimePackage 或文件事务细节。

### 3.2 内部 seams

仅在确有不同 Adapter 时建立内部 seam：

```text
Candidate Source seam
  - Built-in Provider Adapter
  - Imported Codex Candidate Adapter

Change Lowering seam
  - ProjectPatch Adapter
  - Controlled SourcePatch Adapter

Runtime Validation seam
  - Headless Validation Adapter
  - Project Runtime Dev Host Adapter（后续阶段）
```

这些 seam 不暴露给普通用户。用户只看到功能目标、影响、验证、确认和恢复。

## 4. Runtime Module 合同修正

必须明确区分两种 runtime source：

```text
BuiltInEmpty
  moduleId = engine.empty.runtime
  不拥有项目 Cargo manifest
  使用引擎随附 empty_project_player
  空项目可 Preview / Export

ProjectRust
  拥有 project-relative RuntimeModule/Cargo.toml 与 src/**/*.rs
  必须通过 Controlled SourcePatch 产生或修改
  必须验证 manifest、源码、依赖、compile、test 和 artifact identity
```

禁止继续用“填了 Cargo 路径但实际不存在”表达 BuiltInEmpty。兼容读取旧 `aife-project.v2` 空项目，但新建项目必须写出明确的 source kind。

项目 Rust 依赖不能使用只在仓库样例中成立的 `../../../rust/crates` 相对路径。ProjectRust 脚手架必须通过版本化 Engine SDK/Project SDK locator 或等价稳定合同解析依赖；该合同随 Controlled SourcePatch 阶段施工，不在 BuiltInEmpty 阶段伪造。

## 5. Project Readiness

`ProjectReadiness` 是 `ProjectProduction Module` 的第一条真实 Interface。它必须返回结构化报告，不是一个布尔值：

```text
schema_version
status
project_root
project_kind
checks[]
diagnostics[]
next_actions[]
```

最小检查：

```text
manifest schema / engine version
default Scene 存在且可解析
Input asset 存在且可解析
asset root 与必要项目目录存在
BuiltInEmpty 合同一致，或 ProjectRust manifest/source 存在
player artifact availability（按验证级别）
```

状态至少区分：

```text
Ready
Incomplete
Invalid
Unsupported
```

`open_project` 只在 authoring readiness 通过时成功。Preview / Export 在各自入口请求更高验证级别，不能让 Launcher 冒充完整 Build Gate。

## 6. 双通道规则

### 6.1 ProjectPatch

用于已有 schema 和正式 Editor command 能表达的标准修改。继续继承：

```text
schema-first
validator
review
transaction apply
rollback
structured report
```

不得为了 C-01 新增 Player、Enemy、Bullet、Score、Wave 等玩法专用 Engine Core operation。

### 6.2 Controlled SourcePatch

用于复杂项目专用 Rust 逻辑：

```text
只写 project-owned allowlist roots
默认禁止 engine source、RuntimePackage、Library、Build 输出和 AOT 派生产物
先生成隔离 candidate，再做 diff/risk/dependency/compile/test
用户批准后事务 apply
失败可恢复到 before revision
编译器诊断转换成功能级说明，原始 evidence 可展开
```

Rule IR 继续只承担 contract-bound RuleSlot；禁止扩张成 Lua/Blueprint 式通用语言来回避 SourcePatch。

## 7. CandidateProjectRevision

所有来源共用一个候选版本对象：

```text
revision_id
base_project_digest
feature_id / requirement_ids
project_patch
source_patch
asset_imports
changed_paths
validation_state
review_state
artifact_digests
```

候选必须在隔离目录验证，不能在真实项目上边生成边编译。Apply 前检查 base digest，发现漂移则拒绝或重新生成，不做静默覆盖。

## 8. 分阶段施工

### Phase 1：完整空项目合同与 Readiness Gate

状态：已完成并归档，见 `阶段完成记录/2026-07-14-ProjectProduction-Empty-Project-Readiness-v1/00-总览.md`。

```text
修正 BuiltInEmpty / ProjectRust 语义。
新建空项目不再声称拥有不存在的 RuntimeModule source。
建立结构化 ProjectReadinessReport。
旧空项目兼容迁移/读取。
证明空项目可组装 RuntimePackage，并能定位 empty player artifact。
```

### Phase 2：CandidateProjectRevision

状态：已完成并归档，见 `阶段完成记录/2026-07-14-CandidateProjectRevision-v1/00-总览.md`。

建立隔离候选、base digest、changed paths、validation state、批准与回滚合同。

### Phase 3：Controlled SourcePatch

状态：已完成并归档，见 `阶段完成记录/2026-07-14-Controlled-SourcePatch-v1/00-总览.md`。

建立 project-owned Rust allowlist、SourcePatch schema、diff review、compile/test 和事务 Apply。

### Phase 4：正式 Asset Import

状态：已完成并归档，见 `阶段完成记录/2026-07-14-Formal-Asset-Import-v1/00-总览.md`。

建立外部文件复制、GUID/meta、AssetDB/Graph 注册、license/source metadata、冲突和回滚。

### Phase 5：Provider-independent Candidate Entry

状态：已完成并归档，见 `阶段完成记录/2026-07-14-Provider-independent-Candidate-Entry-v1/00-总览.md`。

内置 Provider 和 imported Codex candidate 共用 schema 与 candidate pipeline；Provider transport 失败不再等于项目生产系统不可用。

v1 使用严格 `ProjectCandidateEnvelope`，每个候选只承载一种 `ProjectPatch`、`ControlledSourcePatch` 或 `AssetImport` payload。来源 Adapter 只负责生成 envelope；项目绑定、candidate/validation digest、显式 approval、Apply/rollback 统一由 `ProjectCandidateEntry` 深 Module 调度既有三套正式 lowering。v1 不伪造跨 payload 原子批次，也不进入 Provider Registry、Agent Planner、Dev Host 或 C-01。

### Phase 6：C-01 Golden Gate

状态：已完成并归档。真实 `<LOCAL_TEST_ROOT>\AiFirstGame` 已通过 validation-only v3 Preview/Export/external package/像素验收；见 `施工文档/已完成/250-F-当前可自动化施工文档-C-01-Golden-Gate-v1.md` 与 `阶段完成记录/2026-07-15-C-01-Golden-Gate-v1/00-总览.md`。

从全新空项目通过正式表面完成 Sprite、Input、Prefab、玩法、AUI、Preview、重开和 Windows Export，并保存确定性 evidence。

Phase 6 真实基线确认还需补齐三条通用产品能力：Scene/Prefab component authoring、project rule manifest generation、ProjectRust Player artifact/Dev Host。它们不得携带 C-01 玩法专用语义，只作为 ProjectProduction 的通用 lowering/validation consumer 实现。

## 9. 验证策略

每阶段三层验证：

```text
定向测试：当前 Module Interface 与否定矩阵。
受影响域回归：Launcher / RuntimePackage / Play / Export / ProjectPatch consumers。
最终权威回归：clean exact source、default/all-features、C-01 对应 Golden Gate。
```

长 Runner 继续使用 stage evidence、隔离 target 和一次权威运行纪律。外部 Provider 只在 models/minimal/structured-output smoke 通过且用户授权后进入 canonical request。

## 10. 明确不做

```text
不推倒 Rust Runtime、ECS、Renderer、AUI、RuntimePackage 或 Build Graph。
不把项目玩法写入 Engine Core。
不让 AI 直接写最终 RuntimePackage 或 AOT 派生产物。
不让 ProjectPatch/SourcePatch 绕过 review、validation 或 rollback。
不把 Provider Registry、Agent Planner 或多轮 autonomous loop 塞进 Phase 1。
不以预置 C-01 样例冒充从空项目生产。
```

## 11. 审查输入处理

已完整读取 `其它AI审查目录/45`、`46`、`51`：

| 审查结论 | 处理 |
|---|---|
| 单一深 ProjectProduction Module | 采纳为正式 owner |
| ProjectPatch + Controlled SourcePatch | 采纳为双 lowering 通道 |
| isolated CandidateProjectRevision | 采纳，安排 Phase 2 |
| Project Runtime Dev Host | 采纳为 SourcePatch 后续 consumer，不在 Phase 1 先造进程壳 |
| C-01 记录 SourcePatch 与空 RuntimeModule 缺口 | 采纳为 Phase 1/3/6 验证输入 |
| 不能直写 RuntimeModule 冒充产品能力 | 采纳为硬禁止项 |

## 12. 方案自审

```text
符合用户选择：是，采用方案 C。
Module 是否足够深：是，外部 Interface 小，内部隐藏双通道、候选、验证和事务。
是否增加无真实 Adapter 的 seam：否；Provider/imported、ProjectPatch/SourcePatch 均有两个真实变化点。
是否支持零基础用户：是，用户面对功能、影响、验证和恢复，不面对 Cargo internals。
是否支持复杂项目：是，复杂项目逻辑进入 project-owned Rust，而不是膨胀 IR。
是否过度施工：否，按六阶段纵向推进，Phase 1 不提前实现 SourcePatch/Dev Host。
是否与 195/196/207/242 冲突：否；继承 IR 红线、ProjectPatch 和 ProjectRuntimeModule，只重置项目生产 owner。
```

最终决定：正式采用本方案，第一份施工只执行 Phase 1。
