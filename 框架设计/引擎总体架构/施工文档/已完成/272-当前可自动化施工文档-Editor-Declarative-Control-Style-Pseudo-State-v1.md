# 272 Editor Declarative Control Style / Pseudo-State v1 施工文档

状态：已完成。2026-08-05 按 273 后基线恢复的 Gate F input/authority closure 与 fresh source-built
真实 OS click 已通过；Gate A-F 全部闭环，本文归档到 `施工文档/已完成/`。

正式方案：`272-Editor-Declarative-Control-Style-Pseudo-State-v1方案.md`。

## 1. 施工目标

在 Native Rust Editor retained widget 主线上建立深 `EditorControlStyleModule` 和统一控件交互状态机，
让 `Button / IconButton / Tab / Toggle` 通过版本化样式表、受控 selector、确定性 cascade 与伪状态得到
普通、悬停、按住、选中、勾选、禁用和键盘焦点视觉；支持可选状态图片、Nine-Slice、tint 与 fallback。

完成后，Toolbar、Workspace Dock Tab（含 `Game`）、panel chrome、Project Launcher 和常用决策按钮不得
再各自手写 hover/pressed/selected 颜色分支。普通按钮默认采用 `ReleaseInside` exactly-once 激活；需要
PointerDown 的 Dock Tab 拖拽/菜单类 consumer 必须显式声明 `Press`，不能依赖历史偶然行为。

## 2. 当前基线

已确认现有能力：

```text
EditorFocusInputSystem 已持有 hovered_hit_id / pressed_hit_id / pointer_capture / keyboard_focus
UiRendererConfig 已向 renderer 传入 hovered_hit_id / pressed_hit_id
Toolbar 已手写 normal/hovered/pressed/disabled
Workspace Tab 只手写 active/inactive
EditorInputRouter::route_widget 当前只在 PointerDown 生成命令
EditorWidgetDeclaration/Node 已有 role/enabled/binding/hit_region_id/paint
UiDrawList 已有 Rect/Text/ImageTextureSlot
UiGpuDrawPlan 已有 image quad、UV、clip 与 ordered ImageTextures batch
EditorImageTextureRegistry 已支持 image texture upload/resolve
```

基线风险：工作树当前包含大量前序用户修改，且 `editor_input`、`editor_ui_renderer`、
`editor_wgpu_renderer`、`editor_window_winit` 均有修改。激活前不得 reset、checkout 或覆盖；必须读取当前
文件、记录 272 起始 diff，并把重叠改动视为当前真实基线。

## 3. Owner、consumer 与预计文件

### 3.1 样式 owner

```text
rust/crates/editor_ui_renderer/src/control_style.rs（新增）
rust/crates/editor_ui_renderer/src/widget_tree.rs
rust/crates/editor_ui_renderer/src/widget_extract.rs
rust/crates/editor_ui_renderer/src/draw_list.rs
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_ui_renderer/src/tests.rs
rust/resources/editor/themes/dark-neutral/control-style.v1.json（新增）
rust/resources/editor/themes/dark-neutral/control-*.png（按 Gate B 最小集新增）
rust/resources/editor/themes/dark-neutral/manifest.json（新增或扩展既有主题 manifest）
```

### 3.2 输入 owner

```text
rust/crates/editor_input/src/lib.rs（必要时拆出 widget_interaction.rs）
rust/crates/editor_window_winit/src/focus.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/input_route.rs
```

### 3.3 GPU 与 host adapter

```text
rust/crates/editor_wgpu_renderer/src/draw_plan.rs
rust/crates/editor_wgpu_renderer/src/image_texture.rs
rust/crates/editor_wgpu_renderer/src/real_wgpu.rs
rust/crates/editor_wgpu_renderer/src/render_graph.rs（只在现有 batch metadata 需要同步时修改）
rust/crates/editor_wgpu_renderer/src/tests.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/visual_regression_gate.rs
rust/crates/editor_window_winit/src/bin/editor_ui_authority.rs（只在复用现有 authority 所需时修改）
```

### 3.4 直接 consumer

```text
rust/crates/editor_ui_renderer/src/panels/toolbar.rs
rust/crates/editor_ui_renderer/src/panels/mod.rs（Workspace Tab / panel chrome）
rust/crates/editor_ui_renderer/src/panels/launcher.rs
rust/crates/editor_ui_renderer/src/panels/project_runtime_trust.rs
其它经 consumer inventory 证明属于 v1 的确认/取消控件
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/editor_window_winit/src/tests/workspace_tab_drag.rs
rust/crates/editor_window_winit/src/tests/workspace_panel_chrome.rs
```

预计文件只是当前代码基线上的施工导航。激活复核发现 owner 已移动时，应先修订本文档，不得为了符合
旧路径复制新模块。

## 4. 授权、禁止项与施工窗口

当前施工授权：

```text
允许：修改 272 范围内 Rust 产品源码、主题图片和测试 fixture
允许：运行 Gate A-F 的 owner/affected tests 与 run-owned source-built Editor 视觉验证
允许：在当前连续施工授权内依次执行 Window 1-3，但每个连续窗口不得超过三小时
禁止：替换或安装 production Editor 二进制
禁止：运行 Local CI
禁止：修改 production/安装态二进制、真实用户配置、系统 DPI
禁止：恢复 Tower P0-5 Gate G/H、glyph shard 或 MSDF 并行化
```

本文已完成激活前复核。施工拆成三个连续窗口，每个窗口
不超过三小时：

```text
Window 1：Gate A-B，schema/resolver + image/Nine-Slice/GPU
Window 2：Gate C-D，interaction machine + Toolbar/Workspace consumer
Window 3：Gate E-F，剩余 consumer + affected regression/visual/docs
```

任何连续授权到 2h45m 后不再启动新实现或验证；到 3h 必须停止并保全证据。

## 5. 激活前复核

移动到 `当前/` 前必须完成：

1. 重新读取 272 正式方案、`49`、`54`、当前/待执行目录和本文件。
2. 确认 `当前/` 除 README 外为空，且用户明确授权激活/施工。
3. 记录 `git status --short`，逐一检查上述 owner/consumer 的现有用户改动。
4. 确认 `WidgetRole`、`WidgetPaint`、`DrawCommand`、`UiGpuDrawPlan`、`route_widget`、Dock Tab drag
   路径没有基线漂移；有漂移则修订本文并重新自审。
5. 确认下面的 cargo package/test filter 存在且非零匹配；不存在时用当前 owner suite 替换并写回本文。
6. 明确 Window 授权范围；没有授权的 production replacement、Local CI、真实配置修改继续禁止。
7. 激活时将本文移入 `施工文档/当前/`，同步 `54` 后才可修改源码。

## 6. Gate A：Style schema、selector 与 resolver

### 6.1 实现

1. 在 `editor_ui_renderer` 建立深 `EditorControlStyleModule`，外部 interface 只接收
   `ControlStyleQuery`，返回 `ResolvedControlStyle`；规则匹配、cascade、token、fallback、cache 和
   Trace 保持为内部实现。
2. 冻结唯一 `editor-style-sheet.v1` JSON schema。正式代码不同时维护 YAML/JSON 两种真相。
3. 增加稳定类型：

```text
ControlClass / ControlClassSet（有界、排序、去重）
ControlPseudoStateSet（hover/active/selected/checked/disabled/focus/focus-visible flags）
ControlStyleSelector（role/classes/pseudo/pseudoNot）
ControlStyleDeclarations（只允许 paint property）
ControlStyleQuery / ResolvedControlStyle
ControlStyleSummary / ControlStyleTrace / typed diagnostics
```

4. `WidgetRole` 补齐真实可达的 `IconButton`、`Toggle`；如果当前没有 Toggle consumer，也必须完成角色、
   resolver 和单元测试，但不得伪造产品 consumer。
5. selector 禁止 WidgetId/path/text/HitTarget/panel_id、祖先关系、正则和表达式。
6. cascade 严格按 origin、pseudo count、class count、role、canonical rule index 合并 property；相同输入
   必须 byte-identical，不依赖 map 迭代顺序。
7. 伪状态 declaration 拒绝 width/height/margin/font size 等布局 property；`content_offset` 只能改变内容
   绘制位置。
8. cache key 仅含 sheet generation、role、canonical classes、pseudo bits；WidgetId/label/rect 不得进入。
9. Off 不构造解释字符串；Summary 只含计数/耗时/fallback；Trace 才含 matched/winning rules。

### 6.2 Red-capable owner 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer control_style -- --nocapture
```

必须覆盖：

```text
schema/version/selector validation
normal/hover/active/disabled
selected+hover、selected+active、checked+focus-visible
property merge、same-specificity canonical order
token missing、非法 pseudo layout property、无 base rule fallback
cache key 不含 widget id/label/rect
Off/Summary/Trace 分档
```

### 6.3 停线条件

若实现需要树选择器、脚本求值或 panel 专用 matcher，判定为方案偏离，先修订 272，不进入 Gate B。

## 7. Gate B：ControlBrush、状态图片、Nine-Slice 与 ordered painter

### 7.1 实现

1. 为 DarkNeutral 增加 hash-pinned theme manifest、样式表和最小真实状态图片集。至少让
   `workspace-tab` 的 hover/active/selected 使用可辨识的实际 texture pixels；不得只用 fallback rect
   冒充“不同图片”。
2. 图片使用小型 PNG 与 Nine-Slice，禁止为每个尺寸复制位图。manifest 记录 texture id、路径、尺寸、
   digest、slice inset 和 fallback color。
3. 深模块支持 `None / Solid / Texture / NineSlice`、border、foreground、icon tint、opacity、
   content offset；颜色/边框仍是未配置图片时的正式路径。
4. 将 `ImageTextureSlot` 深化为通用 UV-capable image draw contract，或以等价单一通用 contract 替换；
   禁止增加仅供 theme 使用的平行 DrawCommand。
5. Nine-Slice 在 UI extract/paint owner 展开为最多 9 个稳定 ordered quad；目标尺寸小于边角和时采用
   确定性 clamp，不产生负 rect/UV。
6. clip 必须同时裁剪目标 rect 和 source UV；tint/opacity 进入 image quad/vertex/shader，不能在 CPU
   复制纹理。
7. 复用 `EditorImageTextureRegistry` 作为 GPU adapter。theme loader 校验 manifest/hash/size、解码一次、
   上传一次；不扫描项目目录、不接受任意文件路径。
8. 缺失/corrupt texture 输出 typed diagnostic，并使用明确 fallback color 保持 UI 可操作。
9. Rect -> image -> text/content 的 painter order 必须由 UiDrawList 顺序决定，不能恢复全局材质重排。

### 7.2 Owner/consumer 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer control_brush -- --nocapture
cargo test -p editor_wgpu_renderer image_texture -- --nocapture
cargo test -p editor_wgpu_renderer nine_slice -- --nocapture
cargo test -p editor_wgpu_renderer ordered -- --nocapture
cargo test -p editor_wgpu_renderer --features real-wgpu image_texture -- --nocapture
```

测试 filter 必须在激活复核或施工时确认非零匹配；同一配置已被后续 owner suite 覆盖时不机械重复。

必须证明：

```text
Nine-Slice 目标/UV 九区正确
小尺寸 clamp 与 clip UV 正确
tint/opacity 到 GPU plan 和 shader
同 texture id 多控件只上传一次
missing/corrupt 使用 fallback 且有 diagnostic
实际 texture quad 在 text 前后保持声明顺序
```

### 7.3 停线条件

若现有 image pipeline 无法在不破坏 266 ordered batch 的前提下表达 UV/tint，先保留失败证据并修订
施工设计；不得用 9 张独立整图或 debug overlay 绕过。

## 8. Gate C：统一控件交互状态机与激活策略

### 8.1 实现

1. 在 `editor_input` 建立深 `EditorWidgetInteractionMachine`（名称可按当前 owner 调整），以
   `event + widget tree + prior state` 返回：

```text
interaction snapshot（hover/active/capture/focus-visible）
optional exactly-once activation
disabled feedback
diagnostics
redraw dirty widget ids
```

2. `EditorFocusInputSystem` 只保留其仍拥有的 window/panel/focus职责；pressed/capture 不能与新状态机保持
   两套可分叉真相。
3. `EditorWidgetDeclaration/Node` 增加 `control_classes`、model pseudo state 与
   `ActivationPolicy::{ReleaseInside, Press}`，默认 ReleaseInside。
4. ReleaseInside 状态机：down capture+active；移出清 active 但保留 capture；移回恢复；inside up
   exactly-once activate；outside up/focus lost/cancel 不激活并清理。
5. Press：down exactly-once activate，up 不重复。只能由 consumer inventory 明确选择。
6. keyboard Space/Enter 产生 focus-visible/active 与 exactly-once activation；pointer focus 不错误显示
   focus-visible。
7. disabled 不 active、不 capture、不执行命令，继续产生 127 的 disabled feedback。
8. legacy non-retained input/asset drag/splitter resize/GameView input 不得被普通按钮状态机吞掉。

### 8.2 激活策略 inventory

Gate C 必须先冻结并测试：

```text
Toolbar/Launcher/Trust/Confirm/Cancel：ReleaseInside
Workspace Dock Tab：Press（因现有 activate + begin drag 语义；up 不重复）
Toolbar overflow/menu opening：按真实现有交互明确选择，默认 ReleaseInside
Splitter resize/Tab drag/Asset drag：专用 gesture，不伪装 Button Activate
TextInput：Focus，不套用 ReleaseInside command
```

如真实代码证明某项策略不同，先把理由和测试写回 inventory。

### 8.3 Owner 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_input widget_interaction -- --nocapture
cargo test -p editor_window_winit focus -- --nocapture
```

必须覆盖 272 正式方案第 19.2 节全部矩阵，另加：

```text
Press down 激活一次/up 不重复
Dock Tab down 同时允许既有 drag arm
asset drag/splitter/GameView route 不回归
pointer capture lost 与窗口 FocusLost 无 stuck active
```

### 8.4 停线条件

若同一 pointer event 同时由旧 `route` 和新 interaction machine 生成业务命令，立即停线修复 owner；禁止
在 application call site 用去重集合掩盖双发。

## 9. Gate D：Toolbar、Workspace Tab 与 panel chrome 迁移

### 9.1 实现

1. `UiRendererConfig` 或 retained extraction 接收统一 interaction visual snapshot，不再让各 panel 直接比较
   `hovered_hit_id/pressed_hit_id` 选颜色。
2. Toolbar 声明 `role=Button/IconButton + class=toolbar-control`，删除手写
   disabled/pressed/hovered/normal 分支。
3. Workspace Tab 声明 `role=Tab + class=workspace-tab`；selected 只来自
   `active_panel_id`，`Game` 不得有 id/panel 专用分支。
4. panel lock/more/close 声明共享 `panel-chrome-control` class；disabled/locked 通过 flags/model state。
5. Dock Tab activate/drag/down/up/cross-window 行为保持既有 workspace owner，Press 只负责一次激活，不
   把 drag gesture 移入样式模块。
6. 添加源码 guard：产品 panel renderer 不得重新读取 `hovered_hit_id/pressed_hit_id` 决定控件 paint；
   只允许 style/interaction owner 和测试读取。

### 9.2 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer toolbar -- --nocapture
cargo test -p editor_ui_renderer workspace -- --nocapture
cargo test -p editor_window_winit workspace_tab -- --nocapture
cargo test -p editor_window_winit workspace_panel_chrome -- --nocapture
cargo test -p editor_window_winit native_app -- --nocapture
```

必须证明：

```text
Scene/Game/Timeline/Animator 同 class 自动获得状态视觉
Game 五态可由状态输入稳定构造
Toolbar 命令不双发/不提前触发
Dock Tab 点击、拖拽、跨窗口 dock 不回归
panel close/lock/more disabled 与 selected/locked 可辨
不存在 panel_id == game_view 视觉判断
```

## 10. Gate E：Launcher、决策按钮、Toggle 与通用性闭环

### 10.1 实现

1. 迁移 Project Launcher primary/secondary action、recent project 可点击项中的按钮角色，以及
   Trust/Confirm/Cancel 等正式决策按钮。
2. 对可达 Toggle consumer 迁移 checked 状态；若没有正式 consumer，报告 `no_product_consumer`，只保留
   resolver/interaction owner 测试，不在产品界面造假控件。
3. 建立 `EditorControlStyleCoverageReport`，至少报告：

```text
sheet id/generation/rule count
role/class/pseudo state coverage
migrated consumer groups
remaining interactive consumer groups + reason
manual hover/pressed paint violation count
cache hit/miss、fallback count、texture upload count
```

4. Report 遵守 Off/Summary/Trace；默认 UI 热路径不得构造全量 matched rule 字符串。
5. 未迁移 consumer 必须明确分类为非控件 gesture、TextInput、Viewport 或后续范围；不能静默漏项。

### 10.2 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer launcher -- --nocapture
cargo test -p editor_ui_renderer project_runtime_trust -- --nocapture
cargo test -p editor_window_winit project_runtime_trust -- --nocapture
cargo test -p editor_ui_renderer control_style_coverage -- --nocapture
cargo test -p editor_window_winit release_inside -- --nocapture
```

必须包含 down 尚未提交、inside up 提交一次、outside up 取消、disabled feedback 保留的真实 consumer
测试，而不只测试状态机孤立函数。

## 11. Gate F：受影响回归、视觉矩阵与文档闭环

### 11.1 Development/Integration 回归

按 affected closure 运行一次上层 suite，不机械重复已被同配置覆盖的 filtered test：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer
cargo test -p editor_input
cargo test -p editor_wgpu_renderer
cargo test -p editor_wgpu_renderer --features real-wgpu
cargo test -p editor_window_winit
cargo test -p editor_window_winit --features real-window
cargo fmt --all -- --check
```

若新增/修改 workspace lint surface，运行 repository-owned `quality_gate -- propose-ledger` 只检查 drift，
不得自动应用 ledger。没有 lint surface 变化则明确豁免，不机械运行。

不默认运行完整 workspace、`editor_core`、`engine_runtime`、project_e2e_gate 或 Local CI；只有真实 dependency
closure 证明受影响时才加入并写明原因。

### 11.2 Headless/production-feature 视觉矩阵

在 run-owned target/state/evidence root 中，以同一 source identity 生成：

```text
1280x720 与 1600x900
logical scale 1.00 / 1.25 / 1.50 / 2.00
Game Tab normal / hover / active / selected / selected+hover
普通 Button normal / hover / active / disabled
实际 texture pixels 与 fallback-only 对照
```

每张证据必须报告 target rect、采样 RGBA/hash、texture id、resolved pseudo states、winning rule 和
fallback status。必须证明文字不被背景覆盖、状态变化不改 layout/hit rect。

### 11.3 真实 Windows/WGPU smoke

用户可见完成声明需要一次 source-built、run-owned、`real-window + real-wgpu` Editor 场景：

```text
移动到 Game Tab -> hover image
按下 -> active image
释放 -> selected image
移动到普通按钮 -> hover
按下并拖出后释放 -> 不执行，视觉恢复
```

真实 OS DPI 只资格化主机/显示器实际报告的 scale；不得修改系统 DPI 冒充矩阵。其余 scale 由上面的
production-feature offscreen WGPU 矩阵证明，并在报告中区分 `actual_os` 与 `simulated_logical`。

本 Gate 不授权替换 production/安装态二进制、不修改真实用户配置、不启动 Tower Gate G/H。若后续用户
要求打开正式 Editor，必须另行授权 replacement/launch，不能把 run-owned smoke 说成已安装版本。

### 11.4 性能资格

记录同一 UI model 的 steady-state：

```text
style rule count / cache key count
first resolve 与 steady cache hit duration
hover old/new dirty widget count
frame allocation/resolve count
theme texture upload count（每 identity 至多一次）
```

不预设虚假毫秒阈值；Gate A 先记录当前手写基线，Gate F 要求 steady hover 不解析 JSON、不全树分配
class/string、不重复上传纹理，且没有显著可复现回退。若出现回退，先 owner-level profile 后修复。

## 12. 失败处理

任何 Gate 失败：

1. 保留首个失败 assertion、stdout/stderr、输入 state、resolved style/trace 与证据路径。
2. 分类为方案、施工文档、实现、环境或既有基线。
3. 用当前 owner filtered test 建立最小 red reproduction；不得用 Local CI 或完整 workspace 调试。
4. 修复后只重跑 reproduction 和受影响 consumer；未受影响且输入 identity 未变的证据可复用。
5. 若修复推翻 selector、cascade、activation 或 image contract，先修改 272 正式方案和本文，再继续。

### 12.1 2026-08-04 实际施工记录

已完成：

```text
Gate A：EditorControlStyleModule、受控 selector、确定性 cascade/cache、Off/Summary/Trace
Gate B：Solid/Texture/NineSlice、UV clip、tint、WGPU shader、4 张 hash-pinned PNG、fallback diagnostic
Gate C：统一交互状态机、ReleaseInside/Press、capture、移出/移回、取消和键盘激活
Gate D：Toolbar、Workspace Tab、panel chrome 迁移；无 Game 专用视觉分支
Gate E：Launcher、Trust、AssetPicker、AI decision button 迁移；Toggle 报告 no_product_consumer
Gate F：affected suites、格式、lint drift producer、headless/production-feature 矩阵和真实 WGPU 截图
```

受影响回归：

```text
editor_ui_renderer: 74 passed
editor_input: 36 passed
editor_wgpu_renderer default: 19 passed
editor_wgpu_renderer --features real-wgpu: 26 passed
editor_window_winit default: 175 passed, 1 ignored
editor_window_winit --features real-window: 186 passed, 8 ignored
real-wgpu image texture filtered suite: 4 passed
cargo fmt --all -- --check: passed
git diff --check: passed
editor_ui_renderer clippy owner: 0 new warnings；仅保留 editor_ui_model 既有 warnings
quality_gate --locked -- propose-ledger: candidate/diff 已生成，未应用
```

真实视觉证据：

```text
run root: C:\Users\ZENGHA~1\AppData\Local\Temp\aife-272-gate-f-capture-r2-1785850777333
1280x720: Passed, actual_os_scale=1.25, backend=Vulkan
RGBA SHA256: sha256:d8f0bfe6721e0cfc637c5b20cb4d5853a59f8bcd549df9d67123af0b9ef25694
1600x900: Passed, actual_os_scale=1.25, backend=Vulkan
RGBA SHA256: sha256:07d790f4296ab092a39586822686fdece35d4ee43b8ab7b1a5b2bf4a792dbf74
font: C:\Windows\Fonts\segoeui.ttf;embedded:NotoSansSC-VF.ttf
人工检查：画面非空、中文可见、按钮与文字不重叠、两个尺寸布局稳定
```

唯一未闭环项：

```text
classification: environment
code: authority_input.target_not_foreground
foreground_pid: 16104（Codex 桌面应用）
failed evidence root: C:\Users\ZENGHA~1\AppData\Local\Temp\aife-272-gate-f-click-r3-1785851201086
影响：无法在本自动化会话中证明真实 OS PointerDown/PointerUp 前台点击序列
不影响：owner/consumer exactly-once tests、production-feature visual matrix、真实 WGPU 截图证据
```

已验证 Windows foreground 限制不能通过产品内 `AttachThreadInput` 或 Alt 解锁可靠修复；相关试验代码已
完整撤回。后续只允许两种闭环方式：用户配合将 source-built 测试窗口置前后重跑 authority click，或用户
明确批准此环境项豁免。未获得其中任一项前，不归档 272。

## 13. 完成标准

只有以下条件同时成立才能声明完成：

```text
Gate A-F 全部通过
EditorControlStyleModule interface 与 272 一致
实际状态图片/Nine-Slice 进入 WGPU pixels
ReleaseInside/Press exactly-once consumer 矩阵通过
Toolbar/Workspace/Launcher/decision button consumer 迁移完成
Game 无专用视觉分支
旧 Toolbar 手写交互颜色分支删除
affected crate regression 与 fmt 通过
真实 Windows/WGPU smoke 通过
未运行项和残留 consumer 明确记录
```

Headless 状态计数、DrawCommand 数量或 fallback rect 不能单独证明用户可见完成。

## 14. 文档同步与归档

全部完成后：

1. 生成 `阶段完成记录/2026-08-XX-Editor-Declarative-Control-Style-Pseudo-State-v1/00-总览.md`。
2. 更新 `272` 方案状态、`00-文档地图.md`、`00-阅读顺序.md`、`49`、`54`、
   `施工文档/README.md`、`施工文档/待执行/README.md`、`阶段完成记录/README.md`。
3. 将当前文档移到
   `施工文档/已完成/272-当前可自动化施工文档-Editor-Declarative-Control-Style-Pseudo-State-v1.md`。
4. 记录源码/test harness/theme asset/真实窗口证据 identity，区分 dirty development evidence 与最终
   qualification evidence。
5. Local CI、production/安装态 replacement、真实用户配置、Project Runtime AUI 和 Tower Gate G/H 若未
   单独授权，必须继续写为未执行，不能借 272 自动扩权。

## 15. 施工文档自审

```text
正式方案已由用户确认：是，方案 C
与 272 正式方案一致：是
是否只修 Game 页签：否，建立通用 style/pseudo-state/interaction 链路
是否复制完整 CSS/USS：否，selector 限于 role/classes/pseudo/pseudoNot
是否接管 layout：否，pseudo declaration 禁止布局属性
是否支持真实状态图片：是，Gate B 要求 hash-pinned PNG + Nine-Slice + actual texture pixels
是否区分 active 与 selected/checked：是
是否处理 PointerDown 历史语义：是，Gate C inventory + exactly-once consumer matrix
是否保留 Dock Tab drag：是，显式 Press 且 gesture owner 不迁入样式模块
是否复用 266 ordered painter 与现有 image registry：是
是否修改 Project Runtime AUI/RuntimePackage/塔防项目：否
是否包含每 Gate 测试、停线条件和失败证据：是
是否包含实际 consumer 和真实 Windows 视觉验证：是
是否要求 Local CI/production replacement：否，保持未授权
外部审查文档：未发现针对 272 的其它 AI 审查；无需修改正式方案
工作树重叠风险：已记录，激活前必须复核且禁止 reset/checkout
连续窗口是否受三小时限制：是，拆为 Window 1/2/3
施工时状态：本文曾唯一占用施工槽
最终文档位置：已完成
当前可施工：否；施工已完成
激活前置：已完成；用户明确授权、基线复核、移入当前并同步 54
自审结论：通过
```

## 16. 2026-08-05 临时让位记录

用户明确授权将 272 让出施工槽并激活/施工 273。Gate F 当时停在证据边界，原因不是实现失败，而是唯一
剩余资格证据受 Windows foreground policy 环境阻塞；当前执行槽让位给更高优先级的 273 GameView
Presentation Target / AUI Input Coordinate Consistency P0。

暂停时有效证据：

```text
Gate A-E 产品实现与 owner/consumer tests
Gate F affected crate regression 与 fmt
production-feature 1280x720 / 1600x900 视觉矩阵
真实 Windows/WGPU 截图与 RGBA digest
EditorControlStyleModule、Nine-Slice/image、ReleaseInside/Press 合同
```

未完成项：

```text
真实 OS PointerDown/PointerUp 前台点击序列
classification = environment
code = authority_input.target_not_foreground
```

273 将触碰 `editor_window_winit/src/application.rs`、`input_route.rs`、`tests/native_app.rs` 等共同
consumer。触碰后，272 的 OS input/authority closure 证据视为失效；style resolver、GPU draw、主题资产
及其视觉证据在对应输入 identity 未变化时继续有效。恢复 272 时按最终 273 diff 重跑受影响的
input/authority closure，不机械重跑全部 style/GPU 矩阵。

恢复条件：273 完成或正式暂停并释放当前槽；用户明确授权恢复 272；重新复核当前 input/authority owner、
测试 filter 和 foreground 资格。允许的闭环仍只有用户配合将 source-built 测试窗口置前后重跑，或用户
明确批准环境项豁免。Local CI、production/安装态替换、真实配置和 Tower Gate G/H 仍不自动授权。

## 17. 2026-08-05 恢复与重新激活记录

恢复条件已满足：273 Gate A-F 已完成并归档，当前施工槽已释放；用户明确授权“恢复并激活 272，按
273 后基线施工剩余 Gate F 闭环”。重新复核确认：

```text
EditorControlStyle owner tests：7 个非零匹配
NativeEditorApplication native_app tests：54 个非零匹配
real-window authority/click tests：仍存在
273 后新增 input closure：GameView presentation revision、pointer down/up 与 production authority
```

恢复施工只更新失效的 input/authority evidence。旧 style resolver、theme asset、production-feature
offscreen WGPU matrix 与真实截图在其输入 identity 未变化时继续复用。真实 click 必须使用最新源码构建、
fresh run root 和普通 Windows foreground OS input route；不得用 PostMessage、直接 command 或环境豁免
冒充通过。若 foreground 再次被 Codex 桌面应用占用，记录新的 environment evidence 并等待用户配合或
明确豁免。

## 18. 2026-08-05 Gate F 最终闭环

273 后失效的 input/authority evidence 已按当前源码重新验证：

```text
editor_ui_renderer control_style：7 passed
release_inside_toolbar_waits_for_up_and_cancels_outside_without_dispatch：1 passed
editor_window_winit input_runtime_loop：18 passed
editor_window_winit --features real-window production_authority：6 passed
cargo fmt --all -- --check：passed
272 scope git diff --check：passed
```

273 完成时已覆盖相同输入与配置的全量 winit evidence 继续复用：default 178 passed / 1 ignored，
real-window 189 passed / 8 ignored。272 既有 style/GPU/offscreen matrix 与真实 WGPU 截图的输入 identity
未变化，按施工规则复用而不机械重跑。

fresh 真实 OS click 一次通过：

```text
scenario：272-resume-launcher-release-inside
run root：C:\Users\zenghaoran\AppData\Local\Temp\aife-272-resume-click-1785940192181
source-built binary：C:\Users\zenghaoran\AppData\Local\Temp\aife-272-resume-target-20260805\debug\editor_ui_authority.exe
binary SHA256：sha256:33492732a6b2597ad7a587bf8d7efa586f0e24a920365ee713bba4f852b08f1c
actual OS scale：1.25
backend：Vulkan
foreground verified：true
pointer down/up observed：true / true
widget：editor/control/hit.project_launcher.create_with_ai
command：create_with_ai
after command：start_create_project_with_ai
focused widget：editor/control/hit.project_launcher.create_with_ai
route status：Passed
RGBA SHA256：sha256:1597793fefb5c26a594416dc83e2a3d6c6f1004477c8aaebcff5a765f67f95e4
diagnostics：none
report：evidence/272-resume-launcher-release-inside/report.json
workspace evidence：evidence/272-resume-launcher-release-inside/workspace.json
screenshot：evidence/272-resume-launcher-release-inside/frame.png
```

人工检查确认截图非空，中文、按钮、导航和项目区无重叠或裁切；点击后模态 dim 状态与
`start_create_project_with_ai` 一致。旧 `authority_input.target_not_foreground` 环境阻塞已解除，无需豁免。

本次未运行 Local CI，未替换 production/安装态二进制，未修改真实用户配置或系统 DPI，未启动 Tower
Gate G/H。当前证据绑定 dirty working-tree source-built binary，不声明 clean exact-commit 或安装态资格。
