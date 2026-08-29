# 175-Complex Shooter Authoring Workflow v1 方案

## 1. 系统是什么

`Complex Shooter Authoring Workflow v1` 是编辑器里的复杂项目创作主流程。

它不是新的测试系统，也不是重新做 `M1 Project Authoring Workspace v1`。M1 已经把分散的 Project / Scene / Asset / Prefab / Rule / AUI / Input / Play / Build / Report 域收敛成统一 workspace summary。本系统要继续往上走一步，把这些域组织成用户和 AI 真正能顺着走的创作流程。

通俗说，它回答的问题是：

```text
用户打开编辑器之后，怎样一步一步做出一个复杂打飞机项目？
项目现在做到哪一步了？
还缺什么？
下一步该点哪里？
哪些内容能保存？
哪些内容能进入 Play？
哪些内容能 Build / Run？
失败后去哪里修？
AI 如何读取同一份结构化状态来继续补项目？
```

本系统只做编辑器创作流程主干，不把以下玩法概念做成引擎 API：

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

这些仍然属于项目侧，由 Project Schema / Rule / Prefab / Asset / AUI / Input 定义。

## 2. 当前已有基础

本系统基于以下已完成或已确认系统继续推进：

```text
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
131-M1-Project-Authoring-Workspace-v1方案.md
140-M7-Prefab-Workflow-Reusable-Authoring-Object-System-v1方案.md
141-M8-Schema-driven-Inspector-Details-System-C-full方案.md
142-M9-Asset-Browser-Productization-v1方案.md
143-M10-Input-Mapping-Authoring-Runtime-Productization-v1方案.md
144-M11-Physics2D-Collider-Authoring-Visualization-C-min方案.md
150-AI-first-Editor-Command-Framework-C-min方案.md
165-Complex-Shooter-Real-Project-End-to-End-Gate-v1方案.md
```

当前已经具备：

```text
Project Launcher / Open / Create / Recent Projects
ProjectAuthoringWorkspaceModel
WorkspaceDomainKind / WorkspaceDomainStatus / WorkspaceDomainSummary
Project Browser 基础
Hierarchy / Inspector / Scene Editing 基础
Schema-driven Inspector 基础
Prefab Workflow 基础
Asset Browser Productization 基础
Input Mapping Authoring 基础
AUI 文档 / Runtime Overlay 基础
Build Export panel
DesktopExportPipeline
RuntimePackageBuilder / Loader
Windowed Player / Runtime Present 基础
AI-first Editor Command Framework C-min
Console / Report / Trace 基础
```

当前不足：

```text
这些能力仍然像工具箱，不像完整编辑器流程。
用户无法从一个清晰流程判断项目是否完整。
AI 只能看到 domain summary，缺少“创作任务 / 下一步 / 阻塞原因”的结构化状态。
Play / Build / Report 没有和 Project / Scene / Rule / Asset 形成统一创作闭环。
复杂打飞机样例还不能作为真实 authoring walkthrough 被编辑器引导完成。
```

## 3. 系统边界

### 3.1 本系统负责

```text
统一复杂项目创作步骤
统一每个创作域的完成度
统一下一步任务
统一缺失能力和阻塞原因
统一用户命令入口
统一 AI 可读上下文
统一 Play / Build / Report 在创作流程中的位置
```

### 3.2 本系统不负责

```text
不实现新的渲染底层
不实现新的 Physics2D 求解器
不实现新的 Runtime Rule 执行器
不实现完整商业级视觉脚本编辑器
不实现完整 Unity Prefab override 全能力
不实现完整动画系统
不实现 gameplay 专用 API
不新增测试系统作为本阶段主目标
```

### 3.3 和 M1 的区别

M1 是 workspace data foundation：

```text
EditorUiModel
  -> ProjectAuthoringWorkspaceModel
  -> WorkspaceDomainSummary
```

本系统是 workflow productization：

```text
ProjectAuthoringWorkspaceModel
  -> AuthoringWorkflowState
  -> AuthoringWorkflowStep
  -> AuthoringTask
  -> AuthoringIssue
  -> AuthoringCommand
  -> AI-readable authoring context
```

M1 说明“有哪些域”；本系统说明“如何完成一个项目”。

## 4. 其它引擎源码参考

### 4.1 Unity

源码参考：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/ProjectBrowser/ProjectBrowser.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/SceneHierarchyWindow.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/Inspector/Core/InspectorWindow.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/PlayModeView/PlayModeView.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/BuildPlayerWindow.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/BuildPlayerWindowBuildMethods.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/GUI/WindowLayout.cs
```

Unity 的编辑器创作流程核心是：

```text
ProjectBrowser
  -> SceneHierarchyWindow / SceneView
  -> InspectorWindow
  -> Prefab / AssetDatabase / SerializedObject
  -> PlayModeView
  -> BuildPlayerWindow
```

关键观察：

```text
Unity 不是让每个面板成为独立真相。
Project / Scene / Inspector / Play / Build 形成用户心智上的连续流程。
WindowLayout 会打开 Inspector / Hierarchy / ProjectBrowser 等核心窗口。
BuildPlayerWindow 是正式产品化 Build 入口，不是散落在其它面板里的临时按钮。
```

对我们的启发：

```text
我们应该学习 Unity 的简单创作路径：
Project -> Assets -> Scene -> Inspector -> Play -> Build。
但我们的 AI 友好要求更强，所以需要显式 AuthoringWorkflowState。
```

### 4.2 Unreal Engine

源码参考：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/UnrealEd/Private/LevelEditorViewport.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/UnrealEd/Private/SEditorViewport.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/UnrealEd/Private/Kismet2/DebuggerCommands.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/MainFrame/Private/Frame/MainFrameActions.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/ContentBrowserData/Public/IContentBrowserDataModule.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/ContentBrowserData/Public/ContentBrowserItemData.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/MaterialEditor/Private/MaterialEditor.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/BehaviorTreeEditor/Private/BehaviorTreeEditor.cpp
```

UE 的编辑器创作流程核心是：

```text
MainFrame / LevelEditor
  -> TabManager / WorkflowCentricApplication
  -> ContentBrowser
  -> SceneOutliner / Viewport
  -> DetailsView
  -> CommandList / Transaction
  -> Play In Editor
  -> Cook / Package / BuildCookRun
```

关键观察：

```text
UE 很多编辑器不是单个面板，而是 workflow-oriented app。
TabManager 组织窗口，FUICommandList 组织命令。
DetailsView 不是业务真相，它只呈现选中对象的属性。
PlayWorldCommands 把运行控制纳入统一命令体系。
ContentBrowserDataSubsystem 把资源浏览收敛到统一数据入口。
```

对我们的启发：

```text
我们应该学习 UE 的 workflow spine + command ownership。
但不照搬 UE 的重型 Slate / Toolkit / Module 复杂度。
第一版只建立明确的 workflow state / task / issue / command 结构。
```

### 4.3 Godot

源码参考：

```text
<GODOT_SOURCE>/godot-master/godot-master/editor/editor_node.h
<GODOT_SOURCE>/godot-master/godot-master/editor/editor_node.cpp
<GODOT_SOURCE>/godot-master/godot-master/editor/editor_interface.cpp
<GODOT_SOURCE>/godot-master/godot-master/editor/docks/filesystem_dock.cpp
<GODOT_SOURCE>/godot-master/godot-master/editor/docks/scene_tree_dock.cpp
<GODOT_SOURCE>/godot-master/godot-master/editor/run/editor_run_bar.cpp
<GODOT_SOURCE>/godot-master/godot-master/editor/export/project_export.cpp
<GODOT_SOURCE>/godot-master/godot-master/editor/register_editor_types.cpp
```

Godot 的编辑器创作流程核心是：

```text
EditorNode
  -> FileSystemDock
  -> SceneTreeDock
  -> Inspector
  -> EditorInterface
  -> EditorRunBar
  -> ProjectExportDialog
  -> EditorPlugin
```

关键观察：

```text
Godot 有一个非常明确的中心节点 EditorNode。
EditorInterface 对外暴露编辑器能力。
EditorRunBar 统一 Play / Stop / Run current scene / Run custom scene。
ProjectExportDialog 统一导出配置和执行。
大量功能通过 EditorPlugin 接入，但中心流程不消失。
```

对我们的启发：

```text
我们的 EditorSession / EditorUiModel 可以承担类似 EditorNode / EditorInterface 的中心职责。
但必须避免把所有业务继续堆回一个大文件。
Workflow state 应该独立成清晰模块，而不是继续膨胀 EditorSession。
```

### 4.4 Bevy

源码参考：

```text
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_app/src/app.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_app/src/sub_app.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_ecs/src/lib.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_reflect/src/serde/mod.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_asset/src/lib.rs
```

Bevy 没有成熟官方 Unity/UE 式编辑器，但它给我们两个底层启发：

```text
TypeRegistry / Reflect 适合 schema-driven editing。
AssetServer / AssetLoader 适合资源生命周期和加载状态。
App / Schedule / World 适合运行时组织，不适合直接作为编辑器 UX。
```

对我们的启发：

```text
Bevy 是 Runtime / ECS / Asset / Reflect 的参考，不是编辑器工作流目标。
我们的编辑器体验不能停留在 Bevy examples 级别，必须有 Unity/UE/Godot 式创作路径。
```

## 5. 方案选择

### 5.1 方案 A：继续补单个面板

做法：

```text
哪个面板不能用就补哪个面板。
哪个按钮不能点就补哪个按钮。
哪个报告缺字段就补哪个报告。
```

优点：

```text
短期最快。
每次改动小。
```

缺点：

```text
继续产生散装编辑器。
用户和 AI 都不知道项目整体状态。
Play / Build / Report 仍然和创作流程割裂。
后期很容易形成重复 command / 重复 state / 重复 diagnostics。
```

结论：

```text
不采用。
它违背当前“大系统优先”的讨论规则。
```

### 5.2 方案 B：在 M1 summary 上追加流程提示

做法：

```text
保留现有 ProjectAuthoringWorkspaceModel。
在 UI 上追加 next action / missing items 文本。
不建立正式 AuthoringWorkflowState。
```

优点：

```text
比方案 A 更收敛。
实现成本较低。
```

缺点：

```text
容易变成 summary 上贴一层提示。
没有稳定 workflow truth。
AI 仍然很难区分“域状态”和“项目创作进度”。
后续复杂项目会继续在各域里追加局部规则。
```

结论：

```text
不作为长期路线。
可以作为过渡思路，但本系统不采用。
```

### 5.3 方案 C：Unity/UE/Godot 式 Workflow Spine

做法：

```text
在 M1 的 workspace domain 之上新增正式 AuthoringWorkflowState。
每个 domain 不只汇报自身状态，还汇报它在完整项目创作链里的完成度、阻塞、下一步命令。
UI 展示统一创作主线。
AI 读取同一份 workflow context。
Play / Build / Report 成为流程节点，不再是孤立面板。
```

优点：

```text
最接近 Unity / UE / Godot 的成熟编辑器组织方式。
适合复杂项目长期维护。
适合 AI 读取和生成项目补丁。
不会把打飞机玩法写死进引擎。
能把已有系统重新组织成产品化创作闭环。
```

缺点：

```text
比方案 A/B 需要更清晰的数据结构。
如果边界不严，会膨胀成新的大而全流程框架。
```

结论：

```text
采用方案 C-min。
只做 workflow spine，不重做各 domain 编辑器。
```

## 6. C-min 正式目标

第一版目标：

```text
把复杂打飞机项目从“很多能力已经存在”推进到“编辑器能说明完整创作路径”。
```

必须形成：

```text
AuthoringWorkflowState
AuthoringWorkflowStep
AuthoringDomainProgress
AuthoringTask
AuthoringIssue
AuthoringCommand
AuthoringAiContext
```

必须覆盖的流程节点：

```text
Project
Assets
Scene
Prefab
Rules
Input
AUI
Play
Build
Reports
```

第一版不要求每个域都是完整商业级编辑器，但每个域必须能回答：

```text
这个域是否可进入？
这个域是否已有内容？
这个域是否可保存？
这个域是否阻塞 Play？
这个域是否阻塞 Build？
这个域下一步建议是什么？
这个域相关错误在哪里看？
```

## 7. 核心数据结构

### 7.1 AuthoringWorkflowState

```text
AuthoringWorkflowState
  schema_version
  project_id
  active_step
  steps
  global_status
  can_play
  can_build
  blocking_issues
  recommended_next_tasks
  ai_context_summary
```

职责：

```text
表示完整项目创作流程当前状态。
它是编辑器工作流状态的唯一真相层。
它不替代 ProjectDocument / SceneDocument / AssetDB / RuntimePackage。
它只聚合和解释这些域的状态。
```

### 7.2 AuthoringWorkflowStep

```text
AuthoringWorkflowStep
  id
  domain
  title
  status
  completion
  is_required_for_play
  is_required_for_build
  primary_command
  secondary_commands
  issues
  next_hint
```

建议的 step：

```text
project_open
asset_import
scene_setup
prefab_setup
rule_setup
input_setup
aui_setup
play_preview
build_windows
report_review
```

### 7.3 AuthoringStepStatus

```text
NotAvailable
Empty
NeedsAttention
Ready
Dirty
Running
Blocked
Failed
Complete
```

规则：

```text
NotAvailable: 前置项目未打开或域不可用。
Empty: 域可用但没有内容。
NeedsAttention: 内容存在但缺少推荐配置。
Ready: 可进入下一步。
Dirty: 有未保存修改。
Running: Play / Build / Import 等正在执行。
Blocked: 阻塞 Play 或 Build。
Failed: 最近一次执行失败。
Complete: 当前流程节点已满足 C-min 项目要求。
```

### 7.4 AuthoringTask

```text
AuthoringTask
  id
  domain
  priority
  title
  reason
  command
  is_ai_actionable
  is_user_actionable
```

用途：

```text
给用户显示“下一步做什么”。
给 AI 显示“可以自动补什么”。
```

### 7.5 AuthoringIssue

```text
AuthoringIssue
  id
  domain
  severity
  message
  source_ref
  blocks_play
  blocks_build
  suggested_command
```

规则：

```text
Issue 只描述创作链上的阻塞，不替代底层 diagnostics。
底层 report 仍归 Asset / Rule / Build / Runtime / Render 各自拥有。
Workflow 只聚合关键阻塞。
```

### 7.6 AuthoringCommand

```text
AuthoringCommand
  command_id
  domain
  label
  availability
  payload_kind
```

规则：

```text
AuthoringCommand 不绕过 AI-first Editor Command Framework。
它只引用已有 UiCommand / WorkspaceCommand / EditorCommandCatalog 中的正式 command。
不能在 workflow 里私自执行业务逻辑。
```

## 8. 工作流规则

### 8.1 Project 是入口

```text
没有打开项目时，只允许 Project Home / Recent / Create / Open。
其它步骤必须 NotAvailable。
```

### 8.2 Assets 是内容基础

```text
Assets 没有任何可用资源时，Scene / Prefab / AUI 可以编辑，但 workflow 必须提示资源不足。
资源缺失不一定阻塞 Play，但会阻塞“复杂打飞机完整项目”完成度。
```

### 8.3 Scene 是项目装配中心

```text
Scene 必须能看到当前实体数量、选中对象、缺失组件、未保存状态。
Scene 缺失主场景时阻塞 Play / Build。
```

### 8.4 Prefab 是复用对象入口

```text
Prefab 不写 gameplay 专用概念。
Prefab 只表达可复用 Entity / Component template。
复杂打飞机里的 PlayerPlane / EnemyPlane / Bullet 都是项目 prefab，不是引擎 prefab 类型。
```

### 8.5 Rules 是项目逻辑入口

```text
Workflow 只显示 Rule 是否存在、是否编译、是否注册、是否有阻塞错误。
具体规则仍属于 M2 Project Rule IR / Rust AOT / Runtime Execute。
```

### 8.6 Input 是操作映射入口

```text
第一版只要求 Windows keyboard / mouse 输入映射进入项目。
缺少 Input 不一定阻塞 Build，但会阻塞“可玩”完成度。
```

### 8.7 AUI 是游戏 UI 入口

```text
AUI 负责 HUD / menu / overlay 的项目侧 UI 文档。
Workflow 只显示 AUI 文档数量、绑定状态、缺失资源和阻塞问题。
```

### 8.8 Play 是编辑器预览入口

```text
Play 必须基于当前项目状态判断是否可运行。
Play 不应该绕过保存 / 构建 / RuntimePackage 的正式路径。
C-min 可以允许局部预览，但必须在 workflow 里标明 preview mode。
```

### 8.9 Build 是 Windows 导出入口

```text
Build 必须聚合 Project / Scene / Asset / Rule / Input / AUI 的阻塞状态。
Build 不直接成为底层 DesktopExportPipeline 的替代。
Build 只负责将用户命令路由到正式导出系统，并读取报告。
```

### 8.10 Reports 是修复入口

```text
Reports 不只是日志面板。
它必须能告诉用户：哪个流程节点失败、失败来源是什么、下一步该打开哪个域修复。
```

## 9. UI 呈现规则

第一版 UI 应该像创作流，而不是功能菜单堆叠。

建议结构：

```text
Top Toolbar:
  Project / Save / Play / Build / Reports

Left Workflow Rail:
  Project
  Assets
  Scene
  Prefabs
  Rules
  Input
  AUI
  Play
  Build
  Reports

Center Workspace:
  当前 domain 的主要编辑界面

Right Inspector:
  当前 selection / 当前 workflow step / 当前 issue

Bottom Console / Report:
  当前流程相关日志和错误
```

第一版不要求 UI 完全像 Unity，但必须形成同样的用户心智：

```text
我知道当前在哪一步。
我知道下一步是什么。
我知道为什么不能 Play / Build。
我知道错误该去哪修。
```

## 10. AI 友好规则

AI 不应该解析屏幕文字猜状态，而应该读取结构化 workflow context：

```text
AuthoringAiContext
  project_summary
  active_step
  domain_progress
  missing_required_items
  blocking_issues
  recommended_tasks
  available_commands
```

AI 可以做：

```text
建议下一步
生成资源导入计划
生成 prefab / scene / rule / input / AUI patch
解释 Build 失败原因
根据 workflow issue 定位修复域
```

AI 不可以做：

```text
绕过正式 command 直接改 runtime 内存
把项目玩法写进引擎 API
用临时 fixture 伪装项目完成
把测试通过当作用户创作完成
```

## 11. 和现有系统的关系

### 11.1 editor_ui_model

新增或扩展：

```text
AuthoringWorkflowState
AuthoringWorkflowStep
AuthoringStepStatus
AuthoringTask
AuthoringIssue
AuthoringCommand
AuthoringAiContext
```

约束：

```text
editor_ui_model 只放 UI 可读模型，不读写磁盘，不执行业务。
```

### 11.2 editor_core

新增 workflow composer / service：

```text
AuthoringWorkflowComposer
AuthoringWorkflowService
```

职责：

```text
从 ProjectSession / WorkspaceDomainSummary / reports / build state / play state 聚合 workflow state。
不直接画 UI。
不直接执行底层 build / play / import。
```

### 11.3 editor_ui_renderer

新增 workflow panel / rail：

```text
WorkflowRail
WorkflowStepPanel
WorkflowIssuePanel
WorkflowNextTaskPanel
```

职责：

```text
只根据 AuthoringWorkflowState 生成 DrawCommand / HitRegion。
不推导业务状态。
```

### 11.4 editor_input / command framework

规则：

```text
点击 workflow step -> UiCommandPayload::SetActiveAuthoringStep 或等价 command。
点击 task -> 已注册 command。
不可用 command 必须显示 disabled reason。
```

### 11.5 reports

规则：

```text
Workflow issue 引用底层 report source_ref。
不复制大段底层报告。
不新增第二套 diagnostics 真相。
```

## 12. 不允许做的事

```text
不允许把 Complex Shooter 专用概念写进引擎 API。
不允许为了一个样例项目新增特殊 command。
不允许 workflow 直接操作磁盘文件，必须走对应 domain service。
不允许 workflow 绕过 command framework 执行业务。
不允许把 testing gate 当作 authoring workflow 的主目标。
不允许重新讨论已经完成的 M1/M7/M8/M9/M10/M11 底层方案，除非实现证明方案错误。
不允许让 EditorSession 重新膨胀成上千行大类。
```

## 13. C-min 验收标准

第一版完成后，必须能证明：

```text
打开项目后生成完整 AuthoringWorkflowState。
没有项目时只有 Project step 可用。
有项目但没有主场景时 Scene step 标记阻塞 Play / Build。
导入资源后 Assets step 完成度变化。
创建或加载 scene 后 Scene step 完成度变化。
存在 prefab / rule / input / AUI 文件时对应 step 能反映数量和状态。
Build 失败时 Build / Reports step 能引用失败 source_ref。
AI context 能读到 active_step / blocking_issues / recommended_tasks / available_commands。
UI 能展示 workflow rail 和至少一个 next task / issue。
```

## 14. 分阶段落地建议

### Stage A: 数据模型

```text
在 editor_ui_model 中新增 AuthoringWorkflowState 系列模型。
补最小模型测试。
```

### Stage B: 状态聚合

```text
在 editor_core 中新增 AuthoringWorkflowComposer。
从已有 ProjectAuthoringWorkspaceModel / ProjectSession / Build / Play / Report 聚合状态。
补 composer 测试。
```

### Stage C: Command 接入

```text
新增切换 active workflow step 的 command。
把 task command 映射到已有 command catalog。
补 command availability 测试。
```

### Stage D: UI 呈现

```text
在 editor_ui_renderer 中新增 Workflow Rail / Next Task / Issue 摘要。
补 DrawCommand / HitRegion 测试。
```

### Stage E: AI Context 接入

```text
把 workflow state 摘要接入 WorkspaceContextForAI。
补 AI context 测试。
```

### Stage F: 文档同步

```text
更新 49 / 54 / 00-文档地图。
生成阶段完成记录。
```

## 15. 和其它引擎对比结论

| 项目 | Unity | UE | Godot | Bevy | 我们 |
|---|---|---|---|---|---|
| 编辑器中心 | EditorWindow / Layout | MainFrame / LevelEditor | EditorNode | 无官方完整编辑器 | EditorSession + WorkflowState |
| 项目资源入口 | ProjectBrowser / AssetDatabase | ContentBrowser | FileSystemDock | AssetServer | Asset Workspace / Asset DB |
| 场景入口 | SceneView / Hierarchy | Viewport / Outliner | SceneTreeDock | ECS World | Scene Workspace |
| 属性入口 | InspectorWindow | DetailsView | Inspector | Reflect 可用但无官方编辑器 | Schema-driven Inspector |
| 运行入口 | PlayModeView | PIE / PlayWorldCommands | EditorRunBar | App Runner | Play Workspace |
| 导出入口 | BuildPlayerWindow | Cook / Package / BuildCookRun | ProjectExportDialog | 自行组织 | Build Workspace |
| 工作流主干 | 隐式但成熟 | 显式且重型 | 中心化简单 | 缺失 | 显式且 AI 友好 |
| AI 友好 | 弱 | 弱 | 弱 | 中等底层 | 强 |

结论：

```text
我们的方案最接近 UE 的 workflow spine 思路和 Godot 的中心节点思路，
同时保持 Unity 的简单创作心智。
Bevy 只作为底层 ECS / Reflect / Asset 参考，不作为编辑器 UX 目标。
```

## 16. 严格方案自审

### 16.1 是否合乎用户规格

结论：通过。

理由：

```text
用户明确要求暂停测试系统，回到复杂打飞机真实编辑器流程。
本方案不是测试系统。
用户要求大系统讨论，避免一个按钮一个按钮修。
本方案把 Project / Asset / Scene / Prefab / Rule / Input / AUI / Play / Build / Report 收敛成一个大系统。
用户要求不要为了特定项目增加引擎规则。
本方案明确禁止 Player / Enemy / Bullet / Health / Score 等进入引擎 API。
```

### 16.2 是否合乎已有规则

结论：通过。

理由：

```text
遵守 130 的复杂打飞机缺失能力基线。
不重新推翻 M1，而是在 M1 之上做 productization。
遵守 AI-first command framework，不绕过 command catalog。
遵守文档治理规则，新系统先方案、自审、施工文档、自审，再施工。
```

### 16.3 是否合乎长期主义

结论：通过。

理由：

```text
选择 workflow spine，而不是继续补散装面板。
长期可以扩展到任意项目，不绑定打飞机玩法。
AuthoringWorkflowState 是创作状态真相，不是临时 UI 文案。
后续复杂项目只新增项目侧 schema/rule/prefab，不要求引擎 workflow 新增玩法概念。
```

风险：

```text
如果 AuthoringWorkflowState 承担太多业务，会变成新的大泥球。
必须坚持 workflow 只聚合状态和命令，不直接做 domain 业务。
```

### 16.4 是否合乎方案文字本身

结论：通过。

理由：

```text
方案明确了系统是什么、边界是什么、负责什么、不负责什么。
方案给出了数据结构、工作流规则、UI 呈现规则、AI 规则、现有系统关系和验收标准。
方案没有用测试 gate 替代用户创作流程。
```

### 16.5 是否方便实现

结论：通过，但必须分阶段施工。

理由：

```text
已有 WorkspaceDomainSummary / ProjectAuthoringWorkspaceModel，可作为输入。
已有 editor_ui_model / editor_core / editor_ui_renderer 分层，可分别落地模型、聚合和渲染。
已有 command framework，可避免新增散装点击逻辑。
已有 tests，可按模块补最小测试。
```

### 16.6 是否合理、是否能实现

结论：通过。

理由：

```text
方案没有要求一次实现完整 Unity/UE 级编辑器。
C-min 只要求 workflow spine 和状态聚合。
每个 domain 仍复用现有实现。
Play / Build / Report 只做流程纳入，不重写底层 pipeline。
```

### 16.7 和源码参考是否一致

结论：通过。

理由：

```text
Unity 证明 Project / Hierarchy / Inspector / Play / Build 是稳定创作心智。
UE 证明 workflow-oriented app / command list / details view 是复杂编辑器的成熟路线。
Godot 证明中心 EditorNode + docks + run/export 可以更简单但仍统一。
Bevy 证明底层 ECS/Reflect/Asset 不能替代编辑器产品工作流。
```

### 16.8 最终自审结论

本方案通过自审，可以进入施工文档生成阶段。

施工文档必须继续保持：

```text
一个阶段一个模块。
每个模块完成后跑最小测试。
不能先施工 UI 再补模型。
不能让 workflow 执行业务逻辑。
不能把复杂打飞机玩法写进引擎 API。
```
