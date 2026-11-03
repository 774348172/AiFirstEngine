# 275-AUI Image Cooked Texture / GPU Present v1 方案

> 状态：正式方案，已获用户授权施工。
> 基线：复用 228 Real Texture Decode / GPU Texture Upload / Sprite Textured Present v1 与 210 Multi-stage UI Composition，不新增第二套资产或渲染体系。

## 1. 目标

补齐以下真实链路：

```text
AuiOverlayItemKind::Image + asset_id
  -> RuntimePackage RuntimeAssetIndex resolve
  -> cooked RGBA8 texture payload
  -> stable runtime texture handle + GPU upload
  -> painter-ordered textured UI batch
  -> DrawUiComposition(texture=Some(handle))
  -> WGPU alpha blending present
```

缺失 asset、cooked payload、GPU upload 或 resident handle 必须产生包含 `asset_id` 与失败 stage 的结构化诊断，禁止静默跳过或用纯色 quad 冒充图片。

## 2. Ownership

- `runtime_texture` 继续拥有 RuntimePackage texture resolve、cooked payload load、稳定 handle 与 CPU upload registry。
- `RuntimeRenderer` 只消费已解析的 AUI composition 与 runtime texture binding context；不读取项目目录、AUI binding 或 UI state。
- `RealWgpuBackend` 继续拥有 GPU texture resident registration 与 RGBA alpha pipeline。
- `EditorRuntimePlayInstance` 暴露当前 RuntimePackage 的 texture upload registry；真实 GameView present 在共享 WGPU device 上上传 registry 后执行 RHI plan。
- `runtime_player_winit` 使用同一 registry/handle 规则，不保留独立的 sprite-only texture 枚举实现。

## 3. Batching And Order

UI draw command 必须按 stage 内既有 draw-item painter order 生成。只允许相邻且 pipeline/texture 相同的 item 合批；不同 texture、solid geometry 与 font page 形成边界。Image clip 同时裁剪 position 与 UV，不能只裁 position。

第一版支持整张 RGBA8 texture 与现有 sampler；不新增 nine-slice、atlas subresource、mask、stencil、mipmap 或异步 streaming。

## 4. Diagnostics

结构化代码至少覆盖：

```text
aui_image.asset_id_missing
aui_image.texture_not_resolved
aui_image.texture_upload_failed
wgpu.texture_binding_missing
```

RuntimeRenderer report 记录 image request/resolved/missing 与 textured batch 数；真实 GPU present 将 backend upload/binding 诊断回传 GameView present report。

## 5. Validation

开发验证采用 owner/consumer closure：

- `engine_runtime`: resolve、stable handle、clip UV、same/different texture batching、missing diagnostics、alpha GPU readback。
- `editor_core` / `editor_window_winit`: RuntimePackage registry 暴露、共享 GPU upload、真实 plan consumer。
- `runtime_player_winit`: 共用 registry 与 sprite/AUI texture upload。
- 最终只用新 fresh root 重跑 Tower UI-V1 Gate E；Gate A-D、Local CI、Gate F/G/H 均不执行。

## 6. Red Lines

- 不写入 Tower 专用 asset id、路径、玩法或 UI token。
- Runtime 不扫描项目源目录，不读取真实配置。
- 不修改 production/安装态二进制。
- 不因本能力扩大到通用 UI 重构或完整资源 streaming。

## 7. 自审

方案未推翻 210/228；它只把已存在但未连接的 AUI Image consumer 接到同一 cooked texture 与 WGPU resident seam。Renderer 仍只消费 projection 结果，项目与引擎边界保持不变。

## 8. Gate E 暴露的多帧生命周期与调度约束

真实 Image present 后，Editor GameView 必须同时满足：

- 同一 session、target extent/format、cooked texture source hash 与 FontBundle generation 不变时，GPU backend、pipeline、texture 与 bind group 跨帧常驻；只有资源身份变化才重建，Stop/session replacement 必须退休旧资源。
- Runtime fixed tick deadline 以 16ms 累进，不能在一次较慢 present 后改写为 `now + 16ms` 并丢弃时间债务。
- Editor event loop 每次最多补 8 个 due tick，随后只 present 最新帧；超过上限时丢弃更旧债务，避免 spiral of death。inactive Play 重置 deadline，禁止空闲后突发补帧。

该约束是通用 Editor GameView 生命周期，不包含 Tower 分支，也不修改项目 gameplay、Gate timeout 或 production composition 边界。
