# 256 已终止施工文档：AI Project Bootstrap / Mutation Access Handshake v1

> 终止状态：`superseded_by_256_project_create_reset`。2026-07-24 用户确认旧 256 三工具方案及其全部实现废弃。本施工文档、全部 Gate、授权、candidate、Runner 和 artifact 仅保留历史诊断价值，不得恢复、继续、重试或作为新 256 的施工前置。
>
> 本文移入 `施工文档/历史/` 后，`施工文档/当前/` 不再存在有效 256 施工。新的 `project.create` 施工必须等待新正式方案确认、单独编写施工文档并完成自审和激活。

## 0. 文档状态

```text
正式方案：256-AI-Project-Bootstrap-Mutation-Access-Handshake-v1方案.md
方案选择：方案 B
文档状态：激活前复核通过，已激活
施工状态：施工中（fresh P8Q `058d4b2…` 已执行一次并 terminal failed；下一步需单独授权 scoped gateway_process/Preview descendant ownership remediation；P8F/P8A/P9/P10 未授权）
当前位置：施工文档/当前/
当前施工授权：fresh P8Q 已执行一次且授权已消费；remediation/P8F/P8A/P9/P10 均未授权
激活前 source 基线：commit e7478ab96033cd50b580744d37b27154c0ffdf26，tree ba6623dec839791b57b67660ac46387e40406731，工作树 clean
```

历史激活说明：本文成为唯一当前施工文档后，原始开始施工授权允许按 Gate 顺序执行 P0-P8；截至 2026-07-24，六次一体化 P8 授权均已消费。该历史说明不构成额外 P8Q、P8F、P8A、P9 或 P10 授权。

最新授权终态：P8R1/P8R2 已收敛第五次 fresh P8 的 Windows Shell 缺陷；随后单次 fresh P8 candidate `198373e…` 在固定矩阵第 35 个 `project_e2e_gate --all-features` 失败，`p9Eligible=false`。本次授权已消费且不得重跑。P8S1 只修订 Qualification/Freeze 边界；后续每个代码、验证、冻结或激活切片必须重新授权。

## 1. 施工目标

本施工只实现 256 方案 B：

1. Native Editor 在 launcher/no-project 状态也能发布受限 Gateway discovery 和 production typed MCP surface。
2. 唯一 `AiToolContractRegistry` 增加 `project.create`、`project.open`、`session.request_mutation_access` 三个目标级 Tool。
3. `project.create` 与 `project.open` 只调用正式 Editor-owned `UiCommandPayload::CreateProject/OpenProject`。
4. 路径、名称、manifest、link/reparse 与 containment 规则在副作用前 fail-closed。
5. Create/Open 成功后产生权威 project identity/root digest、bootstrap receipt 和新的 discovery publication。
6. launcher/旧项目 Session 不原地偷换 binding；旧 Session、Grant、operation 与 receipt 明确 stale/reconnect-required。
7. `session.request_mutation_access` 只创建 bounded goal-level `awaiting_user` request。
8. Native Editor 继续是批准/拒绝唯一 owner；未批准不能 active。
9. 255 Catalog 正确解释三个新 Tool 与现有项目内 Tool 的 availability；execute 按最新 owner 事实重检。
10. production MCP conformance 从真实 Native Editor launcher 开始，到 fresh project binding 与一次真实用户授权决议结束。

最终公共认知负担保持为：

```text
project.create
project.open
session.request_mutation_access
```

SafeProjectPath、Editor Command、discovery publication、Session rebind、approval authority、receipt 和 cleanup 都是 Tool 内部 Implementation seam，不是 AI 需要遵守的跨工具菜谱。

## 2. 明确不施工的内容

```text
不增加 start_task、Runner、Workflow、Planner、nextToolIds 或目标完成状态机。
不自动生成或修改游戏内容。
不在 Create/Open 后自动 inspect、request access、mutation、Preview 或 Build。
不自动批准、模拟 Native Editor 点击或绕过 awaiting_user。
不建立任意文件系统、shell、反射式 Editor internals 或项目根外写入 Tool。
不让 caller 提交 projectIdentity、root digest、generation、approvalId、grantRef 或 receipt lineage。
不建立第二份 Tool Registry、项目 registry、approval state 或 Gateway policy 表。
不增加 template network、Marketplace、SDK 下载或项目迁移向导。
不恢复 ProductionCandidateModule、CodexOutcomeEvaluationModule、candidate/freezer/activation/attempt。
不运行 254-R1/R2、Fresh Candidate、F-A/F-B/G、Real Evaluation 或三引擎 B 通道。
不把 production MCP conformance 扩张成真实 AI 游戏制作或 outcome acceptance。
```

如果实现发现必须改变 253 的 Agent-owned planning、254 第 30 节 scope correction、255 的 Catalog responsibility，或必须引入跨工具固定生命周期，立即停止并先修订正式方案。

## 3. 当前代码基线

本文基于 2026-07-23、commit 前缀 `a9f3b6b` 的只读复核。激活时必须重新核对 exact commit/tree、工作树和以下事实。

### 3.1 Editor-owned Create/Open 已存在

```text
rust/crates/editor_ui_model/src/command.rs
  UiCommandPayload::OpenProject { path }
  UiCommandPayload::CreateProject { path, name }

rust/crates/editor_core/src/session.rs
  execute_ui_command_direct() 分派到 self.open_project()/self.create_project()

rust/crates/editor_core/src/services/project_service.rs
  EditorSession::create_project/open_project
  两者复用 ProjectLauncherState，更新 active_project_session、recent projects、asset browser 与 project caches

rust/crates/editor_core/src/project_launcher.rs
  ProjectLauncherState::create_project/open_project
  ProjectSession 持有 ProjectWriteScope

rust/crates/editor_core/src/project_write_scope.rs
  canonical project-relative write、link/reparse rejection、atomic directory publication
```

256 不得在 Gateway Adapter 复制 project skeleton、manifest 写入或 recent-project 更新。

### 3.2 当前 Gateway 不能在 launcher 工作

```text
rust/crates/editor_window_winit/src/application.rs
  reconcile_gateway_host()
  active_project_session=None 时直接 drop gateway_host

rust/crates/ai_tool_gateway/src/editor_host.rs
  EditorGatewayHostBinding 只表达 project identity + canonical root digest
  Host start 时一次性创建 record/server/publication

rust/crates/ai_tool_gateway/src/discovery.rs
  GatewayDiscoveryRecord v1 强制非空 projectIdentity/root digest
  discovery 文件名以 canonical root digest 为键

rust/crates/ai_tool_gateway/src/protocol.rs
  ClientHello v1 强制 expectedProjectIdentity 与 expectedCanonicalProjectRootDigest 非空
  GatewayRequest v1 强制 projectIdentity

rust/crates/ai_tool_gateway/src/remote_adapter.rs
  从 project-bound discovery 构造 project-bound ClientHello
```

这是 256 首个协议缺口。施工必须建立显式 launcher binding；禁止用空字符串伪装项目或跳过校验。

### 3.3 Mutation authority 与 Native Editor UI 已存在

```text
rust/crates/ai_tool_gateway/src/core.rs
  request_goal_mutation_access()
  approval_inbox()
  decide_access()
  session_status()
  access_generation / operation_generation
  awaiting_user / active / revoked / expired

rust/crates/editor_window_winit/src/application.rs
  request_gateway_goal_mutation_access()
  sync_gateway_access_requests()
  ApproveGatewayAccessRequest / RejectGatewayAccessRequest

rust/crates/editor_ui_model/src/command.rs
  Native Editor approval commands
```

当前 request 只能由进程内调用者触发，没有 typed MCP 入口。256 只补这一前门，不重写 authority。

### 3.4 255 Registry/Catalog/MCP 已存在

```text
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
  16 个业务 Tool 的唯一 AiToolContractRegistry

rust/crates/editor_core/src/ai_tool_catalog.rs
  Catalog v2、四态 availability、basis、catalogDigest/availabilityDigest

rust/crates/ai_tool_gateway/src/mcp_stdio.rs
  4 个 control tools + Registry typed projection
  当前没有 project.create/open 或 session.request_mutation_access

rust/crates/ai_tool_gateway/tests/core_scope_contract.rs
  当前 16 Tool surface contract
```

三个新 Tool 必须加入同一 Registry 并走同一 typed schema projection；不得只在 `mcp_stdio.rs` 手写孤立 Tool。

### 3.5 当前测试底座

```text
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/project_write_scope.rs
rust/crates/ai_tool_gateway/src/discovery.rs
rust/crates/ai_tool_gateway/src/editor_host.rs
rust/crates/ai_tool_gateway/src/core.rs
rust/crates/ai_tool_gateway/src/mcp_stdio.rs
rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs
rust/crates/ai_tool_gateway/tests/explicit_reconnect.rs
rust/crates/editor_host/tests/gateway_process.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/project_e2e_gate/src/tests.rs
```

现有 process smoke 在 MCP 外先创建项目；256 必须新增从 launcher production host 开始的覆盖，不能改名冒充已有测试已经证明 no-project 前门。

## 4. 目标 Module、Interface 与内部 seam

### 4.1 公共 Interface

唯一公共目标级 Module 为 `AiProjectAccessModule`。对外只通过 Registry/Gateway typed projection 暴露：

```text
project.create
project.open
session.request_mutation_access
```

不得额外暴露：

```text
validate_project_path
dispatch_editor_command
publish_discovery
rebind_session
create_approval_card
approve_mutation_access
```

这些都是内部 seam 或已有 Native Editor owner。

### 4.2 建议所有权放置

预计新增：

```text
rust/crates/ai_tool_gateway/src/project_access.rs
```

该 Module 负责组合 Gateway owner 和 Editor owner：

```text
typed direct input -> strict contract decode
launcher/project binding preflight
owner-thread Editor Command dispatch
authoritative project binding observation
bootstrap/access receipt composition
host publication/rebind instruction
idempotency and partial-state diagnostic
```

它不能直接写项目文件、直接修改 `EditorSession.active_project_session`、直接批准 request 或持有第二套 discovery/session state。

Registry descriptor、direct input/output schema 可以放在 `editor_core` 现有 contract 附近或一个窄 contract Module；Implementation 仍由 Gateway `AiProjectAccessModule` 拥有。文件边界可在 P1 红测试后按依赖方向微调，但公共 Interface 和 owner 不得改变。

### 4.3 Launcher binding 必须是显式身份

协议不得用 `projectIdentity=""` 表达 launcher。建议使用严格枚举/union：

```text
EditorHostBinding:
  Launcher { editorProcessIdentity, bindingGeneration }
  Project { projectIdentity, canonicalProjectRootDigest, bindingGeneration }
```

Discovery、ClientHello、Session binding 和 request envelope 都必须能区分两态。launcher Session：

```text
只允许 catalog/status/project.create/project.open
没有 project digest/read Grant/mutation access
不能调用任何 project-dependent Tool
```

项目绑定后必须建立 fresh project Session；launcher Session 不升级为项目 Session。

### 4.4 Host 与 publication 生命周期

Native Editor 持有一个 production Gateway Host lifecycle：

```text
launcher host publication
  -> Create/Open Editor Command committed
  -> old launcher endpoint/session reconnect-required
  -> project host publication
  -> fresh project Session
```

如果采用同一 server 内显式 binding rotation，也必须：

- 产生新的 binding identity/generation；
- 让旧 ClientSessionBinding、Grant、operation ownership 和 receipt fail-closed；
- 更新 discovery 的 exact binding；
- 不允许 Adapter 静默改写现有 binding。

实现选择由 P2/P4 红测试约束，不能只靠内部字段变化宣称 rebind。

## 5. 预计涉及文件

### 5.1 必改或预计新增

```text
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
  三个 Tool id/descriptor/direct input schema、Registry 单一真相

rust/crates/editor_core/src/ai_tool_catalog.rs
  三个 Tool 的 owner-local availability；现有项目 Tool no-project blocker 保持

rust/crates/editor_core/src/lib.rs
  仅必要的 contract re-export

rust/crates/ai_tool_gateway/src/project_access.rs
  新增 AiProjectAccessModule 深 Implementation

rust/crates/ai_tool_gateway/src/lib.rs
  Module 登记与窄 re-export

rust/crates/ai_tool_gateway/src/protocol.rs
  launcher/project binding、bootstrap/access request/reply wire schema

rust/crates/ai_tool_gateway/src/discovery.rs
  launcher/project discovery 与 publication generation

rust/crates/ai_tool_gateway/src/editor_host.rs
  launcher host + project host/rebind lifecycle

rust/crates/ai_tool_gateway/src/core.rs
  launcher Session authority、ProjectAccess dispatch、stale cleanup、access request projection

rust/crates/ai_tool_gateway/src/remote_adapter.rs
  launcher/project ClientHello 与 explicit reconnect

rust/crates/ai_tool_gateway/src/named_pipe.rs
  owner-thread ProjectAccess dispatch 与 transport 回归

rust/crates/ai_tool_gateway/src/mcp_stdio.rs
  typed projection/call mapping，不复制 schema

rust/crates/editor_window_winit/src/application.rs
  launcher 状态启动 Gateway Host、project switch reconciliation、approval UI 同步

rust/crates/editor_window_winit/src/tests/native_app.rs
  launcher host、Create/Open、rebind、approval UI

rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs
  production binary typed MCP launcher/project process contract

rust/crates/ai_tool_gateway/tests/explicit_reconnect.rs
  launcher/project/switch stale lineage

rust/crates/ai_tool_gateway/tests/core_scope_contract.rs
  19 Tool surface 与 retired lifecycle absence

rust/crates/editor_host/tests/gateway_process.rs
  real Editor Host launcher discovery/process composition

rust/crates/ai_tool_gateway/src/bin/ai_engine_gateway_codex_config.rs
rust/crates/ai_tool_gateway/tests/codex_config_process.rs
  config receipt failure-atomicity 与 isolated install/replace/rollback process contract

rust/crates/quality_gate/src/gate_256_p8.rs
rust/crates/quality_gate/tests/gate_256_p8_contract.rs
rust/crates/quality_gate/src/lib.rs
rust/crates/quality_gate/Cargo.toml
  P8 唯一 repository-owned construction runner；只编排施工证据，不进入产品 Tool/Catalog/runtime

rust/crates/project_e2e_gate/src/tests.rs
  跨 crate consumer 与 no-project availability
```

### 5.2 仅在红测试证明需要时修改

```text
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/gateway_access.rs 或等价 model 文件
rust/crates/editor_core/src/services/project_service.rs
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/project_write_scope.rs
```

约束：

- `rust/crates/editor_window_winit/src/real_window.rs` 与 `rust/crates/editor_host/src/main.rs` 只读复用既有 `--real-window --isolated-project-launch-root` surface；不得为 Gate 增加 discovery CLI。若定向测试证明 production surface 本身有缺陷，必须先回填本文并使候选失效。
- Create/Open payload 已足够时不新增第二套 Editor Command。
- project launcher/write scope 只有在红测试证明 typed caller 暴露了真实安全缺口时才窄修。
- Codex config schema/public CLI 不因 256 重写；本次 remediation 只修 receipt persist failure-atomicity，并增加隔离 process regression。
- P8 只能操作 Gate 专用 isolated config；真实 Codex config 只允许在 fresh freeze 成功后按 post-freeze activation 合同处理。

### 5.3 默认不得修改

```text
rust/Cargo.lock（若 quality_gate 新增 windows-sys dependency，这是预期窄改；必须审查只出现对应依赖边，任何其它漂移都使本轮 freeze 失效）
除 rust/crates/quality_gate/Cargo.toml 的 Windows construction-runner API 外的其它 Cargo.toml
legacy/**
项目 fixture/source assets
RuntimePackage、Renderer、AUI、ECS、Provider/LLM 业务
254-R1/R2 历史方案、施工与 evidence
255 已完成施工文档和完成记录
```

出现 source、test、manifest/lockfile、fixture、ledger、schema snapshot、Runner/toolchain 或生成清单变更时，既有 qualification/freeze 失效，必须重新 P8Q；只有新的 P8Q 全绿后才能申请 P8F。

## 6. 必须保持的不变量

```text
AiToolContractRegistry 是 19 个业务 Tool 的唯一静态合同真相。
launcher binding 是显式状态，不是空 project identity。
launcher Session 没有 project read/mutation authority。
Create/Open 只调用正式 Editor Command，不直接写 project skeleton。
ProjectWriteScope/ProjectLauncherState 继续拥有路径与项目创建/打开语义。
project identity/root digest 只从 committed Editor project binding 读取。
旧 Session 不原地升级或偷换项目。
旧 Grant、operation、receipt 不能跨 launcher/project 或 project/project binding。
request_mutation_access 只创建 awaiting_user request，不批准、不消费 mutation。
Native Editor approve/reject 是唯一决议 owner。
request receipt 不是 Grant，caller input 不包含 opaque grantRef。
相同 idempotency intent 不重复 Create/Open 或 approval request。
Catalog 只解释 availability，不自动调用 ProjectAccess。
typed execute 在副作用前用最新 binding/access generation 重检。
tools/list 稳定，不返回 nextToolIds 或固定后续顺序。
production conformance 到 access decision 即停止，不执行 mutation/Preview/Build。
production `editor_host` 必须从明确 profile 与 `real-window`/实际所需 surface feature 构建。
`editor_host`、`ai_engine_gateway_mcp`、`ai_engine_gateway_codex_config` 三 binary 必须同 commit/profile seal/reopen。
sealed `editor_host` 必须真实创建顶层窗口、到达 launcher readiness、正常有界关闭并 cleanup。
config command digest 与 MCP executable content digest 分开记录。
isolated config install/replace/receipt/rollback 必须 exact 恢复 before bytes/digest。
receipt 持久化失败时，已改变的 config 必须立即尝试 exact rollback，并报告双重结果。
Codex config 改变后必须 reload/new task；当前 task 不能热加载 typed MCP。
```

## 7. 分 Gate 施工

以下 Gate 只有激活后才可执行。每个 Gate 先跑定向测试；失败在当前 Gate 收敛，不跨 Gate 堆积。

### 256-P0：激活基线、隔离、canonical truth 与 consumer inventory

目标：证明施工起点、历史基线、路径、工具链、磁盘容量和 consumer 都真实成立。

任务：

1. 复核 256 正式方案、240、49、54 与本文无漂移。
2. 确认 `施工文档/当前/` 只有本文且 `待执行/` 不再包含本文。
3. 记录 exact commit/tree、branch、source clean、Rust/Cargo、Git autocrlf/attributes。
4. 创建仓库外 fresh exact-commit worktree；分配独立 `CARGO_TARGET_DIR`、`TEMP/TMP`、fixture、pipe/discovery 与 artifact root。
5. 记录 G: 及所有实际卷 free bytes；估算 P8-P10 并发峰值并保留至少 20% 且 10 GiB cleanup/report margin。
6. 盘点 binding/discovery/ClientHello/Session/access/Catalog/MCP 全部 consumer。
7. 运行现有最小基线和 repository-owned lint ledger producer。

命令：

```powershell
git status --short
git rev-parse HEAD
git rev-parse "HEAD^{tree}"
git branch --show-current
git config --get core.autocrlf
git check-attr -a -- rust/crates/ai_tool_gateway/src/protocol.rs
rustc -Vv
cargo -V
Get-PSDrive -PSProvider FileSystem
rg -n "GatewayDiscoveryRecord|EditorGatewayHostBinding|ClientHello|ClientSessionBinding|request_goal_mutation_access|AiToolContractRegistry|mcp_tools" rust/crates
cd rust
cargo test -p editor_core create_project_writes_minimum_project_skeleton --locked
cargo test -p editor_core open_project_keeps_manifest_bytes_unchanged_and_updates_editor_recent_state --locked
cargo test -p ai_tool_gateway gateway_session_status_connects_with_read_active_and_mutation_not_requested --locked
cargo test -p ai_tool_gateway editor_gateway_host_publishes_resolves_and_removes_its_binding --locked
cargo test -p editor_window_winit gateway_host_lifecycle_rebinds_on_project_switch_and_cleans_up_on_drop --locked
cargo run -p quality_gate --locked -- propose-ledger
$lintDiff = Get-Content -Raw 'target/quality-gate/candidate/lint-debt-ledger-diff.v1.json' | ConvertFrom-Json
if (-not $lintDiff.previous_ledger_readable -or
    @($lintDiff.added_diagnostics).Count -ne 0 -or
    @($lintDiff.removed_diagnostics).Count -ne 0 -or
    @($lintDiff.changed_diagnostics).Count -ne 0 -or
    @($lintDiff.new_suppressions).Count -ne 0 -or
    @($lintDiff.expired_entries).Count -ne 0) {
    throw '256 activation baseline lint truth changed'
}
```

预计成本：低到中；cold cache 以实际记录为准。

进入条件：本文已激活，用户明确授权施工。

退出条件：基线测试、canonical ledger、consumer inventory、路径与容量全部有效。

失败动作：停止并分类为文档、环境或既有基线问题；不得写红测试或实现。

P0 完成证据（2026-07-23）：

```text
result: passed
source worktree: G:\gameEngin-256-p0-e387f43
exact commit: e387f4309735476302b100f2d19e2e6fa33c0464
exact tree: 1a8cf9177800425b08b63913c2e6145a433b2b10
branch: detached
source clean: true
CARGO_TARGET_DIR: G:\EngineTest\Gate256\P0-e387f43\target
TEMP/TMP: G:\EngineTest\Gate256\P0-e387f43\temp
fixture root: G:\EngineTest\Gate256\P0-e387f43\fixture
discovery root: G:\EngineTest\Gate256\P0-e387f43\discovery
artifact root: G:\EngineTest\Gate256\P0-e387f43\artifacts
Rust/Cargo: 1.96.0 / 1.96.0
Git core.autocrlf: true
protocol.rs attribute: text=auto
free before: C=49,845,501,952; G=480,606,384,128; H=140,334,977,024; I=178,570,006,528 bytes
consumer inventory: binding/discovery/ClientHello/Session/access/Catalog/MCP owners matched section 5
baseline tests: 5 passed, 0 failed
lint diff: previous_ledger_readable=true; added=0; removed=0; changed=0; new_suppressions=0; expired=0
```

说明：`propose-ledger` 一次成功生成报告到 repository-owned `rust/target/quality-gate/candidate/`。首次读取包装器误查 `CARGO_TARGET_DIR`，随后只消费同一次已生成报告，没有重跑 producer；这不是产品或基线失败。

### 256-P1：Schema、launcher binding 与 bypass 红测试

目标：先固定当前缺口与未来合同，不先写 production Implementation。

新增红测试至少包括：

```text
project_access_registry_exposes_three_strict_goal_level_tools
project_access_direct_input_rejects_engine_owned_fields
gateway_launcher_discovery_is_explicit_and_project_authority_free
gateway_launcher_client_hello_negotiates_without_fake_project_identity
gateway_launcher_session_rejects_every_project_dependent_tool
project_create_and_open_dispatch_only_editor_owned_commands
project_bootstrap_receipt_binds_committed_project_identity_and_root_digest
gateway_project_bootstrap_requires_explicit_reconnect
gateway_mutation_request_typed_tool_creates_only_awaiting_user
mcp_launcher_surface_is_typed_and_has_no_raw_or_recipe_tool
```

红测试必须证明当前行为：

- no-project 时 Host 被关闭；
- discovery/hello 不能表达 launcher；
- Registry 只有 16 Tool；
- MCP 没有三个目标级 Tool；
- access request 没有 typed MCP 入口；
- create/open 后旧 Session 无 bootstrap receipt/rebind lineage。

命令：

```powershell
cd rust
cargo test -p editor_core project_access_registry --locked
cargo test -p ai_tool_gateway gateway_launcher --locked
cargo test -p ai_tool_gateway project_bootstrap --locked
cargo test -p ai_tool_gateway gateway_mutation_request_typed_tool --locked
cargo test -p ai_tool_gateway mcp_launcher_surface --locked
cargo test -p editor_window_winit gateway_launcher --locked
```

预计成本：低。

进入条件：P0 全绿。

退出条件：测试能编译，且只因正式方案已知缺口失败；每个失败对应一项方案合同。

失败动作：fixture、测试架构或非 256 失败先修测试设计；不得把意外失败当红测试。

P1 完成证据（2026-07-23）：

```text
result: passed_as_expected_red
tests added: 10
project_access_registry: 4 expected failures (Registry 16 != 19; three descriptors absent)
gateway_launcher: 3 expected failures (discovery/ClientHello/session cannot express launcher binding)
project_bootstrap: 1 expected failure (launcher binding cannot establish explicit reconnect lineage)
gateway_mutation_request_typed_tool: 1 expected failure (typed request descriptor absent)
mcp_launcher_surface: 1 expected failure (project.create typed MCP Tool absent)
editor_window_winit gateway_launcher: 1 expected failure (no-project Host is closed)
unexpected compile/fixture failures: 0 after correcting the test-only NativeEditorApplication config argument
```

所有红灯分别对应方案已知缺口；未用不存在的 production type 制造编译失败，未运行 P2 Implementation。

### 256-P2：显式 launcher binding、discovery 与协议兼容

目标：让 production Gateway 在 launcher/no-project 状态安全存在，但不授予 project authority。

任务：

1. 为 Host/discovery/ClientHello/Session binding 建立严格 launcher/project union。
2. launcher identity 绑定 Editor process 与单调 binding/publication generation。
3. launcher discovery 使用独立、确定性且不会与项目 root digest 混淆的 locator key。
4. launcher Session 只允许 control/Catalog/Create/Open；project request envelope 不能伪造。
5. 保留 project-bound v1 compatibility，仅在安全、无第二套真相时投影；不兼容客户端 fail-closed。
6. discovery resolve 继续 current-user、bounded、regular-file/no-symlink、alive process 和 ambiguity fail-closed。
7. Native Editor launcher 启动并持有 Host；退出时清除 publication。

命令：

```powershell
cd rust
cargo test -p ai_tool_gateway gateway_launcher_discovery --locked
cargo test -p ai_tool_gateway gateway_launcher_client_hello --locked
cargo test -p ai_tool_gateway gateway_discovery_is_per_user_bounded_and_removed_on_drop --locked
cargo test -p ai_tool_gateway gateway_discovery_rejects_invalid_filename_but_ignores_stale_process --locked
cargo test -p ai_tool_gateway editor_gateway_host_publishes_resolves_and_removes_its_binding --locked
cargo test -p editor_window_winit gateway_launcher_host --locked
cargo test -p editor_window_winit native_editor_application_starts_in_project_launcher_mode --locked
```

预计成本：中。

进入条件：P1 红测试有效。

退出条件：launcher Host 可真实发布/连接；没有 project identity/digest/read/mutation authority；project-bound 旧路径保持。

失败动作：若实现依赖空字符串、magic project id 或 Adapter 特判绕过校验，删除该实现并回到显式 union。

P2 完成证据（2026-07-23）：

```text
result: passed
binding model: strict nested GatewayEndpointBinding launcher | project union
launcher project identity/root digest/project digest/read grant: None
publication generation: process-owned monotonic u64
launcher discovery filename: launcher-<editor-pid>-<generation>.json
required P2 tests: 7 passed, 0 failed
additional launcher Core tests: 2 passed, 0 failed
ai_tool_gateway full crate: 55 passed; 2 expected future-Gate red tests
expected remaining red: typed mutation request descriptor; 19-Tool MCP surface
unexpected regressions: 0
```

project-bound v1 owner path保持可用；launcher 只允许 status/catalog，任一 project-dependent request 结构化拒绝。项目打开后旧 launcher Session fail-closed，fresh project ClientHello 产生不同 Session binding。实现没有空字符串、magic project id 或 raw/recipe bypass。

### 256-P3：AiProjectAccessModule 与 Create/Open owner 链

目标：实现两个 project bootstrap Tool，隐藏严格内部步骤。

任务：

1. 新增 `AiProjectAccessModule` 深 Implementation。
2. Registry 增加 `project.create/open` descriptor、strict direct input/output schema。
3. create 输入限定安全 parent/location reference、projectName、可选已注册 templateId/idempotencyKey。
4. open 输入限定安全 projectLocation 或 Editor-owned recentProjectId、可选 idempotencyKey。
5. caller internal fields 使用 `deny_unknown_fields`/schema `additionalProperties=false` 拒绝。
6. create/open 通过 owner-thread 调用正式 Editor Command；不得直接调用 filesystem writer。
7. 成功后重新观察 committed `ProjectSession`，从 owner 取得 project identity/root digest。
8. 返回 bootstrap receipt：operation/receipt id、command kind、project identity/root digest、publication generation/session action、diagnostics。
9. transport replay 返回同一 receipt；同 key 不同 intent fail-closed。
10. create 冲突不覆盖；open 不迁移/修复损坏项目；partial state 不冒充 binding success。

最低负向矩阵：

```text
empty/relative/drive-root/ambiguous location
unsafe name、reserved component、link/reparse parent
existing non-empty target
invalid/missing manifest
recentProjectId unknown/stale
caller-owned internal identity/digest/generation/grantRef
same idempotency key with changed content
Editor Command rejected/failed
project switch during command completion
```

命令：

```powershell
cd rust
cargo test -p editor_core project_access_registry --locked
cargo test -p editor_core create_project_writes_minimum_project_skeleton --locked
cargo test -p editor_core open_project_validates_manifest_and_updates_recent_list --locked
cargo test -p editor_core open_project_keeps_manifest_bytes_unchanged_and_updates_editor_recent_state --locked
cargo test -p editor_core project_write_scope_rejects_unsafe_relative_paths --locked
cargo test -p editor_core project_write_scope_publishes_directory_atomically --locked
cargo test -p ai_tool_gateway project_bootstrap --locked
cargo test -p ai_tool_gateway project_access_idempotency --locked
```

预计成本：中。

进入条件：P2 全绿。

退出条件：Create/Open typed call 只走 Editor owner；安全、receipt、idempotency、partial failure 全绿。

失败动作：若 Gateway 开始复制 `ProjectLauncherState` 或 `ProjectWriteScope`，停止并把逻辑移回 owner。

### 256-P4：Publication rotation、fresh Session 与 stale lineage

目标：Create/Open 成功后建立可证明的新项目 binding，不让旧 Session 跨项目存活。

任务：

1. project committed 后推进 binding/publication generation。
2. launcher publication/server 被替换或显式 rotation；新 discovery 绑定 exact project identity/root digest。
3. 旧 launcher Session 返回 reconnect-required/endpoint unavailable。
4. fresh Adapter 从新 discovery 建立 project Session，read generation/digest 与 owner 一致。
5. project->project Open 同样使旧 Session/Grant/request/operation/receipt stale。
6. publication 失败与 reconnect 失败不重复 Create/Open；receipt 明确 `project_committed_but_binding_unavailable` 或等价 partial state。
7. Drop/close/prune 清理旧 request/Grant/endpoint；不能遗留双 active publication ambiguity。

命令：

```powershell
cd rust
cargo test -p ai_tool_gateway gateway_project_bootstrap_requires_explicit_reconnect --locked
cargo test -p ai_tool_gateway --test explicit_reconnect --locked
cargo test -p ai_tool_gateway gateway_binding_rejects_wrong_project_and_invalidates_after_switch --locked
cargo test -p ai_tool_gateway gateway_disconnect_reconnect_does_not_transfer_operation_ownership --locked
cargo test -p ai_tool_gateway gateway_approval_lifecycle_prunes_close_ttl_switch_and_rejects_stale_decision --locked
cargo test -p editor_window_winit gateway_host_lifecycle_rebinds_on_project_switch_and_cleans_up_on_drop --locked
```

预计成本：中。

进入条件：P3 全绿。

退出条件：launcher->project、project->project、failure/cleanup lineage 全绿；不存在原地偷换 binding。

失败动作：任何旧 authority 在新 binding 可用都视为安全失败；不得用测试白名单掩盖。

### 256-P5：Typed mutation access request 与 Native Editor 决议

目标：补齐 typed request 前门，复用既有 authority/UI，不自动批准。

任务：

1. Registry 增加 `session.request_mutation_access` strict schema。
2. caller input 只接受 goalSummary、riskClass、requestedCapabilities、可选 duration 上限/idempotencyKey。
3. risk/capability 由 Gateway owner验证并只能收紧，不能由 caller 降级风险。
4. typed call 复用 `GatewayCore::request_goal_mutation_access()` 或窄 owner Interface。
5. 结果只为 awaiting_user/already_pending/rejected/blocked，并返回 request receipt。
6. request receipt 不含 active grantRef；只有真实 Native Editor approve 后 authority 才产生 active Grant。
7. 同 binding 同 intent replay 不增加 approval card；不同 intent 不合并。
8. pending 冲突、TTL、disconnect、project switch、approve/expiry race、reject/revoke 全部单一 generation 决议。
9. Native Editor 显示来源、project、goal、risk、capabilities、期限；approve/reject 仍通过已有 UI Command。

命令：

```powershell
cd rust
cargo test -p ai_tool_gateway gateway_mutation_request_typed_tool --locked
cargo test -p ai_tool_gateway gateway_session_access_allows_read_before_approval_and_isolates_mutation_sessions --locked
cargo test -p ai_tool_gateway gateway_approval_lifecycle_rotates_mutation_and_cancel_uses_operation_snapshot --locked
cargo test -p ai_tool_gateway gateway_approval_lifecycle_prunes_close_ttl_switch_and_rejects_stale_decision --locked
cargo test -p editor_window_winit gateway_access_native_editor_approval_is_user_driven_and_session_bound --locked
cargo test -p editor_window_winit gateway_mutation_request_typed_tool --locked
```

预计成本：中。

进入条件：P4 全绿。

退出条件：typed request 到 awaiting_user、真实 UI approve/reject、generation/expiry/idempotency 全绿；未批准永远不 active。

失败动作：若 MCP、Test Adapter 或 Module 能直接 approve，立即停止并删除越权路径。

### 256-P6：Catalog、typed MCP 与完整负向矩阵

目标：三个 Tool 在 255 Catalog/MCP 中正确可发现、可解释、不可绕过。

状态矩阵：

| binding/access | create/open | request mutation | 项目内 read | 项目 mutation |
|---|---|---|---|---|
| launcher | ready 或真实 blocker | blocked | blocked | blocked |
| project/read active | blocked 或显式 switch policy | ready | ready | authorization_required |
| awaiting_user | 同上 | already pending/availability reason | ready | authorization_required + await_user_decision |
| mutation active | 同上 | blocked/already active | ready | owner guard 决定 ready |
| stale/reconnect-required | blocked | blocked | blocked | blocked |

任务：

1. Catalog v2 加入三个 descriptor 与 Tool-owned availability。
2. `catalogDigest` 随静态 Registry 增加发生预期版本变化；动态 binding/access 只改变 `availabilityDigest`。
3. `tools/list` 投影 4 control + 19 Registry Tool，继续 `listChanged=false`。
4. MCP call mapping 只通过 Registry schema decode；无 raw bootstrap/approve Tool。
5. direct typed call 在 binding/access drift 后于副作用前拒绝。
6. Catalog/Status 查询不触发 Create/Open/request，不写项目、不增加 approval/operation。
7. structured blocker 保留 owner/basis/generation，且不返回 next Tool payload。

命令：

```powershell
cd rust
cargo test -p editor_core project_access_catalog --locked
cargo test -p editor_core ai_tool_catalog_v2 --locked
cargo test -p ai_tool_gateway mcp_launcher_surface --locked
cargo test -p ai_tool_gateway mcp_typed_tool_projection_uses_registry_direct_input_schemas --locked
cargo test -p ai_tool_gateway mcp_projection_tool_list_advertises_typed_catalog_projection_not_raw_execute --locked
cargo test -p ai_tool_gateway gateway_catalog_no_side_effects --locked
cargo test -p ai_tool_gateway gateway_execute_rechecks_catalog_availability_before_side_effects --locked
cargo test -p ai_tool_gateway --test mcp_process_smoke --locked
cargo test -p ai_tool_gateway --test core_scope_contract --locked
```

预计成本：中。

进入条件：P5 全绿。

退出条件：状态矩阵、19 Tool surface、strict schema、无副作用和 bypass 全绿。

失败动作：若只在 MCP 列表层过滤或 `mcp_stdio.rs` 手写第二 schema，删除重复并回到 Registry/owner guard。

### 256-P7：累计失败集与受影响域回归

目标：一次覆盖所有跨 crate consumer 和施工期间累计失败。

累计失败集必须包含：

```text
P1-P6 每个曾失败测试
launcher discovery/hello/host
Create/Open safety/idempotency/partial state
explicit reconnect/stale lineage
approval/expiry/race
Catalog/MCP strict surface
255 availability/execute guard
254 retired lifecycle absence
```

命令：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_core create_project_writes_minimum_project_skeleton --locked
cargo test -p editor_core open_project_keeps_manifest_bytes_unchanged_and_updates_editor_recent_state --locked
cargo test -p editor_core project_write_scope --locked
cargo test -p editor_core project_access --locked
cargo test -p editor_core ai_tool_catalog --locked
cargo test -p editor_core --lib --locked
cargo test -p ai_tool_gateway --lib --locked
cargo test -p ai_tool_gateway --bin ai_engine_gateway_codex_config --locked
cargo test -p ai_tool_gateway --test codex_config_process --locked
cargo test -p ai_tool_gateway --test core_scope_contract --locked
cargo test -p ai_tool_gateway --test explicit_reconnect --locked
cargo test -p ai_tool_gateway --test mcp_process_smoke --locked
cargo test -p editor_window_winit --lib --locked
cargo test -p editor_window_winit dialog::tests::windows_dialog_configures_exact_initial_directory_before_show --all-features --locked -- --exact
cargo test -p editor_window_winit --all-features --locked
cargo test -p editor_host --bin editor_host real_window --locked
cargo test -p editor_host --test gateway_process --locked
cargo test -p project_e2e_gate --locked
cargo test -p quality_gate gate_256_p8::tests --locked
cargo test -p quality_gate --test gate_256_p8_contract --locked
```

Windows 上新增的两个 `editor_window_winit` 命令必须在 fresh、已存在且可写的 Windows verbatim `TEMP/TMP` 根下运行，例如 `\\?\G:\EngineTest\<fresh-p7-root>\temp`；执行前记录原环境，执行后恢复。定向测试必须先红后绿；完整 all-features 随后覆盖同一环境合同。不得把 `TEMP/TMP` 改回普通路径来获得绿灯。

预计成本：中到高；P8R2 连续施工墙钟预算最多 2 小时，按实际 cold/warm 记录。

进入条件：P6 定向矩阵全绿。

退出条件：受影响 crate 与跨 crate consumer 全绿，ignored tests 逐项记录且无新增。

失败动作：回到拥有该行为的 Gate 修复，再一次性重跑累计失败集；不得直接进入完整 workspace。

P7 完成证据（2026-07-23）：

```text
cargo fmt --check：通过
editor_core 定向矩阵：全部通过
editor_core --lib：553 passed / 0 failed / 2 ignored
ai_tool_gateway --lib：61 passed / 0 failed / 0 ignored
core_scope_contract：3 passed
explicit_reconnect：1 passed
mcp_process_smoke：3 passed
editor_window_winit --lib：122 passed
editor_host gateway_process：2 passed
project_e2e_gate：65 passed / 0 failed / 1 ignored
```

ignored tests 均为既有显式环境 Gate：`validates_against_workspace_engine_sdk`、`llm_real_patch_source_openai_compatible_env_gated_smoke` 与 `c01_golden_gate_full_preview_export_local`。累计失败集发现的三类问题均已在 owner 测试层收敛：16/20 Tool 旧数量与 launcher availability 旧断言、隐含 Catalog scope 旧断言、`ClientHello`/project binding union API 迁移遗漏。没有弱化生产合同，没有运行 P8/P9/P10，也没有执行历史 Gate。

P8 第一次执行与 fixture ownership remediation 证据（2026-07-23）：

```text
失败 candidate commit：751249271ad91b099a9ff7b588e3b77376f9e137
失败 candidate tree：6f68aec758f43af9ef93086685dcb18151a69ec0
失败点：editor_core --all-features，552 passed / 1 failed / 2 ignored
首个失败：build_service_release_package_command_caches_atomic_report
失败报告：G:\EngineTest\Gate256P8-7512492\artifact\p8-preflight-failure-report.v1.json
失败报告 sha256：0e61fb744d2820ababd721131201ce178b8270a1b43d02461aa02267268775f6
remediation commit：4f09d1fb56038b21dc81e49243c34173433a80c6
remediation tree：74390fb5d5b5617a4dd73274473815c5a2e236f2
P7 fmt：通过
P7 editor_core 定向矩阵：全部通过
P7 editor_core --lib：554 passed / 0 failed / 2 ignored
P7 ai_tool_gateway --lib：61 passed
P7 core_scope_contract：3 passed
P7 explicit_reconnect：1 passed
P7 mcp_process_smoke：3 passed
P7 editor_window_winit --lib：122 passed
P7 editor_host gateway_process：2 passed
P7 project_e2e_gate：65 passed / 0 failed / 1 ignored
P7 source/worktree：clean
second fresh P8 authorization：consumed; failed before producer completion
```

根因不是 256 public Tool 设计，而是两处测试 ownership 违规：`desktop_export_complex_shooter_package_contains_assembled_domains` 直接写仓库 sample 的 `Build`，`human_workflow_panel_matrix_is_reachable_through_retained_tabs` 直接打开仓库 sample 并创建 `Library/ProjectIntent/journal.json`。修复后两个 writer 均使用排除生成目录的唯一临时副本，并在 owner 测试中断言 source bytes 不变。失败 P8 candidate 已失效且不得复用。

fresh P8 第二次执行证据（2026-07-23）：

```text
candidate commit：825f5a6a37f3d4fe27a3616eb8769034d12bfa70
candidate tree：cc98ff51484328bd7ab52c8860d2e59dbd0ab69a
source root：G:\gameEngin-256-p8-825f5a6
gate root：G:\EngineTest\Gate256P8-825f5a6
失败阶段：canonical lint ledger producer wrapper
producer completed：false
lint diff artifact present：false
首因：PowerShell `ErrorActionPreference=Stop` 把 Cargo 正常 stderr 编译进度转换为 terminating NativeCommandError
source clean：true
candidate frozen：false
P9/P10 started：false
失败报告：G:\EngineTest\Gate256P8-825f5a6\artifact\p8-preflight-failure-report.v1.json
失败报告 sha256：3edfd7c9b8dffdda22287ceb0358f21c3e9b9869682d9aba6f3024b53791ec78
new P8 authorization immediately after this failure：pending
```

这次失败属于外部 preflight orchestration wrapper 缺陷，不是 256 设计、产品实现、fixture ownership、canonical ledger 或容量失败。由于 producer 未完成且无 lint diff，不能消费或推断 ledger 结果；本次授权已消费，禁止在同一 run 内重试或继续矩阵。

PowerShell native process wrapper remediation 证据（2026-07-23）：

```text
red-capable smoke：native 子进程写 stderr 且 exit 0；旧 `ErrorActionPreference=Stop + 2>&1` 包装稳定抛 NativeCommandError，实际 exit code 丢失为 -1
green smoke：Start-Process 分离 stdout/stderr 并读取 ExitCode；同一子进程 exit=0、stdout=stdout-ok、stderr=stderr-ok
cargo smoke：同一包装执行 cargo -V，exit=0，version=1.96.0
smoke root：G:\EngineTest\Gate256P8-WrapperSmoke-9003087
product Rust/test/manifest/lockfile/ledger/fixture changes：0
construction rule：禁止 `$ErrorActionPreference=Stop` 与 native `2>&1`/Tee-Object 组合；直接调用后检查 `$LASTEXITCODE`，或 Start-Process 分流并检查 ExitCode
new fresh P8 authorization：authorized once; not yet executed
```

该修复不改变 256 产品设计或实现，只修施工调用合同。由于没有 Rust、测试、manifest、lockfile、ledger 或 fixture 变化，既有 remediation P7 仍有效；新的 P8 必须使用包含本规则与当前授权真相的 fresh exact commit。

fresh P8 第三次执行与 direct-wrapper remediation 证据（2026-07-23）：

```text
candidate commit：e358c836b4dc801f87f88b6505d95afbe7ca24a5
candidate tree：c55886b1b2b428a337b5c0ed050b9270d3170f8d
source root：G:\gameEngin-256-p8-e358c83
gate root：G:\EngineTest\Gate256P8-e358c83
canonical fmt：exit 0
canonical ledger producer：exit 0；run count=1
canonical ledger diff：previous readable=true；added=0；removed=0；changed=0；new suppressions=0；expired=0
candidate ledger：168 entries；371 warning occurrences；11 suppressions
失败阶段：P7 editor_core create_project 定向测试 wrapper evidence
测试输出：1 passed / 0 failed / 0 ignored
native exit code：unavailable
首因：Start-Process -Wait 在 Cargo/test process tree 已退出后仍挂起，无法返回可信 exit code
remaining P7 / all-features / binary build：not started
source clean：true
candidate frozen：false
P9/P10 started：false
失败报告：G:\EngineTest\Gate256P8-e358c83\artifact\p8-preflight-failure-report.v1.json
失败报告 sha256：5c727e250ab412f5f4f318cf05edbb5d26313aef0e97876817c5e1e835ef9ad9
direct-wrapper representative smoke：cargo test -p quality_gate proposal_writes_only_candidate_directory --locked；exit=0；1 passed / 0 failed
direct-wrapper smoke root：G:\EngineTest\Gate256P8-DirectWrapperSmoke-9003087
new P8 authorization：authorized once; not yet executed
```

这次失败仍属于施工 wrapper，而不是 256 设计、产品实现、fixture ownership、canonical ledger 或容量。规则进一步收敛为：one-shot Gate 不使用 `Start-Process -Wait`；必须同步直接调用 native command、分离 stdout/stderr、立即保存 `$LASTEXITCODE`，并在 Gate 外先通过代表性 Cargo test。

repository-owned runner / config transaction remediation 与 fresh P7 证据（2026-07-24）：

```text
remediation source parent commit：f7275eb98a4bbeb3925ac24c6245944538bf6b73
remediation source parent tree：76864d0dd6b9e8b2ff465954c1fb5805b9f2a7d8
new candidate exact commit/tree：由本轮 scoped implementation commit 产生；提交后只读解析，并由 fresh P8 v2 artifact 绑定
fresh P8 authorization consumed：true
conditional P8A branch：not entered；expired with failed P8
real Codex config before/after sha256：6aa3f1ad691c5ef2307bacf064e94f882179fb6f9fbbdc7223d08f57e65f2d37 / same

canonical ledger producer：exit 0；run count=1
candidate ledger：168 entries / 371 warning occurrences / 11 suppressions
previous_ledger_readable：true
added / removed / changed / new suppressions / expired：0 / 0 / 0 / 0 / 0
ledger sha256：eae15c0bf78da17c2dab9d3a225091580ba893a584323b8597a49cf8eb6c5dcc
diff sha256：0daf35bbc8556133023a548432b25aeb9e7c4f7b9669923874ab3553c84f0934
external evidence root：G:\EngineTest\Gate256P8-P7Remediation-f7275eb
source-local candidate cleanup：removed

P7 cargo fmt --check：通过
P7 editor_core 定向：create 1；open 1；project_write_scope 6；project_access 5；ai_tool_catalog 8；全部通过
P7 editor_core --lib：554 passed / 0 failed / 2 ignored
P7 ai_tool_gateway --lib：74 passed / 0 failed / 0 ignored
P7 config helper bin：4 passed；codex_config_process：5 passed
P7 core_scope_contract：3 passed
P7 explicit_reconnect：1 passed
P7 mcp_process_smoke：3 passed
P7 editor_window_winit --lib：122 passed
P7 editor_host real_window：2 passed
P7 editor_host gateway_process：2 passed
P7 project_e2e_gate：59 library + 1 atomic publish + 1 PE + 1 release package + 3 consistency passed / 1 ignored
P7 gate_256_p8 owner：40 passed
P7 gate_256_p8_contract：7 passed
quality_gate strict all-targets Clippy：通过
source test pollution：无；tracked/untracked 仍仅为本轮 11 个交付文件
```

本轮修复仍是 construction-only 证据设施，不进入 Catalog/MCP/public runtime。实现建立：

- repository-owned fixed-matrix runner、runner self/build-source lineage、actual child image、bounded timeout/descendant cleanup；
- fresh Gate root 与 started marker 的 atomic no-clobber 消费；
- P8 report/failure/freeze 与 P8A success/failure 的 sidecar-first、JSON-last terminal commit；
- 三 production binary seal、sealed real-window smoke、isolated config 四态 lifecycle 与 receipt lineage；
- config staged/captured/failed-staged 私有同卷恢复目录、Windows `ReplaceFileW` CAS、artifact no-clobber、receipt persist 自动 exact rollback；
- malformed TOML diagnostic 脱敏、config tree ancestor/descendant recovery root 拒绝、P8A marker 前 failure/volume 二次复核。

两轮独立只读终审发现并在 P7 前收敛了 private-root containment、malformed TOML secret 回显、P8A prepare-to-marker failure race、report-to-freeze failure terminal cleanup 和 P8A publish-failure schema 五类缺口。最终终审无剩余 blocker；真实 HWND/GPU/WM_CLOSE、真实 release build/seal、真实卷容量和实际 isolated config mutation只由 fresh P8 证明，定向测试不冒充这些 production evidence。

### 256-P8（历史一体化合同，已停止）：环境等价预检与代码冻结

本节到“fresh P8 v2 第六次执行证据”为止记录旧的一体化 P8 合同和六次历史执行证据。旧合同把广泛回归、production build/seal、真实窗口、isolated config 与 freeze 放进同一个 one-shot attempt，已由 P8S1 停止。以下旧命令、started marker 和 `p8-preflight-*.v2.json` / `p8-candidate-freeze.v2.json` 只能解释历史 artifact，不再授权或定义下一次 P8。

目标：在最终 conformance/Local CI 前证明 exact candidate、容量、路径、工具链和高风险矩阵可重现。

任务：

1. 提交 P1-P7 代码、测试与当前 remediation 合同，记录 exact commit/tree；source clean。runner 必须嵌入其构建 source root、commit、tree 与构建时 cleanliness，并与正式 detached source 精确相等；构建时和运行时 cleanliness 都同时拒绝 tracked、untracked 与 ignored outputs。
2. 在仓库外创建 fresh detached exact-commit worktree。
3. 为 source/target/TEMP/TMP/discovery/pipe/fixture/artifact 分配独立绝对路径。
4. 记录 source、Gate target/temp/artifact/fixture、runner copy、Cargo/Rustup cache 与真实 config 所在卷的 volume root、serial 和 free bytes；满足估算峰值 + 20% 且 10 GiB 余量，并在授权消费前及终态发布前复核卷身份未漂移。
5. 运行 canonical ledger producer并结构化对账；producer 固定写入 `<fresh-source>\rust\target\quality-gate\candidate`，复制 ledger/diff 到 artifact 并核验 digest 后，精确删除该 ignored candidate 目录并记录 `sourceLocalCandidateCleanup=removed`。
6. 运行 fmt、全部 P7 高风险矩阵、default/all-features 定向等价矩阵。
7. 用明确 production profile 与 feature 分开构建 `editor_host` 和 Gateway bins：`editor_host` 必须包含 `real-window` 与实际 production surface feature。
8. seal/reopen `editor_host.exe`、`ai_engine_gateway_mcp.exe`、`ai_engine_gateway_codex_config.exe`，记录 exact commit/tree、Cargo profile、feature set、source/sealed path、size 和 SHA-256。
9. 从 sealed `editor_host.exe` 启动 Gate 专用真实 Native Editor：观察该 PID 的非零顶层窗口 handle 与 launcher discovery/readiness，正常发送 close，要求 bounded exit 0；超时 kill 只能记失败。
10. 从 sealed config helper 对 isolated config 运行 install -> receipt verify -> rollback，以及 wrong-replace fail-closed -> exact replace -> receipt verify -> rollback；要求 exact before bytes/digest 恢复。
11. receipt 必须验证 schema/configPath/beforeDigest/afterDigest/commandDigest/changed/reloadOrNewTaskRequired/backupPath/fragmentPath；另记 MCP binary content SHA-256，不得与 commandDigest 混称。
12. P8 前后真实 Codex config 只比较 digest，必须保持不变；禁止把真实 config 正文复制进 Gate artifact。
13. 保存 preflight report/digest；清理完成角色的可再生 target 后再次记录 free bytes。只有所有证据全绿才冻结并声明 `p9Eligible=true`；之后任何源码、测试、manifest、lockfile、ledger、fixture 或 sealed composition 变化都使 freeze 失效。
14. P8 只能由 `quality_gate` 内的 `gate-256-p8` repository-owned construction runner 编排；禁止在 Gate 现场手写、复制或修改 PowerShell wrapper。该 runner 是施工证据设施，不是 AI-facing `Runner`、产品 Workflow 或公共 Tool。

runner bootstrap、Gate 外 smoke 与唯一正式执行入口：

```powershell
cargo build -p quality_gate --release --bin quality_gate --locked `
  --manifest-path "<fresh-source>\rust\Cargo.toml" `
  --target-dir "<fresh-runner-build-root>"
Copy-Item -LiteralPath "<fresh-runner-build-root>\release\quality_gate.exe" `
  -Destination "<fresh-runner-root>\quality_gate.exe"

& "<fresh-runner-root>\quality_gate.exe" gate-256-p8-wrapper-smoke `
  --source-root "<fresh-source>" `
  --smoke-root "<fresh-wrapper-smoke-root>" `
  --expected-commit "<40-hex-exact-commit>" `
  --expected-tree "<40-hex-exact-tree>"

& "<fresh-runner-root>\quality_gate.exe" gate-256-p8 `
  --source-root "<fresh-source>" `
  --gate-root "<fresh-gate-root>" `
  --expected-commit "<40-hex-exact-commit>" `
  --expected-tree "<40-hex-exact-tree>" `
  --runner-smoke-report "<fresh-wrapper-smoke-root>\artifact\p8-runner-smoke-report.v2.json" `
  --real-codex-config "<absolute-real-codex-config.toml>"
```

`<fresh-source>` 必须是仓库外 detached exact-commit clean worktree。runner bootstrap、smoke 与 Full 必须使用同一 copied `quality_gate.exe`，Full 先核验自身 SHA-256 与 smoke report 完全一致。`<fresh-gate-root>` 必须在调用前不存在，且不得是任何历史 `Gate256P8-*` root。runner 只有在全部参数、source、smoke report、自身 digest、真实 config path 与容量只读前检通过后，才原子创建 `artifact/p8-run-started.v2.json`；该 marker 创建是本次 one-shot 授权的唯一消费点。之后任何失败都禁止对同一授权、candidate 或 Gate root 再调用 `gate-256-p8`。

runner 内部 canonical matrix 必须固定且进入 report，不接受 caller 增删 stage：

```text
git status/tree/toolchain/volume/capacity
cargo fmt --check
cargo run -p quality_gate --locked -- propose-ledger + 五类结构化 diff 对账
施工文档 P7 的全部命令
cargo test -p ai_tool_gateway --all-features --locked
cargo test -p editor_core --all-features --locked
cargo test -p editor_window_winit --all-features --locked
cargo test -p project_e2e_gate --all-features --locked
cargo build -p editor_host --release --bin editor_host --features real-window,real-wgpu-surface --locked
cargo build -p ai_tool_gateway --release --bins --locked
三 binary seal/reopen
sealed real-window launcher smoke
isolated config lifecycle smoke
source/real-config digest/cleanup/final capacity
```

新 runner 在消费 P8 前必须通过 Gate 外 smoke。smoke 只验证此前反复失败的 native command/exit/timeout/cleanup 编排合同，不预执行 P8 的 production capability stages：

```text
cargo test -p quality_gate --lib proposal::tests::proposal_writes_only_candidate_directory --locked -- --exact
```

`gate-256-p8-wrapper-smoke` 使用 `<fresh-wrapper-smoke-root>\target` 与独立 `TEMP/TMP`，生成 `p8-runner-smoke-report.v2.json`，并在报告完成后精确清理 smoke target、证明 source clean 与 descendant process count 为 0。它不得 build/seal production binary、启动 Editor、运行 config lifecycle、读取或改写真实 Codex config、运行完整 P7/all-features matrix、创建 P8 started marker 或产生 freeze evidence，也不能冒充 P7/P8。任一 smoke 失败必须先修 runner/实现并形成新 commit，不能消费 P8。

smoke 通过不只看 Cargo exit 0；stdout 必须证明 exact test 名运行一次、`running 1 test` 与 `1 passed / 0 failed`，零测试成功或宽过滤命中必须失败。

预计成本：高，但低于完整 Local CI；只执行一次完整预检。

native command 调用合同：P8 runner 必须用 argv-based `std::process::Command`/repository-owned executor 同步启动 Cargo/Rust/Git，清晰记录 cwd/environment allowlist，分别捕获 stdout/stderr，并直接读取真实 exit status；禁止在内部生成 PowerShell command string、`2>&1`/`Tee-Object` 或 `Start-Process -Wait`。每个 stage 必须有高于其内部子阶段上限的 bounded timeout；超时只能在清理 exact child process tree 并保存日志后判失败。Gate 外 smoke 必须使用同一 executable 与 helper；普通子进程或 `cargo -V` smoke 不足。

sealed real-window smoke 调用合同：Gate 专用 launch root 必须是 fresh absolute directory，包含空 `picker-start/` 且不存在 `state/`。runner 只在 sealed Editor child environment 中设置 `LOCALAPPDATA=<gate-root>\localappdata` 与 Gate 专用 `TEMP/TMP`，并删除继承的 `AI_ENGINE_GATEWAY_DISCOVERY` / `AI_ENGINE_GATEWAY_PROJECT_ROOT_DIGEST`；discovery root 因而必须精确为 `<gate-root>\localappdata\AiFirstGameEngine\Gateway\discovery`。禁止修改 runner/用户全局环境，也禁止把 `--gateway-discovery-root` 传给 real-window（该参数只属于与 real-window 冲突的 headless preflight）。调用参数只能是 `--real-window --isolated-project-launch-root <launch-root>`。

启动后最多等待 30 秒，child 提前退出立即失败。runner 使用 Win32 `EnumWindows` / `GetWindow` / `GetAncestor` / `GetWindowThreadProcessId`，必须找到该 sealed PID 唯一 visible、ownerless、`GA_ROOT` 等于自身的非零顶层 HWND，窗口标题为 `AI First Engine Editor`，且 HWND owner PID 精确等于 sealed child PID。同时 isolated discovery root 必须恰有一个 stable regular non-link JSON record，大小不超过 64 KiB，并满足 `schemaVersion=ai-tool-gateway-discovery.v1`、`gatewayProtocolVersion=ai-tool-gateway.v1`、`editorProcessId=PID`、`binding.bindingKind=launcher`、project identity/root digest 均为 null、`publicationGeneration>0`、filename=`launcher-<PID>-<generation>.json`、pipe locator 使用正式 prefix。关闭前按原始 bytes 记录 record SHA-256 并复制到 artifact；复制件 digest 必须相同。

readiness 成立后只能对该 HWND 调用一次 Win32 `PostMessageW(HWND, WM_CLOSE, 0, 0)`，要求返回非零、10 秒内 child 自然 exit 0；不得发送第二次 close 或使用其它 fallback 掩盖失败。exit 后 stdout 必须包含 `window_created=true` 与 `present_status=presented`，不得包含 `real_window_feature_not_enabled`、`environment_blocked`、`gpu_init_failed`、`ui_renderer_missing` 或其它 diagnostic；PID 必须消失，exact discovery record 不存在，隔离 discovery root 无残留 publication。只有超时/异常时才允许终止 exact process tree 做 cleanup，且该 run 必须失败。

isolated config smoke 调用合同：不得设置或依赖默认真实 `CODEX_HOME`；每次调用 sealed helper 都必须显式传入 Gate 专用 config path 与 artifact root。固定矩阵为：

```text
missing config -> install sealed MCP -> durable receipt verify -> rollback -> config 恢复为不存在
existing deterministic config（含 unrelated sections 与 old MCP command）-> install sealed MCP conflict 非零 + stdout empty + bytes/receipt count unchanged
wrong expected-current replace -> precondition 非零 + bytes unchanged
exact old->sealed replace -> changed=true receipt verify -> rollback -> byte-exact old config
```

每份成功 receipt 必须在 success stdout 前 durable 落盘，并逐字段验证 schema/configPath/beforeDigest/afterDigest/commandDigest/changed/reloadOrNewTaskRequired/backupPath/fragmentPath；backup/fragment/receipt 都必须留在 isolated artifact root。receipt persist 失败后若 config 已改变，helper 必须自动 rollback 并返回非零；rollback 也失败时必须同时保留 persist/rollback 两个 diagnostic 与人工恢复材料。P8 process smoke 不以测试注入制造 persist failure，该负向由 P7 binary 单元测试覆盖。runner 另记 sealed MCP content SHA-256，禁止与 commandDigest 混称。

成功 report 至少固定以下 owner evidence：

```text
schemaVersion/status/p9Eligible/authorizationScope/prohibitedChannels
runnerPath/runnerSha256/runnerBuildSourceRoot/runnerBuildCommit/runnerBuildTree/runnerBuildSourceClean/runnerSmokePath/runnerSmokeSha256/invocationDigest/runStartedMarker
exact commit/tree/detached/sourceInitialClean/sourceFinalClean/profile/features
每个 stage argv/cwd、实际 inherited/override/removed environment、解析后的 executable path/size/SHA-256、start/end/timeout/exitCode/stdoutPath+digest/stderrPath+digest
canonical ledger producer run count 与五类 diff
三 binary source/sealed/reopen path/size/SHA-256
sealed Editor PID/visible top-level HWND/title/HWND owner PID/PostMessageW(WM_CLOSE) count/exitCode
launcher discovery path/schema/editorProcessId/binding/publicationGeneration/recordSha256/artifact copy/cleanup
isolated config before/after/restored digest、receipt fields、commandDigest、MCP content SHA-256
真实 Codex config before/after digest 相等且未保存正文
各 storage role 的 requested path/volume root/serial/free bytes before/peak/after、target cleanup、source-local ignored candidate cleanup、source clean
success/failure 发布前再次验证 started marker、source/runner/smoke/真实 config/卷身份、全部 stage executable 与三 sealed binary digest
```

新合同使用 `artifact/p8-preflight-report.v2.json`、`artifact/p8-preflight-failure-report.v2.json` 与 `artifact/p8-candidate-freeze.v2.json`，避免把旧 v1 的两 binary/default-feature 语义误认为合格。每个 JSON 都必须有 SHA-256 sidecar。freeze v2 只能在 report 全绿、`p9Eligible=true` 后生成，并绑定 report digest、runner self digest、smoke report digest、三 binary digest 与 exact commit/tree；它还必须声明 `configActivationRequired=true`、P9/P10 未授权。任一失败只生成 failure report v2，不得生成 freeze marker；P8A activation receipt 是独立后续 artifact，禁止回写 immutable freeze。

进入条件：P7 全绿，代码已提交，隔离和容量成立。

退出条件：fresh exact commit 环境全部通过，三 binary、真实窗口、isolated config、digest/report 完整，exact commit frozen 且 `p9Eligible=true`。

失败动作：fail-fast 保存结构化 evidence 并立即停止；本次 one-shot 授权不得重跑。后续修复必须形成新 commit、重新获得 fresh P8 授权并从 P7/P8 执行。P8 不能冒充 P9/P10。

P8 第四次执行历史证据（2026-07-23；旧合同 passed / P9-ineligible）：

```text
result：passed / frozen
candidate commit：dbc63664b290786969cfca9ed26e0ad73ccba4e0
candidate tree：2d12bf3d769e7d0377913f58c5c6686464489f75
source root：G:\gameEngin-256-p8-dbc6366
gate root：G:\EngineTest\Gate256P8-dbc6366
direct wrapper smoke：1 passed / 0 failed；target cleanup removed
direct wrapper smoke report sha256：dc78bfdd1eb062c607cdf492c2ef479a6116ba6dd4eb3af892a26fba86be5ea7
runner script sha256：86ec74631de7b56215401a34f406ed8873481b8aa34a21dd3b9b2a289eb3773d
canonical ledger producer run count：1
canonical ledger：168 entries / 371 warning occurrences / 11 suppressions
canonical diff：added=0 / removed=0 / changed=0 / new suppressions=0 / expired=0
deduplicated Gate stages：20 passed / 0 failed
test summary：1651 passed / 0 failed / 15 ignored occurrences
editor_host.exe：42569216 bytes；sha256 7a2b78f8e8f646047aa97e8b255fb331ba3cedcdf99aa962402de127d9b1f3ab
ai_engine_gateway_mcp.exe：10290688 bytes；sha256 db44463eb744347c5c2cec67b47f81de2cd3d8a9e4a8416c98b8a38b8030a086
binary source/sealed/reopen digest：all equal
source clean：true
target cleanup：removed
free bytes G before/peak/after：432028327936 / 407331454976 / 429021814784
preflight report：G:\EngineTest\Gate256P8-dbc6366\artifact\p8-preflight-report.v1.json
preflight report sha256：f4a0148b3e3ebab7733f60e8a5418cbfcb56b485cf54f10d1a7822090a1d81cc
freeze marker：G:\EngineTest\Gate256P8-dbc6366\artifact\p8-candidate-freeze.v1.json
freeze marker sha256：e5ec328f6fbbdc56ff39c531b039f7bd557ae57d7ec3494a69615a02ba6c6676
P9/P10 started：false
```

本次通过只证明 P8 环境等价预检与代码冻结，不冒充 P9 production MCP conformance 或 P10 authoritative Local CI。P9/P10 仍未授权。

资格复核（2026-07-24）：

```text
historical report status：passed under old contract
p9Eligible：false
缺口 1：editor_host 使用 default=[] 构建，未冻结 real-window production feature
缺口 2：只 seal editor_host.exe 与 ai_engine_gateway_mcp.exe，漏 ai_engine_gateway_codex_config.exe
缺口 3：没有从 sealed editor_host 验证真实顶层窗口/launcher readiness/正常有界关闭
缺口 4：没有 isolated config install/replace/receipt/reload/rollback restoration evidence
缺口 5：没有真实 config activation receipt 与 reload/new-task lineage
旧 report/freeze path 与 SHA-256：完整保留，仅作历史 evidence
reuse for P9：forbidden
```

该复核不推翻 256 的三个 public Tool 或 P0-P7 产品实现；分类为 P8 construction/freeze contract defect。对应 remediation 已形成 `7a29c87…`，随后第五次 fresh P8 v2 进入下述失败终态。

fresh P8 v2 第五次执行证据（2026-07-24）：

```text
candidate commit：7a29c870cbac01c0c18520f9d87cd36caee90f9e
candidate tree：4dcd0a3690fe2c0cc8d9d049b8e638613829dad2
gate root：G:\EngineTest\Gate256P8-7a29c87
started marker sha256：b40642657931ca917b1304a75165d553c9da3e487e868bacf9d2e40aba6575ac
failure report：G:\EngineTest\Gate256P8-7a29c87\artifact\p8-preflight-failure-report.v2.json
failure report sha256：df0028617113093c65d49af77a4895313f15b6bb166c676fd577bf20e5d227a0
stage summary：33 passed / 1 failed；失败发生在第 34 个 all-features-editor-window
failed command：cargo test -p editor_window_winit --all-features --locked
failed test：dialog::tests::windows_dialog_configures_exact_initial_directory_before_show
test summary：131 passed / 1 failed / 9 ignored
error：project.dialog.windows_create_initial_shell_item_failed / E_INVALIDARG 0x80070057
first cause：P8 将 TEMP/TMP 绑定到 \\?\ verbatim root；Editor Windows dialog 将该 filesystem path 原样传给 SHCreateItemFromParsingName，Windows Shell 拒绝该 parsing name
p9Eligible：false
freeze/P8A/P9/P10：均未创建或启动
real Codex config sha256 before/after：6aa3f1ad691c5ef2307bacf064e94f882179fb6f9fbbdc7223d08f57e65f2d37 / same
source/target/temp/candidate cleanup：clean / removed / removed / removed
runner wall time：1382.5 s
authorization：consumed；不得重跑
```

分类：首要是 `editor_window_winit` Windows Shell Adapter 的局部实现缺陷；次要是 P7 只覆盖 default-feature `--lib`、未在 verbatim `TEMP/TMP` 下执行 real-window/all-features 测试的覆盖缺口。不是 256 public Tool/Catalog/MCP 架构缺陷，也不是 P8 runner 缺陷。

### 256-P8R0/P8R1/P8R2：失败闭环与分片 remediation

P8R0（已完成，预算 45 分钟）：

1. 冻结并引用第五次 P8 原始 failure report、started marker 与 digest。
2. 同步正式方案、README、00 文档地图、00 阅读顺序、240、49、54 与施工 README。
3. 把五次 P8 授权、P8A 条件未成立和所有后续 Gate 未授权写成唯一真相。
4. 把 Windows verbatim-path 定向测试前移到 P7。
5. 将连续施工三小时绝对上限写入 `game-engine-construction` skill。

P8R1（已完成，预算最多 90 分钟）：

1. 只在 `editor_window_winit` 的 Windows Shell Adapter 内增加 filesystem verbatim prefix 到 Shell-compatible path 的结构化转换。
2. 只接受可证明同一身份的 verbatim disk/UNC 路径；其它 device namespace fail-closed。禁止随意字符串截断。
3. 先在 fresh verbatim `TEMP/TMP` 下运行 exact failing test，确认红灯；实现后确认绿灯并运行 `editor_window_winit --all-features`。
4. 不修改 P8 runner、P8 TEMP/TMP、公共 Tool、Catalog、MCP、receipt schema 或 production composition。
5. 形成 scoped implementation commit 后停止。

P8R1 完成证据（2026-07-24）：

```text
baseline commit/tree：b062ee579edbee221e8c52d07e9990ca44091a38 / 430aa62c75e871d7c909583cedbb5b391bf3a9f5
implementation commit/tree：f0bfc74def80f1054fc04a082afe308dbecf3a3b / b53f30fa0e43873848780a75cd8c8a3f85998d1d
scope：rust/crates/editor_window_winit/src/dialog.rs only
implementation：private Windows-only windows_shell_parsing_path Adapter；structured Prefix::VerbatimDisk/VerbatimUNC conversion；Verbatim/DeviceNS fail-closed
red exact：exit 101；0 passed / 1 failed；windows_create_initial_shell_item_failed / 0x80070057
green exact：exit 0；1 passed / 0 failed
all-features：exit 0；133 passed / 0 failed / 9 ignored
format：cargo fmt -p editor_window_winit -- --check passed
external evidence root：G:\EngineTest\Gate256P8R1-b062ee5
red stdout/stderr sha256：1777b00d81d3855da04004c302ce230b2b3cc3cfe741608a2bc0387b292a2503 / e87f18fea2b95590165df7a759ccdcbb4cbdbd25f195a01df3e29a55ba93c405
green exact stdout/stderr sha256：c9562a182d9c3cc6f334ebab37926c18cd0e0fd99503683017102289cec54fe4 / a38f832c41bb7a03218d0198117e1f5fa186b22e44f39499a9639ac55306c20c
all-features stdout/stderr sha256：6862cd74c90b3b8c47437a3ee268841632087979ec65501608ac546bdf500bf0 / 5d01fed6105c3c67173495bcc806c7464c20612d8d4d4edc461fd545801737dd
source status after implementation commit：clean
external TEMP cleanup：not performed；explicit Remove-Item was rejected by command safety policy before execution；534 test-generated entries retained under the dedicated P8R1 root
external-state rule：retained P8R1 temp/evidence root不得复用为 P8R2 或 fresh P8；后续必须使用 fresh root
P8R2：completed
fresh P8/P8A/P9/P10：not started
```

P8R1 没有新增公共 Module、Tool、Gate、runner stage 或 evidence schema，也没有改变 P8 runner 的 verbatim `TEMP/TMP`。原 Shell identity 测试仍通过 canonical before/after equality，证明只转换 Shell 表示而未更换目录身份。

P8R2（已完成，预算最多 2 小时）：

1. 对 P8R1 exact commit 运行本节 P7 全部累计失败集。
2. Windows dialog exact test 与 `editor_window_winit --all-features` 必须使用 fresh verbatim `TEMP/TMP`。
3. canonical ledger、source cleanliness 与全部 affected-domain evidence 全绿后停止。
4. 不执行 fresh P8；新的 fresh P8 必须单独授权，预算最多 1 小时。

P8R2 完成证据：

```text
source commit/tree：f0bfc74def80f1054fc04a082afe308dbecf3a3b / b53f30fa0e43873848780a75cd8c8a3f85998d1d
fresh source root：G:\gameEngin-256-p8r2-f0bfc74
Gate root：G:\EngineTest\Gate256P8R2-f0bfc74
canonical ledger：previous_ledger_readable=true；added/removed/changed/new_suppressions/expired 均为 0
ledger diff SHA-256：0daf35bbc8556133023a548432b25aeb9e7c4f7b9669923874ab3553c84f0934
candidate ledger SHA-256：eae15c0bf78da17c2dab9d3a225091580ba893a584323b8597a49cf8eb6c5dcc
P7 matrix：21/21 commands passed；1037 passed / 0 failed / 12 ignored
matrix command elapsed：971.086 seconds
verbatim TEMP/TMP：\\?\G:\EngineTest\Gate256P8R2-f0bfc74\temp
dialog exact：1 passed / 0 failed
editor_window_winit all-features：133 passed / 0 failed / 9 ignored
source status：clean
live Cargo/rustc/Editor/gateway processes：0
artifact：G:\EngineTest\Gate256P8R2-f0bfc74\artifact
evidence manifest SHA-256：f7d9058c266fe325fdea74486e77c665ae090b875ce5da3f0539cfcfcc499477
cleanup：not performed；显式精确路径 cleanup 命令在执行前被 command safety policy 拒绝
retained roots：source worktree、target、temp、artifact 均保留且禁止被 fresh P8 复用
fresh P8/P8A/P9/P10/historical Gates：not started
```

fresh P8 v2 第六次执行证据（2026-07-24）：

```text
candidate commit/tree：198373efd7816dc3e568d922ff00616f4da9d9df / 48d461de0096931cee56c3ed95a0ce5e6fbd0cac
source root：G:\gameEngin-256-p8-198373e
gate root：G:\EngineTest\Gate256P8-198373e
runner SHA-256：48a2708a239c8264d1d462a45fd6c40ffe4910ce5de36f471d4cdf484bcd07d5
wrapper smoke：passed；1 passed / 0 failed；target removed；descendant process count 0
wrapper smoke report SHA-256：47c0423e759b74d8300df9811b1d49c535acaa0ae2dc9cc8a964e9e3b7cbb3f9
started marker SHA-256：957d26c17e24e66803fe3f5ef2bb2200152ddf12ceab5ff1d4b749104b7d28e0
stage summary：34 passed / 1 failed
failed stage：cargo test -p project_e2e_gate --all-features --locked
failed test：tests::llm_worker_lifecycle_gate_covers_transport_join_shutdown_and_privacy
test summary：58 passed / 1 failed / 1 ignored
failure detail：repair_attempt status=Failed；cancel=Rejected；stage=Failed；diagnostic=llm_patch_source.no_active_request
failure report：G:\EngineTest\Gate256P8-198373e\artifact\p8-preflight-failure-report.v2.json
failure report SHA-256：b0b922cbd691df81a1fa75aa1a06cd517ae2252e3e45bb82657f85b805de3361
runner wall time：1476.412 seconds
p9Eligible：false
freeze/P8A/P9/P10：not created / not started
real Codex config before/after SHA-256：e6e3c722b65f456f893f3c48e86c76e43e3043aa25a453550d717632cb78f435 / same
cleanup：target removed；temp removed；source-local candidate removed；source final clean
live Cargo/rustc/runner/Editor/gateway processes：0
authorization：consumed；不得重跑
```

### 256-P8S1：Qualification/Freeze 边界修订

目标：停止旧的一体化 P8，把“发现基线不稳定”的可重复资格验证与“产生 production freeze”的一次性副作用分开，同时不减少任何最终验收覆盖。

授权与预算：

```text
授权：2026-07-24 用户单独授权
预算：最多 30 分钟
允许：只修改 256 正式方案与当前施工文档并完成自审
禁止：Rust/Runner/测试/fixture/ledger 修改；禁止运行 P8Q/P8F/P8A/P9/P10
```

结论：

1. 旧 `256-P8` 一体化合同停止，只保留六次历史 evidence。
2. 新增可重复、只读的 `P8Q Qualification`；它绑定 exact commit/tree、Runner、toolchain、环境和完整矩阵，但永远声明 `p9Eligible=false`。
3. 新增每份有效 qualification artifact 最多一次的 `P8F Freeze`；它只负责 production build/seal、真实窗口、isolated config lifecycle、cleanup 和 immutable freeze，不重复广泛回归。
4. lifecycle flaky 先在 `P8R3A/P8R3B` 独立修复和证明；不能用 P8Q 或 P8F 当调试循环。
5. 现有 construction Runner 必须在 `P8R3C/P8R3D` 完成内部命令和 artifact 分流后，才允许执行 P8Q。
6. 这套拆分只属于 construction governance，不进入 Registry、Catalog、MCP、产品 runtime 或 AI 使用引擎的工作流。

退出条件：正式方案、当前施工文档、后续授权顺序、artifact 代际和失败边界一致；文档自审通过。

失败动作：只修文档冲突；不进入任何后续施工。

### 256-P8R3A：LLM lifecycle synchronization 单点修复

目标：消除 `project_e2e_gate` lifecycle fixture 中两个相等 2 秒 deadline 形成的调度竞争。

预算：最多 45 分钟，单独授权。

范围：

- 只修改 `rust/crates/project_e2e_gate/src/llm_worker_lifecycle.rs` 与直接 owner 测试；
- readiness timeout 结果必须被检查，不能丢弃；
- mock repair server 使用显式 readiness/barrier，并持续等待 cancellation/disconnect；独立 hard safety deadline 只防永久挂起，不能与 client phase deadline 相等；
- 增加确定性回归，证明第二次 repair request active 时 cancellation 能被接受并完成 join/shutdown/privacy 检查；
- 不修改公共 256 Tool、Gateway、Editor、P8 Runner、artifact schema 或正式方案。

进入条件：P8S1 完成且用户单独授权。

退出条件：exact failing test 红/绿证据成立，scoped commit 形成；不运行 P8Q/P8F。

失败动作：保存定向 evidence 并停止，不扩大到 P8 Runner 或产品实现。

P8R3A 完成证据（2026-07-24）：

```text
P8S1 pure-doc baseline commit：00e002a11b973e06b3f84c5c44f90745c2bab4fd
implementation commit：e699cc31a1bf529d0d7a33e3c0d444ebae8773b6
implementation tree：4a9ccde56ab303e26da45c442b941b5ed50f775d
scope：rust/crates/project_e2e_gate/src/llm_worker_lifecycle.rs only
red exact：repair_second_request_outlives_the_old_phase_deadline；0 passed / 1 failed；read=Ok(0)
root cause confirmed：server reused the client 2 s phase deadline as its own lifetime
implementation：5 s required readiness barrier；explicit cancellation-release barrier；10 s independent hard safety deadline
owner green：2 passed / 0 failed
public lifecycle gate green：1 passed / 0 failed；final test runtime 5.99 s
format：cargo fmt -p project_e2e_gate -- --check passed
isolated target/temp root：G:\EngineTest\Gate256P8R3A-baseline-00e002a
cleanup：not performed；精确限定到上述 Gate root 的 PowerShell Remove-Item 在执行前被 command safety policy 拒绝
retained roots：target/temp 保留且禁止被 P8R3B 或任何后续 Gate 复用
live Cargo/rustc/project_e2e_gate/Editor/gateway processes：0
P8R3B/P8R3C/P8R3D/P8Q/P8F/P8A/P9/P10：not started
```

R3A 只修复 test-fixture synchronization，没有修改 `editor_core` lifecycle、公共 256 Tool、Gateway、P8 Runner、artifact schema 或 production composition。并发压力矩阵、`project_e2e_gate --all-features` 全 crate 与受影响 `editor_core` owner tests 属于 P8R3B，当前未执行。

### 256-P8R3B：LLM lifecycle 稳定性资格证明

目标：证明 P8R3A 不是一次偶然绿灯。

预算：最多 60 分钟，单独授权。

矩阵：

```text
exact lifecycle test
parallel stress matrix（至少覆盖旧 12 并发 binary / 3 failures 的负载形态）
cargo test -p project_e2e_gate --all-features --locked
受影响 editor_core lifecycle owner tests
```

进入条件：P8R3A remediation scoped commit clean，repository-owned compiled lifecycle stress harness 已在独立 preparation 中提交并完成 Gate 外 wrapper smoke，用户重新单独授权，fresh isolated target/TEMP/TMP/artifact root。

退出条件：stress 无失败、all-features 全绿、无残留 process、source clean；生成独立 stability report。

失败动作：回到 P8R3A owner 修复；不执行 Runner 分流、P8Q 或 P8F。

P8R3B 失败证据（2026-07-24）：

```text
source commit：05a1203ebf653680a7c37ba1a84e0cb5b876db31
source tree：c0ae1e4e65f1d7033da405c01e4ab35d3a25e00a
fresh source root：G:\gameEngin-256-p8r3b-05a1203
fresh Gate root：G:\EngineTest\Gate256P8R3B-05a1203
exact lifecycle baseline：1 passed / 0 failed；test runtime 5.55 s
12-way stress launcher：两次临时 PowerShell orchestration 均在结构化报告生成前 timeout，因此不计为有效资格证据
single direct control：同一已构建 test binary，exit 101，0 passed / 1 failed，10.16 s
repair_attempt externally visible result：CancelledJoined / Joined / Released / transport abort=true / connection close=true / request_count=2 / cancel latency=6 ms / cancel=Committed / stage=Cancelling / busy=true
first cause：P8R3A pass predicate 仍独立要求 cancellation_release_sent；真实 disconnect 已先让 server 退出并 drop receiver 时，随后 barrier send 返回 Err，导致外部生命周期全部正确仍判 Failed
classification：P8R3A test-fixture implementation defect；不是 256 public Tool design failure
project_e2e_gate --all-features：not run（fail-fast）
editor_core lifecycle owner tests：not run（fail-fast）
live Cargo/rustc/project_e2e_gate processes：0
external artifact：G:\EngineTest\Gate256P8R3B-05a1203\artifact\p8r3b-lifecycle-stability-qualification-failure.v1.json
external artifact sha256：1b6b117b7b3eda79642b3ef325d2766e6a50771370b89ec3cae8859945432387
repository report：施工文档/当前/256-P8R3B-lifecycle-stability-qualification-failure-report-v1.json
P8R3B authorization：consumed；不得自动重试
next：需要用户单独授权 P8R3A scoped remediation
P8R3C/P8R3D/P8Q/P8F/P8A/P9/P10：not authorized / not started
```

修复边界必须保持在 P8R3A owner：实际 transport disconnect 与显式 cancellation barrier release 应是 server-release 的两个可替代成功信号；不能把 disconnect 后 barrier receiver 已关闭解释为 lifecycle 失败。当前 P8R3B 授权不包含该代码修改。

P8R3A scoped remediation 硬合同：

```text
server-release 工程真相：
  repair server observed explicit cancellation barrier release
  OR
  repair server observed actual transport disconnect

cancellation_release.send(...) 的返回值只允许作为 diagnostic：
  send=Ok 不能单独证明 server 已释放
  send=Err 且 connection_close_observed=true 不能判 lifecycle 失败
  只有两个 server observation 都为 false 才能判 server-release 失败

必须先写并通过三个确定性 owner 场景：
  barrier-first：explicit cancellation barrier release 先发生并被 server 观察
  disconnect-first：transport disconnect 先发生，receiver 随 server 正常退出而关闭；整体仍通过
  neither：barrier 与 disconnect 都未观察到；必须 fail-closed

范围：
  只允许修改 project_e2e_gate lifecycle fixture、直接 owner tests 与 test-only diagnostics
  禁止修改 256 public Tool、AiProjectAccessModule、Gateway、Editor lifecycle、production schema 或 runtime
  禁止把 send timing、thread scheduling 或 barrier implementation detail 提升为产品合同
```

P8R3B compiled stress harness 硬合同：

```text
禁止：
  临时 PowerShell launcher
  Gate-local script
  顺序逐进程 WaitForExit
  无结构化 report 的并发命令
  在 fresh P8R3B qualification 内首次调试 wrapper

必须：
  使用 repository-owned compiled construction-only harness
  固定启动 12 个 exact lifecycle test binary child
  每个 child 使用唯一 TEMP/TMP、stdout、stderr 与 result identity
  runner CLI、12-way stage matrix、timeout 与 report schema 固定并受测试
  runner 绑定 source commit/tree、runner executable SHA-256 与 child executable SHA-256
  lifecycle harness 的 source identity 只由 exact commit/tree 与 build-time clean 构成；
    build source absolute path 只作审计信息，不得参与 fresh qualification source 的身份相等判定
  P8/P8A 既有同路径 lineage 合同保持不变，不得随 lifecycle remediation 放宽
  使用 shared overall deadline；外层 deadline 大于全部内部 deadline 与 cleanup/report reserve
  并行 wait-all；任一 timeout/abnormal exit 终止精确 descendant process tree
  原子 no-clobber 生成结构化 report，逐 child 记录 exit、duration、stdout/stderr digest 与 cleanup
  report 只有 12/12 passed、无 surviving descendant、source clean 时才能声明 stress passed

preparation 边界：
  harness 实现、测试和 representative Cargo wrapper smoke 必须在独立授权中完成并形成 clean commit
  wrapper smoke 在 Gate 外执行，不创建 P8R3B started marker，不生成 qualification evidence
  fresh P8R3B 只能消费已经 smoke 通过的精确 harness binary，不得现场修 runner
```

P8R3B harness lineage remediation/rebind smoke（2026-07-24，单独授权，预算最多 60 分钟）：

```text
原因：
  已 smoke binary 把 build source absolute path 当身份字段，
  与 fresh P8R3B 必须使用全新 source root 形成循环绑定

允许：
  只修改 quality_gate lifecycle stress lineage 校验与直接测试
  lifecycle stress 接受不同 absolute root 上的 exact commit/tree clean source
  形成 clean commit 后，在全新 preparation source/target/TEMP/artifact root 构建
  对 exact copied runner/test binary 执行一次 Gate 外 1-child wrapper smoke

禁止：
  不放宽旧 gate-256-p8 / P8A 的同路径 lineage
  不执行 12-child stress qualification
  不创建 P8R3B qualification evidence
  不执行 P8R3C/P8R3D/P8Q/P8F/P8A/P9/P10

退出条件：
  revision/cleanliness drift fail-closed 测试通过
  fresh-root relocation 正向测试通过
  quality_gate 回归通过
  clean exact commit、runner/test executable digest 与 fresh wrapper smoke evidence 冻结
```

完成证据：

```text
commit：7e91de5b3ae4abacef3a14faa92ff4c7474d0b50
tree：5781059f37b6e3a4ab666331df69552a45bcda48
directed lineage tests：2 passed / 0 failed
quality_gate：92 unit tests passed；全部 integration/doc tests passed
build source 与 smoke source：两个不同的 fresh detached exact-commit root
runner sha256：aad43707ba37b938bda5b0a2b236ccb3cb7055ccf4076c967227c7787540a8e6
test executable sha256：0b60a813fd43ceca796c664bbf45adacfe254f78960151c107818cde5047349c
wrapper smoke：1/1 passed；exit 0；timeout=false；survivor=0；temp removed
smoke report：G:\EngineTest\Gate256P8R3BLR-7e91de5-smoke\artifact\p8r3b-lifecycle-stress-wrapper-smoke.v1.json
smoke report sha256：4e5e18d63e068a8a6d80c68813bab766d3500f5c02787a137af89941f027f72f
cleanup：两个 detached source worktree、development/build Cargo target 与 build/launcher TEMP roots 均已删除
repository report：施工文档/当前/256-P8R3B-harness-lineage-remediation-report-v1.json
fresh P8R3B qualification/started evidence：0；authorization not consumed
P8R3C/P8R3D/P8Q/P8F/P8A/P9/P10：not started
```

这两组合同只属于 lifecycle test/construction governance，不修改 256 正式方案的产品 Interface，也不进入 AI-facing Tool Catalog、MCP schema 或用户工作流。

P8R3A scoped remediation 与 compiled harness preparation 完成证据（2026-07-24）：

```text
P8R3A scoped remediation commit：0dd84afcc454b1b5aa347fb28379953b8f5a6e4c
P8R3A scope：rust/crates/project_e2e_gate/src/llm_worker_lifecycle.rs only
P8R3A owner tests：barrier-first / disconnect-first / neither；5 passed / 0 failed
P8R3A public exact lifecycle：1 passed / 0 failed

compiled harness implementation commit：717427d094a65b4fa9b81cde99bcb2d23978752b
cleanup leaf remediation commit：33dd6d603ff11177113d90b22613acf6a748c3c8
exact tree：68c8c6e46f710c6768ceb77d05ca143f71ec74db
construction-only commands：gate-256-p8r3b-stress-smoke / gate-256-p8r3b-stress
fixed child count：smoke=1；stress=12
quality_gate regression：90 unit tests 与全部 crate integration tests passed

first wrapper smoke：717427d… failed only because safe cleanup expected leaf 参数错误；
  child exact lifecycle 1 passed / 0 failed、exit=0、no timeout、no survivor；
  failure evidence 保留在 G:\EngineTest\Gate256P8R3BH-717427d-smoke
fresh wrapper smoke：33dd6d6… passed
smoke report：G:\EngineTest\Gate256P8R3BH-33dd6d6-smoke\artifact\p8r3b-lifecycle-stress-wrapper-smoke.v1.json
smoke report sha256：36694fef16b114cd46c5eae75396c9d31d431c9f695f51f76b089fec4ac52fd0
smoke facts：required/actual child=1/1；exit=0；timedOut=false；
  descendantCleanup=true；survivingProcessCount=0；tempRootCleanup=removed；
  sourceInitialClean/sourceFinalClean=true；JSON sidecar matched
qualification/started evidence：0；fresh P8R3B not run / not authorized

baseline owner diagnosis（2026-07-24）：
  target test：tests::llm_provider_repair_complex_shooter_fake_http_e2e
  historical failure diagnostic：project_runtime.player_artifact_mismatch
  player descriptor digest：64710c…；RuntimePackage expected digest：32e83e…
  root cause：不同 source worktree 错误复用同一 CARGO_TARGET_DIR，
    legacy static-linked complex_shooter_player.exe 成为 stale artifact
  fresh exact source：3f188dd30e048772a49aecb72b94e9318a459f2e
  fresh exact test：5 passed / 0 failed
  fresh full all-features：63 passed / 0 failed / 1 explicit ignored；
    integrations：1 + 1 + 3 passed；doc tests passed
  stdout sha256：9990e4dce082fd8fec19426a1cf7ec07d160f7c630b1654712e76e2bcc28876c
  stderr sha256：02282bfc088cc6a1241e996f13d9ad307bb1fd2edf0b3806db8c4e8d01bca9b5
  repository report：施工文档/当前/256-P8R3B-baseline-owner-diagnosis-report-v1.json
  classification：construction environment ownership defect；not code baseline failure
  code change：none
  next：可另行授权一次 fresh P8R3B；必须使用全新且不跨 source 复用的 target/TEMP/artifact root
```

P8R3B fresh lifecycle stability qualification 完成证据（2026-07-24）：

```text
exact commit：7e91de5b3ae4abacef3a14faa92ff4c7474d0b50
exact tree：5781059f37b6e3a4ab666331df69552a45bcda48
fresh source/target/TEMP/evidence：独立且未复用任何历史或 remediation root
exact lifecycle：1 passed / 0 failed
compiled stress invocation：1 次且仅 1 次；12/12 passed；timeout=0；survivor=0
stress report sha256：258bbad47f32a81175fb82e02cd0228f1cdd99b51b2a68ec9bc0d7eaeaa9c98f
project_e2e_gate --all-features：63 passed / 0 failed / 1 explicit ignored；
  integrations 1 + 1 + 3 passed；doc tests passed
editor_core lifecycle owners (`llm_`)：42 passed / 0 failed / 1 ignored
source initial/final clean：true / true
cleanup：source worktree、Cargo target、Cargo TEMP、stress TEMP removed；live process=0
external artifact：G:\EngineTest\Gate256P8R3B-7e91de5-20260724-175543-evidence\p8r3b-lifecycle-stability-qualification.v1.json
external artifact sha256：88f0c0e47bed3ec2c0c68244b2b3a0108c00a20fe158cf9aa179550a6881fa8d
repository report：施工文档/当前/256-P8R3B-lifecycle-stability-qualification-report-v1.json
P8R3B authorization：consumed；passed；不得重跑
P8R3B 完成时后续状态：P8R3C/P8R3D/P8Q/P8F/P8A/P9/P10 not started / not authorized
P8R3B 完成时 next：可单独授权 P8R3C construction Runner Qualification/Freeze 分流
```

### 256-P8R3C：construction Runner Qualification/Freeze 分流

目标：在 repository-owned `quality_gate` 内把旧 `gate-256-p8` 一体化入口收敛为两个 construction-only 命令和互不混淆的 artifact schema。

预算：最多 60 分钟，单独授权。

必须先写红测试并实现：

```text
gate-256-p8-qualify
  -> 只运行 ledger/P7/default/all-features/历史失败/环境等价矩阵
  -> 禁止 production build/seal、真实窗口、config lifecycle、真实 config 访问
  -> 输出 p8-qualification-report.v1.json 或 p8-qualification-failure.v1.json
  -> success artifact 明确 p9Eligible=false

gate-256-p8-freeze
  -> 必须接收 immutable qualification path + exact SHA-256
  -> 在 started marker 前验证 qualification/source/commit/tree/runner/toolchain/environment lineage
  -> 只运行 production build/seal、真实窗口、isolated config lifecycle 与 cleanup
  -> 输出 p8-production-freeze.v3.json 或 p8-production-freeze-failure.v3.json
```

两个命令、started marker、success/failure artifact 和 sidecar 均须 no-clobber。旧 `gate-256-p8` 入口必须 fail-closed 或仅保留明确的历史诊断用途，不能再创建新资格或 freeze。

范围只限 `quality_gate` construction runner 与直接测试；不得进入产品 Tool/Catalog/MCP/runtime。

进入条件：P8R3B 全绿，用户单独授权。

退出条件：schema、lineage、negative tests 与命令职责全部通过，形成 scoped commit；不执行正式 P8Q/P8F。

失败动作：只修 Runner owner，禁止用实际 one-shot Freeze 调试。

P8R3C 完成记录（2026-07-24）：

```text
baseline remediation commit：
  ef3a2fb fix(quality): reconcile lifecycle lint baseline

P8R3C implementation commit：
  e785e2f0afd11615384b45725a0a6ce87adc535c
tree：
  d45fe1578ccd32cb5538f275710ca987931ac4a7

实现：
  gate-256-p8-qualify
  gate-256-p8-freeze
  旧 gate-256-p8 fail-closed
  qualification/freeze 使用不同 request、started marker、success/failure schema 与 sidecar
  freeze 在 started marker 前验证 immutable qualification SHA-256 及 source/commit/tree/runner/toolchain/environment lineage
  qualification 禁止 production build/seal、真实窗口、config lifecycle 与真实 config 访问
  freeze 禁止重跑 qualification matrix，真实 config 访问保持 false

验证：
  canonical ledger previousLedgerReadable=true
  added/removed/changed/newSuppressions/expired = 0/0/0/0/0
  cargo fmt --check passed
  quality_gate --all-features：94 passed / 0 failed
  gate_256_p8_contract：9 passed / 0 failed
  lifecycle exact：1 passed / 0 failed

construction evidence：
  G:\EngineTest\Gate256P8R3C-e785e2f-20260724-evidence\p8r3c-construction-runner-split.v1.json
sha256：
  623f71265ed7496186b97551677587b0f5b7300503c4fbf1d1993c8be2bac18c

未执行：
  P8R3D/P8Q/P8F/P8A/P9/P10
```

### 256-P8R3D：Runner 分流回归与 wrapper smoke

目标：在消费 P8Q 前证明新 Runner 的命令执行、退出码、timeout、descendant cleanup、artifact 原子发布和职责隔离。

预算：最多 45 分钟，单独授权。

要求：

- 运行 Runner 定向/负向测试和受影响 `quality_gate` 回归；
- 对 qualify 命令做代表性 Cargo wrapper smoke；
- 对 freeze 命令只做不产生 production 副作用的 argument/lineage/negative smoke；
- 断言 qualification 路径不会构建或启动任何 production binary，也不会读取真实 Codex config；
- 断言 freeze 在无效/漂移 qualification 下不会创建 started marker；
- source clean、descendant process count 为 0、临时 target cleanup 完整。

退出条件：独立 regression report 全绿；随后才可申请 P8Q。

P8R3D 完成记录（2026-07-24）：

```text
source commit：
  cc341eb0d602302e347bda99013c1e5ceb3d32e1
source tree：
  fb4e2b1bbc9d381b19495a8c6071e86b075a8b80
source root：
  G:\EngineTest\Gate256P8R3D-cc341eb-20260724-source
run root：
  G:\EngineTest\Gate256P8R3D-cc341eb-20260724-run
exact copied Runner sha256：
  753b82728253bd8794f16cac7f187397d23a8c213e09c419f628b0c447cbf739
Gate 外 representative Cargo wrapper smoke：
  1/1 passed；exit 0；timeout=false；descendant=0；target/TEMP removed
wrapper smoke report sha256：
  2b883cc01e17510bcdd3c61b6d66ad4718a642a44148664edfe4e22f9148eb08
directed contract：
  9 passed / 0 failed / 0 ignored
affected quality_gate：
  118 passed / 0 failed / 0 ignored
qualification negative：
  real config option 与 override 均在写入前拒绝；gate root absent；真实 config sentinel absent
freeze negative：
  漂移 qualification fixture 与 sidecar digest 一致；qualification_lineage_mismatch；
  gate root absent；p8-production-freeze-started.v3.json absent
production side effects：
  production binary 0；Editor launch 0；真实 config read/write 0；P8Q/P8F/P8A/P9/P10 0
cleanup：
  source clean；descendant=0；build/regression/wrapper target 与 TEMP 全部 removed
regression report：
  G:\EngineTest\Gate256P8R3D-cc341eb-20260724-run\evidence\p8r3d-runner-split-regression.v1.json
regression report sha256：
  4bdc826760cb3cb538454d0e351a36f5d99c21e48025801d22ece407c082b041
diagnostic note：
  freeze 负向 smoke 首次 shell 封装因 Windows PowerShell 5 不支持 ProcessStartInfo.ArgumentList，
  Runner 仅收到空参数并返回 usage；未进入 freeze 子命令、未写 gate root、未消费授权。
  随后使用直接 argv 完成规定的同一负向检查。
next：
  fresh P8Q；必须由用户单独授权
```

### 256-P8Q：fresh exact-commit Qualification

目标：在不消费 Freeze 授权的情况下，对一个 fresh exact commit 完成全部基线资格验证。

预算：最多 90 分钟，单独授权；可在失败修复形成新 commit 后重新申请，但每次必须使用 fresh root。

任务：

1. 只读核对 exact clean commit/tree、Runner self/build-source lineage、toolchain、line endings、容量和隔离路径。
2. 运行 canonical ledger producer并结构化对账。
3. 运行完整 P7 累计失败集、default/all-features、历史 P8 失败矩阵和 lifecycle stability matrix。
4. 使用与后续 Freeze 等价的 `CARGO_TARGET_DIR`、verbatim `TEMP/TMP`、环境 allowlist 与 process cleanup 合同。
5. 生成 immutable `p8-qualification-report.v1.json` 与 SHA-256 sidecar；绑定所有命令、计数、ignored tests、artifact digest、Runner/toolchain/environment identity 与 cleanup。
6. 明确记录 `p9Eligible=false`、`productionCompositionBuilt=false`、`freezeAuthorizationConsumed=false`。

禁止：

- build/seal production composition；
- 启动 Native Editor real-window；
- 执行 isolated/real config lifecycle；
- 读取或修改真实 Codex config；
- 创建任何 freeze/activation/P9/P10 marker。

退出条件：qualification artifact 全绿且 immutable，source/isolated clean、cleanup 完整。

失败动作：只生成 qualification failure artifact并停止；Freeze 授权未消费。代码或合同修复必须形成新 commit，再重新获得 P8Q 授权。

P8Q terminal failure 记录（2026-07-24）：

```text
source commit：
  058d4b2fa84c8ef78ca931a38d78ccfb734d71fb
source tree：
  0de11cb5456094645b49ec999a8401098de9f596
source root：
  G:\EngineTest\Gate256P8Q-058d4b2-20260724-source
run root：
  G:\EngineTest\Gate256P8Q-058d4b2-20260724-run
Runner sha256：
  c27ae515f347acbf15e177d4fbc12028f68a01171578277ef60efe937e415906
wrapper smoke：
  passed；report sha256=1b1c9b3abfb95468513edf1d4300317ce949757b7f755730c580df53f947a59d
started marker sha256：
  ba0ece8bcd695215f5fbb4677f470e11bf1079b5f8eff9202d4ecb40870ff2d5
qualification outcome：
  terminal_failed；27 stages passed / 1 stage failed；不得重跑本次授权
first failure：
  p7-editor-host-gateway-process
  cargo test -p editor_host --test gateway_process --locked
  test result=2 passed / 0 failed / 0 ignored；Cargo exit=0；timedOut=false
  descendantsObservedBeforeCleanup=1；descendantCleanup=true；descendantProcessCount=0
classification：
  implementation/test-fixture lifecycle ownership；不是 256 产品设计、施工文档或 Runner 合同错误
bounded diagnosis：
  gateway_process fixture 在 project.preview 到 awaiting_frame_evidence 后关闭 adapter 并结束 Editor preflight，
  未显式证明 Preview child 自然终止；failure evidence 只记录 survivor count、未记录 executable identity，
  因此精确 survivor owner 必须在独立 scoped remediation 中补证并修复
canonical ledger：
  previous readable；added/removed/changed/new suppressions/expired 均为 0
cleanup：
  sourceFinalClean=true；target/temp/source-local-candidate removed；run process count=0
prohibited：
  p9Eligible=false；realConfigAccessed=false；P8F/P8A/P9/P10 均未执行
failure artifact：
  G:\EngineTest\Gate256P8Q-058d4b2-20260724-run\gate\artifact\p8-qualification-failure.v1.json
failure artifact sha256：
  ccfa80ea8be687a0c1d1bab133bd7559e4bb84d99f0da5272e8b41201d68ec35
diagnosis artifact：
  G:\EngineTest\Gate256P8Q-058d4b2-20260724-run\evidence\p8q-failure-diagnosis.v1.json
diagnosis artifact sha256：
  9fea514b658b048d914af2c787970db21f336806576264316a1ab0b9a83efaad
next：
  单独授权 scoped gateway_process/Preview descendant ownership remediation；
  形成新 code commit 后才能申请新的 fresh P8Q
```

### 256-P8F：单次 production Freeze

目标：消费一份仍有效的 P8Q artifact，对同一 exact commit/tree 生成 production-eligible immutable freeze。

预算：最多 60 分钟，每份 qualification artifact 最多单独授权一次。

任务：

1. started marker 前只读验证 qualification artifact/sidecar、source、commit/tree、Runner、toolchain、environment 和 source cleanliness未漂移。
2. 用明确 production profile/features 构建 `editor_host`、`ai_engine_gateway_mcp`、`ai_engine_gateway_codex_config`。
3. seal/reopen 三 binary并记录 source/sealed/reopen digest。
4. 从 sealed Editor 创建真实顶层窗口、验证 launcher discovery/readiness、正常有界关闭。
5. 用 sealed config helper 对 isolated config 完成 install/replace/conflict/receipt/rollback exact restoration。
6. 验证真实 Codex config before/after digest相同，但不保存正文。
7. 保存 cleanup、capacity和无残留 descendant evidence。
8. 生成 immutable `p8-production-freeze.v3.json` 与 SHA-256 sidecar；只有全部通过才能声明 `p9Eligible=true`。

P8F 不重复 ledger、P7、default/all-features 或 lifecycle stress matrix。首次原子创建 `p8-production-freeze-started.v3.json` 才消费授权。

进入条件：P8Q artifact 全绿、未消费、lineage 未漂移，用户单独授权。

失败动作：生成 terminal freeze failure artifact并立即停止；同一 qualification、Gate root 和授权不得重试。修复导致 source/Runner/toolchain/合同变化时，必须重新 P8Q。

后续固定切片：P8A 单独授权且最多 30 分钟；P9 必须在 reload/new task 后单独授权且最多 90 分钟；P10 单独授权，只有保守预计连同 evidence/cleanup 不超过 3 小时时才允许启动。

### 256-P8A：fresh freeze 后的 Codex config activation

目标：只在 fresh P8F `p9Eligible=true` 后，把真实 Codex config 精确指向该 freeze 的 sealed MCP，并留下可回滚的 durable receipt；该步骤不启动或消费 P9。

要求：

1. 重新 hash sealed MCP 与 sealed config helper，必须匹配 fresh freeze。
2. 只读取真实 config 的结构化 current command 与 before digest，不把正文写入 artifact。
3. 已有 `ai_first_game_engine` 条目必须用 exact expected-current `replace`；没有条目才允许 `install`。
4. 只能调用 fresh freeze 中 sealed 的 `ai_engine_gateway_codex_config.exe`。
5. 持久化并验证 receipt、after digest、commandDigest、MCP binary SHA-256、backup/fragment path 与 `reloadOrNewTaskRequired=true`。
6. receipt persist 失败时要求 helper 自动 rollback；rollback 失败则保存双重 diagnostic 与人工恢复材料并立即停止。
7. 生成独立 `artifact/p8-config-activation.v3.json` 与 SHA-256 sidecar，绑定 immutable P8 Qualification/P8F Freeze、三 binary digest、真实 config before/after digest 与 durable receipt；不得修改、替换或补写 `p8-qualification-report.v1.json` 或 `p8-production-freeze.v3.json`。
8. activation 失败只生成独立 `artifact/p8-config-activation-failure.v3.json` 与 SHA-256 sidecar，不得生成 success artifact 或改写 freeze。
9. activation 成功后立即停止当前 task，要求 reload/new task；当前 task 不得声称 typed MCP 已热加载。
10. activation 不授权 P9，不启动 Editor，不创建 request，不请求用户 approval。
11. 交接必须明确后续顺序是“另行授权 P9 -> 先启动 sealed Editor 并等待 launcher discovery ready -> 再 reload/new Codex task -> 真实 typed MCP 验证 provenance/session”；本 task 只记录命令与证据路径，不执行该启动。

固定 construction-only 入口：

```powershell
& "<fresh-freeze-runner>\quality_gate.exe" gate-256-p8-config-activate `
  --freeze "<fresh-freeze-root>\artifact\p8-production-freeze.v3.json" `
  --expected-freeze-sha256 "<64-lowercase-hex>" `
  --real-codex-config "<current-Windows-profile>\.codex\config.toml"
```

该子命令只属于 repository-owned construction runner，不进入
`AiToolContractRegistry`、Catalog、MCP 或产品 runtime。调用它的 executable 必须就是 freeze
绑定的同一 copied runner；caller 不能提供 helper、MCP、report、receipt、backup、fragment、
recovery root 或 rollback target。runner 必须先只读自校验 freeze/report/runner/smoke、三
binary、真实 current-user config identity 与 before digest，并拒绝任意替身 config。

私有恢复目录只能由 runner 按 freeze digest 派生：

```text
%LOCALAPPDATA%\AiFirstGameEngine\Gateway\codex-config-activation\
  gate-256-p8-<freeze-sha256-prefix>\
    logs\
    material\
    temp\
```

真实 config 的 backup、fragment、durable receipt 与 helper stdout/stderr 只能保存在该私有
目录。P8 artifact 只记录路径、digest、receipt 字段和状态，禁止复制 config、backup 或
fragment 正文。私有 root、success/failure artifact 或 started marker 任一已存在都必须在
副作用前 fail-closed。

`artifact/p8-config-activation-started.v3.json` 只能在上述只读校验和私有目录安全校验全部
成立后由 runner 原子发布；该 marker 是唯一 P8A 授权消费点。marker 之前失败不得读取以外
修改真实 config，也不得生成 activation success/failure artifact。marker 之后只允许一次
sealed helper install 或 exact expected-current replace：

```text
helper 非零且真实 config 已 exact 自动恢复：
  不执行第二次 rollback；保存 failure artifact 后停止
helper 非零且未 exact 恢复：
  不信任或拼装 receipt；保留私有恢复材料，保存 failure artifact 后停止
helper 成功：
  先完整验证 durable receipt、config postcondition 与 frozen lineage
  后续任一验证或 success-artifact 发布失败：
    只允许用该已验证 receipt 调用 sealed helper rollback 一次
    验证 byte-exact before 恢复与 recovery material digest 后保存 failure artifact
  全部通过：
    保存 success artifact，真实 config 保持指向 sealed MCP，立即停止并要求 reload/new task
```

rollback 不接受 caller 路径，不能从未验证 receipt 猜测 backup，也不能在 helper 已自动恢复
后重复执行。success 与 failure artifact 必须互斥并采用 sidecar-first、JSON-last 发布；任何
孤立或无法清理的 success JSON/sidecar 都使 P8A 失败。P8A 不得修改 immutable P8
Qualification/Freeze。

进入条件：本次 P8F 全绿且 `p9Eligible=true`。

退出条件：真实 config activation receipt 可核验、rollback material 完整、明确需要 reload/new task；随后立即停止。

失败动作：保留 evidence；能证明自动 rollback 成功时确认真实 config exact 恢复，否则报告人工恢复边界。不得重试 P8F，不得执行 P9/P10。

### 256-P9：真实 Native Editor-hosted production MCP conformance

目标：证明真实产品组合能从 launcher/no-project 完成 bootstrap 与一次用户授权决议。

重要边界：

- 本 Gate 不是 254-R1/R2、F-A、ProductionCandidate acceptance 或真实 AI outcome evaluation。
- 不冻结用户游戏目标，不规定后续 Tool 顺序。
- 不运行任何 project mutation、Preview、Build 或 rollback。
- Test JSON-RPC client、CLI、headless preflight 不能替代本 Gate。

前置：

1. 使用 fresh P8F `p9Eligible=true` frozen exact commit 的 sealed `editor_host`、`ai_engine_gateway_mcp` 与 `ai_engine_gateway_codex_config`；三 binary hash、profile/feature 与 freeze 完全一致。
2. post-freeze Codex config activation receipt 可核验，config command 指向 sealed MCP，且 MCP content SHA-256 与 freeze 一致。
3. 用户单独授权本 Gate，并知道需要在 Native Editor 对唯一一张 request 作一次批准或拒绝。
4. 在启动前只读核对 sealed binary、config receipt、exact commit/tree、fresh discovery root 与 artifact root；任一不成立立即停止且不消费 P9。
5. 授权后先从 sealed path 启动 Native Editor 真实窗口并等待 launcher discovery ready；该进程启动是 P9 attempt 的首次消费动作，之后任何启动/readiness 失败都使本次 P9 失败且不得重试。
6. launcher discovery 必须绑定该 sealed Editor PID，记录 `binding=launcher`、publication generation 与 discovery record digest。
7. 只有第 5-6 项成立后，才 reload 或新建 Codex task；真实 `ai_first_game_engine` typed MCP 必须确认 tools/provenance/session。MCP binary 启动时只解析一次 discovery，因此禁止先开新 task 再补启动 Editor，也不得用 shell、CLI 或 test client代替 typed MCP。
8. discovery root、Editor process、MCP/config binary、commit/tree、config command digest、binary content digest 与 artifact root 全部记录。

真实链路：

```text
production Native Editor launcher
  -> production launcher discovery
  -> real installed typed MCP tools/list + Catalog
  -> typed project.create 或 project.open（二选一，按 Gate fixture）
  -> Editor-owned Command committed
  -> bootstrap receipt
  -> old launcher Session reconnect-required
  -> new project discovery
  -> fresh typed MCP Session exact binding
  -> typed session.request_mutation_access
  -> awaiting_user 且 Native Editor 显示唯一 request
  -> 用户在 Native Editor 批准或拒绝一次
  -> status/accessGeneration/receipt 验证
  -> 立即停止
```

验收证据：

```text
exact commit/tree/binary digest/config command digest
launcher 与 project discovery record digest/generation
old/new session binding identity
Editor Command result/receipt
project identity/root digest
request digest/access generation
Native Editor decision receipt
未执行 project mutation 的 operation/receipt count 证明
cleanup 与 project fixture recoverability
```

执行次数：

```text
每个 frozen exact commit 最多一次 production conformance。
失败后不得自动重试。
先分类设计/文档/实现/环境/人工决议问题；任何 source/Runner/toolchain/合同修复都必须重新 P8Q/P8F；再次 P9 需要用户重新授权。
```

预计成本：中，含人工批准时间；不应包含完整 workspace 构建。

进入条件：fresh P8F `p9Eligible=true`、真实 config activation receipt、用户单独授权与第 4 项只读核对成立。第 5 项启动 sealed Editor 时才消费 P9；随后 launcher discovery ready 后才能 reload/new task 并建立真实 typed MCP Session。

退出条件：链路到一次真实 access decision 全部可审计，且没有 mutation/Preview/Build。

失败动作：保存原始 evidence 后立即停止，不补做第二次 request，不运行 P10。

### 256-P10：一次最终权威 Local CI

目标：对通过 P9 的同一 frozen exact commit 运行 repository-owned authoritative Local CI。

前置：

- P9 通过且 source/fixture/ledger 未变；
- source worktree clean；
- target/temp/artifact/nested worktree 容量满足峰值与余量；
- 用户明确授权最终权威运行；
- authoritative run count 为 0。

命令：

```powershell
cd rust
cargo run -p quality_gate --locked -- local-ci --commit HEAD
```

要求：

```text
exact commit/tree 与 P8Q/P8F/P9 相同
default/all-features、fmt、Clippy/ledger、architecture/suppression 全绿
source/isolated clean
quality/local-ci reports 完整且 digest 可核验
nested worktree cleanup=removed
authoritative run count=1
```

预计成本：高；外层 timeout 必须大于内部 stage timeout 总和并保留 checkout/report/cleanup 余量。

退出条件：Local CI `passed=true` 且 cleanup 完整。

失败动作：保存 artifact 并停止；不得对同一 frozen commit 自动重跑。修复需新 commit、重跑 P8Q/P8F/P8A/P9，并重新获得 P10 授权。

### 256-P11：完成记录、入口同步与归档

目标：只有 P0-P10 全部通过后，关闭施工真相。

任务：

1. 生成 `阶段完成记录/2026-xx-xx-AI-Project-Bootstrap-Mutation-Access-Handshake-v1/00-总览.md`。
2. 记录 exact commit/tree、每 Gate 命令/结果/计数/耗时、失败分类、累计失败集、P9/P10 运行次数、artifact digest 与 cleanup。
3. 更新 256 正式方案施工状态。
4. 同步 README、00-文档地图、00-阅读顺序、240、49、54、施工文档/README、阶段完成记录/README。
5. 将本文从 `待执行/` 或 `当前/` 移到 `施工文档/已完成/`。
6. 确认 `当前/` 与 `待执行/` 没有重复副本。
7. 不自动开始下一个系统，不运行项目 mutation、历史 Gate 或三引擎 B 通道。

预计成本：低，只改文档与 evidence 索引。

进入条件：P9/P10 均通过，所有完成证据可读。

退出条件：入口、方案、施工归档和完成记录一致，工作树 clean。

失败动作：只修文档/索引闭环，不修改 frozen Rust commit；重新执行入口一致性检查。

## 8. 三层验证摘要

| 层级 | Gate | 目的 | 进入条件 | 失败后动作 |
|---|---|---|---|---|
| 定向测试 | P1-P6 | schema、launcher protocol、ProjectAccess、rebind、approval、Catalog/MCP | P0 全绿且上一 Gate 收敛 | 当前 owner Gate 精确修复并加入累计失败集 |
| 受影响域 | P7 | editor_core/Gateway/Host/Native Editor/E2E consumer | P1-P6 全绿 | 回到责任 Gate，禁止直接完整 workspace |
| 资格验证 | P8R3B/P8R3D/P8Q | lifecycle 稳定性、Runner 职责隔离、fresh exact-commit 完整矩阵 | 对应 scoped implementation 已提交且 fresh 隔离成立 | 修责任 owner；不得消费 P8F |
| 生产/最终权威 | P8F/P8A/P9/P10 | 三 production binary、sealed real-window/config activation、production MCP、Local CI、cleanup | P8Q immutable artifact 有效且分别授权 | 保存 evidence；不得自动重试一次性 Gate |

P8Q 不能产生 freeze 或替代 P8F；P8F 不能替代 P9/P10；P9 只证明真实 bootstrap/handshake，P10 只证明 frozen commit 的完整工程质量，四者不能互相冒充。

## 9. 高风险验证清单

```text
[ ] no-project 时 production Gateway Host 真实存在
[ ] launcher binding 使用显式 union，不用空 project identity
[ ] launcher Session 无 read/mutation authority
[ ] 19 个 Registry Tool 单一真相
[ ] project.create/open direct input 不含内部字段
[ ] Create/Open 只走 Editor Command
[ ] ProjectWriteScope/link/reparse/target conflict 保持
[ ] open 不改 manifest，不自动迁移
[ ] bootstrap receipt 绑定 committed project identity/root digest
[ ] launcher->project 与 project->project 都显式 reconnect
[ ] 旧 Session/Grant/request/operation/receipt stale
[ ] publication failure 不重复创建项目
[ ] idempotency replay 不重复副作用
[ ] typed mutation request 只产生 awaiting_user
[ ] pending request 不重复 approval card
[ ] Native Editor approve/reject 是唯一决议
[ ] TTL/disconnect/switch/race 单一 generation
[ ] request receipt 不是 Grant
[ ] Catalog 与 execute 同源 owner guard
[ ] tools/list 稳定且没有 raw/recipe Tool
[ ] Catalog/status 无副作用
[ ] retired R1/R2 lifecycle 继续 absent
[ ] production conformance 不执行 mutation/Preview/Build
[ ] editor_host production profile/feature 明确包含 real-window 与实际 surface
[ ] editor_host/MCP/config helper 三 binary source/sealed/reopen digest 相等
[ ] sealed editor_host 真实创建顶层窗口、launcher ready、正常有界关闭
[ ] isolated config install/replace/conflict/receipt/rollback exact 恢复
[ ] config command digest 与 MCP binary content digest 分开验证
[ ] receipt persist failure 自动 rollback；双失败不虚报恢复
[ ] P8Q qualification 明确 p9Eligible=false 且无 production/config 副作用
[ ] P8F 只消费 exact-bound qualification，不重复广泛矩阵
[ ] fresh P8F freeze 明确 p9Eligible=true
[ ] 真实 config activation receipt 与 reload/new task 要求成立
[ ] final exact commit/tree/binary/config/artifact digest 一致
[ ] source/isolated clean、cleanup removed
```

## 10. 失败收敛规则

1. P0 canonical ledger、baseline、容量或入口失败：停止，不写红测试。
2. P1 非预期红：先修测试设计，不把 fixture/环境失败当需求证据。
3. 用空字符串/magic id 表示 launcher：协议设计失败，回退 P2。
4. Gateway/Adapter 直接写项目：owner 偏离，回退 P3。
5. 原 Session 偷换 project binding：安全失败，回退 P4。
6. typed Tool 能 approve 或取得 caller-owned grantRef：授权设计失败，回退 P5。
7. MCP 手写第二份 schema/policy：Registry 单一真相失败，回退 P6。
8. 受影响域出现新失败：加入累计失败集，回到责任 Gate；不直接跑 workspace。
9. P8Q 任一 source/test/manifest/lockfile/ledger/fixture、Runner/toolchain 或环境 lineage 变化：qualification 失效，重新 P8Q；不得进入 P8F。
10. P8Q 失败：只生成 qualification failure，不消费 P8F；修复形成新 commit 后重新授权 P8Q。
11. P8F 缺任一 required binary、sealed window smoke、isolated config receipt/rollback 或 cleanup：必须 terminal failed 且 `p9Eligible=false`；同一 qualification/授权不得重试。
12. P9 失败：保存 evidence 后停止；不得补发第二张 request 或自动重试。
13. P10 失败：同一 commit 不重跑；分类首因，新 commit 重新 P8Q/P8F/P8A/P9，并重新授权。
14. 任一 Gate 发现需要产品或 AI-facing Runner、candidate lifecycle 或 outcome acceptance：停止并修正式方案，不在施工文档扩项；repository-owned construction runner 不属于该禁令。

## 11. 环境与路径合同

对应 Gate 必须把以下值写入 P0/P8Q/P8F/P9/P10 evidence：

```text
source worktree absolute path
exact commit/tree
CARGO_TARGET_DIR
TEMP/TMP
launcher/project fixture root
discovery root
pipe locator owner/process
artifact root
Local CI nested worktree root
各卷 free bytes before/peak/after
Git line-ending/attribute state
Rust/Cargo version
editor_host/MCP/config helper binary digest、Cargo profile 与 feature set
Codex/real MCP command digest（P8F isolated smoke、P8A activation、P9）
Codex config before/after digest、activation receipt 与 rollback material
```

路径要求：

- 所有 worktree、target、fixture、artifact root 必须显式绝对路径；
- 不使用 workspace root、用户主目录或盘符根作为清理目标；
- recursive cleanup 前重新解析并验证目标仍位于专用隔离根；
- production fixture 与测试 fixture 不共享；
- discovery/pipe/port 不跨 Gate 并行复用；
- P9 Native Editor 与真实 MCP 必须使用同一用户 Session。

## 12. 文档同步边界

### 12.1 已完成的生成/自审阶段

激活前记录曾为：

```text
施工文档：施工文档/待执行/256-当前可自动化施工文档-AI-Project-Bootstrap-Mutation-Access-Handshake-v1.md（历史位置）
状态：已生成并自审，待激活
当前施工文档：无
待执行施工文档：256
明确：不可施工，等待激活前复核与用户授权
```

该阶段已于 2026-07-23 结束；上述文字不是当前入口状态。

### 12.2 激活时

只在复核通过并移动到 `当前/` 后更新：

```text
当前施工文档：256
待执行施工文档：无
施工状态：施工中（fresh P8Q `058d4b2…` terminal failed；下一步需单独授权 scoped gateway_process/Preview descendant ownership remediation；P8F/P8A/P9/P10 未授权）
```

### 12.3 完成时

只有 P0-P10 全绿后，才能标记已完成、生成完成记录并归档。

## 13. 施工文档自审

### 13.1 与 256 正式方案一致性

通过。本文只施工三个目标级 Tool、launcher/project binding、Editor-owned Create/Open、explicit rebind 和 bounded mutation request。真实 conformance 到一次 access decision 即停止。

### 13.2 正式方案是否需要修改

已做两次窄修订。三个 public Tool、owner 与 AI 自由度设计不变；第一次补齐进入 production MCP conformance 前的 binary feature/profile、三 binary seal、sealed real-window smoke、config activation/receipt/reload 合同。P8S1 再把可重复、只读的 Qualification 与单次 production Freeze 分开，避免广泛回归首次在 one-shot Freeze 中发现 flaky。代码复核继续确认正式方案的核心产品缺口真实存在：

```text
application.rs 在 no-project 时 drop Gateway Host
discovery/ClientHello v1 强制 project identity/root digest
Registry 当前 16 Tool
request_goal_mutation_access 已存在但无 typed MCP 入口
```

这些证据深化了施工 Gate，没有推翻方案 B 或扩大公共 Interface。

### 13.3 是否扩大范围

未扩大。预计改动限定在 ProjectAccess contract、Gateway binding/discovery/session、Native Editor Host/UI 和直接 consumer。项目业务、Preview/Build、Provider、Runtime/AUI/Renderer 与历史 lifecycle 均排除。

### 13.4 是否形成工具外菜谱

没有。P2-P5 是施工顺序和单个 Tool 内部正确性，不会进入 AI-facing schema。Tool Result 只返回事实、receipt 和 diagnostics，不返回下一 Tool、固定次数或跨工具状态机。

### 13.5 Module 是否足够深

通过。`AiProjectAccessModule` 用三个目标级 Interface 隐藏 path、Editor Command、binding observation、publication、reconnect、idempotency、receipt 与 authority transition。删除该 Module 后复杂度会重新散落到 MCP/Core/Application，而不是简单消失。

### 13.6 是否建立第二套真相

没有：

```text
Registry 拥有静态 Tool contract
ProjectLauncher/WriteScope 拥有项目创建/打开与路径安全
EditorSession 拥有 active project
Gateway Core 拥有 Session/Grant/request/generation
EditorGatewayHost/Discovery 拥有 publication
Native Editor 拥有 approve/reject
Catalog 只聚合 availability
ProjectAccessModule 只组合，不复制 owner state
```

### 13.7 launcher 协议是否安全

通过。本文明确禁止空 project identity 和原地升级 Session，要求显式 launcher/project union、fresh binding 与旧 lineage fail-closed。

### 13.8 授权边界是否安全

通过。typed request 只到 awaiting_user；request receipt 不是 Grant；真实 user decision 仍由 Native Editor 发出。重复 request、TTL、switch 与 race 都由 Gateway generation 原子决议。

### 13.9 验证层次与成本是否合理

通过。P1-P6 定向、P7 受影响域、P8R3A/B lifecycle 修复与稳定性、P8R3C/D Runner 分流、P8Q exact-commit qualification、P8F production freeze、P8A config activation、P9 交互 conformance、P10 单次 Local CI 分责明确。P8R3A remediation 以 server observation 而不是 sender timing 为真相；P8R3B 在 fresh qualification 前必须已有 repository-owned compiled stress harness 与 Gate 外 wrapper smoke，禁止再用临时 PowerShell。P8Q 负责发现基线问题且无 production 副作用；P8F 才从 sealed binary 证明真实窗口与隔离 config lifecycle；二者都不能冒充 P9 的真实 typed MCP Session。

### 13.10 历史失败是否吸收

通过：

- 255 的 canonical ledger 延迟暴露问题被 P0/P8 前置 producer 对账吸收。
- 255 的磁盘写满问题被 P0/P8/P10 容量与 cleanup margin 吸收。
- 254-R1/R2 把工具一致性、candidate 和 outcome acceptance 耦合的问题没有恢复。
- authoritative failure 不自动重跑；新 commit、fresh preflight 和新授权边界明确。
- 旧 P8 默认 feature binary/两 binary seal/config lineage 漏洞已转成通用 construction rule 与本次 fresh freeze 的硬退出条件。
- P8R3A barrier send/disconnect race 已转成 server-observation 判定合同与三种确定性 owner 场景。
- P8R3B 临时 PowerShell launcher 超时已转成 repository-owned compiled harness、Gate 外 wrapper smoke、shared deadline、逐 child evidence 与 descendant cleanup 硬合同。

### 13.11 外部审查结论

本轮没有新的其它 AI 审查文档。P8S1 根据第六次 P8 的结构化 failure report、10/10 sequential 与 12 并发 9 pass/3 fail 的诊断证据修订了正式方案的施工证据边界；没有修改三个公共 Tool 或产品设计。

### 13.12 是否越权施工

没有。P8R1/P8R2 已按独立授权完成；随后 fresh P8 `198373e…` 只执行一次并在固定矩阵失败，未创建 freeze 或进入 P8A/P9/P10。P8S1 只修改正式方案、当前施工文档和必要入口摘要。P8R3A scoped remediation 只修改 `llm_worker_lifecycle.rs` 与同文件直接 owner 测试。compiled harness preparation 只修改 construction-only `quality_gate`，Gate 外只运行 1-child wrapper smoke。随后单独授权的 fresh P8R3B 只执行一次 12-child stress 与规定 owner/all-features 矩阵并通过；没有进入 P8R3C/P8R3D/P8Q/P8F/P8A/P9/P10。

自审结论：

```text
方案一致性：通过
范围控制：通过
AI 自由度：通过
深 Module：通过
owner 单一真相：通过
launcher/rebind 安全：通过
授权安全：通过
分层验证：通过
环境/容量合同：通过
历史失败吸收：通过
Qualification/Freeze 边界：通过；P8S1 已完成
lifecycle 单点修复：通过；P8R3A 已完成
lifecycle remediation 硬合同：通过；server release 只认 barrier-observed OR disconnect-observed
stress execution 硬合同：通过；fresh P8R3B 前必须完成 compiled harness preparation 与 Gate 外 wrapper smoke
当前状态：fresh P8Q `058d4b2…` executed_once_terminal_failed；下一步需单独授权 scoped gateway_process/Preview descendant ownership remediation
```

## 14. 激活前复核清单

后续只有用户明确授权施工时才执行：

```text
[x] 256 正式方案仍为当前有效方案 B
[x] 240/49/54/施工 README 在激活提交中同步为本文唯一当前施工文档
[x] 施工文档/当前/ 激活前无其它 active 文档
[x] git status clean，exact pre-activation commit/tree 已记录
[x] no-project Host/discovery/ClientHello 缺口仍与本文一致
[x] request_goal_mutation_access/approval UI owner 未漂移
[x] 19 Tool 目标未被其它施工提前实现或改变
[x] P0-P7 既有命令与测试名仍存在
[x] `gate_256_p8` runner 与 P8A activation 已实现；fresh P8 v2 `7a29c87…` 已失败并完整保存 evidence
[x] canonical ledger producer 合同未漂移；当前五类 diff 由 P0 在 exact activation commit 上重新验证
[x] 隔离 worktree/target/temp/fixture/discovery/artifact 路径可用
[x] 各卷容量满足峰值 + 20% 且 10 GiB 余量
[x] old P8 real-window/config freeze 缺口已写入正式方案、施工 Gate 与通用 construction skill
[x] P8R0 文档闭环已完成
[x] P7 已前移 Windows verbatim `TEMP/TMP` 下的 dialog exact test 与 `editor_window_winit --all-features`
[x] 后续施工已拆成不超过三小时的独立授权切片
[x] P8R1 scoped implementation `f0bfc74…`、exact 红绿与 all-features 已通过
[x] P8R2 完整 P7 累计失败集与 verbatim 环境合同已通过
[x] fresh P8 `198373e…` 已执行一次并失败，授权已消费
[x] P8S1 已把一体化 P8 收敛为 P8Q Qualification 与 P8F Freeze
[x] P8Q 不产生 p9Eligible，不构建/封印/启动 production composition，不访问真实 config
[x] P8F 不重复广泛回归，started marker 才消费单次授权
[x] lifecycle remediation/stability 与 Runner 分流均在 P8Q 前独立授权
[x] P8R3A 原单点实现 `e699cc3…` 已完成；scoped remediation `0dd84af…` 与定向红绿已完成
[x] P8R3A remediation 明确覆盖 barrier-first / disconnect-first / neither 三种确定性 owner 场景
[x] send result 只作 diagnostic；server release 只认 barrier-observed OR disconnect-observed
[x] fresh P8R3B 禁止临时 PowerShell/Gate-local script，必须消费已 smoke 的 repository-owned compiled harness
[x] stress harness 固定 12 child、唯一隔离路径、shared deadline、原子 report 与 descendant cleanup
[x] compiled harness `717427d…`、cleanup 修复 `33dd6d6…`、lineage remediation `7e91de5…` 与跨 fresh source root 的 Gate 外 1-child wrapper smoke 已完成
[x] fresh P8R3B 已单次执行并通过：12/12 stress、all-features 与 editor_core owners 全绿，cleanup 完成
[x] `llm_provider_repair_complex_shooter_fake_http_e2e` 旧红灯已归因为跨 worktree target 污染；fresh exact 5/5 与 fresh all-features 全绿，无代码修改
[x] fresh P8Q `058d4b2…` 已执行一次并 terminal failed；当前无 remediation/P8F/P8A/P9/P10 授权
[x] P9 前置明确要求 p9Eligible freeze、真实 activation receipt、reload/new task 和 typed MCP provenance
[x] 用户授权范围不包含历史 R1/R2/F-A/F-B/G/B 通道
[x] 复核通过后本文移入 当前/，54 在同一激活提交中同步
```

激活前复核于 2026-07-23 通过。P0 必须在 pure-doc activation commit 的仓库外 detached worktree 上重新验证动态基线；任一 P0 证据失败即停止，不得进入 P1。
