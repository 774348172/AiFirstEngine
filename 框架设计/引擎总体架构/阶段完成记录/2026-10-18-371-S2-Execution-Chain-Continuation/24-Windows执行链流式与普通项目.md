# 371 §0.30 Windows执行链、流式正确性与普通项目

2026-10-21，北京时间。状态：partial / source_consumer_verified / short_cpu_p95_failed / memory_precondition_failed / formal_deferred。当前产品来源为102-source，消费冻结104无probe DLL；不是最新缓存中的探针DLL，不是S1、S2或371完成。

## 1. 授权、来源与出口

用户授权同一窗口最多8小时，覆盖skill默认3小时，不永久修改skill。实际开工2026-10-20 16:28:57 UTC（21日00:28:57北京）；停止新增上限23:58:57 UTC，硬截止次日00:28:57 UTC。实际22:56:22 UTC转入只读审计和文档收尾；最终收尾时刻及整个窗口耗时以118-closeout.json为准，不重新计时。A沿原dirty分支codex/255-capability-catalog；B、SDK、Physics、安装、宿主、冻结发布和游戏V0.0.5保持。

证据根：[s2-long-20261021-002857](../../../../.artifacts/371/s2-long-20261021-002857)。[115最终审计](../../../../.artifacts/371/s2-long-20261021-002857/115-final-audit.json)复核386项声明输入及457项原包/module/CLI消费文件SHA，全部无漂移；[103变更清单](../../../../.artifacts/371/s2-long-20261021-002857/103-window-source-changes.json)相对开工382项输入为24文件改变、4文件新增。它们是dirty开发来源，不是clean commit资格或全仓封印。

生产链仍为engine_runtime兼容装配→engine_render_native→engine_runtime_core，装配也直接依赖core。固定SSD、Rust1.96.0、locked/-j1、原Release配置保持；未继续CPU/UI拆分。已存在的共享表、常驻模板、完整batch证明、instancing、bundle和任务执行均复用，不重复建设。

| 本块出口 | 当前结果 |
| --- | --- |
| 当前来源归因与已确认重复工作整改 | 已实施下表保留项，并补齐最终102来源的111/113诊断；不将历史分段移植为最终结果 |
| 两项既有流式容量失败 | 测试职责修正，机制/拒绝与真实绘制各有断言，原生产预算不变 |
| 普通项目可见与必要像素/行为 | 最终104 DLL完成120帧、4断言、3截图，独立材质及cell消费者通过 |
| 原规模联合预算与持续资格 | 未闭合：单次100样本CPU P95为13.4941ms；测前物理余量4.840GiB低于6GiB；没有执行正式长窗/三轮 |

## 2. 成本归因与实际消除的工作

开工02诊断绑定00来源，12帧/8样本、完整6814字节；Production5.1075ms、Graph2.0712ms，native prepare累计4.4143ms、execute2.7563ms存在重入/嵌套，不能相加。22任务、110000候选、约29657选中行、9897实例值、11视图；native模板新建0、当前检查2417。该结果用于选owner，不授102最终分段。

| 原owner | 保留的因果修改及真实工作边界 |
| --- | --- |
| native Templates/Bindings | 暖memo不再重写相同Weak项；单次不可变准备合并共享mesh/material/light/environment/shadow解释。相同bind group不足以证明纹理/环境依赖相同；当前buffer、page offset/range、pipeline等仍检查 |
| Production排序 | 删除最终phase全序会覆盖的opaque预排序和小phase缓存键临时整理；混合透明尾稳定分区后保留完整phase/depth/id、后到前透明序和legacy/nonwriting屏障 |
| GeometryBatch可见行 | 每view共享不可变Arc backing及batch范围，取消逐batch独立visible Vec；COW只复制本范围，自包含wire不变，完整tail/跨batch检查不删 |
| Graph/RHI附件 | target/depth共享Arc字符串；命令列表本来已经共享，未再建设一套列表缓存。值比较、格式和旧帧寿命不变 |
| Recipe录制/捕获 | 实际light/shadow group不变时省去重复encoder绑定，pipeline切换保守重置；当前借用内只收集一次相同依赖，保留最终实际依赖sort/dedup及透明顺序 |
| GPU backing核算 | 按真实wgpu对象Eq/Hash去重，唯一纹理才计算mip存储；不可变Recipe保存唯一依赖的位置，Clone重置，不复制GPU handle、不新增资源/完成owner，Trace仍保留完整别名 |
| Streaming | 原progress插入、ACK、最后pin移除维护精确pending数；为0时取消逐draw树查询。scheduled/CPU命中不能推进ready，真实submission维护和退休仍执行 |
| RuntimeRenderer | 原cache miss点的旧CPU产物释放与当前Graph构建最多用一个scoped worker重叠，返回/发布前全部join，小输入/单核/创建失败串行；未把析构移出本帧 |
| DrawResources命名geometry槽 | 纯typed handle解释沿原不可变资源事实复用；所有可变入口清除、COW/wire不继承不匹配事实，不代替当前GPU准入或完成 |

首红计数包括4/32/256多规模：Recipe重复依赖批次访问32/256/2048收敛到首次捕获后零重走；当前registry共享查询收敛到唯一source；命名geometry槽比较4/32/256收敛到各1。每项保留tail、同ID异值、错代、COW、取消、部分上传重试、队列/上传/退休上限与真实完成点消费者。具体计数和原始断言保留在各编号日志，不能用测试数量替代工作量或整帧收益。

95–98在92来源上逐步排除归因：两次紧邻完整shared validation仅约0.132/0.080ms，真正约0.991ms来自geometry_handle命名槽解释。不能再写成“1ms重复完整校验”。102修复后104相对93的CPU均值13.4340→12.5633ms，是单次合并反馈的观察差异。

### 最终102来源的诊断

[113 Production/Graph](../../../../.artifacts/371/s2-long-20261021-002857/113-chain-diagnosis/04-profile.json)：两文件临时探针、12帧/8样本、1751字节完整，无截断。

| 区间 | 均值ms | 包含关系 |
| --- | ---: | --- |
| Production | 4.0820 | 与后续Graph互斥 |
| Graph整体 | 1.3091 | 含下列prefix及报告/返回 |
| Graph prefix | 1.2228 | 含base构建、查找、释放/构建和publication |
| scoped释放与构建 | 0.9724 | 其中append0.1794、compile0.6289；剩余约0.1641，不能再重复计入父区间 |
| cache publication | 0.0190 | 不与上述构建重叠；不是当前主要热点 |

同一113运行renderSubmit11.3197ms，减去Production和Graph后5.9286ms仍包含native、接入和资源/寿命处理，未伪造其完整细分。Graph在prefix之外约0.0864ms。这个113窗口仅归因，不代替104无probe测量。

[111 native](../../../../.artifacts/371/s2-long-20261021-002857/111-native-diagnosis/04-profile.json)绑定同一102产品来源，但属于另一次12帧诊断：prepare3.7974ms；resources/join2.8037ms，其中scan1.2616ms；参数安装0.0431ms；capture/publication0.9048ms，其中template0.8187ms。prepare未覆盖余量约0.0458ms。参数worker验证0.2992ms、整理1.3471ms（mapping0.8855、slot0.3726、index0.0255）与resources重叠，不能视作可直接减掉的主线程时间。灯光0.0598、阴影0.5291在原准备内；backing census0.5108ms是flush间累计，可能含前帧操作，不能全部归到当前shadow子区间。前帧encode0.6520ms包含recipe capture0.1625ms。111与113不能拼成同一帧的精确加总。

104无probe整体CPU12.563338ms、update1.231782ms、renderSubmit11.307461ms，其余约0.024095ms；Present wait0.233943ms另列。以上仅均值可作这个余量计算，各阶段P95/P99不能相加。

### 被否证/撤回的分支

- 14预收集全部GPU对象集合使template约1.12→1.59ms，恢复原owner。
- 82完整draw命令匹配搬到首次匹配未消除整链工作，CPU13.9987→15.0770ms，P95/P99升到18.2288/23.3126ms；四core文件恢复，新增候选测试/合同删除。
- 106用原PlanSources把设备限制检查8/64/512收敛到各1，97 native owner、152直接消费者和3真实native通过；但109单次CPU13.1333/P9514.5015、GPU8.1174ms，没有观察到整帧收益。三文件按102原字节恢复并刷新mtime，保留109失败反馈，不重跑挑绿。计时差异本身不足以证明全部回退都由该修改造成。

参数slot小修改、pipeline缓存、跨帧GPU有效性/预算ledger和新任务池没有凭猜测实施。当前结果没有证明必须推翻已批准的CPU设计；也没有证明所有可能优化均已穷尽。主要缺口是持续性能和资源准入证据，不能把0.1941ms的短窗缺口包装成确定的新架构首因。

## 3. 两项流式失败的分类与保留边界

原失败均已在施工前来源复现geometry.visual_upload_capacity：

1. cell_two_cooked_cells_use_real_segmented_gpu_ack_and_shared_retirement。
2. cell_project_host_real_gpu_upload_receipts_migration_and_retirement_are_consumed。

首因是配方混用了“极小真实分片上传”和“能够真实绘制”的职责。bytes_per_frame=8、staging=32可以验证分片，但最小clear本身需要144字节，不能期待空view或draw成功。原8/32分支仍验证真实分片、每帧≤8、staging≤32、ACK/共享/迁移/卸载/退休，并明确断言visual capacity拒绝；独立4096/4096绘制分支检查实际像素、消失/保留和真实完成点。没有提高生产上传、resident、ticket或退休预算，没有删断言或skip。

这是fixture职责修复，不是通过增大生产预算修引擎。终验foundation50包含这两项；[42图像](../../../../.artifacts/371/s2-long-20261021-002857/42-cell-pixels)保存红常驻、蓝cell及全卸载去蓝留红。原8/32拒绝仍是产品边界。它们不证明原32m/s、1cell预取、两个32MiB cell的完整BM08长窗，本次没有重跑或重新授予BM08资格。

## 4. 普通项目结果与二进制

最终可用DLL：[104-engine_runtime.dll](../../../../.artifacts/371/s2-long-20261021-002857/104-engine_runtime.dll)，25,374,208字节，SHA256 e81358522604db4906622003f673f36f757ee4ae4dc6d500ad787900e98ad527；绑定102-source.json，未安装。module仍为07-project_runtime_module.dll，SHA a35dada34c75587ab021bc2d10270514b672281c8515a6b79f126f4659c16a6f，包/CLI/场景身份复用。

[105最终可见运行](../../../../.artifacts/371/s2-long-20261021-002857/105-visible/result.json)：120帧和120tick、4条相机/提交行为断言、1/60/120三张required截图全部通过，实际已查看首末帧及cell图像。对照：[首帧](../../../../.artifacts/371/s2-long-20261021-002857/105-visible/capture-1.png)、[末帧](../../../../.artifacts/371/s2-long-20261021-002857/105-visible/capture-120.png)。相机运动改变构图，密集彩色受光物体、阴影/雾等沿真实PBR链消费；这是原K1验证负载，不是美术精修游戏。

K1原负载为10000实例、100 resident mesh、128材质、96 BC3全mip纹理、11视图，保持原运动/画质；不能冒称S1的10000 unique mesh或完整BM矩阵。材质替换、法线/粗糙度、alpha/mip与光影的独立像素/行为由53/69及最终102直接消费者覆盖；加载/退场另由ProjectHost真实cell消费者覆盖。不能把这些独立证据说成一个普通游戏已经完整跑遍全部材质/流式路线，更不能用画面非黑证明材质正确。

## 5. 原规模反馈、内存与资格

下列30-old及104均为同K1、104帧/4预热/100有效样本。30-old使用开工旧DLL，104使用102源新DLL；两次相隔数小时，不是随机受控A/B，差异不能全部归因于代码。

| ms | 30-old / 开工旧DLL | 104 / 最终102来源 |
| --- | ---: | ---: |
| CPU mean | 15.710462 | 12.563338 |
| CPU P95 | 17.2979 | 13.4941 |
| CPU P99 | 17.8275 | 14.1341 |
| GPU mean | 8.162509 | 7.912520 |
| GPU P95 | 9.684992 | 8.304640 |
| GPU P99 | 9.808896 | 8.368128 |

CPU均值观察差异-20.032%，CPU P95仍高原13.3ms门槛0.1941ms；P99虽低于16.67ms，只有约1.289秒采样，不能授持续P95/P99。开工01的16.9181ms和§0.26旧20.8507ms都仅8样本，不替代上表。71、78、82、88、93、109等所有中间/负收益反馈保留，不把最终保留源称作“多次重跑选绿”。

104测前FreePhysicalMemory5,075,032KiB≈4.840GiB，已在Player启动前低于6GiB；109候选亦仅约5.011GiB。104进程峰值private commit2,451,718,144字节、working set1,213,591,552字节，仅是该进程的短窗观察。不能把测前环境占用全部算作引擎，也不能从物理余量扣工作集伪造达标；完整owner/role、环境归因及全程资格未完成。原6GiB门槛未降低，不关闭其它应用或改宿主。

104正式内容加载报告37个loading presentation、等待471.27ms，采样起止asset pending为0、CPU resident/mesh/texture数量相同；累计GPU上传和staging随动态运行变化，仍有在途ticket，不把它们写成“全运行资源不变/无在途”。本次没有完整BM08、持续原规模、OS显示关联或正式三轮，按371-G独立延期；不以资格延期掩盖实际功能故障。

## 6. 正确性、失败与施工成本

最终102来源的去重有效终验证据为436项：101组合单测231（几何121、RHI16、renderer48、native默认46）＋102 native ignored50＋102直接消费者152＋102真实native integration3。100局部5项已包含，不重复相加。五个组合默认ignored不计入该436，不授其新版专项资格。108候选的437项虽全绿，候选已撤回，不把它们冒称102最终新执行；最终436按确切原字节/配置复用。

保留generation/revision/epoch、取消/迟到、同ID异值、不可变packet、完整tail、COW、相机/透明/阴影、当前资源、部分上传重试、cell原子性、共同上传/staging/队列/在途/退休和真实GPU完成消费者。scoped fmt、core/default及native/default检查、scoped Clippy通过。最终core闭包140条诊断=core136+依赖4，native闭包总157，102/108相对各自前次无新增；但整个窗口77新增了两条encoding_backing mutable_key_type警告，真实wgpu Eq/Hash为稳定对象身份，已审查，未新增suppression或刷新ledger。不能说整个窗口零新增warning或全仓严格Clippy已绿。

[112失败账本](../../../../.artifacts/371/s2-long-20261021-002857/112-failure-ledger.json)保存43份failed runner回执及完整首错上下文，包含意图内首红、配方/编译错误和撤回分支。实际返工包括：integration数组参数误传为一个逗号字符串；String/Arc迁移类型错误；23/90误用缺native组合的core私有单测后转正确装配；85 fixture的shadow/color及2×模板、额外buffer期望修正；86错误RhiCommand variant修正；81候选反例light.view_identity失败后随82整分支撤回。58错误feature不能证明预期探针生效；78首次启动命令错误未启动Player；95两次摘要解析取错字段、warning初读stdout遗漏诊断均改用真实stderr，未因此重复Player。catch_unwind预期panic与失败用例必须分辨。

[48全部runner成本](../../../../.artifacts/371/s2-long-20261021-002857/48-costs-preliminary.json)及[114手动/Player成本](../../../../.artifacts/371/s2-long-20261021-002857/114-additional-costs.json)互斥归集，含失败、撤回和探针：

| 成本 | 秒 | 分钟 |
| --- | ---: | ---: |
| 32次Release构建 | 7132.277 | 118.87 |
| 85次Test构建 | 2428.056 | 40.47 |
| 9次Clippy | 95.564 | 1.59 |
| 12次Check | 138.015 | 2.30 |
| Cargo合计 | 9793.912 | 163.23 |
| 测试执行（862.897 runner＋67.566手动） | 930.463 | 15.51 |
| 35次Player外层准备及运行 | 1487.293 | 24.79 |

35次Player中18个诊断变体、14个无probe反馈、3次可见运行，不能说全窗只有一次诊断/Release/反馈。18版探针和若干私有组合测试构建明显增加了施工成本，属于本窗施工组织的实际不足；固定SSD/native独立编译并未消除大core与私有组合测试的费用。最后113两文件probe仍触发完整core闭包，不能将native小改2.84秒推广到所有测试。本窗未进一步扩构建框架；后续应消费当前归因选最小owner红例，合并产品修改后终验，不能从历史20.85ms重新诊断，也不再按单字段重启整链。

104反馈外层44.255秒，105可见外层36.903秒；105 child16.549秒已包含在外层，差值20.354秒是准备/编排剩余，不冒称纯Compiler。104约1.289秒采样亦嵌套于运行，均不再次相加。整个窗口另计118实际开工→收尾，不用上述小计替代墙钟。Compiler/Cook、BM03、workspace、正式三轮调用均为0。

## 7. 收尾与下一继续点

18版probe全部有恢复回执，最终386来源无漂移；106三文件和111四文件、113两文件均核原字节并刷新mtime。当前最新SSD Release保留的是113探针构建，不能直接拿它当104最终产物；下一次真实构建会因mtime正确失效，不清缓存，也不为恢复动作机械重编整链。当前可见/反馈只消费冻结104 DLL。

所有测量段只等待，没有并行构建、检索或编辑。Player由Windows Job托管、退出/组释放回执保留；118核对最终无本窗后台施工。未修改安装/宿主/SDK/Physics/游戏或冻结版本，未重试或绕过历史策略拒绝的清理目标；本窗证据保留，不需要清理共享缓存。

已闭合的是上述源码/消费者出口；原三个整改大块、§0.30、S2-D/S2/S1/371保持partial。不是“所有3D功能只剩一个性能测试”：S2实际配方/正式持续预算、371-G完整内存/显示/压力矩阵仍各有缺项，S3/S4本窗未展开。没有新的已确认当前产品崩溃/错误绘制故障据此延期。

继续同一块时复用102/104/111/113及其失效边界，先检查是否有新的真实功能故障或可计数重复工作。CPU短窗尾部和6GiB环境准入仍未闭合；不得在当前环境失败下盲跑正式长窗或反复挑100样本。若要改跨帧资源证明、预算ledger、任务池或命令粒度，须先提交具体合同、反例和选项讨论；当前数据不支持仅凭0.1941ms缺口宣布必须推翻设计。保持原预算/规模和Windows only，Android继续延期，不归档371。

### 最终结束回执

本窗实际结束2026-10-20T23:06:23.083Z，从原开工起累计6小时37分（397.43分钟），未用满8小时；22:56:22 UTC之后仅做身份、文档与进程收尾。最终无本窗Cargo/rustc/Player后台，详见118-closeout.json；不表示整块或S1/S2/371完成。

[118结束回执](../../../../.artifacts/371/s2-long-20261021-002857/118-closeout.json)保存实际墙钟、最终身份、全部费用与进程复核；[117收尾自审](../../../../.artifacts/371/s2-long-20261021-002857/117-closeout-audit.json)检查9份文档链接和声明输入，均无缺失或漂移。
