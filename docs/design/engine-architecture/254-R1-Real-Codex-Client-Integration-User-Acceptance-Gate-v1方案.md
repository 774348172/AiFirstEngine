# 254-R1 - Real Codex Client Integration / User Acceptance Gate v1 方案

> 状态：`superseded_by_254-R2`；仅保留为历史诊断与决策证据，不再是当前规范、施工授权或可执行 Gate  
> 原方案日期：2026-07-17  
> Gate F 复核与修订日期：2026-07-19  
> Gate F 第二次复核与修订日期：2026-07-20  
> Fresh E-R 冻结合同复核与第三次修订日期：2026-07-20  
> 原上游：`254-AI-Tool-Gateway-Codex-Adapter-v1方案.md` 旧第 30 节  
> 替代方案：`254-R2-Production-Conformance-Candidate-Lifecycle-Real-Codex-Outcome-Acceptance-v1方案.md`  
> 性质：历史方案；正文保留当时设计、失败与已完成施工事实，不得据此运行 freezer、创建 candidate、执行 F-A/F-B/G 或生成新的 R1 施工

## 0. 替代声明（2026-07-20）

本方案曾用于修复 typed MCP、session access、approval lifecycle、exact presented-frame evidence、reconnect、production freezer 和失败原子性；这些有效能力事实由 254 Core 第 30 节与 254-R2 选择性继承。R1 随后把真实 Codex acceptance 收敛为固定 action allowlist、exact count/order、frozen direct input 和单次批准剧本，导致合法额外读取被判失败、candidate 身份与 activation/config/plan 发生循环绑定，并使一次性运行承担工具一致性、发布、人工协作和开放目标验收的全部风险。

因此从本声明起：

```text
R1 全文只作历史追溯，不再具有规范优先级。
R1 的 frozen plan、F-A/F-B、R5/R6、candidate/freezer 合同不得继续补丁式修订或执行。
所有历史 candidate、activation、attempt、receipt 和失败 artifact 保持原终态，不得复用或拼接。
新的实现只能从 254-R2 另行生成并确认施工文档；本方案不能作为“开始施工”的入口。
```

历史正文中出现的“当前”“必须”“下一步”“通过”等措辞均按其记录日期解释，不代表替代声明后的当前状态。历史失败不得被改写为成功。

## 1. 决策

首次真实 Codex Gate F 没有证明“只差人工点击”，而是暴露了当前 MCP Interface、session access、审批 UI、Preview evidence 和 reconnect 合同的结构性缺口。旧流程继续重试只会重复创建 session、重复等待批准或在错误 fixture 上失败，不能产生有效验收。

采用 `Session-aware Catalog Projection Typed MCP Surface`：

```text
AiToolContractRegistry（唯一 tool contract）
  -> McpToolProjection（逐工具严格 MCP tools）
  -> Gateway session access/status（内部 Read + 用户批准 mutation）
  -> AI Capability Tool Kernel（能力、mutation、receipt、operation 真相）
  -> EditorSession / RuntimePackage / GameView / Build / Delivery
```

真实客户端不再构造完整 `AiToolInvocation`，不再选择 `$active`，不再手抄 screenshot/frame digest。Gateway/Adapter 从 Catalog 与 session/project binding 补齐内部 envelope。mutation 仍只在 Native Editor 一次明确用户批准后执行。

在新的自动化预检、candidate freeze 和一次 F-A/F-B 完成前，准确状态是：

```text
Gate A-D implementation facts：retained；旧 binary/config candidate 已被后续修改取代。
Gate E old composite preflight：superseded_by_contract_revision。
Gate R5 exact acceptance validator：passed；identity binding schema pending third-revision construction。
Fresh E-R：failed_contract_mismatch；attemptCount=1；retryAllowed=false；freshCandidate=not_created。
Gate F real Codex client acceptance：coverage_pending。
Gate G authoritative regression：blocked_by_gate_f。
```

2026-07-19 首次 254-R1 frozen candidate F-A 已按一次性规则执行并失败。真实 Codex 按公开 typed schema 提交 `projectPatchContextHash=null`，而 `ProjectCandidateEntry` 对 `ProjectPatch` 强制要求当前 session context digest；operation 在一次 Native Editor 批准后以 `project_candidate_entry.digest_invalid` 终止，`commitStarted=false`、项目 digest 未变化。该 candidate 状态固定为 `historical_failed_contract_mismatch`，不得重试或跨候选复用批准/证据。

2026-07-19 第二个 frozen candidate `candidate-20260719-210044` 的一次 F-A 在只读 status/catalog/inspect/search 后等待 Native Editor 批准；session 从 `22:13:22` 到 `22:43:22` 的 30 分钟 TTL 内没有收到用户决定，Gateway 正常 prune approval row，Candidate input 从未提交，`commitStarted=false`、项目 digest 未变化，F-B/G 均未开始。失败 artifact 为 `gate-f/acceptance-artifact.json`，sha256=`bc81ffe4f6a2c37112791f0996b25233a15d86162a26332c71cb3cfe610573f9`；该 candidate 固定为 `historical_failed_approval_timeout`，不得重试。

2026-07-20 第三个 frozen candidate `candidate-20260720-000200` 的一次 F-A 已执行并失败。mutation、receipt lineage、Preview exact presented-frame evidence 与 rollback 均成功，项目恢复到初始 digest；但操作计划在 Preview 后插入了规定链外的 `evidence.read`，该调用因 `evidence_ref_invalid` 失败，visual diagnosis 与 Build/Delivery 链未继续，F-B/G 均未开始。Preview metadata 已在 operation 完成前验证，PNG digest 与 metadata 一致；失败是错误 evidence consumer 与额外调用导致的操作计划违约，不是 Preview evidence 损坏。该 candidate 固定为 `historical_failed_operator_plan_violation`，artifact sha256=`8b022ba4f8cffb9551d490574a1948736e4eeb27131c601d3a602b3191b201b5`，不得重试。

因此先前 E-R 只证明了候选输入与产品能力可用，没有冻结并验证完整的一次性验收计划。Gate R5 已实现并通过。2026-07-20 唯一一次 fresh E-R 的受影响域回归全绿，但在创建 candidate 前发现：plan 强制包含最终 candidate manifest digest，同时 candidate manifest 又必须包含 plan sha256，形成不可施工的 SHA-256 循环绑定；现有 preflight 还硬编码历史 `r2` 标识且不生成 R5 plan。该次 E-R 固定为 `failed_contract_mismatch`，不得重试，fresh candidate 未创建。当前必须先完成本次无环冻结合同修订及对应施工；Gate F 保持 `coverage_pending`，Gate G 保持 `not_started`。

旧 Gate F transcript、旧批准、刷新后的 binary 和任何新截图不得跨候选拼接。

## 2. 真实 Gate F 新证据

### 2.1 已成立的产品基础

- Native Editor 已拥有 Gateway host/discovery 生命周期；
- zero-argument MCP bootstrap 与本机 Codex config 安装/回滚路径已存在；
- Named Pipe、Gateway Core、Tool Kernel、Candidate/receipt、operation、Build/Delivery 等自动化基础已存在；
- `GatewayRemoteAdapter` 固定绑定一个 discovery/pipe/session，能够诚实暴露旧 binding 失效。

### 2.2 被否定的调用合同

1. `aife_execute` 在 `tools/list` 中把 `invocation` 只声明为无类型 object。真实 Codex 因缺少约束加入 `projectIdentity/toolVersion`，严格 Rust decoder 正确拒绝。
2. connect 只记录 `requested_read_scope`；产品批准只签发 `ScopedMutation / ProjectOwnedLowRisk`。search、visual、build 和 delivery 强制要求 `AiCapabilityGrantKind::Read`，因此完整能力链在结构上不可达。
3. `$active` 无法同时代表 session Read capability 和用户批准 mutation grant；多个 grant 时还会产生 ambiguity。
4. `runtime.capture_issue` 与 `ui.explain_visibility` 共用 `ProjectUiExplainInput`，两者都要求调用者先提供 nodeId 和 screenshot/frame digest，导致 `capture -> locate -> explain` 在类型层倒置。
5. 旧 Preview result 只有 RuntimeModule bind digest 与 report path，没有证明用户实际看到的 shared GameView texture 像素。
6. Gateway approval 追加在普通 AI proposals 后，renderer `.take(3)` 会隐藏后续 session；行标签相同且不显示 session/version/age/state/capabilities。
7. closed/expired session 没有形成 Core-owned prune/revoke/UI cleanup 单一真相，stale row 可能继续出现或被点击。
8. `editor_host::run_gateway_process_preflight` 使用 `EditorSession::new()`，没有覆盖生产 Editor 的 empty/complex-shooter/switch-puzzle 三 RuntimeModule composition。
9. 一次性验收不能允许调用者自由决定工具顺序；额外的、看似只读的调用也会改变 authoritative transcript 并中断规定证据链。
10. 只冻结 Candidate direct input 不足以冻结 F-A；工具 allowlist、精确顺序、参数来源、断言和禁止调用同样属于 candidate authority。
11. 30 分钟审批 TTL 前缺少用户 ready handshake；“session 已创建”不能替代“用户已在 Native Editor 前准备批准”。
12. acceptance artifact 与失败分类由操作员手工拼装时，原始工具错误容易被误判为证据损坏，不能稳定区分 `operator_plan_violation` 与 `evidence_corrupt`。
13. `evidence.read` 的 typed schema/description 没有明确暴露 `project-evidence:` prefix 与仅允许 `Library/Reports/`、`Library/AiToolKernel/` 的受控根合同；它不是 Preview visual chain 的合法 consumer。

### 2.3 Fixture 不成立

`<LOCAL_TEST_ROOT>\AiFirstGame` manifest 要求 `project.c01.runtime`；当前生产 Editor 只链接：

```text
empty.runtime
sample.complex-shooter.runtime
sample.switch-puzzle.runtime
```

`descriptor_for_module_id` 修复后，`ai_tool.preview_project_runtime_not_linked` 是正确失败，不是可通过重复批准消除的偶发错误。本轮 Gate F 改用 `samples/complex_shooter_project` 的 disposable copy。若以后必须验收 `AiFirstGame`，需先单独构建并冻结链接 `project.c01.runtime` 的项目专用 Editor。

## 3. Interface 方案比较

### 3.1 方案 A：单一严格 `aife_execute` union

把所有工具输入合并成一个严格 `oneOf`，只增加 session inspect/status。

优点是 MCP tool 数量少、代码改动小。拒绝作为默认 Interface 的原因是 Codex 仍需理解巨大 union、toolId 和 envelope；每次扩展都会继续扩大一个浅入口，发现性和错误定位都差。

### 3.2 方案 B：手写逐工具 MCP tools

每个 Tool Kernel tool 暴露一个严格 MCP tool。

优点是默认调用最简单。拒绝手写实现的原因是会在 `mcp_stdio.rs` 形成第二套 schema/example/annotation 真相，与 Tool Catalog 漂移。

### 3.3 方案 C：Catalog projection typed tools

深化现有 tool contract 区域为 `AiToolContractRegistry`，由 `McpToolProjection` 自动生成逐工具 MCP Interface。

```text
tool id
  -> strict direct input schema
  -> minimal direct input example
  -> canonical decoder -> AiToolInvocationPayload
  -> output schema / capabilities / side effects / annotations
```

该方案兼有逐工具发现性和单一 contract locality。删除投影 Module 后，schema 映射、命名、annotations 和 envelope 构造会重新散落到每个 Adapter，因此它具有足够 Depth。正式采用方案 C。

## 4. 当前架构合同

### 4.1 `AiToolContractRegistry`

Registry 位于 Tool Kernel seam，拥有每个 tool 的直接输入合同。统一规则：

- `input_schema` 只描述 MCP/Adapter 调用者应提交的 tool-specific input，不描述内部 `payloadKind/payload` tagged enum；
- `minimal_input_example` 与 `input_schema` 是同一层形状；
- 所有 object 递归 strict，除明确扩展点外使用 `additionalProperties=false`；
- Candidate 的 ProjectPatch、ControlledSourcePatch 和 AssetImport 分支也必须有递归可判别 schema；
- 每个 minimal example 同时通过 JSON Schema 与 canonical Rust decode；
- direct schema 只暴露调用者拥有的事实；`projectPatchContextHash` 是 active `EditorSession` 派生事实，不得由 MCP caller 提交，typed Adapter 必须在 session-bound dispatch 时捕获并写入内部 `ProjectCandidateEnvelope`；
- 对每个 Candidate payload 分支，合同验证不得止于 schema/decode，必须覆盖 `schema-valid direct input -> canonical decode -> session-owned facts binding -> non-mutating prepare`；ProjectPatch 必须在 durable operation 创建前完成 prepare-valid 检查；
- Catalog、MCP projection、CLI help 和测试均消费同一 Registry。

`project.inspect` 可以映射到 Kernel inspect Interface；其余 Catalog tool 映射到 Kernel execute Interface。这个映射属于 Registry facts，不由 MCP caller 猜测。

### 4.2 面向 Codex 的 MCP Surface

```text
aife_status
aife_catalog
aife_project_inspect
aife_project_search
aife_project_read_object
aife_project_references
aife_project_source_symbols
aife_project_diagnostics
aife_evidence_read
aife_project_mutate_candidate
aife_project_rollback_candidate
aife_project_preview
aife_runtime_capture_issue
aife_ui_locate
aife_ui_explain_visibility
aife_project_trace_ui_owner
aife_project_build_export
aife_project_delivery_verify
aife_observe
aife_cancel
```

MCP caller 只提交 direct tool input。Adapter 负责：

```text
tool name -> toolId/toolVersion/payload variant
MCP request identity -> bounded invocationId
session observed digest -> expectedProjectDigest for read/preview/build/delivery
Candidate envelope or rollback receipt -> expectedProjectDigest for mutation
session access class -> exact Read or mutation grant
```

Candidate/rollback 自带的项目事实必须与当前 session observed digest 交叉验证。调用者添加 `projectIdentity/toolVersion/payloadKind/grantRef` 等未知字段时在 MCP 参数层返回 `-32602`。

`aife_execute` 不再出现在 `tools/list`。为旧测试或兼容客户端暂留时，只允许严格完整 `AiToolInvocation`，并标记 legacy/unadvertised；不得继续使用无类型 schema。

### 4.3 `aife_status`

`aife_status` 不需要 Grant，也不产生副作用。返回 `GatewaySessionStatus`：

```text
session:
  id / clientKind / clientVersion
  connectedAt / lastSeenAt / ageMs / expiresAt
  state
project:
  identity / currentDigest / observedDigest / runtimeModule / catalogDigest
access.read:
  active / effectiveScopes / generation / staleReason
access.mutation:
  awaiting_user | active | revoked | expired
  requestedProfile / capabilities / blockedCapabilities
  grantDigest / expiresAt / remainingBudget
reconnectRequired
nextAction
```

`awaiting_user` 是成功状态。Codex 在用户批准后查询 status，不重复提交 mutation 探测授权。

### 4.4 Session Read 与 mutation Grant

Gateway Core 根据 handshake 的有效 read scope 派生内部 session-bound Read grant：

- connect 时绑定项目和 observed digest，建立 generation 1；
- Catalog/status/project inspect 不需要 mutation approval；
-普通 read tool 使用该 session 的 Read grant；
- 同 session 的正式 mutation/rollback receipt 推进 observed digest 和 read generation；
- 外部未知 project drift 不自动轮换，read 状态变为 stale；
- 显式 project inspect 观察当前 digest 后才建立新 generation；
- Read grant 不进入 opaque mutation grant 集合，也不参与 `$active`。

mutation 仍使用 `ScopedMutation / ProjectOwnedLowRisk` 或显式 Elevated grant。Native Editor 每个 session 同时最多一个 active mutation grant；新批准撤销旧 grant。Gateway 选择 grant，不把 opaque ref 暴露给 MCP。

operation 启动时记录 client session 与 exact grant snapshot。`aife_cancel({operationId})` 根据 operation ownership 校验，不能用当前活动 grant 猜测历史 operation。

### 4.5 Core-owned approval lifecycle

Gateway Core 提供集中 Interface：

```text
approval_inbox(now) -> Vec<GatewayAccessRequest>
decide_access(requestId, decision, actor, now) -> AccessDecisionReceipt
session_status(clientSessionId, now) -> GatewaySessionStatus
prune(now) -> SessionAccessCleanupReport
```

Core 隐藏 session existence/TTL/project binding 校验、grant 构造、单活动 grant、revoke 和 stale decision 拒绝。peer EOF、explicit close、TTL、project switch/close 都清理 session 与所属 grant。

Native Editor 使用专用 `GatewayAccessRequestModel`，不再伪装为 `AiProposedCommand`。每行至少显示：

```text
session short id / client kind / client version
project identity / connected age / state / expires
requested capabilities / blocked capabilities
Approve / Reject actions with unique hit ids
```

审批区域必须 bounded scroll/page，不能 `.take(3)` 静默丢行。两个同版本 Codex 也必须显示可区分的 session 行。

### 4.6 Preview exact presented evidence

`project.preview` 是跨 EditorSession、real window 和 GPU texture 的异步链，不能在 Play command 提交后立即完成：

1. Tool Kernel 创建 operation，进入 `running/awaiting_frame_evidence`；
2. `real_window::present_active_game_view_to_shared_texture` 正常 tick 并把 RHI plan 渲染到 shared GameView texture；
3. 渲染成功后从该 exact texture readback，不另跑一份独立 render；
4. `ProjectPreviewEvidence` Module 在项目受控路径原子写入 PNG 与 metadata；
5. 下一次 Tool Kernel pump 验证 receipt、project/frame/hash 后完成 operation。

受控 evidence 路径：

```text
Library/AiCapability/Preview/<operationId>/frame.png
Library/AiCapability/Preview/<operationId>/frame-evidence.json
```

`PreviewFrameEvidence` 至少包含：

```text
schemaVersion / operationId
projectIdentity / projectDigest
gameViewSessionId / textureId
frameIndex / frameDigest / runtimeFrameHash
screenshotRef / screenshotDigest
width / height / pixelFormat / captureKind
presentReportRef / evidenceDigest
```

GPU shared texture 必须带 `COPY_SRC` usage，readback 处理 row alignment、map timeout 和 format。黑屏可能正是待诊断 bug，因此不能仅因全黑像素拒绝；只拒绝无效尺寸、字节长度、编码、hash、project/frame 归属或 readback 失败。

deterministic/headless capture 是测试 Adapter，不是 Gate F 权威证据。Gate F 只接受 production real-WGPU exact shared-texture evidence。

### 4.7 Visual evidence reference chain

拆分旧 `ProjectUiExplainInput`：

```text
runtime.capture_issue({ frameEvidenceRef, symptom? })
  -> VisualIssueBundle / issueBundleRef

ui.locate({ query, issueBundleRef? })
  -> stable candidates

ui.explain_visibility({ documentPath, nodeId, issueBundleRef })
  -> first failing stage + evidence refs

project.trace_ui_owner({ documentPath, nodeId, issueBundleRef? })
  -> AUI/binding/action/project source ownership
```

调用者不再提供 `screenshotRef/screenshotDigest/frameDigest`。读取 evidence 时重新验证 project identity/digest、metadata digest、PNG bytes digest、operation/frame ownership 和 current retention。`present_package_smoke` 不能作为 production capture fallback。

### 4.8 Project switch 与 reconnect

`GatewayRemoteAdapter` 保持 fixed discovery/pipe/binding。project switch/reopen 后：

- old host/session/grants 立即失效；
- old adapter 返回稳定 `reconnect_required` diagnostic；
- Adapter 不透明 rediscover 到另一个项目；
- 旧 MCP process 结束；
- 新 Codex task/MCP process 通过当前 discovery 建立不同 `clientSessionId`。

透明重连会掩盖 session/grant 生命周期并可能跨项目误连，正式拒绝。

### 4.9 `GateFAcceptancePlan/Validator`

新增候选绑定的窄范围 deep Module，集中 Gate F-A 的 invocation plan、逐步断言、HITL readiness、失败分类、transcript 校验与 artifact 收集。它不替代 Tool Kernel、Candidate、receipt、operation 或 rollback 真相，也不规划任意用户任务；删除该 Module 后，这些一次性验收约束会重新散落到施工文档、操作者记忆和手工 artifact，因此该 Module 具有足够 Depth 与明确 Locality。

计划状态固定为：

```text
binding_preflight
  -> read_discovery
  -> awaiting_user_ready
  -> awaiting_approval
  -> mutation
  -> post_mutation_inspect
  -> preview
  -> capture_issue
  -> locate
  -> explain
  -> owner_trace
  -> build
  -> delivery_verify
  -> rollback
  -> terminal
```

`gate-f/acceptance-plan.json` 至少冻结：plan schema/version、`candidate-binding-manifest.json` digest、config/fixture binding、允许的 typed MCP tool、精确调用顺序和调用次数、每步 literal direct input 或前一步 output-derived 参数规则、预期 assertion、禁止调用、终止与 rollback 规则、失败分类、artifact schema。plan 本身不得包含外层 `candidate-manifest.json` digest 或 `freeze.json` digest；它们必须在 plan 生成后才能确定。Preview visual chain 只能由 Preview result 直接进入 `runtime.capture_issue`；`evidence.read` 明确不在 allowlist 中。

Validator 在调用前判定下一步是否合法，在结果后验证 operation/receipt/grant/project/frame/digest lineage。任何额外工具、错序、缺步、重复 approval/mutation 或参数来源漂移都终止为 `operator_plan_violation`，不得继续消费证据或重试。失败分类固定为：

```text
contract_mismatch
approval_timeout
authoritative_step_failed
operator_plan_violation
evidence_corrupt
cleanup_failed
```

acceptance artifact 必须由 validator 按 transcript 和 authoritative outputs 生成并验证；原始 diagnostic 可以保留，但 terminal classification 不得由操作员从单个错误码手工推断。rollback 是 mutation 后所有 terminal path 的必经步骤；rollback 或恢复 digest 校验失败时最终分类为 `cleanup_failed`，并保留原始首失败作为 cause。

### 4.10 无环 Candidate Freeze 拓扑

冻结关系固定为单向 DAG，不允许任何文件通过自身或下游文件的 digest 反向绑定：

```text
source/binary/config/fixture/tools/schema/input/prepare receipt
  -> candidate-binding-manifest.json
  -> gate-f/acceptance-plan.json
  -> candidate-manifest.json
  -> freeze.json
```

各层职责固定如下：

1. `candidate-binding-manifest.json` 是 plan 可引用的内层叶子聚合。它只绑定 source、Cargo manifests/lock、binary、实际 Codex config、隔离 config receipt、actual compare-and-replace receipt、fixture、typed tools/catalog/schema、exact Candidate direct input、non-mutating prepare receipt、artifact schema 和环境 smoke；不得包含 acceptance plan、外层 candidate manifest 或 freeze marker 的 digest。
2. `gate-f/acceptance-plan.json` 把原 `candidateManifestDigest` 字段替换为 `candidateBindingManifestDigest`。plan 绑定内层 manifest、config digest、fixture root 和 17 步协议，但不绑定任何下游 digest。
3. `candidate-manifest.json` 是外层候选聚合，必须同时绑定 `candidateBindingManifestDigest`、`acceptancePlanDigest`、plan schema、expected step/forbidden-call 摘要、Candidate input/prepare receipt、config replace receipt、cleanup report 与所有冻结根；它不被 plan 反向引用。
4. `freeze.json` 只绑定最终 `candidate-manifest.json` digest、candidate id/root、状态和失效条件。Gate F 入口从 `freeze.json -> candidate-manifest.json -> plan/binding manifest -> leaves` 逐层验证，不允许跳层信任人工摘要。

`GateFAcceptanceBinding` 和 validated acceptance artifact 同时记录 `candidateBindingManifestDigest` 与外层 `candidateManifestDigest`。Validator 用前者与 plan 对齐，用后者证明本次 transcript 属于哪一个完整 frozen candidate。这样保留 candidate lineage，同时消除 plan 与 manifest 的密码学环。

## 5. 真实 Codex 验收合同

### 5.1 F-A：同 session 完整能力链

Fixture 使用生产 Editor 已链接的 complex-shooter disposable copy。绑定同一 frozen source/binary/config manifest：

```text
binding preflight -> freeze/outer candidate/inner binding/plan/config/fixture/discovery/session hashes
aife_status -> aife_catalog -> project.inspect -> project.search
user ready handshake -> start 30-minute session/approval window
Native Editor 一次明确批准 -> aife_status confirms mutation active
Candidate mutation -> receipt
project inspect -> new digest/read generation
Preview -> exact presented frame evidence
runtime.capture_issue -> ui.locate -> ui.explain_visibility -> trace owner
Build/Delivery Verify
rollback/cleanup -> restored digest
```

F-A 只能消费 Gate E-R 冻结并校验 sha256 的 `gate-f/acceptance-plan.json`。计划冻结精确可逆 Candidate direct input；该 input 不包含 `projectPatchContextHash`，并必须已通过真实 complex-shooter session 的 typed decode、session-owned context binding 与 non-mutating prepare。真实 Codex 必须按计划的 allowlist、顺序、次数与参数来源执行，不得增加“辅助读取”、跳步、重试、请求第二次批准或临场发明/补写内部 context/grant/envelope 字段。

E-R 同时冻结 status/catalog/inspect/search 的精确顺序与 search query，visual symptom/query/expected document/node 选择规则，Build/Delivery arguments，以及从 Candidate receipt 获取 rollback direct input 的规则。所有动态值只能来自计划声明的前序 output path。Preview 后直接把计划指定的 `frameEvidenceRef` 提交给 `runtime.capture_issue`；禁止调用 `evidence.read`。

只有用户显式确认已在 Native Editor 前准备批准后，Validator 才能创建/进入 30 分钟 approval session。批准只消费一次 `ProjectOwnedLowRisk` mutation decision；TTL 到期、拒绝或任何证据无效均立即失败收敛，不得重试 F-A。mutation 已发生时仍必须执行一次计划内 rollback 并验证初始 digest 恢复。

冻结工具必须通过当前 Rust `ProjectPatchDocument` / `ScenePatchOperation` 类型构造 F-A payload，不得手写或沿用历史 schemaVersion。首次 executable preflight 进一步证明旧 F-A payload 的 `editor-project-patch.v1` 已过期；该错误与 context mismatch 一并纳入累计失败集。

Validator 从 authoritative transcript/output 生成并验证 Codex Desktop/version/config digest、candidate binding manifest digest、外层 candidate manifest digest、acceptance plan digest、session id、project identity/root/digest、tool calls、grant/receipt/operation ids、PNG/metadata refs 与 digests、Build/Delivery result、elapsed time、diagnostics 和 cleanup。

### 5.2 F-B：显式新 session

```text
switch/reopen project
old adapter returns reconnect_required
old MCP process ends
new Codex task/MCP process starts
new session id differs from F-A
aife_status + catalog + project inspect/search succeed
```

F-B 只证明重连和 read readiness，不再次批准 mutation，也不重跑 mutation、Preview、visual 或 Build。F-A 失败后不得对同 candidate 重跑；F-B 只有在 F-A 通过后才可进入。

## 6. 非目标

254-R1 不做：

- 通用 Agent Planner 或统一产品 Runner；候选绑定的窄范围 Gate F acceptance plan/validator 属于本次修订范围；
- 第二份 Tool Catalog、Candidate、mutation、receipt 或 rollback 真相；
- 任意 shell、网络服务、项目根外写入；
- Unity/UE 或三引擎 B 通道重跑；
- ProductBrokered OS sandbox、插件市场、Installer、签名、自动升级；
- 世界对象、材质、动画、粒子等新视觉工具域；
- 同一个 Adapter 在 project switch 后透明 rediscovery；
- 用自建 JSON-RPC client 或 deterministic screenshot 冒充真实 Codex Gate F。

## 7. 修订 Gate

### Gate R0：Formal Contract Correction / Baseline

更新 254 与 254-R1 正式方案、当前施工文档和入口；冻结当前已知失败。运行最小 baseline，区分已有失败与新增红测试。

### Gate R1：Tool Contract Registry / Typed MCP Projection

规范化 Catalog schema/example/decode，投影逐工具 strict MCP tools，隐藏 raw `aife_execute`。每个 example 同时通过 schema 与 Rust decode；未知/多余字段返回 `-32602`。

### Gate R2：Session Access / Status / Approval Lifecycle

实现 `aife_status`、session Read generation、mutation grant 选择、operation-owned cancel、Core-owned approval inbox/decision/prune 和专用 UI rows。

### Gate R3：Preview / Visual Evidence

实现 pending frame ticket、exact shared texture readback、project-controlled PNG/metadata、Tool Kernel completion barrier 和 evidence-ref visual inputs。

### Gate R4：Production Composition / Reconnect

真实 `editor_host` 预检使用 production 三 RuntimeModule composition 打开 complex-shooter fixture；验证 Preview module binding、old adapter failure 和 new session identity。

### Gate R5：Exact Acceptance Plan / HITL / Artifact Validator

保留已通过的候选绑定 `GateFAcceptancePlan/Validator` 17 步、allowlist、精确顺序/次数、literal/output-derived 参数规则、逐步 assertions、HITL ready handshake、terminal/rollback、failure taxonomy 与 acceptance artifact 语义。第三次修订只替换循环身份字段：plan 的 `candidateManifestDigest` 改为 `candidateBindingManifestDigest`；`GateFAcceptanceBinding` 与 validated artifact 同时持有内层 binding digest 和外层 candidate manifest digest。增加 plan 引用外层 manifest、binding manifest 引用 plan、外层 manifest 缺少 plan digest、freeze 链断裂和 digest tamper 的否定测试。不得借机改变 F-A 步骤或扩展为通用 runner。

### Gate E-R：Composite Automated Preflight / Candidate Freeze

R1-R5 定向测试、受影响域回归和环境等价预检全绿后，只能通过版本化 Rust freezer 冻结新 candidate。freezer 必须按第 4.10 节 DAG 依次生成并严格解码 `candidate-binding-manifest.json`、`gate-f/acceptance-plan.json`、`candidate-manifest.json` 和 `freeze.json`；冻结内容包括精确 F-A reversible Candidate direct input、schema/catalog digest、expected step list、argument-source rules、HITL readiness protocol、failure taxonomy、acceptance artifact schema，以及在 disposable complex-shooter active session 上完成 `decode -> bind session facts -> non-mutating prepare` 的结构化 receipt。三个失败 candidate 和失败 E-R staging 只作历史诊断，均不得作为输入模板、candidate id、payload id、invocation id、config receipt 或发布根复用。

若实际 Codex config 仍绑定历史失败 candidate，不得手工覆盖同名 MCP entry，也不得把旧 receipt 视为新候选证据。必须使用显式 compare-and-replace：输入 manifest 记录的预期旧 MCP absolute path 与新候选 MCP absolute path；只有当前 entry 精确等于预期旧路径时才原子替换，保留完整 before backup、install receipt 和 rollback 能力。entry 缺失、路径不匹配或 config drift 一律 fail-closed，并返回 E-R 处理。

fresh preflight/freezer 必须是 `ai_tool_gateway` 内的版本化 Rust binary/library，不以候选目录中的临时 PowerShell 或历史 `gate_f_candidate_preflight` 常量作为 authority。外部 Interface 收敛为 `CandidateFreezer::run(FreshCandidateFreezeRequest) -> FreshCandidateFreezeReport`；request 只包含全新 candidate id、最终 root、scoped source root、固定 `CARGO_TARGET_DIR`、fixture source、实际 config path、预期旧 MCP absolute path和总 timeout policy，staging/discovery/evidence/config artifact 子路径均由 Module 在已验证 containment 的同卷根内确定。最终 root/candidate id 已存在或命中历史 inventory 时立即拒绝。

完整 DAG 解析只实现一次 `FrozenCandidateVerifier::verify(freezePath) -> FrozenCandidateVerificationReport`，由 freezer 发布后复验和 Gate F binding preflight 共同调用。filesystem、process、clock 与 config fault injection 是 Module 内部 seam，生产 Adapter 与测试 Adapter 不进入外部 Interface。调用方不能逐阶段调用、跳过 cleanup、手写 manifest 或自行决定恢复顺序；删除该 Module 后这些复杂度会重新散落到脚本、Gate F 和操作者，因此它是具有 Depth/Locality 的冻结 Module，而不是命令包装层。

freezer 固定执行阶段：

```text
reserve unique id/root
  -> scoped source + exact Cargo.lock/features
  -> release binaries + disposable fixture
  -> isolated config install smoke
  -> typed tools/catalog/schema + reconnect + real-WGPU smoke
  -> exact Candidate typed construction/decode/bind/non-mutating prepare
  -> compare-and-replace actual config（保留 rollback receipt）
  -> candidate-binding-manifest
  -> acceptance plan generate + strict validate
  -> cleanup verify
  -> outer candidate manifest + closure validate
  -> freeze marker
  -> same-volume atomic publish
  -> reopen and verify complete digest DAG
```

#### Production stage producer 合同（第四次修订，2026-07-20 已确认）

本修订只补齐 freezer 内部生产证据来源，不改变 `CandidateFreezer::run(FreshCandidateFreezeRequest) -> FreshCandidateFreezeReport` 外部 Interface，不向 request 增加 receipt path、阶段开关、Candidate payload、内部 context hash 或可跳过步骤。调用方不能逐阶段驱动生产者；`CandidateFreezeProductionEnvironment` 仅为 `pub(crate)` 内部 seam，由 freezer 按固定状态机调用。测试 Adapter 与 production Adapter 实现同一内部 Interface，但测试 receipt 永远标记 `producerKind=test_adapter`，`FrozenCandidateVerifier` 和 production publish 必须拒绝它。

所有新 receipt 使用 strict serde `deny_unknown_fields`、canonical camelCase JSON、版本字段和原子写入；路径均为 staging root 内 `/` 分隔的规范相对路径，digest 均为小写 `sha256:<64hex>`。receipt 自称 `passed` 不构成证据：freezer 在进入下一阶段前必须重新读取 receipt、验证 schema/producer kind/input lineage，并对每个引用文件独立计算 digest。固定 schema：

```text
candidate-freeze-source-snapshot-receipt.v1
candidate-freeze-release-binary-receipt.v1
candidate-freeze-fixture-snapshot-receipt.v1
candidate-freeze-isolated-config-receipt.v1
candidate-freeze-editor-preflight-receipt.v1
candidate-freeze-cleanup-receipt.v1
candidate-freeze-stage-failure.v1
```

生产者与 ownership 固定如下：

1. `SourceSnapshotProducer`
   - 输入只来自 request 的 canonical scoped source root 与 Module 固定 include/exclude policy；拒绝 symlink/junction/reparse、路径 escape、大小/文件数上限和读取期间漂移。
   - 同卷复制到 `source/`，生成逐文件 SHA-256、mode/size、git HEAD（仅作观察值）、dirty scoped truth、`Cargo.lock`、workspace `Cargo.toml`、`rust-toolchain.toml` 与 selected feature set；source tree digest 由排序后的 canonical entries 计算，不能用 Git commit 替代混合施工真相。
   - receipt 必须记录 source root、frozen root、file/gitlink count、policy version、lock/toolchain/features digest 与完整 source manifest ref/digest。

2. `ReleaseBinaryProducer`
   - 只从已冻结 `source/` 构建，不从活动工作树构建；固定 `CARGO_TARGET_DIR` 显式传播，先执行 locked metadata，再执行 `cargo build --locked --release`，目标固定为 `editor_host`（`real-window,real-wgpu-surface`）和 `ai_tool_gateway` 的 MCP/config/CLI/freezer entry binaries。
   - 使用 production process-group runner；成功退出码只证明命令完成，receipt 还必须记录 exact executable/args/current dir/env allowlist、Cargo.lock/features/source digest、process result、每个复制到 `bin/` 的 canonical path/size/SHA-256 和 PE regular-file identity。缺少任何目标或 build 后 source/lock 漂移均失败。

3. `FixtureSnapshotProducer`
   - 从 request fixture source 做两份同 digest copy：`fixture/<name>` 是 clean reference，`editor-run/<name>` 是后续 production session 与 Gate F 精确 root；拒绝 reparse/path escape，固定排除 `Build/**`、`Library/**`、`.aife/**`、`target/**` 等生成目录。
   - receipt 记录 clean/editor-run manifests、file count、project id、canonical roots与初始 project digest；两份 copy 必须逐文件等价。read generation 属于后续 Gateway session 内事实，只能由 Editor preflight receipt 记录。之后所有 producer 只能绑定 `editor-run/<name>`。

4. `IsolatedConfigProducer`
   - 直接复用 `install_codex_mcp_config` 与 `CodexConfigInstallReceipt`，目标只能是 staging 内 `config/smoke/config.toml`，command 只能是本次 `bin/ai_engine_gateway_mcp.exe`。
   - wrapper receipt 绑定 binary digest、config before/after/fragment/install receipt digests；安装后重新 strict decode，cleanup 时必须 rollback/remove isolated config。不得把实际用户 config 用作 isolated smoke。

5. `EditorFreezePreflightProducer`
   - 新增版本化 Rust producer mode，由本次 release `editor_host.exe` 启动 production three-RuntimeModule `EditorSession`，打开精确 `editor-run/<name>`；其结构化 receipt 是 typed surface、reconnect、real-WGPU 与 Candidate prepare 的唯一联合 authority。Cargo test log、临时 PowerShell、外部 JSON-RPC client transcript或 deterministic Adapter 不能作为该 receipt。
   - producer 内部只允许启动本次 release MCP binary。第一次 MCP session 依次完成 initialize、typed `tools/list`、status、catalog、project inspect/search；随后受控切换并重开同一 fixture，使旧 session 返回 `reconnect_required` 且旧 MCP 退出；第二个 MCP process/session id 必须不同，并再次证明 read active、mutation awaiting_user，禁止批准或 mutation。
   - 在第二个 production session 中执行 non-mutating Preview preflight；必须得到 `ProjectPreviewCaptureKind::RealWgpuExactSharedTexture` 的 exact frame，使用 `ProjectPreviewEvidence` 持久化并复验 PNG/metadata、operation/project/game-view-session/texture/frame ownership 与全部 digest。黑屏允许，invalid dimensions/bytes/format/hash/ownership/readback 不允许。
   - 同一第二 session 由内部 `GateFCandidateInputFactory` 创建 fresh reversible Candidate direct input：candidate/payload/operation id 由本次 request candidate id 派生，目标固定为 fixture 已验证的 project-owned reversible anchor；落盘 direct input不得含 `projectPatchContextHash`、grant、invocation envelope或其它 session-owned字段。producer 必须按 Registry direct schema validate/decode，再调用 `bind_session_owned_invocation_facts`，最后 `ProjectCandidateEntry::prepare`；receipt 记录 direct input/schema/canonical payload digest、内部绑定 context hash、prepared candidate digest，并固定 `commitStarted=false`、`mutationExecuted=false`。
   - receipt 至少同时绑定 editor/MCP binary digests、fixture manifest/project identity/root/initial/final digest、两次 session ids及各自 read generation、tools-list/schema/catalog digests、每次 typed call result digest、reconnect evidence、Preview frame/PNG/metadata refs/digests、Candidate input/prepare refs/digests。任一子证据来自不同 fixture、binary、session 或 operation 时整体失败。

6. `ActualConfigProducer`
   - 直接复用 `replace_codex_mcp_config(actual, expectedCurrentMcp, newMcp, artifactRoot)`；只有当前 command 精确等于 request 预期旧 absolute path 才允许改变。
   - config 写入成功后必须立即把内存 `CodexConfigInstallReceipt` 注册到 freezer rollback guard，再持久化 receipt；因此“config 已改变但 receipt 文件写失败”仍必须按内存 receipt rollback。before/after、backup、fragment、new MCP digest 和实际 installed copy 全部进入 binding manifest。

7. `ManifestPlanProducer`
   - 纯函数生产 `CandidateBindingManifest -> GateFAcceptancePlan -> FrozenCandidateManifest -> CandidateFreezeMarker`，每次编码后立即 strict decode 和 digest closure；禁止生产 Adapter 自己手写 JSON 字符串。
   - plan 的 exact Candidate literal 必须逐字节引用第 5 阶段 direct input；plan 只绑定 inner binding digest，outer manifest 单独绑定 plan/binding，freeze marker 只绑定 outer manifest。

8. `CleanupProducer`
   - cleanup receipt 独立于 editor preflight receipt，在 editor/MCP process tree 全部退出后由 freezer 生成；验证所有已登记 PID 已退出且 reap、discovery root 空、isolated config 已恢复/删除、editor-run project digest 回到初始值、preflight receipt 证明没有 mutation/grant/approval、没有 staging 外写入。read generation 是已退出 session 的证据，只在 preflight receipt 内校验，不由进程外 cleanup 伪造。
   - success 路径保留 actual config 的本次新 MCP binding；failure 路径只按 rollback guard 中本次 receipt 恢复 actual config。cleanup receipt 缺失或任一项失败都禁止生成 outer manifest/freeze。

子进程合同固定为 Windows process group/Job Object，必须启用 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 并登记 cargo/editor/MCP 及后代；现有 `runtime_cli::BoundedChildProcessResult` 可作为进程摘要，但仅杀根进程的实现不足以满足 production freezer。timeout/cancel/wait failure 必须 kill-and-reap 整棵树，记录 kill/wait/read-join error；任一残留进程均为 `cleanup_failed`。

`timeout_ms` 是总 deadline，不是单个命令 timeout，允许范围固定为 10 分钟至 2 小时。Module 保留 120 秒 cleanup reserve；每阶段实际 deadline 为 `min(total_deadline-cleanup_reserve, stage_cap)`，stage cap 固定为 source 120 秒、release build 1800 秒、fixture 120 秒、isolated config 60 秒、editor preflight 600 秒、actual config 60 秒、manifest/finalize 120 秒、reopen verify 60 秒。进入阶段前剩余时间不足 1 秒即 fail-fast；任何阶段不得自行重试或延长总 deadline。

失败 artifact root 由 Module 在 final root 同卷父目录派生为 `.<candidate-id>.failed`，quarantine 为 `.<candidate-id>.quarantine`；两者与 staging/final 任一已存在均在 reserve 阶段拒绝。publish 前失败先完成 process/config/fixture cleanup，再把不含 `freeze.json` 的 journal、stage failure、已产生 receipts 原子保存到 failed root；publish 后 reopen failure 先 rollback config，再原子移动完整 final root 到 quarantine，禁止改写其冻结字节。相同 candidate id、payload id、operation id、root 或 failure/quarantine inventory 永久禁止重试。

freezer 维护结构化 stage journal，每阶段写 `started/completed/failed`、输入/输出 digest、耗时和 cleanup 状态。任何阶段失败都必须终止：若实际 config 已改变则只按本次 receipt rollback；停止本次 editor/MCP 子进程，清理 discovery 和 disposable fixture mutation，禁止发布 final candidate root。失败证据写入候选根之外的 E-R failure root；不得留下可被 Gate F 误认的 `freeze.json`。原子发布后 reopen verification 失败时，不得改写 final root 内任何冻结字节；必须先按本次 receipt rollback config，再把整个 final root 同卷原子移动到 E-R failure root 并记录 `historical_failed_freeze_integrity`。隔离失败则升级为 `cleanup_failed` 并保持 Gate F 阻塞；任何失败 root 均不得原地修补或重试。

### Gate F：One Human Acceptance

只按第 5 节执行一次 F-A/F-B。未取得同候选真实 Codex artifact 时保持 `coverage_pending`。

### Gate G：Regression / Closure

Gate F 通过后在 immutable frozen source + disposable execution copy 运行一次 default/all-features authoritative regression；随后 cleanup、完成记录、入口同步与归档。

## 8. 验证边界与环境

```text
OS：Windows，真实 Named Pipe / Winit / WGPU surface。
施工 target：<LOCAL_TEST_ROOT>\BuildTargets\254-r1-revision
Gate F project：samples/complex_shooter_project disposable copy。
源码：当前混合工作树必须先生成 scoped source manifest；最终权威运行使用等价隔离/frozen source。
候选：代码、Cargo manifests/lock、binary、Codex config、fixture、evidence schema、binding manifest、acceptance plan schema/hash、外层 candidate manifest、exact direct inputs、output-derived parameter rules 或 artifact schema 任一变化都使 freeze 失效。任何上游变化都必须重新生成全部下游 digest，禁止原地改写已发布 candidate。
子进程：显式传播 CARGO_TARGET_DIR、project/evidence roots 与 timeout；不依赖 shell 当前目录或动态临时路径。
cache：预检记录 cold/warm 或命中状态；cache 不得掩盖 exact binary/source digest。
换行/路径：Windows 路径与 CRLF/LF 不进入 canonical project/evidence digest 的隐式差异。
```

验证分四层：

1. 定向红测试：先证明旧 schema 会产生循环或错误引用，再覆盖 plan 只接受 binding digest、内层 manifest 禁止下游 digest、外层 manifest 必须绑定 plan/binding、freeze 必须绑定外层 manifest、全链 tamper/unknown field/历史 id/root/config receipt 拒绝、失败 rollback 与无 `freeze.json` 发布。
2. 受影响域回归：`ai_tool_gateway/editor_core/editor_ui_model/editor_ui_renderer/editor_wgpu_renderer/editor_window_winit/editor_host/project_e2e_gate`；累计失败集一次性收敛。freezer 修改后必须重新跑 R1-R5 与原 fresh E-R 九项矩阵，之前的全绿只能作为 baseline，不能作为新候选证据。
3. 环境等价预检：在测试 Adapter 与隔离 config/root 上完成 freezer contract、DAG verifier 和 config install/replace/rollback 故障注入后，再仅执行一次真实 freeze；验证 fresh id/root、真实 config digest、release binaries、real-WGPU PNG/metadata、MCP reconnect、atomic publish、reopen DAG validation 和 cleanup。任何失败都不重试该 freeze attempt。
4. 最终权威回归：只有新 candidate 冻结、真实 Gate F 通过且 candidate 未漂移后，才运行一次完整 default/all-features Gate G。

## 9. 高风险否定矩阵

```text
tools/list advertises raw untyped aife_execute
Catalog example passes schema but fails Rust decode, or inverse
ProjectPatch direct schema exposes/accepts caller-supplied projectPatchContextHash
schema-valid Candidate branch cannot bind current session facts and complete non-mutating prepare
durable Candidate operation is created before session-owned binding and prepare-valid checks complete
unknown projectIdentity/toolVersion/payloadKind accepted by typed tool
session connects but Read capability absent
Read and mutation share $active or become ambiguous
one session can use another session grant/status/operation
external drift silently rotates read generation
closed/expired/switched session remains in approval UI or accepts stale click
four approval rows are truncated or share hit target
Preview completes before exact presented frame evidence
PNG/meta project, operation, frame or digest mismatch accepted
visual tool accepts caller-supplied screenshot/frame digest
production Editor previews an unlinked RuntimeModule
AiFirstGame is used without project.c01.runtime-linked Editor
old Adapter transparently crosses project switch
F-B repeats mutation approval/full chain
old transcript/new binary/new screenshot are combined
Build/Delivery writes outside controlled roots
acceptance plan allows an extra tool, wrong order, missing step or duplicate approval/mutation
Preview evidence is sent to generic evidence.read instead of runtime.capture_issue
operator plan drift is mislabeled as evidence corruption
acceptance artifact or failure classification is manually assembled instead of transcript-validated
approval session starts before explicit user readiness handshake
acceptance plan references outer candidate manifest or freeze marker digest
candidate-binding-manifest references acceptance plan, outer manifest or freeze marker
outer candidate manifest omits or mismatches binding/plan/config/cleanup digest
freeze marker does not resolve to one strictly valid outer manifest DAG
historical candidate id/root/payload/invocation/config receipt is reused
config compare-and-replace failure leaves actual config changed
pre-publish failure leaves final candidate root or freeze marker
post-publish reopen failure mutates frozen bytes instead of rollback plus atomic quarantine
```

## 10. 方案自审

范围：通过。修订的是已暴露给真实 Codex 的 Interface、session access、审批、Preview evidence 与一次性验收控制，不新增通用 Agent、统一产品 Runner 或工具域。

Depth/Locality：通过。Registry 集中 tool contracts；MCP projection 隐藏 envelope；Gateway Core 集中 session access/approval cleanup；Preview Evidence Module 集中 readback artifact 与验证；GateFAcceptancePlan/Validator 集中 allowlist、顺序、参数来源、HITL、transcript 与 artifact truth，Interface 仅暴露 plan load/advance/validate/finalize 所需的窄操作。

Freezer Module Depth/Locality：通过。外部只暴露一个结构化 freeze request/result 和一个 frozen candidate verify Interface；路径派生、阶段顺序、manifest DAG、进程/config side effects、rollback、cleanup、原子发布与 reopen verification 均在实现内部。测试通过内部 filesystem/process/clock/config Adapter 注入故障，不迫使生产调用方学习内部 seam。

Tool Kernel 真相：通过。Kernel 继续拥有 capability type、tool authorization、Candidate/mutation/receipt/rollback 和 operation 语义；Gateway 只选择 session-bound capability 并管理连接生命周期。

安全：通过。Read 与 mutation 分离；unknown drift、stale decision、cross-session、tampered evidence 和 transparent cross-project reconnect 全部 fail-closed。

AI 自由：通过。产品日常使用仍由逐工具 Interface 提供组合自由；但一次性 Gate F-A 是候选验收协议，必须严格遵守 frozen plan，不允许自由改序、插入工具或改变参数来源。ProjectPatch/Controlled SourcePatch 产品逃生通道保持不变。

真实验收诚实性：第三次修订后通过。production exact shared-texture evidence 和真实 Codex transcript 是 Gate F 必需事实；自建客户端/deterministic capture 只能做 preflight。三次 254-R1 F-A 失败与本次 `failed_contract_mismatch` E-R 分别保留为历史证据。新候选必须通过无环 digest DAG、exact Candidate prepare receipt 与 acceptance plan 的机器验证；不能用手工 manifest、历史常量或重试掩盖冻结、人工协作或操作计划缺口。

Fixture：通过。选择 production Editor 已链接 RuntimeModule 的 disposable complex-shooter；不把正确的 runtime-not-linked 失败误判为重复授权问题。

冻结拓扑：通过。内层 binding manifest 不引用任何下游 artifact；plan 只引用内层 digest；外层 manifest 引用内层与 plan；freeze marker 只引用外层，依赖图无环且可从唯一入口完整重放验证。

失败原子性：通过。actual config 变更发生后任一失败都必须按本次 receipt rollback；final root 仅同卷原子发布，失败 staging 不带 `freeze.json`，不会被 Gate F 误识别为 candidate。

Production producer truth：第四次修订后通过。source/build/fixture/config/editor preflight/cleanup 均有唯一 schema、输入 lineage 和独立 digest 复验；typed MCP、reconnect、real-WGPU 与 Candidate prepare 必须来自同一 production Editor preflight receipt，Cargo/test log、临时脚本、外部 JSON-RPC transcript 或 `producerKind=test_adapter` 不能进入冻结 DAG。

进程与 timeout：通过。所有 production 子进程进入 kill-on-close Windows Job Object；总 deadline、阶段 cap 与 cleanup reserve 由 Module 控制，caller 不能逐阶段延时或重试。timeout 后残留任一 cargo/editor/MCP 后代直接归类 `cleanup_failed`。

Candidate input ownership：通过。落盘 exact direct input 由 fresh candidate id 和 fixture binding 生成，不含 caller/session-owned内部字段；context hash 只在同一 production session decode 后由 Gateway 绑定并进入 prepare receipt，不回写 direct input。

当前施工状态：`Gate R6 passed / new freeze authorization pending`。R6-A/B identity 与共享 verifier、R6-C production stage producers/Editor preflight/完整 freezer transaction，以及 R6-D 受影响域回归均已通过。本状态不构成 fresh freeze、F-A/F-B、Gate G 或三引擎 B 通道授权。

## 11. 正式结论

254-R1 不再继续旧 Gate F 或 E-R 重试循环。第三次修订采用 `binding manifest -> acceptance plan -> outer candidate manifest -> freeze marker` 的无环冻结 DAG；第四次修订把 production source/build/fixture/editor preflight/config/cleanup producer、receipt schema、process-group、deadline 与失败 artifact ownership 补为可施工合同，两次修订均已由用户确认。Gate R6 的实现、受影响域回归与真实 inventory/config 边界核验已完成，当前状态为 `Gate R6 passed / new freeze authorization pending`。此前唯一一次 fresh E-R 已固定失败且不得重试；另获新的明确单次 freeze 授权前不得运行 production freezer 或创建 candidate。新的 acceptance plan、candidate 和 artifact schema 全部冻结前不得执行 Gate F；F-A 仍只能在 user-ready handshake 后开启 30 分钟窗口并严格消费一次批准与一次 frozen plan，Gate G 仍只在同一 fresh candidate 的真实 Gate F 通过后运行一次。
