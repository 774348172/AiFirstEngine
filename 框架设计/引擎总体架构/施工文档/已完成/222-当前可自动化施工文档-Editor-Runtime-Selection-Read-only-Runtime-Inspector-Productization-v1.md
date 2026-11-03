# 222-当前可自动化施工文档-Editor Runtime Selection / Read-only Runtime Inspector Productization v1

## 1. 施工目标

本次施工落地 `222-Editor Runtime Selection / Read-only Runtime Inspector Productization v1` 的 C-min：

```text
Editor GameView Play / Pause / Step
  -> 点击运行时世界对象
  -> AUI consumed 时阻止 world pick
  -> AUI 未消费时执行 WorldPickCollector C-min
  -> 写入带 source 消歧的 runtime selection
  -> InspectorModel 从 active EditorRuntimePlayInstance 的 live World 构建
  -> Inspector 显示只读 Runtime Inspector
```

明确不做：

```text
不新增 EditorRuntimeStateSnapshot。
不新增统一 runtime report。
不做 runtime component 编辑或写回 Scene Document。
不做 Runtime Hierarchy、多选、GPU object-id picking、3D depth picking、透明像素 picking。
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
框架设计/引擎总体架构/222-Editor-Runtime-Selection-Read-only-Runtime-Inspector-Productization-v1方案.md
```

外部审查：

```text
其它AI审查目录/40-222-Editor-Runtime-Selection-Read-only-Runtime-Inspector方案审查.md
```

前置完成能力：

```text
217 Preview RuntimePackage cache
218 In-process Editor GameView runtime instance
219 Shared GPU texture GameView present
220 GameView input / AUI consumed / gameplay fallback
221 Pause / Resume / StepFrame / Stop / Maximize on Play
```

## 3. 涉及文件

预计修改：

```text
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/tests/...
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/inspector.rs 或 model/command 相关导出
rust/crates/project_e2e_gate/...
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

可能不修改：

```text
engine_runtime RenderFrameReportLevel 枚举不在 222-min 回填。
Report Panel 不新增统一 Runtime State Report。
```

## 4. 外部审查分类吸收

必须修改，已进入本施工：

```text
MF-1:
  EditorRuntimePlayInstance 必须新增 live World 只读 accessor。
  build_inspector_model 必须新增 active runtime live World 数据源。
  Runtime Inspector 数据源优先级必须高于 authoring Scene selection。

SF-2:
  runtime selection 必须带 source/domain 消歧。
  不允许 selected_entity_id 同时含义为 active GameView runtime 和 opened RuntimePackage world。

SF-3:
  222 自建 AUI-consumed-then-world-pick 判定。
  不依赖未落地的 full HitCandidate router。

SF-5:
  测试默认 headless deterministic。
  真实窗口/GPU smoke 只允许 optional，不阻塞默认 gate。

SF-1:
  session 字段名使用 editor_runtime_play_instance。

SF-6:
  222 同时收敛 220 WorldPickCollector deferred 与 221 runtime inspector deferred。
```

施工约束：

```text
Report 分档遵守项目 Off / Summary / Trace 规则。
不回填或重命名 RenderFrameReportLevel { Off, Stats, Summary, Evidence }。
Gate C 必须核实 SpriteRenderer2D / render order 字段；缺失时 deterministic fallback + diagnostic。
Gate B 必须落地 selection_source 或 SelectRuntimeEntity 二选一。
```

历史吸收：

```text
审查中提到 220 未施工的风险，以当前入口状态为准：220 已完成并归档。
222 继承 220 GameView-local coordinate transform 与 AUI consumed filter，不重复施工 220。
```

不进入 222：

```text
Godot / Bevy extra inspector 参考。
全项目 report-level convergence。
Runtime Hierarchy / debugger / replay / watch expression。
```

## 5. Gate A：现状锁定

任务：

```text
确认当前施工文档唯一。
读取并锁定 InspectorModel、EditorRuntimePlayInstance、EditorSession、build_inspector_model、World API 基线。
确认 222 只做 C-min，不扩展架构层。
```

测试/命令：

```powershell
Get-ChildItem -LiteralPath "框架设计/引擎总体架构/施工文档/当前"
rg -n "struct InspectorModel|fn build_inspector_model|struct EditorRuntimePlayInstance|selected_entity_id|editor_runtime_play_instance|fn transform|fn sprite_renderer2d|fn collider2d" rust/crates
```

通过标准：

```text
当前目录只有本施工文档。
关键代码入口已确认。
```

## 6. Gate B：runtime selection state + live World accessor

任务：

```text
给 EditorRuntimePlayInstance 增加只读 live World accessor：
  runtime_world() -> &World
  或 world() -> &World

前置消歧 runtime selection：
  优先复用 SelectEntity，但 EditorSession selection state 必须包含 source/domain。
  如复用会导致命令语义不清，则新增 SelectRuntimeEntity。

CommandResult / diagnostics 必须能说明：
  source=active_game_view_runtime
  selected_runtime_entity_id
  pick_status
```

测试/命令：

```powershell
cargo test -p editor_core runtime_selection
cargo test -p editor_core editor_runtime_play
```

通过标准：

```text
active runtime selection 与 opened package world selection 不再共用模糊语义。
测试能直接断言 source/domain。
EditorRuntimePlayInstance 的 World 可被只读读取，不暴露可变写入口。
```

## 7. Gate C：WorldPickCollector C-min

任务：

```text
实现小而真实的 WorldPickCollector C-min。
输入使用 GameView-local pointer 与 active runtime World。
AUI consumed 时返回 blocked_by_aui，不执行 world pick。
AUI 未 consumed 时执行 2D deterministic pick。
优先使用 Transform + Collider2D / SpriteRenderer2D bounds。
缺少可排序字段时使用 deterministic fallback，并输出 diagnostic: pick_order=fallback。
无法计算 bounds 时返回 unsupported，不假装命中。
```

测试/命令：

```powershell
cargo test -p editor_core world_pick
cargo test -p editor_core runtime_selection
```

通过标准：

```text
覆盖 hit / miss / blocked_by_aui / unsupported。
命中排序稳定。
没有依赖不存在的 HitCandidate router。
```

## 8. Gate D：InspectorModel active runtime 数据源

任务：

```text
调整 build_inspector_model 优先级：
  1. selected_aui_node
  2. active GameView runtime selection
  3. authoring Scene selection
  4. opened RuntimePackage world selection
  5. No Selection

Runtime Inspector 必须 readonly=true。
Runtime Inspector 从 editor_runtime_play_instance.runtime_world() 读取 live World。
opened RuntimePackage world Inspector 继续保留，不和 active runtime selection 混淆。
runtime entity 已不存在时显示明确 missing/No Selection diagnostic。
```

测试/命令：

```powershell
cargo test -p editor_core inspector_model
cargo test -p editor_core runtime_inspector
```

通过标准：

```text
active runtime selection 可盖过 authoring Scene selection。
InspectorModel title/selected_entity_id/sections/fields/readonly 可断言。
StepFrame 后重新构建 InspectorModel 读取 live World 当前值。
```

## 9. Gate E：Report / AI / e2e evidence

任务：

```text
优先使用 CommandResult、EditorSession state、InspectorModel 作为证据。
如现有 GameViewPresentReport 需要最小字段，只加轻量 summary：
  runtime_pick_status
  selected_runtime_entity_id
  runtime_inspector_status
不新增统一 RuntimeStateReport。
不默认输出完整 World dump。
补 project_e2e_gate，证明复杂打飞机 C-min 可暂停点选 runtime object 并读 Inspector。
```

测试/命令：

```powershell
cargo test -p editor_core runtime_inspector
cargo test -p project_e2e_gate
```

通过标准：

```text
AI 能从结构化结果判断选中来源、只读原因、miss/blocked/unsupported 原因。
project_e2e_gate 至少覆盖 complex shooter runtime inspect C-min。
```

## 10. Gate F：整体回归、文档同步、归档

任务：

```text
运行整体回归。
生成阶段完成记录。
更新入口文档。
将本施工文档从 当前/ 移到 已完成/。
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
框架设计/引擎总体架构/阶段完成记录/2026-07-08-Editor-Runtime-Selection-Read-only-Runtime-Inspector-Productization-v1/
```

通过标准：

```text
Gate A-F 全部完成。
默认 headless 测试通过。
施工文档已归档。
入口文档说明 222 已完成，并指向下一步。
```

## 11. 施工自审

范围自审：

```text
本施工文档没有把 222 扩展成 debugger / runtime hierarchy / runtime editing。
没有新增 EditorRuntimeStateSnapshot。
没有新增统一 runtime report。
没有要求修改 RenderFrameReportLevel。
没有把 AUI Node 改成 ECS Entity。
```

测试自审：

```text
每个 Gate 都有对应测试命令。
默认测试为 headless deterministic。
真实窗口 / GPU smoke 不作为必须 gate。
```

外部审查自审：

```text
MF-1 / SF-2 / SF-3 / SF-5 / SF-1 / SF-6 已进入 Gate B-D。
MF-3 被约束为“不回填 RenderFrameReportLevel，222 只遵守项目 report 分档”。
MF-2 按当前入口状态处理：220 已完成并归档，222 继承前置能力。
无需要继续修改正式方案后才能施工的阻塞项。
```

结论：

```text
本施工文档可执行。
下一步按 Gate A -> F 施工。
```
