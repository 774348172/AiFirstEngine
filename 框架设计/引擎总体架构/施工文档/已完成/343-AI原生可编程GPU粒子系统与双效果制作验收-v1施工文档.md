# 343 AI 原生可编程 GPU 粒子系统与双效果制作验收 v1 施工文档

状态：2026-09-15 窗口A–F及P1–P9全部通过，343已完成归档。真实均匀活跃负载、16效果并发、GPU计时/计数、局部失效、完整独立交付均闭合，见[F终态证据](../证据/343/窗口F-执行记录.md)及[完成记录](../../阶段完成记录/2026-09-15-343-AI-Native-GPU-Particles-v1/00-总览.md)。旧dev计时、容量分配、scenario摘要及早期复制运行仅保留历史身份。338继续暂停未归档；Provider未安装，本文不授权修改宿主配置。

依据：[343正式方案B](../../343-AI原生可编程GPU粒子系统与双效果制作验收-v1方案.md)、顶层产品/架构v1.1、[49基线](../../49-当前状态与下一步入口.md)、[54唯一施工槽](../../54-当前可自动化施工总入口.md)。343整体只有P1–P9全部通过才完成；单个Gate通过不得归档。

## 1. 目标、必要性与验证经济性自审

- 请求的结果：AI通过效果描述和真实WGSL函数创作大型GPU粒子效果，实际制作两例，各修改两轮并复看、独立交付。不把人工节点编辑器或项目Sprite循环当作完成。
- 最小责任链：Compiler从既有不可变快照校验/派生，普通资产进入唯一RuntimePackage；Runtime控制实例与生命周期；RenderProjection、RHI/WGPU托管模拟/绘制；公开SDK提交意图；既有playtest/observe留证。没有新增AI工具、MCP聚合器、Compiler、DLL、同步总线或缓存服务。
- 激活前缺口（历史基线）：ParticlePayload只有effect_ref；RHI无粒子计算与实例绘制合同；后端逐命令创建顶点缓冲。必须补真实执行，不能只扩schema；当前完成范围见页首及分窗口证据。
- 最小涉及文件：`rust/crates/engine_runtime/src/particle_effect.rs`及粒子子文件（数据/运行）；`project_authoring_execution/src/particle_effect_cook.rs`、`neutral_assembler.rs`与Cargo依赖（Naga复用wgpu26版本）；既有`runtime_package*`、`render_state`、`projection`、`rhi_command_plan`、`engine_rhi`、`wgpu_backend`、`project_runtime_session/native_adapter`的实际接入点；`project_game_sdk`及已有ABI consumer；`runtime_player_winit`观察/捕获消费端；新普通样例`samples/particle_effects_project`。仅在实际需要时改共享合同，不一次性重写这些模块。
- 排除：资源复用库、节点UI、生成AI、新语言、任意Render Pass、通用GPU框架、流体/邻域求解、全CPU镜像、Android资格、全workspace/Local CI、发布精确提交、生产安装。338变体和鼠标验收保持暂停，不借343扩游戏范围。
- 复用：339颜色、340字体、341动画、342音频和337稳定Host的已有源码/交付证据；消费它们的原输入未变时不重跑。新增粒子shader/场景/SDK或DLL变化只更新实际消费者证据；既有安装身份不是343运行证据。
- 成本：这是跨编译器、GPU、渲染及真实创作的新系统，整体估算10–16小时，不能声称一轮三小时全部完成。先执行窗口A（Gate A，预计90–140分钟含审查/测试），后续窗口按依赖连续推进。每次2小时45分不启动新工作，3小时停止并保存证据；任何Gate超过估算先复核成本与失败首因，不扩通用基础设施。

| Gate | 风险/失败合同 | 最便宜的可变红检查 | 终态证据/估算 |
|---|---|---|---|
| A 作者合同与Cook | 丢字段/引用、假WGSL、手写binding、失效摘要漏代码、最终包缺字节 | 类型校验、真实Naga解析/验证、不可变快照、源行定位、包写入/读取与摘要敏感性测试 | 源码/命令/原始测试输出；90–140分钟 |
| B GPU模拟 | 作者函数没执行、二次积分、越界、重复模拟/发射、事件无界 | 真GPU已知输入有界回读，对位置/年龄/自定义状态/计数断言 | GPU身份、容差、读回结果；120–180分钟，可分两窗口 |
| C 正式绘制 | CPU逐粒子伪装、透明/朝向/网格/拖尾错误 | 同一WGPU路径的实际图像和最小像素/深度检查 | 实例/间接绘制、真实相机帧；120–180分钟 |
| D SDK与生命周期 | 失败/NoOp仍执行、旧实体命令串代、暂停追赶、释放在途buffer | SDK/adapter owner与直接Scene/session consumer测试、GPU退役检查 | 命令及生命周期证据；60–120分钟 |
| E AI双效果创作 | 演示代替制作，修改未进入GPU，美术问题被计数掩盖 | 两例首版+各两轮指定修改，真实时间帧复看，5次重播 | 源差异、6版关键帧、逐项视觉判定和用时；120–180分钟 |
| F 规模与独立交付 | 只有容量无活跃、错误计时、导出依赖源目录、改效果重编DLL | 预登记负载、真实GPU计时/活跃数、同最终包独立运行、局部失效两例 | P1–P9总表、性能与独立交付；60–120分钟 |

## 2. 固定作者与执行合同

- 作者文件为`Assets/**/*.particle-effect.json`，`schemaVersion: particle-effect.v1`；行为为项目内`*.particle.wgsl`。资产显式`assetId`/`assetGuid`，纹理/材质/网格复用AssetRef（id/type/guid）；不要求额外手写同义.asset。
- 类型真相放在效果描述：命名发射器、秒/米/弧度、右手坐标Y向上、local/world、固定种子、显式容量/包围体/质量/离屏策略；命名参数有类型/默认值/范围及作用阶段。未知字段、重复名、非有限数、坏资源和非法范围报作者位置。
- 引擎派生`Particle`、`EffectParams`、`ParticleContext`及声明的只读输入/自定义状态。作者提供`particle_init`/`particle_update`普通WGSL函数，只读上下文、只修改返回的自身粒子；无需绑定号/dispatch。Naga负责语法与类型，结构校验拒绝项目entry point、资源绑定、可写全局存储；不能靠字符串黑名单把数学编程封闭化。
- 基础初始化→有序标准初始化→作者初始化；每步年龄/步长→有序标准更新（默认显式积分）→作者更新→碰撞/有界事件→存活回收。自定义接管位置时禁用标准积分；以真GPU测试检查，不能只测生成文本。
- Cooked效果资产保留编译合同、源身份、结构化数据、经过校验的WGSL行为函数库和作者源映射，使用已有asset payload/dependency/hash入包。GPU入口由B接入；Gate A先证明可准备与入包，播放接入在B–D，不在A冒称运行可用。
- `ParticleEffect`组件纯数据；SDK意图play/restart/stop-emitting/clear/pause/resume/set-parameter经现有成功提交通道。play幂等，restart从零；暂停不追赶；清理同时清事件；每源有实例/世代，不累积无限实例。具体Rust命名按既有audio/Animator consumer风格落地。
- GPU持久缓冲、存活索引/计数、需要时排序及间接/实例绘制；不逐颗ECS或Sprite、不默认每帧回读。测试回读有界、运行报告Off/Summary/Trace沿现有规则。

## 3. Gate A：作者合同、真实WGSL与包内数据

任务：typed schema及校验；Naga编译和作者行定位；引用与依赖；既有Assembler入包；描述/程序/资源/合同的摘要失效。首轮不提前搭建复杂GPU框架或制作最终案例。

测试（以下Cargo命令在`G:/gameEngin/rust`执行）：

```powershell
cargo test -p engine_runtime --locked particle_effect -- --nocapture
cargo test -p project_authoring_execution --locked particle_effect -- --nocapture
```

定向集至少覆盖：最小默认效果、多发射器/曲线/参数/绘制配置的roundtrip；未知字段/重复身份/越界/非有限数；资源缺失/类型与GUID不匹配；路径逃逸/缺行为文件；正常WGSL数学与参数类型生成；语法错/类型错映射真实作者行；项目binding/entry point拒绝；不可变快照不重开源；代码/默认值/引用资源变化改变对应Cooked身份，未变效果字节保持；普通RuntimePackage builder/load保留实际WGSL字节。必要新增依赖仅Naga26的wgsl-in；锁文件只更新相关包依赖。

验收A不等于P2/P3通过：实际作者函数执行要等B，修改效果免重编DLL证据要等F。执行结束保存源码路径、真实命令/耗时、失败首因和结果，不创建“343全部完成”记录。

## 4. Gate B/C：GPU执行与渲染

B：批量发射/回收、rate/burst/delay/loop、固定种子、形状/速度/重力/阻力/噪声/曲线、作者函数、自定义状态、plane/sphere碰撞、事件/子发射上限。实现显式模拟步去重和有界追赶；容量满时丢弃计数确定；非支持后端明确unsupported。

Gate B激活复核（2026-09-14 22:27 Asia/Shanghai）：A源码与当前入口一致；现有RealWgpuBackend可复用设备/队列，但offscreen此前只请求WebGL级limits。按实际设备计算能力请求所需limits，不满足时明确拒绝粒子；不创建第二设备或CPU镜像。B新增粒子内部shader派生与GPU资源owner，Compiler将真正compute入口写入既有Cooked资产。GPU按update/free-list→上一模拟步事件路由→spawn→live/indirect计数执行；新生粒子age=0，下步再更新，事件双缓冲令子发射下一步生效。只读参数与自定义状态沿A类型生成；标准积分与自定义接管互斥。累计时间分成最多8个不超过1/60秒子步，余量记为丢弃时间；相同/过期stepId不重复执行。B只完成GPU owner与现有Backend接入，RenderGraph绘制与SDK/session消费者仍属C/D。

B实现细化：`Particle.emit_children`为已声明childEmission规则的位掩码，由引擎每次update前清零，作者可在init/update请求；规则目标/数量/最大代数及总预算仍由描述约束。普通rate保留跨loop的小数量，burst按周期重放；质量缩放作用于累计发射量后做差。容量/事件饱和的丢弃数量有界且可核对，不保证并发原子争用下被接收事件的身份顺序。碰撞前后均检查有限状态；无效粒子不进入事件/live索引。创建shader/pipeline可同步校验，常规simulate无GPU等待或回读；C需接入当前持久缓冲，不能另建CPU模拟。

C：沿既有RenderGraph/RHI实现持久资源及dispatch→排序/实例准备→draw依赖；2D/3D billboard、flipbook、mesh、trail、alpha/additive和需要时深度排序。固定相机/遮挡物检查空间方向和排序；容量不变时不重建整批buffer，默认无GPU wait/readback。

C激活复核：复用B工作树与真实GPU证据。现有后端缺粒子draw和深度attachment；最小新增typed粒子模拟/绘制命令、相机数据、持久绘制索引/拖尾记录，WGPU直接读取B的records/scratch并间接绘制。纹理复用既有注册，网格仅增加一次上传的三角形数据消费，不重构全场景Mesh系统。拖尾在模拟子步记录，重复Present不补假历史；每发射器按视图做GPU深度排序，跨draw维持Graph提交顺序。真实GPU离屏帧沿正式render_to_view捕获；用固定相机、贴图上下标记、前后粒子、遮挡三角形、非对称网格和弯曲拖尾建立可变红像素检查。C不提前实现SDK/session控制、双效果制作或性能资格；本窗口18:02 UTC停止新工作、18:17 UTC硬停止。

```powershell
cargo test -p engine_runtime --features real-wgpu --locked particle_gpu -- --nocapture
cargo test -p project_authoring_execution --features real-wgpu --locked particle_effect -- --nocapture
cargo test -p engine_runtime --features real-wgpu --locked particle_render -- --nocapture
cargo check -p runtime_player_winit --features real-window --locked
```

新测试的具体case名在实现时写入执行记录；GPU适配器获取失败视为该资格未完成，不返回空成功。已知输入不超过64粒子/8步，位置速度绝对误差先定1e-4、年龄1e-5，计数精确；保留不执行自定义函数/重复积分时应失败的反例。单次探索超过20分钟无定位进展回到责任层，不增加另一套runner。

## 5. Gate D：项目控制和现有消费者

2026-09-15 00:24（Asia/Shanghai）激活D：沿现有成功提交输出添加粒子意图，复用实体runtime generation；CPU只保存每源控制/包内资产和投影快照，GPU资源仍由WGPU拥有。RenderScene→Graph/RHI承载不可变粒子资源与控制快照，现有后台/窗口共同消费，不新增AI工具或独立同步总线。材质/网格现有producer只保存数量摘要，没有真实顶点/参数字节；本Gate仅补普通包内资产的真实三角形位置/UV/索引及unlit颜色/纹理消费，拒绝不支持的材质，不建立通用材质图或模型导入系统。验证以SDK/成功提交owner、实际包加载/Scene卸载和同GPU渲染消费者为终态。预计60–120分钟；19:09 UTC停止新工作、19:24 UTC硬停止。E/F和安装不在本轮。

完成SDK、ABI实际受影响consumer、Hydration/RenderProjection、session卸载和资源退役。接续C现有纹理/网格注册与绘制合同，完成包内AssetRef到资源绑定的消费；C对尚未解析的可选material明确报错，D不得忽略该引用或另建材质系统。覆盖play重复、restart暂停时、stop与clear区别、参数作用阶段、local/world、entity同名重建/旧世代、组件移除、Scene实例卸载、session退出、NoOp/failed handler不提交、成功只消费一次。两个真实效果各5次重播在E复用，不在D重复真实窗口。

```powershell
cargo test -p project_game_sdk --locked particle -- --nocapture
cargo test -p engine_runtime --locked particle -- --nocapture
cargo test -p engine_tool_provider --locked particle -- --nocapture
```

Provider没有生产改动时最后一项以现有独立check/run/playtest/observe集成消费代替，不为了测试名添加工具或测试报告层。共享ABI变更后补直接consumer编译，命令及feature写入记录。粒子参数不为每种效果增加API。

D落地合同与终态命令以[D执行记录](../证据/343/窗口D-执行记录.md)为准：typed组件为`engine.particle_effect`，`effectRef / playOnAwake / paused / parameters`；公开SDK为`writer.particle_effect(&entity)`，参数沿描述中的Float/Uint/Vec3/Vec4类型。普通mesh/material `.asset`在`runtimeData`中写真实`positions/uvs/indices`或unlit `baseColor/texture`，Assembler提取纯payload并验证纹理引用。D的Runtime终态使用`real-wgpu`及受影响域联合过滤器；Provider以现有`switch_puzzle_semantic_outcome_uses_its_own_bool_contract`真实playtest/observe消费替代无生产改动的粒子工具测试。提交后不等待的GPU退役和随后共享给Sprite的纹理保留均有实际测试；不把截图helper先回读后的清理当作在途证据。

## 6. Gate E：两个效果的制作、两轮修改和复看

2026-09-15 05:25:47 Asia/Shanghai 激活E，先复核D源码与现有Provider/窗口消费者。使用独立源码Provider绑定普通粒子样例；预登记见[窗口E制作预登记](../证据/343/窗口E-制作预登记.md)。08:10:47停止新工作，08:25:47硬停止。本轮不进入F、安装或宿主配置变更。

项目`samples/particle_effects_project`是普通公开SDK消费者。先记录任务、模型/宿主、源摘要、相机、种子42、时间线、评价标准；项目UI只提供选择、播放、暂停、重播。纹理按普通资源制作/导入，不建设资源库。

| 案例 | 首版与时点 | 修改1 | 修改2 | 必须判定 |
|---|---|---|---|---|
| A 火花烟雾爆炸 | 火花/烟雾flipbook/网格碎片，少量拖尾/碰撞子火花；.10/.35/.80/1.60/3.20秒 | 火花偏上，保留烟雾 | 更快更淡的烟雾，保留火花方向 | 各层可辨、烟雾连续淡出且不随源移动、背景可辨、碎片符合重力、3.2秒清空 |
| B 吸附螺旋爆开 | 自定义WGSL；0–.8吸附、.8–2.2螺旋上升、2.2爆开、4秒清空；.20/.70/1.20/1.90/2.35/3/4秒及相邻帧 | 反转旋转、扩大半径 | 实际修改WGSL，高度决定爆开速度并改变时机 | 轨迹连续、方向/上升/阶段正确、高度速度相关、自定义代码确被GPU执行、最终消失 |

每例首版→check/run→打开真实帧审阅→记录预期修改→改源→同条件复跑/对照；两轮都保留前后diff、Cooked身份、相同时间点及新阶段边沿帧。每个判断引用时间帧；坏方向/跳变/遮挡/残影必须修复。不得用计数/截图文件存在代替视觉评审。

现有Provider独立进程绑定本项目，经九项现有工具check/run/playtest/observe；复用仓库既有进程调用方式，首次运行记录准确命令、二进制/profile及请求，不猜测CLI参数。项目Rust测试用`cargo test --manifest-path samples/particle_effects_project/RuntimeModule/Cargo.toml`。窗口和语义消费同最终包；每例5次重播、暂停/恢复、重复触发、退出无残留。

记录首次可见用时及两轮修改→复看耗时，分开准备/编译/运行/审阅；人工干预、错误/修复次数、作者文件数、是否被迫改Engine源码。无同条件对照不声称快于Unity/Godot。

E终态补充：正式方案7.2/7.3要求的flipbook与螺旋上升已纳入表格，早期v系列因遗漏不计最终资格；q系列完整重做首版和两轮修改。最终两例各5次重播，A1261tick/B1561tick及同包普通窗口通过。相机裁剪、负平面WGSL和独立playtest额外headless预检三项实际缺陷在既有责任层修复。命令、逐轮图像、时间/失败边界和同包身份见[E执行记录](../证据/343/窗口E-执行记录.md)，不将本E结果替代F。

## 7. Gate F：预登记性能、局部失效和独立交付

测试前记录GPU型号/驱动、后端、timestamp能力和源/二进制身份。采用Windows release、1280×720、固定正交相机、简单无纹理4×4像素additive面片、全体在视锥内、均匀分布固定屏幕区域；不以藏粒子降低成本。预热120帧、计时600帧，不含截图/重Trace。设备不支持GPU计时则明确该项未完成，不以CPU提交耗时代替。

预先预算：总帧CPU和GPU各P95≤16.67ms；相对同场景无效果增量CPU P95≤4ms、GPU P95≤8ms；100k粒子专用GPU buffer≤128MiB。CPU和GPU不直接相加推断显示FPS。预算失败保留P8失败并按瓶颈定向修复，不事后放宽；若需要改变已批准架构再讨论。

- 固定1/10/100千真实活跃；随后100千固定总数，1/16/64发射器（1发射器点复用）。报告实际活跃、均值/P95、buffer bytes、draw/dispatch及丢弃数。
- 两真实效果各同时8源（共16），同分辨率相机固定位置；按同预算计时，分别报告烟雾覆盖、拖尾/碰撞和混合成本，不能用简单面片结果代替。
- 受控200千请求/100千容量，确认有界丢弃；clear/退出后实例归零、在途帧完成后粒子buffer退役，基础Renderer持久资源单列。
- 参数默认值修改和WGSL代码修改各一例，记录准备耗时/产物失效及Engine DLL SHA不变；不新增prepare工具。
- 最终版本只做一次正常build/export和verify。完整文件原样复制至独立run-owned目录，运行时屏蔽原作者源路径；两效果真实可见，记录包/模块/程序/资源摘要及关键帧。不改装生产Provider。

## 8. 自审、激活与完成规则

2026-09-14方案复审：无需修改343技术方向。方案中“本次只生成方案”的时间性状态需按新授权更新；没有待处理的用户指定外部审查。既有研究中的GPU执行、AI双源不重复维护、真实视觉/规模证据已落实。

施工文档自审：A–F覆盖P1–P9及功能表；A没有用Cook通过冒称运行；B含真实GPU反例；E包含两轮实际修改；F预算提前固定。无新AI工具/通用框架/资源库/全平台要求。总量超三小时已分窗口，首个窗口以A为完整交付单元，不能为了“直接开始”虚报全部完成。

激活复核：确认338未完成、343批准及待执行文档存在、无并发源码施工；保留dirty worktree，只改上述责任点；338移历史暂停后343移当前，同步49/54/地图/施工索引。后续每个窗口从实际证据续作。

最终P1–P9逐项绑定证据；全部通过后才生成阶段完成记录、将本文移已完成并同步49/54及完成索引。源码、双效果制作、视觉、规模、独立交付和安装六种状态分别报告。未达标准保持当前未完成，338不被连带归档。

## 9. 执行记录

2026-09-14：待执行文档自审后，核对CompilerSourceView、普通asset payload、Naga26依赖和粒子占位基线，通过激活复核。338暂停未归档，343激活；无cargo/rustc并发进程。窗口A开始，尚无源码、GPU或试玩通过声明。

2026-09-14窗口A终态：Gate A passed。typed描述/有界校验、参数与状态类型生成、真实Naga26校验/作者定位、Compiler check/prepare、普通资产入包和局部源依赖摘要已实现。9项Runtime合同、14项Cooker、1项正式快照消费、4项既有Compiler消费者回归，共28项去重通过。首次未知字段吞掉缺陷和包夹具漏InputMapping的失败/修复均保留于[窗口A执行证据](../证据/343/窗口A-执行记录.md)。源码自审通过，无架构方向变更；新增Naga只在作者Compiler，Runtime未新增反向依赖或AI工具。B/C/D/E/F未执行；真实GPU函数执行、效果制作两轮修改、规模及独立可见交付仍为必需完成条件。未安装Provider。

2026-09-14窗口B终态：Gate B passed。复用现有WGPU设备/队列实现持久GPU模拟与有界事件；Compiler真正compute产物经普通payload被Backend消费。RTX3070/Vulkan实际回读证明作者init/update、typed状态/只读输入、位置积分与自定义接管、发射/回收、标准行为/碰撞/子发射、step去重及暂停/清理。两个真实变异程序由同一oracle识别；循环rate小数和碰撞后非有限状态已定点修复。12项GPU owner+9项合同+1项粒子纹理producer+15项Cooker（含GPU consumer）+1项快照集成+1项既有offscreen draw回归，共39项去重测试通过；`runtime_player_winit --features real-window`检查通过。详见[窗口B执行证据](../证据/343/窗口B-执行记录.md)。源码自审未扩大框架/工具边界；未执行C–F、未打开粒子试玩窗口、未安装Provider，不声称视觉/规模/独立交付或343整体完成。下一窗口Gate C将这些GPU资源接入正式RenderGraph/RHI绘制。

2026-09-14窗口C终态：Gate C passed。RenderGraph/RHI正式命令直接消费持久GPU模拟结果并间接绘制，完成2D/3D billboard、真实透视、flipbook、上传mesh、拖尾、Alpha/Additive、每发射器深度排序与固定遮挡检查。修复同种子槽位复用继承旧拖尾和纹理采样器覆盖两项实际缺陷，保留原始失败与复验。34项Runtime粒子相关+16项Compiler相关+21项既有Graph/RHI/Backend回归，共71项去重通过；Windows窗口消费者检查及受影响格式检查通过。22张正式WGPU路径离屏图像已逐张打开审查，见[窗口C执行证据](../证据/343/窗口C-执行记录.md)。本轮无OS粒子试玩窗口、无SDK/Scene接入、无大型负载/独立交付和安装声明；可选material与资产绑定留给D，拖尾附属存活粒子，跨效果仍按Graph绘制顺序。下一窗口Gate D，343整体不归档。

2026-09-15窗口D终态：Gate D passed。公开SDK/ABI成功提交、typed组件/Scene加载、包内纹理/真实网格/unlit材质、初始与运行时参数、RenderProjection及WGPU生命周期接通。修复会话实例编号复用、并行夹具时间戳碰撞、粒子退役误删共享Sprite纹理；在途检查改为真正提交后无等待退役。216项受影响域+4项新增纹理消费者+17项Compiler+1项SDK+1项现有Provider集成，去重239项通过；Windows real-window检查与格式检查通过。实际图像和原始首败/复验见[D执行记录](../证据/343/窗口D-执行记录.md)。这是源码/直接消费者资格，不是安装或完整效果交付；下一窗口E，F仍待完成，343不归档、338继续暂停。

2026-09-15窗口E终态：Gate E passed。普通particle_effects_project完成爆炸/吸附螺旋上升两例、各两轮真实修改、104张Windows捕获审阅、各5次重播与暂停恢复、世界空间源移动、同最终包普通及语义运行。3项实际Engine缺陷最小修复、21项去重定向测试与格式检查通过；失败、漏项纠正、原始源差异/包身份/用时见[E记录](../证据/343/窗口E-执行记录.md)。下一窗口F大型负载、局部失效与独立交付仍未执行，343不归档，338暂停；源码Provider未安装。

2026-09-15窗口F早期尝试（已复核降级）：dev720帧CPU均值7.513/P95 8.798ms，多数采样帧爆炸已结束；仅分配容量、非固定100k的发射器测试、scenario失效及未屏蔽原源的B复制运行不覆盖§7终态。完整缺口与续作预登记见[F执行记录](../证据/343/窗口F-执行记录.md)，不得再引用为P7/P8/P9通过。

2026-09-15窗口F终态：Gate F passed，98项去重受影响测试及格式检查通过。显式GPU时间戳/计数接入既有报告，修复逐发射器Pass、参数上传、小容量排序和不等容量binding。截图发现测试夹具空间序列重复后，仅修正run-owned输入并更新五档简单负载；最终1/64发射器100k空间图像字节相同。100k/64 CPU/GPU P95=4.781/5.402ms，16真实效果=5.560/5.906ms，原预算全部通过；200k请求/100k容量精确丢弃及clear/在途退役通过。隔离默认值/WGSL修改准备均约3.807s，A与Engine/Project DLL不变。最终正常build+windowed verify及46文件完整副本A24/B74帧可见运行通过，原作者路径屏蔽期间成功执行，已恢复；完整身份/失败/范围见F证据。P1–P9总表复核通过，343归档，338保持暂停未归档，安装不变。
