# 196-IR + Rust vs Unity Lua + C# vs UE Blueprint + C++ 方案审查

## 1. 审查问题

当前 `195` 确认的方向是：

```text
IR 写受限规则片段。
Rust 写复杂逻辑、复杂 UI 交互、系统流程和算法。
```

这个方向表面上很像：

```text
Unity:
  Lua 写简单 / 热更 / 表现逻辑
  C# 写复杂逻辑和系统

Unreal Engine:
  Blueprint 写简单 / 设计师逻辑
  C++ 写复杂逻辑和底层系统
```

本审查要回答：

```text
我们的 IR + Rust 是否只是 Unity Lua + C# / UE Blueprint + C++ 的换皮？
如果不是，结构性差异在哪里？
如果是相似路线，我们的优势成立条件是什么？
```

## 2. 一句话结论

三者表面都是“双层逻辑”：

```text
上层写可变业务逻辑。
下层写复杂系统和高性能逻辑。
```

但关键差异是：

```text
Unity Lua 是完整脚本语言。
UE Blueprint 是完整 gameplay scripting system。
本项目 IR 必须是受限规则数据，不是脚本语言。
```

因此：

```text
如果 IR 保持受限，IR + Rust 有 AI-first 可验证优势。
如果 IR 膨胀成脚本语言，它会退化成“差一点的 Lua / 差一点的 Blueprint”。
```

## 3. 对标说明

### 3.1 Unity Lua + C#

Unity 官方主线是 C#。Lua 通常来自第三方热更方案，例如 xLua / XLua 类方案，用于：

```text
热更业务逻辑
活动逻辑
表现层 glue
UI 控制
简单玩法规则
```

C# 常用于：

```text
核心系统
Unity API 访问
复杂玩法系统
性能敏感逻辑
编辑器工具
```

这个路线成熟、商业项目很多，但本质是：

```text
Lua 是完整编程语言。
Lua 层可以不断膨胀，最后变成第二套代码库。
```

### 3.2 UE Blueprint + C++

UE 官方 Blueprint 是可视化脚本系统，常用于：

```text
Actor / Component 逻辑
Widget 交互
关卡脚本
技能 / 任务 / UI glue
设计师可编辑行为
```

C++ 常用于：

```text
底层系统
性能敏感逻辑
复杂框架
可复用 Gameplay Module
引擎扩展
```

这个路线成熟且官方集成强，但本质是：

```text
Blueprint 是图灵完备或接近完整脚本系统。
Blueprint 可以从简单节点变成大型可视化程序。
```

### 3.3 本项目 IR + Rust

本项目路线：

```text
IR:
  只写 Contract-bound RuleSlot 内的受限规则片段。

Rust:
  写 Runtime、AUI Runtime、Project Rust Module、复杂流程、复杂算法、复杂 UI 工作流。
```

IR 不应该写：

```text
UI drag/drop
hit test
focus
scroll
IME
行为树
状态机
A* 寻路
任意循环
任意函数
任意数组 / map 编程
直接 ECS / Renderer / File / Network API
```

## 4. 总表对比

| 维度 | Unity Lua + C# | UE Blueprint + C++ | 本项目 IR + Rust |
|---|---|---|---|
| 上层定位 | Lua 热更 / 表现 / 业务脚本 | 可视化 gameplay scripting | 受限 RuleSlot 数据 |
| 下层定位 | C# 核心系统 / 复杂逻辑 | C++ 底层框架 / 性能逻辑 | Rust 复杂流程 / 系统 / AUI / 算法 |
| 上层是否完整语言 | 是 | 基本是 | 不应该是 |
| 上层表达力 | 高 | 高 | 故意低 |
| 上层热更 | 强，但治理难 | 取决于资产和运行体系，治理复杂 | 窄，但更可验证 |
| AI 生成稳定性 | 中：文本代码好生成，但动态语义难验证 | 低：节点图 / 连线 / 引脚复杂 | 高：结构化数据 + schema |
| AI diff / review | 中：代码 diff 可读但语义自由 | 低：图 diff 难审查 | 高：结构化 diff |
| 静态验证 | 弱 | 弱到中，依赖 Blueprint 编译 | 强，前提是 IR 受限 |
| 影响分析 | 难，脚本可任意调用 | 难，节点可任意调用 | 较易，reads / writes / emits 可显式 |
| 复杂逻辑承载 | Lua 可能吞复杂逻辑，也可推给 C# | Blueprint 可能吞复杂逻辑，也可推给 C++ | 必须推给 Rust |
| 长期风险 | Lua 变第二套大代码库 | Blueprint 变大型图迷宫 | IR 膨胀成劣化脚本语言 |
| 成熟度 | 高 | 极高 | 待建设 |
| 最大优势 | 生态成熟、迭代快 | 官方集成、设计师体验强 | AI-first、可验证、可审查、受限热更 |

## 5. 自走棋装备系统对比

### 5.1 Unity Lua + C#

可能拆法：

```text
Lua:
  can_equip
  equip_cost
  stat_delta_preview
  tooltip display logic
  activity override
  simple UI event handler

C#:
  InventorySystem
  EquipmentSystem
  DragDropController
  UI Framework / UGUI / UI Toolkit integration
  BattleSystem
```

优势：

```text
Lua 热更快。
文本脚本 AI 容易生成。
商业验证多。
```

风险：

```text
Lua 可以继续吞掉 drag/drop、状态机、装备流程。
Lua / C# 双栈调试和桥接成本高。
Lua 动态类型让静态验证和影响分析困难。
```

### 5.2 UE Blueprint + C++

可能拆法：

```text
Blueprint:
  Widget interaction
  can_equip
  tooltip update
  simple equip flow
  designer exposed behavior

C++:
  InventoryComponent
  EquipmentComponent
  Gameplay Ability / Attribute system
  Slate / UMG 底层扩展
  performance critical combat logic
```

优势：

```text
编辑器体验成熟。
可视化强。
设计师友好。
官方工具链强。
```

风险：

```text
Blueprint 图会变大。
AI 生成和审查节点图不稳定。
复杂 UI / gameplay 可能散落在 Widget Blueprint、Actor Blueprint、C++ 多处。
```

### 5.3 本项目 IR + Rust

推荐拆法：

```text
IR:
  can_equip(unit_tags, item_tags, slot_kind) -> bool
  equip_cost(item_level, unit_level) -> number
  stat_delta_preview(unit_stats, item_stats) -> object
  trait_requirement(unit_traits, item_requirement) -> bool

Rust:
  AUI drag/drop
  hit test / focus / scroll / tooltip lifecycle
  EquipmentSystem
  equip action handler
  ProjectUiStateSnapshot builder
  complex sorting / filtering / search
```

配合 `195` 的 Feature Folder：

```text
features/equipment_panel/
  ui/equipment_panel.aui
  rules/equipment_rules.rule
  logic/equipment_actions.rs
  logic/equipment_view_model.rs
  tests/equipment_panel_cases.json
```

优势：

```text
AI 默认改 Feature scope。
IR 规则可结构化验证。
复杂 UI 交互不进 IR。
Rust 逻辑通过编译和测试兜底。
```

风险：

```text
工具链不成熟。
需要严格守住 IR 红线。
需要 Feature Folder / Report / Test 形成习惯。
```

## 6. 核心差异：上层是不是“语言”

### 6.1 Lua 是语言

Lua 有：

```text
变量
函数
闭包
循环
表
模块
元表
动态调用
```

所以 Lua 的优势是灵活，风险也是灵活。

### 6.2 Blueprint 是可视化语言

Blueprint 有：

```text
事件
变量
函数
宏
分支
循环
对象引用
任意引擎 API 节点
```

所以 Blueprint 的优势是可视化和官方集成，风险是图会无限膨胀。

### 6.3 IR 必须不是语言

IR 应该只有：

```text
受限 trigger
受限 condition
受限 value expression
受限 operation
显式 reads / writes / emits
Contract-bound RuleSlot
```

IR 不应该有：

```text
任意函数
任意循环
递归
任意对象引用
任意数组 / map 编程
任意 UI event
任意 ECS / Renderer / File / Network API
```

这就是本项目和 Unity Lua / UE Blueprint 的分水岭。

## 7. AI 适配性对比

### 7.1 AI 写 Lua

AI 写 Lua 比写节点图容易，但问题是：

```text
动态类型。
运行时错误多。
调用边界自由。
很难在应用前做完整 schema validation。
```

AI 能生成，但不一定容易验证。

### 7.2 AI 写 Blueprint

AI 写 Blueprint 的难点是：

```text
节点类型多。
引脚连接复杂。
图布局和语义混在一起。
diff / review 不如文本稳定。
```

AI 能辅助，但长期自动 patch 难度高。

### 7.3 AI 写 IR

AI 写 IR 的优势是：

```text
结构化 JSON / DSL。
op 集有限。
schema 可验证。
诊断可结构化返回。
diff 可审查。
影响范围可由 reads / writes / emits 推导。
```

但前提是：

```text
IR 不能变成通用语言。
```

## 8. 热更新对比

### 8.1 Unity Lua

Lua 热更能力强，适合活动和线上修复。

问题：

```text
热更的是自由脚本。
验证困难。
越热更越可能积累第二套项目逻辑。
```

### 8.2 UE Blueprint

Blueprint 是资产化脚本，编辑器和引擎集成成熟。

问题：

```text
Blueprint 作为完整脚本系统，热更和运行时替换仍需处理完整程序语义。
大型 Blueprint 的影响分析复杂。
```

### 8.3 本项目 IR

IR 热更范围更窄：

```text
公式
条件
权重
简单显示规则
简单业务规则
```

优势：

```text
因为范围窄，所以可以做验证、签名、影响分析和回滚。
```

代价：

```text
复杂流程不热更。
复杂 Rust Module 需要重新构建和发布。
```

这是主动选择，不是缺陷：

```text
用热更范围换可验证性。
```

## 9. 长期维护对比

### 9.1 Unity Lua + C#

长期风险：

```text
Lua / C# 职责边界漂移。
同一业务一部分在 Lua，一部分在 C#。
桥接层、生命周期、热更状态成为 bug 来源。
```

治理方式通常依赖：

```text
团队规范
目录约定
code review
runtime logging
```

### 9.2 UE Blueprint + C++

长期风险：

```text
Blueprint 图膨胀。
Widget / Actor / Component Blueprint 逻辑散落。
复杂逻辑回迁 C++ 成本高。
```

治理方式通常依赖：

```text
Blueprint style guide
C++ base class
Gameplay framework discipline
Editor tooling
```

### 9.3 本项目 IR + Rust

长期风险：

```text
IR 红线松动。
Feature scope 不清晰。
规则和 Rust 分工反复摇摆。
```

治理方式应来自 `195`：

```text
IR 红线。
Feature Asset / Feature Folder。
RuntimePackage 真相。
AUI Binding 只读 ProjectUiStateSnapshot。
Report / Test / SourceMap。
```

注意：

```text
本项目不应新增 Logic Ownership Router 作为治理层。
应通过 Feature scope 降低判断和搜索成本。
```

## 10. 我们的优势成立条件

IR + Rust 的优势不是天然存在的。

它只在以下条件全部成立时成立：

```text
1. IR 只写受限规则数据，不变成脚本语言。
2. 复杂流程、复杂 UI、复杂算法坚定放 Rust。
3. AUI Document 只做 UI 结构，不保存项目运行时语义。
4. Binding 只读 ProjectUiStateSnapshot，不读 ECS，不调用 Project Rule。
5. AuiAction 只表达业务意图，Project Logic 负责执行。
6. Feature Folder 聚合 ui / rules / logic / tests，降低 AI 搜索范围。
7. 所有规则变更有 schema validation / impact report / source map / tests。
```

只要破坏其中最关键的一条：

```text
IR 开始支持任意函数 / 循环 / 状态机 / UI event。
```

则本项目路线会退化为：

```text
差一点的 Lua
差一点的 Blueprint
```

## 11. 最终裁定

### 11.1 是否只是换皮

不是简单换皮。

原因：

```text
Unity Lua + C# 和 UE Blueprint + C++ 的上层都是可编程脚本层。
本项目 IR 的上层必须是受限规则数据层。
```

### 11.2 相似点

确实相似：

```text
都有上层可变逻辑。
都有下层复杂系统代码。
都需要长期维护边界。
```

不能否认这点。

### 11.3 结构性差异

差异在：

```text
Lua / Blueprint 追求表达力。
IR 追求可验证性。

Lua / Blueprint 可以越写越复杂。
IR 必须在复杂前停止。

Lua / Blueprint 的热更 / 编辑能力来自“能编程”。
IR 的热更 / 编辑能力来自“不能随便编程”。
```

### 11.4 推荐保持当前路线

继续采用：

```text
IR + Rust
```

但必须按 `195` 执行：

```text
IR 红线
AUI / IR / Rust 边界
Feature Folder authoring scope
Report / Test / SourceMap
```

## 12. 参考

本项目正式文档：

```text
03-系统分层与混合数据模型.md
05-逻辑系统边界-DSL-IR-RustAOT-ECS.md
09-热更新能力边界.md
10-技术路线与迁移.md
193-Rule-Authoring-Productization-v1方案.md
194-Gameplay-Rule-Asset-and-Rust-Framework-Two-Layer-Mental-Model方案.md
195-Gameplay-Rule-Asset-Rust-Framework-IR-Redline-and-AUI-Logic-Boundary方案.md
```

其它 AI 审查参考：

```text
19-IR膨胀困境分析.md
21-IR vs Blueprint AI适配性对比.md
22-IR vs Blueprint 修正对比(Rust也是AI写的).md
23-全IR方案审查.md
```

外部参考：

```text
Unity Visual Scripting
https://docs.unity3d.com/2022.2/Documentation/Manual/com.unity.visualscripting.html

Unity IL2CPP scripting backend
https://docs.unity3d.com/6000.5/Documentation/Manual/scripting-backends-il2cpp.html

xLua
https://github.com/Tencent/xLua

Unreal Engine Blueprint Visual Scripting
https://dev.epicgames.com/documentation/unreal-engine/blueprints-visual-scripting-in-unreal-engine

Unreal Engine Blueprint Overview
https://dev.epicgames.com/documentation/unreal-engine/overview-of-blueprints-visual-scripting-in-unreal-engine
```

