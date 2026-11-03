# 当前可自动化施工文档：Runtime Viewport Rendering System D-min

> 状态：已确认可施工。
> 长期规则见 `../../71-Runtime-Viewport-Rendering-System方案.md`、`../../59-真实WgpuBackend-RDG-RHI最小门禁方案.md`、`../../55-RenderSceneState-RenderProxy-v1方案.md`、`../../37-Editor-Core与可迁移UI路线.md`。

## 目标

把已有 RuntimeRenderer 输出接入编辑器 viewport texture 路径，形成 headless 可测试的最小闭环：

```text
RenderSceneState
  -> RuntimeRenderer
  -> RuntimeRenderFrameOutput
  -> ViewportTextureDescriptor
  -> ViewportHost
  -> EditorUiRenderer DrawCommand::ViewportTextureSlot
  -> headless report / unit test
```

## 施工边界

本轮必须做：

```text
1. RuntimeRenderer 支持 ViewportTexture target 的结构化输出 descriptor。
2. RuntimeRenderFrameOutput 或现有 output 中增加 target summary / texture descriptor。
3. EditorUiRenderer 的 ViewportTextureSlot 支持 texture_id / target_id / frame 这类可追踪字段。
4. ViewportHost 能保存最近一次 runtime viewport frame summary。
5. Headless 单元测试覆盖 RuntimeRenderer -> ViewportHost -> EditorUiRenderer slot。
```

本轮不做：

```text
真实 GPU texture lifetime。
真实 wgpu surface present。
完整 Render Thread。
完整 swapchain 多帧同步。
真实 D3D12 / Vulkan / Metal backend。
项目逻辑直接控制 RenderGraph / RHI。
```

## 参考引擎对照

```text
UE：
FSceneViewport 管 viewport target，UGameViewportClient 构造 view family，RendererModule 渲染世界。

Unity：
GameView / SceneView 渲染到 RenderTexture，再由编辑器 UI 把 texture 画进面板。

Bevy：
Camera RenderTarget 支持 Window / Image / TextureView / None，target 抽象统一。
```

本轮施工采用：

```text
Unity-like texture embedding
  + Bevy-like target abstraction
  + UE-like Renderer / UI 边界
```

## 文件范围

优先修改：

```text
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_host/src/main.rs
```

必要时补充：

```text
rust/crates/engine_runtime/src/render_state.rs
rust/crates/engine_runtime/src/render_graph.rs
```

文档完成后更新：

```text
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
18-长期主义实现路径.md
阶段完成记录/README.md
施工文档/README.md
```

## 任务拆分

### A1：Runtime viewport texture descriptor

新增或补齐结构：

```text
ViewportTextureDescriptor
  texture_id
  target_id
  width
  height
  format
  color_space
  frame_index
  producer
```

规则：

```text
HeadlessTexture / ViewportTexture 必须能产生 descriptor。
Surface 不强制产生 descriptor。
```

最小测试：

```text
RuntimeRenderer 使用 RuntimeRenderTargetKind::ViewportTexture 时，输出 descriptor。
descriptor.target_id == render_target.target_id。
descriptor.frame_index == input.frame_index。
```

### A2：RuntimeRenderFrameOutput summary

补齐输出 summary：

```text
RuntimeRenderFrameOutput
  frame_index
  target_id
  target_kind
  texture_descriptor?
  render_frame_report
  render_graph_report
  rhi_backend_report?
```

如果当前已有 `RuntimeRendererOutput`，优先扩展现有结构，不新建第二套输出模型。

最小测试：

```text
build(viewport texture input) 后，output 能同时拿到 render_graph / rhi_command_plan / texture_descriptor。
```

### A3：EditorUiRenderer ViewportTextureSlot

扩展：

```text
DrawCommand::ViewportTextureSlot
  rect
  scene_id
  frame
  texture_id?
  target_id?
```

规则：

```text
UI 只声明显示 slot。
UI 不生成 runtime texture。
UI 不读取 RenderGraph / RHI command。
```

最小测试：

```text
SelfUiRenderer::build_draw_list 能生成带 scene_id / frame / texture target 信息的 ViewportTextureSlot。
```

### A4：ViewportHost runtime frame summary

ViewportHost 保存最近一次 runtime viewport 输出：

```text
RuntimeViewportFrameSummary
  viewport_id
  target_id
  texture_id
  frame_index
  width
  height
```

规则：

```text
ViewportHost 负责绑定 editor viewport 与 runtime output。
ViewportHost 不执行渲染。
```

最小测试：

```text
ViewportHost 注册 Scene View 后，接收 RuntimeRenderer output summary。
后续 UI model / draw list 能读取对应 frame 信息。
```

### A5：Headless end-to-end gate

新增一个最小测试：

```text
构造 RenderSceneState + RenderViewState(SceneView)
  -> RuntimeRenderer build ViewportTexture target
  -> ViewportHost ingest output summary
  -> EditorUiRenderer build draw list
  -> 断言存在 ViewportTextureSlot
  -> 断言 slot.frame / texture_id / target_id 可追踪
```

## 验证命令

默认测试：

```powershell
cargo test -p engine_runtime
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit
cargo test -p editor_host
```

必要时全量：

```powershell
cargo test
```

## 完成标准

```text
1. 所有新增结构能 serde 序列化。
2. headless 单元测试通过。
3. RuntimeRenderer 不绕过 RenderGraph / RHI。
4. EditorUiRenderer 不直接渲染游戏世界。
5. ViewportHost 不执行渲染，只保存绑定关系和 frame summary。
6. 文档入口更新完成。
```

## 完成后阶段记录

施工完成后新增：

```text
阶段完成记录/2026-xx-xx-Runtime-Viewport-Rendering-System-D-min/00-总览.md
```

并移动本文档到：

```text
施工文档/已完成/71-当前可自动化施工文档-Runtime-Viewport-Rendering-System-D-min.md
```

