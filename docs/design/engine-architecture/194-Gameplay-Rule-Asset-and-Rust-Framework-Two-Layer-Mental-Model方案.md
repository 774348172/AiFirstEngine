# 194-Gameplay Rule Asset and Rust Framework Two Layer Mental Model 方案

## 1. 一句话说明

本方案收敛项目逻辑 authoring 的用户心智：

```text
用户只需要理解两层：

Rust Gameplay Framework
  -> 负责系统底座、复杂流程、性能、确定性和 ECS 调度

Gameplay Rule Asset
  -> 负责项目上层规则、数值公式、条件、权重和可热更规则槽
```

内部仍可保留 Schema、System Contract、Rule Graph / DSL、Canonical Rule IR、RuntimePackage 等工程产物，但它们不能被描述成用户必须逐层理解和手动维护的独立逻辑层。

## 2. 本方案为什么产生

前序文档已经建立了正式路线：

```text
Natural Language
  -> Feature Spec
  -> Project Model
       Schema
       Blueprint
       Rule Graph / DSL
  -> Canonical Rule IR
  -> Lowered Execution IR
  -> IR Interpreter / Rust AOT
  -> Rust ECS Runtime
```

这个路线作为内部工程管线是合理的。

但如果把它作为用户心智展示：

```text
Schema
Blueprint
Rule Graph
DSL
IR
RuntimePackage
Rust Domain Runtime
```

就会比 Unity 的 C# + Lua、UE 的 C++ + Blueprint 更难理解，也不利于 AI 长期稳定修改。

因此本方案将内部管线收敛为用户可理解的两层模型。

## 3. 与旧文档的差异

### 3.1 04 文档中的 System Blueprint 表述

`04-引擎能力边界与蓝图.md` 中写道：

```text
薄核心 + 标准模块 + 系统蓝图 / 游戏蓝图 + 项目层
System Blueprint：可组合玩法系统蓝图
Game Blueprint：多个 System Blueprint 的推荐组合
```

这个方向仍然保留，但命名和暴露层级需要修正。

问题：

```text
System Blueprint 容易被误解为 UE Blueprint 式节点脚本。
System Blueprint 如果成为用户显式层，会让用户必须判断自己该编辑 Schema、Blueprint、DSL 还是 IR。
```

新规则：

```text
System Blueprint 在用户心智中改称 System Contract / Gameplay System Contract。
它是 Rust Gameplay Framework 与 Gameplay Rule Asset 之间的内部契约。
它声明 Rule Slot、输入输出、Command、Event、Query、验证规则和热更策略。
它不作为用户每天显式编辑的脚本层。
```

### 3.2 05 文档中的多层 Project Model 表述

`05-逻辑系统边界-DSL-IR-RustAOT-ECS.md` 已经写明：

```text
Schema / Blueprint / Rule Graph / DSL 不是连续多层翻译链。
它们属于同一层 Project Model，只是表达不同工程对象。
```

这个规则继续保留，并进一步收敛：

```text
Schema 是 Gameplay Rule Asset / Gameplay Asset 的字段类型和数据契约。
System Contract 是系统边界和规则槽契约。
Rule Graph / DSL 是同一份 Gameplay Rule Asset 的两种编辑视图。
Canonical Rule IR 是 Gameplay Rule Asset 编译出的规范语义。
RuntimePackage 是构建输出，不属于用户的项目逻辑 authoring 层。
```

### 3.3 193 文档中的 Rule Authoring Productization 表述

`193-Rule-Authoring-Productization-v1方案.md` 推荐直接产品化：

```text
RuleAuthoringService
ProjectRuleAsset create/open/select/save
Canonical IR structured edit
```

该方案作为底层能力仍有价值，但作为用户主入口不够准确。

问题：

```text
用户不应以 ProjectRuleAsset / Canonical IR 为主要心智。
复杂项目不应让用户先创建裸 Rule，再手动理解它属于哪个系统。
自走棋、打飞机、RPG 等项目更自然的对象是 Unit / Ability / Trait / Item / Shop Rule / Combat Rule。
```

新规则：

```text
Rule Authoring Productization 必须上移为 Gameplay Asset / Rule Slot Authoring。
ProjectRuleAsset / Canonical Rule IR 是内部真相和构建输入。
用户和 AI 默认修改 Gameplay Rule Asset 或 Gameplay Asset 内的 Rule Slot。
```

## 4. 当前正式用户心智

正式收敛为：

```text
Rust Gameplay Framework + Contract-bound Gameplay Rule Asset
```

解释：

```text
Rust Gameplay Framework:
  系统怎么跑。
  复杂流程、算法、ECS 查询、调度、确定性、性能、生命周期、资源和平台边界。

Gameplay Rule Asset:
  规则怎么变。
  公式、条件、权重、过滤、效果片段、数值、可验证热更。

System Contract:
  两者之间的契约。
  约束 Rule Asset 能读什么、写什么、发什么事件、调用什么命令、何时热更、必须跑什么测试。
```

用户默认看到：

```text
Unit
Ability
Trait
Item
Shop Rule
Combat Rule
Level Rule
UI Binding Rule
```

用户默认不需要直接理解：

```text
Canonical Rule IR
Lowered Execution IR
RuntimePackage manifest
Rust AOT generated source
RuleRegistry internals
ECS storage internals
```

## 5. Rule Graph / DSL / IR 的关系

正式关系：

```text
Rule Graph / DSL = Gameplay Rule Asset 的编辑表达
Canonical Rule IR = Gameplay Rule Asset 的规范语义
```

它们不是：

```text
Rule Graph -> DSL -> IR 的多层手工维护链
```

它们是：

```text
同一个 Gameplay Rule Asset
  -> 可以用 Rule Graph 图形视图编辑
  -> 可以用 DSL 文本视图编辑
  -> 保存 / 构建时生成 Canonical Rule IR
```

边界：

```text
Rule Graph:
  面向用户的图形编辑视图。

DSL:
  面向 AI / 高级用户的文本编辑视图，适合 diff、patch、搜索和批量修改。

Canonical Rule IR:
  面向引擎的规范中间表示，强类型、确定性、可验证、可 source map、可解释执行、可编译到 Rust AOT。
```

IR 必须保留 source map，使运行时错误能回溯到：

```text
Feature Spec
Gameplay Rule Asset
Rule Graph / DSL 节点
Canonical Rule IR node
Runtime trace
```

## 6. System Contract 的保留原因

不能把架构真的砍成只有：

```text
Rust Framework
IR
```

原因是 Rust 与 IR 中间必须有契约，否则 IR 不知道：

```text
能读哪些 Component
能写哪些字段
能发哪些 Event
能提交哪些 Command
哪些规则允许热更
哪些规则必须 deterministic
哪些测试必须覆盖
```

没有 System Contract 会导致两种失败：

```text
IR 太弱：
  只能写简单公式，复杂项目不够用。

IR 太强：
  慢慢变成 Lua / C# / UE Blueprint。
```

因此：

```text
System Contract 必须存在。
但它应该是内部契约资产和高级诊断对象，不是普通用户显式心智层。
```

## 7. 自走棋示例

用户看到的是：

```text
BloodKnight.unit
  stats:
    attack: 70
    maxHp: 900
    lifesteal: 0.25

  combatRules:
    on_damage_dealt:
      heal_self = min(damage * lifesteal, 80)
```

内部对应：

```text
Rust AutoChessCombatFramework:
  回合流程
  目标选择流程
  攻击冷却
  命中事件
  伤害应用
  死亡清理
  确定性随机

System Contract:
  components:
    Health
    CombatStats
    BoardPosition
    BuffList
    TraitSet

  commands:
    RequestAttack
    ApplyDamage
    KillUnit

  events:
    AttackHit
    DamageApplied
    UnitDied

  ruleSlots:
    calculate_damage
    lifesteal_after_damage
    trait_bonus_modifier

Gameplay Rule Asset:
  ruleSlot: lifesteal_after_damage
  expression: heal_self = min(damage * lifesteal, 80)

Canonical Rule IR:
  编译后的强类型规则语义

RuntimePackage:
  发布运行输入
```

用户不需要手动在这些内部层之间跳转。

## 8. 与 Unity / UE 的对标

### Unity

Unity 常见心智：

```text
C# 写系统
ScriptableObject / 表格写数据
Lua 做热更和轻规则
```

我们的收敛心智：

```text
Rust Gameplay Framework 写系统
Gameplay Rule Asset 写数据化 / 可验证 / 可热更规则
```

区别：

```text
不把自由 Lua 作为核心热更路线。
不让规则绕过 System Contract。
AI patch 必须输出结构化 report、影响范围和验证结果。
```

### Unreal Engine

UE 常见心智：

```text
C++ 写底层系统和复杂玩法框架
Blueprint / DataAsset 暴露给设计师编辑
Gameplay Ability System 提供能力、属性、效果框架
```

我们的收敛心智：

```text
Rust Gameplay Framework 类似 C++ / GAS 方向。
Gameplay Rule Asset 类似 Blueprint / DataAsset 的用户入口，但更受控。
```

区别：

```text
Rule Asset 不是完整自由可视化编程语言。
Rule Asset 只运行在 System Contract 声明的 Rule Slot 中。
IR 作为规范语义，服务 AI 验证、热更、AOT 和 trace。
```

## 9. 修正后的方案选择

### 方案 A：继续直接产品化 ProjectRuleAsset / Canonical IR

结论：

```text
不推荐作为用户主入口。
可作为 debug / import-export / fallback / internal service。
```

原因：

```text
用户心智过底层。
复杂项目规则会脱离 Unit / Ability / Trait / Item 等自然对象。
容易让 IR 变成新的脚本语言。
```

### 方案 B：只保留 Rust + IR 两层

结论：

```text
不推荐。
```

原因：

```text
缺少 System Contract。
IR 权限、事件、命令、热更、确定性和测试边界不清楚。
长期会在太弱和太强之间摇摆。
```

### 方案 C：Rust Gameplay Framework + System Contract + Gameplay Rule Asset

结论：

```text
推荐。
```

用户心智仍是两层：

```text
Rust Gameplay Framework
Gameplay Rule Asset
```

内部保留契约和编译链：

```text
System Contract
Schema
Rule Graph / DSL view
Canonical Rule IR
Lowered Execution IR
IR Interpreter / Rust AOT
RuntimePackage
```

理由：

```text
AI 适配性最好：Rule Asset / Contract / IR 都可结构化 patch 和 report。
复杂项目可维护：系统流程在 Rust Framework，规则变化在 Rule Asset。
效率更平衡：用户心智接近 UE C++ + Blueprint，但比自由 Blueprint / Lua 更可验证。
```

## 10. 对 193 的后续要求

如果继续推进 `193-Rule-Authoring-Productization-v1`，必须先按本方案修正标题和范围。

建议新方向：

```text
Gameplay Rule Asset / Rule Slot Authoring Productization v1
```

v1 不应实现为：

```text
裸 ProjectRuleAsset 编辑器
裸 Canonical IR 表单编辑器
完整自由节点图脚本系统
```

v1 应实现为：

```text
Rule Slot Authoring Service
Gameplay Rule Asset Document
Rule Graph / DSL 双视图中的至少一种结构化编辑入口
Canonical IR 作为内部生成 / 验证 / 构建产物
System Contract validation
Rule impact report
RuntimePackage build evidence
```

## 11. 正式规则

后续讨论、方案和施工以以下规则为准：

```text
1. 用户心智只有两层：Rust Gameplay Framework + Gameplay Rule Asset。
2. System Blueprint 改称或解释为 System Contract，不作为 UE Blueprint 式脚本层。
3. Rule Graph / DSL 是 Gameplay Rule Asset 的编辑视图，不是多层真相。
4. Canonical Rule IR 是规范语义，不是用户默认编辑对象。
5. RuntimePackage 是构建输出，不属于项目逻辑 authoring 层。
6. IR v1 必须优先收敛为 Rule Slot 规则片段，不吞掉完整系统流程。
7. 复杂流程、确定性、ECS 查询和生命周期默认属于 Rust Gameplay Framework / Domain Runtime。
8. 所有规则修改必须能输出 source map、impact report、validation report 和测试证据。
9. 官方 Shop / Buff / Ability 等只能作为可选 System Contract / Framework 库，不进入 Core Engine API。
10. 新施工前必须先更新对应方案，使其不再把内部编译链描述成用户必须理解的层。
```

## 12. 参考

本项目文档：

```text
04-引擎能力边界与蓝图.md
05-逻辑系统边界-DSL-IR-RustAOT-ECS.md
09-热更新能力边界.md
11-测试与验证系统.md
186-Project-Rule-Asset-Pipeline-Runtime-Execution-C-min方案.md
187-Project-Rule-Artifact-Module-Lifecycle-B-min方案.md
193-Rule-Authoring-Productization-v1方案.md
```

外部对标：

```text
Unity Visual Scripting Graphs
https://docs.unity3d.com/Packages/com.unity.visualscripting%401.8/manual/vs-graph-types.html

Unity ScriptableObject
https://docs.unity3d.com/Manual/class-ScriptableObject.html

Unreal Engine Blueprints Visual Scripting
https://dev.epicgames.com/documentation/unreal-engine/blueprints-visual-scripting-in-unreal-engine

Unreal Engine Gameplay Ability System
https://dev.epicgames.com/documentation/unreal-engine/gameplay-ability-system-for-unreal-engine
```
