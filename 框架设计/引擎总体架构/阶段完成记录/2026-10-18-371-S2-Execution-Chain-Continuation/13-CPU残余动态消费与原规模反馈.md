# 371 §0.25 CPU残余动态消费与原规模反馈

日期：2026-10-19。371同§0.25 CPU残余动态消费：两检查点有界归因，深度/纹理/recipe及成功事实FIFO在五文件收敛；246项覆盖（162项最终执行、84项按未变依赖复用），五文件fmt/scoped Clippy及默认库check通过。95声明输入/五变化；最终无probe原规模CPU mean/P95 45.3420/52.8266ms，GPU 7.5100/8.2463ms。partial / source_consumer_verified / scale_budget_failed，formalQualified=false。见[13记录](13-CPU残余动态消费与原规模反馈.md)。CPU预算及整块/整链/S2/S1/371仍未闭合；原规模/运动/画质、13.3/16.67ms和6GiB保持，Windows only、Android/371-G延期，B/Physics/SDK/安装/宿主保持。

## 1. 同一整块与实际身份

用户授权“开始下一步施工”，本窗继续同一CPU执行链大块，不按单个字段、缓存或本窗重新宣布整块完成。05:49:26 UTC开始，08:34:26停止新增、08:49:26硬截止；本次同步2026-10-19T07:46:54.933Z。沿原dirty分支codex/255-capability-catalog/F盘既有target，B未激活。95输入先与前窗28逐项核对，前窗29 DLL、原项目模块SHA匹配。最终37为95输入/五变化，源副本在37-source/；无新公开schema、命令版本、GPU布局/shader或资源owner。

五文件：render_geometry.rs（深度z行及内存观测转接）、wgpu_backend.rs（调用内当前驻留/消费已有纯槽事实）、geometry/encoding.rs（依赖先去重再克隆）、render_geometry/proof.rs（原65536条成功事实FIFO）、real/memory.rs（原renderer实际backing采集）。最终40 DLL SHA256 BB95E0190EF8FCFE7F7E4FD861137AD769917BB218CC5B925B4A263D7DC4A8C8，未安装；41实际consumer receipt匹配。同原K1包/scenario/Player digest，模块文件SHA及AOT bind receipt逐项一致。末帧RHI命令3062不等于真实GPU draw数。

用户确认最终测量前自行关闭brotato，19及41前只读复核已关闭，本路未终止其它应用。四次有界反馈均保留Windows Job/reader完成状态；最终释放与join通过。施工没有改安装、宿主配置或权限，没有清理操作。

## 2. 两个源码检查点的成本归因与假设复审

第一次00源（前窗28最终）→01/02 probe→03诊断；第二次15源→23/24 probe→25诊断。均12游戏帧、4预热/8样本，stderr6093/7001bytes无截断；05九文件、27十文件逐字节恢复，各95输入零漂移。下表每列只授自己的源码，不把它赋给最终37。

| mean ms | 基线00+01 probe | 第一组15+23 probe |
| --- | ---: | ---: |
| Production | 22.8212 | 20.8009 |
| Graph | 5.8732 | 5.1062 |
| native准备 | 12.8795 | 11.5467 |
| native执行/提交 | 6.2418 | 4.8477 |
| 选择（Production子项） | 5.1449 | 4.4510 |
| 排序（Production子项） | 2.2543 | 2.1225 |
| 组批（Production子项） | 1.3441 | 1.2060 |
| 完整tail（Production子项） | 2.6035 | 2.8920 |
| receiver（Production子项） | 3.5558 | 3.3415 |
| 纹理驻留（准备子项） | 1.6611 | 1.5391 |
| recipe捕获（执行子项） | 2.6942 | 1.6101 |

前四项互斥，native准备在执行前完成；其余为父区间子项，不叠加。第二次Production未覆盖3.5252ms、Graph余量0.9824、native准备0.0255、执行余量1.4704、renderSubmit四段外2.8986ms，完整嵌套和余量在26-profile。实际均值110000候选、29657.125选择、3043.25批、18785.125唯一实例参数、11视图参数、18797.5旧slots。共享表/工作集/PlanSources/完整batch proof/instancing/bundle已经消费，没有重复建设；不凭validator调用次数推断重复完整校验。

15/18/19第一组无probe CPU mean47.4114/P95 65.8696ms，相对48.5095ms均值仅少2.3%、尾值更高，**未证明可靠整帧改善**。因此没有重复同源挑绿，而在同一整块复审假设，新增实际proof清空/完整callback/消费计数。第二次profile确认frame6 selection cache清空后完整校验3043个batch，6.54ms；frame12再次清空，重做2646个。其它多数采样帧完整selection重做为0；8样本平均711.125个完整重做、unprepared selection0.9861ms，prepared selection1.1462ms和full draw1.1907ms是其它嵌套段。该机制解释部分尖峰，**不能解释全部约47ms均值**。

第二次纹理驻留仍1.5391ms。源码追踪发现五槽入口和通用baseColor入口都直接重读binding，而DrawResources::textures已有不可变/COW失效的纯槽缓存。36反例以既有RenderBindingSet底层完整验证计数确认：预热后32次驻留调用仍完整验证128次。此处才确认重复静态解释，不把registry查询或成功事实入口访问混称完整验证。

## 3. 原owner合并实现与真实首红

| 路径 | 首红真实工作 | 最终结果和边界 |
| --- | --- | --- |
| 选择深度 | 4/32/256选择实际求80/640/5120个标量分量 | 20/160/1280，5/项；只求原(camera×world)z行及z点，f32求和顺序保持；4096例原表达式含抵消/shear/极值/非有限/signed-zero对照。未重关联乘法或删除裁剪、透明顺序、完整tail |
| 纹理驻留与纯槽消费 | 初版每4/32/128draw解释4/32/128次、查当前registry5/33/129次；第一次修复后仍在32次warm调用完整验证128次 | 调用内每binding消费一次、每不同当前handle查一次；两个几何入口接已有DrawResources纯槽事实。最终warm完整binding验证0；COW同ID/new generation、注册/移除、StandardLit-first与slot首错保持，不缓存跨调用驻留 |
| native recipe | 4/32/256不可合并批次共纹理实际克隆4/32/256次 | 实际各1次；当前对象借用列表排序去重，再生成原canonical有序唯一强引用closure。Batch、完整native对象判等、bundle、实际completion退休保持，不保留跨帧裸地址 |
| 原成功事实容量 | 接近容量后两个当前batch下游重做4/32/256次实际兼容检查 | 0/0/0；每类原65536条上限、Weak身份及严格miss回退保持，满时FIFO单条淘汰，重复插入不淘汰其它事实。不是新证明、可见性缓存、全plan跨batch唯一性或GPU准入证明 |

FIFO callback仍无cache借用，可重入；失败不存、COW/过期/真实淘汰后复查。新顺序队列只持metadata，Weak不保活payload或GPU lease；actual capacity按原renderer/decodedCache观测，覆盖当前渲染线程。原hash bucket、Weak控制块和其它线程缺口明记未接入，不称平台不可观测或完整内存资格。未改变generation/不可变packet、完整tail、跨batch唯一性、当前资源准入、部分上传重试、队列/上传上限和旧帧真实GPU完成退休。

## 4. 失败、返工、证据复用与验证范围

首次probe模板引用已改名receiver函数，部分写入后失败，启动的不完整构建立刻取消；没有测试判决或完整计时。随后改成全量变换成功才写文件；普通Node fs UNKNOWN open错误核对并恢复原hash后重试。01失败材料保留，未记作产品运行故障。

07初次深度/驻留首红成立，recipe fixture因graph640×480却请求64×64先被拒绝；只修fixture为原640×480后07b取得三个实际计数首红，独立数学oracle通过。09初版244项消费者通过；10 Clippy新增两项mutable-key及map-clone警告。已提前启动的12 Release在66.28秒主动取消，仅终止经核实本路cargo树，exit -1不算产品失败。recipe改为借用Vec排序去重，15/17恢复原库152/visual2警告，16终验244项。28取得FIFO真实首红，30/31/32得到246项；36取得warm实际128次首红后才接已有槽事实，37为最终源。

最终覆盖246项：38最终执行162项（lit1、native58、visual54、U2 28、lifetime5、geometry15、真实shadow1）；84项按30→37仅wgpu_backend.rs改变的依赖闭包复用32，均为未调用该入口的纯几何/合同消费者，含四项粒子affine/camera/output/origin数学与GeometryFrame测试。FIFO四项、数学深度等复用源和fixture逐项未变；真实阴影GPU消费者被识别后用同一exe补执行，未额外构建。42保存完整用例集合、94项未变输入和复用理由。汇总脚本初次误漏四项粒子命名空间，拒绝复用清单；核对实际函数依赖后修正解析器，没有重跑有效测试或Player。数目不是多次运行相加。

39五文件fmt/scoped Clippy通过，原库152/visual2警告保持；39-default默认库check通过，完整warning保留。没有跑完整Compiler/Cook、BM03、workspace、Local CI、Android或正式三轮。源码、harness、native DLL、模块/包分别记身份；dirty开发反馈不授clean commit、安装或其它配方资格。

## 5. 合并后无probe原规模反馈

| ms | 前窗28/29/30 | 本窗第一组15/18/19 | 最终37/40/41 |
| --- | ---: | ---: | ---: |
| CPU mean | 48.5095 | 47.4114 | 45.3420 |
| CPU P95 | 57.7907 | 65.8696 | 52.8266 |
| CPU P99 | 57.7907 | 65.8696 | 52.8266 |
| renderSubmit mean | 47.2399 | 46.0974 | 44.0750 |
| update mean | 1.2425 | 1.2831 | 1.2380 |
| GPU mean | 7.4968 | 9.3910 | 7.5100 |
| GPU P95 | 7.9555 | 9.4495 | 8.2463 |
| GPU P99 | 7.9555 | 9.4495 | 8.2463 |

CPU mean相对前窗变化-3.1675ms（6.5296%下降），GPU变化0.0132ms。仅是同冻结包/轨迹一次短反馈差异，不能把收益分摊到单项，也不能证明持续改善；GPU算法没有修改。最终12帧/4预热/8样本完整，原13.3/16.67ms及6GiB保持，formalQualified=false。8样本P95/P99不授持续尾延迟或OS显示FPS；最终37只有无probe整帧粗分，26不赋37。

原K1仍10000实例、100常驻mesh、128材质、96张2048² BC3 mip纹理、10M三角形、145灯、四cascade加八spot、1080p/far2000和原运动；不是S1的10000 unique-mesh配方。没有放慢轨迹、减载、扩大预取或降低画质。没有以局部计数或测试通过宣称预算通过，完整资源内存资格仍按371-G延期。

## 6. 构建、检查、测试和Player准备分开计时

| 构建阶段 | 秒 | exit |
| --- | ---: | ---: |
| first probe | 429.49 | 0 |
| initial red with extent fixture error | 199.23 | 0 |
| fixture-corrected red | 97.97 | 0 |
| first consumers | 416.37 | 0 |
| cancelled intermediate release | 66.28 | -1 |
| first final consumers | 353.88 | 0 |
| intermediate unprobed DLL | 382.03 | 0 |
| current source proof diagnosis | 359.76 | 0 |
| proof capacity deterministic red | 141.19 | 0 |
| FIFO consumers | 338.73 | 0 |
| warm binding deterministic red | 18.76 | 0 |
| final affected consumers | 415.86 | 0 |
| merged final unprobed DLL | 386.85 | 0 |

构建合计3606.41秒；Clippy 100.23秒；默认库check 36.26秒；测试进程执行（含首红/fixture修正和各输入版本消费者）245.53秒。额外构建由实际产品变更、首红和一次必要假设复审解释，不为同源性能挑绿。首次probe启动即取消的构建无完整计时，留在窗口未分类余量，不伪造数字。局部patch anchor两次拒绝均无源码变化、未触发构建，归编辑余量。

| 反馈 | outer秒 | 准备/runner余量秒 | child秒 | 采样秒 | 加载等待秒 |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline diagnostic | 56.71 | 38.74 | 17.97 | 0.4195 | 0.6917 |
| intermediate unprobed | 42.74 | 26.27 | 16.46 | 0.3821 | 0.7409 |
| current source diagnostic | 36.56 | 20.66 | 15.90 | 0.3747 | 0.6880 |
| merged final unprobed | 42.86 | 26.78 | 16.09 | 0.3656 | 0.4870 |

反馈outer合计178.88秒；child/采样/加载是其嵌套区间，不能相加。测试exe内部runtime也嵌套在进程wall。至本次同步窗口wall约7048.93秒，分类阶段约4168.46秒；余量含读源码/实现、未完整计时取消、等待/工具/文档，不等同纯编译或引擎运行。42-comparison保存明细。

## 7. 实际出口、剩余依赖与下一继续点

本窗闭合当前成本归因、确认冗余的原owner修复、正确性消费者和合并短反馈；**CPU预算及整个执行链大块仍未闭合。** 已确认部分问题属于消费实现/缓存替换策略，不能据此宣称总体设计已无问题，也不能把当前耗时归因到“完整内存测试或S3功能未做”。剩余仍有必要动态工作与尚未收敛的消费成本。

以37/40/41作为继续基线，复用原包、模块、Player及有效证据。下一重点按26的定位线索审查Production动态选择/排序/组批/receiver及动态plan后的当前准入/参数/Graph余量，先取得必要的当前最小信号，再合并主要路径；26的具体耗时不直接赋37。不要重新建设已生效的常驻表、批量命令、proof或bundle，不把新selection必要检查/全plan唯一性删除。涉及新的命令、跨plan证明或owner边界才形成具体合同调整与选项讨论，已批准范围不重问。

保留完整内存/OS显示/正式三轮/其余lit配方消费者与Android各自状态，不重新作为CPU整改全量前置。Windows only，371不归档，B/Physics/SDK/安装/宿主保持。本窗收尾为partial，不是整块、S2、S1或371完成。

证据：[42比较与复用](../../../../.artifacts/371/s2-residual-20261019/42-comparison.json)、[04基线归因](../../../../.artifacts/371/s2-residual-20261019/04-profile.json)、[26当前检查点归因](../../../../.artifacts/371/s2-residual-20261019/26-profile.json)、[37源码](../../../../.artifacts/371/s2-residual-20261019/37-source.json)、[38最终消费者](../../../../.artifacts/371/s2-residual-20261019/38-test-results.json)、[40 DLL身份](../../../../.artifacts/371/s2-residual-20261019/40-native-identity.json)。
