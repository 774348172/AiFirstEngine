# 219-当前可自动化施工文档：Editor GameView Full GPU Texture Sharing Productization v1

## 1. 施工目标

按 `219-Editor-GameView-Full-GPU-Texture-Sharing-Productization-v1方案.md` 的 `A-min-gpu-gameview` 落地第一版真实 GPU texture present 链路：

```text
Editor Play 仍使用 217 Preview RuntimePackage 真相。
EditorRuntimePlayInstance 仍由 218 提供 in-process runtime session。
Editor 主窗口持有共享 GPU context。
Runtime real-wgpu backend 使用同一 shared Device / Queue 渲染到 GameView offscreen texture。
Editor UI renderer 通过 DrawCommand::ViewportTextureSlot 采样该 texture 到 GameView rect。
GameViewPresentReport / 219 evidence 能区分 presented、fallback_placeholder、gpu_unavailable、failed。
默认自动化不依赖真实 OS window / GPU；真实 GPU smoke 作为 local-only ignored test。
```

必须吸收 `其它AI审查目录/37-219-Editor-GameView-Full-GPU-Texture-Sharing方案审查.md`：

```text
A-min 同步执行：runtime tick -> write texture -> UI sample -> surface present 在 editor 主窗口单帧顺序内执行。
本轮不启用 RenderThreadWorker 独立线程跨 crate 读写共享 texture。
EditorSharedGpuContext 使用共享所有权，RealWgpuUiRenderer 暴露 shared_context() accessor。
headless/mock 行为明确：无真实 GPU 时输出 gpu_unavailable / fallback report，不假装 presented。
复用 RealWgpuBackend::execute_plan_to_surface_view，不重造 RHI 渲染路径。
如 EditorRuntimePlayInstance 没有保留 RhiCommandPlan，补可访问的 last render output / plan 边界，但不把 wgpu handle 放入 editor_core。
resize 只在帧间执行，A-min 不处理异步 in-flight texture。
```

## 2. 涉及文件

预计新增：

```text
rust/crates/editor_wgpu_renderer/src/shared_gpu.rs
rust/crates/editor_wgpu_renderer/src/viewport_texture.rs
```

预计修改：

```text
rust/crates/editor_wgpu_renderer/src/lib.rs
rust/crates/editor_wgpu_renderer/src/real_wgpu.rs
rust/crates/editor_wgpu_renderer/src/draw_plan.rs
rust/crates/editor_wgpu_renderer/src/tests.rs
rust/crates/engine_runtime/src/wgpu_backend.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/viewport.rs
rust/crates/editor_window_winit/src/tests/viewport_runtime.rs
rust/crates/editor_window_winit/src/tests/native_real_window.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/219-Editor-GameView-Full-GPU-Texture-Sharing-Productization-v1方案.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-08-Editor-GameView-Full-GPU-Texture-Sharing-Productization-v1/00-总览.md
```

禁止修改/禁止实现：

```text
不要实现 GameView input / focus / AUI interaction。
不要实现 Pause / Resume / Step。
不要实现 Maximize on Play。
不要实现多 GameView / 多 RuntimeInstance。
不要实现跨进程 texture sharing。
不要把 runtime_player_winit 独立 EventLoop 嵌入 editor。
不要把 placeholder rect 或 descriptor_only 冒充 real_gpu_texture。
不要把 CPU readback / screenshot 当作主线 present。
不要让 editor_core 持有 wgpu::Device / Queue / Texture / TextureView / Sampler / BindGroup。
不要修改与本施工无关的既有工作区改动。
```

## 3. Gate A：shared GPU context / headless summary

任务：

```text
新增 EditorSharedGpuContext / EditorSharedGpuContextSummary。
RealWgpuUiRenderer 改为持有 shared context wrapper，并暴露 shared_context_summary() / shared_context()。
保留现有 rect/text present 行为。
无真实 GPU 的默认测试只验证 summary/report，不要求真实 Device。
```

测试：

```powershell
cd rust
cargo test -p editor_wgpu_renderer shared_gpu_context
cargo test -p editor_wgpu_renderer real_ui_present
```

## 4. Gate B：viewport texture registry

任务：

```text
新增 EditorViewportTextureRegistry。
支持 allocate_or_resize / resolve / mark_rendered / unregister_session。
texture_id 稳定，resize 时 generation 递增。
Stop/unregister 后旧 texture 不可被误采样。
缺 texture 返回 fallback diagnostic。
```

测试：

```powershell
cd rust
cargo test -p editor_wgpu_renderer viewport_texture_registry
```

## 5. Gate C：ViewportTextureSlot draw plan / fallback

任务：

```text
UiGpuDrawPlan 增加 UiGpuViewportTextureQuad。
DrawCommand::ViewportTextureSlot 优先生成 viewport_texture_quads。
没有 registry texture 时 fallback placeholder，并记录 fallback count / diagnostic。
现有 Rect/Text pipeline 不退化。
```

测试：

```powershell
cd rust
cargo test -p editor_wgpu_renderer viewport_texture_pipeline
cargo test -p editor_wgpu_renderer viewport_texture_fallback
```

## 6. Gate D：runtime backend shared texture target

任务：

```text
engine_runtime real-wgpu backend 新增 shared/borrowed device queue 接入边界。
复用 execute_plan_to_surface_view(plan, &TextureView) 渲染到 editor 分配的 texture view。
不让 engine_runtime 依赖 editor_wgpu_renderer。
RhiBackendReport 能说明 target_id / frame_index / presented 状态。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime wgpu_backend
cargo test -p engine_runtime runtime_renderer_viewport_texture
```

## 7. Gate E：EditorRuntimePlayInstance render output access / report state

任务：

```text
补 EditorRuntimePlayInstance last render output / RhiCommandPlan 可访问边界。
GameViewRuntimeFrame / GameViewPresentReport 增加 real_gpu_texture / fallback_placeholder / gpu_unavailable 状态字段或摘要。
descriptor_only 不再是 219 成功态。
editor_core 仍只保存 plan/descriptor/status/report，不保存 wgpu handle。
Report Panel 能显示 219 GPU present evidence 摘要。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_gameview_play
cargo test -p editor_core report_panel
```

## 8. Gate F：editor_window_winit wiring

任务：

```text
editor_window_winit 在 real window renderer 可用时持有 shared GPU context + viewport texture registry。
Play Running 时按 GameView size allocate_or_resize shared texture target。
RedrawRequested 内同步执行 runtime texture write -> UI sample present。
resize 只在帧间处理。
Stop 时 unregister_session。
无 GPU / 无 registry / 缺 texture 时输出 fallback_placeholder 或 gpu_unavailable，不假装 presented。
```

测试：

```powershell
cd rust
cargo test -p editor_window_winit viewport_runtime
cargo test -p editor_window_winit gameview_gpu_texture_present_contract
```

## 9. Gate G：project_e2e_gate evidence

任务：

```text
新增 editor_gameview_gpu_texture_present gate。
打开 samples/complex_shooter_project。
验证 217 Preview RuntimePackage + 218 EditorRuntimePlayInstance 仍完整。
验证 219 evidence 区分 presented / fallback_placeholder / gpu_unavailable / failed。
默认环境允许 gpu_unavailable，但禁止 descriptor_only 冒充 presented。
写 complex-shooter-editor-gameview-gpu-texture-present-productization-report.json。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate editor_gameview_gpu_texture_present
```

## 10. Gate H：local-only real GPU smoke

任务：

```text
真实 OS window + real-wgpu feature 下验证 GameView texture present。
必须证明 texture 被 sample 到 GameView rect，并有非 placeholder 的 pixel/hash evidence。
如果本机 GPU/driver 不可用，最终报告写明未运行或 gpu_unavailable，不把它计为默认 CI 失败。
```

测试：

```powershell
cd rust
cargo test -p editor_window_winit gameview_real_gpu_texture_present --features real-wgpu-surface -- --ignored
```

## 11. 整体回归

全部 Gate 完成后运行：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_wgpu_renderer viewport_texture
cargo test -p engine_runtime wgpu_backend
cargo test -p editor_core editor_gameview_play
cargo test -p editor_core report_panel
cargo test -p editor_window_winit viewport_runtime
cargo test -p project_e2e_gate editor_gameview_gpu_texture_present
cargo test -p project_e2e_gate
```

如整体回归暴露与本施工无关的既有失败，必须在最终报告中说明；不得回退无关改动。

## 12. 施工文档自审

范围自审：

```text
本施工吸收 37 号审查建议，未推翻 219 方案。
本施工只做 A-min-gpu-gameview 的单 Editor window / 单 GameView / 单 RuntimeInstance / 单 texture。
本施工不做 input、Pause/Step、多窗口、跨进程 sharing。
本施工保留 RuntimePackage 真相和 218 EditorRuntimePlayInstance 边界。
本施工要求 default tests deterministic，real GPU smoke local-only。
本施工禁止 placeholder / descriptor_only 冒充 real_gpu_texture。
```

文档同步规则：

```text
施工完成后生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
把本施工文档从 当前/ 移到 已完成/。
如果施工中发现 219 方案错误，先回填正式方案再继续。
```
