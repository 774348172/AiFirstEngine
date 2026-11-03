# 126-当前可自动化施工文档：Unity-like Editor Authoring Workspace v1

> 状态：可施工。来源方案：`126-Unity-like-Editor-Authoring-Workspace-v1方案.md`。

## 1. 施工目标

实现项目打开后的 Unity-like Authoring Workspace C-min：

```text
ProjectBrowser
Hierarchy
Inspector
SceneView / GameView 语义
Toolbar Save/Undo/Redo/Play
Console
AI context
```

## 2. 施工范围

### 2.1 editor_ui_model

新增：

```text
ProjectBrowserModel
ProjectBrowserEntry
ProjectBrowserEntryKind
WorkspaceViewMode
```

`EditorUiModel` 增加：

```text
project_browser
workspace_view_mode
```

新增命令：

```text
SelectProjectBrowserEntry
OpenProjectBrowserEntry
SetWorkspaceViewMode
```

### 2.2 editor_core

`EditorSession`：

```text
active_project_session -> build_project_browser_model
SelectProjectBrowserEntry
OpenProjectBrowserEntry
SetWorkspaceViewMode
```

规则：

```text
OpenProjectBrowserEntry(.scene.json) -> OpenSceneDocument
```

### 2.3 editor_ui_renderer

绘制 ProjectBrowser：

```text
workspace 模式下 bottom/right 区域显示 ProjectBrowser 条目
条目有 hit region
```

### 2.4 editor_input

新增路由：

```text
ProjectBrowserEntry hit -> SelectProjectBrowserEntry
ProjectBrowserAction open -> OpenProjectBrowserEntry
```

### 2.5 editor_window_winit / editor_host

确保：

```text
打开项目后 ProjectBrowser 可见
ProjectBrowser 打开 Scene 后 Hierarchy / Inspector 刷新
```

## 3. 测试

逐模块：

```text
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_input
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

1. 打开项目后 UI model 有 ProjectBrowser 条目。
2. ProjectBrowser 显示 Scenes/Main.scene.json。
3. 点击 ProjectBrowser 条目能选择资源。
4. OpenProjectBrowserEntry 能打开 `.scene.json`。
5. 打开 Scene 后 Hierarchy 显示实体。
6. Inspector 编辑仍走 PropertyEditing / Command / Transaction。
7. Save / Undo / Redo 不被 ProjectBrowser 破坏。
8. AI context 能包含 selected asset / selected entity。

