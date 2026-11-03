# 309-F Default Engine Tool Surface + Skill Workflow Convergence v1 施工文档

> 文档性质：已确认方案 B 的当前施工文档。  
> 当前状态：Gate A-C 已于 2026-09-03 完成并通过；终态同步完成，已归档。  
> 正式方案：`../../309-F-Default-Engine-Tool-Surface-Skill-Workflow-Convergence-v1方案.md`。  
> 正式方案 SHA-256：`2890A22F1C3385B81D4143712C91E37632111F7E851DE294F3CC422AE50960F3`（终态方案）。  
> 生成基线：branch `codex/255-capability-catalog`，HEAD `acaffa8cb95bbb54d093bbf45835a59315c82da5`。  
> 生成日期：2026-09-03。  
> 前置状态：309-A 至 309-F 与 310 已完成归档；激活前施工槽为空，现由本文唯一占用；Tower P1-2 为 `stopped_by_user`。  
> 激活状态：Gate A-C 已完成；本文终态移入 `已完成/`，当前施工槽清空。

## 1. 施工目标与允许声明

本文只施工正式方案中不依赖完整 R3/R4 的立即收敛范围：

1. 继续使用现有一个 Engine Capability Definition/Tool Registry，不新建第二 Registry；
2. 把模型默认工具面从当前 20 项定义收敛为已有真实 owner 的 6 项 canonical 子集；
3. 将现有真实 run 能力从 `engine_project_run` 原子切换为 `engine_runtime_run`，不长期双注册；
4. 隐藏 Host 原生文件工具已经覆盖的 read/search/evidence、浅 UI 查询、日常 project open、
   同步 operation observe/cancel 和 unavailable future definitions；
5. 保持内部诊断/测试可按显式非模型 audience 检查旧实现，但模型 Adapter 不能列出或调用它们；
6. 用最小 ToolResult v2 增加 retryability 和由当前事实支持的局部 transition，不加入目标级 Planner；
7. 增加一份最小、非执行型 Engine authoring Skill，说明 Host 文件工具与 Engine 工具的边界、
   当前 qualified subset 和失败恢复；不实现完整 AI Development Pack 或 Skill catalog runtime；
8. 通过 Native/MCP 同源投影、调用拒绝和 No-Editor process smoke 证明收敛结果。

全部 Gate 通过后最多允许声明：

```text
单一 Engine Tool Registry 可按 audience/maturity 生成模型默认投影；
当前模型默认 Engine tool surface 为 inspect/mutate/rollback/runtime_run/build/delivery_verify 六项；
旧 read/search/evidence/open/UI/operation/unavailable definitions 不再对模型可见或可调用；
engine_runtime_run 使用原 engine_project_run 的真实 owner，不存在公共双名字；
ToolResult v2 能表达紧凑 retryability 和局部合法转移；
一份最小 Skill 说明条件化工作流，但不执行工具、不保存状态、不替 AI 规划；
Native 与 MCP 对模型工具面、调用结果和隐藏工具拒绝语义等价；
完整 R3 check、R4 playtest/observe 和真实 cancellation 仍为 deferred。
```

不得声明 9 项 canonical 工具全部 Ready、完整 R3/R4、完整 AI Development Pack、跨 Host 原生
Provider、真实 cancellation、production 安装态、真实 Codex 配置或“已经超过 Unity/UE/Godot”完成。

## 2. 必要性与验证经济性审计

```text
requested claim：按已确认方案 B 收敛默认 Engine 工具面和 Skill 工作流，且不要过量施工
confirmed gap：Provider 当前向 Native/MCP 投影 20 项；多项重复 Host 文件能力或共用浅文本搜索；run 名称未收敛；process tools 错误声明 supportsCancellation；当前没有产品侧最小 Engine authoring Skill
minimum causal change：在现有 ToolDefinition/Registry 内增加 audience 投影；Native/MCP list/call共用该投影；原子改名真实 run；最小扩展 ToolResult；新增一份静态 Skill
必须修改：engine_tool_provider definition/projection/invocation、MCP adapter、直接受影响测试、一份最小 Skill、完成时入口文档
明确排除：完整 R3 Compiler、R4 Semantic Outcome、第二 Registry/Provider、完整 Skill catalog/digest runtime、真实 cancellation、production、真实配置、Android、Local CI、Tower P1-2
复用证据：309-F Headless Provider、Native/MCP equivalence、No-Editor MCP process smoke；310 Compiler/SDK integration；现有 mutation/rollback/run/build/verify owner tests
证据失效条件：正式方案 hash变化；ToolDefinition/ToolResult schema变化；Native/MCP projection或current run/build owner变化；Skill位置/格式约定变化
预计成本：Gate A 45-60分钟，Gate B 45-60分钟，Gate C 30-45分钟；总计约2-2.75小时，不含异常冷编译等待
总停止条件：需要实现check/playtest/observe、重写Compiler/Runtime/Build、引入第二registry、修改真实Host配置、实现跨进程Job系统或扩展到production/Android时立即停止复核
```

| Gate | 风险 / 失败合同 | 最便宜可打红测试 | 终态证据 | 预计成本与停止点 |
| --- | --- | --- | --- | --- |
| A | 只是删名字、形成第二列表真相，或隐藏工具仍能经 Adapter 调用 | Provider projection owner tests + MCP list/call tests | 模型面精确 6 项、内部定义仍单源、旧名字不可调用 | 45-60 分钟；若需第二 Registry 或兼容 daemon，停止 |
| B | ToolResult 变成 Planner，Skill 执行工具/持状态，或为 catalog 新建 runtime | ToolResult contract tests + Skill 静态边界测试 | v2 局部 transition 有界；单一静态 Skill 无执行代码 | 45-60 分钟；若需完整 AI Development Pack/Host plugin，停止 |
| C | Native/MCP 投影不一致、run 改名破坏真实 No-Editor 路径 | adapter equivalence + 一次既有 MCP process smoke | 同一 6 项列表、同一 canonical result、run/build/verify 通过 | 30-45 分钟；若需真实 Codex/production/Local CI，停止 |

验证经济规则：Gate C 的完整 crate 或 process smoke 如果在相同输入下已经覆盖 Gate A/B 的某个终态测试，
不重复执行同一命令；开发时只跑具体打红测试。冷编译超时不等于产品失败，先保留输出并判断是否继续。
不得新增施工 Runner、第二 fixture 项目、完整 workspace 矩阵或真实 Host 验收来装饰源码级结论。

## 3. 冻结实现边界

### 3.1 一个 Definition source，一个执行 Kernel

允许在现有 `ToolDefinition` 或其同文件紧邻类型中增加最小 audience 元数据和纯过滤函数：

```text
all capability definitions
  -> project_for(ModelDefault)
  -> project_for(InternalDiagnostics)
  -> project_for(ConformanceTest)
```

这三个是同一 Registry 的只读投影，不是三个 Registry。不得复制三份名称数组、schema 或 handler map。
调用授权必须与 `tools/list` 使用同一投影判断，避免“列表隐藏但仍可直接调用”。

若增加序列化字段会改变公开 `engine-tool-definition.v1` 合同，则按实际兼容性选择以下最小路径之一，
并在 Gate A 测试中固定：

- audience 作为 Provider 内部非 wire 元数据，不改变 v1；或
- 确认外部 consumer 实际消费后，一次升级 definition schema，并同步唯一相关 consumer。

不得仅为代码美观创建新 crate、registry service、配置文件或 schema generator。

### 3.2 当前模型默认 6 项

本施工只发布当前已有真实 owner 的 6 项：

```text
engine_project_inspect
engine_project_mutate
engine_project_rollback
engine_runtime_run
engine_project_build
engine_delivery_verify
```

`engine_runtime_run` 必须直接复用现有 `engine_project_run` handler、Compiler、Runtime 和 evidence
lineage；施工采用源码级原子改名，不同时公开两个名字，不增加 deprecated alias。

下列能力只从模型默认投影移除，不要求本施工删除其全部内部实现：

```text
engine_project_open
engine_project_search
engine_project_read_object
engine_project_references
engine_project_source_symbols
engine_project_diagnostics
engine_evidence_read
engine_ui_locate
engine_ui_explain_visibility
engine_project_trace_ui_owner
engine_operation_observe
engine_operation_cancel
engine_project_create
engine_runtime_capture_issue
```

内部保留必须有现有诊断、测试或迁移用途；否则可在不扩大 consumer 范围的情况下删除死实现。不得把
内部保留重新包装成模型可见浅代理。

### 3.3 明确等待的 3 项

以下名称可以在文档与 Skill 中作为 canonical future capability 出现，但不能注册 Ready 空壳：

```text
engine_project_check          -> 等待完整 R3 Compiler
engine_runtime_playtest       -> 等待 R4 Semantic Outcome
engine_runtime_observe        -> 等待 R4 Semantic Outcome
```

当前 `engine_project_diagnostics` 不得改名为 `check`；三个共用文本搜索实现的 UI 工具不得改名或
聚合成 `observe`；`engine_runtime_capture_issue` 不得作为独立 future tool 提升。

### 3.4 ToolResult v2 最小边界

允许在现有 canonical result 上增加：

```text
retryability
recommendedLocalTransitions[]
```

每个 transition 只允许包含稳定 category、可选具体 tool name 和由当前结果支持的 reason。只为以下
确定情形产生 transition：

- revision drift / stale input -> 重新 inspect；
- 参数或源码可修复诊断 -> 修改明确输入后再调用；
- build completed 且返回 deliveryRef -> delivery verify；
- rollback receipt 可用 -> 在用户/AI决定撤销时 rollback。

不得输出跨目标计划、固定工具菜谱、自然语言 Goal 分解或强制下一步。若现有
`diagnostics[].nextAction` 已足以表达某个失败，只从同一事实派生 typed transition，不创建第二诊断真相。
这是 canonical result 的真实 additive contract，应一次升级 `ENGINE_TOOL_RESULT_SCHEMA_VERSION`，不保留
双写 v1/v2 pipeline。

### 3.5 Skill 最小边界

允许新增一份产品侧、宿主可发现的静态 Skill：

```text
ai-development-pack/skills/ai-first-game-engine/SKILL.md
```

若激活前复核发现仓库已经冻结其它 AI Development Pack 根目录，必须使用该既有根，不再创建并行目录。
Skill 只包含：

- 何时使用 Host 原生文件工具；
- 何时使用 6 项当前 qualified Engine tools；
- R3/R4 三项仍不可用；
- 读取 ToolResult、diagnostics、receipt/evidence 和局部 transition 的方式；
- revision drift、mutation rollback、build verify 的条件化恢复。

不新增 Skill loader、catalog service、digest daemon、tool wrapper、脚本或状态文件。Skill 不执行 Engine
tool，不保存 call/revision/receipt，不声称自己是 Agent。若 Host 不能直接发现该目录，只记录后续
AI Development Pack 安装/发布缺口，不在本施工修改真实 Codex/OpenCode 配置。

### 3.6 Cancellation 边界

本施工不实现真实 cancellation。当前 model projection 中所有 process tool 的
`supports_cancellation` 必须为 `false`；`engine_operation_cancel` 不进入模型列表，也不能经 Native/MCP
模型 Adapter 直接调用。

已有内部 operation record 可保留给测试/诊断，但不得宣称已停止 compiler/player/build 子进程。Host signal、
process termination、join/reaper 和 quiescence 等待后续独立 cancellation owner 方案。

## 4. 允许文件与禁止范围

### 4.1 允许新增

```text
ai-development-pack/skills/ai-first-game-engine/SKILL.md
rust/crates/engine_tool_provider/tests/skill_workflow_contract.rs（只有现有测试无法表达Skill边界时）
```

### 4.2 允许修改

```text
rust/crates/engine_tool_provider/src/lib.rs
rust/crates/engine_tool_provider/src/mcp_stdio.rs
rust/crates/engine_tool_provider/tests/host_adapter_equivalence.rs
rust/crates/engine_tool_provider/tests/headless_execution_owner.rs
rust/crates/engine_tool_provider/tests/mcp_process_smoke.rs
rust/crates/engine_tool_provider/tests/isolated_codex_config.rs（仅静态tool name expectation确有变化时）
直接断言engine_project_run或20项模型列表的同crate测试
rust/crates/project_e2e_gate/src/tests.rs（仅更新原子改名直接影响的一处Provider工具名资格断言）
框架设计/引擎总体架构/309-F-Default-Engine-Tool-Surface-Skill-Workflow-Convergence-v1方案.md（仅施工发现正式边界错误时先修订）
README.md、00-文档地图.md、49、54、施工文档README、阶段完成记录README（只在完成归档时同步真实状态）
```

上述清单是边界，不是批量修改授权。没有打红证据的文件保持不动。原则上不修改 Cargo dependency；若新增
静态 Skill test 可用标准库完成，不为文档解析引入新依赖。

### 4.3 明确禁止

```text
不修改 project_authoring_execution、Project Game SDK、Runtime、Renderer、Editor 或 Build owner
不实现 engine_project_check、engine_runtime_playtest、engine_runtime_observe
不把 diagnostics/UI search/capture issue 改名或代理成上述能力
不新建第二 Registry、Provider、Host、Agent、catalog/execute、daemon、Job runtime或workflow engine
不实现 AbortSignal、跨进程cancel、process kill、join/reaper；只修正当前虚假model metadata
不实现完整 AI Development Pack、Skill catalog/digest/安装器、模板或示例矩阵
不修改真实 Codex/OpenCode/DeepSeek 配置，不安装或替换 Provider
不运行真实 Codex task、production binary、Editor窗口、Local CI、完整workspace矩阵
不运行或修改 Android/Gradle/NDK/APK/ADB/Emulator/设备工具链
不恢复或修改 Tower P1-2，不修改三个样例 gameplay/visual 内容
不清理、重置、提交或覆盖工作区中既有用户变更
```

## 5. Gate A：Single Registry Default Surface Cutover

### 5.1 施工任务

1. 激活前确认本文是 `当前/` 唯一施工文档，重算方案 hash，并核对当前 Provider 仍是 20 项基线。
2. 先增加打红测试，证明 Native/MCP 模型默认列表必须精确等于 6 项且顺序稳定。
3. 在同一 ToolDefinition source 中增加最小 audience projection；内部 diagnostics/test 可见性不得复制 schema。
4. 让 NativeHostAdapter 与 McpHostAdapter 的 list 和 call authorization 共用 ModelDefault 投影。
5. 将 `engine_project_run` 原子改为 `engine_runtime_run`，复用原 handler/schema/result/evidence，不保留公共 alias。
6. 模型 Adapter 对隐藏或 future 工具返回稳定 unknown/not-exposed 结果，不允许绕过列表直接调用。
7. 将当前 process tools 的 cancellation metadata 修正为 false；保留真实 side-effect/risk/approval 语义。
8. 更新同 crate 中只因名称/列表改变而失效的测试，不改其它 owner。

### 5.2 最小测试

在 `G:\gameEngin\rust` 执行：

```powershell
cargo test -p engine_tool_provider --lib --locked model_default_projection
cargo test -p engine_tool_provider --lib --locked hidden_tool
cargo test -p engine_tool_provider --test host_adapter_equivalence --locked
```

测试名可按现有模块风格调整，但每个过滤命令必须实际执行非零测试；否则修正过滤条件，不得把 `0 tests`
当作通过。开发循环只跑首次打红测试；Gate 终态执行上述各命令一次。

### 5.3 完成条件

- 只有一个 definitions source 和一个 dispatch Kernel。
- Native/MCP 默认列表精确 6 项，无 open/search/read/evidence/UI/operation/unavailable 工具。
- 隐藏工具不可经模型 Adapter 直接调用。
- 真实 run 只有 `engine_runtime_run` 一个公共名字，行为与 evidence lineage 未改变。
- 当前工具不再虚假声明支持 cancellation。

### 5.4 2026-09-03 Gate A 施工结果

```text
状态：passed；Gate B-C 未开始
单一真相：EngineToolProvider 继续保留 20 项内部 definitions 和唯一 dispatch Kernel；没有新增 Registry、Provider 或兼容 alias
模型默认面：engine_project_inspect、engine_project_mutate、engine_project_rollback、engine_runtime_run、engine_project_build、engine_delivery_verify（顺序冻结）
原子改名：engine_project_run 已从 Provider 定义、dispatch、同 crate 测试和一处直接 E2E 资格断言移除；旧名称只保留在否定测试中
Adapter 授权：NativeHostAdapter 与 McpHostAdapter 共用 ModelDefault 投影；已注册但隐藏的工具返回 engine_provider.tool_not_exposed，未知工具仍返回 unknown
cancellation：全部当前 definitions 的 supportsCancellation=false；side-effect、risk、approval、rollback 和 evidence 语义未改变
外部状态：未修改 production、真实 Codex 配置、Android 或 Tower P1-2；未启动 Editor
```

红灯证据：

```text
cargo test -p engine_tool_provider --lib --locked model_default_projection
FAILED：模型默认列表实际为 20 项而非冻结的 6 项

cargo test -p engine_tool_provider --lib --locked hidden_tool
FAILED：engine_project_diagnostics 经 Native 模型 Adapter 返回 Completed
```

终态验证：

```text
cargo test -p engine_tool_provider --lib --locked model_default_projection
passed：2 passed，0 failed，18 filtered out

cargo test -p engine_tool_provider --lib --locked hidden_tool
passed：2 passed，0 failed，18 filtered out

cargo test -p engine_tool_provider --test host_adapter_equivalence --locked
passed：4 passed，0 failed

cargo fmt --check -p engine_tool_provider -p project_e2e_gate
passed
```

源码身份：

```text
engine_tool_provider/src/lib.rs SHA-256 1D8B95AC628D78E7E685E73134261A0346E2DAF7645B22D09E2638558966BF3A
engine_tool_provider/src/mcp_stdio.rs SHA-256 A5B36DCE0651CC3F74C97D215BF154D4B3CA6768152E2D1AFEED51E4E100A97D
host_adapter_equivalence.rs SHA-256 06EAF3205A5E66F5B2885E84FFC5156C6DB47AA16DA3B30DDB3D1EEF78A48762
headless_execution_owner.rs SHA-256 950A07DBA51240E47FBA6C41BDBA481FA22E374BF6565FE341C209CEF17BB271
mcp_process_smoke.rs SHA-256 0542AC49CA7C01686C3668AC30A3DAD2FA4ADA125C3E8AED9C6DD3165CA552DF
```

`project_e2e_gate` 仅同步一处直接受影响的工具名断言；未运行该 E2E。真实 run/build/verify 和 MCP process
smoke 按施工边界留给 Gate C，不以 Gate A 的源码/Adapter 结果提前声明。

## 6. Gate B：ToolResult Local Flow + Minimal Skill

### 6.1 施工任务

1. 先增加 ToolResult v2 打红测试，固定 retryability 和 recommended local transition 的结构与边界。
2. 在现有 `finish`/result normalization seam 统一派生局部 transition；handler 不各自维护第二份规划逻辑。
3. revision drift、可修复诊断、build-to-verify 和 rollback-available 只按当前确定事实返回有界 transition。
4. 保持 receipt/evidence/project revision/call/operation identity 语义不变，MCP structuredContent 完整投影 v2。
5. 新增一份最小 Engine authoring Skill；内容遵守 3.5，不增加代码执行、状态或 catalog runtime。
6. 使用最小静态测试验证 Skill front matter、Host/Engine 工具职责和 deferred R3/R4 声明；不得解析自然语言
   建立复杂文档 schema。

### 6.2 最小测试

在 `G:\gameEngin\rust` 执行：

```powershell
cargo test -p engine_tool_provider --lib --locked local_transition
cargo test -p engine_tool_provider --test host_adapter_equivalence --locked
```

若新增独立 Skill contract test，再执行：

```powershell
cargo test -p engine_tool_provider --test skill_workflow_contract --locked
```

没有新增独立测试文件时不得保留第三条命令。不得为 Skill 使用 YAML/Markdown 第三方解析库；只检查最小
front matter 和冻结 token。

### 6.3 完成条件

- ToolResult 只有一个 v2 canonical pipeline，无 v1/v2 双写。
- local transition 可由当前 result 重建，不包含目标级计划或固定菜谱。
- Skill 是静态知识投影，不执行、不持状态、不拥有 Agent/Registry/Provider。
- Skill 明确当前 6 项与 R3/R4 deferred 边界，不误报 9 项 Ready。

### 6.4 2026-09-03 Gate B 施工结果

```text
状态：passed；当时 Gate C 尚未开始，现已按第 7.4 节完成并通过
ToolResult：单一 canonical pipeline 已原子升级为 engine-tool-result.v2；没有保留 v1 双写或兼容分支
新增字段：retryability、recommendedLocalTransitions[]；transition 只含稳定 category、可选 toolName 和当前事实 reason
统一派生：正常结果只在 finish seam 调用 derive_local_flow；handler 未各自维护下一步逻辑
有界事实：revision drift -> inspect_latest_revision；参数/明确源码诊断 -> retry_with_correction；build deliveryRef -> verify_delivery；mutation receipt -> rollback_available
规划边界：没有目标分解、固定工具菜谱、跨调用状态或强制下一步
Skill：新增 ai-development-pack/skills/ai-first-game-engine/SKILL.md；目录中只有静态入口文件，无 loader、script、catalog、状态或安装逻辑
deferred：engine_project_check、engine_runtime_playtest、engine_runtime_observe 和真实 cancellation 仍不可用
方案校正：正式方案概念旧名 localNextAction[] 已统一为 wire 名 recommendedLocalTransitions[]；范围和行为未改变
当前正式方案 SHA-256：AE81095551A8641A1A487C4642A897B4ACDAB9AFD267F149D0CB3750F2B3DFE4
外部状态：未修改 production、真实 Codex 配置、Android 或 Tower P1-2；未启动 Editor
```

红灯证据：

```text
cargo test -p engine_tool_provider --lib --locked local_transition
FAILED：ToolRetryability、RecommendedLocalTransition、LocalTransitionCategory 和统一 derive_local_flow 尚不存在

cargo test -p engine_tool_provider --test skill_workflow_contract --locked
FAILED：产品 Engine authoring Skill 路径不存在
```

终态验证：

```text
cargo test -p engine_tool_provider --lib --locked local_transition
passed：2 passed，0 failed，20 filtered out；包含 canonical finish 和 MCP structuredContent v2 投影

cargo test -p engine_tool_provider --test host_adapter_equivalence --locked
passed：4 passed，0 failed

cargo test -p engine_tool_provider --test skill_workflow_contract --locked
passed：1 passed，0 failed

python C:\Users\zenghaoran\.codex\skills\.system\skill-creator\scripts\quick_validate.py G:\gameEngin\ai-development-pack\skills\ai-first-game-engine
passed：Skill is valid!

cargo fmt --check -p engine_tool_provider
passed
```

源码与知识产物身份：

```text
engine_tool_provider/src/lib.rs SHA-256 E774129CE61EE08D18F0DA58B46F6DAA46B02B1712DE07473614852E2B651050
engine_tool_provider/src/mcp_stdio.rs SHA-256 F79318B378A0F3F9B71BD20C6C0F4F3BC7CA227B2AA93947907E95FDDDC021C1
host_adapter_equivalence.rs SHA-256 06EAF3205A5E66F5B2885E84FFC5156C6DB47AA16DA3B30DDB3D1EEF78A48762
skill_workflow_contract.rs SHA-256 B617E444CCF204CD126168C77E86D150BF6E5B99D9F174FB05AA387FA800D06A
ai-first-game-engine/SKILL.md SHA-256 E5E3C0D83924369F7C70CCB00122CA7755D1A4F4DA415CBAFB7F0981B583F26C
```

未运行 Gate C 的完整 crate、MCP process smoke、headless run/build/verify 或全仓格式检查；不得用 Gate B
结果提前声明 Native/MCP process 级终态资格。

## 7. Gate C：Native/MCP Conformance + No-Editor Verification

### 7.1 施工任务

1. 证明 Native/MCP 对同一 6 项 definitions、输入 schema、side effect、maturity 和 cancellation metadata 等价。
2. 证明两种 Adapter 对隐藏工具 direct call 使用等价的 not-exposed/unknown 语义。
3. 迁移既有 No-Editor MCP process smoke：不再调用 `project_open/read_object`，启动参数直接绑定 disposable
   project；使用 Host 文件写模拟普通文件修改；调用 `inspect -> runtime_run -> build -> delivery_verify`。
4. 保持 revision、artifact、delivery identity、evidence refs 和退出码的既有断言。
5. 检查仓库生产源码中没有新增 `engine_catalog`、`engine_execute`、第二 Registry 或 public run alias。
6. 只做源码/测试资格，不安装 Provider、不修改真实 MCP 配置、不启动真实 Codex task。

### 7.2 终态测试

在 `G:\gameEngin\rust` 按 fail-fast 顺序执行：

```powershell
cargo fmt --all -- --check
cargo test -p engine_tool_provider --lib --locked
cargo test -p engine_tool_provider --test host_adapter_equivalence --locked
cargo test -p engine_tool_provider --test mcp_process_smoke --locked mcp_process_smoke_runs_without_editor_or_gateway -- --exact
cargo check -p engine_tool_provider --locked
```

如果 `cargo fmt --all -- --check` 只因本施工范围外既有用户文件失败，记录具体文件，并仅对本次修改 Rust 文件
运行 `rustfmt --check`；不得格式化或修复无关文件。后续命令已经在相同配置完整覆盖某个较窄命令时，不重复
运行被覆盖命令。

### 7.3 完成条件

- Native/MCP model surface 和 canonical result 等价。
- disposable project 在无 Editor/Gateway 情况下完成 inspect/run/build/verify。
- 工具改名和投影没有改变 revision/artifact/delivery/evidence lineage。
- 没有 production、真实 Host、Android、Local CI 或样例玩法施工。

### 7.4 2026-09-03 Gate C 施工结果

```text
状态：passed；Gate A-C 全部完成
唯一测试迁移：mcp_process_smoke 删除模型默认面不再暴露的 engine_project_open/read_object 调用
项目绑定：MCP process 继续通过 --workspace-root/--project-root 直接绑定 disposable project
Host 文件修改：测试在首次 inspect 后直接改写 Scene 文件，第二次 inspect 观察到新 projectRevision
Agent Tool Loop：inspect -> runtime_run -> build -> delivery_verify 在 Editor/Gateway 未启动时通过
身份合同：run/build/verify projectRevision 相同；run/build/verify artifactIdentity 相同；build/verify deliveryIdentity 相同
交付合同：verify status=passed，processExitCode=0，childPlayerExitCode=0，三阶段 evidenceRefs 均指向实际文件
静态合同：没有 engine_catalog、engine_execute、第二 Registry 或 public engine_project_run alias
模型面：Native/MCP 共用同一 6 项 ModelDefault definitions；隐藏 direct call 等价返回 engine_provider.tool_not_exposed
外部状态：未修改 production、真实 Codex 配置、Android 或 Tower P1-2；未启动 Editor；未运行 Local CI
```

首次 process smoke 按旧测试流程调用 `engine_project_open`，被正确拒绝并形成有效红灯：

```text
engine_provider.tool_not_exposed
0 passed，1 failed
```

只修改既有 `tests/mcp_process_smoke.rs` 后终态验证：

```text
cargo fmt --all -- --check
passed

cargo test -p engine_tool_provider --lib --locked
22 passed，0 failed

cargo test -p engine_tool_provider --test host_adapter_equivalence --locked
4 passed，0 failed

cargo test -p engine_tool_provider --test mcp_process_smoke --locked mcp_process_smoke_runs_without_editor_or_gateway -- --exact
1 passed，0 failed；77.76s

cargo check -p engine_tool_provider --locked
passed；仅有既存 engine_runtime dead_code warnings

scoped git diff --check
passed
```

Gate C 只修改测试流程，未修改 lib/Adapter 输入身份，因此首次失败前已通过的 lib 22 项和 Adapter 4 项证据继续有效，
未机械重跑。`mcp_process_smoke.rs` 终态 SHA-256 为
`2262806B675EC55F5733965CB6AAC8C549E2DA2AB53E04011920CDE563E8A9DA`。

## 8. 激活前复核

从 `待执行/` 移入 `当前/` 前必须重新确认：

1. 正式方案路径和 SHA-256 仍匹配；若变化先修订本文并重新自审。
2. `当前/` 没有其它非 README 施工文档；遗留证据文件不计作施工文档，但不得删除。
3. Provider definitions、MCP adapter 和测试仍与本文 20 项基线一致。
4. 仓库没有新增正式 AI Development Pack 根；若已经存在，只调整 Skill 路径，不建立并行根。
5. `engine_project_run` 尚未由其它工作完成 canonical rename；若已变化，缩减 Gate A，不重复迁移。
6. 当前工作树大量既有用户变更仍需保留；不得 reset、checkout 或清理无关文件。
7. 不需要修改 R3/R4、Compiler/Runtime/Build、production、真实配置、Android 或 Tower P1-2。
8. 三 Gate 的过滤测试在当前代码中可命中非零测试，预计连续授权可在三小时内完成；否则先缩小或拆分，
   不在施工中临时扩建 Runner。

复核通过后才可：

```text
移动本文到 施工文档/当前/
更新 54 和施工文档 README 为唯一当前施工
记录激活时 branch/HEAD/方案 hash/允许范围
开始 Gate A
```

### 8.1 2026-09-03 激活复核结果

```text
复核结论：通过；本文可作为 当前/ 唯一施工文档，Gate A 尚未开始
正式方案 SHA-256：4D417294D14BD99E5B960E9350F578345A983CA4C99A0F6D50BF28F3EE0AE1D4（与本文冻结值一致）
激活基线：branch codex/255-capability-catalog，HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5
Provider 基线：20 项 definitions；engine_project_run、engine_project_diagnostics、engine_operation_cancel 仍存在；18 Ready、2 Unavailable
当前施工槽：激活前无其它施工文档；当前目录遗留 254-R2 evidence 文件不占槽且不得删除
文件边界：第 4 节所列源码、测试和文档均可定位；后续只在现有 untracked engine_tool_provider 基线上增量修改
测试入口：Cargo package engine_tool_provider 可发现；Gate A-C 指定的 lib/integration test 文件与测试名均存在
Skill 根：仓库不存在 ai-development-pack；后续只允许按第 4.1 节创建唯一最小 Skill，不建立并行根
工作区：存在大量既有用户变更，包括 engine_tool_provider 整体 untracked 和已有 Skill 修改；全部保留，不 reset、checkout 或 cleanup
范围判断：无需进入完整 R3、R4、真实 cancellation、production、真实 Codex 配置、Android、Local CI 或 Tower P1-2
本轮停止点：完成文档激活后停止；不运行 Cargo 命令，不施工 Gate A-C
```

## 9. 失败处理与停止规则

- Gate A 失败：只修 definition/projection/Adapter authorization，不进入 ToolResult 或 Skill。
- Gate B 失败：只修 result normalization 或静态 Skill，不进入 process smoke。
- Gate C 失败：用最窄 owner test定位；不反向新增 compatibility alias、第二 Registry 或真实 Host 验收。
- 若 `engine_runtime_run` 必须改变 Compiler/Runtime 行为才能通过，说明不是纯命名/投影问题，停止并复核方案。
- 若隐藏工具仍有生产 consumer，先判断它是内部调用还是模型调用；只保留内部 audience，不创建双 public surface。
- 若需要真实 cancellation 才能修当前测试，停止并生成独立 cancellation 研究/方案，不在本文扩建。
- 若连续施工接近 2 小时 45 分钟，不开始新 Gate；保存证据并报告。三小时必须停止。

## 10. 完成、文档同步与归档

全部 Gate 通过后，才执行一次终态同步：

1. 生成 `阶段完成记录/2026-09-03-309-F-Default-Engine-Tool-Surface-Skill-Workflow-Convergence-v1/00-总览.md`；
2. 记录实际修改文件、schema identity、模型工具列表、测试结果、未运行项和残留 R3/R4/cancellation 风险；
3. 更新正式方案状态和真实实现偏差；
4. 更新 `README.md`、`00-文档地图.md`、`49`、`54`、`施工文档/README.md`、
   `阶段完成记录/README.md`；
5. 将本文从 `当前/` 移到 `已完成/`；
6. 清空当前施工槽，不自动激活 R3、R4、cancellation 或其它系统。

不得因源码 Gate 通过而修改 production Provider、真实 Codex 配置或声称已完成真实 Host acceptance。

## 11. 施工文档自审

### 11.1 与正式方案一致性

- 采用方案 B，方案 C 仅作为单 Registry 内部诊断投影：通过。
- 当前只发布 6 项真实子集，不用 unavailable 空壳凑满 9 项：通过。
- 明确 R3 `check`、R4 `playtest/observe` 和真实 cancellation deferred：通过。
- 没有将浅 search/diagnostics/UI/capture 改名伪装新能力：通过。
- Skill 不执行、不持状态、不拥有计划 authority：通过。

### 11.2 过量施工检查

- 未新增 crate、第二 Registry、Provider、Host、Agent、daemon、Job runtime 或 workflow engine：通过。
- 未要求完整 AI Development Pack、catalog/digest/安装器：通过。
- 未修改 Compiler/Runtime/Build/Editor/Android 或样例玩法：通过。
- 只设置 3 个有独立风险合同的 Gate，没有拆成大量微 Gate：通过。
- 验证限于 owner、Adapter 和一条既有 No-Editor process smoke；未要求 Local CI/完整 workspace/真实 Host：通过。
- ToolResult schema 升级只为正式方案冻结的 retryability/local transition，不增加目标级 Planner：通过。

### 11.3 测试覆盖检查

- 默认列表、隐藏 direct call、单 Registry 和 run 原子改名有 Gate A 打红测试：通过。
- ToolResult v2 与 Skill 边界有 Gate B 打红测试：通过。
- Native/MCP 等价和真实 No-Editor run/build/verify 有 Gate C 测试：通过。
- 每个过滤命令要求非零测试；重复覆盖不重跑：通过。
- production、真实 Codex、Android、Local CI 不属于本次源码声明，明确豁免：通过。

### 11.4 当前资格

```text
正式方案：completed_and_archived
施工文档：completed_and_archived
所在目录：已完成/
Gate：A-C passed
能力状态：source_integration_passed
production/config/external-state：未修改
后续施工授权：none
```

结论：本文已按最小范围完成并归档。当前施工槽和待执行队列均为空；完整 R3、R4、真实 cancellation、
production、真实 Codex 配置、Android、Local CI 与 Tower P1-2 均未启动，不得从本文自动续工。
