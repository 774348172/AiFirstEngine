# 247-人工从空项目创建复杂打飞机并导出 Windows 可玩-系统讨论优先级

> 状态：正式讨论优先级文档；不是方案文档，不是施工文档，不授予施工权限。
> 建立日期：2026-07-12。
> 产品目标：没有编程基础和只有少量编程基础的游戏创作爱好者，只通过引擎可见界面和引擎内 AI，从空项目创建复杂打飞机，在编辑器内真实游玩，并导出可脱离编辑器运行的 Windows 游戏。
> 当前隔离：245、246、248-A/B/C、249 均已完成；P0-0.5 v3 已冻结并仅启动 Run 01，Run 02-09 保持 reservation-only idle。v3 Run 01 形成并验证唯一 XOR 终态后再决定后续；Run 01 关闭前不恢复本文 P0-2 队列。249 审查发现的 AUI 7 项作为独立整改输入跟踪，不混入 v3 测量；本文自身仍不授予施工权限。

## 1. 这份文档解决什么问题

当前引擎已经具备大量独立能力：项目创建、Scene、Prefab、Rule、Input、AUI、Editor Play、Windows Export、真实 LLM Patch 等。但“功能存在”不等于普通用户能够连续完成一个项目。

本文中的“真实用户”原按现已历史化的 `01-目标与核心原则.md` 定义。当前产品和架构目标改以 `00-AI-First-Game-Engine-权威产品需求-v1.md` 和 `00-AI-First-Game-Engine-权威架构设计-v1.md` 为准；本文的 Editor-only 用户路径只作历史实验约束，不再定义默认产品入口。

本文把后续讨论目标从“继续增加单点能力”切换为：

```text
真实用户
  -> 打开真实原生编辑器
  -> 从空项目开始
  -> 只使用可见 UI 和正式命令
  -> 创建复杂打飞机全部项目内容
  -> Editor Play 真实游玩
  -> 保存、关闭、重开
  -> 导出 Windows
  -> 在项目目录外启动 Game.exe
  -> 真实窗口、真实输入、真实贴图、真实 UI、真实玩法通过
```

本文只决定“先讨论哪些系统”。每个系统仍必须单独完成：

```text
规则加载与当前证据核对
成熟引擎源码研究
2-3 个方案比较
用户确认
正式方案
方案审查
施工文档生成与自审
进入待执行或当前施工队列
```

## 2. 最终验收边界

### 2.1 起点

验收只能提供：

```text
一个新的空项目目录。
引擎内置通用 Component、标准 Module 和通用空项目骨架。
用户自行准备的原始图片、字体、音频等源资源。
```

禁止预置：

```text
复杂打飞机 Scene。
Player / Enemy / Bullet / Weapon / Wave 专用 Prefab。
打飞机 Rule、AUI、InputMapping。
预先写好的 ComplexShooter Rust RuntimeModule。
手工生成好的 RuntimePackage 或 Windows Build。
直接复制 samples/complex_shooter_project 作为创建结果。
```

### 2.2 允许的用户操作

```text
通过引擎 Project Launcher 创建和打开项目。
通过系统文件选择器或 Asset Browser 导入源资源。
通过 Scene、Hierarchy、Inspector、Prefab、Rule、Input、AUI、Build 等可见 UI 操作。
通过引擎内 AI 提出需求、审阅修改计划、确认或撤销修改。
通过引擎内正式保存、验证、Play、Build、Report 命令。
```

最终验收禁止依赖：

```text
手工编辑项目 JSON。
手工修改 RuntimePackage、manifest 或导出文件。
在外部 IDE 预先编写打飞机项目逻辑后冒充引擎内创作。
要求用户阅读或修复 Rust 编译器错误才能完成主流程。
测试代码直接调用内部 service 绕过真实 UI。
headless present 冒充真实玩家窗口。
```

自动化测试可以复用真实 UI command/hit route 做确定性 replay，但最终 Gate 必须包含一次真实原生窗口人工/自动交互证据。

### 2.3 必须完成的游戏内容

最低复杂打飞机验收包括：

```text
项目与主 Scene。
玩家、至少两类敌人、子弹、可收集物或强化物。
Transform、Sprite、Collision、Health 等数据。
Prefab 创建、实例化和至少一项 override。
玩家移动、射击、敌人移动/生成、碰撞、伤害、死亡、计分、波次。
Input Mapping。
HUD：生命、分数、波次。
暂停菜单和游戏结束/重新开始界面。
至少一种真实贴图；不能全部使用占位色块。
保存、关闭、重新打开后内容和行为一致。
```

### 2.4 Windows 最终结果

```text
导出目录可以复制到项目源目录之外。
Game.exe 不读取编辑器内存或项目源文件。
真实 Windows 窗口成功创建并持续运行。
键盘/鼠标输入可以移动、射击、暂停和确认菜单。
贴图、文字、HUD 和游戏 UI 可见。
射击、碰撞、伤害、死亡、计分和波次行为真实发生。
至少一张真实窗口 screenshot 和一段结构化 input/gameplay trace。
进程正常退出，无未处理 panic。
```

第一阶段不要求 Installer、代码签名、MSIX 或 Store 发布；“可导出 Windows”定义为独立、便携、可运行的文件夹包。

## 3. 当前能力判断

| 能力 | 当前状态 | 说明 |
|---|---|---|
| 原生编辑器启动与项目打开 | 已具备 | 已有 Native Editor、Project Launcher、Workspace |
| Scene/Prefab/Rule/Input/AUI 单域能力 | 大部分已具备 | 已有方案和施工，但缺完整真人连续操作复验 |
| Editor Play | 已具备主要链路 | RuntimePackage Preview、同进程 GameView、输入和 GPU 共享已完成 |
| Windows 文件夹包导出 | 已具备 | 已生成并启动过复杂打飞机 `Game.exe` |
| 真实窗口玩家验收 | 证据不足 | 当前黄金报告主要是 headless，真实窗口/screenshot 仍偏 optional |
| 人工从空项目创建复杂玩法逻辑 | 未证明 | 当前样例依赖预先存在的 Rust Project RuntimeModule |
| 完整人工 Walkthrough | partial | 191 曾输出 MissingCommand、FocusDomainPanel、NeedsContext |
| AI 辅助从空项目创建完整游戏 | 未证明 | 必须复用与可见 UI 相同的 Command/Validator/Build，不得绕过项目真相 |

## 4. 优先级总表

| 优先级 | 系统 | 讨论状态 | 核心目标 |
|---|---|---|---|
| P0-1 | Native Editor UI Interaction Reachability / Layout Convergence v1 | 已完成（248-A/B/C 已归档） | 修复主流程 UI 不能点击、点错、遮挡、缺滚动和状态不清 |
| 临时 blocker | Native Editor Deterministic Project Launcher State Isolation v1 | 已完成（249 已归档） | 已关闭 P0-0.5 Run 01 暴露的隐式 Windows 历史目录与 recent-store 污染；不改变 P0 编号 |
| P0-2 | AI-Primary ProjectProduction / Human From-Blank Golden Walkthrough v1 | 讨论已完成，分阶段施工中（250-A / 250-B / 250-C / 250-D / 250-E 已完成；250-F Phase 6 已激活） | 正在以 C-01 从真实空项目验证候选、Preview、重开和 external Windows Export 全链路 |
| P0-3 | Real Manual Authoring / Command Context Convergence v2 | 待讨论 | 收敛 Path、Selection、Inspector、当前文档等真实命令上下文 |
| P0-4 | AUI Scene Authoring UX / Game UI Creation Convergence v2 | 待讨论 | 让用户在 Scene 中直观创建和编辑项目 HUD、菜单与交互 UI |
| P0-5 | Project Gameplay Logic Human Authoring Productization v1 | 待讨论 | 让用户在引擎内创建复杂玩法，不依赖预写 Rust 打飞机 Module |
| P0-6 | Walkthrough-driven Missing Domain Operations Closure | 动态生成 | 按 Walkthrough 证据逐个收敛真实缺失 Domain，不做巨型合集 |
| P0-7 | Real Editor Play Human Acceptance Gate v1 | 待讨论 | 使用刚刚人工创建的项目完成真实窗口与输入游玩验收 |
| P0-8 | Portable Windows Real-Window Playable Golden Gate v2 | 待讨论 | 在独立目录运行真实 Game.exe，验证窗口、输入、渲染、UI、玩法 |
| P1-1 | Human-facing Diagnostics / Recovery Convergence v1 | 待讨论 | 让普通用户能理解并修复资源、引用、编译、构建和运行错误 |
| P1-2 | Clean Project Save / Reopen / Rebuild Gate v2 | 待讨论 | 验证关闭、重开、重建后项目结构和行为仍一致 |

## 5. P0-1 Native Editor UI Interaction Reachability / Layout Convergence v1

### 5.1 为什么第一

如果编辑器控件本身不能稳定点击，Walkthrough 无法区分：

```text
功能不存在；
HitRegion 缺失或错位；
Command 没有路由；
Context 缺失；
控件被遮挡或滚动不到；
操作失败但没有反馈。
```

因此先治理主创作路径的功能性 UI，再运行完整人工 Walkthrough。

### 5.2 讨论范围

```text
Project Launcher
Workspace / Workflow Rail
Hierarchy / Scene / Inspector
Asset Browser
Prefab / Rule / Input / AUI Authoring
AI Panel（只检查 Editor UI 可用性，不要求 AI 创建游戏）
GameView / Play Controls
Build / Export
Report Panel
```

### 5.3 必须验证

```text
可见的可操作控件必须有正确 HitRegion 和 Command。
不能操作时必须显示 Disabled/Busy/Failed 状态和原因。
视觉位置与点击区域一致，不存在透明或偏移点击区。
1280x720、1600x900、1920x1080 下无关键控件遮挡。
Windows 100%、150%、200% DPI 下文本和点击区域保持一致。
Dock、Resize、Scroll、Tab、Focus、Modal、Menu 工作正常。
Hierarchy/Scene/Inspector Selection 一致。
图标按钮有 Tooltip；文本不会溢出父容器。
真实窗口点击 replay 和像素 screenshot 都通过。
```

本系统不是视觉换肤，不以颜色、圆角或动画为主要目标；只治理会阻断项目创作的 UI 可达性、布局和反馈。

## 6. P0-2 Human From-Blank-Project Golden Walkthrough v1

建立唯一版本化人工场景：

```text
Create Project
Import Assets
Create Scene / Entities / Components
Create Prefabs / Instances / Overrides
Create Input
Create Gameplay Logic
Create AUI HUD / Menus
Save / Close / Reopen
Editor Play
Build Windows
Run exported Game.exe
```

这里的 `Human` 表示用户始终能看见、审阅、拒绝、撤销和接管流程，不表示关闭 AI。AI 可以在每一步生成建议或通过正式 Command 执行已确认修改，但不能使用内部 service、手写 RuntimePackage 或隐藏项目代码绕过用户可见链路。

每一步至少记录：

```text
visible control / hit region
command id
required context
before/after project revision
created/modified assets
validation status
diagnostics / next action
screenshot or structural evidence
```

它既是用户验收场景，也是后续系统优先级的证据来源；不能只生成“命令存在”的覆盖报告。

## 7. P0-3 Real Manual Authoring / Command Context Convergence v2

继承 191，不新增第二套 Command Framework 或 Walkthrough 真相层。

重点收敛：

```text
当前 project/document/scene/prefab/AUI/rule path。
Hierarchy/Scene/Inspector selection。
AssetRef 和目标目录。
新增对象的 parent / insertion point。
File Picker / Dialog result。
需要上下文命令从 FocusOnly 推进到用户可完成操作。
错误上下文给出可执行 next action，不伪造 payload。
```

## 8. P0-4 AUI Scene Authoring UX / Game UI Creation Convergence v2

这是项目游戏 UI 的编辑体验，不是编辑器自身 UI。

必须让用户在统一 Scene 工作区中：

```text
创建 Text/Image/Button/Panel/Scroll/Modal 等 AUI Node。
选择、框选、移动、缩放、锚定和对齐。
拖动改变 AUI 层级和 sibling 顺序。
编辑文字、图片、样式、Binding 和 Action。
观察 UI 与 Scene stage 的前后组合关系。
预览点击、拖拽、滚动、焦点、键盘/手柄导航。
创建 HUD、暂停菜单和游戏结束界面。
```

禁止退回独立 AUI Designer 或手写 AUI JSON 作为默认工作流。

## 9. P0-5 Project Gameplay Logic Human Authoring Productization v1

这是当前最大的不确定点。现有复杂打飞机可以运行，但核心项目逻辑已经预写在 Rust Project RuntimeModule 中。

本系统必须比较并确认一种零基础或少量编程基础用户可完成的正式路径：

```text
Rule Graph / Contract-bound RuleSlot + Standard Modules；或
引擎内 Project Rust Framework 编辑、生成、编译和诊断；或
两者明确分工且用户不需要离开引擎完成主流程。
```

如果路径需要 Project Rust Module，默认由引擎内 AI 生成、修改、编译和解释诊断；用户可以审阅功能级 diff 和结果，但不能被要求理解 Rust 语法或修复编译器错误。

最终至少支持用户创建：

```text
移动、射击、生成、碰撞、伤害、死亡、计分、波次、暂停和重新开始。
```

不能通过给用户预置完整打飞机 RuntimeModule 绕过本系统。

## 10. P0-6 Walkthrough-driven Missing Domain Operations Closure

P0-2/P0-3/P0-4/P0-5 产生的结构化 gap 按 domain 分成独立小系统：

```text
Scene
Asset
Prefab
Rule / Project Logic
Input
AUI
Play
Build / Report
```

规则：

```text
只处理 blocks_walkthrough=true 的真实缺口。
一个施工文档只收敛一个内聚 domain 或一条完整操作链。
不建立“所有缺失操作一次修完”的巨型方案。
完成后重跑同一 Golden Walkthrough，不能另造更弱测试。
```

## 11. P0-7 Real Editor Play Human Acceptance Gate v1

使用 P0-2 中刚刚从空项目创建的产物，不使用预置样例替换：

```text
Play / Pause / Step / Stop。
真实 GameView 画面和 AUI。
真实键盘/鼠标输入。
玩家移动与射击。
敌人、碰撞、伤害、死亡、计分和波次。
停止 Play 后 Authoring 状态符合既有 Apply/Discard 合同。
```

## 12. P0-8 Portable Windows Real-Window Playable Golden Gate v2

必须从 P0-2 人工创建项目导出：

```text
Build/Windows/<profile>/Game.exe
portable RuntimePackage and manifests
real process
real OS window
real keyboard/mouse input
real screenshot
gameplay trace
clean exit
```

复制导出目录到项目源目录之外后再次运行。默认验收不能只使用 `headless_surface`、`screenshotRequested=false` 或确定性输入快照。

## 13. P1 系统

### 13.1 Human-facing Diagnostics / Recovery Convergence v1

统一错误最少说明：

```text
哪里失败。
为什么失败。
影响哪个项目对象。
用户下一步在什么面板做什么。
修复后如何重新验证。
```

重点覆盖缺失资源、悬空 AssetRef、非法 Component 字段、Rule 编译、RuntimeModule 构建、AUI Binding、Windows Export 和 Player 启动。

### 13.2 Clean Project Save / Reopen / Rebuild Gate v2

在人工完整创建后执行：

```text
Save All
关闭编辑器
重新启动并打开项目
验证 Scene/Prefab/Rule/Input/AUI/Project Logic
Editor Play
Clean Rebuild
再次导出和运行
```

## 14. 讨论选择规则

默认顺序：

```text
P0-1 -> P0-2 -> P0-3 -> P0-4 -> P0-5
  -> P0-6 动态缺口
  -> P0-7 -> P0-8
  -> P1-1 -> P1-2
```

允许调整顺序的证据只有：

```text
前置条件尚未满足。
Golden Walkthrough 证明另一个缺口更早阻断。
新测试证明当前系统已由既有实现完整覆盖。
用户明确改变产品目标。
```

不能因为某个系统施工复杂就跳过；也不能因为已有类型、测试或方案名称就判断用户流程已经可用。

每次只讨论一个系统。用户确认前只给 2-3 个方案，不生成正式方案文档；用户确认后，正式系统方案从 248 起顺序编号。

## 15. 与 240、245、246 的关系

```text
240：5.6 审查收敛队列真相。
245：当前唯一施工，不受本文影响。
246：第一待执行施工，不受本文影响。
247：新的人工产品落地讨论优先级，不是施工入口。
248+：本文各系统未来的正式方案编号。
```

本文允许在 245/246 等待期间继续方案讨论，但不允许未来系统跳过 246 取得施工授权。未来施工文档必须按项目规则进入 `待执行/`；除非用户显式重新排序，否则排在 246 之后。

本轮建立 247 时不修改 240、49、54、Skill、`施工文档/当前/` 或 `施工文档/待执行/`。

## 16. AI 与人工验收关系

AI 是目标产品的默认生产参与者，不等待全部人工编辑能力完成后才进入产品路线。但 AI 不能被当作绕过 UI、Command、Context 或 Validator 缺口的捷径。

正式关系：

```text
Human-visible From-Blank Walkthrough
  -> 证明用户能理解、审阅、撤销、接管和恢复

AI-assisted From-Blank Walkthrough
  -> 复用同一 Project Command / Context / Validator / Transaction
  -> 提高生成和修改效率
```

两条验收共享 Project Assets、Patch History、Play、Build 和 Report，不建立 AI 专用项目真相，不直接手写 RuntimePackage，也不要求先完成一套接近 Unity 全量能力的人工编辑器后才开始 AI Gate。

## 17. 整体完成标准

只有以下证据全部成立，247 目标才算完成：

```text
真实空项目起步。
所有关键创作步骤通过真实原生编辑器 UI 或引擎内 AI 的正式可审阅 Command 完成，并可由用户接管或撤销。
没有手工 JSON、预置打飞机 RuntimeModule 或样例复制。
复杂玩法、AUI、输入、Prefab 和资源均由用户建立。
Save/Reopen/Rebuild 一致。
Editor Play 真实可玩。
Windows 导出目录可移出项目树。
Game.exe 真实窗口可见且接受真实输入。
贴图、文字、HUD、玩法和退出均有 screenshot/trace/report 证据。
```

在此之前，项目只能声明“具备部分创作和导出能力”，不能声明“普通用户已经可以从空项目完整制作并发布复杂打飞机”。
