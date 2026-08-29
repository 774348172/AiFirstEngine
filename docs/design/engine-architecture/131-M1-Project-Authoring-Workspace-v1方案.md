# 131-M1 Project Authoring Workspace v1 方案

## 1. 定位

本文定义 `130-复杂打飞机编辑到 Windows 可玩项目缺失能力当前基线` 中的 `M1 Project Authoring Workspace v1`。

M1 的目标不是再补一个面板，也不是重新推翻已有编辑器架构，而是把已经完成的 C-min 能力收敛成统一项目创作工作区：

```text
Project
  -> Scene
  -> Asset
  -> Prefab
  -> Rule
  -> AUI
  -> Input
  -> Play
  -> Build
  -> Report
```

M1 只解决“编辑器里如何统一创作和组织项目”。

它不直接实现完整项目规则运行、真实 GPU 资源绑定、真实 Windowed Player 或完整 Build And Run；这些分别属于：

```text
M2 Project Rule Authoring / Compile / Runtime Execute
M3 RuntimePackage -> Native Windowed Player exe
M4 Runtime Asset Cook -> GPU Resource Binding
M5 Sprite2D 产品级运行链路
M6 Desktop Build And Run 体验
```

但 M1 必须为这些后续系统预留正式入口，避免之后继续散成一堆局部功能。

## 2. 已有基础

M1 不从零开始。它承接这些已确认 / 已完成系统：

```text
122-Editor Authoring Workspace C-min
126-Unity-like Editor Authoring Workspace v1
129-Editor Build / Export Workspace v1
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线
```

当前已经具备：

```text
Project Launcher / Open / Create
ProjectBrowser C-min
Hierarchy / Inspector / Scene Editing C-min
Property Editing 基础
AI Panel 基础
PlaySession headless gate
Build Export panel
Console / RuntimeTrace 基础
```

当前不足：

```text
各能力还是面板级拼接，没有统一 Project Authoring Workspace State。
Scene / Asset / Prefab / Rule / AUI / Input / Play / Build 没有统一 Domain 边界。
AI 看到的是局部 context，不是完整项目创作上下文。
Report 没有把各域诊断统一成项目级视图。
```

## 3. 与成熟引擎对比

| 引擎 | 对应系统 | 成熟做法 | 对我们的启发 |
|---|---|---|---|
| Unity | Project Browser / Hierarchy / Inspector / SceneView / GameView / Build Settings | 用户在一个 Editor Workspace 中管理资源、场景、组件、运行和构建；底层通过 AssetDatabase、SerializedObject、Undo 统一数据修改 | 学习 Unity 的简单用户心智：Project -> Scene -> Inspector -> Play/Build |
| Unreal Engine | LevelEditor / ContentBrowser / SceneOutliner / Details / Toolbar / Project Launcher | 模块很多，但通过 LevelEditor、CommandList、Transaction、DetailsView 组织成统一工作区 | 学习 UE 的命令、事务和模块边界 |
| Godot | EditorNode / FileSystemDock / SceneTreeDock / Inspector / Output / Run | EditorNode 统一组织 FileSystem、Scene、Inspector、Run，结构比 UE 简洁 | 学习 Godot 的短路径和统一中心节点 |
| Bevy | ECS / Assets / Schedule / Examples | Runtime 侧强，但官方编辑器链路弱 | 我们不能只停留在 ECS 示例，需要补编辑器产品链路 |

结论：

```text
成熟引擎都有多个面板，但不会让每个面板成为独立真相。
它们都有统一工作区、统一命令入口、统一事务或 undo、统一错误反馈。
```

## 4. 设计原则

### 4.1 引擎侧 / 项目侧边界

引擎侧只提供通用创作能力：

```text
ProjectDocument
SceneDocument
AssetRecord
PrefabDocument
RuleDocument
AuiDocument
InputMappingDocument
BuildProfile
Report
Command
Transaction
Selection
DirtyState
Validation
```

项目侧定义具体玩法：

```text
PlayerPlane
EnemyPlane
Bullet
Health
Damage
Score
Wave
Weapon
Boss
```

规则：

```text
M1 不允许新增打飞机专用 API。
如果一个字段或命令只对打飞机有意义，它必须进入项目侧 Schema / Rule / Prefab / AUI。
```

### 4.2 单一编辑入口

所有项目编辑都必须经过：

```text
WorkspaceCommand
  -> WorkspaceCommandRouter
  -> Domain Handler
  -> Transaction
  -> Document Write
  -> Dirty / Undo / Diagnostics
  -> WorkspaceState Rebuild
```

禁止：

```text
Panel 直接写文件。
Panel 直接写 Runtime World。
AI Panel 直接改 SceneDocument。
ProjectBrowser 绕过 Transaction 创建资源。
Inspector 绕过 WorkspaceCommand 写组件。
Build 面板私自改项目状态。
```

### 4.3 多 Domain，但统一上下文

M1 允许多个 Domain，但不允许多个真相。

```text
ProjectAuthoringWorkspace
  -> ProjectDomain
  -> SceneDomain
  -> AssetDomain
  -> PrefabDomain
  -> RuleDomain
  -> AuiDomain
  -> InputDomain
  -> PlayDomain
  -> BuildDomain
  -> ReportDomain
```

Domain 只负责本领域状态、命令验证和诊断摘要。

全局统一：

```text
WorkspaceSelection
WorkspaceCommand
WorkspaceTransaction
WorkspaceReport
WorkspaceContextForAI
```

## 5. 系统结构

```text
ProjectAuthoringWorkspace
  state: ProjectAuthoringWorkspaceState
  domains: WorkspaceDomainRegistry
  selection: WorkspaceSelection
  command_router: WorkspaceCommandRouter
  transaction_log: WorkspaceTransactionLog
  report: WorkspaceReport
  ai_context: WorkspaceContextForAI
```

### 5.1 WorkspaceState

第一版状态只存摘要，不复制所有文档内容：

```text
ProjectAuthoringWorkspaceState
  project_root
  project_id
  active_scene_id
  active_document_kind
  active_document_path
  dirty_domains
  selected_target
  domains
  diagnostics
```

### 5.2 WorkspaceSelection

统一选择对象：

```text
WorkspaceSelection
  primary:
    Entity(entity_id)
    Asset(asset_ref)
    Prefab(prefab_id)
    Rule(rule_id)
    AuiDocument(aui_id)
    InputAction(action_id)
    BuildProfile(profile_id)
    ReportEntry(entry_id)
  secondary: Vec<WorkspaceSelectionTarget>
```

规则：

```text
Hierarchy 选 Entity。
Project / Asset Browser 选 Asset / Scene / Prefab / Rule / AUI / Input。
Report 面板选 ReportEntry。
Inspector 根据 selection 自动切换显示内容。
AI 读取 selection，不猜当前面板。
```

### 5.3 WorkspaceCommand

第一版命令分两层：

```text
WorkspaceCommand
  command_id
  source
  target_domain
  payload
  request_id
```

`payload` 按领域 typed：

```text
SceneCommand
AssetCommand
PrefabCommand
RuleCommand
AuiCommand
InputCommand
PlayCommand
BuildCommand
ReportCommand
```

外层统一，内层 typed。这样运行时好分发，AI 和验证层也能读懂。

### 5.4 WorkspaceTransaction

每个会改变项目状态的命令都生成 Transaction：

```text
WorkspaceTransaction
  transaction_id
  command_id
  target_domain
  read_set
  write_set
  before_summary
  after_summary
  diagnostics
  undo_policy
  status
```

第一版可以只做摘要级 undo，不要求所有 Domain 都完整 undo。

但必须保留统一事务边界。

### 5.5 WorkspaceReport

Report 是项目级聚合视图：

```text
WorkspaceReport
  project_status
  dirty_domains
  diagnostics_by_domain
  last_command
  last_transaction
  build_summary
  play_summary
  asset_summary
  rule_summary
  validation_summary
```

Report 不替代 Console。Console 是日志流，Report 是当前状态摘要。

## 6. Domain 第一版边界

### 6.1 ProjectDomain

职责：

```text
项目根目录
project.aife.json
默认 scene
项目保存状态
项目级 validation
```

第一版命令：

```text
OpenProject
CreateProject
SaveProject
ValidateProject
```

### 6.2 SceneDomain

职责：

```text
SceneDocument
Hierarchy
Entity selection
Transform / Component field editing
Scene save / dirty
```

复用：

```text
SceneEditing
PropertyEditing
PreviewWorldSync
```

第一版命令：

```text
OpenScene
CreateEntity
DeleteEntity
RenameEntity
SetEntityTransform
SetComponentField
SaveScene
```

### 6.3 AssetDomain

职责：

```text
Asset records
AssetRef
Importer status
Cook status summary
Asset selection
```

第一版命令：

```text
RefreshAssets
SelectAsset
ImportAsset
PlaceAssetIntoScene
```

### 6.4 PrefabDomain

职责：

```text
Prefab document list
Prefab instance summary
Prefab reference validation
```

第一版命令：

```text
CreatePrefabFromSelection
InstantiatePrefab
ValidatePrefab
```

第一版可以只做最小 prefab document / reference，不做完整 prefab override。

### 6.5 RuleDomain

职责：

```text
Rule manifest
Rule source summary
Compile / register status
Runtime rule status
```

第一版命令：

```text
RefreshRules
ValidateRules
CompileRules
```

M1 不实现完整规则执行，只提供 Workspace 入口和状态。规则运行属于 M2。

### 6.6 AuiDomain

职责：

```text
AUI document list
HUD document summary
binding summary
preview status
```

第一版命令：

```text
RefreshAuiDocuments
SelectAuiDocument
ValidateAuiBindings
```

M1 不实现完整 AUI 编辑器，只提供入口和状态。

### 6.7 InputDomain

职责：

```text
InputMappingDocument
Action list
Binding summary
validation diagnostics
```

第一版命令：

```text
RefreshInputMappings
SelectInputAction
ValidateInputMappings
```

### 6.8 PlayDomain

职责：

```text
Play mode state
RuntimePackage status
Runtime trace summary
```

第一版命令：

```text
Play
Pause
Stop
StepFrame
ReloadRuntimePackage
```

复用现有 PlaySession / RuntimeTrace。

### 6.9 BuildDomain

职责：

```text
Build profile
Export status
DesktopExportReport summary
Output path
```

第一版命令：

```text
ExportDesktopPackage
OpenBuildOutput
OpenBuildReport
```

复用 `129-Editor Build / Export Workspace v1`。

### 6.10 ReportDomain

职责：

```text
聚合 Project / Scene / Asset / Prefab / Rule / AUI / Input / Play / Build diagnostics
提供 AI 可读错误入口
提供用户可读当前阻塞原因
```

第一版命令：

```text
SelectReportEntry
ClearReportFilter
```

## 7. UI 组织规则

M1 的 UI 不是先追求完整美术，而是追求稳定信息架构。

第一版布局：

```text
Toolbar:
  Save / Undo / Redo / Play / Stop / Build / Validate

Left:
  Project / Scene tree

Center:
  SceneView / GameView tabs

Right:
  Inspector / Workspace Context / AI Panel

Bottom:
  Asset Browser / Console / Report / RuntimeTrace / Build
```

规则：

```text
一个 Panel 可以显示多个 Domain 的摘要。
一个 Domain 可以被多个 Panel 展示。
但修改只能走 WorkspaceCommand。
```

第一版不做：

```text
复杂 Dock 拖拽
完整右键菜单系统
完整 Scene Gizmo
Prefab Override UI
完整 Rule Graph Editor
完整 AUI 视觉编辑器
完整 Input Binding UI
复杂搜索过滤
```

这些属于后续 Domain 内部产品化，不影响 M1 架构。

## 8. AI 友好规则

AI 不直接读 UI 像素，也不猜用户当前看哪个面板。

AI 读取：

```text
WorkspaceContextForAI
  project_summary
  active_document_summary
  selection_summary
  domain_summaries
  allowed_commands
  dirty_summary
  diagnostics_summary
  last_transactions
  build_summary
  play_summary
```

AI 只能输出：

```text
WorkspaceCommand
ProjectPatch
ValidationRequest
ExplainDiagnosticsRequest
```

第一版 AI 可以只生成简单 WorkspaceCommand；完整 ProjectPatch 属于 M16。

但 M1 必须保证 AI 有统一上下文，不再从多个面板碎片里拼状态。

## 9. 测试与验收

M1 第一版必须有一个端到端 workspace gate：

```text
Create Project
  -> Open Workspace
  -> Open Scene
  -> Select Entity
  -> Edit Transform
  -> Select Asset
  -> Place Asset Into Scene
  -> Save Scene
  -> Validate Workspace
  -> Play gate
  -> Export gate
  -> WorkspaceReport contains domain summaries
  -> WorkspaceContextForAI contains allowed commands and diagnostics
```

每个 Domain 至少一个最小测试：

```text
ProjectDomain: open/create/save/validate summary
SceneDomain: selection + field edit + dirty
AssetDomain: select/place asset summary
PrefabDomain: prefab list / instantiate command shape
RuleDomain: rule manifest status summary
AuiDomain: aui document status summary
InputDomain: input action status summary
PlayDomain: play status summary
BuildDomain: export status summary
ReportDomain: diagnostics aggregation
```

## 10. 与 130 的关系

M1 只解决 `130` 中的第一项缺失能力：

```text
M1 Project Authoring Workspace v1
```

它必须为以下系统提供入口，但不替它们施工：

```text
M2 Project Rule Authoring / Compile / Runtime Execute
M3 RuntimePackage -> Native Windowed Player exe
M4 Runtime Asset Cook -> GPU Resource Binding
M5 Sprite2D 产品级运行链路
M6 Desktop Build And Run 体验
```

## 11. 推荐施工 Gate

后续如果生成施工文档，建议按以下 Gate：

```text
Gate 1: ProjectAuthoringWorkspaceState / DomainSummary 数据结构
Gate 2: WorkspaceCommand / Transaction / Report 统一模型
Gate 3: Scene / Asset / Build 已有能力接入 Domain
Gate 4: Prefab / Rule / AUI / Input 最小 Domain summary
Gate 5: WorkspaceContextForAI
Gate 6: SelfUiRenderer Workspace 信息架构更新
Gate 7: End-to-end Project Authoring Workspace gate test
```

每个 Gate 必须测试后再进入下一个 Gate。

## 12. 结论

采用：

```text
Project Authoring Workspace v1
完整系统边界 + C-min 落地
```

不采用：

```text
继续散点补面板
一次做完整 Unity/UE 级编辑器
```

M1 的价值是把编辑器能力从“多个能用的小系统”收敛成“一个用户和 AI 都能理解的项目创作工作区”。

下一步应基于本文生成施工文档，而不是重新讨论单个面板。
