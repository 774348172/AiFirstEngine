# 330 First-class Independent Engine Tools + Host Registry Conformance v1 施工文档

状态：已完成（source_mcp_complete_with_host_capability_deferred）。Gate A/B/C/D 已通过；Codex Native Gate E 因宿主不提供 Native Registry connector，按用户决定以外部能力未提供收尾，不修改架构、不伪称通过。
对应正式方案：`框架设计/引擎总体架构/330-First-class-Independent-Engine-Tools-Host-Registry-Conformance-v1方案.md`

## 施工目标

将九项 Engine capability 作为独立 typed tools 直接注册到真实 Host Tool Registry；建立最小 Engine-owned Tool Definition 共同来源；MCP 仅逐工具兼容投影。禁止统一 MCP 大工具、`engine_execute` 和 `catalog/execute` 宿主路由。

## 必须保持

- 不增加默认工具数量；
- 不改变 Engine Tool Kernel 的业务语义、Runtime、Compiler、RuntimePackage 或项目玩法；
- 不新增 Provider Registry、第二个 Agent 或第二份 authority；
- `capture` 继续作为 observe typed request；内部 prepare/refresh/snapshot 等阶段不注册为工具。

## Gate A：Definition Baseline

必要性与验证成本：本 Gate 核对已有工具定义和投影，不重建 Provider。最小源码修改仅为
`rust/crates/engine_tool_provider/tests/mcp_process_smoke.rs` 的失效注入锚点；不修改产品协议或玩法。
定义/权限风险由 lib + host_adapter_equivalence + skill_workflow_contract 覆盖（33项）；
已有真实 MCP smoke 的观测合同失败由该单项回归覆盖。避免重复执行完整 crate 所含的项目冷编译。
预计定向检查秒级，MCP 集成约3分钟；若再次失败，保留完整响应并回到失败 owner，不机械重跑全套。

盘点九项工具的现有 definition、schema、dispatch、capability、结果 envelope、错误和 cancellation。

验证：`cargo test -p engine_tool_provider --lib --locked`（按实际 owner 调整）；definition 数量与名称快照；禁止二级路由负例。

证据：`.artifacts/330-gate-a-definition.json`。

### Gate A 结果（2026-09-12）

- `cargo test -p engine_tool_provider --locked --lib --test host_adapter_equivalence --test skill_workflow_contract`：25+6+2=33 passed；日志 `.artifacts/330-gate-a-focused.txt`。
- `cargo test -p engine_tool_provider --locked --test mcp_process_smoke`：1 passed；最终编译0.95秒、执行186.58秒；日志 `.artifacts/330-mcp-repair-r2.txt`。本轮34项去重测试通过，不宣称完整crate所有测试重跑。
- 首因：计数/观测字符串替换未命中更新后的样例，Player报告 `project_observation.value_undeclared/value_missing`。修复注入锚点并断言唯一命中；第一轮修复被 `validate_project_runtime_format` 拒绝，调整注入格式后通过。失败日志 `.artifacts/330-gate-a-test-rerun.txt`、`.artifacts/330-mcp-repair.txt` 保留。
- 变更仅在测试夹具；玩法负例、修正后的正例、冻结交付复验与证据篡改/缺失拒绝断言保留。修改文件的 `rustfmt --check --edition 2021` 通过；产品源码、安装和持久配置未改。
- Provider九项定义见 `.artifacts/330-gate-a-definition.json`；Native与MCP共享 `tool_definitions_for(ModelDefault)`，无需重复建立定义源。
- 当前宿主九项独立MCP工具元数据见 `.artifacts/330-host-tools-visible.json`；实际inspect返回 completed，证据 `.artifacts/330-host-inspect.json`。这证明MCP逐工具暴露和一次真实调用，不证明Native注册或Gate E全链通过。
- Skill已加入普通施工问题自主修复、验证后继续的规则，`quick_validate.py`通过。

### Gate B/C 结果（2026-09-12）

- `NativeEngineToolProvider` / `NativeHostAdapter` 已确认直接返回九项独立工具定义并共享 Engine Tool Kernel；`host_adapter_equivalence`、`skill_workflow_contract` 共 8 项通过。证据：`.artifacts/330-gate-b-native-registry.json`、`.artifacts/330-gate-b-native.txt`。
- MCP stdio 逐工具 `tools/list` 返回同一九项名称，独立进程 smoke 通过；无统一 `engine_execute`、`toolId` 或 `operation` 路由。证据：`.artifacts/330-gate-a-definition.json`、`.artifacts/330-mcp-repair-r2.txt`。
- 当前环境没有可调用的真实 ChatGPT Native Tool Registry connector，因此 Gate B 的 source-level direct registration 已通过，Real Host Native Registry 仍标记为未验证；不可将 MCP 可见性误报为 Native 宿主连接。

### Gate D/E 结果（2026-09-12）

- `host_adapter_equivalence` 6 项与独立 `mcp_process_smoke` 1 项通过；Native/MCP 的工具定义、结果、诊断、权限、mutation receipt、revision/artifact/delivery identity 和失败分支保持一致。证据：`.artifacts/330-gate-d-e-final.txt`。
- Gate E 阻塞：本工作区没有真实 ChatGPT Native Tool Registry connector，无法完成 Native 宿主发现和真实 Native 调用链。当前 MCP 宿主九项独立工具可见且 inspect 已调用成功，但不构成 Gate E 证据。

## Gate B：Native Direct Registration

实现宿主 Native Provider 的逐工具注册，并验证真实 Host Tool Registry 能直接列出九项独立 typed tools。

验证：Provider owner 测试、真实宿主 discovery 证据；检查每项 schema、grant 与结果类型。

证据：`.artifacts/330-gate-b-native-registry.json`。

## Gate C：MCP Per-tool Projection

将九项工具逐个投影到 MCP，确认 MCP 不出现统一入口、`toolId` 或 `operation` 二次路由。

验证：MCP definition 快照与 Native definition 对照；逐工具 dispatch 测试。

证据：`.artifacts/330-gate-c-mcp-projection.json`。

## Gate D：Native/MCP Equivalence

以相同项目 revision 和 typed 输入执行 Native 与 MCP，比较 result、diagnostics、revision、artifact、delivery identity 和 cancellation。

验证：等价性矩阵；失败、拒绝、取消和权限边界负例。

证据：`.artifacts/330-gate-d-equivalence.json`。

## Gate E：Real Host Conformance

在真实 ChatGPT 宿主执行 inspect、mutate、run、playtest、observe、build、delivery_verify 完整链路，保存工具发现、调用结果和身份一致性证据。

验证：宿主真实调用；不替换生产安装、不修改持久配置，除非另行授权。

证据：`.artifacts/330-gate-e-real-host.json`。

## 停止条件

若宿主只能接受统一 MCP 入口、无法直接注册独立 typed tools、需要新增工具路由、或发现 Native/MCP 语义无法共用同一 Kernel，则停止并保留首个失败证据，先修订正式方案。

## 文档与归档

全部 Gate、整体回归和真实宿主验收通过后，生成阶段完成记录，同步 `49`、`54`、施工文档 README 和阶段完成记录 README，并将本文移动到 `施工文档/已完成/`。

## 施工文档自审

本文件只覆盖宿主工具投影与一致性，不迁移 Engine/Project 语义，不增加工具，不暴露内部机械阶段。每个 Gate 均对应独立风险和证据；真实宿主 Gate 仅在源代码和投影 Gate 通过后执行。
