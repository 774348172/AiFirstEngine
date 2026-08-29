# 186-Project Rule Asset Pipeline & Runtime Execution C-min 方案

## 1. 系统定义

本系统正式命名为：

```text
Project Rule Asset Pipeline & Runtime Execution v1
```

本阶段采用：

```text
C-min：Unity / UE 式完整项目规则系统框架，但第一版只落最小闭环。
```

它不是引擎侧玩法系统，也不是给复杂打飞机硬编码一组规则 API。

它解决的问题是：

```text
AI 文档 / 用户编辑 / 模板生成的项目规则
  -> 作为 ProjectRuleAsset 保存
  -> 转成 Canonical Rule IR
  -> 校验
  -> 生成 Rust AOT 代码
  -> 编译 / 注册
  -> 写入 RuntimePackage / RuntimeRuleManifest
  -> Runtime 通过 ProjectLogicRunner 执行
  -> 通过 LogicContext 访问 ECS / CommandBuffer
  -> 通过 Trace / Report 反馈给编辑器和 AI
```

一句话：

```text
项目侧决定规则内容；引擎侧只提供规则资产进入 Runtime 的完整管线。
```

## 2. 为什么要从 M2 改成这个名字

旧 M2 文档使用过：

```text
Project Rule Authoring / Compile / Runtime Execute
Project Rule IR -> Rust AOT Codegen / Incremental Build / Runtime Execute
```

这些名字容易让系统边界变模糊，好像引擎要直接实现项目玩法规则。

新命名强调三件事：

```text
Project Rule Asset：规则是项目资产，不是引擎 API。
Pipeline：引擎负责资产、校验、编译、打包、注册、执行链路。
Runtime Execution：运行时只通过统一入口执行，不让规则绕过底座。
```

旧 M2 已完成的技术骨架继续保留：

```text
ProjectRuleIr
RuleCompiler
RuleModuleRegistry
ProjectLogicRunner
LogicContext
RuntimeRuleManifest
Rule Trace / Report
```

但从本文档开始，正式产品化方向以本系统为准。

## 3. 和其它引擎的对应关系

### Unity

Unity 的项目逻辑主要通过：

```text
C# Script / MonoBehaviour
Serialized Field
Assembly Compile
PlayerLoop
GameObject / Component API
Console / Profiler / Inspector
```

可以学习：

```text
脚本是项目资产。
脚本字段必须进入序列化 / Inspector / Build / Runtime 共识。
编辑器验证和发布运行围绕同一类脚本产物工作。
用户心智简单：把脚本挂到对象上，运行时进入 PlayerLoop。
```

不照搬：

```text
不采用 C# / MonoBehaviour 作为项目规则真相层。
不复制 Domain Reload 心智。
不把大量隐式生命周期 callback 作为 AI-first 主路线。
```

### Unreal Engine

UE 的项目逻辑主要通过：

```text
C++ / Blueprint Asset
Kismet / Blueprint Compilation
Generated Class / Bytecode / Native Function
Actor Tick / ProcessEvent
Log / Trace / Blueprint Debugger
```

可以学习：

```text
规则资产需要编译产物。
运行时挂入统一对象生命周期。
资产、编译、运行、调试要形成闭环。
Blueprint / C++ 都不直接等于底层渲染、物理、输入系统。
```

不照搬：

```text
第一版不做完整 Blueprint VM。
第一版不做完整节点图编辑器。
第一版不做 UE Live Coding 式进程内 patch。
```

### Godot

Godot 的项目逻辑主要通过：

```text
Script Resource
Node lifecycle
SceneTree
_process / _physics_process
Variant / Object property
```

可以学习：

```text
脚本挂载到场景对象的心智简单。
生命周期入口直观。
脚本、场景、资源之间有清晰引用关系。
```

不照搬：

```text
不以 Node callback 作为项目规则真相层。
不把 Variant / Object 动态调用作为 AI-first 长期核心。
```

### Bevy

Bevy 的项目逻辑主要通过：

```text
World
System
Schedule
Query
Commands
apply_deferred
```

可以学习：

```text
ECS 读写边界清晰。
结构变化走 Commands / deferred apply。
系统执行和数据访问可分析。
```

不照搬：

```text
不把完整 Rust Schedule / SystemSet / before-after 排序规则暴露给普通用户和 AI。
不让项目语义隐藏在过多插件注册和调度规则里。
```

## 4. 我们的长期系统心智

本系统长期对标的是：

```text
Unity Script System
UE Blueprint / C++ Gameplay System
Godot Script Resource
Bevy ECS System Pipeline
```

但本项目的真相层不同：

```text
Unity：C# source / serialized MonoBehaviour
UE：Blueprint asset / C++ source
Godot：script resource
Bevy：Rust systems
我们：Canonical Rule IR
```

正式长期链路：

```text
AI Request / User Edit
  -> Feature Spec / Project Model
  -> ProjectRuleAsset
  -> Canonical Rule IR
  -> Rule Validation
  -> Rust AOT Codegen
  -> Rule Artifact
  -> RuntimeRuleManifest
  -> RuntimePackage
  -> RuleModuleRegistry
  -> ProjectLogicRunner
  -> LogicContext
  -> ECS World / CommandBuffer
  -> Trace / Report
```

## 5. 真相层规则

唯一规则真相层：

```text
Canonical Rule IR
```

以下都不是规则真相层：

```text
ProjectRuleAsset 外壳
Generated Rust Source
Rust AOT Artifact
RuntimeRuleManifest
RuleModuleRegistry
ProjectLogicRunner
Trace / Report
```

它们的职责分别是：

```text
ProjectRuleAsset：项目资产容器，保存 IR、来源、显示名、source map、校验状态。
Generated Rust Source：由 IR 确定性生成的派生代码。
Rust AOT Artifact：由 Generated Rust 编译出的派生产物。
RuntimeRuleManifest：运行时索引，不保存业务逻辑。
RuleModuleRegistry：rule_id 到 Rust 函数的注册表。
ProjectLogicRunner：Runtime 调项目规则的唯一入口。
Trace / Report：诊断与追踪输出，不反向成为业务规则。
```

## 6. C-min 第一版范围

第一版必须做完整框架的最小闭环：

```text
ProjectRuleAsset v1
RuleAssetManifest v1
Canonical Rule IR v1
Rule validation
IR -> Rust AOT codegen
Rust incremental build decision
Rule artifact registry
RuntimeRuleManifest
RuntimePackage 接入
ProjectLogicRunner 执行
LogicContext ECS 访问
CommandBuffer 结构变化应用
Rule Trace / Report
```

第一版必须明确区分两层：

```text
Rule Trigger / Condition：决定规则什么时候执行。
Rule Statement / Operation：决定规则执行后做什么。
```

如果只保留 `operations` 静态列表，IR 只能表达“每次执行都做这些事”，无法表达复杂项目最基本的规则：

```text
按下 fire 时才生成 projectile。
遍历所有 projectile 并按速度移动。
收到 collision event 后才写入项目组件。
```

因此 C-min 的 Canonical Rule IR v1 必须从“静态操作序列”升级为“最小可编程规则结构”。

第一版最小结构：

```text
RuleTrigger
  - phase: FixedUpdate / Update / EventHandler
  - condition: always / action_pressed / event_received

RuleStatement
  - operation
  - when
  - for_each_query

RuleOperation
  - write_component_field
  - spawn_entity / instantiate_prefab
  - despawn_entity / despawn_prefab_instance
  - emit_event
```

第一版必须保留的最小能力：

```text
write_component_field
spawn_entity
despawn_entity
instantiate_prefab
despawn_prefab_instance
action_pressed condition
event_received condition
emit_event operation
for_each_query statement
```

当前代码实际已经存在的 `RuleOperation` 只有：

```text
WriteComponentField
SpawnEntity
DespawnEntity
```

因此 `ProjectRuleAsset`、`RuleAssetManifest`、`action_pressed`、`event_received`、`emit_event`、`for_each_query` 都属于 C-min 后续施工需要新增或收敛的能力，不能在文档中表述成已经完成。

`read_action` 不作为第一版 operation 名称。输入读取应表达为：

```text
condition: action_pressed("fire")
```

codegen 时由生成的 Rust AOT 调用 `LogicContext` 中的输入读取能力。

`emit_event` 可以作为 operation 保留，但它要求 runtime 有最小事件队列。若事件系统未完成，施工时必须先实现最小 `RuntimeEventQueue`，或将 `emit_event` 标记为 disabled diagnostic，不能静默忽略。

## 7. C-min 第一版不做

第一版不做：

```text
完整 Blueprint 图编辑器
完整脚本 IDE
IR Interpreter 正式执行主线
热更新 VM
复杂断点调试器
复杂生命周期图
规则市场 / 插件系统
自动规则排序大系统
项目玩法专用 API
```

其中 `IR Interpreter` 的新规则是：

```text
第一版编辑器验证和发布运行都走 Rust AOT。
IR Interpreter 只作为未来可选后端或历史原型参考，不进入当前 C-min 正式主线。
```

这样避免同时维护两套执行语义。

## 8. 引擎侧和项目侧边界

引擎侧只提供：

```text
ProjectRuleAsset schema
Canonical Rule IR schema
Rule validation
Rule compiler
Rust AOT codegen
Incremental build cache key
Runtime manifest
Rule registry
ProjectLogicRunner
LogicContext
ECS query / read / write
CommandBuffer
Input Action snapshot
Time context
Trace / Report
```

项目侧负责：

```text
具体组件 schema
具体规则内容
具体 prefab / scene / asset / input / AUI
具体业务语义
具体验证场景
```

引擎 API 禁止出现以下项目玩法语义：

```text
Player
Enemy
Bullet
Health
Damage
Score
Wave
Weapon
Boss
Drop
```

这些只能作为项目 Schema / Component / Rule / Prefab / Asset / AUI / Sample Project 存在。

## 9. 规则执行边界

正式规则：

```text
ProjectLogicRunner 是 Runtime 执行项目规则的唯一入口。
LogicContext 是项目规则访问运行时数据的唯一 API。
项目规则不能直接写 Render / Physics / Input / AUI 内部对象。
结构变化必须走 CommandBuffer。
CommandBuffer 在 ProjectLogicRunner 本轮规则执行结束后统一 apply。
RenderExtract / ProjectionAdapter 只能读取规则执行后的 ECS 结果。
```

示例：

```text
Input Action: fire
Project Rule: when fire pressed -> spawn prefab projectile
Prefab: projectile
Components: Transform / SpriteRenderer2D / Collider2D / project-defined data
Runtime: ProjectLogicRunner executes rule, CommandBuffer spawns entity
Render: ProjectionAdapter extracts SpriteRenderer2D after ECS is updated
```

禁止：

```text
engine.spawn_bullet()
engine.damage_enemy()
engine.add_score()
renderer.create_projectile_sprite()
physics.kill_enemy_on_hit()
```

## 10. ProjectRuleAsset v1 建议结构

第一版建议结构：

```json
{
  "schemaVersion": "project-rule-asset.v1",
  "assetId": "asset.rule.fire_projectile",
  "ruleId": "project.rule.fire_projectile",
  "displayName": "Fire Projectile",
  "sourceKind": "aiDoc",
  "enabled": true,
  "canonicalIr": {
    "schemaVersion": "project-rule-ir.v1",
    "ruleId": "project.rule.fire_projectile",
    "phase": "Update",
    "enabled": true,
    "operations": []
  },
  "sourceMap": {
    "featureId": "feature.fire_projectile",
    "documentPath": "Docs/gameplay/fire.md"
  },
  "validation": {
    "status": "unknown",
    "diagnostics": []
  }
}
```

说明：

```text
assetId：项目资产身份。
ruleId：运行时规则身份。
sourceKind：aiDoc / userAuthored / template / imported。
canonicalIr：唯一可执行规则真相。
sourceMap：AI 和用户定位规则来源。
validation：编辑器展示用缓存，不反向成为规则真相。
```

## 11. RuleAssetManifest v1 建议结构

第一版建议结构：

```json
{
  "schemaVersion": "rule-asset-manifest.v1",
  "rules": [
    {
      "assetId": "asset.rule.fire_projectile",
      "ruleId": "project.rule.fire_projectile",
      "phase": "Update",
      "enabled": true,
      "irHash": "sha256:...",
      "artifactId": "rule_artifact_fire_projectile",
      "sourceMapId": "feature.fire_projectile"
    }
  ]
}
```

Manifest 只做索引：

```text
不保存业务公式。
不保存项目玩法语义。
不替代 Canonical Rule IR。
```

## 12. 编译与增量规则

缓存 key 必须至少包含：

```text
rule_id
ir_hash
schema_version
compiler_version
engine_rule_abi_version
target
build_profile
```

重编译规则：

```text
IR hash 变化 -> 重新生成 Rust source。
compiler_version 变化 -> 重新生成 Rust source。
engine_rule_abi_version 变化 -> 重新编译 artifact。
target / build_profile 变化 -> 重新编译 artifact。
只有 validation 缓存变化 -> 不触发重新编译。
```

## 13. Rust AOT Artifact 生成与加载规则

审查结论明确指出：当前 `RuleCompiler` 已能生成 Rust source 字符串，但这还不是完整 Rust AOT 闭环。

完整 Rust AOT 闭环必须包含：

```text
Canonical Rule IR
  -> generate_rust_source
  -> 写入 generated_rules crate / module
  -> 生成 registry.rs
  -> cargo build
  -> 生成 editor verification sidecar / player binary
  -> Runtime 通过静态 RuleRegistry 执行
```

C-min 第一版正式采用：

```text
静态生成 + 静态注册 + 重新编译 sidecar/player。
```

不采用：

```text
动态 DLL / SO / dylib 加载。
libloading 符号查找。
运行中编辑器进程内热替换规则函数。
手动注册项目规则函数作为正式闭环。
```

原因：

```text
动态加载会过早引入 ABI、平台签名、dll 生命周期和安全边界问题。
手动注册只能作为测试 fixture，不能作为 ProjectRuleAsset 管线的正式闭环。
静态生成 + 静态注册更像 Unity/UE 的第一版构建心智：修改脚本/蓝图后重新编译验证产物或 player。
```

第一版生成物规则：

```text
ProjectRulesGenerated/
  Cargo.toml 或 generated module 配置
  src/generated_rules.rs
  src/generated_registry.rs
  src/lib.rs
```

`generated_rules.rs` 只包含由 Canonical Rule IR 确定性生成的 Rust 函数。

`generated_registry.rs` 只负责：

```text
rule_id -> RustAotRule function pointer
rule_id -> artifact metadata
```

它不能保存业务逻辑，也不能手写项目规则。

编辑器验证路径：

```text
ProjectRuleAsset changed
  -> RuleCompiler 生成 generated_rules
  -> cargo build editor-rule-sidecar 或 headless player
  -> sidecar / headless player 加载 RuntimePackage
  -> ProjectLogicRunner 执行生成后的 Rust AOT 规则
  -> 输出 RuleValidationReport / RuntimeTrace
```

发布运行路径：

```text
ProjectRuleAsset changed
  -> RuleCompiler 生成 generated_rules
  -> cargo build player
  -> DesktopExportPipeline 打包 player + RuntimePackage
  -> Windows Player 启动
  -> ProjectLogicRunner 执行静态注册规则
```

这保证：

```text
编辑器验证和发布运行都走 Rust AOT。
不会出现 IR Interpreter 与 Rust AOT 双语义。
不会把手动注册 fixture 当成正式产品链路。
```

手动注册规则只允许存在于：

```text
单元测试
fixture
旧 M2 骨架兼容测试
```

并且必须标记为：

```text
derived artifact placeholder / test-only
```

不能成为项目规则真相层。

## 14. Canonical Rule IR v1 最小可编程表达

为了支撑复杂打飞机这种真实项目，C-min 的 IR 不能停留在静态操作列表。

第一版最小 JSON 形态建议：

```json
{
  "schemaVersion": "project-rule-ir.v1",
  "ruleId": "project.rule.fire_projectile",
  "phase": "Update",
  "enabled": true,
  "trigger": {
    "kind": "actionPressed",
    "actionId": "fire"
  },
  "statements": [
    {
      "kind": "operation",
      "operation": {
        "op": "instantiatePrefab",
        "prefabRef": "asset.prefab.projectile",
        "spawnTransform": {
          "fromEntity": "selected.player",
          "offset": { "type": "vec3", "x": 0.0, "y": 1.0, "z": 0.0 }
        }
      }
    }
  ],
  "sourceMap": {
    "featureId": "feature.fire_projectile"
  }
}
```

移动 projectile 的规则建议：

```json
{
  "schemaVersion": "project-rule-ir.v1",
  "ruleId": "project.rule.move_projectiles",
  "phase": "Update",
  "enabled": true,
  "trigger": { "kind": "always" },
  "statements": [
    {
      "kind": "forEachQuery",
      "query": {
        "all": ["Transform", "project.ProjectileMotion"]
      },
      "statements": [
        {
          "kind": "operation",
          "operation": {
            "op": "writeComponentField",
            "entity": "$entity",
            "componentType": "Transform",
            "fieldPath": "localPosition",
            "valueExpr": {
              "kind": "add",
              "left": { "kind": "field", "componentType": "Transform", "fieldPath": "localPosition" },
              "right": { "kind": "mul", "left": { "kind": "field", "componentType": "project.ProjectileMotion", "fieldPath": "velocity" }, "right": { "kind": "deltaTime" } }
            }
          }
        }
      ]
    }
  ]
}
```

第一版表达能力边界：

```text
支持 action_pressed / always / event_received 三类 trigger。
支持 for_each_query。
支持简单 value expression：literal、field、deltaTime、add、sub、mul。
支持 operation 顺序执行。
支持 emit_event 进入最小 RuntimeEventQueue。
```

第一版不支持：

```text
任意 while 循环。
递归。
任意函数调用。
跨规则隐式排序推断。
项目专用语义函数。
用户手写 Rust 作为真相层。
```

codegen 示例：

```rust
pub fn project_rule_fire_projectile(context: &mut LogicContext<'_>) -> LogicResult {
    let mut result = LogicResult::applied("project.rule.fire_projectile", ExecutorKind::RustAot);
    if !context.action_pressed("fire") {
        return result;
    }
    let command_id = context.instantiate_prefab(
        AssetRef::from("asset.prefab.projectile"),
        SpawnTransform::from_entity_with_offset(EntityId::from("selected.player"), Vec3 { x: 0.0, y: 1.0, z: 0.0 }),
    );
    result.commands.push(command_id.into());
    result
}
```

这段 Rust 不是用户手写真相层，而是 IR 派生物。用户和 AI 应修改 ProjectRuleAsset / Canonical Rule IR，而不是修改生成的 Rust。

## 15. Trace / Report 最小字段

第一版 Trace / Report 必须能回答：

```text
哪条规则执行了？
为什么执行？
读了哪些输入？
写了哪些组件字段？
发出了哪些 Command？
Command 是否成功 apply？
失败时对应哪个 ProjectRuleAsset / sourceMap？
```

建议最小字段：

```text
frame_index
phase
asset_id
rule_id
executor_kind
ir_hash
status
input_actions
writes
commands
diagnostics
source_map
```

## 16. 和复杂打飞机目标的关系

复杂打飞机需要项目规则，但不能变成引擎 API。

正确表达方式：

```text
按 fire action 生成 projectile prefab
projectile 每帧根据项目组件移动
collision event 触发项目规则
项目规则写 project-defined component 字段
AUI 读取项目状态 snapshot 显示分数 / 血量
```

错误表达方式：

```text
引擎内置 BulletSystem
引擎内置 EnemySystem
引擎内置 DamageSystem
引擎内置 ScoreSystem
```

C-min 的目标不是写出打飞机玩法，而是让 AI / 用户创建的打飞机项目规则可以稳定进入 Runtime。

## 17. C-min Gate 拆分

后续施工必须按 gate 拆分，且每个 gate 完成后做模块测试，再进入下一 gate。

### Gate A：ProjectRuleAsset / RuleAssetManifest 数据结构

目标：

```text
新增 ProjectRuleAsset v1。
新增 RuleAssetManifest v1。
明确 ProjectRuleAsset 是新建类型，不是当前已有能力。
ProjectRuleAsset 保存 canonicalIr / sourceMap / validation cache。
RuleAssetManifest 只做索引。
```

测试：

```text
ProjectRuleAsset JSON roundtrip。
RuleAssetManifest JSON roundtrip。
validation cache 变化不改变 IR hash。
```

### Gate B：Canonical Rule IR 最小可编程结构

目标：

```text
从 operations 静态列表升级到 trigger + statements + operations。
新增 action_pressed condition。
新增 for_each_query statement。
新增最小 value expression。
明确 emit_event 依赖 RuntimeEventQueue。
```

测试：

```text
fire action -> instantiate_prefab IR 能校验通过。
always + for_each_query -> write transform IR 能校验通过。
非法 project-specific engine API 名称会被 validation 拦截。
```

### Gate C：Rust AOT codegen

目标：

```text
由 trigger / statements / operations 生成 Rust source。
生成 if action_pressed 分支。
生成 for_each_query 循环。
生成 LogicContext write / command 调用。
生成 source map 注释或 metadata。
```

测试：

```text
fire action IR 生成包含 context.action_pressed 的 Rust。
projectile movement IR 生成 query loop。
生成代码快照稳定。
```

### Gate D：静态 artifact / registry 闭环

目标：

```text
写入 generated_rules crate / module。
生成 generated_registry.rs。
cargo build editor-rule-sidecar 或 headless player。
Runtime 使用静态 RuleRegistry 构建 ProjectLogicRunner。
```

测试：

```text
generated_rules 编译通过。
generated registry 能注册 rule_id。
ProjectLogicRunner 能执行生成规则。
```

### Gate E：RuntimePackage / Player 接入

目标：

```text
RuntimePackage 写入 RuntimeRuleManifest。
Windows Player / headless player 加载 manifest。
ProjectLogicRunner 从 manifest + registry 构建执行计划。
```

测试：

```text
RuntimePackage 包含 rule manifest。
headless player 加载 package 后执行规则。
Trace 能回指 asset_id / rule_id / sourceMap。
```

### Gate F：Editor / AI 入口最小接入

第一版不做完整 Rule Graph 和复杂 Rule Inspector。

第一版入口采用：

```text
ProjectRuleAsset JSON 文件。
AI Project Patch 写入 ProjectRuleAsset。
Workspace 能列出 rule asset。
Build / Verify 按 ProjectRuleAsset 触发编译。
```

后续再补：

```text
Rule Inspector
Rule Graph
复杂可视化调试器
```

测试：

```text
AI Patch 生成一个 ProjectRuleAsset。
Workspace 能读取 rule asset。
Build rule pipeline 能产出 report。
```

## 18. 现有文档影响

本文档对以下旧表达形成收敛：

```text
132-M2-Project-Rule-IR-Rust-AOT-Codegen-Incremental-Build-Runtime-Execute方案.md
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
05-逻辑系统边界-DSL-IR-RustAOT-ECS.md
23-IR-Interpreter-MVP.md
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
```

收敛规则：

```text
旧 M2 技术骨架保留。
正式产品化名称改为 Project Rule Asset Pipeline & Runtime Execution v1。
第一版执行主线改为 Rust AOT only。
IR Interpreter 不再作为 C-min 第一版编辑器验证主线。
项目规则内容属于项目侧，管线能力属于引擎侧。
```

## 19. 方案自审

### 是否合乎规格

合格。

本方案继续服务 `130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md` 中 M2 的 P0 目标，但把命名和边界收敛为更准确的项目规则资产管线。

### 是否合乎已有规则

合格。

方案遵守：

```text
引擎只提供底座能力。
不为特定项目增加规则。
Gameplay Rule Asset / Contract-bound RuleSlot 是用户和 AI 面向的规则资产边界。
Canonical Rule IR 是受限 RuleSlot 的内部规范语义和构建输入。
ProjectLogicRunner 是 Runtime 执行入口。
结构变化走 CommandBuffer。
```

### 是否避免重新讨论已定系统

基本合格。

本文不重开 ECS、Input、Prefab、AUI、RenderProjection、RuntimePackage 等系统，只定义项目规则资产如何进入这些已有底座。

### 是否方便实现

有条件合格。

现有代码已经具备：

```text
rule_ir.rs
rule_compiler.rs
rule_registry.rs
project_logic.rs
logic_executor.rs
gameplay_command.rs
runtime_package.rs
runtime_package_builder.rs
```

但必须明确：

```text
ProjectRuleAsset 还不存在，需要新增。
当前 RuleOperation 只有 WriteComponentField / SpawnEntity / DespawnEntity。
当前 RuleCompiler 只生成 Rust source 字符串，还没有 artifact / registry / player 静态闭环。
当前 Editor Rule Authoring 只应做 JSON / AI Patch 最小入口，不做完整 Rule Graph。
```

后续施工重点是产品化 ProjectRuleAsset / Manifest / Canonical Rule IR 最小可编程表达 / Rust AOT 静态闭环 / package integration，而不是推倒重写。

### 是否合理可维护

合格，但必须遵守 C-min 边界。

它避免了两种风险：

```text
把项目玩法塞进引擎 API。
同时维护 IR Interpreter 和 Rust AOT 两套第一版执行语义。
```

同时保留长期扩展口：

```text
后续可以补 Rule Graph。
后续可以补调试器。
后续可以补热更新后端。
后续可以补更复杂的规则类型。
```

这些扩展都必须继续以 Canonical Rule IR 为真相层。

### 审查修订结论

根据 `其它AI审查目录/13-186-Project-Rule-Asset-Pipeline方案审查.md`，本文档已补充：

```text
Rust AOT Artifact 生成与加载规则。
Canonical Rule IR v1 最小可编程表达。
ProjectRuleAsset 是新增类型的说明。
emit_event / action_pressed / for_each_query 不是既有能力，属于 C-min 施工内容。
Editor Authoring 第一版只做 JSON / AI Patch 最小入口。
C-min Gate 拆分。
```

修订后，186 可以作为后续施工文档的依据。

## 20. 推荐结论

正式采用：

```text
Project Rule Asset Pipeline & Runtime Execution C-min
```

作为 M2 后续产品化主线。

第一版目标不是“写项目玩法”，而是打通：

```text
ProjectRuleAsset
  -> Canonical Rule IR
  -> Rust AOT
  -> RuntimePackage
  -> ProjectLogicRunner
  -> ECS / CommandBuffer
  -> Trace / Report
```

这样复杂打飞机项目的规则可以由 AI / 用户在项目侧产生，再通过引擎的通用规则资产管线进入 Windows Player。
