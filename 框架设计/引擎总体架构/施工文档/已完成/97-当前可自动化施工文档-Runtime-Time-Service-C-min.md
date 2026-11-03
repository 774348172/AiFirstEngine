# 97-当前可自动化施工文档-Runtime Time Service C-min

## 1. 施工目标

按照 `97-Runtime-Time-Service-C-min方案.md` 实现第一版 Unity-like Runtime Time Service。

本轮目标是补齐 Runtime 层通用时间底座，让项目规则可以稳定读取时间并自行决定玩法行为：

```text
RuntimeTime
TimeContext
Timer helper
Cooldown helper
TimeTraceSummary
LogicContext time access
FrameLoop / EngineHostLoop time advance
```

## 2. 明确不做

第一版不引入隐藏调度系统，避免规则膨胀：

```text
不做全局 Timer Manager
不做 Coroutine
不做 Schedule
不做 Delayed Command
不做 InvokeRepeating
不做 Timeline
不做 Animation Event
不做项目玩法概念，例如 Bullet / Enemy / Wave / Skill
```

引擎只提供底座能力。发射频率、敌人波次、技能冷却、倒计时触发行为都由项目规则自己保存状态并决定执行。

## 3. 施工范围

优先修改或新增以下 Rust Runtime 文件：

```text
rust/crates/engine_runtime/src/time.rs
rust/crates/engine_runtime/src/lib.rs
rust/crates/engine_runtime/src/logic_executor.rs
rust/crates/engine_runtime/src/project_logic.rs
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
```

如果现有命名更贴近 `runtime_time.rs`，可以使用该文件名，但必须保证模块边界清晰。

## 4. 数据结构要求

### 4.1 RuntimeTime

```text
RuntimeTime
  time: f32
  delta_time: f32
  unscaled_time: f32
  unscaled_delta_time: f32
  fixed_time: f32
  fixed_delta_time: f32
  frame_count: u64
  fixed_frame_count: u64
  time_scale: f32
  maximum_delta_time: f32
  in_fixed_step: bool
```

规则：

```text
FrameLoop / EngineHostLoop 负责推进 RuntimeTime。
time 和 delta_time 受 time_scale 影响。
unscaled_time 和 unscaled_delta_time 不受 time_scale 影响。
maximum_delta_time 用于 clamp 过大的输入 delta。
time_scale = 0 时，delta_time = 0，但 unscaled_delta_time 保持真实输入。
```

### 4.2 TimeContext

```text
TimeContext
  time
  delta_time
  unscaled_time
  unscaled_delta_time
  fixed_time
  fixed_delta_time
  frame_count
  fixed_frame_count
  time_scale
  in_fixed_step
```

规则：

```text
LogicContext 只暴露只读 TimeContext。
项目逻辑不能直接修改 RuntimeTime。
FrameUpdate 默认使用 delta_time。
FixedUpdate 默认使用 fixed_delta_time。
```

### 4.3 Timer helper

```text
Timer
  duration
  elapsed
  repeat
  use_unscaled_time
```

最小行为：

```text
tick(delta)
reset()
finished()
finished_this_frame()
progress()
```

### 4.4 Cooldown helper

```text
Cooldown
  duration
  remaining
```

最小行为：

```text
tick(delta)
ready()
trigger()
reset()
remaining()
normalized()
```

### 4.5 TimeTraceSummary

```text
TimeTraceSummary
  frame_count
  fixed_frame_count
  delta_time
  unscaled_delta_time
  time_scale
  in_fixed_step
  clamped_by_maximum_delta_time
```

用于 Debug / Evidence / AI 排查。Release 默认不要求记录完整历史。

## 5. 施工步骤

### Step 1: 新增 Runtime Time 模块

实现：

```text
RuntimeTime
TimeContext
TimeTraceSummary
advance_frame(unscaled_delta_time)
advance_fixed_step()
set_time_scale()
set_maximum_delta_time()
```

最小测试：

```text
runtime_time_defaults_are_stable
runtime_time_advance_applies_time_scale
runtime_time_clamps_maximum_delta_time
runtime_time_scale_zero_freezes_scaled_delta
runtime_time_fixed_step_sets_in_fixed_step
time_trace_summary_is_serializable
```

### Step 2: 接入 LogicContext

实现：

```text
LogicContext exposes TimeContext
保留现有 delta_time 兼容入口时，内部应来自 TimeContext
```

最小测试：

```text
logic_context_exposes_time_context
logic_context_delta_time_matches_time_context
```

### Step 3: 接入 ProjectLogicRunner / LogicExecutor

实现：

```text
ProjectLogicRunner receives TimeContext
LogicExecutor 执行规则时读取同一份 TimeContext
FrameUpdate / FixedUpdate 能区分 in_fixed_step
```

最小测试：

```text
project_logic_runner_uses_runtime_time_context
logic_executor_receives_stable_time_context
fixed_update_uses_fixed_delta_time
```

### Step 4: 接入 FrameLoop / EngineHostLoop

实现：

```text
EngineHostLoop 作为最高层心跳源输入 unscaled delta
FrameLoop 推进 RuntimeTime
FrameLoop 把 TimeContext 传给项目逻辑
```

最小测试：

```text
frame_loop_advances_runtime_time_once_per_frame
engine_host_loop_passes_unscaled_delta_to_runtime_time
```

### Step 5: 实现 Timer / Cooldown helper

实现：

```text
Timer tick / repeat / finished_this_frame
Cooldown tick / ready / trigger
```

最小测试：

```text
timer_helper_reports_finished_this_frame
timer_helper_repeat_is_stable
cooldown_helper_tick_and_trigger_are_stable
cooldown_helper_zero_duration_is_ready
```

## 6. 验证命令

优先逐步执行：

```text
cargo fmt --check
cargo test -p engine_runtime runtime_time
cargo test -p engine_runtime logic_context
cargo test -p engine_runtime logic_executor
cargo test -p engine_runtime frame_loop
cargo test -p engine_runtime
cargo test --workspace
```

## 7. 完成标准

```text
RuntimeTime / TimeContext / Timer / Cooldown / TimeTraceSummary 已实现。
项目逻辑能从 LogicContext 读取统一 TimeContext。
FrameLoop / EngineHostLoop 能推进时间。
所有新增能力都有最小单元测试。
不新增全局 TimerManager / Coroutine / Scheduler。
不新增任何项目玩法 API。
cargo fmt --check 通过。
cargo test -p engine_runtime 通过。
cargo test --workspace 通过，或明确记录环境阻塞原因。
```

## 8. 施工完成后需要更新

完成施工后，把本文档移动到：

```text
框架设计/引擎总体架构/施工文档/已完成/
```

并更新：

```text
54-当前可自动化施工总入口.md
施工文档/README.md
阶段完成记录/2026-06-29-Runtime-Time-Service-C-min/00-总览.md
```
