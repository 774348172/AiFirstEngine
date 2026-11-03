# 193-当前可自动化施工文档-Rule Authoring Productization v1

> 来源方案：`193-Rule-Authoring-Productization-v1方案.md`
> 外部审查：`其它AI审查目录/25-193-Rule-Authoring-Productization方案审查.md`
> 施工结论：审查建议已采纳到 193，允许按本施工文档开工。

## 1. 施工目标

把 Rule 域从 `focus_rule_panel / missing command` 推进到真实可执行的产品化入口：

```text
RuleAuthoringService
RuleAuthoringDocument / RuleAuthoringReport / RuleAuthoringPatch
Rule UiCommandPayload
WorkflowCommandResolver Rule commands
ManualWalkthroughCoverageReport Rule executable / needs context evidence
Validate / Compile / Artifact report
project_e2e_gate rule-authoring-productization-report.json
```

本轮不做：

```text
完整节点图编辑器
完整 Readable DSL 编辑器
完整 Rule Card 多视图系统
项目玩法专用 API
Runtime 动态 DLL 热更
```

## 2. 审查处理

外部审查认为 193 方案可直接施工，但建议补三点。已处理：

```text
RuleAuthoringDocument / Report / Patch -> editor_ui_model
RuleAuthoringService -> editor_core
Rule UiCommandPayload -> editor_ui_model/src/command.rs
Diagnostic code -> deterministic explanation template -> Gate D
ManualWalkthroughCoverageReport Rule 域从 focus/missing 推进到 executable / needs context -> Gate C
```

## 3. 涉及文件

预计新增：

```text
rust/crates/editor_ui_model/src/rule_authoring.rs
rust/crates/editor_core/src/rule_authoring.rs
rust/crates/editor_core/src/services/rule_service.rs
rust/crates/project_e2e_gate/src/rule_authoring.rs
```

预计修改：

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_ui_model/src/workflow_command.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_ui_model/src/model.rs
rust/crates/editor_ui_model/src/tests.rs

rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/editor_command.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/ui_model_composer.rs

rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs

框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

## 4. Gate A：Rule Authoring 模型与服务

任务：

```text
新增 editor_ui_model::rule_authoring 模型。
新增 editor_core::RuleAuthoringService。
支持 create/open/save ProjectRuleAsset。
生成 RuleAuthoringDocument / Report / Patch。
human_summary 由结构化规则确定性生成。
```

测试：

```text
cargo test -p editor_core rule_authoring
cargo test -p engine_runtime project_rule_asset
```

## 5. Gate B：结构化 IR 编辑命令

任务：

```text
支持 set trigger。
支持 add/update/remove statement。
支持 add/update/remove operation。
支持 expected_ir_hash 冲突诊断。
所有修改只写 ProjectRuleAsset.canonical_ir。
```

测试：

```text
cargo test -p editor_core rule_authoring
cargo test -p engine_runtime rule_ir
```

## 6. Gate C：Workflow / UiCommand 接入

任务：

```text
新增 Rule UiCommandPayload。
注册 EditorCommandRegistry rule descriptors。
EditorSession 支持 create/open/select/edit/validate/build/save/diagnostics 最小命令。
WorkflowCommandResolver 识别 Rule 域命令。
Rules step 从 focus_rule_panel 升级为 create_rule_asset / open_rule_asset / validate_rule_asset。
manual walkthrough Rule 域 Create/Open/Edit/Validate/Build/Diagnostics 进入 executable / needs context。
```

测试：

```text
cargo test -p editor_ui_model workflow_command
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core authoring_workflow
cargo test -p editor_core manual_walkthrough
```

## 7. Gate D：Validate / Compile / Artifact Report

任务：

```text
RuleAuthoringService 调 ProjectRuleAsset.validate。
RuleAuthoringService 调 RuleCompiler::compile。
生成 static registry source evidence。
从 RuleArtifactManifest / RuleArtifactRegistry / RuntimeRuleManifest 生成 lifecycle evidence。
实现 diagnostic code -> deterministic explanation template。
覆盖 InvalidFieldPath / MissingActionId / MissingPrefabRef / fallback。
report 区分 produced / validated / skipped_by_v1，不把 skipped 写成 passed。
```

测试：

```text
cargo test -p editor_core rule_authoring
cargo test -p engine_runtime rule_compiler
cargo test -p engine_runtime rule_artifact
```

## 8. Gate E：Complex Shooter Rule Authoring Report

任务：

```text
project_e2e_gate 生成 rule-authoring-productization-report.json。
覆盖 samples/complex_shooter_project 的 Rule authoring 状态。
报告 sample rule count / validation / compile evidence / manual walkthrough summary / next_actions。
报告不得引入 Player / Enemy / Bullet / Score / Health / Weapon 等引擎 API。
```

测试：

```text
cargo test -p project_e2e_gate rule_authoring
cargo test -p project_e2e_gate manual_walkthrough
```

## 9. 整体回归

全部 Gate 通过后运行：

```text
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p project_e2e_gate
```

如整体回归暴露本轮相关问题，必须修复后重跑对应测试。

## 10. 完成后文档同步

施工完成且测试通过后：

```text
更新 49-当前状态与下一步入口.md
更新 54-当前可自动化施工总入口.md
更新 施工文档/README.md
新增 阶段完成记录/2026-07-05-Rule-Authoring-Productization-v1/00-总览.md
更新 阶段完成记录/README.md
把本施工文档从 施工文档/当前/ 移动到 施工文档/已完成/
```

## 11. 施工自审

自审结论：

```text
范围与 193 方案一致。
采纳了 25 号审查文档的低风险建议。
没有扩大到节点图 / DSL / Rule Card。
没有把项目玩法 API 写入 engine core。
每个 Gate 都有测试命令。
完成后有整体回归和文档归档要求。
```

允许开工。
