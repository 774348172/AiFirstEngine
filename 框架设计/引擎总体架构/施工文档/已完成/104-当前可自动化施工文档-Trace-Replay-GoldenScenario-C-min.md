# 104-当前可自动化施工文档-Trace / Replay / Golden Scenario C-min

## 1. 目标

按照 `104-Trace-Replay-GoldenScenario-C-min方案.md`，把现有 `engine_runtime::golden` 从 hard-coded smoke enum 扩展成正式的 Rust Native Golden Scenario C-min 验证底座。

目标闭环：

```text
GoldenScenario
  -> GoldenScenarioRunner
  -> FrameLoop tick_runtime_frame_with_input_and_delta
  -> GoldenScenarioReport
  -> 最小测试
```

## 2. 施工边界

本轮做：

```text
GoldenScenario 数据结构
GoldenInputFrame
GoldenCheck / GoldenCheckKind
GoldenCheckResult
GoldenFrameRecord
GoldenScenarioReport
GoldenScenarioRunner
首个失败帧定位
AI-readable failure_summary
```

本轮不做：

```text
完整 replay 文件格式
完整 World Snapshot 历史
跨平台确定性验证
Stress / Soak Runner
旧 TypeScript Replay 迁移
UI 面板接入
磁盘归档
```

## 3. 自动化任务

### A1：文档落地

新增：

```text
104-Trace-Replay-GoldenScenario-C-min方案.md
施工文档/当前/104-当前可自动化施工文档-Trace-Replay-GoldenScenario-C-min.md
```

### A2：Golden 数据结构

修改：

```text
rust/crates/engine_runtime/src/golden.rs
```

新增通用结构：

```text
GoldenScenario
GoldenInputFrame
GoldenCheck
GoldenCheckKind
GoldenExpectedFrame
GoldenCheckResult
GoldenFrameRecord
GoldenScenarioReport
GoldenScenarioStatus
```

### A3：Runner

新增：

```text
GoldenScenarioRunner
GoldenScenarioRunner::run
```

Runner 使用现有：

```text
FrameLoop
RenderSceneState
RenderExtractContext
ActionSnapshot
RuntimeTrace
```

### A4：最小 Check 支持

第一版至少支持：

```text
FrameHashEquals
EntityExists
EntityNotExists
ComponentExists
ComponentFieldEquals
TraceEventExists
GameplayTraceExists
Physics2DPairCountEquals
RenderProxyCountEquals
```

### A5：测试

新增或扩展 `golden.rs` 单元测试：

```text
golden_scenario_runner_passes_key_flow_checks
golden_scenario_runner_reports_first_failed_frame
golden_scenario_runner_supports_trace_checks
golden_scenario_runner_keeps_engine_terms_generic
```

## 4. 验证命令

```powershell
cargo fmt --check
cargo test -p engine_runtime golden
cargo test -p engine_runtime frame_loop
cargo test -p engine_runtime runtime_time
cargo test -p engine_runtime
cargo test --workspace --no-run
```

如果 Windows 策略阻止 test binary 执行，必须记录具体错误，但仍尽量完成 `cargo check` / `--no-run`。

## 5. 完成后归档

施工完成后：

```text
施工文档/当前/104-...md -> 施工文档/已完成/104-...md
阶段完成记录/2026-06-29-Trace-Replay-GoldenScenario-C-min/00-总览.md
更新 54-当前可自动化施工总入口.md
```
