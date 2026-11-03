# 253 当前可自动化施工文档：AI Capability-First Tool Kernel / Agent-Owned Planning v1

> 状态：已完成并归档  
> 激活日期：2026-07-17  
> 正式方案：`../../253-AI-Capability-First-Tool-Kernel-Agent-Owned-Planning-v1方案.md`  
> 用户授权：生成施工文档、自审并开始施工  
> 归档日期：2026-07-17；当前施工槽位已清空

## 1. 施工目标

本次施工把 253 的架构决定落成一个最小但真实、可扩展的正式纵切：

```text
Tool Catalog / inspect
  -> CapabilityGrant
  -> execute mutation / preview tool
  -> structured ToolResult / Operation / Receipt
  -> receipt lineage / drift fail-closed
  -> AI 可根据结果自主选择下一工具
```

完成后必须满足：

- `editor_core` 提供小而深的 `AiCapabilityToolKernel` Interface；
- ToolDescriptor、Invocation、Grant、Result、Operation、Receipt 使用稳定 schema；
- ProjectCandidateEntry 三类 payload 可通过一个 mutation 工具完成 prepare/validate/apply；
- 用户一次 Grant 为每个已验证 Candidate 派生精确 CandidateApproval，不重复请求用户；
- grant lineage 允许同一授权下连续合法修改，拒绝未知 project drift；
- Preview 是独立工具，先验证 project-owned RuntimeModule binding，不静默回落到 empty runtime；
- mutation lane 只覆盖真实 Apply，不再被 252 的 Previewing 长期占用；
- IntentJournal / WorkItem / Diagnosis 保留；新 Tool Kernel 不依赖 WorkItem 才能工作；
- 外部调用不需要 `AdvanceRun` 才能推进工具内部确定性动作；
- 不新增通用 Agent Planner，不保存模型隐藏推理。

## 2. 当前基线与隔离

当前工作树包含 249-252 历史施工和用户已有修改，不能用 `HEAD` 单独代表真实施工基线，也不能为了 clean worktree 丢弃这些未提交能力。因此本次采用等价隔离：

```text
Git HEAD：9f5adc73ca02a5e3fed3f38c62bb9a420e6bd5de
源码真相：当前工作树 + 下列施工前 SHA-256
构建隔离：独立 CARGO_TARGET_DIR=G:\gameEngin-target\253
修改隔离：本文白名单 + 新文件；不还原、不格式化无关用户改动
验证真相：候选源码 fingerprint + 定向/受影响域/default/all-features 结果
```

施工前关键文件 SHA-256：

```text
editor_core/src/lib.rs
  b79886bf53ec75f671b24e137388259185e415ef8c42e73d99ac2d2c99e3683b
editor_core/src/session.rs
  bf6aa2b24e47b50d65b9d8d75fbf135083c42c9778cb1b2bf7c16472c18b0262
editor_core/src/services/play_service.rs
  292040fbd2217ebaaaf999c9246bc4568722a6d7f610906b7ae4eea8f66d4735
editor_core/src/project_candidate_entry.rs
  bf2b768fe6aa3a5c9e5647b181e1f544b3db3ff3108f88823c832799df027983
editor_core/src/project_intent_workflow/model.rs
  47108a15b0ac67ca1a20d26efd88afc328a8ff385668796041700939a330889f
editor_core/src/project_intent_workflow/execution.rs
  27b064376fea02d0dace7d82873e75408ee31eb209b9d39869431d544635f249
editor_core/src/tests/mod.rs
  49703d24daf543a9e841523b849a93960149fbdab8649a500fb306d44dc94ea6
```

若施工中发现这些文件出现非本次改动，先重新读取并兼容，不覆盖未知修改。

## 3. 范围与涉及文件

预计新增：

```text
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
rust/crates/editor_core/src/tests/ai_capability_tool_kernel_tests.rs
```

预计修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/editor_core/src/project_candidate_entry.rs（仅补全 Tool Kernel 回滚所需的首个真实 diagnostic）
rust/crates/editor_core/src/input_mapping_authoring.rs（生成目录不得污染项目引用扫描）
rust/crates/editor_core/src/project_intent_workflow/model.rs
rust/crates/editor_core/src/project_intent_workflow/execution.rs
rust/crates/editor_core/src/services/play_service.rs（仅在 Tool Kernel 不能通过现有正式入口取得 binding evidence 时）
rust/crates/architecture_review/tests/loopback_provider.rs（仅在最终权威回归证明冷启动并发 loopback fixture 不稳定时）
框架设计/引擎总体架构/240-5.6审查剩余问题讨论与施工优先级.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

只有出现真实跨 crate consumer 需要时才扩大文件范围，并先回填本文。

明确不修改：

```text
engine_runtime 的 ECS / Renderer / AUI / Projection 所有权
Unity / UE 项目和安装目录
I:\EngineTest 三引擎 B 通道封存证据
250 / 251 / 252 历史完成记录
具体复杂打飞机玩法
Provider transport / API Key
```

## 4. Gate A：基线与 Tool Kernel schema

任务：

- 运行现有 Candidate、Intent Workflow、Play 最小 baseline；
- 定义 ToolDescriptor、Catalog、InspectRequest/Result、ToolInvocation、ToolResult；
- 定义 OperationSnapshot、CancellationReceipt、MutationReceipt；
- 定义稳定 schema version、deny_unknown_fields 和枚举序列化；
- Kernel 顶层只暴露 `catalog / inspect / execute / observe / cancel`；
- 不为每个工具增加顶层方法。

定向 baseline：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-target\253'
cd G:\gameEngin\rust
cargo test -p editor_core project_candidate_entry
cargo test -p editor_core project_intent_workflow
cargo test -p editor_core play_service
```

Gate 测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel_schema
cargo test -p editor_core ai_capability_tool_kernel_catalog
```

预计成本：baseline 2-15 分钟，增量测试 1-5 分钟。失败后只修 schema/Interface，不进入 Grant。

## 5. Gate B：CapabilityGrant 与 receipt lineage

任务：

- 定义 Read / ScopedMutation / Elevated capability；
- 绑定 project identity、outcome digest、initial base、domain、mutation kind、delete/dependency/network、time/cost/count/expiry；
- grant digest 使用 canonical serialization；
- 每个成功 mutation receipt 推进同一 grant lineage；
- 未知 drift、过期、超预算、跨 domain、高风险越权 fail-closed；
- Grant 不保存工具顺序、Candidate 列表或 plan steps。

Gate 测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel_grant
cargo test -p editor_core ai_capability_tool_kernel_lineage
cargo test -p editor_core ai_capability_tool_kernel_drift
```

预计成本：1-5 分钟。失败后不接入真实 mutation。

## 6. Gate C：深 mutation 工具与 Candidate 复用

任务：

- Catalog 注册统一 `project.mutate.candidate`；
- Invocation 携带 ProjectCandidateEnvelope 和 ValidationContext，不携带用户批准步骤序列；
- Kernel 内部执行 inspect binding -> prepare -> validate -> 派生 CandidateApproval -> apply -> receipt；
- derived approval 精确绑定 Grant、candidate 和 validation digest；
- ToolResult 返回 changed domains、before/after digest、receipt 和 diagnostics；
- Apply 锁只覆盖真实提交；
- rollback 继续使用 ProjectCandidateEntry 正式能力，不复制 lowering。

Gate 测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel_candidate
cargo test -p editor_core ai_capability_tool_kernel_candidate_rejects
cargo test -p editor_core project_candidate_entry
```

预计成本：2-10 分钟。任何 partial write、approval 越权或 receipt 缺失都阻断 Gate。

## 7. Gate D：project-owned Preview 工具

任务：

- Catalog 注册独立 `project.preview`；
- Tool 读取当前 project manifest、RuntimeModule identity 和 Candidate binding；
- ProjectRust 项目必须证明 preview package/runtime binding 与 manifest module 一致；
- 缺少 linked project runtime 时返回稳定 diagnostic，不得静默使用 empty runtime；
- BuiltInEmpty 项目仍可显式使用 empty runtime；
- Preview 不持有 mutation lane；
- ToolResult 返回 preview package report、runtime bind evidence 和 project digest；
- 同步完成的现有 Play 作为 v1 Operation completed snapshot；Interface 保留异步扩展但不伪造后台执行。

Gate 测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel_preview
cargo test -p editor_core ai_capability_tool_kernel_project_runtime
cargo test -p editor_core play_service
```

预计成本：2-15 分钟。发现 project-owned runtime 不可从当前 EditorSession 正式入口验证时，先增加最小深 Interface，不用测试 fixture 绕过。

## 8. Gate E：Operation、cancel、observe 与短 lane

任务：

- 每次 execute 产生可观察 OperationSnapshot；
- completed/failed operation 可跨 Kernel 调用读取；
- 同步操作 cancel 返回 already_terminal；
- cancellable 长操作只声明真实能力，不虚报已终止；
- `ProjectProductionRunState::Previewing` 不再占用 mutation lane；
- 252 历史 Run 仍可读取和收口，但不作为新 Tool Kernel 的执行前门。

Gate 测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel_operation
cargo test -p editor_core ai_capability_tool_kernel_cancel
cargo test -p editor_core project_intent_workflow_mutation_lane
```

预计成本：1-5 分钟。

## 9. Gate F：自主重规划与兼容纵切

任务：

- 用测试模拟同一 Grant 下“检查 -> mutation -> 发现新事实 -> 改变下一工具 -> Preview”；
- 不创建或读取 WorkItem 也能完成小型 mutation；
- WorkItem/Journal 仍可独立捕获并关联 receipt reference；
- Plan 不持久化为 Kernel truth；
- 不调用 AdvanceRun；
- 验证无关 WorkItem 变化不影响 grant lineage。

Gate 测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel_agent_owned_planning
cargo test -p editor_core ai_capability_tool_kernel_without_work_item
cargo test -p editor_core project_intent_workflow
```

预计成本：2-10 分钟。

## 10. Gate G：高风险否定矩阵

至少覆盖：

```text
unknown tool / schema version
wrong project identity
initial base drift / lineage fork
expired grant / mutation count exhausted
domain not allowed
delete / dependency / network / external cost escalation
candidate validation failed
candidate apply receipt missing
ProjectRust preview without matching linked runtime
BuiltInEmpty explicit preview
cancel terminal operation
observe unknown operation
Previewing does not own mutation lane
WorkItem absent
```

Gate 测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel
```

预计成本：2-10 分钟。

## 11. Gate H：受影响域回归

进入条件：Gate A-G 累计失败集全绿，候选源码 fingerprint 已记录。

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-target\253'
cd G:\gameEngin\rust
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_ui_model
cargo test -p project_e2e_gate c01_from_blank_creation
```

预计成本：10-45 分钟。失败时定位首个真实失败，补入累计失败集；不立即启动完整 workspace。

## 12. Gate I：环境等价预检与最终权威回归

预检：

```text
Windows / PowerShell
独立 CARGO_TARGET_DIR
当前源码 fingerprint 冻结
无共享 B 通道 evidence 输出
无 Provider / network 依赖
当前与待执行施工槽位合同有效
```

最终权威命令：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-target\253-authoritative'
cd G:\gameEngin\rust
cargo test --workspace
cargo test --workspace --all-features
```

预计成本：30-120 分钟。只在预检和受影响域全绿后运行一次。若权威回归失败，回到定向累计失败集，不连续盲跑完整 workspace。

权威回归实测扩展：`architecture_review` loopback Provider 测试在 workspace 中偶发首个响应 `Transport`；生产 Provider 未参与 253。串行化和显式 server-thread readiness 都不能单独消除复现；定向观测确认 fixture 只读取一次 16 KB 请求后就响应并关闭，而 strict-schema POST 可以超过该大小。Windows 在 socket 仍有未读请求体时关闭会产生 RST，使已写出的响应偶发丢失。允许测试 fixture 依据 `Content-Length` 完整排空有界请求体后再响应，并保留显式 readiness；不修改生产 Provider 重试、超时、prompt、schema 或 transport 语义。修复后先跑该定向累计失败集，再重新冻结候选源码 fingerprint，最后补跑 final-source default 与 `--all-features` 权威命令。

本次不声称 exact-commit CI：当前基线含用户未提交的 249-252 实现。权威证据必须绑定最终源码 fingerprint、命令、target root、退出码、测试计数和耗时，不能绑定错误的 `HEAD`。

## 13. 外部审查结论分类

输入：53 号协议和 B8 三引擎正式汇总。

```text
必须修改：
  ProjectIntentWorkflow 不得继续拥有开放式执行顺序。
  ProjectRust Preview 不得静默进入 engine.empty.runtime。

施工约束：
  复用 Candidate / validation / receipt，不删除安全治理。
  Preview、Build 和完整回归分离。
  先定向预检，避免重复完整 workspace。
  保留失败和 repair 历史。

已由历史施工吸收：
  250 Candidate 三类 lowering、SafeProjectPath、rollback。
  251 provider-independent from-blank Gate。
  252 IntentEvent / WorkItem / Diagnosis / Journal。

不适用：
  Unity/UE 的具体护盾实现和 B 通道冻结事件不进入 Engine Core。
  B8 主观排名不作为代码验收。
  本次不重跑三引擎 cohort。
```

判断：正式 253 方案已经完整吸收审查，不需要在施工前修改方案；上述必须修改和施工约束已经进入 Gate C-I。

## 14. 施工自审

用户授权：通过。用户明确要求生成施工文档、自审并开始施工。

方案一致性：通过。本文实现 Tool Kernel、有限授权和 Agent-owned planning，不增加统一 Agent Runner。

Module 深度：通过。外部 Interface 固定为五个操作，Candidate 链、Grant 校验和 receipt lineage 藏在 Implementation。

范围：通过。第一轮只施工 editor_core 纵切和 252 lane 降级，不扩入 Runtime/ECS/Renderer/AUI 重写。

安全：通过。Candidate、validation、approval、SafeProjectPath、receipt、rollback 和 drift fail-closed 均保留。

Preview：通过。Gate D 要求真实 project binding evidence；不以通用 Play 成功冒充 project-owned runtime 正确。

测试分层：通过。Gate A-G 定向、Gate H 受影响域、Gate I default/all-features 权威回归已分层。

混合工作树：已显式处理。记录施工前 hash，使用修改白名单和独立 target；不还原用户改动，不虚报 exact commit。

外部审查：通过。53/B8 已分成必须修改、施工约束、历史吸收和不适用四类。

施工槽位：通过。生成前 `当前/` 与 `待执行/` 均无非 README 文档；激活时本文是唯一当前施工文档。

自审结论：本文没有扩大到通用 Planner、没有跳过最终 Gate、没有与 253 冲突；当时允许从 Gate A 开始施工。

## 15. 完成、记录与归档

全部 Gate 已通过，以下动作均已完成：

1. 生成 `阶段完成记录/2026-07-17-AI-Capability-First-Tool-Kernel-Agent-Owned-Planning-v1/00-总览.md`。
2. 记录各 Gate、源码 fingerprint、权威 run 次数、耗时、失败与遗留项。
3. 更新 240：253 施工状态改为已完成。
4. 更新 49、54、00 文档地图、00 阅读顺序、施工文档 README、阶段完成记录 README。
5. 把本文移动到 `施工文档/已完成/`，状态改为已完成。
6. 清空 `施工文档/当前/`，不得自动激活其它施工。

完成证据见 `../../阶段完成记录/2026-07-17-AI-Capability-First-Tool-Kernel-Agent-Owned-Planning-v1/00-总览.md`。当前没有自动激活后续施工。
