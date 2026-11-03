# 83-当前可自动化施工文档：真实默认 Windowed End-to-End Game Gate C-min

## 目标

按照 `83-真实默认WindowedEndToEndGameGate方案.md` 实现 C-min。

目标不是重写 Runtime、Renderer、Asset、Scene 或 Input，而是把现有模块接成一个默认可运行主入口：

```text
Project
  -> Build / Stage Runtime Package
  -> Spawn Rust Runtime
  -> Windowed 或 Headless Runtime Host
  -> EngineHostLoop
  -> Logic / ECS / RenderCommand
  -> RenderThreadWorker
  -> RuntimeRenderer
  -> Present 或 Headless Present
  -> EndToEndGameRunReport
```

## 当前基础

已完成并必须复用：

```text
72-Build-Run-Package-Orchestrator-v1方案.md
75-真实RustRuntimeCLI-ProcessSpawn方案.md
77-真实WindowedRuntime-ViewportPresent-C-min方案.md
79-真实可玩最小循环C-min方案.md
81-真实跨线程RenderThreadQueue-RenderSubmissionPipeline-v1方案.md
82-真实OSRenderThreadWorker-Fence-FrameLag方案.md
```

## 施工原则

```text
不做第二套 Runtime。
不做第二套游戏逻辑。
不复制 Build / Run Orchestrator。
不复制 WindowedRuntimePresent。
不让项目逻辑感知 headless / windowed 差异。
不让默认自动化测试依赖真实窗口。
```

允许新增：

```text
DefaultGameRunRequest
DefaultGameRunMode
DefaultGameRunOrchestrator
EndToEndGameRunReport
HeadlessEndToEndGameGate
WindowedEndToEndGameSmoke
```

## A1：定义 DefaultGameRunRequest

新增或放入合适 runtime/editor bridge 模块：

```text
DefaultGameRunMode
  Headless
  Windowed

DefaultGameRunRequest
  project_path
  mode
  scenario_id
  frame_limit
  launch_runtime_process
  report_path optional
```

测试：

```text
default_game_run_request_defaults_to_headless_for_tests
default_game_run_request_supports_windowed_mode
default_game_run_request_is_json_serializable
```

## A2：定义 EndToEndGameRunReport

字段：

```text
run_id
mode
project_path
staged_run_folder
runtime_package_path
scenario_id
frame_limit

build_status
runtime_spawn_status
package_load_status
asset_load_status
scene_load_status
logic_tick_status
render_extract_status
render_thread_status
rdg_status
rhi_status
surface_status
present_status

frames_requested
frames_completed
first_presented_frame optional
exit_code optional
diagnostics[]
```

测试：

```text
end_to_end_game_run_report_is_json_serializable
end_to_end_game_run_report_locates_build_failure
end_to_end_game_run_report_locates_present_failure
end_to_end_game_run_report_never_reports_unknown_without_layer
```

## A3：实现 DefaultGameRunOrchestrator

编排职责：

```text
DefaultGameRunOrchestrator::run(request)
  -> BuildRunOrchestrator
  -> RuntimeProcessSpawner when launch_runtime_process=true
  -> HeadlessRuntimeHost or WindowedRuntimeHost
  -> collect reports
  -> EndToEndGameRunReport
```

规则：

```text
orchestrator 只编排。
不直接读写 ECS。
不直接生成 RenderCommand。
不直接操作 RHI。
```

测试：

```text
default_game_run_orchestrator_runs_headless_minimal_scenario
default_game_run_orchestrator_uses_staged_runtime_package
default_game_run_orchestrator_does_not_require_editor_project_memory
```

## A4：Headless End-to-End Gate

实现默认自动化 gate：

```text
mode = Headless
frame_limit = 3 到 10
scenario = minimal_game_loop
expected:
  package loaded
  scene loaded
  at least one logic tick
  at least one render submission
  at least one headless present
  report success
```

测试：

```text
headless_end_to_end_game_gate_runs_minimal_scene
headless_end_to_end_game_gate_completes_requested_frames
headless_end_to_end_game_gate_reports_asset_failure
headless_end_to_end_game_gate_reports_scene_failure
headless_end_to_end_game_gate_reports_render_thread_failure
```

## A5：Windowed Smoke Gate

实现真实窗口 smoke，不作为默认必跑测试：

```text
mode = Windowed
feature = real-window, real-wgpu-surface
ignored smoke test
frame_limit = 1 到 3
expected:
  native window created
  surface configured
  runtime ticked
  render thread submitted
  one frame presented
  report success
```

测试：

```text
cargo test -p editor_window_winit --features real-window,real-wgpu-surface windowed_end_to_end_game_smoke -- --ignored --nocapture
```

默认测试不得依赖该 smoke。

## A6：确保 Windowed / Headless 只在外层 backend 分叉

增加断言或测试：

```text
headless_and_windowed_share_runtime_package_path
headless_and_windowed_share_engine_host_loop_path
headless_and_windowed_share_render_thread_submission_path
headless_and_windowed_only_differ_at_surface_present_layer
```

如果难以直接测试路径，至少通过 report 字段和 mock backend 证明：

```text
Runtime Package 相同。
Scenario 相同。
FrameLimit 相同。
Logic Tick 发生。
RenderThread Submit 发生。
差异只出现在 surface_status / present_status。
```

## A7：命令入口

新增一个可调用入口，名称按现有工程习惯选择：

```text
run-default-game
run-default-game --mode headless
run-default-game --mode windowed
```

或在现有 CLI 中增加：

```text
ai_engine_runtime_cli run --mode headless
ai_engine_runtime_cli run --mode windowed
```

第一版必须支持 headless 自动化入口。

windowed 可以先作为 feature-gated smoke。

测试：

```text
runtime_cli_default_run_outputs_end_to_end_report
runtime_cli_headless_run_exits_zero_on_success
runtime_cli_reports_nonzero_exit_with_layered_diagnostic
```

## A8：文档与入口同步

施工完成后必须更新：

```text
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
18-长期主义实现路径.md
00-文档地图.md
施工文档/README.md
阶段完成记录/README.md
```

新增阶段记录：

```text
阶段完成记录/2026-06-28-真实默认WindowedEndToEndGameGate-C-min/00-总览.md
```

完成后移动施工文档：

```text
施工文档/当前/83-当前可自动化施工文档-真实默认WindowedEndToEndGameGate-C-min.md
  -> 施工文档/已完成/
```

## 验收标准

必须通过：

```text
cargo test -p engine_runtime default_game_run
cargo test -p engine_runtime end_to_end_game
cargo test -p editor_window_winit headless_end_to_end_game
```

如果 crate 名称不同，按实际模块调整，但必须保留等价测试。

可选本机 smoke：

```text
cargo test -p editor_window_winit --features real-window,real-wgpu-surface windowed_end_to_end_game_smoke -- --ignored --nocapture
```

验收结果：

```text
1. Headless end-to-end gate 默认可跑。
2. Windowed smoke gate 存在。
3. EndToEndGameRunReport 能定位失败层。
4. 用户默认 Run 路线明确走 Windowed Runtime。
5. 自动化默认 gate 不依赖真实窗口。
6. Windowed / Headless 不分叉 Runtime 主逻辑。
```

## 暂停条件

出现以下情况必须暂停并回到方案讨论：

```text
需要复制一套 Runtime 才能实现 headless。
需要让项目逻辑判断 headless / windowed。
需要绕过 Runtime Package 直接读编辑器内存对象。
需要绕过 EngineHostLoop / RenderThreadWorker 直接 present。
真实窗口 smoke 必须成为默认 CI 必跑项才可验证。
```

