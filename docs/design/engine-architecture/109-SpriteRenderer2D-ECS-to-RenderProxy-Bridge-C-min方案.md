# 109-SpriteRenderer2D ECS-to-RenderProxy Bridge C-min 方案

## 当前归属说明：RenderProjectionAdapter<SpriteRenderer2D>

本文档标题中的 `Bridge` 是历史命名。从 `110-World-Projection-Adapter统一跨域同步规则.md` 起，本系统正式归属为：

```text
RenderProjectionAdapter<SpriteRenderer2D>
```

正确链路：

```text
ECS World.SpriteRenderer2D
  -> RenderProjectionAdapter<SpriteRenderer2D>
  -> RenderCommand
  -> RenderSceneState / RenderProxyPayload::Sprite
```

项目逻辑不能直接写 `RenderProxyPayload::Sprite`。后续 MeshRenderer / Camera / Light / Particle 等类型也应按同一规则新增 RenderProjectionAdapter，不新增独立 Bridge。

## 1. 问题是什么

`SpriteRenderer2D ECS-to-RenderProxy Bridge` 是把 ECS `World` 里的 `SpriteRenderer2D` 组件同步成渲染侧 `RenderProxyPayload::Sprite` 的正式桥。

```text
World / ECS SpriteRenderer2D
  -> DirtyRecord(RenderState)
  -> RenderExtract
  -> RenderCommand
  -> RenderSceneState
  -> RenderProxyPayload::Sprite
  -> RendererFeatureBuilder
  -> RuntimeRenderer
```

它不是 Sprite2D 渲染系统的重新设计。`96-Sprite2D Rendering C-min` 已经完成了 Sprite2D 渲染侧 CPU/headless 最小闭环，本方案只补中间缺失链路。

## 2. 当前已有能力和缺口

已有能力：

```text
ComponentTypeId::sprite_renderer2d
SpriteRenderer2D struct
RenderProxyPayload::Sprite
SpritePayload
RendererFeatureBuilder sprite draw item
RuntimeRenderer sprite draw pass
```

缺口：

```text
ComponentValue::SpriteRenderer2D 尚未存在。
World 尚不能 typed 读写 SpriteRenderer2D。
RenderExtract 尚不能把 SpriteRenderer2D 转成 Sprite proxy。
RenderCommandPayload 当前 AddProxy 仍偏向 Renderable / Mesh。
RuntimeScene Hydration 尚不能把 RuntimeScene 的 Sprite2D 数据灌入 World。
```

## 3. 其他引擎怎么做

| 引擎 | 对应流程 | 对我们的启发 |
|---|---|---|
| Unity | `GameObject + Transform + SpriteRenderer` 进入 native renderer，底层生成可渲染对象并按 sortingLayer / sortingOrder 排序 | 用户改 SpriteRenderer，不能直接改底层渲染对象 |
| Unreal Engine | `Actor / Component` 同步为 `PrimitiveSceneProxy`，Renderer 再生成 draw command | Component 和 render proxy 分离，Game 侧不直接写 RenderThread 状态 |
| Godot | `Node2D / Sprite2D / CanvasItem` 同步到 `RenderingServer` canvas item | Scene node 是 authoring/runtime 对象，RenderingServer 持有渲染侧状态 |
| Bevy | Main World 里的 Sprite component 经 Extract 进入 Render World，再 Prepare / Queue / Render | ECS 组件通过 extract/sync 桥进入渲染侧，和我们最接近 |

结论：

```text
SpriteRenderer2D 应该是 ECS / authoring truth。
RenderProxyPayload::Sprite 应该是 render-side truth。
中间必须有唯一同步桥，不能让项目逻辑直接写 RenderProxy。
```

## 4. 可选方案对比

### 方案 A：继续用 Renderable quad 代替 Sprite

优点：

```text
不用改底层。
```

缺点：

```text
长期错误。
Sprite 的 sprite_ref / color / flip / sorting 字段会被塞进 mesh 概念。
AI 和人类会分不清 Sprite 到底是 Mesh 还是 Sprite。
```

### 方案 B：允许项目逻辑直接写 RenderProxyPayload::Sprite

优点：

```text
实现快。
```

缺点：

```text
破坏 ECS -> Render 的边界。
项目规则会直接依赖渲染侧内部结构。
后期 RenderProxy 修改会连带破坏项目逻辑。
AI 也会更容易生成错误 patch。
```

### 方案 C-min：补正式 ECS-to-RenderProxy Bridge

优点：

```text
边界清晰。
复用现有 DirtyRecord / RenderExtract / RenderCommand / RenderSceneState。
AI 只需要修改 SpriteRenderer2D。
RenderProxy 仍由引擎生成。
```

缺点：

```text
需要扩展 ComponentValue、World、RenderCommandPayload 和 RenderExtract。
```

## 5. 最终规则

采用方案 C-min。

```text
SpriteRenderer2D 是 ECS / authoring truth。
RenderProxyPayload::Sprite 是 render-side truth。
RenderExtract 是唯一同步桥。
项目逻辑不能直接写 RenderProxyPayload::Sprite。
```

AI 修改规则：

```text
AI 可以修改 SpriteRenderer2D.sprite_ref。
AI 可以修改 SpriteRenderer2D.material_ref。
AI 可以修改 SpriteRenderer2D.color / flip / sorting 字段。
AI 不能直接生成 RenderProxy patch。
AI 不能直接配置底层 material batching。
```

## 6. C-min 数据流

```text
World.insert_sprite_renderer2d(entity, sprite)
  -> mark DirtyType::RenderState
  -> RenderExtract sees sprite_renderer2d
  -> AddProxy / UpdateRenderState with RenderProxyPayload::Sprite
  -> apply_batch writes RenderSceneState
  -> RendererFeatureBuilder reads SpritePayload
  -> RuntimeRenderer emits sprite draw pass
```

Transform 修改仍只负责：

```text
DirtyType::Transform
  -> UpdateTransform
```

如果实体有 `SpriteRenderer2D` 但没有 `Transform`，第一版不创建 proxy，并给后续 diagnostics 留口。

## 7. 第一版施工范围

必须支持：

```text
ComponentValue::SpriteRenderer2D
ComponentColumn::SpriteRenderer2D
ArchetypeTable sprite_renderer2d(row)
World.insert_sprite_renderer2d / sprite_renderer2d / remove_sprite_renderer2d
dirty_type_for_component(engine.sprite_renderer2d) = RenderState
RenderCommandPayload 可承载 SpritePayload
RenderExtract 从 SpriteRenderer2D 生成 AddProxy / UpdateRenderState
RenderCommand apply 写入 RenderProxyPayload::Sprite
headless tests 覆盖 ECS -> RenderProxy -> RendererFeatureBuilder / RuntimeRenderer
```

暂不支持：

```text
真实 GPU sprite batching
复杂 atlas packing
复杂透明深度排序
Canvas / AUI 排序
项目玩法规则
完整 Sprite import runtime data 解码
```

## 8. RuntimeScene Hydration 关系

本方案第一版先补通 ECS -> RenderProxy 桥。

RuntimeScene 对 Sprite2D 的正式 JSON 表达可以后续单独补，但规则已经确定：

```text
RuntimeScene Hydration 写入 SpriteRenderer2D。
Hydration 不能直接写 RenderProxyPayload::Sprite。
```

## 9. 完成标准

```text
SpriteRenderer2D 写入 World 后能产生 RenderState dirty。
RenderExtract 能生成 Sprite AddProxy。
RenderSceneState 中 proxy.payload 是 Sprite。
RendererFeatureBuilder 能生成 sprite draw item。
RuntimeRenderer 能生成 sprite draw pass。
Mesh / Renderable 现有行为不回退。
```

