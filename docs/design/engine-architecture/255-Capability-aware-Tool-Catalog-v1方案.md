# Capability-aware Tool Catalog v1 方案

## 0. 文档状态

```text
系统编号：255
方案选择：方案 C（完整可发现目录 + Tool-owned Availability Probe + 结构化四态结果）
确认状态：用户已确认
方案自审：已通过
讨论状态：已完成
施工状态：已完成并归档
当前施工文档：无
待执行施工文档：无
施工归档：施工文档/已完成/255-当前可自动化施工文档-Capability-aware-Tool-Catalog-v1.md
完成记录：阶段完成记录/2026-07-23-Capability-aware-Tool-Catalog-v1/00-总览.md
```

本文是 253 `AI Capability Tool Kernel` 与 254 `AI Tool Gateway / Codex Adapter` 之后的独立正式方案。它只修正工具“当前是否可调用、为什么不可调用、判断基于什么状态”的发现合同，不重开 254-R1/R2，不恢复 production candidate、activation、attempt、真实 AI outcome acceptance、固定 Runner 或三引擎 B 通道。

本文不是施工文档，不授权修改 Rust 代码、生成施工 artifact、激活 `施工文档/当前/` 或执行任何 Gate。

## 1. 一句话说明

Capability-aware Tool Catalog 是 AI 的“当前能力解释器”：所有已注册 typed Tool 始终可发现；每个 Tool 同时返回当前 Session 下的 `ready / authorization_required / blocked / unsupported` 状态、结构化原因和判断依据。

它不是 Workflow、Runner、Planner 或菜谱。AI 仍然根据用户目标和每次 Tool Result 自由决定调用哪个工具、按什么顺序调用、何时改变方案。

## 2. 为什么需要独立方案

253 已把开放式规划权交给 AI，254 已把 Tool Kernel 暴露为真实 typed MCP/Gateway 工具，但当前实现还不能诚实回答“这个工具此刻能不能调用”。

当前实现证据：

1. `rust/crates/editor_core/src/ai_capability_tool_kernel.rs:1093` 的 `catalog()` 直接返回全部 builtin descriptors。
2. 同文件 `catalog_for_session()` 只判断是否存在 active project：没有项目就清空列表，有项目就返回全部工具。
3. 同文件 `descriptor()` 给全部工具写入相同的 `preconditions = ["active_project"]`，没有表达 RuntimeModule、平台、授权、实现和 operation conflict。
4. `rust/crates/ai_tool_gateway/src/mcp_stdio.rs` 声明 MCP `tools.listChanged=false`，这适合稳定 typed tool surface，但意味着动态 readiness 不能依赖工具列表增删来表达。
5. 同文件 `projected_tool_request()` 直接从静态 `AiToolContractRegistry` 解码并构造 invocation；如果未来仅在 Catalog 层过滤工具，typed invocation 仍会绕过过滤。
6. `rust/crates/ai_tool_gateway/src/core.rs` 的 active read generation、mutation access、grant refs 和 operation grant snapshots 是真实 Session authority；这些事实不能由 Tool Kernel 或 MCP Adapter 复制推断。
7. rollback 使用 receipt-bound 历史 operation/grant lineage，不等同于“当前 active mutation grant”。仅按 `requiredCapabilities` 做中央粗分类会给出错误结论。

因此，254 第 11.1 节的目标合同仍正确，但第 28.3 节“project-aware catalog 已实现”的表述过度。当前只实现了静态 Tool Contract Registry 与 active-project 粗过滤，尚未实现 capability-aware availability。

## 3. 目标

本方案必须做到：

1. 所有已注册 typed Tool 保持稳定可发现，不因当前不可用而从目录消失。
2. AI 能区分缺授权、项目状态阻塞、RuntimeModule 不匹配、平台不支持、Host/实现缺失和 operation conflict。
3. 每个判断绑定到 project digest、read generation、runtime binding、access generation 和 operation generation，能够识别陈旧结果。
4. Tool owner 决定自己的真实 readiness，Catalog 不复制各领域规则。
5. Catalog 与 execute 使用同一 availability guard 语义；execute 必须以最新事实重新检查。
6. MCP typed tool list 保持稳定，动态状态通过 `aife_catalog` 和调用失败的结构化 blocker 暴露。
7. Catalog 查询便宜、只读、无外部副作用，不把完整编译、网络探测或进程启动塞进工具发现。
8. 保持 AI 对跨工具计划、顺序、分支和动态重规划的所有权。

## 4. 非目标

本方案不做：

- 不给 AI 返回固定的下一工具、调用顺序、`nextToolIds` 或跨工具状态迁移。
- 不判断用户目标是否完成，不建立 Goal Runner、Workflow 或隐藏 Planner。
- 不自动请求、批准、续期或消费 mutation Grant。
- 不把 Catalog snapshot 当作 Grant、reservation、lock 或执行成功保证。
- 不把平台、RuntimeModule、授权和 operation 规则复制成中央 Capability DSL。
- 不复用 `AiToolCapability` 表示功能支持；它继续只表示安全授权能力。
- 不开放 caller-owned `platformAvailable`、`runtimeAvailable`、`grantActive` 等内部事实字段。
- 不建立动态插件 ABI、远程 Marketplace 或热插拔 Tool Registry。
- 不恢复任何 254-R1/R2 lifecycle、Fresh Candidate、F-A/F-B/G 或三引擎 B 通道。

## 5. 成熟引擎源码对照

### 5.1 Unity

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/Tools/EditorTool.cs
  EditorTool.IsAvailable()

<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/Tools/EditorToolManager.cs
  在 UI / execute 前重新检查 IsAvailable()

<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/BuildPipeline/BuildPipeline.bindings.cs
  BuildPipeline.IsBuildTargetSupported()

<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/MenuItem.cs
  validate function
```

可学习点：注册与当前可用性分离；可用性靠近具体 Tool/Build Target；执行入口重新检查。

不可照搬点：Unity 的许多 availability 只为本地 UI enable/disable 服务，不能直接满足 AI 所需的结构化 reason、basis digest 和跨 Adapter 等价。

### 5.2 Unreal Engine

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Core/Private/Features/ModularFeatures.cpp
  RegisterModularFeature

<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Developer/TargetPlatform/Public/Interfaces/ITargetPlatformSettings.h
  SupportsFeature

<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Developer/TargetPlatform/Public/Interfaces/ITargetPlatformControls.h
  IsSdkInstalled

<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Slate/Public/Framework/Commands/UIAction.h
  FUIAction::CanExecute
```

可学习点：注册、平台支持、SDK/Host readiness 和命令 `CanExecute` 是不同责任，不用一个大布尔值覆盖全部语义。

不可照搬点：本引擎不能把 UI command enablement 当作 Tool authority，也不能让 Adapter 直接查询多个 subsystem 后自行合成结论。

### 5.3 Godot

```text
<GODOT_SOURCE>/godot/editor/export/editor_export_platform.cpp
  can_export() 聚合平台、插件和项目配置，并返回原因

<GODOT_SOURCE>/godot/editor/settings/editor_feature_profile.h
  feature/class/property availability
```

可学习点：聚合判断必须保留原因；平台 owner 负责平台事实。

不可照搬点：不能把 export 专用判断扩张成全 Tool 中央规则表。

### 5.4 Bevy

```text
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_app/src/plugin.rs
  Plugin::ready(&App)

<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_app/src/app.rs
  plugins_state() / is_plugin_added<T>()
```

可学习点：存在性、已注册状态和 readiness 分离；具体 Plugin 对自己的 readiness 负责。

不可照搬点：本引擎 Catalog 面向外部 AI，需要稳定 schema、授权和项目 lineage，不能只返回进程内枚举状态。

### 5.5 共同结论

成熟引擎共同采用的不是跨工具菜谱，而是：

```text
stable registration
  + owner-local readiness
  + execute-time recheck
  + contextual reason
```

本方案采用该共同结构，并补上 AI 工具所需的结构化 basis、digest、authority 和 Adapter 等价合同。

## 6. 与 253 / 254 的关系

### 6.1 253 保持不变

`AI Capability Tool Kernel` 的外部 Interface 仍是：

```text
catalog(CatalogRequest) -> ToolCatalog
inspect(InspectRequest) -> InspectResult
execute(ToolInvocation, CapabilityGrant) -> ToolResult
observe(OperationId) -> OperationSnapshot
cancel(OperationId, CapabilityGrant) -> CancellationReceipt
```

255 只深化既有 `catalog`，不增加 `plan / advance / recommend_workflow / run_goal`。

### 6.2 254 保持不变

继续使用：

```text
Editor-hosted Gateway Core
external thin MCP/CLI Adapter
stable typed MCP projection
session/project binding
Grant / operation / receipt lineage
```

Gateway 继续拥有连接和 Session authority 事实；Tool Kernel 继续拥有工具语义；Adapter 只做协议投影。

### 6.3 255 的增量职责

255 新增的是一个深 Module：`Capability-aware Tool Catalog`。它隐藏 Registry、Session authority、项目、RuntimeModule、平台、Host、实现和 operation 状态的聚合复杂度，对调用者只暴露完整目录和统一 availability 结果。

具体 Tool 的 readiness 通过 Tool-owned Availability Probe 留在真实 owner 附近。Probe 是 Catalog Module 的内部 seam，不是 AI 需要逐个学习的新公共 Interface。

## 7. 所有权与 Seam

| 事实或规则 | 唯一 owner | Catalog 中的作用 |
|---|---|---|
| Tool id、schema、side effects、版本 | `AiToolContractRegistry` | 形成稳定 descriptor 与 `catalogDigest` |
| Tool 特有前置条件 | 具体 Tool / Platform / Runtime owner | 通过 Tool-owned probe 返回 reasons |
| active project、project digest | `EditorSession` / project binding owner | availability basis 与项目状态判断 |
| RuntimeModule binding | project runtime owner | Preview/runtime 类工具 readiness |
| Session read generation、Grant 状态 | `GatewayCore` authority owner | 授权和 freshness 判断 |
| operation owner/conflict | Tool Kernel operation registry + Gateway session owner | conflict blocker 与 generation |
| 四态合成、稳定排序和 digest | Capability-aware Catalog Module | 统一输出，不发明领域规则 |
| typed MCP 映射 | MCP Adapter | 稳定投影，不重算 availability |

Gateway 必须把它真实拥有的状态投影成中立、只读的 authority snapshot 交给 Kernel。它不能根据 tool id 复制 Tool policy；Kernel 也不能绕过 Gateway 猜测 opaque grant ref 是否有效。

## 8. Catalog v2 外部合同

### 8.1 顶层

新 schema 使用独立版本，例如：

```text
schemaVersion: ai-tool-catalog.v2
catalogDigest
availabilityDigest
basis
tools[]
```

其中：

- `catalogDigest` 只覆盖稳定 Tool Contract：descriptor、tool version、schema 和静态语义。
- `availabilityDigest` 覆盖本次动态 availability、reasons 与 basis。
- 动态 Grant、project digest 或 operation 变化不得改变 `catalogDigest`。
- 相同静态 Registry 在不同 Session 应有相同 `catalogDigest`，但可以有不同 `availabilityDigest`。

### 8.2 Tool Entry

每个 entry 至少包含：

```text
descriptor
availability:
  state
  reasons[]:
    code
    category
    message
    resolutionKind
    owner
  basis:
    projectIdentity
    projectDigest
    readGeneration
    runtimeBindingDigest
    accessGeneration
    operationGeneration
  inputDependentChecksRemain
```

字段要求：

- `descriptor` 继续来自唯一 `AiToolContractRegistry`，不能由 probe 改写输入 schema 或 side effects。
- `reasons` 使用稳定 code，按确定性顺序返回；不能只有供人阅读的字符串。
- `basis` 中当前不存在的值使用明确 `null`，不能伪造 digest 或 generation。
- `inputDependentChecksRemain=true` 表示 Catalog 没有具体 invocation input，execute 时仍需校验输入相关事实。
- Entry 不包含 `nextToolIds`、步骤编号、推荐调用序列或自动修复 invocation。

### 8.3 Availability State 四态

```text
ready
authorization_required
blocked
unsupported
```

语义如下：

| state | 含义 | 典型例子 |
|---|---|---|
| `ready` | 当前可观测、与输入无关的条件成立；具体输入仍可能失败 | inspect 可读；rollback 有候选 lineage 且具体 receipt 待校验 |
| `authorization_required` | 非授权前置条件成立，但当前 Session 缺少该调用所需 authority | mutation 尚未批准、Grant 已过期或撤销 |
| `blocked` | Tool 和当前构建支持该能力，但当前项目/Session/runtime/operation 状态暂时阻塞 | 未打开项目、read stale、RuntimeModule 未绑定、冲突 operation 正在运行 |
| `unsupported` | Tool 合同存在，但当前 build/platform/Host/实现不支持，单靠当前 Session 操作不能解除 | 未编译该实现、平台永不支持该 target、Host 缺少必要能力 |

四态是当前事实，不是任务建议。`authorization_required` 不会自动创建审批请求；`blocked` 不会自动调用另一个 Tool；`unsupported` 不会偷偷隐藏 Tool。

### 8.4 多原因合成

Probe 应返回全部当前可确定的独立 blocker，不因首先发现缺授权就隐藏 RuntimeModule 或平台问题。

Entry 的 state 按以下严重度确定：

```text
unsupported > blocked > authorization_required > ready
```

该优先级只用于把多个事实压缩成一个状态标签，不表示 AI 应按这个顺序解决问题。`reasons[]` 的排序必须稳定，例如按 category、owner、code 排序，不能依赖 HashMap 或调用时序。

## 9. Reason 合同

### 9.1 category

v1 至少支持：

```text
authorization
project_state
runtime_module
platform
host
implementation
operation_conflict
session_freshness
```

Tool input 的具体非法值属于 execute-time input diagnostic，不应在无 input 的 Catalog 中伪造。

### 9.2 owner

`owner` 标识谁拥有该事实，而不是谁负责安排下一步：

```text
gateway_authority
project_session
runtime_module
platform
editor_host
tool_implementation
operation_registry
```

### 9.3 resolutionKind

`resolutionKind` 只给解除阻塞的类别，不给跨工具菜谱。v1 可包含：

```text
none
request_authorization
await_user_decision
refresh_session_facts
open_or_switch_project
resolve_project_state
bind_runtime_module
select_supported_platform
wait_or_cancel_conflicting_operation
install_or_enable_support
```

禁止返回：

```text
call project.inspect, then project.preview, then project.build_export
nextToolIds: [...]
step: 3
```

## 10. Tool-owned Availability Probe

### 10.1 内部 Interface

概念 Interface：

```text
probe(AvailabilityContext) -> ToolAvailability
```

它是内部 seam，具体 Rust trait/function 形态由后续施工文档根据当前代码基线决定。正式约束是：

1. probe 位于能访问真实 Tool/Platform/Runtime owner 的位置。
2. probe 返回事实，不执行修复，不请求授权，不启动 operation。
3. probe 不改写 descriptor，不注册新工具，不决定跨工具顺序。
4. 多个 Tool 确实共享同一规则时可以复用小型公共 probe；禁止先建立中央规则语言再要求所有 Tool 填 DSL。
5. 只有一个 owner 的规则不为“未来可能复用”预造 Adapter。

### 10.2 AvailabilityContext

Context 由 engine-owned 事实构成，至少包含：

```text
project snapshot
runtime binding snapshot
platform/host snapshot
authority snapshot
operation snapshot
```

不得从 AI direct input 接受以下布尔值或 digest：

```text
projectOpen
runtimeAvailable
platformSupported
grantActive
operationConflict
accessGeneration
runtimeBindingDigest
```

这些字段只能由真实 owner 生成。

### 10.3 不按 requiredCapabilities 粗暴推断

`AiToolCapability` 继续只描述安全授权。它不是功能 feature enum，也不足以决定 availability。

例如：

- `project.mutate.candidate` 通常依赖当前 active mutation authority。
- `project.rollback.candidate` 依赖具体 receipt 的 operation/grant lineage，不能因为当前没有 active mutation grant 就直接标成 `authorization_required`。
- Preview 是否可用取决于项目 RuntimeModule binding，不是只看 `ReadProject`。
- Build/Delivery 还取决于 target platform、Host 和当前 output/operation 状态。

因此，每个 Tool owner 必须解释自己的 authority 和功能条件。

## 11. Catalog Snapshot 不是执行许可

Availability 只表示查询时刻的只读快照：

```text
Catalog ready at basis A
  != Grant
  != resource reservation
  != operation slot lock
  != execute success guarantee
```

Catalog 返回后，project digest、read generation、Grant、RuntimeModule 或 operation 都可能变化。AI 可以使用 availability 做规划，但不能把它作为绕过 execute-time guard 的凭证。

## 12. Execute-time 同源重检

### 12.1 硬合同

所有 typed 调用在真正 start/mutate/spawn 之前必须：

1. 用最新 engine-owned snapshot 再运行同一个 Tool-owned availability guard。
2. 校验 invocation input、project digest、Grant/receipt lineage 和 tool-specific constraints。
3. 若不再 ready，返回与 Catalog 相同结构的 `availability` blocker 和最新 basis。
4. 不因先前 Catalog 为 ready 而降级检查。

这里的“同一个”指同一 owner 和同一规则 Implementation，不要求 Catalog 与 execute 共享一个陈旧结果对象。

### 12.2 Adapter 不得绕开

MCP `projected_tool_request()` 可以继续使用唯一 Registry 解码 typed input，但静态 descriptor 命中不等于允许执行。所有 MCP/CLI/Editor/Test Adapter 最终都必须进入 Gateway/Kernel 的同源 guard。

只在 `tools/list` 或 `aife_catalog` 过滤而不保护 execute 属于方案失败。

### 12.3 输入依赖检查

以下检查通常只能在 execute 时完成：

- receipt 是否属于当前 Session 的 exact operation/grant lineage；
- target platform/input profile 是否是支持组合；
- output path 是否安全且未冲突；
- Candidate expected base digest 是否仍匹配；
- object/source/evidence ref 是否存在且归属当前项目。

Catalog 应通过 `inputDependentChecksRemain` 如实说明，不尝试枚举所有可能输入。

## 13. Generation、Digest 与失效

### 13.1 Basis

动态 availability 至少绑定：

```text
projectDigest
readGeneration
runtimeBindingDigest
accessGeneration
operationGeneration
```

后续实现若当前缺少 `accessGeneration` 或 `operationGeneration`，应由对应 owner 增加单调 generation，而不是用时间戳或 Adapter 本地计数冒充。

### 13.2 Generation 变化条件

```text
readGeneration：Gateway 重新确认项目事实、检测未知 drift 或重建 read grant 时变化
runtime binding：模块选择、linked runtime set 或 bind receipt 变化时 digest 变化
accessGeneration：mutation request/approve/deny/revoke/expire/renew 和 authority owner 变化时增加
operationGeneration：operation accept/phase/terminal/cleanup 或 conflict set 变化时增加
```

若 owner 能证明某些 phase 不影响 availability，可以只在 availability-relevant transition 增加 generation；该规则必须由 owner 测试覆盖。

### 13.3 Digest

`availabilityDigest` 使用 canonical serialization 计算，并覆盖：

```text
catalogDigest
basis
每个 toolId 的 state / reasons / inputDependentChecksRemain
```

禁止把本地绝对路径、秘密、随机数、wall-clock timestamp 或非确定性容器顺序写入 digest 输入。

## 14. MCP 与其它 Adapter 投影

### 14.1 稳定 typed tool list

MCP `tools/list` 继续投影全部已注册 typed Tool，并保持 `listChanged=false`。项目、Grant 或 operation 状态变化不增删 MCP tool name，因此不会迫使客户端重新协商工具 schema。

`aife_status`、`aife_catalog`、`aife_observe`、`aife_cancel` 继续作为稳定 Gateway 控制工具；业务 Tool 继续保持逐工具 typed input。

### 14.2 动态 Catalog

`aife_catalog` 返回 Catalog v2 完整动态 availability。Native Editor 和 CLI 也读取同一 Kernel 结果，不自行重算。

### 14.3 调用失败

调用一个当前不可用 Tool 时，`structuredContent` 必须包含同 schema 的 availability blocker。MCP `isError=true` 可以继续表达调用失败，但不能只返回拼接字符串。

### 14.4 listChanged 的边界

`listChanged=false` 只表示当前进程内 typed Tool Contract 集合稳定，不表示 availability 永远不变。真正增加/删除 Tool 或改变 input schema 属于新 catalog contract/version 和重新连接问题，不通过动态 availability 偷偷完成。

## 15. 缓存、性能与副作用边界

### 15.1 允许

- 静态 descriptors 与 `catalogDigest` 可按 Registry version 缓存。
- 动态 availability 可按完整 basis tuple 做短期 memoization。
- probe 可读取 EditorSession 中已经物化的项目、RuntimeModule、平台和 operation 状态。
- 多个 Tool 的公共只读 snapshot 可在一次 Catalog 请求内共享。

### 15.2 禁止

Catalog/probe 不得：

- 写项目、Library、Temp、报告或生成目录；
- 启动 subprocess、Build、Preview、编译器或外部 Player；
- 发起网络请求、SDK 下载或费用调用；
- 扫描完整 workspace 或解析全部资产来回答轻量 readiness；
- 消费 Grant mutation count、创建 approval request 或占用 operation slot；
- 隐式刷新造成用户可见状态变化。

如果 readiness 只能通过昂贵验证确定，Catalog 应返回现有事实和 `inputDependentChecksRemain=true`；真正验证留在 Tool 内部的 preflight/execute 阶段。

## 16. 兼容与迁移

### 16.1 Schema 迁移

后续施工应新增 Catalog v2，并通过 Gateway schema negotiation 明确版本。v1 客户端在兼容窗口内可以继续读取 descriptors，但不得把 v1 结果宣传为动态 availability。

兼容期结束条件必须由施工文档定义和测试，不在本方案中承诺永久双写 v1/v2。

### 16.2 `preconditions` 字段

现有 descriptor `preconditions` 可暂时保留为静态说明或迁移字段，但不能再作为当前可调用性的权威真相。动态 availability 才是 Session-specific 结果。

### 16.3 Registry 单一真相

不得为了 Catalog v2 建第二份 Tool id/schema 表。`AiToolContractRegistry` 继续是静态合同唯一真相；availability entry 以 registry descriptor 为键扩展。

### 16.4 254 文档纠偏

254 第 11.1 节的目标要求继续有效；第 28.3 节“已实现 project-aware catalog”应按本方案解释为“已实现静态 Registry/typed schema 基础，动态 capability-aware availability 未施工”。后续施工完成后才能更新为已实现。

## 17. 示例

以下只是 schema 语义示例，不是冻结实现值：

```json
{
  "descriptor": {
    "toolId": "project.mutate.candidate",
    "toolVersion": "1.0.0"
  },
  "availability": {
    "state": "authorization_required",
    "reasons": [
      {
        "code": "gateway.access.mutation_awaiting_user",
        "category": "authorization",
        "message": "Mutation access is awaiting a Native Editor decision.",
        "resolutionKind": "await_user_decision",
        "owner": "gateway_authority"
      }
    ],
    "basis": {
      "projectIdentity": "project-example",
      "projectDigest": "sha256:...",
      "readGeneration": 4,
      "runtimeBindingDigest": "sha256:...",
      "accessGeneration": 7,
      "operationGeneration": 12
    },
    "inputDependentChecksRemain": true
  }
}
```

该结果不表示 AI 必须马上请求批准，也不授权 mutation。AI 可以继续 inspect、改变实现方案、向用户解释取舍或放弃修改。

## 18. 错误与安全合同

1. 未知 Catalog schema/version fail-closed，并返回受支持版本。
2. 未知 Tool id 仍按 contract error 处理，不能伪装成 `unsupported` 已注册工具。
3. owner snapshot 无法取得或 digest 无法计算时，不返回 `ready`；使用结构化 `blocked`/internal diagnostic。
4. read stale、project drift、root mismatch、Session expiry 和 reconnect-required 继续按 254 的更严格规则处理。
5. availability reason 不包含 secret、credential、完整绝对项目路径或内部 panic 文本。
6. Adapter 不能把 `unsupported` 改成 tool missing，也不能把 `authorization_required` 自动升级为审批请求。
7. Catalog 失败不得影响已有 operation、receipt、Grant 或项目状态。

## 19. 后续施工边界

以下只定义未来施工拆分原则，不是施工文档或施工授权：

### Phase 1：Schema 与红测试

- Catalog v2、四态、reason、basis 与 digest schema；
- 完整目录、不再清空不可用 Tool；
- v1 当前行为的差距红测试。

### Phase 2：Availability Context 与 owner generations

- project/runtime/platform/host snapshot；
- Gateway 中立 authority snapshot；
- access/operation generation 与 canonical basis。

### Phase 3：Tool-owned Probes

- read/observe tools；
- mutation/rollback；
- Preview/runtime diagnostics；
- Build/Delivery；
- 共享规则只在真实重复出现后提取。

### Phase 4：Execute-time Guard

- 所有 Adapter 进入同源 guard；
- Catalog-ready 后状态变化的负向测试；
- rollback receipt-bound lineage 与 mutation active grant 分离。

### Phase 5：MCP/CLI/Editor 投影

- `tools/list` 稳定；
- `aife_catalog` 动态结果；
- typed call 返回同结构 blocker；
- Catalog v1/v2 negotiation 与兼容收口。

### Phase 6：回归与性能

- Tool Registry/typed schema 单一真相；
- Gateway/Kernel/Adapter 等价；
- no write/network/subprocess probe 证明；
- default/all-features 与受影响域权威回归。

正式施工前必须单独编写施工文档，重新核对代码基线、测试命令、环境敏感点、累积历史失败集和最终权威回归。不得从本文直接开始代码施工。

## 20. 验收标准

### 20.1 目录正确性

- 无 active project 时已注册 Tool 仍可发现，并以结构化原因标记不可用。
- 同一 Registry 在不同 Session 的 `catalogDigest` 相同。
- Grant/project/runtime/operation 变化只改变 `availabilityDigest` 和相应 entries。
- 每个不可用 entry 至少有一个稳定 reason code 和真实 owner。

### 20.2 执行一致性

- Catalog 与 execute 使用同源 owner guard。
- Catalog ready 后发生 drift，execute 必须拒绝并返回最新 blocker。
- 直接 typed MCP call 不能绕过 availability。
- rollback 不错误依赖当前 active mutation grant，而校验 exact receipt lineage。

### 20.3 AI 自由度

- Catalog 不返回固定顺序、`nextToolIds`、跨工具状态机或目标完成判断。
- AI 可以忽略暂时不可用 Tool，选择其它合法工具或改变计划。
- 单个 Tool 内部仍可有严格 preflight/阶段/事务/cleanup，但不外泄成菜谱。

### 20.4 性能与副作用

- Catalog/probe 不写文件、不联网、不 spawn process、不消费 Grant。
- Catalog 不触发完整编译、Preview 或 Build。
- 缓存结果严格绑定完整 basis，generation 变化后不能返回陈旧 ready。

### 20.5 兼容性

- MCP typed tool list 保持稳定，`listChanged=false` 语义成立。
- Native Editor、CLI、MCP、Test Adapter 对同一 Session snapshot 得到等价 availability。
- direct input 继续只包含 caller-owned 字段。

## 21. 风险与约束

| 风险 | 约束 |
|---|---|
| Catalog 变成第二套能力真相 | Registry 只拥有静态合同；Tool/Platform/Runtime owner 通过 probe 拥有 readiness |
| 中央 Capability DSL 越来越复杂 | 只允许小型真实复用 probe；不建立通用规则语言 |
| Catalog 变成 AI 菜谱 | 不返回顺序、next tool 或目标完成判断，只返回事实与 resolution category |
| 只过滤 Catalog，execute 仍可绕过 | 所有 Adapter 必须进入 execute-time 同源 guard |
| Availability 被误当授权 | snapshot 明确不是 Grant/reservation；execute 重检真实 authority |
| 授权分类误伤 rollback | Tool-owned authority policy；receipt lineage 与 active mutation grant 分离 |
| 动态状态导致 MCP 列表抖动 | 全量稳定 typed list；动态状态只在 `aife_catalog` 和 blocker 中 |
| Probe 太昂贵或有副作用 | 禁止网络、subprocess、写文件和完整构建；只读已物化事实 |
| 缓存返回陈旧 ready | basis tuple + owner generation + execute 重检 |
| Interface 随 Tool 数量膨胀 | 顶层 Kernel Interface 不变，Tool-specific probe 是内部 seam |
| 原因过细导致 AI 学规则表 | 稳定 category/owner/resolutionKind，小而通用；详细事实留给 code/message/evidence |
| 原因过粗又要猜日志 | 保留稳定 reason code、owner、basis 和同结构 execute blocker |

## 22. 方案自审

### 22.1 是否限制 AI 能力

通过。255 只解释当前工具事实，不决定工具选择、调用顺序、分支、停止条件或目标完成。AI 仍拥有跨工具动态规划；Tool 外没有菜谱。

### 22.2 是否把 Tool 内步骤泄漏到 Tool 外

否。Preview、Build、Delivery 等深 Tool 可以在 Implementation 内拥有确定性阶段和 cleanup；Catalog 只返回它们当前是否可开始，不暴露内部阶段作为 AI 必须逐步调用的 Interface。

### 22.3 Module 是否足够深

通过。调用者只学习现有 `catalog` 和统一 availability schema，Registry、authority、project/runtime/platform/operation 聚合被隐藏。删除该 Module 后，这些判断会重新散落到 MCP、CLI、Editor 和每个调用点，说明它提供真实 Leverage 与 Locality。

### 22.4 是否形成中央第二真相

否。静态合同仍由 Registry 唯一拥有；Tool 特有规则仍由 Tool/Platform/Runtime owner 拥有；Gateway 只提供自己真实拥有的 Session authority snapshot；Catalog 只聚合和规范化结果。

### 22.5 是否过度设计

未过度。外部新增只有四态、reason、basis 和一个动态 digest；不增加公共生命周期 Module、Planner、DSL、插件 ABI、审批编排或 Runner。generation 只覆盖现有真实可变 owner，避免用昂贵全量重算或时间戳猜 freshness。

### 22.6 是否诚实处理输入未知

通过。`inputDependentChecksRemain` 明确区分“当前无输入条件成立”和“任意输入都保证成功”。rollback、target platform、receipt 和 output path 不在 Catalog 中伪判。

### 22.7 是否保护执行路径

通过设计。Catalog 不承担授权；真正执行前必须运行同源 guard。静态 Registry 解码、MCP cached list 或旧 Catalog snapshot 都不能绕过最新 project/Grant/runtime/operation 检查。

### 22.8 是否兼容 MCP

通过。全部 registered typed Tool 始终存在，`tools/list` 不随项目状态变化，`listChanged=false` 继续成立；动态 availability 由 `aife_catalog` 和结构化调用 blocker 提供。

### 22.9 是否尊重 254 scope correction

通过。本文只提高 AI 工具的好用、自由和可审计性，不生产精确引擎版本，不运行真实 AI outcome acceptance，不恢复 R1/R2 lifecycle。

### 22.10 是否越权施工

否。正式方案、施工、自审、权威回归、完成记录和归档均已闭环；没有执行任何 254 candidate、attempt、历史 Gate 或 B 通道。

## 23. 最终决定

采用方案 C：

```text
完整可发现目录
  + Tool-owned Availability Probe
  + ready / authorization_required / blocked / unsupported
  + structured reasons and basis
  + stable catalogDigest / dynamic availabilityDigest
  + execute-time same-owner recheck
```

255 已按独立施工文档完成 C0-C10 并归档。最终 frozen candidate `447c3810…` 的唯一一次权威 C9 通过；完整证据见阶段完成记录。
