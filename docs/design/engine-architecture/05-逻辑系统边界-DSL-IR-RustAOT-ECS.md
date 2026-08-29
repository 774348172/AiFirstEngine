# 逻辑系统边界-DSL-IR-RustAOT-ECS

本文档定义项目逻辑、IR、IR Interpreter、Rust AOT 与 Rust ECS Runtime 的正式边界。

> 当前实现基线见 `201-热更新方案收敛与当前实现基线.md`。本文中的 IR Interpreter 热更覆盖是长期逻辑边界；当前代码里的 `IrInterpreterExecutor` 是 validation-only unsupported，当前规则执行主线仍按 `186` / `187` 的 Rust AOT + StaticRegistry 理解。

Project Logic Runner 的正式执行入口和规则见：

```text
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
```

早期 `IR -> WASM` 方案已经移入：

```text
历史文档/05-逻辑系统边界-DSL-IR-WASM-ECS-历史版.md
```

## 当前正式路线

> 当前解释：本文定义 IR 管线的工程边界。根据 `195` / `196`，IR 只作为 Contract-bound RuleSlot 的受限规则数据，不是 Lua / Blueprint 式脚本语言，也不是所有项目逻辑的默认承载层。

正式路线：

```text
Natural Language
  -> Feature Spec
  -> Feature Asset / Feature Folder
  -> Project Assets / Project Model
       Schema / System Contract
       AUI Document
       Gameplay Rule Asset
       Rule Graph / DSL view
  -> Canonical Rule IR for Contract-bound RuleSlot
  -> Lowered Execution IR
  -> IR Interpreter，用于编辑器 / 验证 / 热更覆盖
  -> IR -> Rust AOT，用于正式发布
  -> Rust ECS Runtime
```

核心结论：

```text
Feature Spec 是需求真相。
Feature Asset / Project Assets 是用户和 AI 的默认编辑真相。
Project Model 是工程结构真相。
Canonical Rule IR 是受限 RuleSlot 的内部规范语义和构建输入。
Lowered Execution IR / Rust AOT 是派生执行产物。
IR Interpreter 是解释执行后端。
Rust AOT 是正式发布高性能后端。
Rust ECS Runtime 是运行时状态真相。
WASM 暂不作为当前默认项目逻辑主路线。
```

重要约束：

```text
Schema / System Contract / Rule Graph / DSL 不是连续多层翻译链。
它们属于同一层 Project Model，只是表达不同工程对象。
系统只允许少数真相层，避免多次自由翻译导致语义丢失。
```

IR 红线：

```text
IR 不表达完整战斗流程、完整经济流程、行为树、技能状态机、A* 寻路、复杂 UI 交互流程。
IR 不允许递归、while / unbounded loop、任意函数、任意数组 / map 编程、直接 ECS / Renderer / File / Network API。
复杂 gameplay 流程、复杂算法、复杂 UI 工作流默认进入 Rust Project Module / Rust Framework。
```

## 为什么不以 WASM 作为当前默认路线

WASM 仍然可以作为未来 Web、插件沙箱、第三方 UGC 规则的可选后端，但当前不作为默认项目逻辑路线。

原因：

```text
iOS 上下载并执行改变功能的代码存在平台审核风险
WASM / Rust 双 runtime 会增加调试和边界复杂度
WASM 不是性能优势来源
AI 可控性主要来自 Spec / Schema / Blueprint / DSL / IR，而不是 WASM 本身
```

因此当前采用：

```text
IR Interpreter 负责开发期、验证期、热更覆盖
Rust AOT 负责正式发布性能
```

## 逻辑层级

逻辑分为四层：

```text
Core Engine Logic
Standard / Native Module Logic
System / Domain Blueprint Logic
Project Rule Logic
```

### Core Engine Logic

Core Engine Logic 属于引擎核心，主要由 Rust 实现。

包括：

```text
ECS Runtime Framework
Entity 管理
Component Storage
System Scheduler
Scene / Prefab 加载
Asset Runtime
Build / Export Pipeline
IR Interpreter
IR -> Rust AOT Compiler
Validation / Test Runner
Architecture Governance
Platform Layer
```

用户自然语言和 AI 默认不能修改 Core Engine Logic。

### Standard / Native Module Logic

Standard / Native Module 是引擎提供的通用能力模块，也主要由 Rust 实现。

包括：

```text
Transform / Hierarchy
Render
Physics
Audio
Animation
Navigation
Input
Save
Network Transport
Math / Random / Curve / Spatial Compute
```

这些模块必须保持项目无关。

允许：

```text
SpatialCompute.query_radius(...)
WeightedRandom.sample(...)
Curve.evaluate(...)
AssetModule.load(...)
AudioModule.play(...)
```

不允许：

```text
find_enemies_in_radius(...)
refresh_shop_items(...)
play_boss_death_sound(...)
```

项目语义必须留在项目层的 Schema / Blueprint / DSL / IR 中。

### System / Domain Contract Logic

System / Domain Contract 定义系统结构、流程骨架和边界。历史文档中的 Blueprint 按 Contract 理解，不按 UE Blueprint 式节点脚本理解。

它负责：

```text
系统有哪些 Component
系统接受哪些 Command
系统发出哪些 Event
系统有哪些 Rule Slot
系统读写哪些数据
系统如何和其它系统通信
哪些规则允许热更
哪些测试必须覆盖
```

Contract 不直接实现复杂底层能力，不直接保存具体玩法数据，也不承载完整脚本逻辑。

例子：

```text
EquipmentBlueprint
  commands:
    - EquipItem
    - UnequipItem
  events:
    - ItemEquipped
    - ItemUnequipped
  ruleSlots:
    - can_equip_rule
    - stat_merge_rule
```

### Project Rule Logic

Project Rule Logic 是具体项目中的受限规则片段。

当前解释：

```text
Project Rule Logic 只指可数据化、可验证、可审查、可热更的规则片段。
完整 gameplay 流程、复杂状态机、复杂 UI 交互和复杂算法不属于 IR RuleSlot，默认进入 Rust Project Module / Rust Framework。
```

来源：

```text
Feature Spec
DSL / Graph
IR
Data Asset
Rule Slot
```

执行方式：

```text
开发 / 验证 / 热更覆盖 -> IR Interpreter
正式发布 -> Rust AOT
```

例子：

```text
伤害公式
Buff 修正
装备穿戴条件
商店刷新权重
任务完成条件
掉落规则
简单 UI 可见性 / enable / 显示变换规则
表现选择规则
任务监听
```

不再作为 IR 默认职责的例子：

```text
技能状态机
Buff 生命周期完整流程
子弹飞行完整流程
AI 行为状态
复杂 UI 交互流程
```

## IR 的定位

IR 是 Intermediate Representation，中间表示。

在本引擎中：

```text
Feature Spec = 需求真相
Project Model = 工程结构真相
Canonical Rule IR = 可执行项目规则真相
Lowered Execution IR = 派生执行产物
IR Interpreter = 开发 / 验证 / 热更覆盖执行器
Rust AOT = 正式发布执行器
```

### Canonical Rule IR

Canonical Rule IR 是系统围绕 Contract-bound RuleSlot 建立的内部规范语义层。

当前解释：

```text
Canonical Rule IR 是 Gameplay Rule Asset / RuleSlot 编译出的内部规范语义。
它不是普通用户默认编辑对象，也不表示全部项目逻辑都必须进入 IR。
```

它必须具备：

```text
强类型
确定性
可验证
可追踪
可解释执行
可编译到 Rust AOT
可从运行错误回溯到 DSL / Feature Spec
```

它负责：

```text
保留项目规则语义
保留 Requirement ID
保留 source map
保留读写权限
保留事件输入输出
保留规则依赖
服务 AI 修 Bug
服务编辑器审查
服务验证系统
作为 Lowered Execution IR 和 Rust AOT 的唯一来源
```

## Canonical Rule IR 规则类型

Canonical Rule IR 必须支持两类规则。

### Function Rule

Function Rule 是一次性规则。

适合：

```text
伤害公式
装备属性合并
商店刷新权重
掉落计算
任务条件判断
目标过滤
表现选择
```

特征：

```text
输入固定
输出固定
无长期运行状态
一次执行结束
```

示例：

```text
calculate_damage(input) -> DamageOutput
roll_drop(input) -> DropResult
can_equip(input) -> bool
```

### State Rule / Lifecycle Rule

State Rule / Lifecycle Rule 是带生命周期和运行时状态的规则。

当前限制：

```text
State Rule / Lifecycle Rule 只允许作为受限 RuleSlot 的轻量生命周期规则。
如果需求需要复杂状态机、行为树、寻路、复杂数组算法、长生命周期对象编排，应改用 Rust Project Module。
```

适合：

```text
简单持续效果 tick
简单倒计时规则
简单任务条件监听
简单光环数值修正
简单 UI 可见性 / enable 状态
```

它必须显式声明生命周期：

```text
start
frame_update
fixed_update，可选
event_handler，可选
exit
destroy，可选
```

它必须显式声明运行时状态 Schema：

```text
runtime schema
state version
hot update migration，可选
reset / cleanup behavior
```

它可以表达：

```text
初始化状态
每帧推进
接收事件
发出 Command / Event
触发后续节点
退出和清理
```

### frame_update 规则

State Rule 可以声明 `frame_update`，但 `frame_update` 不强制拆成固定 phase。

原因：

```text
强制 phase 会让规则体系变重。
不同系统的 phase 名称和边界会快速膨胀。
AI 需要先判断功能属于哪个 phase，反而增加维护成本。
```

正式规则：

```text
frame_update = contract-bound rule body
```

也就是：

```text
不强制 precheck / compute / resolve / apply / completion 等固定 phase。
允许规则 body 自己组织顺序。
但必须受到 Contract、类型系统、预算、Trace 和 Command / Effect 分离约束。
```

`frame_update` 必须声明：

```text
read views
write runtime schema
commands
effects
events
native calls
budget
trace fields
```

允许：

```text
受控分支
受控循环
读取声明过的 View
写入声明过的 Runtime
发出声明过的 Command
发出声明过的 Event
发出声明过的 Effect
调用白名单 Native Module
```

不允许：

```text
直接 ECS 读写
直接对象调用
直接渲染操作
文件 / 网络 / 系统时间
无限循环
未声明 Native 调用
未声明 Command / Effect / Event
```

预算约束作用于整个 `frame_update`，不按 phase 拆分：

```text
maxOps
maxNativeCalls
maxLoopItems
maxEmits
```

一句话：

```text
frame_update 允许表达复杂状态逻辑，但不是自由 Update 脚本。
它不靠 phase 约束，而靠 Contract + Type + Budget + Trace 约束。
```

它不允许直接实现：

```text
底层碰撞算法
Actor 底层位置写回
渲染对象直接操作
动画系统直接操作
对象池 / 指针寻址
文件 / 网络 / 平台 API
```

这些能力必须通过 Native Module 或 Runtime Command 执行：

```text
SpatialMovement.sdf_move
ActorMovement.set_position
Presentation.change_anim_speed
EventBus.bind / unbind / emit
SkillGraph.start_node
```

正式规则：

```text
Function Rule 解决“算什么”。
State Rule / Lifecycle Rule 解决“一个项目状态如何随时间运行”。
底层能力仍由 Native Module / Rust Runtime 执行。
Canonical Rule IR 只表达受控的生命周期语义、状态变更、事件和命令。
```

### Lowered Execution IR

Lowered Execution IR 是从 Canonical Rule IR 确定性生成的执行产物。

它负责：

```text
解释器高效执行
Rust AOT codegen
基础优化
执行顺序固定
trace map 保留
后端等价测试
```

它不允许作为用户或 AI 的默认编辑对象。  
如果 Lowered Execution IR 出错，应回溯并修复 Canonical Rule IR / Project Model / Feature Spec。

### IR 不负责

```text
直接操作 ECS 底层存储
直接加载资源
直接播放表现
直接访问文件 / 网络 / 系统时间
直接调用平台 API
直接修改引擎核心能力
```

## 反信息丢失规则

为了避免层数过多导致语义丢失，逻辑管线必须遵守：

```text
少数真相层
多个派生产物
派生产物不可手改
每次转换必须可验证、可回溯、可对比
```

必须提供以下检查：

```text
Requirement Coverage Check:
  检查每条 Feature Spec 是否落到 Project Model / Canonical Rule IR。

IR Semantic Validation:
  检查 Canonical Rule IR 是否满足类型、权限、确定性和模块边界。

Lowering Validation:
  检查 Canonical Rule IR 到 Lowered Execution IR 是否保持语义等价。

Interpreter vs Rust AOT Equivalence Test:
  同一份 Canonical Rule IR 分别走解释器和 Rust AOT，输出、事件、错误和 deterministic hash 必须一致。

Trace Replay:
  运行时 Bug 必须能从 Runtime Trace 回溯到 Lowered IR、Canonical Rule IR、Rule Graph / DSL 和 Feature Spec。
```

## IR Interpreter

IR Interpreter 是 Rust Runtime 内置解释器。

用途：

```text
编辑器即时预览
AI Patch 验证
Logic Test
Scenario Test
热更覆盖规则
Debug Trace
运行时错误回溯
```

热更规则：

```text
基础包规则默认走 Rust AOT
热更覆盖规则走 IR Interpreter
未被热更覆盖的规则继续走 Rust AOT
```

运行选择：

```text
RuleRegistry:
  rule_id:
    base: RustAOT(v1)
    hotfix: IR(v2)，可选
    active: hotfix if exists else base
```

伪代码：

```text
if rule.has_hotfix:
  run_ir_interpreter(rule.hotfix_ir, input)
else:
  run_rust_aot(rule.base_fn, input)
```

## Rust AOT

Rust AOT 是正式发布后端。

流程：

```text
IR
  -> Rust rule codegen
  -> Rust compiler
  -> platform native binary
```

作用：

```text
提升正式包性能
减少解释器运行成本
避免规则长期以解释方式跑高频逻辑
与 Rust ECS Runtime 更自然集成
```

约束：

```text
Rust AOT 产物不是项目真相
Rust AOT 不能由 AI 直接手写
Rust AOT 必须由 IR 确定性生成
Rust AOT 行为必须和 IR Interpreter 等价
```

## 后端等价

同一份 IR 在解释器和 Rust AOT 下必须得到一致结果。

每个发布规则必须通过：

```text
Rust AOT Equivalence Test
```

测试方式：

```text
同一份 IR
  -> IR Interpreter 执行测试输入
  -> Rust AOT 执行测试输入
  -> 对比输出、事件、错误、deterministic hash
```

不一致时：

```text
禁止发布
标记 BackendMismatch
回指 IR node / DSL node / Requirement ID
由 AI 生成修复 Patch Plan
```

## ECS 与上层项目模型映射

正式规则：

```text
E 来自 Scene / Prefab / Runtime Spawn Request。
C 来自 Component Schema + Component Data。
S 来自 Native Module + System / Domain Blueprint + IR Rule。
```

### Entity

底层：

```text
ECS Entity = Rust Runtime 中的运行时对象 ID
```

上层来源：

```text
Scene Entity
Prefab Entity / Prefab Instance
Runtime Spawn Request
```

Scene / Prefab 中声明的 Entity 在运行时被 Spawn 成 Rust ECS Entity。  
Entity 本体保持轻量，复杂关系由 Component、Hierarchy / Transform、Asset Graph 和 Runtime Reference Index 管理。

Runtime Spawn Request 是对象生成领域的内部请求结构。  
它不属于统一万能 RuntimeCommand，也不替代 SceneLifecyclePlan / AssetLoadRequest / RenderCommand。

Project Rule / State Rule 需要运行时生成对象时，只能提交 Runtime Spawn Request：

```text
Project Rule / State Rule
  -> Runtime Spawn Request
  -> Runtime Spawn System
  -> Rust ECS Entity / Component
```

规则边界：

```text
Project Rule / State Rule 不直接调用 ECS spawn。
AI 不直接拼 Rust ECS Component storage。
Runtime Spawn Request 只描述要生成的 prefab、owner、parent、transform 和 componentOverrides。
真正写 ECS 由 Runtime Spawn System 执行。
```

引擎层销毁边界：

```text
Runtime Despawn Request 是对象销毁领域的内部请求结构。
Runtime Despawn System 是唯一删除 ECS Entity 的入口。
Project Rule / State Rule 不直接调用 ECS despawn。
资源 release 不属于 Runtime Despawn Request。
```

EntityRef 边界：

```text
Project Rule / State Rule 不能持有裸 ECS 指针或裸 entity index。
规则中需要引用运行时对象时，使用 RuntimeEntityHandle。
RuntimeEntityHandle 由 Runtime resolve 后才能读写 Component。
resolve 失败必须返回明确 diagnostics，而不是静默失败或自动重绑定。
旧 handle 在 generation 不匹配时不能命中新 Entity。
```

这条规则用于保证：

```text
AI 能定位 entity_not_found / generation_mismatch / pending_despawn / scene_unloaded。
Runtime 槽位复用不会造成旧规则误写新对象。
复杂项目中跨事件、延迟请求、Trace 的 Entity 引用仍可追踪。
```

### Component

底层：

```text
ECS Component = Rust Runtime 中挂在 Entity 上的运行时数据
```

上层来源：

```text
Component Schema = Component 类型定义
Scene / Prefab Component Data = Component 初始值
Runtime Component Data = 运行时产生或修改的数据
Built-in Component = 引擎内置组件
```

Schema 定义字段和类型，Rust Runtime 注册或生成对应 Component Storage。  
Scene / Prefab / Spawn Request 提供初始 Component Data。

### System

底层：

```text
ECS System = Rust Runtime 中被调度执行的系统
```

上层来源分两类：

```text
Native System:
  来自 Engine Core / Native Module
  由 Rust 实现
  例如 TransformSystem / RenderSystem / PhysicsSystem / AudioSystem

Project Rule System:
  来自 System / Domain Contract
  由 Rust Domain Runtime 调度
  可验证规则来自 Contract-bound IR RuleSlot
  发布版可执行 Rust AOT
  热更 / 编辑器 / 验证可执行 IR Interpreter
```

一句话：

```text
上层 Scene / Prefab 生成 Entity。
上层 Schema / Component Data 生成 Component。
上层 Native Module / System Contract / IR RuleSlot 生成或驱动 System。
```

## Project Rule System

Project Rule System 属于项目层，但运行时由 Rust Runtime 调度。

它不是用户手写 Rust，也不是引擎写死的具体玩法。

生成来源：

```text
用户自然语言需求
  -> AI 生成 Feature Spec / System Spec
  -> AI 生成 Feature Asset / Project Assets
  -> AI 生成 Schema / System Contract / Gameplay Rule Asset / Rule Graph / DSL view
  -> 编译器生成 Contract-bound Canonical Rule IR
  -> 编辑器 / 验证 / 热更时由 IR Interpreter 执行
  -> 正式发布时由 IR -> Rust AOT 生成原生规则函数
  -> Build Pipeline 生成 Runtime System 配置和规则后端
  -> Rust Runtime 调度执行
```

结构：

```text
System / Domain Contract:
  trigger
  read schema
  write schema
  event schema
  rule slot
  validation rule

Rust Domain Runtime:
  监听 trigger / command
  查询 ECS 数据
  组装 IR Rule 输入
  按 RuleRegistry 选择 IR Interpreter 或 Rust AOT
  验证规则输出
  写回 Component Storage
  发出 Event / Command

IR Rule:
  根据输入计算输出
  不直接访问 ECS 底层存储
  不直接操作 Entity 生命周期
  不直接 mount Bundle / 加载资源 / 播放表现
```

## 扣血流程示例

项目定义：

```text
HealthComponent Schema:
  hp
  maxHp
  shield

CombatStatsComponent Schema:
  attack
  defense
  element

Environment Schema:
  weather

CombatBlueprint:
  command / event:
    OnAttackHit
    DamageApplied
    DeathCandidate
  ruleSlot:
    calculate_damage
```

运行时：

```text
Rust CombatDomainRuntime
  -> 接收 OnAttackHit
  -> 查询 attacker.CombatStats
  -> 查询 target.Health
  -> 查询 world.Environment
  -> 构造 DamageInput
  -> 调用 calculate_damage(input)
       无热更：Rust AOT
       有热更覆盖：IR Interpreter
  -> 验证 DamageOutput
  -> 写回 target.Health
  -> 发出 DamageApplied / DeathCandidate
```

IR Rule 只负责：

```text
calculate_damage(input) -> output
```

它不直接：

```text
查 ECS
改 Component Storage
播放特效
加载资源
调用平台 API
```

## IR 示例

示意规则：

```text
最终伤害 = max(1, attacker.attack - target.defense)
雨天伤害 * 110 / 100
护盾先吸收
剩余伤害扣 hp
hp 最低 0
hp == 0 时输出 DeathCandidate
```

示意 IR：

```json
{
  "ruleId": "combat.damage.calculate",
  "version": "1.0.0",
  "deterministic": true,
  "inputs": ["DamageInput"],
  "outputs": ["DamageOutput"],
  "sourceMap": {
    "feature": "REQ_COMBAT_DAMAGE_001",
    "dslNode": "combat.damage.basic_formula"
  },
  "body": [
    {
      "op": "let",
      "name": "baseDamage",
      "value": {
        "op": "sub",
        "left": { "op": "get", "path": "attacker.attack" },
        "right": { "op": "get", "path": "target.defense" }
      }
    },
    {
      "op": "let",
      "name": "damage",
      "value": {
        "op": "max",
        "args": [
          { "op": "const", "type": "i32", "value": 1 },
          { "op": "var", "name": "baseDamage" }
        ]
      }
    },
    {
      "op": "if",
      "cond": {
        "op": "eq",
        "left": { "op": "get", "path": "world.weather" },
        "right": { "op": "const", "type": "enum", "value": "rain" }
      },
      "then": [
        {
          "op": "set_var",
          "name": "damage",
          "value": {
            "op": "div",
            "left": {
              "op": "mul",
              "left": { "op": "var", "name": "damage" },
              "right": { "op": "const", "type": "i32", "value": 110 }
            },
            "right": { "op": "const", "type": "i32", "value": 100 }
          }
        }
      ]
    },
    {
      "op": "let",
      "name": "shieldAbsorb",
      "value": {
        "op": "min",
        "args": [
          { "op": "get", "path": "target.shield" },
          { "op": "var", "name": "damage" }
        ]
      }
    },
    {
      "op": "let",
      "name": "remainingDamage",
      "value": {
        "op": "sub",
        "left": { "op": "var", "name": "damage" },
        "right": { "op": "var", "name": "shieldAbsorb" }
      }
    },
    {
      "op": "set_output",
      "path": "new_shield",
      "value": {
        "op": "sub",
        "left": { "op": "get", "path": "target.shield" },
        "right": { "op": "var", "name": "shieldAbsorb" }
      }
    },
    {
      "op": "set_output",
      "path": "new_hp",
      "value": {
        "op": "max",
        "args": [
          { "op": "const", "type": "i32", "value": 0 },
          {
            "op": "sub",
            "left": { "op": "get", "path": "target.hp" },
            "right": { "op": "var", "name": "remainingDamage" }
          }
        ]
      }
    },
    {
      "op": "if",
      "cond": {
        "op": "eq",
        "left": { "op": "output", "path": "new_hp" },
        "right": { "op": "const", "type": "i32", "value": 0 }
      },
      "then": [
        {
          "op": "emit",
          "event": "DeathCandidate",
          "payload": {
            "target": { "op": "get", "path": "target.entity" }
          }
        }
      ]
    }
  ]
}
```

## 热更规则

热更不是替换 Rust AOT。

热更也不是在任意一帧直接替换正在运行的规则实例。

正式规则：

```text
无热更：
  IR -> Rust AOT -> 原生执行

有热更覆盖：
  base rule 继续保留 Rust AOT
  hotfix rule 使用 IR Interpreter
```

热更生效必须发生在专门的热更时间点：

```text
下载 IR Rule Package / Asset Package
校验签名、hash、版本和依赖
通过 Schema / IR / Determinism / Platform Policy Validation
挂载到 Hot Update Staging Area
在允许的 Apply Point 切换 Rule Registry
之后的新规则调用走 IR Interpreter 热更覆盖
```

State Rule / Lifecycle Rule 的额外限制：

```text
正在运行中的 State Rule 实例不能热更。
正在运行的实例继续使用启动时绑定的规则版本。
热更包只影响热更生效点之后新启动的 State Rule 实例。
如果需要修改正在运行状态，必须等待专门安全时间点重新创建状态，不能中途替换 frame_update。
```

Function Rule 的限制：

```text
Function Rule 可以在专门热更时间点后切换版本。
已经进入调用栈的本次调用不替换。
下一次规则调用根据 Rule Registry 使用新版本。
```

热更包必须包含：

```text
IR Rule Package
rule version
hash
signature
source map
compatibility
rollback target
validation report
apply point
affected rule ids
```

高频规则热更必须谨慎。  
如果解释器性能不足，应作为临时 hotfix，并在下个完整版本合并回 Rust AOT。

## 边界总结

```text
Rust Runtime 拥有世界。
Feature Spec 描述用户需求。
Feature Asset / Project Assets 是用户和 AI 的默认编辑入口。
Project Model / System Contract 描述工程结构和规则槽边界。
Canonical Rule IR 描述 Contract-bound RuleSlot 的内部规范语义。
Canonical Rule IR 只支持受限 Function Rule 和轻量 State Rule / Lifecycle Rule。
Lowered Execution IR / Rust AOT 是派生执行产物。
IR Interpreter 执行热更 / 验证 / 编辑器规则。
Rust AOT 执行正式发布规则。
System Contract 组织系统边界。
Schema 定义数据结构。
AI 默认修改 Feature Spec / Feature Folder 内的 Project Assets / AUI Document / Gameplay Rule Asset / Rule Graph / DSL view。
Canonical Rule IR 是内部生成、验证、调试和构建输入，不是普通用户默认编辑对象。
复杂 gameplay 流程、复杂算法、复杂 UI 工作流默认进入 Rust Project Module / Rust Framework，不进入 IR。
AI 默认不直接修改 Lowered Execution IR 或 Rust AOT 产物。
```
