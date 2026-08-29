# 228-Real Texture Decode / GPU Texture Upload / Sprite Textured Present v1 方案

> 状态：正式方案文档。
> 选择：方案 C-min，长期采用构建期 cooked texture payload + 运行时 GPU upload；本轮只实现复杂打飞机项目所需的最小真实贴图闭环。
> 校准日期：2026-07-09。
> 上游入口：`227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md` 的 P0-1。

## 1. 这个系统是干什么的

一句话：

```text
让 RuntimePackage 里的 texture asset 真正变成 GPU texture，并让 SpriteRenderer2D 在 Windows 窗口里画出真实飞机、敌机、子弹和背景贴图。
```

当前复杂打飞机项目已经能把 Scene / PrefabInstance bake 进 RuntimePackage，也有 `SpriteRenderer2D.spriteRef` 指向：

```text
tex-player-ship
tex-enemy-scout
tex-starfield
tex-bullet
```

但这还不等于屏幕上有真实贴图。现在缺的是：

```text
Authoring texture asset
  -> Build/Cook 解码成稳定 cooked texture payload
  -> RuntimePackage / RuntimeAssetIndex
  -> Runtime texture payload load
  -> RenderResourceManager / WGPU texture upload
  -> Sprite2D textured quad sampling
  -> Present
```

本系统完成后，导出的 Windows 窗口必须能看到真实项目贴图，而不是测试几何、纯色 quad 或无诊断 fallback。

命名澄清：

```text
本文标题里的 Texture Decode 指 Build/Cook 阶段把 PNG 解码为 cooked RGBA payload。
Runtime 阶段只加载 RuntimePackage 内 cooked texture，不做项目源 PNG 解码，不扫描项目源目录。
```

## 2. 其它引擎对标

### 2.1 UE：本轮主要学习对象

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Classes\Engine\Texture2D.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Texture2D.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Public\TextureResource.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\RenderCore\Public\RenderResource.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\RHI\Public\RHICommandList.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\2D\Paper2D\Source\Paper2D\Private\PaperSpriteComponent.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\2D\Paper2D\Source\Paper2D\Private\PaperRenderSceneProxy.cpp
```

UE 的关键链路：

```text
UTexture2D / FTexturePlatformData
  -> UTexture2D::CreateResource / UpdateResourceWithParams
  -> FTexture2DResource / FRenderResource
  -> FRHICommandList::CreateTexture / UpdateTexture2D
  -> FRHITexture

UPaperSpriteComponent
  -> CreateSceneProxy / SendRenderDynamicData_Concurrent
  -> FPaperSpriteSceneProxy / FPaperRenderSceneProxy
  -> GetDynamicMeshElements / FMeshBatch
  -> RHI draw
```

可学习点：

```text
Game/Object/Asset 层不直接创建 GPU texture。
Texture asset 有平台/cooked 数据，真正 GPU resource 在 RenderResource/RHI 层。
Component 只生成 SceneProxy / dynamic render data，不直接画。
RHI command list 才执行 texture create/update。
Sprite 渲染通过 material/render proxy/mesh batch 进入统一渲染流程。
```

不可照搬点：

```text
不照搬 UE 完整 UObject、RenderThread、RDG、Material system、Texture Streaming、Virtual Texture。
不把 UE 的黑盒式调试方式带进本项目；本项目必须保留 AI-first 结构化 report。
不为了本轮 P0-1 引入完整 MeshBatch / MaterialRenderProxy 复杂度。
```

本项目采用 UE-like 分层，但保持 C-min：

| UE | 本项目 C-min |
|---|---|
| UTexture2D / FTexturePlatformData | Texture authoring asset + CookedTexturePayload |
| CreateResource / UpdateResourceWithParams | RuntimeRenderAssetProducer / texture prepare |
| FTexture2DResource / FRenderResource | RenderResourceManager + RuntimeTextureRenderResource |
| FRHITexture | WgpuTextureRecord，backend 内部对象 |
| UPaperSpriteComponent | SpriteRenderer2D |
| FPaperSpriteSceneProxy | RenderProxyPayload::Sprite + Sprite2DRenderPipeline |
| FMeshBatch | Sprite2DDrawPlan / DrawSpriteTextured |

### 2.2 Unity

Unity 对标：

```text
Texture2D / SpriteRenderer / Material / native graphics backend
```

可学习点：

```text
用户层简单，只改 SpriteRenderer.sprite / color / sorting。
GPU upload 和 backend resource 对普通项目逻辑隐藏。
```

不可照搬点：

```text
Unity native 黑盒不适合 AI-first 查错。
本项目必须报告 cook/load/upload/bind/present 每一层状态。
```

### 2.3 Bevy

Bevy 对标：

```text
Image CPU asset
  -> PrepareAssets
  -> RenderAssets<GpuImage>
  -> RenderDevice / RenderQueue
```

可学习点：

```text
Rust/ECS 语境下把 source asset 和 GPU representation 分开。
Image -> GpuImage 的 prepare 模式适合我们。
```

不可照搬点：

```text
不引入完整 Bevy RenderWorld / Schedule 复杂度。
```

### 2.4 Godot

Godot 对标：

```text
Texture2D / ImageTexture
  -> RenderingServer
  -> RID
  -> RenderingDevice
```

可学习点：

```text
项目层通过 Resource 间接引用 backend resource。
Texture / material / canvas draw 都进入统一 rendering server/storage。
```

不可照搬点：

```text
不把 RID / backend handle 暴露给项目或 AI。
```

## 3. 本项目当前基线

已存在：

```text
RuntimePackage / RuntimeAssetIndex / cooked_asset_table
RuntimeAssetLoader
RuntimeRenderAssetProducer / RenderAssetPrepare 骨架
RenderResourceManager / RenderResourceHandle
SpriteRenderer2D / RenderProxyPayload::Sprite
Sprite2DRenderPipeline
DrawSpriteTextured / RhiCommandPlan
RealWgpuBackend
runtime_player_winit real-window present / screenshot smoke
```

关键缺口：

```text
1. Texture asset 当前多为 `.asset` 文本占位，不是真实图片或图片 descriptor。
2. RuntimeAssetLoader 当前只读取 cooked bytes 并记录 bytes_len，没有真实 texture pixel payload。
3. Sprite2DRenderPipeline 当前总是生成 sprite_binding_fallback。
4. RealWgpuBackend 当前 shader 是纯 color 输出，没有 texture sampler / bind group。
5. RuntimePackage build 当前没有把 PNG cook 成稳定 RGBA texture payload。
6. Report 还不能区分 real_texture_present / fallback / missing / decode_failed / upload_failed。
```

因此，本轮不能只在 WGPU backend 里硬读路径，也不能只改 Sprite2D pass；必须补一条从 Build/Cook 到 Runtime Present 的真实链路。

## 4. 方案选择

### 4.1 方案 A：WGPU backend 直接按 sprite_ref 读项目图片

不采用。

原因：

```text
破坏 RuntimePackage 是 runtime 真相的规则。
backend 会知道项目目录、AssetRef、cook 规则和文件格式。
以后 D3D12 / Vulkan / Metal backend 会重复资源加载逻辑。
AI 无法稳定判断失败发生在 cook、load、decode、upload 还是 bind。
```

### 4.2 方案 B-min：运行时直接 decode PNG，再 upload

不作为本轮最终选择。

优点：

```text
施工最短，能较快看到真实贴图。
```

问题：

```text
运行时仍要面对源格式 decode，和长期发布/cook 方向不完全一致。
大型项目启动时可能把 decode 成本推到 runtime。
不如构建期 cook 结果稳定，后续平台压缩格式也难自然接入。
```

### 4.3 方案 C-min：构建期 cooked texture payload + 运行时 upload

采用。

长期方向：

```text
Authoring source image
  -> Build/Cook
  -> CookedTexturePayload
  -> RuntimePackage
  -> RuntimeTextureRenderAsset
  -> RenderResourceManager
  -> WGPU texture
  -> Sprite2D present
```

本轮 C-min 只做够复杂打飞机项目：

```text
PNG -> RGBA8
1 mip
2D texture
LinearClamp sampler
AlphaBlend
DefaultSpriteMaterial
Sprite2D textured quad
结构化 report
headless deterministic tests
real-wgpu screenshot smoke optional/local-only
```

延后能力：

```text
JPG / WebP
平台压缩格式 BC/ASTC/ETC
mipmap 生成
texture streaming
atlas packing
sprite slicing / pivot / border
material graph / shader graph
GPU instancing / batching
多相机复杂透明排序
完整 UE-like render thread
```

## 5. 正式链路

### 5.1 Build / Cook

输入：

```text
Assets/tex-player-ship.png
Assets/tex-enemy-scout.png
Assets/tex-starfield.png
Assets/tex-bullet.png
```

或：

```json
{
  "schemaVersion": "texture-asset.v1",
  "assetId": "tex-player-ship",
  "sourceImage": "Assets/Images/player_ship.png",
  "importer": {
    "format": "png",
    "colorSpace": "srgb",
    "sampler": "linearClamp"
  }
}
```

C-min 允许直接 `.png` 作为 source，也允许 `.asset` descriptor 指向 `.png`。但运行时不读取 source path。

Asset ID 规则：

```text
直接 `.png` source 的默认 assetId 来自文件名或已有 meta/GUID 映射。
`.asset` descriptor 可以显式声明 assetId/sourceImage/importer policy，并覆盖默认命名。
两种输入最终都必须生成稳定 RuntimeAssetIndex entry；运行时只看 assetId/guid/type/cookedAssetId，不看 sourceImage。
```

Build/Cook 输出：

```text
cooked/textures/tex-player-ship.texture.json
cooked/textures/tex-player-ship.rgba8
```

metadata 最小字段：

```text
schemaVersion = cooked-texture.v1
assetId
cookedAssetId
sourceHash
width
height
format = rgba8Unorm
colorSpace = srgb
mipCount = 1
byteLength
pixelDataPath
sampler = linearClamp
```

RuntimeAssetIndex 规则：

```text
assetType = texture
loaderKind = texture
cookedAssetId = cooked-tex-player-ship
cooked_asset_table.path = cooked/textures/tex-player-ship.texture.json
```

注意：

```text
RuntimeAssetIndex 指向 cooked texture metadata，而不是项目源 PNG。
pixelDataPath 必须是 RuntimePackage 内相对路径。
Build/Cook 失败必须在 package build report 中出现，不能让 runtime 静默 fallback。
```

施工约束：

```text
如需新增 PNG 解码依赖，优先加在 Build/Cook 侧 crate 或 cook feature 中。
engine_runtime 默认路径不应因为本系统变成“运行时 PNG 解码器”。
```

### 5.2 Runtime Load

运行时只读取 RuntimePackage：

```text
RuntimeAssetIndex.resolve(TextureRef)
  -> cooked texture metadata
  -> rgba8 payload bytes
  -> RuntimeTexturePayload
```

RuntimeAssetLoader 边界：

```text
RuntimeAssetLoader 继续负责 RuntimeAssetIndex resolve、bundle/mount、cooked bytes 读取、handle 和 diagnostics。
RuntimeTexturePayload 可以作为 texture loader / payload reader 的类型化结果挂在 RuntimeAssetLoader 之后。
不要把现有 decoded_cache 直接变成不透明 GPU resource cache；GPU resident 状态属于 RenderResourceManager / WgpuTextureStore。
```

RuntimeTexturePayload 最小字段：

```text
asset_id
cooked_asset_id
width
height
format
color_space
mip_count
rgba8_bytes
sampler
source_hash
```

运行时不做：

```text
读取项目 Assets 目录
读取编辑器 Asset DB
按 sourceMapDebug 猜路径
从 sprite_ref 拼文件路径
```

### 5.3 Render Asset Prepare

Sprite2D 产生 request：

```text
RenderAssetPrepareRequest::sprite_texture(
  frame_index,
  sprite_ref,
  source_entity_id,
  source_proxy_id
)
```

Prepare 过程：

```text
ResolveAssetRef
  -> LoadRuntimeAsset
  -> LoadCookedTexturePayload
  -> CreateTextureRenderAsset
  -> CreateOrReuseRenderResource
  -> BindMaterial
```

这里对应 UE 的：

```text
UTexture2D platform data
  -> FTexture2DResource
  -> RHI resource
```

但本项目保持 AI-first report，不隐藏失败层。

### 5.4 WGPU Upload

RealWgpuBackend / WgpuTextureStore 执行：

```text
device.create_texture(Rgba8Unorm/Srgb policy)
queue.write_texture(rgba8_bytes)
texture.create_view()
device.create_sampler(LinearClamp)
device.create_bind_group(texture_view + sampler)
```

WGPU backend 只接收：

```text
RenderResourceHandle
Texture descriptor
RGBA payload
Sampler descriptor
```

WGPU backend 不接收：

```text
AssetRef
RuntimeAssetIndex
Project path
Source image path
SpriteRenderer2D component
```

Upload / draw 边界：

```text
Texture upload 必须在 render_to_view / draw 之前通过明确的 prepare/register 路径完成。
WgpuTextureStore 以 RenderResourceHandle + generation 作为 key 保存 wgpu::Texture/View/Sampler/BindGroup。
DrawSpriteTextured 只携带可绑定的 render resource / binding 信息，不在 draw 时猜 RuntimePackage、AssetRef 或文件路径。
```

### 5.5 Sprite2D Present

Sprite2D pass：

```text
SpriteRenderer2D
  -> RenderProjectionAdapter<SpriteRenderer2D>
  -> RenderProxyPayload::Sprite
  -> Sprite2DRenderPipeline
  -> Sprite2DDrawPlan(binding=fetched texture binding)
  -> DrawSpriteTextured
  -> RhiCommandPlan
  -> RealWgpuBackend textured pipeline
  -> Present
```

Textured vertex 最小字段：

```text
position: vec2
uv: vec2
color: rgba tint
```

C-min 可以先使用当前固定 quad 尺寸，重点证明真实 texture sampling；Sprite 尺寸、pivot、slicing 后续补。

## 6. 数据结构建议

### 6.1 CookedTextureMetadata

```text
CookedTextureMetadata:
  schema_version: cooked-texture.v1
  asset_id: String
  cooked_asset_id: String
  source_hash: String
  width: u32
  height: u32
  format: Rgba8Unorm
  color_space: Srgb
  mip_count: u32
  byte_length: u64
  pixel_data_path: String
  sampler: LinearClamp | NearestClamp
```

### 6.2 RuntimeTexturePayload

```text
RuntimeTexturePayload:
  asset_id: String
  cooked_asset_id: String
  width: u32
  height: u32
  format: Rgba8Unorm
  color_space: Srgb
  mip_count: u32
  rgba8: Vec<u8>
  sampler: SpriteSampler
```

### 6.3 RuntimeTexturePresentStatus

```text
RuntimeTexturePresentStatus:
  real_texture_present
  cooked_texture_ready
  fallback_placeholder
  missing
  decode_failed
  load_failed
  upload_failed
  bind_failed
```

### 6.4 Report

本轮新增或扩展 report 时必须分 runtime/editor 档位：

```text
Off:
  不生成 trace，不写 JSON，不保留长字符串。

Summary:
  frame_index
  requested_texture_count
  cooked_texture_ready_count
  uploaded_texture_count
  sprite_texture_bound_count
  fallback_count
  failed_count
  statuses

Trace:
  每个 asset_ref 的 stage/event/source_entity_id/source_proxy_id/cooked_asset_id/message。
  只用于测试、gate、debug 或用户显式诊断。
```

必须能表达：

```text
real_texture_present
fallback_placeholder
missing_runtime_asset
missing_cooked_texture_metadata
missing_pixel_payload
decode_failed
upload_failed
bind_failed
```

## 7. 样例项目规则

复杂打飞机样例当前 `.asset` 文本占位不能满足 P0-1。施工时必须收敛为下面任一形式：

方案一：直接图片资产

```text
Assets/tex-player-ship.png
Assets/tex-enemy-scout.png
Assets/tex-starfield.png
Assets/tex-bullet.png
```

方案二：descriptor + 图片

```text
Assets/tex-player-ship.asset
Assets/Images/player_ship.png
```

其中 `.asset` 是 texture descriptor，不再是无结构 placeholder 文本。

验收时必须证明：

```text
RuntimePackage cooked texture count >= 3
player/enemy/background sprite texture status = real_texture_present
fallback_count = 0，除非测试显式构造 missing fixture
```

## 8. 与 137 / 138 / 139 的关系

本文不推翻旧方案，而是把 P0-1 收敛到可施工的最小真实链路。

继承：

```text
137：RuntimeAsset -> RenderAssetPrepare -> RenderResourceManager -> SpriteMaterialBinding -> WGPU。
138：RuntimeRenderAssetProduction 是统一资源生产入口，不新增 TextureBridge / AuiBridge / FontBridge。
139：Sprite2DRenderPipeline 是 Sprite2D draw plan owner。
110：RenderAssetBridge 旧名归属 AssetProjection，不新增独立 bridge family。
227：本系统是复杂打飞机 P0-1。
```

修正：

```text
137/138 中偏 descriptor / resource handle 的部分，本轮必须落到真实 RGBA payload 与真实 WGPU texture upload。
06/07 中 Texture Decoder v1 的 metadata-only 描述仍是历史基线；本轮开始进入 C-min cooked texture payload。
```

## 9. AI / 用户编辑边界

用户和 AI 可以改：

```text
SpriteRenderer2D.sprite_ref
SpriteRenderer2D.material_ref
SpriteRenderer2D.visible
SpriteRenderer2D.sorting_layer
SpriteRenderer2D.order_in_layer
SpriteRenderer2D.sort_z
Texture asset source image / descriptor
BuildProfile texture cook policy
```

用户和 AI 不直接改：

```text
wgpu::Texture
wgpu::TextureView
wgpu::Sampler
wgpu::BindGroup
RenderResourceHandle
RHI pipeline object
Backend texture store
```

AI 查错路径固定为：

```text
1. SpriteRenderer2D.sprite_ref 是否存在。
2. RuntimeAssetIndex 是否能 resolve。
3. cooked texture metadata 是否存在。
4. rgba8 payload 是否存在且 byteLength = width * height * 4。
5. RuntimeTexturePayload 是否 ready。
6. RenderResourceHandle 是否 resident。
7. DrawSpriteTextured 是否携带 texture binding。
8. WGPU backend 是否 upload/bind/present 成功。
9. screenshot/golden 是否能看到非 fallback 真实像素。
```

## 10. 第一版 Gate 建议

### Gate A：Cooked texture schema / build cook

目标：

```text
ProjectRuntimePackageAssembler / RuntimePackageBuilder 能把 PNG texture asset cook 成 metadata + rgba8 payload。
RuntimeAssetIndex 指向 cooked texture metadata。
```

测试：

```text
cargo test -p editor_core project_runtime_package_assembler
cargo test -p engine_runtime runtime_package_builder
```

### Gate B：Runtime texture payload load / diagnostics

目标：

```text
RuntimePackage load 后能读取 CookedTextureMetadata + rgba8 bytes。
缺 metadata / 缺 payload / byteLength 不匹配有明确 report。
```

测试：

```text
cargo test -p engine_runtime runtime_package
cargo test -p engine_runtime runtime_asset
```

### Gate C：RenderAssetPrepare / Sprite2D binding

目标：

```text
Sprite2DRenderPipeline 不再默认 fallback。
可通过 RuntimeAssetIndex + cooked texture payload 生成真实 RenderBindingSet。
```

测试：

```text
cargo test -p engine_runtime sprite2d
cargo test -p engine_runtime render_asset
cargo test -p engine_runtime runtime_renderer
```

### Gate D：RealWgpuBackend textured quad

目标：

```text
RealWgpuBackend 支持 texture sampler bind group 和 textured shader。
DrawSpriteTextured 能采样上传后的真实 texture。
```

测试：

```text
cargo test -p engine_runtime --features real-wgpu wgpu_backend
```

真实 GPU / screenshot 测试可保持 ignored 或 local-only，不进入默认 CI 强制。

### Gate E：复杂打飞机 P0-1 验收

目标：

```text
samples/complex_shooter_project 中 player/enemy/background 至少三个 texture asset 真实 present。
project_e2e_gate 输出 P0-1 report。
```

测试：

```text
cargo test -p project_e2e_gate real_texture_present
cargo check -p runtime_player_winit --features real-window
cargo test -p runtime_player_winit real_windowed_player_screenshot_smoke --features real-window -- --ignored --nocapture
```

说明：真实 OS window / GPU screenshot 是 local-only smoke，不进入默认 CI 强制门禁；默认阻塞门禁以 headless deterministic report 为准。

## 11. 完成标准

本系统完成后必须证明：

```text
1. RuntimePackage 中存在 cooked texture metadata + rgba8 payload。
2. RuntimeAssetIndex 能 resolve tex-player-ship / tex-enemy-scout / tex-starfield。
3. Runtime 不读取项目源目录或编辑器内存。
4. Sprite2D draw plan 使用真实 texture binding，不再固定 sprite_binding_fallback。
5. RealWgpuBackend 上传并采样真实 texture。
6. 复杂打飞机导出/窗口链路能报告 real_texture_present。
7. 缺失、decode、upload、bind 失败都有结构化 report。
8. 没有把 Player / Enemy / Bullet 等玩法名写进引擎 Core API。
```

## 12. 自审

```text
是否符合 227 P0-1：
  是。直接解决真实贴图可见。

是否学习 UE：
  是。采用 Asset/CookedData -> RenderResource -> RHI resource 的分层，Sprite 也通过 render proxy/draw plan 进入渲染流程。

是否避免照搬 UE 过重复杂度：
  是。C-min 不引入完整 RenderThread/RDG/MaterialGraph/TextureStreaming。

是否保持 RuntimePackage 真相：
  是。运行时只读取 RuntimePackage cooked texture，不扫项目源目录。

是否 AI-first：
  是。每层都有 stage/code/status，支持 Summary/Trace 分档。

是否适配复杂项目长期演进：
  是。后续 mipmap、压缩格式、streaming、atlas、material graph 都可以挂在 cooked payload / render resource 层，不推翻本链路。

是否克制施工范围：
  是。本轮只做 PNG->RGBA8、单 mip、Sprite2D textured present。

是否明确 decode 阶段：
  是。decode 放在 Build/Cook，runtime 只消费 cooked RGBA payload。

是否明确 crate / 依赖边界：
  是。PNG 解码依赖优先进入 Build/Cook 侧，不让 engine_runtime 默认路径退化成运行时源图片解码器。

是否明确 GPU resident 边界：
  是。RuntimeAssetLoader 不持有 wgpu 对象；WgpuTextureStore / RenderResourceManager 管理 resident GPU texture。

是否避免真实窗口测试阻塞默认 CI：
  是。真实 GPU screenshot smoke 为 ignored/local-only，默认门禁使用 headless deterministic report。
```

## 13. 结论

正式采用：

```text
方案 C-min：UE-like Cooked Texture Payload + Runtime RenderResource Upload + Sprite2D Textured Present
```

本轮目标不是完整商业纹理系统，而是把复杂打飞机项目最关键的视觉证据打穿：

```text
编辑器/项目资产里有真实 PNG
  -> RuntimePackage 里有 cooked texture
  -> Runtime load/upload 成 GPU texture
  -> Sprite2D 在 Windows 窗口真实显示
```

下一步应根据本文生成施工文档，并在施工文档中按 Gate A-E 执行。
