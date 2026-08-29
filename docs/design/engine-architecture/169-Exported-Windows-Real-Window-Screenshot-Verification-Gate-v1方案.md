# 169-Exported Windows Real Window / Screenshot Verification Gate v1 方案

## 1. 问题是什么

`167-Exported-Windows-Player-Process-Verification-Gate-v1方案.md` 已经完成导出 `Game.exe` 的进程级验证：

```text
exported package
  -> Game.exe verify-exported-player
  -> child Game.exe run-native-player --mode headless-gate
  -> windowed-player-run-report.json
  -> exported-player-process-verification-report.json
```

但它还没有证明用户真实能看到画面：

```text
Game.exe
  -> 真实 OS window
  -> 真实 GPU surface
  -> RuntimePackage / World / Renderer / RHI
  -> present
  -> screenshot artifact
```

本系统解决 Q3 / Q6 / Q26 的剩余缺口：导出的 Windows `Game.exe` 必须能在显式 verification mode 下创建真实窗口、present 多帧、写出截图证据和结构化报告。

## 2. 其他引擎怎么做

Unreal Engine：

```text
FScreenshotRequest
Automation screenshot artifact
Automation report
Viewport / Player 路径截图
```

UE 的启发是：截图是 viewport/player 路径上的自动化证据，不是普通运行时逻辑。

Unity：

```text
BuildPipeline / BuildReport
Player build
PlayerConnection
ScreenCapture.CaptureScreenshot
```

Unity 的启发是：构建产物验证和 Player 运行验证是两类证据；截图证明的是 Player 已经真实运行和渲染。

Bevy：

```text
bevy_winit
app runner / window event loop
bevy_dev_tools::ci_testing Screenshot / ScreenshotAndExit
```

Bevy 的启发是：window runner 管窗口和退出，screenshot 是显式测试事件。

Godot：

```text
DisplayServer
RenderingServer / RenderingDevice
SceneDebugger screenshot request
```

Godot 的启发是：窗口、渲染服务、调试截图分层，不把截图能力污染项目逻辑。

## 3. 方案对比

| 方案 | 做法 | 优点 | 缺点 | 判断 |
|---|---|---|---|---|
| A | 继续只做 headless / process report | 稳定、自动化容易 | 不能证明真实窗口和真实 surface | 不选 |
| B | 只做 runtime_player_winit 模块级 real-window smoke | 能验证 native host 模块 | 不是导出 `Game.exe` 的最终用户路径 | 只作为子测试 |
| C-min | 在 exported `Game.exe` verification 中加入 real-window + screenshot gate | 证据链最真实，边界接近 Unity/UE | 需要补截图 readback / PNG / report 字段 | 推荐 |

## 4. 推荐方案：C-min

标准流程：

```text
Game.exe verify-exported-player --mode windowed --screenshot --frames N
  -> verifier validates exported package
  -> verifier spawns exported Game.exe run-native-player --mode windowed --screenshot --frames N
  -> runtime_cli enters runtime_player_winit
  -> runtime_player_winit creates OS window + wgpu Surface
  -> RuntimePackage load
  -> World hydration
  -> EngineHostLoop tick
  -> RuntimeRenderer / RHI command plan
  -> RealWgpuBackend renders to surface view
  -> screenshot readback writes PNG artifact
  -> surface present
  -> child windowed-player-run-report.json
  -> parent exported-player-process-verification-report.json
```

## 5. 架构规则

```text
1. 最终通过依据必须来自导出目录内的 Game.exe 进程。
2. 普通玩家运行不自动截图、不自动退出、不注入测试输入。
3. --screenshot 只在 verification / explicit CLI 模式下生效。
4. engine_runtime 不依赖 winit，不创建 OS window，不拥有 wgpu::Surface。
5. runtime_player_winit 是正式 Player Host，负责 OS window / event loop / surface / screenshot readback。
6. RuntimePackage / World / EngineHostLoop / RuntimeRenderer / RHI 必须与正常运行共用。
7. screenshot artifact 是验证证据，不是运行时真相层。
8. 真实窗口环境不可用时必须报告 environment_blocked / feature_not_enabled，不伪装 passed。
9. headless-gate 默认自动化路径继续保留；windowed screenshot gate 显式运行。
10. 不新增项目玩法 API，不把 enemy / bullet / score 等项目概念写进引擎底座。
```

## 6. 数据结构边界

`NativePlayerWindowRunRequest` 新增可选截图请求：

```text
screenshot:
  enabled: bool
  path: Option<PathBuf>
  frame_index: u64
```

`NativeWindowHostReport` 新增截图摘要：

```text
screenshot:
  requested: bool
  status: not_requested | captured | write_failed | readback_failed | unsupported
  path: Option<String>
  width: u32
  height: u32
  format: String
  byte_size: Option<u64>
```

`ExportedPlayerProcessVerificationReport` 新增父报告字段：

```text
screenshot_requested: bool
screenshot_status: Option<String>
screenshot_path: Option<String>
```

## 7. 第一版边界

第一版必须做：

```text
runtime_player_winit 支持 screenshot request/report 字段。
real-window feature 下对 surface render result 做 GPU readback。
保存 PNG artifact。
runtime_cli run-native-player 支持 --screenshot / --screenshot-path。
runtime_cli verify-exported-player 支持 --screenshot / --screenshot-path。
exported Game.exe process integration test 覆盖参数和报告字段。
```

第一版不做：

```text
golden image / pixel diff
多截图 / 多窗口 / 长时间 soak
录制视频
截图压缩格式选择
跨平台截图后端
默认 CI 强制真实窗口
```

## 8. 为什么适合我们

AI 友好：

```text
AI 可以直接通过 report 判断失败发生在 package / window / surface / RHI / screenshot / present 哪一层。
```

复杂项目友好：

```text
无论项目多复杂，验证入口不变：导出 Game.exe + RuntimePackage + PlayerHost。
```

长期维护：

```text
真实窗口和截图留在 runtime_player_winit。
未来 D3D12 / Vulkan / Metal backend 可以替换 RHI 后端，不推翻上层验证流程。
```

简单度：

```text
第一版只做单窗口、单 surface、单截图。
不做 pixel diff，不把测试通道变成项目逻辑规则。
```

效率：

```text
截图 readback 只在 explicit verification mode 执行。
普通运行路径不承担 readback 和 PNG 写盘成本。
```

## 9. 方案自审

```text
Specification fit:
  满足用户确认的方案 C：从导出的 Game.exe 路径验证真实窗口、surface、截图证据。

Rule fit:
  继承 117/118/134/167，不让 engine_runtime 拥有 OS window，不新增项目玩法 API。

Textual consistency:
  对外入口、child process、PlayerHost、report artifact 关系清晰，没有把模块测试冒充最终 gate。

Design fit:
  符合 AI-first、复杂项目、长期维护、简单度和效率优先级。

Implementation feasibility:
  runtime_player_winit 已有 real-window surface present 基础，C-min 可在其上增加截图 artifact。

Practical reasonableness:
  C-min 不做黄金图和多平台复杂功能，第一版可通过结构化测试和 feature-gated real-window smoke 渐进验证。
```

结论：

```text
本方案通过自审，可以生成施工文档并开始施工。
```
