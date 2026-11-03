# 251 当前可自动化施工文档：Provider-independent From-Blank Creation Golden Gate v1

> 状态：已完成并归档  
> 对应方案：`251-Provider-independent-From-Blank-Creation-Golden-Gate-v1方案.md`  
> 激活日期：2026-07-16  
> 专用 target：`G:\gameEngin-targets\251`

## 1. 目标

以一个深 `run_from_blank` Interface 完成真实空项目创建、provider-independent imported Codex Candidate construction、Preview、Windows 外部交付和可比较证据。旧 250-F validation-only 只作历史基线，不得作为本次通过条件。

## 2. Gate A：基线与施工授权

已执行：

```text
cargo test -p project_e2e_gate c01_golden_gate_builds_all_ten_candidates_through_common_entry：1 passed
cargo test -p editor_core project_candidate_entry：10 passed
rustc/cargo=1.96.0
G free=约 36 GiB；I free=约 406 GiB
git diff --check：仅既有 LF/CRLF warning
当前/ 原为空；待执行/ 仅 README；250-A..F 已归档
```

## 3. Gate B：Imported Candidate 与 C-01 时间证据

施工：

```text
C-01 construction 的每个 envelope 先序列化为 strict JSON，再通过 ProjectCandidateEntry::from_json_string。
保存 imported source evidence，并验证 source/envelope/candidate digest。
C01 report 升级并记录 construction、first playable、export、total 与 external wait 时间。
内部 approval actor 绑定一次 goal approval identity。
```

测试：

```powershell
cargo test -p project_e2e_gate c01_imported_codex
cargo test -p project_e2e_gate c01_golden_gate_builds_all_ten_candidates
cargo test -p editor_core project_candidate_entry
```

## 4. Gate C：From-Blank 深 Module 与 CLI

施工：

```text
新增 C01FromBlankCreationRequest/Report 与 run_c01_from_blank_creation_gate。
严格验证 root absent、路径互斥、输出 absent、Engine SDK source snapshot。
通过正式 CreateProject command 建立空项目。
记录一次目标授权，调用现有 C-01 owner，禁止 validation-only fallback。
新增 CLI mode c01-from-blank-creation；所有必需路径和 approved-by 显式传入。
失败也持久化结构化 wrapper report。
```

测试：

```powershell
cargo test -p project_e2e_gate c01_from_blank_preflight
cargo test -p project_e2e_gate c01_from_blank_creation
cargo test -p project_e2e_gate c01_from_blank_rejects_existing
```

否定矩阵：已有/非空项目、旧 candidate/evidence、嵌套路径、Engine SDK 内输出、approval 缺失/篡改、CreateProject 失败、内部 report 非 creation mode、Engine source drift。

## 5. Gate D：Windowed Performance Phase Breakdown

施工：

```text
WindowedPlayerFramePerformanceSummary 保留 total，并新增 update/render_submit/present_wait phase summary。
headless present_wait 明确为 0；真实 WGPU window 把 surface acquire/present 等待与 render submit 分开计时。
每项输出 mean/P95/P99；sample 数必须与 600 帧合同一致。
```

测试：

```powershell
cargo test -p runtime_player_winit frame_performance
cargo test -p runtime_cli exported_player
cargo test -p project_e2e_gate c01_performance
```

## 6. Gate E：受影响域预检

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-targets\251'
cargo test -p project_e2e_gate --lib
cargo test -p editor_core project_candidate_entry
cargo test -p runtime_player_winit
cargo test -p runtime_cli
cargo fmt --all -- --check
git diff --check
```

先运行累计失败集，再进入真实 Gate；不得用真实 Gate 调试编译错误。

## 7. Gate F：真实连续 From-Blank Gate

预检四个输出根不存在，再运行 CLI `c01-from-blank-creation`。真实根固定为方案第 6 节。执行期间不修改引擎源码，不读取旧 candidate，不逐 Gate 请求用户确认。

权威报告：

```text
G:\gameEngin-targets\251-r2\evidence\c01-from-blank-creation-report.json
```

必须验证 creation mode、fresh=10/reused=0、一次确认、首次可玩/总耗时、Engine source unchanged、Preview/Export/外部运行、8/8、真实像素和 120+600 phase performance。

R1 于 2026-07-16 在 43.5 秒处失败：Runner 未继承 `CARGO_TARGET_DIR`，Player artifact 默认 build root 落入 Engine SDK，隔离合同返回 `project_runtime.player_artifact_build_root_not_isolated`。R1 全部保留；R2 使用新根，并以 `--prior-attempt-report G:\gameEngin-targets\251\evidence\c01-from-blank-creation-report.json` 记录 repair=1，不复用 R1 项目或候选。

## 8. Gate G：最终回归与归档

真实候选代码与证据冻结后：

```powershell
cargo test --workspace
cargo test --workspace --all-features
```

全部通过后生成完成记录，更新 00/49/54/240/施工 README/阶段完成 README，并把本文移动到 `施工文档/已完成/`。任何源码、manifest、lockfile 或 Gate owner 修改都会使真实 Gate 与最终回归重新失效。

## 9. 施工自审

```text
方案一致：通过；只执行 251。
当前唯一性：通过；激活前 current 为空、pending 仅 README。
基线：通过；现有 construction 与 Candidate Entry 测试为绿。
冻结边界：通过；旧三引擎项目和证据不写。
失败纪律：通过；分层预检、first blocker、累计失败集、一次权威真实 Gate。
产品边界：通过；玩法留在 C-01 fixture/候选，通用引擎只增加报告与性能采样。
完整性：通过；真实窗口、截图、external package 与最终双 workspace 不可降级。
```

结论：本文已激活，可从 Gate B 开始施工。

## 10. 最终执行结果

R1 保留隔离 build root 首阻断；R2 以全新根通过 `creation_mode`，10 个 fresh imported Codex Candidate、0 reused、1 次目标确认、1 次 repair、Engine source unchanged。首次可玩 95.946 秒，总交付 120.589 秒；Preview/Export、8/8、真实像素和 120+600 phase performance 全部通过。default/all-features workspace 最终均通过。权威细节见 `阶段完成记录/2026-07-16-Provider-independent-From-Blank-Creation-Golden-Gate-v1/00-总览.md`。
