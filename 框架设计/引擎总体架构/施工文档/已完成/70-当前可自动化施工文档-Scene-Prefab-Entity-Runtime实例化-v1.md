# 当前可自动化施工文档：Scene / Prefab / Entity Runtime 实例化 v1

状态：已确认，可直接施工。

本文档只定义“怎么做”和“怎么验收”。长期架构规则以对应框架文档为准。

## 规则来源

```text
70-Scene-Prefab-Entity-Runtime实例化方案.md
68-Runtime资源加载系统方案.md
60-AuthoringEntityRef-RuntimeEntityHandle方案.md
15-Scene-Entity-Component-Prefab数据模型.md
16-ECS写入与项目规则边界.md
17-Runtime-FrameLoop.md
52-ECS-Storage-v1方案.md
UE源码参考/Scene-Level-Actor-Instantiation源码参考.md
Unity源码参考/Scene-Prefab-GameObject-Instantiation源码参考.md
Bevy源码参考/Scene-WorldAsset-EntityMap-Instantiation源码参考.md
```

## 目标

把 Runtime Package 里的 Scene / Prefab 数据真实实例化到 Rust ECS，让后续 FrameLoop、RenderExtract、RenderCommand 和 Runtime Renderer 能读取真实 Entity / Component。

目标链路：

```text
Runtime Package
  -> RuntimeAssetLoader load scene / prefab asset
  -> RuntimeInstanceLoader
  -> RuntimeSceneInstance / RuntimePrefabInstance
  -> Rust ECS World
  -> RenderExtract readable
```

## 采用方案

采用方案 D：

```text
Bevy-like EntityMap
+ UE-like 最小阶段化
+ Unity-like 用户心智
```

具体含义：

```text
1. 像 Bevy 一样，先为全部 source entity 分配 runtime entity。
2. 再插入 Component。
3. 再用 SourceEntityId -> RuntimeEntityId map 修正内部 EntityRef。
4. 像 UE 一样保留清晰实例化阶段，方便以后增量 / 分帧。
5. 像 Unity 一样保持用户心智简单：Scene / Prefab load 到目标 scene / parent。
```

第一版不复制：

```text
UE 完整 Actor lifecycle。
Unity native 黑盒对象系统。
Bevy 完整 reflection / TypeRegistry。
```

## 标准阶段

实例化固定为 5 个阶段：

```text
ResolveAssets
AllocateEntities
AttachComponents
RemapReferences
Activate
```

第一版可以同步一次执行完，但 Report 必须记录阶段，后续才能平滑升级为增量或分帧实例化。

## 第一版边界

必须做：

```text
RuntimeSceneInstance
RuntimePrefabInstance
RuntimeInstanceId
SourceEntityId -> RuntimeEntityId map
SceneInstantiateReport
PrefabInstantiateReport
RuntimeInstanceLoader
Scene unload
Prefab despawn
Asset handle tracking
EntityRef remap 最小规则
headless 单元测试
```

暂不做：

```text
Prefab override / variant。
运行时编辑 Prefab asset。
复杂 Scene streaming。
真实分帧 time-slice。
跨场景引用自动修复。
Project Logic lifecycle。
真实 GPU / viewport 输出。
IR Interpreter / Rust AOT 正式接入。
```

## 推荐文件

优先新增或修改：

```text
rust/crates/engine_runtime/src/runtime_instance.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
rust/crates/engine_runtime/src/runtime_instance_diagnostics.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_asset.rs
rust/crates/engine_runtime/src/runtime_asset_loader.rs
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/render_extract.rs
rust/crates/engine_runtime/src/lib.rs
scripts/test-rust-runtime-scene-prefab-instantiation.cjs
package.json
```

不要求文件名完全一致，但模块边界必须保持：

```text
RuntimeAssetLoader 只负责资源加载和 handle。
RuntimeInstanceLoader 负责 Scene / Prefab 实例化流程。
World / ECS 只负责 Entity / Component 写入与删除。
RenderExtract 只读取实例化后的 ECS 数据。
```

## 数据结构要求

### RuntimeInstanceId

```text
RuntimeInstanceId:
  value
```

要求：

```text
全局唯一。
可用于 trace / report / unload / despawn。
不能复用为 asset id。
```

### RuntimeSceneInstance

```text
RuntimeSceneInstance:
  instance_id
  scene_asset_guid
  scene_id
  root_entities
  source_to_runtime_entity
  owned_asset_handles
  state
```

### RuntimePrefabInstance

```text
RuntimePrefabInstance:
  instance_id
  prefab_asset_guid
  root_entity
  parent_entity optional
  target_scene_instance optional
  source_to_runtime_entity
  owned_asset_handles
  state
```

### Instantiate Report

```text
SceneInstantiateReport / PrefabInstantiateReport:
  request_id
  instance_id optional
  asset_ref
  stage
  created_entity_count
  loaded_asset_count
  remapped_reference_count
  diagnostics[]
  source_to_runtime_entity_debug
```

Report 只放查问题必要字段，不塞完整 Component 数据。

## Scene 实例化流程

```text
load_scene_instance(scene_ref):
  1. RuntimeAssetLoader load(scene asset)
  2. parse RuntimeSceneData
  3. collect AssetRef
  4. load required assets
  5. allocate runtime entities for every scene entity
  6. insert Transform / Hierarchy / built-in components
  7. insert project components from schema data
  8. remap EntityRef fields
  9. activate scene instance
  10. return RuntimeSceneInstance + SceneInstantiateReport
```

关键规则：

```text
Scene 文件保存 source entity / authoring entity。
Runtime 不直接复用 authoring id 作为 runtime entity。
所有内部 EntityRef 必须经过 source_to_runtime_entity map 修正。
Transform / Hierarchy 是实例化第一批必备组件。
AssetRef 字段只解析到 RuntimeAssetHandle，不直接塞真实文件路径。
```

## Prefab 实例化流程

```text
instantiate_prefab(prefab_ref, parent_entity optional, target_scene optional):
  1. RuntimeAssetLoader load(prefab asset)
  2. parse RuntimePrefabData
  3. collect AssetRef
  4. load required assets
  5. allocate runtime entities for prefab entity tree
  6. attach root to parent / target scene
  7. insert components
  8. remap internal EntityRef
  9. activate prefab instance
  10. return RuntimePrefabInstance + PrefabInstantiateReport
```

关键规则：

```text
Prefab asset 是模板。
RuntimePrefabInstance 是运行时实例。
Prefab 内部 EntityRef 指向 prefab 内 source entity 时，必须 remap 到本次实例生成的 runtime entity。
Prefab 挂到 parent 时，只改变实例根节点的层级关系，不修改 Prefab asset。
```

## Scene unload / Prefab despawn

Scene unload：

```text
unload_scene_instance(scene_instance_id):
  1. deactivate scene instance
  2. despawn all runtime entities in source_to_runtime_entity map
  3. release asset handles owned by scene instance
  4. remove instance record
  5. emit report
```

Prefab despawn：

```text
despawn_prefab_instance(prefab_instance_id):
  1. despawn all runtime entities in source_to_runtime_entity map
  2. release asset handles owned by prefab instance
  3. remove instance record
  4. emit report
```

第一版要求：

```text
删除 Entity 必须走 World / ECS API。
释放资源必须走 RuntimeAssetLoader / handle 记录。
不允许实例删除时反向修改 Scene asset / Prefab asset。
```

## 诊断规则

必须能报告：

```text
scene asset missing
prefab asset missing
asset dependency missing
unknown component type
component decode failed
entity ref target missing
parent entity missing
duplicate source entity id
unload unknown scene instance
despawn unknown prefab instance
```

诊断字段最小结构：

```text
severity
kind
message
stage
asset_guid optional
source_entity_id optional
runtime_entity_id optional
suggested_fix optional
```

## 测试要求

新增脚本：

```text
scripts/test-rust-runtime-scene-prefab-instantiation.cjs
```

新增 package script：

```text
test:runtimesceneinstance
```

最小测试用例：

```text
1. load scene into ECS creates entities / Transform / Hierarchy。
2. source_to_runtime_entity map exists and count matches source scene entities。
3. scene internal EntityRef remaps to runtime entity。
4. prefab instantiate under parent creates child hierarchy。
5. prefab internal EntityRef remaps per instance。
6. instantiate same prefab twice creates different runtime entities。
7. scene unload despawns owned entities and releases owned asset handles。
8. prefab despawn despawns owned entities and releases owned asset handles。
9. missing scene / prefab asset returns diagnostic instead of panic。
10. instantiate report stages are stable and AI-readable。
11. RenderExtract can read render-facing components after scene instantiate。
```

回归命令：

```powershell
npm.cmd run test:runtimesceneinstance
npm.cmd run test:rustruntimeassetloading
npm.cmd run test:rustruntimepackage
npm.cmd run test:rustruntime
```

如果修改 Rust crate 公共 API，还要跑：

```powershell
cargo test
```

## 验收标准

施工完成后必须满足：

```text
Runtime Package 中的 Scene 可以实例化到 Rust ECS。
Runtime Package 中的 Prefab 可以实例化到 Rust ECS。
Scene / Prefab 实例都有 RuntimeInstanceId。
实例记录能追踪 source entity -> runtime entity。
EntityRef remap 有测试覆盖。
Scene unload / Prefab despawn 有测试覆盖。
Asset handle 加载和释放有记录。
RenderExtract 能从实例化后的 World 读到可渲染数据。
所有新增测试通过。
49 / 54 / 18 / 00 / 施工文档README 在完成后同步更新。
新增阶段完成记录。
```

## 施工顺序

```text
A0：补 RuntimeSceneData / RuntimePrefabData 最小读取结构。
A1：新增 RuntimeInstanceId / RuntimeSceneInstance / RuntimePrefabInstance。
A2：新增 RuntimeInstanceLoader skeleton 和 5 阶段 report。
A3：实现 Scene AllocateEntities / AttachComponents。
A4：实现 Scene EntityRef remap。
A5：实现 Prefab instantiate under parent。
A6：实现 Prefab EntityRef remap 和多实例隔离。
A7：实现 Scene unload / Prefab despawn。
A8：接 RenderExtract 最小读取验证。
A9：补测试脚本和 package script。
A10：更新阶段完成记录与入口文档。
```

## 风险与约束

```text
不要为了第一版实例化引入完整生命周期系统。
不要把实例化阶段扩成新的复杂调度器。
不要让项目逻辑参与实例化过程。
不要让 RenderExtract 反向依赖 RuntimeInstanceLoader。
不要把 source entity id 当作 runtime entity id。
不要让 Prefab instance 修改 Prefab asset。
```

## 和成熟引擎的对应关系

```text
Unity：
  SceneManager.LoadScene / Object.Instantiate 给用户简单入口。
  我们保留简单入口，但把 native 黑盒改成可读 Report。

UE：
  Level add to world / Actor component register / BeginPlay 是阶段化流程。
  我们只保留最小阶段，不引入完整 Actor lifecycle。

Bevy：
  WorldAsset / InstanceId / EntityMap / MapEntities 适合 ECS 实例化。
  我们借用 EntityMap 思路，但不引入完整反射系统。
```

## 完成后的下一步

本施工完成后，最小游戏循环的下一个 P1 是：

```text
Runtime Renderer 输出到真实 viewport texture / surface。
Native Editor Viewport 输入进入 Runtime Action / Game Frame。
```
