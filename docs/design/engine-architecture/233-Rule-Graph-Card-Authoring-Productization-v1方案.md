# 233-Rule Graph / Card Authoring Productization v1 方案

> 状态：正式方案，用户已确认采用 `方案 B`。  
> 校准日期：2026-07-10。  
> 所属路线：`227` 的 `P1-2 Rule Graph / Card Authoring Productization v1`。  
> 前置：`193 Rule Authoring Productization v1`、`195 Gameplay Rule Asset / Rust Framework / IR 红线 / AUI 逻辑边界`、`229 Complex Shooter Gameplay Rule Runtime Execution v1` 已完成。  
> 本文只生成方案，不允许直接施工；施工前仍需审查/自审、施工文档、分 Gate 测试。

## 0. 用户确认结论

本系统确认采用：

```text
方案 B:
  Editable Rule Cards
  + Generated Read-only Rule Graph Preview
  + RuleAuthoringService / structured patch / report 复用
```

落地范围收敛为：

```text
B-min:
  卡片是可编辑主入口。
  图是由 Gameplay Rule Asset / RuleSlot 派生的只读预览。
  图节点点击可以选中对应卡片和 source path。
  v1 不允许自由拖线、不允许用户直接编辑 edge、不新增通用视觉脚本语言。
```

一句话含义：

```text
让用户和 AI 不再面对 raw JSON / 裸 Canonical Rule IR，
而是通过规则卡片编辑 Trigger / Condition / Operation，
同时用只读流程图看清规则从输入到结果的执行关系。
```

核心红线：

```text
Rule Graph / Card 是 Gameplay Rule Asset / RuleSlot 的编辑视图。
Gameplay Rule Asset / RuleSlot 仍是用户和 AI 的规则资产真相。
Canonical Rule IR 是内部规范语义和构建输入。
Runtime 执行仍走 RuntimePackage.rules -> RuleModuleRegistry -> ProjectLogicRunner。
不新增 Blueprint / Lua / VisualScript 式通用脚本系统。
```

## 1. 这个系统是干什么的

它解决的是 P1 自由编辑体验问题：

```text
P0 已证明复杂打飞机能导出、能看见、能运行规则、HUD 能读真实状态。
P1-1 已让用户可以在编辑器里 Build And Run。
下一步需要让用户不写 JSON，也能编辑“开火、移动、碰撞、计分”这些项目规则。
```

本系统在主线中的位置：

```text
Rule Authoring Panel
  -> Rule Card Model
  -> RuleAuthoringService structured edit command
  -> Gameplay Rule Asset / RuleSlot
  -> Canonical Rule IR internal semantic
  -> Validation / Build Report
  -> RuntimePackage rules
  -> ProjectLogicRunner

Rule Graph Preview
  <- derived from Gameplay Rule Asset / RuleSlot
  <- source path / card id / diagnostic path
```

它不是：

```text
不是完整节点图编辑器。
不是新 IR 语言。
不是项目逻辑运行层。
不是打飞机专用规则 API。
不是替代 Rust Project Framework。
```

## 2. 其它引擎/工具对标

### 2.1 Unity

官方参考：

```text
Unity GraphView API:
https://docs.unity3d.com/ScriptReference/Experimental.GraphView.GraphView.html
```

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\GraphViewEditor\Views\GraphView.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\GraphViewEditor\Elements\Node.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\GraphViewEditor\Elements\Port.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\GraphViewEditor\Elements\Edge.cs
```

关键类/机制：

```text
GraphView
GraphViewChange
nodeCreationRequest
graphViewChanged
GetCompatiblePorts
Node / Port / Edge
```

可学习：

```text
图编辑 UI 需要节点、端口、边、选择、删除、连接校验和 change event。
用户视觉理解需要 graph surface。
```

不照搬：

```text
GraphView 是 experimental API，完整照搬会引入大量 UI/交互复杂度。
本项目 v1 不做自由连线，不让图成为规则真相。
```

### 2.2 Unreal Engine

官方参考：

```text
Blueprints Visual Scripting:
https://dev.epicgames.com/documentation/unreal-engine/blueprints-visual-scripting-in-unreal-engine
```

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\BlueprintGraph\Classes\K2Node.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\BlueprintGraph\Classes\EdGraphSchema_K2.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\KismetCompiler\Public\KismetCompiler.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\GraphEditor\Public\SGraphPanel.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\GraphEditor\Public\SGraphNode.h
```

关键类/机制：

```text
UK2Node
ValidateNodeDuringCompilation
AllocateDefaultPins
ExpandNode
CreateNodeHandler
UEdGraphSchema_K2
CanCreateConnection
TryCreateConnection
FKismetCompilerContext::Compile
```

可学习：

```text
节点图必须有 schema 约束、连接校验、编译/展开和诊断。
source path / compiler diagnostic 对长期维护很重要。
```

不照搬：

```text
UE Blueprint 是完整 gameplay scripting system。
本项目已经裁定 IR 不能膨胀成 Blueprint 式脚本语言。
v1 只学习“schema 校验和可回溯诊断”，不复制完整 K2/Blueprint。
```

### 2.3 Godot

官方参考：

```text
Godot 4.0 will discontinue VisualScript:
https://godotengine.org/article/godot-4-will-discontinue-visual-scripting/
```

启发：

```text
通用视觉脚本系统如果长期维护、学习成本和生态支持不足，会变成负担。
因此本项目不应为了“看起来像图”就提前做完整视觉脚本。
```

### 2.4 Bevy

官方参考：

```text
bevy_ecs Schedule:
https://docs.rs/bevy/latest/bevy/ecs/schedule/struct.Schedule.html
bevy_ecs systems:
https://docs.rs/bevy_ecs/latest/bevy_ecs/
```

启发：

```text
复杂执行和调度仍应由 Rust systems / schedule / runner 承担。
图或卡片适合做 authoring view，不适合取代 runtime 执行模型。
```

## 3. 本项目当前基线

已经完成的规则底座：

```text
193 Rule Authoring Productization v1
  RuleAuthoringService
  RuleAuthoringModel / Document / Report / Patch / Diagnostic
  structured trigger / statement / operation edit
  deterministic human explanation template
  Rule UiCommandPayload
  Workflow / ManualWalkthrough Rule domain coverage

225 Project Authoring Asset Completeness / Prefab-Rule Assetization Gate v1
  complex shooter sample 已有 Rule authoring assets。

229 Complex Shooter Gameplay Rule Runtime Execution v1
  RuntimePackage.rules -> RuleModuleRegistry -> ProjectLogicRunner -> FrameLoop 已打通。
```

相关代码：

```text
rust/crates/editor_core/src/rule_authoring.rs
rust/crates/editor_core/src/services/rule_service.rs
rust/crates/editor_ui_model/src/rule_authoring.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/project_e2e_gate/src/rule_authoring.rs
rust/crates/engine_runtime/src/project_rule_asset.rs
rust/crates/engine_runtime/src/rule_ir.rs
rust/crates/engine_runtime/src/rule_compiler.rs
rust/crates/engine_runtime/src/rule_registry.rs
```

当前缺口：

```text
Rule authoring 仍偏 service / command / report。
普通用户仍缺“卡片化”的可读编辑表面。
AI 虽能发结构化 command，但缺少更稳定的 card id / source path / view mapping。
没有 Rule Graph / Rule Card view model。
没有从 Rule Asset 派生的 graph preview。
没有卡片 -> diagnostic -> report -> graph node 的双向定位。
```

## 4. 架构边界

以 `195` / `196` 为最高解释：

```text
用户心智是 Rust Project Framework + Project Assets。
Gameplay Rule Asset 是 Project Assets 的一类，不等于全部项目逻辑。
IR 只存在于 Contract-bound RuleSlot 中，是受限规则数据。
复杂算法、复杂状态机、复杂 UI 工作流默认进 Rust Project Module / Rust Framework。
```

本系统必须遵守：

```text
Rule Card 只编辑 Gameplay Rule Asset / RuleSlot 中已有的受限规则结构。
Rule Graph Preview 只从 Rule Asset 派生，不保存 runtime 语义。
Graph layout 可以作为 editor-only view state 保存，但不得影响规则执行。
所有修改必须落到 RuleAuthoringService / structured RuleAuthoringEditCommand。
Validation / build / runtime 执行仍使用现有规则资产管线。
```

禁止：

```text
新增自由节点连接并直接改变 runtime 语义。
新增任意函数、while、递归、任意数组/map 编程。
让 Rule Graph Document 成为第二真相。
让 Graph edge 绕过 RuleAuthoringService 直接写 Canonical Rule IR。
把 Player / Enemy / Bullet / Score 等打飞机概念写入 engine core API。
把复杂 UI 交互或 AUI tree mutation 放进 Rule Graph。
```

## 5. 方案选项回顾

### 方案 A：Card-only

做法：

```text
只做 Trigger / Condition / Operation 卡片编辑，不显示图。
```

优点：

```text
施工最小。
AI 和用户都容易理解。
完全复用 RuleAuthoringService。
```

问题：

```text
规则流向感不足。
复杂打飞机多个规则之间的关系不直观。
用户仍不容易看出“输入 -> 生成子弹 -> 生命周期 -> 碰撞计分”的链条。
```

结论：

```text
可作为方案 B 的子集，不单独作为本轮主方案。
```

### 方案 B：Card + Read-only Graph Preview

做法：

```text
卡片负责编辑。
图负责从规则资产派生出只读流程预览。
点击图节点定位到卡片、诊断和 source path。
图 layout / expanded state 可以作为 editor-only view state。
```

优点：

```text
用户能看懂规则流向。
AI 仍修改结构化 card/patch，不需要操作自由图。
不把系统扩成 Blueprint。
施工范围可控。
长期可以自然升级到更完整的 graph editor。
```

问题：

```text
v1 不是自由节点图编辑器。
用户不能通过拖线直接创造新语义。
需要设计稳定的 card id / graph node id / source path 映射。
```

结论：

```text
推荐，用户已确认。
```

### 方案 C：Full Node Graph Editor

做法：

```text
做完整节点、端口、边、连接校验、节点创建、拖线、编译/source map。
```

优点：

```text
长期上限最高。
最像 UE Blueprint / Unity Visual Scripting。
```

问题：

```text
施工范围大。
极易诱导 IR 膨胀成通用视觉脚本语言。
需要 schema、port typing、edge editing、graph compiler、undo/redo、layout、debugger。
```

结论：

```text
长期 deferred，不作为本轮 v1。
```

## 6. 推荐方案：B-min

正式采用：

```text
B-min: Editable Rule Cards + Generated Read-only Rule Graph Preview
```

### 6.1 编辑真相

编辑真相保持：

```text
Gameplay Rule Asset / RuleSlot
```

内部构建输入保持：

```text
Canonical Rule IR
```

卡片只是视图和命令入口：

```text
RuleTriggerCard
RuleConditionCard
RuleStatementCard
RuleOperationCard
RuleDiagnosticCard
```

每张卡片必须有：

```text
card_id
asset_path
rule_id
source_path
kind
display_title
summary
editable_fields
diagnostic_refs
command_refs
```

卡片编辑必须转成：

```text
RuleAuthoringEditCommand::SetTrigger
RuleAuthoringEditCommand::AddStatement
RuleAuthoringEditCommand::UpdateStatement
RuleAuthoringEditCommand::RemoveStatement
RuleAuthoringEditCommand::AddOperation
RuleAuthoringEditCommand::UpdateOperation
RuleAuthoringEditCommand::RemoveOperation
```

### 6.2 图预览真相

图预览是派生物：

```text
RuleGraphPreviewModel
  nodes[]
  edges[]
  groups[]
  selected_node_id
  source_map[]
```

节点来源：

```text
Trigger -> trigger node
Statement / Condition -> condition/query node
Operation -> operation node
Diagnostic -> diagnostic badge
Runtime phase -> phase/group label
```

边来源：

```text
规则内部的固定执行顺序。
trigger -> statements -> operations。
多 statement / operation 按 canonical order 生成 edge。
```

v1 不保存：

```text
graph edge semantic
node connection semantic
runtime execution override
```

可保存的 editor-only view state：

```text
node_position
zoom
pan
expanded_groups
selected_card_id
```

规则：

```text
view state 不参与 validate/build/runtime。
view state 丢失时必须能从 Gameplay Rule Asset 重新生成 graph preview。
```

### 6.3 UI 心智

普通用户看到：

```text
左侧：规则列表
中间：Rule Cards
右侧或下方：Read-only Graph Preview
底部：Validation / Build / Diagnostics
```

用户编辑路径：

```text
打开 Rule Asset
  -> 选择 Trigger Card
  -> 选择或修改 action.fire
  -> 添加 Operation Card: Instantiate Prefab
  -> Validate
  -> Graph Preview 自动刷新
  -> Diagnostic 可定位到卡片和图节点
```

AI 编辑路径：

```text
读取 RuleCardAuthoringModel
  -> 生成 RuleAuthoringPatch
  -> 如来自 card UI 临时交互，先由 editor-only card adapter 降低为 RuleAuthoringPatch
  -> 走 RuleAuthoringService apply
  -> validate/build report
  -> graph preview 重新派生
```

## 7. 数据模型建议

新增 editor_ui_model 层模型：

```text
RuleCardAuthoringModel
  project_root
  selected_path
  rule_count
  document: RuleAuthoringDocument
  cards: Vec<RuleCardModel>
  graph_preview: RuleGraphPreviewModel
  commands: Vec<RuleAuthoringCommand>
  report_summary
```

```text
RuleCardModel
  card_id
  kind
  asset_path
  rule_id
  source_path
  title
  summary
  human_explanation
  fields
  allowed_commands
  diagnostics
```

```text
RuleCardFieldModel
  field_id
  label
  field_path
  value_kind
  value_preview
  editable
  enum_options
  asset_ref_options
  validation_state
```

```text
RuleGraphPreviewModel
  schema_version
  asset_path
  rule_id
  ir_hash
  nodes
  edges
  groups
  layout_state
  diagnostics
```

```text
RuleGraphPreviewNode
  node_id
  card_id
  source_path
  kind
  label
  status
  diagnostic_refs
```

```text
RuleGraphPreviewEdge
  edge_id
  from_node_id
  to_node_id
  kind
  label
```

命名规则：

```text
RuleCardAuthoringModel 是产品面 view model。
RuleCardAuthoringModel 只能作为 RuleAuthoringModel 的派生产品面 / 子模型。
RuleCardAuthoringModel 不替代 RuleAuthoringModel，不改变 RuleAuthoringService::build_model 主入口。
RuleGraphPreviewModel 是只读预览 view model。
不得新增 RuleGraphDocument 作为规则真相。
如果后续需要保存布局，只能新增 RuleGraphViewState / editor-only metadata。
```

## 8. Command / Patch 设计

新增或扩展 command：

```text
OpenRuleCard
SelectRuleCard
SetRuleCardField
AddRuleCard
RemoveRuleCard
ValidateRuleCards
BuildRuleFromCards
RefreshRuleGraphPreview
SelectRuleGraphNode
ResetRuleGraphViewLayout
```

约束：

```text
SetRuleCardField 必须翻译成现有 RuleAuthoringEditCommand。
AddRuleCard 必须翻译成 AddStatement 或 AddOperation。
RemoveRuleCard 必须翻译成 RemoveStatement 或 RemoveOperation。
SelectRuleGraphNode 只能改变 selection，不修改规则资产。
RefreshRuleGraphPreview 只能重新派生 view model。
```

v1 默认不启用以下命令：

```text
DuplicateRuleCard
MoveRuleCard
```

原因：

```text
当前 RuleAuthoringEditCommand 尚无 duplicate / reorder 语义。
如果施工不显式补 ReorderStatement / ReorderOperation，并补完整验证和测试，
则 DuplicateRuleCard / MoveRuleCard 必须在 UI command 中 disabled，并输出 reason_disabled。
```

AI patch 仍优先使用：

```text
RuleAuthoringPatch
```

v1 默认不新增独立 `RuleCardPatch`。

如果 UI 交互确实需要临时包装，只允许新增 editor-only adapter：

```text
RuleCardPatch
  patch_id
  asset_path
  expected_ir_hash
  operations[]
  source
```

并且必须满足：

```text
RuleCardPatch 不落盘。
RuleCardPatch 不进入 RuntimePackage。
RuleCardPatch 不作为 AI 默认 patch 格式。
RuleCardPatch 必须在 editor_core 内立即降低为 RuleAuthoringPatch / RuleAuthoringEditCommand。
RuleCardPatch 不能直接写文件，不能绕过 RuleAuthoringService。
```

## 9. Diagnostic / Report 规则

报告必须同时服务用户、AI 和测试：

```text
用户看 card title / human_explanation / suggested_fix。
AI 看 source_path / field_path / diagnostic code / expected_ir_hash。
测试看 graph preview hash / card count / diagnostic mapping。
```

RuleCardAuthoringReport 建议字段：

```text
schema_version
status
asset_path
rule_id
ir_hash
card_count
graph_node_count
graph_edge_count
editable_card_count
read_only_graph
changed_paths
diagnostics
next_actions
source_mappings
```

必须证明：

```text
每个 error/warning diagnostic 至少能定位到 source_path。
如果 source_path 对应 card，必须给出 card_id。
如果 card_id 对应 graph node，必须给出 node_id。
图预览不能把 invalid rule 伪装成 valid，只能带红色/错误状态节点。
```

Report 分档：

```text
Editor default:
  Summary。显示卡片数、图节点数、主要诊断和 next actions。

Gate / test:
  Trace。允许输出完整 card model / graph preview snapshot。

Runtime:
  不输出 RuleCard / GraphPreview report；这是 editor-only authoring 产品面。
```

## 10. 与复杂打飞机的最小验收

本系统不新增打飞机 API，只验证复杂打飞机规则能被卡片化展示和编辑。

最小规则样例：

```text
fire_bullet.rule.json
  Trigger Card:
    actionPressed("action.fire")

  Operation Card:
    instantiatePrefab("prefab-player-bullet")

  Graph Preview:
    [Input action.fire] -> [Instantiate prefab-player-bullet]
```

```text
linear_motion.rule.json
  Statement Card:
    forEachQuery(project.linearMotion + Transform)

  Operation Card:
    write Transform.localPosition

  Graph Preview:
    [Update phase] -> [For each moving entity] -> [Write Transform.localPosition]
```

验收报告必须证明：

```text
complex shooter sample 的至少 3 条规则可以生成 Rule Cards。
至少 1 条规则可以通过 SetRuleCardField / AddRuleCard 修改并保存。
修改后 RuleAuthoringService validate/build 仍通过或给出结构化 diagnostic。
Graph Preview 由修改后的 Rule Asset 重新派生。
Diagnostic 可以从 report 定位到 card_id / source_path / node_id。
没有新增 Player / Enemy / Bullet 专用 engine core API。
```

## 11. 推荐施工 Gate

后续生成施工文档时建议拆为以下 Gate。

### Gate A：Rule Card View Model

目标：

```text
新增 RuleCardAuthoringModel / RuleCardModel / RuleCardFieldModel。
从现有 RuleAuthoringDocument / ProjectRuleAsset 派生 cards。
覆盖 trigger / statement / operation / diagnostics。
```

测试：

```text
cargo test -p editor_ui_model rule_card
cargo test -p editor_core rule_authoring
```

### Gate B：Card Command 到 RuleAuthoringService

目标：

```text
新增 SetRuleCardField / AddRuleCard / RemoveRuleCard / SelectRuleCard 等 command。
所有修改都降低到 RuleAuthoringEditCommand。
支持 expected_ir_hash 冲突诊断。
```

测试：

```text
cargo test -p editor_core rule_card
cargo test -p editor_core rule_authoring
```

### Gate C：Generated Read-only Rule Graph Preview

目标：

```text
新增 RuleGraphPreviewModel。
从 Rule Asset 派生 nodes / edges / groups。
实现 card_id / source_path / node_id 映射。
SelectRuleGraphNode 只改变 selection，不修改规则资产。
```

测试：

```text
cargo test -p editor_ui_model rule_graph_preview
cargo test -p editor_core rule_graph_preview
```

### Gate D：Workflow / Manual Walkthrough / Report Panel

目标：

```text
Rules step 显示 card authoring 能力。
ManualWalkthroughCoverageReport 报告 Rule Card / Graph Preview 能力。
Report Panel 能接入 RuleCardAuthoringReport summary。
```

测试：

```text
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core manual_walkthrough
cargo test -p editor_core report_panel
```

### Gate E：Complex Shooter E2E Report

目标：

```text
project_e2e_gate 生成 complex-shooter-rule-card-authoring-productization-report.json。
覆盖 fire_bullet / linear_motion / lifetime_cleanup 等 sample rule。
证明 card edit -> validate/build -> graph preview refresh。
```

测试：

```text
cargo test -p project_e2e_gate rule_authoring
cargo test -p project_e2e_gate rule_card_authoring
```

### Gate F：文档与入口同步

目标：

```text
施工完成后更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
227 中 P1-2 标记完成后，下一轮默认推进 P1-3 Input Mapping Visual Authoring Panel v1。
```

测试：

```text
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p project_e2e_gate
```

## 12. 不做范围

本轮不做：

```text
完整自由节点图编辑器。
自由连线 / port typing / edge creation。
Graph compiler。
Blueprint VM / VisualScript VM。
Runtime graph execution。
Rule debugger / breakpoint / step over。
复杂状态机 / 行为树 / 技能树编辑器。
把复杂 UI 工作流放进 Rule Graph。
把 Rust Project Module 可视化成通用图。
```

这些能力后续如果需要，应另开系统，并重新审查是否会突破 `195` / `196` 的 IR 红线。

## 13. 风险与治理

### 风险 A：Graph 变成第二真相

治理：

```text
Graph Preview 不保存语义。
Graph Preview 可以从 Rule Asset 重新生成。
Graph Preview 修改命令只允许 selection/layout。
```

### 风险 B：卡片编辑绕过现有服务

治理：

```text
所有 card edit 必须走 RuleAuthoringService。
所有 patch 必须有 expected_ir_hash。
所有 changed_paths 必须进入 report。
默认复用 RuleAuthoringPatch，不新增独立持久化 RuleCardPatch。
```

### 风险 B2：产品面模型替换现有 RuleAuthoringModel

治理：

```text
RuleCardAuthoringModel 只能从 RuleAuthoringModel / RuleAuthoringDocument 派生。
RuleCardAuthoringModel 不替代现有 RuleAuthoringModel。
RuleAuthoringService 仍是 create/open/save/validate/build/apply 的唯一服务入口。
```

### 风险 C：诱导 IR 继续膨胀

治理：

```text
卡片类型只覆盖现有 RuleTrigger / RuleStatement / RuleOperation。
新增卡片类型必须先确认对应 RuleSlot 已存在。
如果规则需要 while / recursion / arbitrary function / complex algorithm，提示转 Rust Project Module。
```

### 风险 D：AI 看懂了但用户仍看不懂

治理：

```text
Card title / summary / human_explanation 必须由确定性模板生成。
每个 card field 必须有 label / value preview / suggested fix。
Raw JSON 只能作为 advanced/debug view。
```

### 风险 E：施工范围膨胀成完整图编辑器

治理：

```text
v1 只读 graph preview。
v1 不做 edge editing。
v1 不做 node creation from graph canvas。
v1 不做 graph compiler。
DuplicateRuleCard / MoveRuleCard v1 默认 disabled，除非施工同步补明确的 RuleAuthoringEditCommand 和测试。
```

## 14. 自审

### 2026-07-10 自审小修

本次自审后补充以下施工前约束：

```text
1. 不默认新增 RuleCardPatch。
   v1 优先复用 RuleAuthoringPatch；
   如需 RuleCardPatch，只能作为 editor-only adapter，并立即降低为 RuleAuthoringPatch / RuleAuthoringEditCommand。

2. DuplicateRuleCard / MoveRuleCard 默认 disabled。
   只有当施工同步新增 ReorderStatement / ReorderOperation 或等价 RuleAuthoringEditCommand，
   并补齐验证和测试后，才允许启用。

3. RuleCardAuthoringModel 是 RuleAuthoringModel 的派生产品面 / 子模型。
   它不替代现有 RuleAuthoringModel，也不改变 RuleAuthoringService 的主入口。
```

### AI 适配性

通过。

理由：

```text
AI 读取的是结构化 card / field / source_path / diagnostic。
AI 修改仍走 RuleAuthoringPatch / RuleAuthoringEditCommand。
Report 可回指 card_id / source_path / node_id。
```

### 复杂项目可维护

通过。

理由：

```text
复杂打飞机规则可用卡片和图预览理解。
规则真相仍在 Gameplay Rule Asset / RuleSlot。
复杂 gameplay / UI workflow 不进入 Rule Graph。
```

### 效率

通过。

理由：

```text
Editor-only view model 不进 runtime 热路径。
Graph preview 可按 rule asset / ir_hash 缓存。
Runtime 不读取卡片或图。
```

### 与 195 / 196 是否冲突

不冲突。

理由：

```text
本方案明确 Rule Graph / Card 是视图，不是新语言。
Canonical Rule IR 仍是内部规范语义。
复杂系统流程仍由 Rust Project Framework / Module 承担。
```

### 与 193 是否重复

不重复。

理由：

```text
193 已完成 RuleAuthoringService / structured command / report。
233 补的是产品面：Rule Cards 和只读 Graph Preview。
```

## 15. 结论

正式采用：

```text
B-min: Editable Rule Cards + Generated Read-only Rule Graph Preview
```

它把规则编辑从“结构化服务已经能改”推进到：

```text
用户能看懂、AI 能稳定改、诊断能定位、图能帮助理解，但系统不会膨胀成新脚本语言。
```

下一步如果进入施工，应先生成施工文档，并严格按 Gate 执行：

```text
Rule Card View Model
-> Card Command lowering to RuleAuthoringService
-> Generated Read-only Graph Preview
-> Workflow / Report Panel
-> Complex Shooter E2E Report
-> 文档与入口同步
```

## 16. 参考

外部官方资料：

```text
Unity GraphView API
https://docs.unity3d.com/ScriptReference/Experimental.GraphView.GraphView.html

Unreal Engine Blueprints Visual Scripting
https://dev.epicgames.com/documentation/unreal-engine/blueprints-visual-scripting-in-unreal-engine

Godot 4.0 will discontinue VisualScript
https://godotengine.org/article/godot-4-will-discontinue-visual-scripting/

Bevy ECS Schedule
https://docs.rs/bevy/latest/bevy/ecs/schedule/struct.Schedule.html

Bevy ECS
https://docs.rs/bevy_ecs/latest/bevy_ecs/
```

本项目资料：

```text
193-Rule-Authoring-Productization-v1方案.md
195-Gameplay-Rule-Asset-Rust-Framework-IR-Redline-and-AUI-Logic-Boundary方案.md
196-IR-Rust-vs-Unity-Lua-CSharp-vs-UE-Blueprint-Cpp方案审查.md
225-Project-Authoring-Asset-Completeness-Prefab-Rule-Assetization-Gate-v1方案.md
229-Complex-Shooter-Gameplay-Rule-Runtime-Execution-v1方案.md
227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md
阶段完成记录/2026-07-05-Rule-Authoring-Productization-v1/00-总览.md
阶段完成记录/2026-07-09-Complex-Shooter-Gameplay-Rule-Runtime-Execution-v1/00-总览.md
```
