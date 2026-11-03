# 371 §0.25 当前视图上界与 native 准入消费

日期：2026-10-19。状态：partial / source_consumer_verified / scale_budget_failed，formalQualified=false。本窗11:11:45 UTC开始，13:56:45停止新增/14:11:45硬截止；本记录按已取得的实现与证据提前收尾，不把时间上限当作缺陷首因。整条CPU执行链、S2-D、S2、S1与371均未完成。

## 授权、基线与实际链路

用户“按照你的建议开始下一步施工”继续同一大块；不是另建大块或重做共享表、instancing、bundle。前窗95输入/21源/24 DLL/25反馈身份复核无漂移，原dirty分支codex/255-capability-catalog、F盘Cargo target复用。未改安装、宿主、B路、Physics、SDK或V0.0.5冻结目录。Windows only，Android与371-G完整内存、OS显示、正式三轮继续延期。

本窗有效产品身份为[10-source](../../../../.artifacts/371/s2-submit-20261019-111145/10-source.json)的95项输入、五文件变化；[13无probe DLL](../../../../.artifacts/371/s2-submit-20261019-111145/13-native-identity.json) SHA256 F6E285F62926B5A9E083A3068583D50D88A97A0C8394E2AFD66209E6BC4F374F，仅诊断使用，未安装。原包、Player、场景与项目模块身份和前窗相同，module实际绑定通过；K1是10000实例/100 resident mesh/128材质/96×2048² BC3 mip纹理/10M triangle/145灯/4cascade+8spot/1080p/far2000与原运动，不冒称S1的10000 unique-mesh配方。RHI命令3062是最终帧命令数，不是真实GPU draw计数。

## 当前成本分解与原因判断

第一次00源诊断只授00源：Production17.1977、Graph5.0239、native准备13.6328、执行5.9404ms。实施后无probe仅小幅变化，按原要求复审假设，再做一次[17当前源诊断](../../../../.artifacts/371/s2-submit-20261019-111145/17-current-diagnosis/04-profile.json)。它绑定10源和独立probe身份，12帧/4预热/8样本、8331 bytes无截断；下表是仪器版本，不能当作无probe性能，不能把两次不同探针的子段差异独立归因给某项修改。

| 当前10源互斥区间 | 均值ms | 内部主要子段（嵌套，不重复相加） |
| --- | ---: | --- |
| Production | 15.5546 | selection3.9043、sort2.3683、batch1.2196、tail1.8175、receiver3.0494、casterBounds1.1299、directionalPose0.5627；余量1.3771 |
| Graph | 4.4923 | append1.3991、compile1.9739；declarations1.6531包含于compile，余量1.1192 |
| native准备 | 11.3056 | capacity1.9029、resources4.9667、parameters3.1429、capture1.2757；余量0.0174 |
| native执行 | 5.2417 | intent1.4302、encodeGeometry2.3150；recipeCapture1.7792和bundleRecord0.2299包含于encode；余量1.4965 |

renderSubmit扣上述四段的未覆盖余量2.3563ms，不能归给任一已知owner。resources内部resourceLoop1.5226、textureResidency1.4578、litBindings0.5704，其余子段及余量在完整JSON。参数内mapping1.7558、instanceRecords1.2246，不能把它们又加到parameters之上。native完整batch证明0.4640ms与跨batch逐row唯一性1.2160ms是不同事实；逐row唯一性仍保留。

每样本实际计数：110000候选、29657.125选中实例、3043.25批次；18785.125实例参数值、11 view值。spatial输入105000，104926.875项直接决策、73.125严格回退（约0.070%）；每view上界11次/88 dot。旧unpreparedSelection、整表proof clear、实例派生缓存满容量clear均为0。batch完整证明3043.25次、完整draw检查5690.5次、静态全表行解释0；不能见到validator就称“重复全验”。

每帧fullPlanPrepares=1、fullPlanReuses=1，是两个调用中一次完整准备、一次同帧复用；不是“每帧两次完整准备”，也不能把同帧命中当跨帧稳定准备已复用。原ValidatedGeometryPlan::matches_commands（native geometry.rs:1302）、prepare_geometry_plan_for_extent（:1893）与reuse_geometry_intent（:1775）仍以完整命令匹配为条件，且Weak过期会miss。代码证明动态view等差异可使其miss，但本次未逐项记录每帧首个miss分支，不能声称已区分Weak过期、view变化、schedule或其它条件的具体份额。bundle已经存在，每样本录制约2964.25命令；这不是缺bundle的证据。

判断：已确认的冗余属于原owner实现粒度，本窗已修；主要剩余成本分散在动态选择、receiver、Graph消费和native准备/执行，尚无单一根因或一个小缓存足以闭合预算的证据。不是仅“正式测试未完成”导致慢，也不能据此判定整个3D方案错误。已有成功证明没有消除这些实际工作；当前执行链尚未收敛。不能从局部计数推断整体将达标。

## 五文件已实施结果及边界

| 原owner | 首红/已消除工作 | 保留边界 |
| --- | --- | --- |
| render_geometry/workset.rs、build_inputs.rs、render_geometry.rs | 同一当前view不再对每row重复计算全局溢出/误差上界；4/32/256行首红32/256/2048 dot，修后每view8；PreparedInputs冻结一次保存逐分量上界，生产路径实际消费 | 不缓存动态可见结果，逐候选平面测试保持；上界不安全则原逐row证明，再不确定则严格culler；独立严格输出及6000余边界/移动/镜像/剪切/极值反例 |
| native geometry.rs | 调用内以view组织完整instance_id集合；view读取由逐row改为每batch每pass，两个view两batch首红8/64/512次，修后4次 | 两次view读取是容量hint与真实检查；总预留≤65536。全selection、跨batch/不同table同view冲突、尾项、容量及首错顺序仍验，不是跨plan证明 |
| wgpu_backend.rs | 调用内缓存交错binding的纹理需求，空需求也保存；两种交错binding首红4/32/256次，修后2次 | 仅同allocation复用解析；detached相等仍完整比较。lit五槽先验与旧首错顺序不变，每调用重新查询当前registry/generation，不能以CPU解析成功当resident |

新上界为原PreparedInputs内联数据；调用内HashSet/HashMap bucket backing的完整内存观测仍未接入，不列为平台不可观测，不授完整census。没有新schema/命令、资源owner、跨plan成功证明、GPU布局或shader，也没有把CPU命中变成上传/退休事实。

## 验证、失败与复用

[07首红](../../../../.artifacts/371/s2-submit-20261019-111145/07-red-test-timing.json)三条真实多规模成本测试预期失败，产品修改后6条窄例通过。[11合并终验](../../../../.artifacts/371/s2-submit-20261019-111145/11-test-results.json)260项去重全部通过：owner158、visual54、U2 28、lifetime5、geometry15；包含旧254项与新6项，不能再把早期窄例相加。五文件fmt、最终scoped Clippy及default库check通过；库152/visual2条既有Clippy warning、default10 warning保持，不把它们写成新失败或已清债。

返工两项均保留：09测试夹具局部声明顺序导致E0597，调整测试生命周期后继续；09b误认为f32::MAX一定在当前正交投影溢出，实际小系数可安全缩小，修正为identity轴系数32×MAX的确定溢出夹具，09c绿。不是通过削弱产品断言挑绿。旧窗口E0502 metadata误继承已保留修正前manifest，并在10澄清；不是本窗第三个产品失败。

17探针13文件已字节恢复，95输入匹配10；首次探针11文件恢复亦有独立记录。260项终验、13无probe DLL、14反馈仍有效，不为探针恢复再构建或重测。复用原包/module/CLI，Compiler、Cook、BM03、workspace和正式三轮调用均为0。

## 原规模无probe反馈及资格

| 同原K1，ms | 前窗21/24/25 | 当前10/13/14 |
| --- | ---: | ---: |
| CPU mean / P95 / P99 | 39.7182 / 44.8813 / 44.8813 | 38.4147 / 40.0713 / 40.0713 |
| GPU mean / P95 / P99 | 7.3518 / 7.3902 / 7.3902 | 7.8309 / 7.8674 / 7.8674 |
| renderSubmit mean | 38.4638 | 37.0628 |
| update mean | 1.2258 | 1.3234 |

CPU均值观察差-1.3035ms（-3.2819%），仅一次8样本短窗，未证明持续收益或主要链收敛；GPU均值增加0.4791ms，没有GPU算法改善声明，波动原因尚未建立。CPU P95短窗仍比13.3ms高26.7713ms；P99比16.67ms高23.4013ms。P95/P99仅描述这8项，不授持续尾分位资格。原预算、规模、运动和画质不变。

首次诊断前物理可用约4.516GiB，最终14前5996360KiB约5.719GiB，第二诊断前6161984KiB约5.876GiB，均为启动前单点，低于6GiB；没有分离全程引擎/测试宿主/环境占用，不归因尚未启动的Player，不从可用内存扣工作集、不关闭其它应用。最终采样private commit峰值2341474304 bytes/working set1201549312 bytes只是采样窗，不等全运行内存资格；原6GiB门槛和371-G延期边界保持。

17构建期间观察到另一任务Cargo构建，保留02-other-build记录；没有终止它，结束后才启动测量。14有效反馈在该并行构建之前；17测量期间只有等待，无检索、编辑或并行构建。三个运行均有Windows Job释放及reader join证据，未改宿主或安装。

## 构建与运行成本

| 互斥成本类别 | 秒 |
| --- | ---: |
| Cargo构建/测试准备，含两次probe与无probeDLL及夹具返工 | 2056.373 |
| Clippy | 36.841 |
| default check | 22.758 |
| fmt | 0.912 |
| 测试执行，含预期首红和夹具失败 | 69.735 |
| 三次反馈outer总计 | 160.373 |

outer包括准备/runner/child，不能与child、加载等待和采样再次求和。17 outer67.577s，其中child17.955s、加载约0.713s、8帧采样约0.325s；outer-child49.622s仅是准备/runner余量，不冒称compiler-only。前两次明细在15-comparison，合并明细在18-final-audit。新增诊断因无probe仅小幅变化而启动，不是同源反复挑绿；本轮不继续追加第三次昂贵探针。

## 下一步具体可讨论范围（草案，尚未批准/激活）

推荐A：先审改“稳定准备域与动态执行域”合同，再在同一整条链整体实施。新增讨论边界只包括跨plan的资源准备成功事实/依赖闭包与失效键，不重问已批准的实例表、批量命令、CPU静态准备证明或纯参数派生。先用现有owner反事实和真实底层操作计数确定完整命令miss中可分离的稳定工作；无法证明主要可消除工作量就不扩大实现，不把11.3056ms全部当成可省成本。

- 现状：原native完整成功plan把view/selection/order与资源需求/结构准备放在同一匹配域。新候选将稳定table/binding/资源声明依赖、pipeline输入与动态view/selection分开，但继续由现有Graph cache、native GeometryGpuState及参数/资源owner持有，不新建第二权威。图声明与native依赖闭包的跨plan成功复用会改变原证明边界，必须审改后才实现。
- 稳定键必须覆盖不可变backing、完整内容/来源、phase、附件布局/format/extent、device代际及其实际依赖；ID/hash只能定位候选。Weak过期/COW、相同ID异值、不同table/新增tail、资源换代、schedule/extent/format变化、失败/部分准备不得复用旧成功。是否为首个miss条件应在owner接缝区分，不能预设全部是相机。
- 动态域仍完整处理当前候选/裁剪/深度/透明顺序、选中tail、跨batch唯一性、当前资源manager lease/generation及native resident、队列/上传上限；CPU准备命中不授GPU槽、已上传、bundle可用或资源仍在的证明。native实际对象和依赖变化须在现有owner判断，真实completion退休保持。
- Production只在原准备数据与table/index消费者中收敛已测receiver/选择工作；Graph只分离可证明稳定的声明/依赖工作，native只复用对应成功事实。不是仅给native加一个bool缓存，也不保证一次即可达预算；用多个规模点的真实操作与一次最终无probe原K1裁定收益。
- 最低反例：移动相机但稳定资源仍须更新view/selection；同ID新table/末项非法/新增未驻留资源必须拒绝；同表不同phase与透明屏障不能误复用；GPU部分上传重试、旧在途帧与真实退休不能被CPU命中覆盖；稳定域失败不发布成功。沿现有owner oracle/像素及直接consumer，先窄例、一次受影响终验、一次无probe反馈，不重跑Compiler/BM03或正式三轮。

备选B：不扩展跨plan证明边界，继续原合同内的receiver索引和参数按table消费。当前receiver3.0494、parameterMap1.7558ms是各自整个子段，不代表这些时间都可移除；必须先用真实计数确认冗余比例。该路线可作为有证据的局部修复，但没有足够证据承诺弥合当前CPU约26.77ms的P95缺口。不推荐把B包装成新的“大块完成”。

A的草案自审：没有把稳定资源需求当驻留成功，没有把同帧reuse=1当跨帧命中，没有把资源/Graph全部计时承诺成收益，没有删除动态检查或降低预算；保持现有owner、CPU路径和画质。待用户确认后才更新正式合同/施工Gate并激活新增证明边界，本记录本身不授施工。

## 已闭合与未闭合

已闭合：本窗五文件实现、真实多规模首红转绿、260项受影响终验/检查、当前源码有界成本分解、有效无probe原K1短反馈、探针恢复与证据身份。未闭合：主要CPU消费路径/原预算及整块，下一步A尚待确认；稳定准备可分离的具体工作份额和每帧首miss分类仍待owner反事实验证。正式资格、全运行内存与原6GiB、OS显示、完整压力配方继续按371-G记录；本窗不宣布S2-A/S2-D/S2/S1/371完成。
