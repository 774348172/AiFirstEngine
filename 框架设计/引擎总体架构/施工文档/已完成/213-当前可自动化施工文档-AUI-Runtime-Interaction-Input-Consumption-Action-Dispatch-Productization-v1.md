# 213-当前可自动化施工文档-AUI Runtime Interaction / Input Consumption / Action Dispatch Productization v1

## 1. 施工目标

本轮只施工 `213-AUI-Runtime-Interaction-Input-Consumption-Action-Dispatch-Productization-v1方案.md` 采用的 B-min+：

```text
AUI RuntimePackage document 已经能 present。
本轮把 AUI hit test / click / drag-drop C-min 接入真实 runtime input。
UI 消费的 pointer input 必须从 gameplay InputResolver 过滤掉。
Runtime / native player / e2e report 必须输出结构化 AUI interaction evidence。
```

本轮不做：

```text
scroll / focus navigation / IME / InputField / gamepad navigation / multi pointer drag
world-space UI raycast
复杂拖拽 ghost visual
项目装备交易执行
authoring SetAuiActionRef.payload 存储修复
Modal 全屏 input blocking / focus trap
editor Scene AUI hit-test 闭合
```

## 2. 依据文件

正式方案：

```text
框架设计/引擎总体架构/213-AUI-Runtime-Interaction-Input-Consumption-Action-Dispatch-Productization-v1方案.md
```

外部审查：

```text
其它AI审查目录/32-213-AUI-Runtime-Interaction-Input-Consumption-Action-Dispatch方案审查.md
```

相关历史规则：

```text
103-AUI-Interaction-System-C-min方案.md
195-Gameplay-Rule-Asset-Rust-Framework-IR-Redline-and-AUI-Logic-Boundary方案.md
199-AUI-ProjectUiStateSnapshot-Producer-v1方案.md
209-AUI-Scene-Unified-Authoring-Productization-v1方案.md
210-RuntimeRenderer-Multi-stage-UI-Composition-Pass-Productization-v1方案.md
211-AUI-Prefab-Template-Reuse-Productization-v1方案.md
```

## 3. 涉及代码文件

预计修改：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/engine_input/src/input_mapping.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
```

预计新增：

```text
rust/crates/project_e2e_gate/src/aui_runtime_interaction.rs
```

可能只读参考：

```text
rust/crates/editor_core/src/services/aui_service.rs
samples/complex_shooter_project/
```

## 4. Gate A：交互 schema 补齐

任务：

```text
在 AuiNode 显式新增 draggable: bool=false、drop_target: bool=false，serde default。
新增 AuiActionEvent::{DragStart, DragMove, Drop}。
新增 AuiCommandKind::{DragStart, DragMove, Drop, DragCancel}。
新增 AuiInteractionConfig 或等价 default，报告 drag_threshold_px=4。
扩展 AuiInteractionTrace，包含 captured_node、drop_target、action_count。
新增 AuiInteractionProductizationReport 或等价 report struct。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_interaction
```

## 5. Gate B：Drag/drop C-min core

任务：

```text
AuiInteractionSystem::process 支持 primary pointer drag state。
PointerDown on draggable node capture source_node。
PointerMove 超过 drag_threshold_px 生成 DragStart，之后生成 DragMove。
PointerUp 命中 drop_target 生成 Drop，否则生成 DragCancel。
DragStart 后不生成 Click。
非 primary pointer 不触发 drag。
payload 由 runtime 生成，仅包含 source_node / target_node / pointer / delta / phase。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_drag
cargo test -p engine_runtime aui_interaction
```

## 6. Gate C1：RuntimeInputFrame filter

任务：

```text
新增 RuntimeInputFrame::filter_consumed_pointer_events 或等价 helper。
采用 clone/rebuild filtered frame。
只移除被 AUI interaction 消费的 pointer event。
不移除 keyboard、mouse wheel、未消费 pointer。
```

测试：

```powershell
cd rust
cargo test -p engine_input aui_input_filter
```

## 7. Gate C2：runtime_player_winit 顺序调整

任务：

```text
把 runtime player frame loop 调整为：
  build AUI present/layout
  run AuiInteractionSystem::process
  filter consumed pointer events
  InputResolver::resolve(filtered_frame)
  EngineFrameInput + render

headless 和 real-window 路径都要使用同一 helper。
无 AUI document 时保持旧 input 行为。
present snapshot 接受 snapshot_frame_lag=1，并写入 report。
```

测试：

```powershell
cd rust
cargo test -p runtime_player_winit aui_interaction
```

## 8. Gate C3：EngineFrameInput / native report 字段位

任务：

```text
EngineFrameInput 增加 aui_interaction: Option<AuiInteractionResult>。
新增 with_aui_interaction builder。
NativeAuiPresentSummary 增加：
  aui_interaction_status
  aui_input_consumed
  aui_action_count
  aui_drop_count
  gameplay_input_filtered
  snapshot_frame_lag
  authoring_action_payload_deferred
  modal_input_blocking_deferred
  editor_hit_test_deferred_to_209
```

测试：

```powershell
cd rust
cargo test -p engine_runtime engine_host_loop
cargo test -p runtime_player_winit aui_interaction
```

## 9. Gate D：project_e2e_gate report

任务：

```text
新增 complex-shooter-aui-runtime-interaction-productization-report.json。
报告必须覆盖 click、drag drop、drag cancel、input filter、deferred flags。
新增 project_e2e_gate public runner 和测试。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate aui_interaction
```

## 10. Gate E：复杂打飞机 fixture 验证

任务：

```text
构造 click fixture：点击 UI button 生成 Click action，并过滤 mouse/left gameplay fire。
构造 drag/drop fixture：source_node -> target_node 生成 Drop action 和 payload。
构造 drag cancel fixture：source_node -> 空白区生成 DragCancel。
确认 payload 不包含 equipment_id / chess_piece_id / ECS pointer / renderer handle。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate
```

## 11. Gate F：文档同步与归档

任务：

```text
更新 00-文档地图.md、00-阅读顺序.md 如需新增当前完成摘要。
更新 49-当前状态与下一步入口.md。
更新 54-当前可自动化施工总入口.md。
更新 施工文档/README.md。
新增 阶段完成记录/2026-07-07-AUI-Runtime-Interaction-Input-Consumption-Action-Dispatch-Productization-v1/00-总览.md。
更新 阶段完成记录/README.md。
所有测试通过后，把本施工文档移动到 施工文档/已完成/。
```

测试：

```powershell
cd rust
cargo fmt --check
```

## 12. 完整回归

最终必须运行：

```powershell
cd rust
cargo fmt --check
cargo test -p engine_runtime aui
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate aui_interaction
cargo test -p project_e2e_gate
```

如果出现跨 crate 编译影响，追加：

```powershell
cd rust
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

## 13. 施工自审

范围自审：

```text
本施工文档只执行 213 B-min+，没有引入完整 EventSystem / Slate / Godot Control 层。
没有新增用户心智层；AUI Document 仍是 UI 结构真相。
drag/drop 状态机属于 Rust AUI Core，不进入 IR。
项目装备交易仍归 Project Module / Transaction RuleSlot。
```

审查采纳自审：

```text
已把 Gate C 拆为 C1/C2/C3。
已固定显式 draggable/drop_target 字段。
已固定 filter 克隆重建路径。
已要求 present/layout -> interaction -> filtered input -> InputResolver 顺序。
已要求 deferred flags：authoring_action_payload_deferred、modal_input_blocking_deferred、editor_hit_test_deferred_to_209。
已要求 snapshot_frame_lag=1。
已要求 primary pointer、drag/click 互斥、DragCancel fixture。
```

测试自审：

```text
每个 Gate 都有对应 cargo test。
最终回归包含 fmt、engine_runtime aui、runtime_player_winit aui、project_e2e_gate aui_interaction、project_e2e_gate。
不允许点击 UI 后 gameplay fire 仍触发却报告 success。
```
