# AI First Game Engine 权威产品需求 v1.1

> 文档类型：权威产品需求，不是架构方案，不是施工文档。  
> 当前状态：用户已确认作为项目最高级产品需求入口。  
> 需求日期：2026-08-30。  
> 最近修订：2026-09-02；收敛 Engine workflow、Skill 与 AI 可见工具的职责。  
> 最新需求优先级：高于此前关于默认 Editor 中间层、Editor authority 和固定线性 AI 工作流的历史表述。  
> 施工状态：禁止据此直接施工。  
> 关联研究：`框架设计/DeepSeekHarness源码参考/AI-First-Game-Engine-Harness-Native-Game-Project-Compiler-研究方案-v0.3.md`。  

## 0. 文档权威性和使用规则

### 0.1 文档目的

本文统一记录 AI First Game Engine 的产品目标、用户体验、竞争目标、平台目标、质量目标和不可妥协边界。

本文只回答：

```text
我们最终要做成什么产品？
用户和 AI 应该怎样使用它？
什么结果才算满足需求？
```

本文不回答：

```text
具体 Module、crate、进程和 schema 如何设计？
先施工哪个系统？
每个 Gate 使用什么命令？
```

这些内容属于架构方案和施工文档，不能反过来改变产品需求。

### 0.2 审核前后的状态

当前文件已经用户确认，是项目最高级产品需求入口。

若其它文档与本文冲突：

```text
权威产品需求
  > 正式架构方案
  > 系统设计方案
  > 施工文档
  > 当前实现
  > 历史研究和实验记录
```

必须修改下层文档或实现，不允许用当前实现困难、历史 Editor 投入或既有工具合同反向削弱需求。

### 0.3 不授权施工

本文即使经用户确认，也只确认产品需求。后续仍需完成架构冲突检查、正式方案、方案自审、施工文档和施工激活，才能修改产品代码。

## 1. 用户最新需求的规范化表达

用户最新需求统一解释为：

> AI First Game Engine 应当像 Web、文件系统和命令行一样，成为 Codex 等 AI 可以直接使用的游戏开发平台。AI仍然以自己原生写代码、资源、UI、测试和工程文件的方式制作游戏，但不再被迫为每个项目临时搭建 Web 小引擎，也不再经过传统游戏编辑器这一中间层。

> 自研引擎必须为 AI 提供比直接使用 Web 更完整、更高性能、更适合原生交付的游戏能力，包括桌面和手机端运行、原生打包、高级渲染、资源管线、真实 Runtime、测试和交付验证。

> 长期目标是 AI + 自研引擎制作游戏的速度、玩法质量、视觉质量、修复效率、运行性能和交付完整性，综合超过 AI + Web、AI + Unity、AI + Unreal Engine 和 AI + Godot，同时保持比传统引擎更贴近 AI 原生编写游戏的使用方式。

本文将用户口述中的“克洛叉”统一解释为 Codex，将“AI直接写游戏”基线统一解释为 Codex 等 AI 使用普通代码、文件和命令行直接开发，将相关“Web”表述解释为 AI 当前常用的 HTML/Canvas/JavaScript/Web Runtime 路径。

### 1.1 本次新增根本需求的一句话版本

> 把自研游戏引擎注册成 AI Agent Tool Runtime 中的一等工具提供者。AI今天怎样直接调用 `write_file` 修改配置、调用 `run_command` 执行程序，未来就应怎样直接调用 `engine_runtime_run`、`engine_runtime_playtest`、`engine_project_build` 等引擎能力完成游戏开发；中间不得再要求 AI连接 Editor、查询二级 Catalog 或调用一个通用 Host 后才能使用真正能力。

## 2. 产品愿景

### PR-VISION-001：产品本质

AI First Game Engine 必须是：

```text
AI-first game development platform
+ self-developed native game runtime
+ direct AI authoring surface
+ runtime observation and verification
+ desktop/mobile delivery
```

它不是：

- 传统 Editor 加一个聊天面板；
- 远程控制 Editor 的 AI 插件；
- Unity、UE 或 Godot 的简单复制品；
- 只有 MCP 外壳的传统游戏引擎；
- 数百个浅 CRUD 工具组成的对象编辑器；
- 一次 prompt 后不可维护的 prompt-to-game 玩具；
- 只适合打飞机 Demo 的项目模板库；
- 只把 Web 游戏包进壳里的跨平台封装器。

### PR-VISION-002：AI 是首要操作主体

引擎公共 Interface 的首要消费者是 AI Agent，例如：

```text
Codex
OpenCode
DeepSeek Harness based agents
其它具备文件、代码、工具和命令执行能力的 AI Agent
```

人类用户描述目标、提供反馈、判断创意和批准必要风险；AI负责理解工程、编写游戏、调用引擎、运行验证、观察结果和持续修复。

### PR-VISION-003：引擎适配 AI

必须由引擎主动适配 AI 的工作方式，而不是要求 AI 学习并模拟传统 Editor 操作流程。

引擎必须主动提供：

- 稳定、机器可读的项目 Interface；
- AI 可发现的代码和 SDK；
- typed engine tools；
- source-level diagnostics；
- 结构化运行结果；
- 示例、模板、Skill 和版本化知识；
- 可验证、可回滚的修改能力；
- 可以直接运行、测试和交付的命令式入口。

## 3. 目标用户

### PR-USER-001：人类目标用户

继续保留历史目标用户：

- 没有编程基础的游戏创作爱好者；
- 只有少量编程基础的游戏创作爱好者；
- 希望使用 AI 快速制作游戏的独立开发者；
- 需要 AI 参与长期工程维护的专业开发者和小型团队。

零基础用户不需要阅读 Rust、JSON、ECS、RHI 或 RuntimePackage internals。AI可以使用这些技术，但必须把需要用户决定的内容翻译为玩法、视觉、平台、时间、成本和风险语言。

### PR-USER-002：AI 目标用户

引擎必须同时把 AI Agent 视为一种正式“开发者用户”。产品设计必须减少 AI 的：

- 上下文搜索量；
- 训练先验缺口；
- 工具 schema 成本；
- 隐式生命周期；
- 无法识别当前工程阶段和合理下一步的成本；
- 反复猜日志的成本；
- 无法观察 Runtime 状态的成本；
- 为引擎内部合同编写样板的成本。

### PR-USER-003：双层体验

默认人类体验是自然语言和结果检查，默认 AI 体验是代码、项目资产、Engine SDK、工具、测试和 Runtime 证据。

不能为了照顾零基础用户，把 AI 限制在低表达的可视化节点或浅工具中；也不能为了让 AI 直接写代码，要求零基础用户理解代码。

## 4. Codex 原生式使用体验

### PR-EXP-001：默认工作方式必须像 Codex 直接开发

目标体验：

```text
用户描述游戏或修改目标
-> AI 搜索和读取项目
-> AI 编写/修改代码、Scene、Prefab、AUI、Input、Rule 和 Asset
-> AI 调用引擎检查
-> AI 启动真实游戏运行
-> AI读取语义状态、截图和诊断
-> AI继续修改
-> AI构建桌面或移动端产物
-> AI运行交付包复验
```

用户不应感觉自己在操作另一套传统游戏引擎产品。AI不应感觉自己在远程点击 Editor。

### PR-EXP-002：直接不等于无治理

“像 Codex 一样直接”表示低摩擦、代码优先、工具可组合和快速反馈，不表示 AI 可以绕过安全、工程和用户授权。

必须同时满足：

- 只读扫描、诊断、运行和观察可以立即进行；
- 用户可以输入不完整、矛盾、跨天的需求和反馈；
- 不要求先形成完整全局 Feature Spec 才能处理局部工作；
- 不要求用户逐个批准 AI 的内部步骤；
- 真正提交项目修改时，根据风险和影响范围进入必要的 validation、approval 和 transaction；
- 低风险局部修改不能被完整线性 Workflow 阻塞；
- 高风险修改必须有 scope、diff、receipt 和 rollback；
- Engine Core、用户系统和外部发布操作继续受权限限制。

### PR-EXP-003：机械内部复杂度不可见，决策阶段必须可见

AI和用户不需要理解或手工执行以下确定性机械步骤：

- Editor instance discovery；
- EditorSession binding；
- Gateway connection lifecycle；
- generic catalog/execute Host；
- runtime binding 修复菜谱；
- ABI glue；
- Asset cook worker；
- RuntimePackage 内部装配顺序；
- platform exporter 内部阶段；
- GPU worker 或设备 worker 生命周期。

这些可以存在于引擎 Implementation 内，但必须由深 Module 隐藏。

但会改变 AI 下一步判断的工程阶段不能被压成黑盒。项目检查、Runtime运行、输入Playtest、语义/视觉观察、构建和交付复验必须具有明确的阶段状态、结构化结果和失败恢复信息。Skill负责向AI解释这些阶段的适用条件和常见流转；Engine负责执行阶段并返回事实；宿主AI仍负责选择下一步。

### PR-EXP-004：普通代码和项目文件是一等入口

AI必须可以像开发普通软件一样读取、搜索和修改项目代码与可版本控制工程文件。

引擎不能强迫 AI 使用几十个对象级工具替代所有代码和文件编辑，也不能把每一个 Component 字段变成独立工具调用。

对结构化工程对象的修改最终仍需经过统一 schema、影响分析、transaction、validation 和 receipt owner。

### PR-EXP-005：快速观察修复循环

高频开发闭环必须是：

```text
edit
-> check
-> run
-> observe
-> fix
```

普通代码、UI、规则和资源修改不应依赖完整 Windows/Android export 才能看到结果。反馈路径必须以秒级为设计目标，并针对不同修改类型制定独立预算。

## 5. 无 Editor 中间层

### PR-NOEDITOR-001：Editor 不得成为默认路径的一部分

以下完整流程必须在 Editor 未安装、未启动、未连接、未聚焦的环境中成立：

```text
create project
inspect project
change code/assets
validate
run real game
playtest
observe/capture
build desktop/mobile
verify delivery artifact
rollback project change
```

### PR-NOEDITOR-002：AI 身份不得绑定 Editor

AI连接身份、项目 identity、权限、批准、operation、receipt 和 Runtime Job 都不能依赖某个 Editor process 或 Editor window。

禁止默认链路：

```text
AI
-> Editor discovery
-> Gateway
-> EditorSession
-> Catalog
-> generic execute
-> Engine
```

### PR-NOEDITOR-003：真实窗口不等于 Editor

游戏运行需要窗口、GPU、音频、输入或设备时，可以按需启动 Runtime/Player/Device worker。

这些 worker 只负责执行和返回证据，不能重新成为隐含 Editor authority。

### PR-NOEDITOR-004：Editor 只允许是可选产品

历史 Editor 能力可以保留，但只能用于：

- 用户主动打开的可视化检查；
- 人工精修；
- 真实窗口、GPU、输入和设备验证；
- 展示同一 Project truth、receipt 和 evidence。

Editor 不得成为：

- AI工具入口；
- 唯一项目 authority；
- 唯一批准 UI；
- 创建、修改、运行、测试、构建或交付的共同前置；
- AI规划者；
- 维护第二套项目真相的地方。

如果未来保留 Editor，它是 optional projection；如果未来不安装 Editor，产品主流程仍必须完整。

## 6. AI 直接调用引擎

### PR-TOOL-001：Engine capability 直接进入 AI 工具集

Codex、OpenCode、DeepSeek Harness 等宿主应直接看到具体 typed engine tools，而不是只看到一个通用 `Engine Capability Host` 再通过 `catalog/execute` 间接选择能力。

### PR-TOOL-002：工具少而深

“引擎是 AI 的巨大工具库”表示引擎拥有广泛能力，不表示每次暴露数百个浅工具。

默认工具必须对应模型可作出不同决策的工程控制点，而不是对应内部函数或每个实现阶段。v1目标工具族收敛为：

```text
engine_project_inspect
engine_project_check
engine_project_mutate
engine_project_rollback
engine_runtime_run
engine_runtime_playtest
engine_runtime_observe
engine_project_build
engine_delivery_verify
```

一个阶段只有在AI可能据其结果停止、分支、修复或改变策略，并且能够独立调用、具有不同成本/权限/生命周期时，才应成为独立工具。refresh、snapshot、incremental prepare、generated glue、RuntimePackage装配、worker管理和证据落盘等机械步骤必须隐藏在相应工具内部，但其失败阶段和诊断必须返回AI。

普通文件搜索、文件读取、证据文件读取和浅文本符号匹配不得重复注册为Engine tools。项目绑定默认由workspace/cwd或Host配置完成；项目初始化走CLI/模板。只有无法唯一绑定时才返回结构化诊断，不新增日常`open`步骤。Runtime视觉捕获并入`engine_runtime_observe`的typed请求，不单独扩张默认工具数。

### PR-TOOL-003：普通代码能力与 Engine tool 并存

Engine tool 不取代 AI 的 read/search/edit/apply_patch/terminal 能力。AI使用普通代码能力创作游戏，使用 Engine tool 调用只有引擎才能可靠提供的深能力。

### PR-TOOL-004：宿主无关

Engine Core 和稳定 Engine Interface 不能依赖 DeepSeek Cordis、OpenCode plugin type、Codex 专有类型或 MCP transport。

Codex、OpenCode、DeepSeek Harness 和 MCP 只能是 Adapter。

### PR-TOOL-005：Typed tools 与 CLI

引擎应支持两种薄入口：

- Host/MCP投影的typed tools，用于Agent阶段判断和修复；
- CLI，用于安装、doctor、项目初始化以及人类或Agent的显式命令调用。

CLI不得复制项目检查、运行、构建、验证或mutation Implementation；与typed tools重叠的命令必须进入同一Engine owner、权限、validation、operation、receipt和cancellation pipeline。v1不再要求一套独立的Agent Programmatic SDK；Project Game SDK继续只负责项目游戏代码的编程Interface。

### PR-TOOL-006：工具和知识共同交付

每个引擎版本必须一起发布：

- ToolDefinition；
- Project Game SDK 类型和文档；
- Skill；
- examples 和 templates；
- source symbols；
- diagnostic code 和 next action；
- 版本兼容信息。

不能假设 AI天然知道自研引擎。工具 schema 本身不足以补齐 Web、Unity、UE 和 Godot 的训练先验差距。

Skill是模型可见的引擎使用工作流层，必须说明阶段目的、进入条件、可跳过条件、失败恢复和何时调用哪个Engine tool。Skill不得执行引擎能力、持有项目状态、替代ToolResult，也不得把所有任务强制成同一条线性菜谱。

### PR-TOOL-007：与普通 AI 工具使用同一种 Agent Tool Loop

对纯大语言模型而言，工具不是模型内部真正执行的函数。目标调用链必须明确为：

```text
用户需求
-> Agent/Harness 把可用 ToolDefinition 提供给模型
-> 模型输出结构化 ToolCall(toolName, arguments)
-> Agent/Harness 完成权限、参数、取消和执行调度
-> 对应 Tool Provider 执行真实能力
-> 结构化 ToolResult 返回模型上下文
-> 模型根据结果继续判断、调用其它工具或回答用户
```

自研引擎必须直接进入这条既有循环，不能另造一套要求模型学习的新 Agent 协议。

### PR-TOOL-008：Engine Tool 与文件工具是同级一等工具

在模型可见的 Tool Registry 中，下列调用应处于同一级别：

```text
read_file(...)
write_file(...)
apply_patch(...)
run_command(...)

engine_project_inspect(...)
engine_project_check(...)
engine_project_mutate(...)
engine_project_rollback(...)
engine_runtime_run(...)
engine_runtime_playtest(...)
engine_runtime_observe(...)
engine_project_build(...)
engine_delivery_verify(...)
```

AI可以先用普通文件工具编写项目代码，再直接用引擎工具运行、观察和交付；也可以对结构化 Scene、Prefab、AUI、Asset 使用引擎提供的受控 change tool。两类工具由同一个 Agent loop 选择和组合。

### PR-TOOL-009：引擎是 Tool Provider，不是第二个 Agent

产品口径可以说“把整个游戏引擎作为 AI工具提供”，但实现含义必须是：

```text
一个 Engine Tool Provider
-> 注册一组稳定、具体、少而深的 Engine tools
-> 每个 tool 隐藏对应的 Compiler/Runtime/Asset/Build Implementation
```

它不表示：

- 只注册一个巨大 `engine(prompt)` 工具，让内部黑盒 Agent 再理解需求；
- 让 Engine Agent 代替 Codex 决定游戏应该怎么做；
- 通过 `engine.execute(toolId, payload)` 二次选择真实工具；
- 在 Engine Tool Provider 内复制宿主的对话、计划和模型循环；
- 把全部 Engine 内部函数逐个暴露给模型。

AI Agent 继续拥有用户目标理解、任务分解、代码生成、工具选择和结果判断。Engine Tool Provider 只拥有确定的引擎能力、权限内执行、验证、Runtime、证据和 canonical result。

Engine可以拥有确定的工程阶段状态、前置条件、结果和局部合法流转，并在ToolResult中给出带原因的下一步候选；这属于引擎能力合同，不等于第二个Agent。Engine不得根据完整用户目标替AI生成项目计划或自动选择玩法方案。

### PR-TOOL-010：Tool-call 等价性验收

必须通过真实宿主验收证明：

1. 不启动 Editor 时，宿主 Tool Registry 能直接列出具体 Engine tools；
2. 模型可以像选择 `write_file` 一样选择某个 `engine_*` tool；
3. 调用不经过 AI可见的二级 `catalog/execute`；
4. 工具调用继承宿主的 call identity、permission、cancel 和 session log；
5. Engine侧继续执行自己的 schema、Grant、transaction、receipt 和 rollback；
6. ToolResult 以结构化结果返回模型，模型可以继续 observe-fix loop；
7. 同一 Engine Interface 可以由 Codex、OpenCode、DeepSeek Harness 或 MCP compatibility Adapter 投影，而 Engine Core 不依赖具体宿主。

## 7. 游戏创作能力

### PR-AUTHOR-001：AI 原生写游戏

AI必须能够直接完成：

- 游戏代码；
- gameplay 系统；
- Scene 和 Prefab；
- UI/AUI；
- Input；
- 资源导入和替换；
- animation、particle、camera 和 audio；
- physics 和 collision；
- save/load 和配置；
- tests 和 playtest scenarios；
- Build Profile；
- 桌面与移动端交付。

### PR-AUTHOR-002：复杂逻辑与结构化资产并存

继续保留历史正确边界：

```text
Rust Project Framework / Project Rust Module
+ Project Assets
```

- 复杂 gameplay、复杂算法和复杂 UI 工作流使用项目代码；
- Scene、Prefab、AUI、Input、Asset、Build Profile 等保持结构化；
- 可安全数据化的规则可以进入受限 RuleSlot；
- 不发明另一门万能脚本语言；
- 不要求 AI 手写 Engine internals、Rust AOT 派生物或最终 RuntimePackage。

### PR-AUTHOR-003：通用深能力

自研引擎必须长期具备完整游戏引擎能力深度，包括但不限于：

- transform-aware spawn、parent、override 和 lifecycle；
- entity identity、timer、schedule 和 deferred mutation；
- input、physics、collision 和 deterministic fixed step；
- renderer、material、animation、particle、camera 和 post effect；
- audio event、mixing 和 lifecycle；
- UI layout、binding、action 和 present；
- Asset identity、dependency、import、cook 和 lineage；
- Scene、Prefab 和引用完整性；
- Build、export、signing 和 device run。

这些能力必须项目无关，不能把 Player、Enemy、Bullet、Tower、Weapon 等具体玩法写入 Engine Core。

### PR-AUTHOR-004：高表达、低样板

AI不应为每个项目重复编写：

- game loop；
- platform window/input bootstrap；
- renderer bootstrap；
- Asset loader；
- Runtime ABI glue；
- World adapter；
- Runtime binding；
- 重复 descriptor；
- Windows/Android packaging glue。

项目代码应聚焦玩法、视觉和产品差异。

### PR-AUTHOR-005：Feature Locality

一个功能相关的需求、逻辑、规则、UI、资产、测试和证据应具有稳定 scope，使 AI修改一个 Feature 时不必读取整个项目。

Feature 组织不能成为新的 Runtime 层，也不能强迫用户先填写完整需求表单。

## 8. Native Runtime 和平台交付

### PR-RUNTIME-001：正式 Native Runtime

自研 Rust Native Runtime 是长期正式 Runtime。Web 可以作为目标平台或参考基线，但不能成为唯一正式 Runtime，也不能通过 WebView 包装假装已经获得完整原生能力。

### PR-RUNTIME-002：桌面和移动端

产品必须支持：

- Windows 原生运行和打包；
- Android 原生运行和 APK 交付；
- 未来扩展其它桌面和移动平台；
- Web 作为可选目标，而不是架构 authority。

手机端要求包括真实设备输入、生命周期、分辨率、资源和性能验证，不能只证明文件成功生成。

### PR-RUNTIME-003：高级渲染和性能上限

相对纯 Web 路径，自研引擎必须提供更高的长期性能与表现上限：

- 原生 GPU backend；
- 可扩展 2D/3D renderer；
- material、lighting、particle、animation 和 post effect；
- 可预测的 frame time、memory 和 loading；
- 平台资源 cook；
- 大型资源和复杂场景优化能力。

不能只宣称“Native 理论上更快”。必须用相同内容、设备和配置进行真实测量。

### PR-RUNTIME-004：RuntimePackage 和交付一致性

RuntimePackage 继续作为正式运行和交付输入真相。Preview、开发 Runtime、Windows/Android 交付包必须使用同源项目 revision 和可证明的产物 identity。

AI和用户不手写最终 RuntimePackage。

### PR-RUNTIME-005：真正可运行、可打包、可复验

每个平台的“完成”至少要求：

- artifact 生成；
- manifest/hash/identity 正确；
- 真实启动；
- 关键玩法场景可执行；
- 输入、画面和生命周期正确；
- 交付包行为与开发运行一致。

## 9. Runtime 观测和自动验证

### PR-OBS-001：AI 不只看日志和截图

Engine必须向 AI 提供受控机器可读的 Runtime 语义，包括：

- stable entity/object identity；
- transform 和 lifecycle；
- selected component facts；
- input timeline；
- spawn/despawn；
- physics contact；
- gameplay event；
- UI hit target、action 和 binding；
- active AssetRef、animation 和 audio event；
- frame/time marker；
- first failure 和 source-mapped diagnostics。

### PR-OBS-002：视觉证据仍然必需

语义状态不能替代视觉检查。Engine必须提供稳定 viewport、关键帧截图、必要的视频或真实窗口证据，让 AI和用户判断构图、层级、反馈、动效、文字和交互质量。

### PR-OBS-003：因果回溯

失败应尽量支持：

```text
user requirement / Feature
-> project source/asset/rule
-> compiled artifact
-> Runtime entity/event
-> gameplay outcome
-> visual evidence
```

AI必须能从结果回到可修改的项目真相，不能被迫从内部日志猜测首因。

### PR-OBS-004：可重放 Playtest

Engine必须支持真实 Runtime 输入回放和跨帧断言，至少覆盖：

- continuous input；
- spawn 和运动；
- collision 和 damage；
- score/state change；
- UI action；
- pause/resume；
- failure/restart；
- 交付包关键场景。

Playtest contract 必须通用，不能把某种游戏玩法写进 Core。

### PR-OBS-005：观测不能破坏正式 Runtime

Runtime report/trace 必须支持 Off/Summary/Trace 分档和明确预算。正式 Runtime 默认关闭非必要高成本观测，不能每帧生成全量 JSON、字符串或 World snapshot。

## 10. 游戏质量需求

### PR-QUALITY-001：技术成功不等于游戏成功

必须独立报告：

```text
technicalExecution
gameplayAcceptance
visualAcceptance
deliveryAcceptance
```

Build 成功、运行 180 帧、Present 非空或进程 exit 0，都不能单独证明游戏可玩或完成。

### PR-QUALITY-002：玩法质量

AI生成的游戏必须通过真实跨帧玩法验收，而不只是 Rule unit test 或 MockWorld 测试。

玩法必须包括需求规定的完整状态闭环，例如输入、反馈、失败、暂停、恢复和重新开始。

### PR-QUALITY-003：视觉质量

Feature/Art Direction/Asset Spec 应描述画风、参考、目标分辨率和最低完成度。

资源应区分：

```text
placeholder
prototype
release
```

未授权 placeholder 不能被静默报告为 release-ready。审美判断由 AI和用户完成，不能把固定审美分数硬编码进 Engine Core。

### PR-QUALITY-004：AI必须持续迭代

AI不能在第一次 Build 或第一次非空截图后停止。只要玩法、视觉、性能或交付验收未达到需求，AI必须能够继续 observe-fix loop，直到通过、用户接受明确降级或外部条件阻塞。

### PR-QUALITY-005：多类型通用性

质量资格不能只由一个打飞机项目证明。至少要覆盖：

- 纵版打飞机；
- 塔防；
- 平台跳跃或另一种强物理/关卡类型；
- 后续再扩展 3D、联网或其它复杂类型。

## 11. 竞争目标

### PR-COMPETE-001：必须比较的路径

长期统一比较：

```text
A. AI + Web
B. AI + Unity
C. AI + Unreal Engine
D. AI + Godot
E. AI + 当前自研引擎
F. AI + 未来正式自研引擎
```

### PR-COMPETE-002：长期综合目标

在相同需求、资源目标、时间预算、验收场景和目标平台条件下，AI + 正式自研引擎应综合优于 AI + Web、Unity、UE 和 Godot。

“综合优于”至少包含：

- 更短的 Time to First Playable；
- 更低的 Preview Feedback Latency；
- 更多的有效 observe-fix loops；
- 更高的玩法验收通过率；
- 相同或更好的人工视觉评价；
- 更低的项目样板和上下文成本；
- 更高的后续修改一次成功率；
- 更短的真实 Bug 修复时间；
- 更可靠的 Preview/Delivery parity；
- 完整桌面和移动端交付；
- 可竞争的包体、frame time、memory 和 loading；
- 更好的长期维护 Locality、receipt 和 rollback。

### PR-COMPETE-003：首次项目也必须竞争

不能只把优势推迟到第五个项目。长期正式产品在第一次创建项目时，也必须让 AI相对 Web/Unity/UE/Godot 少理解环境、少写样板、更快进入真实运行和观察。

同时必须单独记录：

- first-project cost；
- repeated-project marginal cost；
- later-change cost；
- repair cost；
- delivery and maintenance cost。

### PR-COMPETE-004：不能用单指标伪造胜利

下列情况都不能单独称为“超过”：

- Build 更快但游戏不可玩；
- 包体更小但视觉质量差；
- 画面漂亮但不能稳定打包；
- 第一次 Demo 快但后续无法维护；
- 工具调用少但关键意图无法表达；
- 单元测试全过但真实 Runtime 失败。

### PR-COMPETE-005：诚实资格声明

当前自研引擎施工深度低于成熟引擎是允许的，但任何版本只能声明已经被真实 benchmark 和人工审核证明的优势。

长期目标保持不变，阶段版本不得把“未来目标”写成“当前已实现”。

## 12. 工程安全和长期维护

### PR-ENGINEERING-001：Schema-first

Project、Scene、Prefab、AUI、Input、Asset、Rule、Build Profile 和其它核心对象必须有稳定 schema、版本、identity 和 diagnostics。

### PR-ENGINEERING-002：可审查和可回滚

高影响修改必须支持：

- scope；
- semantic diff；
- affected analysis；
- bounded approval/Grant；
- transaction；
- receipt；
- rollback reference；
- project digest/generation；
- source/artifact lineage。

### PR-ENGINEERING-003：治理不能降低直接体验

安全与审计应集中在深 Module 的 mutation seam，不能复制成 Editor、Gateway、Harness 和每个工具各自一套规则。

只读操作和低风险局部开发不得承受完整发布级审批成本。高风险行为必须 fail-closed。

### PR-ENGINEERING-004：单一工程真相

Headless AI、可选 Editor、Runtime、Build 和 Delivery 必须读取同一项目真相，不允许 Editor memory、Gateway cache 或 AI session 私有状态成为第二套工程 authority。

### PR-ENGINEERING-005：复杂项目可维护

引擎必须长期支持：

- Feature Locality；
- stable AssetRef；
- dependency/impact；
- deterministic tests；
- reproducible RuntimePackage；
- source/runtime/artifact identity；
- 多平台配置；
- 团队协作和版本控制；
- 大型项目的增量编译、资源和运行性能演进。

## 13. 自研边界

### PR-SELF-001：完整自研主线

长期正式主线是完整自研 Engine Interface、Project system、Rust Native Runtime、Renderer、Asset pipeline、AUI、Build/Export 和 Runtime observation。

可以参考或使用第三方基础库，但不能把 Godot、Unity 或 UE 作为正式隐藏 Runtime 后端后仍宣称完整自研引擎。

### PR-SELF-002：不重复实现传统引擎产品形态

自研基础游戏能力是必要投入，但公共产品架构不能退化为：

```text
SceneTree clone
+ Script clone
+ Inspector clone
+ Editor authority
+ AI plugin
```

自研差异必须体现在：

- AI-first Interface；
- Headless-first execution；
- Codex-native direct authoring；
- semantic runtime observation；
- outcome acceptance；
- schema/transaction/receipt/rollback；
- desktop/mobile native delivery；
- AI knowledge distribution。

### PR-SELF-003：项目专用能力不得进入 Core

Engine Core 只提供通用游戏能力。任何验证 Demo 中的 Player、Enemy、Bullet、Score、Tower、Wave、Weapon 等概念必须留在项目侧。

## 14. 端到端权威用户场景

### 14.1 从空白创建游戏

```text
用户：做一个有完整画面、音效、Boss、强化和暂停重开的纵版打飞机游戏，
      可以在 Windows 和 Android 手机上运行。

AI：
1. 在当前目录直接创建结构化游戏项目；
2. 编写玩法代码、Scene、Prefab、AUI、Input 和测试；
3. 生成或导入达到目标质量的资源；
4. 调用引擎检查并运行真实 Native Runtime；
5. 使用输入回放验证移动、射击、生成、碰撞、伤害、得分、暂停和重开；
6. 读取语义状态、截图和首因诊断；
7. 持续修改，直到玩法和画面通过；
8. 构建 Windows 和 Android 产物；
9. 启动交付包并复跑关键场景；
10. 返回产物、验证状态和仍需人工判断的内容。
```

全过程不启动 Editor，不连接 Editor Gateway，不要求用户理解项目内部技术细节。

### 14.2 修改已有游戏

```text
用户：给现有游戏增加一个会分裂的 Boss，并保持手机性能。

AI：
1. 定位 Boss Feature scope 和相关资源、逻辑、UI、测试；
2. 读取现有行为和性能证据；
3. 修改项目代码和结构化资产；
4. 运行 affected check 和 gameplay replay；
5. 捕获 Boss 阶段画面与 frame/memory 结果；
6. 修复回归；
7. 构建并验证桌面和手机交付包。
```

### 14.3 修复真实 Bug

```text
用户：手机上偶尔出现子弹从屏幕左上角生成。

AI：
1. 不经 Editor 读取项目和历史证据；
2. 使用 replay/trace 复现；
3. 从错误 bullet entity 回溯 spawn request、source entity 和项目源码；
4. 修改通用项目逻辑而不是增加设备专用补丁；
5. 重放同一输入和随机种子；
6. 验证 Windows/Android 行为一致；
7. 提供修复 receipt 和回滚引用。
```

## 15. 产品验收不变量

以下是不允许被架构折中的产品不变量。

### INV-001：No-Editor Full Path

Editor 二进制不存在时，AI仍能从空项目完成创建、修改、运行、玩法验证、视觉捕获、Windows/Android 构建和交付复验。

### INV-002：Codex-Native Authoring

AI可以使用普通代码、文件和命令式 Engine tools 完成游戏，不需要模拟 Editor UI，不需要逐对象 CRUD，不需要学习内部进程拓扑。

### INV-003：Real Runtime Feedback

AI的主要调试反馈来自与交付语义一致的真实 Runtime，而不是 MockWorld、静态 schema 或仅 Build 报告。

### INV-004：Outcome Before Delivery Claim

只有 technical、gameplay、visual 和 delivery 验收满足需求，或用户明确接受降级，才能声明游戏完成。

### INV-005：Desktop and Mobile Delivery

Windows 和 Android 是必须形成真实运行与交付证据的目标平台，不允许只生成项目文件或 Web 页面。

### INV-006：AI Interface Is First-class

新增 Engine capability 必须同时考虑 AI 可发现性、SDK/Tool projection、diagnostics、observation 和 tests，不能先只完成 Editor 按钮再把 AI 接入留到以后。

### INV-007：No Godot Clone

如果某项设计只是增加传统 Scene/Inspector/Editor 工作流，而没有提升 AI 的表达、反馈、观测或验证 Leverage，它不能以“AI-first”名义进入默认产品主线。

### INV-008：Benchmark-qualified Advantage

“比 Web/Unity/UE/Godot 更快更好”必须由统一 benchmark 和人工审核支持，不能依靠主观架构推断。

### INV-009：First-class Engine Tool Call

AI必须通过与 `read_file/write_file/run_command` 相同的 Agent Tool Loop 直接选择和调用具体 Engine tools。任何要求 AI先连接 Editor、调用通用 Engine Host、查询二级 Catalog 或把需求交给内部黑盒 Engine Agent 的默认实现，都违反本需求。

## 16. 非目标

当前权威需求不要求：

- 当前版本立即在所有功能深度上超过 Unity、UE 和 Godot；
- 用一种 IR 或 DSL 表达所有游戏逻辑；
- 取消所有可视化工具；
- 删除已经完成的 Editor 产品代码；
- 把所有 Engine capability 暴露成单独工具；
- 在 Engine Core 中实现自动游戏设计 Agent；
- 用一条不可跳过的固定线性工作流替代 Codex 自主规划；允许Skill描述条件化引擎使用工作流，允许Engine返回局部阶段和合法流转；
- 让用户阅读内部 JSON、Rust、ECS、RHI 或构建日志；
- 用 AI 主观判断替代引擎测试和真实运行；
- 只针对打飞机或塔防优化；
- 为了宣称跨平台而使用行为不一致的空壳导出。

“当前版本不能立即超过成熟引擎”不降低长期竞争目标，只要求阶段声明诚实、施工按可验证的纵向链路推进。

## 17. 与历史需求的继承关系

### 17.1 继续保留

- AI原生游戏生产引擎定位；
- 零基础和少量编程基础用户可使用；
- 自然语言表达目标；
- AI生成和修改资源、逻辑、场景、UI和项目代码；
- schema-first；
- validation、diff、receipt、rollback；
- Rust Native Runtime；
- RuntimePackage；
- Project Assets 和 AssetRef；
- Rust Project Framework + 受限 RuleSlot；
- Windows/Android 原生交付；
- Editor可用于可视化验证和人工精修；
- AI自主规划；Skill公开条件化引擎使用工作流，Engine拥有确定工程阶段但不拥有目标级Agent Workflow；
- 复杂项目长期可维护。

### 17.2 最新需求明确修正

| 历史表述 | 最新权威需求 |
| --- | --- |
| 默认体验包含可视化 Editor | 默认完整路径不安装、不启动、不连接 Editor |
| Editor/Gateway 是 AI 工具入口 | AI直接使用 Harness-native/host-native Engine tools |
| EditorSession 是项目 authority | 中立 Headless Project Context 是默认 authority，Editor只作 Adapter |
| 所有需求先走完整 Feature Spec 线性流程 | 用户表达自由，局部工作不被全局规格阻塞，提交变更时按风险治理 |
| AI主要通过结构化 Patch 工具修改 | 普通代码/文件是一等入口，结构化对象仍进入统一事务 owner |
| “工具越多越 AI-first” | 只把模型决策控制点做成工具；机械步骤进入Implementation，阶段结果仍对AI可见 |
| Build/Present 成功可代表主要进展 | technical/gameplay/visual/delivery 必须分开 |
| 打飞机能打包是核心终点 | 多类型游戏、长期修改、Bug 修复和桌面/移动交付共同验收 |

### 17.3 顶层架构同步状态

2026-08-31 已生成 `00-AI-First-Game-Engine-权威架构设计-v1.md`，并完成顶层 authority 切换：

- 旧 `01-目标与核心原则.md` 和 `02-AI功能生成流程.md` 已移入 `历史文档/`；
- `00-文档地图.md`、`00-阅读顺序.md`、README 和当前状态入口改为引用本文与新权威架构；
- `20`、`252`、`254`、`256`、`259` 不再拥有默认 Editor/Gateway authority；
- `253`、`255`、RuntimePackage、Native Runtime、ProjectRuntimeAbi/SDK 和现有通用能力按新架构继承矩阵继续有效；
- Harness v0.3 继续作为研究输入，不成为施工 authority。

代码迁移仍需逐个正式子设计、方案自审、施工文档和激活，不因本次文档切换自动发生。

## 18. 权威需求验收指标

正式方案阶段必须为下列指标给出测量方法、基线和目标值：

| ID | 指标 | 必须比较 |
| --- | --- | --- |
| M-01 | Time to First Playable | Web/Unity/UE/Godot/当前/目标 |
| M-02 | Preview Feedback Latency | 按代码、Scene、AUI、Asset 分类 |
| M-03 | Observe-Fix Loops per Hour | 只计算形成真实修复证据的循环 |
| M-04 | Authoring Compression | 玩法代码相对 Glue/descriptor/ABI 样板 |
| M-05 | Context and Token Cost | 首次理解与局部修改分别统计 |
| M-06 | Gameplay Acceptance Pass Rate | 真实 Runtime 跨帧场景 |
| M-07 | Human Visual Review | 相同需求、资源目标和视口 |
| M-08 | Runtime/Delivery Parity | Preview、RuntimePackage、Windows/Android |
| M-09 | Change Success Rate | 后续功能修改一次成功率 |
| M-10 | Repair Time | 真实逻辑、资源、UI、平台 Bug |
| M-11 | Package and Performance | 包体、frame time、memory、loading |
| M-12 | Maintainability | Feature scope、affected、回归和长期修改 |

任何“更快更好”的正式结论必须同时说明：

- 使用的 Engine/AI/模型版本；
- 相同或不同的资源条件；
- 目标平台和设备；
- 时间预算；
- 自动验收结果；
- 人工审核结果；
- 未覆盖的能力；
- 是否首次项目或复用项目。

## 19. 用户审核清单

请用户重点审核以下六项是否准确：

1. 默认完整游戏生产流程完全不经过 Editor；Editor最多只是用户主动打开的可选验证工具。
2. AI像使用 Web/文件/命令行一样直接使用自研引擎，主要创作方式仍是 AI原生写代码和工程内容。
3. 自研引擎解决纯 Web 路径的原生打包、手机端、高级渲染、性能、资源和交付问题。
4. 长期目标是综合超过 AI + Web、Unity、UE 和 Godot，而且首次项目也必须具备竞争力。
5. 安全、schema、validation、receipt 和 rollback 继续保留，但必须隐藏在深 Module 和风险 seam 内，不能把体验重新变成传统 Editor 工作流。
6. 游戏引擎以一等 Tool Provider 进入 AI现有 Agent Tool Loop，具体 Engine tools 与写文件、执行命令工具同级；Engine Provider 不成为第二个 Agent，也不要求二次 `catalog/execute`。
7. Skill公开条件化引擎工作流，Engine tools暴露决策控制点，机械内部阶段不扩张成浅工具但必须返回阶段化诊断。

用户确认后，本文才进入“已确认权威产品需求”状态，并成为后续正式架构重审的最高产品依据。
