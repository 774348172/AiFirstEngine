# 206-LLM Provider / Structured Output Integration v3 方案

## 1. 系统是什么

本系统正式收敛为：

```text
Thin LLM Patch Source v3
```

一句话说明：

```text
它不是新建一套 LLM 子系统，而是让真实 LLM 成为 ProjectPatchDocument JSON 的一个薄输入来源。
```

用户心智只保留这一条：

```text
AI 生成 ProjectPatch
  -> ProjectPatch 导入 / 校验 / 审阅 / 用户确认 / 执行
```

真实工程链路：

```text
User Prompt
  -> ai_service 组装 ProjectPatch 生成提示
  -> Thin LLM Patch Source
  -> raw ProjectPatchDocument JSON 或 provider error
  -> ProjectPatchImportRequest::ai_structured_output(raw_json)
  -> ProjectPatchImportService
  -> PatchReviewModel / AI Panel proposal
  -> User Confirm
  -> EditorSession::execute_patch_as_transaction
  -> PatchApplyReport / PatchHistorySummary / ProjectPatchImportProductizationReport
```

本系统不是：

```text
不是 Provider Registry。
不是 Agent Planner。
不是 Repair Loop。
不是独立 LLM Report 真相层。
不是让 LLM 直接写文件。
不是让 LLM 绕过 ProjectPatchImportService。
不是把真实 provider 变成默认 CI 依赖。
不是扩 Asset / Prefab / AUI / Rule / Build patch capability。
```

## 2. 为什么要从 B-min 改成 B-lite

原 206 的 B-min 方向是对的：

```text
真实 LLM 输出必须是结构化 ProjectPatchDocument。
输出必须进入 205 的 ProjectPatch import / validate / review / apply 链路。
真实 provider 不能污染默认测试。
```

但原 B-min 把实现拆得过厚：

```text
LlmStructuredOutputRequest
LlmStructuredOutputResult
LlmStructuredOutputReport
LlmProvider trait
MockLlmProvider
schema artifact
optional real provider smoke
```

这些名词如果全部产品化，会造成新的心智层：

```text
用户 / AI 需要判断真相是 LLM Report、ProjectPatchImportResult、PatchReviewModel 还是 PatchApplyReport。
```

这违反当前项目规则：

```text
不要为了解决结构复杂，再新增一层结构。
Feature / Patch / Report 应减少判断成本，而不是制造新的长期系统。
```

因此 206 改为 B-lite：

```text
只加一个薄的 LLM Patch Source。
只返回 raw_json / error / provider metadata。
所有 parse / schema / validation / review / apply / report 继续复用 205。
```

## 3. 其它成熟工具 / 引擎对标

### 3.1 OpenAI Structured Outputs

官方资料：

```text
https://platform.openai.com/docs/guides/structured-outputs
https://openai.com/index/introducing-structured-outputs-in-the-api/
```

可学习点：

```text
模型输出应受 JSON Schema 约束。
工程侧不应靠自然语言解析。
即使 provider 支持 structured output，应用侧仍要保留 parse / schema / validation。
```

不照搬：

```text
不把 provider schema adherence 当成项目修改正确性。
最终真相仍是 ProjectPatchImportService / PatchValidator / EditorSession transaction。
```

### 3.2 LangChain / Semantic Kernel

官方资料：

```text
https://docs.langchain.com/oss/python/langchain/structured-output
https://docs.langchain.com/oss/javascript/langchain/structured-output
https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/structured-outputs
```

可学习点：

```text
不同模型可以有不同 structured output 能力。
provider error / refusal / invalid output 应该可诊断。
```

不照搬：

```text
不引入通用 agent framework。
不把 LangChain / Semantic Kernel 的 planner、tool loop、memory 搬进 editor_core。
```

### 3.3 Unity AI Assistant

官方资料：

```text
https://docs.unity3d.com/Packages/com.unity.ai.assistant@latest/
https://docs.unity3d.com/6000.4/Documentation/Manual/ai-menu-access.html
https://unity.com/blog/unity-ai-assistant-ask-plan-agent-mode-explained
```

可学习点：

```text
AI 应接入编辑器上下文。
AI 操作应有 Ask / Plan / Agent 等层次。
```

不照搬：

```text
本轮不做 Agent Mode。
不采用“AI 生成代码并直接改大量文件”作为默认路线。
本项目优先让 AI 生成 ProjectPatchDocument。
```

### 3.4 Unity / UE 编辑事务源码参考

本项目已有源码参考文档：

```text
框架设计/Unity源码参考/AI-Project-Patch-EditorTransaction源码参考.md
框架设计/UE源码参考/AI-Project-Patch-EditorTransaction源码参考.md
```

Unity 对标：

```text
Editor Tool / Inspector
  -> SerializedObject / SerializedProperty
  -> Undo / Dirty
  -> ApplyModifiedProperties
```

UE 对标：

```text
Editor Command / Tool
  -> FScopedTransaction
  -> UObject::Modify
  -> mutation
  -> MarkPackageDirty
```

对 206 的结论：

```text
LLM 只能产生编辑意图。
真实修改必须进入 ProjectPatch import / validator / editor transaction。
```

## 4. 在本引擎中的作用

当前主线目标：

```text
用户 / AI 能在编辑器里制作复杂打飞机项目，并导出 Windows 可玩。
```

已完成：

```text
202 ProjectPatch Productization v1：
  AI proposal 暴露 ProjectPatch evidence，accept 后进入 execute_patch_as_transaction。

205 Imported Patch v2：
  外部 ProjectPatchDocument JSON 可导入、parse、validate、review、apply、report。
```

当前缺口：

```text
AI Panel 仍没有真实 LLM 生成 ProjectPatchDocument JSON 的入口。
ProjectPatchImportRequest 已有 AiStructuredOutput source kind，但没有 provider 生产 raw_json。
```

206 只补这个缺口：

```text
让 LLM 生成 ProjectPatchDocument JSON。
把它当作 205 import 的 raw_json 输入。
如果失败，只产生 provider diagnostic，不产生新的 LLM 真相层。
```

## 5. 当前代码基线

当前已具备：

```text
rust/crates/editor_core/src/project_patch/model.rs
  ProjectPatchDocument
  ProjectPatchImportRequest
  ProjectPatchImportSourceKind::AiStructuredOutput
  ProjectPatchImportResult
  ProjectPatchImportProductizationReport

rust/crates/editor_core/src/project_patch/import.rs
  ProjectPatchImportService::from_json_string / from_file / from_fixture

rust/crates/editor_core/src/services/ai_service.rs
  submit_ai_prompt / plan_ai_response
  import_project_patch / preview_imported_project_patch / apply_imported_project_patch

rust/crates/project_e2e_gate/src/project_patch.rs
  complex shooter ProjectPatch smoke
  complex shooter imported patch smoke
```

当前真实支持：

```text
Scene / Input ProjectPatch apply。
Asset / Prefab / AUI / Rule / Build capability 仍 unsupported / deferred。
真实 LLM provider 未实现。
```

## 5.1 审查采纳结论

已读取：

```text
其它AI审查目录/26-206-LLM-Provider-Structured-Output方案审查.md
```

审查结论：

```text
方案 B-lite 方向正确，可直接进入施工。
无需推翻方案。
```

采纳的施工修正：

```text
1. 代码落点放在 editor_core/src/project_patch/llm_source.rs，与 import.rs 平行。
2. Mock source 必须 deterministic：
   - prompt 包含 create / 创建 / 新建 -> 生成 Scene.CreateEntity ProjectPatch JSON。
   - prompt 包含 invalid_json -> 返回非法 JSON。
   - prompt 包含 provider_error -> 返回 provider error diagnostic。
   - prompt 包含 unsupported / aui / rule / prefab -> 生成 required_capabilities 含 unsupported 的 patch，用于 capability diagnostics。
3. Prompt 组装必须明确当前只支持 Scene/Input capability，避免模型误生成 Asset / Prefab / AUI / Rule / Build patch。
4. 真实 provider HTTP 本轮不引入 reqwest / ureq 等依赖；只保留 env-gated optional stub / skipped smoke，避免施工范围扩大。
5. 不新增 LlmStructuredOutputReport、Provider Registry、Agent Planner、Repair Loop。
```

## 6. 方案选项

### 6.1 方案 A：AI Panel 直接调用真实 LLM

做法：

```text
AI Panel submit prompt 后直接请求真实模型。
模型返回 ProjectPatchDocument JSON。
立刻进入 ProjectPatchImportService。
```

优点：

```text
最少代码。
用户体验直接。
```

缺点：

```text
provider / prompt / schema / import / validate / apply 错误混在一起。
容易把 AI Panel 写死到一个 provider。
真实 provider 容易污染默认测试。
```

结论：

```text
不采用。
```

### 6.2 方案 B-min：Provider Adapter 子系统

做法：

```text
新增 LlmStructuredOutputRequest / Result / Report。
新增 LlmProvider trait / MockLlmProvider / optional real provider。
新增 LLM structured output e2e report。
```

优点：

```text
边界完整。
provider 可替换。
诊断信息充分。
```

缺点：

```text
新增结构太多。
容易形成第二套报告真相。
用户 / AI 心智变重。
后续维护会围绕 LLM 子系统扩张。
```

结论：

```text
不采用原 B-min，收敛为 B-lite。
```

### 6.3 方案 B-lite：Thin LLM Patch Source

做法：

```text
在 ai_service 附近新增一个薄的 LLM patch source。
输入：用户 prompt + 当前可用 ProjectPatch schema/能力摘要。
输出：raw ProjectPatchDocument JSON，或 provider error diagnostic。
成功时立刻封装为 ProjectPatchImportRequest::ai_structured_output。
后续全部交给 ProjectPatchImportService。
```

只允许新增极少结构：

```text
LlmPatchSourceConfig
LlmPatchSourceResult
ThinLlmPatchSource / 或等价函数
```

建议最小结构：

```text
LlmPatchSourceResult
  provider_id
  model
  raw_json
  error_code
  error_message
  latency_ms
```

如果为了更轻，也可以先不公开 trait，只做：

```text
generate_project_patch_json_from_prompt(...)
```

正式代码落点：

```text
rust/crates/editor_core/src/project_patch/llm_source.rs
```

说明：

```text
放在 project_patch 模块下，是因为 LLM patch source 只是 ProjectPatch JSON 来源；
不放成 editor-wide provider subsystem。
```

优点：

```text
最符合当前架构：LLM 只是 ProjectPatch 的输入来源。
不新增 report 真相层。
不新增 Agent / Provider Registry。
默认测试仍可用 deterministic mock。
真实 provider 可 optional / local-only。
```

缺点：

```text
provider 替换能力较弱。
复杂 provider 路由、repair loop、multi-model strategy 需要后续重新讨论。
```

结论：

```text
采用。
```

### 6.4 方案 C：完整 Agent Planner / Repair Loop

做法：

```text
模型读取上下文。
生成 ProjectPatch。
自动根据 diagnostics 修复 patch。
多轮 validate / retry。
必要时跨 Asset / Prefab / AUI / Rule / Build 多域修改。
```

优点：

```text
长期体验最好。
```

缺点：

```text
会新增 Agent state / planner / tool registry / retry policy / context manager / permission manager。
比 B-min 更重。
容易把 205 的稳定 import 链路包进黑盒。
```

结论：

```text
不采用本轮。
```

## 7. 推荐方案

采用：

```text
方案 B-lite：Thin LLM Patch Source
```

过滤依据：

### 7.1 AI 适配性

通过。

```text
LLM 输出仍是 ProjectPatchDocument JSON。
ProjectPatchImportService 继续做 parse / schema / capability / validation / review。
AI 和用户看到的真相仍是 ProjectPatch proposal 和 import diagnostics。
```

### 7.2 复杂项目适配与可维护

通过。

```text
复杂打飞机 / 自走棋需要 AI 批量改项目。
但复杂项目不能让 AI 直接写文件或生成自由脚本。
B-lite 把 LLM 限制为“patch source”，不会引入新的业务层。
```

### 7.3 效率

通过。

```text
复用 205。
新增代码少。
默认测试可 deterministic。
真实 provider optional / local-only。
```

### 7.4 结构复杂度

优于 B-min 和 C。

```text
B-lite 不新增独立 LLM report。
B-lite 不新增 provider registry。
B-lite 不新增 agent loop。
B-lite 不改变 runtime 链路。
```

## 8. v3 正式边界

### 8.1 v3 要做

```text
在 editor_core AI service 附近新增 Thin LLM Patch Source。
支持 mock source：deterministic 输出合法 / 非法 ProjectPatch JSON。
支持 optional real source stub：feature/env/local-only/skipped，不引入 HTTP 依赖。
AI Panel 新增或复用“生成 ProjectPatch”命令。
生成成功后立即进入 ProjectPatchImportRequest::ai_structured_output。
导入结果复用 ProjectPatchImportResult / ProjectPatchImportProductizationReport。
ManualWalkthrough / AuthoringAiContext 只暴露“LLM patch source available / next_action”，不新增 LLM 子系统视图。
project_e2e_gate 默认用 mock source 验证 complex shooter LLM patch source smoke。
```

### 8.2 v3 不做

```text
不新增 LlmStructuredOutputReport。
不新增 Provider Registry。
不新增 Agent Planner。
不新增 Repair Loop。
不自动 apply。
不扩 Asset / Prefab / AUI / Rule / Build patch capability。
不接图像 / 音频 / 3D 生成 provider。
不把真实 provider 作为默认 CI gate。
不保存 API key 到项目文件。
不让 provider 读取任意本地文件。
```

### 8.3 真实 provider 边界

第一版真实 provider 只做边界预留，不做真实 HTTP 调用。

真实 provider 边界只能是：

```text
optional
local-only
explicit env enabled
timeout bounded
redacted diagnostics
never required by default tests
no new HTTP dependency in this stage
```

建议配置：

```text
AI_ENGINE_LLM_PATCH_SOURCE=mock | openai_compatible
AI_ENGINE_LLM_BASE_URL
AI_ENGINE_LLM_MODEL
AI_ENGINE_LLM_API_KEY
AI_ENGINE_LLM_TIMEOUT_MS
```

报告规则：

```text
可以记录 provider_id / model / latency_ms。
不能记录 API key。
raw output 只作为 ProjectPatchImportRequest.raw_json 进入 import。
如果要写 artifact，必须明确 local-only 或 redacted。
```

本轮施工裁定：

```text
不引入 reqwest / ureq / async runtime。
OpenAiCompatible 只返回 skipped / not_implemented diagnostic。
真实 HTTP provider 等用户单独确认后再讨论。
```

## 9. 最小数据结构

### 9.1 LlmPatchSourceConfig

```text
LlmPatchSourceConfig
  source_kind: Mock | OpenAiCompatible
  provider_id
  model
  timeout_ms
  enabled
```

说明：

```text
这只是 editor_core 内部配置，不是新的用户资产。
```

### 9.2 LlmPatchSourceResult

```text
LlmPatchSourceResult
  provider_id
  model
  raw_json
  error_code
  error_message
  latency_ms
```

规则：

```text
raw_json 有值时，必须立刻进入 ProjectPatchImportRequest::ai_structured_output。
error_code 有值时，转成 CommandResult diagnostics / AI Panel message。
```

### 9.3 不新增的结构

本轮明确不新增：

```text
LlmStructuredOutputRequest
LlmStructuredOutputResult
LlmStructuredOutputReport
Provider Registry
Tool Registry
Agent Trace
Repair Session
```

### 9.4 Prompt 组装规则

Thin source prompt 必须包含：

```text
ProjectPatchDocument 输出要求。
当前 schema_version = project-patch.v1。
当前 only supported capabilities = Scene / Input。
禁止生成 Asset / Prefab / AUI / Rule / Build operation。
禁止生成 Player / Enemy / Bullet 等玩法专用 operation。
用户自然语言需求。
当前 editor context 摘要，例如 selected entity / opened project / opened scene。
```

Prompt 组装不是新的 Prompt Template 系统。

它只是 `llm_source.rs` 内部的 deterministic helper：

```text
build_project_patch_generation_prompt(...)
```

后续如果需要完整 prompt 管理，再单独讨论。

## 10. 与现有系统关系

### 10.1 与 205

```text
205 是正式结构化 ProjectPatch import 链路。
206 只是给 205 提供 AiStructuredOutput raw_json。
```

206 不重写：

```text
ProjectPatchImportService
PatchValidator
PatchReviewModel
EditorSession::execute_patch_as_transaction
ProjectPatchImportProductizationReport
```

206 只补：

```text
thin llm patch source
mock source
optional real source
AI Panel prompt -> generated ProjectPatch import
complex shooter mock smoke
```

### 10.2 与 AI Panel

AI Panel 不应该展示新的 LLM 子系统。

它只展示：

```text
生成的 ProjectPatch proposal。
ProjectPatch import diagnostics。
provider error diagnostic。
review_state。
```

### 10.3 与 Manual Walkthrough / AuthoringAiContext

只需暴露：

```text
llm_patch_source_available
active_patch_source_kind
supported_project_patch_capabilities
next_actions
```

不暴露：

```text
LLM request graph
LLM report graph
provider registry state
agent state
```

### 10.4 与全域 patch capability

规则：

```text
LLM patch source 不带来新的 patch domain 能力。
如果模型输出 AUI / Rule / Prefab 等 unsupported capability，205 import diagnostics 必须拒绝或 partial。
```

## 11. 复杂打飞机验收场景

### 场景 A：Mock source 生成合法 Scene patch

输入：

```text
用户意图：创建一个通用测试实体。
source_kind: mock
```

期望：

```text
mock source 返回 raw ProjectPatchDocument JSON。
ProjectPatchImportRequest.source_kind = AiStructuredOutput。
ProjectPatchImportResult.parse_status = Parsed。
PatchReviewModel.validation_status = true。
AI Panel 出现 ProjectPatch proposal。
不自动 apply。
```

### 场景 B：用户确认后应用

输入：

```text
Apply generated ProjectPatch proposal
```

期望：

```text
进入 EditorSession::execute_patch_as_transaction。
PatchApplyReport.status = Committed。
PatchHistorySummary.applied_count 增加。
```

### 场景 C：source 返回非法 JSON

期望：

```text
ProjectPatchImportResult.parse_status = Rejected。
diagnostic 指向 project_patch_import.parse_failed。
项目不变。
AI Panel 显示可理解错误。
```

### 场景 D：source 输出 unsupported capability

期望：

```text
capability_diagnostics 指向 unsupported capability。
next_actions 指向对应 patch capability v2。
项目不变。
```

### 场景 E：真实 provider optional smoke

输入：

```text
env enabled + API key present
ignored/local-only test
```

期望：

```text
有 env 时可以生成 raw ProjectPatch JSON 并进入 import。
无 env / 无网络时 skipped，不影响默认测试。
```

## 12. 可施工 Gate 建议

### Gate A：Thin Source Model

目标：

```text
在 editor_core/src/project_patch/llm_source.rs 新增最小 LlmPatchSourceConfig / LlmPatchSourceResult，或等价内部结构。
不新增 LlmStructuredOutputReport。
```

测试：

```powershell
cargo test -p editor_core llm_patch_source
```

### Gate B：Mock Source

目标：

```text
mock source deterministic 返回合法 ProjectPatch JSON。
mock source 可返回非法 JSON / provider error，用于失败测试。
mock source 支持 unsupported capability fixture。
prompt 组装明确当前 only Scene/Input supported。
```

测试：

```powershell
cargo test -p editor_core llm_patch_source
```

### Gate C：Import Integration

目标：

```text
source raw_json 进入 ProjectPatchImportRequest::ai_structured_output。
成功生成 AI Panel ProjectPatch proposal。
失败进入 CommandResult diagnostics。
```

测试：

```powershell
cargo test -p editor_core project_patch_import
cargo test -p editor_core ai_project_patch
```

### Gate D：AI Panel / Authoring Context

目标：

```text
AI Panel 暴露 generated ProjectPatch proposal。
AuthoringAiContext / ManualWalkthrough 暴露 llm_patch_source_available 和 next_actions。
```

测试：

```powershell
cargo test -p editor_ui_model ai_panel
cargo test -p editor_core authoring_workflow
cargo test -p editor_ui_model manual_walkthrough
```

### Gate E：Complex Shooter E2E Mock Smoke

目标：

```text
project_e2e_gate 用 mock source 生成 ProjectPatch JSON。
复用 ProjectPatchImportProductizationReport 产出证据。
```

测试：

```powershell
cargo test -p project_e2e_gate project_patch
cargo test -p project_e2e_gate
```

### Gate F：Optional Real Source Smoke

目标：

```text
真实 source 仅 feature/env/local-only/skipped。
本轮不引入 HTTP 依赖，不真实调用外部 API。
```

测试：

```powershell
cargo test -p editor_core llm_real_patch_source -- --ignored
```

### Gate G：整体回归与文档同步

目标：

```text
同步 49 / 54 / 施工文档 README / 阶段完成记录。
```

测试：

```powershell
cargo fmt --check
cargo test -p editor_core project_patch
cargo test -p editor_ui_model
cargo test -p project_e2e_gate
```

## 13. 施工时禁止事项

```text
禁止新增 LlmStructuredOutputReport 作为独立真相层。
禁止新增 Provider Registry。
禁止新增 Agent Planner / Repair Loop。
禁止真实 provider 成为默认测试必需条件。
禁止本轮引入 reqwest / ureq / async runtime 等 HTTP 依赖。
禁止把 API key 写入项目文件或 report。
禁止 LLM 直接 fs::write 项目文件。
禁止绕过 ProjectPatchImportService。
禁止绕过 PatchValidator。
禁止自动 apply。
禁止在 206 中扩 Asset / Prefab / AUI / Rule / Build patch capability。
禁止为复杂打飞机写 Player / Enemy / Bullet 专用 provider operation。
```

## 14. 方案自审

### 14.1 是否是当前最顺的下一步

通过。

```text
205 已完成 imported structured patch。
真实 LLM 作为 AiStructuredOutput source 是自然后续。
```

### 14.2 是否避免结构膨胀

通过。

```text
B-lite 不新增运行时层。
B-lite 不新增 report 真相层。
B-lite 不新增 agent loop。
B-lite 复用 205 import / report。
```

### 14.3 是否符合 AI-first

通过。

```text
schema-first。
ProjectPatchDocument 仍是 AI 输出契约。
ProjectPatchImportResult / diagnostics 仍是失败解释入口。
```

### 14.4 是否支撑复杂项目

通过。

```text
复杂项目长期需要真实 AI 生成结构化修改。
但 v3 只接入 source，不自动扩大 patch capability，也不引入 agent。
```

### 14.5 主要风险

风险一：

```text
provider 替换能力较弱。
```

处理：

```text
先用 Thin Source 接真实链路。
以后确实需要多 provider 路由时，再单独讨论 Provider Registry。
```

风险二：

```text
真实 provider 输出符合 JSON 但业务语义错误。
```

处理：

```text
继续依赖 ProjectPatchImportService / PatchValidator / Review / User Confirm。
```

风险三：

```text
后续又想加自动修复循环。
```

处理：

```text
Repair Loop 属于 v4+ Agent 系统，不进入 206。
```

## 15. 最终结论

采用：

```text
方案 B-lite：Thin LLM Patch Source
```

正式判断：

```text
下一步不是全自动 agent，也不是新增完整 provider adapter 子系统。
下一步只是把真实 / mock LLM 当成 ProjectPatchDocument JSON 的薄输入来源，接到 205 已完成的 ProjectPatch import 链路。
```

下一步：

```text
如果用户确认进入施工，基于本文生成新的自动化施工文档。
施工文档必须先自审，再按 Gate A-G 实施和测试。
```
