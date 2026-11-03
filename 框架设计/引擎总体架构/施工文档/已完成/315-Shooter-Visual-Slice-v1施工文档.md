# 315 视觉样例施工

completed；用户“可以按照你的下一步开始施工”；2026-09-10 14:30—15:00 CST内完成，约30分钟。未触及17:15/17:30时间边界。
314归档，当前唯一计划，待执行无计划。无新外部审查，已自审通过。

必要性：当前8x8纹理被放大且没有命中反馈，不能作为游戏视觉样例；变更范围为complex_shooter_project资源/场景/AUI/Prefab/RuntimeModule及其直接项目测试。
不新增通用渲染架构、工具、安装/config变化、Tower/Android/Local CI。保持原资源ID，依赖检查后按正式Compiler生成新交付。
素材来源选择已异步询问；默认采用许可明确的现成素材，不使用未经授权API。保留来源和许可，不删除旧素材。
预算40-100分钟；独立资源探查超过15分钟无成果则调整来源。证据C:/a315-1430/evidence，项目原图保留。

Gate A：收敛素材/布局，记录资产影响与许可；以实际尺寸、透明度和资源引用检查验收，不写仅镜像美术参数的测试。
Gate B：项目发射闪光、命中/爆炸和短生命周期缩放清理，先项目测试捕获新行为再实现；cargo test --manifest-path samples/complex_shooter_project/RuntimeModule/Cargo.toml --target-dir G:/gameEngin/rust/target --locked。
Gate C：项目/Compiler直接consumer验证、fmt、真实宿主重新prepare/run；实际窗口截图检查美术、文字、发射、命中和效果清理；打开新交互窗口。
Gate D：证据/局限/来源/身份、入口同步并归档。复用314未变Runtime证据，不重复587项，更改project源码/素材使旧交付截图失效。

资产影响核对：保留tex-player-ship/tex-enemy-scout/tex-bullet/tex-starfield ID，分别供玩家与HUD、敌机Prefab、子弹Prefab、背景Scene消费。新增tex-muzzle/tex-explosion及muzzle_flash Prefab；复用原explosion_effect ID并补真实Sprite。Kenney Space Shooter Redux CC0，来源/原许可与原PNG保存在Assets/Images/KenneySpace。背景原256x256图按原分辨率平铺为1536x1024；旧8x8图不删。新增资源不会只留在外部缓存。
HUD新增装饰背板允许与文字叠放；Compiler consumer仍检查所有节点界内且不吞输入，非Panel控件互不重叠，避免把正常背板误判为HUD布局退化。直接consumer夹具随4个Prefab、7个纹理更新，不扩大测试矩阵。
Gate B红灯1通过3失败，分别缺枪口闪光、命中反馈、生命周期扩张；实现后4通过。此变更只在项目RuntimeModule内，不改引擎。

14:44 Gate C实际画面拒绝通过：原图背景(约40,44,52)显示成约(110,115,122)，动态Prefab贴图显示纯白块；源码确认Player纹理收集仅active Scene/Animator，无Prefab，WGPU上传固定Rgba8Unorm而忽略cooked sRGB。范围按失败证据最小扩至runtime_player_winit现有收集/上传和engine_runtime::wgpu_backend现有上传owner、对应定向测试；不变工具/schema，不触及Editor安装。字体28px在当前默认字体路径有翻转，项目改用此前已验证的24px，字体大字号路径另记未资格化，不扩展为完整字体系统修复。

## Gate终态与证据

A passed：Kenney Space Shooter Redux原包SHA256 D44A371A8FA1EB5D61A4FED3677954590AA015A78D2AFB5F97924CFE02E3F700，CC0原许可和来源保留。玩家99x75、敌机93x84、激光9x54、反馈48x46、平铺背景1536x1024；透明精灵未丢alpha。原8x8素材保留，正式ID未改变。
B passed：项目4项测试（新反馈红灯1通过3失败，修复后4通过）；最终编译0.99秒/测试0秒。命中生成可见反馈，枪口闪光与子弹同位置，短生命周期效果扩张并销毁。
C passed：纹理收集红灯证明遗漏Prefab-only texture（编译34.74秒/测试0.01秒）；GPU红灯实际创建Texture格式Rgba8Unorm而期望Rgba8UnormSrgb（编译61秒/测试0.69秒）。Player现有收集补包内Prefab引用，不把app icon等所有资源无差别上传；cooked上传按明确format选GPU格式，旧线性RGBA/字体上传保持原接口与行为。
最终去重测试49项：项目4 + runtime_player_winit lib41 + WGPU cooked格式1 + WGPU textured sprite/UI alpha2 + Compiler项目consumer1。编译成本与测试分开：Player12.35秒/0.06秒，GPU格式8.28秒/0.43秒，GPU consumer0.19秒/0.46秒，最终Compiler82秒/5.21秒。Compiler首次72秒/5.29秒通过，后因最终HUD字号及Player源码改变更新一次证据，不重复计数。
命令：项目`cargo test --manifest-path samples/complex_shooter_project/RuntimeModule/Cargo.toml --target-dir G:/gameEngin/rust/target --locked`；rust目录`cargo test -p runtime_player_winit --lib --locked`；`cargo test -p engine_runtime --features real-wgpu --lib real_wgpu_cooked_texture_preserves_srgb_format --locked`；同配置filter `real_wgpu_textured`；`cargo test -p project_authoring_execution --test game_project_compiler_interface game_project_compiler_assembler_owner_prepares_complex_shooter_runtime_input --locked`。受影响fmt和diff检查通过。

真实宿主第一次engine-op-mcp-process-12864-5虽然exit0但视觉失败，不能作为通过证据；第二次engine-op-mcp-process-12864-6含源码修复，实际Windows/Vulkan8/30/60帧都exit0。8帧蓝色枪口闪光/激光正确，30帧红色命中爆炸正确，60帧效果清理且一名敌人/分数100。trace的Prefab实例化3次、command apply7次、failedRecordCount0；7帧发射，13帧枪口清理，23帧命中生成爆炸/敌机与子弹销毁，46帧爆炸清理。复用314输入脚本而非重新编写Runner。
背景采样(20,200)：失败PNG RGB113/117/124；最终PNG42/45/51，与原素材42/45/51一致。最终字形正常，无白色fallback矩形。visual-review.md记录实际图像核验，未以退出0/有PNG/初始化计数代替视觉验收。

D passed：入口与阶段记录同步、施工归档；证据C:/a315-1430/evidence保留red/green/final日志、两次host结果、6张实际PNG/trace、输入脚本和来源背景平铺脚本。C:/a315-1430/materials保留原许可素材包作为来源证据，不执行递归清理；旧交付/窗口不删除。本轮没有安装/config事务，也未调用收费图片API。

## 身份与局限

新Game.exe SHA256 06F5DAB53E1CB5CE0CBBF217314CD4EA119B0D3767BEB611972C711DD76128EC；交付samples/complex_shooter_project/Library/EngineTools/Deliveries/engine-op-mcp-process-12864-6/Run/Windows/dev。
零参数交互窗口已打开，PID5580，AI First Engine Player，Responding=true；保留给用户。旧交付窗口未强行关闭。
deliveryRef engine-delivery:sha256:bbb76bccfef835469cc837eb216fb15d12ce5d1d792cbaa0911c4efd76c10628；projectRevision sha256:ae14e493126ee3cca48e87a7e318c5f053b25902282f78952512aa1809e2af1c。原安装Provider继续负责构建，新Player包含实际修复，不能称已升级Editor/Provider安装。
wgpu_backend源码SHA256 0C3E07E0A6A2EB75F21143B3604B1FFF45F187F6D5B4A9B6845B7A45E977ED18；Player源码4AF47ACA90F564F9CDDBCF61D79873A5799501AF6AA072CBF972C6861611C9AC；项目RuntimeModule 5A23E9D332898E1566E7A140134E5B0B6A468FCE5F45B3FE51345AFB609D30FD；config保持8AC885527781474BF08D50F4E7A232927B848B49804ED2A9CE3ACBF3566D89B6。原dirty/untracked变更保留，不称clean exact commit发布资格。
尚未资格化：28px较大字号字体路径、完整关卡与循环生成敌人、音频、复杂粒子动画、全平台/性能/production Editor呈现。当前项目用24px正确显示分数，反馈为短生命周期精灵扩张，不声称完整粒子系统。315目标是可试玩视觉样例，非完整游戏或成熟引擎画质对标结论。
