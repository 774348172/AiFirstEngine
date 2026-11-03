# 逻辑系统边界-DSL-IR-WASM-ECS

## 逻辑系统边界

讨论 DSL、WASM、Rust、Lua 前，必须先区分逻辑属于哪一层。  
核心原则：

```text
引擎逻辑 = 提供通用执行能力和规则框架
项目逻辑 = 定义具体游戏玩法和内容行为
```

不同语言和执行载体不是天然属于引擎或项目，关键看它们表达的是什么。

### 四层逻辑归属

正式分层：

```text
Core Engine Logic
  通用底层能力，主要由 Rust 实现

Standard Module Logic
  通用但可选的游戏运行时能力，Rust 实现 + schema / DSL / Graph / API 暴露

System Blueprint / Game Blueprint Logic
  系统组织方式、规则槽位、默认策略、适配 Profile 和验证规则

Project Logic
  具体游戏玩法、数值、规则、内容、表现绑定，主要由 Spec / DSL / Graph / WASM / 数据表达
```

Core Engine Logic 包括：

```text
ECS 调度
Scene / Prefab 加载
Asset 加载
Input 抽象
Audio 播放
Renderer
Physics
Animation
UI Runtime
Resource Manager
Patch Transaction
Validation
Architecture Governance
Build Pipeline
Hot Update Manager
WASM Host Runtime
DSL Compiler / Interpreter
Module System
Platform Abstraction
```

这些能力应稳定、通用、跨项目复用，默认不由用户自然语言生成或修改。

Project Logic 包括：

```text
玩家冲刺技能
敌人攻击规则
自走棋羁绊效果
商店刷新规则
卡牌效果
任务条件
关卡触发器
怪物行为
掉落规则
伤害公式
UI 交互规则
剧情分支
技能 VFX 绑定
```

这些是 AI 主要生成和修改的目标，但必须通过 Spec、Patch Plan、Architecture Governance 和 Validation。

### 当前正式路线说明

```text
当前正式路线：
DSL / Graph -> IR
开发 / 验证 / 热更覆盖 -> IR Interpreter
正式发布 -> IR -> Rust AOT
WASM 暂不作为当前默认项目逻辑主路线
```

本文中部仍保留部分早期 `IR -> WASM` 的历史论证，用于记录路线演进，不再作为当前架构规则。  
当前规则以本节和文末 `Project Rule System 正式边界`、`ECS 与上层项目模型映射` 为准。

### Rust / DSL / IR / AOT 的定位

推荐主路径：

```text
Natural Language
  -> Feature Spec
  -> DSL / Graph
  -> IR
  -> IR Interpreter，用于编辑器 / 验证 / 热更覆盖
  -> IR -> Rust AOT，用于正式发布
  -> Rust ECS Runtime
```

Native / IR 的正式边界：

```text
Native 负责能力
IR 负责规则真相
IR Interpreter 负责解释执行
Rust AOT 负责发布性能
```

更具体地说：

```text
引擎模块 / 标准模块 -> Rust -> 各平台原生机器码
项目逻辑模块 -> DSL / IR -> IR Interpreter / Rust AOT
```

即：

```text
Engine / Standard Module = Native
Project Logic Truth = IR
Project Logic Debug / Hotfix = IR Interpreter
Project Logic Release = Rust AOT
```

引擎模块和标准模块需要：

```text
最高性能
平台能力
底层资源访问
稳定 ABI
长期维护
渲染 / 物理 / 资源 / 网络 / 构建能力
```

因此它们应编译为各目标平台的原生机器码：

```text
Windows -> x64 / ARM64 native
Android -> arm64-v8a .so
iOS -> arm64 static lib / framework
macOS -> arm64 / x64 native
Web -> Web runtime / WebAssembly backend
```

项目逻辑需要：

```text
AI 可控
可验证
可热更
可回滚
跨平台一致
权限受限
用户不破坏引擎
```

因此项目逻辑默认生成 WASM，而不是平台机器码。

运行结构：

```text
Platform Native Shell
  -> Native Engine Runtime
       Rust Core Engine
       Rust Standard Modules
       wgpu Renderer
       ECS
       Physics
       Asset System
       WASM Host
  -> Project Package
       Feature Specs
       DSL / Graph
       IR
       project_logic.wasm
       Asset Bundles
       Scene / Prefab
       Config
```

自走棋示例：

```text
Native Engine / Standard Modules:
  Deterministic Simulation Module
  Grid Board Module
  Ability Runtime
  Buff Runtime
  Targeting Runtime
  Event System
  Replay Runtime
  Asset Loader
  Renderer
  Audio
  Input

Project WASM:
  具体技能
  具体 Buff 效果
  具体羁绊
  商店规则
  经济规则
  回合奖励
  伤害公式
  胜负结算
  AI 行为规则
```

如果项目 WASM 出现性能瓶颈，不应默认让项目生成原生机器码。  
正确处理流程：

```text
Profiler 确认热点
  -> 判断是否是通用能力
  -> 如果通用，沉淀为 Rust Standard Module operation
  -> DSL 调用该 native operation
  -> Project WASM 只负责规则参数和组合
```

例如：

```text
复杂寻路慢 -> Navigation Module 用 Rust 原生实现，DSL 调用 find_path()
大规模目标搜索慢 -> Targeting Module 用 Rust 原生实现，DSL 调用 select_targets()
```

这样既保留性能，又不牺牲 AI 可控性。

### 为什么 IR 默认生成 WASM，而不是 C# / C++ Native

项目逻辑主路线确认：

```text
IR -> WASM
```

不采用默认路线：

```text
IR -> C# / C++ -> 各平台机器码
```

原因不是 Native 性能不好，而是项目逻辑的首要目标不是极限性能，而是：

```text
AI 可控
可验证
可热更
多平台一致
用户不懂代码也能维护
迭代快速
```

两条路线对比：

```text
路线 A：
Feature Spec -> DSL / Graph -> IR -> WASM -> Native Runtime Host

路线 B：
Feature Spec -> DSL / Graph -> IR -> C# / C++ -> 各平台编译器 -> 平台机器码
```

#### AI 可控性

IR -> WASM：

```text
项目逻辑只能通过 WASM Host ABI 访问引擎
可以限制 StatePatch / CallNative / EmitEvent / Query
可以做 host call 白名单
可以做权限检查
可以做 fuel / step limit
不能随便访问系统 API
```

IR -> C# / C++ -> Native：

```text
C++ 可以访问内存、崩溃进程、越权调用平台 API
C# 也涉及 GC、反射、线程、IO、AOT 和运行时差异
如果强行限制 C# / C++ 子集，最终等于重新发明一套沙箱语言
```

结论：

```text
AI 可控性：WASM 更适合作为项目逻辑主路线
```

#### 可验证性

IR -> WASM：

```text
只需要验证一种统一目标产物
可以做固定 ABI 检查
可以禁止未声明 host call
可以保留 source map
可以做 hash / signature
可以做 deterministic profile 检查
```

IR -> C# / C++ -> Native：

```text
需要验证生成代码是否等价于 IR
需要验证不同平台编译结果是否一致
需要处理 MSVC / clang / gcc / Android NDK / Apple clang 差异
需要保证 native 产物没有越权行为
```

结论：

```text
可验证性：WASM 更简单、更稳定
```

#### 热更

IR -> WASM：

```text
下载 project_logic.wasm
校验 hash / signature / ABI
在安全点切换
更容易包装成受控规则资源
```

IR -> C# / C++ -> Native：

```text
本质接近下载动态库或替换平台机器码
桌面端可行但复杂
Android / iOS 尤其 iOS 风险高
C# 热更也会遇到 AOT、IL 加载、解释器和平台限制
```

结论：

```text
热更：WASM 明显更适合项目逻辑
```

#### 多平台一致性

IR -> WASM：

```text
同一份 project_logic.wasm
运行在 Windows / Android / iOS / Web 的 Native Runtime Host 上
只要 Host ABI 一致，逻辑更容易保持一致
```

IR -> C# / C++ -> Native：

```text
每个平台独立编译
浮点、编译优化、平台 ABI、运行时行为都可能产生差异
Web 平台还要单独处理
```

结论：

```text
多平台一致性：WASM 更适合
```

#### 查 Bug 和维护

IR -> WASM 可以保留链路：

```text
WASM function
  -> Lowered Instruction IR
  -> Rule Graph IR
  -> DSL node
  -> Requirement ID
```

Bug 修复路径：

```text
Runtime Trace
  -> IR
  -> DSL
  -> Feature Spec
  -> AI Patch
  -> 重新编译 WASM
```

IR -> C# / C++ -> Native 会多一层生成代码：

```text
IR -> C# / C++ -> Native
```

一旦用户或 AI 直接修改生成代码，就会破坏：

```text
Feature Spec
DSL
IR
WASM / Native
Trace Map
```

之间的真相链路。

结论：

```text
维护性：WASM 路线更适合 AI 原生项目
```

#### 性能

Native 的性能更强，适合：

```text
大规模寻路
复杂物理
海量 AI
空间查询
图像处理
压缩 / 解压
平台 SDK
```

但本引擎已经确认：

```text
Native 负责能力
WASM 负责规则
```

因此重计算应沉淀到：

```text
Rust Native Standard Module
```

项目 WASM 只负责：

```text
规则判断
条件组合
数值公式
Command validation
Modifier logic
Query filter / sort
```

结论：

```text
性能：Native 更强，但不适合作为普通项目规则的默认生成目标
```

#### 编译和迭代效率

IR -> WASM：

```text
编译快
产物小
适合频繁 AI 修改
适合快速测试和热更
```

IR -> C++ -> Native：

```text
编译慢
链接慢
每次 AI 修改规则都可能触发平台编译和重新打包
```

结论：

```text
迭代效率：WASM 更适合自然语言驱动开发
```

### 最终边界

正式边界：

```text
Project Logic 主路线：IR -> WASM
Engine / Standard Module 路线：Rust -> Native
高性能能力：Native Standard Module
```

不建议：

```text
普通项目规则 -> C++ native
普通项目规则 -> C# native
```

最终结论：

```text
IR -> WASM 是为了让项目逻辑可控、可验证、可热更、可维护。
IR -> Native 是为了最高性能，更适合引擎能力和标准模块，不适合作为 AI 默认生成项目逻辑。
```

正式规则：

```text
用户自然语言不能直接修改引擎逻辑
DSL 不能修改引擎逻辑
用户可以修改项目逻辑
用户可以修改、创建、导入 System Blueprint / Game Blueprint
System Blueprint / Game Blueprint 不属于 Core Engine Logic
项目逻辑默认由 DSL / Graph 生成 IR，再编译成 WASM
Rust 主要用于 Core Engine 和 Standard Module 底层实现
```

用户逻辑生成流程：

```text
用户自然语言需求
  -> AI 扫描项目上下文
  -> AI 补全需求并向用户提问
  -> 形成 Feature Spec / 需求文档
  -> 用户确认
  -> 根据 Feature Spec 生成 DSL / Graph
  -> 根据 DSL / Graph 生成 IR
  -> IR 通过验证
  -> IR 编译成 WASM
  -> runtime 执行 WASM
```

如果用户需求触及 Core Engine Logic 或 Standard Module 底层实现，不能通过项目 DSL 修改。  
引擎必须提示这是引擎能力或标准模块能力变更，需要进入：

```text
引擎模块开发流程
标准模块开发流程
原生插件开发流程
引擎版本升级流程
```

而不是普通项目逻辑生成流程。

DSL 的职责：

```text
描述玩法规则
描述系统规则槽位
描述数据变化规则
描述事件响应
描述条件判断
描述表现绑定
描述热更规则
描述测试场景
```

DSL 不负责：

```text
修改 ECS 调度器
修改渲染器
修改资源加载器
修改物理引擎
修改 WASM Host
修改平台层
修改 Build Pipeline
修改 Architecture Governance
修改 Hot Update Manager
修改 Standard Module 的 Rust 底层实现
```

### Feature Spec / DSL / IR / WASM 的职责边界

用户需求生成链路不能用“完美复原”作为工程目标。  
更准确的目标是：

```text
Feature Spec 保存完整需求
DSL 覆盖其中可执行规则
Trace Map 证明需求到 DSL 的覆盖关系
IR 由确定性编译器从 DSL 生成
WASM 由确定性编译器从 IR 生成
```

正式定义：

```text
Feature Spec = 需求真相
DSL = 可执行规则真相
IR = 验证和编译真相
WASM = 运行真相
Trace Map = 需求、规则、编译结果和运行行为之间的追踪关系
```

DSL 不负责完整复原用户文档。  
用户文档中可能包含：

```text
设计意图
用户体验描述
美术风格
参考游戏
可选方案
未来扩展
用户确认记录
商业目标
```

这些内容应保存在 Feature Spec 中。  
DSL 只表达可执行、可验证、可编译的项目规则。

例如 Feature Spec 中的需求：

```json
{
  "featureId": "mech_suicide_unit",
  "requirements": [
    {
      "id": "R1",
      "text": "单位血量低于 20% 时触发自爆行为"
    },
    {
      "id": "R2",
      "text": "触发后冲向最近敌人"
    },
    {
      "id": "R3",
      "text": "自爆造成范围伤害"
    },
    {
      "id": "R4",
      "text": "机械族羁绊会提高自爆伤害"
    },
    {
      "id": "R5",
      "text": "播放红色闪烁和爆炸特效"
    }
  ]
}
```

对应 DSL：

```text
skill mech_suicide {
  trigger:
    when self.hp_percent <= 20%

  action:
    move_to nearest_enemy
    deal_area_damage radius 2.5 amount self.max_hp * 0.35
    apply_synergy_modifier mechanical_suicide_bonus
    emit_event "MechSuicideExploded"
}
```

对应 Trace Map：

```json
{
  "R1": ["dsl.skill.mech_suicide.trigger.hp_percent"],
  "R2": ["dsl.skill.mech_suicide.action.move_to"],
  "R3": ["dsl.skill.mech_suicide.action.deal_area_damage"],
  "R4": ["dsl.skill.mech_suicide.action.apply_synergy_modifier"],
  "R5": ["dsl.skill.mech_suicide.action.emit_event"]
}
```

引擎必须提供 Requirement Coverage Check：

```text
每个可执行需求是否有 DSL 映射
每个 DSL 行为是否来自 Feature Spec 或用户确认
是否存在需求没有实现
是否存在 DSL 私自多实现
是否存在表现需求没有绑定事件
```

如果需求没有映射，不能继续生成 IR / WASM。

### DSL 到 IR 到 WASM 的编译原则

AI 可以生成 Feature Spec 和 DSL，但不能自由生成 IR 或 WASM。  
IR 和 WASM 必须由确定性编译器生成：

```text
DSL
  -> Parser
  -> Typed AST
  -> Semantic Check
  -> IR Generation
  -> IR Validation
  -> WASM Codegen
  -> WASM ABI Check
```

DSL 到 IR 必须保证：

```text
确定性：同一份 DSL 永远生成同一份 IR
类型安全：字段、单位、实体、组件类型都可检查
语义封闭：DSL 中每个操作都有明确 IR 对应
权限受控：DSL 不能表达引擎不允许的行为
错误可定位：IR 错误能回指 DSL 节点和 Requirement ID
可测试：IR 可以独立执行测试
```

IR 到 WASM 必须保证：

```text
确定性 codegen
固定 ABI
无未声明 host call
无不确定性 API
可 hash
可签名
可回放验证
可映射回 IR 节点
```

WASM 模块必须保留 source map：

```text
WASM function / instruction
  -> IR node
  -> DSL node
  -> Requirement ID
```

如果 DSL 无法表达某个需求，AI 不能硬写 DSL。  
引擎应该返回：

```text
当前 DSL 无法表达该需求。
原因：需要新的 Standard Module 能力 / 新的 DSL operation / 新的原生模块。
建议：
1. 扩展现有 System Blueprint
2. 新增 DSL operation
3. 新增 Standard Module
4. 进入 Rust module 开发流程
```

DSL 的正式目标：

```text
1. 可覆盖：覆盖 Feature Spec 中的可执行需求
2. 可约束：不能表达越权或破坏架构的行为
3. 可编译：能确定性生成 IR
4. 可追踪：能从 WASM 运行行为追踪回 DSL 和需求
```

### DSL 边界原则

正式原则：

```text
DSL 表达项目规则，不实现底层能力。
所有项目可执行规则必须通过 DSL / Graph。
复杂底层能力由 Native Standard Module 提供，DSL 只调用。
DSL 采用多个领域 DSL，但共享统一类型系统、事件系统、引用系统、IR 和 WASM 后端。
```

这意味着所有用户需求必须进入 Feature Spec，所有可执行项目规则必须进入 DSL / Graph。  
但并不意味着所有需求都由 DSL 自己实现底层能力。

例如：

```text
用户需求：我要 10000 个单位同时做复杂寻路。
Feature Spec：记录完整需求。
DSL：表达哪些单位需要寻路、目标是什么、触发条件是什么。
Native Standard Module：实现高性能寻路算法。
```

错误做法：

```text
让 DSL 自己实现 A*、空间索引、多线程调度。
```

正确做法：

```text
Navigation Module 用 Rust 原生实现 find_path / find_group_paths。
DSL 调用 Navigation Module 暴露的 operation。
```

当现有 DSL 无法表达用户需求时，不能绕过 DSL 直接生成自由代码。  
必须进入扩展流程：

```text
需求无法用现有 DSL 表达
  -> 判断是项目规则缺口还是引擎能力缺口
  -> 如果是项目规则缺口：扩展领域 DSL / System Blueprint
  -> 如果是通用能力缺口：新增 Standard Module / Native Operation
  -> 再让 DSL 调用新能力
```

### 多领域 DSL + 统一 IR

本引擎不采用一个无限膨胀的通用 GameDSL。  
一个大 DSL 虽然语法统一，但长期会导致：

```text
DSL 越来越像通用编程语言
技能、商店、装备、任务、UI 等语义混在一起
验证规则复杂
系统边界不清楚
AI 生成内容更难稳定
```

推荐采用多个领域 DSL：

```text
Skill DSL
Buff DSL
Shop DSL
Equipment DSL
Quest DSL
Economy DSL
AI Behavior DSL
UI Binding DSL
Presentation Binding DSL
Render / Material DSL
```

每个领域 DSL 只负责自己的项目规则。  
例如 Skill DSL：

```text
skill fireball {
  trigger: on_cast
  target: nearest_enemy
  effect:
    deal_damage target amount caster.ap * 1.2
    apply_buff target burning duration 3s
}
```

Shop DSL：

```text
shop battle_shop {
  slots: 5
  currency: gold
  refresh: each_round
  rarity_weight: by_player_level
}
```

Equipment DSL：

```text
equipment fission_core {
  slot: artifact

  modifier:
    add_skill_modifier chain_bounce_bonus
}
```

这些 DSL 不能各自为政。  
它们必须共享：

```text
统一类型系统
统一 Entity / Component 引用系统
统一 Asset 引用系统
统一 Event / Command / Query 模型
统一权限模型
统一确定性规则
统一 IR
统一 WASM 后端
```

编译链路：

```text
Skill DSL
Buff DSL
Shop DSL
Equipment DSL
Quest DSL
AI Behavior DSL
UI Binding DSL
  -> Typed AST
  -> Common IR
  -> WASM
```

这样既避免一个大 DSL 失控，又能保证所有项目逻辑最终进入统一验证、编译和运行管线。

### IR 设计规则

IR 是从用户需求到可验证 WASM 的结构化桥梁，也是 AI 后期维护项目的索引系统。  
它不是用户直接编辑的内容，也不是 AI 自由生成的代码。

正式链路：

```text
用户自然语言
  -> Feature Spec
  -> 多领域 DSL / Graph
  -> Project System IR
  -> Rule Graph IR
  -> Lowered Instruction IR
  -> WASM
```

职责定义：

```text
Feature Spec = 需求真相
DSL / Graph = 项目规则真相
Project System IR = 系统结构真相
Rule Graph IR = 规则语义真相
Lowered Instruction IR = 编译执行真相
WASM = 运行真相
Trace Map = 全链路追踪关系
```

AI 和编译器边界：

```text
AI 可以生成 Feature Spec
AI 可以生成 DSL / Graph
AI 不能自由生成 IR
AI 不能自由生成 WASM
IR 必须由确定性编译器从 DSL / Graph 生成
WASM 必须由确定性编译器从 IR 生成
```

AI 可以根据验证错误修复上游内容：

```text
Feature Spec
DSL
Graph
Blueprint
Project Policy
```

但不能跳过编译器直接改 IR / WASM。

### Project System IR

IR 不只是技能图，而是项目系统结构。  
顶层结构为：

```text
Project System IR
  Schema IR
  Command IR
  Query IR
  Event IR
  Rule Graph IR
  Profile IR
  Presentation Binding IR
  Native Binding IR
  Test IR
  Source Map IR
```

各部分职责：

```text
Schema IR：系统数据模型
Command IR：系统允许的写操作
Query IR：系统允许的读操作
Event IR：系统发出的事件
Rule Graph IR：具体规则逻辑
Profile IR：不同运行上下文，如局内 / 局外、单机 / 帧同步
Presentation Binding IR：UI / VFX / Audio 表现绑定
Native Binding IR：调用 Native Standard Module 的受控接口
Test IR：由需求生成的测试结构
Source Map IR：需求、DSL、IR、WASM、运行时 trace 的映射
```

编译时：

```text
Project System IR
  -> Validation
  -> Rule Graph IR
  -> Lowered Instruction IR
  -> WASM
```

不是所有 System IR 都直接编译成 WASM。  
不同 IR 部分的用途：

```text
Schema IR -> 生成类型、Inspector、序列化、ABI
Command IR -> 生成 WASM 入口和 Native 调用包装
Query IR -> 生成查询接口和 UI 绑定
Rule Graph IR -> 编译成 WASM 规则逻辑
Profile IR -> 参与验证和构建配置
Presentation Binding IR -> 生成 UI / 表现绑定
Native Binding IR -> 生成 host call ABI
Test IR -> 生成测试
```

真正主要编译成 WASM 的是：

```text
Rule Graph IR
Command validation logic
Query filter / sort logic
Modifier logic
Condition logic
```

### Typed Graph IR 与 Lowered Instruction IR

长期采用两层：

```text
Typed Graph IR
  -> Lowered Instruction IR
```

Typed Graph IR 负责：

```text
语义表达
需求解释
类型检查
权限检查
确定性检查
模块契约检查
热更等级检查
source map
AI 修复定位
调试
```

Lowered Instruction IR 负责：

```text
WASM codegen
优化
指令布局
host ABI 调用
性能分析
```

MVP 阶段可以先做：

```text
Typed Graph IR -> WASM
```

后续再拆出 Lowered Instruction IR。

### IR 单元字段

每个 IR 单元至少包含：

```text
id
kind / op
domain
inputs
outputs
effects
types
determinism
permissions
profile
hotUpdatePolicy
moduleCalls
source
contracts
```

一个规则节点必须能说明：

```text
来自哪个 Requirement
来自哪个 DSL 节点
读取什么数据
写入什么 Component
触发什么事件
调用哪个 Native Module
是否要求确定性
是否允许热更
```

### Component 读写规则

Core Engine 不知道背包、装备、商店等业务概念。  
Core 只提供：

```text
Entity
Component
Resource
Collection
Schema
Transaction
StatePatch
Serialization
Query
Event
WASM Host API
```

项目系统的数据存在：

```text
Project State / ECS World / Component Storage
```

WASM 不直接访问底层内存，而是通过受控接口：

```text
read component
write component
state patch
emit event
call native operation
```

IR 中应表达：

```text
ReadComponent
WriteComponent
StatePatch
EmitEvent
```

以上接口是受控语义接口，不代表允许 WASM 像 Unity MonoBehaviour Update 一样逐对象、逐字段、高频调用 Host API。

正式性能边界：

```text
WASM 不直接高频操作引擎对象。
WASM 不逐字段频繁调用 Host API。
WASM 通过声明式 Query 获取最小输入。
WASM 输出 StatePatch / CommandBuffer。
Native Runtime 批量应用结果。
高频 Transform / Physics / Animation / Render / VFX 由 Native 模块执行。
WASM 只负责项目规则、状态转换、事件响应和决策参数。
```

错误模式：

```text
for enemy in enemies:
  pos = host.get_position(enemy)
  host.set_position(enemy, new_pos)
```

正确模式：

```text
Engine ECS World
  -> 生成本规则需要的只读 Snapshot / Query Result
  -> WASM 执行项目规则
  -> 输出 StatePatch / CommandBuffer
  -> Native Runtime 批量 Apply
```

核心原则：

```text
少量大调用
不要大量小调用
```

### WASM / Host 交互模型

WASM 与 Host 的交互必须以批量数据为主。

推荐通道：

```text
InputBuffer
OutputBuffer
CommandBuffer
EventBuffer
StatePatch
```

示例：

```text
DamageInputBuffer:
  attacker_id
  target_id
  attack
  defense
  shield
  hp

DamageOutputBuffer:
  target_id
  new_shield
  new_hp
  reflected_damage
  emitted_events
```

避免：

```text
频繁传字符串
频繁传复杂对象
频繁分配小对象
每个字段一次 Host Call
WASM 持有完整 Scene / Prefab / Transform 树
```

WASM 只持有：

```text
规则输入
局部状态
临时计算数据
少量持久逻辑状态
```

大对象和高频状态留在 Native ECS：

```text
完整 Scene
完整 Prefab
Transform 树
贴图 / Mesh / Animation 数据
物理世界
粒子系统
渲染状态
```

### Query 与 Snapshot

WASM 不应自己逐个查询所有 Entity 和 Component。  
Query 由引擎根据 DSL / IR 声明执行。

例如 DSL / IR 声明：

```text
需要 Transform + Faction + Health
空间范围：space_index 内半径 radius
```

Native Runtime 执行：

```text
SpatialCompute.query_radius(space_index, center, radius)
读取必要 Component
生成紧凑 CandidateBuffer
```

WASM 接收：

```text
CandidateBuffer:
  entity_id
  position
  health
  faction
```

WASM 再根据项目规则过滤：

```text
has Health
Faction 与 caster 敌对
Health.current_hp > 0
```

这样可以把调用模式从：

```text
N * 多次 Host API
```

降低为：

```text
1 次 Query
1 次 Snapshot 输入
1 次 Patch 输出
```

### 高频能力边界

以下能力不应由项目 WASM 逐帧直接控制：

```text
Transform 高频更新
骨骼动画
物理模拟
粒子模拟
寻路更新
大批量单位移动
渲染可见性
碰撞检测
```

它们应由：

```text
Rust Native Runtime
Core Compute Module
Engine Capability Module
```

执行。

项目 WASM 只输出意图：

```text
ApplyMoveIntent(entity, target, speed_modifier)
ChangeAnimationState(entity, Attack)
ApplyBuff(entity, speed_multiplier)
SpawnVfxCommand(asset_ref, transform)
```

Native Runtime 负责：

```text
Transform interpolation
Path following
Animation update
Physics sync
VFX spawn
Audio playback
```

### 内存管理边界

WASM 沙箱内存隔离是优势，也是成本。

正式规则：

```text
WASM 不保存大资源
WASM 不保存完整世界
WASM 不频繁复制大对象
Host / WASM 通过紧凑 Typed Buffer 通信
Entity / Asset 使用 handle / id 引用
CommandBuffer / EventBuffer 每帧复用
WASM 临时内存使用 arena / frame allocator
```

这样避免：

```text
跨边界复制大对象
WASM 内存膨胀
GC / allocator 抖动
Host 与 WASM 状态双写
```

### WASM 适用范围

适合放在 WASM 的项目规则：

```text
伤害结算
技能释放
商店刷新
任务推进
掉落结算
回合开始 / 结束
战斗事件处理
Buff 添加 / 移除
关卡触发
UI 数据绑定规则
```

不适合由 WASM 逐对象高频执行：

```text
每帧移动
物理
动画采样
粒子
大规模 AI steering
Transform 批量同步
```

结论：

```text
WASM 不是 Unity C# MonoBehaviour Update 的直接替代。
WASM 是受控、可验证、可热更的项目规则执行层。
```

Graph IR 可以保留高层业务操作：

```text
DealDamage
AddItem
RemoveItem
ApplyBuff
```

然后 lower 成具体读写和 native call。

### Native Standard Module 调用规则

正式边界：

```text
Native 负责能力
WASM 负责规则
```

IR 通过 Native Binding IR 调用 Native Standard Module。

示例：

```text
CallNative:
  module = Container
  function = insert_or_stack
  contract = Container.insert_or_stack.v1
```

检查项：

```text
模块是否启用
函数是否存在
ABI 是否匹配
参数类型是否正确
是否 deterministic
当前 Profile 是否允许调用
是否允许热更
权限是否满足
```

Standard Module 属于引擎层可选原生能力，不属于项目逻辑。  
默认由引擎官方提供，也可以由可信第三方或高级用户通过独立模块开发流程提供。

### Event IR

事件必须区分：

```text
Logic Event：影响逻辑，可参与确定性
Presentation Event：只影响表现
System Event：引擎内部事件
```

规则：

```text
逻辑层可以发 Logic Event
表现层可以监听 Logic Event
表现层可以发 Presentation Event
Presentation Event 不能影响确定性逻辑
UI / VFX / Animation 不能直接修改逻辑状态
```

### 条件、分支和循环

IR 支持：

```text
If
Switch / Match
ForEach with bounded collection
ForRange with max count
Sequence
```

早期不支持：

```text
while
递归
无限循环
异步 await
线程
自由 IO
```

如果需求需要复杂循环或高性能算法：

```text
沉淀为 Native Standard Module operation
DSL / IR 只调用它
```

这样避免 DSL / IR 变成通用编程语言。

### 确定性规则

每个 IR 节点必须声明：

```text
determinism: required / not_required / presentation_only / profile_dependent
```

帧同步 Profile 下禁止：

```text
系统时间
本地随机数
无序集合遍历
非确定浮点
文件系统
网络 IO
UI 状态读取
表现层状态读取
```

允许：

```text
DeterministicRng
FixedPoint / Integer
排序后的实体查询
固定 tick
声明式事件
声明为 deterministic 的 Native Operation
```

### Source Map / Trace Map

每个 IR 单元必须追踪到：

```text
Feature Spec
Requirement ID
DSL file
DSL node
Graph node
Blueprint source
Patch Plan op
```

WASM 也要能反向追踪：

```text
WASM function
  -> Lowered Instruction IR
  -> Typed Graph IR
  -> DSL node
  -> Requirement ID
```

用于：

```text
Bug 定位
AI 修复
需求覆盖检查
回归测试
运行时 trace
```

### Requirement Coverage Check

引擎必须检查：

```text
每个可执行需求是否有 DSL 映射
每个 DSL 行为是否来自 Feature Spec 或用户确认
每个 DSL 是否生成 IR
每个关键 IR 是否有测试
是否存在需求没有实现
是否存在 DSL 私自多实现
是否存在表现需求没有绑定事件
```

没有通过 Coverage Check，不能生成 WASM。

### IR 示例：背包系统

背包系统不应该让 Core 知道背包业务。

正确结构：

```text
Core Engine:
  Component Storage
  List / Map / Set
  Schema
  StatePatch
  Transaction
  Serialization
  WASM Host API

Optional Standard Module:
  Container Module
  insert
  remove
  move
  stack
  sort
  filter
  batch operation

Inventory Blueprint:
  背包系统结构
  局内 / 局外 Profile
  Command / Query / Event
  UI Binding
  规则槽位
  测试场景

Project WASM:
  当前游戏背包规则
  容量限制
  能否删除
  是否可堆叠
  局内 / 局外差异
```

最终原则：

```text
IR 表达项目系统结构，不只表达单个技能流程。
IR 不实现底层能力，只描述规则、状态读写、事件和 Native 调用。
Core Engine 管通用状态能力，不管具体业务。
Standard Module 提供可选原生能力。
System Blueprint 组织项目系统。
Project WASM 执行具体项目规则。
```

### ECS / Scene / Prefab 定位

本引擎采用混合模型：

```text
编辑器层：Scene / Prefab / Component
AI 层：Feature Spec / DSL / IR
运行时层：Rust ECS World
资源层：Asset Database
打包层：Build Profile
```

参考传统引擎现状：

```text
Unity 主流模型：GameObject + Component + MonoBehaviour
Unity 高性能可选路线：DOTS / Entities ECS

Unreal 主流模型：Actor + Component + UObject / Blueprint / C++
Unreal 大规模数据路线：MassEntity
```

因此，本项目不采用“让用户直接写 ECS”的路线。  
ECS 是 Native Runtime 的内部状态和执行模型，不是用户主要心智模型。

用户和编辑器主要看到：

```text
Scene
Prefab
Entity / Object
Component Inspector
Graph
Feature Spec
Patch Review
```

AI 主要操作：

```text
Feature Spec
DSL / Graph
Project System IR
Patch Plan
Validation Report
```

Runtime 内部使用：

```text
Rust ECS World
Entity ID
Component Storage
Query
System Scheduler
Command Buffer / StatePatch
Serialization
Deterministic Tick，可选
```

### 为什么使用 ECS

选择 ECS 不是因为传统引擎已经完全 ECS 化，而是因为它更匹配本项目的 AI 原生目标。

ECS 对本引擎的价值：

```text
结构化状态，便于 AI 理解和修改
Component Schema 适合 DSL / IR / WASM 读写
统一 ReadComponent / StatePatch / EmitEvent 接口
适合确定性模拟、帧同步、Replay 和 Hash 校验
适合热更、存档、diff、rollback 和 migration
适合 Native Runtime 做高性能批量查询和调度
```

WASM Project Logic 不直接访问对象内存，而是通过：

```text
ReadComponent
StatePatch
EmitEvent
CallNative
```

访问 ECS 状态。  
这比暴露大量对象方法更适合 AI 生成、验证和维护。

### Scene / Prefab 与 ECS 的边界

Scene / Prefab 是编辑器项目模型：

```text
方便用户查看
方便可视化摆放
方便 Inspector 编辑
方便 AI Patch Review
方便资源引用和层级组织
```

ECS World 是运行时执行模型：

```text
高性能存储
批量查询
系统调度
确定性 tick
状态快照
运行时事件
```

加载流程：

```text
Scene / Prefab / Component Data
  -> Schema Validation
  -> Runtime Build
  -> ECS World
  -> WASM Project Logic 通过 Host API 读写状态
```

用户不需要理解：

```text
archetype
chunk
sparse set
system schedule internals
storage layout
```

这些属于 Core Engine / Native Runtime 内部实现。

### ECS 边界规则

正式规则：

```text
ECS Runtime Framework 属于 Core Engine / Native Runtime。
Project Component Schema 属于 Project Layer。
Project System Rules 属于 Project Logic，通过 DSL / IR / WASM 表达。
Standard Modules 可以提供通用 ECS System 能力。
用户不能修改 ECS 底层规则，但可以定义项目组件、系统规则、命令、查询和事件。
```

一句话：

```text
ECS 的运行机制归引擎，ECS 上跑的项目规则归用户项目。
```

### Project Rule System 正式边界

当前正式模型：

```text
Rust ECS 是运行时状态真相。
Entity / Component / System 调度框架属于 Rust Runtime / Engine Core。
IR 是项目规则运行真相。
IR Interpreter 和 Rust AOT 是 IR 的两个执行后端。
```

也就是说：

```text
Entity 管理 = Rust Runtime
Component Storage = Rust Runtime
System Scheduler = Rust Runtime
Native System = Rust 实现
Project Rule System = Rust Domain Runtime + IR Rule
IR Interpreter = 编辑器 / 验证 / 热更覆盖执行后端
Rust AOT = 正式发布高性能执行后端
IR Rule = 项目规则计算，不直接拥有 ECS World
```

Project Rule System 属于项目层，但运行时由 Rust Runtime 调度。  
它不是用户手写 Rust，也不是引擎写死的具体玩法。

生成来源：

```text
用户自然语言需求
  -> AI 生成 Feature Spec / System Spec
  -> AI 生成 Schema / DSL / System Blueprint
  -> 编译器生成 IR
  -> 编辑器 / 验证 / 热更时由 IR Interpreter 执行
  -> 正式发布时由 IR -> Rust AOT 生成原生规则函数
  -> Build Pipeline 生成 Runtime System 配置和规则后端
  -> Rust Runtime 调度执行
```

Project Rule System 的结构：

```text
System Blueprint:
  trigger
  read schema
  write schema
  event schema
  rule function binding
  validation rule

Rust Orchestrator:
  监听 trigger
  查询 ECS 数据
  组装 IR Rule 输入
  按 RuleRegistry 选择 IR Interpreter 或 Rust AOT
  验证规则输出
  写回 Component Storage
  发出 Event / Command

IR Rule:
  根据输入计算输出
  不直接访问 ECS 底层存储
  不直接操作 Entity 生命周期
  不直接 mount Bundle / 加载资源 / 播放表现
```

示例：伤害系统

```text
DamageSystem.blueprint:
  trigger: OnAttackHit
  read:
    - attacker.CombatStats
    - target.Health
    - target.Defense
    - world.Environment
  write:
    - target.Health
  emit:
    - DamageEvent
  rule:
    ir_rule: calculate_damage
```

运行时：

```text
Rust DamageSystem Orchestrator
  -> 监听 OnAttackHit
  -> 查询 attacker / target / world state
  -> 构造 DamageInput
  -> 调用 calculate_damage(input)，无热更时走 Rust AOT，有热更覆盖时走 IR Interpreter
  -> 验证 DamageOutput
  -> 写回 target.Health
  -> 发出 DamageEvent
```

IR Rule 只负责：

```text
calculate_damage(input) -> output
```

因此边界是：

```text
System 流程组织 = Rust Runtime
具体项目规则计算 = IR Rule
系统定义和绑定 = Project System Blueprint / DSL
项目数据结构 = Project Component Schema
真实运行时状态 = Rust ECS World
```

这个模型取代“规则执行层像脚本一样频繁调用 Host API 读写 ECS”的默认想法。  
IR Rule 不把 Rust ECS 变成可随意读写的远程对象系统；Rust Runtime 负责查询、验证、写回和调度。

### ECS 与上层项目模型映射

正式规则：

```text
E 来自 Scene / Prefab / Runtime Spawn Request。
C 来自 Component Schema + Component Data。
S 来自 Native Module + System / Domain Blueprint + IR Rule。
```

对应关系：

```text
ECS Entity:
  上层来源 = Scene Entity / Prefab Entity / Runtime Spawn Request
  运行时含义 = Rust Runtime 中的 EntityId

ECS Component:
  上层来源 = Component Schema / Scene Component Data / Prefab Component Data / Runtime Component Data
  运行时含义 = Rust Runtime 中挂在 Entity 上的 Component Storage

ECS System:
  Native System 来源 = Engine Core / Native Module
  Project Rule System 来源 = System / Domain Blueprint + IR Rule
  运行时含义 = Rust Runtime 调度的系统执行单元
```

一句话：

```text
上层 Scene / Prefab 生成 Entity。
上层 Schema / Component Data 生成 Component。
上层 Native Module / Blueprint / IR Rule 生成或驱动 System。
```

