# 316 持续试玩与画面细节收敛施工

状态：completed。用户已确认范围并授权施工；315归档后激活。2026-09-10约15:10—15:44 CST内完成，未触及18:10硬截止。无新外部审查；基线复核与自审通过。

必要性与验证经济性：空场由项目无补充/清理造成，最低变更为RuntimeModule、Main Scene、现有Prefab、AUI；复用SDK与五个既有规则。新增定时项目状态不是引擎协议。排除安装/config、Local CI、架构扩建和完整战斗系统。

- Gate A：持续生成、回收、移动边界/射击冷却；项目测试先红后绿，模拟60秒并覆盖清空后的恢复、存活数量上限。
- Gate B：枪口尺寸、特效淡出、背景循环、紧凑HUD；现有投影能力优先，项目测试验证状态行为，实际PNG验证效果。
- Gate C：项目测试、Compiler直接consumer、受影响fmt；宿主重新run并用现有Player脚本/截图通道检查至少60秒模拟与命中过程，打开新交互产物。
- Gate D：证据、限制、入口同步并归档。

命令：cargo test --manifest-path samples/complex_shooter_project/RuntimeModule/Cargo.toml --target-dir G:/gameEngin/rust/target --locked；rust目录cargo test -p project_authoring_execution --test game_project_compiler_interface game_project_compiler_assembler_owner_prepares_complex_shooter_runtime_input --locked。
复用315未变引擎纹理修复证据；项目变更使旧交付试玩证据失效，必须重新生成。预算45-90分钟，单一能力探查15分钟无结果则收敛最小路径；禁止以增加测试矩阵代替定位。证据根C:/a316-1510/evidence，保留日志与截图，不删除旧交付。

Gate B实际集成修正：WorldWriteApi::set_component_field不支持SpriteRenderer2D.color，导致淡出mutation失败。最小扩至world_api.rs颜色字段写入与owner回归，RGBA拒绝错误类型/非有限值，保持原visible语义；不扩展全部精灵编辑接口。先补owner红灯，再实现并复验真实Player。

Gate C连续射击3600帧暴露1260/3060帧重复销毁同一敌机（world.entity.missing）。项目侧同帧碰撞结算加入已消耗子弹与待定HP，击杀只结算一次、同帧多目标分数累加。simultaneous_hits先红灯证明同敌机销毁2次，再修复；不改引擎碰撞/销毁协议。实际HUD背板改不透明以免新入场敌机透到数字下面。

## 终态

A passed：每1.5秒最多补两架、总敌机上限12，空场立即补充；波次随实际派出次数推进。越界敌机销毁，玩家限制在战区，按住Space按0.12秒冷却连发。子弹横向清理边界同步扩大，避免玩家靠侧边开火即消失。项目60秒确定性测试在30秒清空敌机后仍恢复，存活数始终1—12。

B passed：枪口按飞机纵向尺寸+0.06定位，缩小闪光；爆炸扩张和RGBA淡出，双背景滚动拼接，HUD上下栏收紧且不透明，明确TRAINING / NO DAMAGE。原Kenney CC0资源ID与许可保留，无新收费API或生成素材。WorldWriteApi补color读写记录与有限值检查，项目使用运行时规范组件ID engine.sprite_renderer2d；原authoring名不能作为运行时字段写入ID。

C passed：最终去重23项测试：项目10、World API9、native adapter3、Compiler consumer1。项目红灯3项（补充/边界/枪口）与同帧销毁红灯1项均变绿；颜色owner红灯world.component.unsupported_field变绿。测试夹具初次FieldPath构造错误/错误组件ID已修正，不把编译失败当行为红灯。
最终编译/测试时耗：项目2.55s/0.40s，World API6.81s/0s，native adapter0.13s/0.11s，Compiler最后0.29s/5.53s（核心变更后的首次consumer重编85s）。受影响fmt及diff --check通过，仅既有换行警告。没有运行完整workspace、Local CI、性能或其它平台矩阵，原因是本轮只承诺Windows项目持续试玩与一个字段写入能力。

实际交付过程：op-7因项目未格式化被build guard拒绝，格式化后继续；首个手动CLI遗漏run-native-player被参数校验拒绝，按现有CLI修正。op-8缺颜色写入时的长跑在定位原因后主动终止本轮PID20496，保留60帧失败PNG作短复现；没有删除用户进程或旧交付。op-9实现颜色淡出后，30秒不射击0失败，60秒射击2个重复销毁失败；headless相同输入trace定位1260/3060帧，随后项目去重修复。失败截图/trace均保留，不当终态证据。

最终op-10真实Windows/Vulkan：qualified-impact 35帧显示命中扩张/淡出；qualified-cleanup 70帧trace确认爆炸第52帧销毁、0失败；qualified-60s实际3600帧完成、0失败、1939次command apply、974次Prefab实例化（包含敌机、子弹与效果，不等于敌机数量）。最终截图SCORE001800 / WAVE10 / HOSTILES08，持续出现敌机且HUD无透影。30秒不射击证据复用op-9 final-30s（波次5/可见11/0失败），对应生成移动逻辑未改；最终HUD与同帧结算以op-10重新验收。报告counters.entityCount是初始hydration口径，不能冒充最终动态实体数；上限依据项目逐帧模拟测试与最终画面，未宣称性能或泄漏资格化。

D passed：正式方案/阶段记录/README/49/54/施工与阶段索引同步，施工归档；当前/待执行为空。证据C:/a316-1510/evidence保留，无递归清理、无Provider/config/Editor安装事务；新交互Game.exe零参数打开供用户试玩。

## 身份与边界

最终交付engine-op-mcp-process-12864-10；Game.exe SHA256 1774F08DE7C64D6E9C9321BDB63C67BF347EA6481779B3B2D0E908EE9D8E176F；路径samples/complex_shooter_project/Library/EngineTools/Deliveries/engine-op-mcp-process-12864-10/Run/Windows/dev/Game.exe。
deliveryRef engine-delivery:sha256:d9bed311809d26b4158adcac81154f93e8bf44d70b950db68e7371a973f31487；revision sha256:24aac03c74a23123eeb69ff12302bdd507f4db4e3bb278e7a33252c8fa28ec63。
项目lib.rs SHA256 21F5E3FCD591C5EC12191ABBCD812B3CC602EA87984CB807817F0DA85F4F0423；continuous_tests.rs 907201D2A3F46EB60D75CAC5556FE23C354C952E673AF323C1887E5949026952；world_api.rs 44DF62EE938FEBECE8793E1BE9507CDA49B44B29E1F77FA3868C01AB64BCC2FF。
仍是训练模式：没有敌方武器/玩家受伤与失败重开、尾焰/转向动画、音频或多帧爆炸/复杂粒子；背景为单速滚动，不是视差背景。28px字体路径未修；本项目继续用24px。原dirty工作树保留，不宣称clean exact commit或整体引擎性能通过。
