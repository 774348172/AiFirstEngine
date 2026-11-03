# 当前可自动化施工文档：ECS Storage v1

本文档列出当前已经可以交给自动化实现、且不需要继续等待用户决策的 ECS Storage v1 施工任务。

依据：

```text
52-ECS-Storage-v1方案.md
39-Rust-Runtime-ECS路线.md
16-ECS写入与项目规则边界.md
17-Runtime-FrameLoop.md
51-RenderDirtyTracker-RenderExtract-RenderCommand闭环方案.md
```

当前代码状态：

```text
rust/crates/engine_runtime 已存在。
当前 World 使用 HashMap<EntityId, EntityMeta / Transform / Renderable>。
当前 EntityId 是 Source Entity 字符串。
scene_loader 已能把 RuntimeScene 加载进 World。
frame_loop 已有 RenderExtract phase 名称。
render_snapshot.rs 仍是旧 MVP 兼容输出。
```

本次施工目标：

```text
把 engine_runtime 的 World 从 HashMap 最小模型升级为最小 Archetype Table Storage。
保留现有 Runtime Package / Scene Loader / Golden Scenario 的可用性。
每一步都必须有最小单元测试。
不实现完整 Scheduler / RenderCommand / RenderSceneState。
```

## 施工原则

```text
一项一测。
不引入 bevy_ecs / hecs 作为正式依赖。
不破坏现有 Runtime Package 加载测试。
不删除旧 render_snapshot.rs，先标记为 compatibility output。
不让项目层或 AI 面对 Archetype / Table API。
World API 是唯一正式写入入口。
```

## A0：当前 Runtime 回归门禁

目标：

```text
确认现有 Rust Runtime 测试在施工前可运行。
记录当前失败项，避免把环境问题误判为改造问题。
```

执行：

```powershell
cd G:\gameEngin\rust
cargo test -p engine_runtime
```

完成标准：

```text
engine_runtime 当前测试通过，或明确记录现有失败。
不修改业务代码。
```

## A1：引入 RuntimeEntityId / SourceEntityId

目标：

```text
拆分运行时内部 id 和项目源 id。
```

新增 / 修改：

```text
rust/crates/engine_runtime/src/ids.rs
```

数据结构：

```text
RuntimeEntityId { index, generation }
SourceEntityId(String)
```

保留兼容：

```text
现有 EntityId 字符串可以先 type alias / wrapper 到 SourceEntityId。
Scene Loader 对外仍能用 "entity-player" 查回 source id。
```

测试：

```text
runtime_entity_id_generation_prevents_stale_reuse
source_entity_id_roundtrip
runtime_entity_id_orders_deterministically
```

## A2：引入 ComponentTypeId / ComponentRegistry

目标：

```text
让 Component 类型不再依赖 Rust TypeName 或临时字符串。
```

新增 / 修改：

```text
rust/crates/engine_runtime/src/components.rs
```

内置 ComponentTypeId：

```text
engine.name
engine.hierarchy
engine.transform
engine.renderable
```

第一版 ComponentStorageKind：

```text
Table
SparseSet
```

测试：

```text
builtin_component_type_ids_are_stable
component_registry_rejects_duplicate_type_id
component_registry_marks_table_storage_kind
```

## A3：实现 ArchetypeSignature / ArchetypeId

目标：

```text
同一组 Component 无论顺序如何，都得到相同 ArchetypeSignature。
```

新增建议：

```text
rust/crates/engine_runtime/src/archetype.rs
```

数据结构：

```text
ArchetypeId
ArchetypeSignature { component_types: Vec<ComponentTypeId> }
```

规则：

```text
component_types 稳定排序。
重复 ComponentTypeId 报错或去重后诊断。
Signature 可作为 HashMap key。
```

测试：

```text
archetype_signature_is_order_independent
archetype_signature_deduplicates_or_rejects_duplicates
archetype_id_is_stable_for_same_signature
```

## A4：实现最小 ArchetypeTable

目标：

```text
用列式表保存同一 Archetype 下的 Entity 和 Component 数据。
```

新增 / 修改：

```text
rust/crates/engine_runtime/src/archetype.rs
rust/crates/engine_runtime/src/world.rs
```

第一版 ComponentColumn：

```text
Name(Vec<Name>)
Hierarchy(Vec<Hierarchy>)
Transform(Vec<Transform>)
Renderable(Vec<Renderable>)
Dynamic(Vec<RuntimeValue>)  // 可以先预留，不一定马上接 Project Component
```

测试：

```text
archetype_table_pushes_row_with_matching_columns
archetype_table_rejects_missing_required_column
archetype_table_swap_remove_updates_row_count
```

## A5：实现 EntitySlot / EntityLocation / SourceId 映射

目标：

```text
World 能知道每个 RuntimeEntity 当前在哪个 ArchetypeTable 的哪一行。
Trace 能从 RuntimeEntityId 回查 SourceEntityId。
```

修改：

```text
rust/crates/engine_runtime/src/world.rs
```

数据结构：

```text
EntitySlot { generation, alive, source_id, location }
EntityLocation { archetype_id, row }
source_to_runtime: HashMap<SourceEntityId, RuntimeEntityId>
```

测试：

```text
spawn_entity_creates_entity_slot_and_source_mapping
lookup_runtime_id_by_source_id
lookup_source_id_by_runtime_id
despawn_invalidates_runtime_id_generation
```

## A6：World spawn with component bundle

目标：

```text
Scene Loader 可以一次性创建 Entity，并按组件组合进入正确 ArchetypeTable。
```

修改：

```text
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/scene_loader.rs
```

第一版 API：

```text
World::spawn_from_source(source_id, name, hierarchy, transform, renderable?)
```

测试：

```text
spawn_transform_only_entity_creates_transform_archetype
spawn_transform_renderable_entity_creates_renderable_archetype
scene_loader_creates_entity_and_transform
world_queries_alive_renderables
```

## A7：Query v1：Transform / Renderable

目标：

```text
alive_renderables 不再扫描多个 HashMap，而是扫描匹配 ArchetypeTable。
```

修改：

```text
rust/crates/engine_runtime/src/world.rs
```

保留对外行为：

```text
World::alive_renderables()
```

测试：

```text
query_transform_renderable_scans_matching_archetype_only
disabled_or_dead_entity_is_filtered
alive_renderables_are_sorted_by_source_id_or_runtime_id_stably
```

## A8：Add / Remove Component 迁移

目标：

```text
添加或删除 Component 时，Entity 能从一个 ArchetypeTable 迁移到另一个 ArchetypeTable。
```

第一版 API：

```text
World::add_renderable(runtime_id, renderable)
World::remove_renderable(runtime_id)
```

后续再泛化成：

```text
World::add_component(entity, component_type, value)
World::remove_component(entity, component_type)
```

测试：

```text
add_renderable_moves_entity_to_renderable_archetype
remove_renderable_moves_entity_to_transform_only_archetype
swap_remove_updates_moved_entity_location
old_runtime_id_still_points_to_same_entity_after_migration
```

## A9：Change / Dirty hook v1

目标：

```text
World 写入能记录最小 ComponentChange。
```

新增建议：

```text
rust/crates/engine_runtime/src/change.rs
```

数据结构：

```text
ComponentChange {
  runtime_entity_id,
  source_entity_id,
  component_type,
  field_path,
  frame,
  write_seq
}
```

测试：

```text
write_transform_records_component_change
add_renderable_records_render_facing_change
clear_frame_changes_clears_only_current_buffer
```

## A10：FrameHash / RuntimeTrace 接入保持

目标：

```text
Storage 改造后，FrameHash / RuntimeTrace / Golden Scenario 仍可运行。
```

修改：

```text
rust/crates/engine_runtime/src/frame_hash.rs
rust/crates/engine_runtime/src/runtime_trace.rs
rust/crates/engine_runtime/src/golden.rs
```

测试：

```text
golden_scenario_hash_is_stable_after_archetype_storage
runtime_trace_uses_source_entity_id_for_readability
```

## A11：旧 RenderSnapshot 兼容输出适配

目标：

```text
不删除旧 render_snapshot.rs。
让旧兼容输出从 Archetype Query 读取数据。
```

原因：

```text
当前编辑器 / Golden / 已有测试可能仍依赖旧兼容输出。
正式渲染主线后续转向 RenderFrameReport / RenderCommand。
```

测试：

```text
render_snapshot_compat_reads_archetype_renderables
render_snapshot_compat_output_is_stable
```

## 本阶段不做

```text
完整 System Scheduler。
完整 Deferred Command Buffer。
完整 RenderCommand / RenderSceneState。
完整 Project Component Dynamic storage。
完整 SparseSet 优化。
完整 Query Optimizer。
真实多线程执行。
真实 Renderer / RHI。
```

## 推荐执行顺序

```text
A0 当前 Runtime 回归门禁
A1 RuntimeEntityId / SourceEntityId
A2 ComponentTypeId / ComponentRegistry
A3 ArchetypeSignature / ArchetypeId
A4 ArchetypeTable
A5 EntitySlot / EntityLocation / SourceId 映射
A6 World spawn with component bundle
A7 Query v1
A8 Add / Remove Component 迁移
A9 Change / Dirty hook v1
A10 FrameHash / RuntimeTrace 接入保持
A11 RenderSnapshot 兼容输出适配
```

## 完成标准

```text
cargo test -p engine_runtime 通过。
现有 Runtime Package scene loader 测试通过。
alive_renderables 行为保持。
新增 Archetype / EntityLocation / ComponentRegistry / Change 测试通过。
旧 RenderSnapshot 只作为 compatibility output，文档和代码注释明确。
外层 API 不暴露 ArchetypeTable 给项目层和 AI。
```

## 2026-06-23 执行状态

本施工文档 A0-A11 已完成。

执行记录：

```text
阶段完成记录/2026-06-23-ECS-Storage-v1/00-总览.md
```

本轮实际完成：

```text
RuntimeEntityId / SourceEntityId
ComponentTypeId / ComponentRegistry
ArchetypeSignature / ArchetypeId
ArchetypeTable / ComponentColumn / ComponentValue
EntitySlot / EntityLocation / SourceId 映射
World spawn with component bundle
Transform / Renderable Query v1
Add / Remove Component 迁移
Dirty hook v1
FrameHash 稳定性验证
RenderSnapshot compatibility output 适配
```

验证结果：

```text
npm.cmd run test:rustruntime
  -> 通过，engine_runtime 31 passed

cargo +stable-x86_64-pc-windows-gnu test
  -> 通过，完整 Rust workspace 通过
```

注意：

```text
直接 cargo test -p engine_runtime 会因本机缺 MSVC link.exe 失败。
项目当前使用 stable-x86_64-pc-windows-gnu 作为 Windows Rust 测试门禁。
```
