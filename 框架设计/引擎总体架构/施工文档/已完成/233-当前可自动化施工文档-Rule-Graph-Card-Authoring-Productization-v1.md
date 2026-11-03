# 233-当前可自动化施工文档-Rule Graph / Card Authoring Productization v1

> 状态：已完成归档。  
> 来源方案：`233-Rule-Graph-Card-Authoring-Productization-v1方案.md`。  
> 采用方案：`B-min: Editable Rule Cards + Generated Read-only Rule Graph Preview`。  
> 施工日期：2026-07-10。  
> 施工完成前，不开启其它施工文档。

## 1. 施工目标

把现有 Rule Authoring 从：

```text
RuleAuthoringService / raw document / structured edit command
```

推进到：

```text
Rule Cards 可编辑产品面
  -> RuleAuthoringService / RuleAuthoringEditCommand
  -> Gameplay Rule Asset / RuleSlot
  -> validate / build / report
  -> Generated read-only Rule Graph Preview
```

本轮目标是让复杂打飞机项目中的规则不再只靠 raw JSON 或裸 IR 理解，而是能通过卡片稳定编辑，通过只读图预览看清 `trigger -> statement -> operation` 的执行关系。

## 2. 范围边界

本轮必须做：

```text
RuleCardAuthoringModel / RuleCardModel / RuleCardFieldModel。
RuleGraphPreviewModel / node / edge / source mapping。
RuleAuthoringModel 派生 card_authoring 子模型，不替代现有模型。
Card edit command 降低到已有 RuleAuthoringEditCommand。
SelectRuleCard / SetRuleCardField / AddRuleCard / RemoveRuleCard / SelectRuleGraphNode。
RuleCardAuthoringReport summary / trace evidence。
ManualWalkthrough / Workflow / Report Panel 能看到 Rule Card / Graph Preview 能力。
project_e2e_gate 生成 complex-shooter-rule-card-authoring-productization-report.json。
分 Gate 测试和整体回归。
```

本轮不做：

```text
完整自由节点图编辑器。
自由拖线 / edge editing / port typing。
从 graph canvas 创建节点。
Graph compiler。
Blueprint / Lua / VisualScript 式通用脚本系统。
Runtime graph execution。
Rule debugger / breakpoint / step over。
复杂状态机 / 行为树 / 技能树编辑器。
把复杂 UI 工作流放进 Rule Graph。
把 Rust Project Module 可视化成通用图。
```

强约束：

```text
Gameplay Rule Asset / RuleSlot 仍是编辑真相。
Canonical Rule IR 仍是内部规范语义和构建输入。
Rule Graph Preview 只读、editor-only、可重新派生。
Runtime 执行仍走 RuntimePackage.rules -> RuleModuleRegistry -> ProjectLogicRunner。
默认不新增独立持久化 RuleCardPatch。
DuplicateRuleCard / MoveRuleCard 本轮 disabled，除非同步补明确 reorder/duplicate edit command 和测试。
```

## 3. 涉及文件

预计修改：

```text
rust/crates/editor_ui_model/src/rule_authoring.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_core/src/rule_authoring.rs
rust/crates/editor_core/src/services/rule_service.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/project_e2e_gate/src/rule_authoring.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
```

可能需要修改：

```text
rust/crates/editor_core/src/session.rs
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_ui_model/src/tests.rs
rust/crates/editor_core/src/tests/*
```

文档同步：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-10-Rule-Graph-Card-Authoring-Productization-v1/00-总览.md
```

## 4. Gate A：Rule Card View Model

目标：

```text
新增 RuleCardAuthoringModel / RuleCardModel / RuleCardFieldModel。
从现有 RuleAuthoringDocument / ProjectRuleAsset 派生 cards。
覆盖 trigger / statement / operation / diagnostics。
卡片 id / source_path / field_path 稳定可回溯。
RuleAuthoringModel 保留原有字段，并新增派生 card_authoring 子模型。
```

建议 card id：

```text
card:trigger
card:statement:{index}
card:operation:{index}
card:diagnostic:{index}
```

建议 source path：

```text
canonicalIr.trigger
canonicalIr.statements[{index}]
canonicalIr.operations[{index}]
```

测试：

```powershell
cd rust
cargo test -p editor_ui_model rule_card
cargo test -p editor_core rule_authoring
```

完成标准：

```text
RuleAuthoringModel 能包含卡片子模型。
现有 rule_authoring 测试不因新增字段破坏。
trigger / statement / operation 能生成确定性 card。
diagnostic 能映射到 source_path，能尽量回指 card_id。
```

## 5. Gate B：Card Command 到 RuleAuthoringService

目标：

```text
新增 SelectRuleCard / SetRuleCardField / AddRuleCard / RemoveRuleCard command。
所有规则资产修改都降低到现有 RuleAuthoringEditCommand。
SetRuleCardField 支持 expected_ir_hash / source_path / field_path。
AddRuleCard 只映射 AddStatement 或 AddOperation。
RemoveRuleCard 只映射 RemoveStatement 或 RemoveOperation。
DuplicateRuleCard / MoveRuleCard 保持 disabled。
```

测试：

```powershell
cd rust
cargo test -p editor_core rule_card
cargo test -p editor_core rule_authoring
```

完成标准：

```text
Card command 不直接写文件。
Card command 不绕过 RuleAuthoringService。
RuleAuthoringPatch 仍是 AI 默认 patch 格式。
若实现 editor-only adapter，它必须立即降低为 RuleAuthoringPatch / RuleAuthoringEditCommand，且不落盘。
duplicate / move 命令未启用时必须有 disabled reason。
```

## 6. Gate C：Generated Read-only Rule Graph Preview

目标：

```text
新增 RuleGraphPreviewModel / RuleGraphPreviewNode / RuleGraphPreviewEdge / group。
从 Rule Asset / RuleAuthoringDocument 派生 nodes / edges / groups。
建立 card_id / source_path / node_id 映射。
SelectRuleGraphNode 只改变 selection，不修改规则资产。
图预览支持 invalid rule 节点错误状态，不把 invalid 伪装成 valid。
```

边生成规则：

```text
trigger -> statements -> operations。
无 statement 时 trigger -> first operation。
多 statement / operation 按 canonical order 生成 edge。
```

测试：

```powershell
cd rust
cargo test -p editor_ui_model rule_graph_preview
cargo test -p editor_core rule_graph_preview
```

完成标准：

```text
Graph Preview 是只读派生模型。
Graph Preview 丢失后能由 Rule Asset 重新生成。
SelectRuleGraphNode 不改变 Rule Asset / RuleAuthoringPatch。
每个 graph node 尽量能回指 card_id 和 source_path。
```

## 7. Gate D：Workflow / Manual Walkthrough / Report Panel

目标：

```text
Rules step 显示 card authoring / graph preview capability。
ManualWalkthroughCoverageReport 暴露 Rule Card / Graph Preview 能力。
Report Panel 注册 RuleCardAuthoringReport summary provider。
Editor report 分档遵守 Off / Summary / Trace；默认 Summary，测试可 Trace。
Runtime 不输出 RuleCard / GraphPreview report。
```

测试：

```powershell
cd rust
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core manual_walkthrough
cargo test -p editor_core report_panel
```

完成标准：

```text
Workflow / ManualWalkthrough 不再把 Rule Card / Graph Preview 固定标为缺口。
Report Panel 能看到 card_count / graph_node_count / diagnostics / next_actions。
Runtime 热路径不新增常驻 card/graph report。
```

## 8. Gate E：Complex Shooter E2E Report

目标：

```text
project_e2e_gate 生成 complex-shooter-rule-card-authoring-productization-report.json。
覆盖 fire_bullet / linear_motion / lifetime_cleanup sample rules。
证明 card edit -> validate/build -> graph preview refresh。
证明 diagnostic -> source_path -> card_id -> node_id 可回溯。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate rule_authoring
cargo test -p project_e2e_gate rule_card_authoring
```

完成标准：

```text
至少 3 条复杂打飞机规则能生成 Rule Cards。
至少 1 条规则能通过 SetRuleCardField 或 AddRuleCard 修改并保存。
修改后 validate/build 通过，或输出结构化 diagnostic。
Graph Preview 由修改后的 Rule Asset 重新派生。
没有新增 Player / Enemy / Bullet 专用 engine core API。
```

## 9. Gate F：文档同步与归档

目标：

```text
生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
将本文档从 施工文档/当前/ 移动到 施工文档/已完成/。
227 中 P1-2 标记为已完成或在入口摘要中声明完成。
下一轮默认推进 227 中的 P1-3。
```

整体回归：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p project_e2e_gate
```

完成标准：

```text
当前/ 目录重新为空。
54 显示无当前可施工文档。
阶段完成记录包含施工范围、测试命令、测试结果、残留风险和下一步建议。
```

## 10. 施工文档自审

### 10.1 与正式方案一致性

结论：通过。

```text
本文档严格采用 233 正式方案 B-min。
卡片是可编辑主入口。
图是由 Gameplay Rule Asset / RuleSlot 派生的只读预览。
不新增 Runtime graph execution。
不新增 Blueprint / Lua / VisualScript 式通用脚本系统。
```

### 10.2 外部审查文档状态

结论：本轮未提供 233 外部审查文档。

```text
没有读取到用户指定的 233 其它 AI 审查文档。
因此本轮按 233 正式方案自审进入施工。
若后续用户提供 233 外部审查文档，必须先暂停扩大实现，按外部审查流程判断是否回填方案或施工文档。
```

### 10.3 范围控制

结论：通过。

```text
没有把 Graph Preview 扩成自由图编辑器。
没有要求实现 edge editing / node creation / graph compiler。
没有把复杂 gameplay / UI 工作流放进 Rule Graph。
DuplicateRuleCard / MoveRuleCard 默认 disabled。
```

### 10.4 测试覆盖

结论：通过。

```text
每个 Gate 都有明确 cargo test 命令。
Gate A-C 覆盖模型、命令和图预览。
Gate D 覆盖 workflow / manual walkthrough / report panel。
Gate E 覆盖复杂打飞机 e2e artifact。
Gate F 覆盖 fmt 和 editor_ui_model / editor_core / project_e2e_gate 整体回归。
```

### 10.5 架构红线

结论：通过。

```text
Gameplay Rule Asset / RuleSlot 仍是编辑真相。
RuleCardAuthoringModel 是 RuleAuthoringModel 的派生子模型，不替代主模型。
RuleAuthoringService 仍是 create / open / save / validate / build / apply 的唯一服务入口。
RuleAuthoringPatch 仍是 AI 默认 patch 格式。
RuntimePackage.rules -> RuleModuleRegistry -> ProjectLogicRunner 链路不变。
```

## 11. 施工顺序

严格按以下顺序执行：

```text
Gate A Rule Card View Model
-> Gate B Card Command lowering to RuleAuthoringService
-> Gate C Generated Read-only Rule Graph Preview
-> Gate D Workflow / Manual Walkthrough / Report Panel
-> Gate E Complex Shooter E2E Report
-> Gate F 文档同步与归档
```

每个 Gate 测试失败时，先修当前 Gate，不跨 Gate 堆积问题。
