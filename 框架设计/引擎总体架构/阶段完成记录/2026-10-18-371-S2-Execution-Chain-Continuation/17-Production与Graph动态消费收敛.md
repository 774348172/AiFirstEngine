# 371 S2-D：Production与Graph动态消费收敛

状态：同§0.25整条执行链续工，partial / source_consumer_verified / scale_budget_failed，formalQualified=false。本窗2026-10-19 15:32:33 UTC开始，18:17:33停止新增实施/验证，18:32:33硬截止。用户授权性能失败后自主归因、修复，本窗已连续推进；没有把各次局部优化另称完成大块。

证据根：`.artifacts/371/s2-production-20261019-153233`。最终40-source的96项输入、43无probe DLL、44原包反馈，47-evidence-review汇总验证与计时。dirty开发身份不授clean commit/安装/正式资格。Windows only；原预算、规模、运动、画质、6GiB保持。Android/371-G延期，未扩入B槽373、Physics、SDK、安装、宿主配置或V0.0.5冻结游戏。

## 最终实际修改

- Graph调用内借用并去重ShadowMaps，仅最终唯一atlas声明分配资源名；不同映射保留完整集合，不缓存跨帧图有效性。
- receiver一次检查当前camera有限性；静态corner按SoA求当前深度，保持原f32乘加、负零初值和向外guard。仅prepared table/index精确关联才直取；detached/COW、来源不一致及极值仍严格回退。
- CompactView消费PhaseCache已有完整静态成功，免逐selected行重复查询instance成功事实；新表/COW/未完成表仍原检查。DrawViewValidation仅在本调用相同不可变view及world-magnitude bits下复用有限界。
- native消费准备扫描生成的当前完整required_textures；纯几何且每个handle实时通过GPU registry才免描述重扫。mixed或缺失handle走原有序角色/首错入口，没有跨帧驻留或上传证明。
- surface readiness消费同一准备owner；任何准备错误和容量等待均先补原有序纹理准入，保留缺失纹理优先的错误顺序。纯结构分类只是提取原谓词。
- intent仅在当前完整commands成功及实际Rgba8Unorm格式下消费Preparation的CPU pipeline字符串，完整记录draw/binding/资源/pipeline报告；不提前设置成功编码标志或允许replay。其它情况保持原路径；default共用完整metadata记录。

最终相对激活身份9文件：render_geometry.rs、render_geometry/{graph,commands,workset,build_inputs,proof}.rs、render_geometry/shadow/planning.rs、wgpu_backend.rs、wgpu_backend/real/geometry.rs。公共schema、命令、GPU成功证明、资源owner均未新增。完整tail、当前selection/相机/透明/阴影、跨batch唯一性、source冲突、device limits、generation、部分上传重试、队列上限、旧在途帧及真实completion退休保持。

## 首红、失败及撤回

02实际操作首红后：atlas字符串复制16/128/1024→各1；receiver camera元素访问64/512/4096→各16；instance成功查询4/32/256→各0；同view有限界计算4/32/256→各1。最初夹具E0428重名/E0597生命周期保留02a，修夹具后才取得目标首红。

最初226项通过后，数值复审核查出Rust Iterator::sum以负零起始；原SoA正零初值存在符号位差异。05修原kernel为负零，独立4096组逐位参考覆盖signed zero/subnormal/极值，随后227项通过；早先226项不代表修正身份。

第二组layer惰性匹配、标量排序键、head矩阵界虽实际计数下降并通过233项，24整帧相对14仅低0.56%，26 Production亦未改善。三项及专属字段/测试全部撤回，三个文件逐字节恢复10源，见27-rollback。20/24/26只是历史试验，不列当前能力。

27/38真实native首红后修原owner：prepare及surface descriptor扫描4/32/256→各0，当前registry仍各查询1次；prepared/camera-only intent字符串构造4/32/256→各0。38首次mixed夹具错误使用不存在的main target，提前触发capability错误，38a保留；改用实际draw target后38b取得两条目标首红。资源移除/恢复/generation、mixed首错、COW/context/未成功准备及report等价均覆盖。

其它返工：非唯一脚本anchor写入前拒绝；比较脚本prior路径修正未重跑原包。46 OS探针首次QueryThreadCycleTime导入E0432，保留日志/计时/身份并恢复17文件，再改用已有windows-sys GetThreadTimes为46b，无新依赖/配置/安装。它是诊断构建失败，不是产品故障。

## 同原规模无probe反馈

每次均12帧、4预热、8样本；新产品身份各一次，不授持续P95/P99、正式三轮或资格。不能将合并观察差异分摊为单项因果收益或GPU算法收益。

| 产品 / DLL / 反馈 | CPU mean / P95 ms | GPU mean ms | 结论 |
| --- | ---: | ---: | --- |
| 前窗16 | 38.8951 / 41.6174 | 11.5071 | 本窗基线 |
| 10 / 13 / 14 | 35.1943 / 40.7460 | 7.3190 | 首组保留 |
| 20 / 23 / 24 | 34.9986 / 39.3093 | 7.3330 | 试验撤回 |
| 30 / 33 / 34 | 31.2635 / 34.8430 | 7.3315 | native准备闭包检查点 |
| **40 / 43 / 44 当前** | **30.6505 / 33.6345** | **7.3130** | **CPU仍失败** |

最终GPU P95 7.3421ms。CPU均值比窗前观察低8.2446ms（21.197%）；最后两消费者相对30仅低0.6131ms，当前intent分段也无改善证据，不以字符串计数下降宣布该阶段性能完成。CPU P95仍比13.3ms高20.3345ms；原P99 16.67ms不变。

K1保持10000实例、100 resident mesh、128材质、96张2048² BC3全mip、10M triangles、145灯、4 cascades+8spots及原1080p/far2000/运动。它不是S1的10000 unique-mesh资格，也不补齐其它lit配方consumer。

## 当前源码归因

46b绑定40源：12帧/8样本、9560bytes stderr完整；17文件按字节恢复、96输入无漂移。01/16/26/36分别只授00/10/20/30源，不可冒认当前40源。

| 当前40源阶段 | mean ms | 包含关系 |
| --- | ---: | --- |
| Production | 12.9399 | selection3.0685、sort2.1828、batch1.2417、tail1.7882、receiver1.3668、caster bounds1.2421、directional pose0.5273为子项 |
| Graph | 3.0021 | compile1.8792，其中declarations1.5650 |
| nativePrepare累计 | 8.4310 | capacity1.7175、resources3.5782、parameters3.0191、capture0.0996为子项 |
| nativeExecute | 6.1475 | intent2.0220、encode2.6873，其中recipe capture2.1965 |

nativePrepare累计含nativeExecute内同帧reuse，四大段不互斥，禁止直接相加。stableRequirements计时跨准备/recipe/intent，也不是独立额外成本。renderSubmit未覆盖余量0.6708ms仅为扣除重叠后的下界，不分配给未知owner。

每帧110000候选、29657.125选中、3043.25 batches、18785.125唯一instance参数、11 views。实时纹理闭包查询96、descriptor扫描0；105000 cull输入中严格fallback73.125、空间上界11次/88dot。unprepared selection、完整instance值检查、cache整表clear均0。Preparation 9129.75调用/9128.5命中/1.25解释，pipeline代表5、source闭包100；intent新增命中调用，命中不等于无成本。

GetThreadTimes只读当前render线程，从Production reset至nativeExecute flush，排除update/尾部report；平均kernel5.8594/user21.4844，实际调度CPU27.3438ms、wall30.8597ms。OS记账有粒度，probe有开销；wall−CPU不能精确归因GPU或调度等待，kernel总量也不能分给函数。证据支持超预算主要仍含真实CPU工作，不支持只差GPU等待/正式测试的解释。

参数内部parameterMap1.6375、instanceRecords1.2246ms；recipe capture2.1965，bundle record仅0.2269ms。共享实例表、完整证明、prepared workset、instancing、bundle及稳定解释已生效。当前candidate热路径直接读prepared列；冷fallback的iter().nth不能冒认当前首因。相机变化造成旧command backing过期，不等于稳定Preparation失效。

本窗修复多个消费者重复工作，并撤回无整帧收益证明的试验。剩余动态选择/排序/组批/验证、参数slot及native捕获在单render线程串行消费；其中有必要动态工作，不能全部删除或称为重复完整校验。现有证据不足以判定3D方案须整体推翻，也未证明另一小缓存足以关闭约20ms P95差距。

## 正确性、身份及施工成本

41终验297项去重通过：owner/native195、visual54、U2 28、lifetime5、geometry15。42九文件fmt/scoped Clippy和新执行default库check通过，既有warnings保留。Compiler/Cook、BM03、workspace、正式三轮均0调用。43无probe DLL SHA256：01e2689decd330f559adfaf57c40a47e065a50f505034b5651b912b5d7ec2c49；44实际consumer匹配，未安装。

| 本窗各轮累计外层成本 | 秒 | 边界 |
| --- | ---: | --- |
| correctness builds | 2309.447 | 含首红、试验及终验构建 |
| Release DLL/probe builds | 3340.855 | 含诊断失败与修复重建，不隐藏返工 |
| 混合owner构建+测试 | 144.441 | 合并计时，不冒称独立纯构建 |
| Clippy / default / fmt | 128.676 / 53.898 / 1.166 | 分列 |
| 消费者harness | 259.007 | 各不同身份累计；最终297项报告执行合计52.52s |
| 单独窄例harness | 14.429 | 非构建 |
| 全部Player反馈外层 | 393.449 | child及内部阶段已包含，不再相加 |

最终41构建317.532s、43 DLL构建377.746s、44反馈外层71.424s；其中child Player15.909s，余55.515s含准备/装配/hash/封存/后处理，不能全部叫纯Compiler或启动。逐次计时见47。44脚本及计时scope误写42-frame，实际绑定first12场景/第5–12帧8样本；48保留原标签并作证据元数据更正，不重跑消费者或改样本。

34启动前系统可用约5.698GiB、44约5.977GiB，低6GiB；14/24约6.503/6.488GiB也不能外推全程达标。均为新Player启动前单点，不是全程峰值或引擎/宿主/环境allocation归因。未扣工作集、关闭应用或改配置。backing census仍按371-G逐项partial，不统称平台不可观测。

## 同整块继续点及未闭合出口

实现、必要正确性、当前源归因和恢复已证明；**主要CPU消费路径、预算及本整块未闭合**。其它lit配方consumer与371-G内存/显示/正式资格分别保留，不是当前CPU失败的替代解释；S2-D/S2/S1/371不归档。

下一窗口仍同§0.25，复用40源/43 DLL/297项，联合推进Production选择→排序→组批→tail及native stage_selected→Records→recipe/intent，先计数真实row读取、键查询、集合和引用复制，划清必要动态工作与可共享派生。当前参数多遍处理、Preparation三处命中为具体候选，须多规模实际工作首红后才实施；不能将父段总时长承诺为收益。完整表、全键、当前selection/首错、跨batch唯一性、GPU实时准入和退休继续原owner负责。

构建仍占主要施工成本，不再为每个小候选单独安排全consumer构建、Release DLL和probe。下窗用现有归因确定合并修改集合，窄例收敛后一次受影响终验、一次无probe反馈；失败回最小owner，不同源挑绿/重跑Compiler。若必须引入并行调度或跨线程证明，先核实PhaseCache锁和thread-local成功事实所有权，再形成具体合同讨论；尚未实施，也未证明是必需的新架构。

本窗最终复核：源码96输入及43 DLL/实际consumer匹配、17文件probe恢复、当前记录链接和受影响路径diff检查通过；无本轮遗留Cargo/rustc/Player进程。49-final-audit记录实际耗时及收尾身份。按施工skill“Do not start a stage whose deadline plus evidence and cleanup reserve exceeds the remaining window.”，下一组合并实现/终验构建/Release DLL/反馈与恢复无法保守装入本窗剩余时间，故提前收尾；不是已到三小时，也不将时间边界解释为CPU超预算根因。无需为文档同步重跑297项或原包。
