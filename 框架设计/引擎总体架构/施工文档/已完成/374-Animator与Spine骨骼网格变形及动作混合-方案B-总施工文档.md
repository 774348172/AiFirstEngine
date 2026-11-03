# 374 Animator 与 Spine 方案 B：总施工文档



> 状态：**Windows功能施工完成并归档；完整性能资格独立待验**。用户2026-10-29明确确认，见§50及完成记录。

> 正式依据：[374 方案 B](../../374-Animator与Spine骨骼网格变形及动作混合-方案B-v1方案.md)，已按 [374-A 审核](../../374-A-方案审核与施工文档自审.md)修订。

> 自审：文档编制审核通过；Gate A已取得版本、FFI/mask及事件可行性证据并冻结定量合同，不冒称生产Animator或规模已实现。

> 施工登记：[54](../../54-当前可自动化施工总入口.md)；本总施工按用户确认的功能出口归档，旧窗口段落保留历史身份。



## 1. 施工目标、完整退出与排除项



在普通二维项目中实现：Spine 导出骨架/atlas/纹理→正式资源登记与增量 Cook→RuntimePackage→固定 Tick Animator 控制→官方 Spine CPU 求值→deform/骨骼/挂点/事件→既有 RenderProjection→真实 GPU。AI 通过单份作者描述、命名 typed 意图及现有独立工具完成接入、检查和观察。



完整退出同时要求：



1. 骨骼与 IK/transform/path 的已冻结子集、region/unweighted/weighted/linked mesh、deform、skin/attachment/draw order 的真实消费。

2. Crossfade、Replace、已验证的连续属性 Additive、OneShot 淡出、连续中断、源局部动作与**真实显式 mask**。

3. 固定 Tick、0 时刻/多 loop 内容事件、暂停、混出抑制、下一合法 callback 消费、模拟挂点和代际隔离。

4. 多页/旋转 atlas、裁剪、PMA/straight alpha、normal/additive/multiply/screen 按能力矩阵有实际像素证据。

5. AI 作者与 SDK/工具闭环、旧 341 兼容、增量构建、多实例/重载/退休、独立 Windows 交付与冻结规模配方。



排除：安装/默认 Provider/宿主配置、V0.0.5/V0.0.6 冻结目录和旧游戏包、371 S3 整体、3D GPU skinning、AUI 内嵌 Spine、编辑器时间轴、自动绑骨、Root Motion 接管、数值参数/seek/公开队列、BlendSpace、反向播放、Android资格、完整workspace回归及Local CI。不得把排除项写为空实现后宣称支持。



## 2. 激活前复核与前置条件



本节记录激活前规则；374已按§15完成复核并从待执行移至当前。Gate A–E现已完成，E主线集成见§26；F已按§27激活，当前结果见§32（§28–31为前序阶段/本轮执行记录）。未激活时仅允许文档准备。



用户要求开始施工后，执行以下复核；已有明确施工授权时不重复索要同一许可：



1. 重读 49/54、374/374-A、本文件与 371 另一条路当前 scope；检查待执行优先级与可用槽。

2. 比较 374-A §4 快照与实际源码，确认基线漂移、新版 crate 边界、Cargo package/features/测试目标及公开工具合同。HEAD 不代表既有 dirty 全部内容，不能从当前 HEAD 新建 worktree 后假定已含必要未提交功能。

3. 采用独立 worktree/`codex/374-spine-animator` 分支作为默认方向，工作路径激活时写入 54；只携带已核对需要的基线，保留用户与 A路修改，不自动 reset/clean/cherry-pick 无关提交。

4. 锁定 374 独立 SSD target、证据根和资源/样本所有权。共享入口的修改归属、合并顺序与 A路写入 54，GPU/正式性能测量避开另一条路高负载。

5. 复核通过，将本文件移到 `施工文档/当前/`，更新 54 对应槽、队列与文档导航/374-A 链接，再开始 Gate A。文档移位不改变 Gate 顺序，不代表已通过技术可行性。



## 3. 涉及文件与所有权



以下为设计定位；具体文件在 Gate A 技术合同缩小并冻结，不得借“相关目录”任意扩大修改。



| Owner | 现有文件/拟新增位置 | 修改目的 |

|---|---|---|

| 官方后端边界 | **拟新增** `rust/crates/engine_spine_runtime/{Cargo.toml,build.rs,src/,native/,tests/}` | 窄 C FFI、Rust 安全包装、只读共享数据/实例独占、mask/事件索引；单独 crate 避免新 C++ 依赖进入无 Spine 的默认热路径 |

| workspace/依赖 | `rust/Cargo.toml`、`rust/Cargo.lock`；精确第三方源码目录由 Gate A 冻结 | 必要 workspace、feature/依赖与可重复离线源；不引入单独构建 daemon |

| Runtime | `engine_runtime_core/src/{frame_loop.rs,project_runtime_session.rs,runtime_package.rs,runtime_package_builder.rs,render_projection_adapter.rs}`，**拟新增** `spine_animation/` | capability/registry、固定 tick、提交、实例候选、挂点与投影 |

| Compiler | `project_authoring_execution/src/{animator_description.rs,project_runtime_package_assembler.rs,neutral_assembler.rs,render_asset_cook.rs}` 与 **拟新增** `spine_import/` | 作者 v2、导出数据检查、atlas依赖、Cook与Assembler单入口 |

| SDK/事件 | `project_runtime_sdk/src/lib.rs`、`project_game_sdk/src/lib.rs`、实际 `project_runtime_abi` 必要合同 | typed意图/句柄/事件与digest；只改实际需要的wire，禁造平行ABI |

| Render core/native | `engine_runtime_core/src/render_geometry*`、`render_projection_adapter*`；`engine_render_native/src/wgpu_backend/real/{geometry*,retained_ui.rs,memory.rs,lifecycle.rs}` 中实际需要的 owner | 有序动态几何/材质/上传/退休；不把 Spine 规则放到 backend，不按目录整块重写 |

| 测试与工具 | 上述 crate 直接 tests、`engine_runtime/tests/`、`engine_tool_provider/tests/`、`project_e2e_gate` | 领域数据与既有 Compiler/playtest/verify消费者；不新增统一工具 |

| 普通样本 | **拟新增** `samples/spine_animator_project/` | 合法导出资源、最小项目逻辑、描述与scenario，不修改现有游戏 |

| 文档 | 374、374-A、未来 `374-B-实际合同与验收.md`、本文件和相关入口 | 合同落地、证据与状态同步 |



共享帧循环、Assembler、SDK/wire/schema、native renderer、Cargo.lock 的最终整合归本路明确的接入窗口；A路仍持有的文件不并发覆盖。不能用第二套 RenderProjection、加载器、报告系统或缓存绕过共享归属。



## 4. 必要性与验证经济性审计



用户要求的最终能力是374 §1–13的真实Spine角色链路。当前A–E已完成，E范围与经济性见§25/26；F完整生命周期与正式Release配方尚未验证。



最小因果修改是：官方后端包装、Spine资源/作者合同、原Runtime/SDK提交与事件消费、原渲染适配及必要普通项目消费者；没有必要顺便重写 Animator2D、全部 ECS、构建系统或 AUI。



| Gate | 风险/失败合同 | 最便宜的可红测试 | 终态证据 | 估计与停止条件 |

|---|---|---|---|---|

| A | 精确库、mask所有权/缓存、事件游标接缝不可行；预算/误差未定 | 后端独立crate小样本与手算事件索引 | 374-B冻结合同、源码与fixture摘要、FFI小测试 | 1–2施工窗；单次源码/构建探针30分钟无新增信号先停止该分支 |

| B | 导入、包身份、deform或真实绘制错误 | importer负例、已知顶点、单离屏图 | 同源Cook/包/CPU结果/GPU像素 | 2–4窗；CPU失败不进入GPU或完整导出 |

| C | 混合残留、mask误过滤、多loop漏发/超量 | 几骨骼/少顶点确定性时间序列 | 精确姿态/轨道/事件golden及所有权证据 | 2–4窗；mask需改架构时先回方案 |

| D | Applied/NoOp错误、阶段重入、旧项目退化、工具假发现 | SDK与session owner、单Compiler消费者 | typed/schema/工具/旧341兼容 | 1–3窗；跨协议扩大先复核必要性 |

| E | AI普通项目实际链路缺段 | 已有Compiler/playtest的单项目消费者 | Windows独立包、实际事件/挂点与截图像素 | 1–2窗；组成不含目标模块则停止，不能反复完整构建猜测 |

| F | 串实例、重载/退休错误或冻结规模失败 | owner负例后一次正式Release配方 | 生命周期与性能/独立交付终态 | 1–3窗；性能超预算先定位owner，不扩GPU蒙皮 |



窗数仅为任务拆分估计，不是完工承诺；每窗最多三小时，2小时45分停止新增实施/验证，三小时停止并记录。若预算显著超估计，先复核原因和阶段范围，不盲目续跑或削弱退出标准。



复用规则：历史341证明旧行为定义，不能直接证明新feature组合；同后端/同源payload的Gate A数值证据可供B/C消费；同最终二进制/fixture/平台的已过像素不机械重跑。产品源码、后端、fixture、测试程序、shader、配置分别记身份，修改只失效实际消费者。



默认不跑全workspace/Local CI/全平台/全部feature，不把真正昂贵的组合lib测试当后端开发内环。`engine_runtime_core` 的 `[lib] test=false`，禁止用该包 `cargo test --lib` 的零测试结果过Gate；私有原owner单测位于 `engine_runtime/src/unit_composition.rs`，对它使用受限过滤或可证明更便宜的公开integration消费者。



## 5. 命令与证据约定



B–F命令从届时已同步的本路仓库根、PowerShell 7.2+执行，复用 `rust/Invoke-EngineBuild.ps1`，显式本路独立target。Gate A按§15使用独立manifest和直接Cargo，具体已执行命令见374-B §9；不修改共享脚本或把374证据写到371默认根。



```powershell

$spine374Target = 'I:/gameEngin-build/374/cargo-target'

$spine374Evidence = Join-Path (Get-Location).Path '.artifacts/374'

```



激活时确认 I 盘可用，必要时选择另一个明确SSD路径并写入54；不静默回退。每次新命令的 shell 重新声明以上变量；每次 `-EvidenceDir` 使用新的 `Gate-阶段-runNN` 子目录。以上变量不作为跨工具调用持久状态。



`engine_spine_runtime`是nested workspace，必须在其真实目录执行Cargo。当前spine feature、B包/渲染消费者与C控制owner已存在；D–F目标须实现并核对后再执行。每个Gate先实现相应测试入口；原目录/包名变化先更新本文件。命令维持 `--locked -j1` 和有效工具链；A只创建自己独立Cargo.lock，未改主workspace锁文件。



每次过滤测试检查实际执行数>0、ignore/feature/skip状态、断言结果和预期case清单。0测试、全部skip、只check编译、测试程序不含目标生产模块，都不能过行为Gate。首错保存具体assertion、输入/日志、exit code、源码与产物身份，不能只留测试名。



下列目标命令是终态验证入口；调试期间先最窄owner红测，再一次终态suite。失败在当前Gate修复，不跨Gate堆功能。



## 6. Gate A：技术合同与后端最小可行性（对应 M0）



### A1 任务与输出



1. 复核官方spine-cpp确切commit、导出版本、许可证与样本再分发条件、Windows编译器/ABI；允许项目自有且由官方parser实际消费的数学测试输入，但不得冒称Editor导出或官方美术。合法输入不足时保留blocked，不假定许可或镜像来源。

2. 建立最小独立后端crate及窄C接口；加载单骨架/图集元数据、推进、读取骨骼/顶点、创建/释放两个独立实例。自定义错误传播与不可恢复OOM明确分开。

3. 在锁定源码逐函数核对时间推进、apply、attachment deform、clip/mask对象构造、timeline property ID、混合hold缓存和C++对象所有权。证明过滤后动画可安全独立拥有；共享只读数据、无裸timeline重复释放。

4. 编译源内容事件索引，验证实际播放时钟、loop区间、入场0键及internal fade计划的来源，不把官方callback默认能力当作完整多loop合同。记录哪些官方回调被屏蔽为对照、哪些生命周期保留。

5. 生成 `374-B-实际合同与验收.md`：精确schema/SDK/feature/包字段与错误码、能力矩阵、样本hash、上限、坐标/PMA与blend方程、事件与mask策略、错误隔离边界。

6. 冻结三类配方（单角色正确性、常规多角色、复杂裁剪压力）的角色数、骨骼/顶点/层/混合/事件、采样数、时长、CPU/GPU/上传/内存P95/P99阈值及硬件/profile；冻结骨骼/顶点/像素容差。先写合同后运行正式配方，不能按结果放宽阈值。



### A2 测试命令（已执行的独立后端）



```powershell

Set-Location C:/Users/zenghaoran/.codex/worktrees/374-spine-gatea/gameEngin/rust/crates/engine_spine_runtime

cargo +1.96.0 test --offline --locked -j1 --target-dir I:/gameEngin-build/374/cargo-target --test gate_a -- spine374_a_ --test-threads=1

cargo +1.96.0 check --offline --locked -j1 --target-dir I:/gameEngin-build/374/cargo-target --lib

cargo +1.96.0 clippy --offline --locked -j1 --target-dir I:/gameEngin-build/374/cargo-target --all-targets -- -D warnings

cargo +1.96.0 fmt --check

```



必须覆盖合法加载/版本拒绝/有限错误、双实例状态隔离、卸载旧引用拒绝、mask对象销毁与property IDs、0/单/多loop事件手算索引。内存所有权动态检测采用所选C++工具链实际支持的工具；不支持时记录替代计数/重复构造释放与限制，不伪造sanitizer通过。



退出：374-B内容完整、后端小探针通过；仅允许B之后在冻结能力内施工。官方源不可得、FFI/mask无法保留语义、事件无法与真实轨道时钟绑定、合法fixture缺失，均停止后续Gate并回填方案，不降级成A或删mask后称完成。



## 7. Gate B：资源、骨骼/deform 与真实渲染（对应 M1）



任务：



- 加入带capability的资源声明/包注册，经原Assembler单入口加载官方数据；atlas纹理引用进入AssetGraph，任意外部路径拒绝。JSON/二进制分别测试，未验证格式拒绝。

- 实现骨骼/约束、region/unweighted/weighted/linked mesh/deform的CPU输出；坐标与实体变换只应用一次，记录动态bounds。

- 接入原Projection的Spine Adapter与真实native几何、裁剪及blend/material能力；保持draw order，不按纹理跨透明段乱合批。

- 在原consumer做多页/旋转atlas、PMA/straight alpha、各blend、裁剪、attachment/skin切换小画面验证。新增native测试target只承载实际消费，不能只返回CPU期望颜色。



```powershell

./rust/Invoke-EngineBuild.ps1 -Package project_authoring_execution -Features spine -Filter spine374_b_ -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'B-import-run01')

./rust/Invoke-EngineBuild.ps1 -Package engine_spine_runtime -Filter spine374_b_ -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'B-pose-run01')

./rust/Invoke-EngineBuild.ps1 -Package engine_render_native -Features real-wgpu -TestTarget spine374_pixels -IncludeIgnored -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'B-pixels-run01')

```



Gate A冻结native只消费中性几何，不新增`spine` feature；Compiler/Provider/Player的真实后端转发见374-B §3。GPU不可用时CPU结果可单独通过，B整体与完整374保持partial。



退出证据：依赖闭包/坏引用/不支持特性负例，手算/官方对照骨骼顶点，包内资源身份，实际像素差和图，源文件缺失运行的包内加载小consumer。只打印顶点数或有截图文件不合格。



## 8. Gate C：控制、动作混合、显式mask与事件（对应 M2，已完成）



任务：



- 控制表编译为紧凑层/参数/动作索引，Spine唯一负责时间线混合；tick时钟、ceil时长、pause及幂等play/restart语义固定。

- 建立base/覆盖层、动作对过渡、once淡出、clear立即恢复、三次以上中断和有界内部计划；过预算预判拒绝不产生半批状态。

- Additive只开放A合同证明的连续属性与setup/reference语义；离散轨道和未验证deform组合拒绝。

- 从全身源动画生成真实mask视图，验证被过滤骨骼/slot/deform，property IDs/hold和同骨多属性；跨IK/transform/path/父级及draw-order危险依赖准确拒绝。不用预先只有上身轨道的动画冒充mask实现。

- 内容事件由单游标枚举，跨多loop/0键/同名多轨/权重0/混出/clear/暂停分别验证；按tick内绝对跨越时刻排序。内容事件预算拒绝和求值失败不发布新挂点/姿态/事件，后端不可恢复错误不伪装可回滚。



```powershell

# 从工作区根同shell加载；脚本会进入374独立目录并设置独立target。

. './07-Tools/Enter-EngineEnvironment.ps1' -Lane 374

Push-Location './rust/crates/engine_spine_runtime'

cargo test --offline --locked -j1 --tests -- --test-threads=1

Pop-Location

Push-Location './rust'

cargo test --offline --locked -j1 -p engine_runtime --features spine --lib -- spine_animation animator2d spine374_b_ --test-threads=1

Pop-Location

```



退出证据：固定tick输入→实际姿态/轨道/内容事件golden，对照官方正常单loop与独立手算多loop；未知动作和超限负例；所有权重复构造/释放证据。未取得显式mask不能关闭C。



## 9. Gate D：AI作者、SDK、session与独立工具（对应 M3）



任务：



- 落地schema-first作者v2，支持Bool/Trigger、有限规则与声明播放/渐退/skin/挂点，未知字段/重复名/非法类型/未支持seek等明确失败；请求秒数与实际tick可观察。

- 扩充现有SDK mutation/session，Applied/NoOp/失败语义保持；检查handle/generation/阶段、registry未初始化命令顺序和非法批次无部分提交。

- FrameLoop只推进一次模拟；动画事件保留发生tick并在下一合法callback可见，项目不重入、不重复FixedUpdate；挂点纯数据快照，不读取GPU或裸ECS引用。

- inspect/check/observe/verify扩领域数据，原九工具保持独立；新能力缺失消费者拒绝，旧341描述与原play行为保持。只有实际wire/ABI发生变化才版本化对应合同，不额外添加统一路由。



```powershell

./rust/Invoke-EngineBuild.ps1 -Package project_runtime_sdk -Filter spine374_d_ -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'D-runtime-sdk-run01')

./rust/Invoke-EngineBuild.ps1 -Package project_game_sdk -Filter spine374_d_ -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'D-game-sdk-run01')

./rust/Invoke-EngineBuild.ps1 -Package project_authoring_execution -Features spine -Filter spine374_d_ -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'D-author-run01')

./rust/Invoke-EngineBuild.ps1 -Package engine_runtime -Features spine -Filter spine374_d_ -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'D-session-run01')

./rust/Invoke-EngineBuild.ps1 -Package engine_tool_provider -Features spine -TestTarget spine374_tools -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'D-tools-run01')

```



Gate A应锁定Provider是否通过编译feature或运行时能力发现承载Spine；需要时上述Provider/Compiler命令补实际feature，不能假定默认装配含目标后端。仅在改动ABI时增加相应ABI owner测试。



旧341兼容在同一最终测试组合中执行：



```powershell

./rust/Invoke-EngineBuild.ps1 -Package engine_runtime -Features spine -Filter animator2d -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'D-legacy-runtime-run01')

./rust/Invoke-EngineBuild.ps1 -Package project_authoring_execution -Features spine -Filter animator2d -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'D-legacy-author-run01')

```



若终态受影响suite已覆盖同配置这些case，直接记录覆盖并复用，不重复。无Spine默认构建的检查在F最终组合做一次，证明可选依赖没有改变旧入口。



## 10. Gate E：普通项目到Windows独立消费（连接 M1–M4）



新增普通样本包含：idle/run、局部攻击与全身源mask两条独立路线、带deform角色、至少一次skin/attachment切换、命名挂点特效及内容事件接收。项目业务留在RuntimeModule，Core不出现Player/Weapon等专用API。



通过原 `GameProjectCompiler`、现有 `playtest` 与 `verify` 实现 `project_authoring_execution/tests/spine374_project.rs` integration消费者，复用准备/发布/启动/语义断言；不新造导出桥或通用施工runner。



```powershell

./rust/Invoke-EngineBuild.ps1 -Package project_authoring_execution -Features spine -TestTarget spine374_project -IncludeIgnored -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'E-project-run01') -TimeoutSeconds 1800

```



该target内部使用Gate A锁定的Windows/profile/Spine装配，分开记录Compiler/Cook、Rust构建、Player运行时间。具体Project Manifest、BuildRequest与scenario字段取现有生产类型，不编造不存在的CLI命令；开发入口可先复用dev，最终独立交付按F的Release身份。



验收实际引擎观测：跑步持续、上身攻击、deform顶点、mask隔离、事件次数、挂点跟随及真实图像。把原样交付复制到独立目录，在不含项目源文件的条件运行；此消费者只限制自己临时目录，不改用户源目录。未安装Provider，不能宣称当前宿主自动可用Spine。



## 11. Gate F：生命周期、正式配方与整体退出（对应 M4）



任务：



- 多实例共享data而不共享状态；禁用/暂停/离屏、销毁重建、过期handle、源替换成功/失败、skin/deform残留清理；重载默认重置，旧事件清空。

- 验证实际GPU完成点后退休、在途缓冲不提前复用，动态裁剪输出/混合链/事件池容量不无限增长；只测本功能触及的生命周期，不重复371全矩阵。

- 按Gate A冻结的三类配方用Release/Off测CPU分段、GPU、上传、稳态/峰值内存和P95/P99；Summary/Trace开销单列。GPU计时与CPU阶段不相加成虚假总时间。无探针最终产物与测量输入绑定。

- 最终一次默认非Spine兼容check；对实际修改文件做fmt/scoped Clippy，新增C++使用锁定工具链编译警告/静态检查；不触碰无关历史格式债务。



```powershell

./rust/Invoke-EngineBuild.ps1 -Package engine_runtime -Features spine -Filter spine374_f_ -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'F-lifecycle-run01')

./rust/Invoke-EngineBuild.ps1 -Package engine_render_native -Features real-wgpu -TestTarget spine374_pixels -IncludeIgnored -Filter spine374_f_ -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'F-retire-run01')

./rust/Invoke-EngineBuild.ps1 -Mode Check -Package engine_runtime -Lib -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'F-default-check-run01')

./rust/Invoke-EngineBuild.ps1 -Mode Clippy -Package engine_runtime -Features spine -Lib -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'F-runtime-clippy-run01')

./rust/Invoke-EngineBuild.ps1 -Mode Clippy -Package project_authoring_execution -Features spine -Lib -TargetDir $spine374Target -EvidenceDir (Join-Path $spine374Evidence 'F-author-clippy-run01')

```



如Gate B确定native本身需要Spine编译feature，则F同步补上；不允许F通过未含目标consumer的feature组合。增改SDK/后端包有各自scoped lint；同配置包含于后续suite的无需重复。



正式性能不能用上述dev-fast Test命令取代；Gate A在374-B冻结基于已有Compiler与Player的确切Release配方启动命令/场景参数，F严格执行并留原始样本。此处不预造新性能runner命令，也不以“命令尚未冻结”绕过性能验收。先确认足够时间/系统余量且另一条路不竞争，再开始高成本配方；预算失败保留partial，定位本功能owner，不擅自改阈值。



功能退出（2026-10-29用户调整，详§50）：§1冻结支持子集内的数值/像素/事件、作者SDK、真实消费者、独立Windows链路、默认旧入口及资源生命周期有对应证据。A–E和F功能子项完成；原规模/完整性能合同单列待验，不因功能归档获资格。测试数不替代需求覆盖。



## 12. 需求覆盖索引与复用失效



| 正式方案要求 | Gate | 关键负例/证据 |

|---|---|---|

| §6 版本/FFI/错误 | A | 错版本、双实例、mask所有权、有限失败/OOM边界 |

| §7 资源/包/Cook | B/D/E | 缺atlas页、路径越界、未知资源、增量依赖、源目录不可用 |

| §8 骨骼/deform/渲染 | B/E | weighted/unweighted/linked、镜像、裁剪、PMA与blend像素 |

| §9 混合/mask | C/E | hold/property ID、过渡中断、clear残留、跨约束拒绝 |

| §10 时钟/事件/挂点 | C/D/E | 0键/多loop/顺序/超预算、下一callback、不重复tick |

| §11 AI/SDK/工具 | D/E | schema错误、能力缺失、Applied/NoOp、旧341、新工具领域发现 |

| §12 报告/预算 | A/D/F | Off无常驻长报告、配方冻结、实际成本及容量失败 |

| §13 生命周期/交付 | E/F | 旧generation、重载原子失败、GPU安全退休、原样独立包 |



有新shader/native geometry变化则更新受影响像素与生命周期证据；只有作者文档变化不失效同源后端数值结果；只有测试变化不自动要求重建相同生产DLL。原包和新包、CPU正确性与真实GPU资格、源码与安装状态分别记账。



## 13. 文档同步、证据和归档



每Gate记录目标、改动文件、源/依赖/测试/fixture/二进制身份、命令、实际case计数、失败首因与修复、复用来源及失效边界、耗时、cleanup和剩余项。证据放本路 `.artifacts/374/` 的独立run目录，摘要写374-B；禁止覆盖失败证据，禁止把诊断报告生成加到runtime默认热路径。



不中途机械更新所有入口。合同变化先回填374/374-B，范围/owner或Gate状态实质变化再更新本文件与54。整体完成后生成 `阶段完成记录/<实际日期>-374-Spine-Animator-v1/00-总览.md`，更新49的源码能力与安装仍未升级事实、54/文档地图/阅读顺序/施工及阶段入口，移本文件到已完成并修复链接。



独立交付不是安装授权，实际Provider切换/宿主激活需后续明确范围；本文件不隐含安装步骤。未完成不能移已完成或生成总完成记录。



临时样本/Player/测试进程清理由本路负责，先保留证据再关闭自有进程；递归删除须核对绝对路径、声明父目录与reparse点，只清本路明确临时子目录，不清共享target/缓存/证据、用户项目或另一条路状态。



## 14. 施工文档自审结论



已先阅读374-A，R1–R7必须修改项均回填正式方案，R8–R10施工约束均进入Gate与激活规则；同名workflow审查不适用。本文件没有绕过M0可行性、没有借共享Animator重写3D/Sprite、没有扩大安装或游戏升级。



编制时未把计划测试写成已通过；验证按owner/直接consumer/真实呈现逐级扩大，默认不跑全workspace。当前A/B/C已通过，C主线关闭见§22；§19的B pending保留历史身份，D–F未启动。历史“待执行不可施工”只适用于激活前，不能覆盖本文件头与§15的Gate A授权。



## 15. Gate A 激活复核（2026-10-21）



工作树：C:/Users/zenghaoran/.codex/worktrees/374-spine-gatea/gameEngin，分支codex/374-spine-animator、起点acaffa8cb95bbb54d093bbf45835a59315c82da5；该HEAD旧于当前dirty开发基线，Gate A不消费其旧引擎实现。后端作为独立Cargo根（临时workspace隔离），只编译新crate和确切官方C++源码，不改主工作树rust/Cargo.toml或Cargo.lock。后续B–F接入前再同步实际依赖源码并串行整合。独立target I:/gameEngin-build/374/cargo-target，证据G:/gameEngin/.artifacts/374/gate-a-20261021-154909。



已发现FrameLoop/session/SDK相对374-A快照漂移，且A路正修改SDK/ABI；本次明确排除这些公共owner，不携带它们的未提交修改。Gate A以独立Cargo manifest执行test/check/clippy，不能使用旧workspace默认装配冒充正式集成；最终记录精确命令。A路当前范围和槽位保留，不与其争用正式性能测量。



## 16. Gate A本窗结果（2026-10-22北京时间）



Gate A通过：159个官方源码/许可blob逐字节核验；MSVC/Rust真正离线编译；11项Rust行为测试（含256次所有权构造释放与3组资源）、13项JSON Schema正负例/3组结构计数、check、零warning scoped Clippy与fmt通过。首次Clippy仅测试无效replace，已修正后重跑；源码获取/换行身份差异已定位并修复，不冒称运行时缺陷。后端、测试、fixture、合同与二进制身份分别见证据`identities.json`及`result.json`，终态日志`A-*-terminal.log`。



技术合同与配方全部写入374-B，后续能力矩阵明确区分已验证探针与待生产接入。选定MSVC安装缺少ASan库，采用官方allocator余额+重复释放与源码核查替代，未宣称sanitizer通过。现有delta step、64-bit mask参数和unsafe parser仅为隔离探针；C/B必须分别补整数权威时钟/完整mask与结构预检，不能直接对项目公开。



未进入B–F、GPU/正式性能/旧341组合/独立交付或安装；总施工保留当前目录、整体partial。B路本窗收尾，不运行后台工作。下次接入先同步实际开发基线并登记共享owner，禁止把此旧HEAD直接合并覆盖A路。主工作树仅同步374文档/导航，未改任何共享产品源码。



自有下载/编译进程已结束。早期临时克隆/锁文件清理遭自动审批策略拒绝，保留为`retained_by_host_policy`，未通过其它API绕过；后续源码通过独立安全获取完成，不依赖这些失败目录。共享target与证据有意保留；未修改安装、配置、用户项目或另一条路状态。



## 17. Gate B本窗激活与验证经济性（2026-10-22）



用户明确授权启动Gate B。窗口05:52:34–08:52:34 UTC，08:37:34停止新增。Gate A证据按身份复用，旧HEAD不是当前引擎；只读复核实际project_authoring_execution、RenderProjectionAdapter、core/native三层装配，禁止将旧workspace合并覆盖A路。共享owner仍归371路；独立后端不消费旧workspace，正式接入前另做串行基线同步。



最小因果修改为engine_spine_runtime的严格输入校验、原官方求值的安全封装及几何提取，不新造Assembler/Renderer。输入形状/引用/预算失败→owner负例；骨骼/weighted/linked/deform/skin/draworder→手算golden；裁剪生成前上界→容量及几何测试。现有A测试与同配置新增B测试在终态同次执行，fmt/scoped Clippy。先控制owner反馈；未接共享原consumer时不构建旧workspace或另造GPU harness。



本窗先投入约45分钟实现/owner验证，若共享装配仍不具串行条件，保留已验证后端与明确剩余出口，B保持partial；不改变B的完整目标和像素阈值。CPU部分命令在独立crate执行cargo test --offline --locked -j1 --test gate_b与已有gate_a，target同54；全Gate原命令仅在实际feature/消费者接入后执行。没有实际GPU结果不得关闭B。



## 18. Gate B本窗收尾：后端源码通过、完整B未关闭



31项去重测试（20 B+11 A）与零warning Clippy/fmt通过，资源/约束/geometry/纹理纯函数和119变异负例见374-B §11；证据根`.artifacts/374/gate-b-20261022-055234`。本窗只修改独立crate及374文档，159上游源和固定fixture保持；无源包、原feature/Assembler/Projection与实际GPU像素均未完成，不能关闭B或授产品能力qualified。共享入口由A持有是当前接入约束，非GPU不可用，也不是默认允许覆盖旧workspace。后续从这些具体剩余出口继续，复用已验证owner证据，不重做A。



新增admission安全限制和scratch推导见实际合同§11，不是另起渲染/包系统。正式消费者未接入前，B三条全Gate测试命令仍为待实现；本窗实际执行：



```powershell

Set-Location C:/Users/zenghaoran/.codex/worktrees/374-spine-gatea/gameEngin/rust/crates/engine_spine_runtime

cargo +1.96.0 test --offline --locked -j1 --target-dir I:/gameEngin-build/374/cargo-target --tests -- --test-threads=1

cargo +1.96.0 clippy --offline --locked -j1 --target-dir I:/gameEngin-build/374/cargo-target --all-targets -- -D warnings

cargo +1.96.0 fmt --check

```



测试进程均已结束，target/源码/证据保留用于继续施工；未启动Player/GPU/正式性能、未修改安装/配置或其它路状态。B本窗提前收尾，整体仍partial且占用当前槽，无后台施工；不是stopped_by_time_limit。





## 2026-10-23 Gate B续窗：隔离当前基线与主线接入准备



用户明确授权继续Gate B，并要求注意371正在施工。08:06:59 UTC开始；默认10:51:59停止新增，11:06:59硬截止。正式范围仍为§7，不启动C–F。



本机371阶段41正修改原loader/cache/geometry/native/Player。本窗仅在`01-Engine/worktrees/374-spine`内实现和验证现有Cook/运行包/RenderProjection/native真实消费者；主目录的共享产品文件仍归A，最终接入/合并串行。本窗不替换主线、旧workspace、安装或冻结版本。31成员当前源码单向同步的逐文件SHA、旧B外层备份、保留后端身份见`.artifacts/374/gate-b-integration-20261023-080659/baseline.json`。这是一份B隔离实现快照，不能冒称含有A后续在途变更或已合入主线。



必要性与验证成本：B的现有CPU输出尚无真实包/GPU消费者。最小变化是在原Assembler、RuntimePackage和Projection中接已冻结Spine输入，native仅消费中性几何。先做依赖/包身份负例和脱离源目录加载，再做小画面像素；不重复已验证的31项后端算法，除非其输入或源码变化。旧341与default feature验证按实际受影响范围；不运行完整workspace或正式性能配方。每条验证记录Cook/编译/执行成本与首错；本窗若因共享主线尚未交接无法合入，则准确保留B隔离验证与主线接入两个状态。





## 19. 2026-10-23 Gate B隔离验证收尾与恢复点



本窗隔离B已接通§7要求的原Compiler/Cook/包/RenderProjection/native消费者；215项去重测试和16套真实像素通过，186个后端文件未变、31项证据复用，scoped Clippy增改行无告警及fmt通过。实际记录见[隔离验证与串行交接](../../阶段完成记录/2026-10-23-374-GateB-Isolated-Integration/00-隔离验证与串行交接.md)，具体合同见374-B §12，时间/进程/源seal见本窗closeout.json。本窗收尾，无后台施工。



保留`01-Engine/worktrees/374-spine`及独立374 target。交接ZIP只含36个B增改文件和186个后端；不得把整个B快照或旧22成员外层覆盖主线。封存时A已继续修改loader/streaming及其测试，故后续必须重查handoff.json的主线漂移；共享owner串行合入后再补受影响包/默认feature/Compiler/native组合验证。B整体partial、总施工保留当前，C–F未启动；不以本窗通过开启后续Gate、安装或正式性能。



恢复顺序：读取49/54与本节→核对A共享owner交接及两边源摘要→串行合入本次真实改动→仅刷新合并影响的组合证据→满足§7出口后关闭B。后端身份未变的31项继续复用，勿重做A或重开已通过的CPU算法实现。原§18“尚无真实包/GPU”的判断只属于2026-10-22历史窗口。





## 20. 2026-10-23 Gate B主线关闭与后续边界



Gate A/B已通过，Gate B已串行合入主线并关闭；本窗260项组合测试通过，31项后端证据按186文件身份复用，16套真实GPU像素通过，scoped Clippy增改行无告警、25文件fmt通过。371阶段42的4GiB图形上限及其它既有成果保留。C–F未启动，374整体仍partial，未安装、游戏固定V0.0.5。



用户直接授权登记交接、按改动合入、组合验证和关闭B。本窗11:23:54 UTC开始，实际结束与源身份见.artifacts/374/gate-b-main-integration-20261023-112354/closeout.json。54只修改B行和B队列摘要，保留A阶段42行。222文件按写前SHA发布，geometry三方合并保留A的4GiB；其余主线源码未被旧快照覆盖。



必要性/经济性复核：请求是关闭B的主线集成缺口，最小变化是已封存36外层+186后端；重叠风险由geometry三方差异、4GiB正负例及真实像素捕获，跨owner风险由Compiler/运行包/Projection/loader直接消费者捕获，默认装配由缺能力拒绝捕获。31后端测试按186不变文件复用；完整workspace、正式性能与C–F均不为本次B出口所需。所有终态命令退出0，失败0、ignored0、源码漂移0。



具体记录见[主线合入与Gate B关闭](../../阶段完成记录/2026-10-23-374-GateB-Main-Integration/00-主线合入与GateB关闭.md)，当前实际合同为374-B §13。§18/19的隔离及pending表述保留历史身份，以本节为当前结果。总施工不整体归档；后续C若获激活，先从当前主线复核/同步独立目录，重新登记与371的共享边界，勿重跑A/B已证明且身份未变的算法。





## 21. 2026-10-23 Gate C激活与必要性复核



用户明确授权开始C。12:23:28 UTC开工，15:08:28停止新增、15:23:28硬截止；证据.artifacts/374/gate-c-20261023-122328。当前主线相对B关闭seal无漂移；独立374目录同步12个A后续文件并保留前态ZIP，不回写主线产品。371阶段43是内存研究；loader/streaming/Player/GPU及其临时诊断归A，本窗只修改独立目录的Spine后端、spine_animation控制owner与必要模块转发，合入仍串行。



请求/缺口：达到§8控制、真实显式mask与源事件出口。最小变化：原FFI补齐有界整数时钟喂入和白名单timeline深拷贝；导入owner检查mask依赖；原spine_animation新增紧凑控制表/每层当前动作与once计划。官方仍唯一混合姿态，不改上游vendor，不新增通用runner、SDK/ABI、FrameLoop、包版本或安装。



风险→最便宜能失败的验证：mask错误由全身源过滤/256位/Bezier/slot/deform/property ID及跨依赖负例捕获；时钟错误由整数长时钟/loop和实际TrackEntry相位对照捕获；混合/clear/中断由真实骨骼顶点golden捕获；事件/预算由多loop、0键、payload、暂停/混出与预拒绝状态不变捕获。先独立后端tests，再engine_runtime --features spine的spine374_c_消费者；收敛后同配置相关旧Animator2D/包消费者、scoped Clippy/fmt，不跑全workspace或正式性能。



复用：A/B现有CPU/GPU与包证据按消费身份复用；新增后端改动使对应后端行为证据需终态刷新，渲染公式/纹理Cook未改不重做16套像素。估计本窗主要实现与owner测试，昂贵编译前留足证据/收尾时间；到15:08:28停止新增，未满足全部C出口时据实保留partial，不进入D–F。





## 22. 2026-10-23 Gate C关闭与继续边界



Gate A/B/C已关闭，Gate C已串行合入主线；256骨骼显式mask、整数Tick控制/混合/中断、once淡出、源事件与模拟挂点通过，93项去重测试通过，scoped Clippy增改文件无告警、10个Rust文件fmt通过。371阶段45成果逐字节保留。D–F未启动，374整体仍partial，未安装，游戏固定V0.0.5。



本窗12:23:28 UTC开工，实际提前收尾及进程状态见.artifacts/374/gate-c-20261023-122328/closeout.json。请求/最小修改/排除项与可红测试的经济性复核见§21，完成出口与逐项证据见[Gate C完成记录](../../阶段完成记录/2026-10-23-374-GateC-Control-Mask/00-控制混合与显式mask完成.md)，实际合同374-B §14。12个C文件按写前SHA发布，未覆盖旧workspace；A45构建/导出结束后在空进程窗口串行接入，A45三个已改文件和1176个其它主线文件保留。



主线46控制/旧Animator+6 Compiler消费者、后端40+1项，合计93去重通过，失败/ignored/源码漂移均0。后端159官方源/许可加UPSTREAM清单、Cargo锁、原fixture/渲染/纹理Cook与s374_geometry实现尾段身份不变；16套B像素据此复用。当前1188文件源seal为`c4f9a57717bf54119fa553c95e2597af04246699b8c9e3f70729b4b2b1884124`，dirty源码集成，不冒称commit/Release/安装资格。隔离46项仅作开发证据，不在93之外重复计数。



Additive重复采样与中断旧mixTime的真实缺陷均以owner红绿修复；once未来empty entry预留与跨层事件总量在提交前拒绝通过。未知命令/预算失败保留已提交Pose/参数/游标；有限后端失败隔离，OOM不恢复。D的作者/Cooked控制器、SDK/session/FrameLoop/generation/下一callback，E普通项目交付和F生命周期/正式配方均未开始；不因C owner完成宣称普通项目完整功能可用。总施工继续保留当前，不整体归档；本窗自有进程已结束，证据及共享构建缓存保留。





## 23. 2026-10-23 Gate D激活与必要性/验证经济性复核



用户明确授权开始D；13:41:05 UTC开工，16:26:05停止新增、16:41:05硬截止。证据.artifacts/374/gate-d-20261023-134105；C封闭1188源seal与当前主线复核并同步独立374目录，完整SHA及漂移见baseline.json，不依赖旧HEAD。371当前A46修改loader/texture verified/prepared/ProductionGeometry/native streaming；B只改作者/Cooked Spine控制数据、SDK intent/快照、原session/native adapter/FrameLoop Spine调用与既有工具域数据，主线最终接入串行。



请求/缺口：使C真实控制owner通过schema-first作者、Cooked索引、原SDK/DeferredMutation/session与工具check/inspect/observe可消费。最小因果修改是原Compiler的v2分流与控制表编译、原registry类型化、原SDK Animator intent与纯数据socket/event查询、原session阶段/handle/原子提交和既有模拟tick调度；无新世界/Assembler/Renderer/ABI主版本/工具路由。E独立项目交付、F规模/性能/完整生命周期、安装与游戏升级均排除。



风险→最便宜可红验证：作者未知字段/重复键/名称/类型/秒到tick/非法mask→原Compiler小样本及控制表负例；SDK wire/digest/旧模块→纯SDK serde与builder/原native adapter消费者；非法phase/generation/批次不部分提交、Applied/NoOp与未初始化顺序→原session owner；固定tick只推进一次/下一合法callback/挂点→现有FrameLoop生产消费者；能力与无后端拒绝→原inspect/check/observe域数据与default消费者。先窄owner，再真实受影响组合，不跑全workspace/Local CI或正式GPU/Release配方。



复用：C控制/后端与B像素按消费身份保留；只刷新D改变的接口/消费者。SDK JSON缓冲边界不变，digest按既有机制更新，不新造C ABI。高成本构建前核对feature/cwd/target且保留收尾时间；不能在本窗完整关闭D时据实记录剩余出口，不跨入E/F。





## 24. 2026-10-23 Gate D关闭与后续边界



Gate A/B/C/D已关闭，D已按30文件增量串行合入主线；作者v2/Cooked索引、SDK意图与纯数据查询、session原子提交及固定Tick接入完成，310项去重测试/配置通过，scoped Clippy增改行无未处置告警、30文件fmt及真实窗口feature编译通过。371阶段46的11源文件逐字节保留。E/F未启动，374整体仍partial，未安装，游戏固定V0.0.5。



本窗13:41:05 UTC开工，提前收尾时刻/进程/逐文件身份见.artifacts/374/gate-d-20261023-134105/closeout.json。§23必要性与验证经济性保持：只接原作者/Compiler、Cooked、SDK/session、FrameLoop及原工具域数据。30文件按登记共享交接→三方SHA→前态ZIP→按改动发布→主线runtime/Compiler/Provider组合和窗口编译验证闭合；371阶段46的11源文件与54 A行保留。



具体实现、需求/测试映射、失败首因与修复、源码seal、构建/测试分列成本及复用见[Gate D完成记录](../../阶段完成记录/2026-10-23-374-GateD-Author-SDK-Session/00-作者SDK与会话接入完成.md)，实际合同374-B §15。SDK digest需沿原AOT重建，C ABI布局/主版本保持；旧模块测试通过不表示冻结DLL可跳过digest。FrameLoop原生模块保持线程内，不将native指针传项目session；包tickRate不匹配明确失败。



D只有源码功能和受影响主线集成资格。E普通项目动态呈现/交付、F完整生命周期与三组正式配方、安装和游戏升级均未启动，总施工不整体归档。后续授权E后复核主线和371最新状态；仅同步必要基线，继续独立target，身份未变的A–D证据按消费路径复用。





## 25. 2026-10-24 Gate E激活与必要性/验证经济性复核



00:36:32 UTC开工，03:21:32停止新增，03:36:32硬截止。证据.artifacts/374/gate-e-20261024-003632。主线D证据与371阶段47–50最新源码已复核，必要基线同步到独立374目录，前态及SHA见baseline.json。54 A行保持；A已续工阶段50，real_window.rs与终止测试归A，B冻结复制到的稳定A50前态、不追随A临时红实现；共享production/EngineHostLoop的Spine接缝本窗由B串行接入，保留371网格源回收、预算与终止保护，不修改A的loader/backend/streaming owner。



请求/已知缺口：普通RuntimeModule通过原Compiler/playtest/verify消费动画、mask、deform、skin/attachment、事件与挂点，动态几何真正进入现有RenderProjection和Player，并将原样产物复制到无源独立目录运行。最小因果修改为既有Spine adapter到生产投影的帧数据接缝、原样本与直接消费者；不新增Renderer/Bridge/导出桥/通用runner。排除F正式Release性能和完整生命周期、Provider安装、既有游戏升级。



风险→最便宜可红验证→终端证据：动态pose未投影→同一帧循环和生产投影直接消费者→顶点随tick变化及真实GPU图像；mask/事件/挂点与业务脱节→原SDK项目模块与Spine快照/RuntimeModule断言→playtest/verify语义；打包偷读源→原Compiler导出后独立目录断源运行→真实Player结果和截图。先owner红绿，后普通样本一次完整构建，分列Cook/构建/运行时间，不用全workspace或F性能循环调试。



复用A–D身份未变的后端数值、静态像素和作者/SDK合同；本窗接缝变化只失效直接动态消费者。预估owner开发验证60–90分钟，样本构建/运行30–45分钟，接入/收尾30分钟；异常编译/运行先保留首错并返回最小owner。超过预算不靠增加runner或降低断言过关，出口不全则保持E partial，停止新增及硬截止按本节执行。





## 26. 2026-10-24 Gate E关闭与后续边界



Gate A–E已关闭；E的22个文件已增量串行合入主线，普通RuntimeModule样本的动态几何、局部攻击/显式mask、deform、换肤、事件和挂点特效完成真实消费。41项去重测试通过，主线scoped Clippy增改行无告警、8个Rust文件fmt通过；WindowsDev原产物与42文件独立副本运行及截图一致。371并行成果保留，后续只读内存观测差异单独审计。F未启动，374整体仍partial，未安装，游戏固定V0.0.5。



本窗00:36:32 UTC开工，原03:21:32停止新增、03:36:32硬截止不变，提前收尾见`.artifacts/374/gate-e-20261024-003632/closeout.json`。§25请求、最小因果修改与排除项保持。22文件通过写前SHA/前态ZIP串行合入，900个非E Rust源/配置在发布时未变，主线25渲染和2项目消费者、scoped Clippy/fmt及已显式通过的完整WindowsDev交付构成终态。



详细需求/测试映射、初次失败、修复、实际Cook/编译/运行分列、189文件后端复用、42文件副本与图像一致性、371后续观测差异审计见[Gate E完成记录](../../阶段完成记录/2026-10-24-374-GateE-Ordinary-Project/00-普通项目动态呈现与独立交付完成.md)，当前合同374-B §16。B不改A的real_window/loader/backend/streaming，完整交付消费A51封存native模板；未覆盖A后来内存观测扩充。Git index未写、原锁保留。



只有E关闭；总施工整体partial并保留当前，F完整生命周期/正式Release配方及安装/游戏升级均未开始。自有Cargo/Player结束，无E后台任务。





## 27. 2026-10-24 Gate F激活与必要性/验证经济性复核



用户授权开始F；01:52:11 UTC开工，04:37:11停止新增实施/验证，04:52:11硬截止。证据`.artifacts/374/gate-f-20261024-015211`。E完成seal、22增改及当前主线/371 A路已复核，独立目录同步稳定最新依赖，A在途foundation F11测试保持隔离。主线真实路径及独立374 target延续，不写Git index、不重置旧改动、不安装或升级游戏V0.0.5。



请求/风险：满足§11生命周期与三类正式配方；最小变化为原Spine session的实例/暂停/重载及原生产几何资源寿命缺口，补直接owner和原native真实完成点消费者；原Compiler integration增加WindowsRelease配方。没有新Renderer、Bridge、Assembler或通用runner。A的real_window/loader/backend/streaming仍归A，若实证要求修改共享测量接缝，先登记具体归属并串行接入，不能另造旁路假装正式Player。



风险→最便宜可红测试：实例/禁用/暂停/重建/原子失败与旧事件→原session数据/实际geometry；资源泄漏/提前复用→同一生产投影加原manager lease与真实GPU提交/等待；规模/预算→冻结recipe结构实算与原Compiler WindowsRelease/Off消费者。先owner红绿和受影响组合，再一轮实际组成；昂贵失败回到最小owner，不循环全workspace或用dev替Release。



A–E后端/公式/静态像素/作者SDK证据按实际身份复用。新测试不重建未变生产二进制；新Spine资源寿命变化只刷新对应consumer。原§8硬件i5-12490F/RTX3070与阈值保持；本机i9-14900K/RTX4090另记资格，不冒称原硬件通过。正式规模、600预热/3600采样×3、Off与其它所有阈值不降低；GPU性能避开A竞争。



预计生命周期实现/owner验证60–90分钟，正式消费者准备/Release构建及配方60–90分钟，预留至少15分钟收尾；缺组成/预算失败先保留具体证据与剩余出口，不机械追加runner、降低阈值或宣称整个374完成。总施工仅在所有F出口实际证明后才整体归档。





## 28. 2026-10-24 Gate F本轮：生命周期合入，正式资格partial



Gate A–E已关闭；F的10个Rust文件已增量串行合入主线，完成World替换隔离、动态内容key回收和同定义只读表共享修复。71项去重测试、默认check、4组scoped Clippy及10文件fmt通过，增改行无新告警。single WindowsRelease实际4200帧与43文件断源副本120帧通过运行，但正式资格缺Spine四段CPU/专属GPU计时；测量关闭仍观测private持续增长，根因未定位。三组×三轮、空场景基线和完整预算尚未闭合，F/374仍partial。371最新成果与登记保留；未安装，游戏固定V0.0.5。



本窗2026-10-24 01:52:11 UTC开工，04:37:11停止新增、04:52:11硬截止保持；实际提前收尾见`.artifacts/374/gate-f-20261024-015211/closeout.json`，无F后台施工。主线组合29 runtime、18资源、6真实GPU、18项目测试通过；2项显式导出/Release ignored另记，隔离重复不累加。189后端文件未变复用41项；无新增C++。



Release首轮prepare/build/组成verify/采样分别1.952/125.550/74.600/75.353秒；组成及进程通过，资格断言失败不伪装性能通过。single测量样本private 566,943,744→942,366,720 bytes，tail1200拟合134,084.55 bytes/frame；独立Off/测量关闭运行仍持续增长。census全部partial，不能把缺口归零或猜归驱动/native leak。其余昂贵配方按资格缺口fail-fast停止。



详细需求映射、首错修复、命令/成本、43文件完整SHA、主线与隔离Release身份分离及恢复点见[Gate F本轮记录](../../阶段完成记录/2026-10-24-374-GateF-Lifecycle-Qualification/00-生命周期修复与正式资格缺口.md)。先在最小owner定位内存持有，再按54登记原测量接缝的具体共享归属，补四段CPU/对应GPU pass、同二进制空场景、三组各三轮、完整预算与Summary/Trace开销。原硬件/负载/阈值不改，本机另列资格。不得在预算评价未实现时删除显式失败guard后假绿；F不关闭、总施工不归档。





## 29. 2026-10-24 Gate F内存定位续窗



用户按建议授权继续Gate F。03:38:28 UTC开工，06:23:28停止新增、06:38:28硬截止；证据`.artifacts/374/gate-f-memory-20261024-033828`。前窗已正式收尾，本窗重新授权计时。先保持前窗Release身份，不为诊断预先重Cook/export或全量重建。



开工时已证实single在Off/测量关闭下private持续增长，当时根因未知；先在原session/production资源与持有关系查最小反事实。最便宜排除边界为冻结配方多周期的官方allocator/原HostLoop消费者；必要时同二进制空场景用于区分Spine与通用Player。没有信号不新增通用probe/runner；诊断脚本均run-owned。每个分支先给10–15分钟探索预算，信号变化后才进入下一层，不能把真实窗口/完整编译当owner调试循环。



最小修复仅限确定的owner及直接消费者；保留首红，先owner红绿，再一次受影响组合与必要原包Release复核。前窗10文件、71测试、189后端/41项和43文件交付按实际身份复用，测试脚本改变不让未变生产包自动失效；改生产源只刷新其消费者。正式四段CPU/专属GPU计时、空场景、三组×三轮及全预算仍是F出口，缺失时保持partial，不降低阈值或假绿。具体共享计时或native修改先写54归属、串行接入，371 A在途文件不覆盖。估计owner定位/修复及直接验证60–90分钟，剩余时间只在资格与成本可控时推进正式出口，预留收尾。





## 30. 2026-10-24 Gate F已确认内存修复合入，正式资格仍partial



Gate A–E已关闭；F本窗已修复wgpu26.0.1 RenderBundle释放及四类缓存的失效持有/元数据回收，196文件按改动串行合入，371成果保留。168项去重主线组合、默认check、9文件fmt和scoped Clippy通过，增改行无新告警；最终WindowsRelease完整40文件副本及single/同原模块空场景各4200帧正常退出。统一5–65秒外部采样single增长7,495,680 bytes、空场景3,821,568 bytes；末尾突降另列原始数据，少量余量仍未完整归因，不授零增长或正式预算资格。四段CPU/专属GPU计时、三组各三轮、全预算与开销仍pending，F/374保持partial。未安装，游戏固定V0.0.5。



§29必要性/分支预算保持：原bundle及三个缓存首红后先owner红绿，原HostLoop又确认实例证明表dead header并修复；最终主线131 geometry、12 native、7 template、18 project按影响面和身份计168项，未重复累计。完整记录见[内存修复与余量边界](../../阶段完成记录/2026-10-24-374-GateF-Memory-Repair/00-内存泄漏修复与余量边界.md)。



正式四段CPU/专属GPU、三组各三轮及完整预算与开销仍缺；少量余量未完整归因。实际Release只授运行/定位，原资格guard和阈值保持。总施工保留当前、B槽保留，未安装，游戏V0.0.5。本窗提前收尾，准确时间、源身份、Git及进程见.artifacts/374/gate-f-memory-20261024-033828/closeout.json。



## 31. 2026-10-24 Gate F正式测量接入续窗

用户授权下一步。06:25:11 UTC开始，09:10:11停止新增、09:25:11硬截止；证据.artifacts/374/gate-f-measurement-20261024-062511。前窗已收尾，本窗重新计时。读取49/54及原合同§8；A当前收尾无后台，启动前只读进程检查无Cargo/rustc/Game。主线额外13个371文件按SHA/前态备份同步到独立374目录，包含最新生命周期、报告观测与故障测试；不以旧隔离源替换当前主线。

必要性：既有正式入口明确拒绝缺四段CPU及Spine专属GPU样本的运行；当前原measurement只采整帧/粒子。最小改动在原Spine control/session/production、HostLoop、native编码及Player原CPU/GPU采样结构接入可选字段，不新建报告系统/Renderer/通用runner/schema代际。共享文件归属按54 B行，源先隔离验证，再按改动串行接主线；保留371其它所有改动。

风险→可红验证：测量Off有额外计时或旧值→owner Off/启停/失败/多tick计数；CPU嵌套双计→不交叠分段与原控制/几何真实消费者；GPU误计普通几何→混合draw分组、粒子独立及真实timestamp消费者；Player漏传/历史错帧→原raw sample生产/过滤消费者；正式出口→原Compiler WindowsRelease/冻结三配方、完整预算评价，不删除budget_pending guard冒绿。当前机器i9/4090只能另列本机资格，原12490F/3070资格不外推。

验证经济性：前窗168与41后端及Release内存诊断按消费身份复用；变更只刷新直接消费者，不重跑全workspace/安装/Local CI。预计测量接入及owner验证60–90分钟，Release/必要实际配方30–60分钟，预留≥15分钟串行接入与文档收尾。新诊断分支10–15分钟无信号停，昂贵构建失败回owner，不机械追加全量重建；正式配方的内存余量、同二进制空场景、三组各三轮、Summary/Trace成本与完整预算未通过时F继续partial。


本窗实现补充（正式结果待实际运行）：既有process sampler直接复用system_available_bytes，不新造系统观测。既有spatial GPU样本补native geometry的parameter/index/clear累计写入数，与原逐帧mesh/texture上传及UI写入分开合计；不误用RetainedUi参数统计冒充Spine上传。显式measurement复用原native物理admission的backing-only census作为全后端requested buffer/texture保守上界，含resident/in-flight/retiring；不走CPU payload/长JSON或分配记录history，不称驱动heap/外部surface完整VRAM。Off不执行该census，Summary/Trace成本仍须另测。

合法混合来源实例化batch保持原绘制，但同一native draw无法拆出Spine专属GPU时间，样本显式计spineMixedSourceBatches并拒绝资格；只看selected rows，不把invisible rows计入。真实GPU测试补Off完整range缓存→按来源分段→Off恢复及半透明像素逐字节一致；预算离线owner补nearest-rank、8GB偏置线性拟合、单帧上传硬上限、warmup系统余量和tail1200增长正反例。CPU projectionEncode明确覆盖Spine中性顶点/索引与Prepare，不冒称通用native编码的独占CPU计时。

首轮Release资格被拒：single实测mixing chain达到3而合同为2；原121tick owner未覆盖第二次周期的30tick中断，扩展361tick在同一controller重现。最小修复限已登记control.rs：在新操作恰逢原mix完成边界时，先让原官方sample提交完成态，再由原官方update/apply回收零权重旧entry；不改vendor、不改冻结中断点/链上限，不引入独立mixer，该额外官方工作计入Official段。另确认二维场景render identity的3D tickPair恒0，周期资格改读原Spine成功advance记录的首/末fixed tick；render identity仍负责CPU/GPU对应。此处是修正验收来源，不降低增长或时间预算。首轮CPU P95及private tail增长超限保持失败；不盲目重编其余配方。

最终single首轮已具完整测量输入，同二进制空场景tail1200也增长约3491 bytes/frame。为区分普通Player/功能增长与测量路径，主线组合结束后仅复用前窗player-memory.py，对当前确切Game/DLL/模块及同一single/空场景各做一次Off、measurement关闭、4200帧外部采样。分支上限10分钟；无新Cook/编译，不改已有二进制/包，不把外部墙钟样本冒称逐帧拟合或正式资格，不因对照而扣除原增长阈值。

## 32. 2026-10-24 Gate F测量合入与预算阻塞收尾

Gate A–E已关闭；F四段CPU/Spine专属GPU及原Player原始采样、完整预算评价与同二进制空场景已接入，20文件串行合入且保留371成果。211项去重主线测试、20文件fmt、默认check及scoped Clippy通过，改动行无新增告警。single配方首轮在修复后复核中具完整测量输入，但Spine CPU P95=0.8018ms>0.5ms、tail1200 private增长=4545.32 bytes/frame>1024；后续重复/配方按失败停止。F/374仍partial，未安装，游戏固定V0.0.5。

06:25:11 UTC开工，本轮于2026-10-24T08:09:12.791324+00:00提前收尾；原09:10:11停止新增及09:25:11硬截止不变。详细来源、211项去重清单、两次Release失败与因果修复、同二进制空场景、Off外部对照、371与Git保留见[本轮记录](../../阶段完成记录/2026-10-24-374-GateF-Measurement-Qualification/00-测量接入与预算阻塞.md)。剩余CPU、增长、三组×三轮、成本与最终产物资格分别保持未完成；不归档374，不安装或升级游戏。本轮无F后台施工。

## 33. 2026-10-24 Gate F原owner成本与增长归因续工

用户明确授权在371施工期间继续可隔离部分。09:13:57 UTC开始，11:58:57停止新增、12:13:57硬截止；证据`.artifacts/374/gate-f-owner-20261024-091357`。A当前通用几何/渲染链归属及主线dirty保持，B在原374独立目录/target操作，原vendor、冻结配方/预算及游戏V0.0.5保持。

必要性：前轮single真实CPU P95 0.8018ms超0.5ms，vertices/projection各自P95 0.3520/0.3936ms，private增长4545.32 bytes/frame超1024。不能把分段P95相加或把全部增长叫Spine泄漏。源码发现小几何每次先按32768顶点/98304索引初始化Rust输出；先用真实backend小几何预留量首红确认，再在原Instance容量选择处修复。仅容量提示可变，不缓存pose、不改变官方控制/采样、导入上限或容量拒绝语义，不改C ABI或vendor。若容量修复不能解释剩余成本，返回实际owner分布，不凭猜测改通用缓存。

风险→验证：小输出仍申请最大空间/容量提示滞后→现有gate_b真实几何、大小附件切换及显式低容量拒绝；pose/顺序/裁剪改变→原backend gate_b/c与真实Spine消费者。projection编码优化仅在原Spine文件内证明与原中性字节/资源key一致后实施。普通Player和measurement增长先读原持有链，确有其它owner原因再登记，不跨A文件。

验证经济性：原211项/41后端按未变身份复用；只刷新改变owner及直接消费者，CPU正确性可并行，当前机器竞争期不得作为性能资格。预计owner首红/修复30–60分钟，直接消费者/fmt/scoped Clippy30–45分钟，必要无竞争Release复核15–30分钟，保留≥15分钟收尾；若A测量/构建仍在进行，保留源码结论与正式性能pending。不以等待为由扩大矩阵，不清target、不写Git index、不运行全workspace/Local CI/安装。任何Release按前轮正式完整预算fail-fast，失败不继续其余配方。

本轮最小范围补充：真实交互窗口的6个CPU测量history均从Vec::new逐帧增长，而同文件limited路径已预留frame_limit。原AuiFrameWorkCounters含固定大小ResourceMemorySample等内嵌状态，不能把其增长直接归于Spine泄漏。B原Player接缝在A行明确只读，现登记lib.rs/real_window.rs及原foundation_cpu_probe.rs：先保留原Vec::new构造路径形成4200帧backing扩容红例，再按已有MAX_CAPTURE_FRAMES约束在启动时预留；普通Off零分配且不求值closure，Off显式CPU probe仍保留有界历史。所有帧/身份与原始样本保留，不扣除旧预算、不声称修复Off路径增长或全部private增长。无GPU依赖，此源级分支及验证预计15–25分钟。

## 34. 2026-10-24 Gate F原owner分配修复收尾

Gate A–E已关闭；F本轮按6文件串行合入Spine默认输出容量随实际几何调整，以及Player有界测量history启动前预留/普通Off零分配。272项不同测试通过，其中主线76项去重组合；6文件fmt和scoped Clippy通过，增改行无新增告警。371当前源码与A行保留。未在371活动窗口启动GPU/正式Release复测，上一版single CPU与增长失败保留为旧源码结果；新源码预算未测，F/374仍partial，未安装，游戏固定V0.0.5。

本轮于2026-10-24T09:40:13.515857+00:00提前收尾，09:13:57起计时不重置；原11:58:57停止新增/12:13:57硬截止保持。详细首红/修复/身份、组合期间A差异及固定出口见[本轮记录](../../阶段完成记录/2026-10-24-374-GateF-Owner-Allocation/00-原owner分配修复与并行交接.md)。F继续partial，原总施工保留当前。

## 2026-10-25 B槽调整
用户当时明确：网页版接替B槽，保留374待续状态；该历史调槽已于2026-10-26本轮重新激活结束，全部成果及失败证据保留。



## 35. 2026-10-26 Gate F隔离Release复测续工

用户明确开始下一轮。11:26北京时间起计，14:11停止新增、14:26硬截止；证据`.artifacts/374/gate-f-resume-20261026-032600`。从待执行移回当前并占B槽；A已激活371 S3，本轮不将A在途源码整包同步到W，也不以旧W覆盖主线。W与上轮修复后的1325文件SHA一致，可复用272项源码验证；主线其后371/375/S3漂移单独保留。

必要性与最小范围：上轮容量与CPU history修复已发布但未生成新Release，旧0.8018ms/4545.32bytes每帧不能代表新源码。先用原Compiler和原冻结single配方构建一次新WindowsRelease、真实4200帧及同二进制空场景，再按原完整预算评价。消费输入冻结，使用既有374 target/Compiler缓存，CARGO_BUILD_JOBS=1；不新增通用runner、schema、缓存或替代渲染器。

风险→验证：旧二进制冒领新源码→源前后SHA与实际build回执/完整交付清单；CPU/增长未达标→原gate_f_budget与原逐帧样本；机器竞争→采样期间每秒记录Cargo/rustc/Player活动，存在A重负载则只授诊断、不授正式资格。single失败即停其余重复/配方，并回最小owner红例修复。只写Spine专属geometry/control、core render_projection_adapter/spine.rs、render_geometry/production/spine.rs及直接tests；共享SDK/schema、World/session、native由A持有，若确需变更另行登记串行边界。

验证经济性：源未变不重跑272项；若修复只更新owner和直接consumer、定向fmt/scoped Clippy；必要的新Release只在因果输入变化后复核。预算全绿后才继续原三配方各三轮；Off/Summary/Trace成本、最终无测量交付与原硬件资格仍独立出口，不删除最终pending guard。预计首次构建/原single及空场景15–30分钟，失败owner分支10–15分钟无新信号即停，预留15分钟收尾。没有实际formal通过不关闭F、不归档。安装/外部配置/冻结游戏/375服务均排除。


### §35本轮中断终态

374本轮恢复时与375全部候选作品接入发生B槽并发登记冲突；371 S3和375保留，374已停止自有构建并退回待执行，等待用户明确顺序。W的1325源文件与上轮分配修复后身份一致、无产品源码改动，既有272项证据仍保留。测试入口构建159秒成功；新Release已成功导出，组合Player在首末观测相隔约52秒后因槽位冲突人为中断（父Cargo exit15）；未完成4200帧组合验证或正式采样，无新预算结论；不是产品编译或性能失败。F/374仍partial、未安装、游戏V0.0.5保持。 详见[中断交接](../../阶段完成记录/2026-10-26-374-GateF-Resume-Conflict/00-并行槽位冲突与中断交接.md)。§35前文为本轮原计划；当前不再激活，未产生正式性能结果。


## 36. 2026-10-26 空槽确认后的Gate F恢复

2026-10-26用户确认371/375均收尾并明确恢复374；13:54:23北京时间开工，16:39:23停止新增、16:54:23硬截止。独立374目录/codex/374-spine-animator及374 target；原1325源码与38文件Release交付SHA未变，沿原Compiler缓存完成single/同二进制空场景预算，失败则回原owner最小修复。B负责Spine专属源码和原Gate F直接消费者，任何必要共享接入先登记后按改动串行合入，保留371 S3和375成果；原预算、三配方各三轮、成本/最终产物/硬件出口不降。未安装，游戏V0.0.5保持；证据.artifacts/374/gate-f-qualification-20261026-055423。

必要性：§35中断发生在组合验证，成功Release并非失败产物；先按不变身份复用缓存和已有产品二进制，不重跑272源码测试，不同步主线后续S3源码覆盖隔离输入。原spine374_f_release_recipes作为唯一配方入口，原Compiler正常发布本轮fixture；engine缓存应复用，实际build回执另记。先一组single/同二进制空场景，每组4200帧，完整预算失败停止后续配方，不将Trace诊断等同Off资格。

风险→便宜验证：缺失/变动交付→逐文件SHA及原编译消费身份；运行/样本/预算失败→原bounded-child/typed raw/budget消费者，保留首因；CPU或增长剩余→原分段和同二进制反事实、原owner红例后最小修复；机器竞争→run-owned进程采样，只在无其它构建/Player竞争时授本机结果。必要源修复只刷新受影响owner/consumer、fmt/scoped Clippy及一轮必要Release。

验证经济性：已有272测试按1325文件身份复用；无新源不重跑。预计首组含缓存发布与三段运行5–10分钟，三配方预算只有上游通过才继续；owner无信号分支10–15分钟止损，≥15分钟收尾。全部配方/成本/最终无测量及原目标硬件出口未闭合时F仍partial，不删除最终pending guard；本机i9/4090结果不外推12490F/3070。共享源码合入以主线现状三方审查，不以旧W替换主线。


### §36 本轮确定性修复边界

首轮完整raw存在6个已完成draw的零时间戳区间；native成功readback允许相等stamp，预算消费者不应把量化零误认丢失。以原budget最小红例修复，同时继续拒绝负数/非有限值、无draw、混源和错帧，不改变时间预算。复用原4200帧single样本，仅补同二进制空场景及离线重评，不重导出/重跑single；三项CPU/增长失败保持。

single投影P95 1.294ms，发现其动态mesh每帧把原始vertex/index bytes展开JSON数字数组后做私有资源摘要。最小改动只优化Spine内部临时mesh key：SHA256按域和各块长度分帧，直接包含原始bytes和所有中性mesh元数据；material继续原摘要。该key只供本进程资源表，不是包资产digest或持久cache协议；不新增schema、cache、pose权威，不改几何/材质/导入/透明顺序。此前要求原JSON key逐字相等的实现约束替换为全内容敏感且无边界碰撞，Cook/外部身份保持原合同。测试覆盖全部字段、原bytes、分帧边界、动态资源代际与冻结packet，正式CPU改善仍由原Release窗口决定。

同二进制empty tail969.74、single1702.31 bytes/frame；Player六history此前只reserve尚未初始化页。已登记Player lib.rs及原foundation_cpu_probe测试：在显式有界采样启动前通过Default初始化全部位置并clear，记录仍从frame1开始，Off零分配且不求值。先以Default构造计数首红证明预初始化，保留原4200帧容量/顺序测试；不减扣实际private增长，不声称全部增长均由此解释。必要新Release只在owner及直接消费者通过后运行一次原入口，失败仍fail-fast；不重跑未变owner的272测试。


§36来源更新：两处owner修复收敛后，按4文件增量三方接主线并补实际S3/设备音频现状的受影响组合；本轮唯一新Release改为消费合入后的主线真实源码，仍使用374 target与Compiler artifact根。隔离W旧Release只保留首错/反事实身份，不外推最新主线。该变更避免为已知不同主线再次资格旧组合，不扩大配方、安装或游戏范围。


### §36 触页反事实否证与S4并行状态修正

Windows原HeapAlloc与本次history同量级7291200 bytes反事实：分配即增加private commit 7311360 bytes，随后完整memset触页增量0。此前history预初始化没有被证明可降低private增长，已撤回Player lib.rs与foundation_cpu_probe测试的本轮改动，保留上轮已证实的容量预留/Off懒求值。constructor计数首红仅证明初始化次数，不能冒充内存因果证据；不把这项无效思路发布为修复。最终产品修复保留Spine动态mesh摘要与GPU零区间预算消费者。

A于13:59:58在另一聊天获用户十小时S4授权，新增C槽；本轮保持其当前登记及在途文件。该变化发生于B恢复之后，不能覆盖A/C或把最初空槽快照当后续空闲资格。4文件接入时按当前A/C行保护；Player两文件随后按SHA撤回本轮变更，S3设备音频保留。当前不启动昂贵且输入不稳定的主线Release，也不与A编译/采样争正式性能。

受影响主线组合首错还暴露两处开工前已存在的S3单测夹具：f04_tests.rs缺MeshProjection.deformation，native_template.rs死header测试使用旧两元key。已逐SHA证明这两个文件自B开工未改，首红见main-core-with-s3-libs.log；按54登记只补None与三元组默认值，不改S3实现/设计/其它测试。既有Jolt库通过显式会话JOLTC_LIB_DIR只读复用，首轮遗漏环境失败不记产品故障。修复后重跑原受影响组合及这两处必要消费者；该成本不等于正式性能资格。

### §36 原观测历史4096容量边界分支

原single与同二进制empty在4097帧均出现private跃升；原native present历史Vec与GPU capture VecDeque均从零容量逐帧扩容，逻辑上限虽已配置4200/4216，未预留backing。此处可在owner精确重现跨4096扩容，不把所有private增长都归于它。先在原owner测试中证明所需容量未在启动阶段保留，再仅按已有有界请求预留：native present按Trace过滤区间与frame_limit交集，Summary无timing/Off不分配；capture只在启用时预留，运行中改容量保持旧截断/omitted及token语义。B另负责原runtime_player_winit/src/native_present.rs的测量历史构造，以及engine_render_native/src/wgpu_backend/real/lifecycle.rs的capture启用/容量预留接缝和同文件直接测试；不改提交/完成/退休/几何及S4接口。

验证经济性：owner容量首红→最小reserve修复→同文件消费者/既有真实lifecycle直接消费者及scoped fmt/Clippy；功能GPU仅创设备与小提交，不进行正式性能采样。新探索10–15分钟无有效首红即停止。主线组合当前执行结束后再合入；按SHA保护并三方接入，保留A/C。源码测试只授扩容行为修复，新的private/CPU预算由后续空闲原Release决定。

§36组合接入补充：切换W/M同一374 target后发现Cargo相对dep-info复用了W旧engine_content rlib；M该模块导出自开工SHA未变，并非A缺少注册。按8个内容有差异的本地crate仅刷新入口mtime、逐SHA确认内容不变，使Cargo正常重编，未清target/安装/改配置。首错保留main-capture-capacity.log，修复回执main-cache-freshness-repair.json。随后真实S4新增MeshProjection.presentation导致原B持有f04夹具缺字段；在54扩展该test-only默认值范围，只补presentation:None（与生产适配器默认相同），不改S4生产实现。首错main-capture-capacity-fresh.log，必要组合再跑一次。

## 37. 2026-10-26 Gate F摘要与观测历史修复收尾

Gate A–E已关闭；F本轮6文件按改动串行合入Spine动态网格二进制分帧摘要、合法GPU零区间判定、present/capture有界历史启动前预留及S3/S4旧测试夹具默认值修正。67项去重主线测试通过，4源fmt与scoped Clippy通过，增改行无新增告警；触页猜想经Windows反事实否证已撤回。原Release single/同二进制empty完整预算仍有CPU与增长三项失败，修复后尚未生成Release；A/C正进行371 S4，不竞争正式性能。F/374保持partial，未安装，游戏V0.0.5保持。本轮已提前收尾，无B路后台施工。

下一步在371 S4输入稳定且机器空闲后，从主线当前源码经原Compiler生成一次WindowsRelease，先完成single及同二进制empty各4200帧的原预算；通过后才继续三配方各三轮、Off/Summary/Trace开销和最终无测量交付。原12490F/3070硬件出口独立保留；不删除最终pending guard，不降低预算。

详见[本轮记录](../../阶段完成记录/2026-10-26-374-GateF-Digest-Qualification/00-摘要优化与资格复核.md)；最终6文件、分批67项来源、共享漂移、触页撤回、主线缓存恢复与Git身份见证据final-publication.json。文档收尾2026-10-26T07:28:41.617308+00:00；实际进程终态见closeout.json。本轮不归档374总施工。

## 38. 2026-10-26 用户五小时授权完成374续工

用户明确授权继续施工直到374完成，最长五小时；09:52:58 UTC/17:52:58北京时间起计，22:37:58停止新增、22:52:58硬截止（北京时间），中途continuation不重置。A/C的371 S4现已收尾，开工进程检查无Cargo/rustc/Game；使用主线main真实S4/Web组合与374 target，保留W历史隔离源，不把旧源码同步覆盖主线。证据.artifacts/374/gate-f-completion-20261026-095258。

必要性：§37摘要和观测预留源码已绿但无新Release；旧失败不能代表新源码。最低因果动作是原Compiler WindowsRelease一次，先冻结single及同二进制empty各4200帧完整预算。原67项按消费身份复用，不机械重跑。若失败，读取互斥CPU四段/逐帧内存与原owner，最小首红修复，保留原负载和阈值；不得凭猜测新增缓存、schema或runner。每个新诊断分支10–15分钟无新信号停止，昂贵producer失败先区分环境/输入/编译/运行首因。

推进顺序：single完整预算→其余重复及regular/clipping三轮→Off/Summary/Trace成本真实消费者→原无测量交付及独立完整副本→全部Gate需求证据审计。硬件/ASan等能力边界按现有合同记录，不以本机4090冒称12490F/3070通过。只在全部原出口被证实满足时关闭F/374；预算或资格仍缺则保持目标active并如实推进。最后≥15分钟保存证据和结束自有进程；不终止他路/服务、不改安装/配置/冻结游戏、不删除pending guard冒绿。

验证经济性：预计主线新S4首次Release/driver编译15–30分钟，single/empty约3–5分钟；成功矩阵约30–45分钟，各必要修复分支另按实测预算。本轮五小时内持续推进实际剩余项，不把局部修复完成替代374整体目标。原Compiler消费输入在导出期间冻结；构建与Player分开计时，身份按源码/测试/产物/外部状态分别绑定。


### §38 Off正式预算消费者修正

本轮新Release single CPU四段合计P95/P99=0.2601/0.2988ms已满足原限；Trace私有尾段1348.77bytes/frame、全程available最低4720680960bytes失败保留。同二进制empty尾段338.84bytes/frame。复核§F原合同要求Release/Off正式预算、Summary/Trace开销单列，而现有入口只运行Trace并让其观测历史增长阻断Off，属于验收消费者缺项。最小修正为原Player显式Off+cpu_probe已有逐帧内存保留全程余量与tail1200线性拟合紧凑统计；普通Off无probe继续零采样，不新增policy/schema版本、缓存或pose权威。原core windowed_player采样窗口、Player measurement/real_window及foundation_cpu_probe测试归B串行，保留S4/Web行为。

正式预算分开消费Off共同CPU/内存与Trace配方身份、Spine四段/GPU/上传/backing；Trace原预算原样保存作成本，不拿它的全程available/历史增长替代Off结果，也不减扣增长。Off必须4200帧/600+3600、全程4.5GiB、tail1200<=1024、同二进制Off空场景增量峰及整帧P95/P99原限；Trace缺失/错actor/帧/CPU-GPU身份仍拒绝。最便宜红例：Off高偏置1024边界/1025失败、warmup单帧余量失败、缺样/错帧拒绝；验收消费者测试保证高Trace历史成本不能替代或掩盖Off真失败。先owner/消费者再必要一次Release；不重跑未变67项，不改原硬件资格guard。

### §38 Off probe渲染身份持有诊断

Release-02 single Off+cpu_probe正式尾段1253.503462bytes/frame仍失败，CPU与其它内存限通过。完全相同二进制普通Off、无测量4200帧正常；外部尾20秒墙钟拟合约807bytes/frame，仅诊断、不授逐帧资格。已知Off probe每帧将完整RenderFrameIdentity及动态geometry identity_views Arc保留到结束，而正式Off只需采样窗口首末tick；这是可精确复现的额外持有。B扩展登记Player lib.rs内部AuiFrameWorkCounters的compact tick pair、measurement identity capture及原foundation_cpu_probe直接测试，real_window按同接缝消费；保留Summary/Trace完整身份与371 firstTick/lastTick，不改公共wire/schema或普通Off路径。先Weak持有首红，最小修复后验证直接消费者；只能由必要的新原Compiler Release证明是否解决真实增长，不能以owner释放证明全部预算通过。其余真实增长如残留仍回owner，原1024上限不改。

### §38 实际持有owner与全帧峰值缺口

Release-03 Off尾段1231.948632bytes/frame仍失败，不能将compact identity修复当作主要增长因果。临时现成HostLoop/GPU owner计数显示600→4200 Rust live增加3890636bytes；native allocator稳定，CPU owner也有较小增长。第3001帧分配且存活至4200的原始调用栈经本机已有Android LLVM符号化：184B来自GeometryBuildInputs::instance_data的DrawInstance，168B来自native InstanceValues::resolve的RecordKey；12/14/12B来自GpuTextureDescriptor::from_surface_target，实际registry逐帧存储描述记录。临时全局allocator仅测试接缝，原文件已逐SHA恢复，诊断不授Release资格。

B串行新增两个已确认owner：core gpu_texture_lifetime.rs每target仅保留最新成功帧记录（原不可变报告仍带该帧事件，registry不拥有实际GPU对象）；native geometry/parameters.rs在选择边界按增长阈值回收已死亡weak实例及参数键、重建内部索引，活跃实例复用/选择serial/溢出与GPU dirty不变。先同文件小输入红例，后原owner及直接消费者、必要一次新Release。另审计原raw预算峰值含预热，新Off采样窗峰值不含预热；在原采样window追加optional全请求帧peak供本消费者，保留371现有sampled peak语义。预热尖峰/缺样负例必须拒绝；原预算不降低。此分支不修改通用pose/geometry缓存或设计边界。


### §38 regular实际CPU持有点之外的成本

single新Release三轮完整预算、成本及43文件断源副本4200帧普通Off通过。regular原Compiler已导出，但组合4200帧在300秒超时；沿相同原二进制720帧/120预热+600采样诊断，总CPU P95/P99 81.4225/92.7773ms，顶点裁剪P95 57.1569ms、投影17.1583ms、official 5.5461ms、control0.4172ms，实际64角色/192活动层。该短窗不是资格。

最小修复先在Spine native/spine374.cpp逐三角clipTriangles/append边界合批，原gate_b.rs保存原逐三角完整几何字节golden（regular/clipping及动作/混合阶段）。最多64三角，每三角既有保守界266顶点/714索引，因此每调用最多17024顶点/45696索引，均小于uint16及原单帧上限；调用前输入索引检查、调用后保守界核对及全帧事务容量仍执行，去重仍限定附件且不排序透明段。不改官方vendor/ABI/schema、负载或预算。旧实现实际CPU失败作性能首红，owner逐字节golden及原裁剪/skin/容量测试证明语义；先owner收敛和有界诊断，不用完整Release作内循环。此分支预计10–15分钟必须形成改善信号，否则停止扩展并保留失败。


### §38 单次投影内完全等价几何复用

64角色同步配方的实际总输出约21万顶点/67万索引，而GPU逐帧上传约161568 bytes（相同内容在原资源层去重）。各实例在此前重复官方geometry提取及同内容中性编码/校验/hash，属于已测57.16+17.16ms中的重复派生工作。最小共享不改变pose权威：每实例仍独立official apply、控制、事件、混合链及挂点。只在一次render_sources内，为同一强保留Asset和ppu，读取官方求值后的完整几何输入（所有bone world x/y/a/b/c/d及active、draw order及slot index、attachment身份、slot/skeleton颜色、slot deform完整float bits）；完整字节Eq才复用不可变Arc<Geometry>。键读取有界，过容量回原提取、不拒绝原可用帧；无跨帧状态。

生产投影仅在本调用内按该Arc身份且pages完全相等复用PreparedMeshRenderer/资源handle；每实体TRS、visibility、provenance、instance id仍各自生成。原manager和冻结packet退休所有权不改，不新增tool/schema/公开项目ABI/世界权威，不改官方vendor。私有native读取函数及Rust GeometryInputKey、control/session、production/spine及直接tests归B串行。最便宜验证是跨实例相同key/几何、不同asset/ppu/clip/skin/attachment/deform/颜色/顺序键隔离、过容量fallback；原完整scalar golden、session World替换/独立pose/事件与真实GPU消费者验证。真正CPU改善仍需同源原Compiler产物短窗，然后才跑原完整资格；不把Debug计时当正式通过。

### §38 official属性查找残余

 Gate F官方属性查找成本的同版本局部修复

新Release原regular短窗保留64实例/192层，vertices/clipping P95 57.1569→1.4229ms、projection 17.1583→0.3604ms，但Spine总CPU P95/P99=7.4427/20.912ms仍失败。残余official慢帧固定在切换/回收时刻；原native owner相同64实例复现。C++ Release没有NDEBUG，临时仅加NDEBUG将owner median/P95/P99 3.2796/7.2726/20.1961→1.9834/5.0336/17.0384ms，不能单独闭合。源码定位官方AnimationState::animationsChanged/computeHold与Animation::hasTimeline消费HashMap<PropertyId,bool>，该名为HashMap的实现实际上逐节点线性find；PropertyId数量增长使切换工作呈平方级。

这是原固定版本内部容器查找整改，不改变官方AnimationState/timeline/混合与事件权威，不新增pose缓存或替代算法。允许仅对原HashMap.h的官方signed PropertyId键附加std::unordered_map<PropertyId,Entry*>索引；完整键Eq、已有链表插入/遍历顺序、值替换、remove/clear、对象生命周期保持。AnimationPair混合配置键保留原查找路径。原vendor文件清单保留为上游基线，UPSTREAM.json追加精确本地补丁身份；此前“全部vendor当前未修改”对这一个头文件由本节明确替代，其余原blob不改。build.rs只在Release定义NDEBUG，Debug断言保持；原Rust导入/结构/索引/帧容量验证不减少，原性能预算不变。

实施前保存原header/build/metadata，保留无补丁64实例Release owner红信号；先同输入owner反事实验证实际因果，再原23项geometry/12全字节golden与直接session/project消费者、scoped Clippy/fmt。单独C++ owner验证PropertyId插入/替换/相邻及头尾删除/清空再用、完整Eq碰撞和原Entries顺序，AnimationPair保持原语义。所有临时计时/临时测试用后逐SHA恢复。若小owner无显著改善，撤回补丁，不扩大自研容器。源收敛后只作一次原Compiler新产物短窗，合格才进原三配方三轮、成本及断源完整交付。原硬件资格独立，不能删除最终guard冒领完成。


## 24. Gate F原生Release跨文件优化

局部反事实沿同一64实例/三层原native owner：属性索引+NDEBUG median/P95/P99 1.9343/3.4738/4.8163ms；仅MSVC /GL直接链接 1.7085/3.3861/3.9067ms，/GL /Ob3为1.6670/3.0679/4.7847ms。该小样本仅证明构建优化有可测收益，不授正式预算。允许backend build.rs在Release且实际MSVC（非clang-cl）启用/GL /Ob3，并通过cc现成link_lib_modifier("-bundle")保留原生静态库供MSVC最终链接；Debug及其它工具链保持原路径。首个默认Rust bundle对GL对象的链接失败保留，未产生Player，不作为运行失败。

不改官方算法、浮点规则、完整Eq、负载、阈值或全局配置；不引入跨帧缓存。先原27项backend/golden及同64实例owner，通过后原Compiler新Release短窗；新产物资格与此前版本分开。若正常Cargo/Compiler链接不能闭合或无实际收益，撤回本构建优化，不扩大链接基础设施。几何缓冲优化限单次提取内输出reserve、xy/remap复用及标准pmr节点池，返回/出错时全释放；原12个全字节golden保持。


§38预算内验证排序：native-08原regular4200帧/600预热/3600采样诊断P95/P99=4.9186/6.411ms通过，但720短窗P95=5.0912ms略超5；两者原始结果均保留，不择优删除。剩余五小时窗口优先验证regular，再clipping；仅run-owned测试循环按配方过滤，原CONTRACT、4200帧、三轮、全部预算/空场景/成本/独立副本及final pending guard保持。临时harness先编译保存SHA，main gate_f.rs在Compiler输入前按原SHA恢复；产品源码与native-08消费的1455项保持一致，临时测试身份独立，不为过滤重新编译生产二进制。每配方终态及缺项分开，不能冒充三配方完成。14:47 UTC最迟终止自有验收树并保留证据，14:52:58前收尾，不扩大授权时间。

### §38 本轮终态

Gate A–E已关闭；F本轮完成真实持有owner回收、Off全请求帧峰值补齐、有界裁剪合批、单次等价几何/准备复用、官方PropertyId查找及Release原生构建优化。原预算与最终guard保持；regular首轮输入齐全，唯一失败Spine CPU P95 5.4442ms>5ms，下一步先按实际交付定位CPU余量，再补矩阵；原硬件资格独立待办，F/374仍partial。本轮五小时窗口收尾，无B自有构建或Player后台；371/375及游戏V0.0.5保留。 详见[五小时记录](../../阶段完成记录/2026-10-26-374-GateF-Five-Hour/00-五小时续工与性能收敛.md)及`.artifacts/374/gate-f-completion-20261026-095258/closing-requirements-audit.json`。

## 39. 2026-10-26 两小时续授权

用户明确追加两小时继续374整体目标。窗口UTC15:18:38–17:18:38（北京时间10月26日23:18:38至27日01:18:38），最后15分钟收尾，continuation不重置。此前五小时结果及首错完整保留，A/C及375不重复施工。唯一产品源main，374专属target；不回写旧worktree，不写索引、不升级冻结游戏、不改安装。

必要性与验证经济性：真实release-09 regular首轮eligible但Spine P95 5.4442ms>5ms；其最慢5%帧official占66.69%，慢帧聚集切换/退休。先在既有native owner按同64实例/三层整数时钟，临时分解prepare/update/setup/apply/world/finite及必要official子阶段。每分支10–15分钟，不能改善因果信号就停止；无产品变更不重建Compiler。最小因果修改只限已证实owner，完整官方时间/混合/事件/数值验证保持。临时插桩和测试用后恢复，源/测试/产物身份分列。

有效修复后先原backend字节golden与直接控制/项目消费者，再fmt/scoped Clippy，随后一次必要原Compiler正式regular；通过才继续其它配方三轮、测量成本与完整独立交付。复用未失效owner证据，不重复371/Web/冻结版测试。原硬件资格独立；不得用4090代替3070，或移除pending guard冒领374完成。完成前逐原出口审计；未完成则保持partial并明确剩余。

### §39 属性重算节点分配的局部反事实

同一native owner分解证明切换时animationsChanged每64实例clear约0.44ms、computeHold约0.95–1.45ms，setup/world/finite小于0.15ms；应用总成本仍由官方apply产生。原PropertyId索引同时分配官方Entry和标准索引节点，clear各释放一次。先仅同一HashMap.h反事实：PropertyId的标准unordered_map直接拥有稳定地址Entry，仍接原双向列表保留逆插入迭代顺序；标准pmr小节点池只复用原始存储，remove/clear照常析构全部逻辑节点，析构map释放全部pool，非PropertyId保留原路径。没有跨实例/跨帧pose复用或官方算法变更。

这是§23内部容器存储方式收敛，替代“链表独立new/delete节点所有权”，不替代链表顺序、完整Eq及逻辑生命周期。先原64实例sample与clear/reinsert/order/fullEq所有者反事实；无实际收益即撤回。性能定位通过后才更新vendor元数据并跑原27项/golden和直接控制/项目消费；之后正式预算重新验证内存，不能以池复用证明满足峰值或374完成。

### §39 主槽S5优先交接

UTC15:33收到主槽371 S5冲突优先通知，按54等待A明确释放SDK/Provider/Compiler及正式共享构建/性能采样。本路无这些owner或公共core/帧循环新增修改；所有B构建/Player进程已自然结束，无GPU任务。临时native/official插桩、gate_b sample测试均按SHA恢复，候选HashMap.h/UPSTREAM/README冻结，见本轮primary-handoff.json。已编译的同源Release backend/golden 23项通过；容器测试覆盖完整有符号键、替换、链表顺序、删除/重用、非PropertyId路径、逻辑析构及102次有界clear/refill，析构后标准资源outstanding=0。32节点chunk首红保留：本机标准池仅保留一个空chunk，已调整4096上限（不是预分配4096）后100次同上限循环无新增上游分配。短窗sample median/P95/P99=1.6993/3.0109/3.8156ms仅局部诊断，不能授正式预算。未证收益不扩展到其它owner；原regular首错和剩余三配方、成本/交付/原硬件义务保持。等待不延长两小时窗口。

## 40. 2026-10-27 十小时续授权与完整出口

用户明确授权继续374直到完成，并追加十小时。首个计时UTC2026-10-26 16:12:53，北京时间27日00:12:53；UTC27日01:57:53停止新增实施/验证、02:12:53硬截止（北京时间09:57:53/10:12:53）。用户本次时限覆盖技能默认三小时，自动续轮不重置；此前两小时窗口由本轮替代。目标是完整374出口，不把一个优化、一个配方或一个窗口视作整体完成。

主槽371 S5优先及54显式释放规则保持。已有候选六项身份与primary-handoff逐SHA核对一致；无B构建/Player后台。仅B登记和本施工文档更新，A/C行完整保留。SDK/Provider/Compiler、公共接入及共享构建/性能测量未释放前不启动；可继续不竞争的源码与证据审计。仅看到进程空闲不视为释放。唯一产品源main，独立374 target，旧worktree不覆盖主线；371/375、冻结游戏V0.0.5与Git索引保持。

必要性与验证经济性：旧真实regular预算eligible但Spine CPU P95 5.4442ms>5ms；当前节点池候选只获局部owner和23项backend/golden证据，没有当前正式预算。先复用未变身份的纯测量/GPU owner证据，补候选影响的backend A/C、控制/生命周期和项目直接消费者；源稳定后才从原Compiler生成一次当前WindowsRelease并正式regular，过后继续clipping/single各三轮、Off/Summary/Trace成本、最终无测量及断源完整副本。预算、负载、全部官方求值和final pending guard不降。编译失败与运行失败分别保留，未变输入不盲目重建。新诊断分支10–15分钟无更窄因果信号就止损，失败回owner，正式矩阵采用fail-fast。

原i5-12490F/RTX3070/指定驱动资格独立；本机i9/4090证据不能替代。完整退出逐§1/§11及合同核对，不以登记代替测试、不删guard假绿。若硬件能力不可达，记录真实依赖并继续可完成的本地出口；未证全部出口前F与374仍partial，不归档总施工。证据根`.artifacts/374/gate-f-ten-hour-20261026-161253`。

## 41. 主槽释放窗的当前候选消费者验证

A在54显式释放UTC2026-10-26 18:12–18:32，期间不改SDK/Compiler/公共runtime、不运行构建或GPU。B冻结实际main当前876项SDK输入15,613,192 bytes，digest `sha256:a7fa13cec392bb8e9de9982df9eec2a7e5a96cdc67f19ac6e0af22044448c97d`；包含A最新窗口/host修复，六项B候选身份与原交接一致。

独立374 target、原test/debug profile与原生Release：A/C共21项、Core Spine39项（2项原显式ignored）、原Compiler项目消费者24项（6项原显式ignored）通过。三族按各自合同记录，不把重叠证据相加。Core实际composition包含spine/real-wgpu；项目driver由当前Compiler重编，二进制SHA `8f40ad3378da85da8f2a3c7ba71acacdebd5d9a8b474309bade047c3a2f24348`。run-owned筛选仅用于后续正式配方次序，原gate_f.rs已逐SHA恢复；源码恢复后SDK指纹无漂移。证据在本轮根`native-a-c.log`、`core-spine.log`、`project-consumers.log`、`released-validation-terminal.json`及`formal-filter-restored.json`。原23项backend/golden按不变输入保留，原性能失败未被定向测试替代。

原regular旧首轮至失败耗时17分16秒；新候选若通过三轮还需独立Off/empty/Summary、无测量断源交付等，18:32前剩余时段不足以预留完整采样和证据。B未把整套可分阶段测试当作不可中断操作强占后续时段；在54登记至少35分钟资源请求，等待A明确扩窗/下一释放段。F与374仍partial，授权总窗口不重置。

## 42. 19:30释放窗：完整regular第三轮失败与单次调用首项比较

A显式释放UTC2026-10-26 19:30–20:15。B保留A startup Sprite/AUI/font资源准备重试及Android exporter/private-target/运行库修复；另发现artifact facade/incremental实际变更，重编当前driver后恢复原formal源码，未使用旧DLL移名。当前生产Game SHA `66957b061895c27527bc42e19e548a05ec65f853f800239e3d57cd7b466f9eaa`，Compiler SDK摘要`sha256:e51eda343b4b3ae0cb6da10744ce1a678dbb69d995b42972aceeab94d204ed17`。该生产包属于本段修复前容器候选，后述首项比较改动没有冒充进此包。

原regular完整三次独立预算均eligible；Spine CPU P95分别4.7161、4.6696、5.0133ms，第三轮超过原5ms，P99分别5.6954、5.7763、6.0508ms。第三轮唯一已实现预算失败是该P95；原负载、600预热/3600样本、60秒窗口、独立Off/同二进制empty和预算均保持。Off CPU P95/P99分别6.0695/6.971、6.0517/6.9225、6.5705/7.5191ms，增量private峰24,461,312/25,309,184/23,871,488 bytes。Trace tail1200斜率-96.514/20.969/5.967 bytes/frame。首轮Summary成本已写入，第三轮预算fail-fast使最终断源副本及clipping/single未执行；不挑选前两轮宣称通过，不把全CPU段普遍变慢的原因直接归于环境。原失败及新失败分别保留。

必要性与验证经济性：分段观测仍见几何阶段约1.2–1.4ms。10秒独立Release后端owner诊断使用实际regular的64份相等26,780-byte几何输入：原HashMap查找中位264,800ns/P95 271,900ns，完整Eq比较40,600ns/P95 46,100ns。只在现有本次render_sources调用中把首项独立保留，完整Eq命中直接共享Arc，其它键继续原HashMap；不跨帧、不改官方求值、时钟、内容或预算，不省略任何实例。因果修改仅session.rs，测试补相同/不同姿态交错及后备表复用、完整几何等价和跨调用不保留。

Core有40项有效不同用例（原广组40通过后，仅增强同名新用例的交错次序并定向重验，不能把40+1相加）；当前项目直接消费者24项、两源fmt与scoped Core Clippy通过，既有158条依赖/原owner告警保留，改动位置无新增告警。最初新测试夹具5处类型错误保留在first-key-core.log，修正后复验。原生A/C21、候选后端23/golden和容器析构证据按未变消费身份复用。新的首项比较候选没有正式Release预算，不继承旧包前两轮通过。

辅助校验器首次按Python WindowsPath大小写折叠排序，与Rust PathBuf逐组件、保留大小写的顺序不同；直接字符串排序假设也已否证。原记录不重写，876个文件SHA在发布至采样结束均无漂移；修正为组件顺序后与实际Compiler e51eda34…一致，详sdk-compiler-digest-crosscheck-components.json。§41旧a7fa…及本段原d6b…只属辅助排序错误值，不作为Compiler产物摘要；原文件级身份有效。该修复仅在run-owned辅助脚本，不使既有生产测量失效。

本段终态/源码归属、原规格剩余任务与下一资源请求见handoff-2015.json。当前输入完整快照first-key-qualified-inputs.zip；Core新候选及全部既有371/375成果保留，无B后台后明确交还A。继续点为下一显式释放中验证新候选原regular，再clipping/single各三轮、成本与最终独立副本；建议连续65分钟来自实际阶段成本，非保证时长。原硬件资格仍独立，F/374保持partial，总十小时截止不重置。


## 43. 21:20释放窗：当前新Release组成超时与提前交还

A明确释放21:20–22:25 UTC连续65分钟，B于21:22登记接收并核对原8项owned源码一致。当前Compiler driver刷新耗时81秒，原formal测试源逐SHA恢复；额外发现A新增BM05 simulation.rs纳入实际Compiler输入，106项身份记录完整。SDK876项当前摘要a29993c9…，A deformation.rs成功源验证memo及其它成果全部保留；本段没有新增产品源码改动。

21:24:17开始首项Eq新候选原regular。实际WindowsRelease成功构建，新Game SHA 1e1ce83a35946500505c983dcabcb14eb54c0541375ee679c69254f80e3f6444，runtime DLL ce3ea94b…；随后原Compiler组成verify在300081ms以process_timeout失败，child报告缺失，尚未执行任何正式repeat预算，clipping/single未启动。首错保存在release-11-regular.log与发布包reports/exported-player-process-verification-report.json；超时总耗时不全计Player或编译成本。

最小同binary诊断：新包120帧/30秒仍无报告，超时前CPU仅1.34375秒；先前原regular已实际测过的旧Game在当前窗口条件下120帧/30秒亦超时。该反事实说明症状也能发生在旧产物上，但尚未证明具体启动、窗口、GPU或环境根因；不得归责新首项Eq或宣称环境已证。未改预算/采样/超时，未再盲目Cook/全量重建。

21:42:24 UTC按主槽优先请求，在已启动旧包诊断原子步骤结束后提前交还共享源码、Cargo/Compiler/Player/GPU/测量资源。新旧诊断树已终态，实查无B所属后台；无关GameSDK不触碰。876项SDK、106项Compiler输入和8项B归属源码无漂移，准确driver、Game/DLL/模块及交还时刻见`.artifacts/374/gate-f-ten-hour-20261026-161253/handoff-2142.json`。本段释放提前结束，等待A下一显式释放；下一步先定位原组成停滞最小owner，不能用又一次完整重建替代诊断。原regular第三轮5.0133ms失败、三配方三轮、成本/最终交付、最终默认兼容check与原12490F/3070资格仍按各自有效身份保留或待验，F/374 partial，不归档。授权截止不重置。

交还后非冲突只读观察：21:45:18 UTC，Windows build26200当前Session1的WTS扩展会话信息返回SessionState0/SessionFlags0，当前锁屏；LogonUI进程亦在Session1。证据release-11-desktop-session.json只记录当前状态，不证明此前300秒超时发生点或具体原因。已向用户请求人工解锁并保持桌面登录，不尝试解锁或修改显示/登录配置。后续须等人工恢复且A重新显式释放，先相同已产binary120帧短复验；不能盲目重新构建或将本观察标成产品修复/预算通过。

## 44. 用户回复与资源等待恢复

2026-10-27用户明确：原验收机已不可用；仅通过54等待交接；桌面已解锁并保持登录。00:48:58 UTC只读WTS确认当前Session1 flags1、locked=false。锁屏外部条件已解除，但此前超时因果仍未证明。原i5-12490F/RTX3070/指定驱动资格保留为不可达外部依赖，不能以本机4090或文档登记代替。

恢复核对8项B源与handoff-2142.json无漂移；A后续共享Compiler/SDK修改按实际消费身份重新核对，不覆盖。当前54仍明确A持有全部冲突资源，B仅登记下一释放请求并进行不竞争的只读审计，不发跨聊天消息。下一释放后先同一release-11已有binary120帧/30秒短复验；若仍停滞，保留首因并在最小owner定界，不盲重建。若成功且剩余资源窗足够，再刷新实际主线输入和driver，原regular三轮/成本/独立副本保持。01:57:53停止新增、02:12:53硬截止不重置；本次回复不是新的计时起点。F/374仍partial，A–E与371/375成果、游戏V0.0.5均保留。证据resume-user-answers-20261027.json。

## 45. 解锁后当前Release正式三轮与CPU突变

A在2026-10-27 01:15 UTC通过54与294-resource-release.json明确交还全部共享资源。B于01:17:05确认桌面flags1/未锁；同release-11已有binary原120帧仅3.125秒/exit0，数据及二进制未变。本观察与此前锁屏时的超时并列保存，不追认此前超时根因。A最新terrain_cook与engine_navigation_native Rust/C++共36项保护SHA、8项B源保持。

当前Compiler driver实际刷新166秒，临时配方选择harness编译后原gate_f.rs逐SHA恢复。SDK879项摘要`sha256:1f7266ad3c4db2f5840fc2711451063ebf50c012f113d4d2e4241e5126eb2b8d`；Compiler Rust源/测试102项含A新增simulation_navigation及BM13 helper。原6项非Spine三维JSON夹具未由本Rust输入清单重复枚举，前窗至本窗逐SHA保持，不属于本次Spine配方。driver SHA f789300dccbc6d59bbe3f8281a858bb2cbd52846a38909061843e484b7e52fd5。原regular于01:20:36开始，prepare 6386ms、Build 469252ms、Compiler组成verify调用74494ms（其中子进程73252ms）；新Release原4200帧组成通过，未复现前窗组成超时。

三轮原4200帧/600预热/3600采样、同binary Off和empty完整执行，结果：

| 轮次 | Spine CPU P95/P99 ms | Off整帧CPU P95/P99 ms | 原完整预算 |
|---|---|---|---|
| 1 | 4.7682 / 5.6838 | 6.0222 / 6.9260 | passed |
| 2 | 4.7710 / 5.6907 | 6.4020 / 7.6445 | passed |
| 3 | 10.8958 / 15.2863 | 14.6808 / 20.0515 | failed：四项CPU预算超限 |

三轮eligible、GPU/上传/backing/Off内存及增长均无其它失败。逐帧报告的实例/层/几何计数与固定tick摘要三轮相同；该比较仅证明报告计数与tick，不声称完整顶点逐字节一致。第三轮约采样1200帧后控制、官方apply/world、几何和投影阶段同时变慢，不能仅凭一个局部键查找假定首因。原5.0133及5.4442ms失败不删除。第一轮Off/Summary/Trace成本分别留原数据，Summary与Trace的整帧P95相对Off差值为-0.2020/+0.1061ms；负差值是独立进程实测差，不作减扣或必然优化声明。第三轮预算失败后原流程fail-fast；未执行clipping/single、最终无测量断源独立副本，final pending guard尚未到达，不把退出101当预期guard通过。

有界诊断仅复用当前产物，无新构建或产品修复。1800帧32.016秒正常，后三个600帧段Spine P95约4.6ms；为覆盖此前绝对帧1800后的突变点，再作一次原4200帧/600+3600的只读进程观测，73.031秒/exit0，Spine P95/P99 4.8047/5.6793ms。72/73观测点Player在前台，Normal优先级与全32逻辑CPU亲和范围未变，系统负载观察约3.41%–17.20%；这些只描述该诊断，不能回推正式第三轮失败原因，不能替换失败轮次或证明正式预算通过。未改亲和、优先级、电源、时钟或登录配置。暂未定位首因，本窗停止扩大诊断，不盲重建；下次在相同binary突变发生时捕获CPU执行/调度条件并对应原四段时序，再决定owner修复。

最终默认非Spine `cargo check -p engine_runtime --lib --locked --offline -j1`于01:47:14–01:47:34通过（19.891秒），既有4条Core warning保留。8项B源、36项A保护源、879 SDK及102项Compiler Rust源无漂移，原formal源恢复；本释放段没有新增产品源、配置、安装、游戏或Git索引修改。运行辅助脚本发现empty-scene Fixture与主配方标签相同，使原glob误取多目录而遗漏预算路径；现按实际Compiler根build.json定位，三份预算路径已补入终态，原第三轮失败和全部阈值不变，未因此重跑。

本段已结束所有自有操作并释放资源，仅文档与证据收尾；原授权01:57:53停止新增、02:12:53硬截止不重置。A–E保留关闭，F/374 partial，总施工不归档。原验收机已由用户确认不可用，12490F/3070资格独立pending，不以4090代替。完整输入/产物、三预算、两次诊断、默认check与剩余任务见`.artifacts/374/gate-f-ten-hour-20261026-161253/release-12-closeout.json`及各原始文件。继续顺序：先定位本次CPU突变 → 原regular完整三轮 → clipping/single各三轮与成本 → 当前候选最终无测量断源副本 → 原合同硬件及全退出审计。

## 46. 2026-10-27 三小时续授权与未变输入复用

用户授权三小时继续374至完成；02:31:33开工，05:16:33 UTC停止新增实施/验证，05:31:33 UTC硬截止（北京时间10:31:33、13:16:33、13:31:33）。已接收54上轮明确释放并核对无冲突进程，8项B/36项A/879 SDK/102 Compiler输入及driver未变。共享源码/构建/Compiler/Player/GPU/测量现由B使用，交接仅经54。先现有Release有界捕获线程CPU时间/周期及前台条件，原regular第三轮CPU失败保留；独立配方可收集各自剩余证据，失败不授整体完成。不盲重建、不改原预算/负载/轮次，原硬件不可用资格pending，A–E关闭、F/374仍partial。371/375及V0.0.5保持；证据.artifacts/374/gate-f-three-hour-20261027-023133。

必要性与验证经济性：复用§45的新Release与既有owner/consumer/default证据。只为原第三轮多阶段CPU突变增加短时只读观测，最多15分钟；不凭猜测改缓存或调度。原失败未排除前不盲重建；不同配方各自独立，完成其原三轮/成本/断源副本能减少独立未证项，但不能抵销regular失败。最终逐完整退出清单审计；原验收硬件pending独立保留，不删最终guard，不归档总施工。

### 46.1 原Release独立配方终态与局部候选

本轮regular首轮Spine CPU P95/P99=5.4199/8.1402ms，原5/8预算失败；clipping前两轮通过，第三轮Spine CPU=10.4271/14.3925ms及Off整帧=18.0419/22.7365ms失败，均保留。single三轮、Off/Summary/Trace成本、empty、最终断源无测量4200帧独立副本全部完成；最终硬件/整体pending guard仍在。

clipping第三轮末段前台丢失是相关线索；9000帧同进程前台/后台/前台诊断完成，但逐帧采样无精确UTC绑定，尚不足以定根因或豁免失败，不改变电源/亲和/优先级。详见本轮focus-block-analysis.json、clipping-foreground-loss.json。

继续实施compare-necessity.md中的四文件最小候选：复用原native编码器完整比较首键字节，仅消除已测重复键物化，失败继续原路径，不跨帧缓存，不共享官方求值，不改负载/预算。先同Release交替微基准，regular成本收益不足10%即止损；通过后刷新owner及A后续22项改动影响的当前项目消费者，之后执行原正式独立配方。既有owner证据仅按未变输入复用；新候选不得继承旧single正式资格。

### 46.2 完整输入比较候选的源码与组合验证

同Release交替120次局部对照regular64实例原键物化+Eq median126400ns、新完整原字节比较96800ns，约23.4%局部成本下降；clipping16实例109600→73500ns，single1实例700→500ns。局部微基准不授整帧收益。后端5项输入边界/完整字节/资产/单位/姿态测试及23项几何golden通过。当前Core相关40项通过，2项显式真实GPU/诊断ignored不冒充执行；scoped后端Clippy无告警，Core保持既有158条且候选改动位置无新增告警，三Rust源fmt通过。

当前主线组合首红为Spine FocusChanged测试调用少传A新增physics参数；仅在本路session.rs测试补None，原invalid_phase和World不变断言保持，修后Core40项通过，首错compare-core.log保留。当前Compiler驱动刷新48.719秒、原gate_f.rs逐SHA恢复；当前项目24项非ignored消费者180.719秒全部通过、6项显式ignored不累加；默认非Spinecheck5.640秒通过。879项实际SDK digest为sha256:b1a429db62d9707f5f044221ddeae5dada7dfe66cf54ef2c229c1143a5994be4，driver SHA6bb2d344b79ee3b0a76a4a8f5191e8b91350c4b0340a7343e426203b37ebc3fb；详细身份compare-start.json/compare-owner-identity.json。

03:44:17 UTC开始本候选原regular正式流程，随后按独立配方继续clipping/single；原负载、预算、三轮、Off/empty/成本、最终断源无测量交付及硬件pending guard保持，候选不继承旧single资格。原失败和前台线索仍保留，暂无CPU突变根因结论。

### 46.3 本候选regular/clipping完整通过与single条件分列

regular与clipping均完成原三轮全部预算、Off/Summary/Trace成本、empty及最终43文件/4200帧无测量断源独立副本。regular三轮Spine CPU P95/P99=4.6953/5.6202、4.4328/5.4006、4.4883/5.5987ms；clipping=7.2701/7.9634、7.4028/8.1389、7.2921/7.9909ms。SDK/Compiler无漂移，最终pending guard保留。

single新候选首轮Spine P95/P99=0.6881/0.9864ms，P95超原0.5，预算失败，后续按原流程停止；Trace进程68次观测仅2次前台，Off/empty均前台。A/B/C同产物短诊断未稳定重现失败，前台变化不足以证明根因，不撤销首错、不宣称产品修复。另收集一次明确保持Player前台的独立原single完整三轮/成本/empty/断源副本：不改源码/预算/负载/轮次/优先级/亲和/电源，不追认原失败或后台资格；只形成当前机器前台条件的独立证据，F与原硬件资格继续pending。预计17分钟，最多一次，若再失败不重复；05:16:33停止新增和05:31:33硬截止保持。

### 46.4 本轮实际终态与交接

regular/clipping各原三轮全部预算、成本/empty及最终43载荷4200帧无测量断源副本通过；single原首轮P95=0.6881ms>0.5失败仍未解释，另一次受控前台三轮/完整步骤通过，不抵销首错。原12490F/3070验收机不可用，原硬件资格pending；Gate A–E关闭，F/374仍partial。 focused-single三轮P95/P99=0.2274/0.2791、0.2487/0.3313、0.2170/0.2555ms，原三轮全部预算/成本/empty/最终断源副本通过，但不能抵销原single失败。后台B短诊断亦正常，焦点单因素解释未证实。

close-audit.json确认10项B/36项A/879 SDK/102 Compiler和driver无漂移，harness恢复，Git索引/锁保持，三个最终副本各43载荷逐SHA一致，无所属后台；前台控制已停止。2026-10-27T05:02:12.682648+00:00明确释放共享资源。原05:16:33停止新增、05:31:33硬截止不重置，未进行安装、游戏升级或Git索引写入。源/测试/产物/成本、长路径审计修复及下一步见[本轮记录](../../阶段完成记录/2026-10-27-374-GateF-Three-Hour/00-三小时续工与条件验证.md)。F未关闭，总施工不归档。


## 47. 2026-10-27 用户批准验收硬件调整（仅文档修订）

用户明确“允许改用当前工作机”；后续正式验收目标为i9-14900K/RTX4090、驱动32.0.16.1088，Windows11 build26200，按374-B§26/HW-20261027生效。原i5-12490F/RTX3070目标被替代并保留历史身份，不授3070资格，不再等待原机；全部原配方、预算、三轮及single首错保持，final pending guard不删除，F/374继续partial。§46及更早的原硬件pending为各窗口当时事实。

本次不延长或重启已于北京时间13:31:33结束的施工窗口，不构建、不启动Player、不占用共享测量资源。下一技术步仍是single首错的最小执行/调度诊断，再收受影响出口；继续施工须按54登记资源及新的施工时间授权。见[修订记录](../../阶段完成记录/2026-10-27-374-Hardware-Baseline/00-验收硬件基准调整.md)。


## 48. 2026-10-27 新授权：374完整退出续工

用户授权继续374全部施工；本续工05:55:58 UTC开始；用户06:05:56 UTC新增授权4小时并明确中小问题自主处理，最终10:05:56 UTC（北京时间18:05:56）硬截止，09:50:56 UTC停止新增。当前i9-14900K/RTX4090验收基准已按HW-20261027批准；全部原负载/预算/三轮及single首错保留。已核对无冲突进程、10项B/36项A/102项Compiler及driver无漂移。B占用本轮所需共享构建/Compiler/Player/GPU/测量，先原产物一次有界CPU执行/调度诊断，捕获信号后回最小owner；不盲重建或重复直到绿。regular/clipping和额外前台single按实际不变身份复用，A–E关闭，F/374仍partial。371/375、游戏V0.0.5、Git索引/锁保持，交接只经54。

必要性/验证经济性：请求是关闭原完整374出口，当前唯一已知性能残余为single首轮0.6881ms>0.5。先复用未变Game/engine/project DLL和原负载，以现有直接Player入口一次2400帧诊断采集CPU执行/上下文切换信号，整个探索分支最多5分钟；录制权限或消费工具不可达、无法绑定窗口或未复现时保留未定并停止该分支，不扩大私有ETL解码器、不循环重跑正式序列。只有证据指向owner才写最小红例/修复；刷新修复影响的owner、直接消费者及失效资格，未变证据复用。全部退出按§1/11/12及374-B§8/26审计；不改阈值/负载、不删guard假绿，不授3070资格、不安装或升级冻结游戏。


### 48.1 管理员采集终态与主槽交接

本次两次UAC均获用户明确批准：第一次管理员-File辅助未进入脚本，默认脚本执行限制阻止启动；第二次以相同内容内联启动成功，未改执行策略。WPR CPU.Light startExit/stopExit均0，06:06:36–06:08:24 UTC录制已停止；Game PID53996原2400帧/1200采样exit0，P95/P99=0.2128/0.2539ms，超0.5ms样本0。未复现首错，不转化为正式资格，不继续扩展ETL解码或循环录制。

B于2026-10-27T06:10:01.880305+00:00完成原子采集并释放共享源码/Compiler/Cargo/Player/GPU/测量给主槽A/371，等待54明确交回；无B所属后台/录制，交接仅经54。用户四小时授权仍于09:50:56 UTC停止新增、10:05:56 UTC硬截止，不重置。single原首轮0.6881ms超0.5ms首错未解释；HW-20261027、原预算/负载/三轮及F partial保持。

只读旧报告比较确认原288个超预算帧中，AUI中位数0.14405ms、renderSubmit1.0509ms；同轮其余帧分别0.0494/0.5344ms，多个独立墙钟阶段共同变慢。此为共现证据，不能单独归因调度、频率或Spine实现。原报告SHA保留于retained-cpu-comparison.json，未新增产品源码、构建或正式轮次。采集/交接证据admin-terminal.json、admin-player-terminal.json、admin-player-analysis.json、main-priority-handoff.json；本探索分支结束。


## 49. 2026-10-29 用户授权的短时施工收口

374于2026-10-29 13:45:15 UTC（北京时间21:45:15）按用户新授权继续，明确不做长测/过度施工。A/C已收尾释放且无冲突构建/Player；B恢复本路接入/构建/短时诊断，交接只经54。范围：复用既有功能和三轮证据，先核对正式test无条件pending的实际缺口，仅修已确认harness问题；single首轮0.6881ms>0.5ms失败保留，未知原因不猜补丁。单次新Player至多60秒、累计至多3分钟，不运行完整三配方长序列；必要编译分开计时，不清共享缓存、不改安装/用户配置或371源码。原预算/负载/三轮不缩短、不以短测授完整资格。16:30:15 UTC停止新增、16:45:15 UTC硬截止（北京时间10月30日00:30:15/00:45:15）；A–E关闭，F/374仍partial。证据.artifacts/374/short-closeout-20261029-134515。

必要性/验证经济性：无产品缺陷红例不改产品；首错原因未定，不继续ETL/录制或重跑直到绿。先检查现有release_recipes末尾的无条件panic是否已与实际预算/成本/交付实现脱节；如有独立逻辑缺陷，修成仍严格拒绝缺项的终态判断，用最小确定性负例验证。未变owner/像素/三轮按真实身份复用；A的后续改变仅刷新实际受影响消费者，不跑workspace/完整矩阵。全部formal要求若仍未证，保持F partial，不以本授权降低验收。


### 49.1 用户明确的验收范围调整与实际接入首错

用户2026-10-29明确同意“允许，功能施工收口，性能资格单列待验”。本轮正式出口调整为374 Windows功能施工完成；原完整性能资格单列待验，原single首轮0.6881ms>0.5ms及所有预算/三轮/HW-20261027/最终guard保留，不授整体性能合格。本轮发现并只修Spine导入器对新RuntimePackageTexturePixels的缺失转换（spine_import.rs: pixels.into()），不改像素、Cook摘要、格式或371存储owner。先当前现有项目消费者短测，再核销功能需求及归档施工；不跑正式性能长序列、不新建通用runner。

必要性/验证：当前启用spine的Compiler编译报E0308，原Vec赋值不符合新owner类型；最小修复仅转换为已有Owned(Vec)包装，From实现不改字节。保留hardware-owner.log首错；复用同一Cargo缓存，只刷新受影响的既有作者Cook、会话/投影消费者及fmt/scoped lint，测试总运行以分钟内为目标。为完整性能入口新增的探索硬件/收口代码已逐SHA撤回，gate_f.rs恢复0ac2ea8c28886892499aa1c4f7cadc69fcb4a11ea23d493ac09be6f33e5abaa3；不让资格整改扩大功能收口。


## 50. 2026-10-29 功能施工完成与独立性能待验

374 Windows功能施工完成；用户已明确确认完整性能资格单独待验。A–E及F功能出口完成，原single首轮0.6881ms>0.5ms失败、原预算/负载/三轮与HW-20261027保留，完整性能未获资格。本轮仅修Spine导入纹理类型接入，9项定向测试通过、实际58.31秒，scoped Clippy及fmt通过；未运行长测或更新安装。总施工按功能出口归档，B资源释放、无所属后台。

用户明确同意“允许，功能施工收口，性能资格单列待验”，修订原§11/13把功能和完整性能绑定的退出/归档要求；其余功能范围不缩减。需求逐项核销、唯一产品修复、短测/编译成本及复用边界见[功能完成记录](../../阶段完成记录/2026-10-29-374-Spine-Animator-v1/00-功能施工完成与性能待验.md)；原完整性能义务转入[独立待验清单](../../阶段完成记录/2026-10-29-374-Spine-Animator-v1/01-完整性能资格待验清单.md)及374-B§27。F性能guard保持，后续不得把本归档解释为完整性能合格或安装更新。
