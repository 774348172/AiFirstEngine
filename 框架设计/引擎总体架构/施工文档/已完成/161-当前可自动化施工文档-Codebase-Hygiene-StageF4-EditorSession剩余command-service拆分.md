# 161-当前可自动化施工文档-Codebase Hygiene Stage F4 EditorSession 剩余 command service 拆分

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
阶段完成记录/2026-07-03-Codebase-Hygiene-StageF3-EditorSession-command-service拆分/00-总览.md
阶段完成记录/2026-07-03-Codebase-Hygiene-StageH-editor_ui_model-domain拆分/00-总览.md
```

Stage F1-F3 已经把 `editor_core` 的入口、事务、UI model composition、project/runtime/scene command service 拆出。Stage G/H 又完成了 `scene_editing` 和 `editor_ui_model` 热点治理。

本阶段继续治理 `editor_core/src/session.rs` 中剩余的 command service：

```text
console command
build command
play command
AI panel command
AI mock planner helper
```

目标：

```text
保持 EditorSession public facade 不变。
保持 execute_command / execute_ui_command_direct 调用路径不变。
把剩余 command method 按 domain 归口到 services/*。
不引入新的业务抽象层，不改 transaction schema，不改 UI command payload。
```

## 2. 目标结构

目标文件：

```text
rust/crates/editor_core/src/services/console_service.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/services/mod.rs
```

`session.rs` 最终保留：

```text
EditorSession struct / Default / new
execute_command / execute_ui_command_direct
read-only accessor
begin_transaction / finish_transaction
少量跨 service 共享 diagnostic helper
测试必须依赖的 public facade
```

共享 helper 的边界：

```text
如果 push_info / push_error / make_diagnostic / diagnostic_to_console_entry 被多个 service 共用，可以暂时留在 session.rs。
如果迁移会制造循环依赖或让 service 之间互相调用，则不迁移。
本阶段不为了追求行数强拆难懂 helper。
```

## 3. 分阶段任务

### F4-0：施工前基线

只记录当前真实状态，不修改代码。

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageF4-EditorSession剩余command-service拆分\before.json
cargo test -p editor_core
```

### F4-1：迁移 console command

迁移：

```text
clear_console
```

落点：

```text
services/console_service.rs
```

阶段测试：

```powershell
cargo test -p editor_core tests::clear_console_is_snapshot_ready_transaction
cargo test -p editor_core
```

### F4-2：迁移 build command

迁移：

```text
export_desktop_package
open_build_output
open_build_report
```

落点：

```text
services/build_service.rs
```

阶段测试：

```powershell
cargo test -p editor_core tests::build_export_desktop_package_updates_report_model
cargo test -p editor_core tests::build_export_open_output_requires_prior_export
cargo test -p editor_core tests::build_export_open_report_after_export_reports_path
cargo test -p editor_core desktop_export
```

### F4-3：迁移 play command

迁移：

```text
start_play_session
stop_play_session
```

按依赖情况评估是否一并迁移：

```text
push_play_session_report
```

落点：

```text
services/play_service.rs
```

阶段测试：

```powershell
cargo test -p editor_core tests::editor_session_play_requires_runtime_package
cargo test -p editor_core tests::editor_session_play_runs_headless_session_when_package_loaded
cargo test -p editor_core tests::editor_session_play_adds_console_summary
cargo test -p editor_core tests::editor_session_stop_without_active_session_reports_diagnostic
cargo test -p editor_core play_session
```

### F4-4：迁移 AI panel command

迁移：

```text
submit_ai_prompt
accept_ai_proposed_command
reject_ai_proposed_command
plan_ai_response
extract_quoted_text
extract_name_after_keyword
```

落点：

```text
services/ai_service.rs
```

阶段测试：

```powershell
cargo test -p editor_core tests::editor_session_ai_prompt_proposes_command
cargo test -p editor_core tests::editor_session_accept_ai_command_executes_scene_edit
cargo test -p editor_core tests::ai_scene_edit_command_uses_editor_session_transaction_path
cargo test -p editor_core tests::ai_scene_edit_command_failure_reports_console_diagnostic
```

### F4-5：评估 diagnostic helper 是否迁移

候选：

```text
push_info
push_error
make_diagnostic
diagnostic_to_console_entry
```

规则：

```text
如果 helper 只服务单一 domain，可以迁移到对应 service。
如果 helper 被多个 service 共用，保留在 session.rs 或单独进入 diagnostic_service.rs。
如果单独迁移会造成循环依赖，保留在 session.rs。
```

阶段测试：

```powershell
cargo test -p editor_core
```

### F4-6：整体回归

```powershell
cargo fmt
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p code_hygiene
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageF4-EditorSession剩余command-service拆分\after.json
```

## 4. 不做什么

```text
不拆 execute_ui_command_direct dispatcher。
不引入完整 Service trait 框架。
不改变 EditorSession 对外 API。
不改变 AI mock planner 的能力边界。
不把 build / play / AI 变成项目侧 gameplay 能力。
不修改 UI command payload / transaction schema / runtime package schema。
```

## 5. 完成后文档同步

施工完成后必须：

```text
新增 阶段完成记录/2026-07-03-Codebase-Hygiene-StageF4-EditorSession剩余command-service拆分/00-总览.md
把本施工文档移动到 施工文档/已完成/
更新 49-当前状态与下一步入口.md
更新 54-当前可自动化施工总入口.md
更新 施工文档/README.md
更新 阶段完成记录/README.md
更新 00-文档地图.md
```

下一施工入口：

```text
施工文档/当前/162-当前可自动化施工文档-Codebase-Hygiene-StageI-editor_core测试domain迁移.md
```

## 6. 验收标准

```text
session.rs 不再承载 build / play / AI / console command method 的主体实现。
EditorSession public facade 不变。
所有 command transaction 语义不变。
editor_core / editor_window_winit / editor_host / code_hygiene 回归通过。
code_hygiene after.json 已生成并写入阶段完成记录。
```

## 7. 施工文档自审

```text
Specification fit:
  对应用户要求的 Codebase Hygiene 剩余阶段施工文档，不开始施工。

Rule fit:
  遵守 151 治理规则，保持 facade，按阶段迁移，每阶段测试。

Textual consistency:
  目标 service、迁移方法、测试命令和验收标准一致。

Design fit:
  继续降低 EditorSession 聚合风险，提升 AI 检索和长期维护能力。

Implementation feasibility:
  F2/F3 已验证 services/* 中 impl EditorSession 的路线可行。

Practical reasonableness:
  只迁移剩余 command service，helper 按依赖实际判断，避免强拆。
```
