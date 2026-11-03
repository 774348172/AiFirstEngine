# 127-当前可自动化施工文档-Editor Interaction Feedback / Command Availability v1

## 施工目标

实现编辑器交互反馈与命令可用性第一版，让按钮、菜单入口、命令执行结果不再静默失败。

## 涉及文件

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_core/src/authoring_workspace.rs
框架设计/引擎总体架构/127-Editor-Interaction-Feedback-Command-Availability-v1方案.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
```

## 阶段 1：数据结构

新增或扩展：

```text
HitRegion.command_id
HitRegion.reason_disabled
HitRegion visual state 输入
InputRouteResult.disabled_feedback
NativeEditorApplicationReport.hovered_hit_id
NativeEditorApplicationReport.pressed_hit_id
NativeEditorApplicationReport.last_feedback
```

测试：

```powershell
cargo test -p editor_ui_renderer -p editor_input
```

## 阶段 2：输入链路

输入路由规则：

```text
enabled hit region -> 正常生成 UiCommand
disabled hit region -> 不生成 UiCommand，生成 disabled feedback
no hit region -> 保持诊断
```

测试：

```powershell
cargo test -p editor_input
```

## 阶段 3：应用壳反馈

`NativeEditorApplication` 维护：

```text
hovered_hit_id
pressed_hit_id
last_feedback
```

点击 disabled command 时：

```text
不进入 EditorSession
更新 last_feedback
redraw_requested = true
```

测试：

```powershell
cargo test -p editor_window_winit
```

## 阶段 4：渲染反馈

`SelfUiRenderer` 根据 interaction state 绘制：

```text
normal
hovered
pressed
disabled
```

第一版重点覆盖 Toolbar。

测试：

```powershell
cargo test -p editor_ui_renderer
```

## 阶段 5：回归验证

完整命令：

```powershell
cargo fmt --check
cargo test -p editor_ui_model -p editor_ui_renderer -p editor_input -p editor_window_winit -p editor_host
cargo test -p editor_window_winit --features real-window,real-wgpu-surface
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

## 完成后文档同步

施工完成后更新：

```text
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
阶段完成记录/2026-06-30-Editor-Interaction-Feedback-Command-Availability-v1/00-总览.md
```

