# 89-当前可自动化施工文档：真实可编辑小项目闭环 C-min

## 目标

按照 `89-真实可编辑小项目闭环-C-min方案.md`，把当前已经完成的编辑器、Scene Editing、Play Session、Runtime、Console、Trace 能力串成一个可自动化验证的小项目编辑闭环。

目标链路：

```text
EditorSession
  -> 打开默认 Scene
  -> Hierarchy 显示 Entity
  -> 选择 Entity
  -> Inspector 显示 Transform
  -> Inspector 修改 Transform
  -> Scene dirty
  -> SaveScene
  -> Undo / Redo
  -> Play 当前 Scene
  -> Console / RuntimeTrace / EditableProjectLoopReport
```

本轮不是做完整生产级编辑器，而是验证 Unity / UE / Godot 式核心编辑工作流是否能在我们的长期架构上跑通。

## 已有基础

必须复用：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_host/src/main.rs
rust/crates/editor_window_winit/src/lib.rs
rust/crates/engine_runtime/src/default_game_run.rs
```

已有规则：

```text
EditorSceneDocument 是编辑真相。
Runtime World 不是编辑真相。
UI / AI / Test 不直接写 Scene 文件。
UI 行为必须先转 UiCommandPayload。
EditorSession 是 Editor Core 的统一入口。
Scene 修改必须走 SceneEditTransaction。
Save 必须走 SceneSavePipeline。
Play / Run 必须走 PlaySessionController。
```

## 明确不做

本轮不实现：

```text
完整 ProjectDock 文件浏览。
真实资源拖拽到 Scene。
真实 Viewport picking。
真实 Gizmo。
完整 Inspector 控件体系。
Prefab Mode。
多选批量编辑。
复杂 Dock layout。
复杂菜单 / 快捷键。
真实字体和完整 UI renderer。
真实窗口 smoke 作为默认测试。
```

## A1：新增 EditableProjectLoopReport

优先修改：

```text
rust/crates/editor_core/src/lib.rs
```

或如结构变大，可新增：

```text
rust/crates/editor_core/src/editable_project_loop.rs
```

新增结构：

```text
EditableProjectLoopReport
EditableProjectLoopDiagnostic
EditableProjectLoopDiagnosticSeverity
```

最小字段：

```text
schema_version
project_id
scene_id
opened_scene
selected_entity_id
inspector_entity_id
dirty_before_save
dirty_after_save
transform_edit_applied
undo_applied
redo_applied
play_started
play_finished
console_entry_count
runtime_trace_entry_count
diagnostics
```

规则：

```text
Report 是 AI / 测试验证入口。
Report 不保存完整 Scene dump。
Report 只保存 id、状态、计数、diagnostics。
如需详细定位，关联 CommandResult / SceneEditTransactionReport / PlaySessionReport。
```

测试：

```text
editable_project_loop_report_serializes
editable_project_loop_report_records_diagnostics
```

运行：

```powershell
cargo test -p editor_core editable_project_loop_report
```

## A2：新增默认小项目 / 默认 Scene fixture helper

优先修改：

```text
rust/crates/editor_core/src/lib.rs
```

或新增：

```text
rust/crates/editor_core/src/editable_project_loop.rs
```

实现 helper：

```text
create_default_editable_scene_fixture()
open_default_editable_scene_for_test()
```

第一版默认 Scene 至少包含：

```text
scene_id: scene-main
entity-player
Transform
Renderable 或最小 preview renderable 数据
```

规则：

```text
默认小项目只是测试 / C-min fixture。
不能把默认小项目写死成唯一项目模型。
打开默认 Scene 必须走 EditorSession / OpenSceneDocument 或同等主路径。
```

测试：

```text
editable_project_loop_opens_default_scene
editable_project_loop_default_scene_has_player_entity
```

运行：

```powershell
cargo test -p editor_core editable_project_loop_opens_default_scene
```

## A3：打通 open scene -> hierarchy -> inspector

修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_ui_model/src/lib.rs
```

目标：

```text
EditorSession 打开默认 Scene 后：
  hierarchy.scene_id = scene-main
  hierarchy.roots 包含 entity-player
  选中 entity-player 后 inspector.selected_entity_id = entity-player
  inspector.title = Player 或对应 Entity name
  inspector sections 至少包含 Transform 信息
```

测试：

```text
editable_project_loop_selects_entity_and_updates_inspector
```

运行：

```powershell
cargo test -p editor_core editable_project_loop_selects_entity
```

## A4：打通 inspector transform edit -> transaction -> dirty -> preview sync

修改：

```text
rust/crates/editor_input/src/lib.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/scene_editing.rs
```

目标：

```text
Inspector 修改 localPosition
  -> UiCommandPayload::SetSceneTransform
  -> EditorSession::execute_command
  -> SceneEditCommand::SetTransform
  -> SceneEditTransaction
  -> EditorSceneDocument 更新
  -> dirty = true
  -> PreviewWorldSync 更新
  -> EditorUiModel.viewport.renderables 更新
```

规则：

```text
Inspector 不直接写 Transform。
测试不绕过 EditorSession 主路径。
失败必须进入 CommandResult diagnostics / Console。
```

测试：

```text
editable_project_loop_edits_transform_and_marks_dirty
editable_project_loop_transform_edit_refreshes_viewport_model
```

运行：

```powershell
cargo test -p editor_core editable_project_loop_edits_transform
cargo test -p editor_input inspector_set_scene_transform
```

## A5：打通 save -> dirty clear

修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/scene_editing.rs
```

目标：

```text
Transform 修改后 dirty = true。
SaveSceneDocument 走 SceneSavePipeline。
保存成功后 dirty = false。
EditableProjectLoopReport 记录 dirty_before_save / dirty_after_save。
```

测试：

```text
editable_project_loop_save_clears_dirty
editable_project_loop_save_uses_scene_save_pipeline
```

运行：

```powershell
cargo test -p editor_core editable_project_loop_save
```

## A6：打通 undo / redo

修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/scene_editing.rs
```

目标：

```text
Transform 修改后 Undo 恢复原值。
Redo 恢复修改值。
Undo / Redo 仍刷新 EditorUiModel / PreviewWorld。
EditableProjectLoopReport 记录 undo_applied / redo_applied。
```

测试：

```text
editable_project_loop_undo_redo_transform
```

运行：

```powershell
cargo test -p editor_core editable_project_loop_undo_redo
```

## A7：打通 play current scene -> PlaySessionReport

修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/engine_runtime/src/default_game_run.rs
```

目标：

```text
当前 Scene 已打开时，UiCommandPayload::Play 能进入 PlaySessionController。
第一版可以走 headless gate。
PlaySessionReport / Console / RuntimeTrace 输出最小反馈。
EditableProjectLoopReport 记录 play_started / play_finished。
```

规则：

```text
PlaySession 属于 Editor 侧。
Runtime 不知道编辑器按钮。
Play 不直接读取 UI 临时状态。
第一版不要求真实窗口 Play 成熟可用。
```

测试：

```text
editable_project_loop_play_current_scene_reports_result
```

运行：

```powershell
cargo test -p editor_core editable_project_loop_play
cargo test -p editor_core play_session
```

## A8：端到端 headless gate

修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_host/src/main.rs
```

实现一个纯 headless 测试闭环：

```text
open default scene
select entity-player
edit localPosition.x
assert dirty
save scene
assert dirty cleared
undo
redo
play
build EditableProjectLoopReport
serialize report
```

测试：

```text
editable_project_loop_end_to_end_headless
editable_project_loop_report_is_ai_readable
```

运行：

```powershell
cargo test -p editor_core editable_project_loop
cargo test -p editor_host editable_project_loop
```

## A9：验证命令

施工完成必须运行：

```powershell
cargo fmt --check
cargo test -p editor_core editable_project_loop
cargo test -p editor_core scene_edit
cargo test -p editor_core play_session
cargo test -p editor_input inspector_set_scene_transform
cargo test -p editor_host editable_project_loop
cargo test -p editor_core
cargo test -p editor_host
cargo test --workspace
```

如果某个 filter 无匹配测试，必须说明是否因为当前 crate 没有对应测试名，而不是直接当作失败。

## A10：完成后文档同步

完成后必须：

```text
1. 将本施工文档移动到 施工文档/已完成。
2. 新增 阶段完成记录/2026-06-28-真实可编辑小项目闭环-C-min/00-总览.md。
3. 更新 89-真实可编辑小项目闭环-C-min方案.md，追加实施完成记录。
4. 更新 87-当前实现状态总览-2026-06-28.md。
5. 更新 49-当前状态与下一步入口.md。
6. 更新 54-当前可自动化施工总入口.md。
7. 更新 18-长期主义实现路径.md。
8. 更新 00-文档地图.md。
9. 更新 README.md。
10. 更新 施工文档/README.md。
11. 更新 阶段完成记录/README.md。
```

## 完成判定

满足以下条件才算完成：

```text
EditableProjectLoopReport 已实现并可序列化。
默认小 Scene 可通过 EditorSession 打开。
Hierarchy / Inspector 能反映选中 Entity。
Transform 修改通过 UiCommand -> EditorSession -> SceneEditTransaction 主路径完成。
Scene dirty / save / dirty clear 可验证。
Undo / Redo 可验证。
Play 当前 Scene 可产生 PlaySessionReport 或明确 diagnostic。
Console / RuntimeTrace 有最小反馈。
端到端 headless test 通过。
cargo test --workspace 通过。
```

## 明确不算完成的情况

```text
只新增方案文档，没有代码实现。
测试直接修改 EditorSceneDocument，绕过 EditorSession。
Inspector 持有自己的 Transform 副本。
Play 直接读取 UI 临时状态。
Save 绕过 SceneSavePipeline。
失败只 panic 或 stderr，没有进入 diagnostic / report。
默认 workspace 测试依赖真实窗口。
```
