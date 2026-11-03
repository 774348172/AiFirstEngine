# 230-当前可自动化施工文档-Project Rule Driven UiStateSnapshot Producer v1

> 状态：当前施工文档。  
> 方案来源：`框架设计/引擎总体架构/230-Project-Rule-Driven-UiStateSnapshot-Producer-v1方案.md`。  
> 外部审查：`其它AI审查目录/43-230-Project-Rule-Driven-UiStateSnapshot-Producer方案审查.md`。  
> 施工日期：2026-07-09。  
> 当前施工目录同一时间只能保留这一份施工文档。

## 1. 施工目标

把复杂打飞机 HUD 的 UI state 从 sample/伪数据 producer 升级为真实 runtime state driven：

```text
Project Rule / ECS World
  -> ComplexShooterRuntimeUiStateProducer
  -> ProjectUiStateSnapshot
  -> AUI Binding Resolve
  -> runtime_player_winit / editor_gameview_play / project_e2e_gate evidence
```

本轮必须证明：

```text
game.score_text 来自 project.sessionState.score，不再来自 frame_index。
player.hp_ratio 来自 entity-player/project.combatState.hp。
game.wave_text 来自 project.sessionState.wave。
producer_id = complex_shooter_runtime_ui_state。
active_binding_paths / cache_status / dirty_domains / source_paths 有结构化证据。
PackageSmokeSnapshot / ComplexShooterSampleUiStateProducer 不再伪装真实项目 UI state。
```

## 2. 审查吸收

读取审查文档后，方案已修改：

```text
MF-1 已吸收：
  ProjectUiStateProducerContext 保留已有 frame_index / package / world，
  只新增 active_binding_paths / report_mode。

MF-2 已吸收：
  ProjectUiStateSnapshotReport 保留已有 producer_id / snapshot_source / value_count /
  produced_paths / declared_binding_paths / missing_paths / type_mismatch_paths / diagnostics；
  只新增 active_binding_paths / dirty_domains / cache_status /
  cache_hit_paths / cache_miss_paths / source_paths。
```

施工约束：

```text
SC-1:
  active_binding_paths v1 使用 HUD AUI Document 全部 declared binding paths。
  不改 build_aui_present_output 外部签名。

SC-2:
  producer cache 属于当前 player loop / EditorRuntimePlayInstance，Stop 后 drop。

SC-3:
  v1 active = 单 HUD document 全 declared paths；多 screen 可见子树 deferred。

SC-4:
  v1 假设无 player respawn 或 maxHp 不变。

SC-5:
  v1 只做 fingerprint-based cache；dirty_domains 汇总 fingerprint 变化。

SC-6:
  旧 ComplexShooterSampleUiStateProducer 保留为 smoke / fixture；
  runtime/player/editor 切到 ComplexShooterRuntimeUiStateProducer。
```

审查中的历史闭环债：

```text
223-229 历史闭环不纳入本轮功能施工范围。
当前 README 已显示 229 完成记录存在，本轮只同步 230 自身入口和阶段记录。
```

## 3. 涉及文件

预计修改：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/project_e2e_gate/src/project_rule_driven_ui_state.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-09-Project-Rule-Driven-UiStateSnapshot-Producer-v1/00-总览.md
```

禁止修改：

```text
不把 score / hp / wave 写入 engine_runtime 通用 API。
不新增完整 ViewModel 资产系统。
不实现完整 dirty graph。
不做 P0-4 Windows golden gate。
不改 RuntimePackage 真相路径。
```

## 4. Gate A：扩展现有 producer/report contract

任务：

```text
在 ProjectUiStateProducerContext 现有 frame_index / package / world 基础上新增 active_binding_paths / report_mode。
在 ProjectUiStateSnapshotReport 现有字段基础上新增 active_binding_paths / dirty_domains / cache_status / cache_hit_paths / cache_miss_paths / source_paths。
保留 199 兼容路径。
```

测试：

```powershell
cargo test -p engine_runtime aui
```

## 5. Gate B：新增 ComplexShooterRuntimeUiStateProducer

任务：

```text
新增 ComplexShooterRuntimeUiStateProducer。
读取 project.sessionState.score / wave。
读取 entity-player/project.combatState.hp。
统计 alive enemy count。
按 active binding path 生成 snapshot。
用 fingerprint cache 复用未变化 path。
report 写 source_paths / cache_hit_paths / cache_miss_paths / dirty_domains。
保留 ComplexShooterSampleUiStateProducer 仅作 smoke / fixture。
```

测试：

```powershell
cargo test -p engine_runtime aui
```

## 6. Gate C：接入 runtime_player_winit 与 editor_gameview_play

任务：

```text
runtime_player_winit:
  player loop 持有 ComplexShooterRuntimeUiStateProducer。
  build_aui_present_output 接收 &mut producer。
  report 继续输出 producer_id / produced_paths / missing_paths。

editor_gameview_play:
  EditorRuntimePlayInstance 持有 ComplexShooterRuntimeUiStateProducer。
  Stop Play 后随 instance drop。
  GameView AUI present 使用真实 runtime producer。
```

测试：

```powershell
cargo test -p runtime_player_winit aui
```

## 7. Gate D：project_e2e_gate 真实 UI state gate

任务：

```text
新增 project_rule_driven_ui_state gate。
先按 229 路径构建 RuntimePackage、hydrate World、运行 ProjectLogicRunner 至 score_changed。
再调用 ComplexShooterRuntimeUiStateProducer 生成 snapshot。
断言 game.score_text 与 score_after 一致。
断言 producer_id、snapshot_source、active_binding_paths、cache_status、source_paths。
写结构化 JSON report。
```

测试：

```powershell
cargo test -p project_e2e_gate complex_shooter_project_rule_driven_ui_state_snapshot
cargo test -p project_e2e_gate complex_shooter_gameplay_rule_runtime
```

## 8. Gate E：整体回归

任务：

```text
跑格式化和本轮相关整体测试。
确认没有把 runtime 热路径变成 Trace 常驻。
确认 PackageSmokeSnapshot 仍只用于 smoke/test。
```

测试：

```powershell
cargo fmt --check
cargo test -p engine_runtime aui
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate complex_shooter_project_rule_driven_ui_state_snapshot
cargo test -p project_e2e_gate complex_shooter_gameplay_rule_runtime
cargo test -p project_e2e_gate
```

## 9. Gate F：文档同步与归档

任务：

```text
生成阶段完成记录：
  阶段完成记录/2026-07-09-Project-Rule-Driven-UiStateSnapshot-Producer-v1/00-总览.md

更新入口：
  49-当前状态与下一步入口.md
  54-当前可自动化施工总入口.md
  施工文档/README.md
  阶段完成记录/README.md

把本施工文档从 当前/ 移动到 已完成/。
```

## 10. 施工文档自审

```text
是否与 230 正式方案一致：
  是。采用 B-min + ActiveBindingDriven + CachedProducer + C-report。

是否吸收 43 审查：
  是。MF-1/MF-2 已要求修改方案；SC-1~SC-6 已进入 Gate。

是否扩大范围：
  否。不做完整 ViewModel、完整 dirty graph、自走棋复杂 UI、P0-4 golden gate。

是否保护架构边界：
  是。AUI Binding 仍只读 ProjectUiStateSnapshot；score/hp/wave 不进入 engine_runtime 通用 API。

是否有分 Gate 测试：
  是。每个代码 Gate 都有 cargo test 命令，最后有整体回归。

是否处理 runtime/editor report 分档：
  是。v1 只要求 Summary/Trace gate 证据，不把完整 Trace 常驻 runtime 热路径。

是否可自动化施工：
  是。涉及文件、任务和测试命令明确，当前/ 目录只有这一份施工文档。
```
