# 192-当前可自动化施工文档-Authoring Walkthrough Missing Operations Convergence v1

## 1. 施工依据

```text
191-Authoring-Walkthrough-Missing-Operations-Convergence-v1方案.md
其它AI审查目录/16-191-Authoring-Walkthrough-Missing-Operations-Convergence方案审查.md
```

16 号审查结论：

```text
191 方案定位正确，可直接进入施工文档。
需吸收低风险建议：
  明确代码落点。
  区分能识别 command 但缺少参数上下文的操作。
```

已回填 191：

```text
ManualAuthoringOperationStatus 增加 ExecutableCommandNeedsContext。
新增第一版代码落点：editor_ui_model / editor_core / project_e2e_gate。
Workflow 不补参数规则增加空 path / 空 selection / 空 binding 禁止伪装。
```

## 2. 施工目标

实现：

```text
Authoring Walkthrough Missing Operations Convergence v1 / C-min
```

目标链路：

```text
ManualAuthoringOperationRequirement
  -> WorkflowCommandResolver / UiCommandPayload 对齐
  -> ManualWalkthroughCoverageAnalyzer
  -> ManualWalkthroughCoverageReport
  -> complex shooter coverage artifact
  -> AuthoringAiContext / WorkspaceContext summary
  -> next construction entry
```

本阶段只做缺失操作覆盖矩阵和结构化 report，不补完整 domain editor。

## 3. 非目标

本阶段不做：

```text
新的 Walkthrough / Operation 执行层
新的 DomainOperationCatalog 真相层
新的 Wizard / 一键复杂项目向导
完整 Prefab Editor
完整 Rule Editor
209 Scene Unified AUI Authoring
M16 AI Project Patch Entry
真实 glyph present
```

禁止新增：

```text
Player / Enemy / Bullet / Health / Damage / Score / Wave / Weapon / Boss / Drop
```

禁止为了 report pass 而：

```text
伪造 UiCommandPayload 参数
把空 path / 空 selection / 空 binding 当成 ExecutableCommand
让 coverage analyzer 越过 domain summary 直接解析 Scene / Prefab / AUI / Rule 原始文件
```

## 4. 涉及文件

预计修改：

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_model/src/authoring_workflow.rs
rust/crates/editor_ui_model/src/workflow_command.rs
rust/crates/editor_ui_model/src/tests.rs

rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/authoring_workflow.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/tests/authoring_workflow_tests.rs
rust/crates/editor_core/src/tests/mod.rs

rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/report.rs
rust/crates/project_e2e_gate/src/vertical_slice.rs
rust/crates/project_e2e_gate/src/tests.rs

框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

按实际代码依赖可少改或新增小文件，例如：

```text
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/project_e2e_gate/src/manual_walkthrough.rs
```

但不得改变本施工目标。

## 5. Gate A：Operation Requirement Matrix

### 5.1 目标

在 `editor_ui_model` 中新增手动 walkthrough 覆盖数据模型和固定操作清单。

新增或等价实现：

```text
ManualAuthoringOperationRequirement
ManualAuthoringOperationStatus
ManualWalkthroughOperationCoverage
ManualWalkthroughCoverageReport
ManualWalkthroughCoverageStatus
ManualWalkthroughDomainSummary
ManualWalkthroughCoverageSummary
MissingOperationGap
MissingOperationSeverity
ManualWalkthroughRequiredContext
manual_authoring_operation_requirements()
```

状态必须包含：

```text
ExecutableCommand
ExecutableCommandNeedsContext
FocusDomainPanel
MissingCommand
MissingDomainService
BlockedByDependency
Deferred
```

### 5.2 规则

操作清单覆盖 191 第 9 节：

```text
Project
Assets
Scene
Prefab
Rules
Input
AUI
Play / Build / Reports
```

每个 operation 必须有：

```text
operation_id
domain
title
user_goal
required_for_play
required_for_build
required_for_complex_project
expected_command_id
expected_payload_kind
required_context
fallback_behavior
```

操作名必须是通用 domain operation，不出现玩法名。

### 5.3 Gate A 测试

运行：

```powershell
cd rust
cargo test -p editor_ui_model manual_walkthrough
```

必须证明：

```text
操作清单非空并覆盖 8 个 domain。
操作清单不包含打飞机专用 API 名称。
每个 operation_id 唯一。
每个复杂项目必需操作有 required_context 和 fallback_behavior。
状态枚举包含 ExecutableCommandNeedsContext。
```

## 6. Gate B：Coverage Analyzer

### 6.1 目标

在 `editor_core` 中新增 coverage analyzer，把 operation requirements 与当前 `WorkflowCommandResolver` / `UiCommandPayload` / workspace summary 对齐。

新增或等价实现：

```text
ManualWalkthroughCoverageAnalyzer
ManualWalkthroughCoverageInput
analyze_manual_walkthrough_coverage(...)
```

输入来源只允许：

```text
ProjectAuthoringWorkspaceModel
AuthoringWorkflowModel
WorkflowCommandResolver
Console / Report summary
```

### 6.2 分类规则

```text
ExecutableCommand:
  resolver 能返回安全 UiCommandPayload，且参数上下文完整。

ExecutableCommandNeedsContext:
  resolver 能识别 command / payload kind，但 payload 中存在空 path、空 id、空 selection 或缺 required_context。

FocusDomainPanel:
  resolver 明确返回 FocusDomainPanel，或者 operation 设计上必须交给 domain panel 填参数。

MissingCommand:
  domain summary 存在，但没有正式 UiCommandPayload / AuthoringCommand 入口。

MissingDomainService:
  domain summary 或底层服务不存在。

BlockedByDependency:
  被其它已知系统阻塞，例如 runtime_text_glyph_present、asset cook、rule runtime。

Deferred:
  v1 不做，必须写原因。
```

Analyzer 不能为了提高通过率而改变 resolver 行为。

### 6.3 Gate B 测试

运行：

```powershell
cd rust
cargo test -p editor_core manual_walkthrough
```

必须证明：

```text
play / export / clear_console 等安全命令识别为 ExecutableCommand。
open_scene_document 空 path 不识别为完整 ExecutableCommand。
create_scene_entity / focus_prefab_panel / focus_rule_panel / focus_aui_panel 识别为 FocusDomainPanel 或 NeedsContext。
Prefab / Rule / AUI 缺真实编辑命令时输出 MissingCommand 或 MissingDomainService。
report status 在存在缺口时为 partial，而不是伪装 pass。
next_actions 能按 blocking gap 排序。
```

## 7. Gate C：Complex Shooter Coverage Artifact

### 7.1 目标

在 `project_e2e_gate` 中为 `samples/complex_shooter_project` 生成 manual walkthrough coverage artifact，并写入现有真实项目纵切报告或新增旁路 report。

产物示例：

```text
target/project-e2e/complex-shooter/manual-walkthrough-coverage-report.json
```

最终路径以现有 artifact 目录为准，但必须在测试中可验证。

### 7.2 规则

复杂项目 report 必须诚实输出：

```text
status = partial
Prefab / Rule / AUI 缺口存在
next_actions 非空
blocks_manual_walkthrough 的 gap 可读
```

不得因为 complex shooter 项目侧已有样例文件就认为用户手动编辑能力已完成。

### 7.3 Gate C 测试

运行：

```powershell
cd rust
cargo test -p project_e2e_gate manual_walkthrough
```

必须证明：

```text
真实 sample project 能生成 coverage report。
report 包含 operation_count / domain_summaries / blocking_gaps / next_actions。
Prefab / Rule / AUI 至少一个 domain 处于 missing 或 focus 状态。
report 不包含 Player / Enemy / Bullet 等玩法 API 作为引擎操作。
```

## 8. Gate D：Workflow / AI Context 接入

### 8.1 目标

让 `AuthoringAiContext` 或 `WorkspaceContext` 暴露 manual walkthrough coverage summary，使用户和 AI 能看到下一轮真实 authoring 缺口。

新增字段或等价摘要：

```text
manual_walkthrough_coverage: Option<ManualWalkthroughCoverageSummary>
```

摘要只保留轻量信息：

```text
status
operation_count
executable_count
needs_context_count
focus_panel_count
missing_command_count
missing_service_count
blocked_count
top_next_actions
```

### 8.2 规则

AI context 只读 coverage summary，不持有完整大 report。

如果当前 UI model 没有打开项目，summary 可以为空或 fail，但必须有诊断。

### 8.3 Gate D 测试

运行：

```powershell
cd rust
cargo test -p editor_core authoring_workflow
cargo test -p editor_ui_model workflow_command
```

必须证明：

```text
AuthoringWorkflow / AI context 能看到 coverage summary。
summary 中 next_actions 与 coverage report 一致。
现有 workflow_command 测试不退化。
```

## 9. Gate E：文档和入口同步

完成实现后更新：

```text
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
施工文档/README.md
阶段完成记录/README.md
```

新增阶段完成记录：

```text
阶段完成记录/2026-07-04-Authoring-Walkthrough-Missing-Operations-Convergence-v1/00-总览.md
```

归档本施工文档到：

```text
施工文档/已完成/192-当前可自动化施工文档-Authoring-Walkthrough-Missing-Operations-Convergence-v1.md
```

## 10. 最终整体测试

全部 Gate 完成后运行：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core manual_walkthrough
cargo test -p project_e2e_gate manual_walkthrough
cargo test -p editor_core authoring_workflow
cargo test -p editor_ui_model workflow_command
cargo test -p project_e2e_gate
```

如某个 filter 没有匹配测试，必须改用对应 crate 的实际 test 名或整 crate 测试，并在阶段记录说明。

## 11. 施工文档自审

### 11.1 是否从正式方案生成

通过。

```text
施工依据是 191 正式方案，并已吸收 16 号审查建议。
```

### 11.2 是否范围可控

通过。

```text
本施工只做 operation matrix / coverage analyzer / report / summary。
不补完整 Prefab / Rule / AUI 编辑器，不新增 Wizard，不进入 M16。
```

### 11.3 是否合乎架构边界

通过。

```text
不新增第二套 Walkthrough / Operation 执行层。
不把复杂打飞机玩法写入引擎 Core。
不绕过 WorkflowCommandResolver / UiCommandPayload / EditorSession 纪律。
Coverage 只读 summary，不越域解析原始 domain 文件。
```

### 11.4 是否可验证

通过。

```text
Gate A-E 都有测试命令。
施工期间必须一边施工一边跑 Gate 测试。
最终整体测试通过后才算完成。
```

### 11.5 主要风险和处理

风险：

```text
Analyzer 变成新的业务层。
为了 pass 伪造 command 参数。
coverage report 过大进入 AI context。
```

处理：

```text
Analyzer 只分类，不执行业务。
参数不足必须标记 ExecutableCommandNeedsContext 或 FocusDomainPanel。
AI context 只暴露 summary。
```

## 12. 完成标准

```text
Manual authoring operation matrix 落地。
Coverage analyzer 能生成 ManualWalkthroughCoverageReport。
samples/complex_shooter_project 能生成 manual walkthrough coverage artifact。
Report 诚实输出 partial 和 next_actions。
Authoring AI context 能读取 coverage summary。
每个 Gate 测试和最终整体测试通过。
阶段完成记录生成。
施工文档归档。
入口文档同步。
```
