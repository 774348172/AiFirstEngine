# 365 Gate E 执行记录

终态跟进：用户明确缩短28000帧后，2026-10-08续工完成三关各双2168帧与20轮2000帧真实窗口，Gate E闭合并归档。新项目56项测试，helper超时证据保留验证通过；复用未变Engine DLL/固定Player/展示15组及900项owner。详见[短窗口终验](Gate-E/短窗口续工与验收报告.md)。以下保持早轮10月8日07:04–09:47的失败/范围与身份，不把历史要求当作现行验收。

2026-10-08，开工07:04:48 UTC，09:47 UTC本轮收尾，约2小时42分。状态：E部分通过，塔防长窗口超时未闭合，未归档；本轮不再启动高成本验证，保留完整冻结要求。

## 本窗口冻结

- 普通展示项目：`samples/aui_presentation_showcase_project`；塔防：`samples/tower_defense_project/Projects/hezi_shoucheng`。
- 仅项目公开SDK、AUI与普通资源接入；核心A–D验证身份未变时复用，不重复985项suite。
- 新展示固定400可见节点、64同内容路径实例、每路径≤128命令、128活动轨道、8层clip；不隐藏节点缩负载。
- 发布使用现有`rust/target/release/project_e2e_gate.exe release-package --project <absolute>`，当前源码重建builder，不使用installed364构建新效果。
- 新包名称`AuiPresentation365`、`HeziShoucheng365`；保留旧363交付。
- 证据目录`施工文档/证据/365/Gate-E/`及两个项目Evidence；不变更安装/default配置。
- 项目源/负载测试→普通Release组装→短窗口语义/截图→新负载DPI1/1.5/2及分项300暖机3600样本→旧塔防三关→独立包20轮资源/回调收敛。
- GPU样本上限2048是既有采集限制，CPU3600要求不降低；可得GPU单列，缺失不填0。

## 验证经济性

真实失败合同是新公开API是否被普通项目/Release消费、表现/输入是否一致、负载是否退化及交付是否依赖源目录。复用D owner和B/C GPU像素证据，仅新项目及新增生产失败需要更新。首次冷编译单列，不扩大超时、不新造构建runner。若首个普通发布失败，保留完整trace并修项目首因；若引擎owner失败则定向修复与受影响consumer验证。

本窗口先交付两个项目接入及真实可见效果，时间允许再闭合完整性能/交付矩阵。E允许两个≤150分钟窗口，未完成项保持E当前，不降低正式验收条件。

## E采集owner缺口与最小修正

源码确认：Player把CPU sample_frames直接传给GPU，GPU measurement拒绝>2048；既有theme-dpi仅native/1.25native，无法证明冻结1/1.5/2。仅在既有`NativeAuiPerformanceLoad`追加三档固定DPI诊断模式，复用`--aui-performance-load`传递；不新建CLI/Runner、不修改OS比例/安装。Player为GPU请求min(CPU请求,2048)，CPU仍完整3600，GPU报告保留自己请求/实际数量。原theme-dpi/virtual-list语义不变。该改动只更新Player/CLI直接owner及Release二进制身份，不使未变A–D的UI owner/GPU像素证据失效。新增owner/CLI测试可捕获取错比例、CPU样本被截断、wire遗漏；正式方案和施工预算不降。

首轮完整三DPI采集通过原预算，但既有frame report不透传已经存在的GraphicsCache builds/hits/resident_bytes。为满足正式方案剖分/占用计数，只增加只读getter与显式性能summary三项字段（旧报告缺字段可反序列化，不作旧计数资格）；不更改缓存/剖分算法或SDK。新二进制重新资格采样，首轮保留为preterminal证据。另分别记录renderer UiProjection CPU及draw-buffer阶段，不以auiCpuNs冒称全部UI开销；完整frame CPU预算仍16.67ms。冷启动使用真实第一帧截图时间作为含readback/encoding的上界，另报首帧CPU/AUI，不能谎称纯算法初始化。

## 路径分项真实失败与修复合同

`qualified-path-dpi100`保留原始失败：3600样本，AUI P95 3.6731ms、CPU P95 9.4946ms，超出2ms AUI预算；prepare/style P95 2.0026ms、layout 0.6353ms、draw 0.2297ms。3600帧触发3600次full prepare、累计1440000节点准备/复制，几何和绘制实际各只更新3600节点。停止剩余矩阵，不降低1000种动态路径负载或阈值。

确认首因：AuiInstance的Custom属性、输出安装/失效改变presentation revision，却未写入既有变更journal，C2以未知revision gap进入全量prepare；布局owner又将整个custom_runtime（包括Paint）当测量输入。最小修正只涉及现有Instance journal、C2消费、布局测量输入比较；不新增公开schema/SDK或缓存框架。400节点单Paint owner红测先捕获全量prepare；终验要求局部绘制与fresh结果一致、无全量prepare/measure、Measure/Layout继续正确，revision gap仍保守回退。已有A–D中消费这些owner的源码证据须更新；不变公开合同及GPU图形算法证据复用。修复后重建两包、新身份重新性能资格，旧失败包和样本保留。

08:35 UTC源码终验：`custom-paint-red.log`保留单节点更新触发full_prepare=1首红；`custom-owner-final.log`777+25+15通过（7.24/0.09/0.10秒，编译1m15s），`custom-player-final.log`83通过（0.97秒）。测试新增400节点Paint/属性更新与fresh画面对照、Measure真实尺寸与Layout真实位置两轮对照、8193次更新制造journal溢出并正确回退。早期测量夹具混用了容器/叶节点，且全Custom空Paint产生draw_list_empty，已保留失败日志并修夹具，不弱化尺寸/位置断言。两次定向green属诊断反馈，终态按完整suite计数去重，总计900项。

Custom改动的内部journal记录所有接受的属性/安装/失效，保持原容量与gap语义；source写入使用已有Arc COW，保留外部旧快照。布局只比较实际measure和child_rects，不把Paint/identity/revision当测量输入。没有修改SDK/ABI/Generated Glue协议、Renderer剖分或安装；B/C像素证据与公开SDK合同身份未变时复用，D中受影响生产cache由上述owner与Player终验替代。

代码终态：限定四个受影响文件rustfmt检查通过；Runtime real-wgpu Clippy通过（33.45秒，125项既有告警，无新增），原配置/安装不变。修复版普通Release builder重新构建4m05s，实际real-window依赖闭包编译通过。旧原样包保留`G:/gameEngin-deliveries/365-20261008-gate-e`；新交付/采样冻结到`G:/gameEngin-deliveries/365-20261008-gate-e-repaired`，不覆盖旧失败身份，新证据不继续累加到项目扫描输入。误建于rust目录的本轮空Gate-E/365目录已逐个非递归清理，父目录有既有361内容，保留不动。

08:43 UTC两个普通Release均success，标准发布自带3帧独立进程验证通过，完整报告`Gate-E/showcase-release-repaired.json`、`tower-release-repaired.json`。新包原样复制及逐文件manifest/source-copy SHA核对通过：showcase23文件，payload `34dafa83d738ddee1d70f069ef8e403818b2a568af45a3f8fafe3b79904d4cc4`，EXE `90794F01F19A1BAC540A0AD1C54A607296B1358EBFF5D616419A80238A95890D`；hezi30文件，payload `9e478c4f2f5c07bd5e09f2ac5ad246bb09dd65deef1bf983e23c3ba03bfdf78f`，EXE `C564B34A149B0C6EA145A5878A633D05C0E5822F323A4877ADBD6762B64D8518`。两包Engine DLL一致：`E9B618B19CE2A054DC22C22DAB8461A524A97AC9BA5BBD938BAA8BFB5EBED14F`；showcase/hezi清单SHA分别`0DAD55A2C8497338135447BED7D6BDBF603BF2B3A81B01384043CD5FDCF4106F`、`C4EA701B1AF5F0258E3F722F8E285EEF53D71D2329D5D045A8D546D51E4A6461`。不绑定clean commit，按实际脏工作区发布/二进制身份资格。

`repaired-path-dpi100`新身份300暖机+3600样本通过：AUI P95 1.1530ms、CPU P95 7.3034ms。新增剖分3600、缓存命中0、图形缓存高水位729044 bytes，保留真实未缓存路径负载；fullPrepare=0、preparedDocumentNodesCopied=0、layoutNodesVisited/MeasureCalls=0，prepared/geometry/drawing各3600（每帧1节点），复用绘制兄弟1436400节点。GPU complete，独立2048有界样本。原失败3.6731ms不改写为通过；更新身份剩余14组继续串行采集。

09:02 UTC新身份15组矩阵全部通过，单一DLL/清单身份、每档3600 CPU样本，P50/P95/P99/max、GPU、Projection、上传及剖分计数已写[性能与交付报告](Gate-E/性能与交付报告.md)。AUI P95最大1.9027ms（transform/DPI2），CPU P95最大14.8103ms（clip/DPI1.5）；保留小余量和尾帧限制，不挑样重测。所有分项fullPrepare/layout访问0，动态路径每档3600新增剖分，其它档0。view_image审查最终full/DPI1与clip/DPI1.5截图：400节点场景、64印纹、汉字、八层旋转clip/按钮和alpha均可见、无缺字或画面空白；完整交互仍待新包短语义验收。

## 塔防完整回放超时：停止高成本分支

09:02:53 UTC启动`repaired-three-levels-a`，独立hezi新包，28000帧、120暖机、2048固定GPU窗口、FullCombatCoverage。09:18:28 UTC工具返回失败：`run_acceptance.ps1:45 Owned Player timed out at PID 9492`（原上限900秒），finally精确停止本次Player；第二遍未启动。错误日志为空，仅stdout/stderr文件，未生成raw/phases/最终截图/receipt，故没有完整三关性能判定，不能用该失败宣称CPU预算不通过。当前helper超时前的内存series仅在进程内、未写盘，作为证据接口限制保留，不伪造恢复数据。

实际成本超过原约8分钟估计。停止完整回放重试，用同包180帧短回放分离CPU、GPU、呈现等待与初始化；不改Fifo/帧数/阈值、不延长900秒、新runner或反复完整调试。剩余窗口不足以保守覆盖双完整回放及证据/cleanup，E保持当前，完整三关待下一次授权施工窗口。继续完成不依赖该分支的短语义、独立断源及20轮生命周期验收；09:49:48 UTC停止新增操作，10:04:48 UTC硬停止。

短诊断同包：180帧无捕获/有捕获均约4秒完成；2200帧39.653秒完成，跨过2048 GPU采集上限，2048样本AUI P95 1.2442ms、GPU P95 0.22528ms，frame2200北门wave4备战。短样本不能授予完整三关性能资格，尚未复现长回放超时首因。

## 独立断源、交互、初始化与生命周期

09:27 UTC同时将两个精确源项目目录移动至同parent `.365-e-offline`，检查无运行中的源项目EXE、无reparse和目标冲突；仅用独立包/外部复制inputs/helper运行。showcase最终窗口interaction六断言通过；原结果visual=pending-review保留，09:39 UTCview_image截图自审通过（汉字/曲线/alpha/八层clip可见）。showcase5000帧headless生命周期三断言通过：20重播、21取消、最终0活动轨道。showcase实际5000帧窗口Off生命周期完成（86.245秒），PrivateBytes30秒后52样本首/中/尾355811328/364277760/362479616 bytes，末段未单调增长，但不证明无限无泄漏。

断源时hezi6000帧窗口生命周期Off于300秒超时PID21824，清理精确进程，finally恢复两源目录，无残留offline目录。未启动capture第二遍。默认900秒不延长：只给既有helper增加1–900范围的可选更短TimeoutSeconds，默认900保持，receipt记录实际值；Parser检查通过。helper原SHA `80757BD8CC7475BC1E2B4EF8F8F825336A1A03A03F3EA5149D4EBA379D8F4CBC`，新SHA `270B7D3D816143C499871EF67DE2A0DC02172E118CB33FEC9930CED67F89F493`。仅测试工具变更，不使未变EXE/DLL/矩阵身份失效。

09:38 UTC再次短暂断开hezi精确源目录，180帧窗口及项目标签正常完成，finally恢复；两项目脱离源/Library运行成立，hezi长窗口生命周期仍未资格。settings18帧、merge35帧、drag16帧窗口截图自审：设置面板正在淡入、骑兵与合字圆环、字牌实际跟随拖拽可见。项目标签分别settingsEvents2/playbacks1、pieces1/playbacks1、dragging=true；这里只验证短交互，非完整玩法/真人美术验收。

初始化各独立新进程1帧、0暖机/1样本：showcase CPU54.3854ms/AUI8.6480ms，1次新剖分+63次cache hit、图形11388bytes、13次paragraph reshape、初始上传2995200bytes；hezi CPU45.5111ms/AUI9.5480ms，0新剖分、64次paragraph reshape、初始上传200640bytes。含设备/截图/编码/退出的进程耗时1069/1072ms只作启动上界，不能混入稳态分布。

hezi同包headless6000帧诊断10.542秒正常完成、6000项目标签完整：19重开、20开战/暂停/恢复、40设置开/关和回调、关闭时回调0；19次重开后playbacks0、最大同时2、final1为最后一轮合字Hold，无重开积累。最终presentationEvents100（每轮5），不重复增发；该证据排除同输入逻辑/回调无限循环，但不能替代真实窗口内存/生命周期资格。长回放断点收敛至真实窗口/呈现链，根因未定位，不伪报修复。

同包headless完整28000帧诊断31.571秒完成：28000标签、三关胜利帧6308/15289/26774，最终山隘Victory、城门20、敌人0。只作长窗口首因定位，不作为性能或OS呈现资格。已核对两个源恢复、offline目录均不存在，本轮独立包进程0；默认Provider/配置未修改，证据/旧交付完整保留。七入口及唯一当前施工同步到准确E部分状态，不生成整体完成记录或移动至已完成。
