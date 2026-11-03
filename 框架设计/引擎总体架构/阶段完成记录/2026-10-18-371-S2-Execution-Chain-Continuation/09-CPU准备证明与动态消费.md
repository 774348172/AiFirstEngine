# 371 S2-D CPU准备证明与动态消费（同§0.25）

日期2026-10-19北京时间。用户明确选A，先审改[正式合同](../../371-S2-CPU准备证明与动态消费合同.md)及自审，再沿原总施工实施。状态partial / source_consumer_verified / scale_budget_failed；源码链路和直接消费者已验证，整链预算、S2-D/S2/S1/371未完成，未更新安装。

## 实际实施与安全边界

原PreparedInputs的完整不可变table产生PreparedPhase：完整表逐行证明数值、binding类型/结构、range、normal/UV，独立兼容类由原完整compatible_instance判等，排序rank不能当兼容证明。全表成功才发布；隐藏非法行回退原selected-only严格路径，不发布部分成功或引入不可见行错误。table Weak身份、完整phase上下文、COW均参与失效。真实成员Vec/Arc接snapshot观测，静态proof Weak/map backing仍明确尚未接入，未声称完整memory census。

每个新selection仍逐项验证schema、容量、head/首项一致、完整索引/重复/current VP×world和深度有限；保留当前动态裁剪、排序、透明/非写深度屏障。完整成功batch沿原proof owner传至Graph/RHI/native，公开value/wire入口保持严格。共享head可省已由全表证明的行静态解释；灯光/阴影/环境/附件/域/rigid view/MVP和当前资源准入另验。proof有界Weak、失败不缓存、无强资源保活。Player当前使用inline dispatcher，未确认跨线程proof丢失，不引入全局缓存；其它线程严格fallback，未授其性能。

原RetainedParameters增加有界InstanceValues，只复用不可变DrawInstance分配身份+环境强度bits对应的纯32float参数。Weak隔离COW/地址回收，至多65536项，overflow只miss/清CPU缓存。view仍使用当前VP、灯光/preview与颜色配置。原完整RecordKey(id+全部bits)、实际content/dirty、部分上传重试、resident、staging/在途/退休额度和真实GPU完成保持；CPU命中不能把待上传资源变clean。Vec实际存储已观测，index hash buckets/Weak backing与调用内动态集合backing明确尚未接入，不归为平台不可观测。

同窗有界诊断后先补一处native实现：完整plan成员集合由BTreeSet改调用内HashSet，无排序消费者，仍逐行验证(view_id, instance_id)、跨batch重复、原容量和device limit。没有跨帧缓存可见结果、GPU-driven、第二资源owner或wire/SDK/shader布局变化。

收尾40原owner红例另确认GPU内容正确性缺陷：满槽同ID从+0替换为-0，dirty预期1、实际0。原RecordKey按bits区分，而content用f32相等漏上传；现dirty也按全部float bits判等，测试覆盖正反替换、未上传重试及真正更新content后clean。未改GPU布局、上传额度或真实完成点。35检查点DLL已构建但未运行Player，最终使用43源/44 DLL。

## 首红、失败与最终验证

01首红为稳定兼容比较实际2而期望0；02b-native首红为纯参数派生实际4而期望0；06-head首红证明下层binding仍实际4而期望0。修复后相机移动重复兼容2→0、下层binding4→0、native纯派生4→0，且CPU复用后未上传参数仍dirty。原flat逐draw、独立GPU像素和旧packet/真实completion消费者保持。失败日志和各源码检查点保留，不能将计数当作帧预算。

02命令误传两个Cargo位置filter，未运行测试；26a新增反例误用不存在的RenderDomain::Screen导致编译错误，停止本路Cargo/rustc树后改为已有World2d，并回owner验证。两者为施工调用/fixture错误，不是运行时或Compiler产品故障。无归因环境的证据，不作环境免责。

早期227项、26与34检查点各228项及最终41 suite不相加。26之后两个u32非零除数3的余数谓词仅改为等价is_multiple_of，27-lint-equivalence保留当时复用依据；最终41已重新执行修正后实际源码，最终229项直接绑定43源，不依赖等价豁免。

| 最终集 | 项数 | 进程秒 |
| --- | ---: | ---: |
| owner | 127 | 16.32 |
| visual | 54 | 17.92 |
| u2 | 28 | 8.43 |
| lifetime | 5 | 4.11 |
| geometry | 15 | 7.64 |

real-wgpu/ignored GPU包含、单测试线程；六文件fmt与受影响lib/四integration scoped Clippy通过，既有库152/visual2条warning保留。覆盖完整tail/COW、hidden非法回退、phase/深度、镜像/UV、同ID异值/环境强度、overflow/expiry/Weak不pin、当前lease、GPU部分重试/旧帧/真实完成；具体名称及输出见[最终验证](../../../../.artifacts/371/s2-cpu-proof-20261018/41-test-results.json)。没有改oracle、规模或预算。

## 原规模无探针反馈及身份

43源为原dirty工作区声明身份，不是clean exact-commit或安装资格；95输入、六文件变化：render_geometry.rs、commands.rs、proof.rs、production.rs、native geometry.rs、parameters.rs。新无probe DLL SHA256 567B1C5AF8C09F4FC4893382DCDF79D6F046319BFE33BA7C2EEC9790BB35D526，实际consumer匹配；module a35dada34c75587ab021bc2d10270514b672281c8515a6b79f126f4659c16a6f及contract/descriptor、CLI/原包/场景复用。package/player/scenario身份逐项相同，原交付和安装未替换。详见[冻结](../../../../.artifacts/371/s2-cpu-proof-20261018/43-source.json)、[实测比较](../../../../.artifacts/371/s2-cpu-proof-20261018/46-comparison.json)。

| ms | 前窗08 | 本窗最终无probe |
| --- | ---: | ---: |
| CPU mean/P95/P99，排除present wait | 75.3769/85.3979/85.3979 | 53.0855/58.9053/58.9053 |
| GPU mean/P95/P99 | 11.8438/12.4109/12.4109 | 7.3042/7.3431/7.3431 |
| renderSubmit mean | 73.9123 | 51.8163 |
| update mean | 1.4280 | 1.2400 |

CPU均值差-22.2914ms，观测降幅29.5733%；这是合并修改的一次12帧（4预热/8样本）短反馈，不拆分归因到单项、不授稳定收益或持续P95/P99资格。CPU预算=false，GPU预算=true，formalQualified=false。RHI commands=3062，数量不证明GPU实际工作等价。GPU历史上涨首因仍unknown；本次短窗回落也不能倒推环境或宣称问题已修复。

保持10000实例/100驻留mesh/128材质/96×2048² BC3 mip/10M triangle/145灯/4cascade+8spot/1080p/far2000及原运动，不等同S1的10000 unique mesh配方。CPU/GPU P95≤13.3ms/P99≤16.67ms、系统可用6GiB未改，不关闭其它应用、不从物理可用量扣工作集。测试期间无并行构建/检索/编辑，Windows Job完整释放。

## 诊断口径、剩余与下一继续点

24诊断只授28检查点源码，尚无最终HashSet修改；日志未截断、8采样完整、七文件probe逐字节恢复，恢复后才构建最终DLL。粗分段Production 23.9181ms、Graph 5.6303ms、native几何准备 19.4357ms；Production内部选择循环4.9067、finish排序/分批/校验2.2257/1.2406/3.3460ms，receiver 4.6491ms、shadow views 7.9599ms。native完整plan容量/重复检查4.3986ms、参数整体7.9233ms，其中input/CPU stage/admission/upload 0.2916/7.4385/0.0351/0.0863ms。嵌套不相加；未测编码/table细节记未测，不填0。见[同窗诊断](../../../../.artifacts/371/s2-cpu-proof-20261018/24-diagnostic/04-profile.json)。

稳定表/兼容类/静态head与纯参数派生已生效，不能再将它们写成缺失功能。本块剩余主要是每帧动态多视图selection/receiver/当前灯影与native参数slot/index消费成本；仅纯32float生成复用不等于整个参数阶段已常驻。下一继续同一§0.25，按现有分段和完整键/dirty/current view合同收敛真实动态工作量；无新首因不重复建缓存或重跑Compiler。若需改变空间索引、命令粒度、GPU-driven、资源owner或公开协议，另列新增边界，不重复申请A已批准范围。当前结果不足以宣称全链O(dirty)。

完整K1持续窗、其它lit配方实际consumer与S2-D出口仍缺；S3动画/蒙皮/Physics/Nav/空间音频、S4世界/LOD/环境、S5普通项目/工具/交付仍有实际能力缺口。完整内存观测/系统6GiB归因/正式三轮/OS显示/Android按371-G延期，不能把所有未完成归为测试。

## 成本与收尾

40 signed-zero owner首红构建与运行153.68s；已完成但未运行Player的35检查点Release构建378.34s。最终consumer构建274.26s、测试进程合计54.42s、Clippy 24.42s；24诊断构建379.42s、反馈外层71.88s；最终无probe Release 376.64s。最终反馈外层44.08s、child（含启动/加载及12帧）15.99s、父准备/编排余量28.09s，余量不称纯Compiler。各早期构建/首红/取消及检查点成本见46比较的costs和原始timing，未隐藏失败。

项目Compiler/Cook各0调用；Cargo/Rust构建按上述成本另列。无BM03、workspace或正式三轮。复用原包/module/CLI和未变证据；原F盘target/-j1，不修改安装或宿主。15:34:47 UTC开窗，18:19:47停止新增/18:34:47硬截止；本次有界范围完成收尾而非达到硬截止，最终验证单独记录实际结束时间。同步本合同/整链方案/总施工§0.25/阶段/49/54及双README；B行和§0.24以下历史字节保持，总施工不归档。

终审reader首轮误把Cargo测试失败码写为1，而40首红原收据为101；修正reader为真实101并保留断言检查，未改原失败收据、产品源码或二进制，未重跑测试/性能。见49-verifier-correction.json与最终48-verification.json。

实际收尾：2026-10-18T16:58:56.2613430+00:00；本授权窗口约84分09秒，未触达时间上限，施工所属进程无残留。源码/消费者验证通过、CPU预算仍失败，见[收尾收据](../../../../.artifacts/371/s2-cpu-proof-20261018/50-closeout.json)。
