# 244-当前可自动化施工文档-Diagnostics-first Scene Hydration / World Mutation Safety v1

> 状态：Gate A-E、整体回归、完成记录与入口同步均已完成；本文归档到 `已完成/`。  
> 正式方案：`../../244-Diagnostics-first-Scene-Hydration-World-Mutation-Safety-v1方案.md`。  
> 方案选择：用户已确认 Runtime B-min+；Editor transaction C deferred。  
> 审查输入：5.6 审查 CQ-06、244 正式方案、243 完成后的当前源码。  
> 执行规则：严格按 Gate A -> B -> C -> D -> E 串行执行；当前 Gate 失败先修复，不跨 Gate。

## 1. 施工目标

关闭 CQ-06：

```text
公开 Scene/Prefab hydration 和 World mutation 输入错误不 unwind。
World structural/component mutation 具有唯一 fallible error contract。
duplicate/missing parent/cycle/component/ref/stale handle 在 mutation 前被拒绝。
RuntimeInstanceLoader 先 prepare，再 commit，成功后才注册 Active instance。
GameplayCommand 与 WorldWriteApi 不再绕过 fallible World boundary。
Runtime 不引入 transaction journal、Undo、World clone 或常驻 report 成本。
```

完成必须证明：

```text
所有公开否定案例返回稳定 code/stage/source/next action。
失败后 existing visible World state 和 dirty records 不变。
失败不注册 SceneInstance/PrefabInstance，不泄漏 asset handles。
legacy scene_loader 与 HydrationProjection 复用同一 prepare 语义。
复杂打飞机与 switch puzzle 的合法 RuntimePackage/Player 链不回归。
default/all-features workspace 回归通过。
```

## 2. 施工前基线

```text
244 正式方案已由用户确认并完成自审。
生成本文前施工文档/当前/为空；生成后本文是唯一当前施工文档。
240 中 CQ-06 讨论状态=已完成，施工状态=未施工。
243 已完成归档，不再占用当前施工入口。
工作树存在 239-243 和用户未提交改动，必须保留，不 reset/checkout/revert。
```

当前源码证据：

```text
World::allocate_runtime_id 对 duplicate source id panic。
World::set_parent / insert_or_replace_component 对 missing entity panic。
WorldWriteApi 返回 Result，但部分写路径仍调用 panic API。
scene_loader 边 spawn 边检查 Transform，duplicate id 会 unwind。
RuntimeInstanceLoader 边 allocate/attach 边诊断，失败可能留下 Entity。
invalid Collider2D / missing EntityRef 可记录 error 后继续 activation。
GameplayCommand Spawn/AddComponent/SetParent 可在 ApplyRecord 前 unwind。
RuntimeEntityId generation 已存在，但 World 没有公开 fallible resolve/despawn contract。
```

施工文档预审修正：

```text
World panic API 收为 crate-private 后，editor_core/editor_window_winit 的直接调用点必须机械迁移。
这些迁移只处理 Result，不修改任何 Editor transaction 所有权。
Prepared hydration 是 RuntimeInstanceLoader 内部深实现，不是 Runtime transaction layer。
```

## 3. 固定范围

### 3.1 必须交付

```text
唯一 WorldMutationError 语义与稳定 error code。
try_spawn/try_insert/try_remove/try_set_parent/try_despawn/try_resolve_runtime handle。
missing/self/cycle parent validation。
shared RuntimeEntity hydration prepare/commit。
Scene/Prefab 完整 preflight、fail-closed activation、asset handle cleanup。
legacy scene_loader / RuntimeSceneHydrator / RuntimeInstanceLoader 语义收敛。
GameplayCommand / WorldWriteApi / direct external World consumer 迁移。
Runtime Off/Summary/Trace 的 source map/debug evidence 成本边界。
duplicate/missing/stale/cycle/component/ref/no-unwind 否定矩阵。
双项目和 workspace 回归。
```

### 3.2 禁止扩入

```text
不新增 Runtime WorldMutationTransaction、rollback journal 或 arbitrary mutation router。
不新增 Editor transaction C、Undo/Redo 或统一 AuthoringTransaction。
不修改 SceneEditTransaction/CommandTransaction/AuiTransaction/ProjectPatch rollback 所有权。
不 clone/restore 整个 World。
不做跨 GameplayCommandBuffer 的 all-or-nothing transaction。
不做网络预测 rollback、Scene streaming、time-sliced hydration、Prefab live update。
不处理 CQ-07/CQ-08。
```

## 4. 跨 Gate 固定合同

### 4.1 World mutation error

```text
code / operation / source_entity_id / runtime_entity_id /
parent_entity_id / component_type / message / suggested_fix
```

稳定 code 至少包括：

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

`RuntimeDiagnostics` / `InstanceDiagnostic` 只做 stage/path/next-action 薄转换，不维护第二套错误语义。

### 4.2 Mutation atomicity

```text
每个 public try_* 在 Err 前不修改 entity map/archetype/hierarchy/component/dirty records。
duplicate spawn 不覆盖 existing Entity。
SetParent 在 mutation 前验证 child/parent/self/cycle。
RuntimeEntityId 同时验证 index/generation/alive。
panic/expect 只能位于已验证的 crate-private invariant helper。
production 不使用 catch_unwind 转换错误。
```

### 4.3 Hydration prepare/commit

```text
ResolveAssets -> PrepareEntities -> CommitEntities -> Activate。
Prepare 完整验证 ids/world collisions/parent graph/Transform/component decode/EntityRef/prefab root/destination。
出现 error 不进入 commit，释放 handles，不注册 instance。
Commit 只消费 already-decoded prepared values；普通输入不得在 commit 中失败。
```

### 4.4 Runtime cost

```text
稳定帧不新增 transaction plan/journal/Undo/before-after/World clone/JSON。
成功 Off path 不构造完整 source->runtime debug map。
Summary 只保留 stage/count/code；Trace 才保留完整 debug map。
静态 Prefab prepare 允许由 loader 缓存；不得每帧重验整个 package。
```

## 5. Gate A - Fallible World Contract

### 5.1 施工

```text
在 World 所有权内建立 WorldMutationError。
实现 public try_spawn_entity / try_spawn_with_components。
实现 public try_insert_* / try_remove_component_value。
实现 public try_set_parent，包含 missing/self/cycle 检查。
实现 try_resolve_runtime_entity / try_despawn_runtime_entity stale generation 检查。
把旧 panic mutation 降为 crate-private prepared/invariant helper。
深化 WorldApiError 为 WorldMutationError alias/薄兼容，不保留第二 code truth。
```

### 5.2 阻断测试

```powershell
cargo test -p engine_runtime world -- --nocapture
cargo test -p engine_runtime world_api -- --nocapture
```

### 5.3 验收

```text
duplicate/missing/self/cycle/stale/type mismatch 返回稳定 code。
catch_unwind 证明公开 API 不 unwind。
失败前后 entity/component/hierarchy/dirty state 一致。
合法 spawn/insert/remove/reparent/despawn 行为不回归。
```

## 6. Gate B - Shared Hydration Preflight / Legacy Loader

### 6.1 施工

```text
建立 engine_runtime 私有 RuntimeEntityHydrationPlan/PreparedRuntimeEntities。
集中 entity namespace、Transform/render/sprite/component decode 与 EntityRef remap。
完整验证 duplicate/world collision/missing parent/self/cycle/missing Transform/component/ref。
legacy scene_loader 委托 shared prepare/commit，不再维护简化循环。
新增 ValidateInput/PrepareEntities/CommitEntities 等可辨识 stage（最终命名以现有 enum 收敛）。
```

### 6.2 阻断测试

```powershell
cargo test -p engine_runtime scene_loader -- --nocapture
cargo test -p engine_runtime runtime_scene_hydration -- --nocapture
```

### 6.3 验收

```text
duplicate first/middle/last、missing parent、cycle、missing Transform、invalid component/ref 均无 unwind。
失败返回 RuntimeLoadResult::failed，value=None。
合法 scene 的 Transform/render/sprite/dynamic component hydration 不回归。
```

## 7. Gate C - RuntimeInstanceLoader Fail-closed Activation

### 7.1 施工

```text
Scene/Prefab 统一改为 resolve assets -> shared prepare -> commit -> activate。
Prefab destination parent/target scene instance/root uniqueness 在 commit 前验证。
删除 allocate_entities/attach_components 边改边验路径。
任何 error 不注册 Active instance，不留下 visible Entity，释放 handles。
RuntimeInstantiateReport 增加 committed/world_changed 等价证据。
Runtime report level 默认 Off，完整 debug map 仅 Trace。
```

### 7.2 阻断测试

```powershell
cargo test -p engine_runtime runtime_instance_loader -- --nocapture
cargo test -p engine_runtime runtime_instance -- --nocapture
cargo test -p engine_runtime runtime_scene_hydration -- --nocapture
```

### 7.3 验收

```text
所有 Scene/Prefab negative fixtures 不注册 instance、不留 Entity、不泄漏 handles。
成功 instance root/source/runtime map 和 unload/despawn 行为不回归。
Projection summary 的 error_count/stage 与 compact diagnostics 一致。
```

## 8. Gate D - Consumer Migration / Report Budget

### 8.1 施工

```text
GameplayCommand Spawn/Add/Remove/SetParent/Despawn 使用 try_* 并输出 stable error_code。
WorldWriteApi 全部写路径委托 try_*。
engine_runtime 内其它 production direct caller 迁移或证明是 crate-private validated fixture。
editor_core PreviewWorldSync/editor_gameview 与 editor_window_winit 直接调用机械迁移。
外部迁移只处理 Result，不引入 Editor transaction。
```

### 8.2 阻断测试

```powershell
cargo test -p engine_runtime gameplay_command -- --nocapture
cargo test -p engine_runtime project_logic -- --nocapture
cargo test -p engine_runtime frame_loop -- --nocapture
cargo test -p editor_core scene_editing -- --nocapture
cargo test -p editor_window_winit viewport_runtime -- --nocapture
```

### 8.3 验收

```text
GameplayCommand 对 invalid input 始终生成 failed ApplyRecord，不 unwind。
WorldWriteApi invalid write 始终 Err。
PreviewWorldSync 与 Editor GameView 合法链不回归。
Runtime Off success path 不生成完整 debug evidence。
```

## 9. Gate E - Cross-project / Full Regression / Docs

### 9.1 施工与验证

```text
增加 CQ-06 validation-only cross-project gate（只有存在真实跨 crate leverage 时）。
复杂打飞机和 switch puzzle 证明合法 Scene/Prefab/RuntimePackage 继续运行。
运行 default/all-features workspace。
生成阶段完成记录，更新入口和 240，归档本文。
```

### 9.2 阻断测试

```powershell
cargo fmt --all -- --check
cargo test -p engine_runtime
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p project_e2e_gate
cargo test --workspace
cargo test --workspace --all-features
```

### 9.3 完成判定

```text
Gate A-D 定向测试全部通过。
default/all-features workspace 全部通过。
CQ-06 在 240 标记施工完成并有阶段完成证据。
施工文档从 当前/ 移入 已完成/。
```

## 10. 预计涉及文件

```text
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/world_api.rs
rust/crates/engine_runtime/src/scene_loader.rs
rust/crates/engine_runtime/src/runtime_entity_hydration.rs（新，private）
rust/crates/engine_runtime/src/runtime_instance_loader.rs
rust/crates/engine_runtime/src/runtime_scene_hydration.rs
rust/crates/engine_runtime/src/runtime_instance.rs
rust/crates/engine_runtime/src/runtime_instance_diagnostics.rs
rust/crates/engine_runtime/src/gameplay_command.rs
rust/crates/engine_runtime/src/diagnostics.rs
rust/crates/engine_runtime/src/lib.rs

rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/scene_editing/preview_world_sync.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_window_winit/src/windowed_runtime_present.rs
rust/crates/editor_window_winit/src/tests/viewport_runtime.rs
rust/crates/editor_window_winit/src/tests/input_runtime_loop.rs

rust/crates/project_e2e_gate/src/cq06_world_mutation_safety.rs（仅需要时）
rust/crates/project_e2e_gate/src/lib.rs（仅需要时）
```

其它调用点因 public API migration 命中时，可以做最小机械修改并在 Gate D 说明；不得静默扩大到 Editor transaction、AUI、ProjectPatch 或其它系统重构。

## 11. 工作树规则

```text
保留所有既有用户/239-243 未提交改动。
不 reset/checkout/revert unrelated changes。
一次只执行本文；不同时施工 CQ-07/CQ-08 或 Editor transaction C。
每 Gate 测试通过后才进入下一 Gate。
实现推翻 244 时先回填正式方案/本文，不用 transaction layer 掩盖 prepare 缺口。
```

## 12. 5.6 审查结论处理

### 12.1 必须修改

```text
scene_loader duplicate id panic -> Gate A/B。
World set_parent/insert missing entity panic -> Gate A。
公开 Result API 内部仍 panic -> Gate A/D。
RuntimeInstanceLoader partial World mutation -> Gate B/C。
```

### 12.2 施工约束

```text
duplicate/missing parent/stale handle/catch_unwind negative matrix 为阻断测试。
public input error 必须 diagnostics-first，不得解析 panic 文本。
Runtime 不引入 Editor transaction 成本。
```

### 12.3 已由历史施工吸收

```text
239 关闭 process/publish/PE/repair scope；本文不重复。
241 关闭 project write containment；本文不触碰文件写入。
242 提供第二项目 Gate；本文复用它验证通用性。
243 关闭 LLM lifecycle；本文不触碰其 transport/controller。
```

### 12.4 不适用

```text
CQ-07/CQ-08 仍按 240 后续独立讨论。
Editor transaction C 是用户明确 deferred 的后续系统。
```

## 13. 施工文档自审

### 13.1 与正式方案一致

通过。Gate A-D 完整覆盖 Runtime B-min+ 的 fallible primitives、shared preflight、prepared commit、consumer migration 和 compact diagnostics；没有引入 Runtime transaction C。

### 13.2 是否处理当前源码真实缺口

通过。施工项直接对应 `world.rs` panic、`scene_loader` 边改边验、`runtime_instance_loader` partial mutation、`world_api` Result/panic 不一致和 `gameplay_command` 无失败记录问题。

### 13.3 是否先处理公共 API 迁移影响

通过。预审已把 `editor_core` / `editor_window_winit` 的直接 World caller 列入 Gate D，只允许机械迁移和 Result 处理，不修改 Editor transaction 所有权。

### 13.4 是否形成深 module而非新层

通过。shared prepared hydration 删除 legacy loader 与 RuntimeInstanceLoader 的重复 decode/validation；它没有 journal、Undo、arbitrary closure、router 或公开 transaction API。

### 13.5 是否保证 Runtime 效率

通过。完整验证只位于 load/instantiate/structural mutation；稳定帧不新增事务、clone、JSON 或 trace。成功 Off path不构造完整 debug map。

### 13.6 测试是否覆盖目标

通过。Gate A-D 有定向测试，Gate E 有 engine/editor/window/e2e/default/all-features 回归；negative matrix 同时证明 error code、no unwind、World unchanged、no handle leak 和 no Active instance。

### 13.7 是否与当前队列冲突

不冲突。生成本文前 `施工文档/当前/` 为空；本文是唯一当前施工文档。CQ-08/CQ-07 保持待讨论，不并行施工。

### 13.8 是否可以开始施工

可以。正式方案已确认，范围/接口/Gate/测试/归档规则完整，自审无阻断。结论：`通过；从 Gate A 开始，失败先修当前 Gate。`

## 14. 最终执行结果

Gate A-E 全部完成。`cargo fmt --all -- --check`、engine/runtime、editor、window、project E2E、workspace default 与 workspace all-features 全部通过；CQ-06 已关闭。完成证据见 `../../阶段完成记录/2026-07-12-Diagnostics-first-Scene-Hydration-World-Mutation-Safety-v1/00-总览.md`。
