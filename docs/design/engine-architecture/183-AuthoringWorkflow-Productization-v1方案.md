# 183-AuthoringWorkflow Productization v1 方案

## 1. 系统是什么

`AuthoringWorkflow Productization v1` 是对现有编辑器创作流程主线的产品化收敛。

它不是新增一套 Walkthrough / Operation / Task 执行业务层，而是把已经存在的 `AuthoringWorkflowModel` 做成真正可点击、可执行、可诊断、AI 可读取的项目创作主流程。

它回答的问题是：

```text
用户打开项目以后，编辑器如何明确告诉用户：
当前项目做到哪一步？
还缺什么？
下一步应该点哪个操作？
这个操作是否真的可执行？
失败以后应该去哪个域修复？
AI 应该读取哪份结构化上下文继续补项目？
```

本系统服务于复杂打飞机项目的真实编辑闭环，但不增加任何打飞机专用 API。

## 2. 为什么不新增 Authoring Operation Walkthrough Spine

前一轮讨论中曾考虑建立：

```text
AuthoringWalkthrough
AuthoringOperation
DomainOperationCatalog
NextActionPlan
```

审查现有代码后确认，这会和现有模型重复：

```text
AuthoringWorkflowModel
AuthoringWorkflowStep
AuthoringTask
AuthoringCommand
AuthoringAiContext
AuthoringWorkflowComposer
```

这些现有类型已经承担了创作步骤、推荐任务、可用命令、阻塞问题、AI 上下文的职责。

因此正式规则是：

```text
不新增 AuthoringOperation 层。
不新增 Walkthrough Spine 层。
不新增 DomainOperationCatalog 层。
不新增第二套任务执行系统。
```

长期路线应收敛为：

```text
ProjectAuthoringWorkspaceModel
  -> AuthoringWorkflowComposer
  -> AuthoringWorkflowModel
      -> steps
      -> recommended_tasks
      -> available_commands
      -> blocking_issues
      -> ai_context
  -> UI clickable workflow
  -> UiCommandPayload / ProjectPatch
  -> EditorSession
```

## 3. 与现有系统关系

本系统基于以下已确认系统继续推进：

```text
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
131-M1-Project-Authoring-Workspace-v1方案.md
150-AI-first-Editor-Command-Framework-C-min方案.md
175-Complex-Shooter-Authoring-Workflow-v1方案.md
181-M16-AI-Project-Patch-Entry-C-min方案.md
```

本系统不是替代 `175`，而是对 `175` 的收敛修正和产品化继续：

```text
175 定义了创作工作流主线。
183 明确不再新增重复 Operation 层，并要求把已有 AuthoringWorkflowModel 变成真实可用流程。
```

## 4. 其他引擎对比

### 4.1 Unity

Unity 没有显式的 `AuthoringWorkflowModel`。

Unity 的创作流程依靠以下 UI 和命令隐式组成：

```text
Project Browser
Hierarchy
Scene View
Inspector
Play Button
Build Settings
Console
```

优点是成熟、简单、用户熟悉。

缺点是流程不是结构化数据，AI 很难直接知道“当前项目下一步应该做什么”。

### 4.2 Unreal Engine

UE 的编辑器更接近命令化系统：

```text
Level Editor
Content Browser
Scene Outliner
Details Panel
CommandList
Transaction
Editor Mode / Tool Framework
Message Log
```

UE 强在命令、事务、工具模式和编辑器扩展能力。

缺点是体系很重，不适合第一版直接完整照搬。

### 4.3 Godot

Godot 的编辑流程更集中：

```text
Scene Tree
Inspector
FileSystem Dock
Run
Export
Output / Debugger
```

优点是路径清晰，用户容易理解。

缺点是对大型复杂编辑器、AI 结构化协作的长期能力不如 UE 式命令体系。

### 4.4 我们的选择

我们采用：

```text
Unity / Godot 的简单创作路径
+ UE 的命令化、事务化、可诊断思想
+ AI 可读取的显式 AuthoringWorkflowModel
```

不采用：

```text
Unity 的纯隐式流程
UE 的完整重型 Command / Tool Framework
额外新增一层 Operation Spine
```

## 5. 正式方案

采用方案 C-min：

```text
AuthoringWorkflow 产品化，不新增业务层。
```

核心结构保持：

```text
ProjectAuthoringWorkspaceModel
  负责汇总 Project / Asset / Scene / Prefab / Rule / Input / AUI / Play / Build / Report 域状态。

AuthoringWorkflowComposer
  负责从 Workspace 状态派生 AuthoringWorkflowModel。

AuthoringWorkflowModel
  是编辑器创作流程状态的唯一真相层。

AuthoringWorkflowStep
  表示流程节点。

AuthoringTask
  表示推荐下一步。

AuthoringCommand
  表示可触发的正式编辑器命令引用。

AuthoringAiContext
  表示 AI 可读取的结构化创作上下文。
```

## 6. 第一版规则

### 6.1 真相层规则

`AuthoringWorkflowModel` 是编辑器创作流程状态的唯一真相层。

不允许再新增同类真相层：

```text
AuthoringOperation
AuthoringWalkthrough
DomainOperationCatalog
NextActionPlan
```

如果后续确实需要更强能力，应优先扩展：

```text
AuthoringWorkflowStep
AuthoringTask
AuthoringCommand
AuthoringAiContext
```

### 6.2 执行规则

Workflow 不直接执行业务逻辑。

所有执行必须进入正式命令入口：

```text
AuthoringCommand
  -> UiCommandPayload / ProjectPatch
  -> EditorSession
  -> CommandTransaction / Domain Service
```

不允许 UI 面板、AI Panel、Workflow 面板绕过 `EditorSession` 直接改项目状态。

### 6.3 命令规则

每个 `AuthoringWorkflowStep` 最多有：

```text
1 个 primary_command
少量 secondary_commands
```

避免把 workflow 面板变成按钮堆。

每个 `AuthoringCommand` 必须明确：

```text
command_id
domain
label
availability
payload_kind
```

如果命令暂时无法真实执行，必须标记为：

```text
Disabled
```

或在 UI 上显示为不可用。

不允许伪装成可点击命令。

`AuthoringCommand` 第一版不携带完整可执行 payload。

它只表达：

```text
这个 workflow step 建议触发哪个正式编辑器命令。
```

真正执行时必须由命令解析器结合当前编辑器上下文生成正式命令：

```text
AuthoringCommand.command_id / payload_kind
  -> WorkflowCommandResolver
  -> UiCommandPayload
  -> EditorSession
```

如果一个命令需要用户补参数，例如实体名称、资源路径、目标父节点、字段值，Workflow 不负责收集这些参数。

这类命令必须采用以下策略之一：

```text
打开或聚焦对应 Domain 面板，让用户在面板中完成参数输入。
使用 Domain 已定义的安全默认值，并且该默认值必须可回滚、可诊断。
标记为 Disabled，直到对应 Domain 提供可执行入口。
```

禁止把 `AuthoringCommand` 扩展成通用参数收集器。

这样可以避免 Workflow 变成第二套 Inspector / Wizard / ProjectPatch 编辑器。

### 6.4 推荐任务规则

`recommended_tasks` 是用户和 AI 的统一下一步来源。

排序优先级：

```text
Failed / Blocked
Required Empty
Dirty / NeedsAttention
Optional Empty
```

推荐任务必须引用已有 `AuthoringCommand`，不能自己定义第二套执行 payload。

### 6.5 AI 规则

AI Panel 必须优先读取：

```text
AuthoringAiContext.active_step
AuthoringAiContext.missing_required_items
AuthoringAiContext.blocking_issues
AuthoringAiContext.recommended_tasks
AuthoringAiContext.available_commands
```

AI 不应再从多个面板状态里自行拼接“当前项目缺什么”。

AI 生成的修改如果涉及多步结构化项目变更，应进入 `ProjectPatch`。

AI 生成的单步用户动作，应进入 `UiCommandPayload`。

### 6.6 Domain 边界规则

Workflow 只负责引导和调度，不承载具体业务规则。

各 Domain 仍负责自己的内部逻辑：

```text
Project: open / create / recent project
Asset: import / browser / asset selection
Scene: open / create / place / select / edit
Prefab: create / instantiate / update
Rule: rule authoring / build / AOT
Input: input mapping authoring
AUI: UI document authoring / binding
Play: runtime package / preview
Build: export / package
Report: diagnostics / build report / console
```

Workflow 不能新增项目侧玩法概念。

禁止新增：

```text
Player
Enemy
Bullet
Health
Damage
Score
Wave
Weapon
Boss
Drop
```

这些必须由项目侧 Schema / Rule / Prefab / Asset / AUI / Input 表达。

## 7. 第一版需要补齐的能力

### 7.1 AuthoringCommand 可执行映射

补齐从 `AuthoringCommand` 到正式命令的解析链路：

```text
command_id / payload_kind
  -> WorkflowCommandResolver
  -> UiCommandPayload
```

`WorkflowCommandResolver` 只允许处理两类命令：

```text
无参命令
  例如 Play / Pause / ClearConsole / RefreshRecentProjects。

上下文足够明确的安全默认命令
  例如打开当前项目、打开当前选中的资源、创建默认 input mapping。
```

对于需要用户补参数的命令，resolver 不应伪造参数。

它应该返回：

```text
Disabled
FocusDomainPanel
OpenDomainEditor
```

由对应 Domain 面板继续完成真实参数输入。

对于需要多步结构化修改且当前已有 ProjectPatch 支持的命令，可以进入：

```text
ProjectPatch
```

但 ProjectPatch 只能作为结构化项目变更入口，不能变成 Workflow 的第二套业务执行器。

### 7.1.1 第一版命令覆盖范围

第一版先补齐核心域的可执行命令覆盖：

```text
Project
Scene
Input
Play
Build
Report
```

这些域至少需要满足：

```text
有 primary command。
命令 availability 与当前 workspace 状态一致。
无参或上下文足够明确的命令可以解析成 UiCommandPayload。
不可执行命令必须明确 Disabled。
```

以下域第一版可以只显示状态和进入面板，不强行补复杂操作：

```text
Prefab
Rule
AUI
```

如果这些域当前没有真实可执行入口，必须显示为：

```text
Disabled
FocusDomainPanel
NotImplemented
```

不能为了让流程看起来完整而伪造可执行操作。

### 7.2 Workflow UI 可点击

Workflow 面板里的以下内容必须可交互：

```text
step primary command
step secondary command
recommended task command
blocking issue suggested command
```

点击后必须生成正式 `UiCommandPayload` 或 `ProjectPatch`。

当前代码已有 `HitRegion / HitTarget / editor_input` 基础，并且已经存在 `AuthoringWorkflowStep` 的 hit region。

但第一版仍要区分两层验收：

```text
数据链路验收
  AuthoringCommand -> WorkflowCommandResolver -> UiCommandPayload / ProjectPatch -> EditorSession -> Workflow refresh。

UI 链路验收
  Workflow panel hit region -> editor_input route -> UiCommandPayload -> EditorSession。
```

施工时必须先让数据链路在 headless 测试中稳定，再接 UI hit region。

如果某类 workflow command 暂时没有 UI hit region，不能把它标记为可点击完成态，只能作为下一步施工项。

### 7.3 状态刷新

每次命令执行后必须刷新：

```text
ProjectAuthoringWorkspaceModel
AuthoringWorkflowModel
AuthoringAiContext
```

UI 和 AI 看到的下一步必须一致。

### 7.4 诊断反馈

Workflow 命令执行结果必须进入：

```text
Console
Report
Command feedback
```

失败时必须能定位：

```text
哪个 step
哪个 command
哪个 domain
失败原因
建议修复动作
```

## 8. 第一版验收路径

### 8.1 用户路径

必须先在 headless 数据链路中跑通：

```text
无项目
  -> active_step = Project
  -> recommended task = Open/Create Project
  -> 执行正式 UiCommandPayload
  -> Workspace 刷新
  -> Scene step missing
  -> recommended task = Open/Create Scene
  -> 执行正式 UiCommandPayload 或聚焦 Scene 面板
  -> Scene ready
  -> can_play / can_build 状态更新
  -> AuthoringAiContext 同步更新
```

在 UI 链路中至少跑通：

```text
Workflow step hit region
  -> editor_input route
  -> SetAuthoringWorkflowStep 或对应可执行 command
  -> EditorSession
  -> Workflow refresh
```

### 8.2 AI 路径

必须能跑通：

```text
AI Panel 读取 AuthoringAiContext
  -> 识别当前缺 Scene / Rule / Input / AUI 等结构化信息
  -> 生成建议、UiCommandPayload 或 ProjectPatch
  -> 用户接受
  -> 执行正式命令
  -> Workflow 状态刷新
```

AI 路径规则：

```text
单步用户动作优先走 UiCommandPayload。
多步结构化项目变更走 ProjectPatch。
ProjectPatch 当前只覆盖已经实现并验证的 operation。
不支持的 operation 必须返回诊断，不能绕过到临时代码。
```

### 8.3 回归路径

至少包含：

```text
无项目 workflow 测试
打开项目 workflow 测试
推荐任务生成测试
AuthoringCommand 解析测试
核心域 primary/secondary command 覆盖测试
Disabled command 不执行测试
需要参数的 command 聚焦 Domain 面板或返回 Disabled 测试
UI hit region -> workflow command 路由测试
EditorSession 执行后 workflow 刷新测试
AI context 内容测试
ProjectPatch supported operation 接入测试
```

## 9. 自审

### 9.1 是否合乎规格

合乎。

本方案围绕 `130` 中复杂打飞机编辑到 Windows 可玩项目缺失能力继续推进，并且解决的是用户真实创作流程，不偏离到测试系统或单个按钮。

### 9.2 是否合乎既定规则

合乎。

方案明确不新增打飞机专用 API，不新增第二套业务执行系统，不绕过 `EditorSession`。

### 9.3 是否合乎方案文字本身

合乎。

核心目标是产品化现有 `AuthoringWorkflowModel`，不是创建新的 Walkthrough / Operation 层。

### 9.4 是否合乎长期设计

合乎。

长期上保留一个清晰的编辑器创作流程真相层，同时保持 Domain 自治和命令事务化。

### 9.5 是否方便实现

合乎。

现有代码已经有 `AuthoringWorkflowModel`、`AuthoringWorkflowComposer`、`AuthoringCommand`、`recommended_tasks`、`HitRegion / HitTarget`、`editor_input` 路由基础和 `ProjectPatch` 基础。

第一版主要是补齐：

```text
WorkflowCommandResolver
核心域 command 覆盖
Disabled / FocusDomainPanel 规则
headless 数据链路测试
UI hit region 路由测试
状态刷新和 AI context 一致性测试
```

这比新增 Walkthrough / Operation 层更小，也更贴近现有代码。

### 9.6 是否合理且能实现

合乎。

本方案避免新增大框架，施工范围可控，并且可以按模块测试。

## 10. 结论

正式采用：

```text
AuthoringWorkflow Productization v1 / C-min
```

核心规则：

```text
不新增 Walkthrough / Operation 层。
产品化现有 AuthoringWorkflowModel。
所有点击和 AI 动作进入正式 UiCommandPayload / ProjectPatch / EditorSession 链路。
Workflow 只做创作引导和命令调度，不承载 Domain 业务逻辑。
```
