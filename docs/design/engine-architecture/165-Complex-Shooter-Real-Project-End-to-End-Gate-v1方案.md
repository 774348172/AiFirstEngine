# 165-Complex Shooter Real Project End-to-End Gate v1 方案

## 1. 问题是什么

本文定义 `Complex Shooter Real Project End-to-End Gate v1`。

它解决 `149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md` 中当前最高优先级问题：

```text
Q1  真实用户级闭环没有被稳定验收
Q3  导出 exe 的真实运行、截图、输入、渲染、报告闭环不够常规化
Q4  复杂样例项目还没有成为长期回归资产
Q5  RuntimePackage save / reload / build / run 一致性仍需真实复杂项目验证
Q6  真实窗口、真实图片、真实 draw result 的可视化验收仍需加强
Q26 真实窗口 smoke 仍被 ignore，真实 OS window/GPU surface 不是默认验收链
```

当前工程已经有很多 C-min / v1 能力：

```text
Project Authoring Workspace
Project Rule IR / Rust AOT demo
Prefab Workflow
Asset Browser
Input Mapping
Physics2D
AUI
RuntimePackageBuilder
Windowed Player
RuntimeRenderer / RHI / WgpuBackend
Trace / Report
```

但这些能力仍主要靠模块测试、fixture、headless smoke、局部 report 证明。
现在缺的是一个真实项目级门禁，证明：

```text
真实项目文件
  -> 编辑器可读取 / 可保存
  -> 构建 RuntimePackage
  -> 导出 Windows player package
  -> 独立启动 player
  -> 加载项目数据
  -> 连续运行多帧
  -> 输入影响世界
  -> 真实资源进入渲染
  -> 生成 AI 可读报告
```

## 2. 边界规则

复杂打飞机只是样例项目，不是引擎内置玩法。

严禁把以下项目侧概念做成引擎 API：

```text
Player
Enemy
Bullet
Damage
Health
Score
Wave
Weapon
Boss
Drop
```

这些只能出现在 sample project 数据、Project Schema、Project Rule、Prefab、Scene、AUI、Input、Golden Scenario 中。

引擎只提供通用底座：

```text
Project file layout
Scene / Entity / Component
ComponentValue / FieldPath
Project Rule compile and execute
Prefab
AssetRef / Import / Cook / RuntimeAssetIndex
InputAction
Physics2D
SpriteRenderer2D
AUI
RuntimePackage
Windowed Player
Render / RHI / WgpuBackend
Trace / Report
Golden Scenario
```

## 3. 其它引擎怎么做

### Unity

Unity 的真实闭环通常是：

```text
Project Assets / Scenes / Prefabs / Scripts
  -> Editor Play Mode
  -> BuildPipeline.BuildPlayer
  -> standalone player
  -> Player log / Profiler / automated tests
```

特点：

```text
样例项目可以有具体玩法，但引擎层仍是 GameObject / Component / Scene / AssetDatabase / BuildPipeline。
Play Mode 和 Build Player 都是常规工作流，不是只靠单元测试。
```

对我们的启发：

```text
必须有真实 sample project。
必须有 editor-to-build-to-player gate。
不能只用 runtime fixture 代替真实项目文件。
```

### Unreal Engine

UE 的真实闭环通常是：

```text
Content / Blueprint / Level / Asset
  -> PIE / Standalone Game
  -> Cook
  -> Package
  -> Automation Test / Gauntlet / logs
```

特点：

```text
PIE 和 Standalone 分开验证。
Cook/Package 是独立产品链路。
复杂样例或测试地图用于持续验收。
```

对我们的启发：

```text
需要区分 headless data gate、editor/workspace gate、exported player gate。
失败时必须输出可定位 report，而不是只说测试失败。
```

### Godot

Godot 的真实闭环通常是：

```text
Project scenes/resources/scripts
  -> editor run
  -> export preset
  -> exported executable
  -> logs / project tests
```

特点：

```text
项目目录本身是真相。
导出配置和运行产物是验证重点。
```

对我们的启发：

```text
Sample project 应该是磁盘上的真实项目目录，而不是 Rust 代码里手写的 sample_package_input。
```

### Bevy

Bevy 更偏代码优先：

```text
App / Plugin / Systems
  -> examples
  -> integration tests
  -> headless or windowed examples
```

特点：

```text
examples 是长期回归资产。
Plugin/System 边界清楚。
```

对我们的启发：

```text
Complex Shooter sample 可以作为 examples/sample_project 级资产。
验证代码不应长期混在 engine_runtime。
```

## 4. 可选方案对比

### 方案 A：继续用 engine_runtime::complex_project_validation

做法：

```text
保持现有 complex_project_validation.rs。
继续在 Rust 里构造 sample world / package input。
增加更多断言。
```

优点：

```text
最快。
对现有测试影响小。
```

缺点：

```text
不是用户真实项目。
项目侧概念继续混在 engine_runtime。
无法证明编辑器保存、构建、导出、player 启动闭环。
```

结论：

```text
不推荐。它只能作为历史 C-min 验证，不应继续扩张。
```

### 方案 B：只做 exported player gate

做法：

```text
手写 RuntimePackage。
直接启动 runtime_player_winit。
验证多帧输入和渲染 report。
```

优点：

```text
比方案 A 更接近玩家体验。
能快速发现 player / runtime / render 问题。
```

缺点：

```text
跳过编辑器 authoring、保存、资源导入、RuntimePackageBuilder。
不能证明用户能从编辑器做出项目。
```

结论：

```text
不够。可以作为本方案的一个子 gate，但不能单独作为总方案。
```

### 方案 C：真实项目端到端 Gate

做法：

```text
建立真实 sample project 目录。
从项目源文件开始验证：
  project manifest
  scenes
  prefabs
  assets
  rules
  input
  aui
通过编辑器/构建/运行时链路生成 RuntimePackage 和 exported package。
再用 headless + optional real-window gate 验收。
```

优点：

```text
最符合 Unity / UE / Godot 的真实产品闭环。
能暴露系统之间的真实断点。
AI 可通过报告理解哪个环节断了。
不会把样例玩法做进引擎 API。
```

缺点：

```text
第一版施工范围较大。
需要先定义 sample project 文件布局和 gate report。
真实窗口 gate 在 CI/本地环境上需要可跳过但可记录。
```

结论：

```text
推荐。
```

## 5. 推荐方案

采用方案 C：真实项目端到端 Gate。

第一版命名：

```text
Complex Shooter Real Project End-to-End Gate v1
```

但实现中的工程名建议使用中性名称：

```text
samples/complex_shooter_project
crates/project_e2e_gate
ComplexProjectE2eGate
```

样例项目可以包含项目侧玩法数据，但引擎 crate 不新增任何 shooter-specific API。

## 6. 目标链路

第一版目标链路：

```text
SampleProjectSource
  -> ProjectAuthoringLoad
  -> SaveReloadCheck
  -> RuntimePackageBuilder
  -> RuntimePackageValidation
  -> DesktopExportPipeline
  -> ExportedPlayerPackage
  -> HeadlessPlayerRun
  -> OptionalRealWindowSmoke
  -> E2eReport
```

### 6.1 SampleProjectSource

真实磁盘目录：

```text
samples/complex_shooter_project/
  project.afengine.json
  Scenes/Main.scene.json
  Prefabs/
  Assets/
  Rules/
  Input/
  AUI/
```

规则：

```text
项目侧文件可以出现 shooter 语义。
engine_runtime / editor_core 的 API 仍只接收通用 Project / Scene / Asset / Rule / Input / AUI。
```

### 6.2 ProjectAuthoringLoad

验证：

```text
编辑器工程层能打开 sample project。
能读取 scene / asset / prefab / input / aui / rule manifest 摘要。
能生成统一 workspace summary。
```

### 6.3 SaveReloadCheck

验证：

```text
加载项目。
执行一个中性编辑事务，例如修改 entity transform 或 prefab instance field。
保存。
重新加载。
结构化摘要一致。
```

### 6.4 RuntimePackageBuilder

验证：

```text
从真实 saved project 生成 RuntimePackage。
不允许直接调用 hand-written sample_package_input 作为最终 gate。
允许临时 fallback，但 report 必须标记为 gap。
```

### 6.5 DesktopExportPipeline

验证：

```text
生成 Windows player package 目录。
目录包含 player executable / runtime_package / reports / manifest。
```

### 6.6 HeadlessPlayerRun

验证：

```text
独立加载 exported runtime_package。
连续运行 N 帧。
注入输入事件。
World 或 runtime report 中能观察到输入导致的变化。
渲染 report 至少包含真实 scene draw result。
```

### 6.7 OptionalRealWindowSmoke

验证：

```text
本地启用 real-window feature 时：
  打开真实 OS window
  创建 GPU surface
  present 至少 1 帧
  输出 screenshot 或 present report

默认 CI / headless 环境：
  不失败
  但 report 必须明确 realWindow.status = skipped / unavailable / passed
```

## 7. Gate 报告结构

新增统一报告：

```text
complex-project-e2e-gate-report.v1
```

字段：

```text
schema_version
gate_id
status: passed | failed | partial | skipped
project_path
build_output_path
exported_package_path
steps[]
gaps[]
artifacts[]
metrics
diagnostics[]
```

step 最小字段：

```text
step_id
domain
status
summary
input_path
output_path
duration_ms
diagnostic_count
```

gaps 最小字段：

```text
code
domain
summary
blocking
suggested_next_system
```

metrics 最小字段：

```text
scene_count
entity_count
prefab_count
asset_count
rule_count
input_action_count
aui_document_count
runtime_package_entity_count
frames_run
draw_item_count
present_count
```

## 8. 第一版通过标准

v1 不是商业级完整游戏，但必须满足：

```text
真实 sample project 存在于磁盘。
Gate 不再只靠 engine_runtime::complex_project_validation 手写 world。
可以从真实项目源文件进入 RuntimePackageBuilder。
可以生成 exported package 目录。
可以 headless 运行 exported package 多帧。
可以输出 AI 可读 e2e report。
如果 real-window 不可运行，必须结构化记录 skipped 原因。
```

允许第一版仍存在 gap：

```text
真实窗口 smoke 可 optional。
美术资源可以最小真实 png。
项目规则可以先用已存在 Rust AOT registered rule gate，但必须标记为 Project Rule 派生产物，不作为新真相层。
部分编辑器可视化操作可以先由 headless editor session 驱动。
```

不允许：

```text
继续只跑 complex_project_validation.rs 就宣称端到端通过。
把 Player / Enemy / Bullet / Score 做成 engine_runtime API。
把真实资源渲染替换成纯颜色块并标记为 passed。
失败只返回 panic 或 assert，不输出 report。
```

## 9. 与现有问题的对应关系

```text
Q1: 用真实项目端到端 gate 验证用户级闭环。
Q3: 用 exported package + player run 验证导出 exe 链路。
Q4: 建立 sample project 作为长期回归资产。
Q5: SaveReloadCheck + RuntimePackageBuilder 验证一致性。
Q6: HeadlessPlayerRun + optional real-window smoke 验证真实 draw result。
Q26: real-window smoke 从 ignored test 变为可报告 gate。
Q21: 后续把 complex_project_validation 从 engine_runtime 迁出时，本 gate 提供承接位置。
```

## 10. 为什么适合我们

AI 友好：

```text
每个阶段都有结构化 step / gap / diagnostic。
AI 可以直接从 report 判断断点，不需要猜测。
```

复杂项目适配：

```text
以真实项目目录为输入，覆盖 Scene / Prefab / Asset / Rule / Input / AUI / Build / Runtime。
这比单元测试更接近复杂项目组合问题。
```

长期可维护：

```text
样例玩法留在 sample project。
引擎 API 仍保持通用底座。
失败变成 gate report，不靠散落的 panic / ignored test。
```

简单度：

```text
第一版只做一个样例项目和一个总 gate。
内部拆 step，但对外只有一个大系统入口。
```

效率：

```text
默认跑 headless gate。
real-window gate 可按 feature / local 环境启用，不拖慢常规回归。
```

## 11. 方案自审

```text
Specification fit:
  满足用户要求：围绕 Complex Shooter Real Project End-to-End Gate v1 给出正式方案，并连接 149 中新增问题。

Rule fit:
  遵守“引擎只提供底座能力，不为特定项目增加规则”。样例玩法只在 sample project 层。

Textual consistency:
  方案明确区分 sample project、gate、engine foundation、optional real-window，不互相替代。

Design fit:
  对齐 AI-first、复杂项目、长期可维护、简单清晰的优先级。

Implementation feasibility:
  当前已有 Project Authoring、RuntimePackageBuilder、DesktopExportPipeline、runtime_player_winit、报告系统，可分阶段接入。

Practical reasonableness:
  不要求第一版一次做完整商业游戏；要求真实项目链路和结构化报告先闭合。
```

结论：

```text
本方案通过自审，可以进入施工文档生成阶段。
```
