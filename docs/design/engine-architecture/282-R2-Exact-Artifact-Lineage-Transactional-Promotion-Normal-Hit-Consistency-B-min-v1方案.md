# 282-R2 Exact Artifact + Lineage Transactional Promotion + Normal Hit Consistency B-min v1 方案

## 1. 文档状态

```text
系统编号：282-R2
父系统：282 Project Editor Composition Cache Lifecycle + Build QoS v1
前置 repair：282-R1 Generated Lock Lineage + Path-Affine Compilation Cache + Tiered Build Deadline v1
问题来源：282 新 Gate H promotion 后 normal loader 未达到 exact_cache / steps=0
用户选择：方案 B-min
当前状态：审查后范围已收缩；原 R2-A 至 R2-C 冻结，当前只保留 harness identity 最小修复
日期：2026-08-12
```

本文是 282 的第二个最小 repair 方案。它不建立第二套 cache manager，不改变 normal loader 的查找算法，
也不恢复已经结束的 Gate H 授权。后续 evidence review 已确认当前 repair 只有一个已证实合同缺口：

1. Gate H qualification harness 的 normal-hit 负向测试错误修改了 `cargo_identity`，导致
   `GeneratedCompositionLockInput.lockInputDigest` 合法变化。

原文把“既有 `promote_exact` 未通用事务化 lineage”也列为本次必修。保留的 Gate H evidence 已证明正确
lineage 当时已经被外层授权流程放入真实 cache，并在失败后移回 `rollback-retained/locks/...`；normal request
只是因为错误 `cargo_identity` 计算了另一个 key。因此 promotion v3 与 artifact+lineage 通用事务不是本次
失败的必要修复，现按第 18 节 scope correction 冻结为独立 deferred enhancement。

282 当前施工文档继续保留且不得归档。后续若要施工，必须另行生成并自审 282-R2 B-min repair 施工文档，
再由用户按窗口明确激活；本文本身不构成代码、production、真实 cache/config 或 Gate H 授权。

## 2. 失败事实与纠正后的诊断

### 2.1 Gate H 已完成的事实

最新 fresh root：

```text
<PRODUCTION_RUN_ROOT>\282-r1-gate-h-20260812-150654
```

关键身份：

```text
candidate hash：5007CDA6E32036D2F5FAD5A4CBD58B6A1AA9ED20E21E23DC905A66D42A5B9BC2
requested identity：sha256:4142b1e905ebeebcd174f3660a821bf72602af2ccb8c661de58921d22baca056
resolved graph：sha256:d7150c7cabaeb067a1ebb69b73c1203820f878361779ea3a1b1ab1b7d564c0af
artifact key：sha256:0b4ed21bbbadd92cea528bfe849e9c7cc999face98fc02588e8c339896a482aa
cold release build：1186097 ms，越过 soft budget 后在 hard deadline 内完成
qualification：open/play/pause/step/resume/stop passed
promotion：promote_exact published
normal loader：进入 generate_composition_lock，未达到 exact_cache / steps=0
rollback：production Editor、真实 cache、配置、recent、trust 与 owned process 全部恢复
```

失败记录：

```text
阶段完成记录/2026-08-11-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/05-Gate-H.md
```

### 2.2 第一直接原因：harness identity 不一致

Gate H source build request 使用：

```rust
cargo_identity: "cargo 282-gate-h-candidate"
```

normal-hit request 同时改了 executable 与 identity：

```rust
cargo_executable: Some(run_root.join("must-not-start-cargo.exe")),
cargo_identity: "cargo 282-gate-h-must-not-run"
```

`cargo_identity` 是 `GeneratedCompositionLockInput` 的稳定输入，并参与 `lockInputDigest`。因此 normal loader
寻找不同 lineage key 是合同规定的正确行为，不是 loader 算法失效。用不存在的 Cargo 路径证明
`steps=0` 时，只允许替换 `cargo_executable`；必须保留 source request 的真实 `cargo_identity`。

### 2.3 第二合同缺口：promotion 没有成对发布 lineage

即使修正 harness，production destination 的 normal prepare 也必须同时拥有：

```text
exact qualified artifact
+ 与其 resolved identity 精确绑定的 generated Cargo.lock
+ 同一 generated-lock lineage.json
```

R1 已证明 lineage 是 normal prepare 的 authority 输入。若 maintenance promotion 只发布 artifact，而 lineage
依赖偶然预热、旧 destination 状态或另一次 build，则 promotion report 不能证明普通 Editor 能 deterministic
地得到 `exact_cache / steps=0`。B-min 因此只深化现有 `promote_exact`，把单个 exact artifact 与其单个 exact
lineage 作为同一事务单元。

### 2.4 不得据此推断的结论

- 当前没有证据要求修改 normal-loader lookup 顺序或 exact-cache 算法；
- 当前没有证据允许 artifact-first lookup 绕过 generated-lock lineage；
- 不能通过 warm-up build、fallback generate-lock 或预填真实 cache 掩盖 lineage promotion 缺口；
- harness 身份错误与 promotion 权限不完整是两个独立问题，修正前者不能替代后者。

## 3. 目标

1. qualification normal-hit request 与 source build request 保持完全一致的语义身份，只用不存在的
   `cargo_executable` 作为“Cargo 不得启动”的执行探针。
2. `promote_exact` 在一次授权事务中发布一个 exact qualified artifact 及其唯一 exact generated-lock lineage。
3. promotion 前从 source `Cargo.lock + lineage.json` 重新计算并验证 lock input、raw lock、resolved graph、
   lineage 与 artifact 的全链摘要，拒绝只相信 report 字段。
4. artifact 与 lineage 必须共同 backup、publish、verify、commit；任一阶段失败时共同 rollback。
5. report 必须能回答 source、copied、final 三阶段两个对象的 hash、路径、动作与 rollback/retained 状态。
6. promotion 后 destination normal `prepare` 必须以 `exact_cache`、`steps=0` 完成，并证明不存在的 Cargo
   executable 从未启动。
7. 保持 R1 requested identity、resolved exactness、trust receipt、qualification seal 和 destination containment
   全部不放宽。

## 4. 非目标

- 不重写 `ProjectEditorCompositionArtifact::prepare` 或 normal-loader 算法。
- 不建立第二个 promotion/cache/lineage manager，不新增通用 workflow 或 daemon。
- 不做 artifact-first lookup，不允许 artifact 绕过 lineage validation。
- 不导入整个 run-owned cache，不提升 path-affine `ct` compilation intermediate。
- 不用 warm-up build 或 generate-lock fallback 把 miss 伪装成 normal hit。
- 不改变 stable ABI、ProjectRuntime trust、dependency policy、AUI、Tower gameplay 或真实项目配置。
- 不替换 production/installed binary，不写真实 cache，不运行 Local CI，不重跑 Gate H。
- 不把 v1/v2 历史 promotion request 自动解释为拥有 lineage mutation authority。

## 5. 不可破坏的不变量

### 5.1 Identity consistency

source build、qualification、promotion 与 destination normal-hit request 必须共享下列语义输入：

```text
cargo_identity
rustc/toolchain identity
target/profile/features
generated manifest inputs
dependency identity
Engine SDK lock/manifests identity
requested composition identity
```

`cargo_executable` 是本机执行位置，不参与以工具语义身份为目的的 digest。负向 no-spawn 测试只替换
`cargo_executable`；修改 `cargo_identity` 必须被测试 helper 拒绝或产生显式 identity-mismatch failure，不能再被
描述为 exact-cache miss。

### 5.2 Exactness 不放宽

promotion transaction 的 artifact 与 lineage 必须共同满足：

```text
requestedIdentityDigest
+ lockInputDigest
+ rawLockDigest
+ resolvedGraphDigest
+ lineageDigest
+ descriptor/executable identity
-> resolvedArtifactKeyDigest
```

任一 digest、schema、artifact key、qualification seal 或 executable hash 不一致都 fail closed。不得通过忽略
`cargo_identity`、使用空 digest、接受近似 compatibility key 或只看目录名来获得 hit。

### 5.3 Authority 与 containment 不变

- normal Editor 仍只有 application-owned cache 的读取/正常构建权限；
- 外部 run-owned artifact/lineage 只有明确 maintenance promotion request 才能写入 destination；
- source、destination staging、backup、final 与 retained 路径都必须经过既有 containment/reparse 检查；
- promotion 不创建或刷新 ProjectRuntime trust receipt；
- qualification seal 只授权其精确 artifact + lineage pair，不扩展为目录级 authority。

### 5.4 Transaction indivisibility

事务成功的最小单位是：

```text
one exact artifact + one exact generated-lock lineage
```

只发布其中一个不算成功。已存在的 destination 对象也必须先验证 exact equality，之后才可作为 no-op
participant 进入同一事务结果；不能因为一个对象已存在就跳过另一个对象的验证或 rollback 规划。

## 6. Owner 与边界

唯一 public owner 继续是：

```text
ProjectEditorCompositionArtifact::promote_exact
```

内部可以深化既有 validation、staging、backup、publish、verify、rollback 和 report 辅助模块，但不得增加第二个
normal caller 可见的 cache manager。normal Editor 仍调用 `prepare`；qualification harness 仍负责构造精确
request 和 no-spawn probe，不拥有 promotion 算法。

预期影响 owner：

```text
rust/crates/editor_core/src/project_editor_composition_artifact.rs
rust/crates/editor_core/src/project_editor_composition_cache_promotion.rs
rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs
```

只有 schema/consumer closure 确有需要时，才窄改已有 production composition caller；不增加 generic crate。

## 7. Promotion Schema v3

### 7.1 Request v3

`project-editor-composition-promotion-request.v3` 在 v2 exact artifact authority 之上增加：

```text
sourceLineageDirectory
sourceCargoLockPath
sourceLineageManifestPath
expectedLockInputDigest
expectedRawLockDigest
expectedResolvedGraphDigest
expectedLineageDigest
expectedResolvedArtifactKeyDigest
expectedLineageSchema
```

request 仍必须携带既有 source artifact、descriptor、qualification seal、requested/resolved identity、expected
executable hash、destination root、backup root 与 authorization facts。路径不能由 digest 自动在其它 run root 搜索；
caller 必须精确声明 source lineage location。

### 7.2 Report v3

`project-editor-composition-promotion-report.v3` 至少对 artifact 与 lineage 分别报告：

```text
sourcePath / sourceHash
stagedPath / copiedHash
finalPath / finalHash
previousDestinationState = absent | exact_existing | different_existing
action = published | verified_noop | restored | retained_for_diagnosis
backupPath / backupHash / backupDisposition
rollbackAttempted / rollbackSucceeded
retainedPath / retainedReason
```

lineage 还必须报告 `Cargo.lock` 与 `lineage.json` 各自 bytes hash，以及重新计算的 lock input、raw lock、
resolved graph、lineage digest。顶层 terminal 只能在两个 participant 都 verified 后为 `committed`。

### 7.3 版本迁移

- writer 只写 v3；
- v1/v2 report 可继续作为历史只读证据；
- v1/v2 request 不含 source lineage authority，不能进入 v3 mutation，也不能用默认路径或空字段补齐；
- 若历史 parser 必须保留，读取后只能得到 `lineage_authority_absent`，不得 silently upgrade；
- v2 promotion source 即使 artifact valid，也必须由 caller 重新提交显式 v3 request 才能施工。

## 8. Source Lineage 验证

promotion 在任何 destination mutation 前完成以下验证：

1. canonicalize 并 containment-check `sourceLineageDirectory`、`Cargo.lock`、`lineage.json`；
2. 读取原始 bytes，验证 schema 与 request 中的 expected facts；
3. 从稳定 build input 重新计算 `lockInputDigest`；
4. 对 `Cargo.lock` 原始 bytes 重新计算 `rawLockDigest`；
5. 用 R1 canonical graph parser 重新计算 `resolvedGraphDigest`；
6. 对 canonical lineage payload 重新计算 `lineageDigest`；
7. 将 lineage facts、requested identity、descriptor 与 executable hash 重新解析为
   `resolvedArtifactKeyDigest`；
8. 验证 qualification seal 精确绑定同一 resolved artifact key 和 executable hash；
9. 验证 source artifact 实际 bytes hash，不接受只来自 report 的 hash；
10. source validation 全绿后才允许创建 destination-owned staging。

任一输入在 read/verify 间变化必须通过 open handle、重复 stat/hash 或既有防 TOCTOU owner 被发现并 fail
closed。不得从 artifact descriptor 猜测或重建缺失的 `Cargo.lock`。

## 9. Artifact + Lineage Promotion Transaction

事务顺序固定为：

```text
validate request authority and containment
-> validate source artifact + source lineage pair
-> inspect and hash both destination participants
-> create destination-owned transaction staging
-> byte-copy artifact, Cargo.lock and lineage.json
-> recompute copied hashes and all lineage/resolved facts
-> prepare backups for every different existing participant
-> publish lineage participant
-> publish artifact participant
-> reopen final files and verify pair exactness
-> commit transaction report
-> clean owned staging/backups according to retention policy
```

lineage 先于 artifact publish，避免 normal reader 在中间态看到“有 artifact、无 lineage”。即使如此，事务必须
使用既有互斥/atomic directory publish 边界，normal reader 不应观察半提交 pair。若现有 destination layout 无法
提供 pair-level atomic visibility，施工必须用 destination transaction marker/lock 深化同一 owner，而不是更改
normal lookup 语义。

### 9.1 Existing destination cases

| Artifact | Lineage | 结果 |
|---|---|---|
| absent | absent | 正常双发布 |
| exact | exact | 双验证后 `verified_noop` |
| exact | absent | 发布 lineage，artifact 作为 verified participant |
| absent | exact | 发布 artifact，lineage 作为 verified participant |
| different | 任意 | backup 后双 participant transaction |
| 任意 | invalid/different | backup 后精确发布，或在 policy 不允许 replacement 时 fail closed |

“exact”必须由 bytes 与全链 digest 重新验证，不能只按目录 key 判断。

## 10. Rollback 与保留证据

任一 publish、final verify 或 report finalization 失败时：

```text
block new normal reads through existing transaction boundary
-> remove newly published participant when owned and verified
-> restore prior artifact from backup if it existed
-> restore prior lineage directory from backup if it existed
-> reopen and hash both restored participants
-> release transaction boundary
-> report rollback terminal and retained evidence
```

rollback 成功必须证明 destination 回到 mutation 前的 pair state，而不只是“rename 返回成功”。rollback 无法
确认时不得自动重试或继续 Gate H；保留 destination-owned staging/backup 并给出精确路径、hash 与人工处置原因。

report finalization failure同样属于事务失败，必须沿用 278 已确立的 terminal/report finalization discipline：
不能先对外声明 committed，再把 report 写失败当 warning。

## 11. Normal-Hit Identity Consistency Rule

qualification helper 应从 source request 派生 normal-hit request：

```rust
let normal_hit_request = source_request
    .clone()
    .with_cargo_executable(run_root.join("must-not-start-cargo.exe"));
```

派生 helper 必须断言除 `cargo_executable` 和允许的 destination/run-control 路径字段外，所有 digest-bearing
semantic fields byte-equal。禁止手工重新构造完整 request 后遗漏或改写 `cargo_identity`。

成功证明必须同时满足：

```text
cacheOutcome = exact_cache
steps = 0
generatedLockOutcome = exact_lineage_hit
cargoSpawnAttempted = false
nonexistent cargo executable remains absent/unstarted
resolvedArtifactKeyDigest = promoted expected key
```

若 identity 不一致，应在进入 normal prepare 前报告 harness/request mismatch，而不是把随后 miss 记为 loader
regression。

## 12. Diagnostics

新增或稳定以下诊断：

```text
project_editor_composition.promotion_source_lineage_authority_missing
project_editor_composition.promotion_source_lineage_path_invalid
project_editor_composition.promotion_lock_input_digest_mismatch
project_editor_composition.promotion_raw_lock_digest_mismatch
project_editor_composition.promotion_resolved_graph_digest_mismatch
project_editor_composition.promotion_lineage_digest_mismatch
project_editor_composition.promotion_artifact_lineage_pair_mismatch
project_editor_composition.promotion_pair_publish_failed
project_editor_composition.promotion_pair_final_verify_failed
project_editor_composition.promotion_pair_rollback_failed
project_editor_composition.qualification_normal_hit_identity_mismatch
project_editor_composition.qualification_unexpected_cargo_spawn
```

每个诊断必须包含稳定 code、terminal/non-terminal 分类、participant、expected/actual digest 和 run-owned 或
destination-owned evidence path；不得只输出自然语言 “cache miss”。

## 13. 测试与资格矩阵

### 13.1 Owner red tests

- v2 request 缺 lineage authority 时必须 fail closed，destination 零 mutation；
- source `Cargo.lock` bytes tamper、lineage JSON tamper、graph edge/checksum tamper 分别失败；
- artifact、descriptor、qualification seal 与 lineage resolved key 任一错配均失败；
- artifact copy 成功但 lineage copy/publish 失败时，两个 participant 均恢复原状；
- lineage publish 成功但 artifact publish/final verify 失败时，两个 participant 均恢复原状；
- prior absent/exact/different 的组合覆盖完整，exact/exact 得到 verified no-op；
- rollback restore 后重新 hash；rollback 不能确认时保留 evidence 并 terminal failed；
- report finalization failure 触发 rollback，不产生 committed 假阳性；
- path-affine `ct` 永不进入 promotion source 或 report 的 portable participant。

### 13.2 Harness tests

- source request 派生 normal-hit request 时只有 `cargo_executable` 改变；
- 修改 `cargo_identity` 的负向 case 在 prepare 前以 identity mismatch 失败；
- 正确派生后 destination normal prepare 为 `exact_cache / steps=0`；
- nonexistent Cargo executable 不存在且 spawn counter 为零；
- 保持 identity、仅改变 executable path 不改变 `lockInputDigest`、resolved graph 或 artifact key。

### 13.3 Fresh source-only integration

在 repository-external fresh roots 中证明：

```text
root A: build + sealed lineage + qualification
root B: empty destination
promote_exact v3: exact artifact + exact lineage pair
root B normal prepare: exact_cache / steps=0 / cargoSpawnAttempted=false
```

另做一次 controlled mid-transaction failure，证明 root B artifact/lineage pair 完整回滚。该 integration 不触碰
production Editor、真实 cache/config，不运行 Local CI。

## 14. Gate H 重新进入条件

本方案生成与自审不恢复 Gate H。只有后续 repair 施工文档获得独立授权并完成其 source-only gates 后，用户才
能再次单独授权新的 282 Gate H。重新进入至少要求：

1. owner red tests、harness identity test、fresh pair promotion/rollback integration 全绿；
2. promotion request/report v3 consumer closure 完成，无 v1/v2 silent authority upgrade；
3. 新 fresh run root、新 candidate/hash、新 qualification seal；
4. production Editor、真实 cache/config/recent/trust 与 process preflight 重新取得；
5. backup、rollback、retained evidence 权限重新取得；
6. normal smoke 使用同 identity + nonexistent executable probe；
7. 不复用本次失败 Gate H 的 mutation authorization、candidate 或 promotion report。

## 15. 建议后续施工拆分

本节仅供后续单独施工文档参考，不构成施工授权：

```text
282-R2 B-min Repair Window A
  Gate R2-A: promotion request/report v3 + v1/v2 fail-closed migration
  Gate R2-B: source lineage full-chain validation + pair transaction/rollback
  Gate R2-C: qualification request derivation + no-spawn identity invariant
  Gate R2-D: owner/consumer red-capable closure

282-R2 B-min Repair Window B
  Gate R2-E: fresh artifact+lineage promotion and normal exact hit
  Gate R2-F: controlled rollback integration + completion/re-entry record
```

Window B 完成也只产生 `eligible_to_request_new_gate_h`，不自动执行 Gate H。

## 16. 停止条件

- 需要弱化 requested/resolved identity、trust、qualification seal 或 executable hash 才能命中；
- 需要让 normal loader 绕过 lineage 或 artifact-first fallback；
- 无法从 source bytes 重算 lock/graph/lineage/artifact 全链 digest；
- artifact 与 lineage 无法在同一 transaction/rollback boundary 内处理；
- 需要复制或宣称 path-affine compilation target 可移植；
- rollback 后无法重新证明 destination pair 与 mutation 前一致；
- 测试只能 warm-up 后通过，不能用 nonexistent Cargo executable 证明 `steps=0`；
- 需要触碰 production、真实 cache/config 或重跑 Gate H 才能完成 repair source closure。

遇到任一停止条件，repair 必须停止并回到方案讨论，不得扩大 B-min。

## 17. 自审记录

自审日期：2026-08-12

| 自审项 | 结果 | 结论 |
|---|---|---|
| 根因是否以真实 Gate H 证据为准 | 通过 | 已纠正为 harness `cargo_identity` 不一致，并保留 promotion pair 合同缺口 |
| 是否与 282/R1 架构一致 | 通过 | 复用 R1 lineage/resolved identity/path-affinity/deadline，不推翻 R1 |
| 是否新增重复 owner | 通过 | 只深化既有 `ProjectEditorCompositionArtifact::promote_exact` |
| identity/trust/exactness 是否放宽 | 通过 | 全部保持或加强；不存在 artifact-first 或空 digest migration |
| schema migration 是否明确 | 通过 | v3 writer；v1/v2 只读且无 lineage mutation authority |
| source validation 是否可独立审计 | 通过 | lock input、raw lock、graph、lineage、artifact key 与 executable 全部重算 |
| transaction 是否覆盖完整 pair | 通过 | artifact + lineage 同 staging/backup/publish/final verify/rollback |
| rollback 与 retained evidence 是否完整 | 通过 | 覆盖 prior absent/exact/different、finalization failure 与 rollback unconfirmed |
| normal-hit 测试是否 red-capable | 通过 | 只换 executable；identity 改动显式失败；不存在 Cargo 路径证明零 spawn |
| 是否保护外部状态 | 通过 | 正式方案阶段不改代码、production、真实 cache/config，不运行 Local CI |
| 是否越权恢复 Gate H | 通过 | 明确要求后续 repair 施工和新的单独 Gate H 授权 |
| 是否生成或激活施工文档 | 通过 | 未生成施工文档，未激活施工，282 当前文档仍保持 blocked/current |

自审结论：

```text
PASSED

282-R2 B-min 正式方案边界完整、根因修正明确、owner 唯一、schema 与 migration 可施工、
artifact+lineage transaction/rollback 可验证、normal-hit 证明可 red-first。

当前没有施工授权。下一步只能是：单独生成并自审 282-R2 B-min repair 施工文档。
不得直接修改代码、触碰 production/真实 cache/config、运行 Local CI 或重跑 Gate H。
```

## 18. 审查后 Scope Correction：Harness Identity Only

本节替代前文中把 promotion v3、full-chain source validation、artifact+lineage pair transaction 和 Window B
fresh pair matrix列为当前 repair 前置的内容；其它 identity/trust/外部状态不变量继续有效。

### 18.1 决定性 evidence

```text
production-promotion-report.json：artifact promotion = promoted
rollback-report.json：lineageAbsentFromRealCache = true（回滚后）
rollback-report.json：retainedLineage = ...\rollback-retained\locks\b4ac...
source request cargo_identity：cargo 282-gate-h-candidate
normal request cargo_identity：cargo 282-gate-h-must-not-run
normal result：generate_composition_lock / spawn_failed
```

这证明正确 lineage 在 mutation 阶段已存在于 destination；normal request 因 semantic identity 变化计算不同
`lockInputDigest`。本次失败不证明 promotion owner 缺少 lineage transaction。

### 18.2 当前最小修复

1. 从 source build request clone/derive normal-hit request；
2. 只替换 `cargo_executable` 为不存在路径；
3. 保持 `cargo_identity` 和其它 digest-bearing fields 不变；
4. 增加一个定向回归，证明 executable-only 变化不改变 generated lock input identity；
5. 保留现有 `ExactCache + steps.is_empty()` 作为 no-Cargo 证明，不新增重复 report 字段。

预计只修改：

```text
rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs
```

### 18.3 冻结内容

以下内容不再是 282-R2 当前 repair 或 Gate H re-entry 前置：

- Promotion Request/Report v3；
- v1/v2 promotion migration；
- 通用 artifact+lineage pair transaction；
- absent/exact/different destination matrix；
- promotion report-finalization rollback fault family；
- `exact_lineage_hit` / `cargoSpawnAttempted` 等重复 report schema。

它们只有在独立 reproduction 证明现有 promotion owner 确实导致故障时，才能另开方案讨论和授权。

### 18.4 修订后自审

```text
confirmed failure：harness cargo_identity mismatch
minimum causal fix：clone source request and change cargo_executable only
must-change files：qualification harness one file
red-capable proof：identity-preservation unit test + existing exact-cache/steps=0 disposable test
deferred：generic promotion v3 and pair transaction hardening
结果：PASSED；范围较原 Window A 显著收缩
```
