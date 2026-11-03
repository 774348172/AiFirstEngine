# 84-当前可自动化施工文档：Editor Play / Run Session System C-min

## 目标

按照 `84-Editor-Play-Run-Session-System方案.md` 实现第一版 Editor Play / Run Session System。

目标不是重写 Runtime、Build、Scene、Asset、Renderer，也不是做完整 UE 式 In-Process PIE，而是把编辑器中的 Run / Stop 收敛成一套清晰的会话编排层：

```text
Toolbar Run / Stop
  -> PlaySessionRequest / StopRequest
  -> PlaySessionController
  -> DefaultGameRunRequest
  -> DefaultGameRunOrchestrator Headless path
  -> PlaySessionReport
  -> Toolbar / Console / RuntimeTrace 最小反馈
```

## 当前基础

必须复用：

```text
83-真实默认WindowedEndToEndGameGate方案.md
84-Editor-Play-Run-Session-System方案.md

engine_runtime::default_game_run:
  DefaultGameRunRequest
  DefaultGameRunMode
  DefaultGameRunOrchestrator
  EndToEndGameRunReport

editor_core:
  EditorSession
  UiCommandPayload::Play / Pause / StepFrame / TickOneFrame
  ToolbarModel / RuntimeRunState
  ConsoleModel / RuntimeTraceModel

editor_input:
  Toolbar command routing
```

## 第一版边界

本轮必须实现：

```text
PlaySessionRequest
PlaySessionMode
PlaySessionState
PlaySessionReport
PlaySessionController
StopRequest 或等价 Stop API
EditorSession 接入 PlaySessionController headless path
Toolbar Play / Stop 状态最小接入
Console / RuntimeTrace 最小反馈
单元测试与 headless smoke test
```

本轮不实现：

```text
完整 UE 式 In-Process PIE。
嵌入式 runtime window 到 editor viewport。
真实 windowed runtime process 生命周期托管。
远程设备运行。
多实例运行。
Play Mode hot reload。
断点调试协议。
运行时 live inspect。
暂停 / 单帧 step 的完整 PlaySession 语义。
```

说明：

```text
WindowedUserRun 的 request / state / report 类型可以先定义。
WindowedUserRun 的真实执行第一版返回明确 diagnostic：windowed_session_not_implemented_in_c_min。
HeadlessGate 必须真实调用 DefaultGameRunOrchestrator。
```

## 施工原则

```text
不做第二套 Runtime。
不复制 Build / Run Orchestrator。
不让 Runtime 知道 Toolbar。
不让 Viewport / Game View 启动 Runtime。
不让项目逻辑依赖 Play Session。
不把 headless 做成第二套游戏逻辑。
```

允许新增：

```text
editor_core::play_session 模块或等价模块。
PlaySessionController 持有当前 session 状态。
PlaySessionReport 包装 EndToEndGameRunReport。
EditorSession 中由 PlaySessionController 负责 Play / Stop。
Toolbar command 从 play/pause 语义逐步收敛到 run/stop 语义。
```

## 建议代码位置

优先位置：

```text
rust/crates/editor_core/src/play_session.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_input/src/lib.rs
```

必要时调整：

```text
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_ui_backend_egui/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
```

但第一版尽量少碰 UI renderer，只要现有 toolbar command 能触发即可。

## A1：新增 PlaySession 数据结构

新增类型：

```text
PLAY_SESSION_REPORT_SCHEMA_VERSION = "play-session-report.v1"

PlaySessionMode
  HeadlessGate
  WindowedUserRun

PlaySessionState
  Idle
  Preparing
  Building
  StagingPackage
  Launching
  Running
  Stopping
  Completed
  Failed

PlaySessionRequestedBy
  Toolbar
  Automation
  EditorCommand

PlaySessionRequest
  session_id
  mode
  project_root
  runtime_package_path
  scene_ref optional
  build_profile optional
  run_profile optional
  frame_limit
  report_path optional
  requested_by

PlaySessionProcessSummary
  launch_requested
  pid optional
  exit_code optional
  status

PlaySessionReport
  schema_version
  session_id
  mode
  state
  request_summary
  build_summary
  runtime_report optional EndToEndGameRunReport
  process_summary
  diagnostics
  started_at_frame optional
  ended_at_frame optional
```

第一版字段可以用字符串和 `PathBuf`，不引入复杂时间系统。`started_at_frame / ended_at_frame` 可以先用 editor revision 或 frame 计数。

测试：

```text
play_session_request_defaults_to_headless_gate
play_session_report_is_json_serializable
play_session_report_wraps_end_to_end_report
play_session_report_keeps_editor_session_separate_from_runtime_report
```

运行：

```powershell
cargo test -p editor_core play_session_request
cargo test -p editor_core play_session_report
```

## A2：实现 PlaySessionController C-min

新增：

```text
PlaySessionController
  current_state
  current_session_id optional
  last_report optional
  queued_request optional
  queued_stop optional
```

核心 API：

```text
PlaySessionController::new()
PlaySessionController::queue_start(request)
PlaySessionController::queue_stop(session_id optional)
PlaySessionController::drain_queued_with_runtime(default_game_runner)
PlaySessionController::run_headless(request)
PlaySessionController::run_windowed_smoke(request)
PlaySessionController::last_report()
PlaySessionController::state()
```

C-min 规则：

```text
queue_start 不立即执行，只记录 request。
drain_queued_with_runtime 代表 editor stable point。
HeadlessGate 调用 DefaultGameRunOrchestrator::run(DefaultGameRunRequest)。
WindowedUserRun 不真实 spawn process，生成明确 diagnostic。
Stop 只停止当前 session；C-min 如果 headless 已同步完成，则 stop 生成 no_active_session 或 already_completed diagnostic。
```

测试：

```text
play_session_controller_queues_start_without_running_immediately
play_session_controller_runs_headless_at_stable_point
play_session_controller_rejects_second_session_while_running
play_session_controller_windowed_returns_explicit_c_min_diagnostic
play_session_controller_stop_only_targets_current_session
```

运行：

```powershell
cargo test -p editor_core play_session_controller
```

## A3：接入 EditorSession Play / Stop

现状：

```text
EditorSession 里有 playing: bool。
UiCommandPayload::Play 直接 set_playing(true)。
UiCommandPayload::Pause 直接 set_playing(false)。
```

本轮改造：

```text
EditorSession 新增 play_session_controller 字段。
UiCommandPayload::Play 改为构造 PlaySessionRequest HeadlessGate 并 queue_start + drain。
UiCommandPayload::Pause 在 C-min 中作为 Stop 处理，queue_stop + drain。
如果没有 runtime_package_path，Play 返回清晰 diagnostic：play_session_requires_runtime_package。
Play 成功后 console 增加 session completed / failed 摘要。
Play 成功后 runtime_trace 尽量显示 runtime_report 的 trace summary；如果无法直接接入 trace，至少 console / diagnostics 有 report path 或 run id。
```

注意：

```text
不要删除 TickOneFrame / StepFrame 的已有能力。
不要把 PlaySessionController 和 FrameLoop 手动 tick 混为一体。
PlaySession 的 headless path 走 DefaultGameRunOrchestrator，不直接调用 frame_loop。
```

测试：

```text
editor_session_play_requires_runtime_package
editor_session_play_runs_headless_session_when_package_loaded
editor_session_stop_without_active_session_reports_diagnostic
editor_session_play_adds_console_summary
editor_session_play_does_not_bypass_default_game_run_orchestrator
```

运行：

```powershell
cargo test -p editor_core editor_session_play
```

## A4：Toolbar / UI Model 最小状态接入

目标：

```text
Toolbar 仍可保留 Play / Pause label，但内部语义改为 Run / Stop。
RuntimeRunState 第一版可以扩展，也可以先映射到现有 NoPackage / Paused / Playing。
```

推荐最小改动：

```text
如果无 runtime package -> RuntimeRunState::NoPackage。
如果 PlaySessionController state 是 Running / Launching / Preparing -> RuntimeRunState::Playing。
如果 Idle / Completed / Failed -> RuntimeRunState::Paused。
```

如改 UI model，则允许扩展：

```text
RuntimeRunState
  NoPackage
  Ready
  Running
  Stopping
  Failed
```

但要同步所有 fixture / renderer / input 测试。

测试：

```text
toolbar_state_no_package_before_package_open
toolbar_state_ready_after_package_open
toolbar_play_command_maps_to_play_session_start
toolbar_pause_command_maps_to_play_session_stop
```

运行：

```powershell
cargo test -p editor_ui_model
cargo test -p editor_input toolbar
cargo test -p editor_core toolbar
```

## A5：Console / RuntimeTrace / Report 最小反馈

Console 必须能区分失败层：

```text
request
build
stage
launch
runtime
package
scene
logic
render
window
```

第一版至少输出：

```text
Play session started: <session_id>
Play session completed: <session_id>
Play session failed: <session_id> layer=<layer> code=<code>
```

RuntimeTrace 第一版要求：

```text
如果 EndToEndGameRunReport 有 frames_completed，则 RuntimeTrace 面板能间接看到最近运行摘要。
如果当前数据结构不适合放 report，则可以先通过 Console + diagnostics 完成 C-min。
```

测试：

```text
play_session_failure_creates_console_error_with_layer
play_session_success_creates_console_info_with_frame_count
play_session_report_is_available_after_command
```

运行：

```powershell
cargo test -p editor_core play_session_console
```

## A6：Headless smoke 与 workspace 测试

必须运行：

```powershell
cargo test -p engine_runtime default_game_run
cargo test -p runtime_cli
cargo test -p editor_core play_session
cargo test -p editor_input toolbar
cargo test --workspace
```

如果 `cargo test --workspace` 因环境或 GUI feature 失败，必须记录失败原因和已通过的更小测试命令。

## A7：完成后文档同步

施工完成后必须：

```text
1. 将本文档移动到 施工文档/已完成/。
2. 新增 阶段完成记录/2026-06-28-EditorPlayRunSessionSystem-C-min/00-总览.md。
3. 更新 49-当前状态与下一步入口.md。
4. 更新 54-当前可自动化施工总入口.md。
5. 更新 18-长期主义实现路径.md。
6. 更新 00-文档地图.md。
7. 更新 施工文档/README.md。
8. 更新 阶段完成记录/README.md。
```

阶段记录必须包含：

```text
实现的类型和模块。
Play / Stop 当前真实能力。
没有实现的能力。
测试命令和结果。
下一步建议。
```

## 完成判定

满足以下条件才算 84 C-min 完成：

```text
PlaySessionRequest / PlaySessionState / PlaySessionReport 已存在并可序列化。
PlaySessionController 能 queue start / stop。
HeadlessGate 能通过 DefaultGameRunOrchestrator 运行并生成 PlaySessionReport。
WindowedUserRun 有明确 C-min diagnostic，不是假成功。
EditorSession 的 Play / Pause 命令不再只是改 playing bool，而是进入 PlaySessionController。
Console 至少能看到成功 / 失败摘要。
最小单元测试通过。
```

## 明确不算完成的情况

```text
只写类型，没有接入 EditorSession。
Play 仍然只是 set_playing(true)。
HeadlessGate 没有调用 DefaultGameRunOrchestrator。
WindowedUserRun 静默成功但没有真实运行或 diagnostic。
没有测试。
没有文档同步。
```
