# 139-Sprite2D Product Runtime Rendering v1 方案

## 1. 本文解决什么

本文定义 M5：

```text
Sprite2D 产品级运行渲染链路 v1
```

它不是重新讨论 `SpriteRenderer2D` 字段，也不是重新讨论 GPU 资源生产。已有规则继续有效：

```text
96-Sprite2D-Rendering-C-min方案.md
110-World-Projection-Adapter统一跨域同步规则.md
138-Runtime-Render-Asset-Production-and-Binding-v1方案.md
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
```

本文补齐的是完整运行链路：

```text
ECS World / RenderSceneState
  -> Sprite2DRenderPipeline
  -> Camera / Layer / Visibility
  -> Sprite2DDrawPlan
  -> RuntimeRenderAssetProduction / RenderBindingSet
  -> RenderGraph
  -> RhiCommandPlan
  -> EngineRHI Backend
```

## 2. 其它引擎对应模块

### Unity

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\Graphics\GraphicsRenderers.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\2D\Sorting\ScriptBindings\SortingGroup.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\RenderPipeline\SortingCriteria.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\RenderPipeline\FilteringSettings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\RenderPipeline\ScriptableRenderContext.cs
```

Unity 对应关系：

```text
SpriteRenderer / Renderer
  -> sortingLayerID / sortingOrder
  -> CullingResults / FilteringSettings / SortingCriteria
  -> RendererList
  -> CommandBuffer.DrawRendererList
  -> native graphics backend
```

可借鉴：

```text
用户层只编辑 SpriteRenderer / Material / Sorting。
渲染层统一过滤、排序、提交。
UI / Canvas 排序不混入普通 Sprite2D 排序。
```

不照搬：

```text
Unity native 渲染黑盒不适合 AI-first。
我们必须显式输出 Projection / Sprite2D / Asset / RHI report。
```

### Unreal Engine

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\2D\Paper2D\Source\Paper2D\Private\PaperSpriteComponent.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\2D\Paper2D\Source\Paper2D\Private\PaperRenderSceneProxy.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\2D\Paper2D\Source\Paper2D\Private\PaperRenderSceneProxy.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Classes\Components\PrimitiveComponent.h
```

UE 对应关系：

```text
UPaperSpriteComponent
  -> CreateSceneProxy
  -> FPaperRenderSceneProxy
  -> GetDynamicMeshElements
  -> FMeshBatch
  -> MeshDrawCommand / Renderer / RHI
```

可借鉴：

```text
Component 不直接画。
Component 先生成 render-side proxy。
Renderer 再把 proxy 变成批次和 draw command。
透明排序字段属于渲染对象 / renderer 层，不属于项目玩法层。
```

不照搬：

```text
第一版不实现 UE 全量 PrimitiveSceneProxy / MeshPass / RDG。
第一版保留 Sprite2D 专用 pipeline，但边界必须能迁入长期 Renderer Core。
```

### Bevy / Godot

Bevy 对应：

```text
Sprite / Transform / Camera
  -> ExtractedSprite
  -> Transparent2d render phase
  -> PhaseSort
  -> Render pass
```

Godot 对应：

```text
Sprite2D / CanvasItem
  -> RenderingServer canvas item
  -> visibility layer / z_index
  -> canvas render
```

可借鉴：

```text
ECS 友好的 extract / queue / phase 心智。
2D 可见性和排序必须显式、稳定、可测试。
```

## 3. 我们与 Unity / UE 的模块对照

| 我们 | Unity | UE | 职责 |
|---|---|---|---|
| SpriteRenderer2D | SpriteRenderer / Renderer | UPaperSpriteComponent | 用户和 AI 可编辑的 2D 渲染组件 |
| Camera2D / RenderViewState | Camera / SRP camera | FSceneView / ViewFamily | 视图、投影、目标 |
| RenderProjectionAdapter<SpriteRenderer2D> | native Renderer sync | CreateSceneProxy / SendRenderDynamicData_Concurrent | 从 ECS 同步到渲染域 |
| RenderProxyPayload::Sprite | native renderer state | FPaperRenderSceneProxy | 渲染域派生状态 |
| Sprite2DRenderPipeline | URP 2D Renderer / RendererList build | Renderer mesh pass | 可见性、排序、draw plan |
| Sprite2DDrawPlan | RendererList / DrawRendererList | FMeshBatch / MeshDrawCommand | 渲染可执行绘制计划 |
| RenderBindingSet | Material/Texture binding | MaterialRenderProxy / RHI resource | 资源绑定 |
| RhiCommandPlan / EngineRHI | native graphics backend | RHI | 后端提交 |

## 4. 正式方案

采用：

```text
方案 C-min：Unity/UE-like 产品级 Sprite2D Pipeline
```

第一版建立长期正确的大结构：

```text
RenderSceneState
  -> Sprite2DRenderPipeline
  -> Sprite2DRenderFrame
  -> Sprite2DDrawPlan
  -> RenderGraph pass
  -> RhiCommandPlan
```

但第一版只做最小产品闭环，不做：

```text
Tilemap
Sprite Mask
2D Light
Sprite Animation
Atlas Packer
Dynamic Batching Optimizer
Custom Shader Graph
复杂多相机合成
```

## 5. 核心规则

### 5.1 Truth 边界

```text
SpriteRenderer2D = authoring/runtime truth
RenderProxyPayload::Sprite = render-side derived state
Sprite2DRenderPipeline = render planning owner
RenderGraph / RhiCommandPlan = execution description
```

项目逻辑不能直接写：

```text
RenderProxy
Sprite2DDrawPlan
RenderGraph pass
RhiCommandPlan
GPU resource handle
```

### 5.2 排序规则

第一版 Sprite2D draw order：

```text
camera_order
render_layer
sorting_layer
order_in_layer
sort_z
stable_proxy_id
```

规则：

```text
材质和纹理批处理不能打乱以上可见顺序。
第一版宁愿少优化，也必须保证结果稳定。
AUI 不进入 Sprite2D 排序。
```

### 5.3 可见性规则

第一版可见性只做通用基础：

```text
proxy enabled
proxy visible
SpritePayload.sprite_ref exists
view layer_mask matches proxy layer, if layer_mask exists
```

第一版不做复杂 frustum / occlusion。后续进入 Renderer Core。

### 5.4 资源绑定规则

Sprite2D 使用 `RuntimeRenderAssetProduction & Binding v1`：

```text
sprite_ref
  -> RuntimeRenderAssetRequest(Sprite2DTexture)
  -> RuntimeRenderAssetProducer
  -> RenderBindingSet
  -> DrawSpriteTextured
```

第一版允许在没有真实 runtime asset record 时生成可报告 fallback binding，但必须在 report 中说明。

### 5.5 Diagnostics

第一版 Sprite2D report 必须能解释：

```text
proxy_not_visible
sprite_missing_ref
sprite_layer_mismatch
sprite_binding_fallback
sprite_ready
```

这些错误属于引擎底座，不包含 Player / Enemy / Bullet / Health 等项目语义。

## 6. 为什么适合我们

AI 友好：

```text
AI 只修改 SpriteRenderer2D / Camera2D / AssetRef / MaterialRef。
AI 查错沿着 Projection -> Sprite2DPipeline -> AssetProduction -> RHI report。
```

复杂项目支持：

```text
背景、飞机、子弹、敌人、爆炸、世界 UI 可以通过 layer / sorting_layer / order_in_layer 稳定表达。
```

可维护：

```text
Sprite2D 绘制计划集中在 Sprite2DRenderPipeline，不继续散落在 RuntimeRenderer。
```

简单度：

```text
第一版不做全量 2D Renderer，只做产品级最小闭环。
```

效率：

```text
先保证排序正确和资源复用，后续 batching 只允许在相同排序桶内优化。
```

## 7. 方案自审

### 7.1 Specification fit

通过。本文解决 M5 Sprite2D 产品级运行链路，并明确对比 Unity / UE。

### 7.2 Rule fit

通过。没有推翻 `96 / 110 / 138`，不新增项目玩法 API。

### 7.3 Textual consistency

通过。Truth 层、派生层、pipeline 层、执行层职责一致。

### 7.4 Design fit

通过。方案优先 AI 可查、复杂项目可扩展、长期可维护，同时避免无数 bridge。

### 7.5 Implementation feasibility

通过。当前代码已有：

```text
SpriteRenderer2D
SpritePayload
RenderProxyPayload::Sprite
DrawItemSortKey
RenderBindingSet
DrawSpriteTextured
EngineRHI
```

只需新增 `Sprite2DRenderPipeline` 并接入 RuntimeRenderer。

### 7.6 Practical reasonableness

通过。第一版功能边界克制，测试可 headless 完成。

## 8. 施工入口

确认后生成施工文档：

```text
施工文档/当前/139-当前可自动化施工文档-Sprite2D-Product-Runtime-Rendering-v1.md
```
