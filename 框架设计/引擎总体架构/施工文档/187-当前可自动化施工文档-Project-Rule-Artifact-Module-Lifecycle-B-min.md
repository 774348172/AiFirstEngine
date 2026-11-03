# 187-当前可自动化施工文档-Project Rule Artifact & Module Lifecycle B-min

## 1. 施工目标

根据 `187-Project-Rule-Artifact-Module-Lifecycle-B-min方案.md`，补齐 Project Rule 的产物生命周期边界：

```text
Canonical Rule IR
  -> RuleCompileReport
  -> RuleArtifactManifest
  -> RuleArtifactRegistry
  -> RuleModuleRegistry
  -> ProjectLogicRunner
```

本次只实现 B-min：

```text
StaticRegistry 是 Windows v1 默认执行路径。
DynamicValidationHost 只作为未来 module_kind 占位，不执行动态加载。
```

## 2. 施工范围

### 模块 A：新增 rule_artifact 模块

新增：

```text
rust/crates/engine_runtime/src/rule_artifact.rs
```

定义：

```text
RULE_ARTIFACT_MANIFEST_SCHEMA_VERSION
RuleArtifactManifest
RuleArtifactManifestEntry
RuleArtifactStatus
RuleArtifactDiagnostic
RuleArtifactRegistry
RuleModuleLifecycleState
RuleArtifactValidationReport
```

提供能力：

```text
expected_rule_artifact_id(rule_id, ir_hash)
RuleArtifactManifest::from_compile_reports(...)
RuleArtifactRegistry::from_manifest(...)
validate_runtime_rule_manifest_artifacts(...)
```

### 模块 B：接入 runtime package 校验

在 `runtime_package.rs` 的 rule manifest 校验里增加：

```text
artifact_id 必须等于 rule-artifact:{rule_id}:{ir_hash}
rust-aot 模式下 enabled rule 必须能匹配 module artifact
StaticRegistry 可以没有 path
DynamicValidationHost 必须有 path，但第一版不执行
```

### 模块 C：接入 RuleModuleRegistry

在 `rule_registry.rs` 中增加 artifact lifecycle 校验：

```text
build_runner 先校验 RuntimeRuleManifest 的 artifact/module 一致性。
enabled rule 缺少注册函数时仍然报 missing_registered_rule。
unsupported module_kind 报 unsupported_rule_module_kind。
```

### 模块 D：ProjectRuleAsset 产物 ID 收敛

修正 `project_rule_asset.rs`：

```text
runtime_rule_manifest_entry_from_asset 使用 rule-artifact:{rule_id}:{ir_hash}
runtime_rule_manifest_from_assets 的 module artifact_id 与规则 artifact_id 保持可校验关系
```

### 模块 E：测试

新增/更新测试：

```text
cargo test -p engine_runtime rule_artifact
cargo test -p engine_runtime rule_registry
cargo test -p engine_runtime project_rule_asset
cargo test -p engine_runtime runtime_package_loader_accepts_rust_aot_rule_manifest
cargo test -p engine_runtime m2_rule_demo
cargo test -p engine_runtime
```

## 3. 验收标准

必须满足：

```text
1. IR hash 和 artifact_id 不一致时加载或构建 runner 会失败。
2. rule manifest 引用了没有 module 声明的 artifact 时会失败。
3. StaticRegistry 路径继续可运行。
4. DynamicValidationHost 不会被误当成真实运行路径。
5. 现有 M2 demo 仍然通过。
```

## 4. 不施工范围

本次不做：

```text
真实 DLL 加载
运行时编译
热替换
跨平台动态卸载
Interpreter 后端
新的项目玩法 API
```

## 5. 施工文档自审

### 是否符合方案

通过。施工项对应 187 方案里的 manifest、registry、lifecycle、StaticRegistry 默认路径。

### 是否会引入临时路线

通过。StaticRegistry 是 v1 执行方式，但被 manifest / artifact_id / ABI 约束，不再是绕过规则的临时路径。

### 是否影响正常 Runtime

影响可控。新增校验只在规则 manifest 存在时生效，`mode = none` 或没有规则的包不受影响。

### 是否可测试

通过。新增模块、registry、package loader、M2 demo 均可用单元测试验证。

