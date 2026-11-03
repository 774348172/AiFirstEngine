# 223-当前可自动化施工文档-Editor Runtime Hierarchy / Play Mode Temporary Inspector Edit Productization v1

## 1. 施工目标

本次施工落地 `223-Editor Runtime Hierarchy / Play Mode Temporary Inspector Edit Productization v1` 的 C-min：

```text
Play Mode:
  单一 Hierarchy 面板自动显示 active EditorRuntimePlayInstance live World
  不显示 Authoring / Runtime / Package domain toggle
  点击 Runtime Hierarchy 节点选择 active runtime entity
  Inspector 支持 Play Mode temporary edit
  临时编辑只写 active runtime World
  StopPlaySession 后临时编辑全部丢弃
```

明确不做：

```text
不把 runtime edit 写回 Scene / Prefab / Rule / RuntimePackage。
不新增用户可见 domain toggle。
不做 Apply Runtime Change To Authoring。
不做 runtime undo stack。
不做 runtime multi-selection。
不做 runtime hierarchy search/filter/component badges 完整版。
不做 runtime spawn/despawn/reparent/reorder。
不做 Vec2 Inspector 全链路，Collider2D.offset 默认不进入 C-min。
不做 FullWorldStateDump / 统一 RuntimeStateSnapshot。
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
框架设计/引擎总体架构/223-Editor-Runtime-Hierarchy-PlayMode-Temporary-Inspector-Edit-Productization-v1方案.md
```

前置完成能力：

```text
217 Editor Play 使用 Preview RuntimePackage。
218 EditorRuntimePlayInstance 创建 active runtime World。
219 Editor GameView GPU texture sharing 主线。
220 GameView input / AUI consumed / gameplay fallback。
221 Pause / Resume / StepFrame / StopPlaySession。
222 GameView runtime pick + selected_entity_source + Runtime Inspector。
```

## 3. 涉及文件

预计修改：

```text
rust/crates/editor_ui_model/src/hierarchy.rs
rust/crates/editor_ui_model/src/inspector.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/services/scene_service.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/tests/runtime_service_tests.rs
rust/crates/project_e2e_gate/src/editor_gameview_play.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

如实际施工发现不需要修改某些文件，以最小实现为准。

## 4. Gate A：现状锁定

任务：

```text
确认当前施工目录只有本文件。
确认 223 正式方案已经生成。
锁定 HierarchyModel / InspectorModel / UiCommandPayload / EditorRuntimePlayInstance / StopPlaySession 基线。
确认不新增可见 domain toggle。
```

测试/命令：

```powershell
Get-ChildItem -LiteralPath "框架设计/引擎总体架构/施工文档/当前"
rg -n "struct HierarchyModel|struct InspectorModel|enum UiCommandPayload|fn build_hierarchy_model|struct EditorRuntimePlayInstance|fn stop_play_session" rust/crates
```

通过标准：

```text
施工目录只有本文件。
关键入口已确认。
```

## 5. Gate B：Hierarchy auto source / model metadata

任务：

```text
HierarchyModel 必须新增 source_domain / status 字段。
HierarchySourceDomain 至少包含:
  AuthoringScene
  ActiveGameViewRuntime
  OpenedRuntimePackage
  Empty

build_hierarchy_model 自动来源规则:
  active editor_runtime_play_instance 存在 -> active runtime World
  否则 editor_scene_document 存在 -> authoring Scene
  否则 opened RuntimePackage world 存在 -> package world
  否则 empty

Play Mode 中主 Hierarchy 显示 active runtime World，不同时承担 authoring Scene 编辑入口。
Stop Play 后自动恢复 authoring Scene。
```

测试/命令：

```powershell
cargo test -p editor_ui_model hierarchy -- --nocapture
cargo test -p editor_core runtime_service_tests -- --nocapture
```

通过标准：

```text
HierarchyModel 能被 AI / tests 判断 source_domain。
Play Mode 时 roots 来自 active runtime World。
UI 模型没有可见 domain toggle 命令。
```

## 6. Gate C：Runtime Hierarchy selection

任务：

```text
新增或明确 Runtime Hierarchy 选择 command:
  SelectRuntimeEntity { entity_id }
  或等价 command，必须把 selection_source 写为 ActiveGameViewRuntime。

该 command 必须:
  只在 active editor_runtime_play_instance 存在时成功。
  验证 entity 存在于 active runtime World。
  清理 selected_aui_node。
  让 Inspector 复用 222 的 active runtime selection 入口。
```

测试/命令：

```powershell
cargo test -p editor_core runtime_service_tests -- --nocapture
```

通过标准：

```text
点击 Runtime Hierarchy 节点后 selected_entity_source=active_game_view_runtime。
Inspector 指向同一个 runtime entity。
missing entity 返回结构化 diagnostic。
```

## 7. Gate D：Play Mode temporary inspector edit

任务：

```text
InspectorModel 必须新增 persistence 字段。
InspectorPersistence 至少包含:
  PersistentAuthoring
  TemporaryPlaySession
  ReadOnlyRuntimePackage
  ReadOnly

ActiveGameViewRuntime Inspector:
  readonly=false
  persistence=TemporaryPlaySession
  fields 根据 allowlist 设置 editable=true

新增 UiCommandPayload::SetRuntimeComponentFieldTemporary:
  entity_id
  component_type
  field_path
  value

新增 EditorRuntimePlayInstance::apply_temporary_component_edit(...) 窄接口。
禁止暴露通用 runtime_world_mut()。
默认不做 Vec2 字段；Collider2D.offset 不进入 C-min。
```

C-min allowlist：

```text
Transform:
  localPosition / localRotation / localScale 或 local_position / local_rotation / local_scale

Renderable:
  visible

SpriteRenderer2D:
  visible
  color
  sorting_layer
  order_in_layer
  sort_z

Collider2D:
  enabled
```

测试/命令：

```powershell
cargo test -p editor_core runtime_temporary -- --nocapture
cargo test -p editor_core runtime_service_tests -- --nocapture
```

通过标准：

```text
SetRuntimeComponentFieldTemporary 修改 active runtime World。
StepFrame 后修改仍存在于当前 Play Session。
不写 editor_scene_document。
不写 RuntimePackage。
unsupported field/type 返回 diagnostic。
```

## 8. Gate E：Stop Play discard summary

任务：

```text
新增 RuntimeTemporaryEditSummary 轻量状态:
  edited_entity_count
  edited_field_count
  last_edited_entity_id
  last_edited_field_path
  discard_policy=discard_on_stop_play

StopPlaySession:
  take/drop editor_runtime_play_instance
  clear runtime selection
  output runtime_temporary_edits_discarded diagnostic/summary
  不写文件、不 dirty、不 save
```

测试/命令：

```powershell
cargo test -p editor_core play_service_tests -- --nocapture
cargo test -p editor_core runtime_service_tests -- --nocapture
```

通过标准：

```text
StopPlaySession 后 runtime instance 为空。
runtime selection 清空。
discard summary 包含编辑计数。
重新 Play 后临时修改不保留。
```

## 9. Gate F：e2e / 回归 / 文档归档

任务：

```text
project_e2e_gate 覆盖复杂打飞机 Play Mode temporary edit。
运行整体回归。
生成阶段完成记录。
更新入口文档。
施工文档归档。
```

测试/命令：

```powershell
cargo fmt --check
cargo test -p editor_core
cargo test -p project_e2e_gate
cargo test -p editor_window_winit input_runtime_loop
```

文档同步：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-08-Editor-Runtime-Hierarchy-PlayMode-Temporary-Inspector-Edit-Productization-v1/
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
本施工文档没有新增可见 domain toggle。
没有实现 Apply Runtime Change To Authoring。
没有把 Play Mode temporary edit 写入 Scene / Prefab / RuntimePackage。
没有要求 runtime_world_mut() 通用可变暴露。
没有把 Vec2 / Collider2D.offset 放进默认 C-min。
没有新增 FullWorldStateDump / 统一 RuntimeStateSnapshot。
```

测试自审：

```text
每个 Gate 都有测试命令。
默认测试为 headless deterministic。
真实 OS window / GPU smoke 不作为必须 gate。
```

方案一致性自审：

```text
施工目标与 223 正式方案一致。
继承 222 的 selection_source 消歧，但把 active runtime Inspector 升级为 TemporaryPlaySession editable。
Opened RuntimePackage Inspector 仍保持只读。
```

结论：

```text
本施工文档可执行。
下一步按 Gate A -> F 施工。
```
