# 106-Build / Runtime Package Completion C-min 方案

## 1. 问题是什么

当前已经有三类相关规则：

```text
07-Build-Export-Pipeline.md
  定义 Build / Export 的总路线。

72-Build-Run-Package-Orchestrator-v1方案.md
  定义 BuildRunRequest -> staged run folder -> Runtime 启动的编排器。

93-复杂打飞机验证所需引擎侧缺失能力清单.md
  指出 Build / Runtime Package 还缺少完整补齐。
```

这次讨论的系统不是重新设计 Build Pipeline，也不是替代 BuildRun Orchestrator。它只补齐中间缺口：

```text
Editor / Project Authoring Data
  -> Build Graph content conversion
  -> Runtime Package
  -> Package Validation
  -> Runtime Package Diff Report
  -> staged run folder 可被 Runtime 读取
```

正式命名：

```text
Build / Runtime Package Completion C-min
```

它的核心职责是：把已经编辑好的 Scene / Prefab / Asset / InputMapping / Schema / Rule manifest 等项目内容，转换成 Runtime 可以稳定读取、可以验证、可以对比、可以被 AI 查错的 Runtime Package。

## 2. 引擎侧与项目侧边界

### 2.1 引擎侧负责

```text
读取项目资产数据库和编辑器保存后的项目文件。
生成 Runtime Package 目录结构。
生成 RuntimeAssetIndex / asset-manifest。
生成 scene package。
生成 prefab package。
生成 input mapping package。
生成 schema package。
生成 rule manifest package。
执行 package validation。
生成 runtime package diff report。
生成 BuildRuntimePackageReport。
把报告交给 BuildRunReport / Console / AI 读取。
```

### 2.2 项目侧负责

```text
具体资源内容。
具体场景内容。
具体 Prefab 内容。
具体输入 action 命名。
具体组件 schema。
具体项目规则内容。
具体生命周期规则内容。
```

引擎不因为某个验证项目而新增玩法概念。本系统只使用通用概念：

```text
asset
assetRef
scene
prefab
entity
component
schema
rule
manifest
package
validation
diff
report
```

## 3. 成熟引擎参考

### 3.1 Unreal Engine

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\AutomationUtils\ProjectParams.cs
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\AutomationUtils\Platform.cs
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\Turnkey\Commands\CreateBuild.cs
```

UE 的路线可以概括为：

```text
BuildCookRun
  -> Cook
  -> Stage
  -> Package
  -> Deploy / Run
```

对我们最有价值的是：

```text
Cook 和 Stage 是明确分层的。
运行时读取 cooked / staged 内容，而不是编辑器内存对象。
Stage 目录是 Run / Deploy / Package 的共同输入。
构建过程产生结构化参数和错误。
```

我们的 `72 Build / Run Package Orchestrator` 已经学习了 UE 的 BuildCookRun / Stage / Run 结构。本系统继续学习 UE 的一点是：先把内容 cook / normalize 成运行时视图，再交给 staged run folder。

### 3.2 Unity

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPlayerWindowBuildMethods.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPipeline\BuildPipeline.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Modules\BeeBuildPostprocessor.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPlayerSceneTreeView.cs
```

Unity 的路线可以概括为：

```text
BuildPlayerOptions
  -> BuildPipeline.BuildPlayer
  -> Player
  -> Data folder
  -> BuildReport
```

对我们有价值的是：

```text
Build 设置对用户表现得简单。
底层会生成 Player 和数据目录。
BuildReport 是构建查错入口。
场景列表和资源依赖会进入构建输入。
```

Unity 的风险是 Editor Play 和正式 Build 底层路径不完全一致。我们的规则要避免这个问题：

```text
Editor Play / Preview 和正式 Run 都尽量读取同一种 Runtime Package。
```

### 3.3 Godot

本地源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\editor\editor_node.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\export
<GODOT_SOURCE>\godot-master\godot-master\editor\file_system\editor_file_system.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\import
<GODOT_SOURCE>\godot-master\godot-master\editor\run\editor_run_native.cpp
```

Godot 的路线可以概括为：

```text
ResourceImporter
  -> EditorFileSystem
  -> ExportPreset
  -> EditorExportPlatform
  -> export / run native
```

对我们有价值的是：

```text
资源导入和导出是明确系统。
ExportPreset 定义平台导出策略。
运行设备 / 平台通过 export platform 统一接入。
```

我们的第一版不做完整平台导出，但要保留 BuildProfile / target / package manifest / report。

### 3.4 Bevy

本地源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_asset
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_winit
```

Bevy 的路线更偏：

```text
cargo build / cargo run
  -> App
  -> AssetPlugin / AssetServer
  -> runtime load assets
```

它的运行时 AssetServer 很清晰，但不是我们要学习的主要 Build Package 方向，因为我们需要编辑器、AI 报告、Runtime Package、可验证构建产物这一整套闭环。

## 4. 方案对比

### 方案 A：只沿用 72，不新增本系统

```text
BuildRunOrchestrator 里顺手写 Runtime Package。
```

优点：

```text
文档和模块少。
第一版看起来快。
```

缺点：

```text
Orchestrator 会膨胀。
Build / Run 编排和内容转换混在一起。
后续 cook sprite / prefab / input / schema / rule manifest 时难维护。
AI 查错会分不清是编排错误还是内容转换错误。
```

不推荐。

### 方案 B：完整平台 Package 系统一次到位

```text
直接做桌面 / Web / Android / iOS 完整 package、签名、压缩包、资源格式转换。
```

优点是长期能力完整。缺点是第一版过大，会把平台包格式、签名、真实压缩、真实纹理转码和 Runtime Package 基础结构混在一起。

不推荐第一版采用。

### 方案 C-min：Runtime Package Completion 独立内容转换层

```text
BuildRunOrchestrator
  -> 调用 RuntimePackageBuilder
  -> RuntimePackageBuilder 负责内容转换 / validation / diff / report
  -> Orchestrator 只负责 stage / launch / aggregate report
```

优点：

```text
边界清楚。
接近 UE Cook / Stage 的分层思想。
AI 能单独阅读 RuntimePackageReport。
复杂项目可以逐步增加 asset kind / schema kind / rule manifest，但入口不变。
Editor Preview 和 Runtime Run 可以共用同一种 package。
```

缺点：

```text
比方案 A 多一个正式模块。
需要维护 RuntimePackageBuilder 的输入 / 输出 schema。
```

推荐采用。

## 5. 最终选择

采用：

```text
方案 C-min：Runtime Package Completion 独立内容转换层。
```

正式结构：

```text
Project Saved Data / Asset DB / Editor Scene Document
  -> RuntimePackageBuildRequest
  -> RuntimePackageBuilder
  -> RuntimePackage
  -> RuntimePackageValidationReport
  -> RuntimePackageDiffReport
  -> BuildRuntimePackageReport
  -> BuildRunOrchestrator StageRunFolder
```

## 6. 与现有文档关系

### 6.1 与 07 的关系

`07-Build-Export-Pipeline.md` 是总规则。本系统是其中 `write_runtime_package` / `cook_project_data` 的 C-min 细化。

### 6.2 与 72 的关系

`72-Build-Run-Package-Orchestrator-v1方案.md` 是编排器。

本系统不负责启动 Runtime，不负责选择 executable，不负责 process spawn。`72` 调用本系统，拿到结果后继续：

```text
StageRunFolder
LaunchRuntime
WriteBuildRunReport
```

### 6.3 与 Asset Pipeline 的关系

Asset Pipeline / Importer 负责：

```text
源文件 -> imported asset record / asset db / import report
```

本系统负责：

```text
asset db / imported asset record -> runtime asset manifest / runtime asset index
```

第一版不做真实纹理压缩、mesh LOD、平台二进制包。

### 6.4 与 Runtime 的关系

Runtime 只读取：

```text
runtime-package
cooked-assets
```

Runtime 不读取：

```text
editor project object
editor scene document memory
asset pipeline transient state
inspector state
AI conversation
```

## 7. C-min 输入

### 7.1 RuntimePackageBuildRequest

第一版最小字段：

```json
{
  "schemaVersion": "runtime-package-build-request.v1",
  "projectRoot": "project",
  "activeSceneId": "scene_main",
  "target": "dev-desktop",
  "mode": "dev-run",
  "outputDir": "dist/dev-desktop/runtime-package",
  "previousPackageManifest": null,
  "includeDebugReadableJson": true
}
```

规则：

```text
target 第一版只要求 dev-desktop。
mode 第一版只要求 dev-run。
includeDebugReadableJson 第一版必须为 true。
previousPackageManifest 用于 diff report，可以为空。
```

### 7.2 Builder 输入源

第一版允许输入：

```text
Project Asset DB
Saved Scene Document
Saved Prefab Document
InputMapping Asset
Project Schema
Project Rule Manifest
AUI Document Manifest
```

不允许输入：

```text
Editor UI 状态。
Inspector 展开状态。
当前选中对象。
Undo 栈。
未保存的临时对象。
```

## 8. C-min 输出结构

第一版输出：

```text
runtime-package/
  manifest.json
  scenes/
    scene-*.json
  prefabs/
    prefab-*.json
  assets/
    asset-manifest.json
    runtime-asset-index.json
  input/
    input-mapping-*.json
  schema/
    component-schema.json
  rules/
    rule-manifest.json
  aui/
    aui-manifest.json
  reports/
    runtime-package-validation-report.json
    runtime-package-diff-report.json
    build-runtime-package-report.json
```

规则：

```text
manifest.json 是 Runtime Package 总入口。
scene / prefab / asset / input / schema / rules / aui 都必须从 manifest 可达。
所有对象必须有 stable id。
所有 AssetRef 必须能在 runtime-asset-index.json 中找到，或者生成明确 diagnostic。
```

## 9. manifest.json v1

第一版最小结构：

```json
{
  "schemaVersion": "runtime-package.v1",
  "packageId": "pkg_dev_desktop_001",
  "target": "dev-desktop",
  "mode": "dev-run",
  "hash": "stable-package-hash",
  "activeScene": "scenes/scene-main.json",
  "assets": "assets/runtime-asset-index.json",
  "prefabs": [
    "prefabs/prefab-player.json"
  ],
  "input": [
    "input/input-mapping-default.json"
  ],
  "schema": "schema/component-schema.json",
  "rules": "rules/rule-manifest.json",
  "aui": "aui/aui-manifest.json",
  "reports": {
    "validation": "reports/runtime-package-validation-report.json",
    "diff": "reports/runtime-package-diff-report.json",
    "build": "reports/build-runtime-package-report.json"
  }
}
```

示例里的 id 只是说明结构，不代表引擎内置项目语义。

## 10. 内容转换规则

### 10.1 Scene

```text
Saved Scene Document
  -> Runtime Scene Package
```

规则：

```text
保留 entity tree。
保留 sourceEntityId。
保留 component data。
保留 assetRef。
保留 prefab instance 和 overrides。
不写 runtimeEntityId。
不写 world matrix。
Transform 使用 localPosition / localRotation / localScale。
```

### 10.2 Prefab

```text
Saved Prefab Document
  -> Runtime Prefab Package
```

规则：

```text
保留 prefabId。
保留 root entity。
保留 child tree。
保留 component data。
保留 default overrides。
不做复杂 prefab 展开。
不做复杂 variant inheritance。
```

第一版只保证 Runtime Instantiator 可以读取并实例化。

### 10.3 Asset

```text
Project Asset DB
  -> runtime-asset-index.json
```

第一版字段：

```json
{
  "assetId": "asset_sprite_001",
  "assetKind": "texture2d",
  "sourcePath": "Assets/image.png",
  "importedPath": "Library/Imported/asset_sprite_001.json",
  "runtimeUri": "cooked-assets/asset_sprite_001",
  "hash": "stable-asset-hash",
  "dependencies": []
}
```

规则：

```text
AssetRef 保存 stable assetId。
Runtime 通过 runtime-asset-index 解析 AssetRef。
sourcePath 只用于 debug/report，不作为 Runtime 加载路径。
第一版 runtimeUri 可以指向 copied/min cooked asset。
```

### 10.4 Input Mapping

```text
InputMapping Asset
  -> input/input-mapping-*.json
```

规则：

```text
保留 action id。
保留 binding。
保留 device kind。
保留 platform override placeholder。
不把具体项目 action 变成引擎内置概念。
```

### 10.5 Schema

```text
Project Component Schema
  -> schema/component-schema.json
```

规则：

```text
引擎 typed component schema 和项目 dynamic component schema 都要进入 package。
Runtime 通过 schema 校验 dynamic component。
Schema 只描述数据结构，不描述玩法语义。
```

### 10.6 Rule Manifest

```text
Project Rule Manifest
  -> rules/rule-manifest.json
```

第一版只做 manifest，不做编译。

规则：

```text
保留 rule id。
保留 rule kind。
保留 rule backend: ir_interpreter | rust_aot。
保留 rule package uri。
不在 RuntimePackageBuilder 中执行项目规则。
不在 RuntimePackageBuilder 中做 IR -> Rust AOT。
```

### 10.7 AUI

```text
AUI Document Manifest
  -> aui/aui-manifest.json
```

规则：

```text
保留 aui document id。
保留 canvas / document uri。
保留 referenced asset ids。
不在 package 阶段计算最终 UI runtime state。
```

## 11. Validation Report

第一版必须验证：

```text
manifest schemaVersion 合法。
activeScene 存在。
scene entity id 稳定且无重复。
component schema 可解析。
component data 符合 schema。
AssetRef 可解析。
PrefabRef 可解析。
InputMapping 可解析。
Rule manifest 可解析。
AUI manifest 可解析。
Runtime 不需要的 editor-only 字段已剥离。
```

最小字段：

```json
{
  "schemaVersion": "runtime-package-validation-report.v1",
  "status": "success",
  "checkedCounts": {
    "scenes": 1,
    "entities": 10,
    "components": 25,
    "assets": 8,
    "prefabs": 3
  },
  "diagnostics": []
}
```

Diagnostic 最小字段：

```json
{
  "severity": "error",
  "code": "MissingAssetRef",
  "message": "AssetRef cannot be resolved.",
  "objectId": "entity_001",
  "path": "components.SpriteRenderer.sprite",
  "suggestion": "Import the missing asset or replace the AssetRef."
}
```

## 12. Diff Report

Runtime Package Diff Report 只用于调试和 AI 查错，不参与 Runtime 执行。

第一版比较：

```text
manifest hash
scene hash
asset index hash
prefab hash
schema hash
rule manifest hash
input mapping hash
aui manifest hash
```

最小字段：

```json
{
  "schemaVersion": "runtime-package-diff-report.v1",
  "previousPackageId": "pkg_old",
  "currentPackageId": "pkg_new",
  "changes": [
    {
      "kind": "scene",
      "id": "scene_main",
      "change": "modified",
      "summary": "entity/component/package hash changed"
    }
  ]
}
```

第一版不做字段级 sourceMap。字段级 sourceMap 会显著增加规则复杂度；第一版通过 stable id / object path / hash / diagnostic 已经能满足 AI 查错。

## 13. BuildRuntimePackageReport

第一版最小字段：

```json
{
  "schemaVersion": "build-runtime-package-report.v1",
  "requestId": "runtime-package-build-001",
  "status": "success",
  "outputs": {
    "packageDir": "dist/dev-desktop/runtime-package",
    "manifest": "dist/dev-desktop/runtime-package/manifest.json"
  },
  "stages": [
    {
      "stageId": "write-scene-package",
      "status": "success",
      "durationMs": 1,
      "diagnostics": []
    }
  ],
  "sourceReports": [
    "reports/runtime-package-validation-report.json",
    "reports/runtime-package-diff-report.json"
  ],
  "diagnostics": []
}
```

## 14. 第一版非目标

第一版不做：

```text
真实 Android / iOS package。
真实签名。
真实安装器。
真实 zip / pak / obb。
真实 ASTC / Basis 转码。
真实 mesh LOD cook。
复杂 prefab variant 展开。
字段级 sourceMap。
复杂 provenance graph。
IR -> Rust AOT 编译。
热更包生成。
Bundle binary pack。
Runtime 执行项目逻辑。
```

这些后续可以扩展，但不能污染 C-min 的边界。

## 15. 为什么适合我们

### AI 友好

```text
所有输入、输出、错误、diff 都是结构化 JSON。
AI 可以通过 stable id / path / diagnostic 定位问题。
AI 不需要理解平台包格式就能修复大部分内容错误。
```

### 复杂项目可维护

```text
Scene / Prefab / Asset / Input / Schema / Rule / AUI 分区明确。
RuntimePackageBuilder 只负责内容转换，不负责启动和运行。
BuildRunOrchestrator 只负责编排，不负责具体内容 cook 细节。
```

### 后期可修改

```text
增加新的 asset kind 时扩展 asset cook adapter。
增加新的 component schema 时扩展 schema validation。
增加新的 runtime package format 时复用同一套 manifest / validation / report。
```

### 简单度

```text
第一版只做 dev-desktop readable package。
不做字段级 sourceMap。
不做完整平台发布。
不做复杂二进制包。
```

### 效率

```text
构建期完成 normalize / strip / validate。
Runtime 只读取 package 和 cooked-assets。
Release 后续可以把相同结构写成 compact binary。
```

## 16. 最小测试场景

### 16.1 最小 Scene Package

输入：

```text
1 个 scene
1 个 root entity
Transform component
SpriteRenderer component
1 个 texture assetRef
```

期望：

```text
生成 manifest.json。
生成 scenes/scene-main.json。
生成 assets/runtime-asset-index.json。
validation success。
RuntimePackageLoader 可以读取。
```

### 16.2 Prefab + Input + Schema Package

输入：

```text
1 个 prefab
1 个 input mapping asset
1 个 project dynamic component schema
```

期望：

```text
生成 prefabs/prefab-*.json。
生成 input/input-mapping-*.json。
生成 schema/component-schema.json。
validation 能检查 PrefabRef / InputMapping / Schema。
```

### 16.3 Missing AssetRef 错误

输入：

```text
Scene 中存在无法解析的 AssetRef。
```

期望：

```text
validation failed。
diagnostic.code = MissingAssetRef。
diagnostic 包含 objectId / path / suggestion。
BuildRuntimePackageReport.status = failed。
BuildRunOrchestrator 不继续 LaunchRuntime。
```

### 16.4 Diff Report

输入：

```text
previous manifest。
current manifest。
current scene hash 改变。
```

期望：

```text
runtime-package-diff-report.json 记录 scene modified。
不要求字段级 diff。
```

## 17. 正式规则总结

```text
Build / Runtime Package Completion C-min 是内容转换层，不是 BuildRun 编排器。
BuildRunOrchestrator 调用 RuntimePackageBuilder，但不吞掉 RuntimePackageReport。
RuntimePackageBuilder 生成 Runtime Package / validation / diff / build report。
Runtime 只读取 runtime-package / cooked-assets。
Editor Preview / Play / Run 应尽量共用同一种 Runtime Package。
第一版使用 Debug Readable JSON Package。
第一版不做复杂平台发布、不做真实二进制包、不做字段级 sourceMap。
所有新能力必须保持通用引擎底座边界，不为具体项目玩法新增引擎概念。
```

## 18. 下一步

确认本文档后，可以生成施工文档：

```text
106-当前可自动化施工文档-Build-Runtime-Package-Completion-C-min.md
```

施工重点：

```text
RuntimePackageBuildRequest
RuntimePackageBuilder
RuntimePackageManifest v1
RuntimePackageValidationReport
RuntimePackageDiffReport
BuildRuntimePackageReport
最小 headless tests
BuildRunOrchestrator 接入 RuntimePackageBuilder 输出
```

