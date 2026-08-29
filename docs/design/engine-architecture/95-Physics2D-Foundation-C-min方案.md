# 95-Physics2D Foundation C-min 方案

## 当前归属说明：Physics2DProjection

本文档中历史出现的 `Physics2DBridge`，从 `110-World-Projection-Adapter统一跨域同步规则.md` 起统一归属为：

```text
Physics2DProjection
```

正确理解：

```text
ECS World physics-facing components
  -> Physics2DProjectionAdapter
  -> Physics2DWorld
  -> CollisionPairReport / Physics2DTrace
```

`Physics2DBridge` 只是早期落地名，不再作为新增系统扩展。后续 Collider2D / Rigidbody2D / Joint2D 等类型只新增或扩展 Physics2DProjectionAdapter，不新增独立 Bridge。

## 1. 文档目的

本文确认 `Physics2D Foundation C-min` 的正式方向。

它不是项目玩法系统，也不是打飞机专用碰撞逻辑。它是引擎侧通用 2D 物理 / 空间查询底座。

核心结论：

```text
可以接入完整 Physics2D 类似系统，并且可以和 ECS 融合。
但第一版不直接实现完整物理求解。
第一版按完整 Physics2D 架构设计，只实现查询和碰撞对 C-min。
```

## 2. 为什么不是只做 2D Spatial Query

`2D Spatial Query / Collision C-min` 能解决第一版打飞机的命中判断，但如果系统命名和架构只停留在空间查询，后续要升级到完整 Physics2D 时会遇到结构迁移：

```text
Overlap Query
  -> Collision Pair
  -> Trigger
  -> Rigidbody2D
  -> Physics Step
  -> WriteBack Transform
```

如果一开始只做松散查询 API，后面容易补出第二套物理入口。

因此正式系统命名收敛为：

```text
Physics2D Foundation C-min
```

第一版能力仍然保持很小，但结构上按完整 Physics2D 预留。

## 3. 与 ECS 的融合原则

正确结构：

```text
ECS World = 游戏对象与组件真相
Physics2D World = 引擎内部物理计算世界
Physics2D Bridge = ECS 与 Physics2D World 的同步层
```

不能让 ECS 和 Physics2D 同时都成为世界真相。

项目规则和 AI 看到的是 ECS 组件：

```text
Transform
Collider2D
Rigidbody2D
PhysicsMaterial2D
PhysicsLayer
```

真正执行 broadphase、narrowphase、碰撞对生成、未来刚体求解的是引擎内部：

```text
Physics2DWorld
```

## 4. 推荐 FrameLoop 位置

长期流程：

```text
FixedUpdate begin
  -> Project Rule 读取输入并写 ECS
  -> Project Rule 可写 Transform / Velocity / Spawn / Despawn
  -> Apply Structural Command
  -> Physics2D Sync From ECS
  -> Physics2D Step
  -> Physics2D WriteBack To ECS
  -> Physics2D Event / Query Result / Trace
  -> Project Rule 可读取碰撞结果
  -> RenderExtract
```

第一版 C-min 不做完整刚体求解，但仍保留这些边界：

```text
Physics2D Sync From ECS
Physics2D Query / Pair Build
Physics2D Trace / Report
```

## 5. 第一版 C-min 只做什么

第一版实现：

```text
Collider2D component
PhysicsLayer / Mask
AABB shape
Circle shape
Physics2DWorld
Physics2DBridge
overlap_aabb
overlap_circle
collision pair report
query trace
headless tests
```

第一版不做：

```text
Rigidbody2D dynamic solver
gravity
friction
restitution
sleep
continuous collision detection
OnCollisionEnter 自动回调
Trigger 自动事件
复杂 polygon shape
物理材质求解
真实 debug draw UI
```

## 6. 项目侧边界

Physics2D Foundation 不理解项目玩法语义。

允许引擎提供：

```text
Collider2D
Shape2D
Layer
Mask
Overlap Query
CollisionPair
PhysicsTrace
```

不允许引擎提供：

```text
BulletHitEnemy
ApplyDamage
EnemyCollision
PlayerPickupItem
AddScore
```

项目侧根据碰撞结果决定业务含义。

例如打飞机：

```text
引擎返回 entity_a 与 entity_b 的 Collider2D 重叠。
项目规则判断 entity_a 是否有 game.Bullet，entity_b 是否有 game.Enemy。
项目规则自己扣血、销毁、加分。
```

## 7. 成熟引擎参考

### Unity

Unity 用户层看到：

```text
Transform
Rigidbody2D
Collider2D
Physics2D.Overlap*
Physics2D.Raycast*
```

底层存在独立 Physics2D 世界。用户通过 Component 配置，物理系统在固定物理步中同步和求解。

对我们的启发：

```text
ECS Component 是用户可理解表面。
Physics2DWorld 是内部执行世界。
Overlap Query 是第一版最实用入口。
```

### Unreal Engine

UE 用户层看到：

```text
Actor
PrimitiveComponent
Collision Channel
LineTrace / Sweep / Overlap
Physics Scene
```

底层由 Chaos Physics 和独立物理场景执行，Game Thread 与 Physics Thread 通过同步点交换数据。

对我们的启发：

```text
碰撞查询和物理求解可以共享底层 Physics World。
查询参数 / Layer / Mask 必须是通用概念。
不要把业务语义写进物理层。
```

### Godot

Godot 用户层看到：

```text
Node2D
Area2D
CollisionShape2D
PhysicsServer2D
```

底层 `PhysicsServer2D` 管理物理世界，场景节点只是用户 authoring 表面。

对我们的启发：

```text
Physics2DWorld 可以是引擎内部服务。
Scene / ECS 只持有用户可编辑组件。
```

### Bevy

Bevy 本体 ECS 是主世界，完整物理通常通过 Rapier / Avian 等插件接入。插件通过 ECS Component 同步到内部物理世界，再把结果写回 ECS。

对我们的启发：

```text
ECS 主世界 + Physics 插件/模块世界 是成熟路线。
Physics2DBridge 是必要边界。
```

## 8. AI 友好规则

AI 修改项目规则时，不直接操作 Physics2DWorld。

AI 可以生成项目规则调用：

```text
physics2d.overlap_aabb(query)
physics2d.overlap_circle(query)
read collision_pair_report
```

AI 不生成：

```text
PhysicsWorld raw mutation
solver internal change
broadphase tree mutation
```

如果自然语言需求是“子弹击中敌人扣血”，AI 应生成项目层规则：

```text
query bullet entities
for each bullet:
  query Physics2D overlap
  filter hit entities by project component
  write project health
  command despawn bullet when needed
```

而不是要求引擎新增：

```text
apply_bullet_damage
```

## 9. 推荐下一步

下一步不是马上施工完整 Physics2D，而是生成：

```text
95-当前可自动化施工文档-Physics2D-Foundation-C-min.md
```

施工范围应限制为：

```text
Collider2D typed component
Physics2DWorld headless data structure
Physics2DBridge sync from ECS
overlap_aabb / overlap_circle
CollisionPairReport
Physics2DTrace
打飞机式中性测试：moving entity overlaps target entity
```

测试中禁止使用：

```text
Bullet
Enemy
Damage
Health
Score
```

测试命名可以使用：

```text
source entity
target entity
moving collider
static collider
```

## 10. 最终规则

```text
Physics2D 可以作为完整引擎系统接入。
ECS World 仍然是游戏对象与组件真相。
Physics2DWorld 是内部物理计算世界。
Physics2DBridge 负责同步。
第一版只实现查询和碰撞对，不实现完整刚体求解。
引擎物理层不理解项目玩法语义。
```
