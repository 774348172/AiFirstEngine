# 254-R1 历史施工文档：Real Codex Client Integration / User Acceptance Gate v1

> 状态：`superseded_by_254-R2`。本文档已从 `施工文档/当前/` 撤销，只保留历史证据；禁止继续执行其中任何 freezer、candidate、F-A/F-B、Gate G 或回归步骤。

> 状态：2026-07-20 Gate R6 已通过；无环 DAG、production stage producers、Editor preflight、完整 freezer transaction 与受影响域回归均已关闭；new freeze authorization `pending`，fresh candidate `not_created`，Gate F/G `not_started`  
> 原激活日期：2026-07-17  
> Gate F 修订日期：2026-07-19  
> Gate F 第二次修订日期：2026-07-20  
> 正式方案：`../../254-R1-Real-Codex-Client-Integration-User-Acceptance-Gate-v1方案.md`  
> 上游修订：`../../254-AI-Tool-Gateway-Codex-Adapter-v1方案.md` 第 30 节  
> 当前授权：Gate R6 施工与回归已完成并停止；未获新的明确单次授权前，不运行 production freezer、不创建 candidate、不执行 F-A/F-B、Gate G 或三引擎 B 通道

## 1. 施工目标

修正首次真实 Codex Gate F 暴露的结构性合同缺口：

```text
untyped raw aife_execute
session Read capability 不可达与 $active grant ambiguity
approval rows 截断/不可区分/stale lifecycle
Preview 没有 actual presented GameView frame evidence
visual capture/locate/explain 输入顺序倒置
production RuntimeModule composition 未被 process preflight 覆盖
project switch 后 reconnect 语义不明确
one-shot Gate F 只冻结 Candidate input，没有冻结完整调用计划
审批 TTL 缺少 user-ready handshake
acceptance artifact 与失败分类依赖人工拼装
evidence.read 的 prefix/root 与合法 consumer 边界不够明确
```

R1-R4 已形成历史实现基础。候选绑定的窄范围 `GateFAcceptancePlan/Validator`（Gate R5）已通过，已集中 allowlist、精确顺序/次数、参数来源、逐步断言、HITL readiness、失败 taxonomy、transcript 校验与 artifact 生成。下一步是在 R1-R5 全绿基础上单独授权并执行 fresh E-R；只有新的 plan/candidate/artifact schema 全部冻结后，才允许一次真实 Codex Gate F，再运行一次 Gate G 权威回归。

禁止继续旧点击循环、用 `I:\EngineTest\AiFirstGame` 配合当前 production Editor 做 Preview、或把旧 transcript/旧批准/新 binary/新 screenshot 跨候选拼接。

## 2. 预计涉及文件

```text
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
rust/crates/editor_core/src/project_visual_diagnostics.rs
rust/crates/editor_core/src/project_preview_evidence.rs（新增）
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/ai_capability_tool_kernel_tests.rs
rust/crates/ai_tool_gateway/src/protocol.rs
rust/crates/ai_tool_gateway/src/core.rs
rust/crates/ai_tool_gateway/src/mcp_stdio.rs
rust/crates/ai_tool_gateway/src/named_pipe.rs
rust/crates/ai_tool_gateway/src/remote_adapter.rs
rust/crates/ai_tool_gateway/src/gate_f_acceptance.rs（新增，或职责等价的单一 Module）
rust/crates/ai_tool_gateway/src/acceptance_artifact.rs（新增或收敛现有 artifact 逻辑）
rust/crates/ai_tool_gateway/src/lib.rs
rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs
rust/crates/ai_tool_gateway/tests/gate_f_acceptance.rs（新增或等价定向测试）
rust/crates/editor_ui_model/src/ai_panel.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_model/src/tests.rs
rust/crates/editor_ui_renderer/src/panels/ai_panel.rs
rust/crates/editor_ui_renderer/src/tests.rs
rust/crates/editor_wgpu_renderer/src/viewport_texture.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/editor_window_winit/src/tests/native_real_window.rs
rust/crates/editor_host/src/main.rs
rust/crates/editor_host/tests/*
rust/crates/project_e2e_gate/tests/*
框架设计/引擎总体架构/254*.md
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/49*.md
框架设计/引擎总体架构/54*.md
框架设计/引擎总体架构/施工文档/README.md
README.md
```

实际只改满足 Gate 的最小文件集；发现同类 contract consumer 时先全仓搜索并收敛到共享 Registry/Core Interface，不逐调用点复制 schema 或 grant 选择。

`evidence.read` 的 schema/description 收紧可落在现有 `editor_core` contract registry 与 MCP projection 文件中；不得借 R5 新建第二份 Tool Catalog 或第二份 operation/receipt 真相。

## 3. 环境、隔离与 fixture

```powershell
$env:CARGO_TARGET_DIR='I:\EngineTest\BuildTargets\254-r1-revision'
Set-Location G:\gameEngin\rust
```

```text
OS：Windows。
工作树：混合未提交变更；不得回退或纳入无关改动。施工记录维护 scoped source manifest。
定向/受影响域：使用独立 CARGO_TARGET_DIR，不与旧 Gate F binary 混用。
Gate F fixture：samples/complex_shooter_project 的 disposable copy。
Gate F/G：fresh frozen source、binary、Codex config、fixture 和 evidence manifest 必须一致。
```

`I:\EngineTest\AiFirstGame` 要求 `project.c01.runtime`，当前 production Editor 未链接该 RuntimeModule。本轮不改该项目历史完成事实，只停止把它当作当前 production Editor Preview fixture。

## 4. Gate R0：Formal Contract Correction / Baseline

任务：

- 更新 254 第 30 节、254-R1 正式方案、本文与所有当前入口；
- 把旧 Gate E 标记为 `superseded_by_gate_f_revision`；
- 把旧 candidate digests 标记为 historical，不再作为 acceptance authority；
- 停止旧 Editor/MCP/Codex acceptance process；
- 运行最小 baseline，记录已有失败，不把新红测试和旧失败混淆。

Baseline：

```powershell
cargo fmt --check
cargo test -p ai_tool_gateway mcp_
cargo test -p ai_tool_gateway gateway_grant_ref
cargo test -p editor_core ai_capability_tool_kernel
cargo test -p editor_window_winit gateway_
```

预计成本：分钟级。任一已有测试失败时先固定失败清单；不进入完整 workspace。

## 5. Gate R1：Tool Contract Registry / Typed MCP Projection

任务：

- 深化现有 `tool_contract` 为单一 `AiToolContractRegistry`；
- 统一每个 descriptor 的 direct input schema、minimal example 与 canonical decode；
- Candidate 三种 payload 分支递归 strict；
- `McpToolProjection` 从 Catalog/Registry 生成逐工具 name/schema/description/annotations；
- 增加 `aife_status`、保留 `aife_catalog/aife_observe/aife_cancel`；
- `aife_execute` 从 `tools/list` 移除，仅在确有兼容需要时保留 strict legacy dispatch；
- typed Adapter 自动构造内部 invocation envelope，拒绝调用方 envelope/grant 字段。
- `projectPatchContextHash` 从 typed direct schema 移除；`ExecuteSessionBound` 必须从当前 `EditorSession` 捕获并覆盖绑定该内部事实，caller 提交同名字段返回 `-32602`；
- Candidate operation 入队前完成 session-owned facts binding 与 non-mutating prepare；失败直接返回 terminal result，不创建 durable operation；
- 为 ProjectPatch、ControlledSourcePatch、AssetImport 三个分支增加 executable conformance 测试，不能只断言 schema 中出现分支名称。
- frozen F-A payload 必须由当前 Rust `ProjectPatchDocument/ScenePatchOperation` 构造；禁止复用旧 transcript 中已过期的 `editor-project-patch.v1`。

先红测试：

```text
tools/list 不广告 aife_execute，广告每个 Catalog descriptor 的 typed tool。
每个 inputSchema 递归 additionalProperties=false。
每个 minimal example 同时通过 JSON Schema 与 strict Rust decode。
ProjectPatch direct input 不含且不能提交 projectPatchContextHash；Gateway 绑定后的内部 envelope 含当前规范 context digest。
三个 Candidate 分支均完成 schema-valid direct input -> canonical decode -> session binding -> non-mutating prepare。
ProjectPatch prepare 失败不会创建 queued operation，也不消耗一次真实 Gate 的等待时间。
projectIdentity/toolVersion/payloadKind/grantRef/未知 nested field 返回 -32602。
toolId 与 payload variant 映射不可能由 caller 伪造。
Catalog schema 与 MCP projection schema digest 一致。
```

定向命令：

```powershell
cargo test -p editor_core ai_tool_contract_
cargo test -p ai_tool_gateway mcp_projection_
cargo test -p ai_tool_gateway mcp_typed_tool_
cargo test -p ai_tool_gateway --test mcp_process_smoke
```

进入条件：R0 baseline 已记录。退出条件：所有 Catalog tools 都可由 strict typed MCP surface 调用，raw untyped entry 不再可发现。

## 6. Gate R2：Session Access / Status / Approval Lifecycle

任务：

- 协议新增 `SessionStatus` request/reply 与 typed `GatewaySessionStatus`；
- connect 时建立 session-bound Read generation；
- read 与 mutation grant 分开解析，不再共享 `$active`；
- same-session receipt 推进 observed digest；external drift 进入 stale，显式 inspect 后才刷新；
- mutation 每 session 最多一个 active grant；
- operation 保存 session + exact grant snapshot，cancel 只接 operationId；
- Gateway Core 集中 `approval_inbox/decide_access/session_status/prune`；
- EOF/close/TTL/project switch 清理 session/grant/UI row，stale click fail-closed；
- Native Editor 使用专用 `GatewayAccessRequestModel` 与唯一 command/hit id；
- 审批区域有界滚动/分页，删除 `.take(3)` 对 gateway rows 的截断。

先红测试：

```text
connect 后无需 execute，status 显示 read active、mutation awaiting_user。
未批准 mutation 时 search/visual/build/delivery 可用 session Read；mutation 被拒绝。
批准一个 session 不改变另一个 session。
同 session 新批准撤销旧 mutation grant，不产生 ambiguity。
external drift 不自动更新 read generation；inspect 后更新。
grant rotation 后仍可按 operation ownership cancel 旧 operation。
两个同版本 Codex 显示不同 session row；四行都可达且 hit id 唯一。
EOF/close/TTL/switch 后 row 消失、grant revoke、stale click 被拒绝。
```

定向命令：

```powershell
cargo test -p ai_tool_gateway gateway_session_status_
cargo test -p ai_tool_gateway gateway_session_access_
cargo test -p ai_tool_gateway gateway_approval_lifecycle_
cargo test -p editor_ui_model gateway_access_
cargo test -p editor_ui_renderer gateway_access_
cargo test -p editor_window_winit gateway_access_
```

退出条件：真实客户端可在一次 mutation 批准前后用 status 获得确定状态，不再重复 execute 探测或要求第二个 read approval。

## 7. Gate R3：Preview / Visual Evidence

任务：

- 新增 `ProjectPreviewEvidence` Module，集中 ticket、PNG/metadata write、digest 与读取验证；
- `EditorSession` 保存 pending Preview frame ticket/result；
- `project.preview` 在 Play 成功后进入 `running/awaiting_frame_evidence`；
- shared GameView texture 增加 `COPY_SRC`，实现 exact texture readback；
- real window 成功 render shared texture 后回填同 frame evidence；
- Tool Kernel 下一 pump 验证后才完成 Preview；
- 拆分 capture/locate/explain/trace inputs，调用者只传 evidence ref；
- 删除 production `present_package_smoke` synthetic fallback；
- tampered/stale/cross-project/cross-operation evidence fail-closed。

先红测试：

```text
Preview 在 frame receipt 前保持 running/awaiting_frame_evidence。
receipt project/operation/frame/hash 不匹配时失败。
PNG 可解码、尺寸/字节/digest 与 metadata 一致。
黑帧本身不因像素值被拒绝；无效长度/编码/readback 才失败。
篡改 PNG 或 metadata 后 runtime.capture_issue 拒绝。
runtime.capture_issue 不接受 caller supplied screenshot/frame digest。
capture -> locate -> explain -> owner trace 的 typed 输入顺序成立。
real-WGPU test 从 shared viewport texture readback，而不是另跑独立 render。
```

定向命令：

```powershell
cargo test -p editor_core preview_frame_evidence_
cargo test -p editor_core visual_evidence_ref_
cargo test -p editor_wgpu_renderer --features real-wgpu viewport_texture_readback_
cargo test -p editor_window_winit --features real-wgpu-surface preview_presented_frame_
cargo test -p editor_core ai_capability_tool_kernel
cargo test -p project_e2e_gate visual_symptom_diagnosis_
```

真实 GPU 测试如因环境 adapter 不可用，必须输出明确 local environment diagnostic；不能用 deterministic Adapter 代替 Gate F 权威证据。

## 8. Gate R4：Production Composition / Explicit Reconnect

任务：

- `editor_host` process preflight 使用与 production `default_editor_linked_project_runtimes()` 相同的三模块组合；
- disposable complex-shooter fixture 经真实 Named Pipe + production Editor Preview 绑定 `sample.complex-shooter.runtime`；
- project switch/reopen 后 old adapter 返回 `reconnect_required`；
- old MCP process 结束，新 process 得到不同 session id；
- Adapter 不透明 rediscovery。

先红测试：

```text
production three-module Editor 可 Preview complex-shooter。
production Editor 对 project.c01.runtime 明确 runtime_not_linked。
switch 后 old adapter 无法继续 status/read/mutation。
new adapter session id 不同且不继承旧 mutation grant。
F-B artifact contract 禁止第二次 mutation approval/full chain。
```

定向命令：

```powershell
cargo test -p editor_host real_editor_gateway_process
cargo test -p editor_host production_runtime_composition_
cargo test -p ai_tool_gateway gateway_explicit_reconnect_
cargo test -p project_e2e_gate gateway_
```

## 9. Gate R5：Exact Acceptance Plan / HITL / Artifact Validator

任务：

- 新增候选绑定的 `GateFAcceptancePlan/Validator`，只服务 254-R1 Gate F，不扩展为通用 Agent Planner 或统一产品 Runner；
- 冻结 plan schema/version/hash、candidate/manifest/config/fixture binding、允许的 typed MCP tools、精确顺序/次数、每步 literal input 或 output-derived 参数规则、assertions、禁止调用和 terminal/rollback 规则；
- 固定状态机：`binding_preflight -> read_discovery -> awaiting_user_ready -> awaiting_approval -> mutation -> post_mutation_inspect -> preview -> capture_issue -> locate -> explain -> owner_trace -> build -> delivery_verify -> rollback -> terminal`；
- user-ready handshake 成立后才能创建/进入 30 分钟 approval session，只消费一次 `ProjectOwnedLowRisk` 决定；
- Preview result 只能直接进入 `runtime.capture_issue`，明确从 Gate F allowlist 排除 `evidence.read`；
- 收紧 `evidence.read` typed schema/description，明确只接受 `project-evidence:` 且根限于 `Library/Reports/`、`Library/AiToolKernel/`，并说明它不是 Preview visual evidence consumer；
- 固定 failure taxonomy：`contract_mismatch`、`approval_timeout`、`authoritative_step_failed`、`operator_plan_violation`、`evidence_corrupt`、`cleanup_failed`；
- artifact 由 validator 根据 authoritative transcript/output 生成并校验，不允许操作员从单个 diagnostic 手工指定 terminal classification；
- mutation 后任一 terminal failure 都进入唯一一次 rollback；恢复 digest 失败时分类为 `cleanup_failed` 并保留原始 cause。

先红测试：

```text
计划外 extra tool（包括 Preview 后 evidence.read）在调用前被拒绝为 operator_plan_violation。
wrong order、missing step、wrong output path、literal/input-source drift 均 fail-closed。
duplicate approval、second mutation、retry authoritative step 均被拒绝。
未收到 user-ready 时不会开启 approval TTL；ready 后只允许一个 approval window。
Preview frameEvidenceRef 直接传给 runtime.capture_issue；wrong ref consumer 被拒绝。
operation/receipt/grant/project/frame/digest lineage 不一致时不能推进状态。
artifact 缺步、改写 transcript、手工 failure classification 或 digest 不一致时 finalize 失败。
evidence corruption 与 operator plan violation 可稳定区分。
mutation 后任何失败都执行 rollback；恢复 digest 不一致得到 cleanup_failed。
```

定向命令（实现时按现有测试模块的最终名称对齐，不用宽泛 workspace run 替代）：

```powershell
cargo test -p ai_tool_gateway gate_f_acceptance_plan_
cargo test -p ai_tool_gateway gate_f_acceptance_validator_
cargo test -p ai_tool_gateway acceptance_artifact_
cargo test -p editor_core ai_tool_contract_evidence_read_
cargo test -p ai_tool_gateway mcp_projection_evidence_read_
```

退出条件：所有 plan drift 与人工 artifact 漂移均在真实 Gate 前由机器拒绝；正常路径无需操作员解释下一步或选择 evidence consumer。

施工结果（2026-07-20）：`passed`。新增候选绑定的 strict plan/validator、30 分钟 user-ready/approval TTL 证据、literal/output-derived input rules、output assertions、固定 failure taxonomy、mutation 后 rollback 收敛与 transcript-validated v2 artifact；`evidence.read` Registry/MCP contract 已明确两个受控根及 Preview 正确 consumer。`cargo fmt --all -- --check`、`cargo test -p ai_tool_gateway`、`cargo test -p editor_core ai_tool_contract_`、`cargo test -p editor_core ai_capability_tool_kernel` 和 visual evidence 定向链均通过。未执行 E-R 或任何 Gate F/G。

## 10. Gate E-R：Affected Regression / Fresh Candidate Freeze

只有 R1-R5 全绿后进入。先跑受影响域：

```powershell
cargo fmt --check
cargo test -p editor_core ai_capability_tool_kernel
cargo test -p ai_tool_gateway
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_wgpu_renderer --features real-wgpu
cargo test -p editor_window_winit
cargo test -p editor_window_winit --features real-wgpu-surface
cargo test -p editor_host
cargo test -p project_e2e_gate gateway_
```

随后做环境等价预检：

```text
fresh/frozen scoped source manifest
exact Cargo.lock and feature set
I:\EngineTest\BuildTargets\254-r1-revision
disposable complex-shooter execution copy
fresh MCP/editor_host/config binaries and digests
typed tools/list snapshot and schema digest
frozen exact F-A reversible Candidate direct input（不含 session-owned context hash）
exact Candidate 在 disposable complex-shooter session 上的 decode/bind/non-mutating-prepare receipt
gate-f/acceptance-plan.json、schema version 与 sha256
exact expected step list、每步调用次数与 allowlist/forbidden calls
exact search query、visual symptom/query/expected document/node 规则
Build/Delivery arguments 与 rollback receipt output-source rule
HITL user-ready/approval TTL protocol
failure taxonomy 与 acceptance artifact schema/digest
real-WGPU Preview PNG/metadata smoke
old/new session reconnect smoke
cleanup report
```

实际 Codex config 若仍指向历史失败 candidate，先运行配置工具的受控 `replace`，参数必须同时给出 manifest 中的预期旧 MCP absolute path 与新 MCP absolute path。工具只在当前同名 entry 精确匹配预期旧路径时原子替换，并生成 before backup、install receipt；缺失、冲突或 drift 必须失败，禁止手工字符串替换绕过 receipt。隔离 config 仍先用 `install` smoke，实际 config 的 before/after digest、replace receipt 和 frozen fragment 一并进入 candidate manifest。

预检全绿后冻结全新 candidate；三个历史失败 candidate 均不得复用。candidate manifest 必须绑定 exact F-A Candidate input、prepare receipt digest、acceptance plan/hash、argument-source rules、failure taxonomy 与 artifact schema。除完成记录/入口同步外，代码、manifest、lockfile、schema、binary、config、fixture、F-A Candidate input、prepare receipt 或上述 plan facts 任一变化都使 freeze 失效并回到 E-R。

## 11. Gate F：One Real Codex Acceptance

进入条件：R5 与 fresh E-R 全绿，fresh candidate、`gate-f/acceptance-plan.json` 和 artifact schema 均已冻结，用户已明确 ready。必须由真实 Codex surface 加载 MCP；测试 JSON-RPC client 不能代替。

### F-A：同项目、同 MCP session、一次批准

```text
binding preflight verifies exact fixture/discovery/session/candidate/config hashes
aife_status -> catalog -> project inspect -> project search
user-ready handshake -> start approval TTL
Native Editor 一次批准 ProjectOwnedLowRisk mutation -> status confirms active
提交 frozen exact F-A Candidate direct input；Gateway 自动绑定 session context；Candidate mutation + receipt
project inspect -> new digest/read generation
Preview -> exact presented frame PNG/metadata
runtime.capture_issue -> ui.locate -> ui.explain_visibility -> trace owner
Build/Delivery Verify
rollback/cleanup -> restored digest
```

Validator 是唯一步骤 authority：只允许 plan 中的 typed tools，严格执行顺序、次数与参数来源；禁止任何额外调用，尤其禁止在 Preview 与 `runtime.capture_issue` 之间调用 `evidence.read`。不得重试 authoritative step、请求第二次批准或执行第二次 mutation。TTL 到期、拒绝、step failure、lineage/digest 无效或 plan drift 均立即进入 terminal failure；mutation 已发生时仍只执行计划内一次 rollback，并验证初始 digest。

`acceptance-artifact.json` 必须由 validator 从 transcript 和 authoritative outputs 生成、校验并原子冻结。原始 `evidence_ref_invalid` 等 diagnostic 不直接决定 failure class：wrong consumer/extra call 属于 `operator_plan_violation`；只有 plan-authorized consumer 对归属、bytes 或 digest 的验证失败才可分类为 `evidence_corrupt`。

### F-B：switch/reopen 后新 session

```text
switch/reopen
old adapter returns reconnect_required
old MCP process ends
new Codex task/MCP process starts
new session id differs from F-A
status + catalog + project inspect/search succeed
```

F-B 不批准 mutation，不重跑 mutation、Preview、visual 或 Build。F-A 失败后不得对同 candidate 重跑；F-B 只有在 F-A 通过后才可进入。

Artifact 至少记录：

```text
source/binary/config/fixture manifest
Codex Desktop version and config digest
typed tools/list and Catalog digest
F-A/F-B distinct session ids
project identity/root/digest/read generations
mutation grant/receipt/operation ids
Preview frame/PNG/metadata refs and digests
visual issue/locate/explain/owner refs
Build/Delivery result
elapsed time / diagnostics / cleanup
```

## 12. Gate G：一次权威回归与归档

Gate F 通过且 candidate 未变化后，在 immutable frozen source + disposable execution copy 运行：

```powershell
cargo test --workspace
cargo test --workspace --all-features
```

预计成本：最高；默认只运行一次 authoritative run。失败时从结构化 artifact 定位首个真实失败，在定向/受影响域收敛累计失败集后才能重新冻结并重跑；不得连续盲跑完整 workspace。

全部通过后：生成阶段完成记录，记录各验证层、authoritative run 次数/耗时/重跑原因、Gate F artifact 与 cleanup；同步 240/49/54/README/文档地图；把本文移动到 `施工文档/已完成/`。

## 13. 高风险否定矩阵

```text
raw untyped aife_execute remains advertised
Catalog/schema/example/Rust decode drift
ProjectPatch direct schema 接受 caller-supplied/null projectPatchContextHash
任一 Candidate 分支 schema/decode 通过但 session-bound prepare 失败
Candidate prepare-invalid 仍创建 durable queued operation
typed tool accepts caller envelope or grant fields
session Read missing or mutation can run before approval
Read/mutation share $active or cross session
external drift silently refreshes Read
stale/closed/expired approval row remains clickable
more than three approval rows become unreachable
Preview completes without exact shared-texture evidence
tampered/cross-project/cross-frame evidence accepted
visual diagnosis uses caller digest or present_package_smoke fallback
production process preflight uses empty EditorSession composition
AiFirstGame is previewed without project.c01.runtime-linked Editor
old Adapter transparently changes project binding
F-B repeats mutation/full chain
old transcript and new candidate artifacts are mixed
Build/Delivery escapes controlled roots
acceptance plan admits an extra tool, wrong order, missing step or duplicate approval/mutation
Preview frame evidence is sent to generic evidence.read instead of runtime.capture_issue
literal/output-derived argument source drifts from frozen plan
approval TTL starts without explicit user-ready handshake
operator plan violation is mislabeled as evidence corruption
acceptance artifact/failure classification is manually assembled or differs from transcript
terminal failure after mutation skips rollback or restored digest validation
```

## 14. 施工文档自审

方案一致性：通过。已先完成 254-R1 正式方案第二次修订，再逐项映射到本施工文档；不再执行已被真实 Gate F 否定的 `$active`/raw execute/伪 visual evidence/自由调用顺序合同。

范围：通过。只修真实 Codex Interface、session access、approval lifecycle、Preview evidence、production composition、reconnect 与候选绑定的一次性验收控制；不新增通用 Planner、统一产品 Runner、工具域、Installer 或 Unity/UE 重跑。

Deep Module：通过。Registry、MCP projection、Gateway Core approval/session access、ProjectPreviewEvidence 与 GateFAcceptancePlan/Validator 各自集中复杂度；后者以窄 Interface 隐藏 allowlist/order/argument lineage/HITL/transcript/artifact 规则，未侵入产品 Tool Kernel 真相。

安全：通过。read/mutation 分离，drift/stale/cross-session/tamper/cross-project/reconnect/plan drift 全部 fail-closed；mutation 后所有 terminal path 强制 rollback 与 restored digest 校验。

测试：通过。Gate R5 新增 extra tool、wrong consumer/order/source、missing/duplicate step、HITL、lineage、artifact classification 与 cleanup 否定测试；随后才跑受影响域，并只在 frozen candidate + Gate F 通过后运行一次 authoritative workspace。

环境：通过。固定 `CARGO_TARGET_DIR=I:\EngineTest\BuildTargets\254-r1-revision`，Gate F 使用 production-linked disposable complex-shooter，真实 GPU evidence 不由 deterministic Adapter 替代。

工作树：有条件通过。当前为混合未提交施工基线，必须维护 scoped source manifest，保留所有无关用户变更；freeze/权威运行使用等价隔离 source。

人工成本/HITL：通过。新 candidate 冻结前不请求用户点击；用户 ready 后才开启 30 分钟 approval session，只消费一次批准。F-B 只验证新 session read，不重复完整链。

Artifact truth：通过。Validator 从 authoritative transcript/output 生成并验证 artifact；区分 `operator_plan_violation` 与 `evidence_corrupt`，禁止人工从原始 diagnostic 改写 terminal classification。

自审结论：三次历史 F-A 分别固定为 `historical_failed_contract_mismatch`、`historical_failed_approval_timeout` 与 `historical_failed_operator_plan_violation`。第三次 Preview evidence 有效，失败来自错误 consumer/额外调用；不得误判为 evidence corruption。Gate R5 及其受影响域回归已通过；2026-07-20 唯一一次 fresh E-R 在创建 candidate 前因冻结合同循环绑定固定为 `failed_contract_mismatch`，不得重试。fresh candidate 尚未创建，Gate F/G 未开始；本轮没有创建或重跑 candidate，也没有执行 F-A/F-B/G 或三引擎 B 通道。

## 15. 2026-07-20 当前检查点

```text
Gate A-D historical implementation facts：retained_pass
Gate E original composite preflight：superseded_by_gate_f_revision
Gate R0 formal contract correction：passed
Gate R1-R4 automated revision：historical_passed implementation basis
Gate R5 acceptance plan/HITL/artifact validator：passed
Gate E-R fresh candidate freeze：failed_contract_mismatch（2026-07-20，仅执行一次，不重试）；fresh candidate not_created
Gate F real Codex acceptance：candidate-20260719-141221=historical_failed_contract_mismatch；candidate-20260719-210044=historical_failed_approval_timeout；candidate-20260720-000200=historical_failed_operator_plan_violation；fresh Gate F=not_started
Gate G authoritative regression：not_started
```

旧候选摘要仅保留为历史诊断，不再具有 acceptance authority：

```text
ai_engine_gateway_mcp.exe sha256=5B6DCAA01E7CEE117E8BBE00C047B6C2F65AF8D06BF17319440C8B9FABBD9EA6
ai_engine_gateway_codex_config.exe sha256=3D862418E514D16AF5F3145817290A291FAAAAC0D56B2085F5DE236026EDA6E9
editor_host.exe sha256=062582ED8DB4F88BC23DAE796373A8FA2589F63F78D9B1109DB8D930EA64DB43
Codex config sha256=0AA738AC9B5D1BC1D207420FD2D97184CC7EBB08E645D8EE511A5F4AB570E73E
install receipt sha256=94E8F554D36F41D48AE3F824F7372A9F51F5B2717B1168A075FAD5E903C9C84F
status=historical_rejected_candidate
```

三个旧 candidate 均固定为历史失败且不得修改或重试。`candidate-20260720-000200` 已完成 rollback，项目恢复初始 digest；其 Preview evidence 有效，但完整 visual/Build/Delivery 链未完成，F-B/G 未开始。Gate R5 已通过。2026-07-20 唯一一次 fresh E-R 已失败并停止；没有创建 fresh candidate，也没有执行 F-A/F-B/G 或三引擎 B 通道。

### 13.1 Fresh E-R 单次执行记录（2026-07-20）

受影响域按第 10 节顺序各执行一次并全部通过：

```text
cargo fmt --check：passed
editor_core ai_capability_tool_kernel：30 passed
ai_tool_gateway：48 unit + 1 reconnect + 10 Gate R5 + 2 MCP process passed
editor_ui_model：56 passed
editor_ui_renderer：32 passed
editor_wgpu_renderer --features real-wgpu：20 passed
editor_window_winit：118 passed
editor_window_winit --features real-wgpu-surface：128 passed，9 local-only ignored
editor_host：18 unit + 1 CLI + 1 gateway process passed
project_e2e_gate gateway_：2 passed
```

环境等价预检在创建 candidate 前 fail-fast：`GateFAcceptancePlan.candidateManifestDigest` 要求绑定最终 candidate manifest digest，而第 10 节又要求 candidate manifest 绑定 `gate-f/acceptance-plan.json` 的 SHA-256，形成最终字节的 SHA-256 循环依赖。现有 `gate_f_candidate_preflight` 同时硬编码历史 `r2` candidate 标识且不生成 R5 acceptance plan，不能作为 fresh freezer 绕过该合同。

```text
classification=failed_contract_mismatch
freshERAttemptCount=1
retryAllowed=false
freshCandidateCreated=false
codexConfigChanged=false
fixtureMutated=false
gateFExecuted=false
gateGExecuted=false
threeEngineBExecuted=false
```

该失败已触发第三次正式方案修订；用户已确认无环内层 binding manifest、外层 candidate manifest 与版本化 Rust fresh preflight/freezer 方向。对应施工任务与停止点见第 14-15 节。

## 14. 第三次修订施工：Gate R6 Acyclic Freeze Identity / Freezer Module

### 14.1 施工目标与边界

已确认正式方案采用以下单向 DAG：

```text
source/binary/config/fixture/tools/schema/input/prepare receipt
  -> candidate-binding-manifest.json
  -> gate-f/acceptance-plan.json
  -> candidate-manifest.json
  -> freeze.json
```

本 Gate 只完成 schema、验证器、freezer Module 与自动化测试。唯一一次 fresh E-R 已失败且不得重试；本 Gate 不运行实际 freeze，不创建 candidate，不修改真实 Codex config，不启动真实 F-A/F-B/G 或三引擎 B 通道。

涉及文件：

```text
rust/crates/ai_tool_gateway/src/gate_f_acceptance.rs
rust/crates/ai_tool_gateway/src/frozen_candidate.rs（新增）
rust/crates/ai_tool_gateway/src/candidate_freezer.rs（新增）
rust/crates/ai_tool_gateway/src/lib.rs
rust/crates/ai_tool_gateway/tests/gate_f_acceptance.rs
rust/crates/ai_tool_gateway/tests/candidate_freezer.rs（新增）
rust/crates/ai_tool_gateway/examples/gate_f_candidate_preflight.rs（删除历史硬编码或改为新 Module 的薄入口）
```

### 14.2 Gate R6-A：红测试 / Identity Schema

先新增并运行红测试，证明当前 `candidateManifestDigest` 循环字段和缺失 DAG schema 无法满足正式方案：

```powershell
cargo test -p ai_tool_gateway --test gate_f_acceptance gate_f_acceptance_plan_uses_acyclic_binding_identity
cargo test -p ai_tool_gateway --test candidate_freezer frozen_candidate_
```

实现要求：

- `GateFAcceptancePlan.candidateManifestDigest` 改为 `candidateBindingManifestDigest`；
- `GateFAcceptanceBinding` 与 validated artifact 同时记录 `candidateBindingManifestDigest`、`candidateManifestDigest`；
- schema version 升级，旧 plan 必须 strict reject，不做静默兼容；
- plan/binding/artifact validation 分别验证内层与外层 digest，禁止混用；
- 17 步、HITL、failure taxonomy、rollback 与 transcript 语义不得变化。

退出条件：定向 schema 测试全绿，旧字段、unknown field、内外 digest 互换全部拒绝。

### 14.3 Gate R6-B：FrozenCandidateVerifier

新增 strict serde schema：

```text
CandidateBindingManifest
FrozenCandidateManifest
CandidateFreezeMarker
FrozenCandidateVerificationReport
```

唯一验证 Interface：

```text
FrozenCandidateVerifier::verify(freeze_path) -> FrozenCandidateVerificationReport
```

验证顺序固定为 `freeze -> outer manifest -> plan/binding manifest -> leaves`。所有引用必须是 candidate root 内规范相对路径；拒绝 absolute、`..`、symlink/junction/reparse escape、unknown field、schema/version drift、digest mismatch、binding manifest 下游反向引用、plan 外层 manifest 引用和缺失 cleanup/config receipt。验证器是 freezer 发布后复验与未来 Gate F binding preflight 的唯一共享真相。

测试：

```powershell
cargo test -p ai_tool_gateway --test candidate_freezer frozen_candidate_
```

退出条件：完整 DAG 通过；每层 tamper、路径 escape、循环/反向引用与历史 identity 复用均 fail-closed。

### 14.4 Gate R6-C：CandidateFreezer Deep Module

外部 Interface 固定为：

```text
CandidateFreezer::run(FreshCandidateFreezeRequest) -> FreshCandidateFreezeReport
```

request 只暴露 candidate id/final root、source root、`CARGO_TARGET_DIR`、fixture source、actual config path、expected current MCP path 与总 timeout。staging/discovery/evidence/config artifact 子路径、阶段顺序、rollback 和 quarantine 都封装在 Module 内；filesystem/process/clock/config 测试 Adapter 仅为 `pub(crate)` 内部 seam。

阶段 journal 固定覆盖：reserve、source/lock/features、release binaries、fixture、isolated config、typed surface/reconnect/WGPU、typed Candidate prepare、actual config compare-and-replace、binding manifest、plan、cleanup、outer manifest、freeze、atomic publish、reopen verify。生产 run 任何失败都输出结构化 report；actual config 已改变时只按本次 receipt rollback；publish 前不得出现 final root/`freeze.json`；publish 后复验失败不得改冻结字节，必须 rollback config 并原子 quarantine。

本轮不调用生产 run。测试通过内部 Adapter 验证阶段次序与失败原子性：

```powershell
cargo test -p ai_tool_gateway --test candidate_freezer candidate_freezer_
```

退出条件：成功路径一次发布并复验；每阶段故障均完成正确 rollback/cleanup，重复 id/root 和历史 inventory 均拒绝。

### 14.5 Gate R6-D：受影响域回归

进入条件：R6-A/B/C 定向测试全绿。按成本顺序执行：

```powershell
$env:CARGO_TARGET_DIR='I:\EngineTest\BuildTargets\254-r1-revision'
cargo fmt --all -- --check
cargo test -p ai_tool_gateway --test gate_f_acceptance
cargo test -p ai_tool_gateway --test candidate_freezer
cargo test -p ai_tool_gateway
cargo test -p editor_core ai_tool_contract_
cargo test -p editor_core ai_capability_tool_kernel
```

本 Gate 不重复 UI/WGPU/window/host/project E-R 矩阵；这些属于获得新单次 freeze 授权后的环境等价预检，不能由本轮定向施工冒充。若共享 schema 改动触及跨 crate consumer，再扩展对应 consumer 测试，不直接运行 Gate G。

退出条件：上述矩阵全绿；candidate 目录计数不变，真实 Codex config digest 不变，无 editor/MCP 残留进程。

### 14.6 文档同步与停止点

R6 完成后只同步 `240/49/54/施工文档 README` 与本施工记录，状态写为：

```text
Gate R6 passed
new freeze authorization pending
fresh candidate not_created
Gate F/G not_started
```

不得把 R6 通过写成 fresh E-R、candidate freeze 或 Gate F 通过。用户未另行授权前立即停止。

## 15. 第三次修订施工文档自审

方案一致性：通过。第 14 节逐项实现已确认正式方案第 4.10 节无环 DAG、第 7 节 R5/E-R 修订与第 8-9 节验证/高风险矩阵；不改变 F-A 17 步、HITL 或 Tool Kernel ownership。

范围：通过。只修改 `ai_tool_gateway` identity schema、共享 verifier、freezer 与测试；不新增通用 Planner/Runner，不修改 Editor 产品行为，不创建 candidate 或执行 Gate。

Depth/Locality：通过。freezer 外部只有一个 request/result Interface，DAG verifier 只有一个 verify Interface；复杂阶段、side effects、rollback、cleanup 和原子发布留在 Module 实现内部，测试 Adapter 不泄露给调用方。

验证分层：通过。R6-A/B/C 是定向红绿测试，R6-D 是受影响域回归；真实环境等价 freeze 与 Gate G 明确留到新授权，未用定向测试冒充权威证据。

历史失败继承：通过。三份历史 candidate 和唯一一次失败 fresh E-R 均只作否定测试来源，不复用 id/root/payload/invocation/config receipt，不修改历史 artifact。

失败原子性：通过。施工测试覆盖 config rollback、无半发布、post-publish quarantine 与 cleanup failure；生产 freezer 本轮不运行，因此真实 config/fixture/candidate 无副作用。

当前代码基线：有条件通过。`ai_tool_gateway` 当前为未跟踪混合施工基线，必须只编辑第 14.1 节文件并保留其它用户改动；固定独立 `CARGO_TARGET_DIR`，不建立会丢失当前未提交真相的假 clean baseline。

自审结论：施工文档与第三次正式方案一致、范围闭合、测试与停止点明确。用户已授权自审后直接施工 Gate R6；可以开始 R6-A 红测试，禁止越过到真实 freeze 或 Gate F/G。

## 16. Gate R6 施工执行与实现自审（2026-07-20）

### 16.1 已完成

```text
R6-A identity schema：passed
  GateFAcceptancePlan 使用 candidateBindingManifestDigest；旧 candidateManifestDigest strict reject。
  binding/artifact 同时持有 inner binding digest 与 outer candidate manifest digest。
  validator 对 active binding 显式校验 inner/outer identity；互换、outer drift 均拒绝。

R6-B FrozenCandidateVerifier：passed
  freeze -> outer manifest -> binding/plan -> leaves 的单向 DAG 复验已实现。
  strict schema/version、canonical relative path、canonical containment、实际 SHA-256、candidate identity、
  binding-plan/config receipt linkage、17-step summary 与 forbidden evidence.read 均 fail-closed。
  完整 DAG、各 digest 层 tamper、absolute/parent/non-normalized path、unknown field、identity drift、
  config receipt drift 测试通过。

R6-C transaction implementation：partially_passed
  CandidateFreezer 外部 Interface 已收敛为 run(request) -> report。
  内部阶段顺序、journal、同卷 staging、atomic publish、shared-verifier reopen、发布前 cleanup、
  config rollback 与发布后 quarantine 已实现并通过 fault-injection Adapter 测试。
  历史 gate_f_candidate_preflight 的 r2 常量已删除，example 仅保留为新 Module 薄入口。
```

### 16.2 验证结果

固定 `CARGO_TARGET_DIR=I:\EngineTest\BuildTargets\254-r1-revision`，顺序执行：

```text
cargo fmt --all -- --check：passed
cargo test -p ai_tool_gateway --test gate_f_acceptance：13 passed
cargo test -p ai_tool_gateway --test candidate_freezer：4 passed
cargo test -p ai_tool_gateway：54 unit + 4 freezer integration + 1 reconnect + 13 Gate F + 2 MCP process passed
cargo test -p editor_core ai_tool_contract_：4 passed
cargo test -p editor_core ai_capability_tool_kernel：30 passed
```

该矩阵只记为 R6 受影响域预回归。由于 R6-C production Adapter 未闭合，不得把它记录为 Gate R6-D 正式退出，也不得同步入口为 `Gate R6 passed`。

### 16.3 实现自审发现的合同缺口（已由第四次正式修订解除）

正式方案规定 production freezer 必须收集 release binaries、isolated config receipt、typed tools/catalog/schema、reconnect、real-WGPU、active-session Candidate `decode -> bind -> prepare`、actual config compare-and-replace 与 cleanup 等 authoritative receipts；但当前方案与代码基线没有为这些 producer 定义可由 freezer 调用的版本化 Rust Interface、结构化输出 schema、阶段 timeout/进程终止合同及 receipt ownership。现有可复用实现只有 config compare-and-replace/rollback；测试与历史临时流程不能冒充其它 production receipts。

production Adapter 当前仍固定返回 `gateway.candidate_freezer.production_evidence_unavailable`，并已验证不会发布 final root、不会改变 config、不会留下 staging。2026-07-20 用户已确认第四次正式方案补充的 producer Interface、receipt schema、process-group、deadline、cleanup 与 failure-root 合同；以下禁令持续到对应 production 实现与测试全绿：

```text
手写或填充占位 receipt
由 shell/临时 PowerShell 拼装 candidate manifest
把测试 Adapter 当作 production freezer
创建 fresh candidate 或执行 E-R/F-A/F-B/G/三引擎 B 通道
```

### 16.4 自审结论

Schema/DAG verifier 与 freezer 事务实现符合 Depth/Locality、失败原子性和本轮副作用边界；production evidence collection 合同现已补齐并确认，但代码尚未实现，所以 Gate R6 仍未完成。下一步按第 17 节继续 R6-C production Adapter 红测试与实现；R6 全绿前不能申请单次 fresh freeze 授权。

边界复核：candidate inventory 仍仅有三个历史目录；真实 Codex config sha256 仍为 `cc06fadef20c778572f1f4d6d85c6e02ac52133ad16fe93c5269bd945102d269`；本轮未创建 candidate、未运行 production freezer、未执行任何 Gate F/G 或三引擎 B 通道。

## 17. 第四次修订施工：Production Stage Producers

### 17.1 范围与涉及文件

只完成 R6-C production Adapter，不运行它指向真实 candidate/config 的成功路径。预计涉及：

```text
rust/crates/ai_tool_gateway/src/candidate_freezer.rs
rust/crates/ai_tool_gateway/src/candidate_freeze_production.rs（新增）
rust/crates/ai_tool_gateway/src/frozen_candidate.rs
rust/crates/ai_tool_gateway/src/lib.rs
rust/crates/ai_tool_gateway/tests/candidate_freezer.rs
rust/crates/runtime_cli/src/bounded_child_process.rs
rust/crates/runtime_cli/tests/bounded_child_process.rs
rust/crates/editor_host/src/main.rs
rust/crates/editor_host/src/candidate_freeze_preflight.rs（新增）
rust/crates/editor_host/tests/gateway_process.rs
必要时 editor_window_winit/editor_wgpu_renderer 的现有 production present seam
```

不得修改 Tool Kernel mutation 语义、Gate F 17 步、fixture 项目内容或真实 Codex config。若 real-WGPU producer 需要新 seam，只允许暴露 exact presented-frame capture 所需的窄 Interface，不新增通用 UI Runner。

### 17.2 R6-C1：receipt schema 与 strict lineage 红绿测试

先为七类 receipt、production/test producer kind、stage failure、journal input/output digest 与 canonical path/digest 编写 strict decode/unknown-field/inner-outer reference 否定测试。必须验证 test Adapter receipt 无法进入 production binding manifest。

```powershell
cargo test -p ai_tool_gateway --test candidate_freezer candidate_freeze_receipt_
cargo test -p ai_tool_gateway candidate_freeze_receipt_ --lib
```

退出条件：所有 receipt schema/version/path/digest/input lineage 可机器复验；不得以 `serde_json::Value` 或日志字符串作为生产 receipt 真相。

### 17.3 R6-C2：process group、source/build/fixture producers

先扩展 `runtime_cli` Windows Job Object process-group Adapter，覆盖 child/grandchild timeout/cancel/wait failure kill-and-reap。再实现 source snapshot、locked release build、binary copy/hash 与双 fixture snapshot producer；build 只从 staging `source/` 读取并显式传播固定 target/env。

```powershell
cargo test -p runtime_cli --test bounded_child_process
cargo test -p ai_tool_gateway candidate_freeze_source_
cargo test -p ai_tool_gateway candidate_freeze_release_
cargo test -p ai_tool_gateway candidate_freeze_fixture_
```

退出条件：reparse/path escape、读取期漂移、lock/features drift、missing binary、source-after-build drift、child/grandchild leak 全部 fail-closed；测试只使用隔离 root/config，不触碰真实 candidate inventory。

### 17.4 R6-C3：production Editor preflight producer

新增 `editor_host` 版本化 `--candidate-freeze-preflight` producer mode 和 strict output receipt。一次进程中使用 production three-RuntimeModule session 与本次 release MCP binary，验证 first typed session、受控 switch/reopen、old reconnect/exit、distinct second session、real-WGPU exact frame、fresh direct input decode/bind/non-mutating prepare。direct input 文件必须断言没有 `projectPatchContextHash` 或其它内部字段。

```powershell
cargo test -p editor_host --test gateway_process candidate_freeze_preflight_
cargo test -p editor_wgpu_renderer --features real-wgpu project_preview_
cargo test -p editor_window_winit --features real-wgpu-surface candidate_freeze_
cargo test -p ai_tool_gateway --test mcp_process_smoke
```

退出条件：联合 receipt 同时绑定 fixture/binary/session/operation/frame/input/prepare lineage；deterministic capture、test producer、日志-only、跨 session 或跨 project 拼接全部拒绝。测试路径不得请求 approval 或执行 mutation。

### 17.5 R6-C4：config guard、cleanup、failure root 与完整 transaction

接入现有 config install/replace/rollback，config 写成功后立即注册内存 rollback guard；实现 total deadline/stage cap/120 秒 cleanup reserve、failed/quarantine inventory、cleanup receipt、manifest pure producer 与 production reopen verification。测试 Adapter 对每阶段注入 before/after-side-effect failure。

```powershell
cargo test -p ai_tool_gateway candidate_freezer_
cargo test -p ai_tool_gateway --test candidate_freezer
cargo test -p editor_host --test gateway_process
```

退出条件：成功测试路径只发布一次并由共享 verifier reopen；publish 前失败留下无 `freeze.json` 的同卷 failed artifact 且 config/进程/fixture 全恢复；publish 后失败先 rollback config 再原子 quarantine；历史/failed/quarantine id inventory 均拒绝。

### 17.6 R6-D 回归与停止点

R6-C1/C2/C3/C4 全绿后，重新执行第 14.5 节矩阵，并扩展：

```powershell
cargo test -p runtime_cli
cargo test -p editor_host
cargo test -p editor_wgpu_renderer --features real-wgpu
cargo test -p editor_window_winit --features real-wgpu-surface
```

本轮仍不得调用真实 `CandidateFreezer::run` 创建 candidate。全部通过后才同步 `Gate R6 passed / new freeze authorization pending`；任何失败先收敛本 Gate，不进入真实 E-R、F-A/F-B、Gate G 或三引擎 B 通道。

## 18. 第四次修订施工文档自审

方案一致性：通过。第 17 节逐项映射正式方案第四次修订的 producer、receipt、process-group、deadline、cleanup/failure-root 合同，不改变无环 DAG 与 Gate F 17 步。

Depth/Locality：通过。caller 仍只调用一个 freezer Interface；production producer 与测试 Adapter 都在内部 seam，Editor preflight 用一个联合 receipt 消除 typed MCP/WGPU/prepare 跨脚本拼接。

证据诚实性：通过。命令 exit code、Cargo test log、PowerShell、外部 JSON-RPC transcript、test producer kind 均不能单独进入 binding manifest；每个 receipt 和引用文件由 freezer 二次验证。

安全与副作用：通过。当前修订只授权后续代码施工与隔离测试，不授权 production freezer 成功运行；真实 config、历史 candidate 和 fixture source 继续只读。config guard、Job Object、cleanup reserve、failed/quarantine root 防止部分副作用被误认成 candidate。

验证：通过。按 schema -> process/source/build/fixture -> joint Editor preflight -> config/cleanup/transaction -> affected regression 分层，未把昂贵 production freeze 当作调试循环。

自审结论：第四次修订合同和施工拆分可执行、无循环引用、没有扩展为通用 Runner。后续施工结果见第 19 节。

## 19. Gate R6-C1/C2/C3/C4 与 R6-D 完成记录（2026-07-20）

### 19.1 实现结果

R6-C1 通过：七类 production receipt 使用 strict versioned schema，拒绝 unknown field、路径/digest/input lineage 漂移和 test producer 混入 production binding。

R6-C2 通过：Windows Job Object 覆盖 child/grandchild 的 timeout/cancel/wait-failure kill-and-reap；source snapshot、locked release binaries 与 clean/editor-run fixture producers 均实现边界、漂移和 digest 校验。

R6-C3 通过：`editor_host --candidate-freeze-preflight` 完成版本化 closed arguments、真实 release MCP 两段 session/reconnect、real-WGPU exact presented-frame readback，以及 fresh direct input 的 decode/bind/non-mutating prepare；caller-owned internal field 被拒绝。

R6-C4 通过：production Adapter 已接入 total/stage deadline、120 秒 cleanup reserve、inventory rejection、config guard/rollback、publish-before `.failed`、post-publish `.quarantine`、联合 receipt lineage、17 步 acceptance plan、cleanup receipt 与共享 reopen verifier。`CandidateFreezer::run(request) -> report` 仍是唯一外部 Interface。

### 19.2 R6-D 回归结果

全部命令使用固定 `CARGO_TARGET_DIR=I:\EngineTest\BuildTargets\254-r1-revision` 串行执行：

```text
cargo fmt --all -- --check：passed
cargo test -p ai_tool_gateway --test gate_f_acceptance：13 passed
cargo test -p ai_tool_gateway --test candidate_freezer：5 passed
cargo test -p ai_tool_gateway：passed（library 63 passed，全部 integration suites passed）
cargo test -p editor_core ai_tool_contract_：4 passed
cargo test -p editor_core ai_capability_tool_kernel：30 passed
cargo test -p runtime_cli：passed（23 unit + 7 bounded child + 4 exported process）
cargo test -p editor_host：passed（21 unit + 1 CLI + 2 gateway process）
cargo test -p editor_wgpu_renderer --features real-wgpu：20 passed
cargo test -p editor_window_winit --features real-wgpu-surface：128 passed，9 local-only smoke ignored，0 failed
```

### 19.3 边界与停止点

真实 candidate inventory 仍精确为 `candidate-20260719-141221`、`candidate-20260719-210044`、`candidate-20260720-000200`，没有新增 `.staging`、`.failed` 或 `.quarantine` root。真实 Codex config SHA-256 仍为 `cc06fadef20c778572f1f4d6d85c6e02ac52133ad16fe93c5269bd945102d269`。

Gate R6 结论：`passed`。本轮未调用真实 production freezer、未创建/freeze candidate、未修改真实 Codex config、未执行 E-R、F-A/F-B、Gate G 或三引擎 B 通道。下一状态固定为 `new freeze authorization pending`；254-R1 整体不归档。
