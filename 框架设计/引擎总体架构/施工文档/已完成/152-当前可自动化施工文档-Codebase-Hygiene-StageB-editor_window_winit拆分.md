# 152-当前可自动化施工文档-Codebase Hygiene Stage B editor_window_winit 拆分

## 1. 施工目标

根据正式方案：

```text
../../151-Codebase-Architecture-Hygiene-Gate-v1方案.md
```

本阶段执行：

```text
Codebase Architecture Hygiene Gate v1 - Stage B editor_window_winit split
```

目标是把：

```text
rust/crates/editor_window_winit/src/lib.rs
```

从 4000+ 行入口大文件，拆成长期主义目标结构下的领域模块。

本阶段必须坚持：

```text
完整方案 C 的最终架构方向。
小步迁移。
每拆一块就测试。
public API 尽量不变。
不改变用户可见行为。
不新增编辑器功能。
不减少测试。
```

## 2. 当前代码分段依据

当前 `editor_window_winit/src/lib.rs` 的主要自然分段：

```text
NativeEditorWindowConfig
PanelRegistry / DockLayoutManager / PanelRect
EditorMainFrame / FocusTarget / EditorFocusInputSystem
ProjectFolderDialog / HeadlessFolderDialogBackend / NativeFolderDialogBackend
ProjectManagerController
EditorCommandSystem / EditorTransactionService
EditorUiModelComposer
NativeEditorApplication / NativeEditorApplicationReport
WgpuSurfacePlan / NativeWindowAttributePlan / winit window attrs
WindowState / HeadlessWindowBackend
SurfaceState / HeadlessSurfaceBackend
ViewportKind / ViewportHost / ViewportInputGateway / RuntimeViewportFrameSummary
Runtime input event conversion
HeadlessRuntimeRenderer / RealWindowGateReport
HeadlessNativeEditorWindowApp / RealNativeEditorWindowReport
real_native_editor_window module
tests
```

Stage B 的拆分必须基于这些真实分段，不做凭空抽象。

## 3. 目标目录

本阶段目标结构：

```text
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_window_winit/src/config.rs
rust/crates/editor_window_winit/src/dock.rs
rust/crates/editor_window_winit/src/focus.rs
rust/crates/editor_window_winit/src/dialog.rs
rust/crates/editor_window_winit/src/project_manager.rs
rust/crates/editor_window_winit/src/command_system.rs
rust/crates/editor_window_winit/src/transaction.rs
rust/crates/editor_window_winit/src/composer.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/window_plan.rs
rust/crates/editor_window_winit/src/headless_window.rs
rust/crates/editor_window_winit/src/surface.rs
rust/crates/editor_window_winit/src/viewport.rs
rust/crates/editor_window_winit/src/input_route.rs
rust/crates/editor_window_winit/src/runtime_render.rs
rust/crates/editor_window_winit/src/gate_report.rs
rust/crates/editor_window_winit/src/headless_app.rs
rust/crates/editor_window_winit/src/real_window.rs
```

保留已有：

```text
continuous_runtime_window.rs
windowed_runtime_present.rs
```

本阶段先用平铺模块文件，不立即建立多级目录。原因：

```text
当前 lib.rs 已经很大，第一步先降低文件体量和职责混杂。
多级目录可在 Stage B 后半或后续阶段再整理。
平铺模块更容易用小补丁迁移，测试风险更低。
```

## 4. 模块归属规则

### config.rs

放：

```text
NativeEditorWindowConfig
```

### dock.rs

放：

```text
EditorPanelDescriptor
DockArea
PanelRegistry
DockLayoutNode
DockSplitAxis
PanelRect
DockLayoutManager
```

### focus.rs

放：

```text
EditorMainFrame
FocusTarget
EditorFocusInputSystem
panel_id_for_hit_target
```

### dialog.rs

放：

```text
ProjectFolderDialogPurpose
ProjectFolderDialogRequest
ProjectFolderDialogResponse
ProjectLocationDialogService
HeadlessFolderDialogBackend
NativeFolderDialogBackend
pick_native_folder
```

### project_manager.rs

放：

```text
ProjectManagerController
default_native_editor_recent_store_path
non_empty_env_path
```

### command_system.rs

放：

```text
EditorCommandDescriptor
EditorCommandSystem
command_id_for_shell_payload
command_feedback_from_result
```

### transaction.rs

放：

```text
EditorTransactionService
```

### composer.rs

放：

```text
EditorUiModelComposer
```

### application.rs

放：

```text
NativeEditorApplicationReport
NativeEditorApplication
```

### window_plan.rs

放：

```text
WgpuSurfacePlan
wgpu_surface_plan
NativeWindowAttributePlan
native_window_attribute_plan
winit_window_attributes
wgpu_instance_descriptor
NativeEditorWindowReadiness
validate_window_skeleton
```

### headless_window.rs

放：

```text
WindowState
WindowSize
HeadlessWindowBackend
```

### surface.rs

放：

```text
SurfaceState
HeadlessSurfaceBackend
```

### viewport.rs

放：

```text
ViewportKind
ViewportOutputKind
SceneCameraState
SceneCameraMode
Vec3
SelectionOutlineState
GizmoState
GizmoMode
ViewportState
RuntimeViewportFrameSummary
ViewportHost
```

### input_route.rs

放：

```text
ViewportInputRouteKind
ViewportInputRoute
ViewportInputGateway
event_hits_rect
event_position
pointer_position_from_event
runtime_input_event_from_editor
runtime_pointer_button
CminInputRoute
route_input_for_cmin
```

### runtime_render.rs

放：

```text
RuntimeFrameDescriptor
HeadlessRuntimeRenderer
frame_without_hash
hash_stable
```

### gate_report.rs

放：

```text
DiagnosticSeverity
GateDiagnostic
GpuState
FrameState
RealWindowGateReport
UiDrawListSummary
summarize_draw_list
build_real_window_gate_report
```

### headless_app.rs

放：

```text
RealNativeEditorWindowDiagnosticSeverity
RealNativeEditorWindowDiagnostic
RealNativeEditorWindowReport
HeadlessNativeEditorWindowApp
```

### real_window.rs

放：

```text
run_real_native_editor_window
run_real_native_editor_window_with_model
real_native_editor_window module
```

## 5. lib.rs 目标

Stage B 完成后，`lib.rs` 只保留：

```rust
mod application;
mod command_system;
mod composer;
mod config;
mod dialog;
mod dock;
mod focus;
mod gate_report;
mod headless_app;
mod headless_window;
mod input_route;
mod project_manager;
mod real_window;
mod runtime_render;
mod surface;
mod transaction;
mod viewport;
mod window_plan;

pub mod continuous_runtime_window;
pub mod windowed_runtime_present;

pub use application::*;
pub use command_system::*;
...
pub use continuous_runtime_window::*;
pub use windowed_runtime_present::*;
```

允许保留少量 crate-level shared imports，但不能继续放领域实现。

## 6. 分步施工计划

### B0：施工前基线

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Hygiene-StageB-editor_window_winit拆分\before.json
cargo test -p editor_window_winit
```

目的：

```text
确认拆分前 baseline 和测试状态。
```

### B1：拆 config / dock / focus

迁移：

```text
NativeEditorWindowConfig -> config.rs
PanelRegistry / DockLayoutManager -> dock.rs
EditorMainFrame / FocusInput -> focus.rs
```

测试：

```powershell
cargo test -p editor_window_winit native_editor_application_has_complete_shell_registry
cargo test -p editor_window_winit native_editor_application_layout_computes_stable_panel_rects
cargo test -p editor_window_winit native_editor_application_text_input_is_owned_by_focus_system
cargo test -p editor_window_winit
```

### B2：拆 dialog / project_manager

迁移：

```text
ProjectFolderDialog* / dialog backend -> dialog.rs
ProjectManagerController -> project_manager.rs
```

测试：

```powershell
cargo test -p editor_window_winit native_editor_application_click_create_uses_headless_dialog_and_persists_recent
cargo test -p editor_window_winit native_editor_application_click_open_uses_headless_dialog
cargo test -p editor_window_winit native_editor_application_loads_recent_projects_from_store
cargo test -p editor_window_winit
```

### B3：拆 command_system / transaction / composer / application

迁移：

```text
EditorCommandSystem -> command_system.rs
EditorTransactionService -> transaction.rs
EditorUiModelComposer -> composer.rs
NativeEditorApplication -> application.rs
```

测试：

```powershell
cargo test -p editor_window_winit native_editor_command_system_registers_build_export_commands
cargo test -p editor_window_winit native_editor_application_ai_proposal_accept_uses_command_system
cargo test -p editor_window_winit native_editor_application_shortcut_routes_to_transaction_service
cargo test -p editor_window_winit
```

### B4：拆 window_plan / headless_window / surface

迁移：

```text
window attribute / wgpu plan -> window_plan.rs
HeadlessWindowBackend -> headless_window.rs
HeadlessSurfaceBackend -> surface.rs
```

测试：

```powershell
cargo test -p editor_window_winit window_skeleton_is_headless_testable
cargo test -p editor_window_winit window_attributes_keep_native_editor_defaults
cargo test -p editor_window_winit headless_window_backend_tracks_resize_redraw_and_close
cargo test -p editor_window_winit headless_surface_backend_tracks_acquire_present_and_loss
cargo test -p editor_window_winit
```

### B5：拆 viewport / input_route / runtime_render

迁移：

```text
ViewportHost / SceneCamera / Gizmo -> viewport.rs
ViewportInputGateway / runtime input conversion -> input_route.rs
RuntimeFrameDescriptor / HeadlessRuntimeRenderer -> runtime_render.rs
```

测试：

```powershell
cargo test -p editor_window_winit viewport_host_registers_resizes_focuses_and_rejects_second_scene_viewport
cargo test -p editor_window_winit scene_view_pointer_drag_routes_to_scene_camera_command
cargo test -p editor_window_winit game_view_space_key_drives_project_logic_and_render_extract
cargo test -p editor_window_winit runtime_input_frame_keeps_events_in_order
cargo test -p editor_window_winit
```

### B6：拆 gate_report / headless_app / real_window

迁移：

```text
RealWindowGateReport -> gate_report.rs
HeadlessNativeEditorWindowApp / RealNativeEditorWindowReport -> headless_app.rs
real window functions and cfg module -> real_window.rs
```

测试：

```powershell
cargo test -p editor_window_winit headless_native_editor_window_app_builds_frame_report
cargo test -p editor_window_winit native_editor_window_headless_frame_draws_ui_model
cargo test -p editor_window_winit real_native_editor_window_report_serializes
cargo test -p editor_window_winit
```

### B7：测试整理和基线对比

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Hygiene-StageB-editor_window_winit拆分\after.json
cargo fmt --check
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p code_hygiene
```

检查：

```text
editor_window_winit/src/lib.rs 行数显著下降。
editor_window_winit 测试数量不减少。
public API 不破坏 editor_host。
after.json 能识别新的热点变化。
```

## 7. 验收标准

必须满足：

```text
editor_window_winit/src/lib.rs 不再包含大块领域实现。
所有迁移模块能由 lib.rs re-export 保持外部 API。
cargo test -p editor_window_winit 通过。
cargo test -p editor_host 通过。
cargo test -p code_hygiene 通过。
cargo fmt --check 通过。
生成 before.json 和 after.json。
阶段完成记录写入结果。
```

建议目标：

```text
editor_window_winit/src/lib.rs 行数降到 500 行以内。
如果因为 cfg(real-window) 或 test module 暂时无法降到 500 行以内，必须在阶段完成记录说明原因。
```

## 8. 风险控制

```text
每一步只移动一个职责块。
移动后先编译测试，不继续叠加错误。
遇到循环依赖时，优先调整模块可见性，不改变 public API。
私有 helper 可以保留 pub(crate)，不为拆分扩大到 pub。
测试失败必须先修复当前小步，不进入下一小步。
```

## 9. 施工自审

```text
Specification fit:
  覆盖用户要求的完整方案 C，进入真实代码整理阶段，而不是只写规则。

Rule fit:
  继承 151 方案，不新增功能，不改变行为，不减少测试。

Textual consistency:
  目标模块、现有代码分段、迁移步骤和测试命令一一对应。

Design fit:
  优先治理 editor_window_winit 这个真实窗口入口，符合 AI 友好和复杂项目长期维护。

Implementation feasibility:
  采用平铺模块文件和小步移动，避免一次性建立深层目录造成路径和可见性混乱。

Practical reasonableness:
  每一步都有对应测试，失败可以局部回滚或修复；不会把整个编辑器拆到不可运行状态。
```

结论：

```text
本施工文档通过自审，可以开始 Stage B 施工。
```
