# RenderDirtyTracker / RenderExtract / RenderCommand 闭环方案

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

本文定义本项目替代 RenderSnapshot 的正式 Game 到 Render 同步闭环。

参考来源：

```text
UE 源码参考：
  框架设计/UE源码参考/GameToRender-Dirty-EndOfFrame-PrimitiveUpdates.md
  框架设计/UE源码参考/RenderCommand-RenderSceneState.md

Unity 参考：
  框架设计/Unity源码参考/PlayerLoop-SRP-CommandBuffer-RenderSync.md

本项目目标：
  AI 友好
  复杂项目可维护
  尽量少规则
  兼顾效率
```

## 1. 结论

正式闭环：

```text
ECS Component Write
  -> RenderDirtyTracker
  -> RenderExtract at EndOfSimulation
  -> RenderCommandQueue
  -> Render Thread
  -> RenderSceneState / RenderProxy
  -> Renderer Feature Builder
  -> RDG
  -> RHI
```

架构废弃：

```text
RenderSnapshot = Deprecated / Transition Only
新渲染能力禁止依赖 RenderSnapshot
不为 RenderSnapshot 设计迁移路线
```

## 2. 核心原则

```text
Game / ECS 不直接给 Render Thread 读。
Render Thread 不读 ECS。
项目逻辑不直接写 RenderCommand。
AI 不直接写 RenderCommand。
ECS 写入只产生 dirty。
RenderExtract 把 dirty 转成 RenderCommand。
RenderSceneState 是 Render Thread 长期状态。
RenderFrameReport 是 AI / 用户可读摘要。
```

## 3. 为什么不照搬 UE

UE 的方案很强，但复杂：

```text
MarkRenderStateDirty
MarkRenderTransformDirty
MarkRenderDynamicDataDirty
MarkRenderInstancesDirty
UWorld EndOfFrame arrays
DoDeferredRenderUpdates_Concurrent
FPrimitiveTransformUpdater
FScenePrimitiveUpdates
GPUScene / VSM / Culling / RayTracing change set
```

本项目第一版只保留必要结构：

```text
DirtyTracker
Extract
CommandQueue
SceneState
FrameReport
```

也就是说：

```text
学习 UE 的增量同步和 EndOfFrame 合并。
不搬 UE 的全部专家复杂度。
```

## 3.1 RenderCommand 对应其它引擎中的什么

RenderCommand 对应的是成熟引擎里 Game 世界向 Render 世界提交变化的桥。

对应关系：

| 引擎 | 对应流程 | 本项目对应 |
|---|---|---|
| Unreal | MarkRenderStateDirty / MarkRenderTransformDirty / DoDeferredRenderUpdates_Concurrent / ENQUEUE_RENDER_COMMAND / FScene | DirtyTracker / RenderExtract / RenderCommandQueue / RenderSceneState |
| Unity | GameObject / Renderer 内部 native 同步 + CommandBuffer / ScriptableRenderContext | Render-facing Component dirty + RenderExtract + Renderer Feature Builder |
| Godot | SceneTree 到 RenderingServer 的命令提交 | ECS 到 RenderSceneState 的命令提交 |
| Bevy | Main World Extract 到 Render World | ECS World RenderExtract 到 RenderSceneState |

本项目最接近：

```text
UE 的 Game Thread -> Render Thread 增量同步
  +
Bevy 的 Main World -> Render World extract 思想
  +
AI 可读的 RenderFrameReport / Trace
```

## 3.2 为什么必须有 RenderCommand

RenderCommand 的目的不是让用户多理解一层，而是让引擎内部有一条清晰、可并行、可追踪的 Game 到 Render 边界。

如果没有 RenderCommand，只有两种路线：

```text
Render Thread 直接读 ECS：
  线程安全差。
  Render 和 Gameplay 状态互相穿透。
  多线程渲染和复杂 viewport 会变难。

每帧复制完整 RenderSnapshot：
  大场景成本高。
  变化少时也按完整场景付费。
  会重新走回已废弃的完整场景 snapshot 路线。
```

RenderCommand 的正式价值：

```text
线程安全：
  Render Thread 不直接读 ECS。

性能：
  只提交本帧变化，按变化量付费。

复杂项目可维护：
  Game 状态和 Render 状态分离。
  RenderSceneState 是渲染长期状态，不污染 Gameplay ECS。

AI 友好：
  每条画面变化都能追溯到 Entity / Component / System / Patch。
  用户问“为什么没动 / 没显示 / 没换材质”时，AI 有证据链可查。
```

例子：

```text
Transform.localPosition 从 (0,0,0) 改成 (0,1,0)
  -> ECS Write API 写 Transform
  -> RenderDirtyTracker 标记 Transform dirty
  -> RenderExtract 生成 UpdateTransform
  -> RenderCommandQueue 提交到 Render Thread
  -> RenderSceneState 更新对应 RenderProxy
  -> RDG / RHI 使用新 transform 绘制
```

正式规则：

```text
RenderCommand 必须存在。
RenderCommand 是引擎内部协议，不是项目层 API。
项目逻辑不能直接生成 RenderCommand。
AI 不能直接生成 RenderCommand。
AI 默认读取 RenderFrameReport / Trace，需要深查时再展开单条 RenderCommand。
```

## 3.3 AI / 用户读取层级

RenderCommand 不作为普通用户和 AI 的默认主视图。

正式读取层级：

```text
普通用户：
  只看 RenderFrameReport 的自然语言解释。

AI 默认：
  读取 RenderFrameReport + RuntimeTrace 摘要。

AI 深度排错 / 引擎开发者：
  才展开单条 RenderCommand。
```

这样设计的原因：

```text
RenderCommand 是底层协议，默认暴露会增加理解成本。
RenderFrameReport 是面向问题解释的摘要，更适合 AI 和用户。
RuntimeTrace 负责把渲染问题和项目规则、资源、Patch 串起来。
RenderCommand 作为最后证据层保留，避免摘要不足时无法查 bug。
```

编辑器行为：

```text
RuntimeTrace / RenderFrameReport 面板默认显示摘要。
单条 changed entity 可以展开 source trace。
只有点击“底层命令”或 AI 深度排错时，才显示 RenderCommand。
```

构建裁剪：

```text
Editor / Debug 构建保留 RenderCommand debug metadata。
Profile 构建保留统计字段和有限 source id。
Release 构建可以裁剪 source_patch / reason string / source_map 等字符串元数据。
Release 构建不能裁剪运行时必要字段。
```

## 4. Dirty 类型

第一版 dirty taxonomy 只保留 4 类，贴近 UE 的底层分类方式：

```text
RenderState
Transform
DynamicData
InstanceData
```

含义：

```text
RenderState：
  需要重建、重挂或重新组织 RenderProxy 的变化。
  例如 mesh 替换、material slot 结构变化、renderable 类型变化、visibility 影响 proxy 存在性、light/camera 类型变化。

Transform：
  local/world transform、bounds、previous transform 相关变化。

DynamicData：
  不需要重建 proxy，但需要更新 proxy 内部动态数据。
  例如 material 参数、texture binding、light intensity、camera fov、animation pose、custom render data、visibility value。

InstanceData：
  instancing transform、per-instance color、per-instance custom data。
```

暂不做：

```text
CullingBounds / CullingLogic / GPUState 这类 UE 底层细分。
DistanceField / RayTracing / VirtualShadowMap 专用 dirty。
每个材质参数一个 dirty 类型。
每个 renderer feature 一个 dirty 类型。
Material / Visibility / LightCamera 这类用户语义 dirty 类型。
```

长期扩展方式：

```text
第一版 dirty 类型稳定后，RenderExtract 内部可把它 lower 成更细的 backend dirty。
上层 dirty taxonomy 不轻易膨胀。
高级渲染需要更多细分时，优先在 RenderSceneState / Renderer Backend 内部处理，不污染项目层 dirty taxonomy。
```

为什么不用 6 类：

```text
Material / Visibility / LightCamera 是用户语义，不是底层同步成本。
按语义拆 dirty 会导致后期继续增加 Shadow / VFX / Animation / Reflection 等类型。
AI 需要判断的底层规则会越来越多。
底层 dirty 应按更新代价分类，而不是按用户概念分类。
```

## 5. Dirty 由谁标记

核心规则：

```text
普通 Project Component 不声明、不维护、不直接触发 Render Dirty。
Render Dirty 只由标准 Render-facing Component 或高级 Render Extension Component 产生。
项目逻辑不手动调用底层 mark_render_dirty。
AI 不生成 mark_render_dirty 代码。
```

这条规则参考 UE 的组件边界：

```text
UActorComponent：
  普通组件基类，不一定参与渲染。

USceneComponent：
  Transform / Visibility / 层级变化由组件内部标记 render dirty。

UPrimitiveComponent：
  Mesh / Material / Light 等可渲染对象通过组件 API 标记 render dirty，并创建 SceneProxy。
```

本项目采用 ECS，不照搬 UE 的 OO 继承，但照搬责任边界：

```text
Project Component：
  Health / Inventory / Skill / Buff / Quest / AIState 等普通项目数据。
  不声明 render dirty。
  不直接同步到 Render Thread。

Render-facing Component：
  Transform / Visibility / MeshRenderer / SpriteRenderer / SkinnedMeshRenderer /
  MaterialBinding / Light / Camera / ParticleRenderer / InstanceRenderer。
  由引擎标准库提供字段到 dirty type 的映射。
  ECS Write API 写入这些组件时自动进入 RenderDirtyTracker。

VisualBinding：
  把项目数据变化转换成标准视觉组件变化。
  例如 Health.hp < 30% 时，写 MaterialBinding.tint = red。

Render Extension Component：
  高级插件 / Native Renderer Module 使用。
  可以声明 dirty metadata。
  需要通过验证层检查。
  不是普通自然语言项目逻辑能力。
```

示例：

```text
写普通 Health Component
  -> 不标记 Render Dirty

HealthWarningVisualBinding 读取 Health.hp
  -> 写 MaterialBinding.tint
  -> 自动标记 DynamicData dirty

写 Transform Component
  -> 自动标记 Transform dirty

写 Mesh / Renderable Component
  -> 自动标记 RenderState dirty

写 Material Slot / Renderable Type / Shadow Flags
  -> 自动标记 RenderState dirty

写 Material Param / Texture Binding / Light Param / Camera Param
  -> 自动标记 DynamicData dirty

写 Visibility
  -> 如果影响 proxy 是否存在，标记 RenderState dirty
  -> 如果只影响 visible flag，标记 DynamicData dirty

写 Instance Buffer / Instance Transform / Instance Custom Data
  -> 自动标记 InstanceData dirty
```

为什么不让所有用户组件声明 DirtyMap：

```text
用户要理解底层渲染同步，复杂度会上升。
AI 生成组件时容易漏标、错标 dirty。
项目后期组件数量变多，DirtyMap 会变成维护灾难。
UE 也没有要求所有 Gameplay Component 字段都知道自己是否影响渲染。
普通数据组件只表达玩法数据，渲染同步只发生在标准渲染组件边界。
```

允许的例外：

```text
引擎内部 Native Renderer Module 可以显式标记 dirty。
低层资源系统可以在 asset reload / hot update 后显式标记相关 dirty。
项目 DSL / IR 不暴露底层 dirty 标记接口。
```

## 6. 标准 Render-facing Component 清单

第一版标准 Render-facing Component 只保留 9 个：

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

这套清单参考 UE / Unity / Godot / Bevy 的共同边界：

```text
UE：
  SceneComponent 承担 Transform / Visibility / 层级。
  PrimitiveComponent / MeshComponent / LightComponent / CameraComponent 承担渲染对象。

Unity：
  Transform 是空间基础。
  Renderer / MeshRenderer / SkinnedMeshRenderer / SpriteRenderer / Light / Camera 是标准视觉组件。

Godot：
  Node3D 承担空间和可见性。
  MeshInstance3D / Light3D / Camera3D 等节点承担可视对象。

Bevy：
  Transform / GlobalTransform + Mesh / Material / Camera / Light 等 ECS 组件组合。
```

本项目采用：

```text
UE 的责任边界。
Bevy 的 ECS 数据结构。
Unity 的用户心智。
AI 只能面向稳定小清单写视觉意图，不直接判断底层 dirty。
```

第一版暂不纳入普通标准组件：

```text
DecalRenderer
TerrainRenderer
ReflectionProbe
PostProcessVolume
RuntimeVirtualTexture
HairRenderer
WaterRenderer
CustomRenderPass
```

这些能力以后走 Render Extension，不污染普通项目层和 AI 默认生成能力。

## 7. Render-facing Component 字段到 Dirty 类型映射

核心 dirty 仍然只有 4 类：

```text
RenderState
Transform
DynamicData
InstanceData
```

第一版字段映射：

| Component | 字段 | Dirty |
|---|---|---|
| Transform | localPosition / localRotation / localScale / parent | Transform |
| Visibility | visible / layerVisible / editorVisible | DynamicData |
| MeshRenderer | meshRef / materialSlots / castShadow / receiveShadow / renderLayer / cullingMode | RenderState |
| SpriteRenderer | spriteRef / atlasRef / materialSlot / sortingLayer | RenderState |
| SpriteRenderer | color / flipX / flipY | DynamicData |
| SkinnedMeshRenderer | meshRef / skeletonRef / materialSlots | RenderState |
| SkinnedMeshRenderer | poseBuffer / blendShapeWeights / skinningData | DynamicData |
| MaterialBinding | materialRef / shaderVariant | RenderState |
| MaterialBinding | params / textureBinding / tint / emission | DynamicData |
| Light | lightType / shadowMode | RenderState |
| Light | color / intensity / range / angle / temperature | DynamicData |
| Camera | projectionType / renderTargetKind | RenderState |
| Camera | fov / orthoSize / near / far / clearColor / viewportRect | DynamicData |
| ParticleRenderer | effectAsset / renderMode / materialSlots | RenderState |
| ParticleRenderer | particleBuffer / aliveRange / simulationOutput | InstanceData |
| InstanceRenderer | meshRef / materialSlots / lodPolicy | RenderState |
| InstanceRenderer | instanceTransforms / instanceColors / customData | InstanceData |

Visibility 第一版统一走 DynamicData：

```text
显示 / 隐藏只改 RenderProxy 的 visible flag。
不因为普通可见性切换销毁 proxy。
只有添加 / 删除 Render-facing Component，或 renderer 类型变化，才进入 AddProxy / RemoveProxy / UpdateRenderState。
```

进入 RenderState 的典型变化：

```text
添加 / 删除 Render-facing Component。
meshRef 改变。
material slot 结构改变。
renderer 类型改变。
lightType 改变。
camera projectionType 改变。
shadowMode / castShadow / receiveShadow 这类影响渲染结构或 pass 归属的字段改变。
```

AI 面向的稳定写入规则：

```text
想移动物体 -> 写 Transform。
想隐藏物体 -> 写 Visibility。
想换模型 -> 写 MeshRenderer.meshRef。
想改颜色 -> 写 MaterialBinding.params.tint。
想调灯光 -> 写 Light.intensity / Light.color。
想换摄像机视野 -> 写 Camera.fov。
```

AI 不需要判断 dirty 类型；引擎根据标准组件字段自动映射 dirty。

## 8. Dirty 合并规则

同一帧内：

```text
同一个 entity 的同一 dirty type 只保留一次。
同一个 entity 多次 Transform 写入，只提交最终 Transform。
RenderState dirty 覆盖 Transform / DynamicData 的部分重建需求时，由 RenderExtract 决定命令合并。
Remove 覆盖所有 update。
Add 后同帧 update 合并进 Add payload。
```

这条规则借鉴 UE 的 EndOfFrame 合并。

好处：

```text
性能好。
用户和 AI 不需要考虑一帧内写了几次。
避免命令队列膨胀。
```

## 9. RenderExtract 同步点

第一版位置：

```text
EngineHostLoop
  -> Input
  -> FixedUpdate / Simulation
  -> Update
  -> LateUpdate
  -> EndOfSimulation
  -> RenderExtract
  -> Submit RenderCommandQueue
  -> Render
  -> Present
```

规则：

```text
RenderExtract 在逻辑写入全部结束后运行。
RenderExtract 只读 ECS 和 DirtyTracker。
RenderExtract 不执行业务逻辑。
RenderExtract 不修改项目组件。
RenderExtract 可以生成 RenderCommand 和 RenderFrameReport。
```

第一版即采用并行 RenderExtract 架构：

```text
Transform dirty 批量并行提取。
DynamicData / InstanceData 分组提取。
CommandQueue 多生产者单消费者。
输出按 frame_index / entity_id / component_type / dirty_type 稳定排序。
```

测试环境可以设置 worker_count=1，但必须走同一套并行提取器和命令队列。

但对上层 API 不增加新规则。

### RenderExtractScheduler

RenderExtract 的并行调度由 RenderExtractScheduler 管理。

它是本项目对 UE EndOfFrame 并行更新的简化版本：

```text
UE UWorld EndOfFrame component arrays
  -> 本项目 RenderDirtyTracker lists

UE DoDeferredRenderUpdates_Concurrent
  -> 本项目 RenderExtract job

UE ParallelFor / TaskGraph
  -> 本项目 WorkerPool / RenderExtractScheduler

UE ENQUEUE_RENDER_COMMAND
  -> 本项目 RenderCommandQueue
```

第一版 RenderExtractScheduler 只做：

```text
读取 dirty lists。
按 dirty type / component type / entity range 分块。
提交 WorkerPool 并行提取。
合并同帧重复更新。
按稳定顺序输出 RenderCommand。
生成 RenderFrameReport。
```

第一版不做：

```text
完整 UE TaskGraph。
任意用户自定义 render extract job。
Renderer feature 级复杂 invalidation graph。
GPUScene / VSM / RayTracing 专用 dirty 分流。
```

稳定性规则：

```text
worker_count=1 和 worker_count>1 必须生成相同语义的 RenderCommand。
RenderCommand 排序必须稳定，不能依赖 worker 完成顺序。
RenderExtract 只读 ECS 和 DirtyTracker，不写项目组件。
RenderExtract 内部并行不向 AI 和普通用户暴露。
```

### 合并、排序和确定性

RenderExtract 的正式原则：

```text
时间顺序是输入真相。
合并后的最终渲染状态是输出真相。
```

含义：

```text
ECS 每次写 Render-facing Component 时，记录 write_sequence。
DirtyTracker 按 write_sequence 记录变化发生顺序。
RenderExtract 不逐条输出所有变化。
RenderExtract 按 entity / component / dirty_type 合并。
同一帧同一字段多次写入，最终值生效。
生命周期命令保留必要先后顺序。
并行 RenderExtract 输出后，按稳定 sort_key 排序。
worker_count=1 和 worker_count>1 输出语义必须一致。
```

普通更新合并：

```text
t1: player.position = (0,0,0)
t2: player.position = (0,1,0)
t3: player.position = (0,2,0)

最终只生成：
  UpdateTransform(player, position=(0,2,0))
```

Report 规则：

```text
Summary 模式可以记录 skipped_redundant_updates = 2。
Evidence 模式才记录完整 from/to 链。
```

生命周期命令规则：

```text
Add 后又改材质：
  生成 1 条 AddProxy，材质最终值合进 Add payload。

Update 后 Remove：
  只生成 RemoveProxy，Update 被覆盖。

Add 后 Remove，且这个 proxy 还没进入 RenderSceneState：
  可以不生成 RenderCommand，只在 Report 记录 covered add/remove。

Remove 后 Add：
  如果是同一个 entity_generation 重新启用，可按语义生成 RemoveProxy -> AddProxy。
  如果 entity_generation 变了，必须生成 Remove old -> Add new。
```

稳定排序规则：

```text
RenderCommand sort_key 至少包含：
  frame_index
  lifecycle_order
  world_id
  scene_id
  entity_id
  entity_generation
  command_type_order
  component_type
  dirty_type
  write_sequence_last

禁止按 worker 完成顺序提交最终队列。
禁止让 HashMap / 并行遍历的非确定顺序影响最终 RenderCommandQueue。
```

命令类型顺序：

```text
RemoveProxy
AddProxy
UpdateRenderState
UpdateTransform
UpdateDynamicData
UpdateInstanceData
```

说明：

```text
Remove 优先用于覆盖无效 update。
Add 必须在 update 前建立 proxy。
RenderState 先于 Transform / DynamicData / InstanceData，因为它可能改变 proxy 结构。
Transform / DynamicData / InstanceData 之间可以按稳定顺序提交。
```

最终规则：

```text
RenderExtract 以时间顺序作为变化记录真相。
RenderCommandQueue 以合并后的最终渲染状态为输出真相。
生命周期命令保留必要时间顺序。
普通更新命令同帧合并，最后值生效。
并行提取后必须稳定排序。
```

## 10. RenderCommand 类型

第一版 RenderCommand 只保留 6 类：

```text
AddProxy
RemoveProxy
UpdateRenderState
UpdateTransform
UpdateDynamicData
UpdateInstanceData
```

这个分类来自 UE 的反推：

```text
UE 上层 dirty 是 4 类：
RenderState / Transform / DynamicData / Instances

本项目 RenderCommand 在 4 类更新命令前面补 2 类生命周期命令：
AddProxy / RemoveProxy

所以第一版是：
2 类生命周期命令 + 4 类更新命令 = 6 类 RenderCommand。
```

不再作为 command type 的用户语义：

```text
Material
Visibility
LightCamera
Renderable
```

它们只能作为 `UpdateRenderState` / `UpdateDynamicData` 的 payload 或 `RenderProxy` 字段。
```

### AddProxy

```text
entity_id
proxy_kind
transform
renderable
material_bindings
visibility
source
```

用于：

```text
Entity 第一次拥有可渲染组件。
Scene / Prefab 实例化。
Renderable 从 disabled 变 enabled。
Light / Camera / Decal / Particle 第一次创建对应 proxy。
```

### RemoveProxy

```text
entity_id
proxy_id
reason
source
```

用于：

```text
Entity 删除。
Renderable 删除。
Scene unload。
```

### UpdateRenderState

```text
entity_id
proxy_id
proxy_kind
renderable_state
material_slots
shadow_flags
render_layer
feature_flags
source
```

用于：

```text
mesh 替换。
renderable 类型变化。
material slot 结构变化。
shadow flags 变化。
render layer / feature flags 变化。
light / camera 类型或 proxy 结构变化。
```

说明：

```text
UpdateRenderState 表示 proxy 结构级变化。
如果变化需要重建 proxy，Render Thread 可以在内部执行 remove + add 或 recreate。
项目层和 AI 不需要知道底层是否重建。
```

### UpdateTransform

```text
entity_id
proxy_id
local_to_world
bounds
previous_transform
source
```

用于：

```text
移动、旋转、缩放。
骨架根节点移动。
相机/灯光位置变化。
```

### UpdateDynamicData

```text
entity_id
proxy_id
dynamic_payload_kind
dynamic_payload
source
```

用于：

```text
材质参数变化。
贴图绑定变化。
visible flag 变化。
light intensity / color / range 变化。
camera fov / near / far / exposure 变化。
custom render data 变化。
animation pose / skinning dynamic data。
```

说明：

```text
UpdateDynamicData 表示不改变 proxy 结构的动态数据更新。
Material / Visibility / LightCamera 不再是 command type，只是 dynamic_payload_kind。
```

### UpdateInstanceData

```text
entity_id
proxy_id
instance_count
instance_buffer_ref
changed_range
source
```

用于：

```text
大量草、子弹、粒子、单位、群体等实例化数据。
```

## 11. RenderCommand 公共字段

RenderCommand 字段分为两层：

```text
Runtime 必要字段：
  command_id
  frame_index
  command_type
  world_id
  scene_id
  entity_id
  entity_generation
  proxy_id optional
  component_type
  dirty_type
  payload_kind
  payload
  resource_refs
  sort_key

Debug metadata：
  source_component
  source_field optional
  source_system optional
  source_rule optional
  source_patch optional
  reason_code
  reason_string optional
  validation_status optional
  source_map optional
  trace_id optional
```

原因：

```text
Render Thread 执行命令只应该依赖 Runtime 必要字段。
AI 查 bug 需要知道命令从哪里来，因此需要 Debug metadata。
用户需要知道为什么画面变化，因此需要 reason / trace。
验证层需要能追踪非法资源引用和无效 proxy，因此需要 validation_status。
```

分层规则：

```text
Runtime 必要字段属于热路径。
Debug metadata 属于旁路调试数据。
RenderSceneState 更新结果不能依赖 Debug metadata。
RenderFrameReport 可以读取 Debug metadata。
Release 构建可以裁剪 source_patch / reason_string / source_map 等重型元数据。
Release 构建不能裁剪 Runtime 必要字段。
Profile 构建可以保留 trace_id / reason_code / validation_status 等轻量 id。
Editor / Debug / Evidence 模式可以保留完整 Debug metadata。
```

字段含义：

```text
command_id：
  本帧或全局唯一命令 id，用于排序、追踪和去重。

frame_index：
  命令所属帧。

command_type：
  AddProxy / RemoveProxy / UpdateRenderState / UpdateTransform / UpdateDynamicData / UpdateInstanceData。

world_id / scene_id：
  多世界、多 scene、编辑器 preview、game view、additive scene 的归属。

entity_id / entity_generation：
  定位源 Entity，并避免旧 EntityId 复用导致误用。

proxy_id：
  已存在 RenderProxy 的目标。AddProxy 时可以为空。

component_type / dirty_type：
  说明命令来自哪个 render-facing component 和哪类 dirty。

payload_kind / payload：
  实际执行数据，例如 transform matrix、material params、light params、instance buffer ref。

resource_refs：
  命令依赖的 mesh / material / texture / shader variant / buffer 等资源句柄。

sort_key：
  RenderExtract 并行输出后的稳定排序键。

Debug metadata：
  只用于 Report / Trace / AI 排错，不参与 Render Thread 正确性。
```

### RenderCommand Payload Schema

RenderCommand 采用：

```text
外层固定 schema。
内层按 command_type / payload_kind 使用 typed payload。
```

方案对比：

| 方案 | 结构 | 优点 | 缺点 | 判断 |
|---|---|---|---|---|
| A | 一个超级固定 payload | 简单直观 | 字段爆炸，大量 optional，后期难维护 | 不选 |
| B | 完全动态 JSON / Map payload | AI 看起来容易 | 慢、难验证、运行时不稳定 | 不选 |
| C | 外层固定 + typed payload | 稳定、可验证、可优化、AI 可读 | 需要维护 payload 类型表 | 采用 |
| D | UE 式 lambda / 函数命令 | 性能强、灵活 | 不利于 AI、Trace、序列化和验证 | 只作底层参考 |

正式结构：

```text
RenderCommand
  runtime_fields
  payload_kind
  payload
  debug_metadata optional
```

第一版 payload 类型：

```text
AddProxyPayload
RemoveProxyPayload
UpdateRenderStatePayload
UpdateTransformPayload
UpdateDynamicDataPayload
UpdateInstanceDataPayload
```

### Payload 类型

AddProxyPayload：

```text
proxy_kind
initial_transform
renderable_state
material_bindings
visibility
resource_refs
```

RemoveProxyPayload：

```text
proxy_id
remove_reason_code
```

UpdateRenderStatePayload：

```text
proxy_kind
mesh_ref optional
material_slots optional
shadow_flags optional
render_layer optional
feature_flags optional
```

UpdateTransformPayload：

```text
local_to_world
previous_local_to_world optional
bounds
```

UpdateDynamicDataPayload：

```text
dynamic_payload_kind
material_params optional
light_params optional
camera_params optional
visibility optional
skinning_data optional
```

UpdateInstanceDataPayload：

```text
instance_count
instance_buffer_ref
changed_range
```

规则：

```text
RenderCommand 外层 schema 固定。
payload 按 command_type / payload_kind 使用 typed payload。
运行时禁止使用 JSON / Map 作为正式 payload。
Debug / Evidence 可以导出 JSON 视图，但那是报告格式，不是运行时格式。
每个 payload 必须有验证规则。
每个 payload 必须能被 RenderFrameReport 摘要化。
payload_kind 必须和 command_type 匹配。
Render Thread 根据 command_type / payload_kind 分派执行。
```

对 AI 的规则：

```text
AI 不直接生成 RenderCommand payload。
AI 生成 RenderIntent / Visual Patch / Material Graph / Preset。
引擎验证后由 RenderExtract / Renderer Feature Builder 生成 typed payload。
AI 在排错时可以查看 payload 的 Evidence JSON 视图。
```

### 构建模式裁剪规则

RenderCommand Runtime 字段永远不裁剪。Debug metadata 和 RenderFrameReport 按构建模式裁剪。

| 构建模式 | RenderCommand Runtime 字段 | RenderCommand Debug metadata | RenderFrameReport | 目的 |
|---|---|---|---|---|
| Editor | 全保留 | 全保留 | 默认 Summary，可升 Evidence | AI 调试、用户解释 |
| Debug | 全保留 | 全保留 | 默认 Summary，可升 Evidence | 开发排错 |
| Profile | 全保留 | 只保留轻量 id | 默认 Stats | 测性能，不污染结果 |
| Release | 全保留 | 默认裁剪 | 默认 Off | 正式运行性能 |
| Crash / Error Evidence | 全保留 | 保留必要 id | 一次性轻量摘要 | 线上定位严重问题 |

Runtime 必须永远保留：

```text
command_id
frame_index
command_type
world_id
scene_id
entity_id
entity_generation
proxy_id optional
component_type
dirty_type
payload_kind
payload
resource_refs
sort_key
```

Release 默认裁剪：

```text
source_component
source_field
source_system
source_rule
source_patch
reason_string
source_map
完整 from/to
完整 RenderCommand payload dump
完整 RenderSceneState dump
完整 ECS dump
```

Release 可以在严重异常时保留轻量 id：

```text
reason_code
validation_status
trace_id optional
resource_id
fallback_code
error_code
```

Profile 保留：

```text
command_count
dirty_entity_count
fallback_count
missing_resource_count
cost
commands_by_type
reason_code
validation_status
```

Editor / Debug / Evidence 可以保留：

```text
source_system
source_patch
source_field
reason_string
source_map
必要 from/to
底层 RenderCommand 展开
资源证据链
```

裁剪规则：

```text
RenderCommand Runtime 字段永远不裁剪。
Debug metadata 按构建模式裁剪。
RenderFrameReport 默认按 Level 生效。
Release 默认 Off。
Release 只在严重异常时生成一次性轻量 Evidence。
Profile 只保留性能统计和轻量 id。
Editor / Debug 才保留 AI 完整证据链。
Evidence 必须支持按帧段开启，不能无限录制。
```

## 12. RenderSceneState

RenderSceneState 是 Render Thread 长期状态。

第一版保存：

```text
proxy_map: EntityId -> RenderProxyId
proxies: RenderProxyId -> RenderProxy
lights
cameras
dirty_stats
resource_bindings
```

RenderProxy 保存：

```text
proxy_id
entity_id
proxy_kind
transform
bounds
renderable_state
material_state
visibility_state
instance_state optional
```

RenderSceneState 不保存：

```text
Gameplay Component
Project Logic State
Inventory / Skill / Buff / Quest
AI Feature Spec
完整 ECS 副本
```

## 13. RenderFrameReport

AI 和用户默认看 RenderFrameReport，不直接看 RenderCommandQueue。

RenderFrameReport 是 Runtime Debug / AI Evidence 系统的一部分。它必须能在 Runtime 生效，但不能作为渲染主流程必需数据全量常开。

第一版字段：

```text
frame_index
report_level
platform
quality_profile
views
counters
changed_entities
render_events
trace_refs
```

作用：

```text
解释为什么画面变了。
解释为什么画面没变。
解释为什么资源丢失或降级。
帮助 AI 定位“用户说颜色没改成功”这类问题。
作为 AI 默认渲染调试入口。
RenderFrameReport 解释不了的问题，再下钻到 RenderCommand。
```

### 第一版字段结构

第一版字段必须少而精。Summary 模式只保存足够定位问题的摘要，Evidence 模式才保存重数据。

```text
views:
  view_id
  view_kind，Scene / Game / Preview / Shadow / Reflection
  camera_id
  visible_count
  culled_count

counters:
  dirty_entity_count
  command_count
  fallback_count
  missing_resource_count
  warning_count
  error_count

changed_entities:
  entity_id
  component
  change_kind
  result，Applied / Skipped / Covered / Failed
  trace_id

render_events:
  severity
  event_code
  entity_id optional
  resource_id optional
  view_id optional
  render_feature optional
  reason_code
  fallback_code optional
  trace_id

trace_refs:
  trace_id
  source_system optional
  source_patch optional
```

第一版默认不记录：

```text
完整 from / to 值。
完整 RenderCommand payload。
完整 RenderSceneState。
完整 ECS World。
长字符串 reason。
完整 source map。
每个 entity 的完整可见性细节。
```

这些只允许在 Level 3 Evidence 模式按帧段开启。

### 用户问题覆盖

RenderFrameReport v1 必须覆盖以下问题：

```text
为什么没显示？
为什么没移动？
为什么材质没变？
为什么变黑？
为什么手机端效果降级？
为什么资源没加载？
为什么 Scene 里能看到，Game 里看不到？
```

对应最小证据：

```text
没显示：
  entity_id / view_id / camera_id / render_status event / reason_code / trace_id

没移动：
  entity_id / component=Transform / change_kind=UpdateTransform / result / trace_id

材质没变：
  entity_id / component=MaterialBinding / resource_id / result / fallback_code / trace_id

变黑：
  entity_id optional / resource_id optional / render_feature / fallback_code / quality_profile / severity / trace_id

手机端降级：
  platform / quality_profile / render_feature / requested_state / actual_state / fallback_code / reason_code

资源没加载：
  resource_id / load_status event / fallback_resource_id optional / reason_code

Scene 能看到 Game 看不到：
  view_id / view_kind / camera_id / visible_count / culled_count / entity render event / reason_code
```

最小解释规则：

```text
Summary 模式回答：
  发生了什么。
  谁受影响。
  结果是什么。
  去哪里深查。

Evidence 模式回答：
  字段具体从什么变到什么。
  底层 RenderCommand 是什么。
  资源 / Shader / Quality / View 的完整证据链是什么。
```

### 生效模式

正式分级：

```text
Level 0 Off：
  Release 默认。
  不生成完整 RenderFrameReport。
  只保留严重错误码、crash evidence、资源缺失摘要。

Level 1 Stats：
  Profile 默认。
  只记录 command_count、dirty_entity_count、fallback_count、resource_missing_count、cost。
  不记录每个 entity 的完整字段变化。

Level 2 Summary：
  Editor Play 默认。
  记录 changed_entities 摘要、commands_by_type、warnings、fallbacks、source ids。
  不记录大量字符串和完整 from/to 大对象。

Level 3 Evidence：
  AI 深度排错、用户点击录制、Golden Test 失败、指定帧段调试时开启。
  记录 source_system、source_patch、changed fields、reason、resource trace、必要 from/to。
```

运行场景：

```text
Editor Play：
  默认 Level 2 Summary。
  用户或 AI 发现视觉问题时，临时提升到 Level 3 Evidence。

Debug Runtime：
  默认 Level 2 Summary，可手动提升到 Evidence。

Profile Runtime：
  默认 Level 1 Stats。
  用于观察渲染同步成本，不能明显污染性能结论。

Release Runtime：
  默认 Level 0 Off。
  只在严重资源缺失、shader fallback、render crash 等情况下记录一次性轻量摘要。

Golden Test：
  可以使用 Level 3 Evidence，但必须限制场景和帧段。
```

性能和边界规则：

```text
RenderFrameReport 是旁路数据。
RenderFrameReport 不能参与游戏逻辑。
RenderFrameReport 不能影响渲染结果。
RenderFrameReport 必须可关闭、可裁剪、可按帧段开启。
RenderFrameReport 不能要求每帧复制完整 ECS World。
RenderFrameReport 不能要求每帧复制完整 RenderSceneState。
RenderFrameReport 不能在 release 默认保存完整 source map、reason string、from/to 大对象。
```

## 14. 与 Unity / UE 对比

| 项目 | UE | Unity | 我们 |
|---|---|---|---|
| 同步方式 | Dirty + EndOfFrame + RenderCommand + FScene | PlayerLoop + Native Renderer + SRP/CommandBuffer | DirtyTracker + RenderExtract + RenderCommand + RenderSceneState |
| 用户心智 | 专家复杂 | 简单 | 简单 |
| AI 可读性 | 弱 | 弱/中 | 强 |
| 性能路线 | 很强 | 强 | 目标强 |
| 第一版复杂度 | 高 | 中 | 中低 |
| 是否完整场景 snapshot | 否 | 公开层不暴露 | 否 |
| 调试报告 | 专家工具 | Profiler / Frame Debugger | RenderFrameReport / Trace |

## 15. 第一版边界

第一版必须做：

```text
RenderDirtyTracker 数据结构。
ECS 写入自动 dirty。
RenderExtract 并行提取器。
6 类 RenderCommand。
多生产者单消费者 RenderCommandQueue。
RenderSceneState 最小 proxy map。
RenderFrameReport。
RenderSnapshot 新功能禁用。
```

第一版不做：

```text
GPUScene。
Virtual Shadow Map cache。
Ray Tracing dirty。
Distance Field dirty。
完整 RDG pass invalidation。
每个 renderer feature 自定义 dirty 类型。
AI 直接写 RenderCommand。
```

## 16. 最小测试用例

测试 1：Transform 更新

```text
输入：
  Entity A 有 Transform + Renderable。
  本帧修改 position 三次。

期望：
  DirtyTracker 只有 Transform dirty。
  RenderExtract 只生成 1 条 UpdateTransform。
  RenderCommand 使用最终 position。
  RenderFrameReport 记录 skipped_redundant_updates = 2。
```

测试 2：同帧 Add + Update

```text
输入：
  本帧创建 Entity B。
  添加 Transform + Renderable + Material。
  同帧修改 Material 参数。

期望：
  RenderExtract 生成 1 条 AddProxy。
  Material 参数合并进 AddProxy payload。
  不额外生成 UpdateDynamicData。
```

测试 3：同帧 Remove 覆盖 Update

```text
输入：
  Entity C 本帧先修改 Transform。
  随后删除 Entity C。

期望：
  RenderExtract 只生成 RemoveProxy。
  不生成 UpdateTransform。
  RenderFrameReport 记录 covered_updates。
```

测试 4：材质替换

```text
输入：
  Entity D 的 materialRef 从 m_old 改成 m_new。

期望：
  如果 material slot 结构变化，DirtyTracker 标记 RenderState dirty。
  如果只是 material 参数变化，DirtyTracker 标记 DynamicData dirty。
  RenderExtract 验证 m_new 存在。
  slot 结构变化生成 UpdateRenderState。
  参数变化生成 UpdateDynamicData。
  如果 m_new 缺失，生成 fallback warning。
```

测试 5：灯光变化

```text
输入：
  DirectionalLight 强度和方向变化。

期望：
  灯光方向变化标记 Transform dirty。
  灯光强度变化标记 DynamicData dirty。
  RenderExtract 生成 UpdateTransform + UpdateDynamicData。
  RenderSceneState 更新 light proxy。
```

## 17. 最终规则

```text
1. RenderSnapshot 架构废弃。
2. 正式 Game 到 Render 同步采用 DirtyTracker -> Extract -> CommandQueue -> SceneState。
3. Dirty 类型第一版只保留 RenderState / Transform / DynamicData / InstanceData。
4. Dirty 只由标准 Render-facing Component / 高级 Render Extension Component 产生，普通 Project Component 不声明 render dirty。
5. 第一版标准 Render-facing Component 只保留 Transform / Visibility / MeshRenderer / SpriteRenderer / SkinnedMeshRenderer / MaterialBinding / Light / Camera / ParticleRenderer / InstanceRenderer。
6. RenderExtract 是唯一把 ECS dirty 转成 RenderCommand 的地方。
7. RenderCommand 第一版只保留 AddProxy / RemoveProxy / UpdateRenderState / UpdateTransform / UpdateDynamicData / UpdateInstanceData。
8. RenderExtract 第一版即按并行提取架构设计，worker_count=1 只作为测试配置。
9. RenderCommandQueue 第一版即按多生产者单消费者模型设计。
10. RenderSceneState 只保存渲染长期状态，不保存 gameplay。
11. AI 默认读取 RenderFrameReport，不直接读 RenderCommandQueue。
12. Material / Visibility / LightCamera / Renderable 不作为 RenderCommand 类型，只作为 command payload 或 RenderProxy 字段。
13. RenderFrameReport v1 采用 views / counters / changed_entities / render_events / trace_refs 五块结构。
14. RenderCommand 字段分为 Runtime 必要字段和 Debug metadata。
15. Release 可以裁剪 Debug metadata，但不能裁剪 Runtime 必要字段。
16. RenderFrameReport / Debug metadata 按 Editor / Debug / Profile / Release / Crash 模式裁剪。
17. RenderExtract 以时间顺序作为输入真相，以合并后的最终状态作为输出真相。
18. 并行 RenderExtract 必须稳定排序，不能依赖 worker 完成顺序。
19. RenderCommand 外层 schema 固定，内层按 command_type / payload_kind 使用 typed payload。
20. 运行时禁止使用 JSON / Map 作为正式 RenderCommand payload。
21. 第一版以简单可解释为主，底层高级 dirty 在 Renderer 后端内部扩展，不污染上层模型。
22. RenderCommandQueue 输出前必须 normalize / merge，RenderSceneState 只通过 apply_batch 消费命令。
23. AddProxy 建立 RenderProxy 和 entity_to_proxy / source_to_proxy 映射，RemoveProxy 删除 proxy 和映射。
24. UpdateTransform 只更新 common.transform / previous_transform / bounds / version，不修改 typed payload。
25. UpdateRenderState 更新 enabled / visible / layer / flags，必要时可替换 typed payload。
26. UpdateDynamicData / UpdateInstanceData 只更新对应 typed payload 的动态数据和实例数据。
27. 同帧合并采用 last value wins，但生命周期命令 AddProxy / RemoveProxy 必须保留顺序。
28. Update missing proxy、Remove 后 Update、Add 已存在且 payload kind 冲突都必须生成 diagnostics / RenderFrameReport 事件。
29. RenderCommand 只负责更新 RenderSceneState，不等同于 RDG / RHI / GPU CommandBuffer。
30. RenderCommandQueue 主线采用 UE-like typed queue，吸收 Bevy Extract discipline 和 O3DE Render Feature boundary。
31. RenderExtract worker 只能写 ThreadLocalCommandBuffer，不能直接写 RenderSceneState。
32. RenderCommandQueue.collect 汇总线程本地命令后，必须 stable_sort / normalize / merge，再 apply_batch。
33. RenderCommand sort_key 第一版采用 frame_index / lifecycle_order / runtime_entity_id / command_type_order / command_id。
34. Unity-like 黑箱同步不适合作为本项目主线，因为 AI-first 需要 diagnostics / RenderFrameReport 证据链。
35. Bevy-like 完整 RenderWorld ECS 第一版不照搬，避免过早增加第二套 World 同步复杂度。
36. normalize / merge 第一版采用 ObjectCommandSlot，按 proxy/entity 聚合命令。
37. ObjectCommandSlot 只保存 existed_at_frame_start、lifecycle、RenderState / Transform / DynamicData / InstanceData payload 和 diagnostics。
38. 同类 payload 后写覆盖前写，不同类 payload 可以并存。
39. 对象最终不存在时普通 Update 不生效，并记录 covered_updates 或 missing_proxy。
40. Remove + Add 表示 Recreate，输出 RemoveProxy + AddProxy。
41. ObjectCommandSlot / MergedRenderCommand 只作为深度排错证据层，AI 默认读 RenderFrameReport 摘要。
42. RenderCommandDiagnostic 是命令级证据，RenderFrameReport 是帧级摘要。
43. RenderCommandDiagnostic 第一版字段只保留 diagnostic_id / frame_index / severity / code / stage / entity ids / proxy_id / command info / result / reason_code / trace_id。
44. RenderFrameReport 第一版字段只保留 frame_index / report_level / counters / changed_entities / render_events / trace_refs。
45. Release 不允许因为 diagnostics 复制完整 RenderSceneState、完整 payload 或完整 source map。
46. Evidence 模式才允许展开 payload 摘要、from/to、source map、ObjectCommandSlot / MergedRenderCommand。
47. 多 View / 多 Camera / Editor Scene View 第一版采用最小持久 View Registry + 每帧临时 RenderFrameViewData。
48. RenderSceneState 只保存全局 RenderProxy 和最小 RenderViewState；visible_proxy_ids / culling result / render phase 由 RenderFrameViewData 每帧生成。
49. Game View、Editor Scene View、Preview、Shadow、Reflection 都是 View；它们共享 RenderSceneState，但不能互相改写 view 状态。
```
