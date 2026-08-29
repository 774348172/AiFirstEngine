# 202-AI Project Patch Entry / Project Patch Productization v1 方案

## 1. 系统是什么

本系统正式命名为：

```text
AI Project Patch Entry / Project Patch Productization v1
```

一句话说明：

```text
它是 AI 修改项目的安全事务入口。
```

它把用户自然语言、AI 计划或导入的结构化修改，收敛成可验证、可审阅、可回滚、可测试的 `ProjectPatchDocument`，再通过正式编辑器命令和事务应用到项目。

目标链路：

```text
Natural Language / Imported Patch / Test
  -> ProjectPatchDocument
  -> Validate
  -> Review
  -> Confirm
  -> Apply through EditorSession / CommandTransaction
  -> PatchApplyReport / PatchHistory
  -> Save / Build / E2E Report evidence
```

它不是：

```text
不是让 AI 直接写 JSON 文件。
不是让 AI 模拟点击 UI。
不是第二套编辑器事务系统。
不是完整 LLM agent。
不是 Prefab / AUI / Rule / Asset 全域 patch 一次做完。
```

## 2. 在其它引擎中的对标

本节按当前 skill 规则，不只参考官方文档，也读取本项目已有源码参考文档，并抽查本地源码命中点：

```text
框架设计/Unity源码参考/AI-Project-Patch-EditorTransaction源码参考.md
框架设计/UE源码参考/AI-Project-Patch-EditorTransaction源码参考.md
框架设计/Godot源码参考/AI-Project-Patch-EditorUndoRedo源码参考.md
框架设计/Bevy源码参考/AI-Project-Patch-Reflect-DynamicWorld源码参考.md
```

### 2.1 Unity

Unity 对标的是：

```text
Editor Tool / Inspector
  -> SerializedObject / SerializedProperty
  -> Undo / Dirty
  -> ApplyModifiedProperties
```

官方参考：

```text
https://docs.unity3d.com/ScriptReference/Undo.RecordObject.html
https://docs.unity3d.com/ScriptReference/SerializedObject.ApplyModifiedProperties.html
```

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector\TransformInspector.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\UIElements\Controls\PropertyField.cs
```

源码命中：

```text
TransformInspector.cs:
  serializedObject.FindProperty("m_LocalPosition")
  serializedObject.ApplyModifiedProperties()
  Undo.SetCurrentGroupName(...)

PropertyField.cs:
  serializedProperty.serializedObject.ApplyModifiedProperties()
```

结论：

```text
Unity 的成熟经验是：编辑器修改要进入 Undo / SerializedObject / Apply 链路，不能随便改文件。
```

我们借鉴：

```text
ProjectPatch 必须是结构化字段 / 对象修改，并进入正式事务和 dirty/save/report。
```

不照搬：

```text
不采用 Unity 的隐式 C# 对象真相。
不让 AI 直接生成一串 Editor 脚本来改项目。
```

### 2.2 Unreal Engine

Unreal 对标的是：

```text
Editor Command / Tool
  -> FScopedTransaction
  -> UObject Modify / Package Dirty
  -> Details / Asset / Save
```

官方参考：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Editor/UnrealEd/FScopedTransaction
```

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\EditorActor.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UMGEditor\Private\WidgetBlueprintEditorUtils.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UMGEditor\Private\WidgetBlueprintOperationUtils.cpp
```

源码命中：

```text
EditorActor.cpp:
  FScopedTransaction Transaction(...)
  Actor->Modify()
  Actor->MarkPackageDirty()

WidgetBlueprintEditorUtils.cpp / WidgetBlueprintOperationUtils.cpp:
  FScopedTransaction Transaction(...)
  WidgetBlueprint->WidgetTree->Modify()
  Parent->Modify()
  BP->MarkPackageDirty()
```

结论：

```text
UE 的成熟经验是：编辑器动作有明确事务边界，工具命令和对象修改分离。
```

我们借鉴：

```text
ProjectPatch 是意图层；EditorSession / CommandTransaction / Domain Service 是执行层。
```

不照搬：

```text
不引入 UObject / Package / Slate 体系。
不把 AI patch 变成 C++ 编辑器插件脚本。
```

### 2.3 Godot

Godot 对标的是：

```text
EditorUndoRedoManager
  -> create_action
  -> add_do_method / add_undo_method
  -> commit_action
```

官方参考：

```text
https://docs.godotengine.org/en/stable/classes/class_editorundoredomanager.html
```

源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\editor\inspector\editor_inspector.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\scene\3d\node_3d_editor_viewport.cpp
```

源码命中：

```text
editor_inspector.cpp:
  undo_redo->create_action(...)
  undo_redo->add_do_method(...)
  undo_redo->add_undo_method(...)
  undo_redo->commit_action()

node_3d_editor_viewport.cpp:
  undo_redo->add_do_method(p_parent, "add_child", ...)
  undo_redo->add_undo_method(p_parent, "remove_child", ...)
  undo_redo->commit_action()
```

结论：

```text
Godot 的成熟经验是：编辑动作应该能说明 do / undo，提交时才生效，并且能标记 unsaved。
```

我们借鉴：

```text
PatchOperation 必须能验证、应用，并在可能时生成 inverse patch。
```

不照搬：

```text
不采用 string method call 作为长期 patch 真相。
```

### 2.4 Bevy

Bevy 对标的是：

```text
Reflect / TypeRegistry / DynamicScene
```

官方参考：

```text
https://docs.rs/bevy_scene/latest/bevy_scene/struct.DynamicScene.html
https://docs.rs/bevy_reflect/latest/bevy_reflect/struct.TypeRegistry.html
```

源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app\src\app.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_asset\src\reflect.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_world_serialization\src\world_asset_spawner.rs
```

源码命中：

```text
app.rs:
  register_type<T>()
  register_type_data<T, D>()

bevy_asset/src/reflect.rs:
  ReflectAsset
  ReflectHandle

world_asset_spawner.rs:
  unregistered component/resource/type diagnostics
  Commands deferred spawn examples
```

结论：

```text
Bevy 的参考价值在 schema / type registry / dynamic scene validation，而不是完整编辑器产品链路。
```

我们借鉴：

```text
ProjectPatch validator 应能检查 type、field path、asset ref、component data。
```

不照搬：

```text
不把 Runtime ECS World 当编辑器项目真相。
```

## 3. 在本引擎中的作用

当前主线目标是：

```text
用户 / AI 能从编辑器制作复杂打飞机项目，并导出 Windows 可玩。
```

这个目标已经具备：

```text
Project Authoring Workspace
AuthoringWorkflow / Manual Walkthrough Coverage
Scene / Input / Rule / AUI / RuntimePackage / Export / Player E2E 多段 C-min
Game UI MVVM ReadModel / ProjectUiStateSnapshot Producer v1
```

但仍缺：

```text
AI patch 生成闭环仍未产品化。
```

也就是说，现在项目已经有很多真实 domain 命令和 report，但 AI 还不能稳定地作为“项目修改者”进入同一条正式链路。`ProjectPatch Productization v1` 要补的是：

```text
让 ProjectPatch 从隐藏的 headless 底座，变成用户和 AI 都能审阅、确认、执行、回滚、测试的产品化入口。
```

## 4. 当前真实基线

### 4.1 已有方案和审查

已有正式方案：

```text
181-M16-AI-Project-Patch-Entry-C-min方案.md
```

已有 AI 审查：

```text
其它AI审查目录/09-M16 AI Project Patch Entry 方案审查.md
```

已有施工文档：

```text
施工文档/182-当前可自动化施工文档-M16-AI-Project-Patch-Entry-C-min.md
```

重要修正：

```text
181 / 182 的 C-min 底座已经基本实现。
本轮不能再把 M16 当成从零施工。
本轮应把它升级为 Project Patch Productization v1。
```

### 4.2 当前代码已具备

代码位置：

```text
rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/project_patch/validator.rs
rust/crates/editor_core/src/project_patch/applier.rs
rust/crates/editor_core/src/project_patch/history.rs
rust/crates/editor_core/src/project_patch/session.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/tests/project_patch_tests.rs
rust/crates/editor_core/src/tests/ai_service_tests.rs
```

已具备能力：

```text
ProjectPatchDocument
PatchOperation
ScenePatchOperation
InputPatchOperation
PatchValidator
PatchReviewModel
PatchApplier
PatchApplyReport
PatchHistory
EditorSession::execute_patch_as_transaction
revert_last_patch_for_test
AI create prompt 内部先生成 ProjectPatch，再展开为 UiCommandPayload
headless tests 覆盖 Scene / Input patch apply / reject / revert
```

当前测试已证明：

```text
project_patch_model_records_scene_capability
project_patch_applier_expands_scene_operation_to_ui_command
project_patch_validator_rejects_missing_scene_entity
project_patch_transaction_creates_scene_entity
project_patch_transaction_rejects_missing_entity_without_mutation
project_patch_input_action_and_binding_apply
project_patch_validator_rejects_duplicate_input_action
project_patch_history_revert_removes_created_entity
ai_project_patch_create_prompt_is_project_patch_planned
```

### 4.3 当前仍未产品化

当前缺口：

```text
AI Panel 对外仍是 AiProposedCommand + UiCommandPayload，不是 ProjectPatch proposal。
PatchReviewModel 没有进入 editor_ui_model 的 AI review surface。
PatchApplyReport / PatchHistory 没有成为用户可见或 e2e artifact。
ProjectPatch 没有进入 manual walkthrough coverage 的可执行证据。
Complex Shooter E2E 没有 patch smoke report。
Prefab / Asset / AUI / Rule patch 仍没有真实应用边界，不能伪装已支持。
真实 LLM structured output 仍未接入。
182 施工文档写有完成记录，但未像后续系统一样形成完整阶段归档摘要。
```

## 5. 方案对比

### 5.1 方案 A：维持隐藏 ProjectPatch C-min

做法：

```text
保持当前 editor_core headless ProjectPatch。
AI Panel 继续输出 UiCommandPayload。
不新增产品化 report / review / e2e。
```

优点：

```text
改动最少。
当前测试已经能跑。
```

缺点：

```text
用户看不到 ProjectPatch。
AI 也无法稳定根据 patch report 修复失败。
复杂项目无法证明 AI 修改项目进入正式闭环。
仍然像“AI 辅助点击命令”，不是“AI 修改项目资产”。
```

结论：

```text
不采用。
```

### 5.2 方案 B：直接扩成全域 ProjectPatch

做法：

```text
一次补 Asset / Prefab / AUI / Rule / Build patch。
AI proposal 全面改成 ProjectPatch。
真实 LLM structured output 同时接入。
```

优点：

```text
长期形态最完整。
一旦成功，AI 修改复杂项目的表达力最强。
```

缺点：

```text
范围过大。
AUI / Rule / Prefab 当前各自还在产品化阶段，强行进入 patch 会制造假支持。
真实 LLM 会把 schema / validation / product surface 的问题混在一起，不利于定位。
```

结论：

```text
不采用本轮。
作为后续 v2 / v3 方向保留。
```

### 5.3 方案 C-min：产品化现有 ProjectPatch C-min

做法：

```text
保留当前 Scene / Input 真实可执行边界。
把 ProjectPatch 变成 AI proposal / review / report / e2e 可见资产。
把 unsupported domain 诚实写进 diagnostics / next_actions。
先不接真实 LLM，只产品化 deterministic planner / imported patch / test patch。
```

优点：

```text
AI 适配性最强：schema-first、reviewable、diagnostic-driven、可回滚。
复杂项目适配强：先证明 ProjectPatch 修改真实项目，不写打飞机专用 API。
效率最好：复用现有 editor_core project_patch，不重建事务系统。
```

缺点：

```text
第一版仍只能真实应用 Scene / Input。
Prefab / AUI / Rule / Asset 需要后续 domain 成熟后逐步接入。
真实 LLM 仍是后续步骤。
```

结论：

```text
采用。
```

## 6. 推荐方案

采用：

```text
方案 C-min：产品化现有 ProjectPatch C-min
```

过滤优先级：

### 6.1 AI 适配性

通过。

```text
ProjectPatchDocument 是 AI 可生成、可校验、可解释的 schema。
PatchValidationReport / PatchApplyReport / PatchHistory 能让 AI 根据结构化结果修复。
Review surface 能让用户和 AI 看到同一个修改计划。
```

### 6.2 复杂项目适配与可维护

通过。

```text
Scene / Input 是复杂打飞机最基础的真实 authoring domain。
Patch 不包含 Player / Enemy / Bullet / Health / Score 等项目专用 API。
后续 Prefab / AUI / Rule / Asset 可以按 capability 分阶段接入，不推翻现有链路。
```

### 6.3 效率

通过。

```text
复用 editor_core 已有 project_patch。
复用 EditorSession / CommandTransaction / UiCommandPayload / Domain Service。
先产品化报告和入口，不重写编辑器。
```

## 7. v1 正式边界

### 7.1 v1 真实落地

```text
ProjectPatch proposal model 接入 AI review surface。
PatchReviewModel 进入 editor_ui_model 或 AI Panel 可序列化模型。
AI Panel create prompt 不再只暴露 UiCommandPayload，而要能暴露 patch review evidence。
Accept ProjectPatch 后走 EditorSession::execute_patch_as_transaction。
PatchApplyReport / PatchHistory 进入 workspace / report / AI context 摘要。
ManualWalkthroughCoverage 能识别 ai_project_patch_entry 的当前可执行能力。
project_e2e_gate 生成 complex shooter ProjectPatch smoke artifact。
文档同步 49 / 54 / 阶段完成记录。
```

### 7.2 v1 只允许真实应用

```text
Scene patch:
  CreateEntity
  RenameEntity
  SetTransform
  SetComponentField
  PlaceAssetIntoScene

Input patch:
  AddInputAction
  AddInputBinding
  SetInputBindingDevicePath
```

说明：

```text
DeleteEntity / RemoveInputAction / RemoveInputBinding 可作为 inverse / 内部能力存在。
如果 forward patch 无法生成可靠 inverse，应在 validate / inverse 阶段拒绝。
```

### 7.3 v1 不做

```text
真实 LLM 接入。
自由文本脚本 patch。
AI 直接写文件。
Asset / Prefab / AUI / Rule patch 真实应用。
完整 visual diff viewer。
完整多 patch merge / conflict resolution。
Patch marketplace / plugin 扩展。
打飞机专用 patch operation。
```

### 7.4 unsupported domain 规则

如果 AI 或导入 patch 请求以下 capability：

```text
Asset
Prefab
Aui
Rule
Build
```

v1 必须：

```text
在 Validate 阶段拒绝或标记 unsupported。
输出清晰 diagnostic。
给出 next_action，例如 aui_authoring_productization / prefab_patch_capability_v2 / rule_patch_capability_v2。
不能偷偷降级成文件写入或 UI command 猜测。
```

## 8. 产品化数据结构建议

### 8.1 AiProjectPatchProposal

建议新增或等价表达：

```text
AiProjectPatchProposal
  proposal_id
  patch
  validation
  review
  review_state
```

如果为了降低改动，第一版也可以保留 `AiProposedCommand`，但必须在 AI Panel model 或 report 中额外暴露：

```text
project_patch_id
patch_title
touched_domains
operation_count
validation_status
diagnostics
requires_confirmation
```

长期应收敛到：

```text
AI proposal 的真相是 ProjectPatch，不是 UiCommandPayload。
```

### 8.2 ProjectPatchProductizationReport

建议新增：

```text
ProjectPatchProductizationReport
  schema_version
  scenario_id
  status
  patch_id
  source
  validation
  review
  apply_report
  history_summary
  supported_capabilities
  unsupported_capabilities
  next_actions
  artifacts
```

用途：

```text
给用户看：AI 改了什么、是否成功、失败怎么修。
给 AI 看：下一次 patch 应该修哪个 operation / field / capability。
给测试看：复杂项目是否真的被 patch 修改过。
```

### 8.3 PatchHistorySummary

建议新增可序列化摘要：

```text
PatchHistorySummary
  applied_count
  last_patch_id
  last_status
  reversible_count
  diagnostics
```

不要求 v1 做完整历史面板，但要能进入 report / AI context。

## 9. 与现有系统关系

### 9.1 与 EditorCommandFramework

规则：

```text
ProjectPatch 是意图层。
UiCommandPayload 是执行层。
EditorSession / CommandTransaction 是正式执行通道。
```

禁止：

```text
Patch handler 绕过 execute_command。
AI 直接改 EditorSession 内部字段。
AI 直接改 Runtime World。
```

### 9.2 与 Manual Walkthrough

关系：

```text
Manual Walkthrough 负责说明用户手动路径缺什么。
ProjectPatch Productization 负责让 AI 以结构化 patch 修改项目。
```

二者共享：

```text
domain command
coverage report
next_actions
```

### 9.3 与 Rule / AUI / Prefab

规则：

```text
Rule / AUI / Prefab 的真实编辑能力不足时，ProjectPatch 不替它们假装完成。
```

v1 只能做：

```text
把这些 domain 作为 unsupported capability 报告出来。
等对应 domain 产品化后，再讨论 ProjectPatch capability v2。
```

### 9.4 与真实 LLM

规则：

```text
先产品化 deterministic ProjectPatch 链路。
再接真实 LLM structured output。
```

原因：

```text
如果第一版同时接 LLM，失败时无法判断是 schema、validator、review、apply、UI 还是模型输出的问题。
```

## 10. 复杂打飞机验收场景

v1 至少要能证明：

### 场景 A：AI / Test Patch 创建通用场景实体

```text
ProjectPatch:
  Scene.CreateEntity name="Patch Spawn"

期望：
  Validate accepted
  Review shows Scene domain / 1 operation
  Apply committed
  Scene hierarchy 出现实体
  PatchHistory 记录 inverse
```

### 场景 B：AI / Test Patch 添加输入动作和绑定

```text
ProjectPatch:
  Input.AddInputAction action.patch_fire
  Input.AddInputBinding keyboard/F

期望：
  Validate accepted
  Apply committed
  Input mapping asset 被正式 domain service 修改
```

### 场景 C：非法 patch 被拒绝且不修改项目

```text
ProjectPatch:
  Scene.SetTransform entity_id="missing"

期望：
  Validate rejected
  Diagnostic 指向 missing entity
  No mutation
```

### 场景 D：unsupported AUI / Rule patch 诚实报告

```text
ProjectPatch:
  required_capabilities=[Aui]

期望：
  Validate rejected 或 unsupported
  Diagnostic 清楚说明 v1 不支持 AUI patch apply
  next_action 指向 AUI authoring / patch capability 后续系统
```

### 场景 E：Complex Shooter ProjectPatch smoke report

```text
samples/complex_shooter_project
  -> open project
  -> generate deterministic ProjectPatch
  -> validate / review / apply
  -> write project-patch-productization-report.json
```

期望：

```text
report.status = pass 或 partial
Scene/Input patch evidence 存在
unsupported domains 不伪装 pass
```

## 11. 可施工 Gate 建议

如果进入施工，建议分 Gate：

### Gate A：Patch Review Product Surface

目标：

```text
让 AI Panel / editor_ui_model 能序列化 ProjectPatch review evidence。
```

测试：

```powershell
cargo test -p editor_ui_model ai_panel
cargo test -p editor_core ai_project_patch
```

### Gate B：Accept ProjectPatch Proposal

目标：

```text
AI accept 路径能从 ProjectPatch proposal 进入 execute_patch_as_transaction。
```

测试：

```powershell
cargo test -p editor_core project_patch
cargo test -p editor_core ai_project_patch
```

### Gate C：Patch Productization Report

目标：

```text
新增 ProjectPatchProductizationReport / PatchHistorySummary。
```

测试：

```powershell
cargo test -p editor_core project_patch
```

### Gate D：Manual Walkthrough / AI Context 接入

目标：

```text
coverage / ai context 能看到 ai_project_patch_entry 当前能力和 next_actions。
```

测试：

```powershell
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core manual_walkthrough
cargo test -p editor_core authoring_workflow
```

### Gate E：Complex Shooter E2E Smoke

目标：

```text
project_e2e_gate 生成 ProjectPatch smoke artifact。
```

测试：

```powershell
cargo test -p project_e2e_gate project_patch
cargo test -p project_e2e_gate
```

### Gate F：文档同步和整体回归

目标：

```text
更新 49 / 54 / 施工文档 README / 阶段完成记录。
```

测试：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core project_patch
cargo test -p project_e2e_gate
```

## 12. 施工时禁止事项

```text
禁止绕过 ProjectPatchValidator。
禁止绕过 EditorSession / CommandTransaction。
禁止把 AI proposal 只保留成 UiCommandPayload 而没有 patch evidence。
禁止为了 complex shooter 写 Player / Enemy / Bullet 等 patch operation。
禁止把 unsupported AUI / Rule / Prefab patch 静默忽略。
禁止第一版接真实 LLM 后再调试 patch 链路。
```

## 13. 方案自审

### 13.1 是否合乎用户目标

通过。

```text
用户要继续推进 AI Project Patch Entry。
本方案没有重复做已完成 C-min，而是转向当前真实缺口：产品化入口、审阅、报告和复杂项目证据。
```

### 13.2 是否合乎项目 skill

通过。

```text
已先说明系统用途、其它引擎对标、本引擎作用。
已参考 Unity / Unreal / Godot / Bevy 的正式编辑修改链路，并读取本项目已有源码参考文档与本地源码关键命中点。
已读取本项目 181 / 182 / 09 审查 / 49 / 54 / 当前 project_patch 代码。
已给出 A/B/C 三个方案，并按 AI 适配性、复杂项目维护、效率过滤。
```

### 13.3 是否合乎当前架构规则

通过。

```text
不把 AI 修改项目变成直接写文件。
不新增第二套事务。
不把 IR 当脚本语言。
不把项目玩法写进引擎 Core。
不把 AUI / Rule / Prefab 的未完成能力伪装成 patch 已支持。
```

### 13.4 是否方便实现

通过。

```text
editor_core 已有 ProjectPatch 底座。
本轮主要补 editor_ui_model / editor_core report / project_e2e_gate evidence。
可以按 Gate 小步验证。
```

### 13.5 主要风险

风险一：

```text
AI Panel 模型改动过大，影响现有 UI command review。
```

处理：

```text
第一版可以兼容 AiProposedCommand，但必须额外暴露 ProjectPatch review evidence。
```

风险二：

```text
用户误以为 v1 已支持全域 patch。
```

处理：

```text
Scene/Input 是真实 apply；AUI/Rule/Prefab/Asset 必须在 report 中显示 unsupported / next_actions。
```

风险三：

```text
PatchApplyReport 成为重复 CommandResult 的另一套报告。
```

处理：

```text
PatchApplyReport 只聚合 patch operation 级结果；command 级诊断仍来自现有 CommandResult。
```

## 14. 最终结论

采用：

```text
方案 C-min：ProjectPatch Productization v1
```

正式判断：

```text
M16 C-min 底座已存在。
下一步不是重做 ProjectPatch，而是把它产品化为 AI / 用户可审阅、可确认、可报告、可测试的项目修改入口。
```

下一步：

```text
如果用户确认进入施工，基于本文生成新的可自动化施工文档。
施工文档必须先自审，再按 Gate A-F 实施和测试。
```
