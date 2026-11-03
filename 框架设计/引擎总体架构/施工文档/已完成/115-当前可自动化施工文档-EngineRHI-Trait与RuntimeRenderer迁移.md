# 115-当前可自动化施工文档：EngineRHI Trait 与 RuntimeRenderer 迁移

## 目标

把 Runtime Renderer 的 Sprite / Mesh present 链路迁到同一套 EngineRHI trait：

```text
RenderSceneState
  -> RuntimeRenderer
  -> RenderGraph
  -> RhiCommandPlan
  -> EngineRhiBackend
```

## 实施内容

```text
engine_rhi.rs:
  EngineRhiBackend 新增 begin_frame / clear / draw / submit / present / finish_report。
  execute_plan 改为默认编排方法。

headless_rhi_backend.rs:
  改为实现 EngineRhiBackend 方法集。
  通过方法调用累计 clear/draw/submit/present/report。

wgpu_backend.rs:
  unavailable skeleton 实现同一 EngineRhiBackend trait。

runtime_renderer.rs:
  新增 render_with_rhi_backend。
  MeshBasic / SpriteBasic 到 RHI command 的测试补齐。

runtime_cli/src/lib.rs:
  修正测试 runtime package fixture，补齐 scene-main 的 RuntimeAssetIndex。
```

## 验证

```powershell
cargo fmt
cargo test -p engine_runtime
cargo test -p runtime_cli
cargo test -p editor_window_winit
cargo check -p editor_host --features real-window
```

## 完成边界

已完成：

```text
EngineRhiBackend 不是单一 execute_plan 薄接口。
RuntimeRenderer 可注入任意 EngineRhiBackend。
Mesh / Sprite 都能走 RenderGraph -> RhiCommandPlan -> EngineRhiBackend::draw。
Headless 自动化 backend 继续可测。
```

未完成：

```text
真实 Runtime WGPU backend。
真实 GPU texture / mesh buffer / shader pipeline。
Windowed Runtime Player C-min。
```
