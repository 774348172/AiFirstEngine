# 81-真实跨线程 RenderThreadQueue / RenderSubmissionPipeline v1 方案

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

本文档定义真实跨线程 Render Thread Queue / Render Submission Pipeline v1 的长期规则。

它解决的问题是：

```text
Runtime / Game Thread 如何把一帧渲染输入提交给 Render Thread。
Render Thread 如何独占 RenderSceneState / RenderResourceManager / RDG / RHI submit。
GPU resource delayed release 如何获得 safe frame / fence 边界。
AI / Trace / Report 如何看懂渲染提交是否卡住、丢帧、失败或延迟释放。
```

本系统不是项目侧规则系统。它是引擎内部渲染线程提交协议。

## 参考引擎

### Unreal Engine

源码参考：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/RenderCore/Public/RenderingThread.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/RenderCore/Public/RenderCommandFence.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/RenderCore/Private/RenderResource.cpp
```

UE 的核心做法：

```text
Game Thread 通过 ENQUEUE_RENDER_COMMAND 向 Render Thread 提交命令。
FRenderCommandFence 用于等待某批 render command 完成。
BeginInitResource / BeginReleaseResource 把 GPU resource 初始化和释放放到渲染线程执行。
FlushRenderingCommands 是强同步工具，正常热路径不应频繁使用。
```

可借鉴：

```text
Game / Render 线程隔离。
Render command 是跨线程协议。
Render resource init / release 必须进入 Render Thread。
Fence 是资源释放、shutdown、强同步的基础。
```

不照搬：

```text
第一版不实现完整 UE TaskGraph。
第一版不实现完整 FRenderCommandPipe。
第一版不实现复杂并行 render command recording。
```

### Unity

源码参考：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Runtime/Export/RenderPipeline/ScriptableRenderContext.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Runtime/Export/PlayerLoop/PlayerLoop.bindings.cs
```

Unity 的核心做法：

```text
用户主要理解 Update / LateUpdate / Render。
SRP 使用 ScriptableRenderContext.ExecuteCommandBuffer / Submit 提交渲染命令。
底层线程和 native renderer 对普通用户隐藏。
```

可借鉴：

```text
用户心智保持简单。
项目侧不接触 RenderThreadQueue。
```

不照搬：

```text
不把提交管线完全黑盒化。
本项目需要 AI 可读 RenderSubmissionReport 来查问题。
```

### Bevy

源码参考：

```text
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_render/src/lib.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_render/src/pipelined_rendering.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_render/src/render_asset.rs
```

Bevy 的核心做法：

```text
MainWorld -> ExtractSchedule -> RenderApp / RenderWorld。
Prepare / Queue / Render 分阶段处理渲染资源和 draw。
```

可借鉴：

```text
Game World 到 Render World 的数据抽取思想。
渲染侧数据独立于 gameplay ECS。
```

不照搬：

```text
本项目已经确定 RenderCommandQueue / RenderSceneState / RenderProxy。
第一版不再引入完整 RenderWorld，避免层级继续变厚。
```

## 方案选择

### 方案 A：UE-like RenderFramePacket + RenderThreadQueue + Fence

```text
Game Thread
  -> RenderExtract
  -> RenderFramePacket
  -> RenderThreadQueue.submit

Render Thread
  -> consume packet
  -> update RenderSceneState / RenderResourceManager
  -> build RDG
  -> compile RHI plan
  -> backend submit / present
  -> signal report / fence
```

优点：

```text
长期边界正确。
GPU resource delayed release 能闭环。
AI 可以通过 report 查提交问题。
和现有 RenderCommand / RenderSceneState / RenderResourceManager 文档一致。
```

缺点：

```text
第一版实现复杂度高于 inline render。
```

### 方案 B：Bevy-like RenderWorld

优点：

```text
ECS 结构清晰。
适合纯 ECS renderer。
```

缺点：

```text
会和现有 RenderSceneState / RenderProxy 重叠。
AI 排查路径会多一层。
```

### 方案 C：Unity-like 黑盒提交

优点：

```text
用户最简单。
```

缺点：

```text
AI 查 bug 能力弱。
资源生命周期和提交延迟不透明。
不适合我们长期的 AI-first 目标。
```

## 最终规则

采用方案 A。

吸收 Bevy 的 Extract 思想，保留 Unity 的用户心智，但底层学习 UE 的 Game Thread -> Render Thread 隔离。

正式链路：

```text
Runtime / Game Thread
  -> ECS Update / LateUpdate
  -> RenderDirtyTracker
  -> RenderExtract
  -> RenderCommandQueue
  -> RenderFramePacket
  -> RenderThreadQueue.submit(packet)

Render Thread
  -> receive RenderFramePacket
  -> apply RenderCommandQueue / RenderSceneState
  -> process RenderResourceRequest / RenderResourceReleaseRequest
  -> RendererFeatureBuilder
  -> RDG build
  -> RHI CommandPlan
  -> Backend submit / present
  -> RenderSubmissionReport
  -> RenderFence / completed_frame
```

## 标准结构

```text
RenderFramePacket
  frame_index
  scene_state
  render_frame_report
  resource_requests
  resource_release_requests
  view_id
  quality_profile
  render_target

RenderSubmissionTicket
  frame_index
  submit_sequence

RenderSubmissionReport
  schema_version
  frame_index
  submit_sequence
  accepted
  submitted
  presented
  completed_frame_index
  queue_depth_after_submit
  queue_wait_frames
  thread_mode
  diagnostics
  render_thread_report

RenderThreadQueue
  submit(packet) -> RenderSubmissionTicket
  process_next() -> Option<RenderSubmissionReport>
  poll_report(ticket) -> Option<RenderSubmissionReport>
  completed_frame_index()
  drain_shutdown()
```

## 强制边界

```text
Game Thread 不直接读写 RenderSceneState。
Render Thread 不直接读 ECS。
跨线程只传 RenderFramePacket / Report / Fence / Shutdown。
RenderFramePacket 进入队列后只读。
RenderThread 独占 RenderSceneState 和 RenderResourceManager。
GPU resource 释放只能发生在 RenderThread / RenderResourceManager 内。
Headless / CI 也必须走 submit / report / fence 边界。
```

## 第一版边界

第一版实现 C-min：

```text
实现 RenderFramePacket owned 数据结构。
实现 RenderSubmissionTicket / RenderSubmissionReport。
实现 RenderThreadQueue C-min。
实现 InlineDeterministic queue mode。
保留 DedicatedThread 枚举，但第一版不 spawn OS thread。
EngineHostLoop 改为通过 submit_frame 进入 RenderThread。
RenderResourceManager 的 safe frame 仍以 completed_frame_index 为基础推进。
```

第一版不做：

```text
完整无锁队列。
完整 OS Render Thread worker loop。
完整 TaskGraph。
多队列 GPU submit。
复杂 GPU timeline semaphore。
```

## AI 友好规则

默认 AI 只看：

```text
RenderSubmissionReport
RenderThreadReport
RenderFrameReport summary
RenderResourceLifetimeReport
```

只有当 report 解释不了问题时，AI 才下钻：

```text
RenderCommandQueue
RDG
RHI CommandPlan
Backend report
```

报告必须回答：

```text
这一帧是否进入 RenderThreadQueue。
是否被 RenderThread 消费。
是否提交到 RHI backend。
是否 present。
资源释放是否等待 safe frame。
队列是否堆积。
```

## 与现有文档关系

```text
17-Runtime-FrameLoop.md
  定义多线程线程域和 Game / Render 隔离。

50-RenderCommand-RenderSceneState方案.md
  定义 RenderCommandQueue / RenderSceneState。

80-GPU-Resource-Pool-RenderResourceLifetime-v1方案.md
  定义 RenderResourceManager 和 delayed release。

本文档补齐：
  真实跨线程 RenderThreadQueue / RenderSubmissionPipeline。
```

