# 189-Project RuntimePackage Assembly Completeness v1 方案

## 1. 系统定义

本系统正式命名为：

```text
Project RuntimePackage Assembly Completeness v1
```

选择方案：

```text
C-min：完整装配框架，第一版每个 domain 接最小真实路径。
```

它解决的问题是：

```text
真实项目目录
  -> Scene / Prefab / Asset / Rule / AUI / Input / BuildProfile
  -> RuntimePackageBuildInput
  -> RuntimePackageBuilder
  -> RuntimePackage
  -> Player 加载和运行
```

一句话：

```text
它是“项目目录到 RuntimePackageBuildInput 的唯一正式装配入口”。
```

它不是玩法系统，不实现：

```text
Player
Enemy
Bullet
Score
Wave
Weapon
Boss
Drop
```

这些仍然只能属于项目侧数据、Prefab、Rule、AUI Binding。

## 2. 为什么现在需要它

188 已经把复杂打飞机样例项目升级为长期真实项目模板，并引入 strict assembly gate。

但当前还有一个关键断点：

```text
样例项目里的内容能被检查存在，
但还没有全部通过一个统一入口进入 RuntimePackageBuildInput。
```

当前 `desktop_export.rs` 已经能导出项目，但主要接入：

```text
Scene
Sprite asset ref
InputMapping
```

还没有完整接入：

```text
Prefabs
Rules
RuleArtifact lifecycle
AUI documents
BuildProfile
project-level assembly report
```

如果继续把每个 domain 都补进 `desktop_export.rs`，它会变成新的巨型拼装器。

所以必须建立：

```text
ProjectRuntimePackageAssembler
```

让它成为项目目录进入 RuntimePackage 的统一入口。

## 3. 和其它引擎的对应关系

### Unity

Unity 对应：

```text
AssetDatabase
Importer
Scene serialization
Prefab serialization
Script Assembly
BuildPipeline.BuildPlayer
```

Unity 的重点是：

```text
编辑器保存的 Scene / Prefab / Script / Asset 都能进入 BuildPlayer。
```

我们学习：

```text
构建入口不应只处理 Scene，而应按项目整体内容装配。
```

不照搬：

```text
不引入 MonoBehaviour / C# Assembly 作为项目规则真相层。
```

### Unreal Engine

UE 对应：

```text
Asset Registry
Cook
Stage
Package
Config
Game Module
```

UE 的重点是：

```text
Cook / Package 会围绕完整项目内容，而不是围绕单个场景文件。
```

我们学习：

```text
项目内容装配、资源索引、规则模块、输出报告必须在构建链路中收敛。
```

不照搬：

```text
不做 UE 式完整 Cook Graph / UBT / UAT / Blueprint VM。
```

### Godot

Godot 对应：

```text
ProjectSettings
Scene
Resource
Script
Export Preset
Export Template
```

Godot 的重点是：

```text
项目目录是统一真相，导出时按资源和 export preset 组装。
```

我们学习：

```text
ProjectRuntimePackageAssembler 应该从 project.aife.json 和 BuildProfiles 进入，而不是散落扫描。
```

不照搬：

```text
不把 Node / GDScript / Signal 作为 Runtime 真相。
```

### Bevy

Bevy 对应：

```text
assets/
ECS startup
AssetServer
wgpu runtime
```

Bevy 可借鉴：

```text
资源和 ECS runtime 的数据驱动心智。
```

但 Bevy 没有完整编辑器打包链路，所以不能作为本系统主参考。

## 4. 方案对比

### 方案 A：继续在 desktop_export.rs 里补字段

做法：

```text
把 Prefab / Rule / AUI / BuildProfile 继续写进 desktop_export.rs。
```

优点：

```text
短期最快。
改动少。
```

缺点：

```text
desktop_export.rs 会变成巨型拼装器。
每个 domain 都会新增一条临时桥。
后续很难判断项目内容到底从哪里进入 RuntimePackage。
```

结论：

```text
不采用。
```

### 方案 B：新增 Assembler，但只接一两个 domain

做法：

```text
新增 ProjectRuntimePackageAssembler，但第一版只接 Prefab 或 Rule。
```

优点：

```text
比 A 结构更清楚。
第一版施工风险小。
```

缺点：

```text
仍然留下半截链路。
AUI / Input / Asset / Rule 后续可能继续各自接桥。
无法解决“完整项目内容进入 RuntimePackage”的核心问题。
```

结论：

```text
不单独采用。
```

### 方案 C-min：完整装配框架，每个 domain 接最小真实路径

做法：

```text
新增 ProjectRuntimePackageAssembler。
它从 project.aife.json / BuildProfiles 出发，
统一读取 Scene / Prefab / Asset / Rule / AUI / Input，
生成 RuntimePackageBuildInput 和 AssemblyReport。
desktop_export.rs 只负责编排导出，不再自己拼每个 domain。
```

优点：

```text
最符合长期主义。
一次立住项目到 RuntimePackage 的正式边界。
避免后续无数临时桥。
能让复杂打飞机样例项目真正进入运行包。
能为后续 Project Rule Gameplay Execution 提供真实运行输入。
```

缺点：

```text
第一版需要动 editor_core 的构建装配结构。
需要给每个 domain 定义最小真实接入深度。
```

结论：

```text
采用 C-min。
```

## 5. 正式架构规则

### 5.1 模块归属

`ProjectRuntimePackageAssembler` 放在：

```text
rust/crates/editor_core
```

理由：

```text
它读取编辑器项目目录和 authoring 文件，属于 Editor / Build 层。
```

`engine_runtime::RuntimePackageBuilder` 保持职责纯净：

```text
只接收结构化 RuntimePackageBuildInput。
不读取 project.aife.json。
不扫描项目目录。
不理解 editor authoring 目录结构。
```

`desktop_export.rs` 的长期职责：

```text
读取 BuildProfile
调用 ProjectRuntimePackageAssembler
调用 RuntimePackageBuilder
stage player / reports / package manifest
运行 player gate
```

它不再直接拼接每个 domain。

### 5.2 唯一入口规则

项目目录进入 RuntimePackage 的唯一入口是：

```text
ProjectRuntimePackageAssembler::assemble(project_root, build_profile)
```

禁止新增：

```text
Scene 专用导出桥
Prefab 专用导出桥
Rule 专用导出桥
AUI 专用导出桥
Input 专用导出桥
```

所有 domain 都必须通过 Assembler 进入：

```text
RuntimePackageBuildInput
```

### 5.3 输出规则

Assembler 输出：

```text
ProjectRuntimePackageAssemblyResult
  build_input: Option<RuntimePackageBuildInput>
  report: ProjectRuntimePackageAssemblyReport
```

Report 至少包含：

```text
schema_version
project_root
build_profile
status
domains
diagnostics
runtime_package_input_summary
next_actions
```

每个 domain 必须报告：

```text
domain_id
status
source_paths
produced_items
diagnostics
```

### 5.4 Domain 接入规则

#### Scene

输入：

```text
project.aife.json.defaultScene
```

输出：

```text
RuntimePackageBuildInput.scenes
```

第一版要求：

```text
default scene 可加载
EditorSceneDocument 可转 RuntimeScene
Transform / SpriteRenderer2D / Dynamic Component 能进入 RuntimeEntity
```

#### Prefab

输入：

```text
Prefabs/*.prefab.json
```

输出：

```text
RuntimePackageBuildInput.prefabs
```

第一版要求：

```text
prefab 可解析
rootEntityId 存在
entities 非空
能写入 RuntimePackage prefabs/
```

#### Asset

输入：

```text
Scene / Prefab / AUI 中引用的 AssetRef
Assets/ 目录中的资源文件
```

输出：

```text
RuntimePackageBuildInput.assets
```

第一版要求：

```text
Scene SpriteRenderer2D assetRef 可解析
Prefab SpriteRenderer2D assetRef 可解析
AUI imageRef 可解析
缺失 asset 必须报告 asset id / source path / domain
```

#### Rule

输入：

```text
Rules/rule-manifest.json
Rules/*.ir.json
```

输出：

```text
RuntimePackageBuildInput.rule_manifest
```

第一版要求：

```text
rule manifest 符合 187 artifact lifecycle
irSource 指向的文件存在
artifactId 与 rule_id / ir_hash 匹配
module 声明对应 artifactId
```

第一版不要求：

```text
真实 Rust AOT 动态编译
动态 DLL 加载
```

#### AUI

输入：

```text
AUI/*.aui.json
```

输出：

```text
RuntimePackageBuildInput.aui_manifest
并复制 AUI 文档进入 RuntimePackage aui/
```

第一版要求：

```text
AUI document 可解析
documentId 存在
imageRef 可解析为 AssetRef
RuntimeAuiManifestEntry 可生成
```

第一版不要求：

```text
完整 ProjectUiState binding
完整 AUI action -> Project Rule
```

这些继续归 185 推进。

#### Input

输入：

```text
Input/*.json
```

输出：

```text
RuntimePackageBuildInput.input_mappings
```

第一版要求：

```text
InputMapping 可加载
validate 无 error
默认 mapping 进入 RuntimePackage
```

#### BuildProfile

输入：

```text
BuildProfiles/windows.dev.json
```

输出：

```text
DesktopExportRequest 参数
```

第一版要求：

```text
target = windows
profile = dev
frameLimit 可读取
headlessSurfaceGate 可读取
realWindowSmoke 为 optional
```

## 6. 和 188 的关系

188 负责：

```text
复杂打飞机样例项目是否完整、是否符合长期规则。
```

189 负责：

```text
完整项目内容如何真正进入 RuntimePackageBuildInput。
```

关系：

```text
188 是项目装配验收。
189 是项目装配执行入口。
```

如果 188 只检查项目存在，而 189 没有把项目装进 RuntimePackage，那么游戏仍然不可玩。

## 7. 第一版 C-min 范围

必须做：

```text
ProjectRuntimePackageAssembler
ProjectRuntimePackageAssemblyReport
BuildProfile 读取
SceneAssembly
PrefabAssembly
AssetAssembly
RuleAssembly
AuiAssembly
InputAssembly
desktop_export.rs 改为调用 Assembler
project_e2e_gate 继续验证完整样例项目导出
```

不做：

```text
完整商业 BuildGraph
真实资源 cook 重写
真实 Rust AOT 编译调度
真实动态模块加载
完整 AUI binding/present
粒子 / 音频 / 动画
打飞机专用 API
```

## 8. 验收标准

最小验收：

```text
cargo test -p editor_core project_runtime_package_assembler
cargo test -p editor_core desktop_export
cargo test -p project_e2e_gate
cargo test -p engine_runtime
```

样例项目验收：

```text
samples/complex_shooter_project
  -> ProjectRuntimePackageAssembler
  -> RuntimePackageBuildInput
```

必须包含：

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
headless player gate 跑多帧
report 能说明每个 domain 是否进入 RuntimePackage
```

## 9. 方案自审

### 是否符合长期规则

通过。方案建立统一 Assembler，避免每个 domain 继续各自接桥。

### 是否符合引擎/项目边界

通过。项目侧语义只作为普通 component / prefab / rule / AUI 数据进入 RuntimePackage，不新增玩法 API。

### 是否会过度复杂

可控。C-min 只做完整框架和每个 domain 的最小真实路径，不做完整 BuildGraph 或商业级 cook。

### 是否方便实现

通过。当前已有 `RuntimePackageBuildInput`、`RuntimePackageBuilder`、`DesktopExportPipeline`、`InputMappingAuthoringService`、`EditorSceneDocument`，主要是把散落在 `desktop_export.rs` 的逻辑收敛为 Assembler。

### 主要风险

风险：

```text
Assembler 变成新的巨型文件。
```

治理：

```text
按 domain 拆分模块或内部函数。
desktop_export.rs 只编排。
RuntimePackageBuilder 保持不扫项目目录。
```

