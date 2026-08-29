# 200-复杂装备 UI / Transaction RuleSlot 压力测试方案

## 1. 测试目标

本文件测试一个问题：

```text
如果做一个自走棋级复杂装备 UI：
  支持背包装备展示
  支持装备拖拽到槽位
  支持槽位更新
  支持复杂 tooltip / 属性预览 / 稀有度框
  支持点击与拖拽后的业务交易

当前 195 / 199 修正后的架构是否能承载？
哪些可以热更？
哪些不能热更？
当前代码已经支持到哪里？
```

一句话结论：

```text
方案层面可以承载。
用户心智仍保持 Rust Project Framework + Project Feature Assets。
复杂装备 UI 的热更点应落在 AUI Document / Data Asset / UI RuleSlot / Transaction RuleSlot / Project UI Hot Logic Module。
Rust 负责 AUI 交互底座、事务原语、完整复杂聚合、排序过滤和系统不变量。
频繁变化的项目侧 UI 热逻辑可以用 Rust 编写并编译为受控 WASM hot module。
当前代码只支持到 AUI click / hover / binding / snapshot C-min；
drag/drop、action payload、ProjectUiStateSnapshotProducer、Transaction RuleSlot hot update、Project UI Hot Logic Module 尚需后续施工。
```

## 2. 外部引擎对标

### Unity

Unity 项目常见做法：

```text
UI 底座能力：
  Unity UI / UI Toolkit 提供 Button、Image、Pointer Event、Drag Event、Layout。

项目侧热更：
  xLua / toLua / ILRuntime / HybridCLR 等方案常用于写 UI 响应、活动逻辑、交易编排。

资源热更：
  Addressables / Remote Content 更新 UI 资源、配置和内容包。
```

对本项目的启发：

```text
拖拽、命中、渲染、布局不应该进 IR。
点击后的业务策略和交易编排需要有热更空间。
Lua 是自由脚本，本项目不能照搬成通用脚本层，而应做受限 Transaction RuleSlot。
Rust-authored WASM 更适合作为本项目项目侧 UI 热逻辑的受控路线。
```

### Unreal Engine

UE 常见做法：

```text
UMG / Slate 负责 UI 控件、输入和拖拽事件。
Blueprint / C++ 负责项目响应。
DataAsset / Blueprint / Patch 内容可以承载可更新配置和逻辑资产。
```

对本项目的启发：

```text
UI authoring surface 和 runtime UI framework 分离。
业务响应可以资产化，但大型图脚本会有维护成本。
```

### Godot

Godot 常见做法：

```text
Control 提供 UI node、布局、输入。
Control 支持 get_drag_data / can_drop_data / drop_data 一类拖放回调。
项目脚本响应拖放和更新业务状态。
```

对本项目的启发：

```text
drag/drop 是 UI framework 的事件机制。
业务策略是项目层。
本项目应把事件机制留在 Rust AUI Core，把受限业务策略放入 RuleSlot。
```

## 3. 用户心智

复杂装备 UI 对用户只暴露两个入口：

```text
Rust Project Framework
  -> AUI Core
  -> Equipment / Inventory / Economy transaction primitive
  -> ProjectUiStateSnapshotProducer
  -> sorting / filtering / tooltip lifecycle / transaction validation

Project Feature Assets
  -> AUI Document
  -> Equipment Data Asset
  -> UI RuleSlot
  -> Transaction RuleSlot
  -> Tests / Reports
```

不要求用户每天判断四层：

```text
AUI / IR / Rust / Snapshot
```

用户和 AI 默认进入 Feature：

```text
features/equipment_panel/
  feature.toml
  ui/equipment_panel.aui
  data/equipment_items.asset
  data/equipment_slots.asset
  rules/equipment_ui_rules.rule
  rules/equipment_transaction_rules.rule
  logic/equipment_primitives.rs
  logic/equipment_snapshot_producer.rs
  tests/equipment_panel_cases.json
  reports/equipment_panel_validation.json
```

Feature Folder 只是编辑期组织和 AI patch scope，不是新的运行时层。

## 4. 复杂装备 UI 构造

### 4.1 AUI Document

```text
equipment_panel
  selected_unit_header
  inventory_grid
    item_cell[0..63]
      item_icon
      rarity_frame
      equipped_badge
      count_text
  equipment_slots
    weapon_slot
    armor_slot
    trinket_slot
  compare_tooltip
    current_item_stats
    target_item_stats
    delta_preview
  action_bar
    equip_button
    unequip_button
    sell_button
```

AUI binding：

```text
ui.equipment.visible
ui.selected_unit.name
ui.selected_unit.class_icon
ui.inventory.items[n].icon
ui.inventory.items[n].rarity_frame
ui.inventory.items[n].count_text
ui.inventory.items[n].is_equipped
ui.equipment.slots.weapon.icon
ui.equipment.slots.weapon.enabled
ui.equipment.slots.weapon.highlight
ui.tooltip.visible
ui.tooltip.item_name
ui.tooltip.stat_delta_text
ui.action.equip_enabled
ui.action.sell_enabled
```

AUI actions：

```text
ui.equipment.item.clicked
ui.equipment.item.drag_begin
ui.equipment.item.drag_move
ui.equipment.item.drop_on_slot
ui.equipment.slot.clicked
ui.equipment.equip_requested
ui.equipment.unequip_requested
ui.equipment.sell_requested
```

### 4.2 Rust AUI Core

负责：

```text
hit test。
hover。
pointer capture。
drag begin / drag move / drop target detection。
scroll。
clip。
draw image / text / rect。
action payload 组装，例如 dragged_item_id、target_slot_id、source_node。
interaction trace。
```

不负责：

```text
装备能否穿戴。
装备后属性怎么变。
背包如何排序。
扣钱或售卖。
修改项目背包状态。
```

### 4.3 ProjectUiStateSnapshotProducer

负责从项目状态生成 UI read model：

```text
读取 selected_unit。
读取 inventory。
读取 equipment slots。
读取 hover / selected / drag preview 状态。
排序并分页 inventory_grid。
生成 tooltip 文本。
生成 stat_delta_text。
生成 icon / rarity_frame / enabled / visible。
输出 ProjectUiStateSnapshot + coverage report。
```

关键边界：

```text
Binding 仍只读 ProjectUiStateSnapshot。
AUI Runtime 不读 ECS。
Renderer 不读 binding path。
```

完整 producer 默认由 Rust Project Module 实现。  
但其中频繁变化、输入输出固定、可批量执行的 UI 热逻辑片段，可以通过 Project UI Hot Logic Module 承载，例如：

```text
tooltip 文本和属性差值格式化。
item display transform。
panel state patch。
AuiAction 到 ProjectCommand 的受限映射。
ProjectUiStateSnapshotPatch 生成。
```

Project UI Hot Logic Module 的推荐执行形态：

```text
Rust source
  -> WASM hot module
  -> read ProjectUiStateInput
  -> output ProjectUiStateSnapshotPatch / ProjectCommand / Diagnostic
```

它不能直接读 ECS、不能改 AUI tree、不能实现 drag/drop framework、不能访问 renderer / file / network / platform API。

### 4.4 UI RuleSlot

可热更的 UI 展示规则：

```text
ui.equipment.visible = current_screen == "planning"。
ui.action.equip_enabled = selected_item != null and can_equip_result == true。
ui.tooltip.visible = hovered_item != null。
rarity_frame = rarity_to_frame(item.rarity)。
warning_text = if gold < equip_cost then "Gold not enough" else ""。
```

### 4.5 Transaction RuleSlot

可热更的业务交易编排：

```text
on ui.equipment.drop_on_slot(item_id, unit_id, slot_id):
  if can_equip(unit_id, item_id, slot_id)
  if player.gold >= equip_cost(item_id, unit_id)
  then economy.debit(player_id, equip_cost)
  then inventory.equip_requested(unit_id, item_id, slot_id)

on ui.equipment.sell_requested(item_id):
  if item_is_sellable(item_id)
  then inventory.remove_requested(item_id)
  then economy.credit(player_id, sell_price(item_id))
```

Transaction RuleSlot 可以热更：

```text
can_equip 条件。
equip_cost / sell_price。
不同职业 / 羁绊 / 装备类型的槽位限制。
事件 action 到业务 command 的编排。
是否需要二次确认。
是否允许战斗阶段换装。
```

Transaction RuleSlot 不直接做：

```text
背包数组移动。
装备槽位写入。
扣钱账本一致性。
版本冲突检查。
随机和事务回滚。
```

### 4.6 Rust Project Module

负责 transaction primitive：

```text
economy.debit(player_id, amount)。
economy.credit(player_id, amount)。
inventory.equip_requested(unit_id, item_id, slot_id)。
inventory.remove_requested(item_id)。
equipment.swap_slots(unit_id, from_slot, to_slot)。
```

Rust 保证：

```text
输入 handle / generation 有效。
背包、单位、槽位版本一致。
事务要么完整成功，要么完整拒绝。
diagnostic 可解释。
确定性随机由 Contract 提供 rng slot。
不会绕过 ECS deferred command / Runtime Command 规则。
```

## 5. 热更矩阵

| 项目 | 能否热更 | 归属 | 说明 |
|---|---:|---|---|
| 装备图标 / 稀有度框资源 | 是 | Data Asset / AUI Asset | 资源热更，需 hash / dependency validation |
| 装备面板布局 | 是，受限 | AUI Document | 通过 RuntimePackage / Asset Package 更新，需 binding coverage |
| 文案 / 颜色 / 样式 | 是 | AUI Document / Data Asset | 需 UI visual regression / binding report |
| visible / enabled 条件 | 是 | UI RuleSlot | 固定输入输出，适合热更 |
| can_equip | 是 | RuleSlot | 典型可验证规则 |
| equip_cost / sell_price | 是 | RuleSlot / Data Asset | 典型数值热更 |
| drop_on_slot -> equip_requested 编排 | 是 | Transaction RuleSlot | 固定 Contract 内可热更 |
| 是否战斗阶段允许换装 | 是 | Transaction RuleSlot | 条件热更 |
| 背包排序权重 | 部分 | RuleSlot / Data Asset | sort key 可热更，排序算法不热更 |
| 背包排序算法 | 否 | Rust Project Module | 算法和性能路径 |
| tooltip 文本 / 属性差值生成 | 是，受限 | Project UI Hot Logic Module | Rust-authored WASM，批量输入输出 |
| item display transform | 是，受限 | Project UI Hot Logic Module | 适合装备格子展示、稀有度映射、局部 snapshot patch |
| panel state / screen flow patch | 是，受限 | Project UI Hot Logic Module | 不能持有跨热更长期 unmanaged state |
| AuiAction -> ProjectCommand 映射 | 是，受限 | Project UI Hot Logic Module / Transaction RuleSlot | 只输出业务意图，不直接改 ECS |
| drag/drop 命中和 pointer capture | 否 | Rust AUI Core | UI framework 机制 |
| 槽位事务写入 | 否 | Rust Project Module | 系统不变量 |
| 经济账本扣款一致性 | 否 | Rust Project Module | 事务原语 |
| 完整 ProjectUiStateSnapshot 聚合算法 | 否 | Rust Project Module | 复杂 read model，局部可变展示片段可走 Project UI Hot Logic Module |
| tooltip 生命周期 / hover delay 状态机 | 通常否 | Rust AUI Core / Project Module | 简单显示条件可热更，生命周期不热更 |
| AUI Core drag/drop/layout/hit test | 否 | Rust AUI Core | 引擎 UI 底座能力 |
| RuntimePackage loader | 否 | Rust Runtime | 引擎底座 |

## 6. 当前代码能力测试

本次是架构压力测试，不是施工完成声明。

根据当前 `engine_runtime::aui` 和 `project_logic` 代码核对：

| 能力 | 当前状态 | 依据 |
|---|---|---|
| ProjectUiStateSnapshot values | 已有 C-min | `ProjectUiStateSnapshot { frame_index, values }` |
| Binding resolve | 已有 C-min | AUI resolver 读取 snapshot path |
| AUI click action | 已有 C-min | `AuiActionEvent::Click` / `AuiActionMapper` |
| Hover command | 已有 C-min | `AuiCommandKind::Hover` |
| PointerDown / PointerUp / PointerMove | 已有 C-min | `AuiInteractionSystem::process` |
| DragBegin / DragMove / Drop | 未实现 | `AuiActionEvent` 当前只有 `Click` |
| action payload | 未实现真实 payload | `AuiCommand.payload` 当前创建为 `None` |
| ProjectUiStateSnapshotProducer trait | 未实现 | 199 是下一步方案，当前仍 smoke snapshot |
| Transaction RuleSlot | 未实现 | 195 / 199 已定边界，代码还没有正式交易规则执行入口 |
| Project UI Hot Logic Module | 未实现 | 09 已定边界；Rust-authored WASM host/interface/runtime 尚未施工 |
| IR runtime hot update | 未实现正式链路 | 当前 `IrInterpreterExecutor` 是 validation-only / crate-internal |
| 复杂装备 UI 可运行端到端 | 未实现 | 缺 drag/drop、payload、producer、transaction rule |

当前可真实支持：

```text
装备面板的静态 AUI 文档。
图标 / 文本 / 简单按钮展示。
Click action。
绑定 snapshot 中的文本 / 数值 / asset_ref。
headless AUI hit test / present C-min。
```

当前不能声称支持：

```text
拖拽装备到槽位。
drop payload 驱动业务交易。
Transaction RuleSlot 热更执行。
Project UI Hot Logic Module 热更执行。
真实 ProjectUiStateSnapshotProducer 从项目状态生成复杂 read model。
复杂装备 UI 端到端可玩。
```

## 7. 架构是否支持

### 支持结论

```text
设计支持。
当前实现不完整。
```

设计上支持的原因：

```text
AUI Document 可承载复杂面板结构。
AUI Runtime Core 可扩展 drag/drop 作为通用 UI 机制。
AuiAction 可表达业务意图。
ProjectUiStateSnapshot 可作为 UI read model。
Transaction RuleSlot 给了类似 Unity Lua 的热更业务编排空间。
Project UI Hot Logic Module 给了项目侧 UI Rust 逻辑的受控热更空间。
Rust Project Module 保护事务原语和复杂算法。
Feature Folder 降低用户和 AI 判断成本。
```

当前实现缺口：

```text
缺 AuiActionEvent::DragBegin / DragMove / Drop。
缺 drop target / pointer capture / drag state report。
缺 action payload schema。
缺 ProjectUiStateSnapshotProducer trait 和 report。
缺 Transaction RuleSlot contract / validation / runtime dispatch。
缺 Project UI Hot Logic Module 的 WASM host interface / validation / apply point。
缺 hot update package apply point。
缺复杂装备 UI headless scenario test。
```

## 8. 最小施工切片建议

如果下一步要把该压力测试变成真实功能，建议不要一次做完整装备系统，而是分 Gate：

```text
Gate A: AUI DragDrop Interaction C-min
  AuiActionEvent 增加 DragBegin / DragMove / Drop。
  AuiInteractionReport 输出 drag trace。
  action payload 包含 source_node / target_node。

Gate B: Equipment Panel AUI Static + Binding
  构造 equipment_panel.aui。
  使用 fixture ProjectUiStateSnapshot 展示 item grid / slots / tooltip。

Gate C: ProjectUiStateSnapshotProducer C-min
  Rust producer 输出 inventory / slots / selected / tooltip。
  binding coverage report 证明 AUI path 被覆盖。

Gate D: Transaction RuleSlot C-min
  只支持 equip_requested 一个交易编排。
  Rust primitive 做 can_apply / apply / reject diagnostic。

Gate E: Hot Update Rule Override C-min
  修改 can_equip / equip_cost / equip_requested 编排。
  在 Apply Point 切换 rule version。
  输出 rollback target 和 validation report。

Gate F: Project UI Hot Logic Module C-min
  Rust-authored WASM 模块只处理 tooltip / display transform / ProjectCommand mapping。
  Host 提供 ProjectUiStateInput，模块输出 ProjectUiStateSnapshotPatch / Diagnostic。
  通过 hash / signature / import whitelist / deterministic test / perf budget 后才能热更。
```

## 9. 自审

是否新增架构层：

```text
没有。
Transaction RuleSlot 是 Contract-bound RuleSlot 的细分，不是新 runtime layer。
Project UI Hot Logic Module 是热更模块类型，不是新的业务分层。
Feature Folder 仍只是编辑期组织。
```

是否违背 195 / 199：

```text
不违背。
本文件采用 195 / 199 修正后的规则：
  Rust 提供事务原语和复杂机制。
  IR 只做固定 Contract 内的受限交易编排。
  项目侧 UI 热逻辑可用 Rust-authored WASM 承载，但 AUI Core 和完整复杂聚合仍在 Rust。
```

是否像 Unity Lua：

```text
相似点：
  项目业务策略可以热更。
  UI action 后的交易编排不必全部写死在 Rust。

不同点：
不允许任意 Lua 式自由代码。
只能调用 Contract 暴露的确定性 primitive。
reads / writes / emits 必须可审查。
热更必须有 validation report / hash / signature / rollback target。
Rust-authored WASM 只承载受控项目 UI 热逻辑，不替代 Lua 的通用脚本生态。
```

最终判断：

```text
复杂装备 UI 是本架构必须支持的目标。
修正后的架构方向合理。
当前实现还有明确缺口，不应把设计支持误报为已完成。
下一步若要施工，优先做 AUI DragDrop Interaction C-min + Equipment Panel Binding fixture。
```
