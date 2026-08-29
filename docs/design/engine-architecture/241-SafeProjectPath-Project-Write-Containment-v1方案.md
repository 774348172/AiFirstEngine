# 241-SafeProjectPath / Project Write Containment v1 方案

> 状态：正式方案已确认并完成方案审查，尚未施工。  
> 建立日期：2026-07-11  
> 选题来源：`240-5.6审查剩余问题讨论与施工优先级.md` Priority 1 / CQ-04。  
> 审查输入：`审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md`、`01-2026-07-11-新增功能增量代码质量审查报告.md`。  
> 用户确认：采用方案 C，以 capability-backed `ProjectWriteScope` 统一项目写入权限。  
> 目标：任何由项目相对路径驱动的创建、替换、删除、rollback、预览和默认构建输出，都不能经 symlink、Windows junction/reparse point、hard link 或路径竞态修改项目根外的文件。

## 1. 这个系统是干什么的

CQ-04 不是新增文件浏览器、VFS、进程沙箱或 AssetDB。它只建立一个工程级安全事实：

```text
编辑器打开一个项目
  -> 获得该项目根目录的一份写 capability
  -> 所有项目相对写入只能穿过这份 capability
  -> 项目中的路径文本不能自行扩大写权限
```

例如：

```text
project root = <PROJECTS_ROOT>/Game
<PROJECTS_ROOT>/Game/Assets/Generated -> <OUTSIDE_ROOT>   // symlink 或 junction
```

当前 lexical `starts_with(project_root)` 仍会认为 `Assets/Generated/image.png` 位于项目内，操作系统却会把写入重定向到 `<OUTSIDE_ROOT>/image.png`。

241 的作用是把“项目根”从一个可随意拼接的 `PathBuf`，升级为一份由打开目录句柄承载、只能执行受限相对操作的写权限。

## 2. 为什么现在必须做

5.6 审查已确认以下正式链路仍可能越过项目根：

```text
AI image generation
  lexical normalize + starts_with
  -> create_dir_all
  -> write png / metadata

Scene save
  lexical normalize + starts_with
  -> atomic_file_replace

ProjectPatch rollback
  reject absolute / .. only
  -> create_dir_all / fs::write / remove_file
```

影响不仅是单次保存错误：

```text
AI / ProjectPatch 的“只能修改当前项目”合同可能失效。
恶意项目可以借 junction 把自动生成内容写到用户其它目录。
rollback 可能在错误路径恢复或删除文件。
默认 Build / Preview / Report 目录也可能经项目内链接落到项目外。
```

这属于 P1 数据安全 seam，必须在 CQ-01 RuntimeModule 大范围迁移前关闭，避免后续复制不安全写入入口。

## 3. 当前代码基线

### 3.1 已有正确参考，但不是完整答案

`engine_runtime/src/runtime_package_path.rs` 已有：

```text
RuntimePackagePath::parse
safe_join_runtime_package
canonical root
nearest existing ancestor
symlink escape rejection
```

它可以拒绝已经存在的静态 escape，但返回的仍是裸 `PathBuf`。检查完成后到实际 `create/write/rename/remove` 之间，目录仍可能被并发替换，所以它不能直接作为方案 C 的完整实现。

RuntimePackage path 还包含 Windows 保留名、package portability 和 collision 规则；这些规则不能机械等同于项目写权限规则。

### 3.2 当前原子文件替换

`engine_runtime/src/atomic_file_replace.rs` 已集中：

```text
create parent
create sibling temp
write + flush + sync
stage existing to backup
rename temp to target
rollback / cleanup
```

它的事务语义应保留，但当前实现接收裸绝对路径，并使用 `exists/create_dir_all/rename/remove_file`。项目写入不得继续直接调用这个 path-based interface。

### 3.3 当前 ProjectSession

`editor_core/src/project_launcher.rs` 的 `ProjectSession` 当前只保存：

```text
project_root: PathBuf
manifest: ProjectManifest
```

ProjectSession 是最自然的 capability 生命周期 owner：项目成功 create/open 时建立，项目关闭或切换时释放。capability 不进入 serde、report 或 RuntimePackage。

### 3.4 当前路径 normalizer 分散

当前至少存在以下重复做法：

```text
services/project_service.rs::normalize_project_relative_path
input_mapping_authoring.rs::normalize_project_relative_path
rule_authoring.rs::normalize_project_relative_path
prefab_workflow.rs::normalize_project_relative_path
ai_image_generation.rs::resolve_project_path
scene_editing.rs::normalize_path + starts_with
project_patch/session.rs::resolve_project_file_snapshot_path
```

其中 Prefab normalizer 会过滤 `..` 而不是拒绝输入。241 必须用一个严格 parser 取代这些写入前 normalizer，不能在旧 helper 上再叠一层 validator。

## 4. 5.6 审查结论分类

### 4.1 必须修改

```text
CQ-04 Scene、AI image、ProjectPatch 写入可经 symlink/junction 逃逸。
所有其它由 active project root 派生的生产写入必须同步复扫，不能只修审查点名的三个文件。
```

### 4.2 施工约束

```text
canonical root、non-existing leaf、symlink、junction/reparse point 必须有明确规则。
创建目录、原子替换、删除、rollback 必须穿过同一 interface。
TOCTOU 不能只用“操作前再 canonicalize 一次”口头关闭。
真实 link fixture 无权限时必须输出显式 skip evidence。
diagnostics 必须 fail closed，并包含 code/stage/path/operation/next action。
```

### 4.3 已由历史施工吸收

```text
236 已建立稳定 JSON、共享原子保存和 save/reload/rebuild 验证基线。
237/239 已建立 atomic directory publish、稳定 publish lock 和 release 验证。
RuntimePackagePath 已有静态 relative path 与 existing ancestor 参考实现。
ProjectPatch 已有 transaction、snapshot 和 rollback 产品链。
```

241 深化这些 module，不新增第二套 ProjectPatch transaction、RuntimePackage assembler、atomic publisher 或 report 真相。

### 4.4 本轮不适用

```text
通用 VFS。
进程级 sandbox。
远程文件系统。
AssetDB 重写。
文件系统 ACL 管理。
任意用户选择目录的全局权限管理器。
CQ-01/CQ-06/CQ-07/CQ-08/INC-02。
```

## 5. 成熟实现与可借鉴点

### 5.1 Rust std::fs::canonicalize

官方文档：

```text
https://doc.rust-lang.org/std/fs/fn.canonicalize.html
```

关键事实：

```text
canonicalize 返回绝对 canonical path，并解析中间 symbolic links。
Unix 当前对应 realpath。
Windows 当前使用 CreateFile + GetFinalPathNameByHandle。
目标不存在时 canonicalize 失败。
```

可学习：canonical root 和 existing ancestor 可以证明静态路径当前解析到哪里。  
不照搬：不能把 canonicalize 后的 `PathBuf` 当成持续有效的写 capability；它没有把检查和使用变成一个原子权限操作。

### 5.2 Windows reparse point

官方文档：

```text
https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points
https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points-and-file-operations
```

关键事实：

```text
NTFS symbolic link、junction/mounted folder 都由 reparse point 实现。
reparse data 可以改变普通文件打开行为。
需要操作 reparse point 本身时，CreateFile 必须显式使用 FILE_FLAG_OPEN_REPARSE_POINT。
reparse tag 可以由文件系统 filter 定义，未知行为不能默认放行。
```

可学习：Windows 不能只测试 `symlink_dir`；junction/reparse point 是独立 blocking fixture。  
不照搬：本项目不直接散落 Win32 FFI 或按 tag 建立第二套 Windows validator。

### 5.3 Bytecode Alliance cap-std

源码与说明：

```text
https://github.com/bytecodealliance/cap-std
cap-std/src/fs/dir.rs
```

关键做法：

```text
Dir 代表一个已经打开的目录，而不是一个字符串 root。
open/create_dir/create_dir_all/rename/remove_file 都相对 Dir 执行。
绝对路径、.. 和逃逸 symlink 返回 PermissionDenied。
Linux 5.6+ 常见路径使用 openat2 / RESOLVE_BENEATH。
其它平台逐组件打开并持有目录句柄。
支持 Windows；Dir source 明确要求目录 handle 的 share mode 不能制造删除竞态。
cap-std 是 Wasmtime/WASI 文件系统 capability 基础之一。
```

可学习：目录句柄才是权限；调用方只得到相对操作，不得到 ambient authority。  
不照搬：不把 cap-std 的全部 std-like surface 暴露给项目 authoring caller，也不让 caller 直接取得底层 `Dir`。

## 6. 候选方案与正式选择

### 6.1 方案 A：共享 canonical safe join

```text
safe_join_project(root, relative) -> PathBuf
```

优点：修改量小，可拒绝静态 symlink/junction escape。  
缺点：返回裸路径；检查与写入分离；atomic replace、rollback、Build 输出仍有 TOCTOU。  
结论：不采用。

### 6.2 方案 B：ProjectWriteScope + std path-based recheck

```text
每个操作前 canonicalize nearest existing ancestor
-> 再调用 std::fs
```

优点：不增加依赖，逻辑集中。  
缺点：仍是 check-then-use；无法诚实关闭并发换链；P1 安全合同只达到静态项目防护。  
结论：不采用。

### 6.3 方案 C：capability-backed ProjectWriteScope

```text
ProjectSession owns ProjectWriteScope
ProjectWriteScope owns an opened root directory capability
all mutations use normalized relative paths and capability-relative operations
```

优点：写权限与目录句柄绑定；一个深 module 覆盖 parse、resolve、create、atomic replace、remove、rollback、subtree publish 和 diagnostics；生产 caller 与测试穿过同一 interface。  
缺点：增加经审核的 `cap-std` 依赖；必须深化原子文件/目录操作并迁移真实写入 caller。  
结论：用户已确认，正式采用。

## 7. 正式架构链

```text
Create/Open Project
  -> establish/canonicalize selected project root
  -> open capability-backed ProjectWriteScope
  -> attach scope lifetime to active ProjectSession

Authoring / AI / Patch / Build request
  -> parse ProjectRelativePath
  -> ProjectWriteScope operation
       -> capability-relative ancestor traversal
       -> fail-closed symlink/junction/reparse policy
       -> capability-relative atomic write/remove/publish
       -> structured ProjectWriteResult / ProjectWriteError
  -> existing domain report / transaction result

Project close/switch
  -> drop ProjectSession
  -> close project root capability
```

禁止：

```text
项目写入 caller 自行 project_root.join(relative) 后调用 fs::write。
项目写入 caller 直接调用 path-based atomic_file_replace。
返回一个“已经验证”的 PathBuf 并让 caller 稍后写入。
旧 normalizer 与新 validator 并存。
用 starts_with、字符串 prefix 或删除 `..` 片段证明 containment。
capability 检查失败后回退到 std::fs。
把 cap_std::fs::Dir 暴露给普通 authoring caller。
```

## 8. Module、interface 与 seam

### 8.1 外部 seam

概念 owner：

```text
editor_core::project_write_scope
```

生命周期 owner：

```text
ProjectSession
```

建议 interface：

```rust
ProjectWriteScope::open(project_root) -> Result<ProjectWriteScope, ProjectWriteError>

scope.read(relative_path) -> Result<Vec<u8>, ProjectWriteError>
scope.write_atomic(relative_path, bytes) -> Result<ProjectWriteReceipt, ProjectWriteError>
scope.remove_file(relative_path) -> Result<ProjectWriteReceipt, ProjectWriteError>
scope.publish_directory_atomic(relative_path, build) -> Result<ProjectWriteReceipt, ProjectWriteError>
```

目录创建、temp/backup name、sync、rollback 和 cleanup 是 implementation，不应要求每个 caller 编排。

对 RuntimePackage/Preview/Build orchestration，可提供 crate-private `ProjectWriteSubtree`，但它只能由 `ProjectWriteScope` 产生，不能从任意 `PathBuf` 构造，也不能进入普通 authoring interface。

### 8.2 内部 capability implementation

实现可深化现有：

```text
engine_runtime::atomic_file_replace
engine_runtime::atomic_directory_publish
```

增加 capability-relative 内部入口，保留现有通用 path-based 入口给明确不属于 active project 的调用方。项目写入只能使用 capability-relative入口。

`cap-std` 版本在施工时固定到 `Cargo.lock`，必须复核 license、Windows feature 和所选版本的真实 junction/race 测试。若所选版本不能满足 Windows containment，不允许降级到方案 B；必须暂停并修改本正式方案。

### 8.3 ProjectSession 持有规则

```text
ProjectSession 中 capability 不参与 Serialize/Deserialize。
若当前 serde/PartialEq derive 没有真实消费者，可删除无用 derive。
若必须保留序列化，只序列化 project_root/manifest，不序列化或反序列化 capability。
反序列化数据不能自行恢复写权限；只有 create/open project 流程可以建立 scope。
Clone 必须共享或安全 duplicate 已打开的目录 capability，不能重新按路径 ambient-open。
```

## 9. ProjectRelativePath 合同

`ProjectRelativePath` 是 `ProjectWriteScope` 内的严格 value type，字段私有。

输入规则：

```text
允许项目 schema 使用的 `/` 路径；兼容入口可先把 `\` 规范为 `/`。
拒绝空路径。
拒绝 absolute、RootDir、Windows Prefix。
拒绝 `.`、`..`、空 segment。
拒绝规范化后语义发生折叠的输入。
保留 Unicode 和普通项目文件名；package portability 规则仍由 RuntimePackagePath 负责。
```

安全规则：

```text
相对路径只是 capability 内的定位信息，不是权限本身。
不得从 display string 反向恢复 ProjectRelativePath 而跳过 parser。
所有 error/report 使用规范化相对路径；绝不把项目外最终解析路径作为成功结果暴露。
```

现有 `normalize_project_relative_path`、`resolve_project_path` 和 snapshot resolver 的写入用途必须被删除或迁移，不能保留 parallel validator。

## 10. Root、non-existing leaf 与 link 规则

### 10.1 Root

```text
create project：用户选择的 ambient root 是一次明确外部授权；先创建 root 本身，再打开 ProjectWriteScope，所有 child 写入改走 scope。
open project：先打开 ProjectWriteScope，再更新 lastOpenedAt 或其它项目文件。
root 本身可以是用户明确选择的 symlink，但 capability 绑定其打开后的真实目录身份；diagnostics 同时保留 display root 与 canonical identity evidence。
root 不存在、不是目录或无法打开时 fail closed。
```

### 10.2 Non-existing leaf

```text
不存在 leaf 不做 `canonicalize(candidate)`。
由 capability 从 root 开始逐组件解析/创建 parent。
任何已存在 parent 都必须仍在 capability 下。
最终 leaf 只能由 scope 的 create-new / atomic replace / publish 操作创建。
```

### 10.3 Symlink / junction / reparse point

```text
intermediate symlink/junction 只有在 capability backend 证明解析后仍 beneath root 时才允许。
逃逸、absolute target、无法解析或未知 reparse 行为一律 fail closed。
最终目标如果是 symlink/reparse point，写入和删除默认拒绝；v1 不隐式跟随，也不隐式替换链接本身。
```

最后一条避免“用户以为保存文档，实际改写链接目标”或“删除链接还是删除目标”的跨平台歧义。未来如确需 link-management，必须增加显式 operation，不复用普通 write/remove。

### 10.4 Hard link

目录 capability 不能证明一个普通文件 inode 是否还有项目外 hard link。因此：

```text
项目文件更新禁止原地 truncate/write 已存在目标。
write_atomic 必须写新 sibling temp，再 rename/replace target。
这样替换项目目录项，不会修改项目外 hard link 指向的旧 inode。
remove_file 只删除项目内目录项，不修改其它 hard link 内容。
```

真实 hard-link fixture 是 241 的 blocking test，不能只测试 symlink。

## 11. 原子替换、删除与 rollback

### 11.1 Atomic write

固定流程：

```text
parse relative path
-> open/create parent through capability
-> create-new sibling temp through parent capability
-> write_all + flush + sync_all
-> inspect final entry without following final symlink/reparse
-> if regular file: rename to sibling backup
-> rename temp to final
-> sync parent when platform/backend supports
-> remove backup through capability
```

所有 temp/backup cleanup 也必须相对同一 parent capability，不使用重新拼接的绝对路径。

### 11.2 Remove

```text
remove_file 只接受 ProjectRelativePath。
不存在目标按 operation policy 返回 not_found 或 idempotent receipt，不能由 caller 自行 exists + remove。
目录删除不进入普通 remove_file；需要时由 directory publish/cleanup implementation 私有处理。
最终 symlink/reparse 默认拒绝。
```

### 11.3 ProjectPatch snapshot / rollback

`ProjectFileSnapshot` 改为保存：

```text
relative_path: ProjectRelativePath
existed_before
before_bytes
```

禁止缓存可长期使用的 `absolute_path`。

rollback：

```text
existed_before
  -> scope.write_atomic(relative_path, before_bytes)

did_not_exist_before
  -> scope.remove_file(relative_path)
```

如果 transaction 期间路径被改成逃逸 link/reparse point，rollback 必须拒绝该磁盘操作并输出 `project_write.rollback_containment_changed`。安全失败优先于强行恢复；内存 transaction 恢复和文件 rollback failure 都必须进入最终 Patch diagnostics。

## 12. 真实 production consumer 清单

生成施工文档前必须再用 filesystem mutation scan 复核，当前至少包括：

### 12.1 项目源文件

```text
project_launcher.rs
  project.aife.json
  Settings/project_settings.json
  default Scene
  open project lastOpenedAt update

scene_editing.rs
  Scene save / autosave

ai_image_generation.rs
  generated PNG
  generated .ai.json metadata

input_mapping_authoring.rs
  Input Mapping save / draft commit

prefab_workflow.rs
  Prefab create/save/apply

aui_authoring.rs + aui_template.rs + services/aui_service.rs
  AUI Document
  AUI Template

rule_authoring.rs + engine_runtime/project_rule_asset.rs
  Rule Asset

services/build_service.rs
  BuildProfiles/windows.release.json

project_patch/session.rs
  snapshot read
  rollback write/remove
```

Rule/AUI/Prefab 等 module 负责 validation/serialization；它们不能继续自己拥有 filesystem write authority。内容 bytes 交给 `ProjectWriteScope`，避免把 `engine_runtime::write_project_rule_asset_json(path)` 继续作为 Editor production 写入口。

### 12.2 项目拥有的生成物

```text
.aife/reports/release-package/latest.json
.aife/editor-preview/**
Build/Windows/dev/**
Build/Windows/<arch>/<profile>/**
desktop export / build-and-run child and verification reports
GameView present report（位于项目拥有的 Preview RuntimePackage report tree 时）
save/reload/rebuild consistency report（report path 位于 active project 时）
默认 RuntimePackage output subtree
```

这些路径虽然不是 source asset，仍由 active project root 派生，必须先获得 `ProjectWriteSubtree`。RuntimePackage 内部仍保留 `RuntimePackagePath` 的 portability/inventory 合同；ProjectWriteScope 负责外层项目 containment，两者职责不同，不互相替代。

### 12.3 明确排除的写入

```text
ProjectRecentStore：属于 Editor 用户配置，不属于 active project。
editable_project_loop::create_default_editable_project_fixture：显式 temp fixture 生成器，不是 active project production write owner。
测试 fixture / temp directory：由测试自身授权，不伪装成 ProjectWriteScope production consumer。
用户在导出 UI 明确选择的项目外 output root：属于 ExplicitExportOutput，不受“必须在项目根”规则限制。
```

`ExplicitExportOutput` 必须是 typed trusted input：

```text
不能从 ProjectPatch/LLM 的普通 relative path 字段构造。
默认 build/release 不自动获得项目外权限。
现有 output_dir override 若只用于 test/internal，施工时收窄可见性；若有真实 UI caller，则建立明确用户授权证据。
继续使用既有 atomic directory publisher 和独立 output-root 验证，不扩成通用 VFS。
```

## 13. TOCTOU 正式规则

方案 C 的完成条件不是“多检查一次 canonical path”，而是：

```text
权限根由已打开目录 handle 表示。
路径遍历和最终 mutation 相对 handle 执行。
中间目录按 capability backend 的 beneath-root 规则打开。
atomic temp/backup/final rename 使用同一 parent capability。
Windows directory handle/share mode 不允许攻击者在检查与使用间替换已打开 parent。
```

并发攻击 fixture：

```text
writer 循环 write_atomic(project/link/file)
attacker 循环在 inside directory 与 outside junction/symlink 之间交换 link
outside sentinel/hash 永远不变
每次 writer 只能成功写项目内文件或返回 structured containment error
```

如果 dependency/backend 在某平台只能提供 check-then-use，方案审查结论被推翻，必须回到“讨论中”；不允许把测试降级成静态 symlink 后仍宣称关闭 TOCTOU。

## 14. Diagnostics 与 receipt

建议：

```text
ProjectWriteError
  code
  operation: open_root | read | write_atomic | remove | publish | rollback
  stage
  project_relative_path
  display_root
  source_kind: invalid_path | io | symlink | reparse | containment | race
  message
  next_action

ProjectWriteReceipt
  operation
  project_relative_path
  outcome: created | replaced | removed | published | unchanged
  bytes_written
```

稳定 code 至少包括：

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

调用方把 error 映射到既有 domain diagnostic code 时，必须保留 `project_write` code/stage/path 作为 source evidence，不能只留下自由文本。

241 不新增常驻 Runtime report。Editor 继续复用现有 Scene/Patch/Build/AI diagnostics；`project_e2e_gate` 可生成 validation-only `project-write-containment-report.v1`。

## 15. 测试矩阵

测试必须穿过生产 `ProjectWriteScope` interface，不直接测试私有 validator 后再用 `std::fs` 模拟成功。

### 15.1 Relative path

```text
normal nested path succeeds
absolute / rooted / Windows prefix rejected
../, ./, empty segment rejected
backslash compatibility produces one canonical relative representation
non-existing parent + leaf created inside root
```

### 15.2 Link/reparse

```text
intermediate internal symlink remains inside root
intermediate symlink escape rejected
final symlink write/remove rejected
Windows directory junction escape rejected
unsupported reparse point rejected when fixture available
root selected through symlink binds opened real directory identity
```

### 15.3 Hard link

```text
outside file has project hard link
write_atomic(project_link) succeeds by replacing directory entry
outside file bytes/hash remain unchanged
remove_file(project_link) does not remove or modify outside file
```

### 15.4 Atomic lifecycle

```text
new write
replace existing regular file
create temp failure
write/sync failure
stage existing failure
commit failure + rollback
cleanup failure evidence
no orphan temp/backup after recoverable failures
```

### 15.5 ProjectPatch

```text
snapshot rejects escape path before apply
rollback existing file uses atomic replace
rollback new file uses safe remove
path changed to junction between capture and rollback -> fail closed diagnostic
AI image/Prefab/AUI/Rule/Input operation failure cannot rollback outside root
```

### 15.6 Output subtree

```text
.aife/editor-preview link escape rejected
Build/Windows link escape rejected
release report parent link escape rejected
RuntimePackage internal safe paths still pass
explicit external export only succeeds through typed explicit authorization
```

### 15.7 Race

```text
directory swap stress on Linux symlink
directory swap stress on Windows junction/reparse
outside sentinel and hash unchanged
writer outcome only success-inside or structured rejection
```

### 15.8 Skip evidence

```text
Linux symlink fixture 默认必须运行。
Windows junction fixture 默认必须运行；它不应依赖 symlink developer mode。
Windows symlink privilege 不可用时，report 必须记录 explicitly_skipped + OS error + junction coverage result。
若平台上 symlink 与 junction/reparse fixture 都未执行，CQ-04 Gate 不得报告 passed。
```

## 16. 预期涉及文件

生成施工文档前必须复扫，当前预计：

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/editor_core/Cargo.toml
rust/crates/engine_runtime/Cargo.toml                  // 若 capability atomic primitive 落在现有 module

rust/crates/editor_core/src/project_write_scope.rs
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/lib.rs

rust/crates/engine_runtime/src/atomic_file_replace.rs
rust/crates/engine_runtime/src/atomic_directory_publish.rs
rust/crates/engine_runtime/src/runtime_package_path.rs   // 只复用/去重实现，不改变 package schema

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

实际 mutation scan 可能发现更多 production caller；施工文档必须记录纳入/排除理由，不能以本列表没有列出为由跳过。

## 17. 推荐施工 Gate

### Gate A：ProjectWriteScope / Capability Foundation

施工：

```text
固定 cap-std dependency。
ProjectRelativePath strict parser。
ProjectWriteScope root capability lifecycle。
structured error/receipt。
ProjectSession create/open/close ownership。
```

测试：

```powershell
cargo test -p editor_core project_write_scope
cargo test -p editor_core project_launcher
```

### Gate B：Capability-relative Atomic File Operations

施工：

```text
write_atomic/remove/read。
temp/backup/rename/rollback 全部 relative to capability。
final link/reparse rejection。
hard-link-safe replace。
existing atomic_file_replace fault matrix 迁移/继承。
```

测试：

```powershell
cargo test -p engine_runtime atomic_file_replace
cargo test -p editor_core project_write_scope
```

### Gate C：Authoring / AI / ProjectPatch Consumers

施工：

```text
Scene、AI image、Input、Prefab、AUI、AUI Template、Rule、BuildProfile migration。
ProjectPatch snapshot/rollback relative-path migration。
删除写入用途的 parallel normalizer/validator。
domain diagnostics 保留 project_write evidence。
```

测试：

```powershell
cargo test -p editor_core scene_editing
cargo test -p editor_core ai_image_generation
cargo test -p editor_core input_mapping
cargo test -p editor_core prefab
cargo test -p editor_core aui
cargo test -p editor_core rule_authoring
cargo test -p editor_core project_patch
```

### Gate D：Project-owned Output Subtrees

施工：

```text
.aife report/preview migration。
default Build/Windows dev/release migration。
RuntimePackage outer project subtree capability。
ExplicitExportOutput typed separation。
capability-relative directory publish/cleanup。
```

测试：

```powershell
cargo test -p editor_core editor_preview_package
cargo test -p editor_core desktop_export
cargo test -p editor_core release_package
cargo test -p engine_runtime atomic_directory_publish
cargo test -p engine_runtime runtime_package
```

### Gate E：Real Link / Junction / Race / E2E

施工：

```text
Linux symlink fixtures。
Windows symlink + junction/reparse fixtures。
hard-link fixture。
concurrent directory-swap stress。
ProjectPatch rollback containment fixture。
project-write-containment-report.v1。
```

测试：

```powershell
cargo test -p project_e2e_gate project_write_containment -- --nocapture
cargo test -p editor_core project_write_containment -- --nocapture
```

### Gate F：Regression / Docs / Closure

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

完成后同步：

```text
240 CQ-04 施工状态。
49 / 54 / 00 文档入口。
施工文档 README。
阶段完成记录 README。
5.6 CQ-04 关闭证据。
```

严格 Clippy/Hygiene 历史债务仍由 CQ-08/CQ-07 处理；241 不得新增 warning。

## 18. 本轮明确不做

```text
不建立通用 VFS mount table。
不建立进程 sandbox 或 restricted token。
不实现远程/cloud filesystem。
不重写 AssetDB。
不为每个 authoring domain 建一个 path guard。
不允许 ProjectPatch/LLM 获得 arbitrary external output capability。
不把 RuntimePackagePath 删除并全部替换成 ProjectRelativePath。
不处理用户主动在项目外打开普通文件的通用编辑器功能。
不顺带执行 CQ-01/CQ-06/CQ-07/CQ-08/INC-02。
```

## 19. 风险与控制

### 风险 1：把 capability wrapper 做成浅转发层

控制：`ProjectWriteScope` 必须隐藏 parser、root handle、link policy、atomic replace、remove、publish、rollback safety 和 diagnostics；caller 不再自己编排 filesystem lifecycle。

### 风险 2：cap-std 依赖被当成无需测试的安全证明

控制：真实 Windows junction、Linux symlink、hard-link 和 race fixture 是 blocking Gate；依赖行为不符合即回改方案，不降级宣传。

### 风险 3：只迁移审查点名的三个 consumer

控制：施工前和 Gate F 都运行 production filesystem mutation inventory；每个命中项记录“迁移/项目外明确授权/测试专用”分类。

### 风险 4：Build/Preview 仍先拿安全 PathBuf 再裸写

控制：项目拥有的生成 subtree 必须持有 capability 或通过 capability-relative publisher；只验证 output root 后交给 path-based builder 不算完成 Gate D。

### 风险 5：原子写入破坏现有 stale-source hash 或 report

控制：保持现有 domain validation、serialization、source hash 和 report schema；只替换最终 I/O seam，receipt/evidence 映射到既有 diagnostics。

### 风险 6：rollback 被 fail-closed 后无法完整恢复

控制：安全优先；内存恢复照常，磁盘 rollback failure 明确进入 Patch result，并保留相对路径和 next action，不尝试 ambient fallback。

### 风险 7：项目创建本身需要项目外写权限

控制：只把用户明确选择并创建 root 视为一次 ambient authority；root 建立后立即切换到 ProjectWriteScope，所有 child 写入受限。

### 风险 8：internal symlink 行为跨平台不一致

控制：intermediate link 只有 capability backend 证明 beneath root 才允许；final link mutation v1 统一拒绝，避免 follow/replace 语义分叉。

## 20. 方案自审

### 20.1 是否符合用户确认

是。正式采用用户确认的方案 C：capability-backed `ProjectWriteScope`，没有改回 canonical safe join。

### 20.2 是否形成深 module

是。

```text
caller 只知道 relative path + read/write/remove/publish interface。
parser、handle、link/reparse、atomic lifecycle、hard-link、rollback 和 diagnostics 都在 implementation 内。
删除 ProjectWriteScope 后，这些复杂性会重新散回 Scene/AI/Patch/AUI/Prefab/Rule/Build 等 caller。
```

它具备真实 Depth、Leverage 和 Locality，不需要为单一 implementation 增加虚构 port。

### 20.3 interface 是否过大

否。四个 operation 覆盖真实不同 mutation 语义：读取 snapshot、原子文件写、文件删除、原子目录发布。目录创建是 implementation，不单独暴露给普通 caller。

### 20.4 是否完整覆盖 CQ-04

是。方案明确处理：

```text
canonical/root capability。
non-existing leaf。
symlink。
Windows junction/reparse point。
hard link。
create/atomic replace/remove/rollback。
TOCTOU。
真实 production consumer inventory。
structured fail-closed diagnostics。
真实 fixture 与 explicit skip evidence。
```

### 20.5 是否保持既有工程真相

是。

```text
ProjectPatch transaction/Review/Confirm 不变。
RuntimePackage assembler/builder/manifest/hash 不变。
atomic save/publish 的事务语义保留并深化。
Scene/Prefab/AUI/Rule/Input serialization 与 validation 不变。
```

### 20.6 是否避免扩大范围

是。没有新增通用 VFS、sandbox、remote filesystem、AssetDB、ACL manager 或第二套 output orchestrator。

### 20.7 是否可以生成施工文档

可以。范围、interface、consumer、diagnostics、负面矩阵、Gate 和整体回归已经固定。下一步仍必须：

```text
读取针对 241 的新增审查
-> 复扫当前 mutation inventory
-> 生成唯一 241 当前施工文档
-> 做施工文档自审
-> 自审通过后才开始 Gate A
```

本方案完成不代表已经施工。

## 21. 2026-07-11 正式方案审查结论

审查对象：

```text
两份 5.6 审查报告。
240 CQ-04 范围与状态规则。
runtime_package_path / atomic_file_replace 当前实现。
Scene / AI image / ProjectPatch 当前问题链。
Input / Prefab / AUI / Rule / BuildProfile 其它项目写入 caller。
default Build / Preview / Report 项目拥有输出。
Rust canonicalize、Microsoft reparse point、cap-std Dir 官方/源码依据。
```

审查中补齐的关键点：

```text
只返回 Safe PathBuf 不能关闭 TOCTOU，正式方案固定为目录 capability。
hard link 不会路径逃逸但会共享 inode，正式方案强制 atomic replace、禁止原地 truncate。
final symlink/reparse mutation 统一拒绝，避免 follow/replace 跨平台歧义。
默认 Build/Preview/Report 属于项目写入，不能只迁移 source asset。
GameView/consistency report 在项目拥有输出树内时也必须穿过同一 capability seam。
显式项目外导出必须 typed authorization，不能由 AI relative path 获得。
ProjectPatch snapshot 不再缓存 absolute_path，rollback 重新穿过 scope。
cap-std 行为必须由真实 junction/race Gate 证明，不能只引用依赖说明。
```

审查未发现以下问题：

```text
没有第二套 ProjectPatch validator/transaction。
没有第二套 RuntimePackage assembler/manifest。
没有通用 VFS 或 sandbox 扩张。
没有让 engine_runtime 持有 active project lifecycle。
没有以静态 canonicalization 冒充 race-safe containment。
```

结论：`通过，可以生成唯一 241 当前施工文档；当前仍为未施工。`

## 22. 正式结论

正式采用：

```text
方案 C：capability-backed ProjectWriteScope

ProjectSession-owned root capability
  + strict ProjectRelativePath
  + capability-relative atomic file replace
  + capability-relative remove/rollback
  + capability-relative project output publish
  + typed ExplicitExportOutput separation
  + real symlink/junction/hard-link/race Gate
  + structured fail-closed diagnostics
```

完成标准：

```text
任何 active-project relative write 都不能修改 project root 外文件。
Scene/AI/Patch/Input/Prefab/AUI/Rule/BuildProfile 不再持有裸 filesystem write authority。
default Build/Preview/Report output 不能经 project link 逃逸。
ProjectPatch rollback 安全状态变化时 fail closed，不 ambient fallback。
Windows junction、Linux symlink、hard link、concurrent swap 全部由真实 Gate 证明。
default/all-features workspace 回归通过。
```

## 23. 后续优先级

241 讨论完成后，5.6 讨论队列的下一个项目是：

```text
Priority 2：CQ-01 Project RuntimeModule / Generic Runtime Decoupling + Second Project Gate v1
```

CQ-04 尚未施工；是否生成并执行 241 施工文档，仍以用户后续指令、`54` 和 `施工文档/当前/` 为准。讨论队列前进不等于跳过 CQ-04 施工。

## 24. 参考

```text
240-5.6审查剩余问题讨论与施工优先级.md
239-Critical-Correctness-and-Safety-Convergence-Gate-v1方案.md
236-Save-Reload-Rebuild-Consistency-Gate-v1方案.md

审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md
审查目录/5.6审查目录/01-2026-07-11-新增功能增量代码质量审查报告.md

rust/crates/engine_runtime/src/runtime_package_path.rs
rust/crates/engine_runtime/src/atomic_file_replace.rs
rust/crates/engine_runtime/src/atomic_directory_publish.rs
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/ai_image_generation.rs
rust/crates/editor_core/src/project_patch/session.rs
rust/crates/editor_core/src/input_mapping_authoring.rs
rust/crates/editor_core/src/prefab_workflow.rs
rust/crates/editor_core/src/aui_authoring.rs
rust/crates/editor_core/src/aui_template.rs
rust/crates/editor_core/src/rule_authoring.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/editor_preview_package.rs
rust/crates/editor_core/src/desktop_export.rs
rust/crates/editor_core/src/release_package.rs

https://doc.rust-lang.org/std/fs/fn.canonicalize.html
https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points
https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points-and-file-operations
https://github.com/bytecodealliance/cap-std
https://github.com/bytecodealliance/cap-std/blob/main/cap-std/src/fs/dir.rs
```
