# 82-真实 OS RenderThread Worker / Fence / FrameLag 方案

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

本文档定义真实 OS RenderThread Worker / Fence / FrameLag 的长期架构规则。

它是 `81-真实跨线程RenderThreadQueue-RenderSubmissionPipeline-v1方案.md` 的下一层。

`81` 已经完成：

```text
RenderFramePacket
RenderThreadQueue
RenderSubmissionTicket
RenderSubmissionReport
EngineHostLoop -> submit_frame_output
DedicatedThread C-min diagnostic
```

但 `81` 第一版没有做：

```text
真实 OS RenderThread worker。
真实跨线程阻塞 / 唤醒 / 退出。
Frame lag 控制。
Flush / Shutdown fence。
RenderThread worker lost / timeout 报告。
```

本文档解决的问题是：

```text
RenderThread 如何真的运行在独立 OS thread。
Game Thread 如何提交 render command 而不是直接执行。
Game 可以领先 Render 多少帧。
什么时候必须等 fence。
Shutdown 如何安全释放 RenderResourceManager / GPU resource。
AI 如何区分 queue 堆积、frame lag、fence 等待、worker 崩溃。
```

## 核心判断

本项目不采用“普通 bounded queue 满了就阻塞”的简单方案作为长期规则。

原因：

```text
queue 满了是性能 / 调度问题。
resource release safe 是同步问题。
shutdown 是生命周期问题。
frame lag 是帧节奏问题。

这些问题不能全部混成一个 backpressure 概念。
否则后期所有卡顿都会被报告成 queue full，AI 和人都难以定位。
```

长期路线采用：

```text
UE-like RenderCommand Dispatcher
  + RenderThread Worker
  + RenderFence
  + FrameLagController
```

## 参考引擎

### Unreal Engine

源码参考：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/RenderCore/Private/RenderingThread.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/RenderCore/Public/RenderingThread.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/RenderCore/Public/RenderCommandFence.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/RenderCore/Private/RenderResource.cpp
```

UE 的关键规则：

```text
StartRenderingThread()
  先 FlushRenderingCommands()
  再打开 GIsThreadedRendering
  创建 FRenderingThread
  用 FRenderCommandFence 等待 RenderThread 真正启动并空闲

RenderingThreadMain()
  把 RenderThread 绑定到 TaskGraph
  调 ProcessThreadUntilRequestReturn()
  RenderThread 不是普通 while recv queue，而是 TaskGraph worker loop

ENQUEUE_RENDER_COMMAND
  如果启用 threaded rendering，就派发到 RenderThread
  如果当前就在 RenderThread 或未启用 threaded rendering，则直接执行
  可通过 RenderCommandPipe / FRenderCommandList 做 recording / submit

FRenderCommandFence
  BeginFence 插入一个 render command
  command 执行时触发 event
  Wait 检查 RenderThread health / timeout
  支持 RenderThread / RHIThread / Swapchain 不同同步深度

StopRenderingThread()
  SuspendTextureStreamingRenderTasks()
  FlushRenderingCommands()
  停 RHI thread
  设置 GIsThreadedRendering=false
  发 ReturnGraphTask 让 RenderThread 退出
  等待线程结束
```

可借鉴：

```text
RenderThread 是真实 worker。
提交入口是 RenderCommand dispatcher，不是裸队列。
Fence 是同步真相。
Flush / Shutdown 是显式同步点。
Frame lag / frame pacing 不能和普通 queue full 混在一起。
RenderThread health / timeout 必须能报告。
```

不照搬：

```text
第一版不实现完整 UE TaskGraph。
第一版不实现完整 FRenderCommandPipe / FRenderCommandList recording。
第一版不实现 RHIThread。
第一版不实现复杂 heartbeat / crash reporter。
```

### Unity

源码参考：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Runtime/Export/RenderPipeline/ScriptableRenderContext.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Runtime/Export/Graphics/RenderingCommandBuffer.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Runtime/Export/Graphics/GraphicsFence.bindings.cs
```

Unity 的关键规则：

```text
用户主要理解 Update / LateUpdate / Render。
SRP 使用 ScriptableRenderContext.ExecuteCommandBuffer / Submit。
GraphicsFence 用于 CPU / GPU / async queue 同步。
底层 threaded renderer 细节对普通用户隐藏。
```

可借鉴：

```text
用户心智保持简单。
Fence 是可理解的同步抽象。
项目侧不直接接触 RenderThread worker。
```

不照搬：

```text
不把底层提交完全黑盒化。
本项目必须输出 AI 可读 RenderSubmissionReport / RenderWorkerReport。
```

### Bevy

源码参考：

```text
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_render/src/pipelined_rendering.rs
```

Bevy 的关键规则：

```text
PipelinedRenderingPlugin 把 rendering 放到另一个线程。
第 N 帧 rendering 可以和第 N+1 帧 simulation 并行。
Main App / Render App 通过 channel 交还 SubApp。
sync / extract 在主线程完成，render schedule 在 render thread 执行。
Drop 时等待 render app 回到主线程，保证非 Send 数据在正确线程释放。
```

可借鉴：

```text
Game / Render 并行一帧。
render worker 生命周期必须安全收口。
Drop / shutdown 不能随意丢弃 render-side 状态。
```

不照搬：

```text
不引入完整 Bevy RenderApp / RenderWorld。
本项目继续使用 RenderFramePacket / RenderThread / RenderSceneState。
```

## 方案对比

### 方案 A：继续 InlineDeterministic

```text
EngineHostLoop 直接同步调用 RenderThread。
```

优点：

```text
最简单。
测试确定性强。
```

缺点：

```text
不是长期真实架构。
无法验证 Game / Render 并行。
GPU resource safe release / present fence 仍是模拟。
后期会被 inline 架构拖住。
```

结论：

```text
只能保留为 headless / CI / deterministic test mode。
不能作为长期 runtime 路线。
```

### 方案 B：普通 bounded queue + backpressure

```text
Game Thread submit packet。
queue 满就 block 或 fail。
RenderThread 从 queue 取 packet。
```

优点：

```text
实现简单。
容易写测试。
```

缺点：

```text
不像 UE。
把 queue full、frame lag、fence wait、resource sync、shutdown 混成一个 backpressure。
AI 报告会变粗。
后期复杂项目遇到卡顿时不好查。
```

结论：

```text
不作为正式方案。
```

### 方案 C：triple buffer / drop old frame

```text
Game 永不阻塞。
Render 落后时丢旧帧或合并帧。
```

优点：

```text
编辑器交互可能更流畅。
```

缺点：

```text
游戏 runtime 语义复杂。
frame report 顺序难解释。
资源释放和 present fence 更难维护。
不适合作为第一版正式 runtime 默认规则。
```

结论：

```text
后续可作为 Editor viewport preview 的特殊策略。
不作为 runtime 默认规则。
```

### 方案 D：UE-like RenderCommand Dispatcher + Fence + FrameLagController

```text
Game Thread 通过 dispatcher 提交 render command。
RenderThread worker 在独立 OS thread 执行 command。
FrameLagController 控制 Game 领先 Render 的最大帧数。
Flush / Shutdown 通过 RenderFence。
```

优点：

```text
最接近 UE 的长期路线。
同步问题、调度问题、生命周期问题分层清楚。
AI report 可以明确区分 queue、frame lag、fence、worker lost。
后续可扩展 TaskGraph-light / RHI thread / native backend。
```

缺点：

```text
比普通 queue 复杂。
需要设计 worker 生命周期和 fence timeout。
```

结论：

```text
采用方案 D。
```

## 最终架构

```text
Runtime / Game Thread
  -> ECS Update / LateUpdate
  -> RenderExtract
  -> RenderFramePacket
  -> RenderCommandDispatcher.submit_frame(packet)
  -> RenderSubmissionTicket

Frame Boundary
  -> FrameLagController.check()
  -> 如果 Game 领先 Render 超过 max_frames_in_flight
  -> wait RenderFence

RenderThread Worker
  -> OS thread loop
  -> receive RenderThreadCommand
  -> SubmitFrame(packet)
  -> RenderThread.execute_packet
  -> RDG / RHI / Backend submit / present
  -> RenderSubmissionReport
  -> completed_frame_index update
  -> signal fence if needed

Shutdown
  -> RenderCommandDispatcher.enqueue_shutdown()
  -> RenderThread worker drain pending commands
  -> RenderResourceManager safe release
  -> signal shutdown fence
  -> join OS thread
```

## 标准结构

### RenderThreadWorker

```text
RenderThreadWorker
  worker_id
  config
  state
  command_sender
  report_receiver
  thread_handle
```

职责：

```text
创建真实 OS render thread。
持有 RenderThread 实例。
执行 RenderThreadCommand。
维护 worker health。
输出 RenderWorkerReport。
```

### RenderCommandDispatcher

```text
RenderCommandDispatcher
  mode
  submit_frame(packet) -> RenderSubmissionTicket
  insert_fence(sync_depth) -> RenderFence
  flush(sync_depth) -> RenderFenceResult
  shutdown() -> RenderFenceResult
  poll_report(ticket) -> Option<RenderSubmissionReport>
```

职责：

```text
Game Thread 唯一提交入口。
隐藏 InlineDeterministic / DedicatedWorker 的差异。
不暴露 RenderThread 内部状态。
```

### RenderThreadCommand

```text
RenderThreadCommand
  SubmitFrame(RenderFramePacket, RenderSubmissionTicket)
  InsertFence(RenderFence)
  Shutdown(RenderFence)
```

第一版只保留三类。

不允许第一版扩张为万能 command bus。

### RenderFence

```text
RenderFence
  fence_id
  sync_depth
  created_frame_index
  target_frame_index optional
  status
```

```text
RenderFenceSyncDepth
  RenderThread
  RhiSubmit
  Present
```

第一版规则：

```text
RenderThread depth 必须实现。
RhiSubmit / Present 可以先通过 headless backend 模拟。
Native RHI 接入后再绑定真实 GPU fence / swapchain present fence。
```

### RenderFenceResult

```text
RenderFenceResult
  fence_id
  sync_depth
  status
  wait_ms
  completed_frame_index
  diagnostics
```

### FrameLagController

```text
FrameLagController
  max_frames_in_flight
  current_game_frame
  completed_render_frame
  should_wait()
  wait_target_frame()
```

第一版默认：

```text
max_frames_in_flight = 1 or 2
```

规则：

```text
FrameLagController 只在 frame boundary 生效。
不要在每次 submit_frame 时盲目阻塞。
```

### RenderWorkerReport

```text
RenderWorkerReport
  schema_version
  worker_id
  mode
  state
  last_submitted_frame
  last_completed_frame
  in_flight_frames
  frame_lag
  fence_wait_count
  timeout_count
  worker_lost
  diagnostics
```

## 强制规则

```text
项目逻辑不能直接接触 RenderThreadWorker。
项目逻辑不能直接创建 RenderFence。
RenderCommandDispatcher 是 Game Thread 到 RenderThread 的唯一入口。
RenderThread worker 独占 RenderThread / RenderResourceManager。
Shutdown 必须通过 Shutdown command + fence + join。
Flush 只允许用于 Editor step、测试、资源强同步、shutdown。
Flush 不允许成为普通每帧热路径。
Frame lag 控制只发生在 frame boundary。
queue full 不等于 frame lag，不等于 fence wait，不等于 worker lost。
```

## 第一版 D-min

第一版实现范围：

```text
真实 spawn OS RenderThread worker。
RenderCommandDispatcher。
RenderThreadCommand::SubmitFrame / InsertFence / Shutdown。
RenderFence / RenderFenceResult。
FrameLagController。
RenderWorkerReport。
DedicatedThread 模式从 diagnostic 升级为真实 worker。
InlineDeterministic 保留为 headless deterministic test mode。
```

第一版不做：

```text
完整 UE TaskGraph。
完整 RenderCommandPipe recording。
RHIThread。
无锁高性能队列。
GPU timeline semaphore。
多 RenderThread。
多 Viewport 独立 worker。
```

## AI 友好规则

AI 默认只看：

```text
RenderSubmissionReport
RenderWorkerReport
RenderFenceResult
RenderResourceLifetimeReport
```

报告必须能回答：

```text
这一帧是否被提交。
这一帧是否被 worker 执行。
这一帧是否 present。
Game 是否领先 Render 太多。
等待发生在 FrameLag、Flush、Shutdown 还是 ResourceRelease。
RenderThread 是否 worker lost / timeout。
```

报告不要只写：

```text
queue full
```

必须拆分为：

```text
frame_lag_exceeded
fence_timeout
worker_lost
shutdown_timeout
resource_release_waiting_safe_frame
```

## 与现有文档关系

```text
17-Runtime-FrameLoop.md
  定义 Game / Render / Worker / IO 线程域。

50-RenderCommand-RenderSceneState方案.md
  定义 RenderCommand / RenderSceneState / RenderProxy。

80-GPU-Resource-Pool-RenderResourceLifetime-v1方案.md
  定义 RenderResourceManager / delayed release。

81-真实跨线程RenderThreadQueue-RenderSubmissionPipeline-v1方案.md
  定义 RenderFramePacket / RenderSubmissionReport / submit pipeline。

本文档补齐：
  真实 OS RenderThread worker、fence、frame lag、shutdown。
```

## 下一步

确认本文档后，可以生成施工文档：

```text
82-当前可自动化施工文档-真实OSRenderThreadWorker-Fence-FrameLag-D-min.md
```

施工应按系统分段：

```text
A1 RenderFence / RenderFenceResult 数据结构
A2 RenderCommandDispatcher 接口
A3 RenderThreadWorker 真实 OS thread
A4 FrameLagController
A5 Shutdown / Flush fence
A6 EngineHostLoop 接入 DedicatedThread
A7 headless deterministic + dedicated worker tests
```

