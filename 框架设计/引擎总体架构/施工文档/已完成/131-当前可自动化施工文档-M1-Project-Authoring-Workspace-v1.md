# 131-当前可自动化施工文档-M1 Project Authoring Workspace v1

## 1. 施工目标

根据：

```text
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
131-M1-Project-Authoring-Workspace-v1方案.md
```

实现 `M1 Project Authoring Workspace v1` 的 C-min 版本。

本次施工目标不是完整 Unity/UE 编辑器，而是建立统一项目创作工作区的底座：

```text
ProjectAuthoringWorkspaceState
WorkspaceDomainSummary
WorkspaceCommand
WorkspaceTransaction
WorkspaceReport
WorkspaceContextForAI
Scene / Asset / Prefab / Rule / AUI / Input / Play / Build / Report Domain summary
SelfUiRenderer 工作区信息架构
End-to-end Project Authoring Workspace gate test
```

## 2. 不做范围

本次不做：

```text
完整 Rule Graph Editor
完整 AUI 视觉编辑器
完整 Input Binding Editor
完整 Prefab Override
完整 Asset Browser 产品化
真实 Windowed Player exe
真实 GPU 资源绑定
Sprite2D 产品级渲染
完整 Build And Run
AI Project Patch 完整入口
```

这些属于后续 M2-M6 / M16。

## 3. 预计涉及文件

优先在已有 crate 内扩展，不新增大框架：

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_core/src/authoring_workspace.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_host/src/main.rs
```

如代码体量过大，可以在 `editor_core` 内新增：

```text
rust/crates/editor_core/src/project_authoring_workspace.rs
```

但必须保持与现有 `EditorAuthoringWorkspace` 兼容，不允许做第二套编辑器。

## 4. Gate 1：Workspace State / Domain Summary 数据结构

### 任务

在 `editor_ui_model` 或 `editor_core` 中建立 C-min 数据结构：

```text
ProjectAuthoringWorkspaceModel / State
WorkspaceDomainSummary
WorkspaceDomainKind
WorkspaceDirtySummary
WorkspaceSelectionTarget
WorkspaceDocumentSummary
```

需要覆盖 Domain：

```text
Project
Scene
Asset
Prefab
Rule
AUI
Input
Play
Build
Report
```

第一版只存 summary，不复制完整文档。

### 验收

```text
模型可 serde JSON 序列化。
默认空项目状态可表达。
打开项目后可表达每个 Domain 的 status / item_count / dirty / diagnostics_count。
```

### 测试

```powershell
cargo test -p editor_ui_model project_authoring_workspace
```

## 5. Gate 2：WorkspaceCommand / Transaction / Report 统一模型

### 任务

建立统一命令和事务摘要：

```text
WorkspaceCommand
WorkspaceCommandPayload
WorkspaceCommandSource
WorkspaceTransactionSummary
WorkspaceReport
WorkspaceDiagnosticSummary
```

命令外层统一，payload 内层 typed：

```text
Scene(...)
Asset(...)
Prefab(...)
Rule(...)
Aui(...)
Input(...)
Play(...)
Build(...)
Report(...)
```

第一版可以把已有 `UiCommandPayload` 映射为 `WorkspaceCommand` 摘要，不要求替换所有现有命令执行路径。

### 验收

```text
Scene / Asset / Build 现有命令能生成 workspace command summary。
Report 能记录 last_command / last_transaction。
AI 可读 command id 和 target domain。
```

### 测试

```powershell
cargo test -p editor_ui_model workspace_command
cargo test -p editor_core workspace_command
```

## 6. Gate 3：Scene / Asset / Build 已有能力接入 Domain

### 任务

把已有能力接入 Domain summary：

```text
SceneDomain:
  active_scene_id
  entity_count
  selected_entity
  dirty
  diagnostics

AssetDomain:
  project_root
  selected_asset_ref
  asset_count
  importer / cook summary placeholder

BuildDomain:
  selected_profile
  last_export_status
  package_dir
  report_path
```

复用现有：

```text
ProjectBrowserModel
HierarchyModel
InspectorModel
BuildExportModel
WorkspaceContext
```

### 验收

```text
打开 / 创建项目后，ProjectAuthoringWorkspace 能显示 Project / Scene / Asset / Build domain summary。
Scene 编辑、Asset 选择、Export 后对应 summary 变化。
```

### 测试

```powershell
cargo test -p editor_core project_authoring_workspace
```

## 7. Gate 4：Prefab / Rule / AUI / Input 最小 Domain Summary

### 任务

先只做最小状态入口，不实现完整编辑器：

```text
PrefabDomain:
  prefab_count
  selected_prefab
  validation_status

RuleDomain:
  rule_count
  compile_status
  runtime_register_status

AuiDomain:
  document_count
  selected_document
  binding_status

InputDomain:
  action_count
  selected_action
  validation_status
```

如果当前项目没有对应文件，summary 必须能表达：

```text
not_configured
empty
valid
invalid
```

### 验收

```text
空项目不会报错。
有最小 manifest 文件时能统计数量。
AI / Report 能看到这些 Domain 是否缺失。
```

### 测试

```powershell
cargo test -p editor_core workspace_domain_summary
```

## 8. Gate 5：WorkspaceContextForAI

### 任务

建立 AI 可读上下文：

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

要求：

```text
AI 不读取像素。
AI 不猜当前面板。
AI 只看结构化 workspace context。
```

### 验收

```text
打开项目、选择 Entity、选择 Asset、执行 Export 后，AI context 能描述当前状态。
allowed_commands 不包含项目玩法专用命令。
```

### 测试

```powershell
cargo test -p editor_core workspace_context_for_ai
```

## 9. Gate 6：SelfUiRenderer Workspace 信息架构更新

### 任务

在现有 Native Editor UI 中增加 Workspace 总览表达：

```text
Workspace Summary / Domains
Project / Scene / Asset / Prefab / Rule / AUI / Input / Play / Build / Report status
Report summary
AI context summary
```

第一版可以是紧凑文本 / 状态条 / 小面板，不追求完整视觉编辑器。

要求：

```text
不要新增项目玩法文案。
不要做复杂 Dock 拖拽。
不要为了显示状态让布局不可点击。
所有按钮仍走 HitRegion -> InputRouter -> Command。
```

### 验收

```text
DrawList 中能看到 Workspace / Domain / Report 关键信息。
HitRegion 不破坏已有 ProjectBrowser / BuildExport / Toolbar。
```

### 测试

```powershell
cargo test -p editor_ui_renderer project_authoring_workspace
cargo test -p editor_input project_authoring_workspace
cargo test -p editor_window_winit project_authoring_workspace
```

## 10. Gate 7：端到端 Workspace Gate Test

### 任务

建立一个端到端测试：

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

可以 headless 执行，不要求真实窗口。

### 验收

```text
测试能证明 M1 不是多个面板拼接，而是统一 Workspace 状态。
WorkspaceReport 能回答当前项目缺什么。
WorkspaceContextForAI 能给 AI 下一步可执行命令。
```

### 测试

```powershell
cargo test -p editor_core project_authoring_workspace_end_to_end
cargo test -p editor_host project_authoring_workspace
```

## 11. 回归测试

所有 Gate 完成后运行：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_ui_renderer
cargo test -p editor_input
cargo test -p editor_window_winit
cargo test -p editor_host
```

## 12. 完成后文档同步

完成后必须：

```text
1. 将本文档从 施工文档/当前/ 移到 施工文档/已完成/。
2. 新增 阶段完成记录/2026-07-01-M1-Project-Authoring-Workspace-v1/00-总览.md。
3. 更新 49-当前状态与下一步入口.md。
4. 更新 54-当前可自动化施工总入口.md。
5. 更新 施工文档/README.md。
```

## 13. 成功标准

本次成功不以“编辑器看起来像 Unity 完整版”为标准。

成功标准是：

```text
项目创作状态有统一 Workspace。
每个核心 Domain 都有 summary。
修改入口收敛到 WorkspaceCommand / Transaction。
错误和缺失能力进入 WorkspaceReport。
AI 能读取 WorkspaceContextForAI。
Scene / Asset / Build 现有能力被统一工作区接管。
```

这一步完成后，后续 M2-M6 才能按统一 Workspace 接口继续接入，而不是继续散着补系统。
