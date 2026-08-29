# M11 Physics2D Collider Authoring / Visualization C-min 方案

## 1. 系统定义

M11 是 Physics2D 碰撞体从编辑器创作、可视化、保存，到 Runtime Physics2D 生效的第一版产品化闭环。

它不是重新实现 Physics2D runtime，也不是新增项目玩法碰撞规则。当前 runtime 已经有：

```text
Collider2D
Physics2DWorld
Physics2DProjectionAdapter<Collider2D>
Physics2D pair report
Physics2D trace
```

M11 要补齐的是编辑器侧和验证侧：

```text
Scene Entity
  -> Collider2D component
  -> Inspector 可编辑 shape / size / radius / offset / layer / mask / sensor / enabled
  -> Viewport Debug Overlay 可视化碰撞体
  -> Selected Entity Collider 高亮
  -> Save Scene
  -> RuntimePackage / Scene Hydration
  -> Physics2D pair report / trace / diagnostics
```

## 2. 边界规则

引擎只提供通用碰撞体 authoring 能力，不提供任何项目玩法 API。

允许的引擎概念：

```text
Entity
Component
Collider2D
Shape2D
Aabb
Circle
Offset
Layer
Mask
Sensor
Enabled
DebugOverlay
Diagnostic
Trace
Report
```

禁止把以下项目语义做成引擎 API：

```text
Player
Enemy
Bullet
Health
Damage
Score
Weapon
Boss
Drop
Hit
Kill
```

这些只能由 Project Schema / Project Rule / Prefab / Scene 数据表达。

## 3. 其他引擎做法

### Unity

Unity 的路线是 `Collider2D` 作为组件，`BoxCollider2D / CircleCollider2D / PolygonCollider2D` 等通过 Inspector 和 Scene 工具编辑。

源码参考：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/Inspector/ColliderEditorBase.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/PolygonEditor.bindings.cs
```

对我们的启发：

```text
Collider 是场景对象上的可编辑组件。
Inspector 和 Scene View 都必须能表达 Collider。
复杂 polygon 编辑可以后置，第一版先做 box / circle。
```

### Unreal Engine

UE 的路线更完整：`UShapeComponent / UBoxComponent / USphereComponent / UCapsuleComponent` 表达形状组件，底层通过 `FBodyInstance / UBodySetup / CollisionProfile` 管理运行时物理和碰撞配置。

源码参考：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Classes/PhysicsEngine/BodySetup.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Classes/PhysicsEngine/BodyInstance.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/TriggerActors.cpp
```

对我们的启发：

```text
长期必须有完整 Collider 编辑器、碰撞配置、运行时实例和 report。
第一版不能直接复制 UE 的 BodySetup / Profile 复杂度。
layer / mask 可以先用简单 bitmask，后续再升级为可命名 CollisionProfile。
```

### Godot

Godot 的 2D 碰撞体系很适合我们参考。`CollisionShape2D` 是节点，`Shape2D` 是资源，支持 debug draw、disabled、configuration warnings。

源码参考：

```text
<GODOT_SOURCE>/godot-master/godot-master/scene/2d/physics/collision_shape_2d.cpp
<GODOT_SOURCE>/godot-master/godot-master/scene/resources/2d/rectangle_shape_2d.cpp
<GODOT_SOURCE>/godot-master/godot-master/scene/resources/2d/circle_shape_2d.cpp
```

对我们的启发：

```text
可视化和诊断是一等能力。
Shape 本身负责几何数据和 debug draw 数据。
Collider 缺 shape、缺 parent / transform 时必须给 configuration warning。
```

### Bevy

Bevy 核心更偏 ECS，不内置完整 2D 物理 authoring；通常由 Rapier / Avian / XPBD 等插件提供。

对我们的启发：

```text
Collider 应该保持为 ECS component。
Debug visualization 应该是独立系统，不污染项目逻辑。
```

## 4. 方案对比

| 方案 | 做法 | 接近 Unity / UE 程度 | 优点 | 缺点 | 结论 |
|---|---|---:|---|---|---|
| A | 只做 Inspector 字段编辑 | 低 | 最快 | 看不到碰撞体，调试困难 | 不选 |
| B | Inspector + Viewport Debug Overlay | 中 | 第一版可用，复杂度可控 | 还没有完整拖拽编辑 | 作为 C-min 的第一版落地范围 |
| C | 完整 Collider 编辑器 | 高 | 最像 Unity / UE，长期最完整 | 第一版成本高，容易拖慢主线 | 作为长期目标 |

最终选择：

```text
长期路线采用 C：完整 Unity / UE 式 Collider 编辑器。
第一版落地采用 C-min：按 C 的架构边界设计，但只实现 B 的可控功能范围。
```

## 5. 正式方案

### 5.1 第一版支持范围

M11 C-min 支持：

```text
Collider2D Aabb
Collider2D Circle
enabled
sensor
layer
mask
offset
Inspector 字段编辑
Viewport Debug Overlay 画出 collider
Selected Entity Collider 高亮
Scene save / reload 后 collider 保持
RuntimePackage / Scene Hydration 后 Physics2D 识别 collider
Physics2D diagnostics / trace / pair report 可定位问题
```

第一版不做：

```text
Polygon / Capsule / Edge / Composite collider
完整拖拽 gizmo
碰撞层矩阵 UI
命名 CollisionProfile
物理材质
碰撞事件玩法 API
运行时刚体动力学
```

### 5.2 数据模型

复用 runtime 当前 `Collider2D`，第一版不新增第二套 authoring truth。

推荐数据结构心智：

```text
Collider2D
  shape: Collider2DShape
  offset: Vec2
  enabled: bool
  sensor: bool
  layer: PhysicsLayer
  mask: PhysicsMask

Collider2DShape
  Aabb { half_extents: Vec2 }
  Circle { radius: f32 }
```

如果当前 runtime 字段名已经不同，施工时优先复用现有字段，不为了文档命名制造迁移。

### 5.3 Editor Inspector

Inspector 只通过 Schema-driven Inspector / Scene Edit Transaction 修改 Collider2D component。

规则：

```text
Inspector 不直接写 Runtime Physics2DWorld。
Inspector 只修改 Scene document 中 Entity 的 Collider2D component。
Scene document 保存后，RuntimePackage / Hydration 再生成 runtime world。
```

第一版字段：

```text
shape_kind: Aabb | Circle
aabb_half_extents.x
aabb_half_extents.y
circle_radius
offset.x
offset.y
enabled
sensor
layer_bits
mask_bits
```

### 5.4 Viewport Debug Overlay

Viewport Debug Overlay 是编辑器显示层，不是 runtime 真相层。

输入：

```text
EditorSceneDocument
SelectedEntity
Transform
Collider2D
Viewport camera / zoom
```

输出：

```text
ColliderDebugDrawList
  item_count
  draw_items
  selected_item
  diagnostics
```

绘制规则：

```text
Aabb 画矩形线框。
Circle 画圆形线框。
enabled=false 使用灰色 / 半透明。
sensor=true 使用虚线或专用颜色。
selected entity 的 collider 高亮。
缺 Transform 的 Collider2D 不绘制，并产生 diagnostic。
非法尺寸 / 半径不绘制，并产生 diagnostic。
```

### 5.5 Runtime 验证

M11 不修改 Physics2D runtime 主流程，只要求 authoring 出来的 Collider2D 能进入现有链路：

```text
Scene document
  -> RuntimePackage
  -> Hydration
  -> ECS World Collider2D
  -> Physics2DProjectionAdapter<Collider2D>
  -> Physics2DWorld
  -> pair report / trace
```

### 5.6 Report / Trace

第一版 report 只记录少而精字段：

```text
collider_count
draw_item_count
selected_entity_id
invalid_collider_count
missing_transform_count
diagnostics[]
```

diagnostic 最小字段：

```text
severity
entity_id
component_type
field_path
message
suggestion
```

## 6. 最小验收场景

### 场景 A：Inspector 编辑并保存 Aabb Collider

```text
Create Entity
Add Transform
Add Collider2D Aabb
Set half_extents=(0.5, 0.5)
Save Scene
Reload Scene
Assert Collider2D 保持
```

### 场景 B：Viewport 可视化

```text
Scene 中有一个 Transform + Collider2D Aabb
Build ColliderDebugDrawList
Assert draw_item_count=1
Select Entity
Assert selected draw item highlighted
```

### 场景 C：Runtime pair report

```text
Scene 中两个 Entity 均有 Transform + Collider2D
RuntimePackage + Hydration
Run one frame
Assert Physics2D pair report 能看到 overlap pair
```

### 场景 D：诊断

```text
Entity 有 Collider2D 但缺 Transform
Build ColliderDebugDrawList
Assert diagnostic: missing Transform
Run Physics2D sync
Assert Physics2D diagnostic 同样可定位 entity
```

## 7. 可施工 Gate

### Gate A：Collider2D Schema / Inspector 接入

目标：

```text
Schema-driven Inspector 能识别并编辑 Collider2D 字段。
Scene Edit Transaction 能写 Collider2D 字段。
```

测试：

```powershell
cargo test -p editor_core collider2d_inspector
cargo test -p editor_core scene_edit_transaction_set_collider2d
```

### Gate B：ColliderDebugDrawList

目标：

```text
从 Scene document 生成 ColliderDebugDrawList。
Aabb / Circle 能转成 viewport debug shape。
selected / enabled / sensor 状态进入 draw item。
```

测试：

```powershell
cargo test -p editor_core collider_debug_draw_list
```

### Gate C：Viewport Model 接入

目标：

```text
Editor UI Model / Viewport model 暴露 collider overlay summary。
不要求真实 Native UI 完整绘制，但模型必须稳定。
```

测试：

```powershell
cargo test -p editor_ui_model collider_overlay
cargo test -p editor_core editor_session_collider_overlay
```

### Gate D：Runtime 验证闭环

目标：

```text
Authoring Scene 中的 Collider2D 保存后能进入 RuntimePackage / Hydration / Physics2D pair report。
```

测试：

```powershell
cargo test -p engine_runtime physics2d
cargo test -p editor_core collider2d_authoring_runtime_roundtrip
```

### Gate E：整体回归

测试：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime
```

## 8. 方案自审

### 8.1 Specification fit

本方案回答的是 M11：Physics2D Collider Authoring / Visualization。它覆盖编辑器 Inspector、Viewport 可视化、保存、Runtime 验证和诊断，不偏离到玩法碰撞逻辑。

### 8.2 Rule fit

方案遵守引擎底座边界：只提供 Collider2D 通用能力，不引入 Player / Enemy / Bullet / Damage 等项目 API。

### 8.3 Textual consistency

文档中长期目标为完整 Unity / UE 式 Collider 编辑器，第一版为 C-min，落地范围明确限制在 Aabb / Circle、Inspector、Debug Overlay、验证闭环，没有与范围冲突。

### 8.4 Design fit

方案对 AI 友好：Collider2D、DebugDrawList、Report 都是结构化数据。对复杂项目有用：碰撞体可见、可改、可诊断。规则数量少，没有新增复杂物理策略。

### 8.5 Implementation feasibility

当前已有 `engine_runtime::physics2d::Collider2D`、Scene Editing、Schema-driven Inspector、Viewport model、Physics2D trace，可增量施工。

### 8.6 Practical reasonableness

第一版不做 polygon、完整 gizmo、CollisionProfile、物理材质，能避免系统过大，同时保留长期升级方向。

结论：

```text
方案通过自审，可以进入施工文档阶段。
```
