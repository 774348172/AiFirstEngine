# 97-Runtime Time Service C-min 方案

本文档定义 Unity-like 的 Runtime Time Service 第一版规则。

它承接：

```text
17-Runtime-FrameLoop.md
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
73-当前可自动化施工文档-ProjectLogicRunner-LogicExecutor-RustECS接入-v1.md
93-复杂打飞机验证所需引擎侧缺失能力清单.md
94-Engine-Gameplay-Foundation-C-min边界方案.md
```

## 1. 本文解决什么

当前 Runtime 已经有：

```text
frame_index
ProjectLogicRunner.delta_time
LogicContext.delta_time
FixedUpdate / FrameUpdate phase
```

但还没有正式的时间服务边界：

```text
delta_time 和 fixed_delta_time 的来源是什么。
time_scale 如何影响项目逻辑。
unscaled time 是否可读。
Timer / Cooldown 到底是引擎系统，还是 helper。
AI 生成项目逻辑时应该使用什么时间入口。
```

## 2. 成熟引擎参考

### Unity

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\Time\Time.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\PlayerLoop\PlayerLoop.bindings.cs
```

Unity 的核心心智：

```text
Time.deltaTime
Time.fixedDeltaTime
Time.time
Time.unscaledDeltaTime
Time.realtimeSinceStartup
Time.timeScale
```

用户脚本默认直接读 `Time`：

```text
position += velocity * Time.deltaTime
```

Unity 不要求用户先理解一个全局 Timer 注册表。

### Unreal Engine

UE 使用：

```text
World Tick DeltaSeconds
FTimerManager
SetTimer / ClearTimer
```

UE 的 TimerManager 很强，但它是一套更重的调度系统。第一版如果照搬，会让本项目过早引入“到点执行回调”的隐藏规则。

### Godot

Godot 使用：

```text
_process(delta)
_physics_process(delta)
Timer Node
```

Godot 的时间心智也很直接：每帧传入 `delta`，Timer 是可选节点，不是所有逻辑必须经过的系统。

### Bevy

Bevy 使用：

```text
Time resource
Timer
Stopwatch
```

Bevy 对 Rust ECS 很自然，但它的 Schedule / Resource 心智不应完整暴露给本项目普通用户和 AI 默认层。

## 3. 正式方案：Unity-like Time Service

正式名称：

```text
Runtime Time Service C-min
```

第一版只做：

```text
RuntimeTime
TimeContext
Timer helper
Cooldown helper
TimeTraceSummary
```

不做：

```text
全局 Timer Manager
Coroutine
Schedule
Delayed Command
InvokeRepeating
Timeline
Animation Event
```

### 3.1 RuntimeTime

第一版结构：

```text
RuntimeTime
  time
  delta_time
  unscaled_time
  unscaled_delta_time
  fixed_time
  fixed_delta_time
  frame_count
  fixed_frame_count
  time_scale
  maximum_delta_time
  in_fixed_step
```

字段含义：

```text
time:
  受 time_scale 影响的运行时间。

delta_time:
  本帧缩放后的时间差。

unscaled_time:
  不受 time_scale 影响的运行时间。

unscaled_delta_time:
  不受 time_scale 影响的帧时间差。

fixed_time:
  固定步累计时间。

fixed_delta_time:
  固定步时间间隔。

frame_count:
  FrameUpdate 帧计数。

fixed_frame_count:
  FixedUpdate 帧计数。

time_scale:
  时间缩放。

maximum_delta_time:
  单帧最大 delta 限制，避免暂停恢复或卡顿后一次性推进过大。

in_fixed_step:
  当前是否处于固定步逻辑。
```

### 3.2 TimeContext

项目规则默认读取 TimeContext，而不是直接修改 RuntimeTime。

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

正式规则：

```text
FrameLoop 负责推进 RuntimeTime。
LogicContext 只暴露只读 TimeContext。
项目规则不能直接写 RuntimeTime。
AI 生成移动 / 冷却 / 生命周期逻辑时，默认使用 Time.delta_time。
FixedUpdate 规则默认使用 Time.fixed_delta_time。
```

### 3.3 Timer / Cooldown helper

Timer 和 Cooldown 只是工具，不是全局系统。

```text
Timer
  duration
  elapsed
  repeat
  use_unscaled_time
```

```text
Cooldown
  duration
  remaining
```

使用心智：

```text
timer.tick(Time.delta_time)
if timer.finished_this_frame:
  项目规则自己决定做什么

cooldown.tick(Time.delta_time)
if cooldown.ready():
  cooldown.trigger()
  项目规则自己决定做什么
```

正式边界：

```text
引擎不维护全局 Timer 注册表。
引擎不负责 Timer 到点后执行项目命令。
项目组件可以保存自己的 Timer / Cooldown 字段。
Timer helper 只负责时间计算。
```

## 4. AI 友好规则

AI 生成项目逻辑时，推荐：

```text
position += velocity * Time.delta_time
cooldown_remaining -= Time.delta_time
if cooldown_remaining <= 0:
  ...
```

AI 不应默认生成：

```text
注册全局 Timer
到点自动回调
跨系统 delayed command
隐藏 coroutine
```

原因：

```text
Unity-like Time 心智简单。
项目状态在哪里修改一眼能看到。
复杂项目后期更容易 Trace 和回放。
AI 不需要理解隐藏调度器。
```

## 5. Trace / Report

第一版 TimeTraceSummary 只记录摘要：

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

规则：

```text
TimeTraceSummary 用于 AI 和用户排查“为什么这一帧移动这么多”。
Release 默认不需要记录完整时间历史。
Debug / Evidence 可以记录每帧 TimeTraceSummary。
```

## 6. 与原 B-min 的区别

原 B-min 容易变成：

```text
Runtime 维护 TimerState 列表
Timer 到点产生 signal
项目再消费 signal
```

正式方案改为：

```text
Runtime 提供 Unity-like Time。
Timer / Cooldown 只是 helper。
项目规则显式读取 Time 并修改自己的状态。
```

这更接近 Unity / Godot，也更符合本项目“少隐藏规则”的原则。

## 7. 第一版测试要求

必须覆盖：

```text
RuntimeTime 默认值稳定。
advance_frame 使用 unscaled_delta_time * time_scale 得到 delta_time。
maximum_delta_time 会 clamp 过大的 unscaled_delta_time。
time_scale = 0 时 delta_time = 0，但 unscaled_delta_time 保持真实输入。
fixed_step 使用 fixed_delta_time 并设置 in_fixed_step。
LogicContext 能读取 TimeContext。
Timer helper tick 后能报告 finished_this_frame。
Cooldown helper tick / trigger 行为稳定。
TimeTraceSummary 可序列化 / 可读。
```

## 8. 下一步

可以生成施工文档：

```text
97-当前可自动化施工文档-Runtime-Time-Service-C-min.md
```

施工范围只做 RuntimeTime / TimeContext / Timer helper / Cooldown helper / TimeTraceSummary 的 headless 闭环，不做全局 Timer Manager。
