# 331 Independent Engine Tool Manuals + Skill Workflow Pack + Capability Maturity v1 施工文档

状态：已完成。对应正式方案：`331-Independent-Engine-Tool-Manuals-Skill-Workflow-Pack-Capability-Maturity-v1方案.md`。

## 施工边界与过量施工检查

本轮只新增 AI Development Pack 文档与最小验证夹具，不修改 Runtime、Compiler、Provider、MCP、工具 schema 或默认工具数量。不新增运行时 Module、状态机、动态 Discovery、统一 Gateway 或固定工作流。若实现需要上述变更，立即停止并回填方案。

## Gate A：Manual Baseline

在 `AI Development Pack/tools/` 建立九份独立工具说明书。每份绑定工具名、schema 版本、用途、输入、结果、诊断、可选恢复方向、权限、副作用和成熟度；不描述内部 prepare/refresh/snapshot 等机械阶段。

验证：目录完整性、工具名唯一性、必需章节存在、说明书未出现 `engine_execute` / `catalog` / 固定强制顺序。

证据：`.artifacts/331-gate-a-manuals.json`。

## Gate B：Lightweight Skill Recommendations

建立 `AI Development Pack/skill/engine-workflow.md`，只写基于当前 ToolResult 的局部可跳过推荐。每条推荐必须使用条件和建议语气，不执行工具、不保存状态、不形成完整菜谱。

验证：推荐规则静态审查与负例检查（不得出现“必须先”“自动调用”“固定顺序”“完整任务计划”）。

证据：`.artifacts/331-gate-b-skill.json`。

## Gate C：Maturity Evidence

建立九项工具的成熟度记录，记录 maturity、版本、证据引用和已知限制。成熟度只提供提示，不隐藏工具、不改变权限。

验证：九项工具一一覆盖，证据引用存在，状态值受限于 `experimental/ready/qualified`。

证据：`.artifacts/331-gate-c-maturity.json`。

## Gate D：AI Scheduling Conformance

使用静态结果样例验证 AI 可以采纳、忽略或替换 Skill 推荐；确认推荐不是执行器或不可跳过状态机。

验证：推荐规则 conformance 检查；不启动真实宿主，不重复 330 的工具调用验收。

证据：`.artifacts/331-gate-d-scheduling.json`。

## 归档条件

Gate A-D 通过后，生成阶段完成记录，同步 `49`、`54`、文档地图、阅读顺序、施工文档 README 和阶段完成记录 README，并将本文移动到 `施工文档/已完成/`。

## 停止条件

需要修改 Engine tool schema、Provider、Runtime、Compiler、MCP transport，或需要把 Skill 实现成执行器、状态机、固定流程时，停止施工并先修订正式方案。

## 完成证据

Gate A-D 全部通过。证据：`.artifacts/331-gates-a-c.json`、`.artifacts/331-gate-d-scheduling.json`。九份说明书位于 `AI Development Pack/tools/`，Skill 推荐位于 `AI Development Pack/skill/engine-workflow.md`，成熟度位于 `AI Development Pack/maturity/capability-maturity.json`。本轮未修改 Engine、Provider、MCP、工具 schema、默认工具数量或项目玩法。
