# 371 §0.31 S1 Windows退出清单与基础消费者补齐

2026-10-21，A路沿codex/255-capability-catalog原dirty工作区。08:42:25 UTC开工，默认3小时；11:27:25停止新增、11:42:25硬截止，历史8小时授权不沿用。实际收尾时刻与墙钟见证据27-closeout.json。Windows only，B路空闲，不改安装、宿主、SDK、Physics、冻结游戏V0.0.5。

**本窗状态：partial / source_consumers_verified / formalQualified=false / installed=false。** 本窗完成可访问内存backing的一组实际补接、BM01/BM09直接生产消费者及固定退出清单；不是第一收尾块、全部S1或整个371完成。暂停扩大S2。

## 固定Windows退出表

| 出口 | 本窗后事实 | 尚欠什么／接收边界 |
| --- | --- | --- |
| 基础空间、batch、取消、packet、cell原子性与真实GPU完成退休 | 既有功能保持；本窗52 foundation、26 loader及GPU双cell消费者通过 | 不重建已有owner，不因观测补接撤销未变绘制算法证据；逐F原规模完整证明仍另列 |
| 可访问backing | Work/Completion嵌套字段、原始/像素读取Vec、cache typed payload/JSON、snapshot纹理描述与view metadata已补接 | decode parser scratch、decoded intermediate、其它World/SDK/renderer/report路径尚未完整接入，不能授全owner覆盖 |
| BM01 foundation | 真实包→Host固定步→10000纯空间对象的0/100/1000/10000 dirty→20真实AUI节点→GPU像素通过 | 当前256²离屏及K1五个keyframe，每key四档共20帧；不是1920×1080、完整K0/K1持续或正式配方；无灯光符合foundation范围 |
| BM09 foundation | 1000对象、0/10km/1000km三锚点、各初始化后20次keep-world重定位、10运动探针、独立位置/投影及GPU图像通过 | 无角色/碰撞/nav声明；未授Release持续性能或完整统一driver资格 |
| BM02/03/08/14 | 历史原包有资格/增量证据；双cell当前直接GPU正确性刷新 | 旧资格保留确切身份，不能授本窗新源Release；按实际失效面更新后续所需资格 |
| BM11/12 foundation | 现有owner负例、双cell ACK/退休和空间1×/1.5×/2×边界证据可用 | 缺原静态/双cell实际故障脚本和完整GPU压力配方；不能把空间数量单测写成BM12已执行 |
| 完整资源/流式性能 | 既有长窗及停顿修复保留历史身份 | 全程分配/峰值、完整stall、系统6GiB及正式独立轮次未闭合；不是本窗已有完整通过而只欠填报告 |
| Windows显示、工具与独立交付 | 既有ETW实现、run/frame/tick/view/origin与capture身份、旧独立项目交付可复用 | 最终OS显示关联/资格与对应工具、独立消费矩阵未核销；本窗无StartTrace尝试，不能称新权限拒绝或采集通过 |
| Android | 用户延期 | 不作为本窗Windows前置，仍不授跨平台S1完成 |

下一继续点固定为**BM01剩余完整轨迹入口与BM11/12 foundation实际故障/压力消费者**，在既有benchmark driver/owner接入，不先重跑Compiler或BM03。通过原规模消费者后再进入完整资源/流式资格；最后核销Windows显示/工具/独立交付。该顺序整理既有S1义务，不新增动画、物理、nav或256cell世界前置。

## 实际产品实现与边界

在runtime_core原owner中增加runtime_asset/memory.rs借用visitor；DecodedAsset缓存与Completion使用同一实际分配定义。覆盖raw/audio、texture描述与Arc<Vec>、mesh/material、Scene/Prefab实体和组件JSON数组/字符串/键，index/mount/diagnostics路径字符串。各Vec按capacity、Arc按真实对象/切片；不以序列化长度、reservation或进程总量替代实际分配。

loader内部opt-in诊断记录沿Work→decode→Completion→cache移动，不改变原bounded channel、预算、ticket、generation或缓存权威。显式capture首次启用后才遍历实际payload并记录身份；Off不构造payload元数据。Work/Completion/读取缓冲诊断lease先注销记录再释放backing；取消、失败、channel drop、poll及关闭走原owner，不保活资产。原worker私有有界Vec与source/pixel ReadBuffer实际capacity可见。首次capture之前已有阶段不回溯扫描；读取Vec→Arc及解码产物转换瞬间未连续覆盖，不能用该采样授全运行峰值。

snapshot补接描述Arc、view target、mesh引用/provenance、纹理描述/像素、当前与previous camera及layers/color pipeline/lights、workset实例表。与loader合并按相同allocation identity去重；共享owner一致、role取并集。Environment沿实际role记录，不把Completion强记成decodedCache。

std channel内部slot、map/set节点、Arc控制块/分配器、wgpu隐式staging与驱动heap仍为API opaque；parser scratch/decoded intermediate及未接入路径单列，不能统称平台不可观测。GPU owner本窗未改，没有新增“完整内存0缺口”声明。私有观测实现没有新增公开schema、全局allocator、加载通道、资源owner或第二生命周期账本。

## 真实消费者与独立参考

BM01由RuntimePackageBuilder生成真实纯空间Scene及AUI manifest，加载后由ProjectRuntimeSession在Host安全点提交变换。固定xorshift32非前缀排列选取四档数量，逐项检查真实SpatialDelta和最终transform。20个红绿HUD经AuiRuntimePresenter→Host UI组合→原生产GPU路径，逐节点中心RGBA和跨相机/dirty完整图像相等。该排列只授确定性数量证明，不冒充完整catalog tick/seed脚本。没有打开尚不支持的统一driver枚举。

BM09由真实包hydration、Host mutation、World Projection、生产render plan和真实wgpu消费。独立f64参考从源网格/镜头常量推导位置与透视屏幕坐标，不从引擎矩阵反推expected；比较三锚点完整GPU图像最大通道差≤1/255，旧snapshot保持不可变，重定位重置current/previous camera历史。最终63帧／63000对象检查，其中3帧初始化、60帧实际重定位，max位置误差0.027832031mm，max屏幕误差0.001572279px；≤1mm/≤0.25px不变。

对应直接源码：
- [内存补接](../../../../rust/crates/engine_runtime_core/src/runtime_asset/memory.rs)
- [loader生命周期](../../../../rust/crates/engine_runtime_core/src/runtime_asset_loader/streaming.rs)
- [内存消费者](../../../../rust/crates/engine_runtime/tests/support/asset_memory.rs)
- [BM01](../../../../rust/crates/engine_runtime/tests/support/foundation_hud.rs)
- [BM09](../../../../rust/crates/engine_runtime/tests/support/foundation_projection.rs)

## 验证、首红与返工

证据根[22-audit.json](../../../../.artifacts/371/s1-closeout-20261021-084225/22-audit.json)及同目录各编号原日志；失败未覆盖。

| 证据 | 实际结果及失效范围 |
| --- | --- |
| 02→03 | Completion真实raw分配与纹理描述对象缺失两项首红→原owner修复后绿 |
| 04、10→11 | 取消/late completion/关闭、深层JSON、Off、snapshot独立描述与共享去重；snapshot缺口首红保留；最终52 passed、5 ignored，产品最终源一致 |
| 12 | 私有loader 26 passed，含真实异步audio/worker gate和ReadBuffer转换/drop/Off；是原重组合--lib，不冒称native快测 |
| 05–09 | BM09初版消费者错误：Off report被unwrap、漏真实纹理准备、keep-world最终值按旧anchor解释错误；修harness，不放宽引擎合同，不fake ACK |
| 14–16 | BM01消费者编译错误及漏AUI manifest导致load后不存在；补正确公开类型/所有权和原manifest后绿；失败全部计时 |
| 18 | 6项真实GPU全部通过：BM01、BM09、两项cell、同handle异值拒绝及大纹理准备；28.28秒外层 |
| 24 | 审核发现旧BM09的20循环含初始化；改成初始化+20次真实重定位，独立刷新1项通过；其它5项18证据复用，不重复全suite |
| 17/19/20/23/25 | 实际core real-wgpu --lib Clippy、core默认check、integration Clippy及所改文件fmt通过；保留既有warning，未加抑制或声称零warning |

有效去重并集84项：52 integration +26私有owner +6 GPU。不是同一最终exe一次执行84项；24仅刷新变动BM09 harness，产品源未变，52/26及18其余5项按身份复用。没有重跑全workspace、Compiler/Cook、BM03、Release、长窗或正式三轮。

原两项cell的8bytes/frame与32byte staging严格visual拒绝、4096可绘制分支保持；本窗未再改配方、生产预算或断言。generation/revision/epoch、取消/迟到、immutable packet、tail/COW、部分上传重试、队列/上传上限、cell原子性、共享pin及真实completion退休保留。

## 系统余量、平台与性能声明

13盘点09:29:38 UTC系统可用5.461GiB，查询cargo/rustc/本路test/Player/brotato无进程；21盘点09:44:08为5.74493GiB并保存top进程working/private，仅为环境观察。引擎测试已退出时仍低于6GiB，说明本窗单点不足不要求引擎运行才能发生；不能因此定量归因旧5.68GiB，更不能倒推历史峰值或相加/扣减working set伪造物理余量。不关闭应用，不改宿主。

13非elevated；logman query -ets成功且列表空，仅为会话盘点。本窗未发起StartTrace/显示采集，不能复述历史policy失败为本窗新失败，也不授OS关联通过。已有windows_etw owner保持，不写新decoder。

本窗没有当前Release CPU/GPU新数据。[27性能记录](27-当前原规模性能与CPU残余成本.md)确切旧DLL cdfc7247…的600样本/7.26秒CPU P95/P99 13.1549/13.9392ms、GPU 9.022464/9.26208ms只保留短窗身份；[29交付](29-普通项目粒子贴花与透明效果交付.md)的DLL3da8db35…也不包含本窗新增观测。不能授当前源、完整K1或正式资源资格。原13.3/16.67ms、6GiB及原规模不变。

## 身份、成本与收尾

开工386项输入与29无漂移；最终原清单10项变化（8产品源+2旧harness），新增产品runtime_asset/memory.rs及3个support harness；22分别保存清单与SHA。387条扩展输入清单含原来的2个测试，不是387条纯产品源码；与5条harness合并去重390项。未改resource_memory reducer、benchmark driver或native GPU owner。

11基础exe SHA AA467CE0…，12私有owner F323BE0E…，18六GPU CC173C40…；最终24 BM09 harness exe 6ECD4858…，完整SHA与配置见各receipt。dev-fast opt-level0，不是Release证据；没有临时probe，无需把旧DLL伪装成本窗产物。

全部失败/返工计入：Cargo测试构建273.675秒（4.56分钟）；默认check 13.048秒；Clippy 22.692秒；测试执行108.991秒；fmt 0.361秒。runner overhead与整个窗口墙钟另见27-closeout，不把私有lib71.33秒构建记成测试执行，不重复相加嵌套runner总时长。工具链1.96.0、locked/-j1、固定I盘target；Compiler/Cook/Player准备与Release为0。

fixture按原Drop清理本路临时包；本窗未对历史policy拒绝目标重试或绕过。末次进程/临时目录盘点见27-closeout。总施工继续保留当前；只同步本路49/54及实际合同，不归档371、不释放为S1已完成。
