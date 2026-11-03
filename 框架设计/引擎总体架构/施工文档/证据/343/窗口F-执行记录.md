# 343 窗口 F：大型负载、局部失效与独立交付

状态：2026-09-15（UTC）Gate F passed。真实活跃规模、完整效果并发、超容量清理、两项隔离失效、完整独立交付均已闭合。现行结论见下方终态及[结构化汇总](../../../../../.artifacts/343-particles/gateF-qualification-summary.json)；早期探索不能替代终态。Provider未安装，宿主配置未修改，338继续暂停未归档。

2026-09-15 02:35 UTC续作预登记：采用施工§7原定Windows release、1280×720、4×4像素无纹理additive、全体可见、120预热+600样本及原预算。既有显式performance开关控制GPU时间戳/小型计数头拷贝，采样结束一次有界读取；默认不采样、不逐帧等待。不新增AI工具。实现/直接消费者测试预计45–70分钟，release构建与负载45–60分钟，局部失效/交付和证据30–40分钟；05:20 UTC后不启动新工作，05:35 UTC硬停止。

旧证据的具体限制：720帧使用dev单次爆炸，多数采样帧效果已结束，不能当零效果或release负载基线；容量测试仅分配、不模拟/绘制，发射器测试各1000容量，总数不固定100k；所谓失效测试实际修改playtest scenario，不是独立参数默认值/WGSL变更；复制dev交付仅B运行74帧，输入脚本在副本外，作者源仍可访问，没有最终build/export+verify与完整文件同一性清单，截图也未完成视觉审阅。以上不能证明P7/P8/P9通过。

续作边界复核：现有Provider/Headless Compiler的正式build只支持windows-dev，F不扩展release导出。性能使用`cargo --release`编译固定Host、Engine DLL和真实Generated Glue/Project Module，以既有`run-native-player --package`直接消费普通Compiler生成且字节未改的RuntimePackage；保留独立二进制/源码/模块身份，仅作release性能组合。曾替换dev交付中的二进制，正式清单正确以`desktop_dev_file_hash_mismatch`拒绝；该失败保留，未重写或删除清单来伪造资格。最终独立交付仍须是正常build的完整原样dev产物。

另一个已确认的F接入缺口：Compiler.verify已有Windowed执行，Provider的现有verify.mode却硬编码只收headless，而HeadlessRhiBackend明确不执行GPU粒子。最小修正限于同一工具现有mode字段向下传递windowed及其schema/说明；默认仍headless、非法值拒绝，保留同一交付静态身份验证和有界进程验收。不新增工具、执行框架或release target；实际窗口verify作为直接consumer终态。

P9准备耗时的最小证据补充：build现有prepareSummary仅有身份/命中数，不能把整个build时间冒称prepare。仅在Provider现有build调用的`prepare_with_artifact_cache`前后计时，向该摘要增加`durationMs`；明确排除refresh、项目/Host构建和export。不新增prepare工具或计时框架，以本轮两次真实隔离修改的返回值直接验收。

## 本轮终态：源码、测量及交付

工作树基于`acaffa8cb95bbb54d093bbf45835a59315c82da5`，含既有未提交修改；本次是实际源码/二进制集成资格，不是clean exact-commit发布。每个计时目录保存`premeasurement.json`、`product-source-hashes.json`、原始Player报告、启动参数/退出码和二进制副本。Run-owned夹具、脚本及失败目录保留在`.artifacts/343-particles`，没有建设永久runner。

最小实现与修复：

- `gpu_frame_measurement.rs`在既有performance开关下请求真实GPU timestamp，记录整帧/模拟/准备/绘制；每发射器仅拷贝48字节计数头和16字节间接参数，不回读粒子数组。计时帧内不map、不等待；结束时一次有界回读。默认不分配采样资源，unsupported/不足样本/超限明确返回。
- `windowed_player.rs`、`runtime_player_winit`将GPU证据传到现有外层报告。计数是实际GPU live/indirect以及最后模拟子步的spawn/drop/collision/invalid，不把容量或CPU实体数冒充吞吐。
- 64发射器真实失败定位到逐发射器/阶段Pass与上传开销：模拟合并Pass、每子步一次参数上传，独立绘制准备/绘制合并Pass；保留命令顺序和重复发射器视图的分隔。真实不等容量测试捕获并修复WGPU late minimum binding size问题。
- 并发烟雾的排序开销定位后，≤64槽使用单工作组排序，较大容量沿原有路径。1/17/36/63/64存活、64/65容量边界、两种视图方向逐像素对照；与大容量排序完全相同。真实混合负载dispatch从424降至264；不减少正式效果层次或并发数。临时`DEBUG-343F`探针已移除。
- Provider只补现有`verify.mode=windowed`传递/schema和`build.prepareSummary.durationMs`。九项独立工具、默认headless、交付身份校验及build/verify解耦保持。

终态去重回归98项：

| 实际命令（`rust`目录） | 结果/编译与执行成本 | 原始日志 |
|---|---|---|
| `cargo test -p engine_runtime --features real-wgpu particle_ -- --nocapture` | 53通过；编译14.02s，执行8.78s | [粒子终态](../../../../../.artifacts/343-particles/gateF-particles-final2-tests.log) |
| `cargo test -p engine_tool_provider --lib --locked -- --nocapture` | 26通过；编译1m45s，执行0.11s | [Provider](../../../../../.artifacts/343-particles/gateF-provider-final-tests.log) |
| `cargo test -p engine_runtime --features real-wgpu --locked wgpu_backend:: -- --nocapture` | 19通过；缓存构建，执行1.86s | [既有绘制消费者](../../../../../.artifacts/343-particles/gateF-backend-consumer-tests.log) |

受影响文件`rustfmt --check`通过，见[格式日志](../../../../../.artifacts/343-particles/gateF-format-check.log)。初期已有真实窗口检查和帧性能消费者单测；它们不重复计入98项。未跑全workspace、Local CI、Android、生产安装或精确提交发布；不属于本轮声明。

## P8：最终空间分布一致的规模矩阵

固定环境：RTX3070 / NVIDIA591.86 / Vulkan，timestamp period 1ns，Windows **cargo release Host + Engine DLL + Project DLL**，普通Compiler的未改字节RuntimePackage。1280×720、正交相机(0,0,10)、halfHeight6.75，4×4像素无纹理additive，固定区域x±10/y±5，全部可见。120预热+600计时样本，Off、不截图。release构建命令：`cargo build -p engine_runtime_host -p runtime_cli --lib --bin ai_project_runtime_player --features runtime_cli/real-window --release --locked`，[日志](../../../../../.artifacts/343-particles/gateF-release-final2-build.log)，1m07s。

正式简单负载只采用`gateF-spatial-*`：同一普通WGSL程序，以每发射器只读`first_id`输入把全局粒子编号分为互不重复的连续区间，1/16/64发射器对应完全相同的10万点集合。此前同种子默认发射器重复空间序列、首个替代hash有条带相关性，经截图审查后降为探索，不用其预算通过冒称分布合格。最终1与64发射器24帧截图SHA均为`69DE6B2B30171E0791F0AE7BDA73213C7996E5D3018B6E7FF38F1C3DE7BF7442`；已打开审阅，均匀覆盖固定区域，无隐藏减负。[最终图像](../../../../../.artifacts/343-particles/gateF-spatial-64-visual/window.png)。这次修正只改run-owned夹具，Engine DLL不变。

| 真实活跃/发射器 | CPU均值/P95 ms | GPU均值/P95 ms | GPU buffer MiB | dispatch/draw | 原始summary |
|---|---|---|---|---|---|
| 0/0 | 1.501 / 2.983 | 0.219 / 0.402 | 0 | 0/0 | [zero](../../../../../.artifacts/343-particles/gateF-qualified-zero/summary.json) |
| 1,000/1 | 1.847 / 3.766 | 0.478 / 0.717 | 0.168 | 5/1 | [1k](../../../../../.artifacts/343-particles/gateF-spatial-1000-1/summary.json) |
| 10,000/1 | 1.894 / 4.139 | 0.565 / 0.840 | 1.728 | 5/1 | [10k](../../../../../.artifacts/343-particles/gateF-spatial-10000-1/summary.json) |
| 100,000/1 | 1.878 / 3.926 | 1.829 / 4.026 | 17.022 | 5/1 | [100k/1](../../../../../.artifacts/343-particles/gateF-spatial-100000-1/summary.json) |
| 100,000/16 | 2.147 / 3.665 | 2.272 / 4.345 | 17.027 | 80/16 | [100k/16](../../../../../.artifacts/343-particles/gateF-spatial-100000-16/summary.json) |
| 100,000/64 | 3.313 / 4.781 | 3.241 / 5.402 | 17.044 | 320/64 | [100k/64](../../../../../.artifacts/343-particles/gateF-spatial-100000-64/summary.json) |
| A/B各8源，16实例/48发射器 | 4.131 / 5.560 | 3.830 / 5.906 | 0.974 | 264/48 | [完整效果](../../../../../.artifacts/343-particles/gateF-qualified-mixed/summary.json) |

全部原预算通过：CPU/GPU各P95≤16.67ms，对同二进制零场景P95增量分别≤4/8ms，100k专用buffer≤128MiB。简单五档全部600帧live和indirect精确等于规定值，drop/invalid均为0。混合负载live平均1703.947、最高2504；241/481帧重播恢复2504，600帧仍1560，只有周期结束归零，并非多数时间空场景。碰撞最后子步最高8、drop/invalid=0。

零效果尾部控制CPU均值/P95=1.670/4.218ms、GPU=0.213/0.392ms，见[尾部控制](../../../../../.artifacts/343-particles/gateF-qualified-zero-end/summary.json)。资格一直使用首个同二进制基线2.983/0.402ms，没有选择更高的尾部CPU基线扩大余量。桌面调度使CPU P95不严格随规模单调；不据小差值给出精确优化百分比，不将CPU/GPU相加或倒数推断显示FPS。

### 真实效果成本、可见覆盖与回收

以下只用于成本观察，不用关闭特性的变体替代完整效果资格。相机/16源/时长保持；每次只移除A的一类开销。B保留其作者世界坐标公式，8实例在画面中心重叠；A按8个源位置分布，未把B移出画面降低成本。[完整画面](../../../../../.artifacts/343-particles/gateF-mixed-visual/window.png)与[无烟雾画面](../../../../../.artifacts/343-particles/gateF-no-smoke-visual/window.png)已打开对照；第24帧烟雾改变41814像素，占窗口4.537%（任一RGB差>2），是可见贡献并集，不是总overdraw。

| 条件 | CPU均值/P95 ms | GPU均值/P95 ms | 模拟/准备/绘制GPU均值 ms | 结构性观察 |
|---|---|---|---|---|
| 完整 | 4.131/5.560 | 3.830/5.906 | 2.010/0.796/0.688 | 264 dispatch，48 draw |
| [无烟雾](../../../../../.artifacts/343-particles/gateF-qualified-no-smoke/summary.json) | 3.812/6.187 | 3.464/5.676 | 1.919/0.509/0.687 | 216 dispatch，40 draw；准备成本下降 |
| [无拖尾](../../../../../.artifacts/343-particles/gateF-qualified-no-trail/summary.json) | 4.039/5.821 | 3.893/5.981 | 1.942/0.862/0.728 | 少8个history dispatch、82816字节buffer；总GPU时间未测出明确改善 |
| [无碰撞及其子发射](../../../../../.artifacts/343-particles/gateF-qualified-no-collision/summary.json) | 4.062/5.606 | 3.954/5.887 | 2.026/0.845/0.732 | collision由最高8降到0；此规模的时间差处于波动内 |

不把负的小差值解释为关闭功能反而更慢的规律，也不为找毫秒以下差异追加测试。结构计数、真实画面与原始计时同时保留。

真实GPU owner测试`particle_render_measurement_actual_gpu_counts_timestamps_clear_and_retirement`提交200k请求/100k容量：精确100k live/indirect/spawn、100k dropped，下步drop=0；clear后live/indirect=0；提交后不等待即移除实例，随后一次有界完成读回确认专用buffer=0。在途引用由WGPU保留到提交完成，不声称驱动已立即归还物理显存或完全无泄漏。

内存边界单列：上述专用buffer包含records、scratch、event、parameters/step、indirect/sort/history；不含Renderer深度纹理（1280×720×4=3,686,400字节）、Surface/背景/字体/材质纹理、mesh、pipeline/驱动缓存。显式测量结果缓冲为12,288,000字节，结束读回另需同量临时buffer；默认不分配，未计入粒子专用buffer预算。基础Renderer资源不随单个效果clear销毁。

## P9：两项隔离修改

独立原样项目副本内分别修改B的radius默认值1.65→1.75，恢复描述后只改WGSL爆开速度2.0→2.25。原作者项目未改；副本最后恢复原始字节。[完整身份/准备记录](../../../../../.artifacts/343-particles/gateF-p9-summary.json)。

| 条件 | Compiler准备秒 | 完整build调用秒 | A/B Cooked身份 | DLL |
|---|---|---|---|---|
| 副本首次 | 4.310 | 23.755 | 基准 | 基准 |
| 参数默认值 | 3.807 | 13.353 | A不变/B变化 | Engine及Project均不变 |
| 隔离WGSL | 3.807 | 13.174 | A不变/B变化 | Engine及Project均不变 |

三次Engine DLL SHA均为`B72A7F845A1D832015EA9444FF57DDC6DF12AF6D29384B8BECF6053A9E8834D6`，项目DLL均为`A6642C04C50F19A61918D144B8B3D5A8BD8B62069D08C17978B6D2C330736EF2`，固定Host缓存为hit。准备时间包含真实Compiler校验与装配，不含refresh/check工具、Host/Project构建或export。未变Cooked字节不等于所有准备逻辑都跳过，不宣称完整粒子Cook已有独立缓存。

## P7：原样交付与最终视觉复验

[最终正常build](../../../../../.artifacts/343-particles/gateF-final-project-build/build.json)与[唯一一次最终verify](../../../../../.artifacts/343-particles/gateF-final-project-build/verify.json)成功；verify为windowed、24帧、真实捕获、子进程与Player退出均0。deliveryIdentity=`sha256:d57f72fca9b23ba8e2c89e3f040f461e7ca62a3b63ecbe5813d4228bcc4597fc`，Windows dev，不是release导出。

完整46文件复制至`.artifacts/343-particles/gateF-independent-final/delivery`。保存[原始清单](../../../../../.artifacts/343-particles/gateF-independent-final/reports/original-files.json)和[复制清单](../../../../../.artifacts/343-particles/gateF-independent-final/reports/copied-files.json)，逐文件路径/长度/SHA完全相同，运行前后未改seal、二进制或包。确认原项目无运行进程后，临时将确切作者目录改名，使原路径不可访问；副本输入/报告也留在run-owned根内。A运行24帧、B运行74帧，均退出0、真实窗口和Engine DLL owner通过。[执行快照](../../../../../.artifacts/343-particles/gateF-independent-final/reports/summary.json)保留审阅前状态；现行[视觉审阅](../../../../../.artifacts/343-particles/gateF-independent-final/reports/visual-review.json)为passed。finally已恢复源目录，见[恢复结果](../../../../../.artifacts/343-particles/gateF-independent-final/reports/restoration.json)。

- [A实际截图](../../../../../.artifacts/343-particles/gateF-independent-final/reports/a/window.png)：上扬橙色火花、短拖尾、三角碎片、半透明烟雾可辨，背景网格清晰；与正常verify截图SHA完全相同（`3435D1D7DC40A8A3C2A3DD84BD399507FDCFCE314AEFF5E7A3B56C6AF0C27444`）。
- [B实际截图](../../../../../.artifacts/343-particles/gateF-independent-final/reports/b/window.png)：蓝色螺旋上升与橙色点缀符合E最终版本，无A残留。完整阶段变化、两轮创作与各5次重播复用E证据；本轮补最终二进制/交付可见消费，不把两张帧图冒称全部时间线重测。

release性能Engine DLL=`F13CDE16D508F318304BE8FCC1CF69BDBA85F480A49F19C1F73525BB0B7C9279`、Host=`C5C18B520ED195384A4E3726CB196993D51FCF8247C04B7CCFB530F39236F6E6`。独立dev交付Engine DLL=`B72A7F…8834D6`、Host=`4904274E…9894D6`、项目DLL=`D229C583…40E074`；全值见46文件清单。源码Provider=`BF5C5EDF73BFD23C30298F697D6D464134166CBD273B3BE9B28DB3ED35A1DE53`，仅独立stdio执行，未安装。

## 失败、修正与证据失效边界

收尾自审：从2026-09-15 02:35 UTC续作至04:17 UTC，约1小时42分，未触及三小时窗口。关键源码与最终性能快照无漂移，受影响文档链接检查无缺失；最终检查另发现正式方案页首仍指向旧`当前/`位置，已修正为归档位置。作者目录恢复、临时隐藏目录不存在、run-owned进程为零，见[收尾审计](../../../../../.artifacts/343-particles/gateF-final-audit.json)。未清理或删除测试证据、共享缓存、用户数据，未修改安装配置。必要性复核：所有修复均对应实际失败或P7–P9证据要求；未引入新的工具/框架/平台Gate，P1–P9完成后不再启动附加验证。

- [缺COPY_SRC的真实GPU红测](../../../../../.artifacts/343-particles/gateF-measurement-tests-3.log)→加入间接计数拷贝所需usage；后续53项包含绿测。早期两次测试夹具编译失败保留`gateF-measurement-tests-1/2.log`。
- [64不等容量绑定红测](../../../../../.artifacts/343-particles/gateF-64-owner-2.log)→每发射器明确实际binding minimum；`gateF-batch-layout-tests.log`及最终53项通过。真实崩溃stderr保留在`gateF-release-64-batched-compute`。
- [windowed verify红测](../../../../../.artifacts/343-particles/gateF-windowed-verify-red.log)→现有模式向下传递；26项及最终真实verify通过。
- CPU预算曾失败：`gateF-release-simple-100k-64`的8.388ms、`gateF-release-64-batch-layout`的6.414ms、`gateF-release-64-batched-draw`的5.858ms；真实完整混合`gateF-mixed-second`为8.245ms。全部原始报告保留，没有放宽阈值。上述旧简单负载重复空间序列，不与最终spatial夹具作严格优化百分比比较。
- 一次release CLI漏real-window feature（`gateF-final-zero`）、一次输入脚本0起始帧（`gateF-mixed-first`）均非产品通过证据，修正调用后使用新目录；不覆盖首败。第一次release模块复制缺RuntimeModule与替换dev seal的失败也保留。
- 新增/修正源码使对应旧二进制计时失效，才重建重测；最终空间夹具修正只重做简单负载，复用同二进制零场景、完整混合、P9与交付证据。没有机械重跑A–E全套。

## 早期尝试（历史，不用于终态）

下列固定条件是旧Windows dev探索，与上方release性能资格分开。NVIDIA GeForce RTX 3070、驱动591.86、Vulkan、1280×720，720帧中单次爆炸多数已结束。

当时保存的探索证据：

- [720帧性能报告](../../../../../.artifacts/343-particles/gateF-baseline/player-report.json)：CPU均值7.513ms、P95 8.798ms、P99 10.447ms；update均值2.551/P95 3.263ms，render_submit均值4.813/P95 5.741ms，present_wait均值0.131/P95 0.175ms。报告当前未填粒子 live、dispatch/draw、buffer bytes 字段，不能转换为 P8 规模资格。
- [容量红测](../../../../../.artifacts/343-particles/gateF-capacity-test.log)首次证明 `maxEventsPerStep=100000` 超过65536上限；调整到合法上限后的[绿测](../../../../../.artifacts/343-particles/gateF-capacity-test-green.log)在真实 RTX3070/Vulkan 上创建1k、10k、100k容量GPU记录、scratch、事件和参数资源，均低于128MiB。
- [发射器边界测试](../../../../../.artifacts/343-particles/gateF-emitter-count-green.log)通过1/16/64发射器、每发射器1000容量的真实GPU资源创建；首次含连字符名称的失败日志被后续绿测覆盖，只有会话输出保留，不能声称该文件保留首败。
- [场景digest回归](../../../../../.artifacts/343-particles/gateF-invalidation-green.log)验证playtest场景源变化获得新digest且prepare不强制playtest；E的q轮次Cooked身份保留作历史，但不能替代F要求的两项隔离变更/耗时/DLL验证。
- [独立复制运行](../../../../../.artifacts/343-particles/gateF-independent-delivery/gateF-run/launch.json)把最终dev交付复制到独立目录，运行只引用复制目录，74/74帧、退出码0、截图captured、Vulkan和Engine Runtime DLL owner通过；对应[Player报告](../../../../../.artifacts/343-particles/gateF-independent-delivery/gateF-run/player-report.json)。

当时的P8缺口：Native Player未填实际粒子计数，样例单次仅169/192容量，尚需真实活跃与规模测量。此缺口已由上方本轮终态闭合；旧entityCount等空字段本身仍不作为粒子吞吐指标。

历史窗口当时为`p8_not_qualified`，不覆盖页首现行结论。所有失败输入、复制产物、报告和缓存保留，未删除用户数据。
