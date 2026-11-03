# 228-当前可自动化施工文档-Real Texture Decode / GPU Texture Upload / Sprite Textured Present v1

> 状态：当前唯一施工文档。
> 对应方案：`框架设计/引擎总体架构/228-Real-Texture-Decode-GPU-Texture-Upload-Sprite-Textured-Present-v1方案.md`。
> 选择：C-min，Build/Cook PNG -> cooked RGBA payload，RuntimePackage load，Sprite2D 真实 texture binding，RealWgpuBackend textured present。
> 日期：2026-07-09。

## 1. 施工目标

把复杂打飞机项目的 Sprite2D 贴图链路从 `sprite_binding_fallback` 推进到真实贴图闭环：

```text
Texture authoring asset / descriptor
  -> ProjectRuntimePackageAssembler
  -> RuntimePackageBuilder cooked texture metadata + rgba8 payload
  -> RuntimeAssetLoader / RuntimeTexturePayload
  -> RuntimeRenderAssetProducer / RenderResourceManager
  -> Sprite2DRenderPipeline real binding
  -> RhiCommandPlan DrawSpriteTextured
  -> RealWgpuBackend texture upload + sampler bind group
  -> headless report / optional local screenshot smoke
```

本轮不做 mipmap、压缩格式、atlas、sprite slicing/pivot、material graph、render thread 重构、批处理优化、完整纹理流式加载。

## 2. 涉及文件

预计修改：

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/editor_core/Cargo.toml
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/engine_runtime/src/lib.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/runtime_asset_loader.rs
rust/crates/engine_runtime/src/render_asset_production.rs
rust/crates/engine_runtime/src/sprite2d_render_pipeline.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/wgpu_backend.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/project_e2e_gate/src/*.rs
samples/complex_shooter_project/**
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

如施工中发现可收敛为更少文件，以实际最小修改为准。

## 3. 施工约束

```text
1. RuntimePackage 是 runtime 真相；runtime 不读取项目源 PNG，不扫描项目 Assets，不读取 editor Asset DB。
2. PNG decode 发生在 Build/Cook 或 editor_core assembly 侧；engine_runtime 默认路径只消费 cooked metadata + rgba8 bytes。
3. 不新增 TextureBridge；使用 RuntimeAssetLoader / RuntimeRenderAssetProducer / RenderResourceManager / Projection 既有边界。
4. RuntimeAssetLoader 不持有 wgpu 对象；GPU resident resource 放在 WgpuTextureStore / RealWgpuBackend 内部。
5. DrawSpriteTextured 不在 draw 时猜 AssetRef 或路径；texture upload 必须在 render_to_view 前通过明确 payload/register 路径准备。
6. Report 必须区分 runtime/editor 与 Off/Summary/Trace 思路；正式 runtime 默认只保留轻量 summary，不常驻长 trace。
7. 不把 Player / Enemy / Bullet / Weapon 等玩法名写进通用引擎 API；样例和 project_e2e_gate 可以出现项目名。
8. 真实 OS window / GPU screenshot 只作为 ignored/local-only smoke，不阻塞默认 CI。
```

## 4. Gate A：Cooked Texture Schema / Build Cook

任务：

```text
1. 新增 CookedTextureMetadata / RuntimeTexturePayload 等 schema-first 类型。
2. ProjectRuntimePackageAssembler 识别 texture source：
   - 直接 `.png`
   - texture `.asset` descriptor 指向 sourceImage
3. Build/Cook 阶段把 PNG 解码为 RGBA8 bytes，并写出：
   - cooked/textures/{assetId}.texture.json
   - cooked/textures/{assetId}.rgba8
4. RuntimeAssetIndex entry 指向 metadata，而不是 source PNG。
5. Build report / assembly diagnostics 能报告 decode_failed / missing_source / invalid_descriptor。
```

测试：

```text
cargo test -p editor_core project_runtime_package_assembler
cargo test -p engine_runtime runtime_package_builder
```

## 5. Gate B：Runtime Texture Payload Load / Diagnostics

任务：

```text
1. RuntimePackageLoader 能加载 cooked texture metadata。
2. RuntimeAssetLoader 保持通用 bytes/handle 边界。
3. 新增 texture payload reader 通过 RuntimeAssetIndex + cooked metadata 读取 rgba8 payload。
4. byteLength 必须等于 width * height * 4；缺 metadata / 缺 payload / 长度不匹配都有结构化 diagnostics。
```

测试：

```text
cargo test -p engine_runtime runtime_package
cargo test -p engine_runtime runtime_asset
```

## 6. Gate C：RenderAssetPrepare / Sprite2D Binding

任务：

```text
1. Sprite2DRenderPipeline 增加可选 asset prepare context。
2. 有 context 时按 sprite_ref resolve texture asset，生成真实 RenderBindingSet。
3. 无 context 时保留原 fallback 行为和诊断，避免破坏旧 headless 单元测试。
4. RuntimeRenderer 支持传入 Sprite2D asset prepare context。
5. RhiCommandPlan 的 DrawSpriteTextured 必须携带 real texture binding，fallback_count 可被测试断言。
```

测试：

```text
cargo test -p engine_runtime sprite2d
cargo test -p engine_runtime render_asset
cargo test -p engine_runtime runtime_renderer
```

## 7. Gate D：RealWgpuBackend Textured Quad

任务：

```text
1. RealWgpuBackend 新增 WgpuTextureStore。
2. WgpuTextureStore 以 RenderResourceHandle + generation 为 key 保存 Texture/View/Sampler/BindGroup。
3. textured shader 支持 uv + sampler2D。
4. render_to_view 前根据 RhiCommandPlan / registered payload 完成 upload/register。
5. DrawSpriteTextured 采样真实 texture；缺 resident texture 时输出 bind_failed / upload_failed diagnostics。
```

测试：

```text
cargo test -p engine_runtime --features real-wgpu wgpu_backend
```

说明：GPU adapter 不可用时，已有 real-wgpu 测试可按现有模式跳过或返回明确 environment diagnostic；不得把纯色 fallback 伪装成真实 texture present。

## 8. Gate E：复杂打飞机 P0-1 验收

任务：

```text
1. samples/complex_shooter_project 至少提供 player/enemy/background 三个真实 texture asset。
2. ProjectRuntimePackageAssembler -> RuntimePackageBuilder 输出 cooked texture count >= 3。
3. project_e2e_gate 新增 real_texture_present report。
4. 验证 player/enemy/background sprite texture status = real_texture_present。
5. fallback_count = 0，除非测试显式构造 missing fixture。
```

测试：

```text
cargo test -p project_e2e_gate real_texture_present
cargo check -p runtime_player_winit --features real-window
cargo test -p runtime_player_winit real_windowed_player_screenshot_smoke --features real-window -- --ignored --nocapture
```

说明：最后一条为 local-only optional smoke；默认完成以 `project_e2e_gate real_texture_present` 和 engine_runtime headless / real-wgpu 单元测试为准。

## 9. 整体回归

全部 Gate 完成后执行：

```text
cargo fmt --check
cargo test -p editor_core project_runtime_package_assembler
cargo test -p engine_runtime runtime_package_builder
cargo test -p engine_runtime runtime_asset
cargo test -p engine_runtime sprite2d
cargo test -p engine_runtime render_asset
cargo test -p engine_runtime runtime_renderer
cargo test -p engine_runtime --features real-wgpu wgpu_backend
cargo test -p project_e2e_gate real_texture_present
cargo test -p project_e2e_gate
```

如果 `real-wgpu` 因本机 GPU/driver 环境失败，必须记录为 environment_blocked，并保证 headless deterministic gate 通过。

## 10. 文档同步与归档

施工完成且整体回归通过后：

```text
1. 生成阶段完成记录：
   框架设计/引擎总体架构/阶段完成记录/2026-07-09-Real-Texture-Decode-GPU-Texture-Upload-Sprite-Textured-Present-v1/00-总览.md
2. 更新：
   - 49-当前状态与下一步入口.md
   - 54-当前可自动化施工总入口.md
   - 施工文档/README.md
   - 阶段完成记录/README.md
   - 227 优先级文档中 P0-1 状态，如已有状态列表
3. 把本文档从 `施工文档/当前/` 移动到 `施工文档/已完成/`。
```

## 11. 施工文档自审

```text
是否严格对应 228 方案：
  是。只做 C-min cooked texture payload、runtime load、Sprite2D binding、RealWgpu texture present。

是否扩大范围：
  否。不做 mipmap、压缩、atlas、slicing、material graph、batching、streaming、完整 render thread。

是否保持 RuntimePackage 真相：
  是。runtime 不读项目源 PNG；sourceImage 只在 Build/Cook 阶段使用。

是否遵守 Projection / Bridge 规则：
  是。不新增 TextureBridge，只接入 RuntimeAssetLoader、RuntimeRenderAssetProducer、RenderResourceManager 和 Sprite2D pipeline。

是否 AI-first 可验证：
  是。每个 Gate 都有结构化 report 或 cargo test；失败必须能定位 cook/load/payload/prepare/upload/bind。

是否处理 runtime/editor report 档位：
  是。默认 runtime 只做轻量 summary；trace 只用于 gate/debug。

是否让真实 GPU 测试阻塞默认 CI：
  否。真实窗口 screenshot 明确 local-only optional；默认 gate 使用 headless deterministic 与可运行 real-wgpu 单元测试。

是否保护通用引擎 API：
  是。通用 API 只出现 texture/sprite/resource 语义，不引入复杂打飞机玩法名。
```
