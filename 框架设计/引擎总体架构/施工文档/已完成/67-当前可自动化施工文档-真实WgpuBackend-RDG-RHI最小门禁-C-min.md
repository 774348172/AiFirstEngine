# 当前可自动化施工文档：真实 WgpuBackend / RDG / RHI 最小门禁 C-min

> 状态：已确认可施工。本文档只定义本轮自动化施工范围，不继续扩展完整 Renderer / RDG / RHI 长期架构规则。长期规则以 `59-真实WgpuBackend-RDG-RHI最小门禁方案.md`、`50-RenderCommand-RenderSceneState方案.md`、`55-RenderSceneState-RenderProxy-v1方案.md`、`17-Runtime-FrameLoop.md` 和 `37-Editor-Core与可迁移UI路线.md` 为准。

## 1. 施工目标

本轮目标是把当前 CPU-side / headless 渲染闭环推进到“真实 GPU 输出的最小门禁”，但只做 C-min：

```text
RenderSceneState
  -> RuntimeRenderer
  -> RDG-min
  -> RHI-min
  -> WgpuBackend-min
  -> Headless texture target / optional real surface smoke
  -> RenderFrameReport / RenderGraphReport / RhiBackendReport
```

核心结果：

```text
RenderCommand 仍只负责 Game 到 RenderSceneState 的同步。
RDG-min 负责一帧 pass / resource / target / dependency / diagnostics。
RHI-min 负责 device / queue / surface / texture / buffer / command encoder / render pass / present 的抽象。
WgpuBackend-min 是 EngineRHI 的第一个后端，不绕过 RHI。
RuntimeRenderer 从 RenderSceneState 生成 RenderGraph，不直接读取 ECS。
Headless 自动化必须可测。
真实 wgpu surface 只作为 feature-gated smoke gate，不能成为唯一验证方式。
```

本轮不是实现完整 Render Thread，也不是实现完整 UE 级 RDG / RHI / GPU Resource Pool。

## 2. 参考引擎对比

| 引擎 | 对应机制 | 可借鉴点 | 本轮取舍 |
|---|---|---|---|
| Unreal | `FScene` -> Renderer -> `FRDGBuilder` -> `FRHICommandList` -> `FDynamicRHI` -> D3D12/Vulkan/Metal | RenderCommand / RDG / RHI / Backend 分层清楚；RDG 管 pass/resource/lifetime/validation | 学分层和门禁，不复制完整 RDG、RHI、async compute、多 GPU |
| Unity | `RenderPipeline` -> `ScriptableRenderContext` -> `CommandBuffer` -> `Submit` -> native renderer | 上层入口简单；context/camera/command buffer/submit 心智清楚 | 学稳定渲染入口，不采用 native 黑箱和用户直接写 CommandBuffer |
| Bevy | Main World -> Extract -> Render World -> RenderGraph / RenderPhase -> wgpu | Rust/ECS/Extract/wgpu 路线可行，headless 思路清晰 | 学底层结构，不把 RenderWorld/Schedule 复杂度暴露给 AI 默认层 |
| Godot | Scene/Node -> RenderingServer -> RenderingDevice/backend | 渲染服务层隔离项目对象与 GPU | 学服务边界，不采用 Node 体系 |

本项目路线：

```text
UE-like RDG / RHI 分层
+ Bevy-like Rust / wgpu 第一后端
+ Unity-like 简单渲染入口
+ AI-first report / diagnostics / headless gate
```

## 3. 硬规则

### 3.1 RenderCommand 和 RHICommand 不能混淆

```text
RenderCommand：
  Game / ECS / RenderDirtyTracker 到 RenderSceneState 的增量状态同步。

RHICommand：
  RDG 编译后给 EngineRHI / Backend 执行的 GPU 命令。
```

项目逻辑和 AI 都不能直接生成 RHICommand / Wgpu command。

### 3.2 WgpuBackend 必须在 RHI 后面

```text
WgpuBackend 是 EngineRHI 的第一个后端。
禁止 RenderSceneState 直接调用 wgpu。
禁止 RuntimeRenderer 跳过 RDG / RHI 直接 present。
```

### 3.3 所有默认自动化必须 headless 可测

```text
Headless texture target 是默认测试路径。
真实 window / surface / GPU 是显式 feature smoke gate。
真实 GPU 失败不能阻塞 headless C-min 测试。
```

### 3.4 Editor UI 和 Runtime Renderer 分离

```text
RuntimeRenderer 负责游戏世界内容。
EditorUiRenderer 负责编辑器外壳、面板、overlay。
Scene Viewport 是 RuntimeRenderer 输出 texture 与 Editor UI 的组合点。
```

本轮可产出 viewport texture descriptor / handle 占位，但不要求完整嵌入真实编辑器 UI。

## 4. 本轮边界

### 本轮做

```text
新增 RDG-min 纯数据结构：RenderGraph / RenderPass / RenderResource / RenderGraphDiagnostic。
新增 RHI-min 抽象：EngineRhiDevice / Queue / Texture / Buffer / Surface / CommandEncoder / RenderPass / Frame。
新增 WgpuBackend-min 边界：headless backend 必须实现，真实 wgpu backend 可以 feature-gated skeleton。
新增 RuntimeRenderer v1：从 RenderSceneState / RenderViewState / QualityProfile / RenderTarget 生成 RenderGraph。
新增 RDG -> RHI command plan 编译层。
新增 HeadlessRhiBackend 或 HeadlessWgpuBackend，执行 clear + test geometry 的可验证 frame。
新增 RenderGraphReport / RhiBackendReport。
新增端到端 headless gate：RenderSceneState -> RuntimeRenderer -> RDG -> RHI -> Backend -> report。
新增最小测试命令。
更新 49 / 54 / 18 / README / 00 / 施工文档 README 当前入口。
```

### 本轮不做

```text
完整 Render Thread。
真实 D3D12 / Vulkan / Metal backend。
完整 wgpu shader pipeline。
完整 GPU resource pool。
复杂 barrier 推导。
async compute。
multi GPU。
完整 shader compiler。
PBR。
shadow map。
post process。
复杂 material graph codegen。
复杂 mesh batching / instancing。
真实 Asset runtime loading。
Scene / Prefab 从 Runtime Package 实例化到 ECS。
Project Logic IR Interpreter / Rust AOT 接入 Rust ECS。
Build / Run 最小游戏包。
```

## 5. 建议代码落点

优先在 Rust runtime / renderer 方向落地。实际落点以当前代码结构为准。

允许新增 Rust crate：

```text
rust/crates/engine_renderer
```

或如果为了保持最小施工，可以先落在：

```text
rust/crates/engine_runtime/src/render_graph.rs
rust/crates/engine_runtime/src/engine_rhi.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/headless_rhi_backend.rs
```

建议文件：

```text
rust/crates/engine_runtime/src/render_graph.rs
rust/crates/engine_runtime/src/render_graph_report.rs
rust/crates/engine_runtime/src/engine_rhi.rs
rust/crates/engine_runtime/src/rhi_command_plan.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/headless_rhi_backend.rs
rust/crates/engine_runtime/src/wgpu_backend.rs，feature-gated skeleton 可选
```

脚本：

```text
scripts/test-rdg-rhi-wgpu-cmin.cjs
```

package script：

```text
test:rdg-rhi-wgpu-cmin
```

## 6. 数据结构要求

### 6.1 RenderGraph v1

```text
RenderGraph:
  graph_id
  frame_index
  views
  resources
  passes
  output_target
  diagnostics
```

规则：

```text
RenderGraph 是一帧图，不是长期状态。
RenderGraph 不直接读取 ECS。
RenderGraph 不保存 Project Logic。
```

### 6.2 RenderPass v1

```text
RenderPass:
  pass_id
  pass_name
  pass_kind
  view_id
  reads
  writes
  color_targets
  depth_target optional
  commands
  debug_source optional
```

第一版 pass_kind：

```text
Clear
DrawTestGeometry
DrawMeshBasic
Present
```

### 6.3 RenderResource v1

```text
RenderResource:
  resource_id
  resource_name
  resource_kind
  format
  size
  usage
  lifetime
```

resource_kind：

```text
Texture
Buffer
SurfaceBackbuffer
ExternalTexture
```

lifetime：

```text
FrameLocal
External
Persistent
```

### 6.4 EngineRHI v1

```text
EngineRhiDevice
EngineRhiQueue
EngineRhiSurface
EngineRhiTexture
EngineRhiBuffer
EngineRhiCommandEncoder
EngineRhiRenderPass
EngineRhiFrame
```

最小接口：

```text
create_device
create_surface
resize_surface
begin_frame
acquire_surface_texture
create_texture
create_buffer
begin_command_encoder
begin_render_pass
set_pipeline
set_viewport
set_vertex_buffer
draw
end_render_pass
submit
present
```

### 6.5 RuntimeRenderer v1

```text
RuntimeRendererInput:
  render_scene_state
  render_view_state
  quality_profile
  render_target

RuntimeRendererOutput:
  render_graph
  rhi_command_plan
  render_frame_report
  render_graph_report
  rhi_backend_report optional
```

### 6.6 Reports

```text
RenderGraphReport:
  frame_index
  graph_id
  pass_count
  resource_count
  output_target
  diagnostics

RhiBackendReport:
  backend_kind
  frame_index
  target_kind
  clear_count
  draw_count
  submit_count
  present_count
  diagnostics
```

Report 必须可 JSON 序列化，字段稳定，供 AI 和人类查错。

## 7. 施工步骤

### A0：基线验证

目标：

```text
确认当前 Rust runtime / native editor 相关测试仍可运行。
```

执行：

```powershell
npm.cmd run test:rustruntime
npm.cmd run test:enginehostloop
npm.cmd run test:native-editor-cmin
```

如果 Rust 工具链或 GUI 工具链失败，记录原因。headless RDG/RHI 施工不应依赖真实 GUI。

### A1：新增 RDG-min 类型层

任务：

```text
新增 RenderGraph / RenderPass / RenderResource / RenderGraphDiagnostic。
实现基础构造和 validation。
```

测试：

```text
empty graph validation reports missing output target。
clear graph validation passes。
pass references missing resource -> diagnostic。
resource lifetime / usage 字段可序列化。
```

### A2：新增 EngineRHI-min trait / interface

任务：

```text
定义 EngineRHI 最小接口。
定义 RhiCommandPlan。
定义 RhiBackendReport。
```

测试：

```text
RhiCommandPlan 可表达 clear / draw / present。
RhiBackendReport 可 JSON 序列化。
EngineRHI 不暴露 Entity / Component / RenderCommand 类型。
```

### A3：RDG -> RHI Command Plan 编译层

任务：

```text
实现 compile_render_graph_to_rhi_plan。
支持 Clear / DrawTestGeometry / Present。
检查 pass read/write/resource/target。
输出 diagnostics。
```

测试：

```text
clear graph -> one clear command + present。
test geometry graph -> clear + draw + present。
missing target -> diagnostic error。
unused resource -> warning。
```

### A4：HeadlessRhiBackend / HeadlessWgpuBackend

任务：

```text
实现 headless backend。
模拟 texture target / surface target。
执行 RhiCommandPlan。
记录 clear_count / draw_count / submit_count / present_count。
输出稳定 frame_hash 或 target_hash。
```

测试：

```text
clear command 改变 target_hash。
draw test geometry 增加 draw_count。
present 增加 present_count。
同样输入得到同样 hash。
backend 不支持某 feature 时输出 fallback / diagnostic。
```

### A5：RuntimeRenderer v1 接入 RenderSceneState

任务：

```text
实现 RuntimeRenderer。
读取 RenderSceneState / RenderViewState。
生成 single-view RenderGraph。
第一版至少支持 clear + test geometry。
如已有 basic mesh RenderProxy，可生成 DrawMeshBasic pass 或降级为 DrawTestGeometry。
```

测试：

```text
空 scene 生成 clear graph。
有 render proxy 生成 draw pass 或明确 fallback。
RuntimeRenderer 不读取 ECS World。
RuntimeRenderer 不修改 RenderSceneState。
```

### A6：端到端 headless C-min gate

任务：

```text
构造最小 RenderSceneState。
RuntimeRenderer 生成 RenderGraph。
RDG 编译 RhiCommandPlan。
Headless backend 执行。
输出 RenderFrameReport / RenderGraphReport / RhiBackendReport。
```

测试：

```text
end-to-end gate passes。
report.pass_count > 0。
report.draw_count >= 1 或 fallback 明确。
diagnostics 无 error。
target_hash 稳定。
```

### A7：WgpuBackend-min feature skeleton

任务：

```text
新增 WgpuBackend-min 结构壳。
真实 wgpu / surface 代码放在显式 feature 后。
如果当前依赖未准备好，可以只做 trait-compatible skeleton + disabled report。
```

测试：

```text
默认构建不依赖真实 GPU。
feature 未启用时输出 backend_unavailable diagnostic。
headless 测试不受影响。
```

### A8：脚本和 package 入口

任务：

```text
新增 scripts/test-rdg-rhi-wgpu-cmin.cjs。
新增 package.json script：test:rdg-rhi-wgpu-cmin。
```

最低测试：

```powershell
npm.cmd run test:rdg-rhi-wgpu-cmin
npm.cmd run test:enginehostloop
npm.cmd run test:native-editor-cmin
npm.cmd run build
```

## 8. 验收标准

必须满足：

```text
RDG-min / RHI-min / Headless backend 都有最小测试。
默认自动化不依赖真实 GPU / window。
WgpuBackend 不绕过 EngineRHI。
RuntimeRenderer 不直接读取 ECS。
RenderCommand 和 RHICommand 类型边界清楚。
RenderGraphReport / RhiBackendReport 可 JSON 序列化。
新增 package script 可运行。
README / 49 / 54 / 18 / 00 / 施工文档 README 已同步当前施工入口。
```

## 9. 风险和暂停条件

遇到以下情况必须暂停，不要硬做：

```text
必须引入完整 wgpu surface 才能继续。
必须实现真实 Render Thread 才能继续。
必须做完整 GPU resource pool / barrier 系统。
必须做真实 shader compiler。
必须接 Asset runtime loading 才能继续。
必须让 RuntimeRenderer 直接读 ECS World。
必须让项目逻辑或 AI 直接生成 RDG / RHI / Wgpu command。
```

允许降级：

```text
真实 WgpuBackend 暂时只做 feature skeleton。
Basic mesh 暂时降级为 test geometry。
Editor viewport texture 嵌入暂时只输出 descriptor / report。
```

## 10. 完成后文档同步

施工完成后必须：

```text
把本文档移动到 施工文档/已完成。
新增 阶段完成记录/2026-xx-xx-RDG-RHI-WgpuBackend-C-min/00-总览.md。
更新 阶段完成记录/README.md。
更新 49-当前状态与下一步入口.md。
更新 54-当前可自动化施工总入口.md。
更新 18-长期主义实现路径.md。
更新 README.md。
更新 00-阅读顺序.md。
更新 施工文档/README.md。
如果施工中产生长期规则，回填 59 / 50 / 55 / 17 / 37 / 47 等对应框架文档。
```

## 11. 本轮之后的下一步

本轮完成后再判断是否进入：

```text
Runtime Package / AssetRegistry / cooked asset 接入 Rust Runtime 真实资源加载。
Scene / Prefab / Entity 从 Runtime Package 实例化到 Rust ECS。
Native Editor Viewport 输入进入 Runtime Action / Game Frame。
真实 OS file watcher / Import Worker。
真实 real-window smoke gate 升级。
```

不要在本轮直接跳到这些方向。
