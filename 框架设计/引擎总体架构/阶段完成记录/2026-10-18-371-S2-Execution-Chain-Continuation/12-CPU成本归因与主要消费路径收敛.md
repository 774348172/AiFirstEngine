# 371 §0.25 CPU成本归因与主要消费路径收敛

日期：2026-10-19。371同§0.25 CPU成本归因与主要消费路径收敛：两个源码检查点完整有界归因，原参数/selection及快照空间事实共六文件收敛，最终237项消费者、六文件fmt/scoped Clippy通过；95声明输入/六变化。最终无probe原规模CPU mean/P95 48.5095/57.7907ms，GPU 7.4968/7.9555ms，partial / source_consumer_verified / scale_budget_failed，formalQualified=false。见[12记录](12-CPU成本归因与主要消费路径收敛.md)。预算/整块/整链/S2/S1/371未完成；原规模、运动、画质、13.3/16.67ms和6GiB保持，Windows only、Android/371-G延期，B/Physics/SDK/安装/宿主保持。

## 1. 本窗实际出口和身份

用户授权继续同一大块：当前成本归因、原owner整体收敛、正确性终验及原规模反馈。02:56:51 UTC开窗，05:41:51停止新增、05:56:51硬截止。沿原dirty分支codex/255-capability-catalog/F盘既有target；B未激活。没有新资源owner、公开schema、GPU布局/shader、安装或宿主配置事务。

基线00的95声明输入与10最终逐项一致。第一次产品为13三变化；最终28为95声明输入/六变化，完整路径/hash及六文件副本分别保存。最终29无probe DLL SHA256：42E877A88684D47C2301EDB745FB5CD6E481642CD11A1CAF36C6D0BB34DFAA45；30实际consumer receipt的diagnosticEngineSha256一致。原K1包、scenario、Player digest与上窗逐项相同；项目模块路径/hash、AOT bind receipt另行核对。源码、测试exe、原生DLL、项目模块/包身份分开保存，不授clean commit或已安装资格。

已闭合的是本窗归因、两组关联产品实现、正确性和短反馈。**CPU预算及整个请求大块仍未闭合，不归档371；不是因为未跑完整内存观测或S3功能尚未开发才出现当前CPU耗时。**

## 2. 当前源码归因、互斥关系与假设复审

第一次00+01 probe→02 DLL/03运行，第二次13+19 probe→20 DLL/21运行；两次均完整12游戏帧，第5–12帧八样本。下表只授各自源码。28最终新增静态几何消费后没有阶段probe；不能将13的5.1192ms receiver或旧分段当成28最终耗时。28当前整帧粗分见第5节。

| mean ms | 整改前00源 | 第一组修改后13源 |
| --- | ---: | ---: |
| Production | 26.8352 | 22.7743 |
| 候选选择（Production子项） | 5.6633 | 4.5464 |
| 排序（Production子项） | 2.5845 | 2.2705 |
| 组批（Production子项） | 1.3591 | 1.2000 |
| 完整selection tail（Production子项） | 3.8778 | 2.5057 |
| receiver（Production子项） | 4.8291 | 5.1192 |
| Graph | 6.6628 | 4.8059 |
| native准备 | 21.1059 | 12.2003 |
| 容量/跨batch唯一性（准备子项） | 3.5471 | 2.8623 |
| 资源准备（准备子项） | 6.5364 | 4.9499 |
| 参数（准备子项） | 9.0998 | 3.1541 |
| instance Records（参数子项） | 2.0314 | 1.2804 |
| 完整plan capture（准备子项） | 1.8998 | 1.2007 |
| native执行/提交 | 7.6633 | 5.7530 |

Production、Graph、native准备、native执行四项互斥；准备发生于prepare_surface_resources，不嵌套于执行。其它行都是父区间子项，不叠加父子时间。第一次参数实例映射5.9745ms；第二次已合并instance/view为单遍，合计1.7037ms，输入展开近0、Records1.2804ms、末端索引0.0205ms；不能把这几个内部时间再加到参数3.1541ms之上。

第二次补齐Production余量：lighting build1.6569、caster bounds1.1370、directional pose0.5290、未覆盖余量3.8097ms；native准备未覆盖0.0333、native执行除intent后4.4173、renderSubmit四主区间外2.5233ms。并未发现“所有validator都在重复完整校验”；共享表、完整batch成功proof、常驻工作集、instancing、bundle均存在并继续消费。native全plan跨batch(view,id)唯一性是独立必要动态事实，保持逐plan执行。

两次实际平均工作量均为110000候选、29657.125选择、3043.25批、18785.125唯一实例参数、11视图参数、18797.5旧slots。原参数阶段9.0998→3.1541ms表明局部成本减少，但跨次诊断不作受控精确因果分摊。第一次无probe13源CPU52.6248ms，较53.3039ms仅约1.3%，**未证明可靠整帧收益**；因此没有重复同源运行挑绿，而是取得第二次新鲜分解并返回receiver/caster owner修复。

首因是已确认的冗余动态消费，不是新枚举或接口缺失：完整选择项临时展开、多遍参数映射与完整值复制、全active槽仍全扫描、prepared tail只为有限性重做完整矩阵乘法；以及同一静态world角点在每次动态receiver/caster再次变换。相机裁剪、透明顺序、阴影可见性、新selection完整tail及当前资源准入仍然必要，未删除。

第一次probe预设12000bytes却漏掉loading frame0输出，实际13413bytes，原reader完整未截断。04保留首拒绝并只修解析器：剔除0，验证游戏1–12完整；不为解析错误重建补采。最初误将prepare视为execute子项也已按真实调用链纠正。第二次4023bytes未超12000预算，1–12完整。05和23分别逐字节恢复七文件；各自95声明输入零漂移。所有生产临时chain_probe已移除，cfg(test)底层工作计数保留。

证据：[04第一成本图](../../../../.artifacts/371/s2-consumption-20261019/04-profile.json)、[22第二成本图](../../../../.artifacts/371/s2-consumption-20261019/22-profile.json)、[23恢复](../../../../.artifacts/371/s2-consumption-20261019/23-restored.json)。

## 3. 同一执行链的产品收敛

| 原owner | 最低真实首红/工作 | 实际实现及正确性边界 |
| --- | --- | --- |
| geometry/parameters | 先展开全部selected tuple，再分别映射instance/view并收集最终索引 | plan直接单次借用迭代，一遍处理instance/view，单份紧凑pair Vec原位解索引；保留实际选择、槽顺序与重试，不跨帧缓存可见结果 |
| InstanceValues/Records | 4/32/256唯一项复制128/1024/8192 float words；稳定槽retain扫描4/32/256 | 纯RecordKey(id/全部bits/canonical桶hash)用内部Arc共享；实际选择Vec为32/256/2048bytes。不同active旧槽全覆盖才省retain，全active扫描0；淘汰、满容量替换、重复key、pending GPU content仍原严格路径 |
| commands/PreparedPhase | 4/32/256选中项仅为有限性执行4/32/256次VP×world | 全表成功后保存world绝对元素界，每次新projection作保守有限性证明，普通prepared tail完整矩阵乘法0；不确定/极值严格回原乘法；索引、重复、兼容、深度及完整tail不省略 |
| PreparedInputs/SpatialBounds→receiver/caster | 4/32/256项实际world角点64/512/4096次，相机分量128/1024/8192 | 静态world corners和完整绝对项有限界进入原快照空间事实；准备后world重复0，相机分量32/256/2048，即8/项。receiver逐项source/index/view关联，额外要求selected Arc与prepared完整行身份相同；COW改动走原严格函数。caster仍按当前visibility/layer/material筛选，原方向光拟合不省略 |

参数选择真实行访问在各唯一项重复两次时由16/128/1024降至8/64/512。首红计数落在旧实际复制/遍历/retain/矩阵求值点；删除旧复制后，最终改查真实Vec字节数，避免用不再执行的旧计数器虚证。没有再次引入iter().nth的平方级随机访问。

有限性数学边界：全表静态成功保证world有限，当前projection全有限且maxAbsProjection×maxAbsWorld < f32::MAX/16才免每行完整乘法；四项dot的乘积和前缀均保留余量。4096组严格矩阵oracle和极值回退通过。快照角点范围保持原f32求和顺序、全部相机分量的绝对项上界以及原向外误差guard；只求必要z分量，不能用未验证z隐藏其它分量溢出。大平移/镜像/缩放/shear/取消误差与原角点算法逐项核对；非有限或溢出返回unknown，不表示空receiver。

Arc key只持纯CPU字节，不持实例、lease或GPU资源；原Weak/COW/serial wrap/零容量/有界清除保持。新角点数据嵌入原SpatialBounds Vec，随原快照/origin/来源/transform身份失效，原cpu_vec按真实size/capacity计入snapshot backing；没有引入另一资源owner或强引用全局缓存。hash桶、free tree、Weak/control backing仍explicit partial，完整census继续371-G，不称平台全部不可观测或已完成。

## 4. 反例、消费者及返工

07第一组多规模三项预期首红保存；10窄例15通过；11为13源235项通过（owner133/visual54/U2 28/lifetime5/geometry15）。新鲜profile确认receiver后，24真实首红完整保存，0.04秒执行；25五项窄例通过，0.22秒内部执行。第二组多规模真正统计world矩阵求值和camera分量，不只数外层调用。

最终27绑定28源：owner135、visual54、U2 28、真实GPU lifetime5、geometry15，共**237项全部通过**。相机运动/新COW row拒绝静态角点并严格回退、非有限camera、corner guard、flat/compact、透明/alpha/阴影像素、generation、部分上传、队列及上传上限、旧在途帧与真实completion消费者保持。10/11/25/27有覆盖重叠，不累加成新增唯一测试数。第二组合并终验由新增产品路径失效面触发，不是为第二次诊断重跑全workspace。

六文件fmt/scoped Clippy退出0，既有warning保留。未运行Compiler/Cook、BM03、全workspace、Android、正式三轮或安装资格；未变包/模块/Player复用。

真实返工与分类：第一次rustfmt误用edition2024且未skip_children，三范围外子文件漂移被13冻结拒绝；仅用与基线hash相同的上窗副本逐字节恢复batch_tests/pages/targets，再按2021/skip_children检查，13b保留。第二次probe误对新迭代器调用旧Vec len，编译失败，移除无效计数改用实际消费点后构建通过；失败20-instrumentation-failed保留。第二组red fixture起初drop顺序不满足借用寿命，修声明顺序后才获得真正运行红例，24-fixture-lifetime-failed保留。它们分别是施工/探针/fixture错误，不归咎宿主或设计。没有把编译失败计成性能首红。

## 5. 同原规模无probe反馈与限制

最终29 DLL+30一次原包K1；测量期间无并行构建、检索、编辑或其它施工。12帧、4预热、8采样，完整窗口，所有运行正常释放Windows Job并join readers。

| ms | 上窗最终基线 | 第一组13源检查点 | 本窗最终28源 |
| --- | ---: | ---: | ---: |
| CPU mean | 53.3039 | 52.6247 | 48.5095 |
| CPU P95 | 61.6049 | 59.7384 | 57.7907 |
| CPU P99 | 61.6049 | 59.7384 | 57.7907 |
| renderSubmit mean | 52.0240 | 51.1776 | 47.2399 |
| update mean | 1.2510 | 1.4157 | 1.2425 |
| GPU mean | 9.9647 | 8.6226 | 7.4968 |
| GPU P95 | 10.7622 | 9.2232 | 7.9555 |
| GPU P99 | 10.7622 | 9.2232 | 7.9555 |

最终CPU均值变化-4.7944ms（-8.9945%）；仅为单次短窗合并修改观察，不精确拆给单个owner。P95距13.3ms还有44.4907ms差距，P99≤16.67ms同样未闭合。八样本的P95/P99不是持续尾部资格；不从CPU倒数推实际显示FPS，GPU变化不归因未改动的GPU算法。正式资格formalQualified=false。

原10000实例/100驻留mesh/128材质/96张2048² BC3 mip纹理/10M三角形/145灯/4cascade+8spot/1080p/far2000、原运动/画质和预算保持；此为原K1，不冒充S1的10000 unique-mesh配方。RHI命令数三侧均3062，不等于实际GPU draw数。不降低6GiB系统可用门槛，不扣减物理可用内存伪造达标；短窗不授全程内存峰值或完整6GiB资格，完整内存/显示/正式三轮继续371-G延期。

最终所有身份与数值见[31对比](../../../../.artifacts/371/s2-consumption-20261019/31-comparison.json)。最后两组实现的分段收益没有用原13 probe填空；当前最终粗分使用上表的真实renderSubmit/update。

## 6. 构建、验证和Player成本

每项采用互斥外层墙钟，嵌套阶段不重复相加。

| 构建阶段 | 分钟 | exit |
| --- | ---: | ---: |
| diagnostic DLL | 6.67 | 0 |
| owner red | 5.54 | 0 |
| owner green | 4.93 | 0 |
| combined consumers | 5.51 | 0 |
| unprobed DLL | 6.96 | 0 |
| second probe instrumentation error | 0.42 | 101 |
| current product diagnostic DLL | 6.79 | 0 |
| shadow red fixture lifetime error | 0.57 | 101 |
| shadow deterministic red | 3.08 | 0 |
| shadow owner green | 2.32 | 0 |
| final combined consumers | 5.24 | 0 |
| final unprobed DLL | 6.43 | 0 |

构建共12次，合计54.47分钟；Clippy 3次共1.74分钟；红/绿/各消费者测试进程外层共2.12分钟；四次反馈外层共3.29分钟。测试内部summary时间小于进程墙钟，未与外层叠加；format单列receipt。

| 反馈 | 外层秒 | 准备/runner余量秒 | Player子进程秒（内含） | 实际8帧采样秒（子项） | 加载等待秒（子项） |
| --- | ---: | ---: | ---: | ---: | ---: |
| diagnostic | 54.45 | 30.97 | 23.48 | 0.5380 | 0.9261 |
| final | 56.36 | 38.77 | 17.59 | 0.4239 | 0.7337 |
| current diagnostic | 43.28 | 26.66 | 16.63 | 0.3976 | 0.4908 |
| merged final | 43.12 | 26.88 | 16.25 | 0.3909 | 0.5060 |

Player进程含启动、资产加载、预热、runtime与收尾；outer减child是父准备/编排余量，不能称纯Compiler或纯Player准备。geometryLoading.waitWallNs只授加载等待，实际采样wallNs单列。尚未独立观测的子进程启动/解码/预热/收尾余量保持未细分，不伪造纯准备耗时；它不与已列的child再次相加。第二次诊断/第二组验证来自首组无可靠整帧收益后的新鲜归因和新产品变更；不是同源重跑挑绿。此次完成文档生成时距开窗约85.95分钟；互斥已分类成本约61.62分钟，余量含实现、分析、脚本/日志修复、等待与文档，不冒充性能测试本身。

## 7. 已闭合、剩余缺口与设计判断

已闭合：两个源码检查点完整成本分解、对应真实工作量首红、六文件实现和237项直接消费者、最终无probe短反馈。未闭合：CPU预算/整个大块/整条执行链、持续K1和其它lit实际recipe、371-G延期资格以及S3–S5。不是“测试开不开都无所谓”能解释的当前执行成本，也不是这些验证通过就意味着整块完成。

本窗末次源码复核确认：RuntimeRenderer::retained_geometry_plan依赖完整GeometryFrame Arc及base graph/target/quality；相机运动使完整frame复用失效，但GraphScheduleCache已经按资源/contracts/pass结构复用schedule，不能再把“缺少图拓扑缓存”当根因。native的PlanSources/SourceComparisons、lit绑定解释memo和完整参数key已存在；ValidatedGeometryPlan的完整命令匹配与逐plan准入仍是不同事实。本窗没有为这些已有能力重复施工，也没有据一处validator调用推定重复完整校验。

当前证据支持既有owner内还有实现粒度和消费效率要收敛，**未证明必须推翻方案B、换后端或新增资源owner**。13诊断已经显示成本分布于Production/Graph/native资源和编码；28最终分段未采，后续不能把这些旧数值直接标成28热点。优先用当前最终粗分及最小有界定位，确认动态plan miss后仍重做的稳定资源/图准备，或其它实际残余；既有缓存/成功proof先核实生效路径。若需要跨plan传播新的拓扑/资源准入事实，再提出具体依赖、失效、反例及旧帧completion边界讨论，现有批准范围不重复申请。

下次仍继续同一§0.25大块，以28源/29 DLL/30反馈为基线，复用未失效消费者和原包。不得把一个字段、cache或下一热点再命名为一个已完成大块；没有整帧收益先复审假设，不继续堆优化或重复挑绿。必要证据保留，共享target/证据根不清理；本窗没有安装、宿主变更或策略拒绝事务。
