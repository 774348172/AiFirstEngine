# 当前可自动化施工文档：Asset Pipeline v1 Importer / Registry 闭环

本文档定义下一轮可以直接自动化实施的 Asset Pipeline v1 施工任务。  
目标不是做完整美术资产处理能力，而是把已经确认的正式资源管线骨架落到代码和测试里。

## 依据文档

```text
25-Asset-DB-Importer-MVP.md
36-文档体系与维护规则.md
54-当前可自动化施工总入口.md
```

## 施工目标

本轮施工目标：

```text
建立 AssetImportUnit 作为资源导入最小执行 / 提交 / 失败 / 阻塞单位。
建立 BatchImportReport 作为批量导入汇总报告。
建立真实 ImportBatchPlan -> ImportQueue -> ImportWorker -> ImportTransaction 最小闭环。
建立 AssetRegistry update / rebuild 最小闭环。
建立 Texture / Audio / Model / Material / Scene / Prefab / Logic-DSLIR 最小 Importer。
建立全局导入占用态的状态模型和测试。
```

本轮不追求：

```text
完整纹理压缩。
完整音频转码。
完整模型骨骼 / 动画 retarget。
真实 shader 编译。
真实 GPU 上传。
真实 Bundle 二进制打包。
完整平台 cook。
多线程 ImportWorker Pool。
复杂 ImportJobGraph。
自动修复资源引用。
```

## 市场方案对照

```text
Unity：AssetDatabase / AssetImportContext / StartAssetEditing / StopAssetEditing。
UE：UAssetImportTask / UFactory / AssetRegistry / Reimport / Cook 依赖。
Bevy：AssetSourceEvent / AssetProcessor / LoadState / DependencyLoadState。
```

本轮取舍：

```text
学习 Unity 的稳定 GUID / meta / importer / artifact。
学习 UE 的任务输入和 AssetRegistry 查询边界。
学习 Bevy 的事件分层和依赖失败状态。
不复制 Unity 黑盒 AssetDatabase。
不复制 UE Package / UObject 复杂度。
不把 Bevy runtime Handle 写入项目资源数据。
```

## A1：AssetImportUnit 数据结构

新增或补齐：

```text
AssetImportUnit
AssetImportUnitState
AssetImportAction
AssetImportChangeKind
```

字段：

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

规则：

```text
一个 AssetImportUnit 对应一个 main asset。
SubAsset 不单独生成 AssetImportUnit。
AssetImportUnit 不保存完整 ImportResult / ArtifactRecord / ImportReport / 二进制 / runtime handle / GPU id。
```

最小测试：

```text
创建 added Texture unit。
创建 moved unit，必须包含 previousSourcePath。
创建 failed unit，必须包含 failureReason。
创建 blocked unit，必须包含 blockReason。
验证 unit 不包含禁止字段。
```

## A2：BatchImportReport 数据结构

新增或补齐：

```text
BatchImportReport
BatchImportSummary
BatchImportIssue
```

最小字段：

```text
reportId
batchId
transactionId
startedAt
finishedAt
summary:
  added
  modified
  deleted
  moved
  success
  failed
  blocked
  warnings
committedUnits
failedUnits
blockedUnits
issues
```

规则：

```text
批量导入不是整批原子回滚。
成功 AssetImportUnit 提交。
失败 AssetImportUnit 进入 failed report。
核心依赖失败时，下游 AssetImportUnit 标记 blocked / dependency_failed。
BatchImportReport 只解释结果，不自动修复项目。
```

最小测试：

```text
一批 3 个 unit：2 成功 1 失败，BatchImportReport summary 正确。
核心依赖失败时，下游 unit blocked，blockedUnits 正确。
失败 unit 不影响成功 unit 的提交结果。
```

## A3：ImportBatchPlan / ImportQueue / ImportWorker 最小闭环

新增或补齐：

```text
ImportBatchPlan
ImportQueue
ImportWorker
ImportTransaction per AssetImportUnit commit
```

流程：

```text
FileChangeBatch
  -> ImportBatchPlan
  -> AssetImportUnit[]
  -> ImportQueue
  -> ImportWorker
  -> ImportTransaction
  -> AssetPipelineDatabase
  -> BatchImportReport
```

规则：

```text
第一版 ImportQueue 是单队列。
第一版 ImportWorker 是单 worker 串行执行。
ImportWorker 不直接写 AssetPipelineDatabase。
ImportWorker 只能通过 ImportTransaction 按 AssetImportUnit 提交。
ImportWorker 失败只生成 report，不自动修复、不自动删除、不自动迁移引用。
```

最小测试：

```text
FileChangeBatch added -> ImportBatchPlan 生成 import unit。
FileChangeBatch modified -> ImportBatchPlan 生成 reimport unit。
FileChangeBatch deleted -> ImportBatchPlan 生成 mark_missing unit。
ImportWorker 执行成功 unit 后 AssetPipelineDatabase 更新。
ImportWorker 执行失败 unit 后 AssetPipelineDatabase 保留旧状态。
```

## A4：ImporterContract v1 与最小 Importer

需要覆盖资源类型：

```text
Texture
Audio
Model
Material
Scene
Prefab
Logic / DSL-IR
```

每类 Importer 第一版只做：

```text
识别文件。
生成 meta。
生成 AssetImportUnit。
输出 ImportResult。
提取直接依赖。
写 ArtifactRecord。
进入 AssetRegistry。
生成失败报告。
```

禁止范围：

```text
Texture 不做真实压缩矩阵。
Audio 不做真实转码矩阵。
Model 不做骨骼 / 动画 retarget。
Material 不做真实 shader 编译。
Scene / Prefab 不做运行时实例化。
Logic / DSL-IR 不做真实 IR -> Rust AOT 编译。
```

最小测试：

```text
Texture 最小导入成功。
Audio 最小导入成功。
Model 最小导入成功，至少能记录 source artifact。
Material 最小导入成功，并能提取 Texture AssetRef 直接依赖。
Scene 最小导入成功，并能提取 Prefab / AssetRef 直接依赖。
Prefab 最小导入成功，并能提取 child asset ref。
Logic / DSL-IR 最小导入成功，并记录 importerVersion / sourceHash。
每类 Importer 至少有一个失败测试。
```

## A5：AssetRegistry update / rebuild 闭环

新增或补齐：

```text
AssetRegistry
AssetRegistryEntry
RegistryUpdater
RegistryReport
```

规则：

```text
AssetPipelineDatabase 是资源唯一真相。
AssetRegistry 是派生轻量索引。
AssetRegistry 不允许被 UI / AI / Importer 直接写入。
只有 ImportTransaction commit 成功后，RegistryUpdater 才能 updateFromTransaction(transactionId)。
Registry 缺失、版本不匹配、校验失败时，rebuildFromDatabase()。
Registry 与 AssetPipelineDatabase 冲突时，以 AssetPipelineDatabase 为准。
```

必须支持查询：

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

最小测试：

```text
成功 import 后 registry 可通过 guid / assetId / sourcePath 查询。
reimport 后 registry 更新 sourceHash / artifactId / lastImportReportId。
delete / missing 后 registry state 正确。
rebuildFromDatabase 后 registry 查询结果与增量更新结果一致。
Registry 与 DB 版本不匹配时生成 RegistryReport。
```

## A6：全局导入占用态

新增或补齐：

```text
AssetImportLock
GlobalImportBusyState
ImportProgressState
```

规则：

```text
大批量资源导入 / 外部文件同步期间，编辑器进入全局导入占用态。
占用态期间 Project / Asset 面板不可读、不可写、不可交互。
占用态期间 AI 不能读取半成品资源状态，AI Patch 不能应用。
占用态只允许显示导入进度、当前阶段和完成后的报告入口。
导入完成后统一刷新 ProjectDock / Inspector / Console / RuntimeTrace 中的资源状态。
```

最小测试：

```text
进入 GlobalImportBusyState 后普通 project mutation 被拒绝。
进入 GlobalImportBusyState 后 AI patch apply 被拒绝。
进入 GlobalImportBusyState 后 Project / Asset read query 返回 busy 状态，而不是半成品数据。
导入完成后 busy 解除，Project / Asset read query 返回刷新后的 Registry 结果。
```

## A7：端到端 Golden Scenario

场景一：小批量成功导入。

```text
导入 Texture + Material + Prefab。
Material 依赖 Texture。
Prefab 依赖 Material。
全部成功。
AssetPipelineDatabase / AssetRegistry / DependencyGraph / BatchImportReport 一致。
```

场景二：依赖失败。

```text
Texture 导入失败。
Material 标记 blocked / dependency_failed。
Prefab 标记 blocked / dependency_failed。
BatchImportReport 能解释失败链路。
```

场景三：外部文件移动。

```text
Texture sourcePath moved。
meta guid 保持。
AssetImportUnit 包含 previousSourcePath。
Registry 更新 sourcePath。
AssetRef 不变化。
```

## 验收门禁

本轮完成必须满足：

```text
每个 A 项都有最小单元测试。
至少 3 个 Golden Scenario 通过。
AssetImportUnit / BatchImportReport / AssetRegistry / ImportWorker 都有结构化测试。
旧的 Asset Pipeline 测试不回归。
文档中没有把 AssetRegistry 写成资源真相。
文档中没有把批量导入写成整批原子回滚。
```

## 完成后必须更新

```text
25-Asset-DB-Importer-MVP.md
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
施工文档/README.md
阶段完成记录/README.md
新增 阶段完成记录/YYYY-MM-DD-Asset-Pipeline-Importer-Registry闭环/
```

完成后将本文档从：

```text
施工文档/当前/
```

移动到：

```text
施工文档/已完成/
```
