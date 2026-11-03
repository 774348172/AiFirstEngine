# 最小 ECS World

## 功能目标

建立 Rust Runtime 自研 ECS 的第一版最小外壳：

```text
EntityId
Transform
Renderable
Hierarchy
alive state
World
Scene -> World loader
```

这一步只支持 Runtime Package scene 落到 World，不做复杂调度。

## 实现文件

```text
rust/crates/engine_runtime/src/ids.rs
rust/crates/engine_runtime/src/math.rs
rust/crates/engine_runtime/src/components.rs
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/scene_loader.rs
```

## 关键实现

```text
EntityId = string wrapper。
Transform = localPosition / localRotation / localScale。
Renderable = meshRef / materialRef / visible / layer。
Hierarchy = parentId / siblingOrder。
World 使用最小 HashMap 存储。
load_scene_into_world 把 RuntimeScene 转成 World。
alive_renderables 返回可渲染实体的稳定排序结果。
```

## 最小测试用例

命令：

```powershell
npm.cmd run test:rustruntime
```

覆盖测试：

```text
scene_loader_creates_entity_and_transform
world_queries_alive_renderables
```

本轮结果：

```text
Scene 可以创建 Entity 与 Transform。
World 可以查询 alive renderables。
测试通过。
```

## 边界

```text
不引入 hecs / bevy_ecs 作为项目 API。
不做 archetype storage。
不做并行调度。
不做 change detection。
不暴露用户手写 Rust System API。
```
