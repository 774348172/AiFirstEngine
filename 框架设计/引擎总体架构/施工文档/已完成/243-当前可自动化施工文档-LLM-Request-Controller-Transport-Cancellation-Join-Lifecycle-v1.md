# 243-当前可自动化施工文档-LLM Request Controller / Transport Cancellation / Join Lifecycle v1

> 状态：Gate A-F、整体回归、完成记录和入口同步均已完成；本文已归档。  
> 正式方案：`../../243-LLM-Request-Controller-Transport-Cancellation-Join-Lifecycle-v1方案.md`。  
> 方案选择：用户已确认方案 B，采用 async cancellable transport 与深 `LlmRequestController`。  
> 方案审查：243 已对照 5.6 增量审查 INC-02、238/239/241/242、当前源码及成熟实现完成审查；施工文档预审补齐了 `resolve_attempt` 决议入口。  
> 执行规则：严格按 Gate A -> B -> C -> D -> E -> F 串行执行；Gate 失败先修当前 Gate，不跨 Gate 堆积问题。

## 1. 施工目标

关闭 INC-02：

```text
Cancel 不再只 take Session receiver 或忽略迟到结果。
每个 logical LLM request 由单一 Controller owner 管理 transport、task、join 和数据释放。
generate 与最多一次 repair 共用 request_id、credential lease 和 terminal latch。
Cancelling 期间保持 busy；只有 Joined terminal 后才能开始下一请求。
Session shutdown 与 Drop fallback 不遗留 detached request task。
llm-patch-request-report.v3 诚实区分本地停止与远端未知状态。
```

完成必须证明：

```text
connect/header/body/retry/repair 等待均可取消。
Cancel accepted 后不会产生可提交 candidate 或第二 terminal。
task stopped + joined + request owner released 前不显示 Cancelled。
显式 window/session shutdown 返回有界 receipt。
Drop fallback 将未 join handle 交给 owned reaper，不直接 detach。
prompt/context/credential 不进入 Debug/serde/report；terminal 后 owner release probe 成立。
正常 generate、一次 repair、context stale、review/confirm/apply 行为不回归。
default/all-features workspace 回归通过。
```

## 2. 施工前基线

```text
243 正式方案已由用户确认并完成方案审查。
生成本文前施工文档/当前/为空；生成后本文是唯一当前施工文档。
240 中 INC-02 讨论状态=已完成，施工状态=未施工。
238 使用 ureq blocking client + detached std::thread worker + receiver-only cancel。
239/241/242 与用户既有未提交改动必须保留，不 reset/checkout/revert。
```

当前源码证据：

```text
editor_core/Cargo.toml 直接依赖 ureq 3.3。
project_patch/llm_http.rs 同步 send/read，并在 retry 路径 thread::sleep。
services/ai_service.rs 的 generate/repair 都直接 std::thread::spawn，JoinHandle 未保存。
session.rs 的 ActiveLlmPatchRequest 保存 prompt/context/config/receiver，但无 token/task owner。
Cancel take active request 后立即写 AiPanelStage::Cancelled 和 final_status=cancelled。
ui_model_composer 只以 active_llm_patch_request.is_some() 判断 busy。
AiPanelStage 没有 Cancelling。
Native editor window close 没有 LLM shutdown receipt。
```

施工文档预审修正：

```text
原 243 interface 只有 continue_repair，无法表达“主线程判断无需 repair 后正常结束”。
正式方案已改为 resolve_attempt(request_id, Complete | Fail | ContinueRepair)。
ShutdownJoinTimedOut 不授权回到 Idle；未 join handle 继续由 reaper 持有。
```

## 3. 固定范围

### 3.1 必须交付

```text
Llm request lifecycle schema、terminal latch、receipt、diagnostics。
llm-patch-request-report.v3 与 Off/Summary/Trace 分档。
non-clone request-scoped LlmCredentialLease 和 prompt/context owner。
LlmTransport 内部 seam。
ReqwestAsyncTransport production Adapter。
ControllableLoopbackTransport deterministic test Adapter。
process-owned LlmAsyncExecutor、task inventory、reaper 和 drain。
深 LlmRequestController start/poll/resolve_attempt/cancel/shutdown interface。
generate + one repair 同一 logical lifecycle。
EditorSession、AI Panel、winit close 与 Drop fallback 接入。
真实 HTTP/1 loopback cancellation、race、privacy 与 workspace Gate。
validation-only llm-worker-lifecycle-report.v1。
```

### 3.2 禁止扩入

```text
不新增 Provider Registry、模型路由、fallback、投票或并发请求队列。
不新增 Agent Planner、tools、MCP、conversation memory 或自动 Apply。
不新增 streaming token UI 或多轮 repair。
不新增 LLM 子进程/IPC/OS sandbox。
不修改 ProjectPatch schema、typed repair scope、Validator、Review/Confirm/transaction。
不改造 Runtime、Renderer、ECS、Player 或导出包为 async/provider consumer。
不承诺远端推理、计费或 provider 数据已停止/删除。
不处理 CQ-06、CQ-07、CQ-08。
```

## 4. 跨 Gate 固定合同

### 4.1 Controller interface

```text
start(request_spec) -> RequestId | diagnostic
poll() -> ordered lifecycle events
resolve_attempt(request_id, Complete | Fail | ContinueRepair) -> result
cancel(request_id, source) -> CancelReceipt
shutdown(deadline) -> ShutdownReceipt
```

`EditorSession` 不接触 Tokio runtime、reqwest client、CancellationToken、JoinHandle、AbortHandle、reaper 或 secret storage。

### 4.2 Request ownership

```text
Controller/request task 独占 original prompt、context JSON 和 credential lease。
Session 只保存 request_id、revision/context hash、phase/attempt 和 Import/repair 业务证据。
LlmTransportConfig 可 Clone 且不含 secret。
LlmCredentialLease 不 Serialize、不明文 Debug、默认不 Clone，request start 时 move。
只证明 credential_owner_status=released，不声明第三方 buffer 全量 zeroized。
```

### 4.3 State / terminal

```text
Starting / RunningGenerate / WaitingForMainThreadDecision / RunningRepair / Cancelling 都 busy。
cancel accepted 在 terminal event commit 前发生时 cancel wins。
每个 request_id 只允许一个 terminal receipt。
CompletedJoined / FailedJoined / CancelledJoined 完成清理后才允许 Idle/new start。
ShutdownJoinTimedOut fail closed，不允许 start；handle 仍由 reaper 持有。
旧 generation event 必须被 Controller terminal latch 拒绝，不能污染新请求。
```

### 4.4 Transport

```text
Reqwest async + rustls，redirect disabled。
HTTPS required；仅 loopback 保留 HTTP。
继承 238 request/response/candidate limits、classification、timeout 和最多一次 retry。
send/connect/TLS/header、chunk body、retry timer 全部 select cancellation。
禁止 response.text().await 后检查大小；必须逐 chunk 累积并限流。
禁止 thread::sleep、ureq、blocking reqwest 或 spawn_blocking 包装 HTTP。
```

### 4.5 Cancel / shutdown deadline

第一版固定预算：

```text
Cancel command：只发 token/状态变更，不同步等待；定向测试不得出现 deadline wait。
Cancel join deadline：2 s。
EditorSession explicit shutdown deadline：5 s。
Drop local join budget：25 ms；随后必须 transfer owned handle 到 executor reaper。
Process executor/reaper drain deadline：5 s。
Provider total timeout：继续由 config 控制，默认 30 s，不能替代 cancel deadline。
```

Gate E 必须用 Windows loopback fixture 验证这些预算。若 CI 证据证明必须调整，只能先回填 243 和本文；不得靠放宽到 provider timeout 或 detach task 通过。

### 4.6 Report honesty

```text
request send 前取消：remote_execution_status=not_started。
request 可能已发送：remote_execution_status=unknown。
Cancelled 只表示本地 task stopped/joined/request owner released。
join timeout、panic、abort failure 不能伪装为普通 Cancelled。
Off 不生成 artifact；Summary 只保留 compact lifecycle；Trace 才保留 redacted transitions/timing。
```

## 5. Gate A - Lifecycle Contract / Credential / Report v3

### 5.1 施工

```text
定义 LlmRequestId、LlmRequestState、LlmRequestEvent、LlmAttemptDecision。
定义 CancelSource、CancelReceipt、ShutdownReceipt、task/local/remote/credential status。
实现 pure terminal latch 与 transition validator。
把 secret 从 cloneable transport metadata 中拆到 LlmCredentialLease。
credential owner 使用 zeroizing storage 或等价 direct dependency；Debug/serde 永不暴露明文。
升级 llm-patch-request-report.v2 -> v3，保留 v2 业务 evidence 并新增 lifecycle fields。
实现新增固定 diagnostics，Cancelled 与 transport failure 分流。
```

### 5.2 阻断测试

```powershell
cargo test -p editor_core llm_request_lifecycle -- --nocapture
cargo test -p editor_core llm_patch_request_report -- --nocapture
cargo test -p editor_core llm_credential -- --nocapture
```

### 5.3 验收

```text
所有合法/非法 transition 有 pure test。
cancel/success/failure race 只能 commit 一个 terminal。
resolve_attempt 的 Complete/Fail/ContinueRepair 均有明确状态转换。
Summary/Trace serde 不含 credential/prompt/context/Authorization/绝对路径。
Off 不产生 report artifact。
v3 schema/version 和字段命名稳定。
```

## 6. Gate B - Async Transport Adapter

### 6.1 施工

```text
在 workspace/editor_core 增加最小 reqwest、Tokio、tokio-util、zeroize direct dependencies。
reqwest 关闭默认 TLS，使用 rustls；只启用 JSON/body stream 所需 feature。
建立内部 LlmTransport seam 与 boxed async result，不向 Session public surface扩散 async。
实现 ReqwestAsyncTransport endpoint policy、request build、错误分类、timeout/retry。
逐 chunk 读取 response，并在每个 await 点 select cancellation/timeout。
实现 ControllableLoopbackTransport 的 phase barrier、cancel/drop/panic probe。
迁移或删除 llm_http.rs；移除 editor_core 对 ureq 的直接依赖。
保留 deterministic mock source，但其 attempt 也进入 Controller task/terminal 合同。
```

### 6.2 阻断测试

```powershell
cargo test -p editor_core llm_transport -- --nocapture
cargo test -p editor_core llm_transport_cancellation -- --nocapture
cargo test -p editor_core llm_patch_source -- --nocapture
```

### 6.3 验收

```text
cancel-before-send、header wait、body chunk、retry timer 均返回 llm_transport.cancelled。
body 超限在读取过程中停止，返回 llm_transport.body_limit_exceeded。
取消后不 retry、不继续读完整 body。
HTTPS/loopback/redirect/size/error/retry 合同不低于 238。
LLM production 路径不再出现 ureq、thread::sleep 或 detached thread。
```

## 7. Gate C - Deep LlmRequestController / Executor / Reaper

### 7.1 施工

```text
实现 process-owned LlmAsyncExecutor，集中 runtime、spawn、active inventory、reaper、drain。
实现 Controller start/poll/resolve_attempt/cancel/shutdown。
Controller 持有 token、task handle、attempt state、terminal latch 和 request-owned data。
attempt task 先 terminal + join，再向 Session 暴露可处理 event。
initial result 进入 WaitingForMainThreadDecision；Complete/Fail 明确结束，ContinueRepair 复用 request。
取消先 token，cancel deadline 内未停止时 abort；始终 await/join 或 transfer reaper ownership。
panic/channel disconnect/join failure/executor unavailable 形成固定 diagnostic/receipt。
实现 Drop fallback：cancel + abort + 25 ms local budget + owned reaper transfer。
executor shutdown drain active/reaper inventory；deadline 后明确失败。
```

### 7.2 阻断测试

```powershell
cargo test -p editor_core llm_request_controller -- --nocapture
cargo test -p editor_core llm_executor_reaper -- --nocapture
```

### 7.3 验收

```text
single active 在 Controller 内成立，而非只检查 Session Option。
Cancelling/decision wait 期间 start 返回 llm_request_controller.busy。
CancelledJoined 后可启动新请求，旧 event 不可提交。
generate/repair 共用 request_id、credential lease 与 terminal latch。
panic、abort failure、join timeout 不被报告为成功 Cancelled。
所有测试结束 active_task_count=0、reaper_count=0。
删除 Controller 后复杂度会重新散回多个 caller，证明 module 有 depth/locality。
```

## 8. Gate D - EditorSession / AI Panel / Window Shutdown

### 8.1 施工

```text
以 Controller handle/request metadata 替换 ActiveLlmPatchRequest receiver-only ownership。
generate command move prompt/context/credential lease 进入 Controller。
pump 消费 ordered event，在主线程执行 Import/Validator/RepairDecision。
主线程用 resolve_attempt 提交 Complete/Fail/ContinueRepair。
Cancel 只进入 Cancelling；收到 CancelledJoined 后才显示 Cancelled/message/report terminal。
AiPanelStage 增加 Cancelling；ui_model busy 读取 Controller lifecycle。
Cancelling 时禁用 Submit/Cancel 重入；Escape 重复触发返回稳定 receipt/diagnostic。
EditorSession 增加显式 shutdown_llm(deadline) 和 Drop fallback。
NativeEditorApplication/window close 先调用显式 shutdown，再退出；保存 shutdown receipt/diagnostic。
```

### 8.2 阻断测试

```powershell
cargo test -p editor_core llm_session_shutdown -- --nocapture
cargo test -p editor_core llm_cancel -- --nocapture
cargo test -p editor_ui_model ai_panel -- --nocapture
cargo test -p editor_window_winit llm_shutdown -- --nocapture
```

### 8.3 验收

```text
Cancel command 不阻塞 frame，UI 先显示 Cancelling。
Cancelling 期间 busy=true，不能提交第二请求。
joined/data released receipt 到达后才显示 Cancelled。
normal generate/repair/context stale/review/confirm/apply 不回归。
window close 有显式 shutdown receipt；直接 drop 走 fallback reaper。
Runtime/Player/export 不初始化 LLM executor/provider。
```

## 9. Gate E - Real Loopback / Race / Privacy E2E

### 9.1 施工

```text
建立 validation-only llm-worker-lifecycle-report.v1 和 project_e2e_gate 入口。
真实 HTTP/1 loopback server 提供 pre-send/connect、header stall、chunked body stall、Retry-After、repair stall。
服务端记录 disconnect/write failure/body stream end，作为本地 connection close evidence。
建立 cancel vs success/failure race barrier、重复 cancel、resubmit before/after join matrix。
建立 Session shutdown/window close/Drop/executor drain/join-timeout fixture。
建立 credential/prompt/context owner drop probe 和 Off/Summary/Trace redaction scan。
记录 command/cancel/shutdown latency 与 deadline receipt。
```

### 9.2 阻断测试

```powershell
cargo test -p project_e2e_gate llm_worker_lifecycle -- --nocapture
cargo test -p editor_core llm_cancel -- --nocapture
cargo test -p editor_core llm_privacy -- --nocapture
```

### 9.3 必过矩阵

```text
cancel before request send -> remote not_started。
cancel while connect/header/body/retry/repair -> local stopped + joined，remote unknown。
server 观察 disconnect/write failure/body end 中至少一个 phase-appropriate evidence。
cancel vs success/failure -> single terminal，无 proposal-after-cancel。
重复 Cancel 稳定；Cancelling resubmit busy；CancelledJoined 后 resubmit 成功。
shutdown/drop 后 active=0、reaper=0；注入 join timeout 必须 fail closed。
credential/prompt/context owner 在 Joined terminal 后释放。
JSON/Debug/serde 无 secret、Authorization、原 prompt/context、绝对路径。
Cancel terminal <= 2 s；session/executor drain <= 5 s；Drop local wait <= 25 ms 后转交 reaper。
```

## 10. Gate F - Dependency Audit / Full Regression / Closure

### 10.1 Source / dependency inventory

```text
复扫 editor_core LLM production 路径：std::thread::spawn、thread::sleep、mpsc receiver-only ownership。
复扫 ureq direct dependency、Cargo.lock 残留来源和 blocking HTTP。
复扫 final_status=cancelled / AiPanelStage::Cancelled 写入，禁止 join 前命中。
复扫 API key/config clone、Debug、Serialize 和 report raw data。
复扫 Runtime/Player/export 对 reqwest/Tokio LLM stack 的依赖。
cargo tree 验证 async/provider dependencies 只在 Editor LLM ownership 内。
所有命中分类；production lifecycle leftover 阻断闭环。
```

建议命令：

```powershell
rg -n "std::thread::spawn|thread::sleep|ureq|final_status.*cancelled|AiPanelStage::Cancelled|api_key" crates/editor_core crates/editor_window_winit crates/project_e2e_gate Cargo.toml Cargo.lock
cargo tree -p editor_core
cargo tree -p engine_runtime
cargo tree -p runtime_player_winit
```

### 10.2 整体回归

在 `rust/` 执行：

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

### 10.3 完成同步

```text
生成 阶段完成记录/2026-07-11-LLM-Request-Controller-Transport-Cancellation-Join-Lifecycle-v1/00-总览.md。
记录每 Gate 命令、通过数量、deadline measurement、inventory 和 report artifact。
更新 240：INC-02 当前状态=已关闭，施工状态=已完成。
更新 00-文档地图、00-阅读顺序、49、54。
更新 施工文档/README、阶段完成记录/README。
把本文从 当前/ 移到 已完成/。
确认 240 下一讨论项仍为 CQ-06。
```

## 11. Diagnostic / report 固定集合

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

llm-patch-request-report.v3
llm-worker-lifecycle-report.v1
```

保留 238 provider/repair diagnostics 与 239 typed repair scope diagnostics，不改名、不降级。

## 12. 预计涉及文件

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/editor_core/Cargo.toml

rust/crates/editor_core/src/project_patch/llm_transport.rs（新）
rust/crates/editor_core/src/project_patch/llm_request_controller.rs（新）
rust/crates/editor_core/src/project_patch/llm_http.rs（迁移后删除或只保留无阻塞 helper）
rust/crates/editor_core/src/project_patch/llm_source.rs
rust/crates/editor_core/src/project_patch/llm_request.rs
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/ai_service_tests.rs
rust/crates/editor_core/src/tests/llm_patch_source_tests.rs
rust/crates/editor_core/src/tests/fixtures.rs

rust/crates/editor_ui_model/src/ai_panel.rs
rust/crates/editor_ui_model/src/tests.rs

rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/editor_window_winit/src/tests/support.rs

rust/crates/project_e2e_gate/src/llm_worker_lifecycle.rs（新）
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
```

测试可放在对应 module 内部 `tests` 子 module；若新增独立文件，应保持上述 owner，不建立第二套 lifecycle implementation。列表外命中必须说明所有权与必要性，不能静默扩范围。

## 13. 工作树规则

```text
保留所有既有用户/239/241/242 未提交改动。
不 reset/checkout/revert unrelated changes。
不重写样例 build artifact，除非 Gate 明确要求且完成记录说明原因。
一次只执行本文；不同时施工 CQ-06/CQ-07/CQ-08。
实现与 243 冲突时先暂停并回填正式方案/本文，不缩成 receiver-only 或 cooperative fake cancel。
每个 Gate 测试通过后才进入下一 Gate。
```

## 14. 外部审查结论处理

### 14.1 必须修改

```text
detached thread / 无 JoinHandle-token -> Gate B/C。
Cancel 只 take receiver -> Gate C/D/E。
Cancel 后可启动第二请求 -> Gate C/D/E single-active matrix。
Session/editor 关闭后 worker/secret 存活 -> Gate A/C/D/E。
ai_service 状态/网络/evidence 分散 -> Gate B/C 深 module。
```

### 14.2 施工约束

```text
真实 transport cancellation、join、Drop、owner release 和双 workspace 均为阻断项。
本地取消不能声称远端推理/计费停止。
ureq 无 public abort，必须由 async transport 取代，不能只加 AtomicBool。
```

### 14.3 已由历史施工吸收

```text
INC-01 typed repair scope 由 239 关闭，本文只回归。
CQ-03/CQ-05/INC-03 由 239 关闭，不重复实现。
CQ-04 由 241 关闭，不触碰 project write containment。
CQ-01 由 242 关闭，不触碰 ProjectRuntimeModule。
238 HTTPS/limits/classification/one retry/one repair/redaction/confirm 继续有效。
```

### 14.4 不适用

```text
CQ-06/CQ-07/CQ-08 由 240 后续独立讨论与施工。
Provider Registry、Agent、streaming、远端 cancellation protocol 不属于 INC-02。
```

## 15. 施工文档自审

### 15.1 与正式方案一致

通过。Gate A-F 覆盖 async transport、Controller、join/reaper、Session shutdown、request-owned data、Report v3 和双 workspace；没有改回 ureq 协作式假取消或扩入子进程方案。

### 15.2 方案缺口是否先修正

通过。预审发现 `WaitingForMainThreadDecision` 缺少无需 repair 的 terminal 入口，已先把 243 改为 `resolve_attempt(Complete | Fail | ContinueRepair)`；同时明确 `ShutdownJoinTimedOut` 不可回 Idle。施工文档不再依赖隐式 drop 结束 request。

### 15.3 外部审查是否完整处理

通过。INC-02 的 worker ownership、真实取消、禁止重叠、shutdown/drop、secret/prompt/context 生命周期和集中 request controller 全部分配到 Gate；其它审查项按历史吸收或不适用分类。

### 15.4 是否形成深 module

通过。caller 只学习 start/poll/resolve_attempt/cancel/shutdown；Tokio、reqwest、token、task、join、reaper、secret、retry 和 race 都隐藏在 implementation。production/test 两个 Adapter 使 transport seam 真实。

### 15.5 Cancel 语义是否诚实

通过。Cancelled 以 stopped + joined + owner released 为前提；请求可能发送后 remote 固定 unknown；join timeout fail closed，不承诺停止计费或远端推理。

### 15.6 测试是否覆盖完整目标

通过。每 Gate 有定向命令；Gate E 覆盖 connect/header/body/retry/repair、race、resubmit、shutdown/drop、deadline、connection close 和 privacy；Gate F 执行依赖审计与双 workspace。

### 15.7 deadline 是否可能靠 detach 通过

已阻断。Cancel 2 s、shutdown/drain 5 s、Drop 25 ms 是独立预算；超时必须保留 handle ownership 并返回失败，不得放宽到 provider timeout 或丢弃 JoinHandle。

### 15.8 是否扩大范围

没有。ProjectPatch、repair scope、Runtime/Player、Provider Registry、Agent、streaming、CQ-06/07/08 都明确排除。

### 15.9 是否可以直接施工

可以。当前目录只有本文一份施工文档，正式方案缺口已回填，自审无阻断。结论：`通过；用户明确开始施工后从 Gate A 开始，当前 Gate 失败时停止并修复，不跨 Gate。`
