# 309 Headless AuthoringProjectContext + Unified Mutation v1 方案

> 文档性质：正式架构子设计。  
> 上级 authority：`00-AI-First-Game-Engine-权威产品需求-v1.md`、`00-AI-First-Game-Engine-权威架构设计-v1.md`。  
> 正式选择：用户已确认方案 B，采用嵌入式温状态 Context，不建立默认常驻 Project Authority Daemon。  
> 当前状态：正式方案已完成自审；309-A 至 309-F 均已完成并归档。309-E 已实现 Scene 最小纵切：完整文档保存 Last-Writer-Wins，结构化增量修改继续 CAS；309-F 已完成 Headless Engine Tool Provider 原子切换、Gateway 生产退役、F1-F12 资格和真实 Codex Agent Tool Loop 验收。当前施工槽与待执行队列均为空。  
> 自审报告：`309-Headless-AuthoringProjectContext-Unified-Mutation-v1-自审报告.md`。  
> 设计日期：2026-08-31。

## 0. 一句话决定

建立协议无关、Editor 无关的 `AuthoringProjectContext` 深 Module，作为项目 identity、canonical revision、
source inventory、refresh、snapshot lease、统一 mutation、drift、receipt 和 recovery 的唯一 owner。

默认 Headless 路径把该 Module 嵌入 Engine Provider 或命令行进程；可选 Editor 通过
`EditorProjectAdapter` 使用同一 Interface。v1 不建立必须安装、发现、连接或常驻的项目 daemon。

## 1. 用户确认的不可违反边界

`AuthoringProjectContext` 是 Engine Provider 内部 Module，不是 AI 可见工具，不是新的 Capability Host，
也不是第二个 Agent。

正式产品调用关系必须保持：

```text
User
  -> Host AI Agent / existing Agent Tool Loop
      -> read_file / write_file / apply_patch / run_command
      -> concrete engine_project_inspect / engine_check / engine_run / engine_playtest /
         engine_observe / engine_build / engine_verify_delivery / engine_rollback
          -> Engine Tool Provider
              -> Engine Tool Kernel / domain owner
                  -> AuthoringProjectContext
```

AI 不得看到或被要求调用：

```text
authoring_context.open
authoring_context.refresh
authoring_context.snapshot
authoring_context.commit
engine.catalog
engine.execute(toolId, payload)
engine(prompt)
connect_editor
discover_project_daemon
```

这些内部 Interface 只能被 Engine Tool Kernel、领域 Module、Compiler 和 Optional Editor Adapter 消费。
未来 R5 `First-class Engine Tool Provider` 负责把深能力直接注册为宿主同级 typed tools；本文不提前实现
Host projection，但任何后续 Tool Provider 设计都不得推翻本边界。

## 2. 设计目标

本文必须同时实现以下目标：

1. Editor 未安装、未启动、未连接时，可以打开、刷新、检查和读取确定项目 snapshot。
2. Headless 和 Optional Editor 不再维护两套 project identity、revision 或 digest。
3. AI 使用普通文件工具修改代码和允许直接编辑的 Project Assets 后，下一个 Engine 语义操作能看到新事实。
4. 结构化、高风险和跨文件修改进入一个短 mutation lane，绑定 expected revision，并返回 receipt/rollback。
5. snapshot 使用期间不阻塞 AI 继续写文件或提交其它修改，Compiler 也不长时间占用 mutation lane。
6. watcher、mtime、Editor dirty flag、Gateway generation 和 Library cache 都不能成为 authority。
7. 崩溃、进程退出、并发 Host 和未知外部写入必须有确定、fail-closed 的结果。
8. 当前 Candidate、SourcePatch、AssetImport、安全路径和 rollback 实现应被复用和深化，不重复发明。
9. Context 删除后，revision、refresh、lease、mutation 和 recovery 复杂度会散落到所有调用方；该 Module 必须保持深。

## 3. 非目标

本文不负责：

- 把 Engine tools 注册进 Codex、OpenCode、DeepSeek Harness 或 MCP；
- 定义 Project Game SDK 的最终符号集；
- 实现完整 Game Project Compiler、Rust 增量编译或 RuntimePackage prepare；
- 定义 run/playtest/observe/capture/replay 的最终语义；
- 建立新的自然语言 Agent、Planner 或隐藏 Workflow；
- 建立用户必须管理的后台 daemon；
- 把所有普通文件写入强制转换为 `project.mutate(goal)`；
- 把 Asset DB、Library、RuntimePackage 或 Compiler cache升级为项目源码真相；
- 重新设计 Scene、Prefab、AUI、Input、Rule、Build Profile 的领域 schema；
- 生成施工文档或授权迁移现有代码。

## 4. 现状证据

### 4.1 当前 authority 绑定 EditorSession

`rust/crates/editor_core/src/session.rs` 中 `EditorSession` 同时持有：

- `active_project_session`；
- Scene、selection、undo、workspace、Asset Browser；
- RuntimePackage、Preview、Play、Build、Report；
- AI panel、LLM request、ProjectPatch history；
- runtime preparation 和 Editor composition。

`ProjectCandidateEntry`、`AiCapabilityToolKernel`、Gateway 和多个项目工具仍以 `&EditorSession` 或
`&mut EditorSession` 作为项目入口。现有默认实现因此无法在 Editor 不存在时成为中立 authority。

### 4.2 当前 digest 基础正确但仍是全量工具函数

`CandidateProjectRevisionStore` 已实现：

- canonical relative path；
- 路径排序；
- 内容 SHA-256；
- `Library/Build/target/.git/.aife/.aife-candidates` 排除；
- Cargo nested `target` 排除；
- symlink/junction/reparse point 拒绝；
- 扫描期间文件长度漂移拒绝；
- candidate base/candidate digest 和 changed paths。

这些能力应迁入 Context 的 source inventory owner，不应让 Gateway、Editor open、Compiler 或每个工具继续
各自调用 digest helper。

### 4.3 当前 mutation 基础可复用但没有统一 lane

`ControlledSourcePatch` 已有 expected base digest、candidate validation、per-file atomic write、before snapshot、
apply 后 digest 验证、失败补偿、sealed receipt 和 drift 后 rollback 拒绝。

`ProjectAssetImport` 已有 GUID/meta、Asset DB/Graph/Registry、import lock、source/derived digest、rollback
snapshot 和 tamper detection。

但当前只有 Asset Import 拥有独立 import lock；ProjectPatch、SourcePatch、Scene、AUI、Input 和其它
authoring owner 尚未共用跨进程 mutation lane。`ProjectPatch` 还同时依赖 Editor 内存 draft 和磁盘写入。

### 4.4 Gateway 拥有第二层观察状态

`ai_tool_gateway` 当前维护 `observed_project_digest`、`read_generation` 和 `GatewayProjectContext`，并在项目
变化时自行推进 generation、撤销 grant 和标记 stale。这些是历史 Editor-hosted 拓扑的派生状态。

迁移后 Gateway 或 Host Adapter只能缓存 Context 返回的 revision；缓存失效不等于生成新的项目事实。

### 4.5 当前已有 clean save 正确性

Scene clean save 已证明：canonical bytes 不变时不写盘、不改变 mtime、不制造 dirty。该合同必须迁入
`EditorProjectAdapter -> commit`，不能因 authority 切换退化。

## 5. 成熟实现参考与取舍

### 5.1 Unity

参考 `AssetDatabase.Refresh`、`StartAssetEditing/StopAssetEditing`、GUID/meta 和 Importer：

- 学习 source/meta/artifact 分层、批量 refresh 和稳定 ID；
- 不照搬 Editor-only AssetDatabase、Domain Reload 和要求 AI 启动 batchmode Editor 的拓扑。

### 5.2 Godot

参考 `EditorFileSystem::scan/scan_changes/update_file/reimport_files`、`ResourceUID` 和 ResourceImporter：

- 学习 scan、变化提示、reimport、UID/path 和 imported cache 分层；
- 不照搬 Editor Node 常驻 owner，也不让 `.godot` cache 成为隐式项目真相。

### 5.3 Unreal

参考 `IAssetRegistry`、`ScanPathsSynchronous`、`FScopedTransaction`、Package identity 和 Commandlet：

- 学习 registry/transaction/headless command 分层；
- 不照搬 UObject/UPackage、二进制资产和 Editor package dirty authority。

### 5.4 Bevy

参考 `AssetServer`、`AssetSource`、`AssetPath`、`AssetEvent` 和 watcher：

- 学习 source abstraction、load state、事件和运行时友好 owner；
- 不把 Runtime asset loading误当完整 authoring transaction/revision。

### 5.5 Git 与 Bazel

Git index证明 working tree变化不等于一个已提交 revision；显式内容集合和 checksum 才能形成确定 snapshot。
Bazel Skyframe证明增量正确性依赖注册输入、确定值和 dependency invalidation；watcher只优化变化发现，不能
替代内容验证。

本文采用这些机制思想，但不要求项目是 Git repository，也不把 Compiler dependency graph 当项目 authority。

## 6. 正式拓扑：方案 B

```text
Host AI Agent
  -> future concrete engine_* tool
      -> HeadlessProjectProvider Adapter
          -> AuthoringProjectContext Interface
              -> EmbeddedAuthoringProjectContext Implementation
                  -> canonical project storage
                  -> warm source inventory / derived index cache
                  -> project-scoped lock + journal + receipts

Optional Editor
  -> EditorProjectAdapter
      -> same AuthoringProjectContext Interface
          -> same canonical project storage and coordination contract
```

正式决定：

- v1 不存在默认 `ProjectAuthorityDaemon`；
- Headless Provider或 CLI进程内持有温状态 Context；
- Editor进程内通过 Adapter持有 Context；
- 每个进程的内存 cache都只是加速层；
- canonical storage、统一 revision算法、跨进程 lock/CAS 和 journal共同保证单一 authority；
- 将来只有大型项目 benchmark证明嵌入式实现不足时，才可增加 IPC Adapter；不得改变 Interface或AI-visible tools。

## 7. Module 与 Adapter

### 7.1 AuthoringProjectContext Module

该 Module拥有：

- canonical project root和manifest绑定；
- project identity和root binding；
- source inventory policy；
- current observed revision；
- structural qualification；
- refresh和派生 Asset DB publication；
- immutable snapshot lease；
- project-scoped authority/mutation lane；
- expected revision CAS；
- commit journal、receipt lineage和rollback；
- crash inspection和recovery状态；
- warm fingerprint/cache失效。

它不拥有：

- 用户目标、AI计划、对话或工具选择；
- Host tool schema、MCP协议或Codex类型；
- Editor window、dock、selection、undo UI或未保存draft；
- Game Project Compiler的依赖图、Rust编译和RuntimePackage装配；
- Runtime Job、playtest、capture或交付状态；
- 具体 gameplay语义。

### 7.2 HeadlessProjectProvider Adapter

默认 AI路径。它把 workspace/project locator、Engine operation和权限内调用映射到 Context Interface，
但不新增自然语言解释、二级 catalog 或跨工具计划。

它可以被 Engine Tool Provider、CLI或测试宿主嵌入。它不是必须发现的独立进程。

### 7.3 EditorProjectAdapter

可选 Editor路径。它负责：

- 把 canonical snapshot投影为Editor文档；
- 将未保存内容标记为local draft；
- 把结构化编辑转换为 ProjectMutation；把完整单文件 Save/Save As 转换为 Context 的原子文档替换；
- 将 Context diagnostics/receipt投影到Editor UI；
- 完整文档保存采用 last-writer-wins，不要求 reload/merge/rebase；结构化增量修改仍按 expected-revision CAS。

它不得拥有独立 revision、Asset DB、Gateway generation或Editor-only project truth。

## 8. 冻结 Interface

概念 Interface冻结为：

```text
open(ProjectLocator, OpenOptions) -> ProjectHandle
refresh(ProjectHandle, RefreshRequest) -> RefreshReport
snapshot(ProjectHandle, SnapshotRequest) -> ProjectSnapshot
save_document(ProjectHandle, DocumentWriteRequest) -> DocumentWriteReport
commit(ProjectHandle, CommitRequest) -> MutationReceipt
rollback(ProjectHandle, RollbackRequest) -> RollbackReceipt
close(ProjectHandle) -> CloseReport
```

具体 Rust类型名可在施工文档中按 crate命名规范细化，但不得增加要求调用方编排内部扫描、Asset DB、lock、
journal、recovery或cache的浅方法。

### 8.1 open

`open`：

1. canonicalize并能力化打开项目根；
2. 验证manifest、project identity、schema版本和root containment；
3. 检查未完成journal和recovery状态；
4. 加载或重建可删除的warm inventory；
5. 执行首次refresh；
6. 返回process-local opaque `ProjectHandle`。

`ProjectHandle` 不是项目identity，不能跨进程序列化、写入AI输入或作为receipt authority。

### 8.2 refresh

`refresh`读取当前canonical inputs，验证内容并返回新的或不变的`ProjectRevision`。它允许更新派生索引，
但不得静默修改canonical source。

### 8.3 snapshot

`snapshot`返回绑定某一revision的不可变逻辑视图和lease。调用方不能把live project root当作长期snapshot。

### 8.4 commit

`commit`只接收领域owner已准备并验证的`ProjectMutation`，在统一lane内执行expected revision CAS、journal、
原子单文件替换、多文件补偿、post-check、revision publication和receipt sealing。

完整单文件文档保存使用同一 Context 的领域无关 `save_document` 语义：它执行路径安全、equal-byte no-write、
短写入协调、单文件原子替换和 refresh，但不执行 expected-revision CAS，也不产生结构化 mutation 的 rollback
义务。最后完成有效替换的写入者覆盖此前内容。多文件文档保存不得使用该语义，必须进入 `commit`。

### 8.5 rollback

`rollback`是绑定exact mutation receipt和expected applied revision的新mutation。存在任何intervening drift时拒绝，
不能自动重放旧inverse mutation。

### 8.6 close

`close`释放process-local lease、watcher和warm cache引用。它不改变canonical项目，不隐式保存Editor draft，
也不删除仍被receipt引用的recovery材料。

## 9. Project Identity 与 Root Binding

项目身份分为两类：

```text
PortableProjectIdentity = manifest.projectId
OpenedProjectBinding = PortableProjectIdentity + canonicalRootFingerprint
```

- `projectId`允许项目副本保留逻辑身份；
- `canonicalRootFingerprint`防止一个process把同ID的两个目录误当同一打开实例；
- root fingerprint不得由AI提供或伪造；
- revision不包含绝对路径，因此同字节项目副本可以得到相同portable revision；
- operation、receipt和rollback同时绑定project identity与opened root，防止跨副本误用。

## 10. Canonical Source Inventory

canonical inventory至少包含：

```text
canonical relative path
content length
content digest
source kind/domain
stable ordering key
```

v1继续排除：

```text
Library/
Build/
target/
.git/
.aife/
.aife-candidates/
任意Cargo root下的target/
```

规则：

- path必须project-relative、UTF-8、portable且无case-fold collision；
- symlink、junction、reparse point和unsupported filesystem entry fail-closed；
- mtime、size和watcher event只能用于决定“是否需要rehash”；
- 最终revision必须由内容identity确认；
- 扫描期间文件变化必须retry或返回`source_changed_during_refresh`；
- inventory policy版本进入revision identity，避免规则变化后错误复用旧cache。

## 11. Canonical Revision

`ProjectRevision`至少表达：

```text
schemaVersion
portableProjectIdentity
sourcePolicyVersion
sourceDigest
revisionId
qualification
diagnosticsDigest
```

其中：

```text
sourceDigest = hash(sorted(canonicalPath, sourceKind, length, contentDigest))
revisionId = hash(revisionSchema, projectIdentity, sourcePolicyVersion, sourceDigest)
```

正式规则：

- `revisionId`是correctness identity；
- generation只允许是某个ProjectHandle生命周期内的观察序号和性能提示；
- generation不得用于跨进程CAS、receipt或rollback正确性；
- 同样内容和policy得到同样revision；
- clean refresh不写canonical文件、不生成新revision、不推进观察generation；
- Library、Asset DB、Compiler cache删除或重建不改变canonical revision；
- Engine版本、Importer recipe和Compiler recipe进入各自derived binding/cache key，不伪装成source revision。

### 11.1 无效项目也是当前事实

普通文件工具可能写出无效JSON、缺失meta或不兼容schema。Context不得继续把旧valid revision当当前真相。

refresh仍发布新内容identity，但qualification可以是：

```text
ready
invalid
recovery_required
```

`inspect`和诊断可以读取invalid snapshot；Compiler prepare、run、build和mutation commit必须按合同拒绝不合格
revision，直到AI修复并再次refresh。

## 12. 普通文件工具进入 revision

AI继续可以执行：

```text
write_file("RuntimeModule/src/player.rs", ...)
apply_patch("AUI/hud.aui.json", ...)
```

文件工具本身不伪造Engine receipt。下一个需要项目语义的Engine operation必须在内部执行：

```text
refresh
-> consume watcher hints / compare warm inventory
-> read and hash actual bytes
-> validate path, manifest, schema and registered asset metadata
-> publish current revision and derived invalidation
-> return diagnostics
-> snapshot exact revision
```

Host Adapter不得要求AI先显式调用`authoring_context.refresh`。具体`engine_check`、`engine_run`等深工具内部决定
freshness并自动进入该步骤。

### 12.1 资源例外

直接写入已注册资源的source/meta/descriptor会进入refresh和validation。

直接放入一个没有stable identity/meta的raw asset时：

- refresh记录新的source事实；
- qualification返回`unregistered_asset`或匹配diagnostic；
- refresh不得静默创建meta或GUID；
- AI使用具体`engine_asset_import`或等价typed change tool进入统一mutation并获得receipt。

该规则避免refresh悄悄修改canonical项目，同时保留普通文件工具与Engine asset能力的组合体验。

## 13. Watcher 与 Warm State

方案 B允许每个process维护：

- path fingerprint；
- previous source inventory；
- dirty path hints；
- derived Asset DB binding；
- open handle和snapshot引用；
-最近diagnostics的可删除cache。

watcher只做：

```text
filesystem event -> mark path/project maybe_dirty
```

watcher不得：

- 直接推进revision；
- 直接生成receipt；
- 以event顺序作为generation；
- 因丢事件而让refresh跳过内容确认；
- 让一个process的cache覆盖另一个process看到的canonical bytes。

没有watcher、watcher溢出或进程刚启动时，refresh退化为安全扫描，结果语义不变。

## 14. Snapshot Lease

`ProjectSnapshot`至少绑定：

```text
snapshotId
projectIdentity/root binding
revisionId
scope
qualification
inventory/materialization manifest
lease owner and expiry policy
```

正式语义：

- snapshot在返回后是不可变逻辑视图；
- 项目随后产生新revision不会改变已返回snapshot；
- snapshot不持有mutation lane；
- Compiler或长operation只能读取snapshot materialization或内容寻址blob，不得继续读取live root；
- 小scope inspect可在返回前读取并封印所需bytes；
- v1可以复用candidate tree copy建立正确实现，再按profile引入content-addressed blob reuse；
- 不允许用source hardlink宣称不可变，因为外部in-place write可能修改hardlink内容；
- lease过期只影响资源保留，不改变已生成artifact的revision lineage；
- operation完成、取消或超时必须释放lease引用。

### 14.1 Compiler 与 commit 并发

正式顺序：

```text
refresh R1
-> snapshot lease S1(R1)
-> release authority publication gate
-> Compiler prepare S1

同时：commit R1 -> R2 可以继续
```

Compiler结果仍正确绑定R1；若调用方请求“当前项目结果”，返回时recheck current revision并把结果标记为
`current`或`superseded`。不得把R1 artifact冒充R2，也不得因长编译阻塞R2提交。

## 15. Unified Mutation

### 15.1 哪些修改必须进入统一 seam

- Scene/Prefab/AUI/Input/Rule等结构化跨文件修改；
- stable identity对象删除、移动、重命名；
- Asset import、替换、引用更新和影响分析；
- schema migration；
- RuntimeModule依赖和toolchain contract变更；
- generated canonical glue发布；
- 平台配置、签名和发布等高风险写入；
- 任何需要原子跨文件语义、receipt或rollback的修改。

### 15.2 ProjectMutation 不是自然语言黑盒

`ProjectMutation`由领域owner产生，至少绑定：

```text
mutationId
mutationKind/domain
expectedRevisionId
validationDigest
declared read/write set
expected before hashes
canonical file operations
derived invalidations
rollback/recovery policy
```

它不得包含需要Context再次理解的自由文本目标。Context不判断“打飞机应该怎么玩”，只协调已验证修改的
确定提交。

### 15.3 领域 owner 与事务 owner分离

```text
Scene/Prefab/AUI/Asset/... domain Module
  -> validate semantics / impact / references
  -> produce Prepared ProjectMutation

AuthoringProjectContext
  -> validate revision/path/write-set contract
  -> serialize commit
  -> apply/recover/receipt
```

Context不复制各领域schema；领域Module也不得绕过Context直接提交canonical跨文件修改。

## 16. Commit 算法

正式commit流程：

```text
1. validate handle, root binding, mutation schema and qualification
2. acquire project-scoped exclusive authority/mutation lane
3. refresh/recheck actual source and compare expectedRevisionId
4. revalidate declared before hashes and write-set containment
5. create sealed write-ahead journal and before snapshots
6. mark journal applying
7. apply per-file atomic create/replace/delete/move plan
8. rescan touched paths and project source
9. reject unknown/intervening writes; compensate own writes when safe
10. publish after revision and derived invalidations
11. persist sealed MutationReceipt and mark journal committed
12. release lane
```

mutation lane只覆盖真实authority publication和commit窗口，不覆盖：

- AI思考；
- candidate生成；
-领域validation；
- Rust编译；
- Asset cook；
- Preview/Runtime；
- 用户等待；
- observation或build。

## 17. 跨进程并发和未知写入

### 17.1 Engine参与者

所有Headless Provider和Editor Adapter必须使用同一project-scoped OS生命周期锁；结构化 mutation 额外使用
同一 CAS 合同。完整单文件 Save 不执行 CAS，但仍必须通过 Context 的安全原子写入语义。内存Mutex只能优化
单process，不能代替跨进程lane。 

锁状态应包含可诊断owner信息，但owner record不是authority。process崩溃后OS锁必须自动释放；残留journal
由recovery处理，不能靠永久lock file猜测活性。

### 17.2 普通文件工具和外部编辑器

普通`write_file`、IDE、Git checkout和其它程序不会遵守Engine lock。完整单文件 Save 与这些工具一样遵循最后
有效写入者覆盖；下一次 Engine 语义操作必须 refresh。结构化 commit 仍必须：

- lane内执行最终expected revision recheck；
- apply后验证declared write-set和whole-project source identity；
- 发现无关外部写入时停止结构化发布，并在不覆盖外部字节的前提下补偿自己的write-set；
- 发现同一路径未知字节时进入`recovery_required`，不得覆盖或猜测合并；
- 返回expected/actual revision、changed paths和明确next-action category。

未知写入不得被悄悄吸收到原mutation receipt。

### 17.3 多AI session

两个AI session可以并行读取同一revision并各自准备candidate。第一个commit成功后，第二个expected revision
失效并返回drift；第二个Agent必须refresh、重新读取和重新生成修改，不能自动重放旧candidate。

## 18. Journal、Receipt 与 Crash Recovery

建议内部控制状态：

```text
.aife/authoring/
  transactions/<mutationId>/journal.json
  transactions/<mutationId>/before/
  receipts/<receiptId>.json
  snapshots/<revisionId>/...
  authority-state.json
```

这些文件不进入canonical source revision；它们是运行中协调、审计和恢复材料。删除全部可重建cache不能改变
项目源码，但删除未完成transaction或仍承诺rollback的材料会使对应操作进入明确`recovery_unavailable`，
不得伪装成功。

journal状态至少包括：

```text
prepared
applying
committed
rolled_back
recovery_required
```

open/refresh必须先检查非terminal journal：

- 当前字节完全匹配before：标记aborted并清理可清理材料；
- 当前字节完全匹配sealed after：补全receipt并标记committed；
- touched paths仅包含可证明的部分apply且无外部重叠：恢复before并验证；
- 出现未知字节、同路径外部覆盖或材料tamper：进入`recovery_required`并禁止新commit；
- recovery不得删除未知外部修改。

Receipt至少表达：

```text
receiptId / mutationId
project identity/root binding
beforeRevisionId / afterRevisionId
validationDigest
declared and actual changed paths/domains
journal/rollback binding digest
diagnostics
rollback availability
```

## 19. Rollback

rollback使用同一个mutation lane，并要求：

- receipt schema和binding digest有效；
- 当前root/project identity匹配；
- current revision精确等于receipt.afterRevisionId；
- rollback material未tamper；
- 没有intervening mutation或外部写入。

通过后恢复before bytes、重新refresh、验证beforeRevisionId并返回RollbackReceipt。任一条件不满足则fail-closed，
要求显式merge/recovery，不允许“尽量撤销”。

## 20. Asset DB 与 Library

authoring source真相：

```text
asset source
stable meta/GUID
descriptor/import settings
Project Assets references
```

派生状态：

```text
Library/AssetPipeline/asset-database.json
Library/AssetPipeline/asset-graph.json
Library/AssetPipeline/asset-registry.json
cooked payloads / thumbnails / Compiler cache
```

规则：

- derived Asset DB必须绑定`sourceRevisionId + importerRecipeVersion`；
- source/meta相同应得到确定的registry；
- refresh可以增量重建和原子发布derived index；
- derived index缺失、过期或损坏时重建，不改变source revision；
- raw asset注册、GUID创建和canonical meta写入必须走typed asset mutation；
- RuntimePackage只消费Compiler准备的绑定产物，不读取live Asset DB猜测当前状态。

## 21. Refresh Publication 与单一 lane

refresh的大部分扫描可以并行、无锁完成，但最终发布current revision和derived binding时必须短暂进入同一个
project authority publication gate：

```text
scan outside gate
-> acquire gate
-> recheck dirty inputs / current source identity
-> publish revision + derived binding atomically
-> release gate
```

若recheck发现扫描结果已过期，refresh必须retry或返回drift，不能发布旧inventory。该gate和mutation lane
属于同一内部协调owner，避免refresh与commit各自维护锁。

## 22. Editor Draft 与 Save

Editor打开项目时记录base revision。Editor内存修改只形成：

```text
LocalDraft(baseRevisionId, dirtyDomains, draftBytes/model)
```

它不能影响Headless snapshot、Compiler、Runtime或Build。

Save流程：

```text
serialize canonical draft bytes
-> equal/current clean: clean_save_no_write
-> different: Context.save_document(single-file atomic replace, last-writer-wins)
-> refresh current canonical revision
```

完整文档 Save 不比较 draft base revision；它可以覆盖保存开始前已经存在的外部版本。clean Save 没有本地 draft
变化时不写入，也不覆盖外部新内容。需要基于旧状态的删除、移动、重命名、引用替换或多文件保存，必须另行生成
`ProjectMutation` 并按 CAS 规则执行。

Save As创建或覆盖 canonical target 时同样通过 Context path/identity 规则，不绕过安全写入。

## 23. 与 Engine Tool Provider 的关系

本文是R1 authority上游，不是R5 Tool Provider本身。

未来具体工具内部典型路径：

```text
engine_check(projectPath)
  -> HeadlessProjectProvider.open/reuse
  -> Context.refresh
  -> Context.snapshot
  -> Game Project Compiler.check
  -> canonical ToolResult

engine_change_apply(change)
  -> domain validate/prepare
  -> Context.commit(expected revision, mutation)
  -> MutationReceipt -> canonical ToolResult

engine_run(projectPath)
  -> Context.refresh/snapshot
  -> Compiler.prepare(snapshot)
  -> Runtime.run(prepared game)
```

AI只看到`engine_check/engine_change_apply/engine_run`等具体工具。内部多步调用不形成AI必须学习的workflow。

## 24. 与 Game Project Compiler 的关系

Context拥有“某个revision是什么”；Compiler拥有“如何把该revision检查并准备为产物”。

Context不得拥有：

- Rust依赖图；
- generated glue recipe；
- RuntimePackage assembly流程；
- target/toolchain readiness；
- prepare cache策略。

Compiler不得：

- 重新扫描live root产生自己的project revision；
- 使用Editor dirty state；
- 把cache key当项目authority；
- 在prepare期间长期持有mutation lane。

## 25. Diagnostics 与 Report

Context diagnostics至少包含：

```text
code
severity
stage: open/refresh/snapshot/commit/rollback/recovery
project identity/revision
source path/domain
expected/actual digest or revision
retryability
next-action category
```

报告分档：

- Off：只保留功能必需结果；
- Summary：revision、changed domains、qualification、首要diagnostics；
- Trace：inventory变化、lease、lock等待、journal和recovery细节。

Trace不得默认进入Runtime热路径，也不得把绝对路径、用户身份或Host secret写入AI可见结果。

## 26. 安全约束

- 所有读写通过project-root capability和canonical relative path；
- 拒绝absolute path、`..`、alternate data stream、reserved device name、case-fold collision；
- 拒绝symlink/junction/reparse逃逸；
- AI direct input不得包含可伪造revision、Grant、root fingerprint或lock owner；
- expected revision由Provider从当前session事实绑定，而不是信任模型自由输入；
- Host approval由后续Adapter组合，Context只执行已授权的Engine mutation contract；
- commit和rollback均有bounded bytes/path/operation limits；
- journal、snapshot和receipt材料必须有digest binding和容量/retention策略；
- cancellation只能在安全checkpoint生效；apply开始后必须完成commit或compensation/recovery状态。

## 27. 性能策略

v1正确性优先级：

```text
content identity
-> warm path fingerprint
-> incremental source inventory
-> derived Asset DB invalidation
-> snapshot blob reuse
```

性能规则：

- warm Context按process生命周期复用inventory；
- watcher减少无效stat/hash，但不改变结果；
- unchanged文件允许复用已验证content digest；
- periodic或风险触发的content audit防止mtime/size cache欺骗；
- snapshot只materialize请求scope和依赖closure；
- snapshot/receipt使用引用计数或pin，operation完成后清理；
- lock等待有deadline和结构化owner diagnostic；
- 不因Editor UI、Host Adapter或Gateway状态使source inventory全失效。

## 28. 当前代码复用与迁移

### 28.1 直接复用/深化

| 当前实现 | 新位置/职责 |
| --- | --- |
| `ProjectWriteScope` / `ProjectRelativePath` | Context安全project I/O基础 |
| `CandidateProjectRevisionStore` | source policy、inventory、revision和snapshot staging基础 |
| `ProjectOpenPreparation` | Headless open/首次refresh基础 |
| `ControlledSourcePatch` | source mutation handler、receipt/rollback基础 |
| `ProjectAssetImport` | asset domain mutation和derived Asset DB基础 |
| `ProjectCandidateEntry` | provider-independent envelope经验；移除EditorSession authority |
| Scene clean-save contract | EditorProjectAdapter clean save资格 |
| Tool Kernel journal/receipt | operation lineage经验，不再拥有project revision |
| Asset Browser fingerprint | warm derived view经验，不再拥有authority generation |

### 28.2 必须迁出/降级

- `EditorSession.active_project_session`降为Editor Adapter持有的handle/projection；
- `ProjectPatch` validation/apply逐步从EditorSession内存owner迁到领域Module + Context commit；
- Gateway `observed_project_digest/read_generation`降为可删除cache或移除；
- `ProjectLauncher`保留Editor UX，不再是Headless open的唯一入口；
- Editor Preview/Build不得自行计算第二套project digest。

### 28.3 建议 crate seam

后续施工优先建立协议无关crate，例如概念名：

```text
authoring_project_context
```

该crate不得依赖：

```text
editor_ui_model
editor_window_winit
ai_tool_gateway
Codex/OpenCode/DeepSeek/MCP types
```

具体crate名在施工文档冻结，但依赖方向不得反转。

## 29. 施工阶段建议

本文不授权施工。未来施工文档应按可独立打红和回归的阶段拆分，建议顺序：

### Stage A：Neutral Read Authority

- 建立Context Interface和Headless open；
- 迁入source policy、revision和qualification；
- Editor不存在时完成open/refresh/snapshot。

### Stage B：Immutable Snapshot Lease

- 建立scope/materialization/lease；
- 证明snapshot期间外部写入不改变其bytes；
- 建立release/cancel/retention。

### Stage C：Unified Mutation Lane

- 建立跨进程OS lock、expected revision CAS、journal和receipt；
- 先迁入ControlledSourcePatch和AssetImport；
- 证明并发commit和未知写入fail-closed。

### Stage D：Crash Recovery

- incomplete journal识别；
- before/after finalize/compensate；
- unknown bytes进入recovery_required。

### Stage E：EditorProjectAdapter

- ProjectSession/Scene draft接入同一Context；
- clean save no-write；
- 完整单文件 Save/Save As 的 last-writer-wins；
- 结构化增量编辑仍使用 CAS，不把 Editor Save 改成增量覆盖。

### Stage F：Headless Engine Tool Provider Atomic Cutover

正式子方案见 `309-F-Headless-Engine-Tool-Provider-Atomic-Cutover-v1方案.md`。本 Stage 一次完成：

- 移除 Gateway 第二套 generation/observed digest authority；
- Tool Kernel 与现有工具改用 Context revision/snapshot/receipt；
- 将 Engine tools 可达的项目执行状态从完整 `EditorSession` 抽离为 Headless-neutral Module；
- 建立 Headless Engine Tool Provider 和 Native/MCP Host Adapter seam；
- Codex 通过 MCP 真实路径直接获得具体 `engine_*` tools；
- Run/Build/Delivery 当前 ready 能力移除 Editor Play/GameView 必选依赖；
- Editor 不再默认托管 Gateway，旧 discovery/named-pipe/Editor-instance 生产拓扑退役；
- 保留 Grant/operation/cancel/receipt/rollback 语义，不保留旧 Gateway 作为 fallback。

本 Stage 已通过一份施工文档和一个连续施工窗口完成，内部 Gate A-I 与 F1-F12 全部通过；施工文档已归档至
`施工文档/已完成/309-F-Headless-Engine-Tool-Provider-Atomic-Cutover-v1施工文档.md`，完成记录见
`阶段完成记录/2026-09-02-309-F-Headless-Engine-Tool-Provider-Atomic-Cutover-v1/00-总览.md`。

每一Stage必须由后续施工文档确定实际文件、测试和回滚范围；当前Tower P1-2施工不属于本方案施工窗口。

## 30. 资格 Gate

### G1：No-Editor Open/Refresh/Snapshot

不构建、不启动Editor和Gateway，独立进程能打开真实项目、refresh并获得确定snapshot和diagnostics。

### G2：Deterministic Revision

相同project bytes/policy得到相同revision；Library/Build/target变化不改变revision；canonical source变化必改变
revision；clean refresh无写入。

### G3：Direct File Tool Compatibility

外部普通文件写入后，下一个语义operation自动refresh；valid变化成为新ready revision，invalid变化成为新
invalid revision并给出source diagnostic，不继续使用旧事实。

### G4：Immutable Snapshot

获取S1后修改live root，S1读取结果保持不变；新refresh得到S2；Compiler消费者不能混用S1/S2。

### G5：Unified CAS and Mutation Lane

两个process从同一base准备mutation，仅一个能commit；另一个得到drift。不同mutation类型共用同一lane，
不存在Asset-only lock孤岛。

### G6：Crash Recovery

在prepared/applying/after-write等checkpoint强制终止process，重新open得到确定committed、restored或
recovery_required，不出现静默半提交。

### G7：Receipt/Rollback

exact after revision可rollback；intervening write、root mismatch、receipt/journal tamper全部拒绝且不覆盖未知字节。

### G8：Two Real Adapters

HeadlessProjectProvider与EditorProjectAdapter通过同一Interface读取相同revision；Editor draft不影响Headless；
clean save不推进revision；dirty完整文档 Save 按最后有效写入者覆盖；结构化 mutation 仍按 CAS 返回 drift。

### G9：No AI-visible Secondary Host

架构和后续Host conformance必须证明：

- Context Interface未注册为AI tools；
- 不需要daemon discovery或Editor connection；
- 不出现AI可见`catalog/execute`；
- 具体`engine_*`工具内部消费Context；
- Provider不拥有Agent计划和模型循环。

G9在本文冻结边界，在R5通过真实Host Tool Registry最终验收；R1不得冒充已经完成Tool Provider资格。

## 31. 风险与控制

| 风险 | 控制 |
| --- | --- |
| Context变成巨大Host | 保持小Interface；Host协议、Compiler、Runtime、AI计划均在外部 |
| 多process warm cache成为多真相 | digest/CAS正确性只依赖canonical bytes和共享协调合同 |
| full scan过慢 | watcher hint、fingerprint、增量inventory；仍保留安全full audit |
| snapshot复制昂贵 | scope materialization和blob reuse；不牺牲不可变语义 |
| multi-file非真正OS原子 | write-ahead journal、before snapshot、post-check和deterministic recovery |
| 外部工具不遵守lock | 完整单文件 Save 以最后有效替换为准；结构化 commit 继续内容recheck、未知写入fail-closed、绝不覆盖未知字节 |
| Library删除破坏authority | Library仅派生；canonical revision可重建；active transaction材料单独诊断 |
| Editor长期保持第二真相 | draft明确local；完整 Save 和结构化 commit 都走 Context；Gateway generation退役 |
| 提前按Tool schema塑造Core | R1只冻结项目Interface；R5只投影已稳定深Module |

## 32. 为什么正式选择方案 B

相对完全无状态方案 A：

- 保留No-Editor和无daemon；
- 避免每个Engine tool重复全量扫描、Asset DB加载和snapshot准备；
- 能为后续Compiler提供稳定增量基础。

相对常驻daemon方案 C：

- 不增加安装、启动、发现、IPC、升级和单点故障；
- 不重新制造用户担心的Engine Capability Host；
- 仍可通过OS lock、CAS和journal处理多process；
- 将来确有大型项目证据时，可以在同一Interface后增加新Adapter。

方案 B的核心不是“少一个进程”，而是：

> 项目正确性属于一个协议无关深Module和canonical storage合同；进程内warm state只提供性能，不拥有第二真相。

## 33. 完成定义

本文只有在以下条件都满足后，才可声明R1正式施工完成：

1. G1-G8由真实代码和测试通过；其中 G8 同时证明完整文档 LWW 与结构化 mutation CAS 两种语义；
2. G9内部边界有静态/集成证据，并明确等待R5真实Host最终资格；
3. Editor不存在时真实项目可open/refresh/snapshot并执行结构检查；
4. 所有结构化、多文件和高风险 canonical mutation进入统一lane；完整单文件文档写入使用 Context 原子替换或有逐项书面迁移状态；
5. Gateway/Editor不再拥有项目revision authority；
6. current source、derived Asset DB、snapshot和Compiler artifact lineage可以结构化解释；
7. 结构化 mutation 的失败、取消、并发、crash和rollback不覆盖未知外部修改；完整单文件 Save 按 LWW 语义允许覆盖此前内容且不留下半文件；
8. 文档、实现和测试使用同一Interface，未为测试暴露内部seam。

在这些条件完成前，只能声明“309正式方案已确认/已自审/施工中”等对应真实状态，不能声明自研引擎已经
作为一等工具直接提供给AI，也不能跳过R2-R4直接用新的Provider黑盒掩盖底层缺口。
