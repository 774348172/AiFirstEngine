# 178-当前可自动化施工文档-Editor Visual Regression / Golden Image Gate v1

## 1. 施工目标

依据：

```text
177-Editor-Visual-Regression-Golden-Image-Gate-v1方案.md
```

实现 `Editor Visual Regression / Golden Image Gate v1` 的 C-min 版本。

目标是建立一个稳定、headless、结构化的编辑器视觉回归门：

```text
NativeEditorApplication
  -> UiDrawList
  -> UiGpuDrawPlan
  -> EditorVisualRegressionEvidence
  -> EditorVisualRegressionBaseline compare
  -> EditorVisualRegressionReport
```

本阶段不做真实 GPU 像素 diff，不做默认真实窗口截图，不自动更新 baseline。

## 2. 涉及文件

预计新增：

```text
rust/crates/editor_window_winit/src/visual_regression_gate.rs
rust/crates/editor_window_winit/src/tests/visual_regression_gate.rs
框架设计/引擎总体架构/阶段完成记录/2026-07-03-Editor-Visual-Regression-Golden-Image-Gate-v1/00-总览.md
```

预计修改：

```text
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_window_winit/src/tests/mod.rs
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

## 3. 分阶段施工

### Stage A：visual regression 数据结构与比较器

新增：

```text
EDITOR_VISUAL_REGRESSION_REPORT_SCHEMA_VERSION
EditorVisualRegressionScenario
EditorVisualRegressionBaseline
EditorVisualRegressionEvidence
EditorVisualRegressionReport
EditorVisualRegressionStatus
EditorVisualRegressionDiagnostic
EditorVisualRegressionRunner
```

规则：

```text
Baseline missing -> BaselineMissing。
Baseline match -> Passed。
Baseline mismatch -> Failed，并输出 diagnostics。
structural_hash 必须稳定，不能依赖随机地址或 HashMap 顺序。
```

阶段测试：

```powershell
cargo test -p editor_window_winit visual_regression_report_serializes
cargo test -p editor_window_winit visual_regression_matching_baseline_passes
cargo test -p editor_window_winit visual_regression_mismatched_baseline_fails
cargo test -p editor_window_winit visual_regression_missing_baseline_is_reported
```

### Stage B：接入 NativeEditorApplication / UiGpuDrawPlan

新增 runner 逻辑：

```text
NativeEditorApplication.frame(width,height)
UiGpuDrawPlan::from_draw_list(app.latest_draw_list())
Evidence::from_app_report_and_draw_plan(...)
```

规则：

```text
runner 不持有 editor session 真相。
runner 不写 baseline 文件。
runner 不创建真实 OS window。
runner 只通过 app 提供的 frame / latest_draw_list 获取证据。
```

阶段测试：

```powershell
cargo test -p editor_window_winit visual_regression_app_scenario_produces_evidence
```

### Stage C：整体回归

执行：

```powershell
cargo fmt --check
cargo test -p editor_window_winit visual_regression
cargo test -p editor_window_winit
cargo test -p editor_wgpu_renderer
```

如果 editor_window_winit 触发上游模型问题，再补充：

```powershell
cargo test -p editor_ui_renderer
cargo test -p editor_core
```

### Stage D：文档归档与入口同步

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
report 可 serde 序列化。
baseline 匹配时通过。
baseline 缺失时返回 BaselineMissing。
baseline 不匹配时失败并给出字段级 diagnostic。
runner 能从 NativeEditorApplication 生成 evidence。
默认测试不需要真实 OS window。
默认测试不需要 GPU readback。
```

## 5. 不允许做的事

```text
不允许自动覆盖 baseline。
不允许把 baseline compare 放进正常 editor frame。
不允许在 SelfUiRenderer 内加入测试逻辑。
不允许把 visual regression 做成项目玩法测试。
不允许默认依赖真实 GPU / OS window。
```

## 6. 施工自审

### 6.1 是否合乎规格

结论：通过。

```text
施工文档只实现用户确认的 177 系统。
默认路径是 headless deterministic visual evidence。
不进入真实窗口像素 diff 的细节泥潭。
```

### 6.2 是否合乎设计

结论：通过。

```text
visual regression gate 放在 editor_window_winit。
SelfUiRenderer / UiGpuDrawPlan 仍然只产出渲染结构。
NativeEditorApplication 仍然只负责应用帧和 draw list。
```

### 6.3 是否方便实现

结论：通过。

```text
已有 NativeEditorApplication::frame / latest_draw_list。
已有 UiGpuDrawPlan::from_draw_list。
已有 editor_window_winit 测试结构。
```

### 6.4 是否合理且能实现

结论：通过。

```text
实现范围小，但边界长期正确。
第一版覆盖 UI 结构性视觉回归，不承诺跨 GPU 像素一致。
后续可扩展 PNG artifact，不需要推翻 report / baseline 结构。
```

## 7. 开始施工条件

本施工文档通过自审，可以开始按 Stage A-D 施工。
