# 当前可自动化施工文档：GPU Resource Pool / Render Resource Lifetime v1

> 状态：已确认可施工。长期规则见 `../../80-GPU-Resource-Pool-RenderResourceLifetime-v1方案.md`。

## 1. 施工目标

按 80 号正式规则实现 GPU Resource Pool / Render Resource Lifetime v1 的 C-min。

目标链路：

```text
RuntimeAsset / RuntimeAssetHandle
  -> RenderAssetBridge
  -> RenderResourceRequest
  -> RenderThread
  -> RenderResourceManager
  -> RenderResourceHandle
  -> RDG / RHI command plan
  -> Backend resource placeholder / headless backend
  -> RenderResourceLifetimeReport
```

第一版目标不是做完整高性能 GPU 内存管理，而是建立长期正确的资源生命周期边界：

```text
CPU Asset / Render Asset / GPU Resource 分层清楚。
RenderThread 独占 RenderResourceManager。
项目逻辑不能直接操作 GPU resource。
RDG 只引用 RenderResourceHandle，不拥有长期资源。
资源创建、复用、热更替换、延迟释放、失败都可报告。
默认测试必须 headless。
```

## 2. 成熟引擎对照

| 引擎 | 对应机制 | 本轮借鉴 | 本轮不做 |
|---|---|---|---|
| Unreal | FRenderResource / BeginInitResource / BeginReleaseResource / InitRHI / ReleaseRHI | 学显式生命周期、RenderThread owner、延迟释放、资源泄漏可诊断 | 不做完整 UE 级 streaming texture、virtual texture、Nanite、Lumen、多 GPU、复杂 barrier |
| Unity | Texture2D / RenderTexture / Mesh / Material / native graphics resource / UnloadUnusedAssets | 学用户不直接操作底层 GPU 对象，通过资源系统表达 load / unload | 不做黑盒生命周期；必须让 AI 能查资源为什么创建、没释放或失败 |
| Bevy | SourceAsset -> Extract -> PrepareAssets -> RenderAssets<T> -> RenderDevice / RenderQueue | 学 Rust/ECS 友好的 CPU asset 到 GPU asset 转换、每帧上传预算 | 不照搬 Bevy RenderWorld 复杂度，不缺失 AI report |
| Godot | Resource -> RenderingServer -> RID / RenderingDevice | 学 handle / id 间接访问 GPU 资源 | 不把 RID 式手动生命周期暴露给项目逻辑 |

项目路线：

```text
UE-like explicit lifetime discipline
+ Bevy-like render asset prepare model
+ Godot-like opaque handle
+ Unity-like user-facing simplicity
+ AI-first structured lifetime report
```

## 3. 本轮必须保持的长期边界

```text
AssetRef / AssetId 是项目资产引用。
RuntimeAsset / RuntimeAssetHandle 是 runtime CPU-side 资产。
RenderAssetKey 是资产在平台 / 质量 / 版本 / 用途下的渲染形态。
RenderResourceHandle 是 GPU 资源池句柄。
BackendResource 只由 RenderResourceManager / Backend 持有。
```

硬规则：

```text
项目逻辑不能直接创建、释放、修改 GPU Resource。
项目逻辑不能持有 backend texture / buffer / pipeline。
RenderThread 独占 RenderResourceManager。
RDG 不拥有长期 GPU Resource。
RDG 只能在一帧内引用 RenderResourceHandle。
RuntimeRenderer 不绕过 RDG / RHI 直接调 backend。
```

## 4. 本轮做什么

只做 C-min：

```text
RenderResourceHandle
RenderAssetKey
RenderResourceState
RenderResourceRecord
RenderResourceRequest
RenderResourceReleaseRequest
RenderResourceManager
RenderUploadBudget
RenderResourceLifetimeReport
RenderResourceEvent
RenderAssetBridge C-min
RenderThread 接入资源池 report
headless tests
```

第一版资源类型：

```text
Texture
MeshBuffer
MaterialParams
ShaderPipeline placeholder
SurfaceFrameTexture
```

第一版状态：

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

## 5. 本轮不做什么

```text
完整 RHI / RDG 重构。
真实 Native RHI / D3D12 / Vulkan / Metal backend。
完整跨线程 RenderThread 高性能队列。
真实 GPU allocator / descriptor heap allocator。
Virtual Texture / Streaming Mip。
Ray tracing acceleration structure。
Full pipeline cache。
Multi-GPU residency。
真实热更下载与 mount。
真实平台显存预算策略。
```

本轮允许在 `InlineDeterministic` / headless backend 下实现 safe frame 模拟，但 API 必须为后续真实 RenderThread queue / fence 留出边界。

## 6. 建议代码落点

优先复用现有 crate：

```text
rust/crates/engine_runtime/src/render_thread.rs
rust/crates/engine_runtime/src/gpu_texture_lifetime.rs
rust/crates/engine_runtime/src/runtime_asset.rs
rust/crates/engine_runtime/src/runtime_asset_loader.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/render_command.rs
rust/crates/engine_runtime/src/render_scene_state.rs
rust/crates/engine_runtime/src/lib.rs
```

允许新增：

```text
rust/crates/engine_runtime/src/render_resource.rs
rust/crates/engine_runtime/src/render_asset_bridge.rs
```

不允许新增第二套 runtime、第二套 renderer、第二套 asset loader。

## 7. 数据结构

### 7.1 RenderResourceHandle

```text
RenderResourceHandle:
  kind
  index
  generation
```

规则：

```text
generation 必须参与校验。
旧 generation 不能错误命中新资源。
handle 不暴露 backend object。
```

### 7.2 RenderAssetKey

```text
RenderAssetKey:
  asset_id
  asset_version
  resource_kind
  platform_profile
  quality_profile
  usage
```

第一版 `platform_profile / quality_profile / usage` 可以用字符串或 enum C-min，但字段必须存在。

### 7.3 RenderResourceRecord

```text
RenderResourceRecord:
  handle
  key
  state
  bytes
  last_used_frame
  pending_release_frame
  owner
  last_error
```

### 7.4 RenderResourceRequest

```text
RenderResourceRequest:
  key
  source
  byte_len
  reason
```

`source` 第一版可以是 typed placeholder：

```text
TextureDescriptor
MeshBufferDescriptor
MaterialParamsDescriptor
ShaderPipelineDescriptor
SurfaceFrameTextureDescriptor
```

### 7.5 RenderResourceLifetimeReport

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

## 8. 施工步骤

### A1：建立 render_resource 模块

新增 `render_resource.rs`，定义纯数据类型：

```text
RenderResourceKind
RenderResourceState
RenderResourceHandle
RenderAssetKey
RenderResourceRecord
RenderResourceRequest
RenderResourceReleaseRequest
RenderUploadBudget
RenderResourceLifetimeReport
RenderResourceEvent
RenderResourceDiagnostic
```

验收：

```text
cargo test -p engine_runtime render_resource_handle_generation_rejects_stale_handle
cargo test -p engine_runtime render_resource_lifetime_report_serializes_stably
```

最小测试要求：

```text
创建 handle。
同 index 不同 generation 必须判定为 stale。
report 可 serde roundtrip。
```

### A2：实现 RenderResourceManager C-min

新增：

```text
RenderResourceManager
```

能力：

```text
request_resource
release_resource
mark_used
end_frame
get
report
```

核心行为：

```text
相同 RenderAssetKey 且 Resident：复用。
相同 asset_id 但 asset_version 不同：创建新 generation，旧资源 Stale。
release 后进入 PendingRelease。
end_frame 超过 safe_frame_delay 后进入 Released。
失败进入 Failed 并记录 diagnostic。
```

验收：

```text
cargo test -p engine_runtime render_resource_pool_creates_and_reuses_texture
cargo test -p engine_runtime render_resource_pool_delays_release_until_safe_frame
cargo test -p engine_runtime render_resource_pool_replaces_hot_update_generation
```

### A3：接入上传预算

实现：

```text
RenderUploadBudget:
  max_bytes_per_frame
  uploaded_bytes_this_frame
  deferred_request_count
```

行为：

```text
预算足够：Requested -> Uploading -> Resident。
预算不足：保留 Requested / deferred，并输出 UploadBudgetDeferred。
超大单个资源允许至少推进一个，避免永远卡住。
```

验收：

```text
cargo test -p engine_runtime render_resource_upload_budget_defers_large_batch
cargo test -p engine_runtime render_resource_upload_budget_allows_one_large_resource_progress
```

### A4：实现 RenderAssetBridge C-min

新增 `render_asset_bridge.rs`。

输入：

```text
RuntimeAssetHandle
RuntimeAssetRecord / RuntimeAsset data
platform_profile
quality_profile
usage
```

输出：

```text
RenderResourceRequest
```

第一版只需要覆盖：

```text
texture -> Texture request
mesh -> MeshBuffer request
material -> MaterialParams request
unknown type -> Failed / UnsupportedFormat diagnostic
```

验收：

```text
cargo test -p engine_runtime render_asset_bridge_builds_texture_request_from_runtime_asset
cargo test -p engine_runtime render_asset_bridge_reports_unsupported_format
```

### A5：RenderThread 接入 RenderResourceManager

修改 `render_thread.rs`：

```text
RenderThread 持有 RenderResourceManager。
RenderThreadFrameInput 可携带 resource requests / release requests。
RenderThreadReport 携带 RenderResourceLifetimeReport。
RuntimeRendererOutput 可继续携带旧 texture_lifetime_report，但新增 resource_lifetime_report。
```

兼容规则：

```text
GpuTextureLifetimeReport 保留，作为 surface / frame texture 局部报告。
RenderResourceLifetimeReport 是完整 GPU resource pool 报告。
两者不能混成一个概念。
```

验收：

```text
cargo test -p engine_runtime render_thread_reports_render_resource_lifetime
cargo test -p engine_runtime render_thread_does_not_expose_backend_resource_to_runtime_logic
```

### A6：Device Lost / Surface Lost C-min

行为：

```text
surface lost：只影响 SurfaceFrameTexture。
device lost：RenderResourceManager 标记 Resident 资源需要重建或 Failed。
headless backend 能模拟 lost。
```

第一版不要求真实 device recovery。

验收：

```text
cargo test -p engine_runtime render_resource_device_lost_marks_resources_for_rebuild
cargo test -p engine_runtime render_resource_surface_lost_only_marks_surface_frame_texture
```

### A7：端到端 headless gate

构造一个最小场景：

```text
RuntimeAsset: texture_a v1
RuntimeAsset: mesh_a v1
RuntimeAsset: material_a v1
Entity 引用 mesh/material/texture
RenderAssetBridge 生成 requests
RenderThread 创建资源
RuntimeRenderer / RDG 只引用 RenderResourceHandle
release 后 delayed release
热更 texture_a v2 后 generation 替换
```

验收：

```text
cargo test -p engine_runtime render_resource_pool_end_to_end_headless_scene
```

必须断言：

```text
created_count > 0
reused_count > 0
uploaded_bytes > 0
pending_release_count 在 release 后出现
旧 generation 被判定 stale
新 generation 被使用
report.ok 或 equivalent diagnostic 没有 error
```

## 9. 最小测试清单

本轮完成前必须至少有：

```text
render_resource_handle_generation_rejects_stale_handle
render_resource_lifetime_report_serializes_stably
render_resource_pool_creates_and_reuses_texture
render_resource_pool_delays_release_until_safe_frame
render_resource_pool_replaces_hot_update_generation
render_resource_upload_budget_defers_large_batch
render_resource_upload_budget_allows_one_large_resource_progress
render_asset_bridge_builds_texture_request_from_runtime_asset
render_asset_bridge_reports_unsupported_format
render_thread_reports_render_resource_lifetime
render_resource_device_lost_marks_resources_for_rebuild
render_resource_surface_lost_only_marks_surface_frame_texture
render_resource_pool_end_to_end_headless_scene
```

推荐整体验证：

```powershell
cargo test -p engine_runtime render_resource
cargo test -p engine_runtime render_asset_bridge
cargo test -p engine_runtime render_thread
```

实际命令以当前 crate test name 为准。

## 10. 成功标准

本轮施工完成后必须满足：

```text
1. 有 RenderResourceManager，并由 RenderThread 持有。
2. 有 RenderResourceHandle / RenderAssetKey / generation 校验。
3. 资源能创建、复用、上传预算延迟、热更替换、延迟释放。
4. RDG / Renderer 不持有 backend object，只持有 RenderResourceHandle。
5. 项目逻辑不能直接操作 GPU resource。
6. 有 RenderResourceLifetimeReport。
7. 所有默认测试 headless 可跑。
8. 不引入第二套 runtime / renderer / asset loader。
```

## 11. 完成后必须更新

施工完成后：

```text
1. 将本文档从 施工文档/当前 移到 施工文档/已完成。
2. 新增 阶段完成记录/2026-xx-xx-GPU-Resource-Pool-RenderResourceLifetime-v1/00-总览.md。
3. 更新 49-当前状态与下一步入口.md。
4. 更新 54-当前可自动化施工总入口.md。
5. 更新 18-长期主义实现路径.md。
6. 更新 施工文档/README.md。
7. 如果施工时调整了长期规则，回填 80 号方案文档。
```

## 12. 退出条件

如果实现时发现以下问题，停止施工并回到方案讨论：

```text
RenderThread 当前结构无法持有 RenderResourceManager。
RuntimeAsset 现有数据不足以生成 RenderAssetKey。
RenderCommand / RenderProxy 已经直接持有 backend resource。
需要完整真实跨线程 queue / fence 才能完成最小 delayed release。
需要重构 RHI / RDG 才能完成 handle 引用。
```

否则按本施工文档直接实现。
