# 125-当前可自动化施工文档：Native Editor Project Open/Create Persistence C-min

> 状态：可施工。来源方案：`125-Native-Editor-Project-OpenCreate-Persistence-C-min方案.md`。

## 1. 施工目标

把 124 的 Project Launcher 从“能画、能通过命令创建/打开”推进到“真实 Project Manager C-min”：

```text
启动加载 recent projects
点击 Open/Create 走 dialog abstraction
创建/打开成功后保存 recent projects
项目状态可验证
headless 测试可控
```

## 2. 施工范围

### 2.1 editor_core

新增/扩展：

```text
ProjectRecentStore
ProjectRecentProjectsDocument
ProjectTemplateRegistry
ProjectTemplateDescriptor
ProjectValidationStatus
```

`ProjectLauncherState` 增加：

```text
load_recent_projects(path)
save_recent_projects(path)
apply_recent_projects(...)
validate_recent_project(...)
create_project_from_template(...)
```

### 2.2 editor_window_winit

新增：

```text
ProjectLocationDialogService
HeadlessFolderDialogBackend
NativeFolderDialogBackend shell
ProjectManagerController
```

`NativeEditorApplication`：

```text
启动时可注入 recent store path
点击 open_project / create_project 空 path 时，调用 dialog service
dialog 返回 path 后生成正式 UiCommandPayload
```

第一版 native backend 如果没有真正 OS dialog crate，则明确返回：

```text
project.dialog.native_backend_not_available
```

headless backend 必须能测试。

### 2.3 editor_host

修正启动 smoke：

```text
默认启动应显示 ProjectLauncher
不再假设初始 draw list 包含 Viewport
```

## 3. 测试

逐模块测试：

```text
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p editor_host
```

最终：

```text
cargo fmt --check
cargo test -p editor_ui_model -p editor_ui_renderer -p editor_input -p editor_core -p editor_window_winit -p editor_host
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

## 4. 验收标准

1. recent projects 能保存到 JSON。
2. recent projects 能从 JSON 加载。
3. missing / invalid project 能被标记状态。
4. create project 成功后写入 recent projects。
5. open project 成功后写入 recent projects。
6. NativeEditorApplication 默认仍进入 ProjectLauncher。
7. headless dialog backend 能驱动 open/create 点击流程。
8. editor_host 初始 smoke 按 ProjectLauncher 规则验证。

## 5. 不做范围

```text
真实 OS folder picker crate 接入
模板市场
云项目
项目升级
自动扫描全盘项目
```

