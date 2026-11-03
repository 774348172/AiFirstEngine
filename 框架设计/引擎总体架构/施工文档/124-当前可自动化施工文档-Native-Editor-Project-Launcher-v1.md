# 124-当前可自动化施工文档：Native Editor Project Launcher v1

> 状态：可施工。来源方案：`124-Native-Editor-Project-Launcher-v1方案.md`。

## 1. 施工目标

实现 Native Editor 启动后的 Unity Hub-like 项目入口页：

```text
打开 editor_host
  -> 显示 Project Launcher
  -> 左侧 Open Project / Create Project
  -> 右侧 Recent Projects 列表
  -> Open / Create 成功后进入 Authoring Workspace
```

## 2. 模块拆分

### 2.1 editor_ui_model

新增：

```text
EditorUiMode
ProjectLauncherModel
RecentProjectEntry
ProjectLauncherCommand
```

`EditorUiModel` 增加：

```text
mode
project_launcher
```

### 2.2 editor_ui_renderer

新增 Project Launcher 绘制路径：

```text
SelfUiRenderer::build_draw_list
  if model.mode == ProjectLauncher
    draw_project_launcher
  else
    draw_authoring_workspace
```

新增 hit target：

```text
ProjectLauncherAction { action_id }
ProjectLauncherRecentProject { project_path }
```

### 2.3 editor_input

新增命令路由：

```text
open_project
create_project
select_recent_project
refresh_recent_projects
```

第一版真实文件夹选择可以先由 host 或后续 native dialog 接入。
本轮 headless 命令使用 payload path。

### 2.4 editor_core

新增 Project Launcher 数据与操作：

```text
ProjectManifest
ProjectLauncherState
ProjectLauncherEvent
create_project_at(path, name)
open_project(path)
recent_projects
```

项目创建最小目录：

```text
project.aife.json
Assets/
Scenes/Main.scene.json
Packages/
Settings/project_settings.json
Library/
```

### 2.5 editor_window_winit

NativeEditorApplication 增加 mode 状态。

启动默认：

```text
ProjectLauncher
```

打开/创建项目成功后切换：

```text
AuthoringWorkspace
```

## 3. 测试

每个模块完成后测试：

```text
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_input
cargo test -p editor_core
cargo test -p editor_window_winit
```

最终：

```text
cargo fmt --check
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

## 4. 验收标准

1. 新 NativeEditorApplication 默认显示 ProjectLauncher。
2. DrawList 中存在 Open Project / Create Project hit region。
3. 无 recent project 时显示空项目列表提示。
4. create_project 命令能生成最小项目目录和 manifest。
5. open_project 命令能校验 manifest 并加入 recent list。
6. 打开/创建成功后 application mode 切到 AuthoringWorkspace。
7. 不打开项目时不显示项目内 Hierarchy / Inspector / Viewport 工作区。

