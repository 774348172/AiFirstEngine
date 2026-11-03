# 144-当前可自动化施工文档-M11 Physics2D Collider Authoring Visualization C-min

## 1. 方案审查结论

对应正式方案：

```text
144-M11-Physics2D-Collider-Authoring-Visualization-C-min方案.md
```

审查结果：

```text
通过。
```

原因：

```text
1. 方案聚焦 M11：Collider2D authoring / visualization / runtime verification。
2. 不重做 Physics2D runtime，不新增项目玩法 API。
3. 当前代码已有 Collider2D、Scene Editing、PreviewWorldSync、Physics2D trace，可增量施工。
4. 第一版只做 Aabb / Circle、Inspector 字段、Viewport overlay model、保存与运行验证，范围可控。
5. 长期按 Unity / UE 式完整 Collider 编辑器保留方向，但第一版不引入完整 gizmo / polygon / CollisionProfile。
```

## 2. 施工目标

实现 M11 C-min：

```text
Scene Entity
  -> Collider2D component
  -> Inspector / SceneEditTransaction 可编辑 Collider2D 字段
  -> ColliderDebugDrawList
  -> ViewportModel 暴露 collider overlay summary
  -> Scene save / reload
  -> RuntimePackage / Hydration / Physics2D pair report 验证
```

## 3. 不做什么

本轮不做：

```text
Polygon / Capsule / Edge / Composite collider
完整拖拽 gizmo
碰撞层矩阵 UI
命名 CollisionProfile
物理材质
碰撞事件玩法 API
运行时刚体动力学
Native UI 真实线框绘制细节
```

禁止新增项目玩法 API：

```text
Player
Enemy
Bullet
Health
Damage
Score
Weapon
Boss
Drop
Hit
Kill
```

## 4. 涉及文件

预计修改：

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/inspector_details.rs
rust/crates/editor_core/src/lib.rs
```

必要时小范围修改：

```text
rust/crates/engine_runtime/src/physics2d.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
```

## 5. 分阶段施工

### Gate A：Collider2D Schema / Inspector / Transaction

目标：

```text
Schema-driven Inspector 能表达 engine.collider2d 字段。
SceneEditTransaction 能稳定写 Collider2D component field。
PreviewWorldSync 能继续把 engine.collider2d 转成 runtime Collider2D。
```

测试：

```powershell
cargo test -p editor_core collider2d_inspector
cargo test -p editor_core scene_edit_transaction_set_collider2d
```

### Gate B：ColliderDebugDrawList

目标：

```text
新增 ColliderDebugDrawList / ColliderDebugDrawItem / ColliderDebugShape / ColliderDebugDiagnostic。
从 EditorSceneDocument + SceneSelection 生成 debug draw list。
Aabb / Circle 支持 world center、size/radius、enabled、sensor、selected。
缺 Transform 或非法尺寸产生 diagnostic。
```

测试：

```powershell
cargo test -p editor_core collider_debug_draw_list
```

### Gate C：ViewportModel 接入

目标：

```text
editor_ui_model::ViewportModel 暴露 collider_overlay。
EditorSession::build_viewport_model 从当前 Scene document 生成 collider overlay。
Runtime snapshot viewport 保持兼容，空 overlay 即可。
```

测试：

```powershell
cargo test -p editor_ui_model collider_overlay
cargo test -p editor_core editor_session_collider_overlay
```

### Gate D：Runtime 验证闭环

目标：

```text
Authoring scene 中 Collider2D 保存后能 reload。
PreviewWorldSync / RuntimePackage / Hydration / Physics2D pair report 能识别 Collider2D。
```

测试：

```powershell
cargo test -p editor_core collider2d_authoring_runtime_roundtrip
cargo test -p engine_runtime physics2d
```

### Gate E：整体回归

目标：

```text
确认 M11 没有破坏 UI model、EditorCore、Runtime Physics2D。
```

测试：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime
```

## 6. 验收标准

必须满足：

```text
1. Collider2D 字段能由 Inspector / SceneEditTransaction 修改。
2. Aabb Collider 可保存 / reload。
3. Circle Collider 可进入 debug draw list。
4. ViewportModel 能暴露 collider overlay summary。
5. 选中 Entity 的 Collider 能被标记 selected。
6. 缺 Transform 的 Collider2D 会产生 diagnostic。
7. 非法 Aabb / Circle 尺寸会产生 diagnostic。
8. 两个重叠 Collider2D 进入 runtime 后能生成 Physics2D pair report。
9. 不新增项目玩法 API。
10. 整体回归通过。
```

## 7. 完成后文档同步

完成后：

```text
1. 将本施工文档移动到 施工文档/已完成/。
2. 新增 阶段完成记录/2026-07-02-M11-Physics2D-Collider-Authoring-Visualization-C-min/00-总览.md。
3. 更新 54-当前可自动化施工总入口.md。
4. 更新 施工文档/README.md。
5. 更新 49-当前状态与下一步入口.md。
6. 必要时更新 00-文档地图.md。
```

## 8. 施工文档自审

### 8.1 Specification fit

施工文档覆盖 M11 C-min：Inspector、SceneEditTransaction、DebugDrawList、ViewportModel、Runtime 验证。

### 8.2 Rule fit

施工边界不重做 Physics2D runtime，不新增项目玩法 API，符合引擎底座能力规则。

### 8.3 Textual consistency

Gate A-D 对应正式方案 5-7 节，测试命令与验收标准一致。

### 8.4 Design fit

施工按大系统 Gate 推进，不拆成孤立小点；数据以结构化 model / report 暴露，符合 AI-first。

### 8.5 Implementation feasibility

当前代码已有 `engine_runtime::physics2d::Collider2D`、`EditorSceneComponent`、`PreviewWorldSync`、`ViewportModel`，可增量实现。

### 8.6 Practical reasonableness

第一版不做完整 gizmo 和 polygon，避免过度膨胀；每个 Gate 都有可跑测试。

结论：

```text
施工文档通过自审，可以开始施工。
```
