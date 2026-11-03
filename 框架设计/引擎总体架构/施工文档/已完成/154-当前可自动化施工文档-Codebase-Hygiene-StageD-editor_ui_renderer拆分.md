# 154-当前可自动化施工文档-Codebase Hygiene Stage D editor_ui_renderer 拆分

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
```

本阶段执行：

```text
Codebase Architecture Hygiene Gate v1 - Stage D editor_ui_renderer split
```

目标是把 `rust/crates/editor_ui_renderer/src/lib.rs` 从单文件 UI 渲染实现拆成可维护的渲染结构：

```text
draw_list: 公开 DrawCommand / HitRegion / UiDrawList 等输出契约
theme: 编辑器 UI 颜色与样式常量
layout: 编辑器窗口区域布局与通用矩形工具
hit_test: hit region 命中查询
renderer: SelfUiRenderer facade 和 build_draw_list 编排
panels: launcher / toolbar / hierarchy / inspector / viewport / console / runtime_trace / workspace / ai_panel / build_export / project_browser
```

本阶段只做结构治理：

```text
不新增编辑器功能
不改变 UI 可见布局和点击行为
不改变 public API 的 crate root 导出
不改变 DrawList / HitRegion / HitTarget 语义
不把业务执行逻辑放进 editor_ui_renderer
不为降行数制造无意义 wrapper
```

## 2. 目标目录

目标结构：

```text
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_ui_renderer/src/draw_list.rs
rust/crates/editor_ui_renderer/src/theme.rs
rust/crates/editor_ui_renderer/src/layout.rs
rust/crates/editor_ui_renderer/src/hit_test.rs
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_ui_renderer/src/panels/mod.rs
rust/crates/editor_ui_renderer/src/panels/launcher.rs
rust/crates/editor_ui_renderer/src/panels/toolbar.rs
rust/crates/editor_ui_renderer/src/panels/hierarchy.rs
rust/crates/editor_ui_renderer/src/panels/inspector.rs
rust/crates/editor_ui_renderer/src/panels/viewport.rs
rust/crates/editor_ui_renderer/src/panels/console.rs
rust/crates/editor_ui_renderer/src/panels/runtime_trace.rs
rust/crates/editor_ui_renderer/src/panels/workspace.rs
rust/crates/editor_ui_renderer/src/panels/ai_panel.rs
rust/crates/editor_ui_renderer/src/panels/build_export.rs
rust/crates/editor_ui_renderer/src/panels/project_browser.rs
```

`lib.rs` 最终只保留 crate facade、公开 re-export 和测试入口。

## 3. 拆分归属

```text
draw_list.rs:
  UiRect / UiPoint / UiColor / DrawCommand / HitTarget / HitRegion / UiDrawList / UiRendererConfig
  hit_region / command_hit_region 等纯构造 helper

theme.rs:
  颜色、hover/pressed/disabled 等视觉状态 helper
  第一版可以只承接已有颜色访问，不引入新主题系统

layout.rs:
  EditorLayout / layout_for / content_rect / command_width / push_border

renderer.rs:
  SelfUiRenderer
  build_draw_list
  顶层 root/menu/panel 编排

hit_test.rs:
  hit_test
  hit_test_any

panels/*:
  各面板 push_* 绘制逻辑
  panel 模块只能写 DrawCommand / HitRegion，不能执行业务命令
```

## 4. 分阶段施工

### D0：施工前基线

生成阶段目录：

```text
框架设计/引擎总体架构/阶段完成记录/2026-07-02-Codebase-Hygiene-StageD-editor_ui_renderer拆分
```

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Hygiene-StageD-editor_ui_renderer拆分\before.json
cargo test -p editor_ui_renderer
```

### D1：拆 draw_list / hit_test / theme

移动公开数据结构和命中查询：

```text
src/draw_list.rs
src/hit_test.rs
src/theme.rs
```

保留 crate root re-export：

```rust
pub use draw_list::{DrawCommand, HitRegion, HitTarget, UiColor, UiDrawList, UiPoint, UiRect, UiRendererConfig};
pub use hit_test::{hit_test, hit_test_any};
```

最小验证：

```powershell
cargo test -p editor_ui_renderer renderer_hit_test_resolves_toolbar_command
cargo test -p editor_ui_renderer disabled_toolbar_command_keeps_hit_region_with_reason
cargo test -p editor_ui_renderer
```

### D2：拆 layout / renderer 顶层编排

移动：

```text
src/layout.rs
src/renderer.rs
```

要求：

```text
SelfUiRenderer 仍从 crate root 导出
build_draw_list 行为不变
layout_for 只描述 UI 区域，不携带业务状态
```

验证：

```powershell
cargo test -p editor_ui_renderer renderer_builds_unity_like_editor_shell
cargo test -p editor_ui_renderer project_launcher_draws_shell_and_recent_projects
cargo test -p editor_ui_renderer
```

### D3：拆核心面板

移动：

```text
src/panels/toolbar.rs
src/panels/hierarchy.rs
src/panels/inspector.rs
src/panels/viewport.rs
src/panels/launcher.rs
```

要求：

```text
面板只接收 model/view rect/config 所需数据
面板只产生 DrawCommand / HitRegion
面板不调用 EditorSession / CommandCatalog / Runtime
```

验证：

```powershell
cargo test -p editor_ui_renderer hierarchy_actions_expose_create_and_delete_hit_regions
cargo test -p editor_ui_renderer inspector_fields_use_stable_field_targets
cargo test -p editor_ui_renderer project_launcher_create_and_open_actions_are_clickable
cargo test -p editor_ui_renderer
```

### D4：拆辅助面板

移动：

```text
src/panels/console.rs
src/panels/runtime_trace.rs
src/panels/workspace.rs
src/panels/ai_panel.rs
src/panels/build_export.rs
src/panels/project_browser.rs
```

验证：

```powershell
cargo test -p editor_ui_renderer project_authoring_workspace_draws_project_browser_entries
cargo test -p editor_ui_renderer build_export_workspace_draws_report_and_commands
cargo test -p editor_ui_renderer ai_panel_draws_review_state_and_proposals
cargo test -p editor_ui_renderer
```

### D5：最终回归与归档

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Hygiene-StageD-editor_ui_renderer拆分\after.json
cargo fmt --check
cargo test -p editor_ui_renderer
cargo test -p editor_input
cargo test -p editor_window_winit
cargo test -p code_hygiene
```

完成后：

```text
生成阶段完成记录/2026-07-02-Codebase-Hygiene-StageD-editor_ui_renderer拆分/00-总览.md
移动本施工文档到 施工文档/已完成
更新 00 / 49 / 54 / README / 施工文档 README / 阶段完成记录 README
```

## 5. 验收标准

```text
editor_ui_renderer/src/lib.rs 变成 facade 级文件。
editor_ui_renderer 不再是单文件 crate。
DrawList / HitRegion / HitTarget public API 兼容。
现有 editor_ui_renderer 测试不减少。
editor_input 与 editor_window_winit 回归通过。
code_hygiene after.json 显示 editor_ui_renderer/src/lib.rs 明显下降。
没有新增编辑器功能、运行时功能或项目侧 gameplay 概念。
```

## 6. 施工文档自审

```text
Specification fit:
  满足用户要求：生成 Stage D editor_ui_renderer 拆分施工文档，审查后直接施工。

Rule fit:
  延续 151 的 Stage D 边界，只做结构治理，不改 DrawList / HitRegion 输出契约，不引入项目侧规则。

Textual consistency:
  目标目录、拆分归属、阶段验证和最终验收一致，均围绕 editor_ui_renderer 单文件治理。

Design fit:
  让 UI renderer 形成长期可维护边界：数据契约、布局、命中、面板、renderer facade 分离，利于 AI 查找和后续面板扩展。

Implementation feasibility:
  当前 lib.rs 内部函数已经天然按 push_* 面板和 helper 聚合，可以机械迁移并通过模块可见性维持行为。

Practical reasonableness:
  按 D1-D5 分阶段迁移，每阶段都有最小测试和最终回归，避免一次性大改后难以定位错误。
```

结论：

```text
本施工文档通过自审，可以开始 Stage D 施工。
```
