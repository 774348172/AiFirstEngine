# 94-Engine Gameplay Foundation C-min 边界方案

## 当前归属说明：Projection 术语

本文中如果出现以下历史名称：

```text
RenderExtract
RenderAssetBridge / Render Asset Bridge
Physics2DBridge
RuntimeScene Hydration
AuiRenderExtract / AuiRendererBridge
SpriteRenderer2D ECS-to-RenderProxy Bridge
```

统一按 `110-World-Projection-Adapter统一跨域同步规则.md` 理解为：

```text
RenderProjection
AssetProjection
Physics2DProjection
HydrationProjection
UiProjection
RenderProjectionAdapter<SpriteRenderer2D>
```

这些名称可以作为历史实现名保留，但不再作为新增架构概念扩展。后续新增类型只新增对应 `ProjectionAdapter`，不新增独立 Bridge。

## 1. 文档目的

本文定义 `Engine Gameplay Foundation C-min` 的正式边界。

这里的 `Gameplay Foundation` 不是玩法框架，不是打飞机模块，不是 RPG 战斗框架，也不是 Unity / UE 式高层 Gameplay Framework。

它只是：

```text
项目规则访问 Runtime ECS 的最小安全接口。
```

它解决的是：

```text
如何查询 Entity
如何读取 Component
如何写入 Component
如何提交 Entity 结构变化
如何记录 Trace 方便 AI 和人排查问题
```

它不解决：

```text
伤害怎么算
敌人怎么找
子弹怎么发射
分数怎么增加
波次怎么生成
技能怎么结算
背包怎么管理
```

这些全部属于项目层。

## 2. 核心风险

如果本系统没有硬边界，它一定会膨胀。

典型错误路径是：

```text
为了打飞机加入 SpawnBullet
为了 RPG 加入 ApplyDamage
为了塔防加入 FindEnemiesInRange
为了 ARPG 加入 ApplyBuff
为了背包加入 AddItem
```

最终 `Engine Gameplay Foundation` 会从底座 API 变成混合玩法框架，导致：

```text
引擎理解项目语义
项目变化会反向推动引擎 API 增长
AI 需要同时维护引擎规则和项目规则
复杂项目后期难以判断 Bug 属于引擎还是项目
文档和代码不断叠规则，长期不可维护
```

因此本系统必须保持非常窄。

## 3. 全局边界规则

正式规则：

```text
引擎只提供底座能力，不为特定项目增加规则。
```

展开为：

```text
Engine Layer 只提供通用原语。
Project Layer 定义业务语义。
Engine Gameplay Foundation 不允许出现任何项目名词。
```

判断标准：

如果一个 API 名字里出现以下词，默认不允许进入 Foundation：

```text
enemy
bullet
damage
health
score
wave
skill
buff
inventory
weapon
quest
team
target
loot
boss
```

Foundation API 只允许使用中性词：

```text
entity
component
field
query
command
trace
schema
value
read
write
spawn
despawn
```

如果某个能力无法用这些中性词表达，优先判断为项目层能力，而不是引擎底座能力。

## 4. 与已有规则的关系

本文继承以下既有规则：

```text
16-ECS写入与项目规则边界.md
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
52-ECS-Storage-v1方案.md
93-复杂打飞机验证所需引擎侧缺失能力清单.md
```

核心继承点：

```text
引擎解决数据安全，不解决业务正确性。
项目解决业务正确性。
AI 主要修改项目层规则，不直接修改 ECS Runtime 底层机制。
业务顺序、结算顺序、业务依赖由项目层表达。
```

## 5. 市场引擎对比

### Unity

Unity 底座能力主要是：

```text
GameObject
Component
GetComponent
AddComponent
Instantiate
Destroy
Transform
Physics query
```

Unity 不会在引擎底座里提供：

```text
SpawnBullet
ApplyDamage
AddScore
StartWave
```

这些由项目脚本、Prefab、ScriptableObject、项目框架实现。

### Unreal Engine

UE 底层核心能力主要是：

```text
Actor
Component
World
SpawnActor
DestroyActor
Tick
Collision Query
Reflection
```

UE 有更高层 Gameplay Framework，但具体项目里的 Damage、Inventory、Quest、Skill 仍然不是 `World / Actor` 最底层 API 必须理解的东西。

本项目第一版不复制 UE Gameplay Framework，只学习它的基础对象生命周期和诊断能力。

### Bevy

Bevy 底座能力主要是：

```text
Entity
Component
Query
Commands
Resource
Event
Schedule
```

Bevy 不理解 `Enemy / Bullet / Health`。这些都是项目自己定义的 Component。

本项目最接近 Bevy 的点是：

```text
Query + Commands + Component 数据访问
```

但本项目不会把 Bevy 的完整 Query 类型系统和 Schedule 心智暴露给普通用户和 AI。

### Godot

Godot 底座能力主要是：

```text
Node
Scene
instantiate
queue_free
get_node
signal
```

Godot 不在底层内置子弹、敌人、血量、得分这些项目概念。

本项目借鉴 Godot 的简单心智，但 Runtime 底层仍采用 ECS。

## 6. Engine Gameplay Foundation C-min 只包含什么

第一版只包含 5 类能力：

```text
Query
Read
Write
Structural Command
Trace
```

这 5 类能力是项目规则访问 ECS 的最小语言。

### 6.1 Query

Query 只负责根据 Component 组合选择 Entity。

允许：

```text
all: [Transform, game.Bullet]
none: [engine.Disabled]
include_disabled: false
limit: none
stable_order: EntityId
```

不允许：

```text
enemy_in_range
nearest_enemy
team == enemy
hp < 50
has_target
```

字段过滤、距离过滤、最近目标、敌我关系都不是 Foundation Query 的职责。

### 6.2 Read

Read 只负责读取 Component 数据。

允许：

```text
read_component(entity, component_type)
```

不允许：

```text
read_health
read_damage
read_enemy_state
```

### 6.3 Write

Write 只负责写 Component 字段或整个 Component。

允许：

```text
write_component_field(entity, component_type, field_path, value)
write_component(entity, component_type, value)
```

不允许：

```text
damage(entity, amount)
heal(entity, amount)
add_score(value)
apply_buff(entity, buff)
```

字段 path 第一版只支持简单 path：

```text
current
local_position.x
stats.attack
```

不支持：

```text
inventory[3].count
buffs[*].duration
items.where(id=xxx)
```

### 6.4 Structural Command

CommandBuffer 只处理 Entity / Component 结构变化。

第一版允许：

```text
SpawnEntity
DespawnEntity
AddComponent
RemoveComponent
SetParent
```

第一版不允许：

```text
SpawnBullet
SpawnEnemy
InstantiateWeapon
DropLoot
ApplyBuff
SendDamageEvent
```

普通字段写入不进入 CommandBuffer，直接通过 Write 生效。

结构变化进入 CommandBuffer，并在 FrameLoop 安全点统一 apply：

```text
ProjectLogicRunner 本轮规则执行结束
  -> CommandBuffer Apply
  -> RenderExtract
```

### 6.5 Trace

Trace 只记录通用 ECS 访问和结构变化。

最小字段：

```text
frame_index
phase
rule_id
operation
entity_id
component_type
field_path
before
after
command_id
source
result
error_code
```

不允许把业务专用字段塞进 Engine Trace：

```text
final_damage
killer_id
critical_hit
gold_reward
combo_count
```

如果项目需要这些字段，应由项目层写 `ProjectTraceEvent`。

## 7. ComponentValue 边界

`ComponentValue` 只表达 Schema 数据，不表达业务对象。

允许：

```text
Bool
I64
F64
String
Vec2
Vec3
Color
EntityRef
AssetRef
Object
Array
Null
```

不允许：

```text
DamageValue
HealthValue
SkillValue
InventoryValue
QuestValue
```

Schema 负责验证 `Object / Array` 的结构是否合法。Runtime 只保存、读写和 Trace。

## 8. 复杂项目如何扩展

复杂项目不通过扩展 Foundation API 来解决业务需求。

复杂项目通过以下方式扩展：

```text
Project Schema
Project Rule
Project Module
Project Pipeline
Project State Rule
Project Trace Event
Project Data Asset
```

示例：

打飞机项目增加：

```text
game.Bullet
game.Enemy
PlayerFireRule
BulletMoveRule
CollisionResolveRule
```

引擎 Foundation 不增加 `SpawnBullet`。

RPG 项目增加：

```text
game.Health
game.DamageEvent
DamagePipeline
DeathCheckRule
```

引擎 Foundation 不增加 `ApplyDamage`。

塔防项目增加：

```text
game.Tower
game.Monster
TargetSelectRule
```

引擎 Foundation 不增加 `FindEnemyInRange`。

如果坐标查询性能不足，应讨论独立的 `2D Spatial Query / Collision C-min`，但该系统也必须只提供空间底座能力，不理解 Enemy / Bullet / Damage。

## 9. AI 生成规则

AI 生成项目规则时，只能调用 Foundation 的中性 API：

```text
query
read_component
write_component_field
write_component
commands.spawn_entity
commands.despawn_entity
commands.add_component
commands.remove_component
commands.set_parent
```

AI 不应该生成或请求引擎新增项目专用 API：

```text
spawn_bullet
apply_damage
add_score
find_enemy
```

如果自然语言需求里出现这些项目概念，AI 应该生成项目侧 Schema / Rule / Pipeline，而不是修改引擎底座。

## 10. 最终结论

```text
Engine Gameplay Foundation C-min 可以保留。
但它不是 Gameplay Framework。
它只是 Runtime ECS Access Foundation。

它只包含 Query / Read / Write / Structural Command / Trace。
它不包含任何项目语义。
```

更短的长期规则：

```text
引擎只提供底座能力，不为特定项目增加规则。
项目变化只能推动 Project Layer 增长，不能推动 Engine Foundation API 膨胀。
```

## 11. 下一步

如果本边界确认，下一步可以继续细化：

```text
QuerySpec v1 数据结构
ComponentValue v1 数据结构
ComponentFieldPath v1 规则
GameplayCommandBuffer v1 命令结构
GameplayTraceRecord v1 最小字段
```
