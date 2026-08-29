# 181-M16 AI Project Patch Entry C-min 方案

## 1. 系统定位

本文定义 `130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md` 中的：

```text
M16 AI Project Patch Entry
```

M16 是本引擎区别传统游戏引擎的关键系统之一。它不是一个 AI Panel 小功能，也不是让 AI 直接模拟点击按钮，更不是从零新建一套编辑系统。

本系统的准确定位是：

```text
在现有 UiCommand + EditorSession + CommandTransaction + AI proposal 机制之上，
新增 ProjectPatch 意图层，
把 AI / 用户的自然语言需求变成可验证、可审阅、可回滚边界清晰的项目修改。
```

长期主链路是：

```text
Natural Language
  -> AiIntent
  -> AiPlan
  -> ProjectPatch
  -> Validate
  -> Review
  -> Apply Transaction
  -> Save / Preview / Build
  -> Trace / Report
```

它的目标是让用户用自然语言修改项目，同时保证所有修改仍然遵守正式编辑器规则：

```text
不能绕过 ProjectAuthoringWorkspace
不能绕过 EditorCommandFramework
不能绕过 Domain Transaction
不能绕过 Save / Dirty / Report
不能直接写 Runtime World
不能直接随机改文件
```

## 2. 当前已有基础

当前项目已经具备：

```text
M1 Project Authoring Workspace v1
M2 Project Rule IR / Rust AOT / Runtime Execute 基础
M7 Prefab Workflow v1
M8 Schema-driven Inspector
M9 Asset Browser Productization
M10 Input Mapping Authoring -> Runtime
M12 AUI 基础方向
M13 Console / Report 基础
150-AI-first Editor Command Framework C-min
EditorSession / CommandTransaction
SceneEditTransaction / SceneUndoStack
AI Panel mock proposal
Complex Shooter Real Authoring-to-Playable Vertical Slice gate
```

更具体地说，Rust 层已经存在：

```text
CommandTransaction:
  transaction_id / request_id / command_id / source / payload
  status / read_set / write_set / diagnostics / state_changes / undo_policy

CommandResult:
  status / diagnostics / console_entries / state_changes / ui_model_revision

AI proposal:
  AiProposedCommand
  AiAcceptProposedCommand
  AiRejectProposedCommand

Scene transaction:
  SceneEditTransaction
  SceneUndoStack
  before / after snapshot
  dirty_state
```

因此 M16 不应重复实现这些能力。它只补现有系统缺少的上层：

```text
ProjectPatchPlan / ProjectPatchDocument
PatchValidator
PatchApplier
PatchHistory / Revert
```

本次修订后的关键原则：

```text
M16 不是新建第二套执行系统。
M16 不是替代 UiCommand / EditorSession / CommandTransaction。
M16 是在现有正式编辑器事务之上增加 ProjectPatch 意图层。
ProjectPatch 负责表达“要改什么”，现有 Command / Transaction 负责“怎么安全执行”。
```

现有 AI Panel 的真实状态是：

```text
prompt
  -> mock planner
  -> UiCommandPayload proposal
  -> accept
  -> execute_ui_payload_as_editor_command
```

这个能力可以保留为 M16 的 UI 入口，但不能继续作为长期真相层。长期真相层必须升级为：

```text
ProjectPatchDocument
```

第一版 mock planner 也要从“生成 UiCommandPayload”升级成“生成 ProjectPatchPlan”。这样第一版验证的是 patch 链路本身，不被真实 LLM 的不确定性干扰。

## 3. 参考引擎源码结论

### 3.1 Unity

源码参考：

```text
框架设计/Unity源码参考/AI-Project-Patch-EditorTransaction源码参考.md
```

Unity 的核心经验：

```text
Inspector / Tool 不直接写文件。
字段修改走 SerializedObject / SerializedProperty。
应用修改走 ApplyModifiedProperties。
Undo / Dirty / AssetDatabase 是正式修改链路。
```

对我们的启发：

```text
ProjectPatch operation 必须是结构化字段路径修改，不能是自由文本替换。
AI 修改必须进入 Dirty / Save / Undo 语义。
```

### 3.2 Unreal Engine

源码参考：

```text
框架设计/UE源码参考/AI-Project-Patch-EditorTransaction源码参考.md
```

UE 的核心经验：

```text
Command 入口和 Transaction 修改分离。
真实编辑动作使用 FScopedTransaction。
被修改对象调用 Modify。
修改后进入 MarkPackageDirty / PostEditChange / save。
```

对我们的启发：

```text
ProjectPatch 必须先成为可审阅计划，再通过领域事务应用。
Patch 不能成为绕过 EditorCommandFramework 的后门。
```

### 3.3 Godot

源码参考：

```text
框架设计/Godot源码参考/AI-Project-Patch-EditorUndoRedo源码参考.md
```

Godot 的核心经验：

```text
EditorUndoRedoManager::create_action
add_do_method / add_undo_method
commit_action
ResourceSaver / PackedScene save
```

对我们的启发：

```text
ProjectPatch 可以是一组可验证 operation。
每个 operation 应能说明 do / undo / rollback 边界。
```

### 3.4 Bevy

源码参考：

```text
框架设计/Bevy源码参考/AI-Project-Patch-Reflect-DynamicWorld源码参考.md
```

Bevy 的核心经验：

```text
Reflect / TypeRegistry
Asset Handle stable reference
DynamicWorld / WorldAsset serialization
Commands deferred mutation
```

对我们的启发：

```text
ProjectPatch 必须能验证 component type、field path、asset ref、rule id。
未知类型和未注册资源必须在 Validate 阶段报告。
```

## 4. 方案选择

### 方案 A：AI 直接生成 UiCommand 序列

链路：

```text
prompt -> Vec<UiCommandPayload> -> execute command
```

优点：

```text
实现最快。
可复用现有 AI Panel mock proposal。
```

缺点：

```text
AI 只能像人一样点按钮。
复杂修改会变成长命令脚本。
缺少跨 Scene / Asset / Rule / AUI / Input 的统一审阅和验证。
容易形成第二套脆弱流程。
```

结论：

```text
不采用。
```

### 方案 B：ProjectPatch + Validate + Apply Transaction

链路：

```text
prompt -> ProjectPatch -> Validate -> Apply
```

优点：

```text
比方案 A 稳定。
已经具备 AI-first 结构化修改雏形。
实现复杂度可控。
```

缺点：

```text
如果没有 AiPlan / Review / Report / Build gate 边界，容易退化成“结构化命令批处理”。
长期仍可能缺少 AI 可解释性。
```

结论：

```text
可作为中间方案，但不够体现本引擎最重要的 AI-first 操作方式。
```

### 方案 C：完整 AI Plan / ProjectPatch / Review / Apply / Revert / Build / Trace 管线

链路：

```text
Natural Language
  -> AiIntent
  -> AiPlan
  -> ProjectPatch
  -> PatchValidationReport
  -> PatchReviewModel
  -> PatchApplyReport
  -> Save / Preview / Build
  -> PatchTraceReport
```

优点：

```text
最符合长期主义。
最适合 AI 和复杂项目维护。
和 Unity / UE / Godot 的正式编辑链路思想一致，但更 AI 原生。
```

缺点：

```text
完整做满过重。
第一版如果贪多，会重新进入细节泥潭。
```

结论：

```text
采用 C-min。
架构按 C 定型，第一版只落地最小可验证范围。
```

## 5. C-min 正式边界

### 5.1 第一版目标

C-min 第一版只做：

```text
Rust 层 ProjectPatchPlan / ProjectPatchDocument 数据模型
Scene + Input 两类 PatchOperation
PatchValidator
PatchDiff / PatchReviewModel
PatchApplier: PatchOperation -> UiCommandPayload 序列
PatchHistory + inverse operation Revert
mock planner 输出 ProjectPatchPlan
通过现有 EditorSession / CommandTransaction / SceneEditTransaction / InputMappingAuthoringService 应用
最小 headless end-to-end gate
```

第一版不做：

```text
真实大模型调用
复杂多轮 planning
完整可视化 diff viewer
完整 rollback UI
Asset / Prefab / AUI / Rule patch 的真实应用
完整 Rule Graph 编辑器
完整 AUI 视觉编辑器
跨文件复杂 conflict merge
插件式 PatchOperation 扩展市场
```

这些不是放弃，而是避免第一版失控。

### 5.2 ProjectPatchDocument

```text
ProjectPatchDocument
  schema_version
  patch_id
  title
  source
  intent_summary
  target_project_root
  required_capabilities
  operations
  expected_outcome
  risk_level
  created_at
```

`source` 第一版：

```text
AiAssistant
Test
ImportedPatch
```

`required_capabilities` 第一版只允许：

```text
Scene
Input
```

长期可以扩展：

```text
Asset
Prefab
Aui
Rule
Build
```

但这些只作为长期 capability 方向记录，不进入 C-min 第一版真实 Apply 范围。

### 5.3 PatchOperation

外层统一：

```text
PatchOperation
  operation_id
  domain
  kind
  target
  payload
  depends_on
  validation_policy
```

内层按 domain typed。C-min 第一版真实落地的 operation 只有 Scene / Input：

```text
ScenePatchOperation
  CreateEntity
  DeleteEntity
  RenameEntity
  SetTransform
  SetComponentField
  PlaceAssetIntoScene

InputPatchOperation
  CreateDefaultInputMapping
  AddInputAction
  AddInputBinding
  SetInputBindingDevicePath
```

长期保留但第一版不应用的 operation 方向：

```text
AssetPatchOperation
  RegisterExistingAsset
  ImportGeneratedAssetMetadata
  SelectAsset

PrefabPatchOperation
  InstantiatePrefab
  CreatePrefabFromSelection

AuiPatchOperation
  CreateAuiDocument
  SetAuiNodeField
  BindAuiField

RulePatchOperation
  AddRuleManifestEntry
  ValidateRuleManifest
```

第一版真实落地：

```text
Scene:
  CreateEntity
  DeleteEntity
  RenameEntity
  SetTransform
  SetComponentField
  PlaceAssetIntoScene

Input:
  CreateDefaultInputMapping
  AddInputAction
  AddInputBinding
  SetInputBindingDevicePath
```

规则：

```text
PatchOperation 是 AI / 用户意图层。
UiCommandPayload 是编辑器执行层。
PatchApplier 负责把一个 PatchOperation 展开成一个或多个 UiCommandPayload。
PatchOperation 不能退化成 UiCommand 的别名。
```

`PlaceAssetIntoScene` 的边界：

```text
它属于 Scene patch，因为它修改的是 SceneDocument。
它第一版只允许引用已经存在、已经导入、已经可被 AssetRef 指向的资源。
它不允许导入新资源、不允许修改 AssetDatabase、不允许生成 Asset metadata。
真正的 AssetPatchOperation 第一版不做真实应用。
```

例子：

```text
ProjectPatchOperation::CreateControllableEntity
  -> UiCommandPayload::CreateSceneEntity
  -> UiCommandPayload::SetSceneTransform
  -> UiCommandPayload::SetSceneComponentField
  -> UiCommandPayload::AddInputAction
  -> UiCommandPayload::AddInputBinding
```

上面的 `CreateControllableEntity` 只是示例意图名，不代表引擎内置玩法 API。具体实体类型、组件和字段仍来自 Project Schema / SceneDocument。

### 5.4 Validate

Validate 不修改项目，只读上下文：

```text
ProjectPatch
  -> ProjectAuthoringWorkspaceContext
  -> Project Schema / SceneDocument / InputMapping
  -> PatchValidationReport
```

Validation 必须检查：

```text
project root exists
active project loaded
target scene exists
entity id exists or create operation can allocate id
component type allowed
field path supported
input action id unique
operation dependencies valid
no forbidden gameplay-specific engine API
operation count within C-min limit
operation domain is Scene or Input
operation conflicts
```

第一版冲突检查至少包括：

```text
same patch deletes and updates same entity
same patch creates duplicate input action id
operation depends_on missing operation_id
operation depends_on a rejected/invalid operation
operation references entity created by later operation without depends_on
```

C-min 操作数限制：

```text
单个 patch 默认最多 32 个 operation。
超过限制先 Rejected，后续再讨论批量 patch。
```

### 5.5 Review

Review 是给用户和 AI 都能读的摘要，不是 UI 装饰：

```text
PatchReviewModel
  patch_id
  title
  summary
  operation_count
  touched_domains
  read_set_preview
  write_set_preview
  risk_level
  validation_status
  diagnostics
  requires_confirmation
```

规则：

```text
任何会修改项目的 patch 都 requires_confirmation = true。
测试可以绕过 UI confirm，但不能绕过 Validate。
```

### 5.6 Apply

Apply 必须走现有正式通道：

```text
ProjectPatch
  -> PatchApplier
  -> Vec<UiCommandPayload>
  -> EditorCommandFramework / EditorSession
  -> CommandTransaction / Domain Transaction
  -> CommandResult
  -> PatchApplyReport
```

禁止：

```text
AI 直接写 JSON 文件。
AI 直接改 EditorSession 内部字段。
AI 直接改 Runtime World。
AI 直接调用 RuntimePackageBuilder 伪造成功。
PatchOperation handler 私自绕开 CommandTransaction。
```

第一版新增一个专门入口：

```text
EditorSession::execute_patch_as_transaction
```

这个入口不是新的事务系统，而是现有 `EditorSession / CommandTransaction / Domain Transaction` 的批量原子适配层。

职责：

```text
1. 调用 PatchValidator 做全量只读验证。
2. 验证通过后，由 PatchApplier 展开 UiCommandPayload 序列。
3. 在单个 ProjectPatchTransaction 边界内依次应用。
4. 全部成功后提交，并记录 PatchHistory。
5. 任一 operation 失败时回滚已应用 operation，或在无法保证回滚时拒绝整个 patch。
```

### 5.7 Undo / rollback 第一版规则

C-min 第一版必须比普通 command 更严格，因为 AI patch 往往表达一个完整意图。规则：

```text
Scene-only patch 必须原子回滚。
Scene + Input patch 默认也必须原子。
如果某个 operation 无法生成 inverse operation，Validate 阶段直接拒绝整个 patch。
PatchApplyReport 必须列出 committed / reverted / skipped / rejected / failed operation。
```

第一版回滚策略：

```text
PatchHistory
  patch_id
  applied_at
  original_patch
  inverse_patch
  apply_report

Revert
  -> inverse_patch
  -> PatchValidator
  -> PatchApplier
  -> execute_patch_as_transaction
```

Scene operation 的 inverse 规则：

```text
CreateEntity -> DeleteEntity
DeleteEntity -> RecreateEntity from before snapshot
RenameEntity -> RenameEntity old name
SetTransform -> SetTransform old transform
SetComponentField -> SetComponentField old value
PlaceAssetIntoScene -> DeleteEntity created entity
```

Input operation 的 inverse 规则：

```text
CreateDefaultInputMapping -> restore previous document or delete created document
AddInputAction -> RemoveInputAction
AddInputBinding -> RemoveInputBinding
SetInputBindingDevicePath -> SetInputBindingDevicePath old value
```

长期路线仍然是：

```text
ProjectPatchTransaction
  -> multi-domain undo group
  -> reversible operation
  -> rollback preview
```

## 6. 数据流

```text
AI Panel / Test / Imported Patch
  -> ProjectPatchDocument
  -> PatchValidator
  -> PatchDiff / PatchReviewModel
  -> user confirm
  -> PatchApplier
  -> UiCommandPayload sequence
  -> EditorCommandFramework
  -> EditorSession / Domain Services
  -> SceneEditTransaction / InputMappingAuthoringService
  -> PatchApplyReport
  -> PatchHistory
  -> WorkspaceReport / Console / AI Panel
```

## 7. 与现有系统关系

### 与 M1

M1 提供统一 workspace context。M16 读取它，不替代它。

```text
M1: 当前项目状态和可编辑领域
M16: 根据 AI 意图生成结构化修改
```

### 与 150 Editor Command Framework

M16 Apply 必须进入 150，不建立第二套执行器。

```text
PatchOperation
  -> PatchApplier
  -> UiCommandPayload / EditorCommandRequest adapter
  -> EditorCommandExecutor
```

### 与 SceneEditTransaction

ScenePatchOperation 必须映射到 SceneEditCommand / UiCommandPayload，再走 SceneEditTransaction。

### 与 Project Rule

RulePatchOperation 第一版不做真实应用。Rule 只作为长期方向保留，不进入 C-min 施工范围。

### 与 Asset / AUI / Input

Input 第一版接入当前已有能力。Asset / AUI 第一版不做真实应用，只保留长期方向。不为复杂打飞机硬编码专用 API。

## 8. 与其它引擎对比

| 项目 | Unity | UE | Godot | Bevy | 我们 |
|---|---|---|---|---|---|
| 编辑入口 | Inspector / Tool / Menu | Command / Tool / Details | Dock / Inspector / Plugin | Runtime app / ECS | AI Plan / ProjectPatch / EditorCommand |
| 修改真相 | SerializedObject | UObject / Package | Node / Resource | World / Asset / Reflect | ProjectDocument / Domain Document |
| 事务 | Undo / Dirty | FScopedTransaction / Modify | EditorUndoRedoManager | Commands 不等同编辑事务 | CommandTransaction / DomainTransaction / PatchApplyReport |
| AI 友好 | 弱 | 弱到中 | 中 | 数据层强，编辑器弱 | 强 |
| 跨域修改 | 分散 | 强但复杂 | 中 | 主要 runtime | ProjectPatch 统一表达 |
| 第一版复杂度 | 成熟黑箱 | 成熟但很重 | 简洁但弱类型 | 不是编辑器方案 | C-min 控制范围 |

我们的方案最像：

```text
UE 的 Command + Transaction 思想
+ Unity 的 Serialized field path 思想
+ Godot 的 do/undo operation 序列思想
+ Bevy 的 Reflect/type validation 思想
```

但我们不照搬任何一个，而是建立 AI-first 的 ProjectPatch 真相层。

## 9. 最小验收场景

### 场景 A：AI 创建一个可渲染实体

```text
prompt: 创建一个 player_plane 实体并放到 (0, -3, 0)
  -> ProjectPatch
  -> Scene.CreateEntity
  -> Scene.SetTransform
  -> Scene.SetComponentField SpriteRenderer2D.textureRef
  -> Validate passed
  -> Apply committed
  -> SceneDocument dirty
  -> Save
```

注意：

```text
player_plane 是项目侧实体名，不是引擎内置 API。
```

### 场景 B：AI 添加输入映射

```text
prompt: 添加 Fire 输入，绑定 Space
  -> ProjectPatch
  -> Input.AddInputAction
  -> Input.AddInputBinding
  -> Validate unique action id
  -> Apply through InputMappingAuthoringService
```

### 场景 C：AI 创建实体并添加输入映射

```text
prompt: 创建一个可控制实体，并把 Space 绑定为 Fire
  -> ProjectPatch
  -> Scene.CreateEntity
  -> Scene.SetTransform
  -> Input.AddInputAction
  -> Input.AddInputBinding
  -> Validate passed
  -> PatchApplier expands to UiCommandPayload sequence
  -> execute_patch_as_transaction committed
  -> PatchHistory records inverse patch
```

### 场景 D：AI 生成修改但验证失败

```text
prompt: 把 missing_entity 的速度改成 10
  -> ProjectPatch
  -> Validate failed
  -> PatchValidationReport says entity missing
  -> no document mutation
```

### 场景 E：Patch Revert

```text
apply patch:
  Scene.CreateEntity
  Input.AddInputAction
  Input.AddInputBinding

history:
  inverse patch recorded

revert:
  inverse patch validates
  entity removed
  input binding removed
  input action removed
```

## 10. C-min 施工建议 Gate

后续如果进入施工，建议分 Gate：

```text
Gate 1: 新增 project_patch 数据模型 crate/module
Gate 2: PatchValidator 只读验证 Scene/Input
Gate 3: PatchApplier 映射 Scene/Input operation 到 UiCommandPayload 序列
Gate 4: execute_patch_as_transaction 复用现有 CommandTransaction 做原子应用与失败回滚
Gate 5: PatchHistory + inverse operation Revert
Gate 6: AI Panel 从 proposed UiCommand 升级为 proposed ProjectPatch
Gate 7: PatchDiff / PatchReviewModel 接入 EditorUiModel
Gate 8: ProjectPatch end-to-end tests
Gate 9: Complex shooter patch smoke gate
```

每个 Gate 完成后必须测试，再进入下一个 Gate。

## 11. 正式规则

```text
1. AI 修改项目的长期真相层是 ProjectPatchDocument / ProjectPatchPlan，不是 UiCommand 序列。
2. AI Panel 可以作为入口，但不能成为修改真相层。
3. ProjectPatch 必须 Validate 后才能 Apply。
4. Validate 不允许修改项目。
5. Apply 必须通过 EditorCommandFramework / EditorSession / Domain Transaction。
6. PatchOperation 必须 typed，禁止自由文本脚本修改文件。
7. PatchOperation 必须记录 domain / kind / target / payload / dependency。
8. PatchOperation 是意图层，UiCommandPayload 是执行层，二者必须通过 PatchApplier 显式翻译。
9. PatchApplyReport 必须列出每个 operation 的状态。
10. C-min 第一版只真实应用 Scene / Input patch。
11. C-min 第一版不接真实 LLM，mock planner 必须输出 ProjectPatchPlan。
12. Patch 必须原子应用；无法生成 inverse operation 时必须在 Validate 阶段拒绝。
13. PatchHistory 必须记录 original_patch / inverse_patch / apply_report。
14. Revert 必须走 inverse patch -> Validate -> Apply，不允许直接手改文档。
15. Scene operation 必须复用 SceneEditTransaction。
16. Input operation 必须复用 InputMappingAuthoringService。
17. Asset / Prefab / AUI / Rule operation 第一版只保留长期 schema 方向，不做真实应用。
18. PlaceAssetIntoScene 第一版只允许使用已存在 AssetRef，不允许导入、生成或修改 AssetDatabase。
19. 不允许为复杂打飞机新增 Player / Enemy / Bullet / Health / Score 等引擎专用 API。
20. 复杂项目能力来自 Project Schema / Rule / Prefab / Asset / AUI / Input，不来自引擎硬编码。
```

## 12. 方案自审

```text
合乎规格:
  本方案对应 130 中 M16，长期解决自然语言修改 Scene / Prefab / Rule / Asset / AUI / Input 的入口不足；C-min 第一版只真实落地 Scene / Input。

合乎规则:
  遵守大系统优先、AI 友好、复杂项目维护、效率、简单度、长期主义、不新增项目专用 API 的规则。

合乎方案文字本身:
  文中统一使用 ProjectPatchDocument / ProjectPatchPlan / PatchOperation / Validate / Review / Apply / Report，不再把 UiCommand proposal 当长期真相。
  文中明确 ProjectPatch 是意图层，UiCommandPayload 是执行层，EditorSession / CommandTransaction 仍是正式执行通道。

合乎设计:
  方案承接 M1 Workspace、150 Command Framework、CommandTransaction、AiProposedCommand、SceneEditTransaction、InputMappingAuthoringService，不推翻既有主线。

方便实现:
  C-min 可从现有 UiCommandPayload / EditorSession / SceneEditTransaction / InputMappingAuthoringService 适配开始，不需要一次重写编辑器。

合理、能实现:
  第一版只做 Scene + Input，且先 mock 后 LLM；PatchHistory 使用 inverse operation，不做大 snapshot，复杂度可控。
```

结论：

```text
方案通过自审。
正式采用 M16 AI Project Patch Entry C-min。
下一步如果用户确认，可以基于本文生成施工文档；当前不直接施工。
```
