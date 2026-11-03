# 134-当前可自动化施工文档-M4 Native Player WindowHost / Runtime Present Integration v1

## 施工目标

按照 `134-M4-Native-Player-WindowHost-RuntimePresentIntegration-v1方案.md` 实施第一版 Native Player WindowHost / Runtime Present 集成。

本阶段目标：

```text
runtime_cli run-native-player --mode windowed
  -> runtime_player_winit
  -> RuntimePackage load
  -> World hydration
  -> EngineHostLoop
  -> RuntimeRenderer/RHI command plan
  -> native window host report
```

默认测试路径保持 headless 可跑；真实 OS window / wgpu surface 走 feature-gated smoke。

## 不做什么

```text
不把 winit/wgpu surface 放入 engine_runtime。
不让 runtime_cli 直接依赖 editor_window_winit。
不为了可见画面手写临时 scene。
不新增项目玩法 API。
不重做编辑器 Play Mode。
```

## 涉及文件

```text
rust/Cargo.toml
rust/crates/runtime_player_winit/Cargo.toml
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/runtime_cli/Cargo.toml
rust/crates/runtime_cli/src/lib.rs
框架设计/引擎总体架构/134-M4-Native-Player-WindowHost-RuntimePresentIntegration-v1方案.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-01-M4-Native-Player-WindowHost-RuntimePresentIntegration-v1/00-总览.md
```

## Gate A：新增 runtime_player_winit crate

动作：

```text
新增 runtime_player_winit crate。
默认 feature 不依赖 winit/wgpu。
real-window feature 才启用 winit/wgpu/pollster 和 engine_runtime/real-wgpu。
```

验收：

```powershell
cargo test -p runtime_player_winit
```

## Gate B：Headless Native Player Host

动作：

```text
定义 NativePlayerWindowConfig。
定义 NativePlayerWindowRunRequest。
定义 NativeWindowHostReport。
实现 run_headless_native_player_from_package。
从 RuntimePackage load + hydrate World。
运行 EngineHostLoop。
用 RenderTarget::surface 跑 render_thread_for_target。
模拟 headless surface acquire/present。
```

验收：

```powershell
cargo test -p runtime_player_winit headless_native_player
cargo test -p runtime_player_winit native_window_host_report
```

## Gate C：Feature-disabled Windowed 报告

动作：

```text
默认 feature 下提供 run_windowed_native_player_from_package。
它不假装成功，返回 feature_not_enabled / native_window_host_required 结构化报告。
```

验收：

```powershell
cargo test -p runtime_player_winit feature_disabled_windowed
```

## Gate D：真实 Windowed Host feature

动作：

```text
real-window feature 下创建 winit EventLoop / OS Window / wgpu Surface。
从 RuntimePackage hydrate World。
EngineHostLoop 生成 RHI command plan。
RealWgpuBackend::execute_plan_to_surface_view。
surface_texture.present。
```

验收：

```powershell
cargo check -p runtime_player_winit --features real-window
cargo test -p runtime_player_winit --features real-window real_windowed_player_smoke -- --ignored --nocapture
```

说明：

```text
真实窗口 smoke 可因本机策略、显卡、远程桌面环境失败。
失败时必须返回 environment_blocked / surface_failed 报告，不影响默认 headless 回归。
```

## Gate E：runtime_cli 接入

动作：

```text
runtime_cli 增加 optional dependency runtime_player_winit。
runtime_cli real-window feature 启用 runtime_player_winit/real-window。
run-native-player --mode windowed:
  feature enabled -> runtime_player_winit::run_windowed_native_player_from_package
  feature disabled -> 保持 M3 native host required 报告
run-native-player --headless-gate 不变。
```

验收：

```powershell
cargo test -p runtime_cli
cargo check -p runtime_cli --features real-window
```

## Gate F：总回归

运行：

```powershell
cargo fmt --check
cargo test -p runtime_player_winit
cargo test -p runtime_cli
cargo test -p engine_runtime windowed_player
cargo test -p engine_runtime default_game_run
cargo test -p editor_core desktop_export
cargo test -p engine_runtime
cargo test -p editor_core
cargo test -p editor_host
```

## 完成后文档动作

```text
施工文档/当前/134-当前可自动化施工文档-M4-Native-Player-WindowHost-RuntimePresentIntegration-v1.md
  -> 施工文档/已完成/134-当前可自动化施工文档-M4-Native-Player-WindowHost-RuntimePresentIntegration-v1.md

新增：
阶段完成记录/2026-07-01-M4-Native-Player-WindowHost-RuntimePresentIntegration-v1/00-总览.md

更新：
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
施工文档/README.md
```

## 施工文档自审

检查结果：

```text
施工目标与 134 方案一致。
没有要求 engine_runtime 依赖 window。
没有要求 runtime_cli 依赖 editor_window_winit。
默认测试不需要弹窗。
真实窗口测试 feature-gated / ignored。
每个 gate 都有测试命令。
完成后文档归档动作明确。
```

暂未发现阻塞施工的遗漏。
