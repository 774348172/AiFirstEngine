# 155-当前可自动化施工文档-Codebase Hygiene Stage E editor_wgpu_renderer 拆分

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
49-当前状态与下一步入口.md
阶段完成记录/2026-07-02-Codebase-Hygiene-StageD-editor_ui_renderer拆分/00-总览.md
```

本阶段执行：

```text
Codebase Architecture Hygiene Gate v1 - Stage E editor_wgpu_renderer split
```

目标是把 `rust/crates/editor_wgpu_renderer/src/lib.rs` 从单文件 WGPU UI renderer 拆成长期可维护结构：

```text
diagnostics: RealUiPresentReport / diagnostic / schema version
draw_plan: UiDrawList -> UiGpuDrawPlan
texture_atlas: CpuGlyphAtlas / glyph atlas data
font_system: EditorFontSystem / builtin debug font / glyph layout
render_graph: UiGpuDrawPlan -> UiRenderGraph -> UiRhiCommandPlan
headless: HeadlessUiGpuRenderer
real_wgpu: real WGPU backend, pipeline, vertices, texture upload
surface: surface/present boundary helpers, first version may stay as a thin module used by real_wgpu
```

本阶段只做结构治理：

```text
不新增渲染功能
不改变 public API 的 crate root 导出
不改变 RealUiPresentReport schema
不改变 UiGpuDrawPlan / UiRenderGraph / UiRhiCommandPlan 语义
不改变 headless present 行为
不改变 real-wgpu feature 的 public type
不把编辑器业务、命令执行、项目 gameplay 概念放入 editor_wgpu_renderer
```

## 2. 当前基线

当前文件：

```text
rust/crates/editor_wgpu_renderer/src/lib.rs: 1989 lines
```

当前主要职责混在单文件内：

```text
UiGpuDrawPlan
UiRenderGraph
UiRhiCommandPlan
EditorFontSystem
CpuGlyphAtlas
BuiltinDebugFont
RealUiPresentReport
HeadlessUiGpuRenderer
real_wgpu feature backend
tests
```

## 3. 目标目录

目标结构：

```text
rust/crates/editor_wgpu_renderer/src/lib.rs
rust/crates/editor_wgpu_renderer/src/diagnostics.rs
rust/crates/editor_wgpu_renderer/src/draw_plan.rs
rust/crates/editor_wgpu_renderer/src/texture_atlas.rs
rust/crates/editor_wgpu_renderer/src/font_system.rs
rust/crates/editor_wgpu_renderer/src/render_graph.rs
rust/crates/editor_wgpu_renderer/src/headless.rs
rust/crates/editor_wgpu_renderer/src/surface.rs
rust/crates/editor_wgpu_renderer/src/real_wgpu.rs
rust/crates/editor_wgpu_renderer/src/tests.rs
```

`lib.rs` 最终只保留 crate facade、模块声明、public re-export 和 feature re-export。

## 4. 拆分归属

```text
diagnostics.rs:
  UI_GPU_DRAW_PLAN_SCHEMA_VERSION
  UI_RENDER_GRAPH_SCHEMA_VERSION
  UI_RHI_COMMAND_PLAN_SCHEMA_VERSION
  REAL_UI_PRESENT_REPORT_SCHEMA_VERSION
  RealUiPresentDiagnosticSeverity
  RealUiPresentDiagnostic
  RealUiPresentReport

draw_plan.rs:
  UiGpuDrawableRect
  UiGpuDrawableRectSource
  UiGpuTextGlyph
  UiUvRect
  UiGpuDrawPlan
  UiGpuDrawPlan::from_draw_list

texture_atlas.rs:
  CpuGlyphAtlas
  GlyphKey
  GlyphAtlasEntry
  atlas allocation / alpha write

font_system.rs:
  TextRenderStats
  EditorFontSystem
  BuiltinDebugFont
  load_system_font / system_font_candidates
  glyph_rows

render_graph.rs:
  UiRenderPassKind
  UiRenderResourceKind
  UiRenderResource
  UiRenderPass
  UiRenderGraph
  UiRhiCommandKind
  UiRhiCommand
  UiRhiCommandPlan

headless.rs:
  HeadlessUiGpuRenderer

surface.rs:
  surface/present 边界 helper
  第一版允许仅承接 real_wgpu 的 surface acquisition / error mapping helper，不强行制造抽象

real_wgpu.rs:
  RealWgpuUiRenderer
  UiVertex / TextVertex
  WGPU pipeline / text pipeline / bind group / vertex conversion
  #[cfg(feature = "real-wgpu")] 测试

tests.rs:
  原有非 real-wgpu 单元测试
```

## 5. 分阶段施工

### E0：施工前基线

生成阶段目录：

```text
框架设计/引擎总体架构/阶段完成记录/2026-07-02-Codebase-Hygiene-StageE-editor_wgpu_renderer拆分
```

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Hygiene-StageE-editor_wgpu_renderer拆分\before.json
cargo test -p editor_wgpu_renderer
cargo test -p editor_wgpu_renderer --features real-wgpu --no-run
```

### E1：拆 diagnostics / render_graph

移动：

```text
src/diagnostics.rs
src/render_graph.rs
```

要求：

```text
schema version 字符串不变
RealUiPresentReport JSON 字段不变
UiRenderGraph / UiRhiCommandPlan public API 仍从 crate root 导出
```

验证：

```powershell
cargo test -p editor_wgpu_renderer report_is_serializable
cargo test -p editor_wgpu_renderer ui_render_graph_compiles_draw_plan_into_passes_and_resources
cargo test -p editor_wgpu_renderer ui_rhi_command_plan_compiles_render_graph_for_backend_execution
cargo test -p editor_wgpu_renderer
```

### E2：拆 texture_atlas / font_system

移动：

```text
src/texture_atlas.rs
src/font_system.rs
```

要求：

```text
字体加载候选路径不变
ab_glyph_atlas / builtin_debug_font 两条路径不变
glyph atlas alpha 数据不变
fallback debug font 行为不变
```

验证：

```powershell
cargo test -p editor_wgpu_renderer draw_plan_counts_rect_text_viewport
cargo test -p editor_wgpu_renderer draw_plan_falls_back_for_unsupported_text
cargo test -p editor_wgpu_renderer draw_plan_skips_text_when_rect_too_small
cargo test -p editor_wgpu_renderer
```

### E3：拆 draw_plan

移动：

```text
src/draw_plan.rs
```

要求：

```text
UiGpuDrawPlan::from_draw_list 行为不变
ViewportTextureSlot placeholder rect 行为不变
empty surface 错误码不变
glyph metrics 和统计字段不变
```

验证：

```powershell
cargo test -p editor_wgpu_renderer draw_plan_rejects_empty_surface
cargo test -p editor_wgpu_renderer headless_renderer_treats_viewport_slot_as_placeholder_rect
cargo test -p editor_wgpu_renderer draw_plan_counts_rect_text_viewport
cargo test -p editor_wgpu_renderer
```

### E4：拆 headless / surface

移动：

```text
src/headless.rs
src/surface.rs
```

要求：

```text
HeadlessUiGpuRenderer public API 不变
present 成功/失败 report 行为不变
surface.rs 不引入长期过度抽象，只承接 surface/present 边界 helper
```

验证：

```powershell
cargo test -p editor_wgpu_renderer headless_renderer_reports_rendered_glyphs
cargo test -p editor_wgpu_renderer report_is_serializable
cargo test -p editor_wgpu_renderer
```

### E5：拆 real_wgpu feature backend

移动：

```text
src/real_wgpu.rs
```

要求：

```text
#[cfg(feature = "real-wgpu")] public re-export 不变
RealWgpuUiRenderer::new / resize / present public API 不变
WGPU pipeline / text pipeline / glyph atlas upload 行为不变
real-wgpu feature 编译通过
```

验证：

```powershell
cargo test -p editor_wgpu_renderer --features real-wgpu --no-run
cargo test -p editor_wgpu_renderer --features real-wgpu wgpu_backend_rect_vertices_require_rhi_draw_rect_command
cargo test -p editor_wgpu_renderer --features real-wgpu wgpu_backend_text_vertices_require_rhi_draw_text_command
```

如果本机环境阻止 real-wgpu 测试运行，则至少保留：

```powershell
cargo test -p editor_wgpu_renderer --features real-wgpu --no-run
```

并在完成记录中写明原因。

### E6：最终回归与归档

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Hygiene-StageE-editor_wgpu_renderer拆分\after.json
cargo fmt --check
cargo test -p editor_wgpu_renderer
cargo test -p editor_wgpu_renderer --features real-wgpu --no-run
cargo test -p editor_window_winit
cargo test -p code_hygiene
```

完成后：

```text
生成 阶段完成记录/2026-07-02-Codebase-Hygiene-StageE-editor_wgpu_renderer拆分/00-总览.md
移动本施工文档到 施工文档/已完成
更新 00-文档地图 / 49 / 54 / README / 施工文档 README / 阶段完成记录 README
```

## 6. 验收标准

```text
editor_wgpu_renderer/src/lib.rs 变成 facade 级文件。
editor_wgpu_renderer 不再是单文件 crate。
UiGpuDrawPlan / UiRenderGraph / UiRhiCommandPlan public API 兼容。
RealUiPresentReport schema 兼容。
HeadlessUiGpuRenderer public API 兼容。
RealWgpuUiRenderer 在 real-wgpu feature 下继续可编译。
现有 editor_wgpu_renderer 测试不减少。
editor_window_winit 回归通过。
code_hygiene after.json 显示 editor_wgpu_renderer/src/lib.rs 明显下降。
没有新增编辑器业务功能、运行时 gameplay 规则或项目侧概念。
```

## 7. 施工文档自审

```text
Specification fit:
  满足用户要求：为 Stage E editor_wgpu_renderer 生成施工文档，不直接开始施工。

Rule fit:
  延续 151 的 Stage E 边界，按 render graph / draw plan / font / atlas / surface / diagnostics 拆分，不重新讨论已确认方案。

Textual consistency:
  目标目录、拆分归属、阶段验证和最终验收一致，均围绕 editor_wgpu_renderer 单文件治理。

Design fit:
  保持 editor_wgpu_renderer 的长期边界：UI DrawList 到 GPU draw plan / render graph / RHI command / present report，不理解业务，不接项目规则。

Implementation feasibility:
  当前 lib.rs 已天然按 UiGpuDrawPlan、FontSystem、RenderGraph、Headless、real_wgpu 分段，适合机械迁移并用现有测试兜底。

Practical reasonableness:
  按 E1-E6 分阶段迁移，每阶段都有针对性测试和最终回归；surface.rs 允许先做薄模块，避免为了拆分制造空抽象。
```

结论：

```text
本施工文档通过自审，可以作为当前可自动化施工文档。
```
