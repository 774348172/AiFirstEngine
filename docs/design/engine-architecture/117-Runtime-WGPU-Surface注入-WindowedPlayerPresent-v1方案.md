# 117-Runtime WGPU Surface 注入 / Windowed Player Present v1 方案

## 问题

116 已经完成真实 `WgpuBackend` 完整 v1 的 offscreen 执行路径：

```text
RuntimeRenderer
  -> RenderGraph
  -> RhiCommandPlan v2
  -> EngineRhiBackend
  -> RealWgpuBackend offscreen
```

但真实窗口路径仍然没有把 `wgpu::Surface` 注入到 Runtime WGPU backend。下一步要补齐：

```text
WindowedRuntimeHost
  -> winit Window
  -> wgpu Surface
  -> Runtime RealWgpuBackend
  -> RhiCommandPlan
  -> submit / present
```

关键边界：

```text
RuntimeRenderer 不知道 window / winit / wgpu surface。
Window Host 负责 OS Window / Surface 生命周期。
WgpuBackend 负责 RHI command -> GPU command -> present。
```

## 其它引擎对比

| 引擎 | 路线 | 可借鉴点 | 不照搬点 |
|---|---|---|---|
| Unreal Engine | Platform Window -> Viewport / SceneViewport -> RenderThread -> RDG/RHI -> RHI Present | Window、Viewport、RenderThread、RHI present 分层清楚 | 不做完整 Slate / FSceneViewport / 多窗口 / swapchain 多帧同步 |
| Unity | PlayerLoop -> Camera / RenderPipeline -> CommandBuffer / Native Graphics Present | 用户心智是 GameView/Player Window，底层 graphics backend present | 不复制 Unity native 黑箱，也不把项目脚本直接接 graphics API |
| Bevy | winit Window -> wgpu Surface -> RenderApp / RenderGraph -> RenderDevice / RenderQueue -> present | Rust + winit + wgpu 的工程组织 | 不把 wgpu 暴露为长期唯一抽象 |
| Godot | Window/DisplayServer -> RenderingServer/RenderingDevice -> backend present | Window system 与 rendering device 分层 | 不第一版做完整 RenderingServer 规模 |

## 最终规则

```text
1. RuntimeRenderer 不依赖 winit / surface / swapchain。
2. engine_runtime 可以在 real-wgpu feature 下提供真实 WGPU backend，但默认不依赖窗口库。
3. Window / Surface lifecycle 归 WindowedRuntimeHost。
4. RealWgpuBackend 负责执行 RhiCommandPlan。
5. Surface lost / resize / acquire failed / present failed 必须进入结构化 report。
6. Headless 自动化路径必须继续存在。
7. 真实窗口只做 feature-gated smoke / 用户运行路径。
8. 不新增项目玩法规则。
9. Surface 注入只能发生在 backend/host 边界，不能让 ECS / RuntimeRenderer / Project Logic 看到 wgpu 类型。
```

## 推荐结构

```text
WindowedRuntimeHost
  owns:
    winit Window
    wgpu Surface
    surface config
    resize / surface lost / acquire

RealWgpuBackend
  owns:
    device / queue
    pipeline cache
    resource registry
    command execution

RuntimeRenderer
  outputs:
    RenderGraph
    RhiCommandPlan
```

执行流程：

```text
WindowedRuntimeHost
  -> create window
  -> create/configure wgpu surface
  -> tick EngineHostLoop
  -> RuntimeRenderer builds RhiCommandPlan
  -> RealWgpuBackend::execute_plan_to_surface(surface_target, plan)
  -> submit
  -> surface_texture.present()
  -> WindowedRuntimePresentReport
```

## v1 边界

必须做：

```text
真实 surface acquire / configure / present。
RhiCommandPlan v2 到 surface render pass。
WindowedRuntimePresentReport 能定位 window / surface / rhi / backend / present。
headless path 保持可测。
feature-gated real-window / real-wgpu-surface smoke。
```

暂不做：

```text
多窗口 / 多 surface。
Android / iOS / Web surface。
复杂 swapchain frame pacing。
真实 asset texture / mesh buffer 完整绑定。
高级材质、批处理、instancing。
```

## 为什么适合我们

AI 友好：

```text
AI 通过 WindowedRuntimePresentReport + RhiBackendReport 就能判断失败层级，不需要读 wgpu surface 细节。
```

复杂项目能力：

```text
GameView、Player Window、多 viewport 后续都扩展 WindowedRuntimeHost，不污染 RuntimeRenderer。
```

长期可维护：

```text
以后 D3D12 / Vulkan / Metal 也是 Host 提供 native surface，Backend 执行 RHI，不推翻上层渲染链。
```
