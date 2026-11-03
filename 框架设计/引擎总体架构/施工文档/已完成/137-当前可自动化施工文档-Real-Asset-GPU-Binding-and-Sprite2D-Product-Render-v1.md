# 137-当前可自动化施工文档-Real Asset GPU Binding and Sprite2D Product Render v1

## 1. 对应规则文档

```text
../137-Real-Asset-GPU-Binding-and-Sprite2D-Product-Render-v1方案.md
```

本施工文档只执行已经确认的 C-min 路线：

```text
RuntimeAssetIndex
  -> RuntimeAssetLoader
  -> RuntimeAssetTypeLoader
  -> RenderAssetPrepare
  -> RenderResourceManager
  -> RenderResourceHandle
  -> SpriteMaterialBinding
  -> EngineRHI
  -> WgpuBackend
```

## 2. 施工原则

```text
一个 Gate 完成后必须跑对应测试，再进入下一个 Gate。
不得让 WgpuBackend 直接解析 AssetRef 或项目资源路径。
不得新增独立 Bridge，历史 RenderAssetBridge 名称只作为兼容实现入口。
不得加入打飞机专用 API。
默认 headless 测试必须可运行；真实 WGPU 测试只能作为 feature-gated gate。
```

## 3. Gate A：RenderAssetPrepare 数据结构和报告

目标：

```text
新增 RenderAssetPrepareRequest。
新增 RenderAssetUsage。
新增 PreparedRenderAsset / PreparedRenderAssetStatus。
新增 SpriteMaterialBinding / SpriteMaterialHandle / SpriteSampler / SpriteBlendMode。
新增 RenderAssetPrepareReport / RenderAssetPrepareEvent。
把现有 RenderAssetBridgeReport 兼容映射到 RenderAssetPrepareReport。
```

代码范围：

```text
rust/crates/engine_runtime/src/render_asset_bridge.rs
rust/crates/engine_runtime/src/lib.rs
```

测试：

```powershell
cargo test -p engine_runtime render_asset_prepare
```

验收：

```text
能构造 SpriteTexture prepare request。
能生成 Ready / MissingRuntimeAsset / UnsupportedFormat 报告。
报告包含 asset_ref / stage / code / source_entity_id / source_proxy_id。
旧 RenderAssetBridge 测试不回退。
```

## 4. Gate B：RuntimeAsset texture/material -> RenderResourceRequest

目标：

```text
RenderAssetPrepare 根据 RuntimeAssetHandle / RuntimeAssetRecord 生成 Texture / MaterialParams RenderResourceRequest。
支持 texture/material 两类第一版资源。
生成 PreparedRenderAsset，并可把 RenderResourceManager 返回的 handle 写入结果。
缺失或不支持类型必须输出 RenderAssetPrepareReport。
```

代码范围：

```text
rust/crates/engine_runtime/src/render_asset_bridge.rs
rust/crates/engine_runtime/src/render_resource.rs
rust/crates/engine_runtime/src/runtime_asset_loader.rs
```

测试：

```powershell
cargo test -p engine_runtime render_asset_prepare
cargo test -p engine_runtime render_resource
```

验收：

```text
texture asset -> RenderResourceKind::Texture。
material asset -> RenderResourceKind::MaterialParams。
request_resource 后 PreparedRenderAsset.status = Ready。
unsupported asset -> UnsupportedFormat / Failed。
```

## 5. Gate C：SpriteDrawItem -> SpriteMaterialBinding -> DrawSpriteTextured

目标：

```text
SpriteRenderer2D 的 sprite_ref 形成 SpriteTexture prepare request。
material_ref 为空时使用 DefaultSpriteBasic。
RHI command plan 中区分 DrawSpriteBasic 与 DrawSpriteTextured。
缺失纹理时 report 诊断，不静默成功。
```

代码范围：

```text
rust/crates/engine_runtime/src/render_state.rs
rust/crates/engine_runtime/src/renderer_feature_builder.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/render_graph.rs
rust/crates/engine_runtime/src/rhi_command_plan.rs
```

测试：

```powershell
cargo test -p engine_runtime sprite
cargo test -p engine_runtime rhi_command_plan
```

验收：

```text
SpriteDrawItem 携带 sprite_ref / material_ref。
DefaultSpriteBasic fallback 可诊断。
DrawPlan 按 sorting_layer / order_in_layer / sort_z / proxy_id 稳定排序。
```

## 6. Gate D：WgpuBackend texture/sampler/bind group 最小真实绑定

目标：

```text
WgpuBackend 消费 RHI texture binding descriptor，不解析 AssetRef。
支持 1x1 / decoded RGBA texture upload 的最小路径。
支持 DefaultSpriteBasic textured quad。
```

代码范围：

```text
rust/crates/engine_runtime/src/wgpu_backend.rs
rust/crates/engine_runtime/src/engine_rhi.rs
```

测试：

```powershell
cargo test -p engine_runtime wgpu_backend
cargo check -p runtime_player_winit --features real-window
```

验收：

```text
headless 默认测试不依赖真实 GPU。
real-window feature 能编译。
WgpuBackend 报告 texture binding ready / failed。
```

## 7. Gate E：RuntimePackage / WindowedPlayer 集成

目标：

```text
WindowedPlayer 加载 RuntimePackage 后能生成真实 texture binding summary。
RuntimePackage 中 scene + texture + SpriteRenderer2D 能走到 DrawSpriteTextured。
缺失 asset 能输出 RuntimeAsset / RenderAssetPrepare / RenderResource 三层诊断。
```

代码范围：

```text
rust/crates/engine_runtime/src/windowed_player.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
```

测试：

```powershell
cargo test -p engine_runtime windowed_player
cargo test -p runtime_player_winit
cargo check -p runtime_player_winit --features real-window
```

验收：

```text
WindowedPlayerGpuBindingSummary 从 descriptor summary 提升为 prepare/resource summary。
包内 texture 可被 RuntimeAssetIndex 解析并绑定。
缺失 texture 不静默成功。
```

## 8. 完成后记录

施工完成后新增：

```text
阶段完成记录/2026-07-01-Real-Asset-GPU-Binding-and-Sprite2D-Product-Render-v1/00-总览.md
```

并将本施工文档移动到：

```text
施工文档/已完成/
```

完成记录必须写明：

```text
实现了哪些 Gate。
每个 Gate 跑了哪些测试。
哪些能力仍是后续系统范围。
是否满足 130 文档的 M4 / M5 P0 验收。
```
