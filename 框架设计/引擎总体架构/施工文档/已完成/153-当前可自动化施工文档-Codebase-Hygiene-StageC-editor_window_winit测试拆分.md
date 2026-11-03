# 153-当前可自动化施工文档-Codebase Hygiene Stage C editor_window_winit 测试拆分

## 1. 施工目标

根据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
阶段完成记录/2026-07-02-Codebase-Hygiene-StageB-editor_window_winit拆分/00-总览.md
```

继续执行：

```text
Codebase Architecture Hygiene Gate v1 - Stage C editor_window_winit test split
```

目标是把 `rust/crates/editor_window_winit/src/lib.rs` 中剩余的大型 `#[cfg(test)] mod tests` 拆到测试目录中，让 `lib.rs` 成为真正的 crate facade。

本阶段只做测试结构治理：

```text
不新增编辑器功能
不改变运行时行为
不减少测试
不改变 public API
不把所有测试简单搬到单个 tests.rs 继续堆
```

## 2. 目标结构

新增：

```text
rust/crates/editor_window_winit/src/tests/mod.rs
rust/crates/editor_window_winit/src/tests/support.rs
rust/crates/editor_window_winit/src/tests/window_surface.rs
rust/crates/editor_window_winit/src/tests/viewport_runtime.rs
rust/crates/editor_window_winit/src/tests/gate_report.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/editor_window_winit/src/tests/native_real_window.rs
rust/crates/editor_window_winit/src/tests/input_runtime_loop.rs
```

`lib.rs` 目标只保留：

```rust
#[cfg(test)]
mod tests;
```

## 3. 拆分归属

```text
window_surface.rs:
  window_plan / headless_window / surface 测试。

viewport_runtime.rs:
  ViewportHost / HeadlessRuntimeRenderer / viewport texture / minimal runtime viewport 测试。

gate_report.rs:
  RealWindowGateReport / RealNativeEditorWindowReport / UiGpuDrawPlan 测试。

native_app.rs:
  HeadlessNativeEditorWindowApp / NativeEditorApplication / command / project launcher / inspector / AI proposal 测试。

native_real_window.rs:
  real-window feature 专属测试和 ignored smoke。

input_runtime_loop.rs:
  ViewportInputGateway / RuntimeInputFrame / InputResolver / project logic to render extract 测试。

support.rs:
  fixture_model / rect / fire_move_runner / project fixture / runtime package fixture 等共享测试工具。
```

## 4. 分阶段施工

### C0：施工前基线

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Hygiene-StageC-editor_window_winit测试拆分\before.json
cargo test -p editor_window_winit
```

### C1：建立 tests 目录和 support

移动共享 imports / fixtures / helpers 到：

```text
src/tests/support.rs
```

最小验证：

```powershell
cargo test -p editor_window_winit window_attributes_keep_native_editor_defaults
```

### C2：拆 window / surface / viewport / report 测试

拆到：

```text
window_surface.rs
viewport_runtime.rs
gate_report.rs
```

验证：

```powershell
cargo test -p editor_window_winit window_attributes_keep_native_editor_defaults
cargo test -p editor_window_winit viewport_host_registers_resizes_focuses_and_rejects_second_scene_viewport
cargo test -p editor_window_winit report_serializes_and_reports_surface_loss
cargo test -p editor_window_winit
```

### C3：拆 Native Editor Application 测试

拆到：

```text
native_app.rs
native_real_window.rs
```

验证：

```powershell
cargo test -p editor_window_winit native_editor_application_has_complete_shell_registry
cargo test -p editor_window_winit native_editor_application_click_create_uses_headless_dialog_and_persists_recent
cargo test -p editor_window_winit native_editor_application_commits_focused_property_edit_through_transaction
cargo test -p editor_window_winit --features real-window --no-run
```

### C4：拆输入到运行时闭环测试

拆到：

```text
input_runtime_loop.rs
```

验证：

```powershell
cargo test -p editor_window_winit game_view_focused_key_down_routes_to_runtime_input_frame
cargo test -p editor_window_winit game_view_space_key_drives_project_logic_and_render_extract
cargo test -p editor_window_winit
```

### C5：最终回归和文档归档

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Hygiene-StageC-editor_window_winit测试拆分\after.json
cargo fmt --check
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p code_hygiene
cargo test -p editor_window_winit --features real-window --no-run
```

完成后：

```text
生成阶段完成记录/2026-07-02-Codebase-Hygiene-StageC-editor_window_winit测试拆分/00-总览.md
移动本施工文档到 施工文档/已完成/
更新 00 / 49 / 54 / 施工文档 README / 阶段完成记录 README
```

## 5. 验收标准

```text
lib.rs 不再承载大型测试实现。
editor_window_winit 测试数量不减少。
real-window feature 编译通过。
code_hygiene after.json 显示 editor_window_winit/src/lib.rs 继续下降。
没有新增运行时或编辑器功能。
```

## 6. 施工自审

```text
Specification fit:
  满足用户要求：生成 Stage C 施工文档并直接施工。

Rule fit:
  延续 151 / Stage B 规则，只做结构治理，不做功能膨胀。

Textual consistency:
  阶段、目标文件、测试命令和归档动作一致。

Design fit:
  让 lib.rs 变成 facade，提升 AI 查找测试和维护测试的能力。

Implementation feasibility:
  当前测试已经集中在单一 tests 模块，适合按领域拆分。

Practical reasonableness:
  每拆一个领域跑对应测试，最后跑完整回归和 feature 编译。
```
