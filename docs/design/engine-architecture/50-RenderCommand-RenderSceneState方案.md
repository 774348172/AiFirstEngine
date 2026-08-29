# RenderCommand / RenderSceneState 方案

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

本文定义本项目 Game 世界到 Render 世界的同步方案。

设计来源：

```text
UE 源码参考：框架设计/UE源码参考/RenderCommand-RenderSceneState.md
Unity 公开参考：PlayerLoop / Scriptable Render Pipeline / ScriptableRenderContext 官方文档
本项目目标：AI 友好、复杂项目可维护、高效率、规则不过度复杂
```

## 结论

本项目采用：

```text
UE-like 增量 RenderCommand + Render-side SceneState
```

不采用：

```text
完整场景 RenderSnapshot 双缓冲
```

正式链路：

```text
Game / ECS / Scene
  -> RenderDirtyTracker
  -> RenderExtract
  -> RenderCommandQueue
  -> Render Thread
  -> RenderSceneState / RenderProxy
  -> Renderer Feature Builder
  -> RDG
  -> RHI
```

AI 看到的链路：

```text
RenderIntent / Visual Patch / Material Graph / Preset
  -> Validation
  -> RenderExtract / Renderer Feature Builder
  -> RenderFrameReport
```

AI 不直接写 RenderCommand。

## AI / 用户读取层级

RenderCommand 是底层同步协议，不是普通用户和 AI 的默认主视图。

正式读取层级：

```text
普通用户：
  只看 RenderFrameReport 的自然语言解释。
  例如“第 120 帧，Player 的位置从 (0,0,0) 变成 (0,1,0)，由 MoveSystem 触发。”

AI 默认：
  读取 RenderFrameReport + RuntimeTrace 摘要。
  用于回答“为什么没显示 / 没移动 / 没换材质 / 为什么降级”。

AI 深度排错 / 引擎开发者：
  才展开单条 RenderCommand。
  用于确认 RenderExtract 是否生成正确命令、Render Thread 是否消费、RenderSceneState 是否更新。
```

原因：

```text
RenderCommand 太底层。
如果 AI 每次都直接面对 RenderCommandQueue，会把简单视觉问题变成底层协议排查。
如果只保留摘要，又会在复杂 bug 时缺少证据。
所以采用默认摘要、按需下钻的分层模型。
```

正式规则：

```text
RenderFrameReport 是 AI / 用户默认读取入口。
RuntimeTrace 提供跨系统时间线。
RenderCommand 只作为深度证据，不作为默认解释对象。
编辑器可以提供“展开底层命令”入口，但默认折叠。
release 构建可以裁剪 RenderCommand 的 source 字符串和 debug metadata。
```

## 为什么必须有 RenderCommand

RenderCommand 不是用户层玩法规则，也不是为了增加架构层级。

它的定位是：

```text
Game / ECS 世界到 Render 世界的增量变化提交协议。
```

如果没有 RenderCommand，只剩两种不合适的路线：

```text
Render Thread 直接读 ECS：
  多线程下会出现读写竞争。
  Render 和 Gameplay 状态互相穿透。
  后续高性能渲染、异步渲染和多 viewport 都会被卡住。

每帧复制完整 Scene / ECS Snapshot：
  大场景成本过高。
  只有少量物体变化时仍然按完整场景付费。
  这会把 RenderSnapshot 重新变成长期底层模型，和当前架构决策冲突。
```

RenderCommand 采用第三条路线：

```text
只提交本帧发生变化的渲染数据。
```

例子：

```text
一个 Entity 的 Transform 从 (0,0,0) 变成 (0,1,0)
  -> ECS 写 Transform
  -> RenderDirtyTracker 标记 Transform dirty
  -> RenderExtract 生成 UpdateTransform
  -> Render Thread 更新 RenderSceneState 中的 RenderProxy
```

这条链路的价值：

```text
线程安全：
  Render Thread 不直接读 ECS。

性能：
  按变化量提交，不复制完整场景。

可维护：
  Game 状态和 Render 状态边界清楚。

AI 友好：
  RenderFrameReport / Trace 可以解释某条画面变化来自哪个 Entity / Component / System / Patch。
```

正式规则：

```text
RenderCommand 必须存在。
RenderCommand 是引擎内部协议，不是项目层 API。
项目逻辑不能直接生成 RenderCommand。
AI 不能直接生成 RenderCommand。
AI 默认读取 RenderFrameReport / Trace，需要深查时再展开单条 RenderCommand。
```

## UE 参考方式

UE 的关键机制：

```text
MarkRenderStateDirty
MarkRenderTransformDirty
MarkRenderDynamicDataDirty
  -> UWorld EndOfFrame update queue
  -> DoDeferredRenderUpdates_Concurrent
  -> FScene Add / Remove / Update
  -> ENQUEUE_RENDER_COMMAND
  -> FScenePrimitiveUpdates
  -> FScene / FPrimitiveSceneInfo / FPrimitiveSceneProxy
```

UE 的优点：

```text
按变化量同步，性能好。
Render 侧长期状态稳定，适合缓存 draw command / GPUScene / culling。
EndOfFrame 合并 dirty，避免一帧内多次无效更新。
dirty 类型明确，State / Transform / DynamicData / InstanceData 分工清晰。
```

UE 的缺点：

```text
内部复杂度高。
调试链路长。
人类专家友好，AI 直接理解成本高。
历史兼容和 editor-only 分支多。
```

## Unity 参考方式

Unity 官方公开层面可确认：

```text
Unity 有固定 PlayerLoop / MonoBehaviour 生命周期。
LateUpdate 在 Update 之后，常用于相机跟随等渲染前收尾。
渲染阶段有 OnPreCull / OnPreRender / OnRenderObject / OnPostRender 等回调。
SRP 中 RenderPipeline.Render 是自定义渲染管线入口。
ScriptableRenderContext 是 C# 渲染管线和 Unity low-level graphics code 之间的接口。
SRP 使用 delayed execution：先构建渲染命令，再提交给底层图形系统。
```

官方参考：

```text
https://docs.unity3d.com/6000.5/Documentation/Manual/execution-order.html
https://docs.unity3d.com/6000.4/Documentation/ScriptReference/Rendering.RenderPipeline.Render.html
https://docs.unity3d.com/6000.4/Documentation/ScriptReference/Rendering.ScriptableRenderContext.html
https://docs.unity3d.com/2021.2/Documentation/Manual/srp-using-scriptable-render-context.html
https://docs.unity3d.com/Packages/com.unity.render-pipelines.core@17.1/manual/srp-creating-simple-render-loop.html
```

Unity 对我们的启发：

```text
用户心智要简单：Update / LateUpdate / Render。
渲染命令提交要延迟执行。
渲染管线入口要稳定。
不同 View / Camera 可以触发不同 render。
```

Unity 不足：

```text
底层 GameObject / Renderer 到 native render state 的同步细节不开源。
AI 无法天然追踪“用户意图 -> 组件变化 -> 渲染命令 -> 最终画面”的结构化证据。
Script Execution Order 对复杂项目容易成为隐式规则。
```

## 我们的标准结构

### RenderDirtyTracker

定位：

```text
记录 ECS 中哪些渲染相关数据发生变化。
不生成渲染命令。
不操作 RenderSceneState。
```

Dirty 类型：

```text
本文档早期 Dirty 类型清单已经被 51 文档收敛。
正式 Dirty 类型以 51 为准：
RenderState / Transform / DynamicData / InstanceData。
```

Dirty 记录格式：

```text
RenderDirtyRecord
  entityId
  componentType
  dirtyFlags
  frameIndex
  sourceSystem
  sourcePatchId
  reason
```

规则：

```text
ECS Component 写入时自动标记 dirty。
项目逻辑不手写 dirty record。
同一 entity 同一帧多次 dirty 合并。
dirty record 只保留本帧需要抽取的信息，不保存长期历史。
```

### RenderExtract

定位：

```text
Game World 到 Render World 的唯一桥。
读取 RenderDirtyTracker 和 ECS 当前值。
生成 RenderCommandQueue。
生成 RenderFrameReport。
```

规则：

```text
RenderExtract 默认只处理 dirty list。
允许在调试 / 场景加载 / 校验时执行 full rebuild extract。
RenderExtract 不创建 GPU resource。
RenderExtract 不执行 RDG。
RenderExtract 不改 Gameplay ECS。
```

### RenderExtract 合并、排序和确定性

RenderExtract 的正式原则：

```text
时间顺序是输入真相。
合并后的最终渲染状态是输出真相。
```

含义：

```text
ECS 每次写 Render-facing Component 时，记录 write_sequence。
DirtyTracker 按 write_sequence 记录变化发生顺序。
RenderExtract 不逐条输出所有变化。
RenderExtract 按 entity / component / dirty_type 合并。
同一帧同一字段多次写入，最终值生效。
生命周期命令保留必要先后顺序。
并行 RenderExtract 输出后，按稳定 sort_key 排序。
worker_count=1 和 worker_count>1 输出语义必须一致。
```

普通更新合并：

```text
t1: player.position = (0,0,0)
t2: player.position = (0,1,0)
t3: player.position = (0,2,0)

最终只生成：
  UpdateTransform(player, position=(0,2,0))
```

Report 规则：

```text
Summary 模式可以记录 skipped_redundant_updates = 2。
Evidence 模式才记录完整 from/to 链。
```

生命周期命令规则：

```text
Add 后又改材质：
  生成 1 条 AddProxy，材质最终值合进 Add payload。

Update 后 Remove：
  只生成 RemoveProxy，Update 被覆盖。

Add 后 Remove，且这个 proxy 还没进入 RenderSceneState：
  可以不生成 RenderCommand，只在 Report 记录 covered add/remove。

Remove 后 Add：
  如果是同一个 entity_generation 重新启用，可按语义生成 RemoveProxy -> AddProxy。
  如果 entity_generation 变了，必须生成 Remove old -> Add new。
```

稳定排序规则：

```text
RenderCommand sort_key 至少包含：
  frame_index
  lifecycle_order
  world_id
  scene_id
  entity_id
  entity_generation
  command_type_order
  component_type
  dirty_type
  write_sequence_last

禁止按 worker 完成顺序提交最终队列。
禁止让 HashMap / 并行遍历的非确定顺序影响最终 RenderCommandQueue。
```

命令类型顺序：

```text
RemoveProxy
AddProxy
UpdateRenderState
UpdateTransform
UpdateDynamicData
UpdateInstanceData
```

说明：

```text
Remove 优先用于覆盖无效 update。
Add 必须在 update 前建立 proxy。
RenderState 先于 Transform / DynamicData / InstanceData，因为它可能改变 proxy 结构。
Transform / DynamicData / InstanceData 之间可以按稳定顺序提交。
```

最终规则：

```text
RenderExtract 以时间顺序作为变化记录真相。
RenderCommandQueue 以合并后的最终渲染状态为输出真相。
生命周期命令保留必要时间顺序。
普通更新命令同帧合并，最后值生效。
并行提取后必须稳定排序。
```

### RenderCommand

标准结构：

```text
RenderCommand
  runtimeFields
  debugMetadata optional
```

Runtime 必要字段：

```text
command_id
frame_index
command_type
world_id
scene_id
entity_id
entity_generation
proxy_id optional
component_type
dirty_type
payload_kind
payload
resource_refs
sort_key
```

Debug metadata：

```text
source_component
source_field optional
source_system optional
source_rule optional
source_patch optional
reason_code
reason_string optional
validation_status optional
source_map optional
trace_id optional
```

分层规则：

```text
Runtime 必要字段属于热路径。
Render Thread 执行 RenderCommand 只能依赖 Runtime 必要字段。
Debug metadata 只服务 Editor / AI / Trace / Report。
Debug metadata 不能影响 RenderSceneState 更新结果。
Release 可以裁剪 Debug metadata。
Release 不能裁剪 Runtime 必要字段。
```

kind：

```text
本文档早期 kind 清单已经被 51 文档收敛。
正式 RenderCommand 类型以 51 为准：
AddProxy / RemoveProxy / UpdateRenderState / UpdateTransform / UpdateDynamicData / UpdateInstanceData。
```

target：

```text
entityId
renderProxyId，若已存在
sceneId
```

source：

```text
componentType
systemId
ruleId
aiPatchId
assetId
sourceMap
```

注意：

```text
source 属于 Debug metadata。
source 不属于 Render Thread 执行命令必须依赖的热路径字段。
正式字段分层以 Runtime 必要字段 / Debug metadata 为准。
```

规则：

```text
RenderCommand 是引擎生成物。
项目逻辑不能直接生成 RenderCommand。
AI 不能直接生成 RenderCommand。
同一 entity 同一 kind 的命令在同一帧默认合并，最后值生效。
Create / Destroy 不随便合并，必须保留生命周期顺序。
Render Thread 不允许依赖 debug metadata 才能正确更新 RenderSceneState。
```

### RenderCommand Payload Schema

RenderCommand 采用：

```text
外层固定 schema。
内层按 command_type / payload_kind 使用 typed payload。
```

原因：

```text
完全固定成超级 payload 会导致字段爆炸，大量 optional，后期难维护。
完全动态 JSON / Map payload 会导致运行时慢、验证困难、AI 解释不稳定。
UE 式 lambda / 函数命令性能强，但不利于 AI、Trace、序列化和验证。
外层固定 + typed payload 能同时满足运行时执行、验证层检查、AI 理解和 Report 摘要。
```

正式结构：

```text
RenderCommand
  runtime_fields
  payload_kind
  payload
  debug_metadata optional
```

第一版 payload 类型：

```text
AddProxyPayload
RemoveProxyPayload
UpdateRenderStatePayload
UpdateTransformPayload
UpdateDynamicDataPayload
UpdateInstanceDataPayload
```

示例：

```text
UpdateTransformPayload:
  local_to_world
  previous_local_to_world optional
  bounds

UpdateRenderStatePayload:
  proxy_kind
  mesh_ref optional
  material_slots optional
  shadow_flags optional
  render_layer optional
  feature_flags optional

UpdateDynamicDataPayload:
  dynamic_payload_kind
  material_params optional
  light_params optional
  camera_params optional
  visibility optional
  skinning_data optional

UpdateInstanceDataPayload:
  instance_count
  instance_buffer_ref
  changed_range
```

规则：

```text
RenderCommand 外层 schema 固定。
payload 按 command_type / payload_kind 使用 typed payload。
运行时禁止使用 JSON / Map 作为正式 payload。
Debug / Evidence 可以导出 JSON 视图，但那是报告格式，不是运行时格式。
每个 payload 必须有验证规则。
每个 payload 必须能被 RenderFrameReport 摘要化。
payload_kind 必须和 command_type 匹配。
Render Thread 根据 command_type / payload_kind 分派执行。
```

对 AI 的规则：

```text
AI 不直接生成 RenderCommand payload。
AI 生成 RenderIntent / Visual Patch / Material Graph / Preset。
引擎验证后由 RenderExtract / Renderer Feature Builder 生成 typed payload。
AI 在排错时可以查看 payload 的 Evidence JSON 视图。
```

### RenderCommandQueue

定位：

```text
本帧 Game 到 Render 的增量命令集合。
```

结构：

```text
RenderCommandQueue
  frameIndex
  commandsByEntity
  lifecycleCommands
  updateCommands
  reportHints
```

规则：

```text
生命周期命令优先：Destroy / Create。
Transform / Material / Mesh 等 update payload 可按 entity 聚合。
命令队列进入 Render Thread 后只读。
第一版即按跨线程命令队列设计。
测试环境 worker_count=1 时也必须走同一套队列和同步点。
```

### RenderProxy

定位：

```text
Entity 在 RenderSceneState 中的长期渲染代理。
```

标准字段：

```text
RenderProxy
  renderProxyId
  entityId
  proxyKind
  transformState
  meshHandle
  materialHandles
  lightState
  cameraState
  bounds
  visibility
  instanceData
  resourceBindings
  debugName
```

规则：

```text
RenderProxy 由 RenderSceneState 创建和销毁。
项目逻辑不能直接持有 RenderProxy 可变引用。
ECS 只保存 entity -> renderProxyId 的弱映射或由 RenderSceneState 维护映射。
RenderProxy 可以缓存渲染侧数据。
```

### RenderSceneState

定位：

```text
Render Thread 长期拥有的渲染世界。
```

包含：

```text
renderProxies
entityToProxyMap
visibleSetCache
lightSet
cameraSet
materialBindings
meshBindings
boundsTree / culling data，后续
gpuSceneData，后续
```

规则：

```text
RenderSceneState 不包含 gameplay state。
RenderSceneState 只通过 RenderCommand 更新。
Renderer Feature Builder 从 RenderSceneState 读取渲染输入。
RDG 不直接读取 ECS。
RHI 不知道 Entity / Project Logic。
```

### RenderFrameReport

定位：

```text
AI / 用户 / Trace 读取的本帧渲染摘要。
Runtime Debug / AI Evidence 系统的一部分。
旁路报告，不是渲染主流程必需数据。
```

标准结构：

```text
RenderFrameReport
  frameIndex
  reportLevel
  platform
  qualityProfile
  views
  counters
  changedEntities
  renderEvents
  traceRefs
```

规则：

```text
RenderFrameReport 是摘要，不是 RenderSceneState 副本。
AI 主要读取 RenderFrameReport，而不是底层 RenderCommandQueue。
出现渲染问题时，Report 必须能追溯到 source component / system / AI patch / asset。
Report 解释不了的问题，才允许 AI 下钻到 RenderCommand。
RenderFrameReport 不能参与游戏逻辑。
RenderFrameReport 不能影响渲染结果。
```

第一版字段结构：

```text
views:
  view_id
  view_kind，Scene / Game / Preview / Shadow / Reflection
  camera_id
  visible_count
  culled_count

counters:
  dirty_entity_count
  command_count
  fallback_count
  missing_resource_count
  warning_count
  error_count

changed_entities:
  entity_id
  component
  change_kind
  result，Applied / Skipped / Covered / Failed
  trace_id

render_events:
  severity
  event_code
  entity_id optional
  resource_id optional
  view_id optional
  render_feature optional
  reason_code
  fallback_code optional
  trace_id

trace_refs:
  trace_id
  source_system optional
  source_patch optional
```

第一版默认不记录：

```text
完整 from / to 值。
完整 RenderCommand payload。
完整 RenderSceneState。
完整 ECS World。
长字符串 reason。
完整 source map。
每个 entity 的完整可见性细节。
```

这些只允许在 Level 3 Evidence 模式按帧段开启。

RenderFrameReport v1 要能回答的用户问题：

```text
为什么没显示？
为什么没移动？
为什么材质没变？
为什么变黑？
为什么手机端效果降级？
为什么资源没加载？
为什么 Scene 里能看到，Game 里看不到？
```

最小解释规则：

```text
Summary 模式回答：
  发生了什么。
  谁受影响。
  结果是什么。
  去哪里深查。

Evidence 模式回答：
  字段具体从什么变到什么。
  底层 RenderCommand 是什么。
  资源 / Shader / Quality / View 的完整证据链是什么。
```

### RenderFrameReport 生效模式

RenderFrameReport 必须是 Runtime 能力，但不能在所有 Runtime 场景中全量常开。

正式分级：

```text
Level 0 Off：
  Release 默认。
  不生成完整 RenderFrameReport。
  只允许保留严重错误码、crash evidence、资源缺失摘要。

Level 1 Stats：
  Profile 默认。
  只记录 command_count、dirty_entity_count、fallback_count、resource_missing_count、cost。
  不记录每个 entity 的完整字段变化。

Level 2 Summary：
  Editor Play 默认。
  记录 changed_entities 摘要、commands_by_type、warnings、fallbacks、source ids。
  不记录大量字符串和完整 from/to 大对象。

Level 3 Evidence：
  AI 深度排错、用户点击录制、Golden Test 失败、指定帧段调试时开启。
  记录 source_system、source_patch、changed fields、reason、resource trace、必要 from/to。
```

生效规则：

```text
Editor Play 默认 Summary。
AI 排错时可以临时提升到 Evidence。
Golden Test / 自动化测试可以使用 Evidence，但应限制场景和帧段。
Profile 默认 Stats。
Release 默认 Off。
Release 只能在严重异常时记录一次性轻量摘要。
```

性能规则：

```text
RenderCommand / RenderSceneState 是主流程。
RenderFrameReport 是旁路报告。
Report 生成必须可关闭、可裁剪、可按帧段开启。
不能因为生成 Report 引入主流程锁竞争。
不能为了 Report 每帧复制完整 RenderSceneState 或 ECS World。
字符串 reason / source path / source map 默认只在 Summary / Evidence 中保留。
```

## 构建模式裁剪规则

RenderCommand Runtime 字段永远不裁剪。Debug metadata 和 RenderFrameReport 按构建模式裁剪。

| 构建模式 | RenderCommand Runtime 字段 | RenderCommand Debug metadata | RenderFrameReport | 目的 |
|---|---|---|---|---|
| Editor | 全保留 | 全保留 | 默认 Summary，可升 Evidence | AI 调试、用户解释 |
| Debug | 全保留 | 全保留 | 默认 Summary，可升 Evidence | 开发排错 |
| Profile | 全保留 | 只保留轻量 id | 默认 Stats | 测性能，不污染结果 |
| Release | 全保留 | 默认裁剪 | 默认 Off | 正式运行性能 |
| Crash / Error Evidence | 全保留 | 保留必要 id | 一次性轻量摘要 | 线上定位严重问题 |

Runtime 必须永远保留：

```text
command_id
frame_index
command_type
world_id
scene_id
entity_id
entity_generation
proxy_id optional
component_type
dirty_type
payload_kind
payload
resource_refs
sort_key
```

Release 默认裁剪：

```text
source_component
source_field
source_system
source_rule
source_patch
reason_string
source_map
完整 from/to
完整 RenderCommand payload dump
完整 RenderSceneState dump
完整 ECS dump
```

Release 可以在严重异常时保留轻量 id：

```text
reason_code
validation_status
trace_id optional
resource_id
fallback_code
error_code
```

Profile 保留：

```text
command_count
dirty_entity_count
fallback_count
missing_resource_count
cost
commands_by_type
reason_code
validation_status
```

Editor / Debug / Evidence 可以保留：

```text
source_system
source_patch
source_field
reason_string
source_map
必要 from/to
底层 RenderCommand 展开
资源证据链
```

裁剪规则：

```text
RenderCommand Runtime 字段永远不裁剪。
Debug metadata 按构建模式裁剪。
RenderFrameReport 默认按 Level 生效。
Release 默认 Off。
Release 只在严重异常时生成一次性轻量 Evidence。
Profile 只保留性能统计和轻量 id。
Editor / Debug 才保留 AI 完整证据链。
Evidence 必须支持按帧段开启，不能无限录制。
```

## 第一版实现边界

第一版可以简化：

```text
RenderSceneState 先只维护 proxy map。
RenderProxy 先支持 MeshRenderer / SpriteRenderer / Camera / Light。
RenderFrameReport 先记录命令摘要。
```

第一版不能简化掉：

```text
RenderDirtyTracker
RenderExtract
RenderCommand
跨线程 RenderCommandQueue
Runtime / Render 线程域隔离
RenderSceneState
RenderFrameReport
AI 不直接写 RenderCommand
Render 不直接读 ECS
```

## 对比

| 项目 | UE | Unity | Bevy | O3DE Atom | 本项目 |
|---|---|---|---|---|---|
| Game 到 Render | Dirty + EndOfFrame + RenderCommand + FScene | PlayerLoop + 内部 native 同步，公开层 SRP 命令提交 | MainWorld -> ExtractSchedule -> RenderWorld -> Queue | Scene + FeatureProcessor + RenderPipeline | DirtyTracker + RenderExtract + typed RenderCommandQueue + RenderSceneState |
| 同步成本 | 按变化量 | 内部不可见，SRP 命令延迟提交 | 按变化量 / RenderWorld 同步 | 按 Feature 数据变化 | 按变化量 |
| 用户心智 | Tick / Component / Render Proxy 较复杂 | Update / LateUpdate / Render 简单 | ECS / RenderWorld 偏工程化 | FeatureProcessor 偏专家 | Update / LateUpdate / Render 简单 |
| AI 友好 | 弱，需要理解大量 C++ 内部状态 | 中，公开 API 简单但 trace 不足 | 中，结构清晰但层级多 | 中，feature 清晰但内部专业 | 强，Report / source trace 一等公民 |
| 大项目能力 | 很强 | 强 | 强 | 强 | 目标强，第一版先保留正确边界 |
| 调试 | 强但复杂，依赖 Insights / 源码经验 | 工具成熟但内部黑箱 | ECS 可观察性强 | Feature 维度清楚 | 结构化 Report + Trace |
| 复杂度 | 高 | 中 | 中高 | 高 | 中，刻意压低用户可见规则 |

正式取舍：

```text
采用 UE-like typed RenderCommandQueue 作为主路线。
吸收 Bevy 的 Extract discipline，但第一版不完整照搬 RenderWorld ECS。
吸收 O3DE Atom 的 Render Feature boundary，但不让 FeatureProcessor 拥有上层可见同步协议。
学习 Unity 的用户心智简单，但不采用 Unity-like 黑箱同步。
```

## 最终规则

```text
1. 本项目采用 UE-like 增量渲染同步路线。
2. ECS Component 写入只标记 dirty，不直接改 RenderSceneState。
3. RenderExtract 是 Game 到 Render 的唯一桥。
4. RenderCommand 是引擎生成的增量命令。
5. RenderProxy / RenderSceneState 是 Render 侧长期状态。
6. 完整 RenderSnapshot 不作为长期底层同步模型。
7. AI 不直接写 RenderCommand，只写 RenderIntent / Visual Patch / Preset / Material Graph。
8. AI 和用户主要读取 RenderFrameReport。
9. 第一版即按 Game / Render 分离和跨线程队列设计。
10. worker_count=1 只是测试配置，不能绕过 RenderCommandQueue 或直接共享 RenderSceneState。
11. RenderCommand 字段分为 Runtime 必要字段和 Debug metadata。
12. Release 可以裁剪 Debug metadata，但不能裁剪 Runtime 必要字段。
13. RenderFrameReport / Debug metadata 按 Editor / Debug / Profile / Release / Crash 模式裁剪。
14. RenderExtract 以时间顺序作为输入真相，以合并后的最终状态作为输出真相。
15. 并行 RenderExtract 必须稳定排序，不能依赖 worker 完成顺序。
16. RenderCommand 外层 schema 固定，内层按 command_type / payload_kind 使用 typed payload。
17. 运行时禁止使用 JSON / Map 作为正式 RenderCommand payload。
18. RenderSceneState 只能由 RenderCommand 更新，项目逻辑和 AI 不能直接写 RenderSceneState / RenderProxy。
19. RenderCommandQueue 必须先 normalize / merge，再由 RenderSceneState.apply_batch 确定性应用。
20. AddProxy / RemoveProxy 管生命周期，UpdateTransform / UpdateRenderState / UpdateDynamicData / UpdateInstanceData 管增量状态。
21. 同帧同 proxy 的 UpdateTransform 只保留最终 transform，但 previous_transform 必须保留第一次更新前的值。
22. Update missing proxy / Remove 后继续 Update / Add 已存在且 payload kind 冲突必须进入 diagnostics / RenderFrameReport，不能静默吞掉。
23. RenderCommand 不是 RDG / RHI / GPU CommandBuffer；它只负责 Game 到 RenderSceneState 的状态同步。
24. RenderCommandQueue 主线采用 UE-like typed queue，内部执行 collect / stable_sort / normalize / merge / apply_batch。
25. RenderExtract worker 输出 ThreadLocalCommandBuffer，再由 RenderCommandQueue.collect 汇总。
26. RenderCommand sort_key 第一版由 frame_index / lifecycle_order / runtime_entity_id / command_type_order / command_id 组成。
27. 同一个 proxy 内生命周期命令必须保序，普通 Update 才允许 last value wins。
28. normalize / merge 第一版采用 UE-like ObjectCommandSlot：同一个 proxy/entity 的命令聚合到同一个 slot。
29. ObjectCommandSlot 保存 existed_at_frame_start、lifecycle、四类 update payload 和 diagnostics。
30. 对象帧开始不存在时，Add + Update 合并为 AddProxy，Update only 记录 missing_proxy，Add + Remove 输出 NoOp。
31. 对象帧开始存在时，Update 多次 last value wins，Update + Remove 输出 RemoveProxy，Remove + Add 输出 Recreate。
32. normalize / merge 是引擎内部规则，AI 默认只读 RenderFrameReport 摘要，不直接判断命令合并。
33. RenderCommandDiagnostic 第一版只记录命令级最小证据：frame、severity、code、stage、entity/proxy/command、result、reason_code、trace_id。
34. RenderFrameReport 第一版只保留 frame_index、report_level、counters、changed_entities、render_events、trace_refs。
35. 第一版 reason_code 只保留 missing_proxy / missing_resource / payload_kind_conflict / update_after_remove / add_existing_proxy / remove_missing_proxy / covered_by_remove / covered_by_noop / merged_last_value_wins / invalid_payload / apply_failed / fallback_used。
36. Release 默认不生成完整 RenderFrameReport，只保留 counters + 严重 error 摘要；Evidence 模式才展开 payload、from/to、source map、ObjectCommandSlot。
37. 多 View / 多 Camera / Editor Scene View 第一版采用 RenderSceneState + RenderViewState + RenderFrameViewData 三层边界。
38. RenderSceneState 保存全局 RenderProxy 和最小 view_registry；RenderViewState 保存视图配置；RenderFrameViewData 保存每帧可见性 / culling / render phase 摘要。
39. visible_proxy_ids / culling result / render phase 不作为长期 RenderSceneState 真相；Editor Scene View 不能直接修改 Game Camera 的 RenderViewState。
```
