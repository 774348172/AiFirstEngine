# 当前可自动化施工文档：Build / Run Package Orchestrator v1

本文档是可直接施工计划。长期规则见：

```text
../../72-Build-Run-Package-Orchestrator-v1方案.md
../../07-Build-Export-Pipeline.md
../../31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
```

## 施工目标

实现第一版本地桌面最小 Build / Run 编排器：

```text
Project
  -> BuildRunRequest
  -> BuildPlan
  -> Runtime Package
  -> cooked-assets
  -> staged run folder
  -> launch command
  -> BuildRunReport
```

第一版只做：

```text
dev-desktop
dev-run
staged run folder
headless-testable launch command
structured BuildRunReport
```

## 非目标

本阶段不做：

```text
真实 Android / iOS 打包。
真实签名。
真实安装器。
真实热更包。
真实 zip / pak / obb。
真实 ASTC / Basis / mesh LOD 转码。
IR -> Rust AOT codegen。
真实 Render Thread。
真实 Native RHI 后端。
TypeScript Runtime 正式保留。
```

如果发现现有代码缺少 Rust runtime executable，第一版应生成明确 report，而不是伪造成功。

## 参考引擎对照

施工时对照：

```text
UE：
  BuildCookRun / ProjectParams / DeploymentContext / RunProjectCommand。
  重点学习 Stage -> Run 边界。

Unity：
  BuildPlayerOptions / BuildPipeline.BuildPlayer / BuildReport。
  重点学习统一 BuildReport 和平台输出。

Bevy：
  App / ScheduleRunnerPlugin / WinitPlugin / AssetPlugin。
  重点学习 headless runner 与 windowed runner 分离。
```

本项目选择：

```text
UE-like BuildCookRun C-min。
```

## 施工范围

### 1. BuildRunRequest v1

新增或扩展数据结构：

```text
schemaVersion: build-run-request.v1
projectPath
target
mode
activeSceneId
outputDir
runtimeExecutable
launch
headless
frameLimit
```

规则：

```text
target 第一版只支持 dev-desktop。
mode 第一版只支持 dev-run。
runtimeExecutable 支持 auto 或显式路径。
```

最小测试：

```text
valid BuildRunRequest passes normalize。
unsupported target returns diagnostic。
missing activeSceneId returns ProjectError。
```

### 2. BuildPlan v1

生成 BuildPlan：

```text
resolve-build-profile
preflight-validate
write-runtime-package
cook-assets-min
select-runtime-executable
stage-run-folder
launch-runtime
write-build-run-report
```

最小测试：

```text
dev-desktop request generates stable stage list。
BuildPlan records runtime.kind=rust-native。
BuildPlan reserves ruleBackend=ir_interpreter | rust_aot。
```

### 3. PreflightValidate

检查：

```text
project file exists。
project schema / active scene 可解析。
outputDir 可写。
runtime package 输入可生成。
```

最小测试：

```text
missing project file fails with ProjectError。
missing active scene fails with ProjectError。
valid fixture passes preflight。
```

### 4. WriteRuntimePackage

复用现有 Runtime Package 生成能力，不另起第二套 project-to-runtime 转换。

输出：

```text
dist/dev-desktop/runtime-package/
  manifest.json
  scenes/
  assets/
  rules/
```

规则：

```text
Runtime Package 是 Runtime 主输入。
不允许 Runtime 读取 editor memory Project Object。
```

最小测试：

```text
valid fixture writes runtime-package/manifest.json。
runtime-package contains active scene data。
manifest records schemaVersion。
```

### 5. CookAssetsMin

第一版只做最小 cooked-assets 目录和报告接线。

输出：

```text
dist/dev-desktop/cooked-assets/
dist/dev-desktop/reports/asset-cook-report.json
```

规则：

```text
可以复用现有 Asset Cook Report。
不做真实转码。
缺失资源必须写入 diagnostic。
```

最小测试：

```text
valid fixture creates cooked-assets directory。
missing asset produces AssetCookError or warning according to existing policy。
asset-cook-report path is referenced by BuildRunReport.sourceReports。
```

### 6. SelectRuntimeExecutable

选择 Rust Runtime executable。

规则：

```text
runtimeExecutable=auto 时按项目约定查找 Rust runtime binary。
找不到时生成 RuntimeExecutableError。
不能伪造 exe。
不能回退到 TypeScript Runtime。
```

最小测试：

```text
explicit runtime executable path is accepted when file exists。
missing runtime executable returns RuntimeExecutableError。
auto selection records searched paths。
```

### 7. StageRunFolder

生成：

```text
dist/dev-desktop/
  runtime/
  runtime-package/
  cooked-assets/
  reports/
  logs/
```

规则：

```text
Stage folder 是 Run 的唯一磁盘输入根目录。
Stage folder 必须可删除重建。
Stage folder 必须不引用编辑器临时内存。
```

最小测试：

```text
valid fixture creates stage folder structure。
stage folder includes runtime-package / cooked-assets / reports / logs。
re-running build-run produces stable folder layout。
```

### 8. LaunchRuntime

生成启动命令：

```text
ai_first_runtime.exe --package ./runtime-package --mode dev-run
```

headless：

```text
ai_first_runtime.exe --package ./runtime-package --mode dev-run --headless --frame-limit 3 --report ./reports/runtime-run-report.json
```

第一版允许：

```text
launch=false 时只生成 launchCommand。
headless 测试默认不打开真实窗口。
runtime executable 不存在时跳过真实启动并写 failed diagnostic。
```

最小测试：

```text
headless request generates --headless --frame-limit。
launch=false does not spawn process。
missing exe produces Launch skipped / RuntimeExecutableError。
```

### 9. BuildRunReport v1

写出：

```text
dist/dev-desktop/reports/build-run-report.json
```

字段：

```text
schemaVersion
requestId
target
mode
status
stages[]
outputs
diagnostics[]
sourceReports[]
```

每个 stage：

```text
stageId
status
inputs
outputs
durationMs
diagnostics
```

最小测试：

```text
success report contains every stage。
failed report contains failed stage and diagnostic。
report outputs include stageDir / runtimePackageDir / cookedAssetsDir / launchCommand。
```

## 建议代码落点

施工前先用 `rg` 确认现有 Build Graph / Runtime Package 代码位置。

优先复用：

```text
scripts/build-graph-core.cjs
scripts/asset-cook.cjs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
```

可能新增：

```text
scripts/build-run-orchestrator.cjs
scripts/build-run-orchestrator.test.cjs
fixtures/build-run/
```

如果现有 Rust runtime 已有 CLI 入口，则接入真实 executable 选择。
如果没有 CLI 入口，本阶段只实现 executable selection diagnostic 和 launch command，不伪造运行成功。

## 回归命令

实现后至少运行：

```powershell
npm.cmd run build
npm.cmd run test:buildgraphcore
npm.cmd run test:assetcook
npm.cmd run test:buildrun
cargo test
```

如果仓库暂时没有 `test:buildrun`，本阶段需要新增。

## 完成标准

```text
存在 build-run request / plan / report 数据闭环。
最小 fixture 可以生成 staged run folder。
BuildRunReport 能解释成功 / 失败。
runtime executable 缺失不会伪成功。
Runtime 输入边界明确为 staged runtime-package。
每个新增能力都有最小测试。
49 / 54 / 18 / README / 施工文档 README 已同步。
```

## 施工完成后必须更新

```text
阶段完成记录/2026-xx-xx-Build-Run-Package-Orchestrator-v1/
施工文档/当前/72-当前可自动化施工文档-Build-Run-Package-Orchestrator-v1.md -> 施工文档/已完成/
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
18-长期主义实现路径.md
README.md
施工文档/README.md
```
