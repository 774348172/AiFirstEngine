# 250-B 当前可自动化施工文档：CandidateProjectRevision v1

> 状态：当前唯一施工文档，已激活  
> 对应方案：`250-AI-Primary-ProjectProduction-Dual-Path-v1方案.md` Phase 2  
> 前置：250-A 已完成并归档  
> 激活日期：2026-07-14

## 1. 目标

建立 `CandidateProjectRevision` 深 Module。调用者只提供基线项目、候选隔离根和受控文件变更；Module 负责确定性基线摘要、隔离物化、变更应用、候选摘要、漂移检查、状态报告和清理。

Phase 2 不决定 Rust SourcePatch allowlist，不执行真实项目 Apply，不接 Provider，也不建设 Dev Host。

## 2. Interface

```text
CandidateProjectRevisionStore::stage(request) -> CandidateProjectRevision
CandidateProjectRevisionStore::verify_base(revision, project_root) -> BaseVerification
CandidateProjectRevisionStore::discard(revision) -> DiscardReceipt
```

`stage` 的文件变更只接受项目相对路径和 CreateOrReplace/Delete；它是内部候选物化原语，不等于用户或 AI 已获任意项目写权限。Phase 3 的 Controlled SourcePatch validator 必须先于该 Interface 调用。

## 3. 涉及文件

```text
rust/crates/editor_core/src/candidate_project_revision.rs
rust/crates/editor_core/src/lib.rs
必要的 Cargo dependency（优先复用 engine_runtime canonical digest）
入口文档、完成记录和施工归档
```

## 4. Gate A：Schema 与摘要

实现 schema-first 对象：revision id、base/candidate digest、project/candidate root、changed paths、status、diagnostics、next actions。

确定性摘要：

```text
按规范化 project-relative path 排序。
摘要绑定 path、file bytes 和 schema version。
排除 Library、Build、target、.git 和临时候选目录。
拒绝 symlink/reparse source、非 UTF 路径、候选根位于项目根内、项目根位于候选根内。
```

测试：

```powershell
cargo test -p editor_core candidate_project_revision::tests::digest
```

## 5. Gate B：隔离物化

```text
先复制到 staging sibling，再原子 rename/publish 为 revision root。
候选 root 必须全新且 revision id 唯一。
复制时排除生成目录，禁止跟随 symlink/reparse。
文件变更通过 ProjectRelativePath 校验。
任一失败清理 staging，不留下 Ready 候选。
```

测试：

```powershell
cargo test -p editor_core candidate_project_revision::tests::stage
```

## 6. Gate C：漂移、失败与清理

```text
verify_base 重新摘要真实项目；相同为 Matched，变化为 Drifted。
revision schema/project root/revision root 不一致 fail closed。
discard 只能删除受 store root 约束的精确 revision root。
重复 discard 返回 Missing，不扩大删除范围。
```

测试：

```powershell
cargo test -p editor_core candidate_project_revision
```

## 7. Gate D：受影响域

```powershell
cargo test -p editor_core project_readiness
cargo test -p editor_core project_patch
cargo test -p editor_core project_consistency
cargo test -p editor_core
cargo test -p project_e2e_gate second_project
```

## 8. Gate E：最终矩阵

使用仓库外隔离 target，避免运行中 `editor_host.exe` 锁定默认 target：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-targets\250b'
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

## 9. 高风险清单

```text
目录逃逸、父子根嵌套、symlink/junction、partial staging、覆盖已有 revision。
生成目录进入 base digest 导致无意义漂移。
Windows 路径大小写和分隔符导致不确定摘要。
真实项目在 candidate stage 期间被修改。
discard 删除 store root 之外内容。
```

## 10. 完成条件

```text
候选永不在真实项目目录原地生成。
base/candidate digest 可复现。
changed paths 与实际候选字节一致。
漂移 fail closed，discard 有严格根约束。
定向、受影响域、default/all-features 全绿。
完成记录明确 Phase 3 SourcePatch 仍未施工。
```

## 11. 自审

```text
符合 250 Phase 2：通过。
读取 45/46 后无需修改正式方案；CandidateRevision 隔离、摘要和恢复约束已进入 Gate。
不提前实现 SourcePatch policy、Provider、Apply 或 Dev Host：通过。
复用 ProjectRelativePath/canonical digest，不新增玩法概念：通过。
当前目录此前为空，本文是唯一非 README 文档：通过。
```
