# 309-A Headless Neutral Read Authority v1 施工文档

> 上级方案：`../../309-Headless-AuthoringProjectContext-Unified-Mutation-v1方案.md`。  
> 方案自审：`../../309-Headless-AuthoringProjectContext-Unified-Mutation-v1-自审报告.md`。  
> 当前状态：2026-08-31 Gate A-C 与终态回归通过；已完成，等待归档。  
> 当前授权：用户已明确暂停 Tower P1-2，并授权按本文 Gate A-C 施工。  
> 完成结果：Tower P1-2 继续位于项目待执行队列；309-A 只读最小纵切达到 `sourceAvailable`。  
> 时间边界：激活后单次连续施工不得超过三小时。

## 1. 施工目标

建立 309 的第一个最小可验证纵切：在不构建、不启动、不连接 Editor 或 Gateway 的独立 Rust crate 中，
完成 canonical project source inventory、确定 revision、Headless `open/refresh` 和 bounded immutable read
snapshot，并让现有 candidate digest 复用同一个 source policy owner。

完成后的唯一产品声明是：

> `AuthoringProjectContext` 的 Headless 只读 authority sourceAvailable；独立消费者可以读取真实项目的稳定
> identity/revision和限定路径不可变 snapshot。309 的 mutation、recovery、Editor cutover 和一等 Engine
> tools 均未完成。

## 2. 为什么只施工这一段

309 完整方案横跨六类高风险能力。第一步真正的因果前置只有“项目是什么、当前 revision 是什么、无 Editor
能否稳定读取”。如果这一步不成立，mutation、Compiler 和 Tool Provider 都会重新绑定 Editor 或各自产生
digest；如果一次把后续能力都做完，则会同时修改 `editor_core`、Gateway、Compiler、Editor 和跨进程状态，
无法形成便宜、可定位的红测试。

因此本文在第一个可用 Headless read seam 处停止。

## 3. 必要性与验证经济性审计

```text
requested claim:
  309 进入受控施工准备，但不得过量施工

minimum causal change:
  新建协议无关 authoring_project_context crate；迁入唯一 source inventory/revision owner；
  提供 embedded headless open/refresh 和 bounded immutable read snapshot；
  让现有 CandidateProjectRevisionStore digest 委托同一 owner

files that must change:
  rust/Cargo.toml
  rust/Cargo.lock
  rust/crates/authoring_project_context/Cargo.toml
  rust/crates/authoring_project_context/src/*.rs
  rust/crates/editor_core/Cargo.toml
  rust/crates/editor_core/src/candidate_project_revision.rs

explicitly excluded or deferred:
  commit/rollback、跨进程mutation lock、journal、crash recovery、Asset DB publication、
  Compiler/RuntimePackage、EditorProjectAdapter、EditorSession/Gateway迁移、Engine Tool Provider、
  Host注册、production二进制、真实配置、Local CI、发布资格

Gate A -> neutral owner/无Editor依赖 -> crate依赖检查 + owner红测试 -> neutral seam evidence
Gate B -> revision错误或旧digest漂移 -> canonical inventory/revision矩阵 + editor_core兼容测试
       -> deterministic revision evidence
Gate C -> snapshot偷读live root -> 写后仍读旧bytes + refresh得到新revision测试
       -> bounded immutable snapshot evidence

reused evidence:
  CandidateProjectRevisionStore现有digest/path/link测试；ProjectOpenPreparation现有nested target测试

consumed identities:
  新crate源码、editor_core兼容委托、相关测试源码、Cargo manifests/lock

invalidation condition:
  上述源码/测试/manifest变化；source policy或revision schema变化；相关用户修改与计划重叠

estimated Gate cost:
  A 10-20分钟，B 25-45分钟，C 20-35分钟，终态回归10-20分钟；总计保守小于2小时

stop/review condition:
  需要修改除列出文件外的生产owner；需要迁移ProjectManifest全量schema；需要新增daemon/IPC；
  需要真实Editor/Gateway/Compiler；测试冷编译或设计漂移使总估算超过2小时15分钟；
  当前工作树重叠修改无法无损保留
```

## 4. 冻结施工边界

### 4.1 必须实现

- 新建 `authoring_project_context` 协议无关 crate；
- crate 不依赖 `editor_core`、`editor_*`、`ai_tool_gateway`、MCP/Codex/OpenCode/DeepSeek 类型；
- 建立 canonical project root、portable project identity、source policy version 和 content-based revision；
- inventory 排除 `Library/Build/target/.git/.aife/.aife-candidates` 及 Cargo tree 的 nested `target`；
- mtime/watcher 不能直接决定 revision；
- symlink/junction/reparse、case-fold collision、扫描中漂移 fail-closed；
- `open` 首次 refresh，`refresh` 返回 unchanged/new/invalid 的结构化结果；
- invalid manifest/source 是当前事实，不回退到旧 valid revision；
- bounded snapshot 只接收显式路径 scope，拥有返回时封印的 bytes，不在返回后读取 live root；
- `CandidateProjectRevisionStore::project_digest*` 委托同一 inventory/source policy owner，并保持既有 digest
  protocol 字节兼容。

### 4.2 明确禁止

- 不实现 309 `commit/rollback`；
- 不创建 OS mutation lock、transaction journal、receipt store 或 recovery daemon；
- 不迁移 `ControlledSourcePatch`、`ProjectAssetImport`、Scene/AUI/Input/Rule 写路径；
- 不改 `EditorSession.active_project_session`；
- 不改 Gateway observed digest/generation、MCP 工具、Host 配置或 tool list；
- 不接入 Compiler、Preview、Runtime、Build、Export 或 Android；
- 不建立全项目 content-addressed snapshot store；
- 不为 `commit/rollback` 或未施工的完整 Interface 添加返回 `unsupported` 的占位方法；
- 不为未来能力预建 schema migration、兼容矩阵、runner 或 generic catalog；
- 不运行完整 workspace、all-features、Local CI、真实窗口、GPU、APK 或 production replacement。

### 4.3 防止二级 Host

新增 crate 只能公开项目级 typed Rust Interface。禁止出现以下 AI 可见或协议层入口：

```text
authoring_context.open/refresh/snapshot tools
engine.catalog
engine.execute(toolId, payload)
engine(prompt)
daemon discovery / connect_editor
模型循环、目标规划或工具选择
```

## 5. 预期文件与 owner

具体文件拆分可在激活前按当前代码小幅调整，但不得扩大 owner 集合：

| 文件 | 责任 |
| --- | --- |
| `rust/Cargo.toml` | 注册新 workspace crate |
| `rust/Cargo.lock` | 只记录已有依赖组合，不引入无关第三方依赖 |
| `rust/crates/authoring_project_context/Cargo.toml` | 最小依赖与 lint inheritance |
| `rust/crates/authoring_project_context/src/lib.rs` | 小而稳定的公开入口与 re-export |
| `rust/crates/authoring_project_context/src/source_inventory.rs` | source policy、scan、内容 identity |
| `rust/crates/authoring_project_context/src/context.rs` | embedded open/refresh/close、handle/revision |
| `rust/crates/authoring_project_context/src/snapshot.rs` | bounded path scope 与 owned immutable bytes |
| `rust/crates/authoring_project_context/src/diagnostic.rs` | open/refresh/snapshot typed diagnostics |
| `rust/crates/authoring_project_context/src/tests.rs` | owner级确定性、invalid、drift、snapshot测试 |
| `rust/crates/editor_core/Cargo.toml` | 依赖 neutral owner |
| `rust/crates/editor_core/src/candidate_project_revision.rs` | 仅保留 candidate职责，digest委托 neutral owner |

如果实现必须移动 `ProjectManifest`、`ProjectWriteScope` 或大量 candidate staging 代码，停止并修订本文，不能
以“顺手清理依赖”为由扩大施工。

## 6. Gate A：Neutral Owner 与 Headless Open

### 6.1 红能力

先写 owner 测试，证明在不依赖或构造 `EditorSession` 的情况下：

- 可打开最小 `project.aife.json` fixture；
- 缺失根、非目录、manifest unreadable/invalid、空 `projectId` 返回 typed diagnostic；
- returned handle 是 process-local opaque value，公开 API 不暴露可伪造的 root fingerprint authority；
- crate normal dependency closure 不包含 `editor_core`、任意 `editor_*` 或 `ai_tool_gateway`。

红测试必须先因 neutral owner 尚不存在或行为缺失而失败；不得用字符串搜索代替行为测试。

### 6.2 最小实现

- 建立新 crate 和 embedded context；
- 只解析 Context 所需 manifest header，不复制 Editor launcher 的 UI/settings逻辑；
- canonicalize root 并绑定 portable identity + process-local root binding；
- `open` 内部执行首次 refresh；
- diagnostics 只覆盖 open/refresh/snapshot 当前需要的 code/stage/path/next action。

### 6.3 Gate 命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib open_
$metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
$package = $metadata.packages | Where-Object name -eq 'authoring_project_context'
$forbidden = $package.dependencies | Where-Object { $_.name -match '^(editor_|editor_core$|ai_tool_gateway$)' }
if ($null -ne $forbidden) { throw "authoring_project_context has forbidden normal dependencies" }
```

`cargo metadata` 只检查新 crate 的 normal dependencies；若包含禁止依赖，Gate 失败。不要为该检查新增通用
architecture runner。

### 6.4 Gate A 完成证据

- owner测试名称、首次红因、转绿结果；
- 新 crate normal dependency 列表；
- 明确记录未启动 Editor/Gateway、未修改外部状态。

## 7. Gate B：Canonical Inventory、Revision 与旧 Digest 兼容

### 7.1 红能力

建立最小矩阵：

1. 相同 bytes + policy 得到相同 revision；
2. canonical source 内容变化改变 revision；
3. `Library/Build/target/.git/.aife/.aife-candidates` 和 nested Cargo `target` 变化不改变 revision；
4. mtime-only 变化不改变 revision；
5. case-fold collision、link/reparse、扫描期间变化 fail-closed；
6. invalid manifest bytes 产生新的 invalid content identity，不继续返回旧 ready revision；
7. clean refresh 不写 canonical 文件；
8. 现有 candidate digest fixture 在委托前后完全相同。

### 7.2 最小实现

- 从 `candidate_project_revision.rs` 迁出或复用 source inventory、排除策略和 streaming hash；
- 新 revision 使用 309 的 project identity + source policy version + source digest；
- 为旧 candidate consumer保留明确的 legacy digest projection，禁止复制第二套扫描器；
- candidate staging/apply/discard 逻辑留在 `editor_core`，不在本 Gate 重构；
- 不加入 watcher；warm cache 只有在测试证明必要时才允许最小实现，否则延期。

### 7.3 Gate 命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib revision_
cargo test -p authoring_project_context --lib inventory_
cargo test -p editor_core --lib candidate_project_revision::tests
cargo test -p editor_core --lib project_open_preparation::tests
```

后两个 `editor_core` 命令是共享 digest consumer 的兼容验证，不运行完整 `editor_core`。

### 7.4 Gate B 完成证据

- revision/inventory矩阵结果；
- legacy digest before/after fixture identity；
- changed/excluded paths 和首个失败 diagnostic；
- 没有新增第二套 source policy 的代码审查结论。

## 8. Gate C：Bounded Immutable Read Snapshot

### 8.1 红能力

测试必须证明：

- snapshot 只读取请求的 canonical source paths；
- snapshot 返回后修改 live root，既有 snapshot bytes 不变；
- 修改后下一次 refresh 得到新 revision，新 snapshot 得到新 bytes；
- snapshot构建期间路径内容漂移返回 drift，不混合两个 revision；
- 请求 excluded、missing、escaping 或 unsupported path 返回 typed diagnostic；
- snapshot对象不保存可供调用方事后读取 live root 的公开路径能力。

### 8.2 最小实现

- 只实现显式 path list 的 bounded scope；
- 返回 owned bytes + path digest + snapshot revision binding；
- 读取后逐项验证 digest，并在 publication 前确认 handle revision仍匹配；
- 不建立磁盘 snapshot store、lease daemon、content-addressed blob GC 或 Compiler integration。

### 8.3 Gate 命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib snapshot_
cargo test -p authoring_project_context --lib
cargo fmt --check -p authoring_project_context -p editor_core
```

完整新 crate suite 已包含 Gate A-B，同一配置下不再重复运行前两个过滤测试。

### 8.4 Gate C 完成证据

- live-root变更前后 snapshot/revision identity；
- drift和非法scope首个diagnostic；
- snapshot不持有live read capability的代码审查结论；
- 格式检查结果。

## 9. 终态回归与完成声明

Gate C 的完整 crate suite 已覆盖新 owner。终态只补直接共享 consumer：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core --lib candidate_project_revision::tests
cargo test -p editor_core --lib project_open_preparation::tests
cargo check -p authoring_project_context -p editor_core
```

若 Gate C 后 `editor_core` 相关文件未再变化，可复用 Gate B 的两项 consumer evidence，不重复运行。`cargo check`
只验证受影响 crate closure，不升级为全 workspace 或 Local CI。

允许的完成声明：

```text
309-A Headless Neutral Read Authority sourceAvailable
G1 的 open/refresh 和 bounded inspect snapshot 已形成源码证据
G2/G3/G4 只在本文覆盖范围内部分通过
```

禁止声明：

```text
309或R1完成
统一mutation完成
Editor和Headless已统一
AI已经能直接调用一等Engine tools
Compiler/run/build/交付链已完成
production或release可用
```

## 10. 失败、停止与修订规则

- 任一 Gate 失败，先保存首个可复现 owner diagnostic，不跨 Gate；
- 共享 digest 委托改变旧 digest protocol 时，优先修复兼容 projection，不迁移所有 consumer；
- 若无法在列出文件内完成，停止并判断是方案错误、基线漂移还是需要独立后续施工；
- 不得为单次 Windows 文件系统测试创建永久 runner；
- 不得因完整 workspace 的无关脏状态扩大验证；
- 施工达到 2小时15分钟仍未进入 Gate C 时停止新增实现，复核范围；
- 2小时45分钟后不开始新 Gate；3小时必须停止并报告 `stopped_by_time_limit`。

## 11. 激活前复核

移动到 `当前/` 前必须逐项确认：

1. Tower P1-2 已完成、归档或由用户明确正式退出，且全仓无其它激活施工；
2. 309 方案和自审报告仍为当前 authority；
3. `rust/Cargo.toml`、`Cargo.lock`、`editor_core/Cargo.toml`、
   `candidate_project_revision.rs` 的当前用户修改已逐项理解并可保留；
4. 没有其它分支/任务正在修改相同 owner；
5. 新 crate 名和依赖方向未与更晚方案冲突；
6. 所有 Gate 命令在当前 toolchain 下有效；
7. 保守估算仍能在三小时窗口内完成；
8. 复核后重新完成本文第 12 节自审；
9. 文档移动到 `施工文档/当前/` 并同步 `54` 后，才获得施工授权。

当前第 3 项已知存在重叠用户修改，因此不能跳过激活前复核。

### 11.1 2026-08-31 激活记录

```text
activationTime：2026-08-31T11:48:53+08:00
repository HEAD：acaffa8cb95bbb54d093bbf45835a59315c82da5
branch：codex/255-capability-catalog
rustc：1.96.0 (ac68faa20 2026-05-25)
cargo：1.96.0 (30a34c682 2026-05-25)
309方案SHA-256：E9492C819F28DA300B7964801083EBA1C173EB3CD6AC2966FEBD8E0A1D58CF21
激活前本文SHA-256：97D9D2381E144998F82A9B2A08B8792D8CE73739E5A8360BF6AF79D737F25CC7
rust/Cargo.toml SHA-256：E4C6D78560361DFB5919B4158F3F0878DBAC65CE6A152E3FBA32887631011561
rust/Cargo.lock SHA-256：AAA0E781AB5264E513845C896FFEA4D06368D1919BBA47FC080A3A969DF3B094
editor_core/Cargo.toml SHA-256：09FE4836C1CDBCFE2C8C759DD56B9936B2055D63688D3FDC59FB23A6172DE712
candidate_project_revision.rs SHA-256：2C4BBAF566D172B0E47240E23CFF055B113834A795C1A0BD637E063C20F288DF
Tower P1-2：用户明确暂停，文档退回项目待执行，项目现场不回滚
global current：没有其它活动施工文档；254文件仅为历史evidence
overlap audit：相关文件已有用户修改；streaming digest、cancellable scan、nested Cargo target与新增依赖全部保留
scope adjustment：无需；仍只修改本文第5节owner集合
test commands：与rustc/cargo 1.96.0兼容
estimated window：小于三小时
preflightResult：PASS；允许从Gate A红测试开始
```

## 12. 施工文档自审

```text
方案一致性：通过；只实现309 Stage A/B中的最小只读纵切，不改变方案B
权威边界：通过；Context仍是内部Module，不注册AI工具或二级Host
范围：通过；一个新owner crate + 一个既有digest consumer委托
过量施工：通过；mutation/recovery/Editor/Gateway/Compiler/Provider全部延期
红能力：通过；每个Gate都有可捕获对应失败的owner或直接consumer测试
验证经济性：通过；不运行完整workspace、Local CI、真实Editor或production环境
用户修改保护：通过；重叠文件列为激活前硬复核，当前不改源码
外部审查：309未提供独立外部AI审查文档；正式方案自审无需修改方案
当前状态真实性：通过；309-A已完成并归档，Tower P1-2保持暂停且未进入当前施工槽
完成声明：通过；只允许sourceAvailable，不宣称309/R1/AI工具体验完成
```

## 13. 完成后的文档动作

只有 Gate A-C 和终态回归满足本文声明后，才执行：

- 生成 `阶段完成记录/2026-xx-xx-309-A-Headless-Neutral-Read-Authority-v1/00-总览.md`；
- 将本文从 `施工文档/当前/` 移到 `施工文档/已完成/`；
- 同步 `README.md`、`00-文档地图.md`、`00-阅读顺序.md`、`49`、`54`、
  `施工文档/README.md` 和 `阶段完成记录/README.md`；
- 明确列出 309 剩余的 immutable snapshot materialization/lease、unified mutation、recovery、Editor Adapter
  和 Gateway retirement；
- 不自动生成或激活下一份施工文档。

## 14. 2026-08-31 完成证据

```text
完成时间：2026-08-31T12:19:21+08:00
源码基线：acaffa8cb95bbb54d093bbf45835a59315c82da5（dirty worktree development evidence；非exact-commit/release资格）
Gate A红：open_ 5/5因authoring_context.not_implemented失败
Gate A绿：open_ 5/5 passed；normal dependencies仅serde/serde_json/sha2
Gate B红：revision_ 3/3因refresh_not_implemented失败；inventory_ 1/1因inventory_not_implemented失败
Gate B绿：revision_ 3/3 passed；inventory_ 4/4 passed
旧digest兼容：candidate_project_revision::tests 12/12 passed；手工project-tree-digest.v1字节协议一致
直接consumer：project_open_preparation::tests 1/1 passed
Gate C红：snapshot_ 4/4因snapshot_not_implemented失败
Gate C绿：snapshot_ 4/4 passed；authoring_project_context完整suite 16/16 passed
格式：cargo fmt --check -p authoring_project_context -p editor_core passed
终态：cargo check -p authoring_project_context -p editor_core passed
复用证据：Gate C后editor_core未再变化，终态未重复运行两项Gate B consumer
外部状态：未启动Editor/Gateway，未修改真实配置、production二进制或项目canonical source
```

实现结论：

- `authoring_project_context` 是唯一 neutral source inventory、source policy、revision 与 bounded snapshot owner；
- `open` 内部完成首次 refresh，后续 manifest 损坏发布新的 invalid revision，不回退旧 ready事实；
- inventory 由内容而非mtime决定identity，排除generated/nested Cargo target，并对case-fold、link/reparse与扫描漂移fail-closed；
- snapshot只物化显式路径为owned bytes，不公开root或事后live-read能力；
- `CandidateProjectRevisionStore::project_digest*` 保留原 `project-tree-digest.v1` 字节协议并委托neutral owner；
- 未新增AI tool、Host、daemon、mutation、journal、recovery、Editor/Gateway/Compiler/Runtime/Build接入。

完成声明严格限制为：

```text
309-A Headless Neutral Read Authority sourceAvailable
G1 open/refresh/bounded inspect snapshot 已有源码证据
G2/G3/G4 仅在309-A覆盖范围内部分通过
```

309剩余工作仍为 immutable snapshot materialization/lease、unified mutation/CAS、journal/recovery、Editor
Adapter、Gateway authority retirement，以及Compiler/Provider的一等Engine tool消费；这些均未由本文授权或完成。
