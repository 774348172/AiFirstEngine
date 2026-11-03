# 117-当前可自动化施工文档-Runtime WGPU Surface 注入 / Windowed Player Present v1

## 目标

把真实 `wgpu::Surface` 接入 Runtime `RealWgpuBackend`：

```text
WindowedRuntimeHost
  -> winit Window / wgpu Surface
  -> RuntimeRenderer
  -> RhiCommandPlan v2
  -> RealWgpuBackend
  -> submit / present
```

## 施工步骤

### A1：RealWgpuBackend surface 执行入口

```text
新增 surface target 执行方法：
  execute_plan_to_surface_texture / render_to_surface_view

保留 offscreen 测试入口。
```

验收：

```text
cargo check -p engine_runtime --features real-wgpu
cargo test -p engine_runtime --features real-wgpu wgpu_backend
```

### A2：WindowedRuntimeHost real surface path

```text
feature = real-window,real-wgpu-surface
创建 winit Window。
创建 wgpu Instance / Surface / Adapter / Device / Queue。
configure surface。
tick runtime。
执行 RhiCommandPlan 到 surface。
present。
```

验收：

```text
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

### A3：Report 接入

```text
WindowedRuntimePresentReport 记录：
  window
  surface
  runtime_tick_status
  rdg_status
  rhi_status
  acquire_status
  submit_status
  present_status
  diagnostics
```

验收：

```text
cargo test -p editor_window_winit windowed_runtime_present
```

### A4：真实 smoke gate

```text
real_windowed_runtime_present_smoke
windowed_end_to_end_game_smoke

保持 ignored，本机 GUI/GPU gate 才手动跑。
```

验收：

```text
cargo test -p editor_window_winit --features real-window,real-wgpu-surface real_windowed_runtime_present_smoke -- --ignored --nocapture
```

### A5：回归

```powershell
cargo fmt
cargo test -p engine_runtime
cargo test -p runtime_cli
cargo test -p editor_window_winit
cargo check -p editor_host --features real-window
cargo check -p editor_host --features real-window,real-wgpu-surface
cargo check -p engine_runtime --features real-wgpu
cargo test -p engine_runtime --features real-wgpu wgpu_backend
```

## 完成后归档

```text
施工文档/已完成/117-当前可自动化施工文档-Runtime-WGPU-Surface注入-WindowedPlayerPresent-v1.md
阶段完成记录/2026-06-30-Runtime-WGPU-Surface注入-WindowedPlayerPresent-v1/00-总览.md
```
