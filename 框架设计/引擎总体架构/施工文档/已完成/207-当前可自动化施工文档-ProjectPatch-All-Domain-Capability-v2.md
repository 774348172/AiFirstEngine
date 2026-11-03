# 207-当前可自动化施工文档-ProjectPatch All-Domain Capability v2

## 1. 施工目标

按 `207-ProjectPatch-All-Domain-Capability-v2方案.md` 的方案 A / All-Domain A-min 施工：

```text
ProjectPatch 从 Scene / Input 扩展到 Asset / Prefab / AUI / Rule / Build。
外部 JSON / LLM ProjectPatch 仍走 ProjectPatchImportService -> PatchValidator -> Review -> User Confirm -> execute_patch_as_transaction。
新增 domain operation 必须映射到已有 EditorSession command / domain service，不允许 ProjectPatch 直接写项目文件。
写项目文件的 operation 必须有 ProjectFileSnapshotSet 回滚保护。
```

## 2. 方案与审查依据

```text
方案文档：
框架设计/引擎总体架构/207-ProjectPatch-All-Domain-Capability-v2方案.md

审查文档：
其它AI审查目录/27-207-ProjectPatch-All-Domain-Capability方案审查.md
```

采纳结论：

```text
采用全域方案 A，不推翻。
保留 12 Gate，不合并。
先做 Model / Validator / ProjectFileSnapshotSet，再接入写文件 domain applier。
Prefab / AUI / Rule / Build UiCommandPayload 当前已存在，但 Gate A 必须重新 sanity check。
```

## 3. 涉及文件

主要代码文件：

```text
rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/project_patch/validator.rs
rust/crates/editor_core/src/project_patch/applier.rs
rust/crates/editor_core/src/project_patch/session.rs
rust/crates/editor_core/src/project_patch/import.rs
rust/crates/editor_core/src/project_patch/llm_source.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/tests/project_patch_tests.rs
rust/crates/editor_core/src/tests/llm_patch_source_tests.rs
rust/crates/project_e2e_gate/src/project_patch.rs
```

可能只读或小范围同步文件：

```text
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_ui_model/src/authoring_workflow.rs
rust/crates/editor_core/src/authoring_workflow.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/lib.rs
```

文档同步文件：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-06-ProjectPatch-All-Domain-Capability-v2/00-总览.md
```

## 4. Gate 任务与测试

### Gate A：Model / Schema

任务：

```text
新增 AssetPatchOperation / PrefabPatchOperation / AuiPatchOperation / RulePatchOperation / BuildPatchOperation。
PatchOperation 支持五个新 domain。
operation_id / depends_on / kind / target_summary / capabilities_for_operations 全域可用。
核验现有 UiCommandPayload / EditorCommandRegistry / EditorSession route。
```

测试：

```powershell
cargo test -p editor_core project_patch_model
cargo test -p editor_core editor_command_registry
cargo test -p editor_ui_model ai_panel
cargo test -p editor_ui_model workflow_command
```

### Gate B：Validator

任务：

```text
新增 validate_asset / validate_prefab / validate_aui / validate_rule / validate_build。
Import capability diagnostics 不再把 Asset / Prefab / AUI / Rule / Build 默认为 unsupported。
路径必须 project-relative 且 normalize 后仍在 project_root 内。
payload decode / context 缺失 / profile 不支持要结构化拒绝。
```

测试：

```powershell
cargo test -p editor_core project_patch_validator
cargo test -p editor_core project_patch_import
```

### Gate C：Project File Snapshot Rollback

任务：

```text
新增 ProjectFileSnapshotSet 或等价内部结构。
execute_patch_as_transaction 在写文件 operation 前 snapshot project-relative path。
失败时恢复文件和 EditorSession 内存 snapshot。
snapshot 不完整时拒绝写文件 operation。
```

测试：

```powershell
cargo test -p editor_core project_patch_rollback
cargo test -p editor_core project_patch
```

### Gate D：Applier 映射

任务：

```text
PatchApplier 将全域 operation 映射到正式 UiCommandPayload 或已有 domain service command。
不新增 direct fs::write applier。
Build 作为验证 operation，不写项目源资产。
```

测试：

```powershell
cargo test -p editor_core project_patch_applier
cargo test -p editor_core ai_project_patch
```

### Gate E：Asset Capability

任务：

```text
支持 Asset.RegisterExistingAsset / GenerateMockImageAsset / ValidateAssetBrowserIndex。
复用 AssetBrowserIndex / AssetBrowserService / MockImageGenerationProvider / AssetPipelineState。
```

测试：

```powershell
cargo test -p editor_core asset_browser
cargo test -p editor_core ai_image_generation
cargo test -p editor_core project_patch_asset
```

### Gate F：Prefab Capability

任务：

```text
支持 Prefab.CreateFromSceneSelection / OpenDocument / SetStageEntityField / SaveDocument / InstantiateInScene / ApplyOverrideToAsset / RevertOverride / ValidateReferences。
全部走现有 Prefab UiCommandPayload / PrefabWorkflowService。
```

测试：

```powershell
cargo test -p editor_core prefab
cargo test -p editor_core project_patch_prefab
```

### Gate G：AUI Capability

任务：

```text
支持 Aui.CreateDocument / OpenDocument / AddNode / SetNodeField / SetBindingPath / SetActionRef / ValidateDocument / SaveDocument / PreviewOverlay。
全部走 AuiAuthoringService / AUI command。
```

测试：

```powershell
cargo test -p editor_core aui_authoring
cargo test -p engine_runtime aui
cargo test -p editor_core project_patch_aui
```

### Gate H：Rule Capability

任务：

```text
支持 Rule.CreateAsset / OpenAsset / SetTrigger / AddStatement / UpdateStatement / RemoveStatement / AddOperation / UpdateOperation / RemoveOperation / ValidateAsset / BuildArtifact。
保持 expected_ir_hash 乐观并发校验。
```

测试：

```powershell
cargo test -p editor_core rule_authoring
cargo test -p engine_runtime rule_compiler
cargo test -p editor_core project_patch_rule
```

### Gate I：Build Capability

任务：

```text
支持 Build.ExportDesktopPackage / OpenBuildReport / OpenBuildOutput。
Build 失败写入 PatchApplyReport diagnostics，不撤销已确认的项目资产修改。
```

测试：

```powershell
cargo test -p editor_core build_export
cargo test -p editor_core project_patch_build
```

### Gate J：LLM / Manual Walkthrough / AI Context

任务：

```text
ThinLlmPatchSource prompt 支持全域 capability。
Mock source 增加 all_domain / asset / prefab / aui / rule / build deterministic fixture。
ManualWalkthrough / AuthoringAiContext 不再把五个 domain 标为 deferred。
```

测试：

```powershell
cargo test -p editor_core llm_patch_source
cargo test -p editor_core authoring_workflow
cargo test -p editor_core manual_walkthrough
cargo test -p editor_ui_model manual_walkthrough
```

### Gate K：Complex Shooter E2E

任务：

```text
project_e2e_gate 生成 complex-shooter-all-domain-project-patch-report.json。
报告覆盖 Asset / Prefab / AUI / Rule / Build 至少各一个 operation。
```

测试：

```powershell
cargo test -p project_e2e_gate project_patch
cargo test -p project_e2e_gate
```

### Gate L：整体回归与文档同步

任务：

```text
整体回归。
生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
施工文档从 当前/ 移到 已完成/。
```

测试：

```powershell
cargo fmt --check
cargo test -p editor_core project_patch
cargo test -p editor_ui_model
cargo test -p engine_runtime aui
cargo test -p engine_runtime rule_compiler
cargo test -p project_e2e_gate
```

## 5. 施工自审

```text
范围自审：
  本施工只扩 ProjectPatch 全域 A-min capability，不做真实 LLM HTTP provider、Agent Planner、Repair Loop、installer/signing/store package。

架构自审：
  不新增独立 Router / Architecture Guard。
  不让 ProjectPatch 直接写项目文件。
  写文件 domain 必须先经过 ProjectFileSnapshotSet。

测试自审：
  每个 Gate 都有对应测试命令。
  Gate C 在 Gate D 和 domain apply 前执行。
  Gate L 必须跑整体回归后才算完成。

风险自审：
  Build operation 是验证动作，失败不撤销已确认的项目修改。
  全域 patch 的 operation_count 仍受限制，超限拒绝。
  禁止 Player / Enemy / Bullet / Score / Weapon 等玩法专用 engine API。
```

## 6. 完成与归档规则

```text
全部 Gate 通过后：
  生成 阶段完成记录/2026-07-06-ProjectPatch-All-Domain-Capability-v2/00-总览.md。
  更新 49-当前状态与下一步入口.md。
  更新 54-当前可自动化施工总入口.md。
  更新 施工文档/README.md。
  更新 阶段完成记录/README.md。
  将本文档移动到 施工文档/已完成/。
```
