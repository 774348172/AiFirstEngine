# 254 当前可自动化施工文档：AI Tool Gateway / Codex Adapter v1

> 状态：已完成；Gate A-K、权威回归、完成记录与归档闭环  
> 生成日期：2026-07-17  
> 正式方案：`../../254-AI-Tool-Gateway-Codex-Adapter-v1方案.md`  
> 用户授权：生成施工文档、自我审查、激活后立即开始施工  
> 上游实现：253 AI Capability Tool Kernel 已完成并归档  
> 竞争限制：本施工不自动重跑三引擎 B 通道
> 激活日期：2026-07-17；激活时源码 fingerprint 与第 4 节一致，`当前/` 原为空

## 0. 当前施工进度

```text
Baseline：通过。
Gate A-K：全部通过。
累计定向：41 组命令，44 个测试实际命中，全部通过。
受影响域：1224 passed / 3 ignored，cargo fmt --check 通过。
最终权威回归：default 1462 passed / 5 ignored；all-features 1473 passed / 14 ignored。
冻结 manifest：b7fa950a5ac1a07d09dc1fd5a327f5b0301c536a6247072bc537eccfd6d64fc6。
最终 cleanup：0 drift / 0 leftover process。
```

254 已实现：

- `ai_tool_gateway`、versioned protocol、严格 schema/limits、per-project discovery、Windows Named Pipe 与 owner-thread dispatch；
- project-aware catalog/search/references/evidence、opaque grantRef、通用 ProjectPatch/Controlled SourcePatch；
- durable async operation、取消、断线重连、背压、恢复和 invocation replay；
- MCP stdio、CLI、Native Editor、Test Adapter 等价性与真实跨进程 MCP smoke；
- Visual-to-Semantic AUI locate/visibility/owner trace 与正式 Candidate 修复复验；
- 受控 Build/Delivery、drift/双写者 fail-closed 与 performance contract；
- Tool Kernel 继续是能力语义真相，没有引入 Agent Runner、任意 shell 或项目根外写入。

实际验证：

```text
cargo fmt --check：通过。
cargo test -p ai_tool_gateway：18 passed。
cargo test -p editor_core：508 passed / 2 ignored。
cargo test -p editor_window_winit：114 passed。
cargo test -p editor_host：17 passed。
cargo test -p engine_runtime：451 passed。
cargo test -p runtime_player_winit：18 passed。
cargo test -p project_e2e_gate：65 passed / 1 ignored。
cargo test -p runtime_cli：33 passed。
cargo test --workspace：1462 passed / 5 ignored。
cargo test --workspace --all-features：1473 passed / 14 ignored。
```

完成证据：`../../阶段完成记录/2026-07-17-AI-Tool-Gateway-Codex-Adapter-v1/00-总览.md`。权威 Cargo 两阶段均为 0 failed；测试改写 6 个样例 Build 生成物导致首次 cleanup 检查失败，恢复冻结候选并全量复核后为 0 drift / 0 leftover process。既有 `engine_runtime` 3 个 dead-code warning 未由 254 引入。

## 1. 施工目标

本施工把 253 的进程内 Tool Kernel 产品化为一个真实、可连接、可恢复的本机 AI Tool Gateway：

```text
Codex / CLI / Native Editor / Test
  -> thin Adapter
  -> per-user local IPC
  -> Editor-hosted Gateway Core
  -> AI Capability Tool Kernel
  -> unique EditorSession
```

完成后必须同时成立：

- Gateway Core 是拥有 handshake、项目 binding、dispatch、幂等、背压和 operation routing 的深 Module；
- Gateway Control Interface 只保留 `connect / dispatch / close`，业务工具继续通过 versioned request union 进入；
- Tool Kernel 继续是 Catalog、Grant、mutation、receipt、rollback 和 diagnostics 的唯一语义真相；
- MCP、CLI、Native Editor、Test Adapter 只做 framing 和展示，不复制业务判断；
- AI 可以广泛观察复杂项目，并能通过 ProjectPatch / Controlled SourcePatch / Formal Asset Import 处理未预设需求；
- Preview、test、Build 等长工具真实异步、可观察、可取消、可重连和可收敛；
- 用户只从画面描述 AUI 问题时，可以沿截图、stable node、binding、layout、draw、runtime state 和 source owner 定位；
- Build/Delivery 和最终权威回归给出结构化证据，不用一次性开放式 Runner 冒充 Gateway。

## 2. 明确非目标

本施工禁止扩入：

```text
通用 Agent Planner、固定跨工具菜谱、ProjectProductionRun v3
任意 shell、任意网络、任意项目根外文件系统入口
反射式全 Editor / ECS / Renderer 内部对象调用
自动修改 Engine Core 的普通项目权限
跨机器 Gateway、TCP listener、云 Gateway
完整项目字段级 provenance graph、常驻录像或线上遥测
提前启用 ProductBrokered 并锁死维护者直接写入
Unity / UE 项目修改或三引擎 B 通道重跑
```

## 3. Module、Interface 与 crate 归属

新增 `ai_tool_gateway` library crate，作为 Gateway Control seam 的 owner：

```text
rust/crates/ai_tool_gateway/
  Cargo.toml
  src/lib.rs
  src/protocol.rs
  src/core.rs
  src/dispatch.rs
  src/discovery.rs
  src/ipc.rs
  src/operation.rs
  src/adapters/{mod,test,cli,mcp}.rs
  tests/
```

归属规则：

- `ai_tool_gateway` 拥有协议、会话 binding、请求边界、连接生命周期、owner-thread dispatch、IPC 和 Adapter 合同；
- `editor_core::ai_capability_tool_kernel` 拥有工具描述、tool-specific schema、Grant、Kernel invocation/result、mutation 和 receipt；
- `editor_window_winit::NativeEditorApplication` 拥有唯一 `EditorSession`，只暴露有界 owner-thread pump/dispatcher 接入点；
- `editor_host` 负责组装并托管 Gateway，不拥有第二份 Session 或 Tool Kernel 语义；
- AUI 视觉诊断实现位于 AUI/runtime/editor 的正式 owner 附近，通过 Tool Kernel 注册，不塞入 Gateway Core；
- Build/Export 复用现有深实现，不复制 pipeline。

删除 `ai_tool_gateway` 后，连接、binding、IPC、幂等、调度和恢复复杂度会散落到四个 Adapter，因此新 crate 不是浅 pass-through。测试只通过 Gateway Control Interface 和 Tool Kernel Interface 验证，不穿透测试私有字段。

## 4. 当前基线与隔离

施工前基线：

```text
Git HEAD: 9f5adc73ca02a5e3fed3f38c62bb9a420e6bd5de
Rust: 1.96.0
Cargo: 1.96.0
OS/Shell: Windows / PowerShell
施工 target: G:\gameEngin-target\254
权威 target: G:\gameEngin-target\254-authoritative
```

当前工作树包含用户修改以及 249-253 的未提交正式实现，`HEAD` 不是当前源码真相。本施工不得 reset、checkout、清理或覆盖这些变更。施工前关键 fingerprint：

```text
rust/Cargo.toml
  a9ede0558fd3ce5e6d99de894ed4db693b00133ee68dcafab4c184b4ac434f61
rust/Cargo.lock
  34a5d14bf51eb0ed0507b1ffcde38a2afb2133865ce5be38372297c3c1d2b262
rust/crates/editor_core/Cargo.toml
  0561dc5726cd25c442c921dcdeda3668706be95ae8d40bd6c7eacb225480aa5b
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
  899de4cc55d074e0814daf317648446a18aa0e8aed60fd4c2e2483143cca6d0c
rust/crates/editor_core/src/lib.rs
  f467307984cd56542543d4d768692f68209e017400d574848dad7c8a2f048e61
rust/crates/editor_core/src/tests/ai_capability_tool_kernel_tests.rs
  09e3be280e95e99242d8f9918f603f25a0357edc499cecd11417de173db2fd62
rust/crates/editor_host/src/main.rs
  aa46cf1c0908c6fc95547f3a6b7389772442302d90fa545c202606e35b296878
rust/crates/editor_window_winit/src/application.rs
  881c0c52a542a18c4cf4816ab65bd9be92e2be6c615a6b4d4b6a5347b4a2e63d
```

隔离规则：

- 每次编辑前重新读取重叠文件；fingerprint 变化且不是本施工造成时，兼容新内容，不覆盖；
- 所有增量构建使用独立 `CARGO_TARGET_DIR=G:\gameEngin-target\254`；
- IPC pipe、discovery、journal、evidence、fixture project、端口和报告根按测试唯一化，禁止并行共享；
- 最终权威回归前冻结候选，并在仓库外建立包含全部当前 tracked/untracked 正式源码的 exact-candidate source snapshot；排除 `.git`、`target`、缓存、历史 Build 和测试输出；
- snapshot 记录 manifest、逐文件 SHA-256、源根、命令、环境和 cleanup；它是混合工作树下的等价隔离证据，不得称为 clean checkout 或 exact commit CI；
- 若无法建立 exact-candidate snapshot，只能报告本地 mixed-worktree 验证，不能宣称 Gate K 完成。

## 5. 预计涉及文件

预计新增：

```text
rust/crates/ai_tool_gateway/Cargo.toml
rust/crates/ai_tool_gateway/src/*.rs
rust/crates/ai_tool_gateway/src/adapters/*.rs
rust/crates/ai_tool_gateway/tests/*.rs
```

预计修改：

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/ai_capability_tool_kernel_tests.rs
rust/crates/editor_window_winit/Cargo.toml
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_host/Cargo.toml
rust/crates/editor_host/src/main.rs
rust/crates/engine_runtime/src/aui.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/project_e2e_gate/Cargo.toml
rust/crates/project_e2e_gate/src/lib.rs
```

只在真实 owner 或跨 crate consumer 要求时扩大范围。新增范围必须先回填本节并重新自审。禁止把具体“开始游戏按钮”玩法写入 Core；Gate G fixture 只能作为项目侧测试数据。

## 6. Baseline 与环境预检

进入条件：本文已激活为 `当前/` 唯一非 README 文档。

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-target\254'
Set-Location G:\gameEngin\rust
rustc --version
cargo --version
cargo metadata --locked --format-version 1 --no-deps
cargo test -p editor_core ai_capability_tool_kernel_schema_and_catalog_are_small_and_strict
cargo test -p editor_core ai_capability_tool_kernel_preview_requires_project_owned_runtime_binding
cargo test -p editor_host --test cli_exit
```

预计成本：2-15 分钟，首次独立 target 可更久。已有失败写入 baseline，不为通过 254 随意修改无关系统；影响 Gate A 的失败先收敛，不能跨 Gate。

额外预检：

```text
当前/ 与待执行/ 槽位合同
Windows Named Pipe 能力和当前用户 ACL 可用性
真实 Editor owner-thread pump 点
子进程环境变量、换行符和路径长度
Codex 官方 MCP 配置与本机版本
```

2026-07-17 已执行的 Codex 资料预检结果：官方 manual helper 请求返回 HTTP 403；按 fallback 尝试 `codex mcp add openaiDeveloperDocs --url https://developers.openai.com/mcp`，本机 `codex.exe` 返回 Access denied。故施工不得凭记忆写死 Codex 配置。Gate A-E 可基于协议无关合同继续；进入 Gate F 真实 MCP smoke 前必须重新取得当前官方资料或本机可验证行为。

## 7. Gate A：Protocol / Schema

任务：

- 新建 `ai_tool_gateway` crate 和 versioned protocol types；
- 定义 `ClientHello / ClientSessionBinding / GatewayRequest / GatewayReply / GatewayEvent / CloseReceipt`；
- `GatewayRequest` 只承载 `catalog / inspect / execute / observe / cancel` tagged union；
- 所有外部输入 `deny_unknown_fields`，未知 version/union、过深 JSON、超长字符串、超大数组和总消息 fail-closed；
- Tool Kernel descriptor 增加独立 `toolVersion`、最小合法 example、tool-specific input/output schema；
- schema 能验证最小合法 invocation，不再共同返回占位 object；
- 不在本 Gate 接入真实 Editor、IPC、MCP 或 mutation。

定向测试：

```powershell
cargo test -p ai_tool_gateway gateway_protocol
cargo test -p ai_tool_gateway gateway_schema_limits
cargo test -p editor_core ai_capability_tool_kernel_tool_specific_schema
```

受影响域：

```powershell
cargo test -p ai_tool_gateway
cargo test -p editor_core ai_capability_tool_kernel
```

预计成本：5-20 分钟。任何 schema 不确定、toolId/payload 漂移或 Kernel/Gateway 重复真相都阻断 Gate B。

## 8. Gate B：Project Binding / Local Security

任务：

- 实现小型 Gateway Control Interface：`connect / dispatch / close`；
- Editor Host 生成不可预测 per-user discovery record 和 Windows Named Pipe locator；
- handshake 校验 client/version/project identity/root digest/PID/read scope；
- discovery 不包含秘密，session nonce/token 不写日志、项目或 report；
- Editor project switch/close/reopen 使旧 binding 失效；
- `NativeEditorApplication` 提供 owner-thread request pump，不让协议线程直接持有 `&mut EditorSession`；
- in-memory Test Adapter 先证明 Interface，再接真实跨进程 local IPC；
- 无 TCP listener，ACL 只允许当前 OS 用户。

定向测试：

```powershell
cargo test -p ai_tool_gateway gateway_binding
cargo test -p ai_tool_gateway gateway_discovery
cargo test -p ai_tool_gateway gateway_named_pipe
cargo test -p editor_window_winit gateway_owner_thread_dispatch
```

受影响域：

```powershell
cargo test -p ai_tool_gateway
cargo test -p editor_window_winit
cargo test -p editor_host
```

预计成本：10-35 分钟。ACL 无法真实证明时允许保留明确 local-only skipped evidence，但不得宣称 Gate B 完成或进入 ProductBrokered。

## 9. Gate C：Project-aware Catalog / Complex Observation

任务：

- Catalog 根据 active project、platform、RuntimeModule、Grant、implementation availability 和 operation conflict 过滤；
- 增加 `project.search / project.read_object / project.references / project.source_symbols / project.diagnostics / evidence.read`；
- 建立有界、稳定排序、分页 continuation token 和 evidence ref；
- 从正式 Scene、Prefab、AUI、Rule、Asset 和 project-owned source 建立按需索引；
- AI 不预知 path/GUID/stable id 也能从名称、文字、类型、binding/action 查询；
- lightweight inspect/search 不触发 workspace compile；
- 二进制和大 artifact 不内联。

定向测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel_project_aware_catalog
cargo test -p editor_core ai_capability_tool_kernel_project_search
cargo test -p editor_core ai_capability_tool_kernel_references
cargo test -p editor_core ai_capability_tool_kernel_evidence_read
cargo test -p ai_tool_gateway gateway_observation_limits
```

受影响域：

```powershell
cargo test -p editor_core
cargo test -p ai_tool_gateway
```

预计成本：15-45 分钟。禁止一次全仓上下文或编译冒充搜索索引。

## 10. Gate D：Grant v2 / Generic Mutation Freedom

任务：

- CapabilityGrant 增加 `ExactDomains / ProjectOwnedLowRisk / Elevated` scope mode；
- 外部 Adapter 只传 opaque `grantRef`，签发、续期、撤销和解析只在 Editor/Gateway trusted side；
- 对外注册 ProjectPatch、Controlled SourcePatch 和 Formal Asset Import 通用路径；
- 复用 Candidate、validation、SafeProjectPath、receipt、rollback 和 lineage，不复制 lowering；
- 同一低风险目标允许跨 AUI/Rule/RuntimeModule project-owned domain，实际变化写 receipt；
- 删除、依赖、网络、费用、发布和 Engine Core 继续升级或拒绝；
- MaintainerCompatibility 外部写入只触发 drift/rebase，不生成虚假 receipt。

定向测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel_scope_mode
cargo test -p editor_core ai_capability_tool_kernel_generic_source_patch
cargo test -p editor_core ai_capability_tool_kernel_generic_mutation_rejects
cargo test -p ai_tool_gateway gateway_grant_ref
cargo test -p ai_tool_gateway gateway_invocation_replay
```

受影响域：

```powershell
cargo test -p editor_core project_candidate_entry
cargo test -p editor_core ai_capability_tool_kernel
cargo test -p ai_tool_gateway
```

预计成本：15-45 分钟。任意未经 receipt 的 Apply、项目外写入或高风险静默扩大都阻断 Gate E。

## 11. Gate E：Async / Cancel / Recovery

任务：

- Kernel 建立 durable operation 后返回 typed accepted result；
- operation journal 记录 queued/preflight/prepared/running/cancelling/terminal 和 artifact refs；
- Preview、validate、targeted test 接入 worker/243 cancellation/bounded child process；
- observe/cancel 可在 execute 返回后真实工作；
- cancel 区分 signal、child termination、commit state 和 terminal；
- Adapter 断线不丢 operation，重连可 observe；
- Editor shutdown/restart 对遗留 Running 做 interrupted/recoverable/failed reconciliation；
- 相同 invocation 精确重放不重复 Apply。

定向测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel_async_operation
cargo test -p editor_core ai_capability_tool_kernel_cancel_worker
cargo test -p editor_core ai_capability_tool_kernel_recovery
cargo test -p ai_tool_gateway gateway_disconnect_reconnect
cargo test -p ai_tool_gateway gateway_backpressure
```

受影响域：

```powershell
cargo test -p editor_core
cargo test -p runtime_cli bounded_child_process
cargo test -p ai_tool_gateway
```

预计成本：20-60 分钟。只改状态、不连接 worker/child 的 cancel 视为失败。

## 12. Gate F：MCP / CLI / Native Editor Adapter Equivalence

前置：当前 Codex 官方 MCP 文档或本机客户端行为已成功验证，具体启动配置写入 smoke artifact，不凭记忆补字段。

任务：

- 实现 MCP stdio、CLI、Native Editor 和 Test 四个 Adapter；
- MCP/CLI 运行于 Editor 外，只通过 local IPC；
- Native Editor 提供 Grant 批准/续期/撤销、operation 进度、取消和结果展示；
- Adapter 不构造 ElevatedGrant，不复制 schema 校验、Candidate、Grant 或 receipt；
- 同一 frozen invocation 的 Catalog、授权、operation identity、diagnostic、receipt 和 after digest 等价；
- 真实跨进程 Codex/MCP smoke 证明工具可发现并调用。

定向测试：

```powershell
cargo test -p ai_tool_gateway adapter_equivalence
cargo test -p ai_tool_gateway mcp_stdio_adapter
cargo test -p ai_tool_gateway cli_adapter
cargo test -p editor_window_winit gateway_native_editor_adapter
```

受影响域：

```powershell
cargo test -p ai_tool_gateway
cargo test -p editor_window_winit
cargo test -p editor_host
```

预计成本：20-60 分钟，真实客户端 smoke 另计。mock-only 不能通过 Gate F。

## 13. Gate G：Visual-to-Semantic AUI First Slice

任务：

- 增加按需 `VisualIssueBundle`，正常 Runtime 保持 Off/compact；
- 注册 `runtime.capture_issue / ui.locate / ui.explain_visibility / project.trace_ui_owner`；
- bundle 绑定 screenshot、project/runtime/frame digest、AUI authored/resolved node、layout/clip/draw/hit-test、binding/state、screen-flow 和 present diagnostics；
- `ui.locate` 可找到不可见节点并处理同名候选；
- `ui.explain_visibility` 返回首个失败阶段，不用像素猜代码；
- `trace_ui_owner` 追踪 AUI asset、binding path、producer、Rule/action 和 project source symbol；
- 冻结项目侧“返回主菜单后开始游戏按钮消失”缺陷，不提供 seed 根因，使用正式 mutation 修复并截图/语义复验。

定向测试：

```powershell
cargo test -p engine_runtime visual_issue_bundle
cargo test -p editor_core ai_capability_tool_kernel_ui_locate
cargo test -p editor_core ai_capability_tool_kernel_ui_visibility
cargo test -p editor_core ai_capability_tool_kernel_ui_owner_trace
cargo test -p project_e2e_gate visual_symptom_diagnosis
```

受影响域：

```powershell
cargo test -p engine_runtime aui
cargo test -p runtime_player_winit
cargo test -p editor_core
cargo test -p project_e2e_gate visual_symptom_diagnosis
```

预计成本：20-60 分钟。截图存在但 stable-id/binding/layout/draw/source evidence 断链时不能通过。

## 14. Gate H：Build / Delivery

任务：

- 注册 `project.build_export / project.delivery_verify` 深工具；
- 复用现有 DesktopExport/ReleasePackage/RuntimePackage 和 external process verification；
- build 快速返回 operation，候选冻结后只运行一次权威 Build/Export；
- 证明项目外交付、无参数启动、真实窗口、非空像素、退出 0；
- Build/Delivery 不持有 mutation lane，不变成需求 Runner。

定向测试：

```powershell
cargo test -p editor_core ai_capability_tool_kernel_build_export
cargo test -p editor_core ai_capability_tool_kernel_delivery_verify
cargo test -p project_e2e_gate gateway_build_delivery
```

受影响域：

```powershell
cargo test -p editor_core build_service
cargo test -p runtime_cli exported_player
cargo test -p project_e2e_gate gateway_build_delivery
```

预计成本：20-90 分钟。构建产物必须使用隔离输出根，不能污染 tracked sample 作为测试真相。

## 15. Gate I：Bypass / Drift

任务：

- MaintainerCompatibility 下外部写入造成 digest drift，Gateway 拒绝旧 lineage 且不伪造 receipt；
- ProductBrokered 只做等价安全测试，证明直接写被 sandbox/ACL 拒绝而通用 Patch 可完成同一需求；
- 未完成真实 OS 约束时，产品状态继续声明“受控路径 + drift 检测”，不提前改默认模式；
- 两个 Agent 同 digest 准备 mutation，仅第一份合法提交，第二份 fail-closed/reinspect。

定向测试：

```powershell
cargo test -p ai_tool_gateway gateway_bypass_drift
cargo test -p ai_tool_gateway gateway_concurrent_writer
cargo test -p editor_core ai_capability_tool_kernel_lineage
```

受影响域：

```powershell
cargo test -p ai_tool_gateway
cargo test -p editor_core ai_capability_tool_kernel
```

预计成本：10-30 分钟。

## 16. Gate J：Performance / Competition Preflight

任务：

- 测量 cold/warm Adapter startup、handshake、Catalog、search、mutation、validation、Preview、Build；
- 分开报告 IPC wait、Editor queue wait、worker、child process 和 cache hit/miss；
- 确认 handshake/search 不触发 workspace compile/import/rebuild；
- changed-domain validation，不重复跑完整 workspace；
- 完成本引擎单通道 unknown-task 预检，证明明显低于两小时并保留余量；
- 只输出是否具备重跑竞争 cohort 的证据，不自动重跑 Unity/UE/B 通道。

定向测试：

```powershell
cargo test -p ai_tool_gateway gateway_performance_contract
cargo test -p project_e2e_gate gateway_unknown_task_preflight
```

预计成本：20-90 分钟。阈值依据实测 baseline 冻结，不在施工前编造毫秒预算。

## 17. Gate K：受影响域与最终权威回归

进入条件：Gate A-J 定向测试与累计失败集全绿，候选源码/依赖/lockfile 已冻结，exact-candidate snapshot 已建立。

第一层，累计定向测试：执行本文 Gate A-J 的全部 filter，预计 15-60 分钟。失败只回到对应 Gate，不启动 workspace。

第二层，受影响域：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-target\254'
Set-Location G:\gameEngin\rust
cargo fmt --check
cargo test -p ai_tool_gateway
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p editor_host
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo test -p project_e2e_gate
```

预计 30-120 分钟。先收敛首个真实失败并加入累计失败集，不连续盲跑 workspace。

第三层，最终权威回归：在仓库外 frozen exact-candidate source snapshot 中运行一次：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-target\254-authoritative'
Set-Location <exact-candidate-snapshot>\rust
cargo test --workspace
cargo test --workspace --all-features
```

最终 artifact 必须记录：source manifest digest、命令、Rust/Cargo/OS、target root、退出码、测试计数、耗时、Gateway fault injection、跨进程/真实 Editor/Preview/player evidence 和 cleanup。任何代码、manifest、lockfile、ledger 或测试变化都使冻结失效，必须重跑环境等价预检后才能再次权威运行。

## 18. 高风险否定矩阵

至少覆盖：

```text
unknown protocol/tool/schema version、invalid union、deep recursion、oversize
wrong project identity/root digest/PID、stale discovery、project switch
nonce/token 日志泄漏、非当前用户 IPC、TCP listener 意外存在
tool-specific schema 与真实 payload 漂移
unknown tool、wrong grant、grant expiry/revoke/budget/risk escalation
same invocation same content replay、same id different content reject
external write drift、two-writer conflict、receipt terminal persist failure
Adapter request body partial close、pipe close、backpressure
worker panic、child timeout/kill failure、Editor shutdown/restart遗留 Running
evidence stale/tampered/oversize/project mismatch
search 不预知 path/GUID、同名对象、pagination 稳定性
SourcePatch 项目外、delete、dependency、network、Engine Core escalation
AUI invisible/off-screen/clip/missing binding/draw/present/hit-test/action failure
Build output isolation、no-arg external run、nonblank present、exit 0
MCP/CLI/Native/Test Adapter 等价性
```

## 19. 施工文档自审

方案一致性：通过。本文完整映射 254 Phase 1-7 与 Gate A-K，没有改变 Editor-hosted Gateway、Agent-owned planning 或 Tool Kernel 唯一语义真相。

范围：通过。新 crate 只拥有跨进程连接与控制面；AUI、Build、Candidate、Grant 和 receipt 仍归原 owner。未引入 Agent Runner、任意 shell、反射式内部调用或网络服务。

Module 深度：通过。Gateway Control Interface 固定为 `connect / dispatch / close`，协议、binding、幂等、dispatch、IPC、恢复与 Adapter 复杂度隐藏在 implementation；四个 Adapter 证明 seam 真实存在。

AI 自由：通过。领域工具不足时保留 ProjectPatch、Controlled SourcePatch、Formal Asset Import 和 allowlisted validation/test，限制的是不可审计副作用，不是未知需求实现。

复杂项目：通过但需施工证明。Gate C 是广泛观察硬门禁，Gate G 是画面描述诊断硬门禁；在两者完成前不得宣称成熟。

安全：通过。MaintainerCompatibility 与 ProductBrokered 分开；未完成 OS 约束时不虚报 broker-only。GrantRef、SafeProjectPath、receipt、drift 和 ACL 均有否定测试。

测试分层：通过。每 Gate 有定向与受影响域，Gate K 有 frozen exact-candidate snapshot 下 default/all-features 权威回归；昂贵 workspace 不用于逐错调试。

混合工作树：通过。已记录 HEAD、工具链和施工前 fingerprint；不把混合工作树冒充 clean checkout/exact commit CI，不还原用户变更。

环境：通过。Windows Named Pipe、ACL、owner-thread、子进程、独立 target、输出根、换行、真实跨进程和 cleanup 均进入 Gate。

Codex 官方兼容性：有界通过。manual helper 当前 HTTP 403、Docs MCP 安装当前 Access denied 已记录；本文没有猜测配置，并把成功的官方/本机兼容 smoke 设为 Gate F 前置，不阻断协议无关 Gate A-E。

外部审查：无需修改正式方案。本轮没有新增与 254 对象一致的其它 AI 审查文档；三引擎 B8 只作为已由 254 方案吸收的竞争约束，不重跑 cohort。

施工槽位：通过。生成时 `当前/` 为空、`待执行/` 只有 README；本文位于 `待执行/` 时明确不可施工。用户已授权激活并开工，仍必须在激活前复核方案、代码、测试和槽位，移动到 `当前/` 并同步 54 后才能修改源码。

自审结论：本文没有扩大正式方案、没有跳过测试、没有建立新菜谱或第二能力真相，可以在激活前复核通过后进入唯一当前施工槽位，从 Baseline 和 Gate A 开始；不允许直接跳到 MCP、IPC、视觉诊断或权威 workspace。

## 20. 激活、状态同步与归档

激活前：

1. 确认 `施工文档/当前/` 为空；
2. 确认 254 正式方案仍有效、253 仍已完成、代码基线未出现不可兼容漂移；
3. 确认 baseline 和 Gate A 命令可执行；
4. 将本文移动到 `施工文档/当前/`；
5. 把本文状态改为“施工中”，同步 54、49、240；
6. 只有以上完成后运行 baseline 和修改代码。

全部 Gate 完成后：

1. 生成 `阶段完成记录/2026-07-17-AI-Tool-Gateway-Codex-Adapter-v1/00-总览.md`；
2. 记录每层测试、权威 run 次数/耗时、失败、累计失败集、性能结果、source manifest 和 cleanup；
3. 更新 254 状态、240、49、54、00 文档地图、00 阅读顺序、施工文档 README、阶段完成记录 README；
4. 把本文从 `当前/` 移到 `已完成/`，清空当前槽位；
5. 不自动激活下一项，不自动重跑三引擎 B 通道。
