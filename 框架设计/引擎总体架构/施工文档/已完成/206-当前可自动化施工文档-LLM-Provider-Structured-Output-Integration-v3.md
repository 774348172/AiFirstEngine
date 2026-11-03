# 206-当前可自动化施工文档-LLM Provider / Structured Output Integration v3

## 1. 施工目标

本轮施工目标：

```text
实现 Thin LLM Patch Source v3。
让 mock / optional real LLM source 作为 ProjectPatchDocument JSON 的薄输入来源。
生成后必须进入 ProjectPatchImportRequest::ai_structured_output -> ProjectPatchImportService。
```

采用方案：

```text
206-LLM-Provider-Structured-Output-Integration-v3方案.md
方案 B-lite：Thin LLM Patch Source
```

本轮不是：

```text
不是 Provider Registry。
不是 Agent Planner。
不是 Repair Loop。
不是独立 LLM Report 真相层。
不是自动 apply。
不是扩 Asset / Prefab / AUI / Rule / Build patch capability。
不是真实 HTTP provider 完整接入。
```

## 2. 审查输入与采纳

已读取审查文档：

```text
其它AI审查目录/26-206-LLM-Provider-Structured-Output方案审查.md
```

审查结论：

```text
206 B-lite 方案正确，可直接施工。
```

采纳事项：

```text
代码落点放在 rust/crates/editor_core/src/project_patch/llm_source.rs。
Mock source deterministic，覆盖合法、非法 JSON、provider error、unsupported capability。
Prompt 组装必须明确 only Scene/Input supported。
真实 provider 本轮只做 env-gated optional stub / skipped smoke。
本轮不引入 reqwest / ureq / async runtime。
```

## 3. 涉及文件

预计新增：

```text
rust/crates/editor_core/src/project_patch/llm_source.rs
rust/crates/editor_core/src/tests/llm_patch_source_tests.rs
阶段完成记录/2026-07-05-LLM-Provider-Structured-Output-Integration-v3/00-总览.md
```

预计修改：

```text
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/editor_core/src/authoring_workflow.rs
rust/crates/editor_core/src/tests/authoring_workflow_tests.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/ai_panel.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_ui_model/src/workflow_command.rs
rust/crates/editor_ui_model/src/tests.rs
rust/crates/project_e2e_gate/src/project_patch.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

如果实现中发现某个文件不需要改，保持不动。

## 4. Gate A：Thin Source Model

目标：

```text
新增 LlmPatchSourceConfig / LlmPatchSourceResult / LlmPatchSourceKind。
新增 build_project_patch_generation_prompt helper。
不新增 LlmStructuredOutputReport。
```

要求：

```text
Config / Result 只作为 editor_core 内部 ProjectPatch source 数据。
prompt helper 必须包含 only Scene/Input supported 说明。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core llm_patch_source
```

## 5. Gate B：Mock Source

目标：

```text
新增 deterministic mock source。
prompt 包含 create / 创建 / 新建 -> 合法 Scene.CreateEntity patch JSON。
prompt 包含 invalid_json -> 非法 JSON。
prompt 包含 provider_error -> provider error。
prompt 包含 unsupported / aui / rule / prefab -> unsupported capability patch。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core llm_patch_source
```

## 6. Gate C：Import Integration / AI Panel

目标：

```text
新增或复用 UI command，让用户 prompt 经过 Thin LLM Patch Source 生成 ProjectPatch JSON。
生成成功后进入 ProjectPatchImportRequest::ai_structured_output。
成功时生成 AI Panel ProjectPatch proposal。
失败时进入 CommandResult diagnostics / AI Panel message。
```

建议命令：

```text
GenerateProjectPatchFromPrompt { prompt }
```

规则：

```text
Preview 不修改项目。
Apply 必须继续使用 ApplyImportedProjectPatch / proposal confirmation。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core project_patch_import
cargo test -p editor_core ai_project_patch
cargo test -p editor_ui_model ai_panel
```

## 7. Gate D：Manual Walkthrough / AuthoringAiContext

目标：

```text
AuthoringAiContext 暴露 llm_patch_source_available / active_patch_source_kind / supported_project_patch_capabilities / next_actions。
ManualWalkthrough 增加 generate_project_patch_from_prompt 操作项。
不暴露 LLM request graph / provider registry / agent state。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core authoring_workflow
cargo test -p editor_core manual_walkthrough
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_ui_model workflow_command
```

## 8. Gate E：Complex Shooter E2E Mock Smoke

目标：

```text
project_e2e_gate 用 mock source 生成 ProjectPatch JSON。
复用 ProjectPatchImportProductizationReport。
写出 complex-shooter-llm-patch-source-productization-report.json。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p project_e2e_gate project_patch
cargo test -p project_e2e_gate
```

## 9. Gate F：Optional Real Source Stub / Ignored Smoke

目标：

```text
OpenAiCompatible / real source 本轮只做 env-gated optional stub。
无 env 时 skipped / not_enabled。
有 env 时也允许返回 not_implemented diagnostic。
不引入 HTTP 依赖。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core llm_real_patch_source -- --ignored
```

## 10. Gate G：整体回归与文档同步

目标：

```text
cargo fmt 通过。
核心 ProjectPatch / AI Panel / project_e2e_gate 回归通过。
生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
施工文档完成后归档到 施工文档/已完成/。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo fmt --check
cargo test -p editor_core project_patch
cargo test -p editor_core ai_project_patch
cargo test -p editor_ui_model
cargo test -p project_e2e_gate
```

## 11. 施工自审

施工前自审：

```text
范围没有扩到 Provider Registry。
范围没有扩到 Agent Planner / Repair Loop。
范围没有新增 LlmStructuredOutputReport。
范围没有扩 Asset / Prefab / AUI / Rule / Build patch capability。
范围没有引入 reqwest / ureq / async runtime。
范围没有自动 apply。
所有真实项目修改仍进入 ProjectPatchImportService / PatchValidator / execute_patch_as_transaction。
```

审查文档判断：

```text
26 审查没有要求修改方案方向。
已采纳低风险施工建议。
可以开始施工。
```

## 12. 完成条件

本轮完成必须满足：

```text
Gate A-G 测试按实际可执行范围通过。
真实 provider stub ignored/local-only 不阻塞默认测试。
ProjectPatchImportProductizationReport 仍是主要报告真相。
阶段完成记录写明完成范围、测试结果、残留风险。
施工文档移动到 施工文档/已完成/。
施工文档/当前/ 为空。
```
