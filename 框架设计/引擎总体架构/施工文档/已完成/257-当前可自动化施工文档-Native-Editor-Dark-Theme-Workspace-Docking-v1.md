# 257 当前可自动化施工文档：Native Editor Dark Theme + Workspace Docking v1

## 0. 文档状态

```text
对应正式方案：../../257-Native-Editor-Dark-Theme-Workspace-Docking-v1方案.md
文档状态：D0-D7、真实窗口、受影响域与 exact-commit Local CI 全部通过；已完成归档
生成日期：2026-07-26
激活时间：2026-07-26T18:22:56.2531062+08:00
窗口 B 激活时间：2026-07-26T19:00:43.4819547+08:00
窗口 B 收口时间：2026-07-26T19:30:29.4054695+08:00
窗口 B 三小时硬停止：2026-07-26T22:00:43.4819547+08:00
窗口 C 激活时间：2026-07-26T19:36:40.7548441+08:00
窗口 C 收口时间：2026-07-26T20:17:55.8446277+08:00
窗口 C 三小时硬停止：2026-07-26T22:36:40.7548441+08:00
窗口 D 激活时间：2026-07-26T20:27:15.6316168+08:00
窗口 D 三小时硬停止：2026-07-26T23:27:15.6316168+08:00
当前施工槽：本文件 D6-D7
施工授权：窗口 D；production panel / DPI / real-window acceptance、受影响域最终回归、Local CI、完成记录与归档
禁止：浮动窗口、多显示器、跨 OS 窗口拖拽、项目 AUI Theme
```

本施工文档只实现 257 方案 B。原生浮动窗口、多显示器、跨窗口拖拽、项目 AUI Theme、用户主题编辑器和通用 stylesheet 均不在本轮。

## 1. 施工目标

在不重建 248 retained WidgetTree 的前提下完成：

```text
EditorTheme::DarkNeutral 语义主题；
EditorWorkspaceDockingModule 单一工作区 owner；
Split / Stack / Tab 工作区树；
Splitter resize；
Tab 激活、排序和跨 Stack dock；
center/edge drop preview 与原子 commit；
close / show / reset；
Editor 用户布局持久化和损坏回退；
当前生产 workspace panel 全覆盖；
固定 EditorLayout::resolve 和旧 dormant DockLayoutManager 退出重复真相；
真实窗口中的 resize / drag / dock 和 actual RGBA 证据。
```

## 2. 激活前基线

文档生成时只读基线：

```text
branch：codex/255-capability-catalog
HEAD：a94874ef26cf2d717b7627d77281f85f2bb9566d
tree：1098c8e11d7e6c0fdcef7a1cbce7ef8e914eb681
worktree：包含 257 正式方案与入口文档的未提交文档变更
当前施工文档：无
待执行施工文档：除 README 外仅本 257 文档
```

激活时不得机械绑定上述 HEAD。必须重新执行只读复核：

```powershell
Set-Location G:\gameEngin
git status --short
git rev-parse HEAD
git rev-parse "HEAD^{tree}"
Get-ChildItem "框架设计\引擎总体架构\施工文档\当前" -Force
Get-ChildItem "框架设计\引擎总体架构\施工文档\待执行" -Force
```

若 257 方案、相关代码或 crate 依赖已经漂移，先修订本施工文档并重新自审；不得执行过期 Gate。

## 3. 当前实现事实

### 3.1 可复用底座

```text
editor_ui_renderer::EditorWidgetTree
editor_ui_renderer::layout_widget_tree
editor_ui_renderer::extract_widget_tree
editor_ui_renderer::pick_widget
editor_window_winit::EditorFocusInputSystem
editor_window_winit::editor_ui_authority
editor_ui_renderer::native_editor_panel_manifest
现有 real-window / real-wgpu-surface / actual RGBA capture
```

### 3.2 必须退出的重复事实

```text
rust/crates/editor_window_winit/src/dock.rs
  -> DockLayoutManager::fixed_default
  -> compute_fixed_rects
  -> 独立 PanelRegistry / DockArea / DockLayoutNode

rust/crates/editor_ui_renderer/src/layout.rs
  -> EditorLayout::resolve
  -> 固定 hierarchy / viewport / inspector / bottom rect

rust/crates/editor_ui_renderer/src/panels/mod.rs
  -> 固定 bottom tab 列表和绝对位置
```

目标不是先保留这些事实再加 `WorkspaceLayoutV2`。生产切换完成后，只允许一个可变 workspace layout owner。

### 3.3 主题基线

`editor_ui_renderer/src/theme.rs` 只有 `UiColor` 常量；生产 panel 和 launcher 仍存在可由主题表达的直接 `UiColor::rgba(...)`。

## 4. 依赖方向与实现 owner

### 4.1 正式 seam

`editor_window_winit` 已依赖 `editor_ui_renderer`，而 248 禁止 `editor_ui_renderer -> editor_window_winit`。

因此实现方向固定为：

```text
editor_ui_renderer
  -> EditorTheme
  -> Workspace tree / invariant / resolve / snapshot
  -> Dock Widget compose / preview paint

editor_window_winit
  -> 持有 EditorWorkspaceDockingModule 实例
  -> OS/editor input 转 WorkspaceIntent
  -> 用户配置路径与原子持久化 adapter
  -> focus / pointer capture / real-window authority
```

若施工时证明更小的现有 seam 合理，可调整文件位置，但不得产生反向 crate 依赖或第二个 layout owner。

### 4.2 预计新增或修改文件

生产范围预计包括：

```text
rust/crates/editor_ui_renderer/src/workspace_docking.rs（新增，名称可按最终 seam 微调）
rust/crates/editor_ui_renderer/src/theme.rs
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_ui_renderer/src/layout.rs
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_ui_renderer/src/panels/mod.rs
rust/crates/editor_ui_renderer/src/panel_manifest.rs
rust/crates/editor_ui_renderer/src/widget_tree.rs
rust/crates/editor_ui_renderer/src/draw_list.rs
rust/crates/editor_window_winit/src/dock.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/focus.rs
rust/crates/editor_window_winit/src/config.rs 或窄的 workspace persistence adapter
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/bin/editor_ui_authority.rs
```

测试范围预计包括：

```text
rust/crates/editor_ui_renderer/src/tests.rs 或同 crate 私有测试模块
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/editor_window_winit/src/tests/reachability_gate.rs
rust/crates/editor_window_winit/src/tests/native_real_window.rs
必要时新增与生产 owner 同 crate 的 workspace_docking 测试文件
```

若必须修改 `editor_input` 或 `editor_wgpu_renderer`，先记录缺口、受影响 consumer 和对应验证；禁止为了方便扩大公共输入或 GPU Interface。

## 5. 非目标与禁止项

```text
不新增独立 editor_workspace_docking crate，除非激活复核出现真实复用/依赖证据并先回填方案；
不新增 257 专用长期 Runner；
不恢复手写 HitRegion；
不让 panel 保存 rect / split ratio / drop target；
不让 renderer 回写 Dock tree；
不把 layout 写入项目 root、Asset DB、RuntimePackage 或 project digest；
不为 Dock action申请项目 mutation Grant；
不修改 Runtime Player、项目 AUI、Build 或 Export 语义；
不执行旧 248 全矩阵作为日常修复循环；
不自动更新 visual baseline；
不运行与受影响声明无关的历史 Gate。
```

## 6. 施工窗口与三小时上限

完整 257 不能保守地保证在一个三小时窗口内完成，因此按同一施工文档拆成四个可停止窗口：

```text
窗口 A：D0-D2，红合同、主题、workspace 核心
窗口 B：D3，生产布局切换与 Splitter
窗口 C：D4-D5，Tab/Dock 与持久化
窗口 D：D6-D7，真实窗口、受影响回归、文档与归档
```

规则：

```text
一个连续授权窗口可在剩余时间足够时连续推进后续 Gate；
一次授权因 agent 报告/停止、范围改变或三小时到期而结束；
2 小时 45 分后不启动新实现或验证；
每个窗口结束时保存已通过/失败证据、清理 owned process/temp，并记录下一精确入口；
不得为了追求 Gate 编号完整而在剩余时间不足时启动昂贵 stage。
```

## 7. D0 - Ownership Baseline 与红合同

### 7.1 目标

先用 owner 级测试捕获当前缺口，不先修改生产代码。

### 7.2 红合同

新增能分别失败的测试：

```text
一个 panel 只能存在于一个 Stack；
无空 Stack / 无退化 Split / active tab 有效；
非法 ratio、重复 panel、未知 panel 和循环 fail closed；
default layout 包含全部生产 dockable panel；
当前固定 layout 不能接受用户 split ratio；
当前 DockTab 不能跨 Stack；
当前主题缺少语义 token 或仍有装饰性 panel-local rgba；
layout schema 损坏必须回到 default，而不是 panic。
```

红测试不得靠 `#[ignore]` 或只匹配未来类型名冒充失败；它必须捕获 257 用户行为或 owner invariant。

### 7.3 定向命令

实现测试名称固定包含 `workspace_docking` 或 `editor_dark_theme`，便于 owner 反馈：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer --lib --locked workspace_docking -- --nocapture
cargo test -p editor_ui_renderer --lib --locked editor_dark_theme -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_docking -- --nocapture
```

D0 证据保留第一条真实 assertion。D0 的目标是得到有效 red，不要求整个 crate 保持红到窗口结束；D1/D2 完成后同一 owner 测试应转绿。

## 8. D1 - EditorTheme::DarkNeutral

### 8.1 任务

1. 定义不可变 `EditorTheme` 和唯一内置 `DarkNeutral`。
2. 建立 257 第 15.2 节语义 token。
3. Renderer、Launcher、Menu、Toolbar、Panel、Tab、Field、Popup、Modal、Overlay 统一从 theme 取值。
4. 移除可由主题表达的生产 panel-local `UiColor::rgba(...)`。
5. 保留数据状态色的必要差异，但状态色也必须通过 theme semantic token。
6. 不实现用户主题切换、配置 schema 或项目覆盖。

### 8.2 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer --lib --locked editor_dark_theme -- --nocapture
cargo test -p editor_ui_renderer --lib --locked theme -- --nocapture
```

静态审查：

```powershell
Set-Location G:\gameEngin
rg -n "UiColor::rgba" rust/crates/editor_ui_renderer/src/panels rust/crates/editor_ui_renderer/src/renderer.rs
```

该搜索允许保留确实不属于主题的动态/诊断计算色，但每个残留必须在自审记录中分类，不能用“测试需要”笼统豁免。

## 9. D2 - Workspace Tree / Invariant / Snapshot

### 9.1 任务

1. 实现稳定 `LayoutNodeId`、`PanelId`、`PanelDescriptor`。
2. 实现 `Split { axis, ratio, first, second }` 与 `Stack { tabs, active }`。
3. 实现 registry reconcile、validate、ratio clamp、minimum-size resolve。
4. 实现内置 default layout。
5. 实现只读 `WorkspaceSnapshot`，包含 resolved rect、splitter、active tabs、drag preview 和 diagnostics。
6. 实现 `WorkspaceIntent -> WorkspaceUpdate` 的小 Interface；内部 tree rewrite 不公开。
7. 现有 panel manifest 是生产 panel 集合的输入，不再维护另一份手写 panel 清单。
8. Toolbar/Menu/Launcher chrome 不进入 dock tree。

### 9.2 最小尺寸策略

```text
descriptor minimum_size 是硬约束输入；
窗口无法满足全部最小尺寸时生成 minimum_size_unsatisfied；
优先保证 center workspace 非负且可恢复；
任何 resolved rect 都不得出现 NaN、Infinity 或负宽高；
窄窗口策略不得靠文字覆盖相邻控件。
```

### 9.3 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer --lib --locked workspace_docking -- --nocapture
cargo test -p editor_ui_renderer --lib --locked panel_manifest -- --nocapture
cargo test -p editor_ui_renderer --lib --locked widget_layout -- --nocapture
```

窗口 A 结束条件：

```text
D0 red 已被 D1/D2 owner 实现转绿；
主题和 tree core 不依赖 editor_window_winit；
尚未声称生产 fixed layout 已退出；
无真实窗口或全 workspace 验证。
```

## 10. D3 - Production Cutover 与 Splitter Resize

### 10.1 任务

1. `NativeEditorApplication` 持有唯一 `EditorWorkspaceDockingModule`。
2. Renderer compose 消费 `WorkspaceSnapshot`，不再调用固定 `EditorLayout::resolve` 生成 workspace panel rect。
3. 迁移、替换或删除 `editor_window_winit::DockLayoutManager` 和重复 `PanelRegistry`。
4. Menu/Toolbar/Status 等 Shell rect 保持 shell owner；workspace body 只由 Module snapshot 解析。
5. 为 Splitter 建立稳定 WidgetId、真实 WidgetTree hit/pick 和轻微放大的命中区域。
6. PointerDown/Move/Up 通过统一 pointer capture 驱动 resize。
7. FocusLost / Escape / capture lost 能收敛，不残留 drag/pressed state。
8. resize 只刷新相关 layout，不重建业务 `EditorUiModel`。

### 10.2 红/绿测试

至少包括：

```text
水平 splitter resize；
垂直 splitter resize；
minimum-size clamp；
pointer capture owner；
PointerUp commit；
FocusLost/Escape cleanup；
window resize 后 ratio 保持；
生产 panel rect 全部来自 workspace snapshot；
固定 EditorLayout 和 compute_fixed_rects 不再是生产入口。
```

### 10.3 命令

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer --lib --locked workspace_docking -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_splitter -- --nocapture
cargo test -p editor_window_winit --lib --locked focus -- --nocapture
cargo test -p editor_window_winit --lib --locked retained_widget -- --nocapture
```

窗口 B 结束条件：

```text
生产 fixed workspace layout 已退出；
Splitter 在 headless application input route 中真实可拖；
尚未声称 Tab 跨区、持久化或真实 OS drag 已完成。
```

## 11. D4 - Tab Stack / Drag Preview / Atomic Dock

### 11.1 任务

1. 当前固定 bottom tab 列表改为按 snapshot 的每个 Stack 组合。
2. 单击 Tab 只激活。
3. 超过逻辑像素阈值后由 Armed 进入 Dragging。
4. Center drop 加入目标 Stack。
5. Left/Right/Top/Bottom drop 创建 Split + Stack。
6. Preview 和 commit 使用同一个 target resolution。
7. Commit 原子完成 source remove、target insert、active、empty cleanup、split collapse、focus transfer 和 revision。
8. stale source/target 或非法 drop 保持原布局并产生 typed diagnostic。
9. Escape、FocusLost 和 capture lost 取消并恢复原布局。
10. Viewport camera drag、Asset drag 和 Dock drag owner 显式互斥。

### 11.2 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer --lib --locked workspace_docking -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_tab_drag -- --nocapture
cargo test -p editor_input --lib --locked asset_browser_drag -- --nocapture
cargo test -p editor_window_winit --lib --locked viewport_runtime -- --nocapture
```

必须覆盖：

```text
click below threshold；
same-stack reorder；
cross-stack center；
四个 edge target；
preview/commit target equality；
source becomes empty；
stale target fail closed；
cancel keeps original layout；
focused panel moves with focus；
drag owners do not cross-consume。
```

## 12. D5 - Close / Show / Reset / Persistence

### 12.1 任务

1. closable panel 可关闭，不可关闭 panel fail closed。
2. Window/Panel 菜单可以恢复 closed panel。
3. reset 使用内置 default layout，不修改项目/selection/document。
4. 定义 `EditorWorkspaceLayout.v1`，只持久化结构、ratio、tabs、active 和 closed panels。
5. 路径属于 Editor 用户状态；测试注入独立 store path。
6. 使用临时文件 + 原子替换或仓库已有等价安全原语。
7. restore 执行 parse、schema、invariant、registry reconcile、clamp、fallback。
8. 未知旧 panel 删除；新增 panel 按 default placement 补入。
9. persistence 失败只产生诊断，不阻止 Editor 启动。
10. 不持久化像素 rect、drag/focus、Widget index、业务 Model 或项目绝对路径。

### 12.2 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer --lib --locked workspace_layout_schema -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_persistence -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_panel_visibility -- --nocapture
```

负向测试至少包括：

```text
missing file；
malformed JSON；
wrong schema；
duplicate panel；
unknown panel；
removed panel；
new panel；
invalid ratio；
write failure；
interrupted/partial temp file；
reset after corrupt restore。
```

窗口 C 结束条件：

```text
主窗口内完整 Dock 行为和用户布局生命周期通过 owner/consumer tests；
外部 store 仅使用 test-owned root；
没有运行真实 OS authority 或最终受影响域回归。
```

## 13. D6 - Production Panel / DPI / Real-window Acceptance

### 13.1 生产 panel 覆盖

按施工时 `native_editor_panel_manifest` 核对所有生产 dockable panel，不使用文档中的静态数量冒充：

```text
Hierarchy；
Viewport；
Inspector；
Asset Browser；
Console；
Runtime Trace；
Authoring Workflow；
Input Mapping；
Build & Export；
AI Panel；
Project Intent；
Report；
施工时 manifest 中新增的其它 workspace panel。
```

Launcher、Menu、Toolbar 和 Modal 只验证主题与 Shell 行为，不强行 Dock。

### 13.2 deterministic 尺寸/DPI

只运行与新 Dock geometry 直接相关的代表矩阵：

```text
1280x720 @ 1.0；
1600x900 @ 1.5；
1920x1080 @ 2.0；
一个窄窗口 minimum-size pressure scenario。
```

这不是重复 248 的全量 DPI 资格；目标只证明新增 splitter/tab/drop geometry 经过既有 DPI adapter 后一致。

### 13.3 真实窗口

窄扩展现有 `editor_ui_authority` 支持受控 drag scenario，禁止新建 257 长期 Runner。真实窗口至少一次覆盖：

```text
目标 PID/HWND/foreground 校验；
真实 PointerDown/Move/Up splitter resize；
真实 Tab drag 到另一个 Stack 或 edge；
drop preview 与 layout revision；
actual RGBA capture；
DarkNeutral 的非空、非单色区域与关键 token 采样；
执行后布局 invariant；
窗口关闭与 owned process cleanup。
```

authority 参数、project fixture、state store、evidence root 都必须显式且 test-owned。不得控制当前用户正在使用的 Editor 窗口。

建议命令骨架，激活后按最终 authority Interface 写成精确命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit --lib --locked workspace_dpi_matrix -- --nocapture
cargo test -p editor_window_winit --all-features --locked workspace_real_window -- --ignored --nocapture --test-threads=1
```

若使用 binary：

```powershell
cargo run -p editor_window_winit --bin editor_ui_authority `
  --features real-window,real-wgpu-surface --locked -- `
  <explicit 257 drag scenario arguments>
```

真实桌面/GPU不可用时记录 `environment_blocked`，不得以 headless matrix 冒充 D6 完成。环境阻塞不授权修改系统 DPI 或外部配置。

## 14. D7 - 受影响域回归、质量与归档

### 14.1 回归选择

257 修改共享 Editor UI Interface，因此最终受影响闭包为：

```text
editor_ui_renderer；
editor_window_winit；
若实际修改则包括 editor_input / editor_wgpu_renderer；
真实 editor_host build composition。
```

项目 Runtime、AUI、RuntimePackage、Build、Export、AI Gateway 未被修改时，不运行其全量产品验收。

### 14.2 命令

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --all -- --check
cargo test -p editor_ui_renderer --locked
cargo test -p editor_window_winit --locked
cargo check -p editor_window_winit --all-features --locked
cargo clippy -p editor_ui_renderer -p editor_window_winit -p editor_input `
  --all-targets --all-features --no-deps --locked -- -D warnings
cargo build -p editor_host --locked --features real-window,real-wgpu-surface
```

条件命令：

```text
修改 editor_input -> cargo test -p editor_input --locked
修改 editor_wgpu_renderer -> cargo test -p editor_wgpu_renderer --all-features --locked
修改共享 Cargo/feature/dependency -> 扩展到实际 consumer closure
```

不默认运行：

```text
cargo test --workspace；
cargo test --workspace --all-features；
project_e2e_gate；
Local CI；
旧 248 的 3x3 + 三档真实 DPI 全矩阵。
```

只有 merge/release policy 或用户后续明确要求 exact-commit qualification 时，才在 clean exact commit 上增加一次 fail-fast Local CI；Local CI 不作为本施工的调试循环。

### 14.3 完成 artifact

最终完成记录至少包含：

```text
source commit/tree；
实际修改文件；
唯一 workspace owner 证明；
旧 fixed layout 退出证明；
theme token 与残留 rgba 分类；
panel manifest 覆盖；
owner/consumer test 结果；
真实窗口 scenario、OS/GPU/DPI、PNG 和 digest；
持久化 store path 与 cleanup；
未运行测试及理由；
elapsed time；
remaining deferred scope。
```

## 15. 失败处理

任何 Gate 失败时：

1. 保存第一条 assertion、stdout/stderr、输入、环境和 cleanup 状态。
2. 分类为设计、施工文档、实现、环境或既有 baseline。
3. 用最小 owner 级 reproduction 修复；不把 D7 或真实窗口作为 repair loop。
4. 只重跑被修改身份影响的 owner 和 consumer。
5. 若发现方案错误，停止扩展代码，先修订 257 正式方案和本施工文档。
6. 若发现 external state 不可安全清理，按 construction skill 记录并停止该 cleanup，不绕过宿主。
7. 失败不自动扩展到原生浮动窗口、多显示器或新 Runner。

非 mutation 的 owner test 在首因修复后可以重跑；真实 OS authority 只有在环境、输入和 owned cleanup 仍满足时才执行，不以次数标签制造一次性 Gate。

## 16. 外部状态与清理

```text
测试 state store、fixture、evidence 和 TEMP/TMP 使用不同的 repository-external role-owned root；
不复用用户当前 Editor 的配置文件；
不向项目 root 写 workspace layout；
真实窗口仅操作本次 authority 启动的 PID/HWND；
超时/异常释放 pointer、关闭窗口、终止 owned descendants；
先保存必要证据，再清理 role-owned temp；
递归删除必须按 construction skill 验证 strict child、无逃逸 reparse point、无存活 descendant；
artifact 保留，temp/target 按 owner 清理。
```

## 17. 文档同步与归档

只有 D0-D7 和 257 验收声明全部成立后：

1. 生成 `阶段完成记录/2026-xx-xx-Native-Editor-Dark-Theme-Workspace-Docking-v1/00-总览.md`。
2. 更新 257 正式方案状态为施工完成并归档。
3. 更新 `00-文档地图.md`。
4. 更新 `49-当前状态与下一步入口.md`。
5. 更新 `54-当前可自动化施工总入口.md`。
6. 更新 `施工文档/README.md`。
7. 更新 `阶段完成记录/README.md`。
8. 将本文件从 `施工文档/当前/` 移到 `施工文档/已完成/`。

任一真实窗口或受影响回归缺失时，不生成完成记录，不标记 257 完成。

## 18. 激活前复核清单

```text
[ ] 257 正式方案仍为已确认方案 B
[ ] 当前/待执行目录状态与入口一致
[ ] 本文件是唯一准备激活的施工文档
[ ] 工作树中的用户变更已识别且不会被覆盖
[ ] crate 依赖仍是 editor_window_winit -> editor_ui_renderer
[ ] 当前生产 renderer 仍使用固定 EditorLayout::resolve
[ ] 旧 DockLayoutManager 仍未驱动生产 renderer
[ ] panel manifest 与生产可见 panel 已重新枚举
[ ] 现有 editor_ui_authority 仍可作为窄扩展入口
[ ] D0 owner 测试名与命令可真实过滤到测试
[ ] 真实窗口/GPU 只在 D6 才需要
[ ] 每个连续窗口有小于三小时的保守预算
[ ] 禁止范围没有被后续代码漂移推翻
```

复核通过后，才可把本文件移动到 `施工文档/当前/` 并同步 54。移动本身不自动授权超出用户明确指定的施工窗口。

## 19. 施工文档自审

### 19.1 是否与正式方案一致

通过。覆盖 DarkNeutral、一个深 Docking Module、Split/Tab/Dock、持久化、全生产 panel 和真实窗口；浮动原生窗口与项目 AUI Theme 保持 deferred。

### 19.2 是否错误重建 248

没有。直接复用 WidgetTree、single geometry、pick、focus、DPI 和 actual RGBA；只为新增 Dock geometry 跑代表矩阵，不重复完整 248 资格。

### 19.3 是否会形成第二套布局真相

不会。D3 明确要求生产 `EditorLayout::resolve` 退出，旧 `DockLayoutManager` 被迁移、替换或删除；完成标准不允许 compatibility owner 长期并存。

### 19.4 依赖方向是否可行

通过。核心类型和 snapshot 落在 renderer 可拥有的 seam，window 只持有 Module 并提供 input/persistence adapter，不建立 renderer 到 window 的反向依赖。

### 19.5 是否把 Theme 变成第二个公共 Module

没有。D1 只实现 renderer 内不可变主题值，没有 session、command、配置生命周期或项目覆盖。

### 19.6 是否限制 AI 或制造工具外菜谱

没有。Gate 顺序只约束源码施工依赖；产品 Interface 只暴露 UI-local intent 和 snapshot，不规定 AI 制作游戏的工具顺序。

### 19.7 测试是否过度

已删减。开发使用 owner filter；共享 Interface 完成后运行两个直接受影响 crate；真实窗口只在 D6；workspace/all-features、project_e2e_gate 和 Local CI 不默认执行。

### 19.8 是否能捕获真实失败

通过。D0 捕获 invariant/theme 缺口；D3 捕获生产 cutover 与 input；D4 捕获原子 drop；D5 捕获 persistence 负向；D6 捕获真实 OS drag、DPI 和像素。

### 19.9 是否遵守三小时规则

通过。文档将施工拆成四个可停止窗口，2:45 不启动新阶段，3:00 强制停止；没有要求单次连续授权完成全部 257。

### 19.10 是否保护用户和外部状态

通过。用户 Editor/config 不作为测试对象；store/fixture/evidence/temp 均要求 role-owned；真实输入绑定本次 PID/HWND；清理遵守 strict-child 合同。

### 19.11 是否可立即施工

当前不可继续 257 完成归档。本文件仍在 `当前/`；窗口 D 的 D6/D7 已实施并通过，后续单次
exact-commit Local CI 授权也已消费，但因 runner 复用跨 run build target 触发 rustc ICE 而失败。
须先在独立施工中修复 runner freshness 合同，257 不得在本授权内重跑 Local CI。

### 19.12 自审结论

```text
正式方案一致性：通过
范围与非目标：通过
依赖方向：通过
单一 owner：通过
失败可诊断性：通过
验证去重：通过
外部状态保护：通过
三小时上限：通过
当前状态：窗口 A-D 已实施；D6/D7 证据通过；Local CI runner failure，257 未完成
```

## 20. 下一步

### 20.1 窗口 A 施工记录

```text
施工开始：2026-07-26T18:22:56.2531062+08:00
施工收口：2026-07-26T18:42:48.5204663+08:00
基线 source：a94874ef26cf2d717b7627d77281f85f2bb9566d
基线 tree：1098c8e11d7e6c0fdcef7a1cbce7ef8e914eb681
D0 theme red：panel-local UiColor::rgba count 8，期望 0
D0 workspace red：duplicate panel、empty stack、invalid active、invalid ratio 被旧空验证接受
D1：EditorTheme::DARK_NEUTRAL 与语义 surface/border/text/selection/status/overlay token 已实现
D2：typed LayoutNodeId/PanelId、manifest-owned dockable membership、Split/Stack invariant、
    default/fallback、minimum-size resolve、snapshot/splitter/diagnostic/drag-preview contract 已实现
消费者 fixture remediation：AI Panel containment 测试由共享颜色定位改为稳定 WidgetId 定位
最终 owner/consumer：cargo test -p editor_ui_renderer --locked，39 passed / 0 failed
直接编译消费者：cargo check -p editor_window_winit --locked passed（仅既有 dead-code warnings）
格式：cargo fmt --all -- --check passed
外部状态：未启动真实 Editor、未修改用户配置、无 owned process/temp 需要清理
未运行：editor_window_winit 行为测试、真实窗口、Local CI；原因是生产 cutover 属于 D3，release authority 属于 D6/D7
```

窗口 A 结束条件成立：

```text
D0 red 已由 D1/D2 owner 实现转绿；
theme/tree core 只依赖 editor_ui_renderer；
生产 EditorLayout::resolve 与旧 DockLayoutManager 尚未退出，未冒充 D3 完成；
未运行真实窗口或全 workspace 验证。
```

### 20.2 窗口 B 施工记录

```text
施工开始：2026-07-26T19:00:43.4819547+08:00
施工收口：2026-07-26T19:30:29.4054695+08:00
耗时：00:29:45.9235148，未触及三小时硬停止
基线 source：a94874ef26cf2d717b7627d77281f85f2bb9566d
基线 tree：1098c8e11d7e6c0fdcef7a1cbce7ef8e914eb681
D3 valid red：workspace_splitter_production_tree_exposes_stable_resize_widgets，实际 0、期望 3
owner：workspace_docking 7 passed / 0 failed
renderer consumer：cargo test -p editor_ui_renderer --locked，41 passed / 0 failed
window focused：workspace_splitter 4 passed；focus filter 11 passed；retained_widget filter 2 passed
首次完整 window regression：120 passed / 3 failed
首次失败根因：7px Splitter hit band 跨边界覆盖 bottom Stack 控件 3.5px，
    导致 dock tab、AI prompt、Asset Browser search 三项既有交互被截获
修复：Splitter hit band 完全归第一侧所有，维持 7px 命中宽度且不覆盖第二侧控件
最终完整 window regression：cargo test -p editor_window_winit --locked，123 passed / 0 failed
格式：cargo fmt --all -- --check passed
静态所有权：EditorLayout::resolve、compute_fixed_rects、DockLayoutManager、crate::dock 均无残留；
    PanelRegistry 只存在于 editor_ui_renderer/src/workspace_docking.rs
补丁卫生：git diff --check passed（仅工作区既有 LF/CRLF 提示）
旧 owner：editor_window_winit/src/dock.rs 已删除，生产应用只持有 EditorWorkspaceDockingModule
范围：未实现 D4 跨 Stack 拖放、D5 persistence、浮动窗口、多显示器或项目 AUI Theme
外部状态：未启动真实 OS 窗口，未运行 Local CI
```

窗口 B 结束条件成立：

```text
生产 panel geometry 已由 WorkspaceSnapshot 唯一提供；
Splitter 通过 retained WidgetTree pick/capture 接收 headless application input；
PointerUp 提交，Escape/FocusLost 回滚，窗口 resize 保留 ratio；
resize 不修改业务 EditorUiModel revision；
旧固定布局 owner 已退出，owner 与完整窗口回归均通过。
```

### 20.3 窗口 C 施工记录

```text
施工开始：2026-07-26T19:36:40.7548441+08:00
施工收口：2026-07-26T20:17:55.8446277+08:00
耗时：00:41:15.0897836，未触及三小时硬停止
基线 source：a94874ef26cf2d717b7627d77281f85f2bb9566d
基线 tree：1098c8e11d7e6c0fdcef7a1cbce7ef8e914eb681
D4 owner red：BeginPanelDrag / UpdatePanelDrag / CommitPanelDrag 与 DockDropZone 缺失，5 个编译错误
D4 consumer red：production retained DockTab PointerDown 后 active_panel_drag_id 实际 None、期望 ai_panel
D5 owner red：ClosePanel / ShowPanel intent 缺失，3 个编译错误
D5 store red：WorkspaceLayoutStore 缺失，3 个编译错误
D4 owner：workspace_docking 12 passed / 0 failed
D4 renderer：cargo test -p editor_ui_renderer --locked，46 passed / 0 failed
D4 window tab：workspace_tab_drag 2 passed / 0 failed
D4 asset owner：editor_input asset_browser_drag 2 passed / 0 failed
D4 viewport consumer：viewport_runtime 4 passed / 0 failed
D5 schema：workspace_layout_schema 2 passed / 0 failed
D5 persistence：workspace_persistence 4 passed / 0 failed
D5 visibility/menu：workspace_panel_visibility 1 passed / 0 failed
既有 DockTab 点击兼容：tests::native_app::dock_tab_click_changes_only_editor_dock_state_and_rebuilds_visible_panel passed
production 默认组合：cargo check -p editor_window_winit --locked passed（仅既有 dead-code warnings）
格式：cargo fmt --all -- --check passed
补丁卫生：git diff --check passed（仅工作区既有 LF/CRLF 提示）
命令修订：施工文档原 editor_window_winit asset_drag 过滤器运行 0 项；
    已改为真实 owner editor_input asset_browser_drag 并执行 2 项
无效证据：三次短测试名 + --exact 各运行 0 项，均未计为通过，随后均以完整路径/真实过滤器重跑
外部状态：所有 persistence 测试只使用唯一 TEMP 子目录并清理成功；
    未读取或写入用户 Editor store，未修改项目 root、项目 digest 或业务 EditorUiModel revision
范围：未实现 D6 真实 OS drag/DPI/像素验收，未执行 D7 最终受影响域回归或 Local CI
```

窗口 C 结束条件成立：

```text
每个 snapshot Stack 生产生成稳定 Tab Widget，active panel 内容跟随 Stack geometry；
Armed/Dragging、统一 target resolver、preview/commit、四边/center drop 与原子 cleanup 已闭环；
Escape/FocusLost/capture release 收敛，Dock/Asset/Viewport owner 定向证据通过；
close/show/reset 与 EditorWorkspaceLayout.v1 用户状态生命周期已闭环；
restore 覆盖 missing、malformed、wrong schema、duplicate、unknown/removed/new panel、ratio clamp、
write failure、partial temp 与 corrupt reset；失败不阻止 Editor；
store 只持久化结构、ratio、tabs、active、closed panels，不持久化项目或业务状态。
```

### 20.4 窗口 D pre-Local-CI 施工记录

```text
施工开始：2026-07-26T20:27:15.6316168+08:00
三小时硬停止：2026-07-26T23:27:15.6316168+08:00
基线 source：a94874ef26cf2d717b7627d77281f85f2bb9566d
D6 deterministic：workspace_dpi_matrix 2 passed / 0 failed；覆盖 1280x720@1.0、1600x900@1.5、
    1920x1080@2.0 与 640x360 minimum pressure；生产 dockable manifest 全覆盖
D6 authority root：G:\EngineTest\257-window-d-20260726-202715
有效 splitter：257-splitter-resize-r3，1280x720@1.25，Vulkan，OS Down/Move/Up，
    foreground/PID passed，layout revision 1->2，actual RGBA sha256:
    6a7cb3cc17c3f9afb0c9437b0415e75e2f2c86fb7c49cb881d34a7b79c3c06af
有效 Tab dock：257-tab-dock-r1，1280x720@1.25，Vulkan，preview=true，
    layout revision 1->3，actual RGBA sha256:
    6ff48fb2ffe2b3e800c2f705a30b9120e476c79bd92ad826b532ba1c9b3d0d17
无效 authority 证据：首次 fixture copy 使用 -LiteralPath 通配符导致空 fixture；
    第二/三次 splitter 证明 OS Down/Up 但未产生独立 Move；均保留且未计为通过
D6 owner remediation：现有 editor_ui_authority 窄扩展为两阶段真实 drag；显式 test-owned
    workspace store；AI Panel 在 256px Stack 中改为 responsive，避免 prompt/submit 重叠
D7 首次窗口回归：renderer 47/47；window 131/132，失败为旧 activate_bottom_panel helper
    只发送 PointerDown，D4 Armed drag 后 Escape 被 workspace lifecycle 消费
D7 fixture remediation：helper 补真实 PointerUp；单点通过；最终 window 132/132
D7 最终：renderer 47/47，editor_input 32/32，window 132/132，all-features check passed，
    scoped --no-deps Clippy -D warnings passed，fmt/diff hygiene passed
production composition：当前用户 editor_host PID 4272 锁定默认 target executable；
    未终止用户进程，改用 role-owned CARGO_TARGET_DIR，editor_host real-window +
    real-wgpu-surface build passed
外部状态：authority owned processes 全部退出；未读取或写入用户 workspace store；
    用户 editor_host 未被操作；evidence 保留，role-owned temp 为空
首次 Local CI candidate：ce2eb3a90282e3e128fbd04c88deaf61a11d9f2f，
    run local-ce2eb3a90282-1785071447，source/isolated clean，cleanup removed；
    fail-fast 首因是 lint ledger 中 0050/0056/0065/0128/0138/0140 已不再发生，
    quality gate 要求删除 resolved entries
ledger owner remediation：精确删除上述 6 项；lint ledger 定向验证 4/4 passed，
    observed 359、known 161、new/resolved/stale/new suppressions 均为 0
环境诊断：source workspace 默认 target 复现因用户 editor_host PID 4272 持有
    rust\target\debug\editor_host.exe 而返回 os error 5；该诊断不是权威 Local CI 失败。
    LocalCiRunner 显式把 isolated verify 的 CARGO_TARGET_DIR 绑定到
    rust\target\quality-gate\local-ci-build，因此 fresh Local CI 不需要也不得终止用户 Editor
runner incident：首次后台控制误判造成两棵 task-owned Local CI 并发；一份留下空 artifact，
    一份 local-6fbca7d2d9f7-1785073520 以 exit 101/report missing 失败。两份均不计为
    qualification；所有 owned descendants 已终止，注册 worktree 已由 git worktree remove 清理
fresh Local CI：candidate 6fbca7d2d9f711f7c1ba7c2ac6fa8011888831d7，
    run local-6fbca7d2d9f7-1785073705，source/isolated clean，cleanup removed；
    toolchain/format/architecture/ledger-Clippy passed，default_workspace failed
default workspace 首因：editor_core 546 passed / 10 failed / 2 ignored；10 项共同由 Windows
    TEMP 的 8.3、长路径与 \\?\ verbatim 表示不一致触发，导致 fixture project root 与
    Active Scene path 被误判为不同 root；257 未修改 editor_core，此项分类为 pre-existing
    test fixture ownership defect
fixture owner remediation：只修改 editor_core tests/fixtures.rs 与 asset_browser.rs 测试
    helper；先规范化已存在 TEMP 根，再移除 Windows verbatim display prefix，不放宽生产
    path_outside_project 合同
owner regression：editor_core --lib 556 passed / 0 failed / 2 ignored；原 10 项与
    scene_save/project_patch containment negative tests 均通过
second fresh Local CI：candidate d888bb990cfbd8e0f0deab97e27707ad12be9e78，
    run local-d888bb990cfb-1785075713，source/isolated clean，cleanup removed；
    toolchain/format/architecture/ledger-Clippy/default workspace passed，all-features failed
all-features 首因：editor_window_winit 142 passed / 1 failed / 9 ignored；唯一失败
    real_window_feature_has_native_project_folder_dialog_backend 仍断言已由 D3 删除的
    project_dock，而 production panel manifest 与后续覆盖测试使用 asset_browser
all-features owner remediation：只把该陈旧测试断言同步为 asset_browser；不修改生产实现
all-features owner regression：完整限定名定向测试 1 passed / 0 failed；首次少模块前缀的
    exact filter 运行 0 tests，未计为通过
窗口收口：已接近 2:45 禁止新阶段点；不在窗口 D 再启动 fresh Local CI，不归档 257
cleanup：fixture/state-store/state-store-tab/temp/build-target 均通过 strict-child 与 reparse
    preflight，且无 owned descendant；宿主策略拒绝单目标原生 Remove-Item -Recurse -Force 后，
    根据后续授权在同一 PowerShell 中使用受控 [System.IO.Directory]::Delete literal fallback；
    五项已删除并验证不存在，evidence 与诊断日志按合同保留
```

### 20.5 精确下一步

已形成 clean exact commit `8b56445e6e96aed0d180c127f525324e2b5a5422`。后续单次 fresh
Local CI 授权已经消费且失败；不得把 257 标记完成或归档。先单独修复 Local CI runner 的
per-run build target 隔离合同并做 owner 级回归，再由新的明确授权决定是否对新的 clean exact
commit 执行 fresh Local CI。通过后才能生成完成记录、同步最终入口并归档本施工文档。

### 20.6 exact-commit Local CI 资格验证记录

```text
授权窗口开始：2026-07-26T23:26:03.7305235+08:00
授权对象：8b56445e6e96aed0d180c127f525324e2b5a5422
source tree：31c5417af2fbc4f541ea1dffa2980280530359c8
启动前状态：source workspace clean
执行次数：1；未重跑
run id：local-8b56445e6e96-1785079676
local report：rust\target\quality-gate\local-ci\local-8b56445e6e96-1785079676\
    local-ci-run-report.v1.json
quality report：rust\target\quality-gate\local-ci\local-8b56445e6e96-1785079676\
    quality-gate-report.v2.json
quality report digest：sha256:b7abb8b2ba2f17cecce403f0c9854c42f11a91a87cf8d1286c80271ed755c5dc
结果：failed；duration_ms 791773；source/isolated workspace clean；cleanup removed
已通过阶段：toolchain、workspace metadata、architecture、format、source suppression、Clippy
失败阶段：default_workspace；exit code 101；duration_ms 658821
直接首因：rustc 1.96.0 编译 ai_tool_gateway test explicit_reconnect 时发生 ICE：
    internal compiler error: no resolution for an import
排除项：不是链接器 OOM、磁盘空间、文件锁、普通编译错误或 257 行为测试失败
runner freshness 缺口：LocalCiRunner 每次创建独立 Git worktree，但所有 run 复用
    rust\target\quality-gate\local-ci-build；workspace tests 又复用其 workspace-tests-target
owner diagnosis：在新的 role-owned CARGO_TARGET_DIR
    G:\EngineTest\257-window-d-20260726-202715\explicit-reconnect-clean-target-2
    执行 cargo test -p ai_tool_gateway --test explicit_reconnect --no-run --locked；
    84.8 秒通过，证明 exact source 可编译，ICE 由共享增量构建状态触发
diagnosis cleanup：两个 role-owned clean target 通过 strict-child、reparse 与 owned-process
    preflight 后使用受控 Directory.Delete fallback 删除，并验证不存在
收口：本次授权已消费；未生成完成记录，未同步完成入口，未归档 257，未重跑 Local CI
```

### 20.7 Local CI runner per-run build target remediation

```text
授权窗口开始：2026-07-27T00:08:32.4163390+08:00
三小时硬停止：2026-07-27T03:08:32.4163390+08:00
基线 source：648bbc3ba0e14d83024321ff7cc339d403791e79
owner：rust/crates/quality_gate/src/local_ci.rs
红测试：local_ci_run_ids_are_unique_within_the_same_second 与
    local_ci_build_targets_are_isolated_by_run_id 在生产 helper 缺失时编译失败，证据有效
实现：run id 增加 process/sequence 唯一身份；CARGO_TARGET_DIR 改为
    target/quality-gate/local-ci-build/<run-id>；每个 run 不再复用其它 run 的增量状态
cleanup：run-owned build target 与临时 Git worktree 一并纳入 cleanup_status；
    任一删除失败均 fail-closed，并输出独立 actionable diagnostic
owner regression：quality_gate unit 50/50；integration 15/15；doc tests 通过
quality：cargo fmt --all -- --check passed；
    cargo clippy -p quality_gate --all-targets --locked -- -D warnings passed
外部状态：测试只使用 G:\EngineTest\257-local-ci-runner-20260727-000832 role-owned target；
    未操作用户 Editor、项目 fixture 或既有 Local CI evidence
下一步：形成 clean exact commit，执行新的 fresh fail-fast Local CI
```

### 20.8 最终 exact-commit Local CI 与归档结论

```text
qualification commit：6fac851b0b6f76837753c09ad6f6eebb0012cfa1
qualification tree：38d1ac9aed8ed4daf617d6e50f3c3dc21743bfd8
run id：local-6fac851b0b6f-1785082542-25324-0
report mode：trace
fail fast：true
stages：12/12 passed
CI status：observed_commit_passed
source workspace：clean
isolated workspace：clean
diagnostics：0
cleanup：removed；临时 Git worktree 与 run-owned build target 均已删除
remediation role root：G:\EngineTest\257-local-ci-runner-20260727-000832 已通过
    strict-child/reparse/owned-process preflight 删除并验证不存在
duration_ms：2125487
quality report sha256：
    bd2c78fbcdf49747b0b093c3210c4495ee0785fa8b0219d26c0694d39c970e14
结论：D0-D7 与 257 验收声明全部成立；允许生成完成记录、同步入口并归档
```
