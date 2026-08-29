# 105-Editor Authoring System C-min 方案

## 定位

Editor Authoring System 是编辑器制作系统的总入口。

它不是重新实现 Scene Editing、Asset Placement、Prefab、Inspector、InputMapping，而是把已有编辑能力收敛成一条长期稳定的制作链路：

```text
User / AI / Test
  -> AuthoringRequest / UiCommandPayload
  -> EditorSession
  -> SceneEditCommand / AssetPlacementResolver / typed authoring helper
  -> SceneEditTransaction
  -> EditorSceneDocument
  -> PreviewWorldSync
  -> Save / Play / Report
```

## 其它引擎对比

Unity 的制作链路是 Project -> Hierarchy / SceneView -> GameObject + Component -> Inspector -> Undo / Dirty / Save -> Play。

UE 的制作链路是 Content Browser -> Level Viewport / Outliner -> Actor / Component -> Details -> Transaction / Dirty / Save -> PIE。

Godot 的制作链路是 FileSystem -> SceneTree / Viewport -> Node / Resource / PackedScene -> Inspector -> Save / Run。

Bevy 没有成熟官方编辑器，但 ECS / Reflect / Scene Asset 方向说明：编辑器应通过结构化数据和反射/Schema 查看并修改 Entity / Component。

我们的路线学习 Unity 的简单心智、UE 的事务边界、Godot 的短链路，并加入 AI 可读的命令和报告。

## 已有系统边界

本系统不重新讨论：

```text
85 Scene Editing v1 C-min
86 真实 UI 命令接入 Scene Editing C-min
89 真实可编辑小项目闭环 C-min
90 ProjectAsset-to-SceneEntity Authoring C-min
91 AI 图片生成到项目资源库闭环 C-min
95 Physics2D Foundation C-min
98 Input Mapping Asset C-min
```

它们继续作为底座存在。

## 正式规则

```text
1. EditorSceneDocument 是编辑真相。
2. Runtime World 不是编辑真相。
3. PreviewWorld 是预览结果，可全量重建。
4. 所有 Scene 源数据修改必须走 EditorSession -> SceneEditTransaction。
5. UI / AI / Test 不能直接写 Scene 文件。
6. 资源进入 Scene 必须创建 Entity + Component / AssetRef，不把资源本体写入 Scene。
7. Prefab 第一版只作为通用 PrefabInstance 组件进入 Scene，不做嵌套 Prefab override。
8. Inspector 第一版由已有 Component 字段和 Schema/metadata 派生视图驱动，字段修改走 SetComponentField。
9. Collider2D Authoring 第一版只保存通用 engine.collider2d 组件数据，不自动触发项目玩法。
10. InputMapping Authoring 第一版只验证和保存 InputMappingAsset，不引入复杂 rebinding UI。
11. Authoring Report 是 AI 和测试的主要验收入口。
12. 引擎只提供通用 authoring 底座，不新增 player/enemy/bullet/health/score 等项目语义 API。
```

## C-min 范围

第一版补齐一个综合 authoring 验收闭环：

```text
Open editable Scene
Create generic Entity
Place asset into Scene
Create PrefabInstance authoring entity
Create Collider2D authoring entity
Edit existing component field through Inspector path
Validate InputMappingAsset
Save Scene
Emit EditorAuthoringReport
```

第一版不做：

```text
真实 Viewport Gizmo
复杂鼠标拾取
多选批量编辑
Prefab Mode
嵌套 Prefab override
复杂 Inspector schema renderer
复杂 Input rebinding UI
Collider 可视化拖拽编辑
```

## 报告结构

```text
EditorAuthoringReport
  schema_version
  project_id
  opened_scene
  scene_id
  created_entity_id
  placed_asset_entity_id
  prefab_entity_id
  collider_entity_id
  inspector_edit_applied
  input_mapping_validated
  dirty_before_save
  dirty_after_save
  console_entry_count
  diagnostics[]
```

## 结论

Editor Authoring System C-min 采用汇总型长期入口，不制造第二套编辑系统。

它的价值是把现有模块组织成一个 AI 可生成、可验证、可追踪的制作闭环，同时保持引擎层只提供通用底座能力。
