# AI First Game Engine 权威架构设计 v1.1

> 文档性质：顶层权威架构设计。
> 上位需求：`00-AI-First-Game-Engine-权威产品需求-v1.1.md`。
> 研究输入：`../DeepSeekHarness源码参考/AI-First-Game-Engine-Harness-Native-Game-Project-Compiler-研究方案-v0.3.md`。
> 生效日期：2026-08-31。
> 最近修订：2026-09-12；明确独立 typed Engine tools 必须直接注册到宿主 Tool Registry，禁止统一 MCP 大工具转发。
> 当前状态：已按用户确认生成，作为新的唯一顶层架构 authority；不授权代码施工。
> 适用原则：下层方案、代码、工具、Editor、Runtime、Build 和测试与本文冲突时，必须先修订下层设计，不能反向削弱本文。

## 0. 文档权威性

### 0.1 本文决定什么

本文冻结以下顶层事实：

- 产品默认入口和用户心智；
- AI、Engine Tool Provider、项目、Runtime、Build、Editor 之间的调用关系；
- 项目 authority、mutation、revision 和派生产物的所有权；
- 核心深 Module、Interface 和 seam；
- No-Editor 默认完整路径；
- 旧架构的继承、降级和退役方向；
- 后续正式子设计与施工准入条件。

本文不冻结未经子设计验证的 crate 名、进程名、具体 Rust 类型布局、命令行参数或绝对性能数字。

### 0.2 权威顺序

发生冲突时，使用以下顺序：

```text
权威产品需求
-> 本权威架构设计
-> 经确认的正式子系统方案
-> 施工文档
-> 代码、schema、测试和运行证据
-> 研究文档、实验报告和历史记录
```

研究文档可以提出修改，但不能自动改变正式架构。施工结果推翻设计时，必须先回填正式设计，再继续扩大实现。

### 0.3 不授权施工

本文生效只表示顶层方向和所有权确定，不表示可以直接修改产品代码。任何代码施工仍必须经过：

```text
正式子设计
-> 子设计自审
-> 施工文档
-> 施工文档自审
-> 激活为唯一当前施工
-> 分 Gate 实施与验证
-> 完成记录和归档
```

## 1. 一句话架构

AI First Game Engine 是面向 AI 原生编程方式的游戏项目编译、运行、观测和交付平台：AI继续像直接开发普通代码项目一样编写游戏代码、UI、资源和工程内容，并通过与 `read_file`、`write_file`、`run_command` 同级的具体 Engine tools 调用引擎深能力；引擎不要求 AI 连接或模拟传统 Editor，也不在内部建立第二个 Agent。

## 2. 顶层架构决定

### AD-001：同一个 Agent Tool Loop

用户目标只交给宿主 AI Agent。该 Agent 拥有：

- 用户意图理解；
- 任务分解和动态规划；
- 代码、内容和测试生成；
- 文件工具与 Engine tool 的选择；
- 对 ToolResult 的判断；
- 修改、重试、换方案和停止决定。

Engine Tool Provider 不理解完整对话，不复制模型循环，不替 AI 制定玩法，不拥有任务计划。

### AD-002：Engine tools 是一等宿主工具

AI可见的默认 Tool Registry 只包含具有独立模型决策价值的具体工具。v1目标工具族为：

```text
read_file
write_file
run_command
engine_project_inspect
engine_project_check
engine_project_mutate
engine_project_rollback
engine_runtime_run
engine_runtime_playtest
engine_runtime_observe
engine_project_build
engine_delivery_verify
```

`engine_runtime_observe`以typed请求覆盖语义观察和视觉捕获。`prepare`、refresh、snapshot、generated glue和RuntimePackage装配是工具内部阶段，不注册为默认工具。普通文件search/read、证据文件读取、浅文本references/symbols、项目open以及同步operation observe/cancel不得与宿主已有能力重复注册；长任务取消优先映射宿主call cancellation，确需跨turn异步operation时才按能力动态提供生命周期控制。

最终工具名称由后续 Tool Provider 正式子设计冻结。无论名称如何，默认 Interface 都不得退化为：

```text
engine_catalog()
engine_execute(toolId, payload)
```

内部 Tool Kernel 可以保留 Registry、catalog 和 dispatch 实现，但宿主 AI看到的必须是稳定、具体、typed 的同级工具。

本条是强制调用拓扑：宿主 AI 必须直接看到并分别调用每个 `engine_*` 工具。禁止将这些工具折叠为单一 MCP/Gateway 工具，再由 MCP/Gateway 通过 `toolId`、`operation` 或字符串路由二次分发。MCP 只能把同一组独立 typed tools 作为兼容投影提供给只支持 MCP 的宿主；它不是 Engine 工具的统一上游或第二级 Agent。

### AD-003：No-Editor 是默认完整路径

以下能力链必须在 Editor 未安装、未启动、未连接、未聚焦时成立；箭头表示可用阶段关系，不表示每次任务必须线性调用全部工具：

```text
CLI/模板创建项目
-> 编写和修改游戏
-> 导入资源
-> 检查项目
-> 准备运行产物
-> 运行和 Playtest
-> 语义观测和视觉捕获
-> 修复
-> Windows / Android 构建
-> 交付复验
```

真实 Runtime window、GPU worker、Player、模拟器和设备不是 Editor。它们可以按工具请求启动，但不能获得项目 authoring authority。

### AD-004：Editor 是可选 Projection

Editor可以保留，用于人工查看、可视化验证、精修、调试和证据展示。Editor满足同一 Project Interface 的 Adapter，并读取同一项目真相。

Editor不得成为：

- AI身份 owner；
- 项目唯一 authority；
- Engine tools 的前置进程；
- Build、Runtime 或 Observation 的必经中间层；
- 独立的 mutation、receipt 或 cache 真相。

### AD-005：AI继续直接写真实游戏

项目代码、测试、配置、声明式 UI 和文本资产是一等 authoring surface。引擎不能把所有创作压缩成对象 CRUD，也不能要求 AI只填写一套封闭 Feature Spec。

Feature Spec、WorkItem、Patch Plan 和可视化表单是按风险和任务复杂度选择的治理工具，不是所有修改的强制公开步骤。

### AD-006：Native Runtime 和跨平台交付

Rust Native Runtime 继续是唯一正式 Runtime。RuntimePackage 继续是发布运行输入真相。项目侧代码和资产必须能够进入 Windows、Android 以及后续移动平台的真实 Player 和交付产物。

Web可以作为目标平台或实验 Adapter，但不能成为正式引擎 Runtime 的默认替代。

### AD-007：质量来自深能力和结果闭环

Harness、MCP 或 native tool projection 只缩短调用路径，不能自动提高游戏质量。长期差异化必须来自：

- Project Game SDK；
- Game Project Compiler；
- 通用深 Engine Capability Modules；
- Semantic Runtime Observation；
- Outcome Evaluation Loop；
- AI Development Pack；
- Native Build and Delivery。

## 3. 产品默认工作流

### 3.1 从空白创建游戏

```text
用户：做一个打飞机游戏
-> Codex读取随版本交付的Skill，获得条件化引擎工作流
-> CLI/模板建立最小项目；Codex编写Rust、AUI、资源描述和测试
-> engine_project_inspect / engine_project_check按需返回项目事实和低成本诊断
-> engine_runtime_run内部完成refresh、check、prepare并启动真实Native Runtime
-> engine_runtime_playtest按需执行输入时间线
-> engine_runtime_observe按需返回语义或视觉证据
-> Codex根据阶段结果继续修改、跳过无关阶段或重试失败阶段
-> engine_project_build生成Windows / Android包
-> engine_delivery_verify对打包产物重新运行和取证
```

用户不需要先创建 Editor session，不需要查询二级 Catalog，不需要把需求交给 Engine Agent。

### 3.2 修改已有游戏

小型、低风险、文本化修改可以直接使用普通文件工具。下一次 Engine tool 调用必须先把当前项目内容解析成新的 canonical revision，并返回任何 drift 或 schema 错误。

跨资源重命名、删除、引用替换、生成目录发布、高风险依赖变更等动作必须进入 Engine-owned mutation seam，获得影响分析、validation、receipt 和 rollback。

### 3.3 修复真实 Bug

```text
读取源码和现有证据
-> engine_runtime_run / engine_runtime_playtest 重现
-> engine_runtime_observe 获取实体、事件、输入和 source mapping
-> 修改最局部代码或内容
-> engine_project_check
-> 重放同一输入时间线
-> 比较 Outcome
-> 验证打包 Player
```

AI不能只以“编译通过”或“窗口打开”宣布游戏 Bug 已修复。

## 4. 总体结构

```text
User
  |
  v
Host AI Agent / Agent Tool Loop
  |-- ordinary file, search and command tools
  |-- concrete engine_* tools
  v
Host Adapter
  v
Engine Tool Provider
  |-- AI Development Pack projection
  |-- ToolDefinition projection
  |-- host approval translation
  v
Engine Tool Kernel
  |-- policy / Grant / operation / receipt / rollback
  |-- canonical ToolResult / diagnostics
  v
AuthoringProjectContext
  |-- Project Authoring Surface
  |-- Project Game SDK + Generated Runtime Glue
  |-- Asset DB / Project Assets / source snapshot
  |-- revision / digest / mutation lane
  v
Game Project Compiler
  |-- inspect / check / prepare
  |-- incremental Rust and asset production
  |-- RuntimePackage assembly
  v
Engine Execution Plane
  |-- Rust Native Runtime
  |-- Renderer / ECS / AUI / Input / Physics / Audio / Animation
  |-- Windows / Android / future platform adapters
  +------------------------+
  |                        |
  v                        v
Semantic Outcome Plane     Build and Delivery Plane
  |                        |
  +-----------+------------+
              v
      Canonical ToolResult

Optional Editor Projection
  -> EditorProjectAdapter
  -> same AuthoringProjectContext
  -> same Compiler / Runtime / Outcome / Build Interfaces
```

## 5. 核心深 Module

### 5.1 Engine Capability Definition Module

该 Module 是引擎能力的静态定义真相。它拥有版本化 ToolDefinition，包括：

- 工具标识和稳定语义；
- typed input/output schema；
- side effects 和风险分类；
- capability/version 需求；
- Summary/Trace 证据形状；
- 文档、示例和诊断知识引用；
- native projection 与 compatibility projection 元数据。

它不拥有动态项目状态、用户授权、Runtime readiness 或 operation 状态。动态 readiness 必须由真实能力 owner 在执行时重新检查。

### 5.2 Engine Tool Provider Module

该 Module 位于宿主 Agent Tool Loop 与引擎内部能力之间。它的外部 Interface只暴露具体 typed tools，隐藏：

- ToolDefinition 注册；
- host-specific tool schema 转换；
- host approval 与 Engine Grant 的组合；
- Tool Kernel invocation；
- operation observe/cancel；
- canonical result 到宿主结果的映射；
- 兼容层差异。

删除该 Module 后，这些复杂度会散落到 Codex、OpenCode、DeepSeek Harness、CLI 和 Editor Adapter，因此它必须保持深，而不是协议透传包装。

禁止把 Agent Planner、游戏生成模型循环、完整对话历史或用户目标状态放入该 Module。

Provider只投影模型决策控制点。一个内部阶段仅当AI可根据其独立结果选择停止、分支、修复或改变策略，并且该阶段具有独立成本、权限或生命周期时，才可成为默认tool。多个名称落到同一浅文本/透传Implementation时，必须合并或删除，而不是依靠命名伪造能力深度。

Engine拥有确定的阶段状态、前置条件、结果和局部合法流转；Skill向AI解释这些阶段如何组合；Host AI选择实际路径。三者不得相互替代。

### 5.3 Engine Tool Kernel Module

现有 253/255 的以下能力继续保留为内部正式基础：

- Tool Registry；
- capability readiness；
- bounded Grant；
- operation identity；
- canonical ToolResult；
- structured diagnostics；
- mutation receipt；
- rollback receipt；
- cancellation；
- drift fail-closed。

Tool Kernel可以拥有内部 `catalog/execute` Interface，但 Host Adapter 不能把该内部 Interface 原样暴露给 AI。

### 5.4 AuthoringProjectContext Module

该 Module 是项目 authoring authority。概念 Interface必须保持小，至少表达：

```text
open(projectLocator) -> ProjectHandle
snapshot(ProjectHandle, Scope) -> ProjectSnapshot
refresh(ProjectHandle, ExpectedRevision?) -> ProjectRevision
commit(ProjectHandle, ExpectedRevision, ProjectMutation) -> MutationReceipt
close(ProjectHandle)
```

方法名称和类型由后续正式子设计冻结。Interface必须隐藏：

- manifest、Asset DB 和 source inventory；
- canonical project identity；
- revision、generation 和 digest；
- file refresh/import；
- mutation serialization；
- drift detection；
- snapshot lease；
- cache invalidation；
- receipt lineage。

该 seam 具有两个真实 Adapter：

- `HeadlessProjectProvider`：默认 AI路径；
- `EditorProjectAdapter`：可选 Editor路径。

二者不能维护两套项目真相。

### 5.5 Project Authoring Surface Module

该 Module定义项目允许 AI和用户创作的稳定表面：

- Rust Project Framework 代码；
- Feature Folder；
- Project Assets；
- Scene、Prefab 和 Component 数据；
- AUI Document 和 binding；
- Input、Build Profile、Asset Spec；
- Contract-bound RuleSlot 和受限 Rule IR；
- 测试、场景和 playtest 输入时间线。

复杂 gameplay、算法和复杂 UI workflow 默认使用 Rust Project Framework。Canonical Rule IR 不是普通用户默认编辑对象，也不得扩张成另一套通用脚本语言。

### 5.6 Project Game SDK Module

Project Game SDK 是 AI写游戏代码时直接消费的稳定 Interface，不是新的 GDScript，也不是 Editor插件 ABI。

它必须提供高表达、低样板的项目侧能力，例如：

- 生命周期和 fixed update；
- WorldRead 和受控 deferred mutation；
- entity handle 与 generation；
- input action；
- AUI action 与 UiState；
- prefab spawn/despawn；
- audio、animation、physics 和 camera 意图；
- semantic observation publication；
-测试和 deterministic fixture。

生成胶水必须自动完成项目注册、ABI facade、RuntimeModule descriptor、capability declaration 和 source mapping。AI不应手工同步多份 descriptor、绑定表或派生代码。

现有 `ProjectRuntimeAbi` / `ProjectRuntimeSdk` 是实现基础，但新的正式子设计必须覆盖 Headless、Editor Preview、exported Player 和移动端的一致消费。

### 5.7 Game Project Compiler Module

该 Module 是项目 revision 到可运行产物的唯一高层准备 owner。概念 Interface：

```text
inspect(ProjectSnapshot, Scope) -> ProjectFacts
check(ProjectSnapshot, CheckProfile) -> CheckReport
prepare(ProjectSnapshot, TargetProfile) -> PreparedGame
```

它隐藏：

- schema validation；
- Rust Project Module 增量编译；
- generated glue；
- Asset import/cook；
- dependency graph；
- producer artifact cache；
- RuntimePackage assembly；
- target/toolchain readiness；
- source-level diagnostics；
- cancellation、lease、cache identity 和 deterministic publication。

`ProjectRuntimePackageAssembler` 继续是项目源进入 `RuntimePackageBuildInput` 的唯一正式装配入口，但它是
`GameProjectCompiler` 内部的唯一装配 Module，而不是与 Compiler 并列的第二个高层 owner。Assembler 必须消费
Compiler 从 operation-bound `ProjectSnapshotLease` 构造的 immutable `SourceView`；不得以裸
`project_root` 重新读取 live filesystem，也不得依赖 Editor workflow。Compiler 负责 project/revision/profile/cache/
diagnostics、lease identity 和 prepare 生命周期；Assembler 负责 neutral schema 读取、领域 cooker、Prefab bake、
source mapping 以及 `RuntimePackageBuildInput` 的唯一组装。Editor 与 Headless Provider 只能通过 Compiler Interface
使用它，不得各自持有装配实现。

Compiler不理解自然语言目标，不规划玩法，不判断游戏是否好玩。

### 5.8 Deep Engine Capability Modules

引擎能力必须按真实领域 owner 建设为深 Module，而不是按每个对象暴露浅 CRUD。长期至少包括：

- Scene / Prefab / ECS data；
- 2D / 3D Renderer；
- Camera / Lighting / Material / VFX；
- AUI；
- Input；
- Physics；
- Animation；
- Audio；
- Asset Pipeline；
- Runtime Debug / Profiling；
- Build / Export / Delivery。

ToolDefinition 是这些能力的投影，不是能力本身。缺少底层能力时必须返回 `capability_gap`，不能由 Provider 伪装实现。

具体游戏中的 Player、Enemy、Bullet、Score、Health、Wave、Weapon 等语义只能存在于项目侧，不能进入 Engine Core。

### 5.9 Engine Execution Plane

Rust Native Runtime 是唯一正式 Runtime。Runtime只消费 RuntimePackage和已绑定项目 RuntimeModule，不扫描项目源目录，不读取 Editor内存对象。

Editor Preview、Headless run、真实窗口 Player 和打包 Player必须共享同一运行语义、RuntimePackage schema和 Project Game SDK contract。允许平台 Adapter 不同，不允许项目逻辑 owner不同。

正式 Runtime trace 分档为 `Off / Summary / Trace`。普通发布默认 Off 或最小功能结果；Trace只在测试、诊断或用户显式请求时启用。

### 5.10 Semantic Outcome Module

该 Module让 AI观察“游戏发生了什么”，而不只读取日志或截图。概念 Interface：

```text
run(PreparedGame, RunProfile) -> RuntimeJob
observe(RuntimeJob, ObservationQuery) -> ObservationResult
playtest(RuntimeJob, InputTimeline) -> PlaytestResult
capture(RuntimeJob, CaptureRequest) -> VisualEvidence
replay(PreparedGame, ReplayInput) -> ReplayResult
```

它必须能够逐步覆盖：

- 稳定实体身份、transform 和关键状态；
- 输入时间线；
- gameplay event 和 collision event；
- AUI action 与可见 UI state；
- source mapping 和因果回溯；
- screenshot、frame、video 或像素证据；
- deterministic replay 或明确的非确定性说明；
- Summary/Trace 预算；
- packaged Player 复验。

现有 Project Observation Contract 可以作为紧凑 observation source，但不能独占整个 Outcome Interface。

### 5.11 Build and Delivery Module

该 Module拥有目标平台的确定性构建和交付复验。它必须隐藏 Rust toolchain、asset staging、RuntimePackage、Player assembly、签名、manifest、package layout 和设备运行复杂度，对 AI返回结构化 BuildReport 和 DeliveryVerificationResult。

Windows和Android是 v1 必须真实支持的资格平台。iOS、Web和其它平台按后续正式路线加入，但不能降低同一项目、RuntimePackage和 ToolResult语义。

### 5.12 AI Development Pack 版本化知识产物

该版本化产物解决模型对自研引擎缺少训练先验的问题。它与引擎版本一起交付：

- Project Game SDK文档和符号索引；
- Engine ToolDefinition和用例；
- Skills和任务级指导；
- 最小、复杂和多类型示例项目；
- 模板和生成器；
- 常见诊断、修复路径和迁移说明；
- Tool/SDK/RuntimePackage兼容矩阵；
- capability gap和平台差异说明。

知识包必须来自正式 Interface和真实测试，不能成为与代码漂移的第二套事实。

其中Skill拥有模型可读的条件化工作流说明：阶段目的、进入/跳过条件、预期ToolResult、失败恢复和局部下一步。Skill不执行工具、不保存运行状态，也不将项目目标转换为固定计划。

### 5.13 Optional Editor Projection Module

Editor只通过正式 Interface消费项目、Compiler、Runtime、Outcome和Build能力。它可以拥有窗口、dock、selection、viewport、gizmo和展示状态，但不能拥有第二套项目或运行真相。

Editor内部未保存的草稿必须明确标记为局部 draft。在提交到 AuthoringProjectContext 前，它不能影响 Headless Build、Runtime或AI看到的 canonical revision。

## 6. Authority 所有权

| 事实 | 唯一 owner | 非 owner |
| --- | --- | --- |
| 用户目标、AI计划、工具选择 | Host AI Agent | Engine Provider、Editor、Compiler |
| Tool静态合同 | Engine Capability Definition Module | 各 Host Adapter、Editor按钮 |
| Tool执行语义 | Engine Tool Kernel和领域能力 owner | MCP/Codex Adapter |
| 项目 identity/revision/digest | AuthoringProjectContext | Editor memory、Gateway cache、AI session |
| 项目源码和Project Assets | canonical project storage + AuthoringProjectContext | RuntimePackage、Library cache |
| RuntimePackage装配 | GameProjectCompiler 内部的唯一 ProjectRuntimePackageAssembler | Scene/Prefab各自临时 exporter、Editor/Provider 自有 assembler |
| 项目准备结果 | Game Project Compiler | Editor open流程、Host Adapter |
| 正式运行语义 | Rust Native Runtime | Editor UI、Web临时引擎 |
| Outcome证据 | Semantic Outcome Module和真实Runtime owner | AI文字推测、截图单独猜测 |
| 构建和交付报告 | Build and Delivery Module | Editor面板、Host Adapter |
| UI显示状态 | Optional Editor Projection | 项目和Runtime authority |
| Agent知识投影 | AI Development Pack | 独立手写Wiki |

任何设计如果让两个 owner 同时维护同一事实，必须拒绝或先定义明确的派生关系和失效规则。

## 7. 普通文件工具与 Engine mutation

### 7.1 直接写文件是正式能力

AI可以使用宿主普通文件工具修改项目根内的文本代码和允许直接编辑的项目内容。引擎不得要求所有修改都转换为通用 `project.mutate(goal)` 黑盒调用。

### 7.2 canonical revision提交规则

文件写入本身不是新的引擎 revision receipt。下一次需要引擎语义的操作必须通过 `AuthoringProjectContext.refresh` 或等价内部步骤：

```text
读取受控项目输入
-> path containment和source inventory
-> schema/import检查
-> 计算canonical revision/digest
-> 更新Asset DB和依赖失效
-> 返回结构化诊断
```

文件 watcher只能提示“可能变化”，不能成为 authority。mtime不能替代内容 identity。

### 7.3 必须使用 Engine mutation的操作

以下操作必须进入统一 mutation seam：

- 删除、移动、重命名稳定身份对象；
- 批量引用替换；
- 影响分析后的资源替换；
- schema migration；
- 依赖和toolchain变更；
- generated artifact发布；
- 高风险平台配置、签名和发布；
- 需要原子跨文件提交的结构化修改。

这些操作必须绑定 expected revision，并返回 changed domains、after revision、diagnostics、receipt和可用 rollback。

### 7.4 并发和drift

Headless AI、可选 Editor和外部文件工具可能同时触碰项目。正式子设计必须实现：

- snapshot lease或等价读一致性；
-单一mutation lane；
- expected revision compare；
- commit前recheck；
- 未知写入fail-closed；
- clean save no-write；
- drift后重新读取，不能自动重放旧mutation。

## 8. Host Adapter 与兼容策略

### 8.1 Native tool projection优先

当Codex、OpenCode或其它宿主支持本地原生typed tool provider时，优先直接注册具体Engine tools。

### 8.2 MCP是兼容Adapter

当宿主只支持MCP时，可以使用MCP stdio或等价本地协议，但必须仍向AI呈现具体typed tools。MCP不得把默认体验退化成先连接Editor或调用通用Host。

### 8.3 Host无关语义

所有Adapter必须通过同一conformance suite证明：

- 相同ToolDefinition；
- 相同输入验证；
- 相同Grant和side-effect语义；
- 相同canonical result；
- 相同operation/cancel/receipt/rollback语义；
- Host差异只存在于注册、批准UI和传输层。

### 8.4 批准与Grant

Host approval负责宿主级用户交互和本机权限提示；Engine Grant负责引擎 mutation seam的可审计授权。Adapter应在一次用户决定下组合两者，避免对同一风险重复询问。

AI direct input不得包含可伪造的用户身份、Grant、project digest或内部authority字段。

## 9. ToolResult 和诊断

所有Engine tools返回canonical result，至少能够表达：

```text
status
project identity / revision
operation identity
summary
typed outputs
diagnostics[]
evidence refs[]
changed domains[]
receipt / rollback ref
retryability
next-action categories
workflow stage / stage results
recommended local transitions
```

默认Summary只返回当前调用需要的紧凑字段；project/revision、operation、lineage和完整阶段细节可在正确性需要时返回，或进入Trace/evidence，不能要求AI每次处理全部内部身份。

`recommended local transitions`可以指出由当前确定事实支持的下一工具、条件和原因，例如检查失败后修复源码、Runtime启动后执行输入时间线、构建完成后复验交付。它不能根据完整用户目标生成跨任务计划，也不能用不可跳过的固定菜谱替AI决定下一步。

诊断必须尽量包含source path、symbol/asset id、stage、reason、expected/actual和可执行修复信息。禁止只返回长日志、退出码或“失败”。

## 10. Project Game SDK 与数据模型规则

- Scene是Entity tree，不是ECS snapshot。
- Component是纯数据；复杂行为进入Rust Project Framework。
- ECS结构变化走deferred command，不暴露裸World mutation。
- 运行时引用使用handle + generation + diagnostics，不暴露裸entity index。
- AUI Document不保存运行时值；binding只读ProjectUiStateSnapshot。
- Runtime Renderer只消费Projection产物，不读取项目source或AUI binding path。
- 所有资源使用GUID/meta/AssetRef，不能以path-only作为长期身份。
- RuntimePackage是运行输入真相，Project Assets是authoring真相，Library/cache是可删除派生产物。

## 11. 增量准备和性能

“AI写得更快”必须同时缩短：

- 首次可运行时间；
- 修改到反馈时间；
- 重复项目准备时间；
- 失败定位时间；
- 构建和交付复验时间。

Game Project Compiler必须使用明确依赖和内容identity进行增量失效。不得因为Editor UI、Host Adapter或无关Runtime implementation变化而重建全部项目。

缓存命中只影响性能，不影响正确性。缓存可删除并由source、schema、toolchain和依赖确定性重建；命中必须验证artifact envelope和digest。

## 12. Semantic Outcome 和游戏质量

结果验收至少分为四类：

| 类型 | 核心问题 |
| --- | --- |
| Technical Outcome | 是否编译、运行、无崩溃、性能是否达标 |
| Gameplay Outcome | 输入、规则、状态、胜负和反馈是否正确 |
| Visual Outcome | 构图、可读性、动画、UI、资源和视觉一致性是否达标 |
| Delivery Outcome | 打包产物是否能在目标平台安装、启动、游玩和复验 |

任何单一类型通过都不能代替其它类型。自动化证据不能完全替代主观画面和玩法审核；人工审核也不能替代可复现技术证据。

## 13. 安全、隐私和可恢复性

- 所有项目写入必须受SafeProjectPath或等价正式owner约束。
- 默认不允许项目根外写入、任意shell、任意网络、任意Engine Core修改。
- 外部费用、发布、签名、依赖、删除和高风险平台操作需要显式提升授权。
- ToolResult、receipt和evidence不得记录API Key、隐藏推理或无必要的完整prompt。
- rollback是确定能力，必须检查当前revision和drift，不能用旧snapshot覆盖未知新修改。
- operation必须可观察、可取消、有界清理，并在宿主或Adapter断连后保持明确终态。

## 14. 版本和兼容

以下对象独立版本化：

- Engine ToolDefinition；
- Host Adapter protocol；
- Project Game SDK；
- ProjectRuntimeAbi；
- Project schema和Asset schema；
- RuntimePackage；
- Observation/Outcome schema；
- Build/Delivery report；
- AI Development Pack。

必须提供兼容矩阵和迁移诊断。不能用一个全局engineVersion迫使所有无关artifact同时失效。

## 15. 旧架构继承与历史化

### 15.1 直接成为历史的旧顶层文档

以下文档从2026-08-31起不再拥有产品或顶层架构authority：

- `历史文档/01-目标与核心原则-旧顶层权威-截至2026-08-30.md`；
- `历史文档/02-AI功能生成流程-旧顶层权威-截至2026-08-30.md`。

其内容只用于解释旧Editor-first和强线性Patch流程，不得作为新施工依据。

### 15.2 继续有效的基础

| 既有设计 | 新状态 |
| --- | --- |
| 195/196 Rust Project Framework与受限IR | 继续有效 |
| 253 Agent-Owned Planning / Tool Kernel / Grant / Receipt / Rollback | 继续有效，Kernel降为Provider内部基础 |
| 255 ToolDefinition/Registry/readiness | 继续有效，AI不需要二次Catalog调用 |
| RuntimePackage / ProjectRuntimePackageAssembler | 继续有效 |
| Rust Native Runtime / ECS / Projection / AUI / Asset / Build | 继续有效 |
| 292 ProjectRuntimeAbi/SDK | 作为Project Game SDK正式子设计的实现基础 |
| 264 Project Observation Contract | 作为Semantic Outcome的紧凑基础 |

### 15.3 被替代或降级的默认权威

| 既有设计 | 新状态 |
| --- | --- |
| 10中的Editor长期默认路线 | 仅保留实现历史和可选Editor技术参考 |
| 20 Editor工程结构 | 仅描述Optional Editor Projection和legacy shell，不是产品入口 |
| 252 Editor Intent/WorkItem/ChangeSet默认流程 | 降为可选复杂任务治理和Editor UX，不是每次修改前置 |
| 254 Editor-hosted Gateway + unique EditorSession | 默认拓扑被替代；Gateway可作为Host Adapter内部实现 |
| 256 Editor-instance identity | 只保留给EditorProjectAdapter，不是Headless身份 |
| 259 Editor/Gateway deep mutation binding | 保留mutation/Grant/rollback经验，authority迁移到AuthoringProjectContext |
| 309-F当前20项工具投影 | Provider/Context/Compiler实现继续有效；默认AI工具面按本次修订收敛，浅查询、重复读取、open和非必要operation工具不再由309-F列表反向定义 |

完成记录、失败记录和施工归档不被删除或伪造重写，继续作为历史证据。

## 16. 正式迁移阶段

### R0：Authority Cutover

- 权威产品需求和本文成为文档入口；
- 旧01/02历史化；
- 文档地图、阅读顺序、README和当前状态同步；
- 不修改代码。

### R1：Headless Project Authority

- 正式设计AuthoringProjectContext；
- 建立HeadlessProjectProvider和EditorProjectAdapter；
- 收敛普通文件修改、refresh、revision、mutation、lease和drift；
- 证明Editor不存在时可inspect/check。

### R2：Project Game SDK和Generated Runtime Glue

- 冻结最小稳定项目编程Interface；
- 复用并深化ProjectRuntimeAbi/SDK；
- 消除手工注册和descriptor胶水；
- 打通Headless、Editor、Player和Android consumer。

### R3：Game Project Compiler

- 建立inspect/check/prepare深Module；
- 将 `ProjectRuntimePackageAssembler` 作为其内部唯一装配 Module，复用现有 cache owner；
- 让 Assembler 只消费 lease-owned `SourceView` 和中立 schema/cooker，不读取 Editor draft 或 live filesystem；
- 建立增量Rust/asset准备和源码级diagnostics；
- 无Editor完成prepare/run。

### R4：Semantic Outcome Loop

- 扩展observation、input timeline、capture和replay；
- 建立Technical/Gameplay/Visual/Delivery Outcome；
- 打包Player可复验。

### R5：First-class Engine Tool Provider

- 具体typed tools直接注册到真实宿主Tool Registry；
- native provider优先，MCP作兼容Adapter；
- Host approval与Engine Grant组合；
- 通过real-host tool-call equivalence Gate。

### R6：Optional Editor Cutover

- Editor只通过EditorProjectAdapter使用同一authority；
- 移除默认Engine tool对Editor process/session的依赖；
- 防止长期双authority；
- 保留用户主动打开的验证和精修体验。

阶段顺序允许在正式子设计证明依赖可分离时小范围并行，但不能跳过R1的项目authority直接建立新的Provider黑盒。

## 17. 后续必须生成的正式子设计

按依赖顺序至少需要：

1. `Headless AuthoringProjectContext + Unified Mutation v1`；
2. `Project Game SDK + Generated Runtime Glue v1`；
3. `Game Project Compiler + Incremental Prepare/Run v1`；
4. `Semantic Runtime Observation + Outcome Acceptance v1`；
5. `First-class Engine Tool Provider + Host Adapter Conformance v1`；
6. `AI Development Pack + Capability Maturity v1`（版本化知识产物，不新增运行时Module）；
7. `No-Editor Qualification + Six-path Benchmark v1`；
8. `Editor/Gateway Authority Migration and Retirement v1`。

其中1是authority上游，2是项目创作Interface，3是准备owner，4是结果闭环，5只投影前四项已经稳定的Interface。Provider的Host投影原则已由本文冻结，但真实施工不能提前用Tool schema反向规定Engine Module形状。

## 18. 资格Gate

### G0：Authority Consistency

所有入口只把权威产品需求和本文列为顶层真相；旧01/02明确历史化；下层冲突有迁移状态。

### G1：No-Editor Full Path

Editor二进制不存在时，真实宿主AI能通过CLI/模板和默认工具族完成create/change/check/run/playtest/observe/build/verify/rollback；prepare、capture落盘和其它机械步骤不要求独立tool call。

### G2：First-class Tool Call

真实宿主Tool Registry直接列出本设计冻结的决策控制点工具。调用链中没有用户可见Editor discovery、通用Engine Host、二级catalog/execute或内部Engine Agent；也没有与普通文件工具重复的浅search/read/evidence工具。

### G3：Single Project Authority

Headless和可选Editor读取同一project identity/revision；并发修改、clean save和drift有确定结果；不存在Gateway cache或Editor memory第二真相。

### G4：SDK and Compiler Leverage

多类型项目使用同一Project Game SDK和Compiler，不需要项目手写ABI/descriptor胶水；局部修改只失效相关依赖。

### G5：Outcome Before Claim

每次完成声明同时具备对应Technical、Gameplay、Visual和Delivery证据，或明确标记尚未验证的类型。

### G6：Six-path Benchmark

在固定模型、需求、资源、时间、平台和审核规则下比较：

```text
AI + Web
AI + Unity
AI + Unreal Engine
AI + Godot
AI + 当前自研引擎
AI + 本目标架构
```

覆盖多类型游戏和create/change/repair任务，同时区分首次项目与重复项目。没有统一benchmark证据时，不得宣称已经综合超过成熟引擎。

## 19. 明确非目标

- 不重新实现一个Godot式传统Editor产品；
- 不建立数百个浅CRUD工具；
- 不建立第二个Engine Agent；
- 不把所有项目修改强制包装成Goal Mutation黑盒；
- 不删除现有Editor代码和历史证据；
- 不因为架构重置而重写已经正确的Runtime、Renderer、ECS、AUI、Asset和Build基础；
- 不在顶层文档中承诺当前尚未实测的性能优势；
- 不把打飞机、塔防或其它具体玩法写入Engine Core；
- 不由本文直接授权代码、production binary、真实配置或外部平台施工。

## 20. 最终不变量

```text
INV-A  AI始终拥有规划权；Engine Provider不是Agent。
INV-B  具体Engine tools与普通文件工具处于同一Agent Tool Loop。
INV-C  默认完整路径不依赖Editor。
INV-D  Headless和Optional Editor只有一个项目authority。
INV-E  AI可以直接写真实项目代码和内容。
INV-F  结构化高风险修改集中在统一mutation seam。
INV-G  Rust Native Runtime和RuntimePackage保持正式运行真相。
INV-H  Tool projection不能代替Engine Module Depth。
INV-I  游戏完成声明必须有Semantic Outcome和真实交付证据。
INV-J  旧能力按继承矩阵迁移，不以架构重置名义重复实现成熟基础。
INV-K  Skill公开条件化引擎工作流；Engine暴露决策阶段和局部合法流转；Host AI始终选择下一步。
```

违反任一不变量的下层方案不得进入施工。
