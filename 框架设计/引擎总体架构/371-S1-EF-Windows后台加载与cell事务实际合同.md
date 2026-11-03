# 371 S1-E/F：Windows后台加载与cell事务实际合同

## 2026-10-23 已释放句柄历史与单项释放（阶段41）

原RuntimeAssetLoader的同步/异步句柄继续共用同一owner。成功释放只递减自身实际持有的decoded cache项，最后一个引用才移除；显式依赖闭包逐项释放，不再为每个句柄扫描整库。已释放状态只作有界历史，最多保留既有StreamingBudget.max_handles条最近记录；异步请求和retain_handle准入需要额度时先回收这类历史，再按原活动句柄上限判断。同步load不新增异步准入限制，活动消费者/失败状态不会为回收历史而被淘汰；历史过期的句柄查询返回None，旧generation仍拒绝，handle ID不复用。

fail_pending内部释放后转为Failed时撤销该次临时历史条目，避免以后显式release被同ID旧条目提前淘汰。预算缩小时同步收敛历史与队列容量；新增队列的实际capacity接原借用内存观测。该修复只限制已释放句柄记录，不代表诊断entries、HashMap高水位或全部进程内存有界；原append-only诊断保留策略、raw/typed消费与GPU真实完成退休合同保持。最终36项loader、57项foundation及4项真实GPU消费者通过；二进制及原规模结果另见阶段41记录，不以源测试授正式资格。

## 2026-10-22 当前loader故障与容量补修（阶段36）

见[36记录](阶段完成记录/2026-10-18-371-S2-Execution-Chain-Continuation/36-S1故障终结与真实GPU压力消费者.md)。完成channel断连关闭新准入、pending终结WorkerStopped；只有JoinHandle真实finished才移除worker及current/retired operations，断连不能冒充payload退休或释放预留。shutdown保留已终结首错并处理poll自动移除worker的竞态。同源新generation/取消隔离及原operation/ready上限保持。

finished typed decoded payload在decodeWork到completion发布间借用观测实际allocation，析构时在backing仍活时移除自身记录，不额外持Arc；Off不扫描payload。parser scratch与解析中的中间结构仍未接，不能把这次补接称完整decoded/全owner覆盖。

真实双32MiB cell消费者：6 operations闭包在75%=4时原子拒绝；GPU实际闭包33,554,476 bytes在75%=25,165,857 bytes准入中，B报geometry.upload_resident_budget且A保持active，最后真实退休归零。无部分World提交，不授physics/nav降级或完整BM11/流式资格。当前loader31、受影响Host39/worker12及真实GPU消费者有效；阶段35未变owner证据按身份复用，S1-E/F仍partial。下文阶段35和更早摘要均为历史身份。

2026-10-21阶段35当前复核：旧无consumer operation按ID保留reservation至真实完成、同源新generation独立准入；完成payload实际128上限含阻塞sender，29 loader通过。双真实32MiB cell的A十次取消/新代/ACK、B持续active、独立红蓝像素及真实退休通过；F03缺chunk/hash/truncation/decode/dependency/layout明确拒绝，无World发布。完整故障/压力/流式资格仍partial，不把这些子例授全F。见[35记录](阶段完成记录/2026-10-18-371-S2-Execution-Chain-Continuation/35-World热消费收敛与S1基础消费者.md)。以下带日期摘要保持各自历史身份。

2026-10-21 §0.33消费者补充：新增F01纯空间burst与100个实际prefab异步请求、下一tick销毁半数及最终释放；F03实际缺chunk/损坏hash错误终结、无World发布。测试按冻结PC配方显式配置max_operations=256，生产默认64保持；prefab完整id/GUID/sub_asset身份严格校验。default及real-wgpu组合各通过两项，后者不自动等于GPU ACK测试。这些是实际owner/Host primitive，未完成BM02/BM03/双32 MiB cell上的全F01～12脚本，也不授完整BM11/压力或全程内存资格。详见[32记录](阶段完成记录/2026-10-18-371-S2-Execution-Chain-Continuation/32-S1-Windows基础配方与故障压力执行链.md)。

2026-10-21用户修订：当前Windows最低系统available为4.5 GiB（4,831,838,208 bytes），以[预算合同](371-S0-B-规模配方与测量预算实际合同.md)及371-budget-v2为准。下文原6 GiB结果保持历史身份；不改变资源生命周期或授予完整资格。


当前增量（2026-10-14）：状态`pc_static_cpu_original_scale_release_qualified`。本续窗减少view计数ID展开、冻结资源句柄一次排序后在原租约表有界批量遍历；新增Animator2D原owner消费既有archetype候选，避免静态3D每tick全实体组件查询。当前state/generation/live lease、动画禁用/退休/代际合同保持，无新缓存/权威/schema。51项去重验证（36项不受新增动画候选路径影响的owner/geometry消费者复用，13项动画owner与2项FrameLoop消费者新执行），六文件fmt及最终库Clippy通过。BM03/BM02各三轮独立Release Summary、每轮3661样本全部通过；六轮最差CPU P95/P99 12.1931/15.0395ms，GPU 6.971392/7.554048ms。本Windows静态CPU与原规模验收整块闭合。原10000unique mesh/256material/64texture/10M triangle/1GiB decoded、CPU/GPU13.3/16.67ms与6GiB预算保持。S1/D/E/F仍in_progress；Off成本、最终Windows显示、完整加载/流式/退休及G/H待闭合；Android延期，本轮未改安装/config/Provider/权限/clock，371仍唯一当前施工。 逐项generation/state/lease检查与实际completion/pins保持，改动不构成驻留缓存或新资源owner。见[续窗记录](阶段完成记录/2026-10-13-371-S1-Static-CPU-Continuation/00-总览.md)；此前窗口仍为历史身份。

2026-10-12；`pc_bm14_static_cook_release_lifecycle_cost_windows_passed`。完整BM-14 static五轮、普通Windows Release双cell长生命周期及限定成本窗口通过，见[本窗记录](阶段完成记录/2026-10-12-371-S1-BM14-Windows-Release/00-总览.md)。**D/E/F整体仍in_progress，S1未完成**；完整运行规模/流式事件预算及D最终OS显示/探针资格pending，Android延期，默认contain安装未更新，371保持唯一当前施工。

## 原有owner内的实现

原RuntimeAssetLoader承接真正后台I/O、64KiB分段读取、哈希及typed解码。原load_async返回Loading；新request_async返回独立consumer handle。一个有界worker以队列优先级调度，运行中的单项不抢占。共享operation独立于consumer；A取消不伤B、全取消迟到结果丢弃、取消operation从当前身份查找中脱离，但按原operation ID保留在途预算到其真实完成/worker终结；同源新generation须重新完整准入，容量不足仍明确拒绝。generation跨loader唯一，完整record/cooked身份用于原decoded_cache；没有第二缓存。旧同步consumer与后台完成合并引用，不覆盖原持有者；未获得cache引用的异步consumer取消不扣同步引用。Prefab/Scene复用原严格schema decoder。

PC后台完成结果的实际持有上限为min(max_operations,128)，包含worker已解码但阻塞发送的那一份；mailbox最多保留该上限减一槽，背压时operation/in-flight/resident预留不得提前释放。此上限不降低256活跃请求档、不增加worker或第二缓存；host按原poll上限继续消费，所有真实结果必须终结。Android子档仍延期。

默认预算：64 operations、4096 handles、16384 dependency edges、单metadata/source文件16MiB、in-flight256MiB、CPU512MiB、decode32MiB、每poll8 completion，DFS深度128。解码在分配前按JSON×64/audio×4保守包络准入；纹理像素另计。cell prepared副本、缓存与在途共享CPU预算；不能把预算缩到已占用以下。CPU数值约束tracked payload/prepared的保守模型，不包括完整索引、任务元数据、allocator和driver总量；这些及进程RSS仍须G测量。数值是接口默认值，不是A规模资格或RSS测量。大资产须显式profile或后续Cook分块，不豁免预算。

原RuntimePackageBuilder以实际落盘字节生成身份。Scene/Prefab为pretty JSON Value字节；builder持有的通用payload按实际bytes。纹理hash为SHA256(metadata字节||RGBA字节)，size指metadata文件，像素尺寸/长度另校验和计费；sourceHash仍是来源信息。自动提取Scene/Prefab静态mesh/material强依赖，合并显式依赖。旧同步格式路径保持；严格异步拒绝不完整身份旧包并要求重建，未新增schema版本。

GPU上传复用原geometry.meshes/textures及completion/retirement ledger。opaque ticket绑定backend epoch；分段mesh和纹理上传，同帧重复pump共享字节预算。默认resident256MiB、staging4MiB、每帧1MiB、128 tickets；预算准入先于物理分配，并计入retiring。Ready来自实际submission Completed，部分上传不可绘制；共享ticket不重复上传，最后释放按真实完成退休，错误backend拒绝，device destroy返回Abandoned。Windows Player原backend自动pump与Host消费已通过；材质/pipeline实际渲染通过，但ACK并未涵盖全部pipeline预热，不授正式首次消费停顿预算。

原RuntimeInstanceLoader使用cell-activation.v1请求、原PreparedRuntimeEntities、原prefab_instances与逻辑RenderResourceManager。静态World3D可指定World/Asset/Render required；Physics/Nav和未支持实体域明确拒绝。Loading/Prepared/Uploading不可被World/gameplay枚举，真实Render ACK后commit发布activation epoch；CPU-only不能冒充Render完成。commit复核revision、World epoch、parent generation、实体碰撞及当前父变换；沿原空间结构预算收费。

soft placement引用不保活，重载获得新runtime identity。退场先核对World/实体代际、foreign children及原逻辑manager的native lease，再动GPU/World。普通prefab despawn不能绕过cell owner。迁移沿原KeepWorld事务，保留实体身份，独立retain原资源闭包、不读盘。首期迁移保守保留原Prefab完整资源/准备预算，未做子树精确资源裁剪；来源释放后常驻实例仍保活，最后释放回落。

## 已验证与明确限制

125项去重测试（新增25）：113 CPU及12真实离屏GPU；16文件scoped fmt、real-wgpu Clippy和default check通过。Clippy保留136项既有告警，未新增抑制；不是零告警声明。实际构建器、原实例、普通构建输入与真实GPU分别消费；双cell共享mesh/纹理、独占材质、删除cooked文件后缓存渲染、常驻迁移及最终回落通过。日志/source/harness/fixture/依赖/安装身份见`.artifacts/371/s1-def-windows-20261011/`。

2026-10-12续窗完成SDK→generated glue→native adapter→session→Host→Windows Player。load/release/migrate意图只在成功回调整批准入；cell回调与同步World/audio/particle/animator操作分开。请求ID非零且单调增加，64排队需求/128活跃或近期回执，终态可淘汰。项目只见typed状态/epoch/诊断、generation实体和非保活软引用，不见backend票据，也不能提供GPU ACK。查询按需借用原owner，生产不逐帧构造全cell快照。原资源manager和backend在模拟安全点处理真实上传回执、激活/卸载，零模拟tick不发布World变化；无render owner保持不可见并诊断。

续窗484项去重测试（新增10）与scoped fmt/Clippy/default consumer检查通过；原生owner、真实GPU Host和camera-only普通项目DLL均被实际消费。换World旧资源泄漏与SDK迁移runtime/authored身份错配在首红后修复。普通项目51帧完成全部事务主动退出，同包另跑41帧捕获，640×360左红4200/右蓝2097像素。完整退出与短窗像素是两份独立证据，不合并为同帧/性能资格；默认截图在frame_limit，提前退出不自动补抓。证据根`.artifacts/371/s1-ef-sdk-player-20261011/`。

E/F仍待BM-08/14 foundation正式资源/停顿/内存与成本资格；D最终DWM因果键/未支持路径、成功ETL/最终显示分类、正式开销仍未闭合。token权限未变，续窗0采集；G/H须待对应前置满足。Android用户延期，不授跨平台资格。

已验证的是dirty development源码、Windows离屏GPU及普通项目DLL/Windows dev短窗功能；未更新Provider/config、未跑Local CI/全workspace/正式BM/Release交付，不授安装可用、正式性能或S1整体资格。

## 2026-10-12 大资源、增量Cook与生命周期诊断

本窗状态`pc_foundation_payload_incremental_lifecycle_diagnostic_passed`，见[进度记录](阶段完成记录/2026-10-12-371-S1-Foundation-Payload-Incremental-Lifecycle/00-总览.md)。原Assembler的native mesh/material producer消费原artifact cache，按源摘要与既有recipe复用，typed输出/依赖/输出摘要验证，坏cache隔离重做；material Hit仍校验当前引用。纹理Produced/Hit与真实分阶段耗时报告修正。未改资源schema、预算或安装。

调用线程streaming准入改为非纹理使用已有JSON×64/audio×4包络（不超过max_decode），纹理仍预留完整decode上限；worker发布前同时检查本次预留容量与总decode上限。首红为小型真实包被32 MiB×小对象误拒绝，修复后owner与实际双cell通过。包索引、worker、共享operation/consumer、ACK及预算数值保持。

离线generator v2产生普通源项目：四张独特2048² RGBA8、两个Prefab各两可见实例、每cell32 MiB实际独特payload；Player不读源生成数据。原Compiler检查实际enabled/visible闭包、真实mesh buffers/RGBA摘要，共享内容不计独特字节；共享/常量/截断/隐藏/未绑定/缺资源负例拒绝。原benchmark driver接受BM-08/foundation-v1 Windows诊断Cook/build，继续拒绝qualification、headless cell ACK和Android。此consumer不扩展A的数值或完整BM-14负载。

同一Windows dev DLL三窗口分别取得20轮完整事务、2698帧截图、2970帧Trace。GPU纹理峰值64 MiB/4张，累计上传1543560992字节；42个空闲样本覆盖21次全量加载后回落（初始释放+20轮）。CPU保留camera-only场景基线1289字节，其余选定cell/CPU reservation/operations/tickets/GPU mesh/texture/staging/retiring域回落。四象限红/蓝像素3165/4194/1234/2121，实际四个实例可见。CPU tracked峰值478033735字节是保守payload/prepared模型，不是进程内存；RSS/private另采，driver总量不可得，不授完整内存资格。

原生执行宿主在验证API、解析执行符号后pin同一Engine DLL到进程结束。Worker有界退出可detach，因此不能立即FreeLibrary执行模块；ABI-only probe仍卸载，失败pin不调用执行入口。此前1200帧截断窗口曾0xC0000005，修复后同规模1200帧实际进程退出0且正常转换报告；pin是进程寿命，不是安装事务，也不保证异常退出继续运行。

冷Cook11个producer Produced；unchanged全部Hit/输出不变；material、独占mesh、shared mesh三种修改各仅1 Produced、10 Hit，输出变更资产精确。双cell子集冷35.56s、暖28.80s、三种修改约27.03–27.10s；没有证明BM-14完整10000 mesh/256 material/64 baseColor全集或正式增量成本。现缓存命中仍需处理大量RGBA JSON，性能缺口保留。

dev Trace CPU P95 3.586ms/最大1035.55ms，仅是诊断数据，部分窗口与CPU Cook/check重叠；独立玩法报告Off+measurement Trace仍有1028.54ms Update主导峰值，排除必须开启玩法Trace才能触发的解释。尚未定位所有首因，也未验证Release能否达冻结预算；不能把P95当首次消费无停顿证明。下一块从已有候选纹理准备/内容身份成本与缓存大payload处理查起，不默认建立新加载器或新诊断体系。

## 2026-10-12 大资源成本因果修复

本整块完成大资源调用线程与纹理缓存成本修复，状态`pc_large_resource_cost_repair_passed`。worker-ready四次大纹理准备960.83→0.783ms，双cell暖prepare28.80→5.23s，普通Windows dev独立Off玩法/Trace测量Update max1027.80→4.71ms、CPU max1028.54→11.33ms；20轮生命周期、21次资源全域回落、四实例像素与239项去重测试通过，scoped fmt/Clippy/default检查通过。**D/E/F仍in_progress，S1未完成**；下一整块为完整BM-14 static全集与Windows Release正式成本/生命周期资格，D最终显示/ETL缺项独立保留。Android延期，G/H未启动，contain安装/config保持，371仍是唯一当前施工。 见[完成记录](阶段完成记录/2026-10-12-371-S1-Large-Resource-Cost-Repair/00-总览.md)。

原缓存typed envelope保持，texture recipe隔离为`texture-png.raw-cache.v2`，metadata长度/JSON与RGBA原始字节编码，完整payload SHA先验后解码；旧包格式、预算、发布/隔离语义保持。原worker创建不可变verified Arc/pixel摘要，producer和GPU admission复用，sampling metadata继续参与身份；公开prepared Arc被替换/改变后回退实际像素hash，动态prepare仍完整校验。原公开owned load_texture_payload及同步fallback保留，worker Ready内部路径避免重复大像素处理。VerifiedTexturePayload字段/创建只在crate内，项目SDK/wire不暴露此内部receipt。

真正同负载64 MiB/20轮dev测量4009帧：CPU P95/max3.8218/11.3289ms，Update P95/max2.1519/4.7066ms，GPU峰值四张64 MiB及21次全选定域回落通过。v3只增加foundation guard至7200ticks，资源与循环保持；首3600帧正常进程退出仍按缺少session完成证据拒绝。source/harness/fixture/binary/external身份分别保留，数据属于本机dev诊断，未授完整BM-14、Release/scan-out/driver内存/安装资格。旧暖28.8s与约1秒Update记录保留为修复前证据；后续按本节与49/54路由，不再把两已修首因列作当前未定位问题。

## 2026-10-12 BM-14静态全集与普通Release窗口

原离线generator/Compiler实际核验10000独特mesh、10000绑定可见实例、256材质、64×2048² baseColor/10M triangles，并合并两个独特32 MiB upload cell。五轮每轮10331实际producer，冷全Produced、无改全Hit/输出相同，材质/mesh/共享texture各1 Produced、其余Hit/精确1输出变化，4材质共享依赖仍有效。Cook执行者test/unoptimized，WindowsRelease是输出目标，不授生产工具速度资格。

原Assembler操作内lazy引用投影修260次重扫，下一操作仍重建；CLI显式长Trace有界读取最高512 MiB、保留独立native报告；Player原采样时钟与逐帧扣Present等待完善真实wall/tick口径。普通60FPS空间分支改交真实elapsed给原SpatialFixedClock，原Host追赶/semantic/2D60政策保持，旧601.997s仅35310ticks失败保留。

新普通Release1920×1080/Vulkan RTX3070，22回落、实际退休后601.990s/36120ticks/主动退出通过。三组Off/Summary独立窗口warmup3600/sample4800、各81.82–81.84s与4910–4911ticks，双cell/64 MiB GPU纹理全sample不变。CPU/GPU P95最高0.7588/0.881664ms，Summary增量逐组通过；fixed frame3601–3900共300帧Trace通过。双方统一Summary必要探针，不授measurement-Off成本；约58.66 app sample帧/s不等于OS 60FPS资格。

275项去重测试、scoped fmt/Clippy/default consumer检查通过；各窗完整性能Summary/verification保留，但旧test脚本共享child/native路径会替换，原生终文件只属于末轮Trace。测量后仅修harness输出父目录，静态检查通过；执行/现行hash分存，不重造历史头部或冒称现行harness真实七窗口已重跑。source/fixture/harness/binary/外部身份和失败见[本窗记录](阶段完成记录/2026-10-12-371-S1-BM14-Windows-Release/00-总览.md)。

完整静态runtime容量、BM-08完整流式时延/停顿/失败/峰值及D OS呈现/ETL、焦点分类/探针成本仍pending；G/H未启动，Android延期，无安装/config/权限/Local CI/全workspace变更。本节为最新限定实际合同，历史dev/未跑Release段落仅保留其当时事实。

## 2026-10-12 Windows双cell事件预算与静态运行终态

三独立普通Release进程各3000帧、47次Active与45次已观察Released，原20轮配对事务保留；帧分辨率接收区间到Active上界P95为1.214303/1.199133/1.198945s，启动保守上界3.356274/3.020291/3.165673s。逐帧采样CPU resident/reserved峰值478033735/135342214 bytes，GPU纹理64MiB、mesh2520 bytes、单帧上传与staging各1MiB、retiring观测0、operation/ticket最多13/2，private最高709787648 bytes。通过的是此双cell输入的事件/启动与采样预算；Trace开销包含，OS文件cache未知，不授BM08全集、瞬时allocator/driver总量或measurement-Off正式成本。

同步CookedTexture Ready沿原DecodedAsset缓存保留验证像素并计入bytes_len，多个材质共享同一RGBA，删除包后准备仍成功；采样变体只复制metadata，旧logical占位规则与释放owner保留。完整BM02因CPU生产帧过载仍失败，BM03按fail-fast未执行；E/F/D/S1均不关闭。见[本窗终证与下一整块](阶段完成记录/2026-10-12-371-S1-Runtime-Budget/00-总览.md)，前述旧窗口pending/成本仅为历史身份，当前资格由49/54与该记录共同定位。

## 2026-10-12 静态生产帧续修的资格边界

本窗不改cell事务/后台读取所有权和Cook输入；GPU静态计划复用仍检查上传pending、resident source和texture存在，加载budget/分段共享completion/取消退休/跨backend与device abandonment直接GPU消费者通过。原三轮双cell事件与静态Cook仅按未变输入保留，旧binary运行窗口不授予本窗新binary。最新完整BM02 Release720帧已完成，但CPU/GPU预算失败；稳态矩阵与BM03未执行，E/F/D/S1均未关闭。下一整块继续生产CPU/GPU成本，Android延期；见[本窗终证](阶段完成记录/2026-10-12-371-S1-Static-CPU/00-总览.md)与49/54。

## 2026-10-12 CPU/GPU续修与流式资格边界

本窗未改变cell事务、后台读取、预算、Cook输入或资源释放owner；静态扫描复用不跳native lease/generation检查，失败准备仍走原重试。41项原3D生产消费者包含生命周期与GPU消费通过。旧Cook与双cell运行证据按原输入/二进制身份保留，不将旧binary资格转给新源。

此前CPU/GPU窗口的38.4330/15.567872ms为历史输入/二进制证据，当前终态以文末本窗记录与49/54为准。

## 2026-10-12 共享消费修复与资格边界

本窗不修改cell事务、后台读取、Cook输入或预算；场景COW与不可变draw共享不改变resource lease/generation、native提交/真实completion/退休边界，直接foundation/旧packet/生命周期/真实GPU消费者已通过。原Cook/双cell证据按消费身份复用，新source/binary不继承旧长窗资格。

165项去重生产测试与scoped检查通过；最新原规模Release720帧CPU P95/P99 22.0250/26.6712ms、GPU P95/P99 7.435264/7.470080ms，预算仍失败，稳态三轮/BM-03未执行。D/E/F/S1不关闭，Android延期、G/H未启动；安装/config保持。见[本窗记录](阶段完成记录/2026-10-12-371-S1-Perf-ETW/00-总览.md)及49/54。

## 2026-10-12 提交与解码窗口：本窗共享消费与规模资格

本窗未改cell事务、后台读取、Cook、resource lease/generation或completion/退休边界；GPU/lifetime/3D/U2消费者已通过。Cook/双cell历史证据按未变消费身份复用，新的source/binary不继承旧长窗资格；不因ETW解码完整或共享键绿例关闭S1。

上一窗口175项去重生产测试/scoped检查通过；其历史原规模Release720帧CPU P95/P99 27.2638/30.9682ms、GPU P95/P99 8.126464/8.977408ms，原预算仍失败，稳态三轮/BM03未执行。D/E/F/S1保持未完成，Android延期、G/H未启动；安装/config保持。见[本窗记录](阶段完成记录/2026-10-12-371-S1-Submit-ETW/00-总览.md)及49/54；此前各窗口数字为其原身份历史证据。

## 2026-10-12 pass与硬件队列生命周期窗口

本窗没有修改cell/background读取、Cook、lease/generation与completion/退休准入；原GPU/lifetime/3D/U2消费者通过。Cook/双cell历史证据按原消费身份复用，新source/binary不继承旧长窗资格；单pass校验复用和DMA输入成功不能替代总规模或OS显示验收。

176项去重测试/scoped检查通过；最新原完整BM-02 Release720帧CPU P95/P99 20.6570/22.4689ms、GPU P95/P99 7.336960/8.009728ms，原预算仍失败，三稳态/BM03未执行。D/E/F/S1未完成，Android延期、G/H未启动，安装/config保持；见[本窗记录](阶段完成记录/2026-10-12-371-S1-Lifetimes-CPU/00-总览.md)及49/54。

## 2026-10-12 帧提交与保留draw校验窗口

本窗未改cell/background加载、Cook、generation/lease/completion/退休准入，旧Cook/cell保留原输入身份。新Release终验真实运行组合已封存，CPU原预算仍失败；不继承旧长窗/稳态或规模资格。

115项去重测试/十文件scoped检查通过；最新原完整BM-02 Release720帧CPU P95/P99 25.9255/30.8423ms、GPU P95/P99 7.874560/8.713216ms，原预算仍失败，三稳态/BM03未执行。D/E/F/S1未完成，Android延期、G/H未启动；见[本窗记录](阶段完成记录/2026-10-12-371-S1-Frame-Submit/00-总览.md)及49/54。

## 2026-10-13 CPU身份与提交意图复用窗口

沿原native成功且有界cache保存original draw Weak、detached描述不持native lease。intent_format只有完整消费且render成功才mark；cold/readback/changedformat/commands拒绝，geometry只跳重复metadata、draw/binding计数和非geometryfallback保持。所有GPU准备/资源/upload/退休/真正绘制仍执行；旧Cook/cell不授新binary长窗。

132项测试/scoped检查通过。最终BM-02 probe CPU P95/P99 11.0417/12.1155ms、GPU P95/P99 5.393408/6.082560ms；原BM-02 probe及三次独立稳态通过；BM-03 probe CPU P95/P99 40.4243/43.8887ms、GPU 9.172992/20.671488ms，预算失败、三稳态未执行。D/E/F/S1未关闭，Android延期/GH未启动；见[本窗记录](阶段完成记录/2026-10-13-371-S1-CPU-Identity/00-总览.md)。


## 2026-10-13 独特资源原生录制窗口

原有界成功native plan保留同view连续段render bundle，共用wgpu原RenderEncoder路径；逐帧原命令/资源/extent/rect/退休准入保持，COW/纹理同handle替换/最后streaming pin释放使录制失效，旧bundle进入completion退休，不持CPU native lease。独特mesh不替换为共享，draw/pass/instance仍按每帧实际执行累加。

92项去重测试/scoped检查通过；BM-02 probe CPU P95/P99 10.8990/14.0645ms、GPU 6.541312/6.600704ms；BM-03 probe CPU P95/P99 20.8690/22.5616ms、GPU 8.582144/8.817664ms；BM02 probe及三稳态通过；BM03预算未闭合。D/E/F/S1仍未关闭，完整Off/流式/退休/最终显示另判定；Android延期，G/H/S2未启。见[本窗记录](阶段完成记录/2026-10-13-371-S1-Unique-Submit/00-总览.md)。


## 2026-10-13 CPU提交预算窗口

批量native lease检查仍逐资源验证代际、状态和原弱引用成员，只合并表锁，不跨帧缓存成功。图内只复用同一不可变pass最后一个成功write/reference声明；未知目标逐command错误及公开resource_refs Vec顺序/重复保持。opaque/TestAndWrite连续域按pipeline/baseColorTexture/material binding/完整batch identity排序，透明与不写depth屏障、相邻兼容判定保持；独特mesh不共享化。

状态`pc_cpu_submit_static_budget_failed`。批量原native lease检查、pass内成功声明查询复用和实际pipeline/texture层次排序已施工；136项去重测试、四产品/一测试fmt及scoped Clippy通过（既有warnings保留）。BM-02 probe CPU P95/P99 12.3282/16.9982ms、GPU 6.700032/6.823936ms；BM-03 probe CPU P95/P99 23.4652/28.8812ms、GPU 9.428992/9.620480ms；BM02 CPU P99超原预算，已停其三稳态；完整静态运行预算未闭合。原独特资源、预算和完整采样保持，实际三组件与native报告独立封存。D/E/F/S1仍未完成，OS最终显示、measurement-Off及其它完整加载/流式/退休资格pending；Android延期，G/H/S2未启。下一整块闭合Windows CPU热路径：沿本窗原Release数据有界区分encoded内plan遍历/geometry/bundle执行与其外图/RHI/资源消费，取得确定owner red后修复；已有Weak身份/参数安装和report校验复用不重复建设，原预算通过才恢复失败recipe三稳态，不同源找绿。无新ETW/提升；安装/Provider/config/权限/clock保持，371唯一当前施工。 见[本窗记录](阶段完成记录/2026-10-13-371-S1-CPU-Submit/00-总览.md)。


## 2026-10-13 CPU热路径窗口

仅在validate_plan_texture_residency一次不可变调用中借用最后完整成功绑定值/最后成功registered handle，完整binding Eq包含所有字段；变化、无效绑定、不同代际与跨调用注销仍走原查询，不跨帧保留成功或保活native lease。公开texture()与其它payload首错语义保持。

状态`pc_hot_path_release_build_allocation_failed`。当前不可变调用内复用完整成功纹理绑定查找和GPU注册handle，公开校验与变化/注销首错保持；118项去重测试、两文件fmt及对应scoped Clippy通过（既有warnings保留）。BM-02 steady-0 CPU P95/P99 12.6068/13.7391ms、GPU 7.534592/9.004032ms；BM-03未产生有效运行窗口，Release构建未完成，运行未资格；BM-02三Summary稳态通过；BM-03 Compiler内存分配失败，仅Cook无build/Player，运行预算未资格。原独特资源/预算/完整采样保持，有效BM02三组件与native报告独立封存；BM03两attempt无新有效组件可seal。D/E/F/S1仍未全部闭合，最终显示、Off和其它完整加载/流式/退休分别pending；Android延期，G/H/S2未启。下一整块闭合Windows大规模Compiler内存与正式稳态资格：先用有界进程/阶段峰值定位Cook发布后到build发布前的实际分配路径，建立最小owner red后修复，再以原10000独特mesh/256材质/64纹理/1000万triangle/1GiB decoded配方完成三Summary稳态；不再盲重跑完整Compiler或以Trace诊断成本授正式失败。候选可读接缝为DesktopExportPipeline.export_prepared与RuntimePackageBuildInput规范化/digest/build，尚未确定根因，不能把疑似clone直接改成架构前置；runtime pass收尾成本保留为诊断，不先重复优化既有纹理/参数/proof/bundle。安装/config/Provider/权限/clock保持，无新ETW/提升，371仍唯一当前施工。 见[本窗记录](阶段完成记录/2026-10-13-371-S1-Hot-Path/00-总览.md)。


## 2026-10-13 构建与大规模性能窗口（当前摘要）

状态`pc_build_closed_bm03_formal_cpu_budget_failed`。包归一化/write plan改为调用内借用，prepared export不再复制完整负载；16MiB owner真实红例峰值16,806,974/33,664,948 bytes降至19,990/100,727 bytes，原digest/字节/失效/发布语义保持，50项去重生产测试及scoped检查通过。原BM03包发布通过；首native冷编译600秒超时保留，仅续编译已有缓存82.5秒成功，再由原owner在6.767秒完成并发布真实Release组合。BM03首正式Summary完整3661样本CPU P95/P99 23.7833/25.7598ms失败，GPU8.291328/9.654272ms达标；首失败停止后两轮。构建已闭合，性能整块未闭合。原规模/预算保持；Debug反事实同60000draw/64纹理下10000组mesh缓冲pass收尾10.4–13.4ms，2组1.32–1.34ms，确定资源数量敏感但不授正式性能。下一步先审定保留独特mesh身份的有界几何缓冲分配、物理字节预算和真实完成退休整体改造，不继续盲跑Compiler或重复优化bundle录制；修复候选不冒称唯一根因。BM02旧三Summary只保留历史组件资格，本窗未授新组合。D/E/F/S1仍in_progress，最终显示、Off及其它加载/流式/退休资格pending；Android延期，G/H/S2未启。安装/config/Provider/权限/clock保持，371仍唯一当前施工。 见[记录](阶段完成记录/2026-10-13-371-S1-Build-Scale/00-总览.md)。此前带日期窗口保持历史身份；本窗未修改本合同资源/退休owner，上述提案尚未实现。

## 2026-10-14 S1资源/成本/工具实施边界

见[本窗记录](阶段完成记录/2026-10-14-371-S1-Foundation-Closeout/00-总览.md)与49/54。双cell/BM02三组真实measurement-Off/Summary成本及300帧过滤Trace通过；两侧显式共同CPU/窗口/资源必要probe，默认Off无采样，不称无探针Off。BM03第二Summary CPU P99 17.2992ms超过原16.67ms，整体成本资格失败/剩余轮未运行，短诊断不撤销。三轮双cell加载服务延迟/分项通过，不等于完整stall、全owner内存或系统/GPU余量；已有去重判定器复用，缺生产贯通，不另写第二真相。

工具保留run/frame/tick/view/origin/capture submission，较早capture核对同帧完整Trace；没有actual显示时间即unavailable，observe不授资格。BM01/09/12只新增空间owner正确性，不替代HUD/屏幕/完整压力recipe/逐F故障。G/H partial、全S1未关闭；Android延期，安装/Provider保持。原负载、预算、重复及OS因果合同不变。

## 2026-10-21 §0.31 S1 Windows收尾补接

当前状态见[30记录](阶段完成记录/2026-10-18-371-S2-Execution-Chain-Continuation/30-S1-Windows退出清单与基础消费者补齐.md)。原loader Work/Completion、读取Vec、typed cache及scene/prefab嵌套、snapshot描述/view metadata实际backing已补接；共享allocation/owner/role合并不改变生命周期，Off不遍历payload。首次capture只启用后续转换，parser scratch/decoded intermediate和其它未接入路径保留缺口，std/驱动opaque单列；不授完整覆盖、全运行峰值或预算资格。

BM01万纯空间/20真实AUI→GPU及四档dirty、K1 keyframe消费者通过；完整K0/K1仍缺。BM09千对象/三锚点各初始化+20次重定位、10运动探针，独立位置/投影/GPU误差通过；无角色/physics/nav声明。BM11逐F原规模与BM12完整静态GPU压力仍缺实际配方；统一driver未开放这些请求，不以空间单测冒充完整recipe。

本窗系统单点5.46–5.74GiB<6GiB，未改门槛或配置，无新Release性能、正式轮次或显示采集资格。既有核心取消/原子性/不可变packet/上传限额/真实完成退休保持；Android延期，S1/371仍partial。历史段落保持各自来源，不能将本补接倒授旧产物。

