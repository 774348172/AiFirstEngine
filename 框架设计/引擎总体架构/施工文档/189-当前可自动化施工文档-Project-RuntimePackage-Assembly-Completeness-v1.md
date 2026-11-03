# 189-当前可自动化施工文档-Project RuntimePackage Assembly Completeness v1

## 1. 施工目标

根据 `189-Project-RuntimePackage-Assembly-Completeness-v1方案.md`，建立 `ProjectRuntimePackageAssembler`，让真实项目目录中的 Scene / Prefab / Asset / Rule / AUI / Input / BuildProfile 统一进入 `RuntimePackageBuildInput`。

核心边界：

```text
ProjectRuntimePackageAssembler 在 editor_core。
RuntimePackageBuilder 不扫描项目目录。
desktop_export.rs 只编排导出，不再拼每个 domain。
```

## 2. 施工阶段

### Gate A：新增 Assembler 数据结构

新增：

```text
rust/crates/editor_core/src/project_runtime_package_assembler.rs
```

定义：

```text
ProjectRuntimePackageAssembler
ProjectRuntimePackageAssemblyRequest
ProjectRuntimePackageAssemblyResult
ProjectRuntimePackageAssemblyReport
ProjectRuntimePackageAssemblyDomain
ProjectRuntimePackageAssemblyDiagnostic
BuildProfile
```

### Gate B：迁移 Scene / Asset / Input 装配

把 `desktop_export.rs` 中已有的：

```text
build_runtime_package_input
collect_input_mappings
editor_scene_to_runtime
asset ref registration
```

迁移到 Assembler。

### Gate C：接入 Prefab / Rule / AUI

Assembler 新增：

```text
Prefabs/*.prefab.json -> RuntimePackageBuildInput.prefabs
Rules/rule-manifest.json -> RuntimePackageBuildInput.rule_manifest
AUI/*.aui.json -> RuntimeAuiManifest
```

### Gate D：desktop_export 改为编排

`DesktopExportPipeline::export` 改为：

```text
read project manifest
read build profile
ProjectRuntimePackageAssembler::assemble
RuntimePackageBuilder::build
copy_runtime_assets
stage player
write reports
```

### Gate E：测试与回归

新增/更新测试：

```text
cargo test -p editor_core project_runtime_package_assembler
cargo test -p editor_core desktop_export
cargo test -p project_e2e_gate
cargo test -p engine_runtime
```

## 3. 不施工范围

本次不做：

```text
完整 BuildGraph
真实资源 cook 重写
真实 Rust AOT 编译调度
动态模块加载
完整 AUI binding/present
粒子 / 音频 / 动画
打飞机专用 API
```

## 4. 验收标准

复杂打飞机样例项目经 Assembler 后必须满足：

```text
scenes >= 1
prefabs >= 3
assets >= 5
rule_manifest.rules >= 3
input_mappings >= 1
aui_manifest.documents >= 1
```

导出后必须：

```text
RuntimePackageBuilder 成功
load_runtime_package 成功
headless player gate 成功
project_e2e_gate 成功
```

## 5. 施工文档自审

### 是否符合 189

通过。施工集中在统一 Assembler，不新增每个 domain 的临时桥。

### 是否避免过度复杂

通过。C-min 只做每个 domain 的最小真实接入，不做完整商业构建系统。

### 是否可自动化验证

通过。editor_core / project_e2e_gate / engine_runtime 都有可运行测试。

### 风险

风险：

```text
Assembler 变成新的巨型文件。
```

治理：

```text
先用清晰 domain 函数分组；如果继续膨胀，后续再拆成 project_runtime_package_assembler/ 子模块。
```

