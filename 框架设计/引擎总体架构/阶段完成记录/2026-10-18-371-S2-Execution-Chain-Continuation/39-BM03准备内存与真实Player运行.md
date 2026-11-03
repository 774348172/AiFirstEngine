# 371 §0.33 阶段39：BM03准备内存与真实Player运行

## 1. 结论与范围

本窗已把修正后的BM03 1×K0贯通到完整Compiler成功返回、实际交付Game.exe加载/绘制/截图/正常退出；修复了准备数据重复持有，以及首次真实运行暴露的网格上传无进度缺陷。**本整块仍partial，S1/S2/371未完成**：运行期系统可用内存未达到4.5 GiB，其他固定退出和完整矩阵尚未核销。

10:19:52 UTC开工，默认3小时包含阅读、构建、测试、等待和收尾；13:04:52停止新增、13:19:52硬截止。本窗在完成当前因果修复和最终反馈后提前收尾；实际结束以证据根`35-closeout.json`为准，不沿用旧5/8小时授权。

证据根：`G:/gameEngin/.artifacts/371/bm03-memory-20261022-101952`。沿原dirty分支`codex/255-capability-catalog`，HEAD `acaffa8cb95bbb54d093bbf45835a59315c82da5`；60项最终输入核对无漂移（34-source-review）。未reset、安装、修改宿主/SDK/Physics或游戏V0.0.5；B路374原登记逐字保留。Windows only，Android与371-G既有延期不变。

## 2. 准备内存首因与实现

一次有界Cook探针观察到同时存活的实际backing：snapshot原始源875,266,200 bytes、独立source view再875,266,200 bytes、mesh/material cooked capacity 1,310,982,144 bytes，以及RGBA 1,073,741,824 bytes。同批序列化实际长度854,574,346 bytes，约435 MiB为持久保留的增长余量。外部采样根进程私有峰约4.16 GiB；这不是完整owner内存资格。

在既有owner内修改：

| 文件（rust/crates下） | 实际消除的工作/寿命 | 保留边界 |
| --- | --- | --- |
| project_authoring_execution/src/game_project_compiler.rs | Prepared私有source改为Arc；导出请求共享冻结backing | 原source读取、摘要及公开语义保持 |
| 同crate/src/desktop_export.rs、project_player_artifact.rs | request clone不再深复制约835 MiB源文件 | serde跳过的私有字段；非新公共schema |
| 同crate/src/render_asset_cook.rs | 冷生产/缓存命中序列化payload进入持久持有前收缩capacity | 字节/hash/typed decode/cache key保持 |
| 同crate/src/platform_3d_benchmark/driver.rs | prepare后释放lease/session；成功交付后、启动Player前释放prepared | Compiler完成源绑定检查后才释放；Player只读发布包 |
| 同crate/tests/game_project_compiler_interface.rs | 证明lease/session释放后prepared及clone仍读原冻结源 | 不删源漂移拒绝或校验 |

两项确定性首红分别证明request clone复制backing、序列化capacity 1024大于len 756。多规模16/1024/65536/1MiB指针及存储存活验证、冷/暖payload长度/容量/摘要验证通过。**最初snapshot→source view的一次独立复制仍存在**；本窗没有改变公共源所有权合同。

最终完整操作的阶段采样根进程私有峰：Cook发布前3.7124 GiB，export阶段3.2032 GiB。启动真实Game后Compiler父进程峰仅16,617,472 bytes（约15.85 MiB），证明跨Player的准备数据重叠已消除。与历史系统available的差异受环境影响，不能全部当代码净收益。

## 3. 真实Player首错：整体网格上传永远无法准入

第一次导出已发布实际Game、DLL和包，但外层Compiler后置source绑定扫描尚未返回即触及1200.432s诊断上限；保留17/18证据，不称整条Compiler成功。直接消费已发布Game后，实际PID 28696运行134.556s、0帧、`geometry.upload_timeout`退出。详细报告`19-player-report.engine-runtime.json`已有，无需为补日志重建。

首因位于原native `prepare_geometry_plan_for_extent`：将全部缺失网格312 MB一次申请原16 MiB/presentation额度；每轮整体拒绝，纹理已上传，网格无prefix进度。发生5505次loading present后超时。不是Cook故障，不是CPU绘制预算失败。

修复文件：`rust/crates/engine_render_native/src/wgpu_backend/real/geometry.rs`。完整plan/source、资源与退休校验仍先完成，沿原mesh/page/queued-upload owner，仅对当前剩余额度容纳的完整mesh前缀计费并写入；保留已写mesh，未写range guard释放，返回原capacity重试。loading clear实际提交后下次presentation继续，不在同presentation制造新额度，不在资源未齐时认证plan或draw。不改ticket/schema、resident/staging/在途/退休上限。

多规模16/64/256真实GPU首红确认旧路径第一帧上传0、应为11。修复后验证实际上传数、同presentation重试不重复获额度、loading clear/真实完成和最终ready。中间N64失败来自测试固定512 bytes额度连768 bytes原子索引表也放不下；修正fixture为N×32额度，保持“索引表可容纳、所有mesh总和不可容纳”的产品反例，不提高生产预算。

**未闭合的边界**：单mesh大于整个presentation额度仍依赖原分片ticket接入；原子索引表自身超过额度不是本前缀修复解决的情形。本窗不宣称全部容量/耗尽配方通过。失败摘要中的waitWallNs没有覆盖超时前全部等待，外部134.556s为本次运行时长；该观测缺口保留。

## 4. 最终实际交付与性能反馈

有效项目源沿阶段38：`.artifacts/371/world-prep-20261022-073738/58-corrected-BM03-1-K0`，源摘要`sha256:fee704a0eeabc550ae5172a958f11b8680b9b719f291f4ba4b943f75f85a464d`。实际10,000独立mesh/instance、256material、64texture、10M triangles、1 GiB decoded RGBA、192 MB vertex和120 MB index；未缩规模。历史六组576张空PNG仍无效，不冒充已修正全部输入。

最终交付：上述项目`Build/31-final-BM03-1-K0/release`。

| 身份 | SHA-256 |
| --- | --- |
| Game.exe | 132a9197c4c81c248c4e04eb2a6494c3c55085ca5c2c38540a2f626fd6627ac1 |
| engine_runtime.dll | 01a523790da92f3a92bae49c95ffdd02270db3debf3974e723bca0677efcb876 |
| package-manifest.json | 11b87df103fb767b4490bf686b04419be4c670d611dbede2b37081eda198512f |
| 最终Compiler工具 | 7ae6b58427b6e3c57a296f1bf10c00b995825b43c167687300ede137e8bb9a28 |

31完整操作exit0、实际Game PID29160完成120帧/Present/截图/exit0；外部898.310s。所有10320 producer均命中，198 Cargo fresh/8 rebuilt；native修复使engine缓存按`build_inputs_changed`重编。首次加载84次present、等待2.7376s、实际上传1,419,648,000 bytes，单帧最高16 MiB、staging峰33,508,800 bytes、在途峰2，未提高预算。此前29是显式诊断DLL覆盖的独立消费者，只授该身份120帧/8断言/1截图，不把其性能冒充最终交付。

随后33直接运行同一最终Game及packaged DLL，120帧预热+600采样、Immediate/unpaced、Summary/nativeEvents=false，无并行施工，exit0完成720帧并截图。

| 当前最终Game短窗 | 均值ms | P95 ms | P99 ms | 最大ms |
| --- | ---: | ---: | ---: | ---: |
| CPU整帧 | 3.6824 | 4.3894 | 4.8624 | 101.5560 |
| CPU扣Present等待 | 3.4829 | 4.1515 | 4.6184 | 101.3101 |
| GPU timestamp | 1.8550 | 2.1043 | 2.5651 | 2.6051 |

update均值0.5737ms、renderSubmit 2.6373ms、Present等待0.1995ms；这些不冒称完整底层成本归因。600样本只跨2.2618s，末帧截图仍计入CPU最大值，未删除异常帧；不能授持续P95/P99或正式资格。加载93次present、等待1.2253s与31属于不同运行，均保留。静态采样起止10k meshes/64 textures/1 GiB纹理不变，非完整流式/回访/卸载窗口。BM-12/foundation-v1是本driver报告标签，本次实际只覆盖其中BM03 1×K0输入，不代表BM12全矩阵。

实际截图`33-player.png`已查看，可见完整万对象纹理网格；不以非黑或人工查看冒充独立像素、PBR、光影资格。没有与本次相同有效输入的旧可绘制结果：修复前为0帧超时，故不计算虚假的整帧提速比，也不与BM01或历史旧DLL跨配方比较。

## 5. 内存门槛：仍失败，和功能/帧预算分开

预算保持：CPU tracked/进程private/GPU tracked分别4/6/4 GiB；系统available至少4.5 GiB（4,831,838,208 bytes）。**进程private不是4 GiB门槛**，也不能用private/working set相减伪造系统余量。

| 最终运行 | Game私有峰GiB | Game working set峰GiB | 运行期间系统available最低GiB | 结论 |
| --- | ---: | ---: | ---: | --- |
| 31完整Compiler→实际Game | 4.5045 | 2.8625 | 3.0443 | private低于6；系统4.5失败 |
| 33最终Game 120+600 | 4.5085 | 2.8237 | 3.1492 | private低于6；系统4.5失败 |

33采样窗口CPU resident报告1,939,628,061 bytes，不等于完整CPU backing覆盖或进程所有权总量。约1Hz外部采样不能证明瞬时绝对峰或全部owner；完整source/renderer/队列/退休覆盖仍按X4核销。原始monitor `minimumAvailableBytes=null`是run-owned PowerShell OrderedDictionary归约问题；保留原始记录，由未改samples生成32/34归约，不修改原证据。

17环境快照显示ChatGPT等测试宿主、系统和其他进程有实际驻留占用；它只是一时快照，不能从中推算运行全程环境精确贡献。已证明Compiler准备父进程不再与Player重叠；**尚不能证明其余余量失败完全是环境或完全是引擎**。不关闭应用、不改宿主配置、不扣减工作集、不降低标准。本轮已完成局部存储修复，但运行期内存仍需要在当前包上定位实际backing与加载临时存储。

## 6. 验证、真实失败与费用

75项去重有效源验证：31 authoring owner、22 Compiler接口消费者、20 native owner、2 native integration；fmt与两个scoped Clippy通过，既有warning保留。native owner中19项复用23当次通过结果，最终fixture的1项由25单独刷新，产品实现身份未变；不称75项全部在最后test二进制重跑。三个未修改的真实Cargo错误检查按14命令明确skip，不运行workspace/BM03其他倍率/正式三轮。World阶段38短窗、X5工具修复按未变消费身份复用，不重做。

真实失败与返工均保留：两项内存首红；一次相对目录实施脚本失败后09 no-change构建，非实现通过；desktop filter第一次0 tests，不计绿，后用正确filter3项；17外层诊断中止；19真实GPU加载超时；native首红；23索引额度fixture独立失败后修fixture；证据归约脚本两次字段形状/编码读取错误已修，原数据不变。普通错误未申请重复授权，未绕过权限拒绝。

| 成本类别 | 实际秒 | 口径 |
| --- | ---: | --- |
| 直接Cargo构建（含诊断、红绿、失败后无变化调用） | 1151.043 | 19.18min；03冷feature组合629.659s单列 |
| 两次导出内嵌Cargo模块/engine构建 | 684.491 | 已包含在下方export运行，不重加总时长 |
| 构建合计 | 1835.534 | **30.59min**，不含Clippy，非整条Compiler耗时 |
| scoped Clippy | 49.077 | 0.82min |
| 定向测试执行含首红/fixture失败/0 tests | 92.735 | 1.55min |
| Cook诊断/两次export/失败Game/诊断消费者/最终反馈 | 2655.065 | 44.25min，包含内嵌Cargo |

最终31内部snapshot182.048s、Cook144.751s、export546.517s、Player验证20.681s、实际Game18.744s；全部嵌套在外部898.310s，不能相加当额外施工时间。export含source绑定/暂存/发布，纯engine Cargo只有186.775s、module0.559s。详细逐调用见34-cost-ledger；整个窗口还包含阅读、编辑、诊断、证据与文档时间，见35-closeout。未清理共享缓存或发布目录，必要失败和原字节证据全部保留。

## 7. 固定出口与下一继续点

| 固定出口 | 本窗后状态 |
| --- | --- |
| X1 World更新/预算 | 阶段38万实体100%dirty 120+600 CPU P95/P99=12.4038/13.6843ms复用；不是当前BM03结果或持续资格 |
| X2 BM11固定F01–12跨owner组合 | 原有局部证据保留，完整固定组合仍未闭合 |
| X3 BM12真实压力 | 有效BM03 1×K0现已Cook→export→真实Game；其余五组有效输入、倍率/K0/K1与耗尽证据仍缺，不扩大退出表 |
| X4 实际backing | 本窗只修准备共享和capacity/寿命；可访问存储完整覆盖仍partial |
| X5 真实工具身份 | 未改消费输入，复用阶段37关联回执与真实工具结果 |
| X6 运行资源/持续与显示资格 | 当前BM03快速CPU/GPU分位入预算；系统4.5GiB失败；完整流式、OS显示/正式资格按371-G和固定清单保留 |

下一继续点优先复用31最终包及Game/DLL，定位BM03运行期资源backing与加载临时存储，分清实际所有权、共享和释放点，再修原owner；不先重跑Compiler/Cook或正式三轮。准备初次源复制、单资源超额度路径如需改变新公共所有权边界，先形成具体合同/反例/选项讨论。本窗不新增全局allocator、第二资源系统或新的S1退出条件，也不以剩余时间不足解释产品缺陷。
