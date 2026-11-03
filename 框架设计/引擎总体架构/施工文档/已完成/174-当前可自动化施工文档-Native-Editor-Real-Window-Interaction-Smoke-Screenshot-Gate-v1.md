# 174-当前可自动化施工文档-Native Editor Real Window Interaction Smoke / Screenshot Gate v1

## 1. 施工目标

实现 Native Editor Real Window Interaction Smoke / Screenshot Gate v1 的 B-C-min。

目标是建立真实窗口交互 smoke gate：

```text
NativeEditorInteractionScenario
  -> controlled real-window interaction bridge
  -> NativeEditorApplication
  -> DrawList / HitRegion / UiCommand
  -> present evidence
  -> screenshot evidence metadata
  -> RealWindowInteractionSmokeReport
```

## 2. 涉及文件

预计新增：

```text
rust/crates/editor_window_winit/src/real_window_interaction_gate.rs
rust/crates/editor_window_winit/src/tests/real_window_interaction_gate.rs
框架设计/引擎总体架构/阶段完成记录/2026-07-03-Native-Editor-Real-Window-Interaction-Smoke-Screenshot-Gate-v1/00-总览.md
```

预计修改：

```text
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_window_winit/src/tests/mod.rs
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

## 3. 阶段 A：报告与数据结构

新增：

```text
RealWindowInteractionSmokeStatus
RealWindowInteractionSmokeScenario
RealWindowInteractionSmokeReport
RealWindowInteractionStepReport
RealWindowScreenshotEvidence
RealWindowInteractionDiagnostic
```

要求：

```text
全部 Serialize / Deserialize。
status 至少支持 Passed / Failed / Skipped。
feature 未启用和环境阻塞必须是结构化状态，不允许 panic。
```

阶段测试：

```powershell
cargo test -p editor_window_winit real_window_interaction_report_serializes
```

## 4. 阶段 B：受控 window event bridge

新增 runner：

```text
RealWindowInteractionSmokeRunner
```

职责：

```text
复用 NativeEditorInteractionScenario。
先 frame 得到 DrawList。
每步找到 HitRegion 中心点。
通过 EditorInputEvent 进入 NativeEditorApplication。
记录每步的 command / mode / selection / feedback / draw count / present evidence。
```

注意：

```text
不能直接调用 EditorSession 伪造业务结果。
不能复制一套 NativeEditorInteractionRunner 的独立规则。
必须复用现有 scenario 的语义。
```

阶段测试：

```powershell
cargo test -p editor_window_winit real_window_interaction_gate
```

## 5. 阶段 C：present / screenshot evidence

第一版截图策略：

```text
默认测试生成 screenshot evidence metadata。
真实 GPU readback 若当前 editor_wgpu_renderer 尚未提供稳定 API，则第一版不强行接入像素文件。
real-window feature 下保留 ignored/local-only 测试入口。
```

报告必须记录：

```text
window_created
surface_created
surface_configured
present_status
frame_count
draw_command_count
hit_region_count
screenshot.kind
screenshot.width
screenshot.height
screenshot.frame_index
```

阶段测试：

```powershell
cargo test -p editor_window_winit screenshot_evidence
```

## 6. 阶段 D：真实窗口 feature 编译边界

要求：

```text
默认 cargo test 不要求打开真实 OS window。
cargo check -p editor_window_winit --features real-window 必须通过。
如果添加 ignored 测试，必须标注 local-only。
```

阶段测试：

```powershell
cargo check -p editor_window_winit --features real-window
```

## 7. 整体验收测试

施工完成后运行：

```powershell
cargo fmt --check
cargo test -p editor_window_winit real_window_interaction
cargo test -p editor_window_winit
cargo test -p editor_core
```

## 8. 文档同步

完成后：

```text
生成阶段完成记录。
把本施工文档移动到 施工文档/已完成/。
更新 00 / 49 / 54 / 149 / 施工文档 README / 阶段完成记录 README。
54 中当前可施工文档恢复为“无”。
49 下一步入口改为新的最值得讨论系统。
```

## 9. 施工自审

### 9.1 是否符合方案

符合 173 方案 B-C-min：第一版真实窗口 smoke + 受控事件桥 + screenshot evidence metadata。

### 9.2 是否符合长期主义

符合。它补的是真实窗口验收层，不污染 EditorSession / Runtime / UI renderer 的业务边界。

### 9.3 是否会增加重复系统

风险可控。施工要求明确复用 NativeEditorInteractionScenario，不再新增平行 scenario 语义。

### 9.4 是否方便测试

默认测试不依赖真实 OS window；真实窗口 feature 只做编译检查或 ignored/local-only。

### 9.5 是否合理

合理。先把真实窗口、present、输入桥、报告闭环建立起来，再逐步接 OS-level driver 和 golden image。

