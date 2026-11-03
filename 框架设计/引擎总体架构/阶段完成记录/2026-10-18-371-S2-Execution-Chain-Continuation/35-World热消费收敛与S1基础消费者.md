# 371 §0.33：World热消费收敛与S1基础消费者

**最终出口见第9节；第2～8节保留端点修复前已记录的来源和失败，不能覆盖最终身份。**

本记录承接同一次八小时授权的34记录，**整块、S1、S2和371仍为partial**。批量空间生产链已接通，当前八组合完整功能序列通过；CPU P95仍未达标，完整故障/压力配方及正式资格仍有缺项。没有安装或冻结游戏升级。

## 1. 窗口与范围

实际开工2026-10-21 **15:17:56 UTC**；本段续建不重置计时。22:47:56停止新增实现/验证，23:17:56硬停止。原工作树G:/gameEngin、分支codex/255-capability-catalog及原dirty保留；B路374隔离Gate A与其登记不变。

已读项目/施工skill、49/54、371 §0.33、B2合同、371-G、34及当前消费者。沿用户已批准A，在原World、事务、timeline、Projection和loader owner内收敛；没有新增World、身份注册表、持久跨次有效性证明或公开row指针。Windows only；Android、完整观测/显示和正式资格按371-G保留；系统余量4.5GiB=4,831,838,208bytes，CPU/GPU P95/P99仍13.3/16.67ms。

证据根：`.artifacts/371/s1-final-20261021-173806/`，下文数字前缀相对此根。前半窗34证据`.artifacts/371/spatial-bulk-20261021-151756/`一并计入费用。Rust1.96.0、--locked -j1、原Release配置及I:/gameEngin-build/371/cargo-target保持。

## 2. 首因与实际产品修改

早段Release归因仍指向模块/转换、原空间事务、begin tick、presentation和Projection，不能继续把它当GPU瓶颈。原通道已贯通，未重复建设renderer模板或新缓存权威。

| 确认的工作或故障 | 原owner内修改与边界 |
| --- | --- |
| SourceEntityId复制及成功路径错误字段格式化 | 共享不可变Arc<str>，Eq/Ord/Hash按内容；不intern，不改source身份。错误字符串只在失败分支构造 |
| 原World查找、typed列及历史工作集多余工作 | 私有source查找哈希化，typed archetype列布局索引；packed timeline与active/pending行工作表，删除同步swap-remove；外部确定排序保持 |
| 历史entity slot持续累积 | 原allocator复用死槽，checked generation、耗尽隔离、清除旧source backing；clone独立，旧key仍拒绝 |
| 宿主临时delta Arc穿越begin_tick造成万次COW | 仅复制tick元数据后结束本owner临时借用；外部旧delta仍保留，真实旧读者场景仍恰N次COW |
| 大enum热意图往返、完整结构搬动和中转发布 | 原buffer保留紧凑key/TRS列；混合命令按原序展开。final overlay与presentation排序小索引，完整求值成功后融合原列写入/原timeline发布 |
| 首次delta重复建合并表 | 唯一首批排序后直接移入不可变发布，同tick后续/重复仍走合并，保留last-write与mask并集 |
| 每次历史body分配、重复本次local canonical检查 | 每活行最多一个unique Arc spare，Arc::get_mut失败则另分配；只在同一独占即时提交内复用已成功local检查，world/inverse/camera/坐标检查保留 |
| 派生World状态和历史平行查找 | SpatialWorldState移入原timeline行，同次发布查找2N→N；tick0/显式tick/reset/树/删除/clone/旧Arc保持 |
| 同源顺序重复排序与唯一输入重复建表 | 最多一个≤16384项完整runtime ID序列及排列；所有当前key/TRS仍先检查，只有完整序列相等才复用排列/唯一性。重复、长度/顺序变化fallback，不缓存位置或有效性 |
| 无父根的单位矩阵通用乘法 | 根直接使用local矩阵，树仍父子乘法；与旧恒等乘法的1000姿态/正负尺度/极值参考对照，数值/逆矩阵/相机检查保留 |
| Projection多次hash及拥有型键复制 | 调用内借用dirty Vec键、单次entry；只去重已确认3D Transform/RenderState，出站ID独立持有，释放集合后clear/recycle。2D重复、无payload、旧proxy移除/挂回保持 |

真实底层工作按1/100/1000/10000等规模首红/修复，不把计数下降直接等同帧耗时收益。39/40一轮typed/spare计数下降但整帧未改善，保留失败并重新归因；后续44/45真实Host首红才确认临时Arc寿命导致COW。

主要源码：engine_runtime_core/src/{ids,world,archetype,spatial_contract,spatial_3d,render_extract,project_runtime_session,frame_loop}及其子模块；loader/streaming与直接测试。开发SDK/ABI/生成胶水边界承接34，不修改安装SDK、Physics、宿主配置或V0.0.5。

## 3. 流式正确性和容量

**F02真实首红**：所有consumer取消后，同源立即新generation被旧operation的Busy阻断。旧operation按原ID进入retired_operations，reservation及operation上限继续计费到真实完成；新代独立完整准入。Reading、Decoding及CPU完成暂停点各10次，反序/正序/交错放行，旧结果不复活，新代Ready，最终预算归还。57首红、58/59修复，75最终29项loader通过。

**ready完成payload首红**：实际积压130份超过128。原单worker mailbox容量为min(max_operations,128)-1，把一个阻塞sender持有的完成payload也计入上界；不提前释放reservation。74真首红130>128，75真实读取/解码积压及关闭通过。

**双真实cell**：两cell各32MiB、共64MiB；B保持active，A连续10次UploadQueued取消/新代/真实GPU ACK激活。旧ticket不发布，红蓝各630像素，最终World/cell/cache/reservation及真实GPU退休回零。原16MiB/帧、128MiB staging、256MiB退休检查保持。尾片用尽同帧上传额度时离屏draw拒绝；既有Player已有preflight/loading Present路径，补证同帧preflight=false、下一逻辑帧=true，不误报为新生产fatal或提高预算。77后在113受影响GPU消费者终验刷新。

**F03补齐的错误消费者**：132定向2项测试通过，覆盖原缺chunk/hash错误，以及新截断文件、哈希合法但不可解码、缺依赖、不支持压缩布局四子例。分别在Read/Integrity/Decode/admission首边界拒绝，无实体/active cell发布，关闭后operation/ticket/reservation/resident回零。这是现有机制的消费者补齐，没有伪造产品首红，也没有改变预算；不支持压缩布局不是“required capability不支持”全部通过的替代物。

## 4. 成本归因与最终原规模反馈

以下为端点修复前127源码/128 DLL、129单次600样本的历史结果；当前最终143/144/145见第9节：

| 最终消费者区间 | mean ms | P95 ms | P99 ms |
| --- | ---: | ---: | ---: |
| CPU excluding Present wait | 12.434794 | **13.6204** | 15.2088 |
| update | 11.957972 | 13.3074 | 14.8299 |
| renderSubmit | 0.285274 | 0.3613 | 0.4224 |
| GPU frame | 0.244277 | 0.377856 | 0.905216 |

CPU均值减update和renderSubmit约0.19155ms，是同采样均值的剩余量，不是新增计时段；P95/P99不能相加，GPU与CPU不相加。CPU最大118.9036ms完整保留。**CPU P95仍超13.3ms约0.3204ms，不能授达标**；P99此短窗入界不授持续/正式资格。

最近详细probe是105/106，**属于100源码，不是最终127源码当前分段**：64完整帧/60样本，逐帧互斥分区校验；frame13.60477ms，transaction5.72307、presentation2.83509、adapter2.42241、extract1.52764、begin0.51282、hash0.40939，其余约0.174ms。transaction内prepare1.08474、order0.04977、evaluate1.72756、write/publish2.42313、delta0.31533；adapter内module1.48643、decode0.39123、convert0.21965。后两组属于嵌套，不能另加。实际候选/评价10000、source lookup10001，已无万次宿主临时COW。106按原字节恢复12文件、刷新mtime，32源100冻结核对；后续只针对已确认矩阵/Projection工作修复，未再给最终源码套用旧细分。

以下无probe结果均为**BM01 foundation：10000隐藏纯空间实体+20真实HUD**；不是万可见PBR对象、BM03或10M triangle绘制成绩：

| 来源 | 样本 | CPU mean/P95/P99 ms |
| --- | ---: | --- |
| 14/15 | 60 | 33.7068 / 37.3115 / 119.4125 |
| 28/29 | 60 | 24.1479 / 25.4642 / 128.1751 |
| 39/40 | 60 | 24.4479 / 27.0907 / 115.3085；没有整帧改善 |
| 54/55 | 60 | 17.9842 / 17.7328 / 112.5084 |
| 80/81 | 60 | 16.3999 / 17.3192 / 111.4662 |
| 92/93 | 60 | 14.0431 / 14.3989 / 93.7631 |
| 101/102 | 60 | 13.9708 / 13.6470 / 108.6032 |
| 101/103 | 600 | 13.23269 / 15.2208 / 17.3770 |
| 117/118 | 600 | 12.249741 / 13.6540 / 16.3939 |
| **128/129，最终** | **600** | **12.434794 / 13.6204 / 15.2088，P95仍失败** |

跨时段环境未完全控制；60与600样本差异不能称严格单项因果收益。56同binary去capture因果对照P99=20.1834ms，支持百毫秒尾项含capture成本；不删原场景失败、不撤销预算要求、不靠重跑挑绿。

129启动单点系统可用4,838,309,888bytes，仅比4.5GiB门槛高6,471,680bytes；102单点4,749,672,448bytes仍失败。不能用启动单点授全程余量，更不能扣其它进程工作集伪造满足。未关闭应用或修改宿主配置。

## 5. 普通项目完整矩阵与身份

94先在91源码/92同engine身份完成八项目K0/K1×0/1/10/100%，各7201tick/37断言/末帧capture通过；八次enginePlayerCacheStatus均hit。历史32高dirty超时保持原证据，不改成通过。

**130在最终128 DLL再次完成八组合**，沿原120s超时，Off/unpaced固定tick：

| track | 0% wall s | 1% wall s | 10% wall s | 100% wall s |
| --- | ---: | ---: | ---: | ---: |
| K0 | 14.262 | 14.915 | 20.070 | 97.264 |
| K1 | 15.288 | 15.492 | 20.625 | 98.143 |

每组7201tick/frame、37断言、末帧capture passed。130复用八个真实合法包/项目module，显式S2D_K1_DIAGNOSTIC_DLL加载当前DLL；**这是当前源码集成，不能授八个原standalone交付新版资格**。Off/unpaced功能序列也不是120秒真实wall性能资格。独立128交付的129无override运行单列。

最终DLL SHA256：`433d5715ea0fd0c97f547d383cdedca5e687c5859cd3626fc0ab730a1573e234`；128 Game SHA256：`0dac3f54d1ea604a2c41948c0c00bdb1ba590e73bd41c622776dde76acc2a972`。127冻结32项源码；131绑定每组报告、包/module、Game/实际诊断DLL、scenario及capture。132之后只有foundation_faults测试源变化，133逐项确认生产源码与产物未变，因此不重建Compiler、不重跑八组合。

本阶段按371-A §4.0 foundation-v1明确**无light consumer**；曾将最终目标灯光条款误报为foundation缺陷，随后核对并纠正，不添加灯光或改变配方。

## 6. 正确性、失败及复用

- 125最终直接消费者115绿：session38、frame_loop23、render_extract14、packed_history4、world20、root参考1、native spatial13及native bulk2。不能用不存在的spatial_bulk_tests过滤器冒称后两项已运行。
- 113真实GPU/合同77绿：foundation61、contracts11、lifetime_gpu5。之后Projection改动有125直接验证；GPU资源/完成点与数学owner未再变，复用其有效范围，不冒称77全在133源码重跑。
- 75 loader29绿；后续未改loader，复用。132 F03两项是补充，旧一项与113重叠，不能简单相加为独立用例总数。
- 124热提交容量owner：10000/15000热提交与暖路径通过；20000原16384/tick拒绝，拆批10000+10000不能绕过，失败无delta/dirty/后半实体改动，下一tick恢复、旧pose/delta保留。不是完整GPU BM12。
- 126 scoped Clippy通过，既有148 warning保留；127生产修改范围fmt及133测试文件fmt check通过。不跑workspace、BM03、正式三轮或不受影响owner。

真实失败与返工保留：85方法可见性失败后修为内部pub(super)；95脚本漏插断言意外绿，不作首红，96真首红；109参考fixture四元数未归一化导致拒绝，110/111将IDENTITY_MATRIX误改cfg(test)影响structure子模块，恢复生产import并规范fixture后112/115通过；121新增测试slice漏借用编译失败后122修复；123将私有即时提交测试放external integration编译失败，移回原owner后124通过，未扩大public API。均属本轮施工错误，不能归为环境故障。109中断runner未落outer时间，131从原日志恢复test-body近似，outer未知，不编造。

早段首次typed/spare优化无帧收益、双cell同帧draw预算拒绝及其preflight定位、实际首红F02/ready均保留原stdout/stderr/报告。恢复probe时按原字节核对并刷新mtime，未清共享缓存。没有绕过历史临时目录策略拒绝；本窗证据根保留，测试fixture按原Drop清理；不删除共享证据根，不操作历史被拒目标。

## 7. 费用

`window-cost-ledger.json`合并34前半窗82账本及本段全部已记录调用，包括失败/返工：

| 类别 | 秒 | 分钟 | 范围 |
| --- | ---: | ---: | --- |
| 直接Cargo build/check | 3241.6604 | 54.0277 | 不含Clippy与下面Compiler外层 |
| Clippy | 231.5930 | 3.8599 | warning保持 |
| 测试执行 | 858.2898 | 14.3048 | 含手动结果；109只有body近似 |
| Compiler外层 | 9344.4940 | 155.7416 | prepare/Cook/export/build，非纯Rust编译 |
| 其中nested build | 9216.7885 | 153.6131 | **已包含在Compiler外层，不另加** |
| Player外层 | 941.6780 | 15.6946 | 含语义harness与窗口；非CPU帧总和 |
| driver构建单列 | 112.5676 | 1.8761 | 前半窗已有费用 |

带UTC已记录调用的合并时间并集14511.0086s（241.8501min），是下界；无UTC的手动调用/driver不进入并集。分类调用可能重叠，不能把上表简单相加冒称独占墙钟。128最终单次Compiler426.603s，129 Player14.351s；132增量构建7.3849s、测试1.0305s。整个窗口实际结束时间由最终closeout单独记录。

构建成本仍大：本段23次Compiler操作中15次消费不同产品/probe源码、每次约420–435s；八次项目变体约58–61s并复用engine。没有将完整Compiler作为每个字段调试循环，但多轮归因/恢复合并后仍消耗大量成本，不能宣称构建问题整体解决。下一窗若生产源码未变，复用128/130，不因文档或132测试变更再构建Release。继续优化须先合并同owner修改，再一次终验；不在本窗额外展开core/UI拆分。

## 8. S1出口审核及下一继续点

| 出口 | 本窗判定 | 未闭合内容 |
| --- | --- | --- |
| SDK/module→原World→history/presentation/Projection | 源码和普通项目已贯通 | 不代表完整371所有3D能力 |
| BM01 K0/K1四dirty完整功能 | 最终DLL八组合通过 | unpaced固定tick，不是持续预算资格 |
| BM01高dirty CPU | **partial，P95=13.6204ms失败** | 需稳定余量，不能因仅超0.32ms下调门槛；当前主要在update，不重建renderer |
| BM09 foundation | 沿30/32千对象三锚点20次/独立像素证据 | 非physics/nav/完整世界 |
| BM11 F01 | 原1000创建/100资源/下一tick销毁半数primitive存在 | 非完整BM02/03与双cell组合schedule |
| BM11 F02 | loader各10取消/新代/迟到及真实双32MiB UploadQueued通过 | 不将CPU/离屏局部证据授完整全部schedule |
| BM11 F03 | 缺chunk/hash/truncation/decode/dependency/layout子例通过 | required capability原规模组合及完整压缩输入/完整配方仍须映射，layout拒绝不是有效解压验证 |
| BM11 F04/F05/F06 | 已有immutable packet、真实completion/旧epoch拒绝、loader关闭等owner基础 | 未完成固定3帧/最大lag、20次resize/minimize/restore+loss、完整decode/render失败/超时组合，不能只链接owner测试授资格 |
| BM11 F07/F08 | 原预算拒绝、图负例和非法GPU提交前拒绝基础存在 | 原75%闭包预算/图各错误子类全配方未齐，不称全故障通过 |
| BM11 F09/F10/F11/F12 | 空间句柄原子性、共享ticket/ACK/cell迁移/soft ref基础存在 | 按foundation子集逐项补完整组合；sim/nav部分延期，不伪造ACK |
| BM12 | 原BM02三倍率K0三帧、当前热提交三倍率边界保留 | K0/K1完整轨迹、all-visible、BM03独特资源倍率与各耗尽消费者未齐 |
| CPU/GPU完整内存观测 | 新私有Vec/Arc backing接原owner观测 | parser scratch/intermediate、SDK/renderer/report等可接入缺口与std/驱动opaque分开；不以reservation/进程总量替代 |
| 系统余量、OS显示、工具与正式资格 | 未授新资格 | 启动单点余量仅约6MiB；全程、三独立轮、OS显示及工具独立消费仍按371-G |

**没有证据要求推翻当前World或3D渲染架构**；已确认的是原执行链实现中的查找/复制/排序/COW与工作集消费成本，以及未齐的配方消费者。批量边界调整已批准并落地，但不保证只要剩余施工结束性能必然达标。

下一窗仍属同一大块：先复用127/128/129/130，针对update内事务/历史发布与presentation的主要真实工作收敛；106只是候选排序依据，如需精确最终分段则一次有界探针，不能直接套旧数值。多个局部修改合并后一次无probe反馈；没有收益则复审假设。BM11/BM12消费者和完整资格另按表核销，不把全部未完成项称作纯性能问题，也不把正式观测重新作为产品开发全量前置。

本窗结束/外部状态及最终链接审核见证据根最终closeout；371保持当前，不归档，不授S1/S2/371全部完成。

## 9. 同窗最终端点收敛与出口

本节覆盖上文修复前“最终128/129/130”当前身份；原结果和失败保留。最终为143源码冻结、144无probe交付、145单次600样本、147八组合功能，148绑定32源及全部产物/包/module/scenario/capture。DLL SHA256：`3c5218251325c1b7013e7f2fbe7380d5e91b208b79deb0c9d578ddcdc5335678`。

### 9.1 最新归因、首红与修改

136/137对133源码作一次有界Release诊断：60样本，逐帧互斥区间完整校验；FrameLoop探针区间11.775595ms，transaction5.114082、presentation2.373055、adapter2.223495、extract1.178357、begin0.372358、hash0.375825，其余约0.13842ms。这是Host tick区间，不是包含native提交的整个CPU帧，预算仍看145实际CPU指标。transaction内prepare0.963030、order0.039000、evaluate1.481532、write/publish2.220043、delta0.297665；presentation求值1.753078，adapter内module1.325313/decode0.350658/convert0.243317。各内层是嵌套，不能再加到外层。仍为10000当前评价、10001 source查找，无万次宿主临时COW。137恢复12文件原字节、mtime和全部133源，临时模块删除。

依此确认presentation仍为较大消费，再审核原owner端点语义。129真实报告末帧的presentationAlphaBits=1065353216（1.0），证明实际端点消费者存在；这不是全窗命中计数。138真实底层重建首红：1实体两个端点重建2次，预期0；不是捏造性能失败。仅独立根、current/previous无父、同runtime身份/anchor、非祖先cache路径可复用对应不可变committed matrix/pose；禁用插值只取current。保留current非插值元数据、每次候选及origin-relative坐标/精度检查，评价全成功后才发布；中间alpha、树/祖先cache、基底不匹配仍原求值，无新owner/持久证明或公开接口。

139/143在1/100/1000/10000验证端点实际重建0、中间恰N、树和基底不匹配仍求值；当前visibility元数据、禁用插值及外部旧pose保持。116最终直接消费者通过，141 foundation62/contracts11/lifetime5共78项通过；141后仅删除派生Copy类型冗余clone，运行语义等价，143又刷新最终直接消费者。142 Clippy及两文件fmt check通过，原148 warning保持。75 loader29身份未变，复用。没有把116+78+29简单相加作独立最终二进制用例数。

一次PowerShell调用尝试串联Clippy和后续runner，被runner自身exit结束，实际只执行Clippy；核对产物后单独执行143，未重跑Clippy或虚报后续已执行。此调度失误及138首红费用均保留。

### 9.2 同配方最终反馈

| 指标 | 129修复前600样本 | 145最终600样本 |
| --- | ---: | ---: |
| CPU mean ms | 12.434794 | 11.909451 |
| CPU P95 ms | 13.6204 | 13.3214 |
| CPU P99 ms | 15.2088 | 15.1229 |
| GPU mean ms | 0.244277 | 0.249844 |
| GPU P95/P99 ms | 0.377856 / 0.905216 | 0.393216 / 0.923648 |

最终update均值11.420165ms、renderSubmit均值0.289668ms；CPU最大127.3469ms完整保留。**该短窗仍有预算失败，持续/正式资格未授**。同BM01 K0/100%万隐藏纯空间+20HUD、4warmup+600样本，不外推万可见PBR/BM03、K1持续或完整S1。137是修复前分段，不冒称143最终细分；跨时段环境差异不能全部归因单项修改，不重跑挑绿。

启动系统可用4643627008bytes，4.5GiB单点判定=False；无全程余量资格，无扣工作集、关应用或宿主配置修改。

### 9.3 最终完整功能与成本

147沿八个合法原包/module显式加载144 DLL，每组合7201tick、37断言及capture通过，沿原120s超时。是当前DLL集成，不是八旧standalone升级；Off/unpaced仍不授持续wall性能。

| track | dirty | ticks | 断言 | Player外层s |
| --- | --- | ---: | ---: | ---: |
| K0 | 0% | 7201 | 37 | 14.403 |
| K0 | 1% | 7201 | 37 | 14.874 |
| K0 | 10% | 7201 | 37 | 19.602 |
| K0 | 100% | 7201 | 37 | 95.732 |
| K1 | 0% | 7201 | 37 | 15.288 |
| K1 | 1% | 7201 | 37 | 15.955 |
| K1 | 10% | 7201 | 37 | 23.267 |
| K1 | 100% | 7201 | 37 | 96.460 |

同窗34+35最终累计：Cargo build/check 56.1918min，Clippy 4.0291min，tests 16.0162min，Compiler外层 169.9988min，Player外层 20.9295min，driver单列1.8761min。Compiler nested 167.7452min已在外层内，不另加；带UTC调用并集265.2499min为下界，分类不简单相加。构建/准备仍是很大成本，不能宣称已解决；136有界归因和144必要最终构建分别计费。失败/返工全部保留。

### 9.4 整体判定与继续点

本窗已闭合所列生产链、取消/ready缺陷、八组合基础功能和局部成本收敛；**S1整体仍partial**。BM11完整static/dual-cell逐F组合、BM12 K0/K1/all-visible/BM03和耗尽、完整可接入backing、全程系统余量、OS显示/工具与正式资格仍缺，Android延期。F03 compression layout拒绝不等于有效压缩流解码/全部required能力组合通过，热提交三倍率不等于完整GPU BM12。

下一同一大块先依据本次短窗判定安排CPU稳健余量/必要持续资格与尚缺配方消费者；优先复用143/144/145/147，先补具体缺口，不重建已有owner或盲跑Compiler/BM03。若需新增架构边界另给具体合同和反例；当前没有推翻World/renderer架构的证据。安装、宿主、Physics和游戏V0.0.5未改，不归档371，不把窗口结束称S1完成。

实际窗口结束时间与源码/链接最终审核由最终closeout记录，原8小时硬截止保持。

## 10. 本窗最终收尾

本次8小时授权窗口已于2026-10-21 22:47:29 UTC提前收尾，实际7.4927小时（26973.798秒）；所有已启动验证结束，无本路后台施工。S1及本整块仍partial；后续不沿用本窗继续启动施工。最终身份143/144/145/147及证据148保持，详见35第9～10节。

最终源码32项SHA核对一致，临时probe已恢复且模块不存在；本路run-root相关Cargo/rustc/Player/Node/Python进程盘点为空，未终止其它任务。54的B路374登记原字节保持。证据根和有效交付保留；测试fixture依原Drop处理，本段不再发起清理、不操作历史策略拒绝目标。安装、宿主配置、Physics、游戏V0.0.5无本路修改。

最终审计见`.artifacts/371/s1-final-20261021-173806/148-final-evidence-audit.json`、费用见`window-cost-ledger.json`、本窗收尾见`150-closeout.json`。保留既有dirty，未reset、未提交或安装。因剩余可实施时段不足以完成新的首红/实现/终验，不再新增诊断或性能重跑；时间边界不是现存性能失败的根因。

## 11. S1完整目标的收尾复核

本节在原窗口停止新增实现/验证后仅核对合同、现有源码入口及既有证据，不运行新测试、构建或性能窗口。上一goal轮属于实际进展：产品实现、证据与权威状态已更新；本次复核不缩小原S1整体目标。依据总施工§0.31固定退出表、§7的S1-E～H和371-A §4.0/4.4；371-G改变开发前置，不删除S1全退出义务。

| 要求 | 当前分类 | 已核对证据/消费者边界 | 尚需证明的出口 |
| --- | --- | --- | --- |
| S1-A/B/C与D/E/F核心 | 已有实现及范围内证据，不重新建设 | 143/144/147；141 GPU消费者及75 loader；34 SDK/ABI合同 | 继续复用；新修改仅失效实际消费面 |
| BM01 foundation | 功能矩阵通过，性能未闭合 | 147八组合；145 CPU P95=13.3214ms>13.3ms，600样本 | 保留失败；事务/历史发布为137已测较大成本，但137属于端点修复前源码，不冒称最终细分 |
| BM09 foundation | 基础精度消费者已有证据 | 30/32的1000对象、三个锚点、20次重定位及独立位置/投影参考 | 复用实际身份，不升级为角色/physics/nav证明 |
| BM11 F01/F02/F03 | 已实现部分真实消费者，完整配方未证 | foundation_faults.rs的F01、双32MiB F02与F03错误子例；75/141 | 补static/dual-cell组合及required capability、压缩数据输入边界；layout拒绝不等于全部F03 |
| BM11 F04/F05/F06 | 有owner基础，固定故障序列未完整贯通 | 141包含真实GPU寿命测试，不能代替指定完整故障脚本 | 补3帧/最大lag内容版本与关闭、各20次窗口事件/设备恢复、decode/render失败/超时终结；不可控真实恢复维度保留未资格 |
| BM11 F07/F08 | 预算/图拒绝基础存在，配方覆盖不全 | foundation预算负例与图owner既有证据 | 补75%闭包限额、各图错误子类在submit前拒绝；不提高生产预算 |
| BM11 F09/F10/F11/F12 foundation子集 | 空间句柄、共享ticket、ACK/epoch与soft-ref基础存在 | platform_3d_foundation及streamed_cells直接消费者 | 按371-A的foundation子集补跨owner组合；动画/物理/nav仍属后续阶段，不新增stub ACK |
| BM12 foundation | 缺完整实际GPU recipe消费者及资格 | foundation_bm12_one_one_point_five_two_scale_preserve_atomic_budget_rejection仅空间提交边界 | 真实BM02/03的1/1.5/2倍率、K0/K1/all-visible及资源/句柄/队列耗尽分别核销 |
| BM02/03/08/14 foundation | 有历史原包与组件证据，不是当前全矩阵资格 | 371-A/35历史身份与既有真实双cell证据 | 逐项核对消费身份、失效范围与缺维度；不因文档变更重跑Compiler或所有旧BM |
| D-MEM-01 | 部分缺实现 | loader Work/Completion、读取Vec、深层cache、snapshot与本窗新增backing已接；371-G剩余清单 | 补parser scratch/intermediate及World/SDK/renderer/report可访问backing；去重共享、保留在途/真实退休 |
| D-MEM-02 | API观测边界与资格缺口 | 371-G列std节点/Arc控制块、allocator/wgpu/driver opaque | 不能冒称完整覆盖；与尚未接入分开，不以估算或进程总量代替owner分配 |
| D-QUAL-01与D-PERF-01 | 余量及正式流式资格未闭合 | 145启动available=4643627008<4831838208 bytes；非全程数据 | 保持4.5GiB、32m/s、提前1cell、双32MiB及原限额；先准入再正式轮，不关闭应用或扣工作集 |
| S1-H/D-OBS-01 | 工具独立消费与实际显示资格未闭合 | 已有run/frame/tick/view/capture关联；无完整actual OS显示资格 | 核对真实No-Editor工具/交付消费者，未匹配字段保持unavailable；不无限扩建私有decoder |
| Android | 用户明确延期 | 371-G D-PLAT-01及当前Windows-only授权 | 单列延期，不扩大本次平台范围，不宣称跨平台S1完成 |

**完成审计不通过：不能把S1整体标为complete。** 下一实施窗应同时推进完整故障/压力消费者与已确认主要CPU工作，不再只围绕0.0214ms超线堆局部缓存；已有143/144/145/147保持可复用。上表是既有要求的核销，不是新增Gate或架构范围；新发现产品故障先最小首红修原owner。原八小时授权不重置；本次只完成收尾复核，没有后台施工。
