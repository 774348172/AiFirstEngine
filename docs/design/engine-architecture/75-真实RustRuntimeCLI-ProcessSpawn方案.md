# 真实 Rust Runtime CLI / Process Spawn 方案

## 当前归属说明：Projection 术语

本文中如果出现以下历史名称：

```text
RenderExtract
RenderAssetBridge / Render Asset Bridge
Physics2DBridge
RuntimeScene Hydration
AuiRenderExtract / AuiRendererBridge
SpriteRenderer2D ECS-to-RenderProxy Bridge
```

统一按 `110-World-Projection-Adapter统一跨域同步规则.md` 理解为：

```text
RenderProjection
AssetProjection
Physics2DProjection
HydrationProjection
UiProjection
RenderProjectionAdapter<SpriteRenderer2D>
```

这些名称可以作为历史实现名保留，但不再作为新增架构概念扩展。后续新增类型只新增对应 `ProjectionAdapter`，不新增独立 Bridge。

本文档定义 Build / Run staged package 之后，编辑器如何真正启动 Rust Runtime 进程，并把运行结果以结构化报告回流给编辑器、用户和 AI。

本方案不重新讨论 Build Graph、Runtime Package、Scene 实例化、ProjectLogicRunner、Viewport 输入回流或 RDG/RHI。相关规则见：

```text
07-Build-Export-Pipeline.md
21-Runtime-Core-Boundary.md
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
68-Runtime资源加载系统方案.md
70-Scene-Prefab-Entity-Runtime实例化方案.md
72-Build-Run-Package-Orchestrator-v1方案.md
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
```

## 问题是什么

当前 Build / Run Package Orchestrator v1 已经能完成：

```text
Project
  -> Build Graph
  -> Runtime Package
  -> cooked assets
  -> Rust Runtime executable
  -> staged run folder
  -> launch command
  -> BuildRunReport
```

但它还没有真正启动一个独立 Rust Runtime 进程。当前缺口是：

```text
Editor / BuildRunOrchestrator
  -> spawn rust runtime process
  -> runtime 从 staged folder 读取 runtime package
  -> runtime 跑 EngineHostLoop / FrameLoop
  -> stdout / stderr / exit code / runtime report 回到编辑器
```

这个系统的核心不是再做一套构建系统，而是确认 Runtime 能脱离编辑器进程独立启动，并且运行结果可以被 AI 和用户稳定诊断。

## 其它引擎怎么做

### Unreal Engine

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\Scripts\RunProjectCommand.Automation.cs
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\AutomationUtils\Platform.cs
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\AutomationUtils\ProcessUtils.cs
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\Win\WinPlatform.Automation.cs
```

UE 路线：

```text
BuildCookRun
  -> Cook
  -> Stage
  -> RunProjectCommand
  -> Platform.RunClient
  -> ProcessUtils.Run / CreateProcess
```

关键点：

```text
Stage 是正式运行前的边界。
运行读取 staged / cooked 内容，而不是编辑器内存对象。
RunClient 是平台抽象点。
进程 stdout / exit code / log 是自动化和诊断的重要输入。
```

### Unity

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPlayerWindow.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPipeline\BuildPipeline.bindings.cs
```

Unity 路线：

```text
BuildPlayerOptions
  -> BuildPipeline.BuildPlayer
  -> BuildOptions.AutoRunPlayer
  -> platform player
  -> BuildReport
```

关键点：

```text
用户看到的是 Build And Run。
底层生成平台 Player 和数据目录。
BuildReport 贯穿构建流程。
Editor Play 和 Build Player 不是完全同一条底层路径，复杂项目需要额外关注一致性。
```

### Bevy

本地源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app\src\app.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app\src\schedule_runner.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_winit\src\lib.rs
```

Bevy 路线：

```text
cargo run
  -> App::run
  -> runner
      -> winit runner
      -> schedule runner / headless runner
```

关键点：

```text
Runtime 本身是独立程序。
runner 决定 windowed 或 headless 执行方式。
App::run 返回 AppExit，适合结构化退出。
```

## 方案对比

### 方案 A：继续只生成 launch command，不真正 spawn

```text
BuildRunOrchestrator
  -> launch command string
  -> report
```

优点：

```text
实现最简单。
不涉及进程生命周期、stdout、timeout、kill。
```

缺点：

```text
不能证明 Runtime 能独立运行。
最小游戏循环仍停在纸面命令。
AI 无法根据真实进程退出码和 runtime report 定位问题。
```

结论：

```text
不采用。它已经是 72 的完成范围，不能继续当下一阶段。
```

### 方案 B：编辑器进程内直接调用 Runtime crate

```text
Editor
  -> engine_runtime crate
  -> EngineHostLoop
```

优点：

```text
调试方便。
速度快。
第一版实现成本低。
```

缺点：

```text
Runtime 没有真正脱离编辑器。
容易让编辑器内存对象和 Runtime 运行状态混在一起。
和 Build / Run staged package 的长期目标冲突。
```

结论：

```text
只允许作为单元测试内部 helper，不作为正式 Run 路线。
```

### 方案 C：UE-like Stage + RunClient，Runtime 内部 Bevy-like runner

```text
BuildRunOrchestrator
  -> Build Runtime Package
  -> Stage Run Folder
  -> RuntimeLaunchRequest
  -> RuntimeProcessSpawner
  -> ai_engine_runtime_cli
      --package staged/runtime-package.json
      --mode headless | windowed
      --frames N
      --report staged/reports/runtime-run-report.json
  -> RuntimeProcessReport
  -> Editor Console / RuntimeTrace
```

优点：

```text
符合 UE 的 Stage / RunClient 边界。
Runtime 真正脱离编辑器进程。
保留 Bevy-like runner，可扩展 headless / windowed。
输入输出结构化，AI 能读懂失败原因。
第一版可以只做本机 headless，不提前引入复杂 IPC。
```

缺点：

```text
比进程内调用多一层进程管理。
需要处理 stdout / stderr / timeout / exit code。
```

结论：

```text
采用方案 C-min。
```

### 方案 D：常驻 Runtime Service + 双向 IPC

```text
Editor
  <-> Runtime Service
      -> command stream
      -> frame stream
      -> live reload
```

优点：

```text
长期编辑器预览能力最强。
适合热重载、实时调试、远程设备和复杂运行会话。
```

缺点：

```text
第一版复杂度过高。
需要协议、连接状态、断线恢复、版本兼容和安全边界。
容易拖慢最小游戏循环。
```

结论：

```text
作为后续长期扩展，不进入第一版。
```

## 正式规则

本项目采用：

```text
UE-like Stage + RunClient
+ Bevy-like Runtime runner
+ AI-native Request / Report
```

正式链路：

```text
BuildRunRequest
  -> BuildPlan
  -> RuntimePackage
  -> StageRunFolder
  -> RuntimeLaunchRequest
  -> RuntimeProcessSpawner
  -> Runtime CLI process
  -> RuntimeRunReport
  -> RuntimeProcessReport
  -> BuildRunReport / Editor Console
```

## 边界规则

```text
1. Runtime 必须能脱离编辑器进程启动。
2. Runtime CLI 只读取 staged runtime-package 和 staged cooked-assets。
3. Runtime CLI 不读取编辑器内存对象。
4. Runtime CLI 不依赖 Electron / TypeScript Runtime。
5. Runtime CLI 第一版只支持本机桌面 headless 运行。
6. Runtime process spawn 由 RuntimeProcessSpawner 统一执行。
7. 编辑器不直接拼裸命令行，必须先生成 RuntimeLaunchRequest。
8. AI / 用户诊断默认读取 RuntimeProcessReport 和 RuntimeRunReport。
9. 第一版不引入常驻服务和双向 IPC。
10. 后续 windowed / remote / device run 复用同一套 Request / Report，不重写主链路。
```

## RuntimeLaunchRequest v1

第一版最小字段：

```text
RuntimeLaunchRequest:
  runtime_executable_path
  staged_run_folder
  runtime_package_path
  cooked_assets_root
  run_mode: headless | windowed
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
路径必须是绝对路径或可解析到 staged run folder 内的路径。
runtime_package_path 必须位于 staged_run_folder 内。
report_path / stdout_log_path / stderr_log_path 默认位于 staged_run_folder/reports。
env 第一版只允许白名单字段。
args 由 RuntimeLaunchRequest 生成，不允许 UI 层自由拼接。
```

## Runtime CLI v1

第一版 CLI：

```text
ai_engine_runtime_cli
  --package <runtime-package.json>
  --assets <cooked-assets-root>
  --mode headless
  --frames <N>
  --report <runtime-run-report.json>
```

第一版只做：

```text
读取 runtime package。
加载主 scene。
创建 World / EngineHostLoop。
headless tick N 帧。
输出 RuntimeRunReport。
返回 exit code。
```

第一版不做：

```text
真实窗口。
真实 GPU surface。
热重载。
编辑器实时 IPC。
远程设备。
Android / iOS。
debugger attach。
```

## RuntimeRunReport v1

第一版最小字段：

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

用途：

```text
证明 Runtime CLI 真实跑过。
证明 Runtime 读取的是 staged package。
证明 FrameLoop / ProjectLogicRunner / RenderExtract 至少完成 N 帧。
给 AI 定位 runtime 加载、逻辑执行、渲染提取、退出原因。
```

## RuntimeProcessReport v1

RuntimeProcessReport 是编辑器 / BuildRunOrchestrator 侧的进程报告：

```text
RuntimeProcessReport:
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

区别：

```text
RuntimeRunReport 由 runtime CLI 写出，说明 runtime 内部发生了什么。
RuntimeProcessReport 由 spawner 写出，说明进程是否成功启动、是否超时、如何退出。
```

## 错误处理

第一版必须结构化处理：

```text
runtime_executable_missing
runtime_package_missing
staged_folder_missing
invalid_launch_request
spawn_failed
timeout
non_zero_exit
runtime_report_missing
runtime_report_invalid
```

规则：

```text
所有错误进入 RuntimeProcessReport.diagnostics。
stdout / stderr 不作为唯一真相，只作为辅助日志。
如果进程启动失败，也必须生成 RuntimeProcessReport。
如果 runtime 内部失败，优先读取 RuntimeRunReport；没有 report 再读 stderr。
```

## 与 BuildRunReport 的关系

BuildRunReport 保留为用户看到的总报告。

```text
BuildRunReport:
  build plan summary
  staged folder summary
  launch command summary
  runtime_process_report?
```

规则：

```text
BuildRunReport 不吞掉 RuntimeProcessReport。
BuildRunReport 可以引用 RuntimeProcessReport 路径和摘要。
RuntimeProcessReport 是 Run 阶段的事实来源。
```

## 测试规则

第一版必须 headless 测试：

```text
RuntimeLaunchRequest 序列化和路径校验。
Runtime CLI 可以读取最小 runtime package 并跑 1 帧。
Runtime CLI 可以跑固定 N 帧并输出 RuntimeRunReport。
RuntimeProcessSpawner 可以 spawn 一个测试 runtime exe 并捕获 exit code。
RuntimeProcessSpawner timeout 会 kill 进程并输出 report。
BuildRunOrchestrator launch=true 时生成 RuntimeProcessReport。
```

## 结论

真实 Rust Runtime CLI / Process Spawn 采用 C-min：

```text
本机桌面
headless mode
固定帧数
真实 process spawn
stdout / stderr capture
exit code / timeout / kill
RuntimeRunReport
RuntimeProcessReport
```

它比继续只生成命令更真实，比常驻 IPC 服务更简单。它把当前最小游戏循环从“staged package 可以生成”推进到“runtime 可以脱离编辑器真实运行”。
