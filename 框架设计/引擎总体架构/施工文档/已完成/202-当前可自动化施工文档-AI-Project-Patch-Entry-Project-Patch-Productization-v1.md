# 202-当前可自动化施工文档-AI Project Patch Entry / Project Patch Productization v1

## 1. 施工目标

对应方案：

```text
框架设计/引擎总体架构/202-AI-Project-Patch-Entry-Project-Patch-Productization-v1方案.md
```

本次目标：

```text
把已经存在的 ProjectPatch C-min 底座产品化。
```

施工完成后，必须形成：

```text
AI / Test / Imported Patch
  -> ProjectPatchDocument
  -> PatchValidationReport
  -> PatchReviewModel
  -> EditorSession::execute_patch_as_transaction
  -> PatchApplyReport
  -> PatchHistorySummary
  -> ProjectPatchProductizationReport
  -> manual walkthrough / AI context / complex shooter e2e evidence
```

本次不是从零实现 ProjectPatch。`181 / 182` 的 C-min 底座已经存在，本次只补产品化入口、报告、AI 可审阅证据和复杂项目 smoke gate。

## 2. 范围边界

本次真实支持：

```text
Scene patch:
  CreateEntity
  RenameEntity
  SetTransform
  SetComponentField
  PlaceAssetIntoScene

Input patch:
  AddInputAction
  AddInputBinding
  SetInputBindingDevicePath
```

本次允许内部 / inverse 使用：

```text
DeleteEntity
RemoveInputAction
RemoveInputBinding
```

本次不做：

```text
真实 LLM 接入
Asset / Prefab / AUI / Rule patch 真实应用
完整 visual diff viewer
完整多 patch merge / conflict resolution
Patch marketplace / plugin 扩展
复杂打飞机专用 patch operation
```

unsupported capability 规则：

```text
Asset / Prefab / Aui / Rule / Build 必须在 Validate 或 report 中诚实呈现 unsupported / next_actions。
不能降级成直接写文件。
不能静默忽略。
```

## 3. 方案审查结论

已读取：

```text
其它AI审查目录/09-M16 AI Project Patch Entry 方案审查.md
181-M16-AI-Project-Patch-Entry-C-min方案.md
182-当前可自动化施工文档-M16-AI-Project-Patch-Entry-C-min.md
202-AI-Project-Patch-Entry-Project-Patch-Productization-v1方案.md
```

采纳结论：

```text
09 审查指出 M16 不应从零造系统，而是在已有 UiCommand / EditorSession / CommandTransaction 上加 ProjectPatch 层。
当前代码已经完成 181 / 182 的核心 C-min。
202 方案把下一步收敛为产品化现有 C-min，而不是扩张成全域 patch。
```

无需修改 202 的判断：

```text
202 已明确 Scene/Input 为真实 apply，AUI/Rule/Prefab/Asset 为 unsupported / later capability。
202 已补充其它引擎源码级参考。
本施工文档只执行 202 的 C-min 产品化范围，不扩大到真实 LLM 或全域 patch。
```

## 4. 涉及文件

预计修改：

```text
G:\gameEngin\.agents\skills\ai-first-game-engine-project\SKILL.md
C:\Users\zenghaoran\.codex\skills\ai-first-game-engine-project\SKILL.md
框架设计/引擎总体架构/202-AI-Project-Patch-Entry-Project-Patch-Productization-v1方案.md
rust/crates/editor_ui_model/src/ai_panel.rs
rust/crates/editor_ui_model/src/authoring_workflow.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_model/src/tests.rs
rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/project_patch/session.rs
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/project_patch_tests.rs
rust/crates/editor_core/src/tests/ai_service_tests.rs
rust/crates/editor_core/src/tests/authoring_workflow_tests.rs
rust/crates/project_e2e_gate/src/project_patch.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

实际施工中如发现可少改文件，以少改为准。

## 5. Gate A：Patch Review Product Surface

目标：

```text
让 AI Panel / editor_ui_model 能序列化 ProjectPatch review evidence。
```

任务：

```text
1. 在 editor_ui_model 中新增或扩展 AI proposal 的 ProjectPatch evidence。
2. evidence 至少包含 patch_id、patch_title、touched_domains、operation_count、validation_status、diagnostics、requires_confirmation。
3. 保持现有 AiProposedCommand / UiCommandPayload 兼容，避免本 Gate 大改 UI command review。
4. 增加 editor_ui_model 序列化 / 模型测试。
```

测试：

```powershell
cargo test -p editor_ui_model ai_panel
cargo test -p editor_core ai_project_patch
```

## 6. Gate B：Accept ProjectPatch Proposal

目标：

```text
AI accept 路径能从 ProjectPatch proposal 进入 execute_patch_as_transaction。
```

任务：

```text
1. EditorSession 内部保存 AI proposal 对应的 ProjectPatchDocument。
2. accept ProjectPatch proposal 时执行 EditorSession::execute_patch_as_transaction。
3. 保留旧 UiCommand proposal 兼容路径。
4. AI create mock planner 必须产生 ProjectPatchDocument + PatchReviewModel evidence。
5. Patch accept 后记录 PatchApplyReport / PatchHistory。
```

测试：

```powershell
cargo test -p editor_core project_patch
cargo test -p editor_core ai_project_patch
```

## 7. Gate C：Patch Productization Report

目标：

```text
新增 ProjectPatchProductizationReport / PatchHistorySummary。
```

任务：

```text
1. 在 editor_core project_patch model 中新增产品化 report 数据结构。
2. 记录 supported_capabilities / unsupported_capabilities / next_actions。
3. 提供从 validation / review / apply / history 汇总 report 的函数。
4. 增加 unsupported capability report 测试。
```

测试：

```powershell
cargo test -p editor_core project_patch
```

## 8. Gate D：Manual Walkthrough / AI Context 接入

目标：

```text
coverage / ai context 能看到 ai_project_patch_entry 当前能力和 next_actions。
```

任务：

```text
1. manual walkthrough requirement matrix 增加 AI Project Patch Entry 操作或能力项。
2. coverage analyzer 能报告当前 Scene/Input patch productized，AUI/Rule/Prefab/Asset patch unsupported。
3. AuthoringAiContext 暴露 ProjectPatch 产品化摘要或 next_actions。
```

测试：

```powershell
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core manual_walkthrough
cargo test -p editor_core authoring_workflow
```

## 9. Gate E：Complex Shooter E2E Smoke

目标：

```text
project_e2e_gate 生成 ProjectPatch smoke artifact。
```

任务：

```text
1. 新增 project_e2e_gate ProjectPatch smoke runner。
2. 打开 samples/complex_shooter_project。
3. 构造 deterministic Scene/Input ProjectPatch。
4. 执行 validate / review / apply。
5. 写出 project-patch-productization-report.json。
6. 测试断言 report 包含 Scene/Input evidence，且 unsupported domains 不伪装 pass。
```

测试：

```powershell
cargo test -p project_e2e_gate project_patch
cargo test -p project_e2e_gate
```

## 10. Gate F：文档同步和整体回归

目标：

```text
完成入口同步、阶段记录、施工文档归档。
```

任务：

```text
1. 生成阶段完成记录：
   阶段完成记录/2026-07-05-AI-Project-Patch-Entry-Project-Patch-Productization-v1/00-总览.md
2. 更新 49 / 54 当前状态和最近完成。
3. 更新 施工文档/README.md。
4. 更新 阶段完成记录/README.md。
5. 把本施工文档从 当前/ 移到 已完成/。
```

整体测试：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core project_patch
cargo test -p project_e2e_gate
```

## 11. 施工自审

合乎方案：

```text
本施工文档只执行 202 方案 C-min 产品化范围。
不扩张到真实 LLM。
不扩张到 Asset / Prefab / AUI / Rule patch 真实应用。
```

合乎项目规则：

```text
先方案，后施工文档。
施工文档位于 施工文档/当前/。
每个 Gate 都有测试命令。
完成后要求整体回归、阶段记录、入口同步和归档。
```

合乎 AI-first：

```text
新增能力以 ProjectPatch / report / review / diagnostics 为真相。
AI 不直接写文件。
AI 不绕过 EditorSession / CommandTransaction。
```

合乎复杂项目维护：

```text
不写复杂打飞机专用 API。
Scene/Input 先产品化，unsupported domain 诚实报告。
```

结论：

```text
施工文档通过自审，可以开始施工。
```
