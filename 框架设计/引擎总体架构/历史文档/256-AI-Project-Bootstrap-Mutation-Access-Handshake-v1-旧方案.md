# AI Project Bootstrap / Mutation Access Handshake v1 旧方案

> 状态：`superseded_by_256_project_create_reset`。2026-07-24 用户确认旧 256 三工具方案及其实现整体废弃。本文仅保留历史设计与失败证据，不再是当前规范、施工授权、实现来源或可执行 Gate。
>
> 禁止依据本文恢复 `project.open`、`session.request_mutation_access`、P8/P8Q/P8F/P8A/P9/P10、construction Runner、freeze、activation 或旧 production conformance。当前 256 真相只来自根目录 `256-Editor-Instance-Gateway-Lifecycle-Minimal-Project-Create-v1方案.md`。

## 0. 文档状态

```text
系统编号：256
方案选择：方案 B（目标级 Project Bootstrap + Mutation Access Handshake）
确认状态：用户已确认
方案自审：已通过
讨论状态：已完成
施工状态：施工中（P8S1/P8R3A 已完成；下一步仅可单独授权 P8R3B；P8R3C/P8R3D/P8Q/P8F/P8A/P9/P10 未授权）
当前施工文档：施工文档/当前/256-当前可自动化施工文档-AI-Project-Bootstrap-Mutation-Access-Handshake-v1.md
待执行施工文档：无
```

本文是 249、251、253、254 与 255 之后的独立正式方案。它只补齐 AI 从 launcher/no-project 状态进入已绑定项目 Session，并向 Native Editor 用户请求 mutation access 的 typed MCP 前门。

本文不是施工文档，具体授权边界以唯一 `施工文档/当前/` 文档为准。第五次 P8 的 Windows Shell 缺陷已由 P8R1/P8R2 收敛；随后单次 fresh P8 candidate `198373e…` 在 `project_e2e_gate --all-features` 失败且 `p9Eligible=false`。该授权已消费且不得重跑。2026-07-24 的 P8S1 只修订施工证据边界：把可重复、只读的 `P8 Qualification` 与一次性的 `P8 Freeze` 分开。代码 remediation、Runner 分流、P8 Qualification、P8 Freeze、P8A、P9、P10 均须分别获得新授权。本文不授权创建或复用历史 candidate，也不授权执行任何历史 Gate。

## 1. 一句话说明

256 让 AI 可以通过少量目标级 typed Tool：

```text
创建或打开一个安全项目
  -> 得到绑定到该项目的 MCP Session
  -> 请求有边界的 mutation access
  -> 等待 Native Editor 用户批准或拒绝
```

它只建立可审计的访问条件，不替 AI 规划游戏制作步骤，也不自动执行任何项目修改。

## 2. 问题

255 完成后，AI 已能发现并理解现有 typed Tool 的动态可用性，但 no-project Session 仍缺少进入项目的正式 typed Interface：

1. 当前业务 Registry 只包含项目内观察、修改、预览、诊断、构建和回滚 Tool，没有 `project.create` 或 `project.open`。
2. Gateway 建立项目 Session 时要求已有项目身份与 root digest；launcher/no-project 状态无法仅靠现有 typed Tool 进入 active project。
3. 当前 MCP process 测试先在 MCP 外调用 Editor Command 创建项目，再发布 discovery；这证明底层能力存在，但不是 AI 可调用的 production typed Interface。
4. Native Editor 已拥有正式 `CreateProject` / `OpenProject` Command，不能由 MCP Adapter 用文件系统拼装代替。
5. mutation access request 与 Native Editor approval lifecycle 已有底座，但当前 MCP control surface 没有明确的 typed 请求入口。
6. 255 可以返回 `authorization_required`，但按其边界只解释状态，不创建 approval request。

结果是：AI 在真实 launcher 状态下能够知道“为什么不能修改”，却不能通过引擎提供的 typed Tool 正式创建/打开项目并发起一次用户可见的 mutation 授权请求。

## 3. 现有实现证据

本方案建立在以下当前代码事实之上，后续施工必须重新核对行号和语义：

```text
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
  当前业务 Tool Registry，没有 project.create/project.open。

rust/crates/ai_tool_gateway/tests/core_scope_contract.rs
  固化当前 typed Tool surface。

rust/crates/ai_tool_gateway/src/protocol.rs
  Gateway 连接前要求项目 identity/root digest。

rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs
  当前测试先通过 Editor Command 创建项目，再发布 discovery。

rust/crates/editor_ui_model/src/command.rs
  UiCommandPayload::OpenProject / CreateProject。

rust/crates/editor_window_winit/src/application.rs
  Native Editor mutation access request/approval 入口。

rust/crates/ai_tool_gateway/src/core.rs
  Session access、awaiting_user/active 状态、Grant 与 generation authority。

rust/crates/ai_tool_gateway/src/mcp_stdio.rs
  当前 MCP control tools 与 typed projection。
```

这些事实说明缺口是“正式 owner 能力没有投影成 typed MCP 目标 Interface”，不是缺少另一套项目创建器、审批系统或跨工具 Workflow。

## 4. 目标

256 必须做到：

1. launcher/no-project MCP Client 能稳定发现项目启动 Tool。
2. `project.create` 只通过正式 Editor-owned CreateProject 路径创建项目。
3. `project.open` 只通过正式 Editor-owned OpenProject 路径打开项目。
4. 所有路径先经过 SafeProjectPath 和项目写入 containment 规则。
5. 成功后由 Editor 产生真实 project identity、canonical root 与 root digest。
6. discovery publication 更新到新项目 binding。
7. 原 Session 明确失效或转入 reconnect-required；新 Session 绑定 exact project identity/root digest。
8. AI 能通过 typed Tool 请求 bounded goal-level mutation access。
9. 请求进入 `awaiting_user`，由 Native Editor 展示并由用户批准或拒绝。
10. 只有真实批准后 mutation access 才能变为 `active`。
11. create/open/request 各自返回可审计 receipt、generation 和结构化诊断。
12. AI 继续拥有项目内 Tool 选择、顺序、分支、停止和动态重规划权。

## 5. 非目标

256 不做：

- 不自动生成、修改、预览、构建或交付游戏。
- 不自动批准、模拟批准、代替用户点击或绕过 Native Editor approval。
- 不在授权后自动调用 mutation Tool。
- 不返回固定后续步骤、`nextToolIds`、调用次数或完成状态机。
- 不建立 `start_task`、Project Runner、Bootstrap Workflow 或隐藏 Planner。
- 不允许任意文件系统路径、项目根外写入或 Adapter 直接创建项目文件。
- 不把 caller input 扩张为内部 digest、generation、grantRef 或 project identity。
- 不建立 production candidate、freezer、activation、attempt 或 release verifier。
- 不证明精确引擎版本通过真实 AI outcome acceptance。
- 不恢复 254-R1/R2、F-A/F-B/G、Real Evaluation 或三引擎 B 通道。

## 6. 方案比较

### 6.1 方案 A：只增加低层 control calls

由 AI 分别调用 path validate、Editor command、discovery publish、session bind 和 access transition。

拒绝原因：

- 泄漏内部生命周期步骤；
- AI 必须学习基础设施菜谱；
- 中间态可被外部调用者错误组合；
- ownership、cleanup 和失败恢复不完整。

### 6.2 方案 B：目标级 Project Bootstrap + Mutation Access Handshake

公共 typed Interface 收敛为：

```text
project.create
project.open
session.request_mutation_access
```

每个 Tool 内部可以执行严格、事务化的 owner 步骤；Tool 外不规定 AI 之后必须调用什么。

选择原因：

- Interface 小，Implementation 隐藏真实复杂度；
- 复用 Editor、SafeProjectPath、Gateway authority 和 255 Catalog 的现有 owner；
- 解决 no-project 和 awaiting-user 的真实缺口；
- 不把项目制作过程变成菜谱。

### 6.3 方案 C：单一 `start_task`

一个 Tool 接受用户目标并自动 create/open、授权、修改、预览和构建。

拒绝原因：

- 混合用户目标、项目生命周期、授权和业务执行；
- 隐藏审批边界；
- 固定跨工具顺序并限制 AI 重规划；
- 重建已从 254 退役的 Runner/candidate lifecycle。

## 7. 深 Module 设计

### 7.1 公共 Module

256 只新增一个公共目标级深 Module：

```text
AiProjectAccessModule
```

公共 Interface 只表达三个用户可理解的目标：

```text
create_project(input) -> ProjectBootstrapResult
open_project(input) -> ProjectBootstrapResult
request_mutation_access(input) -> MutationAccessRequestResult
```

typed MCP Adapter 分别投影为：

```text
project.create
project.open
session.request_mutation_access
```

命名可在施工时按 Registry 现有规范做最小收敛，但不得改变目标级边界或合并为 Runner。

### 7.2 内部 Seam

以下是 Module 内部 Implementation seam，不是 AI 公共菜谱：

```text
SafeProjectPath validation
Editor Command dispatch
project identity/root digest materialization
discovery publication
session invalidation/rebind
mutation access authority transition
Native Editor approval presentation
receipt and diagnostic projection
```

这些步骤可以严格排序、补偿和 fail-closed，因为它们属于一个 Tool 的正确性，不规定 AI 在 Tool 外如何完成游戏目标。

### 7.3 所有权

| 事实或动作 | 唯一 owner | 256 的责任 |
|---|---|---|
| 路径安全与 containment | SafeProjectPath / project write owner | 调用并保留结构化拒绝 |
| 创建/打开项目 | Native Editor project lifecycle owner | 通过正式 Command 请求，不复制实现 |
| project identity/root digest | Editor project binding owner | 接收权威结果，不由 caller 提供 |
| discovery publication | Editor-hosted Gateway publication owner | 原子更新可发现 binding |
| MCP Session identity | Gateway Core | stale fail-closed，建立新 binding |
| mutation access 状态 | Gateway authority owner | 创建 bounded request，维护 generation |
| 用户批准/拒绝 | Native Editor 用户交互 owner | 只展示和消费真实决定 |
| Tool schema/descriptor | AiToolContractRegistry | 注册唯一 typed contract |
| availability | 255 Capability-aware Catalog | 解释当前状态，不代替本 Module 执行动作 |
|协议映射 | MCP Adapter | 薄投影，不重算 owner 事实 |

## 8. Project Create / Open Interface

### 8.1 Caller-owned 输入

`project.create` 只接受完成创建目标所需的用户语义，例如：

```text
parentLocation 或受支持的 location reference
projectName
可选 templateId（仅当正式 Editor 已注册）
可选 idempotencyKey
```

`project.open` 只接受：

```text
projectLocation 或受支持的 recentProjectId
可选 idempotencyKey
```

具体 schema 必须遵守 SafeProjectPath：

- 路径必须是显式 caller intent；
- canonicalization 后再校验 containment；
- 不允许项目根、盘符根、环境变量或隐式当前目录作为模糊写入目标；
- 不允许 caller 伪造 canonical path、project identity、root digest 或 generation；
- recentProjectId 必须由 Editor-owned registry 解析，Adapter 不维护第二份映射。

### 8.2 Engine-owned 字段

以下字段由 owner 产生，禁止出现在 direct caller input：

```text
projectIdentity
canonicalProjectRoot
projectRootDigest
discoveryGeneration
sessionId
sessionBindingDigest
readGeneration
accessGeneration
opaque grantRef
operation/receipt lineage
```

如果 caller 提交这些字段，schema 应拒绝 unknown/internal fields，而不是静默采纳。

### 8.3 Create 内部合同

`project.create` 的原子目标为“通过正式 Editor 路径创建并打开一个可绑定项目”。Implementation 必须：

1. 校验输入 schema、名称和安全路径；
2. 确认目标不是已占用或含混目录；
3. 调用 Editor-owned `CreateProject`；
4. 由 Editor 完成项目初始化并产生 identity/root digest；
5. 确认 active project 与 exact canonical root 一致；
6. 发布新 discovery generation；
7. 使旧 no-project/stale binding 明确失效；
8. 返回 reconnect/binding receipt。

任何一步失败均不得把部分目录宣传为成功项目。能够安全补偿的临时产物由 owner 清理；无法无损清理时返回明确 partial-state diagnostic 和 recoverability，不得静默重试。

### 8.4 Open 内部合同

`project.open` 的原子目标为“通过正式 Editor 路径打开并绑定一个有效项目”。Implementation 必须：

1. 校验安全路径或 recentProjectId；
2. 读取足以识别项目的最小 manifest；
3. 调用 Editor-owned `OpenProject`；
4. 由 Editor 校验项目格式、RuntimeModule 与 migration 条件；
5. 产生/读取权威 project identity/root digest；
6. 发布 discovery 并推进 generation；
7. 使旧 binding fail-closed；
8. 返回 reconnect/binding receipt。

Open 不自动迁移、修复或改写损坏项目。若正式 Editor 需要迁移确认，应返回结构化 blocker，由用户在 Native Editor 决策。

## 9. Discovery 与 Session Rebind

### 9.1 不允许原地偷换项目

项目切换后不能在原 Session id 下静默替换 project identity。否则先前 Catalog、read generation、Grant、operation 与 receipt 会被错误归属于新项目。

标准结果为：

```text
old session:
  state = reconnect_required 或 stale
  boundProjectIdentity 保留旧值/none

new discovery:
  publicationGeneration 增加
  exact project identity/root digest 可验证

new session:
  通过 discovery 重连
  绑定 exact project identity/root digest
  新 read/access/operation generations
```

如果现有 transport 支持受证明的显式 rebind，也必须等价地产生新 session binding identity，并让全部旧 authority token 失效；不得仅改内存路径。

### 9.2 Bootstrap receipt

`ProjectBootstrapResult` 至少包含：

```text
outcome
operationId
receiptId
commandKind
projectIdentity
projectRootDigest
publicationGeneration
sessionAction
newSessionBindingHint
diagnostics[]
```

`sessionAction` 只能表达事实，例如 `reconnect_required` 或 `new_binding_available`，不能给出固定下一 Tool。

绝对项目路径只在本地、受信任且确有必要的 Editor surface 显示；MCP receipt 默认返回安全 reference 或脱敏路径信息。

### 9.3 Stale 规则

以下任一情况必须 fail-closed：

- discovery generation 落后；
- project identity 或 root digest 不匹配；
- Editor 当前项目在操作中被切换；
- Session 已过期、被替换或断开；
- create/open receipt 与当前 publication lineage 不一致；
- 旧项目的 Grant、operation 或 receipt 被带入新 binding。

失败后允许 AI根据最新状态重新选择 create、open、inspect 或停止，但引擎不得自动循环重试。

## 10. Mutation Access Handshake

### 10.1 输入

`session.request_mutation_access` 的 caller-owned 输入只表达用户目标与授权上限：

```text
goalSummary
riskClass
requestedCapabilities
可选 duration/expiry 上限
可选 idempotencyKey
```

合同要求：

- 使用 bounded goal-level Grant，不使用任意长期全局写权限；
- `requestedCapabilities` 只能从已注册安全能力集合选择；
- riskClass 必须由引擎验证或收紧，caller 不能把高风险动作声明成低风险；
- 不接受 grantRef、accessGeneration、approvalId、project digest 等内部字段；
- 不接受后续 Tool 列表、固定顺序或操作次数。

### 10.2 状态机

唯一授权状态机由 Gateway authority owner 持有：

```text
inactive
  -> awaiting_user
  -> active | rejected | expired | cancelled

active
  -> expired | consumed/closed | revoked
```

请求 Tool 只负责从合法状态创建一个 bounded request。它不能批准自己，也不能等待到 active 后继续执行项目 mutation。

### 10.3 Native Editor 决策

`awaiting_user` 时 Native Editor 必须显示：

```text
请求来源与 Session
绑定项目身份
goalSummary
riskClass
requestedCapabilities
期限/消费边界
```

批准、拒绝、撤销都由用户在 Native Editor 进行。MCP 只能通过 status/Catalog/后续调用观察权威状态，不得模拟 UI 决策。

### 10.4 幂等与并发

- 同一 Session、同一 project binding、同一规范化 request 与 idempotencyKey 重复提交，应返回同一 pending/terminal request，不创建多张批准卡。
- 不同目标或能力集合不得错误合并。
- 已存在 awaiting_user 时，新请求默认结构化拒绝或返回 existing request；不得堆叠多个含混审批。
- active Grant 是否允许扩权必须走新的显式用户决定，不能由请求 Tool 原地扩大。
- 项目切换、Session 失效或期限到期立即终止 request/Grant。

### 10.5 Request receipt

结果至少包含：

```text
outcome = awaiting_user | already_pending | rejected | blocked
requestId
receiptId
projectIdentity
requestDigest
accessGeneration
expiresAt
diagnostics[]
```

active 状态与 opaque grantRef 只能由 authority owner 在真实批准后产生，并通过既有 Session access surface 安全消费。request receipt 本身不是 Grant。

## 11. 与 255 Catalog 的集成

256 不改变 255 的责任：

- Catalog 只解释状态，不自动执行 create/open/request；
- no-project 时 `project.create` / `project.open` 应为 `ready` 或带真实 blocker；
- 项目内 Tool 在 no-project 时继续可发现并返回 `blocked`；
- mutation Tool 在无 active access 时返回 `authorization_required`；
- `session.request_mutation_access` 在可创建请求时为 `ready`，存在 pending request 时返回相应结构化状态；
- create/open 成功及 access generation 变化必须推进 availability basis/digest；
- execute 时继续按最新 owner 事实重检，不能依赖旧 Catalog snapshot。

Catalog reason 可以给出 `resolutionKind`，但不得包含自动调用 payload、`nextToolIds` 或顺序。

## 12. 失败、取消与恢复

### 12.1 Create/Open

- schema/path/name 无效：无副作用失败；
- 目标已存在或不是项目：结构化冲突，不覆盖；
- Editor Command 失败：保留 owner diagnostic；
- 初始化中取消：按 Editor transaction 边界补偿；
- discovery publication 失败：项目可以存在，但结果不得宣称 Session 已绑定；
- reconnect 失败：返回可重新发现的 binding evidence，不重做 Create/Open；
- 项目切换竞争：stale 失败，不把 receipt 绑定到错误项目。

### 12.2 Mutation request

- no-project/stale Session：拒绝创建请求；
- 已 pending：返回 existing request；
- 用户拒绝：terminal rejected，不自动再请求；
- TTL 到期：terminal expired；
- Session 断开或项目切换：取消/过期；
- UI presentation 失败：不得留下不可见且可批准的悬空请求；
- approval 与 expiry 并发：authority owner 以单一 generation 原子决议。

### 12.3 重试边界

256 不规定 AI 必须重试。幂等 key 只防止 transport 重放产生重复副作用。

业务重试由 AI 根据诊断和用户目标决定；高风险、用户拒绝、过期或项目漂移后的新请求必须是可区分的新 intent，不能偷偷复用旧 approval。

## 13. 安全合同

1. Adapter 不直接访问文件系统创建项目。
2. 所有 project path 先 canonicalize，再执行 containment 和链接/重解析点检查。
3. create 不覆盖已有非空目录；open 不修复或迁移未知项目。
4. project identity/root digest 只能由 Editor owner 产生。
5. discovery/rebind 必须让旧 Session、Grant 和 receipts fail-closed。
6. mutation access 默认最小权限、有限目标、有限期限。
7. Native Editor 是批准权威；MCP Client 不能自批。
8. receipt/diagnostic 不泄漏 secret、credential、内部 panic 或不必要的绝对路径。
9. cancellation 与失败不能遗留“不可见但 active”的 access。
10. 测试 Adapter 或 headless helper 不能作为 production MCP conformance 的替代证据。

## 14. 成熟引擎对照

### 14.1 Unity

Unity Hub 将创建/打开项目与项目内 Editor Tool 分离。可学习的是生命周期入口与编辑能力分责；不照搬 Hub 的外部产品层、模板网络或 UI 固定流程。

### 14.2 Unreal Engine

Unreal Project Browser 将新建/选择项目作为进入 Editor 的前门。可学习的是项目 identity 切换前后边界清楚；不复制其向导页、模板体系或项目生成菜谱。

### 14.3 Godot

Godot Project Manager 明确区分 create/import/open 与进入具体项目。可学习的是 project list/reference 由 Editor owner 管理；不让 MCP Adapter 自建第二套项目 registry。

### 14.4 Bevy

Bevy 本身是代码框架，没有同等级 Editor Project Manager。它说明不能从 ECS/App 启动原语反推完整 Editor lifecycle；本引擎需要由 Native Editor 明确拥有该责任。

### 14.5 本引擎的增量

本引擎不复制这些产品的 UI Workflow，而是在职责分离上增加：

```text
schema-first typed Interface
SafeProjectPath
project identity/root digest
discovery/session binding receipt
stale fail-closed
Native Editor 显式 bounded mutation approval
```

## 15. 兼容与迁移

1. 现有 16 个项目内 typed Tool id/schema 保持不变。
2. 新增 Tool 进入唯一 `AiToolContractRegistry`，不得维护第二份 MCP schema。
3. no-project discovery 必须能发布最小 bootstrap/control surface。
4. 现有项目内 Session 建立方式在兼容期可继续工作，但必须产出等价 binding facts。
5. 既有 Native Editor Create/Open 继续是唯一 Implementation owner。
6. 既有 mutation approval UI 与 Gateway authority 继续是唯一授权真相。
7. 255 Catalog v2 使用新 Tool-owned availability probe，不新增中央状态机。
8. CLI/Test Adapter 可以做等价投影和分层测试，但不能比 production MCP 获得更宽权限。

## 16. 性能

- no-project discovery 不扫描任意磁盘目录；
- recent project lookup 只读 Editor-owned 索引；
- open 的最小识别不提前执行完整 Build/Preview；
- create/open 的长步骤使用 operation observe/cancel，而不是阻塞 MCP transport；
- discovery publication 与 receipt digest 必须确定性；
- request_mutation_access 不编译、不预览、不修改项目；
- availability probe 继续遵守 255 的便宜、只读、无网络/子进程要求。

## 17. 测试与权威证据

### 17.1 Schema/Registry

- 三个 typed Tool 的稳定 id、schema、side effect 和 unknown-field 拒绝；
- internal fields 不能作为 caller input；
- 现有 Tool schema 无漂移。

### 17.2 Project lifecycle 单元与集成

- create/open 成功、路径拒绝、目标冲突、manifest 无效；
- Editor Command 是唯一执行路径；
- project identity/root digest 来自 owner；
- partial failure、cancel 与 cleanup；
- create/open 幂等和并发切换。

### 17.3 Discovery/Session

- launcher/no-project 可发现 bootstrap Tool；
- publication generation 推进；
- exact binding receipt；
- 旧 Session/Grant/receipt stale；
- reconnect 后 project identity/root digest 等价；
- project switch race fail-closed。

### 17.4 Mutation access

- inactive 到 awaiting_user；
- Native Editor approve/reject/expiry/revoke；
- 未批准不能 active；
- 同一请求 transport replay 不重复；
- pending 冲突、项目切换、Session 断开；
- risk/capability 收紧和 bounded goal-level Grant。

### 17.5 Catalog/execute 等价

- no-project 下 create/open 可用性；
- mutation request 和 mutation Tool 的状态区分；
- access/discovery generation 改变 availability digest；
- direct typed call 不能绕过 execute-time guard。

### 17.6 Production MCP conformance

最终权威证据必须来自真实 Native Editor-hosted production MCP：

```text
launcher/no-project discovery
  -> typed project.create 或 project.open
  -> Editor-owned Command receipt
  -> discovery publication
  -> fresh MCP Session exact binding
  -> typed session.request_mutation_access
  -> awaiting_user
  -> Native Editor 用户批准或拒绝
  -> authority status 与 generation 验证
```

测试 JSON-RPC client、CLI 或 headless preflight 只能作为分层回归，不能替代 production MCP conformance。该 conformance 只证明 bootstrap/handshake 合同，不执行项目 mutation、Preview、Build、真实 AI outcome acceptance 或历史 Gate。

进入该 conformance 前，施工 freeze 必须证明真实 production composition，而不是从源码测试推断 binary 能力：

```text
editor_host 使用明确 production profile 与 real-window/实际所需 GPU surface feature 构建
ai_engine_gateway_mcp 与 ai_engine_gateway_codex_config 使用同一 exact commit/profile 构建
三份 executable 均记录 source/sealed/reopen size 与 SHA-256
直接从 sealed editor_host 创建真实顶层窗口，观察 launcher readiness，正常有界关闭并 cleanup
直接从 sealed config helper 对隔离 config 执行 install/replace/conflict/receipt/rollback
config command digest 与 MCP executable content digest 分开记录
receipt 明确 reloadOrNewTaskRequired，rollback 恢复 exact before bytes/digest
```

只有以上证据、同一 exact commit/tree 的有效 P8 Qualification 和 P8 Freeze 全部一致时，freeze 才能声明 `p9Eligible=true`。`--all-features` 测试、默认 feature binary、headless readiness 或配置片段文本均不能替代 sealed production capability smoke。

P8 的施工证据必须分成两个边界，不能再由同一个 one-shot attempt 同时承担基线发现和生产冻结：

```text
P8 Qualification（可重复、只读）
  -> exact clean commit/tree
  -> canonical ledger 与 P7 累计失败集
  -> default/all-features 与全部历史失败矩阵
  -> 环境等价 TEMP/TMP、toolchain、runner digest 与 cleanup
  -> immutable qualification artifact

P8 Freeze（每个 qualification artifact 仅一次）
  -> 只接受仍与 source/commit/tree/runner/toolchain 完全一致的 qualification artifact
  -> production profile/features build
  -> 三 binary seal/reopen
  -> sealed Native Editor real-window/discovery/normal-close
  -> isolated config lifecycle/receipt/rollback
  -> immutable production freeze
```

Qualification 不构建或封印 production composition，不启动真实 Native Editor，不执行 config lifecycle，不读取或修改真实 Codex config，也不能产生 `p9Eligible=true`。测试失败只使本次 qualification 无效；修复形成新 commit 后可以重新获得 qualification 授权，不消耗 Freeze 授权。

Freeze 不重复运行广泛回归矩阵。它只消费一份尚有效且 exact-bound 的 qualification artifact；首次创建 Freeze started marker 才消费单次授权。任何 lineage、source、binary、窗口、配置事务或 cleanup 证据失败都使本次 Freeze terminal failed，不得自动重试。源码、测试、manifest、lockfile、ledger、fixture、Runner 或 toolchain 变化都会使 qualification 失效，必须先生成新的 qualification artifact。

Qualification 与 Freeze 使用不同 schema 和 artifact 名称。施工文档必须为 qualification artifact、freeze artifact、各自 failure artifact 和 SHA-256 sidecar 定义独立发布与 no-clobber 规则；旧 `p8-preflight-*.v2.json` / `p8-candidate-freeze.v2.json` 只保留历史证据，不得被新 Freeze 消费。

fresh freeze 成功后，真实 Codex config activation 只能使用 sealed config helper，并必须保留 before/after digest、exact expected-current command、receipt 与 rollback material。配置发生变化后必须 reload 或新建 Codex task；当前 task 不能声称热加载了新 typed MCP。receipt 落盘失败属于配置事务失败：如果配置已经改变，helper 必须立即尝试 exact rollback，并同时报告 persist/rollback 两个结果。

P8A 必须生成独立的 `p8-config-activation.v3.json` 与 SHA-256 sidecar；它只引用 immutable P8 Qualification、P8 Freeze、三份 sealed binary digest 和 durable config receipt。P8A 不得修改、替换或补写 Qualification/Freeze；activation 失败只能生成独立 failure artifact，不能把 freeze 改成另一种状态。

P8A 完成后的 production conformance 启动顺序必须是：另行授权 P9，先从 sealed path 启动 Native Editor 并等待绑定该 PID 的 launcher discovery ready，再 reload/new Codex task，最后由真实 `ai_first_game_engine` typed MCP 验证 provenance/session。MCP binary 启动时只解析一次 discovery，禁止先开新 task 再补启动 Editor。sealed Editor 的首次启动是 P9 attempt 的首次消费动作；启动或 readiness 失败即本次 P9 失败，不自动重试。

### 17.7 回归

- 受影响 crate 精确测试；
- default/all-features workspace；
- SafeProjectPath、Editor Command、Gateway、Catalog、approval UI 回归；
- config helper failure-atomicity 与隔离 process-level install/replace/rollback 回归；
- sealed `editor_host` real-window 与 launcher readiness/normal-close smoke；
- production composition 与 typed schema 单一真相检查；
- source/isolated clean、cleanup 和证据 digest 规则由当前施工文档冻结。

## 18. 未来施工拆分原则

以下仅是施工设计约束，不是施工文档：

```text
Phase 1：no-project discovery、schema 与红测试
Phase 2：Editor-owned create/open 与 bootstrap receipt
Phase 3：discovery publication、session rebind 与 stale lineage
Phase 4：typed mutation access request 与 Native Editor approval
Phase 5：255 availability/execute guard 集成
Phase 6：可重复 exact-commit qualification
Phase 7：单次 production binary/config freeze
Phase 8：production MCP conformance 与权威回归
```

施工文档必须基于当时 exact clean baseline 重新拆 Gate、核对测试命令并定义失败收敛。不得把以上 Phase 暴露为 AI 使用引擎时的固定调用菜谱。

## 19. 验收标准

### 19.1 Project bootstrap

- AI 在 launcher/no-project 状态能发现并调用 create/open。
- create/open 只经过正式 Editor owner。
- 路径安全、identity、root digest、publication generation 和 receipt 可验证。
- 项目切换后旧 Session 和所有旧 authority lineage 失效。
- partial failure 不被报告为成功 binding。

### 19.2 Mutation handshake

- typed request 只创建 bounded awaiting_user request。
- Native Editor 显示真实 request，并由用户决定。
- 未批准时 mutation 永远不 active。
- approve/reject/expiry/revoke 具有单一 authority generation。
- request receipt 不是 Grant，caller 不能提交内部 grantRef。

### 19.3 AI 自由度

- 没有 `start_task`、Runner、fixed workflow、`nextToolIds` 或后续调用次数。
- Tool 内部严格步骤不成为 Tool 外合同。
- AI 可在 bootstrap 后 inspect、请求 access、停止、向用户询问或选择其它合法 Tool。
- mutation active 后也不自动执行任何项目修改。

### 19.4 Production 真实性

- 真实 Native Editor、discovery 与 production typed MCP 参与最终 conformance。
- `editor_host` 的 exact production feature/profile、MCP/config helper 三 binary digest 与 sealed-path smoke 可核验。
- 隔离 config receipt/rollback 与真实 config activation receipt 可核验，且新/reloaded task 才能消费新 typed MCP。
- Test/CLI/headless 证据不能冒充真实 MCP。
- conformance 到 mutation active/rejected 即停止，不扩张成游戏制作或 outcome acceptance。

## 20. 风险与约束

| 风险 | 约束 |
|---|---|
| 把 bootstrap 做成新 Runner | 只暴露 create/open/request 三个目标；不执行后续业务 Tool |
| Tool 太浅，AI 学内部步骤 | path/Command/publication/rebind/receipt 留在 Module 内部 |
| Adapter 复制项目创建 | Native Editor Command 是唯一 owner |
| 原 Session 偷换项目 | 新 binding identity，旧 Session/Grant/receipt stale |
| approval request 等于授权 | awaiting_user receipt 不是 Grant；只接受 Native Editor 决定 |
| 多个 pending 请求困扰用户 | 幂等、单 pending 冲突规则、清晰 goal/risk/capabilities |
| caller 伪造内部 lineage | schema 拒绝 internal fields |
| create partial state 被误认成功 | transaction/compensation + partial-state diagnostic |
| 255 Catalog 越权做动作 | Catalog 只解释；256 Tool 执行 owner action |
| conformance 又演化成候选验收 | 权威链路止于 access 决议，不运行 mutation/Preview/Build |
| 增加固定下一步提示 | 只返回状态、receipt 和 diagnostic，不返回 next Tool |
| 历史 Gate 被意外恢复 | 明确排除 R1/R2、candidate、F-A/F-B/G 与三引擎 B 通道 |
| 默认 feature binary 冒充 production | freeze 明确 profile/feature，三 binary seal/reopen，并从 sealed editor_host 做真实窗口 smoke |
| 误以为 Codex config 可热加载 | activation receipt 必须声明 reload/new task；只有新/reloaded task 的 typed MCP 可进入 P9 |
| 广泛回归首次在 one-shot Freeze 内发现 flaky | 先生成独立 Qualification artifact；Freeze 不再运行广泛回归矩阵 |
| Qualification 被误当成 candidate/freeze | Qualification 明确 `p9Eligible=false`，不构建/封印/启动 production composition |
| 修复代码后沿用旧 qualification | source/test/manifest/lockfile/ledger/fixture/Runner/toolchain 任一变化立即使 qualification 失效 |

## 21. 方案自审

### 21.1 是否满足用户真实目标

通过。方案解决的是 AI 使用引擎前缺失的项目入口与用户授权握手，使 AI 能从 launcher 正式进入可审计项目 Session；它没有把“可信引擎版本验收”重新塞回 254/256。

### 21.2 是否限制 AI

通过。公共 Interface 只提供目标级能力和安全边界，不提供跨工具顺序、计划或完成判断。AI 仍拥有工具选择与动态重规划。

### 21.3 是否存在工具外菜谱

通过。create/open 内部的 path、Command、publication 和 rebind 是保证单次 Tool 正确性的 Implementation；request 内部的 authority transition 和 UI presentation 是授权正确性。调用者不需要也不能逐步驱动这些 seam。

### 21.4 深 Module 检查

通过：

```text
Interface：3 个目标级 typed Tool。
Implementation：隐藏路径安全、Editor lifecycle、discovery、Session 与 approval 并发。
深度：少量概念承载多个 owner 的组合复杂度。
边界：不吸收项目内 editing/Preview/Build，也不吸收 Catalog。
```

### 21.5 Deletion test

如果删除 `AiProjectAccessModule`：

- Editor 的 Create/Open UI 仍可独立存在；
- Gateway 的 Session/Grant authority 仍可独立存在；
- 255 Catalog 仍可解释项目内 Tool 不可用；
- 但真实 AI 无法从 no-project typed MCP 安全进入项目并发起 mutation access。

删除 Module 不要求删除 Editor lifecycle、SafeProjectPath、Gateway authority 或 Catalog，说明它是组合这些 owner 的独立深 Module，而不是重复实现。

如果删除任何一个内部 owner 并把逻辑复制进 Adapter，安全、identity 或 authorization 就会出现第二真相，因此禁止这种退化。

### 21.6 与既有系统是否重叠

通过：

- 249：launcher deterministic state/isolation，256 提供 typed 出口。
- 251：验证 from-blank creation，256 提供正式 AI Interface，不复制 Golden Gate。
- 253：AI-owned planning 和 bounded Grant 原则，256 补授权请求前门。
- 254：Gateway/typed MCP/session authority，256 增加窄 public projection。
- 255：解释 availability，256 执行 create/open/request。

### 21.7 安全与失败闭环

通过。方案规定 path containment、owner identity、stale fail-closed、bounded approval、幂等、并发决议、partial-state diagnostic 和真实 production MCP 证据；同时禁止自动重试和自动审批。

### 21.8 复杂度是否合理

通过。复杂度来自真实跨 owner 一致性，集中在一个 Module 内；公共认知负担只有三个目标。没有引入产品或 AI-facing 的候选生产、release sealing、activation/attempt 或 outcome evaluator。

本节的 Qualification 与 production binary/config Freeze 都是施工证据合同，不是新的 public Module、AI-facing Tool、release candidate producer 或固定工作流。它们不改变三个公共 Tool，也不恢复已退役的 254-R1/R2 lifecycle。拆分只改变“何时消费昂贵的一次性授权”，不减少最终测试、production composition、真实窗口、配置事务、typed MCP 或 Local CI 证据。

施工文档可以为一次性环境证明提供单一 repository-owned construction runner，以消除临时 wrapper 漂移；该 runner 只能编排测试、binary seal、isolated config 与证据，不得进入 Registry、Catalog、MCP surface 或用户项目工作流。

## 22. 最终结论

256 正式采用方案 B：

```text
launcher / no project
  -> project.create 或 project.open
  -> Editor-owned CreateProject/OpenProject
  -> discovery publication + fresh Session binding
  -> session.request_mutation_access
  -> awaiting_user
  -> Native Editor 用户批准/拒绝
  -> mutation active 或 terminal denial
```

工具内部拥有保证正确性的严格步骤，工具外没有菜谱。256 只让 AI 获得好用、自由、可审计的项目入口与授权握手，不替 AI 制作游戏，也不证明真实 AI outcome acceptance。

P8S1 已将 P8 拆为可重复 Qualification 与单次 Freeze。下一步只能按施工文档单独授权 lifecycle synchronization remediation；当前不得重跑任何历史或一体化 fresh P8。lifecycle 修复、稳定性资格证明、construction Runner 分流、fresh P8 Qualification、P8 Freeze、P8A、P9、P10 必须分片并分别授权。
