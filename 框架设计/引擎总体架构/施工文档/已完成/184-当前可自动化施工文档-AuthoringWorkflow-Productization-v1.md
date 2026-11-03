# 184-当前可自动化施工文档-AuthoringWorkflow Productization v1

## 1. 施工依据

```text
框架设计/引擎总体架构/183-AuthoringWorkflow-Productization-v1方案.md
```

本阶段只产品化已有 `AuthoringWorkflowModel`，不新增 `AuthoringOperation / AuthoringWalkthrough / DomainOperationCatalog`。

## 2. 施工目标

实现 `AuthoringWorkflow Productization v1 / C-min`：

```text
AuthoringWorkflowModel
  -> AuthoringCommand / recommended_tasks
  -> WorkflowCommandResolver
  -> UiCommandPayload / ProjectPatch
  -> EditorSession
  -> Workflow refresh
```

第一版优先覆盖：

```text
Project
Scene
Input
Play
Build
Report
```

`Prefab / Rule / AUI` 第一版不强行伪造复杂操作，只允许显示状态、Disabled、FocusDomainPanel 或 NotImplemented。

## 3. 非目标

本阶段不做：

```text
新的 Walkthrough / Operation 层
新的通用 Wizard / 参数收集器
新的项目侧玩法 API
完整 Prefab / Rule / AUI 产品化
绕过 EditorSession 的直接状态修改
```

禁止新增：

```text
Player / Enemy / Bullet / Health / Damage / Score / Wave / Weapon / Boss / Drop
```

## 4. 阶段 A：WorkflowCommandResolver

### 4.1 目标

新增轻量解析器：

```text
AuthoringCommand.command_id / payload_kind
  -> WorkflowCommandResolver
  -> WorkflowCommandResolution
```

解析结果：

```text
Command(UiCommandPayload)
FocusDomainPanel { domain }
Disabled { reason }
Unsupported { reason }
```

### 4.2 规则

只允许解析：

```text
无参命令
上下文足够明确的安全默认命令
```

需要用户补参数的命令不能伪造参数，必须返回 `FocusDomainPanel` 或 `Disabled`。

### 4.3 最小测试

```text
resolver maps Play / Export / ClearConsole to UiCommandPayload
resolver disables unavailable command
resolver focuses Scene for CreateSceneEntity when parameter context is missing
resolver does not fabricate gameplay-specific payload
```

## 5. 阶段 B：核心域命令覆盖

### 5.1 目标

补齐核心域第一版命令覆盖：

```text
Project: open_project / refresh_recent_projects
Scene: open_scene_document / create_scene_entity as FocusDomainPanel when missing args
Input: create_default_input_mapping
Play: play
Build: export_desktop_package
Report: open_build_report / clear_console
```

### 5.2 规则

`AuthoringWorkflowStep.primary_command` 必须与当前 domain 状态匹配：

```text
Available
Disabled
```

不可执行的命令不能出现在 `available_commands` 中。

### 5.3 最小测试

```text
core domains expose primary commands
disabled commands are excluded from available_commands
recommended_tasks reference existing commands
```

## 6. 阶段 C：UI 路由接入

### 6.1 目标

在已有 `HitRegion / HitTarget / editor_input` 基础上，补齐 workflow command 路由。

第一版保留现有 step hit：

```text
AuthoringWorkflowStep -> SetAuthoringWorkflowStep
```

并新增可点击 command/task 的 hit target：

```text
AuthoringWorkflowCommand { command_id, payload_kind }
```

`editor_input` 根据 hit target 走 `WorkflowCommandResolver`。

### 6.2 规则

UI 层只产生 hit target，不直接构造业务 payload。

解析和执行必须进入：

```text
WorkflowCommandResolver
UiCommandPayload
EditorSession
```

### 6.3 最小测试

```text
workflow step hit still maps to SetAuthoringWorkflowStep
workflow command hit maps through resolver
disabled command hit returns no executable command
```

## 7. 阶段 D：状态刷新和 AI 一致性

### 7.1 目标

命令执行后验证：

```text
ProjectAuthoringWorkspaceModel
AuthoringWorkflowModel
AuthoringAiContext
```

三者状态一致。

### 7.2 最小测试

```text
execute workflow command -> build_ui_model refreshes active_step / recommended_tasks
AI context sees the same active_step / missing_required_items / available_commands
```

## 8. 阶段 E：整体回归

运行：

```text
cargo test -p editor_ui_model authoring_workflow
cargo test -p editor_core authoring_workflow
cargo test -p editor_input authoring_workflow
cargo test -p editor_ui_renderer authoring_workflow
cargo test -p editor_core
```

如果某个精确过滤器没有匹配测试，应改跑对应 crate 的完整测试。

## 9. 文档自审

### 9.1 是否符合 183

符合。施工只产品化已有 `AuthoringWorkflowModel`，不新增 Operation 真相层。

### 9.2 是否范围可控

符合。只覆盖核心域命令解析和 UI 路由，不补完整 Prefab / Rule / AUI 编辑器。

### 9.3 是否方便实现

符合。当前代码已有 `AuthoringWorkflowModel`、`AuthoringCommand`、`HitRegion / HitTarget`、`editor_input` 和 `EditorSession`。

### 9.4 是否可测试

符合。每阶段都有 headless 或 UI routing 最小测试。

## 10. 完成标准

```text
WorkflowCommandResolver 落地。
核心域命令覆盖测试通过。
Workflow command/task hit region 可路由。
Disabled / FocusDomainPanel 不伪造 payload。
AI context 与 workflow 状态一致。
相关 crate 测试通过。
```
