# 374 Gate D：作者、SDK与原会话接入完成

Gate A/B/C/D已关闭，D已按30文件增量串行合入主线；作者v2/Cooked索引、SDK意图与纯数据查询、session原子提交及固定Tick接入完成，310项去重测试/配置通过，scoped Clippy增改行无未处置告警、30文件fmt及真实窗口feature编译通过。371阶段46的11源文件逐字节保留。E/F未启动，374整体仍partial，未安装，游戏固定V0.0.5。

本窗2026-10-23 13:41:05 UTC开工，提前收尾时间见证据`closeout.json`。只关闭D，总施工保留当前目录。当前产品/施工仍以[49](../../49-当前状态与下一步入口.md)、[54](../../54-当前可自动化施工总入口.md)为准；[实际合同](../../374-B-实际合同与验收.md)§4–6/15与[总施工](../../施工文档/已完成/374-Animator与Spine骨骼网格变形及动作混合-方案B-总施工文档.md)§23/24记录本窗边界。

## 实际实现与需求出口

| 风险/需求 | 原owner中的结果 | 可红证据 |
| --- | --- | --- |
| 作者schema、类型、名称、重复键和未支持操作 | 原Compiler将v2分流到Spine，v1仍走原Sprite；严格字段/唯一映射、名称到索引、显式mask与Bool/Trigger规则；秒数请求与实际ticks均入inspect | main-project的作者包/负例矩阵/duplicate JSON/缓存用例，main-author旧分流与可选字段null负例 |
| Cooked及包绑定 | typed controllers/masks/规则/挂点，经原Assembler和包验证；实体spineAnimator与旧Animator/3D冲突拒绝，控制表/registry身份一致 | main-project包/错digest/绑定负例；骨架Cook Hit但控制表digest改变 |
| SDK和原C ABI | 两SDK九种strict意图、live generation、只读snapshot/socket/event；沿现有JSON缓冲，SDK digest加入animator-session-intents.v1，C布局/主版本保持 | sdk-02的87项、main-runtime原native adapter与serde/builder/dyn reader |
| Applied/NoOp/错误的原子性 | 合法callback先给快照，再整批无native副作用预检，原World prepare/commit成功后才入Animator队列；Rule先整批拒绝Animator | main-runtime Rule前置拒绝、Focus phase、命令/事件预算；main-project NoOp/坏World+合法Animator/坏Animator+World/句柄与代际 |
| 时钟与初始化 | FrameLoop在首次AUI前初始化；核对包tickRate与实际fixed delta；每fixed tick只推进一次，事件保留发生tick供下一合法callback读取 | main-project初始化/同tick/下一callback/时钟不符；main-runtime同tick/逆tick、旧Animator2D |
| 观察与工具消费 | 原inspect/check补spineInspection；原Player报告optional spine，由原observe/verify报告链携带；Off不构造，Summary计数，Trace纯数据 | main-tools实际NativeHostAdapter；main-project断源包headless Player Summary；main-runtime Off/Summary/Trace；main-tools-lib原29项 |
| 可选后端与实际窗口consumer | 未编译后端明确unsupported/reject；Spine原生状态归FrameLoop线程，项目session仍只持纯数据 | default/default-tools；main-window-check实际spine,real-window |

`FrameLoop`因持有thread-confined原生状态移除Clone；未给项目暴露native pointer。每实例每批64条（包括skin），session命令/待发事件有界4096。可预判错误不提交；有限后端错误隔离实例并保留上一成功结果，OOM仍不承诺恢复。Compiler当前产60Hz包，宿主fixed clock不符在AUI前拒绝。

SDK digest变化要求普通项目沿原AOT机制重建；原C函数表/布局保持不代表旧编译DLL可忽略digest检查。两SDK旧wire、原AUI/native adapter和Animator2D行为在同组合通过；冻结V0.0.5不升级。

## 串行交接与保留371

先在54登记共享文件交接，再按D baseline/main/worktree逐文件SHA比较，仅发布22个已有文件和8个新文件；没有冲突。`before-main-publication.zip`保留22文件前态，`handoff-plan.json`/`publication.json`记录三方SHA、写前CAS及1166个非D基线文件保留检查。371阶段46已收尾，11源文件按其final-identity逐字节保留；54 A行SHA也保持。未覆盖整个隔离目录、未写Git索引或删除原空index.lock。

## 终态验证、成本与复用

所有下列命令退出0、失败0、ignored0、源码漂移0；测试共310项，默认与Spine为实际不同配置。前面的隔离开发重跑不重复累计。每轮完整命令、cwd、独立374 target、输入SHA、日志SHA、开始结束时刻和stdout/stderr均在对应inputs/result与日志中；总wall包含构建与执行，不能当作测试时长。

| run | 通过数/结果 | 总wall | Cargo构建 | 测试执行 |
| --- | --- | --- | --- | --- |
| sdk-02 | 23 + 64 | 8.937s | 8.33s | 0.0s + 0.04s |
| main-author-01 | 6 | 31.765s | 30.91s | 0.01s |
| default-03 | 1 | 119.250s | 1m 58s | 0.02s |
| default-tools-03 | 1 | 13.375s | 12.27s | 0.03s |
| main-runtime-01 | 172 | 176.032s | 2m 51s | 3.96s |
| main-project-02 | 13 | 144.203s | 27.41s | 115.91s |
| main-tools-02 | 1 | 10.250s | 9.39s | 0.04s |
| main-tools-lib-02 | 29 | 6.875s | 5.76s | 0.28s |
| clippy-core-03 | 编译/检查 | 10.110s | 9.84s | 不适用 |
| clippy-sdk-03 | 编译/检查 | 3.437s | 3.17s | 不适用 |
| main-clippy-consumers-01 | 编译/检查 | 31.281s | 31.01s | 不适用 |
| main-window-check-01 | 编译/检查 | 22.750s | 22.53s | 不适用 |
| main-fmt-01 | 编译/检查 | 0.547s | 不适用 | 不适用 |

默认使用固定Rust 1.96.0、`--offline --locked -j1`。先加载工作区`07-Tools/Enter-EngineEnvironment.ps1 -Lane 374`；主线验证cwd是`01-Engine/main/rust`，target保持374独立目录。主线runtime过滤`spine_animation animator2d project_runtime_session frame_loop project_runtime_native_adapter`，Compiler运行原`spine374_project`target，Provider运行`spine374_tools`及原lib suite；没有跑全workspace或Local CI。

最终scoped Clippy是core/SDK/Compiler/Provider libs；原依赖和旧owner告警保留，增改行审计为0。新session函数保留原分离owner借用边界，局部允许too_many_arguments；新作者文件沿原结构化Compiler错误，局部允许result_large_err，没有为消除告警重写公共错误/owner。30个增改Rust文件用skip_children格式检查，避免递归改无关子模块。

C后端189文件全部不变，40+1项后端证据复用；B的16套像素只复用未变的静态渲染方程/资源路径，不算本窗动态项目呈现。主线受影响组合重新运行以覆盖371 A46的实际装配；两SDK及原作者分流/无后端拒绝证据按未变消费路径复用。身份与失效条件见reused-evidence.json。

## 首错与修复

保留check01–04、sdk01、native01、compiler01、project-01及Clippy旧失败日志。实现阶段修正新增mutation的match、WorldRead dyn兼容、旧测试构造和可见性/稳定API；SDK Resume及作者Trigger用空结构variant，修复serde unit variant忽略额外字段。Rule新增整批前置拒绝，避免非法Animator前已有World写入；有效Animator+失败World批次不入队。没有削弱负例断言。

clippy-sdk-02的-D warnings失败来自未改的kinematic2d旧needless_range_loop；终态改为按增改行审计，保留旧债，不冒称SDK全库零warning。新代码boolean表达式已简化；所有首错与重跑均有原始日志。昂贵重组只在新增行为/主线身份变化后执行，未重跑旧GPU/正式性能。

收尾的author-null-red实际证明`entity:null`违背冻结JSON Schema却被原Option解析接受；只在原作者解析owner为entity/mask/fadeOutSeconds/onComplete添加缺省允许、显式null拒绝。单owner红绿已留存，四字段负例与合法缺省在main-author通过；随后刷新主线Compiler项目/Provider/Clippy。未受影响的runtime/窗口不因Compiler文件变化重跑。该单文件修复的CAS与前态另存publication-repair-01.json和before-main-repair-01.zip，未覆盖首轮发布证据。

## 身份、收尾与继续点

证据真实路径：`D:/GameEngine/EngineMigration/EngineMigration/01-Engine/main/.artifacts/374/gate-d-20261023-134105`。当前1197文件dirty源码seal：`a16c4498d5fcb11ec40add8f96fdba019cf9a2aa898cd2252f26002cd62f755f`；分开的产品/测试/依赖输入、逐文件SHA见source-identities.json。它不授新commit、发布二进制或安装资格。

本窗只使用run-owned证据/fixture目录和374共享构建缓存，均保留；未修改安装/外部配置，没有需要回滚的配置事务，没有取消另一条路任务。自有Cargo/Player结束情况见closeout-processes.json。E普通项目真实动态呈现/独立交付、F完整生命周期及三组正式配方仍未启动；不能把D的headless功能验证当作它们完成。后续需用户授权激活E，复核主线与两路共享owner；不重做已证明且身份未变的A–D。
