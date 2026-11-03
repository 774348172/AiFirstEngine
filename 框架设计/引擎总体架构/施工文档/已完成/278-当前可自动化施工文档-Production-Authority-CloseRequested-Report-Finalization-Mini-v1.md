# 278 当前可自动化施工文档：Production Authority CloseRequested Report Finalization Mini v1

> 状态：Gate A-D、affected closure、完成记录与归档已完成
> 日期：2026-08-09
> 正式方案：`278-Production-Authority-CloseRequested-Report-Finalization-Mini-v1方案.md`
> 正式方案 SHA256：`728DD30C08B5588C810787292D4D12DC5F4FAE06B2EC514147A5704C4C7269A4`
> 选择：方案 B-Mini

## 1. 当前授权与执行边界

用户本轮明确授权：

```text
激活 278 Window A / Gate A-B
执行激活前复核后开始施工
```

本文件已移动到 `施工文档/当前/`。当前授权不包括：

- Window B / Gate C-D；
- Local CI；
- 启动真实/production Editor；
- 重跑 Tower UI-V2 Gate F/G；
- 替换 production/安装态二进制；
- 修改真实 recent/layout/trust/config；
- 生成阶段完成记录或标记 278 已完成。

激活前复核已通过；本轮只允许施工 Gate A-B，完成后必须停止并等待 Window B 单独授权。

## 2. 施工目标

施工必须完成以下闭环：

1. 修复 277 后 `editor_window_winit` test-only `GameViewRuntimeFrame` fixture 的两个字段漂移，恢复 owner 测试编译。
2. 在 `production_authority` owner 内建立可测试的 terminal finalization interface。
3. 正常完成、已知失败、CloseRequested、winit `exiting` 和 run-app return 都遵守 exactly-once report 不变量。
4. partial step reports、原始 diagnostics、scenario/report v1 schema 与正常 PASS 行为保持。
5. generated specialized Editor 缺失 report 时输出 typed failed report + exit 1，不再 panic 101。
6. 用 headless/owner/consumer tests 完成源码闭环，不把真实 Editor 或 Tower Gate F 当调试循环。
7. 完成后同步方案、240、49、54、施工索引、阶段完成索引并归档本文；Tower Gate F 保持单独授权。

## 3. 非目标与禁止范围

明确不做：

- 不重构完整 `ProductionAuthorityScenario` step engine。
- 不新增 scenario/report v2，不修改 JSON 字段。
- 不建立通用任务 Runner、录制回放或外部测试 DSL。
- 不改变普通 Native Editor 的 CloseRequested 产品行为。
- 不修改 AUI、Animator2D、Input、RuntimePackage、Renderer 或 Tower gameplay。
- 不把 Tower node/action/path 写入引擎。
- 不替换 production/安装态二进制。
- 不运行 Local CI、真实 Editor、Tower Gate F/G、release/export。
- 不 reset/revert/checkout，不覆盖 dirty worktree 中既有改动。

## 4. 激活前基线与必须复核

生成本文时：

```text
HEAD：acaffa8cb95bbb54d093bbf45835a59315c82da5
工作树：dirty-preserved
引擎当前施工：无
待执行：本文生成前为空
Tower 当前施工：UI-V2 v2，Gate F BLOCKED
```

激活前必须重新执行：

```powershell
git rev-parse HEAD
git status --short
Get-FileHash -Algorithm SHA256 `
  "框架设计/引擎总体架构/278-Production-Authority-CloseRequested-Report-Finalization-Mini-v1方案.md"
```

要求：

- 正式方案 hash 仍为本文记录值，或先修订本文并重新自审。
- `施工文档/当前/` 没有其它非 README active 文档。
- 本文是 278 唯一施工计划。
- 四个预计源码文件的当前差异已阅读并分类，不覆盖用户已有修改。
- captured trace 仍存在；若被清理，保留施工文档中的结构化失败事实，不因此重跑真实 Gate F。
- 三小时预算足够完成所授权窗口、证据与必要 cleanup。

## 5. 已知红灯与失败分类

### 5.1 Production 首因

```text
trace：G:/AifeTowerRuns/ui-v2-gate-f-20260808-163440/evidence/gate-f-final-report.json
720 structuredReportProduced=false
exitCode=101
stderr=production authority runner must produce a report
当前源码：WindowEvent::CloseRequested => event_loop.exit()
当前 caller：production_authority_report.expect(...)
分类：engine production authority lifecycle/owner 缺口
```

### 5.2 Test closure 前置失败

已执行但未进入测试：

```powershell
cargo test -p editor_window_winit --features real-window production_authority_ -- --nocapture
```

首个编译错误：

```text
E0063
rust/crates/editor_window_winit/src/editor_frame_publication.rs:152
missing aui_feedback_override_count
missing aui_feedback_profile_ids
```

分类：277 后 test-only consumer fixture 漂移。只允许机械补默认值，不修改 production frame 语义。

## 6. Ownership 与不可变接口

### 6.1 Owner

`editor_window_winit::production_authority` 是 terminal report 唯一 owner，拥有：

```text
ProductionAuthorityTerminal
finalize_once
ensure_terminal_report
fail-closed report constructor/helper
report v1 construction与持久化
```

`RealNativeEditorCaptureApp` 是 winit Adapter，只提交 terminal intent。

`editor_core::project_editor_composition_artifact` 是 generated-source producer，只调用 owner helper，不复制 report 规则。

### 6.2 必须保持的不变量

```text
scenario disabled -> no authority report
active scenario return -> exactly one terminal report
passed/failed report cannot be overwritten by exiting fallback
partial steps preserved in order
known diagnostic preserved
CloseRequested -> failed + authority.window_close_requested
unknown lifecycle exit -> failed + lifecycle fallback code
missing report at generated caller -> typed failed report + exit 1
normal passed report -> schema/status/steps/exit 0 unchanged
```

### 6.3 Schema

保持：

```text
production-editor-authority-scenario.v1
production-editor-authority-report.v1
```

允许的新增 diagnostic code：

```text
authority.window_close_requested
authority.event_loop_exited_without_terminal_report
authority.runner_missing_terminal_report
```

不得新增项目专用字段或用 `passed` 伪装 fallback。

## 7. 拟修改文件与职责

### 7.1 `rust/crates/editor_window_winit/src/editor_frame_publication.rs`

仅 test fixture：

```text
aui_feedback_override_count = 0
aui_feedback_profile_ids = []
```

不得改变 production `EditorFramePublicationModule`。

### 7.2 `rust/crates/editor_window_winit/src/production_authority.rs`

- terminal intent/type；
- pure report construction 或可替代的 internal seam；
- exactly-once/fail-closed helper；
- close/partial/pass/fallback owner tests；
- generated caller 可复用的最小 public helper（仅在确有跨 crate caller 时 public）。

### 7.3 `rust/crates/editor_window_winit/src/real_window.rs`

- CloseRequested 提交明确 failed terminal；
- `ApplicationHandler::exiting` 做 no-overwrite fallback；
- `run_real_native_editor_authority_app` 返回前执行 producer postcondition；
- 保持普通非-scenario authority capture 行为。

### 7.4 `rust/crates/editor_core/src/project_editor_composition_artifact.rs`

- generated main 移除裸 report `expect`；
- 使用 owner typed fail-closed helper；
- 扩展 deterministic/project-agnostic generated-source test。

### 7.5 文档

完成施工时才更新：

```text
278 正式方案状态
240-5.6审查剩余问题讨论与施工优先级.md
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
00-文档地图.md
00-阅读顺序.md
施工文档/README.md
施工文档/待执行/README.md
阶段完成记录/README.md
阶段完成记录/<date>-Production-Authority-CloseRequested-Report-Finalization-Mini-v1/00-总览.md
```

## 8. 施工窗口与三小时边界

建议两窗口：

```text
Window A / Gate A-B
  test closure、red tests、terminal owner、winit lifecycle

Window B / Gate C-D
  generated caller、affected integration、文档闭环与归档
```

任一连续授权不得超过三小时：

```text
2h45m：不启动新实现或验证
3h：停止，保存证据，报告 stopped_by_time_limit
```

Window A 完成不自动授权 Window B；278 完成不自动授权 Tower Gate F。

## 9. Gate A：恢复测试面并建立 red-capable terminal contract

### 9.1 A0 test-only fixture closure

先只补 `editor_frame_publication.rs` test fixture 两个字段。执行：

```powershell
cargo test -p editor_window_winit --features real-window `
  editor_frame_publication_ -- --nocapture
```

要求至少选择现有两个 publication tests；`0 tests` 或编译失败均为 FAIL。

### 9.2 A1 red tests

在 production authority owner seam 写入以下测试，先观察针对当前实现的失败：

```text
production_authority_terminal_close_before_first_step_is_failed
production_authority_terminal_close_preserves_partial_steps
production_authority_terminal_exiting_does_not_overwrite_pass
production_authority_terminal_exiting_does_not_duplicate_known_failure
production_authority_terminal_missing_report_is_fail_closed
```

执行：

```powershell
cargo test -p editor_window_winit --features real-window `
  production_authority_terminal_ -- --nocapture
```

Red evidence 必须记录首个断言，不能只记录退出码。测试必须通过 terminal interface，不启动真实窗口。

### 9.3 Gate A PASS

```text
test fixture compile closure restored
red-capable tests selected > 0
current source fails exact missing-report/no-overwrite contract
no production implementation applied before red evidence
```

### 9.4 Gate A 实际证据（2026-08-09）

```text
A0 command：cargo test -p editor_window_winit --features real-window editor_frame_publication_ -- --nocapture
A0 result：PASS；2 passed / 0 failed / 199 filtered
A0 closure：只补 aui_feedback_override_count=0、aui_feedback_profile_ids=[]

A1 command：cargo test -p editor_window_winit --features real-window production_authority_terminal_ -- --nocapture
A1 result：RED；5 tests selected，3 failed / 2 passed
首个断言：close before first step must produce a report
其它精确红灯：close after a partial run must produce a report；active scenario runner return must fail closed
no-overwrite 基线：passed report 与 known failed report 两项在 red 阶段已保持不变
```

Gate A 结论：通过。红灯发生在 owner terminal contract，未启动真实窗口，未运行 Local CI。

## 10. Gate B：Terminal Finalizer 与 winit lifecycle

### 10.1 实现

1. 在 owner 中实现 `ProductionAuthorityTerminal` 与 exactly-once finalization。
2. report construction 保留 scenario id、elapsed、partial steps 和 diagnostics。
3. `WindowEvent::CloseRequested` 在 scenario active 时生成
   `authority.window_close_requested` failed report，再退出。
4. `ApplicationHandler::exiting` 仅在 active scenario 且 report missing 时生成
   `authority.event_loop_exited_without_terminal_report`。
5. `run_real_native_editor_authority_app` 返回前执行 producer postcondition；已有 report 不变。
6. 非-scenario capture 不创建 authority report。

### 10.2 Green tests

```powershell
cargo test -p editor_window_winit --features real-window `
  production_authority_terminal_ -- --nocapture

cargo test -p editor_window_winit --features real-window `
  production_authority_scenario_ -- --nocapture
```

### 10.3 Gate B PASS

```text
close/partial/pass/known-fail/fallback matrix green
normal pass cannot be overwritten
unknown exit cannot return None for active scenario
no OS window launched
scenario/report schema remains v1
```

### 10.4 Gate B 实际证据（2026-08-09）

```text
terminal green：5 passed / 0 failed / 201 filtered
scenario green：5 passed / 0 failed / 203 filtered
CloseRequested Adapter：failed + authority.window_close_requested
exiting fallback：authority.event_loop_exited_without_terminal_report；已有 PASS/FAIL no-op
runner postcondition：authority.runner_missing_terminal_report；仅 active scenario 生效
non-scenario capture：production_authority_report 保持 None
schema：scenario/report 均保持 v1
格式：三个 Window A Rust 文件 rustfmt --check 通过
```

Window A 源码身份：

```text
editor_frame_publication.rs
  before ECBBCCA96E05121E8F6E47DD14165A493DEC62FF3D90D080A0EA5DC890BA2C59
  after  F206ACC413E70464932E2135913B9075641593306FC9DD35E6055002ECDC8E32
production_authority.rs
  before BD6DCDD6AC06D1A6DD352773F131124A3F61E9861A6D80699BD0DF0D3572CCC8
  after  A4101A5A09EF315F4EF7E066F7F541DC995F8E13F57245C1932F5B3545AB7041
real_window.rs
  before 343A050B43E672C7EAAF1A47FF5419C308EBEAB6F2CECFD585FD09890C4B0236
  after  2754E5D9A7DA199EF74FFBFAA2BB13BD16232FB2DAC50818EFB5867287CEA341
project_editor_composition_artifact.rs
  unchanged F511E8CC1D94D7BCFA797A65072E5605C8716E0C338EBE92B60D2BEC9B4F26C0
```

Gate B 结论：通过。未进入 generated composition Gate C，未启动真实/production Editor，未运行 Local CI。

## 11. Gate C：Generated composition typed fail-closed consumer

### 11.1 Red-capable consumer assertion

先扩展 generated-source test，要求：

```text
forbid expect("production authority runner must produce a report")
require production authority owner fail-closed helper
require typed failed report serialization path
require failed result exit 1
```

当前 generated source 应先使新断言失败。

### 11.2 实现

- generated main 保留 scenario load/validate。
- 从 outcome 取得 report 时使用 owner helper；report missing 生成
  `authority.runner_missing_terminal_report`。
- 输出合法 report v1 JSON。
- `status=passed` 才 exit 0，其余 exit 1。
- 不在 generated source 手工复制 report 字段拼装。

### 11.3 验证

```powershell
cargo test -p editor_core `
  project_editor_composition_generated_source_ -- --nocapture
```

### 11.4 Gate C PASS

```text
generated source deterministic and project-agnostic
no naked report expect
missing-report result is typed failed report
no Tower/project-specific semantics
```

### 11.5 Gate C 实际证据（2026-08-09）

```text
red command：cargo test -p editor_core project_editor_composition_generated_source_ -- --nocapture
red result：1 test selected，0 passed / 1 failed / 651 filtered
首个断言：generated source must not panic when the production authority report is missing

实现：
- production_authority_report_or_fail_closed 由 owner 构造/持久化 typed failed report
- generated source 不再使用 expect("production authority runner must produce a report")
- report missing -> authority.runner_missing_terminal_report
- report.status == passed 才 exit 0，其余 exit 1

green result：1 passed / 0 failed / 651 filtered
```

Gate C 结论：通过。generated source 保持 deterministic/project-agnostic，无 Tower 语义。

## 12. Gate D：Affected closure、文档闭环与归档

### 12.1 代码冻结后的 affected closure

先确认没有 owned 进程或外部状态。本 Gate 不启动真实 Editor。

```powershell
cargo test -p editor_window_winit --features real-window
cargo test -p editor_core project_editor_composition_ -- --nocapture
cargo check -p editor_window_winit --features real-window
cargo fmt --all --check
```

若完整 `editor_window_winit` suite 已包含 Gate A-B filtered tests，最终报告记录覆盖关系，不再机械追加同输入定向 rerun。

### 12.2 Scoped hygiene

```powershell
git diff --check -- `
  rust/crates/editor_window_winit/src/editor_frame_publication.rs `
  rust/crates/editor_window_winit/src/production_authority.rs `
  rust/crates/editor_window_winit/src/real_window.rs `
  rust/crates/editor_core/src/project_editor_composition_artifact.rs `
  "框架设计/引擎总体架构/278-Production-Authority-CloseRequested-Report-Finalization-Mini-v1方案.md" `
  "框架设计/引擎总体架构/施工文档/当前/278-当前可自动化施工文档-Production-Authority-CloseRequested-Report-Finalization-Mini-v1.md"
```

对 untracked 文件另做 trailing-whitespace 检查；不得暂存、提交或创建分支，除非用户另行授权。

### 12.3 文档闭环

全部 Gate 与 affected closure 通过后：

1. 生成阶段完成记录。
2. 更新 278 正式方案为已完成。
3. 更新 240、49、54、文档地图、阅读顺序和施工/完成索引。
4. 把本文从 `施工文档/当前/` 移到 `施工文档/已完成/`。
5. 明确 Tower UI-V2 Gate F 仍需新的项目授权和 fresh root。

### 12.4 Gate D PASS

```text
owner and affected consumer closure green
fmt and scoped diff check green
completion record created
construction document archived
current engine construction slot empty
no external state touched
```

### 12.5 Gate D 实际证据（2026-08-09）

```text
cargo test -p editor_window_winit --features real-window
  PASS：201 passed / 0 failed / 8 ignored（local-only）

cargo test -p editor_core project_editor_composition_ -- --nocapture
  PASS：13 passed / 0 failed / 639 filtered

cargo check -p editor_window_winit --features real-window
  PASS

cargo fmt --all --check
  PASS

scoped git diff check
  PASS

untracked Rust trailing-whitespace check
  PASS（0 matches）
```

Gate D 最终源码身份：

```text
editor_frame_publication.rs F206ACC413E70464932E2135913B9075641593306FC9DD35E6055002ECDC8E32
production_authority.rs 0A3C6349C837C80EEF1B49078C422B69A9694FB62FF3CB7860A8C527DB2D59CF
real_window.rs 2754E5D9A7DA199EF74FFBFAA2BB13BD16232FB2DAC50818EFB5867287CEA341
project_editor_composition_artifact.rs 2723C71FE4455C714300DC1CC1A37BC8C8591A2A69426A0B272CC4397068EE4A
source HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5（dirty-preserved development evidence）
```

Gate D 结论：通过。未运行 Local CI、真实/production Editor、Tower Gate F/G、production replacement 或真实配置修改。

## 13. 失败、诊断与重跑规则

1. Gate A fixture 编译仍失败：先保留首个 E0063/新首因，不进入 terminal implementation。
2. Red test 未选中：视为无效证据，修正 filter，不实现代码。
3. Red test 因无关 baseline 失败：分类并报告，不把完整 suite 当诊断循环。
4. Gate B 失败：只运行 terminal owner reproduction，不运行 Gate C/D 或真实 Editor。
5. Gate C 失败：只修 generated-source owner/consumer，不改变 report schema。
6. 完整 suite 首次暴露新受影响 consumer：修复最小 owner/consumer 后只更新被该修复失效的证据。
7. 禁止通过删除断言、恢复裸 expect、吞 diagnostic 或伪造 passed report 制造 PASS。
8. Local CI、真实 Editor、Tower Gate F 均不作为重试手段。

## 14. External State

本施工预计只编译和运行 headless Rust tests，不创建真实窗口、不写真实配置、不替换二进制。

如果测试产生 repo-local Cargo target，只按现有 Cargo 行为保留；不得清理共享 target。若未来用户单独授权真实
Tower Gate F，必须使用新的 repository-external run root，并由 tower-defense-project 规则单独治理。

## 15. Validation Plan v2 映射

```text
Claim：active scenario terminal report exactly once
Owner：production_authority terminal tests

Claim：winit lifecycle close/exit 不能绕过 report
Owner：real_window Adapter + terminal interface tests

Claim：generated specialized Editor 不 panic
Consumer：editor_core generated-source test

Claim：277 frame fields 不再阻塞 owner suite
Owner：editor_frame_publication test fixture + existing publication tests

Claim：real Tower 720 scenario 真实恢复
不属于 278 source claim；后续 Tower UI-V2 Gate F fresh qualification
```

不复用旧 Gate F BLOCKED 作为 PASS，只复用其失败 trace 作为 red evidence。

## 16. 激活前复核清单

```text
[x] 用户明确授权激活 278 Window A / Gate A-B
[x] 当前/ 无其它 active construction document
[x] 正式方案 SHA256 已核对
[x] source HEAD 与 dirty changes 已记录
[x] 四个源码文件现状已重新读取
[x] 277 test fixture E0063 仍适用；限定为 Gate A0 test-only closure
[x] red test names/commands 可在 Gate A1 新增后选中非零测试
[x] 无 Local CI/真实 Editor/Tower Gate F 授权混入
[x] 授权窗口可在三小时内完成
[x] 本文根据复核结果重新自审
```

复核通过后才可把本文移动到 `施工文档/当前/`；移动本身不自动授权 Window A/B，除非用户指令同时明确授权。

## 17. 文档自审

### 17.1 是否忠实实现正式方案 B-Mini

是。Gate B 同时覆盖 known terminal、`exiting` fallback 和 runner postcondition；Gate C 消除 generated caller 裸 panic。

### 17.2 是否退化为方案 A

否。施工不只修改 CloseRequested，还锁定其它未分类 event-loop exit 的 terminal postcondition。

### 17.3 是否扩大为方案 C

否。不迁移完整 scenario step engine，不新增 Runner DSL、schema v2 或通用文件事务。

### 17.4 是否先有 red-capable test

是。Gate A 和 Gate C 均要求先观察能捕获具体缺陷的红灯，再实现；真实 Gate F 不用于调试。

### 17.5 是否处理现有测试阻塞

是。E0063 被限定为 test-only fixture mechanical closure，置于 Gate A0，不修改 production frame。

### 17.6 是否覆盖实际 consumer

是。owner 为 `editor_window_winit`，generated composition consumer 为 `editor_core`；最终执行两个 affected crate/profile。

### 17.7 是否避免重复验证

是。Gate A-C 是开发期 red/green；Gate D 完整 suite 在代码冻结后作为 affected closure，并记录其覆盖关系。

### 17.8 是否保护外部状态和 dirty worktree

是。不启动窗口、不改配置、不替换二进制、不清理共享 target，不执行 Git reset/revert/checkout。

### 17.9 是否明确激活与授权语义

是。Window A / Gate A-B 与 Window B / Gate C-D 均由用户分别授权并完成；归档后当前施工槽为空。

### 17.10 是否符合三小时限制

是。拆为两个独立窗口；任何连续授权仍受 2h45m/3h 停止规则。

### 17.11 是否包含完成闭环

是。Gate D 包含阶段记录、入口同步、施工归档与 Tower 后续授权边界。

### 17.12 自审结论

```text
正式方案一致性：通过
施工范围：通过
Gate 可执行性：通过
测试与失败规则：通过
外部状态边界：通过
激活与窗口授权语义：通过
结论：278 Gate A-D、affected closure、完成记录与归档条件均已满足
```

## 18. 当前停止点

```text
278 正式方案：已确认并自审
278 施工文档：已生成、自审并激活
施工文档位置：施工文档/当前/
Window A / Gate A-B：已完成
Window B / Gate C-D：已完成
Affected closure：已通过
完成记录：阶段完成记录/2026-08-09-Production-Authority-CloseRequested-Report-Finalization-Mini-v1/00-总览.md
施工归档：施工文档/已完成/278-当前可自动化施工文档-Production-Authority-CloseRequested-Report-Finalization-Mini-v1.md
引擎当前施工：无
当前继续施工授权：无
下一步：Tower UI-V2 Gate F 仅可在新的项目授权和 fresh root 下执行
```
