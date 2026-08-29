# 195-Gameplay Rule Asset / Rust Framework / IR 红线 / AUI 逻辑边界方案

## 1. 一句话结论

本方案在 `194` 的基础上重新收敛项目逻辑和 UI 逻辑：

```text
用户心智仍然尽量保持两层：

Rust Project Framework
  -> 负责复杂系统、运行机制、ECS、AUI Runtime、性能、确定性和生命周期

Project Assets
  -> 负责可编辑内容：
     Gameplay Rule Asset
     AUI Document
     Data Asset
     Binding / Action / Contract-bound RuleSlot
```

更直接地说：

```text
IR 不负责生成 UI 系统。
IR 不负责吞掉所有项目逻辑。
IR 只负责可数据化、可验证、可审查、可热更的规则片段。

复杂算法、系统框架、事务原语和复杂 UI 交互机制，默认由 Rust Framework / Project Rust Module 承担。
固定 Contract 内的受限业务交易编排可以进入 IR RuleSlot 热更。
项目侧 UI 热逻辑如果不是 IR RuleSlot 的合适表达对象，可用 Rust 编写并编译为 Project UI Hot Logic Module / WASM 受控热更。
```

## 2. 本方案修正什么

`194` 已经把用户心智从多层链路：

```text
Schema / Blueprint / Rule Graph / DSL / IR / RuntimePackage / Rust Domain Runtime
```

收敛为：

```text
Rust Gameplay Framework + Gameplay Rule Asset
```

这个方向正确，但还缺两条硬边界：

```text
1. IR 不能继续被描述成所有项目上层逻辑的最终归宿。
2. UI 系统不能被误解为由 IR 生成或由 IR 脚本驱动。
```

因此本方案补充：

```text
IR 红线
AUI / UI 逻辑所有权
自走棋复杂项目边界表
对 193 / 194 / AUI 文档的修正规则
```

## 3. 其它引擎对标结论

### 3.1 Unity

Unity UI Toolkit 的 runtime UI 路线是：

```text
UI Document / UXML
UIDocument GameObject
C# MonoBehaviour 定义 UI 控件行为
Runtime Event System 处理输入事件
Runtime Data Binding 绑定 C# object 到 UI control
```

参考：

```text
https://docs.unity3d.com/6000.0/Documentation/Manual/UIE-get-started-with-runtime-ui.html
https://docs.unity3d.com/6000.2/Documentation/Manual/UIE-Runtime-Event-System.html
https://docs.unity3d.com/6000.0/Documentation/Manual/UIE-runtime-binding.html
```

结论：

```text
Unity 不是用一个规则 IR 生成 UI 系统。
UI 结构是 UI Document / UXML。
UI 行为和复杂逻辑仍由 C# / runtime framework 承担。
Binding 是数据连接，不是完整脚本语言。
```

### 3.2 Unreal Engine

UE 的 Runtime UI 路线是：

```text
UMG Widget Blueprint 负责游戏 HUD / 菜单等 UI authoring
Widget Tree / Widgets 负责界面结构
Slate 是底层 UI programming framework
Slate delegates 读写 Model 数据
```

参考：

```text
https://dev.epicgames.com/documentation/unreal-engine/umg-ui-designer-quick-start-guide-in-unreal-engine
https://dev.epicgames.com/documentation/unreal-engine/slate-ui-framework
https://dev.epicgames.com/documentation/unreal-engine/understanding-the-slate-ui-architecture-in-unreal-engine
```

结论：

```text
UE 也不是让一种规则 IR 吃掉 UI 系统。
UMG / Widget Blueprint 是 UI authoring surface。
Slate / C++ 是底层 UI framework。
复杂 UI 行为可以进入 Blueprint / C++，但这也带来图灵完备和复杂调试成本。
```

### 3.3 Godot

Godot 的 UI 路线是：

```text
Control node tree
Focus / mouse_filter / accept_event
_gui_input / signal / script
```

参考：

```text
https://docs.godotengine.org/en/stable/classes/class_control.html
https://docs.godotengine.org/en/stable/tutorials/inputs/inputevent.html
https://docs.godotengine.org/en/stable/tutorials/ui/gui_navigation.html
```

结论：

```text
Godot 把 UI 机制放在 Control / Viewport / Input 体系里。
脚本处理项目行为。
焦点、输入传播、事件消费是 UI framework 的责任。
```

### 3.4 Bevy

Bevy UI 的路线是：

```text
Node / Style 描述 UI layout
Interaction 表示 Pressed / Hovered / None
ECS systems 处理 UI 行为
```

参考：

```text
https://docs.rs/bevy/latest/bevy/ui/index.html
https://docs.rs/bevy/latest/bevy/ui/struct.Node.html
https://docs.rs/bevy/latest/bevy/ui/enum.Interaction.html
```

结论：

```text
Bevy 也没有把 UI 行为交给独立规则 IR。
UI 是 runtime framework + ECS data + systems。
项目逻辑通过 system 查询 Interaction / UI state 后执行。
```

### 3.5 对本项目的启发

成熟引擎共同点：

```text
UI 结构是资产 / 节点树 / 文档。
UI 运行机制是引擎 framework。
复杂 UI 行为是代码 / 系统 / 脚本。
Binding 是连接 View 和 Model，不是任意脚本。
```

因此本项目不能走：

```text
IR 生成 UI tree
IR 实现 hit test / focus / drag / scroll / IME
IR 脚本直接改 UI tree
IR 直接查询 ECS 生成 UI
```

本项目应走：

```text
AUI Runtime Framework in Rust
AUI Document as UI structure asset
ProjectUiStateSnapshot as UI read model
AuiAction as business intent
Gameplay Rule Asset / IR only inside declared RuleSlot
```

## 4. 其它 AI 审查采纳结论

采纳 `19 / 21 / 22 / 23` 的共同结论：

```text
全 IR 是错误路线。
IR 膨胀成图灵完备语言后，会失去可验证、可热更、可治理优势。
IR 的优势不在表达更复杂逻辑，而在把可配置规则变成受限数据。
即使 Rust 也是 AI 写的，IR 仍有价值，因为能走受限数据的规则不应该走自由代码。
```

正式裁定：

```text
IR 不是 C# / Lua / Blueprint 的替代品。
IR 是项目规则中的安全子集。
Rust 是复杂逻辑和系统机制的承载层。
```

## 5. 重新定义用户心智

`194` 的两层心智继续保留，但措辞从只强调 Gameplay 扩展为 Project：

```text
Rust Project Framework
Project Assets
```

其中：

```text
Rust Project Framework:
  Gameplay Framework
  AUI Runtime Framework
  Project Rust Module
  System Contract runtime enforcement
  ECS / Command / Event / Projection / RuntimePackage load

Project Assets:
  Gameplay Rule Asset
  AUI Document
  Data Asset
  Scene / Prefab
  Binding / Action / Contract-bound RuleSlot
```

没有编程基础和只有少量编程基础的用户默认不需要理解：

```text
Canonical Rule IR
Lowered Execution IR
RuntimePackage manifest internals
Rust AOT generated source
AUI draw list internals
UiProjection internals
ECS storage internals
```

但内部必须保留：

```text
System Contract
Schema
Rule Graph / DSL view
Canonical Rule IR
RuntimePackage
Source Map
Validation Report
Impact Report
Trace
```

## 6. IR 的正式职责

IR 只负责：

```text
可数据化规则
可验证规则
可审查规则
可热更规则
可 source map 的规则
Contract-bound RuleSlot 内的规则片段
```

典型适用：

```text
伤害公式
吸血公式
羁绊加成
商店刷新权重
掉落权重
购买条件
装备穿戴条件
回合奖励
受限业务交易编排
交易前置校验
确定性命令发射
简单 UI 可见性 / enable 条件
简单 UI 数值展示变换
```

IR 不负责：

```text
完整战斗流程
完整经济系统框架
行为树
技能状态机
A* 寻路
复杂排序 / 过滤 / 搜索算法
复杂 UI 交互流程
UI tree runtime mutation
事务一致性原语
跨系统生命周期管理
Renderer / file / network / platform API
```

### 6.1 Transaction RuleSlot 补充规则

前面“复杂流程默认 Rust”不能被误读为：

```text
所有业务交易都必须写死在 Rust。
所有点击后的业务响应都不能热更。
```

正式修正为：

```text
Rust Project Framework 提供稳定系统能力、事务原语和不变量保护。
Contract-bound Transaction RuleSlot 可以表达受限、可验证、可热更的业务交易编排。
```

Transaction RuleSlot 允许：

```text
读取 Contract 声明的项目状态快照。
执行固定输入输出的条件判断。
调用 Contract 暴露的确定性 command / transaction primitive。
发出受限业务命令，例如 shop.reroll_requested、economy.debit_requested、inventory.equip_requested。
选择数据资产中的权重、价格、卡池、奖励表。
在专门热更 Apply Point 后替换规则版本。
```

Transaction RuleSlot 不允许：

```text
直接访问 ECS storage。
直接修改 AUI tree。
直接操作背包内部容器、商店内部槽位数组或经济系统账本。
自己实现事务回滚、锁定、并发版本检查。
自己实现排序 / 搜索 / 随机卡池抽样算法。
直接调用 Renderer / File / Network / Platform API。
跨系统任意调用未声明函数。
```

示例：

```text
shop_refresh_transaction.rule:
  when action == "ui.shop.refresh_requested"
  if player.gold >= shop.refresh_cost
  then economy.debit(player, shop.refresh_cost)
  then shop.reroll_slots(pool_id, locked_slots, deterministic_rng_slot)
```

这里：

```text
economy.debit / shop.reroll_slots 是 Rust 提供的事务原语。
价格、卡池、刷新权重、是否允许刷新可以热更。
扣钱一致性、槽位版本检查、随机确定性由 Rust 原语保证。
```

## 7. IR 红线

后续 IR v1 / v2 都必须遵守这些红线，除非未来正式推翻本方案：

```text
禁止递归。
禁止 while / unbounded loop。
禁止任意函数定义和任意函数调用。
禁止任意数组 / map 编程。
禁止直接访问 ECS storage。
禁止直接访问 Renderer / File / Network / Platform API。
禁止持有裸 Entity index / ECS pointer。
禁止隐式跨规则状态共享。
禁止跨系统随意调用。
禁止把 UI tree mutation 当脚本能力暴露。
禁止实现复杂状态机 / 行为树 / 寻路 / 完整 app flow。
```

IR 可以补的表达力只限于安全规则子集：

```text
ValueExpr 写入字段。
比较运算：less / lessEqual / greater / greaterEqual / equal / notEqual。
逻辑运算：and / or / not。
基础数学：div / min / max / clamp / floor / ceil。
受限 aggregate：count / any / all / sum，且必须由 System Contract 声明输入集合。
受限 random：只能由 Contract 提供 deterministic RNG slot，不能直接读平台随机。
```

这些能力的边界：

```text
不能因此引入任意变量系统。
不能因此引入通用循环。
不能因此引入用户自定义函数。
不能因此允许 IR 自己构造复杂集合算法。
```

## 8. UI 系统到底由什么写

正式答案：

```text
UI 系统由 Rust AUI Runtime Framework 写。
UI 界面结构由 AUI Document 写。
UI 显示数据由 ProjectUiStateSnapshot 提供。
UI 交互输出 AuiAction / AuiCommand。
UI 业务规则中少量可验证片段可以进入 Gameplay Rule Asset / IR RuleSlot。
复杂 UI 交互机制由 Rust AUI Runtime Framework 写。
复杂 UI read model 聚合由 Rust Project Framework / Project Rust Module 写。
点击后的受限业务交易编排可以进入 Contract-bound Transaction RuleSlot。
```

不是：

```text
IR 生成 UI 系统。
IR 直接生成 UI tree。
IR 实现控件生命周期。
IR 直接处理 drag / focus / scroll / IME。
IR binding 直接读 ECS 或调用 Project Rule。
```

## 9. AUI 逻辑所有权表

| 能力 | 归属 | 是否可进 IR | 说明 |
|---|---|---:|---|
| UI tree / node / style / layout 声明 | AUI Document | 否 | 这是 UI 结构资产，不是规则 IR |
| layout 计算 | Rust AUI Runtime | 否 | anchor、offset、grid、scroll content、safe area 等必须 headless 可测 |
| render extract / draw list / UiProjection | Rust AUI Runtime | 否 | Renderer 只消费 AuiOverlayFrame |
| hit test | Rust AUI Runtime | 否 | 不能让 IR 算指针命中 |
| focus / keyboard navigation / gamepad navigation | Rust AUI Runtime | 否 | 属于 UI framework 机制 |
| scroll / list virtualization | Rust AUI Runtime | 否 | 性能和生命周期复杂 |
| drag / drop / pointer capture | Rust AUI Runtime | 否 | 装备拖拽、背包拖拽依赖稳定事件状态机 |
| input field / IME / text editing | Rust AUI Runtime | 否 | 平台和文本系统复杂 |
| modal stack / screen transition | Rust AUI Runtime 或 Project Rust Module | 否 | 是 app flow，不是公式规则 |
| tooltip lifecycle / hover delay | Rust AUI Runtime 或 Project Rust Module | 通常否 | 简单显示条件可进 IR，生命周期不进 |
| ProjectUiStateSnapshot 生成 | Project Rust Module / Gameplay Framework | 否 | UI 只读 snapshot，不直接读 ECS |
| AUI Binding path resolve | Rust AUI Runtime | 否 | Binding 只解析数据，不调用 Project Rule |
| visible / enabled 简单条件 | Gameplay Rule Asset / IR RuleSlot | 是 | 必须 Contract 声明输入和输出 |
| cost / stat delta / can_equip | Gameplay Rule Asset / IR RuleSlot | 是 | 输入输出固定，可验证可热更 |
| 受限业务交易编排 | Transaction RuleSlot / Rust primitive | 是 | IR 编排，Rust 保证事务原语和不变量 |
| shop refresh / equip request 策略 | Transaction RuleSlot + Project Rust Module | 部分 | 价格、条件、权重可热更；抽样、扣款、槽位更新原语在 Rust |
| item sort / complex filter / search | Project Rust Module | 通常否 | 可把 sort key 权重做 IR，但排序流程不进 IR |
| button click 后的业务意图 | AuiAction -> Project Logic | 部分 | AUI 只发 action，业务规则按 Contract 处理 |

## 10. 复杂装备 UI 示例

一个复杂自走棋装备界面应这样拆：

```text
AUI Document:
  inventory_panel
  equipment_slots
  item_grid
  compare_tooltip
  equip_button
  bindings:
    ui.inventory.items
    ui.selected_unit.equipment
    ui.hovered_item.tooltip
  actions:
    ui.item.drag_begin
    ui.item.drop_on_slot
    ui.item.inspect
    ui.equip.confirm
```

```text
Rust AUI Runtime Framework:
  hit test
  hover state
  drag capture
  drop target detection
  scroll view
  list virtualization
  tooltip positioning
  focus navigation
  consumed input report
```

```text
Project Rust Module:
  build ProjectUiStateSnapshot
  maintain selected unit / hovered item / inventory view model
  provide equip / unequip / debit / inventory transaction primitive
  apply transaction primitive or reject with diagnostic
  compute complex item sorting / filtering
```

```text
IR RuleSlot:
  can_equip(unit_tags, item_tags, slot_kind) -> bool
  equip_cost(item_level, unit_level) -> number
  stat_delta_preview(unit_stats, item_stats) -> object
  show_warning(current_gold, equip_cost) -> bool
  on ui.equipment.equip_requested:
    if can_equip and current_gold >= equip_cost
    then economy.debit(...)
    then inventory.equip_requested(...)
```

这个拆法的关键是：

```text
IR 负责规则判断。
IR 可以负责受限交易编排。
Rust 负责交互机制、复杂算法和事务原语。
AUI Document 负责界面结构。
```

## 11. 自走棋复杂项目边界表

| 自走棋能力 | Rust Framework / Project Rust Module | Gameplay Rule Asset / IR | AUI Document / AUI Runtime |
|---|---|---|---|
| 回合状态机 | 是 | 否 | 否 |
| 战斗循环 | 是 | 否 | 否 |
| 索敌 / 行为策略 | 是 | 简单权重可进 | 否 |
| A* 寻路 | 是 | 否 | 否 |
| 攻击冷却调度 | 是 | 冷却数值公式可进 | 否 |
| 伤害应用流程 | 是 | 伤害公式可进 | 否 |
| 吸血 / 暴击 / 护盾公式 | 流程在 Rust | 公式进 IR | 否 |
| 羁绊激活框架 | 是 | 羁绊阈值 / 加成进 IR | 否 |
| 商店刷新流程 | 抽样 / 槽位事务 / 随机确定性 | 价格 / 权重 / 条件 / 受限交易编排可进 | 商店 UI 在 AUI |
| 购买 / 出售流程 | 库存 / 经济事务原语 | 条件 / 价格 / 受限交易编排可进 | 按钮和列表在 AUI |
| 拖棋子上场 | board placement primitive / version check | can_place / place_requested 编排可进 | drag/drop 由 AUI Runtime |
| 装备拖拽 | equip primitive / slot transaction / version check | can_equip / equip_requested 编排可进 | drag/drop / tooltip 由 AUI Runtime |
| 战斗 HUD | 状态快照生成 | 简单显示规则可进 | AUI Document + Binding |
| 复杂列表排序筛选 | 是 | sort weight 可进 | list view / virtualize 由 AUI Runtime |
| 结算界面 | 状态快照生成 | 奖励公式可进 | AUI Document + Binding |

## 12. 方案选项

### 方案 A：全 IR，包括 gameplay 和 UI

做法：

```text
Gameplay 规则、战斗流程、UI tree、UI 交互都用 IR 表达。
```

结论：

```text
拒绝。
```

原因：

```text
IR 会被迫补变量、循环、数组、状态机、UI mutation、事件传播。
最终成为 JSON 形式的劣化脚本语言。
可验证、可热更、可治理优势会消失。
复杂 UI 用 IR 写会比 Rust 更难读、更难调试。
```

### 方案 B：全 Rust，包括所有规则和 UI

做法：

```text
所有 gameplay 规则、UI 业务、公式、条件都由 Rust 编写。
```

结论：

```text
不作为主线。
```

优点：

```text
表达力最高。
性能最好。
心智最接近传统代码项目。
```

问题：

```text
规则和业务交易编排变成自由代码，失去 IR 的热更、结构化 diff、AI patch validation。
数值和平衡改动也要走代码编译和代码 review。
AI 修改风险更接近 Unity C# / UE C++。
```

适用：

```text
复杂系统流程。
复杂算法。
复杂 UI 工作流。
未稳定成 Contract 的探索性业务交易。
暂时没有稳定 Contract 的早期探索代码。
```

### 方案 C：Rust Project Framework + Project Assets + Contract-bound IR RuleSlot

做法：

```text
Rust 写 Framework 和复杂逻辑。
AUI Document 写 UI 结构。
Gameplay Rule Asset 写可验证规则片段。
System Contract / AUI Contract 声明边界。
IR 只作为 RuleSlot 的规范语义和执行格式。
```

结论：

```text
推荐。
```

理由：

```text
AI 适配性最好：
  AUI Document / Rule Asset / Contract / Report 都是结构化对象。

复杂项目可维护：
  复杂算法、事务原语和系统不变量在 Rust。
  频繁变化规则和受限交易编排在 IR。
  UI 结构在 AUI Document。

效率平衡：
  Rust 保证系统性能和复杂行为。
  IR 保证规则和受限交易编排可热更、可审查、可自动验证。
  AUI 保证 UI 不退化成截图或脚本堆。
```

## 13. 不新增治理层：用 Feature Asset / Feature Folder 降低判断成本

### 13.1 先否定一个错误补救

前面的问题是：

```text
Rust / AUI Document / IR / Project Logic 分工正确，
但用户和 AI 后期可能不知道某个逻辑该写到哪里。
```

一个看似自然的补救是新增：

```text
Logic Ownership Router
Architecture Guard
```

本方案明确不推荐把它做成新的运行时层或独立架构层。

原因：

```text
复杂结构外面再套一层治理结构，会继续增加长期维护成本。
治理层自己也会产生 bug。
治理层出问题后又可能诱导继续新增更高层治理。
这会让架构进入“为了解决复杂而制造更多复杂”的循环。
```

正式规则：

```text
不要新增“判断逻辑该去哪”的运行时系统。
要减少“需要判断”的机会。
```

### 13.2 推荐解法：统一功能入口

复杂项目不应让用户或 AI 在全项目范围内直接面对：

```text
AUI Document
IR Rule
Project Rust Logic
Binding
Action
Tests
```

而应提供一个更自然的功能入口：

```text
Feature Asset / Feature Folder
```

例子：

```text
features/equipment_panel/
  feature.toml
  ui/equipment_panel.aui
  rules/equipment_rules.rule
  logic/equipment_actions.rs
  tests/equipment_panel_cases.json
```

用户和 AI 的默认心智是：

```text
我在修改 equipment_panel 这个功能。
```

不是：

```text
我先判断该去 Rust / IR / AUI / Binding 的哪一层。
```

### 13.3 这不是新增运行时层

Feature Asset / Feature Folder 只是：

```text
编辑期组织方式
项目资产目录约定
AI patch scope
测试和报告聚合入口
```

它不是：

```text
新的 runtime domain
新的 logic execution layer
新的 bridge
新的 router
新的 rule VM
新的 ECS system
```

运行时仍然保持原链路：

```text
AUI Document -> RuntimePackage -> AUI Runtime
AuiAction -> Project Rust Logic
IR RuleSlot -> Rule Runtime / Rust AOT
ProjectUiStateSnapshot -> Binding -> Present
```

Feature 只负责把相关文件组织在一起，降低 AI 和人的搜索成本。

### 13.4 Feature 内部固定分区

一个复杂 UI / gameplay feature 可以包含固定分区：

```text
feature:
  id
  owner
  description
  dependencies

ui:
  documents
  bindings
  actions

rules:
  rule_slots
  rule_assets

logic:
  rust_modules
  action_handlers
  view_model_builders

tests:
  scenarios
  golden_reports
```

对自走棋装备面板：

```text
EquipmentPanel.feature
  ui:
    equipment_panel.aui
    bindings:
      ui.inventory.items
      ui.selected_unit.equipment
      ui.hovered_item.tooltip
    actions:
      ui.equipment.equip_requested
      ui.equipment.unequip_requested

  rules:
    can_equip
    equip_cost
    stat_delta_preview

  logic:
    equipment_actions.rs
    equipment_view_model.rs

  tests:
    drag_item_to_slot
    cannot_equip_wrong_class
    tooltip_updates_on_hover
```

### 13.5 AI 修改规则

AI 修复或新增功能时，默认流程改成：

```text
1. 先定位 Feature。
2. 只在 Feature scope 内找 ui / rules / logic / tests。
3. 根据已有文件位置和测试失败点修改对应分区。
4. 不跨 Feature 扩散，除非 report 明确说明依赖影响。
```

例如：

```text
bug: 装备拖拽到槽位后没反应
scope: features/equipment_panel
```

AI 先检查：

```text
ui/equipment_panel.aui 是否声明 drop action。
logic/equipment_actions.rs 是否处理 equip_requested。
rules/equipment_rules.rule 是否拒绝 can_equip。
tests/equipment_panel_cases.json 哪个步骤失败。
```

这个方式不需要一个全局路由器判断“应该去哪层”，因为 Feature 的目录结构已经把候选范围压小了。

### 13.6 Bug 维护规则

复杂 UI bug 的定位优先按 Feature 内证据走：

```text
命中失败：
  看 AUI Runtime report / ui document rect / interaction case。

action 没发出：
  看 AUI Document action declaration / AUI Interaction trace。

action 发出但业务没执行：
  看 Feature logic action handler。

业务被拒绝：
  看 Feature rules / RuleSlot report。

状态变了但 UI 没刷新：
  看 view_model builder / ProjectUiStateSnapshot / Binding report。
```

注意：

```text
这是一套调试约定和文件组织方式。
不是新增一个长期运行的架构层。
```

### 13.7 与 Unity / UE 的类比

它类似于：

```text
Unity:
  一个功能目录里放 Prefab / Script / ScriptableObject / Tests。

Unreal:
  一个 Feature / Plugin / Module 里放 Widget / DataAsset / C++ / Blueprint / Tests。
```

区别是本项目更 AI-first：

```text
Feature Folder 必须能给 AI 提供稳定 scope。
Feature 内必须有结构化 tests / reports。
Feature 不允许绕过 RuntimePackage 和 AUI / IR 边界。
```

### 13.8 正式裁定

后续不把“复杂分层难判断”解决为新增治理层。

正式采用：

```text
Feature Asset / Feature Folder as authoring scope
```

目标：

```text
通过更好的功能组织减少判断，
而不是通过新增架构层管理判断。
```

## 14. 对 193 的修正规则

`193-Rule Authoring Productization v1` 的方向需要按本方案修正：

```text
Rule Authoring Productization 不应该产品化裸 Canonical IR 编辑器。
应该产品化 Gameplay Rule Asset / RuleSlot Authoring。
```

新增要求：

```text
Rule Authoring UI 必须显示规则属于哪个 System Contract / RuleSlot。
Rule Authoring UI 必须提示该规则是否适合 IR。
如果用户试图写复杂流程、循环、数组算法、UI 工作流，应建议转 Project Rust Module。
```

`ProjectRuleAsset / Canonical Rule IR` 保留为：

```text
内部真相
debug / import-export / validation 对象
RuntimePackage 构建输入
```

但不作为普通用户的唯一心智入口。

## 15. 对 AUI 文档的修正规则

`100 / 103 / 185 / 190` 的 AUI 方向继续成立，并补充本方案边界：

```text
AUI Document 是 UI 结构真相。
AUI Runtime Framework 是 UI 运行机制真相。
ProjectUiStateSnapshot 是 UI 读取项目数据的唯一正式输入。
AuiAction 是 UI 传回项目侧的业务意图。
IR 只进入 Project Rule / UI RuleSlot，不进入 AUI Runtime Core。
Project UI Hot Logic Module 只进入项目侧 UI 热逻辑，不进入 AUI Runtime Core。
```

必须继续遵守：

```text
Binding 只读 ProjectUiStateSnapshot，不读 ECS，不调用 Project Rule。
Renderer 只读 AuiOverlayFrame，不读 AuiDocument / binding path / ProjectUiStateSnapshot。
AUI action 进入 Project Logic / Project Rule，不直接改 ECS。
RuntimePackage 是运行输入真相，Runtime 不扫描源目录。
```

新增禁止：

```text
禁止把复杂 UI binding expression 做成任意脚本。
禁止让 AUI node 保存项目专用语义。
禁止让 IR 直接创建 / 删除 / 移动 AUI node。
禁止让 UI drag/drop 状态机由 IR 实现。
禁止把 Project UI Hot Logic Module 扩成 AUI Core 热更或任意 Rust native dylib 热更。
```

## 16. 后续施工前置判断规则

以后每次新增 gameplay 或 UI 能力，先按以下问题分类：

```text
1. 它是不是引擎通用机制？
   是 -> Rust Engine / Rust Framework。

2. 它是不是项目复杂流程或算法？
   是 -> Project Rust Module / Rust Gameplay Framework。

3. 它是不是固定 Contract 内的受限业务交易编排？
   是 -> Transaction RuleSlot / IR；事务原语和系统不变量仍在 Rust。

4. 它是不是 UI 结构、样式、节点和 binding？
   是 -> AUI Document。

5. 它是不是输入输出固定、可验证、可热更的规则片段？
   是 -> Gameplay Rule Asset / IR RuleSlot。

6. 它是不是需要 while、递归、任意函数、数组算法、状态机？
   是 -> 不进 IR。
```

同时优先按 Feature scope 工作：

```text
1. 先找该需求所属 Feature Folder。
2. 如果已有 Feature，优先在 Feature 内修改 ui / rules / logic / tests。
3. 如果没有 Feature，先判断是否需要创建 Feature Folder，而不是在全项目散落新增文件。
4. Feature Folder 只是 authoring scope，不允许新增 runtime layer。
```

## 17. 自审

### AI 适配性

通过。

理由：

```text
AI 修改的主要对象仍是结构化资产：
  AUI Document
  Gameplay Rule Asset
  System Contract
  RuleSlot
  ProjectUiStateSnapshot schema
  Feature Folder

复杂 Rust 代码仍可由 AI 参与，但必须走编译、测试、review。
```

### 复杂项目可维护

通过。

理由：

```text
自走棋级复杂度不会被强塞进 IR。
复杂 UI 不会被强塞进 binding 脚本。
Rust / AUI / IR 的职责清楚。
Feature scope 降低长期搜索和误改成本。
```

### 效率

通过。

理由：

```text
性能敏感和状态复杂的部分走 Rust。
频繁变化的规则走 IR。
UI 结构走 AUI Document，避免每个界面都手写 Rust。
Feature Folder 只改变组织方式，不增加运行时成本。
```

### 主要风险

风险一：

```text
用户仍可能把 IR 理解成万能脚本。
```

处理：

```text
Rule Authoring UI 必须暴露 RuleSlot 适用范围和“不适合 IR”的诊断。
```

风险二：

```text
UI binding 被慢慢扩展成脚本语言。
```

处理：

```text
Binding 保持 path / fallback / simple transform。
复杂 view model 在 Project Rust Module 中生成。
```

风险三：

```text
Rust Project Module 过多，用户心智又变复杂。
```

处理：

```text
默认用户只看到 Feature、Project Assets、可视化规则、运行结果和可理解诊断。
Project Rust Module 作为 AI 受控生成或高级用户使用的系统框架能力，不作为每个小规则的入口，也不要求用户离开引擎使用外部 IDE 才能完成主流程。
```

风险四：

```text
Feature Folder 被误解成新的架构层。
```

处理：

```text
Feature Folder 只作为 authoring scope / asset organization / AI patch scope。
不得新增 runtime execution layer、bridge、router 或 VM。
```

## 18. 最终规则

后续讨论和施工以本方案为准：

```text
1. 用户心智是 Rust Project Framework + Project Assets。
2. Gameplay Rule Asset 是 Project Assets 的一类，不等于全部项目逻辑。
3. AUI Document 是 Project Assets 的一类，不是 IR。
4. IR 只存在于 Contract-bound RuleSlot 中。
5. 复杂算法、系统框架、事务原语和复杂 UI 交互机制默认用 Rust。
6. 固定 Contract 内的受限业务交易编排可以进入 IR RuleSlot 热更。
7. AUI Runtime Core 必须用 Rust 实现，不由 IR 生成。
8. UI binding 只读 ProjectUiStateSnapshot，不读 ECS，不调用 Project Rule。
9. AUI action 只表达业务意图，进入 Project Logic / Project Rule。
10. IR 不允许递归、while、任意函数、任意数组算法、直接 ECS / Renderer / File / Network。
11. 如果某个需求为了进入 IR 必须把 IR 扩成编程语言，就应改为 Rust Module + RuleSlot。
12. 不新增 Logic Ownership Router / Architecture Guard 作为运行时或架构层。
13. 复杂功能默认通过 Feature Asset / Feature Folder 聚合 ui / rules / logic / tests。
14. Feature Folder 只用于编辑期组织、AI patch scope 和测试报告聚合，不改变运行时链路。
```

## 19. 本方案与 194 的关系

`194` 仍然作为二层心智模型的上一版基础文档保留。

本方案是 `194` 的补充和修正版：

```text
194:
  解决 Schema / Blueprint / Rule Graph / DSL / IR / RuntimePackage 层数太多的问题。

195:
  进一步解决 IR 是否会膨胀、UI 是否由 IR 生成、复杂 UI 怎么分工、
  以及不新增治理层时如何降低 AI / 用户判断成本的问题。
```

后续涉及：

```text
Rule Authoring Productization
AUI complex interaction
自走棋复杂项目
复杂打飞机项目
```

都应优先引用本方案的边界。
