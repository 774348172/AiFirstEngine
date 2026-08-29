# 239-Critical Correctness and Safety Convergence Gate v1 方案

> 状态：已完成（用户已确认 B-min+；2026-07-11 Gate A-E、整体回归、完成记录与归档全部闭环）。  
> 选题来源：`审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md` 与 `01-2026-07-11-新增功能增量代码质量审查报告.md`。  
> 前置系统：`237 Release Package Polish / Metadata / Icon / Layout v1`、`238 Real LLM Provider / Minimal Repair Loop v1` 已完成并归档。  
> 目标：阻止 ProjectPatch repair、导出玩家进程验证、原子目录发布和 Windows release PE 验证中的确定性错误继续被报告为成功。  
> 用户确认：采用 B-min+，以三个深模块和一个稳定发布锁合同收敛 CQ-03、CQ-05、INC-01、INC-03。

## 1. 这个系统是干什么的

239 不是新增玩法、编辑器面板或发布形态。它是 237/238 完成后的关键正确性门禁，负责四件事：

```text
Repair Scope
  一次 LLM repair 只能修正原候选，不能增加 operation、切换目标或扩大 destructive/Build scope。

Process Lifecycle
  子进程 stdout/stderr 必须在进程存活期间并发 drain；timeout/error 必须 kill、wait、join，不遗留进程或读取线程。

Publish Lock
  RuntimePackage/release directory 的发布锁必须绑定稳定 lock file，不能 unlock 后删除路径并制造双 publisher。

PE Contract
  stamp 后验证与最终独立 release verifier 必须检查同一份完整 Windows executable resource contract。
```

它大致对标：

```text
Unreal FWindowsPlatformProcess::ExecProcess 的持续 pipe drain 与进程生命周期收口。
UnityEditor.Utils.Program + ProcessOutputStreamReader 的双流读取线程与退出 join。
Windows LockFileEx / DeleteFile 的句柄锁与文件路径生命周期合同。
Godot TemplateModifier 的 FixedFileInfo / StringFileInfo / GroupIcon / ManifestInfo 完整资源集合。
```

239 不新增 `Architecture Guard`、进程常驻服务、发布守护进程或第二套 report 真相。

## 2. 为什么现在必须做

237/238 的产品链已经真实可运行，但 5.6 增量审查证明四个“会错误通过”的缺口仍存在：

```text
INC-01
  repair_scope_guard 只检查 repaired.operations <= 全局 48，初始 1 个 operation 可扩大成 48 个同类 operation。

CQ-03
  exported player verifier 和 Editor Build & Run headless verification 先 try_wait，再读取 piped stdout/stderr；大输出可填满 pipe 并被误报为 timeout。

CQ-05
  AtomicDirectoryPublishGuard::drop 先 unlock，再 remove lock path；B 可锁旧文件而 C 锁新建同名文件，形成双 publisher。

INC-03
  最终 release verifier 已读取 FileVersion、copyright、OriginalFilename、icon sizes、manifest presence，却没有把它们全部纳入通过条件。
```

这些问题位于已经完成的 AI Patch 与 Windows release 正式链路中。继续扩展 Provider Registry、Agent Planner、Launcher、签名或手动 authoring，会放大修复成本并让错误成功报告继续流入后续系统。

## 3. 当前代码基线

### 3.1 Repair scope

当前入口：

```text
rust/crates/editor_core/src/project_patch/llm_repair.rs
  repair_decision
  build_project_patch_repair_prompt
  repair_scope_guard
  diagnostic_fingerprint

rust/crates/editor_core/src/project_patch/model.rs
  PatchOperation
  operation_id
  depends_on
  kind
  target_summary
```

现状：

```text
全局 maximum_operation_count 生效。
risk/capability 不允许提高。
destructive/Build 只按 operation.kind 是否曾出现判断。
没有 initial/repaired operation 数量相等约束。
没有 typed target anchor。
没有 diagnostic 到允许修改字段的精确映射。
```

`target_summary` 是面向人和 report 的展示字符串，不得升级为 repair 安全合同。

### 3.2 Child process verification

当前存在两个正式 piped process 调用点：

```text
rust/crates/runtime_cli/src/exported_player_verification.rs
  run_child_process

rust/crates/editor_core/src/services/build_service.rs
  launch_headless_verification
```

两者都持有相同风险：

```text
spawn piped stdout/stderr
-> take pipe handles
-> loop try_wait
-> process exit/timeout 后才 read_to_string
```

当前没有正式 `RuntimeProcessSpawner` 代码模块；75 的历史文档只定义过这一目标。239 不恢复旧名字作为新架构层，而是在 `runtime_cli` 内建立一个窄的 bounded child process module，供这两个真实调用点复用。

### 3.3 Atomic directory publish

当前入口：

```text
rust/crates/engine_runtime/src/atomic_directory_publish.rs
  AtomicDirectoryPublishGuard
  atomic_directory_publish
  atomic_directory_publish_with_fault
```

RuntimePackage 和 release outer directory 已复用该模块。它的外部接口已经足够小，239 只修正内部 lock lifetime，不再加第二套 publish module。

### 3.4 Windows executable resources

当前入口：

```text
rust/crates/editor_core/src/windows_executable_resources.rs
  stamp_windows_executable_resources
  read_windows_executable_resources
  validate_resource_readback  // 当前私有

rust/crates/editor_core/src/release_package.rs
  verify_release_package_directory
```

stamp 后验证比最终独立 verifier 更严格，两个调用点对同一 PE 合同有两份不同判断。这违反“一个工程真相、一个验证接口”的规则。

## 4. 5.6 外部审查结论分类

### 4.1 必须修改

```text
CQ-03  导出玩家验证 stdout/stderr pipe deadlock 与 wait error cleanup。
CQ-05  unlock 后删除 lock file 的双 publisher 竞态。
INC-01 repair operation count/target/destructive/Build scope 扩张。
INC-03 最终 release verifier 的 PE 资源合同不完整。
```

这四项直接构成 239 的 blocking acceptance criteria。

### 4.2 施工约束

```text
进程测试必须输出至少 1 MiB stdout + 1 MiB stderr。
timeout/wait error 必须证明 child 已 reap、reader 已 join。
锁测试必须覆盖三个独立 OS process 的多轮 handoff，而不是只有单进程线程竞争。
PE 测试必须逐字段 mutation，不能只在正常包上观察字段存在。
repair 测试必须覆盖 1->2、1->48、同 kind 不同 target、Build/destructive target 扩张。
默认与 all-features workspace 回归都必须执行。
```

### 4.3 已由历史施工吸收

```text
CQ-02 all-features feature 条件测试失败已由 238 最终审计修复。
237 的 BuildProfile v2、AssetRef icon、PE stamp、portable layout 和 release process gate 已真实存在。
238 的真实 HTTP、strict schema、一次 repair、Import/Validator/Review/Confirm/transaction 已真实存在。
```

239 继承这些合同，不重复实现 237/238。

### 4.4 本轮不适用但必须进入后续队列

```text
CQ-01 通用 Runtime 与 complex shooter 固定 registry/UI producer 解耦。
CQ-04 SafeProjectPath / symlink / junction 写入逃逸。
CQ-06 diagnostics-first public loader panic 收敛。
CQ-07 Hygiene Gate 真正阻断规模回归。
CQ-08 CI/toolchain/lint budget。
INC-02 LLM worker 真实 cancellation/join。
Real Manual Authoring Walkthrough / Command Context Convergence v2。
```

不纳入 239 的原因是保持唯一施工文档范围可完成；它们不是被否定，而是排在 239 之后重新选择系统。

## 5. 成熟实现与可借鉴点

### 5.1 Rust process pipe

官方文档：

```text
https://doc.rust-lang.org/std/process/struct.Stdio.html
```

官方警告：向 pipe 写入超过 buffer 的数据、又没有同时读取另一方向输出，可能 deadlock；pipe buffer 大小随平台变化。

可学习：

```text
piped stream 必须在 child 存活时持续 drain。
不能把“child 已退出”作为开始读取 stdout/stderr 的前置条件。
```

### 5.2 Unreal Engine

源码：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Core/Private/Windows/WindowsPlatformProcess.cpp
  FWindowsPlatformProcess::ExecProcess
  ReadFromPipes
  ReadPipeToArray
```

关键调用链：

```text
CreatePipe stdout/stderr
-> CreateProcess
-> while IsProcRunning: ReadPipes
-> process exit 后 final ReadPipes
-> convert captured bytes
```

可学习：双流 drain 与 process running/wait 属于同一生命周期。  
不照搬：不引入 UE platform abstraction、Job Object 框架或无界 TArray 输出缓存。

### 5.3 Unity

源码：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/Utils/Program.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/Utils/ProcessOutputStreamReader.cs
```

关键调用链：

```text
Process.Start
-> stdout ProcessOutputStreamReader thread
-> stderr ProcessOutputStreamReader thread
-> process exit
-> GetOutput joins reader thread
```

可学习：reader 在 spawn 后立即开始，进程退出后 join。  
不照搬：不保留无界 line list，不把 Unity 的 `Program` 大接口复制到本项目。

### 5.4 Windows file lock/delete

官方文档：

```text
https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex
https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-deletefilew
```

关键合同：

```text
LockFileEx 对打开的 file handle 建立 shared/exclusive byte-range lock。
DeleteFile 的删除与 open handle / FILE_SHARE_DELETE / delete-on-close 有独立生命周期。
```

可学习：锁的文件身份必须稳定；持锁协议不能在 handoff 窗口删除 path 并允许新 file identity 出现。  
不照搬：不直接手写 LockFileEx；继续使用 Rust `File::try_lock`。

### 5.5 Windows VERSIONINFO / Godot

官方文档与源码：

```text
https://learn.microsoft.com/en-us/windows/win32/menurc/versioninfo-resource
<GODOT_SOURCE>/godot/platform/windows/export/template_modifier.cpp
  FixedFileInfo
  StringFileInfo
  GroupIcon
  ManifestInfo
  TemplateModifier::modify
```

关键合同：

```text
VERSIONINFO 包含 fixed FILEVERSION/PRODUCTVERSION 和字符串资源。
OriginalFilename 属于版本信息的一部分。
Godot 把 fixed version、strings、group icon 和 manifest 作为完整 executable resource set 生成。
```

可学习：final verifier 必须检查与 stamper 相同的完整资源集合。  
不照搬：不重写 PE resource parser；继续使用固定版本 `editpe` 和当前 readback。

## 6. 候选方案与正式选择

### 6.1 方案 A：四处最小修补

```text
llm_repair.rs 增加 operation count 判断。
两个调用点各自加 stdout/stderr reader thread。
AtomicDirectoryPublishGuard 停止 remove_file。
release_package.rs 增加缺失字段比较。
```

优点：施工快。  
缺点：两个 process caller 继续复制生命周期；PE 合同继续两份；repair 仍缺 typed target claim；下一轮容易回归。  
结论：不采用。

### 6.2 方案 B-min+：Deep Contract Convergence

```text
PatchRepairScopeGuard
  typed scope claim + diagnostic-directed mutation policy。

BoundedChildProcessRunner
  spawn + concurrent bounded drain + timeout/kill/wait/join + typed result。

WindowsExecutableResourceContract
  stamper/final verifier 共用完整验证接口。

AtomicDirectoryPublishGuard
  保留现有深接口，改为 stable lock file lifetime。
```

优点：四个缺口在各自自然 seam 内修复；调用方接口小；测试可直接穿过正式接口；不会增加运行时治理层。  
缺点：比局部补丁多一次 process caller 迁移和 typed repair claim 定义。  
结论：正式采用。

### 6.3 方案 C：全面质量治理整包

把 CQ-01、CQ-04、CQ-06-08、INC-02、Clippy、CI、大文件拆分和 manual walkthrough 全部纳入 239。

优点：一次性覆盖所有审查问题。  
缺点：范围不可控，无法按单一施工文档分 Gate 收口，也会混合正确性、架构解耦和工程治理。  
结论：不采用；相关系统进入后续队列。

## 7. 正式架构链

```text
ProjectPatch initial candidate + Import diagnostics
  -> PatchRepairScopeGuard
  -> repaired candidate allowed/rejected
  -> existing Import / Validator / Review / Confirm / Transaction

Exported player / Editor headless verification request
  -> BoundedChildProcessRunner
  -> concurrent bounded stdout/stderr drain
  -> exit/timeout/kill/wait/join
  -> typed process result
  -> existing verification reports

RuntimePackage / release directory publish request
  -> AtomicDirectoryPublishGuard on stable lock file
  -> existing staging/validate/rename/rollback
  -> unlock/close, lock path remains stable

BuildProfileApplication + entrypoint executable
  -> stamp_windows_executable_resources
  -> verify_windows_executable_resource_contract
  -> release staging verification
  -> verify_windows_executable_resource_contract
  -> publish only after full contract passes
```

禁止：

```text
新增 Logic Ownership Router / Architecture Guard。
新增第二套 ProjectPatch Validator。
新增第二套 release manifest 或 RuntimePackage assembler。
在 Editor/Runtime hot path 常驻 239 report。
用 target_summary 文本作安全 claim。
用无界 Vec/String 捕获 child 全量输出。
在 unlock 后删除 publish lock path。
stamper 和 final verifier 维护两份 PE 字段列表。
```

## 8. PatchRepairScopeGuard

### 8.1 Module 与 seam

建议落点：

```text
rust/crates/editor_core/src/project_patch/llm_repair.rs
rust/crates/editor_core/src/project_patch/repair_scope.rs  // 若施工复扫确认拆分更清晰
```

外部 interface 只暴露一个判断入口：

```text
validate_repair_scope(
  initial_import,
  repaired_patch,
  policy
) -> RepairScopeValidation
```

调用方不需要知道每种 operation 的 target anchor、diagnostic field policy 或 destructive/Build 判定细节。

### 8.2 Typed scope claim

每个 parseable initial operation 按原始 slot 生成：

```text
PatchOperationScopeClaim
  slot_index
  operation_kind
  immutable_target_anchor
  original_operation_id
  original_dependencies
  destructive_or_build
```

规则：

```text
initial/repaired operation count 必须相等。
operation slot 顺序必须相等；repair 不是重规划器。
每个 slot 的 operation kind 必须相等。
immutable target anchor 必须相等。
target_project_root 必须相等。
risk 不得提高。
required capabilities 只能保持或收缩，不能增加。
required capabilities 必须覆盖 repaired operations 结构化推导出的全部 domain capability；不得靠删除声明绕过 operation 权限。
schema_version / patch_id / title / source / intent_summary / expected_outcome / created_at 必须 canonical-equal。
```

`immutable_target_anchor` 必须由各 operation variant 结构化生成，示例：

```text
Scene.SetComponentField
  scene path + entity id + component type

Prefab.SetStageEntityField
  prefab path/stage identity + source entity id + component type

AUI.SetNodeField
  document path + node id

Rule.UpdateStatement / UpdateOperation
  rule asset path + statement/operation identity

Build.ExportDesktopPackage
  profile/target claim
```

不得用格式可变的 `target_summary()` 作为 claim。

`PatchOperationScopeClaim`、destructive/Build 分类和字段比较必须对当前全部 `PatchOperation` variant 做穷尽 match；未来新增 variant 时，编译必须迫使 repair policy 同步更新，不允许 `_ => safe`、字符串 `contains("Delete")` 或其它默认放行。

### 8.3 Diagnostic-directed mutation

只有 initial import 中实际出现的 repairable diagnostic 才能授权对应字段改变：

```text
operation_id_required / operation_id_duplicate
  只允许修正 operation id；slot/kind/target 不变。

dependency_missing
  只允许修正该 slot 的 dependency ids；只能引用 repaired patch 内既有 operation，不能新增 operation。

scene.component_field_invalid
  只允许修正被点名 Scene operation 的 field path/value；scene/entity/component anchor 不变。

prefab.stage_field_invalid
  只允许修正被点名 Prefab operation 的 field path/value；prefab/source entity/component anchor 不变。

aui.node_field_invalid
  只允许修正被点名 AUI operation 的 field/value；document/node anchor 不变。

rule.payload_invalid
  只允许修正被点名 Rule operation 的 payload；rule asset/card/statement anchor 不变。
```

没有 diagnostic 授权的 operation 语义字段必须 canonical-equal。

安全授权不得直接信任 `PatchDiagnostic.operation_id` 或 `PatchDiagnostic.target`。这两个字段只作 report evidence，其中 `operation_id_required` 没有 id，而现有 `target` 可能来自 `target_summary()`。正式实现必须用“初始 typed patch + 实际 diagnostic code”重新计算可修复 slot：空 id、重复 id、缺失 dependency 和各 typed field invalid 都从初始 operation 结构本身定位；无法唯一定位时 fail closed。slot 定位后只开放上表指定字段，其它字段保持 canonical-equal。

### 8.4 Destructive / Build

```text
只按 kind 曾出现不足以证明 scope 不变。
destructive/Build operation 必须保持同一 slot、kind、typed target anchor 和数量。
不得把一个 Build operation 扩大成多个 Build operation。
不得把同 kind operation 改到另一 scene/entity/asset/profile。
```

### 8.5 Parse-failed baseline

candidate 无法 parse 时没有 typed baseline，不能声称“scope 已证明不变”。正式规则：

```text
status = scope_unprovable
risk = Low
destructive = forbidden
Build = forbidden
required capabilities 必须与 repaired operations 结构化推导的非 Build domain capability 集合一致
operation count <= min(global maximum, 8)
最终仍必须重新走 Import / Validator / Review / Confirm
```

`8` 是 v1 no-baseline hard cap，避免从无法解析的文本直接生成 48-operation proposal。后续如需调整必须升级 policy/schema，不允许在调用点散落魔法数字。

### 8.6 Result / diagnostics

建议：

```text
RepairScopeValidation
  status: passed | rejected | scope_unprovable_restricted
  initial_operation_count
  repaired_operation_count
  changed_slots
  diagnostic_codes
  rejection_code
```

至少区分：

```text
repair_scope_operation_count_expanded
repair_scope_operation_kind_changed
repair_scope_target_changed
repair_scope_unauthorized_field_changed
repair_scope_dependency_expanded
repair_scope_destructive_or_build_expanded
repair_scope_risk_expanded
repair_scope_capability_expanded
repair_scope_unprovable_limit_exceeded
```

这些 evidence 进入既有 `project.patch` Summary/Trace，不新增独立 LLM report。

## 9. BoundedChildProcessRunner

### 9.1 Module 与 seam

建议新增：

```text
rust/crates/runtime_cli/src/bounded_child_process.rs
```

理由：`runtime_cli` 拥有 exported player process verification；`editor_core` 已依赖 `runtime_cli`，两个真实 production caller 可以复用同一 interface，不新增 crate。

外部 interface：

```text
BoundedChildProcessRequest
  executable
  args
  current_dir
  timeout
  stdout_capture_limit_bytes
  stderr_capture_limit_bytes

run_bounded_child_process(request) -> BoundedChildProcessResult
```

结果：

```text
BoundedChildProcessResult
  process_id
  exit_reason: completed | failed | timeout | wait_failed | spawn_failed
  exit_code
  elapsed_ms
  stdout_summary
  stderr_summary
  stdout_total_bytes
  stderr_total_bytes
  stdout_truncated
  stderr_truncated
  kill_error
  wait_error
```

新增 evidence 会改变既有 product report shape，因此施工时必须升级并迁移消费者：

```text
exported-player-process-verification-report.v1 -> v2
editor-build-and-run-report.v1 -> v2
llm-patch-request-report.v1 -> v2
```

不得在仍声明 v1 时静默追加 required 字段；所有序列化测试、Report Panel provider 和 E2E consumer 必须同步。

### 9.2 生命周期合同

```text
spawn 后立即分别启动 stdout/stderr reader。
reader 必须持续 drain 到 EOF，但内存中只保存 bounded summary。
不能因 summary 已满而停止 drain。
主线程同时执行 bounded wait。
timeout -> kill -> wait/reap -> join readers。
try_wait error -> kill if needed -> wait/reap -> join readers。
normal exit -> wait completion -> join readers。
函数返回时不得遗留 child 或 reader thread。
```

239 v1 不实现任意 process tree/job object 管理；正式 player/fixture 不生成 descendants。未来若出现 descendant pipe inheritance，再单独升级 process-tree policy。

`try_wait`/kill/wait/join 的错误分支必须可测。允许在 module 内增加私有、test-only fault seam（例如 `run_bounded_child_process_with_fault`），但不得把 fault 类型暴露给 production caller。真实 1 MiB 双流测试覆盖 OS pipe；内部 fault test 覆盖 wait error 后仍执行 kill/wait/join。两者共同证明生命周期合同。

### 9.3 Bounded capture

```text
每次 read 使用固定大小 buffer。
total_bytes 始终累计。
capture limit 的单位是原始 byte；summary 只保留配置上限内的原始 byte，再做 UTF-8 lossy 解码。
超限设置 truncated=true，仍继续丢弃式 drain。
默认 product report 最终仍按 Unicode char 截断到 2,000 字符；byte cap 与 char summary cap 不得混为一个未标单位的参数。
```

禁止把 1 MiB 测试输出完整保存在 report/JSON。

### 9.4 正式调用点迁移

必须迁移：

```text
runtime_cli::exported_player_verification::run_child_process
editor_core::services::build_service::launch_headless_verification
```

不迁移：

```text
user windowed detached launch（stdout/stderr 已为 null，生命周期由用户会话持有）。
save/reload child（stderr 重定向到 owned file，不存在 pipe fill deadlock）。
project_e2e release no-arg fixture（stdout/stderr 为 null）。
```

## 10. Stable Atomic Publish Lock

### 10.1 正式规则

`AtomicDirectoryPublishGuard` interface 保持不变，内部改为：

```text
lock path = stable hidden file beside final directory
OpenOptions create/read/write + truncate(false)
try_lock exclusive
Drop: unlock/close only
lock file path remains after release
```

稳定 lock file 是协调对象，不属于发布 payload，不得因为“目录看起来干净”而删除。

### 10.2 三进程 handoff Gate

测试必须使用三个独立 OS process：

```text
A holds lock
B waits/retries and acquires after A
C competes during A->B handoff
```

验收：

```text
任意时刻 active publisher count <= 1。
重复多轮 handoff 无双 owner。
最终 package/release directory 可由正式 loader/verifier 加载。
manifest/content/payload hash 与最后一次合法 publish 一致。
lock file 可存在且不进入 payload inventory/hash。
```

不以单进程 mutex 或仅线程测试替代 OS file-lock 验收。

## 11. WindowsExecutableResourceContract

### 11.1 Module 与 interface

深化现有：

```text
rust/crates/editor_core/src/windows_executable_resources.rs
```

将私有 `validate_resource_readback` 收敛为 stamper 与 final verifier 共用的唯一接口：

```text
verify_windows_executable_resource_contract(
  executable_path,
  expected: WindowsExecutableResourceExpectation
) -> Result<WindowsExecutableResourceReadback, WindowsExecutableResourceError>
```

`WindowsExecutableResourceExpectation` 是 module 内的完整期望值，由 `BuildProfileApplication` 或 release manifest 的 `ReleasePackageApplication` 构造；两个 caller 不各自维护字段列表。

`stamp_windows_executable_resources` 在 stamp 后调用它；`verify_release_package_directory` 也调用它，不再自行维护字段比较。

### 11.2 完整合同

必须同时验证：

```text
ProductName
CompanyName
FileDescription
ProductVersion string
FileVersion string
fixed file version [u16; 4]
fixed product version [u16; 4]
LegalCopyright
OriginalFilename = <executableName>.exe
icon sizes = 16/32/48/64/128/256
application manifest present
```

只要任一字段缺失或不匹配，staging/final verification 都失败，不允许仅凭 inventory hash 自洽通过。

### 11.3 Mutation tests

必须构造逐字段变异 fixture：

```text
wrong/missing ProductName
wrong/missing CompanyName
wrong/missing FileDescription
wrong/missing ProductVersion string
wrong/missing FileVersion string
wrong copyright
wrong OriginalFilename
missing GROUP_ICON or one required size
missing application manifest
fixed file version mismatch
fixed product version mismatch
```

每个变异都必须由独立 `verify_release_package_directory` 拒绝，并输出稳定 diagnostic code/stage/path/next_action。

每次修改 PE 后，fixture 必须同步重算 entrypoint file size/SHA-256 和 `releasePayloadHash`，写回自洽的 release manifest；否则只能证明 inventory/hash verifier 生效，不能证明独立 PE contract 拒绝了“哈希自洽但资源错误”的包。

## 12. 验证产物与 Report 规则

239 不新增 Runtime/Editor 常驻 report provider。证据分两层：

```text
Product reports
  llm-patch-request-report.v2 / project.patch provider 追加 repair scope evidence。
  exported player / build-and-run report 追加 bounded process evidence。
  build.release_package 复用完整 resource contract diagnostics。

Validation-only aggregate
  project_e2e_gate 可生成 critical-correctness-safety-gate-report.v1。
  只用于 Gate/Trace，不进入正式 Runtime hot path。
```

聚合字段建议：

```text
schemaVersion
status
repairScopeSummary
processLifecycleSummary
publishLockSummary
peContractSummary
diagnostics[]
nextActions[]
```

## 13. 预期涉及文件

生成施工文档前必须复扫当前代码，预计涉及：

```text
rust/crates/editor_core/src/project_patch/llm_repair.rs
rust/crates/editor_core/src/project_patch/repair_scope.rs
rust/crates/editor_core/src/project_patch/llm_request.rs
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/windows_executable_resources.rs
rust/crates/editor_core/src/release_package.rs
rust/crates/editor_core/src/report_panel.rs                  // 仅既有 evidence 字段需要时

rust/crates/runtime_cli/src/lib.rs
rust/crates/runtime_cli/src/exported_player_verification.rs
rust/crates/runtime_cli/src/bounded_child_process.rs
rust/crates/runtime_cli/src/bin/bounded_output_fixture.rs
rust/crates/runtime_cli/tests/bounded_child_process.rs

rust/crates/engine_runtime/src/atomic_directory_publish.rs

rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/critical_correctness_safety.rs
rust/crates/project_e2e_gate/src/bin/<atomic-publish-fixture>.rs
```

文件名可在施工复扫后收敛，但不得增加新 crate、第二个 publish module 或第二个 PE parser。

## 14. 推荐施工 Gate

### Gate A：Repair Scope Contract

施工：

```text
typed PatchOperationScopeClaim。
diagnostic-directed mutation policy。
parseable baseline strict equality rules。
parse_failed scope_unprovable restricted policy + hard cap 8。
existing project.patch evidence 接入。
exhaustive variant match 与 typed destructive/Build classification。
llm-patch-request-report schema v2 consumer migration。
```

测试：

```powershell
cargo test -p editor_core llm_repair
cargo test -p editor_core repair_scope
cargo test -p editor_core project_patch
```

必测否定案例：

```text
1 -> 2
1 -> 48
same kind / different target
operation reorder
new dependency target
destructive/Build duplicate or retarget
undiagnosed field mutation
parse_failed > 8
```

### Gate B：Bounded Child Process Lifecycle

施工：

```text
BoundedChildProcessRunner。
双 reader concurrent bounded drain。
normal/timeout/wait-error cleanup。
runtime_cli verifier 与 editor_core headless verification 迁移。
私有 test-only wait fault seam。
exported-player/editor-build-and-run report schema v2 consumer migration。
```

测试：

```powershell
cargo test -p runtime_cli exported_player
cargo test -p runtime_cli bounded_child_process
cargo test -p editor_core build_and_run
```

必测：

```text
stdout >= 1 MiB + stderr >= 1 MiB，exit 0，不误报 timeout。
summary bounded，total_bytes 正确，truncated=true。
timeout 后 child 已 reap、reader 已 join。
非零退出保留 exit code 与双流摘要。
```

### Gate C：Stable Atomic Publish Lock

施工：

```text
stable lock file lifetime。
Drop 不 remove path。
三 OS process handoff fixture。
RuntimePackage/release 双消费者回归。
```

测试：

```powershell
cargo test -p engine_runtime atomic_directory_publish
cargo test -p engine_runtime runtime_package_publish
cargo test -p project_e2e_gate atomic_publish
```

### Gate D：Full PE Resource Contract

施工：

```text
共享 verify_windows_executable_resource_contract。
stamper/final verifier 单一字段真相。
逐字段 mutation fixtures。
每个 mutation 后重算 manifest inventory/hash，保持包自洽。
existing release report diagnostics 接入。
```

测试：

```powershell
cargo test -p editor_core windows_executable_resources
cargo test -p editor_core release_package
cargo test -p project_e2e_gate release_package
```

### Gate E：Aggregate Evidence / Regression / Docs

施工：

```text
validation-only aggregate report。
5.6 审查四项关闭证据。
入口、完成记录和施工归档同步。
```

整体回归：

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

当前严格 Clippy/Hygiene 已有大规模历史债务，不作为 239 完成条件；239 不得新增 warning，CI/lint/hygiene 收敛另开后续系统。

## 15. 本轮明确不做

```text
通用 Runtime 与 complex shooter 固定 registry/UI producer 解耦。
SafeProjectPath / symlink / junction 全写入链路收敛。
LLM transport socket abort、worker cancellation/join。
Provider Registry、Agent Planner、多轮 repair。
Launcher、签名、installer、MSIX、商店发布。
Scene loader panic 全面改造。
CI、rust-toolchain、workspace lint budget、Clippy 清零。
大文件拆分和 Hygiene threshold 全面治理。
Real Manual Authoring Walkthrough v2。
```

## 16. 风险与控制

### 风险 1：把 239 扩大成全面代码质量重构

控制：只关闭 CQ-03、CQ-05、INC-01、INC-03；其它审查项进入后续队列。

### 风险 2：Repair guard 严格到合法修复无法进行

控制：使用 diagnostic-directed mutation；operation id/dependency/field/payload 各有明确允许范围。被拒绝时用户仍可重新生成，不自动放宽 scope。

### 风险 3：Parse-failed 没有 baseline 却声称 scope 不变

控制：显式输出 `scope_unprovable`，使用 Low/non-destructive/non-Build/cap 8 降级政策，并保留人工确认。

### 风险 4：Reader thread 只截断输出后停止读取

控制：截断只停止存储，不停止 drain；1 MiB 双流 fixture 是 blocking test。

### 风险 5：Timeout 路径 join 卡死

控制：顺序固定为 kill -> wait/reap -> join；v1 child 不生成 descendants，未来 descendants 另开 process-tree policy。

### 风险 6：稳定 lock file 被误当成发布垃圾

控制：lock 位于 final directory 同级隐藏路径，明确排除 payload inventory/hash；它是协调对象，允许长期存在。

### 风险 7：共享 PE verifier 改变 237 manifest/hash 真相

控制：只统一 readback acceptance；不改变 BuildProfile v2、release manifest、payload hash、stamping order 或 RuntimePackage。

## 17. 方案自审

### 17.1 是否符合用户确认

是。正式采用：

```text
B-min+：Deep Contract Convergence
```

包括三个深模块和一个稳定发布锁合同，没有扩大为方案 C。

### 17.2 是否完整处理 5.6 审查输入

是。已把两份审查结论分为：

```text
必须修改：CQ-03、CQ-05、INC-01、INC-03。
施工约束：1 MiB 双流、三进程 handoff、PE mutation、repair expansion negative tests。
历史吸收：CQ-02 与 237/238 已完成能力。
本轮不适用：CQ-01、CQ-04、CQ-06-08、INC-02 和 manual walkthrough；均保留后续队列。
```

### 17.3 是否形成深模块

是。

```text
Repair caller 只知道一个 validate interface，不知道各 operation anchor 细节。
Process caller 只知道 request/result，不知道 reader/kill/wait/join 细节。
PE caller 只知道 expected application contract，不维护字段列表。
Atomic publish interface 不变，只修内部 lock lifetime。
```

删除这些模块时，复杂性会重新散回多个 caller，因此它们具备真实 Depth 和 Locality；没有为单一 adapter 引入虚构 port。

### 17.4 是否保持 RuntimePackage / ProjectPatch 真相

是。

```text
RuntimePackage 仍由 ProjectRuntimePackageAssembler -> RuntimePackageBuilder 产生。
ProjectPatch initial/repaired candidate 仍统一进入 Import / Validator / Review / Confirm / Transaction。
239 不新增第二套 schema、validator、assembler 或 manifest。
```

### 17.5 是否满足 report 分档

是。Runtime 不新增 report；Editor 复用现有 Summary/Trace；聚合 artifact 只存在于 validation Gate。

### 17.6 是否避免过度抽象

是。Bounded process module 有两个真实 production caller；PE verifier 有 stamp/final 两个真实 caller；Atomic publish 已有 RuntimePackage/release 两个真实 caller。Repair scope 是纯 in-process 安全判断，不需要 adapter。

### 17.7 是否可以生成施工文档

可以。方案已固定范围、interface、diagnostics、Gate 和测试命令。下一步仍必须：

```text
读取任何针对 239 的新增审查
-> 必要时先修改本正式方案
-> 生成唯一 239 当前施工文档
-> 做施工文档自审
-> 自审通过后才开始 Gate A
```

唯一 239 施工文档已完成 Gate A-E、整体回归、阶段完成记录和归档。

### 17.8 2026-07-11 正式方案审查结论

审查对象与当前系统一致：两份 5.6 审查报告、239 正式方案、237/238 已完成合同，以及 repair/process/publish/PE 当前源码均已复核。审查发现的四个施工级缺口已回填本方案：

```text
repair authorization 从 typed initial patch 重算，不信任 diagnostic target/summary；顶层 metadata 与全部 variant fail closed。
process module 增加私有 test-only wait fault seam，并明确 byte capture 与 2,000-char product summary。
三个新增 evidence 的既有 report 统一升级 v2 并迁移消费者，不在 v1 下静默改 shape。
PE mutation 后重算 manifest inventory/hash，确保验证的是完整资源合同而非先被 hash mismatch 拒绝。
```

这些修改深化既有 module interface，没有新增 crate、第二 validator、第二 publish module、第二 PE parser 或常驻 report 层，也没有把 CQ-01、CQ-04、CQ-06-08、INC-02 扩入 239。结论：`通过，可以生成唯一 239 当前施工文档`。

## 18. 正式结论

正式采用：

```text
B-min+：PatchRepairScopeGuard
       + BoundedChildProcessRunner
       + Stable AtomicDirectoryPublishGuard lock file
       + WindowsExecutableResourceContract
       + existing product reports
       + validation-only aggregate gate evidence
```

完成标准：

```text
repair 无法扩张 operation count/kind/target/destructive/Build scope。
1 MiB 双流 child 不 deadlock、不误报 timeout，所有失败路径无遗留 child/reader。
三进程 publish handoff 永远只有一个 owner，最终产物可正式加载。
缺失任一 PE metadata/icon/manifest 字段的 release directory 都被独立 verifier 拒绝。
default/all-features workspace 回归通过。
```

## 19. 后续优先级

239 完成后重新选择以下系统，默认先处理仍为 P1 的安全/架构问题：

```text
1. CQ-04 SafeProjectPath / symlink-junction write containment。
2. CQ-01 Project RuntimeModule / fixed complex-shooter registry/UI producer decoupling + second-project gate。
3. INC-02 LLM worker cancellation/join lifecycle。
4. CQ-06 diagnostics-first loader panic convergence。
5. Real Manual Authoring Walkthrough / Command Context Convergence v2。
6. CI / toolchain / lint / hygiene gate。
```

若用户明确调整产品目标，可重新排序，但不能把未关闭 P1 缺口当作已完成。

## 20. 实际完成结果

```text
Gate A：typed ProjectPatch repair scope 已覆盖全部 46 个当前 operation variant，llm-patch-request-report 升级 v2。
Gate B：BoundedChildProcessRunner 已迁移两处 production caller，两份 process product report 升级 v2。
Gate C：稳定 publish lock path 与三进程多轮 handoff Gate 已通过。
Gate D：唯一 PE resource contract 与十一项 hash-consistent mutation matrix 已通过。
Gate E：critical-correctness-safety-gate-report.v1、默认 workspace 与 all-features workspace 回归已通过。
```

完成记录：`阶段完成记录/2026-07-11-Critical-Correctness-and-Safety-Convergence-Gate-v1/00-总览.md`。

239 没有扩入 CQ-01、CQ-04、CQ-06-08、INC-02，也没有新增 warning。下一步按第 19 节先讨论 CQ-04，未确认新方案前不直接施工。

## 21. 参考

```text
审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md
审查目录/5.6审查目录/01-2026-07-11-新增功能增量代码质量审查报告.md

237-Release-Package-Polish-Metadata-Icon-Layout-v1方案.md
238-Real-LLM-Provider-Minimal-Repair-Loop-v1方案.md
227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md
75-真实RustRuntimeCLI-ProcessSpawn方案.md

rust/crates/editor_core/src/project_patch/llm_repair.rs
rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/windows_executable_resources.rs
rust/crates/editor_core/src/release_package.rs
rust/crates/runtime_cli/src/exported_player_verification.rs
rust/crates/engine_runtime/src/atomic_directory_publish.rs

<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Core/Private/Windows/WindowsPlatformProcess.cpp
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/Utils/Program.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/Utils/ProcessOutputStreamReader.cs
<GODOT_SOURCE>/godot/platform/windows/export/template_modifier.cpp

https://doc.rust-lang.org/std/process/struct.Stdio.html
https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex
https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-deletefilew
https://learn.microsoft.com/en-us/windows/win32/menurc/versioninfo-resource
```
