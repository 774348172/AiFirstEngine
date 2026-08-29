# Renderer MVP

## 当前归属说明：Projection 术语

本文中如果出现以下历史名称：

```text
RenderExtract
RenderAssetBridge / Render Asset Bridge
Physics2DBridge
RuntimeScene Hydration
AuiRenderExtract / AuiRendererBridge
SpriteRenderer2D ECS-to-RenderProxy Bridge
```

统一按 `110-World-Projection-Adapter统一跨域同步规则.md` 理解为：

```text
RenderProjection
AssetProjection
Physics2DProjection
HydrationProjection
UiProjection
RenderProjectionAdapter<SpriteRenderer2D>
```

这些名称可以作为历史实现名保留，但不再作为新增架构概念扩展。后续新增类型只新增对应 `ProjectionAdapter`，不新增独立 Bridge。

> 当前状态：本文档前半部分记录 Phase 13 TypeScript / Mock Renderer 的过渡 MVP。正式长期渲染同步路线以 `17-Runtime-FrameLoop.md`、`50-RenderCommand-RenderSceneState方案.md`、`51-RenderDirtyTracker-RenderExtract-RenderCommand闭环方案.md` 为准。新功能不得继续把完整 RenderSnapshot 当作 Game -> Render 主链路。

本文记录 Phase 13 Renderer MVP 的第一版实现。

## 定位

Renderer MVP 不是高级渲染效果，也不是最终 Native RHI。

当前目标是先建立正式渲染边界：

```text
RenderSnapshot
  -> RenderGraph
  -> RendererBackend
  -> RenderReport
```

注意：这是 Phase 13 TypeScript / Mock Renderer 的第一版过渡闭环，不是长期渲染线程同步战略。

长期正式路线已经调整为：

```text
Game / ECS / Scene
  -> RenderExtract
  -> RenderCommand Queue
  -> Render Thread
  -> RenderSceneState / RenderProxy
  -> Renderer Feature Builder
  -> RDG / RenderGraph
  -> RHI
  -> Backend
```

因此本文中的 RenderSnapshot 只能理解为：

```text
第一版 MVP 的最小输入结构
AI / Debug 可读的 RenderFrameReport / SnapshotView 雏形
从旧 TypeScript Runtime 迁移到 Rust Native Runtime 前的过渡适配层
```

不能理解为：

```text
完整场景级双缓冲
长期 Render Thread 同步模型
大型场景每帧复制的渲染世界
```

这条链路必须满足：

```text
AI 可审查
依赖可验证
降级可解释
后端可替换
不依赖编辑器 UI
不绑定 Three.js
```

Renderer 与编辑器 UI 的边界：

```text
Runtime Renderer 负责渲染游戏世界内容。
Editor UI Renderer 负责渲染编辑器面板和控件。
Scene Viewport 通过 render target / texture 把 Runtime Renderer 输出嵌入 Editor UI。
```

Scene View / Game View 的区别：

```text
Game View:
  RenderSceneState + GameCamera
    -> Renderer Feature Layer
    -> RDG / RenderGraph
    -> RHI
    -> game surface / present

Scene View:
  RenderSceneState + EditorCamera + EditorViewport flags
    -> Renderer Feature Layer
    -> RDG / RenderGraph
    -> RHI
    -> viewport render target / texture
    -> Editor UI Renderer composite
```

禁止：

```text
Editor UI Renderer 重新实现 3D world renderer。
Runtime Renderer 绘制编辑器 Hierarchy / Inspector / Console / Toolbar。
Scene View 使用独立于 Runtime Renderer 的第二套世界渲染管线。
```

## 当前实现

新增代码：

```text
src/renderer/renderGraph.ts
src/renderer/RendererBackend.ts
src/renderer/MockRendererBackend.ts
src/renderer/rendererPipeline.ts
scripts/test-renderer-mvp.cjs
scripts/test-runtime-bundle-renderer.cjs
```

新增命令：

```powershell
npm.cmd run test:renderer
npm.cmd run test:runtimebundlerenderer
```

## RenderGraph v1

当前 RenderGraph 是纯数据结构：

```text
schemaVersion: render-graph.v1
quality
resources[]
passes[]
source.sceneId
source.renderableCount
source.lightCount
source.hasCamera
```

资源类型：

```text
color-target
depth-target
shadow-map
scene-buffer
material-buffer
present-target
```

Pass 类型：

```text
clear
shadow
opaque
lighting
present
```

当前最小图：

```text
clear-main
shadow-main       // medium/high 且有 directional light 时生成
draw-opaque
present-main
```

## 验证规则

RenderGraph validation 当前检查：

```text
空 pass 列表
重复 resource id
重复 pass id
pass 读不存在的 resource
pass 写不存在的 resource
pass 在 resource 写入前读取 transient/internal resource
```

边界规则：

```text
scene-buffer / material-buffer 是外部输入资源，可以直接读取。
color/depth/shadow/present 等内部资源必须先由前序 pass 写入。
```

这不是玩法规则验证，也不是材质系统验证。
它只负责 RDG/RHI 前的基础依赖正确性。

## RendererBackend v1

当前接口：

```text
RendererBackend
  name
  capabilities
  execute(RenderGraph) -> RenderReport
```

当前实现：

```text
MockRendererBackend
```

Mock backend 不画图，只执行结构化验证和 pass 列表报告。
它的存在是为了让 AI、测试、Build Pipeline 可以先依赖稳定报告，而不是依赖具体图形 API。

## RenderReport v1

RenderReport 结构：

```text
schemaVersion: render-report.v1
ok
backend
requestedQuality
executedQuality
passCount
executedPasses[]
skippedPasses[]
fallback
issues[]
errors[]
```

降级规则：

```text
backend maxQuality 低于 graph requested quality 时，report.fallback 记录 from/to/reason。
backend 不支持 shadow 或降级到 low 时，shadow pass 会进入 skippedPasses。
```

这条规则以后会扩展到：

```text
平台能力
移动端质量档
材质特性
阴影特性
后处理特性
```

## Runtime Bundle 接入

当前 runtime bundle 已暴露：

```text
createMinimalRenderGraph
validateRenderGraph
createMaterialGraphFromRenderable
validateMaterialGraph
compileMaterialGraphToShaderIr
createRendererFeatureRequests
resolveRendererFeatureContract
createDefaultEngineRhiProfile
compileRenderGraphToEngineRhiPlan
createMockRendererBackend
createWgpuRendererBackend
runRendererPipeline
```

规则：

```text
导出 runtime 也必须能访问同一套 Renderer MVP 数据链路。
编辑器 preview 可以继续使用 Three.js，但 Three.js 不等于正式 RendererBackend。
未来 WgpuBackend / Native RHI backend 必须实现同样 RenderGraph / RenderReport 语义。
```

## 当前测试覆盖

当前测试覆盖：

```text
starter RenderSnapshot -> RenderGraph
directional light -> shadow pass
missing resource 被 validation 捕获
high quality backend 执行全部 pass
low capability backend 触发 fallback 并跳过 shadow pass
Runtime snapshot -> renderer pipeline -> RenderReport
runtimeBundle 暴露 renderer API
```

回归命令：

```powershell
npm.cmd run test:renderer
npm.cmd run test:runtimebundlerenderer
npm.cmd run build
```

## 当前边界

暂不做：

```text
真实 GPU 绘制
真实 wgpu / D3D12 / Vulkan / Metal backend
shader IR
material graph
render target 尺寸 / 格式 / barrier 细节
async compute
render pass 性能统计
编辑器 RenderGraph 可视化
```

这些属于后续 Renderer 阶段。

第一版只建立最小、可测、可审查的渲染结构闭环。

## Long-term Addendum: UE-like RenderCommand / RenderSceneState

长期 Renderer 不能以完整 RenderSnapshot 双缓冲为底层。正式规则：

```text
RenderSnapshot 是调试视图 / 过渡输入。
RenderCommand 是 Game 到 Render 的同步单位。
RenderSceneState 是 Render Thread 长期维护的渲染世界。
RenderProxy 是 Entity 在渲染侧的长期代理。
```

同步原则：

```text
Game / ECS 改变 Component。
RenderExtract 只提取 dirty visible state。
RenderExtract 生成增量 RenderCommand。
Render Thread 消费 RenderCommand 并更新 RenderSceneState。
Renderer Feature Builder 从 RenderSceneState 生成 RDG 输入。
RDG / RHI 不直接读取 Gameplay ECS。
```

允许的 RenderCommand 类型：

```text
CreateRenderProxy
DestroyRenderProxy
UpdateTransform
UpdateMesh
UpdateMaterial
UpdateLight
UpdateCamera
UpdateVisibility
UpdateSkinningData
UpdateInstanceData
```

AI 规则：

```text
AI 不直接生成 RenderCommand。
AI 生成 RenderIntent / Visual Patch / Material Graph / Preset / Quality Policy。
引擎验证后生成 RenderCommand。
每条 RenderCommand 必须带 source trace。
RenderFrameReport 向 AI 暴露本帧变化、降级、警告和成本。
```

性能规则：

```text
大型场景按变化量同步，不按场景总量复制。
完整场景级 RenderSnapshot 双缓冲禁止作为长期底层。
局部小数据可以双缓冲，例如 CameraData / FrameConstants / LightList / VisibleList / UI DrawList。
```

## Current Implementation Addendum: Material Graph / Shader IR MVP

Renderer MVP now includes a minimal material path:

```text
Renderable.mesh
  -> MaterialGraph
  -> ShaderIR
  -> RendererPipeline material report
```

MaterialGraph v1:

```text
schemaVersion: material-graph.v1
materialId
label
nodes[]
outputNodeId
source.entityId / assetId / preset
```

Current node kinds:

```text
constant
texture-sample
pbr-output
```

Current value types:

```text
color
float
texture2d
```

ShaderIR v1:

```text
schemaVersion: shader-ir.v1
shaderId
materialId
stage
quality
instructions[]
resources.textures[]
sourceMap.materialGraphId
sourceMap.nodeIds[]
fallback
```

Current instruction kinds:

```text
const_color
const_float
sample_texture
set_surface
```

Validation rule:

```text
AI should generate or patch MaterialGraph.
AI should not directly write WGSL / HLSL / MSL.
MaterialGraph must validate before ShaderIR compile.
ShaderIR keeps sourceMap back to MaterialGraph nodes for debugging and AI repair.
```

Fallback rule:

```text
Low quality ShaderIR skips baseTexture sampling and falls back to baseColor.
The fallback is recorded structurally as from / to / reason.
This allows mobile or low-end profiles to degrade materials without hiding behavior from AI or the user.
```

Current pipeline integration:

```text
runRendererPipeline returns materials[]
Each material report includes graph, shader, ok, errors, warnings.
RenderGraph still owns pass/resource dependencies.
MaterialGraph/ShaderIR owns material surface compilation.
```

Current tests:

```text
create MaterialGraph from renderable mesh fields
validate MaterialGraph
compile high quality ShaderIR with texture sampling
compile low quality ShaderIR with texture fallback
invalid MaterialGraph fails ShaderIR compile
RendererPipeline reports one material result per renderable
runtimeBundle exposes MaterialGraph / ShaderIR APIs
```

## Current Implementation Addendum: Renderer Feature Contract MVP

Renderer MVP now includes a feature contract layer:

```text
RenderSnapshot + requestedQuality + RendererBackendCapabilities
  -> RendererFeatureContract
  -> executedQuality / feature resolutions / fallback reasons
```

FeatureContract v1:

```text
schemaVersion: renderer-feature-contract.v1
requestedQuality
executedQuality
requests[]
resolutions[]
summary.accepted / degraded / unsupported
errors[]
warnings[]
```

Current feature kinds:

```text
present
opaque-renderables
directional-shadow
material-texture
```

Resolution statuses:

```text
accepted
degraded
unsupported
```

Design rule:

```text
AI should not infer renderer behavior from backend names or raw graph passes.
AI should read RendererFeatureContract to understand what was requested, what the backend can execute, and what fallback happened.
RendererFeatureContract is generated before RenderGraph execution.
RendererPipeline uses featureContract.executedQuality to build RenderGraph and compile ShaderIR.
```

Fallback examples:

```text
directional-shadow -> opaque-renderables
material-texture -> base-color
high quality -> low quality
```

Current tests:

```text
feature requests include directional-shadow when a directional light exists
feature requests include material-texture when a renderable uses textureRef
low capability backend resolves high quality requests to low quality
shadow fallback is reported structurally
material texture fallback is reported structurally
RendererPipeline graph quality follows featureContract.executedQuality
ShaderIR quality follows featureContract.executedQuality
runtimeBundle exposes FeatureContract APIs
```

## Current Implementation Addendum: Engine RHI Command Plan / Wgpu Validation Backend MVP

Renderer MVP now includes an Engine RHI command plan layer:

```text
RenderGraph
  -> EngineRhiCommandPlan
  -> RendererBackend execution report
```

EngineRhiCommandPlan v1:

```text
schemaVersion: engine-rhi-command-plan.v1
backendKind
graphQuality
commands[]
source.passCount
source.resourceCount
```

Current command kinds:

```text
create-resource
begin-pass
bind-resource
draw
end-pass
present
```

Current RHI profile:

```text
backendKind: mock / wgpu / d3d12 / vulkan / metal
supportsTransientResources
supportsShadowMap
supportsPresent
```

Boundary rule:

```text
Engine RHI is not a general-purpose graphics API wrapper.
It only serves RenderGraph execution.
It compiles validated RenderGraph data into backend-neutral engine commands.
Future Wgpu / D3D12 / Vulkan / Metal backends must preserve the same command plan and RenderReport semantics.
```

Wgpu validation backend:

```text
WgpuRendererBackend
backend name: wgpu-validation
uses EngineRhiProfile(backendKind=wgpu)
compiles RenderGraph to EngineRhiCommandPlan
returns RenderReport
stores last command plan for debug / tests
```

Current limitation:

```text
WgpuRendererBackend does not call real GPU APIs yet.
It is a validation backend for RDG -> RHI command planning.
Real wgpu device/swapchain/shader/pipeline creation belongs to the next renderer phase.
```

Current tests:

```text
RenderGraph compiles to EngineRhiCommandPlan
RHI plan contains resource creation, draw, present commands
unsupported shadow-map profile fails structurally
WgpuRendererBackend executes graph through RHI command plan
RendererPipeline can run with WgpuRendererBackend
runtimeBundle exposes Engine RHI and Wgpu validation backend APIs
```
