# 224-Editor Play Apply Runtime Change To Authoring v1 方案

## 1. 这个系统是干什么的

本系统解决 Play Mode 调参结果“显式保留到编辑态”的问题。

223 已完成：

```text
Play Mode:
  Runtime Hierarchy 可选择 active runtime entity
  Runtime Inspector 可做 TemporaryPlaySession 临时编辑
  StepFrame 后临时修改仍在当前 Play Session 生效
  StopPlaySession 后临时修改丢弃
```

224 要补上用户最自然的下一步：

```text
我在 Play Mode 调好了这个对象的值
  -> 预览这次 runtime 改动能不能写回
  -> 用户确认
  -> 写回 authoring Scene 或 Prefab instance override
  -> 标记 dirty / 进入 undo / 输出 report
```

本系统对标：

```text
Unity:
  Play Mode 修改默认不保存；常见工作流是复制组件值，退出 Play 后粘贴回编辑态对象。

Unreal:
  Simulate / PIE 中默认不把运行时改动直接保存；提供 Keep Simulation Changes，把模拟世界 Actor 的可编辑属性复制回编辑世界 Actor。
```

本引擎主线中的作用：

```text
把 223 的 Play Mode 临时调参升级为可审查、可确认、可回滚的 authoring patch。
让复杂打飞机项目能在真实 Play 中调 Player / Enemy / Bullet prefab instance 参数，再把有来源的改动保存回项目数据。
```

## 2. 其它引擎源码参考

### Unity

本地源码参考：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/ComponentUtility.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/ComponentUtility.bindings.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/EditorUtility.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/EditorUtility.bindings.cs
```

关键点：

```text
ComponentUtility.CopyComponent / PasteComponentValues:
  用显式 copy / paste 动作把运行时或当前对象的组件值转移到目标对象。

EditorUtility.CopySerializedIfDifferent:
  只复制有差异的序列化数据，避免无意义写入。

Inspector 常规修改:
  SerializedObject / SerializedProperty / ApplyModifiedProperties。
```

可学习：

```text
默认不保留 Play Mode 改动。
保留改动必须是用户显式动作。
复制的是可序列化、可编辑字段，不是运行时内部缓存。
```

不可照搬：

```text
不做黑盒 Component pasteboard。
不把运行时组件对象直接序列化到编辑态。
本项目必须输出结构化 candidate/report，让 AI 和测试知道写回了哪些字段、为什么有些字段不能写回。
```

### Unreal Engine

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/LevelEditor/Private/LevelEditorActions.cpp
  FLevelEditorActionCallbacks::OnKeepSimulationChanges
  FLevelEditorActionCallbacks::CanExecuteKeepSimulationChanges

<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/UnrealEd/Private/Editor.cpp
  EditorUtilities::CopyActorProperties

<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/UnrealEd/Public/Editor.h
  EditorUtilities::FCopyOptions
```

关键点：

```text
Keep Simulation Changes 是显式命令。
它从 SimWorldActor 找到对应 EditorWorldActor，再复制允许编辑的属性。
复制过程有 CopyOptions，例如 OnlyCopyEditOrInterpProperties / SkipInstanceOnlyProperties。
复制后给用户 notification，说明更新了多少 Actor / property。
```

可学习：

```text
运行世界和编辑世界分开。
Apply 必须先找到 runtime object 到 authoring object 的对应关系。
只复制 authoring-safe 字段。
输出数量、结果和失败原因。
```

不可照搬：

```text
不做全 Actor 属性复制。
不做 UE 级 CDO / archetype / Blueprint reinstancing。
不把 runtime-spawned entity 自动写入 Scene。
```

## 3. 本项目当前基线

已完成前置：

```text
217 Editor Play 使用 Preview RuntimePackage。
218 EditorRuntimePlayInstance 创建 active runtime World。
221 Pause / Resume / StepFrame / StopPlaySession。
222 Runtime selection source 消歧与 runtime inspector。
223 Runtime Hierarchy + TemporaryPlaySession Inspector edit。
203 Prefab Authoring Productization v1，已有 Prefab instance override / apply override 基础。
```

当前可用代码基线：

```text
EditorRuntimePlayInstance::runtime_world()
EditorRuntimePlayInstance::temporary_edit_summary()
EditorRuntimePlayInstance::apply_temporary_component_edit(...)
RuntimeTemporaryEditSummary
UiCommandPayload::SetRuntimeComponentFieldTemporary
SceneEditCommand::SetTransform
SceneEditCommand::SetComponentField
PrefabOverride / PrefabWorkflowService::apply_override_to_asset / instance override 基础
```

当前缺口：

```text
Temporary edit record 只保留 summary，不足以稳定生成 Apply candidate。
Runtime entity 需要能追踪 authoring origin。
缺少 ApplyRuntimeChangeToAuthoring command。
缺少 Preview / Candidate / Report。
缺少对 Scene direct write 与 Prefab instance override 的路由。
缺少“不允许写回”的清晰诊断。
```

## 4. 方案选择

### 方案 A：Scene Entity Apply only

只把 runtime entity 的 Transform / component 字段写回同 id 的 authoring Scene entity。

优点：

```text
最简单。
最快形成最小闭环。
```

缺点：

```text
无法处理 Prefab instance。
复杂打飞机中很多对象来自 prefab，实用性不足。
容易让用户误以为 runtime-spawned entity 也能写回。
```

### 方案 B-min：Origin-Mapped Apply Patch（采用）

新增显式 Apply 流程：

```text
Runtime temporary edit
  -> Build Apply Candidate
  -> User / AI Review
  -> Confirm Apply
  -> Scene transaction 或 Prefab instance override
  -> Dirty / Undo / Report
```

只允许有明确 authoring origin 的 runtime entity 写回：

```text
origin_kind=scene_entity:
  写回 authoring Scene entity。

origin_kind=prefab_instance:
  写成 Scene 中该 prefab instance 的 override。

origin_kind=runtime_spawned / unknown:
  不允许 apply，只输出 diagnostic 和 next_action。
```

优点：

```text
AI 适配性最好：candidate/report/diagnostic 都是结构化数据。
复杂项目可维护：不会把运行时生成物写回项目。
符合 Unity/UE 的显式保留心智。
可在 C-min 内落地，不需要一次做完整 diff engine。
```

缺点：

```text
需要 RuntimePackage hydration 阶段保留 authoring origin。
需要 temporary edit 记录从 summary 升级为轻量 record list。
第一版不能处理所有运行时变化，只处理用户临时编辑过的字段。
```

### 方案 C：Full Keep Simulation Changes

复制 runtime entity 当前可编辑状态到 authoring object，尽量接近 UE 的 Keep Simulation Changes。

优点：

```text
功能强。
用户看起来像“一键保存当前运行状态”。
```

缺点：

```text
风险过高。
容易写回运行时缓存、派生状态、生成物、临时状态。
需要完整 diff / property metadata / origin mapping / prefab propagation 策略。
不适合当前复杂打飞机 C-min。
```

## 5. 正式采用：方案 B-min

正式名称：

```text
Editor Play Apply Runtime Change To Authoring v1 = Origin-Mapped Apply Patch B-min
```

用户心智：

```text
Play Mode 里改值默认还是临时的。
只有点击 Apply Runtime Change To Authoring，才会尝试把选中的临时改动写回编辑态。
Apply 前必须能预览 candidate。
Apply 后进入普通 authoring transaction / dirty / undo。
```

内部链路：

```text
RuntimePackage / Hydration report
  -> EditorRuntimePlayInstance 保留 RuntimeSceneHydrationReport
  -> EditorRuntimePlayInstance 建立 runtime_entity_id -> RuntimeAuthoringOrigin 反向索引
  -> Play Mode temporary edit record
  -> ApplyRuntimeChangePreview
  -> ApplyRuntimeChangeCandidate
  -> ApplyRuntimeChangeToAuthoring
  -> SceneEditCommand 或 PrefabOverride
  -> ApplyRuntimeChangeReport
```

## 6. 数据模型

新增或扩展：

```text
RuntimeAuthoringOrigin:
  origin_kind: scene_entity | prefab_instance | runtime_spawned | unknown
  scene_id
  scene_entity_id
  prefab_asset_guid?
  prefab_instance_id?
  prefab_source_entity_id?

RuntimeTemporaryEditRecord:
  扩展现有 editor_gameview_play.rs 中的 RuntimeTemporaryEditRecord，不新建同名结构
  edit_id
  entity_id
  component_type
  field_path
  value_after
  before_summary
  after_summary
  authoring_origin
  apply_policy
  timestamp_or_sequence
  applied

ApplyRuntimeChangeCandidate:
  candidate_id
  runtime_entity_id
  authoring_origin
  component_type
  field_path
  runtime_value
  target_authoring_path
  apply_route: scene_transaction | prefab_instance_override | rejected
  status: ready | blocked | warning
  diagnostics

ApplyRuntimeChangeReport:
  status
  preview_candidate_count
  applied_candidate_count
  rejected_candidate_count
  dirty_domains
  undo_transaction_id?
  diagnostics
  next_actions
```

说明：

```text
RuntimeAuthoringOrigin 是 apply 必需的映射，不是用户可编辑玩法数据。
RuntimeTemporaryEditRecord 只记录用户通过 TemporaryPlaySession 改过的字段，不记录完整 World dump。
224 必须保留 223 现有 RuntimeTemporaryEditRecord 的 entity_id / before_summary / after_summary 命名，新增结构化 value_after 和 authoring_origin，不能把它当新结构重定义。
EditorRuntimePlayInstance 保留 record list，并按 (entity_id, component_type, field_path) 去重保留最新 value_after；edit_id / timestamp_or_sequence 只用于 Trace。
ApplyRuntimeChangeCandidate 是 AI 和用户审查的核心真相。
```

Origin 追踪采用 editor 侧反向索引，不改 `World` / `EntityMeta`：

```text
EditorRuntimePlayInstance::start:
  hydrate_active_scene_into_world(package)
    -> (World, RuntimeSceneHydrationReport)
  保留 RuntimeSceneHydrationReport.instance
  遍历 RuntimeSceneInstance.source_to_runtime_entity
    -> runtime_entity_id -> RuntimeAuthoringOrigin(scene_entity)
  如后续 runtime prefab instance 有 authoring instance 映射:
    -> runtime_entity_id -> RuntimeAuthoringOrigin(prefab_instance)
  不在任何 source map 中的 entity:
    -> runtime_spawned / unknown，Apply candidate blocked
```

Prefab instance B-min 规则：

```text
如果 runtime entity 可映射到 Scene 中的 PrefabInstance root:
  Transform 仍可写回 Scene entity。
  Prefab source entity 字段写为 Scene 内 instance override。
  C-min 不直接 apply 到 Prefab Asset。
```

## 7. C-min 写回范围

允许：

```text
Transform:
  local_position / local_rotation / local_scale 及其子字段

Renderable:
  visible

SpriteRenderer2D:
  visible
  color
  sorting_layer
  order_in_layer
  sort_z

Collider2D:
  enabled
```

写回目标：

```text
Scene entity:
  走 SceneEditCommand::SetTransform / SetComponentField。

Prefab instance:
  写为 Scene 内 prefab instance override。
  C-min 不直接 apply 到 Prefab Asset。
```

Transform 子字段写回规则：

```text
Runtime edit 可能是 local_position.x / local_rotation.z 这种子字段。
SceneEditCommand::SetTransform 只接收 whole Vec3。
Preview / Apply 必须 live 读取 runtime 当前结构化值，并读取 authoring 当前 Transform，做 read-merge-write：
  local_position.x -> 合并成完整 local_position Vec3 后 SetTransform
  local_rotation.z -> 合并成完整 local_rotation Vec3 后 SetTransform
  local_scale.y -> 合并成完整 local_scale Vec3 后 SetTransform
不能把 Debug summary 当作写回值。
```

`SpriteRenderer2D.color` 说明：

```text
223 已通过 InspectorValue::Json + property_editing color path + json_color 支持 Play Mode 临时编辑 color。
224 apply color 时走 SceneEditCommand::SetComponentField 或 PrefabOverride 的 serde_json::Value，不要求新增 InspectorValue::Color。
```

不允许：

```text
runtime-spawned entity 写回。
新增 / 删除 entity。
新增 / 删除 component。
reparent / sibling_order。
asset ref / material ref / texture ref。
AUI runtime transient state。
ProjectUiStateSnapshot runtime 值。
Rule runtime cache。
完整 component JSON paste。
```

## 8. 诊断规则

必须输出清晰诊断：

```text
runtime_authoring_origin_missing
runtime_entity_spawned_not_applyable
target_scene_entity_missing
target_prefab_instance_missing
target_prefab_source_entity_missing
field_not_applyable_to_authoring
value_type_not_supported
requires_prefab_asset_apply_deferred
requires_component_schema_deferred
preview_required_before_apply
candidate_hash_mismatch
```

关键原则：

```text
Apply 失败不能部分静默成功。
每个 candidate 都必须有 ready / blocked / warning 状态。
Confirm Apply 必须校验 candidate_hash，避免 AI 或 UI 用旧 preview 写错对象。
candidate_hash 输入必须至少包含 runtime_entity_id / component_type / field_path / runtime_value / target_authoring_path。
Confirm Apply 时必须重新 live 读取 runtime 当前值并重新计算 hash；不一致则返回 candidate_hash_mismatch。
Apply 前必须校验 EditorRuntimePlayInstance.scene_id == 当前 editor_scene_document.scene_id；不一致则返回 scene_id_mismatch，不能写错 Scene。
```

## 9. 与 223 的关系

223 的规则仍成立：

```text
Play Mode temporary edit 默认不保存。
StopPlaySession 后临时修改丢弃。
不写 RuntimePackage。
不写 Rule / AUI / ProjectUiStateSnapshot。
```

224 只是新增一条显式写回命令：

```text
ApplyRuntimeChangeToAuthoring
```

它不改变 223 的默认 discard 行为。没有执行 Apply 时，Stop 后依旧丢弃。

Apply 后的 discard 计数：

```text
Apply 成功的 RuntimeTemporaryEditRecord 必须标记 applied=true 或移出 pending list。
StopPlaySession 的 runtime_temporary_edits_discarded 只统计未 apply 的 pending records。
已 apply 的记录进入 ApplyRuntimeChangeReport，不再被报告为 discarded。
```

## 10. AI / Report / 测试语义

AI 必须能回答：

```text
当前有哪些 runtime temporary edits 可以 apply？
每个 edit 对应哪个 authoring object？
哪些不能 apply，为什么？
Apply 后修改了 Scene 还是 Prefab instance override？
是否进入 dirty / undo？
有没有写 RuntimePackage？
```

Report 分档：

```text
Runtime hot path:
  不常驻输出完整 report。

Editor Summary:
  candidate count / applied count / rejected count / dirty domains。

Editor Trace:
  测试或用户显式诊断时输出每个 candidate 的 target path 和 diagnostic。
```

不新增：

```text
FullWorldStateDump。
每帧 diff report。
RuntimeStateSnapshot。
跨系统 Logic Ownership Router。
```

## 11. 复杂打飞机验收目标

最小用户体验：

```text
打开复杂打飞机项目。
Play。
Runtime Hierarchy 选择 Player 或可追溯的 prefab instance entity。
Inspector 临时修改 Transform.local_position.x。
点击 Apply Runtime Change To Authoring。
看到 preview candidate:
  runtime entity -> scene entity 或 prefab instance override
  field_path
  runtime_value
  target_authoring_path
确认 Apply。
Stop Play。
Edit Mode 的 Scene / Prefab instance override 保留该值。
Undo 可撤回。
Build / Preview RuntimePackage 重新生成后使用新 authoring 值。
```

自动化 gate：

```text
headless deterministic，不依赖真实 OS window。
构造 active EditorRuntimePlayInstance。
对有 authoring_origin 的 entity 做 temporary edit。
生成 ApplyRuntimeChangeCandidate。
断言 candidate status=ready，target_authoring_path 正确。
确认 Apply。
断言 Scene document dirty=true。
断言 SceneEditCommand / PrefabOverride 已写入。
StopPlaySession 后 runtime instance 清空，但 authoring 修改仍存在。
Undo 后 authoring 修改撤回。
runtime-spawned entity candidate status=blocked。
一次 ApplyRuntimeChangeToAuthoring 是一个原子 authoring undo transaction；失败时不能留下半应用状态。
```

## 12. Deferred 边界

不进入 224 B-min：

```text
Apply 到 Prefab Asset。
Apply runtime-spawned entity as new Scene entity。
多选批量 Apply。
完整 component paste / full object diff。
runtime hierarchy 搜索过滤。
runtime undo stack。
Vec2 Inspector 全链路。
AUI transient state apply。
Rule runtime value apply。
跨进程 player inspect。
多 RuntimeInstance apply。
冲突合并 UI。
```

## 13. 推荐施工 Gate

```text
Gate A: 现状锁定
  确认 223 已完成。
  锁定 RuntimeTemporaryEditSummary / RuntimeTemporaryEditRecord / SceneEditCommand / PrefabOverride 基线。

Gate B: Authoring origin metadata
  EditorRuntimePlayInstance 保留 RuntimeSceneHydrationReport。
  不修改 World / EntityMeta。
  建立 runtime_entity_id -> RuntimeAuthoringOrigin 反向索引。
  Scene-origin entity 可回到 authoring Scene entity。
  Prefab-origin entity 可回到 prefab instance / source entity。
  runtime-spawned / unknown 明确 blocked。

Gate C: Temporary edit record list
  扩展现有 RuntimeTemporaryEditRecord，不新建同名结构。
  223 summary 升级为 summary + record list。
  record 保存 component_type / field_path / latest runtime_value / origin / applied。
  按 (entity_id, component_type, field_path) 去重保留最新 value_after。
  不记录完整 World dump。

Gate D: Preview candidates
  新增 PreviewApplyRuntimeChangeToAuthoring command。
  生成 ApplyRuntimeChangeCandidate / candidate_hash。
  candidate runtime_value 必须 live 读取 runtime World 结构化值。
  ready / blocked / warning 可被 AI 和 UI 读取。

Gate E: Confirm apply
  新增 ApplyRuntimeChangeToAuthoring command。
  校验 candidate_hash，Confirm 时重新 live 读取 runtime 值。
  校验 Play scene_id 与当前打开 Scene 一致。
  scene_entity 走 SceneEditCommand。
  prefab_instance 走 instance override。
  一次 Apply 是一个原子 authoring transaction。
  写 dirty / undo / diagnostics / report。
  成功 apply 的 record 不再进入 Stop discard 计数。

Gate F: e2e / 回归 / 文档
  project_e2e_gate 覆盖复杂打飞机 Play Mode apply flow。
  cargo fmt --check
  cargo test -p editor_ui_model
  cargo test -p editor_core
  cargo test -p editor_core runtime_service_tests -- --nocapture
  cargo test -p editor_core prefab -- --nocapture
  cargo test -p project_e2e_gate -- --nocapture
```

## 14. 外部审查吸收

已读取并吸收：

```text
其它AI审查目录/42-224-Editor-Play-Apply-Runtime-Change-To-Authoring方案审查.md
```

必须修改项已吸收：

```text
MF-1:
  RuntimeTemporaryEditRecord 已存在，方案已改为扩展现有结构，不新建同名结构。
  字段命名保留 entity_id / before_summary / after_summary，并新增 value_after / authoring_origin / apply_policy / applied。

MF-2:
  Origin 追踪采用 EditorRuntimePlayInstance 保留 RuntimeSceneHydrationReport 并建立 runtime_entity_id -> RuntimeAuthoringOrigin 反向索引。
  不修改 World / EntityMeta。
```

施工约束已吸收：

```text
SC-1:
  Transform 子字段写回必须 read-merge-write，不能使用 Debug summary。

SC-2:
  Runtime temporary edit 保留 record list，并按 entity/component/field 去重留最新值。

SC-3:
  Prefab instance route 只做 Scene 内 instance override，不 apply 到 Prefab Asset。

SC-4:
  Apply 前校验 runtime scene_id 与打开的 authoring scene_id。

SC-5:
  Apply 成功记录不进入 Stop discard 计数。

SC-6:
  一次 Apply 是一个原子 authoring transaction。

SC-7:
  candidate_hash 输入和 Confirm live re-read 校验已写入。

SC-8:
  Gate F 补 editor_ui_model 与全量 editor_core。
```

已由历史施工吸收：

```text
审查中提到的 223 闭环风险已由当前阶段完成记录吸收：
阶段完成记录/2026-07-08-Editor-Runtime-Hierarchy-PlayMode-Temporary-Inspector-Edit-Productization-v1/00-总览.md
```

## 15. 最终结论

224 采用 `Origin-Mapped Apply Patch B-min`。

它不是“Play Mode 自动保存”，也不是“把 runtime World 整体复制回项目”。它只做一件事：

```text
把用户在 Play Mode 里通过 TemporaryPlaySession 明确改过、且能追溯 authoring origin 的字段，显式预览并确认写回 authoring 数据。
```

这个方案比 Unity 的手动复制粘贴更适合 AI 审查，比 UE 的完整 Keep Simulation Changes 更适合当前 C-min 风险控制。它保留 223 的默认丢弃语义，同时为复杂打飞机项目提供真正可用的 Play 调参闭环。
