# 82-当前可自动化施工文档：真实 OS RenderThread Worker / Fence / FrameLag D-min

## 目标

按照 `82-真实OSRenderThreadWorker-Fence-FrameLag方案.md` 实现 D-min。

本阶段目标不是复刻 UE TaskGraph，而是把此前 `DedicatedThread` 的 diagnostic 升级为真实 OS RenderThread worker，并建立：

```text
RenderCommandDispatcher
RenderThreadWorker
RenderThreadCommand
RenderFence / RenderFenceResult
FrameLagController
RenderWorkerReport
Flush / Shutdown fence
```

## 当前基础

已完成：

```text
81-真实跨线程RenderThreadQueue-RenderSubmissionPipeline-v1方案.md
RenderFramePacket
RenderThreadQueue
RenderSubmissionTicket
RenderSubmissionReport
RenderThread::submit_frame
RenderThread::submit_frame_output
EngineHostLoop -> RenderFramePacket -> submit_frame_output
```

当前问题：

```text
RenderThreadMode::DedicatedThread 仍只是 diagnostic。
没有真实 OS worker thread。
没有 flush fence。
没有 shutdown fence。
没有 frame lag 控制。
```

## 施工原则

```text
不把所有问题简化成 queue full。
不做 UE TaskGraph。
不做 RenderCommandPipe recording。
不做 RHIThread。
不做 GPU timeline semaphore。
不改项目侧 API。
不让项目逻辑直接接触 RenderThreadWorker / RenderFence。
```

第一版只做引擎内部 D-min。

## A1：RenderFence / RenderFenceResult 数据结构

新增到 `engine_runtime::render_thread` 或独立 `render_thread_worker` 模块：

```text
RenderFenceId
RenderFenceSyncDepth
  RenderThread
  RhiSubmit
  Present

RenderFenceStatus
  Pending
  Completed
  Timeout
  WorkerLost

RenderFence
  fence_id
  sync_depth
  created_frame_index
  target_frame_index optional

RenderFenceResult
  fence_id
  sync_depth
  status
  wait_ms
  completed_frame_index
  diagnostics
```

第一版规则：

```text
RenderThread depth 必须真实实现。
RhiSubmit / Present depth 第一版允许映射到 RenderThread depth，并输出 diagnostic：simulated_sync_depth。
```

测试：

```text
RenderFenceResult 可序列化。
RenderThread depth fence 可以完成。
RhiSubmit / Present depth 有明确 simulated diagnostic。
```

## A2：RenderThreadCommand

新增：

```text
RenderThreadCommand
  SubmitFrame {
    packet: RenderFramePacket,
    ticket: RenderSubmissionTicket
  }
  InsertFence {
    fence: RenderFence
  }
  Shutdown {
    fence: RenderFence
  }
```

规则：

```text
第一版只允许三类 command。
不允许扩张为万能 command bus。
SubmitFrame 必须输出 RenderSubmissionReport。
InsertFence 必须输出 RenderFenceResult。
Shutdown 必须 drain 已提交 command 后输出 RenderFenceResult。
```

测试：

```text
SubmitFrame command 能执行并产出 RenderSubmissionReport。
InsertFence command 能在前序 SubmitFrame 后完成。
Shutdown command 能返回 completed。
```

## A3：RenderThreadWorker 真实 OS thread

新增：

```text
RenderThreadWorker
  worker_id
  config
  state
  command_sender
  report_receiver
  fence_receiver
  thread_handle
```

内部：

```text
spawn std::thread
worker owns RenderThread
worker loop receives RenderThreadCommand
```

第一版可以使用 Rust 标准库：

```text
std::sync::mpsc
std::thread
std::time::Instant
```

规则：

```text
RenderThreadWorker 独占 RenderThread。
Game Thread 不持有 RenderThread 可变引用。
Worker lost 必须能通过 RenderWorkerReport / RenderFenceResult 表达。
```

测试：

```text
worker spawn 后可以提交一帧。
worker report 记录 last_submitted_frame / last_completed_frame。
worker shutdown 后不能继续 submit。
```

## A4：RenderCommandDispatcher

新增：

```text
RenderCommandDispatcher
  mode
  inline_render_thread optional
  worker optional
  submit_frame(packet) -> (RenderSubmissionTicket, Option<RenderSubmissionReport>)
  poll_submission(ticket) -> Option<RenderSubmissionReport>
  insert_fence(sync_depth) -> RenderFence
  wait_fence(fence, timeout) -> RenderFenceResult
  flush(sync_depth) -> RenderFenceResult
  shutdown() -> RenderFenceResult
```

模式：

```text
InlineDeterministic
DedicatedWorker
```

规则：

```text
InlineDeterministic 仍同步执行，用于 headless / CI / deterministic tests。
DedicatedWorker 真实 submit 到 OS worker thread。
EngineHostLoop 不再直接持有 RenderThread。
EngineHostLoop 持有 RenderCommandDispatcher。
```

测试：

```text
InlineDeterministic 路径保持现有测试通过。
DedicatedWorker 路径提交后可 poll 到 report。
flush(RenderThread) 能等待前序 frame 完成。
shutdown 能安全结束 worker。
```

## A5：FrameLagController

新增：

```text
FrameLagController
  max_frames_in_flight
  current_game_frame
  completed_render_frame
  should_wait()
  wait_target_frame()
```

规则：

```text
FrameLagController 只在 frame boundary 生效。
submit_frame 不因为 pending 数直接盲目阻塞。
默认 max_frames_in_flight = 2。
EditorStep / tests 可以配置 max_frames_in_flight = 1。
```

报告字段：

```text
frame_lag
max_frames_in_flight
waited_for_frame_lag
frame_lag_wait_ms
```

测试：

```text
completed_render_frame 落后超过 max_frames_in_flight 时 should_wait = true。
未超过时不等待。
wait_target_frame 返回应等待的 render frame。
```

## A6：RenderWorkerReport

新增：

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

规则：

```text
AI 默认看 RenderWorkerReport / RenderSubmissionReport / RenderFenceResult。
不要只输出 queue_full。
必须区分 frame_lag_exceeded / fence_timeout / worker_lost / shutdown_timeout。
```

测试：

```text
worker report 可序列化。
提交一帧后 last_submitted_frame / last_completed_frame 正确。
shutdown 后 state 正确。
```

## A7：EngineHostLoop 接入

修改：

```text
EngineHostLoop
  render_thread: RenderThread
```

改为：

```text
EngineHostLoop
  render_dispatcher: RenderCommandDispatcher
  frame_lag_controller: FrameLagController
```

规则：

```text
ExportedGame / EditorPlay 可使用 DedicatedWorker 配置。
HeadlessServer 默认仍不 build render。
EditorStep / tests 默认可以使用 InlineDeterministic，保证稳定。
```

第一版可以新增构造函数：

```text
EngineHostLoop::new_with_render_mode(scene_id, RenderThreadMode)
```

测试：

```text
现有 EngineHostLoop 测试不回退。
新增 dedicated worker EngineHostLoop 测试。
EditorStep 可以 flush 到 frame 完成。
```

## A8：Shutdown / Drop 安全

规则：

```text
RenderCommandDispatcher::shutdown 必须显式调用。
Drop 中如果 worker 仍存在，必须尝试 shutdown。
测试中不允许泄漏 worker thread。
Shutdown command 必须在 drain 前序 SubmitFrame 后完成。
```

测试：

```text
shutdown after frame submit 能完成。
shutdown without submit 能完成。
shutdown 后再次 submit 返回 worker_stopped diagnostic。
```

## A9：验证命令

必须通过：

```text
cargo test -p engine_runtime render_thread
cargo test -p engine_runtime engine_host_loop
cargo test -p engine_runtime
```

如果新增独立模块：

```text
cargo test -p engine_runtime render_thread_worker
```

## 验收标准

完成后必须满足：

```text
1. DedicatedThread 不再只是 diagnostic，能真实 spawn OS worker。
2. RenderCommandDispatcher 是 Game -> Render 的统一入口。
3. RenderFence / Flush / Shutdown 有最小可测闭环。
4. FrameLagController 有独立测试，不靠 queue full 解释所有问题。
5. EngineHostLoop 能在 InlineDeterministic 和 DedicatedWorker 两种路径下跑。
6. 所有报告可序列化，AI 能区分 frame_lag / fence_timeout / worker_lost / shutdown。
7. 全量 engine_runtime 测试通过。
```

## 完成后文档动作

完成后移动本文档到：

```text
施工文档/已完成/
```

新增阶段完成记录：

```text
阶段完成记录/2026-06-28-真实OSRenderThreadWorker-Fence-FrameLag-D-min/
```

同步更新：

```text
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
18-长期主义实现路径.md
阶段完成记录/README.md
```

