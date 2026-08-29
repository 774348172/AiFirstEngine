# 254 - AI Tool Gateway / Codex Adapter v1 方案

> 状态：Core 保留并重新确认；2026-07-22 已按用户确认的方案 2 收缩第 30 节范围；254-R1/254-R2 仅作历史证据，不再是当前规范或施工入口
> 确认日期：2026-07-17  
> 范围修订：2026-07-22；254 只负责向 AI 提供好用、自由、可审计的引擎工具，不负责证明某个精确引擎版本通过真实 AI 验收
> 用户选择：总体方案 B，建设 AI Tool Gateway / Codex Adapter；按后续审查结论采用 Editor 托管 Gateway Core + 外置协议 Adapter  
> 上游实现：`250-AI-Primary-ProjectProduction-Dual-Path-v1方案.md`、`251-Provider-independent-From-Blank-Creation-Golden-Gate-v1方案.md`、`253-AI-Capability-First-Tool-Kernel-Agent-Owned-Planning-v1方案.md`  
> 竞争证据：`<LOCAL_TEST_ROOT>\Evidence\P0-0.5-v8\b8\三引擎B通道正式汇总对比.md`  
> 文档性质：架构方案，不是施工文档、施工授权、Codex 配置说明或三引擎 B 通道重跑授权

## 1. 决策

正式采用总体方案 B：在 253 已完成的 `AI Capability Tool Kernel` 之上建设协议无关的本机 `AI Tool Gateway`，由 Native Editor Host 托管唯一 Gateway Core 和唯一 `EditorSession`；Codex、CLI、Editor UI 和测试通过薄 Adapter 使用同一 Tool Catalog、Invocation、Grant、Operation、Receipt 与 Diagnostic 语义。

正式结构：

```text
Codex / 其它 Agent / CLI / Native Editor UI
  -> MCP stdio Adapter / CLI Adapter / Native Editor Adapter / Test Adapter
  -> per-user local IPC
  -> Editor-hosted Gateway Core
  -> AI Capability Tool Kernel
  -> unique EditorSession
  -> Project / RuntimePackage / Preview / Validation / Build / Export
```

核心决策：

```text
Gateway 是本机连接、项目绑定、授权解析、调度、长任务和恢复控制面。
Tool Kernel 是能力、授权、项目 mutation、receipt 和 diagnostics 的唯一语义真相。
Adapter 只转换协议，不复制业务语义。
AI 继续拥有开放式规划、工具组合和动态重规划。
Gateway 不实现 Agent Planner，不规定跨工具菜谱，不拥有用户需求流程。
```

对“Codex 不得绕过 Gateway 直接修改项目”的正式修正是：

```text
禁止不可审计、不可验证、不可回滚的项目写入。
不禁止 AI 处理未预设需求。
当专用领域工具不足时，AI 必须可以使用通用 ProjectPatch、Controlled SourcePatch、正式资产导入和受控验证工具完成 project-owned 工作。
```

Gateway 不是万能的领域实现；它必须提供足够宽的观察面、足够深的领域工具和足够通用的工程逃生通道，使 AI 的创造空间不依赖预先穷举所有工具。

## 2. 本轮审查结论

本方案吸收本轮方案讨论与复审的以下结论：

1. 只做 MCP 包装器没有产品价值；Gateway 必须解决项目绑定、唯一 Session 所有权、线程调度、长任务、断线重连和安全写入。
2. 在通用工具覆盖成熟前强制禁止所有直接写入，会把 AI 限制在引擎预设能力内；正式产品必须保留受控通用 Patch 路径。
3. 复杂需求最先需要的是理解项目。当前 `project.inspect` 只能读取 project identity、digest、RuntimeModule 和 lineage，不足以支持 Scene、Prefab、AUI、Rule、Asset、source 和 runtime diagnosis。
4. 当前 Tool Catalog 的 `inputSchema/outputSchema` 仍是通用占位对象，不能让 Codex 自动构造合法的 tool-specific invocation。
5. 当前 `execute()` 同步占用 `&mut EditorSession`；`observe/cancel` 尚不能真实观察或中断同线程长操作。
6. 当前只有 inspect、Candidate mutation、rollback 和 Preview 四个工具；没有外部 Gateway、Adapter、Grant 签发 UI、Build/Export 和复杂诊断入口。
7. 精确 `allowedDomains[]` 如果被当作用户预先选择的实现清单，会导致复杂任务在 AUI、Rule、RuntimeModule 等低风险 project-owned domain 之间反复询问。
8. 用户只从画面描述问题是零基础用户的正常输入；引擎必须把截图、AUI stable id、binding、layout、draw、runtime state 和 source owner 组成可查询证据链，不能要求用户指出代码。

结论：253 的 Agent-Owned Planning、Candidate、Grant、receipt lineage 和 rollback 不推倒重来；254 负责把它们产品化为 Codex 可使用的深 Gateway，并补齐复杂项目观察、通用修改与第一个视觉诊断证明切片。

## 3. 当前实现基线

### 3.1 已有能力

当前 `rust/crates/editor_core/src/ai_capability_tool_kernel.rs` 已实现：

```text
catalog
inspect
execute
observe
cancel
```

当前内建 Tool ID：

```text
project.inspect
project.mutate.candidate
project.rollback.candidate
project.preview
```

已有安全与工程底座：

```text
ProjectCandidateEntry
ProjectPatch
Controlled SourcePatch
Formal Asset Import
SafeProjectPath / ProjectWriteScope
CapabilityGrant
before/after project digest
MutationReceipt / RollbackReceipt
receipt lineage / drift fail-closed
project-owned RuntimeModule Preview binding
RuntimePackage / Build / Export 既有内部能力
AUI node/binding/layout/draw/present report
real-window screenshot evidence
```

### 3.2 当前缺口

```text
没有 Codex 可连接的 MCP/stdio/CLI 产品入口。
没有 Editor-hosted Gateway Core 或 per-user local IPC。
Catalog 不按当前项目、平台和授权过滤，tool-specific schema 仍是占位。
inspect 不支持 inventory、search、references、source symbols、AUI 或 runtime evidence。
execute 仍是同步路径，长任务不能真实并行 observe/cancel。
cancel 没有关联 worker、child process、cancellation token 和持久化 terminal reconciliation。
没有用户可见的 Grant 签发、续期、撤销和风险解释入口。
没有 validate/test/build_export/delivery_verify/evidence read 深工具。
没有从用户画面描述追踪到 AUI、binding、state 和 source owner 的工具闭环。
```

254 不得把这些未来缺口描述成已经完成。

## 4. 成熟引擎实现研究

### 4.1 Unity + Codex

Unity 常见自动化链：

```text
Codex 读取/修改项目 C# 和文本项目对象
  -> unity.exe -batchmode -executeMethod
  -> Editor 脚本调用 SerializedObject / Undo / AssetDatabase / PrefabUtility
  -> Unity Test Framework
  -> BuildPipeline.BuildPlayer
  -> BuildReport / log / exit code
```

官方入口：

- Editor command line：<https://docs.unity3d.com/6000.0/Documentation/Manual/EditorCommandLineArguments.html>
- `Undo.RecordObject`：<https://docs.unity3d.com/6000.0/Documentation/ScriptReference/Undo.RecordObject.html>
- `AssetDatabase`：<https://docs.unity3d.com/6000.0/Documentation/ScriptReference/AssetDatabase.html>
- `BuildPipeline.BuildPlayer`：<https://docs.unity3d.com/6000.0/Documentation/ScriptReference/BuildPipeline.BuildPlayer.html>
- Unity Test Framework：<https://docs.unity3d.com/Packages/com.unity.test-framework@1.4/manual/index.html>

Unity 的优势来自成熟、宽广、直接的项目工具面，不来自统一 AI Runner。Codex 可以根据当前证据自由组合 C#、Editor API、Play、Test 和 Build。它的弱点是 Undo、源码写入、测试、构建和外部进程没有统一授权、receipt 和跨进程恢复合同。

Unity 的 `-batchmode` 不能与同一个已经打开的项目并行使用；每次冷启动、import 和 domain reload 也可能形成成本。254 应吸收其直接工具调用和深 Build Module，不复制其分散治理与同项目多进程限制。

### 4.2 UE + Codex

UE 常见自动化链：

```text
Codex 修改项目 C++ / Config
  -> UBT
  -> Python / Editor Scripting / UObject API
  -> Commandlet / Automation Test / Data Validation
  -> UAT BuildCookRun
  -> Cook / Stage / Package / Archive / external run
```

本机源码依据：

```text
<UNREAL_LAUNCHER_REFERENCE>\UE_5.8\Engine\Source\Runtime\Engine\Classes\Commandlets\Commandlet.h
<UNREAL_LAUNCHER_REFERENCE>\UE_5.8\Engine\Plugins\Experimental\PythonScriptPlugin\Source\PythonScriptPlugin\Public\IPythonScriptPlugin.h
<UNREAL_LAUNCHER_REFERENCE>\UE_5.8\Engine\Source\Editor\UnrealEd\Public\ScopedTransaction.h
<UNREAL_LAUNCHER_REFERENCE>\UE_5.8\Engine\Plugins\VirtualProduction\RemoteControl
<UNREAL_LAUNCHER_REFERENCE>\UE_5.8\Engine\Source\Programs\AutomationTool\Scripts\BuildCookRun.Automation.cs
```

官方入口：

- Python Editor Scripting：<https://dev.epicgames.com/documentation/en-us/unreal-engine/scripting-the-unreal-editor-using-python>
- Automation Test Framework：<https://dev.epicgames.com/documentation/en-us/unreal-engine/automation-test-framework-in-unreal-engine>
- Unreal Automation Tool：<https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-automation-tool-overview-for-unreal-engine>
- Remote Control HTTP：<https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-api-http-reference-for-unreal-engine>

UE 同时提供 live Editor 和 headless 工具，但 Python、Commandlet、Remote Control、UBT 和 UAT 是不同入口；`FScopedTransaction` 也不能把 C++、UObject、Cook 和外部文件统一成一个全局事务。Remote Control 的宽反射式对象暴露适合受信生产控制，不适合直接照搬成 AI 的默认写入面。

254 应吸收 UE 的 live/headless 分层、Commandlet 式深工具和 BuildCookRun 确定性封装，不复制宽网络监听、反射式全对象写入和多套语义真相。

### 4.3 三者差异

```text
Unity / UE：AI 通过许多成熟专用入口和项目代码自由组合；速度和覆盖优先，统一治理较弱。
本引擎目标：同样保留 AI 自由组合，同时用一个机器可读 Catalog、项目 binding、Grant、Operation、Receipt 和 Diagnostic 合同统一工程动作。
```

只有本引擎的观察面、通用修改面和性能达到可用水平时，统一合同才是优势；否则它只是 Unity/UE 之外新增的一层阻力。

## 5. 目标与非目标

### 5.1 v1 目标

```text
Codex 能发现并连接已经打开的正确项目。
Codex 能通过 machine-readable Catalog 理解每个工具的真实输入、输出、副作用、成本和完成证据。
AI 能广泛读取和搜索复杂项目，不需要预先知道 object id、文件或代码位置。
AI 能优先使用领域深工具，也能在领域工具缺失时使用通用 ProjectPatch / Controlled SourcePatch。
一次用户目标批准可以支持多个短 mutation、repair 和动态重规划，不冻结工具顺序或文件清单。
Preview、validation、test、Build/Export 等长操作真实异步、可观察、可取消、可重连和可恢复。
用户可以只从画面描述 AUI 问题；AI 能在 Preview 证据中定位 node、解释 visibility 并追踪到 project owner。
MCP、CLI、Native Editor 和 Test Adapter 对同一 invocation 产生等价 Kernel 语义。
```

### 5.2 非目标

254 v1 不做：

```text
通用 Agent Planner、固定跨工具状态机或新的 ProductionRun。
让 Gateway 理解所有游戏玩法或替 AI 规划。
任意 shell、任意网络、任意文件系统或反射式内部 API 暴露。
自动修改 Engine Core 作为普通项目能力。
常驻跨机器网络服务、局域网监听或云 Gateway。
自动启动任意 Headless Editor 并同时接管已经打开的项目。
完整全项目字段级 provenance graph。
完整录像、确定性 runtime replay、线上崩溃收集或发布版远程调试。
复制 Unity/UE 的二进制资产格式、C#/C++ 工具入口或完整 Editor 调试器。
在本方案确认时生成施工文档、修改代码、配置 Codex 或重跑 B 通道。
```

## 6. 部署结构选项与选择

### 6.1 结构 1：MCP Server 直接嵌入 Editor

优点：

```text
链路短。
可以直接访问 EditorSession。
少一个进程和 IPC。
```

问题：

```text
MCP/stdio 生命周期与 Editor UI 生命周期耦合。
协议读取、序列化或客户端异常可能阻塞 Editor 主线程。
Codex 不容易启动、发现和重连已经运行的 Editor 内部 stdio server。
协议升级会污染 Editor Host。
```

不采用为正式结构。

### 6.2 结构 2：独立常驻 Gateway daemon

优点：

```text
Codex 容易启动和连接。
协议与 Editor 进程隔离。
可以自然服务多个客户端。
```

问题：

```text
daemon 必须重新拥有项目状态或通过第二套 RPC 控制 Editor。
容易与 Editor 形成两个 Project/Session truth。
项目锁、Preview RuntimeModule、Editor selection 和 unsaved state 难以统一。
daemon 崩溃与 Editor 崩溃成为两个故障域。
```

不采用为 v1 正式结构。

### 6.3 结构 3：Editor 托管 Gateway Core + 外置薄 Adapter

```text
Codex -> MCP stdio Adapter -> local IPC -> Native Editor Host
                                      -> Gateway Core
                                      -> Tool Kernel
                                      -> unique EditorSession
```

优点：

```text
Editor 继续拥有唯一 ProjectSession 和 unsaved state。
Adapter 可以由 Codex 启动、退出和升级，不阻塞 Editor UI。
同一 Gateway Core 可以服务 MCP、CLI、Native Editor 和 Test Adapter。
协议与业务语义分离。
后续可以增加真正拥有 Headless EditorSession 的 Host Adapter，而不改 Tool Kernel。
```

代价：

```text
需要本机 IPC、会话发现、ACL、线程调度、断线重连和背压。
必须明确 Editor shutdown、project switch 和 Adapter crash 语义。
```

正式采用结构 3。v1 只连接已打开的 Native Editor 项目，不自动启动 Headless Editor。

## 7. Module、Interface 与 Seam

### 7.1 Gateway Core

Gateway Core 是一个深 Module，拥有：

```text
client session handshake
protocol/schema negotiation
active project binding
opaque Grant reference resolution
request deduplication and bounds
Editor owner-thread dispatch
long-operation routing
disconnect/reconnect reconciliation
backpressure and bounded response
```

Gateway Core 不拥有：

```text
Candidate lowering
ProjectPatch semantics
Controlled SourcePatch validation
Asset import
Preview implementation
Build/Export implementation
receipt lineage rules
AI plan or user WorkItem lifecycle
```

删除 Gateway Core 后，连接、项目绑定、IPC、调度和恢复复杂度会重新散落到 MCP、CLI 和 Editor caller，因此它不是浅 pass-through。

### 7.2 Gateway Control Interface

Gateway 的外部控制 Interface 保持小：

```text
connect(ClientHello) -> ClientSessionBinding
dispatch(ClientSessionBinding, GatewayRequest) -> GatewayReply
close(ClientSessionBinding) -> CloseReceipt
```

`GatewayRequest` 是 versioned tagged union，承载：

```text
catalog
inspect
execute
observe
cancel
```

证据读取、项目搜索和视觉诊断是 Tool Kernel 中注册的只读工具，不在 Gateway Control Interface 上继续增加独立方法。

### 7.3 Tool Kernel Seam

继续沿用 253：

```text
catalog(CatalogRequest) -> ToolCatalog
inspect(InspectRequest) -> InspectResult
execute(ToolInvocation, CapabilityGrant) -> ToolResult
observe(OperationId) -> OperationSnapshot
cancel(OperationId, CapabilityGrant) -> CancellationReceipt
```

Gateway 不能自己创造另一份 Tool Catalog、Grant 校验、receipt 或 mutation 实现。项目和授权条件下哪些工具可用，由 Tool Kernel 计算；Adapter 只展示结果。

### 7.4 Adapter

首批真实 Adapter：

```text
MCP stdio Adapter：供 Codex 或兼容 Agent 调用。
CLI Adapter：供本地自动化、复现和人工诊断调用。
Native Editor Adapter：供零基础用户审批、查看进度、取消和结果展示。
Test Adapter：使用 in-memory transport 和 fault injection 验证同一 Gateway Interface。
```

MCP 的具体配置字段、启动命令和当前 Codex 客户端兼容性必须在施工前依据当时官方文档和本机版本做 smoke；本方案不写死未经验证的客户端配置。

## 8. 本机连接与项目绑定

### 8.1 传输

Windows v1 使用 per-user Windows Named Pipe 或满足同等安全属性的本机 IPC：

```text
只允许当前 OS 用户访问。
不监听 TCP，不绑定 0.0.0.0，不开放局域网。
pipe 名称不可仅由 project path 可预测。
会话发现信息放在 per-user runtime directory，不写入项目工程真相。
发现记录只保存 Editor PID、project identity、protocol version、pipe locator 和无秘密摘要。
session nonce / token 不写日志、项目 Journal 或 report。
```

跨平台传输是后续 Adapter，不改变 Gateway Control Interface。

### 8.2 Handshake

`ClientHello` 最低包含：

```text
gatewayProtocolVersion
clientKind / clientVersion
supportedSchemaVersions
expectedProjectIdentity
expectedCanonicalProjectRootDigest
requestedReadScope
```

返回 `ClientSessionBinding`：

```text
clientSessionId
editorProcessIdentity
projectIdentity
canonicalProjectRootDigest
projectDigest
gatewayProtocolVersion
effectiveReadScope
catalogDigest
expiresAt
```

后续调用只使用 `clientSessionId + projectIdentity`，不能每次传入任意项目路径。Editor 切换、关闭或替换项目时，旧 binding 失效；Adapter 必须重新 handshake，不能静默改到另一个项目。

### 8.3 唯一 Session 所有权

```text
Native Editor Host 是 v1 唯一 EditorSession owner。
Gateway request 不能从协议线程直接持有 &mut EditorSession。
需要 Editor state 的短动作进入 Editor owner-thread dispatcher。
可隔离的编译、测试、Build 子进程使用 bounded worker/child process。
真实 Apply 短暂进入 mutation lane；Preview、编译、用户等待和结果观察不持有 lane。
```

同一项目只允许一个有效 writer owner。多个客户端可以并发只读；mutation 按 digest lineage 串行提交。

## 9. 能力阶梯

Gateway 必须同时暴露三类工具。缺少任何一类，都不能宣称适合复杂需求。

### 9.1 广泛观察工具

最小集合：

```text
project.inspect
project.search
project.read_object
project.references
project.source_symbols
project.diagnostics
operation.list / receipt lookup
evidence.read
```

观察工具要求：

```text
AI 不需要预先知道 object id、GUID、path 或 source symbol。
支持按名称、文字、类型、domain、stable id、binding path 和 action id 查询。
结果分页、有界、稳定排序，返回 continuation token。
二进制内容不直接内联；返回 metadata、typed inspection 和 evidence ref。
搜索不触发完整 workspace 编译。
读取当前 project-owned source 和正式项目对象，不读取项目外无关路径。
```

用户显式把本地项目连接给本地 Gateway 后，project-owned 只读检查默认不重复请求修改批准。联网发送源码仍由 Provider 授权和隐私规则单独控制，Gateway 本身不把项目内容发送到网络。

### 9.2 领域深工具

```text
Scene / Prefab / AUI / Rule / Build Profile edit
formal asset import
project.validate
project.targeted_test
project.preview
project.build_export
project.delivery_verify
checkpoint / diff / impact / rollback
```

一个 mutation 深工具内部完成必要的 binding、Candidate、validation、Apply、digest、receipt 和 rollback handle，不要求 AI 手工调用一套固定微步骤。

### 9.3 通用工程逃生通道

```text
project.mutate.candidate
ProjectPatch
Controlled SourcePatch
Formal Asset Import
allowlisted project validation / compile / test
```

通用逃生通道保证：

```text
新玩法、新算法和复杂 UI workflow 不要求 Engine Core 预先存在专用 Tool ID。
AI 可以创建或修改 project-owned Rust source、tests 和项目数据。
SourcePatch 仍受 SafeProjectPath、allowlist、Candidate、compile/test validation、Grant、receipt 和 rollback 保护。
不降级为任意 shell 或 project-root 外文件写入。
```

如果任务需要当前 Engine Core 根本不存在的底座能力，工具返回 `capability_gap`，列出缺失合同、可用低风险替代和是否需要 maintainer/elevated 路径；不能假装 Gateway 可以凭空实现不存在的引擎能力。

## 10. Project Knowledge 与复杂项目观察

复杂项目不能依赖一次全量上下文发送，也不能要求 AI 先读完整仓库。Gateway 使用按需、分层、可追踪观察：

```text
Level 1：project summary / domains / active scene / runtime module / diagnostics summary
Level 2：inventory / search result / references / changed objects
Level 3：selected object、AUI document、Rule、source symbol、report detail
Level 4：bounded source context、full evidence artifact、trace
```

每个结果返回：

```text
projectIdentity / projectDigest
object stable identity
source or asset owner
facts / diagnostics
references and reverse references
freshness / cache status
evidence refs
continuation token when truncated
```

AI 根据新证据继续查询，不由 Gateway 预先决定阅读顺序。

## 11. Tool Catalog 正式合同

### 11.1 Project-aware Catalog

Catalog 必须根据以下上下文生成或过滤：

```text
active project identity and digest
platform / editor mode
RuntimeModule binding
effective Grant capabilities
tool implementation availability
current operation conflicts
```

静态 builtin list 只能是 registry input，不能冒充当前项目实际可调用 Catalog。

### 11.2 Tool-specific Schema

每个 `ToolDescriptor` 必须返回真实独立 schema：

```text
toolId / toolVersion
inputSchema with required / enum / oneOf / bounds
outputSchema
at least one minimal valid example
sideEffects
requiredCapabilities
changedDomainClasses
supportsDryRun / cancellation / rollback
expectedDurationClass / costClass
preconditions
stable diagnostic codes
completion evidence
```

禁止所有工具共同返回只有 `schemaVersion` 的占位 object schema。Catalog schema 必须足以让不了解内部 Rust enum 的 Agent 构造合法 invocation。

### 11.3 版本和大小

```text
Gateway protocol、Catalog、ToolDescriptor、Invocation、Result 和 Event 独立版本化。
未知 required semantics fail-closed。
请求、内联文本、数组项、日志和响应都有明确上限。
大证据通过 evidence ref 分块读取，不塞进 MCP 单次响应。
同一 toolId 的语义不静默改变；不兼容变化提升 toolVersion/schemaVersion。
```

## 12. CapabilityGrant v2 语义

### 12.1 授权什么

用户批准：

```text
用户可见目标
允许修改的风险类别
是否删除、增加依赖、联网、付费、发布或进入 Engine Core
时间、外部费用和 mutation 数量预算
```

用户不批准：

```text
完整文件清单
固定工具顺序
预先猜测的实现 domain
AI 隐藏计划或思考链
```

### 12.2 Scope Mode

在现有精确 domain 约束之上增加正式 scope mode：

```text
ExactDomains：适合高敏感或人工明确限定的任务。
ProjectOwnedLowRisk：允许在同一目标内修改 project-owned Scene、Prefab、AUI、Rule、Build Profile 和 RuntimeModule source；实际 domain 逐次记录到 receipt。
Elevated：删除、依赖、网络、费用、发布、Engine Core 或其它高风险能力。
```

`ProjectOwnedLowRisk` 不允许：

```text
项目根外写入
删除，除非显式 allowDelete
依赖变更
任意网络或外部费用
发布
Engine Core 修改
绕过 Candidate / validation / receipt
```

这样“按钮消失”从 AUI 诊断为 RuntimeModule producer 问题时，不因低风险 project-owned domain 改变而反复要求用户理解技术实现；风险等级或用户可见目标变化时仍必须重新批准。

### 12.3 Budget

时间、mutation count 和本地计算预算用于阻止无界循环，不应成为写死的复杂任务规模。预算耗尽但语义与风险不变时，Native Editor 可以请求“继续当前目标”的预算续期，不要求用户重新解释需求；增加外部费用或高风险能力仍需要明确新批准。

### 12.4 Opaque Grant Reference

opaque `grantRef` 只允许存在于 Gateway/Tool Kernel 内部，不作为 typed MCP caller 输入。Gateway 根据当前 session、project、goal 和 tool risk 从 active bounded goal-level Grant 选择并验证 authority，再向 Kernel 提供正式 `CapabilityGrant`。不把签发能力、用户身份、Grant 选择或高权限 Grant 构造交给外部 Agent。Grant digest、budget、receipt lineage 和 project drift 规则继续由 Kernel 拥有。

## 13. 直接写入与安全 Profile

### 13.1 正式原则

```text
禁止不可审计项目写入，不禁止未预设项目实现。
```

### 13.2 MaintainerCompatibility

Gateway 与工具覆盖成熟前允许维护者环境保留直接文件写入，但：

```text
Gateway 每次 mutation 前核对 project digest。
未知外部写入导致 lineage drift 并 fail-closed。
直接写入没有 Gateway receipt，不能被报告为受控 Apply。
AI 必须重新 inspect/rebase，或把外部变化导入正式 Candidate 后继续。
该模式不能作为零基础用户正式安全承诺。
```

### 13.3 ProductBrokered

正式产品目标：

```text
Agent 对项目默认只读。
Gateway 是唯一项目 writer broker。
通用 Controlled SourcePatch 保留 AI 实现自由。
Launcher / process sandbox / OS ACL 提供真实写入约束。
```

254 v1 如果没有完成 OS 级约束，只能声明“受控 Gateway 路径 + drift 检测”，不能宣称已经强制阻止所有绕过。

## 14. Invocation、幂等与 mutation

### 14.1 Invocation

每次 invocation 最低绑定：

```text
clientSessionId
invocationId
toolId / toolVersion
projectIdentity
expectedProjectDigest
grantRef or read scope
tool-specific payload
deadline / response limit
```

### 14.2 幂等

```text
相同 invocationId + 相同 invocation digest 返回原结果或原 operation。
相同 invocationId + 不同内容 fail-closed。
Adapter 重连重试不能重复 Apply。
mutation receipt 是已经发生事实的唯一证明。
```

### 14.3 mutation lane

真实项目 Apply 串行；dry-run、搜索、隔离编译和证据读取可在资源隔离成立时并行。复杂任务由多个短 mutation receipt 组成，不由一个长时间 ProjectProductionRun 持有全局 lane。

## 15. 真正的异步 Operation

### 15.1 Start

长工具的 `execute` 在完成 preflight、授权、项目 binding 和 durable operation creation 后尽快返回：

```text
status=accepted
operationId
initialStage
observeAfterHint
```

因此 Tool Kernel 的结果合同必须版本化增加 `accepted`，或增加语义等价的 typed `OperationAccepted` 结果。该结果由 Kernel 在 durable operation 已建立后产生，Gateway 只能转发，不能在没有 Kernel operation truth 时自行伪造 accepted。

协议线程和 Editor UI 线程不得同步等待完整 Preview、compile、test、Build 或 external run。

### 15.2 Progress

Operation 阶段至少包含：

```text
queued
preflight
prepared
running:<tool-defined-stage>
cancelling
completed / failed / cancelled / interrupted
```

每次阶段更新持久保存：

```text
operationId
projectIdentity / digest lineage
toolId / version
stage / progress summary
bounded diagnostics
artifact refs
started / updated / completed time
worker / child ownership summary
```

支持事件的 Adapter 可以订阅 Gateway events；MCP/CLI 必须始终可以通过 `observe(operationId)` 轮询，不依赖 push 才能完成。

### 15.3 Cancel

取消必须区分：

```text
request_received
worker_cancel_signalled
child_process_terminated
commit_not_started
commit_already_completed
non_interruptible_result_discarded
already_terminal
```

不能只把 Journal 状态改成 Cancelled。真实 worker、bounded child process 和 cancellation token 继续复用 243 生命周期纪律。

### 15.4 Crash / Disconnect / Restart

```text
Adapter 断线默认不回滚已提交 mutation，也不自动杀死可安全继续的 operation。
重连后使用 project binding + operationId 恢复观察。
Editor shutdown 对 active operation 执行 tool-specific cancel/checkpoint policy。
重启发现遗留 Running 时先 reconcile 为 interrupted/recoverable/failed，不永久伪装 Running。
只有 receipt 和当前 digest 证明 mutation 已提交；阶段文字不能证明 Apply。
```

## 16. 视觉描述到项目实现的诊断切片

### 16.1 用户输入

用户可以只提供：

```text
“主菜单里原来叫开始游戏的按钮不见了。”
“从游戏返回主菜单后发生。”
可选截图、圈选区域、点击位置或附件。
```

用户不需要知道 AUI node、binding path、Rule、RuntimeModule、文件或函数。

### 16.2 现有可复用事实

```text
AuiNode：node_id / name / text / visible / binding_refs / action_refs
AuiBindingDiagnostic：node_id / binding_id / path / code
AuiComputedNode：rect / effective_clip_rect / clipped_by_node / visible
AuiDrawCommand / OverlayItem：node_id / rect / text / asset_id / sort key
ProjectUiStateSnapshotReport：active/produced/declared/missing/type-mismatch paths、source_paths
AuiRuntimePresentReport / GameViewPresentReport
real-window screenshot evidence
```

### 16.3 首批视觉诊断工具

```text
runtime.capture_issue
ui.locate
ui.explain_visibility
project.trace_ui_owner
```

`runtime.capture_issue` 在用户显式报告问题或 Trace 模式下生成 `VisualIssueBundle`：

```text
project / RuntimePackage / frame digest
active scene / screen
screenshot evidence ref
authored and resolved AUI document refs
semantic node inventory, including invisible nodes
layout / clip / draw / hit-test summary
ProjectUiStateSnapshot and binding diagnostics refs
recent bounded UI action / screen-flow events
present diagnostics
```

正常 Runtime 默认 Off 或 compact，不每帧生成完整 Bundle。

`ui.locate` 支持按 visible text、node name、action、screen、历史存在和可选 bbox 模糊匹配，返回稳定排序候选及置信依据。按钮当前不可见时仍搜索 authored/resolved node 和最近问题 Bundle，不只搜索截图像素。

`ui.explain_visibility(nodeId, issueBundleRef)` 输出：

```text
authored existence and visible
binding target / input value / fallback / missing / mismatch
canvas and parent visibility chain
computed rect / off-screen / clip chain
draw command existence / cull reason / sort context
hit-test presence
first failing stage and evidence refs
```

`project.trace_ui_owner` 从 `node_id / binding path / action id` 追踪到 AUI asset、ProjectUiStateSnapshot producer、Rule/Action owner 和 project-owned source symbol。v1 只增加 AUI 定向 provenance，不建设完整全项目字段级 source map。

### 16.4 诊断决策链

```text
AUI 中不存在 -> authoring / version / RuntimePackage reachability
存在但 authored/resolved visible=false -> state or visibility binding
visible=true 但 computed off-screen / clipped -> layout / parent / resolution
draw command 不存在 -> culling / unsupported node / composition
draw command 存在但截图不可见 -> renderer / glyph / asset / alpha / ordering
可见但不能点击 -> hit-test / focus / interaction
action 已发出但无效果 -> Rule / project source / state transition
```

### 16.5 v1 边界

v1 首先覆盖 Native Editor Preview 中的 AUI 问题。导出 Player 的离线 Bug Bundle 可以作为受控 artifact 输入，但常驻 live remote debugging、完整录像和线上遥测 deferred。视觉模型可以辅助理解截图，但像素判断不能替代 stable id、binding、layout、draw 和 source evidence。

## 17. 复杂需求与反复修改

复杂任务不进入一次性大 Runner：

```text
用户目标与风险授权
  -> AI search / inspect
  -> AI 选择领域工具或通用 Patch
  -> Kernel 返回 result / operation / receipt / diagnostic
  -> AI 继续、改计划、回滚或查询新事实
  -> 需要新用户语义或更高风险时才请求决定
  -> 适当的定向验证和里程碑 Build/Export
```

支持：

```text
需求逐步补充、暂停、恢复和 Reopen。
实现过程中从 AUI 转到 RuntimeModule 等低风险 project-owned domain。
一个目标包含多个 Candidate、repair、test 和 Preview。
Bug 先复现和诊断，根因确认后再修改。
聊天中断后从 project digest、receipt、operation 和可选 WorkItem 恢复。
多个工具失败后丢弃未执行 Plan，不重置已经提交事实。
```

不保证：

```text
缺失的 Engine Core 能力可以仅靠项目 Patch 实现。
完全模糊且无截图、无屏幕、无状态、无历史对象的描述一定唯一定位。
未经用户允许自动扩大到删除、依赖、网络、费用或发布。
```

必要澄清只使用产品语言，例如“在哪个画面、什么时候消失、能否圈出大概位置”，不能要求用户指出源码或内部 domain。

## 18. Result、Evidence 与 Diagnostics

统一结果继续包含：

```text
status
toolId / toolVersion
operationId
project identity / digest
facts
diagnostics[]
suggestedNextActions[]
changedDomains[]
receiptRef
evidenceRefs[]
timing / cost
```

要求：

```text
diagnostic code 稳定，message 面向人，next action 面向 Agent。
suggestedNextActions 是建议，不是强制 transition。
首个失败阶段、命令退出码和有界 stdout/stderr 可通过 evidence 查询。
evidence ref 绑定 project、operation、digest、artifact hash 和 retention class。
Adapter 不能把完整日志或大截图内联进默认 ToolResult。
```

## 19. 并发与多客户端

```text
多个客户端只读可以并发。
同一项目真实 Apply 由短 mutation lane 串行。
每个 invocation 绑定 client session，但 receipt 绑定项目和 Grant，不因 Adapter 退出丢失。
两个 Agent 基于同一 digest 准备 mutation 时，只允许第一个合法提交；第二个 drift fail-closed 并重新检查。
Preview、Build 和 test 的共享 target、端口、报告根必须隔离或显式排队。
Editor project switch 使旧 client binding 和未启动 invocation 失效。
```

v1 不做跨机器 multi-user merge，也不伪造多个项目 mutation 的全局原子事务。

## 20. 性能要求

Gateway 只有在不显著增加 AI 工具调用成本时才有竞争价值：

```text
Adapter 启动和 handshake 不触发 workspace 编译、资源导入或 RuntimePackage rebuild。
Catalog / search / lightweight inspect 使用当前 Editor cache 和项目索引。
Gateway 保持 warm Editor session，不为每次调用重启 Editor。
mutation validation 由 changed domains 驱动，不默认跑完整 workspace。
长任务立即返回 operationId，不让协议 timeout 冒充引擎失败。
Build/Export 权威运行只在候选冻结和定向验证通过后运行。
cache hit/miss、IPC wait、Editor queue wait、worker time 和 external process time 分开报告。
```

方案不写死未经测量的毫秒门槛。施工文档必须先建立 warm/cold baseline，再冻结 P50/P95、最大 payload、queue 和 end-to-end budget；竞争 Gate 继续记录首个可玩修改和总墙钟。

## 21. 安全与隐私

```text
默认 local-only IPC，per-user ACL，无任意网络 listener。
Gateway 不保存 API Key，不把 token 写入项目或日志。
Adapter 不获得 Editor 内部对象指针或反射式任意调用。
所有 path 进入 SafeProjectPath / ProjectWriteScope。
所有 mutation 进入 Candidate / validation / Grant / receipt。
Read scope 只覆盖用户连接的项目和其受控证据根。
项目源码发送给远程 Provider 继续需要独立授权、预算和 provider preflight。
Evidence 按 Summary/Trace 和 retention class 管理，截图、源码和日志不无限保存。
协议解析限制递归深度、字符串、数组、总消息和并发请求。
```

MCP Adapter 被攻陷时仍不能自行签发 ElevatedGrant、切换项目、访问项目外路径或绕过 Kernel mutation。

## 22. Adapter 等价性

同一个 frozen project、Grant 和 invocation 通过 MCP、CLI、Native Editor/Test Adapter 时必须满足：

```text
相同 Tool Catalog semantics
相同 authorization result
相同 invocation digest / operation identity rule
相同 mutation receipt and project after digest
相同 diagnostic codes
相同 cancellation and recovery semantics
```

协议 framing、进度展示和人类可读格式可以不同；业务事实不能不同。测试通过 Gateway Interface 和 Adapter contract 执行，不绕过 Interface 验证内部字段。

## 23. 分阶段施工边界

本节只定义未来施工拆分，不是施工文档或开工授权。

### Phase 1：Gateway Contract / Host / Local IPC

```text
GatewayRequest/Reply/Event schema
ClientHello / SessionBinding / project switch invalidation
Editor owner-thread dispatcher
per-user local IPC and ACL
in-memory Test Adapter
```

不迁移业务工具，不做 MCP 客户端配置。

### Phase 2：Project-aware Catalog 与 Observation

```text
tool-specific schema and examples
project-aware availability
project.search/read_object/references/source_symbols/diagnostics
bounded evidence.read
pagination / continuation / response limits
```

### Phase 3：Grant v2 与通用修改逃生通道

```text
ProjectOwnedLowRisk scope mode
opaque grantRef resolution
Candidate / ProjectPatch / Controlled SourcePatch externalization
drift / replay / receipt equivalence
MaintainerCompatibility truthfulness
```

### Phase 4：真正异步 Operation

```text
worker ownership
durable stage journal
observe / cancel / reconnect / restart reconciliation
Preview / validate / targeted test
bounded child process lifecycle
```

### Phase 5：MCP / CLI / Native Editor Adapter

```text
MCP stdio protocol mapping
CLI reproducible entry
Native Editor approval/progress/cancel/result UI
Adapter equivalence matrix
current Codex client compatibility smoke
```

### Phase 6：Visual-to-Semantic First Slice

```text
VisualIssueBundle
runtime.capture_issue
ui.locate
ui.explain_visibility
project.trace_ui_owner
Preview screenshot + semantic evidence pairing
```

### Phase 7：Build / Delivery 与竞争预检

```text
project.build_export
project.delivery_verify
external no-arg window / screenshot / exit evidence
from-blank and existing-project unknown-task Gate
本引擎单通道性能预检
```

完整施工可以按 254-A/B/C 等独立施工文档分批，但每批必须在正式方案范围内、只有一个当前施工槽位，并通过前置 Gate 后才能激活下一批。不得为了“一次施工完成”重新建立大 Runner。

## 24. Gate

### Gate A：Protocol / Schema

```text
Gateway、Catalog、ToolDescriptor、Invocation、Result、Event round-trip。
tool-specific schema 能生成并验证最小合法调用。
unknown required semantics、oversize、deep recursion、invalid union fail-closed。
```

### Gate B：Project Binding / Local Security

```text
只连接当前 OS 用户和当前打开项目。
错误 project identity/root digest、Editor PID、stale discovery、project switch 被拒绝。
无 TCP listener；token/nonce 不进入日志和项目。
```

### Gate C：Complex Project Observation

```text
AI 不预知路径或 stable id，仅用名称/类型查询 Scene、Prefab、AUI、Rule、Asset 和 source。
references/reverse references、diagnostics、evidence 分页可用。
Catalog/inspect 不触发完整编译。
```

### Gate D：Generic Mutation Freedom

```text
任务在没有新增专用工具的情况下，通过 Controlled SourcePatch 修改 project-owned Rust 和 tests。
同一 ProjectOwnedLowRisk Grant 支持诊断后跨 AUI/RuntimeModule 低风险 domain。
Candidate、validation、receipt、rollback 和 drift 均成立。
删除/依赖/网络/Engine Core 仍 fail-closed。
```

### Gate E：Async / Cancel / Recovery

```text
长 Preview/test/Build execute 快速返回 operationId。
Adapter 断线重连后可 observe。
真实取消连接 worker/child，不只改状态。
Editor crash/restart 后 Running 收敛为正确 interrupted/recoverable/terminal。
duplicate invocation 不重复 Apply。
```

### Gate F：Adapter Equivalence

```text
同一 frozen invocation 经 MCP、CLI、Native Editor/Test Adapter 产生等价 Kernel 事实。
Adapter 不复制 Candidate、Grant 或 receipt 逻辑。
```

### Gate G：Visual Symptom Diagnosis

冻结一个用户只知道画面描述的 AUI 缺陷，例如：

```text
“返回主菜单后，开始游戏按钮不见了。”
```

AI 不获得代码位置和 seed 根因，必须通过 `runtime.capture_issue -> ui.locate -> ui.explain_visibility -> project.trace_ui_owner` 定位首个失败阶段，使用正式 mutation 修复，并以语义报告和截图复验。用户只提供产品语言，不理解内部对象。

### Gate H：Build / Delivery

```text
候选冻结后一次权威 Build/Export。
项目外交付、无参数启动、真实窗口、非空像素、退出 0。
Build/Delivery 是深工具，不变成开放式需求 Runner。
```

### Gate I：Bypass / Drift

```text
MaintainerCompatibility 外部写入导致 drift，不生成虚假 receipt。
ProductBrokered 等价测试中 Agent 直接写入被 OS/sandbox 拒绝。
Gateway 通用 Patch 仍可完成同一未知需求。
```

### Gate J：Performance / Competition Preflight

```text
记录 cold/warm handshake、catalog、search、mutation、validation、Preview、Build 各阶段。
无重复 workspace 权威回归。
先证明本引擎单通道明显低于两小时并保留余量，再决定是否重跑三引擎 B cohort。
```

### Gate K：权威回归

```text
targeted tests
affected crates/domains
default workspace
all-features workspace
Gateway/Adapter fault injection
exact candidate / clean source / artifact / cleanup contract
```

## 25. 测试与故障注入

至少覆盖：

```text
Adapter 在 request body 中断线。
同 invocation 重试和不同内容 replay。
Editor project switch / close / reopen。
stale project digest 和外部 direct write。
两个 Agent 同时准备 mutation。
Grant expiry、revoke、budget exhaustion 和 risk escalation。
worker panic、child timeout、kill failure、pipe close、partial stdout/stderr。
Journal 损坏、遗留 Running、receipt 已提交但 terminal persist 失败。
tool schema version mismatch。
evidence ref stale、tampered、oversize 和 project mismatch。
visual issue 中同名按钮、不可见节点、off-screen、clip、missing binding、draw/present failure。
```

测试不得只 mock Adapter 后宣称真实 Codex 可用；必须有本机跨进程、真实 Editor、真实 project-owned RuntimeModule、真实 Preview 和真实 external player 证据。

## 26. 迁移与兼容

```text
253 的 Tool Kernel Interface 和已持久化 Journal 是迁移输入，不删除。
Catalog/Grant/Operation schema 升级必须提供明确 version migration 或只读兼容。
旧 ProjectIntentWorkflow 不再拥有 execution；Intent/WorkItem/Diagnosis 继续可选关联 receipt/evidence。
旧 active ProductionRun 不能由 Gateway 猜测继续；按 253 migration/close receipt 收口。
Native Editor 现有 Preview/Build Service 是工具 Implementation，不复制进 Gateway。
```

ProductBrokered OS 写入约束只有在通用 SourcePatch、测试、导入、Build 和诊断覆盖通过后才成为默认；不得先锁死项目再补工具。

## 27. 风险与约束

| 风险 | 约束 |
|---|---|
| Gateway 变成新菜谱 | Gateway 只管理连接和执行事实；AI 拥有跨工具计划 |
| Gateway 变成浅 MCP wrapper | Gateway 必须拥有 binding、dispatch、async lifecycle、reconnect 和 security；删除后复杂度会散落 |
| 专用工具限制未知需求 | 通用 ProjectPatch / Controlled SourcePatch / asset import 作为正式逃生通道 |
| 任意 shell 扩大攻击面 | 只暴露 allowlisted compile/test/build 深工具，不提供任意 shell |
| AI 看不懂复杂项目 | 广泛 search/read/references/source/evidence 工具是 v1 硬 Gate |
| Catalog 看得到但不会调用 | tool-specific schema、example、bounds 和 stable diagnostics |
| 精确 domain 导致频繁询问 | ProjectOwnedLowRisk scope mode；高风险能力仍独立批准 |
| 长任务再次形成黑盒 | start 快速返回、durable stage、observe/cancel/reconnect/reconcile |
| Editor 与 daemon 两套真相 | v1 Gateway Core 由唯一 Native Editor Host 托管 |
| Adapter 复制业务逻辑 | MCP/CLI/Editor/Test equivalence Gate 通过同一 Kernel Interface |
| 视觉模型猜错代码 | screenshot 只作证据；stable id/binding/layout/draw/source trace 决定工程定位 |
| 运行时 Trace 过重 | Off/Summary/Trace 分档，VisualIssueBundle 只在显式诊断时生成 |
| Broker-only 提前锁死 AI | 分 MaintainerCompatibility 与 ProductBrokered；后者必须等待工具覆盖 Gate |
| 安全治理拖慢交付 | warm session、changed-domain validation、短 mutation、一次权威 Build |
| 多客户端覆盖项目 | single writer、digest lineage、idempotency 和 drift fail-closed |

## 28. 方案自审

### 28.1 是否限制 AI 发挥

通过。方案不要求每种需求都有专用工具；ProjectPatch、Controlled SourcePatch、asset import 和 allowlisted compile/test 是正式通用逃生通道。Gateway 限制的是不可审计副作用，不限制 AI 选择算法、项目结构和实现步骤。

### 28.2 是否重新引入统一 Runner

否。Gateway 没有 WorkItem 状态机、candidatePlanSteps、AdvanceRun 或跨工具完成顺序。复杂任务由 Agent 根据每次 ToolResult 动态组合；只有 Preview、Build/Export 等确定性动作内部可以是深工具。

### 28.3 是否能理解复杂项目

目标合同通过，但 project-aware Catalog 的动态 availability 尚未实现。当前已完成 search/references/evidence、稳定分页、范围限制、静态 Tool Contract Registry 与 typed schema；`catalog_for_session()` 仍只按 active project 做粗过滤，不能解释授权、RuntimeModule、平台、Host、实现和 operation conflict。该缺口以 `255-Capability-aware-Tool-Catalog-v1方案.md` 的方案 C 独立收敛，施工完成前不得把现状声明为 capability-aware Catalog 已实现。复杂项目仍通过按需观察而不是一次发送全仓上下文。

### 28.4 是否能处理反复需求和 Bug

通过设计。Plan 可丢弃；多个短 receipt 保留已发生事实；同一低风险用户目标允许跨 project-owned domain；只有语义或风险升级才重新批准。WorkItem/Diagnosis 可选用于跨会话记忆，不是工具前门。

### 28.5 用户只会描述画面时是否可用

通过限定实现。v1 已以 Native Editor Preview AUI 为首个切片，建立 screenshot + semantic node + binding + layout + draw + source owner 证据链，并完成从画面描述到正式 Candidate 修复和语义复验。世界对象、材质、动画等其它视觉域仍需后续深工具。

### 28.6 Gateway 是否成为第二能力真相

否。Tool Kernel 继续拥有 Catalog、Grant、mutation、receipt、rollback 和 diagnostic 语义；Gateway 只拥有连接、binding、dispatch、operation routing 和恢复控制面；Adapter 只做协议转换。

### 28.7 安全是否过度

没有。正式原则从“禁止直接修改”修正为“禁止不可审计写入”；MaintainerCompatibility 保留成熟前兼容，ProductBrokered 只有在通用工具覆盖和 OS 约束 Gate 后启用。

### 28.8 是否不当扩大架构层

没有。Gateway 是跨进程 owned dependency 的必要 seam；MCP、CLI、Native Editor 和 Test 四个 Adapter 已落地并通过等价性与自建客户端跨进程 smoke。该 smoke 只证明工具 Interface 与 Adapter 路径，不承担精确引擎版本或真实 AI outcome acceptance。AUI、RuntimePackage、Scene、AssetDB、Build Graph 和 Runtime ownership 不变；视觉诊断仍是注册工具，不进入 Gateway Core Implementation。

### 28.9 是否诚实反映当前实现

有界通过。第 3 节保留施工前基线；Gateway、真实 async、visual diagnosis、MCP/CLI 协议连接和 Build/Delivery 已由自动化 Gate 证据验证。真实 Codex 完成某个开放目标不再是 254 的完成条件；当前范围真相是本文第 30 节，254-R1/254-R2 只保留历史证据。

### 28.10 是否满足方案到施工规则

通过。用户已明确选择总体方案 B 并确认按审查结论生成正式 254。本文只生成正式架构方案和同步入口；没有生成施工文档、没有激活施工、没有修改代码、没有配置 Codex、没有启动测试或重跑 B 通道。

## 29. 正式结论与下一步

正式结论：

```text
采用 Editor-hosted AI Tool Gateway Core + external MCP/CLI Adapter。
AI 继续拥有开放式规划。
Gateway 统一连接、项目 binding、Grant、dispatch、operation 和恢复。
Tool Kernel 保持唯一能力语义真相。
复杂需求依赖“广泛观察 + 领域深工具 + 通用 SourcePatch 逃生通道”。
ProductBrokered 限制的是不可审计写入，不是 AI 的实现自由。
Visual-to-Semantic AUI diagnosis 是第一个复杂需求证明切片。
```

施工状态：254 的协议、Tool Kernel、Adapter、Gate A-K 自动化验证、default/all-features 权威回归、完成记录和施工归档已经闭环。254-R1/254-R2 的 candidate、activation、attempt 与真实 Codex acceptance 主线已退出 254 当前范围；R2-FC6 保持 `source_snapshot.file_set_mismatch` terminal failed 的历史终态，不重试也不修补为成功。独立退役施工已完成 R2-LR0 至 R2-LR9，终态为 `R2 lifecycle retired / 254 Core simplified`。

完成证据：

```text
施工文档/已完成/254-当前可自动化施工文档-AI-Tool-Gateway-Codex-Adapter-v1.md
阶段完成记录/2026-07-17-AI-Tool-Gateway-Codex-Adapter-v1/00-总览.md
施工文档/已完成/254-当前可自动化施工文档-R2-Lifecycle-Retirement-Core-Simplification-v1.md
阶段完成记录/2026-07-22-R2-Lifecycle-Retirement-Core-Simplification-v1/00-总览.md
```

当前施工槽位与待执行队列均为空。已归档 retirement 施工不授权继续 `ProductionCandidateModule` remediation，不授权重试 R2-FC6，也不授权执行 FC7、Real Evaluation、activation、真实 Codex attempt、F-A/F-B/G、Unity/UE 或三引擎 B 通道。

## 30. 254 范围与 AI 工具质量合同

### 30.1 唯一职责

254 的唯一职责是把引擎能力产品化为外部 AI 可以发现、理解、自由组合并留下可信审计事实的工具：

```text
好用：Catalog 诚实描述当前项目、平台、RuntimeModule 与授权下真正可用的工具；typed input、结果、diagnostic 和恢复动作足够清晰。
自由：AI 拥有跨工具规划、工具选择、调用顺序、分支和动态重规划；引擎不拥有用户需求流程或固定调用菜谱。
可审计：项目绑定、Grant、operation、side effect、receipt、digest、rollback、evidence 与 cleanup 具有结构化 lineage。
```

254 不负责证明某个精确引擎 commit、release package 或 production candidate 通过真实 AI 验收。真实 AI 对开放目标的使用可以作为非阻塞 dogfooding 或产品研究，但不得成为 254 的 candidate Gate、发布身份或完成条件。

### 30.2 保留的 Core

以下 Module、Interface 与语义继续属于 254：

```text
Editor-hosted Gateway Core + external thin MCP/CLI Adapter。
AiToolContractRegistry -> project-aware Catalog -> typed MCP projection。
Tool Kernel Core 对 capability、Grant、mutation、receipt、rollback、operation 与 diagnostic 的唯一所有权。
session/project binding、read generation、digest drift、disconnect/reconnect 与真实 async/cancel/recovery。
广泛 inspect/search/read/references/source/evidence 工具。
领域深工具与 ProjectPatch/Controlled SourcePatch/Asset Import 通用逃生通道。
project.mutate.candidate 与 project.rollback.candidate 的项目修改事务。
exact presented-frame Preview、视觉诊断、Build/Delivery 工具。
```

Gateway 不增加 Agent Planner、WorkItem Runner、跨工具 `next_action` 或验收状态机。单个深工具可以在其 Implementation 内拥有确定性阶段、事务、进程、校验与 cleanup，但这些内部步骤不进入 AI 必须学习的 Interface。

### 30.3 工具质量验证与版本验收的分界

254 必须验证工具本身：

```text
Registry/Catalog/typed MCP schema 只有一个真相来源，direct input 只包含 caller-owned 字段。
项目、session、read generation、Grant、operation 与 receipt 不能串线或跨项目复用。
mutation、rollback、Preview、diagnostics、Build/Delivery 的结构化结果与 digest 可 reopen、可解释、可验证。
长工具具有真实 worker/child ownership、cancel、terminal reconciliation 与 bounded diagnostics。
Test Adapter、MCP/CLI Adapter 和 Native Editor 通过同一 Gateway/Tool Kernel Interface，不复制业务语义。
负向测试覆盖 stale、tamper、drift、越权、错误 project/session 和不支持能力。
```

这些测试可以使用 deterministic fixture、test Adapter、headless smoke 或普通 release binary smoke，但只声明对应工具合同成立。它们不得创建或要求 `ProductionCandidateModule`、candidateId、activationId、attemptId、真实 AI outcome acceptance 或“精确版本已通过真实 AI”的结论。

### 30.4 两类 Candidate 必须分离

`project.mutate.candidate`、`ProjectCandidateEntry`、mutation receipt 与 rollback handle 描述 AI 对用户项目的一次受控修改，是 254 可审计工具能力，继续保留。

`ProductionCandidateModule` 描述精确引擎发布候选的 source snapshot、release build、conformance、seal 与 reopen，不增加 AI 理解或修改项目的能力。它连同 `CodexOutcomeEvaluationModule` 的 candidate/activation/attempt 生命周期退出 254。后续文档和代码不得再用共同的 `candidate` 名称把这两类身份混成一条流程。

### 30.5 254-R2 lifecycle 退役决定

以下内容不再是 254 公共 Interface 或 production composition：

```text
ProductionCandidateModule::produce
CodexOutcomeEvaluationModule::evaluate
engine release candidate preparation/registry/seal/reopen
activation lease/config replacement
acceptance attempt、real-session observation 与 terminal seal
真实 AI outcome validator 与 acceptance artifact
```

正常 Gateway session 不得因 evaluation attempt terminal seal 而拒绝 AI 后续合法工具调用；工具调用审计 journal、receipt lineage 和 operation evidence 继续保留，但不得承担“目标是否完成”的判断。删除上述 lifecycle 后，不新增 `RetirementManager`、兼容 Module、隐藏 Runner 或新的公共 seam。

### 30.6 废弃源码合同

后续获得独立施工授权后，R2 lifecycle 中只服务上述退役职责的源码和测试使用 `git mv` 归档到：

```text
legacy/rust/254-r2-lifecycle/
  README.md
  production-candidate/
  outcome-evaluation/
  shared-lifecycle/
  tests/
```

归档合同：

1. `legacy/rust/254-r2-lifecycle/` 是只读历史参考，不是可构建 crate、可选 feature、兼容实现、测试 Adapter 或恢复入口；目录内不得存在使其加入 Cargo workspace 的 `Cargo.toml`、`build.rs` 或发布清单。
2. active Rust 源码、Cargo dependency、production binary、MCP/CLI/Native Editor Adapter、测试和生成清单不得 import、include、复制或运行 `legacy/**`。
3. 只被 R2 lifecycle 使用的完整文件连同对应测试一起归档；同时包含 254 Core 能力的文件必须先按真实 consumer 拆分，只归档 lifecycle 部分。具有独立 active consumer 的通用能力留在其真实所有者中，不因“可能以后有用”复制两份 Implementation。
4. `README.md` 必须记录原始路径、归档 commit、退出原因、最后真实终态、相关正式方案、错误归档和完成记录，并明确禁止把历史源码作为当前施工入口。
5. 不保留 active shim、旧 CLI 参数、公共 re-export、默认 production composition 或专用 legacy error 来维持已退役 Interface；Git history 与废弃目录共同承担追溯责任。
6. 架构检查必须证明 active source 对 `legacy/**` 为零依赖，Cargo workspace、release inventory 和 binary source manifest 不包含废弃源码。

### 30.7 退役施工边界

正式退役施工必须按依赖方向进行：先建立 consumer/source inventory 与零引用红测试，再解除 `editor_host` production-candidate mode、Native Editor evaluation composition、Gateway attempt terminal-seal 控制和公共导出，随后归档无 active consumer 的源码，最后清理依赖并执行定向、受影响域、环境等价与一次最终 default/all-features 回归。

该施工只做 R2 lifecycle retirement 与 254 Core simplification，不同时实现 capability-aware Catalog 或其它新工具。退役完成后，后续独立方案应优先处理 RuntimeModule/platform/availability-aware Catalog 和工具诊断质量。

### 30.8 范围修订自审

```text
是否限制 AI：否；删除的是版本验收状态机，AI 的工具选择、顺序、分支和动态重规划保持不变。
是否削弱审计：否；Grant、operation、receipt、digest、rollback、evidence 与 cleanup 继续由 Tool Kernel/Gateway 拥有。
是否误删项目 Candidate：否；project.mutate.candidate、ProjectCandidateEntry 和 rollback 明确保留。
是否形成第二套实现：否；legacy 目录不可编译、不可依赖，不保留 active shim、feature 或旧入口。
范围修订当时是否已经授权施工：否；后续独立施工文档已完成激活前复核、移入 `当前/` 并获得用户授权，当前状态以施工文档与 54 为准。
```

本次正式方案修订不授权移动代码、修改 Cargo、生成或激活施工文档、创建 candidate/activation/attempt、运行真实 AI acceptance 或重跑任何历史 Gate。

## 附录 A. 已退役的 Real Codex 生产一致性与结果验收合同（254-R2 历史）

> 本附录冻结 254-R2 曾经确认的设计和失败上下文，只用于历史诊断。附录中的“必须”“公共 Module”“当前实现缺口”和后续施工语句均已失去规范与授权效力；与本文第 30 节冲突时无条件以第 30 节为准。

真实 Codex 验收的历史职责曾是证明正式产品工具能被真实客户端发现、理解、组合并完成开放目标，而不是证明客户端能复读一条预先冻结的调用序列。该历史设计以 `254-R2-Production-Conformance-Candidate-Lifecycle-Real-Codex-Outcome-Acceptance-v1方案.md` 保存完整上下文。

### A.1 保留的 Core

以下决定继续有效：

```text
Editor-hosted Gateway Core + external thin MCP/CLI Adapter。
AiToolContractRegistry 是工具合同唯一真相，typed MCP 由 Registry/Catalog 投影。
Tool Kernel Core 拥有 capability、Grant、mutation、receipt、rollback、operation 与审计语义。
session/project binding、read generation、unknown drift fail-closed。
ProjectPatch、Controlled SourcePatch、Asset Import 是受控通用逃生通道。
Preview 使用 exact presented-frame evidence；视觉诊断与 Build/Delivery 是独立工具能力。
AI 拥有跨工具规划、工具选择、调用顺序、分支和动态重规划。
```

旧 `ProjectIntentWorkflow/candidate_plan_steps/AdvanceRun` 不得继续作为 AI 默认入口；固定产品 Runner、固定工具链和引擎外 `next_action` 均不属于 254 Core。

### A.2 typed MCP、session、Preview 与 reconnect 修订

254-R1 暴露并修复的下列接口事实继续保留，不因 R1 被替代而回退：

```text
真实 Codex 使用逐工具严格 typed MCP，不构造内部 AiToolInvocation union。
direct input 只包含 caller-owned 字段；session-owned context/digest/grant 由 Gateway 补齐并核验。
Read capability 与 mutation Grant 分离；read 不要求人工批准。
approval、TTL、revoke、project switch 与 stale decision 由 Gateway Core 集中管理。
Preview evidence 必须绑定真实 presented frame、operation、project、digest 与 PNG bytes。
project switch/reopen 后旧 session 失效，客户端显式 reconnect，不透明复用旧 binding。
headless、自建 JSON-RPC client 与测试 Adapter 只能证明 conformance，不能冒充真实 Codex acceptance。
```

### A.3 严格性边界

正式边界是：

```text
AI owns the cross-tool plan.
Each tool owns its internal deterministic workflow.
Grant constrains risk, not implementation.
Acceptance validates outcomes and safety invariants, not a trace recipe.
```

单个工具或生命周期 Module 内部可以有严格阶段、事务、超时、回滚和校验；调用者只看到深 Interface。工具外不得要求 AI 遵循固定 tool id 集合、exact input、调用次数、总顺序或 `next_action`。工具间只允许由事实产生的因果依赖，例如 mutation 前必须已批准、Delivery 必须引用真实 Build artifact、rollback 必须引用真实 mutation receipt。

### A.4 bounded goal-level Grant

用户批准的是可见目标及其风险包络，不是未来实现步骤。Grant 至少绑定：

```text
GoalBinding:
  goalId
  userVisibleOutcome
  projectIdentity
  initialProjectDigest

RiskEnvelope:
  ProjectOwnedLowRisk scope
  optional path/object restrictions
  mutation/time/cost budgets
  delete/dependency/network flags
```

同一有效 Grant 内允许为实现目标进行多次低风险 mutation；read 不消耗 mutation budget，budget 不得默认退化为一次 mutation。超出 scope、风险或预算时必须请求新批准。forward mutation authority 可以到期，但既有 receipt 的 rollback authority 必须保持可用。批准等待超时、用户拒绝或外部中断属于 attempt inconclusive，不自动否定 candidate。

`GoalGrantAuthorityModule` 是既有 Tool Kernel Core 的深化，不新增一个 Gate 专用公共 Module。批准摘要必须真正绑定 `GoalBinding + RiskEnvelope`，不能只绑定某一次工具 payload。

### A.5 package identity 与三类事务身份

package identity 描述不可变生产物；candidateId、activationId、attemptId 是三类 lifecycle 事务身份。四类事实必须分离，但不得把 package identity 误写成调用者要执行的 lifecycle step：

| 身份 | 创建时机 | 可变性与失败语义 |
|---|---|---|
| package identity | 生产包完成并通过内部一致性验证后 | 描述可执行内容，不包含 activation/config receipt |
| candidateId | staging 内可预分配；atomic publish 与 reopen 成功后才生效 | seal 前分配值不可观察、不可引用且不构成 candidate；acceptance 的确定性失败终止已生效 candidate |
| activationId | 每次 compare-and-replace activation 时 | 每次激活均新建；绑定配置 receipt 与 durable rollback |
| attemptId | 每次真实 Codex acceptance 开始时 | evidence 不得跨 attempt 拼接；inconclusive 可用新 attempt 重做 |

candidate identity 不得包含 activation 时才产生的配置状态，也不得包含客户端未来调用计划。activation 只能通过 compare-and-replace 发布，并保证可恢复到其记录的前态。

### A.6 生产一致性与候选生命周期 Module

公共生命周期 Interface 收敛为两个目标级深 Module：

```text
ProductionCandidateModule::produce(request)
  -> CandidateDisposition

CodexOutcomeEvaluationModule::evaluate(candidate, goal, riskEnvelope)
  -> EvaluationOperation
```

`ProductionCandidateModule` 对调用者表达“生产一个经过验证且可重开的不可变候选”这一目标。它内部拥有 trusted source snapshot、release build、fixture/Editor preflight、config-independent conformance、失败 artifact、cleanup、candidate seal 与 reopen。Implementation 可以为了 staging 路径和 manifest 原子发布而预分配 candidateId，但只有 atomic publish 与 reopen 成功后该身份才生效并返回给调用者；seal 前分配值不可观察、不可复用，也不构成“消费了一个 candidate”。调用者不得分别调用或编排 prepare/conformance/seal，也不得提交内部 stage order、排除清单或手工拼接 manifest。

`ProductionCandidateModule` 内部必须把 `ImmutableSourceSnapshot` 与 `DisposableConformanceWorkspace` 表达为两种不可混淆的 capability/value role；它们不是新的公共 Module，也不是 caller action。snapshot 是 candidate source identity 的唯一不可变真相，绑定 exact file set、bytes、Git material、receipt 与 reopen；任何可能创建 `Library`、cache、journal、Build、Temp 或其它生成状态的 Editor/fixture action 都不得获得 snapshot project root 的写能力。conformance workspace 由 Module 在 owned work root 中从 snapshot 物化，必须与 snapshot 规范路径互斥、无 symlink/junction/reparse/hard-link writable alias，并用 receipt 绑定来源 snapshot digest、source-relative project path、初始 file set/bytes、materialization kind 与 initial tree digest。

release build 或 source regression 只有在全部可写输出已重定向到 owned external root、输入写入被拒绝或能在阶段后 exact reopen 时，才可直接读取 snapshot；否则也必须在 disposable workspace 中执行。released Editor/MCP headless smoke 只能打开 writable conformance project，不能原地打开 snapshot fixture。workspace 中的生成状态不进入 candidate source identity；producer 必须根据引擎自己的 project/generated-state 真相分类实际 delta，任何 authored material 漂移都 fail-closed。conformance 完成后先验证 snapshot exact reopen 与原 source 未漂移，再清理 workspace 并记录 cleanup receipt。

Implementation 必须保留 crate-private production/test composition seam，使测试能通过同一个 `produce` orchestration 替换真正会变化的内部 Adapter；只用一个 test closure 替换整个 `produce` body 不能证明内部组合。快速组合测试至少覆盖“conformance 写入 workspace、snapshot 不变并可 reopen、workspace 被清理”，最终 production preflight 再用 release inventory 中的真实 Editor/MCP binary 覆盖同一路径。full workspace 测试数量、name-filtered test 或 skipped/ignored preflight 不能替代该 exact composition evidence。

`CodexOutcomeEvaluationModule` 对调用者表达“使用某个 sealed candidate 评价真实 Codex 是否完成开放目标”这一目标。它内部拥有 activation compare-and-replace、lease、真实 client/session binding、goal-level Grant、attempt、outcome validation、rollback 与 cleanup。用户批准仍通过既有 Gateway/UI authority 进入；调用者不调用 `advance`，不手工推动 activate/begin/finalize/rollback，也不获得内部 next step。`EvaluationOperation` 只暴露既有长 operation 所需的 observe/cancel/terminal disposition，不暴露内部阶段菜谱。真实 Codex 不由引擎 shell/CLI 启动；production composition 必须把 Gateway 已连接的真实 client/session 事件与 receipt 通过内部 observer Adapter 接入 Module。只有 test executor 或未接线 Runtime 时不得声明本 Module 已完成 production wiring。

原四类责任继续由内部 Module 集中拥有：`CandidatePreparationModule`、`CandidateRegistryModule`、`ActivationLeaseModule`、`RealCodexAcceptanceModule`。`ReleasePackageModule` 与 `ProductionConformanceModule` 是更内层依赖。它们是 Implementation 的内部 seam 和测试面，不是公共 lifecycle Interface。

candidateId、activationId、attemptId 继续表示三个不可混淆的事实身份，但身份不等于调用者必须执行的三个步骤。produce 内部可以经历多次 seal 前 preparation 而不消费 candidateId；evaluate 每次实际 activation/attempt 仍创建新 activationId/attemptId，并按 receipt 完成 rollback/cleanup。

### A.7 conformance 与真实 Codex acceptance 分层

Production Conformance 分为三层，三者不得互相冒充：

```text
source contract regression:
  以冻结 source snapshot 为身份真相；只读 consumer 使用 snapshot，可能写入的 consumer 使用由 Module 物化的 disposable workspace；验证 schema、decode、authorization、receipt、evidence、cancel/recovery 与 negative cases。

sealed release headless smoke:
  只使用 release package 中的正式 Editor/MCP binary，并只打开绑定 snapshot identity 的 writable conformance project；验证 project binding、discovery、typed MCP read path、snapshot unchanged、workspace delta/cleanup、退出码、artifact inventory 与进程 ownership。

real Editor / Codex outcome acceptance:
  使用真实 Native Editor window、真实 Gateway client/session 和目标级 operation，验证实际使用的 mutation、Preview、visual、Build/Delivery 与 cleanup lineage。
```

前两层可以使用自动化客户端，但只能声称 source contract 或 sealed release headless 路径成立。headless `gateway-process-preflight` 不得命名为 actual production Editor conformance，也不得声称已经验证真实窗口、mutation、Preview 或 Build/Delivery。

Real Codex Acceptance 接收一个开放的用户目标，让真实 Codex 自主观察、选择工具、实施和调整。finalize 只验证：

```text
真实 client、candidate、activation、project 与 attempt 精确绑定。
用户可见目标达到 completion policy。
实际发生的全部 side effects 均被 Grant 授权并处于风险包络内。
实际使用的 evidence 具有合法 ownership、lineage 与 digest。
最终项目状态满足该目标声明的 commit/rollback/cleanup policy。
```

只有当目标或 AI 的实际计划使用某项能力时，才要求该项工具证据。验收不得要求每个 attempt 都执行 mutation、Preview、视觉诊断、Build、Delivery 和 rollback 的全集。

### A.8 结果分类、重试与清理

```text
Preparation/conformance:
  seal 前可重复；失败清理 staging，不消费 candidateId。

Candidate:
  acceptance 的 tool-contract、安全、目标、证据或 cleanup 确定性失败 -> rejected，禁止重试该 candidate。
  attempt inconclusive 不改变 candidate 有效性。

Activation:
  每次重试生成新 activationId；失败按 receipt rollback，禁止复用旧 activation identity。

Attempt:
  每次执行生成新 attemptId；证据不得跨 attempt 合并。
  user decline / approval timeout / external interruption -> inconclusive。
  tool-contract / safety / goal / evidence / cleanup failure -> rejected。
```

任何失败都必须留下机器可读 disposition 和 cleanup 结果。进程 cleanup 的权威事实来自拥有子进程树的 bounded process primitive：Windows 使用 Job Object/handle 的 bind、terminate、wait、release 与 reader join receipt；不得使用运行结束后的裸 PID 枚举推断 ownership，也不得把 PID 复用当作 residual。staging cleanup 必须重新检查精确目录是否不存在。digest 生成失败必须 fail-closed，不得写入全零占位 digest。失败收敛不能靠手工改 manifest、补 transcript、延长历史 session 或复用 candidate/activation/attempt 身份完成。

### A.9 结果不变量，而非调用菜谱

验收 artifact 记录实际发生的事件和证据，不预声明允许列表或固定序列。Validator 校验集合完整性、身份闭包、因果关系、授权边界、最终状态和防篡改；不得因为 AI 做了额外合法 read、采用不同顺序、跳过与目标无关的工具或在证据驱动下重新规划而失败。

不得写入验收规范的内容：

```text
required tool ids / exact tool count / exact total order
冻结 direct input 作为整个 attempt 的操作剧本
每一步的 caller-owned next_action
所有目标一律 Preview + visual + Build + Delivery + rollback
把 approval timeout 当作 candidate rejected
把 activation/config receipt 纳入 candidate digest
```

### A.10 后续实现缺口与 R1 历史定位

R2 当前实现必须基于代码和真实接线事实核对并关闭或显式延后：

```text
移除固定 17-action acceptance validator 对真实 Codex 路径的控制。
移除 candidate manifest 中的 plan/config/step identity 循环绑定。
让 approval digest 绑定真实 goal/risk/budget，并支持有界多 mutation。
补齐 RuntimeModule/platform/availability-aware catalog filtering。
统一 ProjectOwnedLowRisk 与 Build/Delivery 的授权域语义。
补齐 `CodexOutcomeEvaluationModule` 的 Gateway real-session observer Adapter 与 production composition root；deterministic executor 只算内部 engine 测试。
将 source contract regression、sealed release headless smoke 与 real Editor/Codex acceptance 分开报告。
让 failure cleanup receipt 直接引用 bounded process ownership evidence，删除 construction Runner 的裸 PID authority。
核对长 operation 的真实 async/cancel/recovery，而非仅接口异步。
将 legacy workflow/AdvanceRun 降为可选高层产品工具，不作为 Agent 默认协议。
```

254-R1 及其 candidate、F-A/F-B、R5/R6 和 freezer 记录全部保留为历史诊断证据，254-R2 及其 candidate/activation/attempt 记录现在同样只保留历史终态。任何新施工不得依据本附录复活 Production Candidate、真实 AI acceptance、旧 freezer 或 Gate F/G。
