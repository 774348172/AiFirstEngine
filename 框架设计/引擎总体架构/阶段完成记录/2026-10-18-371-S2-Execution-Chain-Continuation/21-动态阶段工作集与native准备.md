# 371 §0.26 动态阶段工作集与 native 准备（21）

日期：2026-10-20。状态：`source_consumer_verified / large_block_partial / scale_budget_failed`。本轮合并修改已被生产消费者采用，有一次无probe整帧反馈；CPU均值下降、观察尾部未改善。原整改三个大块、S2-D/S2/S1与371均未完成。

## 授权、边界与身份

用户“按照你的建议开始下一整块的施工”授权继续[正式方案](../../371-S2-常驻绘制与并行执行整改方案.md)与总施工§0.26的“Production动态阶段工作集与native逐帧准备合并收敛”。08:00:09 UTC开窗，10:45:09停止新增/11:00:09硬截止；实际关闭时间和总时长见15-closeout.json。本轮按有界实现/终验/一次反馈出口收尾，未因时间上限中断，也未宣称原整改整体结束。原dirty分支`codex/255-capability-catalog`、原目录与F盘Cargo target/`--locked -j1`复用，B槽空闲。

证据根：`.artifacts/371/s2-dynamic-20261020-080009`。激活99项前窗声明输入匹配；最终09-source.json仍声明99项，8文件改变（7文件产品/测试、production.rs仅新增测试）。声明身份是开发证据，不是clean exact-commit或全workspace资格。

无probe候选10-engine_runtime.dll SHA256：`f47c097ebc11fece1ac11217eb0374e30dcf069951c517c7e55be64df0c5e301`，未安装。包/module/Player/scenario身份未变，见12-comparison.json；module SHA256仍为`a35dada34c75587ab021bc2d10270514b672281c8515a6b79f126f4659c16a6f`。不跑Compiler/Cook、BM03、全workspace或正式三轮，不改SDK/Physics/安装/宿主/V0.0.5或V0.0.6快照；Android及371-G继续延期。

## 一次有界归因：属于修改前源码

01-diagnosis基于00源，一次13文件临时probe/64计时槽，12帧、4预热、8有效样本，完整stderr5664bytes未截断。13文件逐字节恢复、99输入零漂移后才实施产品修改。以下**不是最终09源profile**，不能把修改前数值减去理论并行收益来伪造最终分解。

| 修改前区间 | mean ms | 边界与判断 |
| --- | ---: | --- |
| Production | 9.9708 | sceneLighting6.3438；颜色及其它余量3.6230未完全分解 |
| sceneLighting前置 | receiver1.1932 / caster1.1491 | 独立只读输入，当时串行；完整输入和错误语义保留 |
| sceneLighting其它 | cluster0.1112 / planning0.5846 / publish1.3137 | 聚类不是整个灯光阶段的主因；publish含refresh0.7164与batch head0.5309 |
| shadow任务组 | 1.9425 | 父线程墙钟，含spawn、inline、等待与合并，不叠加worker墙钟 |
| Graph | 2.7463 | compile1.3173内含declarations1.0037；未覆盖1.2559不归给一个猜测函数 |
| native准备累计 | 6.9768 | 包含execute内同帧reuse，不与execute3.9347相加 |
| native准备内部 | validation1.2394 / resources3.0408 / parameters1.2611 / capture1.3916 | resource scan1.5757、Lit binding0.6190嵌套；参数map0.7616、instance stage0.3773嵌套 |
| native模板 | prepare1.2044 / current0.6010 | current嵌套于prepare/capture；每帧2417次对象核对，新增native模板0 |

colourSelection1.1485、colourCompactFinish3.5943是**调用线程累计**，包含inline shadow jobs，不能称主颜色独占区间或再加到sceneLighting。原每帧producer命令/格式/native模板新增均0，12个shadow任务、29657.125选中参数行、9897.25实例值、11种view值；6124.5是含reuse的准备入口命令长度和，不是两次完整准备。

## 合并实施与真实工作量

| 原owner | 实际修改 | 保留边界与证据 |
| --- | --- | --- |
| build_inputs / CompactView | 保持原CommandCache，构造完本view批次后一次获取锁并attach；refresh_commands同样一次锁处理本view。没有新缓存或资源owner | 首红scale4实际9锁/5view；修复后4/32/256材质批次均3锁/5view，batch数5/33/257。计数是实际mutex获取，未用代理函数调用数冒充；原排序、全tail和验证仍执行 |
| shadow/planning / prepare_tasks | receiver范围与caster角点作为两个独立前置任务，经既有run_bounded合并；生产在prepared且实例≥512、硬件允许时用2线程，小输入串行；无caster light时不做未消费receiver计算 | 在后续最多12个shadow view/4线程任务组之前全部join，不嵌套、不建跨帧队列。4/32/256输出与标量完整对照、独立caster oracle、真实2线程及末行layer错误一致 |
| native geometry / join_validation | 大plan（命令≥512）将不可变参数验证放至一个scoped worker，native CPU扫描留在调用owner；成功汇合后才开始GPU准入/修改。保留原完整两条扫描，降低关键路径而非声称删除必要工作 | 两分支全部完成再返回，参数错误优先；worker panic终结为原准备错误，OS spawn失败保留closure并串行执行。测试覆盖不同真实线程/错误顺序/全部汇合/panic和disabled串行；未强制注入OS spawn失败 |
| native_template / GeometryGpuState | 原prepare调用内HashMap已形成唯一模板闭包，直接发布有界current集合；移除对齐数组的第二次遍历、HashSet和Arc复制来重建同一集合 | 对齐数组保留命令顺序；current是无序保活/观察集合，上限65536不变。Weak memo、当前实际GPU对象/池页offset检查、满容回退与真实completion不变；不是新的GPU有效性proof |

本轮未改变外部wire、SDK、shader、GPU参数layout、资源预算/上传/队列上限。native扫描只在原owner更新CPU memo/stats，GPU上传/绑定/参数写入/退休仍在成功join后。失败释放局部结果，旧在途recording到真实GPU完成点退休；取消/generation及不可变packet继续由原链路负责。

新增前置任务结果是调用内scratch，不建立持久分配owner；receiver/caster/模板引用已有实际payload观察继续生效。HashMap桶、Weak控制块、worker栈及prepare scratch的完整backing尚未接入，明确为未接入，不冒称平台不可观测、完整内存通过或用进程总量代替owner。

## 首红、返工与终验

| 证据 | 首次结果与处理 |
| --- | --- |
| 02-lock-red | scale4实测9次锁，超过view数+1，确定性失败；不是帧耗时断言 |
| 03-check | 新fixture误用不存在的LayerSelection::Mask，编译失败；修为Legacy选择，产品无运行失败 |
| 04 / 05-owner | owner构建通过；执行236通过/1失败。唯一失败为新锁计数fixture误unwrap合法空shadow writer的None；锁断言已通过。修为有batch则验证，无batch必须draws为空，不改产品断言 |
| 06 / 07 | 合并构建修正fixture与直接消费者；修正锁例1、visual54、U2 28、lifetime5、geometry15通过 |
| 08 / 09 | Clippy发现新test的len>0写法，等价改为!is_empty；仅测试表达式，复用原产品和测试证据，复查fmt/Clippy，不重跑绘制或消费者 |

有效正确性证据去重并集**339项**：复用05中未受后续fixture和等价断言修改影响的236项，加07新执行1+102项。不是宣称最终09源重跑339项，也不累计首红/旧失败版本。同一binary多过滤避免重复Cargo构建。native真实GPU反例在>512命令的末尾重复同view/instance，仍报duplicate且mesh上传、binding创建、参数写入均未发生；无validated/current模板残留。真实像素、透明/阴影、错代/COW、完整tail、容量/部分上传及退休消费者通过。

8文件rustfmt通过；real-wgpu库/测试Clippy通过，既有lib152/lib-test172 warnings保持，未修改ledger或宣称零告警。default库check通过（11条既有warning）。没有为等价test写法再次构建完整消费者。

## 一次最终无probe原K1反馈

11运行前确认无构建/检索进程及已知brotato游戏；测量期间无并行构建、检索、编辑。来源09、DLL10与module身份核对后使用原delivery/runner；Windows Job正常等待/释放、reader汇合。无源码probe、输出无CPU371R，99声明输入保持。

CPU预算口径为cpuExcludingPresentWait，不能用含Present等待的meanCpuFrameMs混算。

| 同原K1无probe | CPU mean / 观察P95 ms | GPU mean / 观察P95 ms |
| --- | ---: | ---: |
| 前窗24源 / 28 DLL / 29反馈 | 24.5812 / 26.2381 | 8.3425 / 8.3845 |
| 本窗09源 / 10 DLL / 11反馈 | **22.7726 / 26.3643** | **7.8212 / 8.6108** |

CPU均值下降1.8086ms（7.36%）；观察P95上升0.1262ms，**未证明尾部改善**，更不能称持续达标。只有8有效样本，观察P95/P99均为最大样本，无法据此解释长尾机制。GPU短窗波动不归因于未对照因素。update mean1.1665、renderSubmit21.5795、Present等待0.2524ms；含等待总帧mean23.0251ms。当前剩余仍集中在renderSubmit；没有第二套最终源详细profile，不能将00源分段移植过来。

原CPU/GPU P95≤13.3ms、P99≤16.67ms保持，CPU短窗观察预算失败，GPU短窗观察通过；不授持续、7200配方或正式三轮。原100 resident meshes/10000 instances/128 materials/96张2048全mip BC3/10M triangles/145 lights/4cascades+8spots/1080p/far2000及运动轨迹不变，K1不是S1的10000 unique meshes资格。

启动前物理可用5.65685GiB低于6GiB，仅单点，无全程引擎/宿主/环境归因。不关闭应用、不扣工作集或降低门槛。加载39次present、必要等待0.4779s；单帧上传16MiB/staging32MiB/in-flight2保持。此短窗不替代完整内存、BM08或流式资格。

## 构建与验证成本

完整计时见[13-costs.json](../../../../.artifacts/371/s2-dynamic-20261020-080009/13-costs.json)，包含失败、返工和中间身份。

| 阶段 | 墙钟秒 | 口径 |
| --- | ---: | --- |
| 首红Cargo构建+执行 | 175.40 | 执行嵌套，不再叠加测试时间 |
| check失败 / owner no-run / consumer no-run | 61.90 / 167.91 / 320.46 | 含新fixture编译失败和后续修正；终验构建一次 |
| owner首次执行 / 修正锁例+直接消费者执行 | 31.87 / 39.62 | 236项有效证据复用，不重跑整个owner suite |
| scoped Clippy累计 / default check | 88.95 / 11.06 | Clippy含等价test告警修正复查；fmt累计0.55秒 |
| probe Release / 无probe Release | 368.19 / 387.99 | 两种来源身份，非Compiler/Cook |
| probe反馈outer / 无probe反馈outer | 36.49 / 44.71 | 最终child 16.142秒嵌套于outer，残余含启动前后，不全部冒称构建/准备 |

新fixture的枚举误用、空writer假设及等价assert告警属于可避免的测试返工；不把它们说成引擎功能缺陷，也不隐藏相应构建成本。必要的有probe/无probe身份切换仍是本窗主要准备成本之一；未盲目重跑Compiler/Cook/BM03或整workspace。首次powershell -File被本机脚本执行策略拒绝，未改策略或加入bypass；同一授权命令用普通交互式Cargo调用执行，不作临时目录清理或绕过记录。

## 审核与剩余出口

审核结论：本轮保留了原owner、成功proof与当前资源检查，实际消费者和一次反馈符合声明；**平均改善不等于完成“大块”**。独立准备改为并行没有删除对应总工作；除减少锁获取和二次集合整理外，逐帧选择、薄head发布、Graph与native plan整理仍存在。当前证据支持“已批准结构迁移仍未收敛，且必要动态工作仍有成本”，不支持重建同名缓存、全归因GPU、降低标准，或宣布成熟引擎方案本身错误。

| 原大块 | 状态与尚缺 |
| --- | --- |
| 1 常驻命令与动态参数 | partial：稳定模板/native接管继续有效；薄动态DrawGeometry/batch head和逐帧索引/引用整理未全部收敛 |
| 2 可见性与阶段准备 | partial：shadow view、receiver/caster及native两分支真实有界执行；主颜色选择/排序、shadow发布及其它主要动态准备未整体贯通并行/稳定消费 |
| 3 接管与旧链退役 | partial：消费者通过及一次无probe反馈已做；动态表达/Graph声明/plan消费的旧重复工作、原预算及S2实际配方出口仍缺 |

下一继续点沿既有§0.26：压缩Production最终颜色/阴影head发布→Graph/RHI动态命令→native当前资源与参数消费的逐帧整理，确认实际重复底层工作后合并整改；稳定模板/slot/proof/GraphScheduleCache不重建，必要selection、透明/阴影、当前资源准入及completion不删。当前最终源只知道renderSubmit均值21.5795ms，后续不能按旧分解直接给新函数排名。需更细当前归因时复用已有有界探针，禁止机械照搬旧profile或反复测同一身份挑绿。

正式三轮、完整内存观测、OS显示和Android继续按371-G延期，不成为当前CPU开发全量前置。无安装/宿主事务，无清理绕过；证据目录有意保留。F盘release DLL现为本窗10同源无probe候选，仍未安装。371留在当前，不归档；本窗文档、哈希/进程和链接核对见15-closeout.json。
