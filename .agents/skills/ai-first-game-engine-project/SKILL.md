---
name: ai-first-game-engine-project
description: Project-level rules for the AI First Game Engine repository. Use before discussing architecture, implementation plans, construction docs, code changes, reviews, AUI, RuntimePackage, Projection, Rust runtime/editor, complex shooter validation, build/export, resources, ECS, scene/prefab, or AI patch workflow in G:\gameEngin.
---

# AI First Game Engine Project Rules

每次在 `G:\gameEngin` 讨论架构、方案、施工、审查或改代码前，先加载本 skill，并按下面规则判断。

## 入口顺序

先读当前入口，不凭旧窗口记忆开工：

```text
README.md
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
```

讨论具体系统前，用 `rg -n "系统名|关键概念" 框架设计/引擎总体架构` 查已有方案。已有当前有效方案时，不从零重开，只做缺口分析、方案收敛或施工文档。

## 当前权威与下一步路由

产品与架构边界以以下文档为准；下层方案、历史队列和本 Skill 不得反向覆盖：

```text
框架设计/引擎总体架构/00-AI-First-Game-Engine-权威产品需求-v1.1.md
框架设计/引擎总体架构/00-AI-First-Game-Engine-权威架构设计-v1.1.md
```

询问下一步或讨论新系统时加载 `game-engine-review-priority`，结合架构第 17 节、49 当前基线及已确认子方案选择工作。227 和 240 的旧队列已关闭，只用于追溯或定位可复现旧回归，不再自动选择新主线施工项。

49 保存唯一当前产品/安装基线，54 保存唯一有效施工入口；其它导航引用它们。更新时修正当前块，历史记录保持历史身份，不靠继续追加“最新”覆盖互相冲突的当前状态。

## 主线状态

- 用户于2026-10-19指定游戏开发固定消费V0.0.5，引擎继续独立开发。游戏任务先读仓库根 `GAME_DEVELOPMENT_BASELINE.md`；SDK与Provider必须来自其中的冻结目录，不自动跟随开发源码或沿用旧安装冒称V0.0.5。显式升级后才迁移游戏。

- 正式主线是 No-Editor 的 AI Agent + 九项独立 typed Engine tools + Project Game SDK/Compiler + Rust Runtime；Native Editor 是可选消费端。
- 各工具独立调用；准备和缓存留在引擎内部。Skill 组合方式是可跳过的轻量建议，不强制固定调用顺序。MCP 兼容投影保留同级工具，不折叠成统一大工具。
- `legacy/typescript-prototype/` 只作历史参考，不是正式 runtime/editor/build 入口。
- 当前是否能施工，只以 `54-当前可自动化施工总入口.md` 和 `施工文档/当前/` 为准；`施工文档/待执行/` 只表示已生成、已自审但未激活。
- 有空闲施工槽时，先检查 `待执行/`；按优先级和依赖对候选文档做激活前复核并移动到 `当前/` 后才能实施。
- 源码参考和其它 AI 审查只作参考；采纳结论必须写回正式架构文档或施工文档。

## AI 工作流

- 普通项目文件和真实项目代码是一等输入，可按任务直接读写。Context Scan、Feature Spec、Patch Plan 和验证按风险选择；不把完整方案/施工流程强加给每次项目小改。框架设计变动仍走正式方案确认。
- 所有核心工程对象 schema-first，patch/report 必须可审查、可回滚、可解释。
- 激活、开始、恢复、修复、验证、冻结或完成任何施工 Gate 时，同时加载 `game-engine-construction` skill。
- 项目允许最多三路并行施工（2026-10-26用户更新），每路只执行自己在 `54` 登记的当前施工范围；`待执行/` 中的文档禁止施工。每阶段跑对应测试；完成后写阶段完成记录并归档施工文档。
- 如果实现推翻设计，先更新设计文档，再继续写代码。

## 施工验证原则

公共项目规则只定义结果，不规定固定测试菜谱。具体施工验证由
`game-engine-construction` skill 作为唯一真相，并由当前施工文档按修改范围、风险和成本选择。

- 每个可执行需求必须有能捕获该需求失败的测试，或有明确、可审查的豁免。
- 开发反馈应尽量快速、确定、贴近 owner；不得把完整 workspace 或真实生产环境当调试循环。
- 共享 interface、跨 crate consumer、外部状态或发布产物发生变化时，验证范围必须覆盖实际影响面。
- 同一配置下，上层 suite 已覆盖的定向测试不得机械重复；只有环境、feature、profile 或真实 composition
  不同时才形成独立证据。
- 失败必须保留足以定位首因的输出；不得用只含测试名或退出码的摘要要求 AI 猜修复。
- clean exact commit、binary seal、真实配置、全平台或全 feature 验证只用于对应的集成、发布或激活风险，
  不自动施加给普通源码施工。
- 测试代码、产品源码、发布二进制和外部配置分别计算身份与失效范围；无关变化不得使全部既有证据失效。
- 高成本验证采用 fail-fast，并在开始前确认能完成证据写入和必要 cleanup；并行任务不得共享竞争状态。
- 不因单次事故持续追加永久公共步骤。重复事故优先修复 owner、Runner 或证据接口；新增公共规则必须替代或
  删除旧规则，使总复杂度不增加。

## 方案到施工闭环

每个正式系统从讨论到施工，按此闭环执行：

0. 讨论新系统时，先只输出方案草案和推荐结论，等待用户明确确认选用哪个方案；用户确认前不得新建或更新正式方案文档，也不得生成施工文档。
1. 用户确认方案后，才生成或更新正式方案文档；如果有其它 AI 审查文档，必须先读取审查结论。
2. 审查指出需要修改方案时，先修改正式方案文档，再生成施工文档；审查没有要求修改时，也要把“无需修改”的判断写入施工文档自审。
3. 施工文档必须放在 `施工文档/当前/` 或 `施工文档/待执行/`，并包含施工目标、涉及文件、分 Gate 任务、每 Gate 测试命令、文档同步和归档规则。有空闲槽且授权、依赖和并行边界满足时可激活；三槽已满或条件不满足时进入 `待执行/`。
4. 生成施工文档后先做施工文档自审，确认它没有扩大范围、没有跳过测试、没有和正式方案冲突。进入 `待执行/` 的文档必须明确写明“不可施工”和前置条件。
5. `待执行/` 只是文档队列，不是施工授权。有空闲槽时，按优先级选择一份待执行文档，重新核对方案、代码基线、涉及文件、测试命令、依赖及另一条施工路的共享修改边界；复核通过后将它移动到 `当前/`，并同步 `54` 的对应槽，该移动才叫激活。已有授权足够时不重复请求确认。
6. 正式施工时严格按本路登记的当前文档和范围执行；一边施工一边跑 Gate 测试，失败先修本路当前 Gate，不跨 Gate 堆积问题。
7. 完成声明必须有与实际影响面匹配的验证。复用身份未变的有效证据；不因文档同步或安装接入重新运行已完成的全量回归。
8. 完成后生成阶段完成记录，更新 `49` / `54` / `施工文档/README.md` / `阶段完成记录/README.md` 等入口摘要，并把施工文档从 `当前/` 移到 `已完成/`。
9. 如果施工中发现方案错误，暂停扩大实现，先回填正式方案或施工文档，再继续。

## 待执行施工文档

- `待执行/` 可以在另一份施工文档占用 `当前/` 时保存已确认、已自审的后续计划，但不得运行代码修改、Gate、测试、归档或完成状态更新。
- 待执行文档可以有多份；排列依据是正式优先级文档和前置条件，不以创建时间自动开工。
- 激活前复核必须发现方案或代码基线漂移；需要修改时先更新施工文档并重新自审，禁止原样执行过期计划。
- `当前/` 最多三份非 README 施工文档；同一总施工也可由两路分担明确且互不重复的范围，由 `54` 分别登记。不得把“待执行”写成“施工中”或“当前可施工”。
- 阻塞中的当前施工仍然占用本路槽位，除非正式归档、移动到 `历史/`，或用户明确调整分工。

## 三路并行施工

- 2026-10-26用户明确指定主开发槽优先：371 S5本轮与第二施工槽发生文件、共享接入、构建或测量资源冲突时，主槽先施工；副槽保存已完成修改和证据，结束当前不可中断的原子操作后等待，主槽完成冲突部分并明确释放后继续。无冲突工作可继续；不覆盖副槽修改、不终止不属于本路的进程。冲突范围与释放状态统一写入54，并通知对应施工槽。

- 用户于2026-10-26明确更新：项目允许最多三个并行施工槽位；此前2026-10-16两路上限由此替代。新增槽仍须登记明确范围，共享接入串行。具体功能仍须有已确认方案、相应施工授权和激活前复核。
- `54` 是统一施工登记入口，记录每路范围、当前文档/Gate、工作目录/分支、涉及文件及共享接入点的修改归属。已有总施工中的任务被提取到另一条路时，同步原文档分工，避免重复实施。
- 默认使用独立 Git worktree/分支，构建输出和测试产物分开；保留未提交改动并确认第二路实际依赖的源码基线，不假定新 worktree 含有另一窗的未提交代码。
- 共享文件、SDK/schema、帧循环和公共装配入口明确由一路修改，另一条路按约定接口开发；共享接入、合并与安装串行进行。性能测量避免竞争同一机器资源。
- 各路独立验证自己的修改，合并后补受影响的组合验证；一路完成不代表另一路或总系统完成。入口文档只更新本路相关状态，保留另一条路的在途工作。

## 外部审查文档处理流程

当用户提供 `其它AI审查目录/` 中的审查文档并要求继续方案、施工或测试时，按此流程执行：

1. 先完整读取审查文档，并确认审查对象、审查日期、适用系统编号和当前准备施工的系统编号是否一致。
2. 如果审查对象与当前系统不一致，必须判断它是前置系统约束、历史已完成审查，还是用户给错文件；不能机械套用到当前方案。
3. 将审查结论分为 `必须修改`、`施工约束`、`已由历史施工吸收`、`不适用` 四类，并把判断写入正式方案或施工文档自审。
4. 对 `必须修改` 项，先修改正式方案文档；修改完成后再生成施工文档。
5. 对 `施工约束` 项，写入施工文档 Gate、测试命令或自审，不允许只在最终总结中口头说明。
6. 对 `已由历史施工吸收` 项，引用完成记录或当前入口摘要，并声明当前系统只继承约束、不重复实现。
7. 对 `不适用` 项，必须说明原因，例如审查对象是已完成前置系统、长期 deferred 能力或与当前系统边界无关。
8. 若没有需要修改方案，也必须在施工文档自审中写明“读取审查文档后无需修改正式方案”的判断。
9. 生成施工文档后仍要按常规流程先做施工文档自审，再施工、分 Gate 测试、整体回归、阶段完成记录和入口同步。

## 架构边界

- 引擎只提供项目无关底座能力，不把具体玩法写进 Core。
- 复杂打飞机中的 `Player / Enemy / Bullet / Score / Health / Wave / Weapon` 只能在项目侧数据、Prefab、Rule、AUI Binding 中出现，不能进入通用引擎 API。
- 自然语言不是产物真相；项目文件、代码、资源和验证证据必须可追溯。Feature Spec / Patch Plan 按风险使用，不替代真实项目代码。
- 项目逻辑边界服从顶层 v1.1 和当前有效子方案；`195` / `196` 仅解释既有 Contract-bound RuleSlot，不是更高权威。`IR` 是受限规则数据，不是通用脚本语言。
- 用户/AI 可以修改真实 Rust Project Module、Project Assets、AUI Document 和已有规则资产；`Canonical Rule IR` / Generated Glue 是内部派生产物。
- 复杂 gameplay 流程、复杂算法、复杂 UI 工作流默认用 Rust Project Module / Rust Framework；不要为了进入 IR 增加递归、while、任意函数、任意数组算法、UI event、直接 ECS / Renderer / File / Network API。
- 不新增 Logic Ownership Router / Architecture Guard 作为运行时或架构层；复杂功能默认通过 Feature Asset / Feature Folder 聚合 `ui / rules / logic / tests`，只作为编辑期组织、AI patch scope 和测试报告聚合。
- 用户/AI 不直接手写 Rust AOT 派生产物或底层 engine internals。

## Runtime / ECS / Scene

- Rust Native Runtime 是唯一正式 runtime。
- RuntimePackage 是发布运行输入真相；Runtime 不扫描项目源目录，不读取编辑器内存 Project Object。
- Editor Play/Preview 也应走 RuntimePackage 或等价运行产物路径。
- Scene 是 Entity tree，不是 ECS snapshot。Component 是纯数据，行为进入 Systems / Blueprint / DSL / IR。
- Project Rule 可在允许阶段读写 Component，但 ECS 结构变化必须走 deferred command / Runtime Spawn Request / Runtime Despawn Request。
- Project Rule 不能持有裸 ECS 指针或裸 entity index；运行时引用使用 handle + generation + diagnostics。

## Projection 规则

内部跨域同步统一使用 World Projection / Projection Adapter。旧名称只作历史命名：

```text
RuntimeScene Hydration = HydrationProjection
RenderExtract = RenderProjection
SpriteRenderer2D Bridge = RenderProjectionAdapter<SpriteRenderer2D>
RenderAssetBridge = AssetProjection
Physics2DBridge = Physics2DProjection
AuiRenderExtract / AuiRendererBridge = UiProjection
```

禁止为单个类型新增独立 Bridge / 独立同步器。新增类型只新增对应 ProjectionAdapter 和 Report。

项目逻辑禁止直接操作 `RenderProxy`、`RenderSceneState`、`Physics2DWorld` 内部对象、GPU handle、UI render pass command。

## Build / RuntimePackage

- Build/Export 是 AI-native Build Graph + Export Orchestrator；Runtime 只做轻量加载。
- RuntimePackage v1 必须有稳定 schemaVersion / manifest / hash / diagnostics。
- 用户/AI 不手写最终 Bundle 内容；Build Graph 根据 Asset Graph、Bundle Policy、Build Profile 生成。
- `ProjectRuntimePackageAssembler` 是项目目录进入 `RuntimePackageBuildInput` 的唯一正式装配入口；不要给 Scene / Prefab / Rule / AUI / Input 各自造临时导出桥。

## Asset / Library

- 所有工程对象都按 Asset 管理：texture、material、mesh、audio、UI layout、font、prefab、scene、DSL、IR、build profile、blueprint。
- 项目资源使用 GUID/meta/AssetRef，不靠 path-only 长期引用。
- AI 资源生成以 Asset Spec 为中心，资源必须进入 Project Library / Asset DB / Asset Graph。
- 删除、替换、热更资源必须先做影响分析并输出结构化报告；不能制造悬空引用。

## AUI / HUD

AUI 正式方向是 `AUI Runtime HUD Authoring / Binding / Present v1`，不是 Editor UI、Sprite2D world object、Unity IMGUI clone、UGUI prefab clone、打飞机专用 HUD API。

正确链路：

```text
AUI Document
  -> RuntimePackage
  -> ProjectUiStateSnapshot
  -> Binding Resolve
  -> AuiLayout / AuiDrawList
  -> UiProjection / AuiOverlayFrame
  -> RuntimeRenderer UI Pass
  -> Present
```

规则：

- AUI Document 不保存运行时值，不直接引用 ECS entity，不保存项目专用语义。
- Binding 只读 `ProjectUiStateSnapshot`，不读 ECS，不调用 Project Rule。
- Renderer 只接收已解析后的 `AuiOverlayFrame`，不读取 binding path 或 ProjectUiStateSnapshot。
- AUI action 是业务级 UI 意图，进入 Project Rule；AuiCommand 只表示指针级事件。
- HUD 必须通过 AUI；禁止用 debug overlay 假装 HUD。
- Runtime UI text present 必须能看到真实 glyph，不能只靠 text_count 或占位矩形过验收。
- ProjectUiStateSnapshotProducer 不是全量 UI 镜像器；后续所有 UI state 默认按 active binding path 生成，并走 dirty / cached。禁止在 runtime 热路径每帧全量生成全项目 UI state。

## Testing / Report

- AI 不能说“看起来可以”；必须用测试、结构化 report、trace、diagnostics 证明。
- 每个可执行需求需要测试或明确豁免。
- 默认自动化优先 headless deterministic gate；真实 OS window / GPU screenshot smoke 可作为 local-only 或 optional，不默认阻塞 CI。
- 报告必须能说明 source path / domain / stage / diagnostics / next action，AI 不应猜日志。
- 之后所有 report / trace 设计都必须区分 runtime 与 editor，并显式分档：Off / Summary / Trace。正式 runtime 默认 Off 或只保留功能必需 compact result，不默认生成 JSON、长字符串、完整 candidate trace 或写文件；Summary 只给轻量状态；Trace 只用于测试、gate、debug 或用户显式诊断。Editor report 也必须按 Off / Summary / Trace 分档，Report Panel 只接 Summary / Trace 产物，不把 runtime 热路径变成常驻 report 系统。

## 新系统 / 新方案讨论规范

讨论每个新系统、新方案或重大缺口收敛时，按此顺序：

1. 先用简单明了的话说明：这个系统是干什么的、在其它成熟引擎 / 工具中大致对标什么、在本引擎主线中承担什么作用。
2. 再联网搜索成熟引擎 / 工具的实现做法，优先 Unity / Unreal / Godot / Bevy 和官方文档、源码说明、可信工程资料。
3. 参考其它引擎时，必须尽量深入其源码或本项目已有源码参考文档，明确指出具体源码文件、关键类/函数/调用链、可学习点和不可照搬点；不能只停留在概念类比或官方手册摘要。
4. 再搜索用户提供的引擎源码、本项目源码参考目录和其它 AI 审查目录，重点看它们如何分层、如何验证、哪些做法不适合照搬。
5. 再读取本项目当前入口、已有方案、完成记录、施工文档状态和相关代码基线，不凭旧窗口记忆决策。
6. 先给 2-3 个可选方案，再按本项目规则过滤成 2-3 个推荐方案；此时只在对话中给方案，不写正式方案文档。
7. 等用户明确选择或确认方案后，才允许把选定方案写入正式方案文档；若用户要求继续修改方案，则继续在对话中收敛，不提前落文档。

推荐方案优先级：

1. AI 适配性：schema-first、结构化 patch/report、可验证、可回滚、AI 可生成和审查，避免黑盒隐式状态。
2. 复杂项目适配与可维护：能支撑长期复杂项目，边界清晰，后期可修改、可扩展，不把具体玩法写入引擎 Core。
3. 效率：运行时 / 构建 / 编辑体验效率与施工复杂度平衡，优先小而真实的完整链路。

保持方案粒度是大系统和真实链路，不陷入孤立小功能点。采纳外部做法时，必须在用户确认方案后写回正式方案文档或施工文档。
