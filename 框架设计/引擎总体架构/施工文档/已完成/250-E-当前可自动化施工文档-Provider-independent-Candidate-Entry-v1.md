# 250-E 当前可自动化施工文档：Provider-independent Candidate Entry v1

> 状态：已生成、自审并激活；当前唯一施工文档  
> 对应方案：`250-AI-Primary-ProjectProduction-Dual-Path-v1方案.md` Phase 5  
> 前置完成：250-A ProjectReadiness、250-B CandidateProjectRevision、250-C Controlled SourcePatch、250-D Formal Asset Import  
> 生成日期：2026-07-14  
> 激活日期：2026-07-14

## 1. 施工目标

新增 Rust 主线唯一 `ProjectCandidateEntry` 深 Module，把候选来源与工程修改能力解耦：

```text
Built-in Provider output / Imported Codex JSON / Imported file / Test fixture
  -> ProjectCandidateEnvelope
  -> prepare
  -> ProjectCandidate
  -> validate
  -> ProjectCandidateValidationReport
  -> explicit approval
  -> apply
  -> ProjectCandidateApplyReceipt
  -> rollback
```

`ProjectCandidateEnvelope` v1 每次只允许一种 typed payload：

```text
ProjectPatch
ControlledSourcePatch
AssetImport
```

内置 Provider 与外部 Codex 不再拥有不同 proposal/apply 真相。它们只负责把输出适配为同一 envelope；之后统一绑定 project id、base project digest、语义 context、candidate digest、validation digest 和 approval。

## 2. v1 schema 与边界

核心 schema：

```text
project-candidate-envelope.v1
project-candidate.v1
project-candidate-validation-report.v1
project-candidate-approval.v1
project-candidate-apply-receipt.v1
project-candidate-rollback-receipt.v1
project-candidate-project-binding.v1
```

Envelope 必须包含：

```text
candidateId
sourceKind = built_in_provider | imported_codex | imported_file | test_fixture
sourceLabel
targetProjectId
expectedBaseProjectDigest
payload（严格 tagged enum，恰好一个）
```

规则：

```text
candidateId / sourceLabel 有长度与字符上限。
JSON/file 输入有 8 MiB 总上限，只接受 regular UTF-8 file；不跟随 directory/symlink 冒充候选文件。
source digest、envelope digest、payload digest、candidate digest 使用 canonical SHA-256。
prepare 前复核 active project、project id、project root 与 expected base digest。
ProjectPatch 额外绑定 ProjectPatchLlmContextSnapshot.context_hash。
ControlledSourcePatch 只降低到 250-C 的 allowlist/candidate/compile/test 管线。
AssetImport 只降低到 250-D 的 PNG/import/provenance/license/transaction 管线，并复核 expected source hash。
同一 envelope 不支持多 payload 原子批次；多候选按顺序逐个 review/apply，避免伪造跨三套事务的原子性。
```

外部候选文件是只读输入，不因此获得项目外写权限。`ProjectWriteScope` 仍只是 containment，不是恶意候选、编译器、PNG decoder 或同用户权限代码的安全 sandbox。

## 3. 深 Module Interface

```text
inspect_project_binding(project_root) -> ProjectCandidateProjectBinding
from_json_string(session, request) -> ProjectCandidate
from_file(session, request) -> ProjectCandidate
prepare(session, request) -> ProjectCandidate
validate(session, candidate, context) -> ProjectCandidateValidationReport
apply(session, candidate + validation + approval) -> ProjectCandidateApplyReceipt
rollback(session, apply receipt) -> ProjectCandidateRollbackReceipt
```

Module 隐藏：bounded read、strict parse、canonical digest、project binding、source adapter、lowering、validation dispatch、approval mapping、apply/rollback dispatch 和 receipt tamper detection。

ProjectPatch rollback 复用已有 inverse patch，但必须验证它仍是当前最后一笔 candidate apply，并复核当前 applied context；ControlledSourcePatch 与 AssetImport rollback 直接委托各自已验证 receipt，不复制事务实现。

## 4. Consumer 收敛

现有 EditorSession 保留兼容命令：

```text
GenerateProjectPatchFromPrompt
ImportProjectPatch / PreviewImportedProjectPatch
ApplyImportedProjectPatch
```

但实现改为：

```text
provider raw ProjectPatch -> BuiltInProvider envelope adapter
legacy raw/file ProjectPatch -> ImportedCodex / ImportedFile envelope adapter
两者 -> ProjectCandidateEntry prepare/validate -> 同一种 stored proposal
Apply -> ProjectCandidateEntry approval/apply
```

Provider 请求失败只终止该 Provider request，不删除、不禁用 imported candidate 命令，也不改变候选 schema。现有 ProjectPatchImportService 保留为 ProjectPatch payload validator，不再作为顶层来源真相。

## 5. 预计代码文件

```text
rust/crates/editor_core/src/project_candidate_entry.rs                 new deep module
rust/crates/editor_core/src/lib.rs                                     exports
rust/crates/editor_core/src/candidate_project_revision.rs              public project digest binding
rust/crates/editor_core/src/project_patch/import.rs                    bounded legacy file adapter
rust/crates/editor_core/src/project_patch/session.rs                   bound inverse rollback helper
rust/crates/editor_core/src/project_patch/history.rs                   exact last-entry removal helper
rust/crates/editor_core/src/session.rs                                 one candidate proposal store
rust/crates/editor_core/src/services/ai_service.rs                     provider/import consumer migration
rust/crates/editor_core/src/tests/ai_service_tests.rs                  shared-entry evidence
rust/crates/editor_ui_model/src/ai_panel.rs                            optional candidate evidence fields
```

如果实现要求新增 Provider Registry、Planner、网络协议或 Native file picker，必须先停止并回填方案；它们不属于 Phase 5。

## 6. Gate A：Schema / Bounded Input / Project Binding

施工：

```text
strict envelope/payload/source enums
project binding inspection
canonical envelope/payload/source/candidate digest
8 MiB bounded JSON/file input
candidate id、source label、target project、base digest policy
CandidateProjectRevisionStore project digest public read-only API
```

测试：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-targets\250e'
cargo test -p editor_core project_candidate_entry_policy
cargo test -p editor_core project_candidate_entry_binding
cargo test -p editor_core candidate_project_revision
```

否定矩阵：unknown field/schema/source、空/超长 id、oversize/non-UTF8/non-file input、project id/root/base mismatch、payload tamper、source digest drift。

## 7. Gate B：三类 Payload Lowering / Validation

施工：

```text
ProjectPatch -> ProjectPatchImportService + PatchValidator + semantic context binding
ControlledSourcePatch -> 250-C prepare/validate
AssetImport -> 250-D prepare/validate + expected source hash
统一 validation report/digest/diagnostics/next actions
```

测试：

```powershell
cargo test -p editor_core project_candidate_entry_project_patch
cargo test -p editor_core project_candidate_entry_source_patch
cargo test -p editor_core project_candidate_entry_asset_import
cargo test -p editor_core controlled_source_patch
cargo test -p editor_core project_asset_import
```

三类 payload 都必须证明 prepare/validate 不修改真实项目。SourcePatch 默认只 compile/no-run；真实执行测试仍需显式 trusted policy。

## 8. Gate C：Approval / Apply / Rollback

施工：

```text
approval exact bind candidate + validation
apply 前重算 envelope/candidate/base/context/source binding
ProjectPatch transaction apply + exact-last inverse rollback
SourcePatch / AssetImport typed approval mapping and receipt delegation
common apply/rollback receipt binding digest
tamper/drift/replay fail closed
```

测试：

```powershell
cargo test -p editor_core project_candidate_entry_apply
cargo test -p editor_core project_candidate_entry_rollback
cargo test -p editor_core project_candidate_entry_tamper
cargo test -p editor_core project_write_containment
```

高风险项：approval replay、validation mismatch、base drift、semantic context drift、source drift、receipt tamper、non-last ProjectPatch rollback、nested receipt tamper、rollback 后 before state 不一致。

## 9. Gate D：Provider / Imported Consumer 收敛

施工：

```text
Provider 与 imported raw/file adapter 生成同一 envelope/candidate
Session 只保存一种 ProjectCandidate proposal
existing UI commands/evidence compatibility
provider failure 后 imported path 仍可独立 prepare/review/apply
legacy ProjectPatch import file bounded read
```

测试：

```powershell
cargo test -p editor_core ai_service
cargo test -p editor_core project_patch_import
cargo test -p editor_core llm_patch_source
cargo test -p editor_core project_candidate_entry_provider_independence
```

验收必须证明 built-in Provider candidate 与 imported Codex candidate 具有相同 candidate schema、digest/validation/approval/apply path；只允许 sourceKind/provenance 不同。

## 10. Gate E：受影响域回归

```powershell
cargo test -p editor_core
cargo test -p project_e2e_gate --lib
cargo test -p project_e2e_gate second_project
cargo test -p project_e2e_gate project_write_containment
cargo fmt --all -- --check
git diff --check
```

已有约 45 个 crate-wide Clippy finding 不扩入本阶段；250-E 自有 finding 必须修复，不新增 `allow`、`expect` 或 `cfg_attr` 抑制。

## 11. Gate F：冻结与最终权威回归

预检通过后冻结代码、manifest、lockfile 和 250-E changed-file inventory。最终统一使用：

```text
CARGO_TARGET_DIR=G:\gameEngin-targets\250e
```

权威命令各运行一次：

```powershell
cargo test --workspace
cargo test --workspace --all-features
```

当前工作树继承用户已有修改和 249/250-A/B/C/D 结果，不能冒充 clean exact commit。完成记录必须保存实际权威 run 次数、耗时、冻结文件摘要、既有 warning/ignored 项和 sample artifact 改写边界。代码或依赖若在权威 run 后变化，必须重新预检和重跑权威矩阵。

## 12. 文档同步与归档

全部 Gate 通过后：

```text
生成 阶段完成记录/2026-07-14-Provider-independent-Candidate-Entry-v1/00-总览.md。
更新 00-文档地图、49、54、240、247、250。
更新 施工文档/README 与 阶段完成记录/README。
把本文从 当前/ 移到 已完成/。
```

下一项只是 Phase 6 C-01 Golden Gate。不得自动生成、激活或施工 Phase 6。

## 13. 明确不做

```text
不做多 payload 原子 bundle、候选 merge/rebase UI 或跨候选批量 approval。
不做 Provider Registry、Agent Planner、多 Agent 调度或新增 repair loop。
不做新的网络 transport、API key 管理或 Provider 计费流程。
不做 Project Runtime Dev Host、热重载或外部编译守护进程。
不做 Native file picker、完整 Candidate Dock 或 C-01 场景内容。
不绕过 ProjectPatch、ControlledSourcePatch、ProjectAssetImport 的既有 validator/transaction。
不把外部候选文件当作受信代码，也不扩大项目外写权限。
```

## 14. 施工文档自审

```text
正式方案一致性：通过。本文细化 Candidate Source seam 和 propose(ImportedCandidate)，没有修改双通道、RuntimePackage 或 Rule IR 边界。
前置复核：通过。250-A/B/C/D 均有完成记录；当前/与待执行/在激活前为空。
代码基线：通过。ProjectPatch import、Provider、CandidateRevision、SourcePatch、AssetImport 定向基线全绿。
深度：通过。新增一个 owner 隐藏来源、绑定、dispatch 和 receipt；不复制三套 lowering/transaction。
范围：通过。单 payload v1 避免虚假的跨域原子性；Phase 6、Provider Registry、Dev Host 保持排除。
测试：通过。包含定向、受影响域、default/all-features 三层以及 provider failure/imported success 否定证明。
混合工作树：已显式记录，不声称 clean exact commit，不回滚用户修改。
```

结论：250-E 可激活施工。本文进入 `当前/` 后是唯一可施工文档；Gate A-D 未完成前不得进入整体回归，Gate F 未通过前不得归档或开始 Phase 6。
