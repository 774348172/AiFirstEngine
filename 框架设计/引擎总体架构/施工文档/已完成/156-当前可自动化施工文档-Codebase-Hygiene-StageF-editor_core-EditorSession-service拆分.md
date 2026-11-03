# 156-当前可自动化施工文档-Codebase Hygiene Stage F editor_core / EditorSession service 拆分

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
49-当前状态与下一步入口.md
阶段完成记录/2026-07-02-Codebase-Hygiene-StageE-editor_wgpu_renderer拆分/00-总览.md
```

本阶段执行：

```text
Codebase Architecture Hygiene Gate v1 - Stage F editor_core / EditorSession service split
```

Stage F 的长期目标是：

```text
保持 EditorSession 作为外部 facade。
逐步把 project / scene / build / play / ai / ui model composer 迁入领域 service。
不改变 EditorSession public API。
不改变 UI command 入口。
不改变现有 editor_core / editor_window_winit 行为。
```

本施工文档只执行 Stage F1：

```text
先让 editor_core/src/lib.rs 退出实现承载位。
把 EditorSession 主体迁到 session.rs。
把 lib.rs 内大测试迁到 tests.rs。
保留现有领域模块不重写。
不在本轮深拆 project/scene/build/play/ai service。
```

这样做的原因：

```text
editor_core 是当前最大热点。
直接深拆所有 service 会同时改变太多私有调用边界，风险过高。
先完成 lib.rs facade 化，可以为后续 F2/F3 领域 service 拆分建立清晰入口。
```

## 2. 当前基线

当前热点：

```text
rust/crates/editor_core/src/lib.rs: 5331 lines
rust/crates/editor_core/src/scene_editing.rs: 2404 lines
rust/crates/editor_core/src/inspector_details.rs: 1644 lines
```

当前 `lib.rs` 同时承载：

```text
module declarations
public re-export
EditorSession state
EditorSession command dispatch
project / scene / runtime / build / ai command implementation
UI model composer
diagnostic helper
test helpers
large tests module
```

## 3. 目标目录

本轮目标结构：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/tests.rs
```

`lib.rs` 最终只保留：

```text
mod declarations
public re-export
#[cfg(test)] mod tests;
```

`session.rs` 暂时承接：

```text
CommandStatus / UndoPolicy / StateChangeSummary / CommandTransaction / CommandResult
EditorSession
EditorSession impl
session helper functions
command_for_test
```

`tests.rs` 承接：

```text
原 lib.rs 内 #[cfg(test)] mod tests 的测试内容
```

## 4. 本轮不做什么

```text
不新增编辑器功能。
不修改 UiCommandPayload -> EditorSession 行为。
不修改 EditorCommandRegistry / EditorCommandExecutor 语义。
不修改 SceneEdit / BuildExport / PlaySession / AI 业务流程。
不改 report schema。
不把项目 gameplay 概念加入引擎底座。
不在本轮把 session.rs 再强行拆成十几个 service。
```

## 5. 分阶段施工

### F0：施工前基线

生成阶段目录：

```text
框架设计/引擎总体架构/阶段完成记录/2026-07-02-Codebase-Hygiene-StageF-editor_core-EditorSession-service拆分
```

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Hygiene-StageF-editor_core-EditorSession-service拆分\before.json
cargo test -p editor_core
cargo test -p editor_window_winit
```

### F1：迁移 EditorSession 主体到 session.rs

移动：

```text
EditorSession
CommandStatus
UndoPolicy
StateChangeSummary
CommandTransaction
CommandResult
command_for_test
session helper functions
```

要求：

```text
EditorSession public API 不变。
CommandResult / CommandTransaction 等 public type 仍从 crate root 导出。
EditorCommandExecutor 调用路径不变。
```

验证：

```powershell
cargo test -p editor_core editor_session_creates_empty_ui_model
cargo test -p editor_core tick_without_runtime_package_returns_structured_diagnostic
cargo test -p editor_core editor_session_execute_command_scene_opens_scene_document
cargo test -p editor_core
```

### F2：迁移 lib.rs 测试到 tests.rs

移动：

```text
#[cfg(test)] mod tests
```

要求：

```text
测试数量不减少。
测试 helper 继续只在测试模块内存在。
测试迁移不改变生产代码。
```

验证：

```powershell
cargo test -p editor_core
```

### F3：lib.rs facade 收敛

目标：

```text
lib.rs 只保留模块声明、pub use、测试入口。
新增功能禁止继续写入 lib.rs。
后续 F2/F3 service 拆分必须从 session.rs 继续拆。
```

验证：

```powershell
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p editor_host
```

### F4：最终回归与归档

执行：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Hygiene-StageF-editor_core-EditorSession-service拆分\after.json
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p code_hygiene
```

完成后：

```text
生成 阶段完成记录/2026-07-02-Codebase-Hygiene-StageF-editor_core-EditorSession-service拆分/00-总览.md
移动本施工文档到 施工文档/已完成
更新 00-文档地图 / 49 / 54 / README / 施工文档 README / 阶段完成记录 README
```

## 6. 验收标准

```text
editor_core/src/lib.rs 变成 facade 级文件。
EditorSession public API 兼容。
CommandResult / CommandTransaction / CommandStatus 等 public type 兼容。
editor_core 测试不减少。
editor_window_winit 回归通过。
editor_host 回归通过。
code_hygiene after.json 显示 editor_core/src/lib.rs 明显下降。
没有新增编辑器功能、运行时功能或项目侧 gameplay 规则。
```

## 7. 施工文档自审

```text
Specification fit:
  满足用户要求：生成 Stage F editor_core / EditorSession service 拆分施工文档，自审后直接施工。

Rule fit:
  延续 151 的 Stage F 目标，保持 EditorSession facade，不改变命令入口和外部 API。

Textual consistency:
  本轮明确是 Stage F1：先 lib.rs facade 化和 session.rs 迁移，后续再继续深拆领域 service。

Design fit:
  符合长期主义：先建立稳定模块边界，避免所有领域 service 在同一轮重切导致不可控风险。

Implementation feasibility:
  当前 lib.rs 已经有清晰分界：pub use 区、EditorSession 主体、helper、tests，可机械迁移并用现有测试兜底。

Practical reasonableness:
  每一步都有 editor_core 和 editor_window_winit 回归，先控制风险，再进入下一轮 service 深拆。
```

结论：

```text
本施工文档通过自审，可以开始 Stage F1 施工。
```
