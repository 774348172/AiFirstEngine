# 372：二维局部 GPU 几何与增量呈现——方案 B 总施工文档

## 0. 授权、状态与范围

2026-10-10。用户明确要求“生成施工方案 然后直接开始施工”。上游为[372方案](../../372-二维局部GPU几何与增量呈现-方案B-v1方案.md)及[设计自审](../../372-方案自审与决策记录.md)，继承365／363和共同renderer owner；371保持暂停、恢复点S1-C。

当前状态：`completed`。P0–P3与R01–R12首期范围通过并归档；[实际合同](../../372-实际合同与验收.md)和[阶段记录](../../阶段完成记录/2026-10-10-372-Retained-Graphics/00-总览.md)保存身份／预算／原始失败／终验。Windows样本source／delivery通过，安装保持原contain，371保持暂停。

连续施工起点：2026-10-10 08:11 UTC（北京时间16:11），最迟10:56 UTC不再开始新实现／验证，11:11 UTC停止。本窗口估计140–155分钟含编译／证据／清理，若不足完整终验保留真实已完成阶段，不提前宣称372整体完成。

首期范围：Graphics2D共享局部mesh → AUI／UiProjection → typed RHI retained payload → 真实GPU矩阵／矩形scissor；混合精确CPU路径、参数旧帧保护、预算／淘汰／设备重建；项目局部坐标／分块、CPU与GPU对照及独立Windows样本。

不修改安装Provider、真实配置或旧交付；不恢复371，不增加world vector、专用Tilemap、GPU-driven、GPU复杂mask或第二registry。已有工作树有大量历史／用户改动，按文件保存施工前副本，不reset、不提交整个工作树。

## 1. 必要性与验证经济性自审

确认故障：镜头写入地形内容导致Paint／剖分／上传；局部几何cache之后仍在CPU生成最终顶点。最小因果修正：项目局部内容／公共平移，保留Graphics源mesh共享，原Renderer按项输出GPU参数与CPU精确回退。

必要文件：`engine_runtime/src/{aui,aui_graphics,runtime_renderer,render_graph,rhi_command_plan,wgpu_backend,lib}.rs`，原backend下二维子模块（需要时）；`tests/aui_draw2d_gpu.rs`及直接consumer测试；`runtime_player_winit`性能计量／必要诊断入口；像素项目`RuntimeModule/src/{lib,visual}.rs`、AUI。只有真实跨DLL变更才涉及SDK／Glue／协议升级，默认不新增。

| Gate | 风险／失败合同 | 最便宜可失败测试 | 终态证据／估计 |
|---|---|---|---|
| P0 | R01/R02会被仅缓存GPU但继续CPU算顶点的错误实现漏过 | 现有GPU测试文件增加暖图形位移的投影工作断言，先取得红；保留原故障回放 | 前置清单、首红、内部layout／预算冻结；15–20min |
| P1 | 源几何共享／分类路径未贯通真实shader，顺序或clip出错 | 同一生产Presenter→Renderer→RHI→GPU测试；受影响owner测试 | 共享与零CPU／零geometry上传绿，原GPU图形测试；35–45min |
| P2 | 复杂clip拖全场回退／颜色错误／旧帧参数覆盖／资源早回收 | GPU同机旧plan→新plan→旧plan像素、混合／非整数／旋转clip、容量与重建 | R05–R10真实GPU和CPU负例，受影响check／fmt；30–40min |
| P3 | 游戏仍大内容失效／高draw／仅dev平均下降冒充资格 | 项目tests、普通SDK/DLL构建、固定输入360帧预热210/采样150，静止与滚动 | 原包／项目CPU／retained对照、P95/FPS/上传、独立交付；35–40min |

复用：既有原故障包、输入回放、365已完成CPUclip／颜色／命中合同、371已交付S1-A目标/S1-B空间。新增源码只使实际消费它的测试／二进制证据失效；不重跑3D／Android／LocalCI／全workspace和安装验证。GPU语义使用real-wgpu，source绿不证明安装新版。

内循环复用`G:/gameEngin/rust/target`热缓存；同一target不并发Cargo。测试执行与编译耗时分开记录。首失败输出和修复后结果保存在`.artifacts/372/20261010/`，不只保留名字／退出码。高成本分支超过估计且没有改善信号时复核其必要性，不增加无关Gate。

## 2. P0：前置冻结与首红

1. 保存必要源码与导航施工前副本、实际HEAD／工作树身份和开始时刻；确认当前无有效施工，371历史暂停路径存在。
2. 增加`retained_graphics_translation_avoids_cpu_mesh_and_geometry_upload`生产链测试：暖图形仅平移；断言剖分0、GPU几何上传0、绘制消费CPU顶点生成0，核对位移像素。先在旧路径上运行并保留失败。
3. 内部共享mesh所有权／identity保持原JSON消费，新增typed retained命令，不改SDK/ABI；未经可信Graphics准备的raw Custom mesh走精确原路径。
4. 冻结首选紧凑参数布局：节点仿射、view映射、线性Tint独立字段；GPU局部顶点／索引不因参数变化上传。精确整数target矩形scissor首期合格，非整数不能证明等价时按项CPU回退。
5. 生命周期首选原backend单queue有序执行：packet保存不可变CPU参数，真正消费时write→encode→submit；不得提前为多个未提交packet覆盖同一参数页。几何不变／新incarnation分配，实际queue completion用于淘汰资源退休／有界回收，不用CPU fence／帧延迟。
6. 初始技术上限：局部geometry缓存≤1024项/32MiB，参数页≤1MiB/4096实例，retiring≤32MiB；容量／驱逐／超限须有可观测策略，不能覆盖在途数据。以上来自现有UI32MiB及4096项上限，P1/P2实测冻结实际峰值；若真实负载超限，先查分块／失效，不静默放大。
7. 比较目标冻结为实际Windows目标/相同profile与输入；固定360/210/150与既有回放一致。Release流畅目标60Hz，总CPU P95≤16.67ms作为目标，AUI／draw／显示FPS预算结合P0项目CPU对照写实际合同后进入P3资格，不把原故障dev数字当Release基线。

命令（依赖操作顺序执行，输出写本轮证据）：

```powershell
Set-Location G:/gameEngin/rust
cargo test -p engine_runtime --locked --features real-wgpu --test aui_draw2d_gpu retained_graphics_translation_avoids_cpu_mesh_and_geometry_upload -- --nocapture
```

P0通过表示前置和首红完成，不授GPU快通路或最终性能资格。

## 3. P1：局部GPU消费贯通

1. 原AUI mesh payload保留不可变共享引用，Graphics准备输出标明可信retained来源；内容版本／强引用防止pointer或slot重用误认旧内容。不在热帧hash大mesh。
2. Renderer资格判定只处理支持的无纹理Graphics、最终有效仿射、精度与精确矩形clip；输出原序retained batch，旧vertices batch不跨它合并。
3. 原RenderPass／RHI添加typed retained payload；最终矩阵与独立Canvas到target映射由shader应用一次。headless backend能正确观察计划，无GPU路径不冒充GPU资格。
4. 原wgpu owner增加局部vertex/index缓存和紧凑参数buffer，消费索引绘制；geometry、fallback、instance/view字节／CPU生成计数分别进入显式Summary，不增加默认热日志。
5. surface viewport／contain与offscreen都支持，retained scissor与既有内容scissor求交；混合后恢复正确scissor，不污染下一普通项。

```powershell
cargo test -p engine_runtime --locked --features real-wgpu --test aui_draw2d_gpu -- --nocapture
cargo test -p engine_runtime --locked --lib retained -- --nocapture
cargo check -p runtime_player_winit --locked --features real-window
```

最后一条按该crate真实feature复核；若名称不同，在执行前修正为已有实际feature，禁止创建假feature。P1只能宣称局部链路，不宣布372完成。

## 4. P2：精确混合、参数版本与有界生命周期

1. 混合顺序、线性颜色／straight alpha、glyph／图片、镜像／退化、DPI／AA维持365语义。非整数／旋转／圆角/8层clip的CPU回退只影响相关项。
2. 同一backend将旧不可变plan、新plan、旧plan依次真实执行；变换／颜色／内容替换及queue延迟，画面每次对应正确snapshot。缓存淘汰不早回收，退休依据真实completion；设备重建从有效CPU内容恢复。
3. 容量用尽有确定失败／背压；记录实际resident/retiring/staging峰值，不无限分配或忙等。复核参数write→submit顺序，避免所有CPU准备完成被误当GPU完成。
4. 绘制与命中继续用已发布几何，不修改业务snapshot authority；复用受影响AUI坐标／presentation／动画回归。

```powershell
cargo test -p engine_runtime --locked --features real-wgpu --test aui_draw2d_gpu -- --nocapture
cargo test -p engine_runtime --locked --features real-wgpu --lib retained -- --nocapture
cargo test -p engine_runtime --locked --lib aui_presentation -- --nocapture
cargo check -p engine_runtime --locked --features real-wgpu
```

若P2扩展测试已覆盖同配置P1测试，仅终验执行一次并复用，不机械重复。格式使用实际修改Rust文件的rustfmt检查；Clippy仅受影响crate／feature，不修无关旧债务。

## 5. P3：普通项目对照与交付

1. 像素项目：固定背景/HUD、固定clip父节点、移动内容；静态地形chunks，动态对象独立内容／参数。camera不再进入地形内容key或图元坐标。砖块变化仅更新对应块，形态缓存保留。
2. 必须保留CPU消费对照和retained对照，使用原正式Compiler/Assembler/Player/DLL发布路径。需要时增加明确诊断性的CPU/retained选择，不添加项目GPU控制API或新tool。
3. 项目测试、真实Module／Glue与独立副本通过；原big-still/big-scroll/small-scroll输入复用。报告CPU/P95/FPS/geometry/fallback/parameters/draw，geometry暖变化增量与最终性能同时检查。
4. 默认安装/配置和原交付保持；候选新游戏放`G:/gameEngin-deliveries/372-pixel-jump-retained/`，构建/运行产物身份单独记录。

```powershell
cargo test --manifest-path G:/gameEngin/samples/pixel_jump_adventure_project/RuntimeModule/Cargo.toml --locked
```

普通构建与运行命令在P0复用仓库正式入口／既有诊断脚本实参，绑定新source consumer后写证据；不能用已安装旧引擎构建冒充新renderer消费。固定回放每轮限60s，保存stderr／输入／summary／截图，失败先回到owner，不用整项目窗口反复调试。

## 6. 激活前自审

- 372正式方案已确认，设计自审与两路只读复审已读取；需要修正的零CPU顶点断言和真实GPUcompletion已纳入。无需改顶层设计。
- 当前槽为空、371暂停；本文件移入当前后唯一激活，用户已有直接施工授权。无并发Agent施工。
- Gate映射R01–R12，最便宜owner红测试先行；编译缓存复用，GPU/真实项目只承担不能由CPU证明的语义。无LocalCI／安装／跨平台扩张。
- 本方案的GPU资格和生命周期是用户确认范围，不能因发现共同owner缺口削去旧帧保护；无需把整块371作为前置。
- 预算/feature/具体命令在源码复核后冻结，更新必须写本文件，不悄悄取消断言或变更profile。成本超过窗口时停止新工作、保存证据和真实阶段状态，不伪造总完成。

自审结论：`prepared_review_passed`，允许根据既有用户授权激活本文件。当前scope未扩大，正式方案无需改动；内部布局和首期精确scissor资格属于已确认路线内技术冻结。

## 7. 证据、同步与归档

每Gate保存首红/失败首因、实际命令、源码/测试/二进制身份、compile与run时长及恢复状态；`.artifacts/372/20261010/`不作为产品truth。

### 本轮冻结与进度（2026-10-10）

- P0首红：旧路径暖平移仍生成72个CPU绘制顶点，`p0-red.log`；编译144s，实际测试0.56s。原故障项目dev三组输入与summary复用，不重新覆盖基线。
- P1实际布局：本地索引mesh为强引用、单调incarnation；64byte实例页4096项（256KiB），局部GPU缓存1024项/32MiB，单包独立几何同上；退休32MiB。raw/JSON反序列化mesh不取得可信资格。冷几何验证一次，热帧不遍历顶点。backend单queue write→encode→submit，每次消费重新生成不可变CPU包参数；GPU completion callback与实际poll用于退休，无CPU帧延迟。
- 精确轴矩形：最终target边界为整数时scissor，非整数/圆角/旋转clip逐项原CPU精确回退；surface内容viewport继续应用，并在下一普通draw前恢复。相邻同几何/scissor才实例合批，跨fallback不合并。诊断CPU对照可显式设`AIFE_AUI_CPU_GRAPHICS_REFERENCE=1`（仅本轮子进程），或调用renderer参考模式；正常模式开启retained。
- `p1-gpu.log`首绿3项；`p2-terminal-gpu.log`终验5 Draw2D+4 presentation真实GPU项通过（编译87s，测试2.01+0.55s）：零CPU生成/零暖几何上传、CPU/GPU像素、镜像/tint/多clip/DPI、旧→新→旧、1025次替换/LRU/旧包重建、损坏与4097实例拒绝后恢复。
- `p2-owner.log`发现施工前已存在的旧测试辅助函数名缺失；施工前副本可证明。修测试调用到既有`projected_quad_with_presentation(...,None)`，保持原断言；`p2-owner-repaired.log`163项AUI相关owner测试通过，运行6.09s。
- GPU资源有界超限明确拒绝当前包，不扩大容量；P3项目通过局部地形/角色与可见性保持预算。非Windows/安装/LocalCI不在本期资格内。

### P3终验前负载／预算冻结（09:22 UTC）

Windows x86_64、RTX 3070／Vulkan、960×540实际窗口（原输入及截图尺寸已复核），dev／Release分别对照；原输入big-still、big-scroll、small-scroll-safe，360帧／预热210／采样150。每个profile同一DLL与项目产物，仅子进程显式CPU参考开关不同，保留真实FPS截图，不能用CPU耗时倒数替代。项目固定13个16列地形chunk（含相邻一列AA重叠并按各自条带裁剪）、8平台／48金币／32actor／1玩家槽，按可见性提交。

Release资格目标在最终Release测量前冻结：总CPU P95≤16.67ms、AUI CPU P95≤2ms、retained实际draw≤64／帧（原fallback保持既有计量，本期不新增全后端draw计数），显示FPS目标约60（允许定时HUD采样和垂直同步误差，不证明全程每帧60）。dev单独报告，不套Release资格。暖单图形变换机制仍要求CPU几何生成／剖分／局部GPU几何上传均为0；游戏滚动新可见chunk和形态切换允许冷上传，分别报告。若目标失败，保留失败和实际状态，不降低预算换通过。

首轮真实dev截图发现world Panel默认边框造成x767竖线及chunk AA边缘接缝；原始图与差异保留。修复为显式零边框，以及地形相邻一列重叠／独立Rect条带clip；边界砖块允许更新两个相关chunk，内部砖块仍只更新本chunk。最后帧截图读回／PNG可使max CPU异常升高，保留该帧并解释，不从数据删除。

只有对应范围证明后才写372实际合同/阶段记录，同步49/54、README/地图/阅读顺序。全部R01–R12与首期P0–P3完成后才将本总施工移入已完成；部分完成保留唯一当前文档和下一Gate。安装状态单独保持。3小时停止时记录`stopped_by_time_limit`、已完成/未完成、拥有进程与环境清理结果，禁止归档为完成。

## 本轮终态

P0首红、P1局部GPU、P2语义与有界生命周期、P3项目／正式SDK-DLL／独立dev-Release及冻结性能预算通过；全部原始日志保留于`.artifacts/372/20261010/`。最终9项GPU／24 retained owner／89 Player／27项目测试及fmt／受影响Clippy通过，此前163项AUI owner按输入身份复用。对应测试有重叠不相加。实际结果及证据映射见372实际合同；默认安装未切换，旧游戏与配置未改，owned进程为0。
