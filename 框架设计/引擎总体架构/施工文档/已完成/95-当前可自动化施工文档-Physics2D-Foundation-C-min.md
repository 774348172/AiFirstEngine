# 当前可自动化施工文档：Physics2D Foundation C-min

状态：规则已确认，可以直接施工。

本文档只定义“怎么做”和“怎么验收”。长期边界以对应框架文档为准。

## 1. 规则来源

```text
../../95-Physics2D-Foundation-C-min方案.md
../../93-复杂打飞机验证所需引擎侧缺失能力清单.md
../../94-Engine-Gameplay-Foundation-C-min边界方案.md
../../16-ECS写入与项目规则边界.md
../../17-Runtime-FrameLoop.md
../../52-ECS-Storage-v1方案.md
```

## 2. 本阶段目标

建立 `engine_runtime` 的第一版 2D 物理底座。

本阶段采用完整 Physics2D 系统的长期结构：

```text
ECS World = 游戏对象与组件真相
Physics2DWorld = 引擎内部物理计算世界
Physics2DBridge = ECS 与 Physics2DWorld 的同步层
```

但第一版只实现：

```text
Collider2D typed component
PhysicsLayer / Mask
AABB shape
Circle shape
Physics2DWorld headless data structure
Physics2DBridge sync from ECS
overlap_aabb
overlap_circle
CollisionPairReport
Physics2DTrace / RuntimeTrace 接入
headless tests
```

本阶段不做：

```text
Rigidbody2D dynamic solver
gravity
friction
restitution
sleep
continuous collision detection
OnCollisionEnter 自动回调
Trigger 自动事件
复杂 polygon shape
物理材质求解
真实 debug draw UI
```

硬规则：

```text
引擎物理层不理解项目玩法语义。
禁止新增 Bullet / Enemy / Damage / Health / Score 等项目专用 API。
测试也必须使用 source entity / target entity / moving collider / static collider 等中性命名。
```

## 3. 当前代码状态

当前已有：

```text
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/archetype.rs
rust/crates/engine_runtime/src/components.rs
rust/crates/engine_runtime/src/component_value.rs
rust/crates/engine_runtime/src/query.rs
rust/crates/engine_runtime/src/world_api.rs
rust/crates/engine_runtime/src/logic_executor.rs
rust/crates/engine_runtime/src/project_logic.rs
rust/crates/engine_runtime/src/runtime_trace.rs
rust/crates/engine_runtime/src/frame_loop.rs
```

已有能力：

```text
World / ArchetypeTable
ComponentTypeId
Transform / Renderable / Hierarchy / EntityMeta
RuntimeValue / ComponentValue
QuerySpec
WorldReadApi / WorldWriteApi
LogicContext
GameplayCommandBuffer
GameplayTraceRecord
RuntimeTrace
FrameLoop
```

当前缺口：

```text
没有 Collider2D typed component
没有 PhysicsLayer / Mask
没有 Physics2DWorld
没有 Physics2DBridge
没有 overlap_aabb / overlap_circle
没有 CollisionPairReport
没有 Physics2D trace
没有 Physics2D 与 FrameLoop 的最小接入点
```

## 4. 施工原则

```text
一项一测。
只改 engine_runtime 通用底座。
不新增项目玩法语义类型。
不引入完整物理引擎依赖。
不破坏现有 Transform / Renderable / RenderExtract 流程。
不要求真实窗口。
全部使用 headless 单元测试验证。
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

## 6. A1：Collider2D / Shape2D / LayerMask 数据结构

目标：

```text
新增 Physics2D 第一版可编辑、可序列化、可进入 ECS 的 typed component。
```

建议新增 / 修改：

```text
rust/crates/engine_runtime/src/physics2d.rs
rust/crates/engine_runtime/src/archetype.rs
rust/crates/engine_runtime/src/components.rs
rust/crates/engine_runtime/src/lib.rs
```

数据结构：

```text
PhysicsLayer(u32)
PhysicsMask(u32)

Collider2D:
  shape: Shape2D
  offset: Vec2
  layer: PhysicsLayer
  mask: PhysicsMask
  enabled: bool
  is_sensor: bool

Shape2D:
  Aabb { half_extents: Vec2 }
  Circle { radius: f32 }
```

规则：

```text
Collider2D 是通用引擎组件。
is_sensor 只作为数据保留，第一版不触发自动事件。
Layer / Mask 只做通用过滤，不表达 Enemy / Player 等项目语义。
```

测试：

```text
collider2d_aabb_is_component_value
collider2d_circle_is_component_value
physics_layer_mask_filters_bitwise
collider2d_component_type_is_registered
```

完成标准：

```text
World 可以存储带 Transform + Collider2D 的 Entity。
ComponentValue 可以表达 Collider2D。
不破坏已有 Transform / Renderable 测试。
```

## 7. A2：Physics2DWorld v1

目标：

```text
建立内部 Physics2DWorld headless 数据结构。
```

建议新增：

```text
rust/crates/engine_runtime/src/physics2d.rs
```

数据结构：

```text
Physics2DWorld:
  colliders: BTreeMap<EntityId, Physics2DColliderProxy>

Physics2DColliderProxy:
  entity_id
  world_position: Vec2
  shape
  layer
  mask
  enabled
  is_sensor
```

API：

```text
insert_or_update_collider(entity_id, proxy)
remove_collider(entity_id)
clear()
collider_count()
```

规则：

```text
Physics2DWorld 是内部计算世界。
项目规则不能直接写 Physics2DWorld。
Physics2DWorld 不持有 World 借用。
```

测试：

```text
physics2d_world_inserts_updates_and_removes_collider
physics2d_world_ignores_disabled_collider_in_queries
physics2d_world_keeps_entity_id_order_stable
```

完成标准：

```text
Physics2DWorld 能独立管理 collider proxy。
结果顺序稳定，便于 AI / Trace / Golden Scenario 对比。
```

## 8. A3：Physics2DBridge Sync From ECS

目标：

```text
从 ECS World 同步 Transform + Collider2D 到 Physics2DWorld。
```

建议新增 / 修改：

```text
rust/crates/engine_runtime/src/physics2d.rs
rust/crates/engine_runtime/src/world.rs
```

API：

```text
Physics2DBridge::sync_from_world(world, physics_world) -> Physics2DSyncReport
```

报告字段：

```text
scanned_entities
synced_colliders
removed_colliders
disabled_colliders
diagnostics
```

规则：

```text
ECS World 是真相。
Physics2DWorld 每次由 ECS 同步得到。
第一版可采用 full rebuild 或简单同步，不提前做复杂增量优化。
```

测试：

```text
physics2d_bridge_syncs_transform_and_collider_from_world
physics2d_bridge_updates_proxy_after_transform_change
physics2d_bridge_removes_proxy_when_collider_removed_or_entity_despawned
physics2d_bridge_reports_missing_transform_for_collider
```

完成标准：

```text
ECS 中的 Transform + Collider2D 能进入 Physics2DWorld。
Transform 改动后同步结果更新。
缺 Transform 时产生 diagnostic，不 panic。
```

## 9. A4：Overlap Query v1

目标：

```text
提供第一版 overlap_aabb / overlap_circle 查询。
```

API：

```text
Physics2DWorld::overlap_aabb(query: OverlapAabb2D) -> Vec<Physics2DHit>
Physics2DWorld::overlap_circle(query: OverlapCircle2D) -> Vec<Physics2DHit>
```

数据结构：

```text
OverlapAabb2D:
  center: Vec2
  half_extents: Vec2
  layer
  mask
  include_sensors: bool
  limit: Option<usize>

OverlapCircle2D:
  center: Vec2
  radius: f32
  layer
  mask
  include_sensors: bool
  limit: Option<usize>

Physics2DHit:
  entity_id
  shape
  distance_hint
```

规则：

```text
查询只返回 EntityId 和通用命中数据。
不判断敌我，不扣血，不销毁。
结果按 EntityId 稳定排序。
Layer / Mask 双向过滤第一版必须明确实现。
```

测试：

```text
overlap_aabb_hits_intersecting_aabb
overlap_aabb_filters_by_layer_mask
overlap_circle_hits_circle_and_aabb
overlap_query_excludes_sensor_when_requested
overlap_query_limit_applies_after_stable_order
```

完成标准：

```text
项目规则后续可通过中性 overlap query 拿到候选 Entity。
查询行为稳定可测试。
```

## 10. A5：CollisionPairReport v1

目标：

```text
构建当前 Physics2DWorld 中的碰撞对报告。
```

API：

```text
Physics2DWorld::build_collision_pairs() -> CollisionPairReport
```

数据结构：

```text
CollisionPair:
  entity_a
  entity_b
  shape_a
  shape_b
  is_sensor_pair

CollisionPairReport:
  pairs
  collider_count
  tested_pair_count
  filtered_pair_count
```

规则：

```text
Pair 只表示几何重叠和过滤后成立。
Pair 不表示业务事件。
Pair 排序稳定：先 entity_a，再 entity_b。
entity_a < entity_b。
```

测试：

```text
collision_pair_report_detects_two_overlapping_colliders
collision_pair_report_filters_non_matching_layers
collision_pair_report_sorts_pairs_stably
collision_pair_report_marks_sensor_pair_without_auto_event
```

完成标准：

```text
Frame / Golden Scenario 可以读取 CollisionPairReport。
项目规则可自行解释 Pair。
```

## 11. A6：Physics2DTrace / RuntimeTrace 接入

目标：

```text
Trace 记录 Physics2D 同步、查询和碰撞对报告，便于 AI / 人排查。
```

建议新增 / 修改：

```text
rust/crates/engine_runtime/src/physics2d.rs
rust/crates/engine_runtime/src/runtime_trace.rs
```

最小字段：

```text
frame_index
phase
operation
entity_id
query_kind
shape
layer
mask
hit_count
pair_count
result
error_code
```

operation：

```text
sync_from_world
overlap_aabb
overlap_circle
build_collision_pairs
```

禁止字段：

```text
bullet_id
enemy_id
damage_amount
killer_id
score_delta
```

测试：

```text
physics2d_trace_records_sync_query_and_pair_report
physics2d_trace_keeps_project_semantics_out
physics2d_trace_records_missing_transform_diagnostic
```

完成标准：

```text
Trace 能说明哪一帧同步了多少 Collider，哪次查询命中了多少 Entity，碰撞对数量是多少。
Trace 字段保持中性。
```

## 12. A7：FrameLoop / ProjectLogicRunner 最小接入

目标：

```text
让 Physics2D C-min 能在固定帧流程中稳定运行。
```

第一版接入点：

```text
ProjectLogicRunner 执行规则
  -> Apply Structural Command
  -> Physics2DBridge sync_from_world
  -> Physics2DWorld build_collision_pairs
  -> RuntimeTrace append physics2d records
  -> RenderExtract
```

注意：

```text
第一版不做 Physics2D WriteBack，因为没有动态刚体求解。
第一版不让 Physics2D 修改 Transform。
第一版只产生查询 / pair report / trace。
```

建议新增 / 修改：

```text
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/project_logic.rs
rust/crates/engine_runtime/src/runtime_trace.rs
```

测试场景：

```text
创建 source entity：Transform + Collider2D
创建 target entity：Transform + Collider2D
Project rule 移动 source entity 的 Transform
FrameLoop 执行
Physics2DBridge 同步
CollisionPairReport 出现 source-target pair
RenderExtract 仍正常执行
Trace 记录 physics2d sync / pair report
```

测试：

```text
frame_loop_runs_physics2d_after_project_logic_before_render_extract
frame_loop_physics2d_pair_report_reflects_transform_write
frame_loop_physics2d_trace_is_recorded
frame_loop_physics2d_does_not_write_back_transform_in_c_min
```

完成标准：

```text
Physics2D C-min 已进入主 FrameLoop。
Project Rule 写 Transform 后，同帧 Physics2D 查询 / Pair 能看到最新位置。
RenderExtract 仍看到 ECS Transform 真相。
```

## 13. A8：中性端到端测试

目标：

```text
用非玩法命名验证完整闭环。
```

测试场景：

```text
source entity 初始在 x=0
target entity 在 x=2
source collider half_extents = 0.5
target collider half_extents = 0.5
第一帧：没有 pair
规则把 source local_position.x 写到 1.5
第二帧：产生 source-target pair
Trace 可解释同步和 pair
```

测试：

```text
physics2d_foundation_c_min_end_to_end_overlap_after_transform_write
physics2d_foundation_c_min_query_api_supports_project_rule_usage
physics2d_foundation_c_min_no_project_semantic_terms_in_trace
```

完成标准：

```text
不使用 Bullet / Enemy / Damage / Health / Score。
能证明后续打飞机项目可用项目规则解释碰撞结果。
```

## 14. 全局验收标准

必须通过：

```powershell
cd G:\gameEngin\rust
cargo fmt --check
cargo test -p engine_runtime
cargo test --workspace
```

文档更新：

```text
施工完成后，将本文档移动到 施工文档/已完成
新增 阶段完成记录/2026-xx-xx-Physics2D-Foundation-C-min/00-总览.md
更新 00-文档地图.md
更新 49-当前状态与下一步入口.md
更新 87-当前实现状态总览-2026-06-28.md 或后续最新状态文档
```

## 15. 禁止事项

本阶段禁止：

```text
新增 Bullet / Enemy / Damage / Health / Score 项目语义 API
新增完整 Rigidbody2D solver
新增重力 / 摩擦 / 反弹求解
新增 Trigger 自动事件
新增 OnCollisionEnter 自动回调
新增复杂 polygon shape
让 Physics2DWorld 成为游戏对象真相
让项目规则直接修改 Physics2DWorld 内部 broadphase / proxy
让 Physics2D 在 C-min 阶段写回 Transform
修改真实窗口 UI
引入大型外部物理引擎依赖
```

如果施工过程中发现必须实现刚体求解、Trigger 自动事件或项目玩法语义 API，停止施工，回到 95 方案重新讨论。
