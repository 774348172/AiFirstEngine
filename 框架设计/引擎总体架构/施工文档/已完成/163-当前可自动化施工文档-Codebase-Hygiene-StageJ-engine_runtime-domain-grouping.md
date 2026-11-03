# 163-当前可自动化施工文档-Codebase Hygiene Stage J engine_runtime domain grouping

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
49-当前状态与下一步入口.md
阶段完成记录/2026-07-03-Codebase-Hygiene-StageH-editor_ui_model-domain拆分/00-总览.md
```

当前问题：

```text
engine_runtime 文件数量多，能力覆盖 ECS / FrameLoop / Render / Asset / Input / Physics / Package / Validation 等多个 domain。
lib.rs public module 平铺，AI 和人类检索时缺少稳定 domain 入口。
```

目标：

```text
建立 engine_runtime domain grouping facade。
第一版以 domain/* re-export 和边界文档为主。
不一次性物理移动 60+ runtime 文件。
不破坏现有 public use 路径。
让后续新增 runtime 模块必须选择 domain。
```

## 2. 目标结构

新增：

```text
rust/crates/engine_runtime/src/domain/mod.rs
rust/crates/engine_runtime/src/domain/ecs.rs
rust/crates/engine_runtime/src/domain/frame_loop.rs
rust/crates/engine_runtime/src/domain/render.rs
rust/crates/engine_runtime/src/domain/asset.rs
rust/crates/engine_runtime/src/domain/input.rs
rust/crates/engine_runtime/src/domain/physics.rs
rust/crates/engine_runtime/src/domain/package.rs
rust/crates/engine_runtime/src/domain/logic.rs
rust/crates/engine_runtime/src/domain/validation.rs
```

`lib.rs` 规则：

```text
继续保留现有 public module，避免破坏下游 crate。
新增 pub mod domain。
domain/* 只做稳定分组 re-export，不新增运行时行为。
```

## 3. 分阶段任务

### J-0：施工前基线

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageJ-engine_runtime-domain-grouping\before.json
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

### J-1：建立 domain facade

新增文件：

```text
engine_runtime/src/domain/mod.rs
engine_runtime/src/domain/ecs.rs
engine_runtime/src/domain/frame_loop.rs
engine_runtime/src/domain/render.rs
engine_runtime/src/domain/asset.rs
engine_runtime/src/domain/input.rs
engine_runtime/src/domain/physics.rs
engine_runtime/src/domain/package.rs
engine_runtime/src/domain/logic.rs
engine_runtime/src/domain/validation.rs
```

阶段测试：

```powershell
cargo test -p engine_runtime
```

### J-2：登记 ECS / FrameLoop / Logic domain

建议分组：

```text
ecs:
  archetype, world, world_api, component_value, components, query, ids, field_path

frame_loop:
  engine_host_loop, frame_loop, runtime_time, default_game_run, runtime_run

logic:
  rule_ir, rule_compiler, rule_registry, logic_executor, project_logic, gameplay_command, gameplay_trace
```

阶段测试：

```powershell
cargo test -p engine_runtime world
cargo test -p engine_runtime frame_loop
cargo test -p engine_runtime project_logic
```

### J-3：登记 Render / Asset / Package domain

建议分组：

```text
render:
  engine_rhi, render_command, render_state, runtime_renderer, render_graph, render_resource, wgpu_backend, sprite2d_render_pipeline

asset:
  runtime_asset, runtime_asset_loader, render_asset_production, render_asset_bridge

package:
  runtime_package, runtime_package_builder, runtime_instance, runtime_instance_loader, scene_loader
```

阶段测试：

```powershell
cargo test -p engine_runtime render
cargo test -p engine_runtime runtime_asset
cargo test -p engine_runtime runtime_package
```

### J-4：登记 Input / Physics / Validation domain

建议分组：

```text
input:
  input_action, input_mapping

physics:
  physics2d

validation:
  golden, complex_project_validation, diagnostics, runtime_trace, frame_hash
```

阶段测试：

```powershell
cargo test -p engine_runtime input
cargo test -p engine_runtime physics
cargo test -p engine_runtime golden
```

### J-5：整体回归

```powershell
cargo fmt
cargo fmt --check
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo test -p editor_core
cargo test -p code_hygiene
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageJ-engine_runtime-domain-grouping\after.json
```

## 4. 不做什么

```text
不一次性移动所有 runtime 文件。
不破坏现有 public use 路径。
不改 RuntimePackage schema。
不改 FrameLoop / renderer / asset loader 行为。
不把 domain facade 变成第二套业务实现。
```

## 5. 完成后文档同步

施工完成后必须：

```text
新增 阶段完成记录/2026-07-03-Codebase-Hygiene-StageJ-engine_runtime-domain-grouping/00-总览.md
把本施工文档移动到 施工文档/已完成/
更新 49-当前状态与下一步入口.md
更新 54-当前可自动化施工总入口.md
更新 施工文档/README.md
更新 阶段完成记录/README.md
更新 00-文档地图.md
```

下一施工入口：

```text
施工文档/当前/164-当前可自动化施工文档-Codebase-Hygiene-StageK最终收敛与入口同步.md
```

## 6. 验收标准

```text
engine_runtime::domain::* 可以作为新代码和 AI 检索入口。
现有下游 crate 不需要大规模改 use 路径。
engine_runtime / runtime_player_winit / editor_core / code_hygiene 回归通过。
新增 domain facade 没有引入运行时行为变化。
code_hygiene after.json 已生成并写入阶段完成记录。
```

## 7. 施工文档自审

```text
Specification fit:
  对应剩余 Stage J，只生成施工文档，不开始施工。

Rule fit:
  符合 151 中 engine_runtime 先建 domain grouping facade、不大规模移动文件的规则。

Textual consistency:
  目标 domain、建议 re-export 范围和阶段测试一致。

Design fit:
  提供 runtime 大系统的稳定入口，提升复杂项目维护和 AI 检索能力。

Implementation feasibility:
  re-export facade 风险低，不要求大规模 use 路径迁移。

Practical reasonableness:
  先建立长期边界，再逐步迁移文件，避免一次性大重构。
```
