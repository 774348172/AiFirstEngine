# 264 Project Observation Contract / Post-Commit Production Wait C-Compact v1 方案

## 1. 状态与结论

```text
系统编号：264
方案：C-Compact
状态：用户已确认，正式方案已生成并自审
日期：2026-08-02
上游：260 ProjectRuntimeSession；263 ProductionAuthorityScenario
下游：Tower Defense P0-5 Gate G production wait remediation
施工状态：未生成施工文档，不可施工
```

采用 C-Compact：项目通过版本化、schema-first 的 `Project Observation Contract`
主动公开少量稳定业务事实；项目 RuntimeModule 在现有
`ProjectRuntimeSession` seam 实现只读观察 Adapter；引擎只在本帧 runtime
mutation 成功提交后发布一份紧凑 typed snapshot。Production authority 只增加
`projectValueEquals` 条件，以有界真实时间 timeout 重复读取 snapshot，不重放 action。

引擎不认识 `phase / round / combat / augment` 等塔防概念。塔防首版只公开：

```text
tower.phase : string
tower.round : integer
```

普通等待条件留在可热修改的 scenario JSON，不进入 Rust。只有已由项目
Rust 拥有的权威状态需要通过很窄的 Adapter 公开为 snapshot 值。

## 2. 问题与已有证据

fresh Tower Defense Gate G run：

```text
<TOWER_RUN_ROOT>\p0-5-gate-g-20260802-121707
```

权威阻断报告：

```text
<TOWER_RUN_ROOT>\p0-5-gate-g-20260802-121707\evidence\gate-g-blocked-report.json
```

已证实：

1. 第一轮固定等待 `220` runtime frames 实际耗时约 `24.449s`。
2. 第二轮同样等待 `220` frames 超过当前固定 `30s` step timeout。
3. 失败码为 `authority.scenario_step_timeout`，最后观察 runtime frame 为 `274`。
4. recent-project open、Play、真实 GameView OS input 和 AUI exactly-once 已通过；当前首因
   不是动作没有到达项目。
5. 固定帧数只能证明“runtime 曾经推进”，无法证明“战斗已结算并进入下一轮”。

因此不能通过增大帧数、sleep 或 timeout 掩盖。Production wait 必须从机械时间
条件升级为项目公开业务状态条件。

Gate G 同时还有 `1280x720` HUD 重叠和 `1600x900` combat GameView 视觉空白缺口。
264 只解决 production wait 语义，不宣称修复这两个视觉缺口。

## 3. 目标

264 必须完成一条项目无关的真实链路：

```text
project-owned Observation Contract asset
  -> ProjectManifest optional reference
  -> RuntimePackage cooked contract
  -> ProjectRuntimeSession read-only observation Adapter
  -> RuntimeFrame post-commit sampling
  -> active runtime latest typed snapshot
  -> Editor GameView read-only projection
  -> ProductionAuthority projectValueEquals
  -> bounded polling / structured failure report
```

具体目标：

1. 项目作者和 AI 能从单一结构化资产发现可观察 path、类型、含义和可选值域。
2. 项目不公开的内部状态对引擎和 scenario 仍然不可见。
3. snapshot 必须代表成功提交后的稳定状态，不是 tick 前、AUI present 前或未提交候选。
4. 条件 JSON 可热修改；新增普通等待不要新增 Rust condition variant。
5. 等待是只读重复观察，不重放 click、AUI action 或 project command。
6. 旧项目不声明合同时继续工作，不付出每帧项目观察成本。
7. 失败必须足以让人和 AI 区分路径错误、类型错误、项目合同违约、状态未到达和
   session 切换。

## 4. 非目标

264 v1 不包含：

- Observation Catalog 或可视化编辑器。
- ECS 全量反射、任意 component query 或源映射语言。
- `all / any / not / atLeast` 表达式树。
- 任意比较运算符、浮点 epsilon、正则、列表包含或算术。
- stall timeout、observation revision/digest 等第二套时间机制。
- 通用 EventBus、订阅系统、遥测管道或 UI/test DSL。
- action replay、幂等重放或快照回滚。
- 使用 `ProjectUiStateSnapshot` 作为 gameplay completion 真相。
- 修复 AUI 布局、GameView 空白、DPI seam 或塔防视觉矩阵。
- 对局中热替换 RuntimeModule 或 Observation Contract。

## 5. 成熟引擎源码参考

### 5.1 Unity

源码：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\Scripting\WaitUntil.cs
```

`WaitUntil.keepWaiting` 在调度时重复检查 predicate；timeout 计时和 predicate 分离，并明确
区分 game time 和 real time。264 学习“条件驱动 + 有界 timeout”，不照搬任意 C# closure。

### 5.2 Unreal

源码：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Core\Public\Misc\AutomationTest.h
```

`IAutomationLatentCommand::Update()` 以完成布尔值推进队列，`FUntilCommand` 把条件重试和有界
timeout 组合，`ExecuteLatentCommands()` 严格顺序消费 command。264 学习严格顺序和只有当前
wait 完成才进入下一步，不照搬任意 `TFunction` callback。

### 5.3 Godot

源码：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\main\scene_tree.cpp
```

`process_frame` signal 和 `SceneTreeTimer` 分开存在；帧推进和时间到期都不等于项目业务
完成。264 不再把 frame delta 当作战斗结算代理。

### 5.4 综合结论

成熟引擎的共同点是“条件重复检查、步骤严格顺序、timeout 有界”，但它们的任意函数
回调不适合 AI-first schema 和外部用户项目。本引擎需要 typed declarative condition，不是程序内
closure。

## 6. 方案对比与选择

### 6.1 方案 A：增大帧数和 timeout

优点是不增加接口。缺点是仍然不知道业务何时完成，性能、机器负载和轮次内容变化
都会再次打破参数。拒绝。

### 6.2 方案 B：复用 ProjectUiStateSnapshot

优点是表面代码最少。缺点是它是 AUI active binding 驱动的 UI ViewModel，可以晚一帧、可以因
当前文档未绑定而不生成 path，还可以将 phase 转成本地化文本。它会把 gameplay completion
耦合到 UI present。拒绝。

### 6.3 方案 C-full：通用 Observation Catalog + 表达式 DSL

优点是能表达任意复合查询。缺点是合同、catalog、source mapping、表达式解释器、诊断和
编辑器形成新的浅层系统，超过当前真实需求。拒绝。

### 6.4 方案 C-Compact：公开紧凑标量合同 + 单一等值条件

优点：

- 项目无关，塔防概念不进入引擎。
- 新增 scenario 条件只改 JSON。
- 合同可发现、可校验、可生成、可解释。
- 对旧项目是可选空能力。
- 小接口隐藏 post-commit 时序、session 切换、类型校验、timeout 和诊断。

缺点是需要一次性打通 project asset -> RuntimePackage -> session -> Editor authority 的跨层链路。
这是中等首次成本，但每个新 scenario 的边际成本很低。选择 C-Compact。

## 7. Project Observation Contract

### 7.1 项目声明入口

`project.aife.json` 增加可选顶层引用：

```json
{
  "schemaVersion": "aife-project.v2",
  "observationContract": "Observations/project.observations.json"
}
```

这是 `aife-project.v2` 的可选、向后兼容字段；无字段表示项目不公开 runtime observation。
路径必须经 `ProjectRelativePath / SafeProjectPath` 解析，不允许绝对路径、`..`、link 逃逸或
Runtime 扫描项目源目录。

### 7.2 资产 schema

```json
{
  "schemaVersion": "project-observation-contract.v1",
  "contractId": "tower-defense.runtime-observations",
  "observations": [
    {
      "path": "tower.phase",
      "type": "string",
      "description": "Current authoritative match phase",
      "allowedValues": ["organizing", "combat", "augment", "finished"]
    },
    {
      "path": "tower.round",
      "type": "integer",
      "description": "Current one-based round number"
    }
  ]
}
```

合同是项目源真相，Build/Preview 将其校验并 cook 进 RuntimePackage。Runtime 不读项目源文件。

### 7.3 有界约束

v1 约束：

| 项 | 约束 |
|---|---|
| 每项目合同数 | 0 或 1 |
| observations | 最多 64 项 |
| path | 最长 128 bytes，点分段稳定 ID，不允许空段 |
| description | 必填，最长 256 UTF-8 bytes |
| allowedValues | 可选，最多 32 项，每项必须与 type 一致 |
| value type | `bool / integer / number / string` |
| collections / object / null | 不支持 |

path 在同一 contract 内必须唯一。path 是公开稳定 ID；改名是破坏性合同变更，不得用
项目内部 Rust 字段名或实体 index 充当长期 path。

### 7.4 类型和等值语义

- `bool`：精确布尔等值。
- `integer`：有符号 64-bit 范围内精确等值。
- `number`：只允许 finite number，v1 精确数值等值；不提供 epsilon。
- `string`：UTF-8、大小写敏感精确等值。

`integer` 和 `number` 是不同合同类型。NaN、Infinity、容器和项目专用 enum type 不进入
通用引擎 schema；项目 enum 以 `string + allowedValues` 公开。

### 7.5 合同 identity 与重建边界

Cook 产物包含 canonical contract digest。以下变化只使 RuntimePackage/Preview cache 失效：

- description 或 allowedValues 变化。
- contract path/type 声明变化，但 RuntimeModule 实现未变。
- production scenario 的 path/equals/timeout 变化。

它们不得单独改变 ProjectRust AOT digest 或 262 Editor composition identity。如果项目为新 path 修改
RuntimeModule Rust Adapter，该 Rust 源变化按 262 正常触发项目专属 Editor/Player 重建。

## 8. ProjectRuntimeSession 观察 seam

### 8.1 接口形状

在现有 `ProjectRuntimeSession` 上增加一个默认空实现的只读方法，概念形状为：

```rust
pub trait ProjectRuntimeSession: Send {
    fn session_id(&self) -> &str;
    fn handle_aui_actions(...) -> ProjectRuntimeSessionOutput;
    fn fixed_update(...) -> ProjectRuntimeSessionOutput;

    fn observe(
        &self,
        context: ProjectRuntimeObservationContext<'_>,
    ) -> ProjectRuntimeObservationOutput {
        ProjectRuntimeObservationOutput::empty()
    }
}
```

`ProjectRuntimeObservationContext` 只提供：

```text
runtime frame index
fixed-step time facts
read-only World API
cooked ProjectObservationContract
report level
```

`observe` 不得拿到 mutation buffer、Renderer、UiProjection、filesystem、network 或可变 World。
这是只读 Adapter，不是第二个 gameplay tick。

### 8.2 兼容策略

- 无 `observationContract` 的项目使用默认空 observe，行为不变。
- 已声明 contract 的项目必须为每个声明 path 输出且只输出一个同类型值。
- 这是带默认实现的可选 source-level extension，不把无合同项目强制升级为新
  `project-runtime-module` manifest interface version。
- 262 Engine SDK identity 保证项目专属 Editor/Player 在引擎 trait 变化后从受信 source
  重建；不存在 Rust dylib ABI 兼容伪设。

### 8.3 输出校验

引擎而不是 scenario 负责：

- 按 cooked contract 校验 path 集合和 value type。
- 禁止项目输出未声明 path。
- 为 snapshot 盖上引擎拥有的 `runtimeFrame / sessionId / contractId`。
- 在 observe panic、缺 path、多 path 或类型错时产生结构化合同违约诊断。
- 不在正式 runtime 默认写 JSON 文件或保留全量历史。

成功 snapshot 必须是 complete；v1 不支持 partial snapshot。Runtime 创建后、首个成功
post-commit frame 之前可以处于 `notProducedYet`，这不等于缺 path。

## 9. Post-commit snapshot 语义

### 9.1 唯一采样点

每个完成的 runtime fixed frame 按以下顺序执行：

```text
AUI Action Dispatch
  -> ProjectRuntimeSession::handle_aui_actions
  -> deferred mutation prepare + commit

FixedUpdate
  -> ProjectRuntimeSession::fixed_update
  -> deferred mutation prepare + commit
  -> existing project rule fixed-update completion

RuntimeFramePostCommit
  -> ProjectRuntimeSession::observe (read-only)
  -> contract validation
  -> publish one latest snapshot

Update / render / AUI present
```

`RuntimeFramePostCommit` 是内部阶段名，不是新 gameplay schedule 或项目可注册事件。

### 9.2 失败和生命周期

- 本帧任一 session mutation prepare/commit 进入 terminal fault 时，不发布新 snapshot。
- observe 只能看到当前 session 内部状态和已提交 World。
- Pause 不产生新 fixed-frame snapshot；StepFrame 完成一次正式 fixed frame 后产生一份。
- 单纯 redraw/present 不重新执行 observe。
- Stop 立即清除 active snapshot。Restart 或新 Play session 使旧 `sessionId` snapshot 失效。
- 只保留当前 session 最新 snapshot，不保留时间序列。

### 9.3 新鲜度水位

runner 内部记录最近一个可修改 runtime 的 click/action 完成帧。其后的
`projectValueEquals` 只接受：

```text
snapshot.sessionId == active sessionId
snapshot.runtimeFrame > latest mutating action completion frame
```

这个水位不暴露为 scenario `afterStep` DSL。多个顺序 project value wait 可以读取同一份
已经新于最近 action 的 snapshot，不强制为每个只读 wait 再空转一帧。

## 10. ProductionAuthority 条件

### 10.1 schema

保留现有 `waitFor` step，只新增一个 condition kind。为了保持 predicate 与 timeout 分离，
`timeoutMs` 是 wait step 的可选字段：

```json
{
  "kind": "waitFor",
  "stepId": "round-2-settled",
  "timeoutMs": 90000,
  "condition": {
    "kind": "projectValueEquals",
    "path": "tower.round",
    "equals": 2
  }
}
```

```json
{
  "kind": "waitFor",
  "stepId": "round-2-organizing",
  "timeoutMs": 90000,
  "condition": {
    "kind": "projectValueEquals",
    "path": "tower.phase",
    "equals": "organizing"
  }
}
```

对用户和 AI 的概念接口仍是：

```text
waitForProjectValue(path, equals, timeoutMs)
```

但持久化 schema 复用既有 `waitFor` step，不另建第二种等待执行器。

### 10.2 timeout

- 未写 `timeoutMs` 时使用 scenario 现有 `perStepTimeoutMs`。
- 显式 wait timeout 必须在 `1..=120000ms` 内，且不得超过 overall timeout 剩余时间。
- 只使用 monotonic real time，不使用 game time、frame count 或渲染 present count。
- 不增加 stall timeout、动态延期、自适应预测或无界 retry。
- 现有 scenario overall timeout 仍是最终上界。

### 10.3 评估状态

| 状态 | 结果 |
|---|---|
| active runtime 还没有首份 post-commit snapshot | pending，受 timeout 限制 |
| contract 不存在 | 立即失败 `authority.project_observation_contract_unavailable` |
| path 未声明 | 立即失败 `authority.project_observation_path_unknown` |
| equals JSON 类型与 contract 不一致 | 立即失败 `authority.project_observation_expected_type_mismatch` |
| snapshot 缺 path 或 actual 类型错 | 立即失败 `authority.project_observation_contract_violated` |
| snapshot 不满足 session/frame 新鲜度 | pending |
| actual == expected | passed，进入下一个 step |
| actual != expected | pending，直到满足或 timeout |
| runtime Stop/session 切换且步骤未期待 | 立即失败 session changed |

未知 path 和类型错误不得消耗 90 秒后才报错。

### 10.4 失败证据

project value wait 的 step report 至少包含：

```text
path
declaredType
expected
lastActual
runtimeFrame
sessionId
contractId
elapsedMs
timeoutMs
diagnosticCode
```

Summary/Trace 可以记录这些紧凑事实；Off 不生成额外文件。失败不导出全部项目
snapshot，避免把未参与条件的项目状态扩散到证据。

## 11. Tower Defense 首个 Adapter

### 11.1 状态来源

塔防 `TowerDefenseRuntimeSession` 已拥有权威 `MatchRuntime`，现有 `tower.matchView`
紧凑 read model 也已经输出 `match.phase / match.round`。项目应抽取一个项目内部
纯读取 helper，同时为 World UI projection 和 Observation Adapter 提供原始 phase/round，
避免两套计算逻辑。

Observation 不复用以下 AUI 文本：

```text
tower.phase_text
tower.round_text
```

它直接输出未本地化的稳定值：

```text
tower.phase = "organizing" | "combat" | "augment" | "finished"
tower.round = integer
```

### 11.2 四轮等待

每次点击 `td.start-round` 后，scenario 顺序等待：

```text
tower.round == expected round
tower.phase == expected next phase
```

两个 wait 可读取同一份新于 `td.start-round` action 的 post-commit snapshot。第 3 轮后等待
`tower.phase == "augment"`，选择军略后等待下一份新 snapshot；第 4 轮等待
`tower.phase == "finished"`。

264 不修改塔防战斗时长、核心生命、轮次、补偿或任何 gameplay 结果。

## 12. AI 适配性

### 12.1 可发现

AI 从 `project.aife.json -> observationContract` 获得唯一入口，不需扫描 Rust、ECS 或 AUI
猜测可等待状态。`description` 和可选 `allowedValues` 提供机器可读语义。

### 12.2 可生成

AI 只需生成已有 `waitFor` step 和一个 `projectValueEquals` condition，不需生成 Rust callback、
frame budget 或项目专用 runner 代码。

### 12.3 可校验

Build/Preview 校验 contract；scenario load 校验条件形状；runtime 第一次评估校验 path 和
expected type；post-commit publish 校验 actual value。错误不会退化为无信息 timeout。

### 12.4 可解释和可修复

失败报告同时告诉 AI `expected / lastActual / frame / session / path / declaredType`。AI 可以
判断是条件写错、状态未到达、项目 Adapter 违反合同还是 session 已经更换。

### 12.5 适配度评估

在本方案的 schema、语义描述、类型校验和结构化诊断齐全时，AI 适配度为高。如果实施时
删除 contract validator、description 和 expected/actual 证据，只留裸字符串 path，则不符合
C-Compact，不得通过方案验收。

## 13. 设计成本与性能边界

### 13.1 成本模型

| 成本 | 等级 | 解释 |
|---|---|---|
| 引擎首次实现 | 中 | 跨资产、RuntimePackage、session、GameView 和 authority |
| 无观察旧项目 | 近似零 | 可选字段 + 默认空 observe |
| 项目首次声明 | 低 | 一份合同 + 一个紧凑 Adapter |
| 新增 scenario | 很低 | 只改 JSON，不改 Rust |
| 新增已有 read model 字段 | 低 | 声明 + 投影 + 合同测试 |
| 新增复杂业务里程碑 | 中 | 项目先定义稳定语义，不由引擎猜测 |

### 13.2 运行成本

- 只有存在 cooked contract 的 active runtime 执行 observe。
- 每个完成 fixed frame 最多生成 64 个标量值。
- Host 只保留最新 snapshot，用新快照替换旧快照。
- 正式 runtime 默认不写 report 文件，不保留 history，不序列化 description。
- Production authority 只读内存 snapshot，不通过文件轮询。

如果日后真实项目证明 64 个标量每 fixed frame 仍有性能问题，可以在不改公共合同的情况下
在引擎实现内增加 dirty/cache。v1 不先暴露 requested-path subscription 来增加跨线程协调成本。

## 14. 与现有系统的边界

### 14.1 ProjectUiStateSnapshot

Observation snapshot 和 UI snapshot 可以使用项目内部的同一纯 read-model helper，但两者是不同公共
合同：

| Observation | ProjectUiStateSnapshot |
|---|---|
| 业务完成和验收事实 | AUI 绑定 ViewModel |
| 项目主动公开稳定 path | active binding path 驱动 |
| post-commit fixed-frame 采样 | AUI present 阶段生成 |
| 原始 typed value | 可为本地化文本 |
| 不依赖当前 AUI Document | 受当前 AUI binding 影响 |

### 14.2 ProjectRuntimeSession report

Observation 不得塞进 `ProjectRuntimeSessionFrameReport` Trace 当作常驻全量值历史。Frame report 只记录
snapshot status/value count/diagnostic codes 等紧凑 Summary；真实值只在 active latest snapshot 中存活。

### 14.3 ProductionAuthorityScenario

264 深化 263 已有的 bounded scenario，不建第二个 runner。既有 frame condition 继续用于验证
机械帧推进，但不得再用于代理已公开 observation 的业务完成条件。

## 15. 诊断分层

### 15.1 Build/Preview 诊断

```text
project_observation.contract_read_failed
project_observation.contract_schema_unsupported
project_observation.contract_path_invalid
project_observation.contract_path_duplicate
project_observation.contract_type_invalid
project_observation.contract_allowed_value_type_mismatch
project_observation.contract_limit_exceeded
```

每个诊断必须包含 source path、contractId/path（如可用）、stage 和 next action。

### 15.2 Runtime 诊断

```text
project_observation.observe_panicked
project_observation.value_missing
project_observation.value_undeclared
project_observation.value_type_mismatch
project_observation.snapshot_not_published_after_fault
```

### 15.3 Authority 诊断

```text
authority.project_observation_contract_unavailable
authority.project_observation_path_unknown
authority.project_observation_expected_type_mismatch
authority.project_observation_contract_violated
authority.project_observation_session_changed
authority.scenario_step_timeout
```

## 16. 验收矩阵

### 16.1 Contract owner

1. 无 contract 的 legacy/empty 项目可构建、Play 和导出。
2. 有效 contract 经项目安全路径解析并 cook 进 RuntimePackage。
3. 未知 schema、重复 path、越界路径、超限、类型错和 allowedValues 错有定向负例。
4. contract 元数据变化只使 RuntimePackage cache 失效，不使 composition identity 失效。

### 16.2 Runtime owner

1. observe 严格发生在成功 deferred commit 之后。
2. action + fixed update 同帧时发布 fixed-frame 最终状态，不发布 action 中间态。
3. mutation commit fault 不发布候选 snapshot。
4. Pause 不产生新 snapshot，StepFrame 精确产生一份。
5. Stop/Restart/Play 使旧 session snapshot 不可见。
6. missing/extra/type mismatch/panic 均 fail closed，不用旧 snapshot 伪装新帧。

### 16.3 Authority owner

1. schema 稳定地序列化/deserialize `projectValueEquals`。
2. 条件只读重试，click/action 次数保持 exactly-once。
3. 新鲜度水位拒绝 action 前的 stale snapshot。
4. 未知 path/type 立即失败，actual mismatch 才等待 timeout。
5. 真实时间 timeout 和 overall timeout 同时有效。
6. 失败证据包含 expected/lastActual/runtimeFrame/sessionId，不泄露未相关值。

### 16.4 通用项目矩阵

- Tower Defense：`tower.phase / tower.round`。
- 第二个普通项目：使用完全不同的 namespace 和业务值，证明 engine/runtime/authority
  没有 Tower Defense 特判。
- empty/no-contract 项目：证明向后兼容和零观察成本。

### 16.5 Tower Defense 消费验收

264 施工完成后，只有下列 wait 证据可以声明 production wait 缺口解除：

- 真实 specialized Editor 通过 project value condition 完成四轮、军略和终局状态等待。
- scenario 不再使用固定 `runtimeFrameAdvancedSinceStep` 表示战斗已结算。
- 每个 wait 使用真实 post-commit snapshot 并记录紧凑 step evidence。

这不自动使 P0-5 Gate G 整体通过；AUI/GameView 视觉缺口必须另行方案、施工和
视觉矩阵验收。

## 17. 预期涉及所有权

本节是方案级 ownership，不是施工文档或修改授权。

### 17.1 engine-owned

- `editor_core` 的 ProjectManifest optional reference、contract loader/cooker 和 RuntimePackage assembly。
- `engine_runtime` 的 contract/snapshot types、`ProjectRuntimeSession::observe`、post-commit sampling 和
  active latest snapshot。
- `editor_window_winit` 的 GameView read-only projection、ProductionAuthority schema/evaluator/report。
- 影响范围内的 owner/consumer tests 和第二通用项目矩阵。

### 17.2 project-owned

- `samples/tower_defense_project/Observations/project.observations.json`。
- Tower Defense `project.aife.json` optional reference。
- Tower Defense RuntimeModule 的两值 observe Adapter 和合同测试。
- Tower Gate G scenario 中的条件替换。

引擎施工和塔防项目消费在未来施工文档中必须分 Gate 和分 ownership 记录，不得以
“修塔防”隐藏共享引擎修改。

## 18. 风险与成本护栏

### 18.1 合同膨胀

风险：项目把每个单位、entity 和 UI 字段全部公开，形成第二份 World 镜像。

护栏：64 字段硬上限；只允许稳定业务事实；列表、对象和实体查询不进 v1。

### 18.2 与 UI 双重逻辑

风险：phase/round 在 UI producer 和 observe Adapter 分别计算并漂移。

护栏：项目内部共享纯 read-model helper；两个公共合同保持分离。

### 18.3 字符串 path 误用

风险：AI 生成不存在 path，或项目改名后 scenario 静默失效。

护栏：单一 contract、类型校验、立即 unknown-path 失败、稳定 ID 规则和结构化诊断。

### 18.4 越做越像测试 DSL

风险：不断增加逻辑组合、任意 predicate、事件、变量和回调。

护栏：v1 只有一个 equals condition；多条件使用顺序 wait；任何扩展必须以新正式方案
证明自己不能由稳定项目 milestone path 表达。

### 18.5 观察反向影响 gameplay

风险：observe 引入 lazy mutation、随机消费或文件 I/O，使测试行为改变游戏。

护栏：`&self + WorldReadApi`、无 mutation buffer、无可变 runtime context，定向测试证明开启/关闭
authority observer 不改变游戏 digest 和 action count。

## 19. 方案自审

### 19.1 是否修改了用户确认方向

否。保留 C 的项目公开 Observation Contract 和 post-commit snapshot，用 Compact 护栏删除
catalog、表达式树、事件总线、stall timeout 和 replay。

### 19.2 是否把塔防概念写进引擎

否。引擎只认识 path、标量类型、snapshot 新鲜度和 equals。`tower.*` 只出现在项目资产、
项目 Adapter 和项目 scenario。

### 19.3 是否让普通条件进入 Rust

否。Rust 只提供项目已有权威状态的紧凑只读 Adapter。具体等待 path、值、顺序和
timeout 都在 scenario JSON。

### 19.4 是否把 AUI 当成 gameplay truth

否。Observation 在 fixed-frame post-commit 生成，不依赖 AUI Document、active binding、本地化或
present 成功。

### 19.5 是否新增没有第二 Adapter 的假 seam

否。seam 是已有 `ProjectRuntimeSession` 接口的只读深化，不新建独立 Producer 层。塔防、第二
普通项目和默认空项目形成真实多 Adapter 验证。

### 19.6 是否控制了 AI 和维护成本

是。单一入口、有界 schema、明确类型/语义、结构化 expected/actual 诊断，且 scenario 不需
Rust 重建。

### 19.7 是否与当前施工状态冲突

否。263 仍是唯一 `施工文档/当前/` 引擎文档并保持 blocked。264 只是已确认正式方案，
尚未生成施工文档，不授权修改引擎、塔防项目或重跑 Gate G。

## 20. 后续流程

如果用户要求进入施工文档阶段，下一步必须：

1. 根据 264 生成独立引擎施工文档并自审。
2. 由于 263 仍占用当前施工槽，264 施工文档只能进入 `施工文档/待执行/`，明确
   标记不可施工。
3. 必须单独决定 263 如何收口，且当前施工槽为空后，才能对 264 执行激活前复核。
4. 激活与施工需要新的明确授权；本方案确认不自动继承历史 Gate G、production binary、
   Local CI 或真实用户配置授权。

