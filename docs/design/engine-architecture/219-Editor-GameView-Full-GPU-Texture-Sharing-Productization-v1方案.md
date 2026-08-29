# 219-Editor GameView Full GPU Texture Sharing Productization v1 方案

采用方案：

```text
方案 A 长期路线：Full GPU Texture Sharing
首轮落地：A-min-shooter-single-view
```

简称：

```text
A-min-gpu-gameview
```

## 1. 这个系统是干什么的

一句话：

```text
让 Editor GameView 真正显示 EditorRuntimePlayInstance 渲染出来的 GPU texture，而不是只显示 descriptor、report 或 placeholder。
```

它解决的是 218 之后最直接的缺口：

```text
217 已完成 Preview RuntimePackage cache。
218 已完成 Editor 进程内 RuntimePlayInstance 和 GameViewRuntimeFrame descriptor evidence。
但 GameView 仍是 texture_descriptor_status=descriptor_only，用户看不到真实 runtime 画面。
219 要把 GameView 从“有运行证据”推进到“有真实 GPU 画面”。
```

在其它引擎中的对标：

```text
Unity：
  GameView 使用 RenderTexture 承接运行画面，再由 Editor UI 画到 GameView。

Unreal：
  PIE viewport / FSceneViewport 承接运行世界的 viewport target，同时处理 viewport 尺寸和输入。

Godot：
  GameView / EmbeddedProcess 更偏运行进程显示消费者；运行会话和显示面板分离，这一点可学，但本轮不采用外部进程嵌入路线。
```

在本引擎主线中的作用：

```text
219 是 218 A-min-gameview 的直接后续。
它不改变 RuntimePackage 真相。
它不让 runtime 扫描项目源目录。
它不把 runtime_player_winit 的独立 EventLoop 嵌入 editor。
它补的是 editor 主窗口 wgpu device / runtime offscreen texture / UI renderer texture sampling 之间的真实 GPU 链路。
```

## 2. 外部源码与官方参考

### 2.1 Unity GameView

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\GameView\GameView.cs
```

关键源码点：

```text
GameView.cs:
  RenderTexture m_RenderTexture
  m_RenderTexture = RenderView(...)
  EditorGUIUtility.DrawTextureHdrSupport(drawRect, m_RenderTexture, ...)
  EditorGUIUtility.QueueGameViewInputEvent(Event.current)
```

可学习点：

```text
GameView 的核心是“运行画面先进入 texture，再由编辑器 UI 绘制 texture”。
GameView 自己负责尺寸、缩放、焦点和输入事件排队，不直接负责构建项目或加载运行包。
```

不可照搬点：

```text
Unity 的 native PlayMode / RenderView 深度藏在 C++ engine/editor 一体化里。
本项目不能把大量隐式状态藏在 editor 内存里，必须保留 RuntimePackage / report 真相。
```

官方参考：

```text
https://docs.unity3d.com/Manual/GameView.html
```

### 2.2 Unreal PIE / SceneViewport

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\GameViewportClient.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Slate\SceneViewport.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\PlayLevel.cpp
```

关键源码点：

```text
UGameViewportClient::CreateGameViewport / GetGameViewport
UGameViewportClient::InputKey
FSceneViewport::OnMouseButtonDown / OnKeyDown
FSceneViewport::SetRHIRef / viewport texture update
StartPlayInEditorSession / GeneratePIEViewportWindow
```

可学习点：

```text
Play session request、runtime world/session、viewport target 是分层的。
Viewport 承接尺寸、焦点、输入和 render target。
运行世界不应该直接理解编辑器面板结构。
```

不可照搬点：

```text
不照搬 UE 的 PIE world copy / GWorld 切换 / 多客户端网络 PIE。
本轮只做单 RuntimePackage、单 RuntimeInstance、单 GameView texture。
```

官方参考：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/play-in-editor-settings-in-unreal-engine
```

### 2.3 Godot EditorRun / GameView

已有源码参考：

```text
框架设计/Godot源码参考/11-EditorRun-GameView-PlaySession源码参考.md
```

关键点：

```text
EditorRunBar / EditorRun 管运行会话。
GameView / EmbeddedProcess 是显示和调试消费者。
运行与显示解耦。
```

可学习点：

```text
GameView 不应承担 Play session 编排。
Stop / Debugger / GameView 显示都应订阅运行会话状态。
```

不可照搬点：

```text
Godot 默认更偏外部进程运行；219 采用的是 218 确定的 in-process GameView 主线。
```

## 3. 本项目源码复查结论

### 3.1 已有可复用基础

```text
rust/crates/engine_runtime/src/runtime_renderer.rs
  RuntimeRenderTargetKind::ViewportTexture
  ViewportTextureDescriptor
  RuntimeRendererOutput.texture_descriptor

rust/crates/editor_ui_renderer/src/draw_list.rs
  DrawCommand::ViewportTextureSlot

rust/crates/editor_window_winit/src/viewport.rs
  RuntimeViewportFrameSummary::from_descriptor
  ViewportHost.latest_runtime_frame

rust/crates/editor_core/src/editor_gameview_play.rs
  EditorRuntimePlayInstance
  GameViewRuntimeFrame
  GameViewPresentReport

rust/crates/engine_runtime/src/wgpu_backend.rs
  real::RealWgpuBackend::from_device_queue(...)
  real::RealWgpuBackend::execute_plan_to_surface_view(...)
  real::RealWgpuBackend::render_plan_to_rgba_bytes(...)
```

说明：

```text
71 已经定义了 RuntimeRenderer -> ViewportTextureDescriptor -> ViewportTextureSlot 的长期底座。
218 已经把 RuntimePackage -> EditorRuntimePlayInstance -> GameViewRuntimeFrame 接上。
现在缺的是“同一个 editor GPU context 下，runtime 写 texture，editor UI 采样 texture”。
```

### 3.2 当前关键问题

#### 问题 A：Editor UI 和 Runtime GPU backend 还没有共享 GPU context

当前：

```text
editor_wgpu_renderer::RealWgpuUiRenderer::new(...)
  自己创建 wgpu Instance / Surface / Adapter / Device / Queue

engine_runtime::wgpu_backend::real::RealWgpuBackend::new_offscreen(...)
  也可以自己创建 Instance / Adapter / Device / Queue

editor_window_winit::windowed_runtime_present::RealWindowedRuntimeSurfaceHost
  为独立 runtime surface host 创建自己的 wgpu surface/device/backend
```

长期路线需要改成：

```text
Editor 主窗口创建一个 EditorSharedGpuContext。
Editor UI renderer 和 EditorRuntimePlayInstance 的 runtime offscreen renderer 共用这个 context。
Runtime 不拥有 editor surface，但可以获得同 device/queue 下的 offscreen texture target。
Editor UI renderer 通过 texture_id 从 registry 取 TextureView / Sampler 并绘制到 GameView rect。
```

#### 问题 B：ViewportTextureSlot 现在仍被画成 placeholder rect

当前：

```text
editor_wgpu_renderer/src/draw_plan.rs
  DrawCommand::ViewportTextureSlot -> UiGpuDrawableRectSource::ViewportPlaceholder

editor_wgpu_renderer/src/real_wgpu.rs
  只有 rect pipeline 和 text pipeline。
  没有 viewport texture sampling pipeline / bind group / registry。
```

长期路线需要：

```text
ViewportTextureSlot 不再只变成 placeholder rect。
它应变成 UiGpuViewportTextureQuad，携带 texture_id / target_id / rect / uv。
RealWgpuUiRenderer 根据 texture_id 绑定 RuntimeSharedTextureRegistry 中的 TextureView。
texture 缺失时才 fallback 到 placeholder，并写 diagnostic。
```

#### 问题 C：RuntimeRenderer 现在能出 descriptor，但没有把真实 texture handle 交给 editor

当前：

```text
RuntimeRenderer::build(...) -> ViewportTextureDescriptor
RealWgpuBackend::execute_plan(...) 可以创建 offscreen texture，但 texture 是 backend 内部临时对象。
```

长期路线需要：

```text
Editor 分配或复用 GameView offscreen texture。
Runtime backend 直接 render_to_view 到该 texture view。
RuntimeRenderer / GameViewPresentReport 输出 texture_id 和 GPU present report。
Editor UI renderer 后续同帧或下一帧 sample 同一 texture_id。
```

#### 问题 D：不能退回 runtime_player_winit 独立 EventLoop

36/218 已确认：

```text
runtime_player_winit::run_windowed_native_player_from_package 会创建独立 EventLoop / OS window / surface。
它不能作为 Editor 内嵌 GameView 的实现。
```

219 继续遵守：

```text
不复用 runtime_player_winit 的 EventLoop。
只使用 editor 主窗口的 wgpu context 和 redraw 生命周期。
```

## 4. 重新审视后的方案选择

### 方案 A：Full GPU Texture Sharing

内容：

```text
Editor 主窗口持有 shared wgpu context。
Runtime in-process runner 在同 context 内渲染到 offscreen texture。
Editor UI renderer 直接 sample 这个 texture 到 GameView rect。
```

优点：

```text
长期路线正确，接近 Unity GameView 的 RenderTexture 模式。
不把 placeholder / screenshot / CPU image copy 做成主线。
后续接 GameView input、Pause/Step、screenshot、capture frame 都在同一条真实渲染链路上。
复杂打飞机项目可以真正看到 runtime 画面和 HUD。
```

缺点：

```text
实现范围较大，涉及 editor_wgpu_renderer、engine_runtime real-wgpu backend、editor_window_winit、editor_core。
默认 CI 不能强依赖真实 GPU / OS window，需要保留 headless deterministic gate 和 local-only real GPU smoke。
需要严控 crate 依赖，不能让 editor_core 直接依赖 editor_wgpu_renderer 或 wgpu surface。
```

结论：

```text
采用为长期路线。
首轮只做 A-min-shooter-single-view。
```

### 方案 B：CPU readback / image evidence

内容：

```text
Runtime 渲染到 offscreen texture 后 readback 成 RGBA bytes。
Editor UI 把 image bytes 上传为 UI texture 或只做 report。
```

结论：

```text
不作为主线。
它适合 screenshot / verification，不适合实时 GameView。
```

### 方案 C：Descriptor-only / placeholder C-min

内容：

```text
继续只让 ViewportTextureSlot 携带 texture_id / target_id，UI 仍画 placeholder。
```

结论：

```text
不采用。
218 已经完成 descriptor_only，再做 C-min 会原地踏步。
```

## 5. 正式采用：A-min-shooter-single-view

### 5.1 本轮目标

```text
在 Editor 主窗口真实 wgpu context 内：
  Runtime 渲染到 GameView offscreen texture。
  Editor UI renderer sample 这个 texture。
  GameViewPresentReport 报告 real_gpu_texture_present=presented。
  complex shooter e2e / local real-window smoke 能证明真实 GameView texture present 链路存在。
```

### 5.2 本轮只做的范围

```text
单 Editor window。
单 GameView。
单 EditorRuntimePlayInstance。
单 GameView offscreen texture target。
同进程 shared wgpu Device / Queue。
RuntimeRenderer 输出到 RuntimeRenderTargetKind::ViewportTexture。
Editor UI renderer 通过 ViewportTextureSlot sample texture。
Texture resize 时重建 offscreen texture。
Stop 时释放/注销 GameView runtime texture。
```

### 5.3 本轮不做

```text
GameView input / focus / AUI interaction。
Pause / Resume / Step。
Maximize on Play。
多 GameView / 多 RuntimeInstance。
跨进程 texture sharing。
独立 runtime_player_winit EventLoop。
完整 render thread / fence / frame-lag 调度重构。
完整 sprite material / texture sampling 质量升级。
真实 GPU gate 作为默认 CI 必过项。
```

## 6. 目标架构

### 6.1 新增 EditorSharedGpuContext

归属建议：

```text
editor_wgpu_renderer
```

结构：

```text
EditorSharedGpuContext
  backend_name
  device
  queue
  surface_format
  limits_summary
```

规则：

```text
RealWgpuUiRenderer 创建并持有 EditorSharedGpuContext。
Editor runtime texture present 只能使用该 context。
engine_runtime 不拥有 editor surface。
editor_core 不直接持有 wgpu::Device / wgpu::Queue。
```

### 6.2 新增 EditorViewportTextureRegistry

归属建议：

```text
editor_wgpu_renderer
```

结构：

```text
EditorViewportTextureRegistry
  textures: texture_id -> EditorViewportTextureEntry

EditorViewportTextureEntry
  texture_id
  target_id
  width
  height
  format
  frame_index
  texture
  view
  sampler
  producer
  last_present_status
```

规则：

```text
texture_id 是 editor/runtime 边界的稳定引用。
UI draw list 只携带 texture_id，不携带 wgpu handle。
RealWgpuUiRenderer 在 present 时从 registry 解析 texture_id。
缺 texture 时 fallback placeholder，并写 report。
```

### 6.3 Runtime 侧新增 SharedViewportTextureTarget

归属建议：

```text
engine_runtime::wgpu_backend::real 或 engine_runtime::runtime_renderer feature boundary
```

结构：

```text
SharedViewportTextureTarget
  target_id
  texture_id
  width
  height
  format
  texture_view
```

规则：

```text
Runtime backend 只拿可写 render target view。
Runtime backend 不知道 editor panel、GameView rect、dock layout。
Runtime backend 写完后返回 RhiBackendReport / GpuTextureLifetimeReport / ViewportTextureDescriptor。
```

### 6.4 UI Renderer 新增 ViewportTexture pipeline

当前：

```text
Rect pipeline
Text pipeline
ViewportTextureSlot -> placeholder rect
```

目标：

```text
Rect pipeline
Text pipeline
ViewportTexture pipeline

DrawCommand::ViewportTextureSlot
  -> UiGpuViewportTextureQuad
  -> bind texture_id
  -> draw textured quad
```

### 6.5 219 报告

新增报告：

```text
EditorGameViewGpuTexturePresentReport
  schema_version
  session_id
  status:
    presented
    fallback_placeholder
    gpu_unavailable
    failed
  backend_kind
  shared_gpu_context_status
  target_id
  texture_id
  width
  height
  format
  frame_index
  runtime_render_report_path?
  game_view_present_report_path?
  runtime_target_hash
  ui_sample_status
  texture_registry_status
  lifecycle_event_count
  diagnostics
  next_actions
  deferred_flags
```

## 7. 正确链路

```text
Editor window resumed
  -> RealWgpuUiRenderer::new
  -> EditorSharedGpuContext
  -> EditorViewportTextureRegistry

Toolbar Play
  -> 217 Preview RuntimePackage
  -> 218 EditorRuntimePlayInstance
  -> request GameView shared texture target
  -> RuntimeRenderer builds RhiCommandPlan
  -> RealWgpuBackend renders into shared texture view
  -> GameViewRuntimeFrame texture_descriptor_status=real_gpu_texture
  -> ViewportModel carries texture_id / target_id
  -> EditorUiRenderer DrawCommand::ViewportTextureSlot
  -> RealWgpuUiRenderer samples texture_id from registry
  -> Editor surface present
  -> EditorGameViewGpuTexturePresentReport status=presented
```

## 8. 与 218 的关系

218 已完成：

```text
EditorRuntimePlayInstance
EditorGameViewPlayRunner
GameViewRuntimeFrame
GameViewPresentReport
descriptor_only report
```

219 修改：

```text
GameViewPresentReport.texture_descriptor_status:
  descriptor_only -> real_gpu_texture 或 fallback_placeholder

GameViewPresentReport.game_view_output_kind:
  viewport_texture_descriptor -> shared_gpu_viewport_texture

deferred_flags:
  real_gpu_texture_present_deferred 从 true 变为 false
```

219 不修改：

```text
RuntimePackage 真相。
EditorRuntimePlayInstance 只从 RuntimePackage load。
GameView input 仍 deferred。
runtime_player_winit 仍不进入 editor_core。
```

## 9. 测试策略

默认自动化必须稳定：

```text
cargo fmt --check
cargo test -p editor_wgpu_renderer viewport_texture
cargo test -p engine_runtime wgpu_backend
cargo test -p editor_window_winit viewport_runtime
cargo test -p editor_core editor_gameview_play
cargo test -p project_e2e_gate editor_gameview_gpu_texture_present
```

默认 CI 不强依赖真实 OS window / GPU：

```text
headless tests 验证：
  schema
  texture registry contract
  ViewportTextureSlot resolution
  missing texture fallback diagnostic
  GameViewPresentReport 状态转换
  complex shooter report 中 real_gpu path 的 feature/environment status
```

真实 GPU / OS window smoke 作为 local-only / ignored：

```powershell
cargo test -p editor_window_winit gameview_real_gpu_texture_present --features real-window,real-wgpu -- --ignored
```

local-only smoke 必须证明：

```text
RealWgpuUiRenderer 创建 shared context。
Runtime backend 使用同 context 渲染到 offscreen texture。
UI renderer sample texture 到 GameView rect。
present report status=presented。
screenshot 或 pixel hash 非空/非 placeholder。
```

## 10. 风险与处理

### 风险 A：wgpu device / queue 共享边界扩大

处理：

```text
只在 editor_window_winit / editor_wgpu_renderer / engine_runtime real-wgpu feature 边界共享。
editor_core 只保存 report 和 texture_id，不保存 wgpu handle。
```

### 风险 B：真实 GPU gate 不稳定

处理：

```text
默认 gate 不要求真实 OS window。
真实 GPU smoke 作为 ignored/local-only。
但代码路径必须真实存在，不能只写 schema。
```

### 风险 C：一次把 input / pause / screenshot 都塞进来

处理：

```text
本轮只做 texture present。
输入进入 220。
Pause/Step 进入后续 Play control 系统。
```

### 风险 D：RuntimeRenderer 与 Editor UI Renderer 互相依赖

处理：

```text
通过 texture_id / shared gpu context / registry 边界通信。
Runtime 不读 DrawCommand。
Editor UI 不读 RenderGraph。
```

## 11. 验收标准

必须满足：

```text
EditorRuntimePlayInstance 能为 GameView 请求 shared GPU viewport texture。
Runtime RHI backend 能渲染到 editor 分配的 offscreen texture view。
Editor UI renderer 能通过 ViewportTextureSlot 采样该 texture。
GameViewPresentReport 不再固定 descriptor_only。
219 report 能区分 presented / fallback_placeholder / gpu_unavailable / failed。
Stop 会释放或注销 runtime GameView texture。
complex shooter gate 能报告 219 链路状态。
```

禁止冒充：

```text
不能把 placeholder rect 当作 real_gpu_texture_present。
不能把 CPU screenshot/readback 当作主线 present。
不能创建第二个 editor EventLoop。
不能让 runtime_player_winit 独立 window 冒充 Editor GameView。
不能让 editor_core 保存 wgpu handle。
不能跳过缺 texture diagnostic。
```

## 12. 为什么必须是同进程 shared GPU

这里的“同进程 shared wgpu Device / Queue”不是说把 runtime 逻辑写进 editor_core，也不是让 runtime 变成 editor 的私有模块。

它的准确含义是：

```text
Editor 主窗口创建真实 wgpu Device / Queue / Surface。
Editor UI renderer 使用这个 Device / Queue 画编辑器界面。
EditorRuntimePlayInstance 在 editor 进程内 tick runtime。
Runtime real-wgpu backend 使用同一个 Device / Queue 渲染到 GameView offscreen texture。
Editor UI renderer 再用同一个 Device sample 这个 texture 到 GameView rect。
```

必须同进程的原因：

```text
wgpu::Texture / TextureView / Sampler / BindGroup 是创建它的 Device 下的 GPU 资源。
同一个进程、同一个 Device 下，runtime 写入的 texture 可以被 editor UI renderer 直接采样。
如果跨进程，就不能传 Rust handle，也不能简单传 texture_id；必须走 D3D12 shared handle、Vulkan external memory、Metal IOSurface 或平台专用 IPC/sync。
跨进程 GPU sharing 会引入平台分支、同步 fence、生命周期和安全边界，不适合 219 A-min。
CPU readback 虽然跨边界容易，但会变成 screenshot/image-copy 路线，不是实时 GameView 主线。
```

边界规则：

```text
同进程只发生在 Editor Play。
Export / Standalone runtime 仍然自己创建 Device / Queue。
editor_core 不持有 wgpu handle。
engine_runtime 不知道 editor dock、panel、GameView rect。
共享 GPU context 只暴露在 editor_wgpu_renderer / editor_window_winit / engine_runtime real-wgpu feature 边界。
```

## 13. Crate 边界与依赖规则

### 13.1 editor_wgpu_renderer

职责：

```text
创建并持有 EditorSharedGpuContext。
创建并维护 EditorViewportTextureRegistry。
创建 GameView offscreen texture。
提供 ViewportTexture pipeline，把 ViewportTextureSlot 采样到 editor surface。
输出 RealUiPresentReport / EditorGameViewGpuTexturePresentReport 所需的 GPU present evidence。
```

禁止：

```text
不加载 RuntimePackage。
不 tick runtime world。
不理解项目玩法。
不读取 editor_core 的内部 session object。
```

### 13.2 engine_runtime

职责：

```text
继续负责 RuntimeRenderer、RenderGraph、RhiCommandPlan、real-wgpu backend。
real-wgpu feature 增加“渲染到外部提供的 viewport texture view”的能力。
返回 RhiBackendReport / texture lifetime report / descriptor evidence。
```

禁止：

```text
不依赖 editor_wgpu_renderer。
不保存 editor window / surface / dock layout。
不把 editor-only texture registry 变成 runtime core 概念。
```

### 13.3 editor_window_winit

职责：

```text
真实 OS window 生命周期 owner。
在 resumed 时创建 RealWgpuUiRenderer 和 EditorSharedGpuContext。
在 Play Running 时把 GameView size、runtime frame、shared texture target 串起来。
在 RedrawRequested 中按顺序执行 runtime texture update 和 UI present。
处理 window resize 时的 GameView texture resize/recreate。
Stop 时注销 texture lease。
```

禁止：

```text
不让 runtime_player_winit::run_windowed_native_player_from_package 进入 editor EventLoop。
不创建第二个 runtime EventLoop 冒充 GameView。
```

### 13.4 editor_core

职责：

```text
继续保存 EditorRuntimePlayInstance、GameViewRuntimeFrame、GameViewPresentReport。
只记录 texture_id、target_id、frame_index、status、report path、diagnostics。
提供 Play/Stop/session/report 的纯数据状态。
```

禁止：

```text
不依赖 wgpu。
不保存 Texture / TextureView / Sampler / BindGroup。
不直接调用 RealWgpuUiRenderer。
```

### 13.5 editor_ui_renderer

职责：

```text
继续输出抽象 UiDrawList / DrawCommand::ViewportTextureSlot。
ViewportTextureSlot 携带 rect、scene_id、texture_id、target_id、frame_index。
```

禁止：

```text
不保存 GPU resource。
不判断 texture 是否存在。
是否能 sample texture 由 editor_wgpu_renderer registry 决定。
```

## 14. A-min 数据结构细化

### 14.1 EditorSharedGpuContext

建议归属：

```text
editor_wgpu_renderer
```

结构：

```text
EditorSharedGpuContext
  backend_name
  adapter_info
  surface_format
  device
  queue
  limits_summary
  feature_summary
```

实现约束：

```text
当前 RealWgpuUiRenderer 直接按值持有 wgpu::Device / wgpu::Queue。
219 Gate A 必须先把它收敛为 shared context wrapper。
如果 wgpu API 所需 ownership 不适合直接 clone，就用内部 wrapper/Arc 管理，外部只暴露 device_ref()/queue_ref()。
from_device_queue 当前按值接收 device/queue，219 需要新增 shared/borrrowed variant，避免把 editor 的 Device 移走。
```

### 14.2 EditorViewportTextureRegistry

建议归属：

```text
editor_wgpu_renderer
```

结构：

```text
EditorViewportTextureRegistry
  entries: texture_id -> EditorViewportTextureEntry
  generation
  lifecycle_events

EditorViewportTextureEntry
  texture_id
  target_id
  owner_session_id
  width
  height
  format
  color_space
  generation
  last_frame_index
  texture
  view
  sampler
  producer
  present_status
```

操作：

```text
allocate_or_resize_gameview_target(session_id, target_id, width, height, format) -> EditorViewportTextureLease
resolve(texture_id) -> TextureView/Sampler
mark_rendered(texture_id, frame_index, rhi_report)
unregister_session(session_id)
fallback_reason(texture_id)
```

规则：

```text
texture_id 是 editor_core / draw_list / report 可见的稳定 id。
Texture / TextureView / Sampler 只存在于 registry 内部。
resize 时 texture_id 可以稳定，generation 必须递增。
Stop 时按 session_id 释放或注销，避免旧 texture 被新 session 误采样。
```

### 14.3 RuntimeSharedViewportTextureTarget

建议归属：

```text
engine_runtime real-wgpu feature boundary
```

结构：

```text
RuntimeSharedViewportTextureTarget
  texture_id
  target_id
  width
  height
  format
  frame_index
  texture_view_ref
```

规则：

```text
它是一次 render 调用的临时 target，不是可序列化资产。
engine_runtime 可以拿 texture_view_ref 执行 execute_plan_to_texture_view / execute_plan_to_surface_view 等价方法。
engine_runtime 返回 report 和 descriptor，不保留 editor registry handle。
```

### 14.4 UiGpuViewportTextureQuad

建议归属：

```text
editor_wgpu_renderer::draw_plan
```

结构：

```text
UiGpuViewportTextureQuad
  rect
  uv
  texture_id
  target_id
  frame_index
  fallback_if_missing
```

变化：

```text
UiGpuDrawPlan 增加 viewport_texture_quads。
DrawCommand::ViewportTextureSlot 不再只 push ViewportPlaceholder rect。
缺 texture 时才追加 placeholder rect，并记录 diagnostic/fallback count。
```

### 14.5 EditorGameViewGpuTexturePresentReport

建议归属：

```text
editor_wgpu_renderer 或 editor_window_winit report module。
editor_core 的 GameViewPresentReport 只保存其路径和摘要字段。
```

字段：

```text
schema_version
session_id
status: presented | fallback_placeholder | gpu_unavailable | failed
backend_kind
shared_gpu_context_status
target_id
texture_id
generation
width
height
format
frame_index
runtime_rhi_status
ui_sample_status
texture_registry_status
pixel_evidence_status
diagnostics
next_actions
```

## 15. A-min 执行链路细化

### 15.1 Play start

```text
Toolbar Play
  -> 217 prepare Preview RuntimePackage
  -> 218 create EditorRuntimePlayInstance
  -> editor_window_winit detects Running GameView session
  -> RealWgpuUiRenderer asks registry allocate_or_resize_gameview_target
  -> registry returns texture_id / target_id / texture view lease
```

### 15.2 Runtime frame

```text
EditorRuntimePlayInstance tick
  -> EngineHostLoop outputs RuntimeRendererOutput
  -> RuntimeRendererOutput contains RhiCommandPlan + ViewportTextureDescriptor
  -> engine_runtime real-wgpu backend executes RhiCommandPlan into shared texture view
  -> RhiBackendReport marks presented=true for target_id
  -> registry mark_rendered(texture_id, frame_index)
```

说明：

```text
218 现在 tick_descriptor_frame 只读 descriptor。
219 不要求把 editor_core 变成 GPU owner。
可以新增 editor-window 侧的 gpu present step：拿 editor_core last frame 的 RhiCommandPlan/descriptor 或新增可取的 render frame output，再由 window 层执行 GPU present。
如果当前 EditorRuntimePlayInstance 没有保留 RhiCommandPlan，Gate C 必须先补“last render output summary/plan access”，但不能把 wgpu handle 塞进 editor_core。
```

### 15.3 UI present

```text
ViewportModel carries texture_id / target_id
  -> editor_ui_renderer emits DrawCommand::ViewportTextureSlot
  -> editor_wgpu_renderer builds UiGpuViewportTextureQuad
  -> RealWgpuUiRenderer resolves texture_id in registry
  -> bind texture view + sampler
  -> draw quad into GameView rect on editor surface
```

### 15.4 Resize

```text
GameView rect size changed
  -> registry allocate_or_resize_gameview_target
  -> generation += 1
  -> old texture released after no longer in use
  -> next runtime frame renders into new texture
  -> report records resize event
```

### 15.5 Stop

```text
Stop Play
  -> editor_core drops EditorRuntimePlayInstance
  -> editor_window_winit unregister_session(session_id)
  -> registry removes GameView texture entry
  -> GameViewPresentReport stop_status=stopped
```

## 16. 分 Gate 施工建议

### 16.0 吸收 37 号审查后的施工前修订

`其它AI审查目录/37-219-Editor-GameView-Full-GPU-Texture-Sharing方案审查.md` 判断 219 方向正确，可以进入施工，但施工文档必须吸收以下约束：

```text
1. A-min 同步执行：
   runtime tick -> write shared texture -> editor UI sample -> editor surface present
   全部在 editor 主窗口 RedrawRequested / 单帧更新顺序内执行。
   本轮不启用 RenderThreadWorker 独立线程跨 crate 写读同一 texture。

2. EditorSharedGpuContext 使用共享所有权：
   RealWgpuUiRenderer 内部持有可 clone 的 shared context wrapper。
   editor_window_winit 通过 shared_context() accessor 取得只读共享引用。
   engine_runtime real-wgpu backend 通过 shared/borrowed device queue variant 接入，不能把 editor Device/Queue 移走。

3. headless/mock 行为必须明确：
   默认 CI 不要求真实 wgpu window。
   无真实 GPU / 无 real-window 时，context summary/report 可返回 gpu_unavailable 或 headless mock summary。
   registry、draw_plan、fallback、report 状态仍必须可 deterministic test。

4. Runtime 渲染能力复用已有基础：
   RealWgpuBackend 已有 execute_plan_to_surface_view(plan, &TextureView)。
   219 不重造 RHI 渲染路径，只补 shared device/queue 接入和 editor 分配的 offscreen texture target。

5. RhiCommandPlan / render output 访问：
   如果 EditorRuntimePlayInstance 当前只保留 descriptor evidence，施工时必须补 last render output / RhiCommandPlan 的可访问边界。
   该边界只能暴露可序列化/可克隆的 plan、descriptor、summary，不能把 wgpu handle 放入 editor_core。

6. resize 只在帧间执行：
   A-min 没有异步 in-flight texture。
   GameView size 变化时，在下一帧开始前 allocate_or_resize，旧 texture 可立即释放或通过 generation 失效。
```

这些修订不改变正式采用方案：

```text
仍采用方案 A：Full GPU Texture Sharing。
仍只做 A-min-shooter-single-view。
仍不做 input、Pause/Step、多 GameView、跨进程 texture sharing。
```

### Gate A：shared GPU context contract

目标：

```text
RealWgpuUiRenderer 使用 EditorSharedGpuContext。
保留现有 rect/text present 行为。
新增 headless/mockable context summary report。
```

测试：

```powershell
cd rust
cargo test -p editor_wgpu_renderer shared_gpu_context
cargo test -p editor_wgpu_renderer real_ui_present
```

### Gate B：viewport texture registry

目标：

```text
新增 EditorViewportTextureRegistry。
支持 allocate / resize / resolve / unregister。
缺 texture 能给出 fallback diagnostic。
```

测试：

```powershell
cd rust
cargo test -p editor_wgpu_renderer viewport_texture_registry
```

### Gate C：UI texture sampling pipeline

目标：

```text
UiGpuDrawPlan 增加 UiGpuViewportTextureQuad。
RealWgpuUiRenderer 增加 viewport texture pipeline / bind group layout。
ViewportTextureSlot 有真实 texture 时 sample，没有 texture 时 fallback placeholder。
```

测试：

```powershell
cd rust
cargo test -p editor_wgpu_renderer viewport_texture_pipeline
cargo test -p editor_wgpu_renderer viewport_texture_fallback
```

### Gate D：runtime renders into shared texture target

目标：

```text
engine_runtime real-wgpu backend 支持 borrowed/shared target view render。
不再要求 runtime backend 独占 Device / Queue。
RhiBackendReport 能证明 target_id / texture_id / frame_index 已渲染。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime wgpu_backend
cargo test -p engine_runtime runtime_renderer_viewport_texture
```

### Gate E：editor window wiring

目标：

```text
editor_window_winit 把 RealWgpuUiRenderer、registry、EditorRuntimePlayInstance frame 串起来。
RedrawRequested 中完成 runtime texture update + UI sample present。
resize / stop 生命周期能写 report。
```

测试：

```powershell
cd rust
cargo test -p editor_window_winit viewport_runtime
cargo test -p editor_window_winit gameview_gpu_texture_present_contract
```

### Gate F：editor_core report 升级

目标：

```text
GameViewPresentReport 增加 real_gpu_texture summary 字段。
texture_descriptor_status 可从 descriptor_only 变为 real_gpu_texture 或 fallback_placeholder。
deferred_flags 移除 real_gpu_texture_present_deferred。
Report Panel 能显示 219 GPU present evidence 路径。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_gameview_play
cargo test -p editor_core report_panel
```

### Gate G：complex shooter e2e evidence

目标：

```text
project_e2e_gate 生成 219 report。
默认环境无 GPU 时允许报告 gpu_unavailable，但不能假装 presented。
本地 real GPU smoke 通过时必须 status=presented。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate editor_gameview_gpu_texture_present
```

### Gate H：local-only real GPU smoke

目标：

```text
真实 OS window + real-wgpu feature 下验证 GameView texture 非空、非 placeholder。
可用 screenshot/pixel hash 证明采样路径有效。
```

测试：

```powershell
cd rust
cargo test -p editor_window_winit gameview_real_gpu_texture_present --features real-window,real-wgpu -- --ignored
```

## 17. 测试矩阵与验收口径

默认必过：

```text
schema/report/registry/draw_plan/fallback/Play report/e2e status。
这些测试不要求真实 OS window，不要求机器一定有可用 GPU。
```

本地真实 GPU 必测：

```text
开发机施工结束前必须跑 ignored real GPU smoke。
如果本机 GPU/driver 不可用，必须在报告中写明 gpu_unavailable，并保留可复现命令。
```

验收时必须区分：

```text
presented:
  runtime RHI 写入 shared texture，UI renderer 成功 sample，pixel/hash evidence 非 placeholder。

fallback_placeholder:
  ViewportTextureSlot 存在，但 texture registry 缺失或 sample 失败，UI 画 placeholder 并有 diagnostic。

gpu_unavailable:
  真实 wgpu context 创建失败或当前测试环境不支持 real-window/real-wgpu。

failed:
  RuntimePackage / runtime tick / RHI plan / registry lifecycle 出错。
```

禁止验收：

```text
descriptor_only 被当作 presented。
placeholder rect 被当作真实画面。
CPU readback image 被当作主线 GameView present。
local-only smoke 失败但 report 写 presented。
```

## 18. 自审

```text
是否符合 Unity-like in-editor Play 主线：
  符合。它把 218 的 EditorRuntimePlayInstance 推进到真实 GameView texture present。

是否变成临时方案：
  不是。A-min 是 Full GPU Texture Sharing 的第一块，不是截图/descriptor 替代品。

是否扩大到输入和 Pause：
  没有。219 只做画面 present；GameView Input / Focus / AUI Interaction 留给 220。

是否让 editor_core 变重：
  没有。editor_core 仍只保存数据报告和 texture id，不保存 GPU resource。

是否适合复杂打飞机：
  适合。复杂打飞机最先需要在 Editor Play 中看到真实 runtime 画面和 HUD，219 正好补这个缺口。
```

## 19. 结论

```text
219 应采用方案 A：Full GPU Texture Sharing。
但首轮只做 A-min-shooter-single-view。
这不是临时截图路线，也不是 descriptor C-min，而是长期 GPU texture sharing 架构的第一块真实落地。
做完后，复杂打飞机项目从“GameView 有运行证据”推进到“GameView 能看到真实 runtime 画面/HUD”。
下一轮再讨论 220：Editor GameView Input / Focus / AUI Interaction Bridge v1。
```
