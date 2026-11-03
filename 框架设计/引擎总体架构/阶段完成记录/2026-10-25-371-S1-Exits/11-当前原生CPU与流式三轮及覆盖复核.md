# 当前原生CPU、流式三轮与覆盖复核

2026-10-25，阶段70，证据根`.artifacts/371/s1-exits-20261025-033155`。S1仍partial，本记录承接[10记录](10-静态故障核销与启动内存容量修复.md)。本次续行未修改Rust产品或测试源码，未编译；434冻结的1408输入在终验时无漂移。原十小时授权15:52:25 UTC停止新增、16:07:25 UTC硬截止，不因本记录重新计时。

## 正常导出与当前身份

437正常Compiler导出已成功（382.970秒），439绑定当前Game与DLL；DLL SHA256为`4a3bedabcef438dac320e9355aef2a1c940df393c2d7956633a60f5483382281`。440当前正常交付完成20秒/1200帧，实际窗口及报告均1920×1080，窗口报告修复已有真实consumer。451同一当前原生Host消费原阶段69 BM03 K1包16帧正常结束，保留未打包CLI测试模式的真实身份；449外层包身份不符的0帧拒绝未绕过。

397原2×K1正常包在446第453帧自然触发`three_d.simulation_overloaded`、452帧结束，62.812秒，私有内存约5.888GiB。448核销其有界拒绝及原时钟债务保护；不是完成整条压力轨迹，也不是主档CPU失败。原catch-up每帧最多4tick、债务120tick边界保持，未抬上限。

## 当前CPU与统计成本必须分开

452按原固定六进程/三对计划执行，453 Summary与454 Off各600预热+14000采样，实际1920×1080，采样wall分别146.278/141.181秒，tick与时钟条件通过，未发现竞争编译。Summary CPU P95/P99=12.4590/13.4374ms；Off=12.0397/13.1025ms。两个窗口各自均低于13.3/16.67ms。

首对Summary增量0.4193ms，原门槛max(Off P95×3%,0.2ms)=0.361191ms，超出0.058109ms。459保留失败并停止后续455–458；不得把当前一个CPU合格对说成三对资格，亦不得把统计成本红例说成主场景CPU又回到二十多毫秒。阶段69旧DLL三对资格仍按原身份有效。没有在当前失败后无因果反复跑相同DLL或改门槛。来源分类扫描存在于Summary路径，但尚无证据把它定为该0.058109ms缺口的首因，未据此再做渲染重构。

现有分段Summary均值：renderSubmit 8.631773ms、update 1.115359ms、auiCpu 0.018230ms；Off分别8.429385/1.088996/0.018534ms。渲染提交仍是最大工作段，但跨独立进程的分段差值不能当成同帧的严格成本归因，嵌套UI时间也不能与总工作简单相加。此次只读取已有报告，没有新增profile长跑。

## 双cell三轮的实际结果与范围

| 独立窗 | 终态 | Active后时长 / 内存帧数 | 跨界 / 往返 | 私有内存峰 | 初始等待 / 安全点等待合计 |
| --- | --- | --- | --- | --- | --- |
| 464-stream-ten-minute-0 | passed | 601.862s / 28372帧 | 592 / 74 | 0.818GiB | 189.443ms / 10.213s |
| 465-stream-ten-minute-1 | passed | 601.866s / 28507帧 | 592 / 74 | 0.789GiB | 189.696ms / 10.064s |
| 466-stream-ten-minute-2 | passed | 601.847s / 28180帧 | 592 / 74 | 0.818GiB | 187.491ms / 10.002s |

467序列终态为`three_original_dualcell_windows_passed`。检查保持原32m/s、一个cell预取、两个各32MiB真实payload、实际重复上传/回访、全帧内存、系统可用4.5GiB、进程6GiB、tracked图形4GiB、1920×1080与最终资源退场。每个已通过窗口的等待ledger完整保存，不删除启动/安全点等待，也不以未出现next-region等待声称零停顿。三轮双cell子档不能授予40个独特cell的完整BM08/world规模。

442实际完成604.891秒/28263帧并exit0，但旧harness随后因Summary没有`finalProjectObservation`而KeyError；460保留此首错，不改判为正式合格。463使用既有Trace首帧过滤取得观察，保留逐帧allocationSummary；同时移除441误加的36000渲染帧要求（371-A §6.1流式行没有该条件），模拟tick与十分钟、访问/上传仍须通过。未修改产品以适应harness。

`pendingGpuTickets`来自GeometryUploadSnapshot的活上传收据数，不能当作GPU timestamp未完成数。空纹理替换可以在末次采样保留一个无资源Ready收据；正常finish在采样后释放。463仍要求mesh/texture/staging/retiring/count全部为0、正常退出且无release错误，只容许该已有空收据。保留正常释放与旧owner测试，不将所有非零ticket豁免。

本次全帧显式内存普查的CPU较高，不能用Trace加普查的数据授正式Off/Summary预算。461只导出第3600帧一次原始普查：19558条，其中assets/decodedCache9366条。462沿源证实Windows 3D profile句柄上限65536，release会释放最后引用的decoded payload，但保留有界Released元数据；普查遍历这些历史字符串。它解释了一个增长的诊断工作来源，未证明独占全部开销，也未证明泄漏。没有任意降低句柄上限或递归扫描全部历史来“补齐”计量。

`framesInFlight`现字段来自lifecycle.pending提交队列长度，上传与绘制均可进入；它不直接等于独立呈现帧在途数。本记录不以该字段替代原在途帧资格，既有16提交拒绝及上传在途2边界各保留本义。

## 可访问backing审计纠正

468取代447只靠NotInstrumented文字推断的缺项分类，不改变实际partial/unavailable覆盖。cell→PreparedPrefab→PreparedRuntimeEntities→observe_prepared_component实际调用链已存在；Renderable/Sprite/Animator/audio/particle等typed容量及commit/despawn回归测试已有源码，本轮复用而未重跑，不能再要求重做“其它组件全部接入”。

真正可访问的缺口仍包括AuiSession active/pending状态、MovingViews控制Box及其嵌套内容、Player其它报告与measurement history嵌套。当前frame/Graph/RHI报告和workset主体已有计量，未全部缺失。std HashMap/BTree节点、Arc控制块与库内部scratch分开；局部prepare scratch已在frame census前释放，缺的是该寿命峰值证据，不能凭scope文字认定持续滞留。未建设全局allocator、定制parser、第二账本或驱动私有平台。

## 不过度施工的剩余清单

| 原出口 | 当前未闭合 | 最小后续动作 |
| --- | --- | --- |
| X1 | 当前主场景三对CPU/统计成本未齐；BM01 K0低dirty的280/321/323原成本失败 | 已合格主CPU与统计增量分别记录；先有因果诊断再修原owner，不无因果挑绿重试或重开整条CPU架构 |
| X2 | 原F01–12的foundation静态/双cell组合证据未全部对应 | 复用267/294/304/308及416四静态负例，只补原326中明确缺项；不扩全部调度笛卡尔矩阵，physics/nav/world不反加S1 |
| X3 | 原BM02/03三倍率K0/K1及耗尽/清理的完整证据仍不齐 | 复用已有效配方与受控拒绝；2×压力允许有界拒绝，不要求13.3ms，也不将448说成全轨迹通过 |
| X4 | 明确的UI/控制/报告嵌套和瞬时寿命计量仍partial | 只接现有owner真实backing；先核销已实现覆盖，不把opaque、无泄漏和全owner资格混为一谈 |
| X5 | 真实工具/兼容已通过，451补当前Host准入 | 不重装或重复整套工具链；源码、正常交付与installed分别报告 |
| X6 | 双cell长窗按467核销；完整内存/正式性能与OS显示仍未资格 | 复用已完成长窗，不再写未运行；315 ETW权限5与GOOGLE timing不可用保持unavailable，不扩私有解码或改外部配置 |

371-G已批准的延期接收仍有效，功能准入不以清空完整资格表为前置；完整S1声明也不能把延期直接改判通过。当前未发现由本次审计新复现的崩溃、提前释放、无界队列或持续payload泄漏。已修首因、有效功能证据不重做；完整声明所欠实现、组合验证、性能成本和平台观测分别保留。
