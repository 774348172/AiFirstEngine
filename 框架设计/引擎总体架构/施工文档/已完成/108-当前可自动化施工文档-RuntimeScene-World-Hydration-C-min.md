# 108-当前可自动化施工文档-RuntimeScene World Hydration C-min

## 1. 施工目标

建立 `RuntimeScene -> World` 的正式 Hydration 门面，收敛真实 runtime 入口，避免 `scene_loader` 和 `RuntimeInstanceLoader` 长期并行成为两套真相。

## 2. 施工原则

```text
长期主义：真实 Runtime Package 运行必须走正式 Hydration。
简单优先：不新增项目规则，不新增玩法概念。
复用现有能力：底层继续使用 RuntimeInstanceLoader。
测试优先：每个最小能力必须有单元测试。
兼容历史：scene_loader 暂保留为旧测试工具，不继续扩展。
```

## 3. 代码施工项

### A1. 新增 runtime_scene_hydration 模块

文件：

```text
rust/crates/engine_runtime/src/runtime_scene_hydration.rs
```

新增结构：

```text
RuntimeSceneHydrationRequest
RuntimeSceneHydrationReport
RuntimeSceneHydrator
hydrate_active_scene_into_world
```

要求：

```text
Hydrator 内部持有 RuntimeInstanceLoader。
C-min 默认 mount startup bundle。
Hydration 后收集 initial_dirty_records。
错误通过 RuntimeDiagnostics 暴露给现有 run report。
```

### A2. 暴露模块

文件：

```text
rust/crates/engine_runtime/src/lib.rs
```

新增：

```rust
pub mod runtime_scene_hydration;
```

### A3. Runtime run 接入 Hydration

文件：

```text
rust/crates/engine_runtime/src/runtime_run.rs
rust/crates/engine_runtime/src/default_game_run.rs
```

要求：

```text
真实 package run 不再直接调用 scene_loader::load_scene_into_world。
改为 hydrate_active_scene_into_world。
```

### A4. 暂不删除 scene_loader

规则：

```text
scene_loader 保留为历史测试工具 / compatibility helper。
后续不再在真实 runtime 入口扩展它。
```

## 4. 测试项

### T1. Hydrator 能加载 active scene

验证：

```text
World entity_count 正确。
Transform 存在。
Renderable 存在。
Dynamic Component 存在。
RuntimeSceneInstance active。
```

### T2. Hydration 产生 initial dirty records

验证：

```text
Transform dirty 存在。
RenderState dirty 存在。
DynamicData dirty 存在。
Report initial_dirty_records 与 World 当前 dirty_records 对齐。
```

### T3. hydrate_active_scene_into_world 返回 World + Report

验证：

```text
RuntimeLoadResult ok。
World 创建成功。
Report loaded_asset_count 正确。
```

### T4. 真实 run 回归

命令：

```text
cargo fmt --check
cargo test -p engine_runtime runtime_scene_hydration
cargo test -p engine_runtime default_game_run_orchestrator_runs_headless_minimal_scenario
cargo test -p engine_runtime run_runtime_package_headless
cargo test -p engine_runtime
```

## 5. 完成标准

```text
正式 Hydration API 存在。
真实 runtime run 入口已接入 Hydration。
Hydration 报告包含 instance report 和 initial dirty records。
单元测试通过。
不引入项目特定概念。
```

