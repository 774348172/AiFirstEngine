# 真实 WgpuBackend / RDG / RHI 最小门禁方案

## 当前归属说明：Projection 术语

本文中如果出现以下历史名称：

```text
RenderExtract
RenderAssetBridge / Render Asset Bridge
Physics2DBridge
RuntimeScene Hydration
AuiRenderExtract / AuiRendererBridge
SpriteRenderer2D ECS-to-RenderProxy Bridge
```

统一按 `110-World-Projection-Adapter统一跨域同步规则.md` 理解为：

```text
RenderProjection
AssetProjection
Physics2DProjection
HydrationProjection
UiProjection
RenderProjectionAdapter<SpriteRenderer2D>
```

这些名称可以作为历史实现名保留，但不再作为新增架构概念扩展。后续新增类型只新增对应 `ProjectionAdapter`，不新增独立 Bridge。

本文定义从当前 CPU-side / headless 渲染闭环进入真实 GPU 输出的最小门禁。

本文不是施工文档。施工前仍需另行生成 `施工文档/当前/...`。

## 问题是什么

当前已有链路：

```text
ECS
  -> RenderDirtyTracker
  -> RenderExtract
  -> RenderCommandQueue
  -> RenderSceneState
  -> RendererFeatureBuilder
  -> MinimalRenderer CPU-side report
```

这条链路能证明渲染数据同步正确，但还不能证明游戏真的显示在窗口 / viewport 中。

最小游戏循环需要补上：

```text
RenderSceneState
  -> Runtime Renderer
  -> RDG
  -> RHI
  -> WgpuBackend
  -> texture / surface
  -> Editor Viewport / Window
```

## 源码参考结论

### UE

参考文档：

```text
../UE源码参考/RDG-RHI-RendererBackend源码参考.md
../UE源码参考/RenderCommand-RenderSceneState.md
```

UE 的核心路线：

```text
Game Component dirty
  -> FScene / RenderSceneState
  -> Renderer
  -> FRDGBuilder / RDG Pass
  -> FRHICommandList
  -> FDynamicRHI
  -> D3D12RHI / VulkanRHI / MetalRHI
```

UE 的关键启发：

```text
RenderCommand 和 RHICommand 不是一层。
RDG 负责 pass / resource / dependency / lifetime / validation。
RHI 负责平台图形 API 抽象。
Backend 只负责具体平台实现。
```

### Unity

参考文档：

```text
../Unity源码参考/SRP-CommandBuffer-RendererBackend源码参考.md
../Unity源码参考/PlayerLoop-SRP-CommandBuffer-RenderSync.md
```

Unity 的公开路线：

```text
RenderPipeline
  -> ScriptableRenderContext
  -> Cull / DrawRenderers / ExecuteCommandBuffer
  -> Submit
  -> native renderer / graphics backend
```

Unity 的关键启发：

```text
用户心智要简单。
渲染管线入口要稳定。
图形命令要延迟提交。
底层后端不能暴露给普通项目逻辑。
```

### Bevy

参考文档：

```text
../Bevy源码参考/04-RenderApp-Extract-RenderWorld.md
../Bevy源码参考/05-RenderPhase-RenderGraph-Pipeline.md
```

Bevy 的核心路线：

```text
Main World
  -> Extract
  -> Render World
  -> Render Graph / Render Phase
  -> wgpu
```

Bevy 的关键启发：

```text
Rust + ECS + Extract + wgpu 的路线可行。
但 Bevy 的 RenderWorld / Schedule 体系对自然语言用户不够友好。
本项目应吸收底层结构，不把复杂调度暴露给 AI 默认生成层。
```

## 方案对比

### 方案 A：RenderSceneState 直接接 wgpu

```text
RenderSceneState -> WgpuRenderer -> Surface
```

优点：

```text
最快看到真实画面。
代码量最少。
```

缺点：

```text
绕过 RDG / RHI 长期边界。
后续阴影、后处理、多 View、平台后端会返工。
AI 难以看到 pass / resource / fallback / validation 的结构化证据。
```

结论：

```text
不选。
```

### 方案 B：先做 RHI-min，不做 RDG

```text
RenderSceneState -> RHI Command -> WgpuBackend
```

优点：

```text
平台后端边界较早建立。
比直接 wgpu 更长期。
```

缺点：

```text
缺少 pass / resource / dependency 层。
后面补 RDG 时仍要重组 Renderer。
```

结论：

```text
不作为主路线。
```

### 方案 C-min：RDG-min + RHI-min + WgpuBackend-min

```text
RenderSceneState
  -> RendererFeatureBuilder
  -> RDG-min
  -> RHI-min
  -> WgpuBackend-min
```

优点：

```text
符合 UE / Bevy 的长期分层。
第一版仍可做得很小。
AI 能读懂 pass / resource / target / diagnostics。
后续扩展阴影、材质、后处理、多 View 不推翻底层。
```

缺点：

```text
比直接 wgpu 多一层结构。
第一版施工量更大。
```

结论：

```text
选 C-min。
```

### 方案 D：完整 UE-like Render Thread + RDG + RHI

优点：

```text
长期能力最强。
```

缺点：

```text
第一版过重。
容易在还没看到画面前陷入复杂工程。
```

结论：

```text
不作为第一版。
```

## 推荐方案

采用：

```text
RDG-min + RHI-min + WgpuBackend-min
```

长期结构：

```text
RenderSceneState
  -> RendererFeatureBuilder
  -> RenderGraph
  -> EngineRHI
  -> Backend:
       WgpuBackend
       D3D12Backend
       VulkanBackend
       MetalBackend
```

第一版只实现：

```text
RenderSceneState
  -> single view
  -> clear pass
  -> test triangle / test quad pass
  -> WgpuBackend texture / surface output
  -> RenderFrameReport
```

## 标准结构

### RenderGraph v1

```text
RenderGraph
  graph_id
  frame_index
  views
  resources
  passes
  output_target
  diagnostics
```

规则：

```text
RenderGraph 是一帧图，不是长期状态。
RenderGraph 由 Runtime Renderer / RendererFeatureBuilder 生成。
RenderGraph 不直接读取 ECS。
RenderGraph 不保存 Project Logic。
```

### RenderPass v1

```text
RenderPass
  pass_id
  pass_name
  pass_kind
  view_id
  reads
  writes
  color_targets
  depth_target optional
  commands
  debug_source optional
```

第一版 pass_kind：

```text
Clear
DrawTestGeometry
DrawMeshBasic
Present
```

第一版规则：

```text
Pass 必须声明读写资源。
Pass 执行顺序由 dependency / target 决定。
第一版可以不做复杂 pass culling，但必须能报告 unused resource / missing target。
```

### RenderResource v1

```text
RenderResource
  resource_id
  resource_name
  resource_kind
  format
  size
  usage
  lifetime
```

resource_kind：

```text
Texture
Buffer
SurfaceBackbuffer
ExternalTexture
```

lifetime：

```text
FrameLocal
External
Persistent
```

第一版规则：

```text
FrameLocal 由 RDG-min 管。
External / SurfaceBackbuffer 由 RHI / Backend 提供。
Persistent 第一版只保留结构，不做复杂池化。
```

### EngineRHI v1

```text
EngineRhiDevice
EngineRhiQueue
EngineRhiSurface
EngineRhiTexture
EngineRhiBuffer
EngineRhiCommandEncoder
EngineRhiRenderPass
EngineRhiFrame
```

最小接口：

```text
create_device
create_surface
resize_surface
begin_frame
acquire_surface_texture
create_texture
create_buffer
begin_command_encoder
begin_render_pass
set_pipeline
set_viewport
set_vertex_buffer
draw
end_render_pass
submit
present
```

规则：

```text
EngineRHI 不知道 Entity / Component / Project Logic。
EngineRHI 不读取 RenderCommand。
EngineRHI 只执行 RDG 编译后的 RHI command / pass。
```

### WgpuBackend v1

定位：

```text
EngineRHI 的第一个真实后端。
```

第一版能力：

```text
Headless texture target。
Real surface target。
Clear color。
Test triangle / test quad。
Resize。
Present。
Frame report。
Backend diagnostics。
```

规则：

```text
Headless 必须可自动化测试。
Real surface 只作为本机 smoke gate。
不能让 real window / real GPU 成为唯一测试路径。
WgpuBackend 不能直接读取 ECS / RenderSceneState。
```

### RuntimeRenderer v1

```text
RuntimeRenderer
  input:
    RenderSceneState
    RenderViewState
    QualityProfile
    RenderTarget
  output:
    RenderGraph
    RhiCommandPlan
    RenderFrameReport
```

规则：

```text
RuntimeRenderer 负责游戏世界内容。
EditorUiRenderer 只负责编辑器外壳和控件。
Scene Viewport 是 RuntimeRenderer 输出 texture 与 Editor UI 的组合点。
```

## 最小门禁

第一版必须通过三个 gate。

### Gate 1：Headless GPU-like Gate

目标：

```text
不打开真实窗口，也能验证 RDG/RHI/WgpuBackend 的主要结构。
```

验证：

```text
创建 headless target。
生成 clear pass。
生成 test triangle / quad pass。
执行后得到 frame report。
检查 target size / clear color / command count / diagnostics。
```

### Gate 2：Real Surface Smoke Gate

目标：

```text
本机真实窗口 / surface 可以显示基础画面。
```

验证：

```text
创建 winit window。
创建 wgpu surface。
resize。
clear。
test triangle / quad。
present。
输出 RealSurfaceReport。
```

规则：

```text
Real Surface Smoke Gate 可以依赖本机环境。
但 CI / 默认自动化不能只依赖它。
```

### Gate 3：Editor Viewport Texture Gate

目标：

```text
RuntimeRenderer 输出可以作为 Scene Viewport texture 被 Editor UI 嵌入。
```

验证：

```text
RuntimeRenderer 产出 viewport texture handle / descriptor。
Editor ViewportHost 接收 texture descriptor。
EditorUiRenderer 只绘制外壳和 overlay。
```

## AI 友好规则

AI 默认读取：

```text
RenderFrameReport
RenderGraphReport
RhiBackendReport
Diagnostics
```

AI 默认不直接写：

```text
RenderGraph pass
RHI command
Wgpu command
Shader code
```

AI 可生成：

```text
RenderIntent
Material Graph
Quality Preset
Renderer Feature Config
Debug Capture Request
```

引擎负责转换：

```text
Intent / Graph / Preset
  -> Validation
  -> RendererFeatureBuilder
  -> RDG
  -> RHI
  -> Backend
```

错误反馈必须能回答：

```text
哪一个 pass 失败？
哪一个 resource 缺失？
是 backend 不支持，还是 resource 没加载？
是否发生降级？
降级影响哪个 view / feature？
```

## 第一版不做什么

第一版不做：

```text
完整 Render Thread。
完整 GPU resource pool。
复杂 barrier 推导。
async compute。
multi GPU。
完整 shader compiler。
PBR。
shadow map。
post process。
复杂 material graph codegen。
复杂 mesh batching / instancing。
D3D12 / Vulkan / Metal backend。
```

第一版必须保留位置：

```text
RenderThread boundary
Resource lifetime
Pass dependency
Backend capability
Quality fallback
Diagnostics / Report
```

## 与现有文档关系

本方案依赖：

```text
50-RenderCommand-RenderSceneState方案.md
51-RenderDirtyTracker-RenderExtract-RenderCommand闭环方案.md
55-RenderSceneState-RenderProxy-v1方案.md
17-Runtime-FrameLoop.md
37-Editor-Core与可迁移UI路线.md
47-Native-Editor-Host-BC路线.md
```

本方案补齐：

```text
RenderSceneState 之后如何进入真实 GPU。
Runtime Renderer 和 Editor Viewport 的真实组合方式。
WgpuBackend 在长期 RHI 路线中的位置。
```

## 最终规则

```text
1. 真实 GPU 输出主路线采用 RDG-min + RHI-min + WgpuBackend-min。
2. WgpuBackend 是 EngineRHI 的第一个后端，不是绕过 RHI 的直连 renderer。
3. RenderCommand 只负责 Game 到 RenderSceneState 的同步，不等于 RHI command。
4. RDG-min 负责一帧 pass / resource / target / dependency / diagnostics。
5. RHI-min 负责抽象 device / queue / surface / texture / buffer / command encoder / render pass / present。
6. RuntimeRenderer 从 RenderSceneState 生成 RenderGraph，不直接读取 ECS。
7. EditorUiRenderer 不渲染游戏世界，只嵌入 RuntimeRenderer 输出。
8. 第一版必须有 headless 自动化 gate。
9. real surface 只作为 smoke gate，不能成为唯一验证方式。
10. AI 默认读取 report 和 diagnostics，不直接写 RDG / RHI / Wgpu command。
11. 第一版只做 clear + test geometry + basic mesh 的最小链路。
12. 阴影、PBR、后处理、复杂 shader、完整 Render Thread 都不进入第一版。
```
