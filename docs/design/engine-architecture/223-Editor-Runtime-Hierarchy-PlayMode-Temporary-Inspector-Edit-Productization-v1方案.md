# 223-Editor Runtime Hierarchy / Play Mode Temporary Inspector Edit Productization v1 方案

## 1. 这个系统是干什么的

本系统解决 Play 之后运行时对象“看得见、选得到、临时能改”的问题。

复杂打飞机项目进入 Play 后，会有运行时生成的敌人、子弹、特效、掉落物。222 已经让 GameView 点击对象后显示 Runtime Inspector，但用户仍然缺少一棵运行时实体树。本系统让同一个 Hierarchy 面板在 Play Mode 自动显示 active runtime World，并让 Inspector 支持 Play Mode 临时编辑。

最终用户心智：

```text
Edit Mode:
  Hierarchy = 编辑态 Scene
  Inspector = 可编辑，修改会进入 authoring 数据

Play Mode:
  Hierarchy = 运行中的 Runtime World
  Inspector = 可编辑，但只是当前 Play Session 临时修改
  Stop Play 后临时修改全部丢弃
```

明确修正上一轮讨论中的问题：

```text
不做用户可见的 Authoring Scene / Active Runtime / Opened RuntimePackage domain toggle。
domain 只作为内部 selection_source / hierarchy_source，用于 AI、测试、Inspector 路由和 diagnostics。
Hierarchy 用户体验向 Unity 对齐：一个 Hierarchy，随 Play 状态自动切换上下文。
```

## 2. 其它引擎对标

### Unity

Unity 用户体验是单一 Hierarchy：

```text
Edit Mode 显示当前 Scene GameObject 树。
Play Mode 显示运行中的 Scene 对象状态。
Inspector 在 Play Mode 可以修改对象字段，但默认不保存到编辑态资源。
Stop Play 后运行时改动丢弃。
```

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\HierarchyEditor\Managed\HierarchyWindow.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\Hierarchy\Managed\HierarchyView.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\HierarchyCore\ScriptBindings\Hierarchy.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Selection\Selection.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector
```

可学习：

```text
一个 Hierarchy 面板，不暴露复杂 domain toggle。
Hierarchy / Inspector / Selection 联动。
Play Mode 允许临时调参。
```

不可照搬：

```text
Unity 的 GameObject 同时承担 authoring 和 runtime 心智，本项目必须区分 Scene Document、RuntimePackage 和 active runtime World。
Unity 的 Play Mode 改动丢失常被用户误解，本项目必须在 Inspector 明确标记 Temporary / Discard on Stop。
```

### Unreal Engine

UE 对标是 World Outliner + Details Panel + PIE World。

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Public\PlayInEditorDataTypes.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\PlayLevel.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\Kismet2\DebuggerCommands.cpp
Engine\Source\Editor\SceneOutliner
Engine\Source\Editor\PropertyEditor
```

可学习：

```text
PIE 是编辑器内临时运行 World，不是直接把编辑态世界当运行真相。
UI 层只发 command / request，不直接创建或销毁 World。
Details Panel 由属性模型驱动，不让控件私自改底层对象。
```

不可照搬：

```text
不做 UE 级完整 debugger / Watch / Timeline / Blueprint debug。
不引入复杂 Slate / PropertyEditor 定制体系。
```

### Godot

Godot 对标是 SceneTree + Inspector。可学习的是短链路和清晰选择反馈；不可照搬的是其 Node 同时具备更强脚本运行对象心智。本项目仍坚持 RuntimePackage / World / EditorSession 边界。

## 3. 本项目当前基线

已完成前置能力：

```text
217 Editor Play 使用 Preview RuntimePackage。
218 EditorRuntimePlayInstance 创建 active runtime World。
219 Editor GameView GPU texture sharing 主线。
220 GameView input / AUI consumed / gameplay fallback。
221 Pause / Resume / StepFrame / StopPlaySession。
222 GameView runtime pick + selected_entity_source + Runtime Inspector。
```

当前代码状态：

```text
editor_ui_model::HierarchyModel 已存在。
editor_core::build_hierarchy_model 当前优先显示 editor_scene_document，否则显示 opened RuntimePackage world。
editor_core::EditorSession 已有 editor_runtime_play_instance。
EditorRuntimePlayInstance 已暴露 runtime_world() 只读 accessor。
engine_runtime::World 已有 EntityMeta + Hierarchy(parent_id / sibling_order)。
222 已有 EntitySelectionSource::ActiveGameViewRuntime。
```

缺口：

```text
Hierarchy 还不会在 Play Mode 自动显示 active runtime World。
Inspector 目前对 active runtime selection 是 readonly。
没有 session-local runtime edit command。
没有 Play Mode temporary edit evidence。
Stop Play 后没有显式报告已丢弃 runtime 临时修改。
```

## 4. 正式方案：Unity-like Single Hierarchy Auto Source

采用方案：

```text
方案 B-revised:
  单一 Hierarchy 面板。
  不显示 domain toggle。
  Hierarchy source 根据编辑器状态自动选择。
  Play Mode Inspector 支持临时编辑。
```

Hierarchy source 规则：

```text
如果 active editor_runtime_play_instance 存在:
  Hierarchy 显示 active runtime World
  selection_source = active_game_view_runtime
  Inspector 标记 Play Mode / Temporary

否则如果 editor_scene_document 存在:
  Hierarchy 显示 authoring Scene Document
  selection_source = authoring_scene
  Inspector 可编辑并写入 authoring transaction

否则如果 opened RuntimePackage world 存在:
  Hierarchy 显示 opened RuntimePackage world
  selection_source = opened_runtime_package
  Inspector 默认只读

否则:
  Hierarchy 空
```

用户界面规则：

```text
不出现 Authoring Scene / Active Runtime / Opened RuntimePackage 三段切换按钮。
Play 状态由 Toolbar / GameView / Inspector 标识传达。
Hierarchy 标题或状态文本可显示 Play Mode，不能变成可交互 domain toggle。
Inspector 在 Play Mode 字段旁显示 Temporary / Discard on Stop 语义。
Play Mode 中主 Hierarchy 显示 active runtime World，不再同时承担 authoring Scene 编辑入口。
Stop Play 后主 Hierarchy 自动恢复 authoring Scene。
```

内部模型必须新增：

```text
HierarchySourceDomain:
  AuthoringScene
  ActiveGameViewRuntime
  OpenedRuntimePackage

InspectorPersistence:
  PersistentAuthoring
  TemporaryPlaySession
  ReadOnlyRuntimePackage
```

这些字段只给代码、AI、测试、Report Panel 使用，不作为复杂用户操作入口。
施工时必须保证 UI model / command result / gate report 能读出这些字段，否则 AI 无法稳定判断当前编辑是持久化还是 Play 临时修改。

## 5. Play Mode Temporary Inspector Edit

本轮用户要求：Play Mode Inspector 必须能编辑；Stop Play 后修改丢弃。

因此 223 v1 不再延续 222 的 readonly 限制，而是升级为：

```text
ActiveGameViewRuntime Inspector:
  editable=true
  persistence=temporary_play_session
  discard_policy=discard_on_stop_play
```

编辑路径：

```text
Hierarchy / GameView 选中 runtime entity
  -> Inspector 显示 runtime component fields
  -> 用户编辑字段
  -> UiCommandPayload::SetRuntimeComponentFieldTemporary
  -> EditorSession 校验 selected_entity_source == ActiveGameViewRuntime
  -> 调用 EditorRuntimePlayInstance::apply_temporary_component_edit(...)
  -> 只影响当前 active runtime World
  -> StepFrame / 下一帧继续使用修改后的 runtime 值
  -> StopPlaySession drop editor_runtime_play_instance
  -> 修改全部丢弃
```

写入边界：

```text
不暴露通用 runtime_world_mut() 给 EditorSession 任意调用。
只暴露窄接口:
  apply_temporary_component_edit(entity_id, component_type, field_path, value)

该接口内部必须:
  校验 entity 存在。
  校验 component 存在。
  校验 field_path 在 temporary allowlist 中。
  校验 value type。
  写入 World。
  记录 RuntimeTemporaryEditSummary。
  输出 diagnostic。
```

允许编辑的 C-min 字段：

```text
Transform:
  local_position
  local_rotation
  local_scale

Renderable:
  visible

SpriteRenderer2D:
  visible
  color
  sorting_layer
  order_in_layer
  sort_z

Collider2D:
  enabled

Dynamic component:
  仅允许 schema 标记为 runtime_temporary_editable 的 bool / number / string / Vec3。
```

`Collider2D.offset` 和其它 Vec2 字段不进入 223 C-min，除非施工 Gate 明确先新增 `InspectorValue::Vec2 / InspectorValueType::Vec2`、序列化、属性编辑转换和测试。默认 C-min 不为 Vec2 新增 UI 类型。

本轮不允许：

```text
新增 / 删除 entity。
新增 / 删除 component。
修改 hierarchy parent / sibling_order。
修改 asset ref / material ref / texture ref。
修改 AUI Document。
写回 Scene Document / Prefab / Rule / RuntimePackage。
把 runtime temporary edit 进入 persistent undo / dirty / save。
```

如果字段不可编辑，Inspector 必须给出清晰原因：

```text
field_not_runtime_temporary_editable
entity_not_active_runtime
entity_missing
component_missing
unsupported_value_type
requires_apply_to_authoring_deferred
```

## 6. Stop Play 丢弃规则

Stop Play 是临时编辑的唯一清除边界：

```text
StopPlaySession:
  clear editor_runtime_play_instance
  clear selected_entity_id if selected_entity_source == ActiveGameViewRuntime
  clear selected_entity_source
  clear runtime temporary edit summary
  emit command diagnostic:
    runtime_temporary_edits_discarded
    edited_entity_count
    edited_field_count
```

计数来源：

```text
RuntimeTemporaryEditSummary 必须保存在 EditorRuntimePlayInstance 或 EditorSession 的运行期状态中。
它只记录轻量摘要:
  edited_entity_ids
  edited_field_paths
  edited_field_count
  last_edited_entity_id
  last_edited_field_path
  discard_policy=discard_on_stop_play

不记录完整 World dump。
不记录每帧 field history。
不进入 persistent undo / dirty / save。
```

不做文件写入：

```text
不写 Scene。
不写 Prefab。
不写 RuntimePackage。
不写 project assets。
不标记 dirty。
不进入 export。
```

用户如果想保留 Play Mode 修改，后续单独做：

```text
Apply Runtime Change To Authoring v1
```

该后续系统必须解决 source entity mapping、字段安全写回、Prefab override、Undo/Dirty/Save、AI patch evidence，不塞进 223。

## 7. AI / Report / 测试语义

AI 必须能从结构化模型判断：

```text
Hierarchy 当前显示的是 active runtime World，不是 authoring Scene。
Inspector 当前编辑是 temporary_play_session，不会保存。
Stop Play 后这些 edit 已丢弃。
某个字段为什么可以或不可以临时编辑。
当前 selection 来自 GameView pick 还是 Runtime Hierarchy click。
```

Report 分档遵守项目规则：

```text
runtime hot path 默认不输出完整 World dump。
Editor report 可保留 Summary。
Trace 只用于测试 / debug / 用户显式诊断。
```

建议新增轻量 Summary：

```text
RuntimeHierarchySummary:
  source_domain
  entity_count
  selected_entity_id
  temporary_edit_count
  stale_selection

RuntimeTemporaryEditSummary:
  edited_entity_count
  edited_field_count
  last_edited_field_path
  discard_policy
```

不新增：

```text
FullWorldStateDump
每帧完整 entity/component JSON
统一 RuntimeStateSnapshot
常驻 Trace report
```

## 8. 复杂打飞机验收目标

最小体验：

```text
打开复杂打飞机项目。
点击 Play。
Hierarchy 自动显示 active runtime World 中的 Player / Enemy / Bullet / HUD-independent world entities。
点击 Hierarchy 中的 runtime entity。
Inspector 显示 Play Mode / Temporary 标识。
修改 Transform.local_position.x。
GameView / runtime World 使用修改后的值。
StepFrame 后该值仍在当前 Play Session 中生效。
点击 Stop。
再次 Play，修改不保留，回到 RuntimePackage / authoring 初始值。
```

自动化 gate：

```text
headless deterministic，不依赖真实 OS window。
构造 active EditorRuntimePlayInstance。
build_ui_model 时 Hierarchy 使用 active runtime World。
执行 RuntimeHierarchySelectEntity command。
Inspector editable=true 且 persistence=temporary_play_session。
执行 SetRuntimeComponentFieldTemporary 修改 Transform。
断言 runtime world 值改变。
执行 StepFrame，断言修改仍存在。
执行 StopPlaySession，断言 runtime instance 清空，selection 清空，discard diagnostic 存在。
重新 Play，断言修改未写回 RuntimePackage / authoring Scene。
```

## 9. 与 222 的关系

222 完成的是：

```text
GameView 点击 runtime entity。
Runtime selection source 消歧。
Runtime Inspector 只读显示。
```

223 接续升级：

```text
Hierarchy 也能选择 active runtime entity。
Runtime Inspector 从只读升级为 Play Mode temporary editable。
StopPlaySession 丢弃临时编辑。
```

222 的 readonly 结论仅作为 C-min 历史边界；223 之后，active runtime Inspector 的正式规则变为：

```text
editable=true
persistence=temporary_play_session
discard_on_stop_play=true
```

Opened RuntimePackage Inspector 仍保持只读，避免用户误以为能修改发布产物。

## 10. Deferred 边界

不进入 223：

```text
Apply Play Mode edits to authoring。
runtime edit undo stack。
runtime multi-selection。
Runtime Hierarchy 搜索 / 过滤 / component badges 完整版。
Runtime spawn / despawn authoring commands。
Runtime entity reorder / parenting edit。
GPU object-id picking。
3D depth / transparent picking。
breakpoint / watch / timeline / replay。
跨进程 player inspect。
多 GameView / 多 RuntimeInstance inspect。
```

## 11. 推荐施工 Gate

```text
Gate A: 现状锁定
  确认 222 已完成，当前无施工文档。
  读取 HierarchyModel / InspectorModel / EditorRuntimePlayInstance / StopPlaySession 基线。

Gate B: Hierarchy auto source
  HierarchyModel 必须增加 source_domain / status。
  build_hierarchy_model 优先 active editor_runtime_play_instance.runtime_world()。
  UI 不新增可见 domain toggle。

Gate C: Runtime hierarchy selection
  新增或复用 command 选择 active runtime hierarchy entity。
  selection_source 写入 ActiveGameViewRuntime。
  Inspector 与 222 runtime selection 共用同一条源消歧。

Gate D: Temporary runtime inspector edit
  InspectorModel 必须支持 persistence=temporary_play_session。
  新增 SetRuntimeComponentFieldTemporary command。
  通过 EditorRuntimePlayInstance::apply_temporary_component_edit(...) 窄接口写入 active runtime World，不暴露通用 runtime_world_mut()。
  默认不做 Vec2 字段；如要做 Vec2，先补 InspectorValue::Vec2 全链路。
  不写 authoring 文档。

Gate E: Stop Play discard
  StopPlaySession 清除 runtime instance 和 runtime selection。
  使用 RuntimeTemporaryEditSummary 输出 runtime_temporary_edits_discarded diagnostic / summary。
  重新 Play 后修改不保留。

Gate F: e2e / 文档 / 回归
  project_e2e_gate 覆盖复杂打飞机 Play Mode temporary edit。
  cargo fmt --check
  cargo test -p editor_core
  cargo test -p project_e2e_gate
```

## 12. 最终结论

223 采用 Unity-like 单 Hierarchy 心智：

```text
不显示 domain toggle。
Play Mode 自动显示 active runtime World。
Inspector 支持 Play Mode 临时编辑。
Stop Play 后临时编辑丢弃。
不写回 authoring，后续单独做 Apply To Authoring。
```

这个方案比只读 Runtime Inspector 更接近 Unity 的实际使用体验，同时仍保留本项目需要的 AI-first 结构化边界：每次临时编辑都有 command、diagnostic、field path、persistence 和 discard policy，AI 和测试不会误判它是持久化修改。
