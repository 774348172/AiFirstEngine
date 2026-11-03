# 312 Semantic Runtime Observation + Outcome Acceptance v1

> 文档类型：正式架构子方案
> 状态：用户已确认方案 B；窗口 A-D passed；2026-09-10已归档，source_integration_passed
> 日期：2026-09-09
> 对应阶段：R4 Semantic Outcome Loop
> 选择：有界试玩 + 语义断言 + 证据复验
> 上游：权威架构设计 v1 第 12、16、17 节；264 Observation Contract；310 SDK；311 Compiler

## 1. 结论

R4 回答“游戏是否按预期工作”，不只回答“进程是否正常退出”。项目声明输入时间线和预期业务事实，
一次有界 playtest 自动完成准备、运行、采样、断言、按需截图和进程回收；AI 根据结果选择修复、重试
或查看证据。公开有意义的阶段事实，但不要求 Agent 逐步调用内部机械操作。

首版面向 Windows 本地 No-Editor Player。复用已有观测、输入、捕获、Compiler 和交付模块，不建设
长驻测试服务、订阅系统、第二个 Agent 或通用测试语言。小游戏可以与项目测试场景迭代开发，不必等待
完整 R4；引擎能力通过对应资格后才可作为自动验收真相。

## 2. 已有基础与前置缺口

| 已有基础 | 代码依据 | 使用方式 |
| --- | --- | --- |
| typed contract、snapshot、session/frame | `rust/crates/engine_runtime/src/project_observation.rs` | 业务事实来源，不查询任意 ECS 内部状态 |
| observe 注册、ObservationWriter | `rust/crates/project_game_sdk/src/lib.rs` | 项目公开少量稳定业务值 |
| NativePlayerInputScript、截图 | `rust/crates/runtime_player_winit/src/lib.rs` | 深化已有输入/捕获路径，不建立并行执行器 |
| Compiler、RuntimeExecutionReport、DeliveryRef | `rust/crates/project_authoring_execution/src/game_project_compiler.rs` | 准备、产物身份和交付链 |
| 有界进程、导出验证 | `rust/crates/runtime_cli/src/bounded_child_process.rs`、`exported_player_verification.rs` | 授权、退出、超时和回收 |
| Native/MCP Provider | `rust/crates/engine_tool_provider/src/lib.rs` | 同一 Registry 投影已资格化能力 |

这些是可复用基础，不代表 R4 已实现；已有按帧输入也不自动证明固定模拟 tick 回放或玩法断言。

生成方案时311的MCP错误修复链与终态回归证据尚未闭合。2026-09-09经单独授权恢复311，已补齐错误定位 ->
Host修改 -> check通过、直接run及107项终态测试；发现并修复Provider丢弃源码位置/Compiler诊断问题。
最终证据见311归档施工第14节，入口同步后激活本方案施工。该前置整改不算R4 Gate通过；后续 A 的独立证据见312施工第11节。

## 3. 方案选择与研究依据

| 方案 | 范围 | 判断 |
| --- | --- | --- |
| A 观测优先 | 业务状态与截图，人工判断 | 最快，但不足以重复验收玩法 |
| B 有界验收闭环 | 输入、状态断言、截图、导出复验 | 用户已确认，本方案唯一方向 |
| C 完整交互会话 | 长驻运行、订阅、暂停续跑、任意操控 | 延后，不是小游戏验收前置 |

- 已直接读取本地 Unity 源码 `I:/UnityCode/UnityCsReference-master/UnityCsReference-master/Runtime/Export/Scripting/WaitUntil.cs`。
  `keepWaiting` 检查 predicate，`GetTime()` 区分游戏/真实时间；学习条件与超时分离，不照搬任意回调。
- 已读取 `框架设计/Bevy源码参考/14-AppRunner-Winit-ScheduleRunner-RunSession源码参考.md`。
  `App::run/set_runner`、`ScheduleRunnerPlugin` 与 `WinitPlugin` 展示运行方式与 App 内容分离；
  学习共享项目语义，不把 Headless 通过当成窗口/GPU 通过。此为仓库源码参考，不是最新上游核验。
- 已读取 264 正式方案，继承 post-commit 观测、typed contract、有界等待和等待不重放 action 的约束。
- 在线请求 Godot/Unity 上游源码失败或超时，不宣称完成最新联网核验；本轮没有新的外部 R4 审查结论。

## 4. 项目测试输入

测试场景是项目侧 schema-first 数据，通过现有项目资源机制进入受控输入，不新增独立测试资产库。
窗口 A 已按现有依赖冻结以下合同，不新增 crate、资源 catalog 或通用 DSL。

最小内容：初始场景、适用目标、输入时间线、检查点或有界区间、观测 path 与 typed 预期值、捕获点、
最大帧数与 wall-clock timeout。种子仅在项目实际支持并消费时声明受控。

- 业务值来自 Observation Contract，不从 HUD 文本、调试输出或全量 ECS 反射猜测。
- 首版采用 typed 等值断言；区间条件要求期限内某次已提交 snapshot 满足，不引入任意表达式树或脚本。
- 复杂条件由项目 Rust 计算并公开业务事实；敌人、子弹、胜负等概念不进入 Engine Core。
- 输入在明确模拟阶段应用一次，成功提交后采样；捕获记录对应帧和运行身份。
- 未知 path、类型不符、非法时间线和超限输入拒绝执行；未产出、合同违约、条件未达分别报告。
- 测试场景绑定冻结项目版本，不在执行中重读 live 文件或混用另一版本产物。

### 4.1 窗口 A 冻结合同

唯一 schema owner 为 `runtime_player_winit::semantic_outcome`，版本 `playtest-scenario.v1`。
Compiler 通过同方向的直接依赖复用 DTO；`PreparedRuntimePackage::load_playtest_scenario` 只从其已保留
SourceView 读取 manifest 的 `playtestScenario` 引用及 JSON，返回场景、sourcePath/SHA256、preparationIdentity
和原有 lineage。`scenarioId` 为稳定身份，`initialSceneId` 必须存在于同一 prepared 场景集合中。
这是一个默认场景引用，不是新资产索引；普通 prepare/run/build 不强制声明或加载 playtest。

| 字段/规则 | v1 合同 |
| --- | --- |
| target | windows-headless / windows-windowed；不表示捕获已资格化 |
| inputs | 一基 simulationTick，严格递增；keyboard keyDown/keyUp，使用现有逻辑键名 A-Z、0-9、Space、Enter、Escape、Arrow 方向键 |
| 键状态 | 同 tick 不重复或同时按下/释放，禁止重复按下、无按下的释放，结束前全部释放；无输入 tick 不伪造 action |
| assertions | 唯一 assertionId，ObservationContract path，typed equals；fromSimulationTick/throughSimulationTick 闭区间，同值即检查点；按区间起点非递减 |
| captures | 唯一 captureId，严格递增 presentationFrame，显式 required / subjectiveReview；不把呈现帧当模拟 tick |
| 限额 | 两种帧上限各 1..36000、timeoutMs 1..120000、4096 输入点、每点32次键转换、64断言、16捕获、原始JSON 1 MiB |
| seed | 尚无项目消费接口，任何非空 seed 均明确拒绝 |

typed 等值复用 Runtime 的 bool/integer/number/string，拒绝未知 path、非有限值、类型不符和 allowedValues
之外的期望。number 使用 JSON 小数形式，与 integer 不隐式转换。复杂条件继续在项目 Rust 计算。
缺失的声明项由聚合器生成 not-produced-yet；Technical 必需，Gameplay/Visual 是否必需由场景推导，
指定交付执行另要求 Delivery。所有声明断言必需，optional capture 不参与自动通过判定；没有必需视觉检查
时 Visual 为 not-checked。主观截图只到 pending-review；必需项 failed / unsupported / not-produced-yet /
not-checked / pending-review 都不能算通过，失败优先。执行循环、实际观察来源、session/产物证据验证在 B/C，
本窗纯合同测试不证明运行或视觉结果。

## 5. 执行与工具边界

```text
项目测试场景 + 冻结项目输入或指定交付产物
  -> 既有 Compiler/Player 路径
  -> 输入执行 -> post-commit 观测 -> 断言/按需捕获
  -> 有界结束与回收 -> 紧凑 Outcome 和 evidence refs
```

`engine_runtime_playtest` 执行一次有界测试，直接返回足以作决定的摘要。
`engine_runtime_observe` 按明确 run/evidence 身份读取已产生的业务观测或捕获证据，不隐式重跑、不注入
输入、不查询另一个当前 session，也不是通用文件工具。首版不要求活跃进程；失效引用明确拒绝，
不回退到最新运行。普通文件阅读继续使用 Host 文件工具。

两个工具通过真实 owner 和 Native/MCP 合同后，默认面才由 7 项到 9 项，方案确认不等于 Ready。
不新增 prepare/capture/wait/cache/status/stop 工具。普通 run/build 不强制 playtest；摘要充分时不必
接 observe。复用 ToolResult，只有实际不兼容才升级版本。Skill 解释条件化流程，不持状态或目标计划。

### 5.1 窗口 B 的已实现执行边界

以下保留B收尾时边界；C对窗口与证据的扩展见5.2，不用后续资格覆盖历史验收。

Windows headless 复用 Player 原循环，显式每次推进一个 fixed tick，按真实 fixed_frame_count 注入输入并读取
当次 post-commit snapshot；零步不重复采样/注入，跳步或倒退返回 unsupported，不虚构中间 tick。
场景在项目 Runtime 初始化前校验。键按住期间保留既有 action_pressed 语义，区间等待不重放按键转换。
语义报告只保存声明断言的实际值、首次命中/末次检查帧、期望和状态，不添加全量逐帧 trace。

CLI `playtest --package <RuntimePackage> --scenario <json> --output-dir <fresh-directory>` 复用有界进程 owner，
同一可执行程序以内部 worker 入口运行已有 Player；Windows Job 约束超时及父进程丢失后的后代生命周期。
库内执行只有合作式时间检查；直接调用内部 worker 不具有父级硬超时资格。非零退出、缺报告、身份不符或
未确认回收不能伪装成功。输出目录须新建且由调用者保留/清理，本窗 runId 只标识该输出目录，非交付身份。

本窗 Windows windowed 明确不支持；C 才接入同一判定器、真实窗口捕获及 Compiler/指定交付身份。
SDK observe 的 Player 测试桥证明输入到业务值的链路，不替代 C 的 generated glue/导出组成验证。
Provider 未新增 Ready 工具，主动取消仍不开放；普通 run/build 不要求场景，不添加语义 JSON 热路径。

### 5.2 窗口 C 的执行与证据接线

Compiler新增playtest组合入口、playtest_delivery与observe_playtest，继续复用PreparedPlaytestScenario、
DeliveryRef及生成Player，不建立另一装配器。新建交付仍经过原有live binding检查；指定交付复验只消费
已冻结场景与该交付的Game.exe/data/runtime_package，不从live源码重建。Windows Dev是当前支持的交付布局，
不把Windows Release或其它平台假报为支持。DeliveryRef保留Player及运行包树摘要，启动前验证、执行前后核对。

Windowed与Headless共用语义判定器；测试输入不叠加现场键鼠。Surface失败、提前关窗明确失败，不以模拟重跑
补同一呈现帧。捕获只发生于声明帧，保留session、真实tick、presentation frame、像素尺寸、字节数与SHA256。
截图上限4 Mi pixels、单文件32 MiB；图形读回只用于显式请求，不进入普通运行默认路径。

CLI运行报告记录唯一runId、归一场景SHA256、实际Player及运行包摘要；Compiler报告额外保留原始场景sourcePath/
sourceDigest、preparation/lineage和DeliveryRef。证据引用绑定报告路径、runId与报告摘要；读取核对冻结请求、
capture身份/摘要/限额，不启动进程、不回退最近运行。Provider在D负责从自己的run registry解析这些引用，
底层路径API不直接投影成任意文件读取工具。文件丢失或改变即引用失效，不静默重建证据。

Visual passed只指已要求的原始捕获产出；独立窗口测试再验证真实纹理、非空像素及输入引起的图形位移。
主观审核仍pending-review，不能从截图存在推导可读性或美观。复用既有CPU frame/update/render-submit/
present-wait指标，单位ms、窗口与warm-up显式；截图读回开销不冒充正常玩法或GPU时间，没有阈值不报性能达标。

### 5.3 窗口 D 的 Provider 引用合同

playtest空参数执行当前manifest默认场景；可选deliveryRef只指向本Provider保存的交付及同次prepared场景。
run/build正常不要求场景，若场景有效则随交付保留；没有有效冻结场景的交付不能事后混用live场景复验。
Provider在原交付表保留Compiler/场景，在运行表保留Compiler/报告；不新增对外Registry或服务。
observe仅接受runRef，返回该运行的业务值与捕获引用，不接受任意路径。新Provider或重新绑定项目后旧引用失效。
指定交付/observe不刷新live项目，授权Grant使用保留的project/revision身份；playtest仍须Host批准进程副作用。
试玩的必需Outcome未通过时ToolResult为failed并保留四类结果及证据；observe成功读取失败运行仍是completed，
其output中的失败Outcome不变。摘要足够无需observe；重复同callId复用原操作，不隐式再执行输入。

D真实两个项目测试暴露旧Native rule adapter仍传NULL session，而310 generated glue已要求有效handle。
最小修正仅在该adapter每次registration内把规则回调绑定到既有NativeProjectRuntimeSessionLease；
不使用全局latest session、不创建第二会话、不放宽glue校验、不升级ABI。必须验证规则与observe使用同一会话，
并验证不同绑定不串线、lease终止后不执行旧规则；Runtime owner与实际generated项目链均纳入受影响回归。

## 6. Outcome 与证据

| 类型 | 首版范围 | 不得替代的判断 |
| --- | --- | --- |
| Technical | 启动、崩溃、超时；可采集指标及显式阈值 | 无阈值的测量不是性能达标 |
| Gameplay | 项目业务值满足检查点/区间断言 | 进程成功不等于玩法正确 |
| Visual | 请求帧截图、明确支持的客观检查 | 截图存在不等于画面美观或可读 |
| Delivery | 指定导出产物执行适用场景并复验 | 源项目通过不等于导出产物通过 |

各类分别表示通过、失败、未检查或不支持；主观画面审核保留未决状态。请求的必需证据缺失、不支持、
超时或观测错误均不能折算通过。复用 revision/artifact/delivery 身份，补充必要的场景/run 身份、帧、
实际值/预期值和诊断。原始证据受限落盘，以引用关联；失效或丢失明确报告，不能伪造同次运行证据。

Runtime 默认 Off 或功能必需紧凑结果，测试按需启用 Summary，Trace 仅显式诊断。避免每帧全量 JSON、
ECS dump、常驻截图和未声明观测；核对已有 post-commit producer 成本，不新增永久遥测通道。

## 7. 回放、性能与交付

回放是从相同明确初始条件重新执行输入，不是任意帧回滚或存档恢复。确定性声明限于已验证的项目、
产物、输入、步长、种子和运行配置，不承诺跨平台浮点或逐像素一致。

开发 Player 与导出 Player 复用测试语义；交付复验只消费指定产物和受控测试输入，不悄悄重建 live 项目
代替该产物。目标缺少必要的输入、观测或捕获能力时返回不支持。

性能指标必须有实际来源和单位，模拟、完整帧、GPU 耗时不混名；区分冷启动、预热和测量窗口。
小游戏 benchmark 另定负载、机器、profile、阈值和统计方法；R4 不承诺通用性能数字。Headless 用于语义
与模拟负载；渲染性能必须测窗口/GPU，不能将 Trace 开销混入正常性能结论。

## 8. 生命周期与非目标

复用 Host approval/Grant 和有界子进程 owner，失败或超时确认回收本次进程。帧上限控制模拟工作量，
wall-clock 上限控制执行风险。operation 记录可取消不等于真实子进程取消；主动取消、断连清理的现有
支持与缺口必须在施工自审明确，必要的独立后续范围不能暗中扩为长驻 session。

非目标：通用测试 DSL、任意 ECS 查询、实时订阅、长驻试玩、暂停续跑、时间倒带、自动审美评分、
全平台确定性、性能仪表盘、production/安装态更新、Local CI、恢复已停止 Tower 施工。

## 9. 验收合同与验证经济性

| 合同 | 最窄有效证据 |
| --- | --- |
| typed 场景/观测有效 | 未知 path、错误类型、非法时间线负例不能通过 |
| 输入一次、提交后断言 | Player 小场景验证次数、帧边界、期望命中与故意错误断言 |
| 回放条件明确 | 受控条件两轮结果一致，输入变化改变对应结果 |
| 生命周期有界 | 超时/崩溃保留首因，未确认回收不报成功 |
| 身份与证据一致 | 旧 session、错误产物、缺失截图、失效引用不能通过 |
| 四类结果独立 | 退出码成功但玩法失败；无捕获不报 Visual 通过 |
| 导出不替换对象 | 指定 Player 执行适用场景，不读取 live 项目重建 |
| Native/MCP 语义一致 | 同输入、授权、结果和拒绝；真实 No-Editor process 链 |
| 项目无关 | 一个小游戏正反场景加另一小项目合同，不扩大完整内容矩阵 |

先 owner 红灯，再受影响 consumer，最后一次终态受影响回归；工具清单与成功退出不是语义验收。
有效证据按输入身份复用，真实窗口仅针对 Visual/GPU 声明。施工文档须核对具体文件、命令、成本与
窗口，遵守三小时限制，不自动要求全 workspace 或安装态资格。

## 10. 自审与后续

方案自审通过：符合用户确认 B；复用现有深模块，两个工具对应执行/观察决策点，没有机械工具扩张；
四类 Outcome 分离，未支持项与主观审核不伪装通过。311历史缺口及最终闭合证据已显式记录，不覆盖历史事实。
参考源码范围和联网限制已说明，不把完整会话或性能平台列为小游戏前置。

施工文档：[312 归档施工文档](施工文档/已完成/312-Semantic-Runtime-Observation-Outcome-Acceptance-v1施工文档.md)。A/B/C历史证据见第11/12/13节；D去重201项测试、Runtime会话绑定修复、Native/MCP实际语义链和证据身份见第14节。默认9项Ready，A-D为source_integration_passed；当前/待执行为空，不代表真实Host安装态、主观视觉或整体性能通过。
