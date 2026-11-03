# 371 §0.26 稳定模板寿命与 native 绘制接管（20）

日期：2026-10-20。状态：`requested_scope_source_consumer_verified / large_block_partial / scale_budget_failed`。本窗指定的模板寿命分离、native 实际模板准备和编码消费已贯通；原方案第一大块完整出口、整条 CPU 整改、S2-D/S2/S1 与371仍未完成。不能把这次迁移和335项通过当作性能闭合。

## 授权、范围和身份

用户授权“稳定资源模板寿命分离与 native 绘制模板端到端接管”，沿[正式整改方案](../../371-S2-常驻绘制与并行执行整改方案.md)及总施工§0.26第一大块实施，没有重新建设共享表、slot、proof、instancing、bundle或GraphScheduleCache。06:29:26 UTC开窗，09:14:26停止新增/09:29:26硬截止；实际关闭时间见证据根`33-closeout.json`。原dirty分支`codex/255-capability-catalog`及F盘target/`--locked -j1`复用。B槽空闲，V0.0.6源码发布与游戏固定V0.0.5保持。

证据根：`.artifacts/371/s2-native-template-20261020-062926`。激活时前窗36源98项输入零漂移；最终`24-source.json`声明99项输入，六个本窗产品/测试文件修改或新增。此身份是声明范围的开发证据，不是whole-workspace或clean exact-commit发布资格。

最终无probe `28-engine_runtime.dll` SHA256：`6fc06f7c5462b54ea0aae480dcb1b199a4db9e5cd7e7c6f274a2dcdaf6888486`。原包、场景、Player和module复用，module SHA256仍为`a35dada34c75587ab021bc2d10270514b672281c8515a6b79f126f4659c16a6f`；scenario/package/player摘要见29-final/result.json。没有Compiler/Cook、BM03、workspace、正式三轮、安装、宿主配置、SDK或Physics修改；Android及371-G资格延期。

## 已贯通的实际链路

| 原owner | 本窗实际实现与消费者 | 寿命、失效和回退边界 |
| --- | --- | --- |
| Production / InstanceReuse | 命令表移至原InstanceReuse，由PreparedInputs共享；工作集、实例表和帧lease替换不再创建另一张稳定命令表。真实Production用例按4/32/256次替换检查同一命令/格式事实、0新增模板及旧lease释放 | 没有第二场景/资源registry；当前帧不可变，旧实例静态成功证明不随资源模板传播 |
| retained_commands | 来源改为geometry、geometry binding、material binding、两份provenance五个确切Weak分配，加完整submesh范围与原raster/pass context；不再绑定持lease的DrawResources分配 | Weak阻止地址复用但不保留实际资源/lease；材质、几何、source、range、镜像和pass变体变化失效；容量65536，满时严格回退，不拒绝合法draw；清理移到freeze边界，不对每个溢出draw全表扫描 |
| GeometryGpuState / native_template | 新私有模块保存实际pipeline、mesh buffer/池页offset、index范围/base vertex、material binding及纹理/环境依赖；在原完整准入后，按唯一模板准备。当前模板工作集在原GeometryGpuState，memo只有Weak | 保留事实/环境分配防ABA；每次重准备核对实际当前GPU对象和池页范围，CPU命中不是GPU准入；当前工作集上限65536，原失效、streaming unpin、device destroy释放；旧recording按原真实completion退休 |
| Graph / RHI / native encoding | 继续透传原共享batch/范围与不可变事实；纯几何plan保存对齐native模板引用，编码直接消费。混合Sprite/粒子等plan不获得geometry-only整plan证明，合法geometry范围也消费同一native memo | 纯路径不再重新找pipeline/mesh/material对象；混合与detached保持当前核对及严格兼容。native Batch持模板Arc、动态light/shadow binding和instance range；独立旧标量consumer仅为cfg(test)参考 |
| 内存观察 | 原snapshots/renderer观察实际命令/模板payload、引用Vec、纹理依赖Vec及实际buffer/texture，退休recording保留真实依赖，原去重归属继续生效 | 哈希桶、Weak控制块和准备scratch明确为尚未接入，不是“平台不可观测”；不以reservation/估算或进程总量冒充完整owner backing |

完整raster/context、当前generation/source、透明顺序、相机裁剪、阴影动态数据、完整selection tail、跨batch唯一性、部分上传、容量/队列/上传上限及旧在途帧均保留原owner。GPU内部layout、shader和外部wire本窗未改。

仍保留薄的逐批DrawGeometry动态head、合法detached/混合兼容和逐帧plan准备；没有声称这些分配或全部旧消费路径已退役。

## 首红、返工与终验

| 证据 | 首次失败、原因与处理 |
| --- | --- |
| 01 / 03–04 | 真实Production在scale4第0次工作集替换丢失稳定命令；修复缓存所属寿命及弱来源后通过，旧lease可释放 |
| 04-native-red / 07–11 | 已完成prepare，编码仍解析1次native对象，期望0；native模板准备、对齐引用及实际编码接管后通过 |
| 06 | 新ValidatedGeometryPlan字段漏初始化导致check失败；在原struct初始化修复，保留stderr，不记作运行失败 |
| 12 / 13–15 | 混合Sprite范围仍重新准备1次native模板；加入原GPU owner的有界当前模板工作集，真实完整混合帧连续绘制通过，仍无geometry-only proof |
| 17 / 23 | 终验231过、2失败，停止后续：新native capture又调用材质解释，原共享键计数129而非1；改为消费既有Requirements纹理事实，恢复1。另一个观测fixture漏计新增的native引用数组实际backing；明确新增数组计费后多规模发出数、唯一数与预期一致，不削减覆盖 |
| 22 | 临时Cargo命令误传两个TESTNAME，参数解析失败，无编译/测试结果；改成一次lib no-run后直接exe多过滤，不更改产品或断言 |

最终24源/26终验：owner233、visual54、U2 28、GPU lifetime5、geometry GPU15，**335项**全部通过；不是把各次owner窄例、231项旧通过和335项相加。包含真实像素、材质、阴影、旧帧/退休、错代、COW、完整tail、容量及部分上传消费者。

多规模实际native工作计数：4/32/256个批次均首次解析2份模板，整plan重新准备新增0、当前对象核对2；prepared编码再次解析对象0、共享模板走旧稳定描述比较0。混合帧实际render两次仍新增0。移动池页offset即使buffer相同也拒绝旧模板；释放当前plan/工作集后Weak memo不能保活模板。

27质量：六文件edition2021/skip_children格式检查、scoped real-wgpu Clippy lib/tests及default库check通过。既有lib152/lib-test172 warnings与前窗相同，未改ledger或声称零告警。

## 原规模反馈：预算没有闭合

CPU预算使用`cpuExcludingPresentWait`；总帧`meanCpuFrameMs`含Present等待，不能混用。此前对话中24.8260/26.5238ms为总帧数字，已更正；原始报告不改写。

| 同原K1、无probe身份 | CPU不含Present mean / 观察P95 ms | GPU mean / 观察P95 ms |
| --- | --- | --- |
| 前窗36源 / 39 DLL / 40反馈 | 24.9174 / 26.7368 | 7.6032 / 7.6708 |
| 本窗24源 / 28 DLL / 29反馈 | **24.5812 / 26.2381** | **8.3425 / 8.3845** |

CPU均值下降1.35%，观察P95下降0.4987ms，**没有证明明显或持续的整帧改善**；CPU仍超过P95≤13.3/P99≤16.67ms原预算。GPU本次上升，不将波动归因于未经对照的因素。总帧mean/P95为24.8260/26.5238ms，update1.2437、renderSubmit23.3074、Present等待0.2448ms。

每次仅12帧、4预热、8有效样本，不授持续P95/P99、原7200配方或正式三轮资格。保持100 resident meshes、10000 instances、128 materials、96张2048全mip BC3、10M triangles、145 lights、4 cascades+8 spots、1080p/far2000及原运动轨迹；K1不是S1的10000 unique meshes资格。

启动前实际可用物理内存5.79395GiB，低于原6GiB，只是单点，未做全程引擎/宿主/环境因果归因。没有关闭其它应用、扣减进程工作集或修改门槛。加载40次present、必要等待0.5187秒；单帧上传峰值16MiB、staging32MiB、in-flight2。它们不是全运行内存/流式资格。

## 当前源码的一次有界分解

29无probe收益不足后，30-diagnosis对24源做一次11文件临时probe；12帧完整、stderr3886bytes未截断。结束后11文件逐字节恢复，99声明输入零漂移，复用28/29无probe证据，无须再构建挑绿。**以下分段属于本窗24源**；19记录的24源/26 profile是另一个证据根的历史源码，不能混用。

| 当前24源区间 | 均值ms | 边界 |
| --- | --- | --- |
| Production | 9.9713 | workset0.0006、输入引用0.0032；sceneLighting6.4450；颜色及其它余量3.5225，未细分不得直接归给一个函数 |
| sceneLighting任务组 | 1.9784 | 父线程墙钟，含spawn/等待/合并；不叠加worker墙钟 |
| sceneLighting其余规划/合并余量 | 4.4666 | 仅父区间减任务组；没有证明某个算法独占该余量 |
| Graph | 2.5338 | append0.1671，compile1.2213含declarations0.9427；余量1.1454未细分 |
| native准备累计 | 6.7211 | 包含nativeExecute内同帧reuse，**不可与执行3.7352ms直接相加** |
| native准备内部 | validation1.1216 / resources2.8852 / parameters1.2922 / capture1.3806 | 这些是准备子项；resource scan1.4968、Lit binding0.6192嵌套于resources |
| native模板准备 | 1.1935 | 嵌套于capture；当前对象核对0.5876嵌套于模板准备；并非每帧创建native对象 |
| native执行 | 3.7352 | intent1.3412、geometry encode0.9790，其中recipe capture0.4831；其它余量1.4150未细分 |

完整工作量：每帧**producer命令创建0、格式解释0、native模板capture0**；12个任务、29657.125 selected rows、9897.25实例参数值、11种view参数值。native当前模板核对**2417**次；6124.5是含reuse的准备入口命令长度和，不是两次完整准备；11参数变体也不是全部输出view数。

判断：寿命分离和native接管在真实K1中生效，未完成的重活仍是Production动态阶段工作、逐帧原生准入/参数/模板引用准备及Graph消费。稳定模板的准备与核对也有成本，不能只从编码区间下降承诺整帧下降。现有证据支持“整体迁移仍未收敛”，不支持再次重建同名模板缓存，也不支持把全部时间归因于GPU、平台显示或单个未计时函数。

## 构建与验证成本

计时明细为32-costs.json，包含失败和中间身份；没有把最终一次构建冒充全部施工成本。

| 阶段 | 已记录墙钟 | 口径 |
| --- | --- | --- |
| 7次owner/consumer no-run构建 | 1590.09s | 含中间09的437.51s和最终25的193.13s；全部计入，不重复加最终构建 |
| 两个Cargo首红构建+执行 | 279.83s | 01/12，执行嵌套其中，不再计一次测试 |
| check累计（含06失败） | 89.94s | 含最终default17.77s |
| Clippy / fmt | 87.53 / 0.74s | 最终fmt0.25s已包含；无全workspace |
| 已单独计时的测试执行累计 | 125.03s | 最终335项69.44s已包含 |
| 无probe Release / 一次probe Release | 395.41 / 360.87s | 两种身份分别记录 |
| 无probe反馈outer / probe反馈outer | 52.18 / 35.87s | 无probe child16.06s、outer残余36.12s，均嵌套于52.18s；残余含启动前后，不全部称为准备或编译 |
| 22错误Cargo调用 | 0.13s | 未执行测试，不算性能失败 |

连续构建仍占主要施工成本：混合帧首错及终验暴露的材质重复消费使候选身份变化，发生必要返工；09提前构建消费者后产品又变化，属于本窗可避免的准备浪费。后续应把纯/混合及共享材质计数在同一owner候选内先收敛，再构建终验，不能把本窗重复准备当作永久必跑流程。

## 剩余出口与继续点

| 原大块 | 状态 | 尚缺 |
| --- | --- | --- |
| 1 常驻命令与动态参数 | partial；本窗指定的寿命与native接管源码/消费者出口已闭合 | 薄动态head与逐帧plan收集/引用整理仍在；未证明整条动态准备已只消费必要变化，CPU预算未闭合 |
| 2 可见性/阶段并行 | partial；沿用既有12个有界shadow任务 | Production其余动态颜色/灯光/阴影规划、排序及发布的主要工作尚未整体收敛；层级粗裁剪必须有真实访问与收益证明 |
| 3 生产接管/旧链退役 | partial；纯/混合native消费者已接管本次模板 | 旧逐批表达/合法兼容路径、Off/明细隔离与S2实际配方出口仍缺；不能因cfg(test)标量参考存在就认定生产仍在执行它 |

下一整块继续§0.26已批准范围，围绕**Production动态阶段工作集与native逐帧准备合并收敛**：先把Production颜色/sceneLighting余量和native准入/参数/模板引用准备排成互斥成本及真实工作量，再合并处理主要成本；复用已有效的稳定模板，不再逐字段局部缓存后重建全链。可见性任务/依赖按原正式方案推进，任何必要动态检查、顺序、预算与completion结果不删。4.4666ms等未覆盖余量先有界定位，不预认定根因。

源码正确性、当前短窗性能、S2实际配方、371-G延期资格和已安装可用性分别报告。Android、完整内存观测、OS显示及正式三轮仍按371-G延期；不重新变成当前CPU开发全量前置。F盘Cargo target的release DLL现为临时probe产物，**不得当作生产候选**；可复用的无probe候选仅为本证据根28身份。无probe产物未安装。未执行清理绕过；证据保留，最终进程/文档/哈希核对见33-closeout。
