# 215-当前可自动化施工文档-AUI RectClip / Scrollbar / Keyboard Navigation Productization v1

## 1. 施工目标

本轮只施工 `215-AUI-RectClip-Scrollbar-Keyboard-Navigation-Productization-v1方案.md` 采用的 B-min：

```text
ScrollView / List 子树获得矩形裁剪 effective_clip_rect。
Overlay draw item 携带 clip evidence，完全不可见 item 被 cull 并进入 report。
Hit-test 尊重 clip，clip 外节点不能被点中。
Vertical scrollbar 可见，并支持 thumb drag 修改 scroll offset。
Tab / Shift+Tab / ArrowUp / ArrowDown 移动 focus。
焦点移动到不可见列表项时自动滚入 ScrollView viewport。
runtime_player_winit / project_e2e_gate 输出结构化 215 productization report。
```

本轮不做：

```text
Stencil Mask / Soft Mask / Rounded Clip。
Horizontal scrollbar。
Nested scroll arbitration。
Inertia / elastic。
Virtualized list。
InputField / IME。
Rich text。
完整 gamepad navigation。
Submit action / screen flow。
Scrollbar template / theme editor。
真实 GPU screenshot pixel clip gate。
```

## 2. 依据文件

正式方案：

```text
框架设计/引擎总体架构/215-AUI-RectClip-Scrollbar-Keyboard-Navigation-Productization-v1方案.md
```

外部审查：

```text
未发现 其它AI审查目录/34-215-AUI-RectClip-Scrollbar-Keyboard-Navigation方案审查.md。
本施工文档自审结论：当前按正式方案直接施工，无其它 AI 审查修正项。
```

前置完成系统：

```text
214-AUI-Complex-Controls-Modal-Focus-Scroll-Productization-v1方案.md
213-AUI-Runtime-Interaction-Input-Consumption-Action-Dispatch-Productization-v1方案.md
211-AUI-Prefab-Template-Reuse-Productization-v1方案.md
210-RuntimeRenderer-Multi-stage-UI-Composition-Pass-Productization-v1方案.md
209-AUI-Scene-Unified-Authoring-Productization-v1方案.md
199-AUI-ProjectUiStateSnapshot-Producer-v1方案.md
195-Gameplay-Rule-Asset-Rust-Framework-IR-Redline-and-AUI-Logic-Boundary方案.md
```

## 3. 涉及文件

预计修改：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

预计新增：

```text
rust/crates/project_e2e_gate/src/aui_rectclip_scrollbar_navigation.rs
框架设计/引擎总体架构/阶段完成记录/2026-07-07-AUI-RectClip-Scrollbar-Keyboard-Navigation-Productization-v1/00-总览.md
```

## 4. Gate A：schema / report / clip 基础

任务：

```text
新增 AuiClipPolicy / AuiScrollbarPolicy / AuiScrollbarAxis / AuiScrollbarMetrics。
新增 AuiNavigationMode / AuiNavigationRef。
扩展 AuiNode：clip_policy / scrollbar_policy / navigation，serde default 兼容旧文档。
扩展 AuiComputedNode：effective_clip_rect / clipped_by_node。
扩展 AuiOverlayDrawItem：effective_clip_rect。
新增 AuiRectClipScrollbarNavigationProductizationReport 或等价结构化 report。
report 保留 stencil_mask_deferred、nested_scroll_deferred、inertia_elastic_deferred、virtualized_list_deferred、input_field_ime_deferred、full_gamepad_navigation_deferred。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_clip
```

## 5. Gate B：layout effective clip / draw cull

任务：

```text
ScrollView / List 默认作为 Rect clip root。
显式 clip_policy=Rect 的节点作为 Rect clip root。
layout 计算每个 computed node 的 effective_clip_rect 和 clipped_by_node。
draw extraction 携带 effective_clip_rect。
完全在 clip rect 外的 item 被 cull，并计入 culled_draw_item_count。
AuiLayoutReport / AuiRenderReport 输出 clip_root_count、effective_clip_item_count、culled_draw_item_count。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_clip
cargo test -p engine_runtime aui
```

## 6. Gate C：hit-test clip rejection

任务：

```text
hit-test 跳过 effective_clip_rect 外的节点。
pointer 在 viewport 内但命中被裁剪子节点时不能产生 action。
interaction trace / report 输出 hit_test_clip_rejected_count。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_hit_test
```

## 7. Gate D：vertical scrollbar visual + thumb drag

任务：

```text
为 ScrollView / List 生成 vertical AuiScrollbarMetrics。
content_height <= viewport_height 且 policy=Auto 时 scrollbar invisible。
生成 scrollbar track / thumb overlay draw item，带 source scroll_node_id。
thumb hit-test 优先于 content drag。
thumb drag 更新 AuiScrollState.offset_y。
report 输出 scrollbar_visible_count、scrollbar_thumb_drag_count、scrollbar_offset_change_count。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_scrollbar
```

## 8. Gate E：keyboard navigation + focus visible auto-scroll

任务：

```text
Tab / Shift+Tab 按 tree order 移动 focus。
ArrowUp / ArrowDown 按 computed rect 垂直方向选择 focus 候选。
ArrowLeft / ArrowRight 支持基础候选或保留 schema/report 证据。
Explicit navigation 优先使用 up/down/left/right/next/previous。
Modal active 时 navigation scope 仍限制在 focus_scope_root subtree 内。
focused_node 在 ScrollView/List subtree 内且不可见时，自动调整 scroll offset 使其可见。
report 输出 keyboard_navigation_event_count、focus_move_count、focus_visible_scroll_count。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_navigation
```

## 9. Gate F：runtime_player_winit / project_e2e_gate

任务：

```text
NativeAuiPresentSummary 输出 clip / scrollbar / navigation evidence。
project_e2e_gate 新增 complex-shooter-aui-rectclip-scrollbar-navigation-productization-report.json。
报告覆盖 clip cull、hit-test clip rejection、scrollbar visible、thumb drag、keyboard navigation、focus visible auto-scroll。
```

测试：

```powershell
cd rust
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate aui_rectclip_scrollbar_navigation
cargo test -p project_e2e_gate
```

## 10. Gate G：文档同步与归档

任务：

```text
生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
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
cargo test -p engine_runtime aui_clip
cargo test -p engine_runtime aui_hit_test
cargo test -p engine_runtime aui_scrollbar
cargo test -p engine_runtime aui_navigation
cargo test -p engine_runtime aui
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate aui_rectclip_scrollbar_navigation
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
本施工文档只执行 215 B-min，不引入完整 UGUI EventSystem / Slate Widget / Godot Control。
没有新增用户心智层；AUI Document 仍是 UI 结构真相。
focus / scroll / scrollbar drag state 属于 Rust AUI Runtime transient state，不写回 AUI Document。
IR 不处理 clip、scrollbar、navigation、focus-visible auto-scroll。
```

方案一致性自审：

```text
RectClip 必须进入 overlay/report 证据，不能只保留 clipped_node_count。
Scrollbar 必须能通过 thumb drag 修改 scroll offset，不能只画矩形。
Keyboard navigation 必须改变 focused_node，focus visible auto-scroll 必须改变 scroll offset。
Hit-test 必须尊重 clip。
Renderer 第一版可以只携带 clip evidence，不要求真实 GPU scissor screenshot。
```

测试自审：

```text
每个 Gate 都有对应 cargo test。
最终回归包含 fmt、engine_runtime aui_clip / aui_hit_test / aui_scrollbar / aui_navigation / aui、runtime_player_winit aui、project_e2e_gate 专项与全量。
```
