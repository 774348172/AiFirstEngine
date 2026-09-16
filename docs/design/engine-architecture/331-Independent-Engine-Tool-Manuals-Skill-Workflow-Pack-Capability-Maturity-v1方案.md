# 331 Independent Engine Tool Manuals + Skill Workflow Pack + Capability Maturity v1

状态：正式方案；已根据用户讨论收敛。
日期：2026-09-12

## 1. 目标

为每个独立 Engine tool 提供一份面向 AI 的单独说明书，并由 Skill 提供轻量、可跳过的局部组合建议。说明书解释单个工具如何使用；Skill 只根据当前 ToolResult 推荐可能的下一步；AI Agent 保留最终调度、修改、重试、换方案和停止决定。

## 2. 核心分层

```text
Tool Manual
  -> 解释一个工具的用途、输入、结果和失败恢复

Skill Workflow Pack
  -> 根据当前 ToolResult 提供局部、可跳过的下一步建议

AI Agent
  -> 选择是否采纳建议、调用哪个工具、修改什么以及何时停止

Engine Tool Provider
  -> 执行被 AI 选中的独立 typed tool
```

Skill 不执行工具、不保存运行状态、不拥有项目 authority、不替 AI 规划完整任务。

## 3. 独立工具说明书

为当前九项默认工具分别建立说明书：

```text
engine_project_inspect.md
engine_project_check.md
engine_project_mutate.md
engine_project_rollback.md
engine_runtime_run.md
engine_runtime_playtest.md
engine_runtime_observe.md
engine_project_build.md
engine_delivery_verify.md
```

每份说明书包含：用途、适用时机、typed 输入、前置事实、结果状态、关键输出、失败诊断解释、可选恢复方向、权限与副作用、版本和成熟度。说明书不得暴露 `prepare`、`refresh`、`snapshot`、generated glue、RuntimePackage 装配等工具内部机械阶段。

## 4. Skill 的组合边界

Skill 只表达局部条件化推荐，例如：

```text
如果 check 返回编译诊断，推荐 AI 读取诊断并考虑修复后再次 check。
如果 run 成功且需要输入验证，推荐 AI 考虑 playtest。
如果 playtest 返回 runRef，推荐 AI 在需要更多事实时 observe。
如果 build 返回 deliveryRef，推荐 AI 考虑 delivery_verify。
```

所有推荐必须是可跳过的局部提示，使用“可以考虑”“推荐”“需要更多证据时”等表达。禁止把组合写成固定顺序、强制步骤、自动跳转或不可跳过的状态机。AI 可以采纳、忽略或改用其它合法工具。

## 5. Pack 组织

```text
AI Development Pack
  ├── tools/                 # 九份独立工具说明书
  ├── skill/                 # 轻量局部推荐规则
  ├── schemas/               # 最小机器元数据
  └── maturity/              # 能力成熟度与版本
```

Tool Manual 与 Tool Definition 绑定工具名和 schema 版本；Skill 推荐绑定结果字段和诊断码。任何 schema、结果或错误语义变化都必须使对应说明书或推荐规则进入审查范围。

## 6. Capability Maturity

成熟度只描述能力状态，不决定 AI 是否可以调用：

```text
experimental -> ready -> qualified
```

成熟度记录版本、验证范围、已知限制和证据引用。Skill 可以将成熟度作为提示信息返回，但不得据此隐藏稳定工具或替 AI 做最终选择。

## 7. 施工 Gate

### Gate A：Manual Baseline

建立九份工具说明书目录、模板、版本绑定和术语表；核对说明书与现有 typed schema、结果和错误码一致。

### Gate B：Lightweight Skill Recommendations

实现基于 ToolResult 的局部推荐。验证推荐可跳过、不执行工具、不保存状态、不生成固定全流程。

### Gate C：Maturity Evidence

为九项工具记录当前成熟度、证据身份和已知限制；不改变工具可见性和调用权限。

### Gate D：AI Scheduling Conformance

用结果驱动场景验证 AI 可以采纳、忽略或替换 Skill 推荐；验证 Skill 不阻断合法的不同调度路径。

## 8. 明确不做

- 不新增运行时 Module、Provider、工具或 Agent；
- 不建设动态 Capability Discovery 或统一 `catalog/execute`；
- 不把 Skill 变成执行器、状态机或任务规划器；
- 不规定完整工具调用菜谱；
- 不迁移 Project 玩法、Runtime、Compiler 或 RuntimePackage 语义；
- 不把 Engine 内部阶段暴露给 AI。

## 9. 验收标准

完成后必须证明：九项工具各自有独立、可版本化的 AI 说明书；Skill 只提供基于当前结果的轻量可跳过推荐；AI 仍能自主选择工具和调度路径；成熟度与证据可追溯；工具 schema、结果和错误语义没有漂移；默认工具数量不增加。
