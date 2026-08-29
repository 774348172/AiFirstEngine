# Asset DB / Importer MVP

本文档记录 Asset DB / Importer MVP 的第一版实现规则。

## 定位

Asset DB 是资源真相层的一部分，但它不替代项目语义。

```text
项目侧决定资源是什么、资源被谁引用、什么时候加载和释放。
引擎侧记录资源元数据、GUID、导入器、依赖、外部文件状态和导入锁。
Build Graph / 热更 / AI 资源生成都通过 Asset DB 读取资源状态。
```

第一版采用完整 Unity / UE 式 Importer 系统骨架，但每种资源类型只开放最小导入设置。  
它不是“只登记文件、以后再补导入器”的过渡方案，而是从第一版开始建立正式 Importer 架构。

## Asset Pipeline v1 总方案

Asset Pipeline v1 是资源从“外部文件 / AI 生成文件 / 用户导入文件”进入项目正式资源体系的完整闭环。

它一次性覆盖：

```text
Asset DB
Meta
ImporterContract
ImportSettings
ImportResult
SubAsset
Reimport
Artifact Cache
Dependency Graph
ImportTransaction
AssetImportUnit
ImportReport
Failure / Blocked / Per-unit Commit
Editor Import Lock
AI Repair Candidate
```

参考引擎对比：

| 引擎 | 核心路线 | 优点 | 问题 | 本项目取舍 |
|---|---|---|---|---|
| Unity | AssetDatabase + .meta + Importer + Artifact + dependency hash | 资源身份稳定，导入流程成熟，编辑器体验好 | 很多内部规则不可见，AI 难解释 | 学 .meta / guid / importer / artifact，补结构化报告 |
| UE | AssetRegistry + UFactory / Reimport + DDC + CookDependency | 大项目强，构建缓存和依赖系统成熟 | 系统重，用户理解成本高 | 学长期缓存 / 依赖架构，第一版做 D-min |
| Bevy | AssetServer + AssetProcessor + processed asset + full_hash | Rust 化、hash 驱动、依赖变化追踪清晰 | 编辑器型资源管线不如 Unity / UE 完整 | 学 processed info / dependency hash 的简洁性 |
| Godot | .import + imported cache | 简单直接 | 大项目资源诊断和 AI 可解释性弱 | 参考简洁边界，不采用其能力上限 |

本项目路线：

```text
Unity 的资源身份稳定性
+ UE 的长期缓存 / 依赖架构
+ Bevy 的 Rust 化和 hash 驱动
+ AI-first 的结构化报告
```

完整流程：

```text
FileChange / Manual Import / AI Generated Asset
  -> ImportPlan
  -> ImportTransaction begin
  -> Importer execute
  -> ImportResult
  -> Dependency Extraction
  -> Artifact Cache write
  -> Asset DB per AssetImportUnit commit
  -> ImportReport / BatchImportReport / ReimportReport / CacheReport / DependencyReport
  -> Editor refresh
  -> AI Debug / Repair Candidate
```

核心模块职责：

```text
Asset DB：
项目资源身份、状态、引用、依赖图的真相源。

Meta：
保存 guid / assetId / importer / settings。
类似 Unity .meta，但必须保持 AI 可读。

ImporterContract：
每种资源类型的导入契约。
Texture / Audio / Model / Material / Scene / Prefab / DSL-IR 都必须有。

ImportResult：
一次导入的结构化产物。
包含 mainAsset / subAssets / dependencies / diagnostics。

Artifact Cache：
可删除、可重建的导入产物缓存。
长期路线类似 Unity Artifact + UE DDC，第一版只做本地 D-min。

Dependency Graph：
引擎级依赖系统。
记录 sourceFile / assetRef / artifact / importRule / buildRule。

ImportTransaction：
所有导入变更必须通过事务提交。
失败时不污染 Asset DB。

Reports：
ImportReport / ReimportReport / CacheReport / DependencyReport。
供用户、AI、Console、Debug View 读取。
```

## Asset Pipeline v1 最终施工边界

Asset Pipeline v1 第一版目标是建立正式资源管线骨架，并为每种核心资源类型产出最小数据产物。  
第一版不是完整美术资产处理系统，不追求 Unity / UE 级别的纹理压缩、模型处理、shader 编译和平台 cook 能力。

第一版最小资源类型：

```text
Texture
Audio
Model
Material
Scene
Prefab
Logic / DSL-IR
```

每类资源第一版必须支持：

```text
能识别文件。
能生成 meta。
能生成 AssetImportUnit。
能输出 ImportResult。
能提取直接依赖。
能写 ArtifactRecord。
能进入 AssetRegistry。
能生成失败报告。
```

第一版不做：

```text
真实纹理压缩矩阵。
真实模型骨骼 / 动画 retarget。
真实 shader 编译。
真实音频转码矩阵。
真实预览缓存。
真实 GPU 上传。
完整平台 cook。
```

施工原则：

```text
优先固定正式 ImporterContract / ImportResult / ArtifactRecord / DependencyRecord / AssetImportUnit / Report 骨架。
具体资源类型只做最小导入设置和最小产物。
不要为了某一种资源的高级处理能力破坏统一管线结构。
后续增强资源处理能力时，只扩展对应 Importer 和 Artifact，不改变 Asset DB 真相层。
```

关键规则：

```text
每个 main asset 有稳定 guid。
AssetRef = assetId + guid + type。
SubAsset 不独立生成 guid。
SubAsset 身份 = parent guid + subAsset.kind + subAsset.id。
不提供 SubAsset 提取成独立资源。

Reimport 保持 main asset guid。
Reimport 用 old ImportResult 和 new ImportResult 做 diff。
SubAsset 只按 kind + id 匹配。
删除、改名、冲突不自动迁移引用，必须进入 ReimportReport。

Artifact Cache 不是项目真相源。
删除 Artifact Cache 只会导致重新导入。
ArtifactKey 必须包含 sourceHash / importerVersion / settingsHash / platform / dependencyHash / engineImportVersion。

只保存直接依赖。
递归依赖通过 DependencyGraph 查询。
依赖变化通过 ReverseDependencyIndex 找到受影响资源。
import / build 依赖循环是 error。
runtime 依赖循环第一版是 warning。

Importer 不直接写 Asset DB。
Importer 先写临时 ImportTransaction workspace。
只有单个 AssetImportUnit 的 ImportResult / ArtifactRecord / DependencyRecord 都有效时，该 AssetImportUnit 才能 commit。
AssetImportUnit commit 失败必须保留该资源旧 Asset DB 状态。
失败生成 ImportReport，不自动修复。
AI Repair Candidate 只能 report-only，不能自动删文件、改引用、重导入。
```

第一版必须实现：

```text
Texture / Audio / Model / Material / Scene / Prefab / DSL-IR 的最小 ImporterContract。
本地 Asset DB。
本地 Meta。
本地 Artifact Cache。
ImportTransaction 按 AssetImportUnit 提交。
ImportResult / ImportReport。
DependencyRecord / DependencyGraph。
Reimport C 规则。
编辑器导入锁。
AI Debug 读取报告。
```

第一版不实现：

```text
远程缓存。
团队共享缓存。
复杂 LRU。
完整模型骨骼 / 动画 retarget。
完整 shader 编译管线。
资源高级预览缓存。
运行时动态依赖追踪。
依赖图高级可视化。
自动修复资源引用。
```

本方案冻结后，Asset Pipeline v1 不再继续按单个小规则无限拆分讨论。  
后续只在影响长期路线或施工失败时，才重新打开具体细节规则。

## Bevy Asset 参考边界

Bevy 的 AssetServer / AssetLoader / Handle / Assets<T> / AssetProcessor 对第一版 Asset DB 有参考价值：

```text
AssetServer 负责把路径或资源请求转成可跟踪加载任务。
AssetLoader 负责不同类型资源的导入。
Handle 是运行时引用，不是源项目文件里的语义引用。
Assets<T> 是运行时已加载资源表。
AssetProcessor 负责导入、转换和缓存。
```

本项目采用更 AI 友好的分层：

```text
项目源数据使用 AssetRef / AssetSlot / AssetSet。
Asset DB 记录 guid、sourcePath、importer、dependencies、state。
Runtime 内部可以有 Handle，但 Handle 不写回项目源文件。
Build Graph 根据 Asset DB / Asset Graph 生成 Bundle 和 cooked asset。
```

## 参考引擎结论

Asset DB / Importer v1 采用以下参考结论：

```text
Unity：资源身份由引擎自动生成，项目引用不保存裸路径，底层以 GUID / fileID 保持稳定。
UE：AssetRegistry 自动扫描和维护资源索引，依赖和反向引用通过 Registry 查询。
Bevy：AssetServer / AssetLoader / AssetProcessor 负责加载、处理、hash、processed info 和依赖状态。
```

本项目采用：

```text
Unity-like 自动 meta / GUID。
UE-like 轻量 Asset Registry / dependency / referencer 查询。
Bevy-like loader state / hash / processed info 思路。
AI-native 可读 assetId / import report / repair report。
```

正式规则：

```text
资源身份和索引由引擎自动生成。
用户和 AI 不手写底层 Asset DB。
用户和 AI 不手写散落裸 AssetRef。
路径只是资源当前位置，不是长期身份。
```

## Meta 自动生成规则

普通资源文件进入项目时，Asset DB / Importer 自动生成 `asset.meta.json`。

```text
用户拖入 / 复制资源文件 -> 自动生成 meta。
外部工具或 SVN 拉取资源 -> 扫描后自动生成缺失 meta。
AI 生成资源文件 -> 生成资源文件后自动生成 meta。
```

不采用“普通导入先生成 meta proposal 等用户确认”的路线。

原因：

```text
meta 是技术身份，不是设计决策。
用户导入资源时不应被迫确认每个 meta。
AI 生成大量资源时不应被 meta 确认流程卡住。
真正需要确认的是资源是否绑定到 AssetSlot、是否替换旧资源、是否删除引用。
```

AI 资源生成中的 `AssetRegistrationPlan / asset.meta proposal` 只用于“候选资源进入项目与绑定引用”的审查流程。  
它不改变普通资源导入的规则：普通资源身份 meta 由引擎自动生成。

## AssetRef 保存规则

项目长期 AssetRef 保存：

```text
assetId
guid
type
subAsset? 可选
```

示例：

```json
{
  "assetId": "player_ship_texture",
  "guid": "8c2f2a6d-7c4d-4e10-89fd-4eae5f16f8a1",
  "type": "Texture"
}
```

带子资源的示例：

```json
{
  "assetId": "enemy_model",
  "guid": "2a1f...",
  "type": "Mesh",
  "subAsset": {
    "id": "mesh_body",
    "kind": "Mesh"
  }
}
```

AssetRef 不保存：

```text
sourcePath
bundlePath
runtime handle
cooked asset path
GPU resource id
```

解析规则：

```text
优先用 guid 定位资源。
assetId 用于 AI / 用户可读性和一致性校验。
type 用于验证层快速检查。
guid 找到但 assetId 不一致 -> warning。
assetId 找到但 guid 不一致 -> conflict。
二者都找不到 -> missing。
```

原因：

```text
只保存 guid 对 AI 和用户不友好。
只保存 assetId 对冲突检测、移动恢复和团队协作不够稳定。
assetId + guid + type 能同时满足 AI 可读、机器稳定和验证友好。
```

## ImportTransaction / ImportReport 规则

资源导入不能由 Importer、AI 或编辑器面板直接零散写入 Asset DB。  
所有导入、刷新、外部文件同步和批量资源变更必须先进入导入事务，再由事务按资源单元提交 Asset DB。

参考引擎结论：

```text
Unity：AssetImportContext 能记录导入 warning / error，OnPostprocessAllAssets 能收到 imported / deleted / moved 结果。
UE：AssetRegistry 维护 Added / Removed / Renamed / Updated 事件，并在大项目中依赖资源索引和报告。
Bevy：AssetProcessor 使用 transaction log，导入前 begin，成功后 end，启动时能检查未完成事务。
```

本项目采用：

```text
FileChangeBatch
  -> ImportPlan
  -> ImportTransaction
  -> Asset DB per AssetImportUnit commit
  -> ImportReport
```

正式规则：

```text
ImportTransaction 是导入执行真相。
ImportReport 是用户和 AI 读取的解释结果。
Asset DB 只能通过 ImportTransaction 更新。
Importer 不允许直接写正式 Asset DB。
AI 不允许绕过 ImportTransaction 修改 Asset DB。
每次导入必须生成 ImportReport。
```

事务提交规则：

```text
scan batch
-> plan AssetImportUnit
-> import temp result per AssetImportUnit
-> validate per AssetImportUnit
-> commit successful AssetImportUnit
-> mark failed / blocked AssetImportUnit
-> generate BatchImportReport
```

批量导入不是整批原子回滚。  
每个 AssetImportUnit 独立成功 / 失败。  
成功资源提交到 AssetPipelineDatabase。  
失败资源进入 failed 状态，并生成 ImportReport。  
如果失败的是核心依赖资源，依赖它的资源标记为 blocked / dependency_failed。  
整个批次最后生成 BatchImportReport。  
失败资源不会污染成功资源；失败资源的旧有效 Asset DB 状态继续保留。

BatchImportReport v1 最小规则：

```text
记录本批次 added / modified / deleted / moved / success / failed / blocked / warnings。
记录每个失败 AssetImportUnit 的失败原因。
记录 dependency_failed 链路。
记录本批次成功提交了哪些 AssetImportUnit。
记录本批次没有提交哪些 AssetImportUnit。
只解释结果，不自动修复项目。
```

AssetImportUnit v1 标准结构：

```text
AssetImportUnit 是资源导入最小执行、提交、失败、阻塞单位。
一个 AssetImportUnit 对应一个 main asset。
SubAsset 不单独生成 AssetImportUnit。
AssetImportUnit 只保存执行输入、状态和轻量诊断，不保存完整导入产物。
```

AssetImportUnit v1 字段：

```text
unitId
batchId
sourcePath
previousSourcePath?
changeKind: added | modified | deleted | moved | meta_modified
action: import | reimport | mark_missing | mark_deleted | resolve_conflict | skip
guid?
assetId?
type?
importerId?
importerVersion?
settingsHash?
sourceHash?
dependencyKeys?
state: pending | importing | success | failed | blocked | skipped
blockReason?
failureReason?
startedAt?
finishedAt?
```

AssetImportUnit v1 不保存：

```text
完整 ImportResult。
完整 ArtifactRecord。
完整 ImportReport。
资源二进制。
运行时 Handle。
GPU Resource id。
用户编辑器选择状态。
```

AssetImportUnit v1 执行关系：

```text
FileChangeBatch
  -> ImportBatchPlan
  -> AssetImportUnit[]
  -> ImportWorker 执行每个 Unit
  -> 成功 Unit commit
  -> 失败 Unit failed
  -> 依赖失败 Unit blocked / dependency_failed
  -> BatchImportReport
```

ImportTransaction v1 最小结构：

```json
{
  "schemaVersion": "asset-import-transaction.v1",
  "transactionId": "import_20260624_001",
  "source": "full_scan",
  "state": "planned|running|committed|failed|cancelled",
  "startedAt": "2026-06-24T12:00:00Z",
  "finishedAt": null,
  "inputBatchId": "file_batch_001",
  "units": [
    {
      "unitId": "unit_001",
      "batchId": "file_batch_001",
      "sourcePath": "Assets/Textures/player.png",
      "changeKind": "added",
      "action": "import",
      "assetId": "player_texture",
      "guid": "8c2f...",
      "type": "Texture",
      "importerId": "TextureImporter",
      "state": "success",
      "failureReason": null
    }
  ]
}
```

ImportReport v1 最小结构：

```json
{
  "schemaVersion": "asset-import-report.v1",
  "transactionId": "import_20260624_001",
  "summary": {
    "added": 12,
    "modified": 3,
    "deleted": 1,
    "moved": 2,
    "failed": 1,
    "warnings": 2
  },
  "issues": [
    {
      "severity": "error",
      "code": "meta_conflict",
      "assetPath": "Assets/Textures/enemy.png",
      "assetId": "enemy_texture",
      "guid": "old-guid",
      "message": "资源 meta 中的 guid 与 Asset DB 记录不一致。",
      "nextAction": "保留旧 guid 或把它作为新资源导入，需要用户确认。"
    }
  ],
  "affectedReferences": [
    {
      "assetId": "enemy_texture",
      "guid": "old-guid",
      "usedBy": [
        "Scenes/Main.scene",
        "Prefabs/Enemy.prefab"
      ]
    }
  ]
}
```

ImportReport 只解释结果，不自动修复项目。  
后续 AI 修复必须先读取 ImportReport，再生成可审查 Patch Plan 或 Asset Repair Plan。

## Asset Pipeline 编辑器 / 文件同步 / Build 接入规则

Asset Pipeline v1 不能停留在孤立数据层。编辑器导入、外部文件变化和 Build Pipeline 必须读取同一套 AssetPipelineDatabase / ArtifactStore / DependencyGraph。

参考引擎结论：

```text
Unity：文件进入项目后由 AssetDatabase / Importer / Refresh 统一处理，BuildPipeline 读取导入后的资源状态。
UE：AssetRegistry / UFactory / Reimport / Cook 形成资源索引、导入和构建边界，Cook 不直接依赖散乱源文件语义。
Bevy：AssetProcessor / processed assets 使用 hash 和依赖变化决定资源是否需要重新处理。
Godot：.import / imported cache 路线简单，但诊断和依赖解释能力弱。
```

本项目采用：

```text
FileChangeBatch
  -> FileChangeImportPlan
  -> ImportTransaction
  -> AssetPipelineDatabase
  -> AssetPipelineEditorState
  -> BuildAssetManifest
  -> BuildAssetPreflightReport
```

正式规则：

```text
FileChangeBatch 只记录文件事实，不承载项目语义。
added 进入 import。
modified 进入 reimport。
deleted 只标记 missing 并输出 affected assets，不物理删除文件，不自动改引用。
moved 优先保持原 guid；无法确认 previousPath 时生成 conflict report。
编辑器只读 AssetPipelineEditorState，不直接修 Asset DB。
大批量导入期间必须持有 AssetImportLock，编辑器进入全局导入占用态。
全局导入占用态期间，用户不能读写 Project / Asset 面板内容，AI Patch 不能并发写入。
全局导入占用态只允许显示导入进度、当前阶段和最终报告入口。
导入完成后统一刷新 AssetPipelineEditorState / ProjectDock / Inspector / Console。
Build Pipeline 读取 BuildAssetManifest，不重新扫描散文件作为资源真相。
runtime required missing / failed 资源必须让 Build preflight failed。
editor-only missing / failed 资源只产生 warning。
modified 资源产生 reimport required warning，不能静默使用旧 artifact。
AI 修复候选仍是 report-only / requiresUserApproval / canAutoApply=false。
```

第一版边界：

```text
实现纯数据 FileChangeBatch / EditorBridge / BuildBridge。
不实现真实 OS file watcher。
不实现真实 Import Worker。
不实现真实 ProjectDock / Console UI 深度接入。
不实现真实 GPU 上传、真实 Bundle 二进制打包、真实平台打包。
```

## 真实 File Watcher / Import Worker C-min 规则

真实 file watcher / Import Worker 采用 C-min 路线：学习 UE 的 AssetRegistry / AssetTools / Cook 边界，保留长期正确骨架，但第一版只实现最小可解释子集。

参考引擎结论：

```text
Unity：AssetDatabase.Refresh / ImportAsset / StartAssetEditing / StopAssetEditing / AssetPostprocessor.OnPostprocessAllAssets 表明文件变化会进入统一 AssetDatabase / Importer / Postprocessor 流程，而不是 watcher 直接写资源结果。
UE：AssetRegistry 提供 ScanPathsSynchronous / GetAssetsByPath / GetDependencies / GetReferencers / AssetCreated / AssetDeleted / AssetRenamed / OnFilesLoaded，说明资源索引、依赖查询和资源事件是独立系统。
Bevy：FileWatcher 使用 notify_debouncer_full，并把事件整理为 AddedAsset / ModifiedAsset / RemovedAsset / RenamedAsset / AddedMeta / ModifiedMeta / RemovedMeta / RenamedMeta / RemovedUnknown，说明真实文件系统事件必须先 debounce 和分类。
Godot：.import / imported cache 简洁，但复杂依赖解释、AI 调试和大项目诊断能力弱，只参考边界，不采用弱报告路线。
```

本项目长期骨架：

```text
FileWatcher
  -> RawFileEvent
  -> FileChangeCollector
  -> FileChangeBatch
  -> AssetEventLog
  -> AssetRegistry
  -> ImportBatchPlan
  -> ImportQueue
  -> ImportWorker
  -> ImportTransaction
  -> AssetPipelineDatabase
  -> ImportReport / DependencyReport / CacheReport
```

C-min 第一版正式规则：

```text
FileWatcher 不直接导入资源。
FileWatcher 不直接写 AssetPipelineDatabase。
FileWatcher 只产生 RawFileEvent。
FileChangeCollector 负责 debounce、去重、合并和排序。
FileChangeBatch 是文件变化进入资源管线的第一层稳定数据。
AssetEventLog 只做审计和回溯，不是资源真相。
AssetRegistry 只做快速索引和查询，不是资源真相。
AssetPipelineDatabase 是唯一资源真相。
第一版不实现完整 ImportJobGraph，改为 ImportBatchPlan。
ImportBatchPlan 只决定本批次哪些 AssetImportUnit 需要 import / reimport / mark missing / conflict。
ImportQueue 第一版是单队列。
ImportWorker 第一版是单 worker 串行执行。
ImportWorker 只能通过 ImportTransaction 按 AssetImportUnit 提交。
批量导入不做整批回滚；成功 AssetImportUnit 提交，失败 AssetImportUnit 进入 failed report。
核心依赖失败时，下游 AssetImportUnit 标记为 blocked / dependency_failed。
导入失败只生成 report，不自动修复、不自动删除文件、不自动迁移引用。
AI 只读取 ImportReport / AssetEventLog / Registry 查询结果，再生成可审查 Patch Plan。
```

第一版不实现：

```text
完整 ImportJobGraph。
多线程 ImportWorker Pool。
复杂优先级调度。
跨资源并行导入。
复杂 dependency scheduler。
远程缓存 / 分布式导入。
自动修复引用。
真实资源预览生成。
真实 GPU 上传。
```

复杂项目支撑原则：

```text
批量 SVN / Git / 外部工具同步必须先收束成 FileChangeBatch。
大批量导入期间必须进入全局导入占用态，Project / Asset 面板不可读、不可写、不可交互。
占用态只显示导入进度、当前阶段和导入完成后的报告入口。
导入完成前不展示半成品资源状态。
移动资源优先通过 meta guid / previousPath 保持身份；无法确认时进入 conflict report。
删除资源只标记 missing 并报告 affected assets，不自动清理引用。
Build Pipeline 只读取 AssetPipelineDatabase / AssetRegistry / BuildAssetManifest，不直接把散文件扫描当真相。
```

## AssetRegistry v1 轻量索引规则

AssetRegistry v1 采用轻量派生索引路线：学习 UE 的 AssetRegistry 查询能力，但不复制 UE Package / UObject 复杂度；学习 Unity AssetDatabase 的统一资源身份，但不把 Registry 做成黑盒真相；学习 Bevy 的事件和 handle 分离，但不把运行时 Handle 写入项目资源索引。

参考引擎结论：

```text
Unity：AssetDatabase 提供 Refresh / ImportAsset / MoveAsset / DeleteAsset / RenameAsset / GUID 查询等能力，但用户侧看不到完整内部索引结构。
UE：AssetRegistry 是独立查询层，支持 GetAssets / GetAssetsByPath / GetDependencies / GetReferencers / ScanPathsSynchronous / AssetCreated / AssetDeleted / AssetRenamed。
Bevy：AssetServer 支持 path -> id / handle 查询，AssetEvent 与 FileWatcher event 分层，但 Handle 属于运行时引用，不是项目资源真相。
Godot：资源导入索引更轻，但复杂项目依赖诊断、反向引用和 AI 可解释性不足。
```

本项目正式规则：

```text
AssetPipelineDatabase 是资源唯一真相。
AssetRegistry 是从 AssetPipelineDatabase 派生出来的轻量索引。
AssetRegistry 必须可重建。
AssetRegistry 不允许被 AI / UI / Importer 直接写入。
AssetEventLog 是审计历史，不是资源真相。
ImportReport 是失败原因和解释结果，不是资源真相。
所有正式资源写入只能来自 ImportTransaction commit。
```

AssetRegistry v1 保存：

```text
schemaVersion
registryVersion
builtFromDatabaseVersion
updatedAt

assetsByGuid
assetsByAssetId
assetsBySourcePath
assetsByType
assetsByState
assetsByImporter

dependenciesByGuid
referencersByGuid

lastImportReportByGuid
lastKnownArtifactByGuid
```

AssetRegistryEntry v1 保存：

```text
guid
assetId
type
sourcePath
metaPath?
state
sourceHash
importerId
importerVersion
artifactId?
lastImportReportId?
updatedAt
directDependencies
directReferencers
```

AssetRegistry v1 不保存：

```text
完整 ImportResult。
完整 ArtifactRecord。
完整 ImportReport 内容。
完整 AssetEventLog。
完整源文件内容。
完整资源二进制。
运行时 Handle。
GPU Resource id。
Bundle 内部路径。
Cooked asset path。
用户编辑器选择状态。
```

AssetRegistry v1 必须支持的查询：

```text
getByGuid(guid)
getByAssetId(assetId)
getBySourcePath(path)
getByType(type)
getByState(state)
getDependencies(guid)
getReferencers(guid)
getAffectedAssets(guid)
findMissingAssets()
findFailedAssets()
findModifiedAssets()
```

边界规则：

```text
Registry 查询结果可以用于 ProjectDock / Inspector / Console / Build Pipeline / AI Debug。
Registry 查询结果不能直接作为资源写入依据，写入必须回到 ImportTransaction。
Registry 损坏、缺失或版本不匹配时，必须从 AssetPipelineDatabase rebuild。
Registry 与 AssetPipelineDatabase 不一致时，以 AssetPipelineDatabase 为准，并生成 RegistryReport。
```

AssetRegistry v1 更新规则：

```text
RawFileEvent 不能更新 AssetRegistry。
FileChangeBatch 不能更新 AssetRegistry。
ImportWorker 不能直接更新 AssetRegistry。
只有 ImportTransaction commit 成功后，RegistryUpdater 才能更新 AssetRegistry。

正常路径使用增量更新：updateFromTransaction(transactionId)。
异常路径使用全量重建：rebuildFromDatabase()。
启动项目时，如果 Registry 缺失、schemaVersion 不匹配、builtFromDatabaseVersion 不匹配或校验失败，必须从 AssetPipelineDatabase 全量重建。
用户手动触发“重建资源索引”时，必须从 AssetPipelineDatabase 全量重建。
Registry 与 AssetPipelineDatabase 冲突时，永远以 AssetPipelineDatabase 为准。
Registry update / rebuild 必须生成 RegistryReport。
```

RegistryReport v1 最小字段：

```text
reportId
kind: incremental_update | full_rebuild | consistency_check
sourceDatabaseVersion
targetRegistryVersion
startedAt
finishedAt
summary:
  added
  updated
  removed
  missing
  conflicts
  failed
issues:
  guid
  assetId?
  sourcePath?
  issueKind
  message
  suggestedAction?
```

AI 调试规则：

```text
AI 只能读取 RegistryReport 来解释索引问题。
AI 不能直接修 Registry。
AI 如果需要修复资源问题，必须生成可审查 Patch Plan，并回到 ImportTransaction 流程。
```

## Importer 系统路线

Asset DB / Importer v1 采用完整 Importer 系统骨架。  
第一版功能可以小，但架构不能退化成裸文件登记系统。

参考引擎结论：

```text
Unity：AssetImporter 是统一基类，TextureImporter / ModelImporter / AudioImporter 等具体 Importer 承担不同资源类型的导入设置与结果生成。
UE：UFactory / AssetImportTask / ImportSubsystem 共同形成导入任务、资源创建、重导入和导入事件体系。
Bevy：AssetLoader 通过 Asset type、load、extensions 定义资源加载边界，glTF 使用 label 表达 Scene / Mesh / Texture / Material / Animation 等子资源。
```

本项目采用 C-min 路线：

```text
完整 Unity / UE 式 Importer 系统骨架
+ 每种资源类型只实现最小导入设置
+ 每次导入 / 重导入都走 ImportTransaction
+ 每次导入 / 重导入都生成 ImportReport
+ 支持 subAsset / platformSettings / cacheKey / dependency extraction 的架构入口
```

正式规则：

```text
所有资源必须通过对应 Importer 进入 Asset DB。
不允许资源只作为裸文件登记后绕过 Importer。
每个 AssetType 必须有明确 ImporterContract。
每个 Importer 必须声明支持的 source extension、asset type、import settings schema、platform settings schema、subAsset 策略、cache key 策略和 dependency extraction 策略。
Importer 可以第一版只实现最小导入设置，但不能绕过统一 ImporterContract。
AI Generated Asset 不单独成为资源类型，必须按最终资源类型进入对应 Importer。
```

第一版必须具备的系统骨架：

```text
ImporterRegistry
ImporterContract
ImportSettings
PlatformImportSettings
ImportTransaction
ImportReport
SubAsset
ImportCache
Reimport
Dependency Extraction
```

第一版资源类型与最小设置：

| 资源类型 | Importer | 第一版最小导入设置 |
|---|---|---|
| Texture | TextureImporter | usage / srgb / maxSize / compressionPreset |
| Audio | AudioImporter | usage / streaming / loopDefault / compressionPreset |
| Model / Mesh | ModelImporter | scale / importMaterials / importAnimations / coordinateSystem |
| Material | MaterialImporter | shaderModel / surfaceType / textureSlots |
| Scene | SceneImporter | schemaVersion / rootEntities / additivePolicy |
| Prefab | PrefabImporter | schemaVersion / rootEntity / exposedSlots |
| DSL / IR | LogicImporter | schemaVersion / domain / validationLevel |

第一版不做的复杂能力：

```text
完整纹理压缩矩阵
完整模型骨骼 / 动画 retarget
完整 shader 编译
完整平台 cook
复杂自动修复
完整资源预览缓存
```

这些能力后续扩展具体 Importer 实现和 settings 字段，但不改变 Importer 系统骨架。

## ImporterContract 标准结构

ImporterContract 不承载某个具体资源类型的全部导入参数。  
它只定义所有 Importer 都必须遵守的稳定契约。

具体导入参数必须进入类型化 settings：

```text
TextureImporterSettings
AudioImporterSettings
ModelImporterSettings
MaterialImporterSettings
SceneImporterSettings
PrefabImporterSettings
LogicImporterSettings
```

平台差异必须进入 PlatformImportSettings，不能塞进基础 settings。

正式规则：

```text
ImporterContract 只放所有 Importer 共用的稳定契约。
具体导入参数必须进入 typed ImportSettings。
平台差异必须进入 PlatformImportSettings。
Importer 必须声明 outputSchema、diagnosticsSchema、cachePolicy、reimportPolicy、dependencyPolicy。
Asset DB 记录 importer id / version / settingsHash / platformSettingsHash / sourceHash。
```

ImporterContract v1：

```json
{
  "schemaVersion": "importer-contract.v1",
  "id": "texture.importer",
  "version": 1,
  "displayName": "Texture Importer",
  "assetType": "Texture",
  "source": {
    "extensions": ["png", "jpg", "jpeg", "webp"],
    "mimeTypes": ["image/png", "image/jpeg", "image/webp"]
  },
  "settingsSchema": "texture-import-settings.v1",
  "platformSettingsSchema": "texture-platform-import-settings.v1",
  "outputSchema": "texture-import-result.v1",
  "subAssetPolicy": {
    "supportsSubAssets": false,
    "subAssetKinds": []
  },
  "dependencyPolicy": {
    "mode": "none|explicit|extracted",
    "recordsSourceFiles": true,
    "recordsAssetRefs": true
  },
  "cachePolicy": {
    "cacheKeyInputs": [
      "assetGuid",
      "sourceHash",
      "importerId",
      "importerVersion",
      "settingsHash",
      "platform",
      "platformSettingsHash",
      "dependencyHash",
      "outputSchema",
      "engineImportVersion"
    ]
  },
  "reimportPolicy": {
    "supportsReimport": true,
    "preserveGuid": true,
    "preserveAssetId": true,
    "allowSourceMove": true
  },
  "diagnosticsSchema": "asset-import-diagnostics.v1"
}
```

TextureImporterSettings v1 示例：

```json
{
  "schemaVersion": "texture-import-settings.v1",
  "usage": "default|sprite|normal|ui",
  "srgb": true,
  "maxSize": 2048,
  "compressionPreset": "auto"
}
```

ModelImporterSettings v1 示例：

```json
{
  "schemaVersion": "model-import-settings.v1",
  "scale": 1.0,
  "coordinateSystem": "y_up",
  "importMaterials": true,
  "importAnimations": false
}
```

原因：

```text
AI 先读取统一 ImporterContract，再读取 typed settings，不需要猜每个导入器有什么字段。
新资源类型只新增 ImporterContract 和 settings schema，不修改 Asset DB 主结构。
Texture 设置变化不会影响 Model / Audio / Scene。
公共字段统一，具体字段按类型拆开，避免出现一个难维护的巨大万能 settings。
```

## ImportResult / SubAsset 标准结构

一个源文件可以产生一个 main asset 和多个 sub asset。  
ImportResult 必须显式记录这次导入产生的主资源、子资源、依赖和诊断信息。

参考引擎结论：

```text
Unity：AssetImportContext.SetMainObject 设置主资源，AddObjectToAsset(identifier, obj) 添加子资源，底层引用接近 guid + fileID。
UE：AssetImportTask 记录 ImportedObjectPaths，AssetImportData / FAssetImportInfo 记录 source files、timestamp 和 hash。
Bevy：AssetLoader 可以产出 root asset 和 labeled assets，glTF 使用 Scene / Mesh / Texture / Material / Animation 等 label 表达子资源。
Godot：导入资源生成 .import 和内部缓存，但大型外部模型的子资源身份不如 Unity / UE / Bevy 明确。
```

本项目采用：

```text
ImportResult = 一次导入的结构化产物
ImportedAsset = 主资源
SubAsset = 主资源内部的稳定子资源
```

正式规则：

```text
每次 Importer 必须输出一个 ImportResult。
ImportResult 必须有一个 mainAsset。
ImportResult 可以有 0..N 个 subAssets。
SubAsset 默认不拥有独立 guid。
SubAsset 长期身份由 parent guid + subAsset.id + subAsset.kind 决定。
AssetRef 引用子资源时必须使用 subAsset 对象，而不是字符串路径。
Importer 负责稳定生成 subAsset.id。
```

ImportResult v1：

```json
{
  "schemaVersion": "import-result.v1",
  "importerId": "model.importer",
  "importerVersion": 1,
  "sourcePath": "Assets/Models/enemy.glb",
  "sourceHash": "sha256...",
  "settingsHash": "sha256...",
  "platformSettingsHash": "sha256...",
  "mainAsset": {
    "assetId": "enemy_model",
    "guid": "2a1f...",
    "type": "Model",
    "displayName": "Enemy Model"
  },
  "subAssets": [
    {
      "id": "mesh_body",
      "kind": "Mesh",
      "displayName": "Body",
      "sourceLocator": "gltf.meshes[0]",
      "outputSchema": "mesh-subasset.v1"
    },
    {
      "id": "material_body",
      "kind": "Material",
      "displayName": "Body Material",
      "sourceLocator": "gltf.materials[0]",
      "outputSchema": "material-subasset.v1"
    },
    {
      "id": "animation_idle",
      "kind": "Animation",
      "displayName": "Idle",
      "sourceLocator": "gltf.animations[0]",
      "outputSchema": "animation-subasset.v1"
    }
  ],
  "dependencies": [
    {
      "kind": "sourceFile",
      "path": "Assets/Textures/enemy_albedo.png",
      "hash": "sha256..."
    }
  ],
  "diagnostics": []
}
```

SubAsset id 生成规则：

```text
subAsset.id 必须由 Importer 稳定生成。
优先使用源文件里的稳定名称 / 语义路径。
没有稳定名称时，使用 kind + sourceLocator 派生。
不能使用随机 id。
不能只使用 displayName。
不能只依赖数组下标作为唯一语义依据。
```

示例：

```text
gltf.meshes[0].name = Body
-> subAsset.id = mesh_body
-> sourceLocator = gltf.meshes[0]

gltf.animations[0].name = Idle
-> subAsset.id = animation_idle
-> sourceLocator = gltf.animations[0]

没有 name
-> subAsset.id = mesh_0
-> sourceLocator = gltf.meshes[0]
```

子资源独立化规则：

```text
SubAsset 第一版不默认生成独立 guid。
第一版不提供“提取 SubAsset 为独立资源”的能力。
SubAsset 不允许脱离 parent asset 独立进入 Asset DB。
AssetRef 引用 SubAsset 时，只能通过 parent guid + subAsset.id + subAsset.kind。
项目资源身份体系保持单一路线，避免同时存在 parent-subAsset 和 independent-subAsset 两套生命周期。
```

原因：

```text
Unity 采用一个源 asset guid 下用 fileID 区分子资源。
Bevy 采用 asset path + label。
如果每个子资源都默认生成独立 guid，模型重导入时 guid 维护会显著复杂化。
如果再提供“提取为独立资源”，第一版会额外引入引用迁移、guid 生命周期、回滚和重导入 reconcile 规则。
parent guid + stable subAsset id 更适合 AI 理解和长期引用维护。
```

## Reimport 规则

Reimport 采用方案 C：保持 main asset 身份，使用 stable subAsset id / kind 做结构化 diff。

参考引擎结论：

```text
Unity：AssetImporter.SaveAndReimport 最终重新 ImportAsset；资源 guid 保持稳定，子对象依赖内部标识和 remap。
UE：UAssetImportData 记录 SourceFiles；不同资源类型通过 CanReimport / SetReimportPaths / Reimport 执行类型化重导入。
Bevy：ModifiedAsset / RenamedAsset / ModifiedMeta 等文件事件触发重新处理，并维护依赖传播。
Godot：源文件变化后根据 .import 和缓存重新导入，流程简洁但对复杂 subAsset diff 的结构化表达较弱。
```

正式规则：

```text
Reimport 不是删除旧资源再重新导入。
Reimport 必须保持 main asset 的 guid。
Reimport 默认保持 assetId，除非用户明确作为新资源重新导入。
Reimport 必须通过 ImportTransaction 提交。
Reimport 必须生成 ReimportReport。

Reimport 必须对比 old ImportResult 和 new ImportResult。
SubAsset 匹配规则只有一条：subAsset.kind + subAsset.id。
kind + id 相同 -> updated。
new 有、old 没有 -> added。
old 有、new 没有 -> removed / stale。
id 变化但 Importer 能通过 sourceLocator 证明是同一来源 -> renameCandidate。
无法证明的改名、类型变化或引用迁移 -> conflict。

引擎不自动删除引用。
引擎不自动把引用迁移到另一个 SubAsset。
只有 kind + id 完全相同的 SubAsset 更新可以自动保持引用有效。
removed / renameCandidate / conflict 必须进入 ReimportReport，由用户或 AI 明确确认后再产生后续 Patch。
```

ReimportReport v1：

```json
{
  "schemaVersion": "reimport-report.v1",
  "assetId": "enemy_model",
  "guid": "2a1f...",
  "sourcePath": "Assets/Models/enemy.glb",
  "summary": {
    "updated": 3,
    "added": 1,
    "removed": 1,
    "renameCandidates": 1,
    "conflicts": 0
  },
  "subAssetChanges": [
    {
      "kind": "updated",
      "subAsset": {
        "id": "mesh_body",
        "kind": "Mesh"
      }
    },
    {
      "kind": "removed",
      "subAsset": {
        "id": "animation_idle",
        "kind": "Animation"
      },
      "referencedBy": [
        "Assets/Prefabs/Enemy.prefab"
      ]
    }
  ],
  "requiresUserAction": true
}
```

不采用方案 D：

```text
不为每个 SubAsset 默认生成独立 guid。
不提供 SubAsset 提取为独立资源。
不维护 independent subAsset guid reconcile 表。
原因是方案 D 只让 AssetRef 看起来统一，但会把复杂度转移到 Reimport、引用迁移、删除恢复和冲突处理里。
第一版必须保持一个主身份模型：main asset guid + stable subAsset id / kind。
```

## Artifact Cache / Import Artifact Cache 规则

ImportCache 采用方案 D 的长期架构方向，但第一版只实现 D-min。

它不再被定义为一个简单的本地缓存表，而是定义为本项目的 Import Artifact Cache：

```text
长期路线：类似 Unity Artifact 系统 + UE Derived Data Cache。
第一版实现：project-local Artifact Cache，只做本地文件缓存和最小命中 / 失效判断。
```

参考引擎结论：

```text
Unity：AssetDatabase v2 使用 ArtifactKey / ImportResultID / ArtifactInfo 管理导入产物，并提供 Import Activity Window 查看导入活动。
UE：Derived Data Cache 使用派生数据 key 缓存可重建产物，key 通常包含格式版本、源数据、平台和设置。
Bevy：processed asset 记录 hash / full_hash / process_dependencies，依赖变化会让 processed result 失效。
Godot：.import 和 imported cache 支持简单导入缓存，但复杂诊断能力弱于 Unity / UE。
```

正式定位：

```text
Artifact Cache 只缓存可重建导入产物。
Artifact Cache 不是项目真相源。
删除 Artifact Cache 不应该破坏项目，只会导致重新导入。
Asset DB 记录资源身份、引用关系和当前状态。
ImportResult 记录一次导入的结构化结果。
Meta 记录资源稳定身份和导入设置。
```

长期架构必须保留这些概念：

```text
ArtifactKey
ArtifactRecord
ArtifactStore
ArtifactResolver
CacheNamespace
CacheBackend
CacheReport
```

第一版 D-min 只实现：

```text
CacheNamespace = project-local
CacheBackend = LocalFileCacheBackend
ArtifactStore 写入 Library/Artifacts
ArtifactResolver 只做 artifactKey lookup
CacheReport 只报告 hit / miss / invalidated
不做远程缓存、团队共享缓存、复杂 LRU、缓存压缩、缓存统计 UI、后台预热、跨项目共享。
```

ArtifactKey v1：

```json
{
  "namespace": "project-local",
  "assetGuid": "2a1f...",
  "importerId": "model.importer",
  "importerVersion": 1,
  "platform": "pc",
  "sourceHash": "sha256...",
  "settingsHash": "sha256...",
  "platformSettingsHash": "sha256...",
  "dependencyHash": "sha256...",
  "outputSchema": "model-import-result.v1",
  "engineImportVersion": 1
}
```

ArtifactRecord v1：

```json
{
  "schemaVersion": "artifact-record.v1",
  "artifactKey": {
    "namespace": "project-local",
    "assetGuid": "2a1f...",
    "importerId": "model.importer",
    "importerVersion": 1,
    "platform": "pc",
    "sourceHash": "sha256...",
    "settingsHash": "sha256...",
    "platformSettingsHash": "sha256...",
    "dependencyHash": "sha256...",
    "outputSchema": "model-import-result.v1",
    "engineImportVersion": 1
  },
  "artifact": {
    "artifactId": "sha256...",
    "root": "Library/Artifacts/sha256...",
    "importResultPath": "Library/Artifacts/sha256.../import-result.json",
    "files": [
      {
        "kind": "processedAsset",
        "path": "mesh.bin",
        "hash": "sha256..."
      }
    ]
  },
  "status": "valid",
  "diagnostics": [],
  "createdAt": "2026-06-24T00:00:00Z"
}
```

CacheReport v1：

```json
{
  "schemaVersion": "cache-report.v1",
  "assetId": "enemy_model",
  "guid": "2a1f...",
  "artifactId": "sha256...",
  "result": "hit|miss|invalidated",
  "reason": "sourceHashChanged|settingsHashChanged|platformChanged|dependencyHashChanged|importerVersionChanged|engineImportVersionChanged|notFound",
  "changedInputs": [
    "sourceHash"
  ]
}
```

核心规则：

```text
ArtifactKey 是缓存命中的唯一依据。
ArtifactRecord 只描述缓存产物，不描述项目引用关系。
ArtifactStore 可以被清空，清空后通过 Importer 重建。
Build Pipeline 可以读取 ArtifactStore，但不能把 ArtifactStore 当作 Asset DB。
AI Debug / Console 读取 CacheReport，不直接修改 ArtifactStore。
缓存失效必须解释为 ArtifactKey 中某个输入变化，不能只返回 unknown failed。
```

## Dependency Extraction 规则

Dependency Extraction 采用方案 D 的长期架构方向，但第一版只实现 D-min。

它不被定义为某个 Importer 的局部功能，而是定义为引擎级依赖系统：

```text
Dependency Graph = Asset DB 的正式底层能力
DependencyRecord = 所有依赖的统一表达
DependencyExtractor = Importer / Schema / Build / RuntimePackage 都可使用的依赖提取入口
DependencyHash = Artifact Cache 失效判断依据
ReverseDependencyIndex = 找到谁受影响
```

参考引擎结论：

```text
Unity：AssetImportContext 提供 DependsOnSourceAsset / DependsOnImportedAsset / DependsOnArtifact / DependsOnCustomDependency，AssetDatabase 提供 GetDependencies / GetAssetDependencyHash。
UE：AssetRegistry 记录 Package / Manage / SearchableName 依赖；UAssetImportData 记录 SourceFiles；Cook 阶段使用 FCookDependency 记录构建依赖。
Bevy：processed asset 记录 process_dependencies / full_hash，并维护 dependents，依赖变化会触发重新处理。
Godot：.import 和 imported cache 能表达基础导入依赖，但复杂依赖诊断能力较弱。
```

长期架构必须保留这些概念：

```text
DependencyRecord
DependencyGraph
DependencyExtractor
DependencyHash
ReverseDependencyIndex
DependencyReport
```

第一版 D-min 只实现：

```text
结构化 DependencyRecord。
Asset DB 内部 DependencyGraph。
直接依赖的 DependencyHash。
ReverseDependencyIndex。
Importer 输出 ImportResult.dependencies。
SchemaWalker 自动提取 Scene / Prefab / Material / DSL / IR 中的 AssetRef。
DependencyReport 只报告 added / removed / changed / missing / cycle。
```

第一版不实现：

```text
远程依赖图。
分布式构建依赖。
复杂 CookDependency 函数依赖。
运行时动态依赖追踪。
依赖图高级可视化编辑器。
复杂依赖查询语言。
```

DependencyRecord v1：

```json
{
  "schemaVersion": "dependency-record.v1",
  "kind": "assetRef",
  "usage": "runtime",
  "required": true,
  "target": {
    "assetId": "enemy_albedo",
    "guid": "8c2f...",
    "type": "Texture"
  },
  "source": {
    "extractor": "ModelImporter",
    "locator": "materials[0].baseColorTexture"
  },
  "hash": "sha256..."
}
```

Dependency kind v1：

```text
sourceFile   源文件依赖，例如 glb 引用的外部 png。
assetRef     项目资源依赖，例如 Material 引用 Texture，Scene 引用 Prefab。
artifact     导入产物依赖，例如某个导入产物依赖另一个 artifact。
importRule   导入规则依赖，例如 importer version / shader compiler version / engineImportVersion。
buildRule    构建规则依赖，例如平台构建配置、bundle 策略、压缩配置。
```

Dependency usage v1：

```text
runtime  运行时需要。
import   只在导入阶段需要。
build    只在构建阶段需要。
editor   只在编辑器阶段需要。
```

核心规则：

```text
Importer 不直接写 Asset DB。
Importer 只输出 ImportResult.dependencies。
SchemaWalker 自动提取引擎结构化资源中的 AssetRef。
Asset DB 统一接收 DependencyRecord，建立 DependencyGraph 和 ReverseDependencyIndex。
只存直接依赖，递归依赖通过 DependencyGraph 查询。
Artifact Cache 的 dependencyHash 来自规范化后的直接依赖。
依赖变化后，通过 ReverseDependencyIndex 找到需要 reimport / rebuild 的资源。
DependencyReport 必须能说明依赖从哪里来、为什么变化、影响哪些资源。
```

循环依赖规则：

```text
第一版允许 Asset DB 记录循环依赖，但必须在 DependencyReport 中标记 cycle。
runtime 依赖循环默认 warning。
import / build 依赖循环默认 error，因为可能导致导入或构建顺序无法确定。
引擎不自动打断循环依赖。
AI 可以基于 DependencyReport 生成修复建议，但不能自动修改引用。
```

## 当前实现

新增：

```text
src/engine/assetDatabase.ts
schemas/asset-meta.schema.json
src/services/assetImportLockService.ts
scripts/asset-db.cjs
scripts/asset-file-scanner.cjs
scripts/test-asset-db.cjs
src/asset-generation/assetImportRepairPlan.ts
scripts/test-asset-import-repair-plan.cjs
```

新增命令：

```powershell
npm.cmd run test:assetdb
npm.cmd run test:assetscan
npm.cmd run test:assetlock
npm.cmd run test:assetimportrepair
```

## 数据结构

AssetMeta：

```text
schemaVersion
guid
assetId
name
type
importer
importerSettings
displayName
tags
ai generated / allowReplace / protected
```

AssetDatabase：

```text
schemaVersion
projectName
assets
importLock
```

AssetDatabase 中的单项记录由导入生成，包含：

```text
assetId
guid
assetType
sourcePath
metaPath
importer
state
sourceHash
metaHash
dependencies
referencedBy
diagnostics
```

Asset state：

```text
current   = 外部文件与记录一致
modified  = 外部文件存在，但 hash / size / mtime 已变化
missing   = 外部文件不存在
failed    = importer 失败
ignored   = 被导入规则忽略
conflict  = assetId / guid / meta 冲突
```

## 已支持能力

当前支持：

```text
从 GameProject 创建 AssetDatabase
为每个 Asset 生成稳定 GUID
根据 Asset.type 推断 importer
根据 Scene / Prefab 的 Entity 引用生成依赖图
根据外部文件快照同步 current / modified / missing 状态
扫描 project.assets.source 指向的真实文件
刷新项目资产后补齐新增 AssetMeta
校验重复 GUID、缺失 meta、悬空 dependency
导入锁 begin / end
编辑器项目修改入口可以被导入锁阻止
导入任务有 task id / type / stage / progress / status / errors / history
failed AssetImportTask 可以生成 AssetImportDiagnostic
AssetImportDiagnostic 可以生成 report-only AI Repair Candidate
```

## 当前边界

暂不做：

```text
编辑器 Project 面板 UI 接入
Bundle 分包
热更 mount
资源预览缓存
真实纹理压缩矩阵
真实音频转码矩阵
完整模型骨骼 / 动画 retarget
完整 shader 编译
完整平台 cook
真实 GPU 上传
```

原因：

```text
第一版目标是固定完整 Importer 系统骨架。
具体资源类型只做最小导入设置和最小数据产物，避免第一版被完整美术管线拖死。
真实扫描当前只在脚本层落地。
编辑器已有最小导入锁，但还没有接入完整异步导入任务队列。
复杂平台 cook、预览缓存和高阶资源转换由后续阶段扩展，不改变 ImporterContract。
```

## 编辑器导入任务规则

当前编辑器已经接入最小导入任务队列：

```text
import JSON / GLTF 开始时创建 AssetImportTask
任务记录 id / type / label / stage / progress / status / startedAt
导入自身提交 Project 时允许 bypass
普通 commitProject / updateEntityLive / undo / redo 会被锁阻止
导入成功时任务进入 completed history
导入失败时任务进入 failed history，并记录 errors
工具栏显示当前 task stage / progress
Console 面板显示最近 Asset Tasks
AI 面板显示最近 failed Asset Task，作为 AI Debug View 的最小入口
```

正式规则：

```text
小型单资源导入可以保留局部任务状态。
大批量资源导入 / 外部文件同步期间，编辑器必须进入全局导入占用态。
全局导入占用态期间，用户不能查看、选择、修改 Project / Asset 面板内容。
全局导入占用态期间，AI 不能读取半成品资源状态，AI Patch 不能应用。
全局导入占用态只允许显示导入进度、当前阶段和完成后的 ImportReport / RegistryReport / DependencyReport 入口。
导入完成后，编辑器统一刷新 ProjectDock / Inspector / Console / RuntimeTrace 中的资源状态。
导入任务自身可以通过受控 bypass 写入导入结果。
任务历史保留最近 50 条，供后续 Console / AI Debug View 使用。
Console / AI Debug View 只展示任务事实，不直接自动修复资源。
failed AssetImportTask 的修复候选是 report-only 数据，不自动重新导入、不自动删除 meta、不自动移动文件、不自动替换资源引用。
```

原因：

```text
大项目中 SVN / 外部工具可能一次同步大量资源。
如果同步期间允许用户或 AI 同时改项目，Asset DB / Project Model / Undo Stack 很容易出现不可解释的不一致。
如果同步期间还允许查看和选择资源，用户和 AI 会读到半成品状态，后续 Bug 难以复现。
导入锁是工程流程规则，不是具体玩法功能。
```

## 测试覆盖

当前测试覆盖：

```text
import image/model-like asset creates meta
external source unchanged keeps current
external source changed becomes modified
external source missing becomes missing
duplicate GUID fails
asset dependency graph can be generated
new project asset can refresh into database
import lock blocks concurrent import
asset scanner detects current / missing / generated / folder / outside-root sources
editor asset import lock blocks normal mutation and allows controlled import bypass
asset import task records stage / progress / completed history / failed history
asset import tasks are visible in Console / AI Debug View
failed import task creates asset-import-diagnostic.v1
failed import task creates asset-import-repair-plan.v1
repair candidates are reportOnly / requiresUserApproval / canAutoApply=false
running or errorless task cannot create valid repair candidates
```

## ProjectDock / Console UI 接入规则

问题：

```text
Asset Pipeline v1 已经有 AssetPipelineDatabase / AssetRegistry / ImportReport / BatchImportReport / RegistryReport。
但编辑器 ProjectDock 仍主要读取 project.assets，Console 主要显示 AssetImportTask。
这会导致资源系统的真数据已经存在，但用户和 AI 在 UI 中看不到完整的资源状态、导入结果、依赖问题和 Registry 一致性问题。
```

其他引擎参考：

```text
Unity：
ProjectBrowser 通过 AssetDatabase / AssetImporter / SearchFilter 展示资源。
ConsoleWindow 展示导入、脚本、运行时错误。
Project 面板保持简洁，导入错误主要进入 Console / Import Activity / Inspector。

Unreal Engine：
Content Browser 通过 ContentBrowserDataSubsystem / AssetRegistry 查询资源。
MessageLog / OutputLog 承载导入、验证、构建问题。
Content Browser 不把底层文件扫描结果直接当 UI 真相，而是读 Registry / DataSource。

Bevy：
AssetServer / AssetEvent / LoadState 管理资源状态。
没有强编辑器 UI，但资源状态事件和加载状态边界清晰。

Godot：
FileSystem Dock 展示资源文件。
Import Dock / Output 展示导入设置与导入问题。
资源浏览、导入配置、错误输出分离。
```

我们的正式规则：

```text
ProjectDock / Console UI 采用“UE 数据源 + Unity 简洁交互”的路线。

Asset Pipeline 真相：
AssetPipelineDatabase / AssetRegistry / ImportReport / BatchImportReport / RegistryReport。

ProjectDock 真相来源：
ProjectDock 不再把 project.assets 当资源浏览真相。
ProjectDock 应读取 AssetRegistry 派生出的 EditorAssetViewModel。

Console 真相来源：
ConsoleDock 不只显示 AssetImportTask。
ConsoleDock 应显示 ImportReport / BatchImportReport / RegistryReport 的结构化摘要。

ProjectDock 职责：
展示资源。
展示文件夹 / 类型 / 搜索 / 状态过滤。
展示资源状态角标：current / modified / missing / failed / blocked / generated。
支持选中资源。
支持从 Console 错误跳转到对应资源。
不直接负责解释导入错误。
不直接负责自动修复资源。

ConsoleDock 职责：
展示导入任务进度。
展示导入报告。
展示批量导入报告。
展示 Registry 一致性问题。
展示缺失依赖、重复 GUID、导入失败、blocked asset。
提供 AI 修复候选入口，但修复候选必须是 report-only / requiresUserApproval。
Console 不自动重导入、不自动删除 meta、不自动移动文件、不自动替换引用。

Asset Editor / Inspector D-min 职责：
展示选中资源的 identity、importer 摘要、registry entry、dependencies、referencers、last import report。
第一版不做完整资源预览器、不做真实 importer setting 修改、不做 Apply / Revert、不做多资源同时编辑、不做真实 reimport 执行。
Asset Editor 不持有 AssetPipelineState，不直接写 Asset DB，不直接改 AssetRegistry。
Asset Editor 只读取 AssetPipelineSnapshot / AssetInspectorViewModel。
所有高风险动作先生成 Request / Plan，再由 Controller / Patch / ImportTransaction 执行。

AI 读取规则：
AI 默认读取 AssetRegistry / ImportReport / BatchImportReport / RegistryReport。
AI 不读取 ProjectDock 内部 UI 状态作为资源真相。
AI Patch 不能基于半成品导入状态生成。

大批量导入规则：
全局导入占用态期间，ProjectDock / Inspector / Console 只显示进度和锁定状态。
全局导入占用态期间，用户不能查看、选择、修改 Project / Asset 面板内容。
导入完成后，统一刷新 ProjectDock / Inspector / Console / RuntimeTrace 中的资源状态。
```

推荐数据流：

```text
Importer / ImportWorker
  -> AssetPipelineDatabase
  -> AssetRegistry
  -> EditorAssetViewModel
  -> ProjectDock

Importer / ImportWorker
  -> ImportReport / BatchImportReport / RegistryReport
  -> EditorAssetIssueViewModel
  -> ConsoleDock / AI Debug View
```

第一版边界：

```text
做：
EditorAssetViewModel。
ProjectDock 读取 AssetRegistry 派生数据。
ConsoleDock 读取 ImportReport / BatchImportReport / RegistryReport 摘要。
Console issue 点击跳转 ProjectDock asset。
最小测试覆盖 view model、状态过滤、报告摘要、跳转目标。

不做：
真实 OS file watcher。
真实 GPU 上传。
Bundle 二进制打包。
平台 cook。
完整 Inspector importer setting UI。
真实缩略图缓存。
资源自动修复执行。
```

## Asset Editor / Asset Inspector D-min 规则

问题：

```text
ProjectDock / Console 已经能看到资源列表、导入状态和错误报告。
但用户和 AI 还需要在选中单个资源时，快速看到这个资源为什么存在、由哪个 Importer 生成、依赖谁、被谁引用、最近一次导入出了什么问题。
如果第一版直接做完整 Unity / UE 式 Asset Editor，会把资源预览、Importer 参数编辑、Apply/Revert、重导入执行、专用材质/模型/动画编辑器一次性混进来，规则过重。
如果完全不做 Asset Inspector，AI 和用户又很难定位单个资源的问题。
```

其他引擎参考：

```text
Unity：
ProjectBrowser 负责资源浏览。
Inspector 显示选中 Asset / AssetImporter 的属性。
AssetImporter / AssetDatabase 负责导入和重导入真相。
用户在 Inspector 中可以修改导入设置并 Apply / Revert，但资源数据库不是 Inspector 自己维护。

Unreal Engine：
Content Browser 负责资源浏览。
Details Panel / Asset Editor 显示 UObject / Asset 的属性。
AssetRegistry / ContentBrowserDataSubsystem 负责资源索引和查询。
具体资源类型可以有专用 Asset Editor，但底层资源真相不由 Details Panel 持有。

Godot：
FileSystem Dock 负责文件和资源浏览。
Import Dock 显示导入设置并触发 Reimport。
设计更轻，但复杂依赖、反向引用和 AI 可解释报告弱于 Unity / UE。
```

推荐方案：

```text
长期方向采用 Unity / UE 式 Asset Editor。
第一版只做 D-min：Asset Details + Importer 摘要 + Dependency / Referencer / Report 面板。
```

D-min 结构：

```text
AssetEditorShell
AssetInspectorViewModel
AssetIdentityPanel
AssetImporterPanel
AssetDependencyPanel
AssetReferencerPanel
AssetReportPanel
AssetActionPanel
```

D-min 展示字段：

```text
Identity:
  assetId
  guid
  type
  sourcePath
  state

Importer:
  importerId
  importerVersion
  sourceHash
  settingsHash
  artifactId

Dependencies:
  directDependencies
  missingDependencies

Referencers:
  directReferencers

Reports:
  last ImportReport
  related RegistryReport issues
  related BatchReport issues

Actions:
  Request Reimport
  Create AI Repair Candidate
  Locate in ProjectDock
```

Asset Action 生效规则：

```text
Asset Editor 可以直接触发 Editor Command。
Editor Command 由 AssetPipelineController / PatchService 判断是否允许执行。

低风险动作直接执行：
  Locate in ProjectDock
  Copy assetId / guid / sourcePath
  Open related report
  Switch Asset Inspector tab

高风险动作必须进入 Request / Plan / Transaction：
  Reimport
  Importer setting 修改
  Delete asset
  Move / Rename asset
  Replace AssetRef
  AI Repair
  Batch operation
```

Asset Inspector 接入规则：

```text
ProjectDock 只负责选择 assetId / assetGuid，不把选中资源详情复制成自己的真相。
EditorSelectionState 记录当前 Inspector target，可以是 Entity / Project / Scene / Asset。
Asset Inspector target = Asset 时，必须通过 AssetPipelineSnapshot 派生 AssetInspectorViewModel。
AssetInspectorViewModel 是 Asset Inspector 的唯一展示输入。
Asset Inspector 不直接读取 AssetPipelineState，不直接读取 ImportWorker 内部状态，不直接扫描文件系统。
```

Asset Inspector 数据流：

```text
AssetPipelineState
  -> immutable AssetPipelineSnapshot
  -> createAssetInspectorViewModel(snapshot, selectedAssetId)
  -> Inspector render

ProjectDock
  -> onSelectAsset(assetId)
  -> EditorSelectionState
  -> Inspector target = asset
```

AssetInspectorViewModel v1：

```text
AssetInspectorViewModel
  schemaVersion
  selectedAssetId
  identity
  importer
  dependencies
  referencers
  reports
  actions
  emptyReason?

identity:
  assetId
  guid
  type
  sourcePath
  displayName
  state

importer:
  importerId
  importerVersion
  sourceHash
  settingsHash
  artifactId

dependencies:
  directDependencies
  missingDependencies

referencers:
  directReferencers

reports:
  lastImportReport
  registryIssues
  batchIssues

actions:
  locateInProjectDock
  copyGuid
  openReport
  requestReimport
  createAiRepairCandidate
```

第一版只读规则：

```text
Asset Inspector 第一版显示 Importer 摘要，不提供真实 Importer setting 编辑。
Importer setting 修改后续必须走 ImporterSettingPatchRequest / AssetActionPlan / ImportTransaction。
Asset Inspector 第一版可以展示 action 按钮和生成 request，但不执行真实 reimport / repair。
导入占用态期间，Asset Inspector 只显示 busy / progress / locked message，不读取半成品 registry。
```

Request / Plan / Transaction 定义：

```text
AssetActionRequest：
表示用户或 AI 想做什么。
它记录 intent、target asset、trigger source、reason、requirementId / aiTaskId。

AssetActionPlan：
表示系统准备怎么做。
它记录 planned steps、affected assets、affected files、dependency impact、validation result、需要用户确认的风险。

Transaction：
表示真正执行。
资源导入 / 重导入走 ImportTransaction。
项目数据 / 引用替换走 Patch Transaction。
编辑器命令证据走 CommandTransaction。
Transaction 必须能生成 diagnostics / report / state change summary。
```

第一版不做：

```text
完整资源预览器。
真实 Importer setting 修改。
Apply / Revert。
多资源同时编辑。
真实 reimport 执行。
复杂依赖图可视化。
资源自动修复。
独立 Asset Editor 窗口。
材质 / 模型 / 动画专用编辑器。
```

边界规则：

```text
Asset Editor 不持有 AssetPipelineState。
Asset Editor 只读 AssetPipelineSnapshot / AssetInspectorViewModel。
Asset Editor 不直接写 Asset DB。
Asset Editor 不直接改 AssetRegistry。

Asset Editor 不把所有按钮都变成重流程。
低风险查看类动作可以直接执行。

所有高风险动作先生成 Request / Plan：
  ReimportRequest
  ImporterSettingPatchRequest
  AssetRepairCandidate
  DeleteAssetRequest
  MoveAssetRequest
  RenameAssetRequest
  ReplaceAssetRefRequest

后续执行必须走 Controller / Patch / ImportTransaction。
Request / Plan / Transaction 这一层只服务高风险资源动作，不允许扩散成所有 UI 操作的强制规则。
Asset Editor 永远不直接绕过 Controller 提交资源写入。
```

为什么适合我们：

```text
AI 友好：
单资源问题有稳定 ViewModel，AI 不需要从 UI 状态里猜资源真相。

复杂项目友好：
dependencies / referencers / reports 第一版就进入结构，不等项目变大后再补。

可维护：
Asset Editor 是观察和发起请求的界面，不成为第二套 Asset DB。

简单：
第一版只读和生成请求，不做完整资源编辑器，避免规则一下子膨胀。
```

## Asset Pipeline State 持有规则

问题：

```text
AssetRegistry / ImportReport / BatchImportReport / RegistryReport 不能长期由 ProjectDock、ConsoleDock、Inspector 或 React App 各自持有。
否则后期会出现多套资源状态：ProjectDock 一套、Console 一套、Inspector 一套、AI 一套、Native Editor Host 一套。
这会导致资源真相分裂，AI 难以判断哪个状态可信，用户也难以定位导入和引用问题。
```

其他引擎参考：

```text
Unity：
AssetDatabase 是资源查询入口。
ProjectBrowser 读取 AssetDatabase。
ConsoleWindow 读取 LogEntries。
ProjectBrowser / ConsoleWindow 不自己维护资源数据库。

Unreal Engine：
IAssetRegistry 持有资源 registry 真相。
ContentBrowserDataSubsystem 作为编辑器资源数据源。
ContentBrowser / SAssetView 订阅数据源事件并显示。
MessageLog / OutputLog 展示问题，不持有资源数据库。

Bevy：
AssetServer / Assets<T> 持有资源与加载状态。
AssetEvent / AssetEventSystems 通知资源变化。
系统和 UI 通过事件或查询读取，不各自维护资源真相。

Godot：
EditorFileSystem / ResourceLoader / Import 系统持有资源和导入状态。
FileSystem Dock / Import Dock / Output 只是不同视图。
```

正式规则：

```text
采用方案 B：
Editor Core / Native Editor Host 持有真实 Asset Pipeline State。
React / Electron App 只作为 legacy transition shell 读取 Snapshot / ViewModel。

Asset Pipeline State 是编辑器资源系统唯一运行时真相。
ProjectDock 不持有 AssetRegistry。
ConsoleDock 不持有 reports。
Inspector 不持有 importer setting / dependency 真相。
AI 不读取 UI 内部状态作为资源真相。
```

AssetPipelineState v1 采用“中心 State + immutable Snapshot + 轻量 Event”的路线。
它不是完整 Event Sourcing，也不是每个 UI 面板各自保存一份资源状态。

v1 结构：

```text
AssetPipelineState
  schemaVersion
  stateVersion
  status: ready | importing | failed | rebuilding
  database: AssetPipelineDatabase
  registry: AssetRegistry
  reports:
    importReports: ImportReport[]
    batchReports: BatchImportReport[]
    registryReports: RegistryReport[]
    maxReports: 50
  activeImport:
    busy: boolean
    taskId?
    stage?
    progress?
    startedAt?
    readable: false
  lastCompletedImport?
  updatedAt

AssetPipelineController
  getSnapshot()
  subscribe()
  beginImport()
  applyImportResult()
  applyFileChangeBatch()
  rebuildRegistry()
  clearReports()
```

数据流：

```text
Importer / ImportWorker
  -> AssetPipelineController
  -> AssetPipelineState
  -> immutable AssetPipelineSnapshot
  -> EditorAssetPipelinePanelViewModel
  -> ProjectDock / ConsoleDock / Inspector / AI Debug View
```

AssetPipelineSnapshot v1：

```text
AssetPipelineSnapshot
  stateVersion
  databaseVersion
  registryVersion
  readable
  status
  summary
  registry
  recentReports
  activeImportPublic
```

Snapshot 规则：

```text
Snapshot 是 UI / AI / Console / ProjectDock 的只读输入。
Snapshot 只在资源状态变化、导入完成、registry rebuild 或 report 变化时生成。
Snapshot 不是每帧生成，不保存完整历史。
snapshot.stateVersion 必须递增。
active import 期间只允许读取 activeImportPublic / progress / busy。
active import 期间不允许 UI / AI 读取半成品 registry。
legacy React App 可以临时读取 snapshot，但不能成为长期状态所有者。
```

AssetPipelineEvent v1：

```text
AssetPipelineEvent
  stateVersion
  kind:
    import_started
    import_progress
    import_completed
    import_failed
    registry_rebuilt
    reports_trimmed
  changedAssetGuids
  reportIds
  timestamp
```

Event 规则：

```text
Event 只做通知，不是资源真相。
UI 可以用 Event 决定刷新哪个 ViewModel。
AI 可以读取 Event 辅助解释最近发生了什么。
Event 不用于重建完整 AssetPipelineState。
第一版不做完整 Event Sourcing。
```

生命周期：

```text
Idle
  -> Importing(readable=false)
  -> Commit AssetPipelineDatabase / AssetRegistry / reports
  -> stateVersion++
  -> Generate immutable AssetPipelineSnapshot
  -> Emit AssetPipelineEvent
  -> UI / AI refresh from Snapshot
```

持久化规则：

```text
AssetPipelineDatabase 持久化到项目本地 Library / Cache。
AssetRegistry 可以持久化为可重建缓存，但必须能从 AssetPipelineDatabase 重建。
ImportReport / BatchImportReport / RegistryReport 持久化最近 50 条，放入 Library/Reports/AssetPipeline。
Project 源文件不保存 volatile import reports。
Project 源文件只保存语义项目数据和稳定 AssetRef。
Artifact Cache / Registry Cache / Reports 都是派生数据，可删除后重建或重新生成。
```

核心禁止：

```text
ProjectDock / ConsoleDock / Inspector / AI 不允许直接写 AssetPipelineState。
ProjectDock / ConsoleDock / Inspector / AI 不允许持有自己的 AssetRegistry / reports 真相。
Event 不允许被当作数据库。
Snapshot 不允许被反向写回 AssetPipelineState。
导入过程中的半成品 registry 不允许暴露给 UI / AI。
```

## 下一步

当前施工状态：

```text
Asset Pipeline v1 数据层 / 最小导入闭环已完成。
Asset Pipeline v1 接入编辑器 / 文件同步 / Build Pipeline 已完成。
Asset Pipeline v1 Importer / Registry 闭环已完成。
Asset Pipeline ProjectDock / Console UI 接入已完成。
```

剩余边界：

```text
真实 OS file watcher 尚未实现。
Editor Core / Native Editor Host 真实持有 AssetPipelineState 尚未实现。
Inspector 深度接入 Asset Pipeline 尚未实现。
真实 GPU 上传 / Bundle 二进制打包 / 平台 cook 尚未实现。
AssetPipelineState v1 已完成，下一步如果继续 Asset Pipeline，应优先讨论 Inspector 接入 importer setting / dependencies / referencers，或 Native Editor Host 接入 AssetPipelineState Snapshot。
```
