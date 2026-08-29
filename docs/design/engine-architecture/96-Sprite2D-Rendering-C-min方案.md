# 96-Sprite2D Rendering C-min 方案

## 当前归属说明：Sprite2D 与 RenderProjection

Sprite2D 渲染侧结构仍以本文为准；但 `SpriteRenderer2D` 从 ECS World 同步到 `RenderProxyPayload::Sprite` 的链路，统一归属为：

```text
RenderProjectionAdapter<SpriteRenderer2D>
```

后续不再新增 `Sprite2D Bridge` 或独立同步系统。Sprite2D 的规则边界是：

```text
SpriteRenderer2D = ECS / authoring truth
RenderProxyPayload::Sprite = render-side derived state
RenderProjectionAdapter<SpriteRenderer2D> = 唯一同步路径
```

本文档定义 Sprite2D 第一版的最小长期规则。

它承接：

```text
55-RenderSceneState-RenderProxy-v1方案.md
57-当前可自动化施工文档-EngineHostLoop-RendererFeatureBuilder-MinimalRenderer-v1.md
90-ProjectAsset-to-SceneEntity-Authoring-C-min方案.md
91-AI图片生成到项目资源库闭环-C-min方案.md
93-复杂打飞机验证所需引擎侧缺失能力清单.md
```

## 1. 本文解决什么

当前引擎已经有：

```text
RenderProxyPayload::Sprite
RendererFeatureBuilder
RendererFeatureDrawItem
MinimalRenderer CPU 输出
Project asset -> Scene entity 的基础链路
AI 图片生成 -> 项目资源库的基础链路
```

但 Sprite2D 还缺少正式字段和排序规则：

```text
SpriteRenderer2D 第一版字段结构是什么。
SpritePayload 应保存哪些通用渲染数据。
RendererFeatureDrawItem 如何携带 Sprite2D 排序信息。
DrawItem sort_key 如何保证显示结果稳定。
材质批处理排序、透明深度排序、摄像机距离排序、多 Canvas 排序分别归属哪里。
```

## 2. 成熟引擎参考

### Unity

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\Graphics\GraphicsRenderers.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\2D\Sorting\ScriptBindings\SortingGroup.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\2D\SortingLayer.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\RenderPipeline\SortingCriteria.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\UI\ScriptBindings\UICanvas.bindings.cs
```

Unity 的做法：

```text
SpriteRenderer / Renderer 使用 sortingLayerID / sortingOrder。
Camera / GraphicsSettings 有 transparencySortMode / transparencySortAxis。
RenderPipeline SortingCriteria 区分 SortingLayer / RenderQueue / BackToFront / OptimizeStateChanges / CanvasOrder。
Canvas 自己有 sortingOrder / sortingLayerID / renderOrder。
```

结论：

```text
Sprite 排序、透明排序、材质状态优化、Canvas 排序不是同一层规则。
```

### Unreal Engine

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Classes\Components\PrimitiveComponent.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Renderer\Private\BasePassRendering.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Renderer\Private\MeshDrawCommands.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\UMG\Public\Blueprint\UserWidget.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\UMG\Public\Components\CanvasPanelSlot.h
```

UE 的做法：

```text
透明对象排序属于 Primitive / Renderer。
TranslucencySortPriority / TranslucencySortDistanceOffset 是渲染对象上的高级字段。
MeshDrawCommand 会做 renderer 内部排序和状态优化。
UMG / Slate 使用 ZOrder / LayerId 处理 UI 顺序。
```

结论：

```text
Renderer Core 负责透明和 draw command 排序。
UI 负责 Widget / Canvas 顺序。
项目逻辑不直接参与底层材质批处理排序。
```

### Bevy / Godot

Bevy 的 2D 渲染也把 Sprite / Mesh / Camera 作为通用渲染组件和 render extract 数据处理。
Godot 的 Sprite2D / CanvasItem / RenderingServer 也区分 Scene 节点、2D 可见顺序和底层渲染服务。

对本项目的启发：

```text
Sprite2D 第一版应该是通用组件，不包含项目玩法语义。
排序规则必须简单、确定、可测试。
Renderer 内部优化不能改变用户可见顺序。
UI / Canvas 排序必须独立于 Sprite2D。
```

## 3. 正式方案

### 3.1 SpriteRenderer2D 第一版字段

第一版标准字段：

```text
SpriteRenderer2D
  sprite_ref: Option<String>
  material_ref: Option<String>
  color: [f32; 4]
  flip_x: bool
  flip_y: bool
  sorting_layer: i16
  order_in_layer: i32
  sort_z: f32
  visible: bool
```

默认值：

```text
sprite_ref = None
material_ref = None
color = [1.0, 1.0, 1.0, 1.0]
flip_x = false
flip_y = false
sorting_layer = 0
order_in_layer = 0
sort_z = 0.0
visible = true
```

边界：

```text
SpriteRenderer2D 是引擎通用视觉组件。
它不理解 Player / Enemy / Bullet / Damage / Score。
项目对象通过 Prefab / Schema 使用 SpriteRenderer2D。
```

### 3.2 SpritePayload 第一版字段

RenderProxyPayload::Sprite 保存渲染侧状态：

```text
SpritePayload
  sprite_ref: Option<String>
  material_ref: Option<String>
  color: [f32; 4]
  flip_x: bool
  flip_y: bool
  sorting_layer: i16
  order_in_layer: i32
  sort_z: f32
```

规则：

```text
SpriteRenderer2D 是 ECS / authoring 输入。
SpritePayload 是 RenderSceneState 里的 render-side state。
项目逻辑不能直接写 SpritePayload。
RenderExtract / RenderCommand 负责从 SpriteRenderer2D 更新 SpritePayload。
```

### 3.3 DrawItem sort_key 第一版结构

RendererFeatureDrawItem 增加 Sprite2D 排序字段：

```text
RendererFeatureDrawItem
  proxy_id
  source_entity_id
  payload_kind
  mesh_ref optional
  sprite_ref optional
  material_ref optional
  transform
  visible
  layer
  sorting_layer
  order_in_layer
  sort_z
  sort_key
```

第一版 sort_key：

```text
DrawItemSortKey
  render_domain_order
  sorting_layer
  order_in_layer
  sort_z_quantized
  stable_proxy_id
```

Sprite2D 可见顺序：

```text
Sprite2D draw order =
  render_domain(Sprite2D)
  sorting_layer
  order_in_layer
  sort_z / Transform.z
  stable_proxy_id
```

第一版规则：

```text
RendererFeatureBuilder 生成 draw item 后必须按 sort_key 稳定排序。
sort_z 默认来自 SpriteRenderer2D.sort_z。
如果 SpriteRenderer2D.sort_z 未显式设置，第一版可使用 Transform.local_position.z。
stable_proxy_id 用于相同排序字段下的确定性兜底。
```

### 3.4 排序归属边界

正式归属：

```text
Sprite2D 可见顺序:
  属于 Sprite2D Rendering C-min。
  由 sorting_layer / order_in_layer / sort_z / stable_proxy_id 决定。

透明深度排序:
  属于 Renderer Core / Camera / Material pass。
  不进入 Sprite2D C-min。

摄像机距离排序:
  属于 Renderer Core / Camera mode。
  2D 正交 Sprite 第一版默认不使用。

材质批处理排序:
  属于 Renderer 内部优化。
  不暴露给项目规则和 AI patch。
  只能在相同 Sprite2D 排序桶内合并，不能打乱可见顺序。

多 Canvas / Panel / ZOrder:
  属于 AUI Runtime UI 系统。
  不参与 Sprite2D draw order。
```

## 4. 为什么不等完整渲染系统再做

本方案只定义 CPU 侧最小接口契约：

```text
SpriteRenderer2D
  -> SpritePayload
  -> RendererFeatureDrawItem
  -> deterministic sort_key
```

它不绑定：

```text
具体 RHI backend
真实 GPU sprite batch
完整透明渲染算法
完整材质系统
完整 AUI Runtime UI / Canvas
```

因此可以先施工。

后续 Renderer Core 讨论透明、Camera distance、material batching、RDG pass 时，只能在这个契约之后扩展，不能反过来让 Sprite2D 第一版等待完整渲染系统。

## 5. AI 友好规则

```text
AI 修改 Sprite 显示时，只改 SpriteRenderer2D / Asset / MaterialRef。
AI 不直接生成 RenderProxy patch。
AI 不直接生成 DrawItem sort_key。
AI 不直接配置底层 material batching。
Trace / Report 可以展示 SpriteRenderer2D 字段和最终 sort_key。
```

## 6. 第一版测试要求

必须有 headless 单元测试：

```text
SpritePayload 默认字段正确。
Sprite draw item 携带 sprite_ref / material_ref / color / flip / sorting 字段。
draw_items 按 sorting_layer / order_in_layer / sort_z / proxy_id 稳定排序。
相同排序桶内保留 proxy_id 稳定顺序。
不可见 Sprite 不进入 draw_items 或产生 proxy_not_visible diagnostic。
Mesh draw item 现有行为不被破坏。
```

## 7. 下一步

可以直接生成施工文档并施工：

```text
96-当前可自动化施工文档-Sprite2D-Rendering-C-min.md
```

施工范围只做 CPU/headless 数据闭环，不做真实 GPU Sprite Batch。
