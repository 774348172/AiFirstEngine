# 282 Project Editor Composition Cache Lifecycle + Build QoS v1 方案

## 1. 文档状态

```text
系统编号：282
方案名称：Project Editor Composition Cache Lifecycle + Build QoS v1
问题来源：post-281 production Editor 项目打开与操作严重卡顿诊断
用户选择：方案 C（缓存生命周期 + 构建响应性治理）
当前状态：Window A / Gate A-D 与 Window B / Gate E-G 已完成；Window C / Gate H 在 production mutation 前失败并停止；施工文档仍在当前、未归档
日期：2026-08-11
```

本文深化 262 已有的可信 ProjectRust Editor composition artifact owner，并继承
268/269 的后台 preparation、exactly-once busy 与 compilation compatibility cache。本文不建立
第二套 composition builder，不放宽 exact artifact seal，不把诊断中的一次手工缓存操作固化成无授权自动化。

## 2. 这个系统是做什么的

简单说，它解决两件事：

1. production Editor 更新后，若 fresh run 已经产生了与新 Editor 精确匹配的合格
   composition，在明确授权、完整校验和可回滚前提下把它提升到真实用户缓存，
   避免用户打开项目时重新编译。
2. 当精确缓存确实不存在时，后台 Cargo/rustc 构建不得抢占全部 CPU 和驱动全速
   Editor 重绘；用户仍应能正常拖动、点击和关闭窗口。

在成熟引擎中，这类能力对标的不是渲染优化，而是 Build Tool 的增量产物复用、资源感知并发限制、
后台优先级与可取消生命周期。

## 3. 已确认的首因与证据

### 3.1 真实环境事实

post-281 production Editor 二进制 SHA-256 变为：

```text
f17bedb2a3892486ded074de1e4fa1aef9d4d14cc37e51e35b4155bebd23c786
```

与其匹配的 Tower composition artifact identity：

```text
a270b4fea0b8bbe376891e726f223fbba93db55ff29f9728d7643559efe8b798
```

诊断时该 exact artifact 只存在隔离 run root，不在普通 Editor 使用的真实缓存中。
真实缓存 miss 后，当前 owner 执行：

```text
cargo build --release --locked --offline
```

代表性 `build_composition_release` 耗时 `170852 ms`，期间编译 Engine Runtime、Editor 与 Tower
RuntimeModule 相关 crate。Cargo 没有指定 `--jobs`，使用默认 CPU 并发。

构建结束后性能恢复正常：

```text
Editor idle CPU             0.01 core
UI message latency P95      0.10 ms
resize latency P95          32.84 ms
system idle CPU             7.6%
available memory            about 7 GB
```

因此 Sprite2D、GameView 或 AUI present 不是本次常驻卡顿的主因。

### 3.2 当前代码链

```text
project_editor_composition_artifact.rs
  -> exact identity cache lookup
  -> miss
  -> generated composition staging
  -> cargo generate-lockfile --offline
  -> cargo build --release --locked --offline
  -> descriptor query + exact verification
  -> executable hash + atomic cache publish

application.rs
  -> project-editor-composition-prepare worker
  -> worker 存在期间 redraw_requested = true

real_window.rs
  -> 每次 report 仍要求 redraw 时继续 request_redraw
```

已存在的正确能力：

- 262 的 exact identity、descriptor、executable hash、trust、cache publish 和 handoff；
- 268 的项目打开后台 worker、busy 保护、progress 与 cancel/join 模式；
- 269 的 exact artifact seal 与 compilation compatibility identity 分离，以及 application-owned
  Cargo target 复用；
- shared bounded child process 的 timeout、stdout/stderr 有界捕获和 process result。

当前真实缺口：

- exact artifact 只能在当前 cache root lookup，不能校验并提升已合格的外部 exact artifact；
- Cargo 构建没有 Editor 专用的 jobs / memory / priority policy；
- composition worker 没有纳入 shutdown cancel + join，也没有可取消进程树所有权；
- worker 存在本身会触发连续整帧重绘，而不是由状态变更或有界进度节拍驱动；
- build report 只有宽泛的字符串 cache status，不能回答 jobs、priority、promotion/build
  source、cancel 和 UI responsiveness 等问题。

## 4. 成熟实现参考

### 4.1 Cargo

Cargo 官方配置文档明确：`build.jobs` / `--jobs` 控制并行作业数，默认为逻辑 CPU 数。
这说明当 Editor 与 Cargo 共享一台交互式机器时，引擎必须在 process-launch seam 显式给出资源预算，
不能把 Cargo 的批处理默认直接当作 Editor 产品默认。

参考：

- <https://doc.rust-lang.org/cargo/reference/config.html#buildjobs>
- <https://doc.rust-lang.org/cargo/reference/build-cache.html>

### 4.2 Unreal Build Tool

Unreal Build Tool 的官方配置公开了 `MaxParallelActions`、processor count multiplier、每个 action
内存约束与可用内存检查；同时使用可复用的构建状态减少不必要的重新工作。
可学习点是“产物复用 + CPU/内存感知调度”同时存在，而不是只调一个并发数。

参考：

- <https://dev.epicgames.com/documentation/en-us/unreal-engine/build-configuration-for-unreal-engine>

不直接照搬 UBT 的分布式构建、远程 executor 或巨大配置面；282 v1 只深化现有本机、离线、
release composition 链。

## 5. 方案比较与正式选择

### 5.1 方案 A：只预热真实缓存

优点：当前项目立即收益，改动少。

缺点：每次新 identity 或缺少可提升产物时，Editor 仍会被无约束构建拖慢。

### 5.2 方案 B：只修复构建响应性

优点：冷构建时 Editor 仍可用，能解决拖动与关闭卡顿。

缺点：仍要支付几分钟冷构建，不解决 production update 与 fresh qualification 产物割裂。

### 5.3 方案 C：缓存生命周期 + 构建响应性治理

同时完成：

```text
C1 Editor Composition Build QoS
  -> 动态但有上限的 Cargo jobs
  -> Windows below-normal process priority
  -> cancel + owned process-tree termination + join
  -> progress redraw 10-20 Hz，input redraw 仍立即
  -> typed QoS / duration / cancellation report

C2 Exact Composition Cache Lifecycle
  -> 只接收明确授权的 source run root
  -> exact identity / descriptor / executable hash / qualification 校验
  -> destination-owned staging 再校验
  -> 旧目标备份 + atomic publish + rollback
  -> 无合格 candidate 时回退到 C1 受控构建
```

正式选择：方案 C。

## 6. 目标与非目标

### 6.1 v1 目标

1. 保留 262 exact composition identity、descriptor seal、executable hash 和 trust 全部不变。
2. 真实 cache miss 时，Editor 交互不再因 Cargo/rustc 与无界重绘而饥饿。
3. composition build 必须可取消，Editor shutdown 必须 cancel + terminate owned tree + join。
4. 已验证 exact artifact 可经独立、明确授权的 maintenance 路径提升到真实缓存。
5. 提升必须事务化，不破坏旧缓存，失败可审查、可回滚。
6. UI 明确区分 cache lookup、promoting、warming、compiling、sealing 与 handoff。
7. report 能回答本次是 hit、promotion 还是 build，实际 jobs、priority、各阶段耗时和取消结果。

### 6.2 v1 非目标

- 不修改 Tower gameplay、AUI、Sprite2D、Animator2D、RuntimePackage 或项目配置。
- 不新建通用 build daemon、workflow engine、分布式构建或远程 cache。
- 不修改 Cargo 全局配置、用户项目 `.cargo/config.toml` 或真实项目 manifest。
- 不自动替换 production/installed Editor、Player、MCP 或其它二进制。
- 不将 fresh run root 或任意目录自动扫描为缓存来源。
- 不通过忽略 `editorBuildIdentity`、AOT digest、lock digest 或 executable hash 提高 hit rate。
- 不在 v1 将当前“Editor 整个可执行文件哈希”替换为新 ABI/capability digest。该深化可以
  减少与 composition ABI 无关的 invalidation，但必须作为后续独立方案，不得在 282 中顺手放宽。
- 方案文档不授权 Local CI、真实缓存写入、production 替换或真实 Editor 操作。

## 7. 不可破坏的正式不变量

### 7.1 exact identity 不变

262 的 `ProjectEditorCompositionIdentity.v1` 仍是最终 artifact 复用真相：

```text
Engine SDK/source identity
Editor build identity
toolchain + target triple + profile + features
projectId
moduleId + interfaceVersion + AOT content digest
normalized manifest/dependency/lock digests
composition schema/tool version
```

最终 artifact 只能 exact hit 或 exact promotion。269 的 compilation compatibility identity 只能复用 Cargo
target 中可证明兼容的编译中间产物，不能代替最终 artifact identity。

### 7.2 trust 不变

cache promotion 只证明产物的完整性与精确性，不代表用户对当前 ProjectRust 的运行信任。
Editor 打开项目仍必须经过现有 `ProjectRuntimeTrustModule`。trust receipt stale/denied/required 时必须
fail closed，不得因缓存存在而绕过。

### 7.3 授权不变

普通 Editor 打开项目只能读取自己的 application-owned cache 并在 miss 时走受控构建。
只有单独授权的 production consistency / cache warm-up 操作可以提供 external candidate root 并写入
真实用户缓存。方案实现不得将该权限隐式绑定到 Editor 启动、项目打开或 Play。

## 8. 正式架构

```text
Authorized Production Consistency Adapter              Normal Editor Open
  -> exact candidate root + authority intent             -> trusted project request
  -> promote_exact(request)                               -> prepare(request, control)
                 |                                                     |
                 v                                                     v
      ProjectEditorCompositionArtifact owner (existing deep owner)
        -> expected exact identity
        -> exact destination cache lookup
        -> optional exact promotion transaction
        -> controlled staging / generated composition
        -> internal EditorCompositionBuildQoSModule
        -> descriptor query / executable hash / seal
        -> atomic publish / cache policy
        -> typed report
                 |
                 v
      existing EditorProjectCompositionLauncher / handoff
```

`ProjectEditorCompositionArtifact` 继续是 artifact/cache 唯一 owner。不在 production updater、
`editor_window_winit` 或 Tower 项目中复制 descriptor 校验、哈希、publish 和 cache key 逻辑。

## 9. 深 Module 与 Seam

### 9.1 外部 interface

现有 artifact owner 保持小 interface：

```text
prepare(request, control) -> ProjectEditorCompositionBuildReportV2
promote_exact(request)    -> ProjectEditorCompositionPromotionReportV1
```

`prepare` 隐藏 lookup、staging、Cargo、seal、publish、cleanup 和 report；`promote_exact` 隐藏 candidate
containment、qualification、双重哈希校验、备份、原子发布与回滚。caller 不得逐步编排内部流程。

### 9.2 内部 process-launch seam

新增内部深 `EditorCompositionBuildQoSModule`：

```text
execute(build_plan, qos_policy, cancellation) -> BuildExecutionReport
```

它内部负责：

- 根据逻辑 CPU、可用内存和 typed policy 求出实际 jobs；
- 只通过当次 Cargo 参数/进程环境施加限制，不写入项目或全局配置；
- Windows 上以 below-normal priority 启动 Cargo，并将优先级与进程树所有权传递给 rustc/linker 子进程；
- timeout / cancel 时终止 owned process tree，回收 stdout/stderr reader，然后 join；
- 返回实际 jobs、priority、各步耗时、exit reason 和 cancel/cleanup 事实。

该 seam 必须有两个 Adapter：

```text
SystemCompositionBuildProcessAdapter   production Cargo/rustc process tree
ScriptedCompositionBuildProcessAdapter deterministic owner tests
```

这是内部 seam；不将 Windows process flag、Cargo 命令细节或测试脚本暴露给 Editor UI。

## 10. C1：Build QoS 与交互响应性

### 10.1 jobs 决策

v1 使用 typed default policy，不在命令构造中散落魔法数字：

```text
maxJobs                         4
reservedLogicalProcessors      4
reservedAvailableMemoryBytes   3 GiB
estimatedMemoryPerJobBytes      1 GiB
minJobs                         1
```

实际 jobs 由 CPU 上限、内存上限和 `maxJobs` 取最小值，并且至少为 1。当可用系统信息不可读时
fail soft 到保守值，不回退到 CPU-wide default。当前 12 逻辑线程机器的预期默认为 `jobs=4`。

只在 `build_composition_release` 传入 `--jobs <resolved>`；`generate-lockfile` 不需要并发参数。

### 10.2 Windows 进程优先级

Cargo 根进程使用 below-normal priority class。实现必须验证 rustc/linker 子进程获得预期优先级，
不能只在 report 中声明。其它平台使用 typed `best_effort` 结果并如实报告，不伪造已生效。

### 10.3 cancellation 与 shutdown

composition preparation worker 必须与 project-open / play worker 一样进入 application-owned lifecycle：

```text
Cancel requested
  -> cancellation token terminal
  -> stop spawning further steps
  -> terminate owned Cargo process tree
  -> join output readers
  -> cleanup staging or report retained reason
  -> join preparation worker
  -> exactly one cancelled terminal report
```

Editor `Drop`、主窗口 CloseRequested 与 handoff cancellation 不得只取消 UI activity；必须处理实际 process tree。
如果无法证明 owned tree 已终止，不得删除可能正在被写入的 target/staging，应报告 retained reason。

### 10.4 progress 与 redraw

移除“`composition worker is_some` 就每帧整窗重绘”的条件。新规则：

- pointer、keyboard、window resize、GameView tick 等真实交互仍立即 redraw；
- build state / progress 变更立即 redraw；
- 只为 elapsed-time 刷新的 progress redraw 限制在 `10-20 Hz`，默认 `10 Hz`；
- winit `ControlFlow::WaitUntil` 调度下一个 progress deadline，不使用 busy loop；
- 快速输入不受 progress throttle 影响。

## 11. C2：Exact Cache Promotion / Warm-up 合同

### 11.1 只能由授权 maintenance 路径调用

`promote_exact` 不是普通 Editor command，不进入 ProjectPatch、AUI action、Gateway public tool 或项目配置。
调用方必须显式提供：

```text
authorized source run root
source artifact root
destination application-owned cache root
expected exact composition identity
qualification evidence reference + digest
operation/run id
backup root
```

方案不定义“授权”的自然语言解析；它只接收上层已明确批准后形成的 typed request。

### 11.2 source 校验

候选产物必须全部通过：

1. source artifact root canonicalize 后仍在 authorized run root 内；
2. 拒绝 symlink / junction / reparse escape 与非普通文件；
3. descriptor schema 可支持，identity digest 与 expected exact match；
4. executable 文件名/路径符合 descriptor，重算 SHA-256 与 descriptor exact match；
5. build report 为 success，artifact/descriptor/identity 与当前 candidate exact match；
6. qualification evidence 为已通过，绑定相同 composition identity 和实际 executable hash；
7. expected Editor/toolchain/target/profile/module/AOT/manifest/dependency/lock 全部 exact match。

当前 qualification report 如不足以绑定 executable hash，施工必须先对其 schema 升级或增加封印收据；
不得将“文件名对了”当成已合格。

### 11.3 destination 事务

```text
validate source
  -> copy into destination-owned staging
  -> re-read descriptor and re-hash copied executable
  -> write promotion report candidate
  -> if exact destination already valid: no-op hit
  -> if destination exists but invalid/different: move to operation backup
  -> atomic publish staging to cache/<exact-key>
  -> load through normal cache loader
  -> commit receipt
```

任一步失败时：

- 已有有效 destination 不受影响；
- 已备份的旧 destination 必须恢复；
- staging 清理失败则报告 retained path/reason；
- 不得启动新 Editor，不得修改 trust receipt；
- 是否 fallback 到 C1 构建由 maintenance caller 的 typed policy 决定，不在失败分支中暗中执行。

### 11.4 production update 顺序

282 不拥有 production Editor replacement。与已有 production consistency 流程组合时，正确顺序为：

```text
separately authorized Editor binary replacement
  -> verify installed Editor hash
  -> derive expected composition identity
  -> promote exact qualified artifact if available
     or warm through bounded QoS build
  -> verify normal cache loader exact hit
  -> only then launch normal Editor smoke when separately authorized
```

不得先打开普通 Editor 触发无约束冷构建，再把编译完当作更新闭环。

## 12. Schema 与迁移

### 12.1 保持 v1 不变

```text
project-editor-composition-identity.v1
project-editor-composition-artifact.v1
project-editor-composition-descriptor.v1
project-editor-composition-handoff-ticket.v1
project-editor-composition-launch-receipt.v1
```

这些合同不因 QoS 或 promotion 改变，避免制造不必要的 artifact identity 迁移。

### 12.2 新增/升级

```text
project-editor-composition-build-request.v2
project-editor-composition-build-qos-policy.v1
project-editor-composition-build-report.v2
project-editor-composition-promotion-request.v1
project-editor-composition-promotion-report.v1
```

`BuildRequest.v2` 增加 typed QoS policy；取消 token 是 in-process control，不序列化进 schema。
`BuildReport.v2` 至少包含：

```text
status
sourceKind = exact_cache | exact_promotion | controlled_build
cacheStatus = hit | promoted | miss | invalidated | rebuilt
resolvedJobs
requestedPriority / effectivePriority / priorityApplied
logicalProcessors / availableMemoryBytes / policyDecision
stage durations
cancellationRequested / processTreeTerminated / workerJoined
redrawPolicyHz
artifact / identity / executableHash
cleanupStatus / retainedPaths / diagnostics / nextAction
```

reader 必须继续支持历史 `BuildReport.v1` 的只读解析，writer 只写 v2。不追溯改写旧 cache report。

`PromotionReport.v1` 至少包含 source/destination canonical root、authority operation id、
expected/actual identity、source/copied/final executable hash、qualification evidence digest、backup/rollback/cleanup 状态。

## 13. UI 与状态机

`ProjectOpenActivityPhase` 收敛为能区分实际工作的 typed phase：

```text
Inspecting
CacheLookup
Promoting
Warming
Compiling
Sealing
Handoff
Ready
Cancelled
Failed
```

UI 只显示用户需要的短文案与进度，不暴露 Cargo 命令、缓存绝对路径或长日志。
完整技术证据进 Summary/Trace report。进度未知时使用不确定状态，不伪造百分比。

本系统不将编译进度放入 runtime hot path。Editor report 遵循 `Off / Summary / Trace`：

- Off：只保留功能必需状态；
- Summary：默认显示 phase、elapsed、source kind 和终态；
- Trace：显示 jobs、priority、每阶段证据与 diagnostics。

## 14. Diagnostics

新增或细分稳定诊断码：

```text
project_editor_composition.qos_policy_invalid
project_editor_composition.qos_system_facts_unavailable
project_editor_composition.priority_apply_failed
project_editor_composition.cancelled
project_editor_composition.process_tree_termination_failed
project_editor_composition.worker_join_failed
project_editor_composition.promotion_authority_missing
project_editor_composition.promotion_source_outside_authorized_root
project_editor_composition.promotion_source_untrusted_path
project_editor_composition.promotion_identity_mismatch
project_editor_composition.promotion_executable_hash_mismatch
project_editor_composition.promotion_qualification_mismatch
project_editor_composition.promotion_copy_verification_failed
project_editor_composition.promotion_publish_failed
project_editor_composition.promotion_rollback_failed
```

既有 `cache_invalidated`、`build_step_failed`、`descriptor_query_failed` 与 handoff diagnostics 继续保留。
诊断必须包含 stage、typed facts 和 next action，不允许只输出“构建失败”。

## 15. 正向资格矩阵

| Scenario | 预期结果 |
|---|---|
| destination 已有 exact 有效 artifact | `exact_cache` hit，不启动 Cargo/rustc |
| authorized run root 存在 exact qualified artifact | `exact_promotion`，事务发布后正常 loader hit |
| 没有 candidate，真实 cache miss | `controlled_build`，当前 12 线程机器预期 `jobs=4` |
| 构建中拖动/缩放 Editor | input 立即响应，progress redraw 不超过策略上限 |
| 构建中关闭 Editor | cancel terminal，owned process tree 结束，worker joined |
| promotion 目标旧条目存在 | 备份、发布、正常 loader 验证，或失败回滚 |
| promotion 与 normal open 连接 | 普通 Editor exact hit，不再用用户首次打开支付冷构建 |

## 16. 否定矩阵

| Scenario | 必须结果 |
|---|---|
| identity 任一字段不同 | 拒绝 promotion，不得 compatibility fallback |
| descriptor 哈希与文件不同 | 拒绝，不写 destination |
| qualification evidence 不绑定同一 executable | 拒绝 |
| source path 逃逸 authorized run root | fail closed |
| destination staging 复制后哈希变化 | 拒绝发布，保留旧缓存 |
| priority 设置失败 | 如实报告；按 policy 决定 fail closed 或 jobs=1 保守继续 |
| cancel 后 Cargo/rustc 仍存在 | Gate 失败，不得声明 cleanup complete |
| worker 存在但无状态变更 | 不得无限 request_redraw |
| 普通 Editor 自动扫描 fresh roots | Gate 失败 |
| cache 存在但 trust receipt stale | trust fail closed，不启动 composition |

## 17. 验收与验证方向

本节只定义后续施工文档必须覆盖的结果，不构成当前测试授权。

### 17.1 C1 owner 验证

- jobs 决策覆盖 1/2/4/8/12/32 logical processors、低内存和系统 facts 不可用；
- scripted process Adapter 证明参数、priority、timeout、cancel、tree termination、reader join 与单一终态；
- composition application worker 证明 duplicate prepare exactly-once，Drop/CloseRequested cancel + join；
- redraw scheduler 证明 build idle 节拍上限，输入 redraw 不被 throttle；
- BuildReport v2 roundtrip 与 v1 read compatibility。

### 17.2 C2 owner 验证

- exact candidate 校验与 promotion no-op hit；
- identity/hash/qualification/path containment 否定矩阵；
- cross-volume copy -> destination staging -> re-hash -> atomic publish；
- pre-existing destination 备份与注入失败后回滚；
- normal loader 只从最终 destination 重新读取，不相信内存中的 candidate object。

### 17.3 受影响 consumer

```text
editor_core composition artifact/cache owner
editor_window_winit production composition preparer
NativeEditorApplication worker lifecycle
real_window redraw scheduler / CloseRequested
production qualification report reader if schema binding is deepened
```

### 17.4 真实 Windows 验收方向

在后续单独授权下，使用新 fresh root 和 Tower：

1. 一次真实 controlled cold build，证明 jobs/priority 生效、窗口始终 Responding。
2. 构建期间真实 drag/resize/input smoke；目标为 UI message P95 `< 50 ms`、resize P95 `< 100 ms`。
3. 一次构建期间 CloseRequested，证明无 Cargo/rustc/composition 遗留进程。
4. 一次 run-owned exact artifact -> disposable destination cache promotion，证明 backup/rollback 否定矩阵。
5. 真实用户缓存和普通 production Editor 只能在再次明确授权后触发；方案/源码 Gate 不自动执行。

真实冷构建总耗时不是 C1 的唯一阻断指标；限制并发可能使 wall time 略增加，但必须换来
可用的 Editor。C2 的目标是让有 exact qualified artifact 的普通打开根本不进入该冷构建。

## 18. 预计涉及文件

以下只是后续施工范围评估，不是当前修改清单：

```text
rust/crates/editor_core/src/project_editor_composition.rs
rust/crates/editor_core/src/project_editor_composition_artifact.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_window_winit/src/project_editor_composition_production.rs
rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_window_winit/src/tests/project_editor_composition_production_gate.rs
```

若 shared bounded child process 缺少可取消 process-tree 能力，可在其现有 owner 内深化，但不得为 composition
复制第二套 Windows child process implementation。施工文档必须先根据实际 owner 收窄文件清单。

## 19. 与已有方案的关系

### 19.1 262 Trusted ProjectRust Editor Composition Artifact

282 只深化其 cache lifecycle 和 build process execution。identity、trust、artifact seal、descriptor、cache
isolation 与 handoff 均保持。

### 19.2 268 Project Open Responsiveness + Progress

282 将同样的 cancel/join 标准补到 composition worker，并把 progress redraw 从无界轮转收敛到事件+时钟节拍。

### 19.3 269 Project Open + Editor Play Latency Convergence

282 不重建 compatibility cache，直接复用 269 的 `ct/<compatibility-key>`。final exact artifact 与
compatible Cargo intermediate 仍是两层不同真相。

### 19.4 274 Construction Validation Plan v2

后续施工文档按影响面选择 owner/consumer/真实 Windows 证据，不因涉及 production 概念就自动运行
Local CI 或完整视觉矩阵。

### 19.5 278 Production Authority Finalization

CloseRequested 的 composition build cancellation 应与 278 的 exactly-once terminal finalization 顺序协同，
但 282 不改写 278 report 的业务语义。

## 20. 施工窗口建议

本节只作为生成施工文档时的拆分基线，不代表已授权。

```text
Window A / C1
  Gate A: Build QoS schema, policy resolver, process Adapter red tests
  Gate B: bounded jobs, Windows priority, cancellable process tree
  Gate C: composition worker cancel/join + event/timer-driven redraw
  Gate D: affected owner/consumer + real cold-build responsiveness qualification

Window B / C2
  Gate E: promotion schema, authority/path/identity/hash/qualification red tests
  Gate F: destination staging, backup, atomic publish, rollback
  Gate G: disposable promotion matrix + normal loader exact-hit integration
  Gate H: separately authorized production consistency closure and documentation archive
```

Window A 完成后已能保证不可避免构建时 Editor 可用；Window B 负责让多数已有 exact qualified
artifact 的 production update 不再把冷构建转嫁给用户。

## 21. 回滚与停止条件

### 21.1 回滚粒度

```text
QoS policy / resolver
process priority + process-tree ownership
composition worker cancellation
redraw scheduling
promotion validation
promotion publish/rollback
schema/report/UI phase
```

各粒度必须能独立回退，不要用“放宽 identity 恢复 hit”作为回滚方案。

### 21.2 必须停止

- 需要放宽 exact identity / descriptor / executable hash 才能 promotion；
- qualification 证据无法绑定实际 executable；
- 需要自动扫描未授权目录或修改真实配置；
- Windows 上 cancel 不能终止完整 owned Cargo/rustc/linker tree；
- 缓存替换失败无法恢复旧 destination；
- 为了节流 progress 而连同用户输入或 GameView tick 一起节流；
- 方案需要新建通用 build daemon / workflow engine 才能继续。

## 22. 方案自审

### 22.1 是否真正解决重复卡顿

通过。C2 消除“已有合格产物却仍在普通 Editor 重建”；C1 保证确实需要重建时不饥饿 UI。
两者缺一都只能解决半个问题。

### 22.2 是否破坏 262/269 信任与缓存合同

通过。final artifact 仍 exact，compatibility identity 仍只作 Cargo intermediate 复用；promotion 在正常 loader 前后
各做一次完整校验，trust receipt 不由 promotion 产生或更改。

### 22.3 Module 是否足够深

通过。caller 只学习 `prepare` 或授权 maintenance 下的 `promote_exact`；jobs、priority、process tree、
copy/backup/publish/rollback 和 report 均在 owner implementation 内。System/Scripted 两个 Adapter 使 process seam 真实且可测。

### 22.4 是否不必要地扩大范围

通过。不改 gameplay/render/runtime package，不引入 daemon/远程 cache，不把 stable ABI digest 混入 v1，
不自动替换 production 二进制或修改真实配置。

### 22.5 是否可验证、可回滚、AI 可审查

通过。typed policy/report、单一 owner、确定性 scripted Adapter、否定矩阵、事务备份和 retained reason
使每个终态都能被审查。

### 22.6 外部审查文档

本次没有指定与 282 相匹配的其它 AI 审查文档。无需因外部审查修改正式方案。

### 22.7 自审结论

```text
方案自审：通过
必须修改正式方案：无
施工文档状态：已激活并完成 Gate A-G；Gate H 未通过，未归档
可以自动施工：否
当前施工状态：停止在 Gate H 资格化前；等待 282-R1 repair 施工文档
```

## 23. 下一步

282 Window A / Gate A-D 与 Window B / Gate E-G 已完成。Window C / Gate H 于 2026-08-11
单独授权后，在任何 production mutation 前因 exact composition qualification release build 两次
600 秒 timeout 停止；跨 root compilation target 也不能形成 portable hit。production Editor、真实
Tower cache 与真实配置均未改变，282 施工文档继续保留在 `施工文档/当前/`，不得归档或直接重跑
Gate H。

后续最小 repair 已在
`282-R1-Generated-Lock-Lineage-Path-Affine-Compilation-Cache-Tiered-Build-Deadline-v1方案.md`
按方案 B 确认并通过自审。下一步是单独生成并自审 282-R1 repair 施工文档；该文档应作为当前
282 lineage 的受控 repair window，而不是并发占用第二个施工槽。在获得新的施工授权前，不修改源码、
不运行 repair Gate 或 Local CI、不写真实缓存、不替换 production/installed 二进制、不修改真实配置，
也不重跑 Gate H。
