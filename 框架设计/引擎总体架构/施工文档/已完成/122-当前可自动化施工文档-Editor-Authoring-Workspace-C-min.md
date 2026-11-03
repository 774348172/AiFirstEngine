# 122-当前可自动化施工文档：Editor Authoring Workspace C-min

## 1. 施工目标

按照 `122-Editor-Authoring-Workspace-C-min方案.md`，在现有 Native Editor 架构上补齐统一编辑工作台归口层。

本次施工不重写已有系统：

```text
UiCommandPayload
EditorSession
SceneEditTransaction
SceneEditing
ProjectAsset-to-SceneEntity Authoring
AI Panel proposal / accept
NativeEditorApplication shell
```

本次施工要新增或收敛：

```text
EditorAuthoringWorkspace
WorkspaceState
WorkspaceSelection
WorkspaceContext
WorkspaceCommandRouter / command id 归口
WorkspaceDiagnostics 最小报告
NativeEditorApplication 持有 workspace
```

核心判断：

```text
Inspector TextInput、SceneView picking、ProjectDock interaction 不作为独立主线。
它们先作为 Workspace 的内部命令来源处理。
```

## 2. 涉及文件

预计涉及：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/当前/122-当前可自动化施工文档-Editor-Authoring-Workspace-C-min.md
框架设计/引擎总体架构/施工文档/已完成/122-当前可自动化施工文档-Editor-Authoring-Workspace-C-min.md
框架设计/引擎总体架构/阶段完成记录/2026-06-30-Editor-Authoring-Workspace-C-min/00-总览.md
```

## 3. 阶段任务

### 阶段 1：EditorAuthoringWorkspace 核心结构

在 `editor_core` 增加：

```text
EditorAuthoringWorkspace
WorkspaceState
WorkspaceSelection
WorkspaceContext
WorkspaceDiagnosticsSummary
WorkspaceCommandExecutionReport
```

第一版规则：

```text
Workspace 不持有 SceneDocument 真相。
Workspace 从 EditorUiModel / UiCommand / CommandResult 汇总状态。
Workspace 只做归口、上下文、报告和最小校验。
真正修改仍由 EditorSession -> SceneEditTransaction 执行。
```

验收：

```powershell
cargo test -p editor_core editor_authoring_workspace
```

### 阶段 2：WorkspaceCommandRouter 归口已有 UiCommand

增加：

```text
EditorAuthoringWorkspace::normalize_command
EditorAuthoringWorkspace::record_command_result
EditorAuthoringWorkspace::context_from_model
```

第一版不新增新的 command enum，直接复用 `UiCommandPayload`，避免造第二套命令体系。

验收：

```powershell
cargo test -p editor_core editor_authoring_workspace
```

### 阶段 3：NativeEditorApplication 接入 Workspace

在 `editor_window_winit` 中：

```text
NativeEditorApplication 持有 EditorAuthoringWorkspace。
dispatch_command 先经过 workspace.normalize_command。
CommandResult 返回后 workspace.record_command_result。
NativeEditorApplicationReport 增加 workspace 字段摘要。
```

验收：

```powershell
cargo test -p editor_window_winit
```

### 阶段 4：最小闭环测试

增加或扩展测试：

```text
Hierarchy create/select 进入 Workspace。
Inspector set transform 进入 Workspace。
ProjectDock place asset 进入 Workspace。
AI proposal accept 进入 Workspace。
Undo/Redo 进入 Workspace。
```

验收：

```powershell
cargo test -p editor_core
cargo test -p editor_window_winit
```

### 阶段 5：总体验证

运行：

```powershell
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p editor_host
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

## 4. 不做范围

本次不做：

```text
真实 IME 文本编辑控件
复杂 SceneView picking
完整 Gizmo
完整 Project asset browser
真实 AI provider
多选批量编辑
Prefab Mode
项目玩法 API
```

这些后续只能作为 `EditorAuthoringWorkspace` 内部能力扩展，不再作为独立架构主线。

## 5. 完成后文档同步

完成后：

```text
施工文档/当前/122... -> 施工文档/已完成/122...
新增 阶段完成记录/2026-06-30-Editor-Authoring-Workspace-C-min/00-总览.md
更新 49-当前状态与下一步入口.md
更新 54-当前可自动化施工总入口.md
```

