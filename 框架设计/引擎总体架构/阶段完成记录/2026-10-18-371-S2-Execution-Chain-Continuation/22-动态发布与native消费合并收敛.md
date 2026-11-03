# 371 §0.26 动态发布与native消费合并收敛

2026-10-20。本记录覆盖用户“剩下所有的整改方案全部一起施工直到完成”的同一合并窗口，未拆成新的小块完成。当前状态：partial / source_consumer_verified / scale_budget_failed；formalQualified=false。源码开发结果不等于安装可用或整个S2/S1/371完成。

## 范围、身份与三个出口

本窗08:49:49 UTC开始；11:34:49停止新增实施/验证，11:49:49硬截止。证据根：`.artifacts/371/s2-convergence-20261020-084949`。前窗09源99输入逐SHA复核匹配；原dirty分支codex/255-capability-catalog、F盘独占target及--locked -j1复用，不覆盖其它路修改。35源99输入/15变更文件、39无probe Release DLL、40原规模反馈绑定；具体hash及外部状态审核见42-final-audit.json。不是clean-commit或安装发布资格。

| 原整块出口 | 已实际贯通 | 未闭合 |
| --- | --- | --- |
| 常驻命令与动态参数 | 继续消费已有稳定模板/表/slot；最终附件一次发布；native参数与当前资源实际消费者接通 | 仍有薄动态head和逐帧plan/工作集整理，不能宣称全部旧描述路径退役或CPU预算闭合 |
| 可见性与阶段并行 | 冻结主颜色连续候选≤4分片；一次全局原排序后按原批次边界≤4发布区间；与≤12shadow共≤16任务/≤4执行线程；前置组先join | 仍有排序、规划、共享命令owner串行区间；最终无probe没有完整分段，不能以旧probe归因当前余量 |
| 生产接管/退役及反馈 | 生产不再先发布临时batch再重写head；旧重写路径仅test oracle。真实GPU/视觉/生命周期直接消费者通过，原规模无probe反馈完成 | 原CPU预算仍失败；S2其它实际lit配方及371-G延期资格未补齐，不把短窗当正式出口 |

## 合并实现及安全边界

1. 原Production使用私有PendingGeometryView，选择、接收者、灯光和阴影依赖齐备才一次发布。先以原上下文验证完整tail，安装最终附件和稳定命令，验证最终head后发布绑定完整不可变输入的成功事实；不改变instance/view/depth、透明屏障、全局排序或尾部。
2. receiver直接读取确切PreparedInputs/table/row；未知保守回退。caster消费冻结列并保持原f32运算和顺序。大颜色输入按原索引连续分片并确定性合并；最终批次区间不跨原批次切割，共享selection backing，未复制完整候选。
3. 暖native arena仅将CPU记录临时移入validation分支，当前参数/跨batch唯一性通过后整理payload，与纯资源扫描重叠；所有子任务join，错误/panic均恢复同一arena。GPU写入、content提交、部分上传和真实完成退休仍原owner。超限view入口在提前CPU stage前严格回退原错误路径。
4. 原native模板可保存确切Weak binding/pipeline的本backend CPU registry记录成功，不证明GPU有效。COW/格式/模板变化回严格记录；lit纹理需求先借用去重，仅唯一需求复制环境identity。
5. 参数身份校验单调用内以确切table行形成ordinal，跨view仅首次进行实际ID哈希；不同table/flat/detached仍共享全调用ID域并逐view检查重复。table scratch总≤65536行/256KiB；≤16view启用位图，最大128KiB，更多view沿旧HashSet路径。未选中超大table不扫描，额度不足严格回退，不拒绝合法输入。不是跨帧proof或GPU事实。

保留generation/取消、不可变packet、完整tail/跨batch唯一性、当前registry/admission、容量/队列/上传上限、重试、旧在途帧和真实GPU完成点。无新公共schema、资源owner、宿主或安装变更。snapshot/renderer现有backing继续观察，PendingSelection、分区Arc及worker/ordinal scratch、哈希桶/Weak控制块仍明确partial；没有以reservation或进程总量冒充owner分配。

## 当前成本证据与收益

第一次01诊断属于00源，13文件字节恢复。第一轮24源/18无probe反馈CPU18.2438/P9520.3920ms、GPU8.427264/P959.23136ms，CPU均值较前窗下降19.8872%，仍失败。随后30诊断只属于24源：Production墙钟5.7294、Graph1.9461、nativeExecute2.5056ms；nativePrepare累计6.6280ms含执行重入，不与execute相加。finalJobGroup墙钟3.84275ms；worker参数验证1.4629/整理1.4338ms是并行累计，不与父阶段相加或相减。当前实际每帧110000候选、约29657.125选中行、9897.25独立实例、11 view variants；新native模板创建0，证明旧模板已生效。完整profile3858bytes无截断；11文件已按字节恢复、99输入一致。

该24源诊断和真实红例确认第二轮要消除的是颜色单发布任务和同一行跨view重复ID哈希；不把全部父阶段时长当可消除收益。最终35源只取得下面的无probe整体反馈，不能借30旧分段冒称最终源成本分解。

| 同原K1、12帧/4预热/8样本 | 修改前 | 本窗第一轮 | 最终候选 |
| --- | ---: | ---: | ---: |
| CPU均值ms | 22.7726 | 18.2438 | 17.5068 |
| CPU观察P95/P99 ms | 26.3643 | 20.3920 | 18.5521 |
| GPU均值ms | 7.8212 | 8.4273 | 7.8468 |
| GPU观察P95/P99 ms | 8.6108 | 9.2314 | 8.4951 |

最终renderSubmit均值16.2352ms、update 1.2456ms。相对窗口前CPU均值变化-5.2659ms（下降23.1237%），观察P95距13.3ms为5.2521ms。仅8样本，不能证明持续P95/P99、正式资格或稳定改善；不重复同源运行挑绿。原包/场景/Player digest全部一致，真实module复用。启动前物理可用5.6985GiB，仅单点；6GiB门槛保持，未作全程引擎/宿主/环境分解，不关闭其它应用、不扣工作集伪造达标。

## 验证、首红与返工

最终身份新执行：36动态组14项、并行参考1项、剩余render_geometry 103项、剩余native geometry 101项，去重219；37视觉54、U2 28、真实生命周期5、GPU15，合计102；最终候选共321。不是复用339项冒称重跑。38 scoped Clippy/default check/fmt通过；库152/visual2基线Clippy告警保持，没有新告警。最终test binary hash与各命令/输出见36/37结果。

- 02首红证明临时head进入生产，4/32/256真实计数；31首红证明4实例两view哈希8次，以及大颜色仍单发布任务。修复后独立旧路径/旧checker、多个规模、17view回退、超大未选中table、duplicate、透明/阴影/旧packet对照通过。
- 05编译失败是planning函数可见性及捕获含裸指针的非Sync准备结构；修复为冻结可共享借用，未使用unsafe强制Send/Sync。06/10/16/16c还发生旧fixture参数、变量名和OrderedF32类型同步错误；没有把编译失败算作运行失败。16b发现提前参数stage容量前提需复核后终止确切owned构建，代价仍计入。
- 08新增发布回归在实际重写数已为0时仍保留旧预期，改为合同要求的0重写并保留最终environment/light/shadow、变化与旧packet断言；11材质工作计数尚挂在旧纹理key路径而得到0，计数移至真实borrowed key计算点，继续要求共享key只解释1次。两项均保留原失败日志，不以代理flag代替底层工作。
- 21终验217通过/1实际回归：改变BeginFrame格式时错误复用CPU intent记录成功；原backend入口要求归一化前已为Rgba8Unorm，22修复构建后23全218通过。35最终候选对应风险再次包含于36终验。其它预期panic反例不算未修产品失败。
- 24/18第一轮320项与无probe改善保留为中间身份；之后产品改变，不外推其最终分段或全部consumer身份。最终35/39/40重新合并验证一次。

## 构建、准备与运行成本

逐项实测见29-costs.json，包括失败和取消，未只列最后一次。类别外层累计（秒）：

- build-and-red-test-combined: 171.1762
- quality: 293.5440
- build: 3571.7497
- test-execution: 266.9951
- feedback-outer: 89.0403
- probe-build: 757.6523
- probe-feedback-outer: 75.2071

probe与无probe Release、owner/consumer构建和Clippy/default/fmt各自分列；feedback outer包含准备/子Player，不与child及profile嵌套span重复加总。最终39构建与40反馈独立计时；未调用Compiler/Cook、BM03、workspace或正式三轮。两次probe的重复准备及数次编译返工是本窗实际施工成本，不能都归因于运行性能。长测期间只有等待，无并行编辑/检索/构建。

## 未闭合结果与下一继续点

三个原出口保持partial。已确认本窗产品修复和受影响消费者贯通；原CPU预算仍失败，不能宣称剩余方案已全部施工完成。继续同一§0.26，按最终身份的真实消费者和主要剩余CPU工作量收敛动态head/Graph-plan与native参数/工作集准备；已生效模板不再重建，不将必要裁剪/排序/当前准入一概删除。下一次需要新成本证据时，不能复用24源分段作为35源归因，不预先认定mutex/哈希或某一validator是根因，也不为每个小候选重建全链。

源码复核进一步区分必要动态工作与未证实冗余：当前有界准备仍是同帧`spawn_scoped`，不是常驻worker pool；任务数/线程数通过不等于调度成本已闭合。当前动态head及Graph/native的逐帧整理也仍真实存在，但不能据“存在”就认定全部应删除。其在35源中各占多少、是否构成主要剩余成本尚未测定，不把24源分段外推，也不凭经验立即新增线程池或资源缓存。

下一合并候选的固定验证成本已有实测：owner构建210.93s、直接consumer构建261.04s、质量约59.11s、无probe DLL392.14s、反馈43.64s，约16.1分钟，尚未含实现、测试执行和恢复。若还需要当前源诊断构建约6.5分钟及20–25分钟合并实现，再保留收尾余量，已超过本窗剩余时间。按施工skill“不启动无法在剩余窗口内完成并保留证据/收尾的阶段”停止新增高成本候选，保留同一整块继续点；不是把时间边界当作CPU超预算的技术原因，也不是本轮测试仍挂住。未追加第三次完整probe、同源挑绿或新的一套全量资格。

完整内存观测、OS显示及正式三轮仍按371-G延期；实际lit配方缺口独立保留，Android延期。没有修改安装、宿主、SDK、Physics、V0.0.6或游戏V0.0.5；未清理共享证据/缓存/项目，不存在新的cleanup policy拒绝。进程释放、恢复与闭窗时间以44-closeout.json为准；施工文档不归档。
