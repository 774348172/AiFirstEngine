# 225-Project Authoring Asset Completeness / Prefab-Rule Assetization Gate v1 方案

## 1. 这个系统是干什么的

一句话：

```text
它不是新增引擎功能，而是把复杂打飞机项目补齐成当前引擎已经能理解、能编辑、能验证、能被 AI 修改的标准 authoring asset 结构。
```

当前引擎已经具备：

```text
Prefab Authoring
Rule Authoring
AUI Authoring
ProjectPatch All-Domain Capability
RuntimePackage
Editor Play / GameView
Apply Runtime Change To Authoring
Report Panel
```

但复杂打飞机 sample 仍有两个真实资产化缺口：

```text
Prefab:
  samples/complex_shooter_project 有 3 个 PrefabAsset
  但 Main.scene.json 里当前没有真实 engine.prefab_instance

Rule:
  samples/complex_shooter_project 有 Runtime rule-manifest
  但没有用户 / AI 默认编辑的 .rule.json authoring assets
```

所以 225 的目标不是造新层，而是让 sample project 从：

```text
能跑、能导出、能报告
```

进一步变成：

```text
能用当前 Prefab / Rule / AUI / Input / BuildProfile authoring 资产解释
能被用户在编辑器里看懂和修改
能被 AI 通过 ProjectPatch / Report 稳定修改和审查
```

本系统对标：

```text
Unity:
  Prefab Asset / Prefab Instance / Override
  ScriptableObject / C# Script serialized asset

Unreal:
  Blueprint Class / Actor Instance
  DataAsset / PrimaryDataAsset

Godot:
  PackedScene / Resource
  Scene instance
```

在本引擎主线中的作用：

```text
让复杂打飞机项目不再只是 runtime/export gate 样例，而是一个真实 authoring asset 项目。
```

## 2. 其它引擎源码参考

### Unity

官方参考：

```text
https://docs.unity3d.com/Manual/Prefabs.html
https://docs.unity3d.com/Manual/PrefabInstanceOverrides.html
https://docs.unity3d.com/Manual/class-ScriptableObject.html
```

本地源码参考：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/Prefabs/PrefabUtility.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/SceneManagement/StageManager/PrefabStage/PrefabStage.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/ProjectBrowser/ProjectWindowUtil.cs
```

关键源码点：

```text
PrefabUtility.cs:
  ApplyPrefabInstance(...)
  SaveAsPrefabAsset(...)
  SaveAsPrefabAssetAndConnect(...)
  InstantiatePrefab(...)
  SetPropertyModifications(...)

PrefabStage.cs:
  PrefabUtility.GetPropertyModifications(...)
  PrefabUtility.SaveAsPrefabAsset(...)

ProjectWindowUtil.cs:
  DoCreatePrefab
  DoCreateNewAsset
```

可学习：

```text
Prefab Asset 与 Scene Instance 是两个不同真相。
Scene 中放的是实例，不是把 Prefab 当普通复制对象。
修改实例后必须能表达 override / apply / revert。
资产创建、实例化、保存、修改都进入 editor transaction / asset database。
```

不可照搬：

```text
不复制 Unity GameObject / Component 对象模型。
不复制 Unity native Prefab serialization / PropertyModification 黑盒。
不把复杂打飞机 gameplay 写成引擎内置组件。
本项目必须输出 AI 可读的结构化 report / candidate / diagnostic。
```

### Unreal Engine

官方参考：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/blueprints-visual-scripting-in-unreal-engine
https://dev.epicgames.com/documentation/en-us/unreal-engine/data-assets-in-unreal-engine
```

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/Kismet/Private/BlueprintEditorViewportContextMenuExtender.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Editor/Kismet/Public/BlueprintEditorModule.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Classes/Engine/DataAsset.h
```

关键源码点：

```text
BlueprintEditorViewportContextMenuExtender.cpp:
  Create Blueprint from selected Actor
  FKismetEditorUtilities::CanCreateBlueprintOfClass(...)
  FCreateBlueprintFromActorDialog::OpenDialog(...)

BlueprintEditorModule.h:
  CreateBlueprintEditor(...)

DataAsset.h:
  UDataAsset
  UPrimaryDataAsset
```

可学习：

```text
可复用对象 / 规则 / 数据要成为资产。
Level 里放实例，资产本体可单独编辑。
从场景对象生成可复用资产是显式 authoring 操作。
```

不可照搬：

```text
不复制 UObject / UClass / Blueprint VM。
不把本项目 Rule Asset 做成 UE Blueprint 等价物。
不把 DataAsset 的自由 UObject 引用模型搬进当前 AssetRef / GUID 体系。
```

### Godot

官方参考：

```text
https://docs.godotengine.org/en/stable/classes/class_packedscene.html
https://docs.godotengine.org/en/stable/tutorials/scripting/resources.html
```

本地源码参考：

```text
框架设计/Godot源码参考/04-Object-Node-PackedScene源码参考.md
框架设计/Godot源码参考/AI-Project-Patch-EditorUndoRedo源码参考.md
```

可学习：

```text
PackedScene 保存 authoring data，instantiate 创建运行时对象。
资源和场景节点分离，但编辑器可以把二者作为统一创作体验呈现。
保存资源走 ResourceSaver，编辑变更走 UndoRedo action。
```

不可照搬：

```text
不采用 Node / Variant / dynamic script 作为本项目主线。
不把 PackedScene 的场景即 prefab 心智直接覆盖到当前 Scene / Prefab / RuntimePackage 分层。
```

## 3. 本项目当前基线

已完成前置：

```text
203 Prefab Authoring Productization v1
193 Rule Authoring Productization v1
207 ProjectPatch All-Domain Capability v2
212 Report Panel / Evidence Panel Productization v1
217 Editor Play / RuntimePackage Preview Productization v1
224 Editor Play Apply Runtime Change To Authoring v1
191 Authoring Walkthrough Missing Operations Convergence v1
```

当前 sample 项目事实：

```text
samples/complex_shooter_project/Prefabs/
  enemy_scout.prefab.json
  explosion_effect.prefab.json
  player_bullet.prefab.json

samples/complex_shooter_project/Scenes/Main.scene.json
  当前存在普通 entity-player / entity-enemy-a / entity-enemy-b
  当前没有真实 engine.prefab_instance

samples/complex_shooter_project/Rules/rule-manifest.json
  rule.player-move
  rule.fire-bullet
  rule.lifetime-cleanup

samples/complex_shooter_project/Rules/
  当前没有对应 .rule.json authoring assets
```

当前报告已经诚实暴露这些缺口：

```text
Prefab report:
  3 个 PrefabAsset
  0 个 Scene PrefabInstance
  next_action = instantiate_prefab_in_scene

Rule report:
  rule_authoring_assets_missing
  next_action = create_rule_asset / migrate_runtime_manifest_to_rule_authoring_assets
```

因此 225 不应重新实现 Prefab 或 Rule，而应做：

```text
Analyze -> Candidate -> User/AI Review -> Apply existing authoring commands -> Report -> Gate
```

## 4. 方案选择

### 方案 A：只修复杂打飞机 sample 数据

直接修改 sample：

```text
把 enemy / bullet / explosion 等改成 PrefabInstance 或 Rule Asset。
补齐 Rules/*.rule.json。
```

优点：

```text
最快让 sample 报告变绿。
施工范围小。
```

缺点：

```text
像一次性补样例，不是可复用能力。
AI 无法稳定知道后续新项目是否也缺同类资产化。
缺少结构化候选和迁移报告，后续维护仍会反复靠人工判断。
```

### 方案 B-min：Authoring Asset Completeness Gate（采用）

新增项目资产完整性分析和迁移候选：

```text
ProjectAuthoringAssetCompletenessAnalyzer
  -> ProjectAuthoringAssetCompletenessReport
  -> AssetizationCandidate
  -> 可选迁移命令 / ProjectPatch
  -> 复杂打飞机 gate
```

它检查：

```text
PrefabAsset 是否有真实使用证据：
  Scene PrefabInstance
  Rule / component runtime spawn reference
  explicit waiver

Scene 中的普通实体是否疑似应转成 PrefabInstance：
  与已有 PrefabAsset 结构 / name / component 相似
  但不能自动强制转换，必须生成 candidate

Runtime rule manifest 是否有对应 .rule.json authoring asset：
  rule_id 匹配
  phase 匹配
  artifact / ir source 可追踪
  没有时生成 migration candidate
```

优点：

```text
AI 适配性最好：report / candidate / diagnostic 都是结构化数据。
复杂项目可维护：以后新项目也可以跑同一 gate。
不新增 runtime 层，只复用现有 Prefab / Rule / ProjectPatch 能力。
能把 complex shooter sample 从“能跑”推进到“资产真相可编辑”。
```

缺点：

```text
需要新增一个分析报告和少量迁移命令。
第一版不能自动判断所有普通实体都该变成哪个 Prefab。
Prefab 使用证据需要区分 scene instance、runtime spawn reference、explicit waiver。
```

### 方案 C：完整 Project Template / Feature Folder 资产化

一次把复杂打飞机重组为：

```text
Feature Folder
Prefab
Rule
AUI
Input
Asset
BuildProfile
Tests
```

优点：

```text
长期结构最完整。
更接近真实商业项目组织。
```

缺点：

```text
范围太大。
容易把 Feature Folder 变成新的运行时层。
可能和 195 / 196 的“不新增运行时结构层”冲突。
不适合当前紧接 224 的下一步。
```

## 5. 正式采用：方案 B-min

正式名称：

```text
Project Authoring Asset Completeness / Prefab-Rule Assetization Gate v1
```

用户心智：

```text
这不是新增功能。
这是检查并补齐项目资产真相：
  哪些对象应该是 PrefabAsset / PrefabInstance
  哪些规则应该是 .rule.json authoring asset
  哪些 runtime-only 产物不能作为用户编辑真相
```

内部链路：

```text
Project files
  -> Scan Scene / Prefab / Rule / AUI / Input / BuildProfile
  -> Compare with completed authoring domains
  -> Build assetization candidates
  -> User / AI review
  -> Apply via existing command / ProjectPatch / file transaction
  -> Run existing e2e gates
  -> ProjectAuthoringAssetCompletenessReport
```

它不新增：

```text
Runtime system
Prefab runtime loader
Rule VM
Logic router
Feature Folder runtime layer
```

## 6. 数据模型

新增：

```text
ProjectAuthoringAssetCompletenessReport:
  schema_version
  project_root
  status: passed | partial | failed
  scanned_domains
  prefab_summary
  rule_summary
  candidates
  diagnostics
  next_actions

PrefabCompletenessSummary:
  prefab_asset_count
  scene_prefab_instance_count
  runtime_spawn_reference_count
  unused_prefab_asset_ids
  missing_scene_instance_evidence
  explicit_waivers

RuleCompletenessSummary:
  runtime_manifest_rule_count
  rule_authoring_asset_count
  missing_authoring_rule_ids
  stale_authoring_rule_ids
  migration_candidate_count

AssetizationCandidate:
  candidate_id
  domain: prefab | rule
  source_kind
  source_path
  target_path?
  scene_entity_id?
  prefab_asset_id?
  rule_id?
  confidence: high | medium | low
  status: ready | blocked | warning
  apply_route
  diagnostics
```

说明：

```text
Report 是 AI 和测试的真相。
Candidate 是可审查的迁移建议，不等于自动修改。
Prefab usage 不只等于 Scene PrefabInstance；runtime spawn reference 也可以是合法使用证据。
但复杂打飞机 C-min 至少应保留一个真实 Scene PrefabInstance，用于验证用户可见 PrefabInstance / override 工作流。
Rule manifest 不是用户默认编辑入口；缺 .rule.json 时必须报告 partial。
```

## 7. Prefab 资产化规则

允许识别的 Prefab 使用证据：

```text
Scene PrefabInstance:
  Scene entity 具有 engine.prefab_instance component。

Runtime spawn reference:
  Scene component / Rule Asset 明确引用 prefabId。
  例如 project.spawnEmitter.prefabId = prefab-player-bullet。

Explicit waiver:
  项目显式声明某个 PrefabAsset 当前只作为 library/template，没有进入本场景。
  waiver 必须进入 report，不能静默忽略。
```

普通 Scene Entity 到 PrefabInstance 的迁移候选：

```text
entity-enemy-a / entity-enemy-b
  与 prefab-enemy-scout name / components / sprite / motion 相似
  可生成 ConvertSceneEntityToPrefabInstance candidate

entity-player
  当前含 playerController + spawnEmitter
  不应自动套用 player_bullet prefab
  只能作为 blocked / no_matching_prefab 或后续 Player prefab candidate
```

C-min 推荐：

```text
不要强制所有 enemy 都自动替换。
先至少让 sample Scene 有一个真实 engine.prefab_instance。
保留普通实体时必须在 report 里说明原因。
```

## 8. Rule 资产化规则

Runtime manifest 中的规则：

```text
rule.player-move
rule.fire-bullet
rule.lifetime-cleanup
```

必须能追溯到用户 / AI 可编辑的 authoring rule asset：

```text
Rules/player_move.rule.json
Rules/fire_bullet.rule.json
Rules/lifetime_cleanup.rule.json
```

Rule authoring asset 不等于直接手写 runtime manifest：

```text
.rule.json
  -> RuleAuthoringService validate / build
  -> ProjectRuleAsset / Canonical Rule IR 内部规范语义
  -> artifact lifecycle / static registry source evidence
  -> runtime rule-manifest
  -> RuntimePackage
```

迁移候选规则：

```text
如果 runtime manifest entry 有 ruleId / phase / irSource / artifactId：
  可以生成 MigrateRuntimeRuleManifestEntryToAuthoringRule candidate。

如果缺少足够信息：
  candidate status=blocked
  diagnostic=runtime_manifest_entry_not_migratable
```

C-min 不要求生成完整复杂规则图：

```text
可以生成 minimal .rule.json authoring asset。
必须保留 rule_id / phase / source manifest evidence。
必须让 RuleAuthoringService 能 load / validate / build 或给出明确 diagnostic。
```

## 9. AI / Report / ProjectPatch 语义

AI 必须能回答：

```text
这个项目有哪些 PrefabAsset？
哪些 PrefabAsset 被 Scene 使用？
哪些 PrefabAsset 只被 runtime spawn 引用？
哪些 PrefabAsset 完全未使用？
哪些普通 Scene Entity 建议转成 PrefabInstance？
runtime rule-manifest 中哪些 rule 缺 authoring asset？
迁移后是否还能 build / play / export？
```

Report 分档：

```text
Runtime:
  不新增 runtime report。

Editor Summary:
  prefab_asset_count / prefab_instance_count / missing_rule_asset_count / candidate_count。

Editor Trace:
  每个 candidate 的 source path、target path、diagnostic、apply route。
```

ProjectPatch 关系：

```text
ProjectPatch 可以承载迁移操作，但 ProjectPatch 不是唯一入口。
手动 authoring command 和 ProjectPatch 都必须走同一底层事务 / report。
AI 生成迁移 patch 前必须先读取 ProjectAuthoringAssetCompletenessReport。
```

## 10. 复杂打飞机验收目标

最小用户体验：

```text
打开复杂打飞机项目。
运行 Project Authoring Asset Completeness report。
看到 Prefab / Rule 两个域的真实状态。
看到 entity-enemy-a / entity-enemy-b 的 PrefabInstance 迁移候选。
看到 rule.player-move / rule.fire-bullet / rule.lifetime-cleanup 缺 .rule.json 的迁移候选。
用户确认迁移。
Scene 中出现至少一个真实 engine.prefab_instance。
Rules/ 下出现对应 .rule.json authoring assets。
Prefab Authoring report 不再因为 sample 0 scene instance 固定 partial。
Rule Authoring report 不再因为 sample 缺 .rule.json 固定 partial。
RuntimePackage / Editor Play / Export 仍通过。
```

自动化 gate：

```text
headless deterministic。
扫描 samples/complex_shooter_project。
生成 ProjectAuthoringAssetCompletenessReport。
断言 report 能检测当前 Prefab / Rule 缺口。
执行 C-min 迁移。
断言 Scene 中存在 engine.prefab_instance。
断言 Rules/*.rule.json 存在并能被 RuleAuthoringService 读取。
断言 project_e2e_gate 中 prefab / rule 相关 partial 缺口收敛。
断言 RuntimePackage build / complex shooter e2e 仍通过。
```

## 11. 推荐施工 Gate

```text
Gate A: 现状锁定
  读取 203 / 193 / 207 / 191 / 224 当前规则。
  锁定 samples/complex_shooter_project 当前 Prefab / Rule 缺口。
  不改 runtime。

Gate B: Completeness report
  新增 ProjectAuthoringAssetCompletenessReport。
  扫描 PrefabAsset、Scene PrefabInstance、runtime spawn reference、Rule manifest、Rule authoring asset。
  输出 prefab_summary / rule_summary / diagnostics / next_actions。

Gate C: Assetization candidates
  生成 Prefab candidate：
    convert_scene_entity_to_prefab_instance
    prefab_used_by_runtime_spawn_reference
    unused_prefab_asset
  生成 Rule candidate：
    migrate_runtime_manifest_entry_to_authoring_rule
  所有 candidate 必须有 ready / blocked / warning。

Gate D: C-min apply
  复用现有 Prefab / Rule / ProjectPatch / file transaction。
  对 sample 执行最小迁移。
  不自动推断复杂玩法语义。
  不新增打飞机专用 API。

Gate E: E2E gate
  project_e2e_gate 新增或扩展 complex-shooter-authoring-asset-completeness report。
  断言 Prefab / Rule authoring partial 缺口收敛。
  RuntimePackage / Editor Play / Export 回归通过。

Gate F: 文档同步
  更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
  归档施工文档。
```

## 12. Deferred 边界

不进入 225 B-min：

```text
完整 Feature Folder 重组。
完整 Project Template 系统。
Prefab Variant。
Prefab Asset Apply from Play Mode。
运行时生成对象反向创建 Scene Entity。
所有普通 Scene Entity 的全自动智能归类。
完整 Rule Graph 可视化编辑器。
真实 LLM repair loop。
完整字体导入与复杂文本排版。
runtime undo stack。
```

## 13. 最终结论

225 采用：

```text
Project Authoring Asset Completeness / Prefab-Rule Assetization Gate v1 = 方案 B-min
```

它解决的是：

```text
复杂打飞机项目当前还没有完全资产化为本引擎可理解的 authoring truth。
```

它不解决：

```text
新增运行时系统。
新增玩法 API。
新增脚本语言。
```

它的价值是把复杂打飞机从“演示能跑”推进到“真实项目可长期编辑”：

```text
PrefabAsset / PrefabInstance / RuleAsset / AUI / Input / BuildProfile
  -> 都成为用户和 AI 可审查、可修改、可验证的项目资产。
```
