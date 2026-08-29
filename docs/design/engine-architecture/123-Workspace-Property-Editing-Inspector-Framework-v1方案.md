# 123-Workspace Property Editing / Inspector Framework v1 方案

## 1. 定位

本方案确认 Native Editor 下一阶段采用：

```text
Workspace Property Editing / Inspector Framework v1
```

它替代之前较窄的：

```text
Workspace Field Editing / TextInput C-min
```

原因是当前目标已经升级为完整 Inspector 属性编辑框架，必须覆盖：

```text
完整 IME
多行富文本
复杂数组编辑器
复杂对象嵌套编辑器
颜色选择器
动画曲线
自定义 Inspector 插件
```

但它仍然必须继承 122 的边界：

```text
它是 EditorAuthoringWorkspace 的内部编辑能力。
它不是独立 Inspector TextInput 架构。
它不是项目玩法规则。
它不绕过 EditorSession / Transaction。
```

## 2. 设计问题

如果把 TextInput、颜色选择器、数组编辑器、曲线编辑器、自定义 Inspector 插件分别做成独立系统，会出现：

```text
每种控件一套写入规则。
每种控件一套 Undo / Dirty / Save 处理。
AI 需要理解很多面板细节。
复杂项目后期很难追踪字段到底从哪里被改。
```

所以本方案的核心目标是：

```text
控件可以复杂。
写入入口必须统一。
```

统一真相：

```text
EditorSceneDocument / AssetDocument 是编辑真相。
PropertyTree 是 Inspector 可编辑视图。
PropertyPath 是字段地址。
PropertyValue 是字段值。
PropertyEditCommand 是字段修改意图。
WorkspaceCommand / EditorSession / Transaction 是唯一提交边界。
```

## 3. 成熟引擎参考

### 3.1 Unity

Unity 对应路线：

```text
InspectorWindow
EditorGUI / UI Toolkit
SerializedObject / SerializedProperty
PropertyDrawer / CustomEditor
ApplyModifiedProperties
Undo / Dirty / Save
```

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector\Core\InspectorWindow.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\GUI
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector
```

Unity 的启发：

```text
Inspector 控件很多，但写入不应该散。
SerializedProperty 是字段路径和字段值的统一桥。
PropertyDrawer / CustomEditor 可以换显示方式，但最终仍要进入序列化对象修改、Undo、Dirty。
```

### 3.2 Unreal Engine

UE 对应路线：

```text
DetailsView
PropertyEditor Widget
IPropertyHandle
PropertyCustomization / DetailCustomization
NotifyPreChange / NotifyPostChange
FScopedTransaction
```

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\PropertyEditor\Private\SDetailsView.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\PropertyEditor\Public\PropertyHandle.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\PropertyEditor\Private\UserInterface\PropertyEditor
```

UE 的启发：

```text
复杂 Details 面板必须围绕 PropertyHandle 工作。
自定义控件可以很复杂，但不能绕过 PropertyHandle / Transaction。
交互式修改和最终提交需要区分，但第一版可以先只做提交型修改。
```

### 3.3 Godot

Godot 对应路线：

```text
EditorInspector
EditorProperty
EditorInspectorPlugin
emit_changed(property, value, field, changing)
EditorUndoRedoManager
update_property
```

本地源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\editor\inspector\editor_inspector.h
<GODOT_SOURCE>\godot-master\godot-master\editor\inspector\editor_inspector.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\inspector\editor_properties.cpp
```

Godot 的启发：

```text
结构可以比 UE 短。
属性控件通过统一 changed 事件进入 UndoRedo。
自定义 Inspector 插件是扩展显示和编辑方式，不是绕开编辑器真相。
```

## 4. 方案对比

### 方案 A：继续 TextInput C-min

优点：

```text
最快能编辑数字和字符串。
短期施工量小。
```

缺点：

```text
无法覆盖完整 IME、富文本、数组、嵌套对象、颜色、曲线、自定义 Inspector。
后面会被迫反复加旁路规则。
```

结论：

```text
不采用。
```

### 方案 B：每种复杂控件各自接命令

优点：

```text
每个控件能独立推进。
局部实现简单。
```

缺点：

```text
控件越多，写入规则越多。
AI 难以判断一个字段到底应该走哪个提交路径。
复杂项目后期维护风险高。
```

结论：

```text
不采用。
```

### 方案 C-full：统一 Property Editing Framework

优点：

```text
长期结构正确。
控件复杂度被收敛到 PropertyEditorKind / PropertyEditCommand。
AI 只需要理解 PropertyTree / PropertyPath / PropertyValue / Transaction。
复杂项目可以通过 PropertyMetadata 和 InspectorPlugin 扩展，不扩张底层规则。
```

缺点：

```text
第一版施工量比 TextInput C-min 大。
需要先建立 PropertyTree 和 EditBuffer 骨架。
```

结论：

```text
采用。
施工仍然按模块逐步完成，每个模块完成后测试。
```

## 5. 标准结构

```text
EditorSceneDocument / AssetDocument
  -> PropertySource
  -> PropertyTree
  -> PropertyNode
  -> PropertyPath
  -> PropertyValue
  -> PropertyMetadata
  -> PropertyEditorKind

Inspector UI
  -> PropertyEditorWidget
  -> PropertyEditBuffer
  -> PropertyEditCommand
  -> WorkspaceCommand
  -> EditorSession
  -> Transaction
  -> Document
  -> EditorUiModel rebuild
```

第一版 Rust 数据结构方向：

```text
PropertyPath
PropertyTree
PropertyNode
PropertyValue
PropertyValueType
PropertyEditorKind
PropertyMetadata
PropertyEditBuffer
PropertyEditCommand
PropertyEditCommitReport
InspectorPluginDescriptor
```

## 6. 统一规则

### 6.1 控件不直接写 Document

```text
TextInput / RichText / ArrayEditor / ObjectEditor / ColorPicker / CurveEditor / CustomInspectorPlugin
都不能直接写 EditorSceneDocument。
```

它们只能产生：

```text
PropertyEditCommand
```

然后由 Workspace 转换为：

```text
UiCommandPayload::SetSceneTransform
UiCommandPayload::SetSceneComponentField
```

未来 AssetDocument 字段编辑也走同一套 PropertyEditCommand，再映射到 Asset 编辑命令。

### 6.2 所有字段必须有 PropertyPath

示例：

```text
transform.localPosition
transform.localRotation
transform.localScale
components.SpriteRenderer2D.sprite
components.SpriteRenderer2D.visible
components.CustomStats.attack
components.Inventory.items
```

PropertyPath 是 AI、Trace、Undo、Diagnostics、Save / Reload 对齐字段的关键地址。

### 6.3 PropertyValue 是统一字段值

第一版支持：

```text
String
Bool
Number
Vec3
Color
AssetRef
EntityRef
Array
Object
Curve
RichText
Json
Empty
```

复杂控件不新增底层写入规则，只新增 Value 类型和 EditorKind。

### 6.4 自定义 Inspector 插件只扩展显示和编辑方式

插件可以声明：

```text
适配哪些 component_type / property_path / value_type。
使用哪种 PropertyEditorKind。
允许产生哪些 PropertyEditCommand。
```

插件不允许：

```text
直接改 EditorSceneDocument。
直接写文件。
直接改 Runtime World。
绕过 Transaction。
```

## 7. 完整能力边界

v1 架构必须覆盖：

```text
完整 IME：TextCompositionState -> FocusedPropertyEditor -> EditBuffer。
多行富文本：RichTextValue / RichTextEditBuffer。
复杂数组编辑器：Insert / Remove / Move / SetElement。
复杂对象嵌套编辑器：Object field tree / child PropertyNode。
颜色选择器：Color value / preview / commit。
动画曲线：Curve value / key add remove move set。
自定义 Inspector 插件：Plugin descriptor / allowed command gate。
```

本轮自动化施工不要求一次完成所有高级控件体验，但要求：

```text
框架类型一次定对。
基础字段编辑闭环可测试。
高级控件不能另开第二套架构。
```

## 8. AI 适配规则

AI 默认读取：

```text
PropertyTree summary
selected_entity_id
editable PropertyPath list
allowed PropertyEditCommand schema
Diagnostics
```

AI 默认生成：

```text
PropertyEditCommand
```

AI 不生成：

```text
控件内部事件。
鼠标拖动细节。
Document 直接 patch。
Runtime World 直接 patch。
```

## 9. 施工策略

施工按模块推进：

```text
A. PropertyTree / PropertyPath / PropertyValue 骨架。
B. PropertyEditBuffer / Commit / Cancel。
C. 基础字段映射到 UiCommandPayload。
D. EditorAuthoringWorkspace 接入字段提交。
E. Inspector Field hit / focus / edit report。
F. IME / RichText / Array / Object / Color / Curve / Plugin 后续逐步补齐。
```

每个模块完成后必须执行对应测试，不允许一次性写完后才回归。

## 10. 和 122 的关系

122 的结论继续有效：

```text
EditorAuthoringWorkspace 是统一制作工作台。
WorkspaceCommand 是编辑入口。
Transaction 是修改边界。
```

123 是 122 内部的属性编辑框架：

```text
Hierarchy / Viewport / ProjectDock / AI Panel 仍然归口到 Workspace。
Inspector 字段编辑也归口到 Workspace。
```

## 11. 下一步

生成施工文档：

```text
施工文档/当前/123-当前可自动化施工文档-Workspace-Property-Editing-Inspector-Framework-v1.md
```

然后按模块施工：

```text
模块 A 完成后测试。
模块 B 完成后测试。
模块 C 完成后测试。
模块 D 完成后测试。
最后统一回归。
```
