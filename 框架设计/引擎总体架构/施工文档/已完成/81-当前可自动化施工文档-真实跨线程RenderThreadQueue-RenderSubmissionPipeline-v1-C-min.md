# 81-当前可自动化施工文档-真实跨线程 RenderThreadQueue / RenderSubmissionPipeline v1 C-min

## 目标

按照 `81-真实跨线程RenderThreadQueue-RenderSubmissionPipeline-v1方案.md` 实现第一版 C-min。

目标不是一次实现完整 UE 级 Render Thread，而是先把长期边界落进代码：

```text
RenderFramePacket
RenderThreadQueue
RenderSubmissionTicket
RenderSubmissionReport
EngineHostLoop -> submit_frame
headless deterministic tests
```

## 施工范围

### A1：RenderFramePacket

新增 owned packet：

```text
RenderFramePacket
  frame_index
  render_scene_state
  render_frame_report
  resource_requests
  resource_release_requests
  view_id
  quality_profile
  render_target
```

要求：

```text
不持有借用。
可在线程之间移动。
可由 RenderThreadFrameInput 转换。
```

### A2：RenderSubmissionTicket / RenderSubmissionReport

新增：

```text
RenderSubmissionTicket
RenderSubmissionReport
```

报告字段第一版少而精：

```text
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
```

### A3：RenderThreadQueue C-min

实现：

```text
submit(packet) -> ticket
process_next(render_thread) -> Option<report>
poll_report(ticket) -> Option<report>
completed_frame_index()
drain_shutdown()
pending_len()
```

第一版用 `VecDeque`。

### A4：RenderThread 接入 submit_frame

新增：

```text
RenderThread::submit_frame(packet) -> RenderSubmissionReport
RenderThread::render_frame(input) -> RenderThreadFrameOutput 保留兼容
```

`render_frame` 内部走 packet / submit pipeline。

### A5：EngineHostLoop 改走 submit_frame

要求：

```text
EngineHostLoop 不直接调用底层 render execution。
EngineHostLoop 构建 RenderFramePacket 并提交。
旧测试继续通过。
```

### A6：测试

至少新增 / 更新测试：

```text
RenderFramePacket 是 owned 且能 clone。
RenderThreadQueue submit 后能 process_next。
poll_report 能按 ticket 找到 report。
queue report 能暴露 queue_depth / completed_frame_index。
EngineHostLoop 仍能跑 exported game / editor step。
DedicatedThread 模式第一版返回明确 diagnostic，不假装已 spawn OS thread。
```

## 非目标

```text
不实现完整 OS render thread worker。
不实现无锁高性能队列。
不实现 GPU timeline semaphore。
不实现完整 UE TaskGraph。
不改项目侧 API。
```

## 验收命令

```text
cargo test -p engine_runtime render_thread
cargo test -p engine_runtime engine_host_loop
cargo test -p engine_runtime
```

## 完成后文档动作

完成后移动本文档到：

```text
施工文档/已完成/
```

并新增阶段完成记录：

```text
阶段完成记录/2026-06-28-真实跨线程RenderThreadQueue-RenderSubmissionPipeline-v1/
```

