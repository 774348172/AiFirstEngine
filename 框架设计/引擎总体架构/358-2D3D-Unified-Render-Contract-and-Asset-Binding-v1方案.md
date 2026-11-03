# 358 2D/3D Unified Render Contract and Asset Binding v1 方案

## 1. 状态与决策

- 日期：2026-09-22。
- 文档状态：正式方案；U0–U2源码底座已完成归档，U2通过Windows/Vulkan离屏像素验收，3D方案A项目纵切未执行。实际范围见[U0合同](358-U0-render-contract-v1合同冻结附录.md)、[U1合同](358-U1-native-render-assets与命名绑定合同附录.md)、[U2合同](358-U2-View-Depth-DrawGeometry实际合同.md)和[U2完成记录](阶段完成记录/2026-09-22-358-U2-View-Depth-RHI/00-总览.md)；默认安装未更新。
- 方案版本：v1。版本号描述本文的设计合同，不表示源码、默认安装或生产交付已经具备本文全部能力。
- 目标路线：采用 **U-A Contract-first** 作为外部设计方式，采用 **U-C Domain-first** 作为内部 RenderPhase 组织方式，然后在统一合同上执行后续的 3D Vertical Slice 方案 A。
- 本文不直接授权施工；U0 的执行范围由[独立施工文档](施工文档/已完成/358-U0-2D3D统一渲染合同与兼容接入施工文档.md)限定。默认 Provider、宿主配置和现有项目未改变。
- 当前实现、安装和唯一施工槽仍以 [49 当前状态与下一步入口](49-当前状态与下一步入口.md) 和 [54 当前可自动化施工总入口](54-当前可自动化施工总入口.md) 为准。
- 产品和架构 authority 仍为 [权威产品需求 v1.1](00-AI-First-Game-Engine-权威产品需求-v1.1.md) 与 [权威架构设计 v1.1](00-AI-First-Game-Engine-权威架构设计-v1.1.md)。

### 1.1 选择结论

本方案冻结以下长期方向：

```text
2D/3D authoring component 分域
        -> 共用 Scene / Transform / Asset / View / Resource / Report 合同
        -> 各域通过 RenderProjectionAdapter<T> 投影
        -> Core2D / Core3D / UI 分域 RenderPhase
        -> 共用 RenderGraph / RHI / GPU resource lifetime
        -> 方案 A：3D Vertical Slice
```

统一的是底层生产链和稳定接缝，不是把 2D、3D、AUI、Physics2D、Physics3D 强行压成一个万能对象。

### 1.2 继承关系

本方案继承并深化以下既有设计，不重新打开其已经完成的施工：

| 既有设计 | 本方案继承内容 |
| --- | --- |
| 55 RenderSceneState / RenderProxy | RenderProxy 生命周期、source/runtime identity、view registry |
| 80 GPU Resource Pool | generation、stale handle、safe release、upload budget |
| 102 AUI Render Extract | UI 投影和 UI composition pass 的独立 owner |
| 115 EngineRHI Trait | RuntimeRenderer -> RenderGraph -> RhiCommandPlan -> EngineRhiBackend |
| 116 Real WGPU Backend | backend-neutral command、WGPU 只是第一个 backend |
| 137/138 Render Asset Production | RuntimeAsset -> typed render asset -> resource handle -> binding |
| 353 Ordered Input Acceptance | 既有输入验收合同和同一 delivery/run 语义 |
| 356 AUI C2 | AUI 布局、主题、控件和字体结果不进入 World Render 真相 |

## 2. 问题与目标

### 2.1 当前问题

当前源码已经存在可复用的渲染接缝，但统一层仍不够深：

- `RenderProxyPayload` 已有 Mesh、Sprite、SkinnedMesh、Light、Camera、Particle 等类型，但项目组件注册表仍主要是 Transform、Renderable、SpriteRenderer2D、Collider2D。
- `RendererFeatureDrawItem` 把 `mesh_ref`、`sprite_ref` 和 2D `sorting_layer/order_in_layer/sort_z` flatten 到同一个结构，3D 会继承无效字段。
- `RenderViewState` 主要保存 raw view/projection matrix 和字符串 layer mask，尚无 Camera2D/Camera3D 的投影语义、深度合同和来源身份。
- `RuntimeRenderAsset` 的 Mesh 仍然是 descriptor/count 级别；真实 vertex/index/submesh/material slot 尚未进入 RuntimePackage。
- WGPU 的 MeshBasic 路径仍然可以回退到测试几何，不能证明真实 Mesh 数据被消费。
- per-view culling、inherited visibility、view visibility 和 culling reason 尚未形成完整报告。
- RenderGraph 的资源引用主要是字符串，BindingSet 中的资源句柄没有命名用途。
- 诊断缺少 source path、expected/actual、依赖链、修复建议和 retryability，AI 需要猜底层状态。

这些事实说明，本项目不应该直接在现有 flatten/string/test-geometry 路径上堆 3D 字段。应先冻结统一合同，再逐个迁移已有 2D consumer。

### 2.2 目标

本方案的目标是建立一个长期可扩展、AI 可读、可兼容迁移的统一渲染和资产绑定合同：

1. 让 Sprite2D、Mesh3D、SkinnedMesh、Particle 和 AUI 使用一致的 Asset identity、resource lifetime、report 和 RHI seam。
2. 让 2D/3D 共享 Transform、View、Target、Visibility、RenderGraph 和 GPU resource 管理，但保留分域的绘制语义。
3. 让项目和 AI 通过 typed authoring schema、AssetRef、MaterialPreset、CameraPreset 和字段级 patch 创作渲染内容；真实 Rust 项目代码和已批准的 WGSL 扩展仍是一等输入，不把整个项目创作限制为字段 CRUD。
4. 让 Compiler、Import/Cook、RuntimePackage、RenderProjection、RenderGraph 和 Backend 输出可追溯的 identity 与诊断。
5. 保持现有 Neon、Tower、复杂 Shooter 和 AUI 2D 行为兼容，为后续方案 A 的真实 3D Vertical Slice 提供稳定接缝。既有证据只对其已消费且未改变的源码、测试、二进制和配置身份继续有效；新源码或新二进制不自动继承旧截图资格。

### 2.3 非目标

本方案不承诺以下能力已经交付，也不把它们作为 U0-U2 的施工前置：

- 完整 PBR、材质图、Shader Graph、Nanite、Lumen、虚拟纹理或多 GPU；
- 完整 Skeletal Mesh、动画状态机、布料和复杂 VFX；
- Physics3D、角色控制器、NavMesh 和 Navigation3D；
- 3D 编辑器 Viewport、完整 Scene gizmo 和传统 Inspector 复制；
- Android/非 Windows 3D 资格；
- 网络、多人同步和联机服务；
- 以新增独立工具替代现有九项 typed tool surface。

这些能力属于统一合同稳定后的后续模块，不能在本方案中通过占位字段宣称已经完成。

## 3. 其他引擎研究结论

### 3.1 Godot

Godot 将 `Node2D`/`CanvasItem` 和 `Node3D`/`VisualInstance3D` 分域，但二者都通过 SceneTree、Resource、RenderingServer 和底层 RenderingDevice 进入渲染。Sprite2D 保留自己的帧、纹理、flip 和 Canvas 排序；VisualInstance3D 保留 3D instance、layer 和 transform 语义。

可采用：

- 高层对象不直接创建 GPU 对象；
- RenderingServer/Storage 作为资源和生命周期接缝；
- Canvas 与 3D Scene 分域；
- Importer、Resource 和 Runtime backend 分层。

不照搬：

- 不把 RID 直接暴露给项目或 AI；
- 不把 Variant/dynamic property 当作长期项目合同；
- 不复制完整 ObjectDB、ClassDB 和 Godot 编辑器对象生命周期。

参考：[Godot 总体源码地图](../Godot源码参考/12-GodotAllCode-整体源码地图与参考方案.md)、[Godot Render Asset Binding](../Godot源码参考/11-Runtime-Render-Asset-Production-Binding源码参考.md)。

### 3.2 Unity

Unity 的 `Renderer` 提供 enabled、bounds、material、layer、shadow 等共用概念，但 SpriteRenderer、MeshRenderer、SkinnedMeshRenderer 仍有各自的字段。RenderPipeline/CommandBuffer 负责把高层 Renderer 组织成后端可执行命令。

可采用：

- 项目层只引用 Mesh、Material、Texture 和 Renderer；
- 2D/3D 组件分域，底层资源和渲染管线共用；
- AI 不直接操作 native GPU 生命周期。

不照搬：

- 不复制隐式 AssetDatabase 和 native 黑盒；
- 不让材质和渲染失败只返回无上下文的 native error；
- 不把所有组件行为放入字符串属性。

参考：[Unity Render Asset Binding](../Unity源码参考/Runtime-Render-Asset-Production-Binding源码参考.md)。

### 3.3 Unreal Engine

UE 的 Actor/Component 负责 authoring 和 gameplay 语义；Primitive Component 的 dirty 更新在帧边界合并，然后产生具体 SceneProxy，进入 Renderer/RDG/RHI。StaticMesh、SkeletalMesh、Particle 和 Light 各自拥有不同 proxy，但共享 render resource 和线程生命周期。

可采用：

- Game/Asset 层不直接碰 GPU object；
- Dirty -> projection -> render proxy -> render thread 的边界；
- 帧内 RenderGraph 资源读写由引擎生成并校验；
- Render Resource 的初始化、释放和 owner 可追踪。

不照搬：

- 不复制完整 UObject/Reflection/Blueprint 体系；
- 不把 RDG、RHI、Shader 编译细节暴露给 AI；
- 不以一开始实现 UE 级 PBR、Nanite 或 Lumen 作为统一合同条件。

参考：[UE Runtime Render Asset Binding](../UE源码参考/Runtime-Render-Asset-Production-Binding源码参考.md)、[UE Game-to-Render Dirty Path](../UE源码参考/GameToRender-Dirty-EndOfFrame-PrimitiveUpdates.md)。

### 3.4 Bevy

Bevy 的 Core2d、Core3d 和 UI 使用不同 camera/phase graph，但共享 typed Handle、Extract、Prepare、Queue、RenderAsset 和 RenderGraph。它是 Rust 项目最适合参考的实现组织。

可采用：

- 类型化资产句柄；
- Main World 到 Render World 的显式 Extract；
- 具体域实现自己的 projection/query；
- Material、Mesh、Texture 的 prepare/bind 分离。

不照搬：

- 不复制完整 RenderWorld/Schedule 复杂度；
- 不把项目作者暴露给几十个底层 pipeline key；
- 不把默认诊断深度直接当成 AI 诊断合同。

参考：[Bevy PBR/Camera/Light/Mesh](../Bevy源码参考/09-PBR-Camera-Light-Mesh.md)。

### 3.5 AI 工具层参考

本地 Godot Agent Layer 研究还显示，AI 友好的编辑入口具有以下特征：typed schema、受控 operation、字段级 patch、group undo、diagnostics-first、request identity 和明确的终态。我们应把这一思路用于 2D/3D render authoring，但不让 AI 直接写 RenderProxy、RenderGraph 或 GPU handle。

## 4. 核心架构

### 4.1 唯一事实链

```text
Project Scene / Prefab / AssetRef / MaterialPreset / CameraPreset
        |
        v
Project Compiler + Import/Cook + Dependency Graph
        |
        v
RuntimePackage（source identity + cooked assets + manifests）
        |
        +--> Runtime World / Project Module
        |
        v
RenderProjectionAdapter<T>
        |
        v
Typed RenderInstance / GeometryBinding / MaterialBinding / RenderView
        |
        v
Core2D / Core3D / Shadow / UI RenderPhase
        |
        v
RenderGraph -> RhiCommandPlan -> EngineRhiBackend
        |
        v
Headless / WGPU / Future Backend
        |
        v
Inspect / Check / Observe / Capture / Render Reports
```

规则：

1. Project Scene、Prefab、AssetRef、MaterialPreset 和 CameraPreset 是 authoring 真相。
2. RuntimePackage 是运行时和交付输入真相；Runtime 不扫描项目源文件。
3. RenderProjection 是 World 到 render-facing typed object 的唯一外部接缝。
4. RenderGraph 和 RHI 是引擎派生物；AI 不直接生成它们。
5. AUI 继续通过 UiProjection 和 UI composition pass 进入目标，不成为 World ECS Entity，也不混入 World 的 2D/3D depth sort。

### 4.2 深模块与接缝

本方案按 deep module 原则设计：对外接口小，内部隐藏资源生产、投影、排序、可见性、绑定和后端生命周期。

正式接缝由五个内部合同组成：

```text
RenderInstance
GeometryBinding
MaterialBinding
RenderView
RenderPolicy
```

`RenderProjectionAdapter<T>` 是 Adapter，不是新的项目 API。它负责把 `SpriteRenderer2D`、`MeshRenderer3D`、`Camera3D` 等域对象转换成上述合同，并保留 source path、entity identity 和 asset provenance。

### 4.3 Authoring 域对象

authoring 采用 discriminated typed schema，而不是万能 optional object：

```text
VisualComponent =
    Sprite2D(SpriteRenderer2DSpec)
  | Mesh3D(MeshRenderer3DSpec)
  | SkinnedMesh3D(SkinnedMeshRenderer3DSpec)
  | Particle(ParticleRendererSpec)
```

建议对象：

```text
SpriteRenderer2DSpec:
  sprite: AssetRef<Texture>
  material: AssetRef<MaterialPreset>
  color / flip / sorting_layer / order_in_layer

MeshRenderer3DSpec:
  mesh: AssetRef<Mesh>
  material_slots: [AssetRef<MaterialPreset>]
  cast_shadow / receive_shadow

Camera2DSpec:
  projection: Orthographic
  size / near / far / clear / layer_set

Camera3DSpec:
  projection: Perspective
  fov_y / near / far / clear / layer_set

Light3DSpec:
  kind / color / intensity / range / direction
```

具体字段、默认值和兼容键在 U0 合同冻结时确定。AI 对这些声明式对象做字段级 patch；这不限制正常 Rust 项目代码和已批准的扩展接口。

## 5. 统一合同

### 5.1 AssetRef

所有渲染资产使用结构化引用。下列是逻辑合同，不是另一份独立资产身份或要求复制到每个引用的物理字段：

```text
AssetRef<T>:
  asset_id: stable GUID or project asset id
  asset_type: typed asset kind
  revision: source revision
  content_digest: optional until cook, required in RuntimePackage
  source_path: source mapping only
```

源路径不是身份真相。RuntimePackage 必须记录 source identity、cooked identity、依赖列表和平台/质量 profile。

身份与解析复用现有 `runtime_package::RuntimeAssetRef`、`RuntimeAssetRecord` 和 `RuntimeAssetIndex`。旧引用键保持 `id/type/guid/sub_asset`；revision、digest 和 source mapping 从既有记录及上下文派生，不能在引用中维护第二份可独立修改的事实。U0 只增加类型化投影、验证和来源合同；新增 cooked payload、named binding 和资源就绪证明留给 U1。

### 5.2 Transform

authoring 层需要显式区分 `Transform2D` 和 `Transform3D`，但 Runtime 层可由统一 world transform 传播模块消费。

本方案建议冻结以下默认约定：

```text
右手坐标系
Y 轴向上
单位为米
相机默认朝 -Z
authoring 可使用 Euler degrees
runtime canonical form 使用 quaternion + world matrix
```

这些约定适用于新的 typed 3D 合同。既有 Transform/Sprite2D 保留原数值、单位、旋转和 scale 解释，通过显式 legacy metadata 区分；U0 不将既有 2D 项目自动改为米制，不改写全局 Transform 或 hierarchy 传播。外部 glTF、Unity、UE 资产在后续 Import 阶段完成坐标、单位、前向方向和 winding 转换。最终取值和兼容边界由 U0 Gate 冻结。

### 5.3 RenderInstance

```text
RenderInstance:
  source_entity_id
  runtime_entity_id
  render_proxy_id
  world_transform
  previous_world_transform
  local_bounds
  world_bounds
  authored_visibility
  inherited_visibility
  layer_set
  geometry: GeometryBinding
  material: MaterialBinding
  provenance
```

`RenderInstance` 不包含 Sprite 专属字段，也不直接携带 GPU object。它只表示某个稳定实体在当前帧的 render-facing 身份和引用。

`ViewVisibility` / `CullingDecision` 是以 `(view_id, instance_id)` 为键的派生结果，不是实例的全局布尔字段。U0 对尚未计算的 inherited visibility、culling 和真实 bounds 使用显式 `not_evaluated` / `legacy_approximation`，不得伪报可见、剔除完成或真实 Mesh AABB。

### 5.4 GeometryBinding

```text
GeometryBinding =
    GeneratedSpriteQuad {
        texture: AssetRef<Texture>,
        atlas_region,
        flip,
        color
    }
  | MeshBuffer {
        mesh_asset: AssetRef<Mesh>,
        submesh_index,
        geometry_handle,
        local_bounds
    }
  | SkinnedMesh {
        mesh_asset,
        skeleton_asset,
        skin_binding
    }
  | ParticleGeometry {
        effect_asset,
        mesh_or_billboard_policy
    }
```

`GeometryBinding` 必须携带 vertex layout、topology、index format、submesh range、local bounds 和 cooked payload identity。数量字段不能替代真实 buffer payload。

### 5.5 MaterialBinding

```text
MaterialBinding:
  material_asset: AssetRef<MaterialPreset>
  parameter_block: typed scalar/vector values
  texture_slots: named TextureSlotBinding[]
  blend_policy
  cull_policy
  depth_policy
  fallback_policy
```

项目和 AI 面向 `unlit-basic`、`sprite-alpha`、`pbr-basic` 等 preset；`shader_key` 是内部派生结果，不能作为项目唯一真相。每个 texture slot 必须有名字、期望类型、实际资源身份和 fallback 状态。

### 5.6 RenderView

```text
RenderView:
  view_id
  source_camera_entity_id
  domain: World2D | World3D | UI
  projection:
    Orthographic { size, near, far }
    Perspective { fov_y, near, far }
  viewport
  render_target
  clear_policy
  depth_policy
  layer_set
  coordinate_convention
  view_matrix / projection_matrix: derived only
```

AI 修改 projection 参数，不直接写 view/projection matrix。矩阵由 Camera Projection Module 派生并记录来源参数。

U0 兼容旧 `RenderViewState` 时允许内部 `LegacyMatrix` 分支，原样保存矩阵、view kind、target、viewport、layer mask 和相机来源；不得从任意矩阵猜测 FOV/near/far。新的 Orthographic/Perspective 参数在 U0 只验证合同，实际矩阵生产与默认 consumer 切换属于 U2。

### 5.7 Visibility 与 RenderPolicy

可见性分为四层：

```text
AuthoredVisibility
InheritedVisibility
ViewVisibility
CullingDecision
```

`layer_set` 使用稳定命名集合，编译成内部 mask；不在 authoring 合同中直接要求 AI 写位掩码。

排序策略类型化：

```text
SortPolicy =
    SpriteLayer { sorting_layer, order_in_layer, stable_entity_id }
  | Opaque { pipeline_key, material_id, mesh_id, depth_bucket, stable_entity_id }
  | Transparent { camera_depth, material_id, stable_entity_id }
  | UIComposition { stage, order }
```

3D 不能复用 Sprite 的 quantized z 排序。所有排序都必须有 stable tie-break，不能只依赖浮点深度。

上述排序是目标合同；旧 Sprite 的兼容策略保留 `sort_z == 0` 时使用 transform.z、千分量化和最终 `proxy_id` tie-break，不在 U0 改成 source entity ID。后续策略迁移必须有独立证据。

### 5.8 Provenance 与诊断

每个 RenderInstance、AssetBinding、RenderView 和 RenderPass 都应能回溯：

```text
project_revision
source_path
source_component
source_entity_id
asset_id / content_digest
operation_id
producer_stage
```

统一诊断最小合同：

```text
Diagnostic:
  schema_version
  operation_id
  project_revision
  domain
  stage
  code
  source_path / symbol
  entity_id / asset_id
  expected
  actual
  dependency_chain
  fallback
  retryability
  next_action
```

Runtime 默认 `Off`，只保留功能必需的 compact result；显式诊断请求的 `Summary` 只返回有界高价值错误，`Trace` 才展开 RenderProjection、culling、RenderGraph、RHI command 和资源生命周期，不在每帧默认生成 JSON 或长字符串。任何 fallback 必须标记为 `accepted`、`degraded` 或 `unsupported`。

## 6. 统一资产生产与绑定

正式链路：

```text
Source Asset
  -> Importer
  -> ImportReport
  -> Cooked Asset
  -> RuntimePackage Manifest
  -> RuntimeRenderAssetProducer
  -> Typed RenderAsset
  -> RenderResourceManager
  -> RenderResourceHandle
  -> Named RenderBindingSet
  -> RhiCommandPlan
```

### 6.1 Mesh

Mesh cooked payload 最少必须包含：

```text
vertex bytes
index bytes
vertex layout
topology
index format
submesh ranges
local AABB / sphere
coordinate metadata
material slot names
source/cooked digest
```

`vertex_count` 和 `index_count` 只能作为校验字段，不能由 byte length 推导出假数据。

### 6.2 Material

Material cooked payload 最少必须包含：

```text
material preset id
typed parameter schema
named texture slots
blend/cull/depth policy
shader source mapping
derived pipeline identity
unsupported parameter diagnostics
```

### 6.3 2D 资产复用

Sprite texture、AUI image、Font atlas 和 Particle texture 继续使用统一 Texture resource production，只通过 usage、typed payload 和 composition stage 区分用途。不能为每种纹理新增独立 GPU 上传链。

## 7. RenderPhase 与 RenderGraph

内部使用分域 phase，但共享 RenderGraph/RHI：

```text
Core2D
Core3D
Shadow（后续）
Particle
UI BeforeWorld
UI ScreenOverlay
UI Modal
Present
```

每个 phase 由引擎从高层 RenderInstance、RenderView 和 RenderFeaturePreset 生成。AI 不直接写 RenderGraph。

中立的 DrawGeometry 命令至少需要：

```text
view_id
target
geometry_binding
material_binding
world_transform
depth/blend/cull policy
sort key
provenance
```

旧 `DrawSpriteBasic`、`DrawSpriteTextured`、`DrawMeshBasic` 和 UI command 在迁移期间继续保留，通过 converter 进入新的中立合同。只有新合同拥有真实 Mesh payload 和 Depth 证据后，才允许逐步切换默认路径。

## 8. AI 友好设计

### 8.1 AI 可编辑面

AI 只编辑：

```text
Scene / Prefab typed components
AssetRef
MaterialPreset
CameraPreset
LightPreset
Field-level ProjectPatch
```

AI 不编辑：

```text
RenderProxy
RenderGraph
RhiCommandPlan
GPU handle
bind group
raw projection matrix
binary mesh payload
```

### 8.2 AI 读取面

沿用现有 typed tool surface 的 inspect/check/playtest/observe/capture 语义，增加结构化 domain 输出，而不是新增一套独立工具：

```text
inspect:
  scene entity / camera / asset / bounds / visibility / provenance

check:
  source -> import -> cook -> RuntimePackage -> binding -> phase -> present

observe:
  stable entity transform / camera state / culling reason / material status

capture:
  screenshot / pixel probe / render graph summary / identity
```

### 8.3 AI 修复闭环

```text
inspect context
  -> field patch
  -> check
  -> build/run
  -> observe/capture
  -> diagnostic-driven repair
```

每个结果必须绑定同一 project revision、delivery identity、runRef 和 operationId，避免 AI 将不同交付或不同资产版本的截图混在一起。

## 9. 分阶段迁移计划

### U0：合同冻结与兼容 Adapter

目标：建立不改变现有 2D/AUI 行为的中立类型。

范围：

- 冻结内部 `render-contract.v1` schemaVersion，不连带升级 RuntimePackage、SDK、ABI 或九项工具协议；
- 复用现有 AssetRef，增加 typed reference 投影、Transform/coordinate metadata、RenderView、Visibility、RenderPolicy、Provenance、Diagnostic；
- 增加 `RenderProjectionAdapter<SpriteRenderer2D>`，接入现有 RenderProjection 的 Sprite descriptor 生产点，继续由旧 command/consumer 执行；
- 保留旧序列化键、旧 RenderPayloadKind 和旧 2D command；
- 建立旧 2D 可表达兼容子集内的双向无损转换测试；新 Perspective、多层集合等无法无损还原的值必须显式拒绝降级。旧空引用/无效引用保留旧诊断路径，不因新增 strict schema 而静默丢实体或崩溃。

禁止：

- 不在 U0 直接实现 Depth、PBR、glTF 或 Physics3D；
- 不切换默认 2D RenderGraph；
- 不修改 356 AUI composition owner。

### U1：统一 Asset / Geometry / Material Handle

目标：让 Sprite texture、Mesh geometry、Material、Font atlas 和 RenderTarget 使用同一类生产、生命周期和报告。

范围：

- typed GeometryHandle、MaterialHandle、TextureHandle；
- named RenderBindingSet；
- Mesh/Material cooked payload 的真实字段；
- Import/Cook/Runtime manifest 和 digest；
- 资源缺失、类型不符、fallback、stale handle 报告。

### U2：统一 View / Depth / RenderGraph / RHI

目标：让 Core2D、Core3D、UI 通过同一 RenderGraph/RHI 合同执行，但保持域内语义独立。

范围：

- Camera2D/Camera3D typed projection；
- depth target 和 depth policy；
- per-view visibility/culling summary；
- typed DrawGeometry converter；
- HeadlessRhiBackend 与 WGPU backend 同时消费中立合同；
- 旧 DrawSprite/DrawMesh command 兼容桥。

### A：3D Vertical Slice

U0-U2 通过后才执行原方案 A：

```text
generated cube/plane
  -> Perspective Camera
  -> real vertex/index buffer
  -> Depth
  -> Unlit Material
  -> 2D/3D same RenderGraph/RHI
  -> glTF/GLB static mesh
  -> observe/check/capture
```

A 不是本文的施工范围，只是本文合同的第一个真实 3D consumer。独立正式设计见[359：3D静态场景与AI项目闭环方案A](359-3D静态场景与AI项目闭环-方案A-v1方案.md)，包含场景组件、图元/模型导入、SDK与观察、普通项目及独立交付的A0–A4路线。

## 10. 兼容性和不变量

1. 现有 2D Sprite、AUI、Particle、Font 和 UI composition 的输出不因 U0/U1/U2 自动改变。
2. 旧 RuntimePackage 和旧序列化在迁移期可被读取；新合同通过显式 schemaVersion 区分。
3. AUI 不进入 World 2D/3D 的 depth sort，也不被改造成 ECS Entity。
4. Project Module 不直接创建 GPU resource；GPU 生命周期归 RenderResourceManager/Backend。
5. Renderer 不读取项目源文件、主题继承或业务规则；它只消费 RuntimePackage 派生产物。
6. Physics2D/Physics3D、Navigation2D/Navigation3D 仍是独立深模块，共享 identity、Transform 和 query report，不共享内部算法。
7. 所有新增字段必须有 source mapping、default、invalid input 诊断和兼容策略。
8. 不以 headless 通过代替真实 GPU 画面，也不以截图代替资源、视图和实体状态证据。

## 11. 计划 Gate

这些 Gate 是方案的资格定义，不表示当前已经通过：

| Gate | 目的 | 最小证据 |
| --- | --- | --- |
| G0 Contract | Schema、owner、版本和兼容边界正确 | schema fixture、unknown/invalid rejection、旧 2D decode |
| G1 2D Compatibility | 统一层不破坏已完成项目 | Neon/Tower/Shooter/AUI 既有 headless、截图和 observe 证据 |
| G2 Asset Binding | 资源 identity 和 named binding 正确 | Texture/Mesh/Material/Font/RenderTarget producer report |
| G3 View/Phase | Camera、visibility、depth 和 phase 可解释 | 2D orthographic + 3D perspective view report、culling reason |
| G4 RHI | backend-neutral command 可被 Headless/WGPU 消费 | command plan、resource lifetime、backend report |
| G5 AI Loop | AI 可通过高层 patch 完成诊断闭环 | inspect -> patch -> check -> observe/capture，同一 identity |
| G6 A-Ready | 统一合同可以承载方案 A | generated cube/plane fixture 的 contract-only dry run |

每个 Gate 必须只验证一个风险合同。U0-U2 施工时不得把 Physics3D、PBR、Android 或完整 3D Editor 作为隐含 Gate。

G0–G6 是全方案能力矩阵，不要求每一阶段重跑整套。U0 的终态限于合同与受影响 headless consumer 兼容，旧 GPU 截图只作比较基线或原二进制历史证据；若声明新二进制视觉等价，须另取得该二进制及对应项目/配置的截图证据。Tower P1-2 保持停止，不因引用兼容基线恢复玩法施工。

## 12. 风险与处理

### 12.1 统一结构变成万能 optional object

处理：authoring 使用 discriminated typed schema；Geometry、Material、SortPolicy、Projection 均使用 tagged union；内部共享字段和生命周期，域专属字段放在 Adapter 和 phase 内。

### 12.2 迁移破坏现有证据

处理：新增中立类型和 compat converter，保留旧序列化、旧 command 和旧 2D pipeline；只有新合同有独立证据后才切换默认路径。

### 12.3 AI 看到的接口仍然过低

处理：渲染内容面向 Scene/Prefab/AssetRef/Preset/Field Patch；RenderGraph、RHI 和 GPU handle 不成为普通项目 authoring 输入。正常 Rust 项目代码和已批准的 WGSL 扩展边界保持。

### 12.4 3D 坐标和深度错误难定位

处理：在合同中冻结坐标、单位、clip depth、winding、near/far、透明排序和浮点容差；observe 输出 camera、world transform、bounds、culling reason 和 depth policy。

### 12.5 资源绑定报告不可读

处理：所有 binding 使用 named slot、expected kind、actual identity、fallback 和 dependency chain；禁止无语义 `Vec<RenderResourceHandle>` 作为最终 AI 合同。

## 13. 方案自审

- 目标是否与用户选择一致：是，先统一 2D/3D 合同，再承载方案 A。
- 是否把 2D/3D 错误合并为一个万能组件：否，authoring、Geometry、Projection、SortPolicy 和 RenderPhase 保持类型化分域。
- 是否破坏 353/356 已完成能力：设计上不破坏，要求通过 compat Adapter 和 G1 回归证明。
- 是否把 Mesh descriptor 当成真实 3D 能力：否，明确要求真实 vertex/index/submesh payload 和 binding report。
- 是否把 AI 暴露到 GPU/RHI：否，普通渲染 authoring 使用 typed schema、AssetRef、Preset、Field Patch 和结构化结果，同时保留顶层架构允许的项目代码与扩展接口。
- 是否把未来 PBR、Physics3D、Navigation、Android 偷渡为当前 Gate：否，全部列为后续 consumer 或后续资格范围。
- 是否已经施工：U0–U2已完成，包含真实几何/深度/相机的离屏WGPU证据；方案A项目纵切未执行，默认安装未更新，不声称完整3D产品能力。

## 14. 下一步

用户授权后，U0、U1及[U2施工](施工文档/已完成/358-U2-View-Depth-RenderGraph-RHI施工文档.md)均已完成归档。U2在U1资产基础上接通View/Depth/DrawGeometry及现有RenderGraph/RHI，121项相关测试通过；[实际合同](358-U2-View-Depth-DrawGeometry实际合同.md)明确legacy、逐view深度、来源身份和项目消费边界。原方案A现已展开为[359独立正式方案](359-3D静态场景与AI项目闭环-方案A-v1方案.md)，普通场景组件、generated cube/plane、glTF与语义验收不能以底层离屏测试替代；后续按359生成分阶段施工文档，执行槽见54。

2026-09-22 U0 施工准备复审：先吸收现有 AssetRef owner、兼容子集、旧坐标/排序、per-view visibility、Runtime Off 和证据身份等澄清，再生成施工文档；用户已选定的 U-A + U-C 路线不变。
