# 当前可自动化施工文档：Engine Gameplay Foundation C-min

状态：已确认，可以直接施工。

本文档只定义“怎么做”和“怎么验收”。长期边界以对应框架文档为准。

## 1. 规则来源

```text
../../94-Engine-Gameplay-Foundation-C-min边界方案.md
../../93-复杂打飞机验证所需引擎侧缺失能力清单.md
../../16-ECS写入与项目规则边界.md
../../31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
../../52-ECS-Storage-v1方案.md
../../17-Runtime-FrameLoop.md
```

## 2. 本阶段目标

把 `engine_runtime` 现有 `WorldReadApi / WorldWriteApi / LogicContext` 从“主要只能读写 Transform”扩展为通用 ECS 访问底座。

本阶段只做：

```text
Query
Read
Write
Structural Command
Trace
```

本阶段不做：

```text
Damage / Health / Bullet / Enemy / Score 等项目玩法 API
2D Collision
Prefab Instantiate
Runtime UI
Input Mapping Asset
事件系统
自动业务调度器
```

硬规则：

```text
引擎只提供底座能力，不为特定项目增加规则。
所有新增 API 必须使用 entity / component / field / query / command / trace / value 等中性词。
```

## 3. 当前代码状态

当前已有：

```text
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/world_api.rs
rust/crates/engine_runtime/src/archetype.rs
rust/crates/engine_runtime/src/components.rs
rust/crates/engine_runtime/src/logic_executor.rs
rust/crates/engine_runtime/src/project_logic.rs
rust/crates/engine_runtime/src/runtime_trace.rs
```

已有能力：

```text
World / ArchetypeTable
ComponentTypeId
Transform / Renderable / Hierarchy / EntityMeta
Dynamic Component debug_value
WorldReadApi / WorldWriteApi
LogicContext
ProjectLogicRunner
LogicResult / LogicWrite
RuntimeTrace
```

当前主要缺口：

```text
没有 QuerySpec
没有通用 ComponentValue 数据表达
Dynamic Component 仍是 debug string
WorldReadApi / WorldWriteApi 未暴露通用 read/write
没有 GameplayCommandBuffer
结构变化未通过 LogicContext 统一收集并 apply
Trace 还不是通用 GameplayTraceRecord
```

## 4. 施工原则

```text
一项一测。
只改 engine_runtime 通用底座。
不新增项目语义类型。
不引入 bevy_ecs / hecs 作为正式依赖。
不破坏现有 Transform / Renderable / RenderExtract 流程。
不要求真实窗口。
全部用 headless 单元测试验证。
```

每个阶段完成后至少运行：

```powershell
cd G:\gameEngin\rust
cargo test -p engine_runtime <相关测试名>
```

全部阶段完成后运行：

```powershell
cd G:\gameEngin\rust
cargo fmt --check
cargo test -p engine_runtime
cargo test --workspace
```

## 5. A0：回归门禁

目标：

```text
确认施工前 engine_runtime 当前测试状态。
```

执行：

```powershell
cd G:\gameEngin\rust
cargo test -p engine_runtime
```

完成标准：

```text
engine_runtime 当前测试通过，或明确记录已有失败。
本阶段不修改代码。
```

## 6. A1：ComponentValue v1

目标：

```text
把 Dynamic Component 从 debug string 升级为通用 ComponentValue 数据表达。
```

建议新增 / 修改：

```text
rust/crates/engine_runtime/src/component_value.rs
rust/crates/engine_runtime/src/archetype.rs
rust/crates/engine_runtime/src/lib.rs
```

数据结构：

```text
RuntimeValue:
  Null
  Bool(bool)
  I64(i64)
  F64(f64)
  String(String)
  Vec2 { x, y }
  Vec3 { x, y, z }
  Color { r, g, b, a }
  EntityRef(EntityId)
  AssetRef(String)
  Object(BTreeMap<String, RuntimeValue>)
  Array(Vec<RuntimeValue>)
```

`archetype::ComponentValue` 保留内置 typed 分支：

```text
EntityMeta
Hierarchy
Transform
Renderable
Dynamic { component_type, value: RuntimeValue }
```

不允许新增：

```text
DamageValue
HealthValue
SkillValue
InventoryValue
```

测试：

```text
runtime_value_object_orders_fields_deterministically
runtime_value_supports_vec3_and_asset_ref
dynamic_component_stores_runtime_value
```

完成标准：

```text
Dynamic Component 可以保存 RuntimeValue。
原有 Transform / Renderable 测试不破坏。
```

## 7. A2：QuerySpec v1

目标：

```text
提供最小 ECS Query 能力：按 Component 组合选择 Entity。
```

建议新增：

```text
rust/crates/engine_runtime/src/query.rs
```

数据结构：

```text
QuerySpec:
  all: Vec<ComponentTypeId>
  none: Vec<ComponentTypeId>
  include_disabled: bool
  limit: Option<usize>
  stable_order: StableOrder

StableOrder:
  EntityId
```

World API：

```text
World::query_entities(&QuerySpec) -> Vec<EntityId>
WorldReadApi::query(&QuerySpec) -> Vec<EntityId>
WorldWriteApi::query(&QuerySpec) -> Vec<EntityId>
LogicContext::query(QuerySpec) -> Vec<EntityId>
```

第一版不支持：

```text
any
字段过滤
距离过滤
最近目标
排序表达式
业务标签语义
```

测试：

```text
query_spec_all_returns_entities_with_all_components
query_spec_none_excludes_entities
query_spec_excludes_disabled_by_default
query_spec_limit_is_applied_after_stable_order
logic_context_query_returns_stable_entity_ids
```

完成标准：

```text
可以查询拥有 Transform + project.custom_component 的 Entity。
结果按 EntityId 稳定排序。
```

## 8. A3：通用 Read API

目标：

```text
让项目规则可以读取任意 Component，而不是只读 Transform。
```

建议修改：

```text
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/world_api.rs
rust/crates/engine_runtime/src/logic_executor.rs
```

API：

```text
World::component_value(entity, component_type) -> Option<ComponentValue>
WorldReadApi::read_component(entity, component_type) -> Result<ComponentValue, WorldApiError>
WorldWriteApi::read_component(entity, component_type) -> Result<ComponentValue, WorldApiError>
LogicContext::read_component(entity, component_type) -> Result<ComponentValue, WorldApiError>
```

注意：

```text
返回值可以 clone。
第一版优先简单可靠，不做借用型高级 Query。
```

测试：

```text
world_read_api_reads_dynamic_component
world_read_api_reports_missing_component
logic_context_reads_component_value
```

完成标准：

```text
ProjectLogicRunner 中的 RustAotRule 能读取 project.* Component。
```

## 9. A4：FieldPath v1 与通用 Write API

目标：

```text
让项目规则可以写任意 Component 字段或整个 Component。
```

建议新增 / 修改：

```text
rust/crates/engine_runtime/src/field_path.rs
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/world_api.rs
rust/crates/engine_runtime/src/logic_executor.rs
```

FieldPath v1：

```text
支持：
  field
  field.sub_field
  local_position.x
  local_position.y
  local_position.z

不支持：
  inventory[3].count
  buffs[*].duration
  items.where(id=xxx)
```

API：

```text
WorldWriteApi::write_component(entity, component_type, ComponentValue)
WorldWriteApi::write_component_field(entity, component_type, FieldPath, RuntimeValue)
LogicContext::write_component(...)
LogicContext::write_component_field(...)
```

Dirty 规则：

```text
Transform 字段写入 -> DirtyType::Transform
Renderable 字段写入 -> DirtyType::RenderState
Dynamic Component 写入 -> DirtyType::DynamicData
```

测试：

```text
field_path_accepts_simple_dot_path
field_path_rejects_array_index_path
write_component_field_updates_dynamic_object_field
write_component_field_updates_transform_local_position_x
write_component_replaces_dynamic_component
write_component_field_records_before_after
```

完成标准：

```text
普通 Component 字段写入立即生效。
写入产生 LogicWrite / WorldWriteRecord。
不通过 CommandBuffer 写普通字段。
```

## 10. A5：GameplayCommandBuffer v1

目标：

```text
结构变化统一走 CommandBuffer，并在 ProjectLogicRunner 本轮规则执行结束后、RenderExtract 前 apply。
```

建议新增 / 修改：

```text
rust/crates/engine_runtime/src/gameplay_command.rs
rust/crates/engine_runtime/src/logic_executor.rs
rust/crates/engine_runtime/src/project_logic.rs
rust/crates/engine_runtime/src/world.rs
```

命令：

```text
SpawnEntity {
  entity_id,
  name,
  kind,
  enabled,
  parent_id,
  components
}

DespawnEntity {
  entity_id
}

AddComponent {
  entity_id,
  component_type,
  value
}

RemoveComponent {
  entity_id,
  component_type
}

SetParent {
  entity_id,
  parent_id,
  keep_world_transform
}
```

不允许：

```text
SpawnBullet
ApplyDamage
FindEnemy
AddScore
ApplyBuff
```

LogicContext API：

```text
ctx.commands().spawn_entity(...)
ctx.commands().despawn_entity(...)
ctx.commands().add_component(...)
ctx.commands().remove_component(...)
ctx.commands().set_parent(...)
```

Apply 点：

```text
ProjectLogicRunner.run_phase:
  execute all enabled rule calls
  collect command buffers
  apply commands
  append command apply results to trace/report
```

测试：

```text
command_buffer_spawn_entity_applies_after_rule_execution
command_buffer_despawn_entity_applies_after_rule_execution
command_buffer_add_remove_component_migrates_archetype
command_buffer_set_parent_updates_hierarchy
command_buffer_does_not_apply_before_rule_returns
```

完成标准：

```text
结构变化不在规则中直接穿插修改 ECS。
本轮规则执行结束后统一 apply。
RenderExtract 前能看到 apply 后结果。
```

## 11. A6：GameplayTraceRecord v1

目标：

```text
Trace 记录通用 ECS 访问和结构变化，供 AI / 人排查问题。
```

建议新增 / 修改：

```text
rust/crates/engine_runtime/src/gameplay_trace.rs
rust/crates/engine_runtime/src/runtime_trace.rs
rust/crates/engine_runtime/src/logic_executor.rs
rust/crates/engine_runtime/src/project_logic.rs
```

最小字段：

```text
frame_index
phase
rule_id
operation
entity_id
component_type
field_path
before
after
command_id
source
result
error_code
```

operation：

```text
query
read
write
command_enqueue
command_apply
```

不允许把业务字段加入 Engine Trace：

```text
final_damage
killer_id
critical_hit
gold_reward
combo_count
```

测试：

```text
gameplay_trace_records_query_read_write
gameplay_trace_records_command_enqueue_and_apply
gameplay_trace_records_failed_missing_component
gameplay_trace_keeps_project_semantics_out
```

完成标准：

```text
可以从 Trace 看出哪条 rule 在哪一帧读写了哪个 Entity / Component / Field。
Trace 字段保持中性，不出现项目玩法语义。
```

## 12. A7：ProjectLogicRunner 集成测试

目标：

```text
验证 Query / Read / Write / Command / Trace 可以通过 ProjectLogicRunner 完整跑通。
```

建议测试场景：

```text
创建 entity-source
添加 engine.transform
添加 project.marker dynamic component
Rule 1:
  query all [engine.transform, project.marker]
  read project.marker
  write project.marker.count += 1
  commands.spawn_entity(entity-created, components: project.created_marker)
ProjectLogicRunner 执行 Update
CommandBuffer apply
Trace 记录 query / read / write / command_enqueue / command_apply
```

测试：

```text
project_logic_runner_supports_generic_query_read_write_command_trace
project_logic_runner_keeps_gameplay_foundation_project_semantic_free
```

完成标准：

```text
不使用 Bullet / Enemy / Health / Damage / Score 等项目语义。
只用 project.marker / project.created_marker 这类中性测试组件。
```

## 13. 全局验收标准

必须通过：

```powershell
cd G:\gameEngin\rust
cargo fmt --check
cargo test -p engine_runtime
cargo test --workspace
```

文档更新：

```text
施工完成后，将本文档移动到 施工文档/已完成/
新增 阶段完成记录/2026-xx-xx-EngineGameplayFoundation-C-min/00-总览.md
更新 00-文档地图.md
更新 49-当前状态与下一步入口.md
更新 87-当前实现状态总览-2026-06-28.md 或后续最新状态文档
```

## 14. 禁止事项

本阶段禁止：

```text
新增任何玩法专用 API
新增 Bullet / Enemy / Health / Damage / Score 内置组件
把普通字段写入塞进 CommandBuffer
实现 2D Collision
实现 Prefab Instantiate
实现事件系统
实现自动业务调度器
修改编辑器 UI
引入真实窗口依赖
```

如果施工过程中发现必须新增项目语义 API，停止施工并回到 94 文档重新讨论。
