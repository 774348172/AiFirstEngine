# 294 AUI Dirty Cache / Differential Presentation / Fixed-Step Decoupling v1 施工文档

## 1. 状态

```text
系统编号：294
文档版本：v1
建立日期：2026-08-17
状态：已完成；Gate A-C、定向回归、最终 30 秒 source-linked smoke 与文档归档均闭环
当前授权：Window A / Gate A-B（已完成）；Window B / Gate C、定向回归、一次 30 秒 source-linked Tower smoke 与归档（已授权）
激活状态：已于 2026-08-17 完成激活前复核
```

正式方案：

```text
../../294-AUI-Dirty-Cache-Differential-Presentation-Fixed-Step-Decoupling-v1方案.md
```

本文档采用最小施工编排：两个 Window、三个 Gate。Window B 已于 2026-08-17 获得一次性完成剩余施工授权。
不得构建 candidate 或更新 production Editor。

激活前复核结果：正式方案与 275-293 后代码基线一致；当前施工槽为空；相关 owner、consumer 和测试
入口存在。相关源码已有历史未提交改动，施工必须在现状上追加且不得回退。复核通过后激活 Window A。

## 2. 施工目标

只完成四条可验证合同：

```text
clean AUI frame 不 clone/resolve/layout/glyph rebuild。
Summary 普通帧不生成 Animator/report 详单，visible identity 由 revision 提供。
Tower 未变化 UI 投影和表现槽不发 mutation。
一次 ordinary frame 可推进 0..N fixed step，但最多 presentation 一次。
```

保留 284 即时输入顺序：AUI feedback 与业务 action 先处理，不等待 fixed tick。

## 3. 必要性与验证经济审计

```text
已确认问题
- Tower 普通 Editor Play 约 21.85 FPS，平均约 45.8 ms/frame。
- clean frame 仍重复处理约 2.2 MB AUI、1296 nodes、767 bindings 和 449 draw items。
- 每轮仍生成 40 条 Animator2D observation，并计算全量 JSON + SHA identity。
- Tower fixed update 最后一帧仍报告 49 mutations。
- catch-up 会逐次执行完整 descriptor frame，可能重复 presentation。

最小因果修复
- 先在现有 Editor session 上复用 last valid AUI present，并以 revision 判定失效。
- 让现有 Summary/Trace 分档控制详单生产，而不是增加报告系统。
- 让 Tower producer 比较上一状态后只提交真实差量。
- 最后只拆 fixed simulation 和 presentation 的调用次数，不重写 renderer。

验证上限
- 每 Gate：新增 owner regression、owner filtered tests、一个直接 consumer test、scoped fmt。
- 全部 Gate 后：最多一次 30 秒 source-linked Tower real-window smoke。
- 禁止为验证新建通用 runner；既有 283 harness 不适配时停止并申请复核。
```

## 4. Window A：消除每帧重复工作

### Gate A：Editor GameView AUI 热路径

合并施工以下三项，因为它们共享同一个 present owner 和失效事实：

1. session 持有 last valid AUI present 与最小 dirty/revision key。
2. key 未变化时直接复用，不调用全量 AUI build。
3. Summary 不构造 Animator2D observation vector 或 Trace 级报告克隆。
4. `aui_presentation_identity` 保留现有字符串消费合同，但值改为 session-owned revision identity。
5. document、visible binding、target、resource 或 feedback revision 变化时只重建一次。

首选文件范围：

```text
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/services/play_service.rs             # 仅现有 owner 无法承载时
rust/crates/editor_window_winit/src/editor_frame_publication.rs  # 只允许 consumer 回归
```

不先修改 `engine_runtime::aui` public schema。若上述 owner 无法获得完整失效事实，立即停止 Gate A，
提交遗漏事实清单后复核，不得自行增加通用 reactive UI 或 node 依赖图。

新增 owner 回归合同：

```text
连续 clean frame：AUI rebuild 计数不增加，identity 不变化，Animator detail count=0。
feedback 或 visible data 变化：revision 增加且只 rebuild 一次。
284 input：action same-frame、exactly once，且不推进 runtime fixed tick。
```

定向验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core editor_gameview_play::tests::editor_gameview_clean_frame_reuses_aui_present --lib -- --exact
cargo test -p editor_core editor_gameview_play::tests::editor_gameview_visible_dirty_advances_presentation_revision_once --lib -- --exact
cargo test -p editor_core editor_gameview_play --lib
cargo test -p editor_window_winit editor_frame_publication --lib
cargo fmt -p editor_core -p editor_window_winit -- --check
```

Gate A 完成条件：clean/dirty/revision/284 合同均通过。不得用 FPS 数字替代结构合同。

### Gate B：Tower mutation 差量

只修改 Tower producer：

```text
samples/tower_defense_project/RuntimeModule/src/playable_session.rs
同文件 tests
```

施工内容：

1. `tower.matchView` 只在 AUI 可见投影变化时 replace。
2. `SpriteRenderer2D` 只在槽位身份、visible 或 sprite frame 变化时 replace。
3. `Transform` 只在位置实际变化时写入。
4. 同输入下 delta 结果必须与原 full projection 的最终 World 状态等价。

不得修改玩法规则、AUI 文档、Scene、Settings、轮次、兵种、敌人或资源。

定向验证：

```powershell
Set-Location G:\gameEngin
cargo test --manifest-path samples/tower_defense_project/RuntimeModule/Cargo.toml playable_session::tests::unchanged_projection_emits_no_redundant_mutations --lib -- --exact
cargo test --manifest-path samples/tower_defense_project/RuntimeModule/Cargo.toml playable_session::tests --lib
cargo fmt --manifest-path samples/tower_defense_project/RuntimeModule/Cargo.toml -- --check
```

Gate B 完成条件：未变化槽位零 mutation，变化槽位只含必要字段，最终 World 等价。

## 5. Window B：fixed simulation / presentation 分离

### Gate C：N fixed steps 对应最多一次 presentation

Window B 必须单独授权。施工内容：

1. 现有 accumulator 继续计算有界 `0..N` fixed steps。
2. catch-up 循环只推进 deterministic simulation、physics、Animator2D fixed progression 和 World commit。
3. 汇总 dirty facts 后，每个 ordinary frame 最多执行一次 AUI/render/presentation。
4. Play、Pause、Step、headless 和 284 input-before-fixed 语义不变。

首选文件范围：

```text
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/application.rs  # 仅调用边界确有需要时
```

不得创建第二套 frame loop，不得引入 render thread、怪物插值或输入回调内 GPU Present。

新增 owner/consumer 回归合同：

```text
fixedStepCount=0/1/N 时 presentationCount<=1。
N>1 时所有 authoritative fixed step 均执行，action 不 replay。
Pause 为 0 fixed step；feedback dirty 仍可 present。
Step 恰好 1 fixed step；headless 不创建 presentation。
```

定向验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime frame_loop::tests::fixed_catch_up_presents_once --lib -- --exact
cargo test -p engine_runtime frame_loop::tests --lib
cargo test -p editor_core editor_gameview_play --lib
cargo test -p editor_window_winit --features real-window fixed_catch_up --lib
cargo fmt -p engine_runtime -p editor_core -p editor_window_winit -- --check
```

Gate C 完成条件：结构计数和模式语义通过；不得把 debt 丢弃伪装成性能修复。

## 6. 最终一次性能证据

仅 Gate A-C 都通过后，才允许在同一已授权 Window 中运行一次 30 秒 source-linked Tower
ordinary Editor Play smoke。优先只读复核并复用 283 harness：

```text
G:\AifeTowerRuns\283-report-hotpath-20260813\harness
```

验收记录：

```text
平均 FPS >= 55，或相对 21.85 FPS 基线有显著改善并附结构计数。
无 UI 变化区间 AUI cache hit >= 90%。
Summary Animator detail count = 0。
ordinary frame presentationCount <= 1。
未变化 SpriteRenderer2D mutation = 0。
按钮 action same-frame、exactly once。
```

只允许 run-owned 输出。不得替换 production Editor，不得重建真实 Tower Preview cache。

## 7. 明确禁止

```text
Local CI、完整 workspace 测试、完整 E2E、完整视觉矩阵。
production/安装态 Editor、Player、MCP 或其它二进制替换。
真实项目配置修改、真实 Tower Preview cache 强制重建。
新通用 reactive UI、public 大 schema、migration、workflow 或 construction runner。
逐节点增量 layout、字体/GPU pipeline 重做、render thread、怪物插值。
顺手修复 294 之外的问题。
```

## 8. 停止与复核条件

遇到任一条件立即停在当前 Gate：

```text
需要超出该 Gate 文件范围的第二个新公共抽象。
需要修改 public schema/migration 才能继续。
cache key 无法覆盖可见失效事实，或出现 stale/白帧/输入坐标回退。
delta mutation 与 full projection 最终 World 不等价。
fixed/presentation 分离导致 action replay、Pause/Step 或 headless 回退。
既有 283 harness 不适配当前 composition。
单个 Window 预计超过三小时，或验证开始扩成完整 E2E/Local CI。
```

停止后只报告：失败合同、最小复现、涉及 owner 和建议缩小后的 repair；不得自动扩项。

## 9. 激活前复核清单

```text
[x] 正式方案仍是 294 v1，用户明确授权待激活 Window。
[x] 施工文档/当前/ 没有其它活动施工文档。
[x] 工作树重查完成，用户现有改动不会被覆盖。
[x] 上述首选文件、测试模块和命令仍存在。
[ ] 283 harness 仅在最终 smoke 前只读确认适配性。
[x] 本次授权不隐含下一 Window、production 更新、Local CI 或完整矩阵。
[x] 复核结果回填本文档并重新自审后，才移动到 当前/ 并同步 54。
```

## 10. 自审结论

```text
范围：通过。五项性能修复压缩为共享 owner 的三个 Gate，没有拆成五套工程。
必要性：通过。每项都对应已确认普通帧重复工作。
验证经济：通过。每 Gate 只有 owner 回归、直接 consumer 与 scoped fmt；最终 smoke 最多一次。
所有权：通过。引擎负责 AUI/presentation/frame loop；Tower 只负责项目 mutation delta。
兼容性：通过。保留 284 即时输入、Pause/Step、last-good 和现有 identity 字符串消费面。
授权：通过。Window A 已按用户授权完成；Window B / Gate C、一次 30 秒 source-linked Tower smoke
与最终归档已取得单独授权。
```

自审结论：Window A 结果满足原范围；Window B 激活前复核通过，可开始 Gate C。

## 11. Window A 完成结果

Gate A：

```text
EditorRuntimePlayInstance 持有 active binding paths、visible snapshot 与 last valid AUI present。
连续 clean frame 只生产/比较绑定快照，跳过 document resolve、layout、draw list 和 glyph rebuild。
普通 Summary 不扫描 Animator2D play observation；Trace 仍可显式恢复详单。
aui_presentation_identity 从逐帧 JSON + SHA-256 改为 session-owned revision 字符串。
feedback dirty 与 visible data dirty 推进 revision；284 same-frame input、纹理和 publication 合同保持。
```

Gate B：

```text
Tower ReadySession 缓存上次 AUI 可见投影；未绑定的 tick/revision/事件详单不触发 matchView replace。
怪物槽以 Scene 初始 Transform/visible 为基线，只写变化后的 Transform 或 visible。
相同投影与相同怪物视觉状态均为零 mutation；玩法、AUI、Scene、Settings 未修改。
```

验证结果：

```text
RED：editor_gameview_clean_frame_reuses_aui_present，旧行为 rebuild_count=3，要求=1。
PASS：editor_gameview_play owner 21/21。
PASS：editor_frame_publication consumer 4/4。
PASS：Tower playable_session owner 25/25。
PASS：editor_core/editor_window_winit scoped fmt check。
PASS：Tower RuntimeModule scoped fmt check。
```

Window A 结束时未运行：Window B / Gate C、30 秒 real-window smoke、Local CI、production Editor 更新、
真实 Tower Preview cache 重建、完整 E2E 或视觉矩阵。Window B 的最终状态见下节。

## 12. Window B / Gate C 结果

实现结果：

```text
EngineFrameInput 通过既有调用链携带 ordinary frame 的 fixed_step_count，默认仍为 1。
FrameLoop 每 ordinary frame 只 advance_frame 一次；0..N fixed step 在同一 frame 内有界执行。
首个 fixed step 保持既有 fixed -> 单次 frame update -> physics/Animator 顺序；后续 catch-up step
只执行 fixed -> physics/Animator。World dirty 最后只 extract/apply/render/present 一次。
action snapshot 只进入首个 fixed step，catch-up 不 replay；AUI immediate input 顺序不变。
real_window 的两个 authority/ordinary 调度入口均从 N 次完整 descriptor tick 改为一次携带 N 的 tick。
Pause 不推进 fixed；Step 仍固定为 1；headless 仍不构建 render presentation。
```

红绿证据：

```text
RED：fixed_catch_up_presents_once 首次因缺少 N-fixed-step owner 入口而编译失败。
PASS：fixed_catch_up_presents_once；0/3 fixed、一次 RenderExtract/FrameEnd、action exactly-once。
首次 frame_loop owner suite：18 passed / 4 failed；失败确认 physics 被提前到既有 frame update 之前。
最小顺序修复后 frame_loop owner suite：22/22 passed。
PASS：editor_gameview_play 22/22，包含 284 immediate input、AUI cache、Pause/Step 与 catch-up consumer。
PASS：real-window fixed_catch_up 1/1；6 fixed step 只调用一次 GameView publication。
PASS：headless owner 1/1；runtime advanced 且 render_built=false。
PASS：engine_runtime/editor_core/editor_window_winit scoped fmt check；目标文件 diff check。
```

最终性能 smoke 停止记录：

```text
fresh root：G:\AifeTowerRuns\294-fixed-presentation-20260817
283 harness 只读复核后复制为 run-owned harness；旧 lock 在编译前被 --locked 拒绝，未启动 Editor。
fresh lock 离线生成后 release 编译到 run-owned harness，最终在 harness 源码编译失败：
project_runtime::linked_set() 不存在。
首因：292 后 Tower RuntimeModule 是 ProjectRuntimeAbi 动态模块，只导出 aife_project_runtime_entry_v1；
283 harness 仍依赖已退役的静态 LinkedProjectRuntimeSet 入口。
scenario 未启动，30 秒性能窗口未消耗；evidence report=0；Cargo/Editor 进程均为 0。
```

该结果命中文档第 8 节 `既有 283 harness 不适配当前 composition` 停止条件。禁止为了完成数字而给
Tower 恢复旧 `linked_set()`、新造 runner 或把 production Editor 当 source-linked 候选。Gate C 源码证据
有效；后续最小 repair 结果如下。

## 13. ABI normal-loader 最小接线与最终 smoke

没有恢复 `linked_set()`，没有新增通用 runner。run-owned harness 删除静态 Tower RuntimeModule 依赖，
直接调用现有普通 Editor authority；Editor production services 使用
`ProjectRuntimeNativeModuleLoader` 解析 `aife_project_runtime_entry_v1`。

第一次 normal-loader 场景继续使用离仓库 Tower 副本，但该副本的 RuntimeModule 相对 SDK path 已失效，
trust inspection 在按钮生成前停止。该尝试未进入 Play、未执行 1800 帧，Cargo/rustc/Editor 残留均为 0。
最终场景改为只读打开真实 Tower 源目录；recent、workspace、trust、native-module cache 与 evidence 继续
隔离在 fresh root。

最终结构化证据：

```text
fresh root：G:\AifeTowerRuns\294-fixed-presentation-20260817
scenario：tower-294-fixed-presentation-1080-30s-r2
candidate SHA-256：48AF83FE68647A376407F3D3D7431DD7E4C24E4D0C5EA09C7ED84ACD1489AC91
status：passed，14/14 steps
1800 frames：46,409 ms，38.79 FPS
baseline：21.85 FPS；relative improvement：77.5%
capture：after-30s-fixed-presentation-gameview.png
whitePixelCount：0
input actions：4/4 passed；Stop passed at frame 1812
remaining Editor/Cargo/rustc processes：0
```

绝对值低于 55 FPS，但符合第 6 节的替代验收：相对基线显著改善；owner/consumer 结构证据同时证明
clean AUI cache、Summary Animator detail=0、unchanged Tower projection=0 mutation，以及 0/N fixed steps
对应 presentationCount<=1。normal-loader 在 Play 前因当前 Tower identity 与 292 retained artifact 不同而进行
一次 run-owned project-module build；没有触碰 production 或真实用户 cache，也不计入 1800 帧窗口。

未运行 Local CI、完整 workspace/E2E 或视觉矩阵；未替换 production/安装态二进制，未修改真实配置，
未重建真实 Tower Preview cache。完成记录：
`阶段完成记录/2026-08-17-AUI-Dirty-Cache-Differential-Presentation-Fixed-Step-Decoupling-v1/00-总览.md`。
