# 259 External Codex Authoring Readiness: Connection Recovery + Deep Mutation Contract v1 方案

## 1. 文档状态

```text
系统编号：259
方案版本：v1
讨论结论：方案 B-min 已确认
当前状态：正式方案已生成
施工文档：已激活并位于当前队列；C7-R2 修复切片已生成并自审
施工授权：无
```

本文档只定义长期产品与架构合同，不是施工文档，也不授权修改源码、迁移
Codex 配置、运行 Gate 或执行真实 mutation。

下一步只能是：用户单独授权当前施工文档第 31 节 C7-R2-A source/test。该授权不自动覆盖
production replacement、真实 mutation、rollback 或 Local CI。每个授权窗口必须满足单次
施工不超过 3 小时的硬限制。

## 2. 一句话目的

让用户自己的外部 Codex 能稳定连接当前 Native Editor，并通过少量、目标级、
可审计且可回滚的 typed tools 自由修改当前项目，而无需理解 Candidate、
Grant、digest、generation 或 Editor 内部生命周期。

## 3. 产品决定

259 只推进外部 Codex 路线，内置 AI 继续搁置。

```text
Full Agent：用户自己的外部 Codex
Engine：提供好用、自由、可审计的 typed tools
Native Editor：项目事实、授权、mutation 与结果呈现 owner
```

259 不新增 Planner、Workflow、Runner，也不负责证明某个精确引擎版本通过
真实 AI outcome acceptance。AI 决定做什么和如何组合工具；引擎只保证单个
工具内部的安全步骤、身份绑定、原子修改、诊断与回滚。

这满足“工具内可以有步骤，工具外不能要求 AI 遵循菜谱”的产品原则。

## 4. 已确认的真实问题

### 4.1 Codex 仍绑定 frozen candidate 二进制

当前用户配置中的 `mcp_servers.ai_first_game_engine` 仍指向历史 frozen candidate：

```toml
[mcp_servers.ai_first_game_engine]
command = '<LOCAL_TEST_ROOT>\GateF254R1Frozen\candidate-20260720-000200\bin\ai_engine_gateway_mcp.exe'
```

这使外部 Codex 的长期入口依赖一次性验收产物，而不是稳定安装产物。旧二进制
使用 discovery v1；当前 Editor 使用 discovery v2，实际会产生
`gateway.discovery.record_parse_failed`。

### 4.2 mixed discovery 会阻断唯一有效 Editor

当前 discovery root 同时存在死进程遗留的 v1/v2 记录和一个 live v2 记录。
当前 v2 MCP 遇到旧 v1 记录会返回 `gateway.discovery.schema_unsupported`，
因此唯一健康 Editor 也无法连接。

### 4.3 mutation 授权链没有接入 production Native Editor

Gateway 已有 mutation access request，Native Editor 也已有授权应用 helper，
但 production composition 没有完成调用闭环。没有 active Grant 时 mutation
会正确拒绝，却没有让真实用户在 Native Editor 中批准同一 operation 的生产路径。

### 4.4 Candidate 输入暴露内部所有权

现有 `project.mutate.candidate` 要求 caller 理解并提供 revision、digest、
generation、context hash、validation、Grant/operation/receipt 等内部事实。
这既降低 AI 可用性，也把 stale、伪造和不一致风险推给 caller。

### 4.5 public lifecycle 过多会形成外部菜谱

若把 request access、prepare、validate、apply、resume、receipt 等阶段拆成多个
公共 tool，AI 必须记住固定顺序并搬运内部身份。复杂性没有消失，只是泄漏到了
每个 caller 和每个 prompt。

### 4.6 当前证据锚点

本方案讨论阶段已核实以下实现与运行证据：

```text
Codex config：
<CODEX_HOME>\config.toml

旧 frozen MCP：
<LOCAL_TEST_ROOT>\GateF254R1Frozen\candidate-20260720-000200\bin\ai_engine_gateway_mcp.exe
SHA256 E20C31559B94490020CEF18E045F4D97574970C570421D6CC0E13239E40DCC94

当前 debug MCP：
<repository-root>\rust\target\debug\ai_engine_gateway_mcp.exe
SHA256 A0BE25346A71CF44C2CEEF77875208FCD7F39D1630C3219808AD7F223FE3B74A

当前 production Editor process：
<repository-root>\rust\target\debug\editor_host.exe
PID 9800
SHA256 7FAF55B6C4E9BA6A004FCE0F8E51D0FA6C98ACC76B5CCB873C72FD1B0700B968

discovery owner：
rust/crates/ai_tool_gateway/src/discovery.rs

Gateway mutation access request：
rust/crates/ai_tool_gateway/src/core.rs:267

缺失 active Grant 的拒绝：
rust/crates/ai_tool_gateway/src/core.rs:1257

Native Editor authorization helper：
rust/crates/editor_window_winit/src/application.rs:1435

过度暴露的 Candidate schema：
rust/crates/editor_core/src/ai_capability_tool_kernel.rs:4383

既有 Candidate ownership 正式规则：
254-AI-Tool-Gateway-Codex-Adapter-v1方案.md:1400
```

这些行号和运行 PID 只用于锁定方案形成时的事实，不是未来施工可直接复用的
baseline。施工文档必须重新核对当前 commit 与代码位置，不能把本节当作冻结授权。

## 5. 成熟引擎与工具对照

### 5.1 Unity

Unity 的 Editor connection、asset database、Undo 与 serialization 都由 Editor
持有。外部 automation 通常提交高层命令，不负责拼装 Editor 内部 revision
和 transaction identity。可借鉴的是 Editor-owned context、Undo/rollback 与
单一活动实例；不照搬 Unity 专用菜单、C# scripting 或固定用户操作序列。

### 5.2 Unreal Engine

Unreal 的 Editor scripting、transaction 与 asset tools 把多步实现封装在
Editor-owned operation 中。可借鉴的是 transaction、fail-closed identity 和
目标级工具；不照搬 Blueprint/C++ 双轨或大型 subsystem hierarchy。

### 5.3 Godot

Godot 的 EditorPlugin、EditorInterface 与 UndoRedoManager 把当前 Editor 与场景
状态留在引擎侧。可借鉴的是窄 Interface 和 Editor-owned Undo；不照搬节点脚本
模型或插件生命周期。

### 5.4 本项目结论

成熟引擎共同证明：外部自动化不应拥有 Editor 内部事实。259 应深化现有
Gateway，而不是建立独立 Agent orchestration 平台。

## 6. 方案比较与选择

### 6.1 方案 A：只修连接

只安装稳定 MCP 并容忍 stale discovery，不改变 Candidate public contract。

优点是改动最小；缺点是 AI 仍需理解大量内部字段，连接恢复后依然不好用。

### 6.2 方案 B：连接恢复 + 深 mutation

修复稳定入口与 discovery，同时以目标级 mutation Interface 隐藏内部事实。

完整 B 可能引入 journal、heartbeat、独立 lifecycle tools 和长期 operation
replay，能力完整但超过当前真实需求。

### 6.3 方案 C：外部 Agent 平台

新增 Planner、Workflow、Runner、persistent journal、replay 与内置任务调度。

它会重新限制 AI 的工具组合方式，扩大状态空间，并重复 254-R1/R2 已证明昂贵的
验收基础设施，因此拒绝。

### 6.4 正式选择：B-min

B-min 只解决两个已证实的产品阻塞：

1. 外部 Codex 能稳定连接唯一兼容的当前 Editor。
2. 外部 Codex 能用一个目标级 mutation tool 完成可批准、可审计、可回滚的修改。

不为假设需求预建通用 Agent 平台。

## 7. 正式结构

259 只涉及两个 ownership area：

```text
EditorInstanceGatewayModule
  - 稳定 MCP Adapter 入口
  - Editor discovery、选择、连接与断开
  - typed tool transport
  - operation observation
  - production Native Editor authorization bridge

GoalMutationModule（现有 Gateway / Tool Kernel ownership 内部）
  - caller intent normalization
  - Editor/project fact binding
  - engine-derived risk 与 goal-level Grant
  - internal Candidate preparation/validation/apply
  - receipt、digest、generation 与 rollbackRef
```

`GoalMutationModule` 是内部深 Module，不是新的公共 lifecycle Module。它的复杂
Implementation 通过窄 Interface 提供高 leverage，并把变化集中在 owner 内部。

## 8. 稳定 MCP 安装合同

### 8.1 稳定入口

Windows 安装产物建议固定为：

```text
%LOCALAPPDATA%\AiFirstGameEngine\bin\ai_engine_gateway_mcp.exe
```

该 executable 直接承载 MCP Adapter，不再启动额外 bootstrap forwarding process。
安装路径属于产品 installation ownership，不属于源码 `target/`、测试 candidate、
fixture 或单次 Gate artifact。

### 8.2 一次性配置迁移

迁移工具只能修改：

```text
mcp_servers.ai_first_game_engine
```

迁移必须：

1. 校验当前 command 精确等于预期历史值。
2. 保留全部其它 Codex 配置与格式可解析性。
3. 先生成备份，再原子替换。
4. 生成包含 before/after/hash/backup/rollback 的结构化 receipt。
5. 任一前置漂移时 fail closed，不猜测、不覆盖。
6. 明确提示用户需要 reload 或创建新的 Codex task 才能加载新 MCP。

迁移不是每次启动流程，也不自动编辑未知用户配置。

## 9. Discovery recovery

### 9.1 有界分类

每条 discovery record 只能被分类为：

```text
active_compatible
active_incompatible
dead_stale
invalid_or_security_violation
```

分类必须在有界读取、schema 检查、path ownership、进程身份和 Editor instance
验证后完成。

### 9.2 选择规则

```text
恰好一个 active_compatible：选择并连接
多个 active_compatible：ambiguous，禁止猜测
只有 active_incompatible：返回升级/版本诊断
没有 active_compatible：返回 Editor 不可达诊断
```

regular、engine-owned 且 PID 已死亡的 legacy record 不得阻断唯一
`active_compatible` Editor。

### 9.3 安全规则

symlink、reparse point、oversize、非 owner 文件、异常路径、活进程身份冲突或
无法证明安全的记录必须 fail closed。dead stale record 只允许在同时证明
engine ownership 与 dead process 时 best-effort 删除。

不新增 quarantine 目录、quarantine database 或 discovery GC Module。

## 10. `project.mutate` Interface

### 10.1 public replacement

259 以替换方式收敛 public tools：

```text
project.mutate.candidate -> project.mutate
project.rollback.candidate -> project.rollback
```

旧名称不作为永久 alias 保留。兼容窗口、deprecation diagnostics 和最终移除
由未来施工文档根据 catalog consumer 范围明确，不能形成双重长期真相。

### 10.2 caller-owned input

建议的最小输入：

```json
{
  "schemaVersion": "external-project-mutation-intent.v1",
  "goal": {
    "outcome": "让玩家按空格发射子弹"
  },
  "change": {
    "kind": "project_patch",
    "payload": {}
  }
}
```

caller 只拥有：

```text
goal.outcome
change.kind
change.payload
```

`goal.outcome` 描述用户期望，不是步骤清单。`change.payload` 是真实 change
contract，不是 Candidate lifecycle envelope。

### 10.3 engine-owned facts

以下字段禁止由 caller 补写：

```text
editor/session/project identity
project root
base digest
read generation
project patch context hash
candidate/revision/store/source kind/source label
validation environment
risk classification
Grant identity
operation identity
receipt identity
```

这些事实由 Gateway 在调用时从同一 Editor session 读取、绑定、复核和写入内部
Candidate。缺失或漂移时返回 typed diagnostic，不要求 AI 猜字段。

### 10.4 output

成功结果至少包含：

```text
operationRef
receiptRef
newProjectDigest
newReadGeneration
rollbackRef
compact change summary
diagnostics
```

结果必须足以继续 inspect、preview、build 或 rollback，但不得泄漏下一次调用
需要回填的内部生命周期字段。

## 11. Goal binding、engine-derived risk 与 Grant reuse

### 11.1 goal binding

Module 对 `goal.outcome` 做确定性 normalization。caller 不提供 `goalId`。

goal binding 至少覆盖：

```text
normalized goal outcome
Editor instance
MCP session
project identity
risk class
approved budget
```

### 11.2 risk ownership

risk 由引擎根据实际 change、影响范围、project ownership 与 capability policy
派生。caller 不提供 `riskIntent`，也不能通过声明 low risk 绕过授权。

### 11.3 bounded goal-level Grant

同一精确 normalized goal outcome、session、project、risk 与 budget 可复用仍有效
的 goal-level Grant，支持 AI 在同一目标下做多个低风险迭代。

以下任一变化都禁止复用：

```text
goal outcome 改变
Editor/session/project 改变
risk 升级
budget 耗尽
Grant 过期、撤销或拒绝
project facts 无法重新绑定
```

Grant reuse 是 Module 内部能力，不要求 caller 传 Grant ID 或执行固定菜谱。

## 12. 同一 operation 的批准合同

一个逻辑 mutation 必须保持同一 operation：

```text
project.mutate
  -> same operation: awaiting_user
  -> 用户在 Native Editor 批准一次
  -> same operation: 重新读取并验证 project facts
  -> internal Candidate prepare / validate / apply
  -> receipt + digest + generation + rollbackRef
```

等待期间可使用现有 `aife_observe` 查看同一 operation，不新增：

```text
request_access
resume_mutation
continue_mutation
apply_candidate
approval_status
```

Native Editor 必须明确显示 goal、目标项目、风险、预算与 operation identity。
一次批准只绑定该 goal-level Grant 范围，不是全局永久授权。

## 13. 最小 operation states

公共观察只需要稳定表达：

```text
running
awaiting_user
succeeded
failed
cancelled
```

内部可以有 prepare、validate、apply、receipt 等阶段，但它们不是 caller 必须
驱动的公共状态。typed diagnostics 可附带内部 stage，用于定位，不形成新工具。

## 14. 断连与终态边界

以下情况必须 terminal，且不得自动重试 mutation：

```text
用户拒绝
批准 TTL 到期
Editor/session/project drift
用户取消
MCP disconnect
Editor process 退出或替换
security/ownership violation
apply 结果无法形成可信 receipt
```

断连后 caller 可以重新发现当前 Editor 并发起一个新的显式 mutation，但旧
operation 不跨 task、跨 session replay，也不暗中继续 apply。

## 15. 内部 Candidate 合同

Candidate 机制继续作为内部安全实现存在，用于：

```text
绑定 base facts
构建 deterministic revision
验证 change
原子 apply
生成 receipt lineage
提供 rollback material
```

公共 `project.mutate` Adapter 将 caller intent 与 engine-owned facts 组合为内部
Candidate direct input。所有内部字段必须来自同一 operation、同一 session 和
同一次事实读取；禁止从 caller payload 透传同名字段。

Candidate preparation、validation 与 apply 只允许成为 Module 内部步骤，禁止
再次拆成公共 typed tool 菜谱。

## 16. `project.rollback(rollbackRef)` Interface

caller 只提交引擎先前返回的短引用：

```json
{
  "schemaVersion": "external-project-rollback.v1",
  "rollbackRef": "rbk_..."
}
```

Module 负责解析并验证：

```text
receipt lineage
Editor/session/project identity
current digest/read generation
rollback material ownership
TTL 与一次性/重复调用语义
```

成功返回新的 project digest、read generation、rollback receipt 与 diagnostics。
caller 不提交完整 receipt，不搬运 inverse patch，不提供期望 digest。

若项目已经继续变化，rollback 必须 fail closed，并返回 conflict diagnostics；
不得覆盖后来修改。

## 17. Adapter ownership

MCP Adapter 只负责：

```text
typed schema decode/encode
Gateway Interface 调用
operation observation transport
typed diagnostic mapping
```

它不拥有 discovery policy、risk policy、Candidate construction、Grant reuse、
project apply 或 rollback logic。删除 Adapter 后，这些复杂性仍应集中在两个
ownership area 内，而不是散落到 transport caller，符合 deep Module 删除测试。

## 18. 明确禁止的过度设计

259 不新增：

```text
独立 bootstrap forwarding process
discovery quarantine directory/database
persistent operation journal
cross-task or cross-session replay
MCP heartbeat protocol
caller-owned goalId
caller-owned riskIntent
额外 public lifecycle states
Candidate prepare/validate/apply public tools
Planner / Workflow / Runner
真实 AI outcome acceptance Gate
精确 release candidate freezer
```

若未来出现真实、重复且无法由现有两个 ownership area 吸收的需求，必须另开
方案讨论，不能在 259 施工中顺手恢复。

## 19. 与 253-256 的关系

```text
253：继续提供 capability-first typed Tool Kernel；259 不恢复 Agent-owned planning。
254 Core：继续定义 Gateway/Codex Adapter、审计与安全原则；259深化真实可用性。
254-R1/R2：仅历史证据；不恢复 Candidate acceptance lifecycle。
255：继续提供 capability-aware Tool Catalog；259替换旧 Candidate public entries。
256：继续提供 Editor-instance Gateway lifecycle 与极简 project.create；
     259深化稳定安装、discovery recovery 和已有项目 mutation。
```

259 不推翻 256 的 Editor-instance identity：项目仍是连接内可选、可变化的
context，mutation 开始时再绑定当前精确项目事实。

## 20. 与历史 254-R1/R2 隔离

259 禁止复用历史 frozen candidate、F-A/F-B/G、三引擎 B 通道、R2 Candidate
activation、Real Evaluation、production candidate freezer 或真实 Codex
outcome acceptance 的 artifact、授权与 attempt。

历史失败只提供两个教训：

1. 不把发布候选验证变成 AI 日常工具的前置菜谱。
2. 不让 caller 拥有应由 Editor/Gateway 推导的内部事实。

259 的稳定 MCP installation 是产品安装合同，不是新的 frozen candidate。

## 21. Diagnostics、安全与性能

### 21.1 diagnostics

所有失败必须给出稳定 code、stage、owner、可读 message 与可执行 next action。
至少覆盖：

```text
stable executable missing/incompatible
discovery ambiguous/no compatible Editor
invalid or security-violating discovery record
no project open
authorization rejected/expired
project drift
validation failed
apply failed
rollback conflict
disconnect/cancel
```

### 21.2 安全

所有文件路径继续受 `SafeProjectPath`、project ownership、symlink/reparse 防护和
原子写入合同约束。`change.payload` 不能成为绕开 capability policy 的任意文件
写入口。

### 21.3 性能

discovery 必须有界，不轮询整个磁盘。等待批准不占用热循环；使用已有 operation
observation。digest/context 只在 mutation 的正确性节点计算，不因 259 新增常驻
全项目扫描、heartbeat 或 journal flush。

## 22. 结果导向的验证原则

未来施工验证只证明受影响结果，不建立固定测试菜谱：

```text
稳定安装 command 能被真实 Codex MCP 加载
mixed stale discovery 下能选择唯一 compatible Editor
多 live Editor 时 fail closed
project.mutate 的 caller schema 不接受内部 owner 字段
同一 operation 能 awaiting_user -> approval -> terminal
Grant reuse 严格受 goal/session/project/risk/budget 约束
成功 mutation 产生可信 digest/generation/receipt/rollbackRef
project.rollback 只凭 rollbackRef 且不会覆盖后续 drift
旧 public Candidate entries 完成明确替换
受影响 consumer 与一次 production typed MCP smoke 可证明真实 composition
```

同一配置下，上层验证已覆盖的定向测试不得机械重复。production smoke 只用于证明
真实 composition，不承担完整回归。任何高成本验证必须 fail-fast。

## 23. 建议的未来施工切片

以下只是后续施工文档的候选切片，不是当前施工授权：

```text
S0 stable MCP command + one-time config migration
S1 tolerant secure discovery recovery
S2 project.mutate caller-owned schema + internal fact binding
S3 same-operation goal approval + Grant reuse
S4 project.rollback short ref + old public Candidate contract retirement
S5 affected-domain regression + one production typed MCP smoke
```

施工文档可根据真实依赖合并或进一步拆小，但不得把这些切片转成 AI 必须遵循的
公共工具顺序。任何单次授权窗口不得超过 3 小时。

## 24. 方案验收标准

259 只有同时满足以下条件才可视为实现完成：

1. Codex 配置不再绑定 frozen candidate 或源码 build target。
2. stable installation executable 直接承载 MCP Adapter。
3. 唯一 compatible Editor 不被安全可判定的 dead stale record 阻断。
4. 多 live Editor、incompatible Editor 与安全违规记录均 fail closed。
5. production Native Editor 存在真实、同 operation 的一次批准路径。
6. `project.mutate` caller 只提供 goal outcome 与真实 change payload。
7. risk、facts、Grant、Candidate、operation 与 receipt identity 全部 engine-owned。
8. bounded goal-level Grant 可支持同目标低风险迭代，不扩大授权。
9. reject/TTL/drift/cancel/disconnect 都 terminal 且不自动重试。
10. mutation 成功产生新 digest、read generation、receipt lineage 与 rollbackRef。
11. `project.rollback` 只接受 rollbackRef，并对后来 drift fail closed。
12. 旧 Candidate public Interface 被替换，不形成永久双入口。
13. 没有新增 Planner、Workflow、Runner、journal、heartbeat 或 acceptance freezer。
14. 验证覆盖受影响结果，没有恢复历史 254-R1/R2 Gate。

## 25. 设计自审

### 25.1 是否限制 AI

否。AI 仍可自由选择 inspect、search、mutate、preview、diagnose、build 和 rollback
的组合与顺序。259 只把单个 mutation 内部必须原子且安全的步骤封装起来。

### 25.2 是否形成工具外菜谱

否。批准是同一 `project.mutate` operation 的状态，不是 caller 必须调用的一串
lifecycle tools。Candidate prepare/validate/apply 全部留在内部。

### 25.3 Module 是否过多

否。方案深化现有 `EditorInstanceGatewayModule`，并只在现有 Gateway/Tool Kernel
ownership 内增加一个内部 `GoalMutationModule`。没有增加两个新的公共生命周期
Module。

### 25.4 Interface 是否足够深

是。两个公共 mutation Interface 隐藏 discovery、facts、risk、Grant、Candidate、
validation、apply、receipt 和 rollback lineage。caller 学习成本远小于实现能力。

### 25.5 是否过度设计

已做删减。bootstrap、quarantine、journal、replay、heartbeat、caller-owned identity
和多 public lifecycle tools 均明确删除。剩余内容均对应已经复现的连接或 mutation
阻塞。

### 25.6 是否可审计和可回滚

是。operation/receipt/digest/generation/rollbackRef 保留审计链，同时不要求 caller
拥有内部字段。rollback 对后续 drift fail closed。

### 25.7 是否偏离 254 Core

否。259 落实 254“好用、自由、可审计的引擎工具”定位，并保持 254-R1/R2 历史
acceptance lifecycle 退役，不再让 release candidate 验收支配日常 authoring。

### 25.8 是否已经允许施工

否。施工文档虽已单独生成并自审，但仍位于待执行队列。必须完成激活前复核并获得
明确施工授权后，才可修改代码或外部配置。

## 26. 最终结论

采用方案 B-min：

```text
稳定安装并恢复外部 Codex 到唯一兼容 Native Editor 的连接；
深化现有 Gateway，并以内部 GoalMutationModule 承担 mutation 复杂性；
公共面只保留目标级 project.mutate 与 rollbackRef 级 project.rollback；
同一 operation 内完成一次 Native Editor 批准、事实复核、apply 与 receipt；
不新增任何外部菜谱、Agent orchestration 平台或真实 AI acceptance 系统。
```

这使外部 Codex 路线首先变得真实可用，而不是继续扩张系统数量。

## 27. 2026-07-28 production 多连接恢复修订

Window G / C7 的 production read-only handshake diagnosis 推翻了“只需继续重建 Codex
task”这一恢复假设。稳定安装的 Gateway 在另一个 Gateway 已连接 production Editor 时，
于 `connect_from_discovery` 阶段以 Windows `ERROR_PIPE_BUSY`（os error 231）退出，尚未读取
MCP `initialize`。根因是 Editor Named Pipe 以 `CreateNamedPipeW(..., 1, ...)` 创建，accept
thread 又在整个 peer 长连接期间同步执行 `run_connection`；因此系统只支持一个长连接 client，
现有顺序 reconnect 测试不能证明 app-level connection manager、task connection 或诊断连接
并存时的 production composition。

正式修订如下：

```text
EditorInstanceGatewayModule 的同一 unpredictable pipe locator 必须支持多个本机当前用户连接实例。
每个 pipe peer 独立拥有 ClientSessionBinding，禁止跨 peer dispatch/close 其它 session。
所有 peer 请求仍进入同一个 GatewayOwnerThreadDispatcher；不得并发直接访问 EditorSession。
accept owner 必须持续 re-arm 新实例，不得因一个长连接阻塞后续 client。
client 遇到 re-arm 窗口内的 `ERROR_PIPE_BUSY` 时必须有界等待，不得无限重试或立即误判 endpoint 失效。
每个 connection worker 必须在 EOF、Close、server shutdown 或异常时关闭自己的 session。
server shutdown 必须有界地取消 accept 与全部 owned connection worker，并 join 后返回。
单个 malformed/closed peer 不得终止其它 peer 或整个 accept owner。
```

本修订不新增 Gateway Module、heartbeat、persistent journal、跨任务 replay 或公共 Tool。
它只修复现有 Named Pipe Adapter 的 production composition 和恢复能力。验证必须新增：

1. 两个真实 `GatewayRemoteAdapter` 同时保持连接并各自完成 Catalog dispatch。
2. 两个 peer 获得不同 `clientSessionId`，且现有 exact-session owner 检查继续通过。
3. 一个 peer 关闭后另一个仍可 dispatch；server shutdown 对 accept/worker 均保持有界。
4. 原单客户端、顺序 reconnect、EOF cleanup 与 MCP process smoke 全部继续通过。

只有该修订实现、受影响回归和新的 stable binary/production Editor replacement 分别获得对应
授权并完成后，才可重新进入 C7 typed MCP smoke。

## 28. 2026-07-28 Scene Clean Save / Approval Drift 修订

### 28.1 现场结论

Window G / C7 的唯一真实 mutation 在 `awaiting_user` 期间以
`gateway.operation.project_drifted` terminal failed。该 operation 没有开始 commit，也没有
创建目标 Input 文件。漂移来自普通 Scene 保存路径对兼容旧格式的
`Scenes/Main.scene.json` 做无语义规范化写回。

仓库外副本上的 owner 复现得到与 production 完全相同的摘要变化：

```text
before：
sha256:6757c0b0abaf9a069b164beae90bc7f0b7301ea0092ce5469634a961060291d1

first clean save：
sha256:2647529fb9e319bd7050c9c9cfc68210f38cb26614c06f0ac7b22ee99b0e6b0b

second clean save：
sha256:2647529fb9e319bd7050c9c9cfc68210f38cb26614c06f0ac7b22ee99b0e6b0b
```

首个保存把兼容输入 `data` 规范化为 `fields`，补齐默认字段并改变 JSON 数值/排版；第二次
保存字节稳定。Gateway 的 raw-byte project digest 与 approval 前 fail-closed recheck 均按
现有安全合同正确工作。根因属于 Scene clean save owner，而不是 Gateway drift policy。

### 28.2 正式选择

采用 Scene owner 内的 clean-save no-write 合同，不放宽 Gateway：

```text
same current path + document dirty=false + target still exists
  -> SceneSaveStatus::Unchanged
  -> no serialization
  -> no atomic replace
  -> no file write-set
  -> bytes / mtime / project digest unchanged

dirty document or missing current target
  -> serialize and compare current bytes
  -> equal bytes: clear dirty, return Unchanged, no replace
  -> different bytes: atomic replace, clear dirty, return Saved

explicit Save As to a different target
  -> write the target even when the source document is clean
```

`SceneSaveStatus` 增加 `Unchanged`。`Saved` 只表示真实文件写入，`Unchanged` 表示命令成功但
没有写盘，`Failed` 保持原义。Play autosave 和其它 consumer 必须把 `Unchanged` 视为成功，
但不得报告实际保存或声明文件 write-set。

### 28.3 Gateway 边界保持不变

Gateway 继续比较 approval request 捕获的 raw-byte project digest 与当前 digest。任何真实
项目变化，包括 Editor 内真实 dirty save，仍必须使 awaiting mutation terminal
`gateway.operation.project_drifted`。禁止增加以下例外：

```text
Editor-owned write 自动可信
approval request 自动刷新 digest
terminal mutation 自动重试
JSON semantic digest 替换 whole-project raw-byte digest
按变更文件与 mutation domain 猜测无冲突
```

写入来源不能证明语义无冲突；允许 Editor-owned write 绕过 recheck 会使用户批准的旧事实
失效。未来可以增加 owner/write receipt 改善诊断，但它不能替代重新审批，也不属于本修订。

### 28.4 验证合同

施工至少证明：

1. legacy-compatible Scene 的 clean same-path save 不改变 bytes、mtime 或 project digest。
2. clean save 返回 `Unchanged`，不声明 Scene 文件 write-set。
3. dirty save 仍写盘、清 dirty，并推进 project digest。
4. dirty 但序列化结果与磁盘相同时不 replace，仍清 dirty。
5. clean Save As 创建不同目标；缺失的当前文件可由 Save 重建。
6. awaiting mutation 期间 clean save 不使 approval stale。
7. awaiting mutation 期间真实 dirty save 仍 terminal `gateway.operation.project_drifted`。
8. approval UI dispatch 不隐式派发 `SaveSceneDocument`。

该修订不恢复已失败的 operation，不授权 production replacement、真实 mutation、rollback、
Codex config 或 Local CI。后续必须按施工文档中的独立窗口重新取得授权。
