# 342 AudioSource 实体组件与最小 Runtime 音频播放 v1 施工文档

依据：[342 正式方案 B](../../342-AudioSource实体组件与最小Runtime音频播放-v1方案.md)。2026-09-14 用户授权生成施工文档、自审后直接施工。

状态：2026-09-14 Gate A/B/C 全部完成，`source_and_windows_dev_delivery_passed`，已归档。用户确认实际回录“没有爆音和拖音”，听感缺口闭合；复用已有源码、交付与输出证据，本次只做文档收尾。338 已恢复为唯一当前施工，仍未完成、未归档。详见[完成记录](../../阶段完成记录/2026-09-14-342-AudioSource-Runtime-Audio-v1/00-总览.md)。

## 必要性与验证经济性自审

- 请求与缺口：激光小屋需要真实跳跃/开关/死亡/通关音效；当前只有 Audio 资产分类，没有真实音源装配、解码、Source 与输出。
- 最小因果变化：既有 Compiler/资源加载增加 WAV；通用组件容器增加 AudioSource；SDK/session 增加三个命令；Runtime Audio 与 Windows 输出消费同一交付包。播放实例随实体释放。
- 排除：方案列出的 BGM、3D、混音台、流式/压缩矩阵、编辑器、Android、新工具/独立 DLL、安装宿主更新、完整 workspace/Local CI。339/340/341 不重做。
- 复用：341 SDK/session 提交通道和真实项目工具脚本；340 字库；337 固定 Host/DLL 构建布局。源码/API变化只使直接消费者与新版运行组合证据失效，不重跑旧计时。
- 验证方式：先 owner 可失败测试，再实际受影响组合编译/交付；音频设备输出必须另外验收，headless/计数/离线 WAV 不能冒充可听输出。
- 成本预算：Gate A 45–80 分钟；B 20–35 分钟；C 15–30 分钟及收尾 10 分钟。累计目标不超过 155 分钟，连续授权起点约北京时间 17:55，20:40 后不开始新实现或验证，20:55 前完成证据保留与受控进程退出。实际成本偏离预算先复核首因与最小范围。

## 文件责任与并行边界

| 责任 | 涉及文件/目录 |
|---|---|
| 音源/组件与实例资产 | `engine_runtime/src/components.rs`、`archetype.rs`、`world.rs`、`runtime_entity_hydration.rs`、`runtime_instance_loader.rs`、`runtime_asset_loader.rs`、`runtime_package_builder.rs`；`project_authoring_execution/src/neutral_assembler.rs`、`game_project_compiler/check.rs`；新增小型 Audio PCM/Source 合同文件 |
| 项目命令 | `project_game_sdk`、`project_runtime_sdk`、`engine_runtime/src/project_runtime_native_adapter*`、`project_runtime_session.rs`，相关合同摘要与直接消费者 |
| 播放 owner/真实组合 | `engine_runtime/src/runtime_audio.rs`、`engine_host_loop.rs`；`runtime_player_winit/src/lib.rs` 及内部 Windows 输出适配；必要 Cargo 依赖/features/lock |
| 示例与证据 | `samples/laser_house_project` 的四个 WAV/asset、四个 Source 实体、项目事件和验收；`.artifacts/342-audio/` 保留运行证据 |

并行角色只编辑各自文件；Cargo/build 由主代理统一调度，避免 target 锁竞争。共享 lib/Cargo/合同位置先约定再改，不覆盖现有用户改动。

## Gate A：最小合同与可执行 owner

1. `engine.audio_source` 纯数据为 clipRef/volume，沿通用 components 进入 Scene/Prefab 和共同 hydration；加入实例音源引用收集。
2. PCM16 单/双声道 44.1/48 kHz WAV 校验/解码，真实音源字节来自 SourceView 并进入 RuntimePackage。共享 clip 解码资源，缺失/错误/截断字节明确失败。
3. SDK `audio_source(entity).play/stop/set_paused` → typed intent → session Applied commit；准备阶段无副作用，新命令捕获 runtime identity/generation。
4. Runtime Audio 管每 Source 一个播放实例，重播从头、暂停标志保持、不同 Source 可重叠，消费有序且不重复；对象退役自动释放。
5. 用成熟库接 Windows 输出，库类型不进入项目 SDK；headless 使用内部测试适配。现有 Host 控制更新覆盖零 Fixed Tick 清理和暂停/停止。

验证命令按实际测试模块过滤执行（在仓库根）：

```powershell
cargo test --manifest-path rust/Cargo.toml -p engine_runtime audio --locked
cargo test --manifest-path rust/Cargo.toml -p project_authoring_execution audio --locked
cargo test --manifest-path rust/Cargo.toml -p project_game_sdk audio --locked
cargo test --manifest-path rust/Cargo.toml -p project_runtime_sdk audio --locked
```

失败合同：资产不是真实 PCM、引用/增益无效、重复/丢失播放、NoOp 仍播放、暂停/停止不保持、销毁后残音、排队命令跨世代。定向测试需能捕获对应失败；测试数和首错/复验日志落盘。若实际过滤未覆盖某个影响面，运行其已有直接消费者测试，不能用零测试通过代替。

## Gate B：集成与项目接入

1. 检查 Scene 与 Prefab 的数据、依赖、卸载行为，不扩建 Scene 框架。验证 Source 禁用/移除/销毁、同名实体重建和零 Fixed Tick 控制更新。
2. 激光小屋四 Source 配置普通音源，项目事件边沿提交 play；暂停/恢复和重开显式控制自己持有的 Source。项目音色参考 Godot，对应生成器仅作项目素材工具。
3. 更新 SDK 合同摘要，覆盖实际序列化/native adapter/Engine DLL 与 Player 组合；现有无音频项目保持有效。准备时不播放，不吞真实设备失败。

```powershell
cargo test --manifest-path rust/Cargo.toml -p runtime_player_winit audio --locked
cargo check --manifest-path rust/Cargo.toml -p runtime_player_winit --features real-window --locked
cargo test --manifest-path samples/laser_house_project/RuntimeModule/Cargo.toml --locked
```

直接 consumer 与定向格式检查按实际修改补充；不机械重跑 Gate A 已涵盖测试。项目断言区分实际提交/播放控制与可听输出，不用项目预期计数伪装引擎声音成功。

## Gate C：真实 Windows 音频与收尾

1. 使用既有 Provider 构建、playtest/observe 和同交付 CLI 路径。新版 Provider 仅独立进程使用，不安装、不改宿主配置。保留 Game.exe/DLL/RuntimePackage/音源摘要与工具原始报告。
2. 主路线触发跳跃、开关、通关，失败路线触发死亡；复用同一包。原玩法/Animator2D断言保持，窗口确认视觉没有退化。
3. 记录实际 Windows 音频设备输出，优先回录与事件时间对应，复核暂停/重开清理。设备回录只取本次有界验收区间、保存到本轮证据目录；不启用麦克风。不把离线合成、驱动提交或 playing 计数冒充回录。若实际输出/听感未证实，明确保留 Gate C 未完成。
4. 复用有效证据，不为文档收尾重复构建。受控进程正常退出，证据和有效产物保留，不清理共享缓存/用户文件。
5. 全部声明成立后写阶段完成记录，更新方案状态、49/54/索引/项目能力记录；本文归档到已完成，恢复 338 为唯一当前施工。否则记录已通过与剩余缺口，不能虚报 342/338 完成。

## 文档自审

- 342 B 方向已获用户确认，本次没有修改正式方案的架构选择。
- 每项新增都有对应已确认缺口，三命令/两字段/单 Source 单声部保持；采用内部库与适配，不新增项目层工作流。
- Headless owner 测试和真实输出分别承担不同声明，按影响面验证，不要求 clean commit/全量发布资格。
- 338 仅为执行槽暂存，不归档；本次用户“自审后直接施工”已授权必要的串行切换，不另行询问重复许可。
- 激活前必须确认唯一当前槽、源码可用、测试工具链与磁盘；复核通过后再修改生产代码。

激活复核结果：方案与代码落点一致；Cargo 工具链可用，G 盘可用约 149 GB，无已有 cargo/rustc 构建竞争；已安装 Provider 保持不动。独立文档复审没有必须修改项。已完成 338 暂存与本文件移动，允许执行 Gate A。

## Gate A/B 执行记录（2026-09-14）

- `342-audio-owner-regression.log`：Engine 音频 owner 20、Compiler 2、两个 SDK 各 1，通过。真实 WAV/Scene/Prefab 引用与释放、命令提交/世代、暂停/停止/重播、零 Fixed Tick Host 清理均覆盖。
- `342-sdk-consumer-regression.log`：两个 SDK 全部 19 项通过；`342-session-consumers.log` / `342-frame-consumers.log` / `342-host-consumers.log` 分别 54 / 23 / 28 项通过，过滤交集不重复计为独立测试。
- `342-project-tests.log`：项目全部 10 项通过，既有玩法与 Animator2D 保留；`342-windows-audio-tests.log`：真实 Windows feature 下 3 项通过，包含设备失败进入 Off/Summary 失败摘要与无音频项目不准备设备。
- 首错与修复：内部 tagged unit variant 曾接受未知字段，改为空结构变体保持 JSON 合同并通过严格字段回归；测试夹具缺少 BTreeSet 导入已修复。复审发现音频描述混入 sourceImage 可覆盖 payload，check/assembler 统一拒绝；`342-compiler-audio-final.log` 4 项通过。保留早期失败日志，不改变验收断言。
- 新源码 Provider 用 `cargo build -p engine_tool_provider --bin ai_engine_tool_provider_mcp --locked` 完成（此前把 bin 名误作 package 名的调用失败已保留）。真实 Windows check 通过；最终实际 feature 组合由上述测试与 Gate C 构建共同覆盖。
- 主/死亡路线和 120 帧暂停重开补验复用一个交付；回录只取默认扬声器 loopback，设备探针已确认可用。当前不以这些准备结果宣称 Gate C 已通过。
- 复审说明：目前生产 Host 与 loader 共同结束，输出只在首次更新前安装。将来若提供已播放 Host 的输出热替换并保留外部 loader，需要另行明确资源释放责任；本轮不新增该热替换 API 或施工项。

## Gate C 实际执行与剩余项（2026-09-14）

### 已通过的工程验收

- 源码 Provider 实际执行既有 build → playtest → observe。首个交付为 `engine-op-342-audio-20260914-102953-2/Build/Windows/dev`；主路线 1020 tick / 18 断言 / 8 捕获 / observe 通过，technical/gameplay/visual/delivery 均 passed。原始工具记录在 [342 证据目录](G:/gameEngin/.artifacts/342-audio/evidence)。
- 同交付原样 `failure-checkpoint.scenario.json` 通过 380 tick / 6 断言 / 2 捕获，退出 0。它的 CLI delivery 为 not-checked，交付资格来自前项 Provider，不另算一次交付复验。通关截图实际复核：HUD、机器人、机关与通关提示正常，未观察到本次音频接入导致的视觉退化。
- 四个 Source WAV 与入包 WAV SHA-256 全部一致，见 [packaged-wav-identity.json](G:/gameEngin/.artifacts/342-audio/evidence/packaged-wav-identity.json)。Windows 默认扬声器 `SPEAKER-Z` 的真实 loopback 回录为 48kHz 双声道，未启用麦克风。主路线回录 28.9s、失败路线回录 13.7s，均有实际非零 PCM。
- 对独立源 WAV 的前 150ms 重采样模板做归一化波形相关：跳跃 0.9802、开关 0.9663、死亡 0.9883、通关 0.9719；相应峰值增益约 0.375–0.394，符合 Source 音量 0.4 经重采样的信号。完整原录音、模板方法及相关结果均保留，不把这些数值说成主观音色评价。模板可能匹配其它音效的相近谐波，不把每个局部峰都当成一次业务事件。
- 首轮普通 CLI Trace 报告漏转发 Native audio 字段，实际声音并未丢失。已在现有 `WindowedPlayerRunReport` 增加可省略的 `audioSummary`，由 `runtime_cli::native_window_report` 转发；[定向回归](G:/gameEngin/.artifacts/342-cli-audio-report-regression.log) 1 项通过，覆盖 trace、无音频旧报告兼容、错误与退出码。
- 报告修复后的最终交付为 `engine-op-342-audio-20260914-104004-2/Build/Windows/dev`，现有固定 Game/Host 与 Engine DLL 消费端均重建。120 帧真实窗口补验退出 0：第45帧 play，第47帧 pause，第91帧 resume，第95帧 stop 后清除暂停；playCount=1、stopCount=4、结束时 playingCount=pausedCount=0，音频诊断为空。
- 最终补验的真实回录中只有两段声音，暂停静音间隔约 0.73s；play→resume 的命令间隔与回录起点间隔均为 0.780s；恢复→stop 命令为 0.068s、回录尾段为 0.080s，在 10ms 测量窗和 35ms 设备缓冲容差内，后续超过0.5s无残音。见 [控制验收](G:/gameEngin/.artifacts/342-audio/evidence/20260914-104100-controls/control-verification.json)。

### 证据复用与身份

- 首交付 Game SHA-256 `E3A620BEF70C944B2C780C7F3A5D08571EC31CB400B7C1C63FD6507E27DBCE82`，Engine DLL `54B1BD9584B4F4F8806ECEC67358C261803EC929C0662EEAD72DD9B1D029715C`。
- 最终 Game SHA-256 `B2939BDC462A3FB2D1680BE32E0C89C8C9F2E8AFB8C178AB4C5BDA51FE587466`，Engine DLL `D37C1A0A8F20A2E5AEC3E38F557477DE605A93D2B5F730AB899E699443052E1C`；项目 DLL 在两次交付均为 `5FB5B973DB06B4460C11B10E8D4F65A98B65FED62814E0D41757F4F78DC30C5B`。
- RuntimePackage 共68文件，只有 `reports/build-runtime-package-report.json` 的输出目录/阶段耗时不同；其余67个运行内容与元数据文件逐字节一致，四个音源和项目 DLL 保持。最终代码变化仅转发可选音频报告，播放 owner/项目/音源实现未变，因此复用四事件主/失败输出证据，仅续验受影响的普通 CLI 报告消费链。
- 最终源码 Provider SHA-256 `343E184C471948C5DC2AB7395F27533A0D06B4D8E9129F962B978CFCB29D9D51`；29个受影响源码/清单快照及产物身份见 [identities.json](G:/gameEngin/.artifacts/342-audio/evidence/identities.json)。这是当前 dirty worktree 集成证据，不是 clean commit 发布资格。最终定向 rustfmt check 通过。
- 未安装 Provider、未改当前宿主配置、未动已有 Provider 进程。独立 Provider、Player 和回录进程均按所有权退出，回录/交付/诊断脚本保留为本轮证据，没有删除共享缓存或用户目录。未运行全 workspace/Local CI、Android/release、性能测试，原因是本轮仅新增 Windows 短音频能力及其直接消费者。
- 源码施工轮约17:55–18:53（含文档自审、实现、构建、等待、回录和收尾，约58分钟），未达到三小时上限。[退出检查](G:/gameEngin/.artifacts/342-audio/evidence/cleanup.json)确认本轮受控进程剩余0；当时关键文档的14个本地链接全部可解析。当轮曾等待听感反馈，随后用户已确认通过；本次归档没有重启验证进程。

### 听感确认与归档

已经提供 [实际回录短摘录](G:/gameEngin/.artifacts/342-audio/evidence/listening-excerpt.wav)，对应裁切位置见同名 JSON。依次为跳跃、开关、死亡、通关（随后项目重开截断）、暂停后恢复的跳音；只裁切真实输出并插入分隔静音，没有用离线音源替换。

2026-09-14 用户在本线程明确回复“确认没有爆音和拖音”。本次听感证据来自用户对上述实际回录的确认，助手不声称亲耳听取；此前 `source_and_device_output_passed_listening_pending` 已结束，Gate C 通过。复用身份未变的源码、交付、波形与控制证据，不重跑构建、测试或录音；完成342归档、49/54与索引同步，恢复338。Provider未安装、宿主配置未改的边界保持。
