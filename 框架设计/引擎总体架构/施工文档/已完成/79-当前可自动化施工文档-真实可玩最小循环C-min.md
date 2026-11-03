# 当前可自动化施工文档：真实可玩最小循环 C-min

> 状态：已确认可施工。本文档只定义本轮施工范围。长期规则见 `../../79-真实可玩最小循环C-min方案.md`。

## 1. 施工目标

实现真实可玩最小循环 C-min：

```text
Native Window / HeadlessWindow
  -> EngineHostLoop
  -> Input Pump
  -> InputActionSnapshot
  -> Runtime / Game Thread
  -> ECS World
  -> ProjectLogicRunner
  -> RenderExtract
  -> RenderCommandQueue
  -> Render Thread
  -> RenderSceneState
  -> RuntimeRenderer
  -> RDG / RHI
  -> Surface Present
  -> WindowedContinuousRuntimeReport
```

第一版重点不是做复杂玩法，而是验证最终架构边界能连续运行。

## 2. 本轮必须保持的长期边界

```text
EngineHostLoop 是最高心跳源。
Runtime / Game Thread 是 ECS World 唯一写入 owner。
ProjectLogicRunner 是项目逻辑进入 ECS 的正式入口。
RenderExtract 是 Game -> Render 的唯一边界。
RenderThread 拥有 RenderSceneState。
RuntimeRenderer 不直接读取 ECS。
RuntimeRenderer 不绕过 RDG / RHI。
headless gate 和真实 windowed gate 必须走同一条主流程。
```

## 3. 本轮最小功能

只做：

```text
一个窗口或 headless surface。
一个 Runtime Viewport。
一个 Runtime Scene。
一个可移动 Entity。
最小 InputActionSnapshot。
连续运行 N 帧。
输入驱动 Transform 变化。
Transform 变化进入 RenderCommand / RenderSceneState。
RuntimeRenderer 产出 present report。
结构化 WindowedContinuousRuntimeReport。
```

不做：

```text
多窗口。
完整 Play Mode。
完整 GPU Resource Pool。
完整跨线程高性能队列。
完整材质 / shader pipeline cache。
完整场景切换。
完整热更。
真实平台发布。
```

## 4. 建议代码落点

优先复用现有 Rust crate：

```text
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/engine_runtime/src/render_thread.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/render_scene_state.rs
rust/crates/engine_runtime/src/render_command.rs
rust/crates/engine_runtime/src/project_logic_runner.rs
rust/crates/editor_window_winit/src/windowed_runtime_present.rs
rust/crates/editor_window_winit/src/lib.rs
rust/crates/runtime_cli/src/lib.rs
```

如果现有文件名略有差异，以 `rg` 搜索现有类型为准，不新建第二套 runtime。

允许新增：

```text
rust/crates/engine_runtime/src/windowed_continuous_runtime.rs
rust/crates/engine_runtime/src/windowed_continuous_runtime_report.rs
```

仅当 `editor_window_winit` 需要承载真实窗口入口时，允许新增：

```text
rust/crates/editor_window_winit/src/continuous_runtime_window.rs
```

## 5. 数据结构

新增或补齐：

```text
WindowedContinuousRuntimeRequest:
  frame_count
  fixed_dt
  input_script
  scene_id
  viewport
  backend_kind

WindowedContinuousRuntimeReport:
  ok
  frame_count
  frames[]
  diagnostics[]

WindowedContinuousFrameReport:
  frame_index
  input
  runtime
  logic
  ecs
  render_extract
  render_thread
  renderer
  present
  frame_hash
```

诊断字段：

```text
RuntimeDiagnostic:
  system
  stage
  severity
  code
  message
  frame_index?
  entity_id?
  asset_id?
  command_id?
```

## 6. 最小测试场景

场景：

```text
一个 Entity，带 Transform 和最小可渲染组件。
初始位置 x=0。
输入脚本在第 1-3 帧发送 move_right。
ProjectLogicRunner 每帧根据 InputActionSnapshot 修改 Transform.x。
RenderExtract 生成 TransformChanged / UpsertProxy 类型 RenderCommand。
RenderThread 更新 RenderSceneState。
RuntimeRenderer 输出 frame report。
```

期望：

```text
第 0 帧：x=0。
第 1 帧：x 增加。
第 2 帧：x 继续增加。
第 3 帧：x 继续增加。
第 4 帧：无输入时 x 保持。
RenderSceneState 最终位置等于 ECS 最终位置。
report.ok=true。
```

## 7. 施工步骤

### A1：梳理现有入口

检查并复用：

```text
EngineHostLoop tick。
ProjectLogicRunner 执行入口。
RenderExtract / RenderCommandQueue。
RenderThread submit / tick。
RuntimeRenderer output。
WindowedRuntimePresent C-min。
```

验收：

```text
不新增第二套 FrameLoop。
不新增第二套 RenderSceneState。
不新增第二套 InputActionSnapshot。
```

### A2：实现 WindowedContinuousRuntimeRequest / Report

实现纯数据结构和序列化。

验收：

```text
能在 headless 测试中构造 request。
能输出稳定 JSON report。
```

### A3：实现 Headless Continuous Runtime Gate

用 headless backend 连续运行 N 帧。

验收：

```text
同一条链路必须经过 EngineHostLoop -> ProjectLogicRunner -> RenderExtract -> RenderThread -> RuntimeRenderer。
```

### A4：接入最小输入脚本

把输入脚本转成 InputActionSnapshot。

验收：

```text
项目逻辑只能读 InputActionSnapshot。
不允许项目逻辑读取 raw window event。
```

### A5：实现可移动 Entity 测试

用一个最小规则让输入驱动 Transform 变化。

验收：

```text
ECS 最终 Transform 可断言。
RenderSceneState 最终 Transform 可断言。
```

### A6：接入真实窗口 smoke

在已有 feature gate 下运行真实窗口连续 N 帧。

验收：

```text
默认测试不依赖真实窗口。
真实窗口 smoke 可以 ignored。
失败时必须输出 diagnostics。
```

## 8. 测试门禁

默认必须通过：

```powershell
cd G:\gameEngin\rust
cargo fmt
cargo test --workspace
```

定向测试建议：

```powershell
cargo test -p engine_runtime windowed_continuous_runtime
cargo test -p editor_window_winit headless_windowed_continuous_runtime
```

feature 编译门禁：

```powershell
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

真实窗口 smoke：

```powershell
cargo test -p editor_window_winit --features real-window,real-wgpu-surface real_windowed_continuous_runtime_smoke -- --ignored --nocapture
```

## 9. 完成标准

```text
1. 新增 79 号真实可玩最小循环 C-min headless gate。
2. 连续 N 帧能稳定运行。
3. 输入能驱动 Entity Transform。
4. ECS 最终状态和 RenderSceneState 最终状态一致。
5. RuntimeRenderer / RDG / RHI / Present report 串入总 report。
6. 默认 cargo test --workspace 通过。
7. real-window / real-wgpu-surface feature 编译通过。
8. ignored 真实窗口 smoke 可运行或输出清晰 diagnostics。
9. 完成后新增阶段完成记录。
10. 完成后更新 49 / 54 / 18 / README / 00-文档地图 / 00-阅读顺序 / 施工文档 README。
```

## 10. 失败降级规则

如果真实窗口或 GPU 环境失败：

```text
不回退到绕过 RenderThread。
不回退到绕过 RDG / RHI。
不让默认测试依赖真实 GPU。
保留 headless gate。
把真实失败写入 diagnostics。
```

如果发现现有 runtime 入口不足：

```text
优先扩展现有 EngineHostLoop / RenderThread / RuntimeRenderer。
禁止新建第二套临时 runtime loop。
```
