# 186 当前可自动化施工文档：Project Rule Asset Pipeline & Runtime Execution C-min

依据：

```text
../186-Project-Rule-Asset-Pipeline-Runtime-Execution-C-min方案.md
../../其它AI审查目录/13-186-Project-Rule-Asset-Pipeline方案审查.md
```

## 1. 施工目标

本轮目标不是实现项目玩法，也不是完整 Blueprint / Rule Graph。

本轮目标是打通 C-min 的引擎侧规则资产管线：

```text
ProjectRuleAsset
  -> Canonical Rule IR 最小可编程结构
  -> Rule validation
  -> Rust AOT source / static registry codegen
  -> RuntimeRuleManifest / RuntimePackage 索引
  -> RuleModuleRegistry / ProjectLogicRunner
  -> LogicContext / ECS / CommandBuffer
  -> Trace / Report
```

## 2. 边界

允许实现：

```text
ProjectRuleAsset / RuleAssetManifest 数据结构
RuleTrigger / RuleStatement / RuleOperation 最小表达
action_pressed / always trigger
for_each_query statement
write_component_field / spawn_entity / despawn_entity / instantiate_prefab / despawn_prefab_instance
RuleCompiler codegen
Generated static registry source
Runtime manifest metadata
headless/runtime 单元测试
```

暂不实现：

```text
完整 Rule Graph UI
动态 DLL / libloading
热更新 VM
IR Interpreter 正式执行
任意 while / recursion / user Rust truth
项目专用 Player / Enemy / Bullet / Damage / Score API
```

## 3. Gate A：ProjectRuleAsset / RuleAssetManifest

施工：

```text
新增 engine_runtime::project_rule_asset 模块。
新增 ProjectRuleAsset / RuleAssetManifest / RuleAssetManifestEntry。
ProjectRuleAsset 包含 canonical_ir / source_map / validation。
RuleAssetManifest 只保存索引，不保存业务逻辑。
导出模块并补 JSON roundtrip 测试。
```

验收：

```text
cargo test -p engine_runtime project_rule_asset
```

## 4. Gate B：Canonical Rule IR 最小可编程结构

施工：

```text
扩展 rule_ir.rs：
  ProjectRuleIr 增加 trigger / statements。
  保留 legacy operations 兼容旧 M2 测试。
  新增 RuleTrigger / RuleCondition / RuleStatement / QuerySpecLiteral / RuleValueExpr。
  新增 InstantiatePrefab / DespawnPrefabInstance / EmitEvent operation。
  validation 覆盖 trigger、statement、field path、query。
```

验收：

```text
cargo test -p engine_runtime rule_ir
```

## 5. Gate C：Rust AOT codegen

施工：

```text
扩展 RuleCompiler：
  trigger action_pressed -> if !context.action_pressed(...) return skipped/applied
  for_each_query -> context.query(QuerySpec::all(...)) loop
  write_component_field 支持 entity = "$entity"
  instantiate_prefab / despawn_prefab_instance 生成 LogicContext command 调用
  生成 static registry source helper
```

验收：

```text
cargo test -p engine_runtime rule_compiler
```

## 6. Gate D/E：静态 registry / RuntimePackage 最小接入

施工：

```text
保持 RuleModuleRegistry 当前函数指针注册机制。
新增 generated registry source 生成能力，作为静态 registry 代码产物。
RuntimeRuleManifest 继续作为运行时索引。
ProjectRuleAsset -> RuntimeRuleManifestEntry helper。
```

验收：

```text
cargo test -p engine_runtime rule_registry runtime_package
```

## 7. Gate F：Editor / AI 最小入口

本轮只做引擎侧文件入口，不做 UI：

```text
ProjectRuleAsset JSON 可读写。
AI Patch / Workspace 后续可以写入该 JSON。
Build / Verify 后续通过这些 asset 触发编译。
```

验收：

```text
cargo test -p engine_runtime project_rule_asset
```

## 8. 整体测试

施工完成后运行：

```text
cargo test -p engine_runtime
```

如果影响 workspace 公共接口，再运行：

```text
cargo test
```

## 9. 施工顺序

严格顺序：

```text
1. Gate A，测试通过。
2. Gate B，测试通过。
3. Gate C，测试通过。
4. Gate D/E，测试通过。
5. Gate F，测试通过。
6. 整体测试。
```

每个 gate 失败时先修当前 gate，不跳到下一个 gate。

## 10. 自审

合格。

本文档没有引入项目玩法 API，没有要求动态加载，没有重启 IR Interpreter 主线，且每个 gate 都有可执行测试。
