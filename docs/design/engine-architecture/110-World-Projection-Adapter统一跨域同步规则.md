# 110-World Projection Adapter 统一跨域同步规则

## 1. 这个文档解决什么问题

我们之前在多个系统里反复出现了 `Bridge`：

```text
RuntimeScene Hydration
RenderExtract
RenderAssetBridge
Physics2DBridge
SpriteRenderer2D ECS-to-RenderProxy Bridge
AuiRenderExtract
```

这些系统的共同点不是“桥”，而是：

```text
从一个主真相域，把数据投影到另一个运行域。
```

如果继续为每个类型、每个系统新增独立 Bridge，后期会变成无数条局部规则。AI 和人类工程师都会很难判断：

```text
这个数据是谁的真相？
什么时候同步？
同步时读什么？
写到哪里？
出了问题看哪个 report？
新增一种类型时是新增系统，还是新增适配器？
```

因此从本规则开始，项目统一采用：

```text
World Projection / Projection Adapter
```

旧的 `Bridge` 名称全部收敛为历史命名，不再作为新架构概念扩展。

## 2. 其它引擎参考

### 2.1 Unreal Engine

UE 不是每个渲染对象单独造一条桥，而是统一走 Component 生命周期：

```text
Actor / Component
  -> RegisterComponent
  -> CreateRenderState_Concurrent
  -> CreateSceneProxy
  -> MarkRenderStateDirty / MarkRenderTransformDirty
  -> EndOfFrame deferred update
  -> Render Thread / Scene
```

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Classes\Components\ActorComponent.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Components\ActorComponent.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Components\PrimitiveComponent.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\StaticMeshSceneProxy.cpp
```

关键启发：

```text
统一的是生命周期 / dirty / deferred update / render state。
不同组件只提供自己的 SceneProxy 创建逻辑。
```

### 2.2 Bevy

Bevy 的 ECS 渲染同步使用统一的 Extract 体系：

```text
Main World
  -> ExtractSchedule
  -> ExtractComponent / extract_xxx systems
  -> Render World
  -> Queue / Prepare / Render
```

源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_render\src\extract_component.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_sprite_render\src\lib.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_sprite_render\src\render\mod.rs
```

关键启发：

```text
统一的是 ExtractSchedule。
具体 Sprite / Camera / Material / UI 只注册自己的 extract adapter。
```

### 2.3 Godot

Godot 的场景节点通过统一的 Server 入口提交到运行域：

```text
Node / CanvasItem / VisualInstance
  -> notification / queue_redraw / transform changed
  -> RenderingServer
```

源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\3d\visual_instance_3d.cpp
<GODOT_SOURCE>\godot-master\godot-master\scene\2d\sprite_2d.cpp
```

关键启发：

```text
节点类型不同，但跨域提交入口统一到 RenderingServer / CanvasItem。
```

### 2.4 Unity

Unity 表面上有 `SpriteRenderer / MeshRenderer / Light` 等具体组件，但它们都基于统一的 GameObject / Component / Transform / Renderer 体系。

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\2D\Common\ScriptBindings\SpriteRenderer.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\Graphics\GraphicsRenderers.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Transform\ScriptBindings\TransformHierarchy.bindings.cs
```

关键启发：

```text
Scene 中保存组件数据。
渲染侧数据由引擎内部同步生成。
项目逻辑不直接操作底层渲染对象。
```

## 3. 我们的统一规则

### 3.1 主真相域

本引擎默认真相顺序：

```text
Authoring Project / Scene / Prefab
  -> RuntimePackage
  -> ECS World
  -> Domain Runtime State
```

规则：

```text
RuntimePackage 是发布运行输入真相。
ECS World 是运行时对象和组件真相。
RenderSceneState / Physics2DWorld / AudioState / AUI DrawList 都是派生运行域。
项目逻辑不能直接写派生运行域。
```

### 3.2 Projection Boundary

`Projection Boundary` 是跨域同步边界。

统一结构：

```text
ProjectionBoundary
  source_domain
  target_domain
  schedule_point
  dirty_or_dependency_input
  adapters
  output
  report
```

所有跨域同步必须回答：

```text
从哪里读？
写到哪里？
什么时候跑？
由谁触发？
输出什么命令或状态？
如何报告错误？
```

### 3.3 Projection Adapter

`Projection Adapter` 是某个具体类型在某个 Projection Boundary 下的转换逻辑。

统一结构：

```text
ProjectionAdapter<T>
  can_project(source)
  collect_dependencies(source)
  project(source, target)
  diagnostics()
```

不同类型只能扩展 adapter，不允许新增一条独立跨域流程。

示例：

```text
RenderProjectionAdapter<SpriteRenderer2D>
RenderProjectionAdapter<MeshRenderer>
RenderProjectionAdapter<Camera>
PhysicsProjectionAdapter<Collider2D>
HydrationProjectionAdapter<RuntimeTransform>
HydrationProjectionAdapter<RuntimeSpriteRenderer2D>
AssetProjectionAdapter<RuntimeAssetRef>
UiProjectionAdapter<AuiNode>
```

## 4. 现有系统归拢

### 4.1 RuntimeScene Hydration

旧名称：

```text
RuntimeScene Hydration
RuntimeSceneHydrator
RuntimeInstanceLoader
```

新归属：

```text
HydrationProjection
```

职责：

```text
RuntimePackage / RuntimeScene / RuntimePrefab
  -> ECS World
```

它不是项目逻辑，也不是渲染逻辑。它只负责把 RuntimePackage 数据灌入 ECS World。

### 4.2 RenderExtract

旧名称：

```text
RenderExtract
SpriteRenderer2D ECS-to-RenderProxy Bridge
```

新归属：

```text
RenderProjection
```

职责：

```text
ECS World render-facing components
  -> RenderCommand
  -> RenderSceneState / RenderProxy
```

`SpriteRenderer2D` 不是一座独立桥，而是：

```text
RenderProjectionAdapter<SpriteRenderer2D>
```

### 4.3 RenderAssetBridge

旧名称：

```text
RenderAssetBridge
```

新归属：

```text
AssetProjection
```

职责：

```text
RuntimeAssetRef / RuntimeAssetLoader
  -> typed runtime resource binding
  -> GPU resource request / handle
```

它不负责从 ECS 生成 RenderProxy，也不负责导入资源。

### 4.4 Physics2DBridge

旧名称：

```text
Physics2DBridge
```

新归属：

```text
Physics2DProjection
```

职责：

```text
ECS World physics-facing components
  -> Physics2DWorld
  -> CollisionPairReport / Physics2DTrace
```

第一版只同步 `Transform / Collider2D / layer / mask` 等基础数据，不引入项目玩法语义。

### 4.5 AUI Render Extract

旧名称：

```text
AuiRenderExtract
AuiRendererBridge
```

新归属：

```text
UiProjection
```

职责：

```text
AUI Tree / Layout / DrawList
  -> UI Render Commands
  -> RuntimeRenderer UI pass
```

它不负责 AI 生成图片，也不负责项目 UI 业务逻辑。

## 5. 命名规则

从本规则之后，新系统禁止随意命名为：

```text
xxxBridge
xxx桥
xxx独立同步器
```

除非它是与第三方进程、IPC、网络、外部工具通信的真实桥接。

引擎内部跨域同步统一使用：

```text
xxxProjection
xxxProjectionAdapter
xxxProjectionReport
```

历史代码暂时可以保留旧类型名，但文档和新增代码必须把它们解释为 Projection Adapter 的历史落地。

## 6. 第一版统一结构

第一版不强行一次性重构所有代码文件名，但要建立统一抽象心智：

```text
ProjectionDomain:
  RuntimePackage
  World
  Render
  Physics2D
  AssetRuntime
  UI

ProjectionKind:
  Hydration
  Render
  Physics2D
  Asset
  UI

ProjectionReport:
  projection_kind
  source_domain
  target_domain
  adapter_name
  projected_count
  skipped_count
  error_count
  diagnostics
```

## 7. SpriteRenderer2D 的第一落地

`SpriteRenderer2D` 必须作为统一规则下的第一个落地用例，而不是新增独立桥。

正确链路：

```text
RuntimePackage.RuntimeSpriteRenderer2D
  -> HydrationProjectionAdapter<RuntimeSpriteRenderer2D>
  -> ECS World.SpriteRenderer2D
  -> RenderProjectionAdapter<SpriteRenderer2D>
  -> RenderCommand::AddProxy / UpdateRenderState
  -> RenderProxyPayload::Sprite
```

规则：

```text
RuntimeScene 保存 SpriteRenderer2D authoring/runtime 数据。
HydrationProjection 写入 ECS World。
RenderProjection 读取 ECS World。
项目逻辑不能直接写 RenderProxyPayload::Sprite。
```

## 8. AI 友好原因

AI 排查问题时只需要沿着统一结构看：

```text
source domain
projection kind
adapter
target domain
report
```

不需要再猜：

```text
这个 Bridge 是资源桥？
渲染桥？
物理桥？
RuntimePackage 桥？
它和 RenderExtract 谁先谁后？
```

AI 新增一个类型时，默认问题也统一：

```text
这个类型属于哪个 source domain？
目标 domain 是什么？
需要哪个 ProjectionAdapter？
依赖哪些资源？
输出什么 report？
```

## 9. 长期路线

长期应形成统一调度：

```text
EngineHostLoop
  -> HydrationProjection, only on load/spawn
  -> ProjectLogicRunner
  -> Physics2DProjection
  -> RenderProjection
  -> UiProjection
  -> RenderThread / RuntimeRenderer
```

第一版可以继续使用已有函数和模块，但命名和文档必须收敛。

## 10. 禁止事项

禁止为了单个类型新增独立跨域系统。

禁止项目逻辑直接操作：

```text
RenderProxy
Physics2DWorld internal body
GPU resource handle
RenderSceneState
UI render pass command
```

禁止在 Projection Adapter 中加入项目玩法语义，例如：

```text
enemy
bullet
damage
health
score
weapon
skill
buff
```

这些只能存在于项目层 Schema / Rule / Module 中。

