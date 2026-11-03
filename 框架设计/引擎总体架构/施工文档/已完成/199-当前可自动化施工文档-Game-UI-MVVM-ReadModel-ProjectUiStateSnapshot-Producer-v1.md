# 199-当前可自动化施工文档-Game UI MVVM ReadModel / ProjectUiStateSnapshot Producer v1

## 1. 施工目标

把 AUI HUD 的运行时数据源从 `PackageSmokeSnapshot` 推进到真实项目侧 `ProjectUiStateSnapshotProducer`。

本轮只做 v1 C-min：

```text
RuntimePackage loaded
  -> World hydrated
  -> ProjectUiStateSnapshotProducer
  -> ProjectUiStateSnapshotReport
  -> AUI Binding / Present
  -> runtime_player_winit report
  -> project_e2e_gate report
```

不做：

```text
AUI drag/drop
复杂装备 UI
Transaction RuleSlot
Project UI Hot Logic Module / WASM
runtime_text_glyph_present
```

## 2. 正式依据

```text
199-Game UI MVVM ReadModel / AUI ProjectUiStateSnapshot Producer v1 方案
190-AUI-RuntimePackage-Document-Hydration-Binding-Present-v1方案.md
195-Gameplay-Rule-Asset-Rust-Framework-IR-Redline-and-AUI-Logic-Boundary方案.md
```

当前没有其它 AI 对 199 的专项审查文档；施工前自审结论见本文第 8 节。

## 3. 涉及文件

预计修改：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/project_e2e_gate/src/report.rs
rust/crates/project_e2e_gate/src/gate.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/199-AUI-ProjectUiStateSnapshot-Producer-v1方案.md
```

完成后同步：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-05-Game-UI-MVVM-ReadModel-ProjectUiStateSnapshot-Producer-v1/00-总览.md
```

## 4. Gate A：Producer Contract / Report

任务：

```text
新增 ProjectUiStateSnapshotProducer trait。
新增 ProjectUiStateProducerContext / ProjectUiStateSnapshotOutput / ProjectUiStateSnapshotReport。
AuiSnapshotSource 增加 ProjectProducer。
报告包含 producer_id / value_count / produced_paths / declared_binding_paths / missing_paths / diagnostics。
```

测试：

```powershell
cargo test -p engine_runtime aui
```

## 5. Gate B：AUI Present 接入 ProjectProducer

任务：

```text
AuiRuntimePresenter 支持 present_project_snapshot / present_with_snapshot_output。
AuiRuntimePresentReport 携带 ui_state_snapshot_report。
禁止 ProjectProducer 失败时静默退回 PackageSmokeSnapshot。
```

测试：

```powershell
cargo test -p engine_runtime aui
cargo test -p runtime_player_winit aui
```

## 6. Gate C：复杂打飞机 C-min Producer

任务：

```text
runtime_player_winit 为复杂打飞机样例提供 sample producer。
至少生产：
  game.score_text
  player.hp_ratio
  game.paused
  player.ship_icon
producer report 标明 snapshot_source=project_producer。
```

测试：

```powershell
cargo test -p runtime_player_winit aui
```

## 7. Gate D：E2E 报告证据

任务：

```text
project_e2e_gate metrics 增加 ui_state_snapshot_source / producer_id / produced_path_count / missing_path_count。
aui-runtime-present step summary 显示 ProjectProducer。
测试断言复杂打飞机 e2e 不再把 PackageSmokeSnapshot 当真实项目状态。
```

测试：

```powershell
cargo test -p project_e2e_gate aui
```

整体回归：

```powershell
cargo fmt --check
cargo test -p engine_runtime aui
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate
```

## 8. 施工文档自审

结论：可施工。

自审：

```text
没有扩大到复杂装备 UI。
没有引入新的运行时架构层。
没有让 AUI Runtime 直接读 ECS。
没有让 IR 承担复杂 UI 聚合。
每个 Gate 都有测试命令。
完成后需要阶段记录与入口同步。
```

风险：

```text
v1 sample producer 只能证明 ProjectProducer 链路，不代表完整商业 UI 状态系统。
runtime_text_glyph_present 仍保留为后续缺口。
复杂自走棋装备 / 商店 UI 仍需后续 200 拆 Gate。
```
