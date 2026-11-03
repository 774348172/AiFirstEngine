# 172-当前可自动化施工文档-Native Editor Real Interaction Validation Gate v1

## 1. 施工目标

依据：

```text
171-Native-Editor-Real-Interaction-Validation-Gate-v1方案.md
121-Native-Editor-Application-Shell方案.md
122-Editor-Authoring-Workspace-C-min方案.md
123-Workspace-Property-Editing-Inspector-Framework-v1方案.md
127-Editor-Interaction-Feedback-Command-Availability-v1方案.md
150-AI-first-Editor-Command-Framework-C-min方案.md
```

目标：

```text
在 editor_window_winit 中建立 Native Editor Interaction Scenario Gate。
第一版使用 headless deterministic replay 证明真实交互链路：
EditorInputEvent -> HitRegion -> UiCommand -> EditorSession -> EditorUiModel -> DrawList -> Report。
```

## 2. 涉及文件

预计修改：

```text
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_window_winit/src/interaction_gate.rs
rust/crates/editor_window_winit/src/tests/native_interaction_gate.rs
rust/crates/editor_window_winit/src/tests/mod.rs
```

完成后同步：

```text
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-03-Native-Editor-Real-Interaction-Validation-Gate-v1/00-总览.md
```

## 3. 分阶段施工

### A. Scenario / Report 数据结构

任务：

```text
新增 NativeEditorInteractionScenario。
新增 NativeEditorInteractionStep。
新增 NativeEditorInteractionReport。
新增 NativeEditorInteractionStepReport。
新增 NativeEditorInteractionStatus / Diagnostic。
```

测试：

```powershell
cargo test -p editor_window_winit interaction_report_serializes
```

### B. Headless deterministic runner

任务：

```text
从 hit_region_id 找到 HitRegion。
由 HitRegion 中心点生成 PointerDown。
调用 NativeEditorApplication::handle_input_event。
每步记录 hit target、command_id/status、mode、selection、revision、draw/hit counts。
按 expect_* 字段判断 pass/fail。
```

测试：

```powershell
cargo test -p editor_window_winit interaction_gate
```

### C. 核心创作链路测试

任务：

```text
Project Launcher create project scenario。
Hierarchy select entity scenario。
Inspector field edit + commit scenario。
Build command disabled/enabled report scenario。
AI proposal accept scenario。
```

测试：

```powershell
cargo test -p editor_window_winit native_interaction
```

### D. 文档收口

任务：

```text
生成阶段完成记录。
施工文档移动到 已完成。
更新 00 / 49 / 54 / 149 / 施工 README / 阶段 README。
```

整体测试：

```powershell
cargo test -p editor_window_winit
cargo test -p editor_core
cargo fmt --check
```

## 4. 施工自审

```text
Specification fit:
  覆盖用户确认的 Native Editor Real Interaction Validation Gate v1 C-min。

Rule fit:
  不绕过 EditorCommandSystem / EditorSession，不新增项目玩法 API。

Textual consistency:
  A-D 从数据结构、runner、scenario 测试到文档收口闭合。

Design fit:
  统一未来编辑器交互验收，不继续补零散按钮。

Implementation feasibility:
  现有 NativeEditorApplication 已具备 frame、handle_input_event、dispatch_command、report。

Practical reasonableness:
  第一版只做 headless deterministic replay，不强制真实 OS 自动点击，风险可控。
```

结论：

```text
施工文档通过自审，可以开始施工。
```
