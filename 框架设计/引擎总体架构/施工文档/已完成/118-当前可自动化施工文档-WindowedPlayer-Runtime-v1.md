# 118-当前可自动化施工文档-Windowed Player Runtime v1

## 目标

按照 `118-WindowedPlayer-Runtime-v1完整方案.md` 实施完整 Player Runtime 主线。

执行规则：

```text
一次只施工一个 gate。
每个 gate 完成后必须跑对应测试。
测试通过后才能进入下一个 gate。
如果某个 gate 暴露架构问题，先停下回填规则文档。
```

## Gate 1：WindowedPlayerHost 入口

目标：

```text
新增 WindowedPlayerHost / WindowedPlayerRunReport。
不依赖 EditorHost。
支持 headless 可测路径。
真实 windowed 路径先返回明确 feature/native host report，不做临时假窗口。
```

涉及文件：

```text
rust/crates/engine_runtime/src/windowed_player.rs
rust/crates/engine_runtime/src/lib.rs
```

测试：

```powershell
cargo test -p engine_runtime windowed_player
```

验收：

```text
PlayerHostRequest 可序列化。
Headless gate 能运行 RuntimePackage。
Windowed gate 在 engine_runtime 层不直接创建窗口，返回 native_host_required。
Report 包含 request/package/scene/world/render/present 分层状态。
```

## Gate 2：RuntimePackage Load

目标：

```text
WindowedPlayerHost 从真实 RuntimePackage 目录加载 package。
package 错误进入 report.package_status + diagnostics。
```

测试：

```powershell
cargo test -p engine_runtime windowed_player_package
```

## Gate 3：World Hydration

目标：

```text
RuntimeScene -> ECS World。
report 记录 entity_count / component_count / hydration_status。
```

测试：

```powershell
cargo test -p engine_runtime windowed_player_world
```

## Gate 4：Runtime Asset Load

目标：

```text
AssetRef -> RuntimeAssetRecord / CookedAssetRecord。
Texture / Mesh / Material 最小加载状态进入 report。
```

测试：

```powershell
cargo test -p engine_runtime windowed_player_asset
```

## Gate 5：GPU Resource Binding

目标：

```text
RuntimeAsset -> GPU resource descriptor。
重复资源可缓存。
缺失资源可诊断。
```

测试：

```powershell
cargo test -p engine_runtime windowed_player_gpu
```

## Gate 6：RenderProjection

目标：

```text
ECS render-facing components -> RenderProxy typed payload。
```

测试：

```powershell
cargo test -p engine_runtime windowed_player_projection
```

## Gate 7：RuntimeRenderer

目标：

```text
RenderProxy -> RenderGraph -> RhiCommandPlan。
可见对象生成 draw command。
```

测试：

```powershell
cargo test -p engine_runtime windowed_player_renderer
```

## Gate 8：RealWgpuBackend Present

目标：

```text
RhiCommandPlan -> RealWgpuBackend -> surface present。
真实窗口 smoke 保持 ignored。
```

测试：

```powershell
cargo test -p editor_window_winit --features real-window,real-wgpu-surface real_windowed_runtime_present_smoke -- --ignored --nocapture
```

## Gate 9：Player FrameLoop

目标：

```text
Time / Input / ECS Systems / Physics / Render 多帧顺序稳定。
```

测试：

```powershell
cargo test -p engine_runtime windowed_player_frameloop
```

## Gate 10：Playable Sample Gate

目标：

```text
复杂打飞机样例作为引擎底座验收。
不新增项目专用引擎 API。
```

测试：

```powershell
cargo test -p engine_runtime complex_project_validation
```

## 总回归

每完成 2-3 个 gate 后跑：

```powershell
cargo fmt
cargo test -p engine_runtime
cargo test -p runtime_cli
cargo test -p editor_window_winit
cargo check -p editor_host --features real-window
cargo check -p editor_host --features real-window,real-wgpu-surface
```

## 完成后归档

```text
施工文档/已完成/118-当前可自动化施工文档-WindowedPlayer-Runtime-v1.md
阶段完成记录/2026-06-30-WindowedPlayer-Runtime-v1/00-总览.md
```
