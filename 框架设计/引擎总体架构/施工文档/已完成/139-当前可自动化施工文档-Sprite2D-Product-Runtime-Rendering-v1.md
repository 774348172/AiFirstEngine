# 139-当前可自动化施工文档-Sprite2D Product Runtime Rendering v1

## 1. 施工目标

按 `139-Sprite2D-Product-Runtime-Rendering-v1方案.md` 实现第一版产品级 Sprite2D 运行渲染链路：

```text
RenderSceneState
  -> Sprite2DRenderPipeline
  -> Sprite2DRenderFrame
  -> Sprite2DDrawPlan
  -> RenderGraph pass
  -> RhiCommandPlan
  -> EngineRHI Backend
```

本次施工只补引擎底座能力，不加入打飞机专用 API。

## 2. 不做范围

```text
Tilemap
Sprite Mask
2D Light
Sprite Animation
Atlas Packer
Dynamic Batching Optimizer
Custom Shader Graph
复杂多相机合成
项目玩法概念
```

## 3. Gate A：新增 Sprite2DRenderPipeline 模块

新增文件：

```text
rust/crates/engine_runtime/src/sprite2d_render_pipeline.rs
```

新增结构：

```text
Sprite2DRenderPipeline
Sprite2DRenderFrame
Sprite2DRenderView
Sprite2DDrawPlan
Sprite2DRenderDiagnostic
Sprite2DRenderCounters
```

职责：

```text
从 RendererFeatureFrame / RenderViewState 生成 Sprite2D draw plan。
只处理 Sprite payload。
保持排序稳定。
输出 AI 可读 diagnostic。
```

最小测试：

```text
cargo test -p engine_runtime sprite2d_render_pipeline
```

必须覆盖：

```text
visible sprite 生成 draw plan
missing sprite_ref 产生 sprite_missing_ref
layer mismatch 产生 sprite_layer_mismatch
排序按 layer/order/z/proxy 稳定
```

## 4. Gate B：资源绑定接入

Sprite2D draw plan 必须生成：

```text
RenderBindingSet
```

第一版策略：

```text
有 sprite_ref 时生成 Sprite2DTexture usage 的最小 binding。
没有真实 runtime asset record 时允许 fallback binding，但 diagnostic 必须说明 sprite_binding_fallback。
```

不得新增独立 `SpriteTextureBridge`。

最小测试：

```text
cargo test -p engine_runtime sprite2d_render_pipeline
```

必须覆盖：

```text
draw plan 携带 binding
binding 使用 sprite-2d-texture 语义
fallback_used 可报告
```

## 5. Gate C：RuntimeRenderer 接入

修改：

```text
rust/crates/engine_runtime/src/runtime_renderer.rs
```

规则：

```text
RuntimeRenderer 继续负责 RenderGraph 外壳：clear / present / AUI overlay。
Sprite2D draw pass 由 Sprite2DRenderPipeline 输出。
RuntimeRenderer 不再直接散写 Sprite2D 判断逻辑。
```

最小测试：

```text
cargo test -p engine_runtime runtime_renderer
```

必须覆盖：

```text
sprite scene 生成 DrawSpriteTextured
RhiCommandPlan 携带 SpriteTextured binding
AUI overlay 仍在 Sprite2D pass 之后、Present 之前
mesh 现有行为不破坏
```

## 6. Gate D：Report 接入

`RuntimeRenderFrameReport.diagnostics` 必须能承接 Sprite2D diagnostic。

第一版最小 code：

```text
sprite_ready
sprite_missing_ref
sprite_layer_mismatch
sprite_binding_fallback
proxy_not_visible
```

最小测试：

```text
cargo test -p engine_runtime runtime_renderer
```

必须覆盖：

```text
missing sprite_ref 出现在 RuntimeRenderFrameReport
layer mismatch 出现在 RuntimeRenderFrameReport
```

## 7. Gate E：整体测试

完成后执行：

```text
cargo fmt --check
cargo test -p engine_runtime sprite2d_render_pipeline
cargo test -p engine_runtime renderer_feature_builder
cargo test -p engine_runtime runtime_renderer
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

如 `runtime_player_winit` 因 Windows 应用控制策略拦截 exe，则记录阻塞，不把它伪装为通过。

## 8. 施工文档自审

### 8.1 Specification fit

通过。施工项覆盖 M5 的 Camera/Layer/Sorting/Binding/DrawPlan/Report 最小闭环。

### 8.2 Rule fit

通过。遵守 Projection 规则，不新增独立 bridge，不加入项目玩法 API。

### 8.3 Textual consistency

通过。Gate 顺序从 pipeline 到 renderer 接入，再到 report 和整体测试。

### 8.4 Design fit

通过。把 Sprite2D 逻辑从 RuntimeRenderer 收敛到专门 pipeline，长期更清晰。

### 8.5 Implementation feasibility

通过。当前 runtime 已有 SpritePayload、RendererFeatureFrame、DrawSpriteTextured、RenderBindingSet。

### 8.6 Practical reasonableness

通过。所有 gate 都可 headless 测试，不依赖真实窗口。
