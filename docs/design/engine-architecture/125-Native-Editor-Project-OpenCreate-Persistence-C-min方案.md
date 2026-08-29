# 125-Native Editor Project Open/Create Persistence C-min 方案

> 状态：已确认采用方案 C-min。本文档延续 `124-Native-Editor-Project-Launcher-v1方案.md`，不重新讨论 Project Launcher 是否存在，只讨论如何把它推进到真实可用的 Project Manager 最小闭环。

## 1. 设计问题

124 已完成：

```text
ProjectLauncherMode
OpenProject / CreateProject / SelectRecentProject / RefreshRecentProjects
project.aife.json
ProjectSession
启动页 UI / hit region / command route
```

但当前仍缺真实用户入口：

```text
点击 Open Project 不能弹真实文件夹选择
点击 Create Project 不能弹真实文件夹选择
recent projects 没有持久化
下次启动无法恢复项目列表
项目状态错误不能完整反馈给用户和 AI
```

125 解决的是完整 Project Manager 方向的第一版最小闭环：

```text
真实打开/创建项目
recent projects 持久化
启动加载
最小模板
项目状态诊断
headless 可测试
```

## 2. 参考引擎

| 引擎 | 做法 | 对我们的启发 |
|---|---|---|
| Unity | Unity Hub 管项目列表；Unity Editor 内有 `EditorUtility.SaveFolderPanel` 一类文件夹选择 API | 文件夹选择和项目入口是正式用户流程，不应靠测试命令注入路径 |
| Unreal Engine | `SProjectBrowser.cpp` 使用 `DesktopPlatformModule / IDesktopPlatform`，ProjectBrowser 负责项目浏览、版本、项目项展示 | 需要平台对话框抽象，不让 UI 直接依赖 OS API |
| Godot | `ProjectManager / ProjectDialog / ProjectList` 负责项目列表、创建、导入、空列表、状态、编辑器设置持久化 | Project Manager 是启动层；recent projects 是用户配置，不写入项目目录 |

结论：

```text
Project Launcher 后续应演进为内置 Project Manager。
第一版采用 C-min：保留完整 Project Manager 边界，但只实现最小真实可用功能。
```

## 3. 方案选择

### 方案 A：临时按钮路径

点击按钮写死路径或继续靠测试注入。

不采用。它会让 124 的启动页继续停在“演示 UI”。

### 方案 B：只做 folder dialog + recent 持久化

能解决当前点击问题，但系统边界偏窄，后续模板、导入、扫描、版本状态还要重新开系统。

不采用作为最终命名。

### 方案 C-min：Project Manager C-min

保留完整 Project Manager 边界：

```text
ProjectManagerController
ProjectTemplateRegistry
ProjectLocationDialog
ProjectRecentStore
ProjectValidation
ProjectLauncherDiagnostics
```

第一版只实现：

```text
默认空项目模板
打开已有项目
创建新项目
recent projects JSON 持久化
启动加载 recent projects
缺失/无效项目状态
headless dialog backend
真实 native dialog backend 入口
```

采用。

## 4. 系统结构

```text
NativeEditorApplication
  ProjectLauncherMode
    ProjectManagerController
      ProjectLocationDialogService
        NativeFolderDialogBackend
        HeadlessFolderDialogBackend
      ProjectRecentStore
      ProjectTemplateRegistry
      ProjectValidation
      ProjectLauncherState
```

## 5. 数据规则

### 5.1 用户配置

recent projects 是用户级配置，不属于项目内容。

第一版文件：

```text
%APPDATA%/AIFirstEngine/editor_recent_projects.json
```

非 Windows 平台后续按平台配置目录扩展。headless 测试可指定临时配置文件。

结构：

```json
{
  "schemaVersion": "editor-recent-projects.v1",
  "recentProjects": [
    {
      "name": "PlaneGame",
      "path": "<PROJECTS_ROOT>/PlaneGame",
      "engineVersion": "0.1.0",
      "lastOpenedAt": "timestamp",
      "lastModifiedAt": "timestamp",
      "valid": true,
      "status": "ready"
    }
  ]
}
```

### 5.2 项目模板

C-min 只实现一个内置模板：

```text
EmptyProject
```

创建结果仍是 124 的标准项目结构：

```text
project.aife.json
Assets/
Scenes/Main.scene.json
Packages/
Settings/project_settings.json
Library/
```

模板系统只保留结构，不做模板市场、分类、缩略图。

### 5.3 项目状态

RecentProjectEntry.status 第一版取值：

```text
ready
missing
invalid_manifest
unsupported_version
```

## 6. 流程规则

### 6.1 启动

```text
editor_host start
  -> ProjectRecentStore.load()
  -> ProjectValidation.refresh_recent_projects()
  -> ProjectLauncherMode
```

启动时不自动打开最近项目。

### 6.2 Open Project

```text
click Open Project
  -> ProjectLocationDialogService.pick_open_project_folder()
  -> validate project.aife.json
  -> ProjectSession
  -> ProjectRecentStore.save()
  -> AuthoringWorkspaceMode
```

### 6.3 Create Project

```text
click Create Project
  -> ProjectLocationDialogService.pick_create_project_folder()
  -> ProjectTemplateRegistry.select_default_template()
  -> create project skeleton
  -> ProjectSession
  -> ProjectRecentStore.save()
  -> AuthoringWorkspaceMode
```

第一版项目名：

```text
优先使用用户输入
没有输入时使用文件夹名
文件夹名为空时使用 NewProject
```

## 7. 错误与 AI 反馈

所有错误进入结构化 diagnostics，不只写日志：

```text
project.dialog.cancelled
project.open_failed.manifest_missing
project.open_failed.invalid_manifest
project.open_failed.unsupported_version
project.create_failed.path_not_writable
project.create_failed.project_exists
project.recent_store.load_failed
project.recent_store.save_failed
```

AI 可根据这些错误解释：

```text
当前为什么没有进入工作区
项目为什么打不开
recent list 为什么为空
应该选择哪个文件夹
```

## 8. 第一版不做

```text
云项目
账号
模板市场
多引擎版本安装管理
项目升级向导
自动磁盘扫描所有项目
收藏/标签
删除项目
复杂新建项目向导
```

这些属于 Project Manager 后续版本，不进 C-min。

## 9. 为什么适合我们

按项目优先级：

1. AI 友好：ProjectManager 状态、recent、diagnostics 都是结构化数据。
2. 支持复杂项目：ProjectSession 和 recent 持久化是复杂项目长期入口。
3. 可维护：Dialog / Store / Template / Validation 分层，后续扩展不挤进 UI。
4. 简单：C-min 只实现 EmptyProject、Open、Create、Recent，不上云和模板市场。
5. 性能：只在启动和用户点击时读写配置，不影响运行时。
6. 系统边界完整：一次把 Project Manager 最小闭环定完整，减少后续碎片讨论。

## 10. 施工入口

对应施工文档：

```text
施工文档/125-当前可自动化施工文档-Native-Editor-Project-OpenCreate-Persistence-C-min.md
```

