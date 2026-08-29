# 86-真实 UI 命令接入 Scene Editing C-min 方案

## 定位

本方案定义真实编辑器 UI 如何接入已经完成的 Scene Editing v1 C-min 内核。

它不重新定义 Scene Editing 数据真相，也不替代 85 号方案。

已有前置规则：

```text
85-Scene-Editing-v1-C-min方案.md
EditorSceneDocument 是编辑真相。
Runtime World 不是编辑真相。
PreviewWorld 是 EditorSceneDocument 派生出来的预览结果。
所有 Scene 源数据修改必须走 SceneEditCommand -> SceneEditTransaction。
AI / UI / Test 都不能直接写 Scene 文件或 Runtime ECS。
```

本方案解决的问题是：

```text
用户在真实 UI 上点击 Hierarchy / Inspector / Toolbar / Viewport 时，如何进入 SceneEditCommand。
Inspector 字段修改什么时候提交。
Hierarchy 选择 / 删除 / 创建 / 改父子关系如何提交。
Toolbar 保存 / Undo / Redo 如何提交。
真实 UI 如何避免绕过 85 号 SceneEditTransaction。
```

## 其它引擎参考

### Unity

参考源码方向：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\UIElements\Inspector\InspectorElement.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\UIElements\Bindings\BindingsInterface.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SceneView\SceneView.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\EditorSceneManager.cs
```

Unity 的核心模式：

```text
SceneView / Hierarchy / Inspector 是用户入口。
Inspector 通过 SerializedObject / SerializedProperty 绑定对象字段。
字段变更由 Unity 内部写入对象并进入 Undo / dirty / save 体系。
Scene 保存由 EditorSceneManager 负责。
用户心智是：选中对象，在 Inspector 修改字段，SceneView 看到结果。
```

我们学习：

```text
用户操作路径必须简单。
Hierarchy / Inspector / SceneView 应该围绕同一个选中对象工作。
Inspector 字段编辑不应该要求用户理解底层 ECS。
```

我们不照搬：

```text
不照搬 SerializedObject 黑箱。
不照搬 UnityObject / native object 绑定体系。
不让 UI 直接写底层运行对象。
```

### Unreal Engine

参考源码方向：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\LevelEditor\Public\SLevelViewport.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\LevelEditorViewport.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\EditorActor.cpp
```

UE 的核心模式：

```text
Viewport / Outliner / Details 是用户入口。
编辑动作进入编辑器事务体系。
常见边界是 FScopedTransaction + Modify() + MarkPackageDirty。
不同工具可以很多，但修改必须进入统一事务和 dirty/save 体系。
```

我们学习：

```text
真实 UI 操作必须统一进入事务。
Undo / dirty / diagnostic 不能散在各个面板里。
工具可以扩展，但事务入口必须稳定。
```

我们不照搬：

```text
不照搬 UObject / Actor / Component 反射体系。
不在第一版实现完整 EditorMode / Tool Framework。
不让每个 UI 面板拥有自己的修改真相。
```

### Godot

Godot 的核心模式：

```text
SceneTree / Inspector / EditorPlugin 都是编辑器入口。
场景树和 Inspector 修改通过编辑器内部对象体系统一作用到场景资源。
EditorPlugin 可以扩展工具，但不应该破坏场景资源边界。
```

我们学习：

```text
面板是入口，不是真相。
插件 / 工具应产生编辑请求，而不是绕过核心数据模型。
```

### Bevy

Bevy 本身不是完整 Unity/UE 式编辑器，但其 ECS / Reflect / Scene 思路有参考价值：

```text
ECS 数据可以被反射和序列化。
编辑器可以通过 schema / reflect 信息生成 Inspector。
运行 World 与编辑数据需要明确边界。
```

我们学习：

```text
Inspector 后续应更多来自 Schema / Component metadata。
第一版不把 Inspector 做成复杂 schema renderer，但命令路径要为它预留。
```

## 方案对比

### 方案 A：UI 直接发 SceneEditCommand

```text
Hierarchy / Inspector / Toolbar
  -> SceneEditCommand
  -> SceneEditTransaction
```

优点：

```text
链路最短。
AI 和 UI 使用同一命令结构。
实现速度快。
```

缺点：

```text
UI 层会知道太多 Scene Editing 细节。
Inspector / Hierarchy 后续容易散落命令构造逻辑。
真实 UI、AI、测试的来源差异不清楚。
后期复杂项目里，UI 逻辑会逐渐变厚。
```

结论：

```text
不作为正式方案。
可以在测试或临时调试中使用，但真实 UI 不直接构造复杂 SceneEditCommand。
```

### 方案 B：UI 发 UiCommandPayload，EditorCore 转换成 SceneEditCommand

```text
Hierarchy / Inspector / Toolbar / Viewport
  -> UiCommandPayload
  -> EditorSession
  -> SceneEditCommand
  -> SceneEditTransaction
  -> EditorSceneDocument
```

优点：

```text
UI 层保持薄。
EditorCore 是唯一 Scene 编辑调度点。
AI / UI / Test 来源可以被统一记录。
后续真实 Inspector、Hierarchy、Viewport 工具都不会绕过 Transaction。
更适合 AI 追踪和解释问题。
```

缺点：

```text
比方案 A 多一层转换。
需要维护 UiCommandPayload 与 SceneEditCommand 的映射。
```

结论：

```text
采用。
这是 C-min 的正式路线。
```

### 方案 C：每个面板独立处理自己的编辑逻辑

```text
Hierarchy 自己改 Scene。
Inspector 自己改 Scene。
Viewport 自己改 Scene。
Toolbar 自己处理 Save / Undo。
```

优点：

```text
局部开发很快。
每个面板容易单独实现。
```

缺点：

```text
规则分散。
Undo / dirty / save 容易不一致。
AI 很难判断一次修改到底经过了哪些逻辑。
复杂项目后期极难维护。
```

结论：

```text
禁止作为正式架构。
```

## 推荐方案

采用方案 B 的 C-min：

```text
真实 UI
  -> UiCommandPayload
  -> EditorSession
  -> SceneEditCommand
  -> SceneEditTransaction
  -> EditorSceneDocument
  -> PreviewWorldSync
  -> EditorUiModel refresh
  -> Console / Diagnostic
```

核心判断：

```text
UI 负责表达用户意图。
EditorCore 负责把用户意图转换成 SceneEditCommand。
SceneEditTransaction 负责验证、应用、Undo、Dirty、Diagnostic。
EditorSceneDocument 仍然是编辑真相。
PreviewWorld 仍然只是预览结果。
```

## 第一版命令范围

第一版只新增最小真实 UI 命令：

```text
OpenSceneDocument { path }
SelectSceneEntity { entity_id }
CreateSceneEntity { parent_id optional, name }
DeleteSceneEntity { entity_id }
SetSceneTransform { entity_id, local_position optional, local_rotation optional, local_scale optional }
SetSceneComponentField { entity_id, component_type, field_path, value }
SaveSceneDocument { path optional }
UndoSceneEdit
RedoSceneEdit
```

暂不做：

```text
Duplicate UI command。
复杂 Reparent drag-drop。
复杂多选编辑。
Prefab Mode。
Asset Browser drag into Scene。
Viewport Gizmo。
复杂 Inspector schema renderer。
```

这些能力后续继续走同一条命令链路补充。

## 面板职责

### Toolbar

Toolbar 可以发：

```text
OpenSceneDocument
SaveSceneDocument
UndoSceneEdit
RedoSceneEdit
```

Toolbar 不负责：

```text
不直接保存 Scene 文件。
不直接改 DirtyState。
不直接操作 UndoStack。
```

### Hierarchy

Hierarchy 可以发：

```text
SelectSceneEntity
CreateSceneEntity
DeleteSceneEntity
```

后续可扩展：

```text
ReparentSceneEntity
DuplicateSceneEntity
```

Hierarchy 不负责：

```text
不直接改 EditorSceneDocument。
不直接维护父子树真相。
不直接刷新 PreviewWorld。
```

### Inspector

Inspector 可以发：

```text
SetSceneTransform
SetSceneComponentField
```

第一版字段提交规则：

```text
数字输入：Enter 或 blur 时提交。
Vec3 输入：单轴提交也进入 SetSceneTransform。
Checkbox / Toggle：点击即提交。
JSON 组件字段：第一版只支持单层字段。
非法字段：EditorCore 拒绝，并输出 Console diagnostic。
```

Inspector 不负责：

```text
不直接写组件 JSON。
不直接写 Transform。
不自己维护 Undo。
不自己决定 DirtyState。
```

### Viewport / SceneView

Viewport 第一版只保留输入入口，不实现真实 Gizmo。

可以发：

```text
SelectSceneEntity
```

后续可扩展：

```text
Move / Rotate / Scale tool command
Picking result -> SelectSceneEntity
Gizmo drag -> SetSceneTransform
```

Viewport 不负责：

```text
不直接写 Runtime World。
不直接写 EditorSceneDocument。
不直接发 RenderCommand 作为编辑真相。
```

### AI

AI 仍然直接生成：

```text
SceneEditCommand
```

但进入方式必须和 UI 一样由 EditorSession 执行：

```text
AI
  -> SceneEditCommand
  -> EditorSession
  -> SceneEditTransaction
```

AI 禁止：

```text
禁止直接写 Scene 文件。
禁止直接写 Runtime ECS。
禁止绕过 SceneEditTransaction。
```

## UiCommandPayload 与 SceneEditCommand 映射

第一版映射如下：

```text
UiCommandPayload::OpenSceneDocument
  -> EditorSession::open_scene_document

UiCommandPayload::SelectSceneEntity
  -> SceneEditCommand::SelectEntity

UiCommandPayload::CreateSceneEntity
  -> SceneEditCommand::CreateEntity

UiCommandPayload::DeleteSceneEntity
  -> SceneEditCommand::DeleteEntity

UiCommandPayload::SetSceneTransform
  -> SceneEditCommand::SetTransform

UiCommandPayload::SetSceneComponentField
  -> SceneEditCommand::SetComponentField

UiCommandPayload::SaveSceneDocument
  -> SceneSavePipeline

UiCommandPayload::UndoSceneEdit
  -> SceneEditCommand::Undo

UiCommandPayload::RedoSceneEdit
  -> SceneEditCommand::Redo
```

规则：

```text
UiCommandPayload 只表达用户意图。
SceneEditCommand 表达 Scene 修改语义。
UiCommandPayload 不直接修改 EditorSceneDocument。
EditorSession 是 UI 命令进入 Scene Editing 的唯一入口。
```

## Trace / Diagnostic 规则

每次 UI 触发 Scene 修改后，必须能追踪：

```text
UiCommand.command_id
UiCommand.request_id
UiCommand.source
SceneEditCommand.kind
SceneEditTransactionReport.transaction_id
read_set
write_set
diagnostics
dirty_after
preview_sync_status
```

Console 第一版显示：

```text
成功：Scene edit committed: CommandKind
失败：Scene edit rejected / failed + diagnostic code
保存成功：Saved Scene path
保存失败：Scene save failed + diagnostic code
```

## 与 85 的边界

85 已经完成：

```text
EditorSceneDocument
SceneEditCommand
SceneEditTransaction
SceneDirtyState
SceneUndoStack
PreviewWorldSync
SceneSavePipeline
EditorSession headless Scene Editing API
```

86 新增：

```text
真实 UI 命令枚举。
真实 UI 命令到 SceneEditCommand 的转换规则。
Toolbar / Hierarchy / Inspector / Viewport 的最小编辑入口。
UI model 与真实输入事件之间的 Scene Editing 接入边界。
```

86 不新增：

```text
不新增第二套 SceneDocument。
不新增第二套 Undo。
不新增第二套 DirtyState。
不新增第二套 PreviewWorld。
不重新设计 SceneEditTransaction。
```

## 与其它引擎对比

| 项目 | Unity | UE | Godot | 我们 |
|---|---|---|---|---|
| 用户入口 | SceneView / Hierarchy / Inspector | Viewport / Outliner / Details | SceneTree / Inspector / Dock | Toolbar / Hierarchy / Inspector / Viewport |
| 字段编辑 | SerializedObject / SerializedProperty | Details Panel + UObject property | Inspector property editor | UiCommandPayload -> SceneEditCommand |
| 事务 | Undo / dirty 内置 | FScopedTransaction / Modify / MarkDirty | EditorUndoRedoManager | SceneEditTransaction |
| 编辑真相 | Scene / Object | World / Actor / Package | PackedScene / Node tree | EditorSceneDocument |
| AI 友好 | 弱 | 弱 | 中 | 强，命令和报告结构化 |
| 第一版复杂度 | 成熟系统 | 成熟系统 | 成熟系统 | C-min，只接最小命令 |

## 正式规则

```text
1. 真实 UI 接入 Scene Editing 采用方案 B：UiCommandPayload -> EditorSession -> SceneEditCommand。
2. UI 层只表达用户意图，不直接写 EditorSceneDocument。
3. EditorSession 是真实 UI 进入 Scene Editing 的唯一调度入口。
4. Scene 源数据修改仍然必须走 SceneEditTransaction。
5. Toolbar 不直接保存 Scene 文件，保存必须走 SceneSavePipeline。
6. Hierarchy 不直接改 Entity 树，选择 / 创建 / 删除必须转为 SceneEditCommand。
7. Inspector 不直接写 Transform 或组件字段，字段修改必须转为 SceneEditCommand。
8. Viewport 第一版只接选择命令，不实现真实 Gizmo。
9. AI 仍然只生成 SceneEditCommand，但必须由 EditorSession 执行。
10. 所有 UI 触发的 Scene 修改都必须产生 TransactionReport 或 Console diagnostic。
11. 第一版只做 Open / Select / Create / Delete / SetTransform / SetComponentField / Save / Undo / Redo。
12. 第一版不做 Prefab Mode、复杂多选、复杂 reparent drag-drop、Asset drag into Scene、真实 Gizmo。
```

## 后续施工边界建议

如果进入施工，施工文档范围应是：

```text
扩展 editor_ui_model::UiCommandPayload。
扩展 editor_core::EditorSession::execute_command。
把现有 headless Scene Editing API 收敛到真实 UiCommandPayload 路径。
补 Toolbar / Hierarchy / Inspector 最小命令测试。
补 UI model 刷新测试。
补 Console diagnostic 测试。
```

不应在这一轮施工中做：

```text
真实 Gizmo。
复杂 picking。
复杂 Inspector schema renderer。
Prefab Mode。
Asset Browser drag into Scene。
多选批量编辑。
```

---

## 2026-06-28 更新：86 真实 UI 命令接入 SceneEditing C-min 已完成

- 施工文档已归档到 `施工文档/已完成/86-当前可自动化施工文档-真实UI命令接入SceneEditing-C-min.md`。
- 阶段记录见 `阶段完成记录/2026-06-28-真实UI命令接入SceneEditing-C-min/00-总览.md`。
- 已完成真实 UI 命令到 `EditorSession -> SceneEditTransaction` 的 C-min 闭环。
- 已通过 `cargo test --workspace`。
