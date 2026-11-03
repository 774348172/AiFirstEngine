# 166-当前可自动化施工文档-Complex Shooter Real Project End-to-End Gate v1

## 1. 施工目标

依据：

```text
165-Complex-Shooter-Real-Project-End-to-End-Gate-v1方案.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
```

目标：

```text
建立一个真实 sample project 目录。
新增一个独立 project_e2e_gate crate，不把 gate 继续塞进 engine_runtime。
从 sample project 读取项目源文件。
通过现有 editor_core / engine_runtime / runtime_player_winit 链路生成 RuntimePackage / exported package。
运行 headless player 多帧。
输出 complex-project-e2e-gate-report.v1。
真实窗口 gate 第一版做 optional / reportable，不作为默认阻塞。
```

本施工不做：

```text
不把 Player / Enemy / Bullet / Score 做成 engine_runtime API。
不重写 RuntimePackageBuilder / DesktopExportPipeline / Player。
不继续扩张 engine_runtime::complex_project_validation。
不要求真实窗口在所有环境默认运行。
```

## 2. 涉及文件

预计新增：

```text
rust/crates/project_e2e_gate/Cargo.toml
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/report.rs
rust/crates/project_e2e_gate/src/sample_project.rs
rust/crates/project_e2e_gate/src/gate.rs
rust/crates/project_e2e_gate/src/tests.rs
samples/complex_shooter_project/project.afengine.json
samples/complex_shooter_project/Scenes/Main.scene.json
samples/complex_shooter_project/Prefabs/*.prefab.json
samples/complex_shooter_project/Assets/*
samples/complex_shooter_project/Rules/rule-manifest.json
samples/complex_shooter_project/Input/input.default.json
samples/complex_shooter_project/AUI/hud.aui.json
```

预计修改：

```text
rust/Cargo.toml
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
```

## 3. 分阶段任务

### A. 新增 sample project

任务：

```text
建立 samples/complex_shooter_project。
写入真实项目 manifest、scene、prefab、asset、rule、input、aui 文件。
项目文件允许包含 shooter 语义。
引擎 API 不新增 shooter 专用类型。
```

测试：

```powershell
Test-Path G:\gameEngin\samples\complex_shooter_project\project.afengine.json
```

### B. 新增 project_e2e_gate crate

任务：

```text
新增 crate。
定义 ComplexProjectE2eGateReport / Step / Gap / Metrics / Diagnostic。
定义 ComplexProjectE2eGateRequest。
实现 sample project manifest 读取和基础计数。
```

测试：

```powershell
cargo test -p project_e2e_gate
```

### C. 接入 RuntimePackageBuilder

任务：

```text
从 sample project 源文件构造 RuntimePackageBuildInput。
调用 RuntimePackageBuilder 输出到临时目录。
加载生成的 RuntimePackage 并记录 metrics。
不允许直接调用 engine_runtime::complex_project_validation 作为成功依据。
```

测试：

```powershell
cargo test -p project_e2e_gate runtime_package
```

### D. 接入 DesktopExportPipeline / exported package

任务：

```text
用现有 editor_core DesktopExportPipeline 或等价公开 API 生成导出目录。
如果现有 API 需要完整 EditorProject，则建立最小真实 project fixture。
导出结果必须记录 exported_package_path 和 artifacts。
```

测试：

```powershell
cargo test -p project_e2e_gate export
```

### E. 接入 runtime_player_winit headless run

任务：

```text
用 exported runtime_package 启动 headless player gate。
运行多帧。
注入或记录默认 input frame。
读取 player report。
记录 frames_run / draw_item_count / present_count。
```

测试：

```powershell
cargo test -p project_e2e_gate headless_player
```

### F. Optional real-window report

任务：

```text
默认不打开真实窗口。
report 中写 realWindow step = skipped，说明需要 feature/local 环境。
如果 project_e2e_gate 后续启用 real-window feature，再接 runtime_player_winit real window smoke。
```

测试：

```powershell
cargo test -p project_e2e_gate real_window_skipped_is_reported
```

### G. 整体 gate

任务：

```text
实现 run_complex_project_e2e_gate。
生成完整 complex-project-e2e-gate-report.v1。
确保失败也输出 report。
```

测试：

```powershell
cargo test -p project_e2e_gate
cargo test -p engine_runtime
cargo test -p editor_core
cargo test -p runtime_player_winit
cargo fmt --check
```

## 4. 完成后文档同步

施工完成后：

```text
生成 阶段完成记录/2026-07-03-Complex-Shooter-Real-Project-End-to-End-Gate-v1/00-总览.md
将本施工文档移动到 施工文档/已完成/
更新 00 / 49 / 54 / 施工文档 README / 阶段完成记录 README
回填 149 中 Q1/Q3/Q4/Q5/Q6/Q26 状态
```

## 5. 施工自审

```text
Specification fit:
  对应 165 方案，生成真实项目端到端 gate 的施工计划，并准备直接施工。

Rule fit:
  遵守引擎底座边界。sample project 可以有项目语义，engine_runtime 不新增 shooter API。

Textual consistency:
  分阶段任务和测试命令对应目标链路：sample -> package -> export -> player -> report。

Design fit:
  优先解决真实用户级闭环，不继续沉迷拆文件或小功能。

Implementation feasibility:
  依赖现有 RuntimePackageBuilder、DesktopExportPipeline、runtime_player_winit 和报告系统，可渐进接入。

Practical reasonableness:
  real-window 第一版可报告 skipped，避免环境差异阻塞默认回归，同时不假装真实窗口已通过。
```

结论：

```text
本施工文档通过自审，可以开始施工。
```
