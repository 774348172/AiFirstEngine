# 164-当前可自动化施工文档-Codebase Hygiene Stage K 最终收敛与入口同步

## 1. 施工目标

依据：

```text
151-Codebase-Architecture-Hygiene-Gate-v1方案.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
36-文档体系与维护规则.md
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
```

本阶段是 Codebase Hygiene 当前一轮治理的收尾阶段，不新增功能。

目标：

```text
汇总 Stage A-J 的 before/after hygiene 数据。
确认 Q16-Q20 的真实状态。
确认 施工文档/当前/ 没有已经完成但未归档的文档。
更新 00 / 49 / 54 / README / 阶段完成记录 README。
生成当前工程结构治理总结。
给下一轮大系统讨论提供可信入口。
```

## 2. 前置条件

开始 Stage K 前，必须确认以下阶段已经完成或被明确记录为跳过：

```text
Stage A: Codebase Architecture Hygiene Gate v1
Stage B: editor_window_winit 生产代码拆分
Stage C: editor_window_winit 测试拆分
Stage D: editor_ui_renderer 拆分
Stage E: editor_wgpu_renderer 拆分
Stage F1: editor_core / EditorSession service 初步拆分
Stage F2: EditorSession service 继续拆分
Stage F3: EditorSession command service 拆分
Stage G: scene_editing domain 拆分
Stage H: editor_ui_model domain 拆分
Stage F4: EditorSession 剩余 command service 拆分
Stage I: editor_core 测试 domain 迁移
Stage J: engine_runtime domain grouping
```

规则：

```text
如果某阶段未完成，Stage K 只能记录“未完成/跳过原因”，不能伪造完成状态。
如果某阶段测试未跑完，必须写明实际测试范围，不能写“全部通过”。
```

## 3. 分阶段任务

### K-0：收集当前状态

```powershell
cargo run -p code_hygiene -- --root G:\gameEngin\rust\crates --output G:\gameEngin\框架设计\引擎总体架构\阶段完成记录\2026-07-03-Codebase-Hygiene-StageK最终收敛与入口同步\after.json
cargo fmt --check
cargo test -p code_hygiene
```

### K-1：完整回归

建议执行：

```powershell
cargo test -p editor_core
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_wgpu_renderer
cargo test -p editor_input
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo test -p code_hygiene
```

记录规则：

```text
如果某 crate 测试时间过长或环境缺失，只能记录“未执行/跳过原因”。
如果某测试失败，Stage K 不能归档为完全完成，必须记录失败并保留下一步修复入口。
```

### K-2：生成治理总结

新增：

```text
阶段完成记录/2026-07-03-Codebase-Hygiene-StageK最终收敛与入口同步/00-总览.md
```

必须包含：

```text
Stage A-J 完成状态表
before/after 最大文件对比
仍超过 2000 行的文件
仍超过 1000 行的文件
Q16-Q20 状态
本轮治理没有覆盖的风险
下一轮推荐入口
实际执行的测试命令和结果
```

### K-3：更新入口文档

更新：

```text
00-文档地图.md
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
施工文档/README.md
阶段完成记录/README.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
```

入口文档规则：

```text
只写摘要，不写长流水账。
当前施工目录状态必须真实。
下一步入口必须指向一个大系统，不指向零散小点。
```

### K-4：归档本轮施工文档

当 Stage K 自身完成后：

```text
移动 164 到 施工文档/已完成/
确认 施工文档/当前/ 中没有已经完成的文档。
如果还有未施工文档，必须明确保留原因。
```

## 4. 不做什么

```text
不再顺手拆代码。
不改变 runtime/editor 行为。
不把未完成阶段标记为完成。
不清理用户未授权的无关文件。
不生成新的功能施工文档，除非入口文档明确下一轮大系统。
```

## 5. 验收标准

```text
最终 hygiene report 已生成。
完整回归结果被真实记录。
入口文档与施工文档目录状态一致。
149 Q16-Q20 状态与实际工程一致。
施工文档/当前/ 状态真实可信。
阶段完成记录包含本轮治理总结和下一轮入口建议。
```

## 6. 施工文档自审

```text
Specification fit:
  对应剩余 Stage K 收尾阶段，只生成施工文档，不开始施工。

Rule fit:
  符合文档治理规则和每轮施工完成后同步入口的要求。

Textual consistency:
  明确前置条件，避免把未完成阶段误写为完成。

Design fit:
  让长期讨论重新回到可信入口，减少重复整理和旧文档带偏。

Implementation feasibility:
  主要是报告、测试和文档同步，风险低。

Practical reasonableness:
  收尾阶段不夹带代码重构，降低治理收束风险。
```
