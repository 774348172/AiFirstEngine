# 256 Editor-Instance Gateway Lifecycle + Minimal Project Create v1 方案

## 0. 文档状态

```text
系统编号：256
方案版本：v1 方案 B
方案状态：已确认、已完成施工并归档
确认日期：2026-07-25
正式 Module：EditorInstanceGatewayModule
公共新增 Tool：仅 project.create
施工归档：施工文档/已完成/256-当前可自动化施工文档-Editor-Instance-Gateway-Lifecycle-Minimal-Project-Create-v1.md
施工文档状态：已完成并归档
完成记录：阶段完成记录/2026-07-26-Editor-Instance-Gateway-Lifecycle-Minimal-Project-Create-v1/00-总览.md
最终资格：production typed MCP smoke passed；Local CI run local-e99b5af45476-1785050176 12/12 passed
```

本方案取代 `256-Project-Create-Tool-v1方案.md` 的极简 Tool-only 设计。旧设计的 F0 结论 `deferred_not_cost_effective` 保留为历史证据：它证明当前 project-bound Gateway 无法在 launcher/no-project 状态承载 Tool，不证明本方案 B 不成立。

旧三工具实现、显式 rebind/reconnect、access lineage、P8/P9/P10、candidate/freezer 和专用 Runner 仍已退役，不因本方案恢复。

## 1. 一句话目的

让 AI 连接到一个精确的 Native Editor 进程实例；无论 Editor 处于 launcher 还是已打开项目，该连接都保持稳定，而项目只是连接内部可选、可变化的上下文。AI 可在 launcher 中调用极简 `project.create`，成功后沿用同一连接继续发现和使用项目工具。

## 2. 问题与 F0 证据

当前 Gateway 把“连接身份”和“项目身份”绑定在一起：

```text
no active project
  -> no Gateway host/discovery/session binding

project opens or changes
  -> old project-bound connection cannot naturally evolve
```

F0 已确认：

| 判定 | 结果 |
|---|---|
| no-project Gateway production composition | fail |
| 非项目 operation identity 复用 | fail |
| Native Editor CreateProject owner 局部加固 | pass |
| 既有 production typed MCP launcher smoke | fail |

历史证据：

```text
施工文档/历史/256-已暂缓施工文档-Minimal-Project-Create-Tool-v1.md
阶段完成记录/2026-07-25-Minimal-Project-Create-Feasibility-v1/00-总览.md
```

根因是 Gateway 身份层级错误，不是 `project.create` 需要更多公开生命周期步骤。本方案通过改变连接 owner 修复根因。

## 3. 成熟引擎对标

Unity Hub、Godot Project Manager 和 Unreal Project Browser 都证明：项目管理身份先于项目身份，项目创建 owner 与项目编辑 owner 可以分离。

本地源码证据：

```text
Godot editor/project_manager/project_manager.cpp
  Project Manager 创建/选择项目后启动 Editor。

Godot main/main.cpp
  未指定项目时进入 Project Manager。

Unreal Engine GameProjectGeneration/Private/SProjectDialog.cpp
  GameProjectUtils::CreateProject 后再 OpenProject。
```

本方案吸收：

```text
项目管理上下文可以先于项目存在
创建 owner 与项目编辑 owner 分工
创建成功后进入项目编辑上下文
```

不照搬：

```text
不把 Godot 的进程重启变成 AI reconnect 菜谱
不引入 Unity Hub 的独立产品部署复杂度
不引入 Unreal 的模板、平台和配置广度
```

## 4. 产品价值

本方案不是为了让文件写入更快，而是让 AI：

1. 只连接一次精确 Native Editor 实例。
2. 在无项目状态仍能发现真实可用能力。
3. 用两个字段创建正式默认项目，不手写工程骨架。
4. 创建完成后不需要理解 discovery、rebind 或 reconnect。
5. 对已有项目继续获得 identity/digest、Grant、operation 和 receipt 的审计保护。

## 5. 范围与非目标

### 5.1 本方案必须实现

```text
稳定 Editor 实例 discovery 与连接
LauncherContext / ProjectContext 内部状态
同一连接跨越 launcher -> project.create -> opened project
上下文变化后的 Catalog availability 自动更新
现有项目工具的 identity/digest guard
极简 project.create(requestedProjectRoot, projectName)
上下文变化时旧 project-scoped Grant/operation 失效
多 Editor 实例歧义的 typed diagnostic
```

### 5.2 本方案不实现

```text
project.open
session.request_mutation_access
public session.rebind
reconnectRequired
AI-facing lifecycle recipe
模板、renderer、language、starter content 选择
caller-owned idempotencyKey
新的 project mutation Grant
Preview/Build/Export 自动串联
Workflow、Planner、nextToolIds 或目标状态机
candidate、freezer、activation、真实 AI outcome acceptance
256 专用长期 Runner 或独立 qualification Gate
旧 P8/P8Q/P8F/P8A/P9/P10
```

## 6. 深 Module 与 seam

正式 Module：

```text
EditorInstanceGatewayModule
```

外部 seam 上只有一个稳定 Interface：

```text
connect(exact editor instance)
list typed tools
call typed tool
close
```

调用方不感知内部 launcher/project 切换。Module Implementation 负责 discovery ownership、连接状态、上下文转换、Catalog facts、project guard 与失效处理。

删除测试成立：若删除该 Module，Editor 实例选择、launcher 存活、项目上下文转换、Catalog 刷新和旧 authority 失效会重新散落到 Adapter、Tool caller 和各项目 Tool 中。因此它是深 Module，不是透传包装。

## 7. Editor 实例身份与 discovery

### 7.1 身份 owner

Native Editor 进程启动时产生 engine-owned、进程生命周期内稳定的：

```text
editorInstanceId
```

它不是路径、PID、项目 identity 或 caller 提供的别名。进程结束后该身份失效，不跨进程复用。

### 7.2 discovery

discovery publication 绑定 `editorInstanceId`，在 launcher 与 project 状态均存在。连接必须使用：

```text
精确 discovery endpoint
或精确 editorInstanceId
```

禁止根据最近使用项目、窗口标题、当前目录或“唯一看起来合适”的进程猜测。

发现多个实例而 caller 未精确选择时返回：

```text
ambiguous_editor_instance
```

diagnostic 可列出安全的实例摘要，但不得自行选择。

## 8. 内部上下文模型

一个连接内部只有两种上下文：

```text
LauncherContext {
  editorInstanceId
}

ProjectContext {
  editorInstanceId
  projectIdentity
  canonicalProjectRoot
  projectDigest
  readGeneration
}
```

项目上下文是可选、可替换的事实，不是 Gateway 连接身份。launcher 状态没有 project read/mutation authority。

上下文来自 Native Editor owner 的当前事实；Adapter 和 caller 都不能构造或缓存为第二份真相。

## 9. 稳定连接合同

1. MCP/typed client 连接绑定精确 `editorInstanceId`。
2. 项目创建、关闭或合法切换不创建第二个公共 Session Interface。
3. 同一进程内连接保持可用，Catalog 与 execute guard 读取最新上下文。
4. Editor 进程终止才使连接终止。
5. transport 断开可按现有 transport 语义重连同一实例，但不是项目生命周期步骤。
6. AI 不提交 project binding、context generation 或 rebind request。

## 10. 内部上下文转换

`project.create` 成功时，Module 原子完成：

```text
LauncherContext
  -> Native Editor CreateProject owner creates and opens
  -> verify owner-produced project facts
  -> invalidate old project-scoped authority
  -> publish ProjectContext
  -> refresh availabilityDigest
```

这是 Tool Implementation 内部步骤，不是外部菜谱。成功结果可返回项目事实，但不返回 `reconnectRequired` 或下一步 Tool 顺序。

若项目创建成功但 Editor open/context publish 未完成，调用不得虚报 `created`；按第 16 节失败合同收敛。

## 11. Catalog 与 availability

255 的静态 Tool Contract 与 Tool-owned Availability Probe 继续是唯一目录模型。

```text
LauncherContext:
  project.create 可 ready
  项目 read/mutation Tool blocked

ProjectContext:
  project.create blocked
  项目 Tool 按最新 authority/runtime/platform facts判定
```

`tools/list` 仍列出全部 registered typed Tool。上下文变化只更新 availability 和 `availabilityDigest`，不改变静态 `catalogDigest`。

Catalog 不返回 reconnect 指令、自动 payload 或跨 Tool `nextToolIds`。

## 12. 极简 project.create Interface

### 12.1 Tool identity

```text
toolId: project.create
kind: goal-level command
availability: LauncherContext only
executionOwner: Native Editor project lifecycle owner
```

### 12.2 direct input

输入精确为：

```text
requestedProjectRoot
projectName
```

规则：

- `requestedProjectRoot` 是 caller 选择的最终绝对本地项目根。
- 不展开环境变量，不从工作目录推断，不接受盘符根。
- `projectName` 只有显示名称语义，不携带路径。
- schema 拒绝 unknown fields。

caller 不得提交：

```text
editorInstanceId
canonicalProjectRoot
projectIdentity
projectDigest
readGeneration
sessionId
operationId
receiptId
grantRef
idempotencyKey
templateId
```

### 12.3 成功结果

```text
status: created
receiptId
requestedProjectRoot
canonicalProjectRoot
projectName
projectIdentity
projectDigest
readGeneration
openedInEditor: true
replayed
```

### 12.4 失败 diagnostics

至少区分：

```text
invalid_input
unsupported_location
target_exists
parent_unavailable
create_failed
open_failed
context_transition_failed
cleanup_failed
implementation_unavailable
```

结果必须说明目标是否被创建、owned cleanup 是否完成、是否留下需要用户处理的精确路径。

## 13. operation、replay 与 receipt lineage

1. 复用现有 transport/request/invocation/operation identity。
2. replay ownership 由现有 invocation/operation owner 承担。
3. `project.create` 不引入 caller idempotency key 或第二套 receipt Module。
4. receipt 绑定 `editorInstanceId`、invocation/operation、输入 digest 与 owner 结果。
5. 创建成功后的项目 identity/digest/read generation 来自 Editor owner。
6. 同一已完成 invocation 重放只返回既有结果，不再次创建。
7. 新 invocation 指向已存在目标时返回 `target_exists`，不得猜测它是否由旧调用创建。

## 14. authority 与安全

### 14.1 LauncherContext

只有 launcher-safe Tool authority。不存在隐式 active project，不得读取或修改磁盘上的任意项目。

### 14.2 ProjectContext

现有项目 Tool 继续由 Tool Kernel 的 Grant、mutation、receipt、rollback 和 digest guard 管理。Module 注入当前 engine-owned project facts，caller 不提交这些字段。

### 14.3 上下文变化

任何项目 identity 变化必须使旧 project-scoped Grant、未完成 mutation authority 和不能跨项目继续的 operation 失效。失效是安全收敛，不迁移旧权限。

### 14.4 host side effect

`project.create` 不申请 engine project-mutation Grant，因为 launcher 中尚无项目。宿主可以按其通用重要操作策略要求用户确认，但该确认不进入 Tool direct input，也不形成引擎内第二套 approval lifecycle。

## 15. no-overwrite 与 CreateProject owner

正式 Native Editor CreateProject owner 必须局部加固：

1. 父目录存在且可用。
2. 最终目标必须不存在；已有空目录也失败。
3. 排他声明本 invocation 新建的目标。
4. 只写本 invocation 拥有的创建物。
5. 初始化或打开失败时，只清理仍可证明归本 invocation 所有的内容。
6. 无法证明安全清理时停止并返回 `cleanup_failed`。
7. Adapter 不手写 manifest、Scene、Settings 或维护第二份 project registry。

不为此新建 staging/freezer/rollback Module。

## 16. 失败与部分状态

| 首个失败点 | 必须结果 |
|---|---|
| 输入/目标检查 | 零写入 |
| 排他创建前 | 零写入 |
| 初始化失败 | owned cleanup；报告结果 |
| open 失败 | owned cleanup；不得进入 ProjectContext |
| context publish 失败 | 不虚报成功；连接保持可诊断，旧 authority 已失效 |
| Catalog refresh 失败 | execute guard 仍按最新 owner fact fail closed |
| Editor 进程退出 | 连接终止，所有实例内 authority 失效 |

失败后 AI 可根据 typed diagnostic 自主决定下一步；方案不规定重试菜谱。

## 17. Implementation ownership map

| 责任 | Owner |
|---|---|
| Editor 实例 identity/publication | Native Editor host |
| 稳定连接与内部 context | `EditorInstanceGatewayModule` |
| MCP projection | 既有 MCP Adapter |
| 静态 Tool Contract | `AiToolContractRegistry` |
| availability | 255 Tool-owned probe + Catalog 聚合 |
| project.create schema/dispatch | Tool Kernel |
| 项目创建与打开 | Native Editor CreateProject owner |
| 项目内安全写入 | `ProjectWriteScope` |
| project Grant/mutation/receipt | 既有 Tool Kernel owners |
| operation replay | 既有 request/invocation/operation owner |

MCP Adapter 只适配 transport；不得拥有 Editor lifecycle 或项目真相。

## 18. 兼容与迁移

当前 project-bound Gateway 需要迁移为 process-scoped host：

```text
旧：active project -> create/destroy Gateway host
新：Editor process -> create/destroy Gateway host
```

现有项目 Tool 的公共 direct input 不应因此扩张。其 project guard 改为从 Module 当前 `ProjectContext` 获取。

迁移期间不得并存两套 production discovery owner。测试 Adapter 可以存在，但必须满足同一 Interface，不得具有更宽 authority。

旧 256 代码只可从 Git 历史只读参考，不得机械恢复。

## 19. 验证边界

验证证明共享 Gateway 身份和一个 Tool，不建立新验证产品。

### 19.1 定向 contract

```text
Editor 实例 identity 与精确 discovery
multiple-instance ambiguity
LauncherContext / ProjectContext guard
同一连接内部 context transition
Catalog availability transition
旧 project-scoped authority invalidation
project.create 精确两字段与 no-overwrite
owned cleanup 与 replay
```

### 19.2 受影响域回归

覆盖 Gateway host、protocol、MCP projection、Catalog、Tool Kernel、Editor CreateProject owner，以及所有读取 project binding 的 consumer。必须包含现有 project-bound read/mutation Tool，防止共享身份迁移削弱 guard。

### 19.3 最终 production smoke

只使用既有 production typed MCP 路径验证一次：

```text
connect exact Editor instance in launcher
-> list project.create ready
-> call project.create(two exact fields)
-> same connection remains alive
-> Catalog changes to ProjectContext availability
-> project identity/digest/read generation match Editor owner
-> one existing project read Tool succeeds
-> old launcher/project authority cannot leak
```

不创建 256 专用 Runner、candidate、freezer 或永久 Gate。

## 20. 后续施工建议

本节仅定义未来施工切片，不构成施工文档或授权：

```text
L0  protocol/context red contracts
L1  stable Editor instance discovery and connection
L2  optional project context and stable session transition
L3  Catalog transition and operation/Grant invalidation on context change
L4  minimal project.create and owner hardening
L5  affected-domain regression
L6  one production typed MCP smoke
```

每个切片先定向测试，再运行实际受影响域。只有 L0-L5 全绿且环境等价预检成立后，才运行一次 L6；不得把完整回归当逐 bug 调试循环。

任何单次施工不得超过项目施工规则规定的三小时硬上限；预计超出时必须在施工文档阶段继续拆分。

## 21. 验收标准

1. Gateway host 生命周期绑定 Native Editor 进程，而非 active project。
2. launcher 与 project 状态均能精确 discovery 同一 `editorInstanceId`。
3. 多实例不猜测，歧义返回 typed diagnostic。
4. 一个 MCP 连接跨 `LauncherContext -> ProjectContext` 保持有效。
5. 没有 public rebind/reconnect Interface 或 AI lifecycle 菜谱。
6. Catalog 自动反映上下文，并保持静态 registered Tool 可发现。
7. launcher 没有 project authority。
8. context 变化使旧 project-scoped authority 失效。
9. `project.create` direct input 精确为两个字段。
10. 目标存在零写入，成功由正式 Editor owner 创建并打开。
11. receipt/operation/project facts lineage 一致且不可由 caller 伪造。
12. 不恢复旧三工具、P8/P9/P10、candidate/freezer 或专用 Runner。
13. 一个 production typed MCP smoke 证明真实 composition。

## 22. 方案自审

### 22.1 是否限制 AI

不限制。AI 决定目标、Tool 顺序和后续动作。Module 只隐藏连接与项目上下文的一致性机制，并提供 typed diagnostics。

### 22.2 是否形成工具外菜谱

没有。connect/list/call/close 是通用工具 Interface；项目转换发生在 Tool 内部。AI 不需要执行 discovery -> reconnect -> rebind -> approval 的固定顺序。

### 22.3 是否过度复杂

新增复杂度只修复共享根因：Gateway 进程身份不应等于项目身份。没有新增第二个公共生命周期 Interface，也没有恢复旧验证系统。

### 22.4 是否是深 Module

是。小 Interface 隐藏实例 discovery、launcher 存活、上下文转换、Catalog 更新、authority 失效和 project guard，向所有 Tool 与 Adapter 提供 Leverage 和 Locality。

### 22.5 是否削弱既有安全

没有。launcher authority 更窄；项目 Tool 仍受 project identity/digest/Grant guard；上下文变化明确失效旧 authority。

### 22.6 测试是否膨胀

没有专用长期 Runner。验证分为定向 contract、实际受影响域和一次既有 production smoke，并明确去重和 fail-fast。

## 23. 与旧 256/F0 的关系

旧 F0 是本方案的输入：

```text
已证明：Tool-only 薄接入在现有 project-bound Gateway 上不可行。
本方案修正：连接绑定 Editor instance，project 降为内部可选 context。
```

以下历史终态不改变：

```text
旧 active 256 implementation 已物理退役
旧三工具与显式 reconnect lineage 不恢复
F0 历史施工不继续、不重试
旧 P8/P9/P10 和 construction Runner 不恢复
```

新施工必须另行编写并自审，不继承旧 Gate 编号、旧授权或旧 candidate。

## 24. 最终结论

256 方案 B 的正式形态是：

```text
stable Native Editor process identity
  -> EditorInstanceGatewayModule
  -> optional internal project context
  -> minimal project.create
  -> same connection, automatic capability transition
```

它解决的是 AI 与真实 Editor 的稳定连接和上下文一致性，不是为 AI 增加流程。fresh 施工文档已进入当前施工槽；窗口 A、L2 与 L3 已完成，当前仅执行独立 L4，L5-L6 不在本窗口。
