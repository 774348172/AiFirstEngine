# 141-M8 Schema-driven Inspector / Details System C-full 方案

## 1. 本文解决什么

本文定义 `M8 Schema-driven Inspector / Details System C-full`。

它不是重新做一个 Inspector 面板，也不是推翻已完成的：

```text
123-Workspace-Property-Editing-Inspector-Framework-v1方案.md
```

`123` 已经确定了基础：

```text
InspectorModel
  -> PropertyTree
  -> PropertyPath
  -> PropertyValue
  -> PropertyEditBuffer
  -> PropertyEditCommand
  -> UiCommand / Transaction
```

M8 要补齐的是完整 Unity / UE 级属性编辑架构：

```text
Selection
  -> InspectableTarget
  -> Schema / Reflection
  -> PropertyTree
  -> PropertyHandle
  -> PropertyEditorWidget
  -> TransactionRouter
  -> Scene / Prefab / Asset / Rule / AUI / Input
  -> Undo / Dirty / Save / Report
```

本文采用：

```text
C-full architecture
C-staged implementation
```

含义：

```text
架构一次到位。
施工分阶段完成。
后续只是补控件、插件、目标类型和 report，不重新推翻 PropertyTree / PropertyHandle / TransactionRouter。
```

## 2. 引擎边界

Inspector 是引擎底座能力，只处理：

```text
target
schema
field
path
value
handle
widget
command
transaction
diagnostic
```

不允许把以下项目侧概念做成 Inspector 内置 API：

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

项目侧语义必须通过：

```text
Project Schema
Project Component
Project Rule
Prefab
AUI
Input Mapping
Asset Metadata
```

来表达。

## 3. 其它引擎对应模块

### 3.1 Unity

Unity 对应模块：

```text
InspectorWindow
SerializedObject
SerializedProperty
PropertyDrawer
CustomEditor
Undo / Dirty
Prefab Override
```

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector\Core\InspectorWindow.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SerializedObject.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SerializedProperty.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector
```

可借鉴：

```text
用户看到很多 Inspector 控件，但写入统一走 SerializedProperty。
Prefab Override 与字段编辑深度集成。
Undo / Dirty / Save 不由控件自己处理。
PropertyDrawer / CustomEditor 只扩展显示和编辑体验。
```

不照搬：

```text
Unity native 黑盒较多，不适合 AI-first 调试。
我们的 PropertyTree / PropertyHandle / InspectorReport 必须是结构化、可序列化、AI 可读。
```

### 3.2 Unreal Engine

UE 对应模块：

```text
DetailsView
PropertyEditorModule
IPropertyHandle
IDetailCustomization
Transaction
Actor / Component / Blueprint defaults / Instance override
```

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\PropertyEditor\Private\SDetailsView.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\PropertyEditor\Public\PropertyHandle.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\PropertyEditor\Private\UserInterface\PropertyEditor
```

可借鉴：

```text
复杂编辑器能力围绕 PropertyHandle 工作。
DetailsView 支持不同 target 类型。
IDetailCustomization 扩展 UI，不应该绕过 transaction。
Override 状态可以作为 Details 行的一部分显示。
```

不照搬：

```text
UE UObject / Reflection / Slate Details 体系很重。
我们不引入 UObject，也不把 Inspector 做成 C++ 宏反射体系。
```

### 3.3 Godot

Godot 对应模块：

```text
Object::get_property_list
PropertyInfo
EditorInspector
EditorProperty
EditorInspectorPlugin
Object::get / Object::set
```

本地源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\editor\editor_inspector.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\editor_inspector.h
<GODOT_SOURCE>\godot-master\godot-master\core\object\object.cpp
```

可借鉴：

```text
PropertyInfo 心智简单。
对象暴露属性列表，Inspector 根据类型和 hint 生成控件。
EditorInspectorPlugin 扩展编辑体验。
```

不照搬：

```text
我们底层不是 Godot Node/Object 模型。
我们需要 Schema / ECS / Prefab / Asset / AUI / Rule 多目标统一 Inspector。
```

### 3.4 Bevy

Bevy 没有成熟官方编辑器，但对应底层方向是：

```text
Reflect
TypeRegistry
DynamicScene
World serialization
```

本地源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_reflect
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_world_serialization
```

可借鉴：

```text
ECS 数据适合通过 schema / reflection 暴露给编辑器。
动态组件和场景序列化需要稳定字段路径。
```

不照搬：

```text
Bevy 没有完整 Unity / UE 级 Inspector 产品工作流。
```

## 4. 方案对比

### 4.1 方案 A：继续手写 Inspector

```text
Transform 手写
Mesh 手写
SpriteRenderer2D 手写
Collider2D 手写
Prefab 手写
AUI 手写
Input 手写
```

优点：

```text
短期最快。
```

缺点：

```text
每个系统都会新增一套字段 UI。
AI 要理解大量特殊规则。
复杂项目字段增长后不可维护。
Prefab Override / Undo / Save / Report 难统一。
```

不推荐。

### 4.2 方案 B：Schema -> PropertyTree -> Command

```text
ComponentSchema
  -> PropertyTree
  -> InspectorModel
  -> PropertyEditCommand
```

优点：

```text
简单。
比手写字段强很多。
能继承 123 已有结构。
```

缺点：

```text
缺少 PropertyHandle，复杂 target 和插件扩展会变弱。
多目标、Prefab override、Asset / AUI / Rule 等不同写入后端容易挤进 PropertyTree。
后期更接近 Unity / UE 时可能要补架构层。
```

不作为最终路线。

### 4.3 方案 C：完整 Details System

```text
InspectableTarget
  -> Schema / Reflection
  -> PropertyTree
  -> PropertyHandle
  -> PropertyEditorWidget
  -> InspectorPlugin
  -> TransactionRouter
  -> Report
```

优点：

```text
最接近 Unity / UE。
复杂项目能力强。
AI 可以通过 PropertyTree / PropertyHandle / Report 查问题。
新增 target 类型不会推翻 Inspector。
插件只能扩展显示和编辑方式，不绕开真相层。
```

缺点：

```text
第一版架构复杂度最高。
需要严格限制第一版施工边界，否则容易陷入控件细节。
```

推荐。

## 5. 推荐方案

采用：

```text
方案 C：Schema-driven Inspector / Details System C-full
```

但施工采用：

```text
C-staged implementation
```

第一版必须建立长期核心层：

```text
InspectableTarget
ComponentSchema / ObjectSchema
PropertyTree
PropertyHandle
PropertyEditorWidget model
InspectorPlugin descriptor
TransactionRouter
InspectorReport
```

第一版可以不做完全部真实 UI 控件，但不能再用临时架构绕过这些核心层。

## 6. 核心数据结构

### 6.1 InspectableTarget

统一 Inspector 可检查对象：

```text
InspectableTarget:
  SceneEntity(entity_id)
  PrefabAsset(prefab_id)
  PrefabInstance(instance_id)
  Asset(asset_id)
  ProjectRule(rule_id)
  AuiDocument(aui_id)
  InputMapping(mapping_id)
  BuildProfile(profile_id)
  RuntimeObjectReadonly(runtime_entity_id)
```

规则：

```text
Inspector 不再只服务 Entity。
不同 target 的读写差异由 PropertyHandle / TransactionRouter 处理。
RuntimeObjectReadonly 只能读，不能写。
```

### 6.2 ComponentSchema / ObjectSchema

Schema 是字段来源。

```text
ComponentSchema:
  schema_id
  component_type
  display_name
  fields: FieldSchema[]
  version
```

```text
ObjectSchema:
  schema_id
  target_kind
  display_name
  fields: FieldSchema[]
  version
```

```text
FieldSchema:
  field_path
  label
  value_type
  editor_kind
  readonly
  default_value
  constraints
  enum_options
  asset_filter
  tooltip
  category
  order
```

规则：

```text
AI 生成项目组件时必须生成 ComponentSchema。
Inspector 不猜项目字段语义。
没有 Schema 的组件走 JsonFallback，并产生 warning。
```

### 6.3 PropertyTree

PropertyTree 是 Inspector 的结构化视图。

```text
PropertyTree:
  target
  sections
  diagnostics
```

```text
PropertySection:
  section_id
  title
  groups
  properties
```

```text
PropertyNode:
  property_id
  property_path
  label
  value
  value_type
  editor_kind
  metadata
  children
```

规则：

```text
PropertyTree 不是真相层。
PropertyTree 可以缓存，但必须能从 target + schema + source data 重建。
AI / Trace / Report 默认读 PropertyTree summary。
```

### 6.4 PropertyHandle

PropertyHandle 是方案 C 的关键。

```text
PropertyHandle:
  handle_id
  target
  property_path
  component_type
  value_type
  editor_kind
  read()
  validate(value)
  write(value)
  reset()
  diff_default()
  override_state()
```

它对应：

```text
Unity SerializedProperty
UE IPropertyHandle
Godot PropertyInfo + Object get/set 的编辑器侧封装
```

规则：

```text
PropertyHandle 是唯一字段读写入口。
UI 控件不直接写 Scene / Prefab / Asset / Rule / AUI / Input。
PropertyHandle 不持有 UI 状态，只描述字段读写能力和状态。
```

### 6.5 PropertyEditorWidget

第一版架构必须支持完整控件类型：

```text
TextInput
MultilineText
RichText
Number
Slider
Toggle
Vec2
Vec3
Vec4
ColorPicker
Enum
AssetRefPicker
EntityRefPicker
ArrayEditor
ObjectEditor
CurveEditor
JsonFallback
CustomInspectorPlugin
Readonly
```

第一版施工可以先实现基础渲染 / 命令模型：

```text
String
Bool
Number
Vec3
Enum
AssetRef
EntityRef
JsonFallback
Array / Object 最小展开
```

后续补：

```text
完整 IME
RichText
ColorPicker
CurveEditor
数组拖拽重排
完整插件 UI
```

### 6.6 InspectorPluginDescriptor

插件只能扩展显示和编辑体验。

```text
InspectorPluginDescriptor:
  plugin_id
  target_kind_filter
  component_type_filter
  path_prefix_filter
  provided_editor_kind
  allowed_commands
```

规则：

```text
插件不能绕过 PropertyHandle。
插件不能直接写 SceneDocument / PrefabAsset / AssetDB。
插件输出 PropertyEditCommand 或 PropertyWidgetAction。
```

## 7. 标准流程

### 7.1 读流程

```text
Selection
  -> InspectableTarget
  -> SchemaRegistry resolve schema
  -> PropertyHandleRegistry create handles
  -> PropertyTreeBuilder build tree
  -> InspectorModel
  -> UI render
```

### 7.2 写流程

```text
UI Field Edit
  -> PropertyEditorWidget event
  -> PropertyHandle validate
  -> PropertyEditCommand
  -> TransactionRouter
      -> SceneEditCommand
      -> PrefabOverrideCommand
      -> AssetEditCommand
      -> RuleEditCommand
      -> AuiEditCommand
      -> InputEditCommand
  -> Undo / Dirty / Save marker
  -> InspectorReport
```

### 7.3 Prefab Override 流程

```text
PrefabInstance selected
  -> PropertyHandle detects prefab source
  -> edit field
  -> PrefabOverride
  -> ResolvedPrefabView refresh
  -> Inspector override state update
```

Override 状态：

```text
Inherited
Overridden
MissingSource
InvalidOverride
NotApplicable
```

## 8. TransactionRouter

`TransactionRouter` 是 Inspector 写入唯一出口。

```text
PropertyEditCommand
  -> TransactionRouter.route(command)
```

目标路由：

```text
SceneEntity -> SceneEditCommand
PrefabInstance -> PrefabOverride
PrefabAsset -> PrefabAssetEditCommand
Asset -> AssetEditCommand
ProjectRule -> RuleEditCommand
AuiDocument -> AuiEditCommand
InputMapping -> InputEditCommand
RuntimeObjectReadonly -> reject readonly
```

规则：

```text
Inspector 不直接写任何真相层。
所有写入必须产生 CommandResult / Transaction / Diagnostic。
```

## 9. Diagnostics / Report

第一版最小报告：

```text
InspectorReport:
  schema_version
  selected_target
  property_count
  editable_count
  readonly_count
  invalid_schema_count
  failed_edit_count
  override_count
  diagnostics[]
```

```text
InspectorDiagnostic:
  severity
  code
  target
  schema_id
  component_type
  field_path
  command_id
  message
```

错误码：

```text
missing_schema
invalid_schema
missing_property
readonly_property
invalid_value
unsupported_editor_kind
plugin_rejected
transaction_route_missing
write_failed
prefab_override_failed
```

## 10. 第一版施工边界

M8 第一版必须完成：

```text
C1 Schema / PropertyHandle / PropertyTree 完整骨架
C2 基础控件模型：String / Bool / Number / Vec3 / Enum / AssetRef / EntityRef / JsonFallback
C3 Array / Object 最小展开编辑模型
C4 Prefab Override 状态与写入接入
C5 InspectorReport
C6 TransactionRouter 最小可用
C7 模块测试和整体回归
```

M8 第一版不要求完成真实高级 UI：

```text
完整 IME UI
完整 RichText UI
完整 ColorPicker UI
完整 CurveEditor UI
完整自定义 Inspector 插件热加载
完整多对象编辑
完整数组拖拽重排
```

但上述能力的 enum / descriptor / report slot 必须保留，后续不能推翻架构。

## 11. 与 M7 Prefab Workflow 的关系

M7 已提供：

```text
PrefabAsset
PrefabInstance
PrefabOverride
ResolvedPrefabView
PrefabWorkflowReport
```

M8 必须接入：

```text
PrefabInstance target
PropertyHandle override_state()
TransactionRouter -> PrefabOverride
InspectorReport override_count
```

规则：

```text
Prefab override 是 Inspector 写入后端之一。
不要在 Inspector 内重新发明 Prefab 系统。
```

## 12. 验收测试

第一版必须有：

```text
SchemaRegistry can register and resolve ComponentSchema.
PropertyTreeBuilder builds tree from schema and target data.
PropertyHandle reads value by PropertyPath.
PropertyHandle validates invalid value.
TransactionRouter routes SceneEntity edit to SceneEditCommand.
TransactionRouter routes PrefabInstance edit to PrefabOverride.
Readonly runtime target rejects write.
Missing schema uses JsonFallback and reports warning.
Array / Object can be expanded into child PropertyNode.
InspectorReport counts properties / readonly / editable / failed edit.
No project gameplay terms appear in engine Inspector API.
```

整体回归：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

## 13. 方案自审

### 13.1 Specification fit

本文满足用户选择的方案 C：完整 Unity / UE 级 Inspector 架构。它明确采用 `C-full architecture / C-staged implementation`，不是方案 B。

### 13.2 Rule fit

本文遵守现有规则：

```text
继承 123，不重复重开 Property Editing。
引擎只提供底座能力，不加入玩法 API。
方案对比了 Unity / UE / Godot / Bevy。
使用自审规则。
优先 AI 友好、复杂项目、长期维护、简单度、效率。
```

### 13.3 Textual consistency

术语一致：

```text
Schema 是字段来源。
PropertyTree 是视图。
PropertyHandle 是唯一字段读写入口。
PropertyEditorWidget 是 UI 表达。
TransactionRouter 是唯一写入出口。
InspectorReport 是诊断入口。
```

不存在把 PropertyTree 当真相层，或允许插件绕过写入链路的问题。

### 13.4 Design fit

方案符合长期路线：

```text
接近 Unity SerializedProperty / UE IPropertyHandle。
对 AI 可读可诊断。
支持 Scene / Prefab / Asset / Rule / AUI / Input 多目标。
把复杂度集中在 PropertyHandle 和 TransactionRouter，而不是散到每个系统。
```

### 13.5 Implementation feasibility

当前已有：

```text
PropertyTree
PropertyPath
PropertyValue
PropertyEditBuffer
PropertyEditCommand
InspectorModel
SceneEditCommand
PrefabWorkflowService
```

因此可以增量实现 C-full 架构骨架，不需要推翻现有工程。

### 13.6 Practical reasonableness

第一版只实现 C-full 骨架和基础字段类型，不一次性做完整高级 UI，避免过度施工。

结论：

```text
方案通过自审，可以生成施工文档。
```
