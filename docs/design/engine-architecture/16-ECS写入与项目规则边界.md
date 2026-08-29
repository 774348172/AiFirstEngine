# ECS 写入与项目规则边界

本文档定义 ECS、项目规则、AI 修改和引擎责任的正式边界。

## 核心结论

```text
ECS 默认允许项目规则直接读写 Component。
引擎层只解决引擎级安全问题，不解决项目业务正确性。
项目层负责业务规则顺序、结算顺序和业务依赖。
AI 主要修改项目层规则，不直接修改 ECS Runtime 底层机制。
```

一句话：

```text
引擎解决数据安全，不解决业务正确性。
项目解决业务正确性。
AI 帮项目生成和维护业务正确性。
```

## 引擎层负责

引擎层负责通用运行时能力：

```text
Entity / Component 存储
Component 查询
System 执行
FrameLoop 阶段
并行调度
读写冲突检测
内存安全
结构变化安全
CommandBuffer / Deferred Commands
Profiler / Trace 基础数据
```

Bevy / Unity DOTS / Flecs 等 ECS 的共同经验是：结构变化不能随意穿插在并行查询中执行。  
本项目采用同类原则，但保持用户心智简单：

```text
普通 Component 写入可以在允许阶段直接执行。
Spawn / Despawn / AddComponent / RemoveComponent 进入 Deferred Commands。
Deferred Commands 在 FrameLoop 安全点统一 apply。
这只是引擎级结构安全，不代表引擎替项目判断业务顺序。
```

当多个 System 读写同一个 Component 时，引擎只保证运行时安全：

```text
不会产生数据竞争
不会并发写坏 Component Storage
不会在结构变化中破坏 ECS World
不会生成无法执行的调度循环
```

引擎不判断业务语义：

```text
Damage 是否应该先于 Heal
护盾是否应该先于扣血
金身是否应该最后触发
反弹基于原始伤害还是最终伤害
A 业务规则依赖 B 业务规则是否合理
```

这些属于项目层。

## 项目层负责

项目层负责所有业务正确性：

```text
扣血顺序
Buff 优先级
技能结算顺序
装备结算顺序
伤害 / 治疗 / 护盾 / 反弹 / 死亡保护的业务语义
系统之间的业务依赖
复杂玩法规则的 Pipeline / Solver / State Machine
```

简单项目可以直接写 Component：

```text
BulletHitSystem:
  enemy.Health -= bullet.Damage
```

复杂项目可以在项目层组织业务流程：

```text
DamagePipeline:
  shield
  reduce
  reflect
  golden_body
  apply_health
```

DamagePipeline 是项目规则组织方式，不是所有 Component 默认必须经过的引擎级 Resolver。

## Component 写入规则

默认规则：

```text
System / Rule 默认可以直接写 Component。
```

例如：

```text
Health.value -= 10
Transform.position += Velocity * deltaTime
Ammo.current -= 1
```

引擎可以根据读写冲突决定：

```text
并行执行
串行执行
插入 sync point
禁止非法并行
报告调度循环
```

但引擎不会替项目决定业务顺序。项目如果需要顺序，必须在项目层表达：

```text
DamageSystem before DeathCheckSystem
PoisonTick before HealthRegen
CombatDamagePipeline 内部定义步骤顺序
```

## CommandBuffer 使用边界

CommandBuffer 不作为所有业务写入的默认入口。

CommandBuffer 主要用于：

```text
创建 Entity
销毁 Entity
添加 Component
删除 Component
批量结构变化
跨线程延迟提交
需要延后到安全点执行的 Runtime Command
```

普通 Component 数值写入可以直接执行：

```text
Health.value -= damage
Transform.position = newPosition
```

结构变化建议走 CommandBuffer：

```text
SpawnEntity
DestroyEntity
AddComponent
RemoveComponent
InstantiatePrefab
```

## 冲突处理规则

引擎级冲突：

```text
多个线程同时写同一 Component Storage
System 调度依赖成环
结构变化发生在不安全时间点
读写权限不满足并行调度条件
```

这些由引擎阻止或串行化。

业务级冲突：

```text
多个规则都修改 Health，且顺序影响结果
两个规则互相需要对方本帧计算结果
Buff、伤害、治疗、死亡保护之间存在业务优先级
```

这些由项目层解决。引擎最多提供诊断和 Trace，不替项目定义业务答案。

项目层可选解决方式：

```text
指定系统顺序
合并成一个规则
组织成领域 Pipeline
使用上一帧结果
使用项目自定义 Solver
```

## AI 修改边界

AI 默认修改：

```text
Feature Spec
Component Schema
Rule Graph / DSL / IR
Project Blueprint
项目层 System / Pipeline / State Rule
```

AI 默认不修改：

```text
ECS Component Storage 实现
System Scheduler 底层规则
并行调度器
CommandBuffer 底层机制
Rust AOT 编译产物
```

AI 可以根据业务需求生成项目层规则顺序，例如：

```text
金身在最终致死判断时触发
护盾先于生命值扣除
死亡检查在所有伤害和治疗之后执行
```

但这些是项目业务规则，不是引擎 ECS 默认规则。

## 与主流 ECS 的对齐

本规则对齐 Unity DOTS、Bevy、Flecs 等 ECS 路线的共同点：

```text
Component 可以被系统直接读写。
CommandBuffer / Commands 主要处理延迟结构变化。
调度器解决读写安全和并行冲突。
业务语义顺序由项目系统表达。
```

区别在于本项目需要额外提供 AI 可读的 Trace 和验证报告：

```text
AI 不需要维护底层 scheduler。
AI 需要能解释项目规则为什么这样排序。
AI 需要在业务冲突时生成可理解的修复建议。
```

## 正式边界

```text
ECS Runtime Framework = Engine Layer
Component Schema = Project Layer
Project System / Rule = Project Layer
业务顺序 = Project Layer
读写安全 = Engine Layer
并行调度安全 = Engine Layer
结构变化安全 = Engine Layer
业务正确性 = Project Layer
```
