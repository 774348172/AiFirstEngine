# 371 §0.33 阶段36：S1故障终结与真实GPU压力消费者

日期：2026-10-22。状态：`s1_partial / worker_faults_repaired / scaled_gpu_consumers_passed / bm01_cpu_budget_failed / formal_qualification_pending`。

用户授权“开启新的施工窗口，继续S1剩余出口，复用阶段35证据”。本窗从00:47:26 UTC（北京时间08:47:26）起计时，默认3小时；03:32:26停止新增、03:47:26硬截止。上一窗8小时例外不继承。实际收尾时刻及整个窗口耗时以[closeout](../../../../.artifacts/371/s1-continuation-20261022-004726/45-closeout.json)为准。本窗实现与验证已结束，无后台续工；**本窗没有完成S1、S2或整个371**。

## 1. 基线、边界与证据身份

工作目录G:/gameEngin，分支codex/255-capability-catalog，HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5，保留全部原dirty。B路374原登记逐字保持，不改其工作树。Windows only，Android延期；不改安装、宿主、Physics和游戏V0.0.5。

证据根：[s1-continuation-20261022-004726](../../../../.artifacts/371/s1-continuation-20261022-004726/)。开工00-baseline核对阶段35终源32项无漂移，复用35的143源/144DLL/145短窗/147八组合功能证据及其限制。World、SDK、ABI、native等未改owner证据继续按失效面复用，未重跑完整Compiler配方、BM03、workspace或正式三轮。旧loader29由当前31项更新；旧Host证据由当前39项更新，不能把所有历史测试重复累计。

最终产品/测试输入见26-final-source-freeze.json及[43审计](../../../../.artifacts/371/s1-continuation-20261022-004726/43-evidence-audit.json)：37项冻结输入中Rust/Cargo/固定消费者无漂移，随后只有generate-pressure.py发生消费者修复，单列新SHA；不让harness变化使未变产品DLL全部失效。无临时产品probe待恢复。

本轮最终引擎DLL SHA256：`07557099f51e21ab81206a03d3841e17c43d6854a02e6967f3e00266729592a0`。六个新交付均相同。旧21-current-release为补修Host超时后续tick之前的产物，不能当最终版本；最终版本来自28-pressure-K1-1-build。

## 2. 实际缺陷、首红与原owner修复

| 实际问题 | 原owner内修复与保留边界 | 首红 / 最终证据 |
| --- | --- | --- |
| loader完成channel断连后，pending未可靠终结、预算/worker状态不一致 | 断连关闭准入并将pending标WorkerStopped；仅真实JoinHandle finished后清worker/current/retired operations。未退出线程仍持有其预留，不用断连假装释放；shutdown不覆盖原错误 | 02及06竞态失败；10 loader31 |
| render worker失联未及时关闭准入，消费旧startup报告可能覆盖producer已接受frame/本地计数 | 区分Empty/Disconnected，非正常Stopped标WorkerLost；保留真实frame和本地fence/timeout，立即拒绝不再重复缓存 | 03/04/08；24 worker12 |
| Host资源锁poison时panic或继续准备 | 内部with_resources返回Result，Host在准备前检查poison与worker terminal，竞态仍受控Exit；已经推进的模拟保留真实frame | 14；23 Host39与26真实GPU2 |
| accepted render frame超时后无CPU结果，Host假定必有结果；下一tick重新等旧锁 | 同步消费者有界等待2秒，无结果输出typed错误并关闭准入；任务/资源到真实完成前仍保留。下一tick先检查terminal再准备资源，不继续推进World | 22首红下一tick仍等待2.0099954秒；23修后<250ms、World delta Arc未变、受控Exit、无伪造GPU提交/完成 |
| finished typed decode payload在decodeWork→completion发布之间未观测实际backing | 借用DecodedObservation在payload仍活时登记实际allocation，移交/析构前移除自身记录，不额外持Arc延长寿命；Off不扫描payload | 05 typed PCM backing首红；10 loader31 |

实际产品文件：engine_runtime_core/src/runtime_asset_loader/streaming.rs、render_thread_worker.rs、engine_host_loop.rs、resource_memory.rs。测试补在原loader/Host/worker及engine_runtime现有integration。未新增schema、资源权威、loader或renderer。

这里的“typed decode已观测”仅指**已完成typed payload到completion发布之间**。parser scratch、解析中的中间结构及其它World/SDK/renderer/report backing仍非完整接入，不应再统称平台不可观测，也不应反过来宣称全内存已闭合。

## 3. 两项真实GPU故障消费者

13-fault-consumers保留原owner直接测试；后续Host terminal修复未改变其loader/native消费面，因此按身份复用。

F07 `foundation_f07_dual_cell_seventy_five_percent_queue_and_gpu_closure_refuse_atomically`：双真实32MiB cell；6 operations闭包的75%为4，原子拒绝，无部分World发布。GPU实际闭包33,554,476 bytes，75%为25,165,857 bytes；B收到`geometry.upload_resident_budget`，A仍active，最后真实完成退休归零。不授physics/nav可选降级或完整BM11资格。

F08 `gpu_f08_all_declared_graph_faults_reject_before_submit_and_keep_old_success_valid`：真实合法红三角前后像素一致；未初始化读、feedback、非法subresource、depth format、sample mismatch（attachment_mismatch）及invalid_history六子例全部在提交前拒绝，每例新增GPU submission/upload为0。六类真实拒绝不等于原规模完整BM11。

## 4. 六组普通项目真实GPU轨迹

采用既有BM02静态几何，真实项目→Compiler/Cook→RuntimePackage→项目module→GPU，不用纯空间算式冒称绘制。三个规模分别10000/15000/20000实例、100/150/200 mesh、32/48/64 material、8/12/16张2048²真实纹理、10M/15M/20M实例三角形。

| 轨迹 / 倍率 | 构建标签 | 完整轨迹证据 | Player外层秒 |
| --- | --- | --- | ---: |
| K1 / 1 | 28-pressure-K1-1-build | 31-pressure-K1-1-full | 52.153 |
| K1 / 1.5 | 33-pressure-K1-1-5-build | 39-pressure-K1-1-5-full | 70.548 |
| K1 / 2 | 34-pressure-K1-2-build | 39-pressure-K1-2-full | 81.195 |
| K0 / 1 | 35-pressure-K0-1-build | 39-pressure-K0-1-full | 27.689 |
| K0 / 1.5 | 36-pressure-K0-1-5-build | 39-pressure-K0-1-5-full | 32.442 |
| K0 / 2 | 37-pressure-K0-2-build | 39-pressure-K0-2-full | 36.502 |

每组7201 simulation ticks、7201提交/呈现帧、37状态断言及1920×1080最终截图通过。保留原布局、K0的220m相机、冻结K1轨迹及120秒超时；不修改原交付。Off/unpaced只授完整**模拟轨迹功能证据**，不授持续wall性能或正式资格。

最初复用阶段32旧K0 module在0帧被`native module ABI prefix mismatch`拒绝（旧ABI早于1.7），见30；未放宽拒绝。仅复用真实资产，重建当前SDK项目module，32-current-k0-inputs证明K0 Scene原字节未变。最终首组31显式DLL override与包内DLL同SHA；后五组39走包内DLL、无override。后五导出engine cache hit，只重建项目module。package、module、DLL、scenario、Cook及capture身份绑定在43，不授历史旧包整体升级资格。

生成器两处职责错误已修：放大档committedMatching应为15000/20000；2倍20001项冷空间只读resolve超过既有16384上限，改分批只读查询，未扩大ABI上限、未拆原子mutation。静态初始化仍零批实体写加相机。BM12 driver的run-scenario仍不支持，本窗走已有独立semantic consumer，不能把该工具operation写成已实现。原Cook notExecuted及unqualified保持原时点事实，独立轨迹补充不改写旧报告。

独立像素参考42-k0-pixel-reference.json从源TRS独立计算正交投影，在互不重叠的1m方框内逐实体检查：1倍10000/10000完整入镜均有至少16着色像素、4×4 extent；1.5/2倍各10500完整入镜，另4500/9500在视口外。因此放大档all-visible仍缺，最终一张图也不代替每帧资格。初版参考误假设顶部始终空白，首错42-initial-reference-failure保留，改用不变左空白区；镜头与4×4断言未放宽。K0/K1真实截图已人工查看。

可见结果：[K0一倍](../../../../.artifacts/371/s1-continuation-20261022-004726/39-pressure-K0-1-full/capture-7201.png)、[K1一倍](../../../../.artifacts/371/s1-continuation-20261022-004726/31-pressure-K1-1-full/capture-7201.png)。普通PBR/光影项目保持28/29等既有交付，本轮静态压力不能替代其或BM03资格。

## 5. 当前最终DLL短性能反馈

性能测量期间不并行构建、检索、编辑或其他施工。固定4 warmup+600 samples，CPU正式口径为cpuExcludingPresentWait，GPU为timestamp frame；最大值保留，不剔除截图尾帧。

| 同BM01 K0/100% dirty | 阶段35 / 145旧DLL | 本轮40 / 最终DLL |
| --- | ---: | ---: |
| CPU mean ms | 11.909451 | 11.705638 |
| CPU P95 ms | 13.3214 | **13.4673** |
| CPU P99 ms | 15.1229 | 15.5363 |
| CPU max ms | 127.3469 | 119.819 |
| GPU mean ms | 0.249844 | 0.2262545 |
| GPU P95 ms | 0.393216 | 0.372736 |
| GPU P99 ms | 0.923648 | 0.801792 |

40复用35原Game/module/package/scenario，仅显式换当前DLL。CPU P95仍超13.3ms，**不证明整帧改善，也未达预算**；不再重复跑挑绿。当前update mean11.2237948ms、renderSubmit0.2889733ms，剩余主要区间在update。35的137等旧probe细分不能直接套当前版本；本窗没有新的精确transaction子分段，尚无依据批准新设计改造。总CPU含present的mean/P95/P99为11.947488/13.6925/15.7896ms，不能混口径比较。实际采样窗7.2215秒，非持续资格。

41同最终DLL、包内运行BM02/K0一倍：CPU mean/P95/P99=0.9996353/1.1643/1.383ms，GPU=1.8502025/2.168832/2.317312ms。总CPU含present=1.9993213/2.2139/2.3609ms。采样仅1.2267秒；GPU600完整样本但submissionObservationsOmitted=4，不能授正式资格。起止pending GPU ticket=1，不能冒称本run证明全退休。采样内private peak1,013,026,816、working set673,280,000 bytes，不是全运行/全owner峰值。

40-performance-environment系统available=5,794,983,936 bytes（约5.40GiB），**单点**超过当前4.5GiB=4,831,838,208 bytes，不授全程余量。未关闭其它应用、未从available扣工作集。两窗口实际display timing均unavailable，未闭合OS显示资格。均值不同不能证明环境差异或代码收益的唯一原因。

## 6. 最终验证、真实失败与费用

最终去重有效90测试名：10 loader31（之后owner未变）、23 Host39、24 worker12、13 foundation故障5+F08真实GPU1、26真实Host GPU2。loader/Host等是engine_runtime --lib重组合；不称独立native快测。26 Rust输入冻结后没有再改。7受影响Rust文件fmt通过，25 scoped Clippy和26 default check通过，core148/native17/input2警告保留，不称零债务。

首错/返工保留：01私有字段编译错误；02 loader预留；03 worker失联；04旧startup覆盖frame；05 typed backing；06 shutdown unwrap竞态；07新checkpoint误占phase3导致F02失败（修为独立phase4）；08超时仍准入/立即失败重复缓存；11/12测试Transient应FrameLocal、Prefab Result不能PartialEq编译错误；14 Host poison；22超时后续tick；27项目缺rustfmt；30旧ABI拒绝；42像素参考假设错误。普通施工错误按owner修复，不删失败或降低断言。

全费用见[window-cost-ledger](../../../../.artifacts/371/s1-continuation-20261022-004726/window-cost-ledger.json)，包括首错与返工：

| 类别 | 实际秒 | 口径 |
| --- | ---: | --- |
| Cargo build/check | 745.2854 | 原Runner直接build/check，约12.42min |
| Clippy | 30.7396 | 单列 |
| Runner测试执行 | 68.8207 | 含失败及重试，不当作去重通过数 |
| 直接owner测试执行 | 7.8398 | 10/16/24独立执行，不在上一行 |
| Compiler外层 | 1472.237 | 约24.54min，含准备/项目编译/失败/导出，非纯编译 |
| Player外层 | 349.852 | 含失败、六组轨迹和两次性能反馈，约5.83min |

Compiler嵌套build-timing已经包含在外层，不再相加。21提前导出427.767秒后又发现Host后续tick缺陷，最终28重新导出487.365秒；这是本窗可改进的验证顺序，不能把两次都称不可避免。先收敛故障序列再导出可减少此类返工。27缺rustfmt失败48.642秒及旧ABI失败同样计入，不把失败总时长全部称编译浪费。后五项目engine artifact命中，未重复完整引擎编译。

38清理三个已核实本窗失败fixture，保留52-byte WAV及SHA后删除成功；未触碰历史policy拒绝目标。另两个极小temp候选没有日志中的精确路径绑定，本窗保留并登记，不声称所有temp都清空。没有宿主policy清理拒绝，初次DateTime验证误判已记录并纠正。

## 7. 固定出口审计与下一继续点

| S1剩余出口 | 本窗推进 | 未闭合原因与后续 |
| --- | --- | --- |
| worker/Host故障受控终结 | 上述真实失联、poison、超时及下一tick缺陷已修；90有效终验 | 不等于全BM11固定跨owner故障序列 |
| BM11 F07/F08 | 双32MiB cell实际75%拒绝与六类图提交前拒绝 | F04三帧/最大lag+内容版本、F05各20次真实窗口/设备恢复、跨ownerF09–12及required能力/真实压缩数据等仍缺完整原规模消费者 |
| BM12真实静态GPU压力 | 六档完整模拟轨迹及独立最终像素证据 | BM03、放大档all-visible、资源/句柄/队列耗尽、其它owner阶段及K1三维层次布局/完整配方资格未关闭；统一driver仍缺 |
| CPU预算 | 最终DLL同原BM01一次反馈，主要残余在update | P95=13.4673ms失败；下一轮先做一次有界当前update分段，再据首因修原owner，不因接近门槛挑绿 |
| 实际内存backing | finished typed payload接decodeWork，保留真实worker退场预留 | parser scratch/解析中间结构、World/SDK/renderer/report等可接入缺口仍在；API opaque单列，不能混为不可观测 |
| 全程内存/停顿/显示/正式资格 | available单点5.40GiB、当前两短窗 | 全程4.5GiB、完整流式/停顿、OS显示/工具资格、正式三轮仍未授；按371-G延期，不因本窗功能通过自动关闭 |
| Android | 保持用户延期 | 未施工、未授跨平台资格 |

下一继续同一§0.33，不重做已修worker/已通过六档轨迹。先定位当前BM01 update残余并处理明确owner缺陷，再按固定清单补缺的跨owner生命周期故障/压力消费者。只有真实新架构边界证据才回到方案讨论。**现在仍同时存在未完成消费者/观测接入、未达性能与未授资格，不能概括成只剩性能或只剩测试。**

本窗相关实际合同、371总施工、49/54及各入口同步；371保持当前，不归档。终态证据审计与费用闭合不触发新的构建/测试。

## 8. 实际窗口收尾

开工2026-10-22 00:47:26 UTC，收尾2026-10-22 02:49:12 UTC；全窗7306.373秒（2.0295小时），含阅读、实现、失败、构建、测试、等待、证据和文档。未达到3小时硬上限，不以时限解释性能缺陷。45-closeout复核源身份无漂移、B路原行保持、新记录链接有效、无Cargo/rustc/Player/本路测试进程。两个极小temp候选按45-retained-fixtures保留，未伪称policy拒绝或全部清理。S1仍partial，后续沿第7节继续。
