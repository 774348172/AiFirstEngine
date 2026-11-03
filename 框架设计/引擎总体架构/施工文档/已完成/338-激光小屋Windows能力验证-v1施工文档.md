# 338 激光小屋 Windows 能力验证 v1

状态：2026-09-15已完成。用户在本会话确认“我已经验收通过了”，闭合真实鼠标PAUSE/恢复/RESTART最后验收；复用既有源码、综合交付及两个制作变体证据，归档为Windows能力验证完成。安装宿主未升级。

## 2026-09-15 最终验收与归档

用户在此前明确列出的鼠标验收项后回复“我已经验收通过了 下一步该做什么”，作为人工试玩通过证据；不是自动化点击报告。339–343和本轮338源码/交付证据身份未因文档收尾改变，未重跑测试、构建、截图或音频回录。完成记录见[338总览](../../阶段完成记录/2026-09-15-338-Laser-House-Windows-v1/00-总览.md)。下方执行时状态保留历史身份，不覆盖此终态。

## 2026-09-15 恢复复核与施工自审

- 用户授权：恢复338，把新粒子能力接入激光小屋，完成一版可实际试玩的综合验收。343已归档、当前/待执行无其它有效施工。
- 最小变更：项目RuntimeModule、粒子Assets、Scene、Tests/Observations。使用既有粒子公开SDK，接入激光火花、死亡消散和出口/通关反馈；新增房间必须根据项目数据确定终点，不能固定第三房通关。
- 不扩建引擎、光照/Bloom、新工具或Scene生命周期系统；不安装Provider、不改宿主绑定。源码Provider独立进程消费项目，最终Windows dev交付。
- R1：效果触发、位置、暂停/重置风险 → 项目事件/意图定向测试和真实场景编译 → 项目测试日志、作者资源。
- R2：效果未真实绘制/遮挡、玩法受损 → 同交付主路线与死亡分支playtest/observe、关键截图人工式复核；真实鼠标暂停/恢复/重开 → 窗口输入与截图/报告。
- R3：参数改动/额外房间仅内存夹具 → 两个真实项目变体各build/玩法检查/截图；复用原逻辑，不新增框架API → 变体源码、交付和证据。
- 复用339颜色、340字体、341动画、342音频、343引擎粒子回归/规模/独立交付证据；本轮项目修改仅刷新游戏综合交付，不重复全workspace、底层GPU性能或音频回录。
- 预计60–120分钟；异常先定位项目owner。授权开始约05:36 UTC，最迟08:21 UTC停止新增工作、08:36 UTC停工并留证。真实窗口工具若不可用，保留精确缺口，不以语义注入冒充鼠标点击。
- 自审：既定项目表现与制作验证范围，未改变权威v1.1或343设计。原三房间策划保持，第四房间为独立制作变体；以用户试玩产物和可追溯验收为终态。下面2026-09-14恢复范围和记录保留历史身份。

依据：[激光小屋游戏策划 v1](../../../验证Demo/激光小屋-游戏策划与引擎能力验证-v1.md)。项目目录为 `samples/laser_house_project`，首期Windows dev，九项工具与已安装宿主配置保持。首轮使用337 Provider；后续可复用342源码Provider及已验证交付，安装/宿主绑定状态以49为准。

## 本次恢复复核与最小执行范围

- 342归档释放执行槽，原有338恢复授权保持；本次不重跑已经完成的颜色、字体、Animator2D与音频Gate。
- 当前剩余项目验收：把房间二平台端点/速度修改、复用机关组成额外测试房间，分别落实为真实项目变体交付；补鼠标AUI暂停/恢复/重开按钮的实际输入证据。原有owner测试不能替代这些真实制作结果。
- 用户既定的后续能力方向仍为粒子/光效和更好的美术资源，然后完整Scene生命周期，再进行成熟引擎对比。先用现有Sprite/SDK与Godot实际实现区分项目表达/资源问题和引擎能力缺口；不因本次恢复直接新增通用粒子、光照、Bloom或新的API。
- 字库bundleDigest `sha256:c78679021bbaac9371e7b281c48d0c4bacabc00fa05a77bd8b586c82c0bfd67c`及339–342已有证据继续有效；原始首轮和字体施工记录保留历史身份。源码、安装宿主和交付资格仍分开记录。
- 已确认项目范围可在本文件下继续；需要新的通用Runtime边界时另行讨论/确认方案。此次用户询问下一步仅触发复核与既有收尾，没有自动开工新的系统。

## 必要性与验证经济性自审

- 目标：三个房间的角色跑跳、机关、移动平台、相机、暂停/失败/重开、基础表现和可重复验收；再完成平台参数修改与额外房间复用试验。
- 最小涉及文件：新项目的 RuntimeModule、Scenes、AUI、Input、Assets、Observations、Tests；必要时在现有 `project_game_sdk` 中添加项目无关的纯数据矩形运动辅助。真实游戏只使用公开 SDK 与已有组件/派生胶水。
- 基线：SDK 已有 WorldRead、Transform/组件延迟写入、输入、AUI 和观察；引擎已有相机 Transform 投影。Physics2D 现有重叠/碰撞对不等于角色阻挡响应，不能据其名称声明可用。
- 首轮音频缺口已由342闭合：真实音源、Runtime播放和用户听感确认均有证据。其余未完成项以顶部恢复复核为准；不再把“尚无音频API”作为当前状态。
- 排除：Android、3D、完整刚体/斜坡、联网、存档、新工具、构建优化、安装替换、全 workspace/Local CI。相机、物理辅助不引入新的 ABI 或私有引擎依赖。
- 复用：337 Provider/Host/DLL 安装与运行边界证据；只对新增项目、改动 SDK 和直接消费行为验收。旧项目文件与 dirty worktree 保持。
- 成本：预计 60–150 分钟；单次真实构建/试玩约 2–5 分钟。慢测试/失败先回 owner 定向定位，超过估算不追加全量验证。遵循三小时上限。

| Gate | 任务与风险 | 最小验证 | 终态证据 |
|---|---|---|---|
| A 第一房间 | 项目合同/SDK消费；跑跳撞墙、落地、踏板开门、失败重来 | SDK矩形运动测试；项目逻辑测试；独立Provider check/build和短窗口 | 项目源码、检查/构建报告、第一房间截图 |
| B 三房间 | 平台承载、相机、周期激光、暂停、换房与终态恢复 | 项目确定性测试覆盖真实规则与状态；相机/表现通过窗口观察 | 三房间项目与玩法断言 |
| C 交付与制作能力 | 构建成功但真实玩法失败；修改需要复制核心代码 | 原有playtest/observe场景；真实Player截图；平台参数/第四测试房间试验 | 交付路径、语义报告、截图、能力缺口记录 |

## 执行与验收

先完成房间一，再推进其它房间。房间数据、机关规则、角色跳跃参数属于项目；纯矩形运动辅助没有房间/机器人/激光概念。相机使用真实相机实体，不通过移动所有世界对象伪装镜头。

定向命令：`cargo test --manifest-path samples/laser_house_project/RuntimeModule/Cargo.toml`；SDK变化时 `cargo test -p project_game_sdk --locked`（rust目录）。真实项目使用既有Provider的 check/build/playtest/observe；原宿主仍绑定打飞机，项目验收由同成品独立进程绑定 laser_house_project。

视觉使用统一几何风格原创小型素材，AUI承担HUD与菜单。动画可通过项目动作状态驱动已有精灵/Transform，不宣称已经验证完整动画系统。音效必须有真实消费证据，缺失则保持未完成。

自审结论：方案方向已由用户确认；本文没有改变顶层架构，也没有追加通用发布系统。当前能在既有SDK/组件边界推进的部分直接实施。音频需新合同的情况不强行纳入源代码变更；只记录已证实缺口，不能弱化完整策划的完成条件。

## 收尾条件

全部策划完成条件和必要证据成立后生成完成记录、更新49/54并归档。若存在音频等需讨论的框架边界，记录实际交付、已通过Gate与明确未完成项，保留当前施工，不把无声版本归档为完整策划完成。

## 2026-09-14 首轮施工结果

已生成真实三房间 Windows 无声可玩切片；当前仍未归档。详细证据与限制见 [项目能力验证记录](../../../../samples/laser_house_project/Tests/能力验证记录.md)。

- Gate A：SDK 9 项、项目检查/构建、首房间 8 断言与可见窗口通过；首帧 FixedUpdate 未注册导致的空画面已由项目显式注册修正。窗口创建成功不等于逻辑成功，旧失败保留。
- Gate B：三房间代码实现，项目 6 项测试通过；平台承载、跳离、周期激光、暂停、通关和恢复均已覆盖。音频未实现。
- Gate C：最终真实主路线 1020 tick/13 断言/4 捕获/observe 通过；同交付失败路线 380 tick/6 断言/2 捕获通过；普通窗口 120 帧截图通过。参数修改/第四房间目前限 owner 测试，变体实际交付和鼠标按钮点击尚未验收。
- 真实交付：`samples/laser_house_project/Library/EngineTools/Deliveries/engine-op-338-20260913-161549-2/Build/Windows/dev`。未修改当前安装或原宿主项目绑定。所有已启动的有界验证正常退出。
- 未扩大施工：当前编译器丢失精灵初始颜色，项目以彩色纹理交付；部分字号/小写字形倒置，采用24px大写提示并复核。通用问题留证，不追加宿主构建/安装升级。
- 资格边界：同Scene分组换房不等于引擎Scene生命周期验收；Transform动画不等于Animator2D验收；无声版本不等于完整策划完成。下一轮先收拢剩余项目验收，音频Runtime新边界需要讨论，不能直接扩建。

本轮从约23:40至次日00:20（北京时间），约40分钟，未触及施工时限。复用337固定Host/Engine DLL，未运行无关workspace、Local CI或重复旧性能测量。

## 339 追加施工：颜色编译保真

### 施工文档自审

目标是修复 338 已确认的 `parse_entity` 丢失 Sprite 初始颜色；最小文件为 `neutral_assembler.rs` 及其 owner 测试。排除材质系统、资源烘焙、安装升级和其它表现能力。Gate A 为颜色解析单元/装配测试；Gate B 为 `project_authoring_execution` 测试及激光小屋项目回归；Gate C 为新交付的 RuntimePackage 字段检查和 Windows 截图。失败时保留首个编译诊断，不以默认白色降级。

### 执行结果

- 已在 `parse_entity` 增加 `#RRGGBB` / `#RRGGBBAA` 到 `[f32;4]` 的严格解析；非法值返回 `sprite_renderer2d.color_invalid`。
- 已通过 `cargo test --manifest-path rust/Cargo.toml -p project_authoring_execution --locked`（68 项既有测试通过）以及激光小屋 RuntimeModule 6 项测试。
- 仍需在下一次 Provider 构建中确认新编译器进入交付并检查 RuntimePackage 的静态 Sprite 数值颜色；当前固定 Provider 尚未因本次源码改动而更新安装。
- 动态机关颜色继续使用项目 Runtime deferred mutation；不宣称材质实例能力。
- 2026-09-14 Gate C 复验完成：339 Provider 构建身份 `CADF880FEE38E75461D893D9519B82A62BD28E08B029B489D667D6652F515547`，安装预检 9 项工具通过且 config 未变。新 delivery `engine-delivery:sha256:8fe8f2622e0af6fa5ac254c11c14a149188a464f99e36696abe48e3f9bfe84b3` 的 RuntimePackage `Scenes/scene-main.json` 静态颜色已为数值 RGBA；Windows Player 120 帧 `presented`、截图成功，见 `.artifacts/338-laser-house/color339-window.json` 与 `color339-window.png`。339 颜色字段修复的集成验证通过。

## 340 追加施工：AUI 字体渲染保真

施工文档自审：目标是复现并修复已观察的字形方向问题，排除 schema 升级、字体编辑器和其它表现系统。Gate A 使用 A/g/p/y/2 与多字号定向测试；2026-09-14 红测试已定位 sealed 内置 FontPack MSDF 倒置，新 Cook 和 Renderer 正常。最小修复为现有生成器重建 `resources/runtime/font-packs/aife-default-zh-cn-common-v1/cooked`，不改 Renderer/Cook 算法。Gate B 复跑同一方向测试及字体加载直接消费者；复用已通过 Renderer 测试（生产代码未改）。Gate C 的 Windows 截图仍为独立交付验收，不将源码通过冒充安装生效。

Gate A 首个失败 `.artifacts/340-orientation-first.log` 为 qualification fixture 缺 g；改用仓库内完整 Noto 字体且保留五个指定字符。有效红证据 `.artifacts/340-orientation-complete-font.log`：sealed MSDF 正向/镜像 mismatch 分别为 A 458/110、g 375/141、p 418/124、y 476/118、2 421/83；新 Cook 对应 24px 为 110/458、141/375、124/418、118/476、83/421。Renderer 定向测试 1/1 通过，见 `.artifacts/340-renderer-orientation.log`。只重建产物，预计生成 5–15 分钟；超过预算先检查，不追加构建体系。

源码阶段结果（2026-09-14，早于下方窗口终验）：Gate A/B 方向定位与内置产物修复通过；当时 Gate C Windows 新交付截图待执行。后续终态以下方窗口验收记录为准。

- 修复前，新 Cook 五字形正向、sealed MSDF 五字形倒置，Renderer 上/下顶点分别采样 v0/v1 的定向测试通过。只更新内置 Cook 产物，不修改 Cook 算法或 Renderer 生产代码。
- 使用既有 `cargo run --manifest-path rust/Cargo.toml -p editor_core --bin build_builtin_font_pack --locked` 重建 1,357 字形；生成器编译 2m20s，生成约14分钟。新 bundle digest 为 `sha256:c78679021bbaac9371e7b281c48d0c4bacabc00fa05a77bd8b586c82c0bfd67c`。3 张 bitmap 页哈希全部保持，7 张 MSDF 页及 metadata/manifest 更新；生成日志与前后摘要见 `.artifacts/340-font-pack-rebuild.log`、`340-font-pack-before-hashes.log`、`340-font-pack-after-hashes.log`。
- 同一方向测试已红转绿，覆盖 A/g/p/y/2、16/20/24/34px fresh bitmap、sealed 16/24/32px bitmap 与 64px MSDF。新 sealed MSDF 与 fresh Cook 的归一化轮廓正向 mismatch 五项均为 0。测试同时调用真实 FontPack seal 校验与 RuntimeFontBundleLoader，日志 `.artifacts/340-orientation-after.log`；编译1m15s，测试4.45s。
- `project_font_auto_hybrid_uses_physical_px_scale_outline_and_continuity` 1/1通过，见 `.artifacts/340-hybrid-consumer.log`；复用 Renderer 1/1通过。两处测试源码 rustfmt check 通过。未重复全 workspace、Local CI 或游戏玩法回归，因为生产算法及游戏逻辑未改。
- AUI 在物理字号大于32px，或 bitmap 档位匹配比例不在0.875–1.125时使用 MSDF，解释字号相关触发。不能把 g 等字符的上下像素总数直接当方向结论，最终用完整轮廓与垂直镜像的 mismatch 判别。
- 本轮未更新安装 Provider、宿主配置或激光小屋交付，不宣称当前运行窗口已生效。下一步是包含新内置字库的 Provider/新交付及 Windows 截图验收；现有24px大写规避不能代替此项。方向缺陷不需要方案B，保持当前架构。

### 340 Gate C 真实窗口终验（2026-09-14）

Gate C 已通过，340 方向缺陷的源码与 Windows 新交付验收闭合。详见 [完成记录](../../阶段完成记录/2026-09-14-340-AUI-Font-Window-Acceptance-v1/00-总览.md)及 [身份/证据摘要](G:/gameEngin/.artifacts/340-window/evidence-summary.json)。

- 复用 341 源码 Provider 和 340 修复字库；测试副本 `.artifacts/340-window/project` 只为 AUI 字形板、捕获位置及迁移后 SDK 路径做适配，项目运行逻辑保持。未重建字体、未修改引擎生产代码、未安装 Provider 或更改宿主配置。
- 实际 Windows/Vulkan 1280×720：A/g/p/y/2 的 16/20/24/34/48/64px 共 30 个字形均正向，无缺字/裁切；普通、PAUSED、TRY AGAIN、恢复、ALL ROOMS CLEAR 五张截图通过人工式复核。大字号 A 横杠有细微边缘接缝，标题字距偏紧，作为未修质量观察保留，不影响方向结论。
- 新交付主路线 1020 tick / 18 断言 / 3 捕获及 observe 全通过，Provider 外层 technical/gameplay/visual/delivery 均 passed；真实 Player 子进程耗时 22.375s。失败/恢复复用完全相同的 Game.exe/DLL/RuntimePackage，380 tick / 6 断言 / 2 捕获通过、耗时 10.883s。CLI 补验 delivery 为 not-checked，不另算一次 Provider 交付复验。
- 新 RuntimePackage digest 为 `sha256:603ed258a2e9e53fc4015fcada720e56e2871ff97e11de9a95c12cf8b3830ca9`；字库 bundleDigest 保持 `sha256:c78679021bbaac9371e7b281c48d0c4bacabc00fa05a77bd8b586c82c0bfd67c`。有界验证正常退出，证据和测试副本保留。

### 342 音频闭合与恢复338（2026-09-14）

用户已确认回录“没有爆音和拖音”，342 Gate A/B/C均通过并归档，见[342完成记录](../../阶段完成记录/2026-09-14-342-AudioSource-Runtime-Audio-v1/00-总览.md)。真实WAV入包、AudioSource、SDK三命令、生命周期、Windows输出与暂停/重开均有证据。338恢复当前，未完成、未归档；本次没有重建、重录或重新测试已通过内容。粒子/光效和完整Scene生命周期仍需按现有能力与实际缺口推进，不自动开工新系统。
