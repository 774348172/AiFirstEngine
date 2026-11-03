# 309-F Headless Engine Tool Provider Atomic Cutover v1 施工文档

> 文档性质：唯一 Atomic Cutover 施工文档；已生成并完成静态自审。  
> 当前状态：已完成并归档；Gate A-I 与 F1-F12 全部通过。F11-R3 已在人工新建的真实 Codex projectless 任务中完成唯一 inspect/mutate/run/build/delivery verify Agent Tool Loop，五个 Tool、receipt、revision、artifact、delivery 和进程报告全部通过，外部状态已恢复。  
> 正式方案：`../../309-F-Headless-Engine-Tool-Provider-Atomic-Cutover-v1方案.md`。  
> 上级总方案：`../../309-Headless-AuthoringProjectContext-Unified-Mutation-v1方案.md`。  
> 前置状态：309-A 至 309-E 已完成并归档；Tower P1-2 为 `stopped_by_user`，禁止恢复。  
> 生成基线：branch `codex/255-capability-catalog`，HEAD `acaffa8cb95bbb54d093bbf45835a59315c82da5`；工作区已有大量用户变更，未来施工不得重置。  
> 生成日期：2026-09-01。  
> 终态授权：2026-09-02 用户已授权并完成 Gate I，仅执行终态文档、完成记录和归档。未进入 Local CI、Android 工具链、Tower P1-2、production 或其它项目施工。

## 1. 施工目标与允许声明

本文只施工一个框架结果：用默认无 Editor 的 `Headless Engine Tool Provider` 一次替换
Editor-hosted Gateway 生产拓扑，并在同一 Agent Tool Loop 中向 Codex/MCP 和 Native consumer
提供具体 typed `engine_*` 工具。

全部 Gate 通过后最多可以声明：

```text
Host AI 可以像调用普通文件、Shell 工具一样直接调用具体 engine_* 工具；
Provider、Kernel 和 MCP Adapter 不依赖 editor_* 或 ai_tool_gateway；
项目正确性只依赖 canonical storage + AuthoringProjectContext；
普通文件写入后的下一 Engine 调用自动 refresh；
完整单文档继续 LWW，结构化修改继续 CAS/journal/receipt/rollback；
当前对外宣称 ready 的 run/build/delivery 具有真实 No-Editor 链路；
Editor 是可选验证 Projection，不再托管默认 Gateway；
旧 Gateway discovery、named pipe、Editor instance 和 generation authority 已退出生产组合。
```

不得声明未来 R2-R5 已全部完成、完整 Game SDK/完整 R3 Compiler/Semantic Outcome 已完成、所有引擎领域
都有 AI tool、已经优于 Unity/UE/Godot，或任何未经真实验收的平台发布能力已经完成。

## 2. 必要性与验证经济性审计

```text
requested claim：以一次框架切换把自研引擎作为宿主AI可直接调用的工具提供，不再以Editor/Gateway为前置
confirmed gap：现有MCP经discovery/named pipe进入Editor；Kernel直接接收EditorSession；Gateway重复持有generation/digest；preview依赖Play/GameView；canonical source到RuntimePackageBuildInput和Player/Windows export owner仍在editor_core
minimum causal change：抽出中立项目执行owner；迁移现有Kernel；建立一个Provider crate及MCP binary；将现有Assembler/Player/Windows dev delivery及实际中立依赖闭包迁入同一Compiler/Build owner；以Editor Adapter回接；切断Editor Gateway组合
必须修改：Cargo依赖图、现有Kernel可达的中立项目/观察/诊断/交付代码、Assembler/Player/Desktop export owner及其直接中立schema/cooker、MCP投影、Editor Gateway composition与对应测试
明确排除：未来全工具集、Agent Planner、第二模型循环、daemon/网络服务、Renderer/ECS/AUI/RuntimePackage/Build Pipeline重写、Tower施工、Local CI与生产替换
验证策略：同一施工窗口内按A-I Gate逐步打红/修复；不发布任何中间双架构；最终以F1-F12资格矩阵一次判定
复用证据：309-A-E Context revision/snapshot/CAS/journal/recovery/LWW；现有Kernel Grant/operation/cancel/receipt；现有Assembler/Player/Desktop export行为测试；runtime_cli、RuntimePackageBuilder与exported Player verification
证据失效条件：正式方案被替代、309-A-E owner漂移、现有Kernel调用图显著变化、真实Codex MCP能力变化、工作区用户改动与允许文件冲突
预计成本：Gate A-E证据保留；修订后剩余Gate F-H约2至5个完整施工日。若F-B实际依赖闭包明显超出本文逐项清单，先停止复核，不用新增抽象掩盖成本
停止条件：需要重写Renderer/ECS/AUI功能或RuntimePackage/Build Pipeline行为、补齐未来R2-R5、引入第三个compiler crate/daemon、长期保留双路径、触碰Tower P1-2，或真实配置修改未获单独授权
```

这里的“不过量”不是缩成只删 Gateway digest。那样仍会保留 Editor 前置，不能实现正式方案。
不过量的控制点是：只迁移现有工具可达能力与 No-Editor 资格所必需的执行链，不顺带建设未来能力。

## 3. 冻结终态 Module 与 crate 组合

为避免机械拆 crate，v1 只新增两个 crate：

```text
project_authoring_execution
  - ProjectAuthoringSession / ProjectBinding
  - project locator containment与qualification
  - Context open/reuse/refresh/snapshot/commit/rollback编排
  - GameProjectCompiler深Module
  - prepare/run/build/verify小Interface
  - GameProjectCompiler 内部唯一 ProjectRuntimePackageAssembler 及其中立 schema/cooker owner
  - ProjectPlayerArtifact、runtime player staging、DesktopExportPipeline、safe output 与 Windows dev delivery verification 的中立 owner
  - 现有工具所需的project observation、visual diagnostics、runtime/evidence项目绑定

engine_tool_provider
  - capability_definition
  - engine_tool_kernel
  - provider / ProviderSession
  - native_host_adapter（可嵌入的Rust Interface，不虚构Codex native plugin）
  - mcp_host_adapter
  - ai_engine_tool_provider_mcp binary
  - Codex config artifact/installer逻辑（只生成或测试隔离配置；不自动修改真实配置）
```

冻结依赖方向：

```text
authoring_project_context   engine_input   engine_runtime   runtime_cli
          ^                    ^              ^              ^
          |                    |              |              |
          +--------------------+--------------+--------------+
                               |
                 project_authoring_execution
                   |-- ProjectAuthoringSession
                   `-- GameProjectCompiler / Build owner
                               ^
                               |
                    engine_tool_provider
  |-- capability_definition
  |-- engine_tool_kernel
  |-- provider
  |-- native_host_adapter
  `-- mcp_host_adapter / MCP binary

editor_core / editor_window_winit / editor_host
  -> project_authoring_execution（仅实际需要的可选验证消费）
```

`engine_tool_provider` 和 `project_authoring_execution` 的依赖闭包不得包含 `editor_core`、
`editor_ui_model`、`editor_ui_renderer`、`editor_window_winit`、`editor_host` 或 `ai_tool_gateway`。
不为图中的每个内部 Module 再建 crate/trait；只有发现独立 owner 和独立 consumer 的真实编译边界时，
才允许先回到本文复核，而不是施工中自由增殖抽象。

## 4. 冻结产品与执行合同

### 4.1 AI 可见工具面

Gate A 必须从现有 capability registry 冻结最终名称、schema 和 readiness。默认采用以下映射，
只允许为避免现有命名冲突做一次整体修订：

| 现有 internal id | AI 可见具体工具 | 309-F 决策 |
| --- | --- | --- |
| `project.create` | `engine_project_create` | 迁移现有能力，不扩展模板系统 |
| `project.inspect` | `engine_project_inspect` | Headless ready |
| `project.mutate` | `engine_project_mutate` | Context CAS |
| `project.rollback` | `engine_project_rollback` | exact rollback |
| `project.preview` | `engine_project_run` | 去除 Editor Play/GameView；不能证明则非 ready |
| `project.search` | `engine_project_search` | Headless read |
| `project.read_object` | `engine_project_read_object` | Headless read |
| `project.references` | `engine_project_references` | Headless read |
| `project.source_symbols` | `engine_project_source_symbols` | Headless read |
| `project.diagnostics` | `engine_project_diagnostics` | Headless read/check |
| `evidence.read` | `engine_evidence_read` | 中立 evidence root |
| `runtime.capture_issue` | `engine_runtime_capture_issue` | 仅迁移现有可证明链路 |
| `ui.locate` | `engine_ui_locate` | 无 Editor UI 前置 |
| `ui.explain_visibility` | `engine_ui_explain_visibility` | 无 GameView retained state 前置 |
| `project.trace_ui_owner` | `engine_project_trace_ui_owner` | 无 Editor owner 前置 |
| `project.build_export` | `engine_project_build` | 复用现有 build/export owner |
| `project.delivery_verify` | `engine_delivery_verify` | 复用 exported Player verification |
| operation observe/cancel | `engine_operation_observe` / `engine_operation_cancel` | Provider lifecycle |
| project bind/open | `engine_project_open` | 仅在不能由host/root唯一绑定时使用 |

AI 可见表面严禁出现 `aife_catalog`、`engine_catalog`、`engine_execute`、Context 内部工具、
`gateway_status`、`connect_editor`、`discover_editor`、`editor_instance_id`、`read_generation` 或
`expected_project_digest`。内部 dispatch 可以存在，但不得原样注册给 AI。

### 4.2 Project 与 revision

- 一个 `ProviderSession` 同时只绑定一个 canonical project；活跃 operation 存在时禁止静默 rebind。
- 定位优先级严格为 host 显式 root、MCP `--project-root`、cwd 自身合格、最后才由
  `engine_project_open` 在允许 workspace 内显式绑定。
- 每次 Engine 语义调用前执行安全 refresh/revalidation；watcher 只作 hint，warm cache 不是 authority。
- AI 不提交 expected digest。Provider 注入 Context revision，Kernel 以其生成 bounded Grant。
- read/check 使用 operation-bound immutable snapshot；结构化 mutation 使用 expected-revision CAS。
- 完整单文档保持 309-E LWW；普通 `write_file/apply_patch` 不强制进入 Engine mutation lane。
- receipt/rollback 绑定 canonical project identity、Context before/after revision 与 changed set，
  不绑定 Gateway generation。

### 4.3 Approval、取消与结果

Host approval 与 Engine Grant 分层：Host 决定这次工具调用是否获准，Engine 仍独立验证项目、schema、
scope、revision、risk 和 cancellation。`rejected_by_host`、`authorization_required`、
`unsupported_on_adapter`、`rejected_by_engine`、`cancelled`、`failed` 必须可区分。

Native 与 MCP 必须消费同一 `ToolDefinition` 和 canonical invocation/result。Native v1 的交付是
可嵌入程序 Interface 和 conformance tests，不声称 Codex 已有不存在的第一方 plugin 扩展点。

### 4.4 Run/build/delivery 最小迁移

在 `project_authoring_execution` 内冻结一个中立 `GameProjectCompiler` 深 Module：

```text
prepare(ProjectSnapshotLease, TargetProfile) -> PreparedRuntimePackage
run(PreparedRuntimePackage, RunOptions) -> RuntimeExecutionReport
build(PreparedRuntimePackage, BuildRequest) -> BuildDeliveryReport
verify(DeliveryRef, VerifyRequest) -> DeliveryVerificationReport
```

F-B 只迁移并复用现有 `ProjectRuntimePackageAssembler`、`BuildProfile`、artifact-cache 和真实调用闭包中的
中立 schema/read/parse/cook；它们组成 Compiler 内部唯一装配 owner。Assembler 必须从 operation-owned lease
构造 `SourceView`，不得重新读取 live filesystem。`ProjectPlayerArtifact`、`DesktopExportPipeline`、safe
output/delivery 和 process verification 属于后续 F-C，不在 F-B 偷渡。继续调用原 owner中的
`RuntimePackageBuilder` 与 `engine_runtime`，不得复制或重写 pipeline。Prepared result 必须绑定同一 snapshot
revision/profile/source identity。Editor 与 Provider 都只消费 Compiler Interface；Scene/AUI/Prefab/Font/
Input/Animation cooker 不向 Provider 暴露。无法在对应 Gate 证明 No-Editor 的能力必须从默认 ready set 移除并
返回明确 maturity，不得伪造 screenshot、report 或 success。

## 5. 允许文件与退役清单

### 5.1 允许新增

下列两个 crate 已分别由 Gate B/C 创建；本清单现在同时表示 Gate F 允许继续修改的新增 owner 范围，
不允许再新增第三个 crate：

```text
rust/crates/project_authoring_execution/Cargo.toml
rust/crates/project_authoring_execution/src/**
rust/crates/project_authoring_execution/tests/**
rust/crates/engine_tool_provider/Cargo.toml
rust/crates/engine_tool_provider/src/**
rust/crates/engine_tool_provider/tests/**
```

### 5.2 允许修改

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/editor_core/Cargo.toml
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/desktop_export.rs
rust/crates/editor_core/src/project_player_artifact.rs
rust/crates/editor_core/src/project_runtime_player_staging.rs
rust/crates/editor_core/src/editor_preview_package.rs（只回接中立Compiler/Player Interface）
rust/crates/editor_core/src/android_export.rs（只回接Assembler re-export并做编译回归，不扩移动端）
rust/crates/editor_core/src/release_package.rs（只回接Assembler/Player re-export，不扩release能力）
rust/crates/editor_core/src/report_panel.rs（只适配中立report类型）
rust/crates/editor_core/src/services/build_service.rs（只回接中立build Interface）
rust/crates/editor_core/src/ai_tool_catalog.rs
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/project_observation.rs
rust/crates/editor_core/src/project_visual_diagnostics.rs
rust/crates/editor_core/src/project_delivery_tools.rs
rust/crates/editor_core/src/project_asset_import.rs（只抽出Assembler使用的Asset DB只读schema/loader）
rust/crates/editor_core/src/animator2d_cooker.rs（只迁移现有纯parse/cook owner）
rust/crates/editor_core/src/aui_document_cooker.rs（只迁移现有纯cook owner）
rust/crates/editor_core/src/aui_font_atlas_cooker.rs（只迁移Font cook直接依赖）
rust/crates/editor_core/src/engine_builtin_font_pack.rs（只迁移Assembler直接依赖的built-in pack）
rust/crates/editor_core/src/input_mapping_authoring.rs（只抽出path scan/load纯读取部分）
rust/crates/editor_core/src/prefab_workflow.rs（只抽出Prefab schema/resolve，不迁移stage UI）
rust/crates/editor_core/src/project_assembly_artifact_cache.rs（迁移现有Assembler cache owner）
rust/crates/editor_core/src/project_font_assets.rs（只迁移Font cook所需canonical schema）
rust/crates/editor_core/src/project_font_bundle.rs（只迁移Font cook直接依赖）
rust/crates/editor_core/src/project_font_cook.rs（迁移现有runtime package font cook owner）
rust/crates/editor_core/src/project_write_scope.rs（迁移Compiler/Delivery所需safe output owner）
rust/crates/editor_core/src/scene_editing.rs（只回接中立Scene schema；保留Editor commands/draft）
rust/crates/editor_core/src/scene_editing/document.rs（迁移canonical Scene document schema）
rust/crates/editor_core/src/scene_editing/entity.rs（迁移canonical Scene entity/component schema）
rust/crates/editor_core/src/scene_editing/transform.rs（迁移canonical transform schema）
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/tests/ai_capability_tool_kernel_tests.rs
rust/crates/editor_core/src/tests/ai_tool_catalog_tests.rs
rust/crates/editor_window_winit/Cargo.toml
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/tests/gateway_goal_mutation.rs
rust/crates/editor_host/Cargo.toml
rust/crates/editor_host/src/main.rs
rust/crates/editor_host/tests/gateway_process.rs
rust/crates/project_e2e_gate/Cargo.toml
rust/crates/project_e2e_gate/src/tests.rs（只迁移Gateway直接consumer资格测试）
rust/crates/project_e2e_gate/src/project_editor_composition.rs（只更新生产依赖闭包断言）
rust/crates/project_e2e_gate/src/project_runtime_native_module.rs（只更新生产依赖闭包断言）
rust/crates/project_e2e_gate/src/second_project_runtime.rs（只更新生产依赖闭包断言）
README.md（只修正公开启动/工具路径）
```

Gate F 的 owner 迁移只允许以下四类理由：

| 允许文件组 | 为何必要 | 替代的旧 owner |
| --- | --- | --- |
| F-B：Assembler + BuildProfile + artifact cache | canonical source 到 `RuntimePackageBuildInput` 的唯一入口 | `editor_core::ProjectRuntimePackageAssembler` |
| F-B：canonical Scene/Prefab/AUI/Input/Font/Animation/Asset 纯 schema/cooker | Assembler 的真实直接依赖；不允许依赖 `editor_*` crate | 当前散落在 `editor_core` 的中立 Implementation |
| F-C（已完成）：Player/staging/Desktop export/safe output/delivery verify | 现有 Windows dev build、Player 与 process evidence | `editor_core::ProjectPlayerArtifact/DesktopExportPipeline` |
| Editor preview/build/release/android/report consumers | 保持既有 Editor 行为并消费新 Interface | 旧 crate-local 调用；只做 Adapter/re-export |

实际编译错误若证明还缺一个 F-B 直接文件，必须先在本文记录“调用点、为何无法由已列文件解决、替代哪个旧
owner、对应哪个红测试”后才能加入；不得用模糊通配扩到 Renderer/ECS/AUI 功能、Project SDK、移动端
能力或完整 release pipeline。`project_authoring_execution/src/**` 虽已由 Gate B 创建，Gate F 仍只能在其中
新增上述 Compiler/Build 内部 Module，禁止建立第三个 compiler crate。

### 5.3 同窗迁移后允许删除

```text
rust/crates/ai_tool_gateway/**
```

删除前必须把仍有产品价值的 schema、Grant/operation/cancel/receipt、MCP framing、output limit、
Codex config artifact 和 process smoke 分别迁移到新 owner并先通过替代测试。`discovery.rs`、
`named_pipe.rs`、`remote_adapter.rs`、`editor_host.rs`、GatewayCore generation/digest/reconnect 状态和
Gateway-specific CLI 不迁移；它们在 Gate G 删除。不得将整个旧 crate 复制进新 crate 后换名。

### 5.4 明确禁止

```text
samples/tower_defense_project/** 与 Tower P1-2；
Renderer、ECS、AUI、Audio、Physics、Animation、Prefab、Asset DB 的功能扩张；
RuntimePackage/build/export pipeline 重写；
Agent Planner、LLM provider、第二模型循环、隐藏workflow recipe；
daemon、TCP/HTTP服务、discovery服务或必须常驻的后台进程；
项目canonical source格式迁移；
真实Codex配置、用户环境变量、production Editor/Player/MCP或release包替换；
Local CI、全workspace测试、移动端矩阵和无关视觉验收。
```

## 6. Gate A：激活前基线、调用图与红测试清单

Gate A 是只读复核。激活后也必须先执行，未通过不得修改源码：

1. 重新确认本文位于 `当前/` 且是唯一施工文档，Tower P1-2 仍停止。
2. 记录激活时 branch、HEAD、dirty files；绝不重置用户修改。
3. 冻结 `AiCapabilityToolKernel` 对 `EditorSession` 的每个直接调用及迁移 owner。
4. 冻结 Gateway migration/delete matrix、最终 `engine_*` 名称、schema、readiness 和 permission。
5. 用 `cargo metadata` 冻结直接依赖图；确认两个新增 crate 足够，不先加第三个框架 crate。
6. 为 Gate B-H 的每个行为写出能在旧拓扑上打红或静态失败的测试名。

激活后建议命令：

```powershell
Set-Location G:\gameEngin
git status --short
git branch --show-current
git rev-parse HEAD
rg -n -g "project_*.rs" "EditorSession|active_project_session|create_project_for_ai_tool|execute_command|scene_dirty|GameView|preview" rust\crates\editor_core\src\ai_capability_tool_kernel.rs rust\crates\editor_core\src
rg -n "GatewayCore|observed_project_digest|read_generation|read_stale_reason|discovery|named_pipe|GatewayRemoteAdapter|EditorGatewayHost" rust\crates\ai_tool_gateway rust\crates\editor_window_winit rust\crates\editor_host
Set-Location G:\gameEngin\rust
cargo metadata --no-deps --format-version 1
cargo test -p authoring_project_context --locked
cargo test -p editor_core --locked ai_capability_tool_kernel
cargo test -p ai_tool_gateway --locked
```

这些 baseline 命令只在激活后执行。若现有 owner、测试或依赖图已漂移，先修订本文并重新自审。

## 7. Gate B：Project Authoring Execution 中立抽离

先增加 neutral owner tests，再迁移现有工具真正需要的最小项目能力：

- `ProjectAuthoringSession/ProjectBinding` 隐藏 Context handle、root containment、refresh、snapshot lease、
  structured commit/rollback 与 recovery 编排；
- project observation、source/object/reference 查询和必要 diagnostics 从完整 `EditorSession` 参数脱离；
- build/runtime/evidence 只抽出项目绑定 adapter，不复制其底层 pipeline；
- `editor_core::ProjectSession` 改为消费新 owner；Editor draft/undo/selection 仍留在 Editor；
- 309-A-E 的 revision/snapshot/CAS/recovery/LWW 合同不改变。

Gate B 通过条件：新 crate 不依赖 Editor/Gateway，owner tests 可独立打开临时项目、recovery、refresh、
snapshot、inspect，并证明 Editor 与 Headless 看到相同 Context revision。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p project_authoring_execution --locked
cargo test -p authoring_project_context --locked
cargo test -p editor_core --locked editor_project_adapter
cargo tree -p project_authoring_execution --edges normal
```

## 8. Gate C：Capability 与 Tool Kernel 原子迁移

- 将现有 capability definitions 和 Kernel 迁到 `engine_tool_provider` 内部 Module；
- Kernel 公开 Interface 只接收 neutral project execution facts/operations，不出现 `EditorSession`；
- 保留并迁移 Grant、operation、cancel、receipt、rollback、diagnostic、idempotent replay 合同；
- 删除 AI 输入中的 `expected_project_digest`，移除 read generation、observed digest 和 Editor instance
  作为正确性前置；内部旧 digest 如短暂存在，只能是 Context revision 的单向投影并在 Gate G 删除；
- 动态 readiness 每次调用前向真实 owner 查询，不能由静态 catalog 假报。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_tool_provider --locked engine_tool_kernel
cargo test -p engine_tool_provider --locked grant_
cargo test -p engine_tool_provider --locked operation_
cargo test -p engine_tool_provider --locked rollback_
rg -n "EditorSession|expected_project_digest|read_generation|observed_project_digest" crates\engine_tool_provider crates\project_authoring_execution
```

最后一条命令在允许的文档/测试 fixture 之外命中生产代码即失败。

## 9. Gate D：Headless Provider、session 与文件工具兼容

实现 `attach/tool_definitions/invoke/observe/cancel/detach`，并覆盖：

- 确定性的 project locator 和单 session 单项目 binding；
- Context open/reuse/refresh，warm cache 关闭后结果不变；
- non-terminal operation 存在时拒绝 rebind；
- 新 session 不继承旧 Host approval/Grant；
- 普通文件工具改写 valid canonical source 后，下一 Engine 调用观察新 revision；
- 普通文件工具写出 invalid 内容后，下一调用返回新 invalid qualification，不回退旧事实；
- clean repeated inspect 不写 canonical bytes、不改变 mtime。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_tool_provider --locked provider_session_
cargo test -p engine_tool_provider --locked project_locator_
cargo test -p engine_tool_provider --locked direct_file_refresh_
cargo test -p engine_tool_provider --locked warm_cache_is_not_authority
cargo test -p project_authoring_execution --locked refresh_
```

## 10. Gate E：Native Interface 与 MCP Adapter

- `NativeHostAdapter` 以 Rust API 消费同一 Provider Interface，并有独立 conformance fixture；
- MCP stdio binary 直接调用 Provider，不发现 Editor、不连 named pipe；
- `tools/list` 只列具体 `engine_*` tools，每个 schema 独立验证；
- `tools/call` 注入 host/session/call/permission facts，保持 canonical result/diagnostic/evidence identity；
- 输出上限、取消和 protocol error 与 engine rejection 可区分；
- Codex config installer 只改为生成指向新 binary 的 artifact，并在隔离临时配置中测试。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_tool_provider --locked native_provider_conformance
cargo test -p engine_tool_provider --locked mcp_
cargo test -p engine_tool_provider --test mcp_process_smoke --locked
cargo test -p engine_tool_provider --test host_adapter_equivalence --locked
cargo test -p engine_tool_provider --test isolated_codex_config --locked
```

测试不得编辑 `%USERPROFILE%\.codex` 或当前 Codex 真实配置。F11 的真实宿主验收另见 Gate H。

## 11. Gate F：当前 ready run/build/delivery 的必要 Headless 迁移

用户已确认方案 B（此前选项 1 的中立 Compiler owner 方案）。Gate F 在同一施工文档内按 F-A 至 F-F 执行；这些是一个 Atomic Cutover 内的
可诊断检查点，不是新施工文档或可单独发布阶段。F-A 到 F-F 全绿前 Gate F 不算完成，不得进入 Gate G。

所有子步骤采用 replace-don't-layer：新 Interface 行为测试通过后立即删除对应旧 owner和重复内部测试，
只允许 `editor_core` 保留 Adapter/re-export。禁止让旧 Editor pipeline 与新 Compiler pipeline 长期双份存在。
下列过滤测试每条必须实际执行至少一个测试；零匹配不能记为通过，名称漂移时先更新本文再继续。

### F-A 中立 Compiler Interface 与 owner tests

1. 在 `project_authoring_execution` 内建立 `GameProjectCompiler`，只暴露
   `prepare/run/build/verify` 和四类高层 request/result；不新增第三个 crate/trait 层。
2. `prepare` 只接收 operation-owned `ProjectSnapshotLease` 与 target profile；prepared result 绑定
   project identity、revision、profile 和 artifact digest，不能接收裸 `EditorSession` 或 Editor draft。
3. Compiler 必须从 lease-owned bytes 构造内部 SourceView；不得在获得 lease 后无校验地重新读取可变
   canonical source。若 toolchain 必须使用路径，内部 materialization 仍须绑定 lease identity且完全隐藏。
4. 写 Interface 红测试：同 revision deterministic prepare、R1 lease 在 live source 变为 R2 后仍只消费 R1、
   invalid/missing source fail-closed、prepared/delivery identity 不可跨项目或跨 revision 混用。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p project_authoring_execution --locked game_project_compiler_interface_
cargo tree -p project_authoring_execution --edges normal
```

### F-B 迁移 Assembler，不改行为

F-B 的唯一目标是完成 `SourceView -> RuntimePackageBuildInput` 的中立 ownership move；Player/export 不属于
本 Gate。按以下顺序执行：

1. 先在 `project_authoring_execution` 内建立仅消费 lease-owned bytes 的 Compiler SourceView 适配层和中立
   Assembler 模块；不得接收裸 `project_root` 作为正确性输入，也不得调用 `EditorSession`。
2. 将现有 `ProjectRuntimePackageAssembler`、`BuildProfile`、artifact cache 和其真实调用闭包内的 canonical
   schema/纯 cooker迁入该内部模块；保持它是源码到 `RuntimePackageBuildInput` 的唯一入口。
3. 对混合 Editor workflow Module 只抽出 Assembler 实际调用的读取/parse/cook 部分：Input 不迁移 UI model，
   Prefab 不迁移 stage UI，Scene 不迁移 draft/command，Asset Import 不迁移下载/审批/mutation 工作流。
4. 用现有复杂 shooter、AUI、Animation、Prefab、Rule、optional-domain 和 invalid-source fixtures 比较迁移前后
   status、build input、diagnostic 和 producer/cache report；不借迁移改变 schema或cook结果。
5. 新 owner tests、source-view lease test 和最小真实 Editor consumer 回归通过后，删除 `editor_core` 中
   Assembler Implementation 与重复测试，只保留明确 Adapter/re-export；禁止复制旧文件后两边维护。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p project_authoring_execution --locked project_runtime_package_assembler_
cargo test -p project_authoring_execution --locked animator2d_package_assembler_
cargo test -p project_authoring_execution --locked game_project_compiler_source_view_
cargo tree -p project_authoring_execution --edges normal
```

### F-C 迁移 Player/export owner，不改行为

1. 迁移 `ProjectPlayerArtifact`、runtime player staging、`DesktopExportPipeline`、safe project output 和
   Windows dev delivery verification 的中立 Implementation。
2. `RuntimePackageBuilder`、`runtime_cli` bounded process/verification 和 `engine_runtime` 保持原 owner；
   只由 Compiler/Build Module 调用，不移动、不复制。
3. 保持现有 project-rust build cache identity、artifact inspection、Windows package layout、RuntimePackage
   report、Player process gate和 junction/path containment 合同。
4. 新 owner测试通过后删除对应 `editor_core` Implementation 与重复测试。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p project_authoring_execution --locked project_player_artifact_
cargo test -p project_authoring_execution --locked desktop_export_
```

### F-D Editor Adapter 回接

1. `editor_preview_package`、`services/build_service`、`release_package`、`android_export`、
   `project_asset_import` 中原有 Assembler/Player调用改为消费同一 Compiler Interface或窄 re-export；
   只保持既有行为，不扩 Preview、release、Android 或 Asset Import 功能。
2. Editor 只保留 UI command、draft、selection、progress、Report Panel Adapter；不得重新拥有 prepare/build事实。
3. Editor 与 Provider 用同一 snapshot/profile 的 Compiler conformance test 得到相同 prepared identity、
   status和diagnostic；Editor draft 未保存时不能渗入 Headless prepare。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core --locked editor_preview_package_
cargo test -p editor_core --locked build_service_
cargo check -p editor_core --locked
```

### F-E Provider run/build/delivery 接入

1. `engine_project_run` 从 prepared RuntimePackage/现有 Runtime owner启动，不调用 Editor command、Play、
   GameView 或 retained preview frame。
2. `engine_project_build` 和 `engine_delivery_verify` 只调用 Compiler/Build Interface；新产物使用中立
   Engine Tool 路径/schema，不继续创建 `Gateway` 命名产物。
3. operation 全程绑定同一 snapshot revision；完成、超时、取消、compile failure、process failure 和
   verification failure 可区分。无真实 screenshot/report/evidence 时返回非 ready，不伪造 success。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_tool_provider --locked headless_run_
cargo test -p engine_tool_provider --locked headless_build_
cargo test -p engine_tool_provider --locked delivery_verify_
```

### F-F No-Editor process/evidence 验收

使用测试自有 disposable project/artifact root，在 Editor 未启动的独立 MCP process 中完成
`prepare -> run -> build -> verify`，证明 report/evidence 绑定同一 revision/artifact，且 Provider与
`project_authoring_execution` 依赖树无任何 `editor_*`/Gateway。测试必须管理并回收自己启动的子进程。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_tool_provider --test mcp_process_smoke --locked
cargo test -p project_authoring_execution --locked
cargo test -p engine_tool_provider --locked
cargo test -p runtime_cli --locked
cargo test -p engine_runtime --locked
cargo tree -p project_authoring_execution --edges normal
cargo tree -p engine_tool_provider --edges normal
```

本文不授权触碰 Tower 项目、现有 delivery、production Player、移动端工具链或真实 Codex 配置。

## 12. Gate G：Editor cutover 与 Gateway 退役

只有 Gate B-F 均通过后才能执行终态删除：

- Editor composition 删除 Gateway field、wake、frame pump、approval panel coupling、host start/stop 和
  Editor instance identity；保留 Editor 自身验证 UI，不强迫 Editor 注册 AI tools；
- `editor_host`、`editor_window_winit` 移除 `ai_tool_gateway` 依赖与 Gateway tests；必要行为改由
  Provider conformance tests 覆盖；
- `project_e2e_gate` 移除 `ai_tool_gateway` 依赖；原 unknown-task Gateway 测试迁移为 Provider
  session/concrete-tool/operation 行为测试并改名为
  `provider_unknown_task_preflight_keeps_large_competition_margin`，其它文件只更新 workspace 生产依赖闭包断言；
- workspace 移除 `ai_tool_gateway` member，并删除该 crate；
- 删除 discovery publication/read、named pipe server/client、GatewayRemoteAdapter、GatewayCore、
  read generation/observed digest/reconnect 和旧 Gateway CLI 的生产达性；
- 不保留 feature flag、fallback、双写或兼容 binary；历史证据只留在 Git/历史文档。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core --locked
cargo test -p editor_window_winit --locked
cargo test -p editor_host --locked
cargo test -p project_e2e_gate --locked provider_unknown_task_preflight_keeps_large_competition_margin
cargo metadata --no-deps --format-version 1
cargo tree -p engine_tool_provider --edges normal
cargo tree -p project_authoring_execution --edges normal
Set-Location G:\gameEngin
rg -n "ai_tool_gateway|GatewayCore|GatewayRemoteAdapter|EditorGatewayHost|read_generation|observed_project_digest|connect_editor|discover_editor" rust\Cargo.toml rust\crates README.md
```

终态 `rg` 允许命中明确标注的 migration/negative test string；每个命中必须逐条分类记录。生产代码、
Cargo dependency 或启动文档命中即 Gate G 失败。

## 13. Gate H：F1-F12 资格与端到端验收

### 13.1 Source/integration completion

先用隔离环境完成不修改用户配置的资格：

| 正式资格 | 必须证据 |
| --- | --- |
| F1 Neutral Dependency | `project_authoring_execution`/Provider `cargo tree` + Kernel public Interface 静态扫描无 Editor/Gateway |
| F2 No-Editor Open | 独立 MCP process 在 Editor 未启动时 open/recovery/refresh/snapshot/inspect |
| F3 Concrete Projection | `tools/list` 仅具体 `engine_*`，schema 独立通过 |
| F4 File Refresh | 外部 valid/invalid 两条写入后均观察新事实 |
| F5 Mutation/Recovery | 双 Provider CAS、crash recovery、unknown write fail-closed、sealed receipt、exact rollback |
| F6 Run/Build/Delivery | 同一 snapshot revision 的中立 prepare 与无 Editor 真实 Runtime/Player report/evidence |
| F7 Approval/Grant | permission matrix、scope/risk、拒绝/取消/unsupported 区分 |
| F8 Native/MCP Equivalence | 同 revision/input 的 canonical status、diagnostic、receipt/evidence、side effect 等价 |
| F9 Optional Editor | 同项目同 revision；draft隔离；clean/LWW/CAS；Editor 不启动 Gateway |
| F10 Gateway Retirement | production source/composition 无 discovery/named pipe/Editor id/Gateway fallback |
| F12 Regression/Performance | 309-A-E、Kernel合同、MCP output limit、clean inspect、cache-off、无每帧热路径 |

建议最小受影响闭包：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --locked
cargo test -p project_authoring_execution --locked
cargo test -p engine_tool_provider --locked
cargo test -p editor_core --locked
cargo test -p editor_window_winit --locked
cargo test -p editor_host --locked
cargo test -p project_e2e_gate --locked provider_unknown_task_preflight_keeps_large_competition_margin
cargo test -p runtime_cli --locked
cargo test -p engine_runtime --locked
cargo fmt --all -- --check
cargo metadata --no-deps --format-version 1
```

不得机械扩大为 `cargo test --workspace` 或 Local CI。若受影响闭包经 Gate A 的真实依赖图增加直接
consumer，只追加该 consumer 并记录原因。

### 13.2 F11 Real Codex Tool-call Acceptance

F11 是必要的最终产品验收，但真实 Codex 配置修改、安装态 binary 替换和新 Agent task 均属于外部状态，
需要用户在源码/integration completion 后再次明确授权。未获授权时：

```text
F1-F10/F12 可以记录 source/integration passed；
F11 必须记录 blocked_pending_user_authorization；
309-F 不得标记完成、不得归档、不得退役用户当前安装态 Gateway。
```

获单独授权后，使用 disposable project 和事务化配置备份/恢复完成唯一链路：

```text
Editor 未启动
Codex tools/list -> concrete engine_* tools
engine_project_inspect
普通 file edit 或 engine_project_mutate
engine_project_run / check
engine_project_build
engine_delivery_verify
诊断返回同一 Agent turn，Codex 可据此修复
```

必须记录 config/binary identity、project root、每次 call id、revision、result、receipt/evidence 和恢复状态。
不得把 `catalog/execute`、Context 调用或 Editor connection 步骤藏入验收脚本。

## 14. Gate I：终态文档、完成记录与归档

仅当 F1-F12 全部通过后执行：

1. 生成 `阶段完成记录/2026-09-xx-309-F-Headless-Engine-Tool-Provider-Atomic-Cutover-v1/00-总览.md`，
   记录实际改动、红/绿首因、测试、外部激活、遗留非 ready 能力和未执行项。
2. 更新正式 309、309-F、权威入口、`00-文档地图.md`、`00-阅读顺序.md`、49、54 和施工 README。
3. 将本文从 `待执行/` 激活时先移到 `当前/`，完成后再移到 `已完成/`；任何中间 Gate 不得归档为
   “部分生产切换完成”。
4. 旧 Gateway 只作为 Git/历史文档证据，不恢复为生产 fallback。

Gate I 已在 F1-F12 全部通过后执行；完成记录已生成，本文已由 `当前/` 归档至 `已完成/`。

## 15. 失败、回退与停止规则

- Gate A-G 失败：保留在同一施工窗口修复当前 Gate，不激活安装态，不发布中间拓扑。
- 尚未发布需整体撤回：只通过版本控制撤回整个 Atomic Cutover；不得在代码中保留 runtime fallback。
- F11 未获授权或外部宿主失败：保持源码/integration 状态并记录阻塞，不假报完成，不恢复 Gateway authority。
- 若 run/build 需要重写底层 pipeline、Kernel 迁移需要重写全部 `editor_core`、或出现第三套 revision/lock，
  立即停止并回到正式方案复核。
- 用户工作区已有变更。任何冲突都必须与现有变更协作，禁止 reset/checkout 覆盖。

## 16. 施工完成定义

以下条件缺一不可：

1. F1-F12 全部有通过证据，F11 不能以隔离 MCP smoke 替代真实 Codex Agent Tool Loop。
2. 两个新增 crate 形成冻结依赖方向；Provider/Kernel/MCP 无 Editor/Gateway 依赖。
3. AI 工具表面只有具体 `engine_*` tools，Context lifecycle 和 revision token 不暴露给 AI。
4. Context revision 是唯一 project revision authority，普通文件 refresh、LWW 与结构化 CAS 均保持。
5. `project_authoring_execution::GameProjectCompiler` 是当前 canonical source 到 prepared
   RuntimePackage/Player/Windows dev delivery 的中立高层 owner；Editor 与 Provider 消费同一 Interface，
   旧 Editor Implementation 与重复测试已删除。
6. 当前 ready 的 run/build/delivery 在无 Editor 下产生绑定同一 snapshot revision 的真实 report/evidence；
   其余能力诚实标为非 ready。
7. Native/MCP canonical equivalence、approval/Grant、operation/cancel/receipt/rollback 合同通过。
8. Editor 不默认启动 Gateway；旧 Gateway crate、discovery、named pipe、Editor identity 和 fallback 退出生产组合。
9. 最小受影响闭包、格式、静态依赖和无热路径回归通过；未擅自运行 Local CI。
10. 真实配置/安装态变更有单独用户授权和可恢复 receipt。
11. 完成记录与全部入口同步，本文归档到 `已完成/`。

## 17. 文档自审（2026-09-01）

| 自审项 | 结果 |
| --- | --- |
| 与 309-F 正式方案 C-Atomic 冲突 | 无；保持一次替换、无 daemon、无生产 fallback |
| F1-F12 覆盖 | 全部覆盖；F11 单列真实宿主授权与验收 |
| 长期双架构 | 未建立；中间共存仅限未发布施工窗口，Gate G 必须删除旧路径 |
| 未来 R2-R5 过量并入 | 无；只迁移现有 capability set、Assembler/Player/Windows dev delivery 必要 subset，不宣称完整 R3 |
| crate/trait 过拆 | 无；只冻结两个新 crate，内部 Module 不机械拆分 |
| Provider/Kernel 依赖 Editor | 明确禁止，并由 Gate B/C/G/H 静态与行为验证 |
| Compiler/Build owner | 方案 B 已确认；`project_authoring_execution` 内深 Module，Compiler 内部唯一 Assembler，Editor/Provider 两个真实 consumer |
| Preview/Build 范围 | F-B 迁移既有 Assembler/中立 schema-cooker；F-C 才迁移 Player/Desktop export；RuntimePackageBuilder/runtime_cli保持原 owner |
| replace-don't-layer | F-B/F-C/F-D 明确要求新 Interface 通过后删除旧 owner和重复测试，不保留双 pipeline |
| 权限与 authority | Host approval、Engine Grant、Context revision 分层，AI 不见内部 token |
| 普通文件工具、LWW、CAS | 保持 309-E 与 309-A-D 既有语义 |
| 每个 Gate 能打红 | A 为只读阻断；F-A至F-F逐项有owner/consumer/process/static红测试；G-H有退役/资格Gate；I由F1-F12阻断 |
| 无关验证 | 未要求全 workspace、Local CI、Tower、移动端或视觉矩阵 |
| 真实 Codex 外部修改 | 明确要求独立用户授权；未获授权不得完成/归档 |
| 当前施工状态 | Gate A-I、F1-F12 和 F11-R3 真实 Codex 人工 Agent Tool Loop 全部通过；完成记录已生成，本文已归档，当前施工槽与待执行队列均为空 |
| 本轮实际动作 | 人工 projectless `HOST_READY` 与 F11-R3 唯一 Agent Tool Loop 均通过；真实 task/turn、五个 Engine tool call、mutation receipt、Run/Build/delivery report 已核验，配置/Provider/进程已恢复；Gate I 只同步终态文档并归档 |

自审结论：文档边界自审通过；F-B 至 F-F、Gate G 与 Gate H 13.1 已按 replace-don't-layer、中立 owner 和资格边界完成，F11-R3 已通过且外部状态已恢复，F1-F12 全部通过。Gate I 已完成终态文档、完成记录、入口同步与归档；309-F 整体完成，不自动进入后续施工。

## 18. 激活前复核记录（2026-09-01）

用户已明确授权按本文开始施工。只读复核确认：

- `当前/` 没有另一份施工方案，309-F 是 `待执行/` 中唯一施工文档；
- branch/HEAD 仍为 `codex/255-capability-catalog` / `acaffa8cb95bbb54d093bbf45835a59315c82da5`；
- 工作区有大量既有用户变化，必须原样继承，禁止 reset/checkout/清理；
- Kernel 对 `EditorSession`、Preview/GameView 的依赖与正式方案一致；
- Gateway discovery/named-pipe/generation/Editor host 路径仍在，Atomic Cutover 仍有必要；
- `cargo metadata --no-deps` 成功；两个新增 owner crate 的设计仍成立；
- 发现 `project_e2e_gate` 是 `ai_tool_gateway` 的第三个直接 consumer，已在本次复核中加入
  允许文件、Gate G 和最小受影响闭包；它只迁移直接资格测试和依赖断言，不扩展 E2E 产品功能；
- 未发现需要新增第三个框架 crate、daemon、兼容层或未来 R2-R5 能力的理由。

复核结论：允许激活本文。激活后先运行 Gate A baseline tests；失败时只修复当前 Gate，不跨 Gate。

## 19. 2026-09-01 施工进展与 Gate F 停止/恢复记录

### 19.1 已完成证据

- Gate A baseline 全绿：`authoring_project_context` 45/45、`editor_core ai_capability_tool_kernel`
  31/31、`ai_tool_gateway` 64 个单元测试及集成测试通过。
- Gate B 新增且只新增 `project_authoring_execution` 中立 owner；Context open/recovery/refresh、snapshot
  lease、document save、mutation/rollback/receipt 与 source inventory 已封装，Editor ProjectSession 已改为
  消费该 owner。owner 3/3、scene save contract 10/10 通过。
- Gate C-D 新增 `engine_tool_provider`：具体 `engine_*` definition、Host approval、operation-bound
  Engine Grant、Context revision 注入、snapshot lease、CAS mutation、opaque rollback、exact replay、
  operation observe/cancel、单 session 单项目绑定、valid/invalid 外部文件 refresh、clean inspect 与
  session 隔离均已实现。Provider owner 14/14 后扩展为 17/17 通过；静态扫描无
  `EditorSession/expected_project_digest/read_generation/observed_project_digest`；依赖树无 Editor/Gateway。
- Gate E 新增同 crate Native Adapter、MCP stdio Adapter/binary 和仅生成 artifact 的隔离 Codex 配置
  helper。终态 `cargo test -p engine_tool_provider --locked` 通过：17 个库测试、2 个 Native/MCP
  equivalence 测试、1 个 isolated config 测试、1 个真实 MCP process smoke；没有 discovery、named pipe、
  Editor 连接或真实用户配置写入。
- Gate E 首次 suite 唯一失败是 Windows 临时目录 requested path 与 canonical path 比较口径不同；修正
  test identity 后定向测试与终态 suite 均通过，产品实现未因该失败修改。

### 19.2 Gate F 实际阻塞

调用图确认不存在施工文档预设的“窄 Headless build adapter”：

```text
runtime_cli / engine_runtime
  - 可以运行或验证已经存在的 RuntimePackage / exported package
  - RuntimePackageBuilder 只消费已经组装好的 RuntimePackageBuildInput

editor_core::ProjectRuntimePackageAssembler
  - 才负责 canonical project source -> RuntimePackageBuildInput
  - 直接依赖 Scene、Prefab、AUI、Asset、Font、Input、Rule、Animation 等 Editor-owned authoring 类型

editor_core::DesktopExportPipeline / ProjectPlayerArtifact
  - 才负责 project player 构建、RuntimePackage build、Windows staging/export 与 process verification
```

所以当前不能同时满足：

1. Provider/Kernel/MCP 不依赖 `editor_core`；
2. 不复制或重写 RuntimePackage/build/export pipeline；
3. `engine_project_run/build/delivery_verify` 迁移现有真实 No-Editor 链路。

继续硬做只会形成三种之一：Provider 反向依赖 Editor、复制一套 pipeline、或伪造 ready；三者均违反
309-F 正式方案和本文第15节停止规则。Gate F 因此记为
`blocked_pending_formal_design_review`，不是实现失败，也不是把工具假报完成。当前 Provider 继续将
`engine_project_run`、`engine_runtime_capture_issue`、`engine_project_build`、`engine_delivery_verify`
诚实标为 `unavailable`。

### 19.3 当时必须选择的正式设计边界

停止时只能先回到正式方案选择一项，未经确认不得继续 Gate F/G：

1. 推荐：把 `Project -> RuntimePackageBuildInput -> Player/Export` 抽为真正中立的 Game Project Compiler /
   Build owner，再由 Editor 与 Provider 共同消费；这是架构性 ownership 迁移，需要单独修订允许文件与
   施工预算，不能伪装成一个窄 adapter。
2. 缩小 309-F：允许 run/build 保持 unavailable，仅完成 read/mutate/MCP/Gateway cutover；这会降低已经
   确认的 309-F 完成定义和用户“AI 可直接运行、测试、打包”需求，不推荐。
3. 让 Provider 或 MCP 依赖/启动 Editor 组合；这直接违反权威需求，正式拒绝。

该停止条件已由用户在 2026-09-01 正式确认选项 1；处理结果见 19.4。F11 仍为
`blocked_pending_user_authorization`。

### 19.4 方案 1 确认与 Gate F 恢复入口

用户已确认把 `Project canonical source -> RuntimePackageBuildInput -> Player/Export` 迁为中立 Game
Project Compiler/Build owner。正式方案和本文已同步修订：

- 不新增第三个 compiler crate；owner 是 `project_authoring_execution::GameProjectCompiler` 深 Module；
- 对外 Interface 冻结为 `prepare/run/build/verify`，Compiler 接受 operation-owned snapshot lease；
- 迁移既有 Assembler、Player、Windows dev export 和必要中立 schema/cooker，不重写 pipeline；
- `RuntimePackageBuilder`、`runtime_cli`、`engine_runtime` 保持现有 owner；
- Editor 与 Provider 成为同一 Interface 的两个真实 consumer；采用 replace-don't-layer；
- Gate F 拆为同一 Gate 内的 F-A 至 F-F，全部通过前不得执行 Gate G；
- Tower、移动端能力、完整 R3、Local CI、production/真实配置仍不在授权范围。

因此 `blocked_pending_formal_design_review` 已解除。Gate F-A 已按下节完成；不得从历史停止点直接删除
Gateway或把当前 unavailable 的 run/build/delivery 假报 ready。F11 继续等待源码/integration completion 后的
独立用户授权。

### 19.5 Gate F-A 中立 Compiler Interface 与 owner tests 完成证据

2026-09-01 恢复并完成 Gate F-A，仅修改 `project_authoring_execution` 的中立 Compiler Interface、
owner tests 与直接散列依赖：

- 新增 `GameProjectCompiler` 深 Module，对外冻结 `bind/prepare/run/build/verify`；`prepare` 只消费
  operation-bound `ProjectSnapshotLease` 的 bytes，并绑定 project、root binding、revision、snapshot、profile
  与 source/preparation identity；
- 同 revision prepare、R1 lease/live R2、invalid/missing manifest、跨项目/跨 revision 以及高层 Interface
  类型均有 owner test；
- `run/build/verify` 在 F-B/F-C 尚未迁移前统一返回
  `game_project_compiler.capability_not_migrated`，没有伪造 RuntimePackage 或 delivery ready；
- 未新增第三个 crate/trait 层，未迁移 Assembler、Player、Export，未修改 Editor、Runtime 或 Gateway。

红绿证据：首次红测试因 Interface 尚不存在而 unresolved imports；实现后的首轮 5-test 过滤运行中，4 个
通过，1 个因测试试图直接打开非法 manifest 而被 Context 更早 fail-closed。将夹具改为“valid open -> 外部
写坏 manifest -> refresh 得到 invalid revision lease”后，没有修改产品行为，终态证据如下：

```text
cargo test -p project_authoring_execution --locked game_project_compiler_interface_  -> 5/5 passed
cargo test -p project_authoring_execution --locked                                  -> 8/8 passed
cargo tree -p project_authoring_execution --edges normal                             -> passed
```

依赖树只有 `authoring_project_context` 与 `sha2`，无任何 `editor_*` 或 `ai_tool_gateway`。Gate F-A 完成；
（历史记录）Gate F-B 当时尚未形成可施工的 neutral owner；正式 ownership 拆分复核已在 19.7 完成，当前按修订边界恢复，Gate G 继续禁止。

### 19.6 Gate F-B 施工前置检查与阻塞证据（历史记录）

2026-09-01 已按用户授权开始 Gate F-B，但在最小可行性检查处停止，原因是当前方案缺少可执行的 Assembler
ownership 拆分，继续实现会违反“Provider/Compiler 不依赖 Editor、不得复制 pipeline、不得伪造 ready”的停止规则。

已执行：

```text
cargo test -p project_authoring_execution --locked project_runtime_package_assembler_ -> 0 tests（不可记为通过）
cargo test -p project_authoring_execution --locked animator2d_package_assembler_     -> 0 tests（不可记为通过）
cargo test -p editor_core --locked project_runtime_package_assembler_                 -> 9/9 passed
cargo test -p editor_core --locked animator2d_package_assembler_                      -> 1/1 passed
```

调用图和 owner 事实：

- `editor_core/src/project_runtime_package_assembler.rs` 为 3,850 行单一实现，公开 `ProjectRuntimePackageAssembler::assemble`；
- 该实现直接依赖 Editor-owned `EditorSceneDocument`、Prefab workflow、AUI cooker、Font cook、Input authoring、Asset Import
  和 Project Manifest 等模块，同时产出 `RuntimePackageBuildInput`、Prefab bake、AUI、Texture、Animator、Rule、Font 和
  observation 数据；
- 生产调用方至少包括 `editor_preview_package`、`desktop_export`、`android_export`、`release_package` 与
  `project_asset_import`；Assembler 自身已有 9 个行为测试和 1 个 Animator2D 测试；
- `project_authoring_execution` 当前没有 F-B owner implementation，也没有 Assembler/Animator owner test；直接复制文件会
  形成第二套 pipeline，直接依赖 `editor_core` 则违反 Gate F-A 依赖方向。

阻塞分类：`blocked_pending_formal_design_review`，不是产品测试失败。恢复条件：先在正式方案中冻结
`canonical source -> RuntimePackageBuildInput` 的中立 schema/cooker ownership 列表、Editor workflow 排除列表、
最小真实 consumer 闭包和 replace-don't-layer 删除点；随后修订本施工文档的允许文件与 F-B Gate 测试，再恢复施工。
在复核完成前，不迁移 Assembler 文件、不新增第三 crate、不修改 Editor consumer、不进入 F-C。

### 19.7 2026-09-01 方案 B ownership 复核与 F-B 恢复

用户确认采用方案 B。正式边界复核结论如下：

- `GameProjectCompiler` 是唯一高层 `prepare` owner，负责 project/revision/profile、lease、SourceView、cache、
  diagnostics 和 prepared identity；
- `ProjectRuntimePackageAssembler` 是 Compiler 内部唯一 `RuntimePackageBuildInput` 装配 owner，不是并列的
  第二 Compiler，也不向 Provider 或 Editor 暴露；
- Assembler 只消费 lease-owned `SourceView` 与中立 schema/cooker，禁止接收裸 `project_root` 作为正确性输入，
  禁止重新读取 live filesystem，禁止依赖 `EditorSession`；
- F-B 只迁移 Assembler、BuildProfile、artifact-cache 及真实调用闭包内的中立 Scene/Prefab/AUI/Input/Font/
  Animation/Rule/Asset/Texture/observation schema/read/parse/cook；Player、Desktop export、staging、safe output
  和 delivery verify 留在 F-C；
- Editor draft、selection、undo、stage/command、Asset Import 下载/审批/mutation、窗口/GameView、progress 和
  Report Panel 明确排除；Editor 只保留 Adapter/re-export，Provider 不复制 pipeline；
- 真实最小 consumer 闭包为 `editor_preview_package`、`desktop_export`、`release_package`、`android_export`、
  `project_asset_import` 的 Assembler 直接调用点；F-B 只回接其 prepare/assembly 依赖，不改变 Player/export 行为；
- 不新增第三 crate，不修改 RuntimePackageBuilder、runtime_cli、engine_runtime，不进入 F-C/F-D/G。

正式方案、本文允许文件、F-B 步骤与入口摘要已同步。`blocked_pending_formal_design_review` 已解除；F-B 现在
可恢复，恢复后的首个动作是 SourceView/Assembler owner red test，零匹配仍不得记为通过。

### 19.8 F-B SourceView 前置切片证据

已按恢复后的 F-B 文档执行首个最小动作：在 `project_authoring_execution::GameProjectCompiler` 内新增
`CompilerSourceView`。它从 `ProjectSnapshot` 的 lease-owned bytes 构造有序内存视图，校验文件长度、digest、
唯一性、规范顺序和 manifest scope；当前没有任何 live filesystem fallback。

```text
cargo fmt --package project_authoring_execution -- --check -> passed
cargo test -p project_authoring_execution --locked game_project_compiler_source_view_ -> 1/1 passed
```

第一次测试失败是夹具漏填 `ProjectRevision.diagnostics_digest`，第二次失败是夹具文件顺序不符合 canonical
排序；两次均为测试夹具问题，未改变产品语义。当前仍未搬迁 `ProjectRuntimePackageAssembler`、未修改
`editor_core` consumer、未运行 F-C/F-D/G；下一步必须在此 SourceView 上建立真实 Assembler owner red test，
再按允许闭包迁移实现。

### 19.9 F-B Assembler owner 红测与快照规模核对（2026-09-01）

已建立真实 consumer 红测：

```text
cargo test -p project_authoring_execution --locked game_project_compiler_assembler_owner_prepares_complex_shooter_runtime_input -- --exact --nocapture
```

测试使用 `samples/complex_shooter_project`，通过 `ProjectAuthoringSession` 获取完整
operation-bound snapshot lease，并调用 `GameProjectCompiler::prepare`。首个失败断言为：

```text
F-B requires the Compiler's internal Assembler to produce a RuntimePackageBuildInput
```

当前 `prepare` 返回 `runtime_package_identity=None`，所以失败分类为“中立 Assembler 尚未施工”，不是
fixture、快照或环境失败；该测试保留为 F-B 的 owner red test，未修改生产语义。

只读规模核对显示复杂 Shooter 的 canonical source inventory（排除 `Build/`、`target/`、`Library/`、
`.aife/`、`.git` 生成状态）为 31 个文件、约 1.25 MB，包含 `project.aife.json`、默认 Scene、Build Profile、
AUI、Rule Manifest、Prefab、Asset 与 RuntimeModule 源。当前 operation-bound snapshot 机制按路径复制
lease-owned bytes，没有容量上限或 live filesystem fallback；因此没有发现需要先改 Snapshot schema/容量合同
的证据。后续应在现有 `CompilerSourceView` 上迁移中立 Manifest/BuildProfile/Scene schema 的最小读取层，
再接入唯一 Assembler owner；不得因该规模核对扩展到 F-C Player/Export。

### 19.10 F-B Manifest lease-owned 读取层（2026-09-01）

在保持 Assembler 红测失败的前提下，完成了 F-B 的第一个中立读取增量：

- `project_authoring_execution` 增加直接 `serde` / `serde_json` 依赖；
- `CompilerSourceView` 新增 lease-owned `project.aife.json` 解析；
- `GameProjectCompiler::prepare` 在继续生成 prepared lineage 前校验 Manifest JSON，并校验其
  `projectId`（存在时）与 Context portable project identity 一致；
- 解析过程只访问 snapshot 中的 bytes，不接受 `project_root`，不重新读取 live filesystem；
- 未迁移 Scene、Prefab、Asset、AUI、Input、Font、Animation、Texture 或 RuntimePackageBuilder，
  未改变 Editor consumer，也未进入 F-C。

定向证据：

```text
cargo check -p project_authoring_execution
cargo fmt --package project_authoring_execution -- --check
cargo test -p project_authoring_execution --locked game_project_compiler_interface_ -- --nocapture
cargo test -p project_authoring_execution --locked game_project_compiler_source_view_ -- --nocapture
```

结果：Compiler interface 4 个集成测试通过，SourceView owner 测试通过，格式检查通过。Assembler
owner 红测继续按预期失败，首个断言仍为 `F-B requires the Compiler's internal Assembler to produce a
RuntimePackageBuildInput`；因此 F-B 仍未完成，不能宣称 `prepare/run/build/verify` 的真实 RuntimePackage
或 delivery 能力已 ready。

下一段施工入口是把 BuildProfile 与默认 Scene 的中立 schema/read 层接到同一个 SourceView，再迁移唯一
Assembler 的最小 Scene/Prefab 输入；若出现必须读取 live filesystem 或依赖 Editor workflow 的证据，必须先
在本文记录 owner 冲突，不得用兼容层或第二 pipeline 绕过边界。

### 19.11 F-B 大块施工粒度确认（2026-09-01）

根据用户最新要求，后续不再把 Manifest、BuildProfile、Scene、Prefab 等作为可交付的小步分别结束。
它们统一归入一个 `F-B Assembler Ownership Cutover` 大块，只有以下整体闭包全部完成后才允许把 F-B
标记为通过：

```text
operation-bound SourceView
  -> Manifest / BuildProfile / Scene / Prefab / AUI / Input / Rule
  -> Asset / Texture / Font / Animation / Observation cook
  -> 唯一 ProjectRuntimePackageAssembler
  -> RuntimePackageBuildInput
  -> Compiler prepared identity + diagnostics/report
```

大块施工的内部检查点只用于定位首因，不构成中间交付或阶段完成状态。中途不得：

- 将 `runtime_package_identity` 伪造为 ready；
- 让 `project_authoring_execution` 反向依赖 `editor_core`；
- 保留 Editor 与 Compiler 两套 Assembler pipeline；
- 偷渡 Player、Windows staging/export、delivery verify 或 Gateway 退役。

大块完成时必须一次性运行 F-B owner tests、复杂 Shooter/AUI/Prefab/Rule/Animation fixtures、最小
Editor consumer 回归和 `cargo tree` 依赖方向检查；未达到整体闭包时，施工状态保持
`F-B in progress`，不归档当前文档。

### 19.12 F-B 大块施工启动记录（2026-09-01）

已按用户授权启动完整 F-B 大块。启动预检确认：

- 当前 `editor_core::ProjectRuntimePackageAssembler` 约 3,850 行，直接调用 Scene、Prefab、AUI、
  Input、Rule、Asset、Texture、Font、Animation、Observation 等多组类型；
- `project_authoring_execution` 当前依赖树只包含 `authoring_project_context`、`serde`、`serde_json`、
  `sha2`，尚未反向依赖 `editor_core`，符合 F-A 的中立边界；
- 完整复杂 Shooter canonical inventory 为 31 个文件、约 1.25 MB，SourceView 具备承载条件；
- 真实 Assembler owner 红测仍以 `runtime_package_identity=None` 失败，说明 batch 尚未产生真实
  `RuntimePackageBuildInput`。

本记录不把预检或 Manifest 读取层算作 F-B 完成。后续必须在同一大块内完成中立 schema/cooker ownership
迁移、Compiler 接入和 Editor consumer 回归；若某个类型只能通过 Editor workflow 或 live filesystem
获得，必须先拆出纯读取/解析 owner 或记录边界冲突，不能通过 Editor 依赖、复制实现或伪造 ready 绕过。

### 19.13 F-B 大块当前施工证据（2026-09-01）

大块施工已接入第一批完整项目输入边界：Compiler 的 lease-owned `SourceView` 现在解析完整项目
Manifest，并在存在时解析并校验 `BuildProfiles/windows.dev.json` 的 `profile=dev`、`target=windows`。
该读取层仍完全以内存 snapshot bytes 为输入；没有新增 `project_root` 正确性参数，也没有依赖
`editor_core`。

验证结果：

```text
cargo fmt --package project_authoring_execution             -> passed
cargo test -p project_authoring_execution --locked game_project_compiler_interface_ -> 4 integration + 1 owner passed
cargo test -p project_authoring_execution --locked game_project_compiler_source_view_ -> passed
```

复杂 Shooter 的真实 Assembler owner 红测仍失败于 `runtime_package_identity=None`。因此本记录只证明
Manifest/BuildProfile 已进入中立 SourceView，不能证明 F-B 已完成；后续仍须在同一大块内迁移 Scene、
Prefab、AUI、Input、Rule、Asset、Texture、Font、Animation、Observation cooker 与唯一
`RuntimePackageBuildInput` 装配 owner。

### 19.14 F-B 依赖冲突与边界回填（2026-09-01）

五步整体施工的 owner 预检发现以下事实：

- Scene/Prefab/Input 的现有数据结构仍引用 `editor_ui_model`；
- Observation 仍直接引用 `EditorSession` 并执行 live filesystem 查询；
- Asset Import 与 Font cook 混合了 canonical read/cook 和 Editor 下载、审批、mutation、缓存工作流。

这不是允许把 `project_authoring_execution` 反向依赖 Editor 的理由。正式方案已回填混合模块拆分表：
F-B 只迁移 canonical schema、lease bytes parse、runtime conversion 和纯 cook；Editor workflow 保留在
Adapter，Editor 类型通过转换进入中立数据。该边界修订位于正式方案 22.6，作为本施工继续执行的约束。

因此当前施工状态为 `F-B in progress / boundary clarified`，而非 F-B 完成或永久阻塞。下一次继续施工
仍属于同一大块，但必须先按 22.6 建立中立 Scene/Prefab/Input 数据结构和 Adapter 转换，再接入完整
Assembler；禁止复制现有 3,850 行文件后双边维护。

### 19.15 F-B 中立 Assembler 首个真实闭包（2026-09-01）

已在 `project_authoring_execution` 内建立 lease-owned `CompilerSourceView` 驱动的中立
`neutral_assembler`，并由 `GameProjectCompiler::prepare` 直接调用。该实现只依赖
`authoring_project_context`、`engine_runtime`、serde/sha2 与 PNG cooker，不依赖 `editor_core`、
`editor_ui_model`、`EditorSession` 或 live filesystem。

`PreparedRuntimePackage` 现在携带真实 `RuntimePackageBuildInput`、`assembly_digest` 和稳定
`runtime_package_identity`；复杂 Shooter owner test 已验证 1 个 Scene、3 个 Prefab、1 个 AUI、1 个
Input、6 个 Asset、5 个 Texture payload、5 条 Rule 均来自 lease-owned source，且同 revision prepare
得到相同 digest。PNG RGB/RGBA/灰度输入在 cooker 中统一为 RGBA8。

本次证据：

```text
cargo fmt --package project_authoring_execution
cargo test -p project_authoring_execution --locked       -> 10/10 passed
cargo tree -p project_authoring_execution --edges normal -> no editor_* dependency
cargo check -p editor_core --locked                       -> passed
```

该记录不宣称 F-B 完成。`editor_core::ProjectRuntimePackageAssembler` 旧实现及其 Preview/Export
消费者尚未删除或统一回接；下一段仍属于同一个 F-B 大块，必须完成 Adapter/consumer 回归后才能执行
replace-don't-layer 删除，并再次运行 F-B 终态验证。

### 19.16 F-B Assembler Ownership Cutover 完成证据（2026-09-01）

F-B 已完成，不再保留 Editor 与 Compiler 两条装配 pipeline：

- `editor_core::ProjectRuntimePackageAssembler` 已成为窄 Adapter，路径固定为
  `project_root -> ProjectAuthoringSession -> operation-bound snapshot lease -> GameProjectCompiler::prepare`；
- 中立 Assembler 只消费 `CompilerSourceView` 的 lease-owned bytes，`GameProjectCompiler::prepare` 返回真实
  `RuntimePackageBuildInput`、assembly digest、runtime package identity 与 producer reports；
- Scene、Prefab、AUI、Input、Rule、Asset、Texture、Animation、Observation 继续由中立 Assembler 统一 cook；
- Font schema、bitmap/MSDF cooker、FontBundle pack、sealed built-in FontPack loader 与 artifact cache 已迁入
  `project_authoring_execution`；项目 Font source 从 snapshot 内的 meta/source bytes 解析，不读取 live project tree；
- Editor 只 re-export 中立 Font/Cache 类型；旧 Font schema/cooker/bundle/cache、旧 C-min atlas fallback、旧
  3,850 行 Assembler Implementation 与重复测试文件均已删除；
- Preview、Desktop Export、Release、Android 与 Asset Import 五个现有调用点只剩 Adapter 一条入口；本 Gate
  没有迁移 Player、Desktop staging/export owner、delivery verify、Gateway 或 production 配置。

终态证据：

```text
cargo fmt --package project_authoring_execution --package editor_core --package project_e2e_gate -- --check
  -> passed
cargo test -p project_authoring_execution --locked
  -> 30 unit + 6 integration passed
cargo check -p editor_core --locked
  -> passed
cargo tree -p project_authoring_execution --edges normal
  -> no editor_* / ai_tool_gateway dependency
cargo test -p project_e2e_gate --lib --locked font_bundle_compatibility
  -> 3/3 passed (project FontBundle v2, producer cache Produced->Hit, built-in FontPack)
cargo test -p project_e2e_gate --lib --locked animator2d_generic_and_second_project_use_the_same_cooked_fixed_tick_projection_chain
  -> 1/1 passed
cargo test -p project_e2e_gate --lib --locked prefab_runtime_bake_report_expands_scene_prefab_instances
  -> 1/1 passed
cargo test -p editor_core --lib --locked editor_preview_package_cache_rebuilds_then_hits
  -> 1/1 passed
cargo test -p editor_core --lib --locked desktop_export_complex_shooter_package_contains_assembled_domains
  -> 1/1 passed
cargo test -p editor_core --lib --locked release_package_builds_portable_manifest_and_replaces_stale_payloads
  -> 1/1 passed
cargo test -p editor_core --lib --locked project_asset_import_end_to_end_drives_browser_runtime_package_and_rollback
  -> 1/1 passed
cargo test -p editor_core --lib --locked engine_builtin_font_pack_embedded_seal_loads_when_present
  -> 1/1 passed
```

首次无 `--lib` 的 Font E2E 因 Windows 页面文件不足同时编译多个无关 test target，未取得产品 verdict；随后
限定同一 lib test target 后得到真实结果。三个 Font 测试首次并行运行时，两个测试共享同一 canonical fixture
导致一次 Context 竞争；测试改为各自复制独占临时项目根后，默认并行 3/3 通过，产品实现未为该测试竞争修改。

因此 F-B 记为 `completed`。309-F 仍处于施工中，不生成阶段完成记录、不归档本文；下一 Gate 是 F-C
Player/export owner migration，必须由后续明确施工指令开始。

### 19.17 F-C Player/export Ownership Cutover 完成证据（2026-09-01）

F-C 已按现有行为迁入 `project_authoring_execution` 的中立 Compiler/Build owner，不再由
`editor_core` 持有实现：

- `ProjectPlayerArtifact`、runtime player production staging、`DesktopExportPipeline`、safe project output
  和 Windows dev delivery verification 已迁入中立 crate；
- `RuntimePackageBuilder`、`runtime_cli` bounded process/verification 与 `engine_runtime` 保持原 owner，
  本 Gate 只复用它们，没有移动、复制或重写；
- project-rust build cache identity、artifact inspection、Windows 包中的 `Game.exe`、
  `data/runtime_package`、package manifest、process verification report、Player exit reason/code 与路径安全合同保持；
- `editor_core` 对应文件已收缩为窄 re-export Adapter，旧 Implementation 与文件内重复测试已删除；
- 新中立 owner integration test 使用复杂 Shooter 验证真实 Windows dev package 与 exported Player process，
  并单独验证 project-relative output authority 和 delivery junction containment；
- 本 Gate 只做 F-C 所必需的窄 Adapter 回接，没有开始 F-D 的 Editor consumer 全面统一。

迁移后的 owner 文件：

```text
rust/crates/project_authoring_execution/src/project_player_artifact.rs
rust/crates/project_authoring_execution/src/project_runtime_player_staging.rs
rust/crates/project_authoring_execution/src/desktop_export.rs
rust/crates/project_authoring_execution/src/project_write_scope.rs
rust/crates/project_authoring_execution/src/project_schema.rs
rust/crates/project_authoring_execution/tests/desktop_export_owner.rs
```

Editor 窄 Adapter：

```text
rust/crates/editor_core/src/project_player_artifact.rs
rust/crates/editor_core/src/project_runtime_player_staging.rs
rust/crates/editor_core/src/desktop_export.rs
rust/crates/editor_core/src/project_write_scope.rs
rust/crates/editor_core/src/project_runtime_package_assembler_adapter.rs
```

终态验证结果：

```text
cargo test -p project_authoring_execution --locked project_player_artifact_
  -> 3/3 passed
cargo test -p project_authoring_execution --locked project_runtime_player_production_staging_
  -> 4/4 passed
cargo test -p project_authoring_execution --locked project_write
  -> 5/5 passed
cargo test -p project_authoring_execution --locked project_editor_composition_staging_reuses_player_production_owner
  -> 1/1 passed
cargo test -p project_authoring_execution --test desktop_export_owner --locked desktop_export_owner_builds_windows_layout_and_verifies_player
  -> 1/1 passed
cargo test -p project_authoring_execution --test desktop_export_owner --locked desktop_export_project_relative_output_keeps_project_authority
  -> 1/1 passed
cargo test -p project_authoring_execution --test desktop_export_owner --locked desktop_export_project_relative_output_rejects_delivery_junction
  -> 1/1 passed
cargo test -p editor_core --lib --locked build_export_desktop_package_updates_report_model
  -> 1/1 passed
cargo test -p editor_core --lib --locked project_rust_release_reuses_existing_verified_player_candidate
  -> 1/1 passed
cargo test -p editor_core --lib --locked project_runtime_trust_tracks_source_manifest_dependency_and_editor_identity
  -> 1/1 passed
cargo check -p project_authoring_execution -p editor_core --tests --locked
  -> passed
cargo fmt --package project_authoring_execution --package editor_core -- --check
  -> passed
cargo tree -p project_authoring_execution --edges normal
  -> no editor_core / editor_ui_model / ai_tool_gateway dependency
```

Windows junction 测试最初三次未创建出 junction，首因是测试夹具把 `mklink` 参数错误拼接并使用正斜杠，
不是产品路径安全失败。夹具改为独立参数 `cmd /D /C mklink /J <link> <target>` 并将路径规范为反斜杠后，
同一 containment 测试通过。失败尝试留下的 10 个专用 `%TEMP%\aife-desktop-export-junction*` 目录已逐个确认
为 `%TEMP%` 严格子目录且不含 reparse point，并已全部删除；没有遗留测试进程或临时状态。

本 Gate 未运行 Local CI，未修改 production/安装态、真实 Codex 配置、Android 工具链或 Tower P1-2。
因此 F-C 记为 `completed`。在该历史检查点，309-F 整体仍在施工中且未生成阶段完成记录、未归档本文；
当时停在 F-D 前并等待新的明确指令。后续 F-D 至 F-F 的授权与完成证据见 19.18 至 19.20。

### 19.18 F-D Editor Adapter 回接完成证据（2026-09-01）

F-D 复核确认 Editor Preview、Build/Release、Android 与 Asset Import consumer 均已通过中立 Adapter 或
re-export 使用 `project_authoring_execution` 的唯一 owner，没有第二套 Assembler/Player Implementation 需要迁移。
新增 conformance test 证明 Editor Adapter 与中立 `GameProjectCompiler` 对同一 canonical snapshot 生成相同
`RuntimePackageBuildInput` 和 preparation identity；Editor 内存中的未保存 Scene draft 不改变 canonical bytes，
也不会渗入 Headless prepare。

```text
cargo test -p editor_core --locked editor_preview_package_
  -> 4/4 passed
cargo test -p editor_core --locked build_service_
  -> 10/10 passed
cargo test -p editor_core --locked editor_compiler_adapter_matches_neutral_owner_and_ignores_unsaved_draft -- --nocapture
  -> 1/1 passed
cargo check -p editor_core --locked
  -> passed
```

conformance test 首次带 `--exact` 时筛选到 0 tests，分类为测试筛选错误；移除错误筛选后同一测试真实执行
1/1 通过。F-D 未新增 Editor owner，记为 `completed`。

### 19.19 F-E Provider run/build/delivery 接入完成证据（2026-09-01）

F-E 在现有两个 owner crate 内完成，没有新增第三个 Compiler crate：

- `GameProjectCompiler::run/build/verify` 已消费 operation-bound prepared input 和同一 revision lineage；
- `DesktopExportPipeline::export_prepared` 直接消费 lease-owned `RuntimePackageBuildInput`，不重新读取 live source；
- `engine_project_run`、`engine_project_build`、`engine_delivery_verify` 已成为 Ready 具体工具，使用项目内
  `Library/EngineTools/Deliveries/<operationId>/...` 安全输出和 session-bound opaque `deliveryRef`；
- run 的 `timeoutMs` 已透传至底层 exported Player process verification，默认仍保持 30,000ms；
- 真实 owner test 复制完整 complex shooter 项目，在 Editor 未启动时依次完成 run、build、delivery verify，
  `Game.exe`、package manifest、RuntimePackage manifest 与两个 Player 退出码均通过。

```text
cargo check -p project_authoring_execution -p engine_tool_provider --tests --locked
  -> passed
cargo test -p engine_tool_provider --test headless_execution_owner --locked
  -> 1/1 passed
```

施工中先后修正了 Player build root 零参数调用、模块级 schema helper、集成测试 trait import 和 Windows 临时目录
canonical path 断言；这些均为编译/测试接线问题。修正后真实 run/build/verify 流程通过，F-E 记为 `completed`。

### 19.20 F-F No-Editor process/evidence 验收完成证据（2026-09-01）

F-F 将 MCP smoke 扩展为独立 stdio 子进程的交互式验收。测试复制 disposable complex shooter 项目，顺序执行
`initialize -> tools/list -> engine_project_run -> engine_project_build -> engine_delivery_verify -> shutdown`，并证明：

- 三个具体工具在 MCP tool list 中均为 Ready，工具名不含 Gateway；
- run/build/verify 绑定同一 project revision，run/build 使用同一 Player artifact identity；
- verify 消费 build 返回的不透明 `deliveryRef`，保持同一 delivery identity；
- process/child Player exit code 均为 0，所有 evidence reference 均指向真实文件；
- 测试正常 shutdown 子进程，异常路径由 guard kill/wait，并清理 disposable project。

```text
cargo test -p engine_tool_provider --test mcp_process_smoke --locked
  -> 1/1 passed
cargo test -p project_authoring_execution --locked
  -> 46 unit + 3 desktop owner + 6 compiler interface passed
cargo test -p engine_tool_provider --locked
  -> 17 unit + 5 integration passed
cargo test -p runtime_cli --locked
  -> 24 unit + 8 bounded process + 5 exported Player verification passed
cargo test -p engine_runtime --locked
  -> 568/568 passed
cargo tree -p project_authoring_execution --edges normal
  -> no editor_* / ai_tool_gateway dependency
cargo tree -p engine_tool_provider --edges normal
  -> no editor_* / ai_tool_gateway dependency
```

MCP smoke 首次编译因带 `Drop` 的夹具显式 move `ChildStdin` 失败；删除不必要的 move 后，`shutdown` 主动退出与
异常回收合同均通过。底层回归仅保留既有 `engine_runtime` 三项 dead-code warning，没有新增失败。

Gate F-D、F-E、F-F 已按用户要求顺序完成，Gate F 在该检查点完整通过；随后 Gate G 的完成证据见 19.21。

### 19.21 Gate G Editor Cutover 与 Gateway Retirement 完成证据（2026-09-02）

Gate G 已一次完成 Editor cutover 与旧 Gateway 生产退役：

- Editor composition 删除 Gateway field、wake/frame pump、approval coupling、host/discovery lifecycle 与 Editor instance identity；AI Panel 仅保留 Editor 自身验证 UI；
- `editor_host`、`editor_window_winit`、`project_e2e_gate` 与 workspace 已移除 `ai_tool_gateway` 依赖，旧 Gateway CLI/process tests、window tests 和整个 crate 已删除；
- unknown-task 与 build/delivery 资格已迁为 Provider tests；Editor 默认 composition 只保留 explicit empty runtime，项目专用 RuntimeModule 继续由 project-open preparation 接入；
- 不保留 feature flag、fallback、named pipe/discovery、双写或兼容 binary。

验证中先发现六个异步 Editor 测试依赖旧 frame pump 的隐式耗时，随后确认唯一 Assembler 冷路径约 8 秒；测试轮询改为有界 deadline，并让三个 trust tests 使用各自 disposable ProjectRust fixture，避免并行占用同一 project authority。`editor_host` 另有一条静态链接 complex-shooter 的过期测试，已迁为默认 composition 对项目专用 runtime fail-closed 合同。没有为测试恢复 Gateway 或项目专用静态 runtime。

```text
cargo test -p editor_core --locked
  -> 646 passed, 2 ignored；2 bin tests passed
cargo test -p editor_window_winit --locked
  -> 184 passed, 5 ignored
cargo test -p editor_host --locked
  -> 17 unit + 1 CLI passed
cargo test -p project_e2e_gate --locked provider_unknown_task_preflight_keeps_large_competition_margin -- --nocapture
  -> 1/1 passed
cargo test -p project_e2e_gate --locked provider_build_delivery_exports_once_then_verifies_existing_package -- --nocapture
  -> 1/1 passed
cargo metadata --no-deps --format-version 1
  -> 27 packages；ai_tool_gateway package count = 0
cargo tree -p engine_tool_provider --edges normal
cargo tree -p project_authoring_execution --edges normal
  -> both have 0 editor_* / ai_tool_gateway dependency matches
cargo fmt --all -- --check
  -> passed
rg -n "ai_tool_gateway|GatewayCore|GatewayRemoteAdapter|EditorGatewayHost|read_generation|observed_project_digest|connect_editor|discover_editor" rust/Cargo.toml rust/crates README.md
  -> only README.md:40 and README.md:56 historical ai_tool_gateway records
```

Gate G 完成后 309-F 仍在施工中，不生成阶段完成记录、不归档本文。当前严格停在 Gate H 前；未运行 Local CI，未修改 production/安装态、真实 Codex 配置、Android 工具链或 Tower P1-2，Gate H/I 必须取得新的明确授权后才能施工。

### 19.22 Gate H 13.1 Source/Integration Completion 完成证据（2026-09-02）

本 Gate 只验收 F1-F10 与 F12。Gate G 后产品输入未变化的 Editor/Gateway 终态证据继续有效，未机械重跑
`editor_core`、`editor_window_winit`、`editor_host` 与两条 `project_e2e_gate`；本轮补跑 owner/runtime
受影响闭包，并只为尚未直接证明的 MCP process、F7 permission matrix 和 F8 mutation equivalence 增加窄测试。

| 资格 | 终态证据 | 结果 |
| --- | --- | --- |
| F1 Neutral Dependency | metadata 27 packages、`ai_tool_gateway=0`；Provider/Execution 两棵 normal dependency tree 均无 `editor_*`/Gateway | `source_integration_passed` |
| F2 No-Editor Open | 独立 MCP 子进程完成 explicit open、clean recovery、inspect、lease-backed object read、外部文件 refresh；既有同进程 run/build/verify 保持 | `source_integration_passed` |
| F3 Concrete Projection | Provider/MCP tests 证明 tools/list 仅具体 `engine_*`，无 catalog/execute/context/editor/gateway，schema 独立 | `source_integration_passed` |
| F4 File Refresh | Provider valid external rewrite 与 invalid manifest 两条测试通过；MCP 子进程再次证明外部 rewrite 后 revision 和 snapshot bytes 更新 | `source_integration_passed` |
| F5 Mutation/Recovery | `authoring_project_context` 45 项覆盖双 Context CAS、跨进程 lock、全部 crash checkpoint、unknown write、sealed receipt/tamper 与 exact rollback | `source_integration_passed` |
| F6 Run/Build/Delivery | 中立 Compiler/Assembler/Player/Desktop owner suites、Provider headless owner、MCP process、`runtime_cli` 与 `engine_runtime` 全部通过 | `source_integration_passed` |
| F7 Approval/Grant | 新增汇总测试覆盖 read/write/process permission、risk、capability、operation-bound Grant，以及 host rejected/engine rejected/unsupported/cancelled 四类终态 | `source_integration_passed` |
| F8 Native/MCP Equivalence | read 全结果等价；新增 mutation 测试在同项目 revision/input 上证明 canonical result、receipt/evidence identity 和最终文件 bytes 完全相同 | `source_integration_passed` |
| F9 Optional Editor | 复用 Gate G 646+2、184、17+1 Editor suites；结合本轮 LWW/CAS owner 回归，证明 draft 隔离、clean/LWW/CAS 与无 Gateway Editor composition | `source_integration_passed` |
| F10 Gateway Retirement | workspace 无 Gateway package；生产/Cargo/crate 扫描只命中根 README 两条明确历史记录 | `source_integration_passed` |
| F12 Regression/Performance | 309-A-E owner 回归、Kernel/MCP limit/clean inspect/warm-cache 非 authority、Runtime default report contracts通过；Runtime/CLI 热路径扫描无 Provider/Context authority | `source_integration_passed` |

执行结果：

```text
cargo test -p authoring_project_context --locked
  -> 45 passed
cargo test -p project_authoring_execution --locked
  -> 46 unit + 3 desktop export owner + 6 compiler interface passed
cargo test -p engine_tool_provider --locked
  -> 18 unit + 1 headless owner + 3 Native/MCP equivalence + 1 isolated config + 1 MCP process passed
cargo test -p runtime_cli --locked
  -> 24 unit + 8 bounded process + 5 exported Player verification passed
cargo test -p engine_runtime --locked
  -> 568 passed
cargo fmt --all -- --check
  -> passed
cargo metadata --no-deps --format-version 1
  -> 27 packages；ai_tool_gateway=0
cargo tree -p engine_tool_provider --edges normal
cargo tree -p project_authoring_execution --edges normal
  -> each has 0 editor_* / ai_tool_gateway dependency hits
Gateway retirement static scan
  -> only README.md:40 and README.md:56 historical records
Runtime hot-path authority scan
  -> engine_runtime/runtime_cli Provider/AuthoringProjectContext hits = 0
```

本 Gate 未运行 Local CI，未修改 production/安装态、真实 Codex 配置、Android 工具链或 Tower P1-2。
F11 当时保持 `blocked_pending_user_authorization`；后续唯一真实任务结果见 19.23。309-F 不生成阶段完成记录、不归档，
不得进入 Gate I。

### 19.23 F11 Real Codex Tool-call Acceptance 终止证据（2026-09-02）

用户已授权事务化修改真实 Codex MCP 配置和安装态 Provider，并要求只执行一次真实 Agent Tool Loop。
本次 run 为 `20260902T060735Z`，使用 release Provider
`6EFCA81A...A2FA1`、disposable complex shooter project 和全新 Codex 任务
`01a060bd-d76b-7382-8eab-c4ffcc9ed483`。

新任务在 2,297 ms 后进入 `systemError`，首个失败为：

```text
function_call_output requires call_id on HTTP requests; continuation via previous_response_id
is only supported on Responses WebSocket v2
```

失败发生在任何 Assistant message、Engine tool marker 或 Engine tool output 之前。因此
`engine_project_inspect -> engine_project_mutate -> engine_project_run -> engine_project_build ->
engine_delivery_verify` 五步均为 `not_started`，其 call id、revision、receipt 和 evidence 均为
`null`；没有重试，不能据此给出 Provider 产品通过或失败判定。

失败后终止了任务遗留的、身份匹配本次候选的 Provider PID `28524`。真实 config SHA-256 已从临时
`764B633E...58ADF` 字节级恢复为 `F13FCE5F...C229D`；临时安装态 Provider 已移除；旧 Gateway
二进制保持 `1BB80D88...0538A`；Provider/Editor 进程均为 0。disposable project 未出现 acceptance
mutation、Build 或 Library，仅保留 Provider 启动生成的 authority lock 作为证据。

完整机器可读 receipt 和说明：

```text
G:\gameEngin\.artifacts\309-f-f11\20260902T060735Z\f11-real-codex-acceptance-receipt.json
G:\gameEngin\.artifacts\309-f-f11\20260902T060735Z\README.md
```

F11 记为 `failed_before_engine_tool_call`。309-F 保持当前施工、不归档；Gate I、Local CI、Android
工具链与 Tower P1-2 均未进入。任何 F11 重试都需要新的明确用户授权。

### 19.24 F11-R1 Codex Host Fresh-task Preflight 证据（2026-09-02）

用户授权执行零工具宿主预检：不修改 MCP 配置、不安装 Provider、不调用 Engine tool，只创建一个全新
projectless Codex 任务并要求精确返回 `HOST_READY`。task
`01a060e0-4e47-7482-8070-74831c442b83`、turn
`01a060e0-4f50-7293-b7d4-b379a5641746` 在 1,447 ms 后于 Assistant 输出前失败，错误与 F11 完全相同：

```text
function_call_output requires call_id on HTTP requests; continuation via previous_response_id
is only supported on Responses WebSocket v2
```

线程没有 Assistant message、tool marker 或 tool call。这证明当前阻塞位于 Codex fresh-task continuation，
与 Headless Provider、Engine tool schema、MCP 临时配置或 disposable project 无关；F11 产品判定仍为
`not_obtained`。

Codex App 创建 projectless 任务时自动追加了该临时目录的 trusted entry；这是本轮唯一配置差异，已精确
移除。config SHA-256 最终恢复为预检前 `F13FCE5F...C229D`；旧 Gateway 身份未变，Headless Provider
不存在，Engine Provider/Editor 进程均为 0。

证据：

```text
G:\gameEngin\.artifacts\309-f-f11\20260902T060735Z\F11-R1-Codex-Host-Fresh-task-Preflight.md
G:\gameEngin\.artifacts\309-f-f11\20260902T060735Z\f11-r1-codex-host-preflight-receipt.json
```

F11-R1 终态为 `blocked_codex_host_continuation`。不自动重试 F11，不进入 Gate I。

### 19.25 F11-R2 Updated Codex Host Fresh-task Preflight 证据（2026-09-02）

用户更新 Codex 后授权再次执行零工具 fresh-task 预检。本轮身份为 Codex App
`26.831.2377.0`、CLI `0.152.1`。全新 task
`01a060eb-8ddd-7c20-bd7c-5c2c3b5123e0`、turn
`01a060eb-9006-7ea2-8c0c-d8ba5a44ae71` 只要求返回 `HOST_READY`，仍在 1,419 ms
后、Assistant 输出和 tool call 之前复现相同续传协议错误。

这证明该 Codex 更新没有修复当前 fresh-task continuation 路径；没有调用 Engine tool，没有重试完整
F11，Provider 产品判定仍为 `not_obtained`。

Codex App 自动追加的 projectless trust entry 已精确移除。config 最终恢复为更新后预检前 SHA-256
`4B5761FF...D1EB9`；没有用 F11-R1 的旧 config 覆盖更新。旧 Gateway 身份未变，Headless Provider
未安装，Engine Provider/Editor 进程均为 0。

证据：

```text
G:\gameEngin\.artifacts\309-f-f11\20260902T060735Z\F11-R2-Updated-Codex-Host-Fresh-task-Preflight.md
G:\gameEngin\.artifacts\309-f-f11\20260902T060735Z\f11-r2-updated-codex-host-preflight-receipt.json
```

F11-R2 终态为 `blocked_codex_host_continuation_after_update`。不自动重试完整 F11，不进入 Gate I。

### 19.26 F11-R3 Manual-UI 准备与人工交接证据（2026-09-02）

用户通过 Codex UI 手动创建全新 projectless 任务，并取得精确 `HOST_READY`。该截图已封存，证明人工
任务入口可用；它不反向证明自动 `create_thread` continuation 已修复。用户随后单独授权 F11-R3：允许
事务化修改真实 Codex MCP 配置和安装态 Provider，准备 disposable project，但由用户本人执行唯一真实
Agent Tool Loop。

本轮 run 为 `20260902T073329Z-manual-ui`。准备阶段完成：

```text
config before SHA-256    BAFE0536D4415B0A84F0D819D49A86DDBD21D8828A77D61167D683962B249A64
config active SHA-256    3CCCB05EDDA82F01EB4B246EDD6A800741CC9882E4EF385F2EC31D10E4238FE0
Provider SHA-256         6EFCA81A2734961A303A7ED4ED304CBACF19CF6C3ED2E21E5EE38180325A2FA1
legacy Gateway SHA-256   1BB80D88ECE1320E1D6857DF742C25DA0F167C4F372F17FE2AFCFA667930538A
disposable source digest 9E06E5A017295E49F8BFBA3807A1948DB0B385DA52DCA472CEDC13987E7B78AB
```

配置通过 Codex CLI `mcp remove/add` 结构化切换。资格预检只执行 MCP `initialize`、`tools/list` 和
`shutdown`，没有执行 `tools/call`；20 个 Tool 均以 `engine_*` 暴露，F11 必需的 inspect、mutate、run、
build 与 delivery verify 全部存在。Provider 正常 exit 0，Provider/Editor 进程均为 0。预检产生的唯一
run-owned `authority.lock` 已记录内容与哈希后清除，人工任务将从无 `.aife` 的干净 disposable project 启动。

证据与固定提示词：

```text
G:\gameEngin\.artifacts\309-f-f11\20260902T073329Z-manual-ui\README.md
G:\gameEngin\.artifacts\309-f-f11\20260902T073329Z-manual-ui\F11-R3-Manual-UI-Preparation-Receipt.json
G:\gameEngin\.artifacts\309-f-f11\20260902T073329Z-manual-ui\MANUAL-TEST-PROMPT.md
G:\gameEngin\.artifacts\309-f-f11\20260902T073329Z-manual-ui\manual-host-ready.png
```

当前终态为 `prepared_awaiting_manual_task`，不是 F11 passed。下一步只能由用户手动创建全新 projectless
任务并发送固定提示词；任一失败立即停止、不重试，随后收集 task/turn/call/revision/receipt/evidence 并
按 `config.toml.before` 字节级恢复配置、移除本轮临时 Provider、确认旧 Gateway 身份与进程归零。F11
通过并完成恢复前不得进入 Gate I。

### 19.27 F11-R3 Real Codex Manual Agent Tool Loop 终态证据（2026-09-02）

用户在 Codex UI 手动创建了全新 projectless 任务。Codex App 任务列表与 turn 记录确认：

```text
task  01a06125-de64-7cd3-9b73-c1b719f966b6
turn  01a06125-e37f-7960-b976-e907dec8cf16
cwd   C:\Users\zenghaoran\Documents\Codex\2026-09-02\f11-r3-agent-tool-loop-editor
duration 142566 ms
```

该 turn 没有调用 shell、普通文件工具或 Editor，只按固定提示词各调用一次五个具体 Engine tools：

| 阶段 | MCP call | Codex Tool item | operation | 结果 |
| --- | --- | --- | --- | --- |
| inspect | `mcp-call-2` | `exec-daaf4004-4ff8-48c7-aa5e-0842b76ef483` | `...-1` | completed；initial revision `e7dcd194...385a1` |
| mutate | `mcp-call-3` | `exec-913839ec-4be8-4a05-9380-0d9e38774cb8` | `...-2` | completed；accepted revision `a1a23f3...141cd` |
| run/check | `mcp-call-4` | `exec-fb36c56b-5dcd-43ad-88c5-0dd73e34a96d` | `...-3` | completed；exit 0 |
| build | `mcp-call-5` | `exec-112112dc-a205-4454-affd-1beef004d469` | `...-4` | completed；Windows dev delivery |
| delivery verify | `mcp-call-6` | `exec-b069faa1-b510-4970-9d29-d5f5c4a84b0e` | `...-5` | completed；passed；process/child exit 0 |

mutation receipt `receipt-c9724...ca2f` 的 declared/actual/changed path 均只含
`Data/f11-manual-acceptance.json`，before/after revision 与 Codex 输出一致，`rollbackAvailable=true`。
Run 与 Build package manifest 的 Player artifact hash 均为
`sha256:dafe48d45471388c0009e3fb6c7a4bca95d16b2837cbaa453ecbefd478bf7d2f`；build 与 verify 的
delivery identity 均为 `sha256:9113376b8492962939d5d94991d26aaa8d32d15698def26375e8ef1509492600`。
真实 exported player verification report 为 `passed`，`processExitCode=0`、`childPlayerExitCode=0`、
`childFramesCompleted=1`，无 diagnostics。

`engine_delivery_verify` 的逻辑 `evidenceRefs` 把同一个
`Build/Windows/dev/reports/windowed-player-run-report.json` 返回了两次；物理文件只有一份且哈希、内容均通过。
该重复引用不影响 revision、artifact、delivery 或进程验收结论，记为后续非阻塞的结果去重质量项，不在 F11
窗口扩大施工。

终态证据：

```text
G:\gameEngin\.artifacts\309-f-f11\20260902T073329Z-manual-ui\F11-MANUAL-RESULT.json
G:\gameEngin\.artifacts\309-f-f11\20260902T073329Z-manual-ui\F11-R3-Manual-UI-Acceptance-Receipt.json
G:\gameEngin\.artifacts\309-f-f11\20260902T073329Z-manual-ui\disposable-project\.aife\authoring\receipts\receipt-c9724f48bf594a2989e15bff4bb9bf9f2d55134313eff5910910dff0ceb7ca2f.json
G:\gameEngin\.artifacts\309-f-f11\20260902T073329Z-manual-ui\disposable-project\Library\EngineTools\Deliveries\engine-op-f11-r3-manual-ui-20260902T073329Z-4\Build\Windows\dev\reports\exported-player-process-verification-report.json
```

验收后外部状态已事务化恢复：真实 config 从 manual task 后的 `CB6D61A6...A947BF` 字节级恢复为
pre-F11-R3 `BAFE0536...49A64`；临时 Provider `6EFCA81A...A2FA1` 已移除；两个 session-owned Provider
进程 `23148`、`12248` 已终止；旧 Gateway 保持 `1BB80D88...0538A`；Provider/Editor 进程数为 0。

F11 终态为 `passed_real_codex_manual_agent_tool_loop`。因此 F1-F12 已全部通过。在该检查点 Gate I、Local CI、
Android 工具链和 Tower P1-2 均未进入；随后 Gate I 的明确授权与完成见 19.28。

### 19.28 Gate I 终态文档、完成记录与归档（2026-09-02）

用户已明确授权完整 Gate I。本 Gate 只消费 Gate A-H、F1-F12 与 F11-R3 已有有效证据，没有修改产品源码、
测试、F11 disposable project 或外部状态，也没有重跑任何测试。

已完成：

- 生成 `阶段完成记录/2026-09-02-309-F-Headless-Engine-Tool-Provider-Atomic-Cutover-v1/00-总览.md`；
- 同步正式 309、309-F、根 README、文档地图、阅读顺序、49、54、施工文档 README 与阶段完成记录 README；
- 将本文从 `施工文档/当前/` 移入 `施工文档/已完成/`；
- 确认全局当前施工槽为空、待执行队列为空，Tower P1-2 继续保持 `stopped_by_user`；
- 通过 JSON parse、309-F 过期状态扫描和 scoped `git diff --check` 完成文档一致性自审。

Gate I 终态为 `completed_and_archived`。未运行 Local CI 或任何源码测试；未修改真实 Codex 配置、production、
安装态 Provider、Android 工具链或 Tower P1-2；F11 证据完整保留。309-F 不再占用施工槽，也不自动启动下一系统。
