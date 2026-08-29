# 212-Report Panel / Evidence Panel Productization v1 方案

状态：方案已确认，等待施工文档。

定案：方案 B-reg-min，Unified Report Registry + Single Report Panel。

## 1. 这个系统是干什么的

Report Panel / Evidence Panel 是编辑器里唯一的报告与证据入口。

它要解决的问题不是“多显示一些日志”，而是让所有系统产出的 report 都进入同一个面板：

```text
Build / Runtime / AUI / Rule / Prefab / ProjectPatch / Asset / Manual Walkthrough / E2E Gate
  -> ReportProvider 注册
  -> UnifiedReportEntry / EvidenceEntry
  -> Single Report Panel 展示
  -> AI Context / Patch Repair 使用
```

后续新增 report 时，不再新增一个专用面板，也不直接改 Report Panel UI；只新增一个 `ReportProvider` / adapter 注册进去。

## 2. 其它引擎对标

### Unity

Unity 的 `ConsoleWindow` 负责日志、错误、警告、过滤、搜索、折叠和点击定位；`BuildReportInspector` 则把 BuildReport 拆成多个可浏览页签。

可学习点：

```text
ConsoleWindow:
  日志/错误/警告统一列表
  支持过滤、搜索、折叠、定位源文件

BuildReportInspector:
  单个复杂报告用结构化 view 展开
  Build steps / assets / output files / stripping 等都不是裸 JSON
```

不可照搬点：

```text
Unity Console 更偏日志流，不足以承载 AI-first evidence。
BuildReportInspector 偏单 report 工具，不解决多 domain report 统一注册。
```

参考：

```text
https://docs.unity3d.com/Manual/Console.html
https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/ConsoleWindow.cs
https://github.com/Unity-Technologies/BuildReportInspector
```

### Unreal

Unreal 的 `MessageLog` 更接近注册式方向。不同模块可以注册不同 listing，再统一进入 Message Log UI。

可学习点：

```text
FMessageLogModule::RegisterLogListing
  模块注册自己的 message listing
  UI 不需要知道每个业务模块内部结构
```

不可照搬点：

```text
Unreal MessageLog 仍主要是 message/listing，不是 schema-first report/evidence registry。
我们需要保留原始强类型 report，并投影成 AI 可读 Evidence。
```

参考：

```text
https://dev.epicgames.com/documentation/unreal-engine/API/Developer/MessageLog/FMessageLogModule
https://dev.epicgames.com/documentation/unreal-engine/API/Developer/MessageLog/FMessageLogModule/RegisterLogListing
```

### Godot

Godot 的 Output Panel / EditorLog 是底部统一输出入口，支持 Log / Error / Warning / Editor 分类、过滤、折叠、跳转。

可学习点：

```text
一个统一输出入口
按类型过滤
重复消息折叠
能从消息跳到文件或外部链接
```

不可照搬点：

```text
Godot EditorLog 仍是日志面板，不是 report registry。
我们的复杂项目需要 report_path / artifacts / next_actions / ai_context。
```

参考：

```text
https://docs.godotengine.org/en/stable/tutorials/scripting/debug/output_panel.html
https://github.com/godotengine/godot/blob/master/editor/editor_log.cpp
```

## 3. 当前项目基线

当前项目已经有大量 report / diagnostics / evidence，但入口分散：

```text
EditorDiagnostic:
  severity / code / message / source / command_id / request_id / path / entity_id / trace_entry_id / suggested_action

WorkspaceReportSummary:
  project_status / dirty_domains / diagnostics / last_command / last_transaction / build_status / play_status

BuildExportReportSummary:
  status / profile / target / package_dir / report_path / runtime_package_dir / diagnostic_count

project_e2e_gate:
  complex-shooter-*.json
  diagnostics / next_actions / artifacts

ManualWalkthroughCoverageReport:
  operation coverage / gaps / next_actions

ProjectPatchProductizationReport:
  validation / apply_report / next_actions
```

现状问题：

```text
1. Report domain 现在主要是 diagnostics 数量，不是可浏览 report 面板。
2. BuildReportPanel 旧方案只覆盖 build，不适合继续扩展。
3. AUI / Rule / Prefab / ProjectPatch / E2E 的 report 已经很多，但用户和 AI 没有统一入口。
4. 新 report 如果各自加面板，会让编辑器 UI 和用户心智越来越碎。
```

## 4. 核心原则

### 4.1 单一面板

编辑器中只允许一个正式 Report Panel。

```text
BuildReportPanel
AssetReportPanel
AuiReportPanel
RuleReportPanel
ProjectPatchReportPanel
```

这些都不再作为独立方向扩展。它们只能作为 `ReportProvider` 注册到统一 Report Panel。

### 4.2 注册式接入

新增 report 的标准动作：

```text
1. 保留自己的原始强类型 report。
2. 新增 XxxReportProvider。
3. provider 把原始 report 映射成 UnifiedReportEntry / EvidenceEntry。
4. 注册到 ReportRegistry。
5. Report Panel 自动出现该 report。
```

Report Panel UI 不直接依赖业务 report 类型。

### 4.3 原始 report 仍是真相

统一面板展示的是 projection，不是替代原始 report。

```text
Native Report = 真相
UnifiedReportEntry = UI/AI 摘要
EvidenceEntry = 可定位、可解释、可修复的证据项
```

因此不要求所有系统把原始 report 改成同一个 schema。

### 4.4 Editor-only

ReportRegistry / ReportPanelModel 只属于 Editor。

Runtime、RuntimePackage、Renderer 不依赖 ReportRegistry。

### 4.5 不新增重型 Report DB

v1 不做长期历史数据库。

可以读：

```text
EditorSession 内存中的 last_*_report
当前 project / build output 中已知 artifact path
project_e2e_gate 已知输出 report
```

不做：

```text
全盘扫描所有 JSON
长期趋势分析
跨构建 diff 数据库
后台 report indexer
```

## 5. B-reg-min 架构

### 5.1 数据流

```text
Native typed reports
  -> ReportProvider
  -> ReportRegistry
  -> ReportCollectionService
  -> ReportPanelModel
  -> editor_ui_renderer draw commands
  -> AI Context / ProjectPatch Repair Loop later
```

### 5.2 ReportRegistry

职责：

```text
注册 provider
列出 provider descriptor
按 project/editor context 收集 report
保证 provider_id 唯一
保证 domain / kind / capability 可查询
```

第一版可以是静态注册，不做插件热加载。

### 5.3 ReportProvider

建议接口：

```text
ReportProvider
  descriptor() -> ReportDescriptor
  collect(context) -> Vec<UnifiedReportEntry>
```

provider 只做只读适配：

```text
允许：
  读取 EditorSession last report
  读取已知 report artifact path
  摘要 diagnostics / next_actions / artifacts

禁止：
  执行业务修复
  修改项目文件
  直接调用 Build / Play / Patch apply
  把业务逻辑塞进 ReportPanel
```

### 5.4 ReportDescriptor

```text
ReportDescriptor
  provider_id
  label
  domain
  kind
  source_kind
  supported_schema_versions[]
  capabilities[]
```

capabilities 第一版：

```text
open_raw_report
reveal_path
copy_ai_context
open_related_artifact
filter_by_severity
```

预留但 v1 不实现：

```text
create_patch_from_evidence
run_repair_loop
compare_report_history
```

### 5.5 UnifiedReportEntry

```text
UnifiedReportEntry
  report_id
  provider_id
  title
  domain
  kind
  status
  max_severity
  source_kind
  source_path
  report_path
  schema_version
  summary
  updated_at_label
  evidence_count
  diagnostic_count
  next_action_count
  artifact_count
  evidence[]
  diagnostics[]
  next_actions[]
  artifacts[]
  ai_context
```

`updated_at_label` 第一版可以是稳定字符串，不要求真实时间服务。

### 5.6 EvidenceEntry

```text
EvidenceEntry
  evidence_id
  title
  severity
  code
  message
  domain
  stage
  source_path
  entity_id
  node_id
  command_id
  request_id
  trace_entry_id
  suggested_action
  next_actions[]
  related_artifacts[]
  raw_payload_summary
```

Evidence 必须偏“可修复”而不是普通日志。

例如：

```text
规则 Fire Projectile 中，字段 local_position.x 写入失败。
字段路径不存在。请检查 Transform 组件里是否叫 localPosition 或 position。
source_path = assets/rules/fire_projectile.rule.json
next_actions = ["open_rule_authoring", "inspect_component_schema"]
```

## 6. 第一批 Provider

B-reg-min 必须先接入最有价值的一批 report：

```text
BuildExportReportProvider
  来源：last_desktop_export_report / desktop-export-report.json

PlayRuntimeReportProvider
  来源：last_play_session_report / runtime report

ManualWalkthroughCoverageReportProvider
  来源：ManualWalkthroughCoverageAnalyzer 输出

RuleAuthoringReportProvider
  来源：RuleAuthoringModel / RuleAuthoringReport

PrefabAuthoringReportProvider
  来源：PrefabAuthoringModel / PrefabAuthoringReport

AuiAuthoringReportProvider
  来源：AUI authoring / preview / template / scene unified authoring report

ProjectPatchReportProvider
  来源：ProjectPatchProductizationReport / ImportProductizationReport / LLM patch source report

ComplexShooterE2eReportProvider
  来源：project_e2e_gate 已知 complex-shooter-*.json artifact
```

不要求第一版解析每个 report 的全部字段。第一版要确保：

```text
能进同一个列表
能按 domain / severity / status 过滤
能看到 summary
能看到 diagnostics
能看到 next_actions
能看到 report_path / artifacts
能复制 AI context
```

## 7. Report Panel UI 行为

### 7.1 布局

```text
左侧：
  domain filter
  severity filter
  status filter

中间：
  report list
  每行显示 domain / status / max_severity / title / evidence_count / next_action_count

右侧：
  selected report detail
  summary
  evidence list
  diagnostics
  next_actions
  artifacts
  raw report path
  AI context
```

### 7.2 操作

第一版支持：

```text
SelectReport
FilterReports
OpenRawReport
RevealReportPath
CopyReportAiContext
OpenRelatedArtifact
RefreshReports
```

第一版不自动修复。修复入口只作为 evidence capability 预留。

## 8. AI 适配规则

每个 provider 必须输出 `ai_context` 摘要，最低包含：

```text
report_id
provider_id
domain
status
max_severity
top_diagnostics[]
next_actions[]
source_paths[]
artifact_paths[]
suggested_patch_scope
```

AI 只能基于 `ai_context` 和原始 report path 生成 ProjectPatch，不能从 UI 文本猜测项目真相。

## 9. 和 ProjectPatch / Repair Loop 的关系

Report Panel 不负责修复。

关系是：

```text
Report Panel:
  发现问题、组织证据、提供 AI context

ProjectPatch:
  生成结构化修改
  validate / preview / apply / rollback

Repair Loop:
  后续读取 selected Evidence / AI Context
  调用 provider
  生成 ProjectPatch
```

因此 v1 不实现真实 provider repair loop，只为后续 repair loop 准备稳定入口。

## 10. 和 Console 的关系

Console 仍然可以存在，但职责不同：

```text
Console:
  时间流日志
  命令反馈
  简短消息

Report Panel:
  结构化 report
  diagnostics / evidence / next_actions
  artifacts / raw report path
  AI repair context
```

Console 可以显示“有新 report 产生”，但不能替代 Report Panel。

## 11. 和旧 BuildReportPanel 的关系

旧 `BuildReportPanel` 方向收敛为：

```text
BuildExportReportProvider
  -> Unified Report Panel
```

不再新建独立 BuildReportPanel。

历史文档中 `BuildReportPanel 只读展示 latestBuildExport.buildReportSummary` 的描述只作为历史参考，不作为新施工方向。

## 12. 方案对比

### 方案 A：Console 扩展

优点：

```text
施工最小
复用现有 ConsoleModel / EditorDiagnostic
```

缺点：

```text
仍是日志流
不能统一多 report
新增 report 仍需要额外 UI 约定
AI 修复上下文不足
```

结论：不推荐。

### 方案 B-reg-min：Unified Report Registry + Single Report Panel

优点：

```text
所有 report 一个面板
新增 report 只注册 provider
保留原始强类型 report
AI context 稳定
不增加 runtime 复杂度
不需要重型数据库
```

缺点：

```text
需要定义统一 projection schema
需要为第一批 report 写 adapter
```

结论：推荐并定案。

### 方案 C：Persistent Report Store / Evidence DB

优点：

```text
长期历史、趋势、diff、团队协作更强
```

缺点：

```text
当前过重
会新增持久化索引层
容易让系统继续膨胀
```

结论：后续阶段再做，不进入 v1。

## 13. 施工边界

v1 施工应做：

```text
新增 ReportRegistry / ReportProvider / ReportDescriptor。
新增 UnifiedReportEntry / EvidenceEntry / ReportPanelModel。
EditorSession / UiModelComposer 能生成 ReportPanelModel。
Workspace Report domain 展示来自 ReportPanelModel 的统计。
接入第一批 provider。
Manual Walkthrough Report domain 从 missing/partial 推进。
project_e2e_gate 新增 unified-report-panel-productization-report.json。
```

v1 施工不做：

```text
不做长期 Report DB。
不做后台文件扫描。
不做真实 LLM repair loop。
不做自动修复。
不做每个 report 的完整图形化专用详情页。
不做 runtime 依赖 ReportRegistry。
```

## 14. 验收标准

必须能证明：

```text
1. 所有第一批 report 进入同一个 ReportPanelModel。
2. 新增 provider 不需要改 Report Panel UI schema。
3. Build / Runtime / AUI / Rule / Prefab / ProjectPatch / ManualWalkthrough / E2E 至少各有一个 report entry 或诚实 empty/unsupported evidence。
4. Report domain summary 来自统一 ReportPanelModel。
5. selected report 能展示 diagnostics / next_actions / artifacts / raw report path / AI context。
6. project_e2e_gate 输出 unified report panel productization artifact。
7. 不引入 runtime 依赖，不改变 RuntimePackage。
```

建议测试：

```text
cargo fmt --check
cargo test -p editor_ui_model report_panel
cargo test -p editor_core report_panel
cargo test -p editor_core manual_walkthrough
cargo test -p project_e2e_gate unified_report_panel
cargo test -p project_e2e_gate
```

## 15. 最终结论

本系统采用 B-reg-min：

```text
Unified Report Registry + Single Report Panel
```

它解决的是复杂项目长期维护和 AI 修复入口的问题。

以后所有 report 的接入规则统一为：

```text
新 report = 原始强类型 report + ReportProvider 注册
```

不再为每个 report 新增独立面板，也不把 Report Panel 做成只会显示日志的 Console。
