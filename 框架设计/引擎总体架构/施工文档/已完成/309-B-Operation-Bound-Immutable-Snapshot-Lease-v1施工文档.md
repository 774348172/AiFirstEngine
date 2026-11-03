# 309-B Operation-bound Immutable Snapshot Lease v1 施工文档

> 上级方案：`../../309-Headless-AuthoringProjectContext-Unified-Mutation-v1方案.md`。  
> 前置完成：`../已完成/309-A-Headless-Neutral-Read-Authority-v1施工文档.md`。  
> 当前状态：2026-08-31 Gate A-B与终态验证通过；已完成并归档。  
> 用户授权：2026-08-31 用户明确要求“直接施工309下一步”。  
> 排序解释：309正式顺序的下一步是Stage B，不跳到Stage C mutation。  

## 1. 目标

在309-A bounded owned-bytes snapshot之上建立最小operation-bound snapshot lease：长操作持有的lease只读取
已经封印的snapshot materialization，不回读live project root；lease由Rust所有权保留，显式release或调用方
取消/超时导致的drop释放内存材料。

允许的完成声明仅为：

```text
309-B Operation-bound Immutable Snapshot Lease sourceAvailable
309 Stage B仅在bounded in-memory materialization范围内完成
```

## 2. 必要性与验证经济性

```text
requested claim：直接施工309下一步
minimum causal change：为现有ProjectSnapshot增加独立RAII lease owner、identity、retention与release合同
必须修改：snapshot.rs、context.rs、lib.rs、tests.rs及入口/完成文档
明确排除：disk materialization、content-addressed blob、TTL timer、后台GC、daemon、Compiler/Provider接入、mutation
Gate A -> lease仍回读live root或混用revision -> owner red test -> immutable lease evidence
Gate B -> release/identity/retention不确定 -> lifecycle tests +完整owner suite -> operation-bound evidence
复用：309-A inventory/revision/snapshot 16/16与neutral dependency边界
失效：snapshot schema、lease owner、context lease-id分配或owned materialization变化
预计成本：单crate、两组owner测试，小于90分钟
停止条件：需要磁盘store、OS lock、跨进程lease、Compiler改动或新第三方依赖时停止并修订
```

## 3. 冻结边界

### 3.1 必须实现

- 新增process-local opaque `SnapshotLeaseId`，不能序列化为跨进程authority；
- 新增`SnapshotRetentionPolicy::OperationBound`；
- lease拥有完整`ProjectSnapshot` materialization，不保存或公开live root；
- 同一Context分配不同lease identity；
- S1 lease取得后live root进入R2，S1 bytes/revision仍保持R1，新lease绑定R2；
- 显式`release(self)`返回结构化report；取消/超时依赖Rust drop释放同一owned materialization；
- lease不持有mutation lane，不阻塞后续`refresh`；
- acquire期间发现309-A snapshot drift继续fail-closed。

### 3.2 明确禁止

- 不建立磁盘snapshot目录、candidate tree copy、hardlink、blob store、dedupe或GC；
- 不建立wall-clock TTL、后台线程、timer、lease daemon或跨进程lease registry；
- 不接入Compiler、Provider Tool、Editor、Gateway、Runtime、Build或Export；
- 不实现commit/rollback、OS lock、CAS、journal、receipt或recovery；
- 不修改309-A source inventory、revision或legacy candidate digest协议；
- 不新增第三方依赖、runner、Local CI或production验证。

## 4. Owner与文件

```text
rust/crates/authoring_project_context/src/snapshot.rs  lease类型、owned materialization、release report
rust/crates/authoring_project_context/src/context.rs   process-local lease id分配与acquire入口
rust/crates/authoring_project_context/src/lib.rs       最小公开re-export
rust/crates/authoring_project_context/src/tests.rs     owner red/green与lifecycle证据
```

如果必须修改`editor_core`、Gateway、Compiler或Cargo dependency，立即停止，不以“真实消费”为由扩大本施工。

## 5. Gate A：Immutable Lease Binding

先写`lease_` owner测试并确认因lease API/行为缺失而红：

- acquire S1后改写live root，S1仍返回原bytes与R1；
- Context refresh到R2不改变S1，S2绑定R2；
- lease只包含owned `ProjectSnapshot`，无project root accessor；
- drifted live source不能取得冒充R1的lease。

命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib lease_
```

## 6. Gate B：Lifecycle与终态

- 同一Context连续acquire得到不同opaque lease id；
- retention固定为`operation_bound`；
- `release(self)`返回lease id、snapshot id、revision id和`released` outcome；
- 未显式release的lease可安全drop，不写项目或外部状态；
- 完整owner suite、格式与受影响check通过。

命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib
cargo fmt --check -p authoring_project_context
cargo check -p authoring_project_context
```

完整suite包含Gate A/B，终态不重复运行过滤测试。没有共享consumer变化，因此不运行`editor_core`。

## 7. 激活前复核

```text
activationTime：2026-08-31T12:38:42+08:00
HEAD：acaffa8cb95bbb54d093bbf45835a59315c82da5
branch：codex/255-capability-catalog
rustc/cargo：1.96.0
309方案SHA-256：ADBD2D8BA69D2BC0D78A5D235CAB2FC9DC44A35E11A86EA8765182ED57FF43B2
snapshot.rs：E6401336022F563E011D03F0BE5DCB43320117C8A46FDA3F271FF6A70A1852AB
context.rs：415D86F7BDB9C2F4454FAFD5EDCF58927126BD4A5E9371D5830D3E93C11739B7
lib.rs：9E2BAFF54791700E4F390C78624BF6D1975FA99C806AFD9D53DD06DC37749B74
tests.rs：BD190FB7D2DBDB4D05FF5287271759CFBC2E380865F86F61F6A02B89D77ABB52
当前施工槽：空；254文件仅为历史evidence
Tower P1-2：paused_by_user；不得恢复
重叠修改：309-A源码为本线程刚完成的未提交用户工作，必须原样继承
preflightResult：PASS；当前槽为空、owner范围无冲突，允许从Gate A红测试开始
```

## 8. 自审

```text
方案一致性：通过；严格选择309 Stage B
不过量：通过；只做bounded in-memory operation-bound lease
深Module：通过；调用方不编排scan/hash/materialization细节
AI边界：通过；没有新增AI tool、Host、catalog/execute或Agent
红能力：通过；每项合同均由lease_测试捕获
验证经济性：通过；单crate owner suite，不运行完整workspace/Local CI
外部状态：通过；只使用测试临时fixture，不启动Editor/Gateway
完成声明：通过；不宣称309、Compiler或mutation完成
```

## 9. 施工结果与证据

309-B 已按冻结边界完成。`ProjectSnapshotLease`拥有完整`ProjectSnapshot` materialization，公开
process-local opaque lease identity、operation-bound retention、显式release report，并依赖Rust drop处理
取消/超时路径。`acquire_snapshot_lease`先调用309-A snapshot完成live-source验证和owned bytes物化，成功后
才分配lease id；因此drift失败不会发布lease或消耗identity。

```text
Gate A red：lease_ 3/3 expected failed
首因：authoring_context.snapshot_lease_not_implemented
Gate A green：lease_ 3/3 passed
owner terminal：authoring_project_context完整suite 19/19 passed
format首次检查：仅新增constructor与断言存在rustfmt换行差异
format修复后：cargo fmt --check -p authoring_project_context passed
affected check：cargo check -p authoring_project_context passed
```

验证未启动Editor/Gateway，未运行完整workspace、Local CI或production composition，未修改外部配置、安装态
二进制或真实项目。没有新增磁盘store、registry、timer、线程、第三方依赖、Compiler/Provider consumer或
mutation能力。

## 10. 完成动作

Gate A-B与终态通过后：

- 写`阶段完成记录/2026-08-31-309-B-Operation-Bound-Immutable-Snapshot-Lease-v1/00-总览.md`；
- 本文移到`施工文档/已完成/`；
- 同步README、地图、阅读顺序、49、54、施工/完成记录入口；
- 当前施工槽置空，不自动进入Stage C mutation，不恢复Tower P1-2。
