# Runtime Viewport Rendering System 方案

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

本文档定义 Runtime Renderer 如何把游戏世界输出到编辑器 Scene / Game Viewport。
本文档是长期架构规则，不是施工文档。具体施工见：

```text
施工文档/已完成/71-当前可自动化施工文档-Runtime-Viewport-Rendering-System-D-min.md
```

## 问题是什么

当前已经具备：

```text
Runtime Package
  -> RuntimeAssetLoader
  -> Scene / Prefab / Entity 实例化到 Rust ECS
  -> RenderExtract
  -> RenderCommand
  -> RenderSceneState
  -> RuntimeRenderer
  -> RenderGraph
  -> RhiCommandPlan
  -> HeadlessRhiBackend / WgpuBackend skeleton
```

缺口是：

```text
Runtime Renderer
  -> viewport texture / surface
  -> Editor Scene View / Game View 显示
```

也就是说，Runtime 已经能生成渲染数据和 headless report，但编辑器还没有一条稳定、可测试、长期不返工的链路把 Runtime 世界画面嵌入 viewport。

## 成熟引擎参考

### Unreal Engine

源码参考：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/GameViewportClient.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/Slate/SceneViewport.cpp
```

UE 的关键流程：

```text
FSceneViewport
  -> 管 viewport 尺寸、输入、backbuffer / render target

UGameViewportClient::Draw
  -> 构造 FSceneViewFamily
  -> GetRendererModule().BeginRenderingViewFamily(...)

Renderer / RDG / RHI
  -> 渲染 World / Scene
  -> 输出到 viewport target

Slate / Canvas / Overlay
  -> 负责编辑器 UI、调试 UI、HUD 或 overlay
```

UE 给我们的启发：

```text
Viewport 是输出目标和输入区域。
Renderer 负责渲染世界。
UI 系统不直接渲染世界。
Renderer 不应该理解编辑器面板结构。
```

### Unity

源码参考：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/GameView/GameView.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/SceneView/SceneView.cs
```

Unity 的关键流程：

```text
GameView
  -> 持有 RenderTexture
  -> RenderView(...)
  -> EditorGUIUtility.DrawTextureHdrSupport(...) 把 RenderTexture 画进 GameView
  -> QueueGameViewInputEvent(...) 把编辑器输入排入游戏循环

SceneView
  -> 持有 SceneView Camera
  -> 准备 RenderTexture
  -> Handles.DrawCamera / DrawCameraStep1 / DrawCameraStep2
  -> 叠加 gizmo、selection outline、overlay
```

Unity 给我们的启发：

```text
编辑器嵌入 Runtime 画面，最稳定的方式是先渲染到 texture，再由 UI 显示。
Scene View 和 Game View 可以共享世界数据，但应有独立 camera / view state。
输入要从 viewport 坐标转换后进入 runtime，不应让 runtime 读取编辑器 UI 坐标。
```

### Bevy

源码参考：

```text
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_render/src/camera.rs
```

Bevy 的关键流程：

```text
Camera
  -> RenderTarget:
       Window
       Image
       TextureView
       None

Main World
  -> Extract
  -> Render World
  -> Render Graph / Render Phase
  -> wgpu target
```

Bevy 给我们的启发：

```text
RenderTarget 抽象要早做。
Window / texture / external texture view 应该是同一套 target 语义。
Headless / texture target 对自动化测试很重要。
```

## 方案对比

### 方案 A：Runtime 直接输出到 wgpu surface

```text
RenderSceneState -> WgpuRenderer -> Surface
```

优点：

```text
最快看到真实窗口画面。
第一版代码量最少。
```

缺点：

```text
绕过 RenderGraph / RHI，后期接 D3D12 / Vulkan / Metal 会返工。
AI 看不到结构化 pass / resource / target / diagnostics。
编辑器 viewport、headless 测试、preview texture 后续都要补第二套规则。
```

结论：

```text
不选。
```

### 方案 B：只做 ViewportTexture，不抽象 Surface

```text
RuntimeRenderer -> ViewportTexture -> EditorUiRenderer
```

优点：

```text
接近 Unity GameView。
第一版容易嵌进编辑器。
headless 也较好测试。
```

缺点：

```text
Surface / Window / RenderTexture 后续会重新补抽象。
第一版容易把 ViewportTexture 写死成唯一目标。
```

结论：

```text
不作为长期规则，但吸收其“texture first”的落地方式。
```

### 方案 C：真实 Surface 优先

```text
RuntimeRenderer -> SurfaceTarget -> Window
```

优点：

```text
更接近独立游戏运行窗口。
可以更早验证真实 swapchain / present。
```

缺点：

```text
受 OS / GPU / driver / window event loop 影响大。
默认自动化测试不稳定。
编辑器 Scene / Game View 嵌入路径仍要补。
```

结论：

```text
不作为第一版主线，保留为 smoke gate。
```

### 方案 D-min：统一 RenderTarget 抽象，ViewportTexture 优先

```text
RuntimeRenderer
  -> RenderGraph
  -> RhiCommandPlan
  -> RuntimeRenderTarget:
       HeadlessTexture
       ViewportTexture
       Surface
  -> RuntimeRenderFrameOutput
  -> Editor ViewportHost
  -> EditorUiRenderer DrawCommand::ViewportTextureSlot
```

优点：

```text
长期结构接近 UE / Bevy。
第一版落地接近 Unity 的 RenderTexture 嵌入方式。
AI 可以读取结构化 target / view / pass / report。
headless 自动化稳定。
后续加 Surface、Preview、ShadowMap、RenderTexture 不推翻第一版。
```

缺点：

```text
比直接 wgpu surface 多一层 target 抽象。
需要维护 RuntimeRenderTarget / RuntimeRenderFrameOutput / ViewportTextureDescriptor。
```

结论：

```text
采用。
```

## 最终规则

Runtime Viewport Rendering System 采用 D-min：

```text
RenderSceneState
  -> RuntimeRenderer
  -> RenderGraph
  -> RhiCommandPlan
  -> Backend Target:
       HeadlessTextureTarget
       ViewportTextureTarget
       SurfaceTarget
  -> RuntimeRenderFrameOutput
  -> Editor ViewportHost
  -> EditorUiRenderer ViewportTextureSlot
```

第一版必须支持：

```text
HeadlessTextureTarget
  默认自动化测试路径。

ViewportTextureTarget
  编辑器 Scene / Game View 嵌入路径。

SurfaceTarget
  只作为真实窗口 smoke gate，不作为默认 CI / headless 门禁。
```

第一版不做：

```text
完整真实 GPU resource lifetime。
完整 Render Thread。
完整 swapchain 多帧同步。
真实 D3D12 / Vulkan / Metal backend。
项目逻辑直接控制 RenderGraph / RHI。
Editor UI Renderer 直接渲染游戏世界。
Runtime Renderer 理解编辑器面板结构。
```

## 标准结构

### RuntimeRenderTarget

```text
RuntimeRenderTarget
  target_id
  target_kind:
    HeadlessTexture
    ViewportTexture
    Surface
  width
  height
  format
  color_space
```

规则：

```text
target_kind 决定输出目标类型，不决定渲染内容。
Scene View / Game View 的差异由 RenderViewState 决定。
RuntimeRenderer 只看 RenderViewState 和 RuntimeRenderTarget。
```

### RuntimeRenderFrameOutput

```text
RuntimeRenderFrameOutput
  frame_index
  view_id
  target_id
  target_kind
  texture_descriptor?
  surface_present_result?
  render_frame_report
  render_graph_report
  rhi_backend_report
  diagnostics
```

规则：

```text
HeadlessTexture / ViewportTexture 必须产生 texture_descriptor。
Surface 必须产生 surface_present_result。
报告字段必须足够让 AI 判断“有没有画出来、画到哪里、失败在哪一层”。
```

### ViewportTextureDescriptor

```text
ViewportTextureDescriptor
  texture_id
  target_id
  width
  height
  format
  color_space
  frame_index
  producer: RuntimeRenderer
```

规则：

```text
EditorUiRenderer 只消费 descriptor / slot。
EditorUiRenderer 不拥有 Runtime texture 的生成规则。
ViewportHost 负责把 Scene/Game panel 和 Runtime target 绑定。
```

### Editor ViewportSlot

```text
DrawCommand::ViewportTextureSlot
  rect
  scene_id
  frame
  texture_id?
```

规则：

```text
UI 只声明这里需要显示一个 viewport texture。
实际 texture 由 RuntimeRenderer 输出。
Scene View overlay / toolbar / gizmo hit region 仍归 Editor UI / Editor Tooling。
```

## Scene View 与 Game View

Scene View 和 Game View 不应共享同一个 view state：

```text
RenderSceneState
  -> RenderViewState(Game)
  -> RenderViewState(SceneView)
```

规则：

```text
Game View 使用项目 camera / runtime camera。
Scene View 使用 editor camera / editor view mode。
二者可共享同一个 RenderSceneState。
二者必须拥有独立 RenderTarget / target_id。
```

## 测试门禁

第一版最小测试：

```text
1. HeadlessTextureTarget 能生成 clear + present。
2. ViewportTextureTarget 能生成 texture descriptor。
3. EditorUiRenderer 能生成 ViewportTextureSlot。
4. ViewportHost 能把 slot 和 runtime frame output 关联。
5. SurfaceTarget 未启用真实窗口时只允许生成 smoke report，不阻塞 headless。
```

测试必须 headless 可执行。

真实窗口 / 真实 surface 只能作为本地 smoke gate，不允许成为默认自动化依赖。

## 与现有文档关系

```text
27-Renderer-MVP.md
  保留 Renderer MVP / report-first 规则。

55-RenderSceneState-RenderProxy-v1方案.md
  保留 RenderSceneState / RenderViewState / RenderProxy 规则。

59-真实WgpuBackend-RDG-RHI最小门禁方案.md
  保留 RDG-min / RHI-min / WgpuBackend-min 规则。

71-Runtime-Viewport-Rendering-System方案.md
  新增 Runtime 输出如何进入 Editor Viewport 的规则。
```

## 下一步施工

下一步施工只做 D-min：

```text
1. 补 RuntimeRenderTarget / RuntimeRenderFrameOutput 中缺失的 ViewportTexture 描述字段。
2. 让 RuntimeRenderer 对 ViewportTextureTarget 生成可读 descriptor。
3. 让 ViewportHost 持有 runtime frame output summary。
4. 让 EditorUiRenderer 的 ViewportTextureSlot 能引用 texture_id / frame。
5. 补 headless 单元测试。
```
