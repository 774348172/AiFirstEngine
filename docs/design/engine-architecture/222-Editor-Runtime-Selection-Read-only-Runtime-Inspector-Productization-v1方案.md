# 222-Editor Runtime Selection / Read-only Runtime Inspector Productization v1 方案

## 1. 一句话说明

本系统让用户在 Editor GameView Play 运行、暂停或单帧推进时，可以点选运行时对象，并在 Inspector 中查看该对象当前帧的真实 runtime component 状态。

```text
Editor GameView Play
  -> 用户点击运行画面中的世界对象
  -> 记录 runtime selection target
  -> 通过 EditorSession.editor_runtime_play_instance 找到 active EditorRuntimePlayInstance
  -> 通过新增只读 runtime_world() / world() accessor 读取 live World
  -> 复用 InspectorModel 生成只读 Runtime Inspector
```

它对标 Unity 的 Scene/Game 选中对象后 Inspector 显示 live object 属性，也对标 UE Viewport 选中 Actor 后 Details Panel 显示 live UObject / Actor 属性。

本系统不是新建 debugger 大系统，不是新增 `EditorRuntimeStateSnapshot`，也不是新建统一 runtime report。Inspector 显示仍靠当前已有的 `InspectorModel`。

审查后修正：222 不是“无新建地接到现有链路”。它的真实核心新建量是：

```text
EditorRuntimePlayInstance 暴露只读 live World accessor。
InspectorModel builder 新增 active GameView runtime live World 数据源。
Inspector 数据源优先级调整为 runtime selection 可盖过 authoring Scene selection。
runtime selection 必须带 source 消歧。
```

## 2. 背景与问题

217-221 已经完成了 Editor 内 Play 的主链路：

```text
217 Preview RuntimePackage cache
  -> 218 In-process Editor GameView runtime instance
  -> 219 shared GPU texture GameView present
  -> 220 GameView input / AUI consumed / gameplay fallback
  -> 221 Pause / Resume / StepFrame / Stop / Maximize on Play
```

现在缺口变成：

```text
用户能运行、暂停、单帧推进复杂打飞机项目，
但不能直接点选运行时对象并在 Inspector 中查看它现在的 Transform / Renderable / Collider / runtime metadata。
```

这会影响复杂项目调试：

```text
打飞机:
  暂停后想看某颗 bullet 的位置、可见性、碰撞体、source entity 映射。

自走棋:
  暂停战斗回合后想看某个棋子的运行时位置、状态组件、渲染信息。

复杂 UI / AUI:
  需要区分点到的是 AUI 节点、运行时世界对象，还是空白区域。
```

220 已明确 `WorldPickCollector` deferred；221 也把 `runtime state inspector` / 完整 debugger 类能力留为 deferred。222 同时收敛这两项 deferred 的 C-min：不是全量 debugger，而是 Runtime Selection + 只读 Runtime Inspector。

## 3. 成熟引擎源码参考

### 3.1 Unity

Unity 的普通 Inspector 路线可以概括为：

```text
SceneView / Picking
  -> Selection.activeObject / activeGameObject / activeEntityId
  -> InspectorWindow / ActiveEditorTracker
  -> GenericInspector
  -> SerializedObject(targets)
  -> SerializedProperty iterator
  -> UI 显示 / 编辑 live target property
```

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\SceneView\SceneViewPicking.cs
  PickGameObject(Vector2 mousePosition)
  HandleUtility.PickObject(...)

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Selection\Selection.bindings.cs
  Selection.activeObject
  Selection.activeGameObject
  Selection.activeEntityId
  Selection.objects

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector\Core\GenericInspector.cs
  new SerializedObject(targets, context)
  SerializedObject.Update()
  SerializedObject.GetIterator()
  SerializedObject.ApplyModifiedProperties()
```

可学习点：

```text
Inspector 的真相不是一份额外 snapshot，而是 selection target + property access wrapper。
Selection 保存对象 / id，Inspector 根据当前选中对象构建属性视图。
运行时调试时，用户心智是“我选中了这个对象，所以 Inspector 显示它”。
```

不可照搬点：

```text
Unity 以 GameObject / Component / SerializedObject 为核心。
本项目运行时真相是 RuntimePackage hydrate 后的 World / ECS data，不是 GameObject。
本项目 Inspector 输出必须是 AI 可审查的 InspectorModel。
```

### 3.2 Unreal Engine

UE 的普通 Details Panel 路线可以概括为：

```text
Viewport interaction
  -> GEditor->SelectActor(...)
  -> SDetailsView::SetObject / SetObjects
  -> RootPropertyNodes
  -> SDetailsViewBase::UpdatePropertyMaps
  -> Details layout
```

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\ViewportInteraction\Private\ViewportInteractor.cpp
  GEditor->SelectNone(...)
  GEditor->SelectActor(...)

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\PropertyEditor\Private\SDetailsView.cpp
  SDetailsView::SetObject(UObject*)
  SDetailsView::SetObjects(...)
  SDetailsView::SetObjectArrayPrivate(...)
  SelectedObjects
  RootPropertyNodes

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\PropertyEditor\Private\SDetailsViewBase.cpp
  SDetailsViewBase::UpdatePropertyMaps()
  DetailLayouts
  RootPropertyNodes
```

可学习点：

```text
Details Panel 绑定的是当前选中 UObject / Actor，不是先复制一份 runtime state snapshot。
Property tree / detail layout 是展示模型，不是新的运行时真相。
```

不可照搬点：

```text
UE 的 UObject reflection / Actor selection 体系很重。
本项目应复用现有 UiCommandPayload、EditorSession selection、InspectorModel，不复制 DetailsView 框架。
```

## 4. 本项目当前代码基线

当前已经具备：

```text
rust/crates/editor_ui_model/src/inspector.rs
  InspectorModel
  InspectorSection
  InspectorField
  InspectorValue
  readonly

rust/crates/editor_core/src/ui_model_composer.rs
  build_inspector_model()
  当前已支持：
    selected_aui_node -> AUI Node Inspector
    editor_scene_document + scene_selection.primary_entity_id -> authoring Scene Inspector，可编辑
    selected_entity_id + world -> opened RuntimePackage world Inspector，只读

rust/crates/editor_core/src/session.rs
  scene_selection
  selected_aui_node
  selected_entity_id
  editor_runtime_play_instance
  last_game_view_runtime_frame
  last_game_view_present_report

rust/crates/editor_core/src/editor_gameview_play.rs
  EditorRuntimePlayInstance
  RuntimePackage
  World
  EngineHostLoop
  AuiInteractionState
  pause / resume / step_next_frame / stop
  GameViewPresentReport

rust/crates/engine_runtime/src/world.rs
  entity_count
  archetype_count
  entity_ids
  runtime_id_for_source
  entity
  transform
  renderable
  sprite_renderer2d
  collider2d
  component_value
  query_entities
```

注意：

```text
EditorRuntimePlayInstance 当前持有 live World，但 world 字段是私有字段。
222 施工必须新增只读 accessor，例如 runtime_world() -> &World。
ui_model_composer 当前 selected_entity_id + world 分支读取的是 opened RuntimePackage 静态 world，
不是 active GameView runtime 的 live world。
```

当前关键缺口：

```text
build_inspector_model 当前优先 authoring Scene selection。
editor_runtime_play_instance 的 live World 还不是 Inspector 数据源。
GameView 点击世界对象还没有形成 runtime selection。
SelectEntity 当前可以表达 runtime/opened package world selection，但没有明确区分 active GameView runtime selection。
220 留下 WorldPickCollector deferred。
```

因此 222 不需要从零发明 Inspector。它需要把 active GameView runtime selection 接到现有 `InspectorModel` 生成链路中，并明确新增 live World accessor、runtime data source 分支和 selection source 消歧。

## 5. 关键设计修正

本轮讨论后确认以下修正为正式规则：

```text
1. 不新增 EditorRuntimeStateSnapshot 作为 Inspector 真相。
2. 不新增统一 runtime state report。
3. Inspector 的显示真相是 InspectorModel。
4. InspectorModel 的数据源按上下文来自：
   selected_aui_node
   authoring Scene selection
   active GameView runtime selection
   opened RuntimePackage world selection
5. 当 active GameView runtime 正在 Running / Paused / Stepping，且存在 runtime selection 时，
   Runtime Inspector 优先于 authoring Scene Inspector。
6. Runtime Inspector 默认只读，不写回运行中 World。
```

为什么不新增 `EditorRuntimeStateSnapshot`：

```text
Unity / UE 普通 Inspector 都不是靠新建一份 runtime snapshot 作为主真相。
本项目已有 InspectorModel，重复加 snapshot 会多一层同步、失效、优先级和 bug 来源。
用户已经担心“遇到问题就新增结构层”，222 必须避免这个问题。
```

为什么不新增统一 runtime report：

```text
Report Panel 已经有 provider 机制。
GameViewPresentReport 已经存在。
222 的主需求是用户看 Inspector，不是测试系统看 report。
测试需要证据时，直接断言 selection state / CommandResult / InspectorModel 即可。
必要时只在既有 GameViewPresentReport 中补极轻量 summary 字段，不新建常驻 report 系统。
```

## 6. 可选方案

### 6.1 方案 A：只扩展 GameViewPresentReport

内容：

```text
GameViewPresentReport 增加 picked_entity_id / runtime_selection_summary。
Inspector 不接 active runtime world。
```

优点：

```text
施工很小。
测试容易看到 report。
```

问题：

```text
用户仍然不能像 Unity / UE 一样在 Inspector 中看对象。
把交互调试误导成 report 阅读。
Report 不是用户编辑器主视图。
```

结论：

```text
不选。Report 只能作为证据，不是 222 的产品本体。
```

### 6.2 方案 B：新增 EditorRuntimeStateSnapshot

内容：

```text
每帧从 active runtime world 提取 EditorRuntimeStateSnapshot。
Inspector / AI / Report Panel 都读取 snapshot。
```

优点：

```text
AI 可读性直观。
可以离线保存调试状态。
```

问题：

```text
新增一层真相。
需要处理 snapshot 和 World 的同步、生命周期、内存成本、字段覆盖范围。
用户只想点击对象看 Inspector，却被迫维护一份状态镜像。
Unity / UE 普通 Inspector 并没有用这种模型。
```

结论：

```text
不选。它适合后续 timeline / replay / crash dump，不适合 222 主线。
```

### 6.3 方案 C：Runtime Selection + Read-only Runtime Inspector

内容：

```text
GameView world pick
  -> runtime selection target
  -> InspectorModel 从 editor_runtime_play_instance.runtime_world() 构建
  -> 只读 Runtime Inspector
```

优点：

```text
用户心智最接近 Unity / UE。
复用现有 InspectorModel，不新增架构层。
AI 可以直接读取结构化 InspectorModel。
复杂打飞机调试最直接。
```

风险：

```text
需要把 authoring selection、AUI selection、runtime selection 的优先级讲清楚。
WorldPickCollector C-min 不能一次膨胀成全量 3D / 透明 / 深度 buffer picking。
运行时对象只读，不能让用户误以为正在编辑源 Scene。
```

结论：

```text
采用方案 C-min。
```

## 7. 推荐方案 C-min

222 的正式方案：

```text
Editor Runtime Selection / Read-only Runtime Inspector C-min
```

核心链路：

```text
GameView pointer click in inspect/select mode
  -> GameView focus / coordinate transform 复用已完成的 220 结果
  -> 复用 220 的 AUI interaction consumed result
  -> 如果 AUI consumed: 不做 world pick
  -> 如果 AUI 未消费: 222 自建 WorldPickCollector C-min 判定
  -> 选中 runtime entity id
  -> EditorSession runtime selection state + selection_source
  -> build InspectorModel from editor_runtime_play_instance.runtime_world()
  -> Inspector 显示只读 runtime fields
```

施工前置：

```text
222 依赖 220 已完成的 GameView-local coordinate transform 与 AUI consumed filter。
当前 220 已完成并归档，222 只继承该链路，不重新实现 220。
222 不依赖未落地的 HitCandidateDomain / HitCandidateSortKey / full router。
WorldPickCollector C-min 自建 “AUI consumed -> 未 consumed 则 world pick” 判定。
```

### 7.1 输入模式

为避免破坏普通 gameplay 输入，C-min 应采用保守触发：

```text
优先策略:
  Paused 状态下，GameView click 可以进入 runtime inspect/select。

可选策略:
  Running 状态下，必须显式开启 Debug Select / Inspect Mode，才允许 world pick。

默认:
  普通 Running gameplay click 仍按 220 进入 AUI consumed / gameplay fallback，不被 runtime selection 抢走。
```

这和 Unity 用户常见心智接近：

```text
Play 中调试对象时，用户通常暂停或显式切到观察/调试操作。
普通游玩点击不应突然变成选择对象，导致射击/拖拽/按钮行为被吞掉。
```

### 7.2 Selection 语义

现有 selection 需要收敛成清晰边界：

```text
SelectSceneEntity:
  authoring Scene Document selection。
  Inspector 可编辑源 scene component。

SelectAuiNode:
  AUI Document node selection。
  Inspector 编辑 AUI Document node。

SelectEntity:
  当前用于 opened RuntimePackage world selection。
  222 可以继续复用为 runtime entity selection，但必须在 CommandResult / source 中说明来自 active_game_view_runtime。
```

222 C-min 不允许让同一个 `selected_entity_id` 在 active GameView runtime 与 opened RuntimePackage world 之间保留双语义。必须在施工中前置消歧。

首选做法：

```text
复用现有 SelectEntity command id。
EditorSession 增加 runtime selection source / domain 字段，例如：
  selection_source: active_game_view_runtime | opened_runtime_package | authoring_scene | aui_node
CommandResult 必须输出 source。
InspectorModel builder 必须按 source 选择 live world 或 opened package world。
```

如果 command schema 难以清晰表达，则新增更明确命令：

```text
SelectRuntimeEntity:
  runtime_instance_id
  entity_id
  source: active_game_view_runtime | opened_runtime_package
```

取舍建议：

```text
C-min 优先复用现有 SelectEntity，减少命令面。
但不允许延后 source 消歧；source 字段或 SelectRuntimeEntity 二选一必须进入施工。
这不是新增架构层，只是让 selection state 可审查、可测试。
```

### 7.3 Inspector 数据源优先级

推荐优先级：

```text
1. selected_aui_node 存在:
   显示 AUI Node Inspector。

2. active GameView runtime 存在，且 runtime selection 存在:
   显示 Read-only Runtime Inspector。

3. editor_scene_document 存在，且 scene_selection.primary_entity_id 存在:
   显示 authoring Scene Inspector。

4. opened RuntimePackage world 存在，且 selected_entity_id 存在:
   显示 opened package Runtime Inspector。

5. 否则:
   No Selection。
```

注意：

```text
当前 build_inspector_model 是先看 editor_scene_document，再看 selected_entity_id + world。
222 施工时必须调整优先级，否则 Play 中选中的 runtime entity 会被 authoring Scene 分支挡住。
```

### 7.4 Runtime Inspector 字段范围

C-min 只读展示以下字段：

```text
Metadata:
  runtime entity id
  source entity id，如果 World 能映射
  name / kind / enabled
  runtime instance/session id
  scene id

Transform:
  localPosition
  localRotation
  localScale

Renderable / SpriteRenderer2D:
  meshRef / materialRef / visible
  sprite / texture / tint / layer，如当前 runtime component 已有

Collider2D:
  shape
  size / radius
  offset
  isTrigger / enabled，如当前 runtime component 已有

Raw Component Json:
  对 C-min 未专门格式化的 component_value，允许只读 JSON 展示。
```

不进入 C-min：

```text
运行时编辑 component 值。
把 runtime 修改写回 Scene Document。
多选 runtime Inspector。
复杂 property drawer。
watch expression。
breakpoint / timeline。
```

## 8. Runtime Pick C-min

222 不做 full GPU picking。C-min 的 WorldPickCollector 应小而真实：

```text
输入:
  GameView-local pointer position
  active GameView runtime frame / viewport descriptor
  World entity list
  Transform
  SpriteRenderer2D / Renderable
  Collider2D 或可计算 bounds

输出:
  pick_status: not_requested | hit | miss | blocked_by_aui | unsupported
  selected_runtime_entity_id
  candidate_count
  reason / diagnostic
```

候选策略：

```text
2D / sprite / collider 项目:
  用 Transform + Collider2D 或 Sprite bounds 做 screen-space C-min 命中。

无 collider 但有 sprite:
  可用 sprite/render bounds fallback。

无可计算 bounds:
  返回 unsupported diagnostic，不假装命中。
```

排序策略：

```text
优先尊重当前 render / visual order 可用字段：
  composition stage 不适用于 world sprite。
  sprite layer / z / draw order / entity order 以当前 runtime 已有字段为准。

如果当前没有完整排序字段:
  只做 deterministic fallback，并在 diagnostic 中说明 pick_order=fallback。
```

延后：

```text
3D camera ray + depth buffer picking。
透明像素级 picking。
GPU object id buffer。
复杂 UI/world 混排全域排序。
多 camera / split view picking。
```

## 9. 与 AUI / Scene / Hierarchy 的边界

AUI Node 的真相仍是 AUI Document：

```text
AUI Node 不变成 Runtime ECS Entity。
Runtime AUI interaction 先于 world pick。
如果点击命中 AUI 并被 consumed，222 不再选择 world entity。
如果 AUI 未 consumed，才允许进入 world pick。
```

Scene authoring selection 和 runtime selection 必须区分：

```text
Edit Mode / Scene authoring:
  选中的是 Scene Document entity。
  Inspector 可编辑。

Play Mode / GameView runtime inspect:
  选中的是 Runtime World entity。
  Inspector 只读。

映射关系:
  如果 runtime entity 能通过 runtime_id_for_source / source entity id 找回源 entity，
  Inspector 可以同时显示 sourceEntityId，但不自动切回 authoring selection。
```

Hierarchy 表达建议：

```text
C-min 不要求把 runtime entity 全量塞进 authoring Hierarchy。
可以在 Inspector 标题 / metadata 中明确 Runtime / Read-only。
后续如需 Runtime Hierarchy，应单独讨论，不塞进 222。
```

## 10. Report / AI / 测试边界

222 遵守当前 report 规则：

```text
Runtime report / trace 必须 Off / Summary / Trace 分档。
正式 runtime 默认 Off 或 compact result。
Editor Report Panel 只接 Summary / Trace 产物。
不把 runtime 热路径变成常驻 report 系统。
```

分档消歧：

```text
222 的 Off / Summary / Trace 只约束 runtime selection / world pick / runtime inspector evidence。
既有 RenderFrameReportLevel { Off, Stats, Summary, Evidence } 属于 render frame report，不在 222 中回填或重命名。
这里的 Trace 不是 engine_runtime::runtime_trace 时间线，也不是每帧完整 World dump。
后续如需统一全项目 report level，单独开 report-level convergence，不塞进 222。
```

222 的主证据不是新 report，而是：

```text
CommandResult:
  selection changed
  pick_status
  diagnostics

EditorSession state:
  selected runtime entity id
  active runtime instance still alive

InspectorModel:
  readonly=true
  title
  selected_entity_id
  sections
  fields
```

如确实需要 Report Panel evidence，只允许轻量复用既有 report：

```text
GameViewPresentReport optional summary fields:
  runtime_pick_status
  selected_runtime_entity_id
  runtime_inspector_status
```

不新增：

```text
EditorRuntimeStateSnapshotReport
UnifiedRuntimeStateReport
FullWorldStateDump
每帧完整 entity/component JSON dump
```

AI 需要能回答：

```text
现在选中的是 AUI Node、authoring Scene entity，还是 runtime entity？
Inspector 为什么只读？
runtime entity 是否仍存在？
这个 runtime entity 是否能映射回 source entity？
点击没有选中对象，是被 AUI consumed、没有候选、还是 pick 不支持？
```

## 11. 复杂打飞机验收目标

222 完成后，复杂打飞机项目应具备：

```text
Play 后 GameView 显示运行画面。
Pause 后点击子弹 / 敌人 / 玩家 sprite 或 collider。
Inspector 显示 Runtime / Read-only 标识。
Inspector 能显示该对象的 runtime Transform。
如果对象有 SpriteRenderer2D / Collider2D，Inspector 能显示对应只读字段。
StepFrame 后 Inspector 再构建时能看到新一帧的 runtime 值。
点击 HUD / AUI 按钮时，不误选 world entity。
点击空白区域时返回 miss diagnostic。
```

最小自动化 gate：

```text
测试默认 headless deterministic，不依赖真实 OS window / GPU readback。
构造 active EditorRuntimePlayInstance。
注入或使用已有 sample world 中的 Transform / SpriteRenderer2D / Collider2D entity。
构造 GameView-local click，WorldPickCollector 命中 entity。
执行 selection command。
build_ui_model / build_inspector_model 产出 readonly Runtime Inspector。
断言 selected_entity_id、section_id、field path、readonly。
构造 AUI consumed click，断言不改变 runtime selection。
真实 GameView pick smoke 只能作为 optional / local-only，不阻塞默认 gate。
```

## 12. Deferred 边界

不进入 222 C-min：

```text
运行时属性编辑。
runtime 修改写回 authoring Scene。
runtime multi-selection。
完整 Runtime Hierarchy。
EditorRuntimeStateSnapshot。
统一 runtime state report。
Full HitTraceReport。
GPU object id picking。
3D depth/camera ray picking。
透明像素级 picking。
breakpoint / watch / timeline / replay。
跨进程 player runtime inspect。
多 GameView / 多 RuntimeInstance inspect。
```

这些属于后续系统，不应该为了 222 额外加层。

## 13. 后续施工建议 Gate

进入施工文档时建议拆成：

```text
Gate A: 现状锁定
  读取 217-221 完成记录。
  锁定 InspectorModel / build_inspector_model / EditorRuntimePlayInstance / World API。
  确认 220 已完成 GameView-local coordinate transform 与 AUI consumed filter。
  确认 222 不依赖未落地的 full HitCandidate router。

Gate B: Runtime selection 状态
  明确复用 SelectEntity + selection_source，还是新增 SelectRuntimeEntity。
  不允许 selected_entity_id 保留 active runtime / opened package 双语义。
  CommandResult 必须说明 source=active_game_view_runtime。
  EditorRuntimePlayInstance 必须新增只读 runtime_world() / world() accessor。

Gate C: WorldPickCollector C-min
  从 GameView-local pointer + runtime world 生成 deterministic hit/miss。
  AUI consumed 时不做 world pick。
  自建 AUI-consumed-then-world-pick 判定，不挂不存在的 HitCandidate router。
  核实 SpriteRenderer2D / render order 字段；缺失时使用 deterministic fallback + diagnostic。

Gate D: InspectorModel 数据源优先级
  build_inspector_model 新增 active GameView runtime live World 数据源。
  active GameView runtime selection 优先于 authoring Scene selection。
  opened RuntimePackage world 分支继续用于静态 package inspector。
  Runtime Inspector readonly=true。

Gate E: Tests / Report Panel 最小证据
  测试默认 headless deterministic。
  单测覆盖 hit / miss / blocked_by_aui / missing entity。
  project_e2e_gate 证明 complex shooter runtime inspect C-min。
  不新增统一 runtime report。
  真实窗口 / GPU pick smoke 只作 optional / local-only。

Gate F: 文档同步与回归
  更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
  跑 editor_core / editor_input / project_e2e_gate 相关测试与 cargo fmt --check。
```

## 14. 自审

本方案满足当前项目规则：

```text
没有新增 Runtime 架构层。
没有新增 EditorRuntimeStateSnapshot。
没有新增统一 runtime report。
没有把 Inspector 真相从 InspectorModel 改走。
没有把 AUI Node 改成 Runtime ECS Entity。
没有让 runtime 热路径默认写完整 trace。
没有把 WorldPickCollector C-min 膨胀成 full debugger / GPU picking。
保持 Unity / UE 用户心智：选中对象，Inspector/Details 显示属性。
保持本项目真相：RuntimePackage hydrate 后的 World 是运行时数据源，InspectorModel 是 UI 展示模型。
```

当前结论：

```text
222 采用方案 C-min：
  Editor Runtime Selection / Read-only Runtime Inspector Productization v1

它接续 220/221：
  220 解决 GameView 输入进入 runtime。
  221 解决 Pause / Step / Stop。
  222 解决 Pause / Step 时点选 runtime object 并用 Inspector 查看当前 runtime state。
```

## 15. 外部审查吸收

审查文档：

```text
其它AI审查目录/40-222-Editor-Runtime-Selection-Read-only-Runtime-Inspector方案审查.md
审查对象：222-Editor-Runtime-Selection-Read-only-Runtime-Inspector-Productization-v1方案.md
审查日期：2026-07-08
```

分类处理：

```text
必须修改:
  MF-1 已吸收：方案明确新增 EditorRuntimePlayInstance live World accessor、
    InspectorModel builder 新增 active runtime live World 数据源、并调整数据源优先级。
  SF-2 已吸收：runtime selection 必须前置 source 消歧，
    不允许 selected_entity_id 在 active runtime / opened package 之间保留双语义。
  SF-3 已吸收：222 自建 AUI-consumed-then-world-pick 判定，
    不依赖未落地的 full HitCandidate router。
  SF-5 已吸收：测试默认 headless deterministic，真实窗口 / GPU smoke 只作 optional。
  SF-1 已吸收：session 字段名修正为 editor_runtime_play_instance。
  SF-6 已吸收：222 同时收敛 220 WorldPickCollector deferred 与 221 runtime inspector deferred。

施工约束:
  Report 分档采用项目当前规则 Off / Summary / Trace，
    但不回填 RenderFrameReportLevel { Off, Stats, Summary, Evidence }。
  Gate C 必须核实 SpriteRenderer2D / render order 字段，
    缺失时使用 deterministic fallback + diagnostic。
  Gate B 必须在 source 字段或 SelectRuntimeEntity 中二选一落地。

已由历史施工吸收:
  审查中提到 220 未施工的风险，与当前入口状态不一致。
  当前 220 已完成并归档，GameView-local coordinate transform 与 AUI consumed filter 已有完成记录。
  222 只继承该前置能力，不重复施工 220。

不适用 / 暂不采纳:
  Godot / Bevy inspector 参考为可选增强，不影响当前 C-min 决策。
  统一全项目 report-level convergence 不进入 222，后续单独讨论。
  49 / 54 的施工状态不在本方案中改写；进入施工文档时再按施工流程同步当前施工入口。
```
