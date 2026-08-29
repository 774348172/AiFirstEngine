# 238-Real LLM Provider / Minimal Repair Loop v1 方案

> 状态：已施工完成（`B-min+`；Gate A-G、整体回归、完成记录与归档均已完成）。  
> 方案日期：2026-07-11。  
> 路线优先级：`227` 的 `P3-1`。  
> 采用方案：`B-min+：Guarded Thin Provider + One-shot Repair`。  
> 前置系统：`191 Authoring Walkthrough`、`205 Imported ProjectPatch`、`206 Thin LLM Patch Source`、`207 ProjectPatch All-Domain Capability`、`212 Report Panel`。  
> 目标：让原生编辑器通过真实 HTTP LLM 生成结构化 `ProjectPatchDocument`，对确定性导入或验证错误最多修复一次，再进入既有人工审阅、确认、事务执行与回滚链路。

## 1. 这个系统是干什么的

直白地说：

```text
现在：
用户在 AI Panel 输入需求
  -> deterministic mock
  -> ProjectPatch proposal

238 完成后：
用户在 AI Panel 输入需求
  -> 真实 LLM HTTP provider
  -> 结构化 ProjectPatch candidate
  -> 本地 parse / schema / capability / state validation
  -> 若属于可修复错误，携带结构化 diagnostics 自动修复一次
  -> 再次本地验证
  -> 用户审阅并明确确认
  -> 既有 ProjectPatch transaction 执行或回滚
```

示例：

```text
用户：
  给敌机增加双发射击，并在 HUD 显示当前武器等级。

模型第一次输出：
  Rule operation 引用了不存在的字段 path。

本地验证器：
  project_patch.rule.payload_invalid
  operation_id = update-enemy-fire
  target = Rules/enemy-fire.rule.json

最小修复循环：
  把原始用户意图、原 candidate、允许公开的结构化 diagnostics
  再发送给同一 provider，要求只修正当前 ProjectPatch。

第二次输出合法：
  进入 PatchReviewModel，等待用户确认。
```

本系统不是：

```text
不是完整 Agent Planner。
不是多模型 Provider Registry。
不是让模型直接写项目文件。
不是让模型直接执行 UiCommand、ECS、Renderer、Build Graph 或 shell。
不是无限递归的 validate / retry loop。
不是修复运行时 gameplay bug 的自主代理。
不是自动 Apply。
不是新的 ProjectPatch 真相层。
不是新的独立 LLM Report 系统。
```

## 2. 为什么采用 B-min+

讨论阶段比较了三个方向：

```text
方案 A：Direct HTTP Min
  在当前同步命令里直接调用 HTTP，失败后修复一次。

方案 B-min+：Guarded Thin Provider + One-shot Repair
  保留 ThinLlmPatchSource，增加后台真实 HTTP、结构化输出、一次受控修复和证据。

方案 C：Full Agent Loop
  Provider Registry、多模型路由、工具调用、规划、多轮修复和自主执行。
```

选择 B-min+ 的原因：

### 2.1 AI 适配性

```text
模型输出继续是版本化 ProjectPatchDocument。
Provider 支持 strict JSON Schema 时默认启用。
模型错误由确定性 diagnostics 驱动修复，不靠自然语言猜日志。
修复后的 candidate 必须重新走同一 Import / Validator。
AI 可读 input、candidate、diagnostics、review、result 都有稳定边界。
```

### 2.2 复杂项目维护

```text
不把 LLM 变成第二套编辑器命令系统。
不把 Provider 与 Scene / Prefab / Rule / AUI / Asset / Build 服务直接耦合。
复杂项目仍由 ProjectPatch capability、domain service 和 transaction 保证正确性。
后续替换模型不会改变项目资产格式或 Apply 入口。
```

### 2.3 效率

```text
真实网络请求不阻塞 winit 编辑器主线程。
修复最多一次，避免 token、延迟和费用失控。
默认 CI 继续使用 deterministic mock / local fake HTTP server。
不引入 Tokio、Agent runtime 或多 provider 编排框架。
```

### 2.4 结构复杂度

B-min+ 只增加三项必要能力：

```text
ThinLlmPatchSource 的真实 HTTP 实现。
EditorSession 中一个薄的 pending request / receiver 状态。
一次 repair decision + attempt evidence。
```

它们是现有链路的实现状态，不是新的架构真相层。

## 3. 在本引擎中的作用

本引擎 AI 编辑主线固定为：

```text
Natural-language intent
  -> sanitized structured authoring context
  -> ThinLlmPatchSource
  -> ProjectPatchDocument candidate
  -> ProjectPatchImportService
  -> PatchValidator
  -> bounded repair request when eligible
  -> ProjectPatchImportService
  -> PatchValidator
  -> PatchReviewModel
  -> explicit user confirmation
  -> EditorSession::execute_patch_as_transaction
  -> PatchApplyReport / PatchHistory / existing Project Patch report
```

对复杂打飞机项目，238 负责：

```text
让 AI 能根据用户需求真实生成 Scene / Input / Asset / Prefab / AUI / Rule / Build patch。
让常见格式、字段、operation id、依赖和目标路径错误有一次自动收敛机会。
让失败原因以可读 diagnostics 返回给用户和 AI。
```

238 不负责：

```text
新增玩法表达力。
扩 ProjectPatch operation domain。
替代 Rule / Prefab / AUI / Asset / Build 的正式 authoring service。
替代项目侧 Rust Framework 或 Rule IR。
保证模型第一次就理解完整复杂项目。
```

## 4. 当前代码基线

### 4.1 已有 Thin LLM Patch Source

当前入口：

```text
rust/crates/editor_core/src/project_patch/llm_source.rs
```

已有：

```text
LlmPatchSourceKind::Mock
LlmPatchSourceKind::OpenAiCompatible
LlmPatchSourceConfig
LlmPatchSourceResult
ThinLlmPatchSource::generate_project_patch_json
build_project_patch_generation_prompt
deterministic mock fixtures
```

真实 provider 当前只返回：

```text
llm_patch_source.openai_compatible_not_implemented
```

### 4.2 已有 ProjectPatch 安全链路

当前链路：

```text
ProjectPatchImportRequest
  -> ProjectPatchImportService::from_json_string
  -> serde parse
  -> request / patch schema check
  -> capability diagnostics
  -> PatchValidator::validate
  -> PatchReviewModel
  -> ApplyImportedProjectPatch
  -> EditorSession::execute_patch_as_transaction
```

已有安全边界：

```text
ProjectPatchDocument schema version。
Scene / Input / Asset / Prefab / AUI / Rule / Build capability。
PatchValidator::MAX_OPERATION_COUNT = 48。
forbidden gameplay engine API 检查。
read/write set preview、risk level、requires_confirmation。
EditorSession rollback snapshot。
Project file snapshot rollback。
PatchApplyReport / PatchHistorySummary。
```

238 必须复用这些边界，禁止复制一套 provider 专用 Validator 或 Applier。

### 4.3 当前 AI Panel 调用链

```text
GenerateProjectPatchFromPrompt
  -> project_patch_llm_context_summary
  -> deterministic mock ThinLlmPatchSource
  -> raw JSON
  -> preview_ai_structured_project_patch
  -> ProjectPatchImportService
  -> review proposal
```

当前 `GenerateProjectPatchFromPrompt` 在编辑器命令处理中同步执行。

如果直接加入最长 30 秒 HTTP 请求：

```text
winit 主线程被冻结。
窗口无法重绘。
用户不能取消。
Play / Inspector / Scene 输入无法响应。
```

### 4.4 当前发现的上下文错误

`project_patch_llm_context_summary()` 当前仍输出：

```text
supported_project_patch_capabilities: Scene, Input
unsupported_project_patch_capabilities: Asset, Prefab, AUI, Rule, Build
```

但 207 已完成全域 capability。

238 必须先修正该漂移，并增加回归测试，禁止真实 provider 在错误能力表上工作。

### 4.5 当前缺少 Provider Schema

当前 prompt 只写：

```text
Only use documented ProjectPatchOperation schemas.
```

但请求并没有真正附带完整、机器可执行的 ProjectPatch JSON Schema。

结果是：

```text
模型只能从自然语言提示猜 operation 结构。
Provider 无法做 strict structured output。
Rust serde model 与 provider schema 没有一致性证据。
```

### 4.6 当前 Report 基线

212 已有统一 `Project Patch` Report provider，但当前主要汇总已应用的 PatchHistory。

238 应把 provider / repair attempt evidence 追加到现有 `Project Patch` Report，不新增 `LLM Report` 顶级真相。

## 5. 成熟工具与引擎源码参考

### 5.1 OpenAI Structured Outputs

官方 Structured Outputs 明确：

```text
JSON Schema 可以约束输出结构。
strict mode 比 JSON mode 更可靠地保证 schema adherence。
refusal 需要作为独立状态处理。
结构正确不代表业务语义一定正确。
应用侧仍需要业务验证。
Schema 与代码类型不能长期分叉。
```

本项目学习：

```text
默认请求 strict JSON Schema。
refusal、empty output、truncated output 都作为 provider result 分类。
Rust ProjectPatch model 与 provider schema 必须有自动一致性测试。
即使 strict 成功，仍必须进入 ProjectPatchImportService / PatchValidator。
```

不照搬：

```text
不把 OpenAI SDK response object 变成本引擎真相。
不依赖某个固定模型名。
不把 provider refusal 当 ProjectPatch validation error 去修复。
```

### 5.2 Unity 编辑事务

源码：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Undo\Undo.bindings.cs
  Undo.RecordObject
  Undo.RegisterCompleteObjectUndo

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\AssetDatabase\Editor\ScriptBindings\AssetDatabase.bindings.cs
  AssetDatabase.SaveAssets
  AssetDatabase.Refresh
```

Unity 典型链路：

```text
editor command
  -> RecordObject / RegisterCompleteObjectUndo
  -> mutate serialized object
  -> dirty / save
  -> AssetDatabase refresh / import
```

可学习：外部建议不能绕过编辑事务、dirty 和 save 边界。

### 5.3 Unreal Engine 事务与验证

源码：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Public\ScopedTransaction.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\ScopedTransaction.cpp

FScopedTransaction::Construct
  -> GEditor->BeginTransaction

FScopedTransaction::~FScopedTransaction
  -> GEditor->EndTransaction

FScopedTransaction::Cancel
  -> GEditor->CancelTransaction
```

验证源码：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\Editor\DataValidation\Source\DataValidation\Public\EditorValidatorSubsystem.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\Editor\DataValidation\Source\DataValidation\Private\EditorValidatorSubsystem.cpp

ValidateAssetsWithSettings
  -> ValidateAssetsInternal
  -> FValidateAssetsResults / per-asset details
  -> FMessageLog
```

可学习：transaction 与 validation 是两个边界；修复后的结果仍要重新验证，不能因为模型声称“已修复”就执行。

### 5.4 Godot 编辑动作

源码：

```text
<GODOT_SOURCE>\godot\editor\editor_undo_redo_manager.cpp

EditorUndoRedoManager::create_action
  -> add_do_method / add_do_property
  -> add_undo_method / add_undo_property
  -> commit_action
```

Godot 还按 Scene history 与 global history 区分修改，并通过 `mark_unsaved` 保持保存状态。

可学习：外部自动化必须提交正式编辑动作，而不是直接改底层对象。

### 5.5 用户提供的 UnityAIUI 参考

```text
<UNITY_UI_REFERENCE>\com.oathx.unitycli-master
```

该目录是第三方 UnityCLI，不是 Unity 官方 AI provider 源码。

可学习：

```text
异步 command bridge。
JSON request / response。
command allow/disable。
取消 token。
Unity 主线程上的正式 editor command。
```

不可照搬：

```text
ReflectionCallInvoker 的任意反射调用。
让模型按字符串调用任意 CLR type / method。
绕过 ProjectPatch capability allowlist。
```

## 6. 总体架构

正式采用：

```text
B-min+：Guarded Thin Provider + One-shot Repair
```

架构图：

```text
AI Panel
  -> GenerateProjectPatchFromPrompt
  -> capture ProjectPatchLlmContextSnapshot
  -> start ThinLlmPatchSource background request
  -> HTTP structured output
  -> LlmPatchSourceResult
  -> ProjectPatchImportService
       -> parse/schema/capability/validator
       -> accepted?
            yes -> proposal
            no  -> RepairDecision
                    -> not eligible -> stop + diagnostics
                    -> eligible     -> one repair request
                                         -> import + validate again
                                         -> accepted -> proposal
                                         -> rejected -> stop
  -> context revision/hash check
  -> PatchReviewModel
  -> user confirmation
  -> execute_patch_as_transaction
```

### 6.1 不新增 Provider Registry

保留：

```text
LlmPatchSourceKind::Mock
LlmPatchSourceKind::OpenAiCompatible
```

本轮不新增：

```text
dyn LlmProvider registry
provider discovery
priority routing
fallback model list
load balancing
multi-provider voting
```

### 6.2 不新增 Agent 层

Repair 不是 Agent：

```text
固定最多一次。
固定输入是原意图、原 candidate、结构化 diagnostics、同一 context snapshot。
固定输出仍是一个 ProjectPatchDocument candidate。
没有工具调用。
没有任务分解。
没有自主选择下一步。
没有自动 Apply。
```

## 7. ProjectPatchLlmContextSnapshot

238 需要把当前自由文本摘要收敛为一个内部结构化值：

```text
ProjectPatchLlmContextSnapshot
  schema_version
  request_id
  editor_revision
  context_hash
  project_id
  active_scene_relative_path
  selected_entity_id
  active_authoring_step
  supported_capabilities
  max_operation_count
  available_commands
  project_patch_summary
  prefab_authoring_summary
  aui_authoring_summary
  selected_report_summaries
  project_patch_schema_hash
```

它是一次请求的只读快照，不是新的项目资产，也不写入 RuntimePackage。

### 7.1 上下文来源

必须优先复用：

```text
AuthoringWorkflowModel.ai_context
ProjectPatchAiContextSummary
PrefabAuthoringAiContextSummary
AuiAuthoringAiContextSummary
Report Panel summary evidence
当前 selection / active document 的稳定 ID
```

禁止：

```text
让 Provider 自己扫描项目目录。
把整个 EditorSession 序列化给 Provider。
把任意 Report raw artifact 自动上传。
把 Runtime ECS snapshot 全量上传。
把用户未选择的文件内容自动读取并上传。
```

### 7.2 路径与隐私

默认请求中禁止发送：

```text
<ABSOLUTE_PATH> / <ABSOLUTE_PATH> 等绝对路径。
用户名、主目录、临时目录。
API key、Authorization header。
本机环境变量列表。
```

只允许发送：

```text
project-relative path。
稳定 project / document / entity / asset / rule id。
用户明确输入的自然语言需求。
当前任务必要的结构化摘要。
```

### 7.3 Context Stale Guard

请求启动时记录：

```text
expected_post_start_revision
context_hash
```

`GenerateProjectPatchFromPrompt` 启动命令本身会提交 transaction 并推进一次全局 revision，因此禁止机械要求“结果返回时 revision 必须等于启动前 revision”。正式判定规则：

```text
当前 revision == expected_post_start_revision
  -> 可直接使用原 context hash。

当前 revision != expected_post_start_revision
  -> 在主线程重新捕获同一范围的 ProjectPatchLlmContextSnapshot。
  -> 重新计算 semantic context hash。
  -> hash 相同：允许继续，说明期间只有无关 UI/Report/AI Panel 状态变化。
  -> hash 不同：判定 context_stale。
```

revision 只作为“是否需要重新计算”的快速信号；semantic context hash 才是 stale 的最终依据。

如果用户在等待期间修改了相关工程状态：

```text
结果标记 llm_patch_source.context_stale。
不创建可 Apply proposal。
不进入 repair。
提示用户基于当前状态重新生成。
```

本轮不做三方 merge 或自动 rebase。

## 8. ProjectPatch JSON Schema 合同

### 8.1 单一来源

正式规则：

```text
Rust ProjectPatchDocument / PatchOperation 类型是 canonical contract。
Provider JSON Schema 必须从 canonical Rust contract 生成或由确定性生成器导出。
禁止长期手写一份与 Rust model 平行维护的完整 schema。
```

可采用 `schemars` 等成熟 JSON Schema 生成库，但必须满足：

```text
serde rename / tag / content 规则与 schema 一致。
ProjectPatchOperation tagged union 完整覆盖。
additionalProperties = false。
nullable field 与 Rust Option 语义一致。
schema version 和 schema hash 可测试。
```

### 8.2 Strict Schema 归一化

部分 OpenAI-compatible provider 对 strict schema 有额外约束。

允许存在一个确定性的 provider schema normalization 步骤：

```text
canonical Rust-generated schema
  -> strict-compatible normalization
  -> provider request schema
```

归一化只能改变 schema 表示方式，不能改变 ProjectPatch 业务语义。

必须有测试证明：

```text
每个 ProjectPatch operation fixture 都能通过 serde round-trip。
fixture 对应 JSON 能通过 provider schema。
schema 拒绝未知 operation、未知字段和错误 enum。
schema hash 随 canonical operation contract 变化。
```

### 8.3 Structured Output Mode

配置：

```text
AI_ENGINE_LLM_STRUCTURED_OUTPUT_MODE=strict_json_schema | json_object
```

默认：

```text
strict_json_schema
```

规则：

```text
strict_json_schema：provider 必须接受 schema；不支持时结构化失败，不静默降级。
json_object：仅作为用户显式开启的兼容模式；Report 必须标记 degraded。
```

即使是 strict 模式，响应仍必须经过本地 serde parse 和 PatchValidator。

## 9. 真实 HTTP Provider

### 9.1 配置来源

继承 206 的环境变量方向：

```text
AI_ENGINE_LLM_PATCH_SOURCE=mock | openai_compatible
AI_ENGINE_LLM_BASE_URL
AI_ENGINE_LLM_MODEL
AI_ENGINE_LLM_API_KEY
AI_ENGINE_LLM_TIMEOUT_MS
AI_ENGINE_LLM_STRUCTURED_OUTPUT_MODE
AI_ENGINE_LLM_REPORT_LEVEL=off | summary | trace
```

配置边界：

```text
API key 只能来自进程环境或后续宿主安全凭据接口。
项目资产、BuildProfile、Scene、Rule、AUI、Prefab 和 RuntimePackage 禁止保存 API key。
Provider 配置不进入 Windows 发布包。
```

### 9.2 Endpoint

v1 固定使用 OpenAI-compatible Chat Completions JSON endpoint：

```text
<AI_ENGINE_LLM_BASE_URL>/chat/completions
```

`AI_ENGINE_LLM_BASE_URL` 应包含 API version 根，例如官方 OpenAI 的 `https://api.openai.com/v1`。本轮不同时实现 Responses API 和 Chat Completions 双协议。

规则：

```text
默认只允许 HTTPS。
HTTP 只允许显式 localhost / loopback，用于测试或本地模型。
禁止从项目资产覆盖 base URL。
禁止自动跟随跨 origin redirect。
最终 URL 必须再次校验 scheme / host。
```

### 9.3 HTTP Client

本轮推荐：

```text
ureq blocking client
+ std::thread worker
+ std::sync::mpsc result channel
```

原因：

```text
当前 workspace 没有 Tokio / async-std。
请求只需要非流式 JSON。
阻塞客户端放在后台线程不会阻塞编辑器。
依赖和任务模型小于引入完整 async runtime。
```

施工审查可以替换为同边界的成熟 blocking HTTP client，但不得因此引入 async runtime 或 Provider Framework。

### 9.4 请求限制

必须限制：

```text
connect timeout。
total/read timeout。
maximum request body bytes。
maximum response body bytes。
maximum repair candidate bytes。
maximum diagnostics count / message length。
```

建议 v1 默认：

```text
total timeout = 30 seconds
response body <= 2 MiB
repair attempts = 1
transient transport retries = 1
```

具体字节和 timeout 默认值可在施工文档自审时根据依赖 API 校准，但必须保持硬上限。

### 9.5 Provider Result 分类

`LlmPatchSourceResult` 需要能区分：

```text
Success
Refused
Cancelled
TimedOut
TransportError
HttpClientError
HttpServerError
RateLimited
AuthFailed
StructuredOutputUnsupported
ResponseTooLarge
EmptyOutput
InvalidProviderResponse
```

不能把这些错误全部折叠成一个 `provider_error` 字符串。

### 9.6 Transport Retry

Transport retry 与 Patch repair 是两件不同的事。

允许自动 transport retry 的情况：

```text
短暂连接失败。
明确可重试的 429，且 Retry-After 不超过本地上限。
500 / 502 / 503 / 504。
```

禁止自动 retry：

```text
400 schema/request error。
401 / 403 auth error。
404 endpoint/config error。
provider refusal。
response too large。
用户取消。
```

自动 transport retry 最多一次，并使用有上限的 backoff。

## 10. 后台请求与编辑器生命周期

### 10.1 请求启动

`GenerateProjectPatchFromPrompt` 调整为：

```text
验证 prompt 非空。
验证没有 active LLM request。
捕获 ProjectPatchLlmContextSnapshot。
生成 request_id。
AI Panel busy = true。
启动 background worker。
立即返回 pending/committed command result。
```

禁止在 UiCommand dispatch 中等待网络完成。

### 10.2 请求状态

EditorSession 只保留一个薄状态：

```text
pending request id
started editor revision
context hash
attempt kind = Generate | Repair
attempt index
logical cancel flag / generation token
result receiver
lightweight timing metadata
```

不保存：

```text
HTTP client singleton registry。
provider-owned conversation state。
长期 chat history truth。
任意 tool session。
```

### 10.3 Pump

复用 Asset Browser worker 模式：

```text
std::thread::spawn
  -> sender.send(result)

NativeEditorApplication::frame
  -> EditorSession::pump_llm_patch_request
  -> receiver.try_recv
  -> main-thread import / validation / proposal mutation
```

后台线程只能做：

```text
HTTP request / response read。
provider response envelope parse。
不依赖 EditorSession 的纯数据转换。
```

后台线程禁止直接修改 EditorSession 或项目文件。

### 10.4 Cancel

新增显式取消命令：

```text
CancelLlmPatchRequest
```

v1 取消语义：

```text
立即解除 AI Panel busy。
递增 request generation / 标记 cancelled。
忽略迟到结果。
底层阻塞 socket 可继续到 timeout，但不能再提交 proposal。
```

真正 transport-level abort 留给后续 async transport，不是 v1 前置。

## 11. Minimal Repair Loop

### 11.1 触发位置

Repair 只能发生在：

```text
provider candidate 已返回
  -> ProjectPatchImportService
  -> parse/schema/capability/validation rejected
  -> RepairDecision::Eligible
```

禁止在这些位置自动 repair：

```text
用户确认之后。
Patch transaction 执行过程中。
Apply 失败或 rollback 之后。
Runtime gameplay 执行中。
Windows build/package 运行中。
```

### 11.2 次数

固定：

```text
initial generation attempt = 1
repair attempt <= 1
```

第二次仍失败时立即停止，向用户显示最终 diagnostics。

禁止配置成无限循环；本轮也不提供大于 1 的环境变量。

### 11.3 Repair 输入

Repair request 只包含：

```text
同一原始用户意图。
同一 ProjectPatchLlmContextSnapshot。
第一次 candidate，受 response byte limit 约束。
第一次 Import / Validator 的允许公开 diagnostics。
ProjectPatch schema version / schema hash。
明确指令：只修正 candidate，不扩大需求，不执行任何操作。
```

不得加入：

```text
API key / header。
任意本地日志文件。
Apply side effects。
用户未授权的工程文件内容。
```

### 11.4 Repairable Allowlist

必须由代码中的显式 allowlist 判定，不能用字符串 contains 猜测。

v1 可修复类别：

```text
JSON parse / shape 错误。
缺失或错误的 schema field / enum。
operation_id 缺失或重复。
depends_on 引用缺失。
已知 domain operation payload 字段错误。
Validator 能明确指出 operation_id / target / expected contract 的路径或引用错误。
```

示例 diagnostics：

```text
project_patch_import.parse_failed
project_patch.operation_id_required
project_patch.operation_id_duplicate
project_patch.dependency_missing
```

完整 allowlist 必须在施工时由当前 Validator 真实 code 清单生成并逐项测试。

### 11.5 Non-repairable Denylist

以下情况不得进入 Patch repair：

```text
provider 未配置、auth 失败、拒绝、timeout、网络错误。
structured output 不受支持。
context stale。
未知或不受支持 capability。
forbidden gameplay engine API。
operation_count 超过 48。
安全路径逃逸或越过 project root。
候选要求读取或写入未授权文件。
诊断没有稳定 code。
第一次和第二次错误 fingerprint 相同。
```

### 11.6 Scope Guard

Repair candidate 相比第一次 candidate 必须满足：

```text
仍对应同一用户 request_id。
仍使用同一 context hash。
schema version 不变。
不增加未知 capability。
risk level 不升高。
operation_count 不超过 MAX_OPERATION_COUNT。
不得从低风险局部修改扩大为 Build / delete / destructive scope。
```

如果第一次 candidate 无法 parse，无法进行完整 domain diff 时：

```text
只依赖用户请求上下文的 allowed capabilities、MAX_OPERATION_COUNT、路径安全和最终 Validator。
不得因此跳过任何本地 guard。
```

### 11.7 Diagnostic Fingerprint

为防止模型重复输出同一错误，生成稳定 fingerprint：

```text
sorted(
  diagnostic.code,
  operation_id,
  target
)
```

第二次验证若 fingerprint 与第一次相同：

```text
status = repair_no_progress
停止。
```

## 12. Import、Review、Confirm 与 Apply 边界

### 12.1 Import 仍是唯一入口

无论 initial candidate 还是 repaired candidate，都必须调用：

```text
ProjectPatchImportService::from_json_string
```

禁止：

```text
Provider 直接构造 PatchReviewModel。
Provider 直接调用 PatchValidator 的子函数后跳过 Import。
修复成功后直接调用 PatchApplier。
```

### 12.2 只保留最终合法 Proposal

默认 AI Panel：

```text
第一次 candidate 合法：显示第一次 candidate proposal。
第一次失败、第二次合法：显示 repaired candidate proposal，并标记 repaired_once。
两次都失败：不显示可 Apply proposal，只显示失败摘要和 diagnostics。
```

第一次非法 candidate 可在 Trace 中保留 hash 和受控摘要，但不作为可执行 proposal。

### 12.3 用户确认不可省略

`PatchReviewModel.requires_confirmation` 必须继续为 `true`。

用户必须看到：

```text
title / intent summary。
touched domains。
read set / write set preview。
operation count。
risk level。
是否经过一次 repair。
最终 validation status / diagnostics。
```

只有用户明确执行 Apply 后，才能进入 transaction。

## 13. AI Panel 产品面

### 13.1 状态

AI Panel 至少表达：

```text
Idle
Generating
Repairing
ReadyForReview
Failed
Cancelled
Stale
```

可以由现有 `busy` 加轻量 attempt summary 实现，不要求新增完整状态机框架。

### 13.2 用户可见行为

```text
提交后输入区不冻结整个编辑器。
请求中可取消。
生成中显示 provider/model 和当前阶段，不显示 API key/base authorization。
修复发生时显示“正在根据校验结果修复 1/1”。
失败时显示自然语言摘要和最相关 diagnostics。
成功时进入既有 Patch review UI。
```

### 13.3 并发

v1 每个 EditorSession 最多一个 active LLM patch request。

第二次提交时：

```text
默认拒绝并提示先取消当前请求。
禁止隐式同时运行多个 provider request。
```

多任务队列和并发 Agent deferred。

## 14. Report 与证据

### 14.1 不新增顶级 Report

复用 212：

```text
Report Panel
  -> project.patch
  -> existing Project Patch report
```

238 把最新 provider/repair attempt evidence 追加到该 provider。

禁止新增：

```text
llm.report 顶级真相。
repair.session report registry。
第二套 Patch history。
```

### 14.2 Report Level

遵守项目 Skill：

```text
Off
  不生成 attempt evidence；只保留功能所需 busy/result 状态。

Summary（Editor 默认）
  request id
  provider id
  model
  structured output mode
  initial / repair attempt count
  latency
  final status
  candidate hash
  final diagnostic codes
  context stale / cancelled 标记

Trace（显式诊断）
  Summary 全部内容
  各 attempt latency / HTTP status class
  schema hash / context hash
  redacted diagnostic details
  candidate byte count / operation count / touched domains
```

### 14.3 Runtime 分档

238 只运行在 Editor。

```text
正式 Runtime / 导出 Windows 玩家：Off，且不链接或初始化 provider。
Editor：默认 Summary。
测试 / gate：可显式 Trace。
```

### 14.4 Redaction

任何级别都不得记录：

```text
API key。
Authorization header。
完整环境变量。
默认完整 prompt。
默认完整 raw response。
绝对本机路径。
```

Trace 如需保存可复现 fixture，必须使用 deterministic mock / local fake provider 的脱敏数据，不把真实用户请求写入仓库 artifact。

## 15. 错误代码建议

Provider：

```text
llm_patch_source.not_enabled
llm_patch_source.config_missing
llm_patch_source.base_url_forbidden
llm_patch_source.auth_failed
llm_patch_source.rate_limited
llm_patch_source.timeout
llm_patch_source.transport_failed
llm_patch_source.http_client_error
llm_patch_source.http_server_error
llm_patch_source.response_too_large
llm_patch_source.response_invalid
llm_patch_source.output_empty
llm_patch_source.refused
llm_patch_source.structured_output_unsupported
llm_patch_source.cancelled
llm_patch_source.context_stale
```

Repair：

```text
llm_patch_repair.not_eligible
llm_patch_repair.started
llm_patch_repair.succeeded
llm_patch_repair.failed
llm_patch_repair.scope_expanded
llm_patch_repair.no_progress
llm_patch_repair.attempt_limit_reached
```

错误码必须稳定、可测试、可进入 Report Panel 和 AI context。

## 16. 复杂打飞机验收场景

### 场景 A：真实 provider 生成合法 Scene Patch

```text
用户要求创建一个空的发射点 Entity。
Provider 返回合法 Scene.CreateEntity operation。
Import / Validator accepted。
AI Panel 显示 review。
用户未确认前 Scene 不变化。
用户确认后 transaction committed。
```

### 场景 B：Rule 字段路径修复一次

```text
第一次 candidate 引用不存在的 Rule 字段。
Validator 返回稳定 diagnostic code / operation / target。
RepairDecision = Eligible。
第二次 candidate 修正路径。
第二次 Import / Validator accepted。
Review 标记 repaired_once。
```

### 场景 C：两次都失败

```text
第一次 invalid。
修复后仍 invalid，或 diagnostic fingerprint 相同。
不创建可 Apply proposal。
Report 显示 initial + repair attempt 和最终 diagnostics。
```

### 场景 D：Provider 认证失败

```text
HTTP 401 / 403。
不进入 Patch repair。
不泄露 API key。
AI Panel 返回配置/认证 diagnostic。
```

### 场景 E：用户等待期间修改工程

```text
请求启动后用户修改 Rule / Scene / selection。
editor revision 或 context hash 变化。
迟到结果被标记 context_stale。
不创建 proposal，不 repair，不 Apply。
```

### 场景 F：复杂全域 Patch

```text
用户要求：
  创建/更新敌机 Prefab、发射 Rule、HUD AUI，并做 Build 验证。

Provider 输出 Asset / Prefab / Rule / AUI / Build operations。
operation count <= 48。
所有 operation 进入 207 已有 Validator / Applier。
最终仍由用户审阅 touched domains 和 write set 后确认。
```

### 场景 G：取消

```text
用户在 provider 请求中点击取消。
AI Panel 立即恢复可用。
迟到 response 被忽略。
不产生 proposal。
```

## 17. 测试策略

### 17.1 默认测试禁止依赖公网

默认 CI 使用：

```text
deterministic mock source。
本地 fake OpenAI-compatible HTTP server。
固定 JSON fixtures。
固定 latency / timeout / status fixtures。
```

真实 provider smoke 必须：

```text
#[ignore]
env-gated
local-only
不作为 cargo test 默认成功条件
不把 key / prompt / raw response 写入 artifact
```

### 17.2 必测行为

```text
strict schema request 包含当前 ProjectPatch schema/version/hash。
全域 capability 摘要不再错误标记 Asset/Prefab/AUI/Rule/Build unsupported。
请求上下文不包含绝对路径或 API key。
真实 HTTP 成功响应进入 Import。
401/403 不 retry、不 repair。
429/5xx 最多 retry 一次。
response size limit 生效。
主线程不阻塞，frame pump 可继续。
Cancel 后迟到 result 被忽略。
stale revision 不创建 proposal。
可修复 diagnostic 只触发一次 repair。
denylist diagnostic 不 repair。
repair scope 扩大被拒绝。
repair no-progress 被拒绝。
最终合法 proposal 仍 requires_confirmation。
未确认时项目文件和 EditorSession 不变化。
Apply 继续走 execute_patch_as_transaction。
Summary / Trace 不泄露 secret 和绝对路径。
Runtime / export 不初始化 provider。
```

## 18. 可施工 Gate 建议

正式施工文档生成后建议按以下 Gate 执行，不得把全部能力一次堆到单个 Gate。

### Gate A：Contract / Context / Schema

```text
修正 Scene/Input-only 旧能力摘要。
ProjectPatchLlmContextSnapshot。
relative path / redaction / context hash。
canonical ProjectPatch JSON Schema 生成与一致性测试。
strict schema normalization。
```

### Gate B：HTTP Transport

```text
OpenAiCompatible 真实 JSON HTTP。
env config / HTTPS-localhost policy。
timeout / body limit / redirect guard。
provider result 分类。
transient retry 上限。
local fake server tests。
```

### Gate C：Background Request / Cancel

```text
std::thread + mpsc。
EditorSession pending state。
NativeEditorApplication frame pump。
AI Panel busy / cancel / late-result guard。
主线程响应性测试。
```

### Gate D：One-shot Repair

```text
RepairDecision allowlist / denylist。
repair prompt / same context。
scope guard / diagnostic fingerprint。
最多一次 attempt。
deterministic repair fixtures。
```

### Gate E：Import / Review / Report Integration

```text
initial 和 repaired candidate 都走 ProjectPatchImportService。
最终 proposal / requires_confirmation。
现有 project.patch Report 扩展。
Off / Summary / Trace。
secret/path redaction tests。
```

### Gate F：Complex Shooter E2E

```text
Scene 合法生成。
Rule 一次修复。
Asset / Prefab / AUI / Rule / Build 全域 candidate。
用户确认前无修改。
确认后 transaction committed。
失败和 rollback 路径不回归。
```

### Gate G：Regression / Docs / Archive

```text
cargo fmt --check。
editor_core / editor_ui_model / editor_window_winit tests。
project_e2e_gate。
env-gated ignored real provider smoke（本机可选）。
阶段完成记录。
49 / 54 / 227 / 文档地图同步。
施工文档归档。
```

## 19. 本轮明确不做

```text
Provider Registry。
多模型路由、fallback、投票或 ensemble。
Agent Planner / task graph。
工具调用或 MCP。
模型直接调用 UiCommand。
模型直接读写文件、ECS、Renderer、RuntimePackage。
多轮递归修复。
Apply 后自动诊断和修复。
自动 rebase / three-way merge。
并发 LLM request queue。
流式 token UI。
provider conversation memory。
prompt history 作为项目真相。
真实 provider 作为默认 CI 依赖。
Runtime 内置 provider。
图像、音频、3D 生成 provider。
OS credential vault 产品化。
transport-level immediate socket abort。
```

## 20. 风险与控制

### 风险 1：又增加一层 Agent 架构

控制：不新增 Agent、Registry 或 Planner；只扩 `ThinLlmPatchSource`，repair 固定一次且无工具。

### 风险 2：HTTP 冻结编辑器

控制：真实请求只在后台 worker；主线程只 start / pump / commit result。

### 风险 3：Provider schema 与 Rust 类型漂移

控制：Schema 从 canonical Rust contract 生成；fixture、schema hash 和 CI consistency test 同步验证。

### 风险 4：修复循环扩大修改范围

控制：一次上限、same request/context、risk/domain/write scope guard、MAX_OPERATION_COUNT、重新 Import/Validate、人工确认。

### 风险 5：上下文过期

控制：editor revision + context hash；过期结果不进入 proposal。

### 风险 6：泄露密钥和本机信息

控制：env/host secret only；relative path context；Report redaction；限制 raw prompt/response artifact。

### 风险 7：OpenAI-compatible 实现差异

控制：默认 strict；不支持时明确失败。`json_object` 只能显式开启并标记 degraded，不静默降级。

### 风险 8：网络重试和 Patch 修复混淆

控制：transport retry 与 repair attempt 使用不同状态和计数，各自最多一次。

### 风险 9：AI Panel 变成长期聊天系统

控制：v1 只处理单次用户意图到 Patch proposal；不保存 provider conversation state。

## 21. 方案自检

### 21.1 是否符合用户选择

是。正式采用：

```text
B-min+：Guarded Thin Provider + One-shot Repair
```

### 21.2 是否 AI-first

是。Provider 输入、ProjectPatch 输出、diagnostics、repair decision、review 和 report 都是结构化、可验证、可审查的。

### 21.3 是否适配复杂项目

是。模型不绕过 207 的全域 capability；复杂项目继续使用相同 Patch Validator、domain service、transaction 和 rollback。

### 21.4 是否增加过多结构

否。没有 Provider Registry、Agent Planner、Repair Session truth 或独立 LLM Report。新增的数据只描述一次 pending request 和 attempt evidence。

### 21.5 是否保证编辑器响应

是。真实 HTTP 在后台 worker；winit 主线程只 pump result。

### 21.6 是否保持人工确认

是。initial 或 repaired candidate 最终都只能生成 `requires_confirmation = true` 的 proposal。

### 21.7 是否满足 Report 分档

是。Runtime Off；Editor 默认 Summary；Trace 仅显式诊断，并执行 secret/path redaction。

### 21.8 是否可以直接施工

方案自审已经完成，可以生成施工文档；施工文档完成自审前仍不可施工。

```text
本文件是正式方案。
没有发现适用于 238 的外部审查文档。
自审已补充 revision/context hash 正确判定和 v1 Chat Completions endpoint。
可以生成 238 当前自动化施工文档。
施工文档自审通过后，才能开始 Gate A。
```

## 22. 最终结论

正式采用：

```text
B-min+：ThinLlmPatchSource real HTTP
       + canonical strict ProjectPatch JSON Schema
       + sanitized structured authoring context
       + std::thread / mpsc background request
       + context revision/hash stale guard
       + one transient transport retry
       + one diagnostic-driven repair attempt
       + existing Import / Validator / Review / Confirm / Transaction
       + existing Project Patch Report Off/Summary/Trace
```

严格边界：

```text
LLM 只生成 candidate。
Validator 决定 candidate 是否有效。
Repair 最多一次且不能扩大 scope。
用户决定是否 Apply。
Transaction 决定修改是否提交或回滚。
```

完成入口：

```text
施工文档/已完成/238-当前可自动化施工文档-Real-LLM-Provider-Minimal-Repair-Loop-v1.md
阶段完成记录/2026-07-11-Real-LLM-Provider-Minimal-Repair-Loop-v1/00-总览.md
```

## 23. 参考

```text
191-Authoring-Walkthrough-Missing-Operations-Convergence-v1方案.md
205-AI-Project-Patch-Structured-Output-Imported-Patch-Productization-v2方案.md
206-LLM-Provider-Structured-Output-Integration-v3方案.md
207-ProjectPatch-All-Domain-Capability-v2方案.md
212-Report-Panel-Evidence-Panel-Productization-v1方案.md
227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md

rust/crates/editor_core/src/project_patch/llm_source.rs
rust/crates/editor_core/src/project_patch/import.rs
rust/crates/editor_core/src/project_patch/validator.rs
rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/project_patch/session.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/asset_browser.rs
rust/crates/editor_ui_model/src/ai_panel.rs
rust/crates/editor_ui_model/src/authoring_workflow.rs
rust/crates/editor_window_winit/src/application.rs

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Undo\Undo.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\AssetDatabase\Editor\ScriptBindings\AssetDatabase.bindings.cs

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Public\ScopedTransaction.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\ScopedTransaction.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\Editor\DataValidation\Source\DataValidation\Public\EditorValidatorSubsystem.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\Editor\DataValidation\Source\DataValidation\Private\EditorValidatorSubsystem.cpp

<GODOT_SOURCE>\godot\editor\editor_undo_redo_manager.h
<GODOT_SOURCE>\godot\editor\editor_undo_redo_manager.cpp

<UNITY_UI_REFERENCE>\com.oathx.unitycli-master

https://developers.openai.com/api/docs/guides/structured-outputs
https://developers.openai.com/api/docs/guides/function-calling
https://developers.openai.com/api/docs/guides/error-codes
https://docs.unity3d.com/ScriptReference/Undo.RecordObject.html
https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Editor/UnrealEd/FScopedTransaction
https://dev.epicgames.com/documentation/en-us/unreal-engine/data-validation-in-unreal-engine
https://docs.godotengine.org/en/stable/classes/class_editorundoredomanager.html
```
