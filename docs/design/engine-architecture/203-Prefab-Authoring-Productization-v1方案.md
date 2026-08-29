# 203-Prefab Authoring Productization v1 方案

## 1. 系统是什么

本系统正式命名为：

```text
Prefab Authoring Productization v1
```

一句话说明：

```text
它把已经存在的 Prefab 底座，产品化为用户和 AI 都能稳定创建、打开、实例化、覆盖、保存、验证和报告的真实编辑工作流。
```

它不是重新实现 Runtime Prefab。已有底座继续有效：

```text
140-M7-Prefab-Workflow-Reusable-Authoring-Object-System-v1方案.md
99-Runtime-Prefab-Spawn-Despawn-C-min方案.md
70-Scene-Prefab-Entity-Runtime实例化方案.md
141-M8-Schema-driven-Inspector-Details-System-C-full方案.md
191-Authoring-Walkthrough-Missing-Operations-Convergence-v1方案.md
202-AI-Project-Patch-Entry-Project-Patch-Productization-v1方案.md
```

本轮要解决的真实缺口：

```text
当前有 PrefabAsset / PrefabInstance / PrefabOverride / ResolvedPrefabView / RuntimePackage prefab cook。
但用户手动 walkthrough 仍把 Prefab 域报告为缺真实 authoring command。
AI 也还不能把 Prefab 当成可审阅、可保存、可测试的项目编辑对象。
```

对于复杂打飞机项目，它的作用是：

```text
Enemy / Bullet / Explosion / Pickup 等项目对象不应该靠复制 Scene Entity 维护。
它们应该成为 Prefab Asset，再由 Scene / Rule / RuntimePackage 复用。
```

本系统仍然禁止把以下玩法概念做成引擎内置 API：

```text
Player
Enemy
Bullet
Health
Damage
Score
Wave
Weapon
Boss
Drop
```

这些只允许出现在项目侧 Prefab / Rule / AUI / Data Asset 中。

## 2. 其它引擎对标与源码参考

### 2.1 Unity

对标：

```text
Prefab Asset
Prefab Instance
Prefab Overrides
Apply / Revert
SaveAsPrefabAsset / SaveAsPrefabAssetAndConnect
Prefab Stage / Prefab Mode
```

官方参考：

```text
https://docs.unity3d.com/Manual/Prefabs.html
https://docs.unity3d.com/6000.4/Documentation/ScriptReference/PrefabUtility.html
https://docs.unity3d.com/6000.5/Documentation/Manual/PrefabInstanceOverrides.html
```

本地源码命中：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Prefabs\PrefabUtility.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Prefabs\PrefabUtility.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Prefabs\PrefabOverrides\PrefabOverridesWindow.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Prefabs\PrefabOverrides\PrefabOverridesTreeView.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SceneManagement\StageManager\PrefabStage\PrefabStage.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SceneManagement\StageManager\PrefabStage\PrefabStageUtility.cs
```

关键源码点：

```text
PrefabUtility.cs:
  RevertPrefabInstance(...)
  ApplyPrefabInstance(...)
  SaveAsPrefabAsset(...)
  SaveAsPrefabAssetAndConnect(...)
  InstantiatePrefab(...)

PrefabUtility.bindings.cs:
  GetPropertyModifications(...)
  RevertPrefabInstance(...)
  ApplyPrefabInstance_Internal(...)
  SaveAsPrefabAsset_Internal(...)
  InstantiatePrefab_internal(...)

PrefabOverridesWindow.cs:
  PrefabUtility.ApplyPrefabInstances(...)
  PrefabUtility.RevertPrefabInstance(...)

PrefabStage.cs:
  PrefabStage.CreatePrefabStage(...)
  PrefabStageUtility.LoadPrefabIntoPreviewScene(...)
  PrefabUtility.SaveAsPrefabAsset(...)
```

可学习点：

```text
Prefab Asset 和 Scene Instance 必须分离。
Instance 修改必须能记录为 Override。
Apply / Revert 是对 Override 的显式操作，不应隐藏在普通字段编辑里。
Prefab 编辑需要能进入隔离或上下文视图，但第一版可以只做数据和命令层。
```

不可照搬点：

```text
不照搬完整 Prefab Mode / Variant / Nested Prefab。
不照搬 Unity 隐式 native 序列化和隐藏对象生命周期。
不让 AI 生成 Unity Editor 脚本式修改。
```

### 2.2 Unreal Engine

UE 没有 Unity 同名 Prefab。最接近的是：

```text
Blueprint Class / Actor Class Defaults
Placed Actor Instance
Actor Components
Construction Script
SpawnActor / FinishSpawning
```

官方参考：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/blueprint-overview
https://dev.epicgames.com/documentation/en-us/unreal-engine/construction-script
https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/Actor
https://dev.epicgames.com/documentation/en-us/unreal-engine/components
```

本地源码命中：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Actor.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\World.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\Kismet2\Kismet2.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\Kismet2\KismetReinstanceUtilities.cpp
```

关键源码点：

```text
Actor.cpp:
  AActor::PostSpawnInitialize(...)
  AActor::FinishSpawning(...)
  AActor::ExecuteConstruction(...)
  AActor::RerunConstructionScripts(...)

World.cpp:
  UWorld::SpawnActor(...)
  UpdateWorldComponents(...)

Kismet2.cpp:
  CreateBlueprint(...)
  SimpleConstructionScript->CreateNode(...)
  SimpleConstructionScript->AddNode(...)

KismetReinstanceUtilities.cpp:
  Actor->RerunConstructionScripts(...)
  NewActor->ExecuteConstruction(...)
```

可学习点：

```text
模板类 / 默认值 / 放置实例 / 运行时生成应该是统一生命周期。
组件组合是可复用对象的核心。
编辑器修改 Blueprint 后需要能重建或刷新已放置实例。
```

不可照搬点：

```text
不引入 UObject / UClass / CDO / BlueprintGeneratedClass 体系。
不把 Prefab 做成完整脚本类系统。
不让 Construction Script 变成任意项目逻辑入口。
```

### 2.3 Godot

对标：

```text
PackedScene
SceneState
PackedScene.instantiate()
Node scene_instance_state / inherited_state
```

官方参考：

```text
https://docs.godotengine.org/en/stable/classes/class_packedscene.html
https://docs.godotengine.org/en/stable/tutorials/scripting/nodes_and_scene_instances.html
https://docs.godotengine.org/en/stable/getting_started/step_by_step/instancing.html
```

本地源码命中：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\resources\packed_scene.cpp
<GODOT_SOURCE>\godot-master\godot-master\scene\main\node.cpp
```

关键源码点：

```text
packed_scene.cpp:
  SceneState::can_instantiate()
  SceneState::instantiate(...)
  SceneState::pack(...)
  PackedScene::instantiate(...)
  PackedScene::pack(...)
  PackedScene::get_state()

node.cpp:
  Node::set_scene_instance_state(...)
  Node::get_scene_instance_state()
  Node::set_scene_inherited_state(...)
  Node::duplicate(...)
```

可学习点：

```text
可复用对象可以保持很简单：场景资源 -> instantiate -> 节点树。
保存和实例化都以结构化 scene state 为真相。
编辑器状态和运行时实例状态需要区分。
```

不可照搬点：

```text
我们运行时是 ECS，不采用 Godot Node 运行模型。
不做 Godot 式完整 Scene 继承。
```

### 2.4 Bevy

对标：

```text
DynamicScene
SceneSpawner
InstanceId
EntityMap / MapEntities
DynamicSceneRoot / SceneRoot
```

官方参考：

```text
https://docs.rs/bevy/latest/bevy/scene/struct.SceneSpawner.html
https://docs.rs/bevy/latest/bevy/prelude/struct.DynamicScene.html
https://docs.rs/bevy/latest/bevy/scene/struct.DynamicSceneRoot.html
https://docs.rs/bevy/latest/bevy/scene/struct.Scene.html
```

本地源码命中：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_world_serialization\src\world_asset_spawner.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_world_serialization\src\dynamic_world.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_world_serialization\src\world_asset.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_scene\src\lib.rs
```

关键源码点：

```text
world_asset_spawner.rs:
  InstanceId
  spawn_dynamic(...)
  spawn_dynamic_as_child(...)
  despawn_instance(...)
  spawn_dynamic_sync(...)
  despawn_instance_sync(...)
  spawned_instances

dynamic_world.rs:
  DynamicWorld
  SceneEntityMapper::world_scope(...)
  EntityMap / MapEntities tests
```

可学习点：

```text
ECS Prefab / Scene 实例化必须有 instance id。
source entity 到 runtime entity 的映射必须显式存在。
despawn 应该按 instance 清理，而不是靠名字或裸 entity 猜。
EntityRef remap 是 Prefab 多实例正确性的核心。
```

不可照搬点：

```text
Bevy 没有 Unity-like 编辑器 Prefab 产品流。
我们不能只实现 runtime scene spawning，还要让用户和 AI 能 authoring / save / validate。
```

## 3. 本项目当前基线

### 3.1 已经完成的能力

M7 已完成底座：

```text
阶段完成记录/2026-07-02-M7-Prefab-Workflow-v1/00-总览.md
施工文档/已完成/140-当前可自动化施工文档-M7-Prefab-Workflow-v1.md
```

当前代码已有：

```text
rust/crates/editor_core/src/prefab_workflow.rs
  PrefabAsset
  PrefabEntity
  PrefabInstance
  PrefabOverride
  PrefabRef
  ResolvedPrefabView
  PrefabWorkflowReport
  PrefabWorkflowService
  validate_prefab_asset(...)
  detect_cyclic_prefab_references(...)

rust/crates/editor_core/src/asset_placement.rs
  AssetPlacementResolver 支持 asset_type=prefab。

rust/crates/editor_core/src/inspector_details.rs
  PrefabInstance target。
  PropertyTransactionRouter 可把 PrefabInstance 字段编辑路由为 PrefabOverride。

rust/crates/editor_core/src/project_runtime_package_assembler.rs
  collect_prefabs(...)
  Prefabs/*.json 进入 RuntimePackageBuildInput。

rust/crates/engine_runtime/src/runtime_package_builder.rs
  authoring-prefab-asset.v1 -> runtime-prefab.v1。

rust/crates/engine_runtime/src/runtime_instance_loader.rs
  RuntimePrefabInstance。
  instantiate_prefab_from_package(...)
  despawn_prefab_instance(...)

rust/crates/engine_runtime/src/gameplay_command.rs
  GameplayCommand::InstantiatePrefab
  GameplayCommand::DespawnPrefabInstance
```

样例项目已有 Prefab 文件：

```text
samples/complex_shooter_project/Prefabs/player_bullet.prefab.json
samples/complex_shooter_project/Prefabs/enemy_scout.prefab.json
samples/complex_shooter_project/Prefabs/explosion_effect.prefab.json
```

### 3.2 当前缺口

191 / 192 完成后，manual walkthrough 已经能暴露缺口：

```text
Prefab:
  create_prefab_from_selection
  open_prefab_document
  instantiate_prefab_in_scene
  apply_prefab_changes
  save_prefab_document
  validate_prefab_references
```

当前问题不是没有底层数据结构，而是：

```text
UiCommandPayload 没有 Prefab authoring commands。
WorkflowCommandResolver 对 Prefab 基本只能 FocusDomainPanel。
EditorSession 没有产品化的 create/open/save/validate prefab authoring 命令入口。
PrefabWorkflowReport 没有进入 complex shooter e2e artifact。
样例 Scene 里 Enemy A/B 仍是复制实体，不是 prefab instance。
ProjectPatch 仍不能真实支持 Prefab patch，因为 Prefab authoring domain command 不完整。
```

结论：

```text
下一个系统应补 Prefab Authoring Productization v1。
它是 M7 底座的产品化收尾，也是 ProjectPatch Prefab capability v2 的前置条件。
```

## 4. 方案对比

### 4.1 方案 A：直接扩 ProjectPatch Prefab capability

做法：

```text
在 ProjectPatch 中新增 Prefab operations。
让 AI 直接生成 PrefabPatch。
PatchApplier 直接写 Prefabs/*.prefab.json 或 Scene prefab instance。
```

优点：

```text
AI 修改能力看起来进展最快。
可以直接衔接 202 的 ProjectPatch 产品化入口。
```

缺点：

```text
Prefab authoring domain 自身还没有产品化命令。
AI patch 会绕过用户手动编辑工作流。
失败时很难判断是 patch schema、Prefab service、Scene service 还是 RuntimePackage cook 的问题。
会违反 202 的边界：不要直接扩 ProjectPatch 到未产品化 domain。
```

结论：

```text
不采用。
Prefab patch capability 应在本系统完成后作为 v2 讨论。
```

### 4.2 方案 B-min：Unity-like Prefab Mode 最小产品化

做法：

```text
保留 M7 的 PrefabAsset / PrefabInstance / PrefabOverride / ResolvedPrefabView。
新增最小 Prefab Stage / Prefab Mode 心智：
  用户打开 PrefabAsset 时进入 PrefabStageModel。
  PrefabStageModel 是编辑上下文，不是新的运行时层。
  PrefabStage 中编辑写 PrefabAsset。
  Scene 中编辑 PrefabInstance 写 PrefabOverride。
新增最小 Apply / Revert：
  RevertOverride 删除 instance override。
  ApplyOverrideToPrefab 把单个 override 写回 PrefabAsset，并删除该 instance override。
补 create/open/save/instantiate/validate/report。
让 manual walkthrough 中 Prefab 域从 Focus/Missing 推进到 executable / needs context。
```

优点：

```text
用户心智更接近 Unity：Prefab 可以被打开、编辑、保存，而不是只有散命令。
AI 能明确区分 PrefabAsset 编辑、Scene PrefabInstance 编辑和 Override 编辑。
复杂项目能力强：Enemy/Bullet/Explosion 可以成为可复用资产，并能被实例覆盖。
仍然复用已有 M7 / M8 / M9 / RuntimePackage / Runtime Prefab 底座。
```

缺点：

```text
比纯 command 产品化更复杂。
需要定义 PrefabStageModel、stage dirty/save、stage preview/report。
ApplyOverrideToPrefab 会修改 PrefabAsset 真相，必须严格限制为单个 override、可诊断、可测试。
```

结论：

```text
采用。
这是用户要求的方案 B 的 B-min：有 Prefab Mode 心智，但不做完整 Unity Prefab 系统。
```

### 4.3 方案 C-min：只产品化已有 M7 命令入口

做法：

```text
保留 M7 的 PrefabAsset / PrefabInstance / PrefabOverride / ResolvedPrefabView。
补 UiCommandPayload / WorkflowCommandResolver / EditorSession prefab authoring service。
补 create/open/save/instantiate/validate/revert 最小闭环。
补 complex shooter prefab authoring report。
不建立 PrefabStageModel。
```

优点：

```text
AI 适配性强：schema-first、command/report 可审查、可测试。
施工最快。
复用已有 M7 / M8 / M9 / RuntimePackage / Runtime Prefab 底座。
```

缺点：

```text
用户心智仍不像真正的 Prefab 编辑：能执行命令，但没有“打开 Prefab 进入编辑上下文”的感觉。
AI 查问题时仍容易混淆 PrefabAsset 编辑和 PrefabInstance override 编辑。
长期会在复杂项目里暴露心智债。
不会一次完成 Nested / Variant / full Apply/Revert UI。
Prefab ProjectPatch 仍需后续接入。
```

结论：

```text
不采用本轮。
它太保守，不能满足用户要求的方案 B B-min。
```

## 5. 推荐方案

采用：

```text
方案 B-min：Unity-like Prefab Mode 最小产品化
```

过滤依据：

### 5.1 AI 适配性

通过。

```text
PrefabAsset / PrefabInstance / PrefabOverride 已经是结构化数据。
PrefabStageModel 让 AI 明确知道当前是在编辑 PrefabAsset，而不是普通 Scene Entity。
UiCommandPayload + EditorSession command 能给 AI 稳定入口。
PrefabWorkflowReport 能给 AI 失败诊断和下一步修复建议。
ManualWalkthroughCoverageReport 能确认该域是否真实可操作。
```

### 5.2 复杂项目适配与可维护

通过。

```text
复杂打飞机需要反复复用 Enemy / Bullet / Explosion。
自走棋也需要 Unit / Projectile / Effect / Board Marker 等可复用对象。
Prefab Stage 让模板编辑、实例编辑和 override 管理成为一个可理解的产品闭环。
项目对象复用回到资产系统，而不是散落 Scene Entity。
```

### 5.3 效率

通过。

```text
本轮复用已有代码。
只做最小 Prefab Stage / Prefab Mode，不做完整 Unity Prefab 系统。
把可施工、可测、可报告的 authoring 闭环补上，同时保留用户心智。
```

## 6. v1 正式边界

### 6.1 v1 要做

```text
Prefab authoring command model。
PrefabAuthoringService 或扩展 PrefabWorkflowService 的 open/save/validate/report 能力。
PrefabStageModel / PrefabStageReport。
Create Prefab From Selection。
Open Prefab Document。
Enter Prefab Stage。
Exit Prefab Stage。
Instantiate Prefab In Scene。
Edit Prefab Asset In Stage。
Edit Prefab Instance Override In Scene。
Apply Single Override To Prefab。
Revert Prefab Override。
Save Prefab Document。
Validate Prefab References。
Manual walkthrough / Authoring workflow / AI context 接入。
Complex Shooter Prefab Authoring Productization Report。
```

### 6.2 v1 真实可执行操作

```text
create_prefab_from_selection:
  selected scene entity tree -> PrefabAsset -> Prefabs/*.prefab.json

open_prefab_document:
  Prefabs/*.prefab.json -> PrefabStageModel / Report

enter_prefab_stage:
  PrefabAsset -> isolated PrefabStageModel

exit_prefab_stage:
  save / discard / keep dirty diagnostic

instantiate_prefab_in_scene:
  PrefabAsset ref -> Scene Entity with engine.prefab_instance

edit_prefab_asset_in_stage:
  PrefabStageModel selected entity field edit -> PrefabAsset entity/component field mutation

edit_prefab_instance_override:
  Inspector property edit -> PrefabOverride

apply_override_to_prefab:
  one PrefabOverride -> update PrefabAsset field -> remove instance override

revert_prefab_override:
  Remove one PrefabOverride from Scene prefab instance

save_prefab_document:
  PrefabStageModel / PrefabAsset -> Prefabs/*.prefab.json

validate_prefab_references:
  PrefabAsset + Scene PrefabInstance -> PrefabWorkflowReport
```

### 6.3 v1 不做

```text
完整 Unity Prefab Mode UI。
Nested Prefab。
Prefab Variant。
Prefab inheritance。
Unpack Prefab。
批量 Apply / Revert UI。
Prefab Stage 中运行任意 project rule。
Prefab ProjectPatch capability v2。
真实 LLM direct prefab patch。
项目玩法专用 Prefab operation。
```

### 6.4 ProjectPatch 边界

v1 完成后，ProjectPatch 仍不自动获得 Prefab patch capability。

正确顺序是：

```text
Prefab Authoring Productization v1
  -> Prefab domain commands/report stable
  -> 再讨论 ProjectPatch Prefab capability v2
```

原因：

```text
AI patch 只能调用已有正式 authoring domain。
不能用 patch 绕过用户手动编辑链路。
```

## 7. 建议数据结构

### 7.1 PrefabAuthoringModel

```text
PrefabAuthoringModel
  schema_version
  active_stage: Option<PrefabStageModel>
  open_prefab_paths
  validation_report: PrefabAuthoringReport
```

说明：

```text
它是编辑器 UI / AI context 的可序列化模型。
PrefabAsset 仍是真相。
PrefabStageModel 是编辑上下文，不是运行时层，也不是第二份长期真相。
ResolvedPrefabView 仍是只读视图。
```

### 7.2 PrefabStageModel

```text
PrefabStageModel
  schema_version
  stage_id
  mode: Isolated | InContext
  source_prefab_path
  source_prefab_id
  working_prefab: PrefabAsset
  selected_source_entity_id
  opened_from_instance_entity_id
  opened_from_instance_id
  dirty
  preview: ResolvedPrefabView
  diagnostics
```

规则：

```text
Isolated 是 v1 默认模式。
InContext 只允许在已有 Scene PrefabInstance 上下文中显示 source instance 信息，不做完整 Unity in-context scene rendering。
working_prefab 保存时覆盖 PrefabAsset 文件。
PrefabStageModel 不保存到 RuntimePackage。
```

### 7.3 PrefabStageReport

```text
PrefabStageReport
  schema_version
  stage_id
  mode
  source_prefab_path
  source_prefab_id
  dirty
  selected_source_entity_id
  entity_count
  component_count
  override_count_from_opened_instance
  diagnostics
  next_actions
```

### 7.4 PrefabAuthoringReport

```text
PrefabAuthoringReport
  schema_version
  status
  project_root
  active_stage_id
  active_prefab_path
  prefab_assets_count
  prefab_instances_count
  created_prefab_paths
  instantiated_entity_ids
  overrides_count
  applied_override_count
  reverted_override_count
  stage_report
  diagnostics
  next_actions
```

用途：

```text
给用户看：Prefab 工作流是否真的完成。
给 AI 看：下一次应修哪个 prefab_ref / instance_id / field_path。
给测试看：复杂项目是否真的使用 Prefab，而不是只存在 prefab 文件。
```

### 7.3 UiCommandPayload 建议

```text
CreatePrefabFromSelection {
  scene_path: Option<String>
  root_entity_id: String
  prefab_id: String
  name: String
  replace_selection_with_instance: bool
}

OpenPrefabDocument {
  path: String
}

EnterPrefabStage {
  path: String
  mode: PrefabStageMode
  opened_from_instance_entity_id: Option<String>
}

ExitPrefabStage {
  save_policy: Save | Discard | KeepOpen
}

InstantiatePrefabInScene {
  prefab_id: String
  parent_entity_id: Option<String>
  local_position: Option<Vec3>
}

SetPrefabStageEntityField {
  source_entity_id: String
  component_type: Option<String>
  field_path: String
  value: serde_json::Value
}

ApplyPrefabOverrideToAsset {
  instance_entity_id: String
  target_source_entity_id: String
  component_type: String
  field_path: String
}

SavePrefabDocument {
  path: String
}

ValidatePrefabReferences {
  path: Option<String>
}

RevertPrefabOverride {
  instance_entity_id: String
  target_source_entity_id: String
  component_type: String
  field_path: String
}
```

第一版只允许 `ApplyPrefabOverrideToAsset` 应用单个 override，不做批量 apply，也不做 nested / variant 传播。

## 8. 与现有系统关系

### 8.1 与 M7

```text
M7 提供 Prefab 数据模型和底座。
203 提供产品化 authoring 入口、命令、报告、walkthrough 可执行证据。
```

203 不推翻 140。

### 8.2 与 Inspector

```text
Inspector 负责字段编辑。
Prefab workflow 决定字段写入 SceneEntity 还是 PrefabOverride。
```

当前 `PropertyTransactionRouter` 已有 PrefabInstance 路由，203 要把它接入用户可执行命令和报告。

### 8.3 与 Asset Browser

```text
Asset Browser 负责选择 PrefabAsset。
Prefab Authoring 负责 open / instantiate / validate。
```

当前 `AssetPlacementResolver` 已支持 `asset_type=prefab`，203 应复用它，不另造一套放置流程。

### 8.4 与 RuntimePackage

```text
ProjectRuntimePackageAssembler 收集 Prefabs/*.json。
RuntimePackageBuilder cook authoring-prefab-asset.v1 为 runtime-prefab.v1。
RuntimeInstanceLoader 在运行时实例化 RuntimePrefabData。
```

203 只保证 authoring 侧文件和 report 正确，不把 editor-only 模型带入 runtime。

### 8.5 与 Manual Walkthrough

203 完成后，以下 Prefab 操作应从 Missing / Focus 推进：

```text
create_prefab_from_selection -> executable or executable_needs_context
open_prefab_document -> executable_needs_context
enter_prefab_stage -> executable_needs_context
exit_prefab_stage -> executable_needs_context
instantiate_prefab_in_scene -> executable_needs_context
apply_prefab_changes -> executable_needs_context for single override only
save_prefab_document -> executable_needs_context
validate_prefab_references -> executable
```

## 9. 复杂打飞机验收场景

### 场景 A：从 Scene 敌机创建 Prefab

输入：

```text
selected entity = entity-enemy-a
prefab_id = prefab-enemy-scout-v2
```

期望：

```text
生成 Prefabs/prefab-enemy-scout-v2.prefab.json。
PrefabAsset schema = authoring-prefab-asset.v1。
rootEntityId / entities / component data 完整。
PrefabAuthoringReport 记录 created_prefab_paths。
可立即 EnterPrefabStage 打开该 Prefab。
```

### 场景 B：打开 Prefab Stage 并编辑 PrefabAsset

输入：

```text
path = Prefabs/enemy_scout.prefab.json
stage edit: project.linearMotion.velocity.y = -2.0
```

期望：

```text
EnterPrefabStage 生成 PrefabStageModel。
SetPrefabStageEntityField 修改 working_prefab。
SavePrefabDocument 写回 PrefabAsset。
PrefabStageReport 标记 dirty -> saved。
```

### 场景 C：把 Prefab 实例化进 Scene

输入：

```text
prefab_id = prefab-enemy-scout
position = { x: 0, y: 4, z: 0 }
```

期望：

```text
Scene 新增 prefab_instance entity。
entity.components 包含 engine.prefab_instance。
source.id = prefab-enemy-scout。
Manual walkthrough 把 instantiate_prefab_in_scene 识别为可执行或需要上下文。
```

### 场景 D：编辑实例字段产生 Override，并可单项 Apply / Revert

输入：

```text
PrefabInstance entity
component_type = project.linearMotion
field_path = velocity.y
value = -2.5
```

期望：

```text
普通实例编辑不直接修改 PrefabAsset。
Scene instance 记录 PrefabOverride。
ResolvedPrefabView 应用 override。
Inspector report 标记 override_count。
RevertPrefabOverride 删除该 override。
ApplyPrefabOverrideToAsset 把单个 override 写回 PrefabAsset 并删除该 instance override。
```

### 场景 E：Prefab 验证报告

输入：

```text
samples/complex_shooter_project
```

期望：

```text
PrefabAuthoringReport 能列出 prefab_assets_count >= 3。
能扫描 Scene 中 prefab instances。
缺失 prefab_ref 输出 missing_prefab_asset diagnostic。
```

### 场景 F：端到端构建不退化

输入：

```text
Prefab authoring productization report
ProjectRuntimePackageAssembler
project_e2e_gate
```

期望：

```text
RuntimePackage 中仍包含 prefabs。
complex shooter e2e 继续通过。
Prefab 相关报告可以回指 source path / prefab_ref / instance_id / field_path。
```

## 10. 可施工 Gate 建议

### Gate A：Prefab Command Surface

目标：

```text
editor_ui_model 增加 Prefab authoring UiCommandPayload。
包含 EnterPrefabStage / ExitPrefabStage / SetPrefabStageEntityField / ApplyPrefabOverrideToAsset。
WorkflowCommandResolver / manual walkthrough 能识别 Prefab Stage 操作。
```

测试：

```powershell
cargo test -p editor_ui_model workflow_command
cargo test -p editor_ui_model manual_walkthrough
```

### Gate B：Prefab Stage Model / Authoring Service

目标：

```text
editor_core 增加 PrefabStageModel。
实现 open / enter stage / edit working prefab / save / exit。
实现 create / instantiate / validate / revert / apply single override 的服务入口。
复用 PrefabWorkflowService / AssetPlacementResolver / SceneEditCommand。
```

测试：

```powershell
cargo test -p editor_core prefab
cargo test -p editor_core inspector
```

### Gate C：EditorSession Command Integration

目标：

```text
EditorSession 能执行 Prefab authoring commands。
Scene 中的 PrefabInstance 命令进入现有事务 / dirty / report 链路。
PrefabStage 命令进入 PrefabAuthoringModel dirty / save / report 链路。
```

测试：

```powershell
cargo test -p editor_core editor_command_registry
cargo test -p editor_core prefab
```

### Gate D：Manual Walkthrough / AI Context

目标：

```text
ManualWalkthroughCoverageReport 中 Prefab 域从缺口推进。
AuthoringAiContext 暴露 Prefab authoring summary。
```

测试：

```powershell
cargo test -p editor_core manual_walkthrough
cargo test -p editor_core authoring_workflow
```

### Gate E：Complex Shooter Prefab Productization Report

目标：

```text
project_e2e_gate 生成 complex-shooter-prefab-authoring-productization-report.json。
验证 sample project 的 prefab 文件、实例化、override、RuntimePackage cook 证据。
```

测试：

```powershell
cargo test -p project_e2e_gate prefab
cargo test -p project_e2e_gate
```

### Gate F：文档同步与整体回归

目标：

```text
更新 49 / 54 / 施工文档 README / 阶段完成记录。
归档施工文档。
```

测试：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core prefab
cargo test -p project_e2e_gate
```

## 11. 施工时禁止事项

```text
禁止重写 Runtime Prefab Spawn / Despawn。
禁止新增玩法专用 Prefab operation。
禁止让 AI 直接写 Prefabs/*.json 而绕过 service / validation / report。
禁止把 ResolvedPrefabView 当保存真相。
禁止把 PrefabStageModel 当运行时层或第二份长期真相。
禁止本轮做 Nested / Variant / Prefab inheritance / Unpack。
禁止批量 Apply / Revert 或跨 prefab 传播。
禁止为了 ProjectPatch 提前伪造 Prefab patch capability。
禁止把 Scene 普通复制实体误报为 PrefabInstance。
```

## 12. 方案自审

### 12.1 是否合乎当前下一步规则

通过。

```text
49 / 54 建议从真实 authoring domain 或 ProjectPatch structured output v2 中选。
202 完成记录明确建议优先补 manual walkthrough coverage report 中 blocks_manual_walkthrough 的真实 authoring domain。
Prefab 是当前 coverage 中仍缺真实 authoring command 的核心域。
```

### 12.2 是否重复 M7

不重复。

```text
M7 已经完成 Prefab 数据模型和底座。
203 B-min 聚焦最小 Prefab Stage 心智、UiCommand、EditorSession、manual walkthrough、complex shooter report。
它复用 PrefabAsset / PrefabInstance / PrefabOverride，不重写底座。
```

### 12.3 是否合乎 AI-first

通过。

```text
所有动作收敛到 schema-first command / service / report。
AI 可以读 PrefabAuthoringReport，而不是猜文件。
AI 可以通过 PrefabStageModel 判断当前编辑对象是 PrefabAsset 还是 Scene PrefabInstance。
失败能定位 prefab_ref / instance_id / source_entity_id / field_path。
```

### 12.4 是否合乎复杂项目

通过。

```text
复杂打飞机和自走棋都需要可复用对象。
B-min 的 Prefab Stage 能减少复制实体和长期维护成本，同时让模板编辑变成清晰产品心智。
不把具体玩法写进引擎 Core。
```

### 12.5 主要风险

风险一：

```text
CreatePrefabFromSelection 需要明确选中实体树和保存路径。
```

处理：

```text
第一版允许 command 需要上下文；没有 selection / path 时报告 ExecutableCommandNeedsContext。
```

风险二：

```text
PrefabStageModel 可能被误解为新的运行时层。
```

处理：

```text
文档和施工中明确：PrefabStageModel 只存在于编辑器 authoring context，RuntimePackage 只消费 PrefabAsset cook 结果。
```

风险三：

```text
Apply Override to Prefab 会改模板真相，风险高。
```

处理：

```text
v1 只允许单个 override apply，必须输出 PrefabAuthoringReport，并保留测试覆盖。
```

风险四：

```text
样例 Scene 当前敌机仍是复制实体，可能导致 report 只看到 prefab 文件，看不到 prefab instance。
```

处理：

```text
E2E report 必须区分 prefab_assets_count 与 prefab_instances_count。
第一版可以把 sample migration 作为测试内临时项目，不强行污染原样例。
```

## 13. 最终结论

采用：

```text
Prefab Authoring Productization v1
```

正式判断：

```text
下一个系统不应直接做 ProjectPatch Prefab capability。
也不应一次做完整 Unity Prefab 系统。
应采用方案 B 的 B-min：在已有 M7 Prefab 底座上建立最小 Prefab Stage / Prefab Mode 心智，让用户和 AI 能打开 Prefab、编辑模板、编辑实例 override、单项 apply/revert、保存、验证和报告。
```

下一步：

```text
如果进入施工，基于本文生成 203 自动化施工文档。
施工文档必须先自审，再按 Gate A-F 实施和测试。
```
