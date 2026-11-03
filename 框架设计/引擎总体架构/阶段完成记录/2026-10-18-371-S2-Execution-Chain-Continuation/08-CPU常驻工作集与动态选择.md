# 371 S2-D CPU常驻工作集与动态选择（同§0.25）

日期2026-10-18。用户已选②，已先审改[正式合同](../../371-S2-CPU常驻可见性与阶段工作集合同.md)，再沿原总施工实施；不是另开方案B。状态partial / source_consumer_verified / scale_budget_failed；完整预算、S2-D/S2/S1及371未完成，安装未更新。

## 实施结果与生命周期

原ProductionGeometrySnapshot保有完整不可变实例集合Arc、origin位模式、原lease身份对应的PreparedInputs。仅相机或输出变化时共享同一集合与工作集；world/材质/visibility/layers/provenance/package/generation等改变使受影响工作集重建。原InstanceReuse仍复用实际未变行，但不宣称整个准备为O(dirty)。当前manager/generation/lease每次检查，不将CPU命中当作驻留或GPU完成证明。

每snapshot最多16个origin工作集、每工作集最多16个完整phase上下文；超限正常miss，不拒绝合法输入。无全局强缓存、第二World或注册表。旧packet及资源按原真实completion退休；工作集无反向snapshot引用，snapshot释放后不额外pin旧packet。真实Vec/Arc payload/capacity接snapshot观察，hash buckets、Arc control等缺口仍明确partial，未冒称完整内存覆盖。

每origin持久保存world、f64世界AABB及原f32运算量级界；view保存六个齐次平面。只有误差guard足以证明整个原bounds内/外才快判；非仿射、非有限、f32中间overflow风险和边界不确定均回原八角算法。保留原homogeneous容差、camera_depth结合顺序及阴影候选完整性。没有BVH、SIMD、线程池或GPU-driven切换。

phase rank绑定完整不可变表和真实shadow/domain/depth附件/color pipeline/environment/lighting/shadow mask上下文；等价完整键共享rank，保留depth/id及透明、非写深度、旧Unlit mask屏障。每帧visible selection、动态排序、完整尾项和实时准入继续执行，缓存rank不是新frame成功证明。

同块第二次修改将稳定bounds/visibility/convention/blend/depth与精确layer组放入原PreparedInputs的紧凑候选列；动态候选不再逐项追踪冷Prepare/Projection，只在legacy取源数据或最终head引用原完整行。字段值/浮点计算顺序未改；新增列和layer backing已接原memory owner。没有跨帧可见结果缓存或公开schema/SDK/ABI/shader布局变化。

## 首红、最终验证与失败保留

01真实首红：仅相机移动仍发生输入验证/phase解释，实际(1,2)而期望(0,0)。早期误用短名--exact导致0测试不算首红，随后真实1项失败保留。首次实现编译E0282发生于Arc::new(collect())类型不确定，34.2799s；补明确Arc<Vec<PhaseMember>>，不是运行时故障或环境原因。04六项早期通过不与最终suite重复计数。

首期05共222项通过。14热数据首红明确“selection still chases immutable cold Prepare/Projection metadata”，1项失败；候选bounds/visibility改为实际冻结热列地址后，最终16共223项：owner 121、visual 54、u2 28、lifetime 5、geometry 15。仅最终223项计最终覆盖，不能把两轮及早期六项相加。包含real-wgpu/ignored GPU消费者、单测试线程。六文件fmt及受影响lib/四integration Clippy通过，库152/visual2条既有warning保持。

独立旧flat路径逐draw对照覆盖opaque/mask/blend、稀疏/高剔除、相机单变、同ID world/材质/visibility变化、镜像/剪切和origin128；保存旧packet后继续逐draw验证。另有16,384组仿射/透视/正交空间对照、非有限/overflow回退、phase上下文与COW失效、16 origin容量miss及释放/旧frame存续；既有设备失效、receiver/alpha/mip/光影像素和真实GPU完成点消费者保持。没有改oracle放宽预算或拒绝条件。详见[最终测试](../../../../.artifacts/371/s2-cpu-workset-20261018/16-test-results.json)与[最终冻结](../../../../.artifacts/371/s2-cpu-workset-20261018/18-source.json)。

## 真实原规模反馈

95输入/六变化：render_geometry.rs、build_inputs.rs、commands.rs、production.rs、memory.rs、workset.rs。最终native SHA256 DB9E9E0EC86CB98C7E5556E8845E836CA94BBE81CD748F533A4BDDEDC4EAF054，实际consumer一致；module a35dada34c75587ab021bc2d10270514b672281c8515a6b79f126f4659c16a6f复用，contract/descriptor与原基线相同。包9044b738…、Player f530d2cc…、scenario b0cb2e46…逐项身份匹配；未替换原交付或安装。

| ms | 前窗06 | 本窗首期常驻 | 最终热候选 |
| --- | ---: | ---: | ---: |
| CPU mean/P95/P99，排除present wait | 108.6117/121.8271/121.8271 | 85.8384/104.1457/104.1457 | 75.3769/85.3979/85.3979 |
| GPU mean/P95/P99 | 7.5383/7.9964/7.9964 | 8.6784/9.0634/9.0634 | 11.8438/12.4109/12.4109 |
| renderSubmit mean | 106.2375 | 84.4297 | 73.9123 |
| update mean | 2.3386 | 1.3788 | 1.4280 |

最终相对06 CPU均值差-33.2348ms，相对首期差-10.4615ms；各一次短窗不能授稳定收益或将全部变化归因于单项。命令3062→3062。最终短窗CPU预算=false、GPU=true；正式资格未执行，不用8样本授P95/P99持续资格。

GPU均值相对06增加4.3055ms、相对首期增加3.1654ms；原因未定，不归咎环境，不宣称无成本。已有报告复核三次均为frame5–12/tick4–12、命令3062、上传575667400bytes；loading clear分别38/38/39帧。包/module/轨迹及backend/shader源码未变、独立输出对照通过，只能排除已知规模/语义改动，不能用命令数量证明所有GPU payload/调度相同。未采最终每pass GPU或同时钟负载的受控反事实，不能从CPU下降推断GPU上涨首因；此项作为未闭合观察保留，不再重复短窗挑低值。

保持10000实例/100驻留mesh/128材质/96×2048² BC3 mip/10M triangle/145灯/4cascade+8spot/1080p/far2000和原运动；不等同S1的10000 unique mesh。原CPU/GPU P95≤13.3ms/P99≤16.67ms、系统可用6GiB未降低。测量期间无并行构建/检索/施工，Windows Job完整释放。详见[最终比较](../../../../.artifacts/371/s2-cpu-workset-20261018/21-comparison.json)。

## 同窗诊断：证明已生效与剩余

12诊断基于首期07源，尚未加入最终热候选修改，不能冒称最终19源分段。8采样完整：Production 53.3150ms、Graph 8.0797ms、native准备20.7319ms；workset查找/当前lease输入合计0.0038ms。Production内finish排序/分批/完整校验2.9214/5.7074/11.6330ms，不重复相加。native参数准入/准备5.2548/7.6126ms；现有instancing、共享表、proof和bundle已工作。旧分析器未采编码细节的零占位明确非观测，规范化06-stage-profile删除占位。

13同一诊断DLL仅切换原八角culler，current/legacy各一次。选择循环16.2233/19.9359ms，差-3.7126ms；两次命令均3062、原包/场景/module/native身份一致，支持保留新裁剪而不授稳定整帧差值因果。每次只有循环两端计时，不在逐实例内时钟或打印；日志未截断，Job释放。第二诊断的初始helper锚点不唯一在写源前停止，改为完整thread_local锚点后才应用，不是runtime失败。八个临时probe已逐字节恢复，最终18源不含诊断开关。详见[裁剪反事实](../../../../.artifacts/371/s2-cpu-workset-20261018/13-cull-diagnostic/04-counterfactual.json)。

## 构建与运行成本

首红构建132.3712s，首次实现编译失败34.2799s，早期owner构建177.1071s；首期consumer232.4740s、Clippy38.3411s、无probe native397.2999s，首期反馈外层70.4202s/child19.7880s。12/13诊断构建分别373.4276/376.2159s，均有不同的阶段/反事实问题，不作正式资格。

热候选首红构建170.0055s，最终consumer构建322.9967s、测试进程合计59.5276s、Clippy34.8733s、最终native403.0768s。最终反馈外层43.4284s、child17.2260s，父准备/编排余量26.2024s，不称纯Compiler。Compiler/Cook各0调用，没有BM03、workspace或正式三轮；复用原module/资产/包/CLI证据。无安装/宿主/权限更改，无清理拒绝或绕过。窗口14:06 UTC开始，按16:51停止新增/17:06硬截止保留。

## 下一整块讨论草案（尚未激活新增边界）

本合同的常驻来源/空间事实/phase成员/热候选已进入生产链，CPU仍超预算，不能把“workset已接入”写成整块/S2完成。继续盲改单个容器/字段无法解释整个缺口。建议围绕Production→Graph→native一起审查“稳定数据的准备证明与每帧动态选择的分离”，将原完整CPU成本作为出口；保留CPU路径、原预算/规模/画质和已有资源owner。

拟讨论边界：①完整静态binding/range/material/winding等事实仅为完整不可变表产生可审查准备证明，公开/反序列化/可变入口继续严格全验；②每帧仍逐项证明selected索引、唯一性、当前view数值/深度/顺序和完整tail，只有与完整来源及上下文绑定的静态事实可复用；③Graph/RHI/native按相同表/阶段/选择消费，原native参数owner复用纯CPU派生，GPU槽实际内容、current lease/generation、上传/退休准入及真实完成始终另验；④禁止信任裸地址、head成功、数量或hash代替完整证明，COW/同ID异值/失败/overflow/卸载/旧frame都必须反例；⑤不默认改public wire/schema、引入全packet缓存、第二资源权威或GPU-driven。若实际需要这些，另列影响面。

可选：A先审改以上整条CPU证明/消费合同后整体施工（推荐）；B先推进S2其它实际配方/功能，性能仍明确阻塞；C重新评审GPU-driven，成本及平台面显著扩大。此段是讨论草案，不是已确认正式新方案/施工授权，也不重复申请已批准的工作集、分层、A批量或receiver合同。

按[施工skill](../../../../.agents/skills/game-engine-construction/SKILL.md)：“Pause for user discussion when the repair requires changing the framework design or an architectural boundary.” 若采用A，涉及跨producer/consumer的成功证明传播边界，需要确认该新增范围；常规实现修复继续自主执行。

## 保持与未完成项

当前源码正确性/直接consumer通过；原规模短反馈CPU预算失败。完整K1持续窗、其它lit配方实际consumer及S2-D仍未闭合；S3动画/蒙皮/Physics/Nav/空间音频、S4世界/LOD/环境、S5普通项目/工具/交付仍有实际能力缺口。完整memory census/系统6GiB归因/正式三轮/OS显示关联/Android按371-G延期，不能把所有未完成归为测试。Windows only，不改B/Physics/SDK/安装/宿主，不关应用、不降预算/规模/画质。同步合同/总施工/阶段记录/49/54/双README；B行及§0.24以后历史逐字节保持，371不归档。
