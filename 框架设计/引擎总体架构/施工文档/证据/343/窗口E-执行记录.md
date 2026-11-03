# 343 窗口 E：AI 双效果制作与修改复验

状态：2026-09-15（Asia/Shanghai）Gate E `dual_effect_authoring_and_window_validation_passed`。普通粒子项目、两例首版与各两轮指定修改、实际Windows关键帧审阅、各5次重播、暂停/恢复/重复play、世界空间源移动及同包普通/语义运行已完成。343整体未完成、未归档；F大型负载、局部失效和独立复制交付未执行，338暂停，Provider未安装。

本轮授权“开始施工gate E”。05:25:47开始，06:46:45完成最后身份/进程复核，约81分钟；未触及08:10:47停止新工作/08:25:47硬停止线。验证dirty worktree，基准HEAD为`acaffa8cb95bbb54d093bbf45835a59315c82da5`，不是该提交的发布资格。源码、测试与运行身份见[38文件摘要](窗口E-源码与运行身份.json)。

## 交付内容与制作过程

普通项目：[particle_effects_project](../../../../../samples/particle_effects_project/README.md)。33个项目目录文件（含资产PNG、测试、Cargo.lock、离线生成器和README，排除Library/target）；核心作者效果只有2个JSON和1个WGSL。项目Rust仅提交公开SDK控制意图与观察值，没有逐粒子CPU模拟、专用Engine效果模块或新AI工具。

- A：暖色闪光/向上火花/拖尾、灰蓝4帧烟雾、受重力的真实三角网格碎片及有界碰撞子火花；169总容量。修改1只改变火花Y速度，修改2只缩短/淡化烟雾。
- B：192粒子自定义WGSL，外围吸附→绕倾斜轴上升→爆开，参数包含方向、半径、上升速度、阶段时刻。修改1为JSON反转/扩大，修改2真实改WGSL为高度决定爆开时刻与速度；4秒清空。
- 固定1280×720、相机(0,0,10)/正交半高6.75、seed42、60Hz；每轮修改前登记预期、修改后打开实际帧。无人工编辑作者文件或节点，无外部美术生产干预。

[制作预登记](窗口E-制作预登记.md)、[首败与修复](窗口E-首败与修复.md)、[旧v系列过程审阅](窗口E-逐轮视觉审阅.md)、[最终q系列逐轮审阅](窗口E-最终案例复核.md)。v系列发现遗漏flipbook/上升后不计入最终制作资格；补齐后完整重做q0→q1→q2。

## 最终证据入口

所有版本的原始请求/响应、作者快照、源SHA、包/模块/程序/纹理/网格SHA均在`.artifacts/343-particles/<version>`。捕获索引指向未修改原图；contact/detail图仅拼接/裁剪辅助审阅，不是生成的效果图。

| 案例 | 首版 | 修改1 | 修改2 |
|---|---|---|---|
| A | [q0，5帧](../../../../../.artifacts/343-particles/q0-a-final-pass/contact-sheet.png) | [q1，5帧](../../../../../.artifacts/343-particles/q1-a/detail-sheet.png) | [q2，5帧](../../../../../.artifacts/343-particles/q2-a/detail-sheet.png) |
| B | [q0，15帧](../../../../../.artifacts/343-particles/q0-b-final/detail-sheet.png) | [q1，15帧](../../../../../.artifacts/343-particles/q1-b/detail-sheet.png) | [q2，16帧](../../../../../.artifacts/343-particles/q2-b/detail-sheet.png) |

[四份局部diff与保护项摘要](../../../../../.artifacts/343-particles/qualified-diffs/author-diffs.json)证明A各轮只改目标发射器、B1程序不变、B2程序确实变化；每轮Cooked身份不同，最终A=`6D437DF275405DC69138D5AA4899D41BA50A8242A3346662766072EEE9931B4E`、B=`93EACBC1FCD62BC7F99D7AB3B6DB20B8746A7666A8482777742BF67408254954`，之后稳定性/空间检查保持。

- [B补充邻帧](../../../../../.artifacts/343-particles/q2-b-continuity/detail-sheet.png)：保留原时点并补新边沿，无阶段跳位。
- [A五次重播](../../../../../.artifacts/343-particles/q-final-a-stability/contact-sheet.png)：1261tick/5断言/13捕获；[B五次重播](../../../../../.artifacts/343-particles/q-final-b-stability/contact-sheet.png)：1561tick/5断言/13捕获。均通过playtest与observe。
- [源移动](../../../../../.artifacts/343-particles/q-final-world-space-pass/contact-sheet.png)：260tick/3断言/5捕获；真实source_x=3.0，旧烟雾不跟随。
- [像素核对](../../../../../.artifacts/343-particles/capture-differences.json)：暂停两帧相同、恢复变化、5次空帧相同。B重播最多74像素差1/255；源移动1.60秒仅落地7像素差1/255、烟雾区域0差异；未见残影，不冒称跨GPU逐位确定。

最终共同交付：`samples/particle_effects_project/Library/EngineTools/Deliveries/engine-op-343-q2-b-20260914-222753-3/Playtest/Windows/dev`。交付身份`sha256:280ed3c3749668e8480ba4d34d3a3c16f5ba648834aec629f87967feea9d694a`，包身份`sha256:d63467f09b7183b1717ef059feafb301b5759a94d9e2e5a79174674adaa6f082`。

该包已有B260tick/2断言/16捕获/observe；再由同一个Game.exe执行A260tick/2断言/5捕获，见[同包A结果](../../../../../.artifacts/343-particles/native-semantic-a/playtest/result.json)与[帧审阅](../../../../../.artifacts/343-particles/native-semantic-a/contact-sheet.png)。普通窗口A22帧、B74帧均完成、退出0、截图captured：[A窗口](../../../../../.artifacts/343-particles/native-a/window.png)、[B窗口](../../../../../.artifacts/343-particles/native-b/window.png)。三次启动的launch.json和Player报告保留完整参数/进程。最终包资源和DLL逐文件SHA未变。

这是同开发交付运行验证；没有复制至屏蔽源目录的独立环境，不能代替F。当前作者目录只修正了额外世界空间scenario的Number字面量和README；实际运行效果、模块与资源均与共同交付匹配，未把当前完整源摘要冒称历史包摘要。

## 实际发现的Engine问题

| 唯一责任点 | 失败与修复 | 定向证据 |
|---|---|---|
| particle_effect/program.rs | 负平面距离生成非法`--2.6`，减数加括号 | [红](../../../../../.artifacts/343-particles/negative-plane-red.log) → [Cooker绿](../../../../../.artifacts/343-particles/cooker-green.log) |
| engine_host_loop.rs | 2D相机单位Z矩阵把world z=0裁出WGPU视锥；复用已有2D深度映射 | [红](../../../../../.artifacts/343-particles/camera-depth-red.log) → [绿](../../../../../.artifacts/343-particles/camera-depth-green.log)，另有真实窗口 |
| engine_tool_provider/semantic.rs | 独立playtest漏用已有关闭额外Player预检选项，窗口前先跑不支持粒子的headless | 复用普通build的`with_player_verification(false)`；[原有puzzle真实playtest/observe回归](../../../../../.artifacts/343-particles/provider-playtest-regression.log) |

没有为了样例移动相机躲裁剪、放宽作者合同或添加prepare/粒子专用工具。独立playtest仍自行准备并以真实场景验收；普通headless run仍明确不支持GPU粒子。其摘要未透出粒子unsupported细节是已记录局限，本Gate未扩展诊断架构。

## 命令、耗时与验证边界

定向Rust测试去重21项：Cooker15+公开快照1、相机consumer1、Provider现有集成1、最终项目控制/资产测试3。失败重跑不累加；A–D有效证据复用。命令在rust目录（项目测试从仓库根）：

```powershell
cargo test -p project_authoring_execution --locked particle_effect -- --nocapture
cargo test -p engine_runtime --locked scene_camera_2d_updates_game_view_projection_from_hydrated_component -- --nocapture
cargo test -p engine_tool_provider --locked --test headless_execution_owner switch_puzzle_semantic_outcome_uses_its_own_bool_contract -- --nocapture
cargo test --manifest-path samples/particle_effects_project/RuntimeModule/Cargo.toml
```

项目最终构建0.69秒、3测试通过，见[原始输出](../../../../../.artifacts/343-particles/project-tests-qualified.log)。受影响Rust与项目格式检查通过。源码Provider仅为独立stdio调用而重建；SHA=`3E0E662BA7BB46282E11DF6FBF9C74C6A278B7CD5E57A7787D92A9865E33EA5A`，没有安装或修改宿主绑定。

各轮由run-owned `project-tools.ps1 -Version <label>`执行现有inspect/check/playtest/observe；普通窗口脚本只调用既有CLI，无新生产runner。精确墙钟、工具、Player进程及缓存报告见[逐轮汇总](../../../../../.artifacts/343-particles/q-summary.json)。下表秒数：

| 版本 | check | playtest总计 | 其中Player进程 | 其余准备/封装/报告等 |
|---|---:|---:|---:|---:|
| q0 A | 7.594 | 30.916 | 10.532 | 20.384 |
| q0 B | 7.729 | 31.329 | 13.127 | 18.202 |
| q1 A | 7.662 | 27.986 | 10.493 | 17.493 |
| q1 B | 7.519 | 30.706 | 12.882 | 17.824 |
| q2 A | 7.866 | 27.837 | 10.487 | 17.350 |
| q2 B | 7.654 | 31.192 | 13.226 | 17.966 |

q0 A包含最后一次项目observer代码修正，模块构建1.210秒，格式/依赖锁/describe为.188/.138/.770秒，稳定Engine命中缓存。之后q0 B至全部q1/q2都为project artifact hit/Engine artifact hit、cargoRebuilt=0。每轮同时有正常check与playtest内部准备，上表“其余”不是纯编译时间，不据此重新开展性能优化；P9孤立参数/程序失效测试仍留F。

首张真实可见A在21:57:21 UTC，距开工31分34秒，含Engine修复、编译和失败；B分布修正后于22:00:15打开，距开工34分28秒，见当时v系列审阅记录。这两次还不含后来发现遗漏的flipbook/上升。补齐后的首版A运行22:18:41完成、B22:22:50完成，均在下一轮修改前实际打开；q0图像打开未单独记录精确秒数，不把运行完成时刻冒充打开时刻。

q1 A作者快照22:23:34→22:24:10运行完成→22:24:42切B前完成审阅，修改至复看上界68秒；q1 B快照22:24:42→22:25:21完成→22:26登记审阅，上界78秒。q2 A快照22:27:00→22:27:36完成→22:27:53切B前审阅，上界53秒；q2 B快照22:27:53→22:28:32完成→22:29:07下一scenario前审阅，上界74秒。这些上界含记录/调度，源快照不是首次键入时刻；没有独立纯思考/阅读计时，不把这些数声称为端到端创作速度或与Godot/Unity的等条件对照。

## 自审与收尾

真实帧与控制合同共同验收，未以自动visual=passed代替主观审阅；原先空白窗口曾被自动通过，已明确保留为失败。13组制作/引擎问题及修复都有记录，其中3组触及Engine；没有人为编辑或预制效果替代作者过程。正式要求中的flipbook、上升、原/新阶段边沿、5次重播和同包消费均已补齐。

Gate E完成P6的创作/修改/复看部分，并补P4/P5实际案例；第7.4的独立交付及P7/P8/P9总资格须在F最终复核，不能据此宣称P1–P9全部通过。默认无每帧GPU回读，图像分析只是本轮离线证据。Windows dev不扩展为release/Android或大型效果性能承诺。

收尾无本轮Game/源码Provider或cargo/rustc遗留；既有安装Provider保留。源、失败产物、缓存和所有图像留存，未删除用户数据。受影响格式/diff检查与43个本地文档链接检查通过。49/54、343方案/施工状态及索引同步为E完成、下一窗口F；顶层v1.1、九工具、338暂停与安装态保持。
