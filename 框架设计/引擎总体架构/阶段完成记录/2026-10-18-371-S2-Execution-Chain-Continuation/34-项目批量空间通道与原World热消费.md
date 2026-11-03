# 371 §0.33 项目批量空间通道与原World热消费

本窗收尾，状态为 **source_consumer_verified / high_dirty_budget_failed / formalQualified=false**。用户已选择33记录方案A，已审改[实际合同B2](../../371-S1-B2-项目批量空间通道与热消费合同.md)后整体实施；批准的批量功能链已贯通，不再将SDK/module边界写为待批准。100% dirty仍超预算，同一整块、S1及371保持partial，不以局部功能通过宣称全部完成。

本窗2026-10-21 15:17:56 UTC开工；18:02:56停止新增，18:17:56硬截止。沿G:/gameEngin、codex/255-capability-catalog原dirty，保留其它工作。Windows only、Android及371-G延期，4.5 GiB=4,831,838,208 bytes、CPU/GPU P95≤13.3ms、P99≤16.67ms及原规模/轨迹/画质保持。开发SDK源码变更，安装/宿主/SDK安装包/Physics/冻结发布/游戏V0.0.5保持。

证据根`.artifacts/371/spatial-bulk-20261021-151756/`，以下E指此目录。开工原8源匹配；阶段身份分别绑定产品源码、测试、原项目、模块、Player、引擎DLL和scenario，不授clean commit资格。

## 1. 合同及实际纵向链

- ABI minor 1.6→1.7，追加可选冷解析callback，schema/digest同步；短表先读size-prefix并拒绝，不向旧reader静默发送新语义。
- Project Game SDK返回opaque SpatialEntityHandle，复用原World epoch/slot index/generation；冷批量解析、热typed TRS写入，完整尾项校验后才追加。raw slot generation从0开始，0合法；不是旧packed incarnation的非零约定。
- Generated Glue、session SDK、native adapter均使用有界AIFESB01 envelope，metadata保留JSON，60字节热记录显式little-endian。顺序、混合命令、容量、截断/额外tail及恰一次callback检查保持；无热输出继续旧JSON。
- 普通BM01项目一次解析10000空间节点和camera，每帧提交typed批次。仍为10000隐藏空间实体/20真实HUD、原dirty比例及K0/K1轨迹，独立回读实际值，不以writer自身数据冒充已提交结果。
- 原mutation buffer与World仍是唯一事务owner。纯热即时提交在一个独占World借用内共享最终评价；混合/Prefab/结构及公开延迟prepare继续完整重校验。

成熟实现实际源码核对见E23-mature-source-extracts：Unity `Runtime/Transform/ScriptBindings/TransformAccessArray.bindings.cs`的typed批量native binding，以及Bevy `crates/bevy_transform/src/systems.rs`的`sync_simple_transforms`与无父/无子条件。文件SHA/行段已保存；只采用冷解析/热typed与简单实体分流，不声称掌握Unity未公开native内部吞吐。两次外网raw连接失败亦保留，未伪造远程研究结果。

## 2. 实测首因及原owner修复

E09真实求值首红：规模1/100/1000/10000对应2/200/2000/20000次求值。E10/E20修为N次，保留camera祖先、pending dirty、跨World、旧generation、同ID重生、非法tail与共同预算拒绝。

初次实际Release仍失败后，E34对当时源码取得60样本互斥/嵌套完整归因：adapter4.3332ms（内含wire1.8241、convert2.2139）；transaction84.1102（内含hot_commit83.8406，其内overlay32.6645、components15.7593、publication33.3592）；begin_tick5.4943、presentation13.5604、extract13.8739、hash0.4883ms。嵌套项不能重复相加，probe frame均值122.7236ms只用于归因。180001次source查表/10000次求值每帧。E34-probe-restored原字节恢复、刷新mtime、删除临时module，之后才做以下行消费修复；此分段不能冒称最终源码耗时。

E35真实提交内部source查表首红为11N。简单根实体无pending dirty时，在原slot的当前archetype/row内做完整校验、原列替换，共用原数值评价和单一timeline发布owner；树及复杂状态回原路径，不保存row到下一次提交。E37绿为N次求值、0次source查表，规模1/100/1000/10000均检查；这不是0次身份检查。

E43提取首红：每实体两个dirty记录产生6次source查表。当前行确认无任何mesh/sprite/legacy renderable且没有旧proxy时，无绘制输出可发布，直接结束此记录；E44绿为每记录1次查表。保留simulation/history/presentation；有旧proxy必须继续移除，重新挂回mesh及despawn由E50验证，2D/实际drawable消费不省略。

行路径修复后的E65再次归因绑定53源码：60样本、64帧完整日志，每帧exclusive之和严格等于frame inclusive。frame均值53.3361ms；互斥顶层adapter4.1645、transaction24.0255、begin_tick5.3069、presentation13.6677、extract4.8313、hash0.4998、observe0.0806、snapshot0.0015、自身余量0.7584ms。transaction内hot_commit23.5816、row_commit23.5802；row内prepare4.4051、evaluate2.7058、write2.0954、publish14.3703ms。presentation内evaluate8.8382ms。真实求值10000、source查表30001次/帧。wire1.7131ms含module1.3784、decode0.3304，已不是33记录约30ms的瓶颈。嵌套不相加，probe不授预算；E65已恢复全部12文件和53身份。

依该证据继续合并收敛原timeline owner，没有追加新架构：私有poses/presentation用哈希查找，active/pending/disabled用哈希集合；active drain后复用容量，候选只借用ID并统一排序去重，外部SpatialDelta仍按runtime ID、PresentationDelta仍按source ID排序。共同发布owner使用单次spatial_world entry定位；调用域已由affected set/final overlay保证唯一，仅查询已有RenderState通知，不再为每个本次新通知构造第二棵集合。

E66真实首红：独立叶根两次presentation写祖先cache为2N，规模1/100/1000/10000得到2/200/2000/20000。原相同数值评价保持，独立叶根不再写无人读取的递归cache；E67/E68绿为0。仍验证两档alpha的实际位置、不可变旧pose及排序；树继续原祖先cache。它省的是缓存写入/克隆，不是必要的插值、坐标或当前资源检查。该组最终源码为E74；**E65分段属于它的修前身份，不能当最终源码分段**。

## 3. 正确性、失败与复用

ABI最终7项、session SDK62项、Game SDK22项及platform_3d_sdk 5项通过，真实普通项目Compiler导出继续验证Generated Glue/ABI实际组合。原session终验E38为106通过/1失败，唯一失败是新增夹具使用了canonical合同本来拒绝的1e30缩放；改成合法中间值后E39窄例通过，不放宽产品数值校验。E44提取11项、E50移除/挂回/despawn1项、最终E51 foundation含真实GPU60项通过；有重叠，不能机械相加为去重数。

保留失败记录：旧ABI minor断言、初次夹具遗漏camera、错误拒绝raw generation0已修；E09/E35/E43为真实工作首红。E19宿主编译成功但没有显式ignored，runner正确拒绝0测试，后续使用同产物显式ignored实际执行；147.55s主要是宿主准备，不全算可避免浪费。probe脚本首次Python语法错误未改源码，随后真正probe check/export通过。E36 method可见性、E42 fixture相对路径、E45/E46测试API/转换编译错误均保留并在窄例修复。普通实现错误没有扩到全workspace调试。

timeline组最终E69 session 108项、E70 foundation含真实GPU60项通过；E73 fmt与core scoped Clippy通过，两个新needless_borrow告警已修，core保留148 warning，不宣称全部为历史警告。SDK/Game SDK Clippy有效输入不变，按身份复用。未修改GPU资源owner，基础GPU完成点/共享资源/退休消费者按风险复用并由E70实际组合覆盖；不把这些证据外推为所有新DLL的完整压力资格。E69/E70后只有等价借用清理/格式调整，用E73确认，未机械重复同一大suite。E62 probe前置检查发现原session helper有4个调用而非1个，写入前拒绝，修正预期后执行；E72格式检查捕获借用清理后的格式变化，E73修复。均保留记录，不混写成产品运行故障。

## 4. 普通Release与性能反馈

最终timeline组普通交付75/76/77、绑定74终源及78身份。以下均为不删除长帧的无probe反馈，单位ms；各源码依次变化，不是重复挑绿：

| 源码/配方 | 样本 | CPU mean | P95 | P99 | update mean | GPU mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| E27本环境旧当前DLL，K0/10% | 600 | 16.7676 | 18.8238 | 21.3236 | 16.3277 | 0.2942 |
| E28首轮typed通道，K0/10% | 600 | 12.2961 | 14.4277 | 16.7859 | 11.8819 | 0.2452 |
| E29首轮typed通道，K0/100% | 60 | 119.6258 | 124.7763 | 225.6548 | 117.5577 | 0.3132 |
| E58行/提取版本53，K0/10% | 600 | 5.7013 | 6.6214 | 7.6672 | 5.3197 | 0.0773 |
| E59同53，K0/100% | 60 | 56.2822 | 62.4025 | 165.5146 | 54.1775 | 0.1000 |
| E60同53，K1/10% | 600 | 6.1081 | 6.9616 | 8.1363 | 5.5364 | 0.0870 |
| **E78最终74，K0/10%** | **600** | **4.3307** | **5.1549** | **5.8510** | **3.9554** | **0.0758** |
| **E79最终74，K0/100%** | **60** | **48.7419** | **50.0518** | **139.5040** | **46.8721** | **0.1202** |
| **E80最终74，K1/10%** | **600** | **4.4767** | **5.5850** | **6.8082** | **4.0201** | **0.0651** |

53对应DLL SHA256 `49c680d52d5f7d7a9686df4359605828cc2b76b377c71183b7f86f29ac7d5e3b`，普通项目交付54/56/57的引擎DLL和Player相同；各module/包/scenario独立绑定E58-final-identity和各timing。E61同53的K1/10%完整7201 tick、37项独立断言、末帧20真实HUD捕获通过，50.012秒；measurement Off，只授功能周期。53在10%短窗P95/P99低于预算，100%仍失败；未挑掉94–165ms的CPU max，也未盲跑100%完整轨迹。33记录190.5078ms属于历史时段，不能将跨环境差异全部归为代码收益。

E26启动前系统available单点4,145,307,648 bytes，低于4.5 GiB；没有关闭其他应用或扣减工作集。不据此判断全程内存，也不把它混写成产品分配归因。

E58启动前单点4,594,540,544 bytes仍低于4,831,838,208；它与预算分位值反馈分别报告，不授全程内存/正式资格。BM01本来就是10000隐藏空间实体和20 HUD的空间执行配方，GPU低成本不能外推为完整PBR/重几何3D场景达标。timeline新哈希桶backing尚未逐项接入owner观测，明确是可观测但未接入，不能写成平台不可观测或完整内存覆盖。

最终引擎DLL SHA256 `6af094771b6befddabb277af10d79247318f200d54c982934f5be0e4e174bcc0`；三项目Player和引擎DLL一致，项目module/package/scenario及22项来源分别绑定E74/E78及各timing。主交付`E/projects/K0-10/Build/75-timeline-final-release/release`，另外两个分别76/77；均为普通Compiler产物，没有手改bundle或替换DLL。引擎cache分别rebuilt/hit/hit。最终K1完整7201 tick/37断言/捕获再次通过，39.873秒，measurement Off。E78启动前系统available单点4,573,691,904 bytes，低于原4.5 GiB，未关闭应用/改配置或扣减工作集；不授全程归因。

同本窗K0/10%旧当前DLL均值16.7676→4.3307ms，约下降74.2%；首轮typed的100%119.6258→48.7419ms，约下降59.3%。Player/源码和SDK组合有变化，属于同配方前后反馈，不把全部差值严格归为单个容器修改。最后一组相对53版本100%均值56.2822→48.7419ms，改善约13.4%，仍远超预算；10%/K1的短窗分位值通过不改变此独立失败。最终CPU max保留101.5347/139.5040/95.3905ms；没有截去capture/外层余量。100%当前update46.8721ms、submit0.3009ms，明确仍为更新链成本；E65不是最终分段，最终各细段残余需后续定界，不能继续原样引用14.37/13.67ms。

## 5. 出口、费用与继续点

| 出口 | 判定 |
| --- | --- |
| A合同/审核 | 已审改B2、原S1-B替代边界及总施工自审；Unity/Bevy实际源码映射和限制留证 |
| B通道 | ABI/SDK/冷解析/有界热编码/生成胶水/普通项目实际module已贯通；旧reader明确拒绝 |
| C热消费正确性 | 原World行提交/单次评价、共同timeline、presentation/Projection原消费者已接通，取消/终态/代际/尾部/原子性和旧Arc边界保留 |
| D原规模反馈 | partial：10%静止/运动短窗低于预算且完整运动功能周期通过；100%短窗独立失败，未跑其7201tick/完整矩阵/正式三轮 |
| E证据/文档 | 源码冻结、两组probe恢复、产物绑定、费用及导航收尾；总施工不归档 |

剩余实际缺口仍为BM01高dirty CPU预算及未齐的完整矩阵，BM11逐故障和BM12完整压力消费者尚未齐；371-G/OS/Android继续延期。基础功能不是完全未实现，当前SDK/wire分层已落实；剩余证据指向原空间更新/历史/表现/提取的高逐实体常数，不能把它泛称GPU慢，也没有证明O(N²)。后续在同一A范围先定界74终源update，再评估按已有runtime slot/generation连续消费空间状态和发布数据；不重复建立ABI/bulk/renderer缓存，不承诺单纯换哈希表即可闭合13.3ms。

本窗已按首因完成两组主要owner整改和真实消费反馈。更深的跨空间状态/发布布局改动需要完整定界及实现/验证预算，不在本窗收尾前堆一个来不及验证的分支。没有因为达成局部缓存就宣布整块完成，也不把时间上限当作性能失败原因。

费用见E82-cost-ledger：定向Cargo build/check调用累计1164.960秒（19.42分钟），Clippy106.541秒（1.78分钟），测试执行199.913秒（3.33分钟）；另driver构建112.568秒。Compiler共11次外层2485.293秒（41.42分钟），其内嵌build/export2443.351秒已包含，不能另加；三组普通产物与两组诊断各按实际变化刷新，末组426.351/15.757/15.620秒。13次Player/诊断外层213.087秒，包含启动/验证，不全是采样。各调用类别含Cargo锁等待及少量交叠，不相加冒称独占墙钟；有UTC的调用区间并集4121.270秒（68.69分钟，driver缺UTC另列）。整个窗口含阅读、实现、分析和文档，以E85实际closeout为准；失败和返工均已计入，没有完整workspace/BM03/正式三轮。


## 6. 实际收尾

实际结束2026-10-21 17:32:53 UTC，窗口134.96分钟（含阅读、实现、构建、测试、等待、分析与文档），未触发三小时硬停止。最终22项来源再次核对一致，E34/E65两组probe均已恢复原字节并刷新mtime、临时module不存在。最终HUD与53版本逐字节一致，17个新增文档链接已核验；49/54、B2实际合同、总施工、阶段/施工索引、地图/阅读顺序及根README同步，B路374状态/登记原样保留。无本路在途进程；证据与备份保留，未重试或绕过历史policy拒绝目标。总施工保持当前，功能链已贯通但高dirty预算/其余出口未闭合；下一窗口继续同一A范围，不能重新标为待批准或把本窗结束宣称为S1/371完成。费用和终态绑定E82/E85。
