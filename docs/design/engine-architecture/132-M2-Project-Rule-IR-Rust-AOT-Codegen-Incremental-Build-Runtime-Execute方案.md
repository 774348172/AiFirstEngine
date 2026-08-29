# 132-M2 Project Rule IR -> Rust AOT Codegen / Incremental Build / Runtime Execute 方案

## 当前修正：195 / 196 优先

本文保留为 Project Rule AOT 管线的早期技术骨架。按 `195` / `196` 的当前规则：

```text
用户心智是 Rust Project Framework + Project Assets。
Gameplay Rule Asset / Contract-bound RuleSlot 是用户和 AI 面向的规则资产边界。
Canonical Rule IR 只是受限 RuleSlot 的内部规范语义和构建输入。
本文的 IR -> Rust AOT 链路只适用于受限规则片段，不承载完整 gameplay 流程、复杂算法或复杂 UI 工作流。
```

## 1. 问题是什么

`130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md` 中的 M2 目标是：

```text
Project Rule Authoring / Compile / Runtime Execute
```

本系统解决的是项目规则如何从 AI / 编辑器生成的结构化规则进入 Runtime，并在 Windows Player 和编辑器验证中执行。

正式链路：

```text
Natural Language / AI Request
  -> Project Model
  -> Canonical Rule IR
  -> RuleCompiler
       -> Rust Source Codegen
       -> Rust Incremental Build
       -> Rule Artifact
  -> Rule Runtime Manifest
  -> RuleModuleRegistry
  -> ProjectLogicRunner
  -> LogicContext
  -> ECS World
  -> Trace / Report
```

## 2. 唯一真相层

正式规则：

```text
Gameplay Rule Asset / Contract-bound RuleSlot 是用户和 AI 面向的规则资产边界。
Canonical Rule IR 是受限 RuleSlot 的内部规范语义和构建输入。
```

以下对象都不是规则真相：

```text
Generated Rust Source
Rust AOT Artifact
Rule Runtime Manifest
RuleModuleRegistry
ProjectLogicRunner
Trace / Report
```

它们分别是：

```text
Generated Rust Source：由 IR 确定性生成的派生代码。
Rust AOT Artifact：由 Generated Rust 编译出的派生产物。
Rule Runtime Manifest：运行索引，不保存业务逻辑。
RuleModuleRegistry：rule_id 到 Rust 函数的注册表。
ProjectLogicRunner：Runtime 调项目逻辑的唯一入口。
Trace / Report：诊断结果。
```

## 3. 引擎侧与项目侧边界

引擎侧只提供通用底座：

```text
IR schema
RuleCompiler
Rust source generation
Incremental build cache
Rule manifest
Rule registry
ProjectLogicRunner integration
LogicContext
Query / Read / Write / CommandBuffer
Trace / Report
```

项目侧负责：

```text
具体组件 schema
具体规则 IR
具体 prefab / scene / asset / input / AUI
具体业务语义
```

禁止把以下项目语义加入引擎 API：

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

这些只能作为项目 schema / component / rule / prefab / asset 存在。

## 4. 其它引擎参考

### Unreal Engine

UE 的项目逻辑通过 C++ / Blueprint 进入 Actor / Component Tick，再由 TickTaskManager 调度。

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\TickTaskManager.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Actor.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Components\ActorComponent.cpp
```

可学习：

```text
运行入口统一。
TickGroup / prerequisite 能表达复杂顺序。
C++ / Blueprint 最终都挂入统一 Runtime 生命周期。
```

不采用：

```text
第一版不做 UE Live Coding 式进程内 patch。
第一版不做复杂 Tick prerequisite 图。
```

### Unity

Unity 的项目逻辑通过 C# 脚本编译后进入 PlayerLoop。

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\PlayerLoop\PlayerLoop.bindings.cs
```

可学习：

```text
用户心智简单。
编辑器运行和发布运行都围绕同一类脚本执行产物。
```

不采用：

```text
不复制 C# Domain Reload。
不让脚本源码成为唯一真相。
```

### Bevy

Bevy 通过 System / Schedule / World 执行项目逻辑。

源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_ecs\src\schedule
```

可学习：

```text
ECS Query / Commands / apply_deferred 边界清楚。
```

不采用：

```text
不把完整 Rust ECS Schedule 心智直接暴露给普通用户和 AI。
```

### Godot

Godot 通过 SceneTree / Node lifecycle / _process / _physics_process 执行脚本。

可学习：

```text
生命周期入口直观。
```

不采用：

```text
不采用 Node 脚本解释执行作为底层主路线。
```

### Rust / Cargo

Rust 代码生成通常通过 build script / OUT_DIR / rerun-if-changed 管理派生文件；增量编译依赖 rustc / cargo 缓存。

本项目采用等价思想：

```text
IR hash 没变，不重新生成。
Rule compiler ABI / schema / target 没变，不重新编译。
派生产物放入 target/ai-engine/generated-rules。
```

## 5. 推荐方案

采用：

```text
C-min：IR 真相层 + Rust AOT Codegen + 增量编译 + 编辑器 Sidecar 验证 + 发布静态注册
```

第一版做真实主干：

```text
Rule IR schema v1
RuleCompiler v1
Rust source codegen v1
Incremental build cache v1
Rule manifest v1
RuleModuleRegistry v1
ProjectLogicRunner 接入
Trace / Report
```

第一版不做：

```text
IR Interpreter
热更解释执行
UE Live Coding 式进程内 patch
复杂可视化 Rule Graph
复杂规则自动排序
项目玩法专用 API
```

## 6. Rule IR v1

Rule IR v1 第一版只支持通用操作。

最小结构：

```json
{
  "schemaVersion": "project-rule-ir.v1",
  "ruleId": "project.rule.move",
  "phase": "Update",
  "enabled": true,
  "operations": [
    {
      "op": "writeComponentField",
      "entityId": "entity-a",
      "componentType": "Transform",
      "fieldPath": "localPosition",
      "value": { "type": "vec3", "x": 1.0, "y": 0.0, "z": 0.0 }
    }
  ]
}
```

第一版 operation：

```text
writeComponentField
spawnEntity
despawnEntity
```

后续 operation 必须保持通用命名，不引入项目语义。

## 7. RuleCompiler v1

RuleCompiler 分三步：

```text
Validate IR
Generate Rust Source
Build Rust Artifact
```

每一步都必须输出 report：

```text
RuleCompileReport
  status
  rule_id
  ir_hash
  generated_source_path
  artifact_id
  diagnostics
```

## 8. 增量编译规则

缓存 key：

```text
rule_id
ir_hash
schema_version
compiler_version
engine_rule_abi_version
target
build_profile
```

缓存命中：

```text
不重新生成 Rust source。
不重新执行 cargo build。
保留旧 artifact_id。
```

缓存未命中：

```text
重新生成。
重新编译。
重新写 report。
```

第一版施工可以先实现 deterministic cache decision 和 source generation gate；真实 cargo 编译 gate 独立实现，但接口必须按真实编译设计。

## 9. 模块注册

发布版：

```text
GeneratedRuleRegistry
  rule_id -> fn(&mut LogicContext) -> LogicResult
```

编辑器验证：

```text
Editor
  -> RuleBuildService
  -> RuleValidationHost
  -> RuntimePackage path
  -> scenario path
  -> validation report json
```

动态验证边界不直接传 Rust 内部类型：

```text
禁止跨动态库直接传 World / LogicContext / ComponentValue / RuntimePackage。
只允许传 package_path / scenario_path / output_report_path。
```

## 10. Runtime 执行规则

`ProjectLogicRunner` 是唯一执行入口。

规则只能通过 `LogicContext` 访问：

```text
query
read_component
write_component
write_component_field
command_buffer
input_action
time
trace
```

结构变化必须通过 CommandBuffer。

## 11. Trace / Report

每条规则执行至少记录：

```text
rule_id
phase
executor
ir_hash
queries
reads
writes
commands
errors
```

Codegen / Build / Register 阶段也必须输出诊断，方便 AI 和用户定位：

```text
IR invalid
Codegen unsupported op
Build failed
Rule missing from registry
Rule execution failed
```

## 12. 最小验收

至少用三个 demo 验证：

```text
Demo A：write Transform.localPosition 成功。
Demo B：spawn generic entity 成功，并通过 CommandBuffer apply。
Demo C：非法 field path / unsupported op 生成明确诊断，不产生可执行 artifact。
```

验收必须证明：

```text
IR 是唯一输入。
Rust source 是确定性派生产物。
缓存 key 可判断是否需要重编。
RuleManifest 可进入 RuntimePackage。
ProjectLogicRunner 能执行注册后的 AOT rule。
Trace / Report 能回指 rule_id / ir_hash。
```
