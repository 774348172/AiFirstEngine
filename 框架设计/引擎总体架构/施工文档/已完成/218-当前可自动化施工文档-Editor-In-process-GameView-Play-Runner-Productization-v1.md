# 218-当前可自动化施工文档：Editor In-process GameView Play Runner Productization v1

## 1. 施工目标

按 `218-Editor-In-process-GameView-Play-Runner-Productization-v1方案.md` 的 `A-min-gameview` 落地第一版可验证链路：

```text
Editor Play 使用 217 Preview RuntimePackage。
PlaySessionMode::WindowedUserRun / EditorGameView runner 不再返回 not implemented。
Editor 进程内创建 EditorRuntimePlayInstance。
RuntimePlayInstance 只从 RuntimePackage load，不读项目源目录和 editor unsaved memory。
RuntimePlayInstance hydrate active scene 并 tick frame_count > 0。
RuntimePlayInstance 输出 GameViewRuntimeFrame descriptor evidence。
GameViewPresentReport 写出并链接到 PlaySessionReport。
Report Panel 增加 game_view_present provider。
GameView model 能看到 runtime frame descriptor。
Stop 能清理 runtime instance。
project_e2e_gate 证明 complex shooter sample 走过 218 链路。
```

本施工必须吸收 `其它AI审查目录/36-218-Editor-In-process-GameView-Play-Runner方案审查.md`：

```text
不复用 runtime_player_winit::run_windowed_native_player_from_package 作为内嵌 GameView。
不创建第二个 winit EventLoop。
不声称完成真实 GPU texture present。
A-min 只做 descriptor evidence。
Gate A 增加 PlayRunner trait 或等价 runner 抽象。
A1 集成 ProjectUiStateSnapshotProducer + font atlas AUI present summary。
A1 不接 GameView input，GameView input / AUI interaction 留到 A3。
Gate E 断言 descriptor evidence，而不是断言真实画面。
```

## 2. 涉及文件

预计新增：

```text
rust/crates/editor_core/src/editor_gameview_play.rs
```

预计修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/play_session.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/tests/play_service_tests.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/218-Editor-In-process-GameView-Play-Runner-Productization-v1方案.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-07-Editor-In-process-GameView-Play-Runner-Productization-v1/00-总览.md
```

禁止修改/禁止实现：

```text
不要让 Runtime 读取项目源目录。
不要让 Runtime 读取 EditorSession scene/AUI/rule 内存绕过 RuntimePackage。
不要把 runtime_player_winit 加成 editor_core 依赖。
不要在 editor 内启动 runtime_player_winit 独立 EventLoop。
不要把 HeadlessGate report 冒充 GameView runner。
不要把 TestTexture placeholder 冒充 runtime frame。
不要实现真实 GPU texture present。
不要实现完整 GameView input / AUI interaction。
不要实现 Pause / Step / Maximize on Play。
不要修改与本施工无关的既有工作区改动。
```

## 3. Gate A：schema / report / runner 抽象

任务：

```text
新增 EditorRuntimePlayRequest。
新增 EditorRuntimePlayState。
新增 GameViewRuntimeFrame。
新增 GameViewPresentReport。
新增 EditorGameViewPlayRunner。
新增 PlayRunner trait 或等价 runner 抽象。
DefaultGameRunOrchestrator 作为 HeadlessGate runner。
EditorGameViewPlayRunner 作为 EditorGameView runner。
PlaySessionReport 增加 runner_kind / game_view_present_report_path / game_view_frame_count / game_view_last_frame_hash。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_gameview_play_schema
cargo test -p editor_core play_session
```

## 4. Gate B：EditorRuntimePlayInstance

任务：

```text
实现 EditorRuntimePlayInstance::start。
load RuntimePackage。
hydrate active scene into world。
tick no-input frame_count。
集成 ProjectUiStateSnapshotProducer + font atlas AUI present summary。
输出 GameViewRuntimeFrame：
  frame_count > 0
  frame_hash
  renderable_count
  ui_draw_item_count
  texture_descriptor_status=descriptor_only
  input_bridge_status=deferred
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_runtime_play_instance
cargo test -p engine_runtime engine_host_loop
```

## 5. Gate C：PlayService / Stop 集成

任务：

```text
EditorSession::start_play_session 继续先走 217 PreviewPackageService。
项目已打开时默认走 PlaySessionMode::WindowedUserRun / runner_kind=editor_in_process_gameview。
使用 prepared runtime_package_path 创建 EditorRuntimePlayInstance。
成功时写 GameViewPresentReport 并回填 PlaySessionReport。
失败时报告 preview / load / hydrate / tick / present 层级。
EditorSession::stop_play_session 清理 EditorRuntimePlayInstance。
```

测试：

```powershell
cd rust
cargo test -p editor_core play_service
cargo test -p editor_core editor_gameview_play_runner
```

## 6. Gate D：GameView model / Report Panel

任务：

```text
EditorSession 保存 last_game_view_runtime_frame / last_game_view_present_report。
ui_model_composer 将 last_game_view_runtime_frame 写入 ViewportModel texture_id / target_id / frame / frame_hash。
Report Panel 新增 game_view_present provider。
Console 输出 game view report path / frame_count / descriptor_only。
```

测试：

```powershell
cd rust
cargo test -p editor_core ui_model
cargo test -p editor_core report_panel
cargo test -p editor_window_winit viewport_runtime
```

## 7. Gate E：project_e2e_gate

任务：

```text
新增 editor_gameview_play_runner gate。
打开 samples/complex_shooter_project。
触发 EditorSession::start_play_session。
验证 PreviewPackageReport 存在。
验证 PlaySessionReport runner_kind=editor_in_process_gameview。
验证 GameViewPresentReport 存在。
验证 GameViewRuntimeFrame frame_count > 0 / frame_hash / renderable_count / ui_draw_item_count。
验证 Stop 清理 runtime instance。
写 complex-shooter-editor-gameview-play-runner-productization-report.json。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate editor_gameview_play_runner
```

## 8. 整体回归

全部 Gate 完成后运行：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_core editor_gameview_play
cargo test -p editor_core play_service
cargo test -p editor_core report_panel
cargo test -p editor_window_winit viewport_runtime
cargo test -p project_e2e_gate editor_gameview_play_runner
cargo test -p project_e2e_gate
cargo test -p editor_core
```

如整体回归暴露与本施工无关的既有失败，必须在最终报告中说明；不得为通过测试回退无关改动。

## 9. 施工文档自审

范围自审：

```text
本施工只做 A-min-gameview。
本施工不做真实 GPU texture present。
本施工不接完整 GameView input。
本施工不接 AUI interaction in editor GameView。
本施工不做 Pause / Step / Maximize on Play。
本施工不把 runtime_player_winit 接入 editor_core。
```

36 号审查吸收自审：

```text
run_windowed 独立 EventLoop 不可内嵌：已列为禁止实现。
无 render target sharing：A-min 只验 descriptor evidence。
无 runner trait：Gate A 明确新增 PlayRunner trait 或等价 runner 抽象。
AUI present 统一：Gate B 明确集成 ProjectUiStateSnapshotProducer + font atlas AUI present summary。
input 冲突：Gate B 明确 no-input tick，A3 后续再接。
e2e 验证：Gate E 明确断言 descriptor evidence，不断言真实画面。
```

测试自审：

```text
每个 Gate 都有 cargo test 命令。
WindowedUserRun 不再允许返回 not implemented。
GameView runtime frame 不能用 TestTexture placeholder 冒充。
Stop 必须清理 runtime instance。
整体回归包含 editor_core、editor_window_winit、project_e2e_gate。
```

## 10. 完成后文档同步与归档

完成后必须：

```text
生成阶段完成记录：
  框架设计/引擎总体架构/阶段完成记录/2026-07-07-Editor-In-process-GameView-Play-Runner-Productization-v1/00-总览.md

更新入口：
  框架设计/引擎总体架构/49-当前状态与下一步入口.md
  框架设计/引擎总体架构/54-当前可自动化施工总入口.md
  框架设计/引擎总体架构/施工文档/README.md
  框架设计/引擎总体架构/阶段完成记录/README.md

归档施工文档：
  从 施工文档/当前/
  移到 施工文档/已完成/
```
