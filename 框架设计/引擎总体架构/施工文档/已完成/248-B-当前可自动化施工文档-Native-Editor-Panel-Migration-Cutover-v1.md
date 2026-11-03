# 248-B-已完成自动化施工文档：Native Editor Panel Migration / Cutover v1

> 状态：已完成 Gate B0-B7、权威 QualityGate 与 exact-commit Local CI；已归档。  
> 正式方案：`../../248-Native-Editor-UI-Interaction-Reachability-Layout-Convergence-v1方案.md`。  
> 串行位置：248-A 完成后才能激活；248-C 在本文完成后才能激活。  
> 激活证据：248-A exact commit `10cb163d557328373b080cb5261931f245148830` 的 Local CI passed；当前槽位已释放；A 的 `widget_tree/reconcile/layout/pick/extract` API、30 个生产手写 hit push、`layout_for()` 和输入特判已复核；本文将移动到 `当前/` 并同步 54。  
> 完成证据：候选 commit `49fcf497bbd98f9b4952c1c659c28cd7393a0d88`；完成记录见 `../../阶段完成记录/2026-07-13-Native-Editor-Panel-Migration-Cutover-v1/00-总览.md`。

## 1. 施工目标

把全部当前可见 Native Editor 面板迁移到 248-A 的 EditorWidgetTree，并完成唯一真相切换：

```text
EditorUiModel + DockLayout configuration
  -> PanelRegistry widget subtree factories
  -> keyed WidgetTree reconcile
  -> single layout / clip / pick / focus path
  -> UiDrawList
  -> WGPU
```

终态必须删除生产路径中的重复 `layout_for()`、面板手写 HitRegion 和按 panel/field 硬编码的输入特判。248-B 不负责最终 DPI 实机与像素权威报告，那属于 248-C。

## 2. 必须迁移的生产表面

```text
Project Launcher
Main Menu / Toolbar
Hierarchy
Game / Scene Viewport shell
Inspector
Bottom Dock Tabs
Asset / Project Browser
Workspace / Workflow Rail
Input Mapping Authoring
Build / Export
AI Panel
Console
Runtime Trace
Report Panel entry surface
```

任何仍对用户可见的面板都不能被遗漏后继续手写最终 HitRegion。

## 3. 预计涉及文件

激活时按 248-A 实际落点复核：

```text
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_ui_renderer/src/layout.rs
rust/crates/editor_ui_renderer/src/draw_list.rs
rust/crates/editor_ui_renderer/src/panels/*.rs
rust/crates/editor_ui_renderer/src/widget_*.rs 或 A 的实际模块
rust/crates/editor_ui_renderer/src/tests.rs / tests/*
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/focus.rs
rust/crates/editor_window_winit/src/dock.rs
rust/crates/editor_window_winit/src/input_route.rs
rust/crates/editor_window_winit/src/tests/*
rust/crates/editor_input/src/lib.rs
rust/crates/editor_ui_model/src/layout.rs
rust/crates/editor_ui_model/src/model.rs
rust/crates/editor_ui_model/src/panel-specific models（仅在缺少结构化状态时）
```

禁止为了 Widget 迁移把业务状态从 editor_core/editor_ui_model 搬进 renderer。

## 4. 激活前基线

1. 阅读 248-A 完成记录和 API/遗留清单。
2. 确认 derived HitRegion 仍是兼容 DTO，不是新 authoring API。
3. 清点所有 `DrawCommand`、`hit_regions.push`、`hit_region()`、`command_hit_region()`、`layout_for()` consumer。
4. 清点 `application.rs` 中 Asset Browser、AI Prompt、Inspector Field 等特殊输入分支。
5. 生成“可见控件 -> WidgetId/Role/Activation/State owner”迁移矩阵。
6. 使用独立 clean worktree、exact commit 和隔离 `CARGO_TARGET_DIR`。

预检：

```powershell
rg -n "hit_regions\.push|hit_region\(|command_hit_region\(|layout_for\(" crates/editor_ui_renderer/src crates/editor_window_winit/src
cargo test -p editor_ui_renderer --locked
cargo test -p editor_input --locked
cargo test -p editor_window_winit --locked
cargo check -p editor_window_winit --all-features --locked
```

## 5. Gate B0 - Panel Migration Manifest / Cutover Guard

### 任务

1. 建立版本化测试 fixture 或 Rust manifest，列出所有生产 panel_id 和 subtree root WidgetId。
2. 每个面板声明 state owner、scroll owner、command source 和 required Widget roles。
3. 增加 guard：生产 panel module 禁止直接 push HitRegion。
4. 增加 guard：生产 layout 不再允许第二套 computed panel rect。
5. 明确迁移期间的 feature/cutover 策略：同一运行实例只能选择一个生产 UI authoring path，禁止叠加绘制和双重命中。

### 定向测试

```powershell
cargo test -p editor_ui_renderer panel_manifest --locked
cargo test -p editor_ui_renderer no_manual_hit_region --locked
cargo test -p editor_window_winit panel_registry --locked
```

## 6. Gate B1 - Shell / Dock / Menu / Toolbar / Bottom Tabs

### 任务

1. DockLayoutManager 的 persisted split/stack/tab 配置生成 Widget subtree。
2. 删除 renderer 内重复 panel rect 计算入口；computed geometry 只来自 WidgetTree。
3. Main Menu 变为真实 Menu/MenuItem；暂不支持的项不得伪装成可点击文字。
4. Toolbar Button 使用 command binding、availability、tooltip 和状态 visual。
5. Bottom Project/Console/RuntimeTrace/Report 变为真实 Tab；inactive subtree collapsed。
6. 窄窗口下 Toolbar 使用明确 overflow/menu，不画出 surface。
7. Splitter resize 使用 UI-local action，并保留 Dock state owner。

### 定向测试

```powershell
cargo test -p editor_ui_renderer shell_widget --locked
cargo test -p editor_ui_renderer dock_tab --locked
cargo test -p editor_window_winit toolbar --locked
cargo test -p editor_window_winit dock_layout --locked
```

## 7. Gate B2 - Hierarchy / Inspector / Viewport

### 任务

1. Hierarchy 使用 Tree/Row/ScrollView，stable entity ID，动态 reorder 不丢选择/展开状态。
2. 移除 `.take(n)`、超界继续命中和无滚动列表行为。
3. Inspector 使用 ScrollView 和字段 role；字段 draft 继续归 PropertyEditBuffer。
4. Asset picker、toggle、number/text/value field 使用结构化 activation。
5. ViewportTextureSlot 作为 Widget role，Scene/AUI/Runtime pick 与 Editor chrome pick 维持明确优先级。
6. Hierarchy/Scene/Inspector selection 仍走已有 UiCommand/EditorSession。
7. Runtime Inspector 临时编辑、Apply/Discard 合同不得回归。

### 定向测试

```powershell
cargo test -p editor_ui_renderer hierarchy --locked
cargo test -p editor_ui_renderer inspector --locked
cargo test -p editor_window_winit viewport_runtime --locked
cargo test -p editor_window_winit inspector --locked
cargo test -p editor_core property_edit --locked
```

## 8. Gate B3 - Asset Browser / Workflow / Input Mapping

### 任务

1. Asset Browser list/grid/folder/preview/toolbar/search/picker 迁移到 Widget subtree。
2. Scroll offset 只能有一个 owner；迁移后归 Widget node-local state。现有 `AssetBrowserUiState.scroll_offset` / model 投影必须删除、只读兼容或明确停止参与生产滚动，不能同时由 Widget 和 editor_core 可写。
3. 现有 drag/drop 使用 pointer capture + WidgetPath target，不改变资产业务命令合同。
4. Workflow Rail 的步骤、primary command、recommended task 可滚动且不会重叠。
5. Input Mapping contexts/actions/bindings/properties 和 capture 状态迁移。
6. Search/prompt/input field 通过 focus/text target 解析，减少 `application.rs` panel 字符串特判。

### 定向测试

```powershell
cargo test -p editor_ui_renderer asset_browser --locked
cargo test -p editor_input asset_drag --locked
cargo test -p editor_window_winit asset_browser --locked
cargo test -p editor_window_winit input_mapping --locked
cargo test -p editor_core asset_browser --locked
cargo test -p editor_core input_mapping --locked
```

## 9. Gate B4 - Build / AI / Console / Runtime Trace / Report

### 任务

1. Build/Export command、profile、progress、failed/next action 迁移并可见。
2. AI prompt、submit/cancel、proposal accept/reject 使用 Widget focus/action/command。
3. Console、Runtime Trace 和 Report 使用真实 Tab + ScrollView，不再同时叠画。
4. Report Panel 继续使用已注册 provider，不新建报告总线。
5. Busy 控件阻止重复命令；disabled/failed 状态在 Tooltip/Status feedback 可见。
6. 长文本裁剪、换行/省略策略不能越过父 clip。

### 定向测试

```powershell
cargo test -p editor_ui_renderer build_export --locked
cargo test -p editor_ui_renderer ai_panel --locked
cargo test -p editor_ui_renderer console --locked
cargo test -p editor_window_winit ai_panel --locked
cargo test -p editor_core report_panel --locked
```

## 10. Gate B5 - Focus / Scroll / Popup / Modal / Text Target Convergence

### 任务

1. FocusInputSystem 全部引用 WidgetId，清理旧 FocusTarget 字符串分支。
2. Tab/Shift+Tab 只遍历 visible/focusable/modal-scope 节点。
3. Wheel 从 target 沿 WidgetPath 找最近可滚动祖先。
4. Popup/Menu/Modal 使用 Overlay 和 barrier，点击外部关闭但不误触下层。
5. Pointer capture 支撑 drag/drop 和 splitter。
6. TextInput/IME target 进入现有 draft owner；Widget 不复制 draft。
7. Window focus lost 清理 pressed/capture，不产生悬挂动作。

### 定向测试

```powershell
cargo test -p editor_window_winit focus --locked
cargo test -p editor_window_winit scroll --locked
cargo test -p editor_window_winit modal --locked
cargo test -p editor_window_winit text_input --locked
cargo test -p editor_input --locked
```

## 11. Gate B6 - Cutover / Legacy Removal

### 任务

1. 所有生产面板由 WidgetTree 生成。
2. 删除 `layout_for()` 或使其完全退出生产/测试真相；不保留同义改名函数。
3. 删除 panels 内手写 HitRegion helper 使用和新增入口。
4. InputRouter 使用 WidgetPath/activation；derived HitRegion 仅保留历史测试/报告兼容，并标注 deprecated boundary。
5. 删除已经被 Widget action/path 替代的 application panel 特判。
6. 全仓搜索重复 layout/hit/focus truth；第二个 consumer 出现时收敛共享 API。
7. A 的 legacy manual-hit inventory 必须清零；不能通过更新 allowlist 把旧入口延期。
8. Runtime/项目 AUI 依赖图保持不变。

### 负向检查

```powershell
rg -n "hit_regions\.push|hit_region\(|command_hit_region\(" crates/editor_ui_renderer/src/panels
rg -n "layout_for\(" crates/editor_ui_renderer/src crates/editor_window_winit/src
rg -n "focused_text_field_id\(\).*asset_browser|focused_text_field_id\(\).*ai_panel" crates/editor_window_winit/src
```

期望生产 panel 搜索为零；必要兼容 fixture 必须位于明确 tests/adapter 模块。

### 定向测试

```powershell
cargo test -p editor_ui_renderer cutover --locked
cargo test -p editor_window_winit retained_widget --locked
cargo test -p editor_input --locked
```

## 12. Gate B7 - 受影响域与权威回归

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
cargo clippy -p editor_ui_model -p editor_ui_renderer -p editor_input -p editor_window_winit -p editor_wgpu_renderer --all-targets --all-features --locked -- -D warnings
```

候选冻结后：

```powershell
cargo test --workspace --locked
cargo test --workspace --all-features --locked
cargo run -p quality_gate --locked -- verify --report-mode summary
cargo run -p quality_gate --locked -- local-ci --commit HEAD
```

248-B 不以 248-C 尚未实现的 actual pixel/DPI 实机矩阵作为本包通过条件，但必须保证 real-window/all-features 编译及现有 interaction/visual regression 不退化。

## 13. 完成与归档

```text
生成 248-B 阶段完成记录和面板迁移矩阵。
记录删除的重复 layout/manual hit/special input 入口。
记录 derived HitRegion 剩余 consumer 和淘汰策略。
记录各 Gate、权威 Local CI run/digest/耗时。
归档 248-B，更新 54/施工 README/阶段完成记录 README。
248-C 仍需激活复核；247 P0-1 仍不得标记完成。
```

## 14. 回滚规则

迁移按面板 Gate 保持可编译；失败时回滚当前 Gate 的 Widget subtree factory，不回滚已通过的 retained core。禁止恢复新增手写 HitRegion 作为永久修复。不得覆盖用户无关改动或使用 destructive git 命令。

## 15. 施工文档自审

### 15.1 前置是否正确

通过。硬依赖 248-A 完成记录和实际 API；不能与 A 并行。

### 15.2 是否遗漏生产面板

没有。248 列出的全部当前可见面板均进入 B1-B4，B0 manifest 和 B6 搜索防止静默遗漏。

### 15.3 是否形成双轨

终态不会。B6 删除生产手写 HitRegion、重复 layout 和 panel 特判；derived HitRegion 只留兼容 DTO。

### 15.4 是否污染业务状态

不会。字段草稿、资产选择、AI prompt、build/profile 等保持原 owner；Widget 只持 UI-local state，业务动作仍走 UiCommand。

### 15.5 是否把 C 的验收偷换成 B 的完成

没有。B 只完成生产 UI cutover 和结构回归；真实 DPI/OS click/actual pixel 由 C 完成，P0-1 仍为 partial。

### 15.6 测试是否覆盖高风险链路

覆盖。Shell/Dock、Hierarchy/Inspector/Viewport、Asset/Workflow/Input、Build/AI/Report、Focus/Modal/Text、legacy removal 各有定向与负向检查，最终有 workspace/all-features/Local CI。

### 15.7 当前能否施工

不再施工。Gate B0-B7、权威回归、完成记录和归档均已完成；下一阶段必须单独激活 248-C。

## 16. 自审结论

```text
范围：通过。
前置与串行关系：通过。
迁移完整性：通过。
双轨删除条件：通过。
测试与回归：通过。
当前状态：已完成并归档；248-C 尚需激活复核。
```
