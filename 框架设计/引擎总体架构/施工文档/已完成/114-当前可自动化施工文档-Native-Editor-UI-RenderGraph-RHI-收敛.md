# 114-当前可自动化施工文档：Native Editor UI RenderGraph / RHI 收敛

## 目标

把 Native Editor UI 当前的 WGPU 直连路径收敛为：

```text
UiDrawList
  -> UiGpuDrawPlan
  -> UiRenderGraph
  -> UiRhiCommandPlan
  -> WgpuBackend
```

## 实施范围

修改：

```text
rust/crates/editor_wgpu_renderer/src/lib.rs
框架设计/引擎总体架构/114-Native-Editor-UI-RenderGraph-RHI-收敛方案.md
```

## 已实施内容

```text
新增 UI_RENDER_GRAPH_SCHEMA_VERSION。
新增 UI_RHI_COMMAND_PLAN_SCHEMA_VERSION。
新增 UiRenderGraph / UiRenderPass / UiRenderResource。
新增 UiRhiCommandPlan / UiRhiCommand。
HeadlessUiGpuRenderer 改为走 DrawPlan -> RenderGraph -> RhiCommandPlan。
RealWgpuUiRenderer 改为先生成 RenderGraph / RhiCommandPlan，再由 WGPU backend 执行。
real-wgpu 测试改为验证 WGPU backend 消费 RHI command plan。
新增 RenderGraph / RHI command plan 单元测试。
```

## 验证

```powershell
cargo fmt
cargo test -p editor_wgpu_renderer
cargo test -p editor_wgpu_renderer --features real-wgpu
cargo test -p editor_window_winit
cargo check -p editor_host --features real-window
```

## 完成边界

已完成：

```text
Native Editor UI 内部不再是 DrawList 直接等于 WGPU 执行。
WGPU 被放在 backend 执行层。
headless 和 real-wgpu 共用 UiRenderGraph / UiRhiCommandPlan。
```

未完成：

```text
完整 EngineRHI trait。
Runtime Renderer 迁移到同一 EngineRHI。
D3D12 / Vulkan / Metal backend。
真实 UI texture sampling / viewport texture pass。
```
