# 203-当前可自动化施工文档：Prefab Authoring Productization v1

## 1. 施工目标

本轮只执行 `203-Prefab-Authoring-Productization-v1方案.md` 中已确定的 B-min：

```text
Unity-like Prefab Mode 最小产品化。
```

目标是把已有 M7 Prefab 底座产品化为用户和 AI 都能稳定使用的 authoring 闭环：

```text
PrefabAsset / PrefabInstance / PrefabOverride
  -> PrefabStageModel / PrefabStageReport
  -> Create / Open / Enter Stage / Edit / Save / Exit
  -> Instantiate / Override / Apply Single Override / Revert
  -> Validate / Report / Manual Walkthrough / AI Context / E2E Report
```

本轮不重写 runtime prefab，不做 Nested Prefab / Variant / inheritance / unpack，不做 ProjectPatch Prefab capability v2，不加入任何复杂打飞机玩法专用 API。

## 2. 涉及文件

预计涉及：

```text
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_ui_model/src/workflow_command.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs

rust/crates/editor_core/src/prefab_workflow.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/services/mod.rs
rust/crates/editor_core/src/services/scene_service.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/authoring_workflow.rs
rust/crates/editor_core/src/tests/*

rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/project_e2e_gate/src/prefab_authoring.rs

框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-05-Prefab-Authoring-Productization-v1/00-总览.md
```

实际施工以现有代码结构为准；若某个入口已移动，应复用现有模块，不新造平行体系。

## 3. Gate A：Prefab Command Surface

任务：

```text
在 editor_ui_model 增加 Prefab authoring UiCommandPayload。
新增 PrefabStageMode / PrefabStageSavePolicy 等 UI 命令层枚举。
把 Prefab 命令映射到 WorkspaceDomainKind::Prefab。
WorkflowCommandResolver 能解析 open / enter / instantiate / save / validate / apply / revert 等最小命令。
Manual Walkthrough 中 Prefab 域从单纯缺口推进到 executable_needs_context / executable。
```

测试：

```powershell
cargo test -p editor_ui_model workflow_command
cargo test -p editor_ui_model manual_walkthrough
```

## 4. Gate B：PrefabStageModel / Authoring Service

任务：

```text
在 editor_core prefab_workflow 增加 PrefabStageModel / PrefabStageReport / PrefabAuthoringModel / PrefabAuthoringReport。
实现打开 PrefabAsset 进入 stage。
实现 stage 内字段编辑写 working_prefab。
实现 save / exit 的最小语义。
实现 apply single override to prefab 与 revert override。
实现 validate/report，报告 prefab_assets_count / prefab_instances_count / diagnostics / next_actions。
```

测试：

```powershell
cargo test -p editor_core prefab
cargo test -p editor_core inspector
```

## 5. Gate C：EditorSession Command Integration

任务：

```text
EditorCommandRegistry 暴露 Prefab authoring commands。
EditorSession 能执行 Prefab authoring command。
CreatePrefabFromSelection 使用当前 scene entity tree 生成 PrefabAsset。
Open/Enter/Save/Exit 走 PrefabStageModel。
InstantiatePrefabInScene 复用已有 prefab instance 数据结构。
Prefab instance override 的 apply/revert 进入 scene dirty/report 链路。
命令失败必须返回结构化 diagnostic，不伪装成功。
```

测试：

```powershell
cargo test -p editor_core editor_command_registry
cargo test -p editor_core prefab
```

## 6. Gate D：Manual Walkthrough / AI Context

任务：

```text
ManualWalkthroughCoverageAnalyzer 能把 Prefab authoring productization 识别为已具备真实命令。
AuthoringAiContext 暴露 Prefab authoring summary。
报告中明确 ProjectPatch Prefab capability v2 仍延期，不因本轮自动开启。
```

测试：

```powershell
cargo test -p editor_core manual_walkthrough
cargo test -p editor_core authoring_workflow
```

## 7. Gate E：Complex Shooter Prefab Productization Report

任务：

```text
project_e2e_gate 新增 complex-shooter-prefab-authoring-productization-report.json。
扫描 sample project Prefabs/*.prefab.json。
扫描 Scene 中真实 engine.prefab_instance 数量，区分 prefab_assets_count 与 prefab_instances_count。
通过临时项目或测试场景验证 instantiate / override / apply / revert / save / validate 链路。
报告能输出 source path / prefab_ref / instance_id / field_path / diagnostics / next_actions。
```

测试：

```powershell
cargo test -p project_e2e_gate prefab
cargo test -p project_e2e_gate
```

## 8. Gate F：文档同步、阶段记录、归档与整体回归

任务：

```text
生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
把本施工文档从 施工文档/当前 移动到 施工文档/已完成。
```

整体回归：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core prefab
cargo test -p project_e2e_gate
```

## 9. 施工自审

自审结论：

```text
范围与 203 B-min 一致：做最小 Prefab Stage / Prefab Mode 产品化，不做完整 Unity Prefab 系统。
没有扩大到 Nested / Variant / inheritance / unpack / batch apply。
没有把 PrefabStageModel 放入 RuntimePackage；RuntimePackage 仍只消费保存后的 PrefabAsset cooked output。
没有提前施工 ProjectPatch Prefab capability v2。
每个 Gate 都有测试命令，不能跳过阶段测试。
E2E 报告必须诚实区分 prefab assets 和 scene prefab instances，不能把普通复制实体误报为 PrefabInstance。
```

风险处理：

```text
若现有 EditorSession 缺少某些 scene selection / save path 上下文，命令返回 needs context / diagnostic，不伪造成功。
若 apply override 会修改 PrefabAsset，v1 只允许单条 override apply，并保留 report。
若施工中发现正式方案错误，先回填 203 方案或本施工文档，再继续实现。
```
