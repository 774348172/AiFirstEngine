# 当前可自动化施工文档：EngineHostLoop / RendererFeatureBuilder / Minimal Renderer v1

> 状态提示：EngineHostLoop / RendererFeatureBuilder / Minimal Renderer v1 已完成。本文档保留为已完成施工计划和复盘依据，不再作为当前开工入口；当前入口以 `49-当前状态与下一步入口.md` 和 `54-当前可自动化施工总入口.md` 为准。

## 1. 施工目标

把当前已经存在但尚未完全串起来的 Rust runtime 零件，接成第一版正式一帧闭环：

```text
EngineHostLoop
  -> RuntimeFrame / FrameLoop
  -> World / DirtyRecord
  -> RenderExtract
  -> RenderCommandQueue normalize / merge
  -> RenderSceneState.apply_batch
  -> RendererFeatureBuilder
  -> MinimalRendererFrame
  -> RenderFrameReport / RuntimeTrace / FrameHash
```

这一步的目的不是“画出真实画面”，而是让 runtime 从旧 `RenderSnapshot compatibility output` 主链路，转到正式 `RenderExtract -> RenderCommand -> RenderSceneState -> RendererFeatureBuilder` 主链路。

## 2. 当前前置状态

已完成：

```text
Rust Runtime A0 / A1
ECS Storage v1
DirtyRecord
RenderExtractContext
RenderCommandQueue
RenderSceneState
RenderProxy / RenderViewState / RenderFrameViewData
RenderFrameReport
GoldenScenarioId::RenderCommandSceneState
```

关键源码：

```text
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/render_extract.rs
rust/crates/engine_runtime/src/render_command.rs
rust/crates/engine_runtime/src/render_state.rs
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/golden.rs
```

当前问题：

```text
frame_loop.rs 仍以 RenderSnapshot compatibility output 为主输出。
RenderExtract / RenderCommand / RenderSceneState 已存在，但没有被正式 FrameLoop 接成主链路。
RendererFeatureBuilder 还没有成为 RenderSceneState -> renderer input 的明确层。
Minimal Renderer 还没有 CPU 侧可测试输出。
EngineHostLoop 还没有作为最高层心跳源落到 Rust runtime。
```

## 3. 本轮允许修改范围

允许修改：

```text
rust/crates/engine_runtime/src/lib.rs
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/render_extract.rs
rust/crates/engine_runtime/src/render_command.rs
rust/crates/engine_runtime/src/render_state.rs
rust/crates/engine_runtime/src/golden.rs
```

允许新增：

```text
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/engine_runtime/src/renderer_feature_builder.rs
rust/crates/engine_runtime/src/minimal_renderer.rs
```

必要时允许小范围新增测试 fixture，但不得引入真实窗口或 GPU 依赖。

## 4. 本轮禁止范围

本轮不做：

```text
真实 winit window
真实 wgpu surface
真实 Render Thread
真实 RDG
真实 RHI
真实 GPU buffer / texture / shader / pipeline
真实 draw call
Native Editor Viewport texture 嵌入
Asset DB / Importer
IR -> Rust AOT
Android / iOS / Web 打包
```

禁止把 `RenderSnapshot` 重新变成新渲染主线。

允许保留：

```text
RenderSnapshot compatibility output
```

但它只能作为旧测试和兼容输出，不得作为 RendererFeatureBuilder 的输入。

## 5. 施工拆分

### A0：基线验证

目标：

```text
确认当前 Rust workspace 可测试。
记录当前失败项，如果失败来自工具链环境，暂停实现并写明原因。
```

执行：

```powershell
cargo test --manifest-path rust/Cargo.toml
```

完成标准：

```text
当前已有测试通过，或者失败原因明确且不是本轮修改导致。
```

### A1：新增 EngineHostLoop 最小结构

新增：

```text
engine_host_loop.rs
```

第一版结构：

```text
EngineHostLoop
EngineHostMode
EngineFrameInput
EngineFrameOutput
```

建议最小模式：

```text
HeadlessServer
ExportedGame
EditorPlay
EditorStep
EditorPause
```

第一版不需要接真实 Editor UI，只需要保证最高层心跳源存在。

执行链路：

```text
EngineHostLoop.tick()
  -> FrameBegin trace
  -> RuntimeFrame when mode allows runtime advance
  -> RenderFrame when mode allows render data build
  -> FrameEnd trace
```

测试：

```text
headless server advances runtime frame
editor pause does not advance runtime frame
editor step advances exactly one runtime frame
exported game advances runtime and render data
```

### A2：改造 FrameLoop 为正式 RuntimeFrame 输出

当前 `FrameLoop::tick(&World)` 是只读 world，并输出 RenderSnapshot。

本轮要新增正式路径，建议不要直接删除旧接口，而是新增：

```text
RuntimeFrameLoop
RuntimeFrameInput
RuntimeFrameOutput
```

或在 `FrameLoop` 上新增不破坏兼容的正式方法：

```text
tick_runtime_frame(
  world: &mut World,
  render_scene: &mut RenderSceneState,
  extract: &mut RenderExtractContext,
) -> RuntimeFrameOutput
```

RuntimeFrameOutput 第一版字段：

```text
frame_index
frame_hash
runtime_trace
render_frame_report
merged_render_command_count
applied_render_command_count
render_scene_proxy_count
snapshot_compat optional
```

正式执行顺序：

```text
FrameBegin
FixedUpdate
Update
LateUpdate
RenderExtract
RenderCommandQueue.normalize_merge
RenderSceneState.apply_batch
RenderFrameReport
FrameHash
FrameEnd
```

测试：

```text
runtime frame consumes dirty records once
runtime frame applies render commands to RenderSceneState
runtime frame report contains command counters
runtime frame hash is stable for same input
runtime trace records RenderExtract and FrameEnd
```

### A3：新增 RendererFeatureBuilder v1

新增：

```text
renderer_feature_builder.rs
```

定位：

```text
RenderSceneState -> renderer input 的唯一第一层。
```

第一版不生成 RDG，不生成 RHI command，只生成 CPU 侧可验证的 feature frame。

建议结构：

```text
RendererFeatureBuilder
RendererFeatureFrame
RendererFeatureView
RendererFeatureDrawItem
RendererFeatureDiagnostic
```

RendererFeatureFrame 第一版字段：

```text
frame_index
views
draw_items
diagnostics
counters
```

RendererFeatureDrawItem 第一版字段：

```text
proxy_id
source_entity_id
payload_kind
mesh_ref optional
material_ref optional
transform
visible
layer
```

规则：

```text
RendererFeatureBuilder 只读 RenderSceneState。
RendererFeatureBuilder 不读 ECS World。
RendererFeatureBuilder 不修改 RenderSceneState。
RendererFeatureBuilder 不生成 GPU command。
RendererFeatureBuilder 产生的 draw item 是 CPU 数据，不是 RHI draw call。
```

测试：

```text
builder reads RenderSceneState and produces one draw item
builder ignores invisible proxy or marks it as skipped according to v1 rule
builder produces per-view output from RenderViewState
builder output is stable by proxy_id order
```

### A4：新增 Minimal Renderer v1 CPU 输出

新增：

```text
minimal_renderer.rs
```

定位：

```text
验证 RendererFeatureBuilder 输出能被一个 renderer backend 消费。
```

第一版只做 CPU validation backend，不画真实像素。

建议结构：

```text
MinimalRenderer
MinimalRendererFrame
MinimalRendererDrawRecord
MinimalRendererReport
```

MinimalRendererFrame 第一版字段：

```text
frame_index
view_count
draw_record_count
draw_records
warnings
```

测试：

```text
minimal renderer consumes RendererFeatureFrame
minimal renderer reports draw count
minimal renderer reports missing mesh as warning
minimal renderer does not mutate RenderSceneState
```

### A5：EngineHostLoop 接入 RendererFeatureBuilder / MinimalRenderer

目标：

```text
EngineHostLoop.tick() 能返回一帧完整 CPU 闭环结果。
```

输出建议：

```text
EngineFrameOutput
  frame_index
  runtime_advanced
  render_built
  runtime_trace
  render_frame_report
  renderer_feature_frame
  minimal_renderer_frame
  frame_hash
```

测试：

```text
engine host exported game frame produces renderer feature frame
engine host editor pause keeps last render data or reports no runtime advance
engine host editor step advances one runtime frame and builds one render frame
engine host output includes RenderFrameReport and RuntimeTrace
```

### A6：Golden Scenario

新增 golden scenario：

```text
GoldenScenarioId::EngineHostLoopMinimalRenderer
```

场景：

```text
加载一个包含 Transform + Renderable 的最小 scene。
运行 EngineHostLoop 一帧。
World dirty 被 RenderExtract 消费。
RenderSceneState 产生一个 RenderProxy。
RendererFeatureBuilder 产生一个 draw item。
MinimalRenderer 产生一个 draw record。
RenderFrameReport 记录 applied command。
FrameHash 稳定。
```

测试：

```text
cargo test --manifest-path rust/Cargo.toml engine_host_loop_minimal_renderer
```

### A7：文档和阶段记录要求

执行完成后必须：

```text
把本文档移动到 施工文档/已完成。
新增 阶段完成记录/2026-06-23-EngineHostLoop-RendererFeatureBuilder-MinimalRenderer-v1/00-总览.md。
更新 阶段完成记录/README.md。
更新 49-当前状态与下一步入口.md。
更新 54-当前可自动化施工总入口.md。
更新 18-长期主义实现路径.md。
如果实现结果沉淀出新规则，回填 17 / 50 / 51 / 55。
```

## 6. 最小测试清单

本轮每个需求都必须有最小单元测试。

最低测试命令：

```powershell
cargo test --manifest-path rust/Cargo.toml
```

建议新增或覆盖测试：

```text
engine_host_loop_headless_advances_runtime
engine_host_loop_pause_does_not_advance_runtime
engine_host_loop_step_advances_once
runtime_frame_consumes_dirty_records_once
runtime_frame_applies_render_commands_to_scene_state
renderer_feature_builder_outputs_draw_items
renderer_feature_builder_does_not_read_world
minimal_renderer_consumes_feature_frame
minimal_renderer_reports_missing_mesh
golden_engine_host_loop_minimal_renderer
```

## 7. 验收标准

本轮完成后必须满足：

```text
EngineHostLoop 成为 Rust runtime 最高层心跳源的最小实现。
FrameLoop 正式路径不再以 RenderSnapshot 作为渲染主输入。
RenderExtract / RenderCommandQueue / RenderSceneState 被正式一帧链路调用。
RendererFeatureBuilder 成为 RenderSceneState 到 renderer input 的唯一入口。
MinimalRenderer v1 能消费 RendererFeatureFrame 并生成 CPU draw report。
RenderFrameReport / RuntimeTrace / FrameHash 仍然存在，并能解释本帧发生了什么。
所有新增能力都有 cargo test 覆盖。
```

## 8. 风险和暂停条件

遇到以下情况必须暂停，不要硬做：

```text
需要真实 GPU / wgpu surface 才能继续。
需要真实 Render Thread 才能继续。
需要改动 Asset DB / Importer 才能继续。
需要引入大型第三方渲染库。
需要改变 RenderCommand / RenderSceneState 已确认架构规则。
需要让 RendererFeatureBuilder 直接读取 ECS World。
需要让项目逻辑或 AI 直接生成 RenderCommand / RenderProxy。
```

如果只是 Rust 工具链问题，例如 `link.exe` 或 GUI 链路缺失，本轮可以继续做 headless CPU 测试；真实窗口相关任务必须暂停。

## 9. 本轮之后的下一步

本轮完成后，再判断是否进入：

```text
58-Native Editor real-window gate
59-Asset DB / Importer v1
60-真实 WgpuBackend / RDG / RHI 最小门禁
```

不要在本轮直接跳到这些方向。
