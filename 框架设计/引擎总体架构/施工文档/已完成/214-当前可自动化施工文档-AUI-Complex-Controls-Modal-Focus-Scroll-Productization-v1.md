# 214-当前可自动化施工文档-AUI Complex Controls / Modal-Focus-Scroll Productization v1

## 1. 施工目标

本轮只施工 `214-AUI-Complex-Controls-Modal-Focus-Scroll-Productization-v1方案.md` 采用的 B-min：

```text
Modal 能阻挡弹窗外 pointer / wheel / key 泄漏。
Modal active 时焦点被限制在 topmost modal root subtree 内。
ScrollView / List 支持 wheel scroll、drag scroll 和 layout computed rect offset 证据。
Scene AUI hit-test closure 继续输出 SelectAuiNode / Inspector evidence。
runtime_player_winit / project_e2e_gate 输出结构化 complex controls report。
```

本轮不做：

```text
InputField / IME。
完整 keyboard / gamepad navigation。
nested scroll / inertia / elastic / virtualized list。
Mask / Clip / Scrollbar rendering。
control-specific AuiStyle 扩展。
Slider / Toggle binding target。
authoring action payload channel 修复。
full modal stack / transition animation。
项目装备交易执行逻辑。
```

## 2. 依据文件

正式方案：

```text
框架设计/引擎总体架构/214-AUI-Complex-Controls-Modal-Focus-Scroll-Productization-v1方案.md
```

外部审查：

```text
其它AI审查目录/33-214-AUI-Complex-Controls-Modal-Focus-Scroll方案审查.md
```

前置完成系统：

```text
213-AUI-Runtime-Interaction-Input-Consumption-Action-Dispatch-Productization-v1方案.md
209-AUI-Scene-Unified-Authoring-Productization-v1方案.md
210-RuntimeRenderer-Multi-stage-UI-Composition-Pass-Productization-v1方案.md
211-AUI-Prefab-Template-Reuse-Productization-v1方案.md
199-AUI-ProjectUiStateSnapshot-Producer-v1方案.md
195-Gameplay-Rule-Asset-Rust-Framework-IR-Redline-and-AUI-Logic-Boundary方案.md
```

## 3. 涉及文件

预计修改：

```text
rust/crates/engine_input/src/input_mapping.rs
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
```

预计新增：

```text
rust/crates/project_e2e_gate/src/aui_complex_controls.rs
框架设计/引擎总体架构/阶段完成记录/2026-07-07-AUI-Complex-Controls-Modal-Focus-Scroll-Productization-v1/00-总览.md
```

文档同步：

```text
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

## 4. Gate A：schema / state / filter / report 基础

任务：

```text
扩展 RuntimeInputFrame::filter_consumed_pointer_events 为 filter_consumed_events，并保留旧 helper 兼容调用。
filter_consumed_events 必须能过滤 pointer / MouseWheel / KeyDown / KeyUp / KeyHeld。
新增 AuiFocusState / AuiFocusReason。
扩展 AuiInteractionConfig：modal_blocks_pointer_outside、modal_blocks_wheel_outside、modal_blocks_keyboard、wheel_scroll_px_per_delta、drag_scroll_threshold_px。
扩展 AuiInteractionState：focus、active_modal_root、scroll_offsets、active_scroll_capture。
新增 AuiScrollState。
扩展 AuiCommandKind / AuiActionEvent：Focus / Blur / Cancel / Scroll。
扩展 AuiInteractionResult / AuiInteractionProductizationReport：consumed_event_count_by_kind、focus_change_count、scroll_offset_change_count、scroll_offset_applied、scroll_applied_node_count、clipped_node_count、deferred flags。
新增 key_event_info 或等价路径，KeyDown / KeyUp / KeyHeld 能进入 AUI interaction。
```

测试：

```powershell
cd rust
cargo test -p engine_input aui_input_filter
cargo test -p engine_runtime aui_focus
cargo test -p engine_runtime aui_scroll
```

## 5. Gate B：Modal blocking + focus trap

任务：

```text
按 AuiCanvas.composition_stage == Modal 找 topmost modal root_node。
Modal active 时设置 active_modal_root 和 focus_scope_root。
Modal 不 active 时清空 active_modal_root / focus_scope_root。
pointer 命中 Modal subtree 正常 dispatch；pointer 在 Modal 外 consumed=true 但不产生业务 action。
Modal active 时 wheel/key 按配置 consumed，不能泄漏到 gameplay InputResolver。
PointerDown 命中 interactable node 时设置 focused_node。
Tab / Shift+Tab 只在 focus_scope_root subtree 内循环。
Escape 生成 Cancel command/action，不直接关闭项目 UI。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_modal
cargo test -p engine_runtime aui_focus
```

## 6. Gate C：ScrollView / List C-min

任务：

```text
MouseWheel over ScrollView/List 更新 offset_y，consumed=true。
PointerDown on ScrollView/List 建立 scroll capture。
PointerMove 超过 drag_scroll_threshold_px 后执行 drag scroll，consumed=true。
PointerUp 释放 scroll capture。
offset_y clamp 到 0..max_offset_y。
layout 接收 scroll_offsets，ScrollView/List subtree 的 child computed rect y 必须应用 offset。
layout report 输出 scroll_offset_applied=true、scroll_applied_node_count、clipped_node_count。
不新增 renderer clip pass，不新增 Scrollbar visual。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_scroll
```

## 7. Gate D：runtime_player_winit summary 接线

任务：

```text
runtime loop 继续使用同一个 AuiInteractionState 跨帧保存 focus / scroll / capture。
MouseWheel / KeyDown / Pointer 经 AUI interaction 后再进入 filter_consumed_events。
NativeAuiPresentSummary 增加 modal/focus/scroll/consumed kind/layout offset evidence。
EngineFrameInput 保留 AUI interaction result，供 EngineHostLoop report 使用。
无 AUI document 时保持旧 input 行为。
```

测试：

```powershell
cd rust
cargo test -p runtime_player_winit aui
cargo test -p runtime_player_winit aui_interaction
```

## 8. Gate E：Scene AUI hit-test closure

任务：

```text
复用 209 的 AuiSceneAuthoringService / AuiSceneUnifiedAuthoringReport。
project_e2e_gate 中把 Scene AUI hit route evidence 聚合进 214 complex controls report。
不新增 AUI Designer，不新增 AUI 专用编辑模式。
```

测试：

```powershell
cd rust
cargo test -p editor_input aui_scene
cargo test -p editor_core aui_scene_authoring
```

## 9. Gate F：complex shooter e2e report

任务：

```text
新增 complex-shooter-aui-complex-controls-productization-report.json。
报告覆盖 modal blocking、focus trap、wheel scroll、drag scroll、layout offset、Scene AUI hit-test。
报告必须保留 control_style_deferred、slider_toggle_binding_target_deferred、authoring_action_payload_deferred。
新增 project_e2e_gate public runner 和测试。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate aui_complex_controls
cargo test -p project_e2e_gate
```

## 10. Gate G：文档同步与归档

任务：

```text
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
如当前入口需要补充 214 完成摘要，更新 00-文档地图.md / 00-阅读顺序.md。
新增阶段完成记录。
所有测试通过后，把本施工文档移动到 施工文档/已完成/。
```

测试：

```powershell
cd rust
cargo fmt --check
```

## 11. 完整回归

最终必须运行：

```powershell
cd rust
cargo fmt --check
cargo test -p engine_input aui_input_filter
cargo test -p engine_runtime aui
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate aui_complex_controls
cargo test -p project_e2e_gate
```

如果出现跨 crate 编译影响，追加：

```powershell
cd rust
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

## 12. 施工自审

范围自审：

```text
本施工文档只执行 214 B-min，没有引入完整 UGUI EventSystem / Slate Widget / CommonUI router。
没有新增用户心智层；AUI Document 仍是 UI 结构真相。
focus / scroll / modal state 属于 Rust AUI Runtime transient state，不写回 AUI Document。
IR 不处理 UI state machine。
```

33 号审查采纳自审：

```text
已把 pointer-only filter 风险纳入 Gate A，要求 filter_consumed_events 支持 wheel/key。
已把 key_event_info 纳入 Gate A/B，Tab/Escape 不走 gameplay input resolver 私处理。
已把 Scroll offset 固定为 layout computed rect 路径，Gate C 失败不能用 diagnostic fallback 冒充成功。
已明确 AuiStyle 专属控件样式、Slider/Toggle binding target、authoring action payload channel deferred 并进入 report。
已修正 Button 单节点 + label 心智，不把 Button 错写成 subtree。
```

测试自审：

```text
每个 Gate 都有对应 cargo test。
最终回归包含 fmt、engine_input、engine_runtime aui、runtime_player_winit aui、project_e2e_gate aui_complex_controls、project_e2e_gate。
不允许 Modal active 时 wheel/key 泄漏到 gameplay 却报告 success。
不允许 ScrollView/List 只改 offset_y 不改 computed rect 却报告 success。
```
