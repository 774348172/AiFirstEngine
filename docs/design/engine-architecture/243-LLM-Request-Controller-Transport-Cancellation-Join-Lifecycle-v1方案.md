# 243-LLM Request Controller / Transport Cancellation / Join Lifecycle v1 方案

> 状态：正式方案、Gate A-F、整体回归、完成记录和施工归档均已完成；INC-02 已关闭。  
> 建立日期：2026-07-11。  
> 选题来源：`240-5.6审查剩余问题讨论与施工优先级.md` Priority 3 / INC-02。  
> 前置系统：`238-Real-LLM-Provider-Minimal-Repair-Loop-v1方案.md`、239/241/242 已完成施工。  
> 用户确认：采用讨论方案 B，建立异步可取消 transport 与深 `LlmRequestController`。  
> 目标：Cancel、Session shutdown 和 Drop 不再只丢弃 UI receiver；每个逻辑 LLM 请求必须由单一 owner 管理 transport cancellation、task terminal、join、prompt/context/credential lease 释放和结构化 lifecycle receipt。

## 1. 这个系统是干什么的

直白地说，这个系统负责让“取消 LLM 请求”成为真实工程行为，而不是只改变界面文字。

```text
当前：
用户点击 Cancel
  -> EditorSession 丢弃 receiver
  -> UI 立刻显示 Cancelled
  -> detached std::thread 仍可能继续网络请求、重试和读响应
  -> prompt / context / API key clone 继续存活到 timeout
  -> 用户可立即启动第二个请求

目标：
用户点击 Cancel
  -> request state = Cancelling
  -> cancellation token 通知 async transport
  -> connect / response / body / retry wait 被中断
  -> task 进入唯一 terminal
  -> task join
  -> prompt / context / credential owner 释放
  -> lifecycle receipt 形成
  -> request state = Cancelled
  -> 才允许启动下一个请求
```

它大致对应：

```text
Rust task cancellation + JoinHandle ownership。
UnityWebRequest.Abort 的“尽快停止传输”语义。
Godot HTTPRequest::cancel_request 的 quit -> wait_to_finish -> client close -> idle 顺序。
编辑器窗口关闭前的异步作业 shutdown/drain。
```

## 2. 正式决策

正式采用：

```text
方案 B：Async Cancellable Transport + Deep LlmRequestController

LlmRequestController
  + process-owned LlmAsyncExecutor
  + cancellation token
  + owned async task handle
  + ReqwestAsyncTransport production Adapter
  + ControllableLoopbackTransport test Adapter
  + explicit Cancelling / Joined terminal states
  + Session shutdown / Drop fallback reaper
  + LlmPatchRequestReport v3 lifecycle evidence
```

只迁移 Editor LLM HTTP transport：

```text
移除 editor_core 对 ureq 的直接依赖。
使用 reqwest async + rustls。
使用 Tokio runtime/task/time/sync。
使用 tokio-util CancellationToken 或等价正式 cancellation primitive。
不把 async runtime 扩散到 Runtime、Renderer、ECS、ProjectPatch Validator 或普通 Editor command interface。
```

## 3. 为什么不继续沿用 238 的 ureq 取消语义

238 当时明确选择：

```text
ureq blocking client
+ std::thread worker
+ mpsc receiver
+ Cancel 只忽略迟到结果
+ socket 可继续到 timeout
```

这是 238 的已知 B-min+ 边界，不是实现偏差。5.6 增量审查证明它已经不满足当前资源与隐私生命周期要求。

当前源码事实：

```text
services/ai_service.rs
  generate 与 repair 各自 std::thread::spawn。
  spawn 返回的 JoinHandle 没有保存。
  Cancel take active request 后立即写 final_status=cancelled。

session.rs
  ActiveLlmPatchRequest 保存 receiver、prompt、context_json、config。
  没有 cancellation token / task handle / shutdown receipt。

project_patch/llm_http.rs
  ureq Agent 只有 timeout_global。
  send_json / read_to_string 是同步阻塞调用。
  Retry-After 使用 thread::sleep。

EditorSession
  没有 LLM shutdown，也没有相关 Drop 清理。
```

本机锁定的 `ureq 3.3.0` 源码公开 timeout 配置，但没有请求级 public cancel/abort interface。给现有 worker 增加 `AtomicBool` 只能在 request 前后或 retry 间隙协作检查，不能中断正在进行的 TLS/connect/body I/O。

因此 243 不采用以下假完成：

```text
只保存 std::thread::JoinHandle，但 Cancel 仍等 30 秒 timeout。
只设置 AtomicBool，却继续显示“Cancelled”。
Cancel 后把旧 worker 交给新的 detached reaper thread。
缩短全局 timeout 来伪装取消。
只阻止 proposal 落地，不处理 worker/secret 生命周期。
```

### 3.1 对 238 的取代范围

243 只取代 238 中以下具体条款：

```text
第 9.3 节：ureq blocking client + std::thread worker。
第 10.2/10.3 节：receiver-only pending state 与 detached thread pump。
第 10.4 节：Cancel 立即解除 busy、只忽略迟到结果、socket 可继续到 timeout。
第 13.1 节：缺少 Cancelling 状态。
第 17.2 节：Cancel Gate 只验证迟到结果被忽略。
第 19 节：transport-level immediate abort deferred。
```

238 其余合同继续有效：

```text
HTTPS/loopback endpoint policy。
request/response/candidate size limit。
provider result classification。
最多一次 transport retry 与一次 Patch repair。
ProjectPatch Import/Validator/Review/Confirm/transaction。
context stale guard。
Report Off/Summary/Trace 与 secret/path redaction。
Runtime/export provider Off。
```

## 4. 成熟实现与源码参考

### 4.1 Rust JoinHandle

Rust 官方 `std::thread::JoinHandle` 明确：丢弃 handle 会 detach，之后没有办法 join。

可学习：

```text
worker handle 必须由生命周期 owner 持有。
terminal 不能只根据 receiver 是否还存在判断。
join 是线程/任务完成证据，不是可选清理。
```

不可照搬：

```text
std::thread 本身没有安全的强制终止能力。
对阻塞 ureq 线程保存 JoinHandle 仍不能提供及时取消。
```

### 4.2 UnityWebRequest.Abort

Unity 官方文档定义：进行中的请求应尽快停止上传或下载，取消成为显式错误状态 `User Aborted`。

可学习：

```text
Cancel 必须进入 transport。
取消是稳定终态，不应伪装成普通 provider failure。
对已经完成的请求，Cancel 不应制造第二个终态。
```

不可照搬：

```text
“as soon as possible”不等于远端 provider 已停止推理或停止计费。
本项目必须额外提供 task join 与 secret owner release evidence。
```

### 4.3 Godot HTTPRequest

Godot `scene/main/http_request.cpp`：

```text
HTTPRequest::request_raw
  -> requesting=true 时拒绝第二个请求
  -> threaded 模式启动 thread

HTTPRequest::cancel_request
  -> timer stop
  -> thread_request_quit.set
  -> thread.wait_to_finish
  -> client->close
  -> clear body/state
  -> requesting=false
```

可学习：

```text
取消期间仍处于 requesting/busy。
先通知退出、等待 worker、关闭 client、清理数据，最后才回到 idle。
同一个 owner 管理传输与 worker 状态。
```

不可照搬：

```text
Godot 的 blocking thread + wait_to_finish 可以阻塞调用者。
本 Editor 主线程不能在普通 Cancel command 中同步等待网络线程。
243 使用异步 cancel + frame pump terminal，只有显式 shutdown 可做有界等待。
```

### 4.4 Tokio cancellation

采用的工程语义：

```text
CancellationToken 通知合作取消。
tokio::select! 同时等待 transport future 与 cancellation。
task JoinHandle/AbortHandle 由 Controller 持有。
Cancel 先 token cancel，必要时 task abort；随后 await/join terminal。
time::sleep / timeout 必须位于同一 cancellation select 中。
```

限制：

```text
Tokio task abort 只在异步 yield 点生效。
因此 task 内禁止 std::thread::sleep、ureq、阻塞 DNS/文件 I/O 或 spawn_blocking provider request。
```

## 5. 固定范围

### 5.1 必须交付

```text
深 LlmRequestController module。
process-owned LlmAsyncExecutor 与 task reaper/drain。
async cancellation-capable HTTP transport port。
ReqwestAsyncTransport production Adapter。
ControllableLoopbackTransport deterministic test Adapter。
generate/repair 同一 logical request lifecycle。
Cancelling UI state 与 busy semantics。
Cancel / Session shutdown / Drop fallback ownership。
prompt/context/credential lease 的 request-scoped ownership。
LlmPatchRequestReport v3 lifecycle evidence。
真实 loopback connect/header/body/retry cancel matrix。
default/all-features workspace regression。
```

### 5.2 明确不扩入

```text
Provider Registry、模型路由、fallback、投票或 ensemble。
Agent Planner、tool calling、MCP 或自主任务图。
多请求并发、请求队列或后台批处理。
streaming token UI、conversation memory 或 prompt history truth。
LLM 子进程 worker、IPC protocol 或 OS sandbox。
远端 provider cancellation/billing protocol。
OS credential vault 产品化。
ProjectPatch schema、Validator、repair scope、Apply/rollback 重写。
Runtime、Player 或导出包内 provider。
```

## 6. 深 Module 与 seam

### 6.1 外部 interface

`EditorSession` 只跨以下 interface：

```text
LlmRequestController::start(request_spec) -> Result<RequestId, LlmLifecycleDiagnostic>
LlmRequestController::poll() -> Vec<LlmRequestEvent>
LlmRequestController::resolve_attempt(request_id, decision) -> Result<(), ...>
LlmRequestController::cancel(request_id, cancel_source) -> CancelReceipt
LlmRequestController::shutdown(deadline) -> LlmShutdownReceipt
```

其中 `decision` 是主线程完成 Import / Validator / RepairDecision 后提交的唯一决议：

```text
Complete
Fail(diagnostic_summary)
ContinueRepair(repair_spec)
```

这样 `WaitingForMainThreadDecision` 不依赖隐式 field drop：无需 repair 时也有明确 terminal 入口；需要 repair 时由同一方法继续同一 logical request。该补全不增加 caller 必须协调的第二套状态机。

调用方不需要知道：

```text
Tokio runtime / channel / task handle。
reqwest Client / TLS / HTTP version。
CancellationToken / AbortHandle。
retry timer 实现。
task reaper。
secret storage。
join race / terminal latch。
```

### 6.2 Transport 内部 seam

LLM provider 是 true external dependency，因此定义内部 port：

```text
LlmTransport
  execute(attempt_spec, cancellation) -> async LlmTransportResult
```

两个真实 Adapter：

```text
ReqwestAsyncTransport
  production HTTPS / loopback HTTP。

ControllableLoopbackTransport
  deterministic connect/header/body/retry/cancel/panic/drop probes。
```

该 port 是 Controller implementation 的内部 seam，不向 AI Panel、EditorSession 或 ProjectPatch public surface 暴露 HTTP client 细节。

### 6.3 删除测试

如果删除 Controller，以下复杂度会重新散回 `ai_service.rs`、`session.rs`、window close 和测试：

```text
single-active enforcement。
cancel state/race。
transport abort。
task join/reaper。
repair attempt handoff。
shutdown/drop。
secret owner release。
lifecycle report。
```

因此该 module 提供真实 depth、leverage 和 locality，不是 pass-through wrapper。

## 7. Request-owned 数据合同

### 7.1 LlmRequestSpec

```text
request_id
provider_id / model / endpoint metadata
original_prompt
context_json
context_hash / schema_hash
limits / timeout / retry policy
structured_output_mode
credential_lease
```

### 7.2 Session 保留的数据

`EditorSession` 只保留：

```text
request_id
expected_post_start_revision
context_hash
attempt_index / phase
initial candidate/import evidence（repair 业务需要）
controller handle/reference
```

不再在 `ActiveLlmPatchRequest` 内复制保存：

```text
API key。
Authorization header。
完整 transport config clone。
worker-owned prompt/context clone。
receiver-only worker ownership。
```

### 7.3 Credential lease

将 provider metadata 与 credential owner 分离：

```text
LlmTransportConfig
  可 Clone；不含 secret。

LlmCredentialLease
  不 Serialize。
  不 Debug 明文。
  默认不 Clone。
  request start 时 move 进 Controller/task。
  使用 zeroizing owner storage 或等价受审查实现。
```

报告只允许声明：

```text
credential_owner_status = held | released
```

禁止声明：

```text
all_memory_zeroized
```

原因是 TLS/HTTP library 可能创建内部 header buffer；本项目可以证明 request owner 已释放和 transport task 已 drop，不能证明第三方库进程内每个历史字节都已覆写。

## 8. 生命周期状态机

固定状态：

```text
Idle
Starting
RunningGenerate
WaitingForMainThreadDecision
RunningRepair
Cancelling
CompletedJoined
FailedJoined
CancelledJoined
ShutdownJoinTimedOut
```

主链：

```text
Idle
  -> start
  -> RunningGenerate
  -> attempt result
  -> task join
  -> WaitingForMainThreadDecision
       -> accepted / non-repairable -> CompletedJoined | FailedJoined
       -> eligible repair -> RunningRepair
  -> repair result
  -> task join
  -> CompletedJoined | FailedJoined
```

取消链：

```text
RunningGenerate / WaitingForMainThreadDecision / RunningRepair
  -> cancel accepted
  -> Cancelling
  -> token cancel
  -> task abort fallback
  -> task terminal
  -> task join
  -> request-owned prompt/context/credential release
  -> CancelledJoined
```

### 8.1 Single terminal

Controller 内部使用串行 actor/event loop 或等价 terminal latch，保证：

```text
每个 request_id 只有一个 terminal。
cancel 与 completion race 不会同时产生 proposal 和 Cancelled。
cancel accepted 在 terminal event 提交前发生 -> cancel wins，结果丢弃。
terminal 已提交并 join 后再 cancel -> no_active_request，不改写历史终态。
```

### 8.2 Busy 规则

以下状态全部 `busy=true`：

```text
Starting
RunningGenerate
WaitingForMainThreadDecision
RunningRepair
Cancelling
```

只有 `CompletedJoined / FailedJoined / CancelledJoined` 完成清理后，Controller 才能回到 Idle 并接受新请求。

`ShutdownJoinTimedOut` 是 fail-closed shutdown receipt，不是可复用 terminal：Controller 不得回到 Idle，也不得接受新请求；未 join 的 handle 必须仍由 executor reaper 持有，直到得到 join evidence 或进程 shutdown 明确失败。

## 9. Async HTTP Transport

### 9.1 正式 Adapter

`ReqwestAsyncTransport`：

```text
reqwest async Client
rustls TLS
redirect disabled
HTTPS required；loopback HTTP 保留测试/本地 provider 规则
request/response/candidate byte limits 继承 238
structured output / error classification 继承 238
最多一次 transport retry 继承 238
```

### 9.2 每个等待点都可取消

必须把 cancellation 放进：

```text
request send / connect / TLS / response headers future。
response body chunk read。
Retry-After/backoff timer。
repair transport attempt。
等待 main-thread repair decision 的 controller wait。
```

禁止：

```text
task 内 thread::sleep。
task 内 ureq/blocking reqwest。
spawn_blocking 包装整个 HTTP 请求。
取消后继续 transport retry。
取消后读取完整 body 再返回。
```

### 9.3 Body limit

不得用无界 `response.text().await` 后再检查长度。

正式行为：

```text
按 chunk 累积。
每个 chunk 进入 cancellation select。
累计超过 maximum_response_bytes 立即终止。
body buffer 只属于 request task。
```

### 9.4 Connection close 证据

loopback HTTP/1 fixture 必须在服务端观察：

```text
client disconnect / response write failure / request body stream ended
```

这是本地 transport cancellation 证据，不推广为所有 provider/HTTP2 的远端执行停止证明。

## 10. Cancel 语义

### 10.1 普通 Cancel command

`CancelLlmPatchRequest`：

```text
命令快速返回，不阻塞 winit frame。
AI Panel 从 Generating/Repairing 进入 Cancelling。
Cancel 按钮禁用或转为状态指示。
Submit 继续禁用。
Controller 异步完成 abort/join。
pump 收到 CancelledJoined 后才显示 Cancelled。
```

### 10.2 Cancelled 的严格定义

只有同时满足以下条件才能写 `final_status=cancelled`：

```text
cancel request 已被 Controller 接受。
当前 transport/task 不再执行。
task JoinHandle 已被 await/join。
result channel 不可能再产生可提交 candidate。
request-owned prompt/context/credential owner 已释放。
lifecycle receipt 已形成。
```

### 10.3 远端状态

本项目不能把本地 socket/task cancellation 等同于 provider 服务器取消。

固定报告：

```text
取消发生在 request send 前：remote_execution_status=not_started
请求可能已发送：remote_execution_status=unknown
```

禁止产品文案：

```text
已停止计费。
远端推理已终止。
provider 已删除请求数据。
```

## 11. Session Shutdown 与 Drop

### 11.1 显式 shutdown

Native Editor 关闭前必须调用：

```text
EditorSession::shutdown_llm(deadline)
  -> controller.shutdown(deadline)
  -> cancel active request
  -> await/abort/join task
  -> drain terminal/reaper
  -> return LlmShutdownReceipt
```

普通窗口关闭不再只依赖 Rust field drop 顺序。

### 11.2 Drop fallback

`Drop` 是兜底，不是正常关闭入口：

```text
若 active task 存在：立即 cancel + abort。
在小而固定的 Drop budget 内尝试 join。
未在 budget 内终止时，把 owned JoinHandle 转交 process-owned executor reaper。
禁止直接 drop handle 形成 detached task。
```

executor shutdown 必须 drain reaper；如果最终 deadline 仍失败，返回/记录 `ShutdownJoinTimedOut`，不得伪装成功。

### 11.3 有界定义

正式方案不提前固定毫秒值；施工文档必须根据 loopback fixture 校准：

```text
Cancel command：frame-safe，不能同步等待网络。
Cancel terminal：应在小于 provider total timeout 的独立 cancel deadline 内完成。
Session shutdown：有界等待并输出 receipt。
Drop budget：显著小于 session shutdown deadline，只作 fallback handoff。
```

任何 deadline 都不能通过继续运行 detached worker 来“通过”。

## 12. Generate / Repair 集成

238 的业务链保持：

```text
generate result
  -> main-thread ProjectPatchImportService
  -> PatchValidator
  -> RepairDecision
  -> at most one repair
  -> review / confirm / transaction
```

243 只修改 worker ownership：

```text
initial attempt 结束后先 join，再交付 result event。
主线程完成 Import / Validator / RepairDecision 后，通过同一 request_id 调用 resolve_attempt。
无需 repair 时提交 Complete/Fail，Controller 形成 Joined terminal 并释放 request-owned 数据。
eligible repair 时提交 ContinueRepair(repair_spec)，继续同一 logical request。
Controller 继续持有原 prompt/context/credential lease。
repair attempt 结束后先 join，再交付 result event。
Cancel 在 WaitingForMainThreadDecision 也能关闭 logical request 并释放数据。
```

不修改：

```text
repair allowlist/denylist。
239 typed repair scope guard。
context stale guard。
ProjectPatch Import/Validator/Review/Confirm/transaction。
```

## 13. Report v3

升级：

```text
llm-patch-request-report.v2
  -> llm-patch-request-report.v3
```

保留 v2：

```text
request/provider/model/structured mode。
attempt summaries。
repair scope。
candidate/context/schema hashes（按 level）。
diagnostic codes。
```

新增 lifecycle：

```text
lifecycle_state
terminal_status
cancel_requested
cancel_source = user | session_shutdown | controller_drop | none
transport_cancel_capability = async_abort
transport_abort_requested
transport_abort_observed
task_join_status = joined | panicked | join_timed_out | not_started
credential_owner_status = held | released
local_execution_status = not_started | running | stopped | completed
remote_execution_status = not_started | unknown
cancel_latency_ms
shutdown_latency_ms
```

### 13.1 分档

```text
Off
  只保留功能所需 state；不生成 report artifact。

Summary
  terminal、cancel source、join、credential owner、local/remote status、总 latency。

Trace
  Summary + phase transitions、attempt timing、redacted transport class、deadline evidence。
```

任何级别禁止：

```text
API key / Authorization。
prompt/context 原文。
raw provider body。
绝对路径。
task debug dump 中的 secret-bearing config。
```

## 14. Diagnostic 固定集合

继承 238 provider/repair codes，新增：

```text
llm_request_controller.busy
llm_request_controller.request_not_found
llm_request_controller.cancel_requested
llm_request_controller.cancelled_joined
llm_request_controller.task_panicked
llm_request_controller.task_join_failed
llm_request_controller.shutdown_join_timed_out
llm_request_controller.executor_unavailable
llm_transport.cancelled
llm_transport.abort_failed
llm_transport.body_limit_exceeded
llm_credential.owner_release_unconfirmed
```

`Cancelled` 不使用通用 `transport_failed` 伪装；join timeout 也不能降级成普通 Cancelled。

## 15. 阻断验证矩阵

### 15.1 Transport cancellation

```text
cancel before request send。
cancel while connect/TLS future pending。
cancel while waiting response headers。
cancel while receiving bounded body chunks。
cancel during Retry-After/backoff。
cancel during repair attempt。
```

### 15.2 Lifecycle race

```text
cancel vs success 同时发生，只产生一个 terminal。
cancel vs provider failure 同时发生，只产生一个 terminal。
重复 Cancel 幂等或返回稳定 no-active，不产生第二 receipt。
Cancelling 期间 Submit 返回 busy。
CancelledJoined 后可立即开始新 request。
旧 request event 不能进入新 generation。
```

### 15.3 Shutdown / Drop

```text
Session shutdown during connect/header/body/retry/repair。
window close 显式 shutdown receipt。
直接 drop EditorSession 走 fallback reaper。
executor shutdown 后 active_task_count=0、reaper_count=0。
join timeout fixture 返回明确失败，不报告 cancelled success。
```

### 15.4 Privacy

```text
credential lease drop probe 在 CancelledJoined 前后正确变化。
prompt/context owner drop probe 在 terminal 后释放。
Summary/Trace JSON 不包含 secret、Authorization、prompt/context 原文、绝对路径。
Debug/serde 不暴露 credential。
```

### 15.5 Existing behaviour

```text
正常 generate -> proposal 不回归。
一次 repair -> proposal 不回归。
provider auth/rate-limit/schema/size diagnostics 不回归。
context stale 不创建 proposal。
人工 Confirm 与 transaction Apply 不回归。
Runtime/export 不初始化 async executor/provider。
```

## 16. 可施工 Gate 建议

正式施工文档生成后建议严格串行：

### Gate A：Lifecycle Contract / Report v3

```text
LlmRequestId / state / event / receipt schema。
Report v3 migration。
terminal latch 与 state transition pure tests。
credential lease owner model。
```

建议命令：

```powershell
cargo test -p editor_core llm_request_lifecycle -- --nocapture
cargo test -p editor_core llm_patch_request_report -- --nocapture
```

### Gate B：Async Transport Adapter

```text
Tokio/reqwest rustls dependencies。
ReqwestAsyncTransport。
remove ureq direct dependency。
cancellable send/header/body/retry。
loopback transport cancellation fixtures。
```

建议命令：

```powershell
cargo test -p editor_core llm_transport -- --nocapture
cargo test -p editor_core llm_transport_cancellation -- --nocapture
```

### Gate C：Deep LlmRequestController

```text
executor/task ownership。
start/poll/resolve_attempt/cancel/shutdown。
single terminal / busy / join / reaper。
panic/disconnect/deadline diagnostics。
```

建议命令：

```powershell
cargo test -p editor_core llm_request_controller -- --nocapture
```

### Gate D：EditorSession / AI Panel / Window Shutdown

```text
replace ActiveLlmPatchRequest receiver-only state。
generate/repair 接入 Controller。
AiPanelStage::Cancelling。
NativeEditorApplication explicit shutdown。
Drop fallback。
```

建议命令：

```powershell
cargo test -p editor_core llm_session_shutdown -- --nocapture
cargo test -p editor_window_winit llm_shutdown -- --nocapture
```

### Gate E：E2E / Negative / Privacy

```text
real loopback connect/header/body/retry cancellation。
resubmit-before/after-join。
generate/repair cancel。
secret/prompt/context drop probe。
Report Off/Summary/Trace redaction。
validation-only llm-worker-lifecycle-report.v1。
```

建议命令：

```powershell
cargo test -p project_e2e_gate llm_worker_lifecycle -- --nocapture
cargo test -p editor_core llm_cancel -- --nocapture
```

### Gate F：Dependency Audit / Full Regression / Closure

```text
复扫 detached LLM std::thread::spawn。
复扫 ureq direct dependency。
复扫 Cancelled-before-join 状态写入。
确认 Runtime/Player/export 不依赖 reqwest/Tokio LLM stack。
完成入口同步、阶段记录和施工归档。
```

阻断命令：

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

## 17. 预计涉及文件

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/editor_core/Cargo.toml

rust/crates/editor_core/src/project_patch/llm_transport.rs（新）
rust/crates/editor_core/src/project_patch/llm_request_controller.rs（新）
rust/crates/editor_core/src/project_patch/llm_http.rs（迁移或删除）
rust/crates/editor_core/src/project_patch/llm_source.rs
rust/crates/editor_core/src/project_patch/llm_request.rs
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/ai_service_tests.rs
rust/crates/editor_core/src/tests/llm_patch_source_tests.rs

rust/crates/editor_ui_model/src/ai_panel.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/project_e2e_gate/src/llm_worker_lifecycle.rs（新）
rust/crates/project_e2e_gate/src/lib.rs
```

列表外命中必须在施工文档中说明所有权和必要性，不得静默扩范围。

## 18. 外部审查结论分类

审查对象：

```text
审查目录/5.6审查目录/01-2026-07-11-新增功能增量代码质量审查报告.md
适用项：INC-02 [P2]
当前方案：243
```

### 18.1 必须修改

| 审查结论 | 243 处理 |
|---|---|
| detached thread，没有 JoinHandle/token | 由 LlmRequestController 持有 task handle/token/terminal/join |
| Cancel 只 take receiver | Cancel 进入 Cancelling，transport abort + task join 后才终态 |
| Cancel 后可并发新请求 | Running/Cancelling/decision wait 全部 busy，Joined terminal 后才允许新建 |
| Session 关闭后 worker/secret 存活 | 显式 shutdown + Drop fallback reaper + credential owner receipt |
| ai_service.rs 状态/网络/evidence 分散 | Controller 深 module 集中 lifecycle，transport port 为内部 seam |

### 18.2 施工约束

```text
取消必须覆盖 transport、join、Session Drop 和 secret/prompt/context owner。
ureq 无 abort 时不得继续宣称真实 Cancel；243 正式替换 transport。
底层本地取消不能声称远端 provider 或计费已取消。
默认/all-features workspace 是阻断 Gate。
```

### 18.3 已由历史施工吸收

```text
INC-01 repair scope 已由 239 Gate A 关闭，243 只继承 typed scope guard。
CQ-03/CQ-05/INC-03 已由 239 关闭，不重复实现 process/publish/PE contract。
CQ-04 已由 241 关闭，不重做 project write containment。
CQ-01 已由 242 关闭，不触碰 ProjectRuntimeModule。
238 的 HTTPS/loopback policy、limits、classification、one-shot repair、redaction、人工确认继续有效。
```

### 18.4 不适用

```text
00 全面审查没有 INC-02；它只作为 01 的增量基线，不机械套用其它 CQ 项。
01 中 CQ-06/CQ-07/CQ-08 仍由 240 后续项目处理。
Clippy/hygiene/CI 数据不扩入 243，避免跳过 Priority 4-6 的独立讨论。
```

## 19. 风险与控制

### 风险 1：为一个 HTTP 请求引入过大 async 架构

控制：async 只藏在 Controller/transport implementation；EditorSession 保持同步 start/poll/cancel/shutdown interface，不扩散到 Runtime/ECS/Renderer。

### 风险 2：task abort 仍不能证明远端停止

控制：报告明确 local vs remote；远端状态默认 unknown，不使用“停止计费”文案。

### 风险 3：Drop 无法 await

控制：正常窗口关闭先显式 shutdown；Drop 只做 cancel/abort + 小 budget join，超时 handle 转交 process-owned reaper，不直接 detach。

### 风险 4：新 runtime 自身成为泄漏点

控制：process-owned executor 有 active/reaper inventory、显式 shutdown 和 drain test；Runtime/export 不初始化。

### 风险 5：reqwest body 读取绕过 size limit

控制：逐 chunk 读取并同时检查 cancellation/累计字节；禁止无界 text()。

### 风险 6：凭据“释放”被夸大成“内存完全清零”

控制：只报告 credential owner released；owner 使用 zeroizing storage，但不对第三方 TLS/HTTP 内部历史 buffer 作不可证明承诺。

### 风险 7：Cancel 与 completion race 产生 proposal

控制：Controller 串行 terminal latch；cancel accepted 后任何 candidate event 都不能提交到 Session。

### 风险 8：repair 另起 worker 再次复制旧缺陷

控制：initial/repair 使用同一 logical Controller/request id；两次 attempt 都走相同 cancellation/join contract。

## 20. 方案自审

### 20.1 是否符合用户确认

通过。正式采用方案 B：异步可取消 transport + 深 LlmRequestController，不采用 ureq 协作式假取消或 LLM 子进程方案。

### 20.2 是否解决 INC-02 全部影响

通过。覆盖 worker ownership、transport cancellation、join、禁止重叠、Session shutdown、Drop、API key/prompt/context 生命周期和报告语义。

### 20.3 是否形成深 module

通过。caller 只学习 start/poll/resolve_attempt/cancel/shutdown；Tokio、reqwest、token、task、join、reaper、secret 和 race 均隐藏在 implementation。

### 20.4 seam 是否真实

通过。LLM provider 是 true external；production Reqwest Adapter 与 controllable loopback test Adapter 构成真实 seam。

### 20.5 是否保持 238 业务链

通过。ProjectPatch schema、Import、Validator、repair scope、review、confirm 和 transaction 不变；243 只收敛请求生命周期和 transport。

### 20.6 是否诚实描述取消

通过。`Cancelled` 只表示本地 task stopped/joined/data owner released；远端执行和计费在 request 已发送后固定为 unknown。

### 20.7 是否扩大到 Provider Registry/Agent

没有。没有路由、并发队列、streaming、tools、planner、conversation memory 或自动 Apply。

### 20.8 是否有可验证 Gate

通过。connect/header/body/retry/repair、race、resubmit、shutdown/drop、privacy、report 和双 workspace 均有阻断矩阵。

### 20.9 外部审查是否完整处理

通过。INC-02 五项必须修改全部进入正式合同；其它审查项按已吸收或不适用分类，没有机械扩入。

### 20.10 是否可以生成施工文档

可以，且已按唯一施工文档完成 Gate A-F、整体回归、阶段完成记录与归档。施工文档现位于 `施工文档/已完成/243-当前可自动化施工文档-LLM-Request-Controller-Transport-Cancellation-Join-Lifecycle-v1.md`。

## 21. 正式结论

正式采用：

```text
方案 B：LLM Request Controller / Transport Cancellation / Join Lifecycle v1

Deep LlmRequestController
  + Async Reqwest rustls transport
  + Tokio CancellationToken / task ownership
  + initial + one repair shared logical lifecycle
  + Cancelling until transport stop + join
  + no resubmit before Joined terminal
  + explicit Session shutdown
  + Drop fallback reaper without detached request task
  + request-scoped prompt/context/credential lease
  + local-vs-remote honest lifecycle report v3
```

INC-02 的完成判定不是“有 cancellation token”，而是：

```text
Cancel 能中断真实 loopback transport 等待。
所有 request task 都有 owner 和 join evidence。
Cancelling 期间不能启动第二请求。
Session shutdown/Drop 不遗留 detached request task。
terminal 后 request-owned secret/prompt/context owner 已释放。
报告不泄露数据，也不声称远端计费已停止。
default/all-features workspace 均通过。
```

## 22. 后续优先级

243 讨论完成后，按 `240` 下一讨论项是 Priority 4：

```text
CQ-06 Diagnostics-first Scene Hydration / World Mutation Safety v1
```

243 已完成施工并归档。CQ-06 的 244 正式方案已确认并完成自审，但当前没有可直接施工文档；明确施工时必须先生成并自审唯一 244 施工文档。继续讨论队列时按 `240` 进入 CQ-08。

## 23. 参考

```text
框架设计/引擎总体架构/238-Real-LLM-Provider-Minimal-Repair-Loop-v1方案.md
框架设计/引擎总体架构/239-Critical-Correctness-and-Safety-Convergence-Gate-v1方案.md
框架设计/引擎总体架构/240-5.6审查剩余问题讨论与施工优先级.md
框架设计/引擎总体架构/阶段完成记录/2026-07-11-Real-LLM-Provider-Minimal-Repair-Loop-v1/00-总览.md
审查目录/5.6审查目录/01-2026-07-11-新增功能增量代码质量审查报告.md

rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/project_patch/llm_http.rs
rust/crates/editor_core/src/project_patch/llm_source.rs
rust/crates/editor_core/src/project_patch/llm_request.rs
rust/crates/editor_core/src/project_patch/llm_repair.rs
rust/crates/editor_core/src/tests/ai_service_tests.rs
rust/crates/editor_core/src/tests/llm_patch_source_tests.rs
rust/crates/editor_window_winit/src/application.rs

<USER_HOME>/zenghaoran/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ureq-3.3.0/src/agent.rs
<USER_HOME>/zenghaoran/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ureq-3.3.0/src/config.rs

https://doc.rust-lang.org/std/thread/struct.JoinHandle.html
https://docs.unity.cn/ScriptReference/Networking.UnityWebRequest.Abort.html
https://docs.godotengine.org/en/stable/classes/class_httprequest.html#class-httprequest-method-cancel-request
https://github.com/godotengine/godot/blob/master/scene/main/http_request.cpp
https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html
https://docs.rs/tokio/latest/tokio/task/struct.JoinHandle.html
https://docs.rs/reqwest/latest/reqwest/struct.Client.html
```
