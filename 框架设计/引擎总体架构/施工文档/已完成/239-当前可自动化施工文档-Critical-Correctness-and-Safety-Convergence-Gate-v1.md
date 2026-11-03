# 239-当前可自动化施工文档-Critical Correctness and Safety Convergence Gate v1

> 状态：已完成（Gate A-E、整体回归、完成记录与归档全部闭环）。  
> 正式方案：`../../239-Critical-Correctness-and-Safety-Convergence-Gate-v1方案.md`。  
> 方案选择：用户已确认 `B-min+：Deep Contract Convergence`。  
> 方案审查：2026-07-11 已对照两份 5.6 审查、237/238 合同和当前源码完成审查；审查修改已回填正式方案。  
> 执行规则：严格按 Gate A -> B -> C -> D -> E 串行施工；当前 Gate 失败时停止，不跨 Gate 堆积问题。

## 1. 施工目标

239 只关闭四个会让正式链路错误通过的缺口：

```text
INC-01  ProjectPatch repair 可以扩张 operation count/kind/target/destructive/Build scope。
CQ-03   piped child process 先 wait 后读 stdout/stderr，可死锁并误报 timeout，错误路径可能遗留 child。
CQ-05   publish guard unlock 后删除 lock path，可让两个进程锁住不同文件对象并同时发布。
INC-03  final release verifier 没有验证完整 PE metadata/icon/manifest contract。
```

施工完成必须同时证明：

```text
parseable repair 保持 operation count、slot order、kind、typed target anchor 和未授权字段不变。
parse-failed repair 明确标记 scope_unprovable_restricted，Low、非 destructive、非 Build、最多 8 个 operation。
stdout/stderr 各至少 1 MiB 的 child 可正常退出，摘要有界，timeout/wait-error 均完成 kill/wait/join。
三个独立 OS process 多轮 handoff 任意时刻只有一个 publisher，稳定 lock file 不进入 payload/hash。
哈希与 inventory 自洽但任一 PE 字段/icon size/manifest 被破坏的 release package 都被独立 verifier 拒绝。
default 与 all-features workspace 回归通过。
```

## 2. 施工前基线与审查结论

### 2.1 当前状态

生成本文档前确认：

```text
施工文档/当前/ 为空。
237 Gate A-F 已完成并归档。
238 Gate A-G 已完成并归档。
239 正式方案已完成并通过方案审查。
当前没有 239 代码施工。
```

当前真实代码缺口：

```text
editor_core/project_patch/llm_repair.rs
  repair_scope_guard 只检查全局 48、risk/capability 和 destructive/Build kind 是否出现。

runtime_cli/exported_player_verification.rs
editor_core/services/build_service.rs
  两个 caller 都是 try_wait loop 完成后才 read_to_string(piped stdout/stderr)。

engine_runtime/atomic_directory_publish.rs
  AtomicDirectoryPublishGuard::drop = unlock + remove_file(lock path)。

editor_core/windows_executable_resources.rs
editor_core/release_package.rs
  stamp-time 与 final verifier 分别维护 PE acceptance，final contract 缺字段。
```

### 2.2 外部审查分类

已完整读取：

```text
审查目录/5.6审查目录/00-2026-07-11-项目代码质量全面审查报告.md
审查目录/5.6审查目录/01-2026-07-11-新增功能增量代码质量审查报告.md
```

分类结论：

| 分类 | 项目 | 239 处理方式 |
|---|---|---|
| 必须修改 | CQ-03、CQ-05、INC-01、INC-03 | 分别进入 Gate B、C、A、D |
| 施工约束 | 1 MiB 双流、三进程 handoff、PE mutation、repair expansion negative tests | 写入对应 Gate 的阻断测试 |
| 历史吸收 | CQ-02、237/238 已完成功能 | 只做回归，不重复实现 |
| 本轮不适用 | CQ-01、CQ-04、CQ-06-08、INC-02、manual walkthrough | 保留后续队列，不扩入 239 |

### 2.3 正式方案审查修改

正式方案不是原样放行。源码复核后已先回填四项：

```text
1. repair 授权从 typed initial patch 重新计算，不能信任 diagnostic target/target_summary。
2. process module 提供私有 test-only wait fault seam，并区分 raw-byte capture 与 2,000-char product summary。
3. llm patch / exported process / editor build-and-run 三份既有 report 新增 required evidence 时统一升级 v2。
4. PE mutation 后重算 entrypoint inventory/hash 与 releasePayloadHash，证明资源合同独立拒绝。
```

结论：正式方案已完成必要修改，无剩余阻断项，可以生成并执行本文档。

## 3. 固定边界

### 3.1 本轮必须交付

```text
PatchRepairScopeGuard
BoundedChildProcessRunner
Stable AtomicDirectoryPublishGuard lock file
WindowsExecutableResourceContract
既有 product report v2 evidence
validation-only critical-correctness-safety-gate-report.v1
```

### 3.2 本轮禁止扩入

```text
不新增 crate。
不新增第二套 ProjectPatch validator/import/apply 链。
不新增第二套 atomic publish module。
不新增第二个 PE parser/stamper。
不新增 Runtime/Editor 常驻 report provider。
不实现任意 process tree/job object 管理。
不处理 LLM worker cancellation/join。
不处理 SafeProjectPath/symlink/junction 全写入链路。
不处理 complex shooter registry/UI producer 解耦。
不处理 scene loader panic、CI、toolchain、Clippy、hygiene 全面治理。
不修改 gameplay、AUI、Renderer、RuntimePackage schema 或 BuildProfile v2 产品目标。
```

## 4. 跨 Gate 固定合同

### 4.1 Repair Scope interface

外部只保留一个判断入口：

```text
validate_repair_scope(
  initial_import: &ProjectPatchImportResult,
  repaired: &ProjectPatchDocument,
  policy: RepairScopePolicy
) -> RepairScopeValidation
```

固定结果：

```text
RepairScopeValidation
  status: passed | rejected | scope_unprovable_restricted
  initial_operation_count
  repaired_operation_count
  changed_slots
  diagnostic_codes
  rejection_code
```

固定规则：

```text
parseable baseline：count、slot、kind、typed anchor 相等。
schemaVersion/patchId/title/source/intentSummary/expectedOutcome/createdAt canonical-equal。
targetProjectRoot 相等；risk 不提高；capability 不增加且覆盖 operation-derived domains。
operationId/dependency/semantic field 只在 typed initial operation 自身满足对应 invalid predicate 时开放。
PatchDiagnostic.operation_id/target 只作 evidence，不作授权。
未授权字段 canonical-equal。
typed anchor、destructive/Build 和字段 policy 对全部 PatchOperation variant 穷尽 match，禁止默认放行。
parse-failed：Low、非 destructive、非 Build、operation <= 8、capability 等于 operation-derived non-Build domains。
```

### 4.2 Bounded process interface

```text
BoundedChildProcessRequest
  executable
  args
  current_dir
  timeout
  stdout_capture_limit_bytes
  stderr_capture_limit_bytes

BoundedChildProcessResult
  process_id
  exit_reason: completed | failed | timeout | wait_failed | spawn_failed
  exit_code
  elapsed_ms
  stdout_summary / stderr_summary
  stdout_total_bytes / stderr_total_bytes
  stdout_truncated / stderr_truncated
  spawn_error / kill_error / wait_error / reader_join_error
```

生命周期顺序固定：

```text
spawn -> immediately start stdout/stderr drain readers -> bounded wait
normal -> wait/reap -> join readers
timeout -> kill -> wait/reap -> join readers
try_wait error -> kill if needed -> wait/reap -> join readers
return only after child and both readers are closed
```

reader 达到 capture byte cap 后只停止保存，不停止 drain。product caller 最终仍按 Unicode char 截断到 2,000 字符。

### 4.3 Report schema migration

新增 required evidence 时必须同步升级：

```text
llm-patch-request-report.v1 -> v2
exported-player-process-verification-report.v1 -> v2
editor-build-and-run-report.v1 -> v2
```

所有 serde fixtures、Report Panel provider、release/E2E consumer 同 Gate 更新；禁止 shape 已变但 schemaVersion 仍为 v1。

### 4.4 Stable publish lock

```text
lock path = final directory 同级 .<name>.publish.lock
OpenOptions = create/read/write/truncate(false)
Drop = unlock/close only
lock path 永久稳定存在
lock file 不属于 final directory payload，不进入 inventory/hash
```

### 4.5 PE expectation interface

```text
WindowsExecutableResourceExpectation
  product_name
  company_name
  file_description
  product_version
  file_version
  fixed_file_version
  fixed_product_version
  copyright
  original_filename
  icon_sizes
  manifest_required

verify_windows_executable_resource_contract(path, expectation)
  -> Result<WindowsExecutableResourceReadback, WindowsExecutableResourceError>
```

expectation 分别由 `BuildProfileApplication` 和 manifest `ReleasePackageApplication` 构造；stamper/final verifier 不维护两份字段列表。

## 5. 预计涉及文件

施工前每个 Gate 仍需复扫；当前允许范围：

```text
rust/crates/editor_core/src/project_patch/repair_scope.rs                 // new
rust/crates/editor_core/src/project_patch/llm_repair.rs
rust/crates/editor_core/src/project_patch/llm_request.rs
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/project_patch/model.rs                         // 仅 typed helper 必要时
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/windows_executable_resources.rs
rust/crates/editor_core/src/release_package.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/lib.rs

rust/crates/runtime_cli/src/bounded_child_process.rs                       // new
rust/crates/runtime_cli/src/exported_player_verification.rs
rust/crates/runtime_cli/src/bin/bounded_output_fixture.rs                  // new test fixture
rust/crates/runtime_cli/tests/bounded_child_process.rs                     // new
rust/crates/runtime_cli/src/lib.rs

rust/crates/engine_runtime/src/atomic_directory_publish.rs

rust/crates/project_e2e_gate/src/critical_correctness_safety.rs            // new
rust/crates/project_e2e_gate/src/bin/atomic_publish_fixture.rs             // new
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/release_package.rs                        // 复用/回归
rust/crates/project_e2e_gate/src/editor_build_and_run.rs                   // schema consumer
rust/crates/project_e2e_gate/src/exported_windows_playable_golden.rs       // schema consumer

框架设计/引擎总体架构/239-Critical-Correctness-and-Safety-Convergence-Gate-v1方案.md
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

若施工复扫发现额外直接 consumer，可在同一 Gate 内加入，但必须记录原因；不得借机引入 deferred 系统。

## 6. Gate A - Repair Scope Contract

### 6.1 涉及文件

```text
editor_core/src/project_patch/repair_scope.rs
editor_core/src/project_patch/llm_repair.rs
editor_core/src/project_patch/llm_request.rs
editor_core/src/project_patch/mod.rs
editor_core/src/project_patch/model.rs（仅必要 helper）
editor_core/src/services/ai_service.rs
editor_core/src/report_panel.rs
editor_core/src/lib.rs
```

### 6.2 任务

1. 建立 `PatchRepairScopeGuard` 深 module，将 typed claim、field policy、capability derivation、destructive/Build classification 和 result evidence 收敛到一个 interface。
2. 对 Scene/Input/Asset/Prefab/Aui/Rule/Build 的全部当前 variant 使用穷尽 match；新增 variant 未加入 policy 时必须编译失败。
3. parseable baseline 严格验证 document metadata、count/order/kind/typed anchor 和未授权字段。
4. 通过 typed initial operation 重新计算可修复 slot：空 id、重复 id、missing dependency、Scene/Prefab/AUI/Rule invalid field；不得信任 display target。
5. dependency 修复只能引用 repaired patch 同一 operation 集内存在的 id，不能新增 slot、重排或形成 validator 未拒绝的缺失引用。
6. parse-failed 使用 `REPAIR_SCOPE_UNPROVABLE_MAX_OPERATIONS = 8`，禁止 destructive/Build，要求 Low，并输出 `scope_unprovable_restricted`。
7. 将 AI service 从旧 `repair_scope_guard` 迁移到新 interface；initial/repaired 仍统一走 Import/Validator/Review/Confirm/transaction。
8. `LlmPatchRequestReport` 升级 v2，加入 status/count/changedSlots/rejectionCode 的有界 repair scope evidence；Off/Summary/Trace 分档不保存 raw prompt/candidate。
9. 删除旧 kind-string heuristic 和重复 scope 判断，不保留第二 validator。

### 6.3 阻断测试矩阵

```text
1 -> 2；1 -> 48；operation reorder；kind change。
每个 domain 至少一个 same-kind/different-target；全部 variant 有 typed anchor table test。
Create/Delete/Remove/Build duplicate、retarget、slot change。
top-level metadata change；targetProjectRoot change；risk/capability expansion。
undiagnosed semantic field change。
diagnostic target 伪造不能授权另一 slot。
empty/duplicate operation id 按 typed slot 修复；无法唯一定位 fail closed。
dependency 指向新 id/不存在 id/新 slot 被拒绝。
parse_failed：9 operations、Medium/High、destructive、Build、capability mismatch 被拒绝。
parse_failed：<=8、Low、非 destructive/Build 返回 scope_unprovable_restricted 并继续 Review/Confirm。
```

### 6.4 Gate 测试

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core repair_scope
cargo test -p editor_core llm_repair
cargo test -p editor_core llm_patch
cargo test -p editor_core project_patch
```

### 6.5 通过条件

```text
所有否定矩阵通过。
新 interface 是唯一 repair scope acceptance 入口。
target_summary/PatchDiagnostic.target 不参与安全授权。
llm-patch-request-report.v2 与 Report Panel consumer 同步。
原 initial/repaired Import/Review/Confirm/transaction 链无回归。
```

### 6.6 失败即停

任一 operation variant 没有 typed policy、parse-failed 可进入 Build/destructive、或 report schema 未同步时不进入 Gate B。不得用更宽 operation cap 或字符串 target 比较换取测试通过。

### 6.7 Gate A 施工记录（2026-07-11）

状态：`已完成`。

实际实现：

```text
新增 project_patch/repair_scope.rs，形成 validate_repair_scope 单一 interface。
全部 46 个当前 PatchOperation variant 由穷尽 typed match 生成 kind/anchor/destructive/semantic policy。
operation id、dependency 和四类 semantic repair slot 从 typed initial patch 重新计算；diagnostic target/target_summary 不参与授权。
parseable baseline 固定 metadata/count/order/kind/anchor/未授权字段；parse-failed 固定 Low/non-destructive/non-Build/cap 8。
删除 llm_repair.rs 旧 repair_scope_guard 与 kind-string heuristic。
AI service 迁移到 RepairScopeValidation；rejected/restricted/passed evidence 进入 LlmPatchRequestReport。
llm-patch-request-report.v1 升级 v2，Report Panel consumer 与序列化测试同步。
```

实际涉及文件：

```text
rust/crates/editor_core/src/project_patch/repair_scope.rs
rust/crates/editor_core/src/project_patch/llm_repair.rs
rust/crates/editor_core/src/project_patch/llm_request.rs
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/tests/ai_service_tests.rs
rust/crates/editor_core/src/lib.rs
```

验证结果：

```text
cargo test -p editor_core repair_scope   -> 12 passed
cargo test -p editor_core llm_repair     -> 3 passed
cargo test -p editor_core llm_patch      -> 16 passed, 1 ignored env-gated smoke
cargo test -p editor_core project_patch  -> 50 passed
cargo test -p editor_core                -> 369 passed, 1 ignored env-gated smoke
cargo fmt --all -- --check               -> passed
git diff --check -- rust/crates/editor_core -> passed
```

现有 `engine_runtime` 两条 dead-code warning 与 `editor_core` 两条 unused-import warning 与 5.6 审查基线一致；Gate A 没有新增 warning。后续 Gate B-E 已按本文完成。

## 7. Gate B - Bounded Child Process Lifecycle

### 7.1 涉及文件

```text
runtime_cli/src/bounded_child_process.rs
runtime_cli/src/bin/bounded_output_fixture.rs
runtime_cli/tests/bounded_child_process.rs
runtime_cli/src/exported_player_verification.rs
runtime_cli/src/lib.rs
editor_core/src/services/build_service.rs
editor_core/src/report_panel.rs
project_e2e_gate/src/editor_build_and_run.rs
project_e2e_gate/src/exported_windows_playable_golden.rs
project_e2e_gate/src/release_package.rs
```

### 7.2 任务

1. 在 `runtime_cli` 建立一个 production interface，隐藏 spawn/read/kill/wait/join 细节；不新增 crate 或 RuntimeProcessSpawner 层。
2. spawn 后立即 take stdout/stderr 并启动两个 reader；固定 buffer，raw total 始终累计，capture 满后继续 drain 到 EOF。
3. normal/timeout/try_wait-error 均按固定顺序回收；reader panic/join error 进入 result，不静默成功。
4. 增加私有 test-only fault seam，强制 wait error 后仍走真实 cleanup；fault 类型不得出现在 public request。
5. fixture 支持 `flood-success`、`nonzero`、`timeout`，双流 flood 各至少 1 MiB，不生成 descendant process。
6. 迁移 exported player verifier 和 editor headless verification 两个 production caller；windowed detached/null stdio 等非 pipe caller 不迁移。
7. product report summary 维持最多 2,000 Unicode chars，同时记录 totalBytes/truncated 和 cleanup error evidence。
8. exported process/editor build-and-run report 升级 v2，迁移全部 serde/Report Panel/E2E consumer。

### 7.3 Gate 测试

```powershell
cd G:\gameEngin\rust
cargo test -p runtime_cli --test bounded_child_process
cargo test -p runtime_cli bounded_child_process
cargo test -p runtime_cli exported_player
cargo test -p editor_core build_and_run
cargo test -p project_e2e_gate editor_build_and_run
```

### 7.4 通过条件

```text
双流各 >=1 MiB、exit 0，不 timeout；totalBytes 正确，summary 有界，truncated=true。
nonzero 保留 exit code 和双流摘要。
timeout 与 injected wait error 都完成 kill/wait/reap/join，测试有硬超时，不挂住 cargo test。
两个 production caller 都不再自己实现 wait-then-read。
三个 report v2 consumer 无旧 schema 假设。
```

### 7.5 失败即停

reader 达到 cap 后停止 drain、任一错误路径可能遗留 child/thread、或通过无界 Vec/String 保存 1 MiB 输出时不进入 Gate C。

### 7.6 Gate B 施工记录（2026-07-11）

状态：`已完成`。

实际实现：

```text
新增 runtime_cli::bounded_child_process 深 module，以单一 interface 封装 spawn、双流并发 drain、有界保留、timeout、kill、wait/reap 与 reader join。
stdout/stderr 使用固定 16 KiB buffer，raw total 始终累计；达到 capture cap 后继续 drain 到 EOF。
新增独立 bounded_output_fixture，覆盖双流各 1,048,833 bytes flood-success、exit 23 nonzero 和硬超时。
新增私有 test-only wait fault seam，证明 try_wait error 后仍执行 kill/wait/reap/join；fault 类型未进入 public request。
exported player verifier 与 editor headless verification 均迁移到同一 production interface；windowed detached/null-stdio 路径保持不变。
exported-player-process-verification-report.v1 升级为 v2；editor-build-and-run-report.v1 与其 process summary 升级为 v2。
两份 product report 的 stdout/stderr 文本继续限制为最多 2,000 Unicode chars，并新增 totalBytes/truncated 与 cleanup error evidence。
Report Panel 继续通过 schema constant 消费 v2；project_e2e_gate 的 editor build-and-run、golden 和 release consumers 通过 typed report 同步。
```

实际涉及文件：

```text
rust/crates/runtime_cli/src/bounded_child_process.rs
rust/crates/runtime_cli/src/bin/bounded_output_fixture.rs
rust/crates/runtime_cli/tests/bounded_child_process.rs
rust/crates/runtime_cli/src/exported_player_verification.rs
rust/crates/runtime_cli/src/lib.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/tests/build_service_tests.rs
```

验证结果：

```text
cargo test -p runtime_cli --test bounded_child_process -> 4 passed
cargo test -p runtime_cli bounded_child_process        -> 1 passed
cargo test -p runtime_cli exported_player              -> 6 passed across matching targets
cargo test -p editor_core build_and_run                 -> 4 passed
cargo test -p project_e2e_gate editor_build_and_run     -> 1 passed
cargo fmt --all -- --check                              -> passed
```

现有 `engine_runtime` 两条 dead-code warning 与 `editor_core` 两条 unused-import warning 仍和 5.6 审查基线一致；Gate B 没有新增 warning。下一步只执行 Gate C。

## 8. Gate C - Stable Atomic Publish Lock

### 8.1 涉及文件

```text
engine_runtime/src/atomic_directory_publish.rs
project_e2e_gate/src/bin/atomic_publish_fixture.rs
project_e2e_gate/src/critical_correctness_safety.rs
project_e2e_gate/src/lib.rs
editor_core/src/release_package.rs（仅消费者回归需要时）
```

### 8.2 任务

1. `AtomicDirectoryPublishGuard` 保持外部 interface 不变；OpenOptions 显式 `truncate(false)`。
2. Drop 只 unlock/close，不删除 lock path；移除不再需要的 path ownership 字段。
3. 建立三个独立 OS process fixture：A 持锁，B/C 在 A->下一 owner 交接期间重试竞争。
4. 每个进入 critical section 的进程使用 `create_new` overlap sentinel 检测双 owner；检测到 overlap 立即失败，不继续发布。
5. 重复多轮 handoff；B/C 每轮发布带 sequence/hash 的合法 payload，coordinator 按完成事件确认最终目录来自最后一次合法 publish。
6. 验证最终目录可由正式 RuntimePackage loader/release verifier 对应消费路径加载，manifest/content/payload hash 自洽。
7. 验证稳定 lock file 可以存在，但不在 final directory inventory/hash 内。

### 8.3 Gate 测试

```powershell
cd G:\gameEngin\rust
cargo test -p engine_runtime atomic_directory_publish
cargo test -p engine_runtime runtime_package_publish
cargo test -p editor_core release_package
cargo test -p project_e2e_gate atomic_publish
```

### 8.4 通过条件

```text
三 OS process、多轮 handoff、active publisher <= 1。
release 后稳定 lock path 仍存在，后续 acquire 继续锁同一协调对象。
RuntimePackage/release 原子替换、rollback、last-good 和 hash 回归通过。
```

### 8.5 失败即停

不得用单进程 mutex、仅线程测试、删除 lock file 或把 lock file 塞进 payload inventory 来规避三进程 Gate。失败时不进入 Gate D。

### 8.6 Gate C 施工记录（2026-07-11）

状态：`已完成`。

实际实现：

```text
AtomicDirectoryPublishGuard 外部 interface 保持不变；OpenOptions 显式 truncate(false)。
guard 只持有稳定 lock File；Drop 只 unlock/close，不再删除 lock path。
新增 atomic_publish_fixture 独立 OS process fixture，以及三进程、六轮、共十八次成功 publish 的 handoff test。
每个进程在受锁 staging write 中以 create_new 获取 root-level overlap sentinel；出现双 owner 时立即以 atomic_publish_overlap_detected 失败。
每次 final validation 成功后在锁内追加 owner/payloadHash completion event；coordinator 以最后一次合法 completion 校验最终 payload 和 SHA-256。
稳定 .published.publish.lock 位于 final directory 旁，测试证明它保持存在且不进入 final payload inventory/hash。
现有 RuntimePackage publish、rollback、last-good 与 release package consumer 回归继续通过。
```

实际涉及文件：

```text
rust/crates/engine_runtime/src/atomic_directory_publish.rs
rust/crates/project_e2e_gate/src/bin/atomic_publish_fixture.rs
rust/crates/project_e2e_gate/tests/atomic_publish.rs
```

验证结果：

```text
cargo test -p engine_runtime atomic_directory_publish -> 2 passed
cargo test -p engine_runtime runtime_package_publish  -> 2 passed
cargo test -p editor_core release_package             -> 7 passed
cargo test -p project_e2e_gate atomic_publish          -> 1 passed
cargo fmt --all -- --check                             -> passed
```

Gate C 没有新增 warning。下一步只执行 Gate D。

## 9. Gate D - Full PE Resource Contract

### 9.1 涉及文件

```text
editor_core/src/windows_executable_resources.rs
editor_core/src/release_package.rs
project_e2e_gate/src/release_package.rs
project_e2e_gate/src/critical_correctness_safety.rs
```

### 9.2 任务

1. 建立 `WindowsExecutableResourceExpectation`，集中完整字段列表和 acceptance。
2. stamp 后 readback 与 final `verify_release_package_directory` 调用同一 `verify_windows_executable_resource_contract`。
3. 删除 final verifier 的手写部分字段比较；错误转换保持 code/stage/path/next_action。
4. 正常合同同时验证 ProductName、CompanyName、FileDescription、ProductVersion/FileVersion strings、fixed file/product versions、copyright、OriginalFilename、六个 icon size 和 application manifest。
5. 建立逐字段 PE mutation matrix，包括每个 string、两组 fixed version、GROUP_ICON/单一 required size、manifest。
6. 每次 mutation 后重读 exe，更新 manifest 中 entrypoint size/SHA-256，再用正式 `release_payload_hash` 重算 `releasePayloadHash`，使 inventory/hash 完全自洽。
7. mutation package 必须因 `release_resource_readback_mismatch` 或等价稳定 resource diagnostic 失败，而不是先因 payload hash mismatch 失败。
8. source template 保持不变；仍只修改 staging/release fixture copy。

### 9.3 Gate 测试

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core windows_executable_resources
cargo test -p editor_core release_package
cargo test -p project_e2e_gate release_package
cargo test -p project_e2e_gate pe_contract
```

### 9.4 通过条件

```text
stamper/final verifier 只有一个 PE acceptance interface。
完整 mutation matrix 中每个 package 的 manifest/inventory/hash 都自洽。
每个 mutation 被 resource diagnostic 独立拒绝，正常 package 继续通过。
237 portable layout、manifest-driven entrypoint 和 payload hash 真相不变。
```

### 9.5 失败即停

若 mutation 只被 hash mismatch 拒绝、任一字段未纳入共享 contract、或出现第二 PE parser，不进入 Gate E。

### 9.6 Gate D 施工记录（2026-07-11）

状态：`已完成`。

实际实现：

```text
新增 WindowsExecutableResourceExpectation，并建立 verify_windows_executable_resource_contract 单一 acceptance interface。
stamp_windows_executable_resources 的写后 readback 与 verify_release_package_directory 的 final verification 均调用同一 interface；删除 final verifier 的手写部分字段比较。
共享 contract 完整验证 ProductName、CompanyName、FileDescription、ProductVersion/FileVersion strings、LegalCopyright、OriginalFilename、fixed file/product versions、16/32/48/64/128/256 icon sizes 与 application manifest presence。
staging template 已有 manifest 时原样保留；模板缺失时只向 staging copy 写入标准 asInvoker application manifest，source template 保持不变。
新增真实 release package PE mutation matrix，逐项覆盖七个 version string、两个 fixed version、单个 required icon size 与 application manifest。
每次 mutation 后重算 entrypoint size/SHA-256，并通过正式 release_payload_hash 重算 releasePayloadHash。
十一项 mutation 的 manifest/inventory/hash 全部自洽，均因 release_resource_readback_mismatch 独立失败，没有先出现 payload hash/inventory diagnostic。
```

实际涉及文件：

```text
rust/crates/editor_core/src/windows_executable_resources.rs
rust/crates/editor_core/src/release_package.rs
rust/crates/editor_core/src/lib.rs
rust/crates/project_e2e_gate/Cargo.toml
rust/crates/project_e2e_gate/tests/pe_contract.rs
```

验证结果：

```text
cargo test -p editor_core windows_executable_resources -> 2 passed
cargo test -p editor_core release_package              -> 7 passed
cargo test -p project_e2e_gate release_package         -> 1 passed
cargo test -p project_e2e_gate pe_contract              -> 1 passed (11 mutations)
cargo fmt --all -- --check                              -> passed
```

Gate D 没有新增 warning。下一步只执行 Gate E。

## 10. Gate E - Aggregate Evidence / Full Regression / Docs

### 10.1 涉及文件

```text
project_e2e_gate/src/critical_correctness_safety.rs
project_e2e_gate/src/lib.rs
正式方案、入口文档、施工索引、阶段完成记录
```

### 10.2 任务

1. 生成 validation-only `critical-correctness-safety-gate-report.v1`；只汇总四个正式 interface 的 evidence，不复制 validator/process/publish/PE implementation。
2. 聚合字段固定为 schemaVersion/status/repairScopeSummary/processLifecycleSummary/publishLockSummary/peContractSummary/diagnostics/nextActions。
3. 报告只存在于 test/Gate Trace，不进入 Runtime hot path，不新增 Report Panel provider。
4. 运行所有定向 Gate 和完整 workspace 回归。
5. 只有全部通过后生成 `阶段完成记录/2026-07-11-Critical-Correctness-and-Safety-Convergence-Gate-v1/00-总览.md`。
6. 完成后更新 239 正式方案实际结果、00/49/54/227、施工文档/README 和阶段完成记录/README。
7. 全部完成后把本文档从 `施工文档/当前/` 移到 `施工文档/已完成/`；失败时保持当前，不伪装完成。

### 10.3 定向复验

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core repair_scope
cargo test -p runtime_cli --test bounded_child_process
cargo test -p engine_runtime atomic_directory_publish
cargo test -p project_e2e_gate critical_correctness_safety
cargo test -p editor_core release_package
```

### 10.4 整体回归

```powershell
cd G:\gameEngin\rust
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

当前严格 Clippy/Hygiene 有已知大规模历史债务，不作为 239 完成条件；239 不得新增 warning。不得为让 239 通过而顺手展开 CQ-07/CQ-08。

### 10.5 通过条件

```text
Gate A-D 每项 acceptance 都有结构化 evidence。
aggregate report status=passed 且无 error diagnostic。
fmt、default workspace、all-features workspace 全通过。
完成记录、入口同步、施工归档全部完成。
```

### 10.6 失败即停

任一全量回归失败时先修 239 引入的回归；若失败属于明确历史债务，必须给出与本轮无关的证据并保持本文档未完成，不能删除测试或放宽合同。

### 10.7 Gate E 施工记录（2026-07-11）

状态：`已完成`。

实际实现：

```text
新增 project_e2e_gate::critical_correctness_safety validation-only aggregate module。
critical-correctness-safety-gate-report.v1 固定包含 schemaVersion/status/repairScopeSummary/processLifecycleSummary/publishLockSummary/peContractSummary/diagnostics/nextActions。
aggregate 只消费 RepairScopeValidation、BoundedChildProcessResult 与 Gate C/D 的结构化摘要，不复制 validator/process/publish/PE implementation。
四段 evidence 全部通过时 status=passed、diagnostics/nextActions 为空；任一 acceptance 失败时 fail closed。
没有新增 Runtime/Editor persistent provider，也没有接入 Report Panel。
```

验证结果：

```text
cargo test -p project_e2e_gate critical_correctness_safety -> 2 passed
cargo test -p editor_core repair_scope                    -> 12 passed
cargo test -p runtime_cli --test bounded_child_process    -> 4 passed
cargo test -p engine_runtime atomic_directory_publish     -> 2 passed
cargo test -p editor_core release_package                 -> 7 passed
cargo fmt --all -- --check                                -> passed
cargo test --workspace                                    -> passed
cargo test --workspace --all-features                     -> passed
```

两套 workspace 回归均包含 370 个 editor_core、430 个 engine_runtime、44 个 project_e2e_gate 单元测试，以及全部 integration/doc tests；runtime_player_winit 两个真实 OS window/GPU smoke 保持 local-only ignored。现有四条 5.6 基线 warning 不变，239 没有新增 warning。

阶段完成记录：

```text
阶段完成记录/2026-07-11-Critical-Correctness-and-Safety-Convergence-Gate-v1/00-总览.md
```

入口文档已同步；本文档归档后 `施工文档/当前/` 为空。下一步先讨论 CQ-04 SafeProjectPath / symlink-junction write containment，确认方案前不直接施工。

## 11. Diagnostic 最小集合

### Repair

```text
repair_scope_operation_count_expanded
repair_scope_operation_kind_changed
repair_scope_target_changed
repair_scope_unauthorized_field_changed
repair_scope_dependency_expanded
repair_scope_destructive_or_build_expanded
repair_scope_risk_expanded
repair_scope_capability_expanded
repair_scope_metadata_changed
repair_scope_unprovable_limit_exceeded
repair_scope_authorization_ambiguous
```

### Process

```text
process_spawn_failed
process_timeout
process_wait_failed
process_kill_failed
process_reap_failed
process_reader_join_failed
```

### Publish / PE

```text
output_publish_busy
output_publish_lock_open_failed
atomic_publish_overlap_detected                 // validation-only fixture
release_resource_readback_mismatch
```

所有 product diagnostic 继续提供 source path/domain/stage/message/next action；validation-only code 不进入 Runtime 常驻 report。

## 12. 工作树与安全规则

```text
当前工作树已有用户/生成产物改动；不回退、不覆盖、不清理无关文件。
不修改 samples/complex_shooter_project/Build/Windows/dev 下现有报告，除非正式 Gate 自己生成并在完成记录说明。
测试 fixture 使用独立 temp directory；不把故障 mutation 写回样例 source asset 或正式 template。
不执行 git reset --hard、git checkout -- 或递归删除未验证路径。
每个 Gate 开始前查看 git status；只处理本 Gate 文件。
```

## 13. 施工文档自审

施工前审查结论：`通过，可以从 Gate A 开始施工`。

```text
[x] 当前目录在生成前为空，本文档是唯一当前施工文档。
[x] 用户已确认 B-min+，正式 239 方案已落盘。
[x] 两份 5.6 审查已完整读取并按四类写入本文档。
[x] 方案审查发现的四项缺口已先修改正式方案，不是只写在最终总结。
[x] Gate A-D 一一对应 INC-01、CQ-03、CQ-05、INC-03，没有扩入其它审查项。
[x] 每 Gate 有精确文件、任务、测试、通过条件和失败即停。
[x] process wait-error 有内部 fault seam，1 MiB flood 有真实 OS process fixture。
[x] atomic lock 使用三个独立 OS process，不用线程测试替代。
[x] PE mutation 重算 manifest inventory/hash，能独立证明资源合同。
[x] required report shape 变化明确升级 v2 并迁移消费者。
[x] RuntimePackage/ProjectPatch 真相、Off/Summary/Trace 分档和 no-new-crate 边界保持不变。
[x] 完整回归通过前不生成完成记录、不更新为已完成、不归档本文档。
```

没有发现与 239 正式方案冲突、范围扩大、测试缺口或归档条件不清的问题。Gate A 已按本文完成；下一步只能执行 Gate B。
