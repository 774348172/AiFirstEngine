# 365：通用二维绘制与AUI表现接口方案B——总施工文档

日期：2026-10-08。状态：A–E完成，`source_release_qualified_short_window`，总施工归档。用户明确缩短长回放后，三关各双2168帧及20轮/2000帧窗口验收通过；展示15组矩阵和900项owner证据复用。历史长窗超时未定位、不宣称修复，安装仍364。A/项目Clippy保留各自既有精确豁免，Runtime基线lint边界见执行记录。

正式依据：[365方案B](../../365-通用二维绘制与AUI表现接口-方案B-v1方案.md)，含14.1节施工前补充合同。顶层authority仍为产品/架构v1.1，状态只见[49](../../49-当前状态与下一步入口.md)与[54](../../54-当前可自动化施工总入口.md)。

## 1. 目标、边界与基线

交付公开Draw2D/PathBuilder、统一AUI表现属性/坐标/裁剪/命中和组合补间，通过两普通项目、真实GPU/窗口和独立Release交付验收。不将项目兵种、弹道、美术编排放入引擎。

363 A–E与364安装已闭合，当前默认Provider保持20261007-364。工作区已有大量未提交/未跟踪实现；以文件当前内容为施工基线，保留全部既有工作，不reset、不把脏工作区冒充clean commit。施工不修改真实默认配置或安装目录。

当前/待执行目录没有其它有效施工文档；旧254的txt/json以及`254-R2-R2-O7-environment-equivalence-preflight-report-v1.md`均为历史证据（后者是2026-07-22预检报告），不占执行槽。本文件直接生成在空的有效当前槽，完成下面自审与复核后激活，无需再询问用户。

## 2. 必要性与验证经济性审查

请求结果：完整方案B开始施工。最小因果改动：公开SDK缺少图形创作与表现合同；Runtime公开坐标仅平移，绘制/命中/clip须统一；补间须接入现有owner/时钟。上述各层是完整链路的真实消费者。

明确排除：新UI框架/Renderer/Provider/Compiler、每节点Canvas、完整SVG/CSS、world vector renderer、项目美术重制、扫描/存档优化、自动切换安装配置、全workspace历史回归。

| Gate | 风险/失败合同 | 最便宜的可失败验证 | 终态证据 |
| --- | --- | --- | --- |
| A 公共创作合同 | 非有限数据/容量/路径状态、矩阵顺序和逆变换、动画区间冲突/溢出、重复推进 | SDK owner测试+公开Game SDK消费者编译/测试 | SDK纯合同完成，明确Runtime尚未接入 |
| B 绘制生产链 | 三角剖分/AA错误、稳定图形重传、资源缺失或旧reader误读 | 扩展/资源owner用例，最终真实wgpu像素 | 路径到生产UI Pass及资源打包可消费 |
| C 表现几何链 | 旋转后点错/漏裁剪、父变换不传播、局部更新退化 | 实例/几何/命中owner及GPU clip像素 | 属性/坐标/绘制/输入一致 |
| D 补间生产链 | 基础owner被夺、取消恢复旧值、重复FixedTick计时、释放回调悬空 | 调度/会话及Generated Glue定向测试 | 实际项目播放/事件可用 |
| E 项目与交付 | 只有内部演示、Release退化、外部源依赖、体验不匹配 | 既有项目compiler/语义输入/性能采样，实际窗口与独立包 | 指定Windows Release资格与完整边界 |

复用361/363的主题、字体、反馈、cache、Projection和现有打包链；证据仅在消费的源码/产物/配置未变时复用。变化的公共schema需实际consumer回归，不因文档变化重跑旧Release；未变安装资格仍属于旧二进制。

估计：A 30–60分钟；B/C/D各一个完整施工窗口（90–150分钟，冷编译另报）；E按实际构建/采样拆两个不超过150分钟窗口。每次连续施工上限3小时，2小时45分不开始新操作，预留证据与cleanup。成本明显超估时先停高成本分支检查首因，不加新runner补偿。本轮开始并完成A后写实际证据，后续不伪报完成。

## 3. Gate A：公共创作与数学合同

文件：`rust/crates/project_runtime_sdk/src/aui_draw2d.rs`、`aui_presentation.rs`、`aui_animation.rs`、SDK `lib.rs`；`project_game_sdk/src/lib.rs`、`tests/aui_presentation_contract.rs`。不改`UiProperty`、旧wire或ABI函数表。

任务：

- 有界typed路径/绘制描述、合法子路径状态、caps/joins/fill、图形错误定位；API Rustdoc可说明坐标/单位/限制。
- Transform2D/Affine2D与parent*local组合、pivot、精度与有限值校验、相对奇异判定；供后续Runtime共享，禁止另写不同矩阵。
- 可插值属性、缓动、序列/并行/循环/反向描述；有限时间/深度/容量和并行写入区间校验。
- UI/游戏表现时钟的一帧一次推进、暂停/速度与clamp数学，Runtime调度接入留D，不能提前宣告生效。
- Game SDK公开re-export及第二crate消费测试，保持当前协议指纹不变；合同作为纯SDK能力不自动意味着旧Host会处理新请求。

验收命令（cwd `G:/gameEngin/rust`）：

```powershell
cargo test -p project_runtime_sdk --lib -p project_game_sdk --locked
cargo test -p project_game_sdk --test aui_presentation_contract --test three_d_sdk_a --locked
cargo clippy -p project_runtime_sdk --all-targets --locked -- -D warnings
cargo clippy -p project_game_sdk --all-targets --locked
```

格式仅检查/格式化新增文件及实际修改的两SDK入口。不能用全workspace fmt改动无关文件。终态：新增合同测试和既有SDK回归通过，记录确切数量；源码完成，不宣称Runtime消费或安装更新。

实际结果：SDK50 + Game SDK21 + consumer2/3，去重76项通过；SDK严格Clippy通过。Game SDK原严格lint被未修改的`kinematic2d.rs:47`既有`needless_range_loop`告警阻断，仅对此告警豁免，all-target lint保留它且无新告警，详见[Gate A证据](../证据/365/Gate-A-执行记录.md)。没有增加allow或修改该无关owner。

## 4. Gate B：图形到生产绘制与资产

涉及：SDK扩展合同/wire；Runtime `aui_extension.rs`、`aui_drawing_cache.rs`、`aui.rs`、`aui_present_cache.rs`及`wgpu_backend`实际UI consumer；既有Asset/RuntimePackage和`project_authoring_execution`装配/Generated Glue（仅真实影响文件）。

任务：在既有扩展绘制通路发布稳定图形描述/handle，支持代码与GUID资源；剖分候选评估并冻结依赖。输入预算、几何精度、共享几何缓存、内容替换/释放、缺失资源/旧reader首因闭合。为路径AA制作真实GPU像素原型后冻结技术，不能只靠mesh数量验收。

wire有变化时更新对应版本/schema、SDK指纹与所有reader/Glue；不改C表就不为治理方便升级ABI。确定最终wire字段后写回正式合同附录及兼容测试，保留旧项目重建路径。

验证（先owner过滤调试，终态同配置suite去重执行一次）：

```powershell
cargo test -p project_runtime_sdk --lib --locked
cargo test -p engine_runtime --lib ui_extension --locked
cargo test -p project_authoring_execution --lib generated_runtime_glue --locked
cargo test -p engine_runtime --features real-wgpu --test aui_draw2d_gpu --locked
```

`aui_draw2d_gpu`是本Gate新增并实际包含图元/曲线/AA/洞/精度/缓存恢复的测试target，创建前不得假装已存在。资产owner另选择实际改动的现有装配测试，命令写入Gate执行记录。输出必须进入既有UI Pass，Runtime不读项目源。

2026-10-08施工校正：原过滤`aui_extension`实际匹配0项，不能当证据；改为真实native消费者`ui_extension`。新增geometry owner过滤`aui_graphics`、Assembler cooked资产同名过滤，既有`aui_extension_c4`覆盖旧mesh/文字/图片通路。实际合同已冻结在方案14.2；不改变ABI函数表，不新建绘制Pass。

实际结果：B完整源码/资源/Generated DLL/UI Pass/GPU链通过，去重105项有效测试。清洁帧新增剖分/上传0；DPI恢复、平移、资源替换及裁剪alpha逐像素通过。夹具首因/修复、源码身份、AA和lint边界见[执行记录](../证据/365/Gate-B-执行记录.md)。B完成，不授予C/D/E、Release性能或安装资格。

## 5. Gate C：仿射表现、裁剪与命中

涉及：`aui.rs`、`aui_instance.rs`、`aui_session.rs`、`aui_geometry_cache.rs`、`aui_hit_test_index.rs`、`aui_drawing_cache.rs`、`aui_feedback_cache.rs`；SDK `aui.rs/aui_wire.rs/aui_host.rs`；Renderer UI geometry/clip consumer及实际Glue。

任务：新增typed rotation/scale/pivot/opacity/tint/resource/local-style属性；same-value、owner和原子提交延续旧合同。屏幕布局转父级局部后统一矩阵；AABB宽阶段+局部窄阶段；不可逆节点不命中。精确旋转clip使用现有可扩展clip链，按真实GPU原型冻结shader/mask策略和8层预算。

只transform/tint/opacity不重做路径剖分和glyph shaping；父变化只更新必要后代，清洁兄弟保留。基础属性→动画→反馈的共享最终结果供输入/坐标/绘制消费。记录旧反馈兼容影响，禁止画面一套矩阵、命中一套。

```powershell
cargo test -p engine_runtime --test aui_retained_instance --locked
cargo test -p engine_runtime --test aui_retained_layout --locked
cargo test -p engine_runtime --lib --locked
cargo test -p project_runtime_sdk --lib --locked
cargo test -p runtime_player_winit --lib --locked
cargo test -p engine_runtime --features real-wgpu --test aui_presentation_gpu --locked
```

新增GPU target必须验证镜像、pivot、嵌套clip内外像素、DPI及裁剪漏画；定向输入验证通过原命名View拖拽转换。suite重叠时执行覆盖最完整的配置，记录替代关系，禁止重复刷通过数量。

2026-10-08 C施工复核：采用正式方案14.3的CPU仿射/凸轮廓裁剪与既有UI Pass，8祖先clip+自身图片圆角，0.25屏幕像素弦高、每角256段预算，超限诊断拒绝。UI_UPDATE7/UI_HOST5同步真实消费者，ABI1.6保持。资源Binding与Controller均校验挂载ID/GUID/type；纯表现不引发布局，逐属性稀疏覆盖保留其它Binding。Player/Host成功发布最终几何，反馈与SDK同矩阵；记录反馈layout COW成本，不在C提前宣称E性能。

验证经济性校正：原`aui_geometry`过滤未能证明实际owner测试，改为Runtime完整lib一次，覆盖新增几何/反馈/Session及已有命中回归，替代重叠filters。增加SDK严格Clippy、Editor编译消费者、Generated Glue实际DLL与B绘制GPU target（Renderer变化直接影响），不跑workspace/安装/塔防Release。新增测试使用现有owner seam，不新建Runner。编译失败若仅新增夹具API误用，在夹具修正；断言失败先定向owner修复再终验。

实际结果：C完整SDK/Binding/Controller/几何发布/输入/Renderer/真实DLL与Vulkan消费通过，去重951项。最终Runtime753、常驻实例/布局40、SDK53、Player81、Glue13、绘制consumer10、Editor依赖1通过；SDK严格Clippy及real-window/Editor编译通过，Runtime125项既有告警保留。千节点单叶更新1几何，兄弟draw Arc复用；三个DPI非中心pivot/非均匀缩放/镜像/八层clip与alpha像素通过，静止零新增剖分/上传。越Canvas多边形缺口与悬停提亮提前饱和已由red owner测试修复并复验。首因、身份、复用、COW/精度和安装边界见[C执行记录](../证据/365/Gate-C-执行记录.md)。C关闭，不授予D/E、OS窗口运行、Release性能或安装资格。

## 6. Gate D：组合补间与Runtime生命周期

2026-10-08 D接入复核：复用Gate A描述/PresentationClock、C的常驻属性journal/最终几何、现有UiQuery及UiEvent callback ID通路。只在会话保留有界播放实例和活动属性覆盖，不建立第二套layout/paint/cache。UI_HOST更新v6以纳入真实动画请求/事件合同，UI_UPDATE7、扩展2、ABI1.6保持；旧DLL须重建。状态型query输出预检4096字节且不重试；请求最多1MiB，描述仍256叶/32深。动画播放/暂停/取消/游戏时钟变化随既有callback接受边界提交，拒绝即回滚；返回过的回滚handle不会再次有效。

运行阶段：Player输入先读上次已求值/发布动画几何，随后取消DragStart冲突translation、每frame一次推进并按属性journal必要重建，再组合原控件反馈、发布本帧最终几何。headless/语义回放使用固定微秒delta；真实窗口使用两帧呈现开始间隔，最大250000微秒，与反馈共享。结束事件排队，到后续安全session阶段由现有ui_event派发，不在paint调用项目代码。Active scope由owner及目标有效Active共同暂停；Hidden不取消；Hold仍占有播放scope，完成通知一次，之后释放不重复结束事件。CommitFinal仅可写入预期Controller owner/revision未变的基础属性；Binding owner保留并返回CommitConflict，不隐式夺权。

跨播放默认仅拒绝当时活动的重叠轨道；有delay/串行间隙的轨道到首次实际重叠时Reject实例收到Conflict并释放，Replace实例取消旧播放。容量4096实例/总轨道及有界事件队列；代际身份防slot复用/重开悬空。成本仍90–150分钟，终态按受影响Runtime/SDK/Game SDK/Player/真实Glue消费者验收，定向调试由最终suite覆盖去重；GPU/Release与项目性能留E。

涉及：SDK动画请求与会话/帧输出；`aui_session.rs`、`aui_instance.rs`、`aui_control_feedback.rs`及既有frame/session owner；`runtime_player_winit`帧阶段；Generated Glue和callback ID派发。

任务：每播放实例的generation handle，活动轨道索引、时钟接入、pause/resume/cancel/complete/hold/commit，串并行/loop/reverse，节点/项目生命周期取消。默认Reject、显式Replace；基础Binding/Controller不被夺，覆盖结束读取当前base。CommitFinal检查预期owner/revision，Hold绑定scope。

接入UI及项目游戏clock，每frame index一次推进，安全完成事件不重入paint、不跨DLL传closure；Active停用不追补。拖拽取消冲突位移动画后直接跟鼠标。控件内部反馈共享最终组合和原capture/cancel行为；不复制另一套全量scheduler/cache。

```powershell
cargo test -p engine_runtime --lib aui_animation --locked
cargo test -p engine_runtime --lib aui_control_feedback --locked
cargo test -p engine_runtime --lib aui_session --locked
cargo test -p project_runtime_sdk --lib --locked
cargo test -p project_authoring_execution --lib generated_runtime_glue --locked
cargo test -p runtime_player_winit --lib --locked
```

新增owner测试须覆盖同帧多FixedTick、base中途变更、并行冲突、取消/complete事件单次、slot复用和重开；真实项目使用放入E，不能以纯SDK描述验证冒充D完成。

2026-10-08 D终验经济性复核：最终Runtime完整lib及两个常驻实例/布局target替代上方重叠的动画/反馈/Session过滤。SDK、Game SDK、Player lib覆盖公共supertrait/查询和直接消费；Generated Glue真实DLL验证setter前后混用播放、完成安全派发和NoOp回滚。SDK严格Clippy、Runtime real-wgpu lib Clippy与Editor/Player real-window check只检查实际跨crate影响，不重复GPU像素或发布旧Release。新增生产cache验证最新Binding base、局部几何/绘制复用；4096满额仍可控制原播放。播放不强制flush，接受后捕获CommitFinal stamps。没有新Runner/缓存框架、lint allow或安装变更。

实际结果：D组合补间、双时钟、ownership/接受回滚、生命周期与公开SDK/真实DLL/Player生产帧接入闭合，去重985项通过（Runtime775、常驻实例/布局40、SDK55、Game SDK21、Player81、Glue13）。SDK严格Clippy、Editor/real-window编译及格式通过；Runtime仍125项既有告警，无新增告警。千节点单叶更新1几何、clean兄弟draw Arc复用，Binding覆盖期间base更新及取消恢复最新值通过。Yoyo峰值、ownership-only batch漏发与Result envelope解码已修复；首因、身份、成本及边界见[D执行记录](../证据/365/Gate-D-执行记录.md)。D关闭，E/窗口运行/Release新性能/安装资格未取得。

## 7. Gate E：两普通项目、性能与独立交付

2026-10-08续工口径（用户明确授权缩短）：替换原双28000帧窗口要求为三关各2168帧、各两遍同包窗口切片；每次120暖机+2048 CPU样本、至少600有效战斗样本、AUI含项目typed表现P95≤2ms、CPU P95≤16.67ms。20轮生命周期采用2000帧Off/捕获各一遍，观察内存只作短时趋势，不再要求旧60秒稳定样本。完整30波语义复用已有同包headless；不声称短窗已排除历史长窗超时。普通启动仍北门，显式捕获可选原关卡起点；只修改项目Session/验收helper与输入，未变Engine DLL/A–D/展示矩阵证据按身份复用。每个窗口120秒上限，诊断先用原包3000帧/65秒确认短路径；无新故障证据不扩展引擎修复或新增进度协议。本整块预计30–60分钟；超预算先收敛失败位置，不回到28000帧调试。

项目一：新普通UI展示项目（目录在实际create时冻结），公开接口展示图形、嵌套变换/输入和补间；项目二：塔防小范围视觉切片，仅接入通用能力，不扩展全项目美术改造。修改塔防前加载tower-defense-project skill与项目自身入口。

使用现有check/build/playtest/observe/交付验证，真实窗口证明曲线/clip/alpha/汉字glyph和输入；两项目均不能引用engine private symbols。沿用原三关语义测试，记录create/change/repair业务改动及错误定位成本。

固定新增负载：400节点、64共享路径实例、每路径≤128命令、128活动轨道、8层clip；Windows Release1280×720、DPI1/1.5/2，300暖机+3600样本。分项负载及质量不删减，AUI P95≤2ms、CPU P95≤16.67ms；P50/P95/P99/max、GPU可得时间、geometry/cache/upload计数及初始化另报。363旧三关同条件回归维持原2/16.67ms预算。

2026-10-08 E执行复核：展示目录冻结`samples/aui_presentation_showcase_project`，塔防消费仍普通`Projects/hezi_shoucheng`。现有Player采集CPU/GPU同数量使3600请求被GPU2048上限拒绝，既有theme-dpi也不提供冻结三档；仅补已有诊断负载枚举固定DPI1/1.5/2、GPU有界采集与CPU完整采集分开计数，不改OS设置、不新建Runner。对应Player/CLI直接测试和Release更新，未变A–D证据按身份复用，详见[E记录](../证据/365/Gate-E-执行记录.md)。

```powershell
cargo test -p engine_runtime --lib --locked
cargo test -p project_runtime_sdk -p project_game_sdk -p project_runtime_abi --locked
cargo test -p project_authoring_execution --test game_project_compiler_interface --locked
cargo check -p runtime_player_winit --features real-window --locked
```

构建/窗口/性能使用现有仓库工具及项目helper；运行前在执行记录冻结准确命令、binary identity、profile与硬件，不写虚假的通用脚本名称。GUI/GPU测试采用新Gate B/C target及现有device恢复consumer，身份未变则复用。独立包复制到明确交付目录，断开源/Library依赖后运行，20轮重开验证资源与回调收敛；不以短时内存观测声称无限无泄漏。

不以AUI专项与其它引擎完整CPU混比；无法采集UI时只比较同口径frame。实际安装和默认配置切换不属本轮，交付源码与安装可用分开报告。

## 8. 文档自审与激活复核

2026-10-08自审结果：

- 正式方案复审有必须补充项，已回填14.1节后生成本文件；没有不适用外部审查被强行纳入。
- 全部五模块映射正式方案完整链路，Gate A只声明公共合同，不把B–E列为完成。
- 必须验证的失败合同有owner/consumer、像素或项目证据；新增测试target明确标记将创建，现有target已核对存在。
- ABI/wire不空改版本；纯函数先落SDK，真实数据消费阶段同步协议。当前安装不变。
- 每窗口≤3小时，第一窗口以A的完整模块为终态。冷编译、已有基线失败保留首因；普通修复自主继续，不反复用真实窗口调试。
- 激活前复核：方案365已确认，用户本轮明确授权施工；无其它有效current/pending文档，涉及文件/命令与当前crate基线匹配；工作区脏但可增量修改，无需另建worktree。

结论：施工文档自审通过，可激活并执行Gate A。无需再次向用户确认。

## 9. 执行记录、同步与归档

终态：A–E按用户修订范围完成。三关各双2168帧AUI含项目表现/CPU P95最高1.6229/3.9001ms；20轮2000帧窗口40设置回调、关闭回调0、重开后动画0。项目56测试通过，展示15组矩阵/900项owner/未变A–D证据复用。原包3000帧正常，历史长窗失败保留未定位；短时内存尾段增长约0.99MiB、CPU偶发长帧51.9284ms不删样。新包断源通过并恢复、owned进程0，安装364未改。见[短窗口终验](../证据/365/Gate-E/短窗口续工与验收报告.md)及[总完成记录](../../阶段完成记录/2026-10-08-365-Draw2D-Presentation-Complete/00-总览.md)，归档后执行槽为空；后续安装接入单独处理，不自动修改配置。

各完整模块完成后同步49/54和本文件实际进度；总文档保持当前直至A–E实际闭合。阶段记录不能把整个365归档；全部通过后才写总完成记录、同步入口并将本文件移动到已完成。失败先修当前Gate，不跨Gate堆未验证实现。
