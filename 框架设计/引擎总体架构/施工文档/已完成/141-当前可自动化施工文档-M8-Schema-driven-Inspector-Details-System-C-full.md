# 141-当前可自动化施工文档：M8 Schema-driven Inspector / Details System C-full

## 1. 施工目标

按 `141-M8-Schema-driven-Inspector-Details-System-C-full方案.md` 施工第一版：

```text
C-full architecture
C-staged implementation
```

本次不是重新做 Inspector 面板，也不是推翻 `123-Workspace-Property-Editing-Inspector-Framework-v1方案.md`。

本次施工要补齐长期核心架构：

```text
InspectableTarget
ComponentSchema / ObjectSchema
PropertyHandle
PropertyTreeBuilder
PropertyEditorWidget model
InspectorPluginDescriptor
TransactionRouter
InspectorReport
PrefabOverride integration
```

## 2. 不做什么

本次不实现：

```text
完整 IME UI
完整 RichText UI
完整 ColorPicker UI
完整 CurveEditor UI
完整自定义 Inspector 插件热加载
完整多对象编辑
完整数组拖拽重排
```

不得出现以下引擎内置类型：

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

## 3. 预计涉及文件

主要涉及：

```text
rust/crates/editor_core/src/property_editing.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/prefab_workflow.rs
rust/crates/editor_ui_model/src/lib.rs
```

如果实现中需要新模块，优先：

```text
rust/crates/editor_core/src/inspector_details.rs
```

不新增第二套 Inspector 真相层。

## 4. 分阶段任务

### A0：现状审计

目标：

```text
确认现有 PropertyTree / PropertyValue / PropertyEditCommand / PrefabWorkflowService 的边界。
```

命令：

```powershell
rg -n "PropertyTree|PropertyValue|PropertyEditCommand|InspectorPluginDescriptor|PrefabWorkflowService|InspectorModel" rust/crates/editor_core/src rust/crates/editor_ui_model/src
```

输出：

```text
阶段完成记录中记录缺口。
```

测试：

```powershell
cargo test -p editor_core property_editing
cargo test -p editor_core prefab
```

### A1：InspectableTarget / SchemaRegistry

目标：

```text
新增 InspectableTarget。
新增 ComponentSchema / ObjectSchema / FieldSchema。
新增 ComponentSchemaRegistry。
```

要求：

```text
Schema 是字段来源。
缺 schema 走 JsonFallback warning。
不出现项目玩法语义。
```

测试：

```text
SchemaRegistry can register and resolve ComponentSchema.
Missing schema returns structured diagnostic.
No gameplay terms appear in schema API.
```

命令：

```powershell
cargo test -p editor_core inspector_schema
```

### A2：PropertyHandle

目标：

```text
新增 PropertyHandle / PropertyHandleState / PropertyOverrideState。
PropertyHandle 能读、校验、描述写入能力。
```

要求：

```text
PropertyHandle 是唯一字段读写入口。
UI 不直接写真相层。
```

测试：

```text
PropertyHandle reads value by PropertyPath.
PropertyHandle validates invalid value.
Readonly target rejects write.
```

命令：

```powershell
cargo test -p editor_core property_handle
```

### A3：PropertyTreeBuilder

目标：

```text
从 InspectableTarget + Schema + source data 构建 PropertyTree。
支持 Array / Object 最小展开。
支持 JsonFallback。
```

测试：

```text
PropertyTreeBuilder builds tree from schema and target data.
Array / Object expands into child PropertyNode.
Missing schema uses JsonFallback and reports warning.
```

命令：

```powershell
cargo test -p editor_core property_tree
```

### A4：PropertyEditorWidget model

目标：

```text
补齐 PropertyEditorKind / Widget descriptor。
支持基础控件模型：String / Bool / Number / Vec3 / Enum / AssetRef / EntityRef / JsonFallback。
保留完整 C-full 控件枚举。
```

测试：

```text
FieldSchema maps to expected PropertyEditorKind.
Unsupported editor kind reports diagnostic.
```

命令：

```powershell
cargo test -p editor_core property_widget
cargo test -p editor_ui_model inspector
```

### A5：TransactionRouter

目标：

```text
新增 PropertyTransactionRouter。
支持 SceneEntity -> SceneEditCommand。
支持 PrefabInstance -> PrefabOverride。
支持 RuntimeObjectReadonly -> reject readonly。
```

测试：

```text
TransactionRouter routes SceneEntity edit to SceneEditCommand.
TransactionRouter routes PrefabInstance edit to PrefabOverride.
Readonly runtime target rejects write.
```

命令：

```powershell
cargo test -p editor_core transaction_router
```

### A6：Prefab Override 集成

目标：

```text
PropertyHandle.override_state 支持 PrefabInstance。
Property edit 能进入 PrefabWorkflowService 写 override。
InspectorReport 能统计 override_count。
```

测试：

```text
PrefabInstance property edit creates PrefabOverride.
ResolvedPrefabView applies edit.
Override state is Overridden.
```

命令：

```powershell
cargo test -p editor_core prefab
cargo test -p editor_core property_handle
```

### A7：InspectorReport

目标：

```text
新增 InspectorReport / InspectorDiagnostic。
统计 property_count / editable_count / readonly_count / invalid_schema_count / failed_edit_count / override_count。
```

测试：

```text
InspectorReport counts properties.
InspectorReport records invalid schema.
InspectorReport records failed edit.
```

命令：

```powershell
cargo test -p editor_core inspector_report
```

### A8：EditorSession / InspectorModel 最小接入

目标：

```text
现有 InspectorModel 构建尽量复用 PropertyTreeBuilder。
不能破坏已有场景编辑、字段编辑、Prefab 工作流。
```

测试：

```powershell
cargo test -p editor_core inspector
cargo test -p editor_core property_editing
```

### A9：整体回归

命令：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

## 5. 验收标准

必须满足：

```text
1. ComponentSchema / ObjectSchema 可注册和解析。
2. InspectableTarget 能表达 SceneEntity / PrefabInstance / RuntimeObjectReadonly。
3. PropertyHandle 能读值、校验、输出 override state。
4. PropertyTreeBuilder 能按 Schema 构建 PropertyTree。
5. 缺 Schema 走 JsonFallback 并 report warning。
6. Array / Object 能最小展开。
7. TransactionRouter 能路由 SceneEntity / PrefabInstance / readonly target。
8. PrefabInstance edit 能产生 PrefabOverride。
9. InspectorReport 可序列化并统计关键字段。
10. 现有 editor_core / engine_runtime / runtime_player_winit 回归通过。
11. 不出现项目玩法专用 API。
```

## 6. 文档同步动作

施工完成后必须：

```text
1. 将本文移动到 施工文档/已完成/。
2. 新建 阶段完成记录/2026-xx-xx-M8-Schema-driven-Inspector-Details-System-C-full/00-总览.md。
3. 更新 54-当前可自动化施工总入口.md。
4. 更新 施工文档/README.md。
5. 更新 00-文档地图.md / README.md 当前入口状态。
6. 如果施工中调整规则，回填 141 正式方案文档。
```

## 7. 施工自审

### 7.1 Specification fit

施工文档对应用户选择的方案 C，不是方案 B。它保留完整架构层：InspectableTarget / Schema / PropertyHandle / Widget / Plugin / TransactionRouter / Report。

### 7.2 Rule fit

施工继承 `123` 和 M7，不重复发明 Inspector 或 Prefab，不加入玩法 API。

### 7.3 Textual consistency

施工阶段与正式方案一致：

```text
Schema 是字段来源。
PropertyTree 是视图。
PropertyHandle 是读写入口。
TransactionRouter 是写入出口。
```

### 7.4 Design fit

施工优先 AI 可读、复杂项目能力、长期维护，同时用 staged implementation 控制复杂度。

### 7.5 Implementation feasibility

现有代码已有 PropertyTree / PropertyValue / PropertyEditCommand / PrefabWorkflowService，可以在其上增量实现。

### 7.6 Practical reasonableness

每个阶段都有模块测试；第一版不强行完成所有高级 UI 控件，避免施工失控。

结论：

```text
施工文档通过自审，可以直接开始施工。
```
