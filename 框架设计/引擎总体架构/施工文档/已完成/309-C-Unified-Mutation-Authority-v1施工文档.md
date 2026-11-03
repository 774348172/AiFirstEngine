# 309-C Unified Mutation Authority v1 施工文档

> 上级方案：`../../309-Headless-AuthoringProjectContext-Unified-Mutation-v1方案.md`。  
> 前置完成：`../已完成/309-A-Headless-Neutral-Read-Authority-v1施工文档.md`、`../已完成/309-B-Operation-Bound-Immutable-Snapshot-Lease-v1施工文档.md`。  
> 当前状态：2026-08-31 Gate A-D 与终态验证通过；已完成并归档。  
> 用户授权：2026-08-31 用户明确要求“先完成施工文档，然后再施工 stageC”。  
> 禁止恢复：Tower P1-2 为 `stopped_by_user`，不属于本施工。  

## 1. 目标

在 309-A/B 的 canonical revision 与 immutable snapshot 基础上，建立唯一、领域无关的
`ProjectMutation` 提交 authority：所有 Engine 参与者用同一个 project-scoped OS 生命周期锁、expected
revision CAS、write-ahead journal、before material、sealed receipt 和 exact-revision rollback 合同提交确定
文件操作。随后将 `ControlledSourcePatch` 与 `ProjectAssetImport` 的真实 apply/rollback 迁入该 lane。

允许的完成声明仅为：

```text
309-C Unified Mutation Authority sourceAvailable
ControlledSourcePatch 与 ProjectAssetImport 已共享唯一 mutation lane
并发 Engine commit、expected revision drift 和 commit 窗口未知外部写入 fail-closed
```

不得宣称 Stage D crash recovery、Editor/Gateway authority retirement、AI Tool 接入或 production 可用。

## 2. 必要性与验证经济性

```text
requested claim：完成施工文档并直接施工309 Stage C
minimum causal change：在neutral owner中新增一个ProjectMutation commit/rollback深Module，并让两个既有consumer降低成同一合同
必须修改：authoring_project_context mutation/lock/context/export/tests；两个editor_core consumer及其owner tests
明确排除：Stage D自动finalize/compensate、Editor/Gateway/Compiler/Provider、daemon、production和安装态
Gate A -> mutation可表达自由文本、越界路径或未绑定before -> schema/contract红测试 -> bounded prepared mutation
Gate B -> 两进程同时提交、旧revision提交或未知外部写入被吸收 -> owner concurrency/CAS/fault红测试 -> authority receipt/diagnostic
Gate C -> 两个旧consumer仍持有独立lock/transaction authority -> consumer migration红测试 -> single-lane evidence
Gate D -> rollback覆盖intervening write或receipt/material tamper -> exact-revision rollback红测试 + affected suite -> terminal evidence
复用：309-A canonical inventory/revision、309-B Context handle、既有candidate validation、既有per-file atomic writer经验
失效：mutation schema、canonical revision/source policy、OS lock、journal/receipt binding或两个consumer apply/rollback变化
预计成本：4 Gate、2 crates、约150分钟；不运行workspace/Local CI/production composition
停止条件：预计超过3小时；需要daemon、真实Editor/Gateway、Stage D自动恢复、新通用workflow或第三个consumer时停止并复核
```

Windows OS lock 使用 workspace 已经采用的 `windows-sys 0.59`，只增加本 crate 的直接 target dependency；
Rust 标准库没有跨进程生命周期文件锁，所以这是满足正式方案而非泛化依赖。非 Windows 只实现同语义的最小 Unix
advisory lock；不建立 lock-file 存活推断协议。

## 3. 冻结合同

### 3.1 `ProjectMutation`

必须结构化绑定：

```text
mutationId / domain
expectedRevisionId
validationDigest
declaredReadSet / declaredWriteSet
expectedBeforeHashes
canonical create_or_replace / delete / move operations
```

要求：

- opaque id、digest、路径、数量和总 bytes 有界；
- operation 只能触达 declared write set，move 的 source/target 都必须声明；
- write set 中每一路径都必须有 expected before hash，缺失文件使用显式 `missing`；
- canonical source path 进入 revision diff；`Library` 等派生路径虽不进入 source revision，仍受 before hash 和 write set 约束；
- mutation 不包含自然语言意图、领域 schema、approval 或 validation 实现。

### 3.2 authority commit

固定顺序：

```text
validate -> acquire OS lock -> block nonterminal journal -> refresh/CAS
-> verify before hashes -> seal prepared journal + before material -> applying
-> atomic per-file apply -> verify exact after hashes and source diff
-> publish after revision -> seal receipt -> committed -> release lock
```

- `.aife/authoring/authority.lock` 是 OS lock carrier，不以文件存在判断 owner 活性；
- 控制材料进入 `.aife/authoring/transactions` 与 `receipts`，不进入 canonical source revision；
- 同进程和跨进程竞争只能得到一个 commit winner；loser 返回 busy 或 revision drift，不得合并旧 candidate；
- apply 后出现 write set 外 canonical change，只有当 own paths 仍精确匹配 sealed after bytes 时才补偿 own writes；
- 同路径出现未知 bytes、material tamper 或补偿不安全时标记 `recovery_required` 并阻断后续 commit；
- Stage C 只识别并阻断非终态 journal，不做 Stage D 的 crash 自动判定、finalize 或 compensate。

### 3.3 receipt 与 rollback

- receipt 绑定 mutation/project/root/before revision/after revision/validation/write set/journal/material digest；
- rollback 进入同一 OS lane，要求 current revision 精确等于 receipt after revision；
- before material和receipt binding必须未 tamper；
- rollback按反向顺序原子恢复，验证 before revision后封印 rollback receipt并把 journal标为`rolled_back`；
- 任一条件不满足 fail-closed，不做 best-effort merge。

## 4. Owner 与实际文件

```text
rust/crates/authoring_project_context/Cargo.toml                  OS lock direct target dependency
rust/crates/authoring_project_context/src/authority_lock.rs      project-scoped OS lifecycle lock
rust/crates/authoring_project_context/src/mutation.rs            contract、journal、commit、receipt、rollback
rust/crates/authoring_project_context/src/context.rs             handle/root/revision bound methods
rust/crates/authoring_project_context/src/diagnostic.rs          commit/rollback diagnostic stage
rust/crates/authoring_project_context/src/lib.rs                 最小公开re-export
rust/crates/authoring_project_context/src/tests.rs               owner red/green、CAS、fault和rollback证据
rust/crates/editor_core/src/controlled_source_patch.rs           candidate -> ProjectMutation，旧apply/rollback API兼容
rust/crates/editor_core/src/project_asset_import.rs              candidate -> ProjectMutation，移除独立import authority
rust/crates/editor_core/src/project_write_scope.rs               删除已无consumer的create-new lock-file authority
```

`project_write_scope.rs` 的原子文件写能力保留；旧 `acquire_lock` 在两个指定consumer迁移后已无生产调用，因此随
Gate C删除，避免形成可误用的第二authority。若实现必须修改 Gateway、Editor、Compiler、Provider、Runtime、
Build、项目样例或第三个 mutation consumer，停止并修订本文。

依赖方向保持：

```text
editor_core -> authoring_project_context
```

禁止 `authoring_project_context -> editor_core`。

## 5. Gate A：Mutation Contract

先写 `mutation_contract_` owner tests 并确认因 API/行为缺失而红：

- create/replace/delete/move 合同及 deterministic binding；
- path escape、case collision、duplicate operation、undeclared write、缺少 before hash、无效 digest 被拒绝；
- operation count、单文件和总 bytes 上限；
- source 与 excluded derived path 均可显式声明，但 `.aife/authoring` 永远禁止作为 mutation target。

命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib mutation_contract_
```

## 6. Gate B：OS Lane、CAS、Journal 与 Commit

先写 `mutation_commit_`、`mutation_os_lock_` 和 `mutation_unknown_write_` tests：

- 正常多文件 commit 生成 after revision、journal和sealed receipt；
- stale expected revision 在任何 canonical write 前失败；
- 两个Context线程和两个进程竞争同一expected revision，最多一个成功；
- Engine lock 随 process/file handle 生命周期释放，不靠删除 lock carrier；
- write set 外外部写入触发 own-write compensation且不覆盖外部 bytes；
- 同路径未知 bytes 进入 `recovery_required`；
- 已存在非终态/recovery journal阻断新 commit。

命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib mutation_commit_
cargo test -p authoring_project_context --lib mutation_os_lock_
cargo test -p authoring_project_context --lib mutation_unknown_write_
```

测试使用 run-owned temp fixture和受控 fault hook，不启动 Editor/Gateway，不修改真实项目。

## 7. Gate C：Existing Consumer Migration

先增加能证明 delegation 的 consumer tests：

- `ControlledSourcePatch` 从 validated candidate 构造 expected revision、before hash和确定 operations；
- `ProjectAssetImport` 同样降低 source + AssetPipeline derived operations；
- 两者共享 `.aife/authoring/authority.lock` 和 receipt/journal owner，不再使用独立 import lock；
- 保持现有 public prepare/validate/apply/rollback 调用形状、既有错误映射和上层 candidate envelope；
- 两类 candidate 并发时遵守同一 CAS/lane，旧 candidate loser fail-closed；
- 既有 failure compensation和rollback测试改由统一 owner通过，不保留第二套真实事务 authority。

命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core controlled_source_patch
cargo test -p editor_core project_asset_import
cargo test -p editor_core project_candidate_entry
```

`project_candidate_entry` 是两个 receipt 的直接 consumer，属于必要 affected closure；同配置下不额外重复过滤测试。

## 8. Gate D：Exact Rollback 与终态回归

- exact after revision rollback恢复before revision并生成 rollback receipt；
- intervening source/derived write、receipt/journal/before material tamper均在写入前拒绝；
- rollback失败不覆盖未知 bytes并进入明确 recovery 状态；
- owner、直接consumer、格式和受影响check通过。

命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib
cargo test -p editor_core controlled_source_patch
cargo test -p editor_core project_asset_import
cargo test -p editor_core project_candidate_entry
cargo fmt --check -p authoring_project_context -p editor_core
cargo check -p authoring_project_context -p editor_core
```

终态 owner suite 已包含 Gate A/B 的过滤测试，不重复运行。未修改其它 crate 公共合同，不运行 workspace、Local CI、
production Editor/Gateway、真实配置、安装态或跨平台 release matrix。

## 9. 激活前复核

```text
activationTime：2026-08-31T14:30:56+08:00
HEAD：acaffa8cb95bbb54d093bbf45835a59315c82da5
branch：codex/255-capability-catalog
rustc/cargo：1.96.0
309方案SHA-256：EDC669497552A1BDBC5A5BEB3671E29B7C3F2535E6D8B1CE122A5825A6AB66F1
context.rs：FCE37D2F9EDF94FC462A28ABC050AA0738AAF17A4ECAB28FEB36867929530291
lib.rs：005323C9626E9CDD4017E89A1438DC63B7652C3FBFA605E21DA2D3CDC6850C05
tests.rs：DEB07018AFA79B6DF13AF219589811C11A195978C325BFD37B232A12C96511BC
controlled_source_patch.rs：0976161ACF2973789634CD24CDAA0B156DDB27434863B1950311AF5F638DB104
project_asset_import.rs：F2DE5FB3D320DE2BFE43B595440FD102F08A48996A6AB22C0A7029B399FE6F1B
project_write_scope.rs：78CDF78DCDA7BD822C0297158CF3BBFAEE13D30677131789F41D03A9F0DE9879
当前施工槽：空；当前目录中的254文件仅为历史evidence，不是施工文档
待执行队列：空
Tower P1-2：stopped_by_user；禁止恢复
重叠修改：309-A/B源码和文档为当前dirty worktree中的已完成用户工作，必须继承且不得reset/revert
preflightResult：PASS；Stage C正式方案、前置源码、dependency方向和测试owner均存在
```

## 10. 自审

```text
方案一致性：通过；只施工309正式Stage C，Stage D-F均排除
范围完整性：通过；contract、OS lock/CAS、journal/receipt、两个指定consumer和rollback均有Gate
不过量：通过；不新增daemon、Agent、Tool、workflow、第三个consumer或production流程
深Module：通过；领域owner只产生确定mutation，Context隐藏lock/journal/apply/compensation细节
AI边界：通过；Context仍是Provider内部Module，没有AI可见二级Host或catalog/execute
红能力：通过；每个失败合同都有owner或direct-consumer红测试
验证经济性：通过；两crate owner/consumer closure，不运行完整workspace/Local CI
外部状态：通过；只使用测试fixture和测试子进程，不触碰真实项目、配置、Editor或安装态
依赖审计：通过；保持editor_core -> authoring_project_context；windows-sys为标准库缺失OS锁的最小既有依赖
停止条件：通过；Stage D恢复、第三consumer、production接入或超过3小时均要求停止复核
```

## 11. 施工结果与证据

309-C 已按冻结边界完成。`authoring_project_context` 现在拥有唯一的 project-scoped mutation lane；
`ControlledSourcePatch` 与 `ProjectAssetImport` 只负责把已经验证的 candidate 降低为确定
`ProjectMutation`，真实 commit、补偿与 rollback 均委托 neutral owner。旧 consumer receipt JSON 形状保持
兼容，rollback 通过既有 journal path 与 digest 重新加载并验签统一 receipt。

```text
Gate A red：mutation contract API/校验缺失，新增contract tests能够打红
Gate A green：create/replace/delete/move、bounded write set、before hash与control-path合同通过
Gate B red：缺少跨进程lane、CAS、journal与未知外部写入fail-closed
Gate B green：双Context与双进程竞争最多一个winner；stale CAS、补偿、recovery blocking通过
Gate C red：两个consumer仍持有独立transaction/lock authority
Gate C green：ControlledSourcePatch与ProjectAssetImport共享authoring authority；旧public receipt schema不变
Gate D green：exact-revision rollback、receipt/journal/material tamper与intervening write全部fail-closed

authoring_project_context完整suite：29 passed
controlled_source_patch：15 passed，1 ignored（既有local trusted workspace SDK smoke）
project_asset_import：13 passed
project_candidate_entry：10 passed
project_write_scope：5 passed
cargo fmt --check -p authoring_project_context -p editor_core：passed
cargo check -p authoring_project_context -p editor_core：passed
git diff --check（Stage C affected files）：passed
残留authority扫描：旧acquire_lock、import.lock和旧rollback transaction owner均不存在
```

`cargo check` 只报告 3 个既有 `engine_runtime` dead-code 基线警告：`IrInterpreter`、
`validation_only_ir_interpreter` 与 `remove_sprite_renderer2d`。Stage C 新增源码没有新 warning。

源码身份：

```text
HEAD：acaffa8cb95bbb54d093bbf45835a59315c82da5
branch：codex/255-capability-catalog
evidence：dirty-worktree development/source evidence，不是clean exact-commit或release资格
authority_lock.rs：AEE9436EB829DBDEB2B4EFC40F0C786CB438A1431CCDF0F2C605509CFC76E15A
mutation.rs：ED829DE797D50FD55AD7EBE212B12D860163FAF46AEB10507F3A05BA84865FB1
context.rs：DE9949C0D31F1096DED616991A8205A30509456192C051E2D278EA928D4A4926
diagnostic.rs：2ACAB0F55F82E2C80AD47F0E51131DC7B221D8F840AFB3073381A0D231B2D6B8
lib.rs：EE1582B577E7D9CCC4F460D2440EEDA936414D6551F3C2F75FAFCCB47DC9C171
tests.rs：38D81BA33994B5A4C2921BFB59E982EF4BD6DFF5D7B1FF1BA6C4E565AA0FF11D
controlled_source_patch.rs：767EDFAE554941E76D61A88D11CB6AA3FEDF9BE7FE9044936BE23DB941EF43BB
project_asset_import.rs：56F2ECD430818E2DEF3A667462E2D6F4CB068C7CF554AA6B273F4EB55D9077DA
project_write_scope.rs：43DA932366C05CF4A6B66C13E81EE989C86A463D7460AD4CFA943014FEC5BC09
```

未启动 Editor、Gateway 或 Compiler，未运行完整 workspace、Local CI、production composition、安装态或
真实配置。测试只使用 owner 管理的临时 fixture 与受控测试子进程。Stage D crash recovery/finalize/
compensate、后续 consumer 接入与 authority retirement 继续排除。

## 12. 完成动作

Gate A-D 与终态验证通过后：

- 写 `阶段完成记录/2026-08-31-309-C-Unified-Mutation-Authority-v1/00-总览.md`；
- 本文移到 `施工文档/已完成/`；
- 同步 README、地图、阅读顺序、49、54、施工/完成记录入口；
- 当前施工槽置空；不自动进入 Stage D，不恢复 Tower P1-2。
