# 123-当前可自动化施工文档-Workspace Property Editing / Inspector Framework v1

## 1. 施工目标

根据：

```text
123-Workspace-Property-Editing-Inspector-Framework-v1方案.md
```

实现第一版可验证的属性编辑框架骨架，并跑通基础字段提交链路：

```text
InspectorField / PropertyNode
  -> PropertyEditBuffer
  -> PropertyEditCommand
  -> UiCommandPayload::SetSceneTransform / SetSceneComponentField
  -> EditorAuthoringWorkspace
  -> EditorSession
  -> SceneEditTransaction
```

本施工文档不要求一次完成完整 UI 体验，但必须为下列能力预留统一结构：

```text
完整 IME
多行富文本
复杂数组编辑器
复杂对象嵌套编辑器
颜色选择器
动画曲线
自定义 Inspector 插件
```

## 2. 涉及文件

预计新增：

```text
rust/crates/editor_core/src/property_editing.rs
阶段完成记录/2026-06-30-Workspace-Property-Editing-Inspector-Framework-v1/00-总览.md
```

预计修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/authoring_workspace.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
```

## 3. 模块 A：PropertyTree / PropertyPath / PropertyValue

目标：

```text
新增 Property Editing 核心类型。
能从 InspectorModel 构建 PropertyTree。
能表达基础值和高级值类型。
```

必须包含：

```text
PropertyPath
PropertyTree
PropertyNode
PropertyValue
PropertyValueType
PropertyEditorKind
PropertyMetadata
InspectorPluginDescriptor
```

测试：

```powershell
cargo test -p editor_core property_editing
```

验收：

```text
PropertyPath 能解析 transform.localPosition。
PropertyTree 能从 InspectorModel 生成节点。
PropertyValue 能覆盖 String / Bool / Number / Vec3 / Color / Array / Object / Curve / RichText / Json。
```

## 4. 模块 B：PropertyEditBuffer / Commit / Cancel

目标：

```text
新增字段编辑缓冲区。
支持 begin_edit / input_text / commit / cancel。
提交时输出 PropertyEditCommand。
```

必须包含：

```text
PropertyEditBuffer
TextCompositionState
RichTextBuffer
PropertyEditCommand
PropertyEditCommitReport
```

测试：

```powershell
cargo test -p editor_core property_editing
```

验收：

```text
文本输入能写入 buffer。
commit 能生成 SetValue 命令。
cancel 不产生提交命令。
IME composition 能记录 preedit / committed_text。
```

## 5. 模块 C：PropertyEditCommand -> UiCommandPayload 映射

目标：

```text
基础字段提交能转成现有 UiCommandPayload。
不新增第二套正式编辑命令系统。
```

映射规则：

```text
transform.localPosition -> SetSceneTransform.local_position
transform.localRotation -> SetSceneTransform.local_rotation
transform.localScale -> SetSceneTransform.local_scale
components.{ComponentType}.{fieldPath} -> SetSceneComponentField
```

测试：

```powershell
cargo test -p editor_core property_editing
```

验收：

```text
Transform Vec3 可以映射为 SetSceneTransform。
Component 字段可以映射为 SetSceneComponentField。
非法路径返回 diagnostic，不 panic。
```

## 6. 模块 D：EditorAuthoringWorkspace 接入

目标：

```text
Workspace 持有 PropertyTree 摘要和 PropertyEditBuffer 状态。
Workspace 能把 PropertyEditCommand 归口成 UiCommand。
WorkspaceReport 暴露 property_editing 最小状态。
```

测试：

```powershell
cargo test -p editor_core
```

验收：

```text
Workspace refresh_from_model 后有 property_tree_summary。
Workspace 能提交基础字段编辑命令。
WorkspaceReport 能看到 focused_property_path / editing 状态。
```

## 7. 模块 E：Input / Window 最小接入

目标：

```text
点击 InspectorField 后能聚焦字段。
文本输入可以被 FocusInputSystem 消费并写入 PropertyEditBuffer。
第一版可通过 headless 测试验证，不依赖真实窗口手工点。
```

测试：

```powershell
cargo test -p editor_input
cargo test -p editor_window_winit
```

验收：

```text
InspectorField hit region 能产生聚焦语义。
TextField focus 不把文本泄漏到 Runtime GameView。
基础字段提交仍通过 EditorSession / Transaction。
```

## 8. 分阶段回归规则

每完成一个模块，必须先执行该模块测试，再进入下一模块。

最后统一执行：

```powershell
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_ui_model
cargo test -p editor_input
cargo test -p editor_window_winit
cargo test -p editor_host
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

## 9. 完成后文档同步

施工完成后：

```text
1. 将本文档移动到 施工文档/已完成/。
2. 新增 阶段完成记录/2026-06-30-Workspace-Property-Editing-Inspector-Framework-v1/00-总览.md。
3. 更新 49-当前状态与下一步入口.md。
4. 更新 54-当前可自动化施工总入口.md。
```

## 10. 不允许事项

```text
不允许 Inspector 控件直接写 EditorSceneDocument。
不允许自定义 Inspector 插件绕过 Transaction。
不允许为打飞机项目增加专用字段规则。
不允许新增第二套正式命令系统。
不允许把 IME / RichText / Curve 做成独立架构主线。
```
