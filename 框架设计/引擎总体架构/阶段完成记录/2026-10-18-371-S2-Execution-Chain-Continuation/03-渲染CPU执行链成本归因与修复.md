# 371 S2-D 渲染CPU执行链成本归因与修复（同§0.25）

日期：2026-10-18。状态：partial / source_consumer_verified / scale_budget_failed。本窗有界归因、内部修复和反馈已结束；整条执行链预算、S2-D/S2/S1未完成，formalQualified=false。

## 范围、基线和预算

用户“按照你的建议进行下一步”授权继续[整链方案§15/15.1](../../371-S2-整条渲染执行链整改方案.md)；沿上一窗02的93输入复核无漂移，不重开方案B。18:15北京时间开窗、21:00停止新增/21:15硬截止，本窗在上限前收尾；G:/gameEngin原dirty分支codex/255-capability-catalog、F盘target、--locked/-j1，无代理，B未激活。Windows only，不动安装、宿主配置、SDK、Physics；Android和371-G完整资格继续延期。

CPU/GPU各P95≤13.3ms、P99≤16.67ms保持。pc-rtx3070-v1/i5-12490F、RTX3070、1080p是60FPS桌面目标；没有同机同场景跨引擎跑分，不能承诺任意引擎必过重负载。原K1仍为10000实例、100 resident mesh、128材质、96张2048² BC3 mip纹理、10M三角形、145灯、4cascade+8shadowspot、10%mask/5%blend、far2000及原运动轨迹；不是S1的10000 unique-mesh配方。不降预算、规模、质量或系统6GiB门槛。

## 当前源码归因与上轮不利数据

先列四个可证伪假设：Production重复稳定/多view工作，Graph/native重复selected消费，mask重排GPU成本，短窗/环境差异。只建一次诊断DLL，同一DLL分别current/legacy mask各跑一次原包短窗。阶段边界聚合，无逐实例时钟/打印；完整stderr未截断，实际Job释放。旧探针7文件按原字节恢复后才修改实现，恢复证据05-restored.json；开窗93输入和探针差量绑定00/01，诊断native绑定02，完整输出及分析03/04。临时legacy环境开关已删除，不是产品能力。

| 当前探针 ms | 结果 | 包含关系 |
| --- | ---: | --- |
| CPU mean（排除present wait） | 103.9601 | 探针开发归因，不是无probe资格 |
| Production build | 68.7931 | 主成本 |
| Graph renderer | 7.9784 | 与Production/native阶段区分 |
| native准备 | 16.9055 | 含参数验证/资源/参数/捕获等 |
| finish排序/分批/校验 | 16.6959 / 4.1628 / 11.2779 | 嵌套在Production，不能相加到整帧 |
| lighting/shadow组 | 40.9427 | 嵌套在Production，含shadow views |

10000实例表和ID映射从第2帧起均同Arc复用，已有共享表已工作；不能再以“表没有复用”解释全量成本。native每个测量帧11个执行视图、约3043个真实draw/29653个selected，6个recipe匹配；已有bundle/instancing也非缺失。当前剩余按view/selected执行的构造、纯验证、参数等仍真实存在，但该旧源探针时间不能授最终新源。

同DLL current/legacy GPU mean 7.3801/7.2102ms，差0.1699ms；CPU排除present mean 103.9601/121.1825ms，draw约3043/4889。仅各一次8样本，不是统计置信或单pass归因。不能据批数证明GPU收益，也不能将上窗GPU+2.2810ms全部认定为mask：这次未复现那个量级，未采每pass/overdraw/频率，剩余原因未确定。上窗renderSubmit121.9336ms在当前探针为101.9230ms，也显示跨窗短测不能当受控单变量因果。本次保留不利/不同数据，不靠反复运行挑绿。

## 最小反例、实现及安全边界

| 真实重复工作 | 首红 | 本次实现 |
| --- | --- | --- |
| 同view共享资源状态仍逐可见项求键 | 32个实例求32次，预期2次 | CompactView::finish调用内按完整不可变资源身份及全部被消费实例执行状态复用原完整键；原IdentityMemo仍为键来源。固定view/template、表借用期间防地址回收；没有跨帧裸地址cache。原phase/pipeline/texture/material/batch/depth/id顺序逐项对照，uniform值不入原排序键 |
| opaque shadow按未消费颜色材质区分派生 | 128个变体求128次，预期1次 | 沿既有position-only语义，候选为geometry Arc、typed handle/generation、first/index范围及cull/mirror/depth。仅派生排序不消费颜色；mask/color保留完整资源身份。排序键不充当合批/驻留证明，逐tail材质binding/UV及几何来源/范围兼容仍验 |
| 挂接灯光后重复扫完整selection | 2批再扫2次，预期0次 | 原proof私有refresh操作仅在旧完整selection已成功、schema/table/visible不变、head.instance/view同Arc且depth/cameraDepth相同才转移成功事实；无变化draw不触碰batch。新的head仍完整验证，变更view/instance/depth/深度值、COW尾错、旧失败一律miss/拒绝；Weak/thread-local/65536容量保持，不pin旧batch/lease，不替代实时generation/上传/退休 |

首红06-red.log、08-shadow-red.log真实保留；最终owner/消费者覆盖三个首红、共享资源的cull/mirror/opaque/mask/StandardLit/uniform变体对照、legacy/nonwriting/blend屏障、COW尾错、非法新head及真实completion。没有新增schema、命令类型、资源owner或GPU算法/shader/参数布局。

首轮合并owner为102通过/1失败；新寿命断言把测试自己持有的unchanged clone漏算，实测strong_count=3而断言2。修复为先drop测试自身clone再断言2，没有放宽断言或改引擎。原失败保留10-owner-first.log/10-initial-results.json；仅补建lib测试，四个不含cfg(test)的integration二进制复用，生产prefix与测试变化身份见10b-test-only-repair.json。最终完整owner103通过，随后直接consumer通过。构建时一次测试枚举遇链接文件锁，无测试判决，未据此改产品或重跑构建。

## 最终验证和身份

205项：owner 103、visual 54、u2 28、lifetime 5、geometry 15。2文件rustfmt与限定lib/四类直接consumer Clippy通过；既有engine152/visual2警告保留，无新增warning。SDK/原项目模块/生成器输入未改，复用上一窗有效证据，不重复编译模块、Cook、完整Compiler、BM03或全仓测试。完整命令、EXE身份和日志在[10测试证据](../../../../.artifacts/371/s2-render-cpu-20261018/10-test-results.json)。

冻结[12源码](../../../../.artifacts/371/s2-render-cpu-20261018/12-source.json)：93输入，只有commands.rs/proof.rs变化。无probe native SHA256 7CA0658E205897B9ED4B36ECD54CD2FCF3C7C9171122713CEADE66CE6BF7B680；未变module SHA256 a35dada34c75587ab021bc2d10270514b672281c8515a6b79f126f4659c16a6f，contract 26b33ef3d894811fcb147a0f0cb6d9d2712bf804bba90c9a81ad3a80eeb55ac0，源码descriptor sha256:aa479a82edd1739cb6d9e4319604cd43c85721e73b6a087c9834dd30a81b696a。同原package/player/scenario digest，Windows Job完整释放；native/module显式override是开发组合，不是原交付或安装已更新。未修改包/安装/宿主，不关其它应用；性能运行期间无并行构建、检索或编辑。

## 合并无探针原规模反馈

12帧，4预热/8采样，仅开发反馈，未执行7200帧/正式三轮。

| ms | 上窗无probe | 本窗无probe |
| --- | ---: | ---: |
| CPU mean / P95 / P99（排除present wait） | 124.1995 / 136.2205 / 136.2205 | 110.4981 / 122.3367 / 122.3367 |
| renderSubmit mean | 121.9335 | 107.8937 |
| update mean | 2.2351 | 2.5731 |
| GPU mean / P95 / P99 | 11.7770 / 12.2880 / 12.2880 | 10.9507 / 11.4534 / 11.4534 |

合并CPU mean变化-13.7014ms、减少11.0317%；renderSubmit变化-14.0399ms、GPU变化-0.8264ms。这些是跨窗组合观测，不拆成各修复绝对收益或稳定百分比；命令数3062→3062不作性能替代。短窗CPU预算=false，GPU预算=true；formalQualified=false。上窗差异与当前probe/最终无probe全部保留，不能只挑最优值。详见[15比较](../../../../.artifacts/371/s2-render-cpu-20261018/15-comparison.json)。

## 成本、剩余与收尾审核

诊断Release 380.87s、current/legacy外层41.36/38.86s；首红构建176.21s、首次修复构建158.51s、阴影首红构建95.06s、直接consumer构建414.52s、测试断言补建82.29s、Clippy 48.90s、最终native Release 392.56s。反馈外层43.70s、child 17.70s、父准备/编排余量26.00s；余量不是纯Compiler。这些分项不代替完整墙钟。原module及资产准备复用，没有把构建耗时算进Player帧预算。

下一仍在§0.25集中处理Production的多视图输入与选择准备：按已保留成本账和原owner最小反例，减少稳定输入重建与重复纯派生，保留真实view变化、裁剪/排序、完整selection及实时准入。旧probe只用于定位，不冒称最终新源分段；只有确需新架构边界才另审合同，不重开方案B。

现证据说明实现工作粒度仍有缺口，未要求推翻方案B，也未证明最终Production已整体O(dirty)。后续不能重复建设已有效共享表、instancing、bundle、pure proof或放弃完整选择/实时准入；新源时间须绑定新身份。CPU超预算时不启动昂贵正式三轮。最终持续运行与实际S2配方、完整memory census/系统6GiB、OS显示、Android等继续按371-G单列；本次调用内排序map/Arc backing尚未接入精确census，不能称平台不可观测或内存已完整。

收尾自审：本次原owner内部实现与§15/15.1一致；不降预算/规模/质量，不改外部公开协议和生命周期边界；失败记录保留、受影响验证通过、探针撤回、93输入无漂移。同步49/54、总施工§0.25、整链方案、批合同实现注记及两个README；B行和总施工§0.24以下历史按字节保留，371仍在当前，不归档；局部通过不宣称整块/S2/S1完成。
