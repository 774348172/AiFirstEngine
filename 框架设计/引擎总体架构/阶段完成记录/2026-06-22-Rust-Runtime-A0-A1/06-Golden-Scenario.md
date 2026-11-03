# Golden Scenario 前 4 场景

## 功能目标

用 Golden Scenario 验证 Rust Runtime 主链路，而不是对齐 TypeScript Runtime：

```text
Runtime Package / Scene
  -> ECS World
  -> FrameLoop
  -> RenderSnapshot / Trace / Hash
```

## 实现文件

```text
rust/crates/engine_runtime/src/golden.rs
scripts/test-rust-runtime-golden.cjs
```

相关 npm script：

```text
test:rustruntimegolden
```

## 第一版场景

```text
empty_scene_load
single_entity_transform
renderable_snapshot
fixed_tick_10_frames
```

## 最小测试用例

命令：

```powershell
npm.cmd run test:rustruntimegolden
```

覆盖测试：

```text
golden_empty_scene_load
golden_single_entity_transform
golden_renderable_snapshot
golden_fixed_tick_10_frames
```

本轮结果：

```text
空场景可加载。
单 Entity transform 可读取。
Renderable snapshot 可生成。
固定 tick 10 帧可执行并输出 hash。
测试通过。
```

## 边界

```text
不跑 command_queue_create_destroy。
不跑 projectile_lifetime_cleanup。
不跑 simple_movement_rule。
不跑 runtime_trace_output 扩展场景。
这些会提前引入 command queue / gameplay rule / IR trace 边界。
```
