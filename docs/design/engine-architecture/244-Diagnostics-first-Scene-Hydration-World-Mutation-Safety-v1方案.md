# 244-Diagnostics-first Scene Hydration / World Mutation Safety v1 方案

> 状态：已完成施工、整体回归、阶段完成记录与归档；CQ-06 已关闭。  
> 建立日期：2026-07-11。  
> 选题来源：`240-5.6审查剩余问题讨论与施工优先级.md` Priority 4 / CQ-06。  
> 审查输入：`审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md`、`01-2026-07-11-新增功能增量代码质量审查报告.md`。  
> 用户确认：Runtime 采用方案 B-min+；编辑器事务方案 C deferred，后续单独讨论。  
> 目标：公开 Scene/Prefab hydration、World mutation 和 GameplayCommand 输入错误必须返回结构化 diagnostics，不得 unwind；任何可预见输入错误必须在修改 World 前被拒绝，同时不把编辑器事务系统、Undo、事务日志或常驻 Report 带入 Windows Player。

## 1. 这个系统是干什么的

直白地说，它是 Runtime Scene/Prefab 进入 ECS World 前的“防崩溃和防半成品”边界。

```text
错误 RuntimeScene / RuntimePrefab / GameplayCommand
  -> 完整预检
  -> 返回稳定 diagnostic
  -> World 不产生可观察的半成品
  -> Editor Play / exported Player 不 panic
```

典型错误：

```text
duplicate entity id
missing parent
self parent / hierarchy cycle
missing required Transform
invalid component payload
missing EntityRef target
duplicate spawn into an existing World
missing entity/component
stale RuntimeEntityId generation
```

用户和 AI 应看到：

```text
operation: scene_hydration
stage: ValidateHierarchy
code: world.parent.missing
source: scene.entity.enemy-02.parent_id
message: parent entity does not exist: enemy-root
worldChanged: false
nextAction: Fix parent_id and rebuild the RuntimePackage.
```

它不新增玩法能力，不改变 RuntimePackage 真相，也不是新的事务、脚本或架构路由层。

## 2. 正式决策

正式采用：

```text
方案 B-min+：Fallible World Mutation Primitives
             + Shared Hydration Preflight
             + Prepared Commit
             + Existing Diagnostics/Report Adapters
```

Runtime 正式链：

```text
RuntimeScene / RuntimePrefab / GameplayCommand
  -> validate public input
  -> prepare typed mutation data
  -> try_* World mutation primitives
  -> activate instance only after success
  -> compact result / diagnostics
```

明确不采用本轮讨论过的 Runtime 方案 C：

```text
不新增 WorldMutationTransaction runtime layer。
不在 Runtime 保存通用 rollback journal。
不为每个结构操作记录 before/after。
不 clone 整个 World 后再 swap。
不把 Undo/Redo 或 Editor transaction 带入 exported Player。
不让普通 Transform/physics/animation/value update 经过事务。
```

编辑器事务方案 C 后续单独讨论。未来只能深化或收敛现有：

```text
SceneEditTransaction
CommandTransaction
AuiTransaction
ProjectPatch snapshot/rollback
```

244 不建立第五套 Editor transaction，也不修改上述 Editor 所有权。

## 3. 为什么现在必须做

5.6 审查指出：本项目宣称 diagnostics-first，但公开输入仍能触发 panic。

### 3.1 旧公开 Scene loader

`engine_runtime/src/scene_loader.rs::load_scene_into_world` 返回：

```rust
RuntimeLoadResult<World>
```

但它没有先验证 duplicate entity id，直接调用 `World::spawn_entity`。重复 ID 会进入 `World::allocate_runtime_id` 的 `panic!`，调用者拿不到 `RuntimeLoadResult`。

### 3.2 World public mutation

当前以下路径对缺失或重复对象仍可 panic：

```text
World::spawn_entity / spawn_with_components
World::set_parent
World::insert_transform
World::insert_renderable
World::insert_sprite_renderer2d
World::insert_dynamic_component / insert_component_value
内部 source id -> runtime id / slot 解析
```

`WorldWriteApi` 部分方法虽然返回 `Result`，内部仍调用上述 panic API，因此返回类型和真实行为不一致。

### 3.3 RuntimeInstanceLoader 会边验证边修改

当前 `RuntimeInstanceLoader` 已有 `InstanceDiagnostic`，但仍存在：

```text
allocate_entities 遍历时边 spawn 边检查 duplicate。
全部 skeleton 创建后才发现某 Entity 缺 Transform。
missing parent 没有完整图预检。
invalid Collider2D 会添加 error，但 attach_components 继续返回 true。
missing EntityRef 会添加 error，但仍可能写入未正确 remap 的动态值。
失败路径释放 asset handles，但不统一移除已经创建的 World Entity。
有 error 时仍可能构造或注册 Active instance。
```

这会产生“报告失败，但传入的 existing World 已经被修改”的不一致。

### 3.4 GameplayCommand 可能无法形成失败记录

`SpawnEntity`、`AddComponent`、`SetParent` 直接调用 panic 型 World API。公开错误可能在 `GameplayCommandApplyRecord` 生成前 unwind。

## 4. 5.6 审查结论分类

### 4.1 必须修改

```text
CQ-06：公开 scene loader 不得因 duplicate id panic。
CQ-06：公开 World structural/component mutation 提供 fallible interface。
CQ-06：missing entity/parent/component/stale handle 返回稳定错误。
CQ-06：RuntimeInstanceLoader 与 HydrationProjection 在修改前完成完整输入预检。
CQ-06：GameplayCommand / WorldWriteApi 不得绕过 fallible boundary。
CQ-06：公开失败路径返回 diagnostics，不留下可观察半成品。
```

### 4.2 施工约束

```text
duplicate id / existing id collision / missing parent / self parent / cycle 必须 fail closed。
missing Transform / invalid typed component / missing EntityRef 必须在 commit 前拒绝。
stale RuntimeEntityId 必须按 index + generation 拒绝。
所有公开否定案例必须证明 no unwind。
Hydration 失败不得注册 Active instance。
asset handle 在失败路径必须释放。
Runtime 默认不生成 JSON、长 trace 或完整 source->runtime debug map。
```

### 4.3 已有能力，必须复用

```text
RuntimeLoadResult / RuntimeDiagnostics。
InstanceDiagnostic / RuntimeInstantiateReport。
HydrationProjection / RuntimeSceneHydrator。
RuntimeInstanceLoader 的 SceneInstance / PrefabInstance ownership。
RuntimeEntityId index + generation。
GameplayCommandBuffer 的 deferred structural command boundary。
ProjectionReport 的 adapter summary。
```

### 4.4 不适用或 deferred

```text
Editor transaction C、Undo/Redo、authoring history。
ProjectPatch file transaction / rollback。
通用 database transaction、MVCC、World snapshot。
Runtime hot reload、network replication rollback、prediction rollback。
CQ-07/CQ-08 hygiene/CI/toolchain。
INC-02 LLM request lifecycle；继续由 243 独立施工。
```

## 5. 成熟引擎与源码参考

### 5.1 Unity：公开入口预检，失败使用异常/native error

本机源码：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/
  Runtime/Export/Scripting/UnityEngineObject.bindings.cs
  Runtime/Export/SceneManager/SceneManager.cs
  Runtime/Export/SceneManager/SceneManager.bindings.cs
  Runtime/Transform/ScriptBindings/Transform.bindings.cs
```

关键实现：

```text
Object.Instantiate
  -> CheckNullArgument
  -> reject destroying parent
  -> native clone/instantiate
  -> null clone becomes UnityException

SceneManager.LoadScene
  -> LoadSceneAsyncNameIndexInternal
  -> native method marked ThrowsException

Transform.SetParent
  -> native SetParent
```

可学习：公开入口先检查对象和 parent 生命周期，错误不能静默成功。  
不照搬：native 黑盒、异常和 Console 文本不适合作为本项目 AI diagnostics 真相。

### 5.2 Unreal：SpawnActor 先验证，Attach 返回 bool

本机源码：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/
  Engine/Source/Runtime/Engine/Private/LevelActor.cpp
  Engine/Source/Runtime/Engine/Private/Components/SceneComponent.cpp
  Engine/Source/Runtime/Engine/Classes/Engine/World.h
```

关键实现：

```text
UWorld::SpawnActor
  -> validate class/deprecated/abstract/context/world teardown
  -> validate transform NaN/name collision/collision policy
  -> only then NewObject + PostSpawnInitialize
  -> failure returns nullptr and logs reason

USceneComponent::AttachToComponent
  -> reject attach to self
  -> reject root-to-sibling invalid attachment
  -> reject hierarchy cycle
  -> return false without applying invalid attachment

SpawnActorDeferred / FinishSpawning
  -> separate allocation/construction from final activation
```

可学习：预检、延迟激活、层级环检查、失败不假装成功。  
不照搬：UObject/Actor lifecycle、Construction Script、Fatal name mode 和日志即合同。

### 5.3 Godot：PackedScene 验证父节点和类型

本机源码：

```text
<GODOT_SOURCE>/godot-master/godot-master/
  scene/resources/packed_scene.cpp
  scene/main/node.cpp
```

`SceneState::instantiate` 检查：

```text
empty scene
root parent rule
child parent index/path
missing nested scene/type
property/name/group index
deferred NodePath target
```

缺失类型时可创建 MissingNode/placeholder，部分编辑器场景会恢复 parent 或重命名冲突节点。

可学习：实例化必须验证整棵树并清理 stray node。  
不照搬：Runtime 不自动把 missing parent 挂到 root，不用 placeholder 掩盖无效发布数据；ERR_FAIL/WARN 文本不是稳定结构化合同。

### 5.4 Bevy：Result + Entity generation + Scene entity map

本机源码：

```text
<BEVY_SOURCE>/bevy-main/bevy-main/
  crates/bevy_world_serialization/src/dynamic_world.rs
  crates/bevy_ecs/src/world/mod.rs
  crates/bevy_ecs/src/system/commands/mod.rs
```

关键实现：

```text
DynamicWorld::write_to_world_with
  -> preallocate all destination Entity
  -> EntityHashMap source -> destination
  -> apply_or_insert_mapped components
  -> insert resources after Entity mapping exists
  -> return WorldInstanceSpawnError

World
  -> get_entity_mut returns EntityMutableFetchError
  -> try_insert_batch / try_despawn return Result

Commands
  -> structural changes deferred
  -> fallible command can use error handler
```

可学习：generation handle、fallible mutation、先建立映射再 remap 引用。  
不照搬：Bevy DynamicWorld 在组件类型验证失败前已创建 empty Entity，仍可能留下部分目标 World；本项目必须把可预见错误前移到 prepare 阶段。

## 6. 候选方案与选择

### 6.1 方案 A：只修 legacy scene_loader

优点：范围最小。  
缺点：WorldWriteApi、GameplayCommand、Prefab 和其它公开 mutation 仍能 panic，不能关闭 CQ-06。  
结论：不采用。

### 6.2 方案 B-min+：Fallible primitive + preflight/commit

```text
World public try_* API
+ shared RuntimeEntity hydration preflight
+ typed prepared data
+ activate only after successful commit
+ existing diagnostic adapters
```

优点：

```text
输入错误不 unwind。
Scene/Prefab 不产生半完成 Active instance。
AI 得到稳定 code/stage/source/next action。
Runtime 无事务日志、World clone 和 steady-frame overhead。
legacy loader 与正式 HydrationProjection 不再维护两套验证语义。
```

结论：正式采用。

### 6.3 方案 C：Runtime structural transaction

它会把多项 mutation 组成通用 transaction，保留预检、journal、rollback 和 transaction report。

优势：跨多命令原子性更强。  
问题：进入 Runtime 后会增加 plan allocation、双遍历、journal、rollback 和 report 成本；也会形成新运行时层。  
结论：Runtime 不采用。编辑器事务 C deferred，后续基于现有 Editor transaction 单独讨论。

## 7. Runtime 错误合同

World mutation 只保留一个错误语义真相。可以深化现有 `WorldApiError` 或由 World 拥有 `WorldMutationError`，但不得长期并存两套 code/message 体系。

最低字段：

```rust
pub struct WorldMutationError {
    pub code: &'static str,
    pub operation: &'static str,
    pub source_entity_id: Option<EntityId>,
    pub runtime_entity_id: Option<RuntimeEntityId>,
    pub parent_entity_id: Option<EntityId>,
    pub component_type: Option<ComponentTypeId>,
    pub message: String,
    pub suggested_fix: Option<String>,
}
```

稳定 code：

```text
world.entity.duplicate_id
world.entity.missing
world.entity.stale_handle
world.parent.missing
world.parent.self
world.parent.cycle
world.component.missing
world.component.type_mismatch
world.component.decode_failed
world.entity_ref.missing_target
```

规则：

```text
错误对象只表达事实，不生成长 trace。
InstanceDiagnostic / RuntimeDiagnostics 通过薄转换补充 stage/path/next action。
不得用 catch_unwind 作为 production 错误转换机制。
不得把 panic 文本解析成 diagnostic code。
```

## 8. Fallible World Mutation Primitives

正式 public interface 至少覆盖：

```rust
try_spawn_entity(...)
try_spawn_with_components(...)
try_set_parent(...)
try_insert_component_value(...)
try_remove_component_value(...)
try_despawn_entity(...)
try_resolve_runtime_entity(RuntimeEntityId)
```

typed convenience API 如 `try_insert_transform` 必须委托同一底层 component insertion，不复制 entity existence 和 type validation。

### 8.1 每个操作的原子性

```text
Err 前不修改 entity map/archetype/hierarchy/component/dirty records。
duplicate spawn 不覆盖现有 Entity。
missing entity insert/remove 不 panic。
SetParent 在写入前验证 child、parent、自身和 cycle。
RuntimeEntityId 必须同时匹配 index、generation 和 alive。
```

### 8.2 panic helper 边界

若 archetype 内部仍需要 `expect`，只能用于已由 public `try_*` 证明的私有 invariant：

```text
private / pub(crate) prepared commit helper
debug assertion / engine bug
无法由 RuntimePackage、Project Module 或 public caller 输入直接触发
```

不得保留公开 panic API，再要求所有调用者“记得先检查”。

## 9. Shared Hydration Preflight

`scene_loader`、`RuntimeInstanceLoader` 和 `RuntimeSceneHydrator` 必须共享一个 prepare 语义，不建立第二 validator。

概念接口：

```text
RuntimeEntityHydrationPlan::prepare(
  source entities,
  instance namespace/mode,
  existing World read view,
  component schema/decoder context
) -> Result<PreparedRuntimeEntities, Vec<InstanceDiagnostic>>
```

它是 RuntimeInstanceLoader 内部的深实现，不是 transaction layer，不暴露 Undo/journal/rollback API。

### 9.1 Prepare 阶段完整检查

```text
所有 source id 非空且唯一。
映射后的 world id 不与现有 alive Entity 冲突。
每个 parent 指向同一输入集或允许的 existing parent。
无 self parent、无 hierarchy cycle。
每个 Entity 具有 required Transform。
typed component payload 可完整 decode。
component_type 与 ComponentValue 一致。
全部 EntityRef 可映射。
Prefab root 唯一且 destination parent/scene instance 有效。
所需 asset refs 已成功 resolve/load。
```

出现任何 error：

```text
不进入 World commit。
释放本请求已取得的 asset handles。
不注册 SceneInstance/PrefabInstance。
report.world_changed = false 或等价可证明语义。
```

### 9.2 Commit 阶段

Prepare 在持有同一 `&mut World` 的调用范围内完成；进入 commit 后，所有由外部输入决定的失败条件必须已消除。

```text
allocate prepared Entity skeletons
insert already-decoded components
apply already-validated hierarchy/reference mapping
collect source -> runtime map
only then mark instance Active and register ownership
```

244 不使用通用 rollback journal。若 prepared commit 仍因普通输入返回错误，说明 prepare 合同不完整，必须修 prepare；不得用运行时事务掩盖 validator 缺口。

### 9.3 静态与实例时验证

为避免重复 Prefab 验证影响 Runtime：

```text
RuntimePackage/Prefab 静态内容：duplicate、internal parent graph、required component、decode、internal EntityRef，可在 loader 内缓存成功 prepare 结果。
每次实例化动态条件：world id namespace、destination parent、target instance、handle generation，必须即时检查。
```

不得每帧全量扫描整个 RuntimePackage。

## 10. Consumer 收敛

### 10.1 Legacy scene_loader

`load_scene_into_world` 保持 compatibility facade，但内部必须调用共享 prepare/commit。它不能继续拥有一份简化循环。

返回语义：

```text
success -> RuntimeLoadResult::ok(World, diagnostics)
invalid source -> RuntimeLoadResult::failed(diagnostics)
never unwind from public input
```

### 10.2 RuntimeInstanceLoader / HydrationProjection

```text
ResolveAssets
  -> PrepareEntities
  -> CommitEntities
  -> Activate
```

`InstanceStage` 需要能区分 validate/prepare 与 commit；不得把所有错误都伪装成 `AttachComponents`。

`hydrate_active_scene_into_world` 和 `hydrate_scene(..., &mut existing_world)` 必须具有相同 fail-closed 语义。

### 10.3 GameplayCommand

```text
SpawnEntity -> try_spawn + prepared component checks
AddComponent -> try_insert_component_value
RemoveComponent -> try_remove_component_value
SetParent -> try_set_parent
DespawnEntity -> try_despawn
InstantiatePrefab -> RuntimeInstanceLoader prepared path
```

每条 command 必须稳定生成 `GameplayCommandApplyRecord`：

```text
result = ok | failed
error_code = stable code
no panic
```

244 不保证整个 `GameplayCommandBuffer` 的跨命令事务原子性；这是被明确 deferred 的 Runtime C 语义。

### 10.4 WorldWriteApi

`WorldWriteApi` 继续是项目规则的受限 mutation surface，但所有写入必须委托 World `try_*`；返回 `Result` 的函数内部不得再调用 panic public API。

## 11. Runtime 效率合同

244 必须保证 exported Windows Player 不承担 Editor transaction 成本。

### 11.1 稳定帧热路径不新增

```text
transaction plan
rollback journal
Undo stack
before/after snapshot
JSON report
完整 diagnostics trace
World clone
每帧 package validation
```

### 11.2 实际新增成本

```text
Scene/Prefab load：一次有限预检；只发生在 load/instantiate 边界。
structural mutation：duplicate/missing/generation/parent 检查。
```

多数检查当前已经以 `contains_key/get/unwrap_or_else(panic)` 形式存在。改成 `Result` 不增加新的查找，只改变失败控制流。

`SetParent` 的 cycle 检查最多沿 parent chain 访问，只有 reparent 时发生。普通 Transform、physics、render 和 value update 不受影响。

### 11.3 Report 分档

```text
Runtime Off：成功路径只保留功能必需 compact result；错误保留 compact diagnostic。
Runtime Summary：stage/count/error code，不生成完整 source map。
Runtime Trace：只供 test/gate/显式诊断，可包含 source->runtime debug map。
Editor Report Panel：只消费 Summary/Trace 产物，不迫使 Runtime 常驻生成。
```

## 12. 否定测试矩阵

### 12.1 World primitives

```text
duplicate source id spawn
insert into missing entity
remove missing component
set parent on missing child
set missing parent
self parent
two-node / deep hierarchy cycle
despawn missing entity
stale RuntimeEntityId after despawn/generation change
component type/value mismatch
```

每例必须断言：

```text
returns stable error code
catch_unwind sees no unwind
existing visible World state unchanged
no unexpected dirty record
```

### 12.2 Scene/Prefab hydration

```text
duplicate id at first/middle/last position
id collides with existing World
missing parent / self parent / cycle
missing Transform
invalid Collider2D or typed project component
missing nested EntityRef
missing asset dependency
missing destination parent
invalid/stale target instance
```

每例必须断言：

```text
no Active instance registered
no created Entity remains visible
asset handles released
diagnostic stage/source/code/next action stable
no unwind
```

### 12.3 Consumer regression

```text
legacy scene_loader and HydrationProjection produce equivalent rejection semantics
GameplayCommand always produces failed record
WorldWriteApi always returns Err
valid complex shooter Scene/Prefab still hydrates
second project from 242 still hydrates
Editor GameView and exported Player consume the same RuntimePackage truth
```

## 13. 预期涉及文件

生成施工文档前必须重新扫描 243 完成后的工作树。预计只涉及 Runtime/CQ-06 所有权：

```text
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/world_api.rs
rust/crates/engine_runtime/src/scene_loader.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
rust/crates/engine_runtime/src/runtime_scene_hydration.rs
rust/crates/engine_runtime/src/runtime_instance.rs
rust/crates/engine_runtime/src/runtime_instance_diagnostics.rs
rust/crates/engine_runtime/src/gameplay_command.rs
rust/crates/engine_runtime/src/diagnostics.rs
rust/crates/engine_runtime/src/lib.rs

rust/crates/editor_core/src/scene_editing.rs             // 仅 Preview World 调用迁移
rust/crates/editor_core/src/scene_editing/preview_world_sync.rs
rust/crates/editor_core/src/editor_gameview_play.rs      // 仅直接 World mutation 调用迁移
rust/crates/editor_window_winit/src/windowed_runtime_present.rs
rust/crates/editor_window_winit/src/tests/viewport_runtime.rs
rust/crates/editor_window_winit/src/tests/input_runtime_loop.rs

rust/crates/project_e2e_gate/src/lib.rs            // 仅跨项目否定 Gate 需要时
rust/crates/project_e2e_gate/src/<cq06_gate>.rs    // 仅有真实跨 crate leverage 时
```

允许为 shared prepare 新建一个 `engine_runtime` 私有 module，但不得新增 crate、Bridge、Router、Editor transaction 或第二套 report hierarchy。

上述 `editor_core` / `editor_window_winit` 文件只允许把直接 World mutation 调用机械迁移到 `try_*` 并处理 `Result`；不得修改 SceneEditTransaction、CommandTransaction、AuiTransaction、ProjectPatch rollback 或任何 Editor transaction 所有权。

244 方案生成期间，243 的并行施工已完成并归档。244 没有修改 `editor_core` LLM controller/transport、Cargo async dependency、AI panel 或 243 施工/完成记录；未来生成 244 施工文档前仍须基于 243 完成后的代码重新扫描。

## 14. 推荐施工 Gate

### Gate A：Fallible World Contract

```text
建立唯一 WorldMutationError 语义。
实现 try_spawn/try_insert/try_remove/try_set_parent/try_despawn/handle validation。
现有 panic helper 收为 private invariant。
完成 World primitive negative matrix。
```

建议测试：

```powershell
cargo test -p engine_runtime world
cargo test -p engine_runtime world_api
```

### Gate B：Shared Hydration Preflight

```text
完整 entity/hierarchy/component/ref prepare。
legacy scene_loader 委托 shared prepare/commit。
duplicate/missing parent/cycle/component negative matrix。
```

建议测试：

```powershell
cargo test -p engine_runtime scene_loader
cargo test -p engine_runtime runtime_scene_hydration
```

### Gate C：RuntimeInstanceLoader Fail-closed Activation

```text
asset resolve -> prepare -> commit -> activate。
失败释放 handles，不注册 instance，不留 visible Entity。
Scene/Prefab 使用同一 prepared path。
```

建议测试：

```powershell
cargo test -p engine_runtime runtime_instance_loader
cargo test -p engine_runtime runtime_instance
```

### Gate D：Mutation Consumer Migration / Runtime Report Budget

```text
GameplayCommand 与 WorldWriteApi 全部走 try_*。
稳定 error code 进入现有 apply/instance/runtime diagnostics。
Off/Summary/Trace 成本边界。
```

建议测试：

```powershell
cargo test -p engine_runtime gameplay_command
cargo test -p engine_runtime project_logic
cargo test -p engine_runtime frame_loop
```

### Gate E：Cross-project Regression / Docs

```text
复杂打飞机和 switch puzzle valid hydration 回归。
公开 API catch_unwind negative gate。
入口、240、完成记录和施工归档同步。
```

整体回归由未来唯一 244 施工文档根据 243 完成后的 baseline 固定，至少包括：

```powershell
cargo fmt --all -- --check
cargo test -p engine_runtime
cargo test -p project_e2e_gate
cargo test --workspace
cargo test --workspace --all-features
```

## 15. 本轮明确不做

```text
Editor transaction C 或统一 AuthoringTransaction。
Undo/Redo、Play Mode Apply transaction、ProjectPatch transaction 重写。
跨整个 GameplayCommandBuffer 的 all-or-nothing transaction。
每帧字段写 transaction。
完整 World snapshot/clone/restore。
网络预测 rollback、deterministic replay transaction。
Scene streaming/time-sliced hydration job。
Prefab hot reload / spawned instance live update。
CI/toolchain/lint/hygiene；继续按 240 Priority 5-6。
```

## 16. 风险与控制

### 风险 1：只把 panic 改名为 Result，内部仍先修改后报错

控制：每个 public `try_*` 固定“validate before mutation”；否定测试同时检查 state 和 dirty records。

### 风险 2：shared preflight 演化成新的 Runtime transaction layer

控制：Prepared data 只服务一次 hydration commit；没有 journal、Undo、arbitrary closure、通用 rollback 或独立 runtime routing。

### 风险 3：为了 fail-closed clone 整个 World

控制：完整预检消除可预见 commit error；不做 World clone。若普通输入仍能让 prepared commit 失败，修 prepare 合同。

### 风险 4：验证每次 Prefab instantiate 都重复 decode

控制：静态 Prefab prepare 可由 loader 缓存；每次只检查 destination World 的动态条件。

### 风险 5：错误报告进入稳定帧热路径

控制：成功 Off path 不分配长字符串、JSON、debug map；完整 Trace 仅显式开启或测试使用。

### 风险 6：为兼容旧 caller 继续公开 panic API

控制：迁移 production caller；panic helper 只能 private/pub(crate) 且无法由公开输入直接触发。

### 风险 7：把 Editor transaction C 偷带入 244

控制：244 预计文件清单不含 Editor transaction 所有权；任何 `editor_core` 扩围必须先回到方案讨论。

## 17. 方案自审

### 17.1 是否符合用户确认

通过。正式选择 Runtime B-min+；Editor C deferred，后续单独讨论。

### 17.2 是否完整关闭 CQ-06 设计缺口

通过。覆盖公开 scene loader、World mutation、WorldWriteApi、GameplayCommand、RuntimeInstanceLoader、HydrationProjection、duplicate/missing parent/stale handle 和 no-unwind matrix。

### 17.3 是否保证 Runtime 效率

通过。没有事务日志、Undo、World clone、常驻 JSON/report 或每帧全量验证。新增检查只发生在 load/structural mutation 边界；多数 map lookup 当前已经存在。

### 17.4 是否新增不必要结构

没有。唯一允许的新 module 是 RuntimeInstanceLoader 内部 shared prepare deep module，用来删除 `scene_loader` 与正式 hydration 的重复验证；它不成为运行时架构层。

### 17.5 是否保持 RuntimePackage / Projection 真相

通过。RuntimePackage 仍是运行输入；RuntimeSceneHydration 仍解释为 HydrationProjection；244 不新增独立 Bridge、assembler 或 Runtime 扫描源目录。

### 17.6 是否满足 AI 适配性

通过。错误具有稳定 code、operation、entity/component、stage adapter、suggested fix 和 no-world-change 语义；AI 不需要解析 panic 文本或猜测半成品。

### 17.7 是否适配复杂项目

通过。Scene、Prefab、项目 Rule command 和第二项目共享同一 fail-closed World boundary；不包含 complex shooter 专用语义。

### 17.8 是否正确处理成熟引擎参考

通过。采用 UE 的 preflight/deferred activation、Bevy 的 fallible/generation/entity map、Unity/Godot 的公开输入 guard；拒绝异常/日志黑盒、MissingNode 容错发布和 Bevy partial destination mutation。

### 17.9 是否与 243 冲突

不冲突。244 方案生成期间没有修改或运行 243 的任何施工对象；检测到 243 已由并行施工完成归档后，也没有自动开始 244 施工。

### 17.10 是否可以生成施工文档

设计范围、错误合同、接口、效率边界、否定矩阵和 Gate 已按 Runtime B-min+ 完成施工；整体回归与完成记录见 `阶段完成记录/2026-07-12-Diagnostics-first-Scene-Hydration-World-Mutation-Safety-v1/00-总览.md`。

最终结论：`通过；CQ-06 已关闭，Editor transaction C 继续 deferred`。

## 18. 正式结论

正式采用：

```text
Runtime B-min+

Fallible World Mutation Primitives
  + Shared Hydration Preflight
  + Prepared Commit
  + Fail-closed Instance Activation
  + Existing Diagnostic Adapters
  + Runtime Off/Summary/Trace cost boundary
```

CQ-06 的完成标准：

```text
公开错误输入不 unwind。
duplicate/missing parent/cycle/component/ref/stale handle 返回稳定 diagnostics。
Scene/Prefab 失败不注册 Active instance、不留下 visible Entity、不泄漏 asset handles。
GameplayCommand/WorldWriteApi 始终产生可审查失败结果。
Runtime 不引入 Editor transaction、Undo、World clone 或 steady-frame report 成本。
复杂打飞机、第二项目、default/all-features workspace 回归通过。
```

## 19. 后续优先级

244 方案讨论完成后，按 `240` 下一讨论项是：

```text
Priority 5：CQ-08 Reproducible Toolchain / CI / Lint Budget Gate v1
```

244 已完成施工并归档，当前没有施工中项目；下一讨论项按 240 为 CQ-08。

Editor transaction C 不自动插入 240 队列；只有用户明确重新排优先级或 Priority 5-6 讨论完成后，才单独讨论它。

## 20. 参考

```text
框架设计/引擎总体架构/239-Critical-Correctness-and-Safety-Convergence-Gate-v1方案.md
框架设计/引擎总体架构/240-5.6审查剩余问题讨论与施工优先级.md
框架设计/引擎总体架构/242-Project-RuntimeModule-Generic-Runtime-Decoupling-Second-Project-Gate-v1方案.md
框架设计/引擎总体架构/243-LLM-Request-Controller-Transport-Cancellation-Join-Lifecycle-v1方案.md
审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md
审查目录/5.6审查目录/01-2026-07-11-新增功能增量代码质量审查报告.md

rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/world_api.rs
rust/crates/engine_runtime/src/scene_loader.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
rust/crates/engine_runtime/src/runtime_scene_hydration.rs
rust/crates/engine_runtime/src/runtime_instance.rs
rust/crates/engine_runtime/src/runtime_instance_diagnostics.rs
rust/crates/engine_runtime/src/gameplay_command.rs

<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Runtime/Export/Scripting/UnityEngineObject.bindings.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Runtime/Export/SceneManager/SceneManager.cs
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/LevelActor.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/Components/SceneComponent.cpp
<GODOT_SOURCE>/godot-master/godot-master/scene/resources/packed_scene.cpp
<GODOT_SOURCE>/godot-master/godot-master/scene/main/node.cpp
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_world_serialization/src/dynamic_world.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_ecs/src/world/mod.rs

https://github.com/Unity-Technologies/UnityCsReference/blob/master/Runtime/Export/Scripting/UnityEngineObject.bindings.cs
https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Engine/Engine/UWorld/SpawnActor
https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Engine/Components/USceneComponent/AttachToComponent
https://github.com/godotengine/godot/blob/master/scene/resources/packed_scene.cpp
https://github.com/bevyengine/bevy/blob/main/crates/bevy_world_serialization/src/dynamic_world.rs
https://docs.rs/bevy_ecs/latest/bevy_ecs/world/struct.World.html
```
