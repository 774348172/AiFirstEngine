# 162-当前可自动化施工文档-Codebase Hygiene Stage I editor_core 测试 domain 迁移

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
阶段完成记录/2026-07-03-Codebase-Hygiene-StageF3-EditorSession-command-service拆分/00-总览.md
阶段完成记录/2026-07-03-Codebase-Hygiene-StageH-editor_ui_model-domain拆分/00-总览.md
```

当前问题：

```text
editor_core 的测试保护网已经比较充足。
但测试高度集中在少数大测试文件里，后续重构时定位成本高。
```

目标：

```text
把 editor_core 集成测试按 domain 迁移。
测试数量不减少。
测试语义不改变。
失败信息更接近对应 service/module。
生产代码只在必要的 pub(crate) 可见性问题上做最小调整。
```

## 2. 目标结构

推荐目标：

```text
rust/crates/editor_core/src/tests/
  mod.rs
  fixtures.rs
  project_service_tests.rs
  runtime_service_tests.rs
  scene_service_tests.rs
  build_service_tests.rs
  play_service_tests.rs
  ai_service_tests.rs
  authoring_workspace_tests.rs
  editable_project_loop_tests.rs
```

规则：

```text
测试 helper 进入 fixtures.rs。
每个测试文件只放一个主要 domain 的测试。
测试函数命名保留原语义，必要时只做 domain 前缀整理。
不把集成测试弱化成 mock-only 单测。
```

## 3. 分阶段任务

### I-0：施工前基线

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageI-editor_core测试domain迁移\before.json
cargo test -p editor_core
```

### I-1：建立 tests 目录与公共 fixture

迁移或抽取：

```text
sample runtime package fixture
sample scene fixture
project temp dir helper
通用 EditorSession setup helper
```

落点：

```text
editor_core/src/tests/mod.rs
editor_core/src/tests/fixtures.rs
```

阶段测试：

```powershell
cargo test -p editor_core
```

### I-2：迁移 project / runtime / scene service tests

迁移范围：

```text
project_open_builds_project_browser_entries
project_browser_open_scene_document_refreshes_hierarchy
open_runtime_package_builds_hierarchy_model
tick_one_frame_updates_viewport_and_trace_model
select_entity_updates_readonly_inspector
scene edit command tests under EditorSession path
```

落点：

```text
tests/project_service_tests.rs
tests/runtime_service_tests.rs
tests/scene_service_tests.rs
```

阶段测试：

```powershell
cargo test -p editor_core project_service
cargo test -p editor_core runtime_service
cargo test -p editor_core scene_service
cargo test -p editor_core
```

### I-3：迁移 build / play / AI tests

迁移范围：

```text
build_export_*
editor_session_play_*
editor_session_ai_*
ai_scene_edit_*
```

落点：

```text
tests/build_service_tests.rs
tests/play_service_tests.rs
tests/ai_service_tests.rs
```

阶段测试：

```powershell
cargo test -p editor_core build_export
cargo test -p editor_core play
cargo test -p editor_core ai
cargo test -p editor_core
```

### I-4：迁移 authoring workspace / editable loop tests

迁移范围：

```text
project_authoring_workspace_*
editable_project_loop_*
asset_authoring_loop_*
workspace_domain_summary_*
input_mapping authoring tests if currently placed in editor_core tests
```

落点：

```text
tests/authoring_workspace_tests.rs
tests/editable_project_loop_tests.rs
```

阶段测试：

```powershell
cargo test -p editor_core authoring_workspace
cargo test -p editor_core editable_project_loop
cargo test -p editor_core asset_authoring_loop
cargo test -p editor_core
```

### I-5：整体回归

```powershell
cargo fmt
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p code_hygiene
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageI-editor_core测试domain迁移\after.json
```

## 4. 不做什么

```text
不删除测试。
不降低断言强度。
不把原本覆盖真实 EditorSession 路径的测试改成只测 mock。
不顺手改生产逻辑。
不改变测试目标行为来适配拆分。
```

## 5. 完成后文档同步

施工完成后必须：

```text
新增 阶段完成记录/2026-07-03-Codebase-Hygiene-StageI-editor_core测试domain迁移/00-总览.md
把本施工文档移动到 施工文档/已完成/
更新 49-当前状态与下一步入口.md
更新 54-当前可自动化施工总入口.md
更新 施工文档/README.md
更新 阶段完成记录/README.md
更新 00-文档地图.md
```

下一施工入口：

```text
施工文档/当前/163-当前可自动化施工文档-Codebase-Hygiene-StageJ-engine_runtime-domain-grouping.md
```

## 6. 验收标准

```text
editor_core 测试不再高度集中在单一大文件。
测试按 project/runtime/scene/build/play/AI/authoring workspace 等 domain 组织。
测试数量不减少，关键断言不弱化。
editor_core 全量测试通过。
editor_window_winit / editor_host / code_hygiene 回归通过。
code_hygiene after.json 已生成并写入阶段完成记录。
```

## 7. 施工文档自审

```text
Specification fit:
  对应剩余 Stage I，只生成施工文档，不开始施工。

Rule fit:
  符合 151 中“测试随模块迁移，数量不减少，语义不改变”的规则。

Textual consistency:
  每类测试都有目标文件、迁移范围和阶段测试。

Design fit:
  提升 AI 定位失败测试的能力，降低后续重构风险。

Implementation feasibility:
  Rust 支持 tests/ 子模块，迁移主要是结构调整，风险可控。

Practical reasonableness:
  先抽 fixture，再按 domain 迁移，避免 helper 复制和重复 setup。
```
