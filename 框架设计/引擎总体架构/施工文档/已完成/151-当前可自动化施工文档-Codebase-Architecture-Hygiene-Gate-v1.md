# 151-当前可自动化施工文档-Codebase Architecture Hygiene Gate v1

## 1. 施工目标

根据正式方案：

```text
../../151-Codebase-Architecture-Hygiene-Gate-v1方案.md
```

本轮按完整方案 C 施工，但采用阶段化小步迁移：

```text
先建立代码结构治理门禁和目标骨架。
再按 crate 分阶段拆大文件。
每阶段完成后立刻测试。
不做功能变更。
不减少测试。
```

## 2. 本轮第一批施工范围

第一批只执行阶段 A：

```text
阶段 A：建立治理骨架和门禁。
```

阶段 A 要完成：

```text
新增代码结构健康检查工具。
输出 architecture hygiene report。
记录当前文件体量基线。
建立测试，防止工具输出漂移。
更新文档入口。
```

阶段 A 不做：

```text
不迁移 editor_core 业务逻辑。
不拆 editor_window_winit 真实窗口代码。
不移动 UI model / renderer / WGPU renderer 类型。
不改变任何 public API。
```

## 3. 预计涉及文件

新增：

```text
rust/crates/code_hygiene/Cargo.toml
rust/crates/code_hygiene/src/lib.rs
rust/crates/code_hygiene/src/main.rs
rust/crates/code_hygiene/tests/hygiene_report.rs
```

修改：

```text
rust/Cargo.toml
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-02-Codebase-Architecture-Hygiene-Gate-v1/00-总览.md
```

## 4. 阶段 A 任务

### A1：新增 code_hygiene crate

能力：

```text
扫描 rust/crates 下的 .rs 文件。
统计总文件数、总行数、超过 1000 / 2000 / 4000 行文件数量。
输出最大文件列表。
识别重点热点文件：
  editor_core/src/lib.rs
  editor_window_winit/src/lib.rs
  editor_ui_model/src/lib.rs
  editor_ui_renderer/src/lib.rs
  editor_wgpu_renderer/src/lib.rs
生成 JSON report，便于 AI 和文档读取。
```

测试：

```powershell
cargo test -p code_hygiene
```

### A2：接入 workspace

修改 `rust/Cargo.toml`：

```text
members 增加 crates/code_hygiene。
```

测试：

```powershell
cargo test -p code_hygiene
cargo metadata --no-deps --format-version 1
```

### A3：生成基线报告

命令：

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-02-Codebase-Architecture-Hygiene-Gate-v1\code-hygiene-baseline.json
```

报告必须包含：

```text
files
total_lines
over_1000
over_2000
over_4000
largest_files
hotspots
recommendations
```

### A4：阶段测试

执行：

```powershell
cargo fmt --check
cargo test -p code_hygiene
cargo test -p editor_core
cargo test -p editor_window_winit
```

## 5. 后续阶段入口

阶段 A 完成后，后续施工文档应继续生成阶段 B：

```text
Codebase Architecture Hygiene Gate v1 - Stage B editor_window_winit split
```

阶段 B 才开始拆：

```text
editor_window_winit/src/lib.rs
```

## 6. 风险控制

```text
第一阶段只新增工具和文档，不改业务逻辑。
任何重构前先有基线 report。
后续每个阶段必须对比基线和测试结果。
不通过减少测试来降低行数。
```

## 7. 施工自审

```text
Specification fit:
  满足用户选择完整方案 C，并要求小心、分阶段、边做边测试。

Rule fit:
  不新增玩法规则，不改变引擎底座行为；仅建立代码结构治理门禁。

Textual consistency:
  正式方案是完整目标，施工文档第一批只执行阶段 A，边界明确。

Design fit:
  符合 AI-first 长期维护目标，为后续拆 editor_window_winit / editor_core 做准备。

Implementation feasibility:
  新增独立 code_hygiene crate 风险低，不影响现有业务 crate。

Practical reasonableness:
  不一上来大规模移动文件，先建立可量化基线和工具，避免盲目重构。
```

结论：

```text
施工文档通过自审，可以开始阶段 A 施工。
```
