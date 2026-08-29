# 191-Authoring Walkthrough Missing Operations Convergence v1 方案

## 1. 系统是什么

本系统正式命名为：

```text
Authoring Walkthrough Missing Operations Convergence v1
```

中文定位：

```text
完整用户手动编辑 walkthrough 缺失操作收敛 v1
```

它解决的是 179 / 190 完成后仍然存在的主线缺口：

```text
复杂打飞机真实项目已经能被 EditorSession 打开，
也能进入 export/package/headless-player 纵切，
AuthoringWorkflow 也已经能显示步骤、推荐任务、点击命令并路由到 UiCommand。

但用户如果真的从编辑器里手动制作 / 修改一个复杂项目，
仍然缺一份可审查的“完整操作路线”：
  哪些步骤已经能真实执行？
  哪些只能聚焦面板？
  哪些没有 domain command？
  哪些会阻塞 Play / Build / Export？
  下一轮应该补哪个真实 authoring domain？
```

本方案不是重建 `AuthoringWorkflowModel`，也不是新增第二套 Walkthrough / Operation 执行层。

它是在已完成的 `183-AuthoringWorkflow Productization v1` 之后，做一层缺失操作收敛：

```text
AuthoringWorkflowModel
  -> AuthoringCommand / WorkflowCommandResolver
  -> ManualAuthoringOperationRequirement
  -> ManualWalkthroughCoverageReport
  -> MissingOperationGap
  -> next construction entry
```

## 2. 归属规则

本系统属于 `130-复杂打飞机编辑到 Windows 可玩项目缺失能力当前基线` 中的：

```text
M1 Project Authoring Workspace v1
M7 Prefab Workflow
M8 Schema-driven Inspector
M9 Asset Browser
M10 Input Mapping
M11 Physics2D Collider Authoring / Visualization
M12 AUI HUD Authoring / Binding / Runtime Present
M13 Unified Report Panel
M14 Save / Reload / Rebuild Consistency Gate
M15 Exported Game Golden Scenario
```

它不是 M16 AI Project Patch Entry。

二者关系是：

```text
Manual Walkthrough:
  用户按步骤点击和编辑，暴露真实 domain authoring 缺口。

AI Project Patch:
  AI 根据结构化意图生成 ProjectPatch / UiCommand，复用同一批 domain command。
```

第一版先把“用户手动编辑路线”的缺口收敛清楚，再决定下一份施工文档补哪个 domain。

## 3. 当前真实基线

已经完成：

```text
AuthoringWorkflowModel / Step / Task / Command / AiContext
AuthoringWorkflowComposer
WorkflowCommandResolver
WorkflowCommandResolution
AuthoringWorkflowCommand hit target
editor_input workflow command route
Project / Scene / Input / Play / Build / Report 核心 workflow command 覆盖
Prefab / Rule / AUI 第一版 focus panel command
Complex Shooter Real Authoring-to-Playable Vertical Slice report
AUI RuntimePackage document body / load / binding / overlay / UI pass evidence
```

关键文件：

```text
rust/crates/editor_ui_model/src/authoring_workflow.rs
rust/crates/editor_ui_model/src/workflow_command.rs
rust/crates/editor_core/src/authoring_workflow.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_ui_renderer/src/panels/workspace.rs
rust/crates/project_e2e_gate/src/vertical_slice.rs
```

已完成阶段记录：

```text
阶段完成记录/2026-07-03-AuthoringWorkflow-Productization-v1/00-总览.md
阶段完成记录/2026-07-03-Complex-Shooter-Real-Authoring-to-Playable-Vertical-Slice-v1/00-总览.md
阶段完成记录/2026-07-04-AUI-RuntimePackage-Document-Hydration-Binding-Present-v1/00-总览.md
```

当前仍缺：

```text
完整用户手动编辑 walkthrough 的操作覆盖矩阵。
从 Project -> Assets -> Scene -> Prefab -> Rule -> Input -> AUI -> Play -> Build -> Reports 的真实操作验收。
每个操作的状态分类：可执行 / 可聚焦 / 缺 domain command / 被其它系统阻塞。
针对复杂项目的 missing operation next_actions。
Save / Reload / Rebuild 一致性在 walkthrough 中的显式门禁。
Prefab / Rule / AUI 等 domain 的真实可编辑命令仍不完整。
```

## 4. 联网成熟引擎参考

### 4.1 Unity

参考：

```text
https://docs.unity3d.com/ScriptReference/Undo.RecordObject.html
https://docs.unity3d.com/ScriptReference/SerializedObject.ApplyModifiedProperties.html
https://docs.unity3d.com/Manual/BuildSettings.html
```

结论：

```text
Unity 编辑操作不是直接改文件。
对象修改进入 Undo / SerializedObject / SerializedProperty / ApplyModifiedProperties。
Build 通过 Build Profiles / Build Settings 组织目标平台、场景和配置。
```

可借鉴：

```text
用户手动编辑必须进入可撤销、可保存、可构建的正式链路。
Inspector / Scene / Assets / Build 是连续创作心智。
```

不照搬：

```text
不采用 Unity 的隐式对象真相。
不让 UI 控件直接改 JSON 或 runtime object。
```

### 4.2 Unreal Engine

参考：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/details-panel-customization?application_version=4.27
https://dev.epicgames.com/documentation/en-us/unreal-engine/assets-and-packages?application_version=4.27
https://dev.epicgames.com/documentation/en-us/unreal-engine/reducing-apk-package-size?application_version=4.27
```

结合本地 UE 源码参考：

```text
其它AI审查目录/10-Authoring Walkthrough Spine 方案审查.md
其它AI审查目录/11-183-AuthoringWorkflow Productization v1 方案审查.md
框架设计/UE源码参考/AI-Project-Patch-EditorTransaction源码参考.md
```

结论：

```text
UE 的 Details / ContentBrowser / Transaction / Packaging 共同支撑编辑到打包。
资源移动、引用、打包配置都必须在编辑器内部受控完成。
```

可借鉴：

```text
复杂编辑器需要 command / transaction / details / content browser 的统一纪律。
```

不照搬：

```text
不采用完整 Slate / FScopedTransaction / UObject 体系。
不把 ContentBrowser 路径引用规则照搬成本项目真相层。
```

### 4.3 Godot

参考：

```text
https://docs.godotengine.org/en/stable/classes/class_editorundoredomanager.html
```

结合本地 Godot 源码参考：

```text
框架设计/Godot源码参考/12-GodotAllCode-整体源码地图与参考方案.md
框架设计/Godot源码参考/AI-Project-Patch-EditorUndoRedo源码参考.md
```

结论：

```text
Godot 的 EditorNode / Dock / Inspector / Run / Export 路线简单集中。
EditorUndoRedoManager 证明编辑操作需要明确 do/undo 边界。
```

可借鉴：

```text
中心编辑器状态 + 多个 domain dock + undo/redo + export 是清晰路线。
```

不照搬：

```text
不采用 Node / Signal / Object method string 作为项目真相层。
```

### 4.4 Bevy

参考：

```text
https://docs.rs/bevy_scene/latest/bevy_scene/struct.DynamicScene.html
https://docs.rs/bevy_reflect/latest/bevy_reflect/
```

结论：

```text
Bevy 的 DynamicScene / Reflect 适合参考 schema-driven data、scene serialization 和 runtime world 写入。
但 Bevy 没有 Unity / UE / Godot 级正式编辑器 workflow。
```

可借鉴：

```text
结构化 scene / component / type registry 思路。
```

不照搬：

```text
不把 ECS World / DynamicScene 作为编辑器项目文件真相层。
```

## 5. 本地文档和审查结论

必须遵守：

```text
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
175-Complex-Shooter-Authoring-Workflow-v1方案.md
183-AuthoringWorkflow-Productization-v1方案.md
179-Complex-Shooter-Real-Authoring-to-Playable-Vertical-Slice-v1方案.md
181-M16-AI-Project-Patch-Entry-C-min方案.md
185-M12-AUI-HUD-Authoring-Binding-Runtime-Present-v1方案.md
190-AUI-RuntimePackage-Document-Hydration-Binding-Present-v1方案.md
```

外部审查已经给出强结论：

```text
不要新增 Walkthrough / Operation / DomainOperationCatalog 层。
AuthoringWorkflowModel 骨架已存在。
183 已经完成 WorkflowCommandResolver 和可点击 workflow command C-min。
下一步应选择还没有产品化的真实 Domain，或收敛完整用户手动编辑 walkthrough 中缺失的真实操作。
```

因此本方案只新增：

```text
缺失操作覆盖矩阵。
手动 walkthrough 覆盖报告。
gap -> domain next_action 映射。
```

不新增：

```text
新的 Walkthrough 真相层。
新的 Operation 执行层。
新的 command framework。
新的打飞机专用 API。
```

## 6. 方案对比

### 6.1 方案 A：继续补单个缺失按钮

做法：

```text
看到 Prefab 缺按钮就补 Prefab。
看到 Rule 缺按钮就补 Rule。
看到 AUI 缺按钮就补 AUI。
```

优点：

```text
短期每次改动很小。
```

缺点：

```text
会回到散装编辑器。
无法回答完整用户 walkthrough 还缺什么。
AI 也无法判断哪一步最阻塞。
```

结论：

```text
不采用。
```

### 6.2 方案 B：做一个一键向导 / Wizard

做法：

```text
新增一个向导面板，帮用户自动创建复杂打飞机项目。
```

优点：

```text
演示效果快。
```

缺点：

```text
容易变成打飞机专用流程。
绕过真实 domain authoring。
和 M16 AI Project Patch / ProjectPatch 职责重叠。
```

结论：

```text
不采用。
```

### 6.3 方案 C-min：缺失操作覆盖矩阵 + 手动 Walkthrough 收敛报告

做法：

```text
定义完整用户手动编辑 walkthrough 所需的通用 operation requirements。
把现有 AuthoringWorkflowCommand / UiCommandPayload / Domain 服务映射进去。
生成 ManualWalkthroughCoverageReport。
把每个 gap 分类到 Prefab / Rule / AUI / Asset / Input / Scene / Build / Report 等 domain。
只对已有服务和已有 command 做轻量补齐，不新造大系统。
```

优点：

```text
AI 适配性强：缺口结构化、可读、可排序、可施工。
复杂项目适配强：不写死打飞机玩法，只表达通用编辑操作。
效率高：复用 183/184 已完成的 workflow command / input route / report 基础。
```

缺点：

```text
第一轮不会一次补完 Prefab / Rule / AUI 全部编辑器能力。
它会把真实缺口暴露出来，后续仍要逐个 domain 施工。
```

结论：

```text
采用。
```

## 7. 推荐方案

采用：

```text
方案 C-min：缺失操作覆盖矩阵 + 手动 Walkthrough 收敛报告
```

过滤优先级：

### 7.1 AI 适配性

通过。

```text
ManualAuthoringOperationRequirement 是 schema-first。
ManualWalkthroughCoverageReport 是结构化 report。
MissingOperationGap 可直接进入 next_actions / construction doc。
```

### 7.2 复杂项目适配与可维护

通过。

```text
操作是通用 domain operation，不出现 Player / Enemy / Bullet / Score。
复杂项目只通过项目侧 Scene / Prefab / Rule / Asset / AUI / Input 表达。
后续 domain 能力增强时，只更新覆盖矩阵，不推翻 workflow。
```

### 7.3 效率

通过。

```text
复用 AuthoringWorkflowModel / WorkflowCommandResolver / EditorInputRouter。
优先做 headless deterministic report。
只把已有能力纳入覆盖，不为了矩阵伪造 command。
```

## 8. 核心数据结构

### 8.1 ManualAuthoringOperationRequirement

```text
ManualAuthoringOperationRequirement
  operation_id
  domain
  title
  user_goal
  required_for_play
  required_for_build
  required_for_complex_project
  expected_command_id
  expected_payload_kind
  required_context
  fallback_behavior
```

`required_context` 示例：

```text
None
OpenProject
SelectedAsset
SelectedEntity
SelectedInputMapping
SelectedAuiDocument
BuildProfile
```

### 8.2 ManualAuthoringOperationStatus

```text
ExecutableCommand
ExecutableCommandNeedsContext
FocusDomainPanel
MissingCommand
MissingDomainService
BlockedByDependency
Deferred
```

含义：

```text
ExecutableCommand:
  Workflow / UI / domain 能生成正式 UiCommandPayload 或 ProjectPatch。

ExecutableCommandNeedsContext:
  能识别到正式 command / payload kind，但当前缺少安全执行所需参数上下文，
  例如 path / selected entity / selected asset 为空或未选择。
  第一版必须输出 required_context 和 next_action，不能把空参数 command 伪装成可执行。

FocusDomainPanel:
  Workflow 能把用户带到正确 domain，但具体参数由 domain 面板处理。

MissingCommand:
  domain 能力可能存在，但没有正式 UiCommandPayload / AuthoringCommand 入口。

MissingDomainService:
  底层服务尚不存在或不完整。

BlockedByDependency:
  被其它系统阻塞，例如真实 glyph / asset cook / rule runtime。

Deferred:
  v1 不做，但必须说明原因。
```

### 8.3 ManualWalkthroughCoverageReport

```text
ManualWalkthroughCoverageReport
  schema_version
  project_id
  scenario_id
  status
  operation_count
  executable_count
  focus_panel_count
  missing_command_count
  missing_service_count
  blocked_count
  operations
  domain_summaries
  blocking_gaps
  next_actions
  diagnostics
```

### 8.4 MissingOperationGap

```text
MissingOperationGap
  gap_id
  domain
  operation_id
  severity
  reason
  suggested_system
  suggested_next_action
  blocks_manual_walkthrough
  blocks_play
  blocks_build
```

### 8.5 第一版代码落点

```text
editor_ui_model:
  ManualAuthoringOperationRequirement
  ManualAuthoringOperationStatus
  ManualWalkthroughOperationCoverage
  ManualWalkthroughCoverageReport
  MissingOperationGap
  ManualWalkthroughCoverageSummary

editor_core:
  ManualWalkthroughCoverageAnalyzer
  从 ProjectAuthoringWorkspaceModel / AuthoringWorkflowModel / WorkflowCommandResolver 派生 coverage report

project_e2e_gate:
  对 samples/complex_shooter_project 生成 manual walkthrough coverage artifact
```

边界：

```text
Report 数据结构属于 UI model / report model。
Analyzer 属于 editor_core 业务聚合。
project_e2e_gate 只做真实样例验证，不拥有 authoring 业务规则。
```

## 9. 第一版完整用户 Walkthrough 操作清单

第一版只定义通用操作，不写玩法专名。

### 9.1 Project

```text
OpenProject
CreateProject
RefreshRecentProjects
SaveProject
ReloadProject
```

### 9.2 Assets

```text
BrowseAssets
SelectAsset
OpenAsset
ImportAsset
PlaceAssetIntoScene
ValidateAssetReferences
```

### 9.3 Scene

```text
OpenSceneDocument
CreateSceneDocument
CreateSceneEntity
SelectSceneEntity
RenameSceneEntity
DeleteSceneEntity
SetSceneTransform
AddComponent
SetSceneComponentField
SaveSceneDocument
UndoSceneEdit
RedoSceneEdit
```

### 9.4 Prefab

```text
CreatePrefabFromSelection
OpenPrefabDocument
InstantiatePrefabInScene
ApplyPrefabChanges
SavePrefabDocument
ValidatePrefabReferences
```

### 9.5 Rules

```text
CreateRuleAsset
OpenRuleAsset
EditRuleGraphOrDsl
ValidateRuleAsset
BuildRuleArtifact
RegisterRuleArtifact
InspectRuleDiagnostics
```

### 9.6 Input

```text
CreateDefaultInputMapping
SelectInputMapping
AddInputAction
AddInputBinding
SetInputBindingDevicePath
ValidateInputMapping
SaveInputMapping
```

### 9.7 AUI

```text
CreateAuiDocument
OpenAuiDocument
AddAuiNode
EditAuiNodeField
EditAuiBindingPath
EditAuiActionRef
ValidateAuiDocument
SaveAuiDocument
PreviewAuiOverlay
```

### 9.8 Play / Build / Reports

```text
OpenRuntimePackage
ReloadRuntimePackage
Play
Pause
StepFrame
ResetRuntime
ExportDesktopPackage
OpenBuildOutput
OpenBuildReport
OpenRuntimeReport
OpenAuthoringWalkthroughReport
ClearConsole
SelectTraceEntry
```

## 10. 第一版收敛规则

### 10.1 Workflow 不补参数

`AuthoringWorkflowCommand` 不做通用参数收集器。

需要参数的操作必须：

```text
返回 ExecutableCommandNeedsContext
或
返回 FocusDomainPanel
或交给对应 Domain UI / Inspector
或由 ProjectPatch 在 M16 中表达
```

禁止：

```text
Workflow 为了能执行而伪造实体名、组件字段、资源路径、AUI 节点等参数。
把空 path / 空 selection / 空 binding 当成完整 ExecutableCommand。
```

### 10.2 只读摘要，不越域读取原始文档

Coverage / readiness 第一版默认读取：

```text
ProjectAuthoringWorkspaceModel
AuthoringWorkflowModel
InputMappingAuthoringModel
BuildExportModel
ConsoleModel / Report summaries
```

不要直接绕过 domain service 去解析 Scene / Prefab / AUI / Rule 原始文件。

如摘要不足，应先增强对应 domain summary，而不是让 walkthrough report 自己读文件。

### 10.3 Gap 不等于失败

第一版 report 有三种状态：

```text
pass:
  当前 C-min 手动 walkthrough 必需操作都可执行或可聚焦。

partial:
  主流程可走，但存在 MissingCommand / MissingDomainService。

fail:
  Project / Scene / Build / Report 等基础步骤无法形成最小用户路径。
```

### 10.4 不重复 183 / 184

不得重做：

```text
AuthoringWorkflowModel
WorkflowCommandResolver
AuthoringWorkflowCommand hit target
editor_input route
```

只允许补：

```text
operation requirement matrix
coverage report
missing gap classification
明显缺漏的 command metadata
```

## 11. 可施工 Gate

### Gate A：Operation Requirement Matrix

目标：

```text
新增完整用户手动 walkthrough 操作清单和 domain 分类。
```

产物：

```text
ManualAuthoringOperationRequirement
ManualAuthoringOperationStatus
```

测试：

```text
cargo test -p editor_ui_model manual_walkthrough
```

### Gate B：Coverage Analyzer

目标：

```text
把 operation requirements 与现有 AuthoringWorkflowCommand / UiCommandPayload / WorkflowCommandResolver 对齐。
```

产物：

```text
ManualWalkthroughCoverageReport
MissingOperationGap
```

测试：

```text
cargo test -p editor_core manual_walkthrough
```

### Gate C：真实复杂项目 Coverage Report

目标：

```text
对 samples/complex_shooter_project 生成 manual walkthrough coverage artifact。
```

规则：

```text
不能因为 Prefab / Rule / AUI 缺真实编辑命令而伪装 pass。
必须输出 partial + next_actions。
```

测试：

```text
cargo test -p project_e2e_gate manual_walkthrough
```

### Gate D：Workflow / AI Context 接入

目标：

```text
AuthoringAiContext 或 WorkspaceContext 能引用最新 manual walkthrough coverage summary。
用户和 AI 都能看到下一轮缺失操作。
```

测试：

```text
cargo test -p editor_core authoring_workflow
cargo test -p editor_ui_model workflow_command
```

### Gate E：文档和入口同步

目标：

```text
更新 49 / 54 / 阶段完成记录。
如果生成施工文档，按一份施工文档执行并归档。
```

测试：

```text
cargo fmt --check
相关 crate 测试
```

## 12. 第一版验收标准

必须证明：

```text
完整用户手动 walkthrough 操作清单存在，且不包含打飞机专用 API。
每个操作有 domain / required_context / status / next_action。
现有 WorkflowCommandResolver 可执行命令被识别为 ExecutableCommand。
需要参数上下文的命令被识别为 ExecutableCommandNeedsContext 或 FocusDomainPanel，而不是伪造 payload。
Prefab / Rule / AUI 缺失真实编辑操作时输出 MissingCommand 或 MissingDomainService。
samples/complex_shooter_project 生成 ManualWalkthroughCoverageReport。
Report 能说明下一轮最该补哪个真实 domain authoring 系统。
AI context 能读取 coverage summary。
```

第一版不要求：

```text
一次补完 Prefab Editor。
一次补完 Rule Editor。
一次补完 209 Scene Unified AUI Authoring。
一次补完真实 glyph present。
一次完成 M16 AI Project Patch。
```

## 13. 与后续系统的关系

本系统完成后，下一轮施工选择应由 coverage report 决定。

可能的 next_actions：

```text
rule_authoring_productization
aui_authoring_productization
prefab_authoring_productization
asset_import_productization
save_reload_rebuild_consistency_gate
runtime_text_glyph_present
ai_project_patch_entry
```

选择规则：

```text
优先补 blocks_manual_walkthrough=true 且 blocks_play/build=true 的 domain。
同等阻塞时优先 AI 适配性高、复杂项目复用价值高、施工面更小的系统。
```

## 14. 方案自审

### 14.1 是否合乎用户规格

通过。

```text
用户要求讨论“完整用户手动编辑 walkthrough 缺失操作收敛”。
本方案直接围绕完整用户手动路径，不转向 glyph 或 AI patch。
```

### 14.2 是否合乎新 skill 讨论规范

通过。

```text
已先参考成熟引擎公开文档。
已读取本地源码参考、其它 AI 审查、当前入口、完成记录和代码基线。
给出 A/B/C 三个可选方案，并按 AI 适配性、复杂项目适配与可维护、效率过滤。
```

### 14.3 是否合乎既有架构规则

通过。

```text
不新增 Walkthrough / Operation 执行层。
不绕过 UiCommandPayload / ProjectPatch / EditorSession。
不把具体玩法写入引擎 Core。
不让 Runtime 扫描项目源目录。
不把测试 gate 当成用户创作完成。
```

### 14.4 是否方便实现

通过。

```text
183/184 已经完成 WorkflowCommandResolver 和 workflow command route。
本方案主要新增 coverage/report，不要求一次补完整 domain editor。
可以按 editor_ui_model -> editor_core -> project_e2e_gate -> AI context 分阶段测试。
```

### 14.5 主要风险

风险一：

```text
Coverage report 变成新的大而全业务层。
```

处理：

```text
Coverage 只读 summary，只分类 gap，不执行 domain 业务。
```

风险二：

```text
为了让 report pass 而伪造 command。
```

处理：

```text
需要参数或服务缺失时必须 FocusDomainPanel / MissingCommand / MissingDomainService。
```

风险三：

```text
用户误以为本系统会一次补完所有手动编辑能力。
```

处理：

```text
本系统是缺口收敛和下一轮施工选择，不是完整商业级编辑器一次完工。
```

## 15. 最终结论

采用：

```text
Authoring Walkthrough Missing Operations Convergence v1
方案 C-min：缺失操作覆盖矩阵 + 手动 Walkthrough 收敛报告
```

下一步应基于本文生成可自动化施工文档。

施工文档必须包含：

```text
施工目标
涉及文件
Gate A-E 分阶段任务
每阶段测试命令
完成后阶段记录和施工文档归档规则
```
