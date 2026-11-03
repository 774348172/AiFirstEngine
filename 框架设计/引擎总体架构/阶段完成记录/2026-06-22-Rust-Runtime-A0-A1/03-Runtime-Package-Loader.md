# Runtime Package Rust Loader

## 功能目标

让 Rust Runtime 能读取当前 Build Graph 生成的 Runtime Package v1：

```text
runtime-package/manifest.json
runtime-package/scenes/<scene-id>.json
runtime-package/assets/asset-manifest.json
runtime-package/rules/rule-manifest.json
```

Loader 必须返回结构化 diagnostics，不能 panic。

## 实现文件

```text
rust/crates/engine_runtime/src/diagnostics.rs
rust/crates/engine_runtime/src/runtime_package.rs
scripts/test-rust-runtime-package-loader.cjs
```

相关 npm script：

```text
test:rustruntimepackage
```

## 关键实现

```text
RuntimePackageManifest / RuntimeScene / RuntimeEntity / RuntimeAssetManifest / RuntimeRuleManifest。
读取 manifest 中的 scene path / asset manifest path / rule manifest path。
校验 schemaVersion。
校验 activeSceneId。
校验 entity transform。
校验 model mesh.assetRef。
校验 assetRef 是否存在于 asset manifest。
校验 rules.mode == none。
```

## 最小测试用例

命令：

```powershell
npm.cmd run test:rustruntimepackage
```

覆盖测试：

```text
runtime_package_loader_loads_valid_package
invalid_runtime_package_reports_diagnostics
missing_transform_reports_diagnostics
rule_mode_must_be_none
```

本轮结果：

```text
Rust Loader 可以加载有效 Runtime Package。
缺 assetRef / 缺 transform / 非 none rule mode 都能返回 diagnostics。
测试通过。
```

## 边界

```text
不做 Prefab 展开。
不做 Component lowering。
不做真实资源二进制加载。
不做 Rule compile。
不做 IR AOT。
```
