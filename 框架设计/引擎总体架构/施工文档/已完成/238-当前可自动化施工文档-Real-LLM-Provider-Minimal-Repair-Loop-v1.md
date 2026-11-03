# 238-当前可自动化施工文档-Real LLM Provider / Minimal Repair Loop v1

> 状态：Gate A-G、整体回归、完成记录与入口同步均已完成；已归档。  
> 生成日期：2026-07-11。  
> 正式方案：`../../238-Real-LLM-Provider-Minimal-Repair-Loop-v1方案.md`。  
> 方案选择：用户已确认 `B-min+：Guarded Thin Provider + One-shot Repair`。  
> 前置完成：237 已完成 Gate A-F、整体回归、阶段完成记录与归档；生成本文时 `施工文档/当前/` 原为空。  
> 外部审查：未发现适用于 238 的其它 AI 审查文档；正式方案已完成本轮源码自审并修正 revision/context hash 与 Chat Completions endpoint。  
> 施工规则：一次只执行一个 Gate；当前 Gate 的实现、定向测试和证据全部通过后才能进入下一 Gate。

## 1. 施工目标

把当前链路：

```text
AI Panel fixed mock prompt
  -> ThinLlmPatchSource deterministic mock
  -> ProjectPatchImportService
  -> Patch review / user confirm / transaction
```

收敛为：

```text
AI Panel editable prompt
  -> GenerateProjectPatchFromPrompt
  -> sanitized ProjectPatchLlmContextSnapshot
  -> background OpenAI-compatible /chat/completions HTTP
  -> strict ProjectPatch JSON Schema output
  -> ProjectPatchImportService + PatchValidator
  -> eligible failure: one diagnostic-driven repair attempt
  -> ProjectPatchImportService + PatchValidator again
  -> context semantic hash stale guard
  -> PatchReviewModel(requires_confirmation=true)
  -> explicit user confirmation
  -> EditorSession::execute_patch_as_transaction
  -> existing project.patch Report Off/Summary/Trace evidence
```

最终必须同时证明：

```text
真实 HTTP provider 不再是 stub。
网络请求不阻塞 winit 主线程。
AI Panel 能输入真实 prompt、提交和取消。
Provider 输出受 canonical ProjectPatch schema 约束。
所有 candidate 都进入既有 Import / Validator。
可修复错误最多 repair 一次。
网络 retry 和 Patch repair 独立计数且各最多一次。
过期上下文不产生 proposal。
用户确认前项目和 EditorSession authoring truth 不变化。
API key、Authorization、绝对本机路径不进入 prompt/report/artifact。
正式 Runtime / Windows release 不初始化或链接 provider 工作流。
```

## 2. 施工前基线与审查结论

### 2.1 237 完成状态

权威证据：

```text
施工文档/已完成/237-当前可自动化施工文档-Release-Package-Polish-Metadata-Icon-Layout-v1.md
阶段完成记录/2026-07-11-Release-Package-Polish-Metadata-Icon-Layout-v1/00-总览.md
```

238 不修改 237 的 release contract、BuildProfile v2、PE 资源、RuntimePackage 或发布流程。

### 2.2 238 当前代码基线

```text
rust/crates/editor_core/src/project_patch/llm_source.rs
  Mock 已实现。
  OpenAiCompatible 仍返回 openai_compatible_not_implemented。
  Config 只有 source_kind/provider/model/timeout/enabled。
  Result 只用 raw_json/error_code/error_message，错误分类不足。

rust/crates/editor_core/src/services/ai_service.rs
  GenerateProjectPatchFromPrompt 当前同步调用 deterministic mock。
  project_patch_llm_context_summary 仍错误标记只支持 Scene/Input。
  candidate 成功后进入 ProjectPatchImportService。

rust/crates/editor_core/src/project_patch/import.rs
  已有 parse/schema-version/capability/validator/review 链。

rust/crates/editor_core/src/project_patch/validator.rs
  已有 MAX_OPERATION_COUNT=48 和七 domain validation。

rust/crates/editor_ui_renderer/src/panels/ai_panel.rs
  可见 Submit 仍使用 submit_mock_prompt。
  没有 prompt text field 或 Cancel。

rust/crates/editor_input/src/lib.rs
  submit_mock_prompt 硬编码 create AI Entity。

rust/crates/editor_window_winit/src/application.rs
  frame 已 pump Asset Browser worker，可复用同型 mpsc 模式。

rust/crates/editor_core/src/report_panel.rs
  已注册 project.patch；当前只汇总 PatchHistory。
```

### 2.3 方案自审采纳

正式方案无需改变 B-min+ 主体，但施工必须吸收：

```text
1. Generate 命令本身会推进一次 revision；revision 只触发 semantic context hash 重算，不能直接判 stale。
2. v1 固定 OpenAI-compatible <base>/chat/completions，不同时实现 Responses API。
3. AI Panel 必须提供最小可编辑 prompt，否则真实 provider 没有可用产品入口。
4. strict schema 从 Rust contract 生成；外部 editor_ui_model value types 只补 JsonSchema derive，不复制 schema model。
5. API key 不进入可 Serialize/Debug 明文结构；secret 必须专门 redacted。
6. initial 与 repaired candidate 都必须先 Import/Validate，禁止 repair 后直达 proposal/apply。
```

### 2.4 无外部审查输入

搜索 `其它AI审查目录/` 与 `4.8AI审查目录/`，未发现 238 / Real LLM Provider / Minimal Repair Loop 审查文档。

结论：

```text
没有外部审查要求修改正式方案。
施工按 238 自审修订后的 B-min+ 执行。
```

## 3. 固定边界

### 3.1 必须交付

```text
ProjectPatchLlmContextSnapshot + stable SHA-256 context/schema hash。
canonical ProjectPatch JSON Schema + strict normalization。
OpenAiCompatible Chat Completions HTTP。
HTTPS / loopback HTTP URL policy、redirect/timeout/body limit。
provider result typed classification。
最多一次 transient transport retry。
std::thread + mpsc background request/pump/logical cancel。
editable AI prompt draft + Submit/Cancel。
RepairDecision allowlist/denylist/scope/fingerprint。
最多一次 repair。
context semantic stale guard。
existing ProjectPatch Import/Review/Confirm/Transaction integration。
existing project.patch Report Off/Summary/Trace evidence。
deterministic mock、fake HTTP server、complex shooter E2E 和完整回归。
```

### 3.2 禁止扩入

```text
Provider Registry 或 dyn provider discovery。
多模型 fallback、投票、路由、负载均衡。
Agent Planner、任务图、工具调用、MCP、shell。
模型直接修改文件、EditorSession、ECS、RuntimePackage。
多轮 repair 或可配置无限循环。
Apply 后自动 repair。
自动 rebase / three-way merge。
并发 request queue。
token streaming UI。
长期 provider conversation memory。
prompt history 项目资产化。
Responses API 双协议。
Runtime/导出包内 provider。
图像/音频/3D provider。
OS credential vault 产品化。
真实公网 provider 作为默认测试依赖。
```

## 4. 依赖决策

施工时允许新增：

```toml
# editor_ui_model
schemars = "1.2"

# editor_core
schemars = "1.2"
sha2 = "0.10"
url = "2.5"
ureq = { version = "3.3", default-features = false, features = ["rustls", "json"] }
```

约束：

```text
使用 Cargo.lock 锁定实际版本。
ureq 禁用 gzip，避免解压后响应体绕过大小判断。
不引入 Tokio/async-std。
不新增 JSON Schema validator runtime；本地真相仍是 serde parse + PatchValidator。
strict schema 测试验证 generated schema shape/additionalProperties/required/nullable。
新增依赖 license 和 Rust 1.96 兼容性在 Gate A/B 记录。
```

## 5. 预计涉及文件

### 5.1 Cargo / Schema

```text
rust/Cargo.lock
rust/crates/editor_ui_model/Cargo.toml
rust/crates/editor_core/Cargo.toml

rust/crates/editor_ui_model/src/asset_browser.rs
rust/crates/editor_ui_model/src/input_mapping.rs
rust/crates/editor_ui_model/src/inspector.rs
rust/crates/editor_ui_model/src/viewport.rs

rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/project_patch/schema.rs（新增）
rust/crates/editor_core/src/project_patch/context.rs（新增）
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/lib.rs
```

### 5.2 Provider / Request / Repair

```text
rust/crates/editor_core/src/project_patch/llm_source.rs
rust/crates/editor_core/src/project_patch/llm_http.rs（新增）
rust/crates/editor_core/src/project_patch/llm_request.rs（新增）
rust/crates/editor_core/src/project_patch/llm_repair.rs（新增）
rust/crates/editor_core/src/project_patch/import.rs（只在需要统一 diagnostics helper 时修改）
rust/crates/editor_core/src/project_patch/validator.rs（不扩 domain，只暴露稳定 code 分类所需 helper）
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/session.rs
```

### 5.3 产品面 / Report

```text
rust/crates/editor_ui_model/src/ai_panel.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_ui_model/src/workflow_command.rs
rust/crates/editor_ui_model/src/manual_walkthrough.rs
rust/crates/editor_ui_model/src/lib.rs

rust/crates/editor_ui_renderer/src/draw_list.rs
rust/crates/editor_ui_renderer/src/panels/ai_panel.rs
rust/crates/editor_ui_renderer/src/tests.rs

rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/focus.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/command_system.rs
rust/crates/editor_window_winit/src/tests/native_app.rs

rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/ui_model_composer.rs
```

### 5.4 Tests / E2E / Docs

```text
rust/crates/editor_core/src/tests/llm_patch_source_tests.rs
rust/crates/editor_core/src/tests/ai_service_tests.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/project_e2e_gate/src/project_patch.rs
rust/crates/project_e2e_gate/tests/（仅在需要进程级 fixture 时新增）

框架设计/引擎总体架构/238-Real-LLM-Provider-Minimal-Repair-Loop-v1方案.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

列表是允许范围，不代表每个文件必须修改。施工不得为“匹配列表”制造无意义 churn。

## 6. Gate A：Contract / Context / Schema

### 6.1 任务

```text
1. 为 ProjectPatchDocument、PatchOperation、七 domain operation 和必要 value type 增加 JsonSchema derive。
2. 只给 AssetKind / AssetPlacementMode / InputActionValueKind / Vec3 增加共享 derive；禁止复制 mirror enum。
3. 新增 canonical schema 生成函数和 deterministic strict normalization：
   - object additionalProperties=false
   - object required 覆盖所有 properties
   - 原 Option schema 保留 null
   - 清理 provider 不支持的顶层 metadata
4. canonical JSON 序列化后用 SHA-256 生成 schema_hash。
5. 新增 ProjectPatchLlmContextSnapshot：只含 project-relative path、稳定 id、AuthoringAiContext 摘要、capability、MAX_OPERATION_COUNT 和 schema hash。
6. 修正旧 Scene/Input-only 能力摘要为七 domain。
7. semantic context hash 不包含 request_id、AI Panel messages、Report refresh timestamp、绝对路径。
8. expected_post_start_revision 只作快速信号；revision 变化时重捕获 snapshot 并比较 semantic hash。
```

### 6.2 测试

```powershell
cargo test -p editor_ui_model json_schema
cargo test -p editor_core llm_patch_schema
cargo test -p editor_core llm_patch_context
cargo test -p editor_core llm_patch_source_prompt
```

### 6.3 通过条件

```text
所有七 domain operation 出现在生成 schema。
未知字段在 strict schema 中被 additionalProperties=false 拒绝。
所有 object properties 都 required；Option 对应 nullable。
schema hash 重复生成一致。
context JSON 不包含 G:\、I:\、用户名、API key 或 project_root absolute path。
context capability 为 Scene/Input/Asset/Prefab/AUI/Rule/Build。
无关 AI Panel/Report 状态变化不改变 semantic context hash。
Scene/selection/active authoring context 变化会改变 hash。
```

### 6.4 失败即停

```text
若 schemars 无法准确表达当前 serde tagged enum，先修 canonical contract/schema adapter，不允许手写第二份完整 ProjectPatch schema。
若 shared value type derive 扩散到无关 crate，缩小 derive 范围，不新建 schema crate。
```

## 7. Gate B：Real HTTP Transport

### 7.1 任务

```text
1. 扩展 LlmPatchSourceConfig：base_url、structured_output_mode、timeout/body limits、retry policy。
2. API key 使用不可 Serialize 的 redacted secret wrapper；Debug 永不输出原值。
3. v1 endpoint 固定 <base>/chat/completions。
4. URL policy：
   - https 允许
   - http 只允许 localhost/127.0.0.0/8/::1
   - 禁止 username/password URL
   - 禁止跨 origin redirect；优先禁用自动 redirect
5. strict request：response_format.type=json_schema，strict=true，schema=Gate A 产物。
6. json_object 显式降级模式：response_format.type=json_object，并在 result/report 标记 degraded。
7. 解析 choices[0].message.content/refusal/finish_reason；content 必须是单个 JSON object string。
8. 响应体 hard limit 2 MiB；request/candidate/diagnostic 也设硬上限。
9. typed provider status：Success/Refused/Cancelled/TimedOut/Transport/4xx/5xx/RateLimited/Auth/Unsupported/TooLarge/Empty/Invalid。
10. transient retry 最多一次：短暂连接错误、受限 429、500/502/503/504；401/403/400/404/refusal/cancel 不 retry。
11. latency、http status class、transport_attempt_count 进入轻量 result；header/key 不进入 result。
```

### 7.2 Fake Server

测试只用 `std::net::TcpListener` 本地 loopback fixture，覆盖：

```text
200 strict success。
200 refusal。
200 empty/malformed envelope。
200 body > 2 MiB。
400 structured output unsupported。
401/403 auth failure。
429 Retry-After within cap / beyond cap。
500 then 200：只 retry 一次。
redirect：拒绝。
timeout。
```

### 7.3 测试

```powershell
cargo test -p editor_core llm_http
cargo test -p editor_core llm_patch_source_openai
cargo test -p editor_core llm_provider_redaction
```

### 7.4 通过条件

```text
OpenAiCompatible 不再返回 not_implemented。
fake server 能断言 Authorization、model、messages、response_format 和 schema hash 对应内容。
非 loopback HTTP 在发送前拒绝。
secret 不出现在 Debug/Serialize/error/report。
每种错误有稳定 code 和 next action。
transport retry_count <= 1。
```

## 8. Gate C：Background Request / Prompt UI / Cancel

### 8.1 任务

```text
1. 新增单 active request state：request_id、attempt kind/index、expected revision、context hash、receiver、logical generation/cancel token、timing。
2. GenerateProjectPatchFromPrompt：
   - 校验 prompt 非空和无 active request
   - 捕获 context/schema
   - AI Panel busy=true / stage=Generating
   - std::thread::spawn HTTP
   - 立即提交命令，不等待网络
3. NativeEditorApplication::frame 调用 pump_llm_patch_request；pending 时持续 request_redraw。
4. worker 只持有 owned request/config/context；禁止借用或修改 EditorSession。
5. 新增 CancelLlmPatchRequest；取消后 busy=false，迟到 generation result 丢弃。
6. AI Panel 增加 prompt_draft 与 stage/status summary。
7. AI Panel 绘制真实 prompt field、Submit 或 Cancel；Submit 不再使用 submit_mock_prompt。
8. 输入路由复用 Asset Browser search 的 TextField 模式：字符、Space、Backspace、Escape、Enter。
9. SetAiPromptDraft 是结构化 UiCommand；Enter/Submit 降低为 GenerateProjectPatchFromPrompt，Cancel 降低为 CancelLlmPatchRequest。
10. 每个 EditorSession 只允许一个 active request；第二次提交结构化拒绝。
```

### 8.2 测试

```powershell
cargo test -p editor_ui_model ai_panel_prompt
cargo test -p editor_ui_renderer ai_panel
cargo test -p editor_input ai_panel
cargo test -p editor_window_winit ai_panel
cargo test -p editor_core llm_background_request
cargo test -p editor_core llm_cancel
```

### 8.3 通过条件

```text
提交后命令在 HTTP fixture 阻塞期间立即返回，编辑器 frame_index 继续增长。
busy/stage 在 start/pump/result/cancel 间正确变化。
取消后迟到 response 不产生 proposal、message 或 repair。
AI Panel 可以输入非硬编码 prompt。
Submit busy 时变为 Cancel 或禁用重复提交。
headless EditorSession 可通过 pump helper 确定性完成请求。
```

## 9. Gate D：One-shot Repair

### 9.1 任务

```text
1. 把 provider candidate 先交给 ProjectPatchImportService，统一收集 schema/capability/validation diagnostics。
2. RepairDecision 只接受稳定 code allowlist，不用 message contains。
3. v1 allowlist 起点：
   project_patch_import.parse_failed
   project_patch.operation_id_required
   project_patch.operation_id_duplicate
   project_patch.dependency_missing
   project_patch.scene.component_field_invalid
   project_patch.prefab.stage_field_invalid
   project_patch.aui.node_field_invalid
   project_patch.rule.payload_invalid
4. denylist：provider/config/network/refusal/stale、capability_unknown、gameplay_api_forbidden、operations_too_many、path escape/invalid root、未知 code。
5. Repair request 使用同一 user prompt/context/schema，附 bounded candidate 和 redacted diagnostics。
6. repair attempt 固定最多一次，不提供环境变量扩大次数。
7. scope guard：schema/context/request 不变、risk 不升高、capability 不引入未知、operation<=48、不得新增 destructive/Build domain。
8. diagnostic fingerprint=sorted(code,operation_id,target) SHA-256；第二次相同则 no_progress。
9. repaired candidate 再次完整 Import/Validate；失败后终止，不生成可 Apply proposal。
10. Apply/transaction failure 绝不回到 repair。
```

### 9.2 Deterministic Fixtures

```text
mock initial invalid JSON -> repair valid Scene patch。
mock duplicate operation id -> repair unique ids。
mock denylist operations_too_many -> zero repair calls。
mock repair returns same failure -> no_progress。
mock repair expands risk/domain -> scope_expanded。
mock repair second invalid -> attempt_limit_reached。
```

### 9.3 测试

```powershell
cargo test -p editor_core llm_repair_decision
cargo test -p editor_core llm_repair_scope
cargo test -p editor_core llm_repair_loop
cargo test -p editor_core project_patch_import
cargo test -p editor_core project_patch_validator
```

### 9.4 通过条件

```text
任何路径 repair_attempt_count <= 1。
denylist 从不触发 provider repair request。
initial/repaired 两次都留下可审查 attempt summary。
只有最终 accepted candidate 可进入 proposal。
修复不能绕过 MAX_OPERATION_COUNT、path guard 或 gameplay API guard。
```

## 10. Gate E：Review / Confirm / Report Integration

### 10.1 任务

```text
1. 结果回主线程时执行 semantic context stale guard；revision 变化但 hash 相同可继续。
2. accepted initial/repaired candidate 统一 stage 为 imported ProjectPatch proposal。
3. PatchReviewModel.requires_confirmation 保持 true。
4. AI Panel proposal 显示 touched domains、operation count、risk、validation、repaired_once。
5. 失败时不造 Apply command，只显示自然语言摘要 + stable diagnostics。
6. 新增 LlmPatchReportLevel Off/Summary/Trace，Editor 默认 Summary。
7. 不新增 report provider；扩展现有 project.patch：
   Summary：provider/model/mode/status/attempts/latency/hash/codes
   Trace：status class、per-attempt latency、context/schema hash、bounded redacted details
8. Report/AI context 禁止 key/header/raw env/absolute path；默认不保存完整 prompt/raw response。
9. Runtime/export code path不引用 session/provider state；仅 editor_core/editor UI 使用。
```

### 10.2 测试

```powershell
cargo test -p editor_core llm_context_stale
cargo test -p editor_core ai_project_patch
cargo test -p editor_core report_panel
cargo test -p editor_ui_model ai_panel
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core manual_walkthrough
```

### 10.3 通过条件

```text
stale hash 不创建 proposal、不 repair、不 apply。
无关 revision 变化且 hash 相同仍可创建 proposal。
用户确认前 Scene/Rule/AUI/Prefab/Asset/Build authoring truth 不变化。
接受后仍进入 execute_patch_as_transaction。
Off 不展示 provider evidence；Summary/Trace 分档正确。
搜索 report/model/debug output 找不到测试 secret 和绝对路径 canary。
```

## 11. Gate F：Complex Shooter E2E

### 11.1 场景

```text
A. fake real provider 生成合法 Scene.CreateEntity；review 后确认并 committed。
B. fake provider 首次 Rule payload_invalid，repair 一次后 accepted；review 标记 repaired_once。
C. all-domain Asset/Prefab/AUI/Rule/Build candidate 进入 207 全域 Import/Validator，确认前无修改。
D. 401/refusal/timeout 不 repair、不 proposal。
E. 请求期间修改 selected entity / active scene，result stale。
F. logical cancel 丢弃迟到响应。
G. Report Summary/Trace 结构化且 redacted。
```

### 11.2 测试

```powershell
cargo test -p project_e2e_gate project_patch
cargo test -p project_e2e_gate llm_provider_repair
cargo test -p editor_core project_patch
cargo test -p editor_window_winit ai_panel
```

### 11.3 可选真实 Provider Smoke

```powershell
cargo test -p editor_core llm_real_patch_source -- --ignored
```

规则：

```text
只有 AI_ENGINE_LLM_PATCH_SOURCE/openai-compatible config/key 显式存在时运行。
失败不允许通过打印 key/raw response 诊断。
该 ignored/local-only smoke 不是默认 Gate 通过条件。
默认完成证据必须由 fake HTTP deterministic E2E 提供。
```

## 12. Gate G：整体回归、文档与归档

### 12.1 全量回归

每条独立运行并记录真实通过数：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_input
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit
cargo test -p project_e2e_gate
```

重点历史回归：

```powershell
cargo test -p editor_core project_patch
cargo test -p editor_core report_panel
cargo test -p editor_core authoring_workflow
cargo test -p editor_core manual_walkthrough
cargo test -p editor_core release_package
cargo test -p editor_core desktop_export
cargo test -p runtime_cli
cargo test -p engine_runtime
```

### 12.2 文档同步

全部通过后：

```text
238 正式方案状态改为已施工完成。
生成 阶段完成记录/2026-07-11-Real-LLM-Provider-Minimal-Repair-Loop-v1/00-总览.md。
49 记录真实 provider/repair 完成与 residual deferred。
54 当前施工入口清空并登记 238 最近完成。
227 P3-1 标为完成。
00 文档地图保持 238 有效入口。
施工文档/README.md 更新当前/最近完成。
阶段完成记录/README.md 增加 238。
本文从 当前/ 移动到 已完成/。
```

### 12.3 归档条件

```text
Gate A-F 全部实现和定向测试通过。
Gate G 全量回归通过。
默认测试不访问公网。
没有 active LLM worker/process。
没有 API key/prompt/raw response/absolute path artifact 污染。
完成记录包含依赖版本、测试通过数、ignored smoke 状态和残留边界。
```

任一条件不满足时，本文保持 `当前/`，不得生成完成结论。

## 13. Diagnostic 与 Evidence 最低合同

Provider diagnostic 至少包含：

```text
code
provider_id
model（非 secret）
attempt_kind / attempt_index
stage
message（redacted）
http_status_class（可选）
next_action
```

Repair evidence 至少包含：

```text
initial/final candidate hash
initial/final diagnostic fingerprint
repair decision
repair attempt count
scope guard result
final import/validation status
```

禁止证据字段：

```text
api_key
authorization_header
full environment
absolute project root
unredacted raw prompt/response by default
```

## 14. 工作树与安全规则

当前仓库存在大量用户已有改动和已完成系统文件。施工期间必须：

```text
不 reset、checkout、clean 或回退无关改动。
修改重叠文件前先读当前内容，基于当前状态 apply_patch。
Cargo.lock 只接受本轮新增依赖的机械变化，不整理无关版本。
fake HTTP 只监听 loopback，由测试持有并在结束时释放。
不启动公网请求作为默认测试。
不把 env secret 写入 fixture、snapshot、panic、Debug 或 report。
不修改 samples/complex_shooter_project 的 release/source asset，除非 Gate F 明确需要结构化 fixture 且经过 containment 检查。
```

## 15. 施工文档自审清单

生成后必须逐项确认：

```text
[x] 当前只有 238 一份施工文档。
[x] 正式方案已完成自审且没有未处理外部审查。
[x] Gate A-G 与正式方案边界一致。
[x] AI Panel prompt 输入是现有命令链的产品化，不是新增聊天架构。
[x] Provider 只生成 candidate，不持有写权限。
[x] initial/repaired candidate 都走 Import/Validator。
[x] repair 和 transport retry 各最多一次。
[x] stale 判定使用 semantic hash，避免 start command revision 自误判。
[x] 每 Gate 有精确测试命令、通过条件和失败即停。
[x] 默认测试不依赖真实 key/公网。
[x] Report 复用 project.patch 且 Off/Summary/Trace 分档。
[x] Runtime/release provider Off 且不进入运行路径。
[x] 归档需要完整回归和完成记录。
```

## 16. 当前自审结论

```text
通过。

1. 当前目录只有 238，237 已完成记录并归档，不存在并行施工文档冲突。
2. Gate A-G 与 238 B-min+ 正式方案一致，没有引入 Provider Registry、Agent Planner、对话系统或第二条写盘链路。
3. Provider 只产出候选 ProjectPatch；初次与修复候选均必须经过 ImportService、PatchValidator、Review、用户确认和既有事务 Apply。
4. transport retry 与 diagnostics-driven repair 分别最多一次；stale guard 按 expected_post_start_revision 触发语义重采样，只以 context_hash 变化拒绝候选。
5. 每个 Gate 都有定向命令、通过条件和失败即停；默认测试使用 loopback fake HTTP，不依赖公网或真实 API key。
6. secret、prompt、raw response 和绝对路径均受脱敏约束；runtime/release provider 默认 Off，Report 继续复用 project.patch 的 Off/Summary/Trace 分档。
7. 只有 Gate A-F、Gate G 全量回归、完成记录和入口同步全部完成后才允许归档。
```

施工文档自审已通过；施工时已严格按 Gate A-G 串行执行，没有并行修改 237。

## 17. 施工完成记录

### 17.1 Gate A-F

```text
Gate A：canonical ProjectPatch schema、schema hash、sanitized semantic context 与 stale guard 已完成。
Gate B：OpenAI-compatible Chat Completions HTTP、HTTPS/loopback policy、body limit、redirect guard 与一次 transient retry 已完成。
Gate C：AI prompt、background worker、frame pump、Cancel 与每 EditorSession 单 active request 已完成。
Gate D：diagnostic allowlist、一次 repair、scope/risk/capability/no-progress guard 已完成。
Gate E：initial/repaired candidate 均走 ImportService/Validator/Review/Confirm/transaction；project.patch Off/Summary/Trace evidence 已完成。
Gate F：complex shooter 本地 fake HTTP E2E 已覆盖 repair 与 Asset/Prefab/AUI/Rule/Build 全域 candidate。
```

### 17.2 Gate G 回归

```text
cargo fmt --all -- --check：passed
editor_ui_model：54 passed
editor_core：358 passed，1 ignored（env-gated optional real-provider smoke）
editor_input：30 passed
editor_ui_renderer：13 passed
editor_window_winit：87 passed
project_e2e_gate：42 unit + 1 release integration + 3 consistency integrations passed
runtime_cli：22 unit + 4 integration passed
engine_runtime：426 passed

历史重点回归：
project_patch 39 passed；report_panel 10 passed；authoring_workflow 9 passed；manual_walkthrough 2 passed；
release_package 7 passed；desktop_export 5 passed。
```

### 17.3 最终卫生检查

```text
默认测试不访问公网；真实 provider smoke 保持 ignored + env-gated + local-only。
未发现真实 API key、Authorization header、prompt/raw response 或绝对本机路径 artifact。
未发现 not_implemented、TODO、FIXME、todo! 或 unimplemented! 遗留在 238 实现范围。
没有活动测试命令、LLM worker 或 fake HTTP server 遗留。
```

### 17.4 归档结论

Gate A-G 全部通过。238 可以移入 `施工文档/已完成/`，并以阶段完成记录作为后续追溯入口。

### 17.5 最终完成审计补充

归档后的独立完成审计又执行了 `cargo test --workspace`，并修复两类此前分 crate 回归没有覆盖的测试基线：

```text
editor_ui_backend_egui 的 AiPanelModel fixture 补齐 238 新增的 prompt_draft / stage / status_summary。
runtime_cli 的两个 feature-disabled 断言改为只在 not(feature = "real-window") 时执行，避免 workspace feature unification 产生错误预期。
```

补充验证：

```text
cargo test -p editor_ui_backend_egui：3 passed。
cargo test -p runtime_cli --lib：22 passed。
cargo test -p runtime_cli --all-features --lib：21 passed。
cargo test -p runtime_cli --test exported_player_process_verification：4 passed。
cargo test -p runtime_cli --all-features --test exported_player_process_verification：3 passed。
cargo fmt --all -- --check：passed。
cargo test --workspace：passed；仅保留既有 ignored local-only OS window/GPU smoke。
```

最终完成审计没有改变 238 架构边界，也没有修改 237 实现或文档。
