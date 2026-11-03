# 284 Editor GameView Normal-Frame Immediate AUI Input Dispatch v1 施工文档

状态：Gate A-C 已完成，验证通过，待移动到已完成归档。

正式方案：`../../284-Editor-GameView-Normal-Frame-Immediate-AUI-Input-Dispatch-v1方案.md`

## 1. 授权与启动边界

用户于 2026-08-13 授权开始施工，并随后单独授权 Window B / Gate C。Gate C 激活前复核确认：284 仍是
唯一活动施工，Window A 证据有效，publication owner、real-window consumer 与 Tower linked fixture 均未漂移。

激活前复核已确认：当前无其它活动施工文档；方案和调用链仍有效；必改源码包含前序施工留下的未提交
基线，因此本施工只在当前内容上做窄增量，不回退或覆盖既有修改；计划 test seam 仍存在。

施工分为两个授权窗口：

```text
Window A / Gate A-B
  红测、EngineHost 即时 action owner、Editor GameView 普通帧输入链

Window B / Gate C
  publication identity、受影响 consumer、格式与文档闭环
```

Window A 不自动授权 Window B；当前 Window B 已取得独立授权。连续施工总时长不得超过三小时；到 2h45m
不再启动新 Gate。

## 2. 施工目标

把普通 Production Editor GameView 的输入路径从“输入同步触发完整 runtime tick”改为：

```text
ordinary input
  -> AUI hit / feedback
  -> immediate ProjectRuntimeSession AuiActionDispatch
  -> action-affected observation/binding refresh
  -> AUI composition dirty + request_redraw
  -> existing ordinary redraw / GPU composition / Present
```

并保持：

```text
combat / physics / enemy movement / Animator2D progression -> fixed tick only
unconsumed gameplay input -> next normal runtime tick exactly once
input callback -> no independent GPU Present
renderer / RenderGraph -> no redesign
```

## 3. 基线与 Owner 复核

生成本文时确认的基线：

1. `editor_window_winit/src/application.rs` 的 GameView input route 调用
   `tick_active_game_view_runtime_descriptor_frame_with_input`。
2. `editor_core/src/editor_gameview_play.rs` 在处理 AUI interaction/feedback 后调用完整
   `EngineHostLoop::tick`，连带 fixed update、physics、world extraction 和 Animator2D tick。
3. `project_runtime_session.rs` 已有 mutation prepare/commit、fault、report 和 Animator2DCommand 收集 owner；
   不需要第二套事务协议。
4. `EditorFramePublicationModule` 当前以 `frame_index + frame_hash + extent` 判断 last-good reuse，不能区分
   同一 runtime frame 中的 AUI-only 可见变化。
5. Tower `PlayableInterface` 已有真实 linked RuntimeModule 和 `td.recruit` / `td.start-round` consumer seam。

激活前必须确认这些事实仍成立。若 owner 已漂移，只修订本文中的文件/命令，不得直接扩大设计。

## 4. 必要性审计

```text
confirmed failure
- 每个 GameView pointer 输入同步触发完整 runtime/GameView tick。
- 仅 request_redraw 时，现有 last-good identity 会吞掉同 runtime frame 的 AUI-only 变化。

minimum causal fix
- 增加 EngineHostLoop 普通帧离散 AUI action stage，复用现有 action/mutation/observation owner且不推进时间。
- EditorRuntimePlayInstance 复用稳定 AUI cache，立即 interaction/feedback/action，并私有暂存未消费 gameplay input。
- publication 以最小 AUI presentation identity 区分 AUI-only 更新和真正的 unchanged redraw。

files that must change
- rust/crates/engine_runtime/src/engine_host_loop.rs
- rust/crates/editor_core/src/editor_gameview_play.rs
- rust/crates/editor_core/src/session.rs
- rust/crates/editor_window_winit/src/application.rs
- rust/crates/editor_window_winit/src/editor_frame_publication.rs

conditional narrow files
- rust/crates/engine_runtime/src/frame_loop.rs（仅用于暴露当前 time/Animator2D apply-only 的 crate-private helper）
- rust/crates/engine_runtime/src/project_runtime_session.rs（仅当现有 helper 无法直接复用）
- rust/crates/editor_window_winit/src/real_window.rs（仅当现有 fast reuse 分支需要同步 identity）
- samples/tower_defense_project/Tests/PlayableInterface/tests/playable_interface.rs（只加 consumer 断言）

tests that can fail before and pass after the fix
- immediate AUI dispatch 在 fixed/runtime/physics/render phase 前完成且 exactly once。
- AUI consumed input 不进入 gameplay；unconsumed input 下一 normal tick 消费一次。
- 同 runtime frame 的 AUI revision 重新 publication；完全相同内容继续 last-good reuse。
- Tower start/recruit action 先提交，fixed update 计数在 immediate stage 保持不变。

explicitly deferred nearby enhancements
- catch-up policy、render interpolation、完整 FrameLoop 重构。
- 输入线程、独立 GPU Present、renderer/RenderGraph 重构。
- 新 action queue、公共 event bus、AUI/RuntimePackage/项目 schema migration。
- production Editor 替换、Tower Preview cache、真实配置、Local CI、完整视觉矩阵。
```

## 5. 改动范围

### Engine-owned，授权 Window 后可修改

```text
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/engine_runtime/src/frame_loop.rs（条件式）
rust/crates/engine_runtime/src/project_runtime_session.rs（条件式）
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/editor_frame_publication.rs
rust/crates/editor_window_winit/src/real_window.rs（条件式）
上述 owner 文件中的定向测试
```

### Project-owned，仅测试 consumer 可修改

```text
samples/tower_defense_project/Tests/PlayableInterface/tests/playable_interface.rs
```

Tower RuntimeModule、AUI、Scenes、Settings、生成器和 gameplay 数据均为只读。若验证暴露项目功能缺陷，停止并
单独讨论，不得在 284 中顺手修改。

### 禁止范围

```text
production/安装态 Editor、Player、MCP 或其它二进制替换
Tower Preview cache 或真实配置修改
Local CI、完整 workspace、完整 Tower Gate F/G 或视觉矩阵
独立 input-thread / GPU Present
RuntimeRenderer、RenderGraph、AUI renderer primitive 重构
战斗、物理、怪物移动或 Animator2D 改为 variable update
新增公共 action queue、事务协议、report family 或广域 schema migration
```

## 6. Window A / Gate A：红测与最小合同

### 任务

1. 在 `engine_host_loop` 增加 owner 红测，固定新普通帧 action stage 的合同：
   - action 和 mutation exactly once；
   - `RuntimeTime.frame_count`、`fixed_frame_count` 不变；
   - 不调用 FixedUpdate、project fixed phase、physics、world extraction 或 Animator2D tick；
   - rejected/unhandled/faulted 保留现有语义；
   - action 后只按已有 observation contract 刷新必要 snapshot。
2. 在 `editor_gameview_play` 扩展现有 AUI feedback/input 测试，证明 PointerDown/Up 不再通过完整 tick；
   AUI consumed 输入被过滤，unconsumed gameplay input 不丢失。
3. 先运行这些测试并保存首个能定位旧行为的失败断言。编译错误只有在它直接来自尚未实现的新最小接口时，
   才能作为红测证据；不得用无关基线失败代替。

### 计划测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime engine_host_loop::tests::immediate_aui_action_dispatch_does_not_advance_runtime --lib -- --exact --nocapture
cargo test -p editor_core editor_gameview_play::tests::editor_gameview_immediate_aui_input_does_not_tick_runtime --lib -- --exact --nocapture
cargo test -p editor_core editor_gameview_play::tests::editor_gameview_immediate_input_preserves_unconsumed_gameplay --lib -- --exact --nocapture
```

若激活前确认 Rust test module 的完整路径不同，只修正命令中的 test filter，并在本文记录原因。

### Gate A 退出条件

- 红测能捕获“输入触发完整 tick”和“未消费 gameplay input 因移除同步 tick 而丢失”。
- 没有为制造红测而新增生产 schema、测试 runner 或 production 环境。
- 未进入实现和外部状态操作。

## 7. Window A / Gate B：普通帧即时 AUI 链

### B1. EngineHost 离散 action owner

在 `EngineHostLoop` 增加窄型 ordinary AUI action 方法，直接拥有 active project session 的串行调用：

```text
current committed runtime frame/time snapshot
  -> existing execute_project_runtime_session_stage_with_animator2d(AuiActionDispatch)
  -> existing mutation prepare/commit + terminal fault
  -> apply Animator2D discrete commands once, without Animator2D tick
  -> refresh active project observation when required
  -> return compact in-memory result
```

不得调用 `EngineHostLoop::tick`、`advance_frame`、`advance_fixed_step` 或伪装成 EditorPause host frame。
优先复用现有 crate-private helper；只有可见性或 apply-only owner 确有缺口时，才对 `frame_loop.rs` 或
`project_runtime_session.rs` 做最小提取。

### B2. EditorRuntimePlayInstance 普通帧 route

1. 保留最近一次成功且身份匹配的 AUI resolved document/layout/composition cache。
2. 普通输入 route 立即执行 hit test、capture/focus、Pressed/Released/Activated feedback。
3. Click action 立即交给 B1，action 后只刷新 active observation/binding 与 AUI composition。
4. 仅把未被 AUI 消费的标准化 gameplay input 放入 active instance 私有有界 pending state：
   - 保持离散事件顺序；
   - PointerMove 只保留符合现有输入语义所需的最新状态；
   - 下一次正常 runtime tick drain 一次；
   - Stop/session replacement/focus lost 清理；
   - 绝不存储或重放 `AuiAction`。
5. stale session/layout/target/surface identity fail closed，清理 capture 并请求正常重建，不回退完整同步 tick。

### B3. EditorSession 和 native input 接线

1. `EditorSession` 提供窄的 active GameView ordinary AUI input 委托并同步内存 report/publication 状态。
2. `try_route_production_game_view_input` 改调该入口并 `request_redraw`。
3. 输入回调不得调用 GPU submit/Present；实际合成留给现有普通 redraw。
4. 非 AUI GameView input 保持经下一 normal tick 的既有 `InputResolver` 消费。

### Gate B 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime immediate_aui_action_dispatch --lib -- --nocapture
cargo test -p engine_runtime project_runtime_session_host --lib -- --nocapture
cargo test -p editor_core editor_gameview_immediate --lib -- --nocapture
cargo test -p editor_core editor_gameview_aui_feedback_pointer_down_is_same_frame_and_filters_gameplay --lib -- --exact --nocapture
cargo test -p editor_core editor_gameview_input_tick_allows_gameplay_when_aui_misses --lib -- --exact --nocapture
cargo test -p editor_window_winit tests::native_app::native_editor_application_routes_game_view_content_input_without_ui_double_dispatch --lib -- --exact --nocapture
cargo test -p editor_window_winit tests::native_app::native_editor_application_routes_r7_narrow_portrait_game_view_input --lib -- --exact --nocapture
```

同一配置下若最后一个 suite 已包含前面的 exact test，保留红测证据后只运行覆盖面更大的 suite一次，不机械重复。

### Gate B 退出条件

- PointerDown/Up、业务 action、observation/binding 在 fixed tick 前完成。
- runtime/fixed/physics/world extraction/Animator2D progression 调用计数均未增加。
- action、Animator2D 离散命令和 pending gameplay input 都 exactly once。
- 没有 renderer 重构、公共 queue 或新 report family。

### Window A 完成记录（2026-08-13）

```text
Gate A：完成。红测先因最小接口缺失失败，随后由 Gate B 实现转绿。
Gate B：完成。EngineHost ordinary action owner、Editor immediate route、pending gameplay input、
        FocusLost/cancel cleanup 和 native production input adapter 已闭环。
有效测试：
- engine_host_loop::tests::immediate_aui_action_dispatch_does_not_advance_runtime（1/1）
- project_runtime_session_host（12/12，Gate B 实现未再变化，复用同一输入身份证据）
- editor_gameview_immediate（2/2）
- editor_gameview_play::tests::editor_gameview_cancel_discards_pending_gameplay_input（1/1）
- editor_gameview_play::tests::editor_gameview_aui_feedback_pointer_down_is_same_frame_and_filters_gameplay（1/1）
- editor_gameview_play::tests::editor_gameview_input_tick_allows_gameplay_when_aui_misses（1/1）
- tests::native_app::native_editor_application_routes_game_view_content_input_without_ui_double_dispatch（1/1）
- tests::native_app::native_editor_application_routes_r7_narrow_portrait_game_view_input（1/1）
- scoped rustfmt --check：通过
- scoped git diff --check：通过
无效过滤词：`production_game_view_input` 和缺少模块前缀的 `--exact` 命令均执行 0 tests，
不计入证据；已用上面的完整测试路径替代。
未执行：Window B / Gate C、publication identity、Tower consumer、production Editor、Local CI、真实配置、
        Tower cache、完整视觉矩阵。
```

## 8. Window B / Gate C：Publication 与受影响闭环

### C1. 最小 presentation identity

1. 先在 `editor_frame_publication` 增加红测：相同 runtime identity、不同 AUI presentation identity 必须
   重新提交；identity 完全相同仍复用 last-good。
2. 为 `GameViewRuntimeFrame` 或等价最窄内部 publication input 增加 AUI presentation content identity；
   它只在可见 AUI composition 改变时更新，不改变 runtime `frame_index/frame_hash`。
3. 优先在 `EditorFramePublicationModule` 的私有 last-good record 中同时保存 receipt 与该 identity，避免修改
   `editor_wgpu_renderer::RuntimeContentIdentity` 和 GPU receipt。
4. `real_window` 的 fast reuse 条件若绕过该 identity，收敛为同一判断或委托 publication owner；不得保留
   两套不一致的 reuse 规则。
5. AUI-only revision 使用最近 world/RHI state 生成新的 UI composition plan，并由普通 redraw 提交；输入
   callback 内不 Present，不运行 world extraction。
6. 若必须为序列化 `GameViewRuntimeFrame` 增加字段，采用向后兼容默认值并定向验证 consumer；不自动升级
   AUI、RuntimePackage、项目 action 或 report family。只有红测证明 GPU receipt 本身无法保持一致性时，
   才允许修订施工文档后触碰 `editor_wgpu_renderer`。

### C2. Tower consumer 与受影响回归

Tower 只通过现有 `PlayableInterface` linked runtime fixture 验证一个真实按钮：

```text
td.start-round 或 td.recruit action 已在 immediate stage 提交
same-stage fixed_update_count 不增加
next normal tick 才推进战斗
action 不重放
```

不得为该断言运行完整 ProductionAuthority scenario 或修改 Tower gameplay/AUI/config。

### Gate C 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit editor_frame_publication --lib -- --nocapture
cargo test -p editor_window_winit production_game_view_input --lib -- --nocapture
cargo test -p editor_core editor_gameview --lib -- --nocapture

cargo test --manifest-path G:\gameEngin\samples\tower_defense_project\Tests\PlayableInterface\Cargo.toml --test playable_interface immediate_aui_action_precedes_fixed_tick -- --exact --nocapture

cargo fmt --all -- --check
```

Gate B 已通过且 EngineHost 输入未改变时，Gate C 复用其 `project_runtime_session_host` 证据，不重复运行。
若 `editor_gameview` suite 已执行 Gate B 的 exact tests，也不再单独重跑。`cargo fmt` 只验证格式，不修改文件。

### Gate C 退出条件

- AUI-only 变化产生新 publication，runtime frame identity 不变。
- 完全相同的 world+AUI redraw 继续复用 last-good。
- Tower 真实 action consumer 证明 action-before-fixed 和 no-replay。
- affected owner/consumer 测试与格式检查通过。
- 未执行 production replacement、真实 Editor smoke、cache rebuild、Local CI 或完整矩阵。

### Gate C 完成记录（2026-08-13）

```text
红测：GameViewRuntimeFrame 缺少 aui_presentation_identity，E0560/E0609，准确捕获 owner 缺口。
实现：可见 AUI stages/glyph plan/canvas references 生成稳定 SHA-256 identity；序列化字段带默认值。
publication：私有 last-good record 同时保存 receipt 与 AUI identity；real_window 委托同一 reuse 判断。
Tower consumer：td.recruit immediate action 已提交，同阶段 fixed_update_count 不增加，下一 normal tick 不重放。

验证：
- editor_frame_publication suite：3/3 PASS
- editor_gameview suite：17/17 PASS
- native production input exact tests：2/2 PASS
- Tower immediate_aui_action_precedes_fixed_tick：1/1 PASS
- editor_window_winit --features real-wgpu-surface --lib：cargo check PASS
- cargo fmt --all -- --check：PASS
- scoped git diff --check：PASS（仅既有 LF/CRLF 提示）

复用证据：Window A EngineHost immediate owner 1/1 与 project_runtime_session_host 12/12；
Gate C 未修改其输入源码，因此不机械重跑。
未执行：production Editor 更新、真实普通 Editor smoke、Tower Preview cache、真实配置、Local CI、完整视觉矩阵。
```

## 9. 失败处理

1. 当前 Gate 失败时保留首个失败断言、stdout/stderr 和输入身份，先在 owner seam 修复，不跨 Gate 堆积。
2. 如果失败要求 renderer 重构、公共 schema migration、production binary 或真实配置，停止施工并回填正式
   方案/本文；不得以“顺便修复”扩大 284。
3. 如果现有 AUI cache 无法安全复用，先证明具体 stale identity，再补最小 cache identity；不得退回每次输入
   完整 tick。
4. 工作树已有用户修改一律保留；激活时若与必改文件发生无法合并的冲突，停止并报告，不回退用户改动。

## 10. 完成与归档

Gate A-C 全部通过后才可：

1. 生成 `阶段完成记录/2026-08-xx-Editor-GameView-Normal-Frame-Immediate-AUI-Input-Dispatch-v1/00-总览.md`。
2. 同步 `49`、`54`、施工文档 README、阶段完成记录 README 和正式方案施工状态。
3. 把本文从 `当前/` 移到 `已完成/`，文件名改为
   `284-当前可自动化施工文档-Editor-GameView-Normal-Frame-Immediate-AUI-Input-Dispatch-v1.md`。
4. 明确记录未执行的 production Editor 更新、真实普通 Editor smoke、Tower Preview cache、Local CI 和视觉矩阵；
   不得把 source-level 通过表述成 production 已更新。

## 11. 施工文档自审

```text
正式方案一致性：通过。完整实现普通帧即时 AUI，fixed simulation 保持原 owner。
施工授权：Window A / Gate A-B 与 Window B / Gate C 均已按独立授权完成。
唯一施工槽：284 全部 Gate 完成，完成记录与入口同步后释放施工槽。
因果最小性：通过。只拆输入到 action 阶段，并补防止 AUI-only 更新被吞的 publication identity。
Owner 边界：通过。Editor 不直接取得 ProjectRuntimeSession；EngineHostLoop 保持 active session owner。
项目边界：通过。Tower 只增加测试 consumer，不修改 RuntimeModule、AUI、Scenes 或 gameplay。
Fixed tick 边界：通过。战斗、物理、怪物移动、Animator2D progression 均不进入 ordinary stage。
Present 边界：通过。输入只更新状态并 request_redraw，不新增 callback GPU Present。
Schema 审计：通过。优先私有 last-good record；任何窄字段只服务必要 presentation identity。
验证比例：通过。红测 + affected owner/consumer + fmt；无 Local CI、production、完整 workspace/视觉矩阵。
重复验证：通过。上层 suite 覆盖同配置 exact test 时不重复运行。
三小时边界：通过。拆为两个授权窗口，单窗口到 2h45m 停止开启新 Gate。
```

自审结论：Gate A-C 全部闭环，验证范围与 source-level claim 一致，可以生成完成记录并归档。production
Editor、真实配置、Tower cache、Local CI 和完整视觉矩阵均未执行，也不在完成声明内。
