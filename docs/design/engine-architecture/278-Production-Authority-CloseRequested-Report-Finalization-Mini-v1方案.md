# 278 Production Authority CloseRequested Report Finalization Mini v1 方案

> 状态：方案 B-Mini 已完成施工；Gate A-D、affected closure、完成记录与归档闭环
> 日期：2026-08-09
> 上游：263 Production Editor Authority、273 GameView Presentation Target、277 AUI Control Feedback
> 下游：Tower Defense UI-V2 Gate F fresh 双竖屏资格
> 当前施工状态：已完成并归档到 `施工文档/已完成/`

## 1. 问题与结论

Tower UI-V2 Gate F 的 720×1280 production authority scenario 在已完成部分真实步骤和截图后关闭窗口，
`RealNativeEditorCaptureApp` 直接执行 `event_loop.exit()`，没有生成
`production-editor-authority-report.v1`。generated specialized Editor 随后对
`production_authority_report: Option<_>` 执行裸 `expect`，进程以 exit 101 panic，调用方只能知道“没有报告”，
不能知道最后步骤、关闭原因和已完成证据。

用户已选择方案 B-Mini：

```text
已知终态先 finalize_once
  -> winit ApplicationHandler::exiting 做 exactly-once lifecycle fallback
  -> run_app 返回前执行 producer postcondition
  -> generated composition 使用 typed fail-closed report helper，不再裸 panic
```

本方案不重构完整 ProductionAuthorityScenario DSL，不升级 report schema，不修改 Tower 项目，不把真实
Editor 当调试循环。目标是用一个小而深的 terminal finalization Module，使“成功加载并开始运行的 scenario
返回时必有且仅有一份 terminal report”成为 owner 不变量。

## 2. 简单解释

该能力相当于验收 Runner 的“黑匣子落盘保险”：

- 正常完成：报告 `passed`。
- 条件失败或超时：报告 `failed`，保留原始 diagnostic。
- 用户/系统关闭窗口：报告 `failed`，标记 `authority.window_close_requested`。
- 未分类的 event loop 退出：报告 `failed`，标记 lifecycle fallback diagnostic。
- 内部仍意外遗漏：generated composition 输出 typed failed report 并 exit 1，而不是 panic 101。

它不改变游戏逻辑、AUI 输入或画面，只保证 production authority 的验收事实可以被机器读取。

## 3. Context Scan：当前真实证据

### 3.1 Source identity 与工作树

```text
source HEAD：acaffa8cb95bbb54d093bbf45835a59315c82da5
工作树：dirty-preserved
引擎当前施工：无
引擎待执行施工：无（本文施工文档生成前）
Tower 当前施工：UI-V2 战报主导镜像战场主界面 v2；Gate F BLOCKED
```

已有和新增 dirty change 均属于用户工作树，不得 reset、revert、checkout 或覆盖。

### 3.2 Captured trace 红灯

权威失败证据：

```text
<TOWER_RUN_ROOT>/ui-v2-gate-f-20260808-163440/evidence/gate-f-final-report.json
```

关键事实：

```text
authority1080：production-editor-authority-report.v1，54/54 passed
authority720：structuredReportProduced=false
exitCode=101
stderr=production authority runner must produce a report
720 已产生 organizing / path-rejection / mixed-combat-round-6 三张非空截图
owned Editor/watchdog remaining=0
```

本轮 context scan 使用 captured-trace replay 加当前源码 guard，得到稳定红灯：

```text
RED authority.close_requested_missing_report:
captured 720 run exited 101 without production-authority-report.v1;
current source still has direct CloseRequested exit plus caller expect
```

### 3.3 当前代码链

```text
GeneratedEditor main
  -> ProductionAuthorityScenario::load
  -> run_real_project_editor_composition_authority
  -> RealNativeEditorCaptureApp::window_event
  -> WindowEvent::CloseRequested => event_loop.exit()
  -> run_real_native_editor_authority_app 返回 outcome
  -> outcome.production_authority_report.expect(...)
  -> panic / exit 101
```

当前 `finish_production_scenario` 已具备 `report.is_some()` 防重，但 finalization 分散在正常完成和已知
scenario failure 分支。CloseRequested、GPU/窗口初始化、部分输入/窗口消失等直接退出路径可能绕过它；
`run_real_native_editor_authority_app` 返回前也没有 terminal-report postcondition。

### 3.4 新发现的测试 closure 漂移

只读 context scan 尝试运行：

```powershell
cargo test -p editor_window_winit --features real-window production_authority_ -- --nocapture
```

测试尚未执行即在 test-only fixture 编译失败：

```text
E0063 missing fields:
aui_feedback_override_count
aui_feedback_profile_ids
rust/crates/editor_window_winit/src/editor_frame_publication.rs:152
```

这是 277 后的 affected test consumer fixture 漂移，不是 278 production 首因。278 施工必须先机械同步该
fixture 的两个默认字段，以恢复 owner 测试面；不得借此重新设计 277 或修改 production frame 合同。

## 4. 已排除假设

1. 不是 report JSON 写盘失败：现有 `finish_production_scenario` 即使写盘失败，仍会把内存 report 设为
   `Some`，不会触发 generated main 的 `expect`。
2. 不是 scenario 解析失败：generated main 在 Runner 前已 load + validate；720 已执行部分步骤和截图。
3. 不是 report 序列化失败：panic 发生在取得 report 之前，而不是 JSON serialization 之后。
4. primary 首因是 event loop terminal 没有统一映射为 scenario terminal；CloseRequested 是本次真实触发器。

## 5. 成熟工具参考

### 5.1 winit 0.30.13

本仓库锁定 `winit 0.30.13`。其 `ApplicationHandler::exiting` 是 event loop 即将退出时的 lifecycle
callback，`Event::LoopExiting` 会转发到该入口。可学习点：

- final cleanup/finalization 应收口在 event loop lifecycle，而不是依赖所有事件分支永远记得调用。
- callback 只负责终态收口，不继续执行业务步骤。

不可照搬点：winit 不知道 production authority schema、steps、diagnostics 或 evidence root，这些仍由本引擎 owner 管理。

### 5.2 Bevy bevy_winit Runner

Bevy 的 winit Runner 在调用 `event_loop.exit()` 前保存 `AppExit`，并在 `run_app` 返回后检查 terminal
state；缺失时返回明确错误。可学习点：

- 退出请求前记录终态。
- Runner 返回后再做一次 postcondition。

不可照搬点：Bevy 的 `AppExit` 是应用级退出码，不包含本项目需要的 scenario step reports、截图和
结构化 diagnostics；本引擎不能把它直接当 authority report。

## 6. Ownership 与深 Module

### 6.1 唯一 owner

`editor_window_winit::production_authority` 继续拥有：

- scenario/report v1 schema；
- terminal status 与 diagnostic code；
- report construction；
- exactly-once finalization；
- evidence-root persistence；
- generated composition 的 typed fail-closed fallback helper。

`RealNativeEditorCaptureApp` 只是 winit Adapter：把正常完成、已知失败、CloseRequested、`exiting` 和
run-app return 转换为 owner terminal intent。

`editor_core::project_editor_composition_artifact` 只生成调用代码，不自行拼装另一套 report 语义。

### 6.2 小接口

建议的概念接口：

```rust
enum ProductionAuthorityTerminal {
    Passed,
    Failed { diagnostic: String },
}

finalize_once(terminal) -> &ProductionAuthorityReport
ensure_terminal_report(fallback_diagnostic) -> &ProductionAuthorityReport
production_authority_report_or_fail_closed(scenario, report) -> ProductionAuthorityReport
```

实现可以保留为 crate-private helper + 一个 generated composition 可调用的 public helper；不得为测试暴露
完整 `RealNativeEditorCaptureApp` 或 `ActiveEventLoop`。

### 6.3 不变量

```text
I1  scenario 未启用时，不生成 production authority report。
I2  scenario 已成功加载并进入 Runner 后，返回时必须有 terminal report。
I3  terminal report 只生成一次；fallback 不覆盖先前 passed/failed。
I4  partial step reports 按原顺序保留。
I5  CloseRequested 产生 status=failed + authority.window_close_requested。
I6  未分类 lifecycle exit 产生明确 fallback diagnostic。
I7  generated composition 缺失 report 时输出 typed failed report、exit 1，不 panic。
I8  正常 PASS report 的 schema、status、steps 与退出码保持不变。
```

## 7. Terminal 顺序

```text
正常场景完成
  -> finalize_once(Passed)
  -> event_loop.exit()

已知失败/timeout/input error
  -> finalize_once(Failed(original_diagnostic))
  -> event_loop.exit()

CloseRequested
  -> finalize_once(Failed(authority.window_close_requested))
  -> event_loop.exit()

ApplicationHandler::exiting
  -> 若 report=None：finalize_once(Failed(authority.event_loop_exited_without_terminal_report))
  -> 若 report=Some：no-op

run_app 返回
  -> 若 active scenario + report=None：ensure_terminal_report(authority.runner_missing_terminal_report)
  -> 返回 outcome

generated composition
  -> production_authority_report_or_fail_closed(...)
  -> serialize typed report
  -> passed => exit 0；failed => exit 1
```

## 8. Schema 与 diagnostics

继续使用：

```text
production-editor-authority-scenario.v1
production-editor-authority-report.v1
```

不新增 report 字段，不提升 schemaVersion。使用既有 `status="failed"` 与 `diagnostics: Vec<String>` 表达：

```text
authority.window_close_requested
authority.event_loop_exited_without_terminal_report
authority.runner_missing_terminal_report
```

已有更具体 diagnostic 优先；fallback 只在没有 terminal report 时生效，不重复追加。

## 9. Persistence 与错误语义

Mini 保持现有 evidence root 和 `report.json` 路径。report construction 与 outcome assignment 不应依赖写盘成功；
若持久化失败，typed in-memory report 仍必须返回。是否把 persistence failure 加入 diagnostics，应只在不改变
正常 report contract 的前提下实现；不得为 278 引入通用文件事务框架。

## 10. 预计文件范围

```text
rust/crates/editor_window_winit/src/production_authority.rs
  terminal enum/helper、fail-closed report constructor 与 owner tests

rust/crates/editor_window_winit/src/real_window.rs
  CloseRequested、ApplicationHandler::exiting、run_app return postcondition

rust/crates/editor_core/src/project_editor_composition_artifact.rs
  generated main 移除裸 expect，使用 typed fail-closed helper；生成源码测试

rust/crates/editor_window_winit/src/editor_frame_publication.rs
  仅 test fixture 补齐 277 新增的两个 feedback 字段
```

只有 context scan 证明必需时，才允许调整同 crate 的 re-export；不得扩大到 Runtime、AUI、Tower 或其它 sample。

## 11. 验证合同

### 11.1 Red-capable owner tests

```text
close_before_first_step_produces_failed_terminal_report
close_after_partial_steps_preserves_completed_steps
exiting_after_pass_does_not_overwrite_pass_report
exiting_after_known_failure_does_not_duplicate_diagnostics
unexpected_event_loop_return_produces_typed_failed_report
```

这些测试通过纯 terminal interface 运行，不启动真实 OS window。

### 11.2 Generated consumer tests

必须证明 generated source：

- 不含 `expect("production authority runner must produce a report")`；
- 调用 owner 提供的 typed fail-closed helper；
- report missing 时仍可序列化 v1 failed report；
- failed exit code 为 1，不是 panic 101。

### 11.3 Affected closure

施工时按影响面执行：

```powershell
cargo test -p editor_window_winit --features real-window production_authority_ -- --nocapture
cargo test -p editor_window_winit --features real-window editor_frame_publication_ -- --nocapture
cargo test -p editor_core project_editor_composition_generated_source_ -- --nocapture
cargo fmt --all --check
```

若 filtered tests 没有覆盖全部修改过的 crate consumer，代码冻结后再运行对应 crate 的受影响 suite；不得机械
重复相同输入测试。

### 11.4 真实环境边界

278 不把真实 Editor、Tower Gate F、Local CI 或 production binary replacement 作为施工调试循环。
278 source closure 完成后，Tower UI-V2 才可在新的单独授权下使用 fresh root 重跑 Gate F；该结果属于
Tower consumer qualification，不倒灌为 278 的日常开发测试。

## 12. Compatibility 与迁移

- scenario/report v1 JSON 保持兼容。
- 正常 passed/failed terminal 不变。
- 旧调用者继续可读取 `RealNativeEditorCaptureOutcome`。
- generated composition 从 panic 改为 typed failed report，是 fail-closed 行为修复。
- 不恢复历史 candidate/freezer、旧三工具或已关闭 CQ/INC。
- 不修改普通 Native Editor CloseRequested 生命周期；只作用于 production authority capture adapter。

## 13. 风险与控制

### 风险 A：fallback 覆盖正常 PASS

控制：`finalize_once` 必须先检查已有 report；PASS 后 `exiting` 为 no-op。

### 风险 B：所有退出都被误报为 CloseRequested

控制：已知关闭使用专用 code；其它 lifecycle fallback 使用独立 code。

### 风险 C：generated caller 与 owner 各造一份不同 report

控制：fail-closed constructor 由 `production_authority` owner 提供，generated main 只调用。

### 风险 D：为测试暴露 winit internals

控制：测试 terminal interface；不公开 `RealNativeEditorCaptureApp`、Window 或 ActiveEventLoop。

### 风险 E：把 277 fixture 漂移扩大成新系统

控制：只补两个 test-only 默认字段并记录首个编译失败，不改 277 production 语义。

## 14. 建议施工窗口

```text
Window A / Gate A-B
  Gate A：test fixture closure + terminal owner red tests
  Gate B：finalize_once、CloseRequested 与 exiting lifecycle

Window B / Gate C-D
  Gate C：runner postcondition + generated composition typed fallback
  Gate D：affected closure、文档同步与归档
```

两窗口都不授权真实 Editor、Tower Gate F、Local CI、production replacement 或真实配置。

## 15. Red Lines

- 禁止只修一条 CloseRequested 而保留同类无报告退出。
- 禁止用 `unwrap_or_default` 伪造 passed report。
- 禁止吞掉原始 diagnostic 或丢失 partial step reports。
- 禁止 schemaVersion 漂移而不声明 migration。
- 禁止在 generated main 复制 report construction 规则。
- 禁止把 Tower action/node/path 写入引擎。
- 禁止用真实 Gate F 作为修复循环。
- 禁止修改 production/安装态二进制、真实配置或运行 Local CI。

## 16. 方案自审

### 16.1 是否忠实于用户选择的方案 B

是。方案同时包含已知退出前 finalization、winit `exiting` lifecycle fallback、run-app postcondition 和
caller typed fallback；不是方案 A 的单分支补丁，也没有扩大为方案 C 的完整 Runner 重构。

### 16.2 是否为深 Module

是。caller 只提交 terminal intent 或请求 fail-closed report；exactly-once、partial steps、diagnostics、
persistence 与 schema construction 隐藏在 owner 后面。

### 16.3 是否保持 owner 边界

是。schema/report 属于 `production_authority`；winit app 是 Adapter；generated composition 不拥有报告规则。

### 16.4 是否可红灯验证

是。captured trace 已稳定红；施工测试通过纯 terminal interface 捕获 close/partial/pass/fallback，不依赖真实窗口。

### 16.5 是否过度设计

否。没有抽取完整 `ProductionAuthorityRun`、没有新 DSL、没有 schema v2、没有通用文件事务或外部 Adapter。

### 16.6 是否保护 dirty worktree 与外部状态

是。预计只修改四个已识别文件，保留所有既有改动；施工与验证不触碰真实 Editor、Tower、Local CI、
production binary 或真实配置。

### 16.7 是否处理 context scan 新发现

是。277 test-only fixture 漂移被限定为施工 Gate A 前置，不伪装为 278 production 首因，也不重新设计 277。

### 16.8 自审结论

```text
方案选择：B-Mini
正式方案：通过
范围：最小且完整
schema：v1 保持
owner/test seam：明确
真实环境授权：无
下一步：278 source closure 已完成；Tower UI-V2 Gate F 仍需新的项目授权和 fresh root，不自动执行
```
