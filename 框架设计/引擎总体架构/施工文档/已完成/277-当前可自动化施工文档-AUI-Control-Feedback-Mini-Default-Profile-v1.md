# 277 当前可自动化施工文档：AUI Control Feedback Mini / Default Profile v1

> 状态：已完成；Window A-C / Gate A-F 于 2026-08-09 全部通过，待归档移动。
> 正式方案：`../../277-AUI-Control-Feedback-Mini-Default-Profile-v1方案.md`。
> 方案选择：B，零配置默认反馈 + 紧凑 Profile 覆写。
> 生成日期：2026-08-08。

## 1. 当前授权与执行边界

用户已于 2026-08-09 明确授权激活 277 Window A / Gate A-B。激活前复核已通过，本文已移动到
`施工文档/当前/`。本轮请求只执行激活与复核，尚未开始 Rust 源码修改或 Gate 测试。

当前停止状态：

```text
document location: 施工文档/当前/
active named construction document: 1 (277)
pending named construction document: 0
authorized window: Window A / Gate A-B
implementation start in this turn: no
production / installed mutation authorization: none
```

Window A / Gate A-B 已成为唯一可执行施工范围；Window B-C 仍未授权。本轮到激活闭环即停止，下一次继续时
可直接按 Gate A 的 red-capable owner tests 开始，但仍不得跨入 Window B。

## 2. 施工目标

施工通用引擎 AUI Button 交互反馈真实链路：

```text
RawInputEvent mouse/touch/focus/cancel
  -> RuntimeInputEvent normalized pointer metadata
  -> AuiInteractionSystem hit/capture/hover/pressed/focus
  -> AuiControlInteractionSnapshot + AuiCommand/AuiAction
  -> AuiControlFeedbackModule.advance(explicit presentation delta)
  -> AuiVisualOverrideSet
  -> existing AuiDrawList Rect / Image / Text
  -> UiProjection / RuntimeRenderer UI pass
  -> Editor GameView / windowed Player present
```

完成后必须具备：

1. 所有普通 `AuiNodeKind::Button` 零配置获得 Hover、Pressed、Activated 与 Disabled 反馈；
2. `aui-document.v1` 可确定性迁移到 v2，旧项目未声明 feedback 时使用内建 `button.default.v1`；
3. 项目可声明紧凑 Profile，并通过 `Auto | None | Profile(id)` 选择；
4. Mouse、Touch、Pen 或至少 hover capability 进入规范化输入，不靠事件频率猜设备；
5. PointerDown 的 Pressed override 在本帧 present 生效；
6. Click/Submit 仍由既有 Interaction/Action Mapper exactly-once 产生，Feedback 不派发业务命令；
7. PointerUp outside、cancel、capture lost、window focus lost、hidden/disabled/modal/screen/session replacement
   均不会遗留 stuck pressed；
8. visual transform 只影响最终 draw，不改变 layout/hit/navigation/clip；
9. Editor GameView 与 runtime_player_winit 使用同一 evaluator、Profile 与时间合同；
10. Off/Summary/Trace 诊断可解释 resolved profile、transition、reconcile 与 fallback。

## 3. 非目标与禁止范围

本文不施工：

- Tower 专用按钮 ID、招募/出怪/兵种语义或逐按钮项目 Rust 动画；
- Toggle、Slider、InputField、ScrollView 的自动交互动画；
- 多 pointer 同时按压多个 AUI 控件；
- 状态图片切换、Nine-Slice skin swap、shader/material property 动画；
- 关键帧、Timeline、循环、动画事件、任意属性轨道或完整 AUI Transition 系统；
- Animator2D Clip/Controller、SpriteRenderer2D 或 gameplay fixed-tick 复用；
- 音效、震动、粒子、平台 haptics；
- loading、cooldown、selected、checked 等项目长生命周期状态；
- Button 专用 DrawCommand、第二套 UiProjection、GPU pipeline 或 texture upload；
- Local CI、真实/production Editor、production/安装态二进制替换、真实配置修改；
- Tower Gate、Tower 项目资产修改、完整竖屏视觉矩阵或发布构建。

如施工需要上述任一能力，停止当前 Gate，先修订 277 正式方案和本文，不得“顺手补齐”。

## 4. 当前代码与工作树基线

文档生成时：

```text
repository HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
active named construction document: 0
pending named construction document before generation: 0
AUI feedback implementation: absent
AUI document runtime schema: aui-document.v1
```

工作树高度 dirty，且以下核心文件已有前序修改：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/lib.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/editor_core/src/aui_document_cooker.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/project_e2e_gate/*
```

HEAD 不是当前产品实现的完整基线。本文不得 reset、checkout、revert 或覆盖用户/前序施工变化。激活前必须对
每个拟修改文件执行 scoped diff 复核；发生实质重叠时，应基于当前工作树增量施工，无法安全合并则停止并报告。

当前确认的实现基线：

- `RuntimeInputEvent` 只有 PointerDown/Move/Up/Held，未携带 pointer device，也没有 PointerCancel/Leave；
- `InputDeviceKind` 只有 Keyboard/Mouse/Gamepad/Window，winit Player 尚未处理 `WindowEvent::Touch`；
- `AuiInteractionState` 有私有 `pressed_node` 与 focus/modal 等状态，没有持久 hovered snapshot；
- PointerMove 命中时产生瞬时 Hover command；PointerUp 同节点产生 Click；
- effectively hidden reconciliation 已能清部分 interaction state；
- `AuiLayoutEngine::layout_with_interaction_state` 只消费 scroll/canvas visibility；
- `AuiRuntimePresentPipeline` 当前先解析 layout/draw，Player/GameView 再处理输入；
- AUI schema 是 `aui-document.v1`，`AuiStyle` 无 feedback 字段；
- `control_style_deferred=true`；
- 275 已提供 Image textured batch 与 alpha present，273 已提供 GameView target/input coordinate 一致性。

## 5. Ownership 与不可变接口

### 5.1 唯一 owner

- `engine_input`：Raw/Runtime pointer device、primary pointer phase、cancel/leave/focus-lost 的规范化合同；
- `AuiInteractionSystem`：Hit Test、capture、hover/pressed/focus、Click/Submit、consumption 和只读 snapshot；
- `AuiControlFeedbackModule`：Profile resolve、visual state、transition/pulse memory、reconcile 与 override；
- `AuiDocumentCooker`：v1 -> v2 migration、Profile 验证、canonicalization 与 cook diagnostics；
- `AuiDrawList` extraction：将 visual override 应用到 Button 可视子树；
- Player/GameView session：显式 presentation delta 和 feedback state lifecycle；
- RuntimeRenderer：只消费既有 Rect/Image/Text composition，不读取 interaction/Profile。

### 5.2 必须保持的接口真相

```text
AuiCommand/AuiAction remains business activation truth
AuiComputedRect remains layout/hit/navigation truth
ProjectUiStateSnapshot remains project data truth
RuntimePackage remains cooked AUI truth
UiProjection/AuiOverlayFrame remains renderer input truth
```

Feedback 不得成为第二套 Hit Test、Action Mapper、layout engine、Animator 或 Renderer。

## 6. 拟修改文件与职责

激活前允许按实际代码组织微调文件名，但不得改变 ownership：

```text
rust/crates/engine_input/src/input_mapping.rs
  RuntimePointerDeviceKind、PointerCancel/Leave 或等价规范化事件、serde/backward contract

rust/crates/engine_input/src/device_state.rs
  Raw touch/cancel/focus-lost 到 primary normalized pointer；secondary touch v1 明确忽略/诊断

rust/crates/engine_runtime/src/aui.rs
  AUI v2 schema、interaction snapshot/reconcile、draw extraction seam 与 report 接线

rust/crates/engine_runtime/src/aui_control_feedback.rs (new, preferred)
  深 AuiControlFeedbackModule、Profile resolve、transition、visual override、diagnostics

rust/crates/engine_runtime/src/lib.rs
  仅导出稳定 typed interface，不泄漏内部 transition memory

rust/crates/engine_runtime/src/runtime_package.rs
  normalized aui-document.v2 load/validation，拒绝未迁移或非法 cooked contract

rust/crates/editor_core/src/aui_document_cooker.rs
  v1 migration、v2 canonical output、Profile validation/fallback report

rust/crates/runtime_player_winit/src/lib.rs
  mouse/touch/focus/cancel bridge、per-session feedback state、同帧 interaction->feedback->draw present

rust/crates/editor_core/src/editor_gameview_play.rs
  GameView 同一状态机/时间/lifecycle，Stop/session replacement 清理

rust/crates/project_e2e_gate/src/* (only if needed)
  generic/second-project consumer witness；不得修改 Tower source fixture
```

禁止在 `runtime_renderer`、WGPU backend 或 Tower RuntimeModule 增加 feedback 业务判断。

## 7. 施工窗口与三小时边界

本文按当前 dirty baseline 和跨 input/AUI/session 的影响面，拆为三个独立授权窗口：

```text
Window A: Gate A-B
  input/schema contract + interaction snapshot/cancel

Window B: Gate C-D
  deterministic feedback owner + draw subtree override

Window C: Gate E-F
  Player/GameView integration + affected closure/docs
```

每个连续施工授权最多三小时，包括实现、测试、等待、证据和清理。保守估计不允许一次授权跨越任何两个窗口。
每个 Window 完成后必须停止并报告；下一 Window 需要用户再次明确授权。到 2 小时 45 分不再启动新实现或验证，
到 3 小时按 `stopped_by_time_limit` 停止。窗口之间不允许并行修改共享 AUI/input/session 状态。

## 8. Gate A：Input Capability、AUI v2 Schema 与 Migration

### 8.1 先写 red-capable owner tests

至少增加并先观察失败：

1. mouse pointer 规范化为 hover-capable device；
2. touch start/move/end/cancel 规范化为 primary non-hover pointer；
3. v1 多点触摸只允许一个 primary owner，secondary 不产生 Button Press/Click；
4. focus lost/cancel 能产生可供 AUI reconciliation 消费的明确终止信号；
5. `aui-document.v1` 无 feedback 字段迁移为 v2 + Auto；
6. v2 `Auto / None / Profile(id)` round-trip 与 canonical order 稳定；
7. profile duplicate/missing/非法 scale、opacity、duration、easing 有结构化诊断；
8. cooker 输出 normalized v2，RuntimePackage 拒绝 source v1 直灌或非法 v2。

### 8.2 实现合同

- 增加稳定 `RuntimePointerDeviceKind::{Mouse, Touch, Pen}` 或等价 typed capability；
- Mouse 默认为 hover-capable，Touch 默认为 non-hover；Pen capability 必须显式，不能按名称猜；
- 增加 PointerCancel/Leave，或在同一 typed phase 中表达 Cancel/Leave；禁止用 PointerUp 冒充 cancel；
- Raw input 保存 touch identity，v1 只选定一个 primary touch；secondary 事件可忽略但必须 deterministic；
- 现有 pointer event serde/fixtures 必须有明确迁移或默认，不允许 silent field loss；
- AUI schema 升级为 `aui-document.v2`，v1 只在 cooker migration 边界接受；
- Document feedback registry 和 Node feedback selector 使用 deny/validate unknown fields 的现有项目风格；
- Profile 字段限制为 scale/translation/brightness或tint/opacity/duration/有限 easing；
- 内建 `button.default.v1` 是 Rust 版本化常量，不序列化进每个项目；
- migration/report 记录 source schema、normalized schema、profile/fallback identity。

### 8.3 开发验证

优先使用新测试的稳定 prefix；激活时若实际命名不同，先修订本文再运行：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_input runtime_pointer_ -- --nocapture
cargo test -p editor_core aui_document_feedback_ -- --nocapture
cargo test -p engine_runtime aui_feedback_schema_ -- --nocapture
cargo test -p engine_runtime runtime_package_rejects_invalid_aui_feedback_ -- --nocapture
```

Gate A 只证明 input/schema/cook 合同，不证明交互状态、动画或真实 present。

## 9. Gate B：Interaction Snapshot、Capture 与防卡态语义

### 9.1 Red-capable 状态表

至少覆盖：

```text
mouse move inside -> hovered
mouse down inside -> pressed + activation eligible
drag outside -> pressed_inside false，visual eligibility off
drag back inside -> pressed_inside true
up inside same owner -> exactly-one Click
up outside -> no Click + pressed cleared
pointer cancel / capture lost / focus lost -> pressed cleared
touch down/up inside -> Click，no hover residue
touch cancel -> no Click，no hover residue
hidden/disabled/removed/modal/screen/session change -> reconciled
keyboard/gamepad Submit -> exactly-one existing Submit/Action
drag source path -> no duplicate click or feedback activation
```

### 9.2 实现合同

- `AuiInteractionSystem` 继续唯一执行 Hit Test 与 Click/Submit；
- 增加持久 hovered/pressed-inside/device capability，但不公开可写字段；
- 输出稳定 `AuiControlInteractionSnapshot`；
- PointerUp 无论命中何处都必须结束本次 primary press；只有 inside same owner 才生成 Click；
- PointerCancel、focus lost、capture lost 必须终止 press/drag/scroll capture 中相应 owner；
- touch release 后不得保留 Hover；
- effectively hidden reconciliation 扩展到 disabled、modal scope、screen/session identity；
- Feedback 未来只能观察 snapshot/commands，不能回调 Interaction 改写 Action；
- existing input consumption 与 gameplay filtered frame 结果必须保持。

### 9.3 开发验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime aui_control_interaction_ -- --nocapture
cargo test -p engine_runtime aui_interaction_ -- --nocapture
cargo test -p engine_input aui_input_filter_ -- --nocapture
```

后一个广义 prefix 只在新增定向测试通过后运行，用于捕获既有 modal/drag/scroll/focus 回归；不得用完整
workspace 当修复循环。

Window A 到此结束。Gate A-B 通过只证明 input/schema/cook/interaction owner，不证明 feedback evaluator、
DrawList、Player 或 Editor GameView 已经显示反馈。

## 10. Gate C：AuiControlFeedbackModule Deterministic Evaluator

### 10.1 深模块

优先新增独立 `aui_control_feedback.rs`，公共主入口保持：

```rust
AuiControlFeedbackModule::advance(
    state: &mut AuiControlFeedbackState,
    input: AuiControlFeedbackFrameInput<'_>,
) -> AuiControlFeedbackFrame
```

模块内部负责：

- Button Auto/Profile/None resolve 与内建 fallback；
- `Disabled > Pressed > Hovered > Normal` 基础优先级；
- FocusVisible 与 Activated 正交层；
- from/current/target 值 retarget，不堆积 pulse；
- 同帧 Down+Up+Click 至少保留一个 present 周期；
- hidden/disabled/removed/modal/screen/session reconciliation；
- delta clamp、time reversal、防 NaN/overflow 与状态回收；
- Off/Summary/Trace 分档诊断。

### 10.2 时间与默认值

- 只接受显式 `presentation_delta_us`，禁止 `Instant::now`、SystemTime 或 render FPS 参与求值；
- gameplay Pause 不暂停 AUI feedback；
- `motionScalePermille=0` 禁用动态插值/弹性缩放，但保留即时颜色/透明度；
- 默认 Profile 以正式方案第 10 节为真相：Hover 70ms、Pressed 45ms、Activated 120ms、Cancel 80ms；
- easing 仅允许有限 enum，不解析任意曲线；
- 高 delta 有界收敛并诊断，不产生 overshoot/NaN；
- runtime state 只保存 active/transition 节点，不为所有 Document node 建实例。

### 10.3 Owner tests

```text
zero-config Button resolves button.default.v1
feedback:none produces no override/state
project Profile partial values normalize deterministically
hover/press/release/cancel interpolation at 0/mid/end delta
same-frame click retains visible Activated
repeated click retargets without unbounded pulse count
motionScale=0 keeps immediate contrast feedback
time reversal and large delta report deterministic diagnostics
node remove/disable/session replace reclaims state
same input/delta sequence yields byte-equivalent snapshot/report
```

开发命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime aui_control_feedback_ -- --nocapture
```

Gate C 完成后继续同一 Window B 的 Gate D；不得提前进入 Player/GameView consumer。

## 11. Gate D：Draw Subtree Visual Override

### 11.1 Red-capable draw contract

用一个 Button 根 + Rect/Image/Text/嵌套装饰子树 fixture 证明：

1. Button 根和可视 descendants 围绕同一 computed rect 中心缩放/位移；
2. authored style 后再应用 brightness/tint/opacity multiplier；
3. painter order、texture handle、clip owner 与 draw item kind 不变；
4. `AuiComputedRect`、hit test、navigation 和 Action 结果 byte-equivalent；
5. feedback:none 与非 Button interactable node 不被自动装饰；
6. Disabled 不产生 hover/press override；
7. focus overlay 如进入 v1，只使用既有 Rect command 且不改变布局。

### 11.2 实现合同

- `AuiVisualOverrideSet` 以稳定 owner node id 索引；
- subtree ownership 通过 Document parent/children 关系解析，不猜 `:label` 名称；
- 变换只发生在 draw extraction，不能回写 node rect 或 layout result；
- clip rect 保持 layout 真相；超出 clip 的 transformed geometry 仍按既有 clip 裁剪；
- Image 继续使用 275 的 cooked texture handle/batch，Text 继续使用现有 glyph draw；
- 不新增 `DrawButton`、`AnimatedButton`、feedback texture 或 GPU pipeline；
- `control_style_deferred` 在能力真实接入后改为 false/明确状态，不得只改报告过 Gate。

### 11.3 开发验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime aui_feedback_draw_ -- --nocapture
cargo test -p engine_runtime aui_render_ -- --nocapture
```

第二个 prefix 只捕获既有 Rect/Image/Text/clip/batching 回归，不重复 Gate C evaluator tests。

Window B 到此结束。Gate C-D 通过只证明 feedback evaluator 与 draw extraction，不证明真实 Player/GameView
session 顺序和 lifecycle 已闭环。

## 12. Gate E：Player 与 Editor GameView 同帧 Present

### 12.1 帧顺序

Player 与 GameView 都必须收敛为：

```text
resolved document + stable layout
  -> current RuntimeInputFrame
  -> AuiInteractionSystem
  -> filtered gameplay input + AuiCommand/AuiAction
  -> AuiControlFeedbackModule.advance(delta)
  -> draw extraction with override
  -> projection/present
```

不得通过“沿用上一帧 interaction、下一帧才绘制”规避同帧按下要求。若当前 pipeline API 阻止该顺序，应在
AUI runtime present owner 内调整 seam，不得在 Player 和 Editor 各复制一套反馈算法。

### 12.2 Session lifecycle

- Player window 和 Editor Play instance 各持有独立 feedback state；
- Stop、session replacement、document identity change、viewport target revision change 清空旧 state；
- GameView 与 Player 使用同一 presentation delta 量纲/钳制规则；
- Window focus lost、CursorLeft、Touch Cancel 进入同一 typed reconciliation；
- Editor 非 Play chrome 不消费 Project Runtime AUI feedback；
- Action dispatch/Project Rule 时机不因 draw reorder 重复或延迟；
- 现有 273 coordinate mapping 与 275 image present 不回归。

### 12.3 Consumer tests

```powershell
Set-Location G:\gameEngin\rust
cargo test -p runtime_player_winit aui_feedback_ -- --nocapture
cargo test -p runtime_player_winit aui_interaction_filters_ -- --nocapture
cargo test -p editor_core editor_gameview_aui_feedback_ -- --nocapture
cargo test -p editor_core editor_gameview_input_tick_ -- --nocapture
```

至少有一个 Player 和一个 GameView test 断言 PointerDown 当帧 draw override 已生效，同时现有 Click action
exactly once、gameplay filter 和 portrait coordinate mapping 保持。

## 13. Gate F：Affected Integration、Generic Consumer 与文档闭环

### 13.1 最小 consumer witness

使用 repository-owned generic fixture 或 run-owned 临时项目，至少证明：

```text
aui-document.v1 fixture -> cooker migration -> RuntimePackage v2
zero-config Button -> built-in default feedback
custom Profile Button -> class override
feedback:none Button -> no animation
mouse press/click -> visual + exactly-one action
touch press/cancel -> visual recovery + no false hover/click
Editor GameView and Player -> same resolved profile/state/override for same input sequence
```

至少一个非 Tower fixture 必须使用与 Tower 不同的 document/button/action/profile ID，证明引擎没有项目硬编码。
Tower 本轮不修改；若未来需要人工确认水墨手感，必须另行授权项目 consumer visual smoke。

### 13.2 代码冻结后的受影响 closure

274 v2 原则要求选择最小充分 closure。若最终修改范围与本文一致，建议一次运行：

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --all --check
cargo test -p engine_input
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo test -p editor_core aui_document_ -- --nocapture
cargo test -p editor_core editor_gameview_ -- --nocapture
cargo test -p project_e2e_gate aui_feedback_ -- --nocapture
```

说明：

- Gate A-E 的定向 prefix 是开发反馈；若完整 crate suite 在相同配置下已覆盖，不在冻结后机械重跑同一 filter；
- `editor_core` 当前全量规模较大，而 277 只改 cooker/GameView consumer，默认使用两个受影响 prefix；若激活后
  实际改变公共 session/package/assembler contract，再升级到 `cargo test -p editor_core`；
- `project_e2e_gate` 只在新增独立 consumer witness 时运行对应 prefix；若没有该修改，不为标签而运行；
- `editor_window_winit` 只在真实修改其 touch/input bridge 时加入对应受影响 prefix；
- 不运行 workspace default/all-features、EngineStrict、Local CI 或 release matrix，因为本文声明的是 source-level
  affected Integration，不声明 exact-commit release qualification。

### 13.3 文档闭环

Gate F 完成后必须：

1. 更新 277 正式方案施工状态，不扩大 deferred 能力；
2. 更新 `00-文档地图.md`、`00-阅读顺序.md`、`49`、`54`、施工文档 README；
3. 生成 `阶段完成记录/2026-08-08-AUI-Control-Feedback-Mini-Default-Profile-v1/00-总览.md`；
4. 记录 source identity、实际变更文件、测试/失败/重跑、未运行项与外部状态；
5. 将本文从 `当前/` 移到 `已完成/`；
6. 保持 Tower consumer、Local CI、production/安装态和真实配置未激活。

## 14. Validation Plan v2 映射

本文采用 274 v2 owner/consumer closure，而不是固定测试菜谱：

| Gate | Claim | Owner | Direct evidence | Consumer evidence | Cost |
|---|---|---|---|---|---|
| A | input/schema/migration 可用 | engine_input + cooker/runtime package | 新 prefix | package load contract | low/medium |
| B | interaction/cancel exactly-once | AuiInteractionSystem | 状态表 | gameplay filter regression | low |
| C | feedback evaluator 确定 | AuiControlFeedbackModule | delta/profile/state tests | none | low |
| D | draw override 不改 layout/hit | AUI draw extraction | draw fixture | existing render prefix | low/medium |
| E | Player/GameView 同帧一致 | session consumers | targeted consumer tests | coordinate/action regression | medium |
| F | source affected claim 闭合 | changed crates | frozen affected closure | generic second consumer | medium |

Evidence invalidation 规则：

- 只改 test fixture，不自动使未消费该 fixture 的 owner evidence 失效；
- 修复 evaluator 后重跑 Gate C 与消费其输出的 D/E，不重跑未受影响的 schema migration；
- 修复 Player adapter 不重跑 Editor GameView owner，除非共享 module/input contract 改变；
- 修改公共 RuntimeInputEvent/AUI schema 后，相关 engine_input、cooker、runtime package 和所有实际 consumer evidence
  均需更新；
- 未改变 production binary/config identity 时，不产生 production qualification claim。

## 15. 失败、诊断与重跑规则

1. 每个 Gate 先保留首个失败断言、stdout/stderr、输入序列、schema/profile 和 report；
2. 先建立最小 owner reproduction，再改代码；不使用 Local CI 或完整 workspace 当修复循环；
3. 分类为 design、construction-document、implementation、environment 或 pre-existing baseline；
4. design/doc 不足时先修订 277 方案/本文；
5. 修复后只重跑 reproduction 与被该修改失效的 consumer evidence；
6. 上游失败已使 claim 不成立时 fail-fast，不跨 Gate 堆积失败；
7. 非变异测试解决首因后可重跑；真实/production 尝试未授权，不存在自动 retry；
8. dirty-worktree 测试只作开发/受影响证据，不声称 clean exact-commit release qualification。

## 16. External State 与禁止操作

Gate A-F 默认只使用 workspace source、Cargo target 与测试框架拥有的临时 fixture。本文不授权：

- 打开、关闭或替换当前用户普通/production Editor；
- 修改 production/安装态二进制或 Codex/Editor 真实配置；
- 修改 Tower 项目源文件或真实项目配置；
- 运行 Local CI、Windows release/export 或完整真实视觉矩阵；
- 删除仓库、用户目录、G 盘共享目录或既有 run root；
- 清理不属于本窗口 owner 的进程/临时目录。

测试若创建 run-owned temp，必须记录绝对 root 与 owner，超时终止 owned descendants，并在保留必要失败证据后
按 construction skill 的 strict-child/reparse-point 规则清理。预计本文不需要递归清理授权；若出现需要，先停止报告。

## 17. 完成与归档条件

只有以下全部成立，才能宣称 277 完成：

1. Gate A-F 依序通过；
2. 零配置 Button、Profile、None、Mouse、Touch、Submit、Cancel/失焦/隐藏/禁用均有有效证据；
3. PointerDown 当帧 present、same-frame Click pulse 与 outside release 防卡态成立；
4. layout/hit/navigation/clip/action invariants 有自动化证据；
5. Player 与 Editor GameView 共用模块并通过受影响 consumer tests；
6. 未新增 Button renderer primitive、Animator2D 耦合或 Tower hardcode；
7. 失败与重跑记录完整，未运行项有理由；
8. 完成记录、入口文档和归档同步完成。

Gate F 完成不自动授权 Tower UI consumer、Local CI、production replacement、真实配置、发布构建或 AUI v2 后续系统。

Window C 到此结束并进入文档归档；不得把完成状态解释为任何 deferred consumer/release 授权。

## 18. 激活前复核清单

后续激活 Window A 时必须逐项重新执行；任一失败则本文保留在 `待执行/` 并修订：

1. 用户明确授权激活 277 Window A / Gate A-B；
2. `施工文档/当前/` 没有其它命名施工文档；
3. 本文仍是待执行队首，正式 277 方案未被替代；
4. 读取最新 HEAD、git status 与所有拟修改文件 scoped diff；
5. 确认当前 AUI schema/input/present baseline 与第 4 节一致；
6. 确认 272/273/275/276 完成状态未被回退；
7. 确认 `aui_control_feedback.rs` 或实际模块落位不会形成第二套 public seam；
8. 确认 Gate A 新测试 prefix/包名可执行；若命名或 owner 漂移，先修订本文；
9. 确认 Window A / Gate A-B 的实现、测试、证据和清理保守估计可在三小时内完成；
10. 确认没有 Tower、Local CI、真实 Editor、production/安装态或真实配置隐式授权；
11. 把本文移动到 `施工文档/当前/` 并同步 54 后，才允许第一次源码修改。

## 19. 文档自审

### 19.1 是否忠实实现正式方案 B

通过。施工包含零配置默认 Profile、紧凑覆写、Auto/None/Profile、Button-only、短 transition 和 reduced motion；
没有退化为写死方案 A，也没有扩张成完整状态皮肤/Timeline 方案 C。

### 19.2 是否保持业务 Action 与视觉反馈分离

通过。Interaction/Action Mapper 仍唯一生成 Click/Submit；Feedback 只消费 snapshot/commands 并输出 visual override。
Gate B/E 明确验证 exactly-once 和 gameplay input filtering。

### 19.3 是否保持 Layout/Renderer 边界

通过。Gate D 要求 computed rect、hit/navigation/clip 不变，复用 Rect/Image/Text 与既有 texture/glyph present，
禁止 Button 专用 command/pipeline。

### 19.4 是否满足触摸与防卡态

通过。Gate A 建立 pointer device/cancel 合同，Gate B 覆盖 outside/cancel/focus/modal/session reconciliation，
Gate E 覆盖 Player/GameView bridge；多 pointer 并发明确延期。

### 19.5 是否符合 Rust 底层规则 + schema 上层规则

通过。Rust 深模块拥有状态、时间、取消、插值和诊断；AUI v2 schema 只声明受限 Profile 与 selector，项目不编写
运行时动画代码，也不把反馈写入 ProjectUiStateSnapshot。

### 19.6 是否避免重复验证

通过。Gate A-E 使用 red-capable owner/consumer prefix，Gate F 只在冻结后运行最小受影响 closure；同配置完整 suite
覆盖的 filter 不重复，Local CI/production/真实视觉不为标签而运行。

### 19.7 是否保护 dirty worktree

通过。第 4、18 节要求逐文件重叠复核，禁止 reset/revert；无法安全合并时停止，不覆盖用户或前序施工变化。

### 19.8 是否符合三小时限制

通过。Window A-C 分开授权，每个窗口有明确 Gate 和停止点；不允许一次授权跨窗，到 2:45/3:00 执行硬停止。

### 19.9 是否误授权外部状态

没有。本文不授权 Editor、production/installed、真实配置、Tower、Local CI、release 或递归清理。

### 19.10 自审结论

```text
document review: passed
design alignment: passed
ownership/deep-module review: passed
input/touch/cancel review: passed
layout/action/renderer boundary review: passed
validation duplication review: passed
dirty-worktree safety review: passed
three-hour review: passed
queue state: active document; Window A / Gate A-B complete
implementation allowed now: none after Window A stop point
next allowed action: user explicitly authorizes Window B / Gate C-D before feedback evaluator work begins
```

## 20. 当前停止点

```text
277 construction document: active
document location: 施工文档/当前/
Window A / Gate A: completed
Window A / Gate B: completed and stopped
Window B / Gate C-D: completed and stopped
Window C / Gate E-F: not authorized, not started
source code: Gate A input/schema/cooker/RuntimePackage and Gate B interaction owner changes applied incrementally on dirty baseline
tests: Gate A-B targeted owner, affected interaction regression and consumer compile closure passed
Local CI: not run
real/production Editor: unchanged
production/installed binary: unchanged
real configuration: unchanged
Tower project: unchanged
```

## 21. Window A 激活记录

```text
activation date: 2026-08-09
authorized scope: Window A / Gate A-B
preflight: passed
repository HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
active named construction documents after activation: 1 (277)
pending named construction documents after activation: 0
formal design: 277 scheme B remains effective
completed prerequisites: 272 / 273 / 275 / 276 archives present
feedback module baseline: absent
runtime AUI schema baseline: aui-document.v1
working tree: dirty; relevant overlap belongs to preserved prior 273/275/276/font/Animator2D changes
merge policy: incremental on current worktree; reset/revert/overwrite forbidden
estimated Window A: within three hours including Gate A-B tests, evidence and cleanup
implementation/test started during activation turn: no
forbidden: Window B-C, Tower, Local CI, real/production Editor, production/installed replacement, real configuration
next authorized action: begin Gate A red-capable owner tests and implementation
```

激活前复核逐项结论：

1. 用户授权 Window A / Gate A-B：通过；
2. 激活前当前目录无其它命名施工文档：通过；
3. 277 是唯一待执行队首，正式方案未被替代：通过；
4. HEAD、git status 与拟修改文件 scoped diff 已读取：通过；
5. input/AUI schema/interaction/present 基线仍与第 4 节一致：通过；
6. 272/273/275/276 已完成归档存在：通过；
7. 新 feedback module 可落在 engine_runtime 深模块，不需要第二套 public seam：通过；
8. Gate A 所属 crate 与 planned test prefix 可用，新增 red tests 后再执行：通过；
9. Window A / Gate A-B 保守估计在三小时内：通过；
10. 无 Tower、Local CI、真实 Editor、production/安装态或真实配置隐式授权：通过；
11. 本文已移动到 `当前/`，入口同步完成后才允许下一次源码施工：通过。

## 22. Gate A 施工记录与停止点

### 22.1 本次授权覆盖

```text
execution date: 2026-08-09
latest scoped authorization: Gate A only
entered: Input Capability, AUI v2 Schema, v1 -> v2 Migration, RuntimePackage v2-only validation
not entered: Gate B interaction snapshot/capture/reconciliation implementation
forbidden and unchanged: Window B-C, Tower source, Local CI, real/production Editor,
                         production/installed binary, real configuration
```

### 22.2 Red-capable owner evidence

先增加 owner tests，再执行并观察到预期失败：

```text
cargo test -p engine_input runtime_pointer_ -- --nocapture
  red first failure: E0599 RuntimeInputEvent::pointer_event missing
  additional red contract: RuntimePointerEvent / RuntimePointerPhase /
                           RuntimePointerDeviceKind / touch constructors missing

cargo test -p editor_core aui_document_feedback_ -- --nocapture
  red first failure: E0432 AuiFeedbackSelection /
                     AuiInteractionFeedbackProfile /
                     AuiInteractionFeedbackRegistry /
                     LEGACY_AUI_DOCUMENT_SCHEMA_VERSION missing

cargo test -p engine_runtime runtime_package_rejects_invalid_aui_feedback_ -- --nocapture
  red first failure: E0425 LEGACY_AUI_DOCUMENT_SCHEMA_VERSION and
                     validate_runtime_aui_document_schema missing
```

以上失败均来自计划中的 owner seam，不使用 Local CI 或完整 workspace 作为修复循环。

### 22.3 完成的 Gate A 实现

Input capability：

1. 新增 `RuntimePointerDeviceKind::{Mouse, Touch, Pen}` 与显式 `hover_capable`；
2. 新增 `RuntimePointerPhase::{Down, Move, Up, Held, Cancel, Leave}` 和统一
   `RuntimeInputEvent::Pointer { pointer }`；
3. 既有 `PointerDown/Move/Up/Held` fixture 通过 `pointer_event()` 明确迁移为 Mouse、pointer id 0，
   未删除旧 variant，也未静默丢字段；
4. Raw input 新增 TouchStart/Move/End/Cancel、MouseLeave 和 touch identity；
5. v1 touch ownership 固定为一个 primary owner，secondary start/move/end deterministic ignore；
6. focus lost 对已按下 Mouse 和 active Touch 生成 typed Cancel，不再用 PointerUp 冒充 cancel；
7. AUI 现有 pointer adapter 仅兼容统一 Down/Move/Held/Up；Cancel/Leave 的 interaction reconcile
   保留给 Gate B，本次未进入状态机/capture 施工。

AUI v2 schema 与 migration：

1. `AUI_DOCUMENT_SCHEMA_VERSION` 升级到 `aui-document.v2`，保留
   `LEGACY_AUI_DOCUMENT_SCHEMA_VERSION = aui-document.v1`；
2. 新增 transparent `AuiFeedbackSelection`：`auto` / `none` / profile id；
3. 新增 deny-unknown-fields 的 registry/profile、有限 easing、受限 scale/translation/
   brightness/opacity/duration 字段；
4. 新增 Rust 版本化内建 identity `button.default.v1`，不写入每份项目文档；
5. `AuiNode.feedback` serde default 为 Auto，`AuiDocument.interaction_feedback` default 为 None；
6. Cooker 只在 migration 边界接受 source v1，规范化为 v2、Auto fallback，并输出
   `AuiDocumentV1MigratedToV2`；
7. profile 以 `profile_id` canonical sort；duplicate/missing、motion scale、profile scale、opacity、
   brightness、translation、duration、easing 均有结构化诊断；
8. cook report 记录 source schema、normalized schema 与 fallback identity；
9. RuntimePackage helper 明确拒绝 source v1，并在 body validation 拒绝非法 v2 range/reference。

### 22.4 Green evidence

```text
cargo test -p engine_input runtime_pointer_ -- --nocapture
  passed: 6, failed: 0

cargo test -p engine_runtime aui_feedback_schema_ -- --nocapture
  passed: 2, failed: 0

cargo test -p editor_core aui_document_feedback_ -- --nocapture
  passed: 4, failed: 0

cargo test -p engine_runtime runtime_package_rejects_invalid_aui_feedback_ -- --nocapture
  passed: 2, failed: 0

cargo check -p editor_window_winit -p project_e2e_gate
  passed; only pre-existing dead_code warnings
```

格式与 patch hygiene：相关 Rust 文件已执行 `rustfmt --edition 2021`，相关路径
`git diff --check` 通过。没有执行 Local CI、完整 workspace suite、真实 Editor、Tower Gate、发布构建、
production/安装态替换或真实配置修改。

### 22.5 Gate A 结论

```text
Gate A: passed
Window A: paused at Gate A stop point
Gate B: not entered
Window B-C: not entered
next allowed action: explicit user authorization for Window A / Gate B
```

## 23. Window A / Gate B 激活记录

```text
activation date: 2026-08-09
authorized scope: Window A / Gate B only
Gate A evidence reused: yes; relevant product/test inputs unchanged since Gate A green closure
repository HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
worktree: dirty; Gate A and preserved prior changes remain in place
owner: engine_runtime::aui::AuiInteractionSystem / AuiInteractionState
allowed: red-capable interaction owner tests, snapshot/capture/cancel/reconciliation implementation,
         targeted affected consumer validation, documentation evidence
forbidden: Window B-C, feedback evaluator/draw/present integration, Tower, Local CI,
           real/production Editor, production/installed replacement, real configuration
next action: add and observe Gate B owner red tests
```

## 24. Window A / Gate B 完成记录

### 24.1 Owner red evidence

先新增 `aui_control_interaction_` owner tests，再运行：

```text
cargo test -p engine_runtime aui_control_interaction_ -- --nocapture
  red first actionable failure: E0609 AuiInteractionResult.control_snapshot missing
  additional planned seams confirmed missing: process_session_with_state,
                                       control_reconciliation_count,
                                       AuiCommandKind::PointerCancel
```

该失败来自 Gate B 计划中的 interaction owner seam；未使用 Local CI、完整 workspace、真实 Editor
或 production binary 作为修复循环。

### 24.2 完成的 Gate B 实现

1. `AuiInteractionState` 新增私有 primary press capture、hover、interaction session identity 与
   pending reconciliation 计数；公共写入口未扩散；
2. 新增稳定只读 `AuiControlInteractionSnapshot`，报告 hovered/pressed、inside、pointer id/device/
   hover capability、focus、modal 与 active screen；
3. primary pointer Down 建立稳定 owner；Move outside/back inside 只更新 inside eligibility；Up 无论命中
   何处都结束 owner，只有同 owner inside 才 exactly-one Click；
4. Touch Up/Cancel 不保留 Hover；PointerCancel 清理匹配 owner 的 press/drag/scroll capture 且不生成 Click；
   PointerLeave 清 Hover，并让仍持有的 press 转为 outside；
5. drag/scroll capture 保存 pointer id/device，非 owner pointer 不得释放 capture；captured outside Move/Up
   继续由 UI consume，保持 gameplay filtered frame 合同；
6. effectively hidden reconciliation 扩展到 disabled、removed、modal scope、screen scope 与显式 session
   identity change；`push_screen/pop_screen` 清理计数在下一帧或当前 pop 帧稳定可观测；
7. keyboard/gamepad Submit 与既有 Action 仍 exactly once；未加入 feedback evaluator、DrawList override、
   Player/GameView present 或任何 Window B-C 逻辑。

### 24.3 Green evidence

```text
cargo test -p engine_runtime aui_control_interaction_ -- --nocapture
  passed: 5, failed: 0

cargo test -p engine_runtime aui_interaction_ -- --nocapture
  passed: 6, failed: 0

cargo test -p engine_input aui_input_filter_ -- --nocapture
  passed: 2, failed: 0

cargo test -p engine_runtime aui_drag_ -- --nocapture
  passed: 3, failed: 0

cargo test -p engine_runtime aui_modal_ -- --nocapture
  passed: 1, failed: 0

cargo test -p engine_runtime aui_scroll_ -- --nocapture
  passed: 2, failed: 0

cargo test -p engine_runtime aui_scrollbar_ -- --nocapture
  passed: 1, failed: 0

cargo test -p engine_runtime aui_focus_ -- --nocapture
  passed: 1, failed: 0

cargo test -p engine_runtime aui_effective_visibility_reconciles_hidden_interaction_state_once -- --nocapture
  passed: 1, failed: 0

cargo test -p engine_runtime aui_screen_flow_ -- --nocapture
  passed: 1, failed: 0

cargo test -p engine_runtime aui_gamepad_ -- --nocapture
  passed: 1, failed: 0

cargo check -p runtime_player_winit -p editor_core -p project_e2e_gate -p editor_window_winit
  passed; Gate B introduced no new production warning; remaining warnings are pre-existing dead_code warnings
```

相关 Rust 文件已执行 `rustfmt --edition 2021`。未运行 Local CI、完整 workspace suite、真实或
production Editor、Tower Gate、release build、production/安装态替换、真实配置修改或递归清理。

### 24.4 Window A 结论

```text
Gate A: passed
Gate B: passed
Window A: completed and stopped
Window B / Gate C-D: not authorized, not entered
Window C / Gate E-F: not authorized, not entered
next allowed action: explicit user authorization for Window B / Gate C-D
```

## 25. Window B / Gate C-D 激活记录

```text
activation date: 2026-08-09
authorized scope: Window B / Gate C-D only
Window A evidence reused: yes; Gate A-B product and targeted evidence remain present
repository HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
worktree: dirty; all prior and user-owned changes must be preserved
owner: engine_runtime::aui_control_feedback + AuiLayoutEngine draw extraction seam
allowed: red-capable evaluator/draw owner tests, deterministic feedback state,
         AuiVisualOverrideSet and subtree draw extraction, targeted affected validation,
         documentation evidence
forbidden: Window C / Gate E-F, Player/GameView feedback lifecycle integration, Tower,
           Local CI, real/production Editor, production/installed replacement,
           real configuration, release build
activation review: passed; one active document, scope fits the three-hour window,
                   no external-state mutation required
next action: add and observe Gate C owner red tests
```

## 26. Window B / Gate C-D 完成记录

### 26.1 Owner red evidence

Gate C 先加入 `aui_control_feedback_` owner tests，首次运行：

```text
cargo test -p engine_runtime aui_control_feedback_ -- --nocapture
  red first actionable failure: E0432 unresolved AuiControlFeedbackFrameInput /
                                AuiControlFeedbackModule /
                                AuiControlFeedbackState /
                                AuiFeedbackDiagnosticsLevel
```

Gate D 先加入 `aui_feedback_draw_` owner tests，首次运行：

```text
cargo test -p engine_runtime aui_feedback_draw_ -- --nocapture
  red first actionable failure: E0599
    AuiLayoutEngine::extract_draw_list_with_visual_overrides missing
```

两个失败均来自施工文档预定 owner seam；未使用 Local CI、完整 workspace、真实 Editor、Player 或
production binary 作为修复循环。

### 26.2 Gate C 实现

1. 新增独立 `aui_control_feedback.rs` 深模块，公共入口保持
   `AuiControlFeedbackModule::advance(state, frame_input) -> frame`；
2. 新增 compact `AuiControlFeedbackState`、稳定 `AuiVisualOverrideSet`、逐帧 report 与
   Off/Summary/Trace 诊断等级；
3. Button `auto` 解析内建 `button.default.v1`，支持 registry default/显式 project Profile、missing
   profile fallback 与 `feedback:none`；
4. 实现 `Disabled > Pressed > Hovered > Normal`、FocusVisible 正交增亮、Click/Submit Activated；
5. Hover 70ms、Pressed 45ms、Activated 120ms、Cancel 80ms 使用显式
   `presentation_delta_us` 和有限 easing，不读取 wall clock、render FPS 或 Animator2D fixed tick；
6. from/current/target retarget 保持每节点单一 compact state；同帧 Click 即使 large delta 也至少输出一个
   Activated present，重复命令不堆积 pulse/state；
7. `motion_scale_permille=0` 立即求值但保留颜色/透明度反馈；large delta clamp、time reversal、非有限值
   recovery 均有确定性计数；
8. hidden/removed、session replacement、Modal/Screen identity change 回收 transient state。

### 26.3 Gate D 实现

1. 新增 `AuiLayoutEngine::extract_draw_list_with_visual_overrides`，既有 `extract_draw_list` 保持兼容并走
   同一内部 extraction；
2. override 以稳定 Button owner node id 索引，子树归属通过 Document parent 链解析，不依赖 `:label` 猜测；
3. Button 根及可视 descendants 围绕 owner computed rect 中心统一缩放/位移；LayoutResult、Hit Rect、
   navigation 与 Action 不回写；
4. brightness/opacity 在 authored Rect/Image/Text color 后乘算，支持 `#RRGGBB`/`#RRGGBBAA`；
5. painter order、draw command kind/count、asset id、Text/font payload、effective clip rect 均保持；
6. 没有新增 DrawButton、AnimatedButton、texture、GPU pipeline 或项目语义；
7. `control_style_deferred` 已在真实 evaluator + draw extraction seam 接入后改为 false。

### 26.4 Green evidence

```text
cargo test -p engine_runtime aui_control_feedback_ -- --nocapture
  passed: 7, failed: 0

cargo test -p engine_runtime aui_feedback_draw_ -- --nocapture
  passed: 2, failed: 0

cargo test -p engine_runtime aui_render_ -- --nocapture
  passed: 1, failed: 0

cargo test -p engine_runtime aui_layout_ -- --nocapture
  passed: 1, failed: 0

cargo test -p engine_runtime aui_clip_ -- --nocapture
  passed: 1, failed: 0

cargo test -p engine_runtime aui_image_ -- --nocapture
  passed: 2, failed: 0

cargo test -p engine_runtime aui_text_ -- --nocapture
  passed: 2, failed: 0

cargo test -p engine_runtime aui_renderer_ -- --nocapture
  passed: 2, failed: 0

cargo test -p engine_runtime aui_composition_ -- --nocapture
  passed: 3, failed: 0

cargo test -p engine_runtime aui_interaction_ -- --nocapture
  passed: 6, failed: 0

cargo check -p runtime_player_winit -p editor_core -p project_e2e_gate -p editor_window_winit
  passed; only pre-existing dead_code warnings
```

相关 Rust 文件已执行 `rustfmt --edition 2021 --check`，相关 tracked patch 已通过
`git diff --check`。未运行 Local CI、完整 workspace suite、真实/production Editor、Tower、release、
production/安装态替换、真实配置修改或递归清理。

### 26.5 Window B 结论

```text
Gate C: passed
Gate D: passed
Window B: completed and stopped
Window C / Gate E-F: not authorized, not entered
Player/GameView feedback lifecycle integration: not entered
next allowed action: explicit user authorization for Window C / Gate E-F
```

## 27. Window C / Gate E-F 激活记录

```text
activation date: 2026-08-09
authorized scope: Window C / Gate E-F only
Window A-B evidence reused: yes; owner source and targeted evidence remain present
repository HEAD: acaffa8cb95bbb54d093bbf45835a59315c82da5
worktree: dirty; prior engine/editor/Tower changes must be preserved
owners: engine_runtime AUI present owner + runtime_player_winit + editor_core GameView play
allowed: red-capable Player/GameView consumer tests, shared same-frame feedback present seam,
         session lifecycle/reset, affected integration closure, completion record and archive
forbidden: Tower project mutation, Local CI, real/production Editor, production/installed replacement,
           real configuration, release/export, real visual matrix
activation review: passed; one active document, prior Gates green, scope fits three-hour window,
                   no external-state mutation required
next action: add and observe Gate E Player/GameView consumer red tests
```

## 28. Window C / Gate E-F 完成记录

Player 与 Editor GameView 现共用 `AuiRuntimePresenter::apply_control_feedback_with_fonts`：先处理当前帧
interaction，再以同一显式 1/60 秒 presentation delta 求值 feedback、重建 DrawList/Composition，并在同帧
送入 render target。Player window 与每个 Editor Play instance 持有独立 state；session/document/target identity
进入 reconciliation，Editor Stop 显式 reset。Action dispatch、gameplay filter、720×1280 portrait mapping、字体与
Image present 均保持。

首次冻结 closure 暴露两个历史 v1 fixture（runtime_player headless AUI、RuntimePackage builder）；均只将测试输入
修订为 package 要求的 `aui-document.v2`，owner reproduction 与全量 suite 重跑通过。

```text
cargo fmt --all --check: passed
cargo test -p engine_input: 32 passed
cargo test -p engine_runtime: 555 passed
cargo test -p runtime_player_winit: 20 passed
cargo test -p editor_core aui_document_: 5 passed
cargo test -p editor_core editor_gameview_: 11 passed
Player same-frame feedback owner: 1 passed
Editor GameView same-frame feedback owner: 1 passed
portrait coordinate regression: 1 passed
```

未运行：Local CI、真实/production Editor、Tower、release/export、production/安装态替换、真实配置或视觉矩阵。

```text
Gate E: passed
Gate F: passed
277: completed and archived
current authorization: none
```
