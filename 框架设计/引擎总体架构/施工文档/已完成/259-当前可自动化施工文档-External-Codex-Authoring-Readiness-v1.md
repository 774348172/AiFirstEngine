# 259 当前可自动化施工文档：External Codex Authoring Readiness v1

## 0. 文档状态

```text
正式方案：259-External-Codex-Authoring-Readiness-Connection-Recovery-Deep-Mutation-Contract-v1方案.md
方案选择：B-min
文档版本：v1
文档状态：已完成
当前位置：施工文档/已完成/
施工状态：C0-C7 与 C7-R1/R2 全部完成
可否施工：否；只作完成证据
当前授权：无
生成时 source HEAD：0e4d8d8d89144f4d7ae4bdb9ac7cf8d7032d038f
纯文档基线 commit：d595b0a4da419723554cf087c8a5d01830dbb727
Rust/Cargo：1.96.0
```

本文已完成第 17 节激活前复核、C0-C7、C7-R1 与 C7-R2 全部施工，并已移动到
`施工文档/已完成/`。C7-R1 关闭 Named Pipe 多客户端缺口；C7-R2 关闭 clean Scene save
无效写入，并在 fresh Codex task 上完成唯一 mutation、Native approval、receipt 与 exact
rollback。原 C7 terminal mutation 保留为不可重试失败证据。

归档后仍不授权：

```text
再次迁移 stable config 或 rollback C6（除非单独授权诊断/恢复）
重试原 C7 terminal mutation
复用原 project / approval / grant
运行 production typed MCP smoke
运行 Local CI
```

## 1. 施工目标

本施工只完成 259 B-min：

1. 将 MCP command 收敛到 installation-owned 稳定 executable。
2. 提供只替换 `mcp_servers.ai_first_game_engine` 的一次性事务迁移。
3. discovery 对安全可判定的 dead stale legacy record 容错。
4. discovery 对多个 live Editor、incompatible live Editor 和安全违规 fail closed。
5. 用 `project.mutate` 替换 caller-owned Candidate public Interface。
6. caller 只提交 goal outcome 与真实 change payload。
7. Gateway / Tool Kernel 内部绑定 Editor、session、project、digest、generation、
   risk、Grant、Candidate、operation 和 receipt facts。
8. 一个 mutation operation 内完成 `awaiting_user -> approve once -> apply -> terminal`。
9. 相同 goal/session/project/risk/budget 可复用 bounded goal-level Grant。
10. 用 `project.rollback(rollbackRef)` 替换完整 receipt rollback 输入。
11. 退役旧 `project.mutate.candidate` / `project.rollback.candidate` public entries，
    不保留永久 alias。
12. 用受影响域回归、production process smoke 和一次新 Codex task typed MCP smoke
    证明真实 composition。

## 2. 明确不施工

```text
不实现内置 AI
不新增 Planner、Workflow、Runner
不新增 public request_access / resume / continue / apply_candidate / approval_status
不新增 bootstrap forwarding process
不新增 discovery quarantine、database 或 GC Module
不新增 persistent operation journal、cross-task replay 或 MCP heartbeat
不接受 caller-owned goalId、riskIntent、project digest、generation 或 Grant identity
不把 Candidate prepare / validate / apply 暴露成 typed tools
不恢复 254-R1/R2、F-A/F-B/G、Real Evaluation、candidate freezer 或三引擎 B 通道
不修改真实用户项目来完成 qualification
不把 Preview、diagnostics、Build、Delivery 串成 259 固定验收菜谱
不默认运行完整 workspace Local CI
```

若实现发现必须扩大到以上任一项，立即停止当前阶段，分类为方案或施工文档问题；
先修订正式方案与本文，不得先写兼容层。

## 3. 当前实现基线

本节是生成时快照，激活前必须重新核对。

### 3.1 已存在且必须复用

```text
rust/crates/ai_tool_gateway/src/codex_config.rs
  已有 TOML structured edit、bounded read、backup、atomic replace、
  exact expected command replace、receipt 与 rollback

rust/crates/ai_tool_gateway/src/bin/ai_engine_gateway_codex_config.rs
  已有 install / replace / rollback CLI

rust/crates/ai_tool_gateway/src/discovery.rs
  已有 discovery v2、64 records / 64 KiB limits、explicit path、
  editorInstanceId selection 与 ambiguous diagnostic

rust/crates/ai_tool_gateway/src/core.rs
  已有 access request、approval inbox、goal/risk Grant constructor、
  active Grant 与 operation observation

rust/crates/editor_window_winit/src/application.rs
  已有 Native Editor authorization Inbox、Approve/Reject UI dispatch、
  request/receipt model 与 production Gateway host

rust/crates/editor_core/src/ai_capability_tool_kernel.rs
  已有 Candidate validation/apply、receipt lineage、rollback、
  GoalBinding、RiskEnvelope 与 mutation budget

rust/crates/ai_tool_gateway/src/mcp_stdio.rs
  已有 catalog-driven typed MCP projection、status/catalog/observe/cancel

rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs
  已有真实 ai_engine_gateway_mcp process 测试基础
```

### 3.2 已确认缺口

```text
Codex real config 仍指向历史 frozen candidate MCP binary
没有 installation-owned stable executable producer/transaction receipt
discovery v2 遇到 dead legacy v1 record 会整体失败
project.mutate.candidate public schema 暴露内部 Candidate facts
project.rollback.candidate 要求 caller 搬运完整 receipt
Gateway access request 尚未由 project.mutate 自动绑定同一 operation
approval 后没有同一 operation 自动继续内部 mutation
rollbackRef 尚不存在
旧 Candidate public entries 仍在 Catalog 与 MCP projection 中
```

### 3.3 当前外部状态

生成本文时记录但不冻结：

```text
Codex config：
C:\Users\zenghaoran\.codex\config.toml

expected historical command：
I:\EngineTest\GateF254R1Frozen\candidate-20260720-000200\bin\ai_engine_gateway_mcp.exe

target stable command：
%LOCALAPPDATA%\AiFirstGameEngine\bin\ai_engine_gateway_mcp.exe
```

真实 config、running Editor、discovery records、binary hashes 与 PID 都必须在外部
激活窗口重新读取；本文中的历史值不能作为写入授权。

## 4. 允许涉及文件

### 4.1 核心允许范围

```text
rust/crates/ai_tool_gateway/src/codex_config.rs
rust/crates/ai_tool_gateway/src/discovery.rs
rust/crates/ai_tool_gateway/src/core.rs
rust/crates/ai_tool_gateway/src/protocol.rs
rust/crates/ai_tool_gateway/src/mcp_stdio.rs
rust/crates/ai_tool_gateway/src/remote_adapter.rs
rust/crates/ai_tool_gateway/src/editor_host.rs
rust/crates/ai_tool_gateway/src/lib.rs
rust/crates/ai_tool_gateway/src/bin/ai_engine_gateway_codex_config.rs
rust/crates/ai_tool_gateway/src/bin/ai_engine_gateway_mcp.rs
rust/crates/ai_tool_gateway/tests/stable_install_contract.rs
rust/crates/ai_tool_gateway/tests/discovery_recovery.rs
rust/crates/ai_tool_gateway/tests/deep_mutation_contract.rs
rust/crates/ai_tool_gateway/tests/rollback_contract.rs
rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs

rust/crates/editor_core/src/ai_capability_tool_kernel.rs
rust/crates/editor_core/src/ai_tool_catalog.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/editor_core/src/tests/goal_mutation_contract.rs

rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/tests/mod.rs
rust/crates/editor_window_winit/src/tests/gateway_goal_mutation.rs
```

允许在 `ai_tool_gateway/src/` 或 `editor_core/src/` 中新增一个局部文件承载内部
`GoalMutationModule` 或 stable installation Implementation。它们不得增加 AI-facing
生命周期 Interface。

### 4.2 条件允许范围

只有 red-capable test 或 consumer inventory 证明直接受影响时才允许：

```text
rust/crates/project_e2e_gate/src/**
rust/crates/editor_ui_model/src/**
rust/crates/editor_ui_renderer/src/**
rust/crates/editor_host/src/**
rust/Cargo.toml
rust/Cargo.lock
rust/quality/**
```

`Cargo.toml`、`Cargo.lock`、quality ledger 与 Local CI runner 默认不改。若需要新
dependency、lint ledger 或 Runner 变化，停止当前阶段，先单独审查；不得顺手同步。

### 4.3 禁止范围

```text
legacy/rust/254-r2-lifecycle/**
施工文档/历史/**
历史 candidate / freezer / activation source
samples/** 与真实用户项目
其它 MCP server config
Codex model、approval 或 plugin config
```

## 5. 深 Module 与 Interface 施工合同

### 5.1 `EditorInstanceGatewayModule`

继续拥有：

```text
stable MCP Adapter entry
discovery classify/select/connect
session/project context
operation observation/cancellation
Native Editor authorization bridge
disconnect terminalization
```

不得增加第二个 public Gateway lifecycle。

### 5.2 内部 `GoalMutationModule`

对 caller 的公共面只有：

```text
project.mutate(intent)
project.rollback(rollbackRef)
```

Implementation 内部拥有：

```text
goal normalization
current facts capture and recheck
engine-derived risk
bounded Grant selection/reuse
Candidate construction/validation/apply
receipt lineage
rollbackRef store/resolve
terminal diagnostics
```

测试跨公共 Interface 观察结果，不测试私有 stage ordering。旧浅层 Candidate public
tests 在新 Interface 覆盖后删除或迁移，不叠加维护两套长期测试面。

## 6. 验证与三小时边界

### 6.1 证据分级

| 声明 | 最小证据 | 不自动包含 |
|---|---|---|
| owner implementation | 新增 red-capable contract test | 完整 crate、production smoke |
| shared Interface integration | 实际受影响 crate/consumer closure | 全 workspace、Local CI |
| installed MCP composition | release binary、transaction receipt、process smoke | 真实项目 mutation |
| real Codex activation | 新 task 的 typed MCP status/catalog + disposable project smoke | Preview/Build/F-A |

### 6.2 去重规则

1. 每阶段先跑该阶段的 owner contract；失败先在 owner 内收敛。
2. C5 完整 crate suite 已覆盖的 filtered test，不在 C5 前机械重跑。
3. C5 通过后，若产品源码不再变化，其证据可用于 C6/C7，不重新跑。
4. production process smoke 与 real Codex smoke 证明不同 composition，因此各一次。
5. Local CI 不是 259 默认完成条件。只有 shared dependency、quality contract 或
   release policy 的实际变化无法由 affected closure 覆盖时，才修订本文并单独授权。

### 6.3 时间窗口

所有窗口包括实现、测试、证据和 cleanup，连续授权不超过 3 小时：

```text
窗口 A：C0 stable install/config + C1 discovery
窗口 B：C2 deep project.mutate
窗口 C：C3 same-operation approval + Grant reuse
窗口 D：C4 rollbackRef + old public contract retirement
窗口 E：C5 affected integration + release process smoke
窗口 F：C6 real stable install/config migration；成功后强制停止
窗口 G：C7 new Codex task typed MCP smoke + completion archive
```

窗口可提前停止，不得跨窗口自动继承授权。2:45 后不开始新实现或验证；3:00 标记
`stopped_by_time_limit`，保存首因与 cleanup 后等待新授权。

这些窗口只治理施工，不是 AI 使用引擎的工具菜谱。

## 7. C0：Stable MCP installation 与 config transaction

### 7.1 红合同

新增 `stable_install_contract.rs`，先证明当前缺口：

```text
stable target 固定为 current-user LOCALAPPDATA 下的 installation path
source binary 必须 existing regular absolute file
target parent、staging、backup 与 receipt owner 明确
install 使用 same-parent atomic replace
成功 receipt 绑定 source/installed SHA-256、before/after 与 target
失败恢复 prior installed bytes，不留下 partial executable
config replace 只接受 exact expected current command
config 其它 table/key 原样保留
config receipt persist 失败时事务回滚
reloadOrNewTaskRequired=true
symlink/reparse/oversize/config drift fail closed
```

### 7.2 实现

深化现有 `codex_config` ownership：

```text
stable binary install
config exact replace
combined durable migration receipt
binary/config rollback
typed diagnostics
```

`ai_engine_gateway_codex_config` 可增加一个 repository-owned
`migrate-stable` command，但它只是安装维护 CLI，不是 AI typed tool，也不启动
bootstrap child。

### 7.3 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p ai_tool_gateway --test stable_install_contract
cargo test -p ai_tool_gateway codex_config
```

C0 只使用 isolated fake `LOCALAPPDATA` 与 fake `CODEX_HOME`，禁止读取或修改真实
Codex config。

## 8. C1：Tolerant secure discovery recovery

### 8.1 红合同

新增 `discovery_recovery.rs`，矩阵至少覆盖：

```text
one live compatible + dead regular owned v1 => select live compatible
one live compatible + dead regular owned v2 => select live compatible
two live compatible => ambiguous
only live incompatible => incompatible diagnostic
dead PID cannot excuse symlink/reparse/oversize/non-owned record
explicit discovery path remains strict
record count/size remains bounded
best-effort stale cleanup failure does not hide the live compatible record
```

测试必须用可控 process-liveness Adapter；不得依赖测试机偶然 PID。

### 8.2 实现

在现有 discovery owner 内部分类：

```text
active_compatible
active_incompatible
dead_stale
invalid_or_security_violation
```

分类是内部 Implementation，不扩展 MCP public schema。安全违规始终 fail closed；
只有 engine-owned regular record + confirmed dead PID 才可 best-effort 删除。

### 8.3 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p ai_tool_gateway --test discovery_recovery
```

## 9. C2：Deep `project.mutate` contract

### 9.1 caller schema

新增并冻结：

```text
external-project-mutation-intent.v1
goal.outcome
change.kind
change.payload
```

`deny_unknown_fields` 必须拒绝 caller 补写：

```text
goalId
riskIntent
projectRoot
expectedProjectDigest
readGeneration
projectPatchContextHash
candidateId
revisionId
grantId
operationId
receiptId
```

### 9.2 internal fact binding

`GoalMutationModule` 在调用时从同一 Gateway session 捕获 project facts，派生 risk，
构建内部 Candidate。apply 前必须重新读取并比较 facts。drift 返回 terminal typed
diagnostic，不自动重建 Candidate 或重试。

`change.payload` 必须经过现有 capability-specific decoder，不能成为任意文件写入口。

### 9.3 public replacement start

Catalog/Registry 增加 `project.mutate`，但 C2 尚不移除旧 entries；在 C4 一次性完成
cutover。C2 的临时双入口只存在于未发布施工 commit 内，不得安装或对外激活。

### 9.4 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core goal_mutation_contract
cargo test -p ai_tool_gateway --test deep_mutation_contract caller_schema
```

测试必须断言实际执行数量非零，并覆盖成功事实绑定、unknown internal field 拒绝、
no project、unsupported change 与 project drift。

## 10. C3：Same-operation approval 与 bounded Grant reuse

### 10.1 operation

`project.mutate` 首次调用创建一个 in-memory session-bound operation：

```text
running -> awaiting_user -> running -> succeeded
running -> awaiting_user -> failed/cancelled
```

无 active eligible Grant 时，同一 operation 生成 Native Editor access request。
用户批准后 Gateway owner 自动继续该 operation；caller 不调用 resume/continue。

### 10.2 Native Editor

复用现有 authorization Inbox 和 Approve/Reject command。UI 必须显示：

```text
normalized goal outcome
project identity
derived risk
mutation/time budget
operation short identity
TTL
```

不得增加第二个 approval panel 或独立 AI workflow。

### 10.3 Grant reuse

仅当以下全部相同时复用：

```text
normalized goal outcome
Editor instance
MCP client session
project identity
risk class
remaining approved budget
unexpired/unrevoked Grant
```

goal、project、risk、TTL、budget、disconnect 任一变化都不复用。reject、expiry、
drift、cancel、disconnect 均 terminal，不自动重试。

### 10.4 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p ai_tool_gateway --test deep_mutation_contract same_operation
cargo test -p editor_window_winit gateway_goal_mutation
```

至少覆盖 approve once、reject、TTL、drift、cancel、disconnect、eligible reuse 和
每种 non-reuse。UI test 通过现有 command Interface 驱动，不直接调用私有 continuation。

## 11. C4：`rollbackRef` 与 public cutover

### 11.1 rollbackRef

成功 mutation 返回 session/project-bound opaque `rollbackRef`。内部映射拥有完整
receipt lineage 和 rollback material；caller 不能解码或拼装。

`project.rollback` 只接受：

```json
{
  "schemaVersion": "external-project-rollback.v1",
  "rollbackRef": "rbk_..."
}
```

以下情况 fail closed：

```text
unknown/expired ref
different session/editor/project
current digest/read generation drift
receipt or rollback material mismatch
already consumed ref（按冻结的一次性语义返回稳定 diagnostic）
```

### 11.2 public cutover

一次性从 Registry、Catalog、MCP projection、docs/tests consumer 移除：

```text
project.mutate.candidate
project.rollback.candidate
```

不得保留 alias。内部 Candidate 类型可以继续存在，但不再作为 caller direct schema。

### 11.3 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p ai_tool_gateway --test rollback_contract
cargo test -p editor_core goal_mutation_contract
rg -n 'project\.mutate\.candidate|project\.rollback\.candidate' crates --glob '*.rs'
```

最后一条必须只命中明确的 migration/negative assertion；若 production registry、
projection 或 caller consumer 仍命中，C4 不通过。

## 12. C5：Affected integration 与 release process smoke

### 12.1 进入条件

```text
C0-C4 owner tests 全部通过
旧 public entries 已 cut over
工作树只包含 259 scope
没有真实 config 或 user project 变化
没有运行中的 test-owned descendants
```

### 12.2 受影响域

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --check
cargo test -p editor_core
cargo test -p ai_tool_gateway
cargo test -p editor_window_winit
cargo test -p project_e2e_gate
```

若 consumer inventory 证明 `project_e2e_gate` 未引用任何变更后的 Interface，可在执行
记录中明确豁免；不得默认为了缩短时间静默跳过。

### 12.3 production process smoke

在 repository-external isolated root 中启动实际
`CARGO_BIN_EXE_ai_engine_gateway_mcp`，验证：

```text
mixed stale discovery -> unique active Editor host
tools/list 只公开 project.mutate / project.rollback
project.mutate -> awaiting_user -> approval -> same operation success
receipt/digest/generation/rollbackRef
project.rollback(rollbackRef) -> initial digest restored
MCP close/disconnect 无继续 apply
```

fixture 必须是 run-owned disposable project copy；不得使用用户项目。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p ai_tool_gateway --test mcp_process_smoke external_codex_authoring_process_smoke -- --exact
```

必须报告 `1 passed`；0 tests、ignored 或只测 JSON handler 不合格。

### 12.4 commit-bound release build

C5 全绿后形成 clean exact commit，再构建实际安装输入：

```powershell
Set-Location G:\gameEngin\rust
cargo build --locked --release -p ai_tool_gateway --bin ai_engine_gateway_mcp --bin ai_engine_gateway_codex_config
```

记录 commit/tree、profile、features、两个 binary path/size/SHA-256。这里的 seal 只绑定
即将安装的 binary，不建立 candidate/freezer。

## 13. C6：真实 stable install 与 Codex config migration

### 13.1 独立授权

C6 是唯一真实外部配置写入窗口，必须单独明确授权。启动前重新核对：

```text
C5 clean exact commit 与 release binary seal
real config path 是 regular non-reparse file
ai_first_game_engine command 精确等于用户授权的 expected historical command
stable target 位于当前用户 LOCALAPPDATA installation root
artifact/backup/receipt root 私有且可写
Codex/Editor 当前状态不会导致目标 binary 替换不确定
rollback 命令和恢复材料可用
```

### 13.2 唯一迁移

施工实现冻结的 CLI 语法必须在 C0 写入 `--help` 与 tests。预期调用形态：

```powershell
Set-Location G:\gameEngin\rust
.\target\release\ai_engine_gateway_codex_config.exe migrate-stable `
  'I:\EngineTest\GateF254R1Frozen\candidate-20260720-000200\bin\ai_engine_gateway_mcp.exe' `
  '.\target\release\ai_engine_gateway_mcp.exe'
```

helper 自己解析 real config、stable target 与 private artifact root；caller 不传可覆盖
其它 MCP entry 的 fragment。任何 exact precondition drift 都在零 config 写入下失败。

### 13.3 成功后强制停止

成功必须验证：

```text
stable executable exists and hash matches receipt
real config only target entry changed
backup + combined durable receipt exist
after digest matches real config
reloadOrNewTaskRequired=true
rollback command material complete
```

然后立即停止当前 Codex task，不执行 C7，不声称新 MCP 已加载。用户必须 reload 或创建
新的 Codex task。

若写入结果不确定，禁止重跑 migration；先按 receipt/backup 做只读诊断。

## 14. C7：新 Codex task typed MCP smoke 与完成

### 14.1 进入条件

```text
新 Codex task 已加载 mcp_servers.ai_first_game_engine
typed tools 来自 stable installed executable
一个 run-owned disposable project 已在 production Native Editor 打开
C6 receipt、stable binary hash 与 Editor discovery 均匹配
用户明确授权一次 disposable mutation smoke
```

### 14.2 最小真实 smoke

必须通过真实 typed MCP 调用，禁止 shell/CLI/测试 JSON-RPC client 代替：

```text
aife_status
aife_catalog
project.inspect
project.mutate(disposable harmless project change)
  -> awaiting_user
用户在 Native Editor 批准一次
aife_observe(same operation)
  -> succeeded
project.rollback(returned rollbackRef)
project.inspect
  -> initial digest restored
```

这只是 259 activation qualification，不是普通 AI 用户必须遵循的菜谱。不运行 Preview、
runtime diagnostics、Build、Delivery、F-A/F-B/G 或三引擎通道。

任一 mutation 结果不确定时立即停止，不再次调用 mutation。reject、TTL、drift、cancel、
disconnect 都按 terminal 处理。

### 14.3 完成标准

C7 通过且 disposable project 恢复初始 digest 后：

1. 生成 259 阶段完成记录。
2. 同步 README、00、49、54、施工文档 README、阶段完成记录 README。
3. 将本文从 `当前/` 移到 `已完成/`。
4. 保留 stable installation 与目标 config；删除 run-owned disposable fixture/temp。
5. 报告真实 binary/config/Editor/session/project/operation/receipt/rollback lineage。

## 15. 失败分类与重试

| 失败 | 分类 | 处理 |
|---|---|---|
| 正式 Interface 无法在现有 owner 内实现 | 方案问题 | 停止并先修 259 正式方案 |
| Gate 要求重复或无法证明声明 | 施工文档问题 | 修订本文并重新自审 |
| owner test 首个 assertion 失败 | 实现问题 | 只修 owner，重跑该 contract |
| baseline test 与 259 无关地失败 | baseline 问题 | 保存首因，不顺手扩 scope |
| release build/installed profile 不匹配 | environment/composition | 修复身份，不用单测冒充 |
| config expected command drift | external precondition | 零写入停止，等待用户决定 |
| migration 后状态不确定 | external transaction | 禁止重跑，按 receipt/backup 诊断 |
| real mutation terminal failed | production behavior | 不重试 mutation，回到 owner 诊断 |
| 超过三小时 | time boundary | cleanup、记录 `stopped_by_time_limit` |

非外部、无副作用测试可在同一连续授权窗口内于首因修复后重跑。真实 config migration
和结果不确定的 mutation 不自动重试。

## 16. 证据、cleanup 与文档同步

### 16.1 每窗口记录

```text
window/start/end/elapsed
source HEAD/tree/worktree
changed owners/consumers
tests exact command/count/result
first failure and repair
external state before/after
owned process cleanup
owned temp cleanup
remaining risk/omitted validation
```

### 16.2 cleanup

测试使用独立 `TEMP/TMP/LOCALAPPDATA/CODEX_HOME`。完成后只删除 run-owned strict-child
目录；遵守 `game-engine-construction` skill 的 reparse/process/evidence proof。

真实 config backup、migration receipt 与 stable installed binary 不是 temp，不得在普通
cleanup 中删除。disposable smoke project 必须是 repository-external run-owned copy。

### 16.3 入口状态

激活前：

```text
当前施工：无
待执行：259
施工授权：无
```

激活后：

```text
当前施工：259
待执行：无
只执行用户授权窗口
```

## 17. 激活前复核

只有全部满足才可把本文移动到 `当前/`：

```text
[ ] 259 正式方案与本文已经形成纯文档 commit
[ ] 当前/ 没有其它非 artifact 施工文档
[ ] source HEAD、Rust toolchain 与生成基线漂移已重新审查
[ ] C0-C7 涉及文件和测试入口仍存在或已在本文修订
[ ] current Codex config 只读检查确认 exact expected command
[ ] current discovery mixed-record 问题仍可复现且未被其它修改解决
[ ] current production Native Editor authorization composition 已重新核对
[ ] 没有用户未提交源码变化与 259 owner 冲突
[ ] 窗口 A 保守估算小于 3 小时
[ ] 用户明确授权激活和具体窗口
```

复核失败时本文留在 `待执行/`，先修订并重新自审。不得原样施工过期计划。

## 18. 施工文档自审

### 18.1 与正式方案一致

通过。本文只实现 stable connection recovery、深 `project.mutate`、同 operation
批准、bounded Grant reuse 和 `project.rollback(rollbackRef)`。

### 18.2 正式方案是否需要修改

无需修改。基线调查确认 existing `codex_config`、discovery、Grant、Candidate、
operation observation 和 Native Editor Inbox 可以承载 B-min；施工不需要新增正式
Module 或 public lifecycle。

### 18.3 是否限制 AI

没有。AI 仍自由组合 inspect/search/mutate/preview/diagnostics/build/rollback。本文只封装
单个 mutation 内部正确性步骤。

### 18.4 是否形成外部菜谱

没有。C0-C7 是施工与 qualification 切片，不进入 typed Tool Interface。真实 C7 序列只
证明 activation，不要求普通 AI 用户照做。

### 18.5 Module 是否过多

没有。只深化现有 `EditorInstanceGatewayModule`，并在既有 owner 内实现内部
`GoalMutationModule`。stable installer 是维护 CLI/Implementation，不是公共 Module。

### 18.6 测试是否过多

没有。C0-C4 各自在不同 owner/Interface 上提供 red-capable 反馈；C5 只跑实际 affected
closure；process smoke 与 real Codex smoke 分别证明 process composition 和 host loading。
没有默认 Local CI、全 workspace、Preview/Build 或历史 Gate。

### 18.7 是否重复历史 254-R1/R2

没有。没有 candidate freezer、release candidate、真实 AI outcome acceptance、F-A/F-B/G
或三引擎 cohort。内部 Candidate 只作为 mutation safety Implementation。

### 18.8 外部状态是否安全

通过。真实 config 只在 C6 独立授权中按 exact expected command 事务替换；成功后强制停止，
新 task 才验证加载。C7 只修改 run-owned disposable project 并 rollback。

### 18.9 三小时规则

通过。连接、mutation、approval、rollback、integration、migration 与新 task smoke 已拆为
独立窗口；任何窗口 2:45 停止启动新工作、3:00 强制结束。

### 18.10 是否跳过测试

没有。每个可执行需求都有 owner contract 或明确 integration/activation 证据。相同配置下
不机械重复已被上层 suite 覆盖的 filtered tests。

### 18.11 激活时是否越权

没有。激活时授权只覆盖窗口 A 的 C0-C1 源码、owner tests 与定向验证；不覆盖 C2、
真实配置、production Editor、production smoke 或 Local CI。该授权已由第 20 节结果
消费并关闭。

### 18.12 自审结论

```text
result：passed
scope expansion：none
formal scheme conflict：none
missing executable requirement：none
duplicate qualification：none
external-state authorization：not granted
activation：passed
activation-time next step：执行窗口 A / C0-C1；完成或失败后停止
```

## 19. 2026-07-27 激活前复核与窗口 A 授权

```text
result：passed
pure docs commit：d595b0a4da419723554cf087c8a5d01830dbb727
source baseline before activation：0e4d8d8c17c91e0e895163dd2be6e08ad6b29827
Rust toolchain：1.96.0
source worktree conflicts：none
current construction document conflict：none
Codex config：regular file、non-reparse
actual command：精确匹配 historical frozen MCP path
discovery records：7
active compatible Editor：PID 9800，G:\gameEngin\rust\target\debug\editor_host.exe
dead stale records：6（v1=3，v2=3）
G: space：sufficient
authorization at activation：窗口 A / C0-C1（已消费并关闭）
forbidden：C2-C7、真实 config、production install/smoke、Local CI
```

## 20. 窗口 A / C0-C1 施工结果

### 20.1 C0 stable install/config

实现：

```text
stable target：%LOCALAPPDATA%\AiFirstGameEngine\bin\ai_engine_gateway_mcp.exe
same-parent atomic binary replace
source/installed/previous digest 与 backup
exact expected Codex command read-only preflight
config + binary combined durable migration receipt
failure-atomic config/binary restore
rollback-stable maintenance CLI
symlink/reparse/oversize/path drift fail closed
changed=false config rollback no-op
```

定向验证：

```text
cargo test -p ai_tool_gateway --test stable_install_contract
result：4 passed / 0 failed / 0 ignored

cargo test -p ai_tool_gateway --lib codex_config
result：3 passed / 0 failed
```

### 20.2 C1 discovery recovery

实现：

```text
active_compatible / active_incompatible / dead_stale internal classification
owned legacy v1 filename/root-digest/protocol/pipe validation
dead owned v1/v2 best-effort cleanup
unique compatible Editor selection despite dead stale records
multiple live compatible ambiguity remains fail closed
unknown/malformed/unowned/reparse/oversize remains fail closed
explicit legacy path remains strict
```

定向验证：

```text
cargo test -p ai_tool_gateway --test discovery_recovery
result：6 passed / 0 failed / 0 ignored

cargo test -p ai_tool_gateway --lib discovery
result：8 passed / 0 failed
```

### 20.3 受影响 crate 回归

```text
cargo fmt --check
result：passed

cargo test -p ai_tool_gateway
library：61 passed
core_scope_contract：3 passed
discovery_recovery：6 passed
explicit_reconnect：1 passed
mcp_process_smoke：3 passed
stable_install_contract：4 passed
doc tests：0 failed
```

existing `engine_runtime` dead-code warnings 未由 259 引入，未扩大 scope。

### 20.4 失败与重跑

```text
首次组合定向命令：宿主在 124 秒 command timeout 终止，未产生 test failure evidence
处理：同一 owner 命令使用足够 timeout 重跑，测试通过

一次 cargo fmt --check：只报告新代码格式差异
处理：cargo fmt --all 后重跑 owner tests，全部通过

implementation failure：none
external one-shot consumed：none
```

### 20.5 身份与外部状态

```text
implementation commit：400d2d1bc13f21f06dd621a50f9b4295f2c2dd8b
tree：b20a07d3854c165a0e141618c276e2673cf88d6d
real Codex config：未修改
real command：仍指向 historical frozen MCP
real stable MCP path：不存在
production Editor PID 9800：未重启、未修改
real discovery records：仍为 7
production install/smoke：未执行
Local CI：未执行
```

### 20.6 窗口关闭

```text
window A result：passed
C0：passed
C1：passed
    current authorization：无
    next：如需继续，单独授权窗口 C / C3
```

窗口 A 已停止。窗口 B / C2 的结果见第 21 节。

## 21. 窗口 B / C2 施工结果

### 21.1 Deep `project.mutate` contract

实现：

```text
public project.mutate caller input：
  goal.outcome
  change.kind
  change.payload

内部 GoalMutationModule：
  normalized goal outcome
  Gateway client session 与 read generation
  active project identity/root/digest
  ProjectPatch context hash
  actual-operation-derived risk
  engine-owned ProjectPatch facts
  internal Candidate input
  apply 前 project/context drift revalidation

Gateway：
  拒绝 caller 提交 bound Candidate/internal payload
  将 caller intent 转换为同 session 的内部 bound mutation
  复用既有 Candidate prepare/validate/apply 原子实现
  成功 receipt 的 tool identity 为 project.mutate
  成功后推进 project digest 与 Gateway read generation
```

C2 按计划暂时保留 `project.mutate.candidate`，只增加 `project.mutate`；旧 public
entries 的一次性 cutover 仍属于 C4。没有实现 C3 的 awaiting-user continuation、
same-operation approval 或 bounded Grant reuse。

### 21.2 验证

```text
cargo test -p editor_core goal_mutation_contract -- --test-threads=1
result：2 passed / 0 failed

cargo test -p ai_tool_gateway --test deep_mutation_contract -- --test-threads=1
result：2 passed / 0 failed

cargo test -p ai_tool_gateway goal_mutation_contract_executes_bound_intent_and_advances_session_facts -- --test-threads=1
result：1 passed / 0 failed

cargo test -p editor_core -- --test-threads=1
result：559 passed / 2 ignored / 0 failed

cargo test -p ai_tool_gateway -- --test-threads=1
result：library 62 passed；integration 3 + 2 + 6 + 1 + 3 + 4 passed；0 failed

cargo fmt --all --check
result：passed

git diff --check
result：passed
```

既有 `engine_runtime` 三项 dead-code warning 未由 C2 引入。

### 21.3 事故与证据判定

```text
一次组合并行回归：
  Gateway Windows named-pipe tests 挂起并触发 15 分钟宿主 timeout
  owned PIDs 22052 / 32672 / 27848 已终止
  未产生产品 assertion failure

收敛：
  使用同一源码、单线程完整 crate 回归
  editor_core 与 ai_tool_gateway 均完整通过

分类：
  test concurrency / environment behavior
  不是 C2 产品失败

一次 Gateway 精确名称命令：
  因 Rust 完整测试路径不匹配得到 0 tests
  不计为通过证据
  随后使用名称过滤得到明确 1 passed
```

### 21.4 身份与外部状态

```text
activation commit：b3f4db02df7a10b18333ac0bd662aa6140d6a1c1
implementation commit：71950700dbbdb3bbc01fd747fe41773c4d8f5036
implementation tree：dc0b658ac1f3a8e0bcc5a7dff5dcb3906b19a1d7
real Codex config：未修改
stable MCP installation：未修改
production Editor：未启动、未重启、未修改
production discovery：未修改
production typed MCP smoke：未执行
Local CI：未执行
owned test process：无残留
```

### 21.5 窗口关闭

```text
window B result：passed
C2：passed
current authorization：无
next：如需继续，单独授权窗口 C / C3
forbidden without new authorization：
  C3-C7
  production MCP install/smoke
  real Codex config
  Local CI
```

窗口 B 已停止。窗口 C 仅允许执行 C3，不得进入 C4。

## 22. 窗口 C / C3 施工结果

### 22.1 Same-operation approval

```text
project.mutate 无 eligible Grant：
  创建 session-bound AwaitingUser operation
  同一 operation 绑定 Native Editor access request
  Approve command 自动启动原 invocation
  Kernel operation identity 与 awaiting identity 保持相同

reject / TTL / drift / cancel / disconnect：
  operation terminal
  Candidate 不 apply
  不自动重试
```

没有新增 public `resume`、`continue`、`request_access` 或第二套 approval UI。
Native Editor 继续复用既有 authorization Inbox 与 Approve/Reject command，并显示
operation short identity、derived risk、goal、project、budget 与 TTL。

### 22.2 bounded Grant reuse

Gateway 只在 normalized outcome、Editor instance、client session、project identity、
risk envelope 与剩余 mutation/time/external-cost budget 全部匹配，且 Grant 未过期、
未撤销、lineage digest 等于当前 project digest 时复用。goal、risk、budget、session、
project drift、TTL 或 disconnect 变化均不复用。

### 22.3 验证

```text
cargo fmt --all --check
result：passed

cargo test -p ai_tool_gateway --test deep_mutation_contract same_operation -- --test-threads=1
result：3 passed / 0 failed

cargo test -p editor_window_winit gateway_goal_mutation -- --test-threads=1
result：1 passed / 0 failed

cargo test -p editor_core -- --test-threads=1
result：560 passed / 2 ignored / 0 failed

cargo test -p ai_tool_gateway -- --test-threads=1
result：library 62 passed；全部 integration tests passed；0 failed

cargo test -p editor_ui_model -- --test-threads=1
cargo test -p editor_ui_renderer -- --test-threads=1
cargo test -p editor_window_winit -- --test-threads=1
result：全部 passed；editor_window_winit 142 passed / 0 failed

git diff --check
result：passed
```

既有 `engine_runtime` 三项 dead-code warning 未由 C3 引入。

### 22.4 事故与证据判定

```text
新 C3 tests 初次编译：
  Rust 同一调用中的 mutable/immutable session borrow 冲突
  分类：test implementation
  处理：提前构造 invocation；未改产品语义

drift negative test：
  初始断言错误地要求 external drift 后 digest 仍等于 initial
  分类：test assertion
  处理：绑定 drift 后 digest，继续证明 Candidate 未额外 apply

Windows 首次链接：
  外层 command timeout 时 cargo/rustc 仍在完成链接
  分类：tool timeout / linker cost
  处理：等待 owned processes 完成，缓存后定向测试通过

完整 Gateway regression：
  一个旧 test 仍假定 project.mutate 可在批准前直接 pump
  分类：C3 后 consumer test contract drift
  处理：改为 AwaitingUser -> existing Approve command -> same operation pump

product failure：none
external one-shot consumed：none
```

### 22.5 身份与外部状态

```text
activation commit：64d735e
implementation commit：392336c
real Codex config：未修改
stable MCP installation：未修改
production Editor：未启动、未重启、未修改
production discovery：未修改
production typed MCP smoke：未执行
Local CI：未执行
owned test process：无残留
```

### 22.6 窗口关闭

```text
window C result：passed
C3：passed
current authorization：无
next：如需继续，单独授权窗口 D / C4
forbidden without new authorization：
  C4-C7
  production MCP install/smoke
  real Codex config
  production Editor start/restart
  Local CI
```

窗口 C 已停止。未进入 C4。

## 23. 窗口 D / C4 施工结果

### 23.1 实现

```text
activation commit：c49ce10
implementation commit：cdef3a0
public mutation tools：
  project.mutate
  project.rollback
retired public tools：
  project.mutate.candidate
  project.rollback.candidate
```

Gateway 现在为成功 mutation 生成 session/editor/project/digest/read-generation-bound
opaque `rollbackRef`，内部保存可信 mutation receipt 与 Candidate rollback material。
`project.rollback` 只接受 `schemaVersion + rollbackRef`，并对 unknown、expired、
different session/editor/project、digest/read-generation drift、receipt/material mismatch
和 consumed ref fail closed。成功 rollback 后 ref 一次性消费。

Registry、Catalog、MCP projection source consumer 与 caller tests 已一次性切换到
`project.mutate` / `project.rollback`；旧 direct Candidate schema/example 生成器和两条
ignored 旧 contract tests 已物理删除。Kernel 内部 Candidate prepare/apply/rollback owner
保留，不重新公开为 caller 菜谱。

### 23.2 验证

```text
cargo fmt --all --check
result：passed

cargo test -p ai_tool_gateway --test rollback_contract -- --test-threads=1
result：3 passed / 0 failed

cargo test -p ai_tool_gateway --lib rollback_ref_owner_rejects_expiry_receipt_tamper_and_project_mismatch -- --test-threads=1
result：1 passed / 0 failed

cargo test -p ai_tool_gateway --lib rollback_ref_kernel_rejects_tampered_internal_rollback_material -- --test-threads=1
result：1 passed / 0 failed

cargo test -p editor_core goal_mutation_contract -- --test-threads=1
result：2 passed / 0 failed

cargo test -p editor_core ai_tool_contract_registry_rejects_retired_candidate_public_entries -- --test-threads=1
result：1 passed / 0 failed

cargo test -p editor_core ai_tool_catalog_v2_reports_all_reasons_with_stable_precedence -- --test-threads=1
result：1 passed / 0 failed

cargo test -p ai_tool_gateway --test core_scope_contract -- --test-threads=1
result：3 passed / 0 failed

cargo test -p project_e2e_gate --lib gateway_unknown_task_preflight_keeps_large_competition_margin -- --test-threads=1
result：1 passed / 0 failed

rg -n 'project\.mutate\.candidate|project\.rollback\.candidate' crates --glob '*.rs'
result：NO_LITERAL_MATCHES
```

另对 7 条使用旧 Candidate helper 的 Gateway 定向 consumer tests 完成 intent 迁移并逐条
通过。完整 crate suite、production process smoke、production typed MCP 与 Local CI
属于 C5-C7，未在 C4 执行。

既有 `engine_runtime` 三项 dead-code warning 未由 C4 引入。

### 23.3 事故与证据判定

```text
rollbackRef schema 首次实现：
  repository lightweight validator 把 regex pattern 当成 literal prefix
  分类：schema implementation
  处理：schema 保留长度约束，严格 rbk_ 格式由 Rust decoder 验证

新增 owner tests 首次编译：
  同一表达式 mutable/immutable session borrow 冲突
  分类：test implementation
  处理：提前构造 hello 与 invocation

project_e2e_gate consumer：
  仅改 Tool ID 后仍提交内部 Candidate，随后又复用 mutation 前旧 digest Grant
  分类：C4 caller test contract drift
  处理：迁移为 goal intent + same-operation approval，并在 mutation 后签发当前 read Grant

超时的 project_e2e_gate 组合命令：
  cargo 构建全部 integration targets，外层 timeout 后 owned Cargo 继续编译
  分类：test command scope / tool timeout
  处理：终止仅该 owned Cargo root，改用 --lib 单 test；定向测试通过

旧 Gateway tests：
  两条仍要求 pre-approval rejection 或 direct Candidate concurrent writers
  分类：retired public contract
  处理：删除重复旧测试；same-operation、non-reuse、drift 与 Kernel concurrency owner
        已有当前合同覆盖

product failure：none
external one-shot consumed：none
```

### 23.4 身份与外部状态

```text
activation commit：c49ce10
implementation commit：cdef3a0
real Codex config：未修改
stable MCP installation：未修改
production Editor：未启动、未重启、未修改
production discovery：未修改
production typed MCP smoke：未执行
Local CI：未执行
owned test process：无残留
```

### 23.5 窗口关闭

```text
window D result：passed
C4：passed
historical current authorization at C4 closeout：无
historical next at C4 closeout：如需继续，单独授权窗口 E / C5
historical forbidden without then-new authorization：
  C5-C7
  production MCP install/smoke
  real Codex config
  production Editor start/restart
  Local CI
```

窗口 D closeout 的历史状态到此结束；后续窗口 E / C5 结果见第 24 节。

## 24. 2026-07-28 窗口 E / C5 closeout

### 24.1 结果

```text
window：E / C5 affected integration + release process smoke + release build
result：passed
source commit：f044223475aa49c6933eaad57fcdb40b289b3743
source tree：15801f2521829b4e17c751f3bdfaae256260912a
worktree during qualification：clean source commit；文档 closeout 另行提交
real Codex config：未修改
stable MCP installation：未修改
production Editor：未启动、未重启、未修改
production discovery：未修改
production typed MCP smoke：未执行
Local CI：未执行
owned process cleanup：无残留
owned temp cleanup：process smoke run-owned root removed
```

### 24.2 证据

```text
cargo test -p ai_tool_gateway --test mcp_process_smoke external_codex_authoring_process_smoke -- --exact
result：1 passed / 0 failed / 0 ignored / 3 filtered out

cargo test -p ai_tool_gateway
result：all ai_tool_gateway unit/integration/doc suites passed
notable counts：
  lib unit：62 passed
  core_scope_contract：3 passed
  deep_mutation_contract：5 passed
  discovery_recovery：6 passed
  explicit_reconnect：1 passed
  mcp_process_smoke：4 passed
  rollback_contract：3 passed
  stable_install_contract：4 passed

cargo build --locked --release -p ai_tool_gateway --bin ai_engine_gateway_mcp --bin ai_engine_gateway_codex_config
result：passed
profile：release
features：default Cargo feature set
```

release binary seal：

```text
G:\gameEngin\rust\target\release\ai_engine_gateway_mcp.exe
size：7076352
sha256：a2b129f1b4ddef16adc39dd0a600d283a9f9d594b6f8d44b8976296c7bbb3a47

G:\gameEngin\rust\target\release\ai_engine_gateway_codex_config.exe
size：996352
sha256：75c271028beb8c0ea15bae2b4fc49461d687c421ec1874be2784246da8065e7a
```

既有 warning：

```text
engine_runtime dead-code warning 3 项仍存在，非 C5 引入：
  RuleExecutorHint::IrInterpreter
  RuleCall::validation_only_ir_interpreter
  World::remove_sprite_renderer2d
```

### 24.3 事故与修复

```text
initial exact command：
  cargo test -p ai_tool_gateway --test mcp_process_smoke external_codex_authoring_process_smoke -- --exact
  result：0 tests；不合格证据
  分类：verification asset omission
  修复：新增同名 repository-owned process smoke test，覆盖真实 CARGO_BIN_EXE_ai_engine_gateway_mcp、
        repository-external isolated LOCALAPPDATA、mixed stale discovery、public tools/list、
        project.mutate approval、same-goal Grant reuse、rollbackRef restore 与 disconnect no-apply。

新增 smoke 首轮断言：
  将未授权前 project.mutate 错判为 ready；将 project.rollback 错判为 blocked。
  分类：test implementation drift
  修复：按当前 Catalog contract 收窄为 project.mutate authorization_required；rollback 可公开，
        安全性由 opaque rollbackRef owner check 验证。

product failure：
  same-goal Grant reuse 后的 queued kernel operation 不在 goal_mutation_operations 中；
  Gateway close 删除 operation_grants 前未先 invalidate Tool Kernel authority。
  结果：MCP close 后继续 pump 可 apply pending mutation。
  分类：implementation bug；未偏离 259 设计，属于 close owner boundary 漏处理。
  修复：GatewayCore::remove_session 在移除 session metadata/grants 前，对该 session 的
        operation_grants 全部调用 kernel.invalidate_operation_authority。
  验证：external_codex_authoring_process_smoke 在 close 后重复 pump，project digest 仍等于 initial digest。

cleanup failure：
  Windows 首次 remove_dir_all 时仍有 session/core handle。
  分类：test cleanup ordering
  修复：server.join 后先 drop publication/core/dispatcher/session，再删除 run-owned root。
```

### 24.4 窗口关闭

```text
window E result：passed
C5：passed
current authorization：无
next：如需继续，单独授权窗口 F / C6
forbidden without new authorization：
  C6-C7
  stable MCP install/config migration
  production typed MCP smoke
  real Codex config
  production Editor start/restart
  production project mutation
  Local CI
```

窗口 E 当时已停止；C6 后续结果见第 25 节。

## 25. 2026-07-28 窗口 F / C6 real stable install/config migration

### 25.1 结果

```text
window：F / C6 real stable install/config migration
result：passed
start/evidence time：2026-07-28 10:46:08.146 +08:00
end：2026-07-28 10:48:28.496 +08:00
source HEAD at migration：a280d60171396b2e0cdd3e81e4ca5e07bc5029a1
source tree at migration：586b282f4e0407270c4bff408bc61c628faf714c
product implementation commit reused：f044223475aa49c6933eaad57fcdb40b289b3743
worktree before migration：clean
worktree source changes since C5：docs only；rustChangesSinceC5=none
production Editor：未启动、未重启、未修改
production typed MCP smoke：未执行
Local CI：未执行
C7：未执行
```

### 25.2 前置核对

```text
current config path：C:\Users\zenghaoran\.codex\config.toml
configRegularNonReparse：true
current ai_first_game_engine command before migration：
  I:\EngineTest\GateF254R1Frozen\candidate-20260720-000200\bin\ai_engine_gateway_mcp.exe
expected historical command match：true
historical MCP exists：true
historical MCP sha256：e20c31559b94490020cef18e045f4d97574970c570421d6cc0e13239e40dcc94

source MCP：
  G:\gameEngin\rust\target\release\ai_engine_gateway_mcp.exe
  sha256：a2b129f1b4ddef16adc39dd0a600d283a9f9d594b6f8d44b8976296c7bbb3a47
  matches C5 release seal：true

config helper：
  G:\gameEngin\rust\target\release\ai_engine_gateway_codex_config.exe
  sha256：75c271028beb8c0ea15bae2b4fc49461d687c421ec1874be2784246da8065e7a
  matches C5 release seal：true

stable MCP target：
  C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\bin\ai_engine_gateway_mcp.exe
  under current-user LOCALAPPDATA：true
  no reparse ancestors：true
artifact root：
  C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\Gateway\codex-config
  no reparse ancestors：true
```

### 25.3 执行命令与 receipt

```powershell
Set-Location G:\gameEngin\rust
.\target\release\ai_engine_gateway_codex_config.exe migrate-stable `
  'I:\EngineTest\GateF254R1Frozen\candidate-20260720-000200\bin\ai_engine_gateway_mcp.exe' `
  'G:\gameEngin\rust\target\release\ai_engine_gateway_mcp.exe'
```

stdout：

```text
migrated=true reload_required=true command=C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\bin\ai_engine_gateway_mcp.exe config=C:\Users\zenghaoran\.codex\config.toml receipt=C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\Gateway\codex-config\1785206768146-stable-mcp-migration-receipt.json
```

receipt：

```text
path：C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\Gateway\codex-config\1785206768146-stable-mcp-migration-receipt.json
schemaVersion：ai-tool-gateway-stable-mcp-migration-receipt.v1
sourceMcpDigest：sha256:a2b129f1b4ddef16adc39dd0a600d283a9f9d594b6f8d44b8976296c7bbb3a47
installedMcpDigest：sha256:a2b129f1b4ddef16adc39dd0a600d283a9f9d594b6f8d44b8976296c7bbb3a47
binaryChanged：true
previousInstalledBackupPath：null
config.schemaVersion：ai-tool-gateway-codex-config-install-receipt.v1
config.backupPath：C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\Gateway\codex-config\1785206768392-config-before.toml
config.fragmentPath：C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\Gateway\codex-config\1785206768392-ai-first-game-engine-mcp.toml
config.beforeDigest：sha256:41bf9dfb26ca48adcae42ec1f82cc8488d55fba11bc6cbcb4d8306d339ba82c0
config.afterDigest：sha256:ce4d7ffadcabbbc82f33f45d3a9a714ddb2c8fc412e34627067d4692f67c8735
config.changed：true
config.reloadOrNewTaskRequired：true
```

### 25.4 成功标准验证

```text
receiptExists：true
stable executable exists：true
installedDigestMatchesReceipt：true
sourceEqualsInstalledDigest：true
configBackupExists：true
configFragmentExists：true
currentConfigDigest：sha256:ce4d7ffadcabbbc82f33f45d3a9a714ddb2c8fc412e34627067d4692f67c8735
currentConfigMatchesReceipt：true
oldAifeCommandMatchesExpectedHistorical：true
newAifeCommandMatchesInstalledPath：true
maskedConfigEqualExceptAifeCommand：true
reloadOrNewTaskRequired：true
rollback command material：
  G:\gameEngin\rust\target\release\ai_engine_gateway_codex_config.exe rollback-stable "C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\Gateway\codex-config\1785206768146-stable-mcp-migration-receipt.json"
```

### 25.5 窗口关闭

```text
window F result：passed
C6：passed
current authorization：无
next：reload 或创建新的 Codex task 后，单独授权窗口 G / C7
forbidden without new authorization：
  C7
  production typed MCP smoke
  production Editor start/restart
  production project mutation
  repeated config migration
  rollback-stable C6 receipt（除非单独授权诊断/恢复）
  Local CI
```

窗口 F 已停止。按第 13.3 节强制边界，当前 task 不执行 C7，不声称新 MCP 已加载。

## 26. 2026-07-28 窗口 G / C7 准备与 transport 阻断

### 26.1 已完成的准备

```text
source HEAD：ef2ed6b3c70562b3aeab853b54fbd8f4401142b1
source tree：acb91a05ef56ec5336ab7616a657a7e8a5aa4dd9
worktree before C7：clean
stable MCP：
  C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\bin\ai_engine_gateway_mcp.exe
  sha256：a2b129f1b4ddef16adc39dd0a600d283a9f9d594b6f8d44b8976296c7bbb3a47
C6 receipt：
  C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\Gateway\codex-config\1785206768146-stable-mcp-migration-receipt.json
production Editor：
  G:\gameEngin\rust\target\debug\editor_host.exe
  sha256：3d9ae8c3d829fbf552a856467b54ddc54775bbb8a525713be2b26307f4533c09
  PID：7500
  source：clean HEAD 定向重建；仅既有 engine_runtime 三条 dead-code warning
disposable project：
  G:\AifeC7Runs\run-1785211733308\picker-start\c7-disposable-project
  active in production Editor：true
discovery：
  editorProcessId：7500
  editorInstanceId：sha256:97fe29eadd233a116e0fa014eba039c55d63366e33957ce50515493a5578d315
```

首次 C7 task 发现旧 production Editor 仍公开 Candidate-era Catalog，因此未发 mutation，
关闭旧 Editor 后用当前 clean HEAD 定向重建并启动 PID `7500`。该 task 的 MCP transport
因 Editor replacement 关闭，按规则停止并要求新 task。

### 26.2 新 task 阻断证据

第二个新 task 在 disposable project 已打开后启动。Codex 主进程从 C6 stable path 启动
`ai_engine_gateway_mcp.exe` PID `5920`，但当前 task 的 tool catalog 不包含
`aife_status`、`aife_catalog`、`aife_project_inspect` 或任何 `aife_*` typed tool。

Codex app 本地日志重复记录：

```text
server_name=ai_first_game_engine
has_cached_tools=true
startup_complete=true
MCP server tools unavailable while building tool list
```

分类：

```text
preparation task 1：2026-07-28 11:28:52 +08:00 -> 11:42:08 +08:00
preparation task 2：2026-07-28 12:00:09 +08:00 -> 12:12:22 +08:00
transport verification task：2026-07-28 13:36:58 +08:00 -> 13:51:54 +08:00
failure class：environment/composition
product mutation failure：none
typed MCP tool call count in second task：0
project.mutate count across prepared replacement Editor：0
project.rollback count：0
Native Editor approval consumed：0
project bytes changed by C7 mutation：no
259 completion eligibility：false
```

当前不能用 shell、CLI 或测试 JSON-RPC client 代替真实 typed MCP，也不能把 cached schema
冒充 activation 成功。没有修改 Rust 源码、真实 Codex config 或 disposable project。

### 26.3 窗口关闭与恢复条件

```text
window G result：blocked before typed smoke
C7：not started
current authorization：无
retained production Editor：PID 7500
retained disposable root：G:\AifeC7Runs\run-1785211733308
historical recovery hypothesis（已被第 28 节诊断推翻）：
  修复或更新 Codex app MCP tool-delivery transport
  完整退出并重启修复后的 Codex app，使共享 MCP connection manager 与 child process 全部重建
  创建新的 Codex task
  复核 PID 7500、discovery、stable binary/C6 receipt 与 disposable project
  单独授权 Window G / C7
forbidden without new authorization：
  production typed MCP call
  project.mutate / project.rollback
  repeated config migration or rollback-stable
  Local CI
  259 completion archive
```

本次没有 terminal mutation，因此不消耗“一次真实 mutation”尝试；但连续授权窗口已关闭，
不得在当前 task 内杀死/重启共享 Codex MCP host 后继续。

## 27. 2026-07-28 窗口 G / C7 完整重启后复核

用户在新任务单独授权继续 Window G / C7。该授权允许一次 disposable mutation smoke，
但进入条件中的 typed tool delivery 仍未成立，因此 mutation 授权未消费。

### 27.1 身份与前置状态

```text
verification start：2026-07-28 14:02 +08:00
source HEAD：9c7c2f93432a023c2157ac8d2b4bd64b378a0d79
source tree：bf9c1ac2d4095233fbc57d608a3a1d9cf47b302b
worktree before verification：clean
Codex app version：26.721.4979.0
Codex app PID：27100
Codex app start：2026-07-28 13:55:04 +08:00
stable MCP PID：19776
stable MCP start：2026-07-28 14:01:42 +08:00
stable MCP path：
  C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\bin\ai_engine_gateway_mcp.exe
stable MCP sha256：
  a2b129f1b4ddef16adc39dd0a600d283a9f9d594b6f8d44b8976296c7bbb3a47
C6 installed binary digest match：true
Codex config target command match：true
production Editor PID：7500
disposable project exists：true
disposable project latest file write：2026-07-28 13:36:36 +08:00
verification start 后 project file write：none
```

Codex app 主进程和 stable MCP 子进程均晚于上一次 transport verification，证明完整重启
已经发生；这不是旧 app process 或旧 MCP child 的残留。

### 27.2 Typed tool delivery 结果

```text
tool_search query for aife_status/aife_catalog/project.inspect/project.mutate/project.rollback：
  no ai_first_game_engine tool returned
current callable tool metadata aife_* count：0
aife_status call count：0
aife_catalog call count：0
project.inspect call count：0
project.mutate call count：0
project.rollback call count：0
aife_observe call count：0
Native Editor approval consumed：0
project bytes changed by C7 mutation：no
C7 result：blocked before typed smoke
failure class：environment/composition
```

不得用 shell、CLI、测试 JSON-RPC client 或 cached schema 代替真实 typed MCP。再次完整
重启已执行且未恢复 tool delivery，因此不再把“再重启一次”当作有证据的恢复动作。

### 27.3 当前边界

```text
259 completion eligibility：false
current authorization after report：无
retained production Editor：PID 7500
retained disposable root：G:\AifeC7Runs\run-1785211733308
historical recovery hypothesis（已被第 28 节诊断推翻）：
  修复或更新 Codex app MCP tool-delivery transport
  完整重启修复后的 Codex app
  创建新任务并复核 stable MCP/Editor/disposable project
  单独授权 Window G / C7
forbidden：
  用 shell/CLI/test client 替代 production typed MCP
  project.mutate / project.rollback
  repeated config migration or rollback-stable
  Local CI
```

## 28. C7-R1：Production 多连接恢复修复

### 28.1 诊断证据

用户单独授权 production read-only MCP handshake diagnosis。使用 C6 stable installed binary，
只发送 `initialize`、`notifications/initialized`、`tools/list` 与 `shutdown`，未发送任何
`tools/call`：

```text
diagnostic PID：23872
elapsed：26 ms
exit code：1
stdout：empty
stderr：
  gateway.pipe.client_connect_failed:
  Failed to connect to the local Editor Named Pipe:
  所有的管道范例都在使用中。 (os error 231)
existing Codex-owned Gateway PID：19776
project write：none
mutation/rollback/approval：0
```

代码复核确认 `CreateNamedPipeW` 最大实例数为 `1`，且 accept owner 在一个 peer 的整个
`run_connection` 生命周期内不 re-arm。现有测试只覆盖单客户端与顺序 reconnect，没有覆盖
两个 Gateway 同时保持连接。该结果分类为 implementation/design coverage gap，不再归因为
已证明的 Codex app transport defect。

### 28.2 修复 Gate

```text
C7-R1-A red：
  新增两个 GatewayRemoteAdapter 同时连接同一 Editor pipe 的测试。
  第一个保持连接时，第二个必须完成 Connect + Catalog，不得 ERROR_PIPE_BUSY。

C7-R1-B implementation：
  CreateNamedPipeW 使用多实例容量。
  accept owner 在连接建立后把 peer 移交 owned worker，并立即 re-arm。
  每个 worker 独立 session；请求仍只通过 GatewayOwnerThreadDispatcher 访问 Editor owner。
  shutdown 有界取消并 join accept 与全部 connection worker。

C7-R1-C regression：
  并发双客户端测试通过。
  named_pipe 模块测试通过。
  cargo test -p ai_tool_gateway 通过。
```

失败时只修当前 owner，不进入 production replacement、真实 config migration、C7 typed
mutation 或 Local CI。源码和测试通过后必须先同步新的 source/binary identity，再请求下一
个 production replacement 授权。

### 28.3 执行结果

```text
red command：
  cargo test -p ai_tool_gateway
    named_pipe::tests::named_pipe_server_accepts_concurrent_clients_and_isolates_peer_close
    -- --exact --nocapture
red result：
  gateway.pipe.client_connect_failed
  所有的管道范例都在使用中。 (os error 231)

implementation：
  CreateNamedPipeW max instances -> PIPE_UNLIMITED_INSTANCES
  accept 后移交 per-connection owned worker 并立即 re-arm
  peer session 继续由同一个 GatewayOwnerThreadDispatcher 串行拥有
  ERROR_PIPE_BUSY 使用 WaitNamedPipeW 做 5 秒总期限内的短等待重试
  shutdown 取消 accept/worker synchronous IO，并 join 全部 owned workers
  shutdown session close 入队并唤醒 owner，避免 owner-thread shutdown 互锁

targeted owner tests：
  named_pipe::tests::* = 9 passed
stress：
  concurrent clients + peer close isolation = 20/20 passed
  active worker shutdown = 20/20 passed
affected regression：
  cargo test -p ai_tool_gateway
  lib 64、core scope 3、deep mutation 5、discovery 6、reconnect 1、
  MCP process 4、rollback 3、stable install 4，全部 passed
  doc tests 0 failed
format：
  cargo fmt --all -- --check passed
existing warnings：
  engine_runtime 3 个既有 dead-code warning，未新增 suppression
production state：
  Editor PID 7500 未替换
  stable installed MCP PID 19776 / binary 未替换
  Codex config 未修改
  disposable project 未修改
  C7 typed call/mutation/rollback/approval = 0
```

C7-R1 source/test Gate 已完成。下一步不是直接执行 C7，而是单独授权 production Editor 与
stable MCP replacement；替换后先重复 read-only 双连接 handshake，证明第二 Gateway 不再
出现 error 231，再关闭旧任务并在新任务单独授权 C7。

## 29. C7-R1 production Editor/MCP replacement

用户单独授权 `production Editor/MCP replacement`。该授权只覆盖 production binary
替换、Editor 重启与 read-only MCP dual connection，不覆盖 Codex config、C7 mutation、
rollback、approval 或 Local CI。

### 29.1 Transaction 与身份

```text
source commit：3a0eef5a8a30162ca9a060ef046fb4d3219ae80e
source tree：2a590fc13a2ebc39ad0f04937a98562c64621e8d
worktree before build：clean
release MCP build：
  cargo build --locked --release -p ai_tool_gateway --bin ai_engine_gateway_mcp
production Editor build：
  cargo build --locked -p editor_host --bin editor_host
    --features real-window,real-wgpu-surface
build result：passed
existing warnings：engine_runtime 3 个既有 dead-code warning

old Editor PID：7500
old Editor sha256：3d9ae8c3d829fbf552a856467b54ddc54775bbb8a525713be2b26307f4533c09
old stable MCP sha256：a2b129f1b4ddef16adc39dd0a600d283a9f9d594b6f8d44b8976296c7bbb3a47
new Editor PID：17312
new Editor sha256：0c3da786a1835b1c9b60a4d671f2b476716f7c86bce84d2cab1337de6a59033b
new stable MCP sha256：1bb80d88ece1320e1d6857df742c25da0f167c4f372f17fe2afcfa667930538a
Codex config sha256 before/after：
  4aad60a193727cba68a97840872053f8259235dabbb402681e0314ffe3ac272c
evidence/rollback root：
  C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\Gateway\replacement\
  1785224364062-c7-r1
```

Stable MCP 使用同卷 staged file + `File.Replace`；evidence root 保留旧/新 Editor 与旧/新
MCP 的 byte-exact 副本。首次尝试复用旧 isolated run root 时，新的 Editor 按既有合同拒绝
非空 `picker-start`；事务立即恢复旧 binary。随后使用 fresh role-owned replacement root
启动新 Editor：

```text
G:\AifeC7Runs\run-1785224364062-replacement
```

该 root 启动时只有空 `picker-start` 且不存在 `state`，不复用或修改旧 run state。原
disposable project 保留在原路径，当前新 Editor 停留在 Launcher，尚未打开该项目。

### 29.2 Production read-only dual connection

两个进程均使用真实 installed binary：

```text
C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\bin\
ai_engine_gateway_mcp.exe
```

只发送 `initialize`、`notifications/initialized`、`tools/list` 与 `shutdown`，未发送
`tools/call`：

```text
client 1 PID：14252
client 2 PID：23072
both held concurrently：true
protocolVersion：2025-06-18 / 2025-06-18
tools/list：21 / 21
shutdown response：passed / passed
exit code：0 / 0
stderr：empty / empty
tools/call：0
error 231：not observed
```

### 29.3 Final audit

```text
production Editor PID：17312
discovery schema：ai-tool-gateway-discovery.v2
discovery editorProcessId：17312
live diagnostic MCP process after shutdown：0
staged replacement file retained：false
Codex config changed：false
disposable project file count：17
disposable project bytes：1212110
disposable project aggregate digest before/after：
  22087393958a5b036eb6460bc8ad04724f88e6ad18525443e2a51e86099e5c56
project changed：false
C7 typed call/mutation/rollback/approval：0
Local CI：not run
```

C7-R1 production replacement 与双连接资格验证 passed。当前授权已消费并关闭；不得在本
任务直接进入 C7。下一步必须在新任务通过真实 Editor Launcher 重新打开保留的 disposable
project，复核 Editor/stable MCP/project identity 后，再单独授权窗口 G / C7。

## 30. 2026-07-28 Window G / C7 terminal failure

### 30.1 进入身份与 typed read

```text
verification task：new Codex task
operation start：2026-07-28 16:22:46.163 +08:00
operation end：2026-07-28 16:29:52.272 +08:00
operation duration：426109 ms
source HEAD：e504c523b7c49b51c310edf37ec786c0cac0182e
product source since C7-R1 commit 3a0eef5：Rust changes none；docs only
production Editor PID：32312
production Editor path：G:\gameEngin\rust\target\debug\editor_host.exe
production Editor sha256：
  0c3da786a1835b1c9b60a4d671f2b476716f7c86bce84d2cab1337de6a59033b
stable MCP path：
  C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\bin\ai_engine_gateway_mcp.exe
stable MCP sha256：
  1bb80d88ece1320e1d6857df742c25da0f167c4f372f17fe2afcfa667930538a
Codex config command match：true
Codex config sha256：
  4aad60a193727cba68a97840872053f8259235dabbb402681e0314ffe3ac272c
discovery schema：ai-tool-gateway-discovery.v2
discovery editorProcessId：32312
typed tools delivered：21
aife_status：passed
aife_catalog：passed
project.inspect：passed
project root：
  G:\AifeC7Runs\run-1785211733308\picker-start\c7-disposable-project
initial project digest：
  sha256:6757c0b0abaf9a069b164beae90bc7f0b7301ea0092ce5469634a961060291d1
initial recordedOperationCount：0
```

### 30.2 唯一 mutation 终态

```text
operationId：tool-op-dbcf275d057556a9ed1ef72e
invocationId：mcp-invocation-1
toolId：project.mutate
change：create disposable Input/c7-qualification.input.json
first state：awaiting_user
terminal state：failed
diagnostic：gateway.operation.project_drifted
message：Project facts changed while the goal mutation awaited approval.
commitStarted：false
changedDomains：[]
recordedOperationCount：0
rollbackRef：none
project.rollback call count：0
mutation retry：forbidden
```

批准决策前约 35 ms，`Scenes/Main.scene.json` 被普通 Editor 场景保存路径重写为当前规范化
JSON（`data` 转为 `fields`、补齐 `parentId` / `mesh`、数值与格式规范化）。该写入不是
C7 ProjectPatch：目标 `Input/c7-qualification.input.json` 不存在，operation 也未开始
commit。漂移后的项目 digest 为：

```text
sha256:2647529fb9e319bd7050c9c9cfc68210f38cb26614c06f0ac7b22ee99b0e6b0b
```

### 30.3 终态边界

```text
Window G / C7 result：terminal failed
failure class：external project drift / production behavior
259 completion eligibility：false
current authorization：none
mutation authority：revoked
project.mutate apply count：0
project.rollback count：0
Local CI：not run
completion record：not generated
construction archive：not performed
retained Editor PID：32312
retained disposable root：
  G:\AifeC7Runs\run-1785211733308
next：
  diagnose scene-save drift owner and approval precondition interaction
  revise this construction document before any new production attempt
forbidden：
  retrying the terminal mutation
  issuing a replacement mutation in a new task
  calling project.rollback without rollbackRef
  deleting the retained failure evidence
  Local CI
```

本次 mutation 是施工文档定义的真实一次性尝试。按第 15 节
`real mutation terminal failed -> 不重试 mutation，回到 owner 诊断` 执行；不能把
`nextAction` 中通用的 fresh approval 建议解释为本施工的重试授权。

## 31. C7-R2：Scene Clean Save / Approval Drift Remediation

### 31.1 状态、诊断与方案绑定

```text
生成日期：2026-07-28
用户方案确认：已确认 clean-save no-write，Gateway drift fail-closed 保持不变
正式方案修订：第 28 节
切片状态：已生成、已自审、未授权施工
当前授权：无
原 operation：tool-op-dbcf275d057556a9ed1ef72e，terminal failed，永久禁止重试
原 failure fixture：保留，只读证据，不作为新 qualification 输入
Local CI：不属于默认 Gate
```

owner 复现已经证明：

```text
legacy Scene clean first save：
  sha256:6757c0b0abaf9a069b164beae90bc7f0b7301ea0092ce5469634a961060291d1
  ->
  sha256:2647529fb9e319bd7050c9c9cfc68210f38cb26614c06f0ac7b22ee99b0e6b0b
second save：digest unchanged
production before/after digest：与复现逐字相同
Gateway drift owner contract：passed
approval command source inspection：不显式调用 SaveSceneDocument
persisted command trace：不存在，无法证明具体 UI Save 触发来源
```

失败分类从泛化的 `external project drift / production behavior` 收敛为：

```text
primary owner：SceneSavePipeline clean same-path save
interaction：raw-byte whole-project digest + approval fail-closed recheck
Gateway behavior：correct by current contract
trigger attribution：unproven
```

### 31.2 施工目标

1. clean same-path Scene save 成功但不写盘，保持 bytes、mtime 与 project digest。
2. 真实 dirty save 和 Save As 行为保持可预测、可审计。
3. `Saved` 与 `Unchanged` 明确区分真实写入和成功 no-op。
4. Gateway 继续对任何真实 project drift fail closed。
5. 为 clean save 与 awaiting approval 的交叉行为建立 red-capable regression。
6. 不恢复或重放已经 terminal failed 的 C7 operation。

### 31.3 允许涉及文件

核心 owner：

```text
rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/services/scene_service.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/editor_core/src/tests/scene_save_digest_contract_tests.rs（允许新增）
```

只有 red-capable test 或 consumer inventory 证明需要时：

```text
rust/crates/editor_core/src/scene_editing/tests.rs
rust/crates/editor_core/src/scene_editing/document.rs
rust/crates/ai_tool_gateway/tests/deep_mutation_contract.rs
rust/crates/editor_window_winit/src/tests/gateway_goal_mutation.rs
rust/crates/editor_window_winit/src/application.rs（仅测试证明 approval dispatch 有隐式 Save）
```

禁止：

```text
修改 CandidateProjectRevisionStore raw-byte digest
放宽 Gateway approval/project drift recheck
增加 Editor-owned write bypass 或自动 fresh approval
修改 samples/** 以预先规范化 fixture
修改保留的 C7 failure project
重试原 mutation、调用无 rollbackRef 的 rollback
修改 Codex config 或 stable installation
运行 production mutation、production replacement 或 Local CI
```

### 31.4 C7-R2-A：source/test 修复窗口

该窗口必须由用户单独授权；保守估计小于 3 小时。授权只覆盖第 31.3 节 source/test 文件、
仓库外 test-owned fixture、定向 owner/consumer tests 和受影响 crate 回归。

#### Gate R2-A0：红测与 consumer inventory

先新增并运行期望不变量测试，必须在未修改产品实现时捕获当前失败：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core `
  tests::scene_save_digest_contract_tests::clean_same_path_save_preserves_legacy_scene_bytes_mtime_and_project_digest `
  -- --exact --test-threads=1 --nocapture
```

同时记录：

```text
loaded document dirty=false
save target equals current path
before/after scene bytes and project digest
first failing assertion
SceneSaveStatus / transaction write_set consumers
```

不得使用 production C7 project 作为 fixture；测试复制 `samples/switch_puzzle_project` 到
repository-external test-owned 临时根。失败证据足够后才进入实现。

#### Gate R2-A1：Scene save owner contract

在 `SceneSavePipeline::save_in_scope` 实现：

```text
resolve target + containment/validation
same current path + clean + target exists -> Unchanged, no serialization/write
otherwise serialize
existing target bytes equal serialized bytes -> clear dirty, Unchanged, no replace
different bytes or missing target -> atomic write, clear dirty, Saved
explicit different Save As -> write target
```

`SceneSaveStatus` 增加 `Unchanged`。不得通过修改 serializer alias/default/format来隐藏问题；
兼容读取和 canonical serialization 保持现有合同。

定向 owner tests 至少覆盖：

```text
legacy clean same-path bytes/mtime/digest invariant
modern canonical clean same-path invariant
dirty different bytes -> Saved
dirty equal bytes -> Unchanged + dirty cleared
clean Save As different path -> Saved + target exists
clean current target missing -> Saved + recreated
save validation/containment failure unchanged
second save idempotent
```

Gate 命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core scene_save_digest_contract_tests -- --test-threads=1 --nocapture
cargo test -p editor_core scene_editing::tests -- --test-threads=1
```

#### Gate R2-A2：EditorSession 与 Play consumer

`save_scene_document` 只在 `Saved` 时声明
`editor_scene_document.file` write-set 和 `editor.scene_document.saved`；`Unchanged` 返回
Committed，但使用独立 no-change diagnostic，不声称写盘。`Failed` 保持现有失败合同。

Play autosave 继续只为 dirty Scene 调用 save；`Saved` 和 `Unchanged` 均为成功，
`Failed` 阻止 Play。不得新增 autosave 或改变 Play/RuntimePackage 边界。

Gate 命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core scene_service_tests -- --test-threads=1
cargo test -p editor_core play_service_tests -- --test-threads=1
```

#### Gate R2-A3：approval drift 交叉合同

增加两个相反方向的集成测试：

```text
awaiting mutation + legacy clean same-path save
  -> project digest unchanged
  -> approval request remains eligible
  -> same operation can continue

awaiting mutation + real dirty Scene save
  -> project digest changes
  -> operation terminal gateway.operation.project_drifted
  -> commitStarted=false for the pending mutation
```

同时保留 approval command dispatch 不隐式产生 `SaveSceneDocument` 的 reachability 断言；若
现有 source/test seam 已足以证明，不为此修改 production application。

Gate 命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p ai_tool_gateway --test deep_mutation_contract `
  clean_scene_save_preserves_pending_approval_but_dirty_save_terminalizes_drift `
  -- --exact --test-threads=1 --nocapture
cargo test -p editor_window_winit gateway_goal_mutation -- --test-threads=1
```

#### Gate R2-A4：受影响闭包与窗口关闭

只有 A0-A3 全绿后运行一次受影响闭包，不重复同配置下已被完整 suite 覆盖的 filtered tests：

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --all -- --check
cargo test -p editor_core
cargo test -p ai_tool_gateway
cargo test -p editor_window_winit
```

`project_e2e_gate` 仅在 consumer inventory 证明其编译或行为消费 `SceneSaveStatus` 时加入；
Local CI、production build、stable install 和真实 Editor 不属于 R2-A。

窗口结束时记录 source/tree、diff、测试计数、首因/重跑、temp cleanup 和未运行项，然后停止。
不得在同一授权中自动进入 R2-B。

### 31.5 C7-R2-B：production replacement + read-only qualification

只有 R2-A 完成、产品源码冻结为 clean exact commit 后，才可由用户单独授权 R2-B。该窗口
只覆盖：

```text
按实际受影响产物构建 production Editor
若 ai_tool_gateway 产品源码/依赖未变化，则复用现有 stable MCP，不机械重建/替换
事务化替换实际发生变化的 production binary
重新打开全新的 run-owned disposable project
read-only 验证 Editor/discovery/stable MCP/project identity
验证 clean Save 命令前后 bytes/mtime/project digest 不变
```

R2-B 禁止 Codex config migration、typed `project.mutate`、approval、rollback 和 Local CI。
保留的失败 project 仅作证据，不重新打开作为 qualification project。若 production
replacement 或 read-only invariant 失败，停止并回到 owner 诊断，不进入 R2-C。

### 31.6 C7-R2-C：fresh qualification

R2-C 不是原 C7 mutation 的 retry。只有 R2-B passed 后，必须在新的 Codex task、全新的
run root、全新的 disposable project 和新的用户明确授权下执行。进入条件：

```text
R2-A clean exact commit 与 affected regression passed
R2-B production identity/read-only clean-save invariant passed
stable MCP typed tools 来自已核对的 installed executable
new project initial digest captured
old operation id / old project / old approval / old grant 均未复用
```

最小 typed sequence 仍遵守第 14 节，但使用新的 qualification identity。任何 mutation
terminal failure仍不重试。成功后必须 rollback 新 operation 返回的 `rollbackRef`，并证明
新 disposable project 恢复其初始 digest，才可完成 259。

### 31.7 文档同步与归档

R2-A 通过只更新 source/test 结果和下一授权入口，不生成 259 完成记录。R2-B 通过只记录
production identity/read-only evidence。只有 R2-C 完成并恢复新 project 初始 digest 后：

1. 生成 259 阶段完成记录。
2. 更新 README、00 文档地图、00 阅读顺序、49、54、施工文档 README 和完成记录 README。
3. 将本施工文档从 `当前/` 移到 `已完成/`。
4. 报告新 operation/receipt/rollback lineage，并保留旧 C7 terminal failure 记录。

### 31.8 施工自审

```text
result：passed
formal scheme：第 28 节已同步，无冲突
scope expansion：none
owner：SceneSavePipeline / EditorSession save consumers
Gateway weakening：none
public tool change：none
new module：none
red-capable requirement：A0 + A3
affected consumer closure：editor_core / ai_tool_gateway / editor_window_winit
duplicate validation：filtered tests 只用于开发反馈，A4 suite 后不重复
production mutation authorization：not granted
Local CI authorization：not granted
old terminal mutation retry：forbidden
external failure fixture deletion：forbidden
three-hour split：A/B/C 独立窗口
current executable status：不可施工，等待用户单独授权 C7-R2-A
```

本切片没有把 Scene 格式兼容或 Gateway 安全判断扩成新 Module；修复位于产生无效写入的
owner。它也没有用语义 digest、Editor write bypass 或自动重新审批掩盖真实 drift。

## 32. 2026-07-28 C7-R2-A source/test 执行结果

### 32.1 授权与时间

```text
用户授权：C7-R2-A、C7-R2-B、C7-R2-C，要求直接按序执行
A start：2026-07-28T17:30:21.4603288+08:00
source baseline：e504c523b7c49b51c310edf37ec786c0cac0182e
source baseline tree：3a0eef5a8a30162ca9a060ef046fb4d3219ae80e
Local CI：未授权、未运行
production state during A：未修改
```

三个窗口保持独立进入条件。用户一次明确授权不改变 A failed 不进入 B、B failed 不进入 C
的 fail-fast 边界，也不允许复用原 operation、approval、grant 或失败项目。

### 32.2 A0 红测

新增：

```text
rust/crates/editor_core/src/tests/scene_save_digest_contract_tests.rs
clean_same_path_save_preserves_legacy_scene_bytes_mtime_and_project_digest
```

产品实现修改前首次结果：

```text
FAILED：before_bytes != after_bytes
loaded dirty：false
before bytes：2331
after bytes：3156
before digest：
  sha256:6757c0b0abaf9a069b164beae90bc7f0b7301ea0092ce5469634a961060291d1
expected production after digest：
  sha256:2647529fb9e319bd7050c9c9cfc68210f38cb26614c06f0ac7b22ee99b0e6b0b
classification：expected red；SceneSavePipeline clean same-path write owner
```

### 32.3 实现

```text
SceneSaveStatus：Saved / Unchanged / Failed
clean same current path + existing target：Unchanged，零 serialize/write
serialized bytes equal current bytes：clear dirty + Unchanged，零 replace
different/missing target：atomic write + Saved
explicit different Save As：创建目标
EditorSession write_set：只在 Saved 声明 file write
Play autosave：Saved / Unchanged 均成功，Failed 阻止 Play
Gateway production code：unchanged
Gateway raw-byte digest/drift policy：unchanged
```

### 32.4 owner 与交叉合同

```text
scene_save_digest_contract_tests：6 passed
scene_editing::tests：38 passed
scene_service_tests：32 passed
play_service_tests：11 passed
deep mutation clean/dirty approval cross-contract：1 passed
Native Editor approval consumer：1 passed
```

Gateway 交叉测试首次运行在 clean fixture Save 处得到
`scene.save.path_outside_project`。首因是测试使用 TEMP 原始路径而不是
`active_project_session.project_root` 的规范化根；产品 owner 未修改。fixture 改为使用 active
project root 后，同一测试通过：

```text
clean save：digest unchanged，approval eligible，same operation completed
dirty save：digest changed，operation gateway.operation.project_drifted
dirty mutation target：not created
```

### 32.5 A4 受影响闭包

```text
cargo fmt --all -- --check：passed
cargo test -p editor_core：
  565 passed / 0 failed / 2 ignored
cargo test -p ai_tool_gateway：
  lib 64 passed
  core scope 3 passed
  deep mutation 6 passed
  discovery 6 passed
  reconnect 1 passed
  MCP process 4 passed
  rollback 3 passed
  stable install 4 passed
  doc tests 0 failed
cargo test -p editor_window_winit：
  142 passed / 0 failed / 0 ignored
```

两次过短 runner timeout 分别在 5 秒与 120 秒终止 `editor_core` 完整命令；没有测试 assertion
失败。使用 600 秒正常预算重新执行后完整 suite 通过。既有 warning 保持
`engine_runtime` 3 个 dead-code warning 和 `editor_window_winit` 1 个 test-only
dead-code warning，没有新增 suppression。

### 32.6 A 窗口关闭

```text
result：passed
scope expansion：none
product source changed：editor_core Scene save owner/consumers
test-only changed：editor_core owner contract + ai_tool_gateway cross-contract
production binary changed by A：none
stable MCP changed：none
Codex config changed：none
old failure project changed：none
old mutation retried：false
next：freeze exact source commit；进入已授权 C7-R2-B
```

## 33. C7-R2-B production replacement / read-only 结果

```text
result：passed
role root：G:\AifeC7Runs\run-1785232897624-c7-r2-b
source commit：f92962c5632e2d41cc2f125d7470290d56b93b77
source tree：22d47aabef6d04429ff2938ce61f9f3114d1bc83
old Editor sha256：
  0c3da786a1835b1c9b60a4d671f2b476716f7c86bce84d2cab1337de6a59033b
new Editor sha256：
  1c4dbff6402c0be287f11e8b7b984cbd90ca6120e022fc0e71fd8923259bff15
stable MCP sha256：
  1bb80d88ece1320e1d6857df742c25da0f167c4f372f17fe2afcfa667930538a
```

旧 production Editor 由 PID `32312` 正常关闭；旧 binary 已保存到 B role-owned rollback
目录，并通过 `File.Replace` 事务化替换。新 Editor PID `17832` 以同一 production path 和
`--isolated-project-launch-root` 启动，经真实 Launcher / Open Project 打开全新的
`c7-r2-b-project`。

clean Save 前后：

```text
Scene bytes：2331 -> 2331
Scene sha256：
  c669aa1aed5c411f1457231d8020841426b3bcc7548a51c4d7eaf501297c9f51
  -> same
Scene mtime UTC：
  2026-07-11T09:54:46.6105713Z -> same
project file count：17 -> 17
project aggregate digest：
  cebbfc0a5fe1c39c1179b36ed2a88698b558dbb86d711d9ded2ee12b5baee237
  -> same
Gateway project digest：
  sha256:6757c0b0abaf9a069b164beae90bc7f0b7301ea0092ce5469634a961060291d1
  -> same
```

fresh installed MCP read-only client 返回 21 tools、exit 0、stderr empty，绑定
`editor-pid-17832`，read active、mutation not_requested、reconnectRequired=false、
recordedOperationCount=0。B 未调用 mutation、approval 或 rollback。

## 34. C7-R2-C fresh qualification 结果

setup task `019fa84f-9bfd-7e43-8b60-b2d61b90de4d` 建立全新 run
`G:\AifeC7Runs\run-1785235405520-c7-r2-c`，重启 production Editor 为 PID `8572`，并通过
真实 Launcher 打开全新 project
`G:\AifeC7Runs\run-1785235405520-c7-r2-c\picker-start\c7-r2-c-project`。该 setup task 的
MCP 在 Editor 重启前已绑定旧 PID，首次 status 只得到 reconnect_required；没有 operation、
grant、approval 或 mutation。随后创建真正 qualification task
`019fa86e-1f17-7773-ba64-fe1d4c4629c4`，其 fresh MCP session 为
`gateway-session-62fac77e97d54825bf5af4af285487f9`。

```text
Editor PID：8572
Editor instance：
  sha256:d853c435b1dbaa06385bb2d088c9ada3a3b9576ae39e11af7269ecb191c314b5
Editor sha256：
  1c4dbff6402c0be287f11e8b7b984cbd90ca6120e022fc0e71fd8923259bff15
stable MCP sha256：
  1bb80d88ece1320e1d6857df742c25da0f167c4f372f17fe2afcfa667930538a
catalog：21 tools
catalog digest：
  sha256:5b6cc849fc024f32c15b0f8548200d64619f4570eff8bacc933ea904d1f7ba01
availability digest：
  sha256:efdd62009bd6f17b80cd2f2cdefd5333f175db1119c9c129ec402fbd746e955f
initial recordedOperationCount：0
initial digest：
  sha256:6757c0b0abaf9a069b164beae90bc7f0b7301ea0092ce5469634a961060291d1
```

唯一 mutation：

```text
operation：tool-op-8b564d1e8f4063cc79cfc746
invocation：mcp-invocation-1
goal/candidate：goal-mutation-6c65983db1333497737ad18e0168d843
grant：
  sha256:531b87bf332895ea02ed9a00f3cba545c79c875f17bf1d954d0acdc5ee913bd5
Native approval：operation suffix c79cfc746 matched；Editor OK accepted
created path：Input/c7-r2-c-qualification.input.json
changed digest：
  sha256:7ce63ab072d67e67068f2d40e2c8a6976b8e8ca9e7649a2a7632e811f8bf12d3
candidate digest：
  sha256:99c96565b6cfdba0e3f6ce06143ac334bd914b64693dfa56e277b7547e9f70aa
receipt：receipt-tool-op-8b564d1e8f4063cc79cfc746
receipt digest：
  sha256:b28f57f229d73665ae68fdafd6be0a65678a96ed74bed1299bbcc35a814f1673
rollbackRef：
  rbk_50fe7d9f72fc875bd77546291f8a434ec5680c5f6bde5e2d3ac2f050f3f6797f
```

exact rollback：

```text
operation：tool-op-a26329d10941985bd8bbc421
invocation：mcp-invocation-2
receipt：rollback-tool-op-a26329d10941985bd8bbc421
receipt digest：
  sha256:5b71556a49131377ab5402ea1d01af5345b1b4ef93d28f6018b43b7a593297f5
restored digest：
  sha256:6757c0b0abaf9a069b164beae90bc7f0b7301ea0092ce5469634a961060291d1
final recordedOperationCount：2
qualification file exists：false
```

Gateway durable evidence 为 `Library/AiCapability/tool-kernel-journal.json`；Native approval
pending/cleared captures 已保存在 qualification task outputs。C 创建 task 期间 Codex config
整体文件 hash 由 B 基线 `4aad60…` 变化为 `3e643e…`，但
`mcp_servers.ai_first_game_engine.command` 仍逐字指向同一 stable installed MCP；没有执行
config migration 或 stable install replacement。

## 35. 施工完成与归档

```text
result：passed
C7-R2-A：passed
C7-R2-B：passed
C7-R2-C：passed
old terminal operation retried：false
Local CI：not run
production Editor retained：PID 8572
completion record：
  阶段完成记录/2026-07-28-External-Codex-Authoring-Readiness-v1/00-总览.md
archive：
  施工文档/已完成/259-当前可自动化施工文档-External-Codex-Authoring-Readiness-v1.md
```
