# 124-Native Editor Project Launcher v1 方案

> 状态：已确认。本文档定义 Native Editor 启动后的 Unity Hub-like 项目入口页。它是 `Unity-like Editor Authoring Loop v1` 的前置入口，不是项目内普通 Panel。

## 1. 设计问题

Native Editor 不应该启动后直接进入一个默认项目。

第一版启动流程应为：

```text
启动 editor_host
  -> ProjectLauncherMode
  -> Open Project / Create Project / Recent Projects
  -> 选择或创建项目成功
  -> AuthoringWorkspaceMode
```

没有项目打开时，不加载 Scene、RuntimePackage、Asset DB 全扫描，也不允许 Play / Build。

## 2. 参考引擎

| 引擎 | 对应结构 | 参考 |
|---|---|---|
| Unity | Unity Hub 负责项目列表、打开、创建；Unity Editor 进入项目后才显示 Hierarchy / Scene / Inspector | UnityCsReference 中可见 `ProjectTemplateWindow`、`ProjectBrowser`，Hub 本体不完整开源 |
| Unreal Engine | 启动可进入 Project Browser / New Project，再进入 UnrealEditor | `Engine/Source/Editor/GameProjectGeneration/Private/SProjectBrowser.cpp` |
| Godot | Project Manager 是明确启动层，负责项目列表、创建、导入，进入项目后才启动 EditorNode | `editor/project_manager/project_manager.cpp`、`project_list.cpp`、`project_dialog.cpp` |

结论：Project Launcher / Project Manager 是成熟引擎的正式入口层，不是临时 UI。

## 3. 推荐规则

采用：

```text
方案 B：内置 Project Launcher Mode
```

结构：

```text
NativeEditorApplication
  mode: ProjectLauncher | AuthoringWorkspace

ProjectLauncher
  recent_projects
  selected_project
  search_query
  diagnostics
  commands:
    - OpenProject
    - CreateProject
    - SelectRecentProject
    - RefreshRecentProjects

AuthoringWorkspace
  Hierarchy
  SceneView / GameView
  Inspector
  ProjectDock
  Console
  RuntimeTrace
  AI Panel
```

## 4. UI 规则

启动页学习 Unity Hub 的基本体验：

```text
左侧导航：
  Open Project
  Create Project

右侧主区域：
  Projects
  Search
  Recent Project List
    - name
    - path
    - modified time
    - engine version
    - status
```

无项目时显示空列表，不自动创建项目。

## 5. 项目识别文件

创建项目时生成：

```text
MyProject/
  project.aife.json
  Assets/
  Scenes/
    Main.scene.json
  Packages/
  Settings/
    project_settings.json
  Library/
```

`project.aife.json` 是项目入口文件，类似 Godot 的 `project.godot`、UE 的 `.uproject`。

第一版字段：

```text
schemaVersion
projectId
projectName
engineVersion
createdAt
lastOpenedAt
defaultScene
assetRoot
settingsVersion
```

## 6. Recent Projects

Recent Projects 是用户级编辑器状态，不写入项目目录。

第一版通过 `ProjectLauncherState` 读写，未来可迁移到用户配置目录：

```text
editor_recent_projects.json
```

字段：

```text
name
path
engine_version
last_opened_at
last_modified_at
valid
```

## 7. AI 友好规则

没有打开项目时：

```text
AI 只能解释、创建项目、打开项目、诊断项目入口。
AI 不能修改 Scene / Asset / RuntimePackage。
```

打开项目成功后：

```text
AI 才能读取 ProjectSession / Scene / Asset DB，并生成项目修改计划。
```

Project Launcher 操作必须产生可读事件：

```text
ProjectLauncherEvent
  kind
  project_path
  result
  diagnostics
```

## 8. 不做范围

第一版不做：

```text
账号登录
云项目
团队协作
多引擎版本安装管理
模板市场
复杂模板系统
项目升级向导
```

## 9. 全局讨论规则补充

后续讨论系统时，默认增加一条低优先级规则：

```text
在不违反 AI 友好、复杂项目、可维护、简单、效率的前提下，
系统边界尽量定得更完整，内部细节一次讨论得更充分，
减少用户在零碎细节上反复介入。
```

它的优先级低于既有核心优先级：

```text
1. AI 友好
2. 支持复杂项目
3. 后期可维护、可修改、AI 可改
4. 简单，隐藏规则少
5. 效率和多平台可行
6. 系统边界更宽、细节更完整、减少用户逐点介入
```

## 10. 施工入口

对应施工文档：

```text
施工文档/124-当前可自动化施工文档-Native-Editor-Project-Launcher-v1.md
```

