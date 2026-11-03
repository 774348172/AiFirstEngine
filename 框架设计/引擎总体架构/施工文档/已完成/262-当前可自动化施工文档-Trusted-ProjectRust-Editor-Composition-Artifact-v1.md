# 262 当前可自动化施工文档：Trusted ProjectRust Editor Composition Artifact v1

## 1. 文档状态

```text
系统编号：262
施工文档版本：v1
建立日期：2026-07-31
正式方案：../../262-Trusted-ProjectRust-Editor-Composition-Artifact-v1方案.md
所在队列：施工文档/已完成/
当前状态：Window 5 Gate I 已完成，262 全部 Gate 已关闭并归档
当前施工槽：已释放
当前施工授权：无（Window 5 Gate I 已于 2026-08-01 完成）
```

本文已按五个独立授权窗口完成 Gate A-I 并归档。施工期间只在 repository-external、run-owned
隔离根中构建并启动真实 specialized Editor candidate、真实 Windows window/GPU，并只读消费 Tower Defense
项目；未修改用户真实 trust store，未运行 Local CI，也未替换 production/安装态二进制。

## 2. 施工目标

把 242 已定义但未落地的 Editor build-generated static composition 做成通用 production 链：

```text
受信 ProjectRust project
  -> typed trust decision
  -> 261-R1 production staging
  -> project-specific singleton Editor composition artifact
  -> sealed identity + bounded cache
  -> one-time handoff ticket
  -> candidate Editor readiness ack
  -> existing in-process GameView / Pause / Step / AUI / Inspector / GPU present
```

完成后必须同时成立：

```text
默认 production Editor 不静态依赖 Complex Shooter、Switch Puzzle 或 Tower Defense RuntimeModule。
默认 Editor 只能运行 built-in empty composition；打开 ProjectRust 时必须 prepare/handoff。
项目专属 Editor 只链接当前项目一个 RuntimeModule Adapter。
项目 Rust/AOT identity 变化才重建并重启；数据资产变化只重建 RuntimePackage。
信任、dependency、descriptor、cache、handoff 任一不匹配都 fail closed。
构建/launch 失败不替换 production/安装态 binary，不提前退出旧 Editor。
```

## 3. 非目标与硬边界

本施工禁止：

```text
Rust dylib、native hot reload、live unload；
把 Player executable 当 DLL 注入 Editor；
把塔防或任一样例 moduleId 写入 generated template / production switch；
新增 Logic Ownership Router、EventBus、plugin marketplace 或通用安全平台；
放宽为任意 Cargo dependency、path/git/build.rs/proc-macro；
允许未受信 native code 先运行再补 receipt；
改变 ProjectRuntimeModule / ProjectRuntimeSession runtime interface；
修改塔防玩法、AUI、Scene 或数据来迁就引擎 Gate；
使用 empty runtime、旧样例 linked set 或外部 Player 伪装进程内 GameView；
自动替换 production/安装态 Editor；
自动运行 Local CI。
```

塔防目录在 Gate H 以前只作为只读 consumer。若通用引擎施工必须改塔防项目才能通过，停止并
把问题分类为方案错误、项目缺口或 fixture 缺口，不得直接越界修改。

## 4. 当前代码基线

### 4.1 已有 owner

```text
ProjectManifest / ProjectRuntimeModuleBuildSpec
  rust/crates/editor_core/src/project_launcher.rs

ProjectRuntime player build/staging
  rust/crates/editor_core/src/project_player_artifact.rs
  rust/crates/editor_core/src/project_runtime_player_staging.rs

Preview package / player artifact parity
  rust/crates/editor_core/src/editor_preview_package.rs

Editor GameView consumption
  rust/crates/editor_core/src/services/play_service.rs
  rust/crates/editor_core/src/editor_gameview_play.rs
  rust/crates/editor_core/src/session.rs

Editor composition root
  rust/crates/editor_window_winit/src/linked_project_runtimes.rs
  rust/crates/editor_window_winit/src/application.rs
  rust/crates/editor_window_winit/src/real_window.rs
```

### 4.2 已确认缺口

```text
EditorPreviewPackageService 已构建 ProjectRuntimePlayerArtifact。
PlayService 没有消费该 artifact，也不应把它动态加载。
EditorSession 仍由 default_editor_linked_project_runtimes() 注入静态样例 set。
editor_window_winit regular dependencies 仍包含 complex/switch RuntimeModule。
塔防 package descriptor 因不在 linked set 中正确失败为 project_runtime.module_not_linked。
仓库目前没有 ProjectRuntime trust receipt owner，也没有 composition handoff owner。
```

### 4.3 当前工作树约束

生成本文时工作树包含 260/261、字体、session、staging、样例项目和塔防项目的未提交改动及
untracked 文件。它们视为用户/前序施工成果，不得 reset、checkout、revert 或覆盖。

激活前必须：

```text
记录 git status 和相关文件 identity。
确认 260/261 完成记录对应的源码确实是 262 基线。
确认 project_runtime_player_staging.rs 已存在并通过 261-R1 合同。
对与 262 重叠的 editor_core/editor_window_winit 文件逐个合并，不作整文件回写。
若基线漂移使本文 test/filter/owner 失效，先更新本文并重新自审。
```

## 5. 正式 module 与 interface

### 5.1 ProjectEditorCompositionArtifact

优先落在 `editor_core`，外部 interface 保持一个 prepare：

```rust
ProjectEditorCompositionArtifact::prepare(request)
    -> ProjectEditorCompositionBuildReport
```

它必须是深 module，内部拥有：

```text
manifest/trust/staging identity
generated Cargo/source
bounded child process
descriptor seal
cache lookup/invalidation/publish
diagnostic/report
```

禁止把 `validate_manifest`、`copy_source`、`write_cargo`、`run_cargo`、`seal`、`publish` 等内部
步骤全部暴露给 caller。测试通过 prepare interface 验证结果；内部 process/filesystem Adapter
只供 owner 测试。

### 5.2 ProjectRuntimeTrustDecision

新增窄型 trust owner，建议落在 `editor_core/src/project_runtime_trust.rs`：

```text
project-runtime-trust-decision.v1
decision = trusted | denied | stale
decisionSource = explicit_user | repository_policy
```

receipt store 必须位于 application-owned state root，不写进项目目录，避免项目自行伪造信任。
测试注入独立 temp state root；production 默认根沿用现有 Editor application state 解析方式，不新造
环境变量协议。

receipt identity 至少绑定：

```text
canonical project root identity
projectId
RuntimeModule source digest
normalized manifest/dependency digest
Engine/Editor build identity
```

### 5.3 EditorProjectCompositionLauncher

public interface：

```rust
EditorProjectCompositionLauncher::handoff(artifact, project, state)
    -> EditorCompositionLaunchReceipt
```

process、ticket、ack、timeout、cleanup 复杂度留在 module 内。真实 Windows process Adapter 放在
`editor_window_winit` 或其现有 application launch owner；纯 schema/state machine 保持在
`editor_core`，避免 core 依赖 winit/Windows。

### 5.4 Editor application injection

`editor_window_winit` 增加项目无关启动 interface，语义等价：

```rust
run_editor_with_composition(EditorCompositionLaunchInput)
```

输入至少包含：

```text
singleton LinkedProjectRuntimeSet
sealed composition descriptor
optional validated handoff ticket
project root
workspace state reference
readiness ack sink
```

generated binary 只能跨该 interface 启动 Editor，不复制 application/real-window loop。

## 6. Schema 与 diagnostic 固定集合

### 6.1 Schema

```text
project-editor-composition-build-request.v1
project-editor-composition-artifact.v1
project-editor-composition-descriptor.v1
project-editor-composition-build-report.v1
project-editor-composition-handoff-ticket.v1
project-editor-composition-launch-receipt.v1
project-runtime-trust-decision.v1
```

persisted schema 必须有 `schemaVersion`，关键输入使用 `deny_unknown_fields` 或等价严格解析；新增
兼容字段必须显式 default，不能让错误拼写静默丢失。

### 6.2 Diagnostic

```text
project_editor_composition.trust_required
project_editor_composition.trust_stale
project_editor_composition.manifest_rejected
project_editor_composition.dependency_rejected
project_editor_composition.cache_invalid
project_editor_composition.cache_capacity_exceeded
project_editor_composition.build_failed
project_editor_composition.artifact_descriptor_mismatch
project_editor_composition.engine_identity_mismatch
project_editor_composition.module_not_linked
project_editor_composition.handoff_ticket_invalid
project_editor_composition.readiness_timeout
project_editor_composition.launch_failed
project_editor_composition.cleanup_failed
```

每条 diagnostic 至少包含：

```text
code
stage/domain
message
controlled path（可选）
expected/actual identity 摘要（可选）
nextAction
```

Summary 不记录源码、secret、完整环境变量或无界 stdout/stderr；Trace 只在显式测试/诊断中保留
受限 child output。

## 7. Cache 与资源预算

262 只管理自己拥有的 composition root，不删除或配额管理用户全局 Cargo home。

v1 默认 policy 集中在一个 typed config：

```text
globalSoftLimit = 24 GiB
globalHardLimit = 32 GiB
perProjectSoftLimit = 8 GiB
perProjectHardLimit = 12 GiB
prunePolicy = LRU to soft limit before publish
activeArtifact = pinned
failed/incomplete staging = cleanup candidate
```

测试必须可注入 KiB/MiB 级小额度，不实际制造 GiB fixture。超过 hard limit 且无法安全淘汰时
返回 `cache_capacity_exceeded`，不能删除 active artifact、其它非 owned root 或继续无界构建。

Cache key 必须覆盖 262 方案第 15 节全部 identity。cache hit 仍重新校验 descriptor 和 executable
hash；metadata 缺失、schema 不支持、hash 不一致或 identity 不完整均视为 stale/invalid。

## 8. Handoff 协议

### 8.1 Ticket

一次性 ticket 放在 launcher-owned、严格受控的 handoff root，至少包含：

```text
schemaVersion
nonce
oldEditorInstanceId
expected new composition identity
expected project root/projectId/module descriptor
artifact executable/hash
workspace state reference
createdAt/expiresAt
ack path/identity
```

ticket 使用 create-new + atomic publish；candidate 必须验证路径 containment、expiry、nonce、artifact
hash 和所有 expected identity。ticket consumed 后不能 replay。

### 8.2 Ack

production readiness ack 只能在以下条件全部成立后发布：

```text
candidate process 是目标 sealed executable；
linked singleton descriptor exact match；
project manifest/readiness 通过；
EditorSession 已绑定目标 composition；
native window/application 首帧可用；
Gateway host 已 ready，或当前 composition 明确以测试模式禁用；
workspace state 已加载或给出结构化 recoverable diagnostic。
```

旧 Editor 只在 nonce + composition + project + candidate process identity 匹配的 success ack 后退出。
launch failure、timeout、错误 ack 或 candidate crash 均保留旧 Editor 和当前项目状态。

### 8.3 UI 不阻塞

Cargo build 和 candidate readiness wait 不得阻塞 winit event loop。Editor 显示结构化 preparation
状态；取消只终止本次 owned build/launch worker，不杀其它 Cargo/Editor process。

## 9. 授权窗口总览

为满足三小时硬限制，施工拆成五个独立授权窗口：

```text
Window 1：Gate A-B，合同、信任、staging 复用
Window 2：Gate C-D，generated artifact、descriptor seal、cache
Window 3：Gate E-F，handoff lifecycle、Editor production consumption
Window 4：Gate G-H，通用项目矩阵、真实 Windows/Tower Defense 资格
Window 5：Gate I，受影响回归、文档闭环与归档
```

每个窗口需要单独明确授权；一个窗口结束不自动开始下一个。每个窗口在 2:45 后不启动新实现或
验证，3:00 必须停止并记录 `stopped_by_time_limit`。窗口失败只修当前 Gate，不跨 Gate 堆积。

## 10. Window 1 - Gate A：Schema / Identity / Trust

### 10.1 红测试

先建立通过 public interface 失败的 owner tests：

```text
所有 schema round-trip 与 unknown field rejection；
composition identity 跟踪 engine/toolchain/target/profile/module/AOT/manifest/dependency/lock；
identity 任一字段变化导致不相等；
外部/未知来源 ProjectRust 默认 trust_required；
explicit trusted receipt exact match 才通过；
source/manifest/dependency/engine identity 变化 -> stale；
denied/stale/untrusted 不执行 build Adapter；
project-owned forged receipt 不被读取；
repository policy 只接受明确的 repository-owned sample fixture，不按路径字符串猜测。
```

### 10.2 实现

```text
新增 schema/identity/report types。
新增 ProjectRuntimeTrustDecision 深 module 和 injectable receipt store。
复用 SafeProjectPath/ProjectWriteScope 的 canonical/containment 语义。
把 trust decision 放在 staging/build 之前。
为 Editor caller 返回 needs_user_decision，而不是 bool/字符串错误。
```

### 10.3 测试命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_contract_ -- --nocapture
cargo test -p editor_core project_runtime_trust_ -- --nocapture
```

任一 filter 运行 0 tests 视为 Gate 失败。

### 10.4 退出条件

```text
schema/identity/trust owner tests 全绿。
未受信 native code build Adapter 调用次数为 0。
没有写项目目录、用户真实 application state 或 production binary。
```

## 11. Window 1 - Gate B：Production Staging Reuse

### 11.1 红测试

```text
composition 使用与 ProjectRuntimePlayerArtifact 相同的 normalized staging plan；
Engine SDK、serde、serde_json 正向；
path/git/build.rs/.cargo/config/unsupported crate/escape 否定；
源 Cargo.toml/Cargo.lock/source byte-exact 不变；
--locked --offline；
composition 不复制一份更宽松的 dependency validator；
SourcePatch 最小依赖策略仍拒绝第三方依赖。
```

### 11.2 实现

优先把 261-R1 内部 staging 能力深化为可由 Player artifact 与 Editor composition 共同消费的
私有 module interface；不得复制解析、allowlist 和 source-copy 实现。

允许的窄重构：

```text
ProjectRuntimeProductionStaging::plan(...)
ProjectRuntimeProductionStaging::stage(...)
```

这是内部 interface，不扩大 AI-facing 或项目 public interface。

### 11.3 测试命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_staging_ -- --nocapture
cargo test -p editor_core project_runtime_player_production_staging_ -- --nocapture
cargo test -p editor_core rejects_cargo_build_workspace_target_and_external_dependencies -- --nocapture
```

该 SourcePatch filter 已在 2026-07-31 激活前复核中对齐真实 test name；不得接受 0 tests。

### 11.4 Window 1 关闭

记录：

```text
实际修改文件
owner tests 数量
first failure 与修复
未运行的后续 Gate
外部状态=未修改
elapsed time
```

Window 1 关闭后等待 Window 2 授权。

2026-07-31 Window 1 实际关闭记录：

```text
Gate A：passed
- 新增 project_editor_composition.rs typed schema/identity/build-report/handoff/receipt 合同。
- identity 覆盖 project/module/interface/AOT、Editor build、Engine SDK、toolchain、target/profile、
  normalized manifest/dependency/lock，并以 validate() 拒绝未知 schema、空 required field 和非
  sha256:<64 lowercase hex> identity。
- 新增 application-owned ProjectRuntimeTrustModule；explicit user/repository policy、trusted/denied/
  stale/required、exact project id + canonical path、atomic receipt 与 project-inside-store 防护通过。
- supplied trust root 在 canonicalize 前检查；Windows test-owned junction/reparse root 拒绝通过。

Gate B：passed
- ProjectRuntimeProductionStaging::plan/stage 成为 Player artifact 与 Editor composition 共用的内部
  deep owner，没有复制 dependency parser、allowlist、normalization、lock 或 source-copy 实现。
- Engine SDK + serde + serde_json 正向通过；path/git/build.rs/.cargo/config/target/unsupported crate
  否定通过；SourcePatch 最小依赖策略保持拒绝第三方依赖。

实际产品源码修改：
- rust/crates/editor_core/src/project_editor_composition.rs
- rust/crates/editor_core/src/project_runtime_trust.rs
- rust/crates/editor_core/src/project_runtime_player_staging.rs
- rust/crates/editor_core/src/project_player_artifact.rs
- rust/crates/editor_core/src/lib.rs

验证：composition contract 2/2；trust 6/6；shared composition staging 1/1；production staging 4/4；
SourcePatch policy 1/1；cargo fmt --all -- --check passed。
first failure：junction test 使用 unwrap_err() 时要求 success type 实现 Debug，属于 test compile failure；
修复为显式 match，没有扩大 production interface，随后 trust 6/6 passed。
未运行：Window 2-5、真实 Editor、production composition artifact、Local CI。
外部状态：未修改真实 application trust store；未替换 production/安装态二进制；仅创建并清理 test-owned temp/junction。
elapsed time：约 21 分钟（21:24-21:45，Asia/Shanghai），未触及 2:45/3:00 限制。
```

当前施工文档继续保留在 `施工文档/当前/` 并占用唯一施工槽；Window 2 未授权，不得自动进入。

## 12. Window 2 - Gate C：Generated Composition Artifact

### 12.1 红测试

```text
typed request 生成 deterministic Cargo/source；
generated template 无 shooter/puzzle/tower-defense 名称或 moduleId；
generated Cargo 只依赖 editor_window_winit + normalized RuntimeModuleBuild + required SDK；
linked_project_runtime.rs 构造 singleton LinkedProjectRuntimeSet；
Adapter descriptor 与 expected exact match；
编译后 executable descriptor query/hash seal 通过；
wrong module/interface/AOT/editor identity/executable hash fail closed；
build timeout/output limit/non-zero exit 产生结构化 report；
源项目和 installed/production Editor 不被修改。
```

### 12.2 实现

```text
新增 ProjectEditorCompositionArtifact::prepare。
新增 generated composition source templates，模板集中且由 typed values 渲染。
复用 runtime_cli bounded child process 与 261-R1 build capture policy。
使用 isolated staging/target/publish root。
成功 publish 前完成 descriptor query + executable SHA256 + atomic descriptor/report write。
partial artifact 永不成为 cache hit。
```

### 12.3 Editor crate 收敛

```text
editor_window_winit regular dependencies 移除 complex_shooter_project_runtime 和
switch_puzzle_project_runtime。
样例依赖若仍用于 owner test，只能进入 dev-dependencies 或 test fixture owner。
default_editor_linked_project_runtimes() 收敛为 explicit empty。
generated binary 通过项目无关 run_editor_with_composition interface 注入 singleton set。
```

不得让默认 Editor 通过 feature flag 手写项目列表。

### 12.4 测试命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_generated_ -- --nocapture
cargo test -p editor_core project_editor_composition_artifact_ -- --nocapture
cargo test -p editor_window_winit default_editor_composition_ -- --nocapture
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

### 12.5 退出条件

```text
至少一个 disposable fixture 真实编译出 sealed specialized Editor artifact。
artifact linked set 只有 empty + 当前项目 Adapter；len/descriptor 均可证明。
默认 Editor regular dependency graph 不包含样例 RuntimeModule。
```

## 13. Window 2 - Gate D：Cache / Invalidation / Cleanup

### 13.1 红测试

```text
完全相同 identity cache hit 且不重跑 Cargo；
每一 key domain 变化都 miss；
descriptor/hash 篡改 invalidates；
两个项目最终 artifact/staging 隔离；
LRU 按测试注入额度淘汰到 soft limit；
active artifact pinned；
hard limit 无法释放时 fail closed；
失败/中断 staging bounded cleanup；
cleanup 不越出 owner root，不跟随 reparse point；
retained_by_host_policy 可报告且不伪装 removed。
```

### 13.2 实现

cache 和 cleanup 保持在 composition deep module 内部；不新建 repository-wide construction
runner，不删除用户 Cargo cache，也不复用 `.artifacts/` 中其它历史证据目录。

### 13.3 测试命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_cache_ -- --nocapture
cargo test -p editor_core project_editor_composition_cleanup_ -- --nocapture
```

### 13.4 Window 2 关闭

保存 build report/cache report 和测试摘要到施工拥有的证据根；清理 disposable staging，保留能够
复现首因的失败证据。等待 Window 3 授权。

2026-07-31 Window 2 实际关闭记录：

```text
Gate C：passed
- ProjectEditorCompositionArtifact::prepare(request) 已成为 generated source、bounded Cargo、descriptor
  query、identity match、SHA-256 seal、atomic publish 与 typed report 的唯一外部 interface。
- generated template 使用 project_runtime 通用依赖别名，不包含 shooter/puzzle/tower-defense 名称或 moduleId。
- default Editor regular dependencies 已移除 complex_shooter_project_runtime 与
  switch_puzzle_project_runtime；default linked set 只含 built-in explicit empty。
- editor_window_winit 新增项目无关 run_real_native_editor_window_with_composition interface；generated
  binary 注入 singleton LinkedProjectRuntimeSet，未启动真实 Editor。
- disposable fixture 真实 release 编译 specialized Editor，descriptor query/module/interface/AOT exact match，
  executable hash seal passed；第二次 prepare cache hit 且 Cargo steps=0。

Gate D：passed
- cache key 覆盖 project/module/interface/AOT、Editor/SDK/toolchain/target/profile、normalized
  manifest/dependency/lock；双项目 key/staging identity 隔离。
- descriptor 与 executable tamper invalidation passed；partial/missing report 不成为 hit。
- injectable byte quota 下 LRU to soft、active pin、hard-limit fail closed passed。
- non-zero/timeout/output-truncation 均返回 bounded process report，失败 staging removed。
- project-inside build root 零写入；cache cleanup 不跟随 Windows junction，unsafe cleanup 报
  retained_by_host_policy。

实际产品源码修改：
- rust/crates/editor_core/src/project_editor_composition.rs
- rust/crates/editor_core/src/project_editor_composition_artifact.rs（新增）
- rust/crates/editor_core/src/lib.rs
- rust/crates/editor_window_winit/Cargo.toml
- rust/crates/editor_window_winit/src/linked_project_runtimes.rs
- rust/crates/editor_window_winit/src/real_window.rs
- rust/Cargo.lock（移除 default Editor 样例 RuntimeModule regular dependency edge）

验证：contract 2/2；generated 1/1；artifact 2/2（含一次真实 release build + cache hit，
及 non-zero/timeout/output 三模式）；cache 3/3；cleanup 2/2；default composition 1/1；
cargo check -p editor_window_winit --features real-window,real-wgpu-surface passed；
cargo fmt --all -- --check passed；normal dependency tree 无 complex/switch runtime。

真实 artifact 证据：
- evidence=.artifacts/262-window2/generated-composition/
- identityDigest=sha256:56cf8385b6a3a0a4b0fb7bd0d5397f69f6259a2dc437b545b1353eefc3f16f1a
- executableHash=sha256:60ff2744811f5c1151320561ed76538b98991ecc955c43c23c6b72d781fab3ff
- artifactSizeBytes=37697896

first failures 与修复：
1. owner test compile：实际枚举为 Timeout，且 Drop fixture 禁止 move；修正测试表达，未扩大产品 interface。
2. Windows isolated target 使用 canonical \\?\ path，link.exe 报 LNK1104/LNK1181；owner 在完成
   metadata/canonical 安全校验后规范化 drive/UNC verbatim path，并缩短 staging leaf，真实 build passed。
3. G:/ 测试路径被 mklink 当作 switch；test helper 转为反斜杠，真实 junction cleanup test passed。

外部状态：未启动真实 Editor；未修改真实 trust store；未替换 production/安装态二进制；未运行 Local CI。
cleanup：最终成功 run-owned build/staging 已清理。两次早期失败 fixture 经 exact path、内容和 reparse
检查后尝试递归及逐叶删除均被宿主策略拒绝，未发生部分删除，记录 retained_by_host_policy：
- G:\Aife262W2\cleanup-reparse-20968-1785512018133605100
- G:\Aife262W2\cleanup-reparse-7500-1785512092260248100
elapsed time：约 2 小时 2 分钟（约 21:46-23:48，Asia/Shanghai），未触及 2:45/3:00 限制。
```

当前施工文档继续保留在 `施工文档/当前/` 并占用唯一施工槽；Window 3 未授权，不得自动进入。

## 14. Window 3 - Gate E：Trust UI / Handoff Lifecycle

### 14.1 Trust UI

production Editor 打开需要 native code 的项目时：

```text
先解析 manifest 和 identity，不执行 RuntimeModule。
没有有效 receipt 时显示项目 Runtime 信任确认界面。
界面明确显示项目名、canonical root、moduleId 和 dependency 摘要。
只有显式 Approve 才写 trusted receipt 并进入 prepare。
Deny 写 denied receipt/结果并留在当前 Editor。
关闭/取消等价于未授权，不构建、不启动。
identity stale 时重新确认，不静默沿用旧批准。
```

优先复用现有 Editor retained UI/command/feedback 模式；不得复用 Gateway mutation approval receipt，
两者 identity 和授权语义不同。若需要新增 UI model，只增加一个窄型 trust prompt model 和
Approve/Deny commands，不建设通用安全中心。

2026-08-01 Window 3 激活复核确认现有 `EditorUiModel` 无法表达 ProjectRust native trust，
且 Gateway access request 不可复用。因此启用 19.3 的条件 UI 范围：

```text
editor_ui_model::ProjectRuntimeTrustPromptModel
UiCommandPayload::ApproveProjectRuntimeTrust / DenyProjectRuntimeTrust / CancelProjectRuntimeTrust
editor_ui_renderer retained modal declaration
NativeEditorApplication trust prompt owner/command consumption
```

该界面只显示 project name/canonical root/moduleId/dependency summary/changed identity，只持有
pending request identity，不持有 RuntimeModule 或运行时指针。Approve/Deny 仅写 application-owned
`ProjectRuntimeTrustModule` receipt；Cancel 不写 receipt。对应 owner tests 纳入
`project_runtime_trust_prompt_` filter，renderer test 同该 filter 运行。此细化不增加通用安全中心，
不修改正式 262 方案。

### 14.2 Handoff owner tests

```text
same composition identity -> no relaunch；
different identity -> save state, prepare, launch candidate；
ticket create-new/expiry/nonce/containment；
ticket replay、wrong artifact/project/editor identity 拒绝；
candidate crash/non-zero/timeout/wrong ack -> old Editor stays；
success ack 之前 old exit Adapter 调用次数为 0；
success ack exact match 后只请求一次 graceful exit；
build/ack wait 不阻塞 application tick；
cancel 只终止 owned worker/process；
ticket/ack cleanup 有结构化结果。
```

### 14.3 实现

```text
新增 EditorProjectCompositionLauncher 状态机。
注入 process/clock/ticket/workspace/exit Adapters 供 owner test。
使用一次性 file ticket + atomic ack；不新增网络服务。
旧 Editor flush 现有 workspace layout/state，ticket 只引用受控 state，不序列化运行时指针。
worker completion 在 Editor stable point 消费。
```

### 14.4 测试命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core editor_project_composition_launcher_ -- --nocapture
cargo test -p editor_window_winit project_runtime_trust_prompt_ -- --nocapture
cargo test -p editor_window_winit project_editor_handoff_ -- --nocapture
```

## 15. Window 3 - Gate F：Editor Consumption / Play Parity

### 15.1 实现

```text
application startup 解析 validated composition launch input。
NativeEditorApplication/EditorSession 使用 injected singleton linked set。
项目 open 比较 requested/running composition identity。
identity mismatch 不创建可 Play 的错误 session，进入 prepare/handoff。
identity match 才附加 ProjectSession 并允许 Preview/Play。
PlayService 继续使用 self.linked_project_runtimes + ProjectRuntimeBootstrap。
Preview player_artifact 只作 identity parity 证据，不作动态加载入口。
Pause/Step/Resume/Stop、GPU texture、AUI 和 Runtime Inspector 路径不分叉。
```

### 15.2 否定要求

```text
默认 empty Editor 打开 ProjectRust 后直接 Play -> 必须拒绝并要求 handoff。
specialized Editor 打开另一 ProjectRust -> 必须再次 handoff。
moduleId 相同、AOT 不同 -> 必须 handoff/rebuild，不能旧 artifact Play。
Scene/AUI/Rule/Asset 改动 -> 只 dirty RuntimePackage，不触发 composition rebuild。
任何 mismatch 不 fallback 到 empty/样例 set。
```

### 15.3 测试命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_play_ -- --nocapture
cargo test -p editor_core editor_gameview_project_runtime_session_ -- --nocapture
cargo test -p editor_core editor_gameview_play -- --nocapture
cargo test -p editor_window_winit editor_composition_startup_ -- --nocapture
cargo test -p editor_window_winit native_editor_application_toolbar_play_runs_for_open_project -- --nocapture
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

### 15.4 Window 3 关闭

必须证明 default/specialized composition 的 production dependency 和 runtime bind 行为不同且正确，
并保存 old-stays/new-ready 的 lifecycle evidence。等待 Window 4 授权。

### 15.5 Window 3 实际关闭记录（2026-08-01）

```text
状态：Gate E-F completed and closed
下一步：等待 Window 4 单独授权；不得自动执行 Gate G-H
外部状态：未启动真实 Editor；未修改真实用户 trust store；未替换 production/安装态二进制；未运行 Local CI
证据根：.artifacts/262-window3/window3-summary.md
```

实际完成：

```text
ProjectRuntimeTrustPromptModel + Approve/Deny/Cancel retained modal 与 editor_input routing；
ProjectRuntimeTrustModule 保持唯一 receipt owner，changed Editor build identity 会 stale；
EditorProjectCompositionLauncher 的 create-new ticket、exact ack、timeout/cancel/failed candidate old-stays；
worker completion 只在 NativeEditorApplication frame stable point 消费；
generated specialized Editor 解析唯一绝对 handoff ticket 参数；
candidate 启动前校验 ticket/project/composition/executable/hash/expiry，主窗口首次成功 present 后 exactly-once 写 ready ack；
EditorSession 保存 running composition identity，Project open 与 Play 对 project/module/interface/AOT/singleton exact match；
mismatch 返回 project_editor_composition.handoff_required，不 fallback 到 empty runtime 或外部 Player。
```

验证结果：

```text
editor_project_composition_launcher_：3 passed
project_runtime_trust_prompt_：1 passed
project_editor_handoff_：2 passed
project_editor_composition_play_：1 passed
editor_gameview_project_runtime_session_：1 passed
editor_gameview_play：8 passed
editor_composition_startup_：1 passed
native_editor_application_toolbar_play_runs_for_open_project：1 passed
project_editor_composition_generated_：1 passed
project_editor_composition_artifact_：2 passed（真实隔离 release build，不启动产物）
cargo check -p editor_window_winit --features real-window,real-wgpu-surface：passed
cargo fmt --all -- --check：passed
git diff --check：passed
```

首次失败与修复：

```text
一次并发 Cargo 验证造成共享 target/package-lock 等待；测试本身已通过，改为串行后干净通过，分类为验证编排错误。
generated source 回归发现 Windows canonical \\?\ SDK root 比较不一致；在 trusted_sdk_crate 内规范化 trusted root 后通过。
project_editor_handoff_ 首次工具调用在编译完成前 124 秒超时；确认无遗留 cargo/rustc 后串行重跑 2/2 通过，分类为验证编排超时。
real-window feature check 首次发现 authority 聚合初始化器漏填 candidate_readiness；补 None 后相同命令通过，分类为本窗口实现错误。
```

## 16. Window 4 - Gate G：通用项目与外部项目矩阵

### 16.1 项目矩阵

```text
Complex Shooter
Switch Puzzle
Tower Defense
Engine workspace 外部 ProjectRust fixture
```

前三个项目作为不同 gameplay/runtime consumer；外部 fixture 在 repository-external、run-owned temp
root 中从受控 source fixture 复制生成，证明没有 `samples/`、workspace membership 或相对路径假设。

所有项目必须走：

```text
manifest -> trust -> staging -> generated composition -> descriptor seal
-> launch input -> ProjectRuntimeBootstrap bind -> GameView Play
```

禁止 test-only `EditorSession::with_linked_project_runtimes` 直接注入后宣称 production
composition 通过。

### 16.2 测试命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p project_e2e_gate project_editor_composition_ -- --nocapture
cargo test -p editor_core project_editor_composition_external_project_ -- --nocapture
cargo test -p complex_shooter_project_runtime -- --nocapture
cargo test -p switch_puzzle_project_runtime -- --nocapture
cargo test -p tower_defense_project_runtime -- --nocapture
```

### 16.3 否定矩阵

```text
untrusted/stale/denied
unknown schema/interface
same moduleId wrong AOT
wrong engine/toolchain/target/profile
duplicate linked module
unsupported dependency/path/git/build.rs/.cargo/config
source/manifest/lock/reparse escape
cache tamper
build timeout/output limit/non-zero
ticket replay/expiry/wrong ack/candidate crash
failed candidate leaves old Editor usable
production/installed binary unchanged
```

## 17. Window 4 - Gate H：真实 Windows / Tower Defense 资格

Gate H 是 release/activation 级证据，开始前必须明确授权真实 process/window/GPU 和 isolated
artifact root。默认不替换 production/安装态 Editor，不修改真实用户 trust store，不运行 Local CI。

### 17.1 资格输入

```text
isolated run root
当前 source identity
真实 generated specialized Editor executable/hash
Tower Defense project identity/module descriptor
repository_policy 或 run-owned trust receipt
real-window + real-wgpu-surface features
独立 stdout/stderr/report/screenshot/ack evidence paths
```

### 17.2 真实链

```text
default Editor/process prepares Tower Defense specialized artifact
-> candidate Editor validates one-time ticket
-> real native window first frame + readiness ack
-> old test-owned Editor observes ack and exits gracefully
-> Tower Defense GameView Play
-> visible scene + real glyph HUD
-> AUI action -> ProjectRuntimeSession -> World projection -> next-frame snapshot
-> Pause -> Step exactly once -> Resume -> Stop
-> Runtime Inspector reads active GameView runtime
-> GPU texture present report success
```

还必须验证：

```text
修改一份 run-owned Scene/AUI/Rule 数据只重建 RuntimePackage，不 handoff；
修改 run-owned RuntimeModule identity 使 receipt stale，并要求 rebuild/handoff；
失败 candidate scenario 保留旧 Editor；
真实 production/安装态 editor_host.exe hash 前后不变；
所有 owned child process 在结束/超时后终止并 join。
```

### 17.3 资格入口

施工时优先新增/复用 repository-owned ignored Windows gate，例如：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit --features real-window,real-wgpu-surface project_editor_composition_real_window_ -- --ignored --nocapture
```

该 filter 必须真实生成并启动 specialized artifact，不能只是 mock process。激活前根据最终 test
owner 修订精确命令。运行 0 tests、无 screenshot/ack/report、无 executable hash 或只跑默认
Editor 均视为 Gate H 失败。

### 17.3.1 2026-08-01 Window 4 当前施工记录

Gate G 已在 run root `G:\Aife262Runs\window4-20260801-01` 和当前 Editor source identity
`sha256:b20397e700482cf62948dc142bea1bdc9f9884e3b15044803ee13efbe2385ad2` 下通过完整四项目
production composition 矩阵：

```text
Complex Shooter       sha256:e487bc2f4c7a368d6150801a7341e8f944622f2c6ade4e4476520cc50626ace6 passed
Switch Puzzle         sha256:38803306d3212b85559f98afa8b0bcac83883e9f933080726475dfe6df5a153e passed
Tower Defense         sha256:e0d593af46493db01b14afb3e3f9521e5c95af3a5ae5502f2c4ba59a59e9941c passed
repository-external   sha256:e05912eb10d038a51a8fbf3eb2d9d0a99c51f4e8fd5404939a38148c9aed9a19 passed
```

四项均验证 Open/Play/Pause/Step/Resume/Stop、Pause 不推进、Step 恰好 +1、Resume 不重复推进、Stop
清理 runtime；Windows Job Object root wait、output reader join 和 process-group release 全部完成。正式报告：

```text
G:\Aife262Runs\window4-20260801-01\evidence\gate-g-report.json
```

Gate H 已修复并由真实 owner/前序 candidate 逐步暴露的通用 authority 与 qualification runner 问题：

```text
普通 click 按 Windows message 顺序投递 MouseMove -> LeftDown -> LeftUp；
winit 必须观察 pointer down/up，WidgetTree binding 和 canonical command 必须匹配；
td.recruit 资格输入补齐 PointerDown + PointerUp；
Play 后用正式 SelectRuntimeEntity 命令选择 active runtime entity，再验证 TemporaryPlaySession Inspector；
foreground 失败诊断记录 target/foreground HWND 与 PID，不放宽 fail-closed。
```

前序真实 candidate 已证明真实窗口/WGPU/screenshot、Play、shared GPU texture present、active RuntimePackage、
Pause/Step/Resume/Stop 和 next-frame hash 变化；其剩余 AUI/Inspector runner 缺口已按上文修复并通过 feature compile。
当前 source identity 的最终 Gate H 复跑被外部桌面状态阻断：foreground PID `11456` 为 Windows
`LockApp.exe`，current Tower candidate 正确返回 `authority_input.target_not_foreground`，没有生成伪 screenshot
或 runtime 结果。阻断报告：

```text
G:\Aife262Runs\window4-20260801-01\evidence\gate-h\gate-h-real-window-report.json
```

production/debug `G:\gameEngin\rust\target\debug\editor_host.exe` 前后 SHA256 均为
`1C4DBFF6402C0BE287F11E8B7B984CBD90CA6120E022FC0E71FD8923259BFF15`；没有替换 production/安装态
二进制，没有修改真实 trust store，没有运行 Local CI。当前无遗留 Cargo/rustc/candidate Editor 进程。

桌面解锁后继续施工，真实资格先后暴露并修复了两个 qualification owner 缺口：正式 Editor Play
使用 `ProjectRuntimeSessionReportLevel::Summary`，因此 AUI action handled 判定改为要求
`AuiActionDispatch + actionCount=1 + handled=1 + committed mutations`，不再错误要求仅 Trace 才有的
`actionTrace`；real-window capture 在 shared GameView texture present 后回填 renderer viewport texture
registry count/lifecycle，不再保留初始化时的 0。对应 qualification 单测 `2/2`、
`real-window,real-wgpu-surface` feature check 和 format check 均通过。

修复后的最终 source identity 为
`sha256:8a8ccee34055fc56bff4e9e13387309c7222b1b055e3b15100e8a1fb79c27660`。Gate G 已按该
identity 重新执行完整四项目矩阵，outer owner test `1/1`、四项目 qualification `4/4` 均通过，所有
Windows Job Object root wait/readers join/process group release 完成。最终 Tower Defense artifact：

```text
identity: sha256:0fbb8e53c274af88e4aabfa85ca1efa660bd4f0508a5b82a43b2cfa0b81df040
executable hash: sha256:b7105f9488f391ec9b181bf93a051b1c8e4e02d78b2d7e04fe5087d353415cf7
```

该 artifact 的真实 Windows/WGPU Tower Defense **结构化链资格 runner** 已通过：1600x900、scale factor
1.25、Vulkan shared context、OS foreground verified click、PointerDown/PointerUp、Play、active
RuntimePackage、temporary play-session Inspector、viewport texture registry `1` / lifecycle event `2`、GPU
present、非空截图、`td.recruit` handled、next-frame snapshot changed、Pause 不推进、Step +1、Resume
不重复推进、Stop 清理 runtime 全部成立。证据：

```text
G:\Aife262Runs\window4-20260801-01\evidence\gate-g-report.json
G:\Aife262Runs\window4-20260801-01\evidence\gate-g-final.stdout.log
G:\Aife262Runs\window4-20260801-01\evidence\gate-h\gate-h-real-window-report.json
G:\Aife262Runs\window4-20260801-01\evidence\gate-h\tower-defense-real-window.png
```

但人工像素复核发现 GameView 当前只呈现纯蓝 clear color，没有可辨认的 Tower Defense scene/board，
也没有真实 glyph HUD。因此 `visible scene + real glyph HUD` 尚未通过，非空 screenshot、
`uiDrawItemCount` 或 GPU present 不能代替该像素资格。Gate H 仍施工中；必须定位 shared GameView
texture 的 UI draw payload 为什么没有形成可见像素，并增加 exact shared texture readback/ROI 断言后重跑。

Gate H 的 mutation/handoff owner evidence 进一步确认：launcher exact ready / wrong ack / timeout / cancel
`3/3`，data-only identity stable 与 AOT changed handoff-required `1/1`，handoff CLI/stable-point consumption
`2/2`。但真实 production handoff 资格发现一个 Window 3 遗留实现缺口，不能用 test runner 伪装完成：

```text
NativeEditorApplication production constructor keeps project_runtime_trust_environment=None.
NativeEditorApplication production constructor keeps project_editor_composition_launcher=None.
Approve stores approved_project_runtime_trust, but no production consumer prepares artifact or starts handoff.
No non-test EditorCompositionProcessAdapter / WorkspaceAdapter / ExitAdapter installation exists.
```

因此默认 Editor 审批 ProjectRust 后的 `prepare -> artifact -> ticket -> candidate -> ready ack -> old graceful
exit` 当前仍不可达。Gate H 保持施工中；下一步必须先补齐 production composition orchestration 和真实 process
Adapter，再执行 run-owned data-only/AOT mutation 与真实 handoff/failure/cleanup 资格。不得把现有 mock owner tests
或直接启动 specialized candidate 误报为默认 Editor handoff；不进入 Window 5。

上段 production orchestration 缺口已在本轮后续施工中修复，不再代表当前实现。当前链路为：

```text
Application 稳定点消费 ApprovedProjectRuntimeTrustRequest；
后台 worker 重新校验 exact trust identity 并 prepare production composition artifact；
artifact 完成后生成 ticket、启动 specialized candidate，并轮询 exact ready ack；
ready ack 成立后只请求旧 Editor graceful exit；candidate 失败、超时、取消或 state query 失败时保留旧 Editor；
真实 Workspace/Process/Exit Adapter 已由 production RealNativeEditorApp 构造安装；
qualification authority 使用隔离构造，不安装 production services，不接触真实 trust store。
```

production identity 与 Gate G 的其余字段同构，均覆盖 Project/module/interface、AOT、Engine Cargo.lock、
toolchain、target/profile 与 normalized manifest/dependency/lock digest。两者的 `editorBuildIdentity` 有意绑定不同
authority：默认 production Editor 使用当前 executable SHA-256；Gate G 使用本轮 SDK source identity。因此 Gate G
generated artifact 的真实窗口资格与默认 Editor production handoff 资格必须分别留证，不能互相替代。

GameView 纯蓝首因也已修复：RuntimeRenderer 现在把 AUI Rect/Scrollbar 变换为受 clip 约束的真实 NDC quad，
解析 `#RRGGBB` / `#RRGGBBAA` 颜色，并让 glyph vertex 使用 AUI text color；空 `UiComposition` 不再生成
伪占位 quad。shared WGPU backend 在执行 plan 前注册 active RuntimePackage 的 FontBundle bitmap/MSDF pages，
缺失 texture binding 产生结构化 warning。qualification authority 在 shared texture present 后直接 readback exact
GameView texture，输出 `tower-defense-game-view.png`，并要求 `distinctColorCount >= 4` 且
`nonDominantPixelCount >= 1000`，防止非空窗口截图或纯 clear-color 再次误报通过。

production process 生命周期自审另修复两条窄路径：候选 `try_wait` 返回 Exited 后立即移除已回收的 `Child`
句柄；`candidate_state` 查询失败时 launcher best-effort terminate owned candidate，再清理 handoff 文件并保留旧
Editor。ready 后仍不终止新 Editor。测试夹具根追加原子序号，消除 Windows coarse clock 下并行 ticket 根碰撞。

当前定向验证：

```text
cargo test -p editor_core editor_project_composition_launcher_ -- --nocapture                         4/4 passed
cargo test -p editor_window_winit project_editor_handoff_ -- --nocapture                              2/2 passed
cargo test -p editor_window_winit project_runtime_trust_ -- --nocapture                               4/4 passed
cargo test -p editor_window_winit qualification_ -- --nocapture                                       2/2 passed
cargo check -p editor_window_winit --features real-window,real-wgpu-surface                            passed
cargo fmt --all -- --check                                                                             passed
```

上述源码变化已使旧 `sha256:8a8c...` Gate G source/artifact seal 失效。下一步必须在同一 run-owned root
重新执行完整四项目矩阵，取得新的 Tower artifact 后再做 exact GameView 像素与真实 production handoff 资格；
Gate H 仍施工中，不进入 Window 5。

2026-08-01 本轮 Gate G 重跑已完成。outer owner test `1/1` passed，四项目 qualification `4/4`
passed，全部 cache status 为 `rebuilt`；每项均记录 Windows Job Object ownership，且
`rootProcessWaitCompleted/outputReadersJoined/processGroupReleaseCompleted=true`。新身份与 seal：

```text
Editor source identity  sha256:8248169c6809c8cefaf0e6dcae08562070ad3e334b31292f662d9f9644497093
Complex Shooter         sha256:58918462dd7b26e4b1c925803a75a89a91015ca8bff5432199a092cda80b0c4e
Switch Puzzle           sha256:27ff12576610c9e759179054e4750fb9caa0615c7aef13768277f8e168c44195
Tower Defense           sha256:522d033bab04bbfad8b83ea6ae4da95edc43a702150a1fe6e6a2db21b769d83a
Tower executable        sha256:a8d5b1cc0d85d30593aa16631ca7710cf88dfbe872f9f4ecf322be5b5d61e2a1
repository-external     sha256:9a2827cb4fbc96d1075b47a56adf1fd41ebc9abdcbeb99f45142f19c06f43e04
```

权威报告：

```text
G:\Aife262Runs\window4-20260801-01\evidence\gate-g-report.json
```

随后使用新 Tower artifact 执行 1600x900、scale factor 1.25 的真实 Windows/WGPU Gate H。窗口、surface、
device 与 Vulkan shared context 创建成功，但 OS foreground 仍为 PID `11456` 的 Windows `LockApp.exe`；
目标 candidate PID 为 `12172`。authority 按合同返回：

```text
presentStatus=environment_blocked
authority_input.target_not_foreground
screenshotPath=null
gameViewCapturePath=null
input/runtime/lifecycle 未执行
```

阻断报告：

```text
G:\Aife262Runs\window4-20260801-01\evidence\gate-h\gate-h-real-window-report.json
```

这是环境资格阻断，不是像素 Gate 通过或产品失败；禁止在桌面未真正解锁时重复运行并误报。candidate 已退出，
无 Cargo/rustc/editor_host/composition 残留。production/debug `editor_host.exe` 前后 SHA256 均为
`1C4DBFF6402C0BE287F11E8B7B984CBD90CA6120E022FC0E71FD8923259BFF15`；没有替换 production/安装态
二进制，没有修改真实 trust store，没有运行 Local CI。Gate H 仍施工中；桌面解锁后先重跑新 Tower artifact
的 exact GameView 视觉资格，通过后才执行 run-owned 默认 Editor production handoff 资格，不进入 Window 5。

桌面解锁后的新 Tower artifact 真实视觉资格已通过。exact GameView capture 为 1280x720，
`distinctColorCount=2215`、`nonDominantPixelCount=540544`；人工复核确认已呈现多区块塔防 board、生命条、
粮草/部署/整军等真实中文 glyph，而非 clear color 或占位矩形。`td.recruit`、next-frame snapshot、
Pause/Step/Resume/Stop、Temporary Runtime Inspector 和 shared Vulkan texture present 全部通过。

下一步补齐并执行 run-owned production handoff ignored gate。该 gate 必须通过显式绝对路径 override 同时隔离
composition trust/build/ticket state 和 candidate recent-project/picker state；相对路径 fail closed，禁止
candidate 默认写入 `%LOCALAPPDATA%`。runner 使用独占 Tower 副本验证 data-only identity 稳定且不 handoff、
RuntimeModule 变化使 trust stale 并在批准后真实 rebuild/handoff、ready 后旧 Editor graceful exit、失败 candidate
保留旧 Editor，以及所有 candidate 终止并 join。

首次 production handoff runner 在启动候选前捕获 owner 实现缺口：`runtime_module_source_digest` 虽用于 native
trust identity，却错误地从 project root 递归哈希，导致 AUI data-only 变化令 trust stale。修复收窄 digest 根为
`RuntimeModule/`，并在 owner 测试中加入项目侧 AUI 变化保持 digest 稳定、RuntimeModule example 变化令 digest
变化的双向断言。该失败未启动真实 candidate，未消费外部 one-shot 资格。

第二次 runner 已通过 data-only/stale owner 断言，但在 candidate launch 前由 Windows `LNK1104` 阻断。
首因是 production installer 先追加 `project-editor-compositions`，artifact owner 又追加同名目录，造成重复路径并
放大 linker 输出路径。修复 installer 只把 `state_root` 传给深 artifact owner，由 owner 唯一追加 cache/staging
目录；后续 qualification 使用 run root 下更短的独占 handoff 根。该失败没有启动 candidate、没有 ready ack，
也没有修改真实外部状态。

最终 production handoff gate 已在短 run-owned root
`G:\Aife262Runs\window4-20260801-01\h\1785574445775` 通过，真实报告：

```text
G:\Aife262Runs\window4-20260801-01\h\1785574445775\production-handoff-report.json
```

结构化结果：

```text
dataOnlyTrustStatus=trusted
dataOnlyHandoff=false
aotTrustStatusBeforeApproval=stale
readyCompositionIdentity=sha256:8c8e2007ab75cdffce0be17b4972159f03dfaeaffce62653f0ab454d1bea1591
candidateProcessId=22628
oldEditorGracefulExitRequested=true
candidateTerminatedAndJoined=true
failedCandidateStatus=failed
failedCandidateKeptOldEditor=true
```

candidate 的 composition state、recent-project/picker、workspace layout 与 gateway discovery 均写入该 run-owned
根；无 Cargo/rustc/editor_host/composition/where 残留。production/debug `editor_host.exe` SHA256 仍为
`1C4DBFF6402C0BE287F11E8B7B984CBD90CA6120E022FC0E71FD8923259BFF15`。未修改真实 trust store、recent
projects、workspace layout、gateway installation/config，未替换 production/安装态二进制，未运行 Local CI。

本轮 trust digest、production build-root 和 isolation seam 修复改变了 Editor source identity，因此必须再续跑
Gate G 完整四项目矩阵，并用其新 Tower artifact 重跑 exact GameView 视觉资格，才能关闭 Window 4；不进入
Window 5。

上述续跑已完成，Window 4 以最新 source/artifact seal 正式关闭。最终 Gate G outer owner test `1/1`
passed，四项目 qualification `4/4` passed，`editorBuildIdentity` 为
`sha256:18a430f1df0737cc1fe8e9547a73c5e0b97ff3f6bbe0679dd0c8986809173c0d`。四项目最终 identity：

```text
Complex Shooter       sha256:6935b10298946e9cfc7d36f3dc0eacba64b128c8da96fef4f72e3f2da58eaea1
Switch Puzzle         sha256:b17f8179e63876d120835d88cdfb47d1ac8fd4c766920b9537b94cd1e192c46c
Tower Defense         sha256:4872d7389e3f5cb50759607dabef03cb1b1d6a16d693c7e703ced5210293fa50
repository-external   sha256:39c006b3a245518bc8c9b5b3c4de943cd96976a0bdcddbb5dca33244ceb7f943
Tower executable      sha256:04a9c19ca49f9e336c707f616ca4080b0535798118071feef39d50751da6d3c3
```

四项 process ownership 均为 `windows_job_object`，且
`processGroupCreated/rootProcessBound/rootProcessWaitCompleted/outputReadersJoined/processGroupReleaseCompleted=true`，
`terminationRequested=false`。最终矩阵报告：

```text
G:\Aife262Runs\window4-20260801-01\evidence\gate-g-report.json
```

最新 Tower artifact 的真实 Windows/WGPU 视觉资格也已通过并写回 canonical 报告：1600x900、scale factor
1.25、Vulkan、foreground verified、viewport texture registry `1`、exact GameView
`distinctColorCount=2215` / `nonDominantPixelCount=540544`。人工复核确认存在塔防分区、双生命条、粮草、部署、
整军与真实中文 glyph；`td.recruit` handled、next-frame snapshot changed、Pause/Step/Resume/Stop、Temporary
Runtime Inspector、active RuntimePackage 均通过。报告绑定 exact Tower composition identity
`sha256:4872d7389e3f5cb50759607dabef03cb1b1d6a16d693c7e703ced5210293fa50`，报告 SHA-256 为
`sha256:86ccb5c250873de31a99e629a87372f2c87d4232a7ca0891fc4dd230ec2f3722`。证据：

```text
G:\Aife262Runs\window4-20260801-01\evidence\gate-h\gate-h-real-window-report.json
G:\Aife262Runs\window4-20260801-01\evidence\gate-h\tower-defense-real-window.png
G:\Aife262Runs\window4-20260801-01\evidence\gate-h\tower-defense-game-view.png
G:\Aife262Runs\window4-20260801-01\evidence\gate-h\real-window-rerun.stderr.log
```

production handoff gate 继续由
`G:\Aife262Runs\window4-20260801-01\h\1785574445775\production-handoff-report.json` 提供已通过证据。
最终没有遗留 Cargo/rustc/editor_host/composition/where 进程；production/debug
`G:\gameEngin\rust\target\debug\editor_host.exe` SHA-256 仍为
`1C4DBFF6402C0BE287F11E8B7B984CBD90CA6120E022FC0E71FD8923259BFF15`。Tower Defense 原项目保持只读，
没有修改真实 trust store、recent projects、workspace layout 或 Gateway 配置，没有替换 production/安装态
二进制，也没有运行 Local CI。Window 5 未授权，停止于此。

### 17.4 Window 4 关闭

保存 source/artifact/project/ticket/ack/screenshot/report identities，清理严格属于 run root 的
build/temp/process state。需要保留的证据不得删除；无法清理时记录 exact path 和
`retained_by_host_policy`。等待 Window 5 授权。

## 18. Window 5 - Gate I：Affected Regression / Closure

### 18.1 Source inventory

```powershell
Set-Location G:\gameEngin
rg -n "tower.defense|complex-shooter|switch-puzzle" rust/crates/editor_core/src rust/crates/editor_window_winit/src -g "*.rs"
rg -n "complex_shooter_project_runtime|switch_puzzle_project_runtime|tower_defense_project_runtime" rust/crates/editor_window_winit/Cargo.toml rust/crates/editor_window_winit/src -g "*"
rg -n "fallback|explicit_empty" rust/crates/editor_core/src/project_editor_composition* rust/crates/editor_window_winit/src -g "*.rs"
```

允许：

```text
test fixtures/qualification names；
default Editor 的 explicit empty composition；
diagnostic 文本中对禁止 fallback 的断言。
```

阻断：

```text
production generated template/default set 中的项目名称或 moduleId；
regular dependency 的样例 RuntimeModule；
mismatch 后 fallback 到 empty；
PlayService 启动外部 Player 假装 GameView。
```

### 18.2 Affected regression

按顺序 fail-fast：

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --all -- --check
cargo test -p editor_core
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit
cargo test -p project_e2e_gate
cargo test -p engine_runtime project_runtime_ -- --nocapture
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
cargo check --workspace --all-targets --all-features
```

说明：

```text
不机械重复 Window 4 的真实 process matrix；若产品 source/fixture/runner identity 未变，引用其证据。
不运行 cargo test --workspace --all-features；本次共享 interface consumer 已由三个 owner suite、
project_e2e 和 workspace all-target/all-feature compile 覆盖。
不运行 Local CI；262 claim 不包含 canonical release candidate。
不替换 production/安装态二进制。
```

若实际修改扩展到 `editor_ui_model` / `editor_ui_renderer`，Gate I 必须在施工前把对应 crate 全量
test 加入本文并重新自审，不能只靠 `editor_window_winit` 间接覆盖。

2026-08-01 Window 5 激活复核确认 Window 3 的实际修改已扩展到上述两个 UI crate，因此已在任何
Gate I 回归执行前把 `cargo test -p editor_ui_model` 与 `cargo test -p editor_ui_renderer` 加入顺序清单。
重新自审通过：两项均是实际修改 owner 的全量测试，没有扩大到无关 crate，也没有与后续
`editor_window_winit` consumer、`project_e2e_gate` 或 workspace all-target/all-feature compile 重复声明同一 claim。

### 18.2.1 2026-08-01 Window 5 实际关闭记录

```text
状态：Gate I completed and closed；262 Gate A-I 全部完成
source baseline：acaffa8cb95bbb54d093bbf45835a59315c82da5
source identity：上述基线上的保留用户/前序施工 dirty worktree；未创建提交，不声明 clean exact-commit
elapsed time：约 1 小时 15 分钟（约 17:46-19:01，Asia/Shanghai），未触及 2:45/3:00 限制
外部状态：未修改真实 trust/recent/workspace/Gateway 状态；未替换 production/安装态二进制；未运行 Local CI
```

Source inventory 通过：样例项目名称只存在于 test/qualification/fixture/report 名称或无关既有
证据；`editor_window_winit` 没有样例 RuntimeModule regular dependency；default Editor 只有
`explicit_empty()`；composition mismatch 不 fallback 到 empty；PlayService 没有启动外部 Player
假装 GameView。

受影响回归最终结果：

```text
cargo test -p editor_core：612 passed；0 failed；2 ignored
cargo test -p editor_ui_model：56 passed
cargo test -p editor_ui_renderer：60 passed
cargo test -p editor_window_winit：150 passed；0 failed；1 ignored
cargo test -p project_e2e_gate：65 passed；0 failed；2 ignored；附属 integration 1+1+1+3 passed
cargo test -p engine_runtime project_runtime_ -- --nocapture：35 passed
cargo check -p editor_window_winit --features real-window,real-wgpu-surface：passed
cargo fmt --all -- --check：passed
cargo check --workspace --all-targets --all-features：passed
```

失败、分类与修复：

```text
editor_core 首轮 601 passed / 11 failed / 2 ignored：
  旧 Scene Save、Build Service 和 Preview tests 仍用 default explicit-empty EditorSession 打开
  ProjectRust；按 262 正式合同改为测试专用 singleton composition 注入。Player artifact 数据变化
  旧断言改为 RuntimeModule digest cache hit；multi-module Preview 改为验证 singleton fail-closed。
  LLM cancellation 的负载敏感墙钟断言改为同步取消语义断言。最终 612/0/2。

editor_window_winit 首轮 149 passed / 1 failed / 1 ignored：
  reachability test 同样使用 empty session 打开 ProjectRust；显式注入 test singleton 后最终 150/0/1。

project_e2e_gate 首轮 51 passed / 14 failed / 2 ignored：
  Complex Shooter/C-01 流程仍依赖旧 default sample composition，统一改为真实 sample helper 或
  identity-only singleton；隔离副本移除预存 preview cache，AUI stage 断言接受真实 >=1。
  最终 65/0/2，全部附属 integration binaries 通过。

首次 workspace all-target/all-feature compile：
  editor_input 的 GatewayAccessRequestModel test fixture 漏填 Window 3 新增的 operation_short_id 与
  risk_class，分类为受影响测试合同漂移；补齐字段后 owner test 1/1 与最终 workspace check 通过。

格式复核：
  修复期间产生 5 处 rustfmt 排版差异；执行 cargo fmt --all 后最终 --check 通过。
```

Window 4 真实 process/GPU evidence 未机械重跑；Gate I 的修复只改变 test/fixture 与不被 Window 4
资格消费的代码，继续复用以下已封存证据：

```text
run root：G:\Aife262Runs\window4-20260801-01
editorBuildIdentity：sha256:18a430f1df0737cc1fe8e9547a73c5e0b97ff3f6bbe0679dd0c8986809173c0d
Tower artifact：sha256:4872d7389e3f5cb50759607dabef03cb1b1d6a16d693c7e703ced5210293fa50
Tower executable：sha256:04a9c19ca49f9e336c707f616ca4080b0535798118071feef39d50751da6d3c3
Gate H report：G:\Aife262Runs\window4-20260801-01\evidence\gate-h\gate-h-real-window-report.json
production handoff：G:\Aife262Runs\window4-20260801-01\h\1785574445775\production-handoff-report.json
```

最终进程核对无 Cargo/rustc/editor_host/composition 残留。production/debug
`G:\gameEngin\rust\target\debug\editor_host.exe` SHA-256 仍为
`1C4DBFF6402C0BE287F11E8B7B984CBD90CA6120E022FC0E71FD8923259BFF15`。Tower Defense 原项目保持只读。

### 18.3 完成记录

全部 Gate 通过后生成：

```text
阶段完成记录/2026-07-31-Trusted-ProjectRust-Editor-Composition-Artifact-v1/00-总览.md
```

至少记录：

```text
实际 source identity 与 dirty/clean evidence 限定
每个窗口授权与 elapsed time
实际修改文件
测试命令、test count、结果
失败、分类、修复和重跑
复用的 Window 4 evidence identity
真实 artifact/hash/project/ticket/ack/screenshot
external state 与 cleanup outcome
未运行 Local CI/未替换 installed binary
remaining risks
```

### 18.4 文档闭环

```text
更新 262 正式方案状态为已完成施工。
更新 00-文档地图、00-阅读顺序、49、54。
更新 施工文档/README、待执行/README（如有队列说明）、阶段完成记录/README。
把本文从 当前/ 移到 已完成/，文件名改为
262-当前可自动化施工文档-Trusted-ProjectRust-Editor-Composition-Artifact-v1.md。
确认 当前/ 与 待执行/ 状态真实。
```

完成 262 不自动恢复塔防施工。返回 `tower-defense-project` skill，重新核对 P0-5 当前施工文档
Gate G 基线和授权后才可继续项目施工。

## 19. 预计涉及文件

### 19.1 Engine owner

```text
rust/crates/editor_core/src/project_runtime_trust.rs（新增或等价 owner）
rust/crates/editor_core/src/project_editor_composition.rs（新增深 module）
rust/crates/editor_core/src/project_editor_composition/*（仅内部实现过大时拆分）
rust/crates/editor_core/src/project_runtime_player_staging.rs（深化复用）
rust/crates/editor_core/src/project_player_artifact.rs（复用/窄适配）
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/editor_preview_package.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/Cargo.toml
```

### 19.2 Editor composition / process consumer

```text
rust/crates/editor_window_winit/src/linked_project_runtimes.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_window_winit/src/tests/*（owner/real-window gate）
rust/crates/editor_window_winit/Cargo.toml
```

### 19.3 Conditional UI files

只有现有 application model 无法表达窄型 trust prompt 时才加入：

```text
rust/crates/editor_ui_model/src/*
rust/crates/editor_ui_renderer/src/*
```

加入后必须先写回本文的具体 owner/interface/test，并重新自审；禁止临时散落 UI 状态。

Window 3 已按 14.1 启用该条件范围，实际 owner 限定为：

```text
rust/crates/editor_ui_model/src/project_runtime_trust.rs
rust/crates/editor_ui_model/src/model.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_renderer/src/project_runtime_trust.rs
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/application.rs
```

重新自审：通过。新增的是一个 prompt model、三个显式 decision command 和一个 retained
modal Adapter；trust receipt owner 仍是 `ProjectRuntimeTrustModule`，没有引入第二份 trust 真相，
没有改变 Gate E-F 边界或 Window 4 资格范围。

### 19.4 E2E / fixtures

```text
rust/crates/project_e2e_gate/src/project_editor_composition.rs（新增或等价）
rust/crates/project_e2e_gate/src/lib.rs
rust/fixtures/project_editor_composition/*（受控 source fixture）
samples/complex_shooter_project（只读 consumer，除非 test manifest contract 必需）
samples/switch_puzzle_project（只读 consumer，除非 test manifest contract 必需）
samples/tower_defense_project（只读 qualification consumer）
```

项目 sample 文件默认不得修改。若 test 必须新增通用 fixture，优先放 `rust/fixtures`，不污染真实
项目内容。

### 19.5 文档

```text
框架设计/引擎总体架构/262-Trusted-ProjectRust-Editor-Composition-Artifact-v1方案.md
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

列表外产品文件只有在 owner/consumer closure 明确需要时才能加入；先更新本文、说明原因、增加
对应 test，再重新自审。

## 20. 外部状态与证据规则

```text
Window 1-3 默认只用 test-owned temp/build roots，不启动真实 production Editor。
Window 4 Gate H 才可按明确授权启动真实 isolated candidate Editor/GPU window。
不修改真实用户 ProjectRuntime trust store；使用 repository policy 或 run-owned override。
不修改真实 recent projects、workspace layout、Codex config、Gateway installation。
不替换 production/安装态 editor_host.exe 或 MCP binary。
不运行 Local CI。
不删除 .artifacts 历史证据或其它角色目录。
```

真实 process Gate 必须记录所有 owned PID，timeout/异常时终止并 join owned descendants。递归清理
严格遵守 `game-engine-construction` 的 strict-child、reparse-point、evidence preservation 和
`retained_by_host_policy` 规则。

## 21. Gate 失败处理

```text
保留第一失败 assertion/diagnostic/stdout/stderr/identity。
先分类：方案、施工文档、实现、环境、旧基线。
使用当前 Gate owner test 复现；不以 workspace check 或 Local CI 作调试循环。
修复后只重跑当前 Gate 和被修复改变的 consumer evidence。
方案被推翻时先暂停施工并更新 262 正式方案。
新增依赖、UI owner、外部状态或项目修改时先更新本文并重新自审。
```

不得因为一次环境故障永久增加重复 suite；重复事故优先修 owner/report/runner。

## 22. 激活前复核

必须全部满足：

```text
262 正式方案仍为当前有效方案且未被替代。
施工文档/当前/ 没有非 README 的 active 施工计划；历史证据文件不算 active plan。
本文是待执行队列中唯一或正式优先级最高的计划。
260/261 完成状态和源码基线仍成立。
工作树重叠改动已逐文件分类，不需要 reset/revert。
Cargo package/filter 名称与当前代码匹配，所有定向 filter 预计 >0 tests。
Window 1 能在三小时内完成并保留测试/报告时间。
用户明确授权 262 Window 1。
```

复核通过后：

```text
1. 更新本文基线/命令（如有漂移）并重新自审。
2. 移动本文到 施工文档/当前/。
3. 同步 54、49 和施工文档 README：唯一 active=262 Window 1 Gate A。
4. 再开始红测试和代码修改。
```

## 23. 施工文档自审

### 23.1 是否与 262 正式方案一致

通过。

```text
Gate A：schema/identity/trust
Gate B：261-R1 staging 复用
Gate C：generated singleton artifact
Gate D：cache/invalidation/capacity
Gate E：trust UI/handoff/readiness
Gate F：Editor consumption/Play parity
Gate G：三项目 + workspace 外项目
Gate H：真实 Windows/Tower Defense
Gate I：受影响回归/闭环
```

没有引入 dylib、任意依赖、默认样例白名单或外部 Player 替代 GameView。

### 23.2 是否保持深 module

通过。

`ProjectEditorCompositionArtifact::prepare` 隐藏 staging/build/cache/seal；
`EditorProjectCompositionLauncher::handoff` 隐藏 process/ticket/ack/cleanup。caller 不需要掌握 Cargo
步骤或跨进程状态机，测试跨同一 interface。

### 23.3 是否保持项目/引擎边界

通过。

Engine 只出现通用 project/module/composition/trust identity。三个样例与外部项目只作 consumer；
塔防项目默认只读，禁止为 Gate 修改玩法。

### 23.4 Trust 是否只是形式检查

不是。

trust 在 build/launch 前 fail closed；receipt 不存项目目录，绑定源码/依赖/engine identity；
production Editor 有明确 Approve/Deny/Cancel 路径，stale 必须重新确认。

### 23.5 是否误用 Player artifact

否。

只复用 261-R1 staging/bounded build/identity 模式。generated Editor 是独立 executable，通过
Editor launch interface 注入静态 singleton set，不把 Player executable 动态加载。

### 23.6 是否解决默认样例静态依赖

是。

Gate C 明确要求 regular dependencies 移除 Complex Shooter/Switch Puzzle，默认 Editor 只保留
empty composition；样例只可作为 dev/test consumer。

### 23.7 Cache policy 是否完整

通过。

文档给出集中默认 soft/hard limit、LRU、active pin、项目隔离、tamper invalidation 和 bounded
cleanup；明确不管理用户 Cargo home。

### 23.8 Handoff 是否能失败恢复

通过。

ticket/ack exact identity、旧 Editor ack 前不退出、candidate 失败/超时保留旧进程、worker 不阻塞
event loop，并有真实 Windows Gate。

### 23.9 测试是否与 claim 匹配

通过。

owner red tests 覆盖 schema/trust/staging/build/cache/lifecycle；consumer tests 覆盖 Editor Play；
Gate G 防止只为塔防硬编码；Gate H 用真实 generated executable/window/GPU 证明用户可见链；Gate I
覆盖受影响 crate 和 all-feature compile。没有用 Local CI 代替 owner 证据，也没有机械重复真实矩阵。

### 23.10 三小时限制

通过。

五个授权窗口各自有可关闭的 Gate 和证据，不自动串行。每个窗口受 2:45/3:00 限制。

### 23.11 工作树风险

已识别并受控。

当前存在大量前序未提交改动；本文要求激活前建立 identity、逐文件合并、禁止整文件覆盖和
reset/revert。该风险不阻止生成待执行文档，但阻止未经复核直接施工。

### 23.12 是否需要修改 262 正式方案

不需要。

施工拆解没有扩大或推翻正式方案；cache 默认额度、default Editor 依赖收敛、trust UI 和 file-based
ticket 是方案允许范围内的施工细化。

### 23.13 当前能否施工

```text
结论：Window 1 可以施工
原因：激活前复核通过，本文已移动到 当前/，用户已授权 Gate A-B
限制：Window 1 关闭后停止，不自动进入 Window 2
```
