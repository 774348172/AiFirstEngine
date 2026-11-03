# 170-当前可自动化施工文档-Exported Windows Real Window / Screenshot Verification Gate v1

## 1. 施工目标

依据：

```text
169-Exported-Windows-Real-Window-Screenshot-Verification-Gate-v1方案.md
167-Exported-Windows-Player-Process-Verification-Gate-v1方案.md
117-Runtime-WGPU-Surface注入-WindowedPlayerPresent-v1方案.md
118-WindowedPlayer-Runtime-v1完整方案.md
134-M4-Native-Player-WindowHost-RuntimePresentIntegration-v1方案.md
```

目标：

```text
在已完成的 exported Game.exe process gate 上，增加真实 windowed screenshot verification 通道。
普通运行路径不自动截图。
verification mode 显式开启截图后，导出 Game.exe 能创建真实窗口、present、写 PNG artifact、写结构化报告。
```

## 2. 涉及文件

代码：

```text
rust/crates/engine_runtime/src/windowed_player.rs
rust/crates/engine_runtime/src/wgpu_backend.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/runtime_cli/src/lib.rs
rust/crates/runtime_cli/src/exported_player_verification.rs
rust/crates/runtime_cli/tests/exported_player_process_verification.rs
```

文档：

```text
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-03-Exported-Windows-Real-Window-Screenshot-Verification-Gate-v1/00-总览.md
```

## 3. 分阶段施工

### A. runtime_player_winit 截图请求与报告字段

任务：

```text
新增 NativeWindowScreenshotRequest。
新增 NativeWindowScreenshotReport。
NativePlayerWindowRunRequest 增加 screenshot 字段。
NativeWindowHostReport 增加 screenshot 字段。
headless / feature_not_enabled / missing package 路径给出稳定 screenshot status。
```

测试：

```powershell
cargo test -p runtime_player_winit screenshot
```

### B. runtime_cli 参数接入

任务：

```text
run-native-player 支持 --screenshot。
run-native-player 支持 --screenshot-path <png>。
verify-exported-player 支持 --screenshot。
verify-exported-player 支持 --screenshot-path <png>。
child process 参数透传到导出 Game.exe run-native-player。
parent report 汇总 screenshot_requested / screenshot_status / screenshot_path。
```

测试：

```powershell
cargo test -p runtime_cli screenshot
cargo test -p runtime_cli --test exported_player_process_verification
```

### C. real-window 截图 readback / PNG 写出

任务：

```text
real-window feature 下渲染到可 copy 的 offscreen texture。
GPU readback RGBA bytes。
写 PNG artifact。
截图失败写入 NativeWindowHostReport diagnostics。
```

约束：

```text
不让 engine_runtime 依赖 winit。
不把截图 readback 放入普通运行默认路径。
不做 golden image / pixel diff。
```

测试：

```powershell
cargo check -p runtime_cli --features real-window
cargo test -p engine_runtime --features real-wgpu screenshot
```

真实窗口 smoke 可按环境执行：

```powershell
cargo test -p runtime_player_winit --features real-window real_windowed_player_screenshot_smoke -- --ignored --nocapture
```

### D. exported Game.exe windowed screenshot gate

任务：

```text
扩展 exported process integration test。
覆盖 Game.exe verify-exported-player --mode windowed --screenshot 参数和报告。
未启用 real-window feature 时必须得到 feature_not_enabled / failed report，不伪装 passed。
启用 real-window feature 且本机环境允许时，能写出 screenshot png。
```

测试：

```powershell
cargo test -p runtime_cli --test exported_player_process_verification
cargo test -p runtime_cli
```

### E. 文档收口

任务：

```text
生成阶段完成记录。
将本施工文档移动到 施工文档/已完成/。
更新 00 / 49 / 54 / 149 / 施工 README / 阶段 README。
明确真实窗口 screenshot gate 的通过范围和残留风险。
```

整体测试：

```powershell
cargo test -p runtime_player_winit
cargo test -p runtime_cli
cargo test -p project_e2e_gate
cargo fmt --check
```

## 4. 施工自审

```text
Specification fit:
  覆盖方案 C-min：导出 Game.exe 真实窗口、surface、screenshot artifact、结构化报告。

Rule fit:
  不让 engine_runtime 依赖 winit，不新增项目玩法 API，不绕过 RuntimePackage。

Textual consistency:
  A-E 从请求、报告、CLI、real-window readback、exported process gate 到文档收口闭合。

Design fit:
  保持 AI-readable report；真实窗口错误可定位到 window / surface / RHI / screenshot / present。

Implementation feasibility:
  已有 runtime_player_winit real-window present 基础，C-min 可在其上增加截图 artifact。

Practical reasonableness:
  默认测试不强制真实窗口；真实窗口 smoke 为 feature-gated / ignored，避免环境差异造成假失败。
```

结论：

```text
施工文档通过自审，可以开始施工。
```
