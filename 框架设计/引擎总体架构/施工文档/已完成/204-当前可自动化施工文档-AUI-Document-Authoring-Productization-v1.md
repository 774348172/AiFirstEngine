# 204-当前可自动化施工文档-AUI Document Authoring Productization v1

## 1. 施工来源

正式方案：

```text
框架设计/引擎总体架构/204-AUI-Document-Authoring-Productization-v1方案.md
```

用户已确认：

```text
B-min：AUI Authoring Service 产品化
```

当前施工目录检查：

```text
施工文档/当前/ 施工前为空。
本文是当前唯一可执行施工文档。
```

## 2. 施工目标

把已有 headless `AuiAuthoringService` 产品化为正式 AUI authoring domain：

```text
UiCommandPayload AUI command surface
  -> EditorSession AUI service
  -> AuiTransaction / AuiAuthoringReport / Preview report
  -> ManualWalkthrough / Workflow / AI context summary
  -> project_e2e_gate complex shooter AUI authoring productization report
```

完成后必须证明：

```text
用户 / AI 不需要直接手写 JSON 即可通过正式命令创建、打开、编辑、验证、保存、预览 AUI Document。
ManualWalkthrough 中 AUI authoring 操作不再全部 missing。
保存后的 AUI document 仍可进入现有 AuiDocumentCooker / RuntimePackage / Present 链路。
```

## 3. 不做范围

本轮不做：

```text
209 Scene Unified AUI Authoring。
完整可视化 UI Builder / UMG clone。
ProjectPatch AUI v2。
复杂 ScrollView / InputField / DragDrop 产品化。
runtime_text_glyph_present。
新的运行时架构层或 AUI Bridge。
打飞机专用 HUD API。
```

## 4. 涉及文件

预计修改：

```text
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_ui_model/src/workflow_command.rs
rust/crates/editor_core/src/aui_authoring.rs
rust/crates/editor_core/src/services/aui_service.rs
rust/crates/editor_core/src/services/mod.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/authoring_workflow.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/aui_authoring_tests.rs
rust/crates/project_e2e_gate/src/*
```

按实际代码结构允许微调，但不能绕过：

```text
UiCommandPayload
EditorSession / CommandTransaction
AuiAuthoringService
ManualWalkthroughCoverageReport
project_e2e_gate report artifact
```

预计文档同步：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-05-AUI-Document-Authoring-Productization-v1/00-总览.md
```

施工完成后将本文移动到：

```text
框架设计/引擎总体架构/施工文档/已完成/204-当前可自动化施工文档-AUI-Document-Authoring-Productization-v1.md
```

## 5. Gate A：AUI Command Surface

目标：

```text
新增 AUI UiCommandPayload / command_id / command descriptor。
ManualWalkthrough AUI 操作能识别 expected command。
```

任务：

```text
1. 在 editor_ui_model::UiCommandPayload 增加：
   CreateAuiDocument / OpenAuiDocument / AddAuiNode / SetAuiNodeField
   SetAuiBindingPath / SetAuiActionRef / ValidateAuiDocument / SaveAuiDocument / PreviewAuiOverlay。
2. 增加命令 id 映射。
3. 更新 ManualWalkthrough AUI operation requirement 的 expected_command_id / expected_payload_kind。
4. 更新 EditorCommandRegistry command descriptors。
```

测试：

```powershell
cd rust
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core editor_command_registry
```

通过标准：

```text
AUI command id 可序列化。
ManualWalkthrough 不再把所有 AUI 操作都标记为 MissingCommand。
Command registry 能列出 AUI authoring command descriptor。
```

## 6. Gate B：EditorSession AUI Service

目标：

```text
EditorSession 支持 create / open / add_node / set_field / set_binding / set_action / validate / save。
```

任务：

```text
1. 新增 editor_core::services::aui_service。
2. 补强 AuiAuthoringService：action_refs 编辑、binding path upsert、最小 field path。
3. EditorSession::execute_command 接入 AUI payload。
4. 所有写文件必须走 service / transaction，不直接文本拼 JSON。
5. 更新 / 新增 editor_core AUI authoring tests。
```

测试：

```powershell
cd rust
cargo test -p editor_core aui_authoring
```

通过标准：

```text
create/open/add/edit/binding/action/validate/save 均有测试覆盖。
失败路径输出 EditorDiagnostic 或 AuiAuthoringDiagnostic。
保存后的 AUI document 能被 AuiDocumentCooker 读取。
```

## 7. Gate C：Preview / Report

目标：

```text
新增 AUI authoring productization report。
PreviewAuiOverlay 走 AuiRuntimePresenter 或诚实 partial。
```

任务：

```text
1. 定义 AuiDocumentAuthoringProductizationReport 或等价 report。
2. 预览读取 AuiDocument，通过 AuiRuntimePresenter 生成 AuiOverlayFrame。
3. report 必须输出 draw_item_count / text_command_count / image_command_count / glyph_present / diagnostics。
4. glyph_present=false 时报告 partial，不伪装真实文字可见。
```

测试：

```powershell
cd rust
cargo test -p editor_core aui
cargo test -p engine_runtime aui
```

通过标准：

```text
PreviewAuiOverlay 不使用 debug overlay。
Preview report 能说明成功、partial 或失败原因。
engine_runtime AUI 现有测试不回退。
```

## 8. Gate D：Workflow / AI Context / Manual Walkthrough

目标：

```text
Workflow / ManualWalkthrough / AuthoringAiContext 能报告 AUI authoring 产品化状态。
```

任务：

```text
1. WorkflowCommandResolver 对 AUI command 进入 executable / needs context。
2. ManualWalkthroughCoverageAnalyzer 能汇总 AUI authoring 状态。
3. AuthoringAiContext 暴露 AUI authoring summary。
4. 不伪造空 path / 空 node_id command。
```

测试：

```powershell
cd rust
cargo test -p editor_ui_model workflow_command
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core authoring_workflow
cargo test -p editor_core manual_walkthrough
```

通过标准：

```text
ManualWalkthrough AUI missing_count 收敛。
需要具体 path / node 的操作诚实标记 ExecutableCommandNeedsContext。
AI context 能指出 AUI authoring 已产品化或缺少上下文。
```

## 9. Gate E：Complex Shooter E2E

目标：

```text
project_e2e_gate 生成 complex-shooter-aui-authoring-productization-report.json。
```

任务：

```text
1. 新增 project_e2e_gate AUI authoring productization scenario。
2. 对 samples/complex_shooter_project/AUI/hud.aui.json 执行 open / edit / validate / preview。
3. 在 temp/output 中创建并保存一个 test AUI document，不污染样例源文件。
4. 写出结构化 report artifact。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate aui
cargo test -p project_e2e_gate manual_walkthrough
```

通过标准：

```text
complex shooter report 存在。
报告能看到 create/open/edit/binding/action/validate/save/preview 证据。
样例源文件未被 e2e 测试修改。
```

## 10. Gate F：整体回归与文档同步

目标：

```text
确认 AUI runtime / package / existing authoring domains 未回退，并完成文档归档。
```

测试：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime aui
cargo test -p project_e2e_gate
```

文档动作：

```text
1. 生成阶段完成记录。
2. 更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
3. 将本文从 施工文档/当前/ 移动到 施工文档/已完成/。
```

通过标准：

```text
所有 Gate 测试和整体回归通过。
文档入口显示 204 已完成。
施工文档/当前/ 恢复为空。
```

## 11. 施工自审

### 是否符合 204 方案

符合。本文只施工 B-min：AUI Authoring Service 产品化。

### 是否范围过大

未扩大。本文明确不做完整 Designer、ProjectPatch AUI v2、glyph present、复杂 UI 控件。

### 是否跳过测试

未跳过。每个 Gate 都有测试命令，Gate F 有整体回归。

### 是否可能绕过正式链路

已禁止：

```text
不直接文本拼 JSON。
不绕过 EditorSession 写 AUI。
不让 Runtime/Renderer 读取 authoring source。
```

### 是否与既有完成系统冲突

不冲突：

```text
190 / 199 runtime chain 保持不变。
202 ProjectPatch AUI 仍保持 unsupported，等待后续 v2。
203 Prefab 模式只作为产品化参考，不照搬 Prefab Stage。
```

### 残留风险

```text
AuiAuthoringService 当前字段编辑较窄，施工中只补最小可验证字段。
Preview 可能仍 glyph_present=false，必须诚实 partial。
ManualWalkthrough 中部分 AUI 操作可能需要 context，不能为 pass 伪造参数。
```
