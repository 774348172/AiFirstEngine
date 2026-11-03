# 226 当前可自动化施工文档 - PrefabInstance RuntimePackage Bake / Authoring Prefab Instance Expansion v1

## 1. 施工目标

按 `226-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion-v1方案.md` 执行 C-min：

```text
Scene 中预放的 engine.prefab_instance
  -> ProjectRuntimePackageAssembler 构建期 bake
  -> flattened RuntimeScene entities
  -> RuntimePackage / Editor Play / Export 真实运行
```

同时保留：

```text
RuntimePackage prefabs/*.json
RuntimeInstanceLoader::instantiate_prefab_from_package(...)
```

用于运行中动态 spawn。

本轮不新增 gameplay 专用 API，不新增 Runtime Prefab Authoring 解释器，不新增 Scene 导出桥，不实现 nested prefab / variant / async incremental bake。

## 2. 前置复核

已读取正式方案：

```text
框架设计/引擎总体架构/226-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion-v1方案.md
```

已读取外部审查：

```text
G:\gameEngin\4.8AI审查目录\01-226-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion方案审查.md
```

外部审查结论处理：

```text
必须修改：
  已写回 226 方案：PrefabBakeCatalog、override-before-remap、sibling/enabled、reserved separator、enemy-b 样例一致性。

施工约束：
  本施工文档 Gate 必须覆盖上述五项。

已由当前仓库状态吸收：
  225 闭环已复核：施工文档/当前/ 为空，225 已归档到 已完成，225 阶段完成记录存在，49/54 已记录 225 完成。

不适用：
  外部审查未要求推翻 C-min，未要求 nested prefab / variant / runtime authoring interpreter。
```

当前施工文档目录复核：

```text
施工文档/当前/ 生成本文档前为空。
本文档生成后为唯一当前施工文档。
```

## 3. 涉及文件

预计修改：

```text
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/prefab_workflow.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/project_e2e_gate/src/*.rs
samples/complex_shooter_project/Scenes/Main.scene.json
samples/complex_shooter_project/Prefabs/enemy_scout.prefab.json
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-09-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion-v1/00-总览.md
```

可能只读参考：

```text
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
rust/crates/project_e2e_gate/src/prefab_authoring.rs
rust/crates/project_e2e_gate/src/authoring_asset_completeness.rs
```

## 4. 分 Gate 任务

### Gate A：Prefab bake 数据结构与 assembler 报告

任务：

```text
新增 PrefabRuntimeBakeReport / PrefabRuntimeBakeInstanceEntry / PrefabRuntimeBakeDiagnostic。
ProjectRuntimePackageAssemblyReport 接入 prefab_bake_report。
新增 PrefabBakeCatalog：runtime_source_prefabs + authoring_prefab_by_id。
collect_prefabs 后构建 catalog，先 catalog 再 scene bake，最后 input.prefabs 保留原始 source prefabs。
```

必须覆盖：

```text
authoring-prefab-asset.v1 可进入 authoring_prefab_by_id。
Scene PrefabInstance 引用 runtime-prefab.v1 但无 authoring PrefabAsset 时失败。
```

测试命令：

```powershell
cd rust
cargo test -p editor_core project_runtime_package_assembler -- --nocapture
```

### Gate B：PrefabInstance 展开与 deterministic remap

任务：

```text
editor_scene_to_runtime 改为 prefab-aware bake。
无 PrefabInstance 的普通 Entity 保持现有路径。
有 PrefabInstance 的 Entity：
  PrefabInstance::from_scene_entity
  ResolvedPrefabView::resolve
  override-before-remap
  root sourceEntityId -> Scene placeholder entity id
  child sourceEntityId -> {scene_placeholder_entity_id}__{sourceEntityId}
  engine.prefab_instance 不进入 RuntimeEntity components
```

必须覆盖：

```text
root placement 使用 Scene placeholder transform。
root sibling_order / enabled 继承 Scene placeholder + resolved root。
child sibling_order / enabled 继承 resolved prefab child。
sourceEntityId 包含 "__" 时输出 prefab_source_entity_id_contains_reserved_separator。
```

测试命令：

```powershell
cd rust
cargo test -p editor_core prefab -- --nocapture
cargo test -p editor_core project_runtime_package_assembler -- --nocapture
```

### Gate C：complex shooter sample 收敛

任务：

```text
enemy_scout.prefab.json 补齐 SpriteRenderer2D 和 project.linearMotion。
entity-enemy-a 移除重复本地 SpriteRenderer2D / project.linearMotion，只保留 PrefabInstance + Scene placement / override。
entity-enemy-b 默认迁移为 prefab-enemy-scout 的 Scene PrefabInstance。
```

必须覆盖：

```text
两个 Enemy Scout 都能通过 prefab-enemy-scout bake。
sample 不依赖 authoring-only engine.prefab_instance 在 RuntimeScene 中运行。
```

测试命令：

```powershell
cd rust
cargo test -p project_e2e_gate authoring_asset_completeness -- --nocapture
cargo test -p project_e2e_gate prefab_authoring -- --nocapture
```

### Gate D：project_e2e_gate prefab_runtime_bake

任务：

```text
新增 project_e2e_gate prefab_runtime_bake gate。
生成 complex-shooter-prefab-runtime-bake-report.json。
断言 PrefabRuntimeBakeReport bakedInstanceCount >= 2。
断言 entity-enemy-a / entity-enemy-b 均由 prefab-enemy-scout bake。
断言 RuntimePackage scene 不含 authoring-only engine.prefab_instance。
```

测试命令：

```powershell
cd rust
cargo test -p project_e2e_gate prefab_runtime_bake -- --nocapture
cargo test -p project_e2e_gate -- --nocapture
```

### Gate E：整体回归与文档同步

任务：

```text
cargo fmt --check。
editor_core / engine_runtime / project_e2e_gate 整体回归。
生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
把本文档从 施工文档/当前/ 移动到 施工文档/已完成/。
```

测试命令：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p project_e2e_gate
```

## 5. 文档同步与归档规则

完成后必须新增：

```text
框架设计/引擎总体架构/阶段完成记录/2026-07-09-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion-v1/00-总览.md
```

完成后必须更新：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

归档：

```text
施工文档/当前/226-当前可自动化施工文档-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion-v1.md
  -> 施工文档/已完成/226-当前可自动化施工文档-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion-v1.md
```

## 6. 施工自审

范围自审：

```text
本施工只实现 226 C-min。
不做 nested prefab、variant、unpack/replace、async incremental bake。
不新增 runtime authoring-only prefab_instance 解释器。
不新增 gameplay 专用 API。
```

外部审查吸收自审：

```text
已继承 01-226 审查中的 PrefabBakeCatalog / override-before-remap / sibling-enabled / reserved separator / enemy-b 一致性要求。
已将 225 闭环问题判定为当前仓库状态已吸收，但仍作为施工前复核约束。
```

测试自审：

```text
每个 Gate 都有对应 cargo test。
最终必须跑 fmt、editor_core、engine_runtime、project_e2e_gate。
不允许只靠 report 文字宣称完成。
```

风险自审：

```text
project_runtime_package_assembler.rs 当前是 RuntimePackage 唯一装配入口，本轮改动必须保持 DesktopExport / EditorPreviewPackage 共享同一路径。
sample 迁移必须保持复杂打飞机 e2e 不退化。
如果 Gate 中发现方案规则无法落地，应先回填 226 方案或本文档，再继续施工。
```
