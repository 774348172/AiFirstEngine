# 188-当前可自动化施工文档-Complex Shooter Real Project Assembly v1

## 1. 施工目标

根据 `188-Complex-Shooter-Real-Project-Assembly-v1方案.md`，把复杂打飞机样例项目从测试夹具升级为长期真实项目模板，并让 `project_e2e_gate` 支持 strict assembly mode。

主线：

```text
samples/complex_shooter_project
  -> AssemblyValidator
  -> AssemblyReport
  -> DesktopExportPipeline
  -> RuntimePackage
  -> headless player gate
  -> Authoring-to-Playable report
```

## 2. 施工范围

### Gate A：Assembly Spec / Report 数据结构

在 `rust/crates/project_e2e_gate` 内新增：

```text
assembly.rs
ComplexShooterProjectAssemblySpec
ComplexShooterProjectAssemblyReport
ComplexShooterProjectAssemblyDomain
ComplexShooterProjectAssemblyDiagnostic
```

不新增 crate。

### Gate B：样例项目目录补齐

补齐：

```text
samples/complex_shooter_project/BuildProfiles/windows.dev.json
samples/complex_shooter_project/Reports/.gitkeep
```

### Gate C：Rule manifest 升级到 187

升级：

```text
samples/complex_shooter_project/Rules/rule-manifest.json
```

规则：

```text
每条 enabled rule 必须有 artifactId。
artifactId = rule-artifact:{rule_id}:{ir_hash}
modules 必须声明对应 artifactId。
moduleKind 第一版为 staticRegistry。
```

### Gate D：Project / Scene / Prefab / Asset / Input 可解析检查

AssemblyValidator 必须检查：

```text
project.aife.json 可解析
default scene 可加载
prefab JSON 可解析
asset 文件存在且 SpriteRenderer2D assetRef 可解析
input.default.json 可解析并包含 action
```

### Gate E：AUI / Build / Player 链路检查

AssemblyValidator / e2e gate 必须检查：

```text
hud.aui.json 可解析
BuildProfiles/windows.dev.json 可解析
DesktopExportPipeline 可执行
RuntimePackage 可加载
headless player gate 可跑多帧
```

### Gate F：strict assembly mode 接入

改造：

```text
ComplexProjectE2eGateRequest 增加 strict_assembly: bool
run_complex_project_e2e_gate 在 strict_assembly=true 时先运行 AssemblyValidator
Authoring-to-Playable report 合并 assembly report
```

## 3. 不施工范围

本次不做：

```text
完整商业级打飞机玩法
弹幕编辑器
粒子编辑器
音频系统
动画系统
项目玩法专用引擎 API
真实 OS window 强制验收
```

## 4. 测试计划

分阶段测试：

```text
cargo test -p project_e2e_gate assembly
cargo test -p project_e2e_gate sample_project
cargo test -p project_e2e_gate e2e_gate_runs_export_package_and_headless_player
cargo test -p project_e2e_gate vertical_slice
cargo test -p project_e2e_gate
cargo test -p engine_runtime
```

## 5. 施工文档自审

### 是否符合 188

通过。施工项对应 Gate A-F，没有新增独立 crate，没有新增玩法 API。

### 是否会扩大范围

不会。本次只升级样例项目规范、strict assembly validator 和 e2e gate 接入。

### 是否能被自动化测试

通过。`project_e2e_gate` 已有测试基础，新增 validator 可用单元测试覆盖，最终用全 crate 测试验证。

### 是否保留长期路线

通过。Rule manifest 将升级到 187 artifact lifecycle 规则，不继续保留旧 fixture 格式作为通过条件。

