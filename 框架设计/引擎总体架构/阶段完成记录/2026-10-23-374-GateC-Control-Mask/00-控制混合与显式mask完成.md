# 374 Gate C：控制、混合与显式mask完成

状态：`main_source_gate_c_passed`。Gate A/B/C已关闭，Gate C已串行合入主线；256骨骼显式mask、整数Tick控制/混合/中断、once淡出、源事件与模拟挂点通过，93项去重测试通过，scoped Clippy增改文件无告警、10个Rust文件fmt通过。371阶段45成果逐字节保留。D–F未启动，374整体仍partial，未安装，游戏固定V0.0.5。

## 授权、边界与主线交接

用户明确授权“开始gateC的施工”，并要求保留371并行成果。本窗2026-10-23 12:23:28 UTC开工（北京时间20:23:28）；15:08:28停止新增、15:23:28硬截止，实际提前收尾见closeout.json。沿既有工程施工技能执行，未跨入D–F，未改安装、Provider、SDK/wire、FrameLoop、Physics、游戏版本或正式性能资格。

开工先按B已关闭的1183文件seal复核，同步独立374目录的12处基线差异并保留before-worktree-sync.zip。开发在01-Engine/worktrees/374-spine，使用374独立target；本窗后续371从阶段43/44推进阶段45，C不接管其owner。13:24:06 UTC只发布12个C自有文件，A45 Release构建/导出已经结束，serial-window-processes.json记录空构建窗口；before-main-integration.zip保留发布前态，没有停止A进程。

publication.json记录每个发布文件的前后SHA。A45的asset_memory.rs、render_asset_production/cooked.rs、runtime_asset_loader.rs逐字节保留，其余1176个非C主线文件未覆盖；未写Git索引、reset/clean/checkout/commit。原空index.lock保留。共享49/54文档逐次读取并按写前内容比较，只更新B相关块，保留A新进度。

## 实现与出口覆盖

最小修改为原Spine FFI/校验、spine_animation控制owner及其测试，共12文件；清单见publication.json。没有新增通用施工runner，证据capture/final_audit/sync_docs均为本run自有helper。

| 出口/失败合同 | 已实现与可红证据 |
| --- | --- |
| typed控制与混合唯一权威 | Definition→ControlTable→Controller，紧凑动作/层/参数索引、Bool/Trigger、逐层首匹配、默认base/空overlay、loop所有权/Resume、动作对过渡；compact_bool_trigger_rules、rules_resolve_per_layer、action_pair_mix_override |
| 整数时钟与幂等性 | 整数external/logical Tick、微秒phase、有界loop、暂停不追补、Play/Restart、speed 0..4000；integer_tick_long_clock_pause_and_idempotent_restart |
| 混合/中断/once/clear | 官方AnimationState唯一混合，当前Tick预更新时间；三次中断、clear恢复base、once speed ceil与empty fade、旧计划取消、每层4 entries且预留未来empty；crossfade_and_three_interruptions、interrupt_uses_current_tick、once_plan_reserves_empty_entry等 |
| 真实显式mask | 从全身源生成[u64;4]、256骨骼显式集合；源SHA绑定，frames/curves独立复制、canonical deform ID、Bezier/property hold；real_full_body_filter、256_bones、mask_property_hold、weighted_linked_mask正例 |
| mask依赖与能力限制 | 覆盖骨骼/单轴/Scale/Shear、slot色彩/attachment/deform/draworder、IK/Transform/Path；跨读写、父级驱动、约束target间接animated ancestor、weighted/clipping/draworder危险拒绝；对应constraint/parent_weighted/animated_ancestor负例 |
| Additive reference | 只开放Rotate/Translate；重复绝对采样不累加，离散/deform拒绝；repeated_absolute_samples、additive_translation/rotation |
| 事件语义与容量 | 全层先count再枚举，单实例每Tick≤64，稳定ID/payload/ordinal、0键、多loop、同刻排序、权重0、outgoing/clear/暂停；events_zero_multi_loop、cross_layer_budget、single_loop_callback_count等 |
| 原子拒绝/隔离/挂点 | 未知批次/容量错误不改参数/游标/提交结果，有限后端失败隔离并保留Pose；骨骼矩阵读取、最多32挂点、非均匀缩放/旋转、PPU只除一次；event_budget_and_unknown_batch、finite_backend_failure、socket_units等 |
| 所有权与旧消费者 | mask/state重复创建释放余额、旧Animator26项、原Compiler/Cook/包6项实际消费者；control_sampling_is_bounded_and_owned_clones_release与主线组合 |

本窗C完成的是typed CPU控制owner。D尚未将作者JSON编译到typed表、写Cooked controllers、接SDK/session/FrameLoop、generation或下一合法callback；B的controllers仍为空。OOM为abort边界，session总事件预算与1024实例正式规模、GPU退休、普通项目独立交付和正式性能仍按D–F施工。

## 实际验证、复用与成本

| 验证 | 去重计数 | 总调用秒 | 测试执行秒 | 原始证据 |
| --- | ---: | ---: | ---: | --- |
| 独立后端终态（A11+B20+C9） | 40 | 11.796 | 0.650 | backend-final-result/stdout/stderr |
| 新增weighted linked mask正例 | 1 | 0.922 | 0.000 | backend-weighted-final-result/stdout/stderr |
| 主线控制C20+旧Animator26 | 46 | 169.172 | 0.060 | main-control-result/stdout/stderr |
| 主线Compiler/Cook/运行包消费者 | 6 | 181.687 | 65.590 | main-project-result/stdout/stderr |
| 隔离控制开发反馈（同46项，不重复计数） | — | 29.437 | 0.060 | control-final-result/stdout/stderr |
| 后端Clippy（含最后新增测试） | check | 0.469 | — | clippy-backend-post-test-result及lint-audit |
| core Clippy | check | 8.281 | — | clippy-core-final-result及lint-audit |

93项去重全部通过，失败0、ignored0，终态sourceDrift均空。总调用秒包含构建/启动，不等同于编译或Player独立耗时；Compiler消费者65.59秒包含测试内部Cook/包操作，未拆成伪精确阶段成本。10个Rust增改文件按后端edition2024/core edition2021定向rustfmt通过；native C++已随后端真实构建，未宣称ASan/全平台或新增静态分析资格。lint-audit保留原依赖/旧core告警，12个C增改文件告警0，无新增suppression。

后端40项执行后只插入最后weighted正例；identity-audit证明移除该完整新case可恢复原gate_c.rs SHA，所有产品输入未变，所以旧40项与新1项组成终态41，未机械重跑旧suite。主线46包含371 A45上下文，隔离46不叠加计数。

16套B真实GPU像素按未变renderer/shader、纹理Cook、fixture与legacy几何路径复用；s374_geometry函数整个尾段SHA为`4d2dc5738699e7f89470fca6139d0a2971c1a9c709f3c5b65c853c07cdc96755`。这只保留B几何渲染证据，不宣称C已完成GPU/正式规模性能；D/E/F实际consumer变化时刷新受影响部分。159官方源/许可及UPSTREAM清单、Cargo锁、原fixtures保持。

## 首错、修复与保留证据

1. additive-red.log：相同绝对phase重复求值，base未写某属性时Additive从4累加到7.9999995。修复为受控采样先setToSetupPose，再由官方一次apply完整层栈；后端终态包含对应绿例。
2. interruption-red.log：跨Tick中断读取上一mixTime，结果5、官方参考43。修复prepare_sample仅更新时间，setAnimation前使已有entries处于当前Tick，最终只apply一次；interruption-green.log及主线终态均通过。
3. 早期Clippy误带入无关platform_3d_foundation integration target，隔离目录缺其Input/default.json；这是命令范围错误，保留clippy01.jsonl，不改无关样本，收窄到core --lib与nested backend --tests后通过。mask嵌套if样式告警已修复；未弱化产品断言。

## 身份、实际命令与收尾

1188文件dirty主线源seal：`c4f9a57717bf54119fa553c95e2597af04246699b8c9e3f70729b4b2b1884124`；逐文件SHA、12个C文件与A45上下文分别见source-identities.json，seal算法为UTF-8紧凑JSON数组（sort_keys、ensure_ascii=False）的SHA256。不绑定新Git commit、安装二进制或外部配置。证据根：`D:/GameEngine/EngineMigration/EngineMigration/01-Engine/main/.artifacts/374/gate-c-20261023-122328`。

每次在同一PowerShell进程加载07-Tools/Enter-EngineEnvironment.ps1 -Lane 374；脚本会进入独立目录，主线组合再显式使用main/rust真实cwd。target为`D:/GameEngine/EngineMigration/EngineMigration/05-Build/imported/I/gameengin-build/374/cargo-target`，Rust1.96.0、MSVC14.44.35207，dev/test配置、-j1。SPINE374_EVIDENCE_ROOT分别指向本窗自有证据子目录。实际命令及完整cwd/环境身份在各*-inputs.json：

```text
# nested backend cwd: rust/crates/engine_spine_runtime
cargo test --offline --locked -j1 --tests -- --test-threads=1
cargo test --offline --locked -j1 --test gate_c weighted_linked_mask -- --test-threads=1
cargo clippy --offline --locked -j1 --tests --message-format=json
# main/rust cwd
cargo test --offline --locked -j1 -p engine_runtime --features spine --lib -- spine_animation animator2d spine374_b_ --test-threads=1
cargo test --offline --locked -j1 -p project_authoring_execution --features spine --test spine374_project -- spine374_b_ --test-threads=1
# worktree/rust cwd
cargo clippy --offline --locked -j1 -p engine_runtime_core --features spine --lib --message-format=json
```

本窗源码与组合验证完成，正式文档九处当前入口同步；本阶段记录完成，374总施工因D–F未完成继续保留当前。自有验证进程已结束；证据/测试生成输入与共享374 target有意保留，没有新的仓库外临时目录或安装配置需要回滚。实际结束时间、总耗时、进程快照及文档CAS结果见closeout.json/doc-sync-audit.json。未执行全workspace、默认无Spine重复回归、真实Player独立交付、正式Release配方或Android：本窗未改其装配/资格，相关出口留在D–F。
