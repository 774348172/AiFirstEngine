# 282-R1 当前可自动化施工文档 - Generated Lock Lineage + Path-Affine Compilation Cache + Tiered Build Deadline v1

> 完成状态：R-A 至 R-G 已于 2026-08-12 完成；记录见
> `阶段完成记录/2026-08-11-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/04-R1-Repair.md`。
> 当前只具备重新申请 Gate H 的资格，Gate H 未授权、未执行。

## 1. 文档状态

```text
系统编号：282-R1
父系统：282 Project Editor Composition Cache Lifecycle + Build QoS v1
正式方案：282-R1-Generated-Lock-Lineage-Path-Affine-Compilation-Cache-Tiered-Build-Deadline-v1方案.md
用户选择：方案 B
文档位置：施工文档/历史/
文档状态：已合入 282 当前施工文档；禁止独立施工
生成日期：2026-08-12
当前源码基线：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留的 dirty worktree
当前施工槽：282 父施工文档占用；Gate H 未通过且未归档
当前施工/测试授权：Window A / Gate R-A 至 R-D，执行真相只在 282 当前施工文档
```

本文只定义 282 Gate H 失败后的受控 repair。它不授权修改源码、运行测试、构建 fresh candidate、
运行 Local CI、替换 production/installed binary、写真实用户 cache、修改真实配置或重跑 Gate H。
本文已作为计划证据移入历史，不得作为命令入口；唯一执行真相是 282 当前施工文档第 22 节。

## 2. 失败基线与 repair 目标

282 Window C / Gate H 在任何 production mutation 前停止。保留证据为：

```text
失败记录：阶段完成记录/2026-08-11-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/02-Window-C.md
candidate SHA-256：AA8EB114B44B3022ACD94ED1F541E2FF78E059A542573388F398D2FE39355628
production Editor SHA-256：F17BEDB2A3892486DED074DE1E4FA1AEF9D4D14CC37E51E35B4155BEBD23C786
Tower expected identity：sha256:e76e62c9c4d04bb018d713ccb8f79e1cced22e4d8db0cc83780ff45cc6484ade
Gate G/H compatibility key：554a84eec277b25aed63bca5df81f79d
release attempt 1：jobs=2，600015 ms，timeout
release attempt 2：jobs=1，600004 ms，timeout
production replacement / real cache promotion：未开始
```

repair 必须一次收敛三个合同缺口：

1. 实际 `GeneratedEditor/Cargo.lock` 进入可审计 lineage，resolved dependency graph 进入 compatibility。
2. Cargo target 明确为 path-affine；跨 root copy 只能得到 `path_affine_miss`，不得声明 portable hit。
3. generate-lock、release build、descriptor query 使用 typed deadline；release soft budget 越界只告警，
   hard deadline 才终止，并保留准确 terminal 与 cleanup evidence。

## 3. 授权边界与窗口

后续施工必须分两次单独授权：

```text
Repair Window A / Gate R-A 至 R-D
  schema + owner + consumer source closure
  只使用源码测试与测试自有临时目录

Repair Window B / Gate R-E 至 R-G
  repository-external fresh root A/B integration
  qualification/promotion resolved-identity closure
  repair 完成记录与 Gate H re-entry preflight
```

Window A 不自动进入 Window B。Window B 完成后也不自动恢复或重跑 282 Gate H。Gate H 需要新的
production 范围授权、新 fresh root、新 candidate/exact artifact/qualification seal 和新的 production
preflight；旧 Gate H 授权已经结束。

两个 repair window 均禁止：

- Local CI；
- production/installed Editor、Player、MCP 或其它 binary replacement；
- 真实用户 composition cache 写入、迁移或清理；
- 真实项目配置、Codex config、recent-project、trust tree 修改；
- 自动扫描旧 fresh roots 或导入未点名的 artifact/lineage；
- Tower gameplay、AUI、RuntimePackage、Sprite2D、Animator2D 或 GameView 功能修改；
- stable ABI/capability digest、build daemon、sccache、remote cache 或通用 workflow 扩展。

## 4. 激活方式：合入唯一父施工文档

282 父施工文档仍在 `施工文档/当前/`，因此本文不能作为第二份当前施工文档激活。取得明确的
`282-R1 Repair Window A / Gate R-A 至 R-D` 授权并通过激活前复核后，必须执行一次原子文档事务：

1. 把本文第 1-19 节的可执行 repair 计划合入当前 282 施工文档，新增 `282-R1 Repair` 章节；
2. 在当前 282 文档中记录 R1 正式方案、授权窗口、复核 source/hash 基线和禁止项；
3. 将本文状态改为 `已合入 282 当前施工文档；禁止独立施工`，移到 `施工文档/历史/`；
4. 更新 `54`、施工 README 与待执行 README；
5. 全程保持 `施工文档/当前/` 只有原 282 一份可执行 `.md` 文档。

只有上述事务完成后，才可按合入后的 282 当前文档施工 Gate R-A。不得在本文仍位于 `待执行/` 时
改代码、跑测试或创建 fresh root，也不得仅凭用户说“开始”绕过激活前复核。

## 5. 不可破坏的不变量

1. `ProjectEditorCompositionIdentity.v1` 的 requested identity 字段不删除、不忽略；compatibility hit
   不能替代 exact artifact hit。
2. final artifact 使用 resolved identity 加强 exactness：requested identity digest、lineage digest、
   resolved graph digest 和 resolved artifact key 必须一致。
3. raw lock bytes 只用于 byte-exact seal；canonical resolved graph 才用于 compatibility。两者不得混用。
4. generated root package exact name与 absolute path noise 不进入 resolved graph digest；registry/git
   version/source/checksum/edge 变化必须改变 digest。
5. Cargo target、incremental object、PDB、rmeta 和 build-script output 均为 path-affine；
   `crossRootPortable=false`。
6. 可移植对象只有 sealed lock lineage 和 final exact qualified artifact，且跨 root 使用仍需明确 authority
   与全量重校验。
7. trust receipt、qualification authority、promotion containment、双重哈希、backup/publish/rollback
   权限不放宽。
8. soft budget 不是 timeout；hard deadline 不是 hang detector；cancel 始终优先。
9. Completed/non-zero/Cancelled/Timeout/WaitFailed/SpawnFailed 保持不同 terminal。
10. R1 writer 使用新 schema；历史 schema 只按正式方案规定读取，绝不用空默认字段伪造升级。

## 6. 预计影响文件与当前字节基线

核心 owner：

```text
rust/crates/editor_core/src/project_editor_composition.rs
  SHA-256 788EE47084F3875E6BE1732BD04EBEB461C184A35CF6C75D302F83F9CD219F83

rust/crates/editor_core/src/project_editor_composition_artifact.rs
  SHA-256 042068C038367AC28788134DD6D58EE8E45266E8F446E81599908F11BA9E1BFF

rust/crates/editor_core/src/project_editor_composition_cache_promotion.rs
  SHA-256 A56FB0CBE115F8DFB8365E4F8CBC9B5B7136768014F2229FABF229C37A7D2B50
```

直接 consumer：

```text
rust/crates/editor_core/src/project_runtime_player_staging.rs
  SHA-256 651E0F201C9FB4C076B3C2059951DAACC91CAF39E16126EC785A80BB80E16F46

rust/crates/editor_core/src/editor_project_composition_launcher.rs
  SHA-256 FBE9641BAAB0897403374446C137DC36E83F01DAAB6E1ED5575C7AFAAF330286

rust/crates/editor_window_winit/src/project_editor_composition_production.rs
  SHA-256 CD93F96622564B7E8ED6949BB03ADAD03A5718F52CF41584E138BB91572035CB

rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs
  SHA-256 A8D407EBB5503321731E8D0927EC3FEC4B4052356533E793F13AAF074F450FBE

rust/crates/editor_window_winit/src/tests/project_editor_composition_production_gate.rs
  SHA-256 0FA062CCF5013A5C1C96E86FB0738D2C976F44986F39DD9DAEF4B6717FEF93D9
```

仅在现有 result 无法表达 cleanup evidence 时允许窄改：

```text
rust/crates/runtime_cli/src/bounded_child_process.rs
  SHA-256 8CA14E43196885CA7D0F4B8E35A5D4201E45EE22D8A8C19FFD483AD311DEDB45

rust/crates/runtime_cli/tests/bounded_child_process.rs
  SHA-256 55D39D11CCAA286970DBB53DAA73702B23DFD99F9DFC61F540468B202C8FCCB0
```

这些哈希来自 dirty worktree，只用于激活前发现重叠漂移，不是 clean commit 或 release 资格声明。
激活前必须重新计算；已有用户改动全部保留，禁止 reset、checkout、clean 或覆盖式恢复。

## 7. Gate R-A - Schema、Lock Lineage 与 Resolved Identity

### 7.1 任务

1. 在 `project_editor_composition.rs` 建立并严格验证正式方案第 10 节的新 schema：
   BuildRequest/BuildReport v3、Artifact/Descriptor/Qualification/Promotion v2、
   `GeneratedCompositionLockLineage.v1`、`ResolvedIdentity.v1` 与 typed deadline policy v1。
2. BuildReport reader 保持 v1/v2 历史只读；writer 只写 v3。Artifact/Descriptor v1 可读但不得作为
   R1 v2 promotion source；v1 qualification/promotion 只保留历史证据。
3. `lockInputDigest` 只绑定正式方案第 6.1 节的稳定输入，不含 staging/target root、PID、timestamp 或
   wall clock。
4. 实现 Cargo.lock 的结构化解析与 canonical dependency graph；生成 root package exact name和 absolute
   path 不进入 digest，package identity、version、canonical source、checksum、features 和 sorted edges
   必须进入。
5. 产生 `rawLockDigest` 与 `resolvedGraphDigest`，并建立 requested identity 到 resolved identity 的单向加强。
6. parse ambiguity、unsupported source、缺 checksum、graph inconsistency、raw/graph tamper 均 fail closed。

### 7.2 Red-capable owner tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_r1_contract_ -- --nocapture
cargo test -p editor_core project_editor_composition_lock_lineage_ -- --nocapture
cargo test -p editor_core project_editor_composition_resolved_identity_ -- --nocapture
```

测试矩阵至少覆盖：

```text
v3/v2 round-trip + deny_unknown_fields
v1/v2 historical read + no synthetic v2 promotion upgrade
same input -> same lineage
raw bytes tamper -> raw digest mismatch
graph edge/version/checksum/source change -> graph digest change
generated root package rename -> graph digest unchanged
absolute root relocation -> lineage/graph digest unchanged
ambiguous or unsupported lock graph -> fail closed
requested identity same but lineage/graph changed -> resolved identity changed
```

### 7.3 通过条件

- 两种 digest 语义清楚且测试可捕获混用；
- schema migration 不放宽旧 artifact/promotion；
- normal caller 不需要理解 Cargo.lock parsing 或 canonical graph 内部细节。

## 8. Gate R-B - Lineage Store 与 Path-Affine Compilation Cache v2

### 8.1 任务

1. 在 artifact owner 内建立 `locks/<lock-input-digest>/Cargo.lock + lineage.json`，使用 destination-owned
   staging、hash re-read、atomic publish 与 collision fail-closed。
2. lineage hit 时先验证 input/raw/graph digest，再复制到 GeneratedEditor staging；miss 时才执行
   `cargo generate-lockfile --offline`。
3. 将 compilation compatibility 升级到 v2，绑定 resolved graph 与正式方案第 7.1 节的稳定 build inputs。
4. 对 canonical absolute cache-owner anchor 计算目录分片 digest，并对最终 target root 另算验证 digest，
   布局为 `ct/<compatibility-digest>/<target-anchor-digest-prefix>/affinity.json + release/`。
5. 只有 compatibility、完整 anchor digest 与完整 final target-root digest 同时匹配才报告 `same_root_hit`；检测到其它
   root 的旧 affinity 时创建本 root 子目录并报告 `path_affine_miss`。
6. 旧 v1 `ct` 不迁移、不删除、不作为 v2 hit；normal Editor 不扫描其它 root，不从 `ct` 反推 lineage。

### 8.2 Red-capable owner tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_lineage_store_ -- --nocapture
cargo test -p editor_core project_editor_composition_compilation_cache_v2_ -- --nocapture
cargo test -p editor_core project_editor_composition_compilation_cache_path_affinity_ -- --nocapture
```

必须覆盖：valid hit、raw/graph/input tamper、collision、publish failure、same-root hit、cold、cross-root
copy `path_affine_miss`、affinity tamper、v1 isolation，以及 `crossRootPortable=false`。

### 8.3 通过条件

- 跨 root target copy 永远不会被交给 Cargo 作为 hit；
- lineage 可移植与 target path-affinity 在 schema/report/diagnostic 三层一致；
- 不触碰真实 cache，也不新增 global cache daemon 或通用 manager。

## 9. Gate R-C - Typed Deadline、Terminal 与 Cleanup Evidence

### 9.1 任务

1. 用 `ProjectEditorCompositionBuildDeadlinePolicy.v1` 替代散落 `step_timeout_ms.min(...)`：

```text
generateLockHardDeadlineMs       60000
releaseBuildSoftBudgetMs        600000
releaseBuildHardDeadlineMs     1200000
descriptorQueryHardDeadlineMs    30000
```

2. soft budget 越界只写 warning/evidence，进程继续运行；之后 exit 0 且 sealing 成功仍为 success。
3. hard deadline 才请求终止 owned process tree，并返回 step-specific timeout diagnostic；不得称为 hang。
4. cancel 始终优先，并与 hard timeout 保持不同 terminal。
5. 上层分别映射 Completed/non-zero/Cancelled/Timeout/WaitFailed/SpawnFailed，不再统一折叠为
   `build_step_failed`。
6. termination 发生时将 `owned_process_cleanup_confirmed()` 提升到顶层，明确输出
   `outputReadersJoined`、`rootWaitCompleted`、`processGroupReleased`；自然完成不伪装成 terminated。
7. 仅当现有 `runtime_cli::BoundedChildProcessResult` 无法表达这些事实时，才窄改 shared owner；不得把
   composition-specific soft budget放入通用 process API。

### 9.2 Red-capable tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_deadline_ -- --nocapture
cargo test -p editor_core project_editor_composition_process_terminal_ -- --nocapture
```

如果 `runtime_cli` 实际修改，再运行：

```powershell
cargo test -p runtime_cli --test bounded_child_process -- --nocapture
```

矩阵必须包含 soft budget 后成功、hard timeout、cancel、non-zero、spawn failure、wait failure，以及每个
termination 路径的 root wait/readers join/process group release/owned descendants 为零。

### 9.3 通过条件

- 超 600 秒正常编译不再被误杀，1200 秒仍有产品 hard stop；
- warning、terminal、cleanup evidence 均为 typed truth；
- 不用真实 release build 等 600/1200 秒来测试 owner 状态机。

## 10. Gate R-D - Affected Owner / Consumer Closure

### 10.1 Consumer 更新

更新所有实际构造或消费新 schema 的 owner：production request builder、launcher、qualification seal、
promotion、normal cache loader 和对应测试 fixture。不得用默认空 digest、测试专用 bypass 或 v1 fallback
让 consumer 编译通过。

### 10.2 验证

同一配置下完整 `editor_core` suite 已覆盖 Gate R-A-C 的 filtered tests，因此 R-D 运行完整 suite 后不再
机械重跑 filter：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core
cargo test -p editor_window_winit project_editor_composition_ -- --nocapture
cargo check -p editor_window_winit --features real-window
cargo fmt --all -- --check
```

只有实际修改 `runtime_cli` 时追加：

```powershell
cargo test -p runtime_cli
```

### 10.3 通过条件

- schema owner、artifact owner、promotion、qualification 与 production caller 使用同一 resolved identity；
- source-only owner/consumer closure 全绿；
- 没有运行 Local CI、真实 Editor、production replacement、真实 cache 或真实配置操作；
- Window A 完成后写阶段记录，但不自动进入 Window B。

## 11. Gate R-E - Fresh Root A/B Lineage Portability 与 Path-Affinity

### 11.1 外部状态范围

仅在 Window B 单独授权后建立一个新的 repository-external role root：

```text
C:\AifeProductionRuns\282-r1-<timestamp>\
  root-a\
  root-b\
  copied-ct-negative\
  evidence\
```

每个 root 拥有独立 build/cache/target/temp。禁止复用 282 Gate G/H 的 mutable target，禁止访问真实用户
composition cache；可读取旧失败报告作为历史证据，但不得修改旧 run root。

### 11.2 资格矩阵

1. root A 从 lock miss 生成并封印 lineage，完成 v2 path-affine target 与 exact artifact。
2. 复核 root A 的 raw/graph/input/resolved identity、affinity 和 deadline report。
3. 只把 sealed lineage byte-copy 到 root B，复制后重算全部 digest。
4. root B 必须得到 lineage hit；因 target root 不同必须报告 `path_affine_miss`，release build 最终成功。
5. 将 root A 的旧 `ct` 精确复制到 `copied-ct-negative`，证明旧 affinity 不能形成 hit，也不被 Cargo 消费。
6. 用 controlled process 跨 soft budget 后 exit 0，证明 success + warning；用缩短的测试 policy 跨 hard
   deadline，证明终止、join、cleanup 和无 owned process。

真实 release build 的 hard deadline 最多 1200 秒。开始每次高成本步骤前必须确认三小时窗口剩余量足以
覆盖 hard deadline、证据写入和 cleanup；上游失败使资格无效时 fail-fast，不进行盲目重试。

### 11.3 通过条件

- root A/B 的 resolved graph digest 相同，raw digest 的用途没有被误当 compatibility key；
- root B 为 lineage hit + target `path_affine_miss` + release success；
- copied `ct` 否定样本没有形成 portable hit；
- 所有进程和 mutable path ownership 明确，production/真实 cache/config 字节不变。

## 12. Gate R-F - Qualification / Promotion Resolved-Identity Closure

### 12.1 任务

1. root B exact artifact、descriptor、build report、qualification report/seal 全部绑定 resolved identity。
2. 在 role-owned destination cache 中执行 v2 exact promotion、no-op repeat 和 normal loader exact hit。
3. 任意 requested identity、lineage、resolved graph、resolved artifact key、descriptor、executable、report、
   qualification seal digest 变化必须拒绝。
4. v1 artifact/qualification/promotion 作为 source 必须 fail closed；promotion 不生成或刷新 trust receipt。
5. promotion 的 containment、destination staging、rehash、backup、atomic publish、rollback 继续复用 282
   owner，不复制第二套算法。

### 12.2 Owner 与 fresh integration 验证

若 Window A 后没有修改相关测试代码，复用 R-D owner suite；Window B 只运行新的 v2 定向 owner test 与
fresh integration，不重复完整 crate suite：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_editor_composition_promotion_v2_ -- --nocapture
cargo test -p editor_window_winit project_editor_composition_qualification_v2_ -- --nocapture
```

fresh typed evidence 必须记录 exact promotion、no-op hit、normal prepare `steps=0`，以及 lineage/graph/
resolved-key tamper 否定矩阵。

### 12.3 通过条件

- final exact artifact 的 portability 只来自完整 qualification/promotion authority；
- compilation target 的 path-affinity 不被 final artifact promotion 语义污染；
- 未写真实 cache、未替换 production、未修改真实配置。

## 13. Gate R-G - Repair Completion 与 Gate H Re-entry Preflight

### 13.1 最终受影响闭包

若 R-E/F 只运行已有 source code 且没有修复代码，不重跑 R-D 的完整 suite，直接复用并记录其 source/
test identities。若 R-E/F 暴露缺陷并修改代码，只续跑捕获失败的 owner test和实际受影响 consumer；完整
suite 只更新被该修复失效的部分。

最终始终执行：

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --all -- --check
git diff --check -- `
  crates/editor_core/src/project_editor_composition.rs `
  crates/editor_core/src/project_editor_composition_artifact.rs `
  crates/editor_core/src/project_editor_composition_cache_promotion.rs `
  crates/editor_core/src/project_runtime_player_staging.rs `
  crates/editor_core/src/editor_project_composition_launcher.rs `
  crates/editor_window_winit/src/project_editor_composition_production.rs `
  crates/editor_window_winit/src/project_editor_composition_qualification.rs `
  crates/editor_window_winit/src/tests/project_editor_composition_production_gate.rs `
  crates/runtime_cli/src/bounded_child_process.rs `
  crates/runtime_cli/tests/bounded_child_process.rs
```

上述 scoped diff 只检查本 repair 允许触及的文件；激活后应移除实际未修改的 optional 文件，必要时补入
激活前复核确认的新直接 consumer。不得把工作树中无关用户改动归咎于 R1。

### 13.2 完成记录

新增同一 282 完成记录 lineage 下的 repair 记录，例如：

```text
阶段完成记录/2026-08-11-Project-Editor-Composition-Cache-Lifecycle-Build-QoS-v1/03-R1-Repair.md
```

记录 source identity、dirty baseline、changed files、schema migration、root A/B、raw/graph/resolved digests、
affinity facts、deadline terminals、所有实际测试、复用证据、失败与修复、进程 cleanup、retained paths 和
明确未执行项。

### 13.3 Gate H re-entry preflight

R-G 只判断是否具备重新申请 Gate H 的源码与 fresh integration 前提，不执行 Gate H。必须给出：

```text
repair source/test identity
fresh A/B evidence eligibility
new schema versions and compatibility key
lineage + resolved graph + resolved artifact identity
fresh exact artifact/qualification seal eligibility
production Editor/cache/config 的上次只读基线
new Gate H 必须重新获取的 candidate/hash/preflight/backup authority
```

### 13.4 通过条件

- R-A 至 R-F 的所有 required claim 有 red-capable test 或 fresh evidence；
- 无 production、真实 cache、真实配置 mutation；
- repair 记录与入口同步完成；
- 只声明 `eligible_to_request_gate_h`，不声明 Gate H passed 或 production consistent。

## 14. 失败处理与停止条件

1. 保留首个失败 assertion/diagnostic、stdout/stderr、request/report、input identity 和 process ownership。
2. 先用当前 Gate 的 owner reproduction 修复，不把 Local CI、完整 workspace、真实 Editor 或 Gate H 当调试循环。
3. 需要忽略 identity/lineage/hash/trust/authority 才能通过时立即停止并回填正式方案。
4. Cargo.lock parser 不能确定 canonical graph、source/checksum/edge 时 fail closed，不使用文本删行猜语义。
5. 发现 cross-root target 实际被消费为 hit、soft budget 导致 kill、timeout/cancel terminal 合并、cleanup
   evidence 无法证明时，当前 Gate 失败。
6. 需要修改真实配置、production binary/cache、扫描旧 roots、引入 daemon/remote cache 或扩大到项目功能时停止。
7. 发现与用户改动重叠时保留现状并做增量施工；无法安全合并才请求决策，禁止 revert/reset/clean。
8. 每个连续施工授权最多三小时；2h45m 后不启动新实现或验证，3h 停止并记录
   `stopped_by_time_limit`。

## 15. 外部状态与清理

Window A 只允许测试 owner 创建的临时目录。Window B 只允许操作它自己的
`C:\AifeProductionRuns\282-r1-<timestamp>` role root。

清理任何 recursive child 前必须：

1. 保留 required evidence 和首个失败证据；
2. 确认 owned descendants 已退出；
3. 解析 literal absolute target，证明它是 role root 的严格子路径；
4. 检查 target 及其到 role root 的祖先不存在 reparse escape；
5. 只删除明确列名的 build/cargo-target/temp/failed-state child，不删除 role root、evidence、workspace、
   用户 cache、旧 282 run root 或任何未列名 sibling。

结果记录为 `removed / missing / retained_by_host_policy`。cleanup 失败不重跑已有效证据，但必须保留
retained path/reason。不得对 production binary/cache 做 recursive cleanup。

## 16. 激活前复核清单

1. 用户明确授权 `282-R1 Repair Window A / Gate R-A 至 R-D`。
2. 282 父施工文档仍是 `施工文档/当前/` 中唯一活动 `.md` 文档，Gate H 仍停在 production mutation 前。
3. 本文仍在 `待执行/` 且自审状态有效，没有另一份 R1 被激活或合入。
4. 重读 282 与 282-R1 正式方案、Gate H 失败记录和当前入口，确认范围未漂移。
5. 重算第 6 节所有实际涉及文件哈希，分类 dirty/untracked 用户基线与新漂移。
6. 检查 schema/struct constructor、promotion、qualification、production adapter、launcher 和测试 filter
   名称仍与代码布局一致；变化时先修订本文并重新自审。
7. 确认 Cargo.lock 将使用结构化解析，不引入 regex/字符串删行式 canonicalization。
8. 确认 Window A 能在三小时内完成实现、受影响 closure、证据与必要 cleanup。
9. 按第 4 节完成“合入父文档”原子事务并同步入口后，才允许开始 Gate R-A。

Window B 激活前还必须确认 R-A 至 R-D 的阶段记录完整，明确新的 role root、预计最坏 release hard
deadline、证据/cleanup reserve 与 production 禁止项。

## 17. 完成后的文档状态

R-A 至 R-G 全部通过后：

1. 在 282 父施工文档记录 R1 完成结果，但 282 整体仍停在 Gate H，不能归档；
2. 将已经合入并位于 `历史/` 的本文补记 R1 完成记录链接，不移动回 `当前/`；
3. 更新 282-R1 正式方案、`49`、`54`、`00-文档地图`、`00-阅读顺序`、施工 README、待执行 README
   与阶段完成记录 README；
4. 当前 282 文档只标记 `eligible_to_request_gate_h`；
5. 不自动生成 candidate、不替换 production、不写真实 cache、不重跑 Gate H、不启动其它项目或引擎 Gate。

只有以后新的 Gate H 单独授权完成 production consistency closure，282 才能生成总览、归档父施工文档并
释放当前施工槽。

## 18. 施工文档自审

### 18.1 与正式方案一致性

通过。R-A/R-B/R-C 分别实现 generated lock lineage、path-affine compilation cache 和 tiered deadline；
R-D 是 source owner/consumer closure；R-E/R-F 是正式方案要求的 fresh A/B 与 resolved-identity
qualification/promotion；R-G 只做 repair 完成与 Gate H 重新准入，不越权执行 Gate H。

### 18.2 identity、trust、authority 是否被放宽

没有。requested identity 保留，resolved identity 只加强 exactness；lineage 不产生 trust receipt；v1
artifact/qualification/promotion 不可伪装为 v2 source；promotion containment、hash、backup、rollback 不变。

### 18.3 是否正确区分 portable 与 path-affine

是。raw lock seal、resolved graph compatibility、target-root affinity 和 final exact artifact authority 分为
四种不同事实。R-E 明确只复制 lineage作为正向样本，复制 `ct` 只能作为否定样本。

### 18.4 deadline 是否掩盖 hang 或取消

没有。soft budget 只告警，hard deadline 表达产品上限，cancel 优先；不声称基于时间判断 rustc hang。
terminal 和 cleanup evidence 均有 owner test，真实 release build 只证明 fresh integration。

### 18.5 Module 是否足够深

通过。Cargo.lock 解析、lineage store、compatibility/affinity、deadline mapping、artifact seal 和 promotion
均留在现有 composition owner；normal caller 只提供 typed request 并消费 typed report。没有新增 daemon、
global cache manager、通用 workflow 或 construction runner。

### 18.6 测试是否覆盖可执行需求

通过。每项 schema/digest/migration/affinity/deadline/terminal/cleanup/qualification/promotion 要求都有
red-capable owner test；跨 root 行为由 fresh A/B 捕获；Gate H production claim 明确不在本 repair 中声明。

### 18.7 是否存在机械重复验证

没有。R-D 完整 `editor_core` suite subsume 之前 filter；R-F 只跑新增 v2 owner filter与 fresh integration；
R-G 复用未失效证据。`runtime_cli` 只有实际修改时才跑，Local CI 和完整 workspace 均不运行。

### 18.8 外部状态是否受保护

通过。Window A 仅测试临时目录，Window B 仅新 repository-external role root。production/真实 cache/config
全部禁止；recursive cleanup 使用 strict-child/reparse/process/evidence 约束。

### 18.9 当前 dirty worktree 是否被保护

通过。第 6 节记录字节基线并声明其非 clean qualification；激活前重算，所有已有改动视为用户所有，
禁止 reset/revert/clean。

### 18.10 激活机制是否违反唯一施工文档规则

没有。本文在 `待执行/` 时不可施工；激活时将 Gate 合入唯一的 282 当前文档，并把独立计划移入历史，
不会产生第二份 `当前/` 文档，也不会引用 `待执行/` 作为执行真相。

### 18.11 是否需要修订正式方案

不需要。施工文档没有改变方案 B 的 schema、deadline、portability、authority 或 Gate H 边界，只把实现、
测试、激活和清理步骤具体化。

### 18.12 其它 AI 审查

本轮没有用户指定的 282-R1 外部审查文档。已读取项目规则、施工规则、282/282-R1 方案、282 当前施工
文档、Gate H 失败记录与现有 owner/consumer code，没有需要回填正式方案的冲突。

### 18.13 自审结论

```text
施工文档自审：通过
正式方案需修订：否
当前文档位置：施工文档/待执行/
当前状态：已生成、已自审、未激活、不可施工
当前施工/测试授权：无
当前施工槽：282 父文档继续占用
下一步：等待用户单独授权 282-R1 Repair Window A / Gate R-A 至 R-D，并执行激活前复核
禁止：直接修改代码、运行 repair Gate、Window B、Local CI、production/真实 cache/config mutation、Gate H
```

## 19. 本轮生成记录

```text
本轮只生成并自审本文。
未修改 Rust/source/test code。
未运行 cargo test/check/build、Local CI 或任何 repair Gate。
未创建 fresh role root。
未修改 production/installed binary、真实 cache、真实配置或旧 Gate H 证据。
未激活本文，未改动 282 当前施工文档的执行内容。
```
