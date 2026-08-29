# 205-AI Project Patch Structured Output / Imported Patch Productization v2 方案

## 1. 系统是什么

本系统正式命名为：

```text
AI Project Patch Structured Output / Imported Patch Productization v2
```

一句话说明：

```text
它让 AI 或外部工具产出的结构化 ProjectPatchDocument，可以被导入、解析、校验、审阅、确认、执行、回滚和报告。
```

它补的是 202 完成后的真实缺口：

```text
202 已经证明 ProjectPatch v1 可以在 editor_core 内部生成、审阅、执行 Scene/Input patch。
但外部结构化 patch 还没有正式入口。
AI Panel 也还没有把“外部 JSON / structured output”作为 ProjectPatch proposal 输入。
```

目标链路：

```text
Imported JSON / AI structured output / Test fixture
  -> ProjectPatchImportRequest
  -> Parse / Schema Check
  -> Capability Check
  -> PatchValidator
  -> PatchReviewModel
  -> User Confirm
  -> EditorSession::execute_patch_as_transaction
  -> PatchApplyReport / PatchHistorySummary
  -> ProjectPatchImportProductizationReport / E2E artifact
```

本系统不是：

```text
不是真实 LLM provider 接入。
不是全域 ProjectPatch 一次完成。
不是让 AI 直接写项目文件。
不是让 AI 生成 Rust / C# / Lua / Blueprint 脚本。
不是第二套编辑器事务系统。
```

## 2. 在其它成熟引擎 / 工具中的对标

成熟引擎没有完全等价的“AI structured patch import”，但它们有共同原则：

```text
外部或工具产生的编辑意图，必须进入结构化修改、事务、Undo/Redo、Dirty/Save 和诊断链路。
```

### 2.1 Unity

对标：

```text
Editor tool / Inspector
  -> SerializedObject / SerializedProperty
  -> Undo.RecordObject
  -> SerializedObject.ApplyModifiedProperties
  -> Dirty / Save
```

官方参考：

```text
https://docs.unity3d.com/ScriptReference/Undo.RecordObject.html
https://docs.unity3d.com/ScriptReference/SerializedObject.ApplyModifiedProperties.html
```

源码参考：

```text
框架设计/Unity源码参考/AI-Project-Patch-EditorTransaction源码参考.md
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector\TransformInspector.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\UIElements\Controls\PropertyField.cs
```

可学习点：

```text
字段修改必须先落到可验证的对象 / 属性路径。
Apply 前后有明确 Undo / Dirty 语义。
工具入口不应绕过正式编辑链路。
```

不照搬：

```text
不让 AI 生成 Unity Editor 脚本。
不采用 Unity 的隐式 SerializedObject 作为本项目 patch 真相。
```

### 2.2 Unreal Engine

对标：

```text
Editor command / tool
  -> FScopedTransaction
  -> UObject::Modify
  -> mutation
  -> MarkPackageDirty / Save
```

官方参考：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Editor/UnrealEd/FScopedTransaction
```

源码参考：

```text
框架设计/UE源码参考/AI-Project-Patch-EditorTransaction源码参考.md
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\EditorActor.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UMGEditor\Private\WidgetBlueprintEditorUtils.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UMGEditor\Private\WidgetBlueprintOperationUtils.cpp
```

可学习点：

```text
命令入口和真实事务分离。
一次编辑动作必须有显式事务边界。
复杂 domain 可以有多个 handler，但都不能绕过 transaction。
```

不照搬：

```text
不引入 UObject / Package / Slate / BlueprintGeneratedClass 体系。
不把 ProjectPatch 变成 C++ 编辑器插件脚本。
```

### 2.3 Godot

对标：

```text
EditorUndoRedoManager
  -> create_action
  -> add_do_method / add_undo_method / add_do_property / add_undo_property
  -> commit_action
```

官方参考：

```text
https://docs.godotengine.org/en/stable/classes/class_editorundoredomanager.html
```

源码参考：

```text
框架设计/Godot源码参考/AI-Project-Patch-EditorUndoRedo源码参考.md
<GODOT_SOURCE>\godot-master\godot-master\editor\inspector\editor_inspector.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\scene\3d\node_3d_editor_viewport.cpp
```

可学习点：

```text
编辑操作可以表达为一组可审阅的 do / undo 操作。
commit 前先组织完整动作。
mark_unsaved / history id 是编辑器长期维护必须关心的边界。
```

不照搬：

```text
不让 AI 自由生成 Object method name 和参数。
不采用字符串反射调用作为 patch 真相。
```

### 2.4 Bevy

对标：

```text
Reflect / TypeRegistry / DynamicScene / SceneSpawner
```

官方参考：

```text
https://docs.rs/bevy/latest/bevy/prelude/struct.DynamicScene.html
```

源码参考：

```text
框架设计/Bevy源码参考/AI-Project-Patch-Reflect-DynamicWorld源码参考.md
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app\src\app.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_asset\src\reflect.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_world_serialization\src\world_asset_spawner.rs
```

可学习点：

```text
结构化场景 / 组件数据必须依赖类型注册和 schema。
未知类型、未注册组件、非法字段路径必须在 validate 阶段报告。
```

不照搬：

```text
不把 runtime ECS World 当编辑器项目真相。
不把 Bevy Commands 当编辑器事务系统。
```

## 3. 在本引擎主线中的作用

当前主线目标是：

```text
用户 / AI 能从编辑器制作复杂打飞机项目，并导出 Windows 可玩。
```

已完成基础：

```text
202 ProjectPatch v1：
  ProjectPatchDocument / PatchValidator / PatchApplier / PatchApplyReport / PatchHistorySummary
  Scene/Input patch 可以通过 EditorSession::execute_patch_as_transaction 执行。

203 Prefab Authoring Productization v1：
  Prefab 已有正式 command / service / report。

204 AUI Document Authoring Productization v1：
  AUI Document 已有正式 command / service / report。
```

当前缺口：

```text
外部 ProjectPatch JSON / AI structured output 还不能作为正式输入导入。
AI Panel 的提案仍主要来自内部 mock planner。
没有 ProjectPatchImportReport 说明 parse/schema/capability/validation/review/apply 的每一步结果。
Complex Shooter 没有 imported patch smoke artifact。
```

205 要把 ProjectPatch 从“内部 proposal evidence”推进为：

```text
AI / 外部工具 / 测试夹具都能提交的结构化工程修改入口。
```

## 4. 当前本项目基线

已有正式方案和完成记录：

```text
202-AI-Project-Patch-Entry-Project-Patch-Productization-v1方案.md
阶段完成记录/2026-07-05-AI-Project-Patch-Entry-Project-Patch-Productization-v1/00-总览.md
```

已有审查结论：

```text
其它AI审查目录/09-M16 AI Project Patch Entry 方案审查.md
```

09 审查的关键判断仍然有效：

```text
ProjectPatch 不是从零系统。
第一版先验证 patch -> validate -> apply -> transaction -> history。
不要第一版就接真实 LLM，否则模型输出不稳定会掩盖链路问题。
LLM 输出应作为后续 structured output provider 接入。
```

当前代码已具备：

```text
rust/crates/editor_core/src/project_patch/model.rs
  ProjectPatchDocument
  PatchOperation
  ScenePatchOperation
  InputPatchOperation
  PatchReviewModel
  PatchApplyReport
  ProjectPatchProductizationReport

rust/crates/editor_core/src/project_patch/validator.rs
  PatchValidator

rust/crates/editor_core/src/project_patch/applier.rs
  PatchApplier -> UiCommandPayload

rust/crates/editor_core/src/project_patch/session.rs
  EditorSession::execute_patch_as_transaction
  patch rollback snapshot
  inverse patch

rust/crates/editor_core/src/project_patch/history.rs
  PatchHistory

rust/crates/editor_core/src/services/ai_service.rs
  mock planner 内部生成 ProjectPatch proposal evidence

rust/crates/project_e2e_gate/src/project_patch.rs
  complex shooter ProjectPatch productization smoke
```

当前缺口：

```text
ProjectPatchDocument 没有正式 import/parse service。
没有 ImportProjectPatch / PreviewImportedProjectPatch / ApplyImportedProjectPatch command。
parse error / schema error / capability unsupported / validation rejected 尚未形成独立 import report。
AiProposedCommand 仍需要一个 UiCommandPayload 兼容字段，ProjectPatch proposal 还不是第一等输入对象。
真实 LLM structured output 未接入。
Prefab / AUI / Rule / Asset / Build patch capability 仍不能真实 apply。
```

## 5. 方案选项

### 5.1 方案 A：直接接真实 LLM structured output

做法：

```text
接入真实 LLM provider。
让模型直接输出 ProjectPatchDocument JSON。
马上进入 validate / review / apply。
```

优点：

```text
最接近 AI-first 的最终体验。
可以快速验证模型是否能生成 patch。
```

缺点：

```text
失败原因会混杂：prompt、模型、schema、parser、validator、review、apply、domain command 都可能出错。
难以稳定自动化测试。
真实 provider / key / 网络 / rate limit 不适合作为第一版默认 gate。
```

结论：

```text
不采用本轮。
保留为 205 之后的 LLM Provider / Structured Output Integration v3。
```

### 5.2 方案 B：Imported Patch B-min

做法：

```text
先不接真实 LLM。
新增 ProjectPatchImportService。
支持从 JSON string / file path / test fixture 导入 ProjectPatchDocument。
对导入内容做 parse / schema / capability / validation / review。
通过 AI Panel 或 ProjectPatch review surface 暴露 proposal。
用户确认后走 EditorSession::execute_patch_as_transaction。
输出 ProjectPatchImportProductizationReport。
```

优点：

```text
AI 适配性最高：先固定 structured output contract。
自动化测试稳定：不依赖真实 LLM。
复用 202 的 validator / applier / transaction / history。
为后续真实 LLM 接入提供确定的输入格式和失败报告。
```

缺点：

```text
用户暂时还不能一句话让真实模型生成复杂 patch。
需要用户 / 测试 / 后续 LLM provider 先提供 JSON。
```

结论：

```text
采用。
```

### 5.3 方案 C：Imported Patch + 全域 patch capability

做法：

```text
在 B 的基础上一次支持 Prefab / AUI / Rule / Asset / Build patch。
让 imported patch 能直接修改这些 domain。
```

优点：

```text
能力看起来完整。
更接近复杂项目长期目标。
```

缺点：

```text
范围过大。
Prefab / AUI / Rule 虽已完成 authoring productization，但 patch capability 的 operation schema、inverse、validator、report 尚未单独审查。
Asset / Build patch 边界更重，容易绕过 Asset DB / Build Graph 正式链路。
```

结论：

```text
不采用本轮。
作为 205 完成后的 capability v2/v3 队列。
```

## 6. 推荐方案

采用：

```text
方案 B：Imported Patch B-min
```

过滤依据：

### 6.1 AI 适配性

通过。

```text
它先固定 AI structured output contract。
parse / schema / validation / review / apply / report 全部结构化。
AI 后续失败时可以读 ProjectPatchImportProductizationReport，而不是猜 UI 状态。
```

### 6.2 复杂项目适配与可维护

通过。

```text
复杂打飞机 / 自走棋后续都需要 AI 批量修改工程对象。
205 先证明“外部 patch 进入正式事务”的链路，再逐步扩大 domain capability。
不会把 Player / Enemy / Bullet 等玩法 API 写进 Core。
```

### 6.3 效率

通过。

```text
复用 202 代码。
第一版只新增 import/review/report，不重建 transaction。
测试可 headless deterministic。
```

## 7. v2 正式边界

### 7.1 v2 要做

```text
ProjectPatchImportRequest / ProjectPatchImportResult / ProjectPatchImportProductizationReport。
ProjectPatchImportService：from_json_string / from_file / from_fixture。
ImportProjectPatch command：导入并生成 review proposal，不直接 apply。
PreviewImportedProjectPatch command：只做 parse/schema/validate/review。
ApplyImportedProjectPatch command：确认后进入 execute_patch_as_transaction。
AI Panel / AuthoringAiContext 暴露 imported patch summary。
ManualWalkthrough 增加 imported patch capability。
project_e2e_gate 生成 complex-shooter-imported-project-patch-productization-report.json。
```

### 7.2 v2 真实支持的 apply

```text
Scene patch：
  CreateEntity
  RenameEntity
  SetTransform
  SetComponentField
  PlaceAssetIntoScene

Input patch：
  CreateDefaultInputMapping
  AddInputAction
  AddInputBinding
  SetInputBindingDevicePath
```

说明：

```text
Delete / Remove forward patch 如无法可靠 inverse，必须继续按 validator / inverse 阶段拒绝或只作为 inverse 内部操作。
```

### 7.3 v2 只做 import，不做真实 LLM

允许：

```text
JSON string
JSON file
fixture
未来 LLM provider 传入的 JSON 结果
```

不做：

```text
LLM API 调用。
Prompt 模板管理。
Streaming structured output。
多模型路由。
```

### 7.4 v2 不做全域 patch

以下 capability 在 v2 中必须诚实 unsupported / deferred：

```text
Asset
Prefab
Aui
Rule
Build
```

规则：

```text
不能为了 imported patch 成功而绕过 203/204 的 authoring service。
不能直接 fs::write Prefabs / AUI / Rules。
不能把 unsupported operation 静默忽略。
```

## 8. 建议数据结构

### 8.1 ProjectPatchImportRequest

```text
ProjectPatchImportRequest
  schema_version
  source_kind: JsonString | FilePath | TestFixture | AiStructuredOutput
  source_label
  project_root
  raw_json
  file_path
  expected_patch_id
  dry_run
```

### 8.2 ProjectPatchImportResult

```text
ProjectPatchImportResult
  schema_version
  source_kind
  source_label
  parse_status
  parsed_patch
  schema_diagnostics
  capability_diagnostics
  validation
  review
  proposal_id
  next_actions
```

### 8.3 ProjectPatchImportProductizationReport

```text
ProjectPatchImportProductizationReport
  schema_version
  scenario_id
  status: Pass | Partial | Fail
  source_kind
  source_label
  parse_status
  patch_id
  validation
  review
  apply_report
  history_summary
  supported_capabilities
  unsupported_capabilities
  diagnostics
  next_actions
  artifacts
```

### 8.4 AI Panel model

第一版可以保留兼容：

```text
AiProposedCommand.command: UiCommandPayload
AiProposedCommand.project_patch: ProjectPatchEvidence
```

但 205 应新增或等价表达：

```text
imported_project_patch:
  source_kind
  patch_id
  parse_status
  validation_status
  review_state
```

长期目标：

```text
AI proposal 的真相是 ProjectPatch proposal，UiCommandPayload 只是兼容执行预览或单步 fallback。
```

## 9. 与现有系统关系

### 9.1 与 202

```text
202 是 ProjectPatch 内部产品化。
205 是 ProjectPatch 外部导入 / structured output 产品化。
```

205 不重写：

```text
PatchValidator
PatchApplier
execute_patch_as_transaction
PatchHistory
ProjectPatchProductizationReport
```

205 只补：

```text
import source
parse/schema diagnostics
import proposal lifecycle
import-specific report
complex shooter imported patch artifact
```

### 9.2 与 203 / 204

203 / 204 让 Prefab / AUI 拥有正式 command/service/report。

但 205 不自动获得：

```text
Prefab patch capability
AUI patch capability
```

原因：

```text
Patch capability 需要自己的 operation schema、validator、inverse、applier 映射和 e2e report。
```

### 9.3 与真实 LLM

205 的正确后续是：

```text
LLM Provider / Structured Output Integration v3
```

前置收益：

```text
真实 LLM 只要输出 ProjectPatchDocument JSON。
输出不合法时，205 的 import report 能告诉 AI/用户 parse/schema/validation 哪一步错。
```

### 9.4 与 Manual Walkthrough

Manual Walkthrough 应增加：

```text
import_project_patch
preview_imported_project_patch
apply_imported_project_patch
inspect_project_patch_report
```

状态：

```text
有 JSON / path 上下文时 executable。
无 JSON / path 时 ExecutableCommandNeedsContext。
```

## 10. 复杂打飞机验收场景

### 场景 A：导入合法 Scene patch

输入：

```text
ProjectPatch JSON:
  Scene.CreateEntity name="Imported Patch Smoke"
```

期望：

```text
parse accepted
schema accepted
validation accepted
review shows Scene / 1 operation
apply committed
PatchHistory records inverse
Scene hierarchy 出现实体
```

### 场景 B：导入合法 Input patch

输入：

```text
ProjectPatch JSON:
  Input.AddInputAction action.imported_patch_fire
  Input.AddInputBinding keyboard/I
```

期望：

```text
apply committed
Input mapping asset 通过正式 Input authoring service 修改
report 可看到 operation_results
```

### 场景 C：非法 JSON / schema 被拒绝

输入：

```text
malformed JSON
或 schemaVersion != project-patch.v1
```

期望：

```text
parse_status = rejected
validation 不执行或 rejected
项目不变
diagnostic 指向 parse/schema 错误
```

### 场景 D：unsupported capability 被拒绝

输入：

```text
required_capabilities=[Aui]
```

期望：

```text
capability diagnostic 指向 Aui unsupported
status = partial 或 fail，按是否 apply 决定
next_action = aui_patch_capability_v2
不能静默忽略
```

### 场景 E：Complex Shooter imported patch smoke

输入：

```text
samples/complex_shooter_project
  -> import ProjectPatch fixture
  -> preview
  -> apply
  -> write complex-shooter-imported-project-patch-productization-report.json
```

期望：

```text
report.status = pass
source_kind = TestFixture 或 FilePath
parse / validation / review / apply / history_summary 均有证据
```

## 11. 可施工 Gate 建议

### Gate A：Import Model / Service

目标：

```text
新增 ProjectPatchImportRequest / Result / Report。
新增 ProjectPatchImportService，支持 JSON string / file path。
```

测试：

```powershell
cargo test -p editor_core project_patch_import
```

### Gate B：Command Surface / AI Panel Review

目标：

```text
新增 ImportProjectPatch / PreviewImportedProjectPatch / ApplyImportedProjectPatch command 或等价入口。
AI Panel / editor_ui_model 能序列化 imported patch evidence。
```

测试：

```powershell
cargo test -p editor_ui_model ai_panel
cargo test -p editor_core ai_project_patch
```

### Gate C：Apply Imported Patch

目标：

```text
导入后的 patch proposal 能确认执行，进入 execute_patch_as_transaction。
非法 patch 不修改项目。
```

测试：

```powershell
cargo test -p editor_core project_patch
cargo test -p editor_core project_patch_import
```

### Gate D：Manual Walkthrough / AI Context

目标：

```text
ManualWalkthrough / AuthoringAiContext 能报告 imported patch 能力、needs context 和 unsupported domains。
```

测试：

```powershell
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core manual_walkthrough
cargo test -p editor_core authoring_workflow
```

### Gate E：Complex Shooter E2E

目标：

```text
project_e2e_gate 生成 complex-shooter-imported-project-patch-productization-report.json。
```

测试：

```powershell
cargo test -p project_e2e_gate project_patch
cargo test -p project_e2e_gate
```

### Gate F：整体回归与文档同步

目标：

```text
确认 202 / 203 / 204 不回退，完成阶段记录和施工文档归档。
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
禁止第一版接真实 LLM provider。
禁止导入 patch 后直接 fs::write 项目文件。
禁止绕过 PatchValidator。
禁止绕过 EditorSession::execute_patch_as_transaction。
禁止把 unsupported domain 静默忽略。
禁止为了通过测试写 Player / Enemy / Bullet 等玩法专用 patch operation。
禁止在 205 中提前实现 Prefab / AUI / Rule / Asset / Build patch capability。
```

## 13. 方案自审

### 13.1 是否符合当前下一步入口

通过。

```text
49 / 54 / 阶段完成记录均指向 ProjectPatch structured output / imported patch productization v2。
```

### 13.2 是否重复 202

不重复。

```text
202 已完成内部 ProjectPatch 产品化。
205 只补外部结构化 patch 导入、导入报告和 imported patch e2e。
```

### 13.3 是否和 203 / 204 冲突

不冲突。

```text
203 / 204 是 Prefab / AUI authoring domain 产品化。
205 暂不扩 Prefab / AUI patch capability，只把它们报告为 unsupported / deferred。
```

### 13.4 是否符合 AI-first

通过。

```text
schema-first。
parse / schema / validation / review / apply / history 全部可报告。
后续真实 LLM 只接入 structured output source，不改变执行链路。
```

### 13.5 是否支撑复杂项目

通过。

```text
复杂打飞机和自走棋都需要 AI 批量修改项目。
205 先把外部 patch 入口打稳，再扩 domain capability，适合长期复杂项目维护。
```

### 13.6 主要风险

风险一：

```text
用户误以为 v2 已经接真实 LLM。
```

处理：

```text
文档和 report 明确：source_kind 可以是 AiStructuredOutput，但本轮不调用 provider。
```

风险二：

```text
AI 生成 JSON 不稳定。
```

处理：

```text
先做 import report，让错误稳定落在 parse/schema/validation diagnostics。
```

风险三：

```text
ProjectPatch proposal 与 AiProposedCommand 兼容层继续变复杂。
```

处理：

```text
本轮只新增 imported patch evidence；长期再把 AI proposal 真相收敛到 ProjectPatch proposal。
```

## 14. 最终结论

采用：

```text
AI Project Patch Structured Output / Imported Patch Productization v2
方案 B：Imported Patch B-min
```

正式判断：

```text
下一步不是直接接真实 LLM，也不是一次扩全域 patch。
下一步是先让外部结构化 ProjectPatchDocument 成为正式可导入、可审阅、可确认、可执行、可回滚、可报告的项目修改入口。
```

下一步：

```text
如果用户确认进入施工，基于本文生成新的自动化施工文档。
施工文档必须先自审，再按 Gate A-F 实施和测试。
```
