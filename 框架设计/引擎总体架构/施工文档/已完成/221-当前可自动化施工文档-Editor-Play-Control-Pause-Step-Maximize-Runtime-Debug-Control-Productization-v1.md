# 221-当前可自动化施工文档-Editor Play Control / Pause-Step-Maximize / Runtime Debug Control Productization v1

## 1. 施工目标

按 `221-Editor-Play-Control-Pause-Step-Maximize-Runtime-Debug-Control-Productization-v1方案.md` 的 B-min 实现：

```text
Editor GameView Play Control B-min
  -> Pause 不再 Stop
  -> Play 在 Paused 时 Resume
  -> StepFrame 推进 active GameView runtime exactly one frame
  -> StopPlaySession / Stop 显式释放 active runtime instance
  -> Maximize on Play 进入 Editor view model / command / report
  -> Report 提供 compact play control evidence
```

本轮只控制已有 `EditorRuntimePlayInstance`，不新增 Debugger 大系统，不实现 time scale、breakpoint、timeline、input recording、OS fullscreen、外部 GameView 窗口、多实例 Play。

## 2. 审查文档判断

已读取：

```text
其它AI审查目录/38-220-Editor-GameView-Input-Focus-AUI-HitCandidate-RoutedDispatch方案审查.md
```

判断：

```text
审查对象是 220，不是 221。
220 已完成并归档，完成记录显示已吸收审查重点：
  复用 AuiInteractionSystem::process_with_state，不新增平行 AUI router。
  AUI consumed 后过滤 RuntimeInputFrame，再进入 InputResolver。
  input_bridge_status 输出真实状态。
  WorldPickCollector / EditorOverlayCollector / wheel/text/IME/gamepad bridge / 完整 HitTraceReport 仍 deferred。

对 221 的约束：
  221 不重新打开 220 的命中 / 路由系统。
  221 只控制 active EditorRuntimePlayInstance 是否推进 tick。
  221 report 遵守 runtime/editor 分档，runtime 默认 Off 或 compact result。
  221 测试优先 headless deterministic gate。
```

结论：不需要根据 38-220 推翻 221 方案；已在 221 正式方案中补充“对 38-220 审查文档的判断”。

## 3. 涉及文件

预计涉及：

```text
G:\gameEngin\.agents\skills\ai-first-game-engine-project\SKILL.md
C:\Users\zenghaoran\.codex\skills\ai-first-game-engine-project\SKILL.md
G:\gameEngin\框架设计\引擎总体架构\221-Editor-Play-Control-Pause-Step-Maximize-Runtime-Debug-Control-Productization-v1方案.md
G:\gameEngin\框架设计\引擎总体架构\00-文档地图.md
G:\gameEngin\框架设计\引擎总体架构\49-当前状态与下一步入口.md
G:\gameEngin\框架设计\引擎总体架构\54-当前可自动化施工总入口.md
G:\gameEngin\框架设计\引擎总体架构\施工文档\README.md
G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\README.md
G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-08-Editor-Play-Control-Pause-Step-Maximize-Runtime-Debug-Control-Productization-v1\00-总览.md

G:\gameEngin\rust\crates\editor_ui_model\src\command.rs
G:\gameEngin\rust\crates\editor_ui_model\src\workspace.rs
G:\gameEngin\rust\crates\editor_ui_model\src\toolbar.rs
G:\gameEngin\rust\crates\editor_ui_model\src\model.rs
G:\gameEngin\rust\crates\editor_ui_model\src\lib.rs
G:\gameEngin\rust\crates\editor_core\src\editor_gameview_play.rs
G:\gameEngin\rust\crates\editor_core\src\services\play_service.rs
G:\gameEngin\rust\crates\editor_core\src\session.rs
G:\gameEngin\rust\crates\editor_core\src\ui_model_composer.rs
G:\gameEngin\rust\crates\editor_core\src\editor_command_registry.rs
G:\gameEngin\rust\crates\editor_core\src\report_panel.rs
G:\gameEngin\rust\crates\editor_window_winit\src\command_system.rs
G:\gameEngin\rust\crates\editor_window_winit\src\application.rs
G:\gameEngin\rust\crates\editor_window_winit\src\real_window.rs
G:\gameEngin\rust\crates\project_e2e_gate\src\editor_gameview_play.rs
G:\gameEngin\rust\crates\project_e2e_gate\src\lib.rs
G:\gameEngin\rust\crates\project_e2e_gate\src\tests.rs
```

实际施工时如发现文件范围更窄，按更窄范围实现；如需扩大范围，必须先回填本施工文档。

## 4. Gate 任务与测试

### Gate A：Skill 与方案审查固化

任务：

```text
1. 在项目 skill 中加入“外部审查文档处理流程”。
2. 同步用户目录同名 skill 副本。
3. 在 221 方案中写明 38-220 审查文档判断。
4. 确认施工文档/当前 只有本文件。
```

测试：

```powershell
Select-String -LiteralPath G:\gameEngin\.agents\skills\ai-first-game-engine-project\SKILL.md -Pattern "外部审查文档处理流程"
Select-String -LiteralPath C:\Users\zenghaoran\.codex\skills\ai-first-game-engine-project\SKILL.md -Pattern "外部审查文档处理流程"
Select-String -LiteralPath G:\gameEngin\框架设计\引擎总体架构\221-Editor-Play-Control-Pause-Step-Maximize-Runtime-Debug-Control-Productization-v1方案.md -Pattern "对 38-220 审查文档的判断"
```

### Gate B：Command / UI model 基础

任务：

```text
1. 新增 StopPlaySession command payload。
2. 新增 SetGameViewMaximizeOnPlay 或 ToggleGameViewMaximizeOnPlay command payload。
3. toolbar / workspace / command registry / command_system 登记新命令。
4. 新增 EditorGameViewLayoutState 或等价 view model 字段。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_ui_model command
cargo test -p editor_core editor_command_registry
```

### Gate C：EditorRuntimePlayInstance PlayControl

任务：

```text
1. 产品化 EditorRuntimePlayState / control_state 字段进入 GameViewPresentReport。
2. 实现 pause / resume / step_next_frame。
3. Paused 普通 tick 不推进 EngineHostLoop，只复用 last_frame。
4. StepFrame 在 Paused 时 exactly +1 frame，完成后回到 Paused。
5. report 写明 control_command、runtime_advanced、paused_last_frame_reused、target_runtime_domain。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core gameview
```

### Gate D：EditorSession 命令语义

任务：

```text
1. Play 在 Paused 时 Resume。
2. Pause 不再 Stop。
3. StepFrame 优先控制 active GameView runtime；不存在 active GameView 时才可走旧 RuntimePackage debug tick。
4. StopPlaySession 显式 stop_play_session。
5. EditorSession 暴露 pause/resume/step active GameView runtime API。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core play_service
cargo test -p editor_core gameview
```

### Gate E：Maximize on Play ViewModel

任务：

```text
1. EditorSession 保存 maximize_on_play / is_game_view_maximized / reason / restore region。
2. SetGameViewMaximizeOnPlay 或 ToggleGameViewMaximizeOnPlay 可读写偏好。
3. Play started 且 maximize_on_play=true 时更新 view model。
4. Stop 后恢复自动最大化状态。
5. 不触碰 RuntimeRenderer / RHI / OS fullscreen。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_ui_model
cargo test -p editor_core gameview
```

### Gate F：Report Panel / E2E Gate

任务：

```text
1. Report Panel 展示 play control summary 字段。
2. project_e2e_gate 新增 complex-shooter editor play control report。
3. 覆盖 Play -> Pause -> Step -> Resume -> Stop。
4. 断言 Pause 不推进 frame_count。
5. 断言 StepFrame exactly +1 frame。
6. 断言 Stop 释放 active runtime instance。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo test -p project_e2e_gate editor_gameview
cargo test -p editor_core gameview
```

### Gate G：整体回归与文档同步

任务：

```text
1. cargo fmt。
2. 跑整体回归。
3. 生成阶段完成记录。
4. 更新 49 / 54 / 施工文档 README / 阶段完成记录 README / 00 文档地图摘要。
5. 将本施工文档从 当前/ 移动到 已完成/。
```

测试：

```powershell
cd G:\gameEngin\rust
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_window_winit input_runtime_loop
cargo test -p project_e2e_gate editor_gameview
```

## 5. 施工自审

```text
范围没有扩大到 Full Debugger。
范围没有扩大到 time scale / breakpoint / timeline / replay。
范围没有扩大到 RuntimeRenderer / RHI / OS fullscreen。
38-220 审查被分类为前置系统约束，不机械套用为 221 方案审查。
Pause / Stop 语义必须分离。
StepFrame 必须报告 target_runtime_domain。
Maximize on Play 必须有用户/AI 可读写入口。
每个 Gate 都有测试命令。
完成后必须生成阶段完成记录并归档施工文档。
```

## 6. 完成标准

```text
代码实现完成。
Gate 测试通过。
整体回归通过。
阶段完成记录生成。
入口文档同步。
施工文档归档。
```

