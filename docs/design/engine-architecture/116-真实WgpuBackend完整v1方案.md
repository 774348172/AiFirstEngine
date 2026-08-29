# 116-真实 WgpuBackend 完整 v1 方案

## 问题

Runtime Renderer 已经收敛到：

```text
RenderSceneState
  -> RuntimeRenderer
  -> RenderGraph
  -> RhiCommandPlan
  -> EngineRhiBackend
```

但当前 `WgpuBackend` 仍然只是 unavailable skeleton。下一步不能再做只会 clear / test triangle 的临时版本，而要把 WGPU 做成完整的第一个真实 `EngineRhiBackend`。

这里的长期规则是：WGPU 是第一个真实 backend，不是引擎的最终抽象边界。

## 其它引擎对比

| 引擎 | 路线 | 可借鉴点 | 不照搬点 |
|---|---|---|---|
| Unreal Engine | Renderer / RDG -> RHICommandList -> DynamicRHI -> D3D12/Vulkan/Metal | Renderer、RDG、RHI、Backend 分层；RHI command 是可执行命令；backend 负责资源和提交 | 不第一版实现完整 UE 级 RDG、barrier、transient resource、多 GPU |
| Unity | RenderPipeline -> ScriptableRenderContext / CommandBuffer -> Native Graphics Backend | 上层生成 command buffer，底层 native backend 执行；项目层不碰图形 API | 不做 Unity 多套管线复杂度 |
| Bevy | Extract/Prepare/Queue/Render -> RenderGraph -> wgpu RenderDevice/RenderQueue/PipelineCache | Rust + wgpu 工程实践、PipelineCache、RenderDevice/Queue 分离 | 不把 wgpu 当长期唯一抽象 |
| Godot | RenderingServer / RenderingDevice / backend | 渲染服务和设备后端分层，项目逻辑不直接操作 GPU | 不第一版做完整 RenderingServer 规模 |

## 最终规则

```text
1. WGPU 类型只能出现在 WgpuBackend feature/module 内。
2. RuntimeRenderer 不允许直接调用 wgpu。
3. RenderGraph 不允许保存 wgpu 类型。
4. RhiCommandPlan 必须保存真实绘制所需的 backend-neutral payload。
5. Sprite / Mesh / UI 不能在 RHI 编译阶段丢失资源引用。
6. WgpuBackend 只消费 RhiCommandPlan，不读取 ECS World。
7. GPU resource 生命周期归 WgpuBackend / RenderResourceRegistry 管。
8. 项目逻辑不能直接创建 GPU buffer / texture / pipeline。
9. 所有失败进入 RhiBackendReport，不能只 panic。
10. WgpuBackend 完整 v1 做到真实 Sprite / Mesh / UI 基础 present 能力，不做高级渲染特性。
```

## 完整 v1 边界

必须做：

```text
Engine RHI command schema v2
  外层固定 command schema
  内层按 draw_kind 携带 typed payload

Wgpu backend structure
  device context
  target context
  resource registry
  upload counters
  pipeline cache counters
  backend report

真实 WGPU feature path
  feature = real-wgpu
  offscreen texture target smoke
  后续窗口 surface 由 window host 注入或封装，不让 RuntimeRenderer 依赖窗口库

Headless path
  默认编译仍可测试完整 payload
  HeadlessRhiBackend 按同一 EngineRhiBackend trait 执行
```

暂不做：

```text
Nanite / Lumen / HDRP 级高级渲染
完整 PBR 材质系统
完整 Shader Graph
多 GPU
复杂 resource aliasing / barrier optimizer
D3D12 / Vulkan / Metal backend
项目专属 gameplay 渲染规则
```

## 推荐结构

```text
RuntimeRenderer
  -> RenderGraph
  -> RhiCommandPlan v2
      BeginFrame
      Clear
      Draw(payload: TestGeometry | MeshBasic | SpriteBasic | UiOverlay)
      Submit
      Present
  -> EngineRhiBackend
      HeadlessRhiBackend
      WgpuBackend
```

## 为什么适合我们

AI 友好：

```text
AI 读 RenderGraph / RhiCommandPlan / RhiBackendReport，不需要理解 wgpu API。
```

复杂项目可维护：

```text
Sprite、Mesh、UI、后续 Particle / SkinnedMesh 都走同一 RHI 命令结构，不继续长出无数 bridge。
```

长期可替换：

```text
WGPU 是第一个 backend。后续 D3D12 / Vulkan / Metal 是实现同一个 EngineRhiBackend，而不是推翻 RuntimeRenderer。
```

效率：

```text
pipeline cache、resource registry、upload、surface recover 都留在 backend 内部，后续可以优化 batch / instancing / pipeline specialization。
```
