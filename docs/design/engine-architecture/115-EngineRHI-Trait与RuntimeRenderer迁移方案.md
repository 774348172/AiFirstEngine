# 115-EngineRHI Trait 与 RuntimeRenderer 迁移方案

## 问题

114 已经把 Native Editor UI 收敛成：

```text
UiDrawList -> UiRenderGraph -> UiRhiCommandPlan -> WgpuBackend
```

Runtime 侧也必须走同一类长期路线，不能让 Sprite / Mesh 渲染长期停留在“RenderGraph 有 pass，但 backend 自己私下解析 plan”的状态。

本方案只做底层接口收敛：

```text
RenderSceneState
  -> RuntimeRenderer
  -> RenderGraph
  -> RhiCommandPlan
  -> EngineRhiBackend trait
  -> Backend
```

## 其它引擎对比

### UE

UE 的长期路线是：

```text
Renderer / RDG
  -> RHICommandList
  -> DynamicRHI
  -> D3D12 / Vulkan / Metal
```

可借鉴：

```text
Renderer 生成图和命令。
Backend 实现设备、命令、提交和 present。
Game / ECS 不知道具体图形 API。
```

### Unity

Unity 的路线接近：

```text
RenderPipeline
  -> ScriptableRenderContext / CommandBuffer
  -> native graphics backend
```

可借鉴：

```text
上层提交抽象命令，底层 backend 执行。
项目逻辑不直接接触图形 API。
```

### Bevy

Bevy 直接以 wgpu 为渲染后端，但仍有 Extract / RenderGraph / Pipeline 等分层。  
可借鉴：

```text
Rust 生态下先用 headless / wgpu backend 验证结构可行。
```

不照搬：

```text
本项目长期不把 wgpu 当唯一 RHI 抽象。
```

## 最终规则

```text
1. EngineRhiBackend 是 Runtime Renderer 后端接口。
2. RuntimeRenderer 不直接执行具体 GPU API。
3. RuntimeRenderer 必须先生成 RenderGraph。
4. RenderGraph 必须编译成 RhiCommandPlan。
5. Backend 必须通过 EngineRhiBackend 的 begin_frame / clear / draw / submit / present 执行。
6. execute_plan 只是 EngineRhiBackend 的默认编排方法，不是 backend 私有解析入口。
7. SpriteBasic / MeshBasic 必须编译成 RhiDrawKind 并通过 EngineRhiBackend::draw 执行。
8. HeadlessRhiBackend 是自动化测试后端。
9. WgpuBackend 当前可以是 unavailable skeleton，但必须实现同一 EngineRhiBackend trait。
10. 后续真实 WGPU / D3D12 / Vulkan / Metal backend 都实现同一 trait。
```

## 当前 v1-min 边界

已实现：

```text
EngineRhiBackend 方法集。
HeadlessRhiBackend 按方法集记录 clear / draw / submit / present。
RuntimeRenderer::render_with_rhi_backend。
Sprite / Mesh 到 RhiDrawKind 的测试。
runtime_cli fixture 与 RuntimeAssetIndex 规则对齐。
```

未实现：

```text
真实 WGPU Runtime backend。
GPU buffer / texture / pipeline 资源对象。
Shader / Material 真实绑定。
Surface swapchain present。
D3D12 / Vulkan / Metal backend。
```

## 为什么适合我们

AI 友好：

```text
AI 可以读 RenderGraph / RhiCommandPlan / RhiBackendReport，不需要理解具体 GPU API。
```

复杂项目可维护：

```text
Mesh、Sprite、AUI、后续粒子、材质、阴影都可以统一走 RHI command，不新增一堆 Bridge。
```

简单：

```text
第一版只做 begin_frame / clear / draw / submit / present 五类基础动作。
```

效率：

```text
接口层先稳定，真实 GPU backend 后续再优化资源复用、batch、pipeline cache。
```
