# AI First Game Engine

当前实现、安装状态及下一步只见 [49 当前基线](框架设计/引擎总体架构/49-当前状态与下一步入口.md)；唯一施工槽见 [54 执行入口](框架设计/引擎总体架构/54-当前可自动化施工总入口.md)。顶层产品需求和架构保持 v1.1。

[337 源码完成记录](框架设计/引擎总体架构/阶段完成记录/2026-09-13-337-Fixed-Player-Host-Reuse-v1/00-总览.md)保留固定 Host/Engine DLL 跨项目复用、79 项测试和原样游戏闭环证据。安装接入状态不在 README 重复维护。

## 历史状态摘录

本节保留旧日期记录；其中“当前”“最新”“下一步”、进程身份及授权均只属于当时，不定义现在的基线或施工槽。以下旧记录到“产品定位”前为历史摘录。

2026-09-13 最新：336 Engine Runtime DLL 可见试玩与语义执行接入已归档。70 项去重相关测试通过；原样打飞机交付的 59 文件完整副本通过窗口 120 帧截图、原场景 1800 tick / 16 断言 / 4 捕获及 observe，普通和语义报告均确认 DLL 执行 owner。首次/无修改重复/项目副本注释修改构建分别为 77.223 / 19.055 / 26.329 秒，Engine DLL 摘要保持；这是本轮缓存条件下的绝对耗时，不是全冷对比。新版 Provider 已同路径安装并通过独立进程预检，配置未改；当前会话仍使用旧 PID 15964，状态为 `host_connection_reload_deferred`，未宣称 `host_activated`。当前/待执行无有效施工，下一步是客户端重连后的真实 typed 验收。见 [336 完成记录](框架设计/引擎总体架构/阶段完成记录/2026-09-13-336-Engine-DLL-Visible-Semantic-Execution-v1/00-总览.md)。以下旧日期状态按历史理解。

2026-09-13 最新：335「单一权威交付布局与导出产物一致性 v1」方案 B 已完成归档，65 项去重相关测试通过。导出与验收共用布局/身份检查，Compiler 绑定原始清单摘要；单次成功包的 59 个文件原样复制后运行 1 个 headless 帧，确认 Engine DLL 为执行 owner。证据限于适配了 HUD/documentId 和 `test.frame` 合同的复杂打飞机测试夹具，不代表原样完整玩法、性能或安装宿主验收。九项独立工具、326 build/verify 解耦和 334 内部缓存语义保持；Provider/宿主配置未改。当前与待执行均无有效施工，见 [335 完成记录](框架设计/引擎总体架构/阶段完成记录/2026-09-13-335-Delivery-Consistency-v1/00-总览.md)。下列旧状态按历史时间理解。

2026-09-11最新：326开发构建与交付验收解耦源码集成完成并归档，90项测试通过。本轮报告引用/失败分支/默认策略收尾补丁未更新安装Provider；此前宿主13.693秒缓存构建与显式verify证据保留。当前无有效施工，见[326完成记录](框架设计/引擎总体架构/阶段完成记录/2026-09-11-326-Development-Build-Delivery-Decoupling-v1/00-总览.md)。下列旧状态按历史理解。

2026-09-11最新：325修复版Provider已安装，独立完整打飞机构建首次116.1秒、缓存命中20.7/16.1秒，16断言与4捕获试玩通过。当前宿主Transport closed，等待用户重连后的typed验收；325仍为唯一当前施工。见[325记录](框架设计/引擎总体架构/阶段完成记录/2026-09-11-325-Provider-Activation-Shooter-Build-Timing-v1/00-总览.md)。

2026-09-11最新：324 Player编译缓存复用修复完成归档。9项owner测试通过，SDK局部修改保留未变依赖缓存，最终产物仍绑定完整源码摘要。当前/待执行为空，安装Provider未更新。见[324记录](框架设计/引擎总体架构/阶段完成记录/2026-09-11-324-Player-Compile-Cache-Reuse-v1/00-总览.md)。

2026-09-11 最新：323打飞机操作手感与战斗反馈对齐完成归档。自动双发、鼠标跟随、尾焰、分层星空、固定预算粒子与AUI飘字已实现；138项相关Rust测试，最终开发/优化版各16断言与4捕获通过，指针回放两点像素检查通过。OS自动化输入未确认，真实鼠标手感待用户试玩；默认工具仍9项，安装/config未变。当前/待执行为空。见[323记录](框架设计/引擎总体架构/阶段完成记录/2026-09-11-323-Shooter-Feel-Combat-Feedback-v1/00-总览.md)。以下旧状态按历史时间理解。

2026-09-11 最新：322优化版三档负载复测完成归档。六次720帧/600样本全部通过；12/32、24/64、48/128的CPU均值分别约5.2、7.4、12.9ms，最高P95为14.38ms。复用321优化Player与320原包，无源码/工具/安装/config变更。当前/待执行无有效施工；三档固定负载CPU预算有余量，显示FPS、完整玩法容量与内存泄漏未资格化。见[322记录](框架设计/引擎总体架构/阶段完成记录/2026-09-11-322-Optimized-Shooter-Load-Retest-v1/00-总览.md)。以下旧状态按历史时间理解。

2026-09-11 最新：321帧调度修复与优化构建对照已归档。修复每帧完成后额外等待，48项owner测试通过；当前宿主新dev交付相同720帧约14.9–15.2秒，优化比较版约13.2–13.6秒、CPU均值约4ms。两个profile各8轮重开/64断言通过。当前/待执行无有效施工，安装/config及项目玩法不变，无新工具；未宣称显示60FPS、三档release负载或无泄漏资格。见[321记录](框架设计/引擎总体架构/阶段完成记录/2026-09-11-321-Player-Frame-Pacing-Repair-Optimized-Comparison-v1/00-总览.md)。以下旧状态按历史时间理解。

2026-09-11 最新：320打飞机性能与重开稳定性基线完成归档（measurement_completed）。正常战斗开发态CPU均值约15ms、P95约18ms；三档固定负载均值21.62/35.37/64.09ms。8轮重开64断言及不重开对照64断言通过；两组内存均缓慢增长，未证明无泄漏。确认帧完成后额外等待16.667ms，尚未修复，CPU耗时不能倒数当显示FPS。基线完成不等于稳定60FPS达标。当前/待执行为空，319可玩产物及生产代码/安装/config不变。见[320记录](框架设计/引擎总体架构/阶段完成记录/2026-09-11-320-Shooter-Performance-Restart-Baseline-v1/00-总览.md)。以下旧状态按历史时间理解。

2026-09-10 最新：319打飞机最小游戏闭环完成归档。项目侧敌弹、3HP受伤/无敌反馈、死亡冻结与AUI结算、Enter重开已接通；16项项目测试及当前宿主22条断言/4捕获/observe通过。当前/待执行为空，安装/config不变，无新工具，性能未资格化。见[319记录](框架设计/引擎总体架构/阶段完成记录/2026-09-10-319-Shooter-Minimal-Game-Loop-v1/00-总览.md)。以下旧状态按历史时间理解。

2026-09-10 最新：318打飞机项目playtest/observe接入完成归档。13项Rust测试、当前宿主3600 tick的37条断言和3捕获全部通过，observe同runRef回读成功。复用9项工具，新Player包含317诊断修复；安装/config不变，当前/待执行为空。见[318记录](框架设计/引擎总体架构/阶段完成记录/2026-09-10-318-Shooter-Repeatable-Playtest-v1/00-总览.md)。以下旧状态按历史时间理解。

2026-09-10 最新：317运行失败摘要与诊断修复已归档，90项受影响测试通过，real-window编译通过。规则/字段/命令失败不再漏报或被后续成功帧覆盖，Summary保留最多16条有界定位详情，逻辑失败非零退出。当前/待执行为空；源码集成完成，安装/config及316试玩产物未更新。见[317记录](框架设计/引擎总体架构/阶段完成记录/2026-09-10-317-Runtime-Failure-Summary-Diagnostics-v1/00-总览.md)。下列旧状态按历史时间理解。

2026-09-10 最新：316持续试玩与画面细节收敛完成归档，23项受影响测试通过；最终Player实际3600帧持续射击0失败，第10波仍有敌机。补持续生成/回收、玩家边界/连发、枪口与淡出/滚动背景/紧凑HUD，并修复颜色字段写入与同帧重复销毁。当前/待执行为空，安装/config不变。见316方案、归档施工文档及阶段记录；新交互产物engine-op-mcp-process-12864-10已打开，仍为训练模式，未宣称完整游戏或性能资格化。以下旧“最新”条目按历史时间理解。

2026-09-10 最新：315视觉样例完成归档，49项受影响测试通过；统一Kenney CC0美术/HUD、开火/命中反馈，并修复Prefab贴图漏准备和sRGB上传。真实窗口截图通过，当前/待执行为空。见 `框架设计/引擎总体架构/阶段完成记录/2026-09-10-315-Shooter-Visual-Slice-v1/00-总览.md`。大字号字体路径与完整游戏/性能仍未资格化；以下为历史记录。

2026-09-10 最新状态：314发射位置与销毁呈现修复完成归档，587项测试通过；当前宿主新Player已实证移动后枪口发射、击杀后敌人图像消失，交互窗口已打开。当前/待执行为空。记录见 `框架设计/引擎总体架构/阶段完成记录/2026-09-10-314-Shooter-Spawn-And-Despawn-Present-Repair-v1/00-总览.md`。下文313残留缺陷描述为修复前历史。

2026-09-10 最新状态：313可见试玩链修复完成归档；Compiler/AUI回归79项通过，当前宿主新交付已实证显示HUD、移动与两次发射，修复后交互窗口已打开。当前/待执行为空。详见 `框架设计/引擎总体架构/阶段完成记录/2026-09-10-313-Visible-Shooter-Play-Chain-Repair-v1/00-总览.md`。安装Provider可消费项目v2 HUD，但未升级为本次legacy转换修复；完整击杀视觉闭环与性能尚未验收。下文安装/空槽状态按其历史时间理解。

2026-09-09：311 `Game Project Compiler + Incremental Prepare/Run v1` 窗口 A/B/C 已完成并归档，
施工文档位于 `框架设计/引擎总体架构/施工文档/已完成/311-Game-Project-Compiler-Incremental-Prepare-Run-v1施工文档.md`。
2026-09-10：312 A-D全部通过并归档，source_integration_passed；D去重201项测试通过，默认工具面9项Ready，新增playtest/observe。当前施工槽与待执行为空；安装态未更新。下文旧系统描述为历史状态。
312施工文档：`框架设计/引擎总体架构/施工文档/已完成/312-Semantic-Runtime-Observation-Outcome-Acceptance-v1施工文档.md`，终态证据见第14节。

当前唯一顶层产品与架构 authority 是：

```text
框架设计/引擎总体架构/00-AI-First-Game-Engine-权威产品需求-v1.1.md
框架设计/引擎总体架构/00-AI-First-Game-Engine-权威架构设计-v1.1.md
```

当前正式主线是 AI-native Game Project Compiler and Runtime Platform：AI像直接开发普通代码项目一样编写游戏，并在同一个 Agent Tool Loop 中直接调用具体 Engine tools。Rust Native Runtime 和 RuntimePackage 继续是正式运行基础；Native Editor 已降为可选验证 Projection，不再是 AI、项目、Runtime、Build 或 Tool Provider 的默认 authority。

权威架构下第一个正式子设计是 `框架设计/引擎总体架构/309-Headless-AuthoringProjectContext-Unified-Mutation-v1方案.md`。用户已确认方案 B：采用嵌入式温状态 Context 和统一跨进程 mutation/CAS 合同，不建立默认常驻 daemon；`AuthoringProjectContext` 只作为 Engine Provider 内部 Module，永不成为 AI 可见的二级 Host 或第二个 Agent。309-A 至 309-E 已依次完成 Headless read、snapshot lease、统一 mutation/CAS、crash recovery 和 Editor Scene 文档 LWW；309-F 已完成 C-Atomic Headless Engine Tool Provider 切换、唯一中立 Compiler/Assembler、No-Editor run/build/delivery、Editor cutover 与旧 Gateway 生产退役。Gate A-I 和 F1-F12 全部通过；F11-R3 已在人工新建的真实 Codex projectless 任务中完成唯一 inspect/mutate/run/build/delivery verify Agent Tool Loop，结构化 receipt、revision、artifact、delivery 和进程证据均通过，真实配置与临时 Provider 已恢复。309-F 已生成完成记录并归档，当前施工槽与待执行队列均为空；Tower P1-2 继续 `stopped_by_user`。

权威架构 R2 的第二个正式子设计 `框架设计/引擎总体架构/310-Project-Game-SDK-Generated-Runtime-Glue-v1方案.md` 已完成并归档。方案 C 只新增项目侧 Project Game SDK 外部 Interface，复用既有 Provider 与 GameProjectCompiler；Generated Runtime Glue 是 Compiler 内部派生的 `ProjectRuntimeAbi` Adapter，不是独立 Module、Host、Provider、Registry 或第二个 Compiler。Gate A-D 已通过，三个样例已迁移，旧静态 sample ABI/Player pipeline 已退役，能力状态为 `source_integration_passed`；当前施工槽与待执行队列均为空。

`框架设计/引擎总体架构/309-F-Default-Engine-Tool-Surface-Skill-Workflow-Convergence-v1方案.md`
也已完成 Gate A-C 并归档。当时模型默认 Engine 工具面为6项，后续311扩为7项；Native/MCP
使用同一 Registry 投影和 canonical `engine-tool-result.v2`；最小 Skill 只提供条件化知识与局部恢复提示。
No-Editor MCP process 已通过 `inspect -> runtime_run -> build -> delivery_verify`。完整 R3/R4、真实
cancellation、production、真实 Codex 配置、Android、Local CI 与 Tower P1-2 均未启动；当前施工槽与待执行队列为空。

以下 254-259 内容记录现有实现和历史完成状态；其中 Editor-hosted Gateway / Editor-instance identity 不能再反向定义顶层产品架构。

当前 AI 工具主线已完成 254 Core 范围修正、lifecycle retirement 与 255 Capability-aware Tool Catalog，并已确认 256 方案 B：Gateway 身份绑定稳定 Native Editor 进程实例，项目身份降为连接内可选、可变化的上下文；公共新增 Tool 仍只有极简 `project.create(requestedProjectRoot, projectName)`。254 只负责向 AI 提供好用、自由、可审计的引擎工具，不负责生产精确引擎 release candidate 或证明真实 AI outcome acceptance。254-R1/R2 仅保留历史证据；R2-FC6 的 terminal failed 状态不得重试，也不得执行 FC7、Real Evaluation、activation、真实 Codex attempt、F-A/F-B/G 或三引擎 B 通道。

255 已完成并归档。第二个 frozen candidate `447c3810…` 在 G: 获得单独授权后执行且仅执行一次 C9，run `local-447c3810ddbf-1784787443` 通过，12 stages 全绿，source/isolated clean，cleanup removed。两个 frozen candidate 的 authoritative C9 各运行一次；首次空间失败及容量合同修正均已固化到完成记录。

256 `Editor-Instance Gateway Lifecycle + Minimal Project Create v1` 已完成并归档：production typed MCP smoke passed；exact commit `e99b5af…` 的 canonical Local CI run `local-e99b5af45476-1785050176` 以 Trace + fail-fast 完成 12/12 passed，source/isolated clean，nested cleanup removed。外部 role-owned TEMP 因宿主拒绝已验证的递归删除而记为 `retained_by_host_policy`，不影响资格证据。256 完成时施工槽与待执行队列均为空；旧 F0、旧三工具、P8/P9/P10 或 construction Runner 不得恢复。

257 `Native Editor Dark Theme + Workspace Docking v1` 已完成并归档：黑灰色
`EditorTheme::DarkNeutral`、主窗口内 Split/Stack/Tab/Splitter、原子 Tab dock、panel
close/show/reset 与 `EditorWorkspaceLayout.v1` 持久化已进入生产组合；真实窗口与 DPI 证据、
受影响域回归以及 exact commit `6fac851…` 的 Local CI 12/12 均通过。257 完成时施工槽与待执行
队列为空；真实使用反馈已经形成独立 258 方案，257 仍保持完成态。

258 `Native Editor Floating Workspace + Panel Chrome v1` 已完成并归档。现有深
`EditorWorkspaceDockingModule` 已覆盖细视觉 Splitter、统一 Lock / More / Close Tab、
typed Inspector context Lock、WorkspaceTopology v2、真实浮动原生窗口和跨窗口 Tab dock。
W5 冻结提交为 `ca2eca2…`，W6 最终提交为 `b7b5607…`；三个真实 Windows authority
scenario、受影响域回归和 production Editor replacement 均通过。258 归档时施工槽与待执行队列为空，
258 保持完成态；其真实小游戏制作反馈随后触发了独立 259 方案讨论。

259 `External Codex Authoring Readiness: Connection Recovery + Deep Mutation Contract v1`
已完成并归档。它只深化现有 `EditorInstanceGatewayModule`，并在既有
Gateway / Tool Kernel ownership 内引入内部 `GoalMutationModule`；公共 mutation 面收敛为
`project.mutate` 与 `project.rollback(rollbackRef)`，不新增外部 AI 菜谱、Planner、Workflow、
Runner 或真实 AI outcome acceptance。259 已完成并归档；窗口 A 的 C0 stable install/config 与
C1 discovery recovery 已在提交 `400d2d1…` 完成，`ai_tool_gateway` 受影响回归全绿，真实
Codex config/install/Editor 均未修改。窗口 B / C2 Deep `project.mutate` contract 已在
提交 `7195070…` 完成并通过受影响回归。窗口 C / C3 Same-operation approval 与
bounded Grant reuse 已在提交 `392336c…` 完成，受影响 owner/consumer 回归全绿。
窗口 D / C4 `rollbackRef` 与 public cutover 已在提交 `cdef3a0…` 完成，旧 Candidate
public entries 已移除，opaque rollback owner/consumer 定向验证全绿。窗口 E / C5 affected
integration、release process smoke 与 release build 已在 clean commit `f044223…` 通过。
窗口 F / C6 real stable install/config migration 已按 2026-07-28 单次授权执行并通过：
stable MCP 已安装到当前用户 LOCALAPPDATA，真实 Codex config 只替换
`mcp_servers.ai_first_game_engine.command`，receipt 为
`C:\Users\zenghaoran\AppData\Local\AiFirstGameEngine\Gateway\codex-config\1785206768146-stable-mcp-migration-receipt.json`，
`reloadOrNewTaskRequired=true`。Window G / C7 的多次新任务均未获得 `aife_*` typed tool；
production read-only diagnosis 随后确认第二个 stable Gateway 在 MCP `initialize` 前因 Editor
Named Pipe 最大实例数为 1 而以 Windows error 231 退出。C7-R1 已修复该引擎侧缺口：
Named Pipe 支持多实例、每连接独立 worker、accept 持续 re-arm、client 对瞬时
`ERROR_PIPE_BUSY` 有界等待，shutdown 取消并 join owned workers；双客户端与 shutdown 竞态
各 20/20，完整 `ai_tool_gateway` 回归全绿。exact source `3a0eef5…` 的 production Editor
与 stable MCP 已完成事务化替换；两个 installed MCP client 同时完成 initialize、tools/list
（各 21 tools）与 shutdown，均 exit 0，error 231 已消失。2026-07-28 Window G / C7
新任务已真实获得 21 个 stable MCP typed tools，`aife_status`、`aife_catalog` 与
`project.inspect` 通过。唯一一次 disposable `project.mutate` 在等待用户批准期间检测到
项目漂移并以 `gateway.operation.project_drifted` terminal failed；`commitStarted=false`、
`changedDomains=[]`、`recordedOperationCount=0`，没有生成 `rollbackRef`，也没有执行
`project.rollback`。漂移文件为 `Scenes/Main.scene.json`，由普通 Editor 场景保存路径在批准
决策前重写为规范化 JSON；项目 digest 从 `sha256:6757…291d1` 变为
`sha256:2647…e6b0b`。owner 复现得到完全相同的摘要变化，并证明首次 clean save 会规范化
旧 Scene、第二次保存字节稳定；Gateway fail-closed 行为正确。C7-R2-A 已在 exact commit
`f92962c…` 完成 clean-save no-write 修复和受影响回归；C7-R2-B 已用 SHA256
`1c4dbff6…` 的 production Editor 完成事务化替换，并证明 clean Save 前后 Scene bytes、
mtime 与项目 digest 不变。C7-R2-C 在全新 task、run root、project 和 operation 上完成
21-tool typed qualification：`tool-op-8b564d1e8f4063cc79cfc746` 经 Native Editor 批准后
只创建一个 Input 文件，随后以 opaque rollbackRef 回滚，项目 digest 精确恢复为
`sha256:6757…291d1`。原 terminal mutation 永久禁止重试；Local CI 未运行。

## 产品定位

```text
AI是首要操作主体，继续直接编写真实游戏代码、UI、资源、测试和工程内容。
具体Engine tools与普通文件工具同级；引擎适配AI，不要求AI模拟传统Editor。
默认完整生产路径无Editor；Editor只在用户主动打开时用于验证和精修。
```

产品需求与顶层架构的唯一正式解释见：

```text
框架设计/引擎总体架构/00-AI-First-Game-Engine-权威产品需求-v1.1.md
框架设计/引擎总体架构/00-AI-First-Game-Engine-权威架构设计-v1.1.md
```

## 当前入口

```text
rust/                         正式 Rust workspace
框架设计/引擎总体架构/          架构、规则、施工文档入口
legacy/typescript-prototype/  已退役 TypeScript / Electron 原型，仅历史参考
```

## 常用验证

```powershell
cd rust
cargo fmt --check
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p editor_window_winit
```

## 文档入口

```text
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
```

## TypeScript 原型层状态

早期 TypeScript / Electron / Vite 原型已经退役并移动到：

```text
legacy/typescript-prototype/
```

它不再是正式运行时、编辑器或构建入口。Rust Native Runtime 是唯一正式 runtime。
