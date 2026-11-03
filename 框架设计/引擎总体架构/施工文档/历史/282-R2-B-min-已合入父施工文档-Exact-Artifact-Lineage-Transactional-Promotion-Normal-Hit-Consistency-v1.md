# 282-R2 B-min Repair 施工文档 - Exact Artifact + Lineage Transactional Promotion + Normal Hit Consistency v1

## 1. 文档状态

```text
系统编号：282-R2
父系统：282 Project Editor Composition Cache Lifecycle + Build QoS v1
正式方案：282-R2-Exact-Artifact-Lineage-Transactional-Promotion-Normal-Hit-Consistency-B-min-v1方案.md
用户选择：方案 B-min
位置：施工文档/历史/
状态：已合入 282 当前父施工文档；禁止独立施工
生成日期：2026-08-12
源码基线：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留 dirty worktree
当前施工槽：282 父施工文档占用；新 Gate H 未通过且 282 未归档
当前施工/测试授权：Window A / Gate R2-A 至 R2-D；执行真相只在 282 当前父施工文档第 24 节
```

本文已作为计划证据合入 282 当前父施工文档并移入历史，不再构成独立执行入口。Window A / Gate R2-A 至
R2-D 的唯一执行真相是父施工文档第 24 节；Window B、fresh root、Local CI、production/真实 cache/config
与 Gate H 仍未授权。

## 2. 失败基线与根因

```text
记录：阶段完成记录/2026-08-11-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/05-Gate-H.md
fresh root：C:\AifeProductionRuns\282-r1-gate-h-20260812-150654
candidate：5007CDA6E32036D2F5FAD5A4CBD58B6A1AA9ED20E21E23DC905A66D42A5B9BC2
requested identity：sha256:4142b1e905ebeebcd174f3660a821bf72602af2ccb8c661de58921d22baca056
resolved graph：sha256:d7150c7cabaeb067a1ebb69b73c1203820f878361779ea3a1b1ab1b7d564c0af
artifact key：sha256:0b4ed21bbbadd92cea528bfe849e9c7cc999face98fc02588e8c339896a482aa
qualification/promotion：passed
normal loader：未达到 exact_cache / steps=0
rollback：production Editor、真实 cache/config/recent/trust/process 完整恢复
```

第一直接原因是 Gate H normal-hit request 把 source 的
`cargo_identity = "cargo 282-gate-h-candidate"` 改成了 `cargo 282-gate-h-must-not-run`。
`cargo_identity` 进入 `lockInputDigest`，loader 查找不同 lineage key 是正确行为。负向测试只能替换
`cargo_executable`。现有三个成功 fresh tests 已采用 clone request + executable-only 的正确模式。

第二合同缺口是现有 `promote_exact` 只事务化 artifact；R1 normal prepare 还依赖精确的
`Cargo.lock + lineage.json`。B-min 只深化该 owner，让 one exact artifact + one exact lineage 成为同一
validation/publish/verify/rollback transaction。

## 3. 授权窗口

```text
Window A / Gate R2-A 至 R2-D
  schema v3、source validation、pair transaction/rollback、harness identity、affected closure
  只用源码测试与测试自有临时目录

Window B / Gate R2-E 至 R2-F
  唯一新的 repository-external fresh root
  pair promotion、normal exact hit、controlled rollback、re-entry record
```

每个 Window 必须单独授权。Window B 完成只允许声明 `eligible_to_request_new_gate_h`，不自动进入 Gate H。
两窗均禁止 Local CI、production/installed replacement、真实 cache/config、warm-up fallback、normal-loader
算法修改、path-affine `ct` 提升、第二套 manager/daemon/runner、Tower/AUI/gameplay 修改和 stable ABI redesign。

## 4. 激活方式

282 父文档仍在 `施工文档/当前/`，本文不能成为第二份当前文档。获得 Window A 明确授权后先：

1. 重读正式方案、本文、父文档、R1 完成记录与 `05-Gate-H.md`；
2. 重算第 7 节 hashes，分类重叠漂移并保留用户修改；
3. 核对 schema、test filters、fault seam 与三小时预算；
4. 将本文 Gate 合入唯一 282 当前父文档并记录授权/复核基线；
5. 将本文改为“已合入父文档；禁止独立施工”并移入 `施工文档/历史/`；
6. 同步 `54`、施工 README 与待执行 README。

上述事务完成后才可开始 R2-A。Window B 激活前必须有 Window A 完整证据和新的 fresh-root 授权。

## 5. 不变量

1. normal-hit 只改变 `cargo_executable`；所有 digest-bearing semantic fields 与 source byte-equal。
2. requested/resolved identity、trust、qualification seal 与 executable hash不放宽。
3. promotion 单元固定为 exact artifact + exact lineage；只发布一个不算成功。
4. 从 source bytes 重算 lock-input/raw-lock/graph/lineage/artifact-key 与 artifact/seal hashes。
5. v3 writer 拥有显式 lineage authority；v1/v2 只读，禁止 silent authority upgrade。
6. absent/exact/different 全部走统一 pair plan；exact/exact 也重新验证。
7. lineage 先 publish、artifact 后 publish，normal reader不得观察半提交 pair。
8. publish/final verify/report finalization 失败必须共同 rollback 并 re-hash。
9. rollback unconfirmed 必须 terminal + retained，禁止 retry/Gate H。
10. `ct` 保持 `crossRootPortable=false`，不进入 promotion。

## 6. 允许范围

Window A 默认允许：

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

不预授权 `runtime_cli`、normal-loader owner、Tower 或其它子系统。Window B 默认不改产品源码，只窄调
qualification harness，并在一个新 `C:\AifeProductionRuns\282-r2-bmin-<timestamp>` 内写 run-owned state。

## 7. 当前字节基线

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

这些是 dirty-worktree 漂移检测值，不是 clean/release identity。已有改动属于用户，禁止 reset/revert/clean。

## 8. Gate R2-A - Promotion v3 Schema 与 Migration

任务：

1. request v3 加入 source lineage directory、`Cargo.lock`、`lineage.json` 和 expected lock-input/raw-lock/
   graph/lineage/artifact-key facts。
2. report 对 artifact/lineage 分别记录 source/staged/final path/hash、prior state、action、backup、rollback、retained。
3. writer 只写 v3；v1/v2 report 历史只读；v1/v2 request 在 mutation 前 `lineage_authority_absent`。
4. 保持 deny-unknown、absolute paths、authority id 与 requested/resolved validation，更新直接 constructors。

Red tests：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_promotion_schema_ -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_v2_ -- --nocapture
```

通过：v3 round-trip/unknown rejection；v2 零 mutation；writer/consumer 全部显式 lineage authority。激活前
校准实际 filter，0 tests passed 不算通过。

## 9. Gate R2-B - Source Lineage Full-chain Validation

任务：在 destination mutation 前 containment/reparse-check source paths；从 bytes 重算 lock input、raw lock、
canonical graph、lineage、resolved artifact key、artifact/descriptor/report/seal/executable hashes；复用 R1 结构化
Cargo.lock owner；检测 validation/copy TOCTOU。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_promotion_source_lineage_ -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_pair_mismatch_ -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_rejects_identity_hash_report_seal_and_path_tamper -- --nocapture
```

矩阵覆盖 raw lock、lineage JSON、graph edge/checksum、resolved key、source mutation 与 reparse/outside tamper。
所有失败必须发生在 destination mutation 前并给 expected/actual digest。

## 10. Gate R2-C - Pair Transaction 与 Rollback

深化现有 `promote_exact_with_faults`：destination staging copy 三类文件并重算；处理 absent/exact/different；
backup、lineage publish、artifact publish、final reopen verify；任一阶段或 report finalization 失败共同 rollback。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_promotion_pair_ -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_detects_copy_mutation_and_rolls_back_publish_failure -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_rollback_failure_is_terminal_and_retained -- --nocapture
```

faults 分别覆盖 lineage copy、lineage publish 后 artifact failure、final verify、report finalization、rollback failure。
通过要求 pair 双发布/no-op、mutation 前状态精确恢复，或 rollback unconfirmed 时 terminal retained。

## 11. Gate R2-D - Harness Identity 与 Affected Closure

新增 clone-based request derivation helper；只替换 executable 与允许的 run-control path；prepare 前比较 semantic
fields，identity 改动报 `qualification_normal_hit_identity_mismatch`。修正 Gate H harness 的 identity，但不运行
Gate H test。成功报告必须含 `exact_cache / steps=0 / exact_lineage_hit / cargoSpawnAttempted=false`。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit project_editor_composition_normal_hit_request_identity_ -- --nocapture
cargo test -p editor_core project_editor_composition_promotion_ -- --nocapture
cargo test -p editor_window_winit project_editor_composition_ --lib -- --nocapture
cargo test -p editor_core
cargo check -p editor_window_winit --features real-window
cargo fmt --all -- --check
```

开发时先跑 filters；最终 `editor_core` 与 winit closure subsume 同配置 filters，不机械重复。不得运行 Gate H
candidate/promote tests、真实 release build、production Editor 或 Local CI。完成后写 `06-R2-B-min-Window-A.md`
并停止，不进入 Window B。

## 12. Gate R2-E - Fresh Pair Promotion 与 Normal Hit

经单独授权创建：

```text
C:\AifeProductionRuns\282-r2-bmin-<timestamp>\
  source-qualified/ destination-cache/ rollback-destination/ cargo-target/ temp/ evidence/
```

新 source build 产生 lineage/artifact/seal；v3 pair promotion 到空 destination；clone source request 只换不存在的
Cargo executable；证明 exact_cache、steps=0、exact_lineage_hit、zero spawn 与 expected artifact key；再验证
exact/exact no-op。

```powershell
Set-Location G:\gameEngin\rust
$env:AIFE_282_R2_BMIN_RUN_ROOT='C:\AifeProductionRuns\282-r2-bmin-<timestamp>'
$env:AIFE_282_R2_BMIN_EVIDENCE_ROOT='C:\AifeProductionRuns\282-r2-bmin-<timestamp>\evidence'
cargo test -p editor_window_winit project_editor_composition_282_r2_bmin_fresh_pair_promotion_normal_hit --lib -- --ignored --exact --nocapture
```

不得复用失败 Gate H root/candidate/authority。没有足够 hard deadline + evidence/cleanup reserve 时不得启动。

## 13. Gate R2-F - Controlled Rollback 与 Re-entry Record

在独立 rollback destination 注入 mid-transaction failure，证明 before/after pair hash 一致、无 owned process。
按实际 Window B 变更只续跑失效的 owner/harness evidence。写 `07-R2-B-min-Repair.md`，只声明
`eligible_to_request_new_gate_h`，同步入口，不自动 Gate H。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit project_editor_composition_282_r2_bmin_fresh_pair_rollback --lib -- --ignored --exact --nocapture
```

## 14. Evidence、失败与 Cleanup

每 Gate 保留 source/harness identity、request/report hashes、首个 assertion/diagnostic、stdout/stderr/timing、
artifact+lineage source/staged/final/backup hashes、destination inventory、rollback/retained、process inventory 与
外部状态未变事实。失败先建 owner reproduction，不跨 Gate、不用 Local CI/production 调试。

Window B cleanup 只处理列明的 role-root strict-child `cargo-target/temp/failed staging`。删除前验证 literal absolute
strict child、逐级 reparse、evidence preserved、owned process=0；不得删 role root/evidence/backup/workspace/真实
cache。记录 `removed / missing / retained_by_host_policy`。

## 15. 三小时与停止条件

每个 Window 独立计时。2:45 后不开新实现/验证；3:00 停止并记 `stopped_by_time_limit`。以下情况立即停止回到
方案：需弱化 identity/trust/seal；需改 normal lookup/artifact-first；无法重算全链；pair 无法同事务 rollback；
需复制 `ct`；rollback 无法证明；只能 warm-up 后命中；需触碰 production/真实 cache/config 才能闭环。

## 16. Gate H Re-entry 与归档

R2-F 后仍需新 Gate H 单独授权：new root/candidate/hash/seal、production binary/cache/config/recent/trust/process
preflight、backup/rollback/retained authority、v3 pair promotion 与 same-identity nonexistent-executable smoke。旧
authorization/candidate/report 不复用。

R2 repair 完成时新增 `06-R2-B-min-Window-A.md`、`07-R2-B-min-Repair.md`，更新父文档与各入口；本文在激活时
已移历史。父 282 继续留在当前，不声明完成、不归档、不自动 Gate H/Local CI/其它施工。

## 17. 激活前复核

确认唯一父文档、正式方案有效、Gate H 记录未改、hash 漂移已分类、schema/fault/test seam 存在、成功 fresh
tests 仍证明 executable-only pattern、Window A 可在三小时完成、外部操作均未授权、合入父文档事务可完成，
并在修订后重新自审。

## 18. 施工文档自审

| 自审项 | 结果 | 结论 |
|---|---|---|
| 与正式方案一致 | 通过 | R2-A 至 F 覆盖 v3、full-chain、pair transaction、identity、fresh proof |
| 根因准确 | 通过 | identity mismatch 是直接原因，不误判 loader defect |
| identity/trust 未放宽 | 通过 | v1/v2 无 silent authority；全链加强 |
| owner 唯一 | 通过 | 只深化既有 `promote_exact`；qualification 只派生 request |
| transaction/rollback 完整 | 通过 | 覆盖 pair states、各阶段 fault、finalization 与 retained |
| 测试 red-capable | 通过 | owner filters + affected closure + fresh no-spawn/rollback |
| 无机械重复 | 通过 | full suite subsume filters，按 evidence identity 复用 |
| 外部状态受保护 | 通过 | A 临时目录；B 单 fresh root；production/真实状态禁止 |
| dirty worktree 受保护 | 通过 | hashes 只作漂移检测，禁止 reset/revert/clean |
| 唯一施工文档规则 | 通过 | 激活时合入父文档，本文移历史 |
| 正式方案需修订 | 否 | 当前 owner/fault/request patterns 支持 B-min |
| 外部审查 | 无 | 本轮没有用户指定的匹配审查文档 |

```text
施工文档自审：通过
当前状态：已生成、已自审、未激活、不可施工
当前授权：无
下一步：等待单独授权 Window A / Gate R2-A 至 R2-D，并执行激活前复核
禁止：代码、Gate、Window B、Local CI、production/真实 cache/config、Gate H
```

## 19. 本轮生成记录

```text
只生成并自审本文并同步索引。
未修改 Rust/source/test code，未运行测试/build/Local CI/Gate。
未创建 fresh root，未触碰 production/真实 cache/config/Gate H evidence。
未激活本文，未修改父 282 的执行 Gate。
```
