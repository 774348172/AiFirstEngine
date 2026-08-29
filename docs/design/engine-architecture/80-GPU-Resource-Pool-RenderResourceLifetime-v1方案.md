# GPU Resource Pool / Render Resource Lifetime v1 方案

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

## 定位

本文档定义 GPU Resource Pool / Render Resource Lifetime v1 的长期架构规则。

它解决的问题是：

```text
CPU Asset / Render Asset / GPU Resource 如何分层。
GPU 资源由谁创建、谁持有、谁释放。
资源如何复用、热更替换、延迟销毁。
GPU device / surface lost 时如何恢复。
AI / 用户如何追踪资源为什么创建、为什么没释放、为什么失败。
```

当前已有能力：

```text
76 已完成 Wgpu Surface / GPU Texture Lifetime / RenderThread C-min。
78 已完成 mesh / material / texture GPU upload C-min。
79 已完成真实可玩最小循环 C-min。
```

但这些不是完整 GPU Resource Pool。完整资源池需要本方案作为正式规则，再另行生成施工文档。

## 其它引擎参考

### Unreal Engine

UE 的核心模式是：

```text
Game Thread
  -> BeginInitResource / BeginReleaseResource
  -> Render Thread
  -> FRenderResource::InitRHI / ReleaseRHI
  -> RHI Resource
```

源码参考：

```text
Engine/Source/Runtime/RenderCore/Public/RenderResource.h
Engine/Source/Runtime/RenderCore/Private/RenderResource.cpp
Engine/Source/Runtime/RenderCore/Public/RenderDeferredCleanup.h
Engine/Source/Runtime/RenderCore/Public/RenderCommandFence.h
```

关键特征：

```text
FRenderResource 是 rendering thread owned。
Game Thread 不直接创建 / 释放底层 RHI Resource。
InitRHI / ReleaseRHI 在渲染线程执行。
未 release 就销毁 FRenderResource 会触发严重错误。
支持批量 release、deferred cleanup、fence 等安全释放机制。
```

可借鉴：

```text
资源生命周期必须显式。
渲染线程拥有 GPU 资源。
释放不能立即物理销毁，必须经过 GPU 安全点。
资源必须有 owner / name / diagnostic，方便查错。
```

不照搬：

```text
第一版不做完整 UE 级 streaming texture、virtual texture、Nanite、Lumen、多 GPU、复杂 RHI barrier。
第一版不要求项目开发者理解 FRenderResource 等价概念。
```

### Unity

Unity 的用户层看到的是：

```text
Texture2D
RenderTexture
Mesh
Material
ComputeBuffer / GraphicsBuffer
AssetBundle / Addressables
Resources.UnloadUnusedAssets
```

关键特征：

```text
用户操作对象，真实 GPU 资源生命周期主要隐藏在 native engine。
资源释放通常通过 Destroy / UnloadUnusedAssets / AssetBundle unload / Addressables release 间接触发。
使用简单，但 GPU 资源为什么还在、什么时候真正释放，对用户和 AI 都不够透明。
```

可借鉴：

```text
项目逻辑不应该直接操作底层 GPU 对象。
用户应通过资源系统表达 load / unload / release。
```

不照搬：

```text
不能把 GPU 生命周期完全黑盒化，否则 AI 难以诊断资源泄漏、热更替换、加载失败。
```

### Bevy

Bevy 的核心模式是：

```text
Main World SourceAsset
  -> ExtractSchedule
  -> Render World ExtractedAssets
  -> PrepareAssets
  -> RenderAssets<T>
  -> RenderDevice / RenderQueue
```

源码参考：

```text
crates/bevy_render/src/render_asset.rs
crates/bevy_render/src/erased_render_asset.rs
crates/bevy_render/src/renderer/render_device.rs
crates/bevy_render/src/renderer/mod.rs
```

关键特征：

```text
SourceAsset 和 GPU representation 分离。
RenderAssets<T> 保存渲染世界可用的 GPU 表示。
AssetEvent::Added / Modified / Unused 驱动提取、重建和移除。
RenderAssetBytesPerFrameLimiter 控制每帧上传量，避免卡顿。
```

可借鉴：

```text
Rust / ECS / Render Extract 架构非常适合我们。
CPU asset 到 GPU asset 的转换应该是显式 prepare 阶段。
上传预算应该是资源池的正式能力。
```

不照搬：

```text
Bevy 的诊断和 AI 可读生命周期报告不足，需要我们补充。
```

### Godot

Godot 的核心模式更接近：

```text
Resource / Scene
  -> RenderingServer
  -> RID
  -> RenderingDevice / backend resource
```

关键特征：

```text
RenderingServer 持有底层渲染资源。
用户或引擎通过 RID 引用 server 侧资源。
RID 用完需要释放。
```

可借鉴：

```text
GPU 资源应通过 handle / id 间接访问，不暴露真实 backend object。
```

不照搬：

```text
不把手动 RID 生命周期暴露给普通项目逻辑，否则不够 AI 友好。
```

## 方案选择

### 方案 A：Unity 式隐藏资源管理

```text
项目逻辑只看到 Asset / Texture / Mesh。
底层 GPU 创建释放全部隐藏。
```

优点：

```text
用户理解成本最低。
第一版实现可以很快。
```

缺点：

```text
AI 难以解释资源泄漏、热更替换、上传失败。
复杂项目里资源常驻和释放问题会变成黑盒。
```

结论：

```text
不采用。
```

### 方案 B：Bevy 式 RenderAsset / RenderAssets

```text
SourceAsset 通过 Extract / Prepare 转换成 RenderAsset。
RenderAssets<T> 存 GPU 表示。
```

优点：

```text
Rust / ECS 友好。
结构清晰。
适合 headless 测试。
```

缺点：

```text
缺少足够强的 lifetime report。
热更代际、安全延迟释放、跨 backend 资源状态需要额外补齐。
```

结论：

```text
作为基础思想采用，但不能原样照搬。
```

### 方案 C：UE 式显式 RenderResource 生命周期

```text
每类资源都有显式 init / release / update。
RenderThread 拥有 GPU-side resource。
```

优点：

```text
长期大型项目最稳。
线程边界明确。
查资源泄漏能力强。
```

缺点：

```text
完整照搬复杂度过高。
会把底层概念暴露给过多系统。
```

结论：

```text
采用生命周期纪律，不照搬完整复杂度。
```

### 方案 D：推荐方案，UE 生命周期纪律 + Bevy 资产转换模型 + AI Report

```text
RuntimeAssetLoader
  -> RenderAssetBridge
  -> RenderResourceRequest
  -> RenderThread
  -> RenderResourceManager
  -> EngineRHI
  -> Backend
```

优点：

```text
AI 可读。
长期边界正确。
Rust / ECS / Render Extract 友好。
能支撑复杂项目。
不会把 GPU 细节暴露给项目逻辑。
```

缺点：

```text
比 Unity 黑盒方案多一个 RenderResourceManager。
第一版要认真设计报告和 handle。
```

结论：

```text
采用方案 D，第一版做 C-min。
```

## 正式架构规则

### 分层规则

```text
AssetRef / AssetId
  表示项目资产引用。

RuntimeAsset
  表示已加载、已验证、接近 runtime 的 CPU-side asset。

RenderAssetKey
  表示某个资产在某个平台、质量档、版本、用途下的渲染形态。

RenderResourceHandle
  表示 GPU 资源池中的资源句柄。

BackendResource
  表示 wgpu / D3D12 / Vulkan / Metal 的真实底层对象。
```

规则：

```text
项目逻辑只能持有 AssetRef / RuntimeAssetHandle。
Renderer / RDG 只能引用 RenderResourceHandle。
BackendResource 只能由 RenderResourceManager / Backend 持有。
项目逻辑不能直接创建、释放、修改 GPU Resource。
```

### 所有权规则

```text
RuntimeAssetLoader 负责加载 CPU-side runtime asset。
RenderAssetBridge 负责把 RuntimeAsset 转成 RenderResourceRequest。
RenderThread 独占 RenderResourceManager。
RenderResourceManager 负责 GPU 资源创建、复用、上传、替换、释放。
EngineRHI 只执行已编译的 RHI command / pass。
Backend 只创建真实平台资源。
```

### RDG 边界规则

```text
RDG 不拥有长期 GPU 资源。
RDG 只在一帧内引用 RenderResourceHandle。
RDG 可以创建 frame-local temporary resource。
长期资源生命周期由 RenderResourceManager 管理。
```

这条规则避免 RDG 变成资源数据库，也避免资源池和帧图职责混在一起。

### 第一版资源类型

GPU Resource Pool v1 只支持：

```text
Texture
MeshBuffer
MaterialParams / BindGroup placeholder
ShaderPipeline placeholder
SurfaceFrameTexture
```

暂不做：

```text
Virtual Texture
Streaming Mip
GPU-driven meshlet / cluster resource
Ray tracing acceleration structure
Persistent descriptor heap allocator
Full pipeline cache
Multi-GPU resource residency
```

### Handle 规则

```text
RenderResourceHandle:
  kind
  index
  generation
```

规则：

```text
handle 必须带 generation，防止旧 handle 错误引用新资源。
handle 不暴露 backend object。
handle 可以进入 RenderCommand / RenderProxy / RDG。
handle 不能进入项目逻辑规则层。
```

### Key 规则

```text
RenderAssetKey:
  asset_id
  asset_version
  resource_kind
  platform_profile
  quality_profile
  usage
```

规则：

```text
同一 Asset 在不同平台 / 质量 / 用途下可以生成不同 RenderAssetKey。
热更后 asset_version 改变，必须生成新 generation。
旧 generation 不能立即销毁，必须等待 GPU 安全点。
```

### 状态机

GPU Resource v1 只保留以下状态：

```text
Missing
Requested
Uploading
Resident
Stale
PendingRelease
Released
Failed
```

状态含义：

```text
Missing：资源不存在或尚未请求。
Requested：已收到请求，等待创建 / 上传。
Uploading：CPU 数据正在提交到 GPU。
Resident：GPU 可用。
Stale：已有新版本请求，当前版本等待替换。
PendingRelease：不再被新帧引用，等待 GPU 安全释放。
Released：已释放。
Failed：创建或上传失败。
```

禁止增加更多状态来处理小边界问题。若出现复杂情况，优先通过 event reason / diagnostic 表达，不扩张状态机。

### 创建 / 复用流程

```text
RenderResourceRequest
  -> 查 RenderAssetKey
  -> 已有 Resident 且 generation 匹配：复用
  -> 已有但 version 不匹配：创建新 generation，旧资源 Stale
  -> 不存在：创建 Requested
  -> 上传成功：Resident
  -> 上传失败：Failed
```

### 释放流程

```text
项目侧 release / unload AssetRef
  -> RuntimeAssetLoader 更新 CPU asset 引用状态
  -> RenderAssetBridge 发出 RenderResourceReleaseRequest
  -> RenderResourceManager 标记 PendingRelease
  -> GPU safe frame / fence 后真实释放 backend resource
  -> 状态变为 Released
```

规则：

```text
项目侧 unload 不是物理删除 GPU resource。
GPU resource 不允许立即释放。
真实释放只允许发生在 RenderThread / RenderResourceManager 内。
```

### 热更替换流程

```text
新 asset_version mount
  -> 创建新 RenderAssetKey / generation
  -> 新 generation 上传成功
  -> 后续 frame 使用新 handle
  -> 旧 generation 进入 PendingRelease
  -> GPU 安全点后释放旧资源
```

规则：

```text
热更不能原地覆盖正在被 GPU 使用的资源。
热更资源替换必须有明确 apply point。
失败时继续使用旧 generation，并输出 diagnostic。
```

### Device Lost / Surface Lost

规则：

```text
SurfaceFrameTexture 丢失只重建 surface 相关资源。
Device lost 时 RenderResourceManager 必须把 backend resource 标为需要重建。
RuntimeAsset / RenderAssetKey 仍作为重建依据。
重建失败进入 Failed，并输出 RenderResourceLifetimeReport。
```

第一版只要求：

```text
能标记 lost。
能输出 report。
能在 headless backend 模拟 lost / recreate。
真实复杂 device recovery 后续再扩展。
```

### 上传预算

借鉴 Bevy 的 bytes-per-frame 思路：

```text
RenderUploadBudget:
  max_bytes_per_frame
  uploaded_bytes_this_frame
  deferred_request_count
```

规则：

```text
第一版只统计和软限制上传量。
超过预算的资源可以延后到下一帧。
超大单个资源允许至少推进一个资源，避免永久卡住。
不做复杂 streaming mip。
```

### 诊断报告

```text
RenderResourceLifetimeReport:
  frame_index
  created_count
  reused_count
  uploaded_bytes
  resident_bytes
  pending_release_count
  failed_count
  events
```

```text
RenderResourceEvent:
  event_type
  resource_kind
  asset_id
  generation
  state_before
  state_after
  bytes
  reason
  diagnostic
```

规则：

```text
默认 runtime release build 不输出全量事件。
Editor / Test / Diagnostic 模式可以输出完整 report。
AI 默认读取 summary，再按需读取 typed detail。
报告只解释生命周期，不把 GPU backend object 暴露给项目逻辑。
```

### AI 友好规则

AI 需要能回答：

```text
这个 Asset 为什么还没有 GPU 资源？
这个资源为什么没有释放？
这一帧上传了多少 GPU 数据？
热更后当前使用的是哪个 generation？
资源创建失败是 import、runtime asset、render asset、RHI 还是 backend 问题？
```

因此所有失败必须归类：

```text
MissingRuntimeAsset
InvalidRenderAssetData
UnsupportedFormat
UploadBudgetDeferred
BackendCreateFailed
DeviceLost
ReleasedByUnload
ReplacedByHotUpdate
```

第一版不需要更多错误分类。

## 禁止事项

```text
不允许项目逻辑直接创建 GPU resource。
不允许项目逻辑直接持有 backend texture / buffer / pipeline。
不允许 RDG 拥有长期资源生命周期。
不允许热更原地覆盖正在使用的 GPU 资源。
不允许为了个别资源类型扩张全局状态机。
不允许第一版实现完整 UE 级 streaming / virtual texture / descriptor allocator / pipeline cache。
```

## 第一版门禁测试

后续生成施工文档时，至少需要以下 headless 测试：

```text
render_resource_pool_creates_and_reuses_texture
render_resource_pool_rejects_stale_generation_handle
render_resource_pool_delays_release_until_safe_frame
render_resource_pool_replaces_hot_update_generation
render_resource_lifetime_report_records_create_reuse_upload_release_fail
render_resource_upload_budget_defers_large_batch
render_resource_device_lost_marks_resources_for_rebuild
```

真实 GPU / real wgpu smoke gate 只能作为 feature-gated ignored 测试，不作为默认自动化测试。

## 和现有文档关系

```text
59-真实WgpuBackend-RDG-RHI最小门禁方案.md
  定义 RDG / RHI / WgpuBackend C-min。

76-真实WgpuSurface-GPUTextureLifetime-RenderThread-C-min.md
  已完成 surface texture lifetime 的局部 C-min。

78-最小游戏闭环剩余主线整合施工.md
  已完成 mesh / material / texture GPU upload C-min。

80-GPU-Resource-Pool-RenderResourceLifetime-v1方案.md
  定义完整 GPU Resource Pool / Render Resource Lifetime 的正式规则。
```

本方案确认后，GPU Resource Pool 不再从零讨论。后续只讨论：

```text
施工范围。
数据结构细节。
测试门禁。
与 RenderThread / RHI backend 的落地顺序。
```

## 下一步建议

下一个更值得讨论的系统是：

```text
真实跨线程 Render Thread Queue / Render Submission Pipeline v1
```

原因：

```text
GPU Resource Pool 的正确释放依赖 RenderThread 的安全提交点和 fence / safe frame。
如果 RenderThread 队列仍是假同步，资源池只能做模拟延迟释放，不能形成长期正确边界。
```
