# 248-C-当前可自动化施工文档：Native Editor Real-window Reachability / DPI / Pixel Acceptance v1

> 状态：Gate C0-C6、真实 DPI 权威矩阵和 exact-commit Local CI 已完成；施工文档已归档。
> 正式方案：`../../248-Native-Editor-UI-Interaction-Reachability-Layout-Convergence-v1方案.md`。  
> 串行位置：248-A、248-B 完成后才能激活；这是 P0-1 的最终权威验收包。  
> 激活前置：A/B 已归档且生产 UI 已完成 WidgetTree cutover；真实 Windows 桌面、real-window/WGPU、目标 DPI 环境和截图权限通过预检；移动到 `当前/` 并同步 54。  
> 施工约束：仅按 Gate C0-C6 串行实施；不得用 deterministic scale matrix 冒充 C4 权威证据。
> 2026-07-13 激活复核：代码基线全部通过；当前为可交互 Windows Session 1。DisplayConfig 真实切换探测对 100%/150%/200% 返回 `set_rc=0`，OS 回读档位为 `cur=0/2/4`，结束后已恢复 125% 的 `cur=1`；三档真实环境可调度，激活前置满足。

## 1. 施工目标

证明 retained UI 不只是结构测试可用，而是在真实 Native Editor 窗口中可被人稳定操作：

```text
stable WidgetId
  -> logical geometry
  -> physical OS/window coordinates
  -> real pointer/key/wheel/focus path
  -> WidgetPath / action / command
  -> model/tree/draw update
  -> actual pixel capture
  -> EditorUiReachabilityReport
```

完成 1280x720、1600x900、1920x1080 与 100%、150%、200% DPI 的确定性矩阵，并在真实 Windows 环境取得 OS 报告 scale factor、真实窗口输入和实际 RGBA 像素证据。

## 2. 范围

### 2.1 必须实现

```text
EditorWidgetTreeSnapshot / EditorUiReachabilityReport v1。
Widget reachability、clip、command、disabled reason、focus route diagnostics。
三尺寸 x 三 scale factor 的 deterministic matrix。
真实 winit window + real WGPU surface present。
按 Widget geometry 计算物理坐标的受控 replay。
Windows 权威环境中的实际窗口焦点和真实 pointer/key/wheel 注入。
actual framebuffer/window RGBA capture，不是 metadata-only。
真实 DPI evidence 和 environment_blocked 合同。
主创作面板成功、禁用/失败、Resize/DPI/Scroll/Tab 场景。
像素与结构证据关联、artifact hash、Report Panel 注册。
最终 workspace/all-features/QualityGate/Local CI。
```

### 2.2 不做

```text
不重新设计 WidgetTree 或迁移面板。
不修改项目 AUI Runtime。
不把 OS 自动化放进正常 Editor 帧。
不要求 public CI 具备桌面会话或修改系统 DPI。
不使用截图 OCR 猜 command 成功。
不把单一 GPU 的严格逐像素 hash 作为跨机器唯一 blocker。
不做视觉换肤和动画回归。
```

## 3. 预计涉及文件

按 B 实际代码复核：

```text
rust/crates/editor_window_winit/src/real_window_interaction_gate.rs
rust/crates/editor_window_winit/src/visual_regression_gate.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/gate_report.rs
rust/crates/editor_window_winit/src/tests/real_window_interaction_gate.rs
rust/crates/editor_window_winit/src/tests/visual_regression_gate.rs
rust/crates/editor_window_winit/src/tests/native_real_window.rs
rust/crates/editor_window_winit/src/tests/*reachability*（新增或实际模块）
rust/crates/editor_ui_renderer/src/widget_*.rs
rust/crates/editor_ui_renderer/src/tests.rs / tests/*
rust/crates/editor_wgpu_renderer/src/real.rs 或实际 real-wgpu 模块
rust/crates/editor_wgpu_renderer/src/draw_plan.rs
rust/crates/editor_wgpu_renderer/src/*capture*（必要时新增）
rust/crates/editor_ui_model/src/report_panel.rs（只注册 provider/summary）
rust/crates/editor_core/src/report_panel.rs（仅现有注册链缺口）
target/editor-ui-evidence/（生成物，不提交为源码真相）
```

如果 Windows OS input/capture 需要新依赖，必须限制在 Editor/test/Windows feature，Runtime/Player 依赖图禁止出现。

## 4. 激活前环境预检

激活前必须证明：

```text
Windows 真实交互桌面会话可用，不是纯 service/session 0。
应用窗口可前台聚焦，安全策略允许受控输入。
real-window + real-wgpu-surface 初始化成功。
GPU 支持选定 capture 路径，或 Windows window capture adapter 可用。
至少存在或可调度 100%、150%、200% DPI 的真实 Windows 环境。
artifact 输出目录可写且与源码目录分离。
字体、GPU backend、OS build 和 scale factor 能写入 evidence。
```

缺少真实 DPI 环境时 248-C 可以生成/自审，但激活复核必须标记阻塞，不能先施工后把权威条件降级。

基线命令：

```powershell
cargo test -p editor_window_winit real_window_interaction --locked
cargo test -p editor_window_winit visual_regression --locked
cargo test -p editor_ui_renderer --locked
cargo check -p editor_window_winit --all-features --locked
cargo check -p editor_wgpu_renderer --all-features --locked
```

## 5. Gate C0 - Report Schema / Evidence Provenance

### 任务

1. 定义 `EditorWidgetTreeSnapshot` 和 `EditorUiReachabilityReport v1`。
2. 报告包含 scenario、viewport、f64 scale、Widget counts、reachable、duplicate、clip、command、reason、focus、diagnostics。
3. screenshot evidence 包含 kind、width、height、rgba hash、artifact path、backend、font、OS、GPU 和 frame ID。
4. `metadata-only`、`actual_offscreen_rgba`、`actual_window_rgba` 明确区分。
5. artifact 路径相对 evidence root，禁止泄漏用户绝对路径。
6. Report Panel 复用注册机制，仅显示 Summary/Trace。

### 定向测试

```powershell
cargo test -p editor_window_winit reachability_report --locked
cargo test -p editor_window_winit screenshot_evidence --locked
cargo test -p editor_core report_panel --locked
```

## 6. Gate C1 - Deterministic Size / Scale Matrix

### 任务

1. 建立 3 尺寸 x 1.0/1.5/2.0 共 9 个 deterministic scenarios。
2. 每个 scenario 验证 logical/physical round-trip、layout、clip、pick、glyph plan、Tooltip anchor 和 drag threshold。
3. 遍历 P0-1 面板 stable WidgetId，检查可见控件可达、disabled reason 完整。
4. 检查所有 geometry 位于合法 surface/clip，inactive/collapsed 不命中。
5. 检查最长真实标签/路径不会越过父 clip；必要时 wrap/ellipsis。
6. 输出结构 baseline，但禁止自动覆盖 baseline。

### 定向测试

```powershell
cargo test -p editor_ui_renderer reachability_matrix --locked
cargo test -p editor_window_winit dpi_matrix --locked
cargo test -p editor_window_winit size_matrix --locked
```

## 7. Gate C2 - Actual Pixel Capture

### 任务

1. 检查现有 real-window evidence：当前 metadata-only 必须升级。
2. 优先采用渲染到 capture-capable intermediate texture，再复制/呈现到 surface 的 WGPU 路线；如果 backend 证明不可用，使用受限 Windows window capture adapter。
3. map/readback 处理 bytes_per_row alignment、surface format、alpha、sRGB 和异步 completion。
4. 证明捕获 frame 与报告 frame/WidgetTree revision 相同。
5. 写 actual RGBA/PNG artifact 和 SHA-256；非空像素/关键区域 canvas check。
6. 跨 GPU baseline 使用结构 + 区域/容差证据；精确 hash 只在同环境 baseline 下比较。
7. 捕获失败返回 failed/environment_blocked，不能生成伪 hash。

### 定向测试

```powershell
cargo test -p editor_wgpu_renderer capture --all-features --locked
cargo test -p editor_window_winit actual_pixel --all-features --locked
cargo test -p editor_window_winit visual_regression --locked
```

## 8. Gate C3 - Real-window Coordinate Replay

### 任务

1. 真实窗口 frame 后按 WidgetId 取得 logical geometry。
2. 使用窗口位置、client origin 和 f64 scale factor 转成物理 screen/client 坐标。
3. 默认稳定 Gate 通过受控 winit window-event bridge 回放 pointer/key/wheel。
4. Windows 权威 Gate 聚焦目标窗口并使用受控 OS input adapter 发送 click/wheel/key；限定目标 PID/window handle、超时和自动退出。
5. 验证事件经过窗口层进入 WidgetPath，不能直接调用 handler。
6. 每步记录 before/after command、model/tree revision、focus、screenshot。
7. 输入注入失败、窗口非前台或坐标超界立即停止，不点击其它应用。

### 定向测试

```powershell
cargo test -p editor_window_winit controlled_replay --all-features --locked
cargo test -p editor_window_winit widget_coordinate_route --locked
```

Windows OS-level 场景必须是显式 local/authority command，不进入无桌面的 public CI。

## 9. Gate C4 - Real Windows DPI Authority

### 任务

1. 进程声明/验证 Per-Monitor DPI awareness，不依赖兼容虚拟化。
2. 在 100%、150%、200% 真实环境分别启动窗口并记录 OS/winit 实际 scale factor。
3. 每个环境完成至少一次 Button、ScrollView、TextField focus、Tab、Popup/Modal 场景。
4. 每个环境生成 actual window pixel artifact 和结构化 route evidence。
5. 150% 必须观察到约 1.5 的 f64 scale，不允许 round 成 1/2。
6. 环境不具备目标 scale 时结果为 environment_blocked，不得由 deterministic matrix 替代权威通过。

### 权威场景

```text
100% / 1280x720
150% / 1600x900
200% / 1920x1080
```

如有多显示器，还要验证窗口跨 monitor 后 ScaleFactorChanged 的 layout/pick/capture 一致性；没有多显示器时明确 not_evaluated，不虚构证据。

## 10. Gate C5 - P0-1 Human Workflow UI Matrix

每个面板至少验证：成功操作、disabled/busy/failed 反馈、Scroll/Tab/Resize/DPI 之一。

```text
Project Launcher：Create/Open/Recent 的可达和禁用原因。
Workspace：切换步骤并执行 primary command。
Hierarchy/Scene/Inspector：选择一致、滚动字段、属性编辑 focus。
Asset Browser：search、scroll、select、drag/drop、picker modal。
Prefab/Rule/Input/AUI entry：从可见入口进入正确工作区。
AI Panel：prompt focus、submit/cancel/busy、proposal action。
GameView：Play controls 与 editor/game focus 不泄漏。
Build/Export：profile、build/run、busy/failed feedback。
Report Panel：注册 report、tab、scroll、next action。
```

不要求 C 重新执行完整 P0-2 从空项目流程，但必须证明 P0-2 将依赖的全部编辑器 UI 入口可达。

### 定向测试

```powershell
cargo test -p editor_window_winit native_interaction_gate --locked
cargo test -p editor_window_winit real_window_interaction_gate --locked
cargo test -p editor_window_winit visual_regression_gate --locked
cargo test -p project_e2e_gate --lib --locked
```

## 11. Gate C6 - 权威回归与完成判断

预检：

```powershell
cargo fmt --all -- --check
cargo test -p editor_ui_model --locked
cargo test -p editor_ui_renderer --locked
cargo test -p editor_input --locked
cargo test -p editor_window_winit --locked
cargo test -p editor_wgpu_renderer --locked
cargo test -p editor_core --lib --locked
cargo test -p project_e2e_gate --lib --locked
cargo check -p editor_window_winit --all-features --locked
cargo clippy -p editor_ui_renderer -p editor_input -p editor_window_winit -p editor_wgpu_renderer --all-targets --all-features --locked -- -D warnings
```

候选冻结后：

```powershell
cargo test --workspace --locked
cargo test --workspace --all-features --locked
cargo run -p quality_gate --locked -- verify --report-mode summary
cargo run -p quality_gate --locked -- local-ci --commit HEAD
```

另外必须归档三档 Windows DPI 权威 evidence。Local CI 不具备真实桌面/DPI 时只负责代码权威回归，不能替代 C4。

## 12. Pixel Baseline 规则

```text
baseline 更新必须显式命令和独立 diff review。
失败时不自动覆盖 PNG/hash。
结构变化与像素变化分别报告。
同环境精确 hash；跨环境使用受控容差/区域比较。
字体、GPU、backend、OS、DPI 不匹配时不与错误 baseline 强比。
actual pixel artifact 必须与 scenario/frame/tree revision 绑定。
```

## 13. 安全与效率

```text
OS input/capture 只在显式 Editor test/authority mode。
只操作本进程目标窗口，验证 PID/handle/foreground/coordinate bounds。
超时自动释放按键/按钮、关闭窗口并恢复焦点。
正常 Editor 不生成全树 snapshot、PNG 或 reachability report。
Runtime/Player 不链接 OS automation、Widget report 或 capture adapter。
```

## 14. 完成与归档

只有 A/B/C 全部完成且 C4/C6 权威证据齐全后：

1. 生成 248-C 和 P0-1 总完成记录。
2. 记录 9 组 deterministic matrix 与 3 组真实 Windows DPI evidence。
3. 记录 actual pixel artifacts/hash、OS/GPU/font/backend。
4. 记录真实点击 route、失败收敛和权威 Local CI run/digest/耗时。
5. 更新 247 P0-1 讨论/施工完成状态及 49/54/施工 README/阶段记录 README。
6. 把 C 移入已完成；248 正式方案标记施工完成。
7. 下一个讨论系统才进入 247 P0-2。

任何真实 DPI 或 actual pixel 缺失时不得生成 P0-1 完成结论。

## 15. 回滚规则

```text
Report/capture 失败：保留 B 已完成 Widget UI，不恢复手写 HitRegion。
OS adapter 不稳定：隔离为 authority-only 并保留 controlled replay，但 P0-1 继续未完成。
pixel baseline 错误：删除 candidate artifact，不自动覆盖已批准 baseline。
不得回滚用户无关改动，不使用 destructive git 命令。
```

## 16. 施工文档自审

### 16.1 是否有真实证据而非 metadata

通过。C2/C4 明确要求 actual RGBA/window capture；metadata-only 只能是未评估，不能 passed。

### 16.2 deterministic DPI 是否冒充真实 DPI

不会。C1 是稳定矩阵，C4 是独立 Windows 权威 Gate；目标环境缺失会阻塞完成。

### 16.3 OS 自动化是否可能误点其它窗口

已约束。必须验证目标 PID/handle/foreground/坐标范围，失败立即停止并释放输入；只在显式 authority mode 执行。

### 16.4 是否重复 B 的面板迁移

没有。C 只消费已切换 Widget UI，新增报告、capture、replay 和验收；发现结构问题先回填 B/248，不在 C 建第二套控件。

### 16.5 是否影响 Runtime

不会。OS/capture/report 仅 Editor/test/Windows feature，依赖图 Gate 禁止进入 Player。

### 16.6 测试是否覆盖用户关心的编辑器 UI

覆盖。C5 包含 Launcher、Workspace、Hierarchy/Scene/Inspector、Asset、Prefab/Rule/Input/AUI 入口、AI、GameView、Build 和 Report。

### 16.7 当前能否施工

可以。248-A/B 已完成并归档，代码基线通过；2026-07-13 DisplayConfig 实机探测证明真实 100%/150%/200% 三档可切换并可由 OS 回读。本文已移动到 `当前/`，但最终完成仍必须取得 C4 三档真实窗口、输入和 actual pixel 证据。

## 17. 自审结论

```text
范围：通过。
前置与串行关系：通过。
真实像素证据：通过。
DPI 双层证据：通过。
OS 输入安全：通过。
Runtime 隔离：通过。
当前状态：已激活，可按 Gate C0-C6 串行施工。
```

## 18. 施工结果

248-C 已完成，候选 exact commit 为 `7607acecd4916cefdab69b5569c7b705f74a2308`。

```text
deterministic_matrix=3 sizes x 3 scale factors passed
real_windows_dpi=100% / 150% / 200% passed
authority_scenarios=15 passed / 15 total
actual_rgba_artifacts=15
unique_rgba_hashes=12
report_provider=editor.ui_reachability
report_schema=editor-ui-reachability-report.v1
new_suppressions=0
```

真实 Windows authority 场景在每档 DPI 覆盖 Hierarchy Button、Inspector editable TextField、Report Tab、Inspector Scroll wheel 和 Toolbar overflow Popup。全部场景验证目标 HWND/PID/foreground、真实 `SendInput` down/up 或 wheel、WidgetPath/command/focus，并生成 `frame.png` 与 `report.json`。

最终回归：

```text
workspace default：passed，472.9 s。
workspace all-features：passed，332.4 s。
QualityGate verify：passed，992479 ms；new_entries=0；new_suppressions=0。
Local CI run_id：local-7607acecd491-1783909733。
Local CI：passed，999416 ms。
Local CI quality report digest：sha256:4f806c1a04403439dac99ba8f3896694c572fc2d29f1b5a291ca7d8ececf4fc1。
source_workspace_state=clean
isolated_workspace_state=clean
cleanup_status=removed
```

第一次 QualityGate 预检发现 authority binary 的两个 `filter_map_bool_then` occurrence，并遇到一次 loopback provider 瞬时失败。lint 改为 `filter().map()` 后，loopback 定向连续三次通过；在新 exact commit 上重新执行完整 QualityGate 后 default/all-features/clippy/hygiene 全绿。权威 Local CI 只执行一次并通过。

完成记录：`../../阶段完成记录/2026-07-13-Native-Editor-RealWindow-Reachability-DPI-Pixel-Acceptance-v1/00-总览.md`。
