# 258-R2 Header Paint Containment Remediation v1 施工文档

状态：已完成并归档

基线：`f9a711226083d1d1810029b5cc02b8ba721043a8`

用户授权：执行 258-R2 Header Paint Containment Remediation。

## 1. 目标与根因

修复 Tab viewport 使用完整 Stack 高度绘制背景和边框造成的两个同源缺陷：

- `stack.right - 48px` 处出现贯穿 Panel body 的内部竖线；
- Tab 背景覆盖 Hierarchy 选中背景，只留下右侧 chrome reserve 宽度可见。

持久化 `editor-workspace-layout.v2.json` 的 topology 与 ratio 有效，本施工不得重置、删除或
改写用户布局。

## 2. 范围

允许修改：

```text
rust/crates/editor_ui_renderer/src/panels/mod.rs
rust/crates/editor_ui_renderer/src/tests.rs
```

允许更新 258 完成记录、四份入口和本文档。禁止修改 workspace topology、splitter、dock、
Hierarchy selection 语义、项目数据或 MCP。禁止重跑 W1-W6 和 Local CI。

## 3. R2-A 红测试

测试 production `SelfUiRenderer` draw list：

- Tab background/border paint 必须全部 containment 在 Stack 顶部 24px；
- `header.bottom` 以下不得存在宽度为 `stack.width - 48px` 的 Tab background；
- Hierarchy selected row 的 accent rect 必须保留接近完整 Panel 宽度；
- 现有 Lock / More 24px hit region 与 interaction 不变。

命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer header_paint_containment -- --nocapture
```

必须先失败，再修改实现。

## 4. R2-B 实现

containment 由 `push_workspace_tabs` owner 内部强制执行：无论调用方传入多高 rect，背景、边框
和 Tab 都只能使用高度 24px 的 header rect。不得依赖每个调用方手工裁剪。

## 5. 回归与 production replacement

```powershell
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit workspace_panel_chrome -- --nocapture
cargo fmt --all -- --check
cargo build -p editor_host --features real-window
```

原 PID `4644` 在施工开始前已不存在，不执行停止。构建必须绑定 clean exact 实现提交，随后
以 `--real-window` 打开新 Editor 并确认持续存活。

## 6. 自审

- [x] 修复 paint containment，不修改布局或交互语义
- [x] red test 捕获用户截图中的竖线与选中背景遮挡
- [x] containment 放在 Tab owner 内，不新增公共 Interface
- [x] 不重跑 W1-W6 或 Local CI
- [x] 单次施工预计低于三小时
- [x] 完成后归档并恢复当前施工槽为空

## 7. 执行结果

```text
红测试：occluding PANEL paint count 1 != 0
定向 containment 测试：passed
editor_ui_renderer：59 passed
editor_window_winit workspace_panel_chrome：2 passed
format：passed
实现提交：de5d05c574d93aa36ee0c07eafa231a508cdaec4
原 PID 4644：not_found_before_construction
新 production PID：3404
新 EXE sha256：8a078ac7e2dbdf6090a4547dd324a6ed53cac416f0205913241fa241c85d4e12
persisted layout mutation：none
W1-W6 rerun：false
Local CI：not run, not required
```
