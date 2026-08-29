# 207-ProjectPatch All-Domain Capability v2 方案

## 1. 系统是什么

本系统正式命名为：

```text
ProjectPatch All-Domain Capability v2
```

一句话说明：

```text
它让 ProjectPatch 从只能修改 Scene / Input，扩展为可以安全修改 Asset / Prefab / AUI / Rule / Build 等主要项目域。
```

用户心智：

```text
AI 生成 ProjectPatch
  -> 导入 / 校验 / 审阅
  -> 用户确认
  -> 修改项目资产或执行构建验证
  -> 失败可诊断，修改可回滚或可恢复
```

它解决 205 / 206 之后的真实瓶颈：

```text
205 已能导入外部 ProjectPatch JSON。
206 已能让 LLM 成为 ProjectPatch JSON 的薄输入源。
但当前 ProjectPatch 真实可 apply 的 domain 仍主要是 Scene / Input。
复杂打飞机项目需要 AI 能继续修改 Prefab、AUI、Rule、Asset，并触发 Build 验证。
```

本系统不是：

```text
不是让 AI 直接 fs::write 项目文件。
不是新建第二套编辑器命令系统。
不是新建 Agent Planner / Repair Loop。
不是把 Player / Enemy / Bullet 等玩法概念写进引擎 Core。
不是把 Build Graph / Asset DB / AUI / Rule / Prefab 各自重写一遍。
```

## 2. 为什么采用方案 A

上一轮讨论有三个方向：

```text
方案 A：一次扩 ProjectPatch 的 Asset / Prefab / AUI / Rule / Build 能力。
方案 B：先做单域 Rule Capability。
方案 C：Thin UiCommandPayload Router。
```

本轮用户选择：

```text
采用方案 A。
```

采纳理由：

```text
这些 domain 最终都需要进入 ProjectPatch。
继续按单域方案拆，会让 AI patch 能力长期处于“这里能改、那里不能改”的断裂状态。
复杂打飞机 / 自走棋这类项目需要一次 patch 同时改规则、UI、Prefab、资源并触发构建验证。
```

为控制风险，方案 A 收敛为：

```text
All-Domain A-min。
```

含义：

```text
一次性建立五个 domain 的正式 PatchOperation schema / validator / applier / report。
每个 domain 只接入当前已经产品化或已有服务支撑的最小操作集。
施工仍按 Gate 分域测试，但它们属于同一个 ProjectPatch 全域能力系统，不再拆成五个新系统。
```

## 3. 其它成熟引擎 / 工具对标

成熟引擎没有完全等价的 AI ProjectPatch，但它们共同遵守一个原则：

```text
外部工具或编辑器 UI 产生的修改，必须进入正式结构化编辑、Undo/Redo、Dirty/Save、验证和报告链路。
```

### 3.1 Unity

对标：

```text
Editor Tool / Inspector
  -> SerializedObject / SerializedProperty
  -> ApplyModifiedProperties
  -> Undo / Dirty
  -> AssetDatabase / Scene save
```

官方资料：

```text
https://docs.unity3d.com/2023.1/Documentation/ScriptReference/SerializedObject.ApplyModifiedProperties.html
https://docs.unity3d.com/6000.4/Documentation/ScriptReference/Undo.RecordObject.html
```

本项目已有源码参考：

```text
框架设计/Unity源码参考/AI-Project-Patch-EditorTransaction源码参考.md
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector\TransformInspector.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\UIElements\Controls\PropertyField.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\AssetDatabase\AssetDatabase.cs
```

可学习点：

```text
字段修改必须走结构化属性路径。
Apply 前后有 Undo / Dirty / Save 语义。
资产修改不能绕过 AssetDatabase / Importer。
```

不照搬：

```text
不让 AI 生成 Unity Editor 脚本。
不把 ProjectPatch 降级成 SerializedObject 的薄包装。
```

### 3.2 Unreal Engine

对标：

```text
Editor Command / Tool
  -> FScopedTransaction
  -> UObject::Modify
  -> mutation
  -> PostEditChange / MarkPackageDirty
```

官方资料：

```text
https://dev.epicgames.com/documentation/unreal-engine/API/Editor/UnrealEd/FScopedTransaction
```

本项目已有源码参考：

```text
框架设计/UE源码参考/AI-Project-Patch-EditorTransaction源码参考.md
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\EditorActor.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UMGEditor\Private\WidgetBlueprintEditorUtils.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UMGEditor\Private\WidgetBlueprintOperationUtils.cpp
```

可学习点：

```text
命令入口和真实事务分离。
不同 domain 可以有不同 handler，但都必须进入事务。
复杂 UMG / Actor / Asset 修改不能绕过 Dirty / Package 边界。
```

不照搬：

```text
不引入 UObject / Package / Slate 体系。
不把 ProjectPatch 变成每个工具各自一套脚本入口。
```

### 3.3 Godot

对标：

```text
EditorUndoRedoManager
  -> create_action
  -> add_do_method / add_undo_method / add_do_property / add_undo_property
  -> commit_action
```

官方资料：

```text
https://docs.godotengine.org/en/stable/classes/class_editorundoredomanager.html
https://docs.godotengine.org/en/latest/classes/class_undoredo.html
```

本项目已有源码参考：

```text
框架设计/Godot源码参考/AI-Project-Patch-EditorUndoRedo源码参考.md
<GODOT_SOURCE>\godot-master\godot-master\editor\editor_node.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\inspector\editor_inspector.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\scene\3d\node_3d_editor_viewport.cpp
```

可学习点：

```text
一次用户意图可以拆成一组 do / undo 操作。
每个操作必须能报告目标对象和失败点。
Scene / Resource 修改都要进入统一编辑动作边界。
```

不照搬：

```text
不让 AI 自由生成 Object method 字符串。
不采用动态反射调用作为 patch 真相。
```

### 3.4 Bevy

对标：

```text
Reflect / TypeRegistry / DynamicScene / Asset Handle / Commands
```

官方资料：

```text
https://docs.rs/bevy/latest/bevy/reflect/index.html
```

本项目已有源码参考：

```text
框架设计/Bevy源码参考/AI-Project-Patch-Reflect-DynamicWorld源码参考.md
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app\src\app.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_asset\src\reflect.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_world_serialization\src\world_asset_spawner.rs
```

可学习点：

```text
字段路径、类型、资产引用必须可验证。
未知类型 / 未注册组件 / 缺失资产要在 Validate 阶段报告。
运行时 World 不应成为编辑器项目真相。
```

不照搬：

```text
不把 Bevy Commands 当成编辑器事务系统。
不直接 patch runtime ECS World。
```

## 4. 本项目当前基线

已完成：

```text
202 ProjectPatch Productization v1
  AI proposal 暴露 ProjectPatch evidence，accept 后进入 execute_patch_as_transaction。

205 Imported Patch v2
  外部 ProjectPatch JSON 可导入、parse、validate、review、apply、report。

206 Thin LLM Patch Source v3
  LLM / mock source 只作为 ProjectPatchDocument JSON 输入源。
```

当前代码事实：

```text
rust/crates/editor_core/src/project_patch/model.rs
  PatchCapability 已有 Scene / Input / Asset / Prefab / Aui / Rule / Build。
  PatchOperation 当前只有 Scene / Input。

rust/crates/editor_core/src/project_patch/validator.rs
  当前只接受 Scene / Input capability。

rust/crates/editor_core/src/project_patch/applier.rs
  当前只把 Scene / Input operation 展开为 UiCommandPayload。

rust/crates/editor_core/src/project_patch/session.rs
  execute_patch_as_transaction 已有 validate / inverse / rollback snapshot / history。
```

可复用的 domain service：

```text
Asset:
  rust/crates/editor_core/src/asset_browser.rs
  rust/crates/editor_core/src/ai_image_generation.rs

Prefab:
  rust/crates/editor_core/src/services/prefab_service.rs
  rust/crates/editor_core/src/prefab_workflow.rs

AUI:
  rust/crates/editor_core/src/services/aui_service.rs
  rust/crates/editor_core/src/aui_authoring.rs

Rule:
  rust/crates/editor_core/src/services/rule_service.rs
  rust/crates/editor_core/src/rule_authoring.rs

Build:
  rust/crates/editor_core/src/services/build_service.rs
  rust/crates/editor_core/src/desktop_export.rs
```

当前缺口：

```text
ProjectPatch 无法真实表达 Asset / Prefab / AUI / Rule / Build operation。
Import diagnostics 仍把这些 capability 诚实报告为 unsupported / deferred。
LLM prompt 仍提示不要生成 Asset / Prefab / AUI / Rule / Build operation。
Patch rollback snapshot 当前偏内存状态，不足以覆盖项目文件写入的跨 domain 回滚。
Complex Shooter e2e 没有全域 patch artifact。
```

## 4.1 审查采纳与当前代码复核

已读取审查文档：

```text
其它AI审查目录/27-207-ProjectPatch-All-Domain-Capability方案审查.md
```

审查总体结论：

```text
207 方向正确，方案 A / A-min 可继续。
不需要推翻全域 ProjectPatch 方案。
```

采纳的修改：

```text
ProjectFileSnapshotSet 必须作为前置 Gate，先于任何会写项目文件的 domain apply。
施工顺序必须按“基础 schema / validator / snapshot -> domain capability -> applier/integration/e2e”分批推进。
施工文档必须明确记录：12 Gate 暂不合并，因为全域 patch 的回滚、domain diagnostics 和 e2e 需要独立证据。
```

当前代码复核结论：

```text
审查担心 Prefab / AUI / Rule / Build UiCommandPayload 可能缺失。
当前代码已确认这些 command surface 已存在：
  editor_ui_model/src/command.rs
  editor_core/src/session.rs
  editor_core/src/editor_command_registry.rs

Prefab:
  CreatePrefabFromSelection / OpenPrefabDocument / SetPrefabStageEntityField / SavePrefabDocument
  InstantiatePrefabInScene / ApplyPrefabOverrideToAsset / RevertPrefabOverride / ValidatePrefabReferences

AUI:
  CreateAuiDocument / OpenAuiDocument / AddAuiNode / SetAuiNodeField / SetAuiBindingPath
  SetAuiActionRef / ValidateAuiDocument / SaveAuiDocument / PreviewAuiOverlay

Rule:
  CreateRuleAsset / OpenRuleAsset / SetRuleTrigger / AddRuleStatement / UpdateRuleStatement
  RemoveRuleStatement / AddRuleOperation / UpdateRuleOperation / RemoveRuleOperation
  ValidateRuleAsset / BuildRuleArtifact

Build:
  ExportDesktopPackage / OpenBuildOutput / OpenBuildReport
```

因此：

```text
Prefab / AUI / Rule / Build command surface 不再是 207 的外部前置阻塞。
207 施工仍需在 Gate A 做 command surface sanity check，防止后续改动造成 route 断裂。
```

## 5. 方案 A-min 总体设计

链路保持不变：

```text
User Prompt / JSON / Fixture
  -> ProjectPatchDocument
  -> ProjectPatchImportService
  -> PatchValidator
  -> PatchReviewModel
  -> User Confirm
  -> EditorSession::execute_patch_as_transaction
  -> PatchApplyReport / PatchHistorySummary
  -> ProjectPatchAllDomainProductizationReport
```

核心变化：

```text
PatchOperation 新增 Asset / Prefab / Aui / Rule / Build。
PatchValidator 新增五个 domain validator。
PatchApplier 新增五个 domain operation 到正式 UiCommandPayload / service command 的映射。
ProjectPatchImportService 不再把 Asset / Prefab / AUI / Rule / Build 默认标为 unsupported。
execute_patch_as_transaction 增加项目文件快照回滚能力。
LLM prompt helper 更新 supported capabilities。
project_e2e_gate 增加 all-domain patch smoke。
```

关键边界：

```text
ProjectPatch 不直接写项目文件。
ProjectPatch 只能调用已经存在或本轮补齐的正式 EditorSession command / domain service。
每个 operation 都必须有 operation_id、depends_on、target_summary、kind。
每个写项目文件的 operation 都必须进入 ProjectFileSnapshotSet，用于失败回滚和历史 inverse。
Build operation 属于验证 / 导出动作，不表达玩法逻辑，不改变项目源资产。
```

## 6. v2 Operation Schema

### 6.1 AssetPatchOperation

第一版支持：

```text
Asset.RegisterExistingAsset
  path
  expected_kind

Asset.GenerateMockImageAsset
  prompt
  target_folder
  asset_name
  image_kind
  width
  height
  transparent_background

Asset.ValidateAssetBrowserIndex
  query_kind
```

映射：

```text
RegisterExistingAsset
  -> AssetBrowserIndex / AssetBrowserService pick / report

GenerateMockImageAsset
  -> AiImageGenerationRequest
  -> MockImageGenerationProvider
  -> AssetPipelineState::import_generated_image

ValidateAssetBrowserIndex
  -> AssetBrowserIndex::build
```

不做：

```text
不从任意外部路径复制文件。
不删除 / 替换资产。
不做影响分析不足的资源热更新。
不接真实图像生成 provider。
```

### 6.2 PrefabPatchOperation

第一版支持：

```text
Prefab.CreateFromSceneSelection
  scene_path
  root_entity_id
  prefab_id
  name
  replace_selection_with_instance

Prefab.OpenDocument
  path

Prefab.SetStageEntityField
  source_entity_id
  component_type
  field_path
  value

Prefab.SaveDocument
  path

Prefab.InstantiateInScene
  prefab_id
  parent_entity_id
  local_position

Prefab.ApplyOverrideToAsset
  instance_entity_id
  target_source_entity_id
  component_type
  field_path

Prefab.RevertOverride
  instance_entity_id
  target_source_entity_id
  component_type
  field_path

Prefab.ValidateReferences
  path
```

映射：

```text
UiCommandPayload::CreatePrefabFromSelection
UiCommandPayload::OpenPrefabDocument
UiCommandPayload::SetPrefabStageEntityField
UiCommandPayload::SavePrefabDocument
UiCommandPayload::InstantiatePrefabInScene
UiCommandPayload::ApplyPrefabOverrideToAsset
UiCommandPayload::RevertPrefabOverride
UiCommandPayload::ValidatePrefabReferences
```

不做：

```text
不直接修改 .prefab.json。
不新增 Prefab 专用玩法概念。
不做复杂 Prefab variant / nested Prefab。
```

### 6.3 AuiPatchOperation

第一版支持：

```text
Aui.CreateDocument
  path
  document_id
  width
  height

Aui.OpenDocument
  path

Aui.AddNode
  path
  parent_node_id
  node_id
  kind
  name
  rect

Aui.SetNodeField
  path
  node_id
  schema_path
  value

Aui.SetBindingPath
  path
  node_id
  target_field
  binding_id
  binding_path
  fallback

Aui.SetActionRef
  path
  node_id
  event
  action_id
  payload

Aui.ValidateDocument
  path

Aui.SaveDocument
  path

Aui.PreviewOverlay
  path
```

映射：

```text
UiCommandPayload::CreateAuiDocument
UiCommandPayload::OpenAuiDocument
UiCommandPayload::AddAuiNode
UiCommandPayload::SetAuiNodeField
UiCommandPayload::SetAuiBindingPath
UiCommandPayload::SetAuiActionRef
UiCommandPayload::ValidateAuiDocument
UiCommandPayload::SaveAuiDocument
UiCommandPayload::PreviewAuiOverlay
```

不做：

```text
不在 AUI Document 中保存运行时值。
不让 AUI binding 读取 ECS。
不新增复杂拖拽 Designer。
不把 UI 交互业务逻辑写进 AUI。
```

### 6.4 RulePatchOperation

第一版支持：

```text
Rule.CreateAsset
  path
  rule_id
  display_name

Rule.OpenAsset
  path

Rule.SetTrigger
  path
  trigger
  expected_ir_hash

Rule.AddStatement
  path
  statement
  expected_ir_hash

Rule.UpdateStatement
  path
  index
  statement
  expected_ir_hash

Rule.RemoveStatement
  path
  index
  expected_ir_hash

Rule.AddOperation
  path
  operation
  expected_ir_hash

Rule.UpdateOperation
  path
  index
  operation
  expected_ir_hash

Rule.RemoveOperation
  path
  index
  expected_ir_hash

Rule.ValidateAsset
  path

Rule.BuildArtifact
  path
```

映射：

```text
UiCommandPayload::CreateRuleAsset
UiCommandPayload::OpenRuleAsset
UiCommandPayload::SetRuleTrigger
UiCommandPayload::AddRuleStatement
UiCommandPayload::UpdateRuleStatement
UiCommandPayload::RemoveRuleStatement
UiCommandPayload::AddRuleOperation
UiCommandPayload::UpdateRuleOperation
UiCommandPayload::RemoveRuleOperation
UiCommandPayload::ValidateRuleAsset
UiCommandPayload::BuildRuleArtifact
```

不做：

```text
不扩大 IR 表达力。
不把 IR 变成 Lua / Blueprint 式脚本语言。
不允许 Rule 直接操作 Renderer / File / Network / ECS raw pointer。
不写 Player / Enemy / Bullet 专用 patch operation。
```

### 6.5 BuildPatchOperation

第一版支持：

```text
Build.ExportDesktopPackage
  profile_id

Build.OpenBuildReport

Build.OpenBuildOutput
```

映射：

```text
UiCommandPayload::ExportDesktopPackage
UiCommandPayload::OpenBuildReport
UiCommandPayload::OpenBuildOutput
```

边界：

```text
Build operation 是 patch apply 后的验证 / 导出动作。
Build 不表达项目玩法逻辑。
Build 不改变项目源资产，UndoPolicy 仍为 None。
Build 失败时 ProjectPatchApplyReport 必须能说明失败阶段和报告路径。
```

不做：

```text
不新增 installer / signing / store package。
不新增真实 Windows window screenshot gate。
不把 BuildProfile 任意编辑纳入本轮。
```

## 7. Rollback / Inverse 规则

当前 Scene/Input inverse 主要来自内存状态和已有 service。

全域 patch 需要新增：

```text
ProjectFileSnapshotSet
  root
  tracked_paths
  before_bytes
  existed_before
  restore_on_failure
```

规则：

```text
Prefab / AUI / Rule / Asset 写文件前必须 snapshot 对应 project-relative path。
创建新文件的 inverse 是删除该文件，前提是文件不存在于 before snapshot。
修改已有文件的 inverse 是恢复 before bytes。
Build 输出不进入源资产 inverse，只进入 artifacts/report。
如果 snapshot 无法建立，validator 或 apply 前阶段必须拒绝 patch。
```

失败行为：

```text
任一 ProjectMutation operation 失败：
  恢复 EditorSession 内存 snapshot。
  恢复 ProjectFileSnapshotSet。
  后续 operation 标记 Skipped。
  PatchApplyReport.status = Failed。

Build operation 失败：
  ProjectMutation 已成功时不自动删除源资产变更。
  Report 标记 build verification failed。
  next_actions 指向 open_build_report / fix_build_diagnostics。
```

说明：

```text
Build 是验证动作，不应因为导出失败而静默撤销用户刚确认的项目修改。
```

## 8. Validator 规则

全域 validator 必须新增：

```text
validate_asset
validate_prefab
validate_aui
validate_rule
validate_build
validate_project_file_snapshot_scope
```

公共规则：

```text
operation_id 必填且唯一。
depends_on 必须引用同 patch 内已存在 operation。
operation_count 继续受 MAX_OPERATION_COUNT 限制，必要时从 32 提升到 48，但必须写入测试。
所有 path 必须是项目相对路径，且 lexical normalize 后仍在 project_root 内。
所有写操作必须声明明确 target_summary。
所有 serde_json::Value payload 必须能 decode 成 domain service 已支持的数据结构。
禁止 engine.player / engine.enemy / engine.bullet 等玩法专用 API 字符串。
```

Domain 规则：

```text
Asset:
  只接受项目内路径。
  只接受已支持 kind。
  生成资产只用 mock/local deterministic provider。

Prefab:
  需要 open project。
  create/instantiate 需要 open scene。
  stage edit 需要 active stage 或同 patch 先 OpenDocument。

AUI:
  path 必须是 UI/ 或 Assets/UI/ 下 .aui.json。
  node kind / schema_path / binding target 必须在 AuiAuthoringService 支持范围内。

Rule:
  path 必须是 Rules/*.rule.json。
  trigger / statement / operation 必须能 decode 到当前 Rule IR 类型。
  expected_ir_hash 不匹配时拒绝。

Build:
  profile_id 第一版只允许 None 或 windows-dev。
  Build operation 必须在 mutation operation 之后执行。
```

## 9. Prompt / LLM 规则

206 中的 prompt helper 当前提示：

```text
Do not generate Asset, Prefab, AUI, Rule, or Build operations.
```

207 完成后改为：

```text
supported_project_patch_capabilities:
  Scene
  Input
  Asset
  Prefab
  AUI
  Rule
  Build
```

但必须同时提示：

```text
Only use documented ProjectPatchOperation schemas.
Do not write files directly.
Do not invent gameplay-specific engine APIs.
For complex feature changes, group operations by Feature Folder paths when possible.
Use Build.ExportDesktopPackage only as final verification.
```

Mock LLM source 也要新增 deterministic fixtures：

```text
prompt contains "rule" -> Rule.CreateAsset + Rule.AddOperation + Rule.ValidateAsset
prompt contains "aui" -> Aui.CreateDocument + Aui.AddNode + Aui.ValidateDocument
prompt contains "prefab" -> Prefab.InstantiateInScene 或 Prefab.ValidateReferences
prompt contains "asset" -> Asset.GenerateMockImageAsset + Asset.ValidateAssetBrowserIndex
prompt contains "build" -> Build.ExportDesktopPackage
prompt contains "all_domain" -> 生成覆盖 Asset / Prefab / AUI / Rule / Build 的综合 patch
```

## 10. 复杂打飞机验收场景

### 场景 A：生成资源并放入项目资产库

输入：

```text
Asset.GenerateMockImageAsset
Asset.ValidateAssetBrowserIndex
```

期望：

```text
生成 png 和 metadata。
AssetPipelineState 产生 ProjectAssetRecord。
AssetBrowserModel 能看到资源。
失败不会留下半截 report。
```

### 场景 B：创建 HUD AUI 文档

输入：

```text
Aui.CreateDocument
Aui.AddNode text score_label
Aui.SetBindingPath score_label text.text -> game.score
Aui.ValidateDocument
```

期望：

```text
AUI document 保存为 canonical shape。
Validation ok。
Report 可看到 node_count / binding_count。
```

### 场景 C：创建发射规则

输入：

```text
Rule.CreateAsset
Rule.SetTrigger action=fire
Rule.AddOperation InstantiatePrefab
Rule.ValidateAsset
Rule.BuildArtifact
```

期望：

```text
Rule asset 保存。
RuleAuthoringReport status Valid 或 Built。
diagnostics 有 human_explanation。
```

### 场景 D：Prefab 验证或实例化

输入：

```text
Prefab.InstantiateInScene
Prefab.ValidateReferences
```

期望：

```text
Scene 出现 Prefab instance component。
PrefabAuthoringReport 记录 instantiated_entity_ids。
```

### 场景 E：Build 验证

输入：

```text
Build.ExportDesktopPackage profile=windows-dev
Build.OpenBuildReport
```

期望：

```text
DesktopExportPipeline 运行。
PatchApplyReport operation_results 包含 build status。
失败时 next_actions 指向 build report。
```

### 场景 F：All-domain 综合 patch

输入：

```text
Asset.GenerateMockImageAsset
Aui.CreateDocument
Rule.CreateAsset
Prefab.ValidateReferences
Build.ExportDesktopPackage
```

期望：

```text
ProjectPatchImportResult parse/validation/review accepted。
用户确认后 apply。
每个 domain 都有 operation_result。
project_e2e_gate 生成 complex-shooter-all-domain-project-patch-report.json。
```

## 11. 可施工 Gate 建议

审查后施工顺序修正：

```text
第一批：基础能力
  Gate A Model / Schema
  Gate B Validator
  Gate C Project File Snapshot Rollback

第二批：全域 apply 能力
  Gate D Applier 映射
  Gate E Asset Capability
  Gate F Prefab Capability
  Gate G AUI Capability
  Gate H Rule Capability
  Gate I Build Capability

第三批：AI / walkthrough / e2e 集成
  Gate J LLM / Manual Walkthrough / AI Context
  Gate K Complex Shooter E2E
  Gate L 整体回归与文档同步
```

Gate 数量说明：

```text
保留 12 Gate，不按审查建议合并为 8 Gate。
原因是本系统同时引入全域 operation schema、文件级回滚、五个 domain apply 和 e2e 验证。
每个 Gate 需要独立测试证据，避免全域施工失败时无法定位是 schema、rollback、domain service 还是 e2e 集成问题。
施工文档可以按上述三批推进，但不能跳过任一 Gate 的测试。
```

### Gate A：Model / Schema

目标：

```text
PatchOperation 新增 Asset / Prefab / Aui / Rule / Build enum。
每个 operation 实现 operation_id / depends_on / kind / target_summary。
capabilities_for_operations 支持全域。
核验 Prefab / AUI / Rule / Build UiCommandPayload、EditorCommandRegistry、EditorSession route 已存在并可序列化。
```

测试：

```powershell
cargo test -p editor_core project_patch_model
cargo test -p editor_core editor_command_registry
cargo test -p editor_ui_model ai_panel
cargo test -p editor_ui_model workflow_command
```

### Gate B：Validator

目标：

```text
新增 validate_asset / validate_prefab / validate_aui / validate_rule / validate_build。
unsupported capability 逻辑改为全部支持。
非法路径 / 非法 payload / 缺 context 能结构化拒绝。
```

测试：

```powershell
cargo test -p editor_core project_patch_validator
cargo test -p editor_core project_patch_import
```

### Gate C：Project File Snapshot Rollback

目标：

```text
execute_patch_as_transaction 对项目文件写操作建立 ProjectFileSnapshotSet。
失败时恢复项目文件。
PatchHistory inverse 可恢复项目文件或删除新建文件。
文件 snapshot 不完整时，写文件 operation 必须拒绝。
```

测试：

```powershell
cargo test -p editor_core project_patch_rollback
cargo test -p editor_core project_patch
```

### Gate D：Applier 映射

目标：

```text
PatchApplier 可把全域 operation 映射到正式 UiCommandPayload 或 domain service command。
不新增 direct fs::write applier。
Applier 只能在 ProjectFileSnapshotSet 已可用后接入写文件 domain。
```

测试：

```powershell
cargo test -p editor_core project_patch_applier
cargo test -p editor_core ai_project_patch
```

### Gate E：Asset Capability

目标：

```text
Asset.RegisterExistingAsset / GenerateMockImageAsset / ValidateAssetBrowserIndex 可 apply。
```

测试：

```powershell
cargo test -p editor_core asset_browser
cargo test -p editor_core ai_image_generation
cargo test -p editor_core project_patch_asset
```

### Gate F：Prefab Capability

目标：

```text
Prefab operation 走 existing Prefab command/service。
```

测试：

```powershell
cargo test -p editor_core prefab
cargo test -p editor_core project_patch_prefab
```

### Gate G：AUI Capability

目标：

```text
AUI operation 走 AuiAuthoringService / AUI command。
```

测试：

```powershell
cargo test -p editor_core aui_authoring
cargo test -p engine_runtime aui
cargo test -p editor_core project_patch_aui
```

### Gate H：Rule Capability

目标：

```text
Rule operation 走 RuleAuthoringService / Rule command。
```

测试：

```powershell
cargo test -p editor_core rule_authoring
cargo test -p engine_runtime rule_compiler
cargo test -p editor_core project_patch_rule
```

### Gate I：Build Capability

目标：

```text
Build operation 走 DesktopExportPipeline。
Build 失败生成 report diagnostics，不作为 direct fs patch。
```

测试：

```powershell
cargo test -p editor_core build_export
cargo test -p editor_core project_patch_build
```

### Gate J：LLM / Manual Walkthrough / AI Context

目标：

```text
ThinLlmPatchSource prompt 和 mock fixture 支持全域 capability。
ManualWalkthrough / AuthoringAiContext 不再把 Asset / Prefab / AUI / Rule / Build 标为 deferred。
```

测试：

```powershell
cargo test -p editor_core llm_patch_source
cargo test -p editor_core authoring_workflow
cargo test -p editor_core manual_walkthrough
cargo test -p editor_ui_model manual_walkthrough
```

### Gate K：Complex Shooter E2E

目标：

```text
project_e2e_gate 生成 complex-shooter-all-domain-project-patch-report.json。
覆盖 Asset / Prefab / AUI / Rule / Build 至少各一个 operation。
```

测试：

```powershell
cargo test -p project_e2e_gate project_patch
cargo test -p project_e2e_gate
```

### Gate L：整体回归与文档同步

目标：

```text
同步 49 / 54 / 施工文档 README / 阶段完成记录。
归档施工文档。
```

测试：

```powershell
cargo fmt --check
cargo test -p editor_core project_patch
cargo test -p editor_ui_model
cargo test -p engine_runtime aui
cargo test -p engine_runtime rule_compiler
cargo test -p project_e2e_gate
```

## 12. 施工禁止事项

```text
禁止 ProjectPatch 直接 fs::write AUI / Prefab / Rule / Asset 文件。
禁止绕过 ProjectPatchImportService。
禁止绕过 PatchValidator。
禁止绕过 EditorSession::execute_patch_as_transaction。
禁止 silent ignore unsupported / invalid operation。
禁止为了复杂打飞机加入 Player / Enemy / Bullet / Score / Weapon 专用 operation。
禁止在本轮新增 Agent Planner / Repair Loop。
禁止把真实 LLM provider / 真实图像 provider 作为默认测试依赖。
禁止 Build operation 修改项目源资产。
禁止在文件 snapshot 不完整时继续执行写文件 operation。
```

## 13. 方案自审

### 13.1 是否符合用户选择

通过。

```text
本方案采用方案 A：一次性扩 Asset / Prefab / AUI / Rule / Build capability。
```

### 13.2 是否增加过多结构

可控。

```text
没有新增独立 Router / Agent / Provider / Architecture Guard。
只扩 ProjectPatch 既有 schema、validator、applier、transaction、report。
ProjectFileSnapshotSet 是 patch transaction 的必要回滚能力，不是新的架构层。
```

### 13.3 是否符合 AI-first

通过。

```text
所有新增 domain 都是 schema-first operation。
导入、校验、审阅、确认、执行、报告链路不变。
AI 可以读 operation schema 和 diagnostics，不需要猜 UI 状态。
```

### 13.4 是否支撑复杂项目

通过。

```text
复杂打飞机和自走棋都需要一次 patch 同时修改资源、Prefab、UI、规则并触发构建验证。
全域 ProjectPatch 比单域推进更接近真实项目编辑。
```

### 13.5 主要风险

风险一：

```text
范围大，施工容易膨胀。
```

处理：

```text
采用 A-min，只接已有 service 支撑的最小操作集。
施工按 Gate 分域验证，但归属同一个系统。
```

风险二：

```text
文件回滚不完整会造成半应用状态。
```

处理：

```text
ProjectFileSnapshotSet 是前置 Gate C。
写文件 operation 没有 snapshot 就拒绝。
```

风险三：

```text
Build operation 和项目修改 operation 的事务语义不同。
```

处理：

```text
Build 作为 post-apply verification。
Build 失败不自动撤销已确认的项目资产变更，但必须在 report 中标记 failed / next_actions。
```

风险四：

```text
LLM 可能生成过大的全域 patch。
```

处理：

```text
保留 MAX_OPERATION_COUNT。
超限要求拆 patch。
Prompt 明确只使用 documented operation schema。
```

## 14. 最终结论

采用：

```text
方案 A：ProjectPatch All-Domain Capability v2
```

正式判断：

```text
下一步不再继续只让 ProjectPatch 支持 Scene / Input。
也不再单独排队 Rule / AUI / Prefab 一个一个接。
本轮将 Asset / Prefab / AUI / Rule / Build 一次纳入 ProjectPatch 全域能力，但每个 domain 只做已有正式 service 能支撑的 A-min 操作集。
```

下一步：

```text
如果用户确认进入施工，应基于本文生成自动化施工文档。
施工文档必须先自审，再按 Gate A-L 实施和测试。
```
