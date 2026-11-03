# 180-当前可自动化施工文档-Complex Shooter Real Authoring-to-Playable Vertical Slice v1

## 1. 施工目标

依据：

```text
179-Complex-Shooter-Real-Authoring-to-Playable-Vertical-Slice-v1方案.md
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
```

实现复杂打飞机真实项目级纵切验收：

```text
EditorSession open sample project
  -> EditorUiModel / ProjectAuthoringWorkspaceModel / AuthoringWorkflowModel
  -> Workspace readiness report
  -> run existing ComplexProjectE2eGate
  -> AuthoringToPlayableVerticalSliceReport
```

本阶段不新增项目玩法 API，不重写 export/player，不做真实 OS window mandatory gate。

## 2. 涉及文件

预计新增：

```text
rust/crates/project_e2e_gate/src/vertical_slice.rs
```

预计修改：

```text
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

预计新增完成记录：

```text
框架设计/引擎总体架构/阶段完成记录/2026-07-03-Complex-Shooter-Real-Authoring-to-Playable-Vertical-Slice-v1/00-总览.md
```

## 3. 分阶段施工

### Stage A：新增 vertical slice report 数据结构

新增：

```text
AUTHORING_TO_PLAYABLE_VERTICAL_SLICE_REPORT_SCHEMA_VERSION
AuthoringToPlayableVerticalSliceRequest
AuthoringToPlayableVerticalSliceReport
AuthoringToPlayableVerticalSliceStatus
AuthoringToPlayableStep
AuthoringWorkspaceDomainEvidence
AuthoringWorkflowStepEvidence
AuthoringToPlayableDiagnostic
AuthoringToPlayableMetrics
```

阶段测试：

```powershell
cargo test -p project_e2e_gate authoring_to_playable_report_serializes
```

### Stage B：EditorSession authoring readiness 接入

实现：

```text
EditorSession::new()
session.execute_command(OpenProject)
session.build_ui_model()
collect workspace domains
collect authoring workflow steps
validate required domains
```

必须覆盖：

```text
Project
Assets
Scene
Prefabs
Rules
Input
AUI
Build
Reports
```

阶段测试：

```powershell
cargo test -p project_e2e_gate vertical_slice_opens_sample_project_through_editor_session
```

### Stage C：复用 existing complex project E2E 链路

实现：

```text
run_complex_project_e2e_gate(request)
merge status / metrics / artifacts / diagnostics / gaps
```

规则：

```text
不复制 DesktopExportPipeline 逻辑。
不复制 runtime player 逻辑。
不把 optional real window skipped 当失败。
```

阶段测试：

```powershell
cargo test -p project_e2e_gate vertical_slice_runs_export_package_and_player_chain
```

### Stage D：整体回归

执行：

```powershell
cargo fmt --check
cargo test -p project_e2e_gate
cargo test -p editor_core authoring_workflow
cargo test -p runtime_player_winit
```

### Stage E：文档归档与入口同步

动作：

```text
生成阶段完成记录。
将本施工文档从 施工文档/当前 移动到 施工文档/已完成。
更新 00-文档地图。
更新 49-当前状态与下一步入口。
更新 54-当前可自动化施工总入口。
更新 施工文档/README。
更新 阶段完成记录/README。
```

## 4. 验收标准

完成后必须证明：

```text
真实 sample project 能通过 EditorSession 打开。
UI model 进入 AuthoringWorkspace。
workspace domains 包含 project/assets/scene/prefabs/rules/input/aui/build/reports。
authoring workflow 可读取。
同一项目继续 export/package/headless-player。
report 可序列化。
report 写明 artifacts / diagnostics / gaps。
```

## 5. 不允许做的事

```text
不允许新增 Player / Enemy / Bullet 等引擎专用 API。
不允许重写 DesktopExportPipeline。
不允许重写 runtime_player_winit。
不允许把真实 OS window 设为默认必过。
不允许把测试 fixture 当成用户级闭环。
```

## 6. 施工自审

### 6.1 是否合乎规格

结论：通过。

```text
施工目标就是用户确认的复杂打飞机真实 authoring-to-playable 纵切。
```

### 6.2 是否合乎设计

结论：通过。

```text
新逻辑放在 project_e2e_gate。
EditorSession 只负责编辑器状态。
existing E2E gate 继续负责 export/player 链路。
```

### 6.3 是否方便实现

结论：通过。

```text
现有 EditorSession 已能 OpenProject。
现有 build_ui_model 已能生成 workspace/workflow。
现有 run_complex_project_e2e_gate 已能导出并运行 headless player。
```

### 6.4 是否合理且能实现

结论：通过。

```text
C-min 不追求完整商业游戏，只证明真实样例项目主链路可走通。
```

## 7. 开始施工条件

本施工文档通过自审，可以开始按 Stage A-E 施工。
