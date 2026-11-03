# 365 Gate D：组合补间与 Runtime 生命周期执行记录

日期：2026-10-08。状态：`d_animation_lifecycle_source_integration_complete`。仅关闭 D 源码与 Windows 开发集成，365 A–D 完成、E 未开始；默认安装仍 20261007-364，塔防 Release 未重建。

## 1. 实际实现与终态自审

公开 SDK 增加 AnimationPlay/AnimationHandle、UI/Game clock、play/pause/resume/cancel/game-clock 请求和 typed 结束结果。UiWriteBuffer 可在基础 setter 前后播放动画，操作沿原 callback 事务接受；不强制中途 flush，新播放的 CommitFinal stamps 在全部基础写入接受后捕获。失败/NoOp 回滚播放、控制请求和基础写入，已返回的回滚句柄不会再次有效。Generated DLL 实际使用 Game SDK，未手写第二套项目 ABI。

AuiSession 持有最多 4096 播放、总 4096 轨道和有界结束事件，复用 A 的描述校验/PresentationClock、C 的逐属性 journal 与最终几何。实例只存活动轨道覆盖，不另建 layout、paint 或整树动画缓存。满额拒绝新增播放，仍允许暂停、取消和控制时钟，取消消费已预留的事件容量。session/slot/generation 防回滚、槽复用、动态节点删除和重开后的旧句柄访问。

支持六种 typed 表现属性、串并行、delay、有限/无限循环、reverse/yoyo、缓动、Release/Hold/CommitFinal。活动重叠默认 Reject，延迟重叠到激活时报 Conflict；显式 Replace 取消旧实例。Binding/Controller 保留基础 owner/value，读取接口读 base，几何/绘制才消费覆盖；取消及 Release 恢复最新 base。CommitFinal 对全目标检查预期逐属性 owner/revision，仅可写回仍为 Controller 的属性；Binding 或中途变更返回 CommitConflict。Hold 完成只通知一次，后续 scope 释放不再次通知完成。

每 frame index 推进一次，重复/倒序不推进，delta 最大 250000 微秒。UI 和项目控制的 Game clock 独立，pause/resume 不追补；Active scope 由 owner 和所有目标有效 Active 共同暂停，Instance scope 可继续运行，Hidden 不等于 dispose。目标删除、代际变化、项目 shutdown 清理所属播放与覆盖。结束结果通过既有 ui_event/callback ID 排队，在后续安全会话阶段派发，不在 paint 调用项目代码，不跨 DLL 传闭包。

Player 三条生产帧路径接通：输入读前次发布动画几何 → DragStart 取消冲突 Translation → 时钟推进/必要增量重建 → 原控件反馈 → 发布共享最终几何。语义/headless 用固定微秒 delta，真实窗口用两帧呈现开始间隔，与反馈共享。帧参数合并为一个轻量上下文，消除新增参数数量告警，没有添加 allow。普通 Applied FixedTick 不重复求值全部动画；没有动画变化及 retained journal 时复用当前呈现。

UI_HOST_SCHEMA 更新为 ui-host.v6，SDK 指纹同步；UI_UPDATE7、扩展 v2、ABI1.6 不变，不增加 C 函数表槽。状态型 query 在执行前检查 4096 字节回复容量、不重试，动画请求最大 1MiB，普通读取仍 16KiB，描述仍 256 叶/32 深。旧项目须使用新 SDK/Generated Glue 重建。

自审结论：公开请求、实际 DLL、会话事务/ownership、生命周期和生产缓存/输入消费者闭合，可关闭 D。尚未取得新动画的 OS 窗口视觉、普通项目、Release 性能或独立交付资格，这些保留在 E；不凭操作计数宣称 AUI P95 或跨引擎等价。

## 2. 最终命令与去重结果

cwd `G:/gameEngin/rust`，Rust 1.96.0，Windows dev/test，既有 Cargo target。跨 crate Cargo 串行执行。日志位于本目录的 [Gate-D](Gate-D/) 下。

| 实际命令 | 最终结果与成本 |
| --- | --- |
| `cargo test -p engine_runtime --lib --test aui_retained_instance --test aui_retained_layout --locked` | runtime-final.log：775+25+15 通过，执行 7.05/0.09/0.09 秒；编译 3m02s |
| `cargo test -p project_runtime_sdk -p project_game_sdk -p runtime_player_winit --lib --locked` | sdk-game-player-final.log：55+21+81 通过，执行 0.06/0.00/0.96 秒；编译 1m54s |
| `cargo test -p project_authoring_execution --lib generated_runtime_glue --locked` | glue-final.log：13 通过，执行 14.91 秒含真实子 DLL 构建，外层编译 2m43s |
| `cargo clippy -p project_runtime_sdk --all-targets --locked -- -D warnings` | sdk-clippy.log：严格通过，6.84 秒 |
| `cargo clippy -p engine_runtime --lib --features real-wgpu --locked --message-format=json` | runtime-clippy-final.log：通过，21.79 秒；Runtime125、engine_input2 项既有告警，Runtime 的 code/file/message 与 C 基线完全一致 |
| `cargo check -p editor_core -p runtime_player_winit --features runtime_player_winit/real-window --locked` | consumer-check.log：通过，1m34s；真实窗口及 Editor 编译消费者，不冒充窗口运行 |
| 15 个本轮 Rust 文件的 `rustfmt --check --edition 2021 --config skip_children=true` | format-final.log：退出 0；未格式化无关 workspace 文件 |

去重 985 项：775+25+15+55+21+81+13。新增 Runtime 22 项及 SDK 2 项被最终 suite 覆盖；同配置 owner filters、首次 12 项 Glue 通过、早期 SDK53 和 Player check 不重复计数。Runtime 包括调度 15、会话 4、生产缓存 2、native query 1 项新增用例。

真实 DLL 完成、取消和 NoOp 三组情形验证半程 Translation、pause/resume、Hold、恢复 base、基础 Opacity/Tint 在播放前后写入，以及结束事件在后续 Applied 回调才派发。旧 panic mode103 由 ABI 捕获，是预期分支，未把输出中的 panic 当本次通过的例外。坐标夹具沿 C 使用同 ID、固定矩形 Panel 发布几何，实际 session 仍保留 Text/View 类型和文字更新断言，不声称夹具验证 glyph。

Glue 最终通过后，仅整理了 AuiPresentCache 的帧参数及 Player 调用形式，未修改其实际消费的 SDK/会话/native adapter/调度语义或 fixture。该上下文不进入 DLL 夹具调用，保留 Glue 有效证据，相关生产 helper 由之后 Runtime/Player suite 与 real-window check 验证。没有为参数整理再次重建相同 DLL。

C 的图形/仿射/裁剪真实 Vulkan 结果保持历史资格；本块未修改 Renderer、shader、路径剖分或 GPU 上传 owner，不机械重复旧 GPU 矩阵。新增动画窗口像素、指定 Release 负载及塔防三关属于 E，不能从 C 旧 GPU 或 363 旧性能换算资格。没有运行全 workspace、安装、Local CI 或塔防 Release。

## 3. 失败首因、修复与局部更新证据

- `yoyo-red.log` 捕获转向峰值丢失覆盖、闪回 base；反向区间切换从 `>` 修为 `>=`，峰值及 reverse/yoyo 有限循环最终通过。
- `ownership-wire-red.log` 捕获仅 ownership 的 batch 被 HostUiAccess::stage 空批判断跳过；空批判断纳入 ownership_updates。真实 owner 释放能够进入原事务，CommitFinal 不再读取错误基础 owner。
- `glue-first.log` 的动画 mode200 Faulted：HostUiAccess 错误地直接解码 UiQueryValue，真实 host 回复为 Result envelope。按 `Result<UiQueryValue, UiError>` 解码并两层传播错误；typed SDK 成功/失败回复回归及真实 DLL 最终 13 项通过。
- `animation-production-first.log` 保存新增夹具字段 `.node_id` 的编译首因，正确字段是 `.hit_node`。修后命中 root，`production-hit-diagnosis.log` 确认 C2 Button 无 slot 且 intrinsic width=0；夹具补 Fixed30×30 OutOfFlow slot，保持命中目标断言，没有降低负载或放宽产品行为。
- Binding 场景首次夹具误用构造器顺序、UiValue 名称及缺少 UiAccess import，编译首因保留在 binding-base-first.log。随后错误地要求 resolved_document 包含覆盖，实际 prepared document 保留 base，覆盖属于最终 geometry；改为断言实际 modulation 和独立 base 读取。生产测试证明动画 0.5 覆盖期间 base 0.8→0.3，取消后实际呈现 0.3，无需修改正确的 Binding owner。
- 满额复核发现原 request 入口对所有控制操作检查事件预留预算，会阻止已有播放的取消。容量检查收敛到新增 Play 分支；4096 实际播放用例证明第4097个拒绝、pause/cancel/game-clock 仍可成功且只生成一个结束事件。夹具 Result 解包编译修复保留 capacity-control.log，最终绿色及全 lib 覆盖。
- Runtime 首次 Clippy 为126项，唯一新增告警是动画帧 helper 参数过多；合并帧上下文后回到原125项，未增加 allow 或修改无关历史告警。

生产千节点场景：一个动画 Button +1000 个 clean 兄弟，半程只更新1几何、prepared binding 重建0、clean-999 draw Arc 复用。公共 local_to_screen、实际 hit 和反馈发布共享同一几何；基础 Opacity setter 接受后即时坐标不会重复加 Translation。重复 frame 不推进，清洁调用复用 layout Arc。结束事件在 advance 后不重入，后续 session 阶段单次派发。短时操作计数不等于 Release 性能或完全零分配。

## 4. 来源身份

HEAD 基准 `acaffa8cb95bbb54d093bbf45835a59315c82da5`，工作区保留大量既有修改。下表为实际 owner/consumer 与内联测试 harness 的最终字节，不是 clean commit、整仓 seal 或安装二进制资格。

| 文件 | SHA256 |
| --- | --- |
| rust/Cargo.lock | C870DECC47180F4B73128028E30D29B20DB9D74D69F24C557194B908D8218C07 |
| rust/crates/project_runtime_sdk/src/aui_animation.rs | 609F49FC988AAD89E48BE6884170464079C9C616E19FCD5ECB28AAA72528501A |
| rust/crates/project_runtime_sdk/src/aui.rs | A0192CB2AF3C8B17A51B1CD917AF7E131F8EDF3657FD4A4EF3F7EE95D327ED89 |
| rust/crates/project_runtime_sdk/src/aui_host.rs | 362BAB769DC588F4BD7E9473DD67E96DBACD567A7662D2A44AC37FF5EB788994 |
| rust/crates/project_runtime_sdk/src/aui_wire.rs | C45CB7CCE9A8D4FDF4402823B0A04503C3F31D24C112EB4424BD0C2954821B65 |
| rust/crates/engine_runtime/src/aui_animation.rs | 2ECEAB10AF2C23BAF77383419F2D89957BF6ADA426F1D3E924B2C85FB40A42B0 |
| rust/crates/engine_runtime/src/aui_instance.rs | 9DB001A819F9439610A24B1E43FD78EE51B0C0E8C87754EDC0595F8571E69477 |
| rust/crates/engine_runtime/src/aui_session.rs | 2B4DE130D3BE389E1C3823A5BA0ED18FBF75B02BE3DEB23523F16D72EA6E6A4B |
| rust/crates/engine_runtime/src/aui_present_cache.rs | 8F12C54FEFBA0ABC374CF791E76B2BC52DF576CB48AA6D757DCFBFA01B00C39E |
| rust/crates/engine_runtime/src/engine_host_loop.rs | DC461397D101F52D3174AE7AEDE4111A656902129E50DE7687A9BAD5452F9B95 |
| rust/crates/engine_runtime/src/project_runtime_session.rs | 688CB47C5692F77B7AC06AFE7A3D9326702A12685EB69E380CD74027777760EB |
| rust/crates/engine_runtime/src/project_runtime_native_adapter.rs | 3DB14AA723765691F9780FC7F786E7EFE46A9F4BB3A3B4D4EDFF1343812903C1 |
| rust/crates/engine_runtime/src/project_runtime_native_adapter/ui_tests.rs | 6FFD12135AF7FDC90934271E3B0A3A09696EC98C3E8792F5B2E5AB019828BF56 |
| rust/crates/engine_runtime/src/lib.rs | E3600B9A0E61161D575ACFDD6E3E78C0C8D8AB9B8E17393BD070282BC2911D32 |
| rust/crates/runtime_player_winit/src/lib.rs | 53F4F6372482D5D3A25F9CF306A5C32675273A2DCFFE300EBE28CB49DAC17BFC |
| rust/crates/project_authoring_execution/src/generated_runtime_glue.rs | F1B727ED6462C0954B2FDF59DCB8CFFF4EF9C07A4BB64EF86A594CD7C6E4D25D |

## 5. 成本、外部状态与下一块

本连续授权从06:00:58 UTC /14:00:58 CST开始，最终验证在约06:57 UTC结束，证据/入口同步另计少量时间，约一小时，未触及三小时上限。最大最终编译3m02s与真实 DLL 子构建单列，不当作 frame 开销。

没有更换默认配置或安装，没有启动用户游戏/删除项目/清理共享 target。Generated DLL临时目录由既有 TestDirectoryGuard 在成功和失败路径回收；终态再次检查本轮 fixture 前缀与 Cargo/fixture 子进程，结果见同目录 cleanup-final.log。日志保留首因及修复证据。

下一整块 E：普通 UI 展示项目和塔防视觉切片的公开接口消费、真实窗口/汉字 glyph/输入、固定新增 Release 负载与原三关、独立交付和20轮重开。总施工保持当前，不将整个365归档；源代码可用和已安装可用分开报告。
