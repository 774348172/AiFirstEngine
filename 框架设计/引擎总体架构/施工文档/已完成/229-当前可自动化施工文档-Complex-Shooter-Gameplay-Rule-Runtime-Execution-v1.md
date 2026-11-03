# 229-当前可自动化施工文档-Complex Shooter Gameplay Rule Runtime Execution v1

> 状态：当前唯一施工文档。  
> 对应方案：`框架设计/引擎总体架构/229-Complex-Shooter-Gameplay-Rule-Runtime-Execution-v1方案.md`。  
> 选择：方案 C-min，RuntimePackage manifest-gated Rust AOT Static Registry + 受限 RuleSlot。  
> 日期：2026-07-09。

## 1. 施工目标

把复杂打飞机项目从“RuntimePackage 里有 Scene / Prefab / Rule / Texture”推进到：

```text
RuntimePackage.rules
  -> RuleModuleRegistry
  -> ProjectLogicRunner
  -> EngineHostLoop / FrameLoop
  -> LogicContext + RuntimeCommandContext
  -> ECS World / GameplayCommandBuffer
  -> Physics2D collision evidence
  -> GameplayRuleRuntimeExecutionReport
  -> project_e2e_gate ComplexShooter wrapper report
```

本轮只做 P0-2 C-min：玩家移动、开火生成子弹、linear motion、lifetime cleanup、碰撞证据、score / hp 状态变化证据。

本轮不做完整 Rule Graph UI、完整 RuntimeEventQueue、IR runtime interpreter、动态 DLL 热加载、完整 generated rules crate build、HUD 真实绑定、Windows golden。

## 2. 涉及文件

预计修改：

```text
rust/crates/engine_runtime/src/project_logic.rs
rust/crates/engine_runtime/src/rule_registry.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/engine_runtime/src/logic_executor.rs
rust/crates/engine_runtime/src/runtime_trace.rs
rust/crates/engine_runtime/src/complex_project_validation.rs
rust/crates/engine_runtime/src/lib.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/project_e2e_gate/src/*.rs
samples/complex_shooter_project/Rules/*.json
samples/complex_shooter_project/Prefabs/*.json
samples/complex_shooter_project/Scenes/Main.scene.json
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-09-Complex-Shooter-Gameplay-Rule-Runtime-Execution-v1/00-总览.md
```

如施工中能收敛到更少文件，以最小实际改动为准。

## 3. 施工约束

```text
1. RuntimeRuleManifest 是规则执行计划来源；Static Registry 只提供函数指针。
2. manifest 引用但 registry 缺失时必须输出 missing_registered_rule，不允许静默成功。
3. 不把 Player / Enemy / Bullet / Score / Health / Weapon 写进 engine_runtime 通用 API。
4. 复杂打飞机专用逻辑只能在 sample / project_e2e_gate / 项目侧 static rule placeholder 中出现。
5. 不扩 IR 成脚本语言；本轮 C-min static rules 是 project-side generated/static placeholder。
6. action.fire 不走 EventHandler/EventReceived；必须改为 Update + action_pressed("action.fire")。
7. collision_response 如果作为 Project Rule 执行，必须通过 RuntimeRuleManifest 的通用 PostPhysics phase。
8. FrameLoop 接 RuntimeCommandContext 时必须保留 RuntimeTime 的 delta_time / fixed_delta_time / in_fixed_step。
9. engine_runtime 通用层只能新增 GameplayRuleRuntimeExecutionReport；ComplexShooter 专用 report 只能在 project_e2e_gate / sample wrapper。
10. Report / trace 必须区分 runtime/editor 和 Off/Summary/Trace 思路；默认 runtime 不常驻长 trace。
```

外部审查输入：

```text
本轮未收到 229 外部 AI 审查文档。
施工依据为 229 正式方案与 2026-07-09 自审补充。
```

## 4. Gate A：Rule Runtime Bootstrapping

任务：

```text
1. 保持 RuleModuleRegistry::build_runner 由 RuntimeRuleManifest 构建 RuleExecutionPlan。
2. 扩展 RuntimeRulePhase 支持 PostPhysics。
3. runtime_player_winit / headless gate 能从 package.rules + project static registry 构建 ProjectLogicRunner。
4. 构建失败时把 RuleRegistryError 转为结构化 diagnostic/report，不伪装为 logic ok。
5. manifest disabled rule 不执行，并在 report/trace 中能说明 skipped/disabled。
```

测试：

```text
cargo test -p engine_runtime rule_registry
cargo test -p engine_runtime m2_rule_demo
cargo test -p runtime_player_winit rule_runtime
```

## 5. Gate B：RuntimeCommandContext 接入 FrameLoop

任务：

```text
1. 给 FrameLoop / EngineHostLoop 增加可选 RuntimeFrameContext 或等价输入。
2. tick runtime 时，ProjectLogicRunner 使用 with_runtime 路径应用 InstantiatePrefab / DespawnPrefabInstance。
3. with_runtime 路径仍传入当前帧 TimeContext，不能退回 DEFAULT_FIXED_DELTA_TIME。
4. runtime_player_winit 持有 RuntimeInstanceLoader 并把 RuntimePackage context 传入 host tick。
```

测试：

```text
cargo test -p engine_runtime frame_loop
cargo test -p engine_runtime project_logic
cargo test -p runtime_player_winit rule_runtime
```

## 6. Gate C：Complex Shooter Project Static Rules C-min

任务：

```text
1. 新增复杂打飞机 sample 的 project-side static registry placeholder。
2. 注册 rule.player-move / rule.fire-bullet / rule.linear-motion / rule.lifetime-cleanup。
3. sample Rules/rule-manifest.json 把 rule.fire-bullet 改为 Update。
4. sample rule asset 的 phase / sourceMap 与 manifest 保持一致。
5. player_bullet / enemy_scout / Main.scene 补齐 C-min 所需 SpriteRenderer2D / Collider2D / project.linearMotion / project.lifetime / project.combatState / project.sessionState。
```

测试：

```text
cargo test -p project_e2e_gate complex_shooter_gameplay_rule_runtime
cargo test -p engine_runtime project_logic
```

## 7. Gate D：Collision / Score / Health Evidence

任务：

```text
1. ProjectLogicRunner 支持 PostPhysics phase，FrameLoop 在 Physics2D build_collision_pairs 后调用。
2. LogicContext 只读暴露 CollisionPairSnapshot，不暴露 Physics2DWorld 内部可变对象。
3. collision_response 通过 generic dynamic component 写 project.sessionState.score 和 project.combatState.hp。
4. 不新增任何打飞机专用 engine API。
```

测试：

```text
cargo test -p engine_runtime frame_loop
cargo test -p engine_runtime project_logic
cargo test -p project_e2e_gate complex_shooter_gameplay_rule_runtime
```

## 8. Gate E：Report / Regression Gate

任务：

```text
1. engine_runtime 新增通用 GameplayRuleRuntimeExecutionReport 或等价 summary。
2. project_e2e_gate 生成 ComplexShooterGameplayRuntimeExecutionReport wrapper。
3. report 至少包含 rule manifest summary、registered/missing rule count、input evidence、command summary、spawned prefab summary、collision summary、project state summary、diagnostics。
4. complex_project_validation 关闭 project_rule_pipeline_not_connected gap，或改成更准确 remaining gap。
```

测试：

```text
cargo test -p engine_runtime complex_project_validation
cargo test -p project_e2e_gate complex_shooter_gameplay_rule_runtime
cargo test -p project_e2e_gate
```

## 9. 整体回归

全部 Gate 完成后执行：

```text
cargo fmt --check
cargo test -p engine_runtime rule_registry
cargo test -p engine_runtime project_logic
cargo test -p engine_runtime frame_loop
cargo test -p engine_runtime complex_project_validation
cargo test -p runtime_player_winit
cargo test -p project_e2e_gate complex_shooter_gameplay_rule_runtime
cargo test -p project_e2e_gate
```

如真实窗口 / GPU local-only smoke 被涉及，必须保持 ignored/local-only，不阻塞默认回归。

## 10. 文档同步与归档

施工完成且整体回归通过后：

```text
1. 生成阶段完成记录：
   框架设计/引擎总体架构/阶段完成记录/2026-07-09-Complex-Shooter-Gameplay-Rule-Runtime-Execution-v1/00-总览.md
2. 更新：
   - 49-当前状态与下一步入口.md
   - 54-当前可自动化施工总入口.md
   - 施工文档/README.md
   - 阶段完成记录/README.md
   - 227 优先级文档中 P0-2 状态
3. 把本文档从 `施工文档/当前/` 移动到 `施工文档/已完成/`。
```

## 11. 施工文档自审

```text
是否严格对应 229 方案：
  是。只做 C-min：manifest-gated Rust AOT static registry 进入真实 runtime。

是否扩大范围：
  否。不做完整 RuntimeEventQueue、IR VM、热加载、Rule Graph UI、Windows golden 或 HUD 真实绑定。

是否继承 229 自审补充：
  是。fire-bullet 改 Update；collision_response 不走隐藏 hook；with_runtime 保留 TimeContext；通用 report 与 sample wrapper 分层；health 统一 project.combatState.hp。

是否保持 RuntimePackage 真相：
  是。runtime_player_winit 从 RuntimePackage.rules 和已链接 static registry 构建 runner，不扫描项目源目录。

是否避免 engine core 污染：
  是。engine_runtime 只新增通用 phase/context/report 能力；复杂打飞机语义只允许在 sample/project_e2e_gate。

是否 AI-first 可验证：
  是。每个 Gate 都有 cargo test，report 要能说明 source path / domain / stage / diagnostics。

是否处理默认 runtime 性能：
  是。report 默认 summary，完整 trace 只用于 gate/debug。

是否施工一次只执行一份文档：
  是。当前/ 目录此前为空，本文是当前唯一施工文档。
```
