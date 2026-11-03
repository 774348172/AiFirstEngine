# 140-当前可自动化施工文档：M7 Prefab Workflow v1

## 1. 施工目标

按 `140-M7-Prefab-Workflow-Reusable-Authoring-Object-System-v1方案.md` 实施第一版 Prefab 编辑器产品工作流：

```text
PrefabAsset authoring truth
Scene PrefabInstance
PrefabOverride
ResolvedPrefabView
Inspector override edit
Asset Browser prefab listing / placing
RuntimePackageBuilder prefab resolve
Project Rule InstantiatePrefab compatibility
PrefabWorkflowReport
```

本次施工不是重做运行时实例化；`99-Runtime-Prefab-Spawn-Despawn-C-min方案.md` 继续有效。

## 2. 不做什么

第一版不做：

```text
Prefab Variant
完整 Nested Prefab
完整 Prefab Mode
复杂 Apply / Revert UI
Prefab 继承系统
项目玩法专用 API
```

不得出现以下引擎内置类型：

```text
Player
Enemy
Bullet
Health
Damage
Score
Wave
Weapon
Boss
Drop
```

## 3. 涉及文件

实际涉及：

```text
rust/crates/editor_core/src/prefab_workflow.rs
rust/crates/editor_core/src/lib.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
```

## 4. 分阶段任务

### A0：现状审计

状态：

```text
已完成。
```

结论：

```text
运行时已有 RuntimePrefabData / RuntimeInstanceLoader / GameplayCommand::InstantiatePrefab。
编辑器已有 WorkspaceDomainKind::Prefab 和 AssetPlacementResolver 对 prefab asset_type 的基础支持。
缺口是 authoring prefab 真相层、instance overrides、resolved view、report，以及 RuntimePackageBuilder 对 authoring prefab 的转换。
```

### A1：Prefab authoring 数据模型

状态：

```text
已完成。
```

完成：

```text
PrefabAsset
PrefabEntity
PrefabInstance
PrefabOverride
PrefabRef
ResolvedPrefabView
PrefabWorkflowReport
PrefabWorkflowService
```

### A2：Scene 中放置 PrefabInstance

状态：

```text
已完成。
```

完成：

```text
PrefabInstance 可通过 engine.prefab_instance 组件落在 Scene Entity 上。
PrefabInstance 支持从 Scene Entity 解析。
PrefabWorkflowService 可创建 Scene prefab instance entity。
```

### A3：Inspector Override 编辑

状态：

```text
已完成。
```

完成：

```text
PrefabWorkflowService::override_from_inspector_edit
PrefabWorkflowService::write_override_to_instance_entity
ResolvedPrefabView 应用 component override / transform override。
```

### A4：Apply / Revert 最小命令

状态：

```text
部分完成。
```

完成：

```text
PrefabInstance::remove_override
PrefabWorkflowService::revert_override
```

未做：

```text
完整 Apply / Revert UI。
```

### A5：Asset Browser Prefab 产品流

状态：

```text
复用现有基础能力。
```

说明：

```text
现有 AssetPlacementResolver 已支持 asset_type=prefab。
完整 Asset Browser 产品化归 M9，不在 M7 中扩张。
```

### A6：RuntimePackageBuilder 展开 Prefab

状态：

```text
已完成。
```

完成：

```text
runtime-prefab.v1 原样写入。
authoring-prefab-asset.v1 转换为 runtime-prefab.v1。
unsupported prefab schema 产生 RuntimePackageDiagnostic。
```

### A7：Project Rule InstantiatePrefab 兼容

状态：

```text
已验证。
```

说明：

```text
已有 engine_runtime prefab command / ProjectLogicRunner 测试继续通过。
```

### A8：PrefabWorkflowReport

状态：

```text
已完成。
```

完成：

```text
prefab_assets_count
prefab_instances_count
overrides_count
resolved_instances_count
failed_instances_count
diagnostics
```

### A9：完整回归

状态：

```text
已完成。
```

通过：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

## 5. 验收结果

已满足：

```text
1. 能从 Entity Tree 创建 PrefabAsset。
2. 能表达 Scene 中的 PrefabInstance。
3. Instance 字段编辑能表达为 PrefabOverride。
4. ResolvedPrefabView 能正确应用 override。
5. RuntimePackageBuilder 能展开 authoring PrefabAsset。
6. Project Rule InstantiatePrefab 既有 runtime 链路继续通过。
7. 错误能通过 PrefabWorkflowReport / RuntimePackageDiagnostic 定位。
8. 所有新增能力有最小测试。
9. 未新增项目玩法 API。
```

部分留到后续系统：

```text
Save / reload 的完整项目文档级 Prefab 工作流。
完整 Apply / Revert UI。
Asset Browser 产品化。
Schema-driven Inspector 深度集成。
```

## 6. 施工自审

结论：

```text
施工完成，符合 M7 B-min 边界。
没有重做 Runtime Prefab Spawn / Despawn。
没有引入玩法专用 API。
没有新增平行 Bridge / Projection 概念。
```
