# 84-Editor Play / Run Session System 方案

## 定位

本文档定义编辑器中的 Play / Run / Stop 会话系统。

它不是新的 Runtime、Renderer、Asset、Scene 或 Build 系统，而是把用户在编辑器中点击运行时发生的流程收敛为一个清晰的编辑器侧会话编排层。

目标链路：

```text
Toolbar Run / Stop
  -> PlaySessionRequest
  -> PlaySessionController
  -> Build / Stage Runtime Package
  -> DefaultGameRunRequest
  -> HeadlessGate 或 Windowed Runtime Process
  -> PlaySessionState
  -> Console / RuntimeTrace / PlaySessionReport
```

核心规则：

```text
Play Session 属于 Editor 侧。
Runtime 不知道编辑器按钮。
Viewport / Game View 不负责启动 Runtime，只负责显示运行输出和接收运行状态。
项目逻辑不能直接依赖 Play Session。
```

## 已有基础

本方案不重新讨论以下系统：

```text
07-Build-Export-Pipeline.md
17-Runtime-FrameLoop.md
38-Rust-Native-Runtime-MVP与TypeScript退役规则.md
47-Native-Editor-Host-BC路线.md
72-Build-Run-Package-Orchestrator-v1方案.md
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
75-真实RustRuntimeCLI-ProcessSpawn方案.md
79-真实可玩最小循环C-min方案.md
83-真实默认WindowedEndToEndGameGate方案.md
```

83 已经完成：

```text
DefaultGameRunRequest
DefaultGameRunMode
DefaultGameRunOrchestrator
EndToEndGameRunReport
Headless End-to-End Game Gate
runtime_cli run-default-game
windowed smoke plan
```

84 的职责是在编辑器层接住这些能力：

```text
用户点击 Run / Stop。
编辑器判断当前是否能运行。
编辑器构造 PlaySessionRequest。
编辑器触发 Build / Stage Runtime Package。
编辑器选择 Headless 或 Windowed runner。
编辑器启动 / 停止 runtime process。
编辑器把运行结果反馈到 Toolbar / Console / RuntimeTrace / Report。
```

## 源码参考结论

本方案参考以下源码文档：

```text
../UE源码参考/EditorPlaySession-PIE-Standalone源码参考.md
../Godot源码参考/11-EditorRun-GameView-PlaySession源码参考.md
../Unity源码参考/EditorPlayMode-GameView-BuildAndRun源码参考.md
../Bevy源码参考/14-AppRunner-Winit-ScheduleRunner-RunSession源码参考.md
```

### UE

UE 的核心结构是：

```text
FRequestPlaySessionParams
  -> PlaySessionRequest
  -> StartQueuedPlaySessionRequest
  -> StartPlayInEditorSession / StartPlayInNewProcessSession / Launcher
  -> PIE World / GameInstance / Viewport
  -> RequestEndPlayMap / EndPlayMap
```

可借鉴：

```text
UI 只构造 Request，不直接创建 World。
Start / Stop 都延迟到稳定同步点执行。
Request 和 SessionInfo 分离。
InProcess / NewProcess / Launcher 共用同一套 request 入口。
自动化测试直接构造 request，不依赖 UI。
```

不照搬：

```text
完整 PIE World 复制。
GWorld 切换。
多客户端、Late Join、OnlineSubsystem、VR Preview。
Blueprint debug references 转移。
```

原因：

```text
这些能力很强，但第一版过重。
它们会把编辑器和 runtime 深度耦合，增加 AI 修改和用户查 bug 的理解成本。
```

### Godot

Godot 的核心结构是：

```text
EditorRunBar
  -> EditorRun
  -> OS::create_instance
  -> runtime process
  -> EditorDebuggerNode / GameView / EmbeddedProcess
```

可借鉴：

```text
RunBar / Controller 负责保存、构建、启动 debugger、启动进程、更新按钮。
EditorRun 负责拼运行参数和管理 pid。
GameView 监听 play / stop，只负责显示或嵌入运行进程。
Stop 统一停止进程、停止 debugger、更新 UI 状态。
```

这是本项目第一版最接近的参考路线。

### Unity

Unity 对用户暴露的是少量 PlayMode 状态：

```text
isPlaying
isPlayingOrWillChangePlaymode
isPaused
Step
playModeStateChanged
```

GameView / PlayModeView 主要负责显示、尺寸、焦点和渲染目标，不负责启动逻辑。

可借鉴：

```text
用户可见状态必须少。
GameView / Viewport 不承担 Play Session 编排职责。
BuildAndRun 属于 Build Pipeline，不等同于 Editor Play。
```

### Bevy

Bevy 的核心结构是：

```text
App
  -> Runner
  -> Winit runner 或 ScheduleRunner
```

可借鉴：

```text
Runtime App 和 Runner 分开。
Windowed runner 与 Headless runner 分开。
Headless 测试不依赖窗口。
Windowed runner 由 OS event loop 驱动，不保证 run 后立刻返回。
AppExit / Stop signal 由 runner 统一处理。
```

## 方案选择

### 方案 A：完整 UE 式 In-Process PIE

```text
编辑器进程内复制运行世界。
Game View 直接绑定运行世界。
Stop 时完整还原编辑器世界。
```

优点：

```text
编辑器体验最强。
运行态与编辑器深度联动。
后续可支持复杂调试、断点、live inspect。
```

缺点：

```text
第一版复杂度最高。
编辑器和 runtime 耦合极深。
需要处理运行世界和编辑世界隔离。
Stop / cleanup 极难。
对 AI 生成和查 bug 不友好。
```

不选第一版。

### 方案 B：Godot 式 Runtime 子进程

```text
编辑器点击 Run。
编辑器构造 PlaySessionRequest。
编辑器 build / stage runtime package。
编辑器 spawn runtime process。
编辑器通过 report / trace / console 接收结果。
Viewport 只显示运行输出。
Stop 时结束 runtime process。
```

优点：

```text
边界清晰。
编辑器和 runtime 隔离。
更容易 headless 测试。
更容易把失败变成结构化 report。
更适合 AI 查错。
和当前 runtime_cli / DefaultGameRunRequest 基础一致。
```

缺点：

```text
第一版不会有完整 UE PIE 那种深度编辑态调试。
嵌入式 Game View / 进程窗口托管需要后续补。
编辑器 live inspect 能力需要后续专门设计协议。
```

推荐第一版。

### 方案 C：Unity 式统一 PlayMode 状态外观

```text
对用户只暴露 Playing / Paused / Stopped 等少量状态。
内部可走子进程或 in-process。
GameView 只是显示端。
```

优点：

```text
用户理解成本最低。
UI 状态简单。
```

缺点：

```text
Unity native 内部细节不公开，不能直接照搬内部实现。
如果只学外观，容易隐藏太多状态，不利于 AI 查错。
```

作为用户可见外观采用，但不作为内部完整架构。

## 最终规则

第一版采用：

```text
UE 的 Request / SessionState / 延迟 StartStop / 自动化 API
+ Godot 的 runtime 子进程运行模型
+ Unity 的简单用户可见状态
+ Bevy 的 Headless / Windowed runner 分离
```

不采用：

```text
完整 UE In-Process PIE World。
完整 Unity PlayMode native 一体化。
完整 Godot EmbeddedProcess。
完整多实例 / 多客户端 / 远程设备 / VR Preview。
```

## 标准结构

### PlaySessionRequest

```text
PlaySessionRequest
  session_id
  mode
  project_root
  runtime_package_path
  scene_ref
  build_profile
  run_profile
  frame_limit
  report_path
  requested_by
```

`mode` 第一版只允许：

```text
HeadlessGate
WindowedUserRun
```

后续可扩展但第一版不实现：

```text
ExternalDeviceRun
RemoteRun
MultiInstanceRun
EditorEmbeddedViewport
```

### PlaySessionState

```text
Idle
Preparing
Building
StagingPackage
Launching
Running
Stopping
Completed
Failed
```

用户 UI 默认只显示更少状态：

```text
Not Ready
Ready
Running
Stopping
Failed
```

完整状态用于 AI / Trace / Report，不强迫普通用户理解。

### PlaySessionController

职责：

```text
接收 PlaySessionRequest。
检查是否已有 session。
触发 build / stage。
构造 DefaultGameRunRequest。
启动 HeadlessGate 或 Windowed Runtime Process。
记录 pid / process handle。
接收 EndToEndGameRunReport。
生成 PlaySessionReport。
更新 Toolbar / Console / RuntimeTrace。
处理 StopRequest。
```

禁止职责：

```text
不直接执行项目逻辑。
不直接改 ECS。
不直接管理 Runtime Scene。
不直接读取或写入 RenderCommand。
不替代 Build Graph。
不替代 Runtime Package。
```

### PlaySessionReport

`PlaySessionReport` 包装 `EndToEndGameRunReport`，并补充编辑器会话信息：

```text
PlaySessionReport
  session_id
  mode
  state
  request_summary
  build_summary
  runtime_report: EndToEndGameRunReport
  process_summary
  diagnostics
  started_at
  ended_at
```

原则：

```text
EndToEndGameRunReport 是 runtime 端到端结果。
PlaySessionReport 是编辑器会话结果。
二者不能混成一个结构。
```

## Start / Stop 规则

### Start

```text
Toolbar / Command
  -> PlaySessionRequest
  -> queue request
  -> editor stable point
  -> PlaySessionController::start
```

规则：

```text
UI 回调不直接启动 Runtime。
Start 必须进入队列，在编辑器稳定同步点执行。
运行前必须确保 Runtime Package 已构建或已确认可复用。
WindowedUserRun 默认走真实 Runtime Process。
HeadlessGate 默认走自动化 Headless runner。
```

### Stop

```text
Toolbar / Command
  -> StopRequest
  -> queue stop
  -> editor stable point
  -> PlaySessionController::stop
```

规则：

```text
UI 回调不直接 kill process。
Stop 必须进入队列，在稳定同步点执行。
Stop 只停止 PlaySessionController 拥有的 runtime process。
Stop 后必须生成 PlaySessionReport。
Stop 后必须更新 Toolbar / Console / RuntimeTrace。
```

## Headless / Windowed 分流

```text
HeadlessGate:
  自动化测试默认模式。
  不打开真实窗口。
  必须走同一 Runtime Package / Scene / Logic / ECS / RenderCommand 主链路。
  只允许在 Window / Surface / Present 适配层分叉。

WindowedUserRun:
  用户点击 Run 默认模式。
  打开真实 runtime window。
  由 runtime process 拥有 window/event loop。
  编辑器只追踪状态、pid、report。
```

禁止：

```text
禁止 HeadlessGate 成为第二套游戏逻辑。
禁止 WindowedUserRun 绕过 Runtime Package。
禁止编辑器直接模拟 runtime frame 作为用户默认 Run。
```

## UI 规则

Toolbar 第一版只需要：

```text
Run
Stop
Status
```

Console 显示：

```text
build error
runtime launch error
runtime diagnostics
session stopped / failed / completed
```

RuntimeTrace 显示：

```text
最近一次运行的 runtime trace summary
关键 frame / diagnostic / report link
```

Viewport / Game View 第一版：

```text
不负责启动。
不负责构造 PlaySessionRequest。
只显示运行输出或运行状态。
WindowedUserRun 第一版可先使用独立 runtime window。
后续再讨论嵌入式 viewport。
```

## 自动化测试规则

必须支持不经过 UI 的测试入口：

```text
PlaySessionController::run_headless(request)
PlaySessionController::run_windowed_smoke(request)
```

测试重点：

```text
Request 能构造。
State 能按顺序变化。
HeadlessGate 能生成 PlaySessionReport。
WindowedUserRun 能生成 smoke report。
StopRequest 只停止当前 session。
失败时 Console / Report 能定位到 build / stage / launch / runtime 哪一层失败。
```

## 与 83 的关系

83 是：

```text
真实默认 Windowed End-to-End Game Gate
```

84 是：

```text
编辑器 Play / Run Session 编排层
```

关系：

```text
84 调用 83 的 DefaultGameRunRequest / DefaultGameRunOrchestrator / EndToEndGameRunReport。
83 不知道 84。
Runtime 不知道 Toolbar。
Toolbar 不知道 Runtime 内部细节。
```

## 为什么适合本项目

AI 友好：

```text
AI 只需要理解 Request / State / Report。
错误能定位到 build / stage / launch / runtime。
不会把 UI、Runtime、Scene、ECS、Render 混成一个黑盒。
```

复杂项目适配：

```text
Runtime 子进程隔离编辑器状态。
Runtime Package 是唯一运行输入。
后续可以扩展远程设备、多实例、嵌入式 viewport，但不破坏第一版边界。
```

可维护：

```text
PlaySessionController 是唯一会话拥有者。
Start / Stop 都走队列和稳定同步点。
Report 可追踪。
UI 只是消费者。
```

简单度：

```text
第一版不做完整 PIE。
不做多客户端。
不做复杂 debugger protocol。
不做嵌入式 viewport。
先把 Run / Stop / Report 闭环做对。
```

效率：

```text
用户运行走真实 windowed runtime process。
自动化走 headless gate。
Runtime 主链路不复制。
Window / Surface / Present 是唯一允许分叉点。
```

## 后续不直接讨论的内容

以下内容不属于 84 第一版：

```text
完整 In-Process Play Mode。
嵌入式 runtime window 到 editor viewport。
远程设备运行。
多实例运行。
Play Mode hot reload。
断点调试协议。
运行时 live inspect。
暂停 / 单帧 step。
```

这些可以在 84 第一版施工完成后，作为独立系统继续讨论。

## 下一步

本规则确认后，下一步可以生成施工文档：

```text
施工文档/当前/84-当前可自动化施工文档-EditorPlayRunSessionSystem-C-min.md
```

施工边界建议：

```text
PlaySessionRequest / PlaySessionState / PlaySessionReport。
PlaySessionController headless path。
DefaultGameRunRequest 接入。
Toolbar Run / Stop command model 接入。
Console / RuntimeTrace 最小反馈。
单元测试和 headless smoke test。
```

## 2026-06-28 实施完成补充

```text
84 Editor Play / Run Session System C-min 已完成。

已完成施工文档：
施工文档/已完成/84-当前可自动化施工文档-EditorPlayRunSessionSystem-C-min.md

阶段完成记录：
阶段完成记录/2026-06-28-EditorPlayRunSessionSystem-C-min/00-总览.md

实现边界：
PlaySessionRequest / PlaySessionState / PlaySessionReport / PlaySessionController C-min 已进入 editor_core。
EditorSession Play / Pause 已接入 PlaySessionController。
HeadlessGate 已真实调用 DefaultGameRunOrchestrator。
WindowedUserRun 保留明确 C-min diagnostic，不假装成功。
Console 已输出 play session started / completed / failed 最小摘要。

验证：
cargo test -p editor_core play_session：7 passed
cargo test -p editor_core editor_session_play：4 passed
cargo test -p editor_core editor_session_stop：1 passed
cargo test -p editor_input toolbar：1 passed
cargo test -p engine_runtime default_game_run：10 passed
cargo test -p runtime_cli：7 passed
cargo test --workspace：passed
```
