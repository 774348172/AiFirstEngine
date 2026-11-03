# 250-C 当前可自动化施工文档：Controlled SourcePatch v1

> 状态：已完成并归档  
> 对应方案：`250-AI-Primary-ProjectProduction-Dual-Path-v1方案.md` Phase 3  
> 前置：250-A / 250-B 已完成并归档  
> 生成日期：2026-07-14
> 激活日期：2026-07-14
> 完成日期：2026-07-14

## 1. 目标

建立深 `ControlledSourcePatch` Module。调用者只提交结构化 SourcePatch、项目根、候选 store 和受信 Engine SDK locator；Module 隐藏路径策略、Cargo manifest policy、候选 lowering、编译验证、显式批准绑定、能力约束 Apply、失败恢复和 rollback。

本阶段只处理 project-owned Rust RuntimeModule 和把 `project.aife.json` 切换为 `ProjectRust` 所需的受控 manifest 变更。Provider、Dev Host、Asset Import、任意工程文件写入和 C-01 Golden Gate 不在范围内。

## 2. Interface

```text
ControlledSourcePatch::prepare(request) -> ControlledSourcePatchCandidate
ControlledSourcePatch::validate(candidate, validation_request) -> ControlledSourcePatchValidationReport
ControlledSourcePatch::apply(approved_candidate) -> ControlledSourcePatchApplyReceipt
ControlledSourcePatch::rollback(apply_receipt, project_root) -> ControlledSourcePatchRollbackReceipt
```

`prepare` 是 SourcePatch policy 的唯一入口，并在调用 `CandidateProjectRevisionStore::stage` 前完成 fail-closed validator。不得让调用者直接把 AI/用户文本变成 `CandidateFileChange` 绕过该 Interface。

## 3. v1 SourcePatch policy

只允许 UTF-8 CreateOrReplace/Delete：

```text
project.aife.json
RuntimeModule/Cargo.toml
RuntimeModule/src/**/*.rs
RuntimeModule/tests/**/*.rs
```

硬限制：

```text
最多 64 operations；单文件最多 1 MiB；总文本最多 4 MiB。
路径必须经过 ProjectRelativePath，规范化、唯一并排序。
project.aife.json 必须解析为 aife-project.v2，sourceKind=projectRust，cargoManifest 固定为 RuntimeModule/Cargo.toml。
Cargo package 必须与 project manifest cargoPackage 一致。
禁止 package.build、build.rs、workspace inheritance、workspace/patch/replace、target-specific dependencies、build-dependencies、git/path/registry dependency 和 crate-type/dylib。
v1 依赖只允许 engine_runtime / engine_input 的精确 engine version 字符串；验证时由受信 EngineSdkLocator 写入 validation-copy 内部的 Cargo patch config。
删除 project.aife.json、Cargo.toml 或 src/lib.rs 一律拒绝。
```

## 4. Gate A：schema、allowlist 与 lowering

实现 schema-first 对象：SourcePatch document/operation、candidate、diagnostic、validation report、approval、apply/rollback receipt。所有记录绑定 patch id、revision id、base/candidate digest、changed paths 和 schema version。

测试：

```powershell
cargo test -p editor_core controlled_source_patch::tests::policy
cargo test -p editor_core controlled_source_patch::tests::prepare
```

进入条件：250-B 定向基线通过。失败后只修 schema/policy/prepare 累计失败集，不进入子进程验证。

## 5. Gate B：隔离 validation copy 与 Engine SDK locator

```text
验证前重新调用 CandidateProjectRevisionStore::verify_base，拒绝 base drift 和 candidate tampering。
复制 RuntimeModule 到 candidate store 内部的临时 validation root；任何结果都清理，不修改 candidate digest。
EngineSdkLocator 只能来自 trusted request，不来自 SourcePatch；必须解析 engine_runtime / engine_input Cargo.toml。
使用 runtime_cli::run_bounded_child_process，限制 stdout/stderr 和 deadline。
固定顺序：cargo fmt --check -> cargo check --all-targets -> cargo test --no-run。
默认不执行候选测试代码；TrustedLocalExecuteTests 需要调用者显式授权并追加 cargo test。
```

这里的目录隔离不是恶意代码安全沙箱。Rust test execution、编译器、受信 SDK 的 transitive build dependency 仍拥有当前用户进程权限；report 必须明确 execution policy，禁止把 passed 解读为安全 sandbox 证明。

测试：

```powershell
cargo test -p editor_core controlled_source_patch::tests::validation
cargo test -p runtime_cli --test bounded_child_process
cargo test -p editor_core controlled_source_patch::tests::validation::validates_against_workspace_engine_sdk -- --ignored --exact
```

## 6. Gate C：显式批准、事务 Apply 与 rollback

```text
approval 必须精确绑定 passed validation report、revision id、candidate digest 和 validation digest。
Apply 前再次验证 candidate 与 base；base drift fail closed。
只从已验证 candidate 读取 changed paths；写入使用 ProjectWriteScope write_atomic/remove_file。
Apply 前持久化 before snapshot；任一写入失败立刻反向恢复。
Apply 后 actual project digest 必须等于 candidate digest，否则立即恢复并失败。
成功 receipt 保存 before/after digest、changed paths 和受限 rollback record path。
rollback 只接受原 receipt；当前项目 digest 必须等于 applied digest，漂移则拒绝静默覆盖。
rollback 完成后 digest 必须回到 before digest，并清理 rollback record。
```

测试：

```powershell
cargo test -p editor_core controlled_source_patch::tests::apply
cargo test -p editor_core controlled_source_patch::tests::rollback
cargo test -p editor_core project_write_containment
```

## 7. Gate D：受影响域回归

```powershell
cargo test -p editor_core candidate_project_revision
cargo test -p editor_core project_readiness
cargo test -p editor_core project_patch
cargo test -p editor_core project_consistency
cargo test -p editor_core
cargo test -p project_e2e_gate second_project
```

## 8. Gate E：最终权威矩阵

使用仓库外隔离 target，不关闭用户当前编辑器：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-targets\250c'
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

default/all-features 各只在候选冻结、累计失败集和受影响域全绿后运行一次。代码、manifest、lockfile 或测试在矩阵后变化，必须重新预检并明确旧矩阵失效。

## 9. 涉及文件

```text
rust/crates/editor_core/src/controlled_source_patch.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/Cargo.toml（仅在需要结构化 TOML parser 时）
rust/Cargo.lock（仅依赖解析产生的机械变化）
必要的现有 CandidateProjectRevision / ProjectWriteScope 小型共享 Interface 修正
入口文档、完成记录和施工归档
```

禁止修改 Provider、Editor UI、RuntimePackage、Renderer、AUI、Dev Host 或玩法专用 Project Module。

## 10. 高风险清单

```text
目录逃逸、大小写/\\?\/8.3 路径差异、symlink/junction/reparse。
manifest 伪装、build.rs、path/git dependency、workspace/patch 注入、crate-type 动态库。
候选验证期间 base drift、candidate tampering、Cargo.lock/target 污染 candidate。
validation temp/staging/target 未清理，stdout/stderr 无界，子进程 timeout 未 reap。
approval 重放、validation 与 candidate 不绑定、Apply 前后 digest 不一致。
多文件部分 Apply、rollback 失败、rollback 后 base 漂移、删除范围扩大。
把 TrustedLocal cargo test 误称为安全 sandbox。
```

## 11. 文档同步与归档

全部 Gate 通过后：

```text
写 阶段完成记录/2026-07-14-Controlled-SourcePatch-v1/00-总览.md。
将本施工文档从 当前/ 移到 已完成/。
同步 00-文档地图、49、54、240、247、250、施工文档/README、阶段完成记录/README。
明确 Phase 4 Asset Import、Phase 5 Provider-independent Candidate Entry、Phase 6 C-01 仍未施工。
```

## 12. 自审

```text
符合 250 Phase 3：通过。
复用 CandidateProjectRevision / ProjectWriteScope / bounded child process，不新增平行路径 validator 或子进程 runner：通过。
Interface 保持 prepare/validate/apply/rollback，复杂度留在 Module 内：通过。
SourcePatch 只覆盖 project-owned RuntimeModule 和受控 project manifest，不扩大到任意项目文件：通过。
不新增玩法专用 Core Interface，不扩大 Rule IR：通过。
Provider、Dev Host、Asset Import、C-01 未提前进入：通过。
默认 validation 不执行未受信候选代码；TrustedLocal 执行边界已显式记录：通过。
三层测试、环境隔离、失败收敛、权威运行和文档归档完整：通过。
读取 250、241、242、250-A/B 完成记录后，无需修改正式方案；本施工只细化既有 Phase 3 约束：通过。
```

## 13. 激活前复核条件

```text
施工文档/当前/ 为空。
250-B 归档和完成记录存在。
candidate_project_revision、ProjectWriteScope、bounded child process 定向基线通过。
代码基线仍不存在 production SourcePatch Interface。
工作树混合修改保持原样，使用隔离 target，不覆盖用户修改。
```
