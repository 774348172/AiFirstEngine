# 99-当前可自动化施工文档-Runtime Prefab Spawn / Despawn C-min

## 1. 施工目标

按照 `99-Runtime-Prefab-Spawn-Despawn-C-min方案.md`，补齐项目规则运行时实例化 / 销毁 Prefab 的标准调用链：

```text
Project Rule
  -> LogicContext
  -> GameplayCommandBuffer
  -> ProjectLogicRunner apply point
  -> RuntimeInstanceLoader
  -> World / ECS
  -> RuntimeTrace
```

本阶段不新增玩法概念，不做 Bullet / Enemy / Score 之类项目 API。

## 2. 明确不做

```text
不做 Prefab override。
不做 Prefab variant。
不做运行时编辑 prefab asset。
不做异步分帧 instantiate。
不做 deferred construction callback。
不做项目专用 SpawnBullet / SpawnEnemy。
不让 LogicContext 暴露 RuntimeInstanceLoader。
不让项目规则在同一条规则里立即拿到 root_entity_id。
```

## 3. 施工文件

优先修改：

```text
rust/crates/engine_runtime/src/gameplay_command.rs
rust/crates/engine_runtime/src/logic_executor.rs
rust/crates/engine_runtime/src/project_logic.rs
rust/crates/engine_runtime/src/runtime_trace.rs
```

按需修改：

```text
rust/crates/engine_runtime/src/runtime_instance_loader.rs
rust/crates/engine_runtime/src/frame_loop.rs
```

文档归档：

```text
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/2026-06-29-Runtime-Prefab-Spawn-Despawn-C-min/00-总览.md
```

## 4. 施工步骤

### Step 1：扩展 GameplayCommand

新增：

```text
GameplayCommand::InstantiatePrefab
GameplayCommand::DespawnPrefabInstance
```

字段：

```text
InstantiatePrefab:
  prefab_ref: RuntimeAssetRef
  parent_entity: Option<SourceEntityId>
  target_scene_instance: Option<RuntimeInstanceId>

DespawnPrefabInstance:
  instance_id: RuntimeInstanceId
```

扩展 `GameplayCommandApplyRecord`：

```text
instance_id: Option<RuntimeInstanceId>
prefab_ref_id: Option<String>
created_entity_count: usize
root_entity_id: Option<RuntimeEntityId>
```

最小测试：

```text
gameplay_command_record_can_describe_prefab_instantiate
despawn_prefab_instance_record_can_report_missing_instance
```

### Step 2：新增 runtime-aware command apply

保留现有：

```text
apply_gameplay_commands(world, commands)
```

新增：

```text
apply_gameplay_commands_with_runtime(world, commands, runtime_package, runtime_instance_loader)
```

规则：

```text
没有 runtime context 时，InstantiatePrefab / DespawnPrefabInstance 返回 failed + missing_runtime_context。
有 runtime context 时，调用 RuntimeInstanceLoader。
```

最小测试：

```text
instantiate_prefab_command_requires_runtime_context
instantiate_prefab_command_creates_runtime_prefab_instance
despawn_prefab_instance_command_removes_owned_entities
```

### Step 3：扩展 LogicContext API

新增：

```text
request_instantiate_prefab(prefab_ref, parent_entity, target_scene_instance) -> GameplayCommandId
request_despawn_prefab_instance(instance_id) -> GameplayCommandId
request_despawn_entity(entity_id) -> GameplayCommandId
```

规则：

```text
这些 API 只 enqueue command。
不立即修改 World。
不返回 root entity。
```

最小测试：

```text
logic_context_prefab_request_enqueues_command_only
logic_context_despawn_prefab_request_enqueues_command_only
```

### Step 4：ProjectLogicRunner 接入 Runtime Context

新增可选运行方式：

```text
run_frame_update_with_runtime(...)
run_fixed_update_with_runtime(...)
```

或内部新增：

```text
RuntimeCommandContext
  package
  instance_loader
```

规则：

```text
默认 run_frame_update 仍可用。
默认路径没有 runtime context 时，Prefab 命令给出 structured failed record。
有 runtime context 时，Prefab 命令真正实例化 / 销毁。
```

最小测试：

```text
project_logic_runner_applies_prefab_instantiate_with_runtime_context
project_logic_runner_reports_prefab_command_missing_runtime_context
project_logic_runner_applies_prefab_despawn_with_runtime_context
```

### Step 5：Trace 接入

确保 RuntimeTrace 能记录：

```text
command_enqueue instantiate_prefab
command_apply instantiate_prefab
command_apply despawn_prefab_instance
instance_id
root_entity_id
created_entity_count
error_code
```

最小测试：

```text
runtime_trace_records_prefab_instantiate_apply
runtime_trace_records_prefab_despawn_apply
```

## 5. 验证命令

```text
cargo fmt --check
cargo check --workspace
cargo test -p engine_runtime gameplay_command --no-run
cargo test -p engine_runtime project_logic --no-run
cargo test --workspace --no-run
```

如果 Windows 应用程序控制策略阻止 test binary 执行，需要记录 `os error 4551`，但仍必须保证 `--no-run` 编译通过。

## 6. 完成标准

```text
项目规则可以通过 LogicContext 请求实例化 Prefab。
项目规则可以通过 LogicContext 请求销毁 Prefab Instance。
真实实例化由 ProjectLogicRunner apply point 统一执行。
RuntimeTrace 能记录 enqueue / apply。
没有 runtime context 时有 structured failed record。
有 runtime context 时能创建 / 销毁 RuntimePrefabInstance。
新增能力有 headless 最小测试。
施工文档归档到 已完成。
阶段完成记录补齐。
```

