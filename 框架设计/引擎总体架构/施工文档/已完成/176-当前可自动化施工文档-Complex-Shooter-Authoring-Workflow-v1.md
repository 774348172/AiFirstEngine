# 176-当前可自动化施工文档-Complex Shooter Authoring Workflow v1

## 1. 施工目标

根据：

```text
175-Complex-Shooter-Authoring-Workflow-v1方案.md
```

实现 `Editor Authoring Workflow Spine C-min`。

本阶段不是新增测试系统，不是新增打飞机玩法 API，也不是重写 M1。目标是在现有 `ProjectAuthoringWorkspaceModel` 之上增加可被用户和 AI 读取的创作工作流主干：

```text
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
```

完成后，编辑器 UI model 应能回答：

```text
项目当前处于哪一步
每一步是否可用
每一步是否阻塞 Play / Build
下一步建议做什么
当前阻塞问题是什么
AI 可以读取哪些可执行任务
```

## 2. 不做范围

```text
不做新的 Runtime 逻辑执行器
不做新的渲染底层
不做新的 Physics2D
不做完整 Rule Graph Editor
不做 209 Scene Unified AUI Authoring
不做完整 Unity/UE Prefab override
不把 Player / Enemy / Bullet / Health / Score 写进引擎 API
不把 workflow 变成第二套业务执行入口
```

## 3. 涉及文件

预计新增：

```text
rust/crates/editor_ui_model/src/authoring_workflow.rs
rust/crates/editor_core/src/authoring_workflow.rs
rust/crates/editor_core/src/tests/authoring_workflow_tests.rs
rust/crates/editor_ui_renderer/src/panels/authoring_workflow.rs
框架设计/引擎总体架构/阶段完成记录/2026-07-03-Complex-Shooter-Authoring-Workflow-v1/00-总览.md
```

预计修改：

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_model/src/model.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_ui_model/src/tests.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/editor_command_executor.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/editor_ui_renderer/src/panels/mod.rs
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_ui_renderer/src/tests.rs
rust/crates/editor_input/src/lib.rs
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
```

实际施工时如发现无需修改某文件，可以跳过，但不能新增未记录的大模块。

## 4. 分阶段施工

### Stage A: editor_ui_model workflow 数据模型

新增：

```text
AuthoringWorkflowModel
AuthoringWorkflowStep
AuthoringStepId
AuthoringStepStatus
AuthoringTask
AuthoringIssue
AuthoringCommand
AuthoringAiContext
AuthoringIssueSeverity
AuthoringTaskPriority
```

接入：

```text
EditorUiModel.authoring_workflow
```

最低测试：

```powershell
cargo test -p editor_ui_model authoring_workflow
```

验收：

```text
没有项目时，只有 Project step 可用。
默认 step 顺序稳定。
模型可 serde roundtrip。
```

### Stage B: editor_core workflow 聚合

新增：

```text
AuthoringWorkflowComposer
```

输入：

```text
ProjectAuthoringWorkspaceModel
ProjectSession
Build state
Play state
Report summary
```

输出：

```text
AuthoringWorkflowModel
```

最低测试：

```powershell
cargo test -p editor_core authoring_workflow
```

验收：

```text
无项目 -> Project 可用，其它 NotAvailable。
有项目无场景 -> Scene 阻塞 Play / Build。
有资源 / prefab / rule / input / AUI 文件 -> 对应 step item_count / status 变化。
Build failed -> Build / Reports issue 可见。
```

### Stage C: active workflow step command

新增或扩展：

```text
UiCommandPayload::SetAuthoringWorkflowStep { step_id }
ui_command_id_for_payload
workspace_domain_for_payload
workspace_payload_kind
EditorCommandRegistry entry
EditorCommandExecutor handling
editor_input hit target mapping
```

最低测试：

```powershell
cargo test -p editor_ui_model authoring_workflow
cargo test -p editor_core authoring_workflow
cargo test -p editor_input
```

验收：

```text
点击 workflow step 能产生稳定 command payload。
command 不执行业务，只切换 active step / active workspace domain。
不可用 step 不应该触发业务执行。
```

### Stage D: editor_ui_renderer workflow rail

新增：

```text
panels/authoring_workflow.rs
Workflow rail draw
Next task summary draw
Blocking issue summary draw
HitRegion for workflow step
```

最低测试：

```powershell
cargo test -p editor_ui_renderer authoring_workflow
```

验收：

```text
DrawCommand 中能看到 Project / Assets / Scene / Prefabs / Rules / Input / AUI / Play / Build / Reports。
至少一个 next task 可显示。
至少一个 blocking issue 可显示。
step hit region 可映射到 SetAuthoringWorkflowStep。
```

### Stage E: AI context 接入

接入：

```text
WorkspaceContextForAI 或现有 AI context summary
```

最低测试：

```powershell
cargo test -p editor_core authoring_workspace
cargo test -p editor_core authoring_workflow
```

验收：

```text
AI context 可读 active_step。
AI context 可读 blocking_issues。
AI context 可读 recommended_tasks。
AI context 可读 available_commands。
```

### Stage F: 文档同步

更新：

```text
00-文档地图.md
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
阶段完成记录/2026-07-03-Complex-Shooter-Authoring-Workflow-v1/00-总览.md
```

验收：

```text
当前施工文档移动到 施工文档/已完成/
54 显示无当前可施工文档或下一建议入口
阶段完成记录列出测试命令和结果
```

## 5. 整体回归命令

施工完成后必须执行：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_ui_renderer
cargo test -p editor_input
```

如影响窗口层，再补：

```powershell
cargo test -p editor_window_winit
```

## 6. 施工自审

Specification fit:

```text
符合用户要求：从测试系统回到复杂打飞机真实编辑器创作流程。
符合 175：实现 workflow spine C-min。
不讨论也不实现新的测试系统。
```

Design fit:

```text
模型在 editor_ui_model。
聚合在 editor_core。
渲染在 editor_ui_renderer。
输入映射在 editor_input。
workflow 不直接读写项目文件，不直接执行 Build / Play / Import。
```

Long-term fit:

```text
用显式 workflow state 替代散装面板补丁。
不绑定打飞机玩法。
可扩展到其它复杂项目。
```

Implementation feasibility:

```text
已有 ProjectAuthoringWorkspaceModel / WorkspaceDomainSummary 可作为输入。
已有 command framework 可复用。
已有 renderer panel 分层可扩展。
```

Risk:

```text
最大风险是 workflow 变成第二套业务层。
施工时必须坚持 workflow 只聚合状态和 command 引用。
```

结论：

```text
施工文档通过自审，可以按 Stage A-F 开始施工。
```
