# 242-Project RuntimeModule / Generic Runtime Decoupling + Second Project Gate v1 方案

> 状态：正式方案已确认、完成方案审查并完成施工；完成证据见 `阶段完成记录/2026-07-11-Project-RuntimeModule-Generic-Runtime-Decoupling-Second-Project-Gate-v1/00-总览.md`。  
> 建立日期：2026-07-11  
> 选题来源：`240-5.6审查剩余问题讨论与施工优先级.md` Priority 2 / CQ-01。  
> 审查输入：`审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md`、`01-2026-07-11-新增功能增量代码质量审查报告.md`。  
> 用户确认：采用方案 C，以共享 `ProjectRuntimeBootstrap`、Editor/Dev 静态 linked module set、导出 singleton project player wrapper 和第二项目 Gate 关闭 CQ-01。  
> 目标：通用 Runtime、Player 和 Editor GameView 不再包含复杂打飞机专用规则、producer、input 默认值或 validation 语义；任意项目必须通过构建期静态链接的 `ProjectRuntimeModule` Adapter 和 RuntimePackage module descriptor 完成一次性绑定。

## 1. 这个系统是干什么的

这个系统是“项目编译后大脑的标准插槽”。

```text
Engine Runtime
  负责 RuntimePackage、World、生命周期、输入解析、AUI、Renderer、Projection

Project RuntimeModule
  负责项目 Rust AOT rule implementation
  负责项目 ProjectUiStateSnapshotProducer implementation
  负责项目复杂 Rust Framework / Project Rust Module implementation
```

RuntimePackage 继续是发布运行数据真相，但 JSON 不能自行变成 Rust AOT 函数。构建系统必须把项目 Rust module 静态链接到 Editor/Player 载体，Runtime 启动时再验证“当前 package 要求的 module”与“当前进程实际链接的 module”完全一致。

正式心智：

```text
Project Assets / Feature Spec / Rule Assets
  -> ProjectRuntimePackageAssembler
  -> RuntimePackage v2（数据与 module descriptor）

Project Rust Module / generated Rust AOT rules
  -> Cargo build
  -> statically linked ProjectRuntimeModule Adapter

RuntimePackage v2 + linked Adapter
  -> ProjectRuntimeBootstrap
  -> BoundProjectRuntime
  -> Editor GameView / Headless Player / Windowed Player
```

它不是动态脚本系统、module hot loader、Gameplay Router 或把所有项目塞进一个通用 Player 的中央注册表。

## 2. 为什么现在必须做

5.6 审查确认当前正式 Player 和 Core 被复杂打飞机样例反向绑定：

```text
runtime_player_winit::register_linked_static_rules
  固定注册 rule.player-move / fire-bullet / linear-motion /
  lifetime-cleanup / collision-response

runtime_player_winit headless / real-window
  固定创建 ComplexShooterRuntimeUiStateProducer

editor_core::EditorRuntimePlayInstance
  固定持有 ComplexShooterRuntimeUiStateProducer
  同时使用 EngineHostLoop::new，未装配同一项目 rule runner

engine_runtime::aui
  包含复杂打飞机 entity/component/binding/score/hp/wave/enemy/icon 语义

engine_input / RuntimePackage loader / Editor GameView / Player
  缺项目 input 时回退 action.move/action.fire gameplay_default

desktop/release export
  默认复制通用 ai_engine_runtime_cli，可执行文件没有可验证的项目 module identity
```

直接影响：

```text
第二项目的 Rust AOT rule 会因 missing_registered_rule 失败。
第二项目的 AUI binding path 会被 shooter producer 判定 unsupported。
Editor GameView 与 exported Player 可能执行不同的项目逻辑装配。
错误项目 Player 可以携带 package 启动，到首帧才暴露缺规则或错误 UI。
复杂打飞机 E2E 全通过仍不能证明引擎是通用引擎。
```

CQ-01 是剩余影响面最大的 P1 架构 seam。241 已先关闭迁移期间的项目写入 containment，现在可以安全迁移 RuntimePackage、Editor Play 和 export 选择链。

## 3. 当前代码基线

### 3.1 已有可复用的通用底座

```text
RuleModuleRegistry
  已能以 RuntimeRuleManifest 构建 ProjectLogicRunner。

ProjectLogicRunner / LogicContext
  已提供 Rust AOT rule 的正式执行入口和受限 World/command 能力。

ProjectUiStateSnapshotProducer
  已是项目 UI read model 的正确 interface。

ProjectRuntimePackageAssembler
  已是项目目录进入 RuntimePackageBuildInput 的唯一正式装配入口。

RuntimePackage
  已包含 scenes/assets/rules/input/AUI/font 等运行数据。

Editor in-process GameView / runtime_player_winit / runtime_cli
  已分别具备运行 host，但项目装配仍分散。

DesktopExportPipeline / ReleasePackageBuilder
  已能 stage 和验证 Player，可继续深化为 typed project player artifact。
```

242 不重写这些 module；它把分散的“项目选择与装配”集中到一个新的深 module。

### 3.2 当前缺少的身份合同

当前 `RuntimePackageManifest.project` 只有：

```text
name
version
```

当前 package 没有：

```text
projectId
projectRuntimeModule.moduleId
projectRuntimeModule.interfaceVersion
projectRuntimeModule.aotContentDigest
```

当前 `aife-project.v1` 也没有 project runtime build spec，因此 Editor linked set、RuntimePackage descriptor 和导出 Player target 没有可共享的项目源真相。

当前 rule manifest 虽有 `artifactId` 和 `moduleKind=staticRegistry`，`RuleModuleRegistry` 实际仍只按 `rule_id -> fn` 注册，不能证明当前函数就是 package 声明的 artifact。

### 3.3 当前导出合同

当前 Desktop/Release 默认寻找并复制：

```text
target/debug/ai_engine_runtime_cli.exe
```

这个路径只证明“有一个 Runtime CLI”，不证明：

```text
它链接了当前项目 module。
它的 module interface version 与 package 一致。
它嵌入的 AOT content digest 与 package 一致。
它能创建当前项目 producer。
```

正式导出必须改成 typed `ProjectPlayerArtifact`，并在 stage 前验证其嵌入 descriptor。

## 4. 5.6 审查结论分类

### 4.1 必须修改

```text
CQ-01：固定 complex-shooter rule registry 移出 runtime_player_winit。
CQ-01：ComplexShooter Runtime/Sample UI producer 移出 engine_runtime::aui。
CQ-01：Editor GameView 和 Player 走同一 ProjectRuntimeModule 绑定链。
CQ-01：gameplay_default production fallback 移出通用 input/runtime/player/editor。
CQ-01：complex_project_validation / sample validation wrapper 移到 sample/project 或 project_e2e_gate。
CQ-01：导出 Player 必须与 RuntimePackage module descriptor 匹配。
```

### 4.2 施工约束

```text
必须新增第二个语义不同项目，不得用第二份 shooter 数据假装通用性。
第二项目必须使用不同 rule/component/binding/input/module id。
第二项目必须通过 Editor in-process GameView、Desktop/Release export 和真实 Player process。
错误 project/module 交叉组合必须在 hydration/首帧前 fail closed。
Core/Player production source 必须有 shooter/puzzle 禁词与依赖 Gate。
不新增运行时 Logic Ownership Router 或 Architecture Guard layer。
```

### 4.3 已由历史施工吸收

```text
199：ProjectUiStateSnapshotProducer 与 active binding path / dirty cache 合同。
217/218：Editor Play 以 RuntimePackage 为真相并具备 in-process GameView runner。
229：ProjectLogicRunner、StaticRegistry 与复杂打飞机 rule runtime execution。
236：save/reload/rebuild 与 RuntimePackage consistency Gate。
237：Desktop/Release package、portable entrypoint、process verification 与 PE contract。
239：bounded child process、publish lock 与完整 PE verifier。
241：project-owned Build/Preview/Report 和 export 路径 containment。
```

242 继承这些合同，不建立第二套 RuntimePackage assembler、rule runner、export pipeline、process runner 或 project write module。

### 4.4 本轮不适用

```text
INC-02 LLM worker cancellation/join。
CQ-06 diagnostics-first scene/world mutation。
CQ-07/CQ-08 hygiene/CI/toolchain 治理。
动态脚本 VM、WASM gameplay、native dylib hot load/hot replace。
线上热更新、IR interpreter runtime hotfix。
Provider Registry / Agent Planner。
```

## 5. 成熟实现与可借鉴点

### 5.1 Unreal Engine Modules

官方文档和本机源码：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-modules
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Core/Public/Modules/ModuleInterface.h
```

关键做法：

```text
每个 project/plugin 默认有 primary module。
[ModuleName].Build.cs 声明依赖并让构建系统发现 module。
.uproject/.uplugin 声明 module name/type/loading phase。
IModuleInterface::StartupModule / ShutdownModule 形成一次性生命周期。
```

可学习：项目代码独立 compilation module、构建期依赖、启动时一次装配。  
不照搬：ModuleManager 动态 load/unload、LoadingPhase 图、Live Coding、运行时任意 module 发现。

### 5.2 Bevy Plugin / App

官方源码和本机源码：

```text
https://github.com/bevyengine/bevy/blob/main/crates/bevy_app/src/plugin.rs
https://github.com/bevyengine/bevy/blob/main/crates/bevy_app/src/app.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_app/src/plugin.rs
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_app/src/app.rs
```

关键调用链：

```text
App::add_plugins
  -> add_boxed_plugin / Plugins::add_to_app
  -> Plugin::build
  -> ready / finish / cleanup
```

可学习：小 interface、一次性 build、plugin implementation 隐藏大量注册细节、多个真实 Adapter 证明 seam。  
不照搬：不把完整 engine feature plugin system 暴露给普通项目；不让 ProjectRuntimeModule 动态修改 Renderer、Window 或任意 schedule。

### 5.3 Unity Assembly Definition / PlayerLoop

官方文档和本机源码：

```text
https://docs.unity3d.com/Manual/assembly-definition-files.html
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Runtime/Export/PlayerLoop/PlayerLoop.bindings.cs
```

关键做法：项目脚本按 assembly 独立编译并显式控制依赖；通用 PlayerLoop 通过 `GetDefaultPlayerLoop` / `SetPlayerLoop` 承载已编译项目逻辑。

可学习：项目代码与引擎代码分开编译，Editor/Player 消费同类构建产物。  
不照搬：C# managed domain、Domain Reload、运行时 reflection/script discovery。

### 5.4 Cargo build scripts

官方文档：

```text
https://doc.rust-lang.org/cargo/reference/build-scripts.html
```

正式采用：

```text
生成 source/descriptor 进入 OUT_DIR 或 target 下受控派生目录。
cargo::rerun-if-changed 精确声明 project module/rule source 输入。
构建错误阻断 Cargo，不生成可 stage 的 ProjectPlayerArtifact。
不让 build.rs 修改项目源文件。
```

## 6. 候选方案与正式选择

### 6.1 方案 A：通用 Player 永久链接全项目 Catalog

```text
RuntimePackage.moduleId
  -> generic Player HashMap<moduleId, module factory>
```

优点：一个可执行文件可运行多个已链接项目，Editor/Player 调用简单。  
缺点：通用 Player 反向依赖所有项目；项目增加要修改中心表；正式包携带无关玩法；Catalog 容易演化成 Logic Ownership Router。  
结论：不采用。

### 6.2 方案 B：所有载体均为严格单项目二进制

```text
project-specific Editor Host
project-specific Player
```

优点：依赖最纯，进程只链接一个 Adapter，无任何 module selection。  
缺点：Editor 切换到另一个已知项目也必须换整个 Editor binary；开发工作流和第二项目 Gate 重复载体过多。  
结论：保留为 isolation 参考，不作为当前主方案。

### 6.3 方案 C：共享 Bootstrap + Editor 静态 set + Export singleton wrapper

```text
Shared ProjectRuntimeBootstrap / descriptor contract

Editor / Dev Host
  -> build-generated LinkedProjectRuntimeSet（可含多个已编译 Adapter）
  -> package moduleId 只在 session 启动前精确选择一次

Exported Player
  -> singleton LinkedProjectRuntimeSet
  -> 只链接目标项目 Adapter
```

优点：

```text
通用 Runtime/Player 不知道具体项目语义。
Editor 保留 in-process GameView 和多已编译项目开发能力。
正式导出包只携带一个项目 module。
Player、Editor、headless、windowed 共享完全相同的 bind/diagnostic contract。
无每帧查表、无动态 VM、无 dylib、无 Logic Ownership Router。
```

代价：项目 Rust module 或 linked set 变化后需要 Cargo rebuild；Editor 打开未链接 module 时需要 rebuild/relaunch。  
正式选择：采用方案 C。

## 7. 正式架构链

### 7.1 Project source / build

```text
project.aife.json / project runtime build spec
  -> projectId
  -> runtimeModule.moduleId
  -> project Rust crate / generated rules inputs

Rule Assets / Project Rust source / Cargo.lock / target / profile
  -> deterministic AOT content digest
  -> generated descriptor constant
  -> ProjectRuntimeModule Adapter

ProjectRuntimePackageAssembler
  -> RuntimePackage v2
  -> identical RuntimeProjectModuleRef
```

`ProjectRuntimePackageAssembler` 仍是项目目录进入 runtime data 的唯一入口。module source/digest generation 属于 Build Graph，不让 Runtime 扫描项目源码。

### 7.2 Runtime bind

```text
load RuntimePackage v2
  -> validate package schema/hash
  -> read required RuntimeProjectModuleRef
  -> exact lookup in LinkedProjectRuntimeSet
  -> compare moduleId/interfaceVersion/aotContentDigest
  -> module.install(private registration)
  -> validate ruleId + artifactId + moduleKind + executor
  -> require package-owned default input mapping
  -> create fresh ProjectUiStateSnapshotProducer
  -> build ProjectLogicRunner
  -> BoundProjectRuntime + ProjectRuntimeBindReceipt
```

任何失败都不能返回半初始化 runtime，也不能回退到复杂打飞机 module/input/producer。

### 7.3 Consumer

```text
BoundProjectRuntime
  -> Editor in-process GameView
  -> runtime_player_winit headless
  -> runtime_player_winit real-window
  -> runtime_cli exported entrypoint
```

Renderer、Window、GPU、filesystem、HydrationProjection、AUI interaction 仍归各自通用 module。ProjectRuntimeModule 不能获得这些系统的任意注册权限。

## 8. RuntimePackage v2 合同

### 8.1 Breaking schema bump

新增必填 module identity 是破坏性变更，正式升级：

```text
runtime-package.v1 -> runtime-package.v2
```

不得在 v1 下增加 optional 字段再用 shooter fallback 保持“兼容”。v1 package 必须由显式 migration/rebuild 进入 v2；Runtime loader 返回结构化 rebuild diagnostic。

### 8.2 RuntimeProjectInfo

```rust
pub struct RuntimeProjectInfo {
    pub project_id: String,
    pub name: String,
    pub version: String,
    pub runtime_module: RuntimeProjectModuleRef,
}

pub struct RuntimeProjectModuleRef {
    pub module_id: String,
    pub interface_version: String,
    pub aot_content_digest: String,
}
```

JSON 示例：

```json
{
  "schemaVersion": "runtime-package.v2",
  "project": {
    "projectId": "project-complex-shooter-sample",
    "name": "Complex Shooter Sample",
    "version": "0.1.0",
    "runtimeModule": {
      "moduleId": "sample.complex-shooter.runtime",
      "interfaceVersion": "project-runtime-module.v1",
      "aotContentDigest": "sha256:..."
    }
  }
}
```

### 8.3 AOT content digest

`aotContentDigest` 是确定性构建输入 digest，不是假装成最终 PE 文件 hash：

```text
project runtime module id/version
project Rust source canonical digest
generated Rust AOT rule source/artifact digest
ProjectRuntimeModule interface version
engine rule ABI/contract version
Cargo.lock relevant dependency identity
target triple / build profile
```

Build Graph 把同一个 digest 同时写入 RuntimePackage descriptor 和静态 Adapter descriptor。最终 executable 继续由 release manifest/PE inventory 记录独立文件 hash。

### 8.4 Project manifest v2 / build spec

项目源侧同样进行显式 schema bump：

```text
aife-project.v1 -> aife-project.v2
```

`project.aife.json` 新增必填 build-time spec：

```json
{
  "schemaVersion": "aife-project.v2",
  "projectId": "project-complex-shooter-sample",
  "runtimeModule": {
    "moduleId": "sample.complex-shooter.runtime",
    "interfaceVersion": "project-runtime-module.v1",
    "cargoManifest": "RuntimeModule/Cargo.toml",
    "cargoPackage": "complex_shooter_project_runtime",
    "playerBinary": "complex_shooter_player"
  }
}
```

规则：

```text
cargoManifest 是经 241 ProjectWriteScope/ProjectRelativePath 校验的项目相对路径。
moduleId/interfaceVersion 是 RuntimePackage、linked set、Adapter descriptor 的同源输入。
cargoPackage/playerBinary 只属于 Build Graph，不进入 RuntimePackage。
aotContentDigest 由 Build Graph 根据本 spec 和第 8.3 节输入生成，不由用户手写。
Editor linked set generator、RuntimePackage assembler、Project Player Artifact Builder 都读取同一个 typed spec。
不存在 CLI/env/project name 猜测或三份手写 switch。
```

旧项目 manifest 必须经 deterministic migration 补充显式 runtime module spec；无法确定 module 时返回 migration diagnostic，不能默认指向 Complex Shooter。

## 9. ProjectRuntimeModule 深 module

### 9.1 Project Adapter interface

```rust
pub const PROJECT_RUNTIME_MODULE_INTERFACE_V1: &str =
    "project-runtime-module.v1";

pub trait ProjectRuntimeModule: Send + Sync {
    fn descriptor(&self) -> &'static ProjectRuntimeModuleDescriptor;

    fn install(
        &self,
        registration: &mut ProjectRuntimeRegistration,
    ) -> Result<(), ProjectRuntimeModuleError>;
}
```

`ProjectRuntimeModuleDescriptor`：

```rust
pub struct ProjectRuntimeModuleDescriptor {
    pub module_id: &'static str,
    pub interface_version: &'static str,
    pub aot_content_digest: &'static str,
}
```

### 9.2 私有 registration interface

`ProjectRuntimeRegistration` 字段私有，只允许：

```text
register_rust_aot_rule(rule_id, artifact_id, function)
set_ui_state_producer_factory(factory)
```

禁止加入：

```text
register_renderer
register_window
register_filesystem
register_network
register_arbitrary_schedule
on_every_frame hook
dynamic module lookup
default input mapping
```

InputMapping 是 RuntimePackage 项目资产，不是 module 隐式配置。项目复杂逻辑通过既有 `LogicContext` / command / World 受限 interface 执行。

### 9.3 Shared Bootstrap interface

正式 caller seam：

```rust
ProjectRuntimeBootstrap::bind(
    package: Arc<RuntimePackage>,
    linked_modules: &LinkedProjectRuntimeSet,
) -> Result<BoundProjectRuntime, ProjectRuntimeStartError>
```

`BoundProjectRuntime` 至少拥有：

```text
RuntimePackage identity/reference
ProjectLogicRunner
fresh Box<dyn ProjectUiStateSnapshotProducer>
project-owned default InputMappingAsset
ProjectRuntimeBindReceipt
```

具体字段保持私有；Player/Editor 通过受限 accessor/consume interface 取得现有 frame loop 所需对象。Bootstrap 隐藏 module exact selection、descriptor 校验、registry 构建、artifact 校验、producer 创建、input 选择和 diagnostics 转换。

### 9.4 Depth / leverage / locality

删除 Bootstrap 后，以下复杂度会重新散回 Player headless、Player real-window、Editor GameView、CLI、Desktop Export、Release Export：

```text
module identity match
registry/artifact match
producer factory
input fallback policy
error mapping
bind receipt
```

因此 Bootstrap 是深 module。测试只跨 `bind` interface 验证结果，不越过 interface 读取私有 registry。

## 10. LinkedProjectRuntimeSet 不是 Logic Ownership Router

### 10.1 Editor / Dev set

Editor composition root 可以注入由构建生成的静态 set：

```text
LinkedProjectRuntimeSet
  sample.complex-shooter.runtime -> static Adapter
  sample.switch-puzzle.runtime   -> static Adapter
```

规则：

```text
只在 RuntimePackage session bind 前 exact lookup 一次。
lookup 完成后 BoundProjectRuntime 不再保留 routing decision。
不按 rule/component/entity/frame/action/binding 路由。
set 不允许 prefix/alias/fallback/auto-discovery。
duplicate moduleId 构建或启动失败。
未知 module 返回 project_runtime.module_not_linked，并要求 rebuild/relaunch Editor。
```

这只是 application composition root 的静态 Adapter 集合，不是新的 runtime logic layer。

### 10.2 Export singleton set

每个正式导出 Player 只链接：

```text
generic runtime_cli/runtime_player_winit
exactly one ProjectRuntimeModule Adapter
singleton LinkedProjectRuntimeSet
```

正式包不携带其它项目 module，也不从 RuntimePackage path 动态加载 Rust code。

## 11. Project Player Artifact

### 11.1 Typed artifact

```rust
pub struct ProjectPlayerArtifact {
    pub executable: PathBuf,
    pub descriptor: ProjectRuntimeModuleDescriptorRecord,
    pub executable_hash: String,
    pub build_report_path: PathBuf,
}
```

只有 Build Graph / Project Player Artifact Builder 能构造成功状态 artifact。Desktop/Release caller 不再把任意 `PathBuf` 当作已授权且正确的 Player template。

### 11.2 Descriptor verification

stage 前通过既有 bounded child process 运行只读描述命令：

```text
Game.exe --describe-project-runtime-module
```

输出 compact schema：

```text
project-player-module-descriptor.v1
moduleId
interfaceVersion
aotContentDigest
executableHash / build identity
```

Export Orchestrator 对比 RuntimePackage descriptor；不匹配时禁止 stage/publish。正式无参数启动仍进入 packaged player，不受描述命令影响。

## 12. Input 正式规则

### 12.1 RuntimePackage 必须显式拥有 input

```text
交互项目：项目 InputMappingAsset 声明真实 action/context/binding。
无输入项目：项目仍提供显式空 mapping，例如 input.none。
```

以下 production fallback 必须删除：

```text
RuntimePackage loader -> InputMappingAsset::gameplay_default
RuntimePackage builder -> gameplay_default
runtime_player_winit -> gameplay_default
editor_gameview_play -> gameplay_default
authoring create default -> shooter action set
```

`engine_input` 只保留通用 mapping constructor、schema、validator、resolver 和空 mapping 能力。复杂打飞机的 `action.move/fire/pointer/pause` 来自样例项目资产或项目 template，不来自 Core。

### 12.2 Missing input

RuntimePackage v2 缺 input index、缺 default mapping 或 default id 不存在时：

```text
build/load/bind fail closed
project_runtime.default_input_missing
nextAction = add an explicit project InputMappingAsset and rebuild
```

不能生成空 action snapshot 后继续伪装成功；无输入项目应通过显式空 mapping 表达意图。

## 13. Complex Shooter 项目侧迁移

迁入 project/sample-owned Rust crate：

```text
五条 sample_rule_* Rust AOT implementation。
register_linked_static_rules 等价项目 registration。
ComplexShooterRuntimeUiStateProducer。
ComplexShooterSampleUiStateProducer（若仍有 Gate 价值）。
score/hp/wave/enemy/icon binding 聚合 helper。
复杂打飞机 input default/template。
complex_project_validation wrapper 与 shooter-specific reports。
```

可复用 crate 形态：

```text
samples/complex_shooter_project/RuntimeModule/Cargo.toml
samples/complex_shooter_project/RuntimeModule/src/lib.rs
samples/complex_shooter_project/RuntimeModule/src/ui_state.rs
target/.../generated-rules/*.rs
```

最终位置可按 Cargo workspace 约束调整，但所有权必须保持 project/sample-owned；不得只把文件改名后留在 `engine_runtime` 或 `runtime_player_winit`。

`engine_runtime::m2_rule_demo` 中纯引擎 rule compiler/registry 契约测试可迁成 generic fixture；带项目语义的 demo/validation 移到 `project_e2e_gate` 或 sample module。

## 14. Editor / Player / Export 唯一选择链

### 14.1 Editor GameView

```text
Editor opens project
  -> ProjectRuntimePackageAssembler builds Preview RuntimePackage v2
  -> Editor composition root supplies LinkedProjectRuntimeSet
  -> ProjectRuntimeBootstrap::bind
  -> BoundProjectRuntime
  -> EditorRuntimePlayInstance
```

Editor 不再：

```text
直接创建 ComplexShooterRuntimeUiStateProducer。
使用 EngineHostLoop::new 跳过项目 rules。
在 package 缺 input 时创建 gameplay_default。
```

### 14.2 Headless / real-window Player

两个路径都只接受 `BoundProjectRuntime` 或同一 Bootstrap 结果；不分别实现 registry/producer/input 选择。

### 14.3 Desktop / Release

```text
Project build spec
  -> RuntimePackage v2 descriptor
  -> ProjectPlayerArtifact descriptor
  -> exact match
  -> stage/publish
  -> process verification
```

Desktop/Release report 必须记录同一 bind/build descriptor receipt，不重新解释 module identity。

## 15. 第二项目 Gate

### 15.1 项目选择

新增语义不同的 `Switch Puzzle` 最小真实项目：

```text
projectId: project-switch-puzzle-sample
moduleId: sample.switch-puzzle.runtime

rule:
  rule.toggle-switch
  rule.evaluate-puzzle

component:
  puzzle.switchState
  puzzle.sessionState

binding:
  puzzle.moves_text
  puzzle.solved

input:
  action.toggle-switch -> keyboard/Enter

AUI:
  puzzle-status.aui
```

禁止使用 `Player/Enemy/Bullet/Score/Health/Wave/Weapon` 作为第二项目语义。

### 15.2 正向矩阵

```text
Complex Shooter package + Complex Shooter Adapter -> passed。
Switch Puzzle package + Switch Puzzle Adapter -> passed。

两者均通过：
  ProjectRuntimePackageAssembler
  ProjectRuntimeBootstrap
  Editor in-process GameView
  Desktop Export
  Release Package
  exported Player process
```

Switch Puzzle Gate 必须证明：

```text
Enter input -> action.toggle-switch
-> rule.toggle-switch invoked
-> puzzle.switchState changed
-> project producer outputs puzzle.moves_text / puzzle.solved
-> AUI binding resolves
-> exported process exits successfully and report records module receipt
```

### 15.3 否定矩阵

```text
Shooter package + Puzzle Adapter -> module_id_mismatch。
Puzzle package + Shooter Adapter -> module_id_mismatch。
interfaceVersion mutation -> interface_version_mismatch。
aotContentDigest mutation -> aot_digest_mismatch。
rule artifactId mutation -> rule_artifact_mismatch。
duplicate moduleId in Editor set -> duplicate_linked_module_id。
missing module -> module_not_linked。
missing/default input mismatch -> default_input_missing。
```

全部必须在 hydration/首帧前失败，且不产生部分 World、部分 producer 或部分 Player publish。

## 16. Diagnostics、receipt 与 report

### 16.1 Diagnostic 最小集合

```text
project_runtime.package_v1_rebuild_required
project_runtime.project_manifest_v1_migration_required
project_runtime.module_ref_missing
project_runtime.module_not_linked
project_runtime.duplicate_linked_module_id
project_runtime.module_id_mismatch
project_runtime.interface_version_mismatch
project_runtime.aot_digest_mismatch
project_runtime.registration_failed
project_runtime.duplicate_rule
project_runtime.missing_linked_rule
project_runtime.rule_artifact_mismatch
project_runtime.unsupported_rule_module_kind
project_runtime.unsupported_rule_executor
project_runtime.ui_producer_missing
project_runtime.default_input_missing
project_runtime.player_artifact_mismatch
project_runtime.bind_failed
```

每条至少包含：

```text
stage
projectId
packagePath
requested module descriptor
linked descriptor（若存在）
ruleId / artifactId（若适用）
message
nextAction
```

### 16.2 Bind receipt

```text
project-runtime-bind-receipt.v1
projectId
moduleId
interfaceVersion
aotContentDigest
registeredRuleCount
requiredRuleCount
producerId
defaultInputMappingId
status
```

Player、Editor GameView、Desktop/Release verification 均引用这一语义，不各造一套 module report。

### 16.3 Report levels

```text
Runtime Off：不生成持久 JSON，只保留功能必需状态/错误。
Runtime Summary：compact descriptor/status/count。
Trace：完整 linked candidates、rule/artifact match、source mapping，仅 Gate/debug。
Editor Summary/Trace：可进入 Report Panel。
```

不新增 Runtime 常驻长 report 或每帧 module trace。

## 17. Core hygiene 与依赖 Gate

production scan 至少覆盖：

```text
rust/crates/engine_runtime/src
rust/crates/runtime_player_winit/src
rust/crates/runtime_cli/src
rust/crates/editor_core/src（排除明确 test fixture）
```

禁止残留：

```text
ComplexShooterRuntimeUiStateProducer
ComplexShooterSampleUiStateProducer
rule.player-move
rule.fire-bullet
rule.linear-motion
rule.lifetime-cleanup
rule.collision-response
project.combatState / project.sessionState 等 shooter 固定语义
production InputMappingAsset::gameplay_default fallback
```

依赖要求：

```text
engine_runtime 不依赖任何 sample project module。
runtime_player_winit 不依赖任何 sample project module。
runtime_cli generic library 不依赖任何 sample project module。
project Adapter 单向依赖 engine_runtime / engine_input。
Editor composition root 或 generated link crate 可以依赖多个 project Adapter。
singleton project player wrapper 只依赖一个 project Adapter。
```

这是 build/test Gate，不是运行时 Architecture Guard layer。

## 18. 测试矩阵

### 18.1 Descriptor / registration

```text
exact descriptor match。
missing/mismatch/duplicate module。
duplicate rule。
missing rule。
artifact mismatch。
unsupported module kind/executor。
fresh producer per bind。
```

### 18.2 RuntimePackage v2

```text
v2 round trip。
v1 rebuild-required diagnostic。
projectId/module ref required fields。
deterministic aotContentDigest source inputs。
content hash/inventory 与新增 descriptor 一致。
aife-project.v2 runtimeModule build spec round trip / deterministic migration。
项目 manifest、RuntimePackage、linked Adapter、Player artifact 的 module identity 同源一致。
```

### 18.3 Input

```text
project mapping selected。
explicit empty mapping accepted。
missing input rejected。
default id missing rejected。
Player/Editor 无 gameplay_default fallback。
```

### 18.4 Editor / Player parity

```text
同一 package/module receipt。
同一 required/registered rule count。
同一 producer id。
同一 default input mapping id。
Editor GameView 执行项目 rule，不再只 present scene。
```

### 18.5 Export

```text
typed ProjectPlayerArtifact。
--describe-project-runtime-module descriptor match。
wrong executable rejected before stage。
singleton executable 不含第二项目 module。
真实无参数 exported process 启动。
```

### 18.6 Second project

执行第 15 节完整正向/否定矩阵；只测试 module bind 不算完成，必须覆盖 Editor、export 和真实 process。

### 18.7 Full regression

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

## 19. 预期涉及文件与所有权

### 19.1 engine_runtime

```text
rust/crates/engine_runtime/src/project_runtime_module.rs（新增）
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/rule_registry.rs
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/lib.rs
rust/crates/engine_runtime/src/domain/*
```

### 19.2 generic hosts

```text
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/runtime_cli/src/lib.rs
rust/crates/runtime_cli/src/main.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/editor_preview_package.rs
```

### 19.3 build/export

```text
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/desktop_export.rs
rust/crates/editor_core/src/release_package.rs
```

### 19.4 project adapters / second project

```text
samples/complex_shooter_project/RuntimeModule/**（或等价 project-owned crate）
samples/switch_puzzle_project/**
project-specific player wrapper / generated linked set
```

### 19.5 Gate

```text
rust/crates/project_e2e_gate/src/project_runtime_module.rs
rust/crates/project_e2e_gate/src/second_project_runtime.rs
rust/crates/project_e2e_gate/src/bin/* project player fixtures
```

最终施工文档必须在 Gate A 前重新跑 mutation/dependency/source inventory，不能把本列表当作穷尽真相。

## 20. 推荐施工 Gate

### Gate A：RuntimePackage v2 / ProjectRuntimeModule Foundation

```text
定义 aife-project.v2 build spec、required module descriptor、RuntimePackage v2 loader/builder、ProjectRuntimeModule interface。
定义 private registration、LinkedProjectRuntimeSet、Bootstrap、diagnostics/receipt。
RuleModuleRegistry 升级 ruleId + artifactId match。
```

阻断：descriptor/registration/package v2 单元测试全部通过。

### Gate B：Complex Shooter Adapter Extraction

```text
迁移五条 rule、producer、binding helper、input template、validation wrapper。
Core/Player 删除 shooter implementation 和公开 export。
Complex Shooter 继续通过 module bind 与既有 gameplay/UI Gate。
```

阻断：Complex Shooter 功能不回退；Core hygiene/依赖 Gate 通过。

### Gate C：Shared Editor / Player Bootstrap

```text
迁移 Player headless/real-window。
迁移 Editor in-process GameView。
删除 gameplay_default production fallback。
统一 bind receipt 与 diagnostic mapping。
```

阻断：Editor/Player parity Gate 通过。

### Gate D：Project Player Artifact / Export

```text
Build Graph 生成 singleton wrapper/artifact。
descriptor query 和 typed artifact 验证。
Desktop/Release 只 stage matching project player。
```

阻断：wrong executable negative Gate 与 Complex Shooter exported process 通过。

### Gate E：Second Project Full Gate

```text
新增 Switch Puzzle project/module/assets。
Editor static set 同时链接两个 Adapter。
构建 Puzzle singleton player。
执行正向/交叉/篡改矩阵与真实 process。
```

阻断：第二项目完整 rule/component/input/UI/export/player evidence 通过。

### Gate F：Inventory / Full Regression / Docs

```text
复扫项目语义、依赖、gameplay fallback、旧 generic CLI export path。
运行 fmt/default/all-features workspace。
生成完成记录并同步入口/240/施工归档。
```

## 21. 本轮明确不做

```text
不实现运行时动态 Rust module discovery/load/unload。
不实现 native dylib hot reload/hot replace。
不实现 WASM gameplay 或动态脚本 VM。
不把 ProjectRuntimeModule 扩成完整 Bevy Plugin/UE ModuleManager。
不允许项目 module 注册 Renderer/Window/File/Network/Platform 能力。
不新增 Logic Ownership Router 或运行时 Architecture Guard。
不做通用线上热更新。
不处理 INC-02、CQ-06、CQ-07、CQ-08。
不借 CQ-01 重写 ECS、AUI Runtime、RuntimePackage assembler 或 export pipeline。
```

## 22. 风险与控制

### 风险 1：Editor static set 被做成中央 gameplay router

控制：只允许启动前 exact module ID lookup；无 alias/fallback/per-frame routing；Bound runtime 不保留路由表。

### 风险 2：ProjectRuntimeModule interface 膨胀

控制：v1 只允许 descriptor、rule registration、UI producer factory；新增能力必须先证明至少两个真实 Adapter 都需要。

### 风险 3：每个 Adapter 重写 bind/validation

控制：Adapter 只 `install`；descriptor match、artifact validation、input、receipt 全归共享 Bootstrap。

### 风险 4：RuntimePackage v1 optional 兼容掩盖错误

控制：正式升 v2；v1 返回 rebuild-required，不 fallback shooter。

### 风险 5：AOT digest 与 executable hash 混淆

控制：分别记录 deterministic AOT input digest 与最终 file hash；两者承担不同合同。

### 风险 6：Editor 无法打开新 project Rust module

控制：明确 `module_not_linked -> rebuild/relaunch`；本轮不伪装热加载。

### 风险 7：只移动 producer，Player rules 仍固定

控制：Core source/依赖 Gate + 第二项目 cross-matrix；任一固定 rule 残留均阻断。

### 风险 8：第二项目只是换名字

控制：要求不同 rule/component/binding/input/AUI、真实状态变化和真实导出 process。

### 风险 9：Export 继续接受任意 executable PathBuf

控制：typed ProjectPlayerArtifact + descriptor query；错误 artifact 禁止 stage。

### 风险 10：测试分层叠加旧 shallow tests

控制：以 Bootstrap interface 和两个 Adapter 的行为测试替代固定 Player internals 测试；只保留有独立合同价值的底层测试。

## 23. 方案自审

### 23.1 是否符合用户确认

通过。正式采用方案 C：共享 Bootstrap、Editor 静态 linked set、导出 singleton wrapper、RuntimePackage v2 required descriptor 和 second-project Gate。

### 23.2 是否形成深 module

通过。外部 caller 只需 `ProjectRuntimeBootstrap::bind`；module author 只实现 `descriptor/install`。registry、artifact、producer、input 和 diagnostic 复杂度集中在 implementation，具备 depth、leverage 和 locality。

### 23.3 seam 是否真实

通过。Complex Shooter 与 Switch Puzzle 是两个语义不同的真实 Adapter；不是为单一 sample 建 hypothetical seam。

### 23.4 是否新增 Logic Ownership Router

没有。Editor set 只在 session bind 前做一次 exact lookup；export set 为 singleton；没有 per-frame/per-rule/per-entity routing，也没有运行时 module ownership 决策。

### 23.5 是否保持 RuntimePackage 真相

通过。RuntimePackage v2 保存项目数据和 required module identity；Rust code 静态链接，不从项目源目录或 package path 动态加载。

### 23.6 是否完整关闭 CQ-01

通过。方案覆盖固定 rules、UI producer、gameplay input fallback、validation wrapper、Editor/Player parity、export artifact 和第二项目真实 Gate。

### 23.7 是否保持 195/196 项目逻辑边界

通过。复杂 Rust 逻辑留在 Project Rust Module；Rule Asset/IR 仍是 Contract-bound 受限数据；没有扩大 IR、VM、Renderer/File/Network 权限。

### 23.8 是否可以生成施工文档

方案审查时结论为可以；随后已生成唯一 242 施工文档、完成自审并按 Gate A-F 施工归档。最终证据见 242 阶段完成记录。

## 24. 2026-07-11 正式方案审查结论

已重新对照：

```text
两份 5.6 审查 CQ-01 证据与验收条件。
240 Priority 2 讨论范围与禁止项。
195/196 Rust Project Framework + Project Assets / IR 红线。
199 ProjectUiStateSnapshotProducer 所有权。
217/218 Editor Play RuntimePackage / in-process GameView。
229 StaticRegistry / ProjectLogicRunner。
237/239/241 export、process、publish、write containment 合同。
当前 runtime_player_winit / engine_runtime::aui / editor_gameview_play /
runtime_package / desktop_export 源码。
Unreal IModuleInterface、Bevy Plugin/App、Unity Assembly/PlayerLoop、Cargo build script。
```

审查回填：

```text
将 module interface 从“每个 Adapter 自己 bind package”收窄为 descriptor + install，避免复制 validation。
将 aife-project.v2 runtime module build spec 固定为 Editor/package/player 三条链的唯一项目源映射。
将 runtime package 变更明确为 v2 required schema，不做 optional fallback。
将 input 从 module interface 排除，固定为项目 RuntimePackage 资产。
区分 AOT content digest 与最终 executable hash。
将 Editor static set 定义为 construction-time exact lookup，禁止演化为 runtime router。
将 export executable 收敛为 typed ProjectPlayerArtifact 并增加 descriptor query。
将第二项目提升为 Editor + Desktop/Release + real process 全链 Gate。
```

这些回填使 interface 更深、选择链唯一、错误更早暴露，且没有扩大为动态模块系统。方案审查结论当时为：`通过，可以在用户下一步授权后生成唯一 242 当前施工文档。` 后续授权、施工和归档均已完成。

## 25. 正式结论

正式采用：

```text
方案 C：Project RuntimeModule / Generic Runtime Decoupling + Second Project Gate v1

Shared ProjectRuntimeBootstrap
  + RuntimePackage v2 required ProjectRuntimeModule descriptor
  + ProjectRuntimeModule descriptor/install interface
  + private rule/artifact/UI registration
  + project-owned explicit InputMapping
  + Editor/Dev build-generated static linked set
  + exported singleton project player wrapper
  + typed ProjectPlayerArtifact verification
  + Complex Shooter project-side extraction
  + Switch Puzzle second-project full Gate
```

CQ-01 的完成判定不是“trait 已存在”，而是：

```text
Core/Player 不再含项目专用语义。
Editor 和 exported Player 使用同一 bind receipt。
两个语义不同项目都能通过真实 RuntimePackage/Editor/export/player 链。
错误 module/artifact/input 组合在首帧前结构化失败。
default 与 all-features workspace 均通过。
```

## 26. 后续优先级

242 讨论完成后，按 `240` 下一项是 Priority 3：

```text
INC-02 LLM Worker Cancellation / Join Lifecycle v1
```

CQ-01 已完成施工和归档。当前没有施工项；若继续队列，应先讨论 INC-02，不得把待讨论项直接当成可施工项。

## 27. 参考

```text
框架设计/引擎总体架构/240-5.6审查剩余问题讨论与施工优先级.md
审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md
审查目录/5.6审查目录/01-2026-07-11-新增功能增量代码质量审查报告.md
框架设计/引擎总体架构/195-Gameplay-Rule-Asset-Rust-Framework-IR-Redline-and-AUI-Logic-Boundary方案.md
框架设计/引擎总体架构/196-IR-Rust-vs-Unity-Lua-CSharp-vs-UE-Blueprint-Cpp方案审查.md
框架设计/引擎总体架构/199-AUI-ProjectUiStateSnapshot-Producer-v1方案.md
框架设计/引擎总体架构/217-Editor-Play-RuntimePackage-Preview-Productization-v1方案.md
框架设计/引擎总体架构/218-Editor-In-process-GameView-Play-Runner-Productization-v1方案.md
框架设计/引擎总体架构/229-Complex-Shooter-Gameplay-Rule-Runtime-Execution-v1方案.md
框架设计/引擎总体架构/237-Release-Package-Polish-Metadata-Icon-Layout-v1方案.md
框架设计/引擎总体架构/239-Critical-Correctness-and-Safety-Convergence-Gate-v1方案.md
框架设计/引擎总体架构/241-SafeProjectPath-Project-Write-Containment-v1方案.md

rust/crates/runtime_player_winit/src/lib.rs
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/rule_registry.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/desktop_export.rs
rust/crates/editor_core/src/release_package.rs

https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-modules
https://github.com/bevyengine/bevy/blob/main/crates/bevy_app/src/plugin.rs
https://github.com/bevyengine/bevy/blob/main/crates/bevy_app/src/app.rs
https://docs.unity3d.com/Manual/assembly-definition-files.html
https://doc.rust-lang.org/cargo/reference/build-scripts.html
```
