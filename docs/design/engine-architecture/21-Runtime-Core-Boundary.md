# Runtime Core Boundary / TypeScript Runtime 退役边界

> Current Status Notice：本文档保留为旧 TypeScript Runtime 过渡期边界记录。当前正式 runtime 主线是 Rust Native Runtime；TypeScript / Electron 原型层已归档到 `legacy/typescript-prototype/`。

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

> 当前状态：本文档保留为旧 TypeScript Runtime 过渡期边界记录。正式 runtime 主线已经迁移到 Rust Native Runtime、ECS FrameLoop、RenderExtract、RenderCommand、RenderSceneState 和 RenderFrameReport。后续新能力不得继续围绕 TypeScriptRuntimeBackend、`extractRenderSnapshot()` 或完整 RenderSnapshot 设计。

本文档记录当前阶段的正式 runtime 边界。

## 目标

当前项目仍然使用 TypeScript runtime 作为过渡 prototype，但编辑器、验证层、后续 Rust Runtime 不能继续直接依赖 `tickScene / createRuntimeState` 这类具体实现函数。

本阶段目标：

```text
Editor / Validation / Tooling
  -> RuntimeBackend 接口
  -> 当前 TypeScriptRuntimeBackend 过渡实现
  -> 后续 Rust Runtime Bridge
  -> Rust Native Runtime
```

Rust Native Runtime 是唯一正式 runtime。TypeScriptRuntimeBackend 只用于 Rust Runtime MVP 出现前的过渡，后续必须退役。

## RuntimeBackend v1

第一版接口：

```ts
type RuntimeBackend = {
  name: string;
  loadProject(project: GameProject): RuntimeLoadResult;
  loadScene(sceneId: string): RuntimeLoadResult;
  tick(input: InputState, dt: number): RuntimeTickResult;
  extractRenderSnapshot(): RenderSnapshot;
  getRuntimeTrace(): RuntimeTraceReport;
  validateRuntime(): RuntimeValidationReport;
};
```

兼容说明：

```text
extractRenderSnapshot() 只代表旧 TypeScript prototype 的兼容接口。
正式 Rust Runtime 不以 RenderSnapshot 作为 Game -> Render 同步模型。
正式 Rust Runtime 通过 RenderExtract 生成 RenderCommand，并由 RenderSceneState 维护渲染侧长期状态。
AI / Debug 默认读取 RuntimeTrace / RenderFrameReport / FrameHash。
```

接口原则：

```text
loadProject / loadScene 管运行时加载边界。
tick 管单帧推进。
extractRenderSnapshot 管逻辑世界到渲染输入的提取。
getRuntimeTrace 管系统执行可观察性。
validateRuntime 管 runtime smoke report。
```

RuntimeTraceReport 当前包含：

```text
trace:
  ECS system trace

irRules:
  IR Interpreter rule trace
  ruleId / path / op / label / fields / sourceMap
  来源包括显式 runIrRuleForTrace 和真实 system 内部 IR trace
```

## 当前实现

新增代码：

```text
src/runtime/RuntimeBackend.ts
src/runtime-backends/typescript/TypeScriptRuntimeBackend.ts
src/services/runtimeService.ts
scripts/test-runtime-backend.cjs
```

当前 TypeScriptRuntimeBackend 包装现有能力：

```text
createRuntimeState
tickScene
extractRenderSnapshot
listRuntimeSystems
getRuntimeSystemTrace
listComponentSchemas
runIrRuleForTrace
```

这些函数仍然存在，但它们属于 backend 内部实现，不再是编辑器和验证层的主要入口。

## 已接入位置

```text
src/engine/projectValidation.ts
  runRuntimeSmoke 通过 RuntimeBackend 执行。

src/App.tsx
  Runtime Trace 面板通过 RuntimeBackend 获取 systems / trace。
  Runtime Trace 面板展示 irRules。
  Viewport 播放和单步通过 RuntimeBackend tick。
  Viewport 渲染快照通过 backend.extractRenderSnapshot。
```

## TypeScript Backend 定位

TypeScriptRuntimeBackend 是 transition code：

```text
它用于早期编辑器、Schema、Patch、Trace、Replay 流程验证。
它不再新增正式玩法能力。
它不作为未来 Rust Runtime 的对照 backend。
它不是最终发布 runtime。
Rust Runtime MVP 跑通后，它必须退役并删除运行路径。
```

后续 Rust Runtime Bridge 需要满足 RuntimeService 需要的调用边界，并通过 Golden Scenario Test 验证 runtime 行为。

## Rust Runtime 正式边界

正式路线：

```text
Editor / Validation / Tooling
  -> RuntimeService
  -> Rust Runtime Bridge
  -> Rust Native Runtime
  -> RenderSnapshot / RuntimeTrace / FrameHash / Diagnostics
```

验证标准：

```text
Project Schema
Scene Data
Component Schema
Canonical Rule IR
FrameLoop Spec
Golden Scenario Test
```

不再要求：

```text
TypeScript Runtime vs Rust Runtime 等价测试
TypeScript Runtime 作为长期对照 backend
编辑器长期保留 TypeScript / Rust runtime 选择
```

## Runtime Asset Loading Boundary

正式 Rust Runtime 必须提供资源加载边界：

```text
load_sync(asset_ref) -> LoadedAssetHandle / Error
load_async(asset_ref | asset_set, options) -> AssetLoadRequest
poll_load(request_id) -> AssetLoadStatus
await_load(request_id) -> AssetLoadResult
cancel_load(request_id)
release(handle)
is_loaded(asset_ref)
get_loaded(asset_ref)
```

边界规则：

```text
Runtime 支持同步加载和异步加载。
异步加载是运行时默认推荐路径。
同步加载只用于启动、编辑器、加载界面、小型必需资源和测试。
运行时热路径同步加载必须产生 diagnostics warning。
分阶段加载不属于 Runtime 底层固定模式。
分阶段加载由 Project Loading Rule / Scene Lifecycle 编排。
```

Runtime 负责：

```text
AssetRef -> Active Asset Version -> Bundle / Runtime Package -> Cooked Asset
依赖解析
IO / decode / GPU upload 调度
RuntimeAssetHandle
资源缓存
引用计数
release / unload 执行
status / progress / priority / cancel
RuntimeTrace / diagnostics / report
```

Project 负责：

```text
何时 preload
何时等待加载完成
何时实例化
何时激活场景
何时释放旧资源
是否拆成多个加载阶段
加载界面和黑屏策略
```

AI 规则：

```text
AI 默认生成 LoadPlan / Lifecycle Rule。
AI 不直接生成底层 IO / decode / GPU upload 调用。
AI 必须能通过 diagnostics 解释资源加载失败、缺失依赖、未 mount bundle、同步加载热路径 warning。
```

## RuntimeAssetIndex / CookedAsset Boundary

Rust Runtime 资源加载正式边界：

```text
Runtime 不读取完整编辑器 Asset DB。
Runtime 只读取 Runtime Package 内的 RuntimeAssetIndex / bundle_table / cooked_asset_table / dependency_table。
RuntimeAssetIndex 是 Runtime 资源加载的唯一索引真相。
AssetRef 通过 RuntimeAssetIndex 解析到 cookedAssetId / bundleId / loader_kind。
```

Rust Runtime 解析流程：

```text
AssetRef(guid, assetId, type, subAsset)
  -> RuntimeAssetIndex.find(guid)
  -> validate assetId / type / subAsset
  -> cookedAssetId
  -> bundleId
  -> cooked_asset_table
  -> type_loader_table
  -> load_sync / load_async
  -> RuntimeAssetHandle
```

RuntimeAssetHandle 最小字段：

```text
handle_id
asset_guid
asset_id
asset_type
sub_asset_id
cooked_asset_id
bundle_id
runtime_resource_id
state: Loading | Ready | Failed | Released
ref_count
generation
loader_kind
```

依赖规则：

```text
依赖在 Build 阶段展开。
Rust Runtime 不重新推导业务依赖。
Rust Runtime 只按 dependency_table 校验和执行。
Runtime 必须检查依赖是否存在、bundle 是否 mounted、cooked asset 是否可读、类型是否匹配。
```

第一版边界：

```text
只支持本地 Runtime Package。
只支持本地 cooked asset。
只支持本地 bundle mount。
支持同步 / 异步加载。
支持依赖校验。
支持 RuntimeAssetHandle。
支持 diagnostics。
支持 release。
```

第一版不做：

```text
远程下载
热更 mount
加密
压缩分块 streaming
平台 CDN
增量 patch
跨版本资源替换
运行时重新 cook
编辑器 Asset DB 直连 Runtime
```

## Scene / Prefab Instantiation Boundary

Rust Runtime 加载 Scene / Prefab 的正式边界：

```text
Runtime Package Scene / Prefab
  -> SceneInstantiator
  -> Rust ECS World
```

核心规则：

```text
Scene / Prefab 直接实例化为 Rust ECS Entity / Component。
Runtime 不创建 GameObject / Actor 中间层。
Prefab 是 Entity 模板，不是 Runtime 特殊对象。
Transform 是每个 Scene / Prefab Entity 的必备 Component。
Scene 实例化不强制同步加载所有资源，只校验 AssetRef 可解析。
资源 preload / release 时机由项目侧 Loading Rule / Scene Lifecycle 控制。
```

SceneInstantiator 职责：

```text
读取 Runtime Package manifest。
查找 scene_table / prefab_table。
校验 schemaVersion。
创建 SceneInstanceId。
分配 runtimeEntityId。
建立 sourceEntityId -> runtimeEntityId 映射。
写入 Transform 和普通 Component。
建立 Parent / Children 层级。
展开 Prefab Instance。
应用 Prefab Overrides。
修复 EntityRef / AssetRef。
提交到 ECS World。
发出 SceneLoaded / SceneActivated。
```

ID 边界：

```text
sourceEntityId 来自编辑器 / Runtime Package，是稳定可追踪 ID。
runtimeEntityId 来自 Rust ECS World，是运行时临时 ID。
Trace / Diagnostics / AI Debug 必须能从 runtimeEntityId 回查 sourceEntityId。
```

## Runtime EntityRef / Handle Boundary

Runtime Entity 引用必须采用 generation 校验，不允许把裸 index 当作长期有效引用。

参考路线：

```text
Unity:
  UnityEngine.Object 保存 EntityId / InstanceID，并通过引擎侧有效性检查判断 native object 是否仍存在。

Unreal:
  FWeakObjectPtr 保存 ObjectIndex + ObjectSerialNumber。
  槽位复用后 serial 不同，旧引用不会误指向新 UObject。

Bevy:
  Entity 保存 index + generation。
  despawn / reuse 后旧 Entity 访问会返回 not spawned / invalid。
```

本引擎正式规则：

```text
RuntimeEntityId = index + generation。
RuntimeEntityHandle = RuntimeEntityId + sceneInstanceId + sourceEntityId? + issuedFrame? + debugName?。
SourceEntityId 只用于编辑器、Runtime Package、Trace 和 AI Debug。
RuntimeEntityId 只在当前 Runtime World 内有效。
RuntimeEntityHandle 是跨系统、事件、延迟请求、Trace 中保存 Entity 引用的默认形式。
项目规则 / AI / 编辑器不能保存或传递裸 ECS 指针。
```

使用 Entity 前必须通过 Runtime resolve：

```text
resolve_entity(handle):
  index 不存在 -> entity_not_found
  generation 不匹配 -> generation_mismatch
  entity 已 pending_despawn -> pending_despawn
  scene 已卸载 -> scene_unloaded
  否则返回可访问 Entity
```

性能边界：

```text
ECS Query 热循环内部可以使用本次 query 已验证的直接访问。
跨系统保存、事件队列、延迟请求、Scene / Prefab 引用修复、Trace / AI Debug 必须使用 RuntimeEntityHandle。
不要为每个 Component 字段读写都额外 resolve；只在边界和持久引用入口 resolve。
```

失效诊断：

```text
Despawn 后旧 RuntimeEntityId 立即失效。
同一个 index 复用时必须增加 generation。
旧 handle 不能命中新 Entity。
Runtime 记录有界 tombstone diagnostics，用于 Debug / Trace / AI 查错。
```

tombstone 最小字段：

```text
runtimeEntityId
sourceEntityId?
sceneInstanceId?
despawnFrame
despawnReason
lastKnownName?
```

第一版不做：

```text
复杂引用所有权图。
自动强引用 / 引用计数保活 Entity。
项目侧自定义 EntityRef 生命周期规则。
旧引用自动重绑定到新 Entity。
```

## Scene Lifecycle / Ownership Boundary

Rust Runtime 必须用 SceneInstanceId 管理一次 Scene 加载产生的 Entity 集合。

正式边界：

```text
SceneInstanceId = 一次 Scene 加载产生的运行时实例 ID。
EntityOwner = Runtime Entity 的生命周期归属。
Scene unload = Entity 生命周期操作。
Asset release = Loading Rule / Asset Runtime 操作。
```

EntityOwner：

```text
SceneOwned(sceneInstanceId)
RuntimeOwned(ownerSceneInstanceId optional)
Persistent
```

Runtime 执行规则：

```text
SceneOwned Entity 随 SceneInstance unload 一起 despawn。
RuntimeOwned Entity 默认归属当前 active SceneInstance。
RuntimeOwned 如果 Spawn 指定 parent，则继承 parent 的 EntityOwner。
RuntimeOwned 如果显式指定 ownerSceneInstanceId，则随该 SceneInstance unload 一起 despawn。
Persistent Entity 不随普通 Scene unload despawn。
Persistent Entity 必须显式声明，不能由普通 AI 生成流程默认创建。
AI 创建 Persistent Entity 必须带 reason，并进入 Validation / 用户询问流程。
Scene unload 后，runtimeEntityId 立即失效。
Trace / Diagnostics 仍可通过 sourceEntityId / SceneInstanceId 回查来源。
```

生命周期事件：

```text
SceneLoadRequested
SceneLoaded
SceneActivated
SceneUnloadRequested
SceneDeactivated
SceneUnloaded
EntitySpawned
EntityDespawned
```

Scene unload 不直接释放资源：

```text
Runtime 负责销毁 Entity / Component 和清理引用映射。
AssetRuntime 负责执行 release(handle)。
项目侧 Loading Rule / Scene Lifecycle 决定什么时候 release。
```

## SceneLifecyclePlan Runtime Boundary

SceneLifecyclePlan 是 Project Rule / State Rule 提交给 Runtime 的结构化生命周期请求。  
它不是新的规则系统，也不是 Runtime 脚本语言。

Runtime 执行边界：

```text
Project Rule / State Rule
  -> submit SceneLifecyclePlan
  -> Runtime 检查 scene / assetSet / fallback
  -> AssetRuntime preload
  -> SceneInstantiator load_scene
  -> Scene activation
  -> Scene unload
  -> AssetRuntime release
  -> RuntimeTrace / Diagnostics
```

Runtime 必须保证：

```text
Load 和 Activate 分开执行。
Unload 和 Release 分开执行。
Scene load 成功不代表资源已经全部 ready。
Scene activate 必须经过 Runtime 状态检查。
Scene unload 后 runtimeEntityId 失效。
release 只通过 AssetRuntime 执行。
```

第一版 SceneLifecyclePlan 只支持：

```text
preload
load_scene
activate
unload
release
fallback
diagnostics
```

第一版不支持：

```text
任意循环
任意脚本回调
逐 Entity 操作
底层 IO / decode / GPU upload 调用
绕过 Validation 创建 Persistent Entity
```

Validation 边界：

```text
不建立 SceneLifecyclePlan 专用 Validation DSL。
Runtime / Validation 只做结构、引用和状态检查。
复杂触发条件、项目逻辑和分支仍属于 Project Rule / State Rule。
```

## Runtime Request / Command Domain Boundary

Runtime 内部请求按领域保留独立结构，不合并成一个通用 RuntimeCommand。

正式规则：

```text
Scene 生命周期使用 SceneLifecyclePlan。
运行时生成对象使用 RuntimeSpawnRequest。
资源加载 / 释放使用 AssetLoadRequest / AssetReleaseRequest。
渲染同步使用 RenderCommand。
编辑器操作使用 EditorCommand / UiCommand。
```

边界：

```text
不建立全局万能 RuntimeCommand payload。
不要求所有领域请求共享同一套字段。
各领域 Request / Command 可以共享 trace_id / source / diagnostics 基础元数据。
领域语义保留在各自结构里。
```

原因：

```text
Scene / Asset / Spawn / Render 的生命周期、执行线程、失败处理和调试证据不同。
统一成一个大 RuntimeCommand 会让 payload 变成弱类型杂物箱。
分领域结构更接近 Unity / UE 的模块边界，也更方便 AI 定位问题。
```

## RuntimeSpawnRequest Boundary

RuntimeSpawnRequest 是 Runtime 生成对象的内部领域请求。

Runtime 执行边界：

```text
Project Rule / State Rule
  -> submit RuntimeSpawnRequest
  -> Runtime 检查 prefab / owner / parent / transform
  -> Runtime Spawn System
  -> ECS Entity / Component / Hierarchy
  -> EntitySpawned event
  -> RuntimeTrace / Diagnostics
```

Runtime 必须保证：

```text
项目逻辑和 AI 不能直接写 ECS spawn 细节。
RuntimeSpawnRequest 默认 deferred，在安全 apply point 执行。
指定 parent 时继承 parent 的 EntityOwner。
未指定 owner 时归属当前 active SceneInstance。
Persistent 必须显式声明并通过 Validation。
componentOverrides 只能覆盖 prefab 暴露字段。
```

第一版不允许：

```text
任意脚本回调。
复杂构造流程。
绕过 Prefab 直接拼任意 Component 图。
跨线程立即写 ECS。
AI 默认创建 Persistent。
```

RuntimeSpawnRequest 不负责：

```text
资源加载。
Scene 生命周期切换。
渲染命令生成。
对象销毁。
```

## RuntimeDespawnRequest Boundary

RuntimeDespawnRequest 是 Runtime 销毁 Entity 的内部领域请求。  
这里只定义引擎层销毁机制，不定义项目侧何时触发销毁。

Runtime 执行边界：

```text
RuntimeDespawnRequest
  -> Despawn Queue
  -> Runtime Despawn System
  -> ECS Entity / Component / Hierarchy removal
  -> EntityDespawning / EntityDespawned event
  -> RuntimeTrace / Diagnostics
```

Runtime 必须保证：

```text
Runtime Despawn System 是唯一删除 ECS Entity 的入口。
默认 deferred despawn，在安全 apply point 执行。
重复 despawn 不触发重复事件。
不存在的 Entity 返回 not_found diagnostics，不让 Runtime 崩溃。
Scene unload 批量销毁也走 Runtime Despawn System。
Persistent Entity 只有 explicit_destroy / runtime_shutdown 可以销毁。
```

RuntimeDespawnRequest 不负责：

```text
项目侧销毁条件。
资源 release。
RenderCommand 生成。
Prefab / Scene 实例化。
```

资源边界：

```text
Despawn 只销毁 Entity / Component / Hierarchy 和 Runtime 引用映射。
Asset release 仍由 AssetRuntime / SceneLifecyclePlan 执行。
Render / Physics / Audio 清理通过 EntityDespawned / Dirty / Extract 流程响应。
```

## 测试

新增命令：

```powershell
npm.cmd run test:runtime
```

覆盖：

```text
backend can load starter project
backend can load shooter project
backend tick returns scene state
backend extractRenderSnapshot returns renderables
backend runtime trace contains system entries
backend runtime trace contains IR rule entries
backend validation reports schemas / systems / trace
backend fails on invalid scene
shooter tick can spawn projectile
```

## 当前边界

已经完成：

```text
RuntimeBackend v1 接口。
TypeScriptRuntimeBackend v1。
RuntimeService 最小入口。
Validation runtime smoke 走 backend。
Editor Viewport / Trace 不再直接调用 tickScene / createRuntimeState。
RuntimeTraceReport 可以承载 IR rule trace。
Runtime Trace UI 可以展示 IR rule trace。
```

暂未完成：

```text
Rust Runtime MVP。
Rust Runtime Bridge。
Golden Scenario Test。
TypeScript Runtime 退役。
Sidecar / Native addon 桥接。
Runtime Trace Replay。
IR rule 自动接入每帧 gameplay system。
Runtime Trace UI 的 source map 跳转。
```

这些属于后续阶段，不放入当前最小闭环。

