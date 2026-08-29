# 167-Exported Windows Player Process Verification Gate v1 方案

## 1. 问题是什么

`Complex Shooter Real Project End-to-End Gate v1` 已经证明：

```text
sample project
-> DesktopExportPipeline
-> RuntimePackageBuilder
-> exported package
-> RuntimePackage load
-> headless player run
-> report
```

但它还没有证明真实用户路径：

```text
用户拿到 Build/Windows/dev
-> 启动 Game.exe
-> Game.exe 从自己旁边读取 package-manifest.json / data/runtime_package
-> Game.exe 进入 PlayerHost
-> 创建窗口或 headless verification host
-> 连续运行
-> 写 report / screenshot artifact
```

如果 gate 直接调用 `runtime_player_winit::run_windowed_native_player_from_package()`，它只能证明内部模块能跑，不能证明导出的 `Game.exe`、工作目录、相对路径、进程生命周期和报告路径正确。

所以本系统的真实问题是：

```text
必须建立一个以导出的 Game.exe 进程为最终通过依据的验证 gate。
```

## 2. 其他引擎怎么做

### UE

UE 既有 runtime screenshot，也有 automation artifact：

```text
Engine/Source/Runtime/Engine/Private/UnrealClient.cpp
  FScreenshotRequest::RequestScreenshot

Engine/Source/Runtime/Engine/Private/Tests/AutomationCommon.cpp
  SaveWindowAsScreenshot

Engine/Source/Developer/AutomationController
  screenshot comparison artifact / automation report
```

UE 的做法不是只测渲染函数，而是把 standalone / packaged / automation run 的截图和报告作为验收资产。

### Unity

Unity 的相关路径包括：

```text
BuildPipeline / BuildReport
PlayerConnection
ScreenCapture / PlayerLoop.UpdateCaptureScreenshot
```

Unity 的真实产品验收会区分 Editor PlayMode 和 Player 产物。Player 产物可以通过命令行、日志、PlayerConnection、截图等方式被自动化验证。

### Bevy

Bevy 的对应设计：

```text
bevy_winit
  window / event loop / runner

bevy_dev_tools/ci_testing
  frame-based screenshot / app exit
```

Bevy 也不是绕开 runner 直接调用 renderer，而是让 app 在 runner 路径里按帧触发 screenshot / exit。

### Godot

Godot 的平台层和渲染服务分离：

```text
platform/windows/display_server_windows.*
  window / native handle / input

servers/rendering*
  rendering server
```

导出模板和平台 main loop 才是真实用户路径，不能用内部场景函数替代。

## 3. 方案对比

| 方案 | 做法 | 优点 | 缺点 | 结论 |
|---|---|---|---|---|
| A | gate 直接调用 `runtime_player_winit` 内部函数 | 快 | 不验证 Game.exe、工作目录、导出目录结构、进程生命周期 | 不选 |
| B | 只运行 `cargo test --features real-window` ignored smoke | 能测 native host | 仍不是导出产物路径，也不是用户启动路径 | 不选为最终 gate |
| C | 新增 exported player process verification，启动导出的 Game.exe | 真实验证导出产物、路径、进程、报告 | 需要补 CLI 验证模式和进程 gate | 推荐 |

## 4. 推荐方案：C-min

系统名称：

```text
Exported Windows Player Process Verification Gate v1
```

核心路径：

```text
ExportedPackageDir
  package-manifest.json
  Game.exe
  data/runtime_package
  reports/

VerificationGate
  -> external command: Game.exe verify-exported-player --mode headless-gate|windowed --frames N
  -> verifier spawn child Game.exe run-native-player --mode headless-gate|windowed --frames N --report reports/windowed-player-run-report.json
  -> wait process
  -> read child report
  -> produce exported-player-process-verification-report.v1
```

关键规则：

```text
1. 最终通过依据必须来自导出的 Game.exe 进程。
2. Game.exe 默认正常运行不自动退出、不自动截图、不注入测试输入。
3. 验证运行必须显式使用 verify-exported-player / --verify-exported-player。
4. 验证模式只影响 PlayerHost 外壳：frame_limit、report_path、可选 screenshot / input script。
5. RuntimePackage、EngineHostLoop、Renderer/RHI、Input route 必须与正常运行共用同一套逻辑。
6. gate 不允许把项目玩法概念写进 engine_runtime。
7. 第一版默认跑 process + headless-gate，证明导出 exe / 相对路径 / report / runtime package 真实可用。
8. real-window 模式作为同一进程 gate 的可选模式；如果环境或 feature 不支持，报告 environment_blocked / feature_not_enabled，不伪装 passed。
```

## 5. 第一版边界

第一版做：

```text
Windows package directory verification
Game.exe existence / executable path verification
spawn Game.exe process
default package discovery from exe_dir/data/runtime_package
finite frame verification mode
child windowed-player-run-report.json
parent exported-player-process-verification-report.v1
process stdout/stderr capture summary
headless-gate automated test
windowed mode structured blocked report
```

第一版不做：

```text
installer / signing / store package
多平台
长时间 soak test
真实用户输入录制回放
图像黄金图 pixel diff
强制所有 CI 跑真实 OS window
```

## 6. 为什么适合我们

AI 友好：

```text
报告可以明确定位失败层：
package_manifest_missing
game_exe_missing
process_spawn_failed
process_timeout
child_report_missing
child_report_parse_failed
runtime_package_failed
present_failed
environment_blocked
```

复杂项目友好：

```text
gate 验证导出的完整目录，不依赖测试 fixture。
复杂打飞机项目越复杂，验证路径仍然不变。
```

长期维护友好：

```text
正常运行和验证运行共享 Runtime / Renderer / Input。
差异只在 PlayerHost 外壳，不污染项目逻辑。
```

简单：

```text
第一版只做 Windows、单进程、有限帧、结构化报告。
真实窗口和截图继续作为同一 gate 的后续增强，不另起一套路径。
```

效率：

```text
验证模式只在启动和退出策略上有额外开销，不改变正常游戏运行时性能路径。
```

## 7. 方案自审

```text
Specification fit:
  满足用户要求：测试必须走真实导出的 Game.exe，不用内部函数冒充真实路径。

Rule fit:
  符合 AI-first、复杂项目、长期维护、简单优先原则；不新增项目玩法 API。

Textual consistency:
  明确区分 NormalRun 与 VerificationRun；最终通过依据是导出进程报告。

Design fit:
  延续 117/118/133/134 的 native player / platform host 分层，不把 window/surface 下沉到 engine_runtime。

Implementation feasibility:
  runtime_cli 已有 run-native-player、默认 data/runtime_package 解析和 report 输出雏形，可直接扩展。

Practical reasonableness:
  第一版先验证进程和 headless-gate 真实路径，real-window 环境差异以结构化 blocked 处理，不伪装通过。
```

结论：

```text
本方案通过自审，可以生成施工文档并开始施工。
```
