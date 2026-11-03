# 371 §0.33 阶段38：World更新成本与BM03准备阻塞

本窗集中处理原 S1-X1/X3，没有增加 S1 完成门槛。**当前原规模 BM01/100% dirty 的快速预算反馈已通过；BM03 1×K0 修正源后的完整准备已贯通。S1/S2/371 仍 partial**：快速反馈不授持续资格，Cook 不等于 Player/压力资格，准备期间的系统余量也未满足 4.5 GiB。

## 1. 窗口、范围与身份

2026-10-22 07:37:38 UTC 开工，默认三小时；10:22:38 停止新增、10:37:38 硬停止。实际提前结束时间及总耗时见本窗 `66-closeout.json`，不沿用旧五/八小时授权。仓库 G:/gameEngin，分支 codex/255-capability-catalog，HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5；保留既有 dirty，不提交/reset。B路374最新登记保持。

证据根：[world-prep-20261022-073738](../../../../.artifacts/371/world-prep-20261022-073738/)。00-baseline 复核前窗52项身份无漂移；53-current-evidence 分列当前源码、测试、二进制与各阶段费用。不是 clean commit seal。Rust1.96.0、--locked -j1、I:/gameEngin-build/371/cargo-target、原Release不变；Windows only，Android延期，未改安装/宿主/SDK/Physics/游戏V0.0.5。

必要性与验证经济性复核：World先取一次基线分段，以真实底层计数证明消除工作，再验证原规模；BM03先定位SourceInventory，再隔离通用JSON与原资源Cook，遇实际错误修原producer。排除全workspace、Compiler导出、BM03 Player、正式三轮和其它S1项。原子性、当前资源准入、generation/取消、旧packet/真实GPU退休及生产预算未放宽。公开合同未变，无新增架构边界；本窗没有修改公共skill。

## 2. World：确认成本、原owner修改

12-current-world-profile：修改前当前基线Release，64帧中舍前8帧，56个有界诊断样本。frame inclusive 11.59250ms；transaction 4.93754ms，其中 rows prepare/order/evaluate/write-publish 为0.88666/0.04132/1.39117/2.21560ms；presentation 2.24432ms（evaluate 1.59497）；extract 1.19922ms；adapter 2.24252ms（module 1.36333、decode 0.35926、convert 0.21931）；begin_tick 0.42541、hash 0.38987ms。每帧10000更新/候选、10001 source lookup。**嵌套区间不能相加，这不是最后修改后的细分。**

在既有World/spatial owner内实施：

- 独占根提交内，按rotation/scale逐bit输入复用线性矩阵和规范化旋转；每行仍处理translation与其逆变换/有限性。父级、相机及变化输入保留原路径。
- 当前presentation pass内复用精确前后rotation/scale/alpha输入的原插值算术；不跨tick/帧保留数值证明。新增history行才复制源ID，已有行借用ID。
- 原RenderProjection在scene没有旧proxy、全部dirty为Transform/RenderState、当前所有非空archetype都是无绘制payload的空间行时，直接消费空投影；不再为每条记录分配seen表、hash ID或查询实体。每次重新检查当前archetype，不缓存“上帧没有renderer”的结论；legacy/dynamic、旧proxy移除及新增renderer仍走原消费者。

19首红在N=16观测实际矩阵/插值计算16次而非1；45空投影首红观测N=1仍hash两次。最终多规模1/100/1000/10000的空投影实际hash/源查询均为0。变换逐位fresh对照、signed zero、四元数反向、父级、anchor/generation及旧proxy移除/再新增反例保持。仅减少实现重复工作，没有删除必要动态选择或GPU工作。

## 3. World：当前无probe原规模结果

最终54单独构建 engine_runtime_host，未混用包含authoring工具的feature组合；54-composition-identity对照08基线均为default+real-wgpu，同一core artifact配置。原Game/包/module未变；使用既有runtime_cli消费者及DLL override，非安装替换。

- 旧DLL：871f6faa79e8f8f3954b61bd3fdd3e6fafd271271e5e06f3d0436126f8fde043。
- 最终DLL：c9c77acf9b6e9e89000a1f83ffcb7ac62a0aa3526e4e38461a7098de5ae047be，保存在证据根54-final-engine.dll。
- Game.exe：8e07ea305d2c51fa7787f8ef86074233ac56ca65cbf30d349a7fc537607e04a8；harness b5bacfca12039225947ba8ac16d406fa2200af93a5c9968eed58fd5b6da90840。

复用的83场景曾把t0-dirtyTarget写成1000，实际模块10000。因此09/37/38的语义结果失败，不能当合格反馈；过程数据保留。44-correct-100只修此预期为10000，不改模块、规模、轨迹或负载。最终55/56预先安排旧/新各一次，120 warmup+600采样，8断言及1截图均通过，Player exit=0；运行期间无并行施工。

| 同原规模、有效场景 | CPU mean/P95/P99 ms | update均值 ms | GPU mean/P95/P99 ms |
| --- | --- | ---: | --- |
| 55 旧基线 | 12.452876 / 13.4850 / 14.7505 | 11.706954 | 0.317484 / 0.384000 / 0.721920 |
| 56 当前最终 | 11.363009 / 12.4038 / 13.6843 | 10.634281 | 0.283436 / 0.386048 / 0.769024 |

以原whole CPU字段判定，均值减少1.089867ms（约8.75%），P95减少1.0812ms；P95≤13.3ms、P99≤16.67ms。没有改用excluding-present字段挑绿。最终renderSubmit均值0.299427ms、presentWait 0.234626ms；auiCpu 0.064423ms是嵌套观测，不能另加。12详细owner profile属于修改前；当前最后源码可直接复用的是56上述上层分段与工作计数，不能拼出虚假的最终World细分。

这是BM01空间配方K0/100% dirty的快速预算反馈，不证明完整K0/K1/dirty矩阵、持续性能或正式轮次，也不能外推受光阴影GPU配方。先前失败不撤销。

## 4. BM03：先分离真实首因

原项目位于前窗pressure/BM03-1-K0，冻结源摘要bae00e74…、10395文件/859541433 bytes，10000份真实mesh JSON。05 SourceInventory诊断：首次83.076s中File::open 69.341s、读0.791s、hash11.714s；其余扫描约13s。保留原双扫/漂移检查、lease逐文件读取、取消和摘要算法。**具体OS/设备/安全扫描原因未确认，不能称SSD迁移修复了首次open。**

通用JSON检查原先构造整棵Value，只为读取四个根字段。现在完整消费JSON语法，仅保留schemaVersion/sourceImage/sourceAudio/sourceCubemap；保留数值溢出、深度、非法字符串、尾随数据、重复键及错误类型行为，真正3D语义仍在原owner验证。64份原网格5465916 bytes：通用解析183.237→151.682ms，保留Value节点2000064→128；仅局部收益，未据此宣称准备已解除。

34无probe优化Release工具仍420.479s超时。41粗阶段探针240.477s停止：open 98.492s、inventory到100.765s、lease到102.399s、digest到102.821s；prepare内部source copy0.804s、source-validation至15.931s，随后assembly直到截止未返回。全程CPU约50.83s，说明不能只归因JSON计算或根据私有内存曲线猜测。

原64网格在同一dev-fast测试二进制的最小Cook：

| 模式 | G路径 wall / publish ms | I路径 wall / publish ms |
| --- | --- | --- |
| 无缓存 | 924 / 0 | 913 / 0 |
| 首次冷缓存 | 5658 / 4563 | 1508 / 515 |
| 热缓存 | 849 / 0 | 834 / 0 |

确认大量冷缓存发布I/O是主要因素之一。未删sync_all、原子rename、内容/来源校验或损坏隔离；通过既有outputParent参数把本次缓存放到I:/gameEngin-build/371/world-prep-20261022-073738/.lit-cook-cache。它不是Cargo target，也不是宿主配置更改。

另修复animator_description::expand：没有任何动画描述时，借用本次不可变CompilerSourceView，不再复制整份约859MB输入或解析全部mesh构造空用的纹理索引。44首红直接检查源backing指针，修复后原字节共享；有描述时仍拥有原派生副本，既有动画规则/拒绝例通过。没有改lease/CompilerSourceView公共所有权协议。

## 5. BM03：此前被超时遮住的输入缺陷

52在新SSD冷缓存下231.375s返回真实错误：texture_decode_failed，Assets/texture-000.png为0字节。61-old-png-audit进一步确认前窗六组BM03输入共576张PNG全部为空。旧源摘要只证明字节相同，不能证明图片有效；这些旧输入不可用于压力资格。前窗实际未到Player，不能把这次发现解释为已通过游戏的运行回归。

因果链：generate-source先open("xb")建立空输出，后调用png producer；前窗21复用函数从最大档当前路径读取，误读刚创建的空文件，随后其它档位复制/复用了它。**属于施工配方/producer缺陷，解码拒绝边界是正确的。**

修复原仓库generator：先完成PNG字节生成，再独占创建输出。57三个Python回归中先有两个确定性首红，修复后全部通过：防止读取自身空文件、编码失败不留下空输出、仍拒绝覆盖已有输入。不放宽解码或提高预算。

58在新目录生成修正1×K0：仅替换64 PNG，10331个其它文件保持原字节、不可变hardlink共享；旧项目/失败保留。独立核对每个PNG的CRC、解压长度、逐行滤波与采样像素公式；纹理0与原冻结generator逐字节一致，2048²/真实像素规模保持。新源摘要fee704a0eeabc550ae5172a958f11b8680b9b719f291f4ba4b943f75f85a464d，原摘要明确失效。

## 6. BM03：当前完整准备已完成，资格仍分开

51无probe优化Release工具SHA256 f3007fa3a3bc45d63bfbc7e2a1ffcaf95d2290d55b86a10c30d0c941d7fa5b72。59只针对修正后的输入执行一次完整Cook-only，沿原BM12 foundation adapter消费BM03静态原规模；没有Compiler导出/模块编译/Player。复用10256个身份未变的mesh/material producer，只生产64个已变化纹理。

| 阶段 | 秒 |
| --- | ---: |
| 外部观察总运行 | 73.740364 |
| driver单调时钟总计 | 72.923167 |
| snapshot | 7.398253 |
| Cook（含下面内部阶段） | 62.053171 |
| source identity/copy | 0.772 |
| source validation | 15.071 |
| glue | 0.001 |
| assembly | 45.046 |
| assembly identity | 1.160 |

嵌套时间不重复相加。实际Cook检查：10000实例、10000独立网格、256材质、64纹理、10000000独立及实例三角形、192000000 vertex bytes、120000000 index bytes、1073741824 baseColor decoded bytes。sourceCook=passed；完整request/prepare/producers/cook/result保存在60-bm03-cook-evidence，原输出I盘59-bm03-corrected-cook及缓存继续可复用。

**73.74s不是与旧420s同冷态的严格A/B**：修正了无效图片，复用了未变缓存，系统open状态也不同；只能报告当前确已完成及此前超时，不能承诺净节省固定分钟。未来继续复用上述缓存与修正源，不再消费旧空PNG配方，也不为文档变化重建工具。

全运行约1Hz外部采样：进程私有内存最大4448919552 bytes（约4.14GiB），系统可用物理内存最低1682018304 bytes（约1.57GiB），**低于4.5GiB原门槛**。采样不是瞬时绝对峰值，也不能冒充owner完整分配；没有扣除工作集伪造达标。正确纹理首次进入1GiB真实解码数据，旧失败未执行这些纹理，峰值不能直接当同输入回归/收益比较。未关闭应用或修改宿主，准备内存与运行时资源资格仍未闭合。

## 7. 验证、失败与构建费用

最终50 World/直接consumer 93项、authoring/缓存/动画/prepare 25项通过；57 Python producer 3项通过，共121项最终有效定向验证。复用初期身份仍有效的数值/代际/父级/取消/队列/退休等证据，不把重复过滤运行累计成新覆盖。rustfmt check和scoped Clippy通过；既有large Err/too_many_args等warning保留，不宣称零warning。没有运行全workspace/完整Compiler/BM03 Player/正式三轮。

真实失败/返工均保留：03初次probe脚本失败却触发无效构建；08同类误启动的无probe构建改标为基线，不能当probe；17原全资产诊断180s超时；34/41有界准备停止；37/38错误dirty断言；44一次未加全名的--exact执行0测试（无效证据），45才取得真实首红；48旧消费者仍要求一次逐实体lookup而新空投影应为0，修正真实工作约束后50终验通过；52真实空PNG错误；57 producer首红。Python脚本引号/字符串插入错误同样发生并修正，不能归咎环境权限。

30合并host与authoring工具的首次Release构建扩大共同feature闭包，耗时1008.969s；随后World必须以单独host组合补建35/54，不能将合并DLL与原单独组合硬比较。这是本窗额外流程成本，后续保持生产host与工具的实际配置边界；先用owner测试收敛再付必要Release，不把本窗总费用当单次增量。

53-current-evidence按独立外层回执统计：构建3984.094s（66.40min），Clippy46.779s，定向测试22.903s，准备诊断340.573s，完整/有界准备966.071s，修正源生成41.450s，Player准备及运行74.894s。失败与返工包含在内；部分fixture准备与构建重叠，**各类不可直接相加当窗口总时长**。Cook内部62s不再加到外部73.74s。实际整个窗口以66-closeout保存的实际起止差计算。

临时prep/World探针已恢复原字节并刷新mtime，临时诊断test移出产品源码；06/13/42及最后源码扫描保存恢复证据。最终54 DLL和51工具均无probe。没有清理被策略拒绝的目标或绕过策略。

## 8. 出口与后续仍沿固定清单

- 本窗World成本修复有真实工作计数与当前原规模预算反馈，CPU P95/P99快速反馈通过；不是完整矩阵或持续资格。
- 本窗BM03准备阻塞已定位并解除：缓存发布I/O、空动画复制及无效PNG producer分别有证据。有效1×K0已经完整Cook，**X3总体仍partial**；其它五组旧输入无效，须按修正generator生成新身份再进入原有规模/轨迹/耗尽和Player要求。不要继续对旧空PNG输入做完整准备。
- X2固定BM11组合、X4未接入backing、X6完整流式/4.5GiB/显示/正式资格沿37审计保留；X5实际工具身份修复证据按未变身份复用。没有新增门槛，也没有把延期写成通过。
- 当前无需推翻3D公开架构来处理这两个首因；准备器峰值仍高，需要依原内存owner区分真实驻留/副本后决定后续，不从一次Cook推出所有权设计完备。

本窗两个重点已取得可用结果，提前收尾，不为凑满三小时扩展其它S1施工。总施工继续保留当前状态，不归档为371完成。
