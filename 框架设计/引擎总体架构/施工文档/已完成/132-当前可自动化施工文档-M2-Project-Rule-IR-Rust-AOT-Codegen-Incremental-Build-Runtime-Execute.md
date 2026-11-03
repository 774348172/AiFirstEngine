# 当前可自动化施工文档：M2 Project Rule IR -> Rust AOT Codegen / Incremental Build / Runtime Execute

## 1. 施工目标

根据：

```text
132-M2-Project-Rule-IR-Rust-AOT-Codegen-Incremental-Build-Runtime-Execute方案.md
```

实现 M2 第一版可验证主干：

```text
Rule IR v1
  -> RuleCompiler v1
  -> Rust source codegen
  -> incremental cache decision
  -> RuleRuntimeManifest
  -> RuleModuleRegistry
  -> ProjectLogicRunner execution
  -> Trace / Report
```

## 2. 涉及文件

预计新增：

```text
rust/crates/engine_runtime/src/rule_ir.rs
rust/crates/engine_runtime/src/rule_compiler.rs
rust/crates/engine_runtime/src/rule_registry.rs
```

预计修改：

```text
rust/crates/engine_runtime/src/lib.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/project_logic.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
```

## 3. Gate A：Rule IR v1

实现：

```text
ProjectRuleIr
RulePhase serialization
RuleOperation
RuleRuntimeValue
RuleIrValidationReport
stable_ir_hash
```

最小 operation：

```text
writeComponentField
spawnEntity
despawnEntity
```

测试：

```powershell
cargo test -p engine_runtime rule_ir
```

## 4. Gate B：RuleCompiler v1 / Rust source codegen

实现：

```text
RuleCompiler
RuleCompileRequest
RuleCompileReport
generated Rust source string
generated source path convention
diagnostics for unsupported op / invalid IR
```

测试：

```powershell
cargo test -p engine_runtime rule_compiler
```

## 5. Gate C：Incremental Build Decision

实现：

```text
RuleBuildCacheKey
RuleBuildCacheRecord
RuleBuildDecision
cache hit / miss 判断
compiler_version / engine_rule_abi_version 参与 key
```

第一版可不真实调用 cargo，但接口必须按真实构建设计。

测试：

```powershell
cargo test -p engine_runtime rule_compiler
```

## 6. Gate D：RuleRuntimeManifest / RuntimePackage 接入

实现：

```text
RuntimeRuleManifest 强类型化
RuntimeRuleManifestEntry
phase / enabled / executor / ir_hash / artifact_id / source_map
RuntimePackageBuilder 写入 rule manifest
RuntimePackageLoader validate rule manifest
```

测试：

```powershell
cargo test -p engine_runtime runtime_package
cargo test -p engine_runtime runtime_package_builder
```

## 7. Gate E：RuleModuleRegistry / ProjectLogicRunner 执行接入

实现：

```text
RuleModuleRegistry
register_generated_rule
build RuleExecutionPlan from RuntimeRuleManifest
ProjectLogicRunner::from_rule_manifest_and_registry
missing rule diagnostics
```

测试：

```powershell
cargo test -p engine_runtime rule_registry
cargo test -p engine_runtime project_logic
```

## 8. Gate F：三个小型 demo 测试

Demo A：IR 写 Transform.localPosition。

验收：

```text
IR -> generated Rust source -> registry -> runner -> ECS write -> trace
```

Demo B：IR spawn generic entity。

验收：

```text
IR -> generated Rust source -> CommandBuffer -> apply -> world contains spawned entity
```

Demo C：非法 IR。

验收：

```text
unsupported op / invalid field path 输出明确 diagnostics，不产生成功 artifact。
```

测试：

```powershell
cargo test -p engine_runtime m2_rule
```

## 9. Gate G：文档归档

完成后：

```text
施工文档/当前/132-...md -> 施工文档/已完成/132-...md
新增 阶段完成记录/2026-07-01-M2-Project-Rule-IR-Rust-AOT-Codegen-Incremental-Build-Runtime-Execute/00-总览.md
更新 49 / 54 / 施工文档 README
```

