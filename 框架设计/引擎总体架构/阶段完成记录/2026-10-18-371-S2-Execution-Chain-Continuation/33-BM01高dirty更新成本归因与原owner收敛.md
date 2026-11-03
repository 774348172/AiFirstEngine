# 371 §0.33 BM01高dirty更新成本归因与原owner收敛

同一整块续建，**整块、S1和371仍partial**。本窗已修复确认的重复工作，10% dirty无探针短窗CPU均值28.2341→18.9113ms，下降约33.0%，仍超预算；100% dirty无探针短窗190.5078ms。剩余瓶颈明确在更新链，不能继续泛称渲染提交慢，也不能据此推翻整个3D渲染架构。新的SDK/wire与热空间身份边界尚未获选，不在本窗擅自实现。

开工2026-10-21 13:43:08 UTC，默认3小时；16:28:08停止新增、16:43:08硬截止。实际结束和窗口成本见证据closeout。A路G:/gameEngin、codex/255-capability-catalog原dirty保持，B空闲。Windows only；4.5 GiB（4,831,838,208 bytes）及CPU/GPU P95≤13.3ms/P99≤16.67ms保持。安装、宿主配置、SDK、Physics和游戏V0.0.5未修改。

证据根：`.artifacts/371/s1-update-20261021-134308/`（下称E）。开工19项来源SHA全部匹配；源码身份以18-source-freeze/28-probe2-restore为准，产物对照见29-artifact-comparison。历史32记录的完整配方结果保留其原二进制身份，不迁移为本窗最终DLL的新完整资格。

## 1. 已实现：现有owner的确定性工作收敛

修改范围为`rust/crates/engine_runtime_core/src/`下archetype、components、world、spatial_3d、spatial_3d/structure、project_runtime_session、session/structural、session/tests，共8文件。没有新World、持久校验证明、wire、SDK、资源owner或GPU寿命规则。

1. 已存在组件在原列/row原位替换；只有真正增删组件签名才迁移整行。previous只读取一次并复用比较/field mask。组件读取二分定位已排序signature；内建类型判断使用借用类型名，避免反复分配owned ID。
2. final overlay借用ID/类型名，存在性检查不复制旧payload，spatial node只定位一次table/row；没有显隐override时不额外重复读显隐。空结构请求立即返回空plan。
3. 只有私有、无Prefab的宿主即时事务，在同一个独占`&mut World`借用中prepare后直接应用刚验证的plan；中间没有外部callback、yield或可变World插入点。公开prepare/延迟commit、原commit_for_host及混合Prefab路径继续重新确认generation、当前容量和最终overlay。共用实际提交、记录及历史路径，没有把跨时间成功检查当永久证明。

E07首红真实计数，N=1/16/128/1024：host/public均校验2N、remove_row=N、实体位置变化。修复后host校验N/public校验2N，两者remove_row=0、location稳定。计数位于真正validation/write与row removal底层，仅cfg(test)，不是用缓存命中或时间阈值代替工作证明。

结构keep-world、anchor、history/interpolation、最终camera祖先非单位scale原子拒绝新增host/public对照。stale generation、removed component、invalid tail和容量拒绝原测试保持。未降低上传/staging/在途上限；GPU共享资源和真实完成点退休消费者仍通过。

## 2. 真实Release归因

第一组probe（E04）用于找修复前首因：604帧、600样本，prepare6.6625ms、commit7.5849ms，其中两次overlay validation合计9.5069ms；传播4.4288ms。它只有inclusive时间，**不能相加嵌套项，也不能当最终源码分段**。E05已恢复原字节。

第二组probe绑定本窗最终8源，E26/E27直接既有diagnostic child保存完整stderr，分别604/64行。逐帧断言所有exclusive之和等于frame inclusive；有嵌套/互斥、调用和真实工作计数，不依赖日志截断推断。probe会增加开销，只用于归因，不授预算资格。E28恢复12文件原字节、更新时间戳、删除临时module，并核对最终8源与E18完全相同。

以下为**互斥顶层**耗时；括号为其内部包含项，不能重复相加：

| 更新区间 | 10% dirty（600样本） | 100% dirty（60样本） |
| --- | ---: | ---: |
| native adapter（含module、host查询及JSON decode） | 3.3218ms | 32.8939ms |
| session transaction（含prepare及apply） | 6.8923ms | 77.0291ms |
| spatial propagate | 4.7812ms | 41.7696ms |
| presentation | 1.8335ms | 14.7438ms |
| begin tick/history | 0.8596ms | 5.3919ms |
| RenderExtract | 1.5547ms | 12.8460ms |
| frame hash | 0.5422ms | 0.4877ms |
| snapshot/observe/physics及frame自身余量 | 0.2064ms | 0.8183ms |
| frame合计 | 19.9917ms | 185.9802ms |

细项：prepare5.3714/57.5426ms（包含validation3.7335/36.7100ms）；apply1.3821/17.0202ms。wire3.2540/30.3072ms，包括module_call1.9429/18.4399ms及decode1.3083/11.8616ms；module_call又含query0.8484/6.9442ms，不能把它全部称为JSON成本。module自身exclusive1.1014/11.5026ms包含项目循环、编码等，尚未单独拆出encode。

每帧真实source→table查询21,001/210,001次、host query1,001/10,001次、node3,000/30,000次、evaluate2,000/20,000次、descendants2,000/20,000次。每帧1000/10000个真实变换，返回JSON约234,579/2,364,101 bytes。source lookup只有计数、没有独立计时，不能把整个区间耗时归于字符串查询。4次propagate入口不代表4次全量计算：空入口保持快速返回。2N evaluate来自最终验证与实际传播两个职责，并非没有条件就能删除的重复校验。

确认：现有内部修复确有收益，但100% dirty时模块/wire自身已约30.31ms，超过整帧13.3ms；仅继续删内部validation无法解决全部目标。空间准备、传播、表现和提取也有独立的大成本，不能承诺只换编码便达标。未见本轮规模点出现平方级计数，不能把问题误报为已找到O(N²)。

## 3. 无probe原规模短窗反馈及身份

E19旧交付、E20新交付在同一时段串行运行同一个4 warmup+600样本scenario，测量时没有其它施工。没有删除长帧。

| 同BM01 K0/10%前缀 | CPU mean | P95 | P99 | max | update mean | submit mean | GPU mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 19本时段旧DLL | 28.2341 | 33.4166 | 36.2925 | 129.1356 | 27.7506 | 0.2996 | 0.3237 |
| 20本窗最终DLL | 18.9113 | 22.5833 | 25.1997 | 120.0726 | 18.4443 | 0.2870 | 0.2549 |

均值改善约33%，**CPU仍失败**。Game.exe随构建变化，module实际DLL字节相同，27个RuntimePackage文件中26个相同，仅build-runtime-package-report变化。因此是同配方/模块/有效包内容的前后反馈，不宣称固定Player的严格单变量实验或全部差值严格因果。

最终engine DLL SHA256：`1f3e8260d4e75d0224fd12b64f441675c69ad6087aa355975cdfd0c7cfb7638e`；旧DLL `c9533aa62843e34be46df9bd596376f8f32bccc97a7a34cf98a2ec493f6e234b`。完整Player/module/scenario/harness SHA见E19/E20 timing、E29。最终交付在上窗matrix/K0-10/Build/18-final-release/release，目录名称不代表其源码属于上窗。

E24使用相同最终engine DLL、真实100% dirty项目module，4 warmup+60样本/64tick：CPU190.5078ms、P95 218.6101ms、P99/max304.8350ms；update188.4729ms、submit0.3139ms、GPU0.2146ms。独立断言真实10000个提交值，非只增加descriptor计数。不是完整7201tick或持续P95/P99资格。按当前实测完整高dirty很可能继续超过120秒，故没有盲重跑完整矩阵。

运行前系统available5,155,885,056 bytes是单点满足4.5 GiB，不授全程资格。编译中的另一个低于门槛单点不是正式运行窗口观测；没有扣工作集、关闭其它应用或改宿主配置。完整owner观测、OS关联、三轮/压力资格继续按371-G延期。

## 4. 正确性、失败与成本

有效验证：session22项（E10先21绿1个新测试wrapper断言错，E11修断言通过，E15最终carrier刷新两个新增session）；world18/archetype4/components2/native_adapter40/world_api10共74项直接复用已编译测试exe运行；real-wgpu foundation54通过、6 ignored；最终Option carrier后E16刷新其中5个真实GPU消费者（双cell分片ACK/共享退休、BM01、BM09、same-handle changed texture拒绝）；E15另5个匹配immediate的既有consumer通过。去重并集155项，**不是155项都在最终二进制重新运行**。纯Option carrier变化由新增session及5个GPU消费者刷新，其余按消费身份和等价边界复用。

8源rustfmt check、E17 core real-wgpu scoped Clippy通过，140条warning保留；不宣称零告警或全部warning都是既有。未重跑workspace、BM03、全renderer矩阵或正式三轮。SDK字节未变，原包/模块身份复用已单独核验。

真实返工全部保留：python商店别名改py -3；第一probe插错同名frame函数；新增测试enum与Reparent字段写法错误；wrapper error.code应检查外层mutation_preflight_failed并检查message中的three_d.camera_scale原错误；初版Result<ComponentValue>触发large Err新告警，改Option carrier避免新增boxing/分配。除底层工作首红外，其它上述失败均是施工/harness错误，不能计为产品故障或隐去构建时间。

E30账本包含失败：直接check/单测构建310.899s、Clippy22.051s、runner实际测试46.799s、直接exe测试0.467s；Compiler四次外层合计1403.077s（23.38min），其中build/export口径1386.482s嵌套在内，**不得再相加或称纯Rust时间**。四次外层分别465.472/440.835/65.708/431.062s；最后一次probe因为新源码仍失败而补齐互斥归因。测量/Player及全窗口实际成本见closeout。恢复probe后源码与18完全一致，复用已存在无probe最终DLL，没有为恢复再导出一遍。

## 5. 已闭合与剩余

已闭合本轮归因、真实底层工作红绿、8源现有owner收敛、受影响正确性及无probe短反馈。§0.33大块未闭合：BM01高dirty与4个未完整通过档位、BM11完整故障schedule、BM12完整压力消费者仍缺。BM09实际消费、低dirty完整轨迹与历史GPU寿命只按原身份保留。完整内存/正式资格与Android依旧延期，未安装。

当前有两种层次的问题：已修的是**实现中的无用搬移与可避免重校验**；剩余证据指向**通用字符串/JSON逐项通道与热空间消费分层不足的设计限制**。基础功能并非完全没有实现；全配方消费者也确实未全部施工。三者不能混写成“只是性能”或“整个3D设计错误”。

## 6. 需要讨论的新范围（草案，未批准，不是正式合同）

推荐A：**“项目批量空间通道与热空间消费端到端整改”**。应覆盖项目写入→生成胶水→module/host ABI→原session→World空间传播→presentation→RenderExtract，不能仅换序列化或只加缓存。

拟调整的边界：

- SDK项目侧继续typed批量意图；讨论新增版本化、有界typed POD批量通道，使高频local transform不再逐项字符串JSON往返。冷source ID解析与热generation身份分离，复用已有RuntimeEntityId/slot owner，禁止裸index或跨World句柄；具体World/session epoch及ABI形状待审，不在这里冻结。
- 既有JSON通道保留低频/兼容语义，旧reader对新能力明确拒绝；不能静默丢字段或把新module喂给不支持的host。生成胶水、digest、指针/长度/容量/对齐、callback恰一次、输出生命周期和invalid tail都要有反例。8 MiB/16384原限额保持，不能让typed通道绕容量。
- 原World/session仍唯一事务权威。热批只解析当前身份一次，按已存在typed列消费；结构/Prefab、parent/camera变化不能误用仅值写入的快路径。延迟commit继续重校验；同批失败零写入、同ID重生与跨World旧句柄拒绝。
- 讨论在同一个安全提交边界复用final spatial evaluation，减少validate→propagate重复构造与身份查找；保留camera/ancestor scale、anchor/shear、final overlay与原子性。禁止把公开prepare结果跨变更直接消费。空间timeline/presentation/RenderExtract按真实dirty与稳定运行身份消费，不新增第二份权威World或让表现反写模拟。

施工前必须先审改合同（尤其371-S1-B第1节“复用原DTO、不引入bulk wire”的现行边界），再更新总施工必要性/Gate；本窗不直接违反原“SDK不改”范围。下一块最低出口：1/100/1000/10000真实工作计数和负例、旧/新reader兼容、实际普通项目端到端、新旧在途/历史安全，以及原规模10%/100%无probe反馈；未接近预算时不启动完整7201tick或正式三轮。不得承诺仅A便必达标。

选项B：只继续内部World/空间实现优化，保持SDK/JSON边界，风险较小；但100%模块/wire约30.31ms已超过整帧预算，**不能以B承诺全部高dirty达标**，不推荐作为完整闭合路线。

选项C：调整BM01尺度、dirty、预算或改GPU模拟以规避现有CPU路径；改变既定目标，本轮不采用。

新范围需要讨论的依据是施工skill明确要求：`Pause for user discussion when the repair requires changing the framework design or an architectural boundary.` 已批准的渲染模板/批量消费/证明复用、当前原owner修复不重复申请；此次只新增module/SDK通道与空间热消费边界。


## 7. 实际收尾

实际结束2026-10-21 14:49:54 UTC；窗口66.77分钟，未触发时间上限。构建/check/Clippy与Compiler外层合计28.93分钟（Compiler内层不重复相加），测试47.266秒；六次Player/诊断进程外层合计117.260秒，包含启动/检查，非全部采样时间。阅读、编辑、分析与文档计入整个窗口。12源probe恢复、8源终态SHA再核对、临时module不存在；无本路在途进程。证据和备份保留，没有清理被策略拒绝的旧目标。49/54、实际合同、总施工、阶段/施工索引、总览及根README已同步；总施工不归档。下一新边界待用户选择；不是以时间上限代替性能失败原因。
