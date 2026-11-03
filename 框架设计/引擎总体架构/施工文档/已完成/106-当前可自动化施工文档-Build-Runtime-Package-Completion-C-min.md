# 106-当前可自动化施工文档-Build / Runtime Package Completion C-min

## 1. 施工目标

承接：

```text
../106-Build-Runtime-Package-Completion-C-min方案.md
```

实现第一版 Runtime Package 内容转换、验证、diff、报告闭环。

本次只做 C-min：

```text
RuntimePackageBuildRequest
RuntimePackageBuilder
RuntimePackageManifest v1 写出
RuntimeAssetIndex / asset-manifest 写出
Scene / Prefab / Input / Schema / Rule / AUI 最小写出
RuntimePackageValidationReport
RuntimePackageDiffReport
BuildRuntimePackageReport
最小 headless tests
RuntimePackageLoader 回归验证
```

## 2. 不做内容

本次不做：

```text
真实 Android / iOS package
真实签名
真实安装器
真实 zip / pak / obb
真实 ASTC / Basis 转码
真实 mesh LOD cook
复杂 prefab variant 展开
字段级 sourceMap
复杂 provenance graph
IR -> Rust AOT 编译
热更包生成
Bundle binary pack
Runtime 执行项目逻辑
```

## 3. 代码落点

优先放在：

```text
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/lib.rs
```

原因：

```text
engine_runtime 已经拥有 runtime_package / runtime_asset 的正式 Runtime Package schema 和 loader。
Builder 必须复用同一套 manifest / scene / asset / rule 格式。
施工不应在 editor_core 里创造第二套 Runtime Package schema。
```

## 4. 数据结构

新增：

```text
RuntimePackageBuildRequest
RuntimePackageBuildInput
RuntimePackageSourceAsset
RuntimePackageSourceScene
RuntimePackageSourcePrefab
RuntimePackageSourceInputMapping
RuntimePackageSourceSchema
RuntimePackageSourceRuleManifest
RuntimePackageSourceAuiManifest
RuntimePackageBuilder
RuntimePackageBuildReport
RuntimePackageBuildStageReport
RuntimePackageValidationReport
RuntimePackageDiffReport
RuntimePackageDiagnostic
```

第一版可以用 `serde_json::Value` 承载 Prefab / Input / Schema / AUI 的具体内容，避免提前扩大 schema 复杂度。

## 5. 核心流程

```text
RuntimePackageBuildRequest + RuntimePackageBuildInput
  -> create package folders
  -> write manifest.json
  -> write scenes/scene-*.json
  -> write prefabs/prefab-*.json
  -> write assets/asset-manifest.json
  -> write assets/runtime-asset-index.json
  -> write input/input-mapping-*.json
  -> write schema/component-schema.json
  -> write rules/rule-manifest.json
  -> write aui/aui-manifest.json
  -> validate package
  -> diff previous manifest if provided
  -> write reports/*
  -> return RuntimePackageBuildReport
```

## 6. 验证规则

第一版必须验证：

```text
activeSceneId 存在。
Scene id 稳定且不为空。
Entity id 稳定且不为空。
Entity id 不重复。
Entity transform 存在。
AssetRef 可在 source assets 中解析。
asset id 不为空。
asset id 不重复。
Rule manifest 可写出。
```

验证失败时：

```text
RuntimePackageBuildReport.status = failed
RuntimePackageValidationReport.status = failed
仍写出 report 文件
不伪装成功
```

## 7. 测试任务

### A1：最小 Scene Package

输入：

```text
1 个 scene
1 个 entity
Transform
mesh.textureRef
1 个 texture asset
```

断言：

```text
manifest.json 存在。
scenes/scene-main.json 存在。
assets/asset-manifest.json 存在。
assets/runtime-asset-index.json 存在。
RuntimePackageLoader 可以读取。
BuildRuntimePackageReport.status = success。
```

### A2：Prefab + Input + Schema + AUI

输入：

```text
1 个 prefab
1 个 input mapping JSON
1 个 component schema JSON
1 个 aui manifest JSON
```

断言：

```text
prefabs/*.json 存在。
input/*.json 存在。
schema/component-schema.json 存在。
aui/aui-manifest.json 存在。
validation success。
```

### A3：Missing AssetRef

输入：

```text
Scene 中 textureRef 指向不存在 asset。
```

断言：

```text
BuildRuntimePackageReport.status = failed。
diagnostic.code = MissingAssetRef。
validation report 写出。
RuntimePackageLoader 不作为成功 gate。
```

### A4：Diff Report

输入：

```text
previous manifest hash = old。
current package hash != old。
```

断言：

```text
runtime-package-diff-report.json 存在。
changes 包含 manifest modified。
```

## 8. 施工完成标准

```text
cargo fmt --check
cargo test -p engine_runtime runtime_package_builder
cargo test -p engine_runtime runtime_package
cargo test -p engine_runtime
cargo test --workspace --no-run
```

如果时间允许，再跑：

```text
cargo test --workspace
```

## 9. 完成后文档动作

施工完成后：

```text
移动本文档到 施工文档/已完成/
新建 阶段完成记录/2026-06-29-Build-Runtime-Package-Completion-C-min/00-总览.md
更新 49 / 54 / 00-文档地图 / README 顶部状态
```

