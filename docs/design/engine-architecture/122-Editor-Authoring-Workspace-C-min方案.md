# 122-Editor Authoring Workspace C-min 方案

## 1. 定位

本方案确认 Native Editor 下一阶段采用：

```text
Editor Authoring Workspace / 编辑器制作工作台 C-min
```

它不是单独的 Inspector TextInput、SceneView picking、ProjectDock 点击或 AI Panel 输入系统。
它是这些编辑能力的统一工作台入口。

当前已经完成：

```text
120-Native-Editor-Usable-Panels-B-AI-C-min方案.md
121-Native-Editor-Application-Shell方案.md
```

121 解决的是编辑器应用壳：

```text
WindowEvent
  -> FocusInputSystem
  -> EditorCommandSystem
  -> EditorSession
  -> TransactionService
  -> UiModel rebuild
  -> DrawList
  -> Present
```

122 解决的是用户和 AI 如何真正编辑项目：

```text
Hierarchy / Inspector / Viewport / ProjectDock / AI Panel
  -> WorkspaceCommand
  -> EditorSession
  -> SceneEditTransaction
  -> EditorSceneDocument
  -> Selection / Dirty / Diagnostics
  -> EditorUiModel rebuild
```

本系统必须坚持引擎底座边界：

```text
只提供通用编辑能力。
不新增 player / enemy / bullet / health / score / wave 等项目玩法 API。
项目玩法只能进入 Project Schema / Project Rule / Project Prefab / Project Asset / Project UI。
```

## 2. 设计问题

如果继续按小系统推进，会出现这些风险：

```text
Inspector 有自己的写入规则。
Hierarchy 有自己的写入规则。
Viewport 有自己的选择规则。
ProjectDock 有自己的资源放置规则。
AI Panel 又生成另一套命令。
```

这会导致后期出现多套真相：

```text
UI 真相
SceneDocument 真相
RuntimeWorld 真相
AI Patch 真相
Test Fixture 真相
```

本方案的核心目标是把编辑入口收敛为三条稳定规则：

```text
EditorSceneDocument 是编辑真相。
WorkspaceCommand 是编辑入口。
Transaction 是修改边界。
```

## 3. 成熟引擎参考

### 3.1 Unity

Unity 对应结构：

```text
EditorWindow
SceneView
SceneHierarchy
InspectorWindow
ProjectBrowser
SerializedObject / SerializedProperty
Undo
```

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SceneView\SceneView.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SceneHierarchy.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector\Core\InspectorWindow.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\ProjectBrowser\ProjectBrowser.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SerializedObject.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Undo.cs
```

Unity 的启发：

```text
面板很多，但编辑数据入口收敛到 SerializedObject / SerializedProperty / Undo。
用户心智简单：Hierarchy 选对象，Inspector 改字段，SceneView 操作对象，ProjectBrowser 管资源。
```

我们不直接照搬 Unity 的对象模型，因为本项目底层是 Rust ECS / Schema / RuntimePackage。
但需要学习 Unity 的工作台心智和统一修改入口。

### 3.2 Unreal Engine

UE 对应结构：

```text
LevelEditor
SLevelViewport
SceneOutliner
DetailsView / PropertyEditor
ContentBrowser
FUICommandList
FScopedTransaction
```

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\LevelEditor\Private\SLevelEditor.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\LevelEditor\Private\SLevelViewport.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\LevelEditor\Private\LevelEditor.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\LevelEditor\Private\LevelEditorActions.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\PropertyEditor\Private\SDetailsView.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\ContentBrowser\Private\SContentBrowser.cpp
```

UE 的启发：

```text
复杂编辑器必须有中心化 Command / Transaction 边界。
Viewport / Details / Outliner / ContentBrowser 不应该绕过 Transaction 直接改核心数据。
```

我们不照搬 UE 的 Slate / UObject / Module 重体系。
但需要学习 UE 的命令列表、事务边界、编辑器工作区组织方式。

### 3.3 Godot

Godot 对应结构：

```text
EditorNode
SceneTreeDock
EditorInspector
FileSystemDock
EditorUndoRedoManager
```

本地源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\editor\editor_node.h
<GODOT_SOURCE>\godot-master\godot-master\editor\docks\scene_tree_dock.h
<GODOT_SOURCE>\godot-master\godot-master\editor\docks\scene_tree_dock.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\inspector\editor_inspector.h
<GODOT_SOURCE>\godot-master\godot-master\editor\docks\filesystem_dock.h
<GODOT_SOURCE>\godot-master\godot-master\editor\editor_undo_redo_manager.h
```

Godot 的启发：

```text
结构可以短。
SceneTreeDock / Inspector / FileSystemDock 都围绕 EditorNode 和 EditorUndoRedoManager 工作。
第一版不需要过度抽象成 UE 那样重。
```

## 4. 方案对比

### 方案 A：继续小系统逐个补

```text
Native Editor Inspector TextInput / Field Editing
Native Editor SceneView picking / selection feedback
Native Editor ProjectDock / Asset browser real interaction
```

优点：

```text
短期容易施工。
每个点都能独立测试。
```

缺点：

```text
容易形成多套编辑入口。
AI 需要理解太多面板级规则。
后期 Debug 时很难判断一次修改来自哪个真相。
```

结论：

```text
不采用。
这些小系统可以作为 Workspace 内部落地任务，但不能作为独立架构主线。
```

### 方案 B：把所有交互继续塞进 NativeEditorApplication

优点：

```text
层级最少。
短期代码直观。
```

缺点：

```text
Application Shell 会膨胀成上帝对象。
Shell 同时负责窗口、输入、布局、编辑、AI、事务，会破坏边界。
```

结论：

```text
不采用。
121 Application Shell 只做应用壳和事件主循环，不承担完整 authoring 语义。
```

### 方案 C-min：Editor Authoring Workspace

优点：

```text
把所有编辑行为收敛到一个工作台。
对 AI 友好，AI 只需要读 WorkspaceContext 并生成 WorkspaceCommand。
对复杂项目友好，修改入口稳定。
规则数量可控，不把每个面板变成独立真相。
```

缺点：

```text
需要先定义 WorkspaceState / WorkspaceCommand / WorkspaceContext。
第一版施工比单独做 TextInput 更大。
```

结论：

```text
采用方案 C-min。
```

## 5. 推荐方案

目标结构：

```text
NativeEditorApplication
  -> EditorMainFrame
  -> EditorAuthoringWorkspace
      -> WorkspaceState
      -> WorkspaceSelection
      -> WorkspaceContext
      -> WorkspaceCommandRouter
      -> WorkspaceTransactionAdapter
      -> WorkspaceDiagnostics
      -> WorkspaceUiProjection
  -> EditorSession
  -> EditorUiModelComposer
```

核心数据流：

```text
WindowEvent / HitRegion / AI Proposal / Test Command
  -> WorkspaceInput
  -> WorkspaceCommand
  -> validate command against WorkspaceState
  -> EditorSession
  -> SceneEditTransaction / CommandTransaction
  -> EditorSceneDocument
  -> Selection / Dirty / Diagnostics
  -> WorkspaceContext refresh
  -> EditorUiModel rebuild
```

## 6. 核心规则

### 6.1 编辑真相

```text
EditorSceneDocument 是编辑真相。
Runtime World 不是编辑真相。
PreviewWorld / RuntimePreview 只是预览结果，可以重建。
UI Model 不是编辑真相。
AI Proposal 不是编辑真相。
```

### 6.2 修改入口

所有会改变项目编辑数据的动作必须进入：

```text
WorkspaceCommand
  -> EditorSession
  -> Transaction
```

禁止：

```text
Inspector 直接写 SceneDocument。
Hierarchy 直接写 SceneDocument。
Viewport 直接写 SceneDocument。
ProjectDock 直接写 SceneDocument。
AI Panel 直接写 SceneDocument。
Test 绕过 Transaction 写 SceneDocument。
```

### 6.3 面板职责

面板只负责：

```text
展示 WorkspaceContext。
产生 WorkspaceCommand。
显示 Diagnostics / Report。
```

面板不负责：

```text
保存项目真相。
直接修改 Runtime World。
直接修改 ECS。
绕过 Transaction 修改 Scene。
```

### 6.4 AI 规则

AI Panel 第一版只允许：

```text
读取 WorkspaceContext。
生成 proposed WorkspaceCommand list。
解释风险。
等待用户确认。
确认后通过 WorkspaceCommandRouter 执行。
```

AI Panel 禁止：

```text
直接写文件。
直接写 Runtime World。
直接写 ECS。
绕过 WorkspaceCommand。
绕过 Transaction。
```

## 7. C-min 范围

第一版必须支持：

```text
Hierarchy:
  select entity
  create empty entity
  delete entity
  rename entity

Inspector:
  show selected entity
  edit Transform localPosition / localRotation / localScale
  edit bool / number / string / Vec2 / Vec3 / AssetRef / Json basic field

Viewport:
  show selected entity feedback
  simple entity selection by existing hit region / proxy id

ProjectDock:
  select asset
  place asset into scene as generic entity with AssetRef component

AI Panel:
  read current WorkspaceContext
  produce proposed WorkspaceCommand list
  execute only after confirm

UndoRedo:
  every mutation creates Transaction
  undo / redo refreshes WorkspaceContext and UiModel

Diagnostics:
  invalid command emits diagnostic
  successful command emits minimal trace/report entry
```

第一版不做：

```text
完整 Dock 拖拽
完整 SceneView Gizmo
多选批量编辑
复杂右键菜单
复杂 Inspector 自定义控件
Prefab Mode
材质编辑器
动画编辑器
蓝图/图表编辑器
项目玩法规则
真实 AI provider 接入
```

## 8. 标准结构

### 8.1 WorkspaceState

```text
WorkspaceState
  active_scene_id
  active_panel_id
  focused_panel_id
  hovered_panel_id
  selection
  selected_asset_ref
  active_tool
  dirty_state
  last_command_id
  diagnostics_summary
```

第一版 `selection` 只需要：

```text
WorkspaceSelection
  selected_entity_ids[]
  primary_entity_id
  selected_asset_refs[]
```

### 8.2 WorkspaceContext

`WorkspaceContext` 是给 UI、AI、测试读取的结构化上下文。

```text
WorkspaceContext
  scene_summary
  hierarchy_summary
  selection_summary
  inspector_summary
  project_asset_summary
  console_summary
  diagnostics_summary
  allowed_command_schema
```

规则：

```text
AI 默认只读 WorkspaceContext。
WorkspaceContext 必须是结构化数据，不是 UI 截图文本。
WorkspaceContext 不包含完整项目文件内容，只包含当前编辑所需摘要。
```

### 8.3 WorkspaceCommand

第一版命令：

```text
WorkspaceCommand
  SelectEntity(entity_id)
  CreateEntity(parent_id?, name?)
  DeleteEntity(entity_id)
  RenameEntity(entity_id, new_name)
  SetTransformField(entity_id, field_path, value)
  SetComponentField(entity_id, component_type, field_path, value)
  SelectAsset(asset_ref)
  PlaceAssetInScene(asset_ref, parent_id?, transform?)
  AcceptAiProposal(proposal_id)
  Undo
  Redo
```

命名规则：

```text
命令保持编辑器通用语义。
不出现 player / enemy / bullet / health / score / wave 等项目词。
项目词只能作为 project schema 中的数据值出现。
```

### 8.4 WorkspaceDiagnostics

第一版字段：

```text
WorkspaceDiagnostic
  severity
  code
  message
  command_id
  panel_id
  entity_id?
  asset_ref?
```

第一版不要求复杂错误树。

## 9. 与已有系统关系

### 9.1 与 121 Application Shell

```text
121 负责窗口、主循环、事件入口、present。
122 负责编辑工作台语义。
```

边界：

```text
NativeEditorApplication 可以持有 EditorAuthoringWorkspace。
EditorAuthoringWorkspace 不依赖 winit / wgpu。
```

### 9.2 与 105 Editor Authoring System

```text
105 是总的 authoring 闭环规则。
122 是 Native Editor 内部的 authoring workspace 落地。
```

105 更偏项目制作链路：

```text
User / AI / Test
  -> AuthoringRequest / UiCommandPayload
  -> EditorSession
  -> SceneEditCommand
  -> SceneEditTransaction
  -> Save / Play / Report
```

122 更偏编辑器工作台：

```text
Hierarchy / Inspector / Viewport / ProjectDock / AI Panel
  -> WorkspaceCommand
  -> WorkspaceContext
  -> UiModel rebuild
```

### 9.3 与 120 Usable Panels

```text
120 定义哪些面板第一版可用。
122 定义这些面板如何共享一个编辑工作台真相。
```

### 9.4 与 Scene Editing / ProjectAsset Authoring

122 不重写：

```text
85-Scene-Editing-v1-C-min方案.md
86-真实UI命令接入SceneEditing-C-min方案.md
90-ProjectAsset-to-SceneEntity-Authoring-C-min方案.md
```

122 只把这些能力收敛到 WorkspaceCommand 下。

## 10. 最小验收用例

### 用例 1：Hierarchy 创建并选择 Entity

```text
用户点击 Create Entity
  -> WorkspaceCommand::CreateEntity
  -> Transaction
  -> EditorSceneDocument 新增 Entity
  -> WorkspaceSelection 选中新 Entity
  -> Hierarchy / Inspector 刷新
```

验收：

```text
Hierarchy 出现新 Entity。
Inspector 显示新 Entity。
Undo 后 Entity 消失。
Redo 后 Entity 恢复。
Diagnostics 无 error。
```

### 用例 2：Inspector 修改 Transform

```text
用户在 Inspector 修改 localPosition.x
  -> WorkspaceCommand::SetTransformField
  -> Transaction
  -> EditorSceneDocument 修改 Transform
  -> PreviewWorld / Viewport 可刷新
```

验收：

```text
Inspector 显示新值。
Undo 后恢复旧值。
Viewport selection feedback 仍指向同一 Entity。
```

### 用例 3：ProjectDock 放资源进 Scene

```text
用户选择 sprite asset
  -> WorkspaceCommand::SelectAsset
用户点击 Place In Scene
  -> WorkspaceCommand::PlaceAssetInScene
  -> Transaction
  -> SceneDocument 创建 Entity + AssetRef component
```

验收：

```text
Hierarchy 出现新 Entity。
Inspector 显示 AssetRef。
保存后 RuntimePackageBuilder 可以读取同一 AssetRef。
```

### 用例 4：AI 生成受控编辑

```text
用户输入：创建一个使用当前资源的实体
  -> AI Panel 读取 WorkspaceContext
  -> proposed WorkspaceCommand::PlaceAssetInScene
  -> 用户确认
  -> WorkspaceCommandRouter 执行
```

验收：

```text
AI 不直接写文件。
AI 不直接改 RuntimeWorld。
命令可显示、可确认、可回滚。
```

## 11. 结论

本项目确认采用：

```text
Editor Authoring Workspace C-min
```

它把 Inspector、Hierarchy、Viewport、ProjectDock、AI Panel 的编辑能力收敛到统一工作台。

后续不得再把以下系统作为独立架构主线重新讨论：

```text
Native Editor Inspector TextInput / Field Editing
Native Editor SceneView picking / selection feedback
Native Editor ProjectDock / Asset browser real interaction
AI Panel editor command execution
```

这些都应该作为 `122-Editor Authoring Workspace C-min` 的内部施工阶段处理。

