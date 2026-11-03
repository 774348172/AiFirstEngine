# 256 当前可自动化施工文档：Minimal Project Create Tool v1

## 0. 文档状态

```text
正式方案：256-Project-Create-Tool-v1方案.md
文档状态：已生成、已自审、已执行 F0、已转历史
施工状态：deferred_not_cost_effective
授权来源：2026-07-25 用户明确要求“根据方案生成施工文档，然后按照规则直接开始施工”
授权范围：本施工文档；先执行 F0，只有 F0 通过才允许进入功能施工
激活基线 commit：b90b39f3c00cb1f88c7cb2e52ada1a68a8c291ba
激活基线 tree：eb12d009063add5716720471017f8ebbf29c9558
Rust toolchain：1.96.0
绝对时限：自本次施工授权开始不超过 3 小时
```

本施工只实现一个极简 goal-level typed Tool：

```text
project.create(requestedProjectRoot, projectName)
```

它复用既有 Gateway、Tool Registry、Catalog、operation identity 和 Native Editor
CreateProject owner。任何阶段都不得通过恢复旧 256 实现或新增生命周期基础设施来使
施工通过。

## 1. 目标

1. launcher/no-project 状态可发现并执行 `project.create`。
2. direct input 精确为 `requestedProjectRoot + projectName`，拒绝 unknown fields。
3. 目标已存在时零写入；父目录必须存在；目标由本次 invocation 排他创建。
4. 创建失败或打开失败时，只清理由本次 invocation 可证明拥有且未被外部接管的目标。
5. 成功时当前 Native Editor 已打开新项目，并返回 owner 事实产生的 identity/digest。
6. 复用既有 request/invocation/operation replay，不公开 caller-owned idempotency。
7. 255 Catalog 正确表达 launcher `ready`、project-open `blocked`、未装配
   `unsupported`。

## 2. 绝对停止线

以下任一项成为实现前置时，F0 直接以
`deferred_not_cost_effective` 收敛，禁止进入 F1：

```text
新 Editor-scoped Session lifecycle Module
新 launcher/rebind/reconnect protocol
Codex production config 或 activation 修改
新 mutation Grant / approval lifecycle
新 candidate / freezer / qualification / activation
新 256 专用 Runner 或永久 Gate
AI-facing reconnect/discovery 菜谱
恢复旧 project.open、request_mutation_access 或旧 P8 lineage
```

F0 后任一阶段发现停止线同样立即收敛。不得把测试工具、一次性 helper 或文档措辞
伪装成例外。

## 3. 非目标

```text
不实现 project.open
不在 project-open 状态切换项目
不提供 templateId、dry-run、public cancellation 或 idempotencyKey
不申请 mutation access
不执行 Preview、Build、Export、Delivery Verify 或 rollback
不建立 Workflow、Planner、nextToolIds 或 Tool 外步骤
不建立专用 production smoke Runner
不运行旧 P8/P8Q/P8F/P8A/P9/P10
不运行 254-R1/R2 candidate、F-A/F-B/G 或三引擎 B 通道
```

## 4. 基线与证据规则

### 4.1 精确基线

开始 F0 前必须记录：

```text
source commit / tree
git status
rustc / cargo version
G: 与 TEMP 所在卷剩余空间
canonical lint ledger diff
外部 clean exact-commit worktree 路径
隔离 CARGO_TARGET_DIR / TEMP / TMP / artifact root
```

canonical ledger 命令：

```powershell
cd G:\gameEngin\rust
cargo run -p quality_gate --locked -- propose-ledger
```

只接受：

```text
previous_ledger_readable = true
added_diagnostics = []
removed_diagnostics = []
changed_diagnostics = []
new_suppressions = []
expired_entries = []
```

### 4.2 外部 clean worktree

在仓库外创建本次施工独占、精确 commit 的 detached worktree。目录名必须包含
`256-project-create-f0` 和 commit short id。不得复用历史 candidate、target、TEMP
或 artifact。删除或回收前必须验证解析后的绝对路径位于本次 worktree 根下。

### 4.3 失败纪律

- red contract 必须因预期能力缺失失败，不能因环境、fixture 或编译污染失败。
- 同一个授权阶段不通过“重跑”改变结论；修复后只执行文档明确允许的定向验证。
- 权威环境等价预检若失败，冻结该失败证据并停止；不得自动重新开一次权威运行。
- 任一命令累计接近三小时时，立即停止并记录实际状态，不以长等待跨越时限。

## 5. F0：低成本可行性门

F0 只允许 baseline、只读 inventory、现有测试/编译和必要的最小 compile-time red
contract；在 F0 通过前禁止修改产品源码。

### 5.1 问题 A：no-project Gateway composition

确认现有 production Editor-hosted Gateway 是否在 `active_project_session = None` 时仍有
可用 host/session owner，并能把一个 Registry Tool 投影到真实 typed MCP。必须沿现有
composition，不得新增 launcher bootstrap、rebind、reconnect 或第二 Gateway。

通过条件：

```text
现有 Editor host 生命周期覆盖 launcher/no-project
Catalog 请求无需 active project 才能到达 Tool Kernel
新增 descriptor/probe/dispatch 可作为现有 composition 的局部扩展
```

### 5.2 问题 B：非 project-bound operation identity

确认既有 invocation/operation registry 能让 `project.create` 在创建前没有
`projectDigest` 的情况下获得 engine-owned replay identity。允许把“所有 invocation
都 project-bound”的错误共性抽象局部泛化为 tool-specific binding；不得新建 Session
lifecycle、第二 operation registry 或 caller idempotency。

通过条件：

```text
project.create 可复用既有 invocationId/operationId/replay storage
其他 project-bound Tool 的 expectedProjectDigest 验证保持不变
public direct input 不增加 identity/digest/internal field
```

### 5.3 问题 C：Editor owner 局部安全加固

确认 `ProjectLauncherState::create_project` 与 `EditorSession::create_project` 可在原 owner
内完成：

```text
parent exists
target absent
exclusive target create
初始化/打开失败后的 owned cleanup
cleanup ownership 不确定时报告 cleanup_failed
```

不得新建 staging/freezer/rollback Module。若现有同步调用无法在 owner 内证明安全，
判定不通过。

### 5.4 问题 D：真实 production typed MCP smoke

确认已有 `ai_tool_gateway` production MCP process/Editor host smoke 基础设施可以增加一个
launcher/no-project 场景，而无需新 binary、Runner、config activation、reconnect recipe
或长期 Gate。测试可以使用既有 test harness，但不得把测试 client 当作真实
production composition 的替代品。

### 5.5 F0 退出

输出一份施工文档内的 F0 判定表，四项均须为 `pass`，并写明实际 owner/file/symbol
证据。任何一项为 `fail`：

```text
施工终态：deferred_not_cost_effective
产品源码变化：0
后续 F1-F6：not executed
```

## 6. F1：最小红合同

仅 F0 通过后执行。先增加最小、去重的红测试：

1. request/result serde：精确两字段、unknown field 与 caller-owned internal field 拒绝。
2. Catalog：launcher ready、project-open blocked、implementation unsupported。
3. owner：existing target zero-write、missing parent、exclusive claim、owned cleanup、
   cleanup failure。
4. Kernel/Gateway：no-project invocation、既有 identity replay、不重复创建。
5. production composition smoke 的最小红断言，复用已有测试入口。

F1 不增加全 workspace 回归，不新建 fixture framework。退出条件是失败原因全部精确指向
缺少的 `project.create` 能力。

## 7. F2：最小实现

按 owner 顺序施工：

1. 在既有 CreateProject owner 内加固 no-overwrite 与 owned cleanup。
2. 在既有 Tool contract registry 增加唯一 descriptor 和 typed input/result。
3. 将 availability probe 泛化为 tool-specific project-state 要求。
4. 将 invocation binding 泛化为 `project.create` 的 pre-project binding，同时保持现有
   Tool 的 project digest 合同。
5. 通过既有 Gateway/MCP Adapter 路由到 Editor owner。
6. 由 active project owner 生成成功结果 identity/digest/receipt；Adapter 不读写骨架。

每个改动必须是现有深 Module 的局部责任；不得新增公共 lifecycle Interface。

## 8. F3：定向与受影响域回归

先跑 F1 的定向测试，再只按实际修改面运行：

```powershell
cargo fmt --check
cargo test -p editor_core --lib --locked
cargo test -p ai_tool_gateway --locked
cargo test -p editor_host --locked
cargo test -p editor_window_winit --lib --locked
```

若某 crate 的完整测试已经覆盖相同 filter，不重复运行 filter。只有实际修改涉及其它
crate 才补相应测试，并在本文档记录原因。

随后重新执行 canonical lint producer，ledger 不允许新增或漂移。

## 9. F4：fresh exact-commit 环境等价预检

形成实现 commit 后，从新的仓库外 clean detached worktree 检出精确 commit；隔离
target/TEMP/TMP/artifact，检查空间和 canonical ledger，再运行 F3 中的实际受影响域
矩阵。F4 不是 candidate/freezer/qualification，也不产生激活合同。

失败时冻结一次证据并停止，不自动重跑。

## 10. F5：既有 production typed MCP smoke

仅在 F4 通过后，使用 F0 确认的既有 production composition 运行一次 smoke：

```text
launcher/no-project
-> Catalog project.create = ready
-> typed project.create(exact disposable root, name)
-> exact root exists
-> Native Editor active project matches
-> result identity/digest matches owner facts
-> cleanup disposable project and test-owned state
```

这是普通受影响域 smoke，不是专用 Gate，不建立 candidate、freeze、approval、activation
或 AI 菜谱。若既有基础设施到此不能承载，终态仍为
`deferred_not_cost_effective`，禁止补建 Runner。

## 11. F6：完成与归档

全部通过后：

1. 更新正式方案的施工状态，不改变已确认设计。
2. 同步 `README.md`、`00-文档地图.md`、`49`、`54`、`240` 和
   `施工文档/README.md`。
3. 生成阶段完成记录，包含 commit/tree、测试、ledger、smoke、残留风险和清理结果。
4. 将本文件从 `当前/` 移到 `已完成/`。
5. 确认 `当前/` 不存在第二份活动施工文档。

## 12. 自审

### 12.1 与正式方案一致

通过。公共 Interface 只有 `project.create`，direct input 只有两个 caller-owned 字段；
没有恢复旧三 Tool 或旧施工 lineage。

### 12.2 是否形成 AI 菜谱

没有。F0-F6 是施工治理，不是产品 Tool 调用协议；Tool 成功后不要求 reconnect、
inspect、Preview 或 Build。

### 12.3 是否过度施工

没有。先用 F0 证明四项低成本前提；不成立时以零产品源码变化暂缓。测试只有 contract、
受影响域和一个既有 production smoke，没有新 Runner 或永久 Gate。

### 12.4 是否保护用户数据

是。目标存在即零写入，清理仅限 invocation 可证明拥有的全新目标；不能证明时报告
`cleanup_failed`。

### 12.5 是否限制 AI 能力

没有。AI 自由决定路径、名称和后续动作；Engine 只隐藏骨架创建细节并执行安全边界。

### 12.6 是否满足施工规则

满足。唯一活动文档、精确 clean baseline、canonical ledger、仓库外 isolated worktree、
分层去重测试、失败即停和三小时绝对时限均已写入。F0 通过前禁止产品代码修改。

## 13. F0 实际结论

执行日期：2026-07-25。

### 13.1 基线

```text
commit: b90b39f3c00cb1f88c7cb2e52ada1a68a8c291ba
tree: eb12d009063add5716720471017f8ebbf29c9558
source status: clean
external worktree: G:\gameEngin-worktrees\256-project-create-f0-b90b39f
isolated root: G:\gameEngin-build\256-project-create-f0-b90b39f
rustc/cargo: 1.96.0
G: free before F0: 310310502400 bytes
C: free before F0: 34594365440 bytes
```

canonical ledger：

```text
previous_ledger_readable = true
added_diagnostics = 0
removed_diagnostics = 0
changed_diagnostics = 0
new_suppressions = 0
expired_entries = 0
```

外部 worktree 的 HEAD/tree 与主工作树激活基线完全一致且 clean。F0 收敛后 worktree
已通过 `git worktree remove` 删除；隔离 target/temp/artifacts 均为空并已删除。

### 13.2 四项判定

| 问题 | 结论 | 证据 |
|---|---|---|
| A：no-project Gateway composition | fail | `editor_window_winit/src/application.rs:1044-1062` 在没有 active project 时直接销毁 `gateway_host`；`EditorGatewayHostBinding` 和 discovery record 只接受 project identity/root digest |
| B：非 project-bound operation identity | fail | `ClientHello`、`ClientSessionBinding`、`GatewayRequest`、`AiToolInvocation`、read grant 和 operation snapshot 都强制携带 project identity/digest；`GatewayCore::connect` 首先调用 `current_project_binding(session)?` |
| C：Editor owner 局部安全加固 | pass | `ProjectLauncherState::create_project` 是单一同步 owner，骨架写入均位于同一函数并在 `ProjectWriteScope` 下；可局部改为 parent check + exclusive `create_dir` + 保守 owned cleanup，无需新 Module |
| D：production typed MCP smoke | fail | production MCP 只能解析 project discovery；现有 process smoke 先创建项目，再生成 project-bound discovery。launcher 状态没有 production endpoint 可连接 |

### 13.3 停止线判定

修复 A/B/D 必须至少新增：

```text
launcher/no-project Gateway host identity
launcher discovery publication
non-project ClientHello/ClientSessionBinding/GatewayRequest contract
create 后 launcher binding -> project binding 的 rebind、失效或 reconnect 语义
production MCP 对上述状态转换的处理
```

这不是对现有 Registry/Catalog 的局部扩展，而是新的
launcher/rebind/reconnect protocol，精确命中正式方案第 3.2 节和本文第 2 节停止线。

因此：

```text
施工终态：deferred_not_cost_effective
产品源码修改：0
F1-F6：not executed
旧 256 实现恢复：0
专用 Runner/Gate/candidate：0
```

## 14. 最终状态

```text
F0：terminal fail by formal complexity stop line
F1-F6：not executed
文档：historical / not an executable construction entry
下一步：回到方案讨论，决定放弃 launcher typed create，或明确重开 launcher-level Gateway lifecycle 设计；不得从本文件继续施工
```
