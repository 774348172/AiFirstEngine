# 205-当前可自动化施工文档-AI Project Patch Structured Output / Imported Patch Productization v2

## 1. 施工来源

正式方案：

```text
框架设计/引擎总体架构/205-AI-Project-Patch-Structured-Output-Imported-Patch-Productization-v2方案.md
```

采用方案：

```text
方案 B：Imported Patch B-min
```

当前施工目录检查：

```text
施工文档/当前/ 施工前为空。
本文是当前唯一可执行施工文档。
```

## 2. 施工目标

把 202 已完成的 ProjectPatch 内部产品化，升级为可导入外部结构化 patch 的正式入口：

```text
Imported JSON / file / fixture
  -> ProjectPatchImportService
  -> parse / schema / capability / validation / review
  -> AI Panel / proposal evidence
  -> confirm apply
  -> EditorSession::execute_patch_as_transaction
  -> PatchApplyReport / PatchHistorySummary
  -> complex shooter imported patch report
```

完成后必须证明：

```text
外部 ProjectPatchDocument JSON 可以被导入、预览、审阅、确认执行。
非法 JSON / schema / unsupported capability 会生成结构化 diagnostics，并且不修改项目。
Scene/Input imported patch 仍走正式 EditorSession / CommandTransaction / ProjectPatchValidator。
Prefab / AUI / Rule / Asset / Build patch capability 仍诚实报告 unsupported / deferred。
```

## 3. 不做范围

本轮不做：

```text
真实 LLM provider 接入。
Prompt 模板、streaming structured output、多模型路由。
Prefab / AUI / Rule / Asset / Build patch capability。
直接写项目文件。
新的事务系统。
打飞机专用 patch operation。
```

## 4. 涉及文件

预计修改：

```text
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/ai_panel.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_ui_model/src/workflow_command.rs
rust/crates/editor_ui_model/src/tests.rs

rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/project_patch/import.rs
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/authoring_workflow.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/project_patch_tests.rs

rust/crates/project_e2e_gate/src/project_patch.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
```

按实际结构允许微调，但不能绕过：

```text
ProjectPatchImportService
PatchValidator
EditorSession::execute_patch_as_transaction
AI proposal review evidence
project_e2e_gate imported patch artifact
```

预计文档同步：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-05-AI-Project-Patch-Structured-Output-Imported-Patch-Productization-v2/00-总览.md
```

施工完成后将本文移动到：

```text
框架设计/引擎总体架构/施工文档/已完成/205-当前可自动化施工文档-AI-Project-Patch-Structured-Output-Imported-Patch-Productization-v2.md
```

## 5. Gate A：Import Model / Service

目标：

```text
新增 ProjectPatchImportRequest / Result / ProductizationReport。
新增 ProjectPatchImportService，支持 JSON string / file path。
```

任务：

```text
1. 在 project_patch model 中新增 source_kind / parse_status / import result / import report。
2. 新增 project_patch/import.rs，提供 from_json_string / from_file。
3. parse 失败必须返回 PatchDiagnostic，不 panic。
4. schema / capability / validation / review 必须形成结构化结果。
5. 不做真实 LLM，不做全域 capability。
```

测试：

```powershell
cd rust
cargo test -p editor_core project_patch_import
```

通过标准：

```text
合法 ProjectPatch JSON 可导入并生成 PatchReviewModel。
非法 JSON / schema mismatch 可报告。
unsupported capability 可报告 next_action。
```

## 6. Gate B：Command Surface / AI Panel Review

目标：

```text
新增 imported patch command surface，并让 AI Panel / editor_ui_model 可序列化 imported patch evidence。
```

任务：

```text
1. 新增 ImportProjectPatch / PreviewImportedProjectPatch / ApplyImportedProjectPatch command 或等价 payload。
2. 增加 command_id 映射和 editor command registry descriptor。
3. AiProposedCommand 增加 imported patch evidence，保留既有 project_patch evidence 兼容。
4. EditorSession 保存 imported proposal_id -> ProjectPatchDocument。
5. import/preview 只生成 proposal/review，不直接 apply。
```

测试：

```powershell
cd rust
cargo test -p editor_ui_model ai_panel
cargo test -p editor_core ai_project_patch
```

通过标准：

```text
AI Panel model 可序列化 imported patch evidence。
preview command 不修改项目。
command registry 能列出 imported patch command。
```

## 7. Gate C：Apply Imported Patch

目标：

```text
导入后的 patch proposal 能确认执行，进入 execute_patch_as_transaction。
非法 patch 不修改项目。
```

任务：

```text
1. ApplyImportedProjectPatch 通过 proposal_id 找到 imported patch。
2. apply 必须复用 execute_patch_as_transaction。
3. 失败路径返回 CommandResult diagnostics。
4. 非法 / unsupported patch 不能写项目文件。
5. 保持 AiAcceptProposedCommand 兼容 202 的内部 proposal。
```

测试：

```powershell
cd rust
cargo test -p editor_core project_patch
cargo test -p editor_core project_patch_import
```

通过标准：

```text
合法 imported Scene/Input patch 可 committed。
非法 imported patch rejected 且 no mutation。
PatchHistorySummary 可看到 imported patch 记录。
```

## 8. Gate D：Manual Walkthrough / AI Context

目标：

```text
ManualWalkthrough / AuthoringAiContext 能报告 imported patch 能力、needs context 和 unsupported domains。
```

任务：

```text
1. ManualWalkthrough 增加 import_project_patch / preview_imported_project_patch / apply_imported_project_patch / inspect_project_patch_report。
2. 有 JSON / path 上下文时 executable；无上下文时 ExecutableCommandNeedsContext。
3. AuthoringAiContext 暴露 imported patch summary 和 deferred domain。
4. 不伪造空 JSON / 空 path command。
```

测试：

```powershell
cd rust
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core manual_walkthrough
cargo test -p editor_core authoring_workflow
```

通过标准：

```text
manual walkthrough 可看到 imported patch 操作。
AI context 能说明当前支持 Scene/Input import，Prefab/AUI/Rule/Asset/Build deferred。
```

## 9. Gate E：Complex Shooter E2E

目标：

```text
project_e2e_gate 生成 complex-shooter-imported-project-patch-productization-report.json。
```

任务：

```text
1. 扩展 project_e2e_gate project_patch scenario。
2. 在 temp project / output 下写 imported patch fixture，不污染样例源文件。
3. 对 samples/complex_shooter_project 导入、预览、应用 Scene/Input patch。
4. 写出结构化 imported patch report artifact。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate project_patch
cargo test -p project_e2e_gate
```

通过标准：

```text
complex-shooter-imported-project-patch-productization-report.json 存在。
报告含 parse / validation / review / apply / history_summary。
样例源项目不被测试污染。
```

## 10. Gate F：整体回归与文档同步

目标：

```text
确认 202 / 203 / 204 不回退，完成文档归档。
```

测试：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core project_patch
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
文档入口显示 205 已完成。
施工文档/当前/ 恢复为空。
```

## 11. 施工自审

### 是否符合 205 方案

符合。本文只施工 Imported Patch B-min。

### 是否范围过大

未扩大。本文明确不做真实 LLM、不做全域 patch、不做 Prefab/AUI/Rule/Asset/Build capability。

### 是否跳过测试

未跳过。每个 Gate 都有测试命令，Gate F 有整体回归。

### 是否可能绕过正式链路

已禁止：

```text
不直接写项目文件。
不绕过 PatchValidator。
不绕过 EditorSession::execute_patch_as_transaction。
不把 unsupported domain 静默忽略。
```

### 是否与已完成系统冲突

不冲突：

```text
202 ProjectPatch v1 保持内部 proposal 链路。
203 Prefab authoring 不自动获得 patch capability。
204 AUI authoring 不自动获得 patch capability。
```

### 残留风险

```text
AI Panel 仍保留 UiCommandPayload 兼容字段，本轮只补 imported patch evidence。
真实 LLM structured output 仍需后续 v3。
Scene/Input 以外 domain 仍 deferred，必须在 report 中诚实暴露。
```
