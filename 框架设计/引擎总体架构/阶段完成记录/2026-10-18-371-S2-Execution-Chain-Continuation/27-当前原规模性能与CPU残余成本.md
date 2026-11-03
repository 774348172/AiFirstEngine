# 371 §0.30 当前原规模性能与CPU残余成本

日期：2026-10-21。状态：`source_consumer_verified / current_scale_windows_within_budget / sustained_qualification_pending`。本窗收尾，不是S2-D、S2、S1或371全部完成。

本次当前版本原规模复核已取得改善：同一100样本配方CPU均值12.9664→12.1169ms、P95 14.1158→13.1560ms；最终600样本CPU P95/P99为13.1549/13.9392ms，GPU为9.0225/9.2621ms，均落入13.3/16.67ms阈值。但600样本仅约7.26秒、604tick前缀，绝非完整7200tick K1或持续正式资格；启动前物理可用约5.29GiB仍低于6GiB。不能因此宣布性能与资源全部验收完成。

## 范围与身份

用户批准“当前版本原规模性能复核与CPU剩余成本闭合”。03:24:13 UTC开工，默认3小时；06:09:13停止新增、06:24:13硬截止。本窗提前进入收尾，实际结束和墙钟见证据`50-closeout.json`。A路沿原dirty目录/分支，不覆盖其它任务；B空闲。Windows only；Android/371-G延期，不修改安装、宿主、SDK、Physics或冻结游戏V0.0.5。

证据根：`G:/gameEngin/.artifacts/371/s2-performance-20261021-032413`。

- 初始`00-source.json`与前窗386项runtime来源相符；原Release DLL `00242a14730dc24e9dbdbb348db1945b0870d06fac7827eb93e7c1ad3a955058`。
- 首次合并`22-source.json` / `23-product-engine_runtime.dll`：`a681fa48ea0159a8fc79bda8a7e19e079bf35baf4651e874fdbf80ee1fead837`。
- 最终`42-source.json` / `45-final-engine_runtime.dll`：`cdfc72471adc3bd4504cab3b3f64bdbca49b61c5b7254da67cdd420749d4895e`，原Release、1.96.0、locked/-j1、I盘固定SSD缓存，无probe、未安装。
- 原包/Player复用`143-full-k1-project/Build/OriginalScaleK1/release`；packageDigest `9044b738dd55d3b0b596ff5bc68bf8d4c1ba4e6b7eafc618e17a2ee2a2e91d0b`，playerDigest `f530d2cc2015bf497f08f6c4825e6d5299853c21d300c3ffe7115e1182608817`。
- 原项目模块`07-project_runtime_module.dll` SHA `a35dada34c75587ab021bc2d10270514b672281c8515a6b79f126f4659c16a6f`。复用既有CLI/Windows Job进程owner；Compiler/Cook/BM03/workspace/正式三轮均未执行。
- K1为10000实例、100 mesh、128材质、96 BC3纹理/完整mip；不是S1的10000 unique mesh。1920×1080、原相机轨迹/画质保持；RTX3070、Vulkan、NVIDIA610.88。

## 成本归因：观测身份与嵌套边界

`07-profile`/`08-detail`属于首两项修复前；`25-scheduling/08-scheduling.json`属于22源加临时probe，不能作最终42源分段。以下22源数据仅用来定位最后一次修复，8个诊断帧，不授分位数：

| 区间 | 均值ms | 关系 |
| --- | ---: | --- |
| Production | 4.1173 | 包含选择/灯光/任务墙钟 |
| Graph | 1.3922 | 独立后续区间 |
| native准备 | 4.1431 | 在native执行之前；包含资源/参数/模板工作 |
| native执行 | 2.5264 | 包含编码、提交等，非全部queue.submit |
| native模板 | 1.0973 | native准备内部；不能重复相加 |
| 当前模板检查 | 0.6418 | 模板内部 |
| backing census | 0.4327 | 准备内部 |

该诊断每帧约29657个选中索引、9898个唯一实例、13视图，3062条命令。四组直接spawn调用合计约0.2319ms；join等待含必要worker工作，不能全部算成可消除的线程开销。证据不足以把线程池当当前根因，因此未新建任务调度架构。

上述顶层计时没有完整解释CPU/submit余量，不能用差值猜原因。追加单一提交探针`32-submit-probe/07-summary.json`将原先未细分的submit闭合到真实owner：租约集合收集0.9048ms，encoder.finish+queue.submit为0.0987ms，submit父段1.0055ms。每帧遍历29657.125行，却只保留1个独立租约；重复HashSet查询是已确认的冗余工作。日志还含8次零行加载clear提交；解析先因误设总条数失败，后按完整16条区分8个几何帧，未截断猜测。

最终42源没有再构建完整分段probe：收益由无probe整帧反馈验证。最终已知renderSubmit均值为10.8275ms（100样本）/10.4922ms（600样本），update为相应原始summary；不能把22源阶段数值标成最终42源耗时。

## 实际修改：五文件、三个owner内部消费点

1. `engine_runtime_core/src/render_geometry.rs`、`render_geometry/proof.rs`、`render_geometry/commands.rs`：私有CompactView完整selection成功后，FinalViewState仅调整最终光照/命令元数据；发布最终head时消费该确切instance/view乘积有限性事实，避免再乘一次矩阵。最终环境/灯光/阴影、标量、view和完整tail检查保持；公开严格validate、detached/wire/COW仍严格。未prepared路径仍要首次完整head检查，2N→N；prepared发布不重复该乘法，不能把这说成不检查选择。
2. `engine_render_native/src/wgpu_backend/real/geometry/native_template.rs`：仅一次prepare的不可变registry借用内，按确切活pipeline key分配和完整字符串值复用当前pipeline查询；每模板仍对实际pipeline对象、几何buffer/range及材质依赖判等。fixture真实4个不同key，查询8/64/512→4/4/4。调用结束事实即失效，不新增跨帧GPU有效证明。
3. `engine_render_native/src/wgpu_backend/real/lifecycle.rs`：原提交租约收集提取为私有纯函数；若当前lease与结果Vec最后已保活lease为同一Arc，免去重复HashSet操作，否则仍走原精确集合。每个选中行照常访问，非连续重复/非head/COW/flat/混合/旧帧照常处理；只保存实际被选择资源。4/32/256行×3视图的真实集合操作12/96/768→1/1/1，行访问数完全相同。原pending、queue完成callback、epoch及真实GPU完成点释放不变。

没有修改shader、GPU布局、规模、预算、预取、资源owner、reader或公开schema。新增临时pipeline witness沿既有Current调用内scratch释放，不宣称完整CPU allocation census；租约修复复用原Vec/HashSet，不增第二份租约表。相机裁剪、透明/阴影选择、generation/revision、完整tail、跨batch唯一性、部分上传重试和队列/上传/退休上限保持。

## 原规模无probe结果：保留无收益和失败窗口

CPU均指排除Present等待的合同口径；各分位数不能从总帧或不同区间互相替代。`overall=passed`只表示语义/进程检查通过。

| 记录 / 来源 | 样本 | CPU均值 / P95 / P99 ms | GPU均值 / P95 / P99 ms | 判断 |
| --- | ---: | --- | --- | --- |
| 02 / 初始00源、01 DLL | 100 | 12.9664 / 14.1158 / 14.4134 | 8.0942 / 8.8934 / 9.1085 | CPU P95失败 |
| 24 / 22源、23 DLL | 100 | 12.9144 / 14.1547 / 15.3708 | 8.1002 / 9.0081 / 9.0890 | 前两修复无整帧收益，仍失败 |
| 31 / 同22源、23 DLL | 600 | 12.1957 / 13.5957 / 14.6047 | 7.9027 / 8.9580 / 9.3266 | 更长前缀，CPU P95仍失败 |
| 46 / 最终42源、45 DLL | 100 | 12.1169 / 13.1560 / 13.8835 | 8.3651 / 10.2472 / 11.0653 | 当前窗CPU/GPU阈值通过 |
| 47 / 同最终42源、45 DLL | 600 | 11.7742 / 13.1549 / 13.9392 | 7.9737 / 9.0225 / 9.2621 | 当前扩展窗CPU/GPU阈值通过 |

相同100样本初始→最终CPU均值少0.8495ms（6.55%）、P95少0.9598ms；renderSubmit少0.8418ms，update基本相同。600样本相同前缀22→42源CPU均值少0.4215ms、P95少0.4408ms。不同长度轨迹不能当同输入提速A/B；不能将环境变化全归于代码，GPU也没有声明优化收益。最终P95仅约0.145ms余量，不能推断所有相机位置稳定通过。

100样本实际采样墙钟初始1.3311秒、最终1.2449秒；600样本修改前7.5168秒、最终7.2646秒。4帧warmup保持，600样本的requestedTick600断言及其余四个语义断言通过。复用CLI最多支持604帧，本窗未为拉长测量重建CLI或另造runner。没有运行完整7200tick、持续dirty变体或正式三轮，不授P95/P99持续资格。测量期间无构建、检索、编辑。

## 验证、首红与证据复用

有效去重并集为196项：geometry126 + 单独执行的保守阴影GPU像素1 + native模板5 + lifecycle5 + GPU batch4 + visual28 + U2 27。`15`、`17`、`21-shadow-pixels`验证前两项且其owner代码后续未变；最新lifecycle由`39`验证，真实GPU非head租约直到completion的`40`四项及`41`两个consumer终验覆盖最后修复。不是196项都在最终同一测试二进制重跑。`41`的54/28是用例总数，不是通过数；其中26+1项ignored未执行，未计入196。没有用更多测试补替代原规模反馈。

真实失败/返工保留：首次probe锚点重复在写入前拒绝；native首红旧查询8/64/512，后续期望1/2因fixture实际4个key修正，未降低产品边界；prepared测试一次借用生命周期E0597修正；lease首红37先编译失败（漏MaterialBlend import/借用），38才取得真实12/96/768查询失败，39转绿。中间24/31预算失败保留。24的首次启动guard检测到施工进程时没有开始测量，待进程检查为空后才执行一次有效窗口，不猜测具体进程原因或挑绿。

native私有owner用例本次实际运行`engine_runtime --lib`过滤目标，是重组合单测构建，不能标成独立native integration快测；外部真实`platform_3d_visual`与`unified_render_execution_u2`为集成consumer。既有私有测试seam已能捕获本次故障，未为提速新建测试架构。

五文件fmt通过，core/native scoped Clippy及core默认feature check通过，既有warning保留。四轮临时probe均按原字节恢复并刷新mtime；最终386源哈希匹配42，较00恰好五文件变化。`48-closeout-audit.json`保存明细，`49-fmt.json`为明确fmt回执；不借rg无匹配的退出1误报格式失败。其它dirty工作未reset。

## 费用与施工复盘

`48-closeout-audit.json`逐项聚合runner结果，包含红例、编译失败、返工、无收益产品和全部probe，不将runner父区间与子区间重复相加：

| 互斥已计时类别 | 秒 | 分钟 |
| --- | ---: | ---: |
| Cargo构建（不含Clippy/check） | 2305.625 | 38.427 |
| scoped Clippy | 24.838 | 0.414 |
| core默认feature check | 6.837 | 0.114 |
| 定向测试执行（含失败、单独GPU测试） | 53.918 | 0.899 |
| 九次Player/harness外层（含诊断） | 510.312 | 8.505 |

四次probe Release构建约24.54分钟，两次无probe产品Release约7.88分钟；最终native-only Release107.353秒。核心修改/恢复会让原Release重编core，原owner定向红绿仍用了组合lib；构建拆分不等于所有代码变动都能秒级。最初整体分段仍漏掉提交租约开销，导致先做两项局部优化和进一步探针；这是本窗额外诊断成本，不能写成“只测一次/只构建一次”。以后同类问题先覆盖最终提交和租约owner，再判断调度设计，避免只盯Production/prepare。

Player外层包含复制/身份/启动/资源准备/实际子进程，不全是测量时间。初始父149.619秒、子28.893秒，不能把父时间冒充Player运行或额外加到总计；最终100/600父52.609/44.217秒。所有采样区间单独列于上文。未计时到的阅读、编辑、fmt、审计、文档与等待留在总墙钟，不伪造精确分摊。无Compiler/Cook成本；没有清缓存、改宿主或安装新二进制。

## 出口、未达标项与下一继续点

- 已闭合：当前Release原规模身份复核、两种窗口反馈、三个原owner内部消费修复和受影响正确性证据；本次两个最终窗口CPU/GPU数值通过。现有执行链无需凭本窗结果推翻重写；本次首因是剩余消费实现里的重复工作，不是少了新的3D功能或GPU算力不足。
- 仍未闭合：完整原K1周期/持续dirty变体/正式资格；本次没有证明最终所有CPU子段或所有相机位置都有预算余量。不能把“当前窗口阈值通过”推广成所有性能完成。
- 独立资源前置仍失败：初始测前4.38GiB、最终100/600测前约5.35/5.29GiB，全部小于6GiB；仅启动单点，不是全程最低值或引擎/其它进程因果归因。未关闭其它应用、扣工作集或降低门槛。完整内存owner覆盖/OS显示/三轮及压力矩阵仍按371-G延期。
- 开发继续点回到26记录的普通项目材质/资源变化、重Cook/增量失效与旧帧退休可见对照，复用当前owner及本窗CPU修复；尚未在本窗扩大实施。当前DLL未替换26的交付或全局安装，不能声称普通项目/已安装引擎自动包含本次修复。剩余S2配方consumer按具体风险推进；不以延期资格阻塞所有功能，也不授S1/S2/371完成。

本窗没有新的架构边界变更或清理请求；此前`blocked by policy`目标未重试/绕过。所有自有测量进程正常退出；最终进程审计和源码/交付身份见`50-closeout.json`。371总施工仍留当前，不归档。
