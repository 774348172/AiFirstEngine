# 193-Rule Authoring Productization v1 方案

> 当前修正：本方案早期以 `ProjectRuleAsset / Canonical Rule IR` 为主入口描述。根据 `194` / `195` / `196`，后续产品化应上移为 `Gameplay Rule Asset / RuleSlot Authoring`。`Canonical Rule IR` 是内部规范语义和构建输入，不是普通用户默认编辑对象。

## 1. 系统一句话说明

`Rule Authoring Productization v1` 解决的是：

```text
用户 / AI 如何在编辑器里创建、打开、编辑、验证、构建和诊断 Gameplay Rule Asset / RuleSlot。
```

它不是新的玩法系统，不是打飞机专用逻辑，不是重写 186 / 187。

它是 186 / 187 之上的产品层：

```text
Rule Authoring UI / Service / Command
  -> Gameplay Rule Asset / RuleSlot
  -> Canonical Rule IR 内部规范语义
  -> Validation / Compile Report
  -> RuleArtifact / RuntimeRuleManifest
  -> RuntimePackage
  -> ProjectLogicRunner
```

一句话：

```text
让项目规则不再只是项目目录里的 JSON / manifest，而是用户和 AI 都能稳定操作的编辑器资产。
```

当前边界：

```text
Rule Authoring 不产品化裸 Canonical IR 编辑器作为主入口。
Rule Authoring 不做完整 Lua / Blueprint 式脚本系统。
复杂流程、复杂 UI 工作流、复杂算法应转 Rust Project Module。
```

## 2. 其它引擎中的对标

### Unity

对标：

```text
Visual Scripting Graph / Script Machine / State Machine
C# Script asset + Inspector serialized fields
```

Unity 官方文档把 Visual Scripting Graph 描述为保存应用逻辑的资产，Script Graph 通过节点和连线表达动作、值、顺序和事件。

我们学习：

```text
规则必须是项目资产。
规则编辑入口要让没有编程基础和只有少量编程基础的用户也能操作。
事件 / 每帧 / 条件 / 状态切换是规则 authoring 的核心心智。
```

不照搬：

```text
不采用 C# / MonoBehaviour 作为规则真相层。
不把图节点本身作为唯一真相层。
不复制 Unity Domain Reload 心智。
```

### Unreal Engine

对标：

```text
Blueprint Visual Scripting
Blueprint Asset / Blueprint Class / 编译 / Gameplay runtime
```

UE 官方文档把 Blueprint 称为通过节点界面在 Unreal Editor 内创建 gameplay 元素的完整 gameplay scripting system。

我们学习：

```text
规则创作、编译、运行、诊断必须闭环。
设计师 / AI 可以操作高层规则资产，底层仍由引擎提供可执行边界。
```

不照搬：

```text
第一版不做完整 Blueprint 图编辑器。
第一版不做 Blueprint VM。
第一版不做 Live Coding 或进程内热替换。
```

### Godot

对标：

```text
GDScript Resource
Node lifecycle
Signal event reaction
```

Godot 的 GDScript 是紧密集成到引擎内容创作的脚本语言，Signal 用于让对象对事件作出反应并减少直接引用。

我们学习：

```text
规则需要清楚地挂到项目对象 / 生命周期 / 事件入口。
事件响应是复杂项目规则的长期核心。
```

不照搬：

```text
不把 Node callback 或动态 Variant 调用作为 AI-first 主线。
不让脚本源码绕过 Canonical Rule IR。
```

### Bevy

对标：

```text
ECS System / Update schedule / Commands
```

Bevy 官方文档示例展示了把 system 加入 Update 并运行 App 的基本模式。

我们学习：

```text
规则读写 ECS 必须有边界。
结构变化走 Commands / deferred apply。
执行计划集中生成。
```

不照搬：

```text
不把完整 Rust System / Schedule / Plugin 心智暴露给普通用户和 AI。
```

## 3. 在本引擎中的作用

复杂打飞机项目需要 gameplay logic，但这些概念不能进入引擎 Core：

```text
Player / Enemy / Bullet / Score / Health / Wave / Weapon
```

正确关系是：

```text
Input Action: fire
  -> ProjectRuleAsset: fire_projectile
  -> Canonical Rule IR: actionPressed + instantiatePrefab
  -> RuntimePackage rule manifest
  -> ProjectLogicRunner
  -> CommandBuffer instantiate prefab
```

也就是说：

```text
打飞机规则属于项目侧；
引擎只提供可编辑、可验证、可构建、可运行的通用规则 authoring 链路。
```

本系统完成后，复杂项目 walkthrough 中 Rule 域的缺口应从：

```text
focus_rule_panel / missing command
```

推进到：

```text
create_rule_asset
open_rule_asset
edit_rule_graph_or_dsl
validate_rule_asset
build_rule_artifact
inspect_rule_diagnostics
```

## 4. 当前本项目基线

已存在的基础：

```text
186-Project-Rule-Asset-Pipeline-Runtime-Execution-C-min方案.md
187-Project-Rule-Artifact-Module-Lifecycle-B-min方案.md
191-Authoring-Walkthrough-Missing-Operations-Convergence-v1方案.md
```

代码中已存在：

```text
rust/crates/engine_runtime/src/project_rule_asset.rs
rust/crates/engine_runtime/src/rule_ir.rs
rust/crates/engine_runtime/src/rule_compiler.rs
rust/crates/engine_runtime/src/rule_artifact.rs
rust/crates/engine_runtime/src/rule_registry.rs
rust/crates/engine_runtime/src/project_logic.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_core/src/authoring_workflow.rs
```

当前已经具备：

```text
ProjectRuleAsset JSON read/write
ProjectRuleAsset validation
Canonical Rule IR trigger / statements / operations
actionPressed trigger
forEachQuery statement
instantiatePrefab / despawnPrefabInstance / emitEvent operation
Rust AOT source / static registry source generation
RuntimeRuleManifest metadata
Manual walkthrough 能发现 Rule authoring 缺口
Authoring workflow 当前只能 focus_rule_panel
```

当前缺口：

```text
没有 RuleAuthoringService。
没有 Rule 相关 UiCommandPayload。
没有 rule asset selection / open document 状态。
没有结构化 Rule 编辑操作。
没有编辑器侧 rule validation / compile / artifact report 入口。
没有把 Rule authoring 覆盖结果从 focus panel 推进到 executable commands。
```

## 5. 设计边界

必须遵守：

```text
Gameplay Rule Asset / RuleSlot 是用户和 AI 的默认编辑对象。
Canonical Rule IR 是内部规范语义和构建输入。
ProjectRuleAsset 是底层项目资产容器，可作为 debug / import-export / validation 对象。
Generated Rust / Artifact / Manifest / Registry 都是派生产物。
Editor 和 AI 默认修改 Gameplay Rule Asset / RuleSlot，不修改 generated Rust。
Runtime 只从 RuntimePackage / RuntimeRuleManifest / StaticRegistry 路径执行。
```

禁止：

```text
新增打飞机专用 Rule API。
把 Rule Authoring 做成手写 Rust 编辑器。
把 raw JSON 文本编辑当成唯一产品体验。
把裸 Canonical IR 表单编辑当成普通用户主体验。
把复杂流程、状态机、UI drag/drop、任意函数或循环塞进 IR。
绕过 ProjectRuleAsset 直接写 RuntimeRuleManifest。
让编辑器规则执行走一套、导出 Player 走另一套。
```

## 6. 可选方案

### 方案 A：Raw JSON Rule Asset Editor

做法：

```text
编辑器只提供 ProjectRuleAsset JSON 打开、编辑、保存、验证。
AI patch 也直接写 JSON。
```

优点：

```text
施工最快。
完全贴近现有 ProjectRuleAsset read/write。
AI 容易生成。
```

缺点：

```text
普通用户体验差。
容易写出结构正确但意图难读的规则。
后期复杂项目维护成本高。
```

结论：

```text
只适合作为 fallback / import-export / debug view，不适合作为 v1 主方案。
```

### 方案 B：完整 Node Graph Rule Editor

做法：

```text
直接做 Unity Visual Scripting / UE Blueprint 式节点图编辑器。
节点图编辑后编译到 Canonical Rule IR。
```

优点：

```text
用户体验上限最高。
长期最接近成熟引擎。
可视化表达事件、条件、循环、操作。
```

缺点：

```text
施工范围过大。
图编辑、连线、布局、选择、撤销、校验、source map 都会膨胀。
容易把重点从规则产品链路拉回 UI 细节。
```

结论：

```text
作为 v2 / C-full 方向保留，不作为本轮 v1。
```

### 方案 C-min+Explain：结构化 Rule Authoring Service + 可读解释

做法：

```text
新增 RuleAuthoringService。
编辑器提供结构化操作：create/open/select/add trigger/add statement/add operation/validate/build/save。
UI 第一版是 rule panel + structured forms，不做自由节点图。
RuleAuthoringReport 必须生成 human_summary / human_explanation / suggested_fix。
AI 使用同一套 command / patch 操作。
所有操作最终修改 ProjectRuleAsset.canonicalIr。
保留 raw JSON debug view，但不是主入口。
```

本方案不是 C+ 多视图系统。v1 不新增：

```text
完整 Rule Card 系统
完整 Readable DSL 编辑器
完整节点图
三层并行编辑视图
```

v1 只要求在结构化规则和诊断旁边生成用户能读懂的解释。例如：

```text
规则 Fire Projectile：
当输入 fire 被按下时，生成 projectile prefab。

规则 Fire Projectile 中，“移动 projectile”步骤写入字段 local_position.x 失败。
字段路径不存在。请检查 Transform 组件里是否叫 localPosition 或 position。
```

优点：

```text
AI 适配性最好：命令、patch、report 都可结构化。
复杂项目可维护：规则仍以 Canonical IR 为真相层。
施工范围可控：不提前陷入完整图编辑器。
用户可接手：AI 修不好时，用户能先读 human_summary / human_explanation，而不是直接面对 IR path。
能直接收敛 191 manual walkthrough Rule 域缺口。
```

缺点：

```text
视觉体验不如完整节点图。
自然语言解释必须由结构化诊断派生，不能成为第二真相层。
复杂分支和图布局需要后续版本补。
```

结论：

```text
推荐作为 Rule Authoring Productization v1 主方案。
```

## 7. 推荐方案

采用：

```text
C-min+Explain：RuleAuthoringService + Structured Rule Commands + Validation / Build Report + Human-readable Explain + Workflow Coverage
```

第一版只做真实闭环：

```text
RuleAuthoringService
RuleAuthoringDocument / RuleAuthoringSession
Rule UiCommandPayload
Rule Command Resolver
Rule Panel model
ProjectRuleAsset create/open/select/save
Canonical IR structured edit
Validate rule asset
Compile rule source / static registry source report
Rule artifact lifecycle validation report
Human-readable Rule Summary
Human-readable Diagnostic Explanation / Suggested Fix
Manual walkthrough Rule domain coverage update
Complex shooter sample rule authoring report
```

第一版不做：

```text
完整节点图编辑器
自由连线画布
完整 Rule Card 多视图系统
完整 Readable DSL 编辑器
复杂调试断点
真实动态 DLL 加载
RuntimeEventQueue 完整事件系统
项目玩法 API
自动生成完整商业级打飞机规则
```

## 8. v1 用户链路

最小用户链路：

```text
打开复杂打飞机项目
  -> 进入 Rules step
  -> Create Rule Asset
  -> 设置 trigger: actionPressed("fire")
  -> 添加 operation: instantiatePrefab(asset.prefab.projectile)
  -> Validate Rule Asset
  -> Build Rule Artifact Report
  -> Save Rule Asset
  -> Export / RuntimePackage
  -> Player 通过 ProjectLogicRunner 执行规则
```

最小 AI 链路：

```text
AI request
  -> RuleAuthoringPatch
  -> validate patch
  -> apply to ProjectRuleAsset
  -> RuleValidationReport
  -> RuleCompileReport
  -> AuthoringAiContext / ManualWalkthroughCoverageReport
```

## 9. 数据模型建议

新增编辑器侧模型，不替代 engine_runtime 里的真相层：

```text
RuleAuthoringDocument
  asset_path
  asset: ProjectRuleAsset
  dirty
  selected_statement_path
  selected_operation_path
  human_summary
  validation_report
  compile_report
  artifact_report

RuleAuthoringPatch
  patch_id
  asset_id
  operations[]
  expected_ir_hash
  source

RuleAuthoringReport
  status
  asset_id
  rule_id
  ir_hash
  human_summary
  diagnostics
  changed_paths
  next_actions
```

RuleAuthoringDiagnostic 必须补充用户可读字段：

```text
RuleAuthoringDiagnostic
  code
  path
  severity
  message
  human_explanation
  suggested_fix
```

规则：

```text
human_summary / human_explanation / suggested_fix 都是从结构化 rule / diagnostics / sourceMap 派生的解释。
它们不能成为第二真相层，不能反向覆盖 ProjectRuleAsset / Canonical IR。
如果解释和结构化诊断冲突，以结构化诊断为准，并输出 ExplanationMismatch diagnostic。
解释文本必须由确定性 formatter / template 生成，不能在 validate / build 阶段临时调用 LLM 自由生成。
```

结构化 patch operation：

```text
create_rule_asset
set_rule_trigger
add_statement
update_statement
remove_statement
add_operation
update_operation
remove_operation
validate_rule_asset
compile_rule_asset
save_rule_asset
```

这些操作只修改：

```text
ProjectRuleAsset
ProjectRuleAsset.validation cache
派生 report
```

不直接修改：

```text
RuntimeRuleManifest
Generated Rust source
RuleModuleRegistry
ProjectLogicRunner
```

### 9.1 第一版代码落点

根据 `25-193-Rule-Authoring-Productization方案审查.md` 的审查建议，v1 代码落点固定为：

```text
editor_ui_model
  -> RuleAuthoringDocument
  -> RuleAuthoringReport
  -> RuleAuthoringPatch
  -> RuleAuthoringDiagnostic / Command / Model

editor_core
  -> RuleAuthoringService
  -> Rule authoring session command handlers
  -> diagnostic code -> deterministic explanation template formatter

editor_ui_model/src/command.rs
  -> Rule UiCommandPayload

editor_ui_model/src/workflow_command.rs
  -> Rule domain command resolver

project_e2e_gate
  -> complex shooter rule-authoring-productization-report.json
```

边界：

```text
editor_ui_model 不依赖 engine_runtime。
engine_runtime 的 ProjectRuleAsset / Canonical Rule IR 仍是真相层。
editor_core 负责把 UI model / command / patch 翻译到 engine_runtime 类型。
project_e2e_gate 只生成验收报告，不引入玩法专用 API。
```

## 10. UI / Command 设计

新增 `UiCommandPayload`：

```text
CreateRuleAsset
OpenRuleAsset
SelectRuleAsset
SetRuleTrigger
AddRuleStatement
UpdateRuleStatement
RemoveRuleStatement
AddRuleOperation
UpdateRuleOperation
RemoveRuleOperation
ValidateRuleAsset
BuildRuleArtifact
SaveRuleAsset
OpenRuleDiagnostics
```

`focus_rule_panel` 继续存在，但不再是 Rule 域唯一结果。

默认 UI 规则：

```text
普通用户默认看到 human_summary、结构化表单和 human_explanation。
AI 默认读取结构化 command / patch / report，同时读取 human_summary 辅助理解。
Canonical IR / raw JSON 只作为 Advanced / Debug view，不作为 v1 主编辑入口。
```

Authoring workflow 中 Rules step 的主命令从：

```text
focus_rule_panel
```

升级为：

```text
create_rule_asset 或 open_rule_asset
```

根据上下文：

```text
无 rule asset -> create_rule_asset
有 rule asset -> open_rule_asset / validate_rule_asset
dirty -> save_rule_asset
diagnostics error -> open_rule_diagnostics
```

## 11. Validation / Build 规则

Rule validation 分三层：

```text
Asset validation：ProjectRuleAsset schema / assetId / ruleId / sourceMap。
IR validation：trigger / statement / operation / fieldPath / prefabRef。
Pipeline validation：compile report / artifact id / manifest lifecycle。
```

Build v1 不要求每次真实 cargo build player，但必须输出诚实 report：

```text
generated_rust_source: produced
static_registry_source: produced
artifact_lifecycle: validated
runtime_package_manifest: ready / blocked
cargo_build: skipped_by_v1 或 passed
```

如果某阶段是 skipped，report 必须写明：

```text
skip_reason
next_action
```

不能把 skipped 伪装成 passed。

### 11.1 Human-readable Explain 规则

Validation / Build report 必须同时服务 AI 和用户：

```text
AI 读取结构化 code / path / sourceMap / changed_paths。
用户读取 human_summary / human_explanation / suggested_fix。
```

解释生成规则：

```text
每个 RuleAuthoringReport 至少有一个 human_summary。
每个 error / warning diagnostic 必须有 human_explanation。
每个可修复 diagnostic 应尽量提供 suggested_fix。
human_explanation 必须包含规则名、失败步骤、失败原因和下一步检查点。
human_explanation 必须可测试、可快照比对、可从 diagnostic code / path / sourceMap 稳定复现。
```

示例：

```text
规则 Fire Projectile 中，“移动 projectile”步骤写入字段 local_position.x 失败。
字段路径不存在。请检查 Transform 组件里是否叫 localPosition 或 position。
```

### 11.2 Diagnostic Explain Template 规则

Gate D 必须显式实现 `diagnostic code -> template` 映射：

```text
InvalidFieldPath
  -> Rule {rule_name} has an invalid field path at {path}.
     Check the component field name and use a simple dot path.

MissingActionId
  -> Rule {rule_name} has an action trigger or condition without action_id.
     Set a stable Input action id before validation.

MissingPrefabRef
  -> Rule {rule_name} instantiates a prefab without prefab_ref.id.
     Select a prefab asset from the project library.
```

规则：

```text
template 的输入只能来自 diagnostic code / path / message / suggestion / rule name / sourceMap。
template 不能调用 LLM 临时生成文本。
未识别 code 必须走 deterministic fallback template。
template 输出必须进入测试，至少覆盖 InvalidFieldPath / MissingActionId / MissingPrefabRef / fallback。
```

## 12. 与复杂打飞机的最小验收

本系统验收不写打飞机专用 API，只使用项目侧数据：

```text
rule.fire_projectile:
  trigger = actionPressed("fire")
  operation = instantiatePrefab(asset.prefab.projectile)

rule.move_projectiles:
  trigger = always
  statement = forEachQuery(all=[Transform, project.ProjectileMotion])
  operation = writeComponentField("$entity", Transform.localPosition, ...)
```

验收报告必须证明：

```text
规则由 ProjectRuleAsset 创建。
规则可通过 RuleAuthoringService 修改。
IR hash 改变可被报告。
validation diagnostics 可回指 asset_id / rule_id / path。
compile report 能生成 source / registry evidence。
manual walkthrough Rule 域不再全是 focus/missing。
复杂样例项目仍不新增 Player / Enemy / Bullet 等引擎 API。
```

## 13. Gate 拆分

### Gate A：Rule Authoring 模型与服务

目标：

```text
新增 editor_core RuleAuthoringService。
新增 RuleAuthoringDocument / Report / Patch。
支持 create/open/select/save ProjectRuleAsset。
```

测试：

```text
cargo test -p editor_core rule_authoring
cargo test -p engine_runtime project_rule_asset
```

### Gate B：结构化 IR 编辑命令

目标：

```text
支持 set trigger。
支持 add/update/remove statement。
支持 add/update/remove operation。
支持 dirty / expected_ir_hash 冲突诊断。
```

测试：

```text
cargo test -p editor_core rule_authoring
cargo test -p engine_runtime rule_ir
```

### Gate C：Workflow / UiCommand 接入

目标：

```text
新增 Rule UiCommandPayload。
WorkflowCommandResolver 识别 Rule 域命令。
Rules step 根据上下文给 create/open/validate/save/build。
manual walkthrough Rule 域状态从 missing/focus 推进到 executable / needs context。
ManualWalkthroughCoverageReport 中 Rule 域不再全是 focus/missing，必须能证明 Create/Open/Edit/Validate/Build/Diagnostics 进入可执行命令或 needs context。
```

测试：

```text
cargo test -p editor_ui_model workflow_command
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core authoring_workflow
cargo test -p editor_core manual_walkthrough
```

### Gate D：Validate / Compile / Artifact Report

目标：

```text
RuleAuthoringService 调 ProjectRuleAsset.validate。
调用 RuleCompiler 生成 compile report。
生成 static registry source evidence。
接入 rule_artifact lifecycle validation。
实现 diagnostic code -> deterministic explanation template 映射。
测试 human_explanation / suggested_fix 不由 LLM 生成、不形成第二真相层。
```

测试：

```text
cargo test -p editor_core rule_authoring
cargo test -p engine_runtime rule_compiler
cargo test -p engine_runtime rule_artifact
```

### Gate E：Complex Shooter Rule Authoring Report

目标：

```text
project_e2e_gate 生成 rule-authoring-productization-report.json。
覆盖 samples/complex_shooter_project 的 Rule authoring 状态。
报告 next_actions，不伪装完整 graph editor。
```

测试：

```text
cargo test -p project_e2e_gate rule_authoring
cargo test -p project_e2e_gate manual_walkthrough
```

## 14. 方案自审

### AI 适配性

通过。

理由：

```text
所有编辑操作都是结构化 command / patch。
用户默认编辑对象是 Gameplay Rule Asset / RuleSlot。
ProjectRuleAsset / Canonical Rule IR 作为底层容器、内部规范语义和构建输入。
报告可回指 asset_id / rule_id / ir_hash / path。
AI 不需要手写 Rust。
AI 读取结构化数据，用户通过 human_summary / human_explanation 接手 AI 修不好的问题。
```

### 复杂项目适配与可维护

通过。

理由：

```text
不把打飞机玩法写进引擎。
规则可通过 sourceMap / diagnostics / expected_ir_hash 长期维护。
后续可以在同一 Gameplay Rule Asset 上增加 Rule Graph / DSL 视图。
复杂流程、复杂 UI 工作流和复杂算法不进入 IR。
```

### 效率

通过。

理由：

```text
第一版复用 186 / 187 底座。
不提前做完整节点图。
Build report 可以诚实区分 produced / skipped / passed。
```

### 风险

主要风险：

```text
如果 UI 只做 raw JSON，会退化成开发者工具。
如果第一版直接做完整节点图，会扩大施工范围。
如果第一版做完整 Rule Card / Readable DSL 多视图，会把 v1 产品面拉得过大。
如果 human explanation 手写漂移，会形成第二真相层。
如果 human explanation 由 LLM 临时生成，会导致诊断不可复现、不可测试。
如果 compile report 把 skipped 写成 passed，会污染验收。
```

治理：

```text
v1 主路径必须是结构化 RuleAuthoringService。
raw JSON 只做 debug view。
human_summary / human_explanation 必须由结构化规则和诊断派生。
human_summary / human_explanation 必须由确定性 formatter / template 生成。
v1 不做完整 Rule Card / DSL / 节点图。
report 必须区分 passed / failed / skipped_by_v1。
```

## 15. 推荐结论

正式采用：

```text
Rule Authoring Productization v1：方案 C-min+Explain
```

也就是：

```text
结构化 RuleAuthoringService + Rule Commands + Validation / Build Report + Human-readable Explain + Workflow Coverage
```

它是当前最适合推进复杂打飞机“从编辑器手动编辑到 Windows 可玩”的下一个系统，因为它把 191 暴露出的 Rule 域缺口，连接到 186 / 187 已经存在的规则资产和运行时管线。

## 16. 参考

外部官方资料：

```text
Unity Visual Scripting Graphs
https://docs.unity3d.com/Packages/com.unity.visualscripting%401.8/manual/vs-graph-types.html

Unreal Engine Blueprints Visual Scripting
https://dev.epicgames.com/documentation/unreal-engine/blueprints-visual-scripting-in-unreal-engine

Godot GDScript Reference
https://docs.godotengine.org/en/stable/tutorials/scripting/gdscript/gdscript_basics.html

Godot Signal
https://docs.godotengine.org/en/stable/classes/class_signal.html

Bevy Learn / API Docs
https://bevy.org/learn/
https://docs.rs/bevy/latest/bevy/
```

本项目资料：

```text
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
132-M2-Project-Rule-IR-Rust-AOT-Codegen-Incremental-Build-Runtime-Execute方案.md
186-Project-Rule-Asset-Pipeline-Runtime-Execution-C-min方案.md
187-Project-Rule-Artifact-Module-Lifecycle-B-min方案.md
191-Authoring-Walkthrough-Missing-Operations-Convergence-v1方案.md
阶段完成记录/2026-07-03-Project-Rule-Asset-Pipeline-Runtime-Execution-C-min/00-总览.md
阶段完成记录/2026-07-04-Authoring-Walkthrough-Missing-Operations-Convergence-v1/00-总览.md
```
