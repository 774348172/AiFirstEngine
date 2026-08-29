# 83-真实默认 Windowed End-to-End Game Gate 方案

## 定位

本文档定义默认真实运行入口。

它不是新的 Runtime、Renderer、Asset、Scene 或 Input 系统，而是把已经确认的主链路收敛成一个用户点击 Run 后默认走的真实游戏闭环。

目标链路：

```text
Project
  -> Build / Stage Runtime Package
  -> Spawn Rust Runtime
  -> Open Native Window
  -> Load Scene / Assets
  -> EngineHostLoop
  -> Input -> Logic -> ECS -> RenderCommand
  -> RenderThread -> RDG / RHI -> Surface Present
  -> EndToEndGameRunReport
```

## 已有基础

本方案不重新讨论以下系统：

```text
72-Build-Run-Package-Orchestrator-v1方案.md
75-真实RustRuntimeCLI-ProcessSpawn方案.md
77-真实WindowedRuntime-ViewportPresent-C-min方案.md
79-真实可玩最小循环C-min方案.md
81-真实跨线程RenderThreadQueue-RenderSubmissionPipeline-v1方案.md
82-真实OSRenderThreadWorker-Fence-FrameLag方案.md
```

已经完成的能力：

```text
Runtime Package v1
Build / Run staged package
Rust Runtime CLI / process spawn
Scene / Prefab / Entity Runtime 实例化
Runtime 资源加载
ProjectLogicRunner / LogicExecutor / Rust ECS 接入
RenderCommand / RenderSceneState
RenderThreadQueue / RenderSubmissionPipeline
OS RenderThreadWorker / Fence / FrameLag
Windowed Runtime / Viewport Present C-min
最小游戏闭环 headless gate
```

当前缺口：

```text
用户默认 Run 还没有被收敛成唯一主入口。
真实 windowed 路径和 headless gate 的关系需要明确。
端到端失败还需要统一报告到一个 EndToEndGameRunReport。
```

## Headless 的定义

`headless` 指不打开真实窗口、不依赖真实显示器和真实 window surface 的自动化运行方式。

它不是第二套引擎，也不是第二套游戏逻辑。

```text
Windowed:
  Native Window
  -> real surface
  -> real present

Headless:
  no OS window
  -> headless / simulated surface backend
  -> same runtime main path
  -> structured report
```

允许差异只存在于：

```text
Window / Surface / Present 适配层
```

不允许差异存在于：

```text
Runtime Package
Scene Load
Asset Load
EngineHostLoop
ProjectLogicRunner
ECS
RenderCommand
RenderThread
RuntimeRenderer
Report schema
```

## 其它引擎参考

### Unreal Engine

源码参考：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Launch/Private/LaunchEngineLoop.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Programs/AutomationTool
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Programs/AutomationTool/Scripts/RunProjectCommand.Automation.cs
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/RenderCore/Private/RenderingThread.cpp
```

UE 的核心路线：

```text
BuildCookRun
  -> Cook / Stage
  -> Run
  -> FEngineLoop::Tick
  -> Game Thread
  -> ENQUEUE_RENDER_COMMAND
  -> Render Thread
  -> RHI Present
```

可参考：

```text
Stage 是运行前边界。
默认运行入口必须由引擎拥有。
Game Thread / Render Thread 分离。
日志、exit code、report 是自动化和诊断入口。
```

不照搬：

```text
不做完整 UE PIE。
不做完整 AutomationTool 平台矩阵。
不做 NullRHI / Server / Client 全套模式。
```

### Unity

源码参考：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Runtime/Export/PlayerLoop/PlayerLoop.bindings.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/BuildPlayerWindow.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/BuildPipeline
```

Unity 的核心路线：

```text
Play / Build And Run
  -> PlayerLoop
  -> Update / LateUpdate
  -> Render
  -> GameView / Player Present
```

可参考：

```text
用户心智必须简单。
Run / Play 是用户看到的主入口。
BuildReport / PlayMode test 是诊断和自动化入口。
```

不照搬：

```text
不接受 Editor Play 和 Build Player 行为差异变成长期黑箱。
```

### Godot

源码参考：

```text
<GODOT_SOURCE>/godot-master/godot-master/main/main.cpp
<GODOT_SOURCE>/godot-master/godot-master/scene/main/scene_tree.cpp
<GODOT_SOURCE>/godot-master/godot-master/servers/display_server.h
<GODOT_SOURCE>/godot-master/godot-master/servers/rendering_server.h
```

Godot 的核心路线：

```text
Main::start
  -> Main::iteration
  -> MainLoop / SceneTree
  -> process / physics_process
  -> DisplayServer / RenderingServer
```

可参考：

```text
顶层 MainLoop 清晰。
Scene / Viewport / Display / Render 分层明确。
```

不照搬：

```text
不采用 Node / SceneTree 作为我们的底层真相。
```

### Bevy

源码参考：

```text
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_app/src/app.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_winit/src/state.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_render/src/extract_plugin.rs
```

Bevy 的核心路线：

```text
App::run
  -> runner
  -> winit runner 或 schedule runner
  -> ECS schedule
  -> Extract / RenderApp
  -> wgpu present
```

可参考：

```text
Rust / winit / wgpu 路线接近我们。
windowed runner 和 headless runner 可以分离。
```

不照搬：

```text
不引入 Bevy Schedule / RenderWorld 的复杂规则到项目层。
```

## 方案对比

### 方案 A：继续保持分模块 gate

```text
Build gate
Runtime gate
Windowed present gate
RenderThread gate
Asset gate
```

优点：

```text
实现成本低。
每个模块测试清楚。
```

缺点：

```text
没有默认真实游戏入口。
用户和 AI 仍然不知道一次 Run 到底卡在哪条主链路。
长期容易形成“模块都过，但游戏跑不起来”的假闭环。
```

### 方案 B：默认 Run 走真实 windowed，默认自动化 gate 走 headless

```text
用户默认 Run:
  Windowed End-to-End Game Gate

默认自动化测试:
  Headless End-to-End Game Gate

本机 smoke:
  feature-gated real windowed present
```

优点：

```text
用户路径是真实窗口。
自动化路径稳定，不依赖桌面环境和 GPU window surface。
AI 可以通过同一份 report 定位失败层。
共享同一套 Runtime 主链路，不维护第二套引擎。
```

缺点：

```text
需要维护 WindowedBackend 和 HeadlessBackend 两个最外层 backend。
需要严格限制二者差异，避免行为分叉。
```

### 方案 C：所有 gate 都强制打开真实窗口

优点：

```text
最接近用户实际运行。
```

缺点：

```text
CI / 远程机器 / 无桌面会话 / GPU 驱动 / 窗口焦点都可能导致非代码失败。
AI 无法稳定判断是引擎 bug 还是环境问题。
```

## 最终规则

采用方案 B。

```text
默认用户 Run 必须走真实 Windowed Runtime。
默认自动化 Gate 必须走 Headless Runtime Gate。
本机真实窗口 Smoke 必须存在，但不作为默认 CI 必过项。
Windowed 和 Headless 必须共享 Runtime Package / EngineHostLoop / Logic / ECS / RenderCommand / RenderThread 主链路。
差异只允许存在于 Window / Surface / Present 适配层。
EndToEndGameRunReport 是用户和 AI 查错入口。
第一版只跑固定最小场景，目标是验证主链路，不做完整编辑器 Play Mode。
```

## 默认运行结构

```text
DefaultGameRunRequest
  project_path
  run_mode: Windowed | Headless
  scenario_id
  frame_limit
  report_path
  launch_runtime_process

DefaultGameRunOrchestrator
  -> BuildRunOrchestrator
  -> RuntimeProcessSpawner
  -> WindowedRuntimeHost 或 HeadlessRuntimeHost
  -> EngineHostLoop
  -> RenderThreadWorker
  -> RuntimeRenderer
  -> EndToEndGameRunReport
```

## EndToEndGameRunReport v1

第一版字段少而精：

```text
run_id
mode: Windowed | Headless
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

诊断规则：

```text
失败必须定位到一个 owner layer。
diagnostic 必须包含 code / severity / layer / message。
不得只返回 unknown error。
不得要求用户阅读原始 stdout 才能判断失败阶段。
```

## C-min 边界

第一版只做：

```text
1. 一个默认 run request。
2. 一个 fixed minimal scenario。
3. staged runtime package 输入。
4. Rust runtime process 启动。
5. headless end-to-end gate。
6. windowed smoke gate。
7. 至少跑 3-10 帧。
8. 至少 present 1 帧。
9. 输出 EndToEndGameRunReport。
```

第一版不做：

```text
完整 Play Mode 编辑器状态同步。
多窗口。
热重载。
复杂菜单。
移动端。
复杂场景 streaming。
完整平台发布。
```

## 为什么适合本项目

AI 友好：

```text
AI 可以从 EndToEndGameRunReport 直接判断失败层。
headless gate 稳定，适合每次 AI 修改后自动回归。
windowed smoke 保证真实用户路径不会长期失真。
```

复杂项目能力：

```text
Runtime 只读 staged package，不读编辑器内存对象。
默认 Run 从一开始就是正式 runtime 路径，而不是临时 editor preview。
```

可维护：

```text
本方案只做编排，不新增项目规则。
Windowed / Headless 差异被限制在最外层 backend。
```

效率：

```text
真实用户路径走 native window / surface / present。
自动化不被真实窗口环境拖慢或污染。
```

