# 250-D 当前可自动化施工文档：Formal Asset Import v1

> 状态：已完成并归档  
> 对应方案：`250-AI-Primary-ProjectProduction-Dual-Path-v1方案.md` Phase 4  
> 继承规则：`25-Asset-DB-Importer-MVP.md`、`241-SafeProjectPath-Project-Write-Containment-v1方案.md`  
> 前置完成：250-A ProjectReadiness、250-B CandidateProjectRevision、250-C Controlled SourcePatch  
> 生成日期：2026-07-14  
> 激活日期：2026-07-14  
> 完成日期：2026-07-14

## 1. 施工目标

建立 Rust 主线唯一正式 `ProjectAssetImport` 深 Module：

```text
prepare(request) -> ProjectAssetImportCandidate
validate(candidate) -> ProjectAssetImportValidationReport
apply(candidate + passed validation + approval) -> ProjectAssetImportApplyReceipt
rollback(apply receipt) -> ProjectAssetImportRollbackReceipt
```

Phase 4 必须真实完成：

```text
外部/受信生成文件进入项目 Assets。
生成稳定 GUID 和 AI 可读 sidecar meta。
写入 AssetPipelineDatabase，并生成 AssetGraph / AssetRegistry 派生索引。
保存 source provenance 与 license declaration；不冒充法律判断。
检测路径、assetId、GUID、大小写和替换前置冲突。
Apply 前复核 source/base/candidate/approval binding。
任意写入或复核失败恢复 before state；成功后可显式 rollback。
RuntimePackage Assembler 消费正式 AssetDB 记录，不再只按文件名猜身份。
现有 AI 图片链路不再持有内存态平行 AssetPipelineState。
```

首个真实 Importer 为 PNG texture。它形成 C-01 所需最薄真实资源纵切；Importer contract、数据库和图 schema 保留后续 Audio/Model 扩展点，但本阶段不伪造尚不能验证/cook 的格式。

## 2. 深 Module 与候选边界

`ProjectAssetImport` 隐藏：

```text
外部 source canonical read / drift check / size cap
PNG decode validation
assetId / target path / case-fold collision policy
GUID 生成或 reimport 身份保留
texture descriptor / sidecar meta lowering
AssetPipelineDatabase load / validate / version increment
AssetGraph / AssetRegistry deterministic rebuild
CandidateProjectRevision staging
derived database state digest
approval binding
ImportTransaction lock / snapshot / compensating rollback
structured diagnostics / receipts
```

调用者不能把任意 bytes 直接变成 `CandidateFileChange`，也不能直接写 AssetDB/Graph/Registry。

`CandidateProjectRevision` 继续绑定项目拥有的 source、descriptor 和 meta。`Library/**` 保持 250-B 的派生目录排除合同；AssetDB/Graph/Registry 因此由 `ProjectAssetImportCandidate` 额外保存 before/candidate digest，并在同一 ImportTransaction 中提交和恢复。禁止为了 Phase 4 把整个 `Library` 纳入项目 source digest。

## 3. v1 数据与路径合同

正式路径：

```text
Assets/<target-folder>/<asset-id>.png
Assets/<target-folder>/<asset-id>.asset
Assets/<target-folder>/<asset-id>.asset.meta.json
Library/AssetPipeline/asset-database.json
Library/AssetPipeline/asset-graph.json
Library/AssetPipeline/asset-registry.json
Library/AssetPipeline/import.lock
Library/AssetPipeline/Rollback/<transaction-id>/record.json
Library/AssetPipeline/Rollback/<transaction-id>/snapshot-*.bin
```

规则：

```text
asset-id 只允许 ASCII 小写字母、数字、`-`、`_`，长度 1..128。
target-folder 必须位于 Assets，使用严格 ProjectRelativePath；拒绝绝对路径、`..`、空段和保留名称。
v1 source 必须是 regular PNG，最大 64 MiB；解码输出和尺寸有独立上限，防止压缩炸弹。
GUID 对新导入由 projectId + importId + assetId 的稳定 canonical digest 生成。
reimport 只能在 expected GUID/source hash 与当前 DB 精确匹配时保留 GUID 并替换。
sidecar meta 保存 guid、assetId、type、importer、descriptor/source path、source hash、provenance、license declaration。
license `projectOwned / thirdParty / unknown` 都可记录；unknown 产生 warning，不宣称已获得商业授权。
source URI 若存在必须是可解析 URI；本阶段不联网取证、不下载、不进行许可证法律判断。
```

Schema：

```text
project-asset-import-candidate.v1
project-asset-import-validation-report.v1
project-asset-import-approval.v1
project-asset-import-apply-receipt.v1
project-asset-import-rollback-receipt.v1
project-asset-meta.v1
project-asset-database.v1
project-asset-graph.v1
project-asset-registry.v1
project-asset-import-rollback-record.v1
```

## 4. 冲突、锁与事务规则

冲突策略：

```text
RejectExisting：任何 target path / assetId / GUID / case-fold collision 都 fail closed。
ReplaceMatching：必须提交 expected GUID 与 expected source hash；任一不匹配都拒绝，不静默覆盖。
物理文件存在但 DB 无记录、DB 有记录但 meta/descriptor 缺失、重复 GUID均生成结构化 conflict diagnostic。
Graph/Registry 缺失或 database version 漂移必须被检测，但它们是派生索引：候选从 AssetDB 确定性重建并输出 warning，不得把可重建缓存冒充身份冲突。
```

事务规则：

```text
validate 只读，不写真实项目。
apply 先通过 ProjectWriteScope 获取唯一 import.lock，再复核 source hash、CandidateProjectRevision base、derived state digest、validation digest 和 approval。
写入前把所有 changed path 的 before bytes 放入项目内 rollback snapshot；大文件使用 binary snapshot，不内嵌 JSON 数组。
source / descriptor / meta / database / graph / registry 全部经 ProjectWriteScope 原子文件写入。
任一写入或 after digest 复核失败，反向恢复所有 snapshot，并验证 before digest。
rollback 只在当前 state 精确等于 receipt applied digest 时执行；发生 drift 必须拒绝并要求显式 merge/recovery。
锁释放失败、snapshot cleanup 失败和 rollback 失败不得被成功状态吞掉。
```

目录 capability 是项目 containment，不是恶意文件或解码器安全 sandbox；完成记录必须保留该边界。

## 5. Consumer 收敛

### 5.1 RuntimePackage Assembler

```text
读取 AssetPipelineDatabase 中 current imported record。
使用 DB 的 assetId / GUID / type / descriptor path / source hash。
正式 imported record 覆盖同 assetId 的 legacy basename scan。
现有无 AssetDB 项目保留兼容扫描；兼容项不得伪装为已注册 DB record。
新增 nested Assets 目录覆盖和 imported PNG cook/runtime resolve 测试。
```

### 5.2 Asset Browser

```text
sidecar meta 不作为 Unknown user asset 展示。
正式 descriptor 通过 assetGuid 形成可选 EditorAssetRef。
刷新后 imported descriptor/source 可见，generated/Library/Rollback 不进入用户资源视图。
```

### 5.3 AI Image Generation

```text
删除/停止使用 ai_image_generation.rs 中内存态 AssetPipelineState / ProjectAssetRecord 真相。
Mock provider 输出先是 source，再通过 ProjectAssetImport prepare/validate/approval/apply 正式登记。
生成结果、Asset Browser、Scene placement 和 RuntimePackage 读取同一 AssetDB identity。
```

## 6. 预计涉及文件

```text
rust/crates/editor_core/src/project_asset_import.rs                 new
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/project_write_scope.rs                  import lock primitive
rust/crates/editor_core/src/project_runtime_package_assembler.rs    AssetDB consumer
rust/crates/editor_core/src/asset_browser.rs                        meta hiding / real generated import
rust/crates/editor_core/src/ai_image_generation.rs                  remove parallel in-memory truth
rust/crates/editor_core/Cargo.toml                                  only if current deps prove insufficient
rust/Cargo.lock                                                     only if manifest changes
```

如施工发现需要 EditorSession 新增外部文件选择命令，只能先回填本文并重新自审；本阶段不默认扩大到 Native OS file picker、完整 Import Dock 或 Phase 5 candidate entry。

## 7. Gate A：Policy / Schema / Import Lock

施工：

```text
ProjectAssetImport schema、constants、diagnostics。
source/license metadata validation。
assetId / path / size / PNG / URI policy。
ProjectWriteScope capability-relative exclusive import lock。
AssetDB/Graph/Registry load/validate/rebuild pure functions。
```

测试：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-targets\250d'
cargo test -p editor_core project_asset_import_policy
cargo test -p editor_core project_asset_import_database
cargo test -p editor_core project_write_scope
```

预计成本：首次编译 1-2 分钟，后续秒级。失败时只修 Gate A，不进入候选或 Apply。

## 8. Gate B：Prepare / Validate / Candidate Binding

施工：

```text
外部 source stable read 与 digest。
texture descriptor / sidecar meta lowering。
CandidateProjectRevision stage。
derived DB/Graph/Registry candidate digest。
RejectExisting / ReplaceMatching conflict matrix。
PNG 完整 decode 与 source/base drift validation。
```

测试：

```powershell
cargo test -p editor_core project_asset_import_prepare
cargo test -p editor_core project_asset_import_validate
cargo test -p editor_core candidate_project_revision
```

预计成本：秒级。source/base drift、candidate mutation、duplicate GUID、case collision 和 invalid PNG 必须 fail closed。

## 9. Gate C：Apply / Failure Restore / Rollback

施工：

```text
approval 精确绑定。
import.lock lifecycle。
binary before snapshots 与 rollback record digest。
ProjectWriteScope multi-file apply。
after-state verification。
fault-injection restore 与 explicit rollback。
```

测试：

```powershell
cargo test -p editor_core project_asset_import_apply
cargo test -p editor_core project_asset_import_rollback
cargo test -p editor_core project_asset_import_lock
cargo test -p editor_core project_write_containment
```

高风险矩阵：中途写失败、source drift、project drift、DB drift、approval mismatch、concurrent lock、rollback record tamper、applied state drift、rollback snapshot tamper、outside sentinel unchanged。

## 10. Gate D：真实 Consumer

施工：

```text
RuntimePackage Assembler DB overlay + legacy fallback。
Asset Browser sidecar filtering / typed identity。
AI image generation 正式 ImportTransaction consumer。
新空项目 import -> AssetRef -> RuntimePackage texture payload 纵切。
```

测试：

```powershell
cargo test -p editor_core asset_browser
cargo test -p editor_core ai_image_generation
cargo test -p editor_core project_runtime_package_assembler
cargo test -p editor_core project_asset_import_end_to_end
```

验收必须证明：source copied、meta/DB/Graph/Registry identity 一致、Asset Browser 可选择 descriptor、RuntimePackage 使用相同 GUID 并产生真实 cooked texture payload、rollback 后恢复 before state。

## 11. Gate E：受影响域回归

```powershell
cargo test -p editor_core
cargo test -p project_e2e_gate --lib
cargo test -p project_e2e_gate second_project
cargo test -p project_e2e_gate project_write_containment
```

进入条件：Gate A-D 与累计失败集全绿。预计数分钟；失败先在相同 `CARGO_TARGET_DIR` 精确复现，不立即重跑完整 workspace。

## 12. Gate F：冻结与最终权威回归

环境等价预检：

```powershell
cargo fmt --all -- --check
cargo test -p editor_core
cargo test -p project_e2e_gate second_project
cargo test -p project_e2e_gate project_write_containment
```

预检通过后冻结代码、manifest 和 lockfile。最终使用独立：

```text
CARGO_TARGET_DIR=G:\gameEngin-targets\250d
```

权威命令：

```powershell
cargo test --workspace
cargo test --workspace --all-features
```

当前工作树继承用户已有未提交修改以及 249/250-A/B/C 结果，不能冒充 clean exact commit。最终完成记录必须保存冻结时 changed-file inventory、独立 target、default/all-features 首次权威运行次数和耗时；不得修改或清理用户无关文件。若代码/manifest/lockfile 在权威运行后变化，必须重新预检并重新执行权威矩阵。

## 13. 文档同步与归档

全部 Gate 通过后：

```text
生成 阶段完成记录/2026-07-14-Formal-Asset-Import-v1/00-总览.md。
更新 00-文档地图.md。
更新 49-当前状态与下一步入口.md。
更新 54-当前可自动化施工总入口.md。
更新 240 与 247 的 250 Phase 状态。
更新 250 Phase 4 状态。
更新 施工文档/README.md 与 阶段完成记录/README.md。
把本文从 当前/ 移到 已完成/。
```

完成后下一项只是 Phase 5 Provider-independent Candidate Entry；不得自动生成、激活或施工 Phase 5。

## 14. 本阶段明确不做

```text
不做 Audio/Model/Material 完整 Importer。
不做 OS file watcher、批量 SVN/Git 同步或完整 ImportQueue。
不做 Native 文件选择器、Import Dock、Importer settings UI、thumbnail 新系统。
不做联网下载、license 法律判定或自动商业授权。
不做完整 Artifact Cache / DDC / platform import variants。
不把 Library 全量纳入 CandidateProjectRevision source digest。
不让 ProjectPatch/LLM 获得任意 external read authority。
不进入 Provider、Dev Host、FeatureSpec、Renderer dissolve 或 C-01。
不新增玩法专用 Engine Core API，不扩大 Rule IR。
```

## 15. 施工文档自审

### 15.1 正式方案一致性

通过。本文只细化 250 Phase 4 已确认的 external copy、GUID/meta、AssetDB/Graph、license/source metadata、conflict 和 rollback；没有修改双通道、RuntimePackage 或 Rule IR 边界，无需修改 250 正式方案。

### 15.2 历史实现与审查吸收

```text
25：采纳 Unity-like meta/GUID、ImportTransaction、AssetDB truth、Registry derived、per-unit report；不一次性施工其完整多格式/批处理长期范围。
241：所有 project writes、snapshot 和 rollback 穿过 ProjectWriteScope；外部 source 只读不转授项目外写权限。
250-B/C：复用 CandidateProjectRevision 的 source candidate/base binding 和 approval/rollback纪律；Library derived state使用独立 digest，不破坏排除合同。
44 战略审查：采纳 license/provenance/AssetGraph 作为 AI 连续维护基础；不把 Codex 本身当优势。
45 优先级审查：Phase 4 属于用户已确认 250 主线；用户本轮“开始下一阶段”构成该独立阶段启动授权。
46 实跑协议：V-R1 资源导入是后续 C-01/竞争 Gate 输入；Phase 4 不冒充完整蝎子虚化或三引擎验收。
```

未发现针对 250-D 的独立外部审查要求修改正式方案。

### 15.3 范围、深度与测试

```text
没有把 AssetDB 状态散到 Asset Browser、AI image 或 Assembler。
没有只复制 PNG 而跳过 identity、metadata、conflict、transaction 和 rollback。
没有为长期所有格式造空壳 importer。
包含定向、受影响域和 default/all-features 三层验证。
明确记录混合工作树限制、独立 target 和代码冻结规则。
```

结论：施工文档自审通过。激活前必须再次确认 `当前/` 为空、基线测试仍通过、涉及文件未发生并行漂移；移动到 `当前/` 并更新 `54` 后才可施工。
