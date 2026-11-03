# 209-当前可自动化施工文档-AUI Scene Unified Authoring Productization v1

## 1. 施工目标

本轮施工只实现 `209-AUI-Scene-Unified-Authoring-Productization-v1方案.md` 中已经确认的：

```text
方案 C-min-r1：Truthful Scene Unified AUI Authoring
```

目标是让 AUI 进入 Scene 统一 authoring 链路：

```text
Scene View / Hierarchy / Inspector
  -> AuiNodeAuthoringProxy
  -> WorkspaceSelectionTarget::AuiNode
  -> AUI command / AuiAuthoringService
  -> canonical AuiDocument
  -> existing RuntimePackage / Present chain
```

本轮必须诚实保留边界：

```text
不新增独立 AUI Designer。
不新增 AUI 专用编辑模式；2D 只表示 Scene View projection。
AUI Document 仍是 UI 结构真相。
AUI Node 不变成 Runtime ECS Entity。
AuiNodeAuthoringProxy / SceneVisualOrderAuthoringEntry 只存在于编辑器 view model / report。
当前 runtime 只支持 ScreenOverlay / Modal overlay 域，不假装支持任意 World 前后夹击。
单个 AUI Node 拖到 Scene Entity 前后必须拒绝，并提示 Extract To AUI LayerGroup / Canvas。
```

## 2. 输入依据

正式方案：

```text
框架设计/引擎总体架构/209-AUI-Scene-Unified-Authoring-Productization-v1方案.md
```

外部审查：

```text
其它AI审查目录/28-209-AUI-Scene-Unified-Authoring方案审查.md
```

审查采纳结论：

```text
已采纳 Gate D0：Scene Entity 空间拾取前置。
已采纳 Gate C0 / C1：Hierarchy visual order foundation 与 selection/reorder integration 分拆。
已采纳 VisualOrderKey C-min-r1 只暴露 ScreenOverlay / Modal 可执行域。
已采纳 AuiComputedNode.rect: AuiComputedRect 命名，不使用 computed_rect。
已采纳单个 AUI Node 跨 World 拖拽拒绝，不自动包裹 LayerGroup。
已采纳 AuthoringAiContext runtime_supported / runtime_composition_gap_count / next_required_runtime_gate 真相位。
已采纳 hit-test 注册序与 rebuild 条件必须进入测试和报告。
```

## 3. 涉及文件

预计主要修改：

```text
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_ui_model/src/hierarchy.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/authoring_workflow.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_ui_model/src/lib.rs

rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/authoring_workspace.rs
rust/crates/editor_core/src/authoring_workflow.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/aui_authoring.rs
rust/crates/editor_core/src/services/aui_service.rs

rust/crates/editor_input/src/lib.rs

rust/crates/editor_ui_renderer/src/draw_list.rs
rust/crates/editor_ui_renderer/src/hit_test.rs
rust/crates/editor_ui_renderer/src/panels/hierarchy.rs
rust/crates/editor_ui_renderer/src/panels/workspace.rs
rust/crates/editor_ui_renderer/src/renderer.rs

rust/crates/project_e2e_gate/src/*
rust/crates/engine_runtime/src/aui.rs
```

仅在需要验证 RuntimePackage / present 不回退时读取或测试：

```text
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/runtime_player_winit/src/*
samples/complex_shooter_project
```

文档同步阶段涉及：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-06-AUI-Scene-Unified-Authoring-Productization-v1/00-总览.md
```

## 4. Gate A：Model / Selection Schema

任务：

```text
新增 AuiNodeAuthoringProxy。
新增 SceneVisualOrderAuthoringEntry / SceneVisualOrderAuthoringModel。
新增 VisualOrderKey / VisualOrderIntent / reorder status 类型。
新增 WorkspaceSelectionTarget::AuiNode { document_path, document_id, node_id }。
新增 AuiSceneUnifiedAuthoringReport schema。
VisualOrderKey render_space 只允许 ScreenOverlay / Modal。
Report 包含 visual_order_runtime_supported / visual_order_runtime_support_reason / deferred_to_runtime_composition_gate。
```

测试命令：

```powershell
cargo test -p editor_ui_model aui_scene
cargo test -p editor_core aui_scene
```

验收：

```text
模型能序列化 / clone / debug。
Selection summary 能表达 AuiNode。
Report 能表达跨 World 排序未被 runtime 支持。
不向 RuntimePackage / Scene / ECS 写入 proxy 或 visual order entry。
```

## 5. Gate B：Scene View AUI Overlay Model

任务：

```text
从 AUI Document + AuiLayoutResult 生成 AuiNodeAuthoringProxy。
source_rect 使用 AuiNode.rect。
rect 使用 AuiComputedNode.rect: AuiComputedRect。
支持 Perspective / Orthographic2D 视图模式 metadata。
输出 proxy_count / selectable_proxy_count / diagnostics。
```

测试命令：

```powershell
cargo test -p editor_core aui_scene
cargo test -p engine_runtime aui
```

验收：

```text
proxy 来自真实 AUI layout，不伪造空 document_path / node_id。
proxy 只服务 editor view model / report。
existing AUI runtime layout / present 测试不回退。
```

## 6. Gate C0：Hierarchy Visual Order Model Foundation

任务：

```text
建立 SceneVisualOrderAuthoringModel 构建入口。
Scene Entity / AUI Canvas / AUI LayerGroup / AUI Node 都能生成稳定 entry。
Hierarchy 默认数据源可切换到 visual order authoring model。
底层容器迭代序不得作为默认 authoring 顺序。
工程分桶只允许作为调试视图，不作为默认 authoring 视图。
```

测试命令：

```powershell
cargo test -p editor_ui_model hierarchy_visual_order
cargo test -p editor_core hierarchy_visual_order
```

验收：

```text
entry_id / parent_entry_id / sort key 稳定。
AUI Canvas / LayerGroup 是跨域排序最小安全 UI 粒度。
AUI Node 只允许在同一 AUI Canvas / LayerGroup 内参与 sibling order。
```

## 7. Gate C1：Hierarchy Selection / Reorder Integration

任务：

```text
Hierarchy 点击 AUI entry 可设置 WorkspaceSelectionTarget::AuiNode。
UI 域内拖拽排序生成 VisualOrderKey / AUI sibling order 修改意图。
AUI Canvas / LayerGroup 拖到 Scene Entity 前后只记录 VisualOrderIntent，并标 runtime_supported=false。
单个 AUI Node 拖到 Scene Entity 前后必须拒绝。
拒绝时输出结构化 hint：extract_to_aui_layer_group_or_canvas。
Report 输出 reorder_supported / last_reorder_status / selected_visual_order_key / selected_visual_order_intent。
```

测试命令：

```powershell
cargo test -p editor_ui_renderer hierarchy
cargo test -p editor_core authoring_workflow
```

验收：

```text
不自动插入 LayerGroup。
不隐式重写 AUI Document 层级。
跨 World authoring intent 必须包含 deferred_to_runtime_composition_gate。
```

## 8. Gate D0：Scene Entity 空间拾取前置

任务：

```text
补齐 Scene View 坐标到 Scene Entity 的 authoring pick report。
Viewport pointer 不再只能表示 focus viewport。
Scene pick report 输出 pointer、candidate_count、selected_entity_id、diagnostics。
```

测试命令：

```powershell
cargo test -p editor_input scene_pick
cargo test -p editor_core scene_pick
```

验收：

```text
没有真实空间拾取数据时必须诚实 report unavailable / no_candidates。
不能伪造 selected_entity_id。
该 Gate 只建立前置链路，不要求完整 3D raycast。
```

## 9. Gate D：Scene View Hit-Test Integration

任务：

```text
Scene View hit-test 能选择 AUI node proxy。
Hit-test 与 Hierarchy visual order 使用同一 visual order source。
未命中 AUI 时保留 Scene entity pick path。
注册序固定为 gizmo / active handle -> AUI proxy -> Scene entity -> empty scene。
AUI overlay hit region rebuild 条件明确：document/layout/viewport/projection/selection/visibility/order 变化。
```

测试命令：

```powershell
cargo test -p editor_ui_renderer aui_scene
cargo test -p editor_input aui_scene
```

验收：

```text
hit_test_status 能说明 hit_target_kind / document_path / node_id / entity_id / selection_changed。
命中顺序不依赖偶然 vec append 顺序。
```

## 10. Gate E：Inspector Command Roundtrip

任务：

```text
选中 AuiNode 后 Inspector 显示 AUI 字段。
支持 C-min 字段：document_path / node_id / name / kind / text / visible / interactable / consumeInput / rect / style / binding / action。
字段编辑走现有 UiCommandPayload / EditorSession AUI service / AuiAuthoringService。
保存后刷新 proxy / Inspector / report。
```

测试命令：

```powershell
cargo test -p editor_core aui_authoring
cargo test -p editor_core aui_scene
```

验收：

```text
Inspector 不直接 fs::write AUI JSON。
Inspector 不直接改 runtime World。
Inspector 不直接改 renderer draw command。
```

## 11. Gate F：Manual Walkthrough / AI Context / E2E

任务：

```text
ManualWalkthrough 能报告 AUI Scene unified authoring。
AuthoringAiContext 能说明 AUI 走 Scene 统一编辑链路。
AuthoringAiContext 暴露 visual_order_runtime_supported。
AuthoringAiContext 暴露 visual_order_runtime_support_reason。
AuthoringAiContext 暴露 runtime_composition_gap_count。
AuthoringAiContext 暴露 next_required_runtime_gate。
project_e2e_gate 生成 complex-shooter-aui-scene-unified-authoring-report.json。
```

测试命令：

```powershell
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core manual_walkthrough
cargo test -p project_e2e_gate aui_scene
```

验收：

```text
complex shooter report 不再把 runtime_text_glyph_present 当作 blocking gap。
跨 World visual order 未兑现时必须指向 RuntimeRenderer Multi-stage UI Composition Pass。
AUI Prefab / Template Reuse 作为后续 next_action 出现，不混入本轮施工。
```

## 12. Gate G：整体回归与文档同步

任务：

```text
AUI runtime / package / authoring / project patch 既有能力不回退。
格式化检查通过。
生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
将本施工文档从 当前 移动到 已完成。
```

整体回归命令：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_core
cargo test -p engine_runtime aui
cargo test -p project_e2e_gate
```

验收：

```text
所有 Gate 测试和整体回归通过，或在完成记录中明确记录非本轮阻塞的已知 local-only/ignored 测试。
当前施工目录恢复为空。
54 中“当前可施工文档”恢复为“无”。
```

## 13. 本轮明确不做

```text
独立 AUI Designer。
AUI 专用编辑模式。
拖拽创建控件。
拖拽移动 / resize。
Rect handle。
复杂对齐辅助线。
World-space UI。
任意 AUI Node 与 Scene Entity 直接混排的 runtime 渲染。
RuntimeRenderer 多段 UI composition pass。
拖拽单个 AUI Node 到 Scene Entity 前后时自动生成 LayerGroup。
AUI Prefab / Template Reuse。
ScrollView / InputField / IME。
UI animation / transition。
Theme editor。
真实 GPU screenshot visual diff。
打飞机专用 HUD API。
```

## 14. 施工自审

范围自审：

```text
本施工文档没有扩大到 RuntimeRenderer 多段 UI composition pass。
本施工文档没有扩大到 AUI Prefab / Template Reuse。
本施工文档没有要求把 AUI Node 改成 ECS Entity。
本施工文档把外部审查提出的 D0、C0/C1、runtime_supported 真相位、node 跨域拒绝、字段命名、hit-test 注册序全部写入 Gate。
```

测试自审：

```text
每个 Gate 都有明确 cargo test 命令。
整体回归覆盖 editor_ui_model / editor_ui_renderer / editor_core / engine_runtime aui / project_e2e_gate。
若施工中发现测试命令与实际 crate/test 命名不匹配，必须先修订本施工文档，再继续施工。
```

边界自审：

```text
AuiNodeAuthoringProxy / SceneVisualOrderAuthoringEntry 是 editor view model。
VisualOrderKey C-min-r1 可执行域只有 ScreenOverlay / Modal。
跨 World 排序必须 report runtime_supported=false。
单 AUI Node 跨 World 拖拽必须拒绝并给出 extract_to_aui_layer_group_or_canvas。
Inspector 修改必须走现有 AUI command/service。
```

文档同步自审：

```text
完成后必须生成阶段完成记录。
完成后必须更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
完成后必须将本文档移动到 施工文档/已完成。
```
