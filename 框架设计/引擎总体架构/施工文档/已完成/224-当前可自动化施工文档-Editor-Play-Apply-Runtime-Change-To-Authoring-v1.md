# 224-当前可自动化施工文档-Editor Play Apply Runtime Change To Authoring v1

## 1. 施工目标

本次施工落地 `224-Editor Play Apply Runtime Change To Authoring v1` 的 B-min：

```text
Play Mode:
  Runtime Inspector TemporaryPlaySession 临时编辑
  -> Preview Apply Runtime Change candidate
  -> Confirm Apply Runtime Change To Authoring
  -> 写回 authoring Scene 或 Scene 内 Prefab instance override
  -> dirty / undo / report
```

明确不做：

```text
不做 Play Mode 自动保存。
不把 runtime World 整体复制回项目。
不写 RuntimePackage。
不写 Rule / AUI / ProjectUiStateSnapshot。
不 apply runtime-spawned entity as new Scene entity。
不直接 apply 到 Prefab Asset。
不做多选批量 Apply。
不做完整 component paste / full object diff。
不做 runtime undo stack。
不做 Vec2 Inspector 全链路。
```

当前施工文档唯一性断言：

```text
施工前 `框架设计/引擎总体架构/施工文档/当前/` 为空。
本文件创建后，该目录只能保留本文件。
正式施工只按本文件执行。
```

## 2. 正式依据

正式方案：

```text
框架设计/引擎总体架构/224-Editor-Play-Apply-Runtime-Change-To-Authoring-v1方案.md
```

外部审查：

```text
其它AI审查目录/42-224-Editor-Play-Apply-Runtime-Change-To-Authoring方案审查.md
```

审查吸收：

```text
必须修改:
  MF-1 RuntimeTemporaryEditRecord 已存在，施工必须扩展现有结构，不新建同名结构。
  MF-2 Origin 追踪必须采用 EditorRuntimePlayInstance 保留 hydration report 并建 runtime->origin 反向索引。

施工约束:
  SC-1 Transform 子字段 apply 必须 read-merge-write。
  SC-2 Runtime temporary edit record list 保留并按 entity/component/field 去重留最新。
  SC-3 Prefab route 只写 Scene 内 instance override，不 apply 到 Prefab Asset。
  SC-4 Apply 前校验 runtime scene_id 与打开的 authoring scene_id。
  SC-5 Apply 成功记录不进入 Stop discard 计数。
  SC-6 一次 Apply 是一个原子 authoring transaction。
  SC-7 candidate_hash 输入和 Confirm live re-read 校验。
  SC-8 Gate F 补 editor_ui_model 与全量 editor_core。

已由历史施工吸收:
  223 闭环风险已由阶段完成记录吸收。
```

## 3. 涉及文件

预计修改：

```text
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_ui_model/src/tests.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/runtime_apply.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/services/scene_service.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/editor_core/src/tests/runtime_apply_tests.rs
rust/crates/editor_core/src/tests/runtime_service_tests.rs
rust/crates/project_e2e_gate/src/editor_gameview_play.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

如实际施工发现某些文件不需要修改，以最小实现为准。

## 4. Gate A：现状锁定

任务：

```text
确认当前施工目录只有本文件。
确认 224 正式方案已吸收外部审查 MF-1 / MF-2 / SC-1~SC-8。
锁定 RuntimeTemporaryEditRecord / RuntimeTemporaryEditSummary / SceneEditCommand / PrefabOverride 基线。
```

测试/命令：

```powershell
Get-ChildItem -Path "框架设计/引擎总体架构/施工文档/当前"
rg -n "struct RuntimeTemporaryEditRecord|struct RuntimeTemporaryEditSummary|enum SceneEditCommand|struct PrefabOverride|ApplyRuntimeChange" rust/crates
```

通过标准：

```text
施工目录只有本文件。
方案已吸收审查。
没有已有 ApplyRuntimeChange 实现冲突。
```

## 5. Gate B：Runtime authoring origin index

任务：

```text
新增 RuntimeAuthoringOrigin / RuntimeAuthoringOriginKind。
EditorRuntimePlayInstance::start 保留 RuntimeSceneHydrationReport。
不修改 World / EntityMeta。
基于 RuntimeSceneInstance.source_to_runtime_entity 建 runtime_entity_id -> RuntimeAuthoringOrigin 反向索引。
Scene entity 可回到 authoring Scene entity。
unknown / runtime-spawned entity 返回 blocked origin。
Prefab instance route 只保留 B-min 数据结构与 Scene 内 instance override 目标，不 apply Prefab Asset。
```

测试/命令：

```powershell
cargo test -p editor_core runtime_apply -- --nocapture
cargo test -p editor_core runtime_service_tests -- --nocapture
```

通过标准：

```text
active runtime entity 能查询 authoring origin。
不在 origin map 中的 runtime entity 被标记为 runtime_spawned / unknown。
```

## 6. Gate C：Temporary edit record list

任务：

```text
扩展现有 RuntimeTemporaryEditRecord，不新建同名结构。
新增 edit_id / value_after / authoring_origin / apply_policy / sequence / applied。
EditorRuntimePlayInstance 保留 pending RuntimeTemporaryEditRecord list。
按 (entity_id, component_type, field_path) 去重保留最新 value_after。
RuntimeTemporaryEditSummary 只统计 pending 未 apply record。
```

测试/命令：

```powershell
cargo test -p editor_core runtime_service_tests -- --nocapture
```

通过标准：

```text
Temporary edit 后 record list 可生成 candidate。
同字段重复编辑只保留最新 value_after。
StopPlaySession discard summary 不统计已 applied record。
```

## 7. Gate D：Preview Apply candidates

任务：

```text
新增 UiCommandPayload::PreviewApplyRuntimeChangeToAuthoring。
新增 ApplyRuntimeChangeCandidate / ApplyRuntimeChangeReport。
Preview 从 pending records 生成 candidates。
candidate runtime_value 必须 live 读取 runtime World 结构化值。
candidate_hash 至少包含 runtime_entity_id / component_type / field_path / runtime_value / target_authoring_path。
ready / blocked / warning 状态和 diagnostics 可被 AI / tests 读取。
```

测试/命令：

```powershell
cargo test -p editor_ui_model -- --nocapture
cargo test -p editor_core runtime_apply -- --nocapture
```

通过标准：

```text
Scene-origin temporary edit 生成 ready candidate。
runtime-spawned / unknown 生成 blocked candidate。
scene_id mismatch 生成 blocked diagnostic。
```

## 8. Gate E：Confirm Apply To Authoring

任务：

```text
新增 UiCommandPayload::ApplyRuntimeChangeToAuthoring。
Confirm Apply 必须校验 candidate_hash，并 live re-read runtime value。
Apply 前校验 EditorRuntimePlayInstance.scene_id == editor_scene_document.scene_id。
Transform 子字段做 read-merge-write，最终走 SceneEditCommand::SetTransform whole Vec3。
普通 component field 走 SceneEditCommand::SetComponentField。
Prefab instance route 写 Scene 内 PrefabInstance override；C-min 不 apply Prefab Asset。
一次 Apply 是一个原子 authoring transaction。
Apply 成功记录标记 applied，不进入 Stop discard 计数。
```

测试/命令：

```powershell
cargo test -p editor_core runtime_apply -- --nocapture
cargo test -p editor_core prefab -- --nocapture
cargo test -p editor_core runtime_service_tests -- --nocapture
```

通过标准：

```text
Apply 后 authoring Scene dirty=true。
Stop Play 后 authoring 修改仍存在。
Undo 后 authoring 修改撤回。
stale candidate_hash 被拒绝。
不写 RuntimePackage。
```

## 9. Gate F：e2e / 回归 / 文档归档

任务：

```text
project_e2e_gate 覆盖复杂打飞机 Play Mode apply flow。
运行整体回归。
生成阶段完成记录。
更新入口文档。
施工文档归档。
```

测试/命令：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p project_e2e_gate -- --nocapture
```

文档同步：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-08-Editor-Play-Apply-Runtime-Change-To-Authoring-v1/
```

通过标准：

```text
Gate A-F 全部完成。
默认 headless 测试通过。
施工文档从 当前/ 移动到 已完成/。
当前/ 目录为空。
```

## 10. 施工文档自审

范围自审：

```text
本施工文档没有实现 Full Keep Simulation Changes。
没有 Play Mode 自动保存。
没有把 runtime World 整体复制回 authoring。
没有写 RuntimePackage / Rule / AUI / ProjectUiStateSnapshot。
没有新增 World / EntityMeta origin 字段。
没有 apply runtime-spawned entity。
没有直接 apply 到 Prefab Asset。
```

外部审查自审：

```text
已吸收 MF-1 / MF-2。
已把 SC-1~SC-8 写入 Gate 和测试。
审查中关于 223 闭环的历史风险已由 223 阶段完成记录吸收。
```

测试自审：

```text
每个 Gate 都有测试命令。
默认测试为 headless deterministic。
真实 OS window / GPU smoke 不作为必须 gate。
Gate F 包含 editor_ui_model、全量 editor_core、project_e2e_gate。
```

方案一致性自审：

```text
施工目标与 224 正式方案一致。
224 只新增显式 Preview/Apply command，不改变 223 默认 discard 行为。
Apply 成功记录不再作为 discarded 记录报告。
```

结论：

```text
本施工文档可执行。
下一步按 Gate A -> F 施工。
```
