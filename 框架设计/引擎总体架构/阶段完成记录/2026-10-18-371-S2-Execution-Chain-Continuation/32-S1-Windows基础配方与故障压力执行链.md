# 371 §0.33 S1 Windows基础配方与故障压力执行链

本窗为同一大块的续建，**整块及S1/371仍为partial**。已修复普通项目大批量变换首帧返回缓冲不足，接通BM01/BM09普通项目及统一driver，增加实际故障primitive与静态倍率窗口。BM01高dirty完整轨迹仍有真实超时；BM11/BM12完整配方、完整内存观测和正式资格没有完成。当前系统余量要求为4.5 GiB；没有降低其它预算、规模或画质。

开工2026-10-21 11:08:10 UTC；13:53:10停止新增实现/验证，14:08:10硬截止。实际收尾与成本见本记录末节及证据`closeout.json`。A路沿G:/gameEngin、codex/255-capability-catalog原dirty工作区；B空闲。不改安装、宿主、SDK、Physics、游戏V0.0.5。原生生产链仍为兼容装配→engine_render_native→engine_runtime_core，装配直接依赖core。

证据根：`.artifacts/371/s1-recipes-20261021-110810/`。开工`00-baseline.json`中30记录387项输入全部无漂移；最终源码/harness/模块及产物身份、阶段账本见`summary-facts.json`，不能用HEAD或旧DLL的帧时代表当前源码。

## 1. 真实产品修复与保留的边界

### 1.1 native stateful callback的一次性返回缓冲

普通项目10000实体初始化在原1 MiB返回区失败。确定性C ABI首红表明：16384个合法Spatial mutation实际回复3,320,258 bytes，超过1,048,576 bytes；stateful callback已执行，不能扩大缓冲后重放。

原owner `engine_runtime_core/src/project_runtime_native_adapter.rs`现为每session一次分配并复用**8 MiB有界返回缓冲**，保留SDK默认floor。未改变wire/SDK、16384意图限额、上传预算或GPU内存预算。超限仍明确拒绝，绝不重放已执行callback。最小用例对新缓冲一次成功、旧1 MiB负例一次拒绝均检查callback恰执行一次；9项adapter测试通过。16真实Release窗口3帧通过，原callback错误与同时出现的次生geometry错误均消失，没有据次生错误放宽GPU预算。

这是新增每session约7 MiB主机实际存储；完整owner内存观测仍Partial，不能将这项存储当作已全部归属/预算资格。

### 1.2 宿主生成后立刻丢弃的mutation记录

当前K0/10%短诊断显示主要CPU成本在update。审查实际消费者发现`apply_session_output`只消费提交数量、命令及receipts，从未消费`WorldWriteRecord`，但原提交无条件构造before/after Debug字符串及record overlay。

新增最小首红实际计数记录构造，旧宿主1次写入即生成1条随后丢弃的记录。修复由既有Prepared batch共享同一`commit_inner`：公开`commit`保留完整有序记录，私有host路径不生成记录。**prepare/commit的完整校验、generation、结构预算、原子性、空间历史及结构提交均保留**，没有把两阶段检查视为重复后删除。1/16/128/1024规模点中host实际记录构造为0，公开路径为N、before/after连续，最终World相同；失效generation与混合无效写入仍原子拒绝。20项session测试通过。

39构建曾因机械替换误改DeferredPrefabs.commit而失败；已恢复原prefab调用，记录首错并在40/41通过，不隐瞒返工。记录计数仅cfg(test)，没有生产临时计时探针。

## 2. BM01：普通项目与八档真实轨迹

永久生成器/项目模块模板在`samples/platform_3d_benchmark_project/Benchmark/{generate-foundation.py,foundation-track.rs}`；普通项目为`samples/platform_3d_foundation_project`（K1/1%），其它七档源位于本轮证据matrix目录。输入是10000纯空间独立root、一选中camera、20个真实可见HUD，无mesh/light；固定种子57739792，xorshift/Fisher-Yates嵌套集合，非零dirty每tick真实改变translation及rotation。

每档1初始化tick+7200完整轨迹tick，共7201；每次命令通过当前generation实体解析和原SDK mutation原子提交。检查点回读全部10000个实际已提交transform，并检查实际camera；37断言、末帧HUD capture。非检查点`committedMatching=-1`明确表示未采样，不复用旧成功值。

| 档位 | 完整功能轨迹结果 | Player实际耗时/边界 |
| --- | --- | --- |
| K0/0% | 7201tick/7201frame通过（22） | 9.412s |
| K0/1% | 7201tick/7201frame通过（22） | 30.557s |
| K1/1% | 7201tick/7201frame通过（21） | 32.274s；独立HUD位置/颜色20项通过 |
| K1/0% | 最终DLL上7201tick/7201frame通过（49） | Player进程10.682s；含语义验证12.837s |
| K0/10% | 120秒硬超时（22） | 未完成；不是完整轨迹通过 |
| K0/100%、K1/10%、K1/100% | 未执行 | 高dirty首红保留，避免盲重跑 |

前三档通过属于记录优化前确切交付；49补入独立K1/0%低dirty完整功能轨迹，使用与43相同最终engine artifact `7fe900ee174198c140988f9cc2b0c73dcd158d12ab70c7322183adf516c155f9`，未再次重建整个engine。合计四档通过、一档超时、三档未执行；不冒称八档都在最终DLL重新通过。模板已恢复为每tick解析当前entity的实现，`summary-facts.json`核对canonical与全部matrix模板字节一致。

曾试验项目缓存EntityHandle，K0/10%仍120秒超时；同600样本前缀A/B仅小幅收益，**已撤回**。25单测针对临时变体，不当作最终整个模块已测；55已在恢复后的最终模块重新通过2项完整轨迹/selection纯函数测试，沿用已有module cache，编译+执行共1.034s。新生成的module Cargo.lock属于本轮构建产物身份，不将旧sourceRevision写为当前。

### 显式unpaced功能执行入口

原默认Fifo/60下，仅7201次呈现就超过通用120秒总超时。没有放宽超时、64断言或改变默认工具行为；沿既有私有unpaced真实消费者提取`run_bounded_semantic_playtest_unpaced`，显式请求Immediate/target_fps=0。默认入口仍Fifo/60；worker false字段不序列化、true显式传递，旧严格reader拒绝未知字段，不静默降级。不支持Immediate的设备明确失败。19中3项默认/显式、worker传输与target一致性测试通过，21真实Game.exe贯通。

功能轨迹可尽快执行固定tick，不授真实wall、持续P95/P99或正式资格。正式配方窗口及预算仍服从371-A。初次完整场景曾因声明观察值不全在tick3被正确拒绝，已修项目每tick输出未采样值；18的Fifo超时与22高dirty的CPU超时分别记录，不能混为同一首因。

## 3. BM09：统一driver到三锚点实际重定位

`generate-projection.py`/`projection-track.rs`生成普通SDK项目：1000真实mesh root+camera，无HUD；0/10000/1000000m三锚点各初始化+20次实际keep-world重定位，共63tick。10个motion probe先按旧anchor写入再原子迁移，观察实际local误差≤1mm及camera/anchor/round/matching。

28真实Release窗口**63tick/63frame、45断言、3截图通过**，Player7.383s。32项独立像素检查通过：各锚点10个真实运动probe独立透视投影颜色覆盖，三锚点整图最大差0（上限1/255）。这不是新增0.25px centroid资格；精细误差及current/previous、snapshot、GPU寿命依30未变owner证据复用。模块1项浮点测试通过，f32 subtraction以0.1mm量化误差判定，不要求不合理bitwise相等。

## 4. BM11：已接入的实际故障primitive

`engine_runtime/tests/support/foundation_faults.rs`经原platform_3d_foundation integration消费；旧streamed_cells fixture新增extra-prefab参数，旧caller=0保持原内容语义。

| 实际消费者 | 事实与保留边界 |
| --- | --- |
| F01 primitive | 原10000纯空间base+camera，真实Host同tick生成1000 root、请求100个不同prefab；下一tick销毁前500、旧generation失效、后500存活；资源Ready后全部释放，shutdown pending=0/CPU resident=0 |
| F03 primitive | 实际缺失chunk、损坏hash分别ReadFailed/IntegrityMismatch终结，无World发布，CPU归还 |

10两项通过；最终42在real-wgpu真实feature composition下两项再次通过，**没有把feature存在冒称两项都使用GPU ACK**。F01首次被默认loader max_operations=64正确拒绝；按冻结PC配方显式配置256，生产默认保持。其后fixture漏填prefab sub_asset=root被严格身份检查拒绝，修fixture，未放宽owner。

这两项是纯空间/loader故障primitive，**不是BM02/BM03原规模或双32 MiB cell上的完整F01～12脚本**。F02及其余故障schedule仍缺统一实际消费者，不用旧owner单测替代完整配方资格。

## 5. BM12：真实静态倍率准备与有界窗口

`generate-pressure.py`复用原真实几何/PNG生成器；倍率不仅放大descriptor，而是实际实例、不同mesh/material/texture全部同比放大。driver按冻结catalog推导预期，再检查Cook实际内容，不信源descriptor自报数量。

| 倍率 | 实例 | 不同mesh/material/texture | 实例绑定三角合计 | 实际窗口 |
| --- | --- | --- | --- | --- |
| 1× | 10000 | 100/32/8 | 10000000 | K0，3帧passed |
| 1.5× | 15000 | 150/48/12 | 15000000 | K0，3帧passed |
| 2× | 20000 | 200/64/16 | 20000000 | K0，3帧passed |

每mesh600vertex/1000真实tri，每texture真实非恒色2048²RGBA。静态初始姿态由Scene hydrate，未提高16384 mutation限额；没有HUD或伪造额外负载。31真实反例将一个实例enabled=false，Cook以`benchmark.pressure_payload_ineligible`拒绝，随后恢复原字节；三倍率37在恢复后实际Cook/build/window通过，截图与各自package/module/EXE/DLL身份见summary-facts。

**仅BM02静态倍率的3帧有界消费者**，不是BM12全部完成：未做完整K0/K1、all-visible、BM03、其它领域倍率、耗尽/故障注入或正式压力配方。Cook实例三角总和不等于GPU实际可见绘制量；K0视野未覆盖全部放大布局时不能声称全体同时可见。driver对不支持operation/Android/headless/faultSchedule明确拒绝，所有qualification保持unqualified。

启动加载另有10/14/18次呈现，等待总计318.934/290.681/611.674ms，不算进“3应用帧”而隐藏。原生loading报告maxFrameUploadedBytes各16 MiB、peak staging各16/16/32 MiB、peakInFlight均2；这些是该次有界运行的实测，不授全程峰值资格或零停顿。原上传/staging/在途上限保持。

## 6. 当前CPU反馈、证据复用与出口

以下全部是BM01 K0/10%原配方前缀的**短诊断**：4 warmup+600样本、604tick/frame。使用既有runtime_cli测试exe SHA256 `123A51CD94C996D149B6E1B3BCD26525FCDF25A21C7FFDDE447A5B03F693CB38`（确切值以timing receipt为准），调用真实交付Game/DLL；不重新编译诊断harness。33初版Node JSON roundtrip把0.0变integer而失败，34改用Python保留scalar类型，未放松产品契约。

| 变体 | CPU mean | update mean | renderSubmit mean | GPU mean |
| --- | --- | --- | --- | --- |
| 35原模块/记录修复前 | 29.6146ms | 28.8527ms | 0.2926ms | 0.3368ms |
| 36缓存handle临时变体（已撤回） | 28.4748ms | 27.7660ms | 0.2913ms | 0.3359ms |
| 44最终记录修复/原模块 | 26.8558ms | 26.1089ms | 0.2916ms | 0.2919ms |

44 CPU P95/P99=28.5910/31.2718ms，max=151.5558ms；没有删除长帧。相同配方前缀CPU均值下降2.7588ms（约9.3%），update下降2.7438ms；跨时段环境差异未被完全控制，不把全部差值都声称为单一代码项的严格因果收益。CPU预算仍明显失败，不能靠继续重复跑挑绿。原DLL SHA256 `7718f8a079fb8df2cf57eb4e8c09a6f85f48c27e2165c5b46eaead245c29c8bf`，最终DLL `c9533aa62843e34be46df9bd596376f8f32bccc97a7a34cf98a2ec493f6e234b`，player/package/module及scenario见各自receipt。

当前证据已排除“剩余都在GPU或渲染提交”的判断。记录构造是确认的可删工作，尚不能把整个update成本全部归于它。43只build，不执行Player；44短窗运行期间没有并行构建/检索/编辑。短窗不授完整K0、高dirty持续预算或正式资格。

57对照核对：前后项目module descriptor相同，RuntimePackage各27文件中26个实际内容文件字节相同，唯一差异为build-runtime-package-report.json；Game.exe及engine DLL身份确有变化，分别保留，不冒称固定Player字节未变或仅替换DLL的单变量实验。

46/47为一次有界owner诊断：真实10000纯空间root上分别写1/100/1000/10000实体、各10次，分开计时buffer构造、prepare、host commit与空间传播/dirty drain。dev-fast在1000项为0.6007/22.2181/27.1491/16.7266ms，10000项为6.6777/222.1196/265.7631/158.7065ms，未见平方级膨胀。**这是非优化owner诊断，不能当Release的26ms阶段占比，也未覆盖native JSON/FFI回调成本。**46初版因RuntimeSceneDocument/RuntimeScene类型不符编译失败，47修harness后通过。没有据此删除任何prepare/commit必要校验或继续堆缓存；临时测试已恢复原字节并以当前mtime写回，48-probe-restore记录SHA相等。下一次应在真实Release update中进一步区分callback/wire、prepare、commit及propagate，现有证据尚不能确定剩余26ms的内部首因。

30的spatial数学、投影、snapshot/GPU完成退休owner未改部分按身份复用；宿主提交变化覆盖20项session、两个feature故障消费者及真实Release短窗，不将旧84项整体改称在最终integration重新执行。31的4.5 GiB判定证据复用；本窗未取得全程系统余量/内存峰值资格。27旧受光原规模DLL帧时仍只属于其确切旧来源，不迁移为本轮BM01结论。

剩余：BM01高dirty update主消费归因/整改及未完成档位；BM11完整故障schedule；BM12完整压力消费者；完整owner观测与371-G正式资格/显示关联继续明确延期。Android延期。没有证据要求推翻3D渲染架构，本窗确认的是普通模块执行/主机提交成本和配方消费者缺口；若后续需新架构边界，另按现有讨论流程处理。

## 7. 成本与收尾

构建、测试执行、Compiler导出、Cook/准备、Player及生成各自计时，失败和撤回实验全部保留。`summary-facts.json`列各attempt及已记录阶段；Compiler buildMs包含导出工作，**不是纯Rust编译**。不把outer/driverWall与其嵌套阶段重复相加；失败未产完整stage report时保留outer和已写stage，不记为零耗时。

已记录成本（秒；含首红、返工与撤回实验）：

| 类别 | 实际耗时 | 口径 |
| --- | --- | --- |
| owner/integration测试构建 | 556.008 | Invoke-EngineBuild的Test build，不含执行/Clippy |
| 同批测试实际执行 | 39.642 | 包含失败/临时诊断，测试组之间有重叠，不按次数授新资格 |
| scoped Clippy | 131.765 | core/CLI/authoring lib与real-wgpu foundation integration |
| Compiler导出构建阶段 | 2028.115 | 全部已落盘build-timing，含失败；不是纯Rust编译 |
| 已完整报告的Cook/快照 | 100.487/8.173 | 另有失败只写outer，不能将未报告阶段当零 |
| Compiler内Player阶段 | 507.580 | 含120秒超时与验证/启动；不重复加其外层 |
| 真实短诊断33/35/36/44 | 6.417/24.623/23.726/23.252 | 含harness失败；沿现成exe，不新编Compiler |
| 三倍率源生成 | 43.820 | 与运行/构建分开 |
| 最终项目模块编译+测试 | 1.034 | 已有独立project-modules缓存，test profile，非native快测 |

15/20/26直接CLI构建wall分别2.833/21.651/2.740秒；03最初CLI与12独立项目编译等仅保留Cargo日志（约8.95/21.41秒），不伪造精确独立阶段。早期module缺lock的2.96秒失败与随后生成lock/测试、格式返工均保留原输出。已记录Cargo build与Compiler export约43.07分钟（不含Clippy和单列直接Cargo），加Clippy约45.27分钟；这些是阶段合计，不是整个窗口或可省时间。不能与上一轮不同改动规模直接当受控构建速度比。

54最终15文件fmt通过；53曾因错误mod.rs路径和投影模板格式而失败，已修选择器/模板，不触及实际投影语义。45/50/51/52 scoped Clippy退出0，保留既有及可见警告，未抑制/不宣称零警告。额外Python生成器语法/文档链接与身份收尾检查见最终audit；没有重跑workspace、BM03、正式三轮、原全部84项或无关renderer suite。未做安装/游戏迁移，未修改任何宿主配置。

全部临时生产探针为无；47测试诊断已原字节恢复。失败证据与run-owned源/缓存保留用于下一窗复用，没有尝试此前policy拒绝的清理目标。closeout记录实际起止及无在途本路构建/Player。**本窗收尾仅结束本次窗口，当前总施工不归档；四个大出口中BM01高dirty、完整故障与完整压力仍未闭合。**


实际收尾：2026-10-21 13:39:55 UTC，整个窗口 151.75 分钟（含读取、实现、失败、构建、运行与收尾）。最后不再启动新的Release探针分支：本窗一次core导出约7.7分钟，新增受控变体与恢复无probe终验至少两次导出约15.4分钟，另需运行/取证；已无法在13:53:10停止新增前闭合该分支。保留已定位update及尚未确定内部首因的事实，不盲重建或将部分结果授为整块完成。59进程快照无本路相关构建/Player在途；当前总施工继续保留。收尾脚本初版未显式指定UTF-8而触发Windows默认GBK读取错误，现已显式指定编码，未影响产品或验证结果。
