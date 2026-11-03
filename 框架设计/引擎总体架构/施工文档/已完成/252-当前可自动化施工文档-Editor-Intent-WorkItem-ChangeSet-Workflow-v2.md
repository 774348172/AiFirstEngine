# 252 当前可自动化施工文档：Editor Intent / WorkItem / ChangeSet Workflow v2

> 状态：已完成，等待归档  
> 对应方案：`252-Editor-Goal-level-Iterative-Project-Production-Workflow-v1方案.md` 正文 v2  
> 生成日期：2026-07-16  
> 专用 target：`G:\gameEngin-targets\252`

## 1. 施工目标

建立一个深 `ProjectIntentWorkflow` Module，以小 Interface 承接自由表达、长期 WorkItem、可选诊断、局部 ChangeSet 批准和既有 Candidate 执行：

```text
capture -> observe -> prepare_change -> authorize -> dispatch
```

本次必须证明：

```text
用户表达可以不完整、矛盾、零散和跨会话。
IntentEvent 不可变，WorkItem 的纠正、拆分、合并和重开保留 lineage。
局部 readiness 不被无关 parked/open-question WorkItem 阻塞。
只读诊断不获得项目写权限；instrumentation 必须进入 ChangeSet。
批准只绑定 selected WorkItem revision、proposal、base、风险、费用和 repair policy。
无关 WorkItem 变化不使批准失效；selected WorkItem 语义变化 fail-closed。
from-blank 批准前不创建目标目录；正式 CreateProject 后才接管 journal。
同一项目最多一个 active mutation lane。
ProjectProductionRun 复用 ProjectCandidateEntry，不复制 Apply/rollback 真相。
Intent/WorkItem/Diagnosis/Run 数据不进入 RuntimePackage。
```

## 2. 范围与涉及文件

主要新增或修改：

```text
rust/crates/editor_core/src/project_intent_workflow/mod.rs
rust/crates/editor_core/src/project_intent_workflow/model.rs
rust/crates/editor_core/src/project_intent_workflow/journal.rs
rust/crates/editor_core/src/project_intent_workflow/execution.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/services/project_service.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/editor_core/src/tests/project_intent_workflow_tests.rs
rust/crates/editor_ui_model/src/project_intent.rs
rust/crates/editor_ui_model/src/lib.rs
```

只有产品入口确有需要时才允许触碰：

```text
rust/crates/editor_ui_model/src/launcher.rs
rust/crates/editor_ui_model/src/ai_panel.rs
rust/crates/editor_ui_renderer/**
rust/crates/editor_window_winit/**
rust/crates/project_e2e_gate/**
```

明确不改：

```text
ProjectPatch / ControlledSourcePatch / ProjectAssetImport 的 lowering 与 Apply 真相
RuntimePackage schema 和 runtime 热路径
251 固定 C-01 payload builder 的产品所有权
Unity / UE 项目与三引擎 B 通道
Provider Registry、通用 Agent Planner、任意知识图谱
```

## 3. Gate A：基线、schema 与 journal

任务：

```text
定义 schema-versioned IntentEvent、WorkItem、关系、ProjectIntentSnapshot 与错误模型。
实现 append-only journal、canonical digest、checkpoint 重建和 command-id 幂等。
实现 in-memory、pre-project local draft 和 project Library journal 三种受控存储状态。
capture 只做本地 bounded append，不调用 Provider、编译、Preview 或项目 mutation。
```

施工前基线：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-targets\252'
cargo test -p editor_core project_candidate_entry
cargo test -p editor_core project_service
cargo test -p editor_core session_smoke
cargo fmt --all -- --check
```

已确认结果：10 + 4 + 3 项通过，fmt 通过。首次冷编译超过 120 秒外层 timeout；复用缓存后测试通过，此项是环境成本，不是基线失败。

Gate 测试：

```powershell
cargo test -p editor_core project_intent_workflow_capture
cargo test -p editor_core project_intent_workflow_journal
```

## 4. Gate B：WorkItem 状态、关系与局部 readiness

任务：

```text
实现 idea/requirement/change/bug/question/feedback/experiment。
实现 Park/Resume/Reopen/Merge/Split/Cancel，所有转换追加 journal event。
固定关系只支持 depends_on/blocks/related_to/duplicates/supersedes/caused_by/evidence_for。
readiness 只传播到显式依赖，不建立全局 FeatureSpec Gate。
ProjectGoalSnapshot 仅为 selected WorkItem 的可丢弃派生视图。
```

Gate 测试：

```powershell
cargo test -p editor_core project_intent_workflow_work_item
cargo test -p editor_core project_intent_workflow_readiness
```

## 5. Gate C：from-blank draft、项目 journal 与接管

任务：

```text
pre-project draft store 位于 Launcher 本地显式路径，不能位于目标项目根。
批准前 capture/prepare 不创建 target root。
正式 ProjectLauncherState::create_project 成功后，按 project id、canonical root、initial project digest 和 creation receipt digest 接管。
existing-project open 从 Library 中恢复 journal；缺失时创建空 journal，不猜测聊天历史。
接管失败保持 pre-project draft，不产生半绑定 project journal。
```

Gate 测试：

```powershell
cargo test -p editor_core project_intent_workflow_from_blank
cargo test -p editor_core project_intent_workflow_reopen_persistence
cargo test -p editor_core project_service
```

## 6. Gate D：Diagnosis / Evidence 与只读 capability

任务：

```text
实现 ProjectDiagnosisSession、reproduction、observation、hypothesis、confirmed cause 和 evidence refs。
只读 capability 只允许项目读取、已有测试/Preview/复现和隔离 evidence root 输出。
任何项目文件、依赖、配置或 instrumentation 变化返回 change_set_required blocker。
Reopen 链接旧 proposal、apply receipt 和 regression verification evidence。
```

Gate 测试：

```powershell
cargo test -p editor_core project_intent_workflow_diagnosis
cargo test -p editor_core project_intent_workflow_read_only
```

## 7. Gate E：ChangeSetProposal、Approval 与失效规则

任务：

```text
prepare_change 只选择 ready WorkItem revision；返回完整 proposal 或结构化 blockers。
proposal 绑定 base、selected digests、outcome、exclusion、step、risk、cost、acceptance 和 repair policy。
authorize 原子校验 approval digest、proposal digest、target identity 和 base。
approval replay/tamper、selected semantic drift、base drift 一律 fail-closed。
未选 WorkItem 的新增、修改、Park、Resume 或诊断证据不使 approval 失效。
```

Gate 测试：

```powershell
cargo test -p editor_core project_intent_workflow_change_set
cargo test -p editor_core project_intent_workflow_approval
```

## 8. Gate F：ProjectProductionRun 与 Candidate Entry

任务：

```text
authorize 只创建已绑定、未持有无限 ambient authority 的 Run。
dispatch/AdvanceRun 按顺序 materialize ProjectCandidateEnvelope。
每一步复用 ProjectCandidateEntry::prepare/validate/apply，并派生精确 internal Candidate approval。
每个下一步绑定上一 apply receipt 后的真实 project digest。
一个 ChangeSet 可包含 AssetImport、ProjectPatch、ControlledSourcePatch。
失败提供 retry/reprepare/rollback/keep-current/park；不伪造跨 Candidate 原子事务。
同一 target identity 只有一个 active mutation lane。
```

Gate 测试：

```powershell
cargo test -p editor_core project_intent_workflow_execution
cargo test -p editor_core project_intent_workflow_mutation_lane
cargo test -p editor_core project_candidate_entry
```

## 9. Gate G：EditorSession 与 UI model 产品入口

任务：

```text
EditorSession 持有 workflow 真相，并在 create/open/select-recent 后完成 attach/load。
新增 ProjectIntentModel、ProjectChangeReviewModel、ProjectProductionModel 序列化 view model。
AiPanelModel 只保留对话，不复制 journal/proposal/run 真相。
Launcher 暴露 Create with AI 的模型入口；长操作只发 command，不阻塞 UI thread。
Summary 只暴露计数、当前修改、等待原因和最新结果；Trace 才含 digest/evidence/receipt。
```

Gate 测试：

```powershell
cargo test -p editor_core project_intent_workflow_session
cargo test -p editor_core session_smoke
cargo test -p editor_ui_model
```

## 10. Gate H：确定性产品 workflow Gate

用通用 schema 构造确定性场景，不把 C-01 玩法写入 core：

```text
不少于十次零散、修正、矛盾和无关表达。
parked 大需求不阻塞 ready 小修改。
无法复现 Bug 在保存重开后补证据继续。
一次 ChangeSet approval 派生多个 Candidate。
执行中新增无关 WorkItem 不使 Run stale。
selected WorkItem 语义变化使 Run 安全暂停并要求重新批准。
Save/reopen 后 lineage、proposal、receipt、verification 可追溯。
```

Gate 测试：

```powershell
cargo test -p editor_core project_intent_workflow_golden_gate
```

如果本 Gate 暴露真实 Native Editor/Window 所有权缺口，先补结构化产品入口，再进入受影响域；不得用测试 fixture 伪装产品功能。

## 11. Gate I：受影响域回归

进入条件：Gate A-H 的累计失败集全绿，候选代码和依赖已冻结。

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-targets\252'
cargo test -p editor_core
cargo test -p editor_ui_model
cargo test -p project_e2e_gate c01_from_blank_creation
cargo fmt --all -- --check
git diff --check
```

预计成本：editor_core 数分钟，251 回归可能数分钟到十余分钟。失败时只复现首个真实失败并加入累计失败集，不立即重跑 workspace。

## 12. Gate J：最终权威回归

进入条件：受影响域、环境合同、高风险清单全部通过；除完成记录和入口文档外，代码、manifest、lockfile、测试和 ledger 冻结。

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-targets\252'
cargo test --workspace
cargo test --workspace --all-features
```

最终回归失败时，从首个结构化失败精确复现；累计失败集、受影响域和环境预检重新全绿后才允许第二次权威运行。

## 13. 高风险否定矩阵

```text
[ ] 不完整或互相矛盾的表达仍可捕获。
[ ] WorkItem 来源 lineage 不丢失。
[ ] parked/open-question 项不阻塞无依赖 ready 项。
[ ] unselected WorkItem 更新不使 approval/run stale。
[ ] selected WorkItem 用户可见语义变化使 approval/run stale。
[ ] base project digest drift fail-closed。
[ ] read-only diagnosis 不写项目。
[ ] instrumentation 只能进入 ChangeSet。
[ ] approval digest tamper/replay 失败。
[ ] 同项目只有一个 active mutation lane。
[ ] from-blank 批准前不创建目标目录。
[ ] journal 接管绑定 CreateProject receipt/initial digest。
[ ] Reopen 链接旧 proposal、receipt 和 verification。
[ ] Intent/journal/diagnosis/run 不进入 RuntimePackage。
[ ] command id replay 幂等或返回已有 terminal receipt。
```

## 14. 施工自审

```text
用户授权：通过。用户明确要求生成、自审并按规则开始施工。
正式方案一致：通过。本文以 252 v2 为唯一设计真相，没有恢复全局 FeatureSpec 前门。
当前唯一性：通过。激活前 当前/ 无非 README 文档，待执行/ 只有本文；现已移动到唯一 当前/ 槽位。
范围控制：通过。只新增 workflow 编排，不复制 Candidate、Apply、Build、Preview 或 RuntimePackage 真相。
Module 深度：通过。外部只暴露 capture/observe/prepare_change/authorize/dispatch，复杂持久化和执行留在 Implementation。
用户摩擦：通过。捕获不要求 issue 表单、完整 spec、编程知识或一次说清。
安全边界：通过。诊断只读 capability、批准绑定、base drift 和单 mutation lane 均有否定测试。
from-blank：通过。pre-project draft 与 target root 分离，正式创建后才接管。
测试分层：通过。每 Gate 定向测试、受影响域和最终 default/all-features 均已列出。
环境成本：通过。记录首次冷编译 timeout，不把完整 workspace 用作逐项调试循环。
既有混合变更：通过。本施工不声明 clean exact commit，不回滚、不清理、不把无关变更归为 252 成果。
```

结论：施工文档无扩大范围、无跳过测试、无正式方案冲突；激活复核与入口同步已完成，可以从 Gate A 开始修改代码。

## 15. 完成、记录与归档

全部 Gate 通过后：

```text
生成 阶段完成记录/2026-07-16-Editor-Intent-WorkItem-ChangeSet-Workflow-v2/00-总览.md。
记录三层验证结果、权威 run 次数与耗时、失败重跑原因、慢项和残余风险。
更新 00-文档地图、49、54、240、施工文档/README、阶段完成记录/README。
把本文从 施工文档/当前 移动到 施工文档/已完成，并改为已完成状态。
```
