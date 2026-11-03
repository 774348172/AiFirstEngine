# 282 Project Editor Composition Cache Lifecycle + Build QoS v1 施工文档

## 1. 文档状态

```text
系统编号：282
正式方案：282-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1方案.md
选定方案：方案 C（C1 Build QoS + C2 Exact Cache Lifecycle）
文档状态：已完成；Window A / Gate A-D、Window B / Gate E-G 与 Gate H 断点续验均通过
当前可施工：否；等待归档到施工文档/已完成
激活状态：已激活（2026-08-11）
生成日期：2026-08-11
```

激活前复核已于 2026-08-11 通过。Window A / Gate A-D 与 Window B / Gate E-G 已完成并写入阶段记录。
Gate H 的 fresh candidate 冷编译曾在 production mutation 前安全停止；随后按用户授权改为复用仍有效的旧
qualified candidate、exact artifact、qualification seal 与 generated-lock lineage，仅续验 promotion、normal-hit
和普通 Editor smoke。断点续验已通过，完成事实见第 25.9 节与阶段记录 `08-Gate-H-断点续验完成.md`。

## 2. 施工目标

在不放宽 262 exact identity/trust/artifact seal、不重建 269 compilation compatibility cache 的前提下：

1. 让 composition cache miss 后的 Cargo/rustc 构建遵守有界 CPU/内存策略，Windows 使用
   below-normal priority；默认 policy 的上限为 `jobs=4`，实际值按逻辑处理器与构建时可用内存解析。
2. 将 composition build 接入已有 `runtime_cli` cancellable child process + Windows Job Object owner，
   Editor CloseRequested/Drop 时 cancel、终止 owned tree、回收 reader 并 join worker。
3. 移除 worker 存在期间的无界整窗重绘；progress elapsed 默认 10 Hz，真实 input/resize/
   GameView tick 仍立即 redraw。
4. 新增只对明确授权 maintenance caller 可达的 `promote_exact`，对 qualified run-owned artifact
   完成 containment、identity、descriptor、executable hash、qualification seal、destination copy 再校验、
   backup、atomic publish 与 rollback。
5. 用 typed v2 build report 和 v1 promotion report 证明 source kind、cache status、QoS decision、进程所有权、
   cancellation、各阶段耗时与 cleanup。
6. 在单独 production 授权后，完成新 production Editor + exact Tower composition cache +
   普通 Editor open/input/close smoke 的一致性闭环。

## 3. 当前代码基线

### 3.1 源码状态

```text
HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
worktree: 保留 260-281 及用户的大量未提交变更
282 source implementation: Window A / Gate A-D 与 Window B / Gate E-G 已完成；Window C / Gate H 未通过
```

与 282 直接相关的当前文件中，`application.rs`、`real_window.rs`、`runtime_cli/src/lib.rs`
为 modified，composition schema/artifact/production/qualification owner 为 untracked 保留施工基线。
这些都不是 282 可以 reset/revert/clean 的对象。激活前必须以当时字节重新核对，并在现有文件上增量施工。

### 3.2 已有能力，必须复用

```text
editor_core::ProjectEditorCompositionArtifact
  exact lookup / generated staging / Cargo build / descriptor query / executable hash
  cache publish / LRU / pin / cleanup / compilation compatibility target

runtime_cli::run_bounded_child_process_cancellable
  cancellation token / Windows Job Object / kill-on-close
  root wait / stdout-stderr reader join / ownership evidence

editor_window_winit::NativeEditorApplication
  background composition worker / project-open activity / handoff

editor_window_winit::ProjectEditorCompositionQualificationReport
  composition identity + linked AOT + headless/real-window qualification evidence
```

当前 qualification report 只绑定 composition identity，不绑定候选 executable hash。C2 必须先增加窄型
qualification seal，不得直接把现有 report 当作可提升证明。

### 3.3 当前缺口对应

```text
artifact cache miss                    -> unconditional release Cargo build
BoundedChildProcessRequest             -> no priority request/effective evidence
composition run_required_step          -> non-cancellable entrypoint
composition worker                     -> no cancellation token; Drop does not join it
application redraw_requested           -> worker.is_some() causes continuous redraw
real_window about_to_wait              -> only GameView deadline; no bounded build progress deadline
qualification                          -> no executable-hash-bound promotion proof
artifact owner                         -> no authorized exact promotion transaction
```

## 4. 施工窗口与授权边界

### 4.1 Window A / C1 / Gate A-D

范围：Build QoS schema/resolver、shared process priority evidence、composition cancellation、worker lifecycle、
redraw scheduler、受影响回归与 repository-external fresh root 冷构建响应性资格。

授权不包含：C2 promotion、真实用户 cache、production/installed 二进制、真实配置、
Local CI、完整视觉矩阵。

### 4.2 Window B / C2 source + disposable / Gate E-G

范围：qualification seal、promotion schema、deep owner、thin maintenance adapter、路径/哈希/回滚否定矩阵，
以及仅在 repository-external role-owned disposable roots 中执行的 promotion integration。

授权不包含：真实用户 cache、production/installed 二进制、普通 production Editor、真实配置、
Local CI。

### 4.3 Window C / production consistency / Gate H

Gate H 必须单独授权，且授权必须明确包含需要触发的外部状态：

```text
production Editor byte-exact backup + transactional replacement
real Tower exact composition cache backup + promote/warm-up
normal production Editor open/input/CloseRequested smoke
```

即使 Window A/B 已完成，也不自动获得 Gate H 权限。Gate H 不修改真实配置，不替换 Player/MCP/
其它 installed 二进制，不运行 Local CI。

## 5. 允许与禁止的文件范围

### 5.1 Window A 预计允许

```text
rust/crates/runtime_cli/Cargo.toml
rust/crates/runtime_cli/src/bounded_child_process.rs
rust/crates/runtime_cli/src/lib.rs
rust/crates/runtime_cli/tests/bounded_child_process.rs
rust/crates/editor_core/Cargo.toml
rust/crates/editor_core/src/project_editor_composition.rs
rust/crates/editor_core/src/project_editor_composition_artifact.rs
rust/crates/editor_core/src/editor_composition_build_qos.rs              (may add)
rust/crates/editor_core/src/lib.rs
rust/crates/editor_ui_model/src/launcher.rs
rust/crates/editor_ui_model/src/localization.rs                           (only new phase keys if owner)
rust/crates/editor_ui_model/resources/localization/en-US.editor.json
rust/crates/editor_ui_model/resources/localization/zh-CN.editor.json
rust/crates/editor_ui_renderer/src/panels/launcher.rs
rust/crates/editor_ui_renderer/src/tests.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/project_editor_composition_production.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/editor_window_winit/src/tests/project_editor_composition_production_gate.rs
rust/crates/editor_window_winit/src/tests/project_runtime_trust_prompt.rs
```

`runtime_cli` shared request 的既有直接 consumers（`controlled_source_patch`、`project_player_artifact`、
`build_service`、`exported_player_verification` 与 project E2E）属于 Gate D affected validation 面；除非编译或
行为合同要求，不为 282 机械修改这些文件。

### 5.2 Window B 预计允许

```text
rust/crates/editor_core/Cargo.toml                                        (only thin bin declaration if required)
rust/crates/editor_core/src/project_editor_composition.rs
rust/crates/editor_core/src/project_editor_composition_artifact.rs
rust/crates/editor_core/src/project_editor_composition_cache_promotion.rs (may add)
rust/crates/editor_core/src/bin/project_editor_composition_cache_admin.rs (may add)
rust/crates/editor_core/src/lib.rs
rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs
rust/crates/editor_window_winit/src/tests/project_editor_composition_production_gate.rs
```

### 5.3 Gate H 外部状态

必须在激活时解析并记录精确路径，不在本待执行文档中猜测：

```text
repository-external role root: C:\AifeProductionRuns\282-<timestamp>
production Editor executable: resolve and hash at Gate H preflight
real composition cache root: resolve from production owner, no guessed path
Tower exact cache entry: derive from expected identity, no wildcard
backup roots: strict children of role root
```

2026-08-11 Window C 激活时解析的唯一授权目标：

```text
repository-external role root:
  C:\AifeProductionRuns\282-c3-20260811-225420
production Editor target:
  G:\gameEngin\rust\target\debug\editor_host.exe
real composition owner root:
  C:\Users\zenghaoran\AppData\Local\AI First Engine\Editor
real composition cache owner:
  C:\Users\zenghaoran\AppData\Local\AI First Engine\Editor\project-editor-compositions
Tower exact target:
  只允许新 production candidate hash 派生的单个 cache/<exact-key>；在任何 production mutation 前写入 preflight evidence
binary/cache backup root:
  C:\AifeProductionRuns\282-c3-20260811-225420\backup
read-only config baseline:
  C:\Users\zenghaoran\.codex\config.toml
  C:\Users\zenghaoran\AppData\Local\AI First Engine\Editor\project-runtime-trust
```

激活时 production Editor SHA-256 为
`F17BEDB2A3892486DED074DE1E4FA1AEF9D4D14CC37E51E35B4155BEBD23C786`；owned Cargo/rustc/
Editor process inventory 为空。Gate G source descriptor/build report/qualification report/seal SHA-256 依次为
`4731535B...CA9D`、`DBA2CDCA...8CB`、`38C9BE0C...6F5E`、`066FA26B...2B9`，文件均仍存在。

### 5.4 全程禁止

```text
samples/tower_defense_project gameplay/AUI/assets/RuntimeModule/config
rust/crates/engine_runtime/**
rust/crates/ai_tool_gateway/**
ProjectPatch / Gateway public tool additions
generic build daemon / workflow engine / remote cache
Cargo global config or project .cargo/config.toml
stable Editor ABI/capability digest redesign
Local CI unless later separately authorized
workspace reset/revert/clean
unlisted production/installed binary replacement
real user configuration mutation
```

如果实现发现 shared bounded process owner 必须触及第 5.1 节外的 runtime_cli 直接 consumer，先更新本文
文件清单并重新自审，不在编译失败后临时扩大。

## 6. 固定 Contract

### 6.1 保持不变的 v1

```text
project-editor-composition-identity.v1
project-editor-composition-artifact.v1
project-editor-composition-descriptor.v1
project-editor-composition-handoff-ticket.v1
project-editor-composition-launch-receipt.v1
```

artifact cache key 仍由完整 exact identity 生成；269 compatibility digest 仍只选择 Cargo target root。

### 6.2 C1 schema

```text
project-editor-composition-build-request.v2
project-editor-composition-build-qos-policy.v1
project-editor-composition-build-report.v2
```

Rust types 默认 `deny_unknown_fields`。typed contract 至少包含：

```text
ProjectEditorCompositionBuildQosPolicy
ProjectEditorCompositionSystemFacts
ProjectEditorCompositionBuildQosDecision
ProjectEditorCompositionBuildSourceKind::{ExactCache, ExactPromotion, ControlledBuild}
ProjectEditorCompositionCacheStatus::{Hit, Promoted, Miss, Invalidated, Rebuilt}
ProjectEditorCompositionProcessPriority::{Normal, BelowNormal}
ProjectEditorCompositionBuildReportV2
```

build report v2 必须包含 actual jobs、requested/effective priority、policy decision、每步 elapsed、
cancellation requested、process ownership/cleanup、worker joined、redraw Hz、source kind 和 typed diagnostics。
历史 build report v1 保持只读解析，不追溯重写旧 cache。

### 6.3 shared process interface

只深化现有 `BoundedChildProcessRequest` / result：

```text
requested priority
effective priority / priority applied evidence
```

Windows production Adapter 应在 suspended root child 绑定 Job Object 的既有顺序中施加 below-normal priority，
并证明子进程继承。不新建第二个 Job Object/process tree implementation。

### 6.4 C2 schema

```text
project-editor-composition-qualification-seal.v1
project-editor-composition-promotion-request.v1
project-editor-composition-promotion-report.v1
```

qualification seal 必须绑定：

```text
passed qualification report digest
qualification producer/schema
composition identity digest
actual candidate executable SHA-256
artifact descriptor SHA-256
createdAt / operationId
```

promotion request 必须显式给出 authorized source run root、source artifact root、destination cache root、
expected identity、qualification seal/ref、operation id 和 backup root。不允许“自动找最新”、glob 或环境扫描。

### 6.5 deep Module interface

```text
ProjectEditorCompositionArtifact::prepare(request, control) -> BuildReportV2
ProjectEditorCompositionArtifact::promote_exact(request)    -> PromotionReportV1
```

process-launch 内部 seam：

```text
EditorCompositionBuildQoSModule::execute(plan, policy, cancellation) -> BuildExecutionReport
SystemCompositionBuildProcessAdapter
ScriptedCompositionBuildProcessAdapter
```

CLI 若需要，只能是 typed request file -> `promote_exact` -> report file 的薄 maintenance Adapter。
不得引入任意 shell、搜索 run root、替换 production 二进制或修改配置的功能。

## 7. Gate A - QoS Schema / Resolver / Report Migration

### 7.1 任务

1. 在 composition schema owner 增加 BuildRequest v2、QoS policy/system facts/decision 和 BuildReport v2。
2. 实现纯函数 jobs resolver：CPU cap、memory cap、maxJobs 取最小，下限 1；系统 facts 不可用时
   保守 fallback，禁止回到 CPU-wide default。
3. 固定默认 policy：`maxJobs=4`、`reservedLogicalProcessors=4`、`reservedMemory=3 GiB`、
   `estimatedMemoryPerJob=1 GiB`、`minJobs=1`。
4. 只读解析 build report v1，writer 只写 v2；不改 identity/artifact/descriptor schema。
5. 建立稳定 diagnostics：invalid policy、system facts unavailable、priority failure、cancel/cleanup failure。

### 7.2 Red-capable owner tests

激活后先写失败测试，再实现：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_qos_ -- --nocapture
cargo test -p editor_core project_editor_composition_report_v2_ -- --nocapture
```

矩阵至少覆盖：

```text
logical CPUs: 1 / 2 / 4 / 8 / 12 / 32
available memory: unavailable / below reserve / 4 GiB / 7 GiB / high
invalid zero / inverted policy
12 logical + 7 GiB -> jobs=4
facts unavailable -> conservative non-zero jobs, never CPU-wide
BuildReport v2 roundtrip / unknown field rejection
BuildReport v1 read compatibility / no historical rewrite
```

### 7.3 通过条件

- resolver 字节稳定、无 I/O、无平台分支；
- identity/artifact/descriptor v1 序列化字节不变；
- build report 能回答 policy input 与实际 decision；
- 没有项目/全局 Cargo config 写入。

## 8. Gate B - Process Priority / Composition Cancellation

### 8.1 任务

1. 在 shared bounded child process request/result 增加 typed priority request/evidence，保持现有 caller 默认 Normal。
2. Windows 上对 Cargo root 使用 below-normal priority，以真实 process priority query 证明生效。
3. composition `generate-lockfile`、`build`、descriptor query 全部走可取消路径，共享同一 operation token。
4. `build_composition_release` 只增加 `--jobs <resolved>`；保留 `--release --locked --offline`。
5. cancel/timeout/wait failure 必须复用现有 Job Object ownership evidence；cleanup 未证明时不删正在写入的 staging/target。
6. `EditorCompositionBuildQoSModule` 只通过 System/Scripted Adapter 执行计划，不向 UI 暴露命令细节。

### 8.2 Red-capable owner tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p runtime_cli bounded_child_process_priority_ -- --nocapture
cargo test -p runtime_cli bounded_child_process_cancellation_ -- --nocapture
cargo test -p editor_core project_editor_composition_build_qos_ -- --nocapture
```

测试必须断言：

- Windows helper 实际观察 below-normal，非 Windows 如实返回 best-effort/unsupported；
- Cargo command 包含唯一 `--jobs <resolvedJobs>`，仍包含 release/locked/offline；
- cancel 后 exit reason 为 cancelled，termination requested、root wait、group release、reader join 全部成立；
- 进程失败保留 stdout/stderr/ownership 事实；
- Scripted Adapter 不启动真实 process。

### 8.3 通过条件

- 没有第二套 Job Object/kill tree 实现；
- 无取消的旧 composition process path 不再可达；
- 优先级失败不伪报 applied，并按 policy fail closed 或 jobs=1 保守继续；
- shared request 的其它 consumer 继续默认 Normal。

## 9. Gate C - Worker Lifecycle / Progress / Redraw

### 9.1 任务

1. `ProjectEditorCompositionPreparationAdapter::prepare` 接收 cancellation 与有界 progress sink。
2. composition worker 持有 cancellation token、startedAt、activity revision 和 join handle；duplicate prepare 继续 exactly-once。
3. application Drop/CloseRequested 对 composition worker 执行 cancel + join，与 278 terminal finalization 顺序协同。
4. `ProjectOpenActivityPhase` 补足 CacheLookup/Promoting/Warming/Sealing/Cancelled 等必需 typed phase，更新本地化与 renderer tests。
5. 从 `redraw_requested` 移除 `project_editor_composition_build_worker.is_some()` 无条件分支。
6. 增加 progress deadline：默认 100 ms，只更新 elapsed 时不快于 10 Hz；input/resize/GameView 仍立即。
7. `about_to_wait` 选择 GameView tick 和 progress tick 中最早 deadline，无活动工作时保持 Wait。

### 9.2 Red-capable owner/consumer tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit dropping_application_cancels_and_joins_project_editor_composition_worker -- --nocapture
cargo test -p editor_window_winit project_editor_composition_redraw_ -- --nocapture
cargo test -p editor_window_winit project_editor_composition_progress_ -- --nocapture
cargo test -p editor_ui_model project_open_activity_ -- --nocapture
cargo test -p editor_ui_renderer project_open_activity_ -- --nocapture
```

至少覆盖：

```text
worker exists + no state/deadline -> no redraw
progress deadline reached -> one redraw and next deadline advances
pointer/resize while throttled -> immediate redraw
GameView deadline earlier/later than progress deadline -> earliest WaitUntil
Drop/CloseRequested -> cancellation observed, worker joined
duplicate prepare -> one worker / one Cargo plan
cancelled -> one terminal report, no handoff
```

### 9.3 通过条件

- 关闭窗口不等待构建自然超时；
- progress throttle 不吞掉输入事件；
- UI 只显示短状态，完整技术证据留在 Summary/Trace report；
- cancellation 不生成 handoff ticket。

## 10. Gate D - C1 Affected Integration + Fresh Windows Responsiveness

### 10.1 代码冻结前快速反馈

Gate A-C 修复完后，执行一次受影响 closure。完整 crate suite 在同一配置下覆盖前述 filtered tests，
因此不在 suite 后重复运行全部 filter。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p runtime_cli
cargo test -p editor_core
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit
cargo check -p editor_window_winit --features real-window
cargo fmt --all -- --check
```

### 10.2 repository-external fresh run

只有 Window A 授权明确包含 fresh Windows responsiveness qualification 时执行。

```text
role root: C:\AifeProductionRuns\282-c1-<timestamp>
project: run-owned Tower source copy or read-only source + run-owned staging
composition state/cache: role-owned only
binary: source-built candidate only
production/installed state: untouched
real config: untouched
```

资格步骤：

1. 制造 exact artifact miss，观察 controlled release build 而非预热 hit。
2. report 证明 `sourceKind=controlled_build`、`resolvedJobs<=4`、below-normal effective、
   release/locked/offline 不变。
3. 构建期间采集 Windows Responding、UI message latency、drag/resize 和 CPU/memory samples。
4. 目标：UI message P95 `< 50 ms`，resize P95 `< 100 ms`，窗口全程 Responding。
5. 单独冷构建中发出 CloseRequested，证明 Cargo/rustc/linker/composition 无遗留进程。
6. 保留报告、进程采样与首个失败证据；清理只能按 strict-child 规则处理 role-owned disposable 目录。

### 10.3 Gate D 通过条件

- owner/consumer suites 通过；
- real-window feature 可编译；
- 真实 cold build 满足响应性和进程清理目标；
- 没有写真实用户 cache，没有替换 production/installed 二进制；
- C1 完成不自动启动 C2。

## 11. Gate E - Qualification Seal / Promotion Schema

### 11.1 任务

1. 建立 `qualification-seal.v1`，由已有 headless/real-window qualification producer 在 passed 后生成。
2. seal producer 必须重算 actual candidate executable hash 和 descriptor hash，并绑定 report digest。
3. 建立 promotion request/report v1、typed status、stage、backup/rollback/cleanup 事实和诊断。
4. request 拒绝空路径、非 SHA-256、未知 schema/field、source=destination、backup 不在 role root 等输入。
5. promotion schema 不包含 production binary replacement、config mutation、run-root search 或 fallback shell command。

### 11.2 Red-capable owner tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_promotion_schema_ -- --nocapture
cargo test -p editor_window_winit project_editor_composition_qualification_seal_ -- --nocapture
```

矩阵包含 seal/report/executable/descriptor 任一 digest 变化必须失败，以及 historical qualification
report 没有 seal 时不得提升。

### 11.3 通过条件

- qualification 与 executable 建立字节级绑定；
- schema 不扩大普通 Editor/Gateway 权限；
- 历史 report 可读但不伪装为 sealed qualification。

## 12. Gate F - Exact Promotion Transaction / Thin Adapter

### 12.1 任务

1. 在既有 artifact/cache owner 实现 `promote_exact`，内部复用 normal cache loader/publish/hash/path helper。
2. source artifact/root canonicalize 并验证严格位于 authorized run root；拒绝 symlink/junction/reparse escape。
3. source 验证 exact identity、descriptor、executable hash、build success 和 qualification seal。
4. 复制到 destination-owned staging 后重读 descriptor，重算 executable/report/seal hash。
5. destination 已有有效 exact artifact 时 no-op hit；无效/冲突目标先移到 operation backup。
6. atomic publish 后必须通过 normal cache loader 重新读取；不相信内存 candidate object。
7. 任一失败恢复旧 destination；无法安全清理时报告 retained path/reason。
8. 如需 thin bin，它只读 typed request、调用 owner、原子写 typed report，不实现任何内部步骤。

### 12.2 Red-capable owner tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_promotion_ -- --nocapture
cargo test -p editor_core project_editor_composition_cache_admin_ -- --nocapture
```

必须覆盖：

```text
exact happy path / destination no-op hit
identity mismatch / executable tamper / descriptor tamper / report tamper / seal tamper
source outside root / source reparse escape / destination reparse escape
copy-after-verify mutation
pre-existing invalid destination backup
publish failure injection -> byte-exact rollback
rollback failure -> terminal failed + retained evidence
different project exact artifact rejection
no implicit controlled build fallback
```

### 12.3 通过条件

- project 不能读写其它项目 final artifact；
- promotion 不产生/修改 trust receipt；
- owner 不依赖 Editor UI 或 production updater；
- CLI 不接受 glob、search-latest、shell 或 binary replacement 参数。

## 13. Gate G - Disposable C2 Integration / Affected Regression

### 13.1 disposable integration

使用新 repository-external role root，source/destination/backup 三个 mutable domain 严格分离：

```text
C:\AifeProductionRuns\282-c2-<timestamp>\source-qualified
C:\AifeProductionRuns\282-c2-<timestamp>\destination-cache
C:\AifeProductionRuns\282-c2-<timestamp>\backup
C:\AifeProductionRuns\282-c2-<timestamp>\evidence
```

执行一次 qualified exact promotion、一次 no-op repeat，以及 identity/hash/path/publish rollback 否定矩阵。
正常 `ProjectEditorCompositionArtifact::prepare` 指向 disposable destination 时必须返回
`sourceKind=exact_cache` 且不启动 Cargo/rustc。

### 13.2 affected suite

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core
cargo test -p editor_window_winit
cargo check -p editor_window_winit --features real-window
cargo fmt --all -- --check
```

Window A 已完成且 C2 没有修改 runtime_cli/QoS/UI 时，复用 Gate D 的有效证据，不机械重跑
`runtime_cli` / `editor_ui_model` / `editor_ui_renderer` 全量 suite。如 C2 实际修改了它们，则更新本文影响面并运行相应 consumer。

### 13.3 通过条件

- disposable promotion 和否定矩阵全部有 typed report；
- source/destination/backup identities 明确；
- 失败先保留证据，清理只处理已验证 strict-child 目标；
- 真实用户 cache、production/installed 二进制和真实配置保持字节不变；
- Gate G 不自动进入 Gate H。

## 14. Gate H - Separately Authorized Production Consistency Closure

### 14.1 激活前外部状态 preflight

必须记录：

```text
source identity and relevant dirty baseline
candidate Editor hash
current production Editor path/hash
exact Tower expected composition identity
qualified source artifact root + seal/report digests
real destination cache root + target exact entry
binary/cache backup paths
real configuration digest before operation
owned-process inventory
```

没有 exact qualified candidate 时，可以选择在真实 cache root 用 C1 controlled warm-up，但该分支必须在 Gate H
授权中明确，不得在 promotion failure 后暗中触发。

### 14.2 事务顺序

1. 在 fresh role root 构建 282 production Editor candidate，不直接覆盖 production path。
2. 验证 candidate 可启动且 schema/owner tests 已绑定相同源码。
3. byte-exact 备份旧 production Editor，事务替换并 re-hash。
4. 依新 production Editor hash 派生 Tower expected exact composition identity。
5. 对现有目标 exact entry 做 byte-exact backup；只处理该项，不重建/清理其它 cache。
6. 若存在 exact qualified artifact，执行 `promote_exact`；否则按已授权的 controlled warm-up。
7. 用 normal cache loader 确认 exact hit，且未启动新 Cargo/rustc。
8. 启动普通 production Editor，打开 Tower，做 open/drag/resize 与一次真实 input smoke。
9. 如需验证 CloseRequested build cancel，使用独立 disposable exact miss 运行，不破坏已提升的正常 cache。
10. 保留 Editor 供用户查看只有在授权明确要求时执行；否则正常关闭并确认无 owned descendants。

### 14.3 回滚

任一 production 步骤失败：

```text
stop further mutation
preserve first failure evidence
terminate owned processes
restore cache target from byte-exact backup when changed
restore production Editor from byte-exact backup when changed
re-hash restored targets
prove real config digest unchanged
report rollback status and retained paths
```

回滚失败为 terminal failed，不得继续启动 Editor 或把当前状态标记为 consistent。

### 14.4 Gate H 通过条件

- production Editor 已绑定 282 实现源码并有 old/new/backup hash；
- Tower expected exact artifact 在真实 cache 中由 normal loader hit；
- 普通 Editor 打开、拖动、resize 和 input smoke 通过；
- 不存在遗留 Cargo/rustc/composition/editor owned process；
- 真实配置 digest 不变；Player/MCP/其它 installed 二进制不变；
- 未运行 Local CI，除非 Gate H 授权另行明确包含。

## 15. 失败处理与停止条件

1. 每个 Gate 保留首个 assertion/diagnostic、request/report、stdout/stderr、process ownership 和 environment facts。
2. 先使用当前 Gate 的 owner reproduction 修复，不用完整 crate suite、真实 production 或 Local CI 做调试循环。
3. 修改 shared bounded process interface 时，覆盖实际改动的 consumer；不因共享 workspace 自动跑无关 crate。
4. 需要放宽 exact identity/hash/trust 才能通过时，立即停止并回填正式方案。
5. 需要搜索未授权 run root、修改真实配置、引入 generic daemon/workflow 或 stable ABI redesign 时停止。
6. cancel 后 owned process tree 未终止、reader/worker 未 join，或 cache rollback 无法完成时，当前 Gate 失败。
7. 任一新发现的重叠用户改动优先保全；无法增量施工时暂停请求决策，禁止 reset/revert。
8. 每个连续施工窗口受三小时限制；2h45m 后不启动新 Gate，3h 停止并记录
   `stopped_by_time_limit`。

## 16. 外部状态与清理

### 16.1 Window A/B

只允许使用该窗口自己的 repository-external role root。每个并行/串行角色拥有独立 cache、target、temp、
evidence 和 process namespace。不得共用 mutable cache root 做并发测试。

清理前必须：

1. 保留所有必要报告与首个失败证据；
2. 确认所有 owned descendants 已退出；
3. 将目标解析为 role root 的严格子路径，检查目标及所有祖先无 reparse escape；
4. 只删除明确列名的 disposable child，不删 role root、evidence、workspace、shared target 或用户 cache。

失败按 `removed / missing / retained_by_host_policy` 记录，不因 cleanup 失败重跑已有效资格。

### 16.2 Window C

真实 production binary/cache 只做事务备份与必要替换，不做递归清理。backup 作为 Gate H 证据保留，
不在完成时自动删除。

## 17. 激活前复核清单

激活 Window A 前必须重新完成：

1. 用户明确授权激活 `282 Window A / Gate A-D`。
2. `施工文档/当前/` 没有另一份活动 `.md` 施工文档。
3. 重读 282 正式方案，exact/trust/compatibility 不变量仍有效。
4. 重算第 3 节相关文件字节/hash，分类文档生成后漂移与既有保留变更。
5. 确认 shared `runtime_cli` cancellable/Job Object owner 仍存在，没有新的 priority seam 已由其它施工完成。
6. 确认本文测试 filter 名与当时 module/test layout 可执行；如需修订，先更新本文并重新自审。
7. 确认 Window A 的实现、affected suite、fresh cold build、evidence 和 cleanup 能在三小时内完成。
8. 激活时只移动本文到 `当前/`，同步 `54` 和施工 README，不开始 Gate A，除非用户同一授权明确要求开始。

Window B 激活前还必须确认 Window A / Gate A-D 已有完整通过证据。
Window C / Gate H 激活前还必须确认 Window B / Gate E-G 通过，并逐项列出获授权的 production/cache 目标。

## 18. 完成后文档同步与归档

只有 Gate A-H 全部通过，且 Gate H production/cache/config 恢复事实已证明后：

1. 生成 `阶段完成记录/2026-??-??-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/00-总览.md`。
2. 记录 source identity、relevant dirty baseline、test harness、fresh run roots、binary/cache/config identities、
   实际运行测试、复用证据、未执行项、备份/回滚与 retained state。
3. 更新 282 正式方案状态为已施工，不改 deferred stable ABI/capability digest 结论。
4. 更新 `49`、`54`、`00-文档地图`、`00-阅读顺序`、`施工文档/README.md`、
   `施工文档/待执行/README.md` 和 `阶段完成记录/README.md`。
5. 将本文状态改为已完成，移到 `施工文档/已完成/`。
6. 确认 `当前/` 施工槽释放，不自动启动 stable Editor ABI digest、Tower 功能、Local CI 或其它 Gate。

如 Gate H 未获授权或未通过，Window A/B 可写阶段证据，但 282 不得整体归档或声明 production 闭环完成。

## 19. Window A / Gate A-D 施工结果

```text
状态：完成
日期：2026-08-11
source identity：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留的 dirty worktree
fresh role root：C:\AifeProductionRuns\282-c1-20260811-174607
阶段记录：阶段完成记录/2026-08-11-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/00-Window-A.md
后续授权：无；Window B / Gate E-G 与 Window C / Gate H 均未授权
```

Gate A-C 已完成 Build Request/Report v2、v1 只读兼容、typed QoS policy/system facts/decision、
Windows 可用内存采集、shared bounded process priority/cancel、composition 全步骤统一 owner、application
Drop cancel/join、bounded progress 与 10 Hz redraw deadline。默认 policy 保持 `maxJobs=4`；本次 cold build
测得可用内存约 6.72 GiB，扣除 3 GiB reserve 后由 memory cap 正确解析为 `resolvedJobs=3`，不得强制或
伪报为 4。

Gate D 使用唯一 fresh role root 完成 cold controlled build、real generated Editor handoff、真实 Windows
响应性与 compiling 阶段 cancellation。cold report 为 `sourceKind=controlled_build`、`cacheStatus=rebuilt`、
below-normal applied、`--release --locked --offline --jobs 3`；真实响应性 60 次 message 无 timeout，message
P95 `0.4599 ms`、resize P95 `22.5584 ms`，标题栏 drag 后仍 Responding。cancellation 在 70 ms 内完成，
failure report 为 `project_editor_composition.cancelled`、`cancellationRequested=true`、
`processTreeTerminated=true`；外层 application worker 与 candidate 均已 join，run-owned 进程无遗留。

affected suites、real-window check、fmt 与 scoped diff check 通过。`editor_core` 首次完整回归唯一失败是新增
qualification test harness 未 rustfmt，修正该排版后失败项定向重跑通过；其余 662 passed / 2 ignored
证据未失效。普通 folder picker 自动化失败只记录为 `project_open.manifest_unreadable` 环境/自动化路径问题，
不计入 Gate D 成功证据。未运行 Local CI，未修改 production/installed 二进制、真实配置、真实用户 cache
或 Tower 项目源内容。

## 20. 施工文档自审

### 20.1 与正式方案是否一致

通过。Gate A-D 对应 C1，Gate E-G 对应 C2 source/disposable，Gate H 对应单独授权的 production
consistency closure。Window B 中的真实 production 操作被进一步拆为 Window C，是授权与三小时风险收窄，
没有改变方案行为。

### 20.2 是否放宽 identity / trust / cache

没有。identity/artifact/descriptor/handoff v1 保持不变；compatibility digest 只复用 Cargo intermediate；
promotion 要求 exact identity 和 executable-bound qualification seal；trust receipt 仍由现有 owner 决定。

### 20.3 是否复制已有 process/cache owner

没有。C1 复用 `runtime_cli` cancellable + Windows Job Object，只补 priority/QoS evidence；C2 复用
`ProjectEditorCompositionArtifact` normal loader/hash/publish/path helper，不新建 generic cache 或第二套 builder。

### 20.4 Module 是否足够深

通过。normal caller 只学习 `prepare`，授权 maintenance caller 只学习 `promote_exact`。jobs/priority/
process tree/redraw 以及 containment/copy/hash/backup/publish/rollback 都隐藏在 owner implementation 中。

### 20.5 测试是否与 claim 匹配

通过。Gate A-C/E-F 用 red-capable owner tests；Gate D/G 用受影响 crate closure；真实 Windows cold build、
input/resize/CloseRequested 只证明平台与交互响应性；Gate H 只证明 production/cache 组合。
没有用 Local CI 或完整视觉矩阵代替 owner reproduction。

### 20.6 是否机械重复验证

没有。同配置的 full crate suite subsume 前述 filter；Window B 不修改 C1 consumer 时复用 Gate D 证据；
fresh/production 只因为 environment/binary/cache identity 不同而保留为独立证据。

### 20.7 是否保护外部状态

通过。Window A/B 只用 role-owned roots；Window C 需单独点名 binary/cache 授权、byte-exact backup、
transactional replace/publish、rollback 与 config digest 不变证明。没有提供宽泛递归删除权限。

### 20.8 是否保护现有用户改动

通过。文档明确记录 dirty/untracked 保留基线，激活前以文件字节而非 clean HEAD 假设复核，
禁止 reset/revert/clean。

### 20.9 是否覆盖每个可执行需求

通过。QoS 决策、jobs/priority、cancel/process ownership、redraw deadline、v1/v2 migration、qualification seal、
containment、双重哈希、backup/publish/rollback、normal loader hit、真实响应性与 production consistency 均有明确 Gate 与通过条件。

### 20.10 是否需要修订正式方案

不需要。代码基线已存在 shared cancellable/Job Object owner，这使实现更小，但不改变正式方案的
QoS/cancel 结果。将 production Gate H 单独拆窗只收窄授权和时间风险。

### 20.11 其它 AI 审查

本次没有与 282 相匹配的其它 AI 审查输入。已读取项目规则、施工规则、262/268/269/274/278
完成基线与当前 owner code，没有必须修改正式方案的冲突。

### 20.12 自审结论

```text
施工文档自审：通过
正式方案需修订：否
当前文档位置：施工文档/当前/
当前状态：Window B / Gate E-G 已完成；Window C / Gate H 未激活
当前施工/测试授权：无
下一步：等待单独授权 Window C / Gate H；不得自动进入
```

## 21. Window B / Gate E-G 施工结果

```text
状态：完成
日期：2026-08-11
fresh role root：C:\AifeProductionRuns\282-c2-20260811-204604
阶段记录：阶段完成记录/2026-08-11-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/01-Window-B.md
后续授权：无；Window C / Gate H 仍未授权
```

Gate E-F 已完成 executable-bound qualification seal、typed promotion schema、exact owner、thin admin、
containment/reparse/hash/copy/backup/publish/rollback/retained evidence 矩阵。Gate G 用当前 HEAD 真实 generated
Editor candidate 完成 qualification、exact promotion、no-op 与 normal `prepare` exact cache hit；normal prepare
`steps=0`，没有启动 Cargo/rustc fallback。fresh root 保留全部 typed evidence。

affected regression 最终为 `editor_core` 671 passed / 2 intentional ignored、`editor_window_winit` 191 passed /
3 intentional ignored；real-window check、fmt 与 scoped diff check 通过。未运行 Local CI，production Editor
hash 保持 `F17BEDB2...C786`，未修改 production/installed binary、真实配置、真实用户 cache 或真实 Tower。
282 不归档，不自动进入 Gate H。

## 22. 282-R1 Repair 激活记录与唯一执行计划

```text
激活日期：2026-08-12
用户授权：Repair Window A / Gate R-A 至 R-D
正式方案：282-R1-Generated-Lock-Lineage-Path-Affine-Compilation-Cache-Tiered-Build-Deadline-v1方案.md（方案 B）
激活前复核：通过
源码基线：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留的 dirty worktree
当前授权：只允许 Window A / R-A 至 R-D
未授权：Window B / R-E 至 R-G、Gate H、Local CI、fresh root、production/installed binary、真实 cache/config
独立计划：已移入施工文档/历史/，只作合入证据，不再是执行入口
```

R1 修复不建立第二套 composition builder。requested identity、trust、qualification/promotion authority
继续保持；新增 resolved identity 只加强 exactness。raw generated lock bytes 用于 seal，canonical resolved
graph 用于 compatibility；Cargo target 明确 `crossRootPortable=false`。soft budget 只告警，hard deadline 才
终止，cancel 优先，Completed/non-zero/Cancelled/Timeout/WaitFailed/SpawnFailed 保持不同终态。

### 22.1 Gate R-A - Schema、Lock Lineage 与 Resolved Identity

1. 新增 BuildRequest/BuildReport v3、Artifact/Descriptor/Qualification/Promotion v2、
   `GeneratedCompositionLockLineage.v1`、`ResolvedIdentity.v1` 和 typed deadline policy v1。
2. BuildReport v1/v2 只读；旧 artifact/qualification/promotion 不可作为 v2 promotion source，禁止空默认升级。
3. `lockInputDigest` 排除 root/PID/time；结构化解析 Cargo.lock，canonical graph 排除 generated root exact name
   与 absolute path，保留 package identity/version/source/checksum/features/sorted edges。
4. `rawLockDigest` 与 `resolvedGraphDigest` 分离；tamper、ambiguity、unsupported source、缺 checksum fail closed。
5. 定向测试：`project_editor_composition_r1_contract_`、`lock_lineage_`、`resolved_identity_`。

### 22.2 Gate R-B - Lineage Store 与 Path-Affine Compilation Cache v2

1. owner 管理 `locks/<lock-input-digest>/Cargo.lock + lineage.json`，以 staging/rehash/atomic publish/collision
   fail-closed 发布；valid hit 才跳过 `cargo generate-lockfile --offline`。
2. compilation cache v2 绑定 resolved graph 与稳定 build inputs；目录分片使用 canonical cache-owner
   anchor digest，`affinity.json` 另保存最终 canonical target-root digest，布局为
   `ct/<compatibility>/<target-anchor-prefix>/affinity.json + release/`。
3. 只有 compatibility + full anchor digest + full final root digest 同时匹配才是 `same_root_hit`；其它 root 报 `path_affine_miss` 并用
   新 target，不消费复制产物。v1 `ct` 不迁移、不删除、不命中。
4. 定向测试：`lineage_store_`、`compilation_cache_v2_`、`compilation_cache_path_affinity_`。

### 22.3 Gate R-C - Typed Deadline、Terminal 与 Cleanup Evidence

```text
generateLockHardDeadlineMs       60000
releaseBuildSoftBudgetMs        600000
releaseBuildHardDeadlineMs     1200000
descriptorQueryHardDeadlineMs    30000
```

1. 删除 composition owner 的散落 `step_timeout_ms.min(...)`；soft budget 越界不 kill，hard deadline 才终止。
2. 保留不同 process terminal，并把 output readers joined、root wait completed、process group released 与
   owned cleanup confirmed 提升到顶层 report；自然完成不伪装 terminated。
3. 只有现有 bounded process result 无法表达时才窄改 runtime_cli；composition soft budget不下沉通用 API。
4. 定向测试：`deadline_`、`process_terminal_`；runtime_cli 实际修改时跑其 bounded process test。

### 22.4 Gate R-D - Affected Owner / Consumer Closure

更新 production request builder、launcher、qualification seal、promotion、normal loader 与 fixture，统一使用
resolved identity。定向 red tests通过后运行一次 subsuming closure：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core
cargo test -p editor_window_winit project_editor_composition_ -- --nocapture
cargo check -p editor_window_winit --features real-window
cargo fmt --all -- --check
```

仅实际修改 runtime_cli 时追加 `cargo test -p runtime_cli`。Window A 完成后写
`03-R1-Window-A.md`，不得进入 R-E。

### 22.5 Window B / Gate R-E 至 R-G（已完成）

用户于 2026-08-12 单独授权 Window B。使用一个新 repository-external root 的 A/B 两个独立 target验证：
A 生成 sealed lineage；B 只复制 lineage 后得到 lineage hit + target `cold`；只复制 lineage 时没有旧 target
affinity 可供 owner 判定跨 root miss。另将旧 `ct` 复制到 `copied-ct-negative`，该否定样本必须得到
`path_affine_miss`，且旧 intermediate 不形成 `same_root_hit` 或被 Cargo 消费。
随后在 role-owned destination 完成 resolved-identity qualification/promotion/no-op/normal hit，最后只记录
`eligible_to_request_gate_h`。Window B 不触碰 production/真实 cache/config，也不自动进入 Gate H。

### 22.6 R1 激活前复核结果

```text
唯一当前施工文档：通过（本文）
正式方案与 Gate H 失败记录：通过
10 个 owner/consumer 文件哈希：与 282-R1 生成基线一致
测试入口：存在；新增 filter 将随 red tests 建立
Cargo.lock parser：必须使用结构化 TOML/serde 数据，不允许字符串删行 canonicalization
path-affinity 可实现性修订：anchor digest 定位目录 + final target digest 验证，避免路径自引用
三小时窗口：Window A 预计可完成
施工文档自审：仍有效
```

### 22.7 R1 Window A / Gate R-A 至 R-D 施工结果

```text
状态：完成
日期：2026-08-12
阶段记录：阶段完成记录/2026-08-11-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/03-R1-Window-A.md
editor_core：677 passed / 2 intentional ignored
editor_window_winit composition filter：8 passed / 3 intentional ignored
real-window check / fmt / scoped diff check：通过
runtime_cli：本窗口未修改，不追加 suite
```

schema v3/v2、generated lock lineage、resolved identity、path-affine compilation cache v2、typed deadline、
terminal/cleanup evidence 与直接消费者已闭环。施工中发现并修复 generated manifest staging absolute path
污染 lock input/artifact key 的 owner 缺陷；相同请求的真实 specialized Editor 第二次 prepare 已恢复 exact hit。

Window B / Gate R-E 至 R-G 已于 2026-08-12 单独授权并通过激活前复核；允许创建唯一的新 fresh role root。
仍不得运行 Local CI、触碰 production/真实 cache/config 或进入 Gate H。282 保留当前，不归档。

### 22.8 R1 Window B / Gate R-E 至 R-G 施工结果

```text
状态：完成；eligible_to_request_gate_h
日期：2026-08-12
fresh role root：C:\AifeProductionRuns\282-r1-20260812-135636
阶段记录：阶段完成记录/2026-08-11-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/04-R1-Repair.md
```

root A cold、root B lineage hit + target cold、copied-ct negative `path_affine_miss` 均完成真实 release build；
B/negative 越过 600 秒 soft budget 后继续成功。resolved graph/artifact key 在 A/B/negative、qualification、
promotion 和 normal loader 间一致。qualification passed，exact promotion/no-op 与 normal
`exact_cache/steps=0` 通过。Gate H 仍未授权，282 不归档。

## 23. 282-R1 新 Gate H 执行结果

```text
状态：未通过；normal loader exact hit 失败后完整回滚
日期：2026-08-12
fresh root：C:\AifeProductionRuns\282-r1-gate-h-20260812-150654
阶段记录：阶段完成记录/2026-08-11-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/05-Gate-H.md
production Editor：已恢复 F17BEDB2...C786
真实 cache：已恢复 14 entries 与原 inventory digest
```

candidate 构建、candidate-bound cold composition、qualification seal 与 `promote_exact` 均通过；cold build 在
600 秒 soft budget 后继续并于 1,186,097 ms 自然完成。首个 post-mutation 失败是 normal loader 没有消费已
发布的 generated lock lineage，重新进入 `generate_composition_lock`，因此无法证明 `exact_cache/steps=0`。
失败后新 exact entry/lineage 已移回 run root，旧 Editor byte-exact 恢复，配置/recent/trust/cache inventory
摘要均与 preflight 一致，owned process 为 0。

当前无继续施工授权。282 保持当前、不归档；下一步必须先讨论并授权最小 R2 lineage promotion/normal-loader
repair，修复与重新资格化前不得再次进入 Gate H。

## 24. 282-R2 B-min Repair 原激活计划（审查后冻结）

```text
激活日期：2026-08-12
用户授权：282-R2 B-min Repair Window A / Gate R2-A 至 R2-D
正式方案：282-R2-Exact-Artifact-Lineage-Transactional-Promotion-Normal-Hit-Consistency-B-min-v1方案.md
激活前复核：通过
源码基线：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留 dirty worktree
当前授权：原 Window A / Gate R2-A 至 R2-D 已由用户要求重新审查；本节过量范围冻结
未授权：Window B / R2-E 至 R2-F、fresh root、Local CI、production/installed binary、真实 cache/config、Gate H
独立计划：已移入施工文档/历史/，只作合入证据，不再是执行入口
```

### 24.1 激活前复核结果

```text
正式方案与施工文档自审：通过；无需修订正式方案
R1 完成记录与 05-Gate-H 回滚事实：一致
唯一可执行施工文档：本文；当前目录其它 md/txt/json 为历史 evidence，不构成施工计划
五个核心 owner/consumer hash：与 R2 施工文档生成基线完全一致
promotion schema v2、fault injection seam、成功/失败 qualification harness：存在
新增 R2 filter：尚不存在，符合 red-first 施工预期；0 tests passed 禁止过 Gate
dirty worktree：保留；禁止 reset/revert/clean/覆盖恢复
三小时可行性：Window A 为源码 owner + affected closure，可在独立三小时窗口内执行
外部状态：不需要且未授权；不创建 repository-external fresh root
```

复核 hash：

```text
project_editor_composition.rs
  7BFBC5E4EE526EBF732AAAEC368EAE2999621BC5A719506D72EA15123F0284F7
project_editor_composition_artifact.rs
  672B9F2851EE61255BC0E215F919407245D5677468663495B70FCCCC73DAC930
project_editor_composition_cache_promotion.rs
  D847AEB3182C3211944A4EAD8EE83CEC53E9DE6FC0781C40273DF04A1CAA1FCD
editor_core/src/lib.rs
  5FBC9315C6CC3A85EBA22AE8377CF1AEE49682D13D97633822A1AA68E8E9F4F9
project_editor_composition_qualification.rs
  EF64485970BBE2EC529525DEB87A2F6FE9FBB46A225594E48748BE4AD4003F5A
```

### 24.2 Window A 不变量与文件范围

1. normal-hit probe 只改变 `cargo_executable`；`cargo_identity` 与所有 digest-bearing semantic fields
   必须与 source request byte-equal。
2. requested/resolved identity、trust、qualification seal 与 executable hash 不放宽。
3. promotion transaction participant 固定为 one exact artifact + one exact generated-lock lineage。
4. source bytes 必须重算 lock-input/raw-lock/graph/lineage/artifact-key 与 artifact/seal hashes。
5. v3 writer 显式拥有 lineage authority；v1/v2 只读且不能 silent upgrade mutation authority。
6. absent/exact/different 全部走 pair plan；exact/exact 也必须双验证。
7. publish/final verify/report finalization 失败时 pair 共同 rollback 并 re-hash。
8. `ct` 保持 `crossRootPortable=false`，不进入 promotion。

默认允许修改：

```text
rust/crates/editor_core/src/project_editor_composition.rs
rust/crates/editor_core/src/project_editor_composition_artifact.rs
rust/crates/editor_core/src/project_editor_composition_cache_promotion.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs
```

仅为直接 consumer closure 可窄改：

```text
rust/crates/editor_window_winit/src/project_editor_composition_production.rs
rust/crates/editor_window_winit/src/tests/project_editor_composition_production_gate.rs
```

不预授权修改 `runtime_cli`、normal-loader owner、Tower 或其它子系统。若必须扩大范围才能实现，立即停止并
回到方案审查。

### 24.3 Gate R2-A - Promotion v3 Schema 与 Migration

1. request v3 加入 source lineage directory、`Cargo.lock`、`lineage.json` 以及 expected lock-input/raw-lock/
   graph/lineage/artifact-key facts。
2. report 对 artifact/lineage 分别报告 source/staged/final path/hash、prior state、action、backup、rollback、retained。
3. writer 只写 v3；v1/v2 report 历史只读；v1/v2 request 在 mutation 前以 lineage authority missing fail closed。
4. 保持 deny-unknown、absolute paths、authority id、requested/resolved validation；更新直接 constructors。

先建立能使旧实现失败的 red test，再实现：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_promotion_schema_ -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_v2_ -- --nocapture
```

通过条件：v3 round-trip/unknown rejection；v2 destination 零 mutation；所有 writer/consumer 显式提供 lineage
authority；filter 实际执行数大于零。

### 24.4 Gate R2-B - Source Lineage Full-chain Validation

在 destination mutation 前 containment/reparse-check source paths；从 bytes 重算 lock input、raw lock、canonical
graph、lineage、resolved artifact key、artifact/descriptor/report/seal/executable hashes；复用 R1 结构化
Cargo.lock owner并检测 validation/copy TOCTOU。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_promotion_source_lineage_ -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_pair_mismatch_ -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_rejects_identity_hash_report_seal_and_path_tamper -- --nocapture
```

矩阵至少覆盖 raw lock、lineage JSON、graph edge/checksum、resolved key、source mutation、reparse/outside tamper。
失败必须在 destination mutation 前并给 expected/actual digest。

### 24.5 Gate R2-C - Pair Transaction 与 Rollback

深化现有 `promote_exact_with_faults`：destination staging copy artifact/`Cargo.lock`/`lineage.json` 并重算；处理
absent/exact/different；backup、lineage publish、artifact publish、final reopen verify；任何阶段或 report
finalization 失败共同 rollback。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_promotion_pair_ -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_detects_copy_mutation_and_rolls_back_publish_failure -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_rollback_failure_is_terminal_and_retained -- --nocapture
```

fault matrix 分别覆盖 lineage copy、lineage publish 后 artifact failure、final verify、report finalization、rollback
failure。pair 双发布/no-op 必须通过；失败要么恢复 mutation 前 pair，要么 terminal retained。

### 24.6 Gate R2-D - Harness Identity 与 Affected Closure

1. 增加 clone-based request derivation helper，只替换 executable 与允许的 run-control path。
2. prepare 前比较 semantic fields；identity 改动报告 `qualification_normal_hit_identity_mismatch`。
3. 修正 Gate H harness 的 `cargo_identity`，但不得运行 Gate H test。
4. 正确 request contract 为 `exact_cache / steps=0 / exact_lineage_hit / cargoSpawnAttempted=false`。
5. 更新 promotion/admin/qualification/production 直接 consumers 与 diagnostics。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit project_editor_composition_normal_hit_request_identity_ -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_ -- --nocapture
cargo test -p editor_window_winit project_editor_composition_ --lib -- --nocapture
cargo test -p editor_core
cargo check -p editor_window_winit --features real-window
cargo fmt --all -- --check
```

开发循环先跑新增 owner filters；最终完整 suite 在同配置下 subsume 前述 filters，不机械重复。Window A 完成后
写 `06-R2-B-min-Window-A.md`，记录 source/test identity、首个红灯、实现、实际命令和未执行项，然后停止。
不得创建 fresh root、进入 Window B、运行 Gate H candidate/promote tests、Local CI 或触碰 production/真实状态。

### 24.7 Evidence、失败、时间与终点

每 Gate 保留 source/harness identity、request/report hash、首个 assertion/diagnostic、stdout/stderr/timing、
artifact+lineage facts、destination before/after、rollback/retained 与外部状态未触碰事实。失败先做当前 owner
reproduction，不跨 Gate、不用 Local CI/production 调试。

Window A 自激活完成开始计入三小时：2:45 后不启动新实现/验证，3:00 停止并记录
`stopped_by_time_limit`。需弱化 identity/trust/seal、修改 normal lookup、无法 pair rollback 或必须触碰外部状态时
立即停止回到方案。

原 Window A 未开始任何代码或测试。审查确认 R2-A 至 R2-C 及 R2-D 中的 schema/report 扩展属于过量施工，
本节不再是当前执行入口；由第 25 节最小施工计划替代。Window B / R2-E 至 R2-F、fresh root、Gate H 与所有
外部 mutation 仍未授权。

## 25. 282-R2 B-min 修订后最小施工计划 - Harness Identity Consistency

```text
修订日期：2026-08-12
范围来源：用户要求审核并消除过量施工
正式方案：第 18 节 Harness Identity Only scope correction
当前状态：已完成（2026-08-12）；282 父施工文档继续保留当前
当前授权映射：原 R2-A 至 R2-D 授权收缩为本节单一 Gate R2-M；不扩大授权
未授权：promotion schema/transaction、Window B、fresh root、Local CI、production/真实 cache/config、Gate H
```

### 25.1 必要性审计

```text
confirmed failure
  Gate H normal-hit request 把 cargo_identity 从 candidate 改成 must-not-run，导致 lockInputDigest 改变。

minimum causal fix
  从 source request 派生 normal-hit request，只替换 cargo_executable。

files that must change
  rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs

red-capable tests
  修改 cargo_identity 的 request 必须与 source semantic identity 不同；只改 executable 必须保持相同。
  disposable cache hit 继续用现有 ExactCache + steps=0 证明没有 Cargo step。

explicitly deferred
  promotion v3、lineage pair transaction、migration/fault matrix、重复 report 字段。
```

### 25.2 Gate R2-M - 最小实现

1. 提取一个测试可见的 clone-based normal-hit request helper，或在现有 test 中直接 clone source request；
2. helper 只更新 `build_root`（当 destination 不同）与 `cargo_executable`，不重写 `cargo_identity`；
3. Gate H harness 使用 source request 保存的真实 `cargo_identity`，删除
   `cargo 282-gate-h-must-not-run`；
4. 增加一个不启动 Cargo、不创建 fresh root 的 unit test，比较 source 与派生 request 的
   `GeneratedCompositionLockInput` 相关稳定输入，至少明确断言 `cargo_identity` 保持一致；
5. 不修改 `ProjectEditorCompositionPromotionRequest/Report`、`promote_exact`、normal loader 或 build report schema。

允许修改文件只有：

```text
rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs
```

如果实现需要修改其它产品文件，立即停止并重新审查，不自动扩大。

### 25.3 最小验证

开发循环与最终证据只运行实际受影响的定向测试：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit project_editor_composition_normal_hit_request_preserves_cargo_identity --lib -- --exact --nocapture
cargo fmt --all -- --check
```

若新增 test 位于 module path，激活施工时先用 `cargo test -p editor_window_winit --lib -- --list` 确认 exact 名，
不得以 0 tests passed 过 Gate。只有编译错误显示直接 consumer 受影响时才追加最窄 `cargo check -p
editor_window_winit`；不默认运行完整 `editor_core`、winit composition suite、real-window check 或任何 ignored
Gate H test。

### 25.4 通过条件与终点

- source 与派生 normal-hit request 的 `cargo_identity` 相同；
- 只改变 executable 不改变用于 generated lock identity 的 semantic input；
- Gate H harness 不再出现 `cargo 282-gate-h-must-not-run`；
- 定向 test 实际执行 1 个且通过；format 通过；
- 写 `06-R2-B-min-Window-A.md`，明确这是 source/harness 修复，不声称 production Gate H 已通过；
- 完成后停止。不得进入 Window B、创建 fresh root、运行 Local CI、触碰 production/真实 cache/config 或
  重跑 Gate H。

### 25.5 修订后施工自审

```text
与已证实首因一致：是
是否包含推测性架构增强：否
预计产品文件：1
新增 schema/owner/transaction/report 字段：0
定向测试：1 + fmt
复用现有证明：ExactCache + steps=0 已足以表达 no-Cargo
施工文档自审：通过
```

### 25.6 Gate R2-M 完成结果

```text
完成日期：2026-08-12
source：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留 dirty worktree
产品文件：rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs
结果：完成
```

Gate H candidate source request 已集中到共享构造入口；normal-hit request 从 source request clone 后只替换
`build_root` 与 `cargo_executable`，不再重写 `cargo_identity`。Gate H harness 中
`cargo 282-gate-h-must-not-run` 已删除。新增纯内存 unit test 逐项证明 `cargo_identity`、expected identity、
project/sdk roots、schema、cache/QoS/deadline policy 与 capture limit 保持一致，仅两个允许字段改变。

验证结果：

```text
cargo test -p editor_window_winit project_editor_composition_qualification::tests::project_editor_composition_normal_hit_request_preserves_cargo_identity --lib -- --exact --nocapture
  1 passed; 0 failed; 198 filtered out

cargo fmt --all -- --check
  passed
```

未运行 ignored Gate H tests、fresh root、Local CI、完整 suite、real-window check 或 release build；未修改
promotion schema/transaction、normal loader、build report schema、production/installed binary、真实 cache/config、
Tower project。R2-M 只修复 source/harness identity consistency，不声明 Gate H 已通过。282 父施工文档继续保留
`当前/`；下一步仅为 `eligible_to_request_new_gate_h`，必须取得新的单独授权。

### 25.7 新 282 Gate H 结果

新 Gate H 于 2026-08-12 获得单独授权并使用 fresh root
`C:\AifeProductionRuns\282-r2-gate-h-20260812-201612` 执行。candidate
`8B7156FF138DDCF46C89EC7A5AF56908A3E74B967AF9CE28148B388A3A87A9F9` readiness 通过，production
Editor `F17BEDB2...C786` 已完成 byte-exact backup，preflight 记录真实 cache 14 项、generated-lock lineage 0 项。

Gate 在任何 production mutation 前停止：candidate-bound `generate-lockfile` 945 ms 成功，release build 使用
`--release --locked --offline --jobs 1`，在 1,200,017 ms 达到 hard deadline。report 为 `Failed`，diagnostic
为 `project_editor_composition.release_build_hard_timeout`；process tree 已终止、wait/output readers 已完成，
staging cleanup 为 `removed`。因此未执行 production Editor replacement、exact promotion 或 normal-hit，不能对
R2-M identity repair 作 production 通过/失败结论。

收尾确认 production/backup hash 一致，真实 cache inventory、locks inventory、trust、Codex config、recent
projects 均与 preflight 相同，owned process 为 0；因 mutation 未开始，无需回滚。未运行 Local CI，未修改真实
配置，未替换 Player/MCP/其它安装态二进制。Gate H 不得直接重跑；下一步先讨论 candidate-bound cold
compilation deadline/reuse 的最小 repair。

### 25.8 Gate H 断点续验 - 复用旧 qualified candidate

2026-08-12 用户授权按以下最小链路继续 Gate H：

```text
校验旧 candidate/artifact/seal 哈希
-> 备份当前 production Editor 和目标 cache entry
-> 恢复并使用旧 qualified candidate
-> 重新 promote 旧 qualified artifact
-> 只运行修复后的 normal-hit
-> 普通 Editor smoke
```

本分支替代 14.2 中对本次续验不必要的 fresh candidate build、cold composition build、重复 qualification
与重复 seal。只有同时满足以下条件才允许复用：

1. 旧 candidate、source artifact、descriptor、build report、qualification report/seal 文件仍完整；
2. 文件实际 SHA-256 与旧 Gate H retained evidence 一致；
3. R2-M 只改变 `#[cfg(test)]` Gate H harness 的 normal-hit request 构造，不改变 candidate、composition
   artifact 或其任何 digest-bearing product input；
4. 当前 production Editor、真实 cache、真实配置与 owned-process 状态在 mutation 前重新记录；
5. normal-hit 必须通过当前修复后的 harness 执行，并证明 `ExactCache`、`steps=0`、resolved identity exact；
6. 任一校验或 post-mutation 步骤失败时立即恢复本次备份并停止。

本次允许修改的外部状态仅限 production Editor 和目标 exact cache/lineage；不运行 Local CI，不修改真实配置，
不替换 Player/MCP/其它安装态二进制，不清理其它 cache，不构建新 candidate，不运行冷 compilation 或重复
qualification。普通 Editor smoke 只覆盖打开 Tower、Play 及一次真实输入；不扩展为完整视觉矩阵。

必要性审计：

```text
confirmed failure: old Gate H normal-hit request 错误改写 cargo_identity；新 Gate H 未到该 branch 即冷编译超时
minimum causal closure: 复用已 qualified/sealed 的旧产品证据，仅续跑 promotion 后的修复 normal-hit 与 smoke
files that must change: 当前施工文档与本次阶段记录；无需修改产品源码
red-capable test: 修复后的 ignored promote-and-normal-hit harness，失败时能直接捕获非 ExactCache/steps!=0
explicitly deferred: deadline 调整、cross-root compilation reuse、新 schema/runner/transaction、完整矩阵
```

施工文档自审：通过。该分支缩小而不扩大 Gate H；它不削弱 exact identity/seal 校验，只避免重新生产未失效
的 evidence。旧 candidate 为 `5007CDA6...B9BC2`，旧 artifact key 为 `0b4ed21b...2aa`；未完成 seal 的
`8B7156FF...F9` candidate 不参与本次续验。

首次断点续验的 `promote_exact` 成功，但 normal-hit 报告证明 promotion owner 只发布了
`cache/<artifact-key>`，没有发布其 `locks/<lock-input-key>`；loader 因此进入 `generate_composition_lock` 并按
预期被不存在的 Cargo 拦截。production Editor、exact cache 与 lineage 已全部回滚到 mutation 前状态。

旧 Gate H source root 与旧 rollback evidence 均保留同一份 lineage：key
`b4ac8181...e0a`，`Cargo.lock` SHA-256 `1CFCCC1E...7A3D`，`lineage.json` SHA-256
`08EF2A98...1618`，两份逐字节一致。本次“恢复旧 qualified artifact”明确包含先将这份已有 sealed lineage
复制到真实 `locks/<lock-input-key>`，再调用既有 `promote_exact` 发布 exact cache entry。该操作不生成 lock、
不调用 Cargo、不修改 schema/owner/transaction；若后续 normal-hit 失败，两个目标仍按同一回滚规则移出。

### 25.9 Gate H 断点续验完成结果

2026-08-13 断点续验完成。旧 candidate `5007CDA6...B9BC2`、artifact executable
`4C3351F6...FCB6`、descriptor、build report、qualification report/seal 与 lineage 全部先按实际 SHA-256
复核。production Editor 原件 `F17BEDB2...C786` 已备份，随后精确替换为旧 qualified candidate。

首次续验中 `promote_exact` 成功但 normal-hit 因真实 `locks/<lock-input-key>` 缺失而 fail；该次 mutation 已
立即完整回滚。复核 source 与 retained lineage 字节一致后，将旧 qualified artifact 的 exact cache entry 与
对应 sealed lineage 作为一组恢复。修复后的同一 ignored harness 随后通过：

```text
project_editor_composition_282_gate_h_promote_and_normal_hit
1 passed; 0 failed; 198 filtered out
normal report: sourceKind=exact_cache, cacheStatus=hit, steps=[]
```

最小普通 Editor 真实窗口 smoke 使用 run-owned Tower snapshot 与 run-owned recent/workspace 状态，7 个步骤
全部通过：open recent、workspace ready、Play、runtime active、720x1280 Contain 下点击 `recruit-button` 并观察
`td.recruit`、Stop、runtime stopped。场景耗时 `1,364 ms`，diagnostics 为空。

最终 production Editor SHA-256 为 `5007CDA6...B9BC2`；真实 exact cache 与 lineage 均存在，owned
Editor/Cargo/rustc process 为 0。旧 production Editor 备份、失败回滚产物和最终证据保留在
`C:\AifeProductionRuns\282-r1-gate-h-20260812-150654\resume-20260812`。未运行 Local CI，未修改真实配置，
未替换 Player/MCP/其它安装态二进制，未运行完整视觉矩阵，未重新构建 candidate、冷 composition、
qualification 或 seal。282 全部 Gate 完成，可以归档。
