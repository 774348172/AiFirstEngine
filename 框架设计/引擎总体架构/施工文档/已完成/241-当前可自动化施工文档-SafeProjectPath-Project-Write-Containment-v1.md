# 241-当前可自动化施工文档-SafeProjectPath / Project Write Containment v1

> 状态：已完成，2026-07-11 已通过 Gate A-F、默认 workspace 与 all-features workspace 回归并归档。  
> 正式方案：`../../241-SafeProjectPath-Project-Write-Containment-v1方案.md`。  
> 方案选择：用户已确认方案 C，采用 capability-backed `ProjectWriteScope`。  
> 方案审查：2026-07-11 已对照两份 5.6 审查、当前源码、Rust/Microsoft 官方行为和 cap-std 源码完成审查；审查补丁已回填正式方案。  
> 执行规则：严格按 Gate A -> B -> C -> D -> E -> F 串行施工；当前 Gate 失败时先修复，不跨 Gate 堆积问题。

## 1. 施工目标

关闭 CQ-04：

```text
任何由 active project 相对路径驱动的 read/write/remove/rollback/publish，
都不能经 symlink、Windows junction/reparse point、hard link 或并发换链修改项目根外文件。
```

施工完成必须证明：

```text
ProjectSession 持有唯一 project root capability。
普通 authoring caller 不再持有裸 filesystem write authority。
Scene/AI/Patch/Input/Prefab/AUI/Rule/BuildProfile 全部穿过 ProjectWriteScope。
default Build/Preview/Report output 不经项目内 link 逃逸。
ProjectPatch snapshot 不缓存 absolute_path，rollback 重新穿过 scope。
hard-link target 更新不原地 truncate，项目外 inode 内容保持不变。
Linux symlink、Windows junction、hard link 和 concurrent swap 有真实 Gate 证据。
default/all-features workspace 回归通过。
```

## 2. 施工前基线与审查结论

### 2.1 当前状态

```text
241 正式方案已由用户确认并通过方案审查。
施工文档/当前/ 在生成本文前为空。
240 中 CQ-04 讨论状态=已完成，施工状态=未施工。
239 已完成并归档；工作树仍包含其未提交产物和用户改动，施工不得回退。
```

### 2.2 外部审查分类

| 分类 | 项目 | 241 处理方式 |
|---|---|---|
| 必须修改 | CQ-04 Scene、AI image、ProjectPatch symlink/junction escape | Gate A-E 完整关闭 |
| 施工约束 | canonical root、non-existing leaf、link/reparse、TOCTOU、真实 fixture、显式 skip | 写入 Gate A/B/E 阻断条件 |
| 历史吸收 | RuntimePackagePath、atomic file/directory publish、ProjectPatch transaction | 深化或复用，不造第二套真相 |
| 本轮不适用 | CQ-01/CQ-06/CQ-07/CQ-08/INC-02、VFS、sandbox、remote FS、AssetDB | 不扩入 241 |

### 2.3 正式方案审查回填

```text
Safe PathBuf 不能关闭 TOCTOU，固定为目录 capability。
hard link 共享 inode，固定为 atomic replace、禁止原地 truncate。
final symlink/reparse write/remove 统一拒绝。
default Build/Preview/Report 属于 project-owned output。
ExplicitExportOutput 必须 typed，不能由 AI relative path 构造。
ProjectPatch snapshot 只保存 ProjectRelativePath + bytes。
cap-std 必须通过真实 junction/race Gate，不能只引用依赖文档。
GameView/consistency report 已补入 consumer inventory。
```

结论：读取审查后无需再次修改正式方案，可以执行本文档。

## 3. 固定边界

### 3.1 必须交付

```text
ProjectRelativePath strict parser。
ProjectWriteScope root capability lifecycle。
capability-relative read/write_atomic/remove。
capability-relative project-owned directory publish。
ProjectWriteError / ProjectWriteReceipt。
全部 production project-write consumer migration。
real link/junction/hard-link/race validation report。
```

### 3.2 禁止扩入

```text
不新增通用 VFS、mount table、process sandbox、remote filesystem、ACL manager。
不新增第二套 ProjectPatch transaction/validator。
不新增第二套 RuntimePackage assembler/manifest。
不新增每-domain path guard。
不删除 RuntimePackagePath portability contract。
不允许 containment failure 回退到 std::fs。
不处理 CQ-01/CQ-06/CQ-07/CQ-08/INC-02。
不修改 gameplay、AUI runtime、Renderer 或 ECS 产品语义。
```

## 4. 跨 Gate 固定合同

### 4.1 ProjectWriteScope interface

```text
ProjectWriteScope::open(project_root)
scope.read(relative)
scope.write_atomic(relative, bytes)
scope.remove_file(relative)
scope.publish_directory_atomic(relative, builder)
```

普通 caller 不得到 `cap_std::fs::Dir` 或验证后的裸 `PathBuf`。目录创建、temp/backup、sync、rollback、cleanup 属于 implementation。

### 4.2 Relative path

```text
接受规范项目相对路径；入口兼容 `\` -> `/`。
拒绝 empty、absolute、RootDir、Prefix、`.`、`..`、empty segment。
字段私有；display string 不能跳过 parser 反向恢复 authority。
```

### 4.3 Link / hard-link

```text
intermediate link 仅当 capability backend 证明 beneath root 时允许。
escape/unknown reparse fail closed。
final symlink/reparse write/remove 拒绝。
existing regular file 永不原地 truncate；atomic replace 断开 hard-link inode alias。
```

### 4.4 ProjectPatch rollback

```text
snapshot = ProjectRelativePath + existed_before + before_bytes。
restore existing -> scope.write_atomic。
restore new -> scope.remove_file。
containment changed -> project_write.rollback_containment_changed，不 ambient fallback。
```

### 4.5 Explicit external output

```text
default Build/Preview/Report = project-owned，必须使用 ProjectWriteScope/subtree。
user-selected external output = typed ExplicitExportOutput。
ProjectPatch/LLM relative path 不能构造 ExplicitExportOutput。
```

## 5. 预计涉及文件

施工前和 Gate F 都必须复扫 filesystem mutation inventory：

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/editor_core/Cargo.toml
rust/crates/engine_runtime/Cargo.toml

rust/crates/editor_core/src/project_write_scope.rs
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/lib.rs

rust/crates/engine_runtime/src/atomic_file_replace.rs
rust/crates/engine_runtime/src/atomic_directory_publish.rs
rust/crates/engine_runtime/src/runtime_package_path.rs

rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/ai_image_generation.rs
rust/crates/editor_core/src/input_mapping_authoring.rs
rust/crates/editor_core/src/prefab_workflow.rs
rust/crates/editor_core/src/aui_authoring.rs
rust/crates/editor_core/src/aui_template.rs
rust/crates/editor_core/src/rule_authoring.rs
rust/crates/editor_core/src/services/aui_service.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/project_patch/session.rs
rust/crates/editor_core/src/project_consistency.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/editor_preview_package.rs
rust/crates/editor_core/src/desktop_export.rs
rust/crates/editor_core/src/release_package.rs

rust/crates/project_e2e_gate/src/project_write_containment.rs
```

列表外 mutation 命中必须分类为“迁移 / typed project-external / fixture-only”，不能静默忽略。

## 6. Gate A - ProjectWriteScope Foundation

### 6.1 施工

```text
固定 cap-std dependency/version。
实现 ProjectRelativePath parser。
实现 ProjectWriteError / Receipt。
实现 ProjectWriteScope::open/read 基础。
ProjectSession create/open/clone/drop 绑定 capability；不 serde capability。
```

### 6.2 阻断测试

```powershell
cargo test -p editor_core project_write_scope
cargo test -p editor_core project_launcher
```

### 6.3 验收

```text
absolute/parent/dot/empty path 被稳定 code 拒绝。
non-existing leaf 可由 scope 安全定位。
root open failure fail closed。
ProjectSession clone 不 ambient reopen。
```

## 7. Gate B - Capability Atomic File Operations

### 7.1 施工

```text
实现 capability-relative write_atomic/remove。
temp/backup/final rename 和 cleanup 均相对同一 parent capability。
final link/reparse inspection 不跟随。
hard-link update 使用 replace，不修改外部 inode。
继承 atomic_file_replace fault/rollback 语义。
```

### 7.2 阻断测试

```powershell
cargo test -p engine_runtime atomic_file_replace
cargo test -p editor_core project_write_scope
```

### 7.3 验收

```text
create/replace/rollback/cleanup matrix 通过。
hard-link outside sentinel/hash 不变。
final symlink/reparse write/remove 拒绝。
无可恢复的 orphan temp/backup。
```

## 8. Gate C - Authoring / AI / ProjectPatch Migration

### 8.1 施工

```text
迁移 Project launcher child writes。
迁移 Scene、AI image、Input、Prefab、AUI、AUI Template、Rule、BuildProfile。
迁移 ProjectPatch snapshot/rollback。
删除写入用途的 parallel normalize/resolve helper。
domain diagnostic 保留 project_write code/stage/path。
```

### 8.2 阻断测试

```powershell
cargo test -p editor_core scene_editing
cargo test -p editor_core ai_image_generation
cargo test -p editor_core input_mapping
cargo test -p editor_core prefab
cargo test -p editor_core aui
cargo test -p editor_core rule_authoring
cargo test -p editor_core project_patch
```

### 8.3 验收

```text
所有上述 production save/apply/rollback 不再直接 project_root.join + std::fs mutation。
失败保持 dirty/transaction diagnostics。
ProjectPatch rollback 路径变化时安全失败。
```

## 9. Gate D - Project-owned Output Subtrees

### 9.1 施工

```text
迁移 .aife/reports 与 editor-preview。
迁移 GameView/consistency project-owned reports。
迁移 default Build/Windows dev/release。
为 RuntimePackage outer output 建立 capability subtree/publish。
建立 ExplicitExportOutput typed separation。
```

### 9.2 阻断测试

```powershell
cargo test -p editor_core editor_preview_package
cargo test -p editor_core desktop_export
cargo test -p editor_core release_package
cargo test -p editor_core gameview
cargo test -p engine_runtime atomic_directory_publish
cargo test -p engine_runtime runtime_package
```

### 9.3 验收

```text
Build/.aife parent 为 escape link 时不写项目外。
RuntimePackage internal path/inventory/hash 合同不变。
external output 只从 typed trusted constructor 进入。
```

## 10. Gate E - Real Link / Junction / Race Evidence

### 10.1 施工

```text
Linux symlink inside/escape/final fixtures。
Windows junction escape blocking fixture。
Windows symlink optional fixture + explicit skip evidence。
hard-link fixture。
concurrent directory swap stress。
ProjectPatch rollback containment fixture。
validation-only project-write-containment-report.v1。
```

### 10.2 阻断测试

```powershell
cargo test -p editor_core project_write_containment -- --nocapture
cargo test -p project_e2e_gate project_write_containment -- --nocapture
```

### 10.3 验收

```text
outside sentinel/hash 永远不变。
writer 只可能 success-inside 或 structured rejection。
Windows junction 必须运行。
symlink 无权限时 report 明确 explicitly_skipped + OS error + junction coverage。
symlink 与 junction/reparse 均未执行时 Gate 不得 passed。
```

## 11. Gate F - Inventory / Full Regression / Docs

### 11.1 Inventory

```text
复扫 production fs::write/create_dir/rename/remove/copy/atomic_file_replace。
每个命中记录迁移、typed external 或 fixture-only 理由。
241 范围内不得残留 parallel validator 和 ambient fallback。
```

### 11.2 整体回归

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

### 11.3 完成同步

```text
生成 阶段完成记录/2026-07-11-SafeProjectPath-Project-Write-Containment-v1/00-总览.md。
更新 240：CQ-04 施工状态=已完成并记录证据。
更新 00-文档地图、00-阅读顺序、49、54。
更新 施工文档/README.md、阶段完成记录/README.md。
把本文从 当前/ 移到 已完成/。
```

## 12. Diagnostic 最小集合

```text
project_write.root_unavailable
project_write.path_empty
project_write.path_not_relative
project_write.path_parent_component
project_write.path_ambiguous
project_write.symlink_escape
project_write.final_link_rejected
project_write.reparse_unsupported
project_write.capability_denied
project_write.atomic_create_temp_failed
project_write.atomic_commit_failed
project_write.atomic_rollback_failed
project_write.remove_failed
project_write.rollback_containment_changed
project_write.publish_failed
```

## 13. 工作树与安全规则

```text
保留所有既有用户/239 未提交改动。
不 reset/checkout/revert unrelated changes。
生成物只在正式 Gate 要求时更新。
依赖行为不满足方案 C 时暂停并回填 241，不降级为方案 B。
```

## 14. 施工文档自审

### 14.1 与正式方案一致

通过。Gate A-F、interface、consumer、diagnostic、真实 fixture 和完成同步均直接来自 241，没有新增 VFS/sandbox/AssetDB 范围。

### 14.2 审查输入处理

通过。CQ-04 必须修改和 link/TOCTOU/skip 施工约束进入阻断 Gate；RuntimePackage/atomic/ProjectPatch 历史能力只深化；其它 5.6 项明确不适用。

### 14.3 是否形成深 module

通过。普通 caller 只使用 `ProjectWriteScope` 小 interface；parser、capability、atomic lifecycle、link/hard-link、rollback 和 diagnostics 均隐藏在 implementation。

### 14.4 测试是否完整

通过。每个 Gate 有命令和阻断验收；Gate E 使用真实 OS filesystem fixture；Gate F 执行 default/all-features workspace。

### 14.5 是否处理所有项目写入

通过。source asset、ProjectPatch、default Build/Preview/Report 均进入 inventory；external/fixture 有显式分类规则。

### 14.6 是否可以直接施工

可以。正式方案无需修改，当前目录只有本文一份施工文档。自审结论：`通过，开始 Gate A；任一 Gate 失败时停止并修复当前 Gate。`

## 15. 实际施工结果与完成自审

### 15.1 Gate A-F 结果

```text
Gate A：ProjectRelativePath、ProjectWriteScope、ProjectDirectoryWriter、ProjectWriteError/Outcome/Receipt 已落地；ProjectSession 持有并复用 project root capability。
Gate B：read/create/write_atomic/remove 与 capability-relative directory publish 已落地；final link/reparse fail closed；hard-link 更新采用 sibling atomic replace。
Gate C：Project launcher、Scene、AI image、Input、Prefab、AUI、AUI Template、Rule、BuildProfile 与 ProjectPatch snapshot/rollback 已迁移。
Gate D：Preview、GameView、consistency、desktop export、release package 与默认 Build/Preview/Report 已收敛；外部输出使用 ExplicitExportOutput。
Gate E：Windows junction、hard link、16 轮 concurrent junction swap 与 ProjectPatch rollback 换链均真实通过；文件 symlink 因 OS error 1314 显式跳过；全部 outside sentinel 未变化。
Gate F：production mutation inventory 已分类；格式检查、默认 workspace、all-features workspace 均通过。
```

### 15.2 最终验证

```powershell
cargo test -p editor_core project_write_containment -- --nocapture
cargo test -p project_e2e_gate project_write_containment -- --nocapture
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

结果：全部通过。Windows 文件 symlink fixture 因“客户端没有所需的特权 (os error 1314)”记录为 `explicitly_skipped`；Windows 必需的 junction/reparse coverage 已执行并通过，聚合 `project-write-containment-report.v1` 状态为 `passed`。

### 15.3 完成自审

```text
范围：没有扩入 VFS、sandbox、remote filesystem、AssetDB、CQ-01/CQ-06/CQ-07/CQ-08/INC-02。
权限：普通 authoring caller 不获得 cap_std::fs::Dir；ProjectPatch snapshot 不保存绝对路径；外部输出必须显式 typed authorization。
竞态：真实 junction/hard-link/concurrent swap/rollback swap evidence 的项目外 sentinel 全部不变。
回归：default 与 all-features workspace 均通过；仅保留 engine_runtime::project_logic 既有 dead-code warning。
文档：完成记录、优先级、入口、地图、阅读顺序与归档位置已同步。
```

最终结论：`CQ-04 已关闭；241 施工完成并归档。下一项按 240 进入 CQ-01 方案讨论，本施工不启动 CQ-01。`
