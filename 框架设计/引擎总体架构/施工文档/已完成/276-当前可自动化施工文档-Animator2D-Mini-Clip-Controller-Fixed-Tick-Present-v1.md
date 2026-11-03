# 276 当前可自动化施工文档：Animator2D Mini Clip / Controller / Fixed-Tick Present v1

> 状态：Window A-C / Gate A-F 已完成，文档已归档到 `施工文档/已完成/`。
> 正式方案：`../../276-Animator2D-Mini-Clip-Controller-Fixed-Tick-Present-v1方案.md`。
> 方案选择：C / 3-Mini。
> 生成日期：2026-08-07。

## 1. 当前授权与执行边界

Window A-C / Gate A-F 已在分窗授权下完成。Window C 只施工 Editor authoring/preview、只读 Play
observation、generic/second-project integration、受影响源码回归与文档闭环；没有进入 Tower 或发布验证。
归档后的状态为：

```text
276 无继续施工授权
Tower P1-2 保持未激活
274 v2-B/C/D 保持未授权
不可自动运行 Local CI、真实 Editor 或 production/安装态替换
不可自动激活 Tower P1-2 或其它项目 Gate
```

## 2. 施工目标

施工通用引擎 `Animator2D Mini` 的真实完整链路：

```text
SpriteAnimationClip2D / AnimatorController2D schema Assets
  -> ProjectRuntimePackageAssembler validation + cook
  -> CookedAnimator2DRegistry in RuntimePackage
  -> Animator2D scene component hydration
  -> Animator2DModule load / apply / tick
  -> SpriteRenderer2D.sprite_ref
  -> existing RenderProjectionAdapter<SpriteRenderer2D>
  -> existing AssetProjection / Sprite2D renderer / GPU present
```

完成后，引擎应支持：

1. 离散 Sprite 帧、正整数 `durationTicks`、Loop / Once；
2. 单层 Controller、Entry State、State / Transition；
3. Bool / Trigger 参数和 `immediate | clip_end` 转换；
4. 显式 priority + stable transition ID 决胜，每 fixed tick 最多一次转换；
5. `speedPermille` fixed-point 播放；
6. `SetBool / SetTrigger / ResetTrigger` typed project intent；
7. Pause 不推进、StepFrame 精确推进一个 fixed tick；
8. Editor Clip/Controller/Component authoring、最小 Preview 和 Play Mode 只读状态；
9. RuntimePackage、Editor Play 和 exported Player 使用同一 cooked 数据与 evaluator；
10. `Off / Summary / Trace` 分档诊断。

## 3. 非目标与禁止范围

本文不施工：

- Tower 的角色动画资产、10 轮、四类兵种、五类敌人、Boss 或任何 gameplay；
- 骨骼、蒙皮、Avatar、Retarget、Root Motion、IK；
- 任意属性曲线、Layer、Blend Tree、Sub-State Machine、Any State、Crossfade；
- animation event 驱动 gameplay、Timeline、音频轨；
- Int/Float 参数、脚本 condition、Exit Time 百分比；
- 反向播放、运行时 controller override、完整可编辑节点图；
- GPU atlas packer、texture streaming 或 Sprite Renderer 重构；
- 第二条 Animator render bridge、第二套 texture/GPU present；
- 通用 EventBus、Workflow engine、Logic Ownership Router 或 Architecture Guard；
- production/安装态 Editor replacement、真实用户配置修改和 Local CI。

如果实现需要上述任一能力，停止当前 Gate，先修订正式方案和本文，不得以“顺手补齐”扩大范围。

## 4. 当前代码与工作树基线

文档生成时：

```text
repository HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
active named construction document: 0
pending named construction document before generation: 0
Animator2D implementation: absent
```

工作树高度 dirty，且 `engine_runtime`、`editor_core`、`editor_ui_model`、`editor_window_winit`、
`runtime_player_winit` 与 `project_e2e_gate` 均存在前序用户/施工变化，部分正式代码还是 untracked。
因此 HEAD 不是当前产品实现的完整基线。本文不回退、不覆盖这些变化；激活前必须按实际 working tree
重新读取 owner/consumer，并检查每个拟修改文件的重叠 diff。

当前确认的实现基线：

- `engine_runtime::SpriteRenderer2D` 只有静态 `sprite_ref` 等表现字段；
- `RuntimeSceneEntity` 有 typed `spriteRenderer2D` 和动态 project components；
- `ProjectRuntimePackageAssembler` 通过 `split_sprite_component` / `sprite_component_to_runtime` cook Sprite；
- RuntimePackage load、scene hydration 与 `RenderProjectionAdapter<SpriteRenderer2D>` 已存在；
- Project RuntimeModule 只读 World，通过 `ProjectRuntimeMutationBuffer` 提交 mutation；
- `WorldWriteApi` 支持动态字段写入，但没有 Animator2D typed intent；
- Editor 已有 Inspector、Asset Browser、authoring service 和 Preview Package 基础，不得再造平行 Editor 框架。

## 5. Ownership 与不可变接口

### 5.1 唯一 owner

- `ProjectRuntimePackageAssembler`：authoring schema parse、跨资产引用校验、cook 与 package 输入；
- `CookedAnimator2DRegistry`：RuntimePackage 内 immutable clips/controllers 真相；
- `Animator2DModule`：实例内存、parameter、trigger、state、cursor、transition 与 fixed-tick 求值唯一 owner；
- `ProjectRuntimeMutationBuffer`：项目 typed animation intent 的 batch/commit owner；
- `SpriteRenderer2D`：最终离散 Sprite 表现值；不拥有动画状态机；
- Editor authoring service/model：资产编辑与 preview session owner；不复制 evaluator。

### 5.2 Public seam

实现应保持概念接口：

```text
Animator2DModule::load(cooked_registry)
Animator2DModule::apply(command_batch)
Animator2DModule::tick(world, fixed_time_context, report_level)
  -> Animator2DFrameResult
```

项目 public intent 只允许：

```text
Animator2DCommand::SetBool
Animator2DCommand::SetTrigger
Animator2DCommand::ResetTrigger
```

禁止公开 `PlayClip / ForceState / SetFrame`，禁止项目直接修改 clip cursor、state memory、trigger storage、
`RenderProxy` 或 GPU handle。

## 6. 预计涉及文件

激活时可根据实际模块边界微调，但不得改变 ownership 或扩大 scope。

### 6.1 engine_runtime

预计新增：

```text
rust/crates/engine_runtime/src/animator2d/mod.rs
rust/crates/engine_runtime/src/animator2d/schema.rs
rust/crates/engine_runtime/src/animator2d/evaluator.rs
rust/crates/engine_runtime/src/animator2d/diagnostics.rs
rust/crates/engine_runtime/src/animator2d/tests.rs
```

预计修改：

```text
rust/crates/engine_runtime/src/lib.rs
rust/crates/engine_runtime/src/components.rs
rust/crates/engine_runtime/src/component_value.rs
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/runtime_entity_hydration.rs
rust/crates/engine_runtime/src/runtime_scene_hydration.rs
rust/crates/engine_runtime/src/project_runtime_session.rs
rust/crates/engine_runtime/src/project_runtime_session/tests.rs
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
```

只有实际 load/session ownership 需要时才修改 `engine_host_loop.rs` 或 `runtime_instance_loader.rs`；不能为了
方便把 Animator2D 再接成独立 Bridge。

### 6.2 editor_core / editor_ui_model / editor_window_winit

预计新增：

```text
rust/crates/editor_core/src/animator2d_authoring.rs
rust/crates/editor_core/src/animator2d_cooker.rs
rust/crates/editor_core/src/tests/animator2d_authoring_tests.rs
rust/crates/editor_ui_model/src/animator2d.rs
```

预计修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/asset_browser.rs
rust/crates/editor_core/src/inspector_details.rs
rust/crates/editor_core/src/authoring_workspace.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_model/src/model.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/inspector.rs
rust/crates/editor_window_winit/src/command_system.rs
rust/crates/editor_window_winit/src/application.rs
```

Editor 变更必须复用既有 command/session/model/composer/renderer 链；不新增另一套 UI toolkit 或 asset database。

### 6.3 Consumer tests / fixtures

预计修改或新增：

```text
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/project_e2e_gate/src/second_project_runtime.rs
run-owned or repository test fixture for generic Animator2D Mini
```

如果 `runtime_player_winit` 无需产品代码修改，只增加/复用测试即可，不制造空改动。Tower 项目目录不在
本文允许范围内。

## 7. 施工窗口与三小时边界

按 owner 分成三个可独立收口的施工窗口：

```text
Window A: Gate A-B，schema/cook + RuntimePackage/hydration
Window B: Gate C-D，evaluator + typed intent/fixed-tick scheduling
Window C: Gate E-F，Editor authoring/preview + affected integration/docs
```

每个连续施工授权最多三小时。保守估计不允许一次授权跨越全部 A-F；每个窗口结束后应报告实际状态，
下一窗口需要新的明确授权。2 小时 45 分后不开始新实现或验证，3 小时停止并记录
`stopped_by_time_limit`。窗口划分不改变 Gate 依赖顺序，也不允许并行修改共享 World/RuntimePackage 状态。

## 8. Gate A：Schema、Cooked Registry 与 Authoring Validation

### 8.1 任务

1. 建立 versioned `SpriteAnimationClip2D`、`AnimatorController2D`、`Animator2D` authoring contract。
2. 建立 cooked clip/controller/parameter/state/transition registry，字符串引用解析为稳定 cooked identity/index。
3. 拒绝未知 schema/field、duplicate ID、空帧、zero duration、missing entry/state/clip/sprite、参数类型失配、
   非法 speed、非法 transition timing。
4. 明确 canonical ordering/digest，禁止依赖 HashMap 或文件枚举顺序。
5. 只支持正式方案列出的 Mini enum，不预埋 deferred enum variant。

### 8.2 Red-capable owner tests

激活后先让不存在能力的测试失败，再实现：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime animator2d_schema_ -- --nocapture
cargo test -p editor_core animator2d_cook_ -- --nocapture
```

至少覆盖合法 round-trip、unknown schema/field、空帧、zero duration、duplicate ID、missing reference、
condition/parameter kind mismatch、priority/ID stable order、permuted input same digest、semantic change different digest。

### 8.3 通过条件

- schema-first 且 fail closed；
- authoring data 与 cooked immutable registry 分离；
- Runtime 不需要项目 path 才能解释 registry；
- 没有 gameplay、Tower 或 render pipeline 字段进入 contract。

## 9. Gate B：RuntimePackage、Component 与 Hydration

### 9.1 任务

1. `ProjectRuntimePackageAssembler` 发现动画资产并使用 Gate A cooker；它仍是唯一项目装配入口。
2. RuntimePackage manifest/digest 覆盖 cooked registry，build/load round-trip 字节稳定。
3. `RuntimeSceneEntity` 增加 typed Animator2D component，不把它留作无验证 dynamic blob。
4. Hydration 将 Animator2D 与 SpriteRenderer2D 写入 ECS；缺失 SpriteRenderer2D 结构化失败，不隐式创建。
5. generation replacement / Stop / despawn 所需 identity 随 package/session 生命周期传递，但 hydration 不推进 tick。

### 9.2 Red-capable owner tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core animator2d_package_ -- --nocapture
cargo test -p engine_runtime animator2d_package_ -- --nocapture
cargo test -p engine_runtime animator2d_hydration_ -- --nocapture
```

至少覆盖 assemble/load/hydrate round-trip、manifest/digest、missing registry/controller/renderer、generation identity、
Runtime 不扫描源目录、hydration 后 entry frame 尚未被多推进。

### 9.3 通过条件

- Preview/Play/Export 可消费同一 RuntimePackage 数据；
- 无第二个项目导出桥；
- 错误引用不以空 controller 或默认动画伪装成功；
- 现有静态 Sprite2D package/hydration 行为不回归。

Window A 到此结束。Gate A-B 通过只证明合同与 package 链，不证明运行求值或 Editor 产品面。

## 10. Gate C：Animator2DModule Deterministic Evaluator

### 10.1 任务

1. 实现深 `Animator2DModule::load/apply/tick` 和 per-entity private memory。
2. attach 首 tick 输出 entry 首帧，不隐式推进；Loop/Once 按 `durationTicks` 推进。
3. `speedPermille` 使用整数 accumulator，高倍速逐边界处理，不使用 wall-clock/f32 time。
4. 先决胜 immediate；未转换才推进；跨越结束边界后决胜 clip_end；每 fixed tick 至多一次转换。
5. priority 降序、stable transition ID 升序；Trigger 只由真正胜出的 transition 消费。
6. reconcile attach/despawn/disabled/controller generation replacement；退休旧 memory。
7. 最终只写 `SpriteRenderer2D.sprite_ref`，复用 World dirty/projection 链。
8. 实现 Off/Summary/Trace；production Off/Summary 不构造逐实体长 trace。

### 10.2 Red-capable owner tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime animator2d_evaluator_ -- --nocapture
cargo test -p engine_runtime animator2d_transition_ -- --nocapture
cargo test -p engine_runtime animator2d_lifecycle_ -- --nocapture
cargo test -p engine_runtime animator2d_diagnostics_ -- --nocapture
```

测试矩阵至少包含：

```text
entry first frame
duration boundary before/after
loop wrap / once hold
speed 500 / 1000 / 2500 permille
immediate before advance
clip_end on actual boundary only
priority then stable-id tie break
one transition per fixed tick
winning trigger consumed exactly once
losing trigger retained
bool true/false
missing entity/controller/clip/renderer
disable / despawn / generation replacement
same registry + commands + ticks -> same frame/state/report
Off/Summary no per-entity Trace allocation
```

### 10.3 通过条件

- evaluator 不依赖 wall-clock、render FPS 或 map iteration；
- runtime state 不泄漏到 public component/schema；
- 不新增 RenderProjectionAdapter 或 Animator Bridge；
- existing SpriteRenderer2D dirty/projection 能观察帧变化。

## 11. Gate D：Typed Project Intent 与 Fixed-Tick 调度

### 11.1 任务

1. 在 `ProjectRuntimeMutationBuffer` 增加有序 Animator2D command batch，保持 existing World mutation 语义。
2. Project RuntimeModule 暴露窄 typed sink/helper，只能 SetBool/SetTrigger/ResetTrigger。
3. command 在 project/physics/post-physics mutation 全部提交后应用，在 RenderExtract 前 tick。
4. Pause/non-advancing frame 不 apply/consume/advance；StepFrame 只执行一个正式 fixed tick。
5. 有界 catch-up 中每个实际 fixed tick 恰好求值一次；丢弃的 wall-clock 债务不伪造 tick。
6. invalid entity/parameter/generation 结构化 fail closed，不降级为动态字段写入。
7. Frame result 向 host/runtime report 提供 compact Summary，Trace 仅显式 scope。

### 11.2 Red-capable owner/consumer tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime animator2d_intent_ -- --nocapture
cargo test -p engine_runtime animator2d_schedule_ -- --nocapture
cargo test -p engine_runtime project_runtime_session::tests::animator2d_ -- --nocapture
cargo test -p engine_runtime animator2d_projection_ -- --nocapture
```

至少覆盖 same-tick project intent -> Animator -> RenderExtract、command stable order、invalid parameter、Pause、
StepFrame、two fixed ticks、bounded catch-up、Stop/session replacement、Sprite RenderProjection changed/unchanged dirty。

### 11.3 通过条件

- 调度顺序有可失败测试，不靠文字约定；
- gameplay 不能绕过 typed seam；
- Renderer、Physics 和 AUI owner 未被 Animator2D 侵入；
- project rule 与 animation present 在同一 fixed tick 语义闭环。

Window B 到此结束。Gate C-D 通过后才能进入 Editor authoring；不得先用 Editor 专用算法伪造 Preview。

## 12. Gate E：Editor Authoring、Preview 与 Runtime Observation

### 12.1 任务

1. Clip Inspector：frame list、Sprite picker、durationTicks、Loop/Once、增删与稳定排序。
2. Controller Inspector：parameter/state/transition table、entry、condition、priority 与引用 picker。
3. Animator2D Component Inspector：controller、enabled、initial bools。
4. Command -> Session -> Service -> saved schema 链支持 create/open/edit/validate/save/reload，禁止 UI 直接写文件。
5. 最小 Preview 支持 Play/Pause/Restart/StepTick/current state/frame；调用同一 evaluator 的隔离实例。
6. Play Mode 只读显示 current state/clip/frame、parameter/trigger 摘要和最近诊断。
7. Preview session 与 Play session 隔离；关闭/切换资产时释放 preview memory。
8. 不做完整节点 Graph；如显示关系，只能是 schema 派生只读视图。

### 12.2 Red-capable owner/consumer tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core animator2d_authoring_ -- --nocapture
cargo test -p editor_core animator2d_preview_ -- --nocapture
cargo test -p editor_ui_model animator2d_ -- --nocapture
cargo test -p editor_window_winit animator2d_ -- --nocapture
```

至少覆盖 command routing、dirty/save/reload、invalid edit diagnostics、stable reorder、picker refs、preview
Play/Pause/Restart/Step、preview/play isolation、Play Mode read-only、关闭释放、无 editable graph command。

### 12.3 通过条件

- 用户无需手写 JSON；
- Preview 和 Runtime 使用同一状态转换语义；
- Editor 不拥有第二份 animation truth；
- 不修改 production binary 或真实 Editor 配置。

## 13. Gate F：Affected Integration、Second Project 与文档闭环

### 13.1 冻结前 scoped review

确认最终 diff 只触及第 6 节 owner/consumer，且不存在：

```text
Tower hardcode or Tower asset
Animator2D Bridge / second render path
direct RenderProxy or GPU write
gameplay animation event
wall-clock animation time
editable full graph
deferred feature public stub
Local CI / production / installed / real-config mutation
```

### 13.2 Generic end-to-end consumer

用项目无关 fixture 证明：

```text
schema Assets
  -> assemble/cook/package/load/hydrate
  -> typed SetTrigger
  -> fixed ticks
  -> deterministic SpriteRenderer2D frame sequence
  -> existing RenderProjection consumer
```

再用第二个非 Tower fixture/controller 证明引擎没有依赖特定 asset ID、状态名或 gameplay。Headless sequence
是本施工的 Integration claim；Tower 竖屏真实视觉、production Editor 和 exported Windows screenshot
属于后续项目/发布 claim，不在本文重复。

### 13.3 Authoritative affected suites

Gate A-E filters 是开发反馈。最终代码冻结后运行一次受影响 closure；同一配置下完整 crate suite 已覆盖的
filter 不再机械重跑。

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --all --check
cargo test -p engine_runtime
cargo test -p editor_core
cargo test -p editor_ui_model
cargo test -p editor_window_winit --lib
cargo test -p runtime_player_winit
cargo test -p project_e2e_gate
```

不运行 workspace default/all-features、EngineStrict、Local CI、真实 Editor、production replacement、真实配置、
Tower Gate 或 Windows release matrix，因为本文只声明 source-level affected Integration。若激活时实际改到
feature-gated/platform-only owner，必须先修订本文并增加对应 variant，不能用 default suite冒充覆盖。

### 13.4 通过条件

- generic + second-project consumer 都通过；
- owner 与受影响 crates 全绿；
- schema/package/runtime/editor/report identity 可追溯；
- 没有未解释 warning、ignored fatal diagnostic 或与 276 不相关的修复。

## 14. 失败处理与停止条件

1. 保存首个失败 assertion/diagnostic、fixture、stdout/stderr 和相关 identity；只记录 test name/exit code 无效。
2. 使用当前 Gate 最小 owner reproduction 修复，不用完整 affected suite 或 Local CI 当调试循环。
3. 共享 schema/interface 修复后重跑当前 owner test；只更新被该修复失效的 consumer evidence。
4. 若失败证明正式方案错误、需要 deferred feature、第二渲染链或 Tower gameplay，立即停止并回填方案。
5. 若碰到前序 dirty worktree 重叠，读取并保留用户变化；不得 reset、checkout、revert 或覆盖。
6. 上游失败使后续 claim 不成立时 fail-fast，不跨 Gate 堆积失败。
7. 每个窗口遵守 2:45/3:00 停止线；超时保留证据并报告，不自动进入下一窗口。

## 15. 外部状态与清理

Gate A-F 正常施工只使用 workspace 源码、Cargo target 和测试框架的 run-owned temp fixture。本文不授权：

- 启动普通或 production Editor；
- 替换 production/安装态二进制；
- 修改真实用户配置；
- 创建或清理 Tower fresh root；
- 递归删除 workspace、Cargo target、历史 evidence 或共享 run root。

测试临时目录应由测试 owner 自动清理。失败时先保留最小复现；任何递归清理仍须重新证明 literal target 是
声明的 repository-external role root 严格子目录，并遵循 `game-engine-construction` strict-child 规则。本文本身
不提供递归删除授权。

## 16. 完成后的文档与归档动作

只有 Gate A-F 全部通过后：

1. 生成 `阶段完成记录/2026-??-??-Animator2D-Mini-Clip-Controller-Fixed-Tick-Present-v1/00-总览.md`；
2. 把本文状态改为已完成，并从 `施工文档/当前/` 移到 `施工文档/已完成/`；
3. 更新 276 正式方案的施工状态，但不把 deferred 能力改成已实现；
4. 更新 `00-文档地图.md`、`00-阅读顺序.md`、`49`、`54`、`施工文档/README.md`、
   `施工文档/待执行/README.md` 与 `阶段完成记录/README.md`；
5. 记录源码、fixture、test harness 与报告 identity，区分 dirty development evidence 和最终 affected evidence；
6. 保持 Tower P1-2/角色动画未激活；后续 Tower consumer 必须另立项目施工文档并单独授权。

Gate F 完成不自动授权 Local CI、production replacement、真实配置、Tower 竖屏视觉 Gate 或 Animator2D v2。

## 17. 激活前复核清单

激活时必须逐项重新执行，只要一项失败，本文留在待执行并修订、自审：

1. 用户明确授权激活 276，并明确首个施工窗口范围；
2. `施工文档/当前/` 没有另一份活动施工文档；
3. 276 正式方案仍有效，没有新审查要求先修订方案；
4. 重新读取 `54`、当前源码 owner/consumer 与 Cargo package/test names；
5. 记录实际 source identity；不能只记录 HEAD，必须记录 relevant dirty/untracked inputs；
6. 对第 6 节每个拟修改文件检查现有 diff，确认可与用户变化合并；
7. 确认 Gate tests 仍能按 prefix/包名执行，若模块落位漂移则先修订命令；
8. 确认 Window A 的保守实现、测试、证据和清理能在三小时内完成；
9. 确认没有 Tower、Local CI、production/安装态或真实配置隐式授权；
10. 移动本文到 `当前/`，同步 `54` 为唯一活动文档，并再次自审状态措辞。

## 18. 施工文档自审

### 18.1 与正式方案是否一致

通过。Gate A-F 覆盖 schema/cook、RuntimePackage/hydration、深 evaluator、typed intent、fixed-tick 调度、
Editor authoring/preview、diagnostics 和 affected consumer；没有更改 276 的 Clip/Controller/Component 分层。

### 18.2 是否形成深 Module

通过。外部只见 cooked registry、typed command batch 和 frame result；cursor、state memory、trigger consumption、
transition candidate 与 lifecycle 保持在 `Animator2DModule` 内。输出只写现有 SpriteRenderer2D。

### 18.3 是否覆盖确定性红线

通过。duration 使用 tick、speed 使用整数 fixed-point、priority + stable ID 决胜、每 tick 最多一次 transition、
Trigger 消费点唯一、Pause/StepFrame 与 catch-up 都有 red-capable test。

### 18.4 是否扩大到完整 Animator

没有。骨骼、曲线、Blend Tree、Layer、Crossfade、事件、完整 Graph、反向播放和 atlas 全部禁止，且不建立 public stub。

### 18.5 是否保持 Rust + schema 两层规则

通过。schema 只表达 Clip/Controller/Component；Rust 拥有 cook、runtime evaluator 和 typed intent。项目 gameplay
不直接写 Renderer 或 Animator memory，也不由动画事件反向驱动玩法。

### 18.6 是否把 Tower 偷渡进引擎施工

没有。Gate F 使用 generic 与 second-project fixture；Tower 资产、玩法和竖屏视觉明确留给后续项目施工。

### 18.7 测试是否与 claim 匹配

通过。Gate A-E 使用 owner-level red-capable filters；Gate F 只在代码冻结后运行受影响 crate closure，并避免
同配置重复。没有用 Local CI、production 或真实窗口证明不需要它们的 source-level Integration claim。

### 18.8 是否覆盖所有可执行需求

通过。schema rejection、cook digest、package/hydration、帧推进、转换决胜、Trigger、lifecycle、调度、typed
intent、Editor save/reload/preview、second project 和 diagnostics 均有对应测试矩阵。

### 18.9 是否保护 dirty worktree 与外部状态

通过。本文明确 HEAD 不是完整基线，激活前要求逐文件检查重叠 diff；禁止 reset/revert。施工不需要真实配置、
production binary 或递归清理授权。

### 18.10 是否符合三小时限制

通过。施工按三个 owner 窗口划分，禁止一个连续授权跨越 A-F；每窗口保留证据/cleanup 预算与 2:45/3:00 停止线。

### 18.11 是否需要修改正式方案

无需修改设计内容。施工分解没有发现与 276 冲突的实现前提；只是把正式方案的 owner/consumer 和验证要求落为
可执行 Gate。当前没有针对 276 的其它 AI 审查输入。

### 18.12 是否可以直接施工

Window A-B 已完成；Window C 已取得单独授权。当前结论是：

```text
自审通过
状态：Window A / Gate A-B、Window B / Gate C-D 已完成；Window C / Gate E-F 已激活
施工授权：仅 Window C / Gate E-F
下一步：执行 Gate E-F，完成后停止并按第 16 节闭环
```

## 19. Window A 激活记录

```text
date: 2026-08-07
activation: passed
authorized scope: Window A / Gate A-B
source: 施工文档/待执行/276-当前可自动化施工文档-Animator2D-Mini-Clip-Controller-Fixed-Tick-Present-v1.md
destination: 施工文档/当前/276-当前可自动化施工文档-Animator2D-Mini-Clip-Controller-Fixed-Tick-Present-v1.md
active named construction documents after move: 1
pending named construction documents after move: 0
repository HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
working tree: dirty; relevant RuntimePackage/editor changes retained as current source input
implementation state: not started
tests run during activation: none
forbidden: Window B-C, Tower, Local CI, real Editor, production/installed replacement, real configuration
```

## 20. Window A / Gate A-B 完成记录

2026-08-07 已完成当前授权范围，本文继续留在 `施工文档/当前/`。本记录不是 276 总阶段完成记录，
不授权 Gate C-F。

### 20.1 Gate A

- 新增 strict/versioned `SpriteAnimationClip2DAsset`、`AnimatorController2DAsset` 与 cooked registry；
- authoring 引用在 cook 时解析为稳定 index，集合按 stable ID canonical ordering；
- registry 使用 canonical SHA-256 digest，并拒绝 schema/field、空帧、zero duration、duplicate ID、
  missing sprite/clip/state、非法 speed 与 condition/parameter kind mismatch；
- authoring schema 与 immutable cooked registry 已分离，runtime registry 不依赖项目 path。

### 20.2 Gate B

- `ProjectRuntimePackageAssembler` 从 `Animations/` 发现严格后缀的 Clip/Controller 资产并调用唯一 cooker；
- RuntimePackage assembly digest、`manifest.json` index 与 `animator2d/registry.json` 覆盖同一 registry digest；
- package load 只读取 manifest 指向的 cooked registry，旧 package 缺少 index 时使用显式 empty registry，不扫描项目源目录；
- `RuntimeEntity.animator2d`、`ComponentTypeId::animator2d`、typed archetype column 与 World getter 已建立；
- hydration 要求同实体 typed `SpriteRenderer2D`，缺失时结构化失败；hydration 只复制初始 identity/bool，
  不建立 evaluator memory，也不推进动画 tick；
- registry 文件缺失、controller identity 缺失/不匹配均 fail closed。

### 20.3 定向证据

```text
cargo test -p engine_runtime animator2d_schema_ -- --nocapture
  2 passed, 0 failed
cargo test -p editor_core animator2d_cook_ -- --nocapture
  6 passed, 0 failed
cargo test -p editor_core animator2d_package_ -- --nocapture
  1 passed, 0 failed
cargo test -p engine_runtime animator2d_package_ -- --nocapture
  1 passed, 0 failed
cargo test -p engine_runtime animator2d_hydration_ -- --nocapture
  1 passed, 0 failed
```

受影响 owner 回归：component registry、既有 minimal RuntimePackage build/load、既有 scene hydration 各
`1 passed, 0 failed`。未运行 Local CI、真实/production Editor、production/安装态替换、真实配置或 Tower。

### 20.4 停止点

```text
Window A: complete
Gate A-B: complete
Window B / Gate C-D: activated, not yet complete
Window C / Gate E-F: not authorized, not started
next authorized action: execute Gate C-D
```

## 21. Window B 激活记录

```text
activation date: 2026-08-07
authorized scope: Window B / Gate C-D
preflight: passed
active named construction documents: 1
pending named construction documents: 0
repository HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
baseline: Window A Gate A-B complete on preserved dirty worktree
forbidden: Window C, Tower, Local CI, real Editor, production/installed replacement, real configuration
```

## 22. Window B / Gate C-D 完成记录

2026-08-07 已完成当前授权范围并停止。本文继续留在 `施工文档/当前/`；本记录不是 276 总阶段完成记录，
不授权 Window C / Gate E-F。

### 22.1 Gate C

- 新增深 `Animator2DModule::load/apply/tick` 与私有 per-entity memory；
- 固定 tick 离散推进覆盖 entry 首帧、duration boundary、Loop/Once、500/1000/2500 permille；
- immediate 与 clip_end 严格按边界决胜，每 tick 至多一次转换，priority/stable ID 顺序稳定；
- Bool/Trigger、胜者 Trigger 消费、败者 Trigger 保留、disable/despawn/generation/registry replacement 均已闭环；
- Off/Summary/Trace 分档，Summary 诊断按 entity/code/generation 去重，最终仅写既有 `SpriteRenderer2D`。

### 22.2 Gate D

- `ProjectRuntimeMutationBuffer` 提供有序 `SetBool/SetTrigger/ResetTrigger` typed intent batch；
- World mutation 全批 commit 后才暴露 Animator2D command，preflight 失败时 command 不泄漏；
- FrameLoop 在 project/physics/post-physics 后、RenderExtract 前 apply/tick，同 tick Sprite 变化走既有 dirty/projection；
- 真实 `EngineHostMode::EditorPause -> EditorStep -> EditorPlay` owner test 证明 Pause 不 attach/消费，Step 恰好推进
  一个 fixed tick，随后第二个正式 fixed tick 连续计数；
- 大 delta 被 maximum delta 钳制且每次 host 调用最多执行一个正式 fixed tick，不按 wall-clock 债务伪造 tick；
  host/session replacement 从零实例内存开始，不继承旧 session 状态；
- 正常帧结束时刷新 `TimeTraceSummary`，host 输出的 `fixed_frame_count` 与已执行 fixed tick 一致。

### 22.3 定向证据

```text
cargo test -p engine_runtime animator2d_ -- --nocapture
  21 passed, 0 failed
cargo test -p engine_runtime project_runtime_session::tests:: -- --nocapture
  13 passed, 0 failed
cargo test -p engine_runtime runtime_time::tests:: -- --nocapture
  10 passed, 0 failed
cargo test -p engine_runtime runtime_frame_runs_logic_before_render_extract -- --nocapture
  1 passed, 0 failed
cargo test -p engine_runtime frame_loop_runs_physics2d_after_project_logic_before_render_extract -- --nocapture
  1 passed, 0 failed
cargo test -p engine_runtime sprite_renderer2d_ -- --nocapture
  7 passed, 0 failed
```

施工文档要求的 Gate C-D 八条筛选命令也已逐条通过。未运行 Local CI、真实/production Editor、
production/安装态替换、真实配置或 Tower；未进入 Window C。

### 22.4 停止点

```text
Window A / Gate A-B: complete
Window B / Gate C-D: complete
Window C / Gate E-F: not authorized, not started
document location: 施工文档/当前/
next authorized action: none; wait for explicit Window C authorization
```

## 23. Window C 激活记录

```text
activation date: 2026-08-07
authorized scope: Window C / Gate E-F
preflight: passed
active named construction documents: 1 (276; other files in 当前/ are historical 254 evidence)
pending named construction documents: 0
repository HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
baseline: Window A-B / Gate A-D complete on preserved dirty worktree
relevant existing inputs: untracked animator2d/project_runtime_session modules plus modified runtime/editor consumers
estimated window: within three hours including affected closure and document archive
forbidden: Tower, Local CI, real/production Editor, production/installed replacement, real configuration
```

## 24. Window C / Gate E-F 完成记录

### 24.1 Gate E

- 增加 Clip、Controller 与 Animator2D Component 的 typed authoring model/command/service；支持 create、open、
  edit、validate、atomic save 与 reload，活动资产身份显式保留 `{ path, asset_id }`；
- 最小隔离 Preview 直接复用正式 `Animator2DModule` evaluator，支持 Play/Pause/Restart/StepTick 与 Bool/Trigger；
- Editor Play 安装已验证的 cooked Animator2D registry，并在 GameView frame 暴露只读 state/clip/frame、参数、
  trigger 和诊断 observation；Stop/session replacement 清空 observation；
- 关系视图保持 schema 派生只读，不实现 editable/full graph；
- `editor_window_winit` headless reachability 证明 authoring controls 可达且不依赖真实 Editor。

### 24.2 Gate F

- `project_e2e_gate` 使用 Complex Shooter 与 Switch Puzzle 的 run-owned 临时副本，分别配置不同 controller、
  state、trigger 与 sprite ID，证明同一 `schema -> cook -> package -> load -> hydrate -> typed trigger ->
  fixed ticks -> SpriteRenderer2D -> RenderProjection` 链；fixture 未修改两个样例源项目，也未触碰 Tower；
- 修复 Editor GameView 未安装 RuntimePackage Animator2D registry 的 consumer 缺口；失败时输出结构化诊断；
- 修复 headless Player 未向既有 renderer 传入 cooked texture binding context 的 consumer 缺口；只复用
  `RuntimeTextureUploadRegistry`、`Sprite2DTextureBindingContext` 与 `RuntimeTextureBindingContext`，没有新增
  GPU upload/render path；
- 补齐 save/reload/rebuild assembly-domain 的 `Animator2D` witness mapping。

### 24.3 Authoritative affected evidence

```text
cargo fmt --all --check: passed
cargo test -p engine_runtime: 537 passed
cargo test -p editor_core: 646 passed, 2 ignored
cargo test -p editor_ui_model: 64 passed
cargo test -p editor_window_winit --lib: 180 passed, 1 ignored
cargo test -p runtime_player_winit: 19 passed
cargo test -p project_e2e_gate: library 67 passed, 2 ignored; integration tests 6 passed
```

Gate E 定向证据：authoring 3、preview 2、UI model 1、winit reachability 1 均通过。Gate F 双项目 Animator2D
E2E 与原先失败的 Complex Shooter desktop export owner reproduction 均通过；其后由上述完整受影响 suite
覆盖，未机械重复同配置定向测试。

### 24.4 Scoped review

最终扫描未发现 Tower hardcode/asset、Animator2D 第二渲染路径、直接 RenderProxy/GPU 写入、gameplay
animation event、wall-clock animation、editable full graph 或 deferred public stub。测试中的 `SystemTime`
只生成 run-owned 临时目录名；Player 的 `Instant` 只记录既有性能指标，不参与 Animator2D 求值。

## 25. 最终停止点

```text
Window A-C / Gate A-F: complete
construction document: archived
Tower P1-2: inactive
Local CI: not run
real/production Editor: not launched or replaced
production/installed binary: not replaced
real configuration: unchanged
next authorized action: none
```
