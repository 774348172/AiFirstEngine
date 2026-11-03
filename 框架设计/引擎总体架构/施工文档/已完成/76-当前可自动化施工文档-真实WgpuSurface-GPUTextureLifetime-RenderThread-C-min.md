# 已完成施工文档：真实 Wgpu Surface / GPU Texture Lifetime / Render Thread C-min

> 状态：已完成。本文档保留为 76 号施工计划和复盘依据，不再作为当前开工入口；当前入口以 `../../49-当前状态与下一步入口.md` 和 `../../54-当前可自动化施工总入口.md` 为准。

长期规则见：

```text
17-Runtime-FrameLoop.md
27-Renderer-MVP.md
47-Native-Editor-Host-BC路线.md
50-RenderCommand-RenderSceneState方案.md
55-RenderSceneState-RenderProxy-v1方案.md
59-真实WgpuBackend-RDG-RHI最小门禁方案.md
71-Runtime-Viewport-Rendering-System方案.md
```

## 1. 施工目标

在 67 / 71 已完成的 headless RDG / RHI / RuntimeViewport 基座上，补齐真实可玩最小循环里最关键的渲染缺口：

```text
RenderSceneState
  -> Render Thread C-min
  -> RuntimeRenderer
  -> RenderGraph
  -> RhiCommandPlan
  -> WgpuSurface / ViewportGpuTexture
  -> Present / TextureDescriptor
  -> RuntimeRenderFrameOutput / RenderThreadReport
```

本轮完成后，项目至少要能证明：

```text
1. RenderThread 有独立边界，不再只是 Runtime/Game Thread 内直接调用渲染。
2. GPU texture 有明确创建、resize、acquire、present、release / lost 记录。
3. Wgpu surface 真实路径可以作为 feature-gated smoke gate。
4. 默认自动化仍能 headless 测试，不依赖本机 GPU / driver / window。
5. AI 和用户能通过报告判断失败发生在 RenderThread、Surface、Texture、RDG、RHI 还是 Backend。
```

## 2. 前置条件

已完成：

```text
67-真实WgpuBackend-RDG-RHI最小门禁-C-min：
  RenderGraph / RHI Command Plan / HeadlessRhiBackend / WgpuBackend skeleton。

71-Runtime-Viewport-Rendering-System-D-min：
  RuntimeRenderTarget / ViewportTextureDescriptor / RuntimeRenderFrameOutput / ViewportHost slot。

74-Native-Editor-Viewport输入回流RuntimeFrame-C-min：
  Viewport 输入能进入 Runtime Action / Game Frame。

75-真实RustRuntimeCLI-ProcessSpawn-C-min：
  BuildRunOrchestrator 能真实启动 staged Rust runtime。
```

本轮不重复实现这些内容，只在它们之上补真实 GPU surface / texture lifetime / Render Thread C-min。

## 3. 成熟引擎对照

| 引擎 | 对应机制 | 本轮借鉴 | 本轮不做 |
|---|---|---|---|
| Unreal | Game Thread -> Render Command -> Render Thread -> RDG -> RHI -> Backend | 学 Game / Render 线程隔离、RenderSceneState owner、RDG/RHI 分层、结构化同步点 | 不做完整 TaskGraph、并行渲染器、Nanite/Lumen、完整资源池 |
| Unity | PlayerLoop -> RenderPipeline -> CommandBuffer -> native graphics backend -> GameView/SceneView RenderTexture | 学稳定渲染入口、RenderTexture / GameView 嵌入、用户心智简单 | 不开放用户直接写底层 CommandBuffer，不复制 Unity native 黑箱 |
| Bevy | Main World -> Extract -> RenderApp / Render World -> RenderGraph -> wgpu | 学 Rust + wgpu + Extract 可行性、headless 与 real backend 分离 | 不暴露 Bevy Schedule / RenderWorld 复杂度给 AI 默认层 |
| Godot | Scene tree -> RenderingServer -> RenderingDevice / backend | 学渲染服务隔离项目对象与 GPU | 不采用 Node 体系，不把渲染服务变成项目规则入口 |

项目路线：

```text
UE-like Game / Render 隔离
+ UE-like RDG / RHI 分层
+ Bevy-like Rust / wgpu 第一真实后端
+ Unity-like texture-first viewport 嵌入
+ AI-first report / trace / headless gate
```

## 4. 硬规则

### 4.1 Render Thread 是边界，不是玩法线程

```text
Render Thread 只消费 RenderCommand / RenderSceneState / RenderResourceHandle / RHI plan。
Render Thread 不读写 Gameplay ECS。
Render Thread 不执行项目规则。
Render Thread 不修改 Runtime World。
```

第一版允许 worker_count=1 / same-process executor 做确定性测试，但 API 和报告必须体现线程域边界。

### 4.2 真实 surface 只能作为 smoke gate

```text
默认测试路径：HeadlessSurfaceBackend / HeadlessTextureBackend。
真实 winit + wgpu surface：feature-gated local smoke gate。
真实 GPU 失败不能阻塞默认 headless CI。
```

### 4.3 GPU texture lifetime 必须显式

每个 runtime viewport / surface texture 必须有可追踪生命周期：

```text
create
configure / resize
acquire
render
present
release / recycle
lost / recreate
```

报告必须能说明：

```text
texture_id
target_id
frame_index
size
format
state
owner
last_error
```

### 4.4 不绕过 RDG / RHI

```text
RuntimeRenderer 不能直接调用 wgpu。
RenderSceneState 不能直接 present。
WgpuSurfaceBackend 必须挂在 EngineRHI / Backend 后面。
```

## 5. 本轮做什么

### A1：RenderThread C-min 数据边界

新增或扩展：

```text
RenderThread
RenderThreadConfig
RenderThreadCommand
RenderThreadFrameInput
RenderThreadFrameOutput
RenderThreadReport
RenderThreadDiagnostic
```

第一版能力：

```text
接收 RenderCommand batch / RenderViewState / RuntimeRenderTarget。
维护或引用 RenderSceneState owner 边界。
调用 RuntimeRenderer -> RDG -> RHI -> Backend。
输出 RuntimeRenderFrameOutput 和 RenderThreadReport。
```

最小测试：

```text
render_thread_consumes_render_commands_without_reading_world
render_thread_outputs_frame_report
render_thread_keeps_scene_state_owner_boundary
```

### A2：GPU Texture Lifetime v1

新增或扩展：

```text
GpuTextureId
GpuTextureDescriptor
GpuTextureState
GpuTextureLifetimeEvent
GpuTextureLifetimeReport
RuntimeGpuTextureRegistry
```

第一版状态：

```text
Created
Configured
Acquired
Rendered
Presented
Released
Lost
```

最小测试：

```text
gpu_texture_lifetime_records_create_acquire_present_release
gpu_texture_lifetime_records_resize_recreate
gpu_texture_lifetime_reports_surface_lost
```

### A3：Headless Surface / Texture Backend 升级

在不依赖真实 GPU 的情况下模拟：

```text
create_surface
configure_surface
acquire_surface_texture
present
surface_lost
resize
```

最小测试：

```text
headless_surface_backend_acquires_and_presents_texture
headless_surface_backend_reports_surface_lost
headless_surface_backend_reconfigures_on_resize
```

### A4：真实 Wgpu Surface smoke gate

新增 feature-gated 真实路径：

```text
real-wgpu-surface
```

第一版只要求：

```text
创建 winit window。
创建 wgpu instance / adapter / device / queue / surface。
configure surface。
clear 一帧。
present 一帧。
输出 RealWgpuSurfaceReport。
```

边界：

```text
默认 cargo test 不启用该 feature。
本机 smoke 命令可单独运行。
失败只生成 diagnostic，不影响 headless 默认测试。
```

建议 smoke 命令：

```powershell
cargo test -p editor_window_winit --features real-wgpu-surface real_wgpu_surface_smoke -- --ignored --nocapture
```

实际命令以当前 crate 结构为准。

### A5：RuntimeRenderer 接 RenderThread

把现有路径：

```text
EngineHostLoop / FrameLoop
  -> RenderExtract
  -> RuntimeRenderer / HeadlessRhiBackend
```

收敛为：

```text
EngineHostLoop / FrameLoop
  -> RenderExtract
  -> RenderThreadFrameInput
  -> RenderThread
  -> RuntimeRenderer
  -> RuntimeRenderFrameOutput
```

第一版可同进程同步执行，但不允许命名和接口继续表现成“Game Thread 直接渲染”。

最小测试：

```text
engine_host_loop_submits_render_frame_to_render_thread
runtime_frame_output_contains_texture_lifetime_report
runtime_frame_report_identifies_render_thread_stage
```

### A6：报告与诊断

新增或扩展：

```text
RenderThreadReport
GpuTextureLifetimeReport
RealWgpuSurfaceReport
RuntimeRenderFrameOutput
RuntimeRenderDiagnostics
```

字段少而精：

```text
schema_version
frame_index
target_id
target_kind
thread_mode
backend
surface_state
texture_events[]
rdg_status
rhi_status
present_status
diagnostics[]
```

最小测试：

```text
render_thread_report_is_json_serializable
render_report_locates_backend_failure_layer
render_report_locates_surface_lost_layer
```

## 6. 本轮不做什么

```text
完整 UE 级 Render Thread。
完整跨线程无锁队列。
完整 GPU resource pool。
完整 swapchain 多帧同步。
完整 wgpu shader / pipeline / bind group 系统。
完整 mesh / material / texture GPU 上传系统。
完整 D3D12 / Vulkan / Metal backend。
PBR / shadow / post process。
复杂 visibility / culling / batching / instancing。
用户项目逻辑控制 RenderGraph / RHI。
真实编辑器复杂 SceneView gizmo / selection picking。
真实平台发布 GPU 初始化。
```

## 7. 建议代码落点

优先保持现有 engine_runtime / editor_window_winit / editor_host 分层，不急着新增大 crate。

可能落点：

```text
rust/crates/engine_runtime/src/render_thread.rs
rust/crates/engine_runtime/src/gpu_texture_lifetime.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/headless_rhi_backend.rs
rust/crates/engine_runtime/src/wgpu_backend.rs
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_host/src/main.rs
```

如果当前结构更适合拆 crate，可以后续再拆：

```text
rust/crates/engine_renderer
```

但本轮不要为了拆 crate 阻塞闭环。

## 8. 测试门禁

默认必须通过：

```powershell
cd G:\gameEngin\rust
cargo test --workspace
```

建议新增定向测试：

```powershell
cargo test -p engine_runtime render_thread
cargo test -p engine_runtime gpu_texture_lifetime
cargo test -p editor_window_winit surface
```

如果实现真实 smoke feature，再补：

```powershell
cargo test -p editor_window_winit --features real-wgpu-surface real_wgpu_surface_smoke -- --ignored --nocapture
```

Headless 自动化验收：

```text
1. 不需要真实 GPU 也能验证 surface lifecycle 模拟路径。
2. 不需要真实窗口也能验证 RenderThread C-min。
3. RuntimeRenderFrameOutput 能解释 texture / surface / render thread 状态。
4. Surface lost / resize / present failure 都能产出结构化 diagnostic。
```

## 9. 完成标准

```text
1. RenderThread C-min 存在，并成为 RuntimeRenderer 的正式入口。
2. GPU Texture Lifetime v1 存在，并覆盖 create / acquire / present / release / lost。
3. HeadlessSurfaceBackend 能模拟真实 surface lifecycle。
4. Real Wgpu Surface smoke gate 有 feature-gated 路径。
5. RuntimeRenderFrameOutput 包含 render thread / texture lifetime / surface present 信息。
6. 默认 cargo test --workspace 通过。
7. 文档同步完成：49 / 54 / 18 / README / 00 / 施工文档 README。
8. 施工完成后新增阶段完成记录，并把本文档移动到 施工文档/已完成。
```

## 10. 下一阶段候选

本轮完成后再看测试结果决定下一步：

```text
真实 OS file watcher / Import Worker。
真实 mesh / material / texture GPU upload。
真实 Bundle 二进制打包 / 平台 cook。
真实 windowed runtime run。
```
