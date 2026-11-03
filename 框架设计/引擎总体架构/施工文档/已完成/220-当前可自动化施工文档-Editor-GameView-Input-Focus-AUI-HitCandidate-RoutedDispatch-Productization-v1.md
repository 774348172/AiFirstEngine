# 220-当前可自动化施工文档：Editor GameView Input / Focus / AUI HitCandidate RoutedDispatch Productization v1

## 1. 施工目标

按 `220-Editor-GameView-Input-Focus-AUI-HitCandidate-RoutedDispatch-Productization-v1方案.md` 的 `B+-min` 落地第一版可测试链路：

```text
EditorInputEvent
  -> GameView focus / hover gate
  -> GameView-local RuntimeInputFrame
  -> EditorRuntimePlayInstance tick with input
  -> AuiInteractionSystem::process_with_state
  -> filter_consumed_events
  -> InputResolver / gameplay fallback
  -> GameViewPresentReport Summary evidence
```

本轮不新增完整 `HitCandidate / TargetPath / RoutedDispatch` 平行系统，不实现 WorldPickCollector，不修改 `RenderFrameReportLevel`。

## 2. 审查吸收

已读取 `其它AI审查目录/38-220-Editor-GameView-Input-Focus-AUI-HitCandidate-RoutedDispatch方案审查.md`，本施工必须吸收：

1. `MF-1`：220 的 `Off / Summary / Trace` 只约束 GameView input bridge / HitTraceReport，不替换 `RenderFrameReportLevel { Off, Stats, Summary, Evidence }`；`Trace` 档位不等于 `RuntimeTrace`。
2. `MF-2`：220-min 复用 `AuiInteractionSystem::process_with_state`，不得新增平行第二套 AUI 命中系统。
3. `SF-1`：本轮输入转换只保证 pointer + keyboard C-min；wheel / text / IME / gamepad 如未做必须明确 deferred。
4. `SF-1`：pointer 坐标必须从 window 坐标转换为 GameView-local/runtime 坐标，不能静默传 window 坐标。
5. `SF-2`：测试默认 headless deterministic，真实 winit window smoke 只 optional / ignored。

方案文档已回填上述裁决。

## 3. 涉及文件

预计修改：

```text
rust/crates/editor_window_winit/src/input_route.rs
rust/crates/editor_window_winit/src/tests/input_runtime_loop.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/tests/play_service_tests.rs
rust/crates/project_e2e_gate/src/editor_gameview_play.rs
框架设计/引擎总体架构/220-Editor-GameView-Input-Focus-AUI-HitCandidate-RoutedDispatch-Productization-v1方案.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-08-Editor-GameView-Input-Focus-AUI-HitCandidate-RoutedDispatch-Productization-v1/00-总览.md
```

如实现中发现不需要修改某个预计文件，不强行触碰。

## 4. Gate A：GameView-local 输入转换

任务：

1. 在 `ViewportInputGateway` 中把 pointer window 坐标转换为 GameView-local/runtime 坐标。
2. RuntimeInputFrame 中的 pointer event 和 pointer_position 必须使用 local 坐标。
3. route reason / summary 必须能说明坐标转换已生效。
4. keyboard 事件继续只在 GameView focused 时进入 RuntimeInputFrame。
5. wheel / text / IME / gamepad 继续 deferred，不在本 Gate 偷做。

测试：

```powershell
cargo test -p editor_window_winit game_view_pointer_coordinates_are_transformed_to_local_runtime_space
cargo test -p editor_window_winit game_view_focused_key_down_routes_to_runtime_input_frame
cargo test -p editor_window_winit game_view_unfocused_key_down_is_ignored
```

## 5. Gate B：EditorRuntimePlayInstance input tick

任务：

1. 为 Editor GameView runtime tick 增加可选 `RuntimeInputFrame` 输入。
2. 复用 `AuiRuntimePresenter` 的 resolved document / layout，调用 `AuiInteractionSystem::process_with_state`。
3. 用 `AuiInteractionResult.consumed_event_indices` 过滤 RuntimeInputFrame。
4. 剩余输入进入 `InputResolver::resolve`，生成 `ActionSnapshot` 和 `InputTraceSummary`。
5. 将 `ActionSnapshot`、`InputTraceSummary`、`AuiInteractionResult` 写入 `EngineFrameInput`。
6. `input_bridge_status` 从 `deferred` 变为真实状态：
   - `not_requested`：本帧无 runtime input。
   - `runtime_input_frame`：有输入且未被 AUI 消费。
   - `runtime_input_frame_filtered_by_aui`：有输入且 AUI 消费了部分或全部事件。
7. Summary evidence 只写轻量计数，不新增默认 HitTraceReport 文件。

测试：

```powershell
cargo test -p editor_core editor_gameview_input_tick_filters_aui_click_before_gameplay
cargo test -p editor_core editor_gameview_input_tick_allows_gameplay_when_aui_misses
cargo test -p editor_core editor_runtime_play_instance_runs_descriptor_frames
```

## 6. Gate C：EditorSession / e2e report 接入

任务：

1. `EditorSession` 暴露 active GameView runtime input tick API。
2. `project_e2e_gate` 的 Editor GameView Play report 不再要求 `input_bridge_status == deferred`，改为验证 input bridge 真实状态。
3. 保留 real window smoke 为 optional / ignored，不作为默认 CI。

测试：

```powershell
cargo test -p editor_core gameview
cargo test -p project_e2e_gate editor_gameview
```

## 7. Gate D：整体回归与文档同步

任务：

1. 跑格式和相关回归。
2. 生成阶段完成记录。
3. 更新 `49` / `54` / `施工文档/README.md` / `阶段完成记录/README.md`。
4. 将本文档从 `施工文档/当前/` 移动到 `施工文档/已完成/`。

测试：

```powershell
cargo fmt --check
cargo test -p editor_window_winit input_runtime_loop
cargo test -p editor_core gameview
cargo test -p project_e2e_gate editor_gameview
```

## 8. 施工自审

本施工文档自审结论：

1. 范围没有扩展到 WorldPickCollector、EditorOverlayCollector、完整 routed dispatch、wheel/text/IME/gamepad bridge。
2. 不修改 `RenderFrameReportLevel`，不引入 report 档位统一大工程。
3. 不新增平行 AUI router，复用 `AuiInteractionSystem`。
4. 每个 Gate 都有 headless deterministic 测试命令。
5. 完成后必须跑整体回归并同步入口文档，不能只改代码。

可以开始施工。
