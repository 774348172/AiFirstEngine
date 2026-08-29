# 276-Animator2D Mini Clip / Controller / Fixed-Tick Present v1 方案

> 状态：正式方案，方案 C（3-Mini）已完成 Window A-C / Gate A-F 施工并归档。
> 日期：2026-08-07。
> 定位：通用引擎 2D 离散帧动画最小产品能力，不是 Tower 专用功能，也不是完整 Animator/Mecanim。

## 1. 问题与目标

当前引擎已经具备 Sprite2D authoring、RuntimePackage cook/hydration、ECS 到 RenderProxy 的
`RenderProjectionAdapter<SpriteRenderer2D>`、cooked texture resolve、GPU upload 与真实 present，
但 `SpriteRenderer2D` 只能持有单张 `sprite_ref`。项目若要实现待机、移动、攻击、受击、死亡等
离散帧动画，只能在项目规则中手工计时并直接改渲染字段。这会同时造成三类问题：

- 动画游标、循环和状态转换泄漏到每个项目，无法复用，也难以确定性验证；
- 项目 gameplay 规则与 Renderer 表现状态耦合，破坏“Rust 底层规则 + schema 上层规则”的边界；
- Editor Preview、Play、StepFrame 与导出 Player 容易使用不同时间源或不同实现。

本方案增加一个纯 2D 的 `Animator2D Mini`：以 schema 资产描述 Clip 和单层 Controller，以 Rust
深模块拥有固定 tick 播放、状态转换、trigger 消费和诊断，最终只更新现有
`SpriteRenderer2D.sprite_ref`，复用既有投影与 GPU present 链路。

首版必须能够：

1. 播放由离散 Sprite 帧组成的 Loop / Once Clip；
2. 使用单层 State / Transition Controller 在多个 Clip 间切换；
3. 接收 Bool / Trigger typed intent；
4. 在 Editor Play、Pause、StepFrame 和导出 Player 中按相同 fixed tick 得到相同帧；
5. 在 Editor 中完成资产编辑、最小预览和运行态观察；
6. 缺失资产、无效状态机或运行时失配必须结构化报告，不能静默降级。

## 2. Context Scan：当前实现基线

### 2.1 静态 Sprite2D 链路

当前真实 owner/consumer 为：

```text
Project Asset / Scene component
  -> editor_core::ProjectRuntimePackageAssembler
  -> engine_runtime::RuntimeSpriteRenderer2D
  -> runtime_entity_hydration
  -> ECS SpriteRenderer2D
  -> RenderProjectionAdapter<SpriteRenderer2D>
  -> RenderProxy / AssetProjection
  -> Sprite2D renderer / WGPU present
```

关键代码基线：

- `rust/crates/engine_runtime/src/components.rs`：`SpriteRenderer2D` 只拥有 sprite、材质、颜色、翻转、排序和可见性；
- `rust/crates/engine_runtime/src/runtime_package.rs`：`RuntimeSpriteRenderer2D` 是 cooked scene component；
- `rust/crates/editor_core/src/project_runtime_package_assembler.rs`：`split_sprite_component` 与 `sprite_component_to_runtime` 完成 authoring 到 RuntimePackage 的转换；
- `rust/crates/engine_runtime/src/runtime_entity_hydration.rs`：RuntimePackage scene entity hydration；
- `rust/crates/engine_runtime/src/render_extract.rs`：现有 `RenderProjectionAdapter<SpriteRenderer2D>`；
- `rust/crates/engine_runtime/src/frame_loop.rs`：project fixed update、project logic、physics、post-physics 后进入 RenderExtract；
- `rust/crates/engine_runtime/src/project_runtime_session.rs`：项目模块只读 World，通过 mutation buffer 提交写入；
- `rust/crates/engine_runtime/src/world_api.rs`：已有动态字段写入能力，但没有 Animator2D typed intent。

`139-Sprite2D-Product-Runtime-Rendering-v1方案.md` 已明确把 Sprite Animation 延期。因此 276
不能重构 Sprite Renderer，也不能创建第二条 animated-sprite render bridge；动画只应成为现有
Sprite2D consumer 之前的表现求值层。

### 2.2 调度与项目边界

项目玩法真相仍由 Project Rust RuntimeModule 与 Gameplay Rule Asset/RuleSlot 拥有。Animator2D
不能读取波次、生命、攻击目标等项目私有数据，也不能通过动画事件反向修改 gameplay。

Animator2D 必须在本 tick 的项目 mutation、physics 与 post-physics 均完成之后运行，并在
RenderExtract 之前把选定帧写入 SpriteRenderer2D。这样同一 tick 的 gameplay intent 可以影响
同一 present，Renderer 仍只观察已提交的 ECS 表现状态。

建议固定顺序：

```text
Project Runtime fixed update
  -> Project logic / mutation commit
  -> Physics2D
  -> Post-physics project commit
  -> Animator2DModule.apply(intent batch)
  -> Animator2DModule.tick(fixed tick)
  -> RenderExtract
  -> existing Sprite2D present
```

## 3. 外部实现参考

### 3.1 Unity 6 与 Unity 源码

参考：

- <https://docs.unity3d.com/6000.0/Documentation/Manual/class-AnimatorController.html>
- <https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationClips.html>
- `<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Animation\AnimatorController.cs`
- `<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\Animation\ScriptBindings\Animator.bindings.cs`
- `<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\Animation\ScriptBindings\AnimationClip.bindings.cs`
- `<UNITY_LEGACY_SOURCE>\Runtime\Animation`
- `<UNITY_LEGACY_SOURCE>\Runtime\mecanim`

可借鉴部分是 Clip、Controller、State、Transition 的职责分离，以及常量数据与每实例运行内存分离。
Unity 的 `SetBool` / `SetTrigger` 也证明参数化 intent 是稳定项目接口。Unity 4.3 的
`StateMachineConstant / StateConstant / TransitionConstant / ConditionConstant` 和
`EvaluateStateMachine(Constant, Input, Output, Memory, Workspace)` 进一步说明 cooked 常量、输入、输出和
实例内存不应混为一个对象。

不照搬 Mecanim、骨骼、Avatar、Root Motion、IK、Layer、Blend Tree 或任意属性曲线。

### 3.2 Godot

参考：

- <https://docs.godotengine.org/en/stable/tutorials/2d/2d_sprite_animation.html>
- <https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html>
- Godot source：`scene/2d/animated_sprite_2d.cpp`、`scene/resources/sprite_frames.cpp`、`scene/animation/animation_tree.cpp`

Godot 把轻量 `AnimatedSprite2D + SpriteFrames` 与复杂 `AnimationTree` 分开。`SpriteFrames` 持有帧、
时长和 loop，`AnimatedSprite2D` 持有播放状态。276 采用同样的最小能力边界，但把多个 Clip 的切换
收敛到一个单层 Controller，避免项目再次手写状态切换。

### 3.3 Bevy

参考：<https://bevy.org/examples/2d-rendering/sprite-animation/> 及 `examples/2d/sprite_animation.rs`。

Bevy 官方最小样例使用 `Timer + TextureAtlas.index` 推进帧，适合作为 fixed-tick 帧游标的参照，
但不提供 Controller、确定性 transition priority 或 trigger 语义，因此不能单独覆盖本项目需求。

## 4. 方案比较与结论

### 4.1 方案 A：项目侧 Sprite 帧播放器

由每个 Project RuntimeModule 保存 timer/index 并写 `SpriteRenderer2D.sprite_ref`。

优点是改动最小；缺点是算法重复、项目直接拥有 Renderer 写入、Editor Preview 无法复用、Controller
继续由玩法代码手写。它只能解决一次性样例，不能形成引擎产品能力，拒绝。

### 4.2 方案 B：AnimatedSprite2D 单组件

新增 `AnimatedSprite2D`，组件直接引用帧集并拥有 play/stop/loop，不提供 Controller。

它适合装饰性循环动画，接口很小；但待机/攻击/受击/死亡仍需项目切换 Clip，Bool/Trigger 和状态
转换缺失。后续补 Controller 时容易形成两套运行状态，暂不选择。

### 4.3 方案 C：Animator2D 3-Mini

分离 `SpriteAnimationClip2D`、`AnimatorController2D` 与 `Animator2D` Component；用单个深
`Animator2DModule` cook 后求值，最终写现有 SpriteRenderer2D。

这是选择方案。它比完整 Animator 小得多，但一次性形成稳定边界：项目只表达动画意图，Rust 模块
统一处理播放和转换，未来增加 crossfade/layer 时也不需要改变 Renderer 或项目 gameplay owner。

## 5. Ownership 与深模块接口

### 5.1 Ownership

- `SpriteAnimationClip2D Asset`：项目 schema 资产，拥有帧顺序、每帧 tick 时长和播放模式；
- `AnimatorController2D Asset`：项目 schema 资产，拥有参数声明、entry、state、transition 与稳定优先级；
- `Animator2D Component`：场景实体 authoring 配置，只引用 controller、初始参数和启用状态；
- `ProjectRuntimePackageAssembler`：校验引用并 cook 为运行时 registry，不执行动画；
- `Animator2DModule`：唯一运行 owner，拥有实例生命周期、参数、trigger、current state、clip cursor 和 fixed-tick 求值；
- `SpriteRenderer2D`：保持最终离散 sprite 表现值，不拥有动画语义；
- `RenderProjectionAdapter<SpriteRenderer2D>`：保持唯一 ECS 到渲染投影；
- Project RuntimeModule：只能提交 typed animation intent，不能写 Animator 内存或 RenderProxy。

### 5.2 深模块 public interface

概念接口收敛为：

```text
Animator2DModule::load(cooked_registry)
Animator2DModule::apply(command_batch)
Animator2DModule::tick(world, fixed_time_context, report_level)
  -> Animator2DFrameResult
```

`load` 接收 RuntimePackage 内完整且已验证的 immutable registry；`apply` 接收按稳定顺序排列的 typed
command；`tick` 只推进一个 fixed tick，并将最终 sprite 输出写入 World。外部不能访问或修改 clip
cursor、transition candidate、trigger consumption 与 entity state memory。

`Animator2DFrameResult` 只公开聚合事实，例如 evaluated/changed/failed entity 数、诊断和可选 trace，
不把内部 workspace 变成公共 API。

## 6. Authoring Schema

以下 JSON 仅表达合同形状；具体文件后缀和 serde 类型名在施工文档中依据现有 Asset Registry 规则落位。

### 6.1 SpriteAnimationClip2D

```json
{
  "schema": "sprite-animation-clip-2d.v1",
  "assetId": "tower/units/guard/idle",
  "playback": "loop",
  "frames": [
    { "spriteRef": "sprites/guard/idle_00", "durationTicks": 4 },
    { "spriteRef": "sprites/guard/idle_01", "durationTicks": 4 }
  ]
}
```

规则：

- `frames` 至少一帧，保持 authoring 顺序；
- `durationTicks` 为正整数，首版不接受秒数或浮点 duration；
- `playback` 仅 `loop | once`；Once 到末帧后保持末帧并标记 completed；
- 每个 `spriteRef` 必须在 cook 时解析，Runtime 不扫描项目目录；
- Clip 不包含 gameplay event、任意属性曲线、音频或 Transform track。

### 6.2 AnimatorController2D

```json
{
  "schema": "animator-controller-2d.v1",
  "assetId": "tower/units/guard/controller",
  "parameters": [
    { "id": "isMoving", "kind": "bool", "default": false },
    { "id": "attack", "kind": "trigger" }
  ],
  "entryStateId": "idle",
  "states": [
    { "id": "idle", "clipRef": "tower/units/guard/idle", "speedPermille": 1000 },
    { "id": "attack", "clipRef": "tower/units/guard/attack", "speedPermille": 1000 }
  ],
  "transitions": [
    {
      "id": "idle_to_attack",
      "from": "idle",
      "to": "attack",
      "when": "immediate",
      "priority": 100,
      "conditions": [{ "parameter": "attack", "op": "triggered" }]
    },
    {
      "id": "attack_to_idle",
      "from": "attack",
      "to": "idle",
      "when": "clip_end",
      "priority": 100,
      "conditions": []
    }
  ]
}
```

Mini 语义：

- 单层 Controller；每个 State 精确引用一个 Clip；
- 参数仅 Bool / Trigger；不加入 Int、Float、expression tree 或脚本 condition；
- Bool condition 仅 equals true/false，Trigger condition 仅 `triggered`；
- transition timing 仅 `immediate | clip_end`；
- 同一 tick 最多执行一次 transition，禁止 transition chain；
- 先按显式 `priority` 降序，再按稳定 transition ID 升序决胜；禁止依赖 map 遍历顺序；
- Trigger 只有在包含该 Trigger 的胜出 transition 真正执行后才消费一次；失败候选不得消费；
- 进入新 State 时 clip cursor 从首帧开始，新 State 首帧在同一 tick 成为 present 输出；
- `speedPermille` 是正 fixed-point 倍速，默认 1000；首版不支持 0、负数或反向播放。

### 6.3 Animator2D Component

```json
{
  "type": "Animator2D",
  "controllerRef": "tower/units/guard/controller",
  "enabled": true,
  "initialBools": { "isMoving": false }
}
```

实体必须同时存在 `SpriteRenderer2D`。缺失 Renderer 时 fail closed 并报告，不隐式创建组件。Controller
引用变化、entity despawn、Play Stop 或 RuntimePackage generation replacement 时，模块必须确定性退休旧实例内存。

## 7. RuntimePackage、Cook 与 Hydration

目标链路：

```text
Clip / Controller schema Assets + Animator2D scene component
  -> ProjectRuntimePackageAssembler validation/cook
  -> CookedAnimator2DRegistry
     - cooked clips
     - cooked controllers
     - resolved stable indices
  -> RuntimePackage load
  -> Animator2D component hydration
  -> Animator2DModule instance attach
```

Cook 阶段应把字符串引用解析为稳定 cooked identity/index，并完成跨资产校验：duplicate ID、missing entry、
missing state/clip/sprite、invalid condition type、zero duration、invalid speed、unreachable target 等。运行时不
重新解析项目源文件，也不因错误资产采用猜测性 fallback controller。

Authoring Asset 与 cooked registry 分离；RuntimePackage manifest/digest 必须覆盖动画 cooked payload，
从而保持 Preview、Play、Export identity 可比较。Hydration 只恢复声明组件，不能预先推进动画 tick。

## 8. Fixed-Tick 求值规则

Animator2D 使用引擎 fixed tick，不读取 wall-clock 或 present duration。确定性输入至少包含 session/generation、
fixed tick index 与有序 command batch。

每实例每 tick 顺序：

1. 应用本 tick typed commands；
2. 收集满足条件的 `immediate` transition，依据 priority + stable ID 选择至多一个；
3. 若 immediate 胜出，执行 transition、消费其 Trigger 并重置新 Clip cursor，本 tick 不再推进新 Clip；
4. 若没有 immediate transition，则向 fixed-point accumulator 加入 `speedPermille`，每累计 1000 消耗一个
   animation tick；允许高倍速在一个 fixed tick 内消耗多个 animation tick，但逐帧处理边界；
5. 若本 tick 的推进首次越过 Once 末尾或完成一个 Loop 周期边界，收集满足条件的 `clip_end` transition，
   仍按 priority + stable ID 选择至多一个；
6. 若 clip_end 胜出，执行 transition、消费其 Trigger 并重置新 Clip cursor；否则 Loop 回到首帧，Once 保持末帧；
7. 计算最终 Sprite frame，必要时写 `SpriteRenderer2D.sprite_ref`，再生成 Summary 或 Trace evidence。

`clip_end` 的含义是“本 fixed tick 的动画量实际跨过 clip 结束边界”，不是 Once 已完成后每个后续 tick 都
持续为真。一个 fixed tick 即使以高倍速跨过多个 loop 边界，也最多选择一次 clip_end transition；一旦执行
任意 transition，本 tick 不再求值第二条 transition，也不再推进新 State。

首个 attach tick 输出 entry state 首帧，不因 attach 隐式多推进一帧。Pause 时 fixed tick 不发生，因此动画
自然停止；Editor `StepFrame` 精确触发一个正式 fixed tick，不能维护 Editor 专用 preview clock。

若一个低帧率 present 触发引擎既有有界 catch-up，则 Animator2D 对每个实际执行的 fixed tick 恰好求值一次；
被引擎丢弃的 wall-clock 债务不能伪造动画 tick。

## 9. Project Typed Animation Intent

项目不能调用任意字段写入修改 Animator2D 内部状态。新增窄型 typed command：

```text
Animator2DCommand::SetBool { entity, parameter_id, value }
Animator2DCommand::SetTrigger { entity, parameter_id }
Animator2DCommand::ResetTrigger { entity, parameter_id }
```

commands 通过现有 Project Runtime mutation/commit 生命周期收集，并在 Animator2D 节点统一应用。批内顺序
必须稳定；目标 entity、parameter kind 或 controller generation 不匹配时产生诊断，不执行部分猜测。

`PlayClip`、`ForceState`、`SetFrame` 不进入 v1 public project interface，否则项目会绕过 Controller 并重新
拥有表现状态机。Editor Preview 可以使用 editor-owned preview control，但不能把该控制面暴露为 gameplay API。

## 10. Editor Authoring 与 Preview

首版产品面包含：

- Clip Asset Inspector：帧列表、Sprite picker、durationTicks、Loop/Once、增删与稳定排序；
- Controller Asset Inspector：参数表、entry state、state 表、transition 表、condition 与 priority 编辑；
- Animator2D Component Inspector：controller picker、enabled、initial bools；
- 最小 Preview：Play/Pause/Restart、逐 tick、当前帧/current state/elapsed tick；
- Play Mode 只读观察：current state、clip、frame、pending bool/trigger 摘要和最近诊断。

首版不做完整节点式 Animator Graph 编辑器。可以提供由 schema 派生的只读关系视图，但表格/Inspector 才是
authoring 真相。Preview 必须调用同一 Animator2D evaluator 或其无 World 的同语义实例，不复制一套 Editor
帧推进算法。Preview state 与真实 Play session state 严格隔离。

## 11. Diagnostics 与 Observation

报告等级：

```text
Off     仅保留 fatal aggregate，不构造逐实体明细
Summary 帧级计数与去重诊断，production 默认
Trace   指定实体/preview 的 transition candidate、决胜、frame cursor 与 trigger consumption
```

诊断代码至少覆盖：

```text
animator2d.controller_missing
animator2d.clip_missing
animator2d.sprite_missing
animator2d.entry_state_invalid
animator2d.transition_target_invalid
animator2d.condition_parameter_invalid
animator2d.renderer_missing
animator2d.command_entity_missing
animator2d.command_parameter_invalid
animator2d.runtime_generation_mismatch
```

生产默认不得每实体每帧写日志。相同静态错误按 session/generation/entity/code 去重；Trace 必须显式开启并
有实体或 preview scope。诊断包含 asset/entity/parameter identity 和 failure stage，但不泄漏内部 mutable pointer。

## 12. Validation Plan

本节定义未来施工验收边界，不代表本轮已授权执行。

### 12.1 Owner tests

- schema/cook：合法/非法 Clip、Controller、稳定排序、跨资产引用与 digest；
- evaluator：Loop、Once、durationTicks、speedPermille accumulator、entry 首帧；
- transition：Bool、Trigger、immediate、clip_end、priority/ID 决胜、每 tick 一次转换；
- trigger：仅胜出转换消费、Reset、未命中保留；
- lifecycle：attach/despawn、Stop、generation replacement、missing Renderer fail closed；
- determinism：相同 cooked registry + commands + tick sequence 得到相同 state/frame/report。

### 12.2 Consumer closure

- RuntimePackage assemble/load/hydration 能 round-trip Animator2D；
- Editor Clip/Controller/Component authoring save/reload 与 Preview 使用同一语义；
- Project RuntimeModule typed intent 在 commit 后、RenderExtract 前生效；
- existing Sprite RenderProjection / cooked texture / GPU present 无动画专用分支；
- Pause 不推进、StepFrame 推进一个 tick；Editor Play 与 exported Player 帧序列一致；
- second-project fixture 证明引擎无 Tower hardcode。

### 12.3 真实项目消费

Tower 可在单独项目施工中选取最小角色建立 idle/attack/death Clip 与 Controller，验证竖屏真实 GameView
和 windowed player。但 Tower visual gate 只能验证 consumer，不代替引擎 owner tests，也不得把 10 轮、
兵种、敌人或 Boss gameplay 实现并入 276。

验证应遵循 274 Construction Validation Plan v2 的 owner/consumer closure 和按风险升级规则；具体 Gate、
fresh root、是否运行 Local CI 或是否替换 production binary，必须在未来独立施工文档中逐项授权。

## 13. 明确延期

以下能力不进入 Animator2D Mini v1：

- 骨骼、蒙皮、Avatar、Retarget、Root Motion、IK；
- 任意 Transform/Color/Material/Shader 属性曲线；
- Blend Tree、Layer、Sub-State Machine、Any State、Crossfade；
- animation event 驱动 gameplay、Timeline、音频轨；
- Int/Float 参数、脚本表达式、Exit Time 百分比；
- 反向播放、随机播放、运行时 controller override；
- 完整可编辑节点图、曲线编辑器、录制模式；
- GPU atlas packer、运行时 atlas 重排或 texture streaming。

后续能力必须基于真实项目压力重新立项，不得预埋公开半成品接口。

## 14. Red Lines

- Animator2D 是表现系统，不是生命、攻击、波次或胜负状态的 gameplay truth；
- 项目规则只能提交 typed intent，不能直接写 RenderProxy、Animator memory 或帧游标；
- 动画事件不能反向改变 gameplay；
- 不新增 Animator2D Bridge，继续使用 `RenderProjectionAdapter<SpriteRenderer2D>`；
- Runtime 不扫描项目目录，只消费 RuntimePackage cooked registry；
- 不为 Tower 资产 ID、兵种、敌人、Boss 或竖屏尺寸写引擎分支；
- 不重构现有 SpriteRenderer2D，不创建第二套 texture/GPU present；
- 不把 Preview clock、wall-clock 或 render FPS 当作 runtime animation time；
- 本方案确认不等于施工授权，也不授权 Local CI、production/安装态替换或真实配置修改。

## 15. Tower Consumer 边界

Tower P1-1/P1-2 或后续 UI/角色动画工作可以消费 276，但项目层仍按两层规则组织：

```text
Rust Project Framework
  决定角色当前 gameplay 状态并提交 SetBool / SetTrigger

Project schema Assets
  定义 SpriteAnimationClip2D / AnimatorController2D / Animator2D component

Animator2D engine module
  将表现 intent 确定性求值为 SpriteRenderer2D.sprite_ref
```

10 轮、四类基础兵种、五类敌人和 Boss 的数值、AI、攻击与生成规则不属于 Animator2D。角色 Clip 和
Controller 资产也应在 Tower 项目施工文档中单独列出，不能由引擎方案暗含生成。

## 16. 方案自审

### 16.1 完整性

方案覆盖 authoring schema、cook、RuntimePackage、hydration、fixed-tick evaluator、项目 typed intent、
Editor authoring/preview、现有 render consumer、诊断和验证闭环。没有把“能播放帧”误当成 Controller、
Editor 和导出一致性已经自动成立。

### 16.2 深模块检查

公共接口只有 `load / apply / tick` 三类责任，复杂的游标、transition、trigger、实例 lifecycle 和 trace
均隐藏在 `Animator2DModule` 内；调用方不需要理解状态机内部数据结构。该模块通过复用现有
SpriteRenderer2D 输出减少了跨域接口，而非增加一组 Bridge。

### 16.3 确定性检查

所有 duration 使用 tick，speed 使用 fixed-point；transition 有显式 priority 与 stable ID 决胜；每 tick
最多一次转换；Trigger 消费点唯一；Pause/StepFrame 复用正式 fixed tick。方案没有依赖 wall-clock、hash map
顺序或 render FPS。

### 16.4 架构边界检查

底层求值规则位于 Rust，引擎外部配置位于 schema；项目只提交 typed intent。Animator2D 不拥有 gameplay
真相、不读取 Tower 私有规则、不写 RenderProxy，并继续复用 RuntimePackage 与 Projection 体系，符合 194/195/196
的两层心智与 110 的统一投影规则。

### 16.5 范围检查

本方案是可支撑 idle/attack/hit/death 的最小 Controller，而非缩水后仍保留大量未实现接口的完整 Animator。
骨骼、曲线、混合、事件、完整 Graph 与 atlas 均明确延期。276 Window A-C / Gate A-F 已全部完成；
施工文档归档在 `施工文档/已完成/`，阶段完成记录保留受影响源码证据与未执行边界。

## 17. 下一步入口

施工文档已归档在
`施工文档/已完成/276-当前可自动化施工文档-Animator2D-Mini-Clip-Controller-Fixed-Tick-Present-v1.md`，
完成记录为 `阶段完成记录/2026-08-08-Animator2D-Mini-Clip-Controller-Fixed-Tick-Present-v1/00-总览.md`。
Tower P1-2、274 v2-B/C/D、production Editor、Local CI、production/安装态替换和真实配置均未自动进入。
