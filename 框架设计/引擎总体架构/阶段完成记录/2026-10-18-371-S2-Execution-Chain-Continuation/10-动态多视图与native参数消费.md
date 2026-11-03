# 371 S2-D 动态多视图准备与native参数消费（同§0.25）

日期2026-10-19，北京时间。用户授权“继续施工下一整块”，承接已批准A及09记录，同一未闭合CPU执行链。状态partial / source_consumer_verified / scale_budget_failed；本窗口修复和直接验证已收尾，整块预算、S2-D/S2/S1/371仍未完成。未更换安装或宿主配置。

## 实际实现与审改边界

仅改原render_geometry/build_inputs.rs、shadow/planning.rs与wgpu_backend/real/geometry/parameters.rs。不是新增资源owner、schema、命令粒度、GPU布局、shader或SDK；正式合同内实现说明先审改再施工，见[CPU合同](../../371-S2-CPU准备证明与动态消费合同.md)末节。

receiver在当前不可变调用内复用一次成功的完整view值比较；相同ID异分配仍完整比较，新调用不继承成功。生产append_scene_lighting直接借用同一GeometryBuildInputs已有compact_indices，避免重新构造全量source描述和ID表；旧入口保持调用内索引及重复ID检查。长度、选中ID/索引边界、当前源geometry/binding逐项保持；错位、越界、缺项返回unknown保守保留caster。深度8角计算和原误差guard不变。原flat深度范围作独立对照，实际生产入口验证不重建索引。

native原InstanceValues按Weak不可变分配身份+强度bits保存完整32float参数和canonical RecordKey。RecordKey包含完整ID/全部float bits及预计算哈希；哈希只选bucket，Eq仍比较完整内容，强制碰撞用例不能合并+0/-0。Records仍根据完整键、真实已上传content和bits决定slot/dirty，CPU命中不能使待上传数据clean。移除重复的完整键临时表，使用已有有界entry内的调用serial与本次局部索引，溢出/serial wrap/零容量/重排重新解析；不跨帧保存可见结果或GPU成功。相邻相同不可变view只在本次stage省重复map访问，其它view仍完整建键。原GPU容量/上传额度/失败重试/在途/真实completion退休保持。

新增entry实际Vec backing由现有capacity×真实size观测；HashMap桶、Weak控制块、调用内队列等已有未接入项仍是尚未接入，不冒称平台不可观测或完整memory census。完整观测与正式资格继续按371-G延期。

## 首因证据与失败处理

一次baseline源码诊断保留95输入身份。stderr总17479字节、上限16384字节，完整12帧分段分析明确失败；只分析完整前缀第5–11帧七样本，省略第12帧，不为补日志再构建/运行。五个probe文件按字节恢复后才实施产品修复。详见[诊断](../../../../.artifacts/371/s2-dynamic-20261019/04-profile.json)、[捕获限制](../../../../.artifacts/371/s2-dynamic-20261019/04-capture-limitation.json)、[恢复](../../../../.artifacts/371/s2-dynamic-20261019/05-restored.json)。诊断不是最终源码分段。

baseline粗分段：Production22.1069ms、Graph4.6623ms、native到capture16.4497ms；native容量2.6219ms，参数stage7.7318ms，内含instance memo3.5008ms、view memo累计3.9097ms、实例record key2.7021ms。灯光receiver独占约4.4426ms；finish排序/分批/验证分别为累计2.1493/3.3797/6.2197ms。嵌套不可相加，不把旧分段赋给最终修改。

06确定性首红：同一不可变receiver view比较实际32、预期1；稳定native完整实例key构造实际4、预期0。修复后保留完整值/flat oracle与新调用/COW/相同ID异值、溢出、碰撞和pending dirty反例。08检查点231项通过后仍补同一owner现有索引消费，故08不能授最终源，也不重复相加测试数。

11终验第一次128通过、1失败：新增索引参考调用后，测试计数器累计两次receiver范围计算512行，原单次上限256行断言位置错误。没有放宽上限；只将断言移回对应调用后。11失败保留，11b先窄例通过，再owner129和四个实际消费者终验。修复只在cfg(test)区；10与10b生产前缀按字节相同，四个11已构建integration二进制复用，避免为fixture重建全部集成exe。



本窗14中间无probe原规模出现显著性能回退：CPU/GPU mean 422.6873/41.9446ms，失败完整保留；19同环境旧DLL反事实55.4920/9.3681ms，不能归因环境。20/21/22小输出当前源probe仅2680bytes、12帧/8采样完整：receiver362.2697ms、Production381.1418ms、native参数5.7527ms，嵌套不相加。源和诊断身份独立，23三文件probe按原字节恢复后修复产品。这是本轮施工实现缺陷；首因定位为新receiver的iter().nth逐次投影前缀，24实际原迭代器内计数反例32选中→528行；不是额外inspect人为制造成本。25原InstanceInputs增加边界检查的直接slice get，receiver目标行读取降至32，仍逐项验证ID/geometry/binding/原深度误差界，缺项/越界unknown保留。iter也复用同一行解释，Owned/Borrowed/Retained无第二套字段语义。26最终源是三文件，27最终owner与四integration均按修复后产品重新构建/执行；11b的fixture修复及其四binary复用只作为当时历史依据，不授26产品。GPU中间上涨的独立机制未证明，本次CPU首因不冒称GPU算法修复。

## 最终验证与身份

| 集合 | 通过数 | 进程秒 |
| --- | ---: | ---: |
| owner | 129 | 21.28 |
| visual | 54 | 18.74 |
| u2 | 28 | 8.50 |
| lifetime | 5 | 3.55 |
| geometry | 15 | 6.89 |

最终去重231项，real-wgpu、ignored真实GPU包含、单测试线程。修复窄例只证明故障恢复，不另加计数。三文件fmt、affected lib/四integration scoped Clippy通过；依赖engine_input2、库152、visual2条既有warning照实保留。原flat逐draw/独立像素、当前lease、旧packet/卸载/真实GPU completion以及部分上传重试消费者均在该受影响集合中。

[26源身份](../../../../.artifacts/371/s2-dynamic-20261019/26-source.json)：95输入、三文件变化，原dirty分支codex/255-capability-catalog，不是clean exact-commit资格；全部输入终审无漂移。27 owner和四integration消费26最终生产/测试源；11b仅为历史检查点。[终验](../../../../.artifacts/371/s2-dynamic-20261019/27-test-results.json)保存exe SHA；[比较](../../../../.artifacts/371/s2-dynamic-20261019/31-comparison.json)保存全部身份与计时。

新无probe Release DLL SHA256 108ACD321655077BC3A3186321EA98BA3F872F9103C0C3B2A2ED0739DBBA8999，实际加载身份匹配；module a35dada34c75587ab021bc2d10270514b672281c8515a6b79f126f4659c16a6f及contract/descriptor、CLI、原包和scenario均复用身份未变的有效产物。原交付与安装未替换，源码通过不等于已安装可用。

## 原规模实际反馈

| ms | 09最终 | 本窗最终 |
| --- | ---: | ---: |
| CPU mean/P95/P99，排除present wait | 53.0855/58.9053/58.9053 | 53.3039/61.6049/61.6049 |
| GPU mean/P95/P99 | 7.3042/7.3431/7.3431 | 9.9647/10.7622/10.7622 |
| renderSubmit mean | 51.8163 | 52.0240 |
| update mean | 1.2400 | 1.2510 |

CPU均值观测差0.2184ms（0.4114%），本次未证实整帧收益。一轮12帧、4预热/8样本，不授可重复因果收益或持续P95/P99资格。CPU短窗预算=false，GPU短窗预算=true，formalQualified=false；RHI命令前/后3062/3062，命令数不能代替实际GPU工作或独立像素证明。

保持10000实例/100驻留mesh/128材质/96×2048² BC3 mip/10M triangle/145灯/4cascade+8spot/1080p/far2000/原运动。不是S1的10000 unique mesh配方。CPU/GPU P95≤13.3、P99≤16.67ms、系统可用6GiB保持；本次短窗不补授全程内存资格。测量期间没有并行构建/检索/编辑；Windows Job退出与身份见consumer result。

## 成本与验证经济性复核

| 阶段记录 | 外层秒 |
| --- | ---: |
| 02-build-probe-timing.json | 382.73 |
| 03-feedback-timing.json | 59.78 |
| 06-red-build-timing.json | 206.03 |
| 06-red-timing.json | 1.44 |
| 08-consumer-build-timing.json | 985.14 |
| 11-consumer-build-timing.json | 621.76 |
| 11b-owner-build-timing.json | 97.67 |
| 11b-narrow-timing.json | 1.48 |
| 12-clippy-timing.json | 47.04 |
| 13-native-build-timing.json | 423.35 |
| 14-feedback-timing.json | 59.50 |
| 19-old-feedback-timing.json | 36.89 |
| 21-build-probe-timing.json | 378.20 |
| 22-feedback-timing.json | 41.39 |
| 24-red-build-timing.json | 130.68 |
| 25-green-build-timing.json | 165.36 |
| 25-green-timing.json | 1.43 |
| 27-consumer-build-timing.json | 187.51 |
| 28-clippy-timing.json | 65.85 |
| 29-native-build-timing.json | 402.74 |
| 30-feedback-timing.json | 42.67 |

每个测试进程单列见上表；child Player时间见30-k1/result及engine-player，不把外层编排余量称Compiler。本窗没有Compiler/Cook、BM03、全workspace或正式三轮。红绿/检查点/最终构建均保留，不把旧DLL当新源资格。

08合并consumer准备16分25秒后仍完善现有source索引，11再次付出约10分21秒构建成本；此顺序不经济，已经保留并纠正为产品实现收敛后终验。11 fixture修复只补lib-test构建与必要复验，复用四integration。编译/链接是准备成本；内存低点及分页观测不证明构建慢或产品帧超预算的因果，不更改宿主/应用或以环境免责。

## 整块出口、剩余与终审

当前已闭合：本窗原输入/receiver/native参数因果修复、完整值与失败反例、真实消费者、一次无probe原规模反馈及源码/测试/native/module/包身份分离。CPU仍显著超过原预算，整条执行链性能缺口未闭合。 不把这次局部实现称整块、S2或S1完成。

下一继续同一§0.25：依据剩余动态Production多视图选择、finish完整消费和native slot/index执行成本收敛CPU链；本窗未给最终源重做probe，不以旧分段断言最终热点或再增缓存。若当前既有操作计数不能区分残余首因，再选一次有严格输出预算的有界诊断，然后回原owner最小反例；不重跑Compiler。只有新命令粒度/资源owner/跨帧事实边界变化才另审设计，当前已批准合同不重复申请。

完整K1持续窗口和其它lit实际配方消费者、S3–S5功能及371-G延期资格仍分别缺失；Android延期。S1功能链、安全准入、完整性能与安装状态分开报告。49/54、总施工§0.25、实际合同、阶段总览与双README同步；54 B行及总施工§0.24以下保持字节不变。无需要清理的自有临时测试目录；证据根/共享Cargo target保留，无绕过清理策略。

实际收尾UTC 2026-10-19T02:33:32.463Z，自00:54:31起99.02分钟。终审[33-final-audit](../../../../.artifacts/371/s2-dynamic-20261019/33-final-audit.json)：95输入无漂移、fixture生产前缀/54 B/总施工受保护历史保持、二进制实际身份与Job释放一致、无遗留构建/Player进程。

### 2026-10-19证据来源更正（不改变本记录原运行结果）

后续08:08:37 UTC窗口误用同名证据根，部分早期probe文件/日志历史身份不可信。旧04现从保留的03-current原始输出重解析，原11帧前缀/7诊断样本限制保持；旧26源、27终验、29 DLL、30反馈、31比较和33审计未改。原件缺失/身份不可信列表见[来源通知](../../../../.artifacts/371/s2-dynamic-20261019/EVIDENCE-COLLISION-NOTICE.json)，不得将重名材料当原历史证据；不声称列表内每名称都已证明曾存在。见[14记录](14-当前动态工作收敛与原规模反馈.md)，没有重跑原Player或改写本记录性能数。
