# 312 Semantic Runtime Observation + Outcome Acceptance v1 施工文档

> 日期：2026-09-09
> 正式方案：[312 方案 B](../../312-Semantic-Runtime-Observation-Outcome-Acceptance-v1方案.md)
> 状态：窗口 A-D passed；source_integration_passed；2026-09-10 完成归档
> 授权范围：用户授权“窗口 D：Provider、Skill 与终态”；不含安装/真实配置/Local CI
> 目标：Windows No-Editor 有界试玩、语义断言和指定交付复验；不是全平台或整体性能资格

## 1. 激活前置

311最终补验收已完成并归档，本计划经授权成为唯一当前施工文档。
当前目录中的旧254 evidence不是施工授权，不清理、不重跑。以下前置清单保留，执行结果见第10节。

激活前必须：

1. 核对 311 原施工第 7 节和第 12/13 节：补齐或找到真实“错误定位 -> Host 修改 -> check passed”的 MCP
   证据，并覆盖不显式 check 直接 run；核对其约定的终态受影响回归，不能以成功链或投影测试代替。
2. 311 缺口应在其单独授权的恢复窗口处理，写回其记录，纠正归档和入口矛盾；不得算作 312 Gate。
   复用输入未失效的证据，不机械重做 A/B；312 待执行期间禁止实施其代码或 Gate。
3. 核对正式方案、当前源码、依赖/feature、测试组成及用户工作树变化；确认没有其它当前计划。
4. 用户授权后，前置通过才将本文件移动到当前并同步 54；各窗口授权独立，不自动跨窗口施工。

本文件不是重新打开 311 的授权，也不创建第二个活动施工槽。

## 2. 代码基线与所有权

生成时 HEAD：`acaffa8cb95bbb54d093bbf45835a59315c82da5`。保留既有 dirty/untracked 工作树，不作
exact-commit 声明；实际激活时记录受影响源码、测试和依赖身份，不以 HEAD 代替未提交源码身份。

| 范围 | 现有文件/入口 | 允许的修改 |
| --- | --- | --- |
| 业务观测 | `rust/crates/engine_runtime/src/project_observation.rs`、`project_runtime_session.rs`、`frame_loop.rs` | typed 值校验和已有 post-commit 采样；不新增 ECS 反射 |
| 项目 SDK | `rust/crates/project_game_sdk/src/lib.rs` | 复用 observe/ObservationWriter；只有证实接口缺口才改并补 consumer 验证 |
| Player | `rust/crates/runtime_player_winit/src/lib.rs` | 有界输入、断言、捕获、结果输出；可提取同 crate 私有实现文件 |
| 进程/交付 | `rust/crates/runtime_cli/src/lib.rs`、`main.rs`、`exported_player_verification.rs`、`bounded_child_process.rs` | 场景传递、指定产物运行、超时回收；复用既有进程 owner |
| 准备/派生 Host | `rust/crates/project_authoring_execution/src/game_project_compiler.rs`、`project_runtime_player_staging.rs`、`desktop_export.rs`、`lib.rs` | 冻结测试输入、准备和派生 Player 接线、交付身份 |
| Provider | `rust/crates/engine_tool_provider/src/lib.rs`、`mcp_stdio.rs` | 同一 Registry、playtest/observe、权限与结果投影 |
| 产品知识 | `ai-development-pack/skills/ai-first-game-engine/SKILL.md` | 条件化流程；不执行、不持状态 |
| 测试 | 上述 crate 的 lib tests；已有 Compiler interface、desktop_export_owner、headless_execution_owner、host_adapter_equivalence、mcp_process_smoke、skill_workflow_contract | 扩展既有夹具；必要时新增最小集成测试目标 |

当前 `runtime_player_winit` 默认 features 为空；`real-window` 启用 Winit/WGPU 和 engine_runtime/real-wgpu。
`runtime_cli/real-window` 转发该 feature。不能用默认编译或绿色 0 tests 宣称真实截图通过。
项目 fixture 使用源样例的临时副本或测试内小项目；不恢复 Tower 施工，不修改生产项目内容。
不新增 crate/daemon/Registry/观测 owner；SDK/ABI/schema 版本不为形式完整而统一升级。

## 3. 必要性与验证经济性自审

请求主张：一次有界试玩能证明业务条件，证据可按运行读取，指定导出 Player 可执行相同适用场景。
最小因果修改：连接现有输入/观测/捕获与 Player，补 typed 场景和断言，再接 Compiler/Provider。
明确排除：长驻会话、订阅、通用 DSL、全平台确定性、性能平台、安装态、Local CI、真实 Codex 配置。

| 窗口/Gate | 风险合同 | 最便宜的红灯测试 | 终态证据 | 估算/停止条件 |
| --- | --- | --- | --- | --- |
| A 合同 | 错误 path/type/time 被通过、状态混淆 | schema 与断言 owner 正反例 | typed contract 和失败诊断 | 60-100 分钟；需通用 DSL 则停 |
| B Player | 重复输入、提交前采样、无限等待 | 小型 Player 场景及超时/重复运行 | 实际业务状态和进程终态 | 90-140 分钟；需第二执行器则停 |
| C 交付/捕获 | 偷换 live 项目、证据串线、假视觉通过 | 指定导出产物正反例及窄 real-window smoke | 产物/运行/捕获身份一致 | 90-140 分钟；缺图形环境则保持未通过 |
| D 工具/终态 | 假 Ready、审批不等价、Skill 不一致 | Native/MCP 合同与真实 process 链 | 9 项真实默认工具与四类结果 | 90-140 分钟；安装/真实宿主需求则停 |

每个估算含证据和清理；2小时45分不再开始新实现/验证，3小时停止。若成本明显超估，先复核前提，
不扩 runner 或测试矩阵。复用既有观测、进程、Compiler 证据需记录消费的输入身份；仅失效部分重验。
命令目录为 `G:\gameEngin\rust`；仅执行当前授权窗口，失败先处理首因。

## 4. 窗口 A：场景与 Outcome 合同

状态：passed（2026-09-09；只授权 A，证据见第11节）。

冻结落点：`runtime_player_winit::semantic_outcome` 唯一拥有 JSON 场景与 Outcome DTO；Compiler 增加对已有
Player crate 的直接依赖（方向仍 Compiler -> Player -> Runtime），不经 CLI 复制类型。项目 manifest 的
`playtestScenario` 引用一个默认场景 JSON，场景内 `scenarioId` 是稳定身份；读取沿用 observationContract
的 manifest/source 路径机制，保留原始字节摘要、来源路径和 prepared lineage，不建立第二个资源 catalog。
`initialSceneId` 必须在同一 prepared 场景集合内，ObservationContract 同样来自该 prepared 输入。
模拟输入/断言使用一基 `simulationTick`；截图单独使用一基 `presentationFrame`，两种上限独立。
只支持已有 keyboard down/up 适配；拒绝重复按下、无对应按下的释放及未释放结尾。无项目种子消费接口，
本版 seed 一律拒绝。区间为闭区间，期限内任一已提交采样命中即满足，不能将呈现帧直接转换为 tick。
本窗只实现纯合同与聚合，不接执行循环；实际 tick 输入/提交后采样、session 证据绑定在 B/C 验证。

- 在已有依赖方向内冻结场景 schema 的唯一所有者与序列化位置，不在多 crate 复制 DTO。可使用现有
  Player 模块承载输入合同，Runtime 继续只拥有观测语义；若依赖图要求新架构 owner，先暂停修订方案。
- 冻结初始场景、目标、时间线、检查点/区间、typed 等值条件、捕获点、帧/时间限制及边界。
  明确模拟帧与呈现帧关系，不能把现有 frame_index 默认当作固定 tick；输入适配仅选择已支持类型。
- 场景通过现有资源/manifest 和冻结 SourceView 获取；记录路径/摘要，不新增测试资产库。
- Outcome 分 Technical/Gameplay/Visual/Delivery；未检查、不支持、未产出、失败不变成通过。
  UI 不代替业务事实；未知 path/type、越界帧、重复或非法顺序、未支持种子均有明确拒绝。
- 原始捕获是否必需、主观视觉待审核、必需证据缺失的聚合规则须通过负例冻结。

owner 局部红灯通过后，终态命令：

```powershell
cargo test -p runtime_player_winit --lib --locked
cargo test -p project_authoring_execution --test game_project_compiler_interface --locked
cargo fmt -p runtime_player_winit -p project_authoring_execution -- --check
```

若 A 实际修改 engine_runtime 观测 owner，增加 `cargo test -p engine_runtime project_observation --locked`，
并核实该筛选确实执行新增测试；否则复用既有观测证据。不改 SDK 时不跑无关 SDK suite。

## 5. 窗口 B：有界 Player 语义执行

状态：passed；前置 A passed。2026-09-09 21:49 CST 开始，终态证据见第12节；未跨入 C/D。

实施落点复核：Player默认循环本就每次推进一个fixed tick；本窗显式保持该受控调度，消费实际fixed_frame_count
和当次post-commit观测，不用呈现frame_index推算tick。零步只等待不重复输入/采样，批量多步不能冒充逐tick采样，
若检测到跳tick则明确unsupported。无需给Runtime新增永久逐步trace或第二执行器。
首条执行纵切为Windows headless；Windowed场景本窗明确unsupported，C接入同一判定器与实际窗口捕获后再通过。
CLI以同一Player子进程消费冻结场景文件，复用BoundedChildProcessRequest/Windows Job；父进程返回前核验回收，
父进程丢失时Job关闭杀子进程。库内合作超时不冒充对任意Rust阻塞的硬上限，硬上限由进程owner承担。
临时数据使用本窗独占外部TEMP子目录；只清理本窗可重建子项，不动A遗留空目录或旧证据。

- 复用 NativePlayerInputScript 和输入映射，不直接篡改业务状态来模拟玩家操作。定义边界帧输入一次、
  无输入帧状态、按键释放；多 fixed step 或零 fixed step 下不重复或伪造 post-commit 观测。
- 通过 SDK observe 发布少量业务值，按已冻结合同采样/断言；区间等待只观察，不重放 action。
- 至少一条“实际输入改变项目状态”的正例与故意错误期望负例；类型/合同违约不能被正常退出掩盖。
- 相同受控初始条件两轮语义结果一致，改变输入能改变对应结果；不承诺逐像素或跨平台确定性。
- 帧限制和真实时间 timeout 独立；复用子进程 owner，异常/超时确认回收，保留首因。
  主动取消不在本窗新增，保持 supportsCancellation=false；断连后最长存活受已传给执行 owner 的上限约束，
  必须验证没有失去所有权的无限进程。若现有 owner 无法满足，停在此边界处理，不宣称 lifecycle 通过。
- Summary 只含必要值；Off/未测试普通运行不新增全量 trace、截图或 JSON 热路径。

```powershell
cargo test -p runtime_player_winit --lib --locked
cargo test -p runtime_cli --locked
cargo fmt -p runtime_player_winit -p runtime_cli -- --check
```

如修改 frame_loop/session，增加对应 owner 定向测试，筛选名须在代码完成后验证为非零；D 覆盖时不重复计数。

## 6. 窗口 C：交付复验与捕获证据

状态：passed；前置 B passed。2026-09-09 22:28 CST开始，终态证据见第13节；未跨入D。

落点复核：沿用Player真实窗口循环和B判定器，测试时屏蔽外部输入；Surface重试不得重新推进同一呈现帧，
测试模式遇到失效明确失败，普通运行恢复语义不改。捕获只存声明帧，关联run/session/tick及图片摘要。
Compiler复用DeliveryRef与既有生成Player，指定交付复验不得重建live项目；报告读取校验引用及文件摘要。
本窗临时根为C:/Users/zenghaoran/AppData/Local/Temp/aife-312-c-20260909-2228，temp为可清理子项，evidence保留。
生成Player首轮长路径链接LNK1181，另建独占短路径role root C:/a312c-2228，后续TEMP/TMP改为其temp子目录；
不修改系统TEMP、共享Cargo target或生产配置。失败长路径fixture保留到首因证据落盘后再清理。
不启动production Editor、不改真实配置、不激活工具；真实窗口证据需实际项目模块/feature齐备后运行。

- 经现有 Compiler/派生 Host 接线消费同一场景合同；准备期间只消费冻结输入，不重读 live 源码。
- 指定 DeliveryRef 后仅执行指定 Player；测试在导出后破坏 live 项目，复验仍使用原产物而非重建替代。
- 场景摘要、run/session、帧、项目/产物/交付身份与捕获绑定；错误引用、不同 session、文件丢失或超限
  返回明确失败，不能回退最新运行。observe 后续读取不需要常驻 Player。
- 真实窗口捕获指定帧，验证实际图片尺寸、非空内容及输入触发的可识别状态；截图存在本身不是视觉质量通过。
  Headless 请求不支持的捕获要报告 unsupported；不得跳过真实窗口证据却宣布 Visual 全部通过。
- 指标来源/单位、采样窗口明确；只有已支持的指标能测，无阈值不报性能达标。不新建完整 benchmark。
- 在已有 Player lib 测试中添加 `semantic_outcome_window_capture` 精确测试名，feature-gated 且明确 ignored
  为本地窗口测试，使用项目运行组成而非空窗口；不新造大型 runner。

```powershell
cargo test -p project_authoring_execution --lib --test game_project_compiler_interface --test desktop_export_owner --locked
cargo test -p runtime_player_winit --lib --locked
cargo test -p runtime_cli --locked
cargo test -p runtime_player_winit --features real-window --lib semantic_outcome_window_capture --locked -- --ignored --nocapture
cargo fmt -p project_authoring_execution -p runtime_player_winit -p runtime_cli -- --check
```

窗口测试名为本窗计划新增，执行前 `--list --ignored` 必须列出该测试，正式运行必须实际执行且有捕获证据。
先验证 feature 与项目组成完整；缺 GPU/桌面条件记录阻塞，不启动 production Editor 来绕过。窗口关闭和图像
证据保全计入本窗时间；不把图像主观审美纳入自动通过。

## 7. 窗口 D：Provider、Skill 与终态

状态：passed；终态证据见第14节。2026-09-09 23:17 CST开始；原定次日02:02停止新实现/验证，02:17硬停止。
本窗独占外部root为C:/a312d-2317，temp为可清理子项，evidence保留；不动C证据及共享Cargo target。
复核现有Compiler接口可复用，Provider内保存交付绑定的compiler/可选冻结场景和run报告；不是新Registry服务。
指定交付/observe不刷新live项目，Grant绑定保留的project/revision；未知引用先拒绝，observe没有路径输入。
新工具初始Unavailable，owner与adapter合同通过后才转Ready并运行最终MCP链。按skill-creator只更新产品Skill
的条件化知识和行为验证，不增加脚本或状态。C实窗源/feature/夹具不改则复用，不重跑GPU。

D首轮实测补充：Puzzle actual=false、Shooter计数actual=0；进程正常回收，Technical passed但Gameplay failed。
源码核对project_runtime_native_adapter::invoke_rule仍传NULL，generated_runtime_glue::with_session拒绝空handle。
仅扩大到该Runtime adapter局部修复及其owner测试，复用既有session lease，不升级ABI/SDK/schema。
先让fake ABI拒绝空handle得到红灯，再验证registration绑定/失效和两个真实generated项目；不通过修改期望绕过。
追加cargo test -p engine_runtime project_runtime_native_adapter --locked，D终态覆盖其直接Player/Compiler consumer。
C的纯Rust TestModule捕获链不消费该Native adapter；逐项核对后可复用对应图片，不据此证明native capture新组合。

- 单一 Registry 定义 playtest 为进程副作用、observe 为读取已产生证据；复用 Host/Grant、ToolResult。
  审批拒绝不能启动 Player；observe 不能变成任意路径读取。新工具先保持非 Ready，终态通过后才资格化。
- Native/MCP schema、输入拒绝、权限、诊断、run/evidence 语义相同；默认精确 9 项，不增加机械工具。
- 扩展现有 MCP smoke：错误期望的 playtest 失败 -> Host 修复测试场景 -> playtest 通过 -> observe
  指向同一运行 -> 指定导出复验。保留普通 run/build 不要求 playtest 的可选路径。
- observe 不增加执行次数，证据来自指定 run；未知/失效引用拒绝，不自动重跑。
- 一个小项目覆盖真实输入正反例，另一小项目验证合同不硬编码玩法；不扩完整游戏内容矩阵。
- 同步产品 Skill 与语义合同测试，断言“check 可选、playtest 执行、observe 只读、deferred 不冒充 Ready”，
  不能再以只出现工具名字的静态测试代替具体流程约束。按 Skill 编辑要求加载 skill-creator 后再改产品 Skill。

终态只运行受影响闭包，不作为开发内循环：

```powershell
cargo test -p runtime_player_winit -p runtime_cli -p project_authoring_execution -p engine_tool_provider --locked
cargo check -p editor_core --locked
cargo fmt -p runtime_player_winit -p runtime_cli -p project_authoring_execution -p engine_tool_provider -- --check
```

若 engine_runtime/SDK 有实际修改，增加其受影响 owner 和直接 consumer 测试并记明失效依据，不无条件全 workspace。
C 的 real-window 证据若消费的源/feature/夹具未变则复用；有变只重验对应捕获，不重复全部旧 Gate。
第一次命令包含 process smoke，必须检查实际执行数量、ignored 清单与首因；不能用绿色 0 tests 或隐藏失败归档。

## 8. 证据、清理与完成标准

每窗记录：实际源码/测试/依赖身份，命令、编译与测试耗时、passed/failed/ignored、失败首因与修复、
复用证据及失效条件、外部状态和剩余风险。dirty 开发证据不得冒充 exact-commit/安装态资格。

进程和 fixture 只使用 run-owned 临时根，激活时解析 literal 路径并登记所有权；Windows 使用短 build root。
不清理共享 Cargo target、样例、其它任务状态或旧 254 证据。只对确认终止且保留必要证据的本次可重建子项
按 construction Skill 检查路径/reparse/进程后清理；宿主拒绝记 retained_by_host_policy，不无限重试。

A-D 全通过、整体受影响回归通过才可完成。生成实际日期的 312 阶段完成记录，更新方案、README、文档地图、
阅读顺序、49、54、施工/待执行 README、阶段记录入口，移动到已完成并验证所有当前路径引用。
过程状态保留且标注历史；最终结论必须唯一，不允许只新增“已完成”段落而留下冲突当前状态。
文档落盘不代表测试通过，目录移动不代表资格完成，7->9 工具清单也不等于语义闭环。

## 9. 施工文档自审

- 与正式 B 一致：有界执行、只读证据、四类 Outcome、同产物复验；不增加新工具族或服务。
- 生成时311前置未闭合，因此曾放待执行；现经单独授权完成前置，激活记录见第10节。
- 已核对现有 crate/入口与 real-window feature；计划新增窗口测试已标明，不冒称现有绿色证据。
- 每 Gate 对应可失败合同，有阶段测试、终态闭包、证据复用和成本停止条件。
- 未收到新的外部审查文档；正式方案无需为生成本文而修改技术范围，仅同步文档状态。
- 自审结果：通过；激活复核：通过；A-D passed。各窗口独立授权及证据见第11-14节；C/D接线及Runtime首因修复已回填正式方案第5.2/5.3节。最终默认9项，无额外工具族或安装态声明。

## 10. 激活复核记录（2026-09-09）

用户已明确授权“先闭合311剩余证据缺口，授权激活312”。311最终证据位于
`../已完成/311-Game-Project-Compiler-Incremental-Prepare-Run-v1施工文档.md`第14节：MCP错误定位/Host修复/check
通过及直接run，终态两crate107项测试、editor_core编译及格式检查全部通过；311归档后才移动本文件。

基线HEAD仍为acaffa8cb95bbb54d093bbf45835a59315c82da5，dirty/untracked保留。唯一新增相关接口漂移为
Provider的可选sourceLocation/ compilerDiagnostics以及源码错误局部恢复提示，已有Native/MCP等价测试；
不推翻312的ToolResult复用方向。Runtime/SDK/Player的312目标接口与feature路径仍存在，未被本次修复修改。
第2节文件、crate与real-window feature已核对；计划新增semantic_outcome_window_capture仍为未来工作，
不把测试名不存在当通过，不在激活时运行312 Gate。Cargo.lock和toolchain身份继承311第14节。

正式方案B无需技术范围变更；前置证据闭合已同步，当前槽仅312，待执行无计划。激活自审通过。
以上为激活当轮历史事实；后续 A 已获得独立授权并完成，见第11节。不安装、不改真实配置、不升级为9项Ready。

## 11. 窗口 A 实施与终态证据（2026-09-09）

用户授权“开始施工312 窗口 A：场景与 Outcome 合同”。21:14 CST 开始，21:37完成；约23分钟，低于60-100分钟估算，
未跨窗口。HEAD仍为 `acaffa8cb95bbb54d093bbf45835a59315c82da5`，dirty/untracked开发证据，不是exact-commit或安装态资格。

### 11.1 已实现范围

- Player 新增同 crate `semantic_outcome.rs`：单一 serde schema、双时钟边界、输入顺序与键状态、typed 等值、
  捕获请求和四类 Outcome。全部必需项由场景驱动聚合；缺证据不补成功，主观视觉保留 pending-review。
- Compiler 新增内部 `game_project_compiler/playtest.rs`，从 prepared SourceView 解析 manifest 默认场景引用，
  校验初始场景/目标/业务合同，保留来源路径、原始 SHA256、原有 lineage。错误含源码路径、字段或JSON行列。
- direct dependency 仅补已有方向 Compiler -> Player；Cargo.lock只增加该crate依赖边，无版本/工具链升级。
- 不改 Runtime observation、SDK、CLI执行、Provider、真实窗口。7项 Ready 不变；B/C仍须证明真实输入、
  post-commit采样、超时回收及证据身份。纯 matches_sample/聚合不是完整执行器。

冻结字段、类型和限额见正式方案第4.1节；新增测试覆盖四类标量、闭区间/检查点、unknown path/type/target、
seed、越界/重复/无序、非法键状态、缺必需截图/断言、未检查Delivery和主观审核。Compiler负例覆盖缺引用、
越界路径、JSON错误、超大输入、未知初始场景，正例覆盖原始digest与live四类输入变坏后仍消费旧lease。

### 11.2 失败与修复

1. 首次Player定向命令没有进入测试：`lib.rs`既有 `player_wgpu_backends_uses_gl_only_for_android_x86_64`
   在默认feature引用 `wgpu` / `real_window`，6个E0433。只补该测试 `cfg(feature="real-window")`；
   真实窗口分支仍保留，未伪称GPU通过。
2. 随后 owner 红灯实际运行1项：`semantic_scenario_rejects_unknown_observation_path` 对 `hud.text` 的校验
   `unwrap_err()`收到 `Ok(())`。实现 ObservationContract 校验后定向9项通过，首因可由该负例重现。
3. 收尾将 overall 从人工Gameplay/Visual开关收紧为场景推导；更新Player后复验Compiler，出现18 passed / 1 failed：
   `check_real_cargo_uses_lease_reports_rust_span_and_does_not_edit_sources` 的有效R1得到生成glue第46行E0599，
   `no method named erase found for type u32`，恰是同 suite 的故意错误项目内容。保留此失败，不以此前19绿覆盖。
   夹具同标签、PID、时间戳配合 `create_dir_all` 存在别名风险；固定相同时间戳的最小测试真实复现：
   两个路径相同，第二次覆盖第一次manifest。原失败未捕获两个源目录身份，故该机制复现不冒称原现场路径已被记录。
   仅测试Fixture增加进程内序号并改独占create_dir，固定时钟负例转绿；不改Compiler产品或恢复311施工。

### 11.3 实际命令与结果

工作目录 `G:\gameEngin\rust`，默认features、locked、test profile。

| 命令 | 最终结果 | 编译/准备与测试耗时 |
| --- | --- | --- |
| cargo test -p runtime_player_winit --lib --locked | 36 passed，0 failed，0 ignored | 1.98s / 0.02s |
| cargo test -p project_authoring_execution --test game_project_compiler_interface --locked | 20 passed，0 failed，0 ignored | 0.20s / 21.44s（含真实Cargo子进程） |
| cargo fmt -p runtime_player_winit -p project_authoring_execution -- --check | exit 0 | <1s |

最终56项，不叠加定向9/4/1项或较早终态36/19项来重复计数。测试Fixture修复只使Compiler测试证据失效，
Player源码/输入未变，复用最终36项，不机械重跑。Runtime观测owner、SDK未变，复用其既有合同与本轮Player
consumer验证，不跑无关owner suite。real-window、整个workspace、Local CI、Editor/安装态均不属于A主张。
编译沿用3个既有engine_runtime dead_code警告；不掩盖为零警告，也不扩无关修复。

### 11.4 输入身份（SHA256）

路径均相对 `rust/`，包括untracked新增源码，不能用git diff替代这些身份。

| 文件 | SHA256 |
| --- | --- |
| crates/runtime_player_winit/src/lib.rs | FECA82084955C8AD0F6CEF6C4DB6D0CB143FF6EE468047BD304775168CB0924E |
| crates/runtime_player_winit/src/semantic_outcome.rs | 9F1964DCC071A0B42FFF7F9BB0EC1A8D30922F30E3ED845DDDA1135C7CF2C908 |
| crates/project_authoring_execution/src/game_project_compiler.rs | B0C7C6E3D03016304AD3D59E8E136F4662B69CFA9492C69F9B399E608D921007 |
| crates/project_authoring_execution/src/game_project_compiler/playtest.rs | 1818A60E795FDF3D772045432E347596E436141F669B3B680663A69418CAEE16 |
| crates/project_authoring_execution/src/lib.rs | 74518527FC8CD50295D89AD4CE200E31FBFB16B4C7CC5759EA502363705FFDA4 |
| crates/project_authoring_execution/tests/game_project_compiler_interface.rs | 75FC5A17AF2CE08B278E532196A1576807C1A0502BF346D8F3369C52623D7C02 |
| crates/project_authoring_execution/Cargo.toml | 2973864D70270E62BCDA882BBE9AE228707BECA024B6DB89D2188777349E09ED |
| Cargo.lock | 1645FEAA854AF924DE4B9DE401B3F723E6F70CB134AB2D2A69E21A4D17C88835 |
| rust-toolchain.toml（未改） | A66423362263724F4C028788AD3B66826A86521E5E6456CF58D4E076B5886FCD |
| crates/engine_runtime/src/project_observation.rs（本窗未改） | B9D2F208971FEF685A6AB61FA438612B64B1AA1AA0EAA0E26188B3F1DB4EAC6A |

### 11.5 清理与边界

所有命令/测试及Cargo子进程已终止；本次Compiler fixture与check scratch经测试自身删除，终态扫描无残留。
Player既有测试留下的16个role root，仅清理其14个runtime-package与2个reports子目录；逐个literal路径验证
strict-child、所有祖先和后代无reparse、相关进程已退出。原生Remove-Item被host拒绝；按construction Skill
允许的同目标fallback重新安全检查后Directory.Delete，16项均removed，空父role root保留。未清理共享target、
旧311 sentinel、旧254证据或任何样例；临时包/PNG可由原测试重建，不是窗口C视觉证据。

清理目标清单（以下相对 `C:\Users\zenghaoran\AppData\Local\Temp\`，全部 removed）：

```text
runtime-player-winit-aui-present-cache-1788960466611442300\runtime-package
runtime-player-winit-aui-present-cache-1788960558316942800\runtime-package
runtime-player-winit-feature-disabled-1788960466614292800\runtime-package
runtime-player-winit-feature-disabled-1788960558318088200\runtime-package
runtime-player-winit-feature-disabled-screenshot-1788960466614600000\runtime-package
runtime-player-winit-feature-disabled-screenshot-1788960558318447900\runtime-package
runtime-player-winit-headless-one-frame-1788960466615464300\runtime-package
runtime-player-winit-headless-one-frame-1788960558318932500\runtime-package
runtime-player-winit-png-writer-1788960466617908300\reports
runtime-player-winit-png-writer-1788960558322133700\reports
runtime-player-winit-project-runtime-session-1788960466618557300\runtime-package
runtime-player-winit-project-runtime-session-1788960558322264000\runtime-package
runtime-player-winit-runtime-package-input-1788960466615491500\runtime-package
runtime-player-winit-runtime-package-input-1788960558318999800\runtime-package
runtime-player-winit-sprite-texture-collection-1788960466618658900\runtime-package
runtime-player-winit-sprite-texture-collection-1788960558322394000\runtime-package
```

以上为 A 收尾时的历史结论：A passed；当时 B/C/D not_started。之后 B 获独立授权并完成，见第12节。

## 12. 窗口 B 实施与终态证据（2026-09-09）

用户授权“开始施工窗口 B：有界 Player 语义执行”。21:49 CST 开始；本窗完成时间与清理结果见12.5。
HEAD仍为 `acaffa8cb95bbb54d093bbf45835a59315c82da5`，以下为dirty/untracked源码开发证据，不是安装态、
exact-commit或发布资格。只改Player/CLI产品与相关测试，未改Runtime/SDK产品、Compiler、Provider或真实配置。

### 12.1 已实现与实际边界

- Player新增同crate私有semantic_playtest模块；复用原headless循环、输入映射和Runtime post-commit观测。
  初始化项目Runtime前拒绝无效场景，受控每次一fixed tick；读取真实计数，零步去重、多步跳跃拒绝。
- 只保留声明断言的typed实际/预期、首次命中或最后检查的tick/presentation frame、session/contract与诊断。
  相同初始状态两轮一致；输入按下/释放只各一次，按住两tick实际产生两次响应，移除输入或错误期望失败。
  等待区间命中、提交后状态、旧snapshot/session/contract、缺观察、合同违约及项目回调panic均有负例。
- CLI新增playtest命令和有界API，同一可执行程序内部worker消费场景，复用原Windows Job进程owner，
  不新增runner/crate/daemon。运行目录独占新建，request/player/result JSON只在显式测试路径产生；
  缺报告、错误run/scenario、非零退出伪通过、回收未确认均不成功，stdout/stderr分别上限16KiB，报告读取4MiB。
- 真实子进程测试覆盖CLI公开入口、worker、无效初始场景、空报告、非零退出、硬超时，以及强制结束父级owner；
  最终直接查询夹具记录的worker/descendant PID退出状态，未以报告字段或“文件未出现”单独代替清理证明。
- 无断言空场景只得Technical passed，Gameplay/Visual为not-checked。Headless必需捕获unsupported；
  Windowed、指定Delivery与generated glue接线在C。SDK只加Player dev-dependency并在夹具调用注册observe；
  不把小测试桥冒称实际导出项目组成。ordinary运行没有semantic_playtest字段或新增逐帧JSON。
- 库内超时合作式；硬边界必须经CLI有界owner，原始内部worker直接启动没有该资格。主动取消未新增，
  工具supportsCancellation=false不变。当前runId是独占输出目录标识，完整冻结场景/交付证据身份留C。

### 12.2 红灯、首因与修复

1. owner最小红灯真实失败：`Zero-step frame must not repeat a committed sample`。修复以实际fixed计数判定
   新提交、重复和跳跃；不拿presentation index伪造tick。
2. 新夹具首次加载失败：rule缺irHash，补齐后artifactId不满足`rule-artifact:<ruleId>:<irHash>`；
   修正测试包，不改Runtime规则装配。随后期望1实际2，确认既有action_pressed表示持续按住，按两tick改期望2。
3. 首次完整CLI回归：lib25与进程owner8通过，exported suite为7 passed/1 failed；
   semantic_playtest_missing_report_nonzero_and_timeout_fail_closed的fixture.nonzero预期Failed，实际Timeout。
   该夹具预算500ms，PID12464，elapsed1811ms，stdout/stderr为空，cleanupConfirmed=true，Technical仍Failed。
   证据仅证明超时前未得到普通退出，不能断言OS调度或杀毒软件为唯一根因。分类为测试启动预算不足；
   普通退出预算改5s，故意阻塞预算2s（夹具睡30s），总等待断言8s，并直接核查两个owned PID已退出。
   没有放宽产品timeout上限或把Timeout改成成功。局部3项转绿后，完整CLI41项重新通过。

### 12.3 最终命令与结果

命令目录 `G:\gameEngin\rust`，默认features、locked、test profile；独占TEMP/TMP见12.5。

| 命令 | 最终结果 | 编译/准备与测试耗时 |
| --- | --- | --- |
| cargo test -p runtime_player_winit --lib --locked | 41 passed，0 failed，0 ignored | 3.40s / 0.05s |
| cargo test -p runtime_cli --locked | 41 passed，0 failed，0 ignored（lib25 + bounded8 + exported8） | 0.50s / 0.05s + 1.91s + 2.30s；命令9.09s |
| cargo fmt -p runtime_player_winit -p runtime_cli -- --check | exit 0 | 0.46s |

最终82项，不叠加局部5/3项、早期CLI24项或失败终态。修复只改CLI测试夹具，Player41项输入未变，故复用，
未机械重跑。Runtime/SDK产品与Compiler合同读取未改，不重跑其owner；实际SDK consumer由Player测试覆盖。
CLI main/fixture/doc-test的0 tests仅为构建目标，不当独立测试通过数。沿用3个既有Runtime dead_code warning。
不运行real-window、全workspace、Local CI、真实Editor/安装态或性能Gate，因为不属于本窗主张。

### 12.4 输入身份（SHA256）

路径相对rust/；A的semantic_outcome.rs仍为9F1964DC…2C908，完整摘要见11.4。工具链未改。

| 文件 | SHA256 |
| --- | --- |
| crates/runtime_player_winit/src/lib.rs | 0415458D80D9DDBB108B2801914B3C58D2C1379A48ADED97433E399F1D0B20AC |
| crates/runtime_player_winit/src/semantic_playtest.rs | D313FC33F9F6A44D534790B1A272ECAE82742D2192A608CCC59588A0D98781C8 |
| crates/runtime_player_winit/Cargo.toml | 65927F84738A77F42090972419E4DD279F57FAAF73076508D5D4F0E677C6BC5C |
| crates/runtime_cli/src/lib.rs | F81E2A3F2D1621F35568760AFBC8F5731139218DD53203B0B794FA498325A0FB |
| crates/runtime_cli/src/semantic_playtest.rs | 894FD05D6338B7925843188C0F72E73328DEA209833A5E4745739802DE5F5F67 |
| crates/runtime_cli/src/bin/bounded_output_fixture.rs | 71031E63AF30A317A393C176D2A87AA96B56426D3587B2C1A7E5385F1950984C |
| crates/runtime_cli/tests/exported_player_process_verification.rs | 957C946A9CB1762D968133551FC8312971CEDBAD274B5CF10D094E5A917188BC |
| crates/runtime_cli/src/bounded_child_process.rs（未改） | 8CA14E43196885CA7D0F4B8E35A5D4201E45EE22D8A8C19FFD483AD311DEDB45 |
| Cargo.lock（只增SDK dev依赖边，无版本更新） | 4E64DE068B6E5FE66CF89C5BE49782FDEE7B1F51677DEDF35EBFB157FED9A2BE |

### 12.5 清理与当前结论

22:20 CST收尾，约31分钟，低于90-140分钟估算和三小时上限。所有测试及owned子进程已退出。
主体测试TEMP/TMP为 `C:\Users\zenghaoran\AppData\Local\Temp\aife-312-b-20260909-2149\temp`。
最早一次CLI lib24项命令漏设独占TEMP，产生下表13个测试role root，后续命令均已修正；不是产品缺陷。
清理前先保全12.2首因及12.3/12.4身份，验证literal strict-child、祖先/后代无reparse和相关进程退出。
原生Remove-Item整条命令被host拒绝；重新验证后按construction Skill仅对已确认目录使用Directory.Delete，
18个目录全部removed。9个直属文件未使用目录fallback，记retained_by_host_policy，不无限尝试绕过。
所有role父目录保留；共享Cargo target、A历史空目录、311/254证据、样例与production/config均未清理或修改。

下表根均相对 `C:\Users\zenghaoran\AppData\Local\Temp\`；删除项为可重建测试包/报告，不是视觉资格证据。

| 本窗role root | removed子目录 | retained_by_host_policy直属文件 |
| --- | --- | --- |
| aife-312-b-20260909-2149 | temp | 无 |
| exported-player-verification-missing-game-exe-1788962789778908700 | Build | 无 |
| exported-player-verification-release-manifest-entrypoint-1788962789778776800 | ComplexShooter | 无 |
| runtime-cli-native-player-exe-package-1788962789779355300 | bin | 无 |
| runtime-cli-native-player-headless-report-1788962789779019600 | reports、runtime-package | 无 |
| runtime-cli-native-player-missing-package-1788962789779279900 | reports | 无 |
| runtime-cli-native-player-windowed-report-conversion-1788962789780101200 | runtime-package | 无 |
| runtime-cli-native-player-windowed-required-1788962789779326300 | reports、runtime-package | 无 |
| runtime-cli-one-frame-1788962789812466500 | reports、runtime-package | 无 |
| runtime-cli-packaged-entrypoint-desktop-dev-1788962789780515200 | data | Game.exe、package-manifest.json |
| runtime-cli-packaged-entrypoint-mismatch-1788962789780133900 | data | ComplexShooter.exe、Other.exe、package-manifest.json |
| runtime-cli-packaged-entrypoint-report-off-1788962789780566600 | data | ComplexShooter.exe、package-manifest.json |
| runtime-cli-packaged-entrypoint-resolve-1788962789780527900 | data | ComplexShooter.exe、package-manifest.json |
| runtime-cli-writes-report-1788962789812778900 | reports、runtime-package | 无 |

以上为B结束时的历史状态：当时A/B passed，C/D not_started。最终状态以第14节为准。

## 13. 窗口 C 实施与终态证据（2026-09-09）

用户授权“开始施工：窗口 C：交付复验与捕获证据”。22:28 CST开始；本节记录源码开发和实际Windows测试，
HEAD继承acaffa8cb95bbb54d093bbf45835a59315c82da5，保留dirty/untracked，不作exact-commit或安装态资格声明。

### 13.1 实现与边界

- Compiler新增playtest、playtest_delivery、observe_playtest，复用原build/DeliveryRef与冻结场景。
  导出后破坏live Rust和场景，指定Game.exe仍执行原冻结场景并观测test.frame=3，Gameplay/Delivery passed。
  错误Player、改变运行包、不同revision/场景lineage均拒绝；新建交付仍执行原live binding检查。
- CLI复用既有有界子进程与Job owner；报告绑定run/scenario/Player/运行包摘要，执行前后检查产物未变。
  只读证据接口核对报告seal、冻结请求、run及capture身份；重复observe不增加进程执行。
  错误session/frame/尺寸、引用过期、文件缺失/替换/超限、场景请求改变均有拒绝测试。
- Windowed复用B判定器和现有真实窗口循环，固定一步推进、提交后采样，屏蔽外部输入；Surface故障直接失败。
  捕获绑定session/tick/presentation frame，限制4 Mi pixels/32 MiB；未请求时不新增普通运行截图热路径。
- 实窗测试使用已链接TestModule/SDK observe、真实cooked纹理和sprite；640x480 Vulkan呈现4帧，
  捕获1/4帧。PNG解码验证尺寸、红色像素超过20及横向质心位移超过5px，证明输入引起图形变化。
  捕获存在只证明原始证据产出，主观视觉审核仍pending-review。Headless捕获仍unsupported。
- Windows Dev交付布局已验证；Windows Release/其它平台不在本窗资格内。直接库窗口调用为合作有界，
  硬超时/进程回收由CLI owner及其进程测试证明，不混称直接窗口测试覆盖任意阻塞回收。
- Provider/D未施工；未来Provider必须从自身registry解析run引用，不能将底层文件路径API暴露成任意读取。
  SDK/Runtime产品源码、生产Editor、真实配置、安装状态未修改；默认仍7项Ready。

### 13.2 终态验证

目录G:/gameEngin/rust；最后默认回归TEMP/TMP仅在本次子进程设置为C:/a312c-2228/temp。
以下为去重终态162项，不叠加定向重跑、A/B历史或空测试目标。

| 命令 | 结果 | 编译/准备与测试耗时 |
| --- | --- | --- |
| cargo test -p runtime_player_winit --lib --locked | 41 passed，0 failed/ignored | 5.35s / 0.05s |
| cargo test -p runtime_cli --locked | 41 passed：lib25、bounded8、exported8；0 failed/ignored | 35.61s（含共享target锁等待）/ 0.07+1.91+4.39s |
| cargo test -p project_authoring_execution --lib --test game_project_compiler_interface --test desktop_export_owner --locked | 79 passed：56+3+20；0 failed/ignored | 30.31s / 17.40+102.07+20.64s |
| cargo test -p runtime_player_winit --features real-window --lib semantic_outcome_window_capture --locked -- --ignored --nocapture | 实际1 passed，0 failed/ignored，44 filtered | 35.21s / 1.18s |
| cargo fmt -p project_authoring_execution -p runtime_player_winit -p runtime_cli -- --check | exit 0 | 1.20s |

实窗执行前--list --ignored精确列出1项，初次feature编译68s；不是绿色0 tests。
AIFE_SEMANTIC_CAPTURE_EVIDENCE指向下述window-final。Player默认回归后只有feature测试内容调整及格式整理，
默认消费者输入不变，复用41项结果；最后格式检查后无Rust修改，不因文档同步重跑。
现存Runtime三条dead_code警告未扩大处理。未跑全workspace、D/Provider、Editor/安装态、Local CI、Android或Tower，
因为未修改其owner且没有对应授权/资格主张。CLI身份测试中的合成文件只证明摘要绑定，不冒充视觉证据。

### 13.3 失败与修复

1. 新公共签名首次编译缺Arc导入，改为std::sync::Arc；CLI中间接线曾先引用尚未完成的摘要helper，补齐后编译通过。
2. 首次截图测试报No visible project sprite；PNG合法但全黑。夹具缺少Sprite所需纹理引用/资产，补真实cooked纹理，
   没有修改Renderer/Runtime。失败window-capture保留；v2通过后增加质心/指标/有界读取检查，window-final重验通过。
3. desktop夹具首次引用私有player_artifact_build_root编译失败，改为复用本地已知路径变量。
4. 在损坏live Rust后尝试新建交付得到game_project_compiler.execution_project_drifted，是既有正确保护；
   改测试次序为先导出再破坏live，再用playtest_delivery复验，没有绕过build保护。
5. 临时项目observe源码未满足rustfmt，validate_project_runtime_format exit1。用stdin最小格式复现确定BTreeMap
   元组需要多行并修正夹具。初始guard已删该次fixture，未保留完整原始日志，仅有阶段输出和最小复现，不虚称全量留存。
6. 长路径派生构建validate_project_runtime_tests_compile exit101，LINK LNK1181无法打开依赖.obj
   （serde_core/zmij等）。改用独占短TEMP后同一功能测试101.22s通过，最终完整desktop suite再次通过。
   记录为路径敏感环境失败及短路径消除现象，不声称已对链接器根因作完整取证；原始构建报告已另存。
   测试guard改为panic时保留失败fixture。未改工具链、系统TEMP或共享target配置。

### 13.4 保留证据与测量含义

证据根：C:/Users/zenghaoran/AppData/Local/Temp/aife-312-c-20260909-2228/evidence。
保留window-capture（初次失败）、window-capture-v2（中间通过）、window-final（最终）及long-path-build-failure.json。
最终图片均640x480、1,229,438字节；session=test.playtest.session，截图1/4分别绑定tick1/4。
合同摘要sha256:47eaa1f0c3d188a25b69bbe12ad9b22df90f8ffadabcd6c26192f828eee3d088。
业务断言包括tick1 presses=1、tick4 held+released=2和commits=4，均passed。

| 相对证据路径 | SHA256 |
| --- | --- |
| window-final/capture-1.png | 2587B3DAC6B13A26AD2771CD616400B76DE265DF8DA542B5FE8DF231BEEC2193 |
| window-final/capture-4.png | AB25DCE85FCBAC2CBEADBE770B4DF0F496CEA595D5847035A3AED7B321CB1645 |
| window-final/player.json | A8BA306A6C50EB0EA92A3A48032BB861EA32E7A09E81B9B9126B7B4FB6522D1A |
| long-path-build-failure.json | 63DBE6BA346F4C8606C348F634A306914F5972C311410094E93C016A44FCF292 |

既有CPU性能采样warmup=0、requested/observed=4，单位ms：frame mean=63.53775、p95/p99=126.9048，
update mean=0.6459、renderSubmit mean=0.928025、presentWait mean=0.219875。包含截图读回/PNG开销，
不是正常游戏帧率、GPU计时或性能达标证据；无性能阈值。noAUI警告符合此无HUD夹具，不代表HUD资格。

### 13.5 源码与依赖身份

以下路径相对rust/，SHA256为最后格式检查后的实际源码；包含测试身份，不用HEAD替代未提交文件。

| 路径 | SHA256 |
| --- | --- |
| crates/runtime_player_winit/src/lib.rs | 399820EFB126EB701AC2F4AA8CB3F282124B780B431440B643A0EF499524AB5E |
| crates/runtime_player_winit/src/semantic_playtest.rs | 5CEC5728C69AB52BE81B71EB94A262BA996C2F352C10EDFBF6BDF331892A4A6E |
| crates/runtime_player_winit/Cargo.toml | 833FD6D3CD2FF810DE5B26B32183C27745589D6904E736A8C4AB6CBB2C80682A |
| crates/runtime_cli/src/lib.rs | B298D605BF7B5CBBF6204886C468A24FD9B297551809F33C52E6723C33EE6648 |
| crates/runtime_cli/src/semantic_playtest.rs | BB9C579CB80962BF99C2C0193CDD7ABB149295F741557CC6E6145FF6DC938035 |
| crates/runtime_cli/Cargo.toml | 566257D03F5D38E3C3F7993EF65D6026A87BD5DFD54BBBAE65034D2BAB86CDCF |
| crates/runtime_cli/tests/exported_player_process_verification.rs | A1246A0FC89C5FBDDDC5DE47DA09E8D485D408BC786B0B738B89ADFD95230A51 |
| crates/project_authoring_execution/src/game_project_compiler.rs | 7DC85BE8F09BAADBA1C5C12C6A92D23CD2506EBAA972EC8BCEEA06D756034CC1 |
| crates/project_authoring_execution/src/game_project_compiler/playtest.rs | 6F8656674AAB0CF7ED304B49F004384C68F19272AD768B07CF3E5F9CBB0B0F7F |
| crates/project_authoring_execution/src/lib.rs | 19B6A0ACE31E1943DCEA2E5E01E7E2910033C72527214EF78BA88024DEACCE60 |
| crates/project_authoring_execution/tests/desktop_export_owner.rs | 40A8F53884BDDDE3851F24062ADBAE799F78523FCEDA362C1C06E8ABEFA46BCF |
| Cargo.lock | A8486AB4CEB2DB06BD346C7B7B68ECBFB4D318314574864B49CA0ABE28FEBB09 |

仅增加CLI sha2=0.10与Player dev png=0.17的已有版本依赖边，无版本/工具链升级。
以上输入改变时仅失效消费它的测试；D若未改变实窗源/feature/夹具，可复用独立实窗证据。

### 13.6 清理与当前结论

清理前已核验两个literal路径为各自role root的严格子项，目标/祖先/后代无reparse，相关cargo/rustc/test/Player
进程无存活，四份保留证据摘要与第13.4节一致。两次Remove-Item均被宿主在启动前拒绝，按施工skill允许的
同路径IO.Directory.Delete后备执行并核对Test-Path=false，均removed：

- C:/Users/zenghaoran/AppData/Local/Temp/aife-312-c-20260909-2228/temp
- C:/a312c-2228/temp

仅删除本窗可重建临时fixture/构建文件（未送回收站，可通过测试重建）；evidence、role root、共享target、
B遗留文件及用户工作树全部保留。未更改持久系统TEMP/生产配置，无需回滚安装状态。
23:12 CST完成证据与入口收尾，约44分钟，低于90-140分钟估算和3小时上限；没有跨入D。

以上为C结束时的历史结论：当时A-C passed，D not_started，312保持当前，默认7项；不覆盖第14节终态。

## 14. 窗口 D 实施与终态证据（2026-09-10）

用户授权“窗口 D：Provider、Skill 与终态”。2026-09-09 23:17 CST开始，次日完成归档。
HEAD继承acaffa8cb95bbb54d093bbf45835a59315c82da5，dirty/untracked保留；只作源码集成声明。

### 14.1 实现与可失败合同

- 同一Provider definitions/dispatch内部23项，Native/MCP默认精确9项Ready：inspect/check/mutate/rollback/
  runtime_run/build/runtime_playtest/runtime_observe/delivery_verify（均使用既有engine_完整名称）。
  仅新增playtest/observe两个深工具，无新crate、schema、ABI、daemon或Registry服务。
- playtest接受空对象或deliveryRef，observe只接受runRef；拒绝未知字段、路径、null/空引用及运行参数注入。
  普通run/build无需场景。指定交付消费原Compiler、产物和可选冻结场景；无场景则拒绝，不读取live补齐。
  observe和指定交付不刷新live源，Grant绑定保留的项目/revision；项目rebind清除引用，另一Provider不接受旧引用。
- playtest必需Outcome未通过则Tool Failed，保留四类结果及纠正诊断；observe成功读取仍Completed，
  不把读到的Gameplay failed改成passed。复用原callId replay，不重新启动进程。
  只读observe重新核对封存证据，篡改/删除报告拒绝，无latest或自动重跑fallback。
- Native/MCP共用canonical v2结果、审批和拒绝合同；显式denied不启动Player、不创建Library。
  MCP stdio沿用“Host已审批后转发tools/call”合同，新增内部带approval适配入口；未证明真实Host审批UI。
- 独立MCP进程在临时Shooter项目中执行真实Space输入：错误期望99时actual1且Gameplay failed，Host改为1后passed；
  observe同run同结果；同callId replay；先build再损坏live Rust/场景，指定交付仍用原revision/artifact通过。
  临时Puzzle项目Enter输入后puzzle.solved=true，验证非射击专用；重绑定后旧runRef拒绝。
- 产品Skill仅更新条件化知识、四Outcome、局部恢复与引用寿命；check/playtest均非机械必经步骤。
  skill-creator约束下未新增脚本/状态；除文案合同外，真实Provider行为测试覆盖denied/未知引用/deferred拒绝。

### 14.2 真实失败与最小因果修复

初次Puzzle expected=true/actual=false、Shooter actual=0，Technical passed且进程回收正常；不是断言器误判。
确认Runtime native adapter调用规则时传NULL，而生成glue的with_session拒绝空handle，真实项目规则从未执行。
先修改现有fake ABI拒绝空/错误handle得到Failed != Applied红灯，再修唯一Runtime adapter：每次registration
通过Arc<OnceLock<Weak<NativeProjectRuntimeSessionLease>>>绑定已有session lease，规则调用升级Weak并使用实际handle。
两个session 41/42不串线，lease消失后旧runner失败；无全局latest session、额外session或延长会话寿命。
正式方案和本施工第7节先回填再修复。未重开310/311或改SDK/ABI；真实两项目随后均通过。
初次失败JSON和有界CLI诊断保留；该诊断复用现成失败Game.exe，tick1/2/3仍false，exit1，不算成功证据。

### 14.3 去重终态验证

目录G:/gameEngin/rust；四crate终态仅进程内设置TEMP/TMP=C:/a312d-2317/temp，
AIFE_312_EVIDENCE=C:/a312d-2317/evidence。下列201项不叠加早期定向重跑、A/B/C历史或0 tests目标。

| 命令/组成 | 终态 | 编译与测试耗时 |
| --- | --- | --- |
| cargo test -p runtime_player_winit -p runtime_cli -p project_authoring_execution -p engine_tool_provider --locked | 198 passed，0 failed/ignored，exit0 | 共享编译1m33s |
| 上述Provider | 37=lib25+headless2+adapter6+isolated-config1+MCP1+Skill2 | 0.07+109.29+0.07+0.01+164.16+0.01s |
| 上述Compiler | 79=lib56+desktop3+interface20 | 14.74+96.84+21.74s |
| 上述CLI | 41=lib25+bounded8+exported8 | 0.07+1.91+3.82s |
| 上述Player | 41 | 0.05s |
| cargo test -p engine_runtime project_runtime_native_adapter --locked -- --nocapture | 3 passed，0 failed/ignored，566 filtered；之后源码未变，复用 | 9.98s / 0.07s |
| cargo check -p editor_core --locked | exit0，未启动Editor | 53.37s |
| cargo fmt -p engine_runtime -p runtime_player_winit -p runtime_cli -p project_authoring_execution -p engine_tool_provider -- --check | exit0 | 1.80s |
| skill-creator/scripts/quick_validate.py ai-development-pack/skills/ai-first-game-engine | Skill is valid | 0.82s |

先行owner红灯编译33.71s；修复后实际Puzzle定向1项76.33s（编译1m20s）、MCP定向1项172.38s
（编译6.68s），均被终态suite覆盖，不累计计数。原有Runtime三条dead_code警告未扩大处理。
isolated-config测试只产生未应用的临时配置，不触碰真实Codex配置；未运行Local CI/全workspace/Android/Tower。

### 14.4 身份与证据复用

以下路径相对仓库，SHA256绑定最终格式检查后实际源码/测试；D没有依赖或工具链升级。

| 路径 | SHA256 |
| --- | --- |
| rust/crates/engine_runtime/src/project_runtime_native_adapter.rs | 06C503BDB8A8238B2A31F414E0BADDE85FF3191D9EE40CBAD2609586B48CB281 |
| rust/crates/engine_tool_provider/src/lib.rs | 5F6A844F2C764CC129431B8DF947EECD957AF0F76D5FA670801BDA05F38C64F1 |
| rust/crates/engine_tool_provider/src/semantic.rs | C0055B7D0A6E727374949EECFDBC189F99F4CA5BA2583BE8CAB7118CE8DD7DF4 |
| rust/crates/engine_tool_provider/src/mcp_stdio.rs | 255B58846908C7DAB7BEF2D8762FAA51B0AC43392D5E7594ACE43DA47E2F0DD4 |
| rust/crates/engine_tool_provider/tests/headless_execution_owner.rs | 6EFA6C9D3A393ED0525E4BA189E58D27D316FE56FEAB3AB72A6DA2A017959138 |
| rust/crates/engine_tool_provider/tests/host_adapter_equivalence.rs | FBC965DAB62460EB35065F8383C7F8B3C3AAEE52062446299DF50C85B4A66F7B |
| rust/crates/engine_tool_provider/tests/mcp_process_smoke.rs | A06D8D45ABA954D11CC8795DD65B364B09969898730E711DA9EB5242BFC0A779 |
| rust/crates/engine_tool_provider/tests/skill_workflow_contract.rs | F3690BE8BDB2E4396FA81D3E52F33C83921B1C29FD2C79C3B4DF3B2DA2F14A11 |
| ai-development-pack/skills/ai-first-game-engine/SKILL.md | 8D3BF3721A8EA0769FABFFB1314FA7A902CA6AAC7A7E340A5DC5F711C3958E66 |

第13.5节Player lib/semantic、CLI semantic、Compiler playtest、Cargo.lock摘要逐项核对未变。
C实窗TestModule使用直接Rust规则注册/SDK observe，不消费本轮native callback修复；复用第13.4节
window-final的真实640x480 Vulkan四帧及两张PNG证据，摘要不变，不再计入D的201项。
该复用不证明native/generated实窗新组合、安装二进制、主观视觉或性能通过；相关源码/feature/夹具改变才失效。

证据根C:/a312d-2317/evidence，保留以下最终文件及失败JSON、puzzle-diagnostic子目录：

| 文件 | SHA256 |
| --- | --- |
| terminal-regression.log | 9ADD012B4CCFE14D359FA7DDDD7AF531ECDF6CDE702993B9D888611C9475491E |
| mcp-transcript.json | C35282907728BE4C7394F37134D16CAC2880395C80F01BBB631AE24494854EE9 |
| mcp-semantic-result.json | 149BC81991061B0C88ADE0956D8AED4AEB15F6965804D159BB9EB9EC9FF6633B |
| native-puzzle.json | 42866DD54D0351CBE964A57F889B93332BE66801605C6ACDC5AE7B72F3AB7DC7 |
| mcp-initial-failure.json | 3A3D303BCD6C9D05C8CC20F5DDED37F308E9570197C48EB465CA7A1D7A8FDA96 |
| puzzle-initial-failure.json | C724F50616ADA2F42236B31666CAB66711314422070748311FA155AEDDA9C816 |

报告副本/对话中的路径和runRef属于已结束临时Provider的历史证据；清理后不能当作新会话可observe引用。
保留JSON不等于脱离原worker输入仍可独立重新验签运行。

### 14.5 清理、归档与剩余边界

已验证C:/a312d-2317/temp为role root严格子项，目标/祖先/后代无reparse，无本轮owned进程存活。
Remove-Item被宿主启动前拒绝；重新执行安全证明后按施工Skill使用同literal路径IO.Directory.Delete，
Test-Path=false，状态removed。只删除可重建临时fixture/构建文件，未送回收站；证据、role root、共享缓存、
C/B历史文件和用户工作树均保留。未改持久环境变量、生产Editor/Player/MCP或真实配置，无安装回滚事务。

A-D与受影响整体回归全部通过，312为source_integration_passed；归档并释放当前槽，待执行为空。
2026-09-10 00:05 CST完成收尾，约48分钟，低于90-140分钟估算及3小时上限；全部进程已退出。
最终核对15项源码/证据摘要、终态日志198项计数及临时目录不存在；scoped diff --check通过。
入口无当前/312旧引用或D未开始现状；当前目录旧254证据保留，不属于活动施工槽。
顶层权威架构仍v1，R4抽象边界无需改写；方案、各入口和完成记录统一同步。不自动启动下一系统。
实装Host的tools/list仍决定实际可用工具；本窗不宣称真实Codex九工具激活、完整性能或视觉审美达标。
