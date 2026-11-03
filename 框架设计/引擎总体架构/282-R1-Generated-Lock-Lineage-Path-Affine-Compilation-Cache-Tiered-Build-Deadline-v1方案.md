# 282-R1 Generated Lock Lineage + Path-Affine Compilation Cache + Tiered Build Deadline v1 方案

## 1. 文档状态

```text
系统编号：282-R1
父系统：282 Project Editor Composition Cache Lifecycle + Build QoS v1
问题来源：282 Window C / Gate H exact composition qualification failure
用户选择：方案 B
当前状态：正式方案已确认；方案自审通过；施工文档已生成并自审，位于待执行、未激活
日期：2026-08-12
```

本文是 282 的最小 repair，不建立第二套 composition builder。它只深化现有
`ProjectEditorCompositionArtifact::prepare` owner，补齐 generated lock lineage、Cargo compilation
cache 的 path-affinity 真相和 release build 的分层 deadline。282 Gate H 在 repair 完成前不得直接重跑。

## 2. 问题与结论

282 Gate H 在 production mutation 前停止。production Editor、真实 composition cache、真实配置、
Player/MCP 与其它 installed binary 均未改变。失败不是单一的“机器慢”，而是三个合同缺口叠加：

1. `GeneratedEditor/Cargo.lock` 每次由 `cargo generate-lockfile --offline` 重新解析，但 269 compatibility
   key 只绑定 Engine SDK `Cargo.lock`，没有绑定实际 generated resolution。
2. `CARGO_TARGET_DIR` 位于 run-owned absolute root；Cargo/rustc intermediate 含 path-sensitive fingerprint。
   相同 compatibility key 只能证明依赖语义可能相容，不能证明 target directory 可跨 root 复制复用。
3. `build_composition_release` 使用固定 600 秒 hard timeout。底层能区分 `Timeout / Cancelled /
   WaitFailed`，上层却折叠为 `build_step_failed`，且 timeout cleanup 没有提升为顶层证据。

真实证据：

```text
Gate G compatibility key：554a84eec277b25aed63bca5df81f79d
Gate H compatibility key：554a84eec277b25aed63bca5df81f79d
attempt 1：jobs=2，600015 ms，timeout
attempt 2：jobs=1，600004 ms，timeout
跨 root copy：因 absolute target path 重编
原 Gate G target root：因 generated resolution drift 补编依赖
```

因此方案 B 的正式结论是：

```text
Generated lock 是可移植、可封印的小型 lineage artifact；
Cargo target intermediate 是 path-affine、不可跨 root 提升的派生缓存；
release build 使用 soft budget + hard deadline，不能把超 soft budget 等同于 hang。
```

## 3. 目标与非目标

### 3.1 目标

1. 相同 generated build 输入必须得到可审计的 lock lineage；实际 resolved dependency graph 进入
   compilation compatibility identity。
2. exact composition artifact 必须绑定 requested identity、lock lineage 与 executable hash，不允许同一
   requested identity 暗含不同 generated resolution。
3. compilation cache 明确报告 `same_root_hit` 或 `path_affine_miss`；跨 root copy 永不声明 portable hit。
4. `generate lock / release build / descriptor query` 使用各自 deadline；release build 超过 soft budget
   只告警，超过 hard deadline 才终止。
5. Timeout、Cancel、WaitFailed、non-zero exit 分别形成稳定 diagnostic，并如实提升 process-tree cleanup。
6. 保持 normal Editor caller 的小 interface；所有复杂度留在现有 artifact owner 内。

### 3.2 非目标

- 不放宽 262 trust、ProjectRust dependency policy、descriptor、executable hash 或 qualification 要求。
- 不自动扫描 fresh run root，不自动导入外部 lock lineage 或 exact artifact。
- 不建立全局 build daemon、远程 cache、sccache、分布式编译或通用 workflow engine。
- 不宣称 Cargo target、incremental object、PDB、rmeta 或 build-script output 跨 absolute root 可移植。
- 不修改 Cargo 全局配置、项目 `.cargo/config.toml`、Tower gameplay/AUI/RuntimePackage 或真实配置。
- 不替换 production/installed binary，不写真实用户 cache，不运行 Local CI，不重跑 Gate H。
- 不做 CPU/activity sampling 型“hang detector”；超过时间阈值本身不足以证明 hang。

## 4. 不可破坏的不变量

### 4.1 Requested identity 不放宽

`ProjectEditorCompositionIdentity.v1` 继续绑定现有 Engine/Editor/project/AOT/toolchain/target/profile/
manifest/dependency/trusted lock 输入。R1 不删除字段、不忽略字段，也不把 compatibility hit 当成 exact hit。

### 4.2 Final artifact exactness 加强

最终 artifact 的 resolved identity 新增内部复合真相：

```text
requestedIdentityDigest
+ generatedLockLineageDigest
+ generatedResolvedGraphDigest
+ descriptor/executable identity
-> resolvedArtifactKeyDigest
```

`requestedIdentityDigest` 相同但 generated lineage 不同的两个产物不是同一个 exact artifact。旧
`ProjectEditorCompositionIdentity.v1` 仍可用于 trust、handoff 和项目语义；cache descriptor、qualification
seal 与 promotion 必须额外绑定 resolved identity。这里是加强 exactness，不是放宽或替换 262 identity。

### 4.3 Trust 与 authority 不变

- lock lineage 不产生或刷新 ProjectRuntime trust receipt；
- normal Editor 只能读取 application-owned lineage/cache；
- 外部 lineage/artifact 只有明确授权的 maintenance path 才能导入；
- promotion 仍要求 authorized run root、containment、qualification、双重 hash、backup、publish 与 rollback。

## 5. 正式架构

```text
Normal Editor / Qualification caller
  -> prepare(request, control)
       -> staging plan + requested identity validation
       -> GeneratedCompositionLockLineageModule
            -> derive lockInputDigest
            -> load/verify sealed lineage OR resolve once
            -> raw Cargo.lock digest
            -> normalized resolved graph digest
            -> atomic lineage publish
       -> CompilationCachePathAffinityModule
            -> portable compatibility digest
            -> canonical target-root binding
            -> same-root target selection
       -> CompositionBuildDeadlineModule
            -> lock hard deadline
            -> release soft budget + hard deadline
            -> descriptor hard deadline
       -> Cargo/rustc through existing bounded process owner
       -> descriptor + resolved identity + executable hash
       -> exact artifact publish
       -> BuildReport v3
```

这些是 `ProjectEditorCompositionArtifact` 的内部模块，不新增 normal caller 可见的 orchestration interface。
删除该 owner 后，上述复杂度会散回 production preparer、qualification 和 maintenance caller，因此它继续是
一个有实际 Depth 的 module。

## 6. Generated Lock Lineage

### 6.1 Lineage 输入

新增 `GeneratedCompositionLockLineage.v1`。`lockInputDigest` 只由稳定、可移植输入计算：

```text
schema/tool version
Cargo version + rustc/toolchain identity
target triple + profile + fixed generated feature set
normalized GeneratedEditor manifest template digest
normalized RuntimeModuleBuild manifest digest
normalized dependency identity digest
Engine SDK Cargo.lock digest
trusted Engine crate manifest-set digest
```

不得把 staging root、target root、timestamp、PID、sequence、wall clock 或其它 absolute run path 放入
`lockInputDigest`。

### 6.2 两种 digest 不得混淆

lineage 同时保存：

```text
rawLockDigest
  = GeneratedEditor/Cargo.lock 原始 bytes 的 SHA-256
  = 用于 byte-exact seal、copy verification 和审计

resolvedGraphDigest
  = Cargo.lock 解析后的 canonical dependency graph digest
  = 用于 compilation compatibility
```

`resolvedGraphDigest` 的 canonical graph：

- registry/git package 绑定 name、version、canonical source、checksum、sorted dependency edges；
- Engine/path package 绑定 package name、version、features 与 trusted source/manifests digest；
- 去除 generated root package 的 exact 名称；
- path source 不记录 absolute filesystem path；
- dependency node 与 edge 均稳定排序；
- duplicate name/version/source 必须通过完整 package identity 消歧；
- parse ambiguity、缺 checksum、unsupported source 或 graph inconsistency 一律 fail closed。

不能直接用 `rawLockDigest` 作为 compatibility digest，因为 generated root package name 随 exact
composition identity 改变；也不能只用 Engine SDK lock digest，因为实际 generated graph 是它的受控子图。

### 6.3 产生与复用

application-owned build root 增加：

```text
project-editor-compositions/
  locks/<lock-input-digest>/
    Cargo.lock
    lineage.json
```

规则：

1. lineage hit 时先校验 schema、input digest、raw bytes digest 与 canonical graph digest，再复制到 staging；
2. lineage miss 时才运行 `cargo generate-lockfile --offline`；
3. 解析并封印生成结果后，通过 destination-owned staging + atomic rename 发布 lineage；
4. 同 key 已存在且 exact valid 时 no-op；不同 bytes/graph 时报告 collision，不覆盖；
5. lineage 可在明确授权下 byte-copy 到另一 fresh root，因为其 schema 不含 absolute path；复制后必须重算全部 digest；
6. normal Editor 不搜索其它 root，也不从旧 `ct` 目录反推 lineage。

## 7. Path-Affine Compilation Cache

### 7.1 Compatibility v2

`project-editor-composition-compilation-cache.v2` 的 portable compatibility digest 绑定：

```text
schema version
toolchain + Cargo identity
target triple + profile + generated feature set
normalized generated manifest template digest
normalized dependency identity digest
resolvedGraphDigest
trusted Engine crate manifest-set digest
```

editor build hash、project id 与 AOT source digest 仍不进入 compatibility digest；Cargo 自己的 fingerprint
决定哪些 path crate 需要重新编译。它们仍进入 final exact artifact identity。

### 7.2 Root binding

目录定位不能直接使用“最终 target root 自身 digest”作为该 root 的目录名，否则目录名会参与被摘要路径，
形成自引用。v1 因此分开计算 owner anchor 与最终 target root：

```text
canonicalTargetAnchorDigest = sha256(canonical absolute application cache root UTF-8 form)
canonicalTargetRootDigest = sha256(canonical absolute final target root UTF-8 form)
```

目录布局为：

```text
ct/<compatibility-digest>/<canonical-target-anchor-digest-prefix>/
  affinity.json
  release/...
```

`affinity.json` 保存完整 compatibility digest、完整 canonical anchor digest、完整 canonical final target root
digest、schema 与创建工具身份。只有三者都匹配才是 `same_root_hit`。复制整个 `ct` 到新 root 后，旧
affinity 子目录保留为不可用历史
intermediate；owner 为新 canonical root 创建新子目录并报告 `path_affine_miss`，不把旧目录交给 Cargo。

### 7.3 Portability 状态

BuildReport 必须输出：

```text
compilationCacheCompatibilityDigest
compilationCacheAffinity = same_root_hit | path_affine_miss | cold
canonicalTargetAnchorDigest
canonicalTargetRootDigest
crossRootPortable = false
```

任何 promotion、copy 或文档都不得把 `ct` 描述为 portable cache。可移植对象只有 sealed lock lineage 和
最终 exact qualified artifact。

### 7.4 v1 迁移

旧 `ct/<v1-key>` 不迁移、不复制、不删除，也不作为 v2 hit。writer 只创建 v2 layout；旧目录由现有容量/
维护策略以后处理，本 repair 不做真实 cache cleanup。

## 8. Tiered Build Deadline

### 8.1 Typed policy

新增 `ProjectEditorCompositionBuildDeadlinePolicy.v1`：

```text
generateLockHardDeadlineMs       60000
releaseBuildSoftBudgetMs        600000
releaseBuildHardDeadlineMs     1200000
descriptorQueryHardDeadlineMs    30000
```

合同要求：

- 所有值非零；
- release soft budget 必须小于 hard deadline；
- hard deadline 仍受 construction 三小时总窗口约束；
- caller 不以散落的 `min(600_000)` 覆盖 typed policy；
- user cancel / Editor CloseRequested 始终优先于任何 deadline。

### 8.2 Soft budget 语义

超过 release soft budget：

- 不终止 Cargo/rustc；
- report 设置 `softBudgetExceeded=true` 与首次越界 elapsed；
- Editor Summary 显示“仍在编译，可取消”，不得显示 hang 或 failure；
- 完成后 exit 0、reader joined、descriptor/seal 成功仍算正常 success。

### 8.3 Hard deadline 语义

超过 hard deadline：

```text
request owned process-tree termination
-> wait root
-> release Job Object / process group
-> join stdout/stderr readers
-> preserve bounded output
-> cleanup staging only when ownership cleanup confirmed
-> terminal release_build_hard_timeout
```

hard timeout 说明“超出产品允许的最长构建时间”，不自动说明 rustc hang。真正的 stall/activity detector
需要额外 OS/process telemetry，不属于本 repair。

## 9. Terminal 与 Evidence 语义

上层必须保持底层 terminal，不再统一折叠：

| Process result | Composition diagnostic | cancellationRequested | cleanup evidence |
|---|---|---:|---|
| Completed + exit 0 | success | false | normal wait/release/join |
| Failed/non-zero | `build_step_failed` | false | normal wait/release/join |
| Cancelled | `cancelled` | true | terminate/wait/release/join |
| Timeout | `release_build_hard_timeout` 或对应 step timeout | false | terminate/wait/release/join |
| WaitFailed | `build_process_wait_failed` | false | terminate/wait/release/join |
| SpawnFailed | `build_process_spawn_failed` | false | no child or owned cleanup |

`processTreeTerminated` 不只服务 Cancelled。凡 termination requested，顶层都必须从
`owned_process_cleanup_confirmed()` 提升结果；同时增加 `outputReadersJoined`、`rootWaitCompleted`、
`processGroupReleased`。不能把 terminated、natural completed 和 cleanup confirmed 混成一个 bool。

## 10. Schema 与缓存键

### 10.1 新 schema

```text
generated-composition-lock-lineage.v1
project-editor-composition-resolved-identity.v1
project-editor-composition-build-deadline-policy.v1
project-editor-composition-build-request.v3
project-editor-composition-build-report.v3
project-editor-composition-artifact.v2
project-editor-composition-descriptor.v2
project-editor-composition-qualification-seal.v2
project-editor-composition-promotion-request.v2
project-editor-composition-promotion-report.v2
project-editor-composition-compilation-cache.v2
```

`ResolvedIdentity.v1` 至少包含 requested identity digest、lineage digest、resolved graph digest 与
resolved artifact key digest。descriptor、artifact、qualification seal 和 promotion request/report 必须一致。

### 10.2 读取迁移

- BuildReport reader 继续只读 v1/v2；writer 只写 v3；
- BuildRequest normal caller writer 只写 v3；历史 v1/v2 仅测试/历史 report 读取，不用于新 promotion；
- Artifact/Descriptor v1 可按原 exact key 读取，但不能作为 R1 v2 promotion source；
- Qualification/Promotion v1 作为历史证据保留，不追溯改写；
- v2 exact artifact cache miss 时走正常 lineage/build，不原地“升级”旧 artifact；
- 所有 migration 都 fail closed，不通过 default 空 digest 伪造 v2。

## 11. Diagnostics

新增稳定诊断：

```text
project_editor_composition.lock_lineage_input_mismatch
project_editor_composition.lock_lineage_raw_digest_mismatch
project_editor_composition.lock_lineage_graph_digest_mismatch
project_editor_composition.lock_lineage_collision
project_editor_composition.lock_lineage_publish_failed
project_editor_composition.compilation_cache_path_affine_miss
project_editor_composition.compilation_cache_affinity_invalid
project_editor_composition.release_build_soft_budget_exceeded
project_editor_composition.release_build_hard_timeout
project_editor_composition.build_process_spawn_failed
project_editor_composition.build_process_wait_failed
project_editor_composition.process_tree_cleanup_unconfirmed
project_editor_composition.resolved_identity_mismatch
project_editor_composition.promotion_lineage_mismatch
```

`path_affine_miss` 与 `soft_budget_exceeded` 默认是 warning/evidence，不单独使 build failed。

## 12. Owner 与影响面

预期源码 owner：

```text
rust/crates/editor_core/src/project_editor_composition.rs
rust/crates/editor_core/src/project_editor_composition_artifact.rs
rust/crates/editor_core/src/project_editor_composition_cache_promotion.rs
rust/crates/editor_window_winit/src/project_editor_composition_production.rs
rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs
```

只有在现有 `BoundedChildProcessResult` 无法表达所需 terminal/cleanup 字段时，才允许窄改：

```text
rust/crates/runtime_cli/src/bounded_child_process.rs
```

不新增 generic lock daemon、cache manager crate 或 construction runner。测试 seam 继续使用既有 scripted Cargo/
process adapter；不要为单个测试把内部步骤暴露成 public interface。

## 13. 验证要求

### 13.1 Owner red tests

- 同 input 解析出相同 lineage，raw bytes tamper 与 graph tamper 分别失败；
- generated root package 名变化不改变 resolved graph digest；registry version/checksum/edge 变化必须改变；
- absolute path 变化不改变 lock lineage digest；
- same root 命中同 affinity，跨 root copy 必须 `path_affine_miss` 且不消费旧 intermediate；
- v1 `ct` 不被 v2 writer 复用；
- soft budget 越界后 exit 0 仍成功；hard deadline、cancel、wait failure 均 kill/reap/join 并保留独立 terminal；
- timeout cleanup confirmed 时顶层 evidence 不再错误为 false；
- descriptor/seal/promotion 任一 lineage mismatch 均 fail closed。

### 13.2 Source-only integration

使用 repository-external fresh role roots，至少证明：

1. root A 产生 sealed lineage + v2 path-affine target + exact artifact；
2. 只复制 lineage 到 root B，lock hit、target cold、release build 成功；只复制 lineage 时没有旧 target
   affinity 可供 owner 判定跨 root miss，禁止伪造 `path_affine_miss`；
3. 将 root A 的旧 `ct` 复制到独立 `copied-ct-negative` build root，必须报告 `path_affine_miss`，且旧
   intermediate 不能形成 `same_root_hit` 或被 Cargo 消费；
4. controlled slow process 跨 soft budget 后完成；controlled over-hard process 被终止且无 owned process；
5. qualification seal 和 promotion source 精确绑定 resolved identity。

此阶段不触碰 production Editor、真实用户 cache 或真实配置，也不需要完整视觉矩阵。

### 13.3 Gate H 续跑资格

只有 source-only owner/consumer closure 和 fresh integration 通过后，才可由用户单独授权新的 Gate H：

- 新 fresh root；
- 新 production candidate hash；
- 新 v2 exact artifact/qualification seal；
- 原 production binary/cache/config preflight；
- 事务化 replacement/promotion/normal Editor smoke；
- 失败仍在 production mutation 前 fail-fast。

旧 Gate H authorization 已结束，不能由本方案选择自动恢复。

## 14. 建议施工拆分

本节只供后续施工文档使用，不代表施工授权：

```text
Repair Window A
  Gate R-A: lock lineage + canonical graph + resolved identity schema
  Gate R-B: path-affine compatibility v2 + v1 isolation
  Gate R-C: deadline policy + terminal/cleanup report semantics
  Gate R-D: affected owner/consumer closure

Repair Window B
  Gate R-E: fresh root A/B lineage portability and path-affinity integration
  Gate R-F: qualification/promotion resolved-identity closure
  Gate R-G: repair completion record and Gate H re-entry preflight
```

Repair Window 不替换 production Editor，不写真实 cache。完成 R-A 至 R-G 后，282 仍不自动进入 Gate H。

## 15. 停止条件

- 需要删除/忽略 exact identity 字段才能复用；
- 需要把 absolute target output 声称为 portable；
- canonical graph 无法消除 path/root package 噪声而不丢失 dependency identity；
- lineage 不能绑定实际 Cargo.lock bytes；
- soft budget 被实现为隐式 kill；
- hard timeout 或 cancel 后不能证明 owned process cleanup；
- 需要修改项目/全局 Cargo config、真实配置或 production state；
- 需要新 daemon、远程 cache、稳定 ABI redesign 或其它超出最小 repair 的能力。

## 16. 方案自审

### 16.1 是否覆盖三个已确认缺口

通过。lineage 绑定实际 generated resolution；path affinity 取消错误的跨 root portability 承诺；typed
soft/hard deadline 区分慢编译与真正的产品 hard stop。

### 16.2 是否保持深 Module

通过。caller 仍只调用 `prepare` 或既有 authorized `promote_exact`。lineage resolve、graph canonicalization、
affinity selection、deadline 与 terminal mapping 均隐藏在 artifact owner implementation 内。

### 16.3 是否放宽 exact/trust/authority

没有。ResolvedIdentity 使 final exactness 更强；trust 与 promotion authority 不变；normal Editor 不获得
外部 lineage 扫描或 promotion 权限。

### 16.4 是否错误承诺跨 root target portability

没有。`ct` 明确 `crossRootPortable=false`；只有 lock lineage 与 final exact qualified artifact 可在明确授权、
重新校验后跨 root 使用。

### 16.5 Deadline 是否掩盖 hang

没有。soft budget 只告警，hard deadline 表达产品上限，不声称诊断出 hang；用户取消始终优先。

### 16.6 是否扩大施工范围

没有。不改 Tower、RuntimePackage、Editor UI 大结构、Local CI、production binary/cache/config；不新增 daemon、
workflow 或远程 cache。

### 16.7 外部审查

本轮没有用户指定的 282-R1 外部审查文档。方案依据 282 Gate H 保留证据、现有 owner code、Cargo lock/
target 行为和项目施工规则收敛，无需引入其它系统。

### 16.8 自审结论

```text
方案自审：通过
选定方案：方案 B
正式方案需再次修订：否
施工文档状态：已生成并自审；位于待执行、不可施工
可以修改代码或重跑 Gate H：否
```

## 17. 下一步

282-R1 施工文档已生成、自审并进入 `施工文档/待执行/`。下一步是由用户单独授权
`282-R1 Repair Window A / Gate R-A 至 R-D`，随后执行激活前复核。因为 282 原施工文档仍占用
`施工文档/当前/`，激活时必须将 R1 Gate 合入唯一的 282 当前文档，并把独立 R1 计划移入历史，不能
并发启动第二份当前施工。在完成该事务前不修改代码、不运行 repair Gate、不运行 Local CI、不替换
production/installed binary、不写真实 cache、不修改真实配置，也不重跑 Gate H。
