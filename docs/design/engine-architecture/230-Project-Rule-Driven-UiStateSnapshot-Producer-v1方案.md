# 230-Project Rule Driven UiStateSnapshot Producer v1 方案

> 状态：正式方案，用户已确认采用 `B-min + ActiveBindingDriven + CachedProducer + C-report`。  
> 校准日期：2026-07-09。  
> 所属路线：`227` 的 P0-3。  
> 前置：`199` 已完成 ProjectUiStateSnapshot Producer C-min；`229` 已完成复杂打飞机玩法规则真实 runtime 执行。  
> 本文只生成方案，不允许直接施工；施工前仍需审查/自审和施工文档。

## 0. 用户确认结论

本系统确认采用：

```text
B-min + ActiveBindingDriven + CachedProducer + C-report
```

含义：

```text
B-min:
  项目侧真实 UiState producer 读取 runtime World / project state，
  替换 ComplexShooterSampleUiStateProducer 的样例/伪数据逻辑。

ActiveBindingDriven:
  只根据当前 active AUI Document 实际声明的 binding paths 生产 UI state。

CachedProducer:
  producer 持久化并复用上一帧 snapshot/cache；
  后续所有复杂 UI state 默认走 dirty / cached。

C-report:
  用最小结构化报告证明 snapshot_source、producer_id、produced_paths、
  missing_paths、dirty_domains、cache_status 和是否仍使用样例/伪数据。
```

核心红线：

```text
ProjectUiStateSnapshotProducer 不是全量 UI 镜像器。
P0-3 可以让复杂打飞机小 HUD 每帧轻量刷新。
但不能设计成每帧全量生成全项目 UI state。
复杂 UI 后续一律按 active binding path + dirty domain + cached value 收敛成本。
```

## 1. 一句话说明

这个系统让复杂打飞机 HUD 从：

```text
AUI 能显示，但分数/血量/波次仍来自 sample/static producer
```

变成：

```text
HUD 显示的分数、血量、波次、敌人数来自真实 Project Rule / ECS runtime state。
```

它在本引擎里的位置：

```text
Project Rule / ECS World
  -> Project-owned UiStateSnapshotProducer
  -> ProjectUiStateSnapshot
  -> AUI Binding Resolve
  -> AuiLayout / AuiDrawList
  -> RuntimeRenderer UI Pass
  -> Present
```

它不是新 UI 框架，也不是 IR 扩展；它是把真实 gameplay state 转成 AUI 可读 UI ReadModel 的项目侧 Rust 能力。

## 2. 为什么现在做它

`227` 的 P0 顺序是：

```text
P0-1 Real Texture Decode / GPU Texture Upload / Sprite Textured Present v1
P0-2 Complex Shooter Gameplay Rule Runtime Execution v1
P0-3 Project Rule Driven UiStateSnapshot Producer v1
P0-4 Exported Windows Playable Golden Gate v1
```

当前状态：

```text
228 已让真实 PNG / Sprite texture 进入 RuntimePackage 和 real wgpu present。
229 已让复杂打飞机规则真实运行：移动、开火、spawn/despawn、碰撞、扣血、计分。
199 已有 ProjectUiStateSnapshotProducer trait / report / AUI present 接入。
```

剩余缺口：

```text
ComplexShooterSampleUiStateProducer 仍偏 C-min 样例。
score 当前可能按 frame_index 伪造。
hp_ratio 可能只看玩家实体是否存在。
wave / enemy_count 可能不是来自真实 project.sessionState。
```

所以 P0-3 的目标不是重新发明 ProjectUiStateSnapshot，而是把 199 的 producer 基线升级为：

```text
真实 runtime state driven
active binding path driven
dirty / cached
可报告、可测试、可被 P0-4 golden gate 验证
```

## 3. 其它引擎对标

### Unity

对标：

```text
UI Toolkit runtime binding / data source。
UGUI 中项目脚本或 ViewModel 更新 Text / Image / Slider 等 UI 组件。
```

源码参考：

```text
<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\Graphic.cs
<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\CanvasUpdateRegistry.cs
<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\Text.cs
<UNITY_UI_REFERENCE>\com.unity.ugui\Runtime\UGUI\UI\Core\Image.cs
```

可学习点：

```text
UI 显示层有 dirty / rebuild 机制。
UI 属性变化不应导致整个 UI 全量重建。
项目侧代码负责把 GameState 转成 UI 可显示状态。
```

不可照搬点：

```text
Unity UGUI 的 UI 节点是 GameObject / Component。
本引擎 AUI 真相是 AUI Document，不让 AUI node 变成 runtime ECS gameplay entity。
```

### Unreal Engine

对标：

```text
UMG ViewModel / View Binding。
项目代码或 Blueprint 更新 ViewModel，UMG 绑定 ViewModel 字段。
```

可学习点：

```text
UI 绑定 read model，而不是 UI widget 直接读取底层 gameplay 系统。
ViewModel 边界适合复杂 UI 长期维护。
```

不可照搬点：

```text
UE 的 Blueprint / UObject 反射系统很重。
本引擎 v1 不新增大型 UObject/Blueprint 式 UI ViewModel 系统。
```

### Godot / Bevy

对标：

```text
Godot: 脚本 / signals 把游戏状态更新到 Control tree。
Bevy: Systems 查询 state 并更新 UI Node / Text / Interaction 相关组件。
```

可学习点：

```text
项目系统负责 UI state 同步。
UI runtime 本身不应吞下复杂业务聚合。
```

不可照搬点：

```text
本项目 AUI Binding 只读 ProjectUiStateSnapshot；
不让 AUI Binding 直接 query ECS，也不让 UI node 直接成为 gameplay ECS entity。
```

## 4. 当前本项目基线

已有能力：

```text
engine_runtime::aui::ProjectUiStateSnapshot
engine_runtime::aui::ProjectUiStateSnapshotProducer
engine_runtime::aui::ProjectUiStateProducerContext
engine_runtime::aui::ProjectUiStateSnapshotOutput
engine_runtime::aui::ProjectUiStateSnapshotReport
AuiRuntimePresenter::present_project_snapshot_with_font_atlases
runtime_player_winit / editor_gameview_play 中已有 producer 调用点
```

关键代码位置：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/editor_core/src/editor_gameview_play.rs
```

当前问题：

```text
ComplexShooterSampleUiStateProducer 名称和行为仍像 sample。
producer 在 runtime/player/editor present 时临时 new，不能保存 cache。
输出值仍可能带样例语义，例如 frame_index 推 score。
没有证明 HUD score/hp/wave 来自 229 真实 rule writes。
没有 active binding path 输入，容易滑向全量 UI snapshot。
dirty/cache 还只是方案约束，没有进入 report 证据。
```

229 已提供真实数据来源：

```text
project.sessionState.score
project.sessionState.wave
project.combatState.hp
project.combatState.team
project.combatState.scoreValue
Physics2D collision evidence
GameplayRuleRuntimeExecutionReport
```

## 5. 范围

本轮做：

```text
正式 project-owned complex shooter runtime UI state producer。
读取真实 World / project dynamic components。
按 active AUI binding paths 生成 ProjectUiStateSnapshot。
producer 持久化并缓存上一帧值。
dirty/cache summary 进入 ProjectUiStateSnapshotReport 或 wrapper report。
runtime_player_winit / editor_gameview_play 改用真实 producer。
project_e2e_gate 证明 score/hp/wave 与真实 runtime state 对齐。
```

本轮不做：

```text
完整自走棋装备/商店 UI read model。
完整 UI schema editor。
大型 ViewModel 资产系统。
IR 解释 UI 聚合逻辑。
任意脚本语言。
全量 dirty graph。
跨线程 UI state cache。
真实 Windows golden gate；这属于 P0-4。
```

## 6. 正式链路

目标链路：

```text
RuntimePackage loaded
  -> RuntimeScene hydrated into World
  -> ProjectLogicRunner executes rules
  -> project.sessionState / project.combatState updated
  -> Active AUI Document declares binding paths
  -> UiStateSnapshotProducer receives active_binding_paths
  -> Producer reads only required project state
  -> Producer reuses cached values for unchanged paths
  -> ProjectUiStateSnapshotOutput(snapshot, report)
  -> AUI Binding Resolve
  -> AuiRuntimePresentOutput includes ui_state_snapshot_report
  -> runtime_player_winit / project_e2e_gate reports evidence
```

禁止链路：

```text
AUI Binding -> ECS World
AUI Runtime Core -> project.sessionState
Renderer -> ProjectUiStateSnapshot
ProjectUiStateSnapshotProducer -> all UI data every frame
engine_runtime public API -> score/hp/wave 专用字段
```

## 7. 数据与接口建议

### 7.1 Producer 名称

当前 `ComplexShooterSampleUiStateProducer` 应收敛为更明确的项目侧 producer：

```text
ComplexShooterRuntimeUiStateProducer
```

语义：

```text
sample:
  只允许 smoke / fixture。

runtime:
  必须读取真实 RuntimePackage / World 状态。
```

如为了兼容测试保留旧类型，旧类型必须降级为 wrapper 或 deprecated fixture，不能继续伪装成真实 runtime producer。

v1 取舍：

```text
新增 ComplexShooterRuntimeUiStateProducer。
保留 ComplexShooterSampleUiStateProducer，仅用于 smoke / fixture / 199 兼容测试。
runtime_player_winit / editor_gameview_play / P0-3 e2e gate 必须切到 runtime producer。
旧 sample producer 不删除，避免破坏 199 既有 smoke 测试；但不得作为真实项目 UI state 证据。
```

### 7.2 Producer 生命周期

正式 producer 应持久化：

```text
player/editor play session 创建一次。
每帧调用 produce。
内部保存 previous_source_fingerprints 和 previous_snapshot values。
```

不推荐：

```text
每帧 new producer。
每帧丢弃 cache。
每帧生成与当前 AUI 无关的全部 UI paths。
```

P0-3 可以接受实现上的最小持久化：

```text
runtime_player_winit headless/window loop 中持有 producer。
editor_gameview_play session 中持有 producer。
测试 fixture 可继续临时构造。
```

生命周期规则：

```text
producer cache 属于当前 player loop / EditorRuntimePlayInstance。
Stop Play 或 player loop 结束时 producer 随 instance drop。
cache 不跨 Play Session 复用，避免上一轮 Play 的 UI state 污染下一轮。
```

### 7.3 Producer Context

当前 `ProjectUiStateProducerContext` 已有：

```text
frame_index
package
world
```

v1 只在现有 context 基础上新增：

```text
active_binding_paths
report_mode
```

后续可扩展：

```text
dirty_domains
runtime_trace_summary
input_ui_summary
previous_snapshot_cache
```

v1-min 不要求一次做完整 dirty graph；可以通过 source fingerprint 与上一帧缓存比较来得到 cache hit/miss。

active binding path 提取规则：

```text
v1 取 active = 当前 HUD AUI Document 全部 declared binding paths。
原因是复杂打飞机当前只有一个 HUD document，visible screen 子树和 declared paths 等价。
后续多 screen / modal / hidden panel 再收敛为 visible subtree binding paths。
```

注入时机：

```text
producer 在 produce 时从 context.package 的当前 active AUI Document 提取 binding paths。
不要为了 v1 改造 build_aui_present_output 的调用签名。
presenter 之后仍可生成 declared_binding_paths report，两者语义不同：
  active_binding_paths = producer 跑前的输入约束；
  declared_binding_paths = presenter 跑后从 document 汇总出的覆盖报告。
```

### 7.4 Snapshot Output

输出仍是：

```text
ProjectUiStateSnapshotOutput
  snapshot: ProjectUiStateSnapshot
  report: ProjectUiStateSnapshotReport
```

当前 `ProjectUiStateSnapshotReport` 已有：

```text
producer_id
snapshot_source
value_count
produced_paths[]
declared_binding_paths[]
missing_paths[]
type_mismatch_paths[]
diagnostics[]
```

v1 在现有 report 上扩展，或通过 wrapper report 表达新增字段：

```text
active_binding_paths[]
dirty_domains[]
cache_status
cache_hit_paths[]
cache_miss_paths[]
source_paths[]
```

字段关系：

```text
active_binding_paths:
  producer 输入侧，表示本帧应生产哪些 path。

declared_binding_paths:
  presenter 输出侧，表示 AUI Document 实际声明了哪些 path。

produced_paths:
  snapshot 实际给出的 path。

type_mismatch_paths:
  已存在，继续由 binding report 填充。
```

Report 分档：

```text
Off:
  runtime 热路径默认不写完整 JSON。

Summary:
  producer_id / status / value_count / cache_status / dirty_domains / missing_count。

Trace:
  active_binding_paths / produced_paths / source_paths / cache_hit_paths / cache_miss_paths / diagnostics。
```

## 8. 复杂打飞机 HUD 映射

P0-3 最小必须覆盖：

```text
game.score_text
player.hp_ratio
game.paused
player.ship_icon
game.wave_text
game.enemy_count_text
```

建议来源：

| Binding path | 来源 | 输出 |
|---|---|---|
| `game.score_text` | `entity-session-state/project.sessionState.score` | `SCORE 001200` |
| `game.wave_text` | `entity-session-state/project.sessionState.wave` | `WAVE 3` |
| `player.hp_ratio` | `entity-player/project.combatState.hp` + max hp cache | `0.0..1.0` |
| `game.enemy_count_text` | `project.combatState.team == "enemy"` 的 alive entity count | `"3"` |
| `game.paused` | runtime/player pause state；v1 无数据时 fallback false 并 report | `bool` |
| `player.ship_icon` | package asset id / stable AUI asset ref | `tex-player-ship` |

`player.hp_ratio` 的 v1-min 规则：

```text
优先读取 maxHp 字段。
如果项目当前没有 maxHp，则 producer 在首次看到 player hp 时记录 initial_player_hp 作为 max hp cache。
如果 player entity 不存在，则 hp_ratio = 0.0，并写入 diagnostic 或 source_status=missing_player。
```

v1 限制：

```text
复杂打飞机 P0-3 假设无 player respawn 或 maxHp 不变。
respawn 后动态 maxHp、换机导致 maxHp 变化等能力 deferred。
后续需要通过显式 maxHp 字段或 player profile source fingerprint 解决。
```

`game.score_text` 的验收规则：

```text
不能再用 frame_index 推导分数。
必须读取 229 collision-response 写入后的 project.sessionState.score。
project_e2e_gate 应证明 score_after 与 snapshot 中的 game.score_text 一致。
```

## 9. Dirty / Cached 规则

全局规则：

```text
所有后续 UI state 默认 dirty / cached。
```

P0-3 最小实现可以采用 source fingerprint：

```text
source_fingerprint(path) =
  该 binding path 实际读取的 project component field / asset id / runtime flag 的值摘要。

如果 fingerprint 不变：
  复用上一帧 cached AuiBindingValue。

如果 fingerprint 变化：
  重新计算该 binding path。
```

v1 dirty/cache 收敛：

```text
v1 不实现独立显式 dirty graph。
fingerprint 变化即视为该 path dirty。
dirty_domains 在 report 中由 fingerprint 变化的 source path / binding path 汇总得到。
显式 dirty graph 留给后续复杂 UI 系统。
```

复杂 UI 后续扩展时再引入更细 dirty domains：

```text
gameplay_dirty
inventory_dirty
shop_dirty
equipment_dirty
selection_dirty
screen_flow_dirty
localization_dirty
asset_dirty
```

运行时热路径规则：

```text
小 HUD 可以每帧检查少量 source fingerprints。
大型列表 / 背包 / 商店 / tooltip 不允许每帧排序和全量格式化。
不可见 screen / 未声明 binding path 不生成 UI state。
```

## 10. AI 与用户维护规则

AI 修 bug 时按以下路径定位：

```text
HUD 文本位置、样式、绑定 path 错：
  改 AUI Document。

HUD 值没有出现：
  看 ProjectUiStateSnapshotReport 的 missing_paths / produced_paths。

HUD 值出现但不真实：
  查 producer source_paths 和 source_fingerprint。

分数真实状态没变：
  查 Project Rule / GameplayRuleRuntimeExecutionReport。

分数真实状态变了但 HUD 没变：
  查 UiStateSnapshotProducer dirty/cache 和 binding report。

UI 渲染命令有值但屏幕不显示：
  查 AUI present / glyph / renderer / UI pass。
```

用户心智：

```text
AUI Document:
  这里有一个文本，它绑定 game.score_text。

Project Rule:
  子弹打中敌人后，project.sessionState.score 增加。

UiStateSnapshotProducer:
  把 score 数字格式化成 "SCORE 001200" 给 AUI。
```

## 11. 测试与验收

P0-3 必须新增或更新 gate：

```text
complex_shooter_project_rule_driven_ui_state_snapshot
```

必须证明：

```text
RuntimePackage 加载真实 AUI Document。
ProjectLogicRunner 运行至少到 collision-response 计分。
World 中 project.sessionState.score > 0。
ProjectUiStateSnapshot 中 game.score_text 与 score_after 一致。
player.hp_ratio 来自 entity-player/project.combatState.hp 或 missing_player 状态。
game.wave_text 来自 project.sessionState.wave。
snapshot_source = ProjectProducer。
producer_id = complex_shooter_runtime_ui_state。
active_binding_paths 不为空。
produced_paths 覆盖当前 AUI binding paths。
missing_paths 为空，或有明确 fallback diagnostic。
cache_status / dirty_domains 进入 Summary 或 Trace report。
没有使用 PackageSmokeSnapshot 伪装真实项目 UI。
没有 frame_index score 伪数据。
```

推荐测试命令：

```powershell
cargo fmt --check
cargo test -p engine_runtime aui
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate complex_shooter_project_rule_driven_ui_state_snapshot
cargo test -p project_e2e_gate complex_shooter_gameplay_rule_runtime
cargo test -p project_e2e_gate
```

## 12. 后续施工建议

Gate A：收敛 producer contract

```text
在现有 ProjectUiStateProducerContext(frame_index/package/world) 基础上新增 active_binding_paths / report_mode。
保留 199 兼容路径，避免一次性破坏现有 tests。
active_binding_paths v1 由 producer 从 context.package 的 HUD document 提取。
旧 ComplexShooterSampleUiStateProducer 保留为 smoke / fixture，真实 runtime 不再使用它。
```

Gate B：实现持久化 runtime producer

```text
新增 ComplexShooterRuntimeUiStateProducer。
读取真实 project.sessionState / project.combatState。
内部保存上一帧 cache / source fingerprints。
producer cache 只属于当前 player loop / EditorRuntimePlayInstance，Stop 后 drop。
v1 假设无 player respawn 或 maxHp 不变。
```

Gate C：接入 runtime_player_winit / editor_gameview_play

```text
build_aui_present_output 不再每次 new sample producer。
play session 或 player loop 持有 producer。
PackageSmokeSnapshot 只保留 smoke/test。
```

Gate D：结构化 report

```text
在现有 ProjectUiStateSnapshotReport 基础上扩展或用 wrapper report 输出 active_binding_paths / cache_status / dirty_domains / source_paths。
Report 分档遵守 Off / Summary / Trace。
v1 只做 fingerprint-based cache；dirty_domains 汇总 fingerprint 变化，不做独立 dirty graph。
```

Gate E：complex shooter e2e

```text
先跑 229 gameplay rule runtime gate。
再跑 P0-3 UI snapshot gate。
证明 score_after 与 game.score_text 对齐。
```

Gate F：文档同步

```text
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
完成后写阶段完成记录并归档施工文档。
```

## 13. 自审

是否符合 199：

```text
符合。继续使用 ProjectUiStateSnapshotProducer / ProjectUiStateSnapshot / report。
本方案不是替代 199，而是把 199 的 C-min 升级到 P0-3 的真实项目状态驱动。
```

是否符合 227：

```text
符合。P0-3 的目的就是让 HUD 分数、血量、波次来自真实规则/ECS 状态。
```

是否符合 195 / 196 的项目逻辑边界：

```text
符合。复杂 UI 聚合在 Rust Project Module / Project Framework；
AUI Binding 只读 snapshot；
具体 score/hp/wave 不进入 engine_runtime 通用 API。
```

是否避免新增层数：

```text
是。Dirty/cache 是 producer 执行策略，不是新增架构层。
Active binding path 是 AUI Document 已有 binding 的输入约束，不是新系统层。
```

是否 AI 友好：

```text
是。report 能告诉 AI：
  哪些 binding path 被声明；
  哪些 path 被生产；
  值来自哪些 project source path；
  哪些 path 命中 cache；
  哪些 path missing 或 type mismatch。
```

是否有性能约束：

```text
有。禁止全项目 UI state 每帧全量生成；
复杂 UI 后续默认 dirty / cached；
runtime report 默认 Off / Summary，不把 Trace 常驻热路径。
```

## 14. 外部审查吸收

已读取：

```text
其它AI审查目录/43-230-Project-Rule-Driven-UiStateSnapshot-Producer方案审查.md
```

审查对象与本方案一致，审查日期为 2026-07-09。吸收分类如下。

必须修改，已写回本方案：

```text
MF-1:
  §7.3 修正为“保留现有 frame_index / package / world，
  只新增 active_binding_paths / report_mode”。

MF-2:
  §7.4 修正为“扩展现有 ProjectUiStateSnapshotReport 或 wrapper report”，
  明确 producer_id / snapshot_source / value_count / produced_paths /
  declared_binding_paths / missing_paths / type_mismatch_paths / diagnostics 已存在；
  v1 新增 active_binding_paths / dirty_domains / cache_status /
  cache_hit_paths / cache_miss_paths / source_paths。
```

施工约束，已写入本方案并必须进入施工文档：

```text
SC-1:
  active_binding_paths v1 由 producer 从 context.package 的 HUD AUI Document 提取，
  不为了 v1 改造 build_aui_present_output 签名。

SC-2:
  producer cache 属于当前 player loop / EditorRuntimePlayInstance，
  Stop Play 或 loop 结束后 drop，不跨 Play Session 复用。

SC-3:
  v1 active = 当前 HUD AUI Document 全部 declared binding paths；
  多 screen / hidden subtree 的可见子树过滤 deferred。

SC-4:
  v1 假设无 player respawn 或 maxHp 不变；
  动态 maxHp / respawn 处理 deferred。

SC-5:
  v1 只做 fingerprint-based cache；
  dirty_domains 由 fingerprint 变化汇总，不做独立 dirty graph。

SC-6:
  旧 ComplexShooterSampleUiStateProducer 保留为 smoke / fixture；
  runtime/player/editor 切到 ComplexShooterRuntimeUiStateProducer。
```

不适用或不纳入本轮：

```text
审查提到的 223-229 历史闭环债不属于 230 功能实现范围。
本轮只同步 230 自己的 49 / 54 / 施工文档 README / 阶段完成记录 README。
如发现 229 阶段记录已存在，则施工文档中标记为“前置已满足”。
```

## 15. 结论

正式采用：

```text
Project Rule Driven UiStateSnapshot Producer v1
  = project-owned runtime UiStateSnapshotProducer
  + active binding path driven
  + dirty / cached UI state
  + minimal coverage/source/cache report
```

下一步：

```text
如有其它 AI 审查文档，先读取审查并判断是否修改本方案。
如无审查文档，可进入施工文档生成：
施工文档/当前/230-当前可自动化施工文档-Project-Rule-Driven-UiStateSnapshot-Producer-v1.md
```
