# 286-R1 GameView Contain / Camera2D / Texture Last-Good Minimal Repair v1

> 状态：已完成，待归档；用户于 2026-08-14 明确授权直接引擎修复。
> 上游：273 GameView Presentation、265 Stable GameView Publication、275 AUI Image、281 Sprite2D Texture。

## 目标

只修复真实 Tower Editor Play 已确认的三个缺陷：

1. GameView texture slot 的实际绘制 content rect 与输入 `Contain` presentation 使用同一结果；
2. Sprite2D 使用场景 `project.camera2d.orthographicSize`，不再固定使用 `7.5`；
3. GPU texture handle 未驻留时禁止白色 solid fallback，失败 publication 保留 last-good surface 并报告缺失 handle。

## 必要性审计

```text
confirmed failure:
- 720x1280 texture 被完整 slot 拉伸，但 input 只接受 Contain content rect；
- scene camera orthographicSize=9.6，Sprite2D renderer 固定 7.5；
- missing resident texture 继续走 solid pipeline，AUI Image 默认白色顶点形成白块。

minimum causal fix:
- draw list 与 ViewportHost 同时消费一个 resolved display content rect；
- 从现有 hydrated scene camera dynamic component 提取正交半高并写入现有 RenderViewState；
- missing texture 使本次 GPU write 失败，publication module 不推进并复用 last-good。

files that must change:
- editor_ui_renderer / editor_window_winit 的 GameView texture rect owner 与测试；
- engine_runtime 的 camera projection / Sprite2D vertices owner 与测试；
- engine_runtime WGPU backend、editor_window_winit publication consumer 与测试。

tests that can fail before and pass after:
- 720x1280 Contain draw rect 与 display->target round-trip；
- orthographicSize=9.6 的 Sprite 像素/NDC 中心合同；
- missing UI texture 不提交白色 fallback，last-good receipt 不推进。

explicitly deferred nearby enhancements:
- 不新增 Camera2D authoring schema/migration；
- 不实现异步 texture streaming/device-loss 总重构；
- 不修改 Player/MCP/production Editor/安装态二进制；
- 不运行 Local CI 或完整视觉矩阵。
```

## Gate A：Contain Display/Input 单一 Presentation

- 为 GameView draw model 携带 resolved display content rect；
- texture quad、hit region、ViewportHost 注册使用同一 rect；
- owner test 覆盖 landscape slot + portrait target + Contain，左右 gutter 不绘制也不路由。

## Gate B：Scene Camera2D Projection

- 读取 active camera entity 的现有 `project.camera2d.orthographicSize`；
- 生成并复用 `RenderViewState.projection_matrix`；
- Sprite2D vertices 消费 view projection，不再读取固定 `7.5`；
- 缺失/非法 camera 时保留兼容默认值并给确定行为。

## Gate C：Missing Texture Last-Good

- WGPU missing texture handle 为 render failure，不绘制 untextured image quad；
- Editor publication 仅在 backend 无 error 时 mark/publish；
- 失败时保留 last-good surface/receipt，并回传包含 handle 的结构化 diagnostic。

## 验证

开发期只运行直接 owner 与受影响 consumer：

```powershell
cd G:\gameEngin\rust
cargo fmt --check
cargo test -p engine_runtime <targeted filters>
cargo test -p editor_ui_renderer <targeted filters>
cargo test -p editor_window_winit <targeted filters>
```

不运行 Local CI，不替换 production/安装态二进制，不修改真实配置，不重建 Tower Preview cache。

## 完成条件

- 三个 red-capable owner test 通过；
- 受影响 crate 测试无新增失败；
- 记录实际运行与省略项后归档本文档并同步入口。

## 完成结果

- Gate A：GameView texture slot 与 `HitTarget::Viewport` 共同消费
  `GameViewPresentationModule::resolve` 的 `display_content_rect`；`720x1280 Contain` 与
  `Stretch` owner test 通过。
- Gate B：EngineHost 从已水合的启用场景 camera 读取 `project.camera2d.orthographicSize` 和
  Transform，写入现有 Game `RenderViewState`；Sprite2D 顶点消费 view/projection matrix，合法场景相机
  不再使用固定 `7.5`。
- Gate C：RealWgpuBackend 在 encoder、clear、submit、present 之前拒绝未驻留的必需 texture handle，
  输出 `wgpu.texture_binding_missing` Error；Editor 在稳定纹理 allocate/resize 前执行同一预检，失败
  publication 不推进 last-good receipt/index。

验证通过：`editor_ui_renderer` 77/77，`engine_runtime` 559/559，`editor_window_winit`
194 passed / 7 ignored，真实 WGPU 缺失纹理 owner test 1/1，`real-wgpu-surface` check 与 fmt/diff check。
未运行 Local CI，未替换 production/安装态二进制，未修改真实配置，未重建 Tower Preview cache，未运行
真实 Editor smoke 或视觉矩阵。
