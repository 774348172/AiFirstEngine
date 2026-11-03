# 138-当前可自动化施工文档-Runtime Render Asset Production & Binding v1

## 1. 对应正式方案

```text
../138-Runtime-Render-Asset-Production-and-Binding-v1方案.md
```

本施工文档执行已经确认的正式系统：

```text
Runtime Render Asset Production & Binding v1
```

系统目标是把 runtime 可见资产和运行时 descriptor，统一生产成 renderer 可消费的 typed render asset，再交给 RenderResourceManager / EngineRHI 创建 GPU resource 和 binding。

第一版必须接入：

```text
Texture
Material
Mesh
AUI image
Font atlas
Particle texture
Render target
```

## 2. 方案自审结论

按 `Proposal Self-Review Rule` 审查，`138` 正式方案通过，可以进入施工。

```text
1. Specification fit:
   方案解决的是“Runtime 资产如何变成渲染可用资源并完成 binding”，不是单点 texture upload，符合当前系统边界。

2. Rule fit:
   方案遵守 EngineRHI / RenderResourceManager / RuntimeRenderer 边界，不允许 WgpuBackend 直接读取 AssetRef、RuntimePackage 或项目资源路径。

3. Textual consistency:
   文档中的 RuntimeRenderAssetProducer、Typed RenderAsset、RenderBindingSet、RenderAssetProductionReport 职责一致，没有互相冲突。

4. Design fit:
   方案延续 AI-first、复杂项目可扩展、长期可维护和减少隐藏规则的方向；资源差异进入 kind / usage / typed payload，而不是散成多个平级 bridge。

5. Implementation feasibility:
   现有 137 已经打通 RuntimeAsset -> RenderResourceManager -> DrawSpriteTextured 的结构链，本阶段是在该链路上扩展统一 production / binding，具备落地基础。

6. Practical reasonableness:
   v1 边界清楚，只做最小生产、绑定、诊断和测试入口；完整 Material Graph、Shader Graph、Texture Streaming、复杂 RenderTarget Pooling 等明确排除。
```

结论：

```text
方案过审。
不需要重新出方案。
可以按本施工文档开始自动化施工。
```

## 3. 施工硬规则

```text
不得新增 TextureBridge / MeshBridge / FontBridge / AuiBridge 等平级桥。
每种资源允许有 typed producer，但必须注册到统一 RuntimeRenderAssetProducer 系统下。
Producer 只产出 backend-neutral typed render asset，不创建 wgpu::Texture / wgpu::Buffer / bind group。
WgpuBackend 只消费 EngineRHI / RHI command / backend resource descriptor，不理解 RuntimePackage、AssetRef 或项目路径。
RenderResourceManager 负责资源创建、复用、释放和 handle 分配。
RenderBindingSet 是 renderer draw 可消费的统一 binding 输出。
所有失败、fallback、ready 状态进入统一 RenderAssetProductionReport。
```

如果实现过程中出现下列情况，必须停止并回到方案层修正：

```text
1. 某类资源需要绕过 RuntimeRenderAssetProducer 才能渲染。
2. WgpuBackend 需要读取项目资源语义才能上传。
3. 每类资源开始拥有自己的 report / lifecycle / binding 规则。
4. AUI image、Font atlas、Particle texture 被做成独立上传系统，而不是 Texture 类资源的 usage。
```

## 4. 不施工范围

本阶段不做：

```text
完整 Asset Importer / Cook 规则
完整 Material Graph
完整 Shader Graph
完整 Particle System
完整 AUI authoring / layout
完整 Font shaping / IME
完整 Mesh optimizer / meshlet / Nanite-like 路线
完整 Texture streaming / virtual texture
复杂 RenderTarget pooling / aliasing / transient graph allocator
跨平台 D3D12 / Vulkan / Metal 后端
```

## 5. Gate A：核心类型与 Report

目标：

```text
新增 RuntimeRenderAssetKind。
新增 RuntimeRenderAssetUsage。
新增 RuntimeRenderAssetRequest。
新增 RuntimeRenderAssetId / RuntimeRenderAssetHandle。
新增 RuntimeRenderAssetStatus。
新增 RuntimeRenderAssetProductionReport。
新增 RuntimeRenderAssetProductionEvent。
新增 RuntimeRenderAssetProductionStage。
新增 RuntimeRenderAssetProductionCode。
```

第一版事件字段：

```text
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

第一版 stage：

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

第一版 code：

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

建议代码范围：

```text
rust/crates/engine_runtime/src/render_asset_production.rs
rust/crates/engine_runtime/src/render_resource.rs
rust/crates/engine_runtime/src/lib.rs
```

测试：

```powershell
cargo test -p engine_runtime render_asset_production_report
```

验收：

```text
能构造七类 kind / usage 的 request。
Ready / Failed / FallbackUsed 都能进入统一 report。
不需要真实 GPU 也能跑完 headless 测试。
```

## 6. Gate B：Texture 系共享链路

目标：

```text
实现 TextureProducer。
AUI image 作为 Texture usage 接入。
Particle texture 作为 Texture usage 接入。
支持 runtime asset bytes / cooked blob -> decoded RGBA / texture descriptor。
输出 TextureRenderAsset。
生成 texture RenderResourceRequest。
生成最小 texture RenderBindingSet。
```

规则：

```text
AUI image 不新建 AuiImageBridge。
Particle texture 不新建 ParticleTextureBridge。
Font atlas 暂不放在本 Gate，放到 Gate E。
TextureProducer 不创建 WGPU texture。
```

建议代码范围：

```text
rust/crates/engine_runtime/src/render_asset_production.rs
rust/crates/engine_runtime/src/runtime_asset_loader.rs
rust/crates/engine_runtime/src/render_resource.rs
rust/crates/engine_runtime/src/rhi_command_plan.rs
```

测试：

```powershell
cargo test -p engine_runtime render_asset_texture_production
cargo test -p engine_runtime render_asset_aui_image_texture_usage
cargo test -p engine_runtime render_asset_particle_texture_usage
```

验收：

```text
普通 texture / AUI image / particle texture 共享同一 producer 路径。
usage 能区分 Sprite2DTexture / AuiImageTexture / ParticleTexture。
缺失 asset、decode 失败、unsupported format 都进入统一 report。
```

## 7. Gate C：Material 最小 Binding

目标：

```text
实现 MaterialProducer。
新增 MaterialRenderAsset。
支持 material_model / shader_key / params / texture_slots / blend_mode / cull_mode。
输出 MaterialBinding 相关 RenderBindingSet。
支持 DefaultSpriteBasic / DefaultAuiBasic / DefaultParticleBasic fallback。
```

规则：

```text
本 Gate 不做完整 Material Graph。
MaterialProducer 不编译 shader graph。
Texture slot 只引用 RenderResourceHandle 或 production request 结果。
fallback 必须进入 report。
```

建议代码范围：

```text
rust/crates/engine_runtime/src/render_asset_production.rs
rust/crates/engine_runtime/src/render_resource.rs
rust/crates/engine_runtime/src/rhi_command_plan.rs
```

测试：

```powershell
cargo test -p engine_runtime render_asset_material_production
cargo test -p engine_runtime render_asset_material_binding_fallback
```

验收：

```text
material asset 能产出 MaterialRenderAsset。
texture slot 缺失时能 report fallback。
Sprite / AUI / Particle 能引用最小 material binding。
```

## 8. Gate D：Mesh Buffer Production

目标：

```text
实现 MeshProducer。
新增 MeshRenderAsset。
支持 vertex_layout / vertex_bytes / index_bytes / index_format / submeshes / bounds。
输出 mesh buffer RenderResourceRequest。
生成 mesh draw 可消费的 binding。
```

规则：

```text
本 Gate 不做 mesh optimizer。
本 Gate 不做 meshlet / Nanite-like 路线。
MeshProducer 不创建 backend buffer。
```

建议代码范围：

```text
rust/crates/engine_runtime/src/render_asset_production.rs
rust/crates/engine_runtime/src/render_resource.rs
rust/crates/engine_runtime/src/render_graph.rs
rust/crates/engine_runtime/src/rhi_command_plan.rs
```

测试：

```powershell
cargo test -p engine_runtime render_asset_mesh_production
cargo test -p engine_runtime render_asset_mesh_binding
```

验收：

```text
最小 quad mesh 或 triangle mesh 能产出 vertex/index resource request。
非法 vertex layout 能进入 InvalidDescriptor report。
DrawMeshBasic 能消费 mesh binding。
```

## 9. Gate E：Font Atlas Production

目标：

```text
实现 FontAtlasProducer。
新增 FontAtlasRenderAsset。
支持 atlas_texture / glyph_metadata / font_key / atlas_generation。
复用 Texture 类资源创建 atlas texture。
输出 DrawFontGlyphs 可消费 binding。
```

规则：

```text
本 Gate 不做完整 shaping / IME。
Font atlas texture 仍然走 texture resource 生产和上传。
glyph metadata 保留在 typed payload / binding metadata，不塞进 WGPU backend。
```

建议代码范围：

```text
rust/crates/engine_runtime/src/render_asset_production.rs
rust/crates/editor_wgpu_renderer/src/font_system.rs
rust/crates/engine_runtime/src/render_resource.rs
rust/crates/engine_runtime/src/rhi_command_plan.rs
```

测试：

```powershell
cargo test -p engine_runtime render_asset_font_atlas_production
cargo test -p editor_wgpu_renderer font
```

验收：

```text
font atlas 能生成 texture resource request。
glyph metadata 能被 DrawFontGlyphs 引用。
atlas generation 变化能触发新 request 或 reuse 判断。
```

## 10. Gate F：RenderTarget Production

目标：

```text
实现 RenderTargetProducer。
新增 RenderTargetRenderAsset。
支持 SurfaceRenderTarget / OffscreenRenderTarget / ViewportRenderTarget / IntermediateRenderTarget。
支持 size / format / clear_color / sample_count / usage_flags。
RenderGraph / RHI 可引用 RenderTarget handle。
```

规则：

```text
RenderGraph 可以引用 RenderTarget，但不拥有长期 RenderTarget 生命周期。
RenderResourceManager 负责创建、复用和释放。
sample_count 第一版固定为 1。
sampled usage 必须显式声明，不能默认把所有 render target 当 texture sample。
```

建议代码范围：

```text
rust/crates/engine_runtime/src/render_asset_production.rs
rust/crates/engine_runtime/src/render_resource.rs
rust/crates/engine_runtime/src/render_graph.rs
rust/crates/engine_runtime/src/engine_rhi.rs
```

测试：

```powershell
cargo test -p engine_runtime render_asset_render_target_production
cargo test -p engine_runtime render_graph
```

验收：

```text
surface target / offscreen target 都能生成 RenderResourceHandle。
RenderGraph pass 能引用 target handle。
非法 size / format 能进入 InvalidDescriptor report。
```

## 11. Gate G：RuntimeRenderer / EngineRHI / WgpuBackend 接入

目标：

```text
RuntimeRenderer 消费 RuntimeRenderAssetStore / RenderBindingSet。
RhiCommandPlan 引用 RenderResourceHandle / RenderBindingSet。
EngineRHI trait 接收 backend-neutral resource / binding / command descriptor。
WgpuBackend 只在最后阶段创建真实 WGPU texture / buffer / bind group。
```

规则：

```text
RuntimeRenderer 不直接 new wgpu object。
WgpuBackend 不 decode asset bytes。
WgpuBackend 不读 RuntimeAssetLoader。
所有 backend upload / binding 失败必须回写 report 或 backend diagnostics。
```

建议代码范围：

```text
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/rhi_command_plan.rs
rust/crates/engine_runtime/src/engine_rhi.rs
rust/crates/engine_runtime/src/wgpu_backend.rs
rust/crates/runtime_player_winit/src/lib.rs
```

测试：

```powershell
cargo test -p engine_runtime rhi_command_plan
cargo test -p engine_runtime runtime_renderer
cargo test -p engine_runtime wgpu_backend
cargo check -p runtime_player_winit --features real-window
```

验收：

```text
headless RHI path 能验证 resource / binding descriptor。
real-window feature 能编译通过。
WgpuBackend report 能说明 uploaded / reused / failed。
```

## 12. Gate H：全类型 Headless 验证与 Real-WGPU Smoke

目标：

```text
建立一个覆盖七类资源的 headless integration test。
建立 feature-gated real-wgpu smoke。
确保复杂打飞机样例所需的 texture / material / mesh / AUI / font / particle / render target 都有最小生产和绑定入口。
```

测试：

```powershell
cargo fmt --check
cargo test -p engine_runtime render_asset_production
cargo test -p engine_runtime render_resource
cargo test -p engine_runtime runtime_renderer
cargo test -p editor_wgpu_renderer font
cargo test -p runtime_player_winit
cargo check -p runtime_player_winit --features real-window
```

验收：

```text
七类资源都有最小成功路径。
七类资源都有至少一个失败诊断路径。
默认测试不依赖真实 GPU。
real-window / real-wgpu 只作为 feature-gated smoke，不阻塞默认 headless CI。
```

## 13. 完成后文档同步

施工完成后必须：

```text
1. 将本施工文档移动到：
   框架设计/引擎总体架构/施工文档/已完成/

2. 新增阶段完成记录：
   框架设计/引擎总体架构/阶段完成记录/2026-07-01-Runtime-Render-Asset-Production-and-Binding-v1/00-总览.md

3. 更新：
   框架设计/引擎总体架构/54-当前可自动化施工总入口.md
   框架设计/引擎总体架构/施工文档/README.md

4. 完成记录必须写明：
   实现了哪些 Gate。
   每个 Gate 跑了哪些测试。
   是否仍有 feature-gated real-wgpu smoke 没跑。
   是否有偏离 138 正式方案的地方。
```
