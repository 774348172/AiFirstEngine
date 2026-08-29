# 218-Editor In-process GameView Play Runner Productization v1 方案

采用方案：

```text
方案 A 总方向：Unity-like In-process Embedded GameView
第一轮落地：A-min staged in-process runtime + GameView contract
```

简称：

```text
A-min-gameview
```

## 1. 这个系统是干什么的

一句话：

```text
让用户在编辑器里点击 Play 后，不只得到 headless report，而是在 Editor GameView 中运行 RuntimePackage 对应的 runtime 实例，并逐步获得真实画面、输入、Stop、Pause 和报告证据。
```

它解决的问题：

```text
217 已经让 Play 自动准备 Preview RuntimePackage，并能用 HeadlessGate 证明包可运行。
但用户仍不能像 Unity GameView 那样在编辑器里看到和操作游戏。
复杂打飞机项目要真正可编辑、可验证、可调试，必须有 Editor 内 GameView Play 体验。
```

它在其它成熟引擎中的对标：

```text
Unity：
  GameView + PlayMode。
  GameView 处理 target size、focus、toolbar、maximize-on-play、playModeStateChanged。

Unreal：
  PIE / New Editor Window / Standalone。
  PlayLevel.cpp 使用 RequestPlaySession -> StartQueuedPlaySessionRequest -> StartPlayInEditorSession / StartPlayInNewProcessSession。

Godot：
  EditorRunBar / EditorRun / GameView。
  默认运行进程和 GameView/Debugger 分离，GameView 是显示与调试消费者。
```

它在本引擎主线中的作用：

```text
217 负责准备 Preview RuntimePackage。
218 负责把 Preview RuntimePackage 作为 Editor Play 的真实运行输入，在 Editor 内创建可控 RuntimePlayInstance。
218 不改变 RuntimePackage 真相，不让 runtime 扫描项目源目录，也不让项目玩法进入 Core。
```

## 2. 关键判断：方案 A 可以选，但不能一次做完

方案 A 的完整目标是：

```text
Editor Play 后，Runtime 运行在 Editor 进程内。
GameView 是编辑器里的真实运行视图。
输入从 GameView 进入 RuntimeInputFrame。
RuntimeRenderer 输出到 Editor GameView texture。
Stop 能停止 in-process runtime。
报告能证明 RuntimePackage / frame / input / present 链路。
```

判断：

```text
方案 A 是最终体验最好的方向。
但不能一次施工完整 Unity-like GameView。
```

原因：

```text
一次做完会同时牵动 editor_core、editor_window_winit、editor_ui_model、editor_ui_renderer、engine_runtime、RuntimeRenderer、GPU texture lifetime、input routing、AUI interaction、Stop/Pause lifecycle。
如果一次性把 runtime_player_winit 或 RuntimeRenderer 深接进 editor_core，会造成 crate 耦合和测试困难。
如果第一版就要求真实 GPU texture + 真实输入 + Pause/Step + embedded window，会让施工范围失控。
```

因此正式路线是：

```text
采用方案 A 作为总方向。
第一轮只做 A-min。
后续用 A1 / A2 / A3 / A4 分阶段收敛。
```

## 3. 外部源码参考

### 3.1 Unity GameView / PlayMode

本机源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\GameView\GameView.cs
  GameView : PlayModeView
  OnEnable 注册 EditorApplication.playModeStateChanged
  OnPlayModeStateChanged 处理 EnteredPlayMode / ExitingPlayMode
  targetRenderSize / targetSize / GameViewSize / zoom / focus / toolbar

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\GUI\EditorApplicationLayout.cs
  InitPlaymodeLayout
  FinalizePlaymodeLayout
  SetPlaymodeLayout
  SetStopmodeLayout
```

官方参考：

```text
https://docs.unity3d.com/Manual/GameView.html
```

可学习点：

```text
GameView 是 PlayMode 显示和输入焦点的核心用户心智。
进入 PlayMode 前先准备 GameView 布局、尺寸和 focus，runtime 初始化完成后再 finalize。
GameView 本身负责显示、尺寸、焦点、工具栏，不应该负责 RuntimePackage 构建。
```

不照搬：

```text
不照搬 Unity 深度 native/managed PlayMode 黑箱。
不做 domain reload / scene reload 语义。
不让 runtime 直接读 editor unsaved scene 内存作为真相。
```

### 3.2 Unreal PIE / Standalone / New Process

本机源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\PlayLevel.cpp
  UEditorEngine::RequestPlaySession
  UEditorEngine::StartQueuedPlaySessionRequest
  UEditorEngine::StartQueuedPlaySessionRequestImpl
  UEditorEngine::StartPlayInEditorSession
  UEditorEngine::StartPlayInNewProcessSession
  UEditorEngine::GeneratePIEViewportWindow
```

官方参考：

```text
https://dev.epicgames.com/documentation/unreal-engine/play-in-editor-settings-in-unreal-engine
```

可学习点：

```text
Play 是会话请求，不是按钮里直接执行所有逻辑。
InProcess / NewProcess / Launcher 共享 PlaySession request model。
PIE viewport、window、world instance、debugger、stop lifecycle 都有清晰边界。
```

不照搬：

```text
不做 UE PIE World 复制体系。
不做 GWorld 切换。
不第一版做多客户端、多进程、多 viewport。
```

### 3.3 Godot EditorRun / GameView

本项目已有源码参考：

```text
框架设计/Godot源码参考/11-EditorRun-GameView-PlaySession源码参考.md
```

官方参考：

```text
https://docs.godotengine.org/en/latest/tutorials/editor/command_line_tutorial.html
```

可学习点：

```text
RunBar 负责编排 Play/Stop，EditorRun 负责启动，GameView/Debugger 是消费者。
运行和显示可以拆开，GameView 不应该承担构建和会话编排。
```

不照搬：

```text
218 选择的是 in-process GameView 总方向，不把 Godot 的外部进程方案作为最终体验。
但 Godot 的 PlaySession/RunBar/GameView 分层可作为 218 的边界参考。
```

## 4. 本项目当前基线

已完成基础：

```text
84 Editor Play / Run Session System C-min：
  PlaySessionRequest / PlaySessionController / PlaySessionReport。
  HeadlessGate 真实调用 DefaultGameRunOrchestrator。
  WindowedUserRun 当前返回明确 diagnostic。

217 Editor Play / RuntimePackage Preview Productization v1：
  Editor Play 自动准备 Preview RuntimePackage。
  Preview cache / dirty domain / PlaySessionReport link 已完成。

210 RuntimeRenderer Multi-stage UI Composition Pass：
  RuntimeRenderer 支持 UI pass composition。

213-216 AUI Runtime Interaction / Complex Controls / Navigation / Text Entry：
  runtime_player_winit 中已有 AUI present -> interaction -> filtered input -> runtime input 链路。

editor_window_winit：
  已有 SceneView / GameView viewport model、ViewportInputGateway、GameView focused -> RuntimeInputFrame 雏形。

runtime_player_winit：
  已有 NativePlayerWindowRunRequest、real-window feature、screenshot report、AUI present/input evidence。
```

当前代码基线：

```text
rust/crates/editor_core/src/play_session.rs
  PlaySessionMode::WindowedUserRun 仍调用 run_windowed_smoke。
  diagnostic = windowed_session_not_implemented_in_c_min。

rust/crates/editor_core/src/services/play_service.rs
  start_play_session 当前固定构造 PlaySessionRequest::headless_gate。
  已能通过 EditorPreviewPackageService 准备 runtime_package_path。

rust/crates/editor_window_winit/src/viewport.rs
  ViewportKind::Game / ViewportHost / RuntimeViewportFrameSummary 已存在。
  但 GameView 输出仍是 TestTexture / placeholder。

rust/crates/editor_window_winit/src/input_route.rs
  GameView focused 时可生成 RuntimeInputFrame。
  但尚未接真实 EditorRuntimePlayInstance。

rust/crates/runtime_player_winit/src/lib.rs
  真实 OS window 和 AUI runtime 链路已有。
  但它是 Player Host，不应被 editor_core 直接强耦合为第一版方案。
```

真实缺口：

```text
没有 EditorRuntimePlayInstance。
没有 EditorGameViewPlayRunner。
没有 GameViewRuntimeFrame / GameViewPresentReport。
PlaySessionMode::WindowedUserRun 仍不是可运行路径。
GameView 不能从 RuntimeRenderer 得到 runtime frame texture/descriptor。
GameView focused input 还没有进入真实 runtime tick。
Stop/Pause/Step 对 in-process runtime 尚未产品化。
```

### 4.1 吸收 36 号审查后的修正

`其它AI审查目录/36-218-Editor-In-process-GameView-Play-Runner方案审查.md` 判断 218 方向正确，但指出施工前必须吸收以下技术约束：

```text
1. runtime_player_winit::run_windowed_native_player_from_package 不能直接内嵌到 Editor GameView。
   它会创建独立 winit EventLoop、阻塞 run_app、创建独立 OS window 和独立 Surface。
   218 A-min 禁止复用该路径来冒充 Editor 内嵌 GameView。

2. 当前没有 runtime -> editor 的真实 GPU texture 共享/贴图管线。
   A-min 只做 GameViewRuntimeFrame descriptor evidence，不声称完成 real GPU texture present。

3. PlaySessionController 当前接收 DefaultGameRunOrchestrator 具体类型。
   施工 Gate A 必须先引入 PlayRunner trait 或等价 runner 抽象，让 HeadlessGate runner 与 EditorGameViewPlayRunner 共用 PlaySessionController。

4. A1 必须明确 AUI present 策略。
   EditorRuntimePlayInstance tick 时应使用与 runtime_player_winit 对齐的 ProjectUiStateSnapshotProducer + font atlas AUI present 链路，不能继续使用 package_smoke 占位。

5. A1 不接 GameView input。
   A1 只做 no-input deterministic tick；GameView focused input / AUI interaction 留到 A3。

6. A-min e2e 只断言 descriptor evidence：
   frame_count > 0、frame_hash、renderable_count、ui_draw_item_count、GameViewPresentReport link。
   不断言真实画面和真实输入。
```

同时修正基线措辞：

```text
217 之后 Toolbar Play enabled 条件已经是 has_project || has_runtime_package。
218 不再把“没有 active RuntimePackage 时 Play disabled”作为当前基线。
```

## 5. 可选方案

### 方案 A：Unity-like In-process Embedded GameView

内容：

```text
Editor 进程内创建 RuntimePlayInstance。
RuntimePackage 由 217 准备。
EngineHostLoop / RuntimeRenderer 在 editor-controlled runner 中 tick。
GameView 显示 runtime 输出。
GameView focused input 进入 RuntimeInputFrame。
Stop/Pause/Step 直接控制 in-process runtime。
```

优点：

```text
用户体验最接近 Unity。
输入、画面、暂停、单帧、调试都可以成为编辑器内一等能力。
复杂打飞机项目迭代体验最好。
后续 SceneView / GameView / Inspector / Report 能形成统一 authoring loop。
```

缺点：

```text
实现风险最大。
容易造成 editor/runtime/GPU surface 生命周期耦合。
一次做完会范围失控。
需要强测试和分阶段边界。
```

结论：

```text
采用为总方向，但只能分阶段落地。
第一轮采用 A-min，不一次完成完整 Unity-like GameView。
```

### 方案 B：Standalone WindowedUserRun

内容：

```text
Editor Play 准备 RuntimePackage。
Editor 启动 runtime_cli run-native-player 或 runtime_player_winit 子进程。
游戏在独立窗口运行。
Editor 只管理 pid / report / Stop。
```

优点：

```text
最稳。
复用 runtime_player_winit。
风险低，和 Godot / UE Standalone 相似。
```

缺点：

```text
不是用户希望的 Unity-like Editor GameView。
GameView 仍不是真实运行视图。
后续还要回到方案 A。
```

结论：

```text
不作为本轮采用方案。
可作为 fallback/debug runner 或 A-min 失败时的后备路线。
```

### 方案 C：Child Process + GameView Session Proxy

内容：

```text
运行仍在子进程。
GameView 只显示运行状态、pid、last screenshot、report link、Stop。
后续再做嵌入或 frame streaming。
```

优点：

```text
AI-first 报告好做。
不会破坏边界。
比 B 更像编辑器体验。
```

缺点：

```text
仍不是真正 in-process GameView。
无法满足用户明确选择的方案 A。
```

结论：

```text
不采用为主线。
其中 GameView session/report 面板能力可被 A-min 吸收。
```

## 6. 正式采用：A-min-gameview

正式命名：

```text
Editor In-process GameView Play Runner Productization v1
```

采用：

```text
方案 A 总方向。
第一轮只做 A-min。
```

核心链路：

```text
Toolbar Play
  -> 217 EditorPreviewPackageService::prepare
  -> RuntimePackage path
  -> EditorGameViewPlayRunner::start
  -> EditorRuntimePlayInstance::load_runtime_package
  -> hydrate active scene into runtime world
  -> tick EngineHostLoop for frame(s)
  -> produce GameViewRuntimeFrame
  -> update Editor GameView model / ViewportHost runtime frame descriptor
  -> PlaySessionReport + GameViewPresentReport
```

## 7. 分阶段路线

### A0：In-process GameView Contract

目标：

```text
定义边界，不急着真实渲染。
```

新增概念：

```text
EditorGameViewPlayRunner
EditorRuntimePlayInstance
EditorRuntimePlayRequest
EditorRuntimePlayState
GameViewRuntimeFrame
GameViewPresentReport
GameViewInputBridge
```

A0 必须回答：

```text
RuntimePackage path 从哪里来。
Runtime instance 生命周期谁负责。
GameView frame summary 如何进入 editor_ui_model / editor_window_winit。
PlaySessionReport 如何链接 GameViewPresentReport。
Stop 如何清理 instance。
```

A0 不做：

```text
真实 GPU texture。
真实 GameView input。
Pause/Step。
Maximize on Play。
```

### A1：Headless Runtime Instance in Editor

目标：

```text
在 Editor 进程内创建 runtime instance，并 tick 若干帧。
输出结构化 frame/report，不要求真实画面。
```

链路：

```text
Preview RuntimePackage
  -> load_runtime_package
  -> hydrate_active_scene_into_world
  -> EngineHostLoop tick(no input)
  -> ProjectUiStateSnapshotProducer + font atlas AUI present
  -> RuntimeFrameOutput / trace / frame_hash
  -> GameViewRuntimeFrame summary
```

验收：

```text
PlaySessionMode::WindowedUserRun 不再返回 windowed_session_not_implemented_in_c_min。
A1 可用 in-process runner 产出 frame_count > 0。
A1 的 GameViewRuntimeFrame 能报告 renderable_count 和 ui_draw_item_count。
A1 不接 GameView input，不做 AUI interaction；input_bridge_status=deferred。
Stop 可释放 EditorRuntimePlayInstance。
Report 能显示 package path、scene id、frame count、diagnostics。
```

### A2：GameView Texture Present

目标：

```text
把 RuntimeRenderer 输出接到 Editor GameView texture slot。
```

链路：

```text
RuntimeRenderer
  -> ViewportTextureDescriptor
  -> GameViewRuntimeFrame
  -> editor_window_winit ViewportHost
  -> editor_ui_renderer DrawCommand::ViewportTextureSlot
```

验收：

```text
GameView model 不再只是 placeholder。
GameViewRuntimeFrame 包含 target_id / texture_id / frame_index / width / height。
headless visual gate 能看到 ViewportTextureSlot 来自 runtime frame。
```

A2 注意：

```text
v1 可以先使用 headless texture descriptor / software evidence。
真实 GPU texture lifetime 和 real-window screenshot 可以作为 optional/local-only。
不能为了演示画面而绕过 RuntimeRenderer。
不能复用 runtime_player_winit::run_windowed_native_player_from_package 作为内嵌实现。
后续真实 GPU texture 需要单独建立 offscreen texture -> editor renderer blit 或等价 texture sharing 管线。
```

### A3：GameView Input / Focus / AUI Interaction

目标：

```text
GameView focused 输入进入 RuntimeInputFrame，再驱动 runtime tick。
```

链路：

```text
EditorInputEvent
  -> ViewportInputGateway
  -> RuntimeInputFrame
  -> EditorRuntimePlayInstance::tick_with_input
  -> AUI interaction / gameplay input
  -> next GameViewRuntimeFrame
```

验收：

```text
GameView 未 focused 时输入不进入 runtime。
GameView focused 时 pointer/key/text/gamepad C-min 可进入 RuntimeInputFrame。
AUI consumed input 不泄漏到 gameplay。
Report 输出 input_route / consumed / action count / frame id。
```

### A4：Unity-like 增强

后续独立讨论：

```text
Pause / Resume / Step。
Maximize on Play / Play Focused / Play Unfocused。
Stats overlay。
Screenshot / capture frame。
GameView size/aspect/device simulation。
Debugger hooks。
多实例。
真正 embedded native surface 或 editor GPU texture sharing。
```

## 8. A-min 本轮范围

本轮只做：

```text
A0：contract/schema/report。
A1：Editor 进程内 runtime instance 能 load RuntimePackage 并 tick frame。
A1 集成 AUI present summary，但不接 input。
A2 的最小 descriptor 接口预留，但不要求真实 GPU texture。
GameView model 可以显示 runtime frame descriptor / report evidence。
PlaySessionReport 链接 GameViewPresentReport。
Stop 能清理 instance。
```

本轮不做：

```text
真实 GPU texture present。
完整 input routing。
完整 AUI interaction in editor GameView。
runtime_player_winit 独立 EventLoop 内嵌。
Pause / Step。
Maximize on Play。
多实例。
外部子进程 runner。
runtime_player_winit 直接进入 editor_core 依赖。
```

## 9. 新增结构

### 9.1 EditorRuntimePlayRequest

```text
EditorRuntimePlayRequest
  schema_version
  session_id
  project_root
  runtime_package_path
  scene_ref
  run_profile
  frame_limit
  requested_by
  preview_package_report_path
```

### 9.2 EditorRuntimePlayInstance

```text
EditorRuntimePlayInstance
  session_id
  runtime_package_path
  package_summary
  world
  frame_loop
  engine_host_loop
  state
  frame_index
  last_frame
  diagnostics
```

规则：

```text
它是 Editor 侧 runtime session object。
它不写回 Scene/Prefab/AUI authoring document。
它不扫描项目源目录。
它只能从 RuntimePackage load。
```

### 9.3 GameViewRuntimeFrame

```text
GameViewRuntimeFrame
  schema_version
  session_id
  scene_id
  frame_index
  frame_hash
  target_id
  texture_id
  width
  height
  renderable_count
  ui_draw_item_count
  aui_present_status
  input_bridge_status
  diagnostics
```

A-min 规则：

```text
target_id / texture_id 可以是 descriptor evidence。
不要求真实 GPU resource handle。
```

### 9.4 GameViewPresentReport

schema：

```text
editor-gameview-present-report.v1
```

字段：

```text
schema_version
session_id
status
runtime_package_path
preview_package_report_path
scene_id
frame_count
last_frame_hash
game_view_output_kind
texture_descriptor_status
input_bridge_status
aui_present_status
stop_status
diagnostics
next_actions
deferred_flags
```

deferred flags：

```text
real_gpu_texture_present_deferred=true
gameview_input_bridge_deferred=true
aui_interaction_in_editor_gameview_deferred=true
pause_step_deferred=true
maximize_on_play_deferred=true
embedded_native_surface_deferred=true
multi_instance_play_deferred=true
```

## 10. PlaySession 集成规则

新增或修正：

```text
PlaySessionMode::WindowedUserRun 在 218 A-min 中表示 editor in-process GameView runner。
旧 diagnostic windowed_session_not_implemented_in_c_min 必须被替换为真实 A-min report。
```

命名说明：

```text
WindowedUserRun 是历史名字。
218 A-min 可以保留 enum 兼容，但 report 中必须说明 runner_kind=editor_in_process_gameview。
后续可把 PlaySessionMode 扩展为 HeadlessGate / EditorGameView / ExternalWindowed。
```

PlaySessionReport 扩展：

```text
game_view_present_report_path
game_view_frame_count
game_view_last_frame_hash
runner_kind
```

规则：

```text
PlaySessionReport 不吞掉 PreviewPackageReport。
PlaySessionReport 不吞掉 GameViewPresentReport。
Report Panel 需要能分别定位 preview package、play session、game view present 三层失败。
```

## 11. GameView / UI 行为

A-min 用户可见行为：

```text
项目已打开：
  Play enabled。
  点击 Play 后准备 Preview RuntimePackage。
  创建 EditorRuntimePlayInstance。
  GameView 显示 Running / frame id / package path / report link / runtime frame descriptor。

Stop：
  清理 EditorRuntimePlayInstance。
  GameView 状态回到 Stopped。

失败：
  Console 和 Report Panel 指向 preview / load / hydrate / tick / present 哪一层失败。
```

A-min 不承诺：

```text
用户已经能完整操作游戏。
用户已经能看到真实 GPU 画面。
Play Maximized / Focused 已完成。
```

## 12. 复杂打飞机项目例子

### 12.1 点击 Play

```text
217 准备 preview runtime package。
218 创建 EditorRuntimePlayInstance。
加载复杂打飞机 active scene。
tick 3 帧。
GameView report 显示 frame_count=3、scene_id、renderable_count、ui_draw_item_count。
```

### 12.2 再次点击 Play，无内容变化

```text
217 cache hit。
218 重新创建或复用 runtime instance。
tick frame。
报告显示 preview cache hit + game view runner completed。
```

### 12.3 Stop

```text
Stop 请求进入 PlaySessionController。
EditorRuntimePlayInstance 被释放。
GameViewPresentReport stop_status=stopped。
```

### 12.4 运行包加载失败

```text
PreviewPackageReport 可能成功。
EditorRuntimePlayInstance load_runtime_package 失败。
PlaySessionReport state=failed。
GameViewPresentReport status=failed。
diagnostic 指向 runtime_package_path。
```

## 13. AI-first 报告必须回答的问题

```text
这次 Play 用的是哪个 Preview RuntimePackage？
Preview package 是 cache hit 还是 rebuild？
EditorRuntimePlayInstance 是否创建成功？
RuntimePackage load 是否成功？
Active scene hydration 是否成功？
tick 了几帧？
GameView 是否收到 runtime frame descriptor？
Stop 是否清理了 instance？
失败发生在 preview / load / hydrate / tick / present / stop 哪一层？
哪些能力仍 deferred？
```

## 14. 拟施工 Gate

### Gate A：schema / report / mode

目标：

```text
新增 EditorRuntimePlayRequest / GameViewRuntimeFrame / GameViewPresentReport。
PlaySessionReport 增加 game view link 字段。
新增 PlayRunner trait 或等价 runner 抽象。
DefaultGameRunOrchestrator 作为 HeadlessGate runner 实现该抽象。
EditorGameViewPlayRunner 作为 WindowedUserRun / EditorGameView runner 实现该抽象。
WindowedUserRun 改为可接 A-min runner，不再固定 not implemented。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_gameview_play_schema
cargo test -p editor_core play_session
```

### Gate B：EditorRuntimePlayInstance

目标：

```text
实现 load RuntimePackage -> hydrate world -> tick frame 的 in-process runtime instance。
不接 GPU texture，不接 input。
集成 ProjectUiStateSnapshotProducer + font atlas AUI present summary。
输出 GameViewRuntimeFrame。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_runtime_play_instance
cargo test -p engine_runtime engine_host_loop
```

### Gate C：EditorGameViewPlayRunner / PlayService integration

目标：

```text
PlayService 在项目已打开时仍先走 217 preview package。
随后以 PlaySessionMode::WindowedUserRun / EditorGameView runner 创建 runtime instance。
Stop 清理 runtime instance。
Console 输出 GameView report path。
```

测试：

```powershell
cd rust
cargo test -p editor_core play_service
cargo test -p editor_core editor_gameview_play_runner
```

### Gate D：GameView model / Report Panel

目标：

```text
ui_model_composer 将 last GameViewRuntimeFrame 写入 ViewportModel。
Report Panel 增加 game_view_present provider。
GameView 不再只是 TestTexture placeholder，而能显示 runtime frame descriptor evidence。
本 Gate 仍不声明 real_gpu_texture_present；texture_descriptor_status=descriptor_only。
```

测试：

```powershell
cd rust
cargo test -p editor_core ui_model
cargo test -p editor_core report_panel
cargo test -p editor_window_winit viewport_runtime
```

### Gate E：project_e2e_gate

目标：

```text
新增 complex-shooter-editor-gameview-play-runner-productization-report.json。
打开 samples/complex_shooter_project。
点击 Play 走 preview package。
创建 in-process runtime instance。
tick frame_count > 0。
GameViewPresentReport 链接 PreviewPackageReport 和 PlaySessionReport。
断言 GameViewRuntimeFrame.frame_hash / renderable_count / ui_draw_item_count / texture_descriptor_status。
Stop 清理 instance。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate editor_gameview_play_runner
```

## 15. 验收标准

A-min 必须满足：

```text
项目已打开时 Play 使用 217 Preview RuntimePackage。
WindowedUserRun / EditorGameView runner 不再返回 not implemented。
Editor 进程内可创建 RuntimePlayInstance。
RuntimePlayInstance 只从 RuntimePackage load，不读项目源目录。
RuntimePlayInstance 能 hydrate active scene 并 tick frame_count > 0。
GameViewPresentReport 写出并链接到 PlaySessionReport。
GameView model 能看到 runtime frame descriptor。
Stop 能清理 runtime instance。
project_e2e_gate 能证明 complex shooter sample 走过 218 链路。
```

禁止冒充：

```text
用 HeadlessGate report 冒充 GameView runner。
用 TestTexture placeholder 冒充 runtime frame。
Runtime 读取 EditorSession scene document 内存绕过 RuntimePackage。
把 runtime_player_winit 强行依赖进 editor_core。
报告写 real_gpu_texture_present 但没有真实 texture evidence。
Stop 只改 UI 状态但不释放 runtime instance。
```

## 16. 自审

### 16.1 是否违背用户选择方案 A

```text
不违背。
方案 A 被确定为总方向。
A-min 是方案 A 的第一阶段，不是改选 B 或 C。
```

### 16.2 为什么不能一次完成完整 A

```text
完整 A 同时涉及 runtime lifecycle、renderer texture、editor window GPU resource、input bridge、AUI interaction、focus、pause/step、layout。
一次施工会扩大范围并降低可验证性。
A-min 先证明 in-process runtime session 和 GameView contract，后续逐步替换 descriptor 为真实 texture、再接 input。
```

### 16.3 是否和 RuntimePackage 真相冲突

```text
不冲突。
218 的 runtime instance 只加载 217 准备出的 Preview RuntimePackage。
不读项目源目录，不读 editor unsaved memory。
```

### 16.4 是否增加太多结构

```text
新增结构集中在 Editor Play/GameView 边界。
不新增 runtime 架构层。
不新增项目侧逻辑层。
结构的目的只是隔离 editor runtime session、GameView present、report 三个职责。
```

### 16.5 是否适合复杂打飞机项目

```text
适合。
复杂打飞机最需要高频 Play 反馈。
217 解决运行包准备速度和证据；218 A-min 开始把反馈接进 Editor GameView。
后续 A2/A3 完成后，用户可以在编辑器内看到 HUD、操作输入、验证 UI interaction 和 gameplay。
```

## 17. 结论

```text
218 采用方案 A 总方向。
第一轮只施工 A-min-gameview。
不要一次性实现完整 Unity-like GameView。
施工文档必须按 A0/A1/A2-min/Report/e2e 分 Gate，并明确真实 GPU texture、完整输入、Pause/Step、Maximize on Play 是后续阶段。
```
