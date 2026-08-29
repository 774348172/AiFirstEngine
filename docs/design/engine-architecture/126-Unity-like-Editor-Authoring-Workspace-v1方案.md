# 126-Unity-like Editor Authoring Workspace v1 方案

> 状态：已确认采用 C-min。本文档延续 120 / 121 / 122 / 123 / 124 / 125，不重新设计 Hierarchy、Inspector、SceneEditing、PropertyEditing，而是把已有能力收敛成项目打开后的 Unity-like 编辑工作台。

## 1. 设计问题

124 / 125 已经完成：

```text
ProjectLauncher
Open / Create Project
Recent Projects
ProjectSession
```

下一步需要解决：

```text
打开项目后，用户能不能像 Unity 一样进入一个可编辑工作区？
```

这不是单独补一个面板，而是把这些编辑器能力串成闭环：

```text
ProjectBrowser
Hierarchy
SceneView / GameView
Inspector
Console
AI Panel
Toolbar
Save / Dirty / Undo / Redo / Play
```

## 2. 参考引擎

| 引擎 | 对应结构 | 对我们的启发 |
|---|---|---|
| Unity | `ProjectBrowser.cs`、`SceneView.cs`、`InspectorWindow.cs`、`PropertyEditor.cs`、`SceneHierarchy.cs` | 用户心智是 Project -> Scene/Hierarchy -> Inspector -> Save/Play |
| Unreal Engine | `LevelEditor`、`SceneOutliner`、`PropertyEditor`、`ContentBrowser` | 模块多，但由 LevelEditor/Tab 管理统一组织 |
| Godot | `editor_node`、`file_system`、`scene`、`inspector`、`docks` | 工作流直接：FileSystem + SceneTree + Inspector + Viewport |

结论：

```text
我们学习 Unity/Godot 的简单心智，学习 UE 的模块边界；
内部继续保持 AI 可读的 Schema / Command / Transaction / Trace。
```

## 3. 方案选择

### 方案 A：继续单点补面板

不采用。它会继续导致 ProjectBrowser、Hierarchy、Inspector、SceneView 分散施工，整体体验长期不闭环。

### 方案 B：一次实现完整 Unity/UE 工作区

不采用第一版。完整 Dock、Gizmo、Prefab override、复杂 Inspector、拖拽资源、搜索过滤会过大。

### 方案 C-min：Unity-like Authoring Workspace v1

采用。

保留完整系统边界，但第一版只做最小真实闭环：

```text
打开项目
显示项目文件
打开 Scene
创建/选择/删除/重命名 Entity
Inspector 编辑 Transform / Component 字段
Save / Undo / Redo
Console 显示命令结果
AI Panel 能读取当前 workspace context
```

## 4. 系统结构

```text
ProjectSession
  -> AuthoringWorkspace
      -> WorkspaceDocumentState
      -> WorkspacePanelState
      -> WorkspaceSelection
      -> WorkspaceCommandRouter
      -> WorkspaceDirtyState
      -> WorkspaceDiagnostics
      -> WorkspaceContextForAI

Panels:
  Toolbar
  ProjectBrowser
  Hierarchy
  SceneView
  GameView
  Inspector
  Console
  AI Panel
```

## 5. 关键规则

Panel 不直接改数据。

统一流程：

```text
Panel Interaction
  -> UiCommand
  -> WorkspaceCommandRouter
  -> EditorSession / SceneEdit / PropertyEdit
  -> Transaction
  -> Dirty / Undo / Diagnostics
  -> Rebuild UiModel
```

约束：

```text
Hierarchy 不直接改 Scene
Inspector 不直接写 Component
ProjectBrowser 不直接创建 Runtime 对象
AI Panel 不直接改文件
```

## 6. C-min 范围

### 6.1 ProjectBrowser

第一版显示：

```text
Assets/
Scenes/
Settings/
```

能力：

```text
显示项目文件条目
选择文件
打开 .scene.json
显示 missing / invalid / selected 状态
```

### 6.2 Hierarchy

沿用已有 SceneEditing：

```text
显示 Entity 树
选择 Entity
创建 Entity
删除 Entity
重命名 Entity
```

### 6.3 Inspector

沿用 PropertyEditing：

```text
显示 Entity 基础信息
显示 Transform
显示 Component 字段
支持文本 / 数字 / bool / Vec3 最小编辑
```

### 6.4 SceneView / GameView

第一版只做视图语义分离：

```text
SceneView = 编辑视图
GameView = Runtime 预览
```

不做复杂 Gizmo。

### 6.5 Toolbar

第一版必须有：

```text
Save Scene
Undo
Redo
Play
Stop/Pause
Build disabled reason
```

### 6.6 Console / AI Panel

Console：

```text
显示命令结果和错误
```

AI Panel context：

```text
当前项目
当前场景
当前选中 Entity
当前选中 Asset
当前 diagnostics
允许命令列表
```

## 7. 不做范围

```text
复杂 Dock 拖拽
完整 Prefab Override
完整 Scene Gizmo
完整 Asset Import UI
完整 Inspector 插件系统
多对象编辑
复杂搜索过滤
复杂右键菜单
资源拖拽进 Scene
```

## 8. 验收用例

```text
打开项目
-> ProjectBrowser 显示 Scenes/Main.scene.json
-> 打开 Main.scene.json
-> Hierarchy 显示 Scene
-> 创建 Entity
-> 选中 Entity
-> Inspector 修改 localPosition
-> Save Scene
-> Undo / Redo
-> Console 显示命令记录
-> AI Panel 能读取当前 selection/context
```

## 9. 施工入口

对应施工文档：

```text
施工文档/126-当前可自动化施工文档-Unity-like-Editor-Authoring-Workspace-v1.md
```

