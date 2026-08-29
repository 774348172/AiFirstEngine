# 188-Complex Shooter Real Project Assembly v1 方案

## 1. 系统定义

本系统正式命名为：

```text
Complex Shooter Real Project Assembly v1
```

它不是打飞机玩法系统，也不是新增一组 `Player / Enemy / Bullet / Score` 引擎 API。

它的职责是：

```text
把复杂打飞机样例项目升级为长期真实项目模板，
并用它持续验证编辑器创作、项目资产、RuntimePackage、Windows Player 的完整闭环。
```

最终主线：

```text
Complex Shooter Project Template
  -> Project Manifest
  -> Scene
  -> Prefab
  -> Asset
  -> Project Rule
  -> Input Mapping
  -> AUI HUD
  -> Build Profile
  -> RuntimePackage
  -> Windows Player
  -> End-to-End Report
```

一句话：

```text
复杂打飞机是项目侧长期验收对象，不是引擎侧玩法内置模块。
```

## 2. 当前基础

当前工程已经存在：

```text
samples/complex_shooter_project
rust/crates/project_e2e_gate
Authoring-to-Playable Vertical Slice Gate
DesktopExportPipeline
RuntimePackageBuilder / Loader
Windowed Player headless gate
```

当前 `project_e2e_gate` 测试可以通过，但这只能说明：

```text
现有样例能跑过当前 gate。
```

它还不能充分说明：

```text
样例项目已经符合最新长期架构规则。
```

例如，187 已经确定 Project Rule 必须进入：

```text
Canonical Rule IR
  -> Rust AOT 派生产物
  -> RuleArtifactManifest
  -> RuleArtifactRegistry
  -> RuleModuleLifecycle
  -> ProjectLogicRunner
```

但当前样例项目的规则 manifest 仍需要被升级和严格验证：

```text
artifactId = rule-artifact:{rule_id}:{ir_hash}
modules 必须按 artifact lifecycle 声明
StaticRegistry 是 v1 默认执行路径
DynamicValidationHost 不允许在 B-min 真实执行
```

因此本系统要把样例项目从“测试夹具”升级成“长期真实项目模板”。

## 3. 和其它引擎的对应关系

### Unity

Unity 对应的是：

```text
Sample Project / Template Project
Scene
Prefab
AssetDatabase
Script Assembly
Input Settings
UI Canvas
BuildPipeline.BuildPlayer
```

Unity 的重点不是只验证某个 SpriteRenderer 或 Input 模块，而是用完整项目验证：

```text
编辑器内容能保存
资源能导入
脚本能编译
场景能运行
Player 能构建
```

我们学习：

```text
真实样例项目必须作为长期回归对象。
```

不照搬：

```text
不引入 MonoBehaviour / C# Assembly 作为本项目真相层。
```

### Unreal Engine

UE 对应的是：

```text
Feature Sample / Template Project
Content Browser
Level
Blueprint / C++ Module
Cook
Package
Standalone Game
```

UE 的重点是：

```text
Cook / Package / Run 必须围绕真实项目内容，而不是孤立模块测试。
```

我们学习：

```text
样例项目要覆盖 Content、Gameplay Module、UI、Input、Package、Runtime 报告。
```

不照搬：

```text
不引入 UE 式完整 Gameplay Framework、Blueprint VM 或 C++ module 热编译。
```

### Godot

Godot 对应的是：

```text
Demo Project
Project Manager
Scene
Resource
Script
Export Preset
Export Template
```

Godot 的重点是：

```text
项目目录、场景、资源、导出模板必须组成统一用户心智。
```

我们学习：

```text
样例项目目录结构必须清晰，可被编辑器、AI、构建系统共同读取。
```

不照搬：

```text
不把 Node / GDScript / Signal 作为本项目 Runtime 真相。
```

### Bevy

Bevy 对应的是：

```text
examples
assets
ECS systems
cargo run
```

可学习：

```text
示例项目应可自动测试，且能覆盖 ECS / Assets / wgpu 主路径。
```

不照搬：

```text
Bevy 没有完整编辑器产品链路，我们不能只停在 examples 级别。
```

## 4. 方案对比

### 方案 A：继续维护当前 sample，缺什么补什么

做法：

```text
继续在 samples/complex_shooter_project 上零散补文件。
```

优点：

```text
最快。
改动小。
```

缺点：

```text
样例会继续像 fixture。
容易跟最新架构规则脱节。
无法形成稳定项目模板规范。
```

结论：

```text
不推荐。
```

### 方案 B：只加严 project_e2e_gate

做法：

```text
在 project_e2e_gate 中增加更多检查。
```

优点：

```text
能暴露问题。
复用现有 gate。
```

缺点：

```text
它仍然偏测试系统。
不能单独定义“真实样例项目应该长什么样”。
容易让测试逻辑反向决定项目结构。
```

结论：

```text
可作为 C 的一部分，但不能单独作为最终方案。
```

### 方案 C-min：真实项目模板规范 + 严格装配验收

做法：

```text
定义 Complex Shooter Real Project Assembly 规则。
升级 samples/complex_shooter_project 为长期真实项目模板。
新增/强化 Assembly Validator。
让 Authoring-to-Playable Gate 使用 strict mode 验证样例项目。
```

优点：

```text
最符合长期主义。
不把打飞机玩法写入引擎 API。
能持续发现编辑器、资源、规则、AUI、Input、Build、Player 链路断点。
样例项目可以成为后续所有大系统的真实验收对象。
```

缺点：

```text
第一版需要整理现有样例项目结构。
会暴露一些历史临时规则，需要逐步升级。
```

结论：

```text
采用 C-min。
```

## 5. 正式规则

### 5.0 前置依赖规则

本系统是“真实项目装配与验收主线”，不是孤立底层模块。因此它允许依赖已经确定的前置系统。

强依赖：

```text
186-Project Rule Asset Pipeline & Runtime Execution
187-Project Rule Artifact & Module Lifecycle
185-M12 AUI HUD Authoring / Binding / Runtime Present
137/138 Runtime Render Asset Production & Binding
179 Authoring-to-Playable Vertical Slice
```

依赖处理规则：

```text
Rule 域必须按 186/187 的最终规则验收，不再保留旧 manifest 作为通过条件。
AUI 域第一版允许先检查 document 可解析和 RuntimePackage 可携带；binding/present 深度验收随 185 落地逐步加严。
Asset 域第一版不能只数文件，至少要检查 AssetRef 可解析和 RuntimePackage 中可索引；GPU binding 深度验收随 137/138 主链路加严。
Real OS Window smoke 保持可选，不作为 CI 默认阻塞项。
```

如果某个依赖尚未完全落地，Assembly Report 必须把对应域标记为：

```text
partial / blocked_by_dependency
```

不能静默通过，也不能为了通过验收降低长期规则。

### 5.1 项目侧语义规则

样例项目目录和项目文件中允许出现：

```text
Player
Enemy
Bullet
Health
Damage
Score
Wave
Weapon
Boss
Drop
```

因为这些是项目侧语义。

引擎通用 API、Runtime 底座、Editor 通用服务中不允许新增这些专用概念。

引擎只能提供：

```text
Entity
Component
ComponentValue
FieldPath
Query
CommandBuffer
Prefab
AssetRef
RuntimePackage
InputAction
Physics2D
SpriteRenderer2D
AUI
Trace
Report
Build
WindowedPlayer
```

### 5.2 样例项目目录规则

复杂打飞机样例项目必须长期保持以下结构：

```text
samples/complex_shooter_project/
  project.aife.json
  Settings/
  Scenes/
  Prefabs/
  Assets/
  Rules/
  AUI/
  Input/
  BuildProfiles/
  Reports/
```

第一版允许 `BuildProfiles/` 和 `Reports/` 由施工阶段补齐。

### 5.3 必需 Domain 规则

样例项目必须至少包含：

```text
Project Manifest
Main Scene
Sprite/Texture assets
Player-like prefab or scene entity
Projectile-like prefab
Enemy-like prefab or scene entity
Effect-like prefab
Project Rule manifest
Project Rule IR files
Input Mapping
AUI HUD document
Windows dev build profile
```

这些名称属于样例项目，不进入引擎 API。

### 5.4 Rule 规则

样例项目规则必须符合 186/187：

```text
Gameplay Rule Asset / Contract-bound RuleSlot 是用户和 AI 面向的规则资产边界。
Canonical Rule IR 是受限 RuleSlot 的内部规范语义和构建输入。
RuntimeRuleManifest 不是业务真相层。
Rust AOT 是 IR 派生产物。
artifactId 必须为 rule-artifact:{rule_id}:{ir_hash}。
modules 必须按 RuleArtifactLifecycle 声明。
StaticRegistry 是 v1 默认执行路径。
DynamicValidationHost 只作为未来占位，不在 B-min 执行。
```

验收必须检查：

```text
rule entry 是否有 irSource
rule entry 是否有 irHash
rule entry 是否有 artifactId
artifactId 是否与 rule_id / ir_hash 匹配
module 是否声明对应 artifactId
ProjectLogicRunner 是否能通过 registry 构建
```

### 5.5 Asset 规则

样例项目资产必须走正式资产链路：

```text
Authoring Asset
  -> Asset DB / Importer
  -> Cooked Asset
  -> RuntimeAssetIndex
  -> Runtime load
  -> Render Asset Production
  -> GPU / RHI binding
```

验收不能只检查文件数量，必须逐步升级为检查：

```text
AssetRef 可解析
Cooked path 存在
RuntimeAssetIndex 可加载
Sprite2D texture binding 可报告
缺失资源能定位到 asset id / source path / stage
```

### 5.6 Scene / Prefab 规则

样例项目场景和 prefab 必须能走：

```text
EditorSceneDocument
  -> RuntimePackageBuilder
  -> RuntimeScene
  -> World Hydration
  -> RenderExtract / Physics2D / ProjectLogic
```

验收必须检查：

```text
scene 可加载
entity 数量达标
prefab 可加载
prefab 可实例化
Transform / SpriteRenderer2D / Collider2D / project dynamic component 能进入 Runtime
```

### 5.7 Input 规则

样例项目输入必须通过 InputMapping：

```text
Input Mapping Asset
  -> RuntimePackage
  -> Runtime Input Resolver
  -> ActionSnapshot
  -> Project Rule
```

不允许样例项目 gate 通过硬编码按键绕过 InputMapping。

### 5.8 AUI 规则

样例项目 HUD 必须通过 AUI：

```text
AUI Document
  -> RuntimePackage
  -> Project UI State
  -> Binding Resolve
  -> AuiLayout / DrawList
  -> RuntimeRenderer UI Pass
  -> Player Present
```

不允许用调试 overlay 假装 HUD。

### 5.9 Build / Player 规则

样例项目必须能进入：

```text
DesktopExportPipeline
  -> RuntimePackage
  -> Windows player package
  -> headless surface gate
  -> optional real window smoke
```

第一版验收以 headless surface gate 为默认稳定测试。

真实 OS window smoke 保留为可选项，因为它依赖本机 GPU / 窗口环境。

### 5.10 Report 规则

Assembly report 必须结构化输出：

```text
schema_version
project_path
status
domains
required_items
rule_artifact_lifecycle
asset_binding_summary
runtime_package_summary
player_run_summary
blocking_gaps
diagnostics
next_actions
```

AI 不应该通过猜日志理解失败原因。

## 6. 第一版 C-min 范围

第一版需要实现或补齐：

```text
ComplexShooterProjectAssemblySpec
ComplexShooterProjectAssemblyValidator
ComplexShooterProjectAssemblyReport
samples/complex_shooter_project 结构升级
Rules/rule-manifest.json 升级到 187 artifact lifecycle 规则
project_e2e_gate strict assembly mode
Authoring-to-Playable Gate 引用 assembly report
```

代码落点规则：

```text
AssemblySpec / AssemblyValidator / AssemblyReport 放在 rust/crates/project_e2e_gate。
不新增独立 crate。
AssemblyValidator 是现有 project_e2e_gate 的 strict mode 升级，不是第二套验收系统。
Authoring-to-Playable Gate 只消费 AssemblyReport，不反向定义项目结构。
```

第一版不做：

```text
完整商业级打飞机游戏
真实关卡编辑器
弹幕编辑器
完整粒子编辑器
完整音频系统
完整动画系统
项目玩法专用引擎 API
```

## 7. C-min 每域验收深度

第一版必须明确每个 domain 检查到哪一层，避免“逐步升级”变成施工时理解不一致。

| Domain | C-min 第一版必须检查 | 后续升级方向 |
|---|---|---|
| Project | `project.aife.json` 存在、可解析、包含 projectId / projectName / defaultScene | manifest 字段完整性、版本迁移、项目设置一致性 |
| Scene | default scene 存在、可由 `EditorSceneDocument` 加载、entity 数量达标 | scene -> RuntimeScene -> World Hydration 逐实体一致性 |
| Prefab | 必需 prefab 文件存在、可解析、root/entity 结构合法 | prefab 实例化、覆盖项、保存回写、批量更新 |
| Asset | 必需 asset 文件存在、AssetRef 可解析、RuntimePackage 中可索引 | cooked path、RuntimeAssetIndex 加载、GPU/RHI binding 报告 |
| Rule | manifest 符合 187：irSource / irHash / artifactId / module artifact 匹配 | IR 文件真实 hash、Rust AOT codegen、RuleArtifactRegistry、ProjectLogicRunner 全闭环 |
| Input | `input.default.json` 存在、可解析、包含 move/fire/pause 等项目 action | Runtime InputResolver 生成 ActionSnapshot 并驱动 Project Rule |
| AUI | `hud.aui.json` 存在、可解析、RuntimePackage 可携带 AUI manifest | ProjectUiState binding、AUI present、AUI action -> Project Rule |
| Build | Windows dev build profile 存在或可由默认规则生成，DesktopExportPipeline 可执行 | 完整 Build And Run UI、失败定位、输出目录产品化 |
| Player | 导出 RuntimePackage 可加载，headless surface gate 可跑多帧 | 双击 exe、真实 OS window smoke、截图验收 |
| Report | AssemblyReport 结构化输出 status / domains / diagnostics / next_actions | 汇总 Build / Runtime / Asset / Rule / Render 全链路报告 |

关键约束：

```text
C-min 允许某些域是 partial，但必须说清楚阻塞依赖和下一步。
不允许只靠文件数量通过。
不允许测试 fixture 绕过 RuntimePackage / ProjectLogicRunner / InputMapping / AUI document。
```

## 8. Gate 拆分

188 的施工必须按 Gate 拆分，避免一次性把 9 个 domain 混在一起。

### Gate A：Assembly Spec / Report 数据结构

```text
ComplexShooterProjectAssemblySpec
ComplexShooterProjectAssemblyDomain
ComplexShooterProjectAssemblyReport
ComplexShooterProjectAssemblyDiagnostic
```

验收：

```text
report 可序列化
domain 状态可表达 passed / partial / failed / skipped
diagnostics 能定位 path / code / message
```

### Gate B：样例项目目录补齐

补齐：

```text
BuildProfiles/
Reports/
```

验收：

```text
project.aife.json
Settings/
Scenes/
Prefabs/
Assets/
Rules/
AUI/
Input/
BuildProfiles/
Reports/
```

全部存在。

### Gate C：Rule manifest 升级到 187

升级：

```text
rule entry artifactId = rule-artifact:{rule_id}:{ir_hash}
modules 声明对应 artifactId
StaticRegistry 作为 v1 默认执行路径
```

验收：

```text
AssemblyValidator 能拒绝旧的 sample-project-rules module id。
AssemblyValidator 能指出 rule / module / artifactId 不匹配。
```

### Gate D：Project / Scene / Prefab / Asset / Input 可解析检查

从“文件数量统计”升级为“可解析和可定位”。

验收：

```text
Project manifest 可解析
Scene document 可加载
Prefab 文件可解析
AssetRef 可解析
InputMapping 可解析
缺失项能给出 domain + path + next_action
```

### Gate E：AUI / Build / Player 链路检查

验收：

```text
AUI document 可解析并进入 RuntimePackage
DesktopExportPipeline 可执行
RuntimePackage 可加载
headless surface gate 可跑多帧
```

AUI binding / present 深度验收随 185 继续加严。

### Gate F：strict assembly mode 接入

验收：

```text
project_e2e_gate strict assembly mode 会先跑 AssemblyValidator
Authoring-to-Playable Gate 合并 AssemblyReport
最终 report 同时包含 assembly / export / runtime package / player run 状态
```

## 9. 验收标准

最小验收：

```text
cargo test -p project_e2e_gate
cargo test -p engine_runtime
```

新增验收用例至少覆盖：

```text
样例项目结构完整
样例项目 rule manifest 符合 187
缺失必需 domain 时 report 指出具体缺口
Authoring-to-Playable Gate 能合并 assembly report
导出 RuntimePackage 后能加载并跑 player gate
```

## 10. 方案自审

### 是否符合已有规则

通过。方案保持“引擎只提供底座能力，不为特定项目增加规则”的原则。打飞机语义只存在于样例项目侧。

### 是否符合长期路线

通过。它把样例项目升级为长期真实验收对象，而不是继续依赖零散 fixture。

### 是否会增加不必要复杂度

可控。新增的是项目装配规范和 validator，不是新增玩法框架。复杂度用于收敛现有系统，而不是扩张新系统。

### 是否方便实现

通过。当前已有 `samples/complex_shooter_project`、`project_e2e_gate`、`vertical_slice`、`RuntimePackageBuilder`、`DesktopExportPipeline`，施工可在现有基础上加严和升级。

### 主要风险

风险：

```text
如果 assembly validator 只检查文件数量，会继续停留在夹具级别。
```

治理：

```text
validator 必须逐步检查真实链路：rule artifact、asset binding、runtime package、player run，而不是只做目录扫描。
```

### 外部方案审查吸收记录

已吸收 `其它AI审查目录/14-188-Complex-Shooter-Real-Project-Assembly方案审查.md` 中的有效建议：

```text
补充前置依赖规则。
补充 C-min 每域验收深度。
明确 AssemblyValidator 放在 project_e2e_gate，不新增 crate。
补充 Gate A-F 拆分。
明确不能只做文件数量检查。
```

未采纳的方向：

```text
不把 Rule 域降级为只检查 manifest 字段存在。
```

理由：

```text
187 已经落地，复杂样例项目必须跟随长期规则升级；否则 188 会继续纵容旧 fixture 路径。
```
