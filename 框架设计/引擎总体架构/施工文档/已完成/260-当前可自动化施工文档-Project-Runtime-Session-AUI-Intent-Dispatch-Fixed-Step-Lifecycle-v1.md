# 260-当前可自动化施工文档-Project Runtime Session / AUI Intent Dispatch / Fixed-Step Lifecycle v1

## 1. 文档状态

```text
正式方案：../../260-Project-Runtime-Session-AUI-Intent-Dispatch-Fixed-Step-Lifecycle-v1方案.md
方案选择：用户已确认 E3 窄型 ProjectRuntimeSession
方案审查：已完成
施工文档：已生成并完成自审
当前状态：Gate A-F 已完成并归档
所在目录：施工文档/已完成/
当前施工授权：已结束
已完成：Gate A-F
施工中：无
```

2026-07-30 已完成激活前复核：正式方案、代码基线、测试命令、前置状态和唯一施工槽
均匹配，用户已明确授权窗口 1。窗口 1 禁止：

```text
修改 runtime_player_winit、editor_core、project_modules 或 project_players
执行 Gate D-F
生成完整阶段完成记录或归档本文
继续塔防 P0-5 项目施工
```

Gate A-C 全部通过后，本文保持在 `施工文档/当前/`，状态更新为“窗口 1 已完成、窗口 2
未授权”，等待用户再次明确授权。

2026-07-30 用户已明确授权窗口 2。重新核对工作树后，Player / Editor / project module /
project player / project_e2e_gate 范围没有并行改动；Window 1 engine owner 改动保持原样。
当前从 Gate D 开始，窗口 2 不扩大到真实窗口、production binary、Local CI 或塔防 P0-5。

2026-07-30 窗口 2 已完成。Player / Editor consumer migration、Complex Shooter /
Switch Puzzle project session registration、Switch Puzzle action-to-fixed-to-snapshot E2E、
受影响 package all-target/all-feature closure、格式与源代码边界审计均已通过。完整 workspace
check 暴露的 `editor_input` fixture 缺字段问题与 260 修改无关，已按原始 `HEAD` 基线缺陷记录，
未在本施工中越界修复。阶段完成记录和正式入口已同步，施工授权结束。

## 2. 施工目标

补齐 213 与 242 之间缺失的正式 runtime 写侧链路：

```text
AuiInteractionResult.actions
  -> EngineFrameInput
  -> ProjectRuntimeSession::handle_aui_actions exactly-once
  -> ProjectRuntimeMutationBuffer preflight/commit
  -> ProjectRuntimeSession::fixed_update
  -> ProjectLogicRunner existing phases
  -> Physics / RenderProjection
  -> 下一次 ProjectUiStateSnapshot production
```

完成必须证明：

```text
每个 BoundProjectRuntime 创建一份独立 session。
缺失、重复或失败的 session factory 在首帧前 fail closed。
action batch 保持 vector 顺序，每个 advancing frame 最多分发一次。
Rejected、Unhandled、Faulted 和暂停帧 action 都不重放。
session 与 ProjectLogicRunner 使用同一正式 fixed-step 节点。
session 不获得即时 WorldWriteApi，只输出 deferred mutation batch。
mutation batch 全量 preflight 后提交；失败时不能带漂移继续运行。
Player、Editor GameView、headless e2e 使用同一公共 session chain。
Complex Shooter、Switch Puzzle 和 Empty project 显式注册 session。
第二项目 Gate 证明 Engine Core 没有塔防专用语义。
Off/Summary/Trace 和 compact EngineFrameOutput report 可审计。
```

## 3. 施工前基线

### 3.1 已确认方案

```text
195：AUI action 是业务意图，进入 Project Logic / Project Rule。
199：ProjectUiStateSnapshotProducer 保持只读。
213：AUI interaction、input consumption 和 AuiAction generation 已完成。
220：Editor GameView 已能生成 AuiInteractionResult。
242：ProjectRuntimeModule / Bootstrap / BoundProjectRuntime 已完成。
260：E3 窄型 ProjectRuntimeSession 已确认并完成方案自审。
```

### 3.2 当前代码事实

```text
engine_runtime::aui 已有 AuiAction 与 AuiInteractionResult.actions。
EngineFrameInput 已有 aui_interaction。
EngineHostLoop::tick_internal 当前没有把 aui_interaction 交给项目 callback。
LogicContext 没有 AUI action accessor，本施工也不增加。
ProjectRuntimeRegistration 当前只有 rule 和 UI producer 注册。
BoundProjectRuntime::into_parts 当前返回 logic/producer/input/receipt。
runtime_player_winit headless/windowed 都会拆分 BoundProjectRuntime。
EditorRuntimePlayInstance 也会拆分 BoundProjectRuntime。
FrameLoop 当前每个 advancing host frame 执行一个 fixed step，再执行 Update。
WorldWriteApi 当前是即时写入，不能直接作为 session public write contract。
World 可 Clone，但不得用整 World 每 fixed step clone 作为 production mutation 方案。
RuntimeInstanceLoader 当前不可 Clone。
```

### 3.3 当前工作树

生成本文时，预计涉及的 Rust owner 文件没有已知未提交改动。激活前必须重新执行：

```powershell
git status --short -- rust/crates/engine_runtime/src rust/crates/runtime_player_winit/src/lib.rs rust/crates/editor_core/src/editor_gameview_play.rs rust/crates/project_e2e_gate rust/project_modules rust/project_players
```

若出现用户或其它任务的新改动：

```text
不 reset/checkout/revert。
先判断是否与 260 owner 重叠。
不重叠则保留并继续。
重叠则把实际基线和兼容方式写回本文，再重新自审。
只有无法安全合并时才请求用户决定。
```

## 4. 固定范围

### 4.1 必须交付

```text
ProjectRuntimeSession trait / create context / callback context / output。
ProjectRuntimeSessionFactory 与显式注册。
EmptyProjectRuntimeSession。
BoundProjectRuntime session ownership 与 bind receipt identity。
ProjectRuntimeMutationBuffer / prepared batch / commit report。
EngineHostLoop exactly-once action dispatch。
FrameLoop session FixedUpdate hook 与固定顺序。
non-advancing discard / EditorStep exact-once。
Off/Summary/Trace session report。
runtime_player_winit headless/windowed composition。
EditorRuntimePlayInstance Play/Pause/Step/Stop lifecycle。
Complex Shooter / Switch Puzzle / Empty project session migration。
第二项目与 cross-consumer e2e。
入口文档、施工归档和阶段完成记录。
```

### 4.2 禁止扩入

```text
塔防项目 RuntimeModule、Scene、AUI、Rules 或 Project Assets。
AUI Document authoring、dynamic payload、repeater 或 UI renderer。
LogicContext AUI accessor。
新的 Logic Ownership Router / EventBus / SignalBus。
Lua、WASM、Blueprint、dylib hot replace。
session snapshot / save game / rollback netcode。
通用 Update/LateUpdate/PostPhysics/session lifecycle 镜像。
第二个 fixed-step accumulator。
同帧生产两次 ProjectUiStateSnapshot。
production Editor replacement、真实安装配置或真实窗口像素验收。
Local CI、clean exact-commit qualification 或 release candidate。
```

### 4.3 项目边界

施工期间允许修改的 project-owned consumer 仅限现有通用回归项目：

```text
rust/project_modules/complex_shooter
rust/project_modules/switch_puzzle
rust/project_players/complex_shooter
rust/project_players/switch_puzzle
rust/project_players/empty
```

`samples/tower_defense_project` 全部只读。若构建链要求同步
`samples/complex_shooter_project/RuntimeModule` 或
`samples/switch_puzzle_project/RuntimeModule`，激活前必须先确认其 source/mirror owner，
只做同一 interface migration，不扩玩法。

## 5. 跨 Gate 固定合同

### 5.1 深 module

优先新增：

```text
engine_runtime::project_runtime_session
```

该 module 拥有：

```text
session interface
create/callback context
action batch view
mutation batch
prepared mutation
session output
diagnostics
report
no-op session
```

`project_runtime_module.rs` 只负责 registration/bootstrap/bind，`engine_host_loop.rs` 只负责编排。
不要把全部实现继续堆入现有大文件。

### 5.2 Session interface

施工目标接口：

```rust
pub trait ProjectRuntimeSession {
    fn session_id(&self) -> &str;

    fn handle_aui_actions(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
        batch: ProjectAuiActionBatch<'_>,
    ) -> ProjectRuntimeSessionOutput;

    fn fixed_update(
        &mut self,
        context: ProjectRuntimeSessionContext<'_>,
    ) -> ProjectRuntimeSessionOutput;
}
```

只允许因 Rust ownership/borrow checker 做等价签名调整。禁止增加其它 phase callback。

### 5.3 Registration

```text
ProjectRuntimeRegistration::set_runtime_session_factory
每 module 正好一次
duplicate -> registration_failed
missing -> session_missing
factory error -> bind failed
empty session_id -> bind failed
```

`EmptyProjectRuntimeModule`、Complex Shooter、Switch Puzzle 都显式注册，不使用静默 fallback。

### 5.4 Mutation batch

v1 batch 至少表达：

```text
replace component
write component field
write transform
existing GameplayCommand structural operation
```

正式执行：

```text
callback 构造 output + batch
  -> disposition 不允许提交：drop
  -> disposition applied：prepare 全部操作
  -> prepare 全部成功：consume prepared batch commit
  -> prepare/commit 失败：terminal project session fault
```

限制：

```text
不向 session 暴露 WorldWriteApi。
prepare 阶段不得修改 World、RuntimeInstanceLoader 或 session-visible handle table。
commit 必须消费 prepared typed data，不重新解析项目 payload。
不使用每 fixed step clone 整个 World 的实现。
结构操作复用 244 PreparedRuntimeEntities / RuntimeInstanceLoader 正式模式。
无法做到 infallible prepared commit 的 structural operation 在 v1 明确 reject，不做部分提交。
```

项目内部 session state 已推进而 World projection commit 失败时，不伪造回滚；runtime 进入
terminal fault、保留证据并停止后续 phase/frame。

### 5.5 Action batch

```text
输入只读借用现有 AuiAction。
vector index 是唯一稳定顺序。
不按 action_id/node_id/event 排序。
一个 EngineFrameInput batch 最多分发一次。
空/None batch 不调用 handle_aui_actions。
Rejected/Unhandled/Faulted 不缓存、不重试。
payload 对 Engine Core opaque。
Summary 不保存 raw payload。
Trace 只保存 presence/length/digest。
```

### 5.6 帧序

```text
FrameBegin
  -> session handle AUI actions
  -> action mutation prepare/commit
  -> session fixed_update
  -> fixed mutation prepare/commit
  -> ProjectLogicRunner FixedUpdate
  -> ProjectLogicRunner Update
  -> Physics2D
  -> ProjectLogicRunner PostPhysics
  -> RenderExtract
  -> FrameEnd
```

如果 session terminal fault：

```text
不执行后续 project phase、physics 或 render extraction。
EngineFrameOutput 仍返回 compact fault report。
下一帧不再次进入 session。
```

非 advancing mode：

```text
不调用 action handler。
不调用 fixed_update。
不排队。
有 action 时记录 discarded_non_advancing_mode。
```

### 5.7 UI read model

```text
ProjectUiStateSnapshotProducer 继续在 consumer 侧只读 World。
260 不把 producer 放进 session。
当前 snapshot_frame_lag=1 保持。
Renderer 不读取 session report 或项目 command。
```

## 6. 施工窗口

260 保守估计不能在一次三小时施工授权内同时完成 owner、全部 consumer、e2e 和闭环。
本文划分两个连续性独立的施工窗口：

```text
窗口 1：Gate A-C，引擎 owner 与定向测试。
窗口 2：Gate D-F，consumer/e2e/回归/文档闭环。
```

规则：

```text
激活本文和用户第一次“开始施工”只授权窗口 1。
窗口 1 完成后停止，保留本文在 当前/，报告证据并等待用户明确继续窗口 2。
窗口 2 需要新的施工授权和重新核对工作树。
每个窗口独立遵守三小时上限。
任一窗口 2:45 后不启动新 Gate。
```

### 6.1 窗口 1 完成结果

2026-07-30，Gate A-C 已完成：

```text
Gate A：ProjectRuntimeSession trait/factory/create context/no-op session、exactly-one registration、
        bind identity/status、BoundProjectRuntimeParts、RuntimeModule interface v2。
Gate B：typed ProjectRuntimeMutationBuffer、全量只读 preflight、prepared batch、稳定顺序 commit、
        staged/committed/rejected report；GameplayCommand structural operation 在 v1 显式
        project_runtime.mutation_unsupported，未触碰 RuntimeInstanceLoader。
Gate C：EngineHostLoop exactly-once action owner、FrameLoop 同一 fixed-step session hook、
        non-advancing discard、Off/Summary/Trace、panic/fault/mutation terminal latch。
```

实际产品源码：

```text
rust/crates/engine_runtime/src/project_runtime_session.rs
rust/crates/engine_runtime/src/project_runtime_session/tests.rs
rust/crates/engine_runtime/src/project_runtime_module.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/world_api.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_run.rs
rust/crates/engine_runtime/src/lib.rs
```

验证：

```text
基线红测：project_runtime_session_missing_factory_fails 旧实现按预期失败。
project_runtime_session_*：7 passed。
project_runtime_module：14 passed。
project_runtime_mutation_*：9 passed。
runtime_instance_loader：12 passed。
gameplay_command：4 passed。
project_runtime_session_host_*：12 passed。
engine_host_loop：20 passed。
frame_loop：19 passed。
cargo fmt --all -- --check：passed。
cargo test -p engine_runtime：480 passed，0 failed。
```

范围核对：

```text
未修改 runtime_player_winit、editor_core、project_e2e_gate、project_modules、project_players。
未修改 samples/tower_defense_project；该目录仍是用户原有未跟踪项目。
未增加 LogicContext AUI accessor、EventBus、Router、脚本 VM 或 session snapshot。
本文继续占用 当前/；未归档、未生成完整阶段完成记录。
```

## 7. Gate A - Session Contract / Registration / Bind

### 7.1 红测试

先增加能在旧实现上失败的定向测试，测试名统一使用前缀
`project_runtime_session_`：

```text
missing factory fails before BoundProjectRuntime
duplicate factory fails
factory error fails
empty session id fails
two binds create distinct sessions
bind receipt exposes session id/status
explicit no-op session binds
```

### 7.2 施工

```text
新增 project_runtime_session module 和 public facade。
实现 trait/context/output/report 最小类型。
实现 factory type 与 EmptyProjectRuntimeSession。
扩展 ProjectRuntimeRegistration / RegistrationRuntimeParts。
扩展 ProjectRuntimeBootstrap / BoundProjectRuntime。
把 into_parts tuple 收敛为 named BoundProjectRuntimeParts，避免新增第五/第六 tuple 位置。
扩展 ProjectRuntimeBindReceipt schema/version。
迁移 engine_runtime 内所有测试 fixture 到显式 no-op session。
```

`BoundProjectRuntimeParts` 至少包含：

```text
project_logic
project_runtime_session
ui_state_producer
default_input_mapping
receipt
```

### 7.3 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime project_runtime_session_ -- --nocapture
cargo test -p engine_runtime project_runtime_module -- --nocapture
```

测试输出如果过滤后为 0 tests，Gate 失败；必须修正测试命名或命令，不能把 0 tests 记为通过。

### 7.4 退出条件

```text
registration/factory/bind matrix 全绿。
每 bind 独立 session。
无隐式 session fallback。
bind receipt 可审计。
还没有接 EngineHostLoop，不提前声称 AUI action 可执行。
```

## 8. Gate B - Deferred Mutation Prepare / Commit

### 8.1 红测试

测试名统一前缀 `project_runtime_mutation_`，至少覆盖：

```text
component replacement prepare failure leaves World unchanged
field write prepare failure leaves World unchanged
transform target missing leaves World unchanged
mixed valid/invalid batch zero commit
prepared batch preserves operation order
rejected output drops batch
structural command prepare failure does not activate instance
successful prepared batch commits exact writes once
commit report separates staged/committed/rejected counts
```

### 8.2 施工

```text
实现 ProjectRuntimeMutationBuffer。
实现 operation typed enum，不接收任意 closure。
实现 read-only target preflight。
复用 World fallible mutation、PreparedRuntimeEntities 和 RuntimeInstanceLoader owner。
实现 PreparedProjectRuntimeMutationBatch。
实现 commit report / diagnostics。
把 RuntimeInstanceLoader structural preparation 中需要的 private helper 做最小 owner 提取。
```

若发现现有 GameplayCommand 无法在不修改 loader 的情况下完整 prepare：

```text
先把该 structural kind 标记 unsupported_in_project_runtime_session_v1。
组件/字段/transform batch 继续完成。
不得退回逐条 apply 后失败。
不得临时 clone/replace RuntimeInstanceLoader。
将 deferred structural kind 写入 report 和完成记录。
```

这属于 260 已允许的 v1 fail-closed 收敛，不得伪装为完整 structural support。

### 8.3 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime project_runtime_mutation_ -- --nocapture
cargo test -p engine_runtime runtime_instance_loader -- --nocapture
cargo test -p engine_runtime gameplay_command -- --nocapture
```

### 8.4 退出条件

```text
session public context 没有 WorldWriteApi。
prepare failure 零 World/instance side effect。
commit 有结构化 report。
unsupported structural kind 显式诊断。
没有 whole-World-per-tick clone。
```

## 9. Gate C - EngineHostLoop Dispatch / Fixed-Step / Fault

### 9.1 红测试

测试名统一前缀 `project_runtime_session_host_`：

```text
one click dispatches once
next empty frame does not replay
multiple actions preserve vector order
rejected/unhandled do not replay
non-advancing frame discards without queue
EditorStep-equivalent advancing frame calls once
fixed_update runs once before ProjectLogicRunner FixedUpdate
action mutation is visible to same-frame session fixed_update
fixed mutation is visible to ProjectLogicRunner FixedUpdate
session terminal fault skips later phases
faulted instance rejects future frame entry
Off/Summary/Trace allocation/report boundary
raw payload absent from Summary
```

### 9.2 施工

```text
EngineHostLoop 成为唯一 action dispatch owner。
EngineHostLoop 或其 FrameLoop composition 持有 ProjectRuntimeSession。
新增 with_project_runtime_parts 或等价 typed constructor。
旧 with_project_logic 只用于 engine unit fixture，并显式使用 no-op session。
把 aui_interaction.actions 以只读 batch 传入一次。
在 FrameLoop 固定位置调用 session fixed_update。
apply prepared mutation 后再进入下一 stage。
扩展 RuntimeTrace compact record 和 EngineFrameOutput session report。
实现 terminal session fault latch。
```

不得：

```text
把 actions clone 到多 phase。
把 actions 注入 LogicContext。
把 action batch 保留到 EngineHostLoop 字段供下一帧重放。
在 FrameLoop 新建第二 runtime time/accumulator。
```

### 9.3 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime project_runtime_session_host_ -- --nocapture
cargo test -p engine_runtime engine_host_loop -- --nocapture
cargo test -p engine_runtime frame_loop -- --nocapture
```

### 9.4 窗口 1 结束

Gate A-C 全绿后：

```text
记录实际修改文件和定向测试。
确认未修改 runtime_player_winit/editor/project modules。
本文保持 当前/ 且标记窗口 1 已完成、窗口 2 未授权。
停止施工并向用户报告。
```

不得自动进入 Gate D。

## 10. Gate D - Player / Editor Consumer Migration

Gate D 只在窗口 2 明确授权后开始。

### 10.1 红测试

测试名：

```text
runtime_player_project_runtime_session_*
editor_gameview_project_runtime_session_*
```

覆盖：

```text
headless Player 从 BoundProjectRuntimeParts 取得同一 session。
windowed composition 编译并使用同一构造入口。
Editor Play 创建新 session。
Pause 不调用 session。
Step 调用 action/fixed 各最多一次。
Stop drop session。
Stop 后再次 Play 创建不同 session。
Player/Editor report 暴露相同 session id/status/counters。
UI producer 仍与 session 分离并只读 World。
```

### 10.2 施工

```text
迁移 runtime_player_winit 两个 bound_runtime.into_parts consumer。
迁移 EditorRuntimePlayInstance construction/fields/tick。
更新 runtime_cli/project player compile closure。
把 session compact report 接入现有 Native Player / GameView report。
保留现有 AUI present-before-tick 和 snapshot_frame_lag=1。
```

### 10.3 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p runtime_player_winit runtime_player_project_runtime_session_ -- --nocapture
cargo test -p runtime_player_winit aui_interaction_filters_mouse_fire_and_dispatches_click_action -- --nocapture
cargo test -p editor_core editor_gameview_project_runtime_session_ -- --nocapture
cargo test -p editor_core editor_gameview_play -- --nocapture
cargo check -p runtime_cli --features real-window
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

过滤测试为 0 时 Gate 失败。

### 10.4 退出条件

```text
Player/Editor 没有 session 旁路。
Play/Pause/Step/Stop 生命周期通过。
real-window feature composition 能编译。
不启动真实窗口、不替换 production binary。
```

## 11. Gate E - Project Modules / Second Project / E2E

### 11.1 Project module migration

```text
Complex Shooter 注册显式 no-op session。
Empty project 注册显式 no-op session。
Switch Puzzle 注册语义不同的最小 stateful session。
```

Switch Puzzle 最小 session 只用于通用性证明：

```text
接收 AUI action_id=ui.toggle-switch。
按 action vector 顺序记录一次项目 mutation。
在 fixed_update 更新一个现有 puzzle presentation component 或计数。
UI producer 下一次 snapshot 读取该 World 状态。
```

其实现必须位于 `switch_puzzle_project_runtime`，不得把 puzzle action/component 写进 Engine。

若现有 Switch Puzzle AUI Document 没有 action ref：

```text
只在 Switch Puzzle sample/project-owned AUI 中增加固定 button action。
不扩 AUI authoring schema。
不修改塔防项目。
```

### 11.2 E2E report

新增或扩展：

```text
project-runtime-session-e2e-report.v1
```

至少记录：

```text
project/module/session identity
consumer kind
input frame/action order
handled/rejected/unhandled
fixed update count
staged/committed mutation count
next-frame UI snapshot value
fault/diagnostics
status
```

正向链：

```text
Switch Puzzle AUI click
  -> AuiInteractionResult
  -> EngineHostLoop
  -> Switch Puzzle session
  -> mutation commit
  -> fixed_update
  -> next snapshot
```

否定矩阵：

```text
missing session factory
wrong session id
duplicate factory
rejected action
non-advancing discard
mutation preflight failure
terminal fault no next-frame reentry
Switch package + Complex adapter
Complex package + Switch adapter
```

### 11.3 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p complex_shooter_project_runtime -- --nocapture
cargo test -p switch_puzzle_project_runtime -- --nocapture
cargo check -p complex_shooter_player -p switch_puzzle_player -p empty_project_player
cargo test -p project_e2e_gate project_runtime_session_ -- --nocapture
cargo test -p project_e2e_gate second_project_runtime -- --nocapture
```

### 11.4 退出条件

```text
至少一个非塔防项目完成真实 AUI action -> session -> fixed -> snapshot 链。
Complex/Empty 显式 no-op 兼容。
交叉 module mismatch 继续 fail closed。
Core 源码没有 ui.toggle-switch 或 puzzle component 语义。
```

## 12. Gate F - Affected Regression / Hygiene / Closure

### 12.1 Source inventory

复扫：

```powershell
rg -n "recruit|deployUnit|mergeUnits|sellUnit|unlockCell|selectAugment|tower.defense" rust/crates/engine_runtime rust/crates/runtime_player_winit rust/crates/editor_core rust/crates/project_e2e_gate
rg -n "WorldWriteApi" rust/crates/engine_runtime/src/project_runtime_session.rs
rg -n "aui_actions|AuiAction" rust/crates/engine_runtime/src/logic_executor.rs
```

允许：

```text
通用 AuiAction 类型引用。
测试描述中的禁止项说明。
```

阻断：

```text
Engine Core 项目专用 action/component。
session public context 暴露 WorldWriteApi。
LogicContext 新增 AUI accessor。
```

### 12.2 Affected regression

按顺序执行，前一项失败先修，不把后续全跑当诊断：

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --all -- --check
cargo test -p engine_runtime
cargo test -p complex_shooter_project_runtime
cargo test -p switch_puzzle_project_runtime
cargo test -p runtime_player_winit
cargo test -p editor_core editor_gameview -- --nocapture
cargo test -p project_e2e_gate project_runtime_session_ -- --nocapture
cargo test -p project_e2e_gate second_project_runtime -- --nocapture
cargo check --workspace --all-targets --all-features
```

说明：

```text
不运行 cargo test --workspace --all-features，因为 260 的 runtime public interface closure 已由
定向测试和 workspace all-target/all-feature compile 覆盖；与 AI Gateway、Editor UI 或
其它无关 crate 的全测试不为本次 claim 增加独立证据。
若施工实际修改扩大到其它 consumer，再把其 owner test 加入本文后重新自审。
```

### 12.3 不需要的 release assurance

260 完成 claim 不包括：

```text
安装二进制
production Editor replacement
真实窗口像素
真实用户配置
Local CI
clean exact commit
发布包签名
```

因此不运行对应 Gate。若用户后来要求 release/production qualification，必须单独授权并生成
相称的补充施工文档，不能在 Gate F 临时扩大。

### 12.4 完成同步

所有 Gate 完成后：

```text
生成 阶段完成记录/2026-07-30-Project-Runtime-Session-AUI-Intent-Dispatch-Fixed-Step-Lifecycle-v1/00-总览.md。
记录实际 source identity、测试命令、通过数量、失败与重跑。
更新 260 正式方案状态为已完成施工。
更新 240 第 8/19 节施工状态。
更新 00-文档地图、00-阅读顺序、49、54。
更新 施工文档/README 和 阶段完成记录/README。
把本文从 当前/ 移到 已完成/。
确认 当前/ 与 待执行/ 均符合队列状态。
```

只有完成上述闭环，才允许回到塔防 skill 继续 P0-5 项目方案 C。

## 13. Diagnostic / report 固定集合

名称可因现有命名规范做窄调整，但语义必须覆盖：

```text
project_runtime.session_missing
project_runtime.session_duplicate
project_runtime.session_factory_failed
project_runtime.session_id_missing
project_runtime.session_faulted
project_runtime.aui_action_unhandled
project_runtime.aui_action_rejected
project_runtime.aui_batch_discarded_non_advancing
project_runtime.mutation_unsupported
project_runtime.mutation_preflight_failed
project_runtime.mutation_commit_failed
project_runtime.session_reentry_after_fault

project-runtime-bind-receipt.v2 或等价兼容升级
project-runtime-session-report.v1
project-runtime-session-e2e-report.v1
```

Report 必须区分：

```text
runtime/editor consumer
Off/Summary/Trace
action dispatch/fixed update stage
staged/committed/rejected mutation
recoverable action rejection/terminal session fault
```

## 14. 预计涉及文件

### 14.1 Engine owner

```text
rust/crates/engine_runtime/src/project_runtime_session.rs（新增）
rust/crates/engine_runtime/src/project_runtime_module.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/world_api.rs
rust/crates/engine_runtime/src/gameplay_command.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
rust/crates/engine_runtime/src/lib.rs
rust/crates/engine_runtime/src/domain/*
```

若 `project_runtime_session.rs` 超过 1000 行，测试优先迁入同 owner 的
`project_runtime_session/tests.rs`，不新建平行架构层。

### 14.2 Consumer

```text
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_window_winit/src/linked_project_runtimes.rs（仅编译/必要装配）
rust/crates/runtime_cli/src/lib.rs（仅当 typed parts 透传需要）
```

### 14.3 Project adapters / players

```text
rust/project_modules/complex_shooter/src/lib.rs
rust/project_modules/switch_puzzle/src/lib.rs
rust/project_players/complex_shooter/src/main.rs（预期仅编译）
rust/project_players/switch_puzzle/src/main.rs（预期仅编译）
rust/project_players/empty/src/main.rs（预期仅编译）
```

### 14.4 E2E

```text
rust/crates/project_e2e_gate/src/project_runtime_session.rs（新增或等价 owner）
rust/crates/project_e2e_gate/src/second_project_runtime.rs
rust/crates/project_e2e_gate/src/lib.rs
samples/switch_puzzle_project/AUI/puzzle.aui.json（仅 Gate E 确认需要时）
```

### 14.5 文档

```text
框架设计/引擎总体架构/260-Project-Runtime-Session-AUI-Intent-Dispatch-Fixed-Step-Lifecycle-v1方案.md
框架设计/引擎总体架构/240-5.6审查剩余问题讨论与施工优先级.md
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

列表外文件只有在 owner/consumer closure 明确需要时才允许加入；必须先写回本文并重新自审。

## 15. 工作树与外部状态规则

```text
保留所有用户未提交改动。
不 reset/checkout/revert unrelated changes。
不创建 repository-external qualification worktree。
不删除当前目录残留的历史 254 artifact。
测试不得写真实 Codex config、LOCALAPPDATA installation 或 production project。
需要临时文件时使用测试拥有的唯一 temp root，并由测试 owner 清理。
不启动 production Editor 或真实窗口。
不生成 tower defense project 文件。
```

Gate 失败时：

```text
保留第一失败的 assertion/diagnostic/stdout/stderr。
先用当前 Gate owner 测试复现。
分类为方案、施工文档、实现、环境或旧基线。
修复后只重跑当前 Gate 和受影响 consumer。
不使用 workspace check 作为调试循环。
```

## 16. 激活前复核条件

必须全部满足：

```text
260 正式方案仍是当前有效方案。
240 第 19 节仍指向本文。
施工文档/当前/ 没有匹配 `*-当前可自动化施工文档-*.md` 或被 54 声明为 active 的施工计划。
当前目录遗留的 254 report/artifact 不计作施工计划，但不得修改、删除或复用。
本文是待执行队列中最高优先级文档。
涉及 Rust owner 的新改动已分类并可安全合并。
Gate 命令与当前 Cargo package/filter 名称匹配。
窗口 1 可以在三小时内完成并留出测试/报告时间。
用户明确授权开始 260 窗口 1。
```

通过后执行：

```text
把本文从 待执行/ 移到 当前/。
更新 54：唯一当前施工=260，窗口 1，Gate A。
更新 240/49/施工 README。
再次做施工文档自审。
然后才开始 Gate A 红测试。
```

## 17. 施工文档自审

### 17.1 与 260 正式方案一致

通过。

```text
Gate A 覆盖 session factory/bind/lifecycle。
Gate B 覆盖 deferred mutation 和零副作用 preflight。
Gate C 覆盖 exactly-once、fixed order、fault/report。
Gate D 覆盖 Player/Editor consumer。
Gate E 覆盖第二项目和完整 action-to-snapshot 链。
Gate F 覆盖受影响回归和文档闭环。
```

没有把 E1 LogicContext 注入或 E2 独立 handler 混回方案。

### 17.2 是否保持深 module

通过。

公共复杂度收敛在 `project_runtime_session`；registration/bootstrap 和 host 只做装配、调度。
没有新增 Router、通用 EventBus 或 phase mirror。

### 17.3 是否保持项目/引擎边界

通过。

Engine API 没有塔防概念；塔防目录只读。Switch Puzzle 只作为现有第二项目通用性证明，
项目语义留在项目 module。

### 17.4 是否保持规则热更新边界

通过。

本文只实现项目 Rust session 入口和复杂不变量，不迁移普通 RuleStatement，也不扩 IR 或
脚本系统。

### 17.5 Mutation 合同是否自洽

通过，已吸收方案自审修正。

```text
session 只读 World。
写入进入 typed deferred batch。
prepare 零副作用。
commit 失败 terminal fault。
不使用 whole-World-per-tick clone。
不伪造项目内部状态回滚。
```

### 17.6 测试是否与 claim 匹配

通过。

定向 owner 测试、Player/Editor consumer、第二项目 e2e 和 workspace all-feature compile 覆盖
shared interface closure。没有要求无关 AI/Editor UI 全测试，也没有用真实窗口/Local CI
伪装必要证据。

### 17.7 三小时限制

通过。

工程范围被拆成两个独立授权窗口。窗口 1 不自动进入窗口 2；每个窗口在 2:45 停止启动新
工作，并在三小时硬截止前保存证据和状态。

### 17.8 当前是否可以施工

施工已完成，不可继续使用本授权施工。

```text
激活复核：通过。
结论：窗口 1 Gate A-C、窗口 2 Gate D-F 均已完成并通过受影响回归。
当前范围：无。
当前 Gate：无。
施工授权：已结束。
```
