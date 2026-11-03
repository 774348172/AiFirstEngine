# 248-A-待执行自动化施工文档：Native Editor Retained Widget Core / Single Geometry v1

> 状态：已完成激活前复核并移入 `当前/`；248-A 是唯一已激活施工文档，可以施工。  
> 正式方案：`../../248-Native-Editor-UI-Interaction-Reachability-Layout-Convergence-v1方案.md`。  
> 串行位置：248-A / 248-B / 248-C 的第一份。  
> 激活证据：247、248 和 A/B/C 计划已进入 exact commit `b4db228`；原 `当前/` 为空；方案、代码、依赖、命令和前置状态复核通过。  
> 串行约束：施工期间 B/C 继续留在 `待执行/`，禁止并行实施。

## 1. 施工目标

建立 Editor-only retained core，但不迁移现有业务面板：

```text
EditorWidgetTree
  -> stable WidgetId / parent-child invariant
  -> keyed compose/reconcile
  -> Taffy logical layout
  -> effective clip / z-order
  -> pick / WidgetPath
  -> UiDrawList extract
  -> derived HitRegion compatibility snapshot
```

同时建立准确的 `f64 scale_factor` 与 logical/physical 坐标边界，为 248-B 面板迁移和 248-C 权威验收提供稳定底座。

248-A 完成后仍是中间状态，不代表 P0-1 完成；现有面板仍可暂时走旧路径，但禁止新增新的手写 HitRegion 扩展。

## 2. 范围

### 2.1 必须实现

```text
WidgetId / semantic path / role / node / tree schema。
EditorWidgetAction 与 EditorCommandBinding 的互斥 activation。
Dock、node-local、focus/input、business state 的类型边界。
keyed reconcile：reuse/create/remove/replace/reorder。
duplicate ID / orphan / cycle / incompatible role fail closed。
Taffy 最小 feature、layout adapter、text measure seam。
logical rect、effective clip、z-order、visibility。
top-most pick、disabled target、minimal WidgetPath route。
DrawList extract 和 derived HitRegionSnapshot。
WGPU clip/scissor 输出合同。
f64 scale factor 和 physical/logical round-trip。
synthetic retained-tree integration fixture。
结构化 diagnostics 和 core 性能 evidence。
```

### 2.2 明确不做

```text
不迁移 launcher/toolbar/hierarchy/inspector/asset/build/AI 等生产面板。
不删除旧 layout_for 和旧 panel HitRegion。
不实现完整 Menu、Modal、Dock drag 或复杂文本编辑器。
不生成 EditorUiReachabilityReport 权威报告。
不实现真实 OS SendInput 或真实 DPI 环境验收。
不做 actual window pixel capture。
不新增 editor_widget_tree crate。
不进入 Runtime Player 或项目 AUI。
```

## 3. 预计涉及文件

激活前必须按当前代码复核；建议写集为：

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/editor_ui_renderer/Cargo.toml
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_ui_renderer/src/draw_list.rs
rust/crates/editor_ui_renderer/src/widget_tree.rs（新增）
rust/crates/editor_ui_renderer/src/widget_reconcile.rs（新增）
rust/crates/editor_ui_renderer/src/widget_layout.rs（新增）
rust/crates/editor_ui_renderer/src/widget_pick.rs（新增）
rust/crates/editor_ui_renderer/src/widget_extract.rs（新增）
rust/crates/editor_ui_renderer/src/tests.rs 或 tests/widget_*.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/config.rs
rust/crates/editor_window_winit/src/window_plan.rs
rust/crates/editor_window_winit/src/headless_window.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/tests/*dpi* 或现有对应测试
rust/crates/editor_wgpu_renderer/src/draw_plan.rs
rust/crates/editor_wgpu_renderer/src/real.rs 或当前 real-wgpu renderer 模块
rust/crates/editor_wgpu_renderer/src/tests.rs
```

不允许为了文件名机械匹配强建全部新文件；如果 246 规则或当前模块结构要求更深聚合，应在激活复核中调整并重新自审。

## 4. 基线与隔离

激活后先执行：

```text
记录 exact commit、rustc/cargo 版本、Cargo.lock digest。
确认 247/248/248-A 已在基线中。
确认主工作树用户改动；施工使用专用 clean worktree/branch 或等价隔离。
显式设置独立 CARGO_TARGET_DIR，避免和其它任务竞争 target/二进制锁。
记录现有 editor_ui_renderer/editor_input/editor_window_winit/editor_wgpu_renderer 测试结果。
记录当前 scale_factor/dpi_scale 类型和真实窗口 feature 编译状态。
```

预检命令从 `G:\gameEngin\rust` 运行：

```powershell
cargo test -p editor_ui_renderer --locked
cargo test -p editor_input --locked
cargo test -p editor_window_winit --locked
cargo test -p editor_wgpu_renderer --locked
cargo check -p editor_window_winit --all-features --locked
cargo check -p editor_wgpu_renderer --all-features --locked
```

基线已有失败必须记录，不得在施工结束后才首次发现。

## 5. Gate A0 - Taffy 依赖与最小 Spike

### 任务

1. 核对 Taffy 当前稳定版本、license、MSRV、Cargo feature 和依赖树。
2. 仅启用 C-min 所需的 std/block/flex/content-size/tree 能力；Grid 未被现有面板证明需要时不启用。
3. 建立最小 synthetic tree，验证 horizontal/vertical、min/max、flex grow/shrink、gap、absolute overlay。
4. 验证 text measure callback、logical f32 输出和相同输入确定性。
5. 验证 overflow/scroll 只由 adapter 解释，不把业务状态塞进 Taffy node。
6. 记录依赖、compile-time 和 binary impact。

### 定向测试

```powershell
cargo test -p editor_ui_renderer widget_layout --locked
cargo tree -p editor_ui_renderer -e normal
cargo check -p editor_ui_renderer --locked
```

### 失败处理

Taffy 无法满足核心合同时暂停施工、回填 248 并重新审查；禁止转而手写完整 Flexbox。

## 6. Gate A1 - Widget Schema / Invariant / State Ownership

### 任务

1. 建立 `WidgetId`、`WidgetRole`、`EditorWidgetNode`、`EditorWidgetTree`。
2. 建立 `EditorWidgetAction` 与 `EditorCommandBinding`，业务副作用只能使用后者。
3. 明确 Dock persisted、node-local、focus/input transient、business state 四类 owner。
4. 建立唯一 ID、parent/children 对称、无循环、root 唯一、role/activation 合法验证。
5. ID 采用语义 key，禁止地址、裸 index 和每帧自增身份。
6. serde 仅用于 snapshot/evidence；反序列化 snapshot 不得直接恢复可写业务状态。

### 定向测试

```powershell
cargo test -p editor_ui_renderer widget_tree --locked
cargo test -p editor_ui_renderer widget_invariant --locked
cargo clippy -p editor_ui_renderer --all-targets --locked -- -D warnings
```

## 7. Gate A2 - Keyed Compose / Reconcile

### 任务

1. 实现 previous tree + declaration 的 keyed reconcile。
2. 相同 ID/兼容 role 复用 node-local state。
3. role 不兼容显式 replace 并记录诊断。
4. 动态列表 reorder 按 stable item key 保持正确状态。
5. 删除节点时返回 removed IDs，供 FocusInputSystem 清理 hover/press/focus/capture。
6. duplicate ID fail closed，禁止 last-write-wins。
7. 记录 reuse/create/remove/replace counts。

### 定向测试

```powershell
cargo test -p editor_ui_renderer widget_reconcile --locked
cargo test -p editor_ui_renderer widget_dynamic_list --locked
```

## 8. Gate A3 - Layout / Clip / Extract

### 任务

1. 将最小 `EditorWidgetLayoutStyle` 转换为 Taffy style。
2. 计算 logical rect；Dock split/overlay/scroll 使用明确 adapter。
3. 自顶向下计算 effective clip 和确定性 z-order。
4. visibility=collapsed 不布局/绘制/命中；hidden 可布局但不绘制/命中。
5. 从同一 geometry 提取 DrawCommand 和 clip/scissor metadata。
6. `editor_wgpu_renderer` 消费 clip/scissor，CPU/GPU 裁剪一致。
7. 文本、图片、Viewport slot 不得越过 effective clip。

### 定向测试

```powershell
cargo test -p editor_ui_renderer widget_layout --locked
cargo test -p editor_ui_renderer widget_clip --locked
cargo test -p editor_ui_renderer widget_extract --locked
cargo test -p editor_wgpu_renderer clip --locked
```

## 9. Gate A4 - Pick / WidgetPath / Derived HitRegion

### 任务

1. top-most pick 同时考虑 visibility、effective clip、z-order 和 enabled。
2. disabled target 返回自身及 reason，不能穿透到底层。
3. 构造 root-to-target WidgetPath。
4. 支持最小 target/bubble 语义和 pointer-capture target override。
5. 从 Widget geometry/activation 自动导出兼容 HitRegionSnapshot。
6. Panel API 不得要求调用方同时提供两个 rect。
7. 保持现有 InputRouter consumer 可编译，但不开放新的手写 HitRegion 入口。

### 定向测试

```powershell
cargo test -p editor_ui_renderer widget_pick --locked
cargo test -p editor_ui_renderer derived_hit_region --locked
cargo test -p editor_input --locked
```

## 10. Gate A5 - f64 DPI / Coordinate Boundary

### 任务

1. 将不能表达 1.5 的整数 DPI 合同升级为 `f64 scale_factor`。
2. 建立 physical -> logical -> physical round-trip helper，并集中在窗口/GPU边界。
3. `ScaleFactorChanged` 同步 surface、layout、glyph 和 pointer mapping dirty。
4. Panel/Widget 不读取 OS DPI，不自行 round。
5. headless 注入 1.0/1.5/2.0 验证相同 logical target 可被对应 physical point 命中。
6. 兼容旧报告时明确 schema/version，不在旧字段下静默改变单位。

### 定向测试

```powershell
cargo test -p editor_window_winit dpi --locked
cargo test -p editor_window_winit window_surface --locked
cargo test -p editor_ui_renderer logical_coordinates --locked
cargo check -p editor_window_winit --all-features --locked
```

## 11. Gate A6 - Synthetic Application Integration

### 任务

1. 在不迁移生产面板的前提下建立 synthetic WidgetTree scenario。
2. 验证 model/declaration -> reconcile -> layout -> extract -> pick -> action/command binding。
3. 验证 removed node 清理 FocusInputSystem 引用。
4. 验证 pointer move 不重建业务 Model，全树 compose 只由相关 revision 触发。
5. 输出 core 性能 evidence：compose/reconcile/layout/extract/pick 和 node counts。
6. 保证 Runtime/Player 依赖图不出现 WidgetTree/Taffy。
7. 建立 committed legacy manual-hit inventory，按文件/symbol 记录 A 激活时已有手写 HitRegion consumer；A 之后任何新增 consumer 或已有 consumer 净增长都 fail，直到 B 删除清单。

### 定向测试

```powershell
cargo test -p editor_window_winit retained_widget --locked
cargo test -p editor_ui_renderer --locked
cargo test -p editor_ui_renderer legacy_hit_region_budget --locked
cargo test -p editor_input --locked
cargo test -p editor_wgpu_renderer --locked
```

## 12. Gate A7 - 受影响域与权威回归

先运行：

```powershell
cargo fmt --all -- --check
cargo test -p editor_ui_renderer --locked
cargo test -p editor_input --locked
cargo test -p editor_window_winit --locked
cargo test -p editor_wgpu_renderer --locked
cargo test -p editor_core --lib --locked
cargo clippy -p editor_ui_renderer -p editor_input -p editor_window_winit -p editor_wgpu_renderer --all-targets --all-features --locked -- -D warnings
cargo check -p editor_window_winit --all-features --locked
```

候选冻结并完成环境等价预检后，运行：

```powershell
cargo test --workspace --locked
cargo test --workspace --all-features --locked
cargo run -p quality_gate --locked -- verify --report-mode summary
cargo run -p quality_gate --locked -- local-ci --commit HEAD
```

Local CI 必须绑定 clean exact commit；脏工作树 verify 不能冒充权威结果。最终候选代码、Cargo.lock、测试或文档变化后必须重新预检。

## 13. 完成与归档

248-A 完成时：

```text
生成独立阶段完成记录。
记录 Taffy 版本/license/features/MSRV 和依赖影响。
记录 core API、兼容 HitRegion Adapter 和尚未迁移面板清单。
记录各验证层、权威 Local CI run/digest/耗时。
将 248-A 从 当前/ 移到 已完成/。
更新施工 README、54 和阶段完成记录 README。
248-B 仍在 待执行/；只能在 A 完成后做激活复核。
247 P0-1 和 248 不得标记全部施工完成。
```

## 14. 回滚规则

```text
Taffy spike 失败：回滚未采用依赖并回填方案，不保留半套 layout adapter。
Schema/reconcile 失败：保留现有生产面板路径，移除未接入 synthetic core。
DPI schema 改动失败：按版本化 Adapter 回退，不伪造 150% 支持。
不得回滚用户无关改动，不使用 reset --hard 或 checkout --。
```

## 15. 施工文档自审

### 15.1 与 248 正式方案一致

通过。只实现 retained core、单一 geometry、Taffy、derived HitRegion 和 f64 DPI；没有提前迁移面板或做 C 的权威验收。

### 15.2 是否形成第二套长期 UI

没有。A 只建立替代底座；兼容 HitRegion 必须由 tree 派生，且 B 负责切换和删除旧入口。A 同时冻结激活时的 legacy manual-hit inventory，防止 A/B 之间继续积累手写 consumer。

### 15.3 状态归属是否重复

没有。文档要求四类状态唯一 owner，并用 reconcile/removal evidence 清理瞬时引用。

### 15.4 依赖选择是否可审查

可以。Taffy 必须先通过 license/MSRV/features/确定性 spike；失败必须回填方案，不能施工中换成自研 Flexbox。

### 15.5 测试是否分层

是。每 Gate 有定向测试，A7 有受影响域、default/all-features、QualityGate 和 exact-commit Local CI。

### 15.6 是否错误宣称 P0-1 完成

不会。A 只完成核心，B/C 是硬前置；完成记录必须列出未迁移面板。

### 15.7 当前能否施工

可以。本文已通过激活前复核并成为 `当前/` 唯一施工文档；必须严格按 Gate A0-A7 施工。

## 16. 自审结论

```text
范围：通过。
前置与串行关系：通过。
架构边界：通过。
测试与权威证据：通过。
队列规则：通过。
当前状态：已激活，可按 Gate A0-A7 串行施工。
```
