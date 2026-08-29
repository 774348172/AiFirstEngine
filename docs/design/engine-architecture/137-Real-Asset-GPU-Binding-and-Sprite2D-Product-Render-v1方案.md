# 137-Real Asset GPU Binding and Sprite2D Product Render v1 方案

## 1. 本文解决什么问题

本文把 `130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md` 中的两个 P0 缺口收敛成一个大系统：

```text
M4 Runtime Asset Cook -> GPU Resource Binding
M5 Sprite2D 产品级运行链路
```

原因是这两件事不能继续分开做。Sprite2D 如果不能绑定真实图片、材质和 GPU 资源，只能画测试色块；GPU Resource Binding 如果不服务于真实 Sprite2D、AUI、Mesh 等绘制链路，也只会停留在 descriptor / smoke 层。

目标链路：

```text
Imported Asset
  -> Cooked Asset
  -> RuntimeAssetIndex
  -> RuntimeAssetLoader
  -> RuntimeAssetTypeLoader
  -> RenderAssetPrepare
  -> RenderResourceManager
  -> RenderResourceHandle
  -> SpriteMaterialBinding
  -> Sprite2D DrawPlan
  -> EngineRHI
  -> WgpuBackend
  -> Surface Present
```

第一版不追求完整商业渲染器，但必须走长期正确架构。不能再让 WGPU backend 直接猜 `sprite_ref` 文件路径，也不能让 Sprite2D 绕过资源系统直接创建 GPU 对象。

## 2. 已有规则和当前缺口

已有规则：

```text
06-资源系统架构.md
  已定义 RuntimeAssetIndex / RuntimeAssetLoader / RuntimeAssetHandle。

07-Build-Export-Pipeline.md
  已定义 Build / Cook / RuntimePackage / Asset Cook Report 边界。

80-GPU-Resource-Pool-RenderResourceLifetime-v1方案.md
  已定义 RenderResourceManager / RenderResourceHandle / lifetime report。

96-Sprite2D-Rendering-C-min方案.md
  已定义 SpriteRenderer2D / SpritePayload / sort_key 的 CPU/headless 链路。

109-SpriteRenderer2D-ECS-to-RenderProxy-Bridge-C-min方案.md
  已定义 SpriteRenderer2D 通过 RenderProjectionAdapter 进入 RenderProxy。

110-World-Projection-Adapter统一跨域同步规则.md
  已规定不再新增零散 Bridge，统一使用 Projection / Adapter 心智模型。

115-EngineRHI-Trait与RuntimeRenderer迁移方案.md
116-真实WgpuBackend完整v1方案.md
117-Runtime-WGPU-Surface注入-WindowedPlayerPresent-v1方案.md
118-WindowedPlayer-Runtime-v1完整方案.md
  已定义 RuntimeRenderer / EngineRHI / WgpuBackend / Surface Present 的长期方向。

136-M6-RuntimePackage-InputMapping-Resource-Loading-v1方案.md
  已补 RuntimePackage 输入映射和资源加载基础。
```

当前缺口：

```text
RuntimePackage 里有 RuntimeAssetIndex，但 Sprite2D 绘制还没有产品级真实纹理绑定。
RenderResourceManager 已有 lifetime 模型，但 RuntimeAsset -> RenderResource 的准备层还不完整。
RuntimeRenderer 能生成 sprite pass，但 DrawSpriteBasic 仍偏 smoke / descriptor。
WindowedPlayer 有 GPU binding summary，但还不是完整资源上传、绑定和绘制闭环。
缺失资源时缺少能让用户和 AI 快速定位的统一错误字段。
```

## 3. 其它引擎怎么做

### 3.1 Unity

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\2D\Common\ScriptBindings\Sprites.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\2D\SpriteAtlas\ScriptBindings\SpriteAtlas.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector\SpriteRendererEditor.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\Graphics\Texture.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\Graphics\Texture.bindings.cs
```

Unity 用户侧看到的是：

```text
SpriteRenderer.sprite
Sprite.texture
Texture2D
Material
sortingLayer / sortingOrder
```

真正 GPU 资源创建和绑定主要在 native engine 内部。优点是用户层很简单；缺点是对 AI 和引擎调试不够透明，资源为什么没有显示、为什么还没释放、为什么换图后没刷新，通常要依赖 Unity 内部工具。

对我们的启发：

```text
用户和 AI 应该只改 SpriteRenderer2D / AssetRef / MaterialRef。
项目逻辑不应该直接接触 GPU resource。
但我们不能完全黑盒化，必须有 RuntimeAsset / RenderAsset / RenderResource report。
```

### 3.2 Unreal Engine

本地源码参考方向：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Classes\Engine\Texture2D.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Texture2D.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\RenderCore\Public\RenderResource.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\RenderCore\Private\RenderResource.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\2D\Paper2D
```

UE 的核心分层：

```text
Game Thread UObject / UTexture / UMaterial / Component
  -> Render Thread SceneProxy / FRenderResource
  -> RHI Resource
```

UE 不让游戏逻辑直接操作 RHI 纹理。资源通过 render command / render thread 初始化和释放，底层资源生命周期用 `FRenderResource` 和 RHI 体系管理。

对我们的启发：

```text
RuntimeAsset 和 RenderResource 必须分层。
RenderResourceManager / RenderThread 才能持有 GPU-side resource。
Game / Runtime World 只能产生渲染意图和 AssetRef，不能直接创建 WGPU texture。
```

### 3.3 Bevy

本地源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_sprite\src\sprite.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_render\src\render_asset.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_render\src\texture\gpu_image.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_render\src\render_resource\bind_group.rs
```

Bevy 的模式非常接近我们：

```text
Main World Sprite { image: Handle<Image> }
  -> Extract
  -> PrepareAssets
  -> RenderAssets<GpuImage>
  -> bind group / draw
```

它把 CPU asset 和 GPU representation 明确分开，并有 RenderAsset prepare 阶段。这个方向适合 Rust / ECS，但 Bevy 的报告默认不够 AI-first，需要我们补齐更强的诊断字段。

### 3.4 Godot

本地源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\2d\sprite_2d.cpp
<GODOT_SOURCE>\godot-master\godot-master\scene\resources\texture.cpp
<GODOT_SOURCE>\godot-master\godot-master\servers\rendering_server.*
<GODOT_SOURCE>\godot-master\godot-master\servers\rendering\renderer_rd\storage_rd\texture_storage.cpp
```

Godot 的模式：

```text
Scene Node / Resource
  -> RenderingServer
  -> RID
  -> backend resource
```

Godot 通过 RenderingServer / RID 隔离场景对象和底层渲染资源。它的资源服务边界很清楚，但 RID 概念如果直接暴露给项目规则，对 AI-first 项目会偏底层。

## 4. 方案对比

### 方案 A：继续保留 descriptor / smoke 资源绑定

```text
SpriteRenderer2D.sprite_ref
  -> RuntimeRenderer 记录 sprite_ref
  -> WgpuBackend 画测试 quad / 测试颜色
```

优点：

```text
改动最少。
现有测试容易通过。
```

缺点：

```text
不能显示真实图片。
不能证明 Windows 导出包真的可玩。
AI 无法定位资源到底是 import、cook、load、decode、upload 还是 bind 失败。
```

结论：

```text
不采用。它只能作为历史 smoke gate，不再作为产品级路线。
```

### 方案 B：WgpuBackend 直接按 sprite_ref 加载图片

```text
SpriteRenderer2D.sprite_ref
  -> RuntimeRenderer
  -> WgpuBackend 直接读取文件 / 解码图片 / 创建 texture
```

优点：

```text
第一版看起来最快。
代码路径短。
```

缺点：

```text
破坏 RuntimeAssetIndex 是唯一运行时资源索引真相的规则。
WGPU backend 会知道项目资产路径、bundle、cook 规则，耦合严重。
以后换 D3D12 / Vulkan / Metal backend 会重复资源加载逻辑。
错误会散在 backend 里，AI 很难判断是哪一层失败。
```

结论：

```text
不采用。这个方案短期诱人，但长期会把资源系统和渲染 backend 粘死。
```

### 方案 C-min：RuntimeAsset -> RenderAssetPrepare -> RenderResourceManager

```text
SpriteRenderer2D / AssetRef
  -> RenderProxyPayload::Sprite
  -> RendererFeatureBuilder
  -> RenderAssetPrepareRequest
  -> RuntimeAssetLoader / RuntimeAssetTypeLoader
  -> PreparedRenderAsset
  -> RenderResourceManager
  -> RenderResourceHandle
  -> SpriteMaterialBinding
  -> EngineRHI
  -> WgpuBackend
```

优点：

```text
接近 UE 的 Game Asset / Render Resource 分层。
接近 Bevy 的 Extract / Prepare / RenderAssets 模式。
AI 能沿 RuntimeAssetIndex、prepare report、resource lifetime report 查问题。
WGPU backend 不知道 AssetRef，只执行 RHI 和 GPU resource 创建。
后续 Mesh、AUI、Particle、Font atlas 也可以复用同一套 prepare/resource 结构。
```

缺点：

```text
比方案 B 多一个 RenderAssetPrepare 层。
第一版需要补清楚 request、prepared asset、binding、report。
```

结论：

```text
推荐采用。
```

### 方案 D：第一版直接做完整商业渲染资源系统

范围包括：

```text
Texture streaming
Mip streaming
Atlas packing
Material graph
Pipeline cache
Descriptor allocator
Virtual texture
Async streaming budget
GPU residency
```

优点：

```text
长期最完整。
```

缺点：

```text
第一版过大。
会把现在目标从“打通可玩 Windows 包”拖成完整 renderer 研发。
也会增加大量 AI 和人类都难维护的规则。
```

结论：

```text
不采用。第一版只留下可扩展位置，不实现完整商业特性。
```

## 5. 正式推荐方案

采用方案 C-min：

```text
RuntimeAssetIndex 是运行时资产索引真相。
RuntimeAssetLoader 是 CPU-side runtime asset 加载入口。
RuntimeAssetTypeLoader 是 typed runtime resource 解码入口。
RenderAssetPrepare 是 CPU asset 到 render resource request 的准备层。
RenderResourceManager 是 GPU resource lifetime 真相。
RenderResourceHandle 是 renderer / RHI 可引用的资源句柄。
WgpuBackend 只创建和使用 backend resource，不解析 AssetRef。
```

一句话版本：

```text
Sprite2D 只说“我要画这个 AssetRef”；
资源系统负责把 AssetRef 变成 RuntimeAsset；
RenderAssetPrepare 负责把 RuntimeAsset 变成 RenderResourceHandle；
RHI/backend 只负责把 RenderResourceHandle 对应的 GPU 对象画出来。
```

## 6. 系统边界

### 6.1 项目侧可以表达什么

项目侧和 AI patch 只能表达：

```text
SpriteRenderer2D.sprite_ref
SpriteRenderer2D.material_ref
SpriteRenderer2D.color
SpriteRenderer2D.visible
SpriteRenderer2D.sorting_layer
SpriteRenderer2D.order_in_layer
SpriteRenderer2D.sort_z
```

项目侧不能表达：

```text
wgpu::Texture
wgpu::BindGroup
RenderResourceHandle
BackendResource
GPU upload command
RHI pipeline object
```

### 6.2 引擎侧真相层

```text
Authoring truth:
  Project Asset / Scene / Prefab / SpriteRenderer2D

Runtime asset truth:
  RuntimePackage / RuntimeAssetIndex / RuntimeAssetHandle

Render resource truth:
  RenderAssetPrepareReport / RenderResourceManager / RenderResourceHandle

Frame draw truth:
  RenderFramePacket / RendererFeatureDrawItem / DrawPlan / RHI command plan

Backend truth:
  WgpuBackend internal texture / sampler / bind group / pipeline
```

### 6.3 不再新增零散 Bridge

历史名词 `RenderAssetBridge` 统一归入：

```text
AssetProjection / RenderAssetPrepare
```

SpriteRenderer2D 仍归入：

```text
RenderProjectionAdapter<SpriteRenderer2D>
```

后续新增 MeshRenderer、ParticleRenderer、AUI Image、FontAtlas 时，只新增对应 typed adapter / prepare handler，不新增平行的独立桥系统。

## 7. v1 数据结构

### 7.1 RenderAssetPrepareRequest

```text
RenderAssetPrepareRequest:
  request_id: String
  frame_index: u64
  asset_ref: String
  expected_asset_type: RuntimeAssetType
  usage: RenderAssetUsage
  source_entity_id: Option<EntityId>
  source_proxy_id: Option<RenderProxyId>
  source_component: Option<String>
  material_ref: Option<String>
```

```text
RenderAssetUsage:
  SpriteTexture
  SpriteMaterial
  MeshGeometry
  AuiTexture
  FontAtlas
```

第一版重点实现：

```text
SpriteTexture
SpriteMaterial
```

### 7.2 PreparedRenderAsset

```text
PreparedRenderAsset:
  asset_ref: String
  asset_id: String
  cooked_asset_id: String
  resource_kind: RenderResourceKind
  resource_handle: Option<RenderResourceHandle>
  status: PreparedRenderAssetStatus
  byte_size: u64
  version: u64
```

```text
PreparedRenderAssetStatus:
  Ready
  Deferred
  MissingRuntimeAsset
  UnsupportedFormat
  DecodeFailed
  UploadFailed
  Failed
```

### 7.3 SpriteMaterialBinding

```text
SpriteMaterialBinding:
  texture: Option<RenderResourceHandle>
  material: SpriteMaterialHandle
  sampler: SpriteSampler
  blend_mode: SpriteBlendMode
  fallback_used: bool
```

```text
SpriteMaterialHandle:
  DefaultSpriteBasic
  RenderResource(RenderResourceHandle)
```

```text
SpriteSampler:
  LinearClamp
  NearestClamp
```

```text
SpriteBlendMode:
  Opaque
  AlphaBlend
```

第一版规则：

```text
没有 material_ref 时使用 DefaultSpriteBasic。
没有 sampler 设置时使用 LinearClamp。
Sprite 默认 AlphaBlend。
缺失 texture 时 editor/dev 可显示 placeholder，release 是否 fail 由 BuildProfile 决定。
```

### 7.4 RenderAssetPrepareReport

```text
RenderAssetPrepareReport:
  frame_index: u64
  request_count: usize
  ready_count: usize
  deferred_count: usize
  failed_count: usize
  uploaded_bytes: u64
  events: Vec<RenderAssetPrepareEvent>
```

```text
RenderAssetPrepareEvent:
  request_id: String
  asset_ref: String
  asset_id: Option<String>
  cooked_asset_id: Option<String>
  stage: RenderAssetPrepareStage
  severity: DiagnosticSeverity
  code: RenderAssetPrepareCode
  message: String
  source_entity_id: Option<EntityId>
  source_proxy_id: Option<RenderProxyId>
```

```text
RenderAssetPrepareStage:
  ResolveAssetRef
  LoadRuntimeAsset
  DecodeRuntimeAsset
  CreateRenderResourceRequest
  UploadGpuResource
  BindMaterial
```

```text
RenderAssetPrepareCode:
  MissingAssetRef
  MissingRuntimeAssetIndexEntry
  MissingCookedAsset
  UnsupportedTextureFormat
  DecodeFailed
  UploadBudgetDeferred
  RenderResourceCreateFailed
  MissingMaterial
  FallbackMaterialUsed
  Ready
```

## 8. 运行流程

### 8.1 正常 Sprite2D 绘制流程

```text
RuntimePackage load
  -> RuntimeAssetIndex ready

RuntimeScene Hydration
  -> World.insert SpriteRenderer2D(sprite_ref="assets/player.png")

RenderProjectionAdapter<SpriteRenderer2D>
  -> RenderProxyPayload::Sprite

RendererFeatureBuilder
  -> SpriteDrawItem(sprite_ref, material_ref, sort_key)
  -> RenderAssetPrepareRequest(SpriteTexture)

RenderAssetPrepare
  -> RuntimeAssetIndex.resolve(sprite_ref)
  -> RuntimeAssetLoader.load(cooked_asset_id)
  -> RuntimeAssetTypeLoader.decode(texture)
  -> RenderResourceRequest(Texture)
  -> RenderResourceManager.create_or_reuse
  -> PreparedRenderAsset(resource_handle)

RuntimeRenderer
  -> SpriteMaterialBinding(texture handle + default material)
  -> DrawSpriteTextured command

EngineRHI / WgpuBackend
  -> bind texture / sampler / material
  -> draw quad
  -> present
```

### 8.2 缺失资源流程

```text
SpriteRenderer2D.sprite_ref = "missing/player.png"
  -> RuntimeAssetIndex resolve failed
  -> RenderAssetPrepareReport event:
       stage = ResolveAssetRef
       code = MissingRuntimeAssetIndexEntry
       source_entity_id = entity id
       source_proxy_id = proxy id
  -> Editor/dev profile 可画 placeholder
  -> Release profile 可按 BuildProfile 策略 fail build 或 fail runtime load
```

关键规则：

```text
缺失资源不能静默变成白块。
placeholder 只能伴随 diagnostic。
AI 默认先读 summary，再按 asset_ref / entity_id 查 detail。
```

### 8.3 热更新 / 资源替换流程

第一版只保留正确边界，不实现复杂热更：

```text
new RuntimeAssetIndex fragment mounted
  -> asset version changes
  -> RenderAssetPrepare creates new RenderAssetKey
  -> RenderResourceManager creates new generation
  -> old generation PendingRelease
  -> GPU safe frame 后释放
```

规则：

```text
不允许原地覆盖正在被 GPU 使用的资源。
资源替换失败时继续使用旧 generation，并输出 diagnostic。
```

## 9. Sprite2D 产品级 v1 范围

必须支持：

```text
真实图片作为 Sprite2D texture。
RuntimePackage 中的 sprite_ref 通过 RuntimeAssetIndex 解析。
SpriteRenderer2D.color tint。
visible。
flip_x / flip_y。
sorting_layer / order_in_layer / sort_z / stable_proxy_id 稳定排序。
AlphaBlend。
DefaultSpriteBasic material。
资源缺失 / 解码失败 / 上传失败 report。
headless tests。
feature-gated real WGPU smoke。
```

第一版不支持：

```text
SpriteAtlas packing。
九宫格。
复杂 Sprite slicing。
Material Graph。
Shader Graph。
自定义 Sprite shader。
Texture streaming。
Mip streaming。
GPU instancing batch。
多相机复杂透明排序。
Lighting 2D。
Particle 专用渲染。
```

这些不支持项必须留扩展位置，但不能拖慢第一版真实图片显示闭环。

## 10. AI 友好规则

AI 修改可见对象时，只能生成类似 patch：

```text
SetComponentField(entity, "SpriteRenderer2D.sprite_ref", "asset://player")
SetComponentField(entity, "SpriteRenderer2D.color", [1.0, 1.0, 1.0, 1.0])
SetComponentField(entity, "SpriteRenderer2D.order_in_layer", 10)
```

AI 不能生成：

```text
CreateWgpuTexture(...)
CreateBindGroup(...)
SetRenderResourceHandle(...)
PatchBackendResource(...)
```

AI 查问题的固定路径：

```text
1. 查 SpriteRenderer2D.sprite_ref 是否存在。
2. 查 RuntimeAssetIndex 是否能解析 asset_ref。
3. 查 RuntimeAssetLoader 是否加载 cooked asset。
4. 查 RuntimeAssetTypeLoader 是否 decode 成 texture/material。
5. 查 RenderAssetPrepareReport 是否生成 resource request。
6. 查 RenderResourceLifetimeReport 是否 Resident。
7. 查 RuntimeRenderer / RHI command plan 是否有 DrawSpriteTextured。
8. 查 WgpuBackend report 是否 present 成功。
```

这条路径必须写进报告字段和测试命名里，避免后期人和 AI 都不知道从哪里查。

## 11. 和其它引擎方案对比

| 项目 | Unity | UE | Bevy | Godot | 我们 |
|---|---|---|---|---|---|
| 用户侧表达 | SpriteRenderer / Sprite / Texture2D | Component / UTexture / UMaterial | Sprite + Handle<Image> | Sprite2D + Resource | SpriteRenderer2D + AssetRef |
| GPU 资源所有权 | native engine 隐藏 | RenderThread / RHI | Render World / RenderAssets | RenderingServer / RID | RenderResourceManager / EngineRHI |
| 资源准备层 | native 隐藏 | render command / resource init | Extract / PrepareAssets | Resource -> Server | RenderAssetPrepare |
| AI 可查性 | 弱 | 中等，工具强但复杂 | 中等 | 中等 | 强，报告字段固定 |
| 第一版复杂度 | 对我们不可复用 | 完整照搬太重 | 适合借鉴 | handle 思路适合借鉴 | C-min，保留长期边界 |
| 长期复杂项目 | 强 | 很强 | 中强 | 强 | 目标强 |

结论：

```text
我们的方案更接近 Bevy 的 RenderAsset prepare 心智模型，
吸收 UE 的 render resource ownership 纪律，
借鉴 Godot 的间接资源 handle，
保留 Unity 用户侧简单的 SpriteRenderer2D / AssetRef 体验。
```

## 12. 最小测试用例

后续施工文档必须至少包含以下测试。

### 12.1 真实纹理准备成功

```text
输入：
  RuntimeAssetIndex 中有 texture asset。
  SpriteRenderer2D.sprite_ref 指向该 asset。

期望：
  RenderAssetPrepareRequest 生成。
  PreparedRenderAsset.status = Ready。
  RenderResourceManager 中有 Texture resource。
  RHI command plan 中有 DrawSpriteTextured。
```

### 12.2 缺失 texture 可诊断

```text
输入：
  SpriteRenderer2D.sprite_ref 指向不存在 asset。

期望：
  不静默成功。
  RenderAssetPrepareReport 包含 MissingRuntimeAssetIndexEntry。
  event 携带 source_entity_id / source_proxy_id / asset_ref。
```

### 12.3 多 Sprite 稳定排序

```text
输入：
  三个 SpriteRenderer2D 使用不同 sorting_layer / order_in_layer。

期望：
  DrawPlan 按 sort_key 稳定排序。
  相同 sort 字段时用 stable_proxy_id 兜底。
```

### 12.4 默认材质 fallback

```text
输入：
  SpriteRenderer2D 有 sprite_ref，但 material_ref = None。

期望：
  SpriteMaterialBinding.material = DefaultSpriteBasic。
  report 中可记录 FallbackMaterialUsed，severity 不高于 info。
```

### 12.5 Windowed Player 集成

```text
输入：
  RuntimePackage 包含 scene + texture + SpriteRenderer2D。

期望：
  WindowedPlayer load package。
  resource binding summary 显示 texture ready。
  real-window feature 下能 present。
  headless 默认测试不依赖真实 GPU。
```

## 13. 第一版施工边界建议

施工时建议拆成 5 个 gate，每个 gate 单独测试后再进入下一步：

```text
Gate A: RenderAssetPrepare 数据结构和 report。
Gate B: RuntimeAsset texture/material -> RenderResourceRequest。
Gate C: SpriteDrawItem -> SpriteMaterialBinding -> DrawSpriteTextured。
Gate D: WgpuBackend texture/sampler/bind group 最小真实绑定。
Gate E: RuntimePackage / WindowedPlayer 集成和缺失资源诊断。
```

禁止在本系统中顺手实现：

```text
SpriteAtlas。
Material Graph。
粒子系统。
项目玩法逻辑。
打飞机专用 Bullet / Enemy / Health API。
复杂 UI HUD。
```

这些如果需要，必须进入对应系统文档，而不是塞进资源绑定和 Sprite2D 产品链路。

## 14. 完成标准

本系统完成后，必须能证明：

```text
从编辑器导入一张图片。
RuntimePackage 中存在 cooked texture 记录。
Scene 中的 SpriteRenderer2D 引用该 AssetRef。
Windowed Player 加载 RuntimePackage。
RuntimeAssetIndex 解析 AssetRef。
RuntimeAssetLoader 加载 cooked asset。
RenderAssetPrepare 生成 RenderResourceHandle。
RuntimeRenderer 生成 DrawSpriteTextured。
WgpuBackend 用真实 texture 绘制到窗口。
缺失资源时能明确报告失败层级。
```

这才算 M4 / M5 从 smoke 进入产品级 v1。

## 15. 下一步

如果确认本文方案，下一步生成施工文档：

```text
施工文档/当前/137-当前可自动化施工文档-Real-Asset-GPU-Binding-and-Sprite2D-Product-Render-v1.md
```

施工文档必须按 gate 执行：

```text
完成一个 gate
  -> 跑对应最小测试
  -> 更新阶段完成记录
  -> 再进入下一个 gate
```

不要一次性大改到无法定位失败点。
