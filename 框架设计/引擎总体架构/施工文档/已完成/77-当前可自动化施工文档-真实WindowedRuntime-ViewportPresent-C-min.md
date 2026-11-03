# 已完成施工文档：真实 Windowed Runtime / Viewport Present C-min

> 状态：已完成。本文档保留为 77 号施工计划和复盘依据，不再作为当前开工入口；当前入口以 `../../49-当前状态与下一步入口.md` 和 `../../54-当前可自动化施工总入口.md` 为准。

长期规则见：

```text
17-Runtime-FrameLoop.md
27-Renderer-MVP.md
37-Editor-Core与可迁移UI路线.md
47-Native-Editor-Host-BC路线.md
59-真实WgpuBackend-RDG-RHI最小门禁方案.md
71-Runtime-Viewport-Rendering-System方案.md
```

前置施工记录：

```text
施工文档/已完成/66-当前可自动化施工文档-Native-Editor-RealWindow-ViewportSurface-C-min.md
施工文档/已完成/71-当前可自动化施工文档-Runtime-Viewport-Rendering-System-D-min.md
施工文档/已完成/76-当前可自动化施工文档-真实WgpuSurface-GPUTextureLifetime-RenderThread-C-min.md
```

## 1. 施工目标

把当前已有 headless viewport / RenderThread / surface lifecycle 推进到“本机真实窗口能 present 一帧”的最小闭环：

```text
NativeEditorHost / WindowedRuntimeHost
  -> Winit Window
  -> Wgpu Surface
  -> Runtime tick one frame
  -> RenderThreadFrameInput
  -> RuntimeRenderer
  -> RDG
  -> RHI Command Plan
  -> acquire surface texture
  -> clear / minimal draw
  -> submit
  -> present
  -> WindowedRuntimePresentReport
```

完成后至少证明：

```text
1. 可以在 feature-gated 路径打开真实 OS window。
2. 可以创建 wgpu surface / device / queue 并 configure surface。
3. Runtime 可以跑一帧并提交 RenderThread。
4. RenderThread 可以输出到真实 surface target。
5. 可以 acquire / render / present 一帧。
6. 失败能通过结构化 report 定位到 window / surface / device / render_thread / rdg / rhi / present。
7. 默认自动化仍然 headless，不依赖本机 GPU / driver / window。
```

## 2. 成熟引擎对照

| 引擎 | 对应机制 | 本轮借鉴 | 本轮不做 |
|---|---|---|---|
| Unreal | Platform Window -> Viewport -> SceneViewport -> RenderThread -> RHI Present | 学窗口、Viewport、RenderThread、RHI present 分层 | 不做完整 Slate / FSceneViewport / 多窗口 / swapchain 多帧同步 |
| Unity | PlayerLoop -> GameView / SceneView RenderTexture -> Native Graphics Present | 学用户心智简单的 GameView / SceneView present 入口 | 不做完整编辑器 GameView 工具链，不复制 Unity native 黑箱 |
| Bevy | winit Window + wgpu Surface + RenderApp Extract/Prepare/Queue/Render | 学 Rust + winit + wgpu 的真实窗口路径和 headless 分离 | 不引入 Bevy Schedule / RenderWorld 复杂度 |
| Godot | DisplayServer / RenderingServer / Viewport present | 学 Display / Render server 分离和 viewport 输出 | 不采用 Node / Server 全套架构 |

项目路线：

```text
UE-like Window / Viewport / RenderThread / RHI present 分层
+ Bevy-like winit / wgpu 第一真实后端
+ Unity-like GameView 可理解入口
+ AI-first headless report / smoke gate
```

## 3. 硬规则

### 3.1 真实窗口只作为 feature-gated smoke gate

```text
默认 cargo test --workspace 不打开真实窗口。
默认自动化仍走 HeadlessSurfaceBackend / HeadlessTextureBackend。
真实窗口路径必须挂在 real-window / real-wgpu-surface feature 后。
真实 GPU / driver / window 失败不能阻塞默认 headless 测试。
```

### 3.2 不绕过既有渲染边界

```text
WindowedRuntimeHost 不能直接读写 Gameplay ECS。
WindowedRuntimeHost 不能直接 present RenderSceneState。
RuntimeRenderer 不能绕过 RDG / RHI 直接调用 wgpu。
WgpuSurfaceBackend 必须挂在 EngineRHI / Backend 边界后。
Editor UI Renderer 不渲染游戏世界。
Runtime Renderer 不渲染编辑器面板。
```

### 3.3 本轮只做 present，不做完整 GPU 资源系统

本轮可以 clear color / test geometry / minimal draw。

本轮不做：

```text
真实 mesh / material / texture GPU upload。
完整 shader / pipeline / bind group 系统。
完整 GPU resource pool。
完整 swapchain 多帧同步。
完整跨线程 RenderThread 队列。
多窗口。
移动端窗口。
平台发布。
```

## 4. 本轮做什么

### A1：WindowedRuntimePresent 数据结构

新增或扩展：

```text
WindowedRuntimeConfig
WindowedRuntimeFrameInput
WindowedRuntimeFrameOutput
WindowedRuntimePresentReport
WindowedRuntimeDiagnostic
RealSurfaceTarget
RealSurfacePresentStatus
```

字段保持少而精：

```text
schema_version
frame_index
window_id
target_id
surface_size
surface_format
present_mode
runtime_tick_status
render_thread_status
rdg_status
rhi_status
acquire_status
submit_status
present_status
diagnostics[]
```

最小测试：

```text
windowed_runtime_present_report_is_json_serializable
windowed_runtime_present_report_locates_surface_failure
windowed_runtime_present_report_locates_render_failure
```

### A2：HeadlessWindowedRuntimePresentBackend

在不打开真实窗口的情况下模拟：

```text
create_window
create_surface
configure_surface
tick_runtime_frame
render_thread_frame
acquire_surface_texture
submit
present
resize
surface_lost
```

最小测试：

```text
headless_windowed_runtime_presents_one_frame
headless_windowed_runtime_reports_resize
headless_windowed_runtime_reports_surface_lost
```

### A3：真实 winit + wgpu surface smoke path

feature：

```text
real-window
real-wgpu-surface
```

第一版只要求：

```text
创建一个 winit window。
创建 wgpu instance / adapter / device / queue / surface。
configure surface。
clear 一帧。
present 一帧。
输出 WindowedRuntimePresentReport。
```

建议命令：

```powershell
cargo test -p editor_window_winit --features real-window,real-wgpu-surface real_windowed_runtime_present_smoke -- --ignored --nocapture
```

如果当前 crate feature 组合不同，以实际 Cargo.toml 为准，但必须保持显式 feature gate。

### A4：Runtime one-frame 接入

使用已有：

```text
EngineHostLoop
ProjectLogicRunner
RenderExtract
RenderSceneState
RenderThread
RuntimeRenderer
```

本轮只要求一帧：

```text
load minimal runtime package or construct minimal test scene
tick one frame
submit RenderThreadFrameInput
produce RuntimeRendererOutput
present to target
```

最小测试：

```text
windowed_runtime_host_ticks_one_frame_before_present
windowed_runtime_host_present_report_contains_render_thread_report
```

### A5：Surface resize / lost 诊断

必须覆盖：

```text
resize 后重新 configure surface。
surface lost 后输出 diagnostic。
acquire 失败不 panic。
present 失败不 panic。
```

最小测试：

```text
windowed_runtime_resize_reconfigures_surface
windowed_runtime_surface_lost_generates_diagnostic
windowed_runtime_present_failure_generates_diagnostic
```

## 5. 建议代码落点

优先沿用现有 crate，不新增大 crate：

```text
rust/crates/editor_window_winit/src/lib.rs
rust/crates/engine_runtime/src/render_thread.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/gpu_texture_lifetime.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/editor_host/src/main.rs
```

如果实现中发现 `editor_window_winit/src/lib.rs` 继续膨胀，可以只做轻量拆分：

```text
rust/crates/editor_window_winit/src/windowed_runtime_present.rs
rust/crates/editor_window_winit/src/real_wgpu_surface.rs
```

不要为了拆 crate 阻塞本轮闭环。

## 6. 本轮不做什么

```text
真实 mesh / material / texture GPU upload。
真实 Asset -> GPU resource pipeline。
完整 bind group / shader / pipeline cache。
完整 RenderGraph GPU resource aliasing。
完整 GPU resource pool。
完整跨线程 RenderThread。
完整 editor SceneView gizmo / picking。
多窗口 / 多 surface。
Android / iOS / Web surface。
真实平台发布。
```

## 7. 测试门禁

默认必须通过：

```powershell
cd G:\gameEngin\rust
cargo fmt
cargo test --workspace
```

定向测试：

```powershell
cargo test -p editor_window_winit headless_windowed_runtime
cargo test -p engine_runtime render_thread
cargo test -p engine_runtime gpu_texture_lifetime
```

feature 编译门禁：

```powershell
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

本机真实窗口 smoke gate：

```powershell
cargo test -p editor_window_winit --features real-window,real-wgpu-surface real_windowed_runtime_present_smoke -- --ignored --nocapture
```

说明：

```text
ignored smoke 允许因本机 GPU / driver / window 环境失败而输出 diagnostic。
默认 headless 测试不得依赖 ignored smoke。
```

## 8. 完成标准

```text
1. 存在 WindowedRuntimePresentReport。
2. headless windowed runtime present 一帧测试通过。
3. real-window / real-wgpu-surface feature 编译通过。
4. 如果本机环境允许，ignored smoke 能打开窗口并 present 一帧。
5. resize / surface lost / acquire failure / present failure 都有结构化 diagnostic。
6. EngineHostLoop -> RenderThread -> RuntimeRenderer 的边界不被破坏。
7. 默认 cargo test --workspace 通过。
8. 文档同步完成：49 / 54 / 18 / README / 00 / 施工文档 README。
9. 完成后新增阶段完成记录，并把本文档移到施工文档/已完成。
```

## 9. 失败降级规则

如果真实窗口 smoke 因环境失败：

```text
不回退到绕过 RDG / RHI。
不让默认测试依赖真实窗口。
保留 headless present report。
把真实失败写入 WindowedRuntimePresentReport.diagnostics。
```

如果 wgpu API 或平台差异导致实现复杂：

```text
先完成 feature 编译门禁。
先完成 headless present C-min。
真实 smoke 可以 ignored，但必须有清晰 diagnostic。
```

## 10. 完成后下一步

本轮完成后，优先级再回到：

```text
真实 OS file watcher / Import Worker。
真实 mesh / material / texture GPU upload。
真实 Bundle 二进制打包 / 平台 cook。
```
