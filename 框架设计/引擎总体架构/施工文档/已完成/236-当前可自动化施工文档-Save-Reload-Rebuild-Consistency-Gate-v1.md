# 236-当前可自动化施工文档-Save / Reload / Rebuild Consistency Gate v1

> 状态：当前可施工；施工文档自审通过，尚未开始施工。  
> 施工文档日期：2026-07-10。  
> 对应正式方案：`../../236-Save-Reload-Rebuild-Consistency-Gate-v1方案.md`。  
> 采用方案：`B-min+: Canonical Multi-Checkpoint Consistency Gate`。  
> 外部审查：未提供针对 236 的外部方案审查文档；236 已完成内部方案自审，并把 6 项修正写回正式方案。  
> 当前队列：本文件必须是 `施工文档/当前/` 中唯一施工文档。

## 0. 施工目标

建立一条低频、严格、可解释的一致性证据链：

```text
复杂项目临时副本
  -> process A 真实编辑与保存
  -> process B 独立重开与 typed reload
  -> BuildRecipeDigest / AssemblyInputDigest
  -> RuntimePackage safe staging/publish A
  -> 清除临时派生产物
  -> RuntimePackage safe staging/publish B
  -> RuntimeContentHash / payload tree compare
  -> RuntimePackage + Asset + Prefab + Texture 正式 loader
  -> save-reload-rebuild-consistency-report.v1
  -> Report Panel validation.save_reload_rebuild
```

本轮必须交付：

```text
SHA-256、domain-separated、length-framed 的 aife-canonical-framed.v1 摘要合同。
BuildRecipeDigest、AssemblyInputDigest、RuntimeContentHash、PayloadTreeDigest、LoadedSemanticDigest 分层。
runtime-package.v1 的 contentHash 固定输出 sha256:<64 lowercase hex>，兼容读取历史 string。
RuntimePackage Builder 单一内部 write plan / payload inventory。
package-relative path containment、Windows collision 和 loader safe-join。
单写者、same-parent staging/backup、pre/post-load、rollback/recovery publish。
Scene/Prefab/Rule/AUI/Input 共用原子文件替换合同，Prefab 成功写盘后才清 dirty。
process-isolated SavedAuthoring / ReopenedAuthoring checkpoint。
机器可读 source/runtime witness 与 mutation/scope coverage。
复杂打飞机双构建、stale artifact、last-good、正式 loader E2E。
Editor Off/Summary/Trace 报告和 active-project-scoped Report Panel provider；Runtime 默认 Off。
```

## 1. 不得突破的边界

```text
Project typed authoring assets 仍是编辑真相。
ProjectRuntimePackageAssembler 仍是项目目录 -> RuntimePackageBuildInput 的唯一正式入口。
RuntimePackage 仍是运行真相；checkpoint/digest/report 都是可丢弃 editor/test evidence。
不新增第二 assembler、第二 manifest、ConsistencyAsset、ProjectSnapshotAsset 或持久化 checkpoint 资产。
不新增 SaveManager、SaveOwnershipRouter、每 domain ConsistencyManager 或 Runtime 常驻 consistency service。
Builder write plan/inventory 只能是 engine_runtime 内部实现，不成为用户心智或公共业务层。
process child helper 只服务 236 E2E/CLI 验证，不进入 Game.exe Runtime 热路径。
Runtime 默认不生成 consistency JSON、trace 或文件扫描。
严格 Gate 不接入每次 Play/Build；217 高频 preview fingerprint 保持原职责。
不实现 CAS/DDC、远程缓存、完整增量 Build Graph、跨平台 byte-identical build 或双 Runtime frame replay。
不实现全局 Save All UI、外部修改 merge UI、通用 crash journal、installer/signing/store package。
不把复杂打飞机玩法名写入 engine_runtime/editor_core 通用接口；玩法 witness 只在 project_e2e_gate fixture 中出现。
所有破坏性编辑、cache cleanup、staging、rollback 测试只操作验证过的临时 workspace。
不得修改仓库中的 samples/complex_shooter_project 源目录来让测试通过。
不得清理、回退或格式化无关 dirty worktree。
```

## 2. 当前基线与固定施工接口

当前代码事实：

```text
RuntimePackageBuilder::stable_package_hash 仍是 FNV1a64 + 手工 parts，覆盖不完整。
RuntimePackageBuilder 仍直接写 request.output_dir，旧 payload 可能残留。
Builder/Loader 对 AUI/Font/Texture/manifest path 尚无统一 containment/collision 校验。
PrefabWorkflowService::save_stage 在磁盘写入前清 dirty。
Scene/Prefab/AUI/Rule 多数仍直接 fs::write；Input 有独立 transactional_write。
load_runtime_package 不单独物化 Prefab 正文或 Texture RGBA payload。
ProjectRuntimePackageAssemblyReport 尚无完整跨 domain source/runtime mapping。
project_e2e_gate 当前只有 library target，没有 236 process child runner。
ReportRegistry 尚未注册 validation.save_reload_rebuild。
```

依赖决策：

```text
engine_runtime 增加 sha2 0.10，用于 SHA-256。
当前 rustc 1.96 已具备标准库 File lock 能力；优先使用 std::fs::File 的 try_lock/unlock，不新增 fs2。
unique staging/temp 名称使用 process id + atomic counter + time/attempt id，不新增 uuid/tempfile。
如果实际编译证明标准库 lock 在目标 toolchain/平台不可用，必须先回填施工文档再选择单一成熟依赖，不能静默改成易残留 create_new lockfile。
```

固定模块接口方向：

```text
engine_runtime::canonical_digest
  ConsistencyDigest
  ConsistencyDigestKind
  digest_canonical(kind, payload_schema, typed_value/frames)

engine_runtime::atomic_file_replace
  atomic_replace_file(target, bytes) -> structured result
  fault injection 只作为模块内部测试 seam

engine_runtime::runtime_package_path
  validate_package_relative_path
  safe_join_package_path
  Windows collision key

RuntimePackageBuilder
  request.output_dir 继续表示 final output
  内部完成 write plan -> staging -> validation -> publish

editor_core::project_consistency
  BuildRecipeDigest / AssemblyInputDigest
  checkpoint/report schema
  active-project cached report state

project_e2e_gate
  parent runner + process A/process B child modes
```

兼容规则：

```text
RuntimePackageManifest 继续使用 runtime-package.v1，不为 contentHash 新增第二 manifest。
新 Builder 输出 contentHash=sha256:<hex>；Loader 继续容忍历史 opaque string。
236 Gate 只接受可重算的 sha256:<hex> 作为新一致性证据。
RuntimePackageBuildRequest.output_dir、现有 Build/Preview/Export 调用方式保持不变。
现有 report/manifest JSON 字段除明确新增的可选 evidence 外不做无关迁移。
```

## 3. 预计涉及文件

摘要、IO、路径、Builder 与 Runtime loader：

```text
rust/crates/engine_runtime/Cargo.toml
rust/Cargo.lock
rust/crates/engine_runtime/src/canonical_digest.rs（新增）
rust/crates/engine_runtime/src/atomic_file_replace.rs（新增）
rust/crates/engine_runtime/src/runtime_package_path.rs（新增）
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_asset_loader.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
rust/crates/engine_runtime/src/runtime_texture.rs
rust/crates/engine_runtime/src/project_rule_asset.rs
rust/crates/engine_runtime/src/lib.rs
```

Assembler、保存链和 Editor report：

```text
rust/crates/editor_core/src/project_consistency.rs（新增）
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/prefab_workflow.rs
rust/crates/editor_core/src/services/prefab_service.rs
rust/crates/editor_core/src/aui_authoring.rs
rust/crates/editor_core/src/input_mapping_authoring.rs
rust/crates/editor_core/src/rule_authoring.rs（只在真实调用链需要时）
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/services/project_service.rs（只在加载 active-project report cache 时需要）
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/*（按现有 domain 分组）
```

Process runner、复杂打飞机 Gate 与报告：

```text
rust/crates/project_e2e_gate/Cargo.toml（仅增加 bin/integration test 所需配置）
rust/crates/project_e2e_gate/src/save_reload_rebuild_consistency.rs（新增）
rust/crates/project_e2e_gate/src/bin/save_reload_rebuild_consistency_gate.rs（新增）
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/tests/save_reload_rebuild_consistency.rs（新增）
```

文档与归档：

```text
框架设计/引擎总体架构/236-Save-Reload-Rebuild-Consistency-Gate-v1方案.md
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md
```

允许根据实际模块所有权微调文件，但不得借机拆新 crate、重写全部 Build 系统或增加新的持久化架构层。

## 4. Gate A - Canonical Digest 与作用域合同

任务：

```text
1. 增加 sha2 依赖和 canonical_digest 模块。
2. 实现 ConsistencyDigest(schema_version, kind, algorithm, canonical_encoding, value)。
3. 固定 aife-canonical-framed.v1 preimage：magic + 长度分帧的 schema/kind/payload schema/payload bytes。
4. 对 object key、enum tag、string、integer、finite float、-0、missing/default、ordered/unordered collection 实现 236 规则。
5. 大型 FontAtlas/Texture bytes 直接流式进入 SHA-256，不先拼接巨型字符串。
6. 禁止 parts.join("|") 和按 domain 手工漏字段列表；完整 typed value 或 Builder write plan 是唯一 contribution 来源。
7. 在 editor_core 实现 BuildRecipeDigest：ProjectManifest、selected BuildProfile、active scene、request semantic fields、schema/cooker versions。
   ProjectManifest.createdAt/lastOpenedAt 等非语义时间字段必须显式排除；不能直接 hash raw project JSON。
8. 为 RuntimePackageBuildInput/source structs 增加完整 Serialize canonical view，实现 AssemblyInputDigest；top-level 无序集合按 stable id，嵌套语义顺序保留。
9. 为 RuntimeContentHash 建立 Builder 内部 RuntimePackageWritePlan/PayloadInventory；content hash 与写盘共用同一 plan。
10. 明确 RuntimeContentHash 排除 contentHash 自身、previous manifest、输出路径、request id、reports 和 recipe-only 参数。
11. Builder 输出 manifest.contentHash=sha256:<64 lowercase hex>；legacy hash 只兼容读取，不通过 236 strict evidence。
12. 对 digest scope 增加结构化 diagnostic：canonical_encode_failed、digest_insensitive、digest_scope_violation、unsupported_content_hash_algorithm。
13. PayloadTreeDigest 对排序后的 `package-relative path + raw file SHA-256` 做长度分帧；包含 manifest/runtime payload，排除 reports/**。
```

Gate 测试：

```powershell
cargo test -p engine_runtime canonical_digest
cargo test -p engine_runtime runtime_package_digest
cargo test -p editor_core project_consistency_digest
```

通过条件：

```text
SHA-256 known vector 与 64 位小写 hex 通过。
不同 kind/schema 的相同 payload 得到不同摘要。
JSON 空格/key 原始顺序变化不改变 semantic digest。
Scene/Prefab/Rule/AUI/Input 的语义顺序变化会改变对应摘要。
BuildProfile.frameLimit/headlessSurfaceGate 改变 BuildRecipeDigest，但不改变无差异 payload 的 RuntimeContentHash。
active scene/component schema/font bitmap/texture RGBA 等有效变化改变正确摘要。
contentHash 可由同一 write plan 重算，且 preimage 不包含 contentHash 自身。
现有 FNV 手工 stable_package_hash 不再作为 Builder 正式 contentHash。
每条测试命令至少执行一个新增测试；0 tests filtered 不算通过。
```

### Gate A 施工记录（2026-07-10）

状态：`已完成`。

已落地：

```text
- engine_runtime::canonical_digest：SHA-256、aife-canonical-framed.v1、长度分帧、canonical JSON、PayloadTreeDigest。
- RuntimePackageWritePlan：同一 plan 生成 RuntimeContentHash、PayloadTreeDigest 和实际 payload 写盘；contentHash 输出固定为 sha256:<64 lowercase hex>。
- AssemblyInputDigest：顶层 source collection 按 stable id；嵌套数组保持语义顺序；FontAtlas/Texture 原始 bytes 先流式 SHA-256，不生成巨型 JSON 字符串。
- editor_core::BuildRecipeDigest：显式排除 ProjectManifest.createdAt / lastOpenedAt，包含 BuildProfile、active scene 和 schema/cooker versions。
- legacy opaque contentHash 仍可由 RuntimePackage loader 读取；236 strict evidence 只接受 sha256: 值。
```

已通过：

```powershell
cargo test -p engine_runtime runtime_package_builder  # 9 passed
cargo test -p engine_runtime canonical_digest          # 5 passed
cargo test -p engine_runtime runtime_package_digest    # 2 passed
cargo test -p editor_core project_consistency_digest   # 1 passed
```

## 5. Gate B - Save Correctness 与共享原子替换

任务：

```text
1. 在 engine_runtime 提取项目无关 atomic_file_replace 深模块，供 engine_runtime/editor_core 共同复用。
2. 接口完成 same-directory unique temp、write-all、flush/sync、existing backup、rename commit、rollback、cleanup。
3. 不对外暴露 SaveManager、document registry 或 domain ownership；fault injection 只在模块内部测试。
4. SceneSavePipeline 改用共享 helper；成功后再 refresh path/hash/dirty。
5. PrefabWorkflowService::save_stage 不再提前清 dirty；Prefab service 在 save_asset 成功后显式 mark saved。
6. AuiDocument save 改用共享 helper；失败时保留原文件。
7. ProjectRuleAsset save 改用同一 helper；RuleAuthoring 不复制另一份 transaction 实现。
8. InputMapping 保存保留 source-hash guard/normalize 语义，但删除独立 temp/backup 实现并复用 helper。
9. 所有失败返回 domain/path/stage/error/next action；不得吞掉 rollback 失败。
10. v1 只承诺 write/rename/process commit failure safety；不伪称已实现断电 durable journal。
```

Gate 测试：

```powershell
cargo test -p engine_runtime atomic_file_replace
cargo test -p editor_core scene_save
cargo test -p editor_core prefab_authoring
cargo test -p editor_core aui_authoring
cargo test -p editor_core rule_authoring
cargo test -p editor_core input_mapping_authoring
```

通过条件：

```text
成功提交后目标 bytes 正确，temp/backup 不残留。
write/rename/commit fault injection 后旧文件保持不变或被确定性恢复。
Scene/Prefab/Input working copy 只有成功提交后才 clean。
Prefab 保存失败后 stage.dirty 保持 true。
Rule/AUI 保存失败不会截断原 source。
Input stale source hash 仍拒绝覆盖外部修改。
所有 saver 复用同一个 atomic helper，不出现第二套 temp/backup 算法。
```

### Gate B 施工记录（2026-07-10）

状态：`已完成`。

```text
- engine_runtime 新增共享 atomic_file_replace：same-directory unique temp/backup、write/flush/sync、commit、rollback、cleanup 和确定性 fault injection。
- Scene/Prefab/AUI/Rule/Input saver 全部复用共享 helper；Prefab save_stage 不再提前清 dirty，只有 save_asset 成功后 mark saved。
- 保存失败保留旧文件和 working-copy dirty 状态，并返回 stage/path/error/next action。
```

```powershell
cargo test -p engine_runtime atomic_file_replace  # 2 passed
cargo test -p editor_core scene_save              # 10 passed
cargo test -p editor_core prefab_authoring        # 1 passed
cargo test -p editor_core aui_authoring           # 5 passed
cargo test -p editor_core rule_authoring          # 7 passed
cargo test -p editor_core input_mapping_authoring # 9 passed
```

## 6. Gate C - RuntimePackage Safe Path、Staging 与 Publish

任务：

```text
1. 实现 runtime_package_path：非空 relative `/` path、禁止 root/drive/`.`/`..`、containment 和 safe join。
2. 实现 Windows collision key，拒绝大小写折叠、尾随点/空格、保留设备名和重复 claim。
3. Scene/Prefab/AUI/Input/Font/Texture id 生成文件名之前验证安全单段 path segment。
4. Builder write plan 生成唯一 payload inventory；manifest/runtime URI 与 inventory 双向校验，unowned/unreferenced runtime payload fail closed。
5. RuntimePackage loader、AUI/Font/Input loader、RuntimeAssetLoader、load_runtime_texture_payload 对所有 manifest/cooked path 使用同一 safe join。
6. RuntimePackageBuilder::build 保持 output_dir=final，内部创建 same-parent/same-volume unique staging/backup。
7. 使用 rustc 1.96 标准库文件锁实现每 final output 单写者 publish guard；busy 立即结构化失败，不等待死锁。
8. staging 写完后先 load_runtime_package，再加载 inventory 声明的 runtime-loadable typed payload；Prefab 文档和 Texture payload 必须走正式 loader。
9. pre-validation 成功后 final->backup、staging->final，再从 final 进行 post-publish load verification。
10. publish/post-load 失败恢复 last-good；恢复失败保留路径证据并返回 output_publish_rollback_failed。
11. 启动时处理 package-local orphan 状态：final 缺失恢复 backup；final+backup 时先验证 final 再决定保留/恢复。
12. reports/** 不进入 RuntimeContentHash/PayloadTreeDigest；失败 attempt 不得写进或污染 last-good final。
13. 在临时 final 注入 stale sentinel 或先构建额外 payload，正式 publish 后必须消失。
14. 使用 deterministic fault injection 验证 failed-build-preserves-last-good，不依赖偶发权限错误。
15. 保持 EditorPreviewPackage、DesktopExport、BuildAndRun 的调用接口和成功路径兼容。
```

Gate 测试：

```powershell
cargo test -p engine_runtime runtime_package_path
cargo test -p engine_runtime runtime_package_staging
cargo test -p engine_runtime runtime_package_publish
cargo test -p engine_runtime runtime_asset_loader
cargo test -p editor_core editor_preview_package
cargo test -p editor_core desktop_export
```

通过条件：

```text
absolute/traversal/root escape/symlink escape/Windows collision 在写盘或读取前失败。
两个 publisher 竞争同一 final 时只有一个成功，测试有硬超时且无死锁。
staging 与 backup 始终位于已验证 output root；清理函数拒绝 root 外路径。
stale sentinel/被删除 payload 不出现在新 final。
构建、pre-load、rename、post-load 任一失败均保留或恢复 last-good。
成功 final 的 inventory、RuntimeContentHash、PayloadTreeDigest 和正式 loader evidence 一致。
现有 Preview/Export package 测试继续通过，旧调用者不需要了解 staging 细节。
```

### Gate C 施工记录（2026-07-10）

状态：`已完成`。

```text
- 新增 runtime_package_path：规范 relative path、Windows collision/reserved-name 检查、safe join、containment 与 symlink escape 防护。
- Builder 改为同父目录 staging/backup、标准库 single-writer lock、pre/post formal load、rollback/orphan recovery 和 stale payload replacement。
- Builder inventory 成为 payload 所有权真相；RuntimePackage/AUI/Font/Input/Asset/Texture loader 统一使用 safe join。
```

```powershell
cargo test -p engine_runtime runtime_package_path       # 6 passed
cargo test -p engine_runtime runtime_package_staging    # 1 passed
cargo test -p engine_runtime runtime_package_publish    # 2 passed
cargo test -p engine_runtime runtime_asset_loader       # 10 passed
cargo test -p editor_core editor_preview_package        # 3 passed
cargo test -p editor_core desktop_export                # 5 passed
```

## 7. Gate D - Process-Isolated Authoring Save / Reopen

任务：

```text
1. 在 editor_core 定义 save-reload-rebuild-checkpoint.v1、SaveReloadRebuildConsistencyReport 和 SourceRuntimeWitness 类型。
2. ProjectRuntimePackageAssemblyReport 增加 `#[serde(default)]` 的 additive project-relative source mappings：domain/source path/object/build input path/runtime path；保持 v1 reader 兼容并补旧 JSON 缺字段测试。
3. mapping 来自 typed loader/assembler/builder inventory；禁止在 complex shooter 测试中手写永远通过的 expected map。
4. 新增 project_e2e_gate bin：run、author-save-child、reopen-read-child 三种模式；CLI 固定 project/temp-root/report/report-level/child-invocation 参数，child mode 必须校验 parent token。
5. parent 为每次 child launch 生成独立 invocation id、checkpoint path 和 120 秒硬超时；记录 executable/exit/status/PID 作为 Trace。
6. process A 在 complex shooter 临时副本 OpenProject，通过现有 UiCommand/domain service 完成 Scene、Prefab、Rule、AUI、Input、AssetRef 代表性编辑。
7. process A 通过真实 Save command 保存 Scene/Prefab/Input；Rule/AUI 使用其正式持久化命令；保存失败立即退出失败。
8. process A 写 SavedAuthoring checkpoint artifact 后正常退出，不把 EditorSession/typed object 传给 process B。
9. process B 独立启动，new EditorSession + OpenProject + typed loaders，写 ReopenedAuthoring checkpoint 后退出。
10. parent 验证两个不同 OS child invocation、成功 exit、reopen_mode=process_isolated，再比较 domain digest/stable id/witness/source path。
11. 同进程 drop/new EditorSession 测试可保留为 fast test，但不能让正式 E2E Passed。
12. 所有 child artifact/项目副本位于验证过的 temp root；超时必须终止 child 并清理 handle，不遗留后台进程。
```

复杂打飞机最小 witness：

```text
Scene：entity-player transform 或 component typed field + AssetRef。
Prefab：prefab-player-bullet 的稳定 sourceEntityId 字段，保存后 stage clean。
Rule：现有 Rule Card 可编辑字段并保留 canonical IR/rule identity。
AUI：hud-main 的 node field/binding/imageRef。
Input：stable binding_id 对应 device/trigger/processor 字段。
Asset：Asset Browser 同类型结构化 AssetRef/texture descriptor evidence。
Project/BuildProfile：typed digest 与 source path；不要求普通 UI 在本 Gate 修改它们。
```

Gate 测试：

```powershell
cargo test -p editor_core project_consistency
cargo test -p project_e2e_gate --test save_reload_rebuild_consistency process_isolated_authoring
```

通过条件：

```text
process A 在退出前完成真实命令和保存，仓库样例源文件 hash 不变。
process B 不接收 A 的内存对象，只读临时项目磁盘真相。
parent 证明两个独立 child invocation；不能只靠可能复用的 PID。
SavedAuthoring/ReopenedAuthoring 各 domain semantic digest、stable ids 和 required witness 相等。
任一 save/reopen/timeout/child exit 失败都有 checkpoint/domain/path/object/next action。
测试命令真实执行 integration test 和 bin；0 tests filtered 不算通过。
```

### Gate D 施工记录（2026-07-10）

状态：`已完成`。

已落地：

```text
- 新增 save-reload-rebuild checkpoint/report/witness schema，以及原子读写入口。
- ProjectRuntimePackageAssemblyReport 增加向后兼容的 sourceMappings，覆盖 Project/BuildProfile/Scene/Prefab/Rule/AUI/Input/Asset。
- project_e2e_gate 以两个独立 OS child invocation 执行真实 author-save 与 disk-only reopen；child 具有 parent token、独立 invocation id、120 秒硬超时和 stderr 失败证据。
- Process A 通过现有 UiCommand/正式持久化入口编辑并保存 Scene/Prefab/Rule/AUI/Input；Process B 创建新 EditorSession 后只从临时项目磁盘重开。
- 修复 AUI 保存后 documentId 规范化为 document_id 时 source mapping 错退回猜测路径的问题；source/runtime path 继续保持分离。
- E2E 比较 SavedAuthoring/ReopenedAuthoring 的 domain digest、stable ids、source paths 和 parent token，并验证仓库 sample byte hash 未改变。
```

已通过：

```powershell
cargo test -p editor_core project_consistency  # 2 passed
cargo test -p project_e2e_gate --test save_reload_rebuild_consistency process_isolated_authoring  # 1 passed
```

## 8. Gate E - Clean Rebuild、Formal Loader、Mutation 与 Report Panel

任务：

```text
1. parent 从 ReopenedAuthoring 磁盘状态生成 BuildRecipeDigest A / AssemblyInputDigest A。
2. 在 owned temp final A 放入 stale sentinel，调用正式 Builder safe publish；验证 sentinel 消失并加载 published final A。
3. 清除 temp project 下 preview cache/prior derived outputs；路径必须通过 containment 检查。
4. fresh assemble/build 到独立 final B，生成 BuildRecipeDigest B / AssemblyInputDigest B。
5. 比较 recipe、assembly input、manifest sha256 contentHash、runtime file inventory 和 PayloadTreeDigest A/B。
6. 从 published final A/B 调用 load_runtime_package；加载 inventory 声明的全部 runtime-loadable typed assets/Prefab documents/Textures。
7. 至少实例化一个 Prefab，通过 RuntimeAssetIndex resolve representative AssetRef，并比较 Rule/Input/AUI/Font/Texture loaded semantics。
8. 连接 assembler source mapping、Builder inventory 和 loader result，生成 required SourceRuntimeWitness；缺失/重复/unresolved mapping 失败。
9. 完成 mutation matrix：Scene/Prefab/Rule/AUI/Input/Project/BuildProfile/active scene/component schema/Font/Texture/AssetRef/deletion/order。
10. 完成 no-semantic/scope matrix：JSON formatting/key order、typed default、reports path/duration、recipe-only BuildProfile field。
11. 输出 save-reload-rebuild-consistency-report.v1：checkpoint/comparison/domain/mutation/witness/artifact/diagnostic/next action。
12. report artifact 默认位于 active project 的 `.aife/reports/save-reload-rebuild/latest.json`；E2E 对仓库样例使用 temp output，不写 sample source。
13. editor_core 使用 active-project-scoped cached report state；OpenProject/显式 refresh 时加载一次，Report Panel compose/frame 不做文件 IO。
14. 注册 `validation.save_reload_rebuild` provider；Editor 默认 Summary（只显示已存在 cache，不自动运行 Gate），支持 Off/Summary/Trace；Summary 不暴露 temp 绝对路径，Trace 才含 digest/mapping/process/publish 细节。
15. 当前项目没有 artifact 或 project_id 不匹配时显示 NotRun；切换项目不能展示上一项目结果。
16. Runtime 不依赖 project_consistency/report model，不生成 report，不扫描项目源目录。
17. latest report artifact 使用 Gate B 的 atomic helper 提交；写入失败不能覆盖上一份完整 report，session cache 只接收成功解析且 project_id 匹配的结果。
```

Gate 测试：

```powershell
cargo test -p engine_runtime runtime_package_digest
cargo test -p engine_runtime runtime_instance_loader
cargo test -p engine_runtime runtime_texture
cargo test -p editor_core report_panel
cargo test -p editor_core project_consistency
cargo test -p project_e2e_gate --test save_reload_rebuild_consistency save_reload_rebuild_consistency
```

通过条件：

```text
BuildRecipe A == B、AssemblyInput A == B、RuntimeContentHash A == B、PayloadTreeDigest A == B。
contentHash 为 sha256:<hex> 且通过同一 canonical write plan 重算。
published A/B 正式 loader semantic digest 相等。
inventory 声明的 runtime-loadable typed payload 全部加载；代表性 Prefab 实例化和 Texture RGBA evidence 非空。
所有 required source/runtime witness resolved，AI 可定位 domain/source/runtime path/object/field。
所有 sensitivity/scope tests 进入 report；弱 hash、错误作用域和 stale payload 均不能 Passed。
Report Panel 只显示 active project artifact；Summary/Trace 内容与成本分档正确。
复杂打飞机仓库样例源目录在 Gate 前后 byte hash 不变。
```

### Gate E 施工记录（2026-07-10）

状态：`已完成`。

已落地：

```text
- 从 Process B 的 reopened 磁盘真相执行两次独立 clean assemble/build；比较 BuildRecipe、AssemblyInput、RuntimeContentHash、PayloadTreeDigest 和 payload inventory。
- RuntimePackage A 先注入 stale sentinel，正式 safe publish 后验证已清除；A/B 均通过正式 RuntimePackage loader。
- 对加载后的 Scene/Asset/Rule/Input/AUI/Font typed semantics 分域比较；逐个加载全部 Texture RGBA 与 Prefab document，并至少实例化一个代表 Prefab。
- mutation matrix 覆盖 Scene/Prefab/Rule/AUI/Input/Project/BuildProfile/active scene/component schema/Font/Texture/AssetRef/deletion/order，以及 JSON key order、typed default、project timestamps 和 report scope。
- source/runtime witness 由 assembler sourceMappings、Builder inventory 和正式 loader 结果连接生成；8 个 required domain 均 resolved。
- 新增 project_e2e_gate `run` 父模式，支持 project/temp-root/report/report-level，并以两个独立 child invocation 执行正式 Gate。
- EditorSession 增加 active-project-scoped report cache：OpenProject/RecentProject/显式 Refresh 时读盘，compose/frame 只读内存；projectId 不匹配时拒绝缓存。
- Report Panel 注册 validation.save_reload_rebuild；默认 Summary 隐藏摘要值、进程和临时绝对路径，Trace 展示 comparison/mutation/process/witness/artifact，Off 不收集 provider。
- latest report 使用共享 atomic_file_replace；Runtime 不依赖 editor consistency report model。
```

已通过：

```powershell
cargo test -p engine_runtime runtime_package_digest  # 2 passed
cargo test -p engine_runtime runtime_instance_loader # 8 passed
cargo test -p engine_runtime runtime_texture         # 2 passed
cargo test -p editor_core report_panel                # 7 passed
cargo test -p editor_core project_consistency         # 2 passed
cargo test -p project_e2e_gate --test save_reload_rebuild_consistency save_reload_rebuild_consistency  # 1 passed
cargo test -p project_e2e_gate --test save_reload_rebuild_consistency run_mode_writes_passing_report_and_returns_success  # 1 passed
```

## 9. Gate F - 整体回归、文档与归档

整体回归在 `G:\gameEngin\rust` 分命令执行：

```powershell
cargo fmt --check
cargo test -p engine_input
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_input
cargo test -p editor_ui_renderer
cargo test -p editor_wgpu_renderer
cargo test -p editor_window_winit
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo test -p project_e2e_gate
```

测试规则：

```text
每个 Gate 先跑定向测试，通过后才能进入下一 Gate。
新增 filter 命令必须确认至少执行一个对应新测试；0 tests filtered 不算通过。
process/publish concurrency 测试必须有硬超时，不能留下后台 child 或锁。
长命令记录耗时；明显停滞时先诊断具体测试，不无限等待也不直接跳过。
失败先修当前 Gate，不跨 Gate 堆积问题。
只修 236 或本轮直接暴露的回归，不整理无关 dirty worktree。
不以人工目测、日志关键字或仅比较一个总 hash 代替结构化证据。
```

全部测试通过后才能：

```text
生成 阶段完成记录/2026-07-10-Save-Reload-Rebuild-Consistency-Gate-v1/00-总览.md。
把 236 正式方案状态更新为已施工完成，并记录实际实现/测试边界。
更新 00-文档地图.md。
更新 49-当前状态与下一步入口.md。
更新 54-当前可自动化施工总入口.md。
更新 施工文档/README.md。
更新 阶段完成记录/README.md。
更新 227，将 P2-1 标为完成，下一默认系统切换到 P2-2 Release Package Polish / Metadata / Icon / Layout v1。
把本施工文档从 施工文档/当前/ 移到 施工文档/已完成/。
确认 施工文档/当前/ 重新为空。
```

任何 Gate 或整体回归未通过时，不得生成“已完成”阶段记录，不得归档本施工文档，也不得把 227 的 P2-1 标为完成。

### Gate F 施工记录（2026-07-10）

状态：`已完成`。

```text
cargo fmt --check：通过。
engine_input：26 passed。
editor_ui_model：52 passed。
editor_core：330 passed，1 ignored（既有 env-gated）；首次全量并行运行暴露 Preview 测试复制 Build 派生产物与 atomic publish 的竞态，修正测试副本边界后全量通过。
editor_input：29 passed。
editor_ui_renderer：12 passed。
editor_wgpu_renderer：14 passed。
editor_window_winit：85 passed。
engine_runtime：421 passed。
runtime_player_winit：17 passed。
project_e2e_gate：41 unit + 3 consistency integration passed。
```

Gate A-F、整体回归、完成记录和入口同步均已完成；本施工文档可归档。

## 10. 施工文档自审

审查结论：`通过，可以从 Gate A 开始施工`。

审查记录：

```text
外部审查：未提供针对 236 的外部方案审查文档。
方案吸收：236 自审发现的 digest scope、process isolation、path/publish safety、formal loader、mutation completeness、canonical encoding 六项均已进入具体 Gate。
兼容补充：contentHash string 固定为 sha256:<hex>，保持 runtime-package.v1，不新增第二 manifest。
真相边界：authoring assets / assembler input / RuntimePackage 三类真相未改变；checkpoint/report 都是可丢弃 evidence。
结构复杂度：canonical encoder、atomic replace、path validator 和 Builder write plan 都是现有模块的深实现，没有新增用户架构层。
依赖：只预期增加 sha2；rustc 1.96 优先使用标准库 file lock，不无理由引入 fs2/uuid/tempfile。
保存：Scene/Prefab/Rule/AUI/Input 共用一个 atomic helper；Prefab dirty 时序和失败保持已列为强制测试。
发布：containment、Windows collision、single writer、stale removal、pre/post-load、rollback 和 last-good 均有确定性测试。
进程：正式 reopen 必须由两个 OS child invocation 完成；同进程 Session 测试不能代替。
运行读取：RuntimePackage、Asset、Prefab、Texture 正式 loader 都进入验收，没有用 fs::read 冒充 Runtime 闭环。
摘要输入：BuildRecipe 明确排除 project 时间字段；PayloadTreeDigest 明确按路径与 raw file digest 分帧。
映射：source/runtime witness 来自 assembler report + Builder inventory + loader result，不允许 sample-only 手写 expected map。
报告：Editor Off/Summary/Trace 且 active-project scoped/cache-backed；Runtime 默认 Off，frame compose 不读文件。
性能：严格双 build/进程重开只用于显式 Gate；普通 Play/Runtime 热路径不执行。
范围：未包含 CAS/DDC、Save All UI、双 Runtime replay、跨平台 byte-identical、installer/signing 等 deferred 能力。
测试：每个 Gate 有定向命令和可判定通过条件，最后包含正式 workspace 全量回归。
安全：破坏性操作限定到 containment 验证过的 temp/output root，不修改仓库样例，不回退无关工作树。
归档：阶段记录、入口同步、227 状态和当前施工文档移动条件均已明确。
```

自审未发现阻塞施工的剩余冲突。施工必须严格从 Gate A 开始；当前 Gate 的代码、定向测试和证据通过后，才能进入下一 Gate。
