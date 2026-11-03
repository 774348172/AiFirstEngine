# 120-当前可自动化施工文档：Native Editor Usable Panels B+AI C-min

## 1. 目标

根据 `120-Native-Editor-Usable-Panels-B-AI-C-min方案.md`，实现 Native Editor 第一版可用面板闭环。

本次施工只做：

```text
Toolbar
Hierarchy 深度可用的最小动作
Inspector 深度可用的最小动作
Viewport Shell
Console
RuntimeTrace
AI Panel mock planner
```

不做：

```text
Dock / Tab
Asset Browser
Build Panel
真实 AI 模型调用
复杂 TextInput
复杂 Gizmo
完整资源选择器
```

## 2. 施工步骤

### A1：扩展 editor_ui_model

新增：

```text
AiPanelModel
AiPanelMessage
AiPanelResponse
AiProposedCommand
AiCommandReviewState
```

扩展：

```text
EditorUiModel.ai_panel
UiCommandPayload::RenameSceneEntity
UiCommandPayload::AiSubmitPrompt
UiCommandPayload::AiAcceptProposedCommand
UiCommandPayload::AiRejectProposedCommand
InspectorField.editable
```

### A2：扩展 EditorSession

实现：

```text
RenameSceneEntity -> SceneEditCommand::RenameEntity 或等价编辑命令
AiSubmitPrompt -> deterministic mock planner
AiAcceptProposedCommand -> 执行 proposed UiCommand
AiRejectProposedCommand -> 清理 / 标记 rejected
build_ai_panel_model
```

### A3：扩展 SelfUiRenderer

实现绘制：

```text
AI Panel
Inspector field rows
Hierarchy action hit region
AI proposed command hit region
```

新增 HitTarget：

```text
InspectorField { field_id }
HierarchyAction { action_id }
AiPanelAction { action_id }
AiProposedCommand { proposal_id }
```

### A4：扩展 EditorInputRouter

实现：

```text
Hierarchy create / delete / rename action -> UiCommand
AI accept / reject proposed command -> UiCommand
Inspector field hit first version -> no direct text edit, but route helper preserved
```

文本输入第一版不走真实键盘输入，测试通过 helper command 创建。

### A5：测试

必须新增或更新测试：

```text
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_input
cargo test -p editor_core
```

重点断言：

```text
ui_model_contains_ai_panel
renderer_outputs_ai_panel_hit_regions
input_routes_ai_accept_command
editor_session_renames_scene_entity
editor_session_ai_prompt_proposes_command
editor_session_accept_ai_command_executes_scene_edit
```

### A6：回归

至少运行：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_input
cargo test -p editor_core
```

如时间允许，运行：

```powershell
cargo test --workspace
```

## 3. 完成记录

施工完成后：

```text
施工文档/当前/120-... 移动到 施工文档/已完成/
新增 阶段完成记录/2026-xx-xx-Native-Editor-Usable-Panels-B-AI-C-min/00-总览.md
更新 49-当前状态与下一步入口.md
```

