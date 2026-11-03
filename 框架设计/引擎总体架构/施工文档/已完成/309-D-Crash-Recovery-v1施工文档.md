# 309-D Crash Recovery v1 施工文档

> 上级方案：`../../309-Headless-AuthoringProjectContext-Unified-Mutation-v1方案.md`。  
> 前置完成：`../已完成/309-A-Headless-Neutral-Read-Authority-v1施工文档.md`、`../已完成/309-B-Operation-Bound-Immutable-Snapshot-Lease-v1施工文档.md`、`../已完成/309-C-Unified-Mutation-Authority-v1施工文档.md`。  
> 当前状态：2026-08-31T18:42:57+08:00 Gate A-C、终态验证、完成记录与归档已完成。  
> 用户授权：2026-08-31 用户明确要求修正309状态、生成并自审309-D施工文档，再激活施工。  
> 禁止恢复：Tower P1-2 为 `stopped_by_user`，不属于本施工。  

## 1. 目标

在309-C已经存在的OS mutation lane、sealed journal、before material和确定文件操作基础上，完成Stage D最小
crash recovery：进程在prepared、applying、部分/全部after-write或journal committed但receipt尚未发布时退出，
下一次Headless open必须在同一project-scoped authority lock内得到唯一、可重复、fail-closed的结果。

允许的完成声明仅为：

```text
309-D Crash Recovery sourceAvailable
open能够把可证明的incomplete transaction收敛为aborted、committed或restored_before
unknown touched-path bytes、journal/material tamper进入recovery_required并阻断新commit
```

不得宣称Stage E EditorProjectAdapter、Stage F Gateway authority retirement、Provider/Compiler接入、production
可用、机器断电/文件系统损坏恢复或安装态迁移。

## 2. 必要性与验证经济性

```text
requested claim：生成、自审并激活309-D Crash Recovery施工文档
confirmed gap：309-C只阻断nonterminal journal；open不会识别并确定finalize/compensate
minimum causal change：在authoring_project_context内部增加单一recovery owner，并让open在发布handle前持锁执行
必须修改：mutation.rs、context.rs、diagnostic.rs、lib.rs、tests.rs
明确排除：Editor/Gateway/Compiler/Provider、daemon、后台恢复服务、第三consumer、production和安装态
Gate A -> recovery绕过OS lane或误信tampered journal -> owner classifier/lock红测试 -> bounded recovery report
Gate B -> prepared/applying/after-write崩溃留下半提交 -> child-process crash红测试 -> deterministic reopen outcome
Gate C -> unknown bytes被覆盖或committed无receipt静默通过 -> tamper/finalize红测试 + affected closure -> terminal evidence
复用：309-C journal/before material/root binding/atomic writer/receipt sealing/OS lock与29/29 owner证据
失效：journal state/binding、before/after classification、open顺序、receipt seal或recovery lock变化
预计成本：3 Gate、1 owner crate加2个直接consumer回归、约120分钟
停止条件：需要Stage E/F、持久daemon、真实Editor/Gateway、on-disk通用迁移框架、第三consumer或预计超过3小时
```

进程崩溃测试复用当前Rust unit-test executable作为受控child，不新增常驻runner、独立测试产品或真实项目。
本阶段模拟process exit/abort，不把突然断电、磁盘控制器丢写、跨卷原子性或恶意管理员修改纳入完成声明。

## 3. 冻结恢复合同

### 3.1 open顺序与唯一authority

`open`在发布`ProjectHandle`和首次refresh之前：

```text
canonicalize root -> acquire project OS authority lock -> inspect authority transactions
-> classify/recover or fail closed -> release lock -> validate manifest -> publish handle -> refresh
```

- recovery必须使用309-C同一`.aife/authoring/authority.lock`，不能新增`recovery.lock`；
- journal中的project identity和root binding必须验签并绑定当前canonical root；
- recovery未形成terminal结果时不得发布handle、refresh旧事实或允许新commit；
- 保持现有`open(...) -> ProjectHandle`兼容；允许新增一个返回structured recovery result的深open report入口，
  但旧open必须委托同一实现，调用方不得手工编排scan/classify/apply步骤；
- `DiagnosticStage`增加`Recovery`，成功与失败结果必须可结构化区分，不靠日志文本猜测。

### 3.2 deterministic classification

对每个sealed journal，基于declared touched paths、sealed before digest/material和deterministic after digest分类：

```text
all touched paths == before
  -> terminal aborted；不生成mutation receipt

all touched paths == after
  -> 证明own writes之外没有被吸收的canonical change后补全after revision与sealed receipt；terminal committed
  -> 若存在unrelated external drift，安全补偿own after bytes到before并保留外部bytes；terminal restored_before

mixture of exact before/after only
  -> 只恢复仍属于该mutation的exact after bytes；验证own paths回到before；terminal restored_before

any touched path != before && != after
  -> 不写该路径；terminal recovery_required并阻断open/commit
```

恢复动作必须幂等。恢复过程自身再次崩溃后，下一次open仍使用同一journal重新分类，不依赖进程内flag或
“已经尝试过”计数。

### 3.3 journal与receipt边界

- 增加明确terminal `aborted`状态；`rolled_back`继续表示已恢复before的terminal事务；
- prepared/applying必须验证journal binding、snapshot集合、before material digest、mutation contract和root binding；
- committed journal但receipt缺失时，只有journal、after bytes和after revision全部可证明才补发相同确定receipt；
- receipt存在但内容tamper、journal tamper、material缺失/tamper或多个冲突nonterminal journal均fail-closed；
- recovery不得删除journal、before material、未知外部文件或仍承诺rollback的材料；
- 不建立journal GC、retention daemon、历史receipt迁移或管理UI。

### 3.4 crash checkpoints

测试必须覆盖真实child process在以下durable checkpoint直接退出，不运行Rust destructor或普通补偿：

```text
prepared journal sealed
applying before first operation
after one operation / partial apply
after all operations before verification
committed journal sealed before receipt publication
```

可以为既有`MutationCommitProgress`增加最小checkpoint事件，以便测试和受控owner故障注入；不得把checkpoint
扩展成AI-facing workflow、通用transaction scripting或production crash控制接口。

## 4. Owner与实际文件

```text
rust/crates/authoring_project_context/src/mutation.rs    recovery classifier、动作、journal/receipt terminal seal
rust/crates/authoring_project_context/src/context.rs     open持锁恢复、兼容open与structured report入口
rust/crates/authoring_project_context/src/diagnostic.rs  Recovery diagnostic stage
rust/crates/authoring_project_context/src/lib.rs         最小report/disposition/checkpoint re-export
rust/crates/authoring_project_context/src/tests.rs       owner red/green、child crash、tamper与幂等证据
```

如实现必须修改`editor_core`产品源码、Gateway、Editor、Compiler、Provider、Runtime、Build、项目样例、Cargo
dependency或新增crate，停止并修订本文。`editor_core`只作为两个既有consumer的回归验证，不是施工owner。

## 5. Gate A：Recovery Classification与Open Authority

先写`recovery_open_`和`recovery_classification_` owner tests并确认因恢复能力缺失而红：

- open在handle/refresh发布前取得与mutation相同OS lock；另一process持锁时返回typed busy/unavailable；
- sealed prepared且all-before -> aborted；正常open成功，source revision不变；
- applying partial且paths只为exact before/after -> restored_before；正常open成功；
- all-after且无外部drift -> committed并补全相同确定receipt；
- all-after但存在write-set外canonical drift -> 不吸收外部bytes，补偿own writes并报告restored_before；
- report按journal canonical path确定排序并绑定mutation id、prior state、outcome和diagnostics；
- 恢复后再次open不重写terminal journal/receipt，不改变canonical revision。

命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib recovery_open_
cargo test -p authoring_project_context --lib recovery_classification_
```

## 6. Gate B：Real Child-process Crash Checkpoints

先写`recovery_crash_` tests：

- child在prepared checkpoint直接退出，parent reopen得到aborted；
- child在applying before first write退出，parent reopen得到aborted；
- 多文件mutation在第一operation后退出，parent reopen精确恢复before；
- child在all-write/before-verification退出，parent reopen确定finalize或安全恢复，不留半提交；
- child在committed-journal/before-receipt退出，parent reopen补发sealed receipt；
- 每个场景reopen两次，第二次结果terminal且bytes/revision/receipt稳定；
- child退出后OS authority lock由生命周期释放，无永久busy。

命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib recovery_crash_
```

测试fixture和child mode均由owner测试持有。禁止启动Editor/Gateway、修改真实项目或创建仓库外长期状态。

## 7. Gate C：Unknown/Tamper与终态回归

先写`recovery_fail_closed_` tests：

- touched path出现neither-before-nor-after bytes -> `recovery_required`，未知bytes保持不变；
- journal binding/schema/root/project tamper、before material tamper或缺失 -> open失败且不写项目；
- 两个冲突nonterminal journal -> fail-closed，不猜测处理顺序；
- recovery action中途再次退出 -> 下一次open幂等收敛或保持recovery_required；
- recovery_required阻断后续commit/rollback，terminal aborted/committed/restored允许正常新commit；
- 309-A/B/C owner行为及两个现有consumer保持通过。

终态命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib
cargo test -p editor_core controlled_source_patch
cargo test -p editor_core project_asset_import
cargo fmt --check -p authoring_project_context
cargo check -p authoring_project_context -p editor_core
```

完整owner suite已包含Gate A-C过滤测试，终态不机械重复过滤命令。`project_candidate_entry` receipt schema未改、
`project_write_scope`未改，因此不重复运行；若施工实际改变任一公共receipt形状或其直接consumer，则先修订本文并
加入对应回归。禁止运行完整workspace、Local CI、production composition、真实配置或安装态。

## 8. 激活前复核

```text
activationTime：2026-08-31T16:26:07+08:00
HEAD：acaffa8cb95bbb54d093bbf45835a59315c82da5
branch：codex/255-capability-catalog
rustc/cargo：1.96.0
309方案SHA-256：ED25544C77C0A168DCEEB2EBBB93D9F80514A36D2FD50885B0E40AE94C6CB37A
context.rs：DE9949C0D31F1096DED616991A8205A30509456192C051E2D278EA928D4A4926
mutation.rs：ED829DE797D50FD55AD7EBE212B12D860163FAF46AEB10507F3A05BA84865FB1
diagnostic.rs：2ACAB0F55F82E2C80AD47F0E51131DC7B221D8F840AFB3073381A0D231B2D6B8
lib.rs：EE1582B577E7D9CCC4F460D2440EEDA936414D6551F3C2F75FAFCCB47DC9C171
tests.rs：38D81BA33994B5A4C2921BFB59E982EF4BD6DFF5D7B1FF1BA6C4E565AA0FF11D
激活复核时当前施工槽：空；当前目录中的254文件仅为历史evidence，不是施工文档
待执行队列：本文；其它全局与Tower待执行文档为空
Tower P1-2：stopped_by_user；禁止恢复
重叠修改：309-A/B/C源码和文档为当前dirty worktree中的已完成用户工作，必须继承且不得reset/revert
preflightResult：PASS；正式方案、Stage C源码身份、唯一空槽、测试owner和施工边界均匹配
```

## 9. 自审

```text
方案一致性：通过；只施工309正式Stage D与G6，不改Stage E/F
范围完整性：通过；open authority、五个crash checkpoint、finalize/restore/unknown/tamper均有Gate
不过量：通过；复用309-C journal、material、lock、writer和receipt，不新增daemon/crate/runner/第三consumer
深Module：通过；调用方只open并读取report，不编排journal扫描、分类或恢复动作
AI边界：通过；Context仍是Provider内部Module，没有AI可见Tool、二级Host、catalog/execute或Agent
兼容性：通过；保留现有open返回handle入口，structured report为同一owner的附加入口
红能力：通过；owner-level fixture与真实child exit能够捕获静默半提交、误finalize和覆盖未知bytes
验证经济性：通过；单owner完整suite加两个直接consumer，不运行workspace/Local CI/production
外部状态：通过；只使用测试temp fixture和受控child process，不修改真实项目或配置
停止条件：通过；Stage E/F、通用迁移/daemon、第三consumer、外部状态或超过3小时均要求停止复核
正式方案状态修正：通过；只把顶部施工事实更新为A-C已完成、D-F未施工，未改变架构决定
```

## 10. 施工结果与证据

309-D 已在冻结边界内完成。`EmbeddedAuthoringProjectContext::open` 保持原返回类型并委托
`open_with_report`；两者都会在读取manifest、发布handle和首次refresh前，通过309-C同一OS authority lock
调用单一recovery owner。恢复报告按journal canonical path排序，表达`no_recovery`、`aborted`、`committed`
或`restored_before`；未知bytes和tamper通过`DiagnosticStage::Recovery` fail-closed。

```text
Gate A red：ProjectRecoveryDisposition与open_with_report不存在，过滤测试因API缺失编译失败
Gate A green：clean open、同一OS lock、all-before、partial、all-after与外部漂移分类通过
Gate B green：prepared、applying-before-write、partial、all-write-before-verify、committed-before-receipt五个真实child exit通过
Gate C green：unknown touched bytes、journal/material/receipt tamper、冲突journal、恢复过程再次exit均通过

authoring_project_context完整suite：41 passed
controlled_source_patch：15 passed，1 ignored（既有local trusted workspace SDK smoke）
project_asset_import：13 passed
cargo fmt --check -p authoring_project_context：passed
cargo check -p authoring_project_context -p editor_core：passed
```

`project_asset_import`首次终态运行暴露既有consumer错误类别兼容：篡改统一journal时，open-time recovery先于
rollback owner发现decode失败。修复只在recovery owner内将journal/receipt decode失败归一到309-C既有
`rollback_journal_invalid`/`rollback_receipt_invalid`类别，并保留Recovery stage；失败用例定向转绿后，owner与
两个完整consumer过滤suite均重新运行并通过。

实现新增`Aborted` journal terminal、prepared/committed两个最小commit checkpoint、确定receipt补发、
before/after/unknown分类、write-set外漂移保护和恢复动作checkpoint。恢复自身在第一条path恢复后直接退出的
child测试证明下一次open仍可从磁盘事实幂等收敛。没有新增daemon、crate、依赖、Editor/Gateway接入或第三consumer。

源码身份：

```text
HEAD：acaffa8cb95bbb54d093bbf45835a59315c82da5
branch：codex/255-capability-catalog
evidence：dirty-worktree development/source evidence，不是clean exact-commit或release资格
context.rs：254E44EE79570A0163BEABCD7DCF369CA390ED619B47BAEBE7C6B7938610C076
mutation.rs：AB39D7FD03918BEDFB91BAE6484C0057E12FDB5E955B78467C56118B0AC2F057
diagnostic.rs：CEBAF8A604C1D384B8003FECEFFAD0442068D9B4C46BB008D0BDB599B8E73065
lib.rs：8657E7C3C91F1CC2BBDE908AFF0DFF77C310B2BC1874CBBB5E449777D8BE2B99
tests.rs：717E8BBBB3B9B01183F0FBBB38C8DF202E42652769D829883E6F8FA1025563C0
```

只使用owner管理的临时fixture和当前test executable child process；child退出后OS lock由生命周期释放，fixture由
测试清理。未启动Editor/Gateway/Compiler，未运行完整workspace、Local CI、production、真实项目、真实配置或
安装态。`cargo check`只有既有`engine_runtime`三条dead-code warning。Tower P1-2保持`stopped_by_user`。

## 11. 完成与归档

全部Gate和终态验证通过后：

- 写`阶段完成记录/2026-08-31-309-D-Crash-Recovery-v1/00-总览.md`；
- 本文记录红/绿首因、实际文件、测试、遗漏项和外部状态后移到`施工文档/已完成/`；
- 同步README、地图、阅读顺序、49、54、施工/完成记录入口；
- 当前施工槽置空；不自动进入Stage E，不恢复Tower P1-2。
