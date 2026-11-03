# 309-F Headless Engine Tool Provider Atomic Cutover v1 方案

> 文档性质：309 Stage F 正式子方案；已由用户确认并完成施工。Gate A-I 与 F1-F12 全部通过；方案 B（Compiler 内部唯一 Assembler）和 C-Atomic Headless Provider 切换已落地。  
> 上级 authority：`00-AI-First-Game-Engine-权威产品需求-v1.md`、`00-AI-First-Game-Engine-权威架构设计-v1.md`。  
> 上级方案：`309-Headless-AuthoringProjectContext-Unified-Mutation-v1方案.md`。  
> 完成状态：309-A 至 309-F 已完成并归档；309-F 施工文档位于 `施工文档/已完成/`，当前施工槽与待执行队列均为空。  
> 正式决策：采用 C-Atomic，一次替换 Editor-hosted Gateway 默认拓扑，不建立常驻项目 daemon，不保留双 authority 生产路径。  
> 设计日期：2026-09-01。

## 0. 一句话决定

建立默认无 Editor 的 `Headless Engine Tool Provider`，将具体 typed `engine_*` 工具直接投影到
宿主 AI 的同一 Agent Tool Loop；同一施工窗口内移除 Gateway 的项目 generation authority、
Editor process/session 前置、discovery/named-pipe 默认拓扑和 Tool Kernel 对完整 `EditorSession`
的依赖。

新 Provider 只是宿主 Tool Registry 与引擎深 Module 之间的执行 Module，不理解完整对话，
不持有 Agent 计划，不运行第二个模型循环。

## 1. 名称澄清与上级方案修订关系

本文的 C-Atomic 不是 309 中已否决的“常驻 Project Authority Daemon 方案 C”。

```text
已否决的旧方案 C
  = 必须安装、启动、发现、连接的常驻项目 daemon

本文 C-Atomic
  = 可嵌入宿主或作为 MCP stdio 子进程启动的 Headless Tool Provider
  = 正确性继续来自 canonical storage + AuthoringProjectContext + OS lock/CAS
  = 不需要可发现的长生命项目服务
```

上级 309 原将 Stage F 限定为“移除 Gateway 第二套 generation 真相”。用户已确认
框架级改动不采用长期过渡拓扑，因此本方案将 Stage F 正式修订为：

> 退役 Gateway authority 的同时，建立并切换到第一方 Headless Engine Tool Provider。

上级 309 与当前入口文档已同步该正式决策；施工、资格验收、真实 Codex Agent Tool Loop、完成记录和归档
均已闭环。本方案仍只定义 Stage F 边界，不自动激活后续系统。

## 2. 要解决的根本问题

当前生产路径仍然是：

```text
Host AI / MCP
  -> ai_engine_gateway_mcp
      -> Gateway discovery
          -> Editor-owned named pipe
              -> GatewayCore
                  -> EditorSession
                      -> AiCapabilityToolKernel
                          -> ProjectCandidate / domain tools
                              -> AuthoringProjectContext
```

这条路径存在四个架构性问题。

### 2.1 AI 依然间接依赖 Editor

`AiCapabilityToolKernel` 的主要 Interface 仍接受 `&EditorSession` 或 `&mut EditorSession`。
`EditorSession` 同时持有 Project、Scene draft、selection、GameView、Play、Preview、AI Panel、
LLM request 和 Editor composition 状态。

即使 Gateway digest 全部替换为 Context revision，只要 Kernel 仍要求 `EditorSession`，默认
AI 路径就仍然不是 Headless。

### 2.2 Gateway 仍有重复的项目状态门禁

Gateway 仍持有：

- `observed_project_digest`；
- `read_generation`；
- `read_stale_reason`；
- session-bound project context；
- reconnect-required 与 Editor-instance identity。

Tool Kernel 仍持有 `expected_project_digest`、Grant `initial_base_digest`、Goal read generation 和
Candidate 旧 digest。而 `AuthoringProjectContext` 已经拥有 canonical revision、snapshot lease、CAS、
journal、receipt、rollback 和 recovery。

这些重复门禁增加了重新连接、过期判定和诊断分歧，却不再提供新的正确性。

### 2.3 工具执行语义仍含 Editor/Gateway 假设

当前 `project.create`、`project.preview`、`project.build_export`、`project.delivery_verify`
的 contract、路径、诊断和 evidence 中仍存在 Editor launcher、Editor Play、GameView 和 Gateway
delivery root 语义。它们不能被原样换名为 `engine_*` 后宣称 Headless 完成。

### 2.4 方案 B 只修 authority，不修默认产品拓扑

狭义 Stage F 可以让 Gateway 改用 Context revision，但 Editor process、Gateway discovery、named pipe、
EditorSession 和 reconnect 工作流仍会存在。这不满足“引擎像普通 AI 工具一样直接使用”
的产品需求。

## 3. 设计目标

本方案必须同时实现：

1. 不安装、不启动、不发现、不连接 Editor 时，真实宿主仍能列出并调用具体
   `engine_*` 工具。
2. Provider 的默认生产依赖图不包含 `editor_*`、`editor_ui_*`、`editor_window_*`
   或 `ai_tool_gateway`。
3. Host AI 仍拥有用户目标理解、计划、代码生成、工具选择和结果判断。
4. Provider 自动 open/reuse/refresh Context，AI 不编排 Context 内部操作。
5. 普通 `write_file` / `apply_patch` 与 Engine tools 可在同一 Agent loop 中交错使用；下一个
   Engine 语义操作必须观察到最新 canonical 内容。
6. 完整单文档写入继续使用 309-E LWW；结构化增量、跨文件和高风险 mutation 继续
   使用 Context revision CAS、journal、receipt 和 rollback。
7. 保留现有 Tool Kernel 中有价值的 Grant、operation、cancellation、receipt、rollback 和
   diagnostics 语义，但移除重复项目 authority。
8. Native Host Adapter 与 MCP Adapter 消费同一 Provider Interface；协议差异不进入 Engine Core。
9. Editor 继续是可选验证 Projection，与 Headless AI 共享同一 canonical authority，但不托管
   默认 Engine tools。
10. 一个施工窗口完成新拓扑激活与旧拓扑退役，不发布或归档中间双架构状态。

## 4. 非目标与不得扩大的范围

本文不负责：

- 实现新的 Agent Planner、自动游戏生成模型循环或隐藏 workflow；
- 建立常驻 Project Authority Daemon、网络服务、发现中心或用户必须管理的后台进程；
- 一次实现 Project Game SDK、完整 R3 Game Project Compiler、Semantic Outcome 或 AI Development Pack
  的全部未来能力；309-F 只迁移当前 run/build/delivery 所必需的既有 Compiler/Build owner；
- 一次补齐 Scene、Prefab、AUI、Animation、Audio、Physics、Particle 和移动端的全部
  `engine_*` 工具；
- 重写 Renderer、ECS、RuntimePackage、Asset DB 或 Build Pipeline；
- 将 Context 的 `open/refresh/snapshot/commit` 直接注册为 AI 工具；
- 对 AI 暴露 `engine_catalog`、`engine_execute(toolId, payload)` 或巨大 `engine(prompt)`；
- 为了保留旧 Gateway 客户端而长期维护双协议、双进程和双 authority；
- 将全部 `editor_core` 一次性重写或拆解。只移动现有 Engine tools 可达路径上必须
  Headless-neutral 的状态与领域能力。

方案只一次定型“宿主如何调引擎”的框架，不把“未来全部工具都已完成”作为
309-F 的虚假完成条件。

## 5. 成熟实现参考与采纳边界

### 5.1 DeepSeek Harness

`DeepSeekHarness源码参考/deepseek-harness/packages/core/tools/src/index.ts` 将工具定义、schema、
execution identity、approval、cancellation、result normalization 和 Tool Runtime 放在稳定 Tool 层；
具体文件、shell、goal 等 Provider 仍拥有自己的领域正确性。

本方案采纳：

- ToolDefinition 与领域 Implementation 分离；
- Host approval 在工具执行边缘组合；
- 调用身份、取消、输出和诊断有稳定规范；
- Tool Runtime 不接管领域存储和领域 revision。

不照搬 Cordis plugin/service 框架、TypeScript runtime 或 Harness 自身 Agent lifecycle。

### 5.2 OpenCode

`OpenCode源码参考/opencode/packages/opencode/src/tool/registry.ts` 负责 builtin/plugin/MCP 工具投影；
`packages/opencode/src/session/tools.ts` 在真实 Agent session 中组合 permission、tool-call identity
和 MCP execute；具体 filesystem tool 仍在领域 Implementation 内完成路径校验和写入。

本方案采纳：

- Registry 只投影具体工具，不生成项目 revision；
- native tool、plugin tool 和 MCP tool 可通过同一 Agent Tool Loop 消费；
- permission 是宿主责任，领域 Provider 仍执行自己的安全检查。

不照搬普通 `write/edit` 的单文件乐观语义来替代本项目已实现的跨文件 CAS、
journal 和 crash recovery。

### 5.3 成熟实现结论

成熟 Agent Harness 的共性是：

```text
Agent 拥有计划
Tool Registry 拥有投影与调度
Host permission 拥有用户授权交互
Domain Provider 拥有真实执行与领域正确性
Canonical storage owner 拥有 revision/CAS/atomicity
```

本项目不应让 Gateway 或 Tool Registry 重新拥有项目 revision，也不应在 Provider 内复制 Agent。

## 6. 正式目标拓扑

```text
User
  -> Host AI Agent / existing Agent Tool Loop
      |-- read_file / write_file / apply_patch / run_command
      |-- concrete engine_project_* / engine_run / engine_build / engine_verify_* tools
      v
  Host Adapter
      |-- NativeHostAdapter (宿主存在第一方扩展点时)
      |-- McpHostAdapter (Codex 当前真实兼容路径)
      v
  Headless Engine Tool Provider
      |-- ProviderSession / workspace-project binding
      |-- ToolDefinition projection
      |-- Host approval translation
      |-- Context open / reuse / refresh
      |-- canonical ToolResult mapping
      v
  Engine Tool Kernel
      |-- contract / availability / policy
      |-- Grant / operation / cancel
      |-- receipt / rollback / diagnostics
      |-- domain dispatch
      v
  Project Authoring Execution Module
      |-- AuthoringProjectContext
      |-- project observation / structured change
      |-- GameProjectCompiler / Build owner
      |     |-- canonical source -> RuntimePackageBuildInput
      |     |-- RuntimePackage / Project Player / Windows dev package
      |     `-- run / delivery verification reports
      |-- runtime / outcome adapters
      v
  canonical project storage / RuntimePackage / Player / evidence

Optional Editor
  -> EditorProjectAdapter
      -> same AuthoringProjectContext and domain Modules
  -> optional Editor verification UI
```

删除 `Headless Engine Tool Provider` 后，ToolDefinition 投影、Host approval 转换、Provider session、
Context 生命周期、Kernel invocation 和 result normalization 会散落到 Codex、OpenCode、
DeepSeek Harness、CLI 和 Editor 组合层，因此该 Module 具有真实深度，不是空转包装。

## 7. 核心 Module 与 Interface

### 7.1 Engine Capability Definition Module

保留并从 Editor-specific 语义中中立化现有 `AiToolContractRegistry` 的有效能力：

- 稳定 internal tool id 与 version；
- typed input/output schema；
- side effects、risk、duration 和 cancellation 分类；
- capability/readiness 要求；
- canonical result、diagnostics 和 evidence 形状；
- native/MCP 投影元数据。

该 Module 不持有动态项目状态、Provider session、Grant、operation 或 Runtime readiness。
动态 readiness 必须在每次调用前由真实 owner 重新检查。

### 7.2 Project Authoring Execution Module

将现有 Engine tools 可达路径上必须 Headless-neutral 的项目状态从完整 `EditorSession` 中抽出。
概念可命名为 `ProjectAuthoringSession`，其 Interface 至少向内部领域 Module 提供：

```text
open(ProjectLocator) -> ProjectBinding
current_facts() -> ProjectFacts
refresh() -> ProjectRevision
snapshot(Scope) -> ProjectSnapshotLease
prepare_change(Intent, SnapshotLease) -> ValidatedChange
commit_change(ValidatedChange, ExpectedRevision) -> MutationReceipt
rollback(ReceiptRef) -> RollbackReceipt
close()
```

具体 Rust 方法和类型名由后续施工文档根据实际代码冻结，但 Interface 必须隐藏：

- `ProjectSession` 内部字段；
- Context mutex/handle 编排；
- manifest/settings/source inventory 重新加载；
- snapshot lease 保留与释放；
- ProjectWriteScope 与 canonical path containment；
- Candidate lowering、validation 和 Context mutation lowering；
- build/runtime/evidence 的项目绑定。

Editor 只通过 `EditorProjectAdapter` 将必要的 draft/save/verification 状态组合到该 Module；
Project Authoring Execution Module 不反向依赖 Editor。

### 7.3 Game Project Compiler / Build Module

用户已确认方案 B（此前选项 1 的中立 Compiler owner 方案）：把当前位于 `editor_core` 的
`canonical project source -> RuntimePackageBuildInput -> Project Player / Windows dev package`
迁为 `project_authoring_execution` 内的中立深 Module，概念名为 `GameProjectCompiler`。它是
权威架构 5.7 已定义的 Game Project Compiler 在 309-F 中的必要子集，不新增第三个 crate，也不宣称
完整 R3 已完成。

该 Module 对 Provider 和 Editor 只暴露以下高层 Interface：

```text
prepare(ProjectSnapshotLease, TargetProfile) -> PreparedRuntimePackage
run(PreparedRuntimePackage, RunOptions) -> RuntimeExecutionReport
build(PreparedRuntimePackage, BuildRequest) -> BuildDeliveryReport
verify(DeliveryRef, VerifyRequest) -> DeliveryVerificationReport
```

Interface 冻结以下语义：

- `prepare` 的输入必须绑定 Context 提供的 operation-owned immutable snapshot lease；输出携带
  project identity、revision、profile、RuntimePackage identity 和 diagnostics，不能混用两个 revision；
- `run` 只消费已准备产物并调用现有 Runtime/Player owner，不通过 Editor Play、GameView 或 UI command；
- `build` 复用同一 prepared identity 和现有 RuntimePackage/Player/export 实现，输出中立 delivery ref；
- `verify` 只验证中立 delivery ref 指向的真实产物，返回现有 process/report/evidence，不接受任意未授权路径；
- cancellation、timeout、process failure、compile failure 和 verification failure 必须保持可区分；
- Scene、Prefab、AUI、Asset、Font、Input、Rule、Animation cooker、staging 路径和 process 编排均为
  Module 内部 Implementation，不进入 Provider Interface。

迁移采用 ownership move，不复制 pipeline，并分清 F-B/F-C 的边界：

1. Gate F-B 只把 `ProjectRuntimePackageAssembler` 变为 `GameProjectCompiler` 内部的唯一装配 Module，连同
   `BuildProfile`、artifact-cache 的中立接口/实现，以及其真实调用闭包内的中立 schema/read/parse/cook 定义；
   它必须消费 lease-owned `SourceView`，继续是项目源进入 `RuntimePackageBuildInput` 的唯一入口；
2. F-B 的中立闭包包括 Project Manifest/Build Profile、Scene/Prefab schema 与 bake、AUI/Font、Input、Rule、
   Animation、Asset/Texture、observation/source mapping 读取和 cooker。它不包含 draft、selection、undo、
   command/stage、下载/审批/mutation、窗口/GameView、progress/report panel 或 Editor launcher；
3. Gate F-C 才处理 `ProjectPlayerArtifact`、`DesktopExportPipeline`、player staging、safe project output 和
   Windows dev delivery verification；F-B 不预支 F-C，也不改变 Player/export 行为；
4. `RuntimePackageBuilder`、`runtime_cli` process/runtime verification 和 `engine_runtime` 保持原 owner，
   Compiler 只调用，不复制或搬迁；
5. Editor launcher、draft、selection、undo、progress UI、Report Panel 和 UI command 留在 `editor_core`，
   通过 `EditorProjectAdapter` 消费同一 Compiler Interface；Provider 不解析 Editor state、不直接调用 cooker、
   也不保留另一份 assembler/export 实现。

现有 Assembler 依赖的一些中立 schema/cooker 当前物理上位于 `editor_core`。只允许迁移 Assembler 的真实
调用闭包中不含 Editor workflow 的定义；混合 Module 必须抽出其纯读取/解析/cook 部分，Editor 反向消费新的
中立 owner，不得把整个 Editor 工作流搬入 `project_authoring_execution`。新 owner tests 与 Editor consumer
回归通过后删除旧 Implementation 和重复测试，只保留明确的 Adapter/re-export，禁止长期双份实现。

### 7.4 Engine Tool Kernel Module

Kernel 继续拥有：

- Tool contract validation；
- capability availability；
- bounded Grant；
- operation identity/state/journal；
- cancellation；
- canonical ToolResult；
- mutation/rollback receipt lineage；
- structured diagnostics；
- invocation exact replay 等幂等性。

Kernel 不再接收完整 `EditorSession`，而是消费 Project Authoring Execution Module 提供的
中立项目事实与领域操作。

必须退役或内部化：

- AI 可见输入中的 `expected_project_digest`；
- Gateway `read_generation`；
- Gateway observed digest/stale reason；
- 以 Editor-instance identity 作为 Grant 前置；
- 以 `EditorSession` dirty/GameView/Play 状态作为默认 Headless 工具事实。

AI 可见 tool input 只表达用户意图和工具必要参数。Provider 在调用时将当前
`ProjectRevision`、snapshot lease、host session 和 approval facts 注入 canonical invocation。

Kernel 内部可以使用通用 `lookup/dispatch` Interface，但 Host Adapter 不得将其原样注册给 AI。

### 7.5 Headless Engine Tool Provider Module

Provider 是本方案的外部深 Module。其概念 Interface 为：

```text
attach(HostSessionContext) -> ProviderSession
tool_definitions(ProviderSession) -> ToolDefinitionSet
invoke(ProviderSession, HostToolCall) -> CanonicalToolCallOutcome
observe(ProviderSession, OperationRef) -> OperationSnapshot
cancel(ProviderSession, OperationRef) -> CancellationReceipt
detach(ProviderSession) -> DetachReceipt
```

`tool_definitions/invoke` 是 Host Adapter 消费的内部程序 Interface，不是 AI 可见的
`engine_catalog/engine_execute`。Host Adapter 必须把每个 ToolDefinition 注册为独立具体工具。

Provider 隐藏：

- 宿主 session 与 project binding；
- project locator 校验；
- Context open/reuse/refresh；
- Host approval 与 Engine Grant 组合；
- Tool Kernel invocation；
- operation observe/cancel；
- canonical result 到宿主 result 的稳定映射；
- Trace/Summary 证据限制和输出大小限制；
- warm Context/cache 生命周期。

Provider 不得持有：

- 完整对话历史；
- 用户的游戏设计目标；
- Agent todo/plan；
- 自动重试的开放式模型循环；
- 另一个 LLM Provider；
- 代替 Host AI 选择下一个工具的 workflow recipe。

### 7.6 Host Adapter

Host Adapter 只负责宿主协议差异：

- ToolDefinition schema 转换；
- 工具名命名空间；
- call/session/message identity 注入；
- approval/permission 结果转换；
- cancellation signal 转换；
- canonical ToolResult 转换为宿主 content/structuredContent；
- 大结果截断与 evidence reference 保留。

必须存在两个真实 Adapter 角色：

1. `NativeHostAdapter`：用于存在第一方 Tool Registry 扩展点的宿主；
2. `McpHostAdapter`：用于 Codex 当前真实可用路径以及其它 MCP-compatible 宿主。

MCP Adapter 是 compatibility transport，不是新 authority。MCP process 由宿主按 session 启动并通过
stdio 调用，不发布 discovery file，不连接 Editor named pipe，不成为项目常驻 daemon。

### 7.7 EditorProjectAdapter

Editor 继续使用 309-E 冻结的语义：

- draft/undo/selection 是 Editor local state；
- 完整单文档 Save/Save As 使用 LWW；
- 结构化增量 mutation 使用 Context CAS；
- clean save 不产生写入；
- Editor UI revision 不是 project revision。

Editor 不再默认启动 `EditorGatewayHost`。如未来 Editor AI Panel 需要使用 Engine tools，
它必须作为可选 Host Adapter 消费同一 Provider Interface，不得恢复 Editor-owned Gateway
或 Editor-owned project authority。

## 8. 依赖方向与 crate 组合

最终依赖方向必须至少满足：

```text
authoring_project_context   engine_input   engine_runtime   runtime_cli
        ^                        ^              ^              ^
        |                        |              |              |
        +------------------------+--------------+--------------+
                                 |
                 project_authoring_execution
                   |-- ProjectAuthoringSession
                   `-- GameProjectCompiler / Build owner
                                 ^
                                 |
                  engine_tool_kernel / engine_tool_provider
                                 ^
                                 |----------------------|
                         native_host_adapter       mcp_host_adapter

editor_core / editor_window
        -> project_authoring_execution
        -> EditorProjectAdapter
        -> optional provider adapter at composition root only
```

必须通过静态依赖 Gate 证明：

```text
engine_tool_provider
engine_tool_kernel
project_authoring_execution
mcp_host_adapter
```

不依赖：

```text
editor_core
editor_ui_model
editor_ui_renderer
editor_window_winit
editor_host
ai_tool_gateway
```

`project_authoring_execution` 可以新增对现有中立 Engine/Runtime crates 的直接依赖，但不得依赖任何
`editor_*` crate。若 Assembler 依赖类型当前来自 `editor_ui_model` 或 Editor workflow Module，Gate F-B
必须把最小 canonical schema/纯 cooker ownership 迁到中立位置，再由 Editor Adapter 消费；不得用
`project_authoring_execution -> editor_core/editor_ui_model` 规避本次 owner 修订。

为控制拆分量，不要为图中每一个 Module 机械建立一个 crate。本次仍只允许既有施工文档冻结的两个新增 crate：
`project_authoring_execution` 与 `engine_tool_provider`。Game Project Compiler/Build 是前者内部的深 Module；
Capability Definition/Kernel 与 Host Adapter 可以继续位于后者内部 Module/binary；上述依赖方向不得因减少
crate 而反转。

如果新 Provider 仍直接依赖整个 `editor_core` 或构造一个隐藏 `EditorSession`，则本方案不合格。

## 9. Project Locator 与 Provider Session 生命周期

### 9.1 项目定位优先级

Provider 不依赖 Editor recent-project state，也不在任意父目录下无界递归搜索项目。
项目定位使用确定优先级：

1. Host Adapter 提供的显式 project/workspace root；
2. MCP process 显式 `--project-root` 配置；
3. process working directory 本身是合格项目根时自动绑定；
4. 无法唯一确定时，返回结构化 `project_binding_required`，由具体
   `engine_project_open` 工具显式选择允许 workspace 内的项目。

`engine_project_open` 是具体项目工具，不是 Context Interface 透传。它执行 path containment、
manifest qualification、recovery 和 canonical identity 绑定，并返回项目绑定结果。

### 9.2 单 session 项目绑定

v1 一个 ProviderSession 同时只有一个 active project binding，避免每个工具都要 AI 重复填写
`projectRoot/projectRef`。

- 无非终态 operation 时可显式 rebind；
- 存在非终态 operation 时，不允许原 session 静默切换项目；
- 同一 Host 并行处理多项目时建立多个 ProviderSession；
- operation、Grant、receipt 和 evidence 始终绑定 canonical project identity/root binding。

### 9.3 Context reuse

Provider 可按 canonical root binding 在进程内复用 warm Context，但 warm cache 不拥有新 authority。

- 每次语义工具调用前至少执行安全 refresh/revalidation；
- watcher 只能作为 refresh hint；
- 多进程正确性继续依赖 canonical bytes、OS lock、CAS 和 journal；
- 最后一个 session 离开且无非终态 operation/snapshot lease 时可回收 warm state。

### 9.4 断线与重连

新连接建立新 ProviderSession；旧 Host approval 和 mutation Grant 不自动跨 session 恢复。
已持久化 operation/receipt 可在绑定同一项目并通过 identity 校验后 observe/recover。

断线不再表示“重新发现 Editor instance”，只表示宿主 Adapter/Provider session 的交通生命周期。

## 10. Tool Projection 与 AI 可见表面

### 10.1 具体工具

AI 可见的是独立 typed tools，例如：

```text
engine_project_open
engine_project_inspect
engine_project_search
engine_project_read_object
engine_project_references
engine_project_source_symbols
engine_project_mutate
engine_project_rollback
engine_project_run
engine_runtime_capture_issue
engine_ui_locate
engine_ui_explain_visibility
engine_project_build
engine_delivery_verify
engine_operation_observe
engine_operation_cancel
```

最终名称必须在施工前根据权威架构命名和现有 tool contract 兼容性统一冻结。
本列表只冻结“具体工具”边界，不承诺未实现的完整未来工具集。

### 10.2 禁止暴露

AI 不得看到：

```text
aife_catalog
engine_catalog
engine_execute
authoring_context_refresh
authoring_context_snapshot
gateway_status
connect_editor
discover_editor
editor_instance_id
read_generation
expected_project_digest
```

Host Adapter 可在 Agent turn 开始前内部读取 ToolDefinitionSet 并注册工具，但这不是模型需要
手动调用的第二级 catalog。

### 10.3 普通文件工具兼容

正式用法保持：

```text
AI -> write_file 修改 Rust 游戏代码
AI -> engine_project_inspect/check
Provider -> Context refresh -> 发现新 revision
AI -> engine_project_run
AI -> engine_project_build
```

Provider 不强迫普通文件写入经过 `engine_project_mutate`。如果最后写入导致项目 invalid，
下一个 Engine 工具返回 invalid revision 和源级诊断，不恢复旧字节，不继续使用旧事实。

## 11. Host Approval 与 Engine Grant

Host approval 和 Engine Grant 是两个不同问题：

```text
Host approval
  = 用户/宿主是否允许这次 tool call 发生

Engine Grant
  = 这次调用在哪个项目、revision、domain、write scope、risk 和时间内有效
```

正式顺序：

1. Host Adapter 提供 call/session identity 与 permission facts；
2. Provider 验证 Adapter identity 和 tool contract；
3. Provider 打开/刷新项目并确定当前 revision；
4. Kernel 根据已验证的参数和真实项目事实生成 bounded Grant；
5. 执行前再检查 availability、scope、revision 和 cancellation；
6. 返回 canonical ToolResult/receipt。

Native Adapter 在宿主允许时可以通过 approval callback 完成动态授权。MCP Adapter 依赖
宿主在 `tools/call` 前完成工具权限判定，再由 Engine 独立限制 project scope/revision/risk。

不允许：

- 用 AI 输入字段自我声明“已授权”；
- 将 Editor approval panel 作为 Provider 必选前置；
- 将一次宿主 approval 扩大为跨 session 无限 mutation 能力；
- MCP 缺少安全表达时静默降级 elevated mutation。无法安全投影的能力必须
  `authorization_required` 或 `unsupported_on_adapter`，不得猜测授权。

## 12. Revision、Snapshot 与 Mutation 语义

### 12.1 Read/inspect/check

```text
tool call
  -> Provider refresh
  -> Context publishes Rn
  -> acquire immutable snapshot lease at Rn
  -> domain read/check
  -> canonical result binds Rn
  -> release lease
```

长操作必须绑定 operation-owned lease，不能在一次 Compiler/Build/Runtime 中混用多个 revision。

### 12.2 结构化 mutation

```text
refresh -> Rn
snapshot/prepare/validate at Rn
Host approval + Engine Grant
Context commit expected Rn
  -> success: Rn+1 + receipt
  -> drift: no write + structured diagnostic
```

Kernel 不再自行重新扫描一个竞争 digest 作为第二 authority。Candidate/Goal Mutation 的旧
digest 字段如在内部迁移期仍需保留，必须严格由 Context `ProjectRevision` 投影，不能重算或
暴露给 AI。最终生产状态不保留两个可独立变化的 revision/digest owner。

### 12.3 完整文档保存

309-E 语义不变：

- 有效完整单文档 Save 使用 LWW；
- equal bytes/clean save 不写；
- 写入后 refresh 发布真实 revision；
- 多文件、引用迁移、稳定身份移动和结构化增量不得伪装为 LWW Save。

### 12.4 Receipt 与 rollback

Provider/Kernel receipt 保留 tool-call identity、Grant、project identity、before/after revision、changed set、
validation digest 和 Context receipt binding。Rollback 最终由 Context exact-after-revision 和 root-binding 语义判定，
不依赖 Gateway session generation。

## 13. Run、Preview、Build 与 Outcome 必要迁移

方案 C 不能只把 `project.preview` 重命名为 `engine_run`。当前 Preview 依赖 Editor `Play`、
linked Editor RuntimeModule、GameView present report 和 Editor retained frame。这些都是必须消除的前置。

Gate F 复核已证明当前不存在可直接复用的“窄 Headless build adapter”：canonical project source 到
`RuntimePackageBuildInput` 的 Assembler，以及 Project Player/Windows export owner 都仍在 `editor_core`。
用户已正式确认方案 B（此前选项 1 的中立 Compiler owner 方案），因此本节不再允许 Provider 依赖 Editor、复制 pipeline 或把 run/build 假报 ready；
必须先完成第 7.3 节定义的中立 owner 迁移。

309-F 只迁移现有工具达到 No-Editor 资格所必需的最小真实链路：

```text
ProjectSnapshot at Rn
  -> project_authoring_execution::GameProjectCompiler::prepare
  -> existing ProjectRuntimePackageAssembler implementation
  -> RuntimePackage or exported Player
  -> headless/windowed Runtime execution
  -> bounded report/screenshot/evidence
  -> canonical ToolResult bound to Rn
```

迁移规则：

1. `engine_run` 不调用 Editor UI command；
2. runtime binding 来自项目 manifest/Project Runtime ABI 和实际准备产物，不来自当前 Editor
   静态 linked set；
3. screenshot/evidence 由 Runtime/Outcome owner 产生，不需要 Editor GameView 保留状态；
4. Build/Delivery 输出使用中立 Engine Tool 目录和 schema，不继续新建 `Gateway` 命名路径；
5. 将现有 RuntimePackage assembler、Project Player、Desktop export 与必要 safe-output/verification
   Implementation 迁入中立 owner；Runtime CLI、RuntimePackageBuilder 和 exported player verification 保持
   原 owner并被复用，不在 309-F 新建另一套 runtime/build pipeline；
6. 暂时无法无 Editor 证明的工具不得假报 ready。它要么在本施工窗口完成必要迁移，
   要么从默认 ready tool set 中移除并返回明确 maturity。

Compiler/Build owner 的事实边界冻结为：

| 事实/行为 | 唯一 owner | Editor/Provider 的角色 |
| --- | --- | --- |
| project identity/revision/snapshot bytes | `AuthoringProjectContext` | 请求并持有 operation lease，不另造 revision |
| canonical source -> `RuntimePackageBuildInput` | Compiler 内部唯一 `ProjectRuntimePackageAssembler`，物理归属中立 Compiler | 只调用 `prepare` |
| prepared package identity/diagnostics | `GameProjectCompiler` | 映射进 UI 或 ToolResult |
| RuntimePackage build bytes/report | 现有 `RuntimePackageBuilder` | Compiler 编排，调用方不直接拼装 |
| Project Player artifact/build report | 中立 `ProjectPlayerArtifact` owner | Editor/Provider 只消费结果 |
| Windows dev staging/export report | 中立 Build/Delivery owner | Editor 显示进度，Provider 返回 evidence |
| exported process verification | 现有 `runtime_cli` verification owner | 中立 Build/Delivery owner调用 |
| Editor draft/selection/progress/Report Panel | `editor_core` | 仅 UI Adapter，不参与 Headless 正确性 |

`PreparedRuntimePackage` 和 `DeliveryRef` 是中立 opaque result，不是新的 canonical project authority；其
identity 必须可追溯到 snapshot revision、profile、RuntimePackage/artifact digest。缓存只影响性能，删除
缓存后仍能从 snapshot 和既有 owner 重建正确结果。

本节不宣称权威架构 R3/R4 的完整 Compiler/Semantic Outcome 已完成；只消除当前工具
对 Editor 的必选依赖，并为后续 R3/R4 预留稳定 Module 消费位置。

## 14. 旧 Gateway 能力迁移/删除矩阵

| 当前能力 | 最终归属 | 决策 |
| --- | --- | --- |
| Tool descriptor/schema | Capability Definition Module | 中立化后迁移 |
| MCP tools/list/call 投影 | McpHostAdapter | 保留协议能力，改为直接调 Provider |
| call/session identity | Host Adapter + ProviderSession | 保留，去除 Editor identity |
| Host access request/decision | Host Adapter approval translation | 保留语义，不依赖 Editor UI |
| Engine Grant | Tool Kernel | 保留并改绑 Context revision |
| operation/observe/cancel | Provider + Tool Kernel | 保留 |
| mutation/rollback receipt | Tool Kernel + Context | 保留，去除 Gateway generation |
| observed project digest | AuthoringProjectContext revision | 删除 Gateway owner |
| read generation/stale reason | Context refresh/result diagnostics | 删除 |
| Editor instance identity | Optional Editor Adapter local identity | 从默认 AI 路径删除 |
| discovery publication | 无 | 删除默认生产使用 |
| named pipe Editor transport | 无 | 删除默认生产使用 |
| Editor frame pump | Provider worker/operation scheduler | 删除 Editor 依赖 |
| Codex MCP config installer | McpHostAdapter artifact/config installer | 保留用户价值，更换目标 binary |
| Gateway delivery root/schema | Engine Tool evidence/delivery owner | 迁移中立命名 |
| Gateway reconnect | Adapter transport lifecycle | 收窄为 Provider session 重建 |

最终生产 composition 不得存在：

```text
MCP -> GatewayRemoteAdapter -> named pipe -> EditorGatewayHost -> GatewayCore
```

也不得保留这条链路作为默认失败时的隐藏 fallback。

## 15. Atomic Cutover 施工原则

“一次改完”的正式含义是：

- 用户一次确认最终拓扑和完整施工范围；
- 只生成一份 309-F 施工文档；
- 在一个施工窗口/分支内完成中立抽离、Provider、Adapter、Editor 退役和回归；
- 中间实现可为了测试短暂共存，但不发布、不归档为完成、不列入生产 composition；
- 最终验收只接受新 Provider 默认路径和 Optional Editor Adapter；
- 不为旧 Gateway 建立长期 feature flag、兼容层或双写。

一次切换不等于跳过内部 Gate。施工文档仍必须按可独立打红和定位的 Gate 执行编译、
定向测试、真实组合测试和最终回归。这些 Gate 是同一切换内部的验证点，不是多个
长期产品阶段。

## 16. 兼容、激活与回退

### 16.1 项目数据兼容

- 不更改 canonical project source 格式作为本方案的前置；
- Context revision/source policy、mutation journal、receipt 和 rollback 应保持向前兼容；
- 只有 Gateway-specific session/discovery/protocol artifact 可被退役；
- 迁移 Gateway 命名的 derived Library/evidence 不得改写 canonical source authority。

### 16.2 激活

仅当本文全部资格 Gate 通过后：

1. Codex/MCP 安装目标切换到新 Provider Adapter；
2. Editor composition 移除 Gateway 默认启动；
3. 删除旧 discovery/named-pipe 生产达性；
4. 同步文档入口与完成记录。

### 16.3 回退

本方案不设计运行时“自动回退到旧 Gateway”。

- 激活前失败：修复当前 Gate，不激活新 composition；
- 尚未发布时需整体撤回：通过源码版本控制回退整个切换，不在代码中保留双路径；
- 发布后修复：在 Provider 拓扑内修复，不恢复 Editor authority。

## 17. 资格 Gate（未来施工文档必须全部覆盖）

### F1：Neutral Dependency Direction

- Provider、Kernel、MCP Adapter 的静态依赖图不包含任何 `editor_*` 或 `ai_tool_gateway`；
- Tool Kernel 公开 Interface 不出现 `EditorSession`；
- Editor 是 neutral Module 的 consumer，依赖方向不反转。

### F2：No-Editor Provider Open

不构建、不启动 Editor/Gateway，独立 Provider process 能绑定真实项目、执行 recovery、
refresh、snapshot 和 inspect，并返回 Context revision/diagnostics。

### F3：Concrete Tool Projection

真实宿主 Tool Registry 列出独立 `engine_*` tools；不存在 AI 可见 `catalog/execute`、
Context 工具或 Editor connection 工具；每个 tool schema 可独立验证。

### F4：Direct File Refresh Compatibility

AI 通过普通文件工具改写 canonical source 后，下一 Engine tool 自动观察新 revision；
valid/invalid 变化均不使用旧事实。

### F5：Unified Mutation/Recovery/Rollback

- 两个 Provider process 从同一 revision 准备结构化 mutation，只有一个 commit 成功；
- crash recovery、unknown external write fail-closed、sealed receipt 和 exact rollback 全部保持；
- Gateway generation 不参与任何正确性判定。

### F6：Headless Run/Build/Delivery

不存在 EditorSession、Editor Play 或 GameView 时，当前宣称 ready 的 run/build/delivery 工具能通过
真实 Runtime/Player 执行，产生绑定同一 revision 的结构化 report/evidence。

### F7：Approval and Grant Composition

- read/write/process-spawn 工具在 Host 中具有正确 permission/side-effect 分类；
- Host approval 不代替 Engine schema/scope/revision/risk 检查；
- Engine Grant 不跨 session 静默扩大；
- 取消、拒绝、adapter unsupported 和 engine rejected 是可区分结果。

### F8：Native/MCP Canonical Equivalence

同一 ToolDefinition、同一项目 revision 和同一输入通过 Native Provider Interface 与 MCP Adapter 执行时，
具有等价 canonical status、diagnostics、receipt/evidence identity 和 side effects；仅宿主包装形状不同。

309-F 必须交付可嵌入 Native Provider Interface 和真实 Codex MCP 路径。如当前 Codex 不提供
第一方 binary/plugin Tool Registry 扩展点，不虚构“Native Codex 已完成”；后续宿主只增加
NativeHostAdapter，不改 Provider/Kernel/Context 框架。

### F9：Optional Editor Coexistence

- Editor 与 Headless Provider 打开同一项目时观察同一 canonical revision；
- Editor draft 不影响 Headless，clean save 不写，完整文档保存 LWW，结构化 mutation CAS；
- Editor 不启动 Gateway 也能完成其现有验证 Projection 职责。

### F10：Gateway Retirement

真实生产 composition、安装配置和进程验收证明：

- 不发布/读取 Gateway discovery file；
- 不启动 Editor named pipe server；
- 不需要 Editor instance id；
- 不包含 GatewayCore/read generation/observed digest 生产路径；
- 旧 Gateway 不是 fallback；
- 旧 Gateway 内部状态测试已由 Provider Interface 行为测试替代，不双重维护。

### F11：Real Codex Tool-call Acceptance

在真实 Codex Agent Tool Loop 中：

1. 不启动 Editor；
2. Codex 能与 `read_file/write_file/run_command` 一样选择具体 Engine tool；
3. 完成至少一条 `inspect -> file edit/structured mutation -> run/check -> build/verify` 链路；
4. ToolResult 回到同一 Agent turn，Codex 可依诊断继续修复；
5. 调用无 AI 可见二级 catalog/execute 或 Editor connection 步骤。

### F12：Regression and Performance

- 309-A 至 309-E 全部行为合同回归；
- Tool Kernel 现有 Grant/operation/cancel/receipt/rollback 要求回归；
- 真实 MCP process smoke 和输出上限回归；
- clean repeated inspect 不产生 canonical 写入；
- warm Context 只提供性能，禁用 warm cache 后结果仍正确；
- 不引入每帧 scan/report 或 Runtime 默认 Trace 热路径。

## 18. 与权威架构 R5 的关系

本方案将权威架构中 `First-class Engine Tool Provider` 的框架载体和第一条真实 Codex
兼容路径提前完成，以避免先继续扩大 Editor-hosted Gateway，之后再做第二次框架迁移。

但 309-F 完成时只能声明：

```text
Headless Provider framework: qualified
Codex MCP concrete-tool path: qualified
Current migrated capability set: 按 maturity 逐项 qualified
```

不能自动声明：

```text
Project Game SDK: complete
Game Project Compiler: complete
Semantic Outcome Loop: complete
All future engine_* tools: complete
Every native AI host adapter: complete
Authority R5/R6 entire roadmap: complete
```

后续 R2/R3/R4 增加的能力只应作为新 ToolDefinition/domain handler 进入同一 Provider，不再改变
AI -> Host Adapter -> Provider -> Kernel -> Context/Compiler/Runtime 的顶层拓扑。

## 19. 风险与控制

| 风险 | 控制 |
| --- | --- |
| 改动面大，一次切换难以定位失败 | 一份施工文档内部分 Gate，每 Gate 可独立打红；不发布中间拓扑 |
| Provider 变成巨大 Host | Provider 保持小 Interface；领域执行留在 Project/Runtime/Build owner |
| 新 Provider 只包装旧 Gateway | F1/F10 强制删除 Editor/Gateway 依赖与生产达性 |
| 中立 Compiler 变成重写完整 R3 | 只迁移当前 Assembler/Player/Windows dev delivery 既有实现和实际依赖闭包，不新增增量图、Project SDK或新平台 |
| 为了 Headless 大量重写 editor_core | ownership move + Editor Adapter 回接；新 Interface 测试通过即删除旧 owner，禁止双份 pipeline |
| 中立 crate 仍暗中依赖 Editor schema | 静态 Gate 禁止所有 `editor_*` 依赖；只迁移 Assembler 实际调用的 canonical schema/纯 cooker |
| Preview 假 Headless | F6 必须通过真实 Runtime/Player，不调 Editor Play/GameView |
| MCP 权限语义弱于 native host | Host approval + Engine Grant 分层；无法安全投影的 elevated 能力 fail-closed |
| 多项目 session 串绑 | v1 每 ProviderSession 单 active project；operation/receipt 绑定 canonical identity |
| warm cache 成为第二真相 | 每次语义调用 refresh/revalidate；正确性只依赖 storage/Context |
| 旧客户端兼容拖延双架构 | 不保留运行时 fallback；一次切换安装目标 |
| 施工顺便扩张到未来全部工具 | 只迁移现有 capability set 及 No-Editor 所必需的执行链 |
| 把框架完成误报为产品能力全完成 | 按 Provider/Adapter/tool maturity 分层报告，不跳过 R2-R4 |

## 20. 施工文档的范围要求

本正式方案已由用户确认，并已通过唯一施工文档完成、自审和归档：
`施工文档/已完成/309-F-Headless-Engine-Tool-Provider-Atomic-Cutover-v1施工文档.md`。该文档保持：

1. 实际 crate/module 拆分与依赖图；
2. `EditorSession` 可达字段/方法迁移清单；
3. Gateway 能力迁移/删除的逐文件清单；
4. Tool contract 命名、schema、result 和 maturity 迁移清单；
5. ProviderSession/project locator/approval/operation 实现 Gate；
6. `project_authoring_execution::GameProjectCompiler` Interface，以及其内部唯一 Assembler 的中立 schema/cooker
   ownership move；Player/Desktop export owner 另在 F-C 按前置条件迁移，并由 Editor Adapter 回接；
7. Preview/Run/Build/Delivery 的 No-Editor 必要迁移；
8. MCP 真实 process 与 Codex 验收；
9. Editor composition 退役 Gateway 的清单；
10. F1-F12 每一 Gate 的定向测试、回归范围和失败证据；
11. 文档同步、完成记录和旧 Gateway 历史化规则。

施工文档不得将本次改动再拆为多份可独立激活的 A/B/C 生产方案，也不得在某个
中间 Gate 完成后宣称“309-F 部分生产切换完成”。

## 21. 完成定义

309-F 只有在以下条件全部成立时才能宣称完成：

1. F1-F12 全部通过；
2. 真实 Codex 在无 Editor 路径下直接调用具体 Engine tools；
3. Provider/Kernel 不依赖 Editor crates 或隐藏 `EditorSession`；
4. Context revision 是唯一 project revision authority；Gateway generation/observed digest 已从生产正确性中删除；
5. `GameProjectCompiler` 是 canonical source 到 prepared RuntimePackage/Player/Windows dev delivery 的中立
   高层 owner；其内部唯一 `ProjectRuntimePackageAssembler` 负责 `RuntimePackageBuildInput` 装配，Editor 与
   Provider 消费同一 Interface，旧 Editor owner和重复 pipeline 已删除；
6. 当前宣称 ready 的 run/build/delivery 工具不调用 Editor Play/GameView；
7. Native Provider Interface 与 MCP Adapter 消费同一 ToolDefinition/Kernel/Context 链路；
8. Editor 不再默认托管 Gateway，只作为可选 Projection；
9. discovery/named pipe/Editor instance 不存在于默认产品组合和安装配置；
10. 旧 Gateway 不是 fallback，旧内部状态测试已被新 Interface 行为测试替代；
11. 309-A 至 309-E 的 revision、snapshot、CAS、recovery、rollback、LWW 语义全部保持；
12. 文档明确区分“本次必要 Compiler subset 完成”与“完整 R3/R2-R5 未来能力完成”；
13. 已生成阶段完成记录，上级 309、文档地图、阅读顺序、49/54 和施工入口使用同一状态。

## 22. 方案自审

### 22.1 与权威产品需求一致性

- AI 看到具体 `engine_*` tools，与普通文件/命令工具在同一 Agent Tool Loop：通过。
- Provider 不是第二 Agent，不拥有计划和模型循环：通过。
- 默认完整路径无 Editor：通过，并由 F2/F6/F10/F11 强制验收。
- MCP 只是 Adapter，不是 AI 可见二级 Host：通过。

### 22.2 与 309-A 至 309-E 一致性

- Context 仍是唯一 revision/mutation authority：通过。
- snapshot lease、CAS、journal、recovery、receipt、rollback 不降级：通过。
- 完整单文档 LWW 与结构化 CAS 区分保留：通过。
- 不建立常驻 Project Authority Daemon：通过。

### 22.3 Module 深度与依赖方向

- Provider 隐藏多宿主重复复杂度，删除后复杂度会散落：是，符合深 Module。
- Project Authoring Execution Module 同时有 Headless Provider 和 EditorProjectAdapter 两个真实 consumer：是，
  seam 不是为测试虚构。
- Game Project Compiler/Build Module 通过 `prepare/run/build/verify` 隐藏内部唯一 Assembler、cooker、player
  staging、export 与 verification 复杂度；Assembler 只消费 lease-owned SourceView，Editor 与 Provider 是两个
  真实 consumer：是，符合深 Module。
- 方案是替换旧 Gateway，不是在旧 Gateway 上叠层：通过。
- 没有为每个内部 helper 强制建 crate/trait：通过。

### 22.4 过量施工控制

- 一次切换顶层拓扑，但只迁移现有工具可达能力：通过。
- 不将未来 R2-R4 全部能力并入 309-F：通过。
- F-B 只迁移现有 Assembler 及其实际中立 schema/cooker 闭包；F-C 才迁移 Player/Windows dev delivery；不新建第三 crate、
  不扩 Renderer/ECS/AUI/Asset 功能、不做移动端或完整 R3：通过。
- 只生成并激活唯一施工文档；当前授权仍受施工文档 Gate 和禁止范围约束：通过。

### 22.5 用户已确认的最终边界

1. 上级 309 Stage F 从狭义 authority retirement 正式扩大为 C-Atomic Provider cutover；
2. 旧 Gateway 不作为发布后 fallback；
3. 一份施工文档/一个施工窗口完成，内部仍使用 Gate 验证；
4. 方案 B 已确认：中立 Game Project Compiler/Build owner 是 `project_authoring_execution` 内部深 Module，
   Compiler 内部由唯一 Assembler 负责 `RuntimePackageBuildInput` 装配，不新增第三个 crate，Editor 与 Provider
   消费同一 Interface；
5. 309-F 只冻结最终框架、迁移当前 tool set 和必要 Compiler subset，不假报完整 R3 或 R2-R5 未来全能力完成。

### 22.6 F-B 混合模块拆分边界修订（2026-09-01）

实施前的依赖复核发现，当前若干 Assembler 直接依赖并非纯 schema/cooker：Scene/Prefab/Input 引用
`editor_ui_model`，Observation 引用 `EditorSession` 与 live filesystem，Asset Import/Font cook 同时
包含 Editor workflow。因此“迁移纯部分”必须具体解释为以下 ownership 规则：

| 当前混合模块 | F-B 中立 owner 允许迁移 | 明确留在 Editor Adapter 的部分 |
| --- | --- | --- |
| Scene | canonical Scene document/entity/transform/component 数据结构、lease bytes parse、runtime conversion | draft、selection、undo、commands、stage state、UI model vector types |
| Prefab | Prefab asset/instance/override schema、resolve/bake 纯算法 | Prefab stage、编辑命令、Inspector/selection、workflow report UI |
| Input | mapping JSON schema、path discovery over SourceView、runtime mapping conversion | input editor state、UI command、editor interaction |
| Observation | source mapping/contract schema 与 snapshot bytes 解析 | `EditorSession` 查询、live project scan、Editor observation index |
| Asset Import | Asset meta/registry/graph canonical read schema、PNG/texture decode/cook | 下载、审批、mutation、import session、thumbnail UI |
| Font | embedded pack、font metadata/atlas/bundle cook 使用的 lease bytes | 字体面板、编辑器缓存控制、交互式 profile workflow |

中立实现位于现有 `project_authoring_execution` crate 内部，不新增第三个 crate，也不通过
`project_authoring_execution -> editor_core/editor_ui_model` 规避依赖方向。Assembler 只接受
`CompilerSourceView` 和上述中立数据；需要路径的 toolchain 只能使用与 lease identity 绑定的内部
materialization，不能把 live filesystem 重新变成 authority。Editor 对外保留的类型只能是 Adapter
转换，不得继续被 Compiler 或 Provider 直接引用。

该修订不改变 F-B/F-C 分界：Player、staging、Desktop export、delivery verify 仍属于 F-C；
`RuntimePackageBuilder`、`runtime_cli` 与 `engine_runtime` 仍保持原 owner。它只补充 F-B 的可执行
拆分规则，作为后续五步施工的必要前置，不构成 F-B 完成声明。
