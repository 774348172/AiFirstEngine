# 138-Runtime Render Asset Production and Binding v1 方案

## 1. 本文解决什么问题

本文把原先较窄的：

```text
Real Texture Decode and WGPU Texture Upload v1
```

升级并改名为：

```text
Runtime Render Asset Production & Binding v1
```

原因是：我们现在缺的不是“贴图上传”这个单点，而是一个统一系统，把 runtime 资源生产成 renderer 可消费的资源，并完成 GPU resource / binding 的边界收敛。

第一版必须接入以下类型：

```text
Texture
Material
Mesh
AUI image
Font atlas
Particle texture
Render target
```

但第一版的“接入”不是完整商业级功能，而是每类都有长期正确的最小生产、绑定、诊断和测试入口。

## 2. 已有规则关系

本系统建立在这些已确认规则之上：

```text
06-资源系统架构.md
07-Build-Export-Pipeline.md
80-GPU-Resource-Pool-RenderResourceLifetime-v1方案.md
96-Sprite2D-Rendering-C-min方案.md
100-AUI-AI-First-Runtime-UI-System方案.md
102-AUI-Render-Extract-RuntimeRenderer接入方案.md
113-Native-Editor-FontSystem-v1方案.md
115-EngineRHI-Trait与RuntimeRenderer迁移方案.md
116-真实WgpuBackend完整v1方案.md
137-Real-Asset-GPU-Binding-and-Sprite2D-Product-Render-v1方案.md
```

137 已经打通结构链：

```text
RuntimeAsset -> RenderAssetPrepare -> RenderResourceManager -> RenderResourceHandle -> DrawSpriteTextured
```

但 137 没有完成真实 decoded pixels / mesh data / material params / font atlas / render target 到 backend resource 的统一生产与绑定。本文补的是这块。

## 3. 源码参考

```text
UE源码参考/Runtime-Render-Asset-Production-Binding源码参考.md
Unity源码参考/Runtime-Render-Asset-Production-Binding源码参考.md
Bevy源码参考/14-Runtime-Render-Asset-Production-Binding.md
Godot源码参考/11-Runtime-Render-Asset-Production-Binding源码参考.md
```

### UE

UE 的长期路线：

```text
UObject / UTexture / UMaterial / UStaticMesh
  -> render resource / proxy
  -> FRenderResource
  -> FRHIResource
  -> backend
```

可借鉴：

```text
Asset/Game 层不直接碰 GPU object。
FRenderResource 明确由 render thread 初始化和释放。
UI/font 动态纹理也走 render resource / RHI 纪律。
```

不照搬：

```text
完整材质编译、texture streaming、Nanite、Slate resource manager。
```

### Unity

Unity 用户层看到：

```text
Texture / Mesh / Material / SpriteRenderer / RenderTexture
```

真实 GPU 生命周期在 native engine 中。

可借鉴：

```text
项目层心智简单。
Renderer 组件只引用资源对象和参数，不管理 GPU binding。
```

不照搬：

```text
native 黑盒。我们的 AI-first 引擎必须有结构化 report。
```

### Bevy

Bevy 最接近我们的 Rust 路线：

```text
Main World Asset
  -> ExtractedAssets
  -> PrepareAssets
  -> RenderAssets<GpuAsset>
  -> RenderDevice / RenderQueue
```

可借鉴：

```text
RenderAsset trait。
SourceAsset -> GPU representation。
GpuImage = texture + texture_view + sampler。
FontAtlas 使用 Image 作为 atlas texture。
```

不照搬：

```text
完整 RenderWorld/Schedule 复杂度。
默认诊断不足以支持 AI 查错。
```

### Godot

Godot 路线：

```text
Resource / Node
  -> RenderingServer
  -> RID
  -> TextureStorage / MaterialStorage / MeshStorage
  -> RenderingDevice
```

可借鉴：

```text
统一 server/storage 入口。
Texture / Material / Mesh / Canvas / RenderTarget 都走 handle。
Render target 是 texture storage 下的特殊资源。
```

不照搬：

```text
把 RID 这种底层 handle 直接暴露给项目/AI。
```

## 4. 方案对比

### 方案 A：继续按类型各做一座桥

```text
TextureBridge
MaterialBridge
MeshBridge
AuiImageBridge
FontAtlasBridge
ParticleTextureBridge
RenderTargetBridge
```

优点：

```text
短期写起来直观。
```

缺点：

```text
会再次长出无数 bridge。
每类资源的错误报告、生命周期、fallback、upload 规则不一致。
AI 后期很难判断该查哪座桥。
```

结论：

```text
不采用。
```

### 方案 B：WgpuBackend 直接处理所有资产

```text
AssetRef / bytes
  -> WgpuBackend decode/upload/bind
```

优点：

```text
第一眼最短。
```

缺点：

```text
破坏 EngineRHI 抽象。
WGPU backend 会懂 RuntimePackage / AssetRef / Decode。
以后 D3D12/Vulkan/Metal 会重复实现一遍资源生产逻辑。
```

结论：

```text
不采用。
```

### 方案 C：统一 RuntimeRenderAssetProduction

```text
RuntimeAsset / runtime-created descriptor
  -> RuntimeRenderAssetProducer
  -> Typed RenderAsset
  -> RenderResourceManager
  -> RenderResourceHandle
  -> RenderBindingSet
  -> RHI command
  -> Backend
```

优点：

```text
接近 Bevy 的 RenderAsset prepare。
吸收 UE 的 RenderResource 生命周期边界。
吸收 Godot 的统一 handle/service 思路。
保留 Unity 式项目层简单心智。
AI 可以沿统一 report 查问题。
```

缺点：

```text
比单点 texture upload 多一个 production 层。
第一版必须认真定义类型边界，否则会变成新黑盒。
```

结论：

```text
采用。第一版做 C-min，但七类资源都必须接入。
```

## 5. 正式推荐方案

采用：

```text
Runtime Render Asset Production & Binding v1
```

统一链路：

```text
RuntimeAssetIndex / RuntimeAssetLoader
  -> RuntimeRenderAssetProducer
  -> RuntimeRenderAssetStore
  -> RenderResourceManager
  -> RenderResourceHandle
  -> RenderBindingSet
  -> RhiCommandPlan
  -> EngineRHI Backend
```

一句话解释：

```text
项目层只表达“我要用哪个资产做什么”；
Production 层把资产变成渲染可理解的 typed render asset；
ResourceManager 把 typed render asset 变成 GPU resource handle；
Binding 层把 resource handle 组织成 draw 可用的绑定；
Backend 只执行 RHI，不反向理解项目资产。
```

## 6. 系统边界

### 6.1 不属于本系统

```text
Asset Importer / Cook 规则
完整 Material Graph
完整 Shader Graph
完整 Particle System
完整 UI Layout / AUI authoring
完整 Font shaping / IME
完整 Mesh optimizer / Nanite / meshlet
完整 Texture streaming / virtual texture
```

### 6.2 属于本系统

```text
从 RuntimeAsset 或 runtime-created descriptor 生产 render-facing typed asset。
把 typed asset 交给 RenderResourceManager 创建或复用 RenderResourceHandle。
生成最小 RenderBindingSet。
输出 AI 可读的 RenderAssetProductionReport。
保证 Texture / Material / Mesh / AUI / Font / Particle / RenderTarget 不再各自长桥。
```

## 7. v1 类型结构

### 7.1 RuntimeRenderAssetKind

```text
RuntimeRenderAssetKind:
  Texture
  Material
  Mesh
  AuiImage
  FontAtlas
  ParticleTexture
  RenderTarget
```

### 7.2 RuntimeRenderAssetUsage

```text
RuntimeRenderAssetUsage:
  Sprite2DTexture
  MeshAlbedoTexture
  AuiImageTexture
  FontAtlasTexture
  ParticleTexture
  MaterialBinding
  MeshGeometry
  SurfaceRenderTarget
  OffscreenRenderTarget
```

关键规则：

```text
AUI image / Particle texture / Font atlas texture 不新建上传系统。
它们都是 Texture 类资源的不同 usage。
区别写在 usage 和 typed payload 中。
```

### 7.3 Typed RenderAsset

```text
TextureRenderAsset:
  width
  height
  format
  color_space
  mip_count
  pixel_data_or_cooked_blob
  sampler
  usage

MaterialRenderAsset:
  material_model
  shader_key
  scalar_params
  vector_params
  texture_slots
  blend_mode
  cull_mode

MeshRenderAsset:
  vertex_layout
  vertex_bytes
  index_bytes
  index_format
  submeshes
  bounds

AuiImageRenderAsset:
  texture_asset_ref
  nine_slice
  tint
  sampling
  usage = AuiImageTexture

FontAtlasRenderAsset:
  atlas_texture
  glyph_metadata
  font_key
  atlas_generation

ParticleTextureRenderAsset:
  texture_asset_ref
  sampler
  flipbook_layout
  usage = ParticleTexture

RenderTargetRenderAsset:
  target_kind
  width
  height
  format
  clear_color
  sample_count
  usage_flags
```

## 8. 生产器规则

统一入口：

```text
RuntimeRenderAssetProducer
```

允许有 typed producer：

```text
TextureProducer
MaterialProducer
MeshProducer
AuiImageProducer
FontAtlasProducer
ParticleTextureProducer
RenderTargetProducer
```

但它们必须注册到统一系统下，不能形成平级 bridge。

规则：

```text
Producer 只产出 backend-neutral typed asset。
Producer 不能创建 wgpu::Texture / wgpu::Buffer / bind group。
Producer 不能读取 ECS World。
Producer 可以读取 RuntimeAssetLoader、FontSystem atlas data、AUI image descriptor、RenderTarget descriptor。
```

## 9. Binding 规则

统一输出：

```text
RenderBindingSet
```

第一版只保留最小类型：

```text
RenderBindingSet:
  binding_id
  binding_kind
  resources: Vec<RenderResourceHandle>
  material_handle: Option<RenderResourceHandle>
  sampler
  fallback_used
  debug_label
```

绑定规则：

```text
DrawSpriteTextured 使用 Texture + Material binding。
DrawMeshBasic 使用 MeshBuffer + Material binding。
DrawAuiImage 使用 Texture + AUI material binding。
DrawFontGlyphs 使用 FontAtlas texture + glyph metadata。
DrawParticleBasic 使用 ParticleTexture + Particle material binding。
RenderTarget 作为 pass target / external target，不作为普通 sampled texture，除非显式声明 sampled usage。
```

## 10. RenderTarget 特别规则

Render target 不来自普通 RuntimeAsset 文件。它来自运行时 descriptor：

```text
SurfaceRenderTarget
OffscreenRenderTarget
ViewportRenderTarget
IntermediateRenderTarget
```

第一版支持：

```text
size
format
clear_color
sample_count = 1
usage: color_attachment / sampled / present
```

暂不支持：

```text
MSAA resolve chain
HDR pipeline
depth pyramid
VRS
temporary resource aliasing
complex render target pooling
```

规则：

```text
RenderGraph 可以引用 RenderTarget。
RenderGraph 不拥有长期 RenderTarget 生命周期。
RenderResourceManager 负责创建、复用、释放。
```

## 11. AI 友好诊断

统一报告：

```text
RenderAssetProductionReport:
  frame_index
  request_count
  produced_count
  reused_count
  failed_count
  uploaded_bytes
  fallback_count
  events
```

事件字段：

```text
RenderAssetProductionEvent:
  request_id
  kind
  usage
  asset_ref
  asset_id
  source_entity_id
  source_component
  producer
  stage
  code
  severity
  resource_handle
  fallback_used
  message
```

阶段：

```text
ResolveSource
LoadRuntimeAsset
Decode
ProduceTypedAsset
CreateResourceRequest
CreateOrReuseResource
CreateBinding
Ready
Failed
```

错误码第一版只保留：

```text
MissingAssetRef
MissingRuntimeAsset
UnsupportedFormat
DecodeFailed
InvalidDescriptor
UploadFailed
BindingFailed
FallbackUsed
Ready
```

规则：

```text
不要为每个资源类型发明一套错误码。
类型差异进入 kind / usage / producer / message。
```

## 12. 和其它引擎对比

| 项目 | UE | Unity | Bevy | Godot | 我们 |
|---|---|---|---|---|---|
| 项目层表达 | UObject/Component | Texture/Mesh/Material/Renderer | Asset Handle / Component | Resource/Node | AssetRef / Component / Descriptor |
| 生产层 | RenderResource/Proxy | native hidden | RenderAsset prepare | RenderingServer/Storage | RuntimeRenderAssetProducer |
| GPU 所有权 | RenderThread/RHI | native engine | RenderDevice/RenderQueue | RenderingDevice/RID | RenderResourceManager/EngineRHI |
| UI/Font | Slate resource/font atlas | UI native | FontAtlas -> Image -> GpuImage | CanvasItem/Font texture | AUI/FontAtlas producer |
| RenderTarget | RHI/RDG/Viewport | RenderTexture/native | RenderTarget/TextureView | TextureStorage render_target | RenderTargetRenderAsset |
| AI 可查性 | 中等，工具强 | 弱，黑盒 | 中等 | 中等 | 强，统一 report |
| 第一版复杂度 | 不可照搬 | 黑盒不可照搬 | 可借鉴 | 可借鉴 | C-min 统一系统 |

## 13. 为什么适合我们

AI 友好：

```text
所有资源类型都走同一条 report。
AI 不需要猜是 TextureBridge、FontBridge 还是 AuiBridge。
```

复杂项目能力：

```text
Texture / Material / Mesh / UI / Font / Particle / RenderTarget 的资源生命周期统一。
后续 streaming、batch、atlas、pipeline cache 可以在 producer/resource manager 扩展。
```

长期可维护：

```text
WGPU 不成为架构真相。
RenderResourceHandle 是 renderer/RHI 之间的稳定边界。
Typed producer 是可扩展点，但不是新系统。
```

简单度：

```text
只有一个生产系统、一个资源管理器、一个 binding 输出。
类型差异放在 typed payload 和 usage，不扩散到底层规则。
```

效率：

```text
v1 先保证资源复用和最小上传统计。
后续 upload budget、mesh allocator、texture streaming 可以按同一接口加入。
```

## 14. 第一版完成标准

第一版完成后必须能证明：

```text
Texture 可以从 runtime asset 生产为 RenderResourceHandle。
Material 可以生产最小 binding。
Mesh 可以生产 vertex/index buffer handle。
AUI image 通过同一 texture production 链路绘制。
Font atlas 通过 atlas texture + glyph metadata 绑定。
Particle texture 作为 texture usage 接入，不新开桥。
Render target 可以作为 surface/offscreen target 被 RenderGraph/RHI 引用。
缺失或失败资源能在统一 report 中定位。
Headless 测试能覆盖全部类型的 production/binding。
Real WGPU 测试只作为 feature-gated smoke，不阻塞默认 CI。
```

## 15. 下一步

如果确认本文方案，下一步再生成施工文档：

```text
施工文档/当前/138-当前可自动化施工文档-Runtime-Render-Asset-Production-and-Binding-v1.md
```

施工必须按模块推进：

```text
1. 核心数据结构和 report。
2. Texture + AUI image + Particle texture 共享链路。
3. Material 最小 binding。
4. Mesh buffer production。
5. Font atlas production。
6. Render target production。
7. RuntimeRenderer/RHI 接入。
8. Headless 全类型测试 + real-wgpu smoke。
```

每个模块完成后都要跑对应最小测试，再进入下一模块。
