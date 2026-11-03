# 212-当前可自动化施工文档-Report Panel / Evidence Panel Productization v1

状态：当前可施工。

依据方案：

```text
框架设计/引擎总体架构/212-Report-Panel-Evidence-Panel-Productization-v1方案.md
```

定案方案：

```text
B-reg-min：Unified Report Registry + Single Report Panel
```

## 1. 施工目标

把编辑器中的报告入口从分散摘要升级为统一注册式 Report Panel：

```text
Native typed reports
  -> ReportProvider
  -> ReportRegistry
  -> UnifiedReportEntry / EvidenceEntry
  -> ReportPanelModel
  -> Workspace Report domain summary / AI context / project_e2e_gate evidence
```

新增 report 后只能通过 provider 注册进入统一面板，不新增独立 report panel。

## 2. 涉及文件

预计新增：

```text
rust/crates/editor_ui_model/src/report_panel.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/project_e2e_gate/src/unified_report_panel.rs
阶段完成记录/2026-07-06-Report-Panel-Evidence-Panel-Productization-v1/00-总览.md
```

预计修改：

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_model/src/model.rs
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/tests.rs

rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/tests/authoring_workflow_tests.rs

rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs

框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

如果实现中发现无需修改某个预计文件，以实际 diff 为准；不得扩大到 runtime / RuntimePackage / renderer 内部。

## 3. Gate A：UI Model Schema

任务：

```text
新增 ReportPanelModel / ReportRegistrySummary / ReportDescriptor。
新增 UnifiedReportEntry / EvidenceEntry / ReportArtifactRef / ReportAiContext。
新增 filter / selected_report_id / summary counts。
把 ReportPanelModel 接入 EditorUiModel。
新增 report panel 相关 UiCommandPayload：SelectReportEntry / RefreshReports / CopyReportAiContext / OpenRawReport / RevealReportPath / OpenRelatedReportArtifact。
更新 workspace domain mapping。
```

测试：

```text
cargo test -p editor_ui_model report_panel
```

通过标准：

```text
ReportPanelModel 可序列化。
至少能表达 provider descriptor、report entry、evidence、next_actions、artifacts、ai_context。
新增 report 命令属于 WorkspaceDomainKind::Report。
```

## 4. Gate B：Editor Report Registry / Providers

任务：

```text
新增 editor_core::report_panel。
定义 ReportProvider / ReportProviderContext / ReportRegistry。
实现静态 provider 注册，不做插件热加载。
接入第一批 provider：
  BuildExportReportProvider
  PlayRuntimeReportProvider
  ManualWalkthroughCoverageReportProvider
  RuleAuthoringReportProvider
  PrefabAuthoringReportProvider
  AuiAuthoringReportProvider
  ProjectPatchReportProvider
  DiagnosticsReportProvider
  ComplexShooterE2eReportProvider 或诚实 placeholder provider
```

测试：

```text
cargo test -p editor_core report_panel
```

通过标准：

```text
ReportRegistry 能列出 provider descriptor。
EditorSession 能构建 ReportPanelModel。
每个第一批 provider 至少生成真实 entry 或诚实 empty/unsupported entry。
provider 只读，不修改项目文件，不触发 build/play/patch apply。
```

## 5. Gate C：UiModelComposer / Workspace / Manual Walkthrough 接入

任务：

```text
EditorSession::build_ui_model 填充 report_panel。
Workspace Report domain summary 来自 ReportPanelModel，而不是只来自 EditorDiagnostic 数量。
Manual Walkthrough 的 Report domain 不再把 unified_report_panel_productization 作为 missing。
保留 Console 职责，不把 Console 扩成 Report Panel。
```

测试：

```text
cargo test -p editor_core manual_walkthrough
cargo test -p editor_core authoring_workflow
cargo test -p editor_ui_model manual_walkthrough
```

通过标准：

```text
Report domain 能看到 report_count / evidence_count / next_action_count。
ManualWalkthroughCoverageReport 对 Report domain 给出可执行/needs_context，而非缺失统一 Report Panel。
```

## 6. Gate D：Project E2E Gate

任务：

```text
新增 unified-report-panel productization e2e report。
打开 complex shooter sample，通过 EditorSession 构建 ReportPanelModel。
输出 complex-shooter-unified-report-panel-productization-report.json。
报告中记录 provider_count / report_count / evidence_count / ai_context_count / domain coverage。
```

测试：

```text
cargo test -p project_e2e_gate unified_report_panel
```

通过标准：

```text
报告 artifact 存在。
至少覆盖 Build / Play / AUI / Rule / Prefab / ProjectPatch / ManualWalkthrough / Diagnostics / E2E placeholder。
没有要求真实 repair loop。
```

## 7. Gate E：整体回归

测试：

```text
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p project_e2e_gate
```

通过标准：

```text
所有测试通过。
没有 runtime / RuntimePackage / renderer 依赖 ReportRegistry。
```

## 8. 文档同步和归档

施工完成后：

```text
生成阶段完成记录：
  阶段完成记录/2026-07-06-Report-Panel-Evidence-Panel-Productization-v1/00-总览.md

更新入口：
  49-当前状态与下一步入口.md
  54-当前可自动化施工总入口.md
  施工文档/README.md
  阶段完成记录/README.md

移动施工文档：
  施工文档/当前/212-当前可自动化施工文档-Report-Panel-Evidence-Panel-Productization-v1.md
  -> 施工文档/已完成/212-当前可自动化施工文档-Report-Panel-Evidence-Panel-Productization-v1.md
```

## 9. 施工自审

本施工文档符合 212 方案：

```text
是统一注册式 Report Panel，不是 Console 扩展。
新增 report 的接入方式是 provider 注册。
保留原始强类型 report，不要求改成统一原始 JSON。
ReportRegistry 只属于 editor，不进入 runtime。
不做长期 Report DB、后台扫描、真实 repair loop、自动修复。
每个 Gate 都有测试命令。
当前施工目录原本为空，本文件是唯一当前施工文档。
```
