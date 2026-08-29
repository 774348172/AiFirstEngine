# Build / Run Package Orchestrator v1 方案

本文档定义第一版“构建并运行最小游戏包”的正式规则。

目标不是做完整发布系统，而是建立一个长期正确的最小闭环：

```text
Project
  -> Build Graph
  -> Runtime Package
  -> cooked assets
  -> Rust Runtime executable
  -> staged run folder
  -> Run
  -> BuildRunReport
```

## 问题定义

当前项目已经具备：

```text
Runtime Package 数据层
Runtime 资源加载系统
Scene / Prefab / Entity Runtime 实例化
Rust ECS / FrameLoop / RenderCommand / RenderSceneState
Runtime Viewport Rendering System D-min
Build Graph / Asset Pipeline / Bundle Report
```

但还缺一个统一编排器，把这些产物变成“可以从磁盘独立启动的最小游戏包”。

正式规则：

```text
Editor Preview / Run / Export 不能直接把编辑器内存 Project Object 交给 Runtime。
Runtime 必须只读取 staged runtime-package 和 staged cooked-assets。
Runtime 必须能脱离编辑器进程启动。
Build / Run 只负责生成和运行包，不负责在运行时重新解释编辑器对象。
```

## 成熟引擎参考

### Unreal Engine

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\AutomationUtils\ProjectParams.cs
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\AutomationUtils\Platform.cs
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Programs\AutomationTool\Scripts\RunProjectCommand.Automation.cs
```

UE 的核心路线是：

```text
BuildCookRun
  -> ProjectParams
  -> Cook
  -> Stage / DeploymentContext
  -> Package
  -> Deploy / Run
```

关键点：

```text
Stage 是非常重要的中间层。
运行时读取 staged 后的 cooked 内容，而不是编辑器内存对象。
平台差异通过 Platform Automation 接入。
Run 阶段可以复用 staged 输出。
```

### Unity

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPlayerWindow.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPipeline\BuildPipeline.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPipeline\BuildPlayerContext.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPipeline\BuildPipelineInterfaces.cs
```

Unity 的核心路线是：

```text
BuildPlayerOptions
  -> BuildPipeline.BuildPlayer
  -> platform player
  -> Data folder / managed/native player data
  -> BuildReport
```

关键点：

```text
用户看到的是 Build Settings / Build And Run。
底层会生成目标平台 Player 和数据目录。
BuildReport 贯穿构建回调和后处理。
编辑器 Play 和正式 Build 不是完全同一条底层路径，因此大型项目常需要额外处理一致性问题。
```

### Bevy

本地源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app\src\app.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_app\src\schedule_runner.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_winit\src\lib.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_asset\src\lib.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_internal\src\default_plugins.rs
```

Bevy 的核心路线是：

```text
cargo run / cargo build
  -> App
  -> DefaultPlugins / MinimalPlugins
  -> AssetPlugin
  -> WinitPlugin 或 ScheduleRunnerPlugin
```

关键点：

```text
Bevy 更像代码工程运行，不像 UE / Unity 那样内置完整 BuildCookRun UI。
资源加载通过 AssetPlugin / AssetServer 进入 runtime。
headless 和 windowed runner 可以分开。
```

## 方案对比

### 方案 A：继续沿用现有平台 Export 脚本

```text
export:win / export:web / export:android / export:ios 各自生成输出。
Run 直接复用某个平台脚本的结果。
```

优点：

```text
当前代码改动少。
能快速复用已有 Build Graph Core。
```

缺点：

```text
Run 和 Export 边界不清。
容易继续产生平台脚本各自为政。
AI 查错需要读多个平台输出。
Runtime Package / Runtime EXE / cooked assets 的最小闭环不够明确。
```

### 方案 B：只做 Runtime Package Runner

```text
直接让 Rust Runtime 读取 runtime-package。
不处理 staged folder，不处理 cooked assets，不处理 BuildRunReport。
```

优点：

```text
实现最简单。
适合 runtime crate 单测。
```

缺点：

```text
不是完整游戏包。
无法验证 Build Graph 到 Runtime 的真实交接。
资源、报告、启动参数、运行日志缺少统一入口。
```

### 方案 C-min：UE-like BuildCookRun 最小版

```text
BuildRunRequest
  -> ResolveBuildProfile
  -> GenerateBuildPlan
  -> PreflightValidate
  -> WriteRuntimePackage
  -> CookAssetsMin
  -> SelectRuntimeExecutable
  -> StageRunFolder
  -> LaunchRuntime
  -> WriteBuildRunReport
```

优点：

```text
路线接近 UE 的 BuildCookRun / Stage / Run，长期方向稳。
比完整发布系统小，不把签名、安装器、移动端打包提前引入。
Run 的输入和输出都结构化，AI 容易理解和查错。
能验证 Runtime Package、cooked assets、Rust Runtime executable 是否真的能组合运行。
```

缺点：

```text
比单纯 Runtime Package Runner 多一层 Orchestrator。
第一版需要维护 staged folder 和 report schema。
```

### 方案 D：完整发布管线一次到位

```text
桌面 / Web / Android / iOS 全平台真实打包、签名、安装、热更包、压缩 bundle、真实 cook 全部接入。
```

优点：

```text
长期能力最完整。
```

缺点：

```text
第一版过大。
会把平台工具链、签名、移动端包格式、真实资源转码和 Runtime 闭环混在一起。
不利于 AI 和用户定位最小游戏包的问题。
```

## 最终选择

采用：

```text
方案 C-min：UE-like BuildCookRun 最小版。
```

正式命名：

```text
Build / Run Package Orchestrator v1
```

它学习 UE 的 BuildCookRun / Stage / Run 思路，但第一版只做本地桌面 staged run folder 和 Rust Runtime 启动。

## 第一版职责

引擎负责：

```text
解析 BuildRunRequest。
生成 BuildPlan。
执行 PreflightValidate。
写出 Runtime Package。
生成最小 cooked-assets 目录。
选择 Rust Runtime executable。
生成 staged run folder。
生成 Runtime 启动参数。
可选启动 Runtime。
写出 BuildRunReport。
```

项目负责：

```text
Build Profile。
Platform Profile。
active scene。
项目资源和规则数据。
```

AI 负责：

```text
解释 BuildRunReport。
根据 diagnostics 生成修复建议。
必要时生成 ProjectPatchPlan。
```

Runtime 负责：

```text
读取 staged runtime-package。
读取 staged cooked-assets / RuntimeAssetIndex。
创建 ECS World。
执行 FrameLoop / ProjectLogicRunner。
输出 RuntimeRunReport / RuntimeTrace / RenderFrameReport。
```

## 第一版流程

```text
BuildRunRequest
  -> ResolveBuildProfile
  -> GenerateBuildPlan
  -> PreflightValidate
  -> WriteRuntimePackage
  -> CookAssetsMin
  -> SelectRuntimeExecutable
  -> StageRunFolder
  -> LaunchRuntime
  -> WriteBuildRunReport
```

说明：

```text
LaunchRuntime 可以在 headless 测试中只生成 launch command，不一定真实启动窗口。
如果 Rust Runtime executable 不存在，必须生成明确 diagnostic，不能静默成功。
```

## BuildRunRequest v1

第一版最小字段：

```json
{
  "schemaVersion": "build-run-request.v1",
  "projectPath": "project/project.json",
  "target": "dev-desktop",
  "mode": "dev-run",
  "activeSceneId": "scene_main",
  "outputDir": "dist/dev-desktop",
  "runtimeExecutable": "auto",
  "launch": true,
  "headless": false,
  "frameLimit": null
}
```

规则：

```text
target 第一版只要求 dev-desktop。
mode 第一版只要求 dev-run。
runtimeExecutable 可以是 auto 或显式路径。
headless 用于自动化测试。
frameLimit 用于 headless smoke test。
```

## BuildPlan v1

第一版最小字段：

```json
{
  "schemaVersion": "build-plan.v1",
  "target": "dev-desktop",
  "mode": "dev-run",
  "stages": [
    "resolve-build-profile",
    "preflight-validate",
    "write-runtime-package",
    "cook-assets-min",
    "select-runtime-executable",
    "stage-run-folder",
    "launch-runtime",
    "write-build-run-report"
  ],
  "runtime": {
    "kind": "rust-native",
    "ruleBackend": "ir_interpreter"
  }
}
```

规则：

```text
ruleBackend 第一版允许 ir_interpreter。
schema 必须预留 rust_aot。
Build / Run 只把规则放入 Runtime Package，不在 Orchestrator 内执行项目规则。
```

## staged run folder v1

第一版输出结构：

```text
dist/dev-desktop/
  runtime/
    ai_first_runtime.exe
  runtime-package/
    manifest.json
    scenes/
    assets/
    rules/
  cooked-assets/
  reports/
    build-run-report.json
    runtime-package-report.json
    asset-cook-report.json
  logs/
```

规则：

```text
runtime-package 是 Runtime 的主输入。
cooked-assets 是资源运行时输入。
reports 是 AI / 用户查错输入。
logs 是运行日志输入。
Runtime 不读取 editor project object。
Runtime 不依赖 editor process。
```

## Runtime 启动参数

第一版标准命令：

```text
ai_first_runtime.exe --package ./runtime-package --mode dev-run
```

headless 测试可追加：

```text
--headless --frame-limit 3 --report ./reports/runtime-run-report.json
```

规则：

```text
Runtime 入口只接受 package path 和运行参数。
不接受 editor memory pointer。
不接受直接 project object。
```

## BuildRunReport v1

第一版最小字段：

```json
{
  "schemaVersion": "build-run-report.v1",
  "requestId": "build-run-001",
  "target": "dev-desktop",
  "mode": "dev-run",
  "status": "success",
  "stages": [],
  "outputs": {
    "stageDir": "dist/dev-desktop",
    "runtimePackageDir": "dist/dev-desktop/runtime-package",
    "cookedAssetsDir": "dist/dev-desktop/cooked-assets",
    "runtimeExecutable": "dist/dev-desktop/runtime/ai_first_runtime.exe",
    "launchCommand": "..."
  },
  "diagnostics": [],
  "sourceReports": []
}
```

每个 stage 必须记录：

```text
stageId
status: success / warning / failed / skipped
inputs
outputs
durationMs
diagnostics
```

## 错误分类

第一版 diagnostics 至少支持：

```text
ProjectError：项目文件、active scene、schema 错误。
RuntimePackageError：runtime package 写出或校验错误。
AssetCookError：资源 cook / copy / index 错误。
RuntimeExecutableError：runtime executable 缺失或不可执行。
StageFolderError：staged folder 写入失败。
LaunchError：runtime 启动失败。
ReportError：报告写入失败。
```

错误必须能被 AI 和用户看懂：

```text
错误：没有找到 Rust Runtime executable。
影响：无法启动 dev-desktop 运行包。
建议：先执行 cargo build，或在 BuildRunRequest.runtimeExecutable 中指定路径。
```

## 与 ProjectLogicRunner 的关系

Build / Run Orchestrator 不执行项目逻辑。

正式边界：

```text
Build / Run Orchestrator
  -> 生成 Runtime Package / rules / assets / launch command

Rust Runtime
  -> 加载 Runtime Package
  -> 创建 ECS World
  -> FrameLoop 调用 ProjectLogicRunner
  -> ProjectLogicRunner 调用 LogicExecutor
  -> LogicExecutor 使用 IR Interpreter 或 Rust AOT
```

规则：

```text
如果 Rust AOT 尚未实现，Runtime Package 可以标记 ruleBackend=ir_interpreter。
schema 必须保留 ruleBackend: ir_interpreter | rust_aot。
不允许 Orchestrator 为了跑通而引入第二套 TypeScript Runtime。
```

## 第一版非目标

第一版不做：

```text
真实 Android / iOS 打包。
真实签名。
安装器。
Store package。
真实热更包。
真实 zip / pak / obb。
真实 ASTC / Basis / mesh LOD 转码。
IR -> Rust AOT codegen。
真实 Render Thread。
真实 Native RHI 后端。
```

这些能力后续仍由 Build Graph / Platform Pipeline 扩展，但不能污染第一版最小运行包闭环。

## 测试规则

每个实现需求都必须有最小测试。

第一版至少测试：

```text
缺少 active scene 时生成 ProjectError。
最小 fixture 可以生成 staged run folder。
staged run folder 包含 runtime-package / cooked-assets / reports。
BuildRunReport 记录每个 stage 状态。
runtime executable 缺失时生成 RuntimeExecutableError。
headless launch command 生成正确。
Runtime 不读取 editor project object。
```

如果真实启动 runtime：

```text
必须支持 --headless --frame-limit 3。
必须写出 runtime-run-report.json。
测试不依赖真实窗口。
```

## 为什么适合我们

```text
AI 友好：
  所有输入、输出、阶段和错误都是结构化数据。

复杂项目可维护：
  Runtime 只读 staged package，Build Graph 和 Runtime 边界清楚。

后期可修改：
  平台打包、签名、真实 cook、Rust AOT 都可以作为阶段扩展，不需要推翻第一版。

效率：
  重型转换在 Build Graph，Runtime 只做轻量加载。

简单度：
  第一版只做 dev-desktop staged run，不提前引入完整发布系统。
```

## 关联文档

```text
07-Build-Export-Pipeline.md
18-长期主义实现路径.md
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
38-Rust-Native-Runtime-MVP与TypeScript退役规则.md
68-Runtime资源加载系统方案.md
70-Scene-Prefab-Entity-Runtime实例化方案.md
71-Runtime-Viewport-Rendering-System方案.md
```
