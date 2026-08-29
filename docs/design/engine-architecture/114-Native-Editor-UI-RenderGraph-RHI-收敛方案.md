# 114-Native Editor UI RenderGraph / RHI 收敛方案

## 问题

`editor_wgpu_renderer` 已经可以把 Native Editor UI 画到 WGPU Surface，并且已经接入 FontSystem v1。  
但长期架构不能让编辑器上层直接依赖 WGPU 心智，否则后续切换到 `EngineRHI + Backend` 时会返工。

本方案只收敛一件事：

```text
EditorUiDrawList
  -> UiRenderGraph / UiRenderPass
  -> UiRhiCommandPlan
  -> WgpuBackend
```

## 其它引擎对比

### UE

UE Slate 的路线接近：

```text
Slate DrawElements
  -> SlateRenderer
  -> RHI command
  -> D3D12 / Vulkan / Metal backend
```

可借鉴：

```text
UI draw element 不直接等于平台 GPU API。
Renderer 和 RHI / Backend 分层。
```

### Unity

Unity Editor UI / UI Toolkit 不把平台图形 API 暴露给普通 EditorWindow。  
可借鉴：

```text
用户和编辑器模型看到的是 UI / Panel / Command。
底层 backend 可以变化。
```

### Bevy

Bevy 以 wgpu 为主要图形后端，路线简单直接。  
可借鉴：

```text
Rust + winit + wgpu 作为第一版真实后端可行。
```

不直接照搬：

```text
我们不把 wgpu 作为长期唯一抽象边界，而是放在 Backend 层。
```

## 最终规则

```text
1. editor_wgpu_renderer crate 可以继续保留，但它的定位是 Native Editor UI renderer backend crate。
2. Editor Core / Editor UI Model / SelfUiRenderer 不允许依赖 wgpu 类型。
3. UiDrawList 先编译成 UiGpuDrawPlan，作为当前兼容层。
4. UiGpuDrawPlan 必须继续编译成 UiRenderGraph。
5. UiRenderGraph 必须继续编译成 UiRhiCommandPlan。
6. WgpuBackend 只执行 UiRhiCommandPlan，不直接成为上层架构真相。
7. Headless renderer 和 Real WGPU renderer 都必须走同一套 UiRenderGraph / UiRhiCommandPlan。
8. Report / Diagnostics 必须能暴露 RenderGraph / RHI plan 阶段错误。
```

## 当前 v1-min 结构

```text
UiRenderGraph
  resources:
    SurfaceBackbuffer
    VertexBuffer
    GlyphAtlasTexture

  passes:
    Clear
    DrawRects
    DrawText
    Present
```

```text
UiRhiCommandPlan
  commands:
    ClearSurface
    DrawRectBatch
    DrawTextBatch
    PresentSurface
```

## 边界

本次收敛负责：

```text
Native Editor UI 绘制链路分层。
让 WGPU 成为 backend，而不是上层渲染真相。
为后续 EngineRHI / 多后端迁移留接口位置。
```

本次不负责：

```text
完整 Runtime Renderer RHI 化。
把 Runtime Sprite2D / Mesh 渲染迁入同一 RHI。
D3D12 / Vulkan / Metal backend。
复杂 UI clipping / batching / texture atlas eviction。
```

## 为什么适合我们

AI 友好：

```text
AI 可以读 UiRenderGraph / UiRhiCommandPlan / Report，不需要理解 wgpu 细节。
```

复杂项目可维护：

```text
以后 UI pass、font atlas、viewport texture、clip pass 可以挂到 graph，不把逻辑散进 WGPU present 函数。
```

简单：

```text
第一版只加 Clear / DrawRects / DrawText / Present 四类 pass，不引入完整 RDG。
```

效率：

```text
当前只是结构收敛，不改变绘制行为。
后续再做 batch 合并、atlas revision、GPU resource reuse。
```
