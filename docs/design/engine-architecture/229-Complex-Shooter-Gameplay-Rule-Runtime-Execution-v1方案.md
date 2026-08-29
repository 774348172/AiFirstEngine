# 229-Complex Shooter Gameplay Rule Runtime Execution v1 方案

> 状态：正式方案，用户已确认采用方案 C。  
> 校准日期：2026-07-09。  
> 所属路线：`227` 的 P0-2。  
> 前置：`225`、`226`、`228` 已完成。  
> 本文只生成方案，不允许直接施工；施工前仍需审查/自审和施工文档。

## 0. 用户确认结论

本系统已确认采用：

```text
方案 C：RuntimePackage manifest-gated Rust AOT Static Registry + 受限 RuleSlot
```

并收敛为 P0-2 可施工版本：

```text
C-min：先让复杂打飞机 sample 的项目侧 Rust AOT/static registry 规则，
通过 RuntimePackage.rules / RuleModuleRegistry / ProjectLogicRunner 真实进入 player runtime。
```

本方案的核心不是“新增一门脚本语言”，而是把已有规则资产管线接到真实运行：

```text
Rule Asset / RuntimeRuleManifest 负责可审查身份和执行计划。
Rust AOT/static registry 负责复杂打飞机 C-min 玩法执行。
ProjectLogicRunner 负责统一调度。
GameplayCommandBuffer / RuntimeInstanceLoader 负责 spawn/despawn 等结构变化。
Report 负责给 AI、测试和用户证明规则确实运行。
```

## 1. 一句话说明

这个系统让复杂打飞机项目从：

```text
RuntimePackage 里有 Scene / Prefab / Rule / Texture
```

变成：

```text
运行时真的按项目规则移动、开火、生成子弹、处理生命周期、碰撞、扣血、计分。
```

它对标其它引擎里的：

```text
Unity: C# / MonoBehaviour 进入 PlayerLoop
Unreal: C++ / Blueprint 进入 Actor / Component Tick 和 TickTaskManager
Godot: Script 进入 Node._process / _physics_process
Bevy: System 进入 Schedule，结构变化走 Commands / apply_deferred
```

在本引擎里，它不是新脚本语言，也不是把打飞机玩法写进 engine core，而是把项目侧规则运行接到已有链路：

```text
Project Rule Assets / RuntimeRuleManifest
  -> RuleModuleRegistry
  -> ProjectLogicRunner
  -> LogicContext
  -> ECS World / GameplayCommandBuffer
  -> Physics2D / Collision evidence
  -> RenderProjection / AUI snapshot 后续读取
  -> Structured Report
```

## 2. 为什么现在讨论它

`227` 已确认当前 P0 顺序：

```text
P0-1 Real Texture Decode / GPU Texture Upload / Sprite Textured Present v1
P0-2 Complex Shooter Gameplay Rule Runtime Execution v1
P0-3 Project Rule Driven UiStateSnapshot Producer v1
P0-4 Exported Windows Playable Golden Gate v1
```

其中 `228` 已完成 P0-1：

```text
真实 PNG -> cooked texture metadata + rgba8 payload
  -> RuntimePackage
  -> Sprite2D real texture binding
  -> RealWgpuBackend texture upload / sampler bind group
  -> real_texture_present report
```

现在最硬的缺口已经变成：

```text
看得见，但还不能证明真的能玩。
```

## 3. 外部引擎源码/文档参考

### 3.1 Unity

官方文档：

```text
https://docs.unity3d.com/ScriptReference/LowLevel.PlayerLoop.html
```

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\PlayerLoop\PlayerLoop.bindings.cs
```

关键点：

```text
ScriptRunBehaviourFixedUpdate
ScriptRunBehaviourUpdate
ScriptRunBehaviourLateUpdate
UpdateAllRenderers
```

可学习：

```text
项目脚本最终进入统一 PlayerLoop。
逻辑先更新，渲染同步在后。
用户心智简单：项目逻辑不是零散被渲染器或输入系统调用。
```

不照搬：

```text
不复制 MonoBehaviour callback 大全集。
不把脚本源码当成本项目唯一真相。
不引入 C# Domain Reload 心智。
```

### 3.2 Unreal Engine

官方文档：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/actor-ticking-in-unreal-engine
```

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\TickTaskManager.cpp
```

关键点：

```text
FTickTaskManager
StartFrame
RunTickGroup
TG_PrePhysics / TG_DuringPhysics / TG_PostPhysics
DoDeferredRemoves
```

可学习：

```text
项目逻辑必须进入统一 tick 管线。
物理前、物理后、渲染前的阶段边界要明确。
Actor/Component 可以 tick，但具体顺序受 TickGroup 和依赖关系控制。
```

不照搬：

```text
第一版不做完整 Tick prerequisite 图。
不做 UE Blueprint VM。
不做 Live Coding / Hot Reload。
```

### 3.3 Bevy

官方文档：

```text
https://docs.rs/bevy_ecs/latest/bevy_ecs/schedule/struct.Schedule.html
```

源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_ecs\src\schedule\schedule.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_ecs\src\schedule\auto_insert_apply_deferred.rs
```

关键点：

```text
Schedule::run
SystemSchedule
apply_deferred
AutoInsertApplyDeferredPass
```

可学习：

```text
规则/系统执行必须有集中 schedule。
结构变化不直接改 World，走 Commands / deferred apply。
```

不照搬：

```text
不把 Bevy 的完整 Schedule/SystemSet 心智暴露给普通用户和 AI。
```

### 3.4 Godot

官方文档：

```text
https://docs.godotengine.org/en/stable/tutorials/scripting/idle_and_physics_processing.html
```

源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\main\node.cpp
```

关键点：

```text
NOTIFICATION_PROCESS -> _process(delta)
NOTIFICATION_PHYSICS_PROCESS -> _physics_process(delta)
set_process
set_physics_process
```

可学习：

```text
每帧逻辑和固定物理逻辑分开。
脚本生命周期入口要直观。
```

不照搬：

```text
不采用 Node callback / Variant 动态调用作为本项目长期核心。
```

## 4. 本项目当前基线

### 4.1 已经有的底座

相关代码：

```text
rust/crates/engine_runtime/src/project_logic.rs
rust/crates/engine_runtime/src/logic_executor.rs
rust/crates/engine_runtime/src/rule_registry.rs
rust/crates/engine_runtime/src/rule_artifact.rs
rust/crates/engine_runtime/src/rule_compiler.rs
rust/crates/engine_runtime/src/project_rule_asset.rs
rust/crates/engine_runtime/src/gameplay_command.rs
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/runtime_scene_hydration.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
```

当前已经具备：

```text
ProjectRuleAsset
Canonical Rule IR v1
RuleCompiler 生成 Rust source 字符串
generate_static_registry_source
RuntimeRuleManifest
RuleArtifactManifest / RuleArtifactRegistry
RuleModuleRegistry
ProjectLogicRunner
LogicContext.action_pressed
LogicContext.query / read_component / write_component_field
GameplayCommandBuffer
InstantiatePrefab / DespawnPrefabInstance command
Physics2D collision pair detection
RuntimeSceneHydrator 把 RuntimePackage scene/prefab hydration 到 World
```

### 4.2 当前没有闭合的缺口

#### 缺口 A：player 没有使用 package rules 构建 runner

`runtime_player_winit` 当前会：

```text
load_runtime_package
hydrate_active_scene_into_world
resolve input
EngineHostLoop::new(...)
host.tick(...)
```

但 `EngineHostLoop::new(...)` 内部是空 `ProjectLogicRunner`。

因此：

```text
RuntimePackage 里有 rules，不代表 player 会执行 rules。
```

#### 缺口 B：FrameLoop 不带 RuntimeCommandContext

`ProjectLogicRunner` 已经有：

```text
run_frame_update_with_runtime(...)
run_fixed_update_with_runtime(...)
```

它能让 `InstantiatePrefab` 通过 `RuntimeInstanceLoader` 从 RuntimePackage 生成实体。

但 `FrameLoop::tick_runtime_frame_with_input_and_delta(...)` 当前调用的是：

```text
run_fixed_update_with_time_context(...)
run_frame_update_with_time_context(...)
```

这条路径没有 `RuntimeCommandContext`，所以规则里发出的 `InstantiatePrefab` 会走到：

```text
missing_runtime_context
```

#### 缺口 C：EventHandler / RuntimeEventQueue 未形成运行时路径

`RuleExecutionPlan` 有：

```text
fixed_update
frame_update
event_handler
```

但当前 FrameLoop 没有真正执行 event handler pass。

`RuleCompiler` 对 `EventReceived` / `EmitEvent` 也仍输出 unsupported diagnostic：

```text
eventReceived trigger requires RuntimeEventQueue
emitEvent requires RuntimeEventQueue
```

所以本轮不能把复杂打飞机的开火/碰撞建立在 EventReceived 上。

#### 缺口 D：sample rule asset 仍是空语义

当前 sample 有：

```text
samples/complex_shooter_project/Rules/player_move.rule.json
samples/complex_shooter_project/Rules/fire_bullet.rule.json
samples/complex_shooter_project/Rules/lifetime_cleanup.rule.json
samples/complex_shooter_project/Rules/rule-manifest.json
```

但 3 个 rule asset 的 `canonicalIr.statements` / `operations` 目前都是空数组。

`rule-manifest.json` 引用：

```text
Rules/generated/sample_project_rules.rs
```

但这个文件当前并不存在。

#### 缺口 E：复杂项目验证明确标出规则管线未连接

`rust/crates/engine_runtime/src/complex_project_validation.rs` 当前还有：

```text
project_rule_pipeline_not_connected
```

诊断说明：

```text
sample behavior is executed by validation fixture code,
not by real Project Rule / IR / Rust AOT generation.
```

## 5. 必须遵守的边界

以 `195` / `196` 为最高解释：

```text
用户心智是 Rust Project Framework + Project Assets。
Gameplay Rule Asset 只是 Project Assets 的一类，不等于全部项目逻辑。
IR 只存在于 Contract-bound RuleSlot 中，是受限规则数据。
IR 不是 Lua / Blueprint 式脚本语言。
复杂算法、系统流程、复杂状态机默认进 Rust Project Module / Rust Framework。
如果为了进入 IR 必须把 IR 扩成编程语言，就应改为 Rust Module + RuleSlot。
```

因此本系统禁止：

```text
把 Player / Enemy / Bullet / Score / Health / Weapon 做成 engine core API。
把 Physics2D 内部对象直接暴露给项目规则。
让规则直接操作 RenderProxy / GPU handle / AUI internals。
为了打飞机移动/碰撞把 IR 加 while、任意函数、任意数组算法。
新增 Logic Ownership Router / Architecture Guard 作为运行时层。
```

## 6. 可选方案

### 方案 A：全 IR Gameplay Execution

做法：

```text
把玩家移动、敌人移动、开火、子弹生命周期、碰撞、扣血、计分都写进 Canonical Rule IR。
Runtime 通过 IR -> Rust AOT codegen 或 IR interpreter 执行。
```

优点：

```text
规则资产最可见。
AI patch 和结构化 diff 最直接。
理论上热更空间最大。
```

问题：

```text
当前 IR/codegen 还不能表达 velocity * deltaTime、碰撞 pair 遍历、复杂状态更新。
为了支持复杂打飞机，会很快增加循环、函数、数组算法和事件系统。
这会把 IR 推向劣化脚本语言，和 195 / 196 冲突。
当前 Runtime IR interpreter 是 validation-only unsupported，不是正式执行路径。
```

结论：

```text
不推荐作为 P0-2 主线。
```

### 方案 B：纯 Rust Project Module

做法：

```text
复杂打飞机项目逻辑全部写成 Rust Project Module。
RuntimePackage 只负责 Scene / Prefab / Asset / Input / AUI 数据。
Rule assets 只作为文档或未来入口，不参与本轮运行。
```

优点：

```text
最快。
性能最好。
适合复杂算法、碰撞响应、生命周期管理。
接近 Unity C# / UE C++ 的落地方式。
```

问题：

```text
Rule Authoring 变成摆设。
AI 和用户难以从 Rule Asset / Report 追踪规则来源。
后续要把规则编辑产品化时会反向迁移。
```

结论：

```text
可作为极限救火，但不推荐作为正式 P0-2。
```

### 方案 C：RuntimePackage manifest-gated Rust AOT Static Registry + 受限 RuleSlot

做法：

```text
RuntimePackage 的 RuntimeRuleManifest 是运行时执行计划来源。
RuleModuleRegistry 提供已编译/已链接的 Rust AOT rule 函数。
ProjectLogicRunner 按 manifest phase/order 调用规则。
复杂打飞机 C-min 使用项目侧 Rust AOT/static registry 实现真实玩法。
Rule Asset / sourceMap / artifactId / irHash 继续作为 AI 和用户可审查入口。
简单、稳定、受限的 RuleSlot 后续继续由 Canonical IR -> Rust AOT 派生。
```

关键解释：

```text
这不是新增一层脚本系统。
这是把 186 / 187 已有的 ProjectLogicRunner + RuleModuleRegistry 真正接到 player。
```

项目心智压缩为：

```text
Project Assets
  -> RuntimePackage
  -> Project Gameplay Runtime
  -> ECS / Command / Report
```

优点：

```text
AI 适配性：manifest、rule asset、sourceMap、report 都是结构化对象。
复杂项目维护：复杂流程在 Rust，RuleSlot 保持受限。
效率：运行时走 Rust AOT/static registry，不走解释器。
施工风险：复用现有 ProjectLogicRunner / GameplayCommandBuffer / HydrationProjection。
```

风险：

```text
如果 C-min 手写 Rust AOT 函数不受 manifest/hash/sourceMap 约束，会变成第二真相层。
如果把 sample 玩法函数写进 engine core，会污染引擎边界。
如果不接 RuntimeCommandContext，spawn prefab 仍无法成功。
```

结论：

```text
推荐。
```

## 7. 推荐方案：C-min

正式采用：

```text
C-min: RuntimePackage manifest-gated Rust AOT Static Registry + Complex Shooter Project Runtime Evidence
```

本轮目标不是完成通用脚本系统，而是打穿复杂打飞机最小可玩规则链：

```text
RuntimePackage
  -> package.rules
  -> RuleModuleRegistry
  -> ProjectLogicRunner
  -> EngineHostLoop / FrameLoop
  -> LogicContext + RuntimeCommandContext
  -> ECS World / GameplayCommandBuffer
  -> Physics2D collision evidence
  -> Gameplay Runtime Execution Report
```

### 7.0 方案 C 的三条主线

方案 C-min 分成三条必须同时成立的链路。

#### 主线 A：Rule boot chain

把 RuntimePackage 中的规则声明变成运行时执行计划：

```text
RuntimePackage.rules/rule-manifest.json
  -> validate_runtime_rule_manifest_artifacts
  -> RuleModuleRegistry
  -> ProjectLogicRunner::from_rule_manifest_and_registry
  -> RuleExecutionPlan
```

规则：

```text
RuntimeRuleManifest 是执行计划来源。
Static registry 只提供函数指针，不决定哪些规则运行。
manifest 引用但 registry 缺失时必须报 missing_registered_rule，不允许静默成功。
manifest disabled 的 rule 不执行，但应能在 report 中说明 skipped/disabled。
```

#### 主线 B：Runtime tick chain

把 ProjectLogicRunner 接到真实 player 帧循环：

```text
runtime_player_winit
  -> load_runtime_package
  -> hydrate_active_scene_into_world
  -> build ProjectLogicRunner from package.rules + registry
  -> EngineHostLoop / FrameLoop tick
  -> ProjectLogicRunner phase execution
```

规则：

```text
Editor Play 和 exported player 后续应走同一类 runner 构建入口。
P0-2 先保证 runtime_player_winit / headless gate 路径真实运行。
FrameLoop 必须能给规则执行传入 RuntimeCommandContext，否则 InstantiatePrefab 仍会失败。
```

#### 主线 C：Gameplay evidence chain

把“规则执行过”变成可验证 gameplay 事实：

```text
input evidence
  -> action.move / action.fire
  -> logic result
  -> command enqueue/apply
  -> spawned prefab / changed transform
  -> collision evidence
  -> project state changed
  -> structured report
```

规则：

```text
不能只证明 rule count > 0。
必须证明至少一个输入驱动了一个 gameplay 结果。
必须证明 spawn/despawn 走 RuntimePackage prefab，不手造验证实体冒充。
必须证明 score/health/session state 通过 generic dynamic component 写入，不新增打飞机 engine API。
```

### 7.1 最小 gameplay 范围

P0-2 C-min 必须至少证明：

```text
player movement:
  action.move -> 修改 entity-player Transform.local_position

fire bullet:
  action.fire -> InstantiatePrefab(prefab-player-bullet)

linear motion:
  project.linearMotion.velocity -> 每帧移动 enemy / bullet

lifetime cleanup:
  超出边界或 lifetime 到期 -> DespawnEntity / DespawnPrefabInstance

collision response:
  bullet vs enemy -> despawn bullet/enemy 或标记 inactive
  player vs enemy -> 修改 project.combatState / project.sessionState

score / health:
  project.sessionState.score 增加
  project.combatState.hp 减少或保持可验证字段
```

项目语义只能出现在：

```text
samples/complex_shooter_project/**
project-side generated/static rule module
project_e2e_gate report / fixture
```

不得进入：

```text
engine_runtime 通用 API 命名
RuntimePackage 通用 schema 字段命名
Renderer / Physics / AUI core API
```

### 7.2 规则资产的角色

本轮不把 IR 扩成脚本语言。

Rule Asset 在 C-min 中承担：

```text
规则身份：ruleId / assetId
规则来源：sourceMap
执行计划：phase / enabled / order
产物校验：artifactId / irHash
AI/用户定位：displayName / diagnostics / report
```

如果本轮使用手写 Rust AOT 函数模拟 generated artifact，必须标记为：

```text
project-side static generated placeholder
```

并满足：

```text
函数只在 manifest 声明且 artifact/hash 匹配时注册。
函数 report 必须回指 ruleId / artifactId / sourceMap。
函数不能新增 manifest / rule asset 没声明的业务规则。
函数不能写在 engine core 通用 API 中。
```

项目侧 static registry 的命名和放置建议：

```text
complex_shooter_project_rules
  register_complex_shooter_project_rules(registry)
  rule_player_move(context)
  rule_fire_bullet(context)
  rule_linear_motion(context)
  rule_lifetime_cleanup(context)
  rule_collision_response(context)
```

边界：

```text
这些函数可以作为 C-min 的 project-side generated/static placeholder。
它们可以存在于 sample / project_e2e_gate / runtime player 可链接的项目侧模块中。
它们不能作为 engine_runtime 默认内置玩法系统。
```

后续 C-full 再把这些 placeholder 收敛为：

```text
ProjectRuleAsset / RuleSlot
  -> generated Rust source
  -> generated static registry
  -> cargo build player
```

### 7.3 FrameLoop 接入规则

P0-2 不新增新 runtime layer，只扩通现有调用链。

当前：

```text
FrameLoop -> run_frame_update_with_time_context
```

应收敛为：

```text
FrameLoop + optional RuntimeCommandContext
  -> run_fixed_update_with_runtime / equivalent
  -> run_frame_update_with_runtime / equivalent
  -> Physics2D sync + build_collision_pairs
  -> optional post-physics project rule pass
  -> RenderProjection
```

C-min 可接受两种实现：

```text
方案 C-min-1:
  增加 RuntimeFrameContext，把 package + RuntimeInstanceLoader 传进 FrameLoop。

方案 C-min-2:
  EngineHostLoop 持有 RuntimeSceneHydrator / RuntimeInstanceLoader，
  tick 时把 RuntimeCommandContext 借给 ProjectLogicRunner。
```

优先 C-min-1：

```text
更贴近当前 ProjectLogicRunner 已有 with_runtime API。
改动面小。
不会新增独立 gameplay manager。
```

### 7.4 Event / Collision 边界

本轮不做完整 RuntimeEventQueue。

因此：

```text
action.fire 不走 EventHandler/EventReceived，先走 Update + action_pressed。
collision response 如果作为 Project Rule，必须由 RuntimeRuleManifest 声明 phase。
```

如果施工时确实需要暴露碰撞输入，只允许增加受控只读输入：

```text
LogicContext.collision_pairs()
```

或等价的只读 `CollisionPairSnapshot`。

它只能包含：

```text
entity_a
entity_b
collider / layer / mask 的必要摘要
```

禁止暴露：

```text
Physics2DWorld 内部可变对象
窄相位临时结构
Renderer / GPU 对象
```

### 7.5 sample asset 小修范围

当前 sample 需要被规则运行证据使用，因此施工时可以小修 project asset：

```text
player_bullet.prefab.json:
  补 SpriteRenderer2D
  补 Collider2D 或 project collision marker
  补 project.lifetime 或 bounds cleanup 字段

enemy_scout.prefab.json:
  确认 SpriteRenderer2D / project.linearMotion / Collider2D

Main.scene.json:
  确认 entity-player 有 playerController / spawnEmitter / Collider2D / player state
  确认 session state 可记录 score / wave / health

Rules/*.rule.json:
  从空 statements/operations 改为能回指真实规则 intent 的最小结构
  或明确标注本轮为 static generated placeholder

Rules/rule-manifest.json:
  rule.fire-bullet 当前是 EventHandler，但 C-min 不做 RuntimeEventQueue
  因此施工必须先改为 Update + action_pressed("action.fire")
  如果增加 rule.collision-response，必须走通用 PostPhysics phase，不允许在 manifest 外偷偷执行
```

这些都属于项目侧 sample 资产，不污染 engine core。

### 7.6 方案 C-min 的最小数据约定

为了避免把打飞机语义写进引擎 API，本轮只约定 sample project 内的数据形态。

建议项目动态组件：

```text
project.playerController:
  moveAction
  fireAction
  speed

project.spawnEmitter:
  prefabId
  cooldown
  elapsed

project.linearMotion:
  velocity

project.lifetime:
  age
  maxAge
  bounds

project.combatState:
  team
  hp
  damage
  scoreValue

project.sessionState:
  score
  wave
```

规则：

```text
组件名和字段名只属于 samples/complex_shooter_project。
engine_runtime 只按 ComponentTypeId + RuntimeValue + FieldPath 处理。
若字段缺失，规则 report 要给出自然语言诊断和 sourceMap，不 panic。
```

### 7.7 规则执行阶段

C-min 阶段顺序建议：

```text
FixedUpdate:
  可选，当前不强制使用。

Update:
  player_move
  fire_bullet
  linear_motion
  lifetime_cleanup

PostPhysics / CollisionResponse:
  collision_response
```

当前 `RulePhase` 还没有 PostPhysics。自审后修正为：

```text
如果 collision_response 被定义为 Project Rule，就必须由 RuntimeRuleManifest 声明。
因此 C-min 允许新增一个通用 RuntimeRulePhase::PostPhysics。
它只表示“Physics2D pair report 生成后、RenderProjection 前”的运行阶段。
它不能包含 bullet/enemy/score/health 等项目专用语义。
```

C-min 不再推荐 manifest 外的隐藏 hook：

```text
不允许在 FrameLoop 里直接调用 complex_shooter_collision_response。
不允许 registry 之外的函数绕过 RuntimeRuleManifest 执行。
如果施工认为 PostPhysics phase 超出本轮范围，则 collision_response 只能作为 validation evidence，
不能宣称“规则驱动的碰撞响应已完成”。
```

## 8. 运行报告

新增或扩展报告分两层：

```text
engine_runtime 通用层:
  GameplayRuleRuntimeExecutionReport

project_e2e_gate / sample 层:
  ComplexShooterGameplayRuntimeExecutionReport
```

规则：

```text
engine_runtime 中不得新增带 ComplexShooter / Player / Enemy / Bullet / Score / Health 语义的公共报告类型。
复杂打飞机专用报告只能在 sample / project_e2e_gate / 测试 fixture 中包装通用报告。
```

最小字段：

```text
schemaVersion
scenarioId
status
packagePath
ruleManifestSummary
registeredRuleCount
missingRegisteredRules
framesSimulated
inputEvidence
ruleExecutionSummary
commandSummary
spawnedPrefabSummary
collisionSummary
projectStateSummary
diagnostics
```

最小验收指标：

```text
rule.player-move executed or explicitly skipped with reason
rule.fire-bullet consumed action.fire and enqueued instantiate_prefab
prefab-player-bullet instantiated from RuntimePackage
linear motion changed at least one transform
collision pair evidence exists after simulated frames
score/health/session project state changed through generic dynamic component write
no engine core project-specific API was used
```

Report 分档规则：

```text
Runtime default:
  Off 或 Summary，不写长 trace 文件。

Gate / test:
  Summary + selected Trace。

Editor / Report Panel:
  只接收 Summary / Trace 产物，不把 runtime 热路径变成常驻长报告。
```

## 9. 和 P0-3 / P0-4 的关系

### P0-3 Project Rule Driven UiStateSnapshot Producer v1

P0-2 只负责让 gameplay 状态真实变化。

P0-3 才负责：

```text
project gameplay state
  -> ProjectUiStateSnapshot
  -> AUI Binding
  -> HUD score / health / wave present
```

本轮可以产生 `project.sessionState.score` / `project.combatState.hp`，但不要求 HUD 已绑定真实值。

### P0-4 Exported Windows Playable Golden Gate v1

P0-4 是最终验收：

```text
真实 Windows exe
真实窗口/GPU
真实输入
真实贴图
真实 gameplay
真实 HUD
像素/输入/碰撞/计分 evidence
```

P0-2 只负责玩法执行链路，P0-4 再做窗口级 golden。

## 10. 推荐施工切分

后续生成施工文档时建议分 Gate：

### Gate A：Rule Runtime Bootstrapping

目标：

```text
runtime_player_winit / headless gate 从 RuntimePackage.rules + static registry 构建 ProjectLogicRunner。
缺失注册函数输出结构化 diagnostic，不静默成功。
```

测试：

```text
manifest 有 rule 但 registry 缺失 -> missing_registered_rule diagnostic
manifest + registry 匹配 -> runner plan 包含 3+ rules
```

### Gate B：RuntimeCommandContext 接入 FrameLoop

目标：

```text
ProjectLogicRunner 在 player frame tick 中带 package + RuntimeInstanceLoader。
InstantiatePrefab 能从 RuntimePackage 生成 World entity。
同时必须保留 RuntimeTime 产生的 delta_time / fixed_delta_time / in_fixed_step。
```

测试：

```text
action.fire -> instantiate_prefab command_apply ok
world entity_count 增加
trace 记录 source prefab-player-bullet
delta_time rule 在 with_runtime 路径下仍读取当前帧时间，而不是 DEFAULT_FIXED_DELTA_TIME
```

### Gate C：Complex Shooter Project Static Rules C-min

目标：

```text
项目侧 static Rust AOT placeholder 注册 player_move / fire_bullet / lifetime_cleanup。
可选增加 collision_response rule。
```

测试：

```text
action.move -> player transform changed
action.fire -> bullet prefab spawned
linearMotion -> bullet/enemy transform changed
lifetime -> out-of-bounds entity removed
```

### Gate D：Collision / Score / Health Evidence

目标：

```text
经过若干帧，Physics2D 产生碰撞证据。
项目状态通过 generic dynamic component 更新 score / health。
collision_response 如果作为规则执行，必须通过 RuntimeRuleManifest 的通用 PostPhysics phase。
```

测试：

```text
collision_pair_count >= 1
score changed
health changed or validated stable
no project-specific engine API
```

### Gate E：Report / Regression Gate

目标：

```text
新增 engine_runtime 通用 GameplayRuleRuntimeExecutionReport。
project_e2e_gate 可包装 ComplexShooterGameplayRuntimeExecutionReport。
关闭 complex_project_validation 的 project_rule_pipeline_not_connected gap，或新增更准确的 remaining gap。
```

测试：

```text
cargo test -p engine_runtime project_logic
cargo test -p engine_runtime complex_project_validation
cargo test -p runtime_player_winit
cargo test -p project_e2e_gate complex_shooter
```

具体命令以施工文档自审为准。

### Gate F：文档与入口同步

目标：

```text
完成施工后更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
如果关闭 project_rule_pipeline_not_connected，必须在完成记录中写明证据。
如果只改成更准确的 remaining gap，也必须写明剩余阻塞属于 P0-3 / P0-4 / 后续系统。
```

测试：

```text
检查施工文档归档。
检查阶段完成记录存在。
检查 227 下一项推进到 P0-3。
```

## 11. 不做范围

P0-2 不做：

```text
完整 Rule Graph UI。
完整 RuntimeEventQueue。
IR Interpreter runtime hotfix。
动态 DLL / dylib 热加载。
通用 generated_rules crate 写盘 + cargo build 完整产品化。
完整 Windows golden。
HUD 真实绑定。
完整波次系统、Boss、掉落、装备系统。
```

这些分别归：

```text
Rule Authoring / Rule Graph Productization
RuntimeEventQueue 后续系统
热更新后续系统
Rule Artifact / Build Productization 后续系统
P0-4 Exported Windows Playable Golden Gate
P0-3 Project Rule Driven UiStateSnapshot Producer
后续复杂 gameplay 扩展
```

## 12. 自审

### 2026-07-09 自审补充

本次自审结论：

```text
方案 C-min 主体成立，但必须小修施工约束。
修正后不推翻方案 C，不新增脚本语言，也不新增独立 gameplay manager。
```

已修正的问题：

```text
1. sample 中 rule.fire-bullet 当前是 EventHandler；
   C-min 不做 RuntimeEventQueue，因此施工必须改为 Update + action_pressed。

2. collision_response 不能通过 manifest 外隐藏 hook 执行；
   如果它是 Project Rule，就必须新增通用 RuntimeRulePhase::PostPhysics 并由 manifest 声明。

3. RuntimeCommandContext 接入 FrameLoop 时不能丢 RuntimeTime；
   run_with_runtime 路径必须同时携带 delta_time / fixed_delta_time / in_fixed_step。

4. engine_runtime 不能新增 ComplexShooter 专用公共报告类型；
   通用层用 GameplayRuleRuntimeExecutionReport，project_e2e_gate 再包装复杂打飞机专用报告。

5. health 字段统一收敛到 project.combatState.hp；
   避免同一方案里同时出现 project.playerState.health 和 project.combatState.hp 两套心智。
```

施工文档必须继承这些约束，否则不能进入正式施工。

### 是否符合 227

符合。

`228` 已完成真实贴图，当前下一项应是 P0-2。本文没有跳到 P0-3/P0-4。

### 是否符合 195 / 196

基本符合。

本文明确不推荐全 IR gameplay，不把 IR 扩成脚本语言。复杂移动、碰撞、状态更新默认由项目侧 Rust AOT/static registry 承担，RuleSlot 保持受限。

### 是否避免 engine core 污染

符合，但施工必须继续守住：

```text
Player / Enemy / Bullet / Score / Health 只能出现在 sample/project-side/report。
```

### 是否避免新增不必要结构

符合。

本文不新增 Logic Ownership Router / Architecture Guard / Gameplay Manager 运行时层，只把现有：

```text
RuntimeRuleManifest
RuleModuleRegistry
ProjectLogicRunner
FrameLoop
GameplayCommandBuffer
RuntimeInstanceLoader
```

连接起来。

### 是否足够支持复杂打飞机 P0

有条件符合。

必须在施工中补：

```text
player 规则注册入口
RuntimeCommandContext 传入 FrameLoop
sample static rule implementation
collision/score/health evidence
structured report
```

否则仍会停留在“包里有规则文件，但规则没跑”。

### 最大风险

最大风险是 C-min 为了快，把 sample gameplay 写成 engine_runtime 内置逻辑。

治理规则：

```text
任何 sample gameplay 函数必须是 project-side/generated/static placeholder。
engine_runtime 只暴露 generic ECS / command / report 能力。
```

## 13. 结论

推荐采用：

```text
方案 C-min：RuntimePackage manifest-gated Rust AOT Static Registry + 受限 RuleSlot
```

下一步如果进入施工，应先生成施工文档，并从最小可验证链路开始：

```text
package.rules
  -> registry build runner
  -> FrameLoop with RuntimeCommandContext
  -> action.fire instantiate prefab
  -> linear motion / lifetime / collision / score evidence
  -> GameplayRuleRuntimeExecutionReport
  -> project_e2e_gate ComplexShooterGameplayRuntimeExecutionReport wrapper
```

这一步做完后，复杂打飞机项目才从：

```text
导出窗口能看到飞机
```

推进到：

```text
导出/预览 runtime 里飞机真的能动、能射击、能碰撞、能计分。
```
