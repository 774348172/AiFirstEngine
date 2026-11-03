# 当前可自动化施工文档：真实 Rust Runtime CLI / Process Spawn C-min

本文档是 75 号方案的施工文档。正式规则见：

```text
75-真实RustRuntimeCLI-ProcessSpawn方案.md
72-Build-Run-Package-Orchestrator-v1方案.md
21-Runtime-Core-Boundary.md
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
```

## 目标

实现真实 Rust Runtime CLI / Process Spawn 的最小闭环：

```text
BuildRunOrchestrator
  -> StageRunFolder
  -> RuntimeLaunchRequest
  -> RuntimeProcessSpawner
  -> ai_engine_runtime_cli process
  -> RuntimeRunReport
  -> RuntimeProcessReport
  -> BuildRunReport / Console
```

施工完成后必须能用 headless 测试证明：

```text
Runtime CLI 能从 staged runtime-package 启动。
Runtime CLI 能跑固定 N 帧。
Runtime CLI 能输出 RuntimeRunReport。
Spawner 能真实 spawn process。
Spawner 能捕获 stdout / stderr / exit code。
Spawner 能处理 timeout / kill。
BuildRunOrchestrator launch=true 时能接入 RuntimeProcessReport。
```

## 非目标

第一版不做：

```text
真实 windowed runtime。
真实 wgpu surface。
Render Thread。
常驻 Runtime Service。
双向 IPC。
热重载。
远程设备。
Android / iOS。
debugger attach。
完整平台发布。
```

## 当前基础

已完成：

```text
Runtime Package v1。
Runtime 资源加载系统 v1。
Scene / Prefab / Entity Runtime 实例化 v1。
EngineHostLoop / FrameLoop。
ProjectLogicRunner / LogicExecutor / Rust ECS 接入 v1。
Runtime Viewport Rendering System D-min。
Build / Run Package Orchestrator v1。
Native Editor Viewport 输入回流 Runtime Frame C-min。
```

## 自动化任务 A1：新增 Runtime CLI crate / binary

新增或扩展 Rust workspace：

```text
rust/crates/runtime_cli
```

或如果当前 workspace 已有合适 binary crate，则在现有 crate 中新增正式 runtime CLI binary。

CLI 第一版参数：

```text
--package <runtime-package.json>
--assets <cooked-assets-root>
--mode headless
--frames <N>
--report <runtime-run-report.json>
```

验收：

```text
无参数时返回结构化错误。
缺 package 时返回 non-zero exit code。
合法最小 package 时能输出 RuntimeRunReport。
```

最小测试：

```text
runtime_cli_rejects_missing_package
runtime_cli_runs_minimal_package_one_frame
runtime_cli_writes_runtime_run_report
```

## 自动化任务 A2：定义 RuntimeRunReport v1

在 runtime 侧新增结构：

```text
RuntimeRunReport:
  schema_version
  run_id
  package_path
  mode
  frame_limit
  frames_executed
  exit_reason
  exit_code
  diagnostics[]
  last_frame_hash
  last_runtime_trace_summary
  last_render_frame_summary
```

验收：

```text
可 JSON 序列化。
成功运行时 frames_executed == frame_limit。
失败运行时 diagnostics 有明确 code。
```

最小测试：

```text
runtime_run_report_serializes
runtime_run_report_records_successful_fixed_frame_run
runtime_run_report_records_package_load_failure
```

## 自动化任务 A3：实现 RuntimeLaunchRequest v1

在 Build / Run Orchestrator 或新 run 模块中新增：

```text
RuntimeLaunchRequest:
  runtime_executable_path
  staged_run_folder
  runtime_package_path
  cooked_assets_root
  run_mode
  frame_limit
  report_path
  stdout_log_path
  stderr_log_path
  timeout_ms
  env
  args
```

规则：

```text
UI / Editor 不直接拼裸命令行。
RuntimeLaunchRequest 负责生成 args。
路径必须可校验。
runtime_package_path / report_path 默认位于 staged_run_folder 内。
```

最小测试：

```text
runtime_launch_request_builds_cli_args
runtime_launch_request_rejects_package_outside_staged_folder
runtime_launch_request_defaults_report_paths_inside_staged_folder
```

## 自动化任务 A4：实现 RuntimeProcessSpawner

新增：

```text
RuntimeProcessSpawner
RuntimeProcessReport
```

RuntimeProcessReport 字段：

```text
launch_request_summary
process_started
process_id
start_time
end_time
duration_ms
exit_code
timed_out
killed
stdout_log_path
stderr_log_path
runtime_run_report_path
diagnostics[]
```

第一版要求：

```text
使用 std::process::Command。
捕获 stdout / stderr 到文件。
等待进程退出。
支持 timeout。
timeout 后 kill process。
无论成功失败都生成 RuntimeProcessReport。
```

最小测试：

```text
spawner_runs_test_process_and_captures_exit_code
spawner_writes_stdout_and_stderr_logs
spawner_timeout_kills_process_and_reports_timeout
spawner_reports_missing_executable
```

## 自动化任务 A5：接入 BuildRunOrchestrator

扩展 72 的 Build / Run 链路：

```text
launch=false:
  只生成 launch command / request / BuildRunReport

launch=true:
  StageRunFolder
  -> RuntimeLaunchRequest
  -> RuntimeProcessSpawner.spawn_and_wait
  -> RuntimeProcessReport
  -> BuildRunReport.runtime_process_report
```

验收：

```text
launch=false 行为不破坏。
launch=true 能真实执行 runtime CLI。
BuildRunReport 能引用 RuntimeProcessReport。
RuntimeProcessReport 不被 BuildRunReport 吞掉。
```

最小测试：

```text
build_run_launch_false_does_not_spawn_process
build_run_launch_true_spawns_runtime_cli_and_records_process_report
build_run_report_references_runtime_process_report_path
```

## 自动化任务 A6：端到端 headless 场景测试

新增端到端测试：

```text
1. 构造最小 runtime package。
2. Stage run folder。
3. launch=true。
4. Runtime CLI 读取 package。
5. EngineHostLoop tick 3 frames。
6. RuntimeRunReport 写出。
7. RuntimeProcessReport 写出。
8. BuildRunReport 引用 process report。
```

验收：

```text
frames_executed == 3。
exit_code == 0。
RuntimeRunReport 存在。
RuntimeProcessReport 存在。
BuildRunReport 能定位 RuntimeProcessReport。
stdout / stderr log path 存在。
```

最小测试：

```text
build_run_launch_true_runs_runtime_cli_three_frames
```

## 自动化任务 A7：文档同步

施工完成后必须：

```text
1. 新增 阶段完成记录/2026-xx-xx-真实RustRuntimeCLI-ProcessSpawn-C-min/00-总览.md
2. 将本文档从 施工文档/当前 移到 施工文档/已完成。
3. 更新 49-当前状态与下一步入口.md。
4. 更新 54-当前可自动化施工总入口.md。
5. 更新 18-长期主义实现路径.md。
6. 更新 README.md。
7. 更新 00-文档地图.md。
8. 更新 施工文档/README.md。
9. 更新 阶段完成记录/README.md。
```

## 测试命令

施工过程至少运行：

```powershell
cd G:\gameEngin\rust
cargo test --workspace
```

如果 Windows 默认 target 遇到旧测试二进制锁定，可使用干净临时 target：

```powershell
$stamp = Get-Date -Format 'yyyyMMddHHmmss'
$env:CARGO_TARGET_DIR="G:\gameEngin\.tmp\cargo-target-75-$stamp"
cargo test --workspace
```

## 完成标准

```text
1. Runtime CLI crate / binary 存在。
2. Runtime CLI 能读取最小 runtime package 并跑固定帧数。
3. RuntimeRunReport v1 写出并可 JSON 序列化。
4. RuntimeLaunchRequest v1 能生成受控 args。
5. RuntimeProcessSpawner 能真实 spawn process。
6. RuntimeProcessReport v1 能记录 exit code / timeout / stdout / stderr。
7. BuildRunOrchestrator launch=true 能接入真实 process spawn。
8. 端到端 headless 测试通过。
9. cargo test --workspace 通过。
10. 文档同步完成。
```
