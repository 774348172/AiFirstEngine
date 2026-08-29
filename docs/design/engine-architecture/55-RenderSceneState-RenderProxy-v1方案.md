# RenderSceneState / RenderProxy v1 方案

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

本文档定义 Game / ECS 世界同步到 Render 世界后，Render 侧第一版长期状态的最小结构。

它承接：

```text
50-RenderCommand-RenderSceneState方案.md
51-RenderDirtyTracker-RenderExtract-RenderCommand闭环方案.md
52-ECS-Storage-v1方案.md
施工文档/已完成/53-当前可自动化施工文档-ECS-Storage-v1.md
```

当前前置已经成立：

```text
ECS Storage v1 已完成最小 Archetype Table Storage。
World Write API 已能记录 DirtyRecord。
RenderSnapshot 已标记为 compatibility output。
正式 Game -> Render 同步必须走 RenderDirtyTracker / RenderExtract / RenderCommand / RenderSceneState。
```

## 1. 本文要解决什么

之前已经确认：

```text
Game / ECS 不直接给 Render Thread 读。
Render Thread 不直接读 ECS。
ECS 写入只产生 dirty。
RenderExtract 把 dirty 转成 RenderCommand。
RenderCommand 更新 RenderSceneState。
```

现在需要确认的是：

```text
RenderSceneState 里到底保存什么？
RenderProxy 最小字段是什么？
哪些字段不应该进入 v1？
AI 默认读取哪一层？
```

## 2. 成熟引擎源码和公开资料结论

### Unreal Engine

本地源码：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Renderer\Private\ScenePrivate.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Renderer\Private\RendererScene.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Renderer\Public\PrimitiveSceneInfo.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Public\PrimitiveSceneProxy.h
```

关键结构：

```text
FScene
FPrimitiveSceneInfo
FPrimitiveSceneProxy
PrimitiveComponentIdToInfoMap
PrimitiveSceneProxies
PrimitiveUpdates
```

公开资料：

```text
https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/Engine/FPrimitiveSceneProxy
https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/Renderer/FPrimitiveSceneInfo
```

UE 的核心设计：

```text
Game Thread 上的 UPrimitiveComponent 不直接被 Render Thread 读取。
UPrimitiveComponent 创建 FPrimitiveSceneProxy。
Renderer 内部用 FPrimitiveSceneInfo 保存单个 Primitive 的 render-side state。
FScene 保存所有 Primitive 的长期渲染状态。
Game Thread 变化通过 Add / Remove / Update command 同步到 Render Thread。
```

UE 的启发：

```text
Render 侧必须有长期状态。
RenderProxy / SceneInfo 是内部对象，不是用户层对象。
Render-side object 与 Game-side component 有稳定映射。
高性能渲染需要 RenderSceneState 能保存缓存、索引、bounds、proxy、dirty state。
```

不应照搬：

```text
FScene 的全部复杂度。
Nanite / Lumen / VSM / RayTracing / GPUScene 的所有字段。
大量历史兼容和 editor-only 分支。
```

### Unity

本地源码：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\RenderPipeline\ScriptableRenderContext.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\RenderPipeline\ScriptableRenderContext.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\RenderPipeline\CullingResults.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\RenderPipeline\RendererList.bindings.cs
```

关键公开结构：

```text
ScriptableRenderContext
CullingResults
RendererList
RendererListParams
DrawingSettings
FilteringSettings
CommandBuffer
```

公开资料：

```text
https://docs.unity3d.com/6000.4/Documentation/ScriptReference/Rendering.CullingResults.html
https://docs.unity3d.com/6000.0/Documentation/ScriptReference/Rendering.ScriptableRenderContext.DrawRenderers.html
```

Unity 的核心设计：

```text
用户在 C# 侧看不到完整 native render-side scene state。
SRP 暴露的是 culling result、renderer list、drawing settings 和 command submission。
真实 Renderer / GameObject / Transform 到 native renderer 的同步藏在 native 层。
```

Unity 的启发：

```text
用户心智应该简单。
RenderSceneState 不应该暴露给普通用户。
RenderFrameReport 应作为用户和 AI 的默认读取入口。
RendererFeatureBuilder / RDG / RHI 才对应 Unity SRP / CommandBuffer 的图形命令层。
```

Unity 的不足：

```text
底层黑箱强。
复杂视觉同步 bug 对 AI 不友好。
用户很难从 C# 层看到完整原因链。
```

### Bevy

本地源码：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_render\src\lib.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_render\src\extract_plugin.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_render\src\sync_world.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_render\src\view\mod.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_pbr\src\render\mesh.rs
```

关键结构：

```text
RenderApp
RenderWorld
ExtractSchedule
MainWorld
MainEntity
RenderEntity
RenderMeshInstances
ExtractedView
RenderVisibleEntities
```

公开资料：

```text
https://docs.rs/bevy/latest/bevy/render/struct.ExtractSchedule.html
https://docs.rs/bevy/latest/bevy/render/struct.Extract.html
```

Bevy 的核心设计：

```text
Main World 跑游戏逻辑。
Render World 跑渲染准备和渲染。
ExtractSchedule 从 MainWorld 提取数据到 RenderWorld。
RenderMeshInstances 等结构保存 render-side mesh instance state。
```

Bevy 的启发：

```text
Main World / Render World 分离是合理的。
Extract 阶段应该尽量短。
Render-side state 可以按渲染领域拆成资源和实例集合。
MainEntity / RenderEntity 映射对调试和 trace 很重要。
```

不应照搬：

```text
Bevy 的用户可见 ECS / plugin / schedule API。
让 AI 直接面对 RenderApp / ExtractSchedule / Query trait。
```

### Godot

公开资料：

```text
https://docs.godotengine.org/en/stable/classes/class_renderingserver.html
```

Godot 的核心设计：

```text
RenderingServer 是所有可见内容的后端 API。
Scene / Node 系统挂载到 RenderingServer 上显示。
RenderingServer 内部是不透明实现。
```

Godot 的启发：

```text
Render 侧可以有独立 server / state。
普通 Scene 层不需要知道底层实现。
```

Godot 对我们的警示：

```text
如果允许用户直接绕过 Scene / Entity 系统调用 RenderingServer，会丢失编辑器语义和 AI 可维护性。
本项目不应让普通项目逻辑直接创建底层 RenderProxy。
```

### O3DE Atom

公开资料：

```text
https://docs.o3de.org/docs/atom-guide/what-is-atom/
https://docs.o3de.org/docs/atom-guide/dev-guide/rpi/working-with-scene-and-rendering-pipeline/
```

O3DE Atom 的启发：

```text
现代渲染通常有 Scene / Feature Processor / Render Pipeline / RHI 分层。
Simulation 数据进入渲染后，应由渲染特性模块消费，而不是直接变成 GPU 命令。
```

对我们的取舍：

```text
学习 Scene / Feature / Pipeline 分层。
v1 不引入完整 Feature Processor 体系，避免层级过早膨胀。
```

## 3. 横向对比

| 引擎 | Render-side state | 用户是否直接面对 | 优点 | 缺点 |
|---|---|---|---|---|
| UE | FScene / FPrimitiveSceneInfo / FPrimitiveSceneProxy | 否 | 高性能，Game/Render 边界清楚，复杂渲染能力强 | 内部复杂，AI 直接读成本高 |
| Unity | native renderer state，C# 只见 CullingResults / RendererList / Context | 基本否 | 用户心智简单，SRP 入口稳定 | 底层黑箱，AI 排错证据弱 |
| Bevy | RenderWorld / Extracted data / RenderMeshInstances | 高级用户可见 | Rust ECS 分离清楚，Extract 模型清晰 | API 对普通用户和 AI 偏复杂 |
| Godot | RenderingServer opaque state | 可直接调用但通常不建议 | 后端边界干净 | 绕过 Scene 后编辑器语义弱 |
| O3DE Atom | Scene / Feature Processor / Render Pipeline | 专业开发者可见 | 现代数据驱动渲染 | 层级多，第一版照搬会重 |

## 4. 本项目正式方向

本项目采用：

```text
UE-like RenderSceneState / RenderProxy
  +
Bevy-like Main World -> Render World extract
  +
Unity-like 简单用户心智
  +
AI-readable RenderFrameReport / RuntimeTrace
```

正式链路：

```text
ECS Component Write
  -> DirtyRecord
  -> RenderDirtyTracker
  -> RenderExtract
  -> RenderCommand
  -> RenderSceneState
  -> RendererFeatureBuilder
  -> RDG
  -> RHI
```

用户 / AI 默认链路：

```text
用户自然语言 / AI Patch
  -> Project Schema / Component / Rule
  -> ECS Write
  -> RenderFrameReport / RuntimeTrace
```

正式规则：

```text
RenderSceneState 是引擎内部 Render 侧真相。
RenderProxy 是引擎内部渲染对象。
项目逻辑不能直接创建 / 修改 RenderProxy。
AI 不能直接生成 RenderProxy。
AI 默认读取 RenderFrameReport / RuntimeTrace。
只有深度排错和引擎开发时才展开 RenderCommand / RenderProxy。
```

## 5. RenderSceneState v1 最小结构

第一版结构：

```text
RenderSceneState
  frame_index
  proxies: Map<RenderProxyId, RenderProxy>
  entity_to_proxy: Map<RuntimeEntityId, RenderProxyId>
  source_to_proxy: Map<SourceEntityId, RenderProxyId>
  views: Map<RenderViewId, RenderViewState>
  diagnostics
```

字段含义：

```text
frame_index:
  当前 RenderSceneState 已应用到的帧。

proxies:
  Render 侧长期保存的渲染对象。

entity_to_proxy:
  RuntimeEntityId 到 RenderProxyId 的映射，用于高效更新。

source_to_proxy:
  SourceEntityId 到 RenderProxyId 的映射，用于 AI / Trace / Editor 可读。

views:
  Game View / Scene View / Shadow View / Reflection View 等视图状态。

diagnostics:
  资源缺失、命令非法、proxy 缺失等结构化诊断。
```

## 6. RenderProxy v1 最小字段与拆分规则

正式结构：

```text
RenderProxy
  common: RenderProxyCommon
  payload: RenderProxyPayload
```

```text
RenderProxyCommon
  proxy_id
  runtime_entity_id
  source_entity_id
  proxy_kind
  enabled
  visible
  layer
  transform
  previous_transform
  bounds
  render_flags
  version
```

```text
RenderProxyPayload
  Mesh(MeshPayload)
  Sprite(SpritePayload)
  SkinnedMesh(SkinnedMeshPayload)
  Light(LightPayload)
  Camera(CameraPayload)
  Particle(ParticlePayload)
  Instance(InstancePayload)
```

Common 字段含义：

```text
proxy_id:
  Render 侧稳定 id。

runtime_entity_id:
  对应 ECS 运行时 entity。

source_entity_id:
  对应项目源 entity，用于 AI / Trace / Editor。

proxy_kind:
  Mesh / Sprite / SkinnedMesh / Light / Camera / Particle / Instance。

enabled / visible:
  enabled 是对象是否参与 RenderSceneState。
  visible 是渲染可见性输入，不等于最终 camera culling 结果。

layer:
  渲染层 / 过滤层。
  layer 只负责渲染过滤，不等同于 Sprite2D 的 sortingLayer，也不等同于 UI Canvas layer。

transform:
  当前世界 transform。

previous_transform:
  motion vector、temporal、动画和 debug 需要。

bounds:
  culling、shadow、spatial index 需要。

render_flags:
  shadow caster / receiver / static / dynamic / transparent / editor selected 等少量 flags。

version:
  RenderProxy 每次更新递增，用于调试、缓存和 report。
```

Payload 字段原则：

```text
MeshPayload:
  mesh_ref
  material_refs
  submesh range / lod hint，v1 可选

SpritePayload:
  sprite_ref
  material_ref
  color
  flip
  sorting_layer
  order_in_layer
  sort_z，v1 可选

SkinnedMeshPayload:
  mesh_ref
  material_refs
  skeleton_ref
  skinning_buffer_ref，v1 可选

LightPayload:
  light_kind
  color
  intensity
  range
  shadow settings，v1 可选

CameraPayload:
  projection
  fov / ortho size
  near / far
  target / clear settings

ParticlePayload:
  emitter_ref
  material_ref
  simulation_space

InstancePayload:
  mesh_ref
  material_refs
  instance_count
  instance_data_ref
```

第一版 payload 只保留能完成 Scene View / Game View 最小显示的字段。  
高级字段可以先作为可选字段或后续扩展，不在 v1 一次性填满。

### 6.1.1 Sprite / UI / 透明排序边界

SpritePayload 的排序字段只服务 Sprite2D 可见顺序：

```text
Sprite2D draw order =
  render_domain(Sprite2D)
  sorting_layer
  order_in_layer
  sort_z / transform.z
  stable_proxy_id
```

正式边界：

```text
SpritePayload 不承载完整透明深度排序策略。
透明深度排序属于 Renderer Core / Camera / Material pass。
摄像机距离排序属于 Renderer Core / Camera 模式。
材质批处理排序属于 Renderer 内部优化。
多 Canvas / Panel / ZOrder 属于 Runtime UI，不属于 SpritePayload。
Renderer 可以在相同 Sprite2D 排序桶内做 material / atlas 合批，但不能为了合批改变可见顺序。
```

### 6.2 为什么采用 common + typed payload

正式决策：

```text
RenderProxy 采用统一 common 外壳 + typed payload。
```

不采用纯通用大对象：

```text
RenderProxy
  mesh_ref?
  sprite_ref?
  light_color?
  camera_fov?
  particle_emitter?
  ...
```

原因：

```text
字段会无限膨胀。
不同 proxy 类型的非法字段组合太多。
验证层和 AI 很难判断哪些字段有效。
```

不采用完全分裂的多个顶层 proxy：

```text
MeshProxy
SpriteProxy
LightProxy
CameraProxy
ParticleProxy
InstanceProxy
```

原因：

```text
Trace / Report / Validation 会重复实现很多套。
RenderCommand 会分裂成大量 AddMeshProxy / AddLightProxy / UpdateCameraProxy。
AI 排查“这个 Entity 为什么没显示”时，需要先知道它落在哪个 proxy map。
复杂项目后期规则容易碎。
```

采用 common + typed payload 的原因：

```text
AI 默认读取 common 即可回答大多数问题。
Trace / Report / Validation 有统一入口。
RendererFeatureBuilder 仍然可以读取强类型 payload。
RenderCommand 外层保持统一，payload 内层保持类型安全。
后端以后可以把 payload 拆到 typed storage 优化，不破坏外层协议。
```

### 6.3 与 UE 的差别

UE 更接近：

```text
FPrimitiveSceneProxy 基类
  -> FStaticMeshSceneProxy
  -> FSkeletalMeshSceneProxy
  -> FInstancedStaticMeshSceneProxy
  -> ...

FScene / FPrimitiveSceneInfo 统一管理这些 proxy。
```

UE 使用 C++ 多态和大量虚函数：

```text
GetDynamicMeshElements
DrawStaticElements
GetTypeHash
```

本项目采用：

```text
RenderProxyCommon
RenderProxyPayload enum
RendererFeatureBuilder match typed payload
```

差别：

```text
UE 偏专家 C++ 多态对象体系。
本项目偏 AI 可读的数据结构体系。
UE 的 proxy 本身带很多行为。
本项目的 proxy 尽量只保存 render-side state，行为放到 RendererFeatureBuilder / RDG / RHI。
```

正式原则：

```text
学习 UE 的统一 FScene + typed proxy 思想。
不照搬 UE 的复杂虚函数代理对象。
```

### 6.4 访问规则

正式规则：

```text
AI / Trace / Report 默认只读取 RenderProxyCommon。
RendererFeatureBuilder 才读取 RenderProxyPayload。
项目逻辑不能直接写 RenderProxy。
AI 不能直接生成 RenderProxy patch。
RenderCommand 外层保持统一，payload 内层 typed。
```

用户修改视觉时：

```text
用户自然语言
  -> AI 修改 Render-facing Component / Material / Asset / Preset
  -> ECS Write API 产生 dirty
  -> RenderExtract 生成 RenderCommand
  -> RenderCommand 更新 RenderProxy
```

禁止：

```text
用户自然语言
  -> AI 直接改 RenderProxy
```

## 7. RenderViewState v1 最小字段

第一版采用最小持久 View Registry + 每帧临时 View Data。

核心边界：

```text
RenderSceneState = 场景里有什么。
RenderViewState = 从哪里看、怎么看、渲染到哪里。
RenderFrameViewData = 这一帧这个视图看到了什么。
```

RenderSceneState 可以保存最小 view_registry，但不能把每个 view 的完整可见性结果塞回长期场景状态。

第一版 RenderSceneState 结构补充：

```text
RenderSceneState
  proxies: Map<RenderProxyId, RenderProxy>
  entity_to_proxy: Map<RuntimeEntityId, RenderProxyId>
  source_to_proxy: Map<SourceEntityId, RenderProxyId>
  view_registry: Map<RenderViewId, RenderViewState>
```

第一版 RenderViewState 结构：

```text
RenderViewState
  view_id
  source_entity_id optional
  view_kind
  viewport
  target
  view_matrix
  projection_matrix
  clear_color
  layer_mask optional
  version
```

字段含义：

```text
view_kind:
  Game / SceneView / Preview / Shadow / Reflection。

source_entity_id:
  Game Camera 可以来自 ECS Camera Entity。
  Editor Scene View / Preview View 可以没有 source_entity_id。

target:
  Window / ViewportTexture / RenderTexture / ShadowMap / PreviewTexture。
```

第一版 RenderFrameViewData 结构：

```text
RenderFrameViewData
  view_id
  visible_proxy_ids
  culling_result_summary
  render_phase_summary
  diagnostics
```

隔离规则：

```text
visible_proxy_ids / culling result / render phase 属于 RenderFrameViewData，不属于长期 RenderSceneState 真相。
RenderFrameViewData 每帧生成，可以复用内存，但不能作为项目逻辑输入。
Editor Scene View 不能直接修改 Game Camera 的 RenderViewState。
Preview / Shadow / Reflection 都是 View，不是独立 Scene。
RenderCommand 更新 RenderProxy；View 状态由 Camera / EditorView 提取生成或更新。
AI 默认看 RenderFrameReport 的 per-view 摘要，不直接看完整 culling 列表。
```

与其他引擎的对应关系：

```text
UE:
  FScene 类似 RenderSceneState。
  FSceneViewFamily / FViewInfo 类似每次渲染的 View 输入和 View 数据。
  UE 不把每个 view 的完整 culling / phase 结果写回 FScene。

Unity:
  Camera / ScriptableRenderContext 围绕单个或多个 Camera 执行 cull / draw / submit。
  内部状态较黑箱，用户心智简单，但 AI 证据链弱。

Bevy:
  ExtractedCamera / ExtractedView 表示提取后的 view。
  ViewSortedRenderPhases / ViewBinnedRenderPhases 按 view 存放每帧渲染队列。
  本项目吸收 per-view frame data，但第一版不照搬完整 RenderWorld ECS。
```

## 8. v1 不进入 RenderSceneState 的内容

第一版不放：

```text
GPU buffer handle
RHI texture handle
RDG pass
shader permutation
真实 draw command
material compiled pipeline
shadow atlas allocation
Nanite / Lumen / VSM / ray tracing 专用字段
完整 culling result
每个 view 的完整 visible_proxy_ids 长期缓存
每个 view 的完整 render phase 列表长期缓存
完整 render graph resource lifetime
```

原因：

```text
这些属于 Renderer Backend / RDG / RHI / 高级渲染 feature。
如果过早放进 RenderSceneState，会让 RenderSceneState 变成另一个巨型 renderer。
```

## 9. AI 读取规则

AI 默认读取：

```text
RenderFrameReport
RuntimeTrace
SourceEntityId
Changed entity summary
diagnostics
```

AI 深度排错才读取：

```text
RenderCommand
RenderProxy
RenderSceneState diff
```

正式规则：

```text
RenderSceneState / RenderProxy 是可下钻证据层，不是默认主视图。
普通用户不需要理解 RenderProxy。
AI 不直接生成 RenderProxy patch。
AI 修改视觉时，必须修改 Render-facing Component / Material / Asset / Preset，再由引擎生成 RenderCommand。
```

## 10. RenderCommand 更新 RenderSceneState 规则

RenderProxy 拆分方式已经确认：

```text
统一 RenderProxy common 外壳 + typed payload。
```

### 10.1 路线来源和取舍

正式路线：

```text
UE-like typed RenderCommandQueue
+ Bevy-like Extract discipline
+ O3DE-like Render Feature boundary
+ AI-readable diagnostics / RenderFrameReport
```

对其他引擎的取舍：

```text
UE:
  学习 Game Thread -> Render Thread 的增量命令同步。
  学习 FScene / PrimitiveSceneInfo / PrimitiveSceneProxy 这类渲染侧长期状态。
  学习 typed update payload + dirty flags。
  不照搬 UE 大量 C++ 虚函数代理和专家型内部调试复杂度。

Unity:
  学习用户心智简单，用户不需要理解底层同步队列。
  不采用 Unity-like 黑箱同步，因为 AI-first 需要可追溯 diagnostics / report。
  不把 SRP CommandBuffer 和本项目 RenderCommand 混为一层。

Bevy:
  学习 MainWorld -> ExtractSchedule -> RenderWorld 的清晰边界。
  学习 Changed / Removed 驱动的提取纪律。
  第一版不完整照搬 RenderWorld ECS，避免多一套 World 同步规则增加复杂度。

O3DE Atom:
  学习 Scene + FeatureProcessor / Feature boundary。
  第一版不让每个 FeatureProcessor 直接拥有一套上层可见同步协议。
```

路线判断：

```text
RenderCommandQueue 是内部同步队列，不是用户 / AI 主心智。
AI / 用户仍然只改 Render-facing Component / Material / Asset / Preset。
RenderExtract 负责把 dirty 变成 RenderCommand。
RenderCommandQueue 负责 collect / stable_sort / normalize / merge。
RenderSceneState.apply_batch 负责确定性应用。
RendererFeatureBuilder 负责从 RenderSceneState 生成 RDG 输入。
```

RenderCommand 更新 RenderSceneState 的边界：

```text
RenderCommand 是 Game / ECS 侧变化同步到 Render 侧的命令。
RenderSceneState.apply_batch(commands) 是 Render 侧更新长期状态的唯一入口。
RenderCommand 不是 RDG / RHI / GPU CommandBuffer。
RenderCommand 只更新 RenderProxy / RenderSceneState，不直接产生 draw call。
RendererFeatureBuilder 后续再从 RenderSceneState 构建 RDG 输入。
```

第一版 RenderCommand 字段：

```text
command_id
frame_index
source_entity_id
runtime_entity_id
proxy_id optional
command_type
payload
source_dirty_type
source_trace
```

第一版 command_type 只保留：

```text
AddProxy
RemoveProxy
UpdateRenderState
UpdateTransform
UpdateDynamicData
UpdateInstanceData
```

每类命令的作用：

```text
AddProxy:
  创建 RenderProxy。
  写入 RenderProxyCommon + typed payload。
  建立 entity_to_proxy / source_to_proxy 映射。

RemoveProxy:
  删除 RenderProxy。
  删除 entity_to_proxy / source_to_proxy 映射。
  记录 removed event。

UpdateTransform:
  只更新 common.transform / previous_transform / bounds / version。
  不修改 payload。

UpdateRenderState:
  更新 enabled / visible / layer / flags。
  必要时替换整个 typed payload。

UpdateDynamicData:
  更新动态 payload 字段。
  例如 material params / light intensity / camera fov。

UpdateInstanceData:
  更新 instance payload。
  例如 instance count / instance data ref。
```

同帧合并和异常规则：

```text
AddProxy + RemoveProxy:
  保留生命周期顺序，不做自由合并。

UpdateTransform:
  同 proxy 同帧多次更新，最终 transform 生效。
  previous_transform 保留第一次更新前的值。

UpdateRenderState:
  同字段 last value wins。
  payload kind 改变必须走 UpdateRenderState，不能走 UpdateDynamicData。

UpdateDynamicData:
  同字段 last value wins。
  不同字段可以合并。

RemoveProxy then Update:
  丢弃 update，并产生 diagnostic。

Update missing proxy:
  产生 diagnostic，不自动创建 proxy。

AddProxy existing:
  相同 payload kind 可降级为 UpdateRenderState。
  不同 payload kind 产生 conflict diagnostic。
```

第一版执行流程：

```text
RenderExtract workers
  -> ThreadLocalCommandBuffer
  -> RenderCommandQueue.collect()
  -> stable_sort()
  -> normalize()
  -> merge()
  -> RenderSceneState.apply_batch(commands)
  -> diagnostics
  -> RenderFrameReport summary
```

第一版 RenderCommandQueue 结构：

```text
RenderCommandQueue
  frame_index
  pending_commands: Vec<RenderCommand>
  diagnostics: Vec<RenderCommandDiagnostic>
```

第一版 RenderCommand 结构：

```text
RenderCommand
  command_id
  frame_index
  source_entity_id
  runtime_entity_id
  proxy_id optional
  command_type
  payload_kind
  payload
  sort_key
  source_dirty_type
  trace_id optional
```

第一版 sort_key：

```text
frame_index
lifecycle_order
runtime_entity_id
command_type_order
command_id
```

第一版 command_type_order：

```text
RemoveProxy
AddProxy
UpdateRenderState
UpdateTransform
UpdateDynamicData
UpdateInstanceData
```

特殊规则：

```text
同一个 proxy 内，生命周期命令不能被普通排序破坏。
AddProxy / RemoveProxy 必须保留必要发生顺序。
普通 Update 命令才允许 last value wins 合并。
```

### 10.2 normalize / merge 精确规则

第一版采用 UE-like Object Command Slot Merge。
不要把 RenderCommandQueue 当成普通 append-only 数组。

流程：

```text
Raw RenderCommand[]
  -> stable_sort
  -> 按 proxy/entity 聚合成 ObjectCommandSlot
  -> 每个 slot 内 normalize / merge
  -> 输出 MergedRenderCommand[]
  -> RenderSceneState.apply_batch
```

ObjectCommandSlot 第一版结构：

```text
ObjectCommandSlot
  runtime_entity_id
  proxy_id optional
  existed_at_frame_start
  lifecycle: None / Add / Remove / Recreate / NoOp
  render_state_payload optional
  transform_payload optional
  dynamic_data_payload optional
  instance_data_payload optional
  diagnostics[]
```

核心规则：

```text
1. 同一个 proxy/entity 的命令先进同一个 ObjectCommandSlot。
2. 生命周期命令 AddProxy / RemoveProxy 优先级最高。
3. 普通 Update 只在对象最终仍存在时生效。
4. 同类 payload 后写覆盖前写。
5. 不同类 payload 可以并存。
6. Transform 多次更新只保留最终 transform。
7. previous_transform 保留帧开始时或第一次更新前的值。
8. Update missing proxy 不自动创建，记录 diagnostic。
9. Remove missing proxy 不崩溃，记录 diagnostic。
10. Add existing proxy：相同 payload kind 可转 UpdateRenderState，不同 payload kind 记录 conflict。
```

对象帧开始不存在：

```text
Add + Update:
  合并成 AddProxy。
  Update 内容并入 AddProxy payload。

Add + Remove:
  输出 NoOp。
  记录 covered_by_remove。

Update only:
  不自动创建 proxy。
  记录 diagnostic: missing_proxy。

Remove only:
  不崩溃。
  记录 diagnostic: missing_proxy_remove。
```

对象帧开始存在：

```text
Update 多次:
  同字段 last value wins。
  不同字段合并。

Update + Remove:
  输出 RemoveProxy。
  Update 被覆盖。
  记录 covered_updates。

Remove + Add:
  输出 Recreate。
  实际命令为 RemoveProxy + AddProxy。

Add only:
  视为 Add existing。
  相同 payload kind 转 UpdateRenderState。
  不同 payload kind 记录 conflict。
```

正式边界：

```text
normalize / merge 是引擎内部规则，不是用户 / AI 的主心智。
AI 不需要判断命令合并。
AI 默认只读取 RenderFrameReport 的 merged / covered / failed 摘要。
深度排错时才展开 ObjectCommandSlot / MergedRenderCommand。
```

### 10.3 diagnostics / RenderFrameReport 最小错误字段

diagnostics 第一版只回答：

```text
某条 RenderCommand 为什么没生效。
某条 RenderCommand 为什么被合并。
某条 RenderCommand 为什么失败。
失败影响了哪个 entity / proxy / resource。
```

diagnostics 不是完整日志系统，也不是完整 Frame Debugger。

第一版分两层：

```text
RenderCommandDiagnostic:
  记录单个命令 / 对象的问题。

RenderFrameReport:
  汇总一帧发生了什么。
```

RenderCommandDiagnostic 最小字段：

```text
diagnostic_id
frame_index
severity: Info / Warning / Error
code
stage: Collect / Sort / Normalize / Merge / Apply
runtime_entity_id optional
source_entity_id optional
proxy_id optional
command_id optional
command_type optional
payload_kind optional
result: Applied / Merged / Covered / Skipped / Failed
reason_code
trace_id optional
```

第一版默认不记录：

```text
长字符串 reason
完整 payload dump
完整 RenderProxy dump
完整 from/to
完整 source map
```

这些只允许 Evidence 模式打开。

RenderFrameReport 最小字段：

```text
frame_index
report_level
counters
changed_entities
render_events
trace_refs
```

counters：

```text
raw_command_count
merged_command_count
applied_command_count
covered_command_count
skipped_command_count
failed_command_count
warning_count
error_count
missing_proxy_count
missing_resource_count
fallback_count
```

changed_entities：

```text
runtime_entity_id
source_entity_id optional
proxy_id optional
change_kind
result
reason_code optional
trace_id optional
```

render_events：

```text
severity
event_code
stage
runtime_entity_id optional
proxy_id optional
resource_id optional
command_type optional
reason_code
trace_id optional
```

trace_refs：

```text
trace_id
source_system optional
source_patch optional
```

第一版 reason_code / event_code：

```text
missing_proxy
missing_resource
payload_kind_conflict
update_after_remove
add_existing_proxy
remove_missing_proxy
covered_by_remove
covered_by_noop
merged_last_value_wins
invalid_payload
apply_failed
fallback_used
```

模式裁剪：

```text
Release:
  默认不生成完整 RenderFrameReport。
  只保留 counters + 严重 error 摘要。

Profile:
  counters + cost。
  不记录 entity 细节。

Editor / Debug:
  counters + changed_entities + render_events。

Evidence:
  才记录 payload 摘要、from/to、source map、ObjectCommandSlot 展开。
```

正式规则：

```text
RenderCommandDiagnostic 是命令级证据。
RenderFrameReport 是帧级摘要。
AI 默认读取 RenderFrameReport，不直接读取完整 diagnostics 列表。
只有 Debug / Evidence 才能展开 diagnostics、ObjectCommandSlot、payload 摘要。
Release 不能因为 diagnostics 复制完整 RenderSceneState 或 payload。
```

正式规则：

```text
RenderSceneState 只能由 RenderCommand 更新。
RenderCommandQueue 必须先 normalize / merge，再 apply_batch。
apply_batch 必须确定性执行，不能依赖 worker 完成顺序。
更新失败必须进入 diagnostics / RenderFrameReport，不能静默吞掉。
RenderProxy 的 common 字段可以被通用 apply 逻辑处理。
RenderProxy 的 typed payload 只能由对应 command payload 和 RendererFeatureBuilder 理解。
```

## 11. 下一步待决策

RenderCommandQueue 主路线、normalize / merge 精确规则、diagnostics / RenderFrameReport 最小字段、多 View / 多 Camera / Editor Scene View 状态隔离已经确认。

RenderCommand / RenderSceneState v1 CPU-side sync 已完成，对应记录：

```text
施工文档/已完成/56-当前可自动化施工文档-RenderCommand-RenderSceneState-v1.md
阶段完成记录/2026-06-23-RenderCommand-RenderSceneState-v1/
```

下一步不在本文档继续追加施工细节，应进入：

```text
EngineHostLoop / Runtime FrameLoop 正式一帧闭环。
RendererFeatureBuilder：RenderSceneState -> Renderer 输入。
Minimal Renderer v1。
```

