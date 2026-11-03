# 264 当前可自动化施工文档：Project Observation Contract / Post-Commit Production Wait C-Compact v1

## 1. 施工状态

```text
方案：264-Project-Observation-Contract-Post-Commit-Production-Wait-C-Compact-v1方案.md
方案选择：C-Compact
文档状态：已生成、已自审、已通过激活前复核
施工状态：Gate A-F 已完成并归档
前置：263 已正式暂停并移入历史；本文已获得唯一当前施工槽
授权：2026-08-02 用户要求按 264 方案生成施工文档并直接开始施工
时间上限：本次连续施工不超过三小时
```

本文已从 `施工文档/待执行/` 激活到 `施工文档/当前/`，是唯一活动引擎施工文档。

## 2. 施工目标

打通一条项目无关的 typed observation 链路：

```text
project.aife.json optional observationContract
  -> project-observation-contract.v1 asset
  -> ProjectRuntimePackageAssembler / RuntimePackage
  -> ProjectRuntimeSession::observe
  -> fixed-frame post-commit snapshot
  -> Editor GameView latest observation
  -> ProductionAuthority projectValueEquals
  -> Tower Defense tower.phase / tower.round
```

结果必须让 Production authority 以项目公开业务状态等待塔防轮次，不再用固定帧数
代理战斗结算。普通条件仍在 scenario JSON，塔防概念不进入引擎。

## 3. 授权和禁止项

允许：

- 修改 264 明确列出的 engine-owned Rust source/schema/tests。
- 修改 Tower Defense 项目 manifest、Observation asset、RuntimeModule Adapter 和项目测试。
- 修改一个第二普通样例项目作为无塔防特判消费证据。
- 运行 owner-level 定向测试、受影响 crate 回归、feature check 和 format check。

禁止：

- 运行 Local CI。
- 替换 production/安装态 Editor 或 Player。
- 修改真实用户配置、recent-project store 或稳定 MCP install。
- 重跑真实 Tower Gate G、进入 Gate H 或修复 AUI/GameView 视觉缺口。
- 新增 Observation Catalog、表达式树、EventBus、stall timeout、replay 或新 DPI seam。
- 清理、回退或覆盖工作树中既有用户修改。

## 4. 代码基线与所有权

### 4.1 engine-owned

| Owner | 预期文件 |
|---|---|
| Project manifest / assembly | `editor_core/src/project_launcher.rs`, `project_runtime_package_assembler.rs`, 相关 tests |
| RuntimePackage contract | `engine_runtime/src/runtime_package.rs`, `runtime_package_builder.rs`, 新 observation module |
| Runtime session seam | `engine_runtime/src/project_runtime_session.rs`, `frame_loop.rs`, `engine_host_loop.rs` |
| GameView consumer | `editor_core/src/editor_gameview_play.rs`, `session.rs` |
| Production authority | `editor_window_winit/src/production_authority.rs`, `real_window.rs`, owner tests |

如实际 owner 已在邻近深 Module 内，优先深化它，不为了与表格文件名一致新建浅层
pass-through module。

### 4.2 project-owned

```text
samples/tower_defense_project/project.aife.json
samples/tower_defense_project/Observations/project.observations.json
samples/tower_defense_project/RuntimeModule/src/*
samples/tower_defense_project/RuntimeModule/tests/*
samples/switch_puzzle_project/project.aife.json or equivalent second-project fixture
samples/switch_puzzle_project/RuntimeModule/src/* or equivalent second-project fixture
```

Tower Defense 只能使用完成后普通项目作者同样可用的公共 seam。

## 5. Gate A：Observation Contract 和 RuntimePackage

### A1. 合同类型

在 `engine_runtime` 定义：

- `project-observation-contract.v1` schema version。
- `ProjectObservationType`：Bool/Integer/Number/String。
- `ProjectObservationValue`：有界 typed scalar。
- contract/entry/diagnostic/validation types。
- 最多 64 paths、path/description/allowedValues 上限与稳定 path 校验。

先增加精确负例：未知 schema、重复 path、非法 path、超限、allowedValues 类型错。

### A2. ProjectManifest 和安全路径

`ProjectManifest` 增加可选 `observationContract`。Assembler 只通过既有安全项目路径读取并校验合同，
生成 source path/stage/next action 完整诊断。

### A3. RuntimePackage

contract 以 optional cooked typed data 进入 RuntimePackage；Runtime 不扫描项目源目录。无 contract 的旧 package
保持兼容。contract 元数据变化不得改变 ProjectRust AOT/composition identity。

### A4. Gate A 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime project_observation
cargo test -p editor_core project_observation
```

Gate A 通过标准：正负合同测试通过，无 contract 项目回归通过，且 RuntimePackage 中只有 cooked
typed contract。

2026-08-02 Gate A 结果：

```text
cargo test -p engine_runtime project_observation：5 passed
cargo test -p editor_core project_observation：2 passed
结论：contract owner 正负例、安全项目路径、assembly digest、cooked manifest 和 no-contract 兼容通过
备注：editor_core 首次编译曾因 124 秒命令上限超时且无测试结果；增量重跑通过，不属于产品失败
```

## 6. Gate B：ProjectRuntimeSession post-commit snapshot

### B1. 深化现有 seam

`ProjectRuntimeSession` 增加带默认空实现的只读 `observe(&self, context)`。context 只有 runtime
frame/time、WorldReadApi、cooked contract 和 report level，无 mutation/renderer/filesystem/network。

### B2. 发布时序

FrameLoop 只在 AUI action mutation commit、session FixedUpdate mutation commit 和既有 project fixed update 完成后
调用 observe。Host 盖上 frame/session/contract identity，校验 complete snapshot 后只保留最新一份。

任一 mutation terminal fault 或 observe 合同违约都不得把候选 snapshot 发布为成功新帧。

### B3. 生命周期

- Pause 不生成新 snapshot。
- StepFrame 精确产生一份。
- Stop 清除 snapshot。
- Restart/Play 更换 sessionId，旧 snapshot 不可见。
- 只保留 latest，不保留历史。

### B4. Gate B 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime project_observation
cargo test -p engine_runtime project_runtime_session
```

必须有能捕获 tick 前采样、commit-fault stale publish、Pause/Step/Stop 错误的定向测试。

2026-08-02 Gate B 结果：

```text
cargo test -p engine_runtime project_observation：5 passed
cargo test -p engine_runtime project_runtime_session：33 passed
结论：post-commit 最终状态、terminal mutation fault、Pause/Step/clear、缺失/额外/类型错/panic fail closed 通过
```

## 7. Gate C：GameView 与 ProductionAuthority

### C1. GameView 只读投影

`EditorGameViewPlayRunner`/`GameViewPresentReport` 对 Editor session 暴露最新 compact observation snapshot。
不把它写入 AUI snapshot，不保留全量历史，不新建文件轮询通道。

### C2. scenario schema

现有 `WaitFor` step 增加可选 `timeoutMs`；`ProductionAuthorityCondition` 只增加：

```text
ProjectValueEquals { path, equals }
```

`equals` 只能是 bool/integer/finite number/string。显式 timeout 上限 120000ms，overall timeout 仍是最终
上界。

### C3. 评估与诊断

- no snapshot yet：pending。
- no contract/unknown path/expected type mismatch/actual contract violation：immediate failure。
- stale frame/wrong session/actual mismatch：按方案的新鲜度与生命周期规则处理。
- equality passed：严格顺序进入下一步。

step report 增加 path/declaredType/expected/lastActual/runtimeFrame/sessionId/contractId/timeoutMs，不导出未相关
snapshot values。轮询只读取，不重放动作。

### C4. Gate C 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_observation
cargo test -p editor_window_winit production_authority
```

必须覆盖 schema roundtrip、step-specific timeout、stale snapshot、unknown path/type immediate failure、actual mismatch timeout 和
action exactly-once。

2026-08-02 Gate C 结果：

```text
cargo test -p editor_core project_observation：3 passed
cargo test -p editor_window_winit production_authority：5 passed
cargo check -p editor_window_winit --features real-window,real-wgpu-surface：passed（Gate C 开发反馈）
结论：GameView latest projection、Stop clear、schema/timeout、typed/fresh/session evaluator 和 compact evidence 通过
```

## 8. Gate D：通用项目矩阵

至少使用三类 Adapter：

1. empty/no-contract 项目：默认空 observe，无行为回归。
2. Switch Puzzle 或等价第二项目：使用不同 namespace/type/value 完成 contract -> runtime -> condition。
3. Tower Defense：作为后续 Gate E 真实消费者。

禁止 engine/core/test helper 出现 `tower.phase / tower.round / organizing / augment` 特判。

### Gate D 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p project_e2e_gate second_project_runtime
cargo test -p editor_core project_observation
```

2026-08-02 Gate D 结果：

```text
cargo test -p project_e2e_gate second_project_runtime：3 passed
cargo test -p editor_core project_observation：3 passed（沿用 Gate C 后未失效的同 source identity 证据）
结论：empty/no-contract 默认 Adapter、Switch Puzzle puzzle.solved bool Adapter、headless snapshot 和
Editor projection 均通过；engine/runtime/authority 中没有 Tower Defense 特判
备注：project_e2e_gate 首次执行因 304 秒工具上限结束且未产生测试结论；缓存后重跑 3 项全通过，
其中 release process consumer 约 96 秒
```

## 9. Gate E：Tower Defense Adapter 和 scenario

### E1. 项目合同

新增 `Observations/project.observations.json`，只声明：

```text
tower.phase : string
tower.round : integer
```

`project.aife.json` 只增加合同引用。

### E2. RuntimeModule Adapter

`TowerDefenseRuntimeSession::observe` 从已有权威 `MatchRuntime`/项目内部紧凑 read-model helper 输出原始
phase/round。不读 `tower.phase_text / tower.round_text`，不增加第二套 gameplay 状态。

### E3. scenario replacement

修改 repository-owned/run fixture scenario 生成路径，用 `projectValueEquals` 替换表示战斗结算的
`runtimeFrameAdvancedSinceStep`。保留 frame condition 用于纯机械帧推进验证。

### E4. Gate E 测试

```powershell
Set-Location G:\gameEngin
cargo test --manifest-path samples\tower_defense_project\RuntimeModule\Cargo.toml
cargo test --manifest-path samples\tower_defense_project\Tests\PlayableInterface\Cargo.toml
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit production_authority
```

本 Gate 不启动真实 Editor，不重跑 Gate G，不处理视觉缺口。

2026-08-02 Gate E 结果：

```text
Tower Defense RuntimeModule：80 passed
Tower Defense PlayableInterface public-seam：2 passed
  - 四轮/军略/终局/Restart 与 manifest -> RuntimePackage -> Host observation 链路通过
  - full-flow fixture 只使用 tower.round == 2/3 表示两次结算，旧固定帧代理为 0
editor_window_winit production_authority：5 passed
JSON 结构校验：Observation contract 与 production scenario 均通过
结论：tower.phase/tower.round 只来自 MatchRuntime authoritative snapshot；scenario 普通等待保持 JSON 可热改
备注：原草案命令 cargo test -p editor_core tower_defense 匹配 0 tests，不作为通过证据；已由项目自有
PlayableInterface public-seam 测试替代。首次新增 fixture 测试因测试闭包返回类型编译失败，最小修复后重跑通过；
产品代码与 fixture 未发生失败
```

## 10. Gate F：受影响回归

按共享接口和跨 crate consumer 影响面执行：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime
cargo test -p editor_core
cargo test -p editor_window_winit
cargo test -p project_e2e_gate
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
cargo fmt --all -- --check
```

上层 suite 已包含的同配置定向测试不再机械重复。任一失败先保留首个具体 assertion/diagnostic，
回到 owner 级修复，不用全量 suite 作调试循环。

不运行 Local CI、不产生 exact-commit/release 证据、不替换 production binary。

2026-08-02 Gate F 结果：

```text
cargo test -p engine_runtime：502 passed
cargo test -p editor_core：616 passed，2 ignored
cargo test -p editor_window_winit：160 passed，1 ignored
cargo test -p project_e2e_gate -- --test-threads=1：主 crate 65 passed，2 ignored；
  integration tests 1 + 1 + 1 + 3 passed
cargo check -p editor_window_winit --features real-window,real-wgpu-surface：passed
cargo fmt --all -- --check：passed
```

回归修复与隔离记录：

- `editor_core` 首轮 615 passed 后唯一失败来自 project Player artifact 内部
  `cargo fmt --check`；执行 workspace formatter 后 owner 复现 1/1 与全量 616/616 通过。
- `project_e2e_gate` 默认并发冷运行曾使 120 秒 preflight 时间断言达到 133178ms；owner
  单独复现 3.35 秒通过。缓存后默认并发重跑又暴露两个 fake-HTTP/lifecycle 测试互相竞争，
  owner 复现分别 4.98 秒和 0.98 秒通过。最终按独占 mutable state 原则串行运行完整 suite，
  所有非 ignored 测试通过；未修改无关 LLM/Gateway 产品代码或测试阈值。

Gate F 结论：passed。结果是 dirty-preserved source construction evidence，不声明 clean exact
commit、Local CI、production binary 或真实 Gate G 资格。

## 11. 证据与报告

本次是 dirty-worktree 上的普通 source construction：

- owner/consumer test 结果是开发和受影响回归证据，不宣称 clean exact commit。
- 不读写真实外部用户配置。
- 不需要 repository-external run root，因为本文不运行真实 production Gate。
- 测试失败记录必须绑定 source/test identity，不覆盖 263 已有 Gate F 阻断证据。

## 12. 文档同步和完成规则

全部 Gate 通过后：

1. 生成 `阶段完成记录/2026-08-02-Project-Observation-Contract-Post-Commit-Production-Wait-C-Compact-v1/00-总览.md`。
2. 更新 `00-文档地图.md`、`00-阅读顺序.md`、`49-当前状态与下一步入口.md`、
   `54-当前可自动化施工总入口.md`、`施工文档/README.md`、`阶段完成记录/README.md`。
3. 把本文移到 `施工文档/已完成/`。
4. 明确记录未运行 Local CI、未重跑 Gate G、未替换 production binary，且视觉缺口仍未解决。
5. 264 完成只解除 production wait 的 source capability 缺口，不自动将 263/P0-5 Gate G 改为通过。

## 13. 施工文档自审

### 13.1 是否扩大正式方案

否。不包含 catalog、表达式树、EventBus、stall timeout、replay、AUI truth 或视觉修复。

### 13.2 是否给每个要求提供捕获失败的测试

是。Gate A-C 覆盖 owner 正负合同、post-commit 时序和 authority 语义；Gate D-E 覆盖第二项目
和塔防 consumer；Gate F 覆盖受影响 crate closure。

### 13.3 是否与 264 正式方案冲突

否。持久化 schema 继续复用 `WaitFor` step，predicate 和 timeout 分离；引擎只认识通用标量。

### 13.4 是否保留了项目/引擎所有权

是。Tower 概念只出现在 project-owned 合同和 Adapter；engine-owned 只实现公共 seam。

### 13.5 是否误用高成本验证

否。不运行 Local CI、不做 production replacement、不重跑真实 Gate G；只使用 owner/consumer 和
受影响 crate 回归支持 source-level 施工结论。

### 13.6 是否可直接施工

是。263 已保全并移入历史，当前施工槽在复核时为空；264 正式方案、代码 owner、测试包和
禁止项均复核通过。本文移入 `当前/` 后可从 Gate A 开始施工。

## 14. 激活前复核条件

激活时必须重新确认：

```text
264 正式方案仍是最新已确认方案。
263 Gate A-E 和 Gate F blocked 证据已保全，文档以暂停状态移入历史。
施工文档/当前/ 除 README 外为空。
工作树既有修改未被回退，本文 owner 列表与当前代码仍一致。
测试命令对当前 workspace/package 名仍有效。
授权不包含 Local CI、production binary、真实用户配置或 Gate G 重跑。
```

复核通过后：

1. 把本文状态修订为 `当前，可施工`。
2. 移入 `施工文档/当前/`。
3. 同步 `54-当前可自动化施工总入口.md` 和 `施工文档/README.md`。
4. 从 Gate A 开始施工，不跨 Gate 堆积失败。

### 14.1 2026-08-02 激活前复核结果

```text
264 正式方案：present，仍为最新已确认方案
263 现场：已移入历史/263-已暂停施工文档-*.md，Gate F blocked 证据保留
当前施工槽：激活前无其它 *当前可自动化施工文档*.md
owner 文件：present
test packages：engine_runtime/editor_core/editor_window_winit/project_e2e_gate present
工作树：dirty-preserved，禁止回退既有修改
授权排除：Local CI / production replacement / real user config / real Gate G
复核结论：passed，激活 264
```
