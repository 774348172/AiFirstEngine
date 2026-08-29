# 199-Game UI MVVM ReadModel / AUI ProjectUiStateSnapshot Producer v1 方案

正式名称：

```text
Game UI MVVM ReadModel / ProjectUiStateSnapshot Producer v1
```

短名：

```text
ProjectUiStateSnapshot Producer v1
```

中文心智：

```text
游戏 UI 的 MVVM ReadModel 生产器。
```

本名称只作为理解模型，不把项目改造成标准桌面 MVC / MVVM 框架。

## 1. 系统定义

本系统解决一个问题：

```text
AUI Document 已经能进入 RuntimePackage，也能 Binding / Layout / Present。
但 Binding 当前主要依赖 package_smoke_snapshot。
真实项目运行时，谁把 gameplay / UI / project state 转成 ProjectUiStateSnapshot？
```

一句话：

```text
ProjectUiStateSnapshot Producer 是项目侧 Rust 读模型生产器。
它把运行时项目状态整理成 AUI Binding 可读取的扁平 snapshot。
```

按 MVVM 类比：

```text
ECS / Gameplay State / Project Runtime State
  ~= Model

ProjectUiStateSnapshotProducer
  ~= ViewModel Producer / Presenter Adapter

ProjectUiStateSnapshot
  ~= ViewModel / ReadModel

AUI Document
  ~= View 声明 / UI Layout

AUI Binding
  ~= View 绑定 ViewModel 字段

AUI Runtime / Renderer
  ~= View 渲染执行层
```

重要限制：

```text
这是“游戏引擎版 MVVM ReadModel”，不是标准桌面 MVVM。
AUI Document 不直接读 Model。
AUI Runtime 不直接读 ECS。
ProjectUiStateSnapshot 是 UI 能读取的唯一运行时数据入口。
复杂 UI 状态聚合由 Rust Project Module / Project Framework 承担。
```

它不是：

```text
不是 AUI Document。
不是通用 UI 脚本系统。
不是 IR 运行时解释器。
不是 AUI Runtime 直接读 ECS 的后门。
不是 Logic Ownership Router / Architecture Guard 新层。
```

它是：

```text
Rust Project Framework / Project Rust Module 的一个可注册能力。
AUI Binding 的唯一运行时数据输入。
复杂 UI 工作流和复杂聚合逻辑的 Rust 承载点。
频繁变化、输入输出固定、可批量验证的项目侧 UI 热逻辑，可以从 producer 中切出为 Project UI Hot Logic Module / Rust-authored WASM。
```

## 2. 对标说明

### Unity

Unity UI Toolkit runtime binding 的核心思路是把 UI control 属性绑定到 C# object / data source，而不是让 UI 文档自己计算复杂业务状态。

参考：

```text
https://docs.unity3d.com/6000.0/Documentation/Manual/UIE-runtime-binding.html
https://docs.unity3d.com/6000.0/Documentation/Manual/UIE-runtime-binding-define-data-source.html
```

对本项目的启发：

```text
AUI Document 只声明绑定路径。
ProjectUiStateSnapshot 类似 runtime data source。
复杂数据准备在 Rust 项目模块中完成。
```

### Unreal Engine

UE 的 UMG Viewmodel 让 UI 绑定到 Viewmodel 字段，程序侧负责把 Viewmodel 接到应用代码；UMG 属性绑定也把 Widget 属性绑定到 Blueprint 的函数或变量。

参考：

```text
https://dev.epicgames.com/documentation/unreal-engine/umg-viewmodel-for-unreal-engine
https://dev.epicgames.com/documentation/unreal-engine/property-binding-for-umg-in-unreal-engine
https://dev.epicgames.com/documentation/unreal-engine/understanding-the-slate-ui-architecture-in-unreal-engine
```

对本项目的启发：

```text
ProjectUiStateSnapshot 是项目 UI ViewModel。
AUI Binding 只读它。
复杂 UI 逻辑由 Rust Project Module / Project Framework 更新这个 read model。
```

### Godot

Godot UI 是 Control node tree，UI 输入通过 Control / Viewport 体系过滤，业务响应通常走信号和脚本。

参考：

```text
https://docs.godotengine.org/en/stable/classes/class_control.html
https://docs.godotengine.org/en/stable/getting_started/step_by_step/signals.html
```

对本项目的启发：

```text
UI tree / input propagation 是 UI framework 的职责。
业务状态与 UI 展示之间需要清晰的事件 / 数据边界。
```

### Bevy

Bevy UI 用 Node 描述布局，Interaction 表示 UI 节点输入状态，项目系统查询这些组件并更新应用状态。

参考：

```text
https://docs.rs/bevy/latest/bevy/ui/struct.Node.html
https://docs.rs/bevy/latest/bevy/ui/enum.Interaction.html
```

对本项目的启发：

```text
UI runtime 可以是数据驱动。
项目系统负责把 gameplay state 和 UI state 转换成界面可读数据。
```

## 3. 当前本项目基线

已具备：

```text
engine_runtime::aui::ProjectUiStateSnapshot
AuiBindingRef target_field / path / fallback
AuiRuntimeResolver::resolve_bindings(document, snapshot)
AuiRuntimePresenter::present(document, snapshot_source, snapshot)
AuiOverlayFrame -> RuntimeRenderer UI Pass
RuntimePackage AUI document load / present report
```

当前缺口：

```text
AuiRuntimePresenter::package_smoke_snapshot 仍是固定测试数据。
没有正式 ProjectUiStateSnapshotProducer。
没有 snapshot schema / diagnostics / source report。
复杂打飞机 HUD 还不能从真实运行时状态驱动。
自走棋商店 / 装备 / 羁绊 / 回合 UI 更不能只靠 smoke snapshot。
```

当前不能做：

```text
不能让 AUI Binding 直接读 ECS。
不能让 AUI Document 保存运行时值。
不能让 IR 负责复杂 UI 聚合逻辑。
不能新增 Logic Ownership Router / Architecture Guard。
不能把“复杂 UI 用 Rust”误读成“所有 UI 项目侧规则一律不能 IR”。
```

## 4. 方案选项

### 方案 A：AUI Runtime 直接从 ECS 生成 Snapshot

做法：

```text
AUI Runtime 读取 ECS / World / Project Rule 状态。
根据 binding path 自动解析到 Component 字段。
生成 ProjectUiStateSnapshot。
```

优点：

```text
短期接线快。
少一个项目侧 provider 注册点。
```

问题：

```text
违反 Binding 只读 ProjectUiStateSnapshot 的边界。
AUI Runtime 会知道 ECS 和项目语义。
复杂 UI 聚合会进入 AUI Core。
后续自走棋装备 / 商店 / 羁绊 UI 会把 AUI 变成业务逻辑容器。
```

结论：

```text
不采用。
```

### 方案 B：Rust ProjectUiStateSnapshotProducer trait

做法：

```text
engine_runtime 定义 ProjectUiStateSnapshotProducer trait。
Rust Project Module / Project Framework 实现 producer。
EngineHostLoop / player 在 AUI present 前调用 producer。
producer 输出 ProjectUiStateSnapshot 和 ProjectUiStateSnapshotReport。
```

优点：

```text
边界清晰。
复杂 UI 聚合逻辑留在 Rust 项目模块。
AUI Binding 仍只读 snapshot。
不新增运行时架构层。
适合复杂打飞机和自走棋。
```

问题：

```text
需要定义 provider 注册位置。
第一版需要一个 default / sample producer。
AI 修改复杂 UI 状态逻辑时会进入 Rust Project Module，不如纯数据编辑轻。
```

结论：

```text
推荐作为 v1 主方案。
```

### 方案 C：Snapshot Schema + Rust Producer 混合

做法：

```text
在方案 B 基础上增加 ProjectUiStateSchema / BindingPathManifest。
Schema 声明允许输出哪些 binding path、类型、来源域、fallback 策略。
Rust producer 负责计算值。
AUI / tests 用 schema 校验 snapshot 覆盖率和类型。
```

优点：

```text
AI 更容易审查和修改 AUI binding。
能发现 AUI Document 绑定了不存在的 path。
能为复杂 UI 提供稳定的长期治理。
```

问题：

```text
比方案 B 多一份项目资产。
如果第一版做太满，会变成新结构负担。
```

结论：

```text
推荐作为 v1 的最小增强：
只做 snapshot report 中的 declared_paths / produced_paths / missing_paths / type_mismatch。
暂不引入大型 schema 编辑器。
```

## 5. 正式推荐方案

采用：

```text
B-min + ActiveBindingDriven + CachedProducer + C-report
Rust ProjectUiStateSnapshotProducer
  + active binding path 驱动
  + dirty / cached UI state
  + 最小 BindingPath coverage report
```

本轮施工采用更明确的正式名：

```text
Game UI MVVM ReadModel / ProjectUiStateSnapshot Producer v1
```

v1 只完成 ReadModel 生产链路，不完成完整复杂 UI 工作流：

```text
做：
  producer contract
  producer context / output / report
  snapshot_source=ProjectProducer
  AUI present 接收 ProjectProducer snapshot
  runtime_player_winit / project_e2e_gate 输出证据
  complex shooter C-min sample producer
  active binding path 驱动的最小 HUD snapshot
  producer 持久化与上一帧 snapshot cache 约束
  dirty domain / cache status 写入轻量 report

不做：
  AUI drag/drop
  装备面板完整交互
  Transaction RuleSlot
  Project UI Hot Logic Module / WASM
  runtime text glyph_present
  任意 UI 脚本语言
  每帧全量生成全部 UI 状态
  把 ProjectUiStateSnapshot 当成全项目 UI 镜像
```

用户心智仍是两层：

```text
Rust Project Framework
  -> 负责 ProjectUiStateSnapshotProducer 和复杂 UI 状态聚合。

Project Assets
  -> AUI Document 声明 node / style / binding path / action id。
  -> Contract-bound UI RuleSlot 可声明简单 action mapping / display rule。
  -> 未来可有轻量 BindingPath manifest / report。
```

正式链路：

```text
RuntimePackage loaded
  -> World / Project Runtime State updated
  -> AUI Document declared binding paths collected
  -> Rust ProjectUiStateSnapshotProducer::produce(frame_context, active_binding_paths)
  -> ProjectUiStateSnapshot
  -> ProjectUiStateSnapshotReport
  -> AuiRuntimePresenter::present(document, ProjectProducer, snapshot)
  -> AuiBindingReport
  -> AuiLayout / AuiDrawList
  -> UiProjection / AuiOverlayFrame
  -> RuntimeRenderer UI Pass
```

### 5.1 UI state 性能总规则：Dirty + Cached

从本方案开始，所有后续 UI state 生产默认遵守：

```text
ProjectUiStateSnapshotProducer 不是全量游戏状态镜像器。
它只生产当前 active AUI Document 实际声明的 binding paths 所需数据。
所有复杂 UI 后续都必须走 dirty 标记和 cache 复用。
```

允许：

```text
复杂打飞机 HUD 这类小型常驻 HUD，可以每帧轻量刷新 score / hp / wave。
但即使是每帧刷新，也只刷新 active binding paths。
```

禁止：

```text
每帧扫描全 World 并生成全部 UI 数据。
每帧重建背包 / 商店 / 羁绊 / tooltip / 列表等复杂 UI read model。
把未打开面板、未声明 binding path、不可见 screen 的数据提前全量生成。
```

复杂 UI 的默认策略：

```text
数据源变化时标 dirty：
  gameplay_dirty
  inventory_dirty
  shop_dirty
  equipment_dirty
  selection_dirty
  screen_flow_dirty
  localization_dirty

produce 时只处理：
  当前 visible / active screen。
  当前 AUI Document 声明的 binding paths。
  dirty domain 命中的数据。

未 dirty 的 path 复用上一帧 cached value。
```

Report 分档：

```text
Runtime 默认 Off 或 Summary：
  只保留 producer_id / snapshot_source / value_count / cache_status / dirty_domains。

Editor / Test / Debug Trace：
  才输出 produced_paths / declared_binding_paths / missing_paths / type_mismatch / cache_hit_paths / cache_miss_paths。
```

## 6. 边界规则

### 6.1 AUI Runtime Core

AUI Runtime Core 可以：

```text
接收 ProjectUiStateSnapshot。
根据 binding_refs resolve 值。
生成 binding report / layout report / present report。
```

AUI Runtime Core 不可以：

```text
直接读取 ECS World。
调用 Project Rule。
调用 Rust Project Module 的业务函数。
保存项目专用状态。
解释 IR。
```

### 6.2 Rust Project Module / Project Framework

Rust Project Module 可以：

```text
读取 ECS / project runtime state / input-derived UI state。
计算复杂 UI read model。
输出 ProjectUiStateSnapshot。
输出 ProjectUiStateSnapshotReport。
```

Rust Project Module 不应该：

```text
直接改 AuiDocument 结构。
直接生成 AuiOverlayFrame。
直接调用 renderer。
把通用 AUI binding 规则写死成项目 API。
```

### 6.3 IR / RuleSlot

IR / RuleSlot 可以：

```text
在受限规则片段内修改被允许的项目数据。
通过项目状态间接影响 ProjectUiStateSnapshot。
在固定 Contract 内编排受限业务交易，并在热更 Apply Point 后替换规则版本。
```

IR / RuleSlot 不可以：

```text
生成 ProjectUiStateSnapshot 的复杂聚合逻辑。
实现 UI drag/drop、背包排序、羁绊统计、装备推荐。
实现商店抽样算法、背包事务原语、装备槽位一致性检查。
直接写 AUI node tree。
直接读 renderer / file / network / ECS internals。
```

### 6.4 UI 项目侧逻辑分级

为了对齐 Unity “UI 底座能力由引擎提供，项目侧 UI 逻辑可由 Lua / C# 接响应”的心智，本项目采用下面的边界。

这不是新增架构层，只是原有两层心智里的归属判断：

```text
Rust Project Framework
  -> Rust AUI Core
  -> Rust Project Module / ProjectUiStateSnapshotProducer

Project Assets
  -> AUI Document
  -> Data Asset
  -> Contract-bound UI RuleSlot / IR
```

#### Rust AUI Core

负责控件天生能力：

```text
Button 点击判定。
Pointer / keyboard / gamepad 输入分发。
hit test / hover / focus。
drag capture / drop target detection。
scroll / clipping / list virtualization。
Image / Text / layout / draw list。
```

这些能力不能进 IR。

#### AUI Document / Project Assets

负责界面怎么摆、控件怎么声明：

```text
这里有一个 button。
button 使用哪个 image asset。
button 文本 / 样式 / 布局 / binding path。
button 点击后发出哪个 action_id。
某个 image 的 source 绑定到哪个 snapshot path。
```

这些是结构化 UI 资产，不是 IR 脚本。

#### Contract-bound UI RuleSlot / IR

允许承担简单、受限、可验证的 UI 胶水规则：

```text
on ui.shop.refresh.clicked -> emit shop.refresh_requested。
enabled = player.gold >= shop.refresh_cost。
visible = round.phase == "planning"。
image = item.rarity -> rarity_frame_asset。
text = simple_format("{gold}", player.gold)。
warning = current_gold < equip_cost。
```

这类规则可以是本项目的“安全版 Lua”：它只表达固定输入输出的 UI 响应 / 展示规则，不接触 UI 机制，不读 ECS，不调用 renderer，不创建或移动 node。

#### Contract-bound Transaction RuleSlot / IR

允许承担受限、可验证、可热更的业务交易编排：

```text
on ui.shop.refresh.clicked:
  if player.gold >= shop.refresh_cost
  then economy.debit(player, shop.refresh_cost)
  then shop.reroll_slots(pool_id, locked_slots, rng_slot)

on ui.equipment.drop_on_slot:
  if can_equip(selected_unit, dragged_item, target_slot)
  then economy.debit(player, equip_cost)
  then inventory.equip_requested(selected_unit, dragged_item, target_slot)
```

这里的 `economy.debit`、`shop.reroll_slots`、`inventory.equip_requested` 不是任意函数调用，而是 Rust Project Module 通过 Contract 暴露的确定性 transaction primitive。

Transaction RuleSlot 可以热更：

```text
价格。
条件。
权重。
卡池 / 装备表引用。
是否允许某类装备进入某槽位。
某个 UI action 映射到哪个业务 command。
```

Transaction RuleSlot 不可以热更：

```text
事务原语的内部一致性。
背包容器数据结构。
随机抽样算法实现。
拖拽状态机。
ProjectUiStateSnapshot 复杂聚合算法。
```

#### Rust Project Module

复杂 UI 工作流仍然进 Rust：

```text
装备拖拽事务。
背包排序 / 过滤 / 搜索。
商店抽样和槽位更新原语。
选中单位 / hover item / tooltip 生命周期。
面板状态机和页面流转。
网络 / 存档 / 异步加载。
ProjectUiStateSnapshot 复杂聚合。
```

判断规则：

```text
如果它像“条件 / 映射 / 简单响应”，优先考虑 UI RuleSlot / IR。
如果它像“固定 Contract 内的业务交易编排”，优先考虑 Transaction RuleSlot / IR。
如果它像“流程 / 状态机 / 复杂集合算法 / 生命周期”，进入 Rust Project Module。
如果它像“点击、拖拽、渲染、布局、输入传播”，进入 Rust AUI Core。
如果它像“按钮长什么样、绑定哪个 path、发哪个 action”，进入 AUI Document。
```

## 7. v1 数据结构建议

### 7.1 Producer trait

建议形态：

```rust
pub trait ProjectUiStateSnapshotProducer {
    fn producer_id(&self) -> &str;
    fn produce(
        &mut self,
        context: ProjectUiStateProducerContext<'_>,
    ) -> ProjectUiStateSnapshotOutput;
}
```

### 7.2 Producer context

第一版最小上下文：

```text
frame_index
time_context
world read API
runtime package summary
active_binding_paths
dirty_domains
previous_snapshot_cache
optional input / UI interaction summary
```

注意：

```text
context 可以读 World。
AUI Runtime Resolver 不能读 World。
```

### 7.3 Output / Report

建议输出：

```text
ProjectUiStateSnapshotOutput
  snapshot: ProjectUiStateSnapshot
  report: ProjectUiStateSnapshotReport
```

Report 最小字段：

```text
schema_version
producer_id
frame_index
snapshot_source
value_count
active_binding_paths[]
produced_paths[]
declared_binding_paths[]
missing_paths[]
type_mismatch[]
dirty_domains[]
cache_status
cache_hit_paths[]
cache_miss_paths[]
diagnostics[]
```

v1 报告判断规则：

```text
status=passed:
  producer 成功输出 snapshot；
  AUI Document 声明的 binding path 全部由 snapshot 覆盖，或有明确 fallback 诊断；
  active binding path 均来自真实 producer 或 cache；
  没有 type mismatch。

status=partial:
  snapshot 已产生，但存在 missing path / fallback / cache_miss / glyph_present=false 等非阻塞缺口。

status=failed:
  producer 无法运行；
  ProjectProducer 被要求但实际仍使用 PackageSmokeSnapshot；
  binding path 类型错误导致 AUI present failed。
  未经允许的全量 UI snapshot 进入 runtime 热路径。
```

### 7.4 Snapshot source

当前 `AuiSnapshotSource` 应扩展：

```text
PackageSmokeSnapshot
ProjectProducer
TestFixture
```

规则：

```text
PackageSmokeSnapshot 只能用于 C-min smoke。
ProjectProducer 才能声明真实项目 UI state 已接通。
```

## 8. 复杂项目示例

### 8.1 复杂打飞机 HUD

AUI Document binding：

```text
game.score_text -> Text.text
player.hp_ratio -> ProgressBar.value
player.weapon_name -> Text.text
game.wave_text -> Text.text
game.paused -> Panel.visible
```

Rust producer 负责：

```text
从 gameplay state / ECS / project runtime state 读取分数、生命、武器、波次。
格式化 SCORE 000120。
把 hp 当前值 / 最大值转换成 0.0 - 1.0。
输出 ProjectUiStateSnapshot。
只生成 HUD 当前声明的 active binding paths。
score / hp / wave 未变化时允许复用 cached value。
```

IR 负责：

```text
只在 RuleSlot 内修改受限 gameplay 数据，例如得分增加或受伤。
不负责格式化 HUD，不负责读取 AUI Document。
可以负责简单 UI 条件，例如 paused_panel.visible = game.paused。
```

### 8.2 自走棋装备 / 商店 UI

AUI Document binding：

```text
shop.gold_text
shop.refresh_cost_text
shop.slots[0].unit_name
shop.slots[0].price_text
inventory.items[0].icon
synergy.active_list_text
round.phase_text
```

Rust producer 负责：

```text
从经济系统、商店系统、背包系统、羁绊系统读取状态。
排序和过滤装备 / 单位列表。
计算羁绊展示文本。
格式化价格、星级、锁定状态。
输出 snapshot。
只在对应面板 visible 且 binding path active 时生成。
inventory_dirty / shop_dirty / selection_dirty 未触发时复用缓存。
```

IR 负责：

```text
可以表达受限规则，例如某个装备增加攻击力、刷新花费倍率。
可以表达简单 UI 条件，例如 gold 不足时禁用购买按钮。
可以表达受限交易编排，例如 refresh_clicked -> debit + reroll_requested。
不负责商店列表聚合、拖拽流程、复杂排序、UI 面板状态机。
```

### 8.3 按钮能力例子

一个自走棋装备按钮：

```text
Rust AUI Core:
  button 可点击。
  button 可 hover。
  button 可作为 drop target。
  image / text / layout 能被渲染。
```

```text
AUI Document:
  node_id = "equip_button"
  kind = Button
  image = binding("ui.selected_item.icon")
  enabled = binding("ui.equipment.can_equip")
  actions.click = "ui.equipment.equip_requested"
```

```text
UI RuleSlot / IR:
  ui.equipment.can_equip =
    selected_item != null
    and selected_unit != null
    and player.gold >= equip_cost

  ui.selected_item.frame_image =
    rarity_to_frame(selected_item.rarity)
```

```text
Transaction RuleSlot / IR:
  on ui.equipment.equip_requested:
    if ui.equipment.can_equip
    then economy.debit(player, equip_cost)
    then inventory.equip_requested(selected_unit, selected_item, target_slot)
```

```text
Rust Project Module:
  提供 economy.debit / inventory.equip_requested transaction primitive。
  校验当前单位、物品、槽位和事务版本。
  执行或拒绝 equip transaction。
  维护背包 / 单位状态。
  生成 ProjectUiStateSnapshot。
```

这个例子说明：

```text
按钮的“能力”由 Rust AUI Core 提供。
按钮的“声明”由 AUI Document 提供。
按钮的“简单条件和映射”可以由 UI RuleSlot / IR 提供。
按钮背后的“受限业务交易编排”可以由 Transaction RuleSlot / IR 热更。
按钮背后的“事务原语和复杂状态维护”由 Rust Project Module 提供。
```

## 9. AI 修改规则

AI 默认可改：

```text
AUI Document binding path。
Project Assets 中的 UI 配置数据。
轻量 BindingPath manifest。
受限 RuleSlot。
Rust Project Module 中的 producer 实现，但必须走施工文档和测试。
```

AI 不默认改：

```text
engine_runtime AUI Core。
Renderer / RHI。
Canonical Rule IR 手写产物。
RuntimePackage 最终 bundle 文件。
```

AI 修 bug 时按这个判断：

```text
UI 位置 / 文案控件 / 绑定路径错：优先改 AUI Document。
UI 绑定值缺失 / 类型错：优先查 ProjectUiStateSnapshotReport。
UI 值计算错：改 Rust ProjectUiStateSnapshotProducer。
tooltip / display transform / panel state patch 这类项目侧 UI 热逻辑错：优先查 Project UI Hot Logic Module report。
交易条件 / 价格 / 权重 / action 编排错：优先查 Transaction RuleSlot / RuleSlot report。
事务一致性 / 槽位版本 / 容器状态错：查 Rust Project Module。
玩法数据变化错：查 Rust Project Module 或受限 RuleSlot。
AUI draw/pass/glyph 错：查 AUI Runtime / renderer。
```

## 10. 测试与报告

第一版必须有：

```text
producer can produce snapshot for sample project
binding paths declared by AUI document are covered or reported missing
type mismatch produces diagnostic
package smoke snapshot cannot be reported as real project state
runtime present report includes snapshot_source=ProjectProducer
complex shooter e2e report includes ui_state_snapshot_report
producer does not build a full UI mirror when active binding paths are limited
cache_status / dirty_domains are reported at least in Summary or Trace gates
```

验收红线：

```text
PackageSmokeSnapshot 不能伪装成真实项目 UI state。
ProjectProducer 必须在 runtime_player_winit 报告中可见。
project_e2e_gate 必须记录 snapshot_source / producer_id / produced_paths / missing_paths。
复杂打飞机 HUD 至少覆盖 game.score_text / player.hp_ratio / game.paused / player.ship_icon。
P0-3 允许小 HUD 每帧轻量刷新，但禁止全项目 UI state 每帧全量生成。
复杂 UI 后续必须走 dirty / cached；没有 dirty 的 path 应复用 cached value。
```

推荐测试命令：

```powershell
cargo test -p engine_runtime aui
cargo test -p engine_runtime project_ui_state
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate aui
cargo fmt --check
```

## 11. 分阶段落地建议

Gate A：定义 producer contract

```text
新增 ProjectUiStateSnapshotProducer trait / context / output / report。
AuiSnapshotSource 增加 ProjectProducer。
不接复杂项目。
```

Gate B：接入 AUI present

```text
AuiRuntimePresenter 支持 producer snapshot input。
report 写出 snapshot_source / value_count / diagnostics。
收集当前 AUI Document 的 active binding paths 并传给 producer。
```

Gate C：复杂打飞机 C-min producer

```text
为 samples/complex_shooter_project 提供 Rust sample producer。
先覆盖 score / hp / paused / weapon / wave。
读取真实 project.sessionState / project.combatState。
只生成 HUD active binding paths；producer 保留上一帧 cache。
```

Gate D：E2E report

```text
project_e2e_gate / runtime_player_winit 报告 ProjectProducer 是否参与。
禁止把 PackageSmokeSnapshot 伪装成真实项目状态。
报告 cache_status / dirty_domains，证明不是全量 UI 镜像器。
```

Gate E：文档同步

```text
更新 49 / 54 / 阶段完成记录。
如果施工完成，归档施工文档。
```

## 12. 自审

是否符合 `195` / `196`：

```text
符合。复杂 UI 状态聚合进入 Rust Project Module，不进入 IR；
同时允许 Contract-bound UI RuleSlot 承担简单、受限、可验证的 UI 胶水规则；
固定 Contract 内的受限业务交易编排允许进入 Transaction RuleSlot 热更。
```

是否新增不必要结构层：

```text
没有。Producer 是 Rust Project Framework 能力，不是新 runtime 架构层。
BindingPath coverage report 是诊断，不是业务所有权路由器。
Dirty / cache 是 producer 的执行策略和 report 证据，不是新增 UI 架构层。
```

是否适配复杂项目：

```text
适配。打飞机 HUD、自走棋商店 / 背包 / 羁绊都可以由 Rust producer 聚合；
商店刷新、装备请求等业务交易策略可以由 Transaction RuleSlot 热更，
事务原语和复杂聚合仍由 Rust Project Module 保证。
复杂 UI 不要求每帧全量重算；默认按 active binding path、dirty domain 和 cached value 收敛成本。
```

是否 AI 友好：

```text
较好。AUI Document 与 snapshot report 让 AI 能定位是 binding path、值生产、玩法数据还是 present 问题。
```

## 13. 结论

正式采用：

```text
Game UI MVVM ReadModel / AUI ProjectUiStateSnapshot Producer v1
  = Rust ProjectUiStateSnapshotProducer
  + active binding path 驱动
  + dirty / cached UI state
  + 最小 BindingPath coverage report。
```

下一步：

```text
读取本方案。
如无审查文档，生成施工文档前先做方案自审。
如有其它 AI 审查文档，先读审查再决定是否修改本方案。
```
