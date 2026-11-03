# 255 当前施工文档：Capability-aware Tool Catalog v1

## 0. 文档状态

```text
正式方案：255-Capability-aware-Tool-Catalog-v1方案.md
方案选择：方案 C
文档状态：已完成激活前复核并激活
施工状态：已完成（第二个 frozen candidate 的单次 C9 权威运行通过；C10 已完成）
当前位置：施工文档/已完成/
当前施工授权：用户已于 2026-07-23 明确授权直接开始 255 施工
结论：255-C0 至 255-C10 已完成并归档，不得再次执行 C9
```

本文件是当前唯一施工文档。用户单独授权、激活前复核、唯一施工槽位与入口同步均已成立；施工只能依次执行 `255-C0` 至 `255-C10`。

255 正式方案、施工文档、254 第 28.3 节纠偏与入口同步随本次激活形成专用 Git 基线。C0 必须在该 clean exact commit 上记录 commit/tree 并建立仓库外隔离 worktree；不得把后续 dirty development tree 当作权威回归真相。

## 1. 施工目标

本施工只实现 255 方案 C：

1. 将当前静态 `AiToolCatalog` 深化为完整可发现的 Catalog v2。
2. 为每个 registered typed Tool 返回 `ready / authorization_required / blocked / unsupported`、结构化 reason、basis 和 `inputDependentChecksRemain`。
3. 保持 `AiToolContractRegistry` 为静态 Tool Contract 唯一真相。
4. 建立只读、纯数据的 availability context，以及 Tool/Platform/Runtime owner 附近的内部 probe。
5. 由 Gateway 提供它真实拥有的 Session authority snapshot，并增加 `accessGeneration / operationGeneration`。
6. 将 `catalogDigest` 固定为静态 Tool Contract digest，将动态状态放入独立 `availabilityDigest`。
7. 在 inspect 和所有 typed invocation 真正读取、排队、mutation 或 spawn 前执行同源 availability 重检。
8. MCP `tools/list` 始终投影完整稳定 typed surface，继续满足 `listChanged=false`。
9. `aife_catalog` 返回 v2 动态 availability；不可用调用返回相同 schema 的结构化 blocker。
10. 在兼容窗口内保留 Catalog v1 descriptors-only 投影，但不把 v1 宣传为动态 availability。

## 2. 明确不施工的内容

```text
不增加 Planner、Workflow、Runner、Goal completion state 或 nextToolIds。
不增加中央 Capability DSL 或功能 feature enum。
不复用 AiToolCapability 表示平台/Runtime/实现支持。
不增加 public per-tool probe trait 或每 Tool 一个公共方法。
不自动 request/approve/renew/revoke/consume Grant。
不把 Catalog snapshot 变成 reservation、lock 或 execute token。
不新增网络探测、SDK 下载、subprocess probe、Build/Preview preflight 或生成文件。
不建立动态插件 ABI、Marketplace 或热插拔 Tool Registry。
不重写 ProjectCandidate、Preview、Build/Delivery、visual diagnostics 的业务 Implementation。
不恢复 ProductionCandidateModule、CodexOutcomeEvaluationModule 或任何 254-R1/R2 lifecycle。
不创建 Fresh Candidate，不执行 F-A/F-B/G、Real Evaluation 或三引擎 B 通道。
```

若施工中发现完成 255 必须改变 253 的 Agent-owned planning、254 第 30 节 scope correction，或需要新增跨工具生命周期，必须停止并先修订正式方案；不得在本文内扩项。

## 3. 当前代码基线

激活前必须重新核对以下事实；当前文档基于 2026-07-23 工作树：

### 3.1 Kernel 与 Registry

```text
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
  AI_TOOL_CATALOG_SCHEMA_VERSION = ai-tool-catalog.v1
  AiToolCatalog { schema_version, tools: Vec<AiToolDescriptor> }
  AiToolContractRegistry::new() -> builtin_descriptors()
  catalog() -> 静态 descriptors
  catalog_for_session() -> 无 active project 时清空，否则全量返回
  descriptor() -> 所有 Tool preconditions 都是 active_project
  start() -> invocation preflight / authorization / queue / operation
```

Catalog、invocation、operation、receipt 与所有 builtin Tool 当前集中在同一大文件。255 可以新增一个 catalog 专属 Module 以集中 schema、聚合、排序和 digest，但不得借机重写全部 Tool Kernel 或机械迁移无关代码。

### 3.2 Gateway authority

```text
rust/crates/ai_tool_gateway/src/core.rs
  ActiveClientSession 已拥有 read_generation、read_grant、read_stale_reason、mutation_state
  grants / access_requests / operation_grants 由 GatewayCore 拥有
  当前缺少 access_generation / operation_generation
  resolve_session_bound_grant() 对普通 read/mutation 和 rollback 分流
  resolve_rollback_grant() 使用 receipt-bound operation/grant lineage
  catalogDigest 当前由整个 session catalog 序列化计算
```

Gateway 是 opaque grant ref、Native Editor approval、Session access 和 operation ownership 的真实 owner。Kernel/Adapter 不得复制这些规则。

### 3.3 MCP 与协议

```text
rust/crates/ai_tool_gateway/src/protocol.rs
rust/crates/ai_tool_gateway/src/remote_adapter.rs
rust/crates/ai_tool_gateway/src/mcp_stdio.rs
rust/crates/ai_tool_gateway/src/named_pipe.rs
```

当前 MCP：

```text
initialize -> tools.listChanged=false
tools/list -> Gateway Catalog -> 4 control tools + registry business tools
aife_catalog -> Gateway Catalog
typed call -> static AiToolContractRegistry decode -> Gateway session-bound dispatch
```

静态 Registry decode 可以保留，但不能代表执行许可；Gateway/Kernel guard 必须覆盖该路径。

### 3.4 跨 crate consumer

```text
rust/crates/editor_host/tests/gateway_process.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs
rust/crates/ai_tool_gateway/tests/explicit_reconnect.rs
rust/crates/ai_tool_gateway/tests/core_scope_contract.rs
```

这些 consumer 必须随 schema migration 一次收敛；禁止只修 `editor_core` 测试后声明完成。

## 4. 目标 Module、Interface 与内部 seam

### 4.1 外部 Interface 不增加方法

253 的 Tool Kernel 外部 Interface 保持：

```text
catalog
inspect
execute/start
observe
cancel
```

255 不增加 `probe_tool / can_execute / resolve_blocker / recommend_next` 等 AI-facing 方法。

### 4.2 新增 catalog 专属深 Module

预计新增：

```text
rust/crates/editor_core/src/ai_tool_catalog.rs
```

该 Module 负责：

```text
Catalog v1/v2 schema 与严格 serde 合同
四态与 reason/category/owner/resolutionKind schema
basis 与 authority snapshot schema
state precedence
reason 稳定排序
catalogDigest / availabilityDigest canonical composition
v1 descriptors-only compatibility projection
v2 entry composition
```

该 Module 只暴露一个供 Tool Kernel 使用的 catalog compose Interface；测试也通过该 Interface 验证行为。不要为测试公开内部 sorter、digest helper 或每 Tool probe。

### 4.3 Tool-owned probe 是内部 seam

Tool-specific probe 采用 crate 内私有函数/小型内部结构，靠近现有真实检查：

```text
project/session 基础事实 -> Tool Kernel / ProjectCandidateEntry 已有 binding 检查
mutation/rollback authority -> Gateway authority snapshot + 现有 grant/receipt lineage owner
Preview/runtime -> Project runtime binding 与 LinkedProjectRuntimeSet 事实
Build/Delivery -> ProjectDeliveryTools / host/platform 已实现能力
operation conflict -> Tool Kernel queue + Gateway operation ownership事实
```

不先建立通用 DSL，不给每个 Tool 注册一组字符串 predicate，不增加 public trait。只有出现至少两个真实 owner consumer 时才提取公共内部 probe。

### 4.4 Availability context 必须是纯数据

`AiToolAvailabilityContext` 或等价内部类型只能包含 engine-owned snapshot：

```text
project identity/digest/state
runtime source/module/interface/binding digest
host/platform/implemented profile facts
Gateway read/mutation authority state and generations
owned operation ids / conflict facts
Tool Kernel queue/operation facts
```

probe 不接收可写 `EditorSession`、filesystem writer、network client、process runner 或 approval controller。Context 构建允许读取已物化状态，但不得写文件、刷新项目、启动进程或改变 Grant。

### 4.5 V1/V2 兼容投影

施工必须提供一个共同 response abstraction，由同一 Registry 生成：

```text
ai-tool-catalog.v1 -> 原 descriptors-only JSON shape
ai-tool-catalog.v2 -> catalogDigest + availabilityDigest + basis + entries
```

要求：

1. 新内部/官方 Adapter 默认协商 v2。
2. `ClientHello.supportedSchemaVersions` 在兼容窗口内同时支持 v2 和 v1，并选择最高共同版本。
3. Session binding 记录实际 negotiated catalog schema，后续请求不能悄悄切换。
4. v1 只由 v2 使用的同一 Registry descriptor 投影，不建立第二份 Tool 表。
5. v1 不包含动态字段，也不声称 ready。
6. 255 全程保持 v1 兼容窗口；只有全部 first-party 与公开支持的 client 都有 v2 negotiation 证据、supported-client matrix 明确移除 v1、并且用户单独批准 schema-breaking 施工后，未来方案才可关闭该窗口。
7. C5 必须测试“共同支持 v2 时选择 v2、仅支持 v1 时保持 descriptors-only、无共同版本时 fail-closed”；未来真正移除 v1 时还必须新增“v1-only client fail-closed”红/绿测试。v1 removal 不属于 255，当前兼容不承诺永久双写。

## 5. 预计涉及文件

### 5.1 必改或预计新增

```text
rust/crates/editor_core/src/ai_tool_catalog.rs                         新增 catalog 深 Module
rust/crates/editor_core/src/ai_capability_tool_kernel.rs              Registry 接入、owner probes、start/inspect guard
rust/crates/editor_core/src/lib.rs                                    受控 re-export
rust/crates/editor_core/src/tests/mod.rs                              新测试 Module 登记
rust/crates/editor_core/src/tests/ai_tool_catalog_tests.rs            新增 Catalog Interface 测试
rust/crates/editor_core/src/tests/ai_capability_tool_kernel_tests.rs   既有 Kernel/Preview/rollback/operation 回归

rust/crates/ai_tool_gateway/src/core.rs                               authority snapshot、generation、dispatch guard
rust/crates/ai_tool_gateway/src/protocol.rs                           v1/v2 negotiation 与 blocker wire schema
rust/crates/ai_tool_gateway/src/remote_adapter.rs                     默认 v2、兼容 v1
rust/crates/ai_tool_gateway/src/mcp_stdio.rs                           稳定 tools/list 与动态 aife_catalog
rust/crates/ai_tool_gateway/src/named_pipe.rs                          transport/real MCP 回归
rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs                 real process typed MCP 合同
rust/crates/ai_tool_gateway/tests/explicit_reconnect.rs                switch/reconnect generation 合同
rust/crates/ai_tool_gateway/tests/core_scope_contract.rs               Core tool surface 保留

rust/crates/editor_host/tests/gateway_process.rs                       Host consumer migration
rust/crates/editor_window_winit/src/tests/native_app.rs                Native Editor consumer migration
rust/crates/project_e2e_gate/src/tests.rs                              Gateway E2E consumer migration
```

### 5.2 仅在红测试证明需要时修改

```text
rust/crates/editor_core/src/project_delivery_tools.rs
rust/crates/editor_core/src/session.rs
rust/crates/engine_runtime/src/project_runtime_module.rs
```

这些文件只能增加/复用只读 owner fact 或窄内部 availability helper。禁止改变 Build/Delivery、RuntimeModule resolve 或 EditorSession 的业务语义。若已有只读事实足够，不得为“架构整齐”修改它们。

### 5.3 默认不得修改

```text
Cargo.toml / Cargo.lock（除非新增依赖不可避免；本方案预计无需新依赖）
legacy/**
项目 fixture/source assets
254-R1/R2 历史方案、历史施工和历史 evidence
Provider、LLM、AUI、Renderer、ECS、RuntimePackage 业务实现
```

出现 `Cargo.toml`、`Cargo.lock`、fixture 或生成清单变更时，候选冻结失效，必须回到 `255-C7` 重新完成受影响域和环境预检。

## 6. 必须保持的不变量

```text
AiToolContractRegistry 是唯一静态 Tool Contract 真相。
所有 registered business Tool 始终出现在 v2 Catalog；无项目也不能清空。
MCP 4 个 control tools + 全部 registry tools 的 typed name/schema 保持稳定。
catalogDigest 不受 project/Grant/runtime/operation 状态影响。
availabilityDigest 不包含时间戳、随机数、秘密、绝对路径或非确定性容器顺序。
reasons 全量保留并稳定排序；state 只按 unsupported > blocked > authorization_required > ready 聚合。
awaiting_user 使用 await_user_decision，不触发第二个 approval request。
rollback 不依赖当前 active mutation grant；exact receipt authority 继续绑定 owning session/operation/grant。
Catalog ready 不创建 operation，不消费 Grant，不写项目，也不保证 execute 成功。
typed direct call 在最新事实不满足时必须在 side effect 前失败。
Adapter 不计算 Tool policy，不过滤掉 unavailable Tool。
AiToolCapability 只表示安全授权，不扩展为功能 feature enum。
```

## 7. 分 Gate 施工

以下 Gate 只有激活后才可执行。每个 Gate 先跑定向测试；失败必须在当前 Gate 收敛，不跨 Gate 累积。

### 255-C0：激活前基线、隔离与 consumer inventory

目标：证明施工起点、命令、consumer 和环境真实存在，不在 dirty tree 上直接开工。

任务：

1. 复核 255 正式方案仍为当前规范，`240` 无更高优先级冲突。
2. 记录 exact commit/tree、branch、`git status`、Rust/Cargo 版本和现有 Catalog schema。
3. 在仓库外创建 fresh exact-commit worktree；使用独立 `CARGO_TARGET_DIR`、`TEMP/TMP` 和 artifact root。
4. 盘点所有 `AiToolCatalog*`、`AI_TOOL_CATALOG_SCHEMA_VERSION`、`GatewayReplyPayload::Catalog` 和 `catalogDigest` consumer。
5. 运行最小旧基线，确认现有测试不是预先失败。
6. 运行最终 Gate 依赖的 canonical lint ledger producer，并结构化确认 committed ledger、diagnostics 与 suppression inventory 无漂移。

命令（激活时在隔离 worktree 执行）：

```powershell
git status --short
git rev-parse HEAD
git rev-parse "HEAD^{tree}"
rustc -Vv
cargo -V
rg -n "AiToolCatalog|AI_TOOL_CATALOG_SCHEMA_VERSION|GatewayReplyPayload::Catalog|catalogDigest" rust/crates
cd rust
cargo test -p editor_core ai_capability_tool_kernel_project_aware_catalog --locked
cargo test -p editor_core ai_capability_tool_kernel_schema_and_catalog_are_small_and_strict --locked
cargo test -p ai_tool_gateway gateway_session_status_connects_with_read_active_and_mutation_not_requested --locked
cargo test -p ai_tool_gateway mcp_projection_tool_list_advertises_typed_catalog_projection_not_raw_execute --locked
cargo run -p quality_gate --locked -- propose-ledger
$lintDiff = Get-Content -Raw 'target/quality-gate/candidate/lint-debt-ledger-diff.v1.json' | ConvertFrom-Json
if (-not $lintDiff.previous_ledger_readable -or
    @($lintDiff.added_diagnostics).Count -ne 0 -or
    @($lintDiff.removed_diagnostics).Count -ne 0 -or
    @($lintDiff.changed_diagnostics).Count -ne 0 -or
    @($lintDiff.new_suppressions).Count -ne 0 -or
    @($lintDiff.expired_entries).Count -ne 0) {
    throw 'activation baseline lint reconciliation changed'
}
```

预计成本：低到中；warm cache 数分钟，cold cache 以实际记录为准。

进入条件：本文已激活；clean exact baseline 与隔离路径已核对。

退出条件：consumer inventory 完整，四个旧基线测试通过，canonical lint/suppression diff 五类均为空，已有失败为零或已单独记录并由用户裁定。

失败动作：基线或 consumer 不符时停止，修订本文并重新自审；不得写 red test 或实现。

执行结果（2026-07-23）：通过。激活基线 commit `6239dd8039c619056db3a6a6f0c53a312f61666b`、tree `03e810a1074628a7b9063ee772b0307579a95b68`；仓库外 detached worktree `G:\gameEngin-255-c0-6239dd8` clean，Rust/Cargo 1.96.0，四个旧基线测试均为 1 passed / 0 failed。既有 `engine_runtime` 三个 dead-code warning 单独记录，不属于 255 新失败。

事后纠偏（2026-07-23）：原 C0 没有运行最终 C9 依赖的 canonical lint ledger reconciliation，因此“通过”只证明四个定向测试，不足以证明最终质量基线有效。该缺口导致 committed ledger 的既有漂移到 C8 才暴露。C0 合同现已补齐；之后任何施工若最终 Gate 消费 committed ledger/suppression/architecture/golden/schema/generated baseline，必须在实现前验证对应真相，不能只记录控制台 warning。

### 255-C1：Catalog v2 与 bypass 合同红测试

目标：先证明当前实现确实违反 255，不先写 production Implementation。

新增测试至少包括：

```text
ai_tool_catalog_v2_schema_is_strict_complete_and_deterministic
ai_tool_catalog_v2_keeps_every_registered_tool_visible_without_project
ai_tool_catalog_v2_separates_contract_and_availability_digests
ai_tool_catalog_v2_reports_all_reasons_with_stable_precedence
gateway_catalog_authority_states_and_generations_are_session_owned
gateway_execute_rechecks_catalog_availability_before_side_effects
gateway_rollback_availability_does_not_require_active_mutation_grant
gateway_catalog_schema_negotiation_selects_v2_and_preserves_v1_window
mcp_catalog_v2_keeps_typed_list_stable_and_returns_dynamic_availability
```

红测试必须分别证明当前错误：

- empty EditorSession Catalog 被清空；
- v1 没有 availability/reason/basis；
- catalog digest 混入 session catalog；
- Gateway 没有 access/operation generation；
- typed MCP decode 可以直接走旧 dispatch，没有公共 availability blocker；
- rollback 不能按普通 active mutation authority 分类。

命令：

```powershell
cd rust
cargo test -p editor_core ai_tool_catalog_v2 --locked
cargo test -p ai_tool_gateway gateway_catalog_authority --locked
cargo test -p ai_tool_gateway gateway_execute_rechecks_catalog_availability --locked
cargo test -p ai_tool_gateway gateway_rollback_availability --locked
cargo test -p ai_tool_gateway mcp_catalog_v2 --locked
```

预计成本：低；只编译/运行定向测试。

进入条件：255-C0 全绿。

退出条件：测试能编译，且只因预期的 255 缺口失败；每个失败断言映射到正式方案条款。

失败动作：若测试因 fixture、路径、旧基线或非 255 问题失败，先修测试设计；不得把非预期失败当红测试证据。

### 255-C2：Catalog 深 Module、schema 与 digest 实现

目标：实现纯 Catalog composition，不接 Gateway mutation authority 或执行 guard。

任务：

1. 新增 `ai_tool_catalog.rs`，建立严格 v1/v2 response、entry、availability、reason、basis schema。
2. v2 entry 使用 `{ descriptor, availability }`，不把动态字段塞回 `AiToolDescriptor`。
3. v1 projection 保持旧 descriptors-only JSON shape。
4. 统一四态 precedence、reason 排序和 null basis 表达。
5. `catalogDigest` 只覆盖 canonical descriptors；`availabilityDigest` 覆盖 catalogDigest、basis 和按 toolId canonical 排序的动态结果。
6. 输出顺序继续以 Registry 稳定顺序为准；digest 不依赖 HashMap/BTreeMap 插入偶然性。
7. unknown schema、unknown required semantics 和 malformed digest fail-closed。

命令：

```powershell
cd rust
cargo test -p editor_core ai_tool_catalog_v2_schema --locked
cargo test -p editor_core ai_tool_catalog_v2_keeps_every_registered_tool_visible_without_project --locked
cargo test -p editor_core ai_tool_catalog_v2_separates_contract_and_availability_digests --locked
cargo test -p editor_core ai_tool_catalog_v2_reports_all_reasons_with_stable_precedence --locked
cargo test -p editor_core ai_capability_tool_kernel_schema_and_catalog_are_small_and_strict --locked
```

预计成本：低到中。

进入条件：255-C1 红测试有效。

退出条件：schema/digest/composition 定向测试全绿；v1/v2 都来自同一 Registry；尚未伪造 Gateway authority。

失败动作：出现第二份 descriptor 表、public helper 膨胀或动态字段污染 descriptor 时停止并重构当前 Gate。

### 255-C3：Engine-owned context、generation 与 Tool-owned probes

目标：把真实 owner 事实接入 Catalog，不建立中央 Capability DSL。

任务：

1. Gateway 为每个 active client session 增加单调 `accessGeneration` 和 `operationGeneration`。
2. access generation 覆盖 request/awaiting/approve/deny/revoke/expire/renew/owner change；无状态变化不得无故增加。
3. operation generation 覆盖 accept、availability-relevant transition、terminal、cleanup/ownership removal；不得使用 wall-clock 替代。
4. Gateway 构造中立 authority snapshot；不按 tool id 决策 readiness。
5. Tool Kernel 从 active project、runtime module、linked runtime、host/platform 和 queue/operation 构造纯 availability context。
6. 按真实 owner 建立 read/inspect、mutation、rollback、Preview/runtime/visual、Build/Delivery 内部 probes。
7. Catalog 缺少具体 input 时用 `inputDependentChecksRemain`，不得猜 receipt、profile、package path 或 evidence ref。
8. 多 blocker 全量返回；read stale / unsupported 等高优先级状态不得被缺授权遮蔽。

最低状态矩阵：

| 场景 | 必须结果 |
|---|---|
| no active project | 全部 registered Tool 仍可见；project-dependent Tool 为 `blocked` |
| read active / mutation not requested | read Tool `ready`；mutation `authorization_required` |
| mutation awaiting user | mutation reason 为 `await_user_decision`；不创建第二请求 |
| active mutation Grant | candidate mutation 可 `ready`，仍保留 input-dependent checks |
| mutation expired/revoked | `authorization_required`，access generation 已变化 |
| read stale | project Tool `blocked`，basis 使用最新 generation |
| runtime module 未绑定/项目绑定不匹配 | Preview/runtime/visual Tool `blocked`；只有当前 build/platform/Host/实现缺失才是 `unsupported` |
| host 不支持 windows-dev | Build/Delivery 为 `unsupported`，不 spawn 探测 |
| operation queue 满 | 受影响 Tool `blocked` + `operation_conflict` |
| rollback 无具体 input | 不因无 active mutation Grant 错判；`inputDependentChecksRemain=true` |

命令：

```powershell
cd rust
cargo test -p editor_core ai_tool_catalog_v2 --locked
cargo test -p editor_core ai_capability_tool_kernel_preview_requires_project_owned_runtime_binding --locked
cargo test -p ai_tool_gateway gateway_catalog_authority --locked
cargo test -p ai_tool_gateway gateway_approval_lifecycle_rotates_mutation_and_cancel_uses_operation_snapshot --locked
cargo test -p ai_tool_gateway gateway_session_access_external_drift_requires_explicit_inspect_refresh --locked
cargo test -p ai_tool_gateway gateway_session_operation_ownership_isolates_observe_and_cancel --locked
```

预计成本：中。

进入条件：255-C2 全绿。

退出条件：状态矩阵、generation 和 digest 变化全部由真实 owner 事实驱动；Catalog 查询没有副作用。

失败动作：若 Gateway 开始维护 tool-id policy 表，或 Kernel 猜 opaque grant ref，视为 ownership 偏离，回退当前 Gate 重新放置 seam。

### 255-C4：Inspect/Execute 同源重检与结构化 blocker

目标：Catalog 不成为软提示；所有 typed 入口在 side effect 前重检。

任务：

1. `project.inspect` 进入同源 project/read availability guard。
2. 普通 typed invocation 在 queue/operation record/mutation/spawn 前用最新 context 重检。
3. Gateway 使用同一 authority owner 检查当前 read/mutation state；Kernel 复用 Tool owner probe 检查 project/runtime/platform/operation。
4. rollback 继续由 `resolve_rollback_grant()` 的 exact receipt/operation/grant lineage 决定，不要求 active mutation Grant。
5. `AiToolDiagnostic` / `GatewayDiagnostic` 或等价 rejection envelope 增加可选的同 schema `availability` blocker；非 availability 错误保持原诊断兼容。
6. Catalog ready 后发生 project/access/runtime/operation drift 时，调用必须返回最新 basis 和 blocker，不创建 operation、receipt 或项目写入。
7. invocation input 的具体 schema、receipt、path、profile 和 digest 校验继续保留；availability 不吞掉更精确的 input diagnostic。

定向场景：

```text
catalog ready -> external project drift -> typed call blocked before operation
catalog mutation ready -> Grant revoke/expire -> typed call authorization_required
catalog preview ready -> runtime binding change -> typed call blocked
valid rollback receipt + expired forward Grant -> rollback authority preserved
wrong-session/tampered rollback receipt -> structured authority blocker
queue state change after Catalog -> latest operationGeneration returned
```

命令：

```powershell
cd rust
cargo test -p ai_tool_gateway gateway_execute_rechecks_catalog_availability --locked
cargo test -p ai_tool_gateway gateway_expired_forward_grant_preserves_receipt_bound_rollback_authority --locked
cargo test -p ai_tool_gateway gateway_bypass_drift_rejects_external_write_without_fake_receipt --locked
cargo test -p ai_tool_gateway gateway_concurrent_writer_allows_only_first_same_digest_candidate --locked
cargo test -p ai_tool_gateway gateway_invocation_replay_returns_same_result_and_rejects_changed_content --locked
cargo test -p editor_core ai_capability_tool_kernel_grant_rejects_tamper_expiry_and_domain_escalation --locked
cargo test -p editor_core ai_capability_tool_kernel_preview_requires_project_owned_runtime_binding --locked
```

预计成本：中。

进入条件：255-C3 全绿。

退出条件：所有 stale-ready/bypass 场景在 side effect 前拒绝；rollback lineage 保持；rejection 含同结构 blocker。

失败动作：若为通过测试而只在 MCP 层过滤或重复 Tool policy，必须删除重复并回到 Gateway/Kernel owner seam。

### 255-C5：Gateway schema negotiation 与 MCP/Adapter 稳定投影

目标：对外提供动态 Catalog，同时不让 MCP typed surface 抖动。

任务：

1. 增加 `ai-tool-catalog.v2`，保留 v1 compatibility projection。
2. Client/Gateway 协商实际 schema 并写入 Session binding；错误版本 fail-closed。
3. Session binding `catalogDigest` 改为静态 Registry digest，不再对 session-specific Catalog 整体求 digest。
4. `aife_catalog` 默认返回 v2 availability。
5. `tools/list` 从 v1/v2 response abstraction 读取同一 descriptors，始终包含 4 control tools 与全部 registry tools。
6. 保持 `tools.listChanged=false`；project/Grant/operation 变化只改变 `aife_catalog`，不增删 tool name/schema。
7. typed call 的 `structuredContent` 在不可用时包含 availability blocker，`isError=true`。
8. CLI/MCP/Editor/Test Adapter 不重算 readiness。

命令：

```powershell
cd rust
cargo test -p ai_tool_gateway mcp_catalog_v2 --locked
cargo test -p ai_tool_gateway gateway_catalog_schema_negotiation --locked
cargo test -p ai_tool_gateway mcp_projection_tool_list_advertises_typed_catalog_projection_not_raw_execute --locked
cargo test -p ai_tool_gateway mcp_typed_tool_projection_uses_registry_direct_input_schemas --locked
cargo test -p ai_tool_gateway adapter_equivalence_preserves_catalog_and_frozen_invocation_identity --locked
cargo test -p ai_tool_gateway gateway_named_pipe_uses_current_user_acl_and_owner_thread_dispatch --locked
cargo test -p ai_tool_gateway --test mcp_process_smoke --locked
cargo test -p editor_host --test gateway_process --locked
```

预计成本：中；real process smoke 可能受 cold binary build 影响。

进入条件：255-C4 全绿。

退出条件：v1/v2 negotiation、stable tools/list、dynamic aife_catalog、real named-pipe/MCP process 全绿。

失败动作：若 v1 compatibility 需要第二 Registry 或 permanent shim Module，停止并修订迁移实现；不得牺牲单一真相。

### 255-C6：高风险负向矩阵、无副作用与性能

目标：验证 255 不会为了“更聪明的 Catalog”增加新副作用、审批重复或热路径成本。

必须覆盖：

```text
unknown schema / unknown Tool / malformed reason owner -> fail-closed
v2+v1 client 选择 v2；v1-only client 得到 descriptors-only；无共同 schema fail-closed
caller direct input 含 platformAvailable/runtimeAvailable/grantActive 等内部字段 -> reject
multiple reasons precedence 与稳定排序
相同 descriptors + 不同 Session -> catalogDigest 相同
project/access/runtime/operation generation 变化 -> availabilityDigest 变化
相同 basis 重复 Catalog -> byte/digest deterministic
Catalog 前后 project tree/digest、operation count、access request count、Grant budget 不变
awaiting_user 重复 Catalog -> approval request count 不变
Catalog/probe 不生成 Library/Temp/report，不启动 Preview/Build/Player，不访问网络
warm/cold Catalog 仍满足现有 control-plane performance contract，不触发完整项目扫描
project switch/reconnect 后旧 Session 不能复用 availability
```

无副作用必须主要由 Interface 形状保证：probe 只接收 pure snapshot；测试中的 filesystem/operation/access/grant 前后对比是补充证据，不用脆弱的源码字符串扫描代替设计。

命令：

```powershell
cd rust
cargo test -p editor_core ai_tool_catalog_probe_is_side_effect_free --locked
cargo test -p editor_core ai_tool_catalog_v2 --locked
cargo test -p ai_tool_gateway gateway_catalog_no_side_effects --locked
cargo test -p ai_tool_gateway gateway_performance_contract_measures_control_plane_without_build_side_effects --locked
cargo test -p ai_tool_gateway gateway_approval_lifecycle_prunes_close_ttl_switch_and_rejects_stale_decision --locked
cargo test -p ai_tool_gateway gateway_disconnect_reconnect_does_not_transfer_operation_ownership --locked
cargo test -p ai_tool_gateway --test explicit_reconnect --locked
```

预计成本：中。

进入条件：255-C5 全绿。

退出条件：全部负向、side-effect、determinism、reconnect 和 performance 合同通过。

失败动作：新失败加入累计失败集；先在定向层修复并重跑 C1-C6 累计失败集，不启动完整 workspace。

### 255-C7：受影响域回归

目标：覆盖共享 schema 的全部跨 crate consumer。

命令：

```powershell
cd rust
cargo test -p editor_core --locked
cargo test -p ai_tool_gateway --locked
cargo test -p editor_host --locked
cargo test -p editor_window_winit --locked
cargo test -p project_e2e_gate --locked
```

预计成本：高；cold cache 与真实进程测试按实际记录，不用历史平均值设置 kill deadline。

进入条件：255-C1 至 C6 定向与累计失败集全绿。

退出条件：五个受影响 crate 全绿；测试计数与 ignored 清单记录；无未解释新增 warning/ignore/suppression。

失败动作：以首个真实失败为准，回到对应 Gate；修复后先重跑累计失败集，再重跑受影响 crate，不直接启动最终权威回归。

执行结果（2026-07-23）：通过。C1 红测试只因 Catalog v1、缺少动态 availability/generation/execute blocker 等预期 255 缺口失败；C2-C6 已完成 v1/v2 strict projection、静态/动态 digest、owner-owned context、access/operation generation、execute-time recheck、结构化 blocker、稳定 MCP projection、无副作用和 reconnect 负向矩阵。C7 结果为：`editor_core` 548 passed / 2 ignored，`ai_tool_gateway` 57 passed / 0 ignored（含 lib、core scope、explicit reconnect、MCP process），`editor_host` 21 passed / 0 ignored，`editor_window_winit` 119 passed / 0 ignored，`project_e2e_gate` 65 passed / 1 ignored。ignored 项均为既有显式 local/provider gate；既有 `engine_runtime` 三个 dead-code warning 和 `editor_window_winit` 既有 dead-code warning 未新增 suppression。E2E 改写的 tracked `samples/complex_shooter_project/Build/Windows/dev` 生成物已按 cleanup 合同精确恢复，未进入候选。

### 255-C8：环境等价预检与候选冻结

目标：在 final Local CI 前一次性证明 exact candidate、路径、工具链和高风险矩阵可重现。

环境合同：

```text
Windows 真实施工环境
Rust 1.96.0 / rust-toolchain.toml
仓库外 fresh exact-commit worktree
clean source state
独立且路径长度受控的 CARGO_TARGET_DIR
独立 TEMP/TMP 与 artifact root
Git text normalization 与工作树换行状态已记录
无共享 target/fixture/pipe/artifact 路径
子进程继承同一 CARGO_TARGET_DIR/TEMP/TMP
记录所有相关卷 free bytes；容量覆盖预计并发峰值，并额外保留至少 20% 且 10 GiB cleanup/report 余量
保存结构化证据后清理已完成 C7/C8 的可再生 Cargo target，或将 C9 target 放到容量充足的独立卷
```

命令：

```powershell
git status --short
git diff --check
git rev-parse HEAD
git rev-parse "HEAD^{tree}"
cd rust
cargo fmt --all -- --check
cargo run -p quality_gate --locked -- propose-ledger
$lintDiff = Get-Content -Raw 'target/quality-gate/candidate/lint-debt-ledger-diff.v1.json' | ConvertFrom-Json
if (-not $lintDiff.previous_ledger_readable -or
    @($lintDiff.added_diagnostics).Count -ne 0 -or
    @($lintDiff.removed_diagnostics).Count -ne 0 -or
    @($lintDiff.changed_diagnostics).Count -ne 0 -or
    @($lintDiff.new_suppressions).Count -ne 0 -or
    @($lintDiff.expired_entries).Count -ne 0) {
    throw 'lint ledger reconciliation changed'
}
cargo test -p editor_core ai_tool_catalog_v2 --locked
cargo test -p ai_tool_gateway gateway_catalog --locked
cargo test -p ai_tool_gateway mcp_catalog_v2 --locked
cargo test -p ai_tool_gateway gateway_execute_rechecks_catalog_availability --locked
cargo test -p ai_tool_gateway --test mcp_process_smoke --locked
cargo test -p editor_host --test gateway_process --locked
cargo test -p editor_window_winit --locked
cargo test -p project_e2e_gate --locked
```

预计成本：高，但低于完整 Local CI；只在 C7 全绿后执行一次完整预检。

进入条件：C7 全绿；所有累计失败已纳入命令；代码、manifest、lockfile 和 tests 已冻结。

退出条件：clean exact-commit 环境全绿；lint candidate 只写入隔离 target，committed ledger 不变，`previous_ledger_readable=true` 且五类 diff 均为空；记录 commit/tree、环境根、日志 digest、test counts 和 cleanup 计划；候选冻结。

冻结规则：除完成记录与入口同步外，Rust、Cargo、tests、fixture、ledger 或生成清单任何变化都使 C8 失效，必须重新执行 C7/C8。

失败动作：不启动 C9；在同一隔离环境定位首个失败，回到相应 Gate 收敛。

首个 C8 执行记录（2026-07-23）：candidate commit `3d6117994a9e41d16f6be7bb4a6172f8bacb6b85`、tree `155d7637ff63e5a8f6e82eacc2c685055418fa4a` 在 `G:\gameEngin-255-c8-3d61179` 执行原命令 `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`，因 `engine_input` 2 条、`editor_ui_model` 13 条既有 warning 失败。同样 15 条在激活基线 `6239dd8039c619056db3a6a6f0c53a312f61666b` 精确复现，证明不是 255 引入。原命令绕过 245 已建立的 lint debt ledger 合同，属于施工文档错误，不是实现失败；不修复无关债务，不新增 runner/suppression。该 candidate 永久废弃，不得原地重试或冒充 C8 通过；本次文档修订提交后，必须使用新的 exact candidate、仓库外 detached worktree、独立 target/TEMP/TMP/artifact root 从头执行 C8。C9 authoritative run count 仍为 0。

修订后 fresh C8 执行记录（2026-07-23）：candidate commit `fa696e4b1ea79833577d000af80f320c4dad63bc`、tree `8a086c497f4b7620381464b4cf1f94325379601e`，detached worktree `G:\gameEngin-255-c8-fa696e4`，隔离根 `G:\EngineTest\255-catalog\c8-fa696e4`。clean、`git diff --check`、fmt 与 `propose-ledger` producer 均成功，但 canonical diff 为 `added_diagnostics=32`、`removed_diagnostics=1`、`changed_diagnostics=1`、`new_suppressions=4`、`expired_entries=0`，因此按退出条件失败并 fail-fast，后续测试矩阵及 C9 均未执行。对激活基线 `6239dd8039c619056db3a6a6f0c53a312f61666b` 运行同一 producer 得到完全相同的 counts、candidate ledger SHA-256 `6b76010573b811b43d9e7c5a12049f8b5f0cbabefbd966a0ba74843b898514c4` 与 diff SHA-256 `33d0f240b3a160a20ed34be7aec0e699c217e4b741a7f0a783e3f6c303d66237`；255 相对激活基线新增 lint/suppression 为 0，真实阻塞是 committed ledger SHA-256 `16f19a44fc22b237f7e7b28aeb185da05df1638486511cbac00743c8b66730c1` 在 255 激活前已经落后于 observed workspace。不得用基线等值替代 canonical ledger Gate，也不得直接运行必然失败的 C9；解除阻塞需要单独审查并授权 245 lint ledger maintenance，之后形成新的 exact candidate 并重新执行 C8。

245 lint ledger maintenance（2026-07-23，用户已单独授权）：逐项审查 canonical diff。`runtime_cli` fixture 的 `clippy::zombie_processes` 是真实自然退出路径回收缺口，已通过持有 grandchild handle 并在自然路径 `kill + wait` 修复；`runtime_cli --test bounded_child_process` 7 passed，目标 binary `--no-deps -D warnings` 通过。其余 31 条为结构/复杂度或表达式维护债务；4 个 suppression 均为具体函数上的 `too_many_arguments`，无 crate-wide allow，`expired_entries=0`。使用现有 `propose-ledger` 生成 168-entry / 371-occurrence / 11-suppression candidate，经审查后机械落盘 committed ledger；未新增 runner、Module、lint override 或自动 apply 路径。ledger 或源码变化使旧 candidate 继续失效，必须形成新 exact candidate 并从 C7/C8 重跑。

修复后 C7/C8（2026-07-23）：exact candidate commit `71496ae0fa6ffd4a9d5f8a827fb3464e0c35c098`、tree `130bdefd4625cca35b355f7d9fb70b2fc5d8d7e5`。fresh C7 在 `G:\gameEngin-255-c7-71496ae` 全绿：`editor_core` 548 passed / 2 ignored、`ai_tool_gateway` 合计 57 passed、`editor_host` 合计 21 passed、`editor_window_winit` 119 passed、`project_e2e_gate` 合计 64 passed / 1 ignored，耗时 741 秒。fresh C8 在 `G:\gameEngin-255-c8-71496ae` 全绿：ledger 168 entries / 371 occurrences / 11 suppressions，五类 diff 均为 0；全部定向、process、Native Window 与 E2E 矩阵通过，耗时 631 秒；测试生成的 `Library/ProjectIntent/journal.json` 已删除，source clean，candidate 冻结。

### 255-C9：一次最终权威 Local CI

目标：对 C8 同一 frozen exact commit 运行一次 authoritative Local CI，统一覆盖 trusted exact commit、default/all-features workspace、lint、artifact 与 cleanup。

唯一命令：

```powershell
cd rust
cargo run -p quality_gate --locked -- local-ci --commit HEAD
```

预计成本：最高；外层 deadline 必须覆盖全部 stage timeout、detached worktree、artifact 和 cleanup 余量，不使用历史平均耗时作为 kill deadline。

进入条件：C8 对同一 exact commit 全绿并冻结；source worktree clean；用户已授权本施工及最终权威运行。

权威证据必须包含：

```text
resolved commit/tree 与 requested HEAD 一致
detached source workspace_state=clean
default/all-features、fmt、clippy、architecture/suppression 等 Local CI stage 全绿
每 stage start/end/exit code/duration/log digest
artifact digest 与 report schema 有效
source/staged/nested worktree clean
detached worktree、TEMP 和可清理 artifact 按合同 cleanup
authoritative run count = 1
```

退出条件：Local CI `passed=true` 且 cleanup 完整。

失败动作：保留结构化 artifact，以首个真实失败为准；不得在 failed candidate 上补跑第二次权威运行。修复必须回到 C7/C8 形成新 frozen commit，第二次 C9 需用户另行明确授权，并在完成记录解释为什么预检未捕获。

首次 C9 执行结果（2026-07-23）：failed，authoritative run count `1`，run id `local-71496ae0fa6f-1784779299`，duration `263589 ms`，source clean、exact commit 正确、isolated worktree cleanup=`removed`。外层报告最初只见 0 字节 quality report/EOF；随后只运行非 authoritative 的同源 `quality_gate verify` 诊断，确认 toolchain、metadata、format、suppression、Clippy 与 ledger 全部通过，真实首因是 G: free bytes=`0`，`default_workspace`、`all_features_workspace`、`code_hygiene` 与最终 report write 均因 Windows `os error 112` 失败。证据：`../../阶段完成记录/2026-07-23-Capability-aware-Tool-Catalog-v1/artifacts/255-C9-first-authoritative-failure-report-v1.json`、`../../阶段完成记录/2026-07-23-Capability-aware-Tool-Catalog-v1/artifacts/255-C9-diagnostic-quality-failure-v1.json`。已先保存证据，再用 `cargo clean --target-dir` 只清理可再生的 255 C7/C8/C9 Cargo targets，G: 恢复 `42.42 GiB`；未删除源码或用户目录。该 candidate 不得重跑 C9；容量合同修订后必须形成新 exact candidate、fresh C7/C8，第二次 C9 需要用户另行明确授权。

容量修订后的 fresh preflight（2026-07-23）：新 exact candidate commit `447c3810ddbf329cc7c81d5667dc390c23725504`、tree `bb44a11753b503ae1c3f412b1329e2b784116b72`。C7 使用 `H:\gameEngin-255-c7-447c381` 与独立 `H:\EngineTest\255-catalog\c7-447c381`：首次 `editor_core` 完整并行执行出现一次 `build_service_release_profile_icon_picker_saves_with_source_guard` 偶发失败；该 Rust tree 与已通过 candidate 相同，单测随后 20/20 压力通过，完整并行 `editor_core` 以 548 passed / 2 ignored 通过，未形成可复现代码缺陷，未修改 production/test 代码。其余 C7 为 Gateway 57、Host 21、Window 119、E2E 65 passed / 1 ignored，全绿。

C8 使用全新 `H:\gameEngin-255-c8-447c381` 与独立 `H:\EngineTest\255-catalog\c8-447c381`。source clean、exact commit/tree、fmt 全部成立；repository-owned `propose-ledger` 为 168 entries / 371 occurrences / 11 suppressions，`previous_ledger_readable=true` 且 added/removed/changed/new suppression/expired 五类 diff 全 0。第一次矩阵包装因 PowerShell 函数输出与布尔返回混用而不能作为退出码证据，未用于冻结；修正 wrapper 后全部命令逐项 exit 0：Catalog 5、Gateway 定向 3+1+1、MCP process 2、Host process 2、Window 119、E2E 65 passed / 1 ignored。两个 source worktree cleanup 后 clean；C7/C8 target 分别约 17.5 GiB / 18.1 GiB，保存证据后用 `cargo clean --target-dir` 清理，H: 恢复 `566.16 GiB`。结构化证据：`../../阶段完成记录/2026-07-23-Capability-aware-Tool-Catalog-v1/artifacts/255-C7-C8-fresh-preflight-report-v1.json`。candidate 已冻结。

第二次 C9 执行结果（2026-07-23）：用户单独授权在 G: 对新 frozen candidate 执行第二次且仅一次 C9。exact commit `447c3810ddbf329cc7c81d5667dc390c23725504`、tree `bb44a11753b503ae1c3f412b1329e2b784116b72`，fresh source `G:\gameEngin-255-c9-447c381`，隔离根 `G:\EngineTest\255-catalog\c9-447c381`。run id `local-447c3810ddbf-1784787443`，耗时 `2091540 ms`，`passed=true`；12 个 stage 全部 `status=passed`，source/isolated workspace 均 clean，diagnostics 为空，nested worktree cleanup=`removed`。run report SHA-256 `b7136694225dee15be7fadacff372b46aba04852af9a7475f89523ba43b318cb`；quality report SHA-256 与报告 digest 均为 `3116d08ad401d22159ff5dbbb839ffb1879c921e529cd4e0770d69151705667f`。该授权已消费，禁止再次执行 C9。

### 255-C10：完成记录、入口同步与施工归档

只有 C9 通过后执行：

1. 生成 `阶段完成记录/2026-XX-XX-Capability-aware-Tool-Catalog-v1/00-总览.md`。
2. 记录每 Gate 命令、结果、测试计数、耗时、失败收敛、权威 run 次数、artifact digest、cleanup 和遗留风险。
3. 将 255 正式方案、`240`、`49`、`54`、`00-文档地图`、`00-阅读顺序`、根 `README.md` 同步为施工完成。
4. 更新 `施工文档/README.md` 与 `阶段完成记录/README.md`。
5. 将本文从 `施工文档/当前/` 移到 `施工文档/已完成/`。
6. 不自动开始下一系统，不运行真实 AI acceptance、历史 R1/R2 Gate 或三引擎 B 通道。

执行结果（2026-07-23）：已生成阶段完成记录并固化首次失败、fresh C7/C8、第二次权威 run 与 quality report；正式方案、240、49、54、文档地图、阅读顺序、根 README、施工与完成记录索引已同步；本文已归档到 `施工文档/已完成/`。C10 只修改文档与 evidence，没有修改 frozen Rust candidate。

预计成本：低；只做文档、索引和归档，不改 frozen Rust candidate。

## 8. 三层验证摘要

| 层级 | Gate | 目的 | 进入条件 | 失败后动作 |
|---|---|---|---|---|
| 定向测试 | C1-C6 | schema、owner、guard、MCP、负向与副作用 | 上一 Gate 全绿 | 当前 Gate 精确修复并维护累计失败集 |
| 受影响域 | C7 | editor_core/Gateway/Host/Native Editor/E2E consumer | 所有定向测试全绿 | 回到责任 Gate，禁止直接完整重跑 |
| 最终权威 | C8-C9 | clean exact commit、环境等价、default/all-features 与 cleanup | 受影响域全绿且候选冻结 | 保留 artifact，重新预检；权威重跑需新授权 |

定向测试和 C8 preflight 不能冒充 C9；C9 也不能替代各 Gate 的局部失败定位证据。

## 9. 高风险验证清单

激活后逐项维护状态，不得在施工文档生成阶段预先勾选：

```text
[x] Catalog v2 schema strict，unknown fields/versions fail-closed。
[x] 无 active project 时 registered Tool 全部可见。
[x] v1 descriptors-only compatibility 来自同一 Registry。
[x] v1 compatibility window 的保留、协商与未来退出条件已有测试和显式授权边界。
[x] catalogDigest 静态；availabilityDigest 动态且 deterministic。
[x] reason category/owner/resolutionKind/state precedence 正确。
[x] awaiting_user 不创建第二 approval request。
[x] accessGeneration 覆盖所有真实 authority transition。
[x] operationGeneration 覆盖 availability-relevant operation transition。
[x] rollback receipt-bound authority 不依赖 active mutation Grant。
[x] stale Catalog 不能绕过 inspect/execute guard。
[x] rejected invocation 在 operation/side effect 前返回同结构 blocker。
[x] Preview/runtime binding mismatch 被 owner probe 正确解释。
[x] Build/Delivery unsupported 不启动 subprocess 探测。
[x] Catalog repeated call 不写文件、不联网、不 spawn、不消费 Grant。
[x] MCP tools/list 名称/schema 稳定且 listChanged=false。
[x] aife_catalog、CLI、Editor、Test Adapter availability 等价。
[x] editor_host、editor_window_winit、project_e2e_gate consumer 已迁移。
[x] 无新增 Planner/Runner/Capability DSL/public per-tool probe Interface。
[x] 无 Cargo/fixture/legacy/R1/R2 非范围变更。
[x] C8 clean exact-commit 预检全绿。
[x] 每个 frozen candidate 的 C9 authoritative Local CI 只运行一次；第二个 candidate 通过且 cleanup 完整。
```

## 10. 失败收敛规则

- C0 基线失败：停止并先区分已有失败、环境问题和文档漂移。
- C1 红测试出现非预期失败：修测试设计，不修改 production 代码掩盖。
- 同类 availability 规则第二次复制：全仓搜索并收敛到真实 owner 或共享内部 helper。
- Gateway 出现 tool-id policy 表：视为第二真相，回退 C3。
- Adapter 自行组合 readiness：视为 bypass，回退 C4/C5。
- 为性能引入陈旧 cache：必须证明完整 basis key 与 execute recheck，否则删除 cache。
- v1 compatibility 需要复制 Registry：停止并重做 projection，不接受双真相。
- C7 新失败加入累计失败集；先定向、再 affected crate。
- C8 失败不运行 C9；代码变化使冻结失效。
- C9 失败保留 artifact，不猜修复，不自动重跑。
- 任一失败都不授权 Fresh Candidate、F-A/F-B/G、Real Evaluation 或三引擎 B 通道。

## 11. 文档同步边界

### 11.1 生成/自审阶段的历史状态（已完成）

只允许同步为：

```text
讨论状态：已完成
施工状态：待执行
当前施工：无
待执行施工：施工文档/待执行/255-当前可自动化施工文档-Capability-aware-Tool-Catalog-v1.md
明确：不可施工，等待激活前复核与用户授权
```

不得写“施工中”“当前可施工”“Gate 已运行”或“实现完成”。

### 11.2 激活状态（已完成）

复核通过后本文已移动到 `施工文档/当前/`，`54` 已同步为唯一施工入口。专用激活 commit 是 C0 要记录的 exact baseline；移动本身不等于 C0 已通过，仍须实际执行 C0。

### 11.3 完成时

只有 C0-C9 全部通过才执行 C10，写完成记录并归档。

## 12. 施工文档自审

### 12.1 与 255 正式方案一致性

通过。本文逐项覆盖完整目录、四态、Tool-owned probe、结构化 reason/basis、静态/动态 digest、execute-time recheck、MCP 稳定投影、无副作用、v1 兼容窗口及退出条件、兼容迁移和三层验证。没有修改 255 的目标或非目标。

### 12.2 正式方案是否仍需修改

无需修改。代码复核确认 255 所列七个核心缺口仍真实存在；施工只把正式方案具体化为文件、Gate 和测试，没有发现需要推翻方案 C 的新证据。

### 12.3 是否扩大范围

未扩大。预计改动限定在 Catalog/Tool Kernel/Gateway/Adapter 与直接 consumer。`project_delivery_tools.rs`、`session.rs`、`project_runtime_module.rs` 只有红测试证明缺少 owner fact 时才可窄改；Cargo、fixture、Provider、Runtime 业务和历史 lifecycle 默认禁止修改。

### 12.4 是否限制 AI 或形成菜谱

不会。C0-C10 是一次性代码施工顺序，不是产品运行 Interface。最终 Catalog 不返回步骤、nextToolIds 或目标完成判断；AI 仍自由选择工具和顺序。

### 12.5 Module 是否足够深

通过。AI-facing Kernel Interface 不增加方法；Catalog 专属 Module 通过一个 compose Interface 隐藏 schema、兼容、聚合、排序和 digest。Tool-specific probe 留在内部 owner seam，不为每个 Tool 新增公共方法。按 deletion test，删除该 Module 后复杂度会重新散落到 Kernel/Gateway/MCP/consumer，而不是简单消失。

### 12.6 是否形成第二套能力真相

否。Registry 只拥有静态 contract；Gateway 只拥有 Session authority；Tool/Runtime/Platform owner 只拥有各自 readiness；Catalog 只聚合。v1 是从同一 descriptors 生成的只读 projection，不是第二 Registry。

### 12.7 rollback 与授权是否被粗暴分类

没有。本文专门要求 rollback 使用 exact receipt-bound operation/grant lineage，并把“无具体 input”标成 input-dependent；不能按 active mutation Grant 或 `requiredCapabilities` 粗判。

### 12.8 验证层次与成本是否合理

合理。C1-C6 是便宜定向层，C7 才跑五个 affected crates，C8 在 clean exact commit 做环境等价预检，C9 使用唯一 `quality_gate local-ci` 覆盖最终完整矩阵。完整 workspace 不作为逐错调试循环。

### 12.9 环境敏感点是否完整

已覆盖 clean/exact commit、仓库外 worktree、独立 `CARGO_TARGET_DIR`、TEMP/TMP/artifact root、Windows、Rust 1.96.0、换行状态、子进程环境、cache warm/cold、日志 digest 和 cleanup。Catalog 本身不得新增外部环境依赖。

### 12.10 外部审查结论

本轮没有用户提供新的其它 AI 审查文档。255 正式方案已吸收 Unity/Unreal/Godot/Bevy 源码对照；本文只继承“注册与 readiness 分离、owner-local check、execute recheck”的施工约束，不重复扩大外部引擎研究。

### 12.11 是否越权施工

没有。用户已明确授权直接开始 255；激活前复核确认方案、优先级、唯一施工槽位、代码基线和测试入口均成立，本文已移动到 `当前/`。当前已按授权依次完成 C0-C7，尚未生成完成记录，且未触碰任何历史 R1/R2/B 通道。

自审结论：

```text
方案一致性：通过
范围控制：通过
深 Module / ownership：通过
AI 自由度：通过
rollback/authority：通过
测试分层：通过
环境合同：通过
入口状态：已完成并归档（authoritative run 总数=2，每个 frozen candidate 各一次；第二个 candidate 通过）
```

## 13. 激活前复核结果

2026-07-23 激活时逐项确认：

```text
[x] 用户明确授权激活 255，而不只是询问下一步。
[x] 255 正式方案仍是当前有效方案，240 状态无冲突。
[x] 施工文档/当前/ 没有其它 active 非 README 施工文档；其中仅保留历史 evidence artifact。
[x] 本文激活前位于待执行/，且未被其它方案替代。
[x] 专用激活 commit 将包含 255 正式方案、本文、254 纠偏与入口同步。
[x] dirty changes 已确认全部属于该文档链路；施工使用 clean exact commit 隔离。
[x] C0 中所有路径、命令、test filter 和 consumer 仍存在。
[x] Cargo.toml/Cargo.lock 与 Rust 源码无未提交漂移；Catalog v1、Gateway protocol 和 Tool Registry 与文档基线一致。
[x] 没有恢复 254-R1/R2 lifecycle、Fresh Candidate 或 B 通道的授权。
[x] 复核结果已回填本文并重新完成自审。
```

以下为 2026-07-23 激活时的历史记录；当时本文已移动到：

```text
施工文档/当前/255-当前可自动化施工文档-Capability-aware-Tool-Catalog-v1.md（历史激活路径）
```

并同步 `54`。施工结束后本文已归档到 `施工文档/已完成/`，当前入口不再指向上述历史路径。
