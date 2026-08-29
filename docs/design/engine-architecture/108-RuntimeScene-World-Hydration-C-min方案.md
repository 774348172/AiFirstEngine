# 108-RuntimeScene World Hydration C-min 方案

## 当前归属说明：HydrationProjection

本文档中的 `RuntimeScene World Hydration / RuntimeSceneHydrator`，从 `110-World-Projection-Adapter统一跨域同步规则.md` 起统一归属为：

```text
HydrationProjection
```

正确链路：

```text
RuntimePackage / RuntimeScene / RuntimePrefab
  -> HydrationProjectionAdapter
  -> ECS World
```

它不是项目逻辑，也不是渲染逻辑。后续 RuntimeSpriteRenderer2D、RuntimeCollider2D 等类型进入 World 时，只新增对应 HydrationProjectionAdapter。

## 1. 问题是什么

`RuntimeScene World Hydration` 是把 Runtime Package 里的 `RuntimeScene` 纯数据实例化进 Rust ECS `World` 的正式入口。

```text
RuntimePackage / RuntimeScene
  -> Hydration
  -> World Entity + Component
  -> RuntimeSceneInstance
  -> Initial Dirty Records
  -> Logic / Physics / RenderExtract
```

它不是项目玩法系统，不包含 enemy / bullet / health / score 等项目规则。它只处理引擎底座概念：

```text
scene
entity
component
transform
hierarchy
asset ref
entity ref remap
runtime instance
report
dirty record
```

## 2. 为什么现在需要它

复杂打飞机验证已经跑通了大量底座能力，但仍暴露出一个核心缺口：

```text
RuntimePackage 能读取 RuntimeScene，
但复杂验证仍然手动创建 World。
```

这说明 `RuntimeScene -> World -> Logic / Physics / Render` 还没有一个正式、唯一、可追踪的入口。

当前代码里已有两条相关路径：

```text
scene_loader::load_scene_into_world
  早期简单工具，只支持 Entity / Transform / Hierarchy / Renderable。

RuntimeInstanceLoader
  更正式，已经支持 asset resolve、entity map、dynamic component、
  entity ref remap、scene instance、prefab instance、unload / despawn。
```

C-min 的目标不是新造第三套系统，而是把正式入口收敛到 `RuntimeInstanceLoader` 能力之上。

## 3. 其他引擎怎么做

| 引擎 | 对应机制 | 对我们的启发 |
|---|---|---|
| Unity | `SceneManager.LoadScene / LoadSceneAsync` 加载序列化 Scene，创建 GameObject 树和 Component 实例 | Scene Load 必须是官方入口，Transform / Component 进入运行时后再进入生命周期 |
| Unreal Engine | Map / Level 加载到 `UWorld / ULevel / Actor / Component`，随后注册、初始化、BeginPlay、渲染资源同步 | 场景加载应分阶段：解析资源、分配对象、挂组件、引用重映射、激活 |
| Godot | `PackedScene.instantiate()` 把场景资源实例化为 Node 树，再挂入 SceneTree | Scene 资源和运行时实例必须分开，实例需要可追踪、可释放 |
| Bevy | Scene / DynamicScene spawn 到 ECS World，并通过 EntityMap remap Entity 引用 | ECS 场景实例化必须有 source entity 到 runtime entity 的映射 |

## 4. 可选方案对比

### 方案 A：继续扩展 scene_loader

优点：

```text
最简单，改动少。
```

缺点：

```text
会形成第二套正式加载路线。
AssetRef、Prefab、Dynamic Component、Scene unload、Report 都会重复。
后期 AI 和人类调试会不知道哪个入口才是真相。
```

### 方案 B：直接把 RuntimeInstanceLoader 当作唯一入口

优点：

```text
已有能力最完整，和 Bevy-like EntityMap / UE-like 分阶段加载方向一致。
```

缺点：

```text
名字表达的是 instance loader，不够明确表达 RuntimeScene -> World 这个正式门禁。
报告也偏 instantiate report，不够面向 hydration / AI debug。
```

### 方案 C-min：新增 Hydration 门面，底层复用 RuntimeInstanceLoader

优点：

```text
只有一条真实加载路线。
保留 RuntimeInstanceLoader 已有能力。
对外语义清楚：RuntimeSceneHydrator / RuntimeSceneHydrationReport。
AI 可读、可测、可解释。
不引入项目特定规则。
```

缺点：

```text
比直接用 scene_loader 多一个正式门面。
需要逐步把真实 runtime 入口切到 Hydration。
```

## 5. 最终规则

采用方案 C-min。

```text
RuntimePackage
  -> RuntimeSceneHydrationRequest
  -> RuntimeSceneHydrator
  -> RuntimeInstanceLoader
  -> World
  -> RuntimeSceneInstance
  -> RuntimeSceneHydrationReport
```

关键规则：

```text
RuntimeScene Hydration 是 RuntimePackage 进入 World 的唯一正式入口。

真实 Runtime / Run / Play 路径必须走 Hydration。

scene_loader::load_scene_into_world 只保留为历史测试工具或兼容 helper，
不能继续扩展为第二套正式 runtime loader。

Hydration 必须通过 World 写入 API 插入组件，
由 World 的 dirty 机制自然产生 initial dirty records。

Hydration 不执行项目规则，不触发生命周期回调，不知道具体玩法概念。
```

## 6. C-min 执行阶段

第一版执行阶段：

```text
1. 创建 RuntimeSceneHydrationRequest。
2. 根据 RuntimePackage 创建 RuntimeSceneHydrator。
3. C-min 默认 mount startup bundle。
4. RuntimeInstanceLoader resolve scene asset 和依赖 asset refs。
5. 分配全部 Entity。
6. 写入 Hierarchy / Transform。
7. 写入 Renderable 等已支持的 typed engine component。
8. 写入 RuntimeProjectComponent dynamic component。
9. remap entityRef。
10. 记录本次加载新增的 initial dirty records。
11. 返回 RuntimeSceneInstance + RuntimeSceneHydrationReport。
```

## 7. 第一版支持范围

支持：

```text
RuntimeScene -> World
source entity id -> runtime entity id map
Hierarchy / Transform
RuntimeMesh -> Renderable
RuntimeProjectComponent -> Dynamic Component
entityRef remap
AssetRef 基础解析和 bundle mount 检查
initial dirty records
HydrationReport
真实 runtime headless run 接入 Hydration
```

暂不支持：

```text
复杂 async streaming
复杂 additive scene lifecycle
Prefab variant
项目规则执行
生命周期回调
热更新
完整 SpriteRenderer2D hydration，等 Sprite2D ECS-to-RenderProxy bridge 完整后接入
```

## 8. 对 AI 友好的原因

AI 后续排查场景加载问题时，只需要看一条链：

```text
RuntimePackage
RuntimeSceneHydrationReport
World dirty records
RenderExtract report
RenderFrameReport
```

不会出现：

```text
编辑器走 scene_loader
runtime 走 instance_loader
测试手写 world
渲染又假设另一个入口
```

这比增加大量隐式规则更好维护。

## 9. 后续可施工点

下一步可以继续做：

```text
SpriteRenderer2D ECS-to-RenderProxy Bridge
Editor Saved Project -> RuntimePackageBuilder Integration Gate
Project Rule Authoring / Compile / Runtime Execute Gate
```

