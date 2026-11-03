# 262 Trusted ProjectRust Editor Composition Artifact v1 方案

## 1. 文档状态

```text
系统编号：262
方案版本：v1
建立日期：2026-07-31
缺口来源：塔防 P0-5 首个真实可玩界面方案 C / Editor RuntimeModule 装配失败
正式选择：方案 C（Trusted ProjectRust Editor Composition Artifact）
当前状态：正式方案已确认，方案自审通过，已完成施工
施工状态：Window 1-5 / Gate A-I 全部完成并归档（2026-08-01）
```

本文补齐 242 已经确定、但尚未落地的 Editor build-generated composition：Editor 根据当前
项目声明生成并启动一个只链接该项目 `RuntimeModule` 的专属 composition artifact。本文不
推翻 `ProjectRuntimeBootstrap`、`LinkedProjectRuntimeSet` 或 RuntimePackage module descriptor，
也不把塔防模块加入 production Editor 固定白名单。

## 2. 一句话目标

```text
受信任且符合 ProjectRust RuntimeModule 合同的普通用户项目
  -> 从项目声明确定性构建项目专属 Editor composition artifact
  -> 受控 handoff 到该 Editor 进程
  -> 继续使用现有进程内 GameView、Pause/Step、GPU texture、AUI 和 Runtime Inspector
```

项目 Rust/AOT 或 composition identity 变化时允许重建并受控重启 Editor；Scene、AUI、资源、
Input 和 Gameplay Rule 数据变化仍只重建 RuntimePackage，不重启 Editor。

## 3. 当前缺口与证据

### 3.1 已经成立的部分

`EditorPreviewPackageService` 已能为 `ProjectRust` 项目构建
`ProjectRuntimePlayerArtifact`，报告也能保存 `player_artifact`。261-R1 已为普通项目建立独立
production staging，并对 Engine SDK、`serde`、`serde_json` 和依赖身份做受控规范化。

塔防项目已经通过 `project.aife.json` 声明：

```json
{
  "sourceKind": "projectRust",
  "moduleId": "sample.tower-defense.runtime",
  "interfaceVersion": "project-runtime-module.v2",
  "cargoManifest": "RuntimeModule/Cargo.toml",
  "cargoPackage": "tower_defense_project_runtime",
  "playerBinary": "tower_defense_player"
}
```

### 3.2 尚未成立的部分

当前 Editor Play 链为：

```text
EditorPreviewPackageService
  -> 成功构建 RuntimePackage 和 ProjectRuntimePlayerArtifact
  -> report.player_artifact 保存 artifact

PlayService
  -> 只消费 RuntimePackage
  -> EditorGameViewPlayRunner::with_linked_modules(
       self.linked_project_runtimes
     )

EditorSession
  -> default_editor_linked_project_runtimes()
  -> explicit empty + Complex Shooter + Switch Puzzle
  -> 不包含 sample.tower-defense.runtime
```

因此 `ProjectRuntimeBootstrap::bind` 会正确失败为：

```text
project_runtime.module_not_linked
```

这不是塔防 RuntimeModule 自身错误，也不是 RuntimePackage 或 Player artifact 构建错误；缺口是
production Editor composition 没有消费项目声明和已构建 artifact。

### 3.3 当前证据位置

```text
rust/crates/editor_core/src/editor_preview_package.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_core/src/project_player_artifact.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_window_winit/src/linked_project_runtimes.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
samples/tower_defense_project/project.aife.json
```

## 4. 成熟实现研究

### 4.1 Unreal Modules

Unreal 通过项目/插件 descriptor 声明模块，再由 Unreal Build Tool 把目标所需模块组合进目标；
模块具有显式启动/关闭生命周期。可学习的是“项目声明 + 构建期 composition + 明确生命周期”，
不照搬其动态模块卸载或 Live Coding。

参考：<https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-modules>

### 4.2 Unity Assembly Definition

Unity Assembly Definition 把项目代码划分为可独立编译的托管程序集，并通过 Domain Reload 管理
编辑器内代码变化。它证明项目代码需要明确的编译 identity 和依赖图，但托管程序集加载与 Rust
native code 的 ABI 条件不同，不能直接照搬 Domain Reload。

参考：<https://docs.unity3d.com/Manual/assembly-definition-files.html>

### 4.3 Bevy Plugin

Bevy plugin 是 Rust crate，在编译期链接进应用并向 `App` 注册能力。它最接近本方案：项目模块
作为静态 Rust Adapter 进入 composition，运行前完成注册，不要求稳定跨动态库 Rust ABI。

参考：<https://bevyengine.org/learn/quick-start/getting-started/plugins/>

### 4.4 Godot GDExtension

Godot 能运行时加载原生扩展，是因为 GDExtension 专门定义了稳定 C interface、生命周期和类型
注册协议。本引擎当前 `ProjectRuntimeModule` 是 Rust trait interface，没有等价的稳定 native
extension ABI，因此不能用 Rust trait object 跨 DLL 边界伪装成 GDExtension。

参考：<https://docs.godotengine.org/en/stable/tutorials/scripting/gdextension/what_is_gdextension.html>

### 4.5 Rust ABI 结论

Rust Reference 只对显式 ABI 作定义；默认 Rust ABI 不提供跨编译器版本、crate graph 或 trait
object layout 的稳定兼容保证。本方案继续采用静态链接和进程重启，不引入 dylib hot reload。

参考：<https://doc.rust-lang.org/reference/abi.html>

## 5. 方案比较与正式选择

### 5.1 方案 A：把塔防静态加入默认 Editor

```text
优点：修改最少，可立即进入塔防 GameView。
缺点：每个新项目都要改引擎 Cargo/composition；production Editor 逐渐成为样例白名单。
结论：拒绝。它只能修塔防，不能适配普通用户项目。
```

### 5.2 方案 B：运行时加载项目 Rust dylib

```text
优点：理论上无需重启 Editor。
缺点：需要稳定 native extension ABI、版本协商、内存所有权、panic/unwind、卸载和安全隔离。
结论：拒绝。当前 ProjectRuntimeModule trait 不具备该 ABI 合同。
```

### 5.3 方案 C：受信 ProjectRust 项目专属 Editor composition artifact

```text
优点：沿用 242 静态链接模型；保持进程内 GameView 完整体验；对项目通用；失败可审计。
代价：项目 Rust/AOT 变化需要构建并重启 Editor；必须建立 trust、dependency、cache 和 handoff 合同。
正式选择：方案 C。
```

## 6. 适配范围

“适配其它用户项目”的正式含义是：

```text
符合 ProjectRust RuntimeModule 合同
+ 用户显式信任该项目执行 native code
+ 依赖通过 production staging policy
+ module descriptor / AOT / engine identity 完全一致
= 可以生成并运行项目专属 Editor composition artifact
```

它不表示：

```text
任意 Cargo 项目都能运行；
任意 crates.io/path/git/build.rs/proc-macro 都默认受支持；
下载来的项目无需确认即可在 Editor 进程执行；
Rust 代码变化可以 native hot reload；
一个 specialized Editor 可以静默切换到任意其它 ProjectRust 项目。
```

## 7. 目标与非目标

### 7.1 v1 目标

```text
1. 从 project.aife.json 和 RuntimeModule/Cargo.toml 确定 composition 输入。
2. 验证 linked_set()、module descriptor 和 RuntimePackage required descriptor 一致。
3. 构建只链接当前项目 RuntimeModule 的 Editor executable artifact。
4. 在 composition identity 改变时完成可恢复的 Editor handoff。
5. 复用现有 in-process GameView、Pause/Step、GPU texture、AUI 和 Inspector。
6. 项目数据变化只重建 RuntimePackage；项目 native identity 变化才重建/relaunch。
7. 结构化报告 trust、dependency、cache、build、launch 和 bind 结果。
```

### 7.2 v1 非目标

```text
Rust dylib / native hot reload / live unload；
未受信项目的隔离 Player 编辑体验；
任意第三方依赖支持；
通用插件市场、签名基础设施或企业级安全平台；
同时把多个普通用户项目链接进一个默认 production Editor；
修改 ProjectRuntimeModule runtime interface 或项目玩法合同；
为塔防新增任何引擎专用分支。
```

## 8. 正式架构

```text
Project root
  -> project.aife.json runtimeModule spec
  -> RuntimeModule/Cargo.toml + lock + source
  -> ProjectRuntimePlayer production staging policy（261-R1）
  -> ProjectEditorCompositionArtifact::prepare(request)
       -> trust decision
       -> normalized dependency identity
       -> composition identity/cache lookup
       -> isolated build-generated composition crate
       -> project-specific Editor executable + descriptor + report
  -> EditorProjectCompositionLauncher::handoff(...)
       -> launch candidate Editor
       -> candidate validates artifact/project/package identities
       -> readiness acknowledgement
       -> transfer recoverable Editor state
       -> old Editor exits only after ack
  -> EditorSession with singleton LinkedProjectRuntimeSet
  -> ProjectRuntimeBootstrap::bind
  -> existing Editor GameView Play
```

`LinkedProjectRuntimeSet` 仍只在 session bind 前 exact lookup 一次。它不是 per-frame router，
也不负责动态发现或加载 native code。

## 9. 深 module 与 seam

### 9.1 ProjectEditorCompositionArtifact

外部 interface：

```rust
ProjectEditorCompositionArtifact::prepare(request)
    -> ProjectEditorCompositionBuildReport
```

该深 module 隐藏：

```text
项目声明校验
trust 判定
261-R1 staging 复用
Engine SDK 与 toolchain identity
build-generated Cargo/composition source
linked_set() 生成
cache 命中与失效
受限 child process
artifact publish 和 descriptor seal
```

caller 不能分别调用十余个浅步骤后自行拼装结果；成功报告必须携带可启动 artifact，失败报告
必须给出 diagnostics 和 nextAction。

### 9.2 EditorProjectCompositionLauncher

外部 interface：

```rust
EditorProjectCompositionLauncher::handoff(artifact, project, state)
    -> EditorCompositionLaunchReceipt
```

该深 module 隐藏：

```text
旧 Editor 状态保存
candidate process 启动
handoff ticket 传递
readiness/identity acknowledgement
超时和 candidate 失败处理
旧进程退出时机
临时 ticket 清理
```

### 9.3 不新增的 seam

v1 不抽象假想的 native module loader。静态 composition 只有一个 production Adapter；测试可有
fake process/build adapter，但公共 interface 不暴露 Cargo 每一步或平台进程细节。

## 10. Schema 合同

新增 schema：

```text
project-editor-composition-build-request.v1
project-editor-composition-artifact.v1
project-editor-composition-descriptor.v1
project-editor-composition-build-report.v1
project-editor-composition-handoff-ticket.v1
project-editor-composition-launch-receipt.v1
project-runtime-trust-decision.v1
```

所有 persisted schema 必须具备 `schemaVersion`、拒绝未知关键字段，并按既有 structured
diagnostic 规范输出 `code/message/path/nextAction`。

## 11. Composition artifact identity

descriptor 至少包含：

```text
projectId
moduleId
interfaceVersion
aotContentDigest
editorBuildIdentity
engineSdkDigest
toolchainIdentity
targetTriple
profile
normalizedManifestDigest
normalizedDependencyDigest
dependencyLockDigest
executableHash
createdAt
```

`editorBuildIdentity` 必须代表 Editor source/features/composition ABI，而不能只用版本字符串。
`aotContentDigest` 继续遵守 242 的 module descriptor 合同，不由 Editor 猜测或覆盖。

成功启动前至少验证：

```text
artifact descriptor
== handoff ticket expected composition
== project manifest/runtime module build identity
== generated linked module descriptor
== RuntimePackage required project module descriptor
```

任一不一致均拒绝进入 Play。

## 12. Trust 模型

### 12.1 为什么必须显式信任

ProjectRust 是 native code。把项目模块静态链接进 Editor，等价于允许其在用户权限下、Editor
进程内执行。项目路径 containment 和 Cargo dependency policy 不能替代用户信任决定。

### 12.2 v1 trust receipt

`project-runtime-trust-decision.v1` 至少记录：

```text
projectCanonicalRootIdentity
projectId
runtimeModuleSourceDigest
normalizedManifestDigest
normalizedDependencyDigest
engineBuildIdentity
decision = trusted | denied | stale
decidedAt
decisionSource = explicit_user | repository_policy
```

规则：

```text
本地仓库内由用户创建、且已有受控 repository policy 的样例可得到 repository_policy 信任；
外部下载、解压、clone 或来源未知项目默认 untrusted；
首次 native build/Play 前必须明确展示并记录信任决定；
RuntimeModule source/manifest/dependency identity 改变后旧 receipt 变为 stale；
denied/untrusted/stale 时禁止 build/launch/Play，不 fallback；
receipt 只授权当前受控 composition，不授权项目任意外部程序或网络行为。
```

v1 只建设窄型 trust decision/receipt，不建设证书、发布者账户、远程声誉或通用安全中心。
未来可为未受信项目提供隔离 Player 进程，但不在本文范围。

## 13. Dependency policy

### 13.1 复用 261-R1

Editor composition 必须复用普通项目 production staging 的同一 manifest parser、source tree
containment、trusted Engine SDK resolve、normalized dependency identity 和 locked/offline 规则。
不得复制一份更宽松的 Editor-only Cargo validator。

### 13.2 v1 允许范围

```text
Engine SDK：engine_runtime、engine_input（按项目实际需要）
受控第三方：serde、serde_json
Cargo：--locked --offline
项目 source：只读复制到隔离 staging
```

### 13.3 v1 拒绝范围

```text
项目 .cargo/config
任意 path 或 git dependency
registry override / patch / replace / workspace inheritance
build.rs / build-dependencies
未经方案扩展的 proc-macro、crate-type、target-specific dependency
输出路径、symlink、junction、reparse point 逃逸
构建期间联网解析或下载
```

未来扩展 crates.io 支持时，必须同时定义 exact lock、registry/source identity、feature identity、
build.rs/proc-macro policy、path/git policy 和对应否定测试，不能简单删除 allowlist。

## 14. Build-generated composition

隔离 staging 中生成 Editor composition crate，不修改项目源 Cargo 文件，也不修改 production/
安装态 Editor：

```text
Composition/
  Cargo.toml
  src/
    main.rs
    linked_project_runtime.rs
  composition-descriptor.json
  build-report.json
RuntimeModule/               source byte-preserving copy
RuntimeModuleBuild/          261-R1 normalized build manifest
Target/                      bounded project build target
Published/                   sealed project-specific artifact
```

generated `linked_project_runtime.rs` 只允许根据已验证的 typed build spec 生成：

```text
singleton LinkedProjectRuntimeSet
  -> project RuntimeModule Adapter
```

禁止模板出现项目名、塔防/射击/拼图语义、手写 module ID switch 或默认样例 set。module ID 和
descriptor 必须来自项目 Adapter 的静态 descriptor，并与 expected descriptor exact match。

## 15. Cache 与失效

### 15.1 Cache key

```text
Engine SDK/source digest
Editor build identity
Rust toolchain identity
target triple + profile + relevant features
projectId
moduleId + interfaceVersion + AOT content digest
normalized manifest digest
normalized dependency digest
dependency lock digest
composition schema/tool version
```

任何一项变化都不得复用旧最终 artifact。

### 15.2 Cache 分层

```text
可信共享层：Cargo registry/source 和可证明相同的 Engine SDK dependency outputs
项目隔离层：RuntimeModule staging、target、build logs
项目专属层：最终 Editor composition artifact、descriptor、report
```

项目不能读取或覆盖其它项目的最终 artifact identity。共享缓存只共享可由 Cargo/lock/source
identity 验证的依赖输出，不共享未验证项目 build output。

### 15.3 容量与淘汰

v1 必须有：

```text
全局容量上限
单项目容量上限
LRU 或同等确定性淘汰策略
不淘汰当前运行 artifact
失败/中断 staging 的 bounded cleanup
清理报告和 retained reason
```

禁止每个打开过的项目永久保留数 GB `target`。具体默认容量由施工文档结合现有 cache owner
确定，不能在实现中散落魔法数字。

## 16. Project switch 与 handoff lifecycle

### 16.1 相同 identity

```text
requested composition identity == running composition identity
  -> 不重启 Editor
  -> 直接打开/刷新项目
  -> RuntimePackage 按 dirty domains 重建
```

### 16.2 不同 identity

```text
1. 当前 Editor 保存可恢复 workspace/layout/project state。
2. prepare 目标 composition artifact；旧 Editor 继续可用。
3. 构建成功后生成一次性 handoff ticket。
4. 启动目标 specialized Editor。
5. 新 Editor 验证 ticket、artifact、project、engine 和 module identities。
6. 新 Editor 打开项目并返回 readiness acknowledgement。
7. 旧 Editor 收到匹配 ack 后退出。
8. ack 超时或 candidate 失败时保留旧 Editor，清理 ticket/candidate 临时状态。
```

specialized Editor 打开另一个不同 `ProjectRust` composition 时必须再次 handoff，不能在旧
linked set 上尝试 Play，也不能静默使用 empty runtime。

### 16.3 Rust 与数据变化

```text
RuntimeModule source/manifest/lock/descriptor/AOT digest 变化
  -> trust receipt 重新评估
  -> composition cache identity 变化
  -> rebuild + controlled relaunch

Scene/Prefab/Asset/AUI/Input/Font/Gameplay Rule 数据变化
  -> composition identity 不变
  -> 只重建 RuntimePackage
  -> 不重启 Editor
```

## 17. Editor Play 消费合同

262 完成后，`PlayService` 不需要在每次 Play 动态加载 artifact。正确关系是：

```text
Editor process launch
  -> composition root 构造 singleton LinkedProjectRuntimeSet
  -> EditorSession 持有 linked set

Play
  -> Preview Package 确认 RuntimePackage descriptor
  -> running composition identity exact check
  -> EditorGameViewPlayRunner::with_linked_modules(current linked set)
  -> ProjectRuntimeBootstrap::bind
```

`report.player_artifact` 可以继续作为 Preview/Export identity parity 证据，但不能把独立 Player
executable 当 DLL 注入 Editor，也不能由 `PlayService` 启动外部 Player 后伪装成进程内 GameView。

## 18. Failure-closed 与 diagnostics

禁止以下 fallback：

```text
fallback 到 explicit empty runtime；
fallback 到 Complex Shooter / Switch Puzzle 静态 set；
忽略 AOT digest 使用同 moduleId 旧 artifact；
构建失败后替换 production/安装态 Editor；
candidate 未 ack 就退出旧 Editor；
信任缺失时仅警告后继续执行。
```

diagnostic code 至少区分：

```text
project_editor_composition.trust_required
project_editor_composition.trust_stale
project_editor_composition.manifest_rejected
project_editor_composition.dependency_rejected
project_editor_composition.cache_invalid
project_editor_composition.build_failed
project_editor_composition.artifact_descriptor_mismatch
project_editor_composition.engine_identity_mismatch
project_editor_composition.module_not_linked
project_editor_composition.handoff_ticket_invalid
project_editor_composition.readiness_timeout
project_editor_composition.launch_failed
```

每个失败报告必须包含失败 domain/stage、稳定 code、相关受控 path、expected/actual identity 摘要和
可执行 `nextAction`。默认 Summary 不记录源码、环境 secret 或完整用户目录；Trace 仅供显式诊断。

## 19. 正向资格矩阵

### 19.1 通用项目矩阵

至少覆盖：

```text
Complex Shooter：复杂规则、输入、UI producer/session
Switch Puzzle：不同玩法与 AUI action/session
Tower Defense：第三个真实玩法，serde/serde_json，完整 P0-5 GameView
Engine workspace 外部 fixture：不同根目录、无 samples 路径假设
```

四者必须经过相同 public project manifest、staging、composition、launch 和 bind seam。禁止 fixture
通过 test-only module injection 绕过 production composition。

### 19.2 真实消费矩阵

```text
打开项目 -> composition prepare/cache -> handoff/readiness
Editor GameView Play -> Pause -> Step -> Resume -> Stop
AUI action -> ProjectRuntimeSession -> World projection -> next-frame UI snapshot
Runtime Inspector 读取 active GameView runtime
GPU texture present 和字体/AUI 不回归
Scene/AUI/Rule 数据变化不重启 Editor
RuntimeModule 变化触发新 identity、重建和 handoff
```

## 20. 否定矩阵

必须至少覆盖：

```text
未信任外部项目
stale trust receipt
错误 schema/interfaceVersion
moduleId 相同但 AOT digest 不同
artifact executable hash 不一致
engine SDK/editor build/toolchain identity 不一致
duplicate linked module ID
generated set 未链接 expected module
不支持的 crates.io dependency
path/git/build.rs/.cargo/config
manifest/lock/source link 或路径逃逸
cache descriptor 被篡改
构建进程失败、超时、输出超限
candidate 启动失败/readiness 超时/错误 ack
切换项目时旧 Editor 状态保留
失败后 production/安装态 binary 未变化
```

所有否定项都必须证明没有 fallback、没有启动项目 native code、没有提前退出旧 Editor。

## 21. 测试与 Gate 方向

未来施工文档至少拆分：

```text
Gate A：schema、composition identity、trust decision owner tests
Gate B：复用 261-R1 staging 与依赖否定矩阵
Gate C：build-generated singleton composition 和 descriptor seal
Gate D：cache identity、隔离、容量与淘汰
Gate E：handoff ticket、readiness、失败恢复
Gate F：EditorSession/PlayService production consumption
Gate G：三项目 + workspace 外项目真实 composition
Gate H：Tower Defense P0-5 GameView、Pause/Step、AUI/Inspector/GPU/字体资格
Gate I：受影响 crate 全量回归与文档/入口归档
```

具体测试命令、Windows production artifact 资格、是否运行 Local CI、是否替换安装态二进制，必须
由后续施工文档和用户授权单独确定；本文不预授权这些外部状态变更。

## 22. 预计涉及文件

以下只用于后续施工范围评估，不是当前修改清单：

```text
rust/crates/editor_core/src/editor_preview_package.rs
rust/crates/editor_core/src/project_player_artifact.rs
rust/crates/editor_core/src/project_runtime_player_staging.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_window_winit/src/linked_project_runtimes.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/Cargo.toml
可能新增：project_editor_composition_artifact.rs
可能新增：editor_project_composition_launcher.rs
对应 owner/consumer/production composition tests 与 fixtures
```

施工前必须重新定位真实 owner，优先深化现有 `ProjectRuntimePlayerArtifact` staging/build 能力，
避免复制 Cargo/build/process/cache 实现。若 `editor_window_winit` 不是合适的 artifact owner，施工
文档可调整内部文件落点，但不得改变本文 interface 和失败合同。

## 23. 与既有方案关系

### 23.1 242

262 是 242 的 Editor build-generated static set 落地：

```text
保持 ProjectRuntimeBootstrap exact bind；
保持 RuntimePackage required descriptor；
保持 LinkedProjectRuntimeSet construction-time lookup；
保持 module_not_linked -> rebuild/relaunch；
保持拒绝 dylib；
把“中央多样例静态 set”收敛为“项目专属 singleton composition”。
```

### 23.2 250-F / 256 / 257 / 258

项目启动仍从现有 Editor launcher/application composition 进入；handoff 必须保留确定性项目启动、
稳定 Editor instance/Gateway 生命周期，以及 workspace layout/floating window state。262 不新增第二套
Editor shell 或 Gateway。

### 23.3 260

262 只保证塔防 `ProjectRuntimeSession` Adapter 真正链接进 Editor；AUI intent exactly-once、
FixedUpdate、deferred World mutation 和 session 生命周期继续由 260 owner 执行。

### 23.4 261 / 261-R1

262 复用字体和 RuntimePackage 既有链，不改变 FontBundle。Cargo/source/dependency staging 复用
261-R1 普通项目策略，不放宽 SourcePatch 最小依赖策略。

### 23.5 塔防 P0-5

262 是独立引擎缺口方案。塔防仍是普通外部用户项目：

```text
引擎施工不得写入塔防 gameplay 特例；
塔防施工不得静态修改引擎默认 linked set；
262 完成并通过真实通用资格后，才恢复 P0-5 Gate G 的 Editor 可玩闭环。
```

## 24. 回滚与停止条件

### 24.1 可回滚单位

```text
schema/module owner
composition builder
cache
launcher/handoff
Editor production consumption
```

每个 Gate 应保持单独可回滚；未通过 Gate 的 artifact 不进入下一 Gate，也不替换 production/
安装态二进制。

### 24.2 必须停止并回到方案讨论

```text
实现需要稳定 Rust dylib ABI；
必须放宽为任意 Cargo dependency 才能继续；
必须让未受信项目在 Editor 进程执行；
无法在 readiness ack 前保留旧 Editor；
需要修改 ProjectRuntimeModule runtime interface 才能装配；
需要写入塔防/射击/拼图硬编码；
缓存隔离或路径 containment 无法证明；
实现要求直接替换 production/安装态 Editor 才能测试 owner 合同。
```

## 25. 方案自审

### 25.1 是否能适配其它用户项目

能，但范围是“受信任且符合受控 ProjectRust 合同的项目”，不是任意 Rust 项目。通用性由 typed
manifest、generated singleton composition、无项目名模板，以及三种玩法加 workspace 外项目 Gate
证明。

### 25.2 是否把塔防硬编码进引擎

否。塔防只作为真实 consumer 和资格项目，production 模板与引擎 source 禁止出现塔防 module ID
或玩法概念。

### 25.3 是否推翻 242

否。262 完成的是 242 已选择的 build-generated Editor composition；Bootstrap、descriptor exact
bind、static linked set 和 rebuild/relaunch 原则均保留。

### 25.4 是否误用 Player artifact

否。Player artifact/staging 提供可复用的受控构建基础和 identity 证据，但 Editor 生成自己的
composition executable；不会把 Player executable 当作 DLL 注入 GameView。

### 25.5 是否保留进程内编辑体验

是。专属 Editor 启动后仍使用原有 `EditorGameViewPlayRunner`、Pause/Step、GPU texture、AUI 和
Inspector。只有 native composition identity 改变才 handoff。

### 25.6 是否处理信任与依赖风险

是。native code 显式信任、stale receipt、261-R1 allowlist、locked/offline、路径 containment 和
失败关闭均为 v1 必须合同；未将其推迟为“以后再补”。

### 25.7 是否控制缓存和项目切换成本

是。cache key 覆盖 engine/toolchain/module/manifest/dependency/lock identity，并要求容量上限、
淘汰和项目隔离。项目切换以 readiness ack handoff 保证旧 Editor 可恢复。

### 25.8 是否过早扩大范围

否。dylib、native hot reload、未受信隔离运行、任意依赖和通用安全平台均明确 deferred。

### 25.9 自审结论

```text
结论：通过
必须修改正式方案：无
当前可施工：否
原因：262 施工文档仍在待执行，尚未激活，当前没有施工授权
```

## 26. 下一步

下一步只能在用户明确授权后执行：

```text
对 262 Window 1 做激活前复核
  -> 对照当前代码/工作树基线修订并重新自审
  -> 移动施工文档到 当前/ 并同步 54
  -> 确认 Window 1 授权仍有效
  -> 分 Gate 实施与验证
```

在 262 施工完成前，塔防 P0-5 不得通过把塔防 RuntimeModule 静态加入默认 production Editor、
使用 empty runtime 或外部 Player 替代进程内 GameView 来伪装 Gate G 完成。
