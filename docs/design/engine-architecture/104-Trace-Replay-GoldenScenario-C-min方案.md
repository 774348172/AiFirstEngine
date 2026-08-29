# 104-Trace / Replay / Golden Scenario C-min 方案

## 1. 定位

本系统是 Rust Native Runtime 的 AI-first 验证底座。

它不把 Golden Scenario 定位成万能验收系统，而是定位为：

```text
AI-first Scenario Regression Gate
```

也就是：

```text
用固定 Runtime Package / Scene
用固定 Time / Input
跑真实 FrameLoop
收集 Trace / FrameHash / CheckResult
生成 AI 可读的回归报告
```

它解决的是 AI 修改后有没有破坏关键运行链路，而不是替代全部测试。

## 2. 和复杂需求的关系

复杂需求不能只靠 Golden Scenario 验收。正式验证链路必须分层：

```text
Static Validation
Logic Test
Contract Test
Golden Scenario
Stress / Soak Scenario
Trace Diff
```

Golden Scenario 只负责端到端关键流程回归：

```text
输入是否进入 Runtime
规则是否执行
ECS 是否发生预期读写
Physics / Prefab / AUI / Render Extract 是否被真实链路触发
最终状态或关键 Trace 是否符合预期
```

复杂需求的局部规则、公式、边界条件必须由 Logic Test / Contract Test 覆盖。

## 3. 其他引擎对比

| 引擎 | 对应能力 | 特点 | 我们借鉴什么 |
|---|---|---|---|
| Unity | PlayMode Test / EditMode Test / Profiler / Recorder | 可以跑真实场景，但 AI 证据弱 | 学习 PlayMode Test 的真实运行验证，不照搬人工日志模式 |
| UE | Automation Test / Functional Test / Insights Trace / Network Replay | 验证和 Trace 很强，但体系重 | 学习 Functional Test + Trace 证据，不做第一版重型录制器 |
| Bevy | headless App / Schedule test / Diagnostics | ECS 测试很轻，适合自动化 | 学习 headless runner 和确定性小场景验证 |
| Godot | doctest / SceneTree test / Profiler | 简洁，偏场景和单元验证 | 学习低复杂度测试入口 |

结论：

```text
成熟引擎都不是靠一个场景测试解决复杂项目。
我们的优势应该是把测试、Trace、Diff、AI 修复证据打通。
```

## 4. 第一版边界

第一版做：

```text
GoldenScenario 数据结构
GoldenScenarioRunner
GoldenScenarioReport
GoldenCheck
GoldenCheckResult
GoldenFrameRecord
首个失败帧定位
AI-readable failure summary
```

第一版不做：

```text
完整录像系统
全量 World Snapshot 历史
强制所有项目帧确定性
网络帧同步 Replay
用户游玩录像
跨平台 bit-level replay
项目玩法语义 API
```

禁止引擎层出现：

```text
enemy
bullet
damage
health
score
wave
skill
weapon
inventory
quest
boss
```

这些只能出现在项目 Schema / 项目规则 / 项目测试数据里。

## 5. 标准结构

```text
GoldenScenario
  schema_version
  scenario_id
  name
  fixed_delta_time
  frame_count
  input_frames
  checks
```

```text
GoldenInputFrame
  frame_index
  action_snapshot
```

```text
GoldenCheck
  check_id
  expected_frame
  kind
```

第一版 check kind：

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

字段保持通用：

```text
entity_id
component_type
field_path
expected_value
system_id
phase
operation
```

## 6. 运行流程

```text
GoldenScenarioRunner
  -> load / receive initial World
  -> build FrameLoop
  -> apply fixed_delta_time
  -> for frame 1..N:
       take ActionSnapshot for frame
       tick_runtime_frame_with_input_and_delta
       collect frame_hash / trace summary / render count / physics count
       run checks whose expected_frame == current frame or AnyFrame
  -> build GoldenScenarioReport
```

## 7. Report 规则

Report 必须给 AI 和用户看得懂的结构化结果：

```text
scenario_id
status
frames_run
first_failed_frame
check_results
frame_records
failure_summary
```

失败摘要必须包含：

```text
失败 check_id
失败帧
预期
实际
关联 trace 数量
建议排查域
```

第一版建议排查域只使用通用域：

```text
input
logic
ecs
physics2d
spawn_despawn
render_extract
aui
unknown
```

## 8. 和旧 Runtime Replay MVP 的关系

`26-Runtime-Replay-MVP.md` 是旧 TypeScript 原型期文档。

正式规则：

```text
旧 TypeScript Runtime Replay 只作为历史参考。
Rust Native Runtime 正确性由 Golden Scenario / Test Graph 验证。
不得继续扩张旧 TypeScript Replay 成第二套正式引擎验证标准。
```

## 9. 核心结论

```text
Golden Scenario 有必要存在，但不能承担全部复杂需求验收。
它是端到端关键流程回归门禁。
复杂需求必须组合 Static Validation / Logic Test / Contract Test / Golden Scenario / Stress Scenario / Trace Diff。
第一版只做轻量、通用、AI 可读、headless 可测试的 Rust Native 验证底座。
```
