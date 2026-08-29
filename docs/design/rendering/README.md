# Current Status Notice

本文档顶部的 Rust / RHI / RDG / WgpuBackend 长期渲染路线仍可作为参考。文中若出现 React / Electron / npm / TypeScript 原型层相关描述，应按历史记录理解，不代表当前正式入口。

# 渲染技术路线设计

## 结论

本项目长期渲染技术路线确认如下：

```text
底层语言：Rust
底层参照：Unreal-like RHI + RDG + Renderer Feature System
渲染底座：Engine RHI Interface
首个验证后端：WgpuBackend
长期高性能后端：D3D12Backend / VulkanBackend / MetalBackend
渲染架构：自研 RDG / Render Dependency Graph
渲染功能层：Renderer Feature Layer
材质系统：Material Graph / Shader IR
数据组织：schema-first
视觉复用：preset-first
平台适配：Quality Profile / Feature Level
AI 能力：AI Render Director + patch / validation / downgrade loop
```

目标不是复制 Unity 或 Unreal 的人类编辑器工作流，而是建设一套同时满足以下条件的 AI 原生渲染系统：

```text
强渲染
AI 可控
AI 可验证
多平台可降级
可导出桌面和移动端游戏
```

## 设计原则

### 1. 高级渲染能力必须结构化

AI 不应该直接随意修改底层 shader、render pass 或 GPU pipeline。AI 应该生成结构化渲染意图，然后由引擎编译、验证和降级。

推荐输入形式：

```json
{
  "op": "apply_visual_style",
  "scene": "scene-shooter",
  "style": "neon_sci_fi",
  "qualityTarget": "mobile_high_60fps"
}
```

不推荐输入形式：

```text
AI 直接写任意 shader / 任意 render pass / 任意 GPU pipeline
```

### 2. 渲染数据 schema-first

所有渲染相关数据都必须有明确 schema：

```text
Material Schema
Material Graph Schema
Lighting Preset Schema
PostFX Preset Schema
Render Feature Schema
Quality Profile Schema
Platform Capability Schema
```

schema 的作用：

```text
限制 AI 输出范围
检查字段完整性
检查参数类型
检查平台兼容性
支持 patch diff / rollback / review
```

### 3. 复杂效果 graph 化

复杂材质和后处理不应暴露为一堆散乱参数，而应组织成 graph。

示例：

```text
Texture -> NormalMap
Noise -> ColorRamp -> Emissive
Fresnel -> Rim Light
BaseColor + Metallic + Roughness -> PBR Material
```

AI 生成节点图，引擎负责：

```text
验证 graph 合法性
编译 Shader IR / WGSL 验证后端
做平台能力检查
生成 shader variants
失败时回传错误
```

### 4. 高级视觉 preset-first

中小型游戏需要高质量默认效果，而不是每次从零调参数。

预设类型：

```text
Visual Style Preset
Lighting Preset
Material Preset
PostFX Preset
VFX Preset
Quality Preset
```

AI 更适合选择、组合和微调 preset，而不是无约束地生成任意渲染参数。

### 5. 平台可降级

任何高级渲染能力都必须能根据平台自动降级。

示例：

```json
{
  "name": "mobile_high_60fps",
  "features": {
    "shadows": "medium",
    "bloom": true,
    "ssao": false,
    "ssr": false,
    "textureMaxSize": 1024,
    "maxLights": 8,
    "renderScale": 0.85
  }
}
```

AI 生成视觉方案后，引擎必须进行：

```text
schema validate
shader compile validate
render budget validate
platform capability validate
fallback generation
```

## 总体架构

```text
renderer/
  ai_render_layer/
    render_director
    render_intent
    visual_spec
    feature_contract
    visual_patch

  schema/
    material_schema
    lighting_schema
    shadow_schema
    gi_schema
    postfx_schema
    quality_profile_schema
    platform_capability_schema

  feature/
    mesh_renderer_feature
    material_feature
    lighting_feature
    shadow_feature
    gi_reflection_feature
    postfx_feature
    vfx_feature
    virtual_geometry_feature_future
    virtual_shadow_feature_future
    dynamic_gi_feature_future

  rdg/
    render_dependency_graph
    pass_node
    resource_node
    resource_lifetime
    dependency_resolver
    transient_resource_allocator
    barrier_planner
    async_compute_scheduler
    pass_culling
    graph_validation
    graph_debugger

  backend/
    engine_rhi_interface
    backend_capabilities
    surface
    device
    queue
    wgpu_backend_first
    d3d12_backend_future
    vulkan_backend_future
    metal_backend_future

  material/
    material_schema
    material_graph
    shader_ir
    wgsl_compiler_for_wgpu_backend
    hlsl_compiler_future
    msl_compiler_future
    spirv_compiler_future
    shader_variant_cache

  lighting/
    light_schema
    lighting_presets
    shadow_system
    reflection_probes
    light_probes

  postfx/
    bloom
    tone_mapping
    color_grading
    ssao
    motion_blur

  quality/
    quality_profiles
    feature_levels
    platform_capabilities
    downgrade_rules

  validation/
    schema_validation
    shader_validation
    render_budget_check
    platform_check

```

## 核心模块

### Unreal-like 底层路线与 AI-native 上层

本项目渲染底层工程路线参考 Unreal 的核心分层思想：

```text
Renderer Feature Layer
  -> RDG / Render Dependency Graph
  -> RHI / Render Hardware Interface
  -> D3D12 / Vulkan / Metal / WgpuBackend
```

但本项目不复制 Unreal 的人类专家工作流。Unreal 的渲染能力主要由美术、TA 和图形程序员通过编辑器参数、材质图、C++、项目设置和平台配置控制；本项目的控制层必须 AI-native。

正式规则：RDG 前面只能有一个直接生产者，即 Renderer Feature Builder。AI Intent、Visual Spec、Render Patch、Feature Contract、Material Graph、Preset、Quality Profile、Capability Profile、Validation / Fallback 都是 Builder 的结构化输入，不是逐层翻译的线性管线。

```text
AI / Editor / Material / Quality / Platform Inputs
  -> Renderer Feature Builder
  -> RDG / Render Dependency Graph
  -> RHI / Render Hardware Interface
  -> Backend
```

Builder 输入：

```text
AI Render Intent
Visual Spec
AI Render Patch
Render Feature Contract
Material Graph / Material Preset
Lighting / Shadow / GI / PostFX Preset
Quality Profile
Capability Profile
Validation / Fallback Policy
Scene / View / Entity render data
```

禁止把渲染生成流程实现成：

```text
Intent -> Patch -> Contract -> Variant -> Material -> RDG
```

这种线性翻译链路会造成层级过多、信息丢失、调试困难和后期维护成本上升。

正确实现方式是：

```text
All source inputs
  -> Renderer Feature Builder 一次性收集、验证、选择 variant
  -> 输出 RDG pass / resource declaration
```

正式边界：

```text
底层工程路线对齐 Unreal：RHI + RDG + Renderer Feature System
上层控制路线区别于 Unreal：AI Schema / Graph / Preset / Patch / Validation
RDG 前直接生产者只有 Renderer Feature Builder
长期高级能力参考 Unreal：Nanite-like / Lumen-like / Virtual Shadow Map-like / Substrate-like / World Partition-like
```

这些高级能力是长期方向，不进入第一阶段 MVP。第一阶段只保留架构位置和数据边界。

### Game 到 Render 同步：增量命令与渲染侧状态

正式长期路线采用 Unreal-like 的 Game Thread / Render Thread 隔离思想：

```text
Game / ECS / Scene
  -> RenderExtract
  -> RenderCommand Queue
  -> Render Thread
  -> RenderSceneState / RenderProxy
  -> Renderer Feature Builder
  -> RDG
  -> RHI
  -> Backend / GPU
```

对应 Unreal 的核心链路：

```text
UPrimitiveComponent
  -> FPrimitiveSceneProxy / FPrimitiveSceneInfo
  -> ENQUEUE_RENDER_COMMAND
  -> FScene
  -> FSceneRenderer
  -> RDG
  -> RHI
```

本项目不采用完整场景 `RenderSnapshot` 双缓冲作为长期底层战略。原因：

```text
完整场景双缓冲按场景总量付费。
增量 RenderCommand 按变化量付费。
大型项目中一帧通常只有部分对象变化，完整复制渲染世界会浪费 CPU、内存和带宽。
```

正式规则：

```text
Game / ECS 是玩法世界。
RenderSceneState 是渲染世界。
Render Thread 不直接读 Gameplay ECS。
RenderExtract 是唯一桥梁。
RenderExtract 生成增量 RenderCommand，不生成完整渲染世界副本。
Render Thread 消费 RenderCommand 并维护 RenderProxy / RenderSceneState。
Renderer Feature Builder 从 RenderSceneState 读取渲染输入。
```

RenderCommand 标准类别：

```text
AddProxy
RemoveProxy
UpdateRenderState
UpdateTransform
UpdateDynamicData
UpdateInstanceData
```

RenderProxy 标准定位：

```text
RenderProxy 是 Entity 在 Render Thread 的长期渲染代理。
RenderProxy 持有渲染需要的数据和资源句柄。
项目逻辑不能直接操作 RenderProxy。
项目逻辑只能修改 ECS Component。
RenderExtract 根据 dirty state 生成 RenderCommand。
```

Render Dirty 责任边界：

```text
普通 Project Component 不声明 render dirty。
Render Dirty 只由标准 Render-facing Component 或高级 Render Extension Component 产生。
Health / Inventory / Skill / Buff / Quest / AIState 等普通项目组件不直接同步到 Render Thread。
Transform / Visibility / MeshRenderer / MaterialBinding / Light / Camera / ParticleRenderer / InstanceRenderer 等标准视觉组件由引擎维护 dirty 映射。
项目逻辑如果要影响视觉，必须写入标准视觉组件，或通过 VisualBinding 输出到标准视觉组件。
高级插件 / Native Renderer Module 可以声明 dirty metadata，但这不是普通自然语言项目逻辑能力。
```

第一版标准 Render-facing Component 清单：

```text
Transform
Visibility
MeshRenderer
SpriteRenderer
SkinnedMeshRenderer
MaterialBinding
Light
Camera
ParticleRenderer
InstanceRenderer
```

字段到 Dirty 类型的第一版映射：

```text
Transform.localPosition / localRotation / localScale / parent -> Transform
Visibility.visible / layerVisible / editorVisible -> DynamicData
MeshRenderer.meshRef / materialSlots / shadow / renderLayer / cullingMode -> RenderState
SpriteRenderer.spriteRef / atlasRef / materialSlot / sortingLayer / orderInLayer / sortZ -> RenderState
SpriteRenderer.color / flipX / flipY -> DynamicData
SkinnedMeshRenderer.meshRef / skeletonRef / materialSlots -> RenderState
SkinnedMeshRenderer.poseBuffer / blendShapeWeights / skinningData -> DynamicData
MaterialBinding.materialRef / shaderVariant -> RenderState
MaterialBinding.params / textureBinding / tint / emission -> DynamicData
Light.lightType / shadowMode -> RenderState
Light.color / intensity / range / angle / temperature -> DynamicData
Camera.projectionType / renderTargetKind -> RenderState
Camera.fov / orthoSize / near / far / clearColor / viewportRect -> DynamicData
ParticleRenderer.effectAsset / renderMode / materialSlots -> RenderState
ParticleRenderer.particleBuffer / aliveRange / simulationOutput -> InstanceData
InstanceRenderer.meshRef / materialSlots / lodPolicy -> RenderState
InstanceRenderer.instanceTransforms / instanceColors / customData -> InstanceData
```

Sprite2D / UI / Renderer Core 排序归属：

```text
Sprite2D 可见顺序:
  由 SpriteRenderer.sortingLayer / orderInLayer / sortZ / stable_entity_id 决定。
  第一版采用确定性排序，不引入复杂透明深度规则。

透明深度排序:
  属于 Renderer Core / Camera / Material pass。
  后续由 Renderer Feature Builder / RDG / Camera policy 支持。

摄像机距离排序:
  属于 Renderer Core / Camera 模式。
  2D 正交 Sprite 第一版默认不使用 camera-distance sort。

材质批处理排序:
  属于 Renderer 内部优化。
  不暴露为项目规则，不由 AI 直接配置。
  只能在不改变可见排序结果的前提下，在相同排序桶内合并 draw。

多 Canvas / Panel / ZOrder:
  属于 Runtime UI / HUD 系统。
  不参与 Sprite2D draw order。
```

AI 友好规则：

```text
AI 不直接生成底层 RenderCommand / RDG / RHI。
AI 生成 RenderIntent / Visual Patch / Material Graph / Preset / Quality Policy。
引擎验证后生成底层 RenderCommand。
每条 RenderCommand 必须携带 source_entity / source_component / source_system / source_ai_patch / reason / frame_index。
RenderFrameReport 提供给 AI 和用户查看本帧变化、降级、警告、成本和 trace。
```

RenderFrameReport 定位：

```text
RenderFrameReport 是调试和 AI 理解用的摘要。
它不是完整 RenderSceneState 副本。
它不能成为 Render Thread 的底层输入。
```

允许局部双缓冲：

```text
ViewUniform
CameraData
FrameConstants
LightList
VisibleList
SkinningPalette
ParticleBuffer
UI DrawList
```

禁止作为长期架构双缓冲：

```text
完整 Scene
完整 ECS World
完整 RenderProxy 世界
完整 Asset 状态
```

RenderSnapshot 废弃规则：

```text
RenderSnapshot 在架构上标记为 Deprecated / Transition Only。
新渲染能力禁止继续依赖 RenderSnapshot。
不为 RenderSnapshot 设计长期迁移路线。
正式渲染闭环直接采用 RenderDirtyTracker -> RenderExtract -> RenderCommand -> RenderSceneState。
历史代码或阶段记录中出现 RenderSnapshot，只代表旧 MVP 兼容输出。
```

### Engine RHI Interface 与 Backend

渲染架构必须按 Engine RHI Interface 设计，而不是让 Render Graph、Material Graph 或 AI 渲染数据直接依赖 wgpu。

WgpuBackend 只是首个验证 backend，用来快速跑通 Render Graph、Material Graph、Quality Profile 和 AI Render Director。长期高性能路线是 Native D3D12 / Vulkan / Metal backend。

目标平台：

```text
Windows: D3D12Backend / VulkanBackend，WgpuBackend 可用于早期验证
macOS: MetalBackend，WgpuBackend 可用于早期验证
iOS: MetalBackend，WgpuBackend 可用于早期验证
Android: VulkanBackend，WgpuBackend 可用于早期验证
Web: WebGPUBackend / WgpuBackend，后期单独评估
```

正式规则：

```text
AI Schema / Graph / Preset
  -> Renderer Feature Layer
  -> RDG / Render Dependency Graph
  -> Material Graph / Shader IR
  -> Engine RHI Interface
  -> Backend:
       WgpuBackend，前期功能验证
       D3D12Backend，后期 Windows 高性能
       VulkanBackend，后期 Android / Linux / Windows 高性能
       MetalBackend，后期 Apple 平台高性能
```

约束：

```text
上层渲染系统不能出现 wgpu 类型
Renderer Feature Layer / RDG 不能依赖 wgpu
Material Graph 不能只绑定 WGSL
Shader IR 必须允许后续生成 WGSL / HLSL / MSL / SPIR-V 路线
资源生命周期由自研 RDG 管理
平台能力由 Feature Level / Quality Profile 管理
```

RHI 不是 Feature API，也不是通用 GPU API。RHI 不应该暴露 `drawParticles` 这类功能语义，也不应该向上层暴露 `createBuffer / createTexture / CommandEncoder / RenderPassEncoder` 这类类似 wgpu / WebGPU 的通用接口。

RHI 的正式定位：

```text
RDG 编译后命令计划的后端执行契约
```

Engine RHI 标准结构：

```text
EngineRhiInterfaceV1

public surface:
  queryCapabilities(): RhiCapabilities
  beginFrame(frameContext): RhiFrame
  executeCompiledFrame(compiledFrame): RhiExecutionReport
  endFrame(frame): void
  destroyDeferredResources(): void

compiled frame:
  resources: RhiResourceDesc[]
  pipelines: RhiPipelineDesc[]
  resourceSets: RhiResourceSetDesc[]
  commands: RhiCommand[]
  barriers: RhiBarrier[]
  debug: RhiDebugInfo
```

Backend 内部可以有：

```text
createBuffer
createTexture
createPipeline
command encoder
render pass encoder
D3D12 / Vulkan / Metal / wgpu native handles
```

但这些不能出现在 RHI 公开接口，也不能被 Renderer Feature、AI 或项目逻辑直接拿到。

正式规则：

```text
AI 不能调用 RHI
Renderer Feature 不能直接调用 RHI
RDG Compiler 是 RhiCompiledFrame 的唯一生产者
RHI 不接收 Feature Intent
RHI 不暴露通用 CommandEncoder API
RHI resource 在 execute 阶段创建 / 导入
Backend native handle 不能逃出 backend
backend-specific extension 必须通过 capability-gated command 表达
```

RHI 两色粒子验证样例见：

```text
框架设计/验证Demo/rhi-two-color-particles/README.md
```

### RDG / Render Dependency Graph

RDG 用来组织渲染 pass、资源依赖、临时资源生命周期、资源状态转换和执行调度。它不是 RHI，也不是具体渲染功能；它是 Renderer Feature Layer 和 RHI 之间的帧级调度与验证层。

典型 pass：

```text
Depth Prepass
Shadow Pass
GBuffer 或 Forward+ Pass
Lighting Pass
Transparent Pass
PostFX Pass
UI Pass
```

作用：

```text
显式表达渲染步骤
方便 AI 理解渲染结构
自动检查资源依赖
支持按平台裁剪 pass
支持性能分析和降级
管理临时资源生命周期
规划资源状态转换 / barrier
支持 pass culling
支持异步 compute 调度
支持 RDG debug / insights
```

RDG 标准结构：

```text
RdgGraph
  id
  resources
  passes
  extractedResources
  frameRoots
  debugOptions

RdgResource
  id
  type
    texture | buffer | accelerationStructure
  lifetime
    imported | transient | history | extracted
  desc
  usage
  debugName

RdgPass
  id
  name
  sourceFeature
  sourceVariant
  executionDomain
    raster | compute | raytrace | copy | present
  queue
    graphics | compute | copy
  flags
    allowCulling
    allowAsyncCompute
    hasSideEffect
  resourceBindings

ResourceBinding
  resource
  access
    read | write | readWrite
  usage
    sampled | renderTarget | depthRead | depthWrite | storage | copySrc | copyDst | present

CompilerOutputs
  dependencies
  culledPasses
  barrierPlan
  lifetime
  schedule
  rhiCommandPlan
```

RDG 编译流程：

```text
Renderer Feature Variant
  -> RDG Pass / Resource Declaration
  -> Frame Root / Composite Pass
  -> Dependency Build
  -> Pass Culling
  -> Resource Lifetime
  -> Barrier Plan
  -> Queue Schedule
  -> RHI Command Plan
```

正式规则：

```text
RDG 只接收 Renderer Feature Layer 已选 Variant 后生成的 pass/resource 声明
RDG resource 不等于 RHI resource
RHI resource 只能在 backend execute 阶段出现
pass 依赖由 resourceBindings 自动推导
pass culling 必须以 frameRoots / extractedResources / hasSideEffect 为根
Shadow / GI / Bloom 等 feature 输出必须被 Composite Pass 或 Frame Root 消费，否则会被合法裁剪
```

RDG 标准结构验证样例见：

```text
框架设计/验证Demo/rdg-standard-structure/README.md
```

### Renderer Feature Builder / Renderer Feature Layer

Renderer Feature Layer 承载真正的渲染功能。Renderer Feature Builder 是 RDG 的唯一直接生产者，负责收集 AI、编辑器、材质、质量、平台能力、场景和视图数据，然后一次性生成 RDG pass / resource declaration。

Renderer Feature Builder 的定位：

```text
不是 AI 层
不是 Material Graph 层
不是 Quality Profile 层
不是 RHI 层
不是线性翻译管线

它是 RDG 前的唯一汇总、验证、variant 选择和 RDG 声明生成层。
```

示例：

```text
Mesh Renderer Feature
Material Feature
Lighting Feature
Shadow Feature
GI / Reflection Feature
PostFX Feature
VFX Feature
Virtual Geometry Feature，未来
Virtual Shadow Feature，未来
Dynamic GI Feature，未来
```

每个 Feature 必须定义：

```text
identity
  id
  version
  featureType
  ownerLayer

intent
  intentSchema
  userVisibleControls
  aiPatchSchema

resources
  inputs
  outputs
  internalResources
  historyResources

scheduling
  pipelineStage
  framePhase
  orderingConstraints
  asyncPolicy

variants
  implementationVariants
  variantSelectionPolicy
  fallbackChain

constraints
  capabilityRequirements
  qualityBudget
  memoryBudget
  platformDenyList

rdg
  rdgTemplates
  resourceLifetime
  barrierHints
  debugViews

validation
  validationRules
  traceSchema
  testCases
```

平台差异不能散落在 Scene、Material、AI Patch 或项目逻辑里，必须集中在 Feature Contract、Capability、Variant、Quality Profile 和 Fallback Chain 中。

Feature 选择必须发生在 RDG 生成之前，但选择过程必须在 Renderer Feature Builder 内部完成，不能拆成多层线性翻译：

```text
Inputs:
  AI Render Intent
  Render Patch
  Feature Contract
  Material Graph / Preset
  Quality Profile
  Capability Profile
  Scene / View data

Renderer Feature Builder:
  validate inputs
  resolve quality and fallback
  select implementation variant
  bind material / shader variants
  emit RDG pass / resource declaration

RDG:
  compile frame graph
  cull unused passes
  resolve resource lifetime
  emit RHI command plan
```

原因：

```text
同一个视觉意图在不同平台会展开成完全不同的 RDG pass
如果先生成 RDG 再做平台降级，会导致大量图重写
Capability 只能说明能不能跑，Quality Budget 决定该不该跑
Fallback Chain 必须显式，否则低端平台可能没有合法实现
```

Shadow Feature 验证样例见：

```text
框架设计/验证Demo/shadow-feature-contract/README.md
```

Render Feature Contract 标准结构验证样例见：

```text
框架设计/验证Demo/render-feature-contract-standard/README.md
```

### Material Graph / Material IR / Shader IR

材质系统不直接让 AI 写 WGSL / HLSL / MSL，也不能把 Material Graph 直接等同于某一个 shader 文件。

参考 Unreal 的 Material / Shader 路线，本项目材质必须拆成：

```text
Material Graph = AI / 用户可编辑节点图
Material IR = 材质语义真相
Shader IR = 后端 shader 生成输入
Shader Variant = pass / platform / quality / usage 派生产物
Backend Codegen = WGSL / HLSL / MSL / SPIR-V 等目标代码生成
```

流程：

```text
Material Graph
  -> Material Validation
  -> Material IR
  -> Pass / Platform / Quality / Usage Variant Selection
  -> Shader IR Variant
  -> Backend Shader Codegen
       WGSL for WgpuBackend
       HLSL for D3D12Backend
       MSL for MetalBackend
       SPIR-V / GLSL route for VulkanBackend，后续确认
  -> Shader Compile Validation
  -> Shader Variant Cache
```

Material Asset 标准结构：

```text
MaterialAsset
  id
  version
  domain
    surface | particle | post_process | ui | decal
  blendMode
    opaque | masked | translucent | additive
  shadingModel
    unlit | pbr_lit | thin_translucent | water_lit | layered_substrate
  usage
    mesh_static | mesh_skinned | particle_billboard | terrain | ui
  qualityPolicy
  featureRequirements

MaterialGraph
  nodes
  typedPins
  resourceSlots
  uniformSlots
  attributes
  outputs

MaterialIR
  surfaceModel
  lightingModel
  blendState
  renderState
  requiredPasses
  featureFlags

ShaderIR
  entryPoints
  stageInputs
  resourceBindings
  uniformLayout
  operations
  outputs
  debugSourceMap

ShaderVariant
  pass
  platform
  quality
  vertexFactory
  defines
  fallback
```

早期只支持受控节点：

```text
TextureSample
Color
Multiply
Add
NormalMap
Fresnel
Noise
ColorRamp
PBRSurface
UnlitSurface
Emissive
LayerBlend
ScreenRefraction
Time
ScrollUV
SubstrateSlab，先作为高级材质预留
```

正式规则：

```text
WGSL / HLSL / MSL 不是源数据
Material Graph 不能只绑定 WgpuBackend
Material Domain / Blend Mode / Shading Model / Usage 必须是一等字段
同一 Material 必须能生成多个 Shader Variant
移动端 / Web 可通过 Material IR 降级，而不是让 AI 改 shader 代码
Substrate-like 分层材质必须走 composition IR，不能只靠算术节点拼凑
```

Material Graph / Shader IR 验证样例见：

```text
框架设计/验证Demo/material-graph-shader-ir/README.md
```

### Capability / Quality / Fallback 统一系统

跨平台降级不能只依靠一个 `quality = low / medium / high` 枚举。  
本项目需要统一的 Capability / Quality / Policy / Fallback Resolver，服务 Renderer Feature、Material、RDG、RHI、PostFX 和 Particles。

推荐内置：

```text
mobile_low_30fps
mobile_high_60fps
desktop_medium
desktop_high
web_balanced
```

标准结构：

```text
CapabilityProfile
  platform
  backend
  rendererTier
  hardwareClass
  capabilities
  limits

QualityProfile
  targetFps
  budgets
  scalabilityGroups

ProjectPolicy
  priorities
  requiredFallback
  forbidden
  platformOverrides

FeatureRequest
  layer
  group
  intent
  variants

Variant
  id
  requires
  cost
  profiles
  fallbackReason

FallbackResolver
  capabilityCheck
  policyCheck
  budgetCheck
  fallbackSelection
  traceReport
```

Quality Profile 控制预算和质量目标：

```text
阴影质量
最大动态光数量
后处理开关
贴图最大尺寸
材质复杂度
粒子数量
render scale
LOD 策略
反射策略
```

正式选择流程：

```text
Feature / Material / RHI Request
  -> Capability Check
  -> Project Policy Check
  -> Quality Budget Check
  -> Fallback Selection
  -> Validation / Trace Report
```

正式规则：

```text
CapabilityProfile 描述设备 / 后端能做什么
QualityProfile 描述当前目标质量和预算
ProjectPolicy 描述项目允许什么、禁止什么、优先级是什么
FallbackResolver 负责从候选 Variant 里选一个可运行、可解释、可验证的结果
支持不代表启用，启用必须通过预算和策略检查
降级结果必须能追踪原因，供 AI 和用户审查
```

Capability / Quality / Fallback 验证样例见：

```text
框架设计/验证Demo/quality-capability-fallback/README.md
```

### AI Render Director

AI Render Director 是本项目差异化重点。

职责：

```text
根据自然语言生成视觉风格 patch
自动布光
自动选择材质 preset
自动配置后处理
自动生成移动端降级方案
自动检查性能预算
将验证错误回传给 AI 修复
```

示例用户输入：

```text
把这个打飞机游戏变成高质量霓虹科幻风格，手机上也要流畅。
```

AI Render Director 输出：

```text
AiRenderPatch
  patchId
  patchType
  schemaVersion
  sourceIntent
  scope
  operations
  constraints
  validationGates
  trace
```

Patch operation 标准结构：

```text
PatchOperation
  opId
  type
    set_visual_style
    apply_lighting_preset
    set_feature_intent
    set_quality_targets
    apply_material_preset
    set_postfx_intent
    optimize_assets
  target
  payload
  sourceMap
```

AI Render Patch 正式规则：

```text
AI 不直接生成 RDG / RHI / shader 代码
AI 生成结构化意图和操作
每条 operation 必须有 opId、target、payload、sourceMap
Patch 必须声明 sourceIntent、scope、targetProfiles、constraints、validationGates、trace
引擎负责验证、解析、降级和执行
```

验证输出必须分成两类：

```text
userExplanations
  给用户看的自然语言解释

aiRepairHints
  给 AI 自动修复用的结构化建议
```

非法配置必须阻止。例如：

```text
手机端强制开启光追反射，不允许降级
```

应输出：

```text
status: failed
riskLevel: blocked
error.code: fallback.required_but_forbidden
aiRepairHint: enable_fallback_or_limit_to_desktop
```

AI Render Patch 验证样例见：

```text
框架设计/验证Demo/ai-render-patch/README.md
```

## 阶段路线

### 阶段 1：基础商业画面

目标：达到中小型游戏可接受画面，不追求 AAA。

功能：

```text
Forward / Forward+ renderer
PBR material
Directional / Point / Spot light
Shadow map
IBL
Skybox
Tone mapping
Bloom
Color grading
glTF loading
KTX2 / Basis texture
```

### 阶段 2：高级表现

目标：明显区别于玩具级引擎。

功能：

```text
SSAO / GTAO
SSR
Reflection probes
Light probes
GPU particles
Decals
Animation / skinning
TAA / FXAA
FSR-like upscaling interface
LOD
Occlusion culling
```

### 阶段 3：高端实验

目标：长期挑战更高端视觉能力。

功能：

```text
GPU-driven rendering
Clustered rendering
Virtual shadow maps
Sparse / virtual textures
Meshlet / cluster culling
Realtime GI
Large world streaming
```

## 与 Unity / Unreal 的区别

Unity / Unreal 的渲染能力成熟，但工作流主要面向人类美术和程序员。

本项目渲染系统的目标是：

```text
底层工程路线重点参考 Unreal 的 RHI / RDG / Renderer Feature System
学习 Unreal 的高级渲染能力边界
不照搬 Unreal 的人类专家工作流
用 schema / graph / preset / patch / validation 组织渲染工程
```

核心差异：

```text
Unreal: 人类专家驱动 Renderer Feature / RDG / RHI
本项目: AI 生成结构化 Render Intent / Feature Contract / Preset / Patch，再由 Renderer Feature / RDG / RHI 执行
```

## 当前项目迁移建议

当前项目状态：

```text
编辑器：React / Electron
预览：Three.js
项目数据：TypeScript types + JSON
导出：Electron runtime MVP
```

建议迁移：

```text
短期：继续用 Three.js 做编辑器预览
中期：Rust + Engine RHI + WgpuBackend 做导出端功能验证
长期：编辑器预览与 runtime 共享 schema / material / lighting / postfx 数据
```

下一步优先级：

```text
1. 固定 GameProject / Scene / Entity / Component schema
2. 设计 Material Schema 和 Quality Profile Schema
3. 设计 Visual Patch Protocol
4. 做 Rust + Engine RHI + WgpuBackend runtime MVP
5. 让编辑器导出同一份 schema 给 runtime
```

## 最终判断

本项目选择这条路线的原因：

```text
底层工程路线对齐 Unreal，保证长期高级渲染上限
Engine RHI 保证长期 Native D3D12 / Vulkan / Metal 高性能路线，WgpuBackend 只作为首个功能验证后端
Rust 提供长期安全和工程可控性
RDG 支持高级渲染组织、资源生命周期、barrier、pass culling 和调试
Renderer Feature Layer 承载 Shadow / GI / PostFX / VFX / Virtual Geometry 等高级能力
Material Graph / Shader IR 支持 AI 生成、验证和多后端 shader 编译
schema-first 支持 AI 验证和自动修复
Quality Profile 支持桌面和移动端降级
AI Render Director 构成区别于 Unity / Unreal 的核心卖点
```

最终目标：

```text
不要做“AI 调 Unity 参数”的引擎。
要做“AI 能理解、生成、验证、降级并发布渲染工程”的引擎。
```



