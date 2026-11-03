# 210-当前可自动化施工文档-RuntimeRenderer Multi-stage UI Composition Pass Productization v1

## 1. 施工目标

根据：

```text
210-RuntimeRenderer-Multi-stage-UI-Composition-Pass-Productization-v1方案.md
其它AI审查目录/29-210-RuntimeRenderer-Multi-stage-UI-Composition-Pass方案审查.md
```

本轮采用：

```text
方案 C-min：Fixed Stage AUI Composition Frame
```

目标：

```text
把当前单一 AUI ScreenOverlay pass 扩展为：
  AUI BeforeWorld
  World / Sprite
  AUI ScreenOverlay
  AUI Modal
  Present

并让 209 中 Canvas / LayerGroup 级视觉顺序具备 runtime evidence。
```

关键边界：

```text
不把 AUI Node 变成 Runtime ECS Entity。
不做完整 WorldSpace UI / UI Camera / Depth-aware UI。
不做单个 AUI Node 与 Scene Entity 任意混排。
BeforeWorld 是 screen-space composition stage，不等于 AuiCanvasMode::WorldSpace。
Modal 本轮只代表 rendering composition stage，不承诺 input blocking / focus trap。
```

## 2. 审查吸收项

本施工文档已吸收 29 号审查要求：

```text
1. AuiCanvas 新增 composition_stage 字段，默认 ScreenOverlay。
2. AuiLayout / AuiDrawList / UiProjection 按 composition_stage 分桶，避免 BeforeWorld 永远空。
3. AuiOverlaySortKey 接入 AuiCanvas.layer / AuiCanvas.sorting_order。
4. SceneVisualOrderRenderSpace 补 BeforeWorld 变体。
5. RuntimeRenderFrameReport 输出 stage presence / skip evidence。
6. e2e report 说明 Modal 是 rendering-only。
```

## 3. 涉及文件

预计修改：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/render_graph.rs
rust/crates/engine_runtime/src/rhi_command_plan.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/render_thread.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/editor_ui_model/src/aui_scene_authoring.rs
rust/crates/editor_core/src/aui_scene_authoring.rs
rust/crates/editor_core/src/authoring_workflow.rs
rust/crates/project_e2e_gate/src/aui_scene_authoring.rs
rust/crates/project_e2e_gate/src/tests.rs
```

预计文档同步：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-06-RuntimeRenderer-Multi-stage-UI-Composition-Pass-Productization-v1/00-总览.md
```

## 4. Gate A：AUI Composition Frame Schema

任务：

```text
新增 AuiCompositionStage：BeforeWorld / ScreenOverlay / Modal。
新增 AuiCanvas.composition_stage，serde default 为 ScreenOverlay。
新增 AuiCompositionFrame / AuiCompositionStageFrame / AuiCompositionReport。
AuiOverlayFrame 旧路径兼容映射到 ScreenOverlay。
AuiLayoutEngine 只 layout ScreenOverlay-mode canvas，但按 composition_stage 保留 stage 信息。
AuiDrawList / AuiOverlayDrawItem 携带 canvas_id / composition_stage / sort_key。
AuiRendererBridge / composition builder 接入 canvas.layer / sorting_order。
新增测试证明 BeforeWorld / ScreenOverlay / Modal 都能真实产生 draw item。
新增测试证明 sort_key 来自 canvas.layer / sorting_order。
```

测试：

```powershell
cargo test -p engine_runtime aui
```

## 5. Gate B：RenderGraph / RHI UI Composition Command

任务：

```text
RenderPassKind 新增 DrawUiComposition。
RenderPassCommand 新增 DrawUiComposition { stage, item_count, text_count, image_count, glyph_count, font_atlas_id, text_pass_inserted, debug_label }。
RhiDrawKind / RhiDrawPayload 新增 UiComposition。
旧 DrawUiOverlay 继续可编译，语义等价 ScreenOverlay。
新增测试覆盖 UiComposition stage payload 序列化和 RHI compile。
```

测试：

```powershell
cargo test -p engine_runtime render_graph
cargo test -p engine_runtime rhi_command_plan
```

## 6. Gate C：RuntimeRenderer Multi-stage Pass Order

任务：

```text
RuntimeRendererInput 支持 aui_composition: Option<&AuiCompositionFrame>。
旧 aui_overlay 路径通过 compatibility 映射为 ScreenOverlay composition。
RuntimeRenderer pass 顺序：
  clear-main
  draw-aui-before-world optional
  world / mesh / sprite existing
  draw-aui-screen-overlay optional
  draw-aui-modal optional
  present-main
空 stage 不生成 draw pass。
RuntimeRenderFrameReport 新增 stage evidence：
  ui_composition_stage_count
  ui_before_world_item_count
  ui_screen_overlay_item_count
  ui_modal_item_count
  ui_before_world_pass_present / skipped
  ui_screen_overlay_pass_present / skipped
  ui_modal_pass_present / skipped
```

测试：

```powershell
cargo test -p engine_runtime runtime_renderer
```

## 7. Gate D：RenderThread / EngineHostLoop / Player Forwarding

任务：

```text
RenderThreadFrameInput / RenderFramePacket 支持 AuiCompositionFrame。
EngineFrameInput 支持 with_aui_composition。
EngineHostLoop render_thread_for_target 支持 composition。
runtime_player_winit 把 AuiRuntimePresenter 输出的 composition frame 转发到 RuntimeRenderer。
旧 overlay 转发兼容保留。
NativeAuiPresentSummary 保持 glyph evidence，不回退。
```

测试：

```powershell
cargo test -p engine_runtime render_thread
cargo test -p runtime_player_winit aui
```

## 8. Gate E：209 Authoring Report Runtime Gap 收敛

任务：

```text
SceneVisualOrderRenderSpace 新增 BeforeWorld。
VisualOrderKey 新增 before_world 构造器。
editor_core::aui_scene_authoring 根据 AuiCanvas.composition_stage 生成 visual_order_key。
BeforeWorld / ScreenOverlay / Modal 的 ScreenOverlay-mode canvas runtime_supported=true。
ScreenCamera / WorldSpace 仍 runtime_supported=false。
单 AUI Node 跨 World 仍 rejected。
AuthoringAiContext / project_e2e_gate runtime_composition_gap_count 根据真实 gap 更新。
```

测试：

```powershell
cargo test -p editor_ui_model aui_scene
cargo test -p editor_core aui_scene
cargo test -p project_e2e_gate aui_scene
```

## 9. Gate F：复杂打飞机 E2E Evidence

任务：

```text
project_e2e_gate 输出 runtime UI composition evidence：
  before_world pass present 或 skipped
  screen_overlay pass present
  modal pass present 或 skipped
  pass order 正确
  glyph_present 不回退
  modal_rendering_only=true
```

测试：

```powershell
cargo test -p project_e2e_gate
```

## 10. Gate G：整体回归与文档同步

任务：

```text
同步 49 / 54 / 施工文档 README / 阶段完成记录 README。
生成阶段完成记录。
施工完成后把本文档移动到 施工文档/已完成/。
```

测试：

```powershell
cargo fmt --check
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p project_e2e_gate
```

## 11. 施工自审

范围检查：

```text
本施工只做 RuntimeRenderer 多段 UI composition pass。
不做完整 WorldSpace UI / UI Camera / Depth-aware UI。
不做 AUI 控件复杂行为。
不改 Project Rule / IR。
```

审查吸收检查：

```text
已把 stage 存储位置定为 AuiCanvas.composition_stage。
已把 layout/extract 分桶列为 Gate A。
已把 sort_key 接线列为 Gate A。
已把 BeforeWorld authoring 枚举列为 Gate E。
已把 stage presence / skip evidence 列为 Gate C。
已把 Modal rendering-only 列为 Gate F。
```

测试检查：

```text
每个 Gate 都有对应 cargo test。
整体回归包含 fmt、engine_runtime、runtime_player_winit、editor_ui_model、editor_core、project_e2e_gate。
```

风险检查：

```text
不允许用空 BeforeWorld pass 假装完成。
不允许用 WorldSpace canvas 假装 BeforeWorld。
不允许用单 overlay pass 假装多 stage。
不允许让 RuntimeRenderer 读取 AuiDocument / Binding / ProjectUiStateSnapshot。
```

结论：

```text
本文档可施工。
```
