# 256 当前施工文档：Editor-Instance Gateway Lifecycle + Minimal Project Create v1

## 0. 文档状态

```text
正式方案：256-Editor-Instance-Gateway-Lifecycle-Minimal-Project-Create-v1方案.md
方案选择：方案 B
文档版本：fresh v1 + 2026-07-26 validation-boundary amendment
文档状态：已完成并归档
当前位置：施工文档/已完成/
施工状态：production typed MCP smoke passed；Local CI 12/12 passed
可否施工：否；施工已完成
激活状态：已激活
施工授权：窗口已完成并关闭；当前无 active 测试或源码授权
生成基线 commit：5857d0db3ad36569613bddb8b7f3dacb8fac7f5d
生成基线 tree：738860c40328d8c8fd3fbc457d7e62034aadb4b0
Rust/Cargo：1.96.0
```

本文是新方案 B 的 fresh 施工文档，不是旧
`256-已暂缓施工文档-Minimal-Project-Create-Tool-v1.md` 的续跑。旧 F0、
旧三工具、旧 Gate 编号、旧授权、P8/P9/P10、candidate/freezer 和 construction
Runner 均不得继承。

只有完成激活前复核、将本文移动到 `施工文档/当前/`、同步 `54` 并获得用户明确授权后，
才能执行施工。生成和自审本文不授权源码修改、测试、Gate、外部 worktree 或 Local CI。

## 1. 施工目标

本施工只实现正式方案 B：

1. Native Editor 进程启动时创建稳定、engine-owned `editorInstanceId`。
2. Gateway host/discovery 生命周期绑定 Editor 进程，不再绑定 active project。
3. 一个 production typed MCP 连接在 launcher 与 project 状态间保持有效。
4. `EditorInstanceGatewayModule` 内部持有 `LauncherContext` 或 `ProjectContext`。
5. Catalog 自动随内部 context 更新 availability，不要求 caller reconnect/rebind。
6. context 变化时旧 project-scoped Grant、operation authority 和 read state fail closed。
7. 新增唯一 Tool：

```text
project.create(requestedProjectRoot, projectName)
```

8. `project.create` 复用正式 Native Editor CreateProject owner，目标存在零写入，失败只清理
   invocation 可证明拥有的全新目标。
9. 成功 receipt 返回 Editor owner 产生的 project identity、digest 和 read generation。
10. 通过一次既有 production typed MCP smoke 证明真实 composition。

## 2. 明确不施工

```text
不实现 project.open
不实现 session.request_mutation_access
不公开 session.rebind 或 reconnectRequired
不要求 AI 执行 launcher/discovery/reconnect 菜谱
不新增第二个公共 Gateway lifecycle Interface
不接受 caller-owned project identity/digest/read generation/idempotencyKey
不增加 project mutation Grant 来批准 project.create
不增加模板、renderer、language 或 starter content
不自动串联 inspect、Preview、Build、Export 或 rollback
不建立 Planner、Workflow、nextToolIds 或目标状态机
不建立 256 专用 Runner、candidate、freezer、activation 或长期 Gate
不恢复旧 P8/P8Q/P8F/P8A/P9/P10
不执行 254-R1/R2、F-A/F-B/G、Real Evaluation 或三引擎 B 通道
```

若施工发现必须扩大到以上任一项，立即停止当前 Gate，分类为设计或施工文档问题，先修订
正式方案或本文；不得在代码里先做兼容层。

## 3. 当前代码基线

激活前必须重新核对，不能只依赖本节的生成时快照。

### 3.1 Native Editor host

```text
rust/crates/editor_window_winit/src/application.rs
  NativeEditorApplication 持有 gateway_host
  reconcile_gateway_host() 读取 active_project_session()
  no project 时 gateway_host.take()
  project identity/root digest 变化时销毁并重建 host
```

当前历史测试：

```text
rust/crates/editor_window_winit/src/tests/gateway_reconnect.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
```

它们明确断言 project switch 后旧 discovery 被删除并显式 reconnect。新施工必须用稳定
instance/context transition 合同替代这些断言，不得保留双重 production 语义。

### 3.2 Gateway protocol 与 Core

```text
rust/crates/ai_tool_gateway/src/protocol.rs
  ClientHello 强制 expected_project_identity/root_digest
  ClientSessionBinding 强制 project_identity/digest
  GatewaySessionState 包含 ReconnectRequired

rust/crates/ai_tool_gateway/src/core.rs
  GatewayCore::connect() 首先 current_project_binding(session)?
  ActiveClientSession 直接拥有 project-bound read grant
  GatewayRequest/Reply 绑定 caller 提交的 project_identity
  多个 dispatch/grant/operation 路径重复 current_project_binding()
```

新协议语义不能在 v1 字段上静默改变。所有语义改变的 discovery、hello、binding、status、
request/reply schema 必须升级版本；新 production host 只发布新版本。禁止同时发布两个
production discovery owner。

### 3.3 Discovery 与 Adapter

```text
rust/crates/ai_tool_gateway/src/discovery.rs
  discovery filename 和冲突 owner 绑定 canonical project root digest

rust/crates/ai_tool_gateway/src/editor_host.rs
  EditorGatewayHostBinding = project identity + root digest

rust/crates/ai_tool_gateway/src/remote_adapter.rs
rust/crates/ai_tool_gateway/src/named_pipe.rs
rust/crates/ai_tool_gateway/src/mcp_stdio.rs
  connect 和 typed projection 假设 Session 已有 project status/digest
```

### 3.4 Tool Kernel 与 Catalog

```text
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
  当前没有 TOOL_ID_PROJECT_CREATE
  Registry/decoder/invocation 全部面向 project-bound Tool
  availability_context(None) 返回 launcher 默认 context
  operation journal 与 loaded_project_identity 绑定 active project

rust/crates/editor_core/src/ai_tool_catalog.rs
  255 已提供完整 registered Tool、四态 availability、basis 与 availabilityDigest
```

255 是必须复用的 Catalog 真相。不得新建 launcher Tool registry、第二 availability
枚举或 Adapter-owned readiness。

### 3.5 CreateProject owner

```text
rust/crates/editor_core/src/project_launcher.rs
  ProjectLauncherState::create_project()
  当前使用 fs::create_dir_all(project_root)
  可进入已有目录
  错误为 String
  无 invocation-owned cleanup 结果

rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_ui_model/src/command.rs
  UI CreateProject consumer 与 active project 接管
```

Tool Adapter 不得解析 String 来生成 diagnostic，也不得手写项目骨架。typed failure 和
owned cleanup 必须在 CreateProject owner 内形成，再由 UI/Tool Adapter 做薄映射。

## 4. 预期涉及文件

### 4.1 核心允许范围

```text
rust/crates/ai_tool_gateway/src/protocol.rs
rust/crates/ai_tool_gateway/src/discovery.rs
rust/crates/ai_tool_gateway/src/editor_host.rs
rust/crates/ai_tool_gateway/src/core.rs
rust/crates/ai_tool_gateway/src/remote_adapter.rs
rust/crates/ai_tool_gateway/src/named_pipe.rs
rust/crates/ai_tool_gateway/src/mcp_stdio.rs
rust/crates/ai_tool_gateway/src/lib.rs
rust/crates/ai_tool_gateway/src/bin/ai_engine_gateway_mcp.rs
rust/crates/ai_tool_gateway/src/bin/ai_engine_gateway_cli.rs
rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs
rust/crates/ai_tool_gateway/tests/explicit_reconnect.rs

rust/crates/editor_core/src/ai_capability_tool_kernel.rs
rust/crates/editor_core/src/ai_tool_catalog.rs
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/lib.rs

rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/tests/gateway_reconnect.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/editor_window_winit/src/tests/mod.rs

rust/crates/editor_host/src/**
```

允许在 `ai_tool_gateway/src/` 内新增一个局部源码文件承载
`EditorInstanceGatewayModule` 的 context/transition Implementation，但公共 Interface
不得超过正式方案的 connect/list/call/close 语义。

### 4.2 条件允许范围

只有 red contract 证明直接 consumer 必须迁移时才允许：

```text
rust/crates/project_e2e_gate/src/**
rust/crates/editor_input/src/**
rust/crates/editor_ui_model/src/**
rust/crates/editor_ui_renderer/src/**
rust/Cargo.toml
rust/Cargo.lock
rust/quality/**
```

`Cargo.toml`、`Cargo.lock` 和 `rust/quality/**` 默认不改。需要新依赖、ledger 或
architecture baseline 变化时，当前 Gate 必须停止，先单独审查原因；禁止顺手同步。

### 4.3 禁止范围

```text
legacy/rust/254-r2-lifecycle/**
历史 256 源码或 fixture
samples/complex_shooter_project/**
真实用户项目
Codex MCP production config
施工文档/当前/ 中的历史 evidence artifact
```

## 5. 验证声明与三小时合同

L0-L6 是 2026-07-25 至 2026-07-26 已执行施工的历史编号，不再作为后续修复必须从头重放的
固定菜谱。后续只按当前声明选择最小适用验证：

| 声明 | 适用条件 | 允许验证 | 不自动包含 |
|---|---|---|---|
| Development diagnosis | 定位 C01 首因或修复诊断证据 | 单一 red-capable owner test、必要 instrumentation | affected crates、production smoke、Local CI |
| Integration repair | 修改 owner、fixture、verification harness 或共享 consumer | 修复测试与实际受影响域 | 无关 crate、production composition |
| Release completion | 要求完成 256 并形成 commit-bound 发布证据 | 未失效的 receipt + 当前 release policy 所需矩阵 | 已有有效证据的重复 producer |

规则：

1. 每次执行前核对当前文档、HEAD、工作树和用户授权；文档修订本身不授权测试或源码修改。
2. dirty worktree 可以提供开发反馈，但不能冒充 commit-bound integration/release evidence。
3. 完整 suite 已覆盖的同配置 filtered test 不重复运行；历史失败只在仍属于实际影响面时进入回归。
4. 测试、产品源码、fixture、runner、production binary 和外部配置分别计算身份与失效范围。
5. 非 mutation 测试在首因或证据缺口处理后可在同一连续施工授权窗口内重跑，无需逐次取得用户授权；真实一次性外部操作仍需 fresh eligibility。
6. 连续施工总时长不超过三小时；2:45 后不开始新实现或验证，3:00 强制停止并保存证据与 cleanup。

这些约束只治理施工声明，不进入 public Tool Interface，也不规定 AI 使用引擎的步骤。

## 6. L0：激活基线与红合同

### 6.1 激活基线

激活后先记录：

```text
activation commit/tree
branch
git status
rustc/cargo version
Cargo.lock digest
Windows/PowerShell version
line-ending state
G:、TEMP、target、artifact 所在卷可用字节
```

建立仓库外、fresh exact-commit detached worktree，使用独立：

```text
CARGO_TARGET_DIR
TEMP
TMP
fixture root
discovery root
pipe names
artifact root
```

基线 producer：

```powershell
Set-Location G:\gameEngin\rust
cargo run -p quality_gate --locked -- propose-ledger
```

结构化读取：

```text
rust/target/quality-gate/candidate/lint-debt-ledger-diff.v1.json
```

只接受：

```text
previous_ledger_readable = true
added_diagnostics = []
removed_diagnostics = []
changed_diagnostics = []
new_suppressions = []
expired_entries = []
```

任何 baseline 漂移先失败收敛，不写 feature red test，不自动 apply ledger。

### 6.2 红合同

先增加最小红测试，失败必须精确来自能力缺失：

```text
editor instance identity 在 launcher 存在且进程内稳定
discovery path/pipe 不因 project context 改变
multiple Editor instance ambiguity fail closed
ClientHello 绑定 expectedEditorInstanceId，不接受 caller project binding
SessionBinding/Status 的 projectContext 为 optional
同一 connection 从 LauncherContext 转到 ProjectContext
Catalog launcher/project availability transition
context 变化失效旧 project-scoped Grant/operation authority
project.create 精确两字段与 unknown/internal field 拒绝
```

建议定向命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p ai_tool_gateway --lib --locked editor_instance
cargo test -p ai_tool_gateway --lib --locked gateway_context
cargo test -p editor_window_winit --lib --locked gateway_instance
cargo test -p editor_core --lib --locked project_create_contract
```

红测试不得通过恢复旧 `GatewayEndpointBinding::{Launcher, Project}`、显式 reconnect 或旧
bootstrap receipt。

### 6.3 L0 退出

输出红测试清单、每个预期失败点和非预期失败。只有 baseline 绿色、红测试因新能力缺失
而红，才进入 L1。

## 7. L1：稳定 Editor instance host/discovery

### 7.1 实现

1. Native Editor application 启动时创建一次 `editorInstanceId`。
2. `EditorGatewayHostBinding` 改为稳定 instance identity，不含 project root identity。
3. host 在 launcher/no-project 状态启动，在 Editor application drop 时停止。
4. discovery filename 与 pipe locator 绑定 instance identity，project switch 不变。
5. discovery resolver 支持 exact path 或 exact `editorInstanceId`。
6. 未提供精确 selector 且存在多个 live instance 时返回
   `gateway.discovery.ambiguous_editor_instance`。
7. stale PID、oversize、非 regular file、atomic publish/remove 等现有安全合同保留。

### 7.2 协议版本

至少升级：

```text
Gateway discovery schema
Gateway protocol version
ClientHello schema
SessionBinding/Status schema
Gateway request/reply schema
```

新 schema 必须 strict/deny unknown fields。旧 v1 production discovery 不与 v2 并存；
遇到 v1 返回 typed unsupported diagnostic，不通过兼容 shim 猜项目。

### 7.3 定向验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p ai_tool_gateway --lib --locked discovery
cargo test -p ai_tool_gateway --lib --locked editor_gateway_host
cargo test -p editor_window_winit --lib --locked gateway_host
```

负向矩阵：

```text
launcher publication exists
project create/open/switch 后 publication path 与 pipe 不变
two live instances + no selector => ambiguity
exact editorInstanceId => exact one
stale process record ignored/removed only under existing safe rules
Editor drop => publication removed and pipe stops
```

### 7.4 L1 退出

L1 commit 只包含 instance host/discovery/protocol red-green 所需修改。若实现需要公开
launcher/project endpoint variant，L1 失败并回到正式方案，不进入 L2。

## 8. L2：内部 context 与稳定 Session

### 8.1 context model

`EditorInstanceGatewayModule` 内部只允许：

```text
LauncherContext { editorInstanceId }

ProjectContext {
  editorInstanceId
  projectIdentity
  canonicalProjectRootDigest
  projectDigest
  readGeneration
}
```

context 由 `EditorSession` owner 读取，不由 Adapter/caller 构造。

### 8.2 Session wire contract

新 `ClientHello`：

```text
expectedEditorInstanceId
client kind/version
supported schema versions
requested read scope
```

禁止 expected project identity/root digest。

新 Session binding/status：

```text
editorInstanceId
clientSessionId
projectContext: null | engine-owned snapshot
catalog/availability facts
expiry
```

新 request/reply envelope 只绑定 session/request，不要求 caller 重复 project identity。
project Tool 的 `expectedProjectDigest` 若属于 Tool invocation 内部并发控制，继续保留；
Module 在 execute 时注入/核对当前 ProjectContext，不能由 Adapter从陈旧 status 猜测。

### 8.3 context reconcile

在 connect、dispatch、owner-thread pump 和 Catalog/status 读取前使用同一个内部 reconcile：

```text
None -> LauncherContext
LauncherContext -> ProjectContext
ProjectContext(A) -> ProjectContext(B or newer digest)
ProjectContext -> LauncherContext
```

转换不得关闭 transport、改变 pipe 或要求第二个 ClientHello。

### 8.4 定向验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p ai_tool_gateway --lib --locked gateway_context
cargo test -p ai_tool_gateway --test explicit_reconnect --locked
cargo test -p editor_window_winit --lib --locked gateway_context
```

`explicit_reconnect` 历史文件允许重写为“旧显式 reconnect 不再需要且不继承 authority”的
回归；不得继续断言 endpoint 因项目变化而替换。

## 9. L3：Catalog 与 authority invalidation

### 9.1 Catalog

复用 255：

```text
LauncherContext:
  project.create = ready（Implementation 已装配时）
  project read/mutation Tool = blocked

ProjectContext:
  project.create = blocked
  其它 Tool 按现有 owner facts 判定
```

`tools/list` 保持完整 registered Tool surface，`catalogDigest` 只随静态 contract，
`availabilityDigest` 随 context/basis 改变。

### 9.2 authority

1. LauncherContext 不创建 project read grant。
2. 进入 ProjectContext 时创建/刷新该 Session 的 engine-owned read authority。
3. project identity 变化时撤销旧 mutation grant、pending approval、operation grant snapshot。
4. 旧 operation 只能按其 terminal receipt 观察；不得继续对新项目执行或取消。
5. project digest 变化保留既有 read stale/refresh 合同。
6. ProjectContext -> LauncherContext 时所有 project-scoped authority fail closed。
7. 多个 client Session 都从同一 Module context reconcile，不各自维护项目真相。

L3 不得注册或实现 `project.create`。LauncherContext 只能将尚未装配的
`project.create` 表达为非 ready；L4 才能改变其 Implementation 装配事实。

operation authority 失效不能通过删除 Gateway observation snapshot 冒充。queued operation
在 context identity 变化后必须 fail closed，不能写入或取消新项目；其 terminal
outcome/receipt 仍须经既有 operation owner 可观察。若现有 owner 无法同时满足
“不可继续/取消”和“terminal 可观察”，L3 失败并停止，不得扩大到 L4。

### 9.3 定向验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core --lib --locked catalog
cargo test -p ai_tool_gateway --lib --locked gateway_catalog
cargo test -p ai_tool_gateway --lib --locked context
cargo test -p ai_tool_gateway --lib --locked grant
cargo test -p ai_tool_gateway --lib --locked operation
cargo test -p ai_tool_gateway --lib --locked mcp_projection
```

高风险断言：

```text
availabilityDigest 变化但 catalogDigest 不变
launcher 不泄漏 project identity/read/mutation authority
old grant 不能用于新 project
old queued operation 不能落到新 project
old operation 不能在新 context 取消，但 terminal outcome/receipt 仍可观察
stale Catalog 不能绕过 execute-time guard
MCP Adapter 不组合 readiness、不缓存第二份 context
多个 Session 读取同一 owner context truth
```

### 9.4 L3-only 窗口退出

形成单一 L3 代码 commit，记录 cumulative failure set，回填 L3 结果并停止。任何
session/reconnect 兼容方案需要 caller 执行额外步骤即失败；无论完成或失败均不得进入 L4。

## 10. L4：Minimal project.create 与 owner 加固

### 10.1 CreateProject owner

在 `ProjectLauncherState` owner 内形成 typed 内部结果：

```text
requested root validation
parent exists and usable
target absent
exclusive target claim
default skeleton creation
Editor project open/session adoption
owner-produced project facts
owned cleanup outcome
```

目标已存在（包括空目录）必须零写入。失败 cleanup 只允许删除 invocation 排他创建且仍可
证明未被外部接管的目标；无法证明时保留现场并返回 `cleanup_failed`。

现有 UI CreateProject Interface 可薄映射 typed error 为用户文本，但不得保留一个更宽松、
可覆盖已有目录的第二 owner。

### 10.2 Tool contract

Registry 只新增：

```text
toolId: project.create
input:
  requestedProjectRoot
  projectName
additionalProperties: false
```

caller-owned internal 字段全部拒绝。`project.create` 不需要 project mutation Grant；宿主的
通用重要操作确认在引擎 Tool 合同之外。

成功结果至少包含：

```text
status = created
receiptId
requestedProjectRoot
canonicalProjectRoot
projectName
projectIdentity
projectDigest
readGeneration
openedInEditor = true
replayed
```

### 10.3 operation/replay

复用现有 request/invocation/operation identity：

```text
同一 invocation + 同一 canonical input => 返回原 receipt，不再次创建
同一 invocation + 不同 input => replay conflict
新 invocation + 已存在 target => target_exists
```

launcher 阶段 replay state 可由当前 Editor-instance Session 内存 owner 持有；成功进入项目
后按现有 journal owner 持久化可审计 receipt。不得新建 caller idempotency Module。

### 10.4 MCP projection

`mcp_stdio` 必须直接从 Registry schema 投影 `project.create`。它不得先读取 project
status 来填 expected project digest，也不得提交 editor instance/project internal 字段。

### 10.5 定向与负向测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core --lib --locked project_launcher
cargo test -p editor_core --lib --locked project_create
cargo test -p ai_tool_gateway --lib --locked project_create
cargo test -p ai_tool_gateway --lib --locked invocation_replay
cargo test -p ai_tool_gateway --lib --locked mcp_projection
cargo test -p editor_window_winit --lib --locked create_project
```

必须覆盖：

```text
exact two-field schema
unknown/internal field rejection
relative/root/env-expanded/invalid name rejection
missing parent
target file/existing empty dir/existing project zero-write
exclusive-create contention exactly one owner
initialize failure owned cleanup
open/context transition failure不虚报 created
cleanup ownership uncertain
success identity/digest/read generation lineage
same invocation replay
new invocation target_exists
project-open context project.create blocked before side effect
```

### 10.6 L4 退出

L4 不运行完整 workspace 或 production smoke。完成后形成 commit，记录所有新 diagnostic
code、receipt schema 和累计失败集。

## 11. Integration：受影响域与身份边界

### 11.1 历史 L5 定位

第 25、26、28 节保留 L5 的真实运行历史，不改写其当时使用的规则和证据。自
2026-07-26 公共测试纪律删减后，后续修复不再机械重跑“累计定向 + 全部 affected crate +
fresh L5”。验证由实际修改和声明决定。

### 11.2 当前身份

```text
256 production source：
  editor_core / ai_tool_gateway / editor_host / editor_window_winit 中实现 project.create
  与 Editor-instance Gateway lifecycle 的 production 源码及 Cargo production dependency

verification harness：
  ai_tool_gateway tests、project_e2e_gate tests、quality_gate runner/report

fixture/generated contract：
  C01 input、candidate/evidence 临时根、lint/architecture/suppression truth

production composition：
  ai_engine_gateway_mcp.exe + default profile/features + Native Editor/discovery/pipe

external configuration：
  本施工没有修改真实 Codex MCP config
```

某类输入变化只失效消费该输入的证据。只有 production source、production dependency、
profile/features 或 launch composition 变化时，才失效第 29.1 节已经通过的 production
typed MCP smoke。只修改 `project_e2e_gate`、`quality_gate` 或其 test-owned fixture 时，
不得机械重跑无变化的 Editor/Gateway owner suites 或 production smoke。

### 11.3 Development 与 affected domain

- 诊断先运行能复现当前首因的单一 C01 owner test，并保留完整 assertion/stdout/stderr。
- 修复后运行该 reproduction；只有共享 contract 或直接 consumer 变化时才扩展受影响域。
- 如果运行完整 crate suite，同配置下被它覆盖的 filtered test 不再单独重复。
- `TEMP/TMP`、fixture 和 artifact 仅在测试实际写入这些状态时隔离；路径必须 canonical 且
  role-owned。
- lint ledger、suppression、architecture 或生成 contract 只有被修改或作为 release policy
  输入时才重新 producer/validate；不得自动更新 ledger。

### 11.4 commit-bound integration

需要声明“某个 commit 的 integration 已通过”时，才建立 clean exact-commit source。该
qualification 运行修复后的 reproduction、实际受影响 consumer 和被修改的 repository-owned
producer。它不重复未失效的 production composition receipt，也不要求固定磁盘比例；长任务
只需证明容量覆盖预计峰值、证据和 cleanup。

### 11.5 失败

首个失败 artifact 必须包含具体 assertion 或 diagnostic。若报告只剩 test name/exit code，
先修复证据接口或直接 owner reproduction，不得凭摘要改实现，也不得启动 Local CI 调试。

## 12. Release completion：production receipt 与 Local CI

### 12.1 已有 production receipt

第 29.1 节已经证明以下 production composition：

```text
source commit：a817f706e0260953261b1f72a83b96de454089eb
binary sha256：de8f8b2927416cc0669c150b505c8ad44f6a1fb22805ebaacfbe23b9870184a9
platform/profile/features：Windows / Cargo built-in test / default
typed project.create production smoke：passed
binary reopen/unchanged：passed
```

该 receipt 继续有效，除非第 11.2 节定义的 production source、dependency、binary 或 launch
composition 发生变化。C01/quality runner/test-only 修复形成新 commit 时，必须记录 production
输入未变和 receipt reuse，但不得仅因 Git commit 不同重复执行 smoke。

不修改真实 Codex config，不声称当前 Codex task 热加载 Tool。

### 12.2 Local CI 资格

Local CI 只在以下条件同时成立后激活一个连续施工授权窗口：

```text
C01 首因已由完整证据定位
修复后的 red-capable reproduction 通过
实际受影响域通过
用于 release claim 的 source commit clean 且精确
Local CI runner 能保留第一个完整失败 assertion/stdout/stderr
高成本 stage 在上游失败后 fail-fast，除非后续 stage 明确收集不同证据
容量和三小时剩余时间覆盖 runner、artifact 与 cleanup
```

repository-owned 命令仍为：

```powershell
Set-Location G:\gameEngin\rust
cargo run -p quality_gate --locked -- local-ci --commit HEAD
```

当前 245 release policy 要求的矩阵在其正式方案修订前仍由 canonical runner 决定；256 文档
不另造第二矩阵。后续应单独审查全 workspace `all-features` 是否能收敛为 feature owner 与
consumer closure，但该跨系统优化不是 C01 remediation 的前置实现。

### 12.3 Local CI 失败与重试

Local CI 是 non-mutating qualification，不是一次性真实外部验收。失败后禁止立即用它调试；
首因或证据缺口在 Development/Integration 层处理后，可以在同一连续施工授权窗口内为新的
exact commit 和新 run root 再次运行，无需逐次取得用户授权。该窗口包含实现、测试、等待、
证据和 cleanup，总时长不得超过三小时；agent 停止或向用户交付结果后窗口结束。只更新
verification harness 时不失效第 12.1 节 production receipt。

### 12.4 完成闭环

只有有效 production receipt 与当前 release policy 的 Local CI 都通过：

1. 生成 `阶段完成记录/2026-XX-XX-Editor-Instance-Gateway-Lifecycle-Minimal-Project-Create-v1/00-总览.md`。
2. 记录每个授权窗口的起止时间、commit/tree、测试计数、失败、重跑原因、artifact digest、
   process cleanup 和三小时状态。
3. 同步正式方案、根 README、`00-文档地图`、`00-阅读顺序`、`240`、`49`、`54`、
   `施工文档/README.md`、`阶段完成记录/README.md`。
4. 将本文从 `当前/` 移到 `已完成/`。
5. 不自动开始下一系统，不执行任何历史 254/256 Gate。

## 13. 验证声明摘要

| 声明 | 目的 | 进入条件 | 失败动作 |
|---|---|---|---|
| Development | 快速、可定位地复现 owner failure | 当前测试授权 | 保存完整首因；不扩展到 Local CI |
| Integration | 证明实际修改及 consumer | reproduction 绿色、影响面明确 | 只修复失败 owner/consumer |
| Release completion | 证明可复用 production receipt 与 canonical Local CI | 未失效 receipt、clean exact commit、单独授权 | 返回 Development/Integration；按输入失效证据 |

不同环境或 composition 必须有独立证据；相同输入和环境下，上层 suite 已覆盖的测试不得
机械重复。

## 14. 高风险验证清单

施工时逐项更新，不得在生成阶段预先标记通过：

```text
[ ] editorInstanceId engine-owned、进程内稳定、进程间不复用。
[ ] launcher/no-project 有 production host/discovery。
[ ] project create/open/switch 不改变 discovery path 或 pipe。
[ ] multiple instance ambiguity fail closed；exact selector 正确。
[ ] v2 wire schema strict，v1 不被静默误解。
[ ] ClientHello/request 不接受 caller project binding。
[ ] SessionBinding/Status projectContext optional。
[ ] 同一 connection 跨 LauncherContext/ProjectContext。
[ ] launcher 无 project read/mutation authority。
[ ] context identity 变化撤销旧 grant/approval/operation authority。
[ ] project digest drift 保留既有 stale/refresh 保护。
[ ] registered Tool surface 稳定；catalogDigest/availabilityDigest 分工正确。
[ ] project.create 只有两个 direct input 字段。
[ ] target exists 所有形态均零写入。
[ ] exclusive create 竞争不产生双 owner。
[ ] failure cleanup 只删除 invocation-owned target。
[ ] cleanup uncertain 返回 cleanup_failed 并保留精确路径。
[ ] Tool Adapter 不手写项目骨架、不解析错误字符串。
[ ] project.create 不消费 project mutation Grant。
[ ] replay 不重复创建，changed input conflict。
[ ] result/receipt/operation/project facts lineage 一致。
[ ] MCP Adapter 不读取陈旧 status 填充 caller internal field。
[ ] production MCP/pipe/session 在 create 前后未重启。
[ ] 一个既有 project read Tool 在同一连接成功。
[ ] child process、pipe、discovery、fixture、TEMP、target cleanup 完整。
[ ] 无旧三工具、rebind/reconnect、P8/P9/P10、Runner/candidate/freezer 恢复。
[ ] canonical ledger/suppression 无漂移。
[x] production smoke receipt 已绑定 binary/composition 并通过。
[ ] C01 owner failure 有完整 assertion/stdout/stderr，可由快速 reproduction 定位。
[ ] 修复后的实际受影响域全绿；没有机械重跑无关 suite。
[ ] Local CI exact commit、当前 release policy 矩阵和 cleanup 全绿。
```

## 15. 失败分类与收敛

| 首因 | 分类 | 动作 |
|---|---|---|
| 正式方案需要 public reconnect/rebind | 设计问题 | 停止并修订正式方案 |
| Gate/命令/fixture 与真实代码不符 | 施工文档问题 | 修订本文并重新自审 |
| red contract 失败点正确 | 预期缺口 | 在当前 Gate 最小实现 |
| 定向测试新增同类第二失败 | Implementation 共性 | 全仓搜索 consumer，收敛 shared Interface |
| baseline ledger/clean/toolchain 漂移 | 基线/环境 | feature 施工前失败收敛 |
| production smoke 暴露 composition 缺口 | Implementation 或 qualification 缺口 | 保存证据；仅在 composition 修复并重新具备资格后运行 |
| Local CI 只保留 test name/exit code | evidence Interface 缺口 | 先做 owner reproduction 或修 runner evidence，不猜实现 |
| Local CI 发现实现/fixture 缺口 | Integration failure | 修 reproduction 与实际受影响域；按输入失效证据 |
| 达到三小时 | time limit | cleanup + stop report，不把它记为 Gate failure |

禁止：

```text
凭摘要猜修复
修改 expected output 迎合错误实现
用 dirty worktree 定向通过冒充 commit-bound integration/release evidence
用测试 JSON-RPC client 冒充 production typed MCP
用 Local CI 逐错调试
因 test-only 变化机械重跑未失效的 production smoke
用 legacy 代码机械恢复速度换取通过
```

## 16. 入口与归档规则

### 16.1 当前激活状态

```text
正式方案：已确认
施工文档：已激活
当前位置：当前/
可施工：否；L6 production smoke、C01 remediation 与 Local CI runner remediation passed
当前施工：绑定 e99b5af 的 fresh fail-fast Local CI 连续施工窗口已激活
授权：窗口内允许多次 Local CI；失败后先收敛 owner 首因；三小时硬停止
```

### 16.2 激活

仅在用户明确授权后：

1. 重新读正式方案、入口、本文和当前代码。
2. 核对所有路径、test filter、protocol symbols 和无冲突施工。
3. 修订漂移并重新自审。
4. 将本文移动到 `当前/`。
5. 同步 `54` 为唯一施工入口。
6. 形成纯文档激活 commit。
7. 用户授权哪个窗口，只执行哪个窗口。

### 16.3 完成

只有有效 production receipt、当前 release policy 的 Local CI 和必要 cleanup 全部通过才写
完成记录和归档。历史 L0-L6 不因文档改名重跑。

## 17. 施工文档自审

### 17.1 与正式方案一致

通过。施工围绕一个 `EditorInstanceGatewayModule`、内部 optional project context 和唯一
`project.create`；没有增加第二公共 lifecycle Interface。

### 17.2 正式方案是否需要再修改

无需修改。代码复核与旧 F0 再次确认正式方案描述的根因准确：host、discovery、hello、
binding 和 operation 当前确实 project-bound；CreateProject owner 确实可局部加固。

### 17.3 是否恢复旧 256

没有。本文明确禁止 legacy source、旧三工具、endpoint variant、显式 reconnect、
bootstrap receipt、P8/P9/P10、Runner/candidate/freezer。旧 F0 只提供证据。

### 17.4 是否形成 AI 菜谱

没有。L0-L6 是代码施工顺序，不进入 Tool Catalog 或 public Interface。产品侧 AI 只需
通用 connect/list/call/close；context transition 在 Tool Implementation 内完成。

### 17.5 是否限制 AI

没有。AI 自由决定是否创建、路径、名称和创建后的 Tool 顺序。限制只保护精确 Editor
实例、项目 authority 和已有目标。

### 17.6 Module 是否足够深

通过。外部 Interface 没有增加 lifecycle 方法，Implementation 隐藏 discovery、host、
context reconcile、Catalog basis、authority invalidation 和 project guard。删除该 Module
会使这些复杂度重新散落到 Native Editor、Gateway Core、Adapter 和每个 Tool。

### 17.7 ownership 是否清晰

通过：

```text
Native Editor host -> editorInstanceId/publication
EditorInstanceGatewayModule -> connection/context transition
Tool Registry -> static contract
255 probe/Catalog -> availability
CreateProject owner -> skeleton/no-overwrite/cleanup
Tool Kernel -> invocation/operation/receipt
Gateway -> Session authority
MCP Adapter -> transport projection only
```

### 17.8 schema 与迁移是否安全

通过。wire 语义变化要求新版本，不原地把 required project 字段变 optional；production
只保留一个 discovery owner，旧 v1 fail closed。

### 17.9 测试是否过多

原判断已被 L6 事实推翻并完成修订。旧合同把 filtered tests、完整 crate、fresh worktree、
production smoke 和全 workspace Local CI 机械串联，还把任意 test-only commit 变化等同于
production receipt 失效。第 5、11、12、13、15 节现已改为按声明选择、suite 覆盖去重、
身份分离和 fail-fast；不降低真实 composition 或最终 release claim。

### 17.10 三小时规则

通过。保留三小时绝对上限和 2:45 cleanup 边界，但不再把短施工人为拆成固定 Gate。
长 stage 只在预计执行、证据与 cleanup 能装入剩余时间时启动。

### 17.11 环境等价合同

通过并收敛。clean/exact commit 只用于 commit-bound integration/release claim；隔离
target/TEMP/discovery/fixture 只在实际存在竞争或副作用时要求。production composition 和
外部配置仍按其真实身份验证，不向普通源码施工扩散。

### 17.12 用户数据安全

通过。所有创建目标 test-owned；目标存在零写入；cleanup 只处理 invocation-owned
目标；无法证明 ownership 时 fail closed。

### 17.13 外部审查

本轮没有新的其它 AI 审查文档。正式方案已吸收 Unity/Godot/Unreal 的项目管理身份与
创建 owner 分工；施工无需扩展外部引擎研究。

### 17.14 当前是否越权

没有。用户本轮只授权公共纪律与当前施工文档修订；未授权 C01 test、源码、Local CI 或
production smoke。历史窗口 A-L6 的运行事实保持不变。

自审结论：

```text
方案一致性：通过
范围控制：通过
深 Module：通过
AI 自由度：通过
authority/数据安全：通过
schema migration：通过
验证按声明选择与去重：通过
证据身份与失效边界：通过
三小时边界：通过
环境合同按实际风险应用：通过
入口状态：production smoke、C01 remediation 与 runner remediation passed；e99b5af fresh Local CI 连续施工窗口已激活
```

## 18. 激活前复核清单

2026-07-25 激活前复核结果：

```text
[x] 用户明确授权激活并开始施工。
[x] 正式方案仍是当前真相。
[x] 当前/ 没有其它 active 施工文档；仅有历史 evidence artifact。
[x] 本文激活前位于待执行/ 且未被替代。
[x] HEAD `a1b40ec2c862fa99442b7ef254a91050b70fa0a3`、tree
    `ca5483da4edecaf5163621b4eb82155c676511df`，worktree clean。
[x] protocol/discovery/Core/CreateProject symbols 仍存在。
[x] Cargo package 与目标 test owner 仍存在；新 filter 将由 L0 red contract 创建。
[x] Cargo.lock、lint ledger、architecture baseline 无未提交漂移；canonical producer 在 L0
    真实运行后才判定 baseline。
[x] 外部 worktree/target/temp/artifact 使用独占新路径；容量在 L0 启动前实测。
[x] 没有恢复旧 256、254-R1/R2 或三引擎 B 授权。
[x] 本次授权窗口为 A（L0-L1），不含 L2-L6。
[x] 复核结果已写回本文并重新自审。
```

## 19. 窗口 A（L0-L1）施工记录

2026-07-25 窗口 A 已在三小时上限内完成，未进入 L2-L6。

```text
窗口开始：2026-07-25T10:30:11.6048926+08:00
窗口结束：2026-07-25T10:59:59.3317381+08:00
激活提交：42c874d63cbb3eb434fa0e2387c6c35255f34cb7
L1 代码提交：1dafd218939066b486fbce1ae1aa2026871b5087
L1 tree：a1a91c6944429033888fa923207c031605e62768
隔离根：G:\EngineTest\256-gateway\window-a-42c874d
```

### 19.1 L0

canonical ledger 激活基线与施工后复核均为：

```text
previous_ledger_readable=true
added_diagnostics=[]
removed_diagnostics=[]
changed_diagnostics=[]
new_suppressions=[]
expired_entries=[]
entries=167
warning_occurrences=369
suppressions=11
```

红合同首先因旧 discovery/host 仍要求 project identity/root、没有
`editorInstanceId` 而失败，失败原因精确属于新能力缺失。未发现环境或依赖漂移。

### 19.2 L1 结果

已完成：

```text
Native Editor application 启动时生成稳定、engine-owned editorInstanceId
launcher/no-project 状态启动 Gateway host/discovery
host binding、discovery filename 与 pipe locator 绑定 editorInstanceId
project open/switch 不更换 discovery path 或 pipe
exact path / exact editorInstanceId resolver
多 live Editor 无 selector 时 typed ambiguity
Editor drop 删除 publication 并停止 pipe
discovery、ClientHello、SessionBinding/Status、request/reply 与 pipe wire 升级到 v2
旧 discovery v1 返回 gateway.discovery.schema_unsupported
CLI/MCP selector 改为 AI_ENGINE_GATEWAY_EDITOR_INSTANCE_ID
```

验证结果：

```text
cargo test -p ai_tool_gateway --lib --locked discovery
  8 passed
cargo test -p ai_tool_gateway --lib --locked editor_gateway_host
  2 passed
cargo test -p editor_window_winit --lib --locked gateway_host
  1 passed
cargo test -p ai_tool_gateway --lib --locked
  55 passed
cargo check -p ai_tool_gateway --all-targets --locked
  passed
canonical propose-ledger
  passed；六项零漂移
```

额外运行 `cargo test -p editor_window_winit --lib --locked` 得到 `118 passed /
1 failed`。唯一失败为历史
`gateway_explicit_reconnect_real_pipe_retires_old_adapter_without_grant_inheritance`：
它仍断言 project switch 删除旧 endpoint 并要求显式 reconnect。本文第 8.4 节已明确
该旧合同在 L2 重写为稳定 connection/context transition，因此这是预期的 L2 红项，
不是 L1 回归；窗口 A 不越权修改。

### 19.3 停止边界

```text
窗口 A：完成
L2：完成并停止
L3-L6：未授权、未施工
旧 256 / P8-P10 / Runner / candidate / freezer：未执行
254-R1/R2、F-A/F-B、Gate G、三引擎 B 通道：未执行
当时下一步：等待用户单独复核与授权 L3；不得自动继续（现已由第 21 节独立授权取代）
```

## 20. L2-only 授权窗口

```text
授权时间：2026-07-25
窗口开始：2026-07-25T11:44:11.1429890+08:00
授权基线 commit：e64ca7d8cc4c7d22e08281feefda13552b82f664
授权基线 tree：90db857c363ffa6c20a07d12ef86c11845e89da3
范围：
  1. 退役 project switch 删除 endpoint / 强制 reconnect 的旧测试菜谱。
  2. 保留并重写旧项目 authority 不得继承的合同。
  3. 保持 L2 施工前 baseline 可解释，不接受 session_unknown 作为最终行为。
  4. 建立 L2 红测试并实现 stable session/context reconcile 到绿色。
禁止：L3-L6、旧 256 Gate、254-R1/R2、F-A/F-B/G、三引擎 B 通道。
三小时硬停止：2026-07-25T14:44:11.1429890+08:00
```

### 20.1 L2 执行结果

```text
执行状态：completed / stopped
完成时间：2026-07-25T12:33:53.7367342+08:00
耗时：约 50 分钟
代码 commit：d0a7dd7eb93117e85a7e59f1578270d82a26d868
代码 tree：2c2c946533c70dc67a3e3474b581ce9cef536e28
L3-L6：未授权、未执行
旧 256 Gate、254-R1/R2、F-A/F-B/G、三引擎 B 通道：未执行
```

完成事实：

1. `ClientHello` 只绑定稳定 Editor instance；Session binding/status 使用可选、engine-owned
   `ProjectContext`，request/reply 不再要求 caller 重复 project identity。
2. launcher 可建立同一 Session；launcher -> project、project A -> project B、
   project -> launcher 均由 Core reconcile，transport、pipe、discovery 和
   `clientSessionId` 保持。
3. project identity 转换会撤销旧 mutation grant、pending approval 与 operation
   ownership；同项目 digest 漂移继续保持既有 `stale -> inspect refresh` 合同。
4. 旧 project switch 删除 endpoint、杀掉 MCP process、启动第二 Adapter 的测试菜谱已退役；
   real pipe 与 MCP process 测试均证明同一 Adapter/Session 可读取新项目状态且旧 authority
   不继承。

验证结果：

```text
cargo test -p ai_tool_gateway --lib --locked
  56 passed
cargo test -p ai_tool_gateway --test explicit_reconnect --locked
  1 passed
cargo test -p ai_tool_gateway --test mcp_process_smoke --locked
  2 passed
cargo test -p editor_window_winit --lib --locked
  119 passed
cargo check -p ai_tool_gateway --all-targets --locked
  passed
cargo check -p project_e2e_gate --all-targets --locked
  passed
cargo check -p editor_window_winit --all-targets --all-features --locked
  passed
cargo check -p editor_host --all-targets --locked
  passed
canonical propose-ledger
  previous_ledger_readable=true
  added/removed/changed diagnostics=0
  new_suppressions=0
  expired_entries=0
  entries=167, warning_occurrences=369, suppressions=11
```

累计失败与收敛：

- Editor consumer 与 MCP process smoke 残留顶层 project binding 字段，统一迁移到
  `project_context`。
- 旧 digest 自动刷新实现曾越过第 9.2 节边界，已纠正为同项目 digest 继续 stale/inspect
  refresh；只对 identity/context 转换执行稳定 reconcile。
- 首次 ledger producer 因 `editor_host` 测试残留旧字段编译失败；修复后新增的唯一
  dead-code diagnostic 来自已退役 reconnect helper，删除 helper 后六项零漂移。

## 21. L3-only 授权与激活前复核

```text
授权范围：仅 L3 Catalog 与 authority invalidation
窗口开始：2026-07-25T12:51:44.3410783+08:00
三小时硬停止：2026-07-25T15:51:44.3410783+08:00
保守预算：2 小时 15 分
授权基线 commit：cbd7e4ccebb9b815fc22c4142775dc785dcfe8f4
授权基线 tree：854e3726e815dcc24448e4472f209a1e481a0571
禁止：L4-L6、project.create 实现/注册、旧 256 Gate、254-R1/R2、
      F-A/F-B/G、Real Evaluation、三引擎 B 通道
完成或失败后：回填 L3 证据并立即停止
```

激活前自审：

```text
[x] 当前/ 只有一份非 README 施工文档。
[x] HEAD/tree 与 L2 完成入口一致，主工作树 clean。
[x] L3 只消费 L2 已建立的 stable session/context，不重做 L2。
[x] L3 只修改 Catalog availability 与 authority invalidation，不进入 project.create。
[x] static catalogDigest 与 dynamic availabilityDigest 分工保持 255 合同。
[x] execute-time guard 仍读取 owner truth，stale Catalog 不能授权执行。
[x] operation 失效要求 fail closed 且保留 terminal observation，不以删除 snapshot 代替。
[x] 定向 filter 已改为现有 context/grant/operation/catalog 族，不依赖不存在的
    project_switch 名称。
[x] L3 形成单一代码 commit；L4-L6 需要新授权。
[x] 2:45 后只允许 cleanup/stop report；3:00 无条件停止。
```

自审结论：L3-only 范围闭合、可在三小时内执行；未形成 AI 工具外部菜谱，未扩大公共
Interface，未授权任何后续 Gate。

## 22. L3-only 施工结果

```text
执行状态：completed / stopped
窗口开始：2026-07-25T12:51:44.3410783+08:00
完成时间：2026-07-25T13:27:56+08:00
文档授权 commit：c6450e0e510deafa2b1f45db72643b26d8057913
文档授权 tree：90103a24930552aafaa2a8c8df16a92c0458ca3b
L3 代码 commit：a3068f286bc8dd5aa932d8dcc327654c17e6685a
L3 代码 tree：528707d1632e76285d7bcbfd96524e085ddcaa24
L3 完成时历史状态：L4-L6 未授权、未执行；L4 后续结果见第 24 节
```

完成事实：

1. launcher/project Catalog 保持相同 `catalogDigest`，`availabilityDigest` 随 engine-owned
   context/basis 变化；project read Tool 在 launcher blocked、进入项目后 ready。
2. 多个 Gateway Session 从同一 Editor owner context 获取相同 availability truth，不缓存
   第二份 Adapter context。
3. project identity 变化继续撤销 mutation grant 与 pending approval；旧 operation authority
   变为 observe-only，不能执行或取消到新项目。
4. Tool Kernel owner 将 context 切换时未完成的旧 operation 收敛为 `Interrupted` terminal；
   Gateway 缓存 owner snapshot，使多个 Session 在新项目 hydrate 后仍可观察 terminal outcome。
5. detached old operation 不进入新项目 rollback lineage，不用旧 terminal digest 刷新新项目
   read state；stale Catalog 仍由 execute-time owner guard 阻挡。
6. 未新增、注册或实现 `project.create`，未进入 L4。

验证结果：

```text
L3 定向矩阵：
  editor_core catalog：10 passed
  Gateway gateway_catalog：4 passed
  Gateway context：6 passed
  Gateway grant：3 passed
  Gateway operation：6 passed
  Gateway mcp_projection：3 passed

累计/受影响 owner：
  editor_core --lib：549 passed / 2 ignored
  ai_tool_gateway --lib：58 passed
  explicit_reconnect：1 passed
  mcp_process_smoke：2 passed

canonical ledger：
  previous_ledger_readable=true
  added/removed/changed diagnostics=0
  new_suppressions=0
  expired_entries=0
  entries=167, warning_occurrences=369, suppressions=11
```

失败与收敛：

- 首次 baseline wrapper 外层 timeout 过短，未产生 ledger 且无残留进程；改用足够的同步
  timeout 后 baseline 六类零漂移。该问题不消费 one-shot Gate。
- 首次新增测试有 Rust borrow 编译错误；修正为先构造 invocation。
- 第二次命令误加 `--exact` 导致 0 tests；按“计数必须大于 0”拒绝该结果并重跑。
- 初版实现只保留 grant snapshot，Kernel 新项目 hydrate 后仍丢旧 operation；改由 Kernel
  owner 形成 terminal snapshot。
- 双 Session 回归发现后续空 reconcile 会覆盖已缓存 terminal；改为只有实际 owner snapshot
  才能写入 cache。
- ledger 首次发现 `clippy::filter_map_bool_then`；修正真实代码后六类恢复为零，未改 ledger。

cleanup：

```text
external source worktree：removed
isolated Cargo target：cargo clean removed 9128 files / 11.0 GiB
artifact logs：保留于 G:\EngineTest\256-gateway\l3-c6450e0\artifacts
isolated target：目录已移除
isolated TEMP：宿主拒绝递归 Remove-Item，保留 519 个可再生临时条目
fixture/discovery：宿主拒绝递归 Remove-Item，保留空目录
running descendant process：none
```

停止边界：

```text
L3：completed
L3 完成时历史状态：L4-L6 未授权、未施工；L4 后续结果见第 24 节
旧 256 Gate、P8/P9/P10、Runner/candidate/freezer：未执行
254-R1/R2、F-A/F-B/G、Real Evaluation、三引擎 B 通道：未执行
当时下一步：等待用户单独复核与授权 L4；该授权已由第 23 节消费并在第 24 节完成
```

## 23. L4-only 授权与激活前复核

```text
授权范围：仅 L4 Minimal project.create 与 CreateProject owner 加固
窗口开始：2026-07-25T13:32:36.1442603+08:00
三小时硬停止：2026-07-25T16:32:36.1442603+08:00
保守预算：2 小时 30 分
授权基线 commit：e44be4a21f721eb1d12e125e99ad2b34515061bb
授权基线 tree：96e4454ea70d93358c08ec8378c1e99789c5b57f
禁止：L5-L6、fresh preflight、production smoke、Local CI、旧 256 Gate、
      254-R1/R2、F-A/F-B/G、Real Evaluation、三引擎 B 通道
完成或失败后：回填 L4 证据并立即停止
```

激活前自审：

```text
[x] L3 已完成，代码与入口 clean。
[x] L4 只加固既有 CreateProject owner 并新增唯一 project.create Tool。
[x] direct input 精确为 requestedProjectRoot/projectName，拒绝 internal 字段。
[x] 不新增 project.open、mutation approval、rebind/reconnect 或 caller idempotency。
[x] 目标存在零写入，cleanup 只处理 invocation-owned 新目标。
[x] Registry/Kernel/MCP Adapter 复用现有 owner，不建立第二 lifecycle Module。
[x] 定向与累计测试可在 2 小时 30 分预算内完成。
[x] L5-L6 不在当前连续三小时窗口。
```

自审结论：L4-only 范围闭合，可开始施工；L5-L6 必须在后续独立窗口重新核对 baseline。

## 24. L4 完成证据与停止

```text
窗口开始：2026-07-25T13:32:36.1442603+08:00
窗口完成：2026-07-25T14:35:36.1812374+08:00
结果：L4 completed / stopped
代码 commit：b3c4ba358c73ea917c0ffe2ff51ae3e14966d947
代码 tree：6aa69c4900a279198069c180ab82592efe8fd825
L5-L6：not active
fresh preflight / production smoke / Local CI：未执行
```

### 24.1 owner 与 Tool 结果

```text
ProjectLauncherState：
  absolute/canonical root validation
  parent existence/usability validation
  target absent + exclusive create claim
  existing file/empty directory/project zero-write
  invocation-owned claim marker
  owned cleanup only while claim and target shape remain intact
  uncertain ownership => cleanup_failed and preserve target

project.create direct input：
  requestedProjectRoot
  projectName
  additionalProperties = false

成功 receipt facts：
  status = created
  receiptId
  requestedProjectRoot
  canonicalProjectRoot
  projectName
  projectIdentity
  projectDigest
  readGeneration
  openedInEditor = true
  replayed

execution：
  LauncherContext 无 project read/mutation Grant 执行
  成功后同一 Gateway session 转入 ProjectContext
  exact invocation replay 返回同 receiptId 且 replayed = true
  changed replay => ai_tool.invocation_replay_mismatch
  ProjectContext 新 create 在副作用前失败
  MCP project.create 分支不读取 SessionStatus、不填 project internal facts
```

### 24.2 diagnostic code

```text
project_create.root_empty
project_create.root_not_absolute
project_create.root_not_canonical
project_create.root_expansion_forbidden
project_create.name_invalid
project_create.root_is_filesystem_root
project_create.parent_missing
project_create.parent_unusable
project_create.template_missing
project_create.target_exists
project_create.target_claim_failed
project_create.initialize_failed
project_create.cleanup_failed
project_create.context_transition_failed
project_create.launcher_required
ai_tool.project_create_invocation_invalid
ai_tool.project_create_payload_invalid
ai_tool.invocation_replay_mismatch
```

### 24.3 累计失败集与收敛

```text
首轮 red：
  3 个预期失败，分别证明 Registry 缺失、相对路径被接受、已有目标被写入；
  1 个既有测试保持通过。

累计 owner 回归首轮：
  43 个失败集中于 4 个共享 fixture producer 与 3 处旧 Catalog/Registry 断言；
  fixture 改为 producer 先拥有 fresh target，生产 owner 未放宽。

Gateway/Window：
  1 个 MCP 静态工具数量断言由 20 更新为 21；
  3 个 Native Editor fixture 补齐存在的 parent owner root。

ledger：
  中间出现 items_after_test_module 与历史 large_enum_variant 指纹变化；
  测试模块移至文件末尾，project.create receipt 改用严格 facts schema；
  未修改 ledger，最终六类零漂移。
```

### 24.4 最终验证

```text
L4 六组定向：全部通过
editor_core --lib --locked：556 passed / 2 ignored
ai_tool_gateway --lib --locked：61 passed
editor_window_winit --lib --locked：119 passed

ignored：
  controlled_source_patch::tests::validation::validates_against_workspace_engine_sdk
    原因：local trusted workspace Engine SDK smoke
  tests::llm_patch_source_tests::llm_real_patch_source_openai_compatible_env_gated_smoke
    原因：requires explicit editor-only provider environment and network

canonical ledger：
  previous_ledger_readable = true
  added_diagnostics = []
  removed_diagnostics = []
  changed_diagnostics = []
  new_suppressions = []
  expired_entries = []
```

L4 自审结论：实现未恢复旧三工具、旧 lifecycle、caller 菜谱、candidate/freezer 或
production acceptance 流程；唯一公共新增仍是 `project.create`。本窗口到此停止。L5 只有在
单独新授权和 fresh baseline 复核后才能执行，L6 继续保持未激活。

### 24.5 cleanup

```text
source worktree：removed；Git worktree registration removed
Cargo target：removed by cargo clean；10977 files / 13.2 GiB
artifacts：retained
temp：retained_by_host_policy
temp cleanup：PowerShell exact-path recursive removal was rejected before execution；
  未使用其它 shell 或策略绕过，1507 个隔离临时子目录保留
running Cargo/Rust test session：none
```

## 25. L5-only 授权与激活前复核

```text
授权时间：2026-07-25
授权范围：仅 L5 受影响域回归与 fresh exact-commit preflight
窗口开始：2026-07-25T22:29:02.9996804+08:00
三小时硬停止：2026-07-26T01:29:02.9996804+08:00
保守预算：2 小时 45 分
授权基线 commit：a0d2929748347b3585aa66b71d98982d12c0811e
授权基线 tree：50eebe30cb9bdabf13d7f6cdc66dfdcae3d501cf
禁止：L6 production typed MCP smoke、Local CI、旧 256 Gate、254-R1/R2、
  F-A/F-B/G、Real Evaluation、三引擎 B 通道
```

用户同时要求开始 L5 与 L6，但本文第 5 节和三小时绝对上限不允许把两个各预算
2 小时 45 分的窗口连续执行。本窗口只消费 L5；L5 完成或失败后立即停止。L6 只能在
后续独立窗口重新核对 frozen commit、production binary composition、容量和 one-shot
授权后执行。

激活前复核：

```text
[x] 当前目录仍只有本文一份非 README 施工文档。
[x] L4 implementation commit 与 completion/cleanup 文档 commit 可达。
[x] 主工作树在本次文档修订前 clean。
[x] L5 不运行 production typed MCP smoke 或 Local CI。
[x] L5 使用新的仓库外 exact-commit worktree、target、TEMP/TMP 与 artifacts。
[x] cleanup skill 只允许精确、已解析、施工 role-owned 子目录递归删除；
    宿主拒绝时记录 retained_by_host_policy，禁止绕过。
```

### 25.1 首次 L5 环境失败与合同修正

```text
失败阶段：主工作树 editor_core --lib --locked
结果：546 passed / 10 failed / 2 ignored
首因：宿主默认 TEMP 使用 C:\Users\ZENGHA~1\... 8.3 别名；
  project root 保留短路径，而 canonical scene path 返回长路径，
  触发 project_patch.file_snapshot.scene_path_outside_project。
分类：施工文档 / 环境合同遗漏，不是 project.create 实现回归。
代表性确认：
  project_patch_transaction_creates_scene_entity：canonical L5 TEMP 下 passed
  asset_browser_picker_confirm_writes_structured_scene_asset_ref：canonical L5 TEMP 下 passed
one-shot production smoke / Local CI：未执行
```

处理：第 11.2 节现在要求所有 L5 命令显式绑定 exact canonical role-owned `TEMP/TMP`。
该修订形成新文档 commit 后，使用新的 fresh exact-commit worktree重新执行 L5；不得复用
失败 worktree 冒充新 preflight。

### 25.2 第二次 L5 累计失败与修复

```text
fresh commit：b5fd2df0881e23eab53ce223007ef10522b66f61
editor_core：556 passed / 2 ignored
ai_tool_gateway lib：61 passed
失败：tests/core_scope_contract.rs::core_catalog_retains_ai_authoring_tools
首因：L4 新增 project.create 后，该 integration contract 的静态 Catalog 期望未同步；
  L4 的 --lib 回归未覆盖 integration tests。
分类：L4 测试实现遗漏。
修复：只在 core_scope_contract.rs 导入并期望 TOOL_ID_PROJECT_CREATE；
  全仓搜索未发现第二个同类静态列表。
定向复核：core_scope_contract 3 passed。
one-shot production smoke / Local CI：未执行。
```

测试文件变化使 `b5fd2df` 不再具备 L6 资格。修复形成新 commit 后必须再次建立 fresh
exact-commit worktree，并从 canonical ledger、累计失败集与受影响 crate 重跑 L5。

### 25.3 第三次 L5 fmt 阻断

```text
fresh commit：d8d65dd6134100938569ddd499f7e19252ec7a07
canonical ledger：六类零漂移
fmt：failed；仅 core_scope_contract.rs import wrapping 与 rustfmt 输出不一致
测试：未启动
分类：局部施工格式错误
修复：cargo fmt --all；仅该 integration test 文件变化
```

格式变化使 `d8d65dd` 不再具备 L6 资格。形成新 commit 后再次建立 fresh exact-commit
worktree；不得在第三次 worktree 上继续测试。

### 25.4 第四次 L5 integration fixture 阻断

```text
fresh commit：265f75649b13a0b564feb721e4415901c77c077e
canonical ledger / fmt / editor_core：通过；editor_core 556 passed / 2 ignored
ai_tool_gateway lib：61 passed
失败：mcp_process_smoke 2 failed
首因：
  tools/list 静态数量仍为 20，实际唯一新增 project.create 后为 21；
  project-switch fixture 未创建 owner-owned parent，CreateProject 正确返回 Failed。
分类：L4 integration fixture 漏同步。
修复：数量更新为 21；只创建 parent，不预创建 first/second project target。
全仓同类搜索：无第二处。
定向复核：mcp_process_smoke 2 passed；fmt passed。
one-shot production smoke / Local CI：未执行。
```

integration test 变化使 `265f756` 不再具备 L6 资格。形成新 commit 后再次 fresh 重跑 L5。

## 26. L5 完成证据与停止

```text
窗口开始：2026-07-25T22:29:02.9996804+08:00
窗口完成：2026-07-25T23:28:43.6584004+08:00
结果：L5 completed / stopped
frozen implementation commit：cb58411ef58c7639cf96895b819e5dfb7aa924a0
frozen tree：cfa336d1ae3ae45bb3a661e8dfb2f5d315bd2172
fresh root：G:\EngineTest\256-gateway\l5-cb58411
source before cleanup：clean
main worktree：clean
L6 production smoke / Local CI：未执行
```

最终 fresh preflight：

```text
canonical ledger：
  previous_ledger_readable = true
  added / removed / changed / new_suppressions / expired = 0
  digest = B04735F14303AC995D3360E6DA868D79954E82C61AB28C25E3A341859A54164A
cargo fmt --check：passed
editor_core --lib --locked：556 passed / 2 ignored
ai_tool_gateway --locked：
  lib 61 passed
  core_scope_contract 3 passed
  gateway_process_integration 1 passed
  mcp_process_smoke 2 passed
editor_host --locked：18 + 1 + 2 passed
editor_window_winit --lib --locked：119 passed
project_e2e_gate：未运行；L4 未修改该直接 consumer
```

cleanup：

```text
source worktree：removed；Git registration removed
Cargo target：removed by cargo clean；10204 files / 12.6 GiB
artifacts：retained，12 files
temp：retained_by_host_policy，478 children
temp exact-path Remove-Item：宿主在执行前拒绝；未绕过
```

L5 最终结论：fresh exact-commit preflight 全绿，`cb58411` 是 L6 唯一 eligible frozen
implementation commit。本连续窗口到此停止。L6 必须在新的独立窗口重新核对 commit/tree、
production binary profile/features、容量和 one-shot 授权后才能执行。

## 27. L6 独立授权窗口激活前复核失败

```text
授权时间：2026-07-26
窗口开始：2026-07-26T09:35:14.8620354+08:00
三小时硬停止：2026-07-26T12:35:14.8620354+08:00
授权范围：L6 production typed MCP smoke、Local CI、完成归档
复核 frozen commit：cb58411ef58c7639cf96895b819e5dfb7aa924a0
复核 frozen tree：cfa336d1ae3ae45bb3a661e8dfb2f5d315bd2172
结果：preflight_failed / frozen commit ineligible for L6
production typed MCP smoke run count：0
Local CI run count：0
```

首因：

```text
rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs：
  确实通过 CARGO_BIN_EXE_ai_engine_gateway_mcp 启动 production MCP binary；
  但第一个测试先由 EditorSession 直接 CreateProject，再调用 status/catalog/inspect/search；
  第二个测试也由 EditorSession 直接 CreateProject/OpenProject，只验证同进程 session switch；
  两个测试都没有调用 typed aife_project_create。

rust/crates/editor_window_winit/src/tests/gateway_reconnect.rs：
  没有 typed project.create process smoke。
```

所以 frozen commit 没有证明第 12.2 节的以下必要事实：

```text
LauncherContext tools/list 中 project.create = ready
typed project.create exact two-field direct input
正式 Editor owner 创建并打开目标
同一 MCP process/pipe/client session 转入 ProjectContext
receipt project identity/digest/read generation 与 Editor owner 一致
随后一个 project read Tool 在同一连接成功
launcher authority 不泄漏
```

分类：L5 production-composition 验证覆盖遗漏，不是 L6 runtime smoke 失败。继续运行现有
`mcp_process_smoke` 会用弱测试冒充 L6；直接启动 Local CI 会越过无效上游证据。按 fail-fast
停止，未构建或执行 L6 one-shot smoke，未运行 Local CI，未归档本文。

下一步必须回到一个独立 L5 remediation 窗口：补齐 repository-owned production MCP
process smoke，使其从 LauncherContext typed `project.create` 完成第 12.2 节整条链路；测试
变化后形成新 commit，重新执行 fresh L5。只有新的 frozen commit 通过后，才能重新授权 L6。

## 28. L5 remediation 独立授权窗口

```text
授权时间：2026-07-26
窗口开始：2026-07-26T09:43:55.4779328+08:00
三小时硬停止：2026-07-26T12:43:55.4779328+08:00
范围：
  repository-owned production MCP typed project.create process smoke
  定向与受影响 crate
  新 commit
  fresh exact-commit L5
禁止：
  L6 one-shot production smoke
  Local CI
  最终归档
  旧 256/254 Gate 与三引擎 B 通道
```

实现边界：

```text
只修改 mcp_process_smoke.rs 和施工证据文档；
不修改 production Tool schema、Gateway、Editor owner 或 public Interface；
测试从空 EditorSession / LauncherContext 启动真实 ai_engine_gateway_mcp；
只提交 requestedProjectRoot + projectName；
同一 process/pipe/client session 完成 create 后 status/catalog/inspect；
验证 receipt facts、project identity/digest/read generation、availability transition 和 cleanup。
```

### 28.1 remediation 实现与 commit 前验证

```text
实现文件：
  rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs
production 代码/API 变化：无
新增测试：
  mcp_process_typed_project_create_transitions_launcher_session_in_place
```

该测试通过 `CARGO_BIN_EXE_ai_engine_gateway_mcp` 启动真实 production MCP process，
从 LauncherContext 在同一 pipe/client session 依次执行 initialize、tools/list、status、
catalog、typed project.create、status、catalog、project.inspect 和 shutdown。调用
`aife_project_create` 的 caller-owned input 精确为 `requestedProjectRoot + projectName`；
测试同时验证 schema、receipt facts、固定 session/Editor instance、动态 project
identity/digest/read generation、Catalog availability transition、随后 read Tool 和
owner/discovery/project-root cleanup。

commit 前验证：

```text
cargo fmt --check：passed
新增 production MCP process smoke：1 passed
ai_tool_gateway --locked：
  lib 61 passed
  core_scope_contract 3 passed
  explicit_reconnect 1 passed
  mcp_process_smoke 3 passed
editor_core --lib --locked：556 passed / 2 ignored
editor_host --locked：18 + 1 + 2 passed
editor_window_winit --lib --locked：119 passed
```

已知清理事实：

```text
成功测试拥有并清理自己的 discovery publication、project root 和 child process。
早期失败迭代遗留目录位于显式 canonical role-owned TEMP：
  G:\EngineTest\256-gateway\l5-remediation-dev-temp
宿主在执行前拒绝精确路径 PowerShell recursive removal：
  retained_by_host_policy
未使用其它 shell 或策略绕过。
```

上述验证只证明 remediation implementation 可形成新 commit；它不等价于 fresh L5。
新 commit 形成后必须从仓库外 exact-commit worktree 重新执行第 11.2、11.4 和 12.2 节，
且本窗口仍禁止 L6 one-shot production smoke 与 Local CI。

### 28.2 fresh L5 完成证据与停止

```text
窗口完成：2026-07-26T10:21:11.2507708+08:00
结果：L5 remediation passed / stopped
frozen implementation commit：a817f706e0260953261b1f72a83b96de454089eb
frozen tree：d48ac3a8f2668e139e711f55113d5c46e1a7afb7
fresh root：G:\EngineTest\256-gateway\l5-remediation-a817f70
fresh source before cleanup：clean
main worktree before completion docs：clean
L6 production smoke run count：0
Local CI run count：0
```

fresh preflight：

```text
capacity：G: 263.94 GiB free before start
canonical ledger：
  previous_ledger_readable = true
  added_diagnostics = 0
  removed_diagnostics = 0
  changed_diagnostics = 0
  new_suppressions = 0
  expired_entries = 0
  digest = B04735F14303AC995D3360E6DA868D79954E82C61AB28C25E3A341859A54164A
cargo fmt --check：passed
editor_core --lib --locked：556 passed / 2 ignored
ai_tool_gateway --locked：
  lib 61 passed
  core_scope_contract 3 passed
  explicit_reconnect 1 passed
  mcp_process_smoke 3 passed
editor_host --locked：18 + 1 + 2 passed
editor_window_winit --lib --locked：119 passed
```

ignored tests：

```text
controlled_source_patch::tests::validation::validates_against_workspace_engine_sdk
  local trusted workspace Engine SDK smoke
tests::llm_patch_source_tests::llm_real_patch_source_openai_compatible_env_gated_smoke
  requires explicit editor-only OpenAI-compatible provider environment and network
```

evidence 与 cleanup：

```text
report：
  G:\EngineTest\256-gateway\l5-remediation-a817f70\artifacts\l5-remediation-report.v1.json
report sha256：
  d2f522285c2b7454f9fe1fc9e87d2b839653c4957b4e1e748d27d44756fdad54
Cargo target：removed_by_cargo_clean
source worktree：removed；Git registration removed
running Cargo/Rust descendant：0
artifacts：retained
temp：retained_by_host_policy；478 children
temp cleanup：精确路径 PowerShell recursive removal 在执行前被宿主拒绝；未绕过
```

L5 最终结论：`a817f706…` 是新的 L6 eligible frozen implementation commit。当前授权
窗口到此停止；不得在本窗口运行 L6 production smoke、Local CI 或最终归档。下一步必须由
用户单独授权 L6，并在新窗口重新核对 exact commit/tree、production binary composition、
容量、one-shot run count 与剩余三小时时间。

## 29. L6 独立授权窗口

```text
授权时间：2026-07-26
窗口开始：2026-07-26T10:46:48.2428482+08:00
三小时硬停止：2026-07-26T13:46:48.2428482+08:00
保守预算：production composition/preflight/smoke 30 分钟；Local CI 120 分钟；evidence/cleanup/archive 30 分钟
授权范围：
  frozen commit/tree 与 L5 evidence 复核
  production ai_engine_gateway_mcp profile/features/build/seal
  一次 production typed MCP smoke
  smoke 通过后一次 Local CI
  两者通过后的完成记录、入口同步与归档
禁止：
  smoke 或 Local CI 自动重跑
  修改 production code/schema/test/manifest/lockfile/ledger
  真实 Codex config 修改或热加载声明
  旧 256/254 Gate、F-A/F-B/G 与三引擎 B 通道
```

激活前复核：

```text
[x] L5 report readable，sha256 =
    d2f522285c2b7454f9fe1fc9e87d2b839653c4957b4e1e748d27d44756fdad54
[x] L5 result = passed_stopped
[x] frozen implementation commit =
    a817f706e0260953261b1f72a83b96de454089eb
[x] frozen tree =
    d48ac3a8f2668e139e711f55113d5c46e1a7afb7
[x] production smoke run count = 0
[x] Local CI run count = 0
[x] 主工作树在本次授权文档修改前 clean
[x] G: 可用 263.43 GiB，满足 source/target/temp/artifacts、Local CI nested worktree 与余量
```

### 29.1 L6 结果：Local CI 失败并停止

```text
窗口停止：2026-07-26T11:35:18.6439647+08:00
结果：failed_stopped
frozen implementation commit：a817f706e0260953261b1f72a83b96de454089eb
frozen tree：d48ac3a8f2668e139e711f55113d5c46e1a7afb7
L6 root：G:\EngineTest\256-gateway\l6-a817f70
production smoke run count：1
Local CI run count：1
自动重跑：0
完成归档：未执行
```

production composition 与 smoke：

```text
平台：Windows
profile：Cargo built-in test
features：default
ai_engine_gateway_mcp.exe bytes：9981952
binary sha256：
  de8f8b2927416cc0669c150b505c8ad44f6a1fb22805ebaacfbe23b9870184a9
reopen verification：passed
smoke 后 binary digest：unchanged
production typed MCP smoke：
  mcp_process_typed_project_create_transitions_launcher_session_in_place = passed
  gateway_context_transition_real_pipe_keeps_adapter_without_grant_inheritance = passed
  duration = 3.444 seconds
  test-owned temp children after smoke = 0
```

Local CI：

```text
run id：local-a817f706e026-1785034539
duration：2011.453 seconds
source workspace：clean
isolated workspace：clean
nested worktree cleanup：removed
format / architecture / suppression / clippy / code_hygiene：passed
default_workspace：failed，exit 101，not timed out
all_features_workspace：failed，exit 101，not timed out
两者共同失败测试：
  project_e2e_gate::c01_golden_gate::tests::
  c01_golden_gate_builds_all_ten_candidates_through_common_entry
```

证据边界：

```text
quality report 使用 summary mode，只保留失败 test name、exit 101 和末尾摘要；
没有保留具体 panic/assertion 文本，trace = null；
test-owned TEMP 中没有留下 c01-golden-gate-* 目录；
因此本窗口只能证明共同失败 owner test，不能猜测更细断言。
不得通过重跑该 test 消费第二次 Local CI 或冒充本次 artifact。
```

artifact：

```text
failure report：
  G:\EngineTest\256-gateway\l6-a817f70\artifacts\l6-failure-report.v1.json
failure report sha256：
  635f9bd990b5a6b23d5110d82313b349f450d9a9879d3cc11c9ae7987a5040b5
local-ci-run-report sha256：
  aef86f8ee887c6b446c8205fb9a54b82d05427eac2c9375853f5b69979afd267
quality-gate-report sha256：
  f2b03656d719c90f41ae145e93375203c4a34cdcf884883a3fa9ccefda2a3e96
```

cleanup：

```text
Local CI nested worktree：removed
L6 source worktree / Git registration：removed
external Cargo target：removed
Local CI build target：removed
running Cargo/Rust/Gateway process：0
artifacts：retained
temp：retained_by_host_policy；1232 children
temp cleanup：精确路径 PowerShell recursive removal 在执行前被宿主拒绝；未绕过
```

L6 当时结论：本次 production Tool/Gateway smoke 通过，权威失败位于已有
`project_e2e_gate` C01 golden gate owner test 或其 fixture/evidence 环境，而不是本次 typed
MCP smoke。summary artifact 没有保留具体 assertion。该窗口已经停止；后续边界由第 30 节
2026-07-26 amendment 取代，不得从本历史结论推导完整 L5 或 production smoke 重跑。

## 30. 2026-07-26 公共验证纪律对齐与下一步

### 30.1 修订原因

L6 证明了旧验证合同存在四个结构问题：

```text
L5 没有覆盖 C01，L6 才突然扩大到全 workspace
default 失败后仍继续 all-features，重复消耗高成本
summary report 丢失具体 panic/assertion
test/fixture 修复被机械绑定为 production smoke 整体失效
```

这些是施工验证与 evidence Interface 问题，不推翻 256 产品方案。公共项目/施工 skill 已删减
为按声明选择、按输入失效和 fail-fast；第 5、11、12、13、15、17 节已同步。

### 30.2 当前可复用证据

```text
production typed MCP receipt：有效
binary sha256：
  de8f8b2927416cc0669c150b505c8ad44f6a1fb22805ebaacfbe23b9870184a9
project.create / Gateway production source：本轮文档修订未改变
真实 Codex config：未修改
Local CI result：failed，不可作为完成证据
C01 internal assertion：未知，必须重新取得
```

只有后续修改触及 production source、Cargo production dependency、profile/features、binary
或 launch composition，才重新执行 production smoke。

### 30.3 下一独立授权：C01 owner diagnosis

下一施工只建立 red-capable、可定位的反馈循环，不运行 L5、production smoke 或 Local CI：

```powershell
Set-Location G:\gameEngin\rust
$env:TEMP = '<canonical role-owned diagnosis temp>'
$env:TMP = $env:TEMP
$env:RUST_BACKTRACE = '1'
cargo test -p project_e2e_gate --lib --locked `
  c01_golden_gate::tests::c01_golden_gate_builds_all_ten_candidates_through_common_entry `
  -- --exact --nocapture --test-threads=1
```

必须分别保存 stdout、stderr、完整 panic/backtrace、fixture root、测试二进制、exit code、duration
和 cleanup 状态。若不能复现，则比较 L6 exact environment 与 diagnosis environment，禁止直接
提出代码修复。

本切片允许的初始动作只有只读复核和上述一次诊断运行。若诊断证明需要修改
`project_e2e_gate` fixture/test 或 `quality_gate` evidence retention，必须停止并回填具体
owner、red assertion、修改范围和 affected consumers，再由用户授权 Integration repair。

### 30.4 后续完成边界

Integration repair 通过后：

1. 形成 clean exact commit，仅运行实际受影响域。
2. 记录 production source/composition 是否变化。
3. 未变化则复用第 30.2 节 receipt；变化才重新执行 production smoke。
4. 单独授权 Local CI；runner 必须保留完整首因并在上游失败后 fail-fast。
5. Local CI 通过后才生成完成记录、同步入口并归档本文。

本轮只完成文档修订与自审，没有消费 C01 diagnosis、Integration repair、production smoke 或
Local CI 授权。

### 30.5 C01 diagnosis 结果与 scoped remediation 授权

一次且仅一次的 C01 owner diagnosis 已在 source commit
`77c390fd2a1f7412c77eef2aea8e917bd049be5e` 执行，退出码为 `101`。完整证据位于：

```text
G:\EngineTest\256-gateway\c01-diagnosis-77c390f\artifacts
```

首个 red assertion 为：

```text
rust/crates/project_e2e_gate/src/c01_golden_gate.rs:2549
left: Failed
right: Committed
```

失败发生在十个 candidate 开始构造之前。`project.create` 的正式合同与产品实现都要求目标父目录
已存在；C01 fixture owner 创建了唯一 `root\project` 路径，却没有先创建自己拥有的唯一 `root`。
因此分类为 `project_e2e_gate fixture ownership / contract synchronization`，不是 256 方案错误，
也不修改 `project.create` 产品实现。

本次 Integration repair 的授权范围固定为：

1. owner：`rust/crates/project_e2e_gate/src/c01_golden_gate.rs`。
2. 在常规 C01 golden gate test 与同构的 ignored local Preview/export test 中，由 fixture 显式创建
   自己拥有的唯一父目录；常规测试在进入 candidate construction 前释放初始 project session，
   使 Windows cleanup 不再被遮蔽 binding 持有的目录 capability 阻塞。
3. 两处 CreateProject assertion 都输出完整 `CommandResult`，避免再次丢失 diagnostics。
4. 只运行
   `c01_golden_gate_builds_all_ten_candidates_through_common_entry` 定向测试；ignored 真实窗口 Gate
   只参加编译，不执行。
5. 不运行 L5、production smoke 或 Local CI；不修改 production source、dependency、profile、
   feature、binary、launch composition 或真实配置。

自审结论：该修复只使历史 C01 fixture 遵守已确认的 no-overwrite/parent ownership 合同，不改变
正式方案。定向测试能直接捕获原始 red assertion；production receipt 的任何输入都未变化，因此
继续复用第 30.2 节证据。

### 30.6 scoped remediation 结果

最终 owner verification：

```text
command:
  cargo test -p project_e2e_gate --lib --locked
    c01_golden_gate::tests::c01_golden_gate_builds_all_ten_candidates_through_common_entry
    -- --exact --nocapture --test-threads=1
result:
  1 passed / 0 failed / 63 filtered out
duration:
  52.680 seconds
cleanup:
  no c01-golden-gate fixture remained
  no Cargo/Rust descendant remained
evidence:
  G:\EngineTest\256-gateway\c01-remediation-77c390f-2\artifacts
```

首次修复验证虽通过，但初始 project session 的遮蔽 binding 使 Windows test cleanup 留下空目录；
补充显式 `drop(session)` 后，第二次验证通过且 cleanup clean。首次空目录的外部递归删除被宿主
策略拒绝，已记录为 `retained_by_host_policy`，路径为：

```text
G:\EngineTest\256-gateway\c01-remediation-77c390f-1\temp\
  c01-golden-gate-1785043152909668100
```

本修复只改变 `project_e2e_gate` test/fixture identity，没有改变 production source、dependency、
profile、feature、binary、launch composition 或真实配置。第 30.2 节 production receipt 继续有效。
下一步需要单独授权一次 fail-fast Local CI；本授权未运行 Local CI。

### 30.7 fail-fast Local CI 启动前复核与未消费授权

用户曾授权绑定 `463f81c4e3b2756ebbe7cadca67f270d215550d4` 的一次 fail-fast Local CI。
启动前只读复核发现：

```text
local_ci.rs：内部 verify 硬编码 --report-mode summary
runner.rs：default_workspace 失败后仍继续 all_features_workspace 与 code_hygiene
```

这不满足第 12.2 节资格，因此 Local CI command 未启动、授权未消费、run count 未增加。
分类为 verification harness implementation gap，不是 256 产品实现失败。

### 30.8 Local CI runner remediation 结果

本次 scoped remediation 只修改 `rust/crates/quality_gate`：

1. `verify` 新增显式 `--fail-fast` 请求，默认行为保持不变。
2. Local CI canonical invocation 固定为 `--report-mode trace --fail-fast`。
3. report 记录 `fail_fast`，Local CI 验证 Trace 与 fail-fast 均为真。
4. metadata/architecture/format/suppression/Clippy/default/all-features 任一已执行 stage 失败或超时后，
   fail-fast request 立即写 report 并停止后续 stage。
5. Trace 保存 command executor 已有 16 MiB/stream 有界捕获的完整 stdout/stderr；超过上限时保留
   head/tail 和明确 truncation marker，不再二次裁剪为末尾 8 KiB。

验证：

```text
cargo test -p quality_gate --locked：
  62 tests passed across unit/integration/doc targets
cargo test -p quality_gate --lib --locked
  local_ci::tests::commit_bound_quality_report_validation_is_fail_closed -- --exact：
  1 passed
cargo clippy -p quality_gate --all-targets --locked -- -D warnings：
  passed
cargo fmt --all -- --check：
  passed
```

本修复只改变 verification harness identity，不改变 production source、dependency、binary、
profile、feature、launch composition 或真实配置。第 30.2 节 production receipt 继续有效。
形成 clean exact commit 后，下一步需要用户重新单独授权一次绑定该 commit 的 fail-fast Local CI；
本次 runner remediation 授权未运行 Local CI。

### 30.9 `4475519` Local CI 外部包装层失败

用户授权绑定以下 clean exact source：

```text
commit：4475519770444a495e51dab4a792f41737236971
tree：90fe9c0ee75d0113af1b0b1248bf210a5243703d
command：cargo run -p quality_gate --locked -- local-ci --commit HEAD
external root：G:\EngineTest\256-gateway\local-ci-4475519
```

canonical command 启动后，外部 PowerShell 5.1 证据包装器因
`$ErrorActionPreference = 'Stop'` 把 Cargo 正常写到原生 stderr 的
`Compiling quality_gate ...` 进度误判为 `NativeCommandError`，约 0.6 秒即终止包装脚本。
stdout/stderr 重定向文件均为空，未生成 Local CI run ID、`local-ci-run-report.v1.json` 或
`quality-gate-report.v2.json`，也没有残留 Cargo/Rust/quality_gate 进程。

分类：

```text
owner：external invocation/evidence wrapper
不是：256 设计、产品实现、C01 fixture 或 canonical Local CI runner failure
授权：已消费；本次不得重跑
资格：没有 Local CI pass/fail 资格证据
production receipt：输入未变，继续有效
```

后续外部证据包装合同固定为：

1. 使用 `Start-Process -Wait -PassThru` 启动 canonical command。
2. 使用 `-RedirectStandardOutput` 与 `-RedirectStandardError` 分离原生输出。
3. 仅以返回进程对象的 `ExitCode` 判定命令结果；不得用 PowerShell error stream 代替。
4. 无论进程结果如何都写 invocation metadata，然后再解析 repository-owned report。
5. 必须绑定新的 clean exact commit、使用新的外部 root 并取得新的单独授权；不得复用本次目录。

### 30.10 Local CI 连续施工授权边界修订与当前窗口

用户于 `2026-07-26T15:13:02.1295841+08:00` 修改施工边界：

```text
一个连续施工窗口只需一次用户授权
窗口内允许多次 Local CI
Local CI 失败后先做 owner 级诊断/修复，再用新 run root 重跑
不把 Local CI 当调试循环
窗口内实现、测试、等待、证据和 cleanup 总时长不得超过三小时
2:45 后不启动新实现或验证；3:00 强制停止
```

当前授权绑定：

```text
qualification source commit：e99b5af4547658a8f0f5239851b2b1c14d6b45dc
qualification source tree：de26d2db3185a8a9c2220b2fc91d2de0bd1fd02f
窗口开始：2026-07-26T15:13:02.1295841+08:00
停止启动新工作：2026-07-26T17:58:02.1295841+08:00
硬停止：2026-07-26T18:13:02.1295841+08:00
允许：fresh fail-fast Local CI；必要的 owner diagnosis、scoped repair、affected tests 和后续 Local CI
禁止：L5、production smoke、254 Gate、F-A/F-B/G、三引擎 B 通道
```

规则修订会形成 governance-only 新 commit，不改变 `e99b5af…` 的 Rust source/tree。为保持用户
指定的 qualification identity，Local CI 从 repository-external、detached、clean 的
`e99b5af…` exact-commit worktree 执行；每次运行使用新的 repository-external evidence root。

### 30.11 `e99b5af` fresh Local CI 与最终归档

Run 01：

```text
run id：local-e99b5af45476-1785050176
qualification commit：e99b5af4547658a8f0f5239851b2b1c14d6b45dc
qualification tree：de26d2db3185a8a9c2220b2fc91d2de0bd1fd02f
report mode：trace
fail fast：true
stage result：12/12 passed
CI execution status：observed_commit_passed
source workspace：clean
isolated workspace：clean
diagnostics：0
nested cleanup：removed
quality report sha256：8bdc695b7bd3cca82100237addd6ecda82b5dbe322347ba6bfcca3b7f8f86544
```

外部证据：

```text
G:\EngineTest\256-gateway\local-ci-e99b5af-run-01\artifacts
local-ci-evidence-summary.v1.json sha256：
  24ef70dca5740bc1f012951a6ceca18c50bf0c19b676de7027e19ecff261eb58
```

本窗口只运行一次 Local CI，因为 Run 01 通过，无需重跑。qualification source worktree 已删除，
剩余 Cargo/Rust/quality_gate 进程为 0。外部 TEMP
`G:\EngineTest\256-gateway\local-ci-e99b5af-run-01\temp` 在严格子目录、junction target 和
owned process 检查通过后，仍被宿主策略拒绝递归删除，终态记录为
`retained_by_host_policy`；没有使用其它 shell 或绕过策略。

```text
cleanup-report.v1.json sha256：
  976e0e38791d69ff042e549ef4a5c62c7064fb22e61d2b2b45f44c479b443c31
```

production receipt 输入未变化且继续有效。至此 production typed MCP smoke 与当前 release
policy Local CI 均通过，完成记录和入口已同步，本文归档到 `施工文档/已完成/`。
