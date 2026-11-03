# 309-F Default Engine Tool Surface + Skill Workflow Convergence v1

> 文档类型：正式架构子方案（研究结论固化）  
> 方案状态：已完成并归档；Gate A-C 于 2026-09-03 全部通过，能力状态为 `source_integration_passed`  
> 方案选择：方案 B（分阶段兼容收敛），吸收方案 C 的内部诊断投影  
> 生成日期：2026-09-03  
> 适用范围：Engine Provider 默认模型工具面、Skill 工作流投影、ToolResult 局部流转、Host cancellation 边界

## 1. 摘要

本方案解决一个具体问题：当前 `engine_tool_provider` 注册了 20 项工具，但其中一部分只是
Host 已有文件工具的重复投影，另一部分是浅文本搜索或尚未具备真实 owner 的未来能力。模型
如果看到这 20 项，会产生工具选择噪声、错误的能力预期和不必要的 Engine 编排步骤。

本方案冻结以下方向：

1. 以权威架构冻结的 9 个模型决策控制点作为 canonical Engine tool surface。
2. 保留一个 Engine Capability Definition/Tool Registry；不建立第二个 Registry、第二个
   Provider、第二个 Host 或第二个 Agent。
3. 使用 `audience` 和 `maturity` 投影策略区分模型默认面、内部诊断面和测试面。内部实现可以
   继续存在，但未经真实 owner 资格化不得进入模型默认面。
4. 立即移除与 Codex 原生 `read_file`、`write_file`、`run_command`、`rg/search` 重叠的浅工具
   默认投影；不得通过改名把浅实现伪装成 `check` 或 `observe`。
5. 真实 `engine_project_check` 等待完整 R3 Compiler；真实 `engine_runtime_playtest` 和
   `engine_runtime_observe` 等待 R4 Semantic Outcome；真实跨 turn cancellation 等待
   Host signal、child-process owner 和 quiescence 合同完成。
6. Skill 只发布条件化使用说明、进入/跳过条件、预期结果和局部恢复提示，不执行工具、不保存
   项目状态、不替 Host AI 规划完整任务。

## 2. 权威来源与继承关系

### 2.1 上游权威

本方案服从：

- `00-AI-First-Game-Engine-权威产品需求-v1.md`；
- `00-AI-First-Game-Engine-权威架构设计-v1.md`；
- `309-F-Headless-Engine-Tool-Provider-Atomic-Cutover-v1方案.md`；
- `310-Project-Game-SDK-Generated-Runtime-Glue-v1方案.md`。

309-F 已完成的 Headless Provider、统一 Compiler/Assembler、No-Editor run/build/delivery 和
旧 Gateway 退役继续有效。本方案只修正其“模型默认工具投影”层，不重写已经完成的 owner。

### 2.2 对旧研究方案的修订

`AI-First-Game-Engine-Harness-Native-Tool-Pack-研究方案-v0.1.md` 是研究参考，不再作为默认工具
数量或模型工具菜谱的权威。其关于 typed ToolDefinition、统一执行管线、Skill catalog、MCP
兼容 Adapter、Job/cancellation 和 Native/PTC 分离的结论继续有效；其中“冻结较大现有工具集
再进行投影”的路线由本方案修订为“9 个 canonical 决策点 + Host 原生工具同级 + maturity
分阶段投影”。

## 3. 问题定义

### 3.1 当前 20 项 Provider 工具

当前实现位于 `rust/crates/engine_tool_provider/src/lib.rs`，注册 18 项 Ready 和 2 项
Unavailable：

```text
engine_project_open
engine_project_inspect
engine_project_search
engine_project_read_object
engine_project_references
engine_project_source_symbols
engine_project_diagnostics
engine_project_mutate
engine_project_rollback
engine_evidence_read
engine_ui_locate
engine_ui_explain_visibility
engine_project_trace_ui_owner
engine_operation_observe
engine_operation_cancel
engine_project_run
engine_project_build
engine_delivery_verify
engine_project_create             (Unavailable)
engine_runtime_capture_issue      (Unavailable)
```

其中 `search/references/source_symbols/ui_locate/ui_explain_visibility/trace_ui_owner` 当前共用
浅文本搜索实现；`diagnostics` 返回 qualification diagnostics，不等于 Compiler check；
`operation_cancel` 只改变 Provider 内 operation record，不等于已经停止 compiler/player/build
子进程。

### 3.2 权威目标工具

默认模型面最终收敛为 9 个决策控制点：

```text
engine_project_inspect
engine_project_check
engine_project_mutate
engine_project_rollback
engine_runtime_run
engine_runtime_playtest
engine_runtime_observe
engine_project_build
engine_delivery_verify
```

这 9 项表示模型可以据此决定“继续、修复、重试、换方案、停止”的阶段，而不是表示每项在
本方案落地时都已经拥有完整实现。

### 3.3 与 Host 原生文件工具的边界

AI 仍可像普通代码项目一样使用 Host 原生工具：

```text
read_file / write_file / apply_patch / run_command / rg
```

Engine 工具不重复提供普通文件读取、全文搜索和证据文件读取。Engine 只接管需要项目语义、
revision/CAS、Runtime 执行、Compiler 检查、构建或交付验收的动作。

## 4. 设计目标与非目标

### 4.1 目标

- AI 直接在同一个 Agent Tool Loop 中选择 Engine typed tools；不需要 Editor discovery、
  Gateway connection 或 `engine_catalog -> engine_execute` 二级调用。
- 模型工具面保持小而深，避免把内部机械阶段暴露为 AI 菜谱。
- 内部实现可以逐步成熟，模型不会提前看到未资格化能力。
- Native Host、MCP、未来 OpenCode/DeepSeek Harness Adapter 使用同一 ToolDefinition、
  canonical result 和 Grant 语义。
- ToolResult 能够给出当前事实支持的局部下一步，但不替 AI 制定目标级计划。
- 取消、权限、并发、结果投影和 Skill 生命周期具有可审计边界。

### 4.2 非目标

- 不在本方案内实现完整 R3 Compiler。
- 不在本方案内实现 R4 Semantic Outcome、视觉理解或确定性 replay。
- 不建立第二个 Tool Registry、第二个 Agent、通用 `engine_execute` 或 PTC-only 游戏开发入口。
- 不把旧浅工具改名为新能力。
- 不删除 309-F/310 的完成证据、历史实现或 Optional Editor Projection。
- 不修改 production、真实 Codex 配置、Android 工具链、Tower P1-2 或 Local CI。

## 5. 核心架构

### 5.1 单一 Registry 与多 audience 投影

```text
Engine Capability Definition
  - stable name
  - input/output schema
  - side effect / capability / risk
  - maturity
  - audience policy
  - cancellation / rollback declaration
          |
          v
Single Engine Tool Registry / Kernel
          |
          +--> Model default projection
          +--> Internal diagnostics projection
          +--> Conformance/test projection
          +--> Native/MCP/other Host adapters
```

`audience` 只影响“谁能看到这项 definition”，不产生新的执行 authority。`maturity` 描述
实现是否真实可用，不能由名称、Schema 或旧 Adapter 推断。模型默认投影至少要求：

```text
audience includes model
maturity is qualified for the current Host/target
no duplicate Host-native responsibility
real owner and result contract are present
```

### 5.2 Engine、Host AI、Skill 的职责

```text
Host AI
  用户目标、计划、工具选择、重试/分支/停止、最终判断

Skill
  阶段说明、进入/跳过条件、预期 ToolResult、局部恢复提示

Engine Provider/Kernel
  参数校验、Grant、revision/CAS、确定性执行、结构化结果、receipt/evidence

Runtime/Compiler/Build owners
  真正的检查、运行、观察、准备、装配、构建和交付语义
```

Skill 不得保存项目 revision、操作状态或 rollbackRef；ToolResult 不得生成跨任务固定
执行计划。Host 可以根据多个 ToolResult 自主选择下一步。

### 5.3 内部机械阶段隐藏

以下阶段只存在于 Engine Implementation 内部：

```text
refresh -> snapshot lease -> prepare -> generated glue -> schema/cook
-> RuntimePackage assembly -> player staging -> process verify
```

模型只看到能够改变决策的结果，不需要先调用 `refresh`、`prepare`、`assemble` 或读取内部
缓存。缓存删除后仍应能从 canonical source 和既有 owner 重建相同结果。

## 6. 20 项到 canonical surface 的迁移矩阵

| 当前定义 | canonical 归属 | 模型默认面决策 | 说明 |
|---|---|---|---|
| `engine_project_open` | Host/workspace project binding | 移除 | 日常绑定由 cwd/workspace/Host context 提供；显式 rebind 仅作内部/受控 Adapter 能力 |
| `engine_project_inspect` | `engine_project_inspect` | 保留 | 返回项目事实、identity 和 revision |
| `engine_project_search` | Host `rg/search` | 移除 | 不重复全文搜索 |
| `engine_project_read_object` | Host `read_file` | 移除 | 普通文本对象读取不需要 Engine 投影 |
| `engine_project_references` | Host search/LSP；未来语义索引 | 移除 | 当前浅文本实现不具备语义引用保证 |
| `engine_project_source_symbols` | Host LSP/search；未来语义索引 | 移除 | 当前不是稳定 symbol owner |
| `engine_project_diagnostics` | R3 Compiler diagnostics | 不改名直接暴露 | 当前不足以称为 `check` |
| `engine_project_mutate` | `engine_project_mutate` | 保留 | 结构化 mutation、CAS、receipt |
| `engine_project_rollback` | `engine_project_rollback` | 保留 | 精确 rollback 和 drift 检查 |
| `engine_evidence_read` | Host `read_file` | 移除 | evidence ref 返回路径，内容由 Host 按需读取 |
| `engine_ui_locate` | R4 `engine_runtime_observe` typed query | 暂时移除 | 当前浅文本搜索不能冒充语义观察 |
| `engine_ui_explain_visibility` | R4 `engine_runtime_observe` typed query | 暂时移除 | 等待真实 Runtime/AUI observation owner |
| `engine_project_trace_ui_owner` | R4 observation/source mapping | 暂时移除 | 等待 source mapping 和因果回溯 |
| `engine_operation_observe` | 动态异步 Job lifecycle | 默认移除 | 仅在真实跨 turn Job 存在时按能力动态提供 |
| `engine_operation_cancel` | Host signal + Job cancellation | 默认移除 | 当前 record 标记不等于进程终止 |
| `engine_project_run` | `engine_runtime_run` | 版本化收敛 | 不能永久依赖两个公共名字；迁移时保留内部 Adapter，不以改名伪造新语义 |
| `engine_project_build` | `engine_project_build` | 保留 | 构建决策点 |
| `engine_delivery_verify` | `engine_delivery_verify` | 保留 | 交付验收决策点 |
| `engine_project_create` (Unavailable) | CLI/template | 不纳入 9 项 | 创建入口不是运行期 Engine decision point |
| `engine_runtime_capture_issue` (Unavailable) | R4 `engine_runtime_observe` typed request | 暂不提供 | 不建立独立工具，不提前承诺 capture owner |

### 6.1 当前默认可用集合

在 R3/R4 尚未资格化前，模型只应看到已经有真实 owner 的子集，例如：

```text
engine_project_inspect
engine_project_mutate
engine_project_rollback
engine_runtime_run       (版本化名称收敛前由现有实现提供等价语义)
engine_project_build
engine_delivery_verify
```

`engine_project_check`、`engine_runtime_playtest`、`engine_runtime_observe` 的 canonical
identity 可以提前冻结，但只有对应 owner、schema、ToolResult 和 conformance 证据齐全后才
进入 `model` audience。不得发布返回“暂未实现”的 Ready 空壳。

## 7. ToolResult 合同与局部流转

### 7.1 Canonical 结果

Engine 工具返回结构化 canonical result，至少包含：

```text
schemaVersion
callId
toolName
status
operationId
projectIdentity / projectRevision (when project-bound)
output (typed)
diagnostics[]
receiptRef / rollbackRef (when applicable)
evidenceRefs[]
retryability
recommendedLocalTransitions[]
```

结果的结构化值是正确性真相；文本渲染、Editor panel card 和 Host 展示是可替换投影，不能
再从展示文本反解析 revision、receipt 或 evidence。

### 7.2 局部下一步

`recommendedLocalTransitions` 只能表达当前调用事实支持的有限转移，例如：

```text
check failed -> inspect the reported source path and edit it
run completed -> observe the requested runtime state
build completed -> verify the returned deliveryRef
mutation rejected by revision drift -> re-inspect before retry
```

它不能表达“为了完成打飞机游戏必须依次调用 17 个工具”，也不能替 Host AI 决定完整项目
计划。

### 7.3 Native、MCP 和未来 Harness Adapter 等价性

所有 Adapter 必须投影同一份：

- input schema 和 validation；
- side effect、permission 和 Engine Grant；
- canonical output、diagnostics、receipt/evidence；
- cancellation 和 terminal-state 语义。

MCP 只是兼容 transport。不得增加 Editor connection、二级 catalog/execute 或独立 revision
authority。

## 8. Skill 工作流设计

### 8.1 Skill 内容

Skill 以版本化知识产物发布，包含：

- 适用任务和前置条件；
- 哪些 Engine tools 已在当前 maturity 可用；
- 进入/跳过条件；
- 预期 ToolResult 字段和典型诊断；
- 失败后的局部修复方向；
- 何时需要 Host 原生文件工具；
- 何时必须使用结构化 mutation、rollback 或 delivery verify。

### 8.2 Skill 不拥有的内容

Skill 不得：

- 执行 Engine tool；
- 保存或修改项目状态；
- 持有长期 lease、operation、receipt 或 Grant；
- 把用户目标展开成不可跳过的固定计划；
- 用 prose 结果替代 canonical ToolResult；
- 把尚未成熟的工具写成 Ready。

### 8.3 Skill catalog

Skill catalog 可在 Agent step 开始时提供 name、description、provider、scope 和 digest；完整
Skill 内容按需加载。catalog 不等于执行许可，也不包含项目状态。catalog 不完整时保留上一份
last-good catalog，不向模型发布半套工作流说明。

## 9. Cancellation、Job 与 Host 边界

### 9.1 当前缺口

当前 Rust `HostToolCall` 没有 `AbortSignal`/cancel token；`NativeEngineToolProvider::cancel`
可以记录 `Cancelled`，但不能证明同步执行中的 child process 已停止。因此当前
`supports_cancellation = side_effect == ProcessSpawn` 只能视为不充分的 metadata，不能作为
真实能力资格。

### 9.2 目标合同

真实长任务需要：

```text
Host call signal
  -> Host Adapter
  -> Provider/Kernel
  -> Compiler / Runtime player / Build worker
  -> child process termination
  -> join and owned-resource cleanup
  -> one terminal result
```

取消必须区分：

- call 尚未发布 Job：没有 Job 可取消；
- Job 已发布：返回稳定 Job identity，并由 Job owner 负责取消和清理；
- 已到 terminal：返回 `already_terminal`；
- 不属于当前 session：返回 `not_found`。

### 9.3 工具面规则

在真实 cancellation owner 完成前：

- 不把同步 `engine_operation_cancel` 暴露给模型默认面；
- 不把 operation record 的状态改变描述成进程已终止；
- 不使用长期 operation 工具掩盖同步 Provider 的生命周期缺口；
- timeout、abort、dispose 必须等待 owned work quiescence，不能粗暴 abandon promise。

## 10. 迁移阶段

### Phase 0：冻结投影规则（本方案范围）

- 建立 canonical 9 项名称和定义状态表；
- 增加 audience/maturity/projection 的设计语义；
- 固定 20 项迁移矩阵；
- 固定 Host 原生工具和 Engine 工具边界；
- 固定 Skill、ToolResult 和 cancellation 的非伪装规则。

### Phase 1：立即收敛默认模型面

- 隐藏 search/read/evidence/references/source_symbols；
- 隐藏日常 project open；
- 隐藏浅 UI 查询；
- 隐藏同步 operation observe/cancel；
- 保留真实 inspect/mutate/rollback/run/build/verify；
- 对外报告“当前 canonical subset”，不宣称 9 项全部 ready。

### Phase 2：R3 Compiler 接入

只有在完整 R3 owner 证明后，才发布 `engine_project_check`：

- schema/source/asset diagnostics；
- prepare 输入和 revision 一致性；
- 增量依赖图和 cache identity；
- no-Editor prepare/run；
- Compiler 级 cancellation 或明确的同步 bounded contract。

### Phase 3：R4 Semantic Outcome 接入

只有在完整 R4 owner 证明后，才发布 `engine_runtime_playtest` 和
`engine_runtime_observe`：

- 输入时间线和 deterministic/replay 语义；
- 实体、事件、AUI、source mapping 查询；
- Technical/Gameplay/Visual/Delivery Outcome；
- capture 作为 observe typed request；
- packaged Player 复验和 evidence lineage。

### Phase 4：真实 cancellation owner 接入

- Host signal 到 Provider/worker/process 的传播；
- Windows/其它平台 child process 终止策略；
- join/reaper/quiescence；
- Job observe/cancel 只在跨 turn 异步能力真实存在时动态投影。

### Phase 5：跨 Host conformance

在 Codex、MCP、OpenCode 或 DeepSeek Harness Adapter 中验证同一 Engine definition 的输入、
Grant、结果、错误、取消和权限语义。宿主差异只存在于注册、审批 UI 和 transport。

## 11. 成熟项目借鉴边界

### 11.1 Codex

当前可验证的 Codex 使用方式是原生文件和命令工具与 Engine typed tools 处于同一个 Agent
Tool Loop。Engine 应直接作为同级具体工具注册，不要求 AI 先连接 Editor 或调用通用
`engine_execute`。OpenAI Codex 官方页面在本环境返回 403，因此本方案不对未验证的内部
实现细节作额外断言。

### 11.2 OpenCode

本地参考源码：

```text
框架设计/OpenCode源码参考/opencode/packages/opencode/src/tool/registry.ts
框架设计/OpenCode源码参考/opencode/packages/opencode/src/tool/tool.ts
框架设计/OpenCode源码参考/opencode/packages/opencode/src/tool/skill.ts
框架设计/OpenCode源码参考/opencode/packages/opencode/src/util/process.ts
```

可采纳：统一 ToolRegistry、按 Agent/模型/权限投影工具、Skill 作为按需加载说明、
AbortSignal 传入工具和真实子进程终止。

不可照搬：OpenCode 的通用文件工具不能替代 Engine Semantic Outcome；其 `grep/read` 不能
包装成 Engine 深能力。

### 11.3 DeepSeek Harness

本地参考源码：

```text
框架设计/DeepSeekHarness源码参考/deepseek-harness/packages/core/tools/src/index.ts
框架设计/DeepSeekHarness源码参考/deepseek-harness/packages/core/tools/src/ptc.ts
框架设计/DeepSeekHarness源码参考/deepseek-harness/packages/skill/tool-skill/src/index.ts
框架设计/DeepSeekHarness源码参考/deepseek-harness/packages/mcp/mcp-client/src/tools.ts
```

可采纳：canonical output schema、pre/execute/post/result 管线、不可变参数与结果、并发安全
分类、Skill catalog/digest/按需加载、MCP 工具集原子换代和 signal/quiescence 合同。

不可照搬：PTC/`run_code` 不能成为新的 `engine_execute`；Harness workflow 不得夺取 Host AI
的目标规划权；同进程代码不能因为有 AbortSignal 就被错误宣称可以硬终止。

## 12. 资格 Gate

本方案本身不授权施工；后续施工文档至少必须覆盖：

### G0：Authority consistency

- 本方案与权威架构、309-F、310 无冲突；
- 旧 v0.1 仅作参考；
- 20→9 矩阵和当前代码基线一致。

### G1：Single Registry projection

- 一个 Registry 产生模型、内部和测试投影；
- 没有第二 Provider、第二 catalog/execute 或第二 project authority；
- audience/maturity 不能绕过 execution-time validation。

### G2：Host overlap removal

- 默认面不再包含普通 search/read/evidence 和浅 UI 查询；
- 项目 binding 不要求日常 `project_open`；
- Engine ToolResult 返回 evidence ref，Host 可自行读取证据文件。

### G3：No false capability

- `diagnostics` 未被改名伪装成 `check`；
- 浅搜索未被改名伪装成 `observe`；
- record cancel 未被报告为 child process terminated；
- 未资格化工具不会以 Ready 出现在模型默认面。

### G4：Skill/ToolResult boundary

- Skill 无执行、状态和计划 authority；
- canonical result、展示文本和 Editor panel projection 分离；
- recommendedLocalTransitions 只提供局部事实转移。

### G5：R3/R4/cancellation readiness

- R3 前不发布真实 `check`；
- R4 前不发布真实 `playtest/observe/capture`；
- cancellation owner 未完成前不发布默认 operation cancel。

### G6：Host conformance

- Native/MCP/未来 Host Adapter 对同一工具的 schema、Grant、result、错误和终态语义一致；
- 只允许注册和 transport 差异；
- 不运行真实 production Codex 配置作为普通源码 Gate，真实 Host 验收另行授权。

## 13. 验收结果声明格式

后续实现不得只写“工具面已完成”，必须分别声明：

```text
canonical surface frozen: yes/no
model projection subset: <qualified tool names>
internal diagnostics projection: <names and audience>
R3 check qualified: yes/no
R4 playtest qualified: yes/no
R4 observe qualified: yes/no
real cancellation qualified: yes/no
Host conformance: <adapter and evidence>
```

只有对应 owner、schema、测试和真实证据齐全，才能将某项从 internal/deferred 提升为 model
qualified。工具名称数量本身不能作为完成度指标。

## 14. 最终不变量

```text
INV-1  一个 Engine Capability Definition/Tool Registry 是唯一工具合同 authority。
INV-2  模型看到的是具体 typed decision tools，不是 catalog/execute 二级 API。
INV-3  Host 原生文件工具和 Engine 工具同级，但职责不重复。
INV-4  9 项 canonical identity 与 9 项已实现能力严格区分。
INV-5  Skill 只提供条件化知识和局部工作流提示，不执行、不持状态、不规划。
INV-6  ToolResult 的结构化事实优先于文本、截图或 Editor card。
INV-7  不得以改名、浅代理或空壳 Schema 伪造 R3/R4/cancellation 能力。
INV-8  cancellation 只有在 signal 传播、终止、join 和 quiescence 具备后才可声明真实。
INV-9  Provider 不是第二个 Agent；Host AI 保留目标理解、选择和最终判断。
INV-10  309-F/310 已完成的 Runtime、Compiler subset、SDK 和交付 owner 继续复用。
```

## 15. 实施结果

2026-09-03 已按方案 B 完成 Gate A-C：

```text
canonical surface frozen: yes
model projection subset: engine_project_inspect, engine_project_mutate, engine_project_rollback,
                         engine_runtime_run, engine_project_build, engine_delivery_verify
internal diagnostics projection: 其余既有 definitions 仅保留内部诊断/测试 audience
R3 check qualified: no
R4 playtest qualified: no
R4 observe qualified: no
real cancellation qualified: no
Host conformance: NativeHostAdapter 与 McpHostAdapter definitions、schema、metadata、拒绝语义和
                  canonical result 等价；No-Editor MCP process smoke passed
```

ToolResult 已单线升级为 `engine-tool-result.v2`，只增加事实驱动的 `retryability` 与
`recommendedLocalTransitions`；仓库新增一份无执行、无状态、无规划 authority 的最小 Engine Skill。
既有 process smoke 已移除模型默认面不再暴露的 `engine_project_open/read_object`，改由启动参数绑定
disposable project、Host 文件写入模拟普通源码修改，再完成 `inspect -> runtime_run -> build ->
delivery_verify`。本次没有修改 production、真实 Codex 配置、Android、Local CI 或 Tower P1-2。

完成记录见：
`阶段完成记录/2026-09-03-309-F-Default-Engine-Tool-Surface-Skill-Workflow-Convergence-v1/00-总览.md`。

## 16. 方案结论

采用方案 B，吸收方案 C 的内部诊断投影，但不建立第二 Registry。短期先收敛模型默认面，
确保 AI 只看到真实且有独立决策价值的 Engine 工具；中期以 R3 Compiler 提供 `check`，以
R4 Semantic Outcome 提供 `playtest/observe`；并行补齐真实 Host cancellation。这样既保持
“AI 像使用 Codex 原生工具一样直接写游戏”的体验，又保留自研引擎在结构化项目 mutation、
Native Runtime、跨平台构建、交付验证和可回滚证据上的长期优势。

本文件是已经完成并归档的正式方案。后续 R3、R4、真实 cancellation、production 安装或真实 Codex
验收仍需要独立方案、施工文档与明确授权，不得从本次完成状态自动启动。
