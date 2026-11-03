# 258-R3 Editor UI Typography Scale Remediation v1 施工文档

状态：completed

用户授权：所有 Editor UI 字号在现有值上增加 5，并同步扩大对应 UI。

## 1. 目标

在不改变 workspace topology、dock/drag、Panel 命令、项目数据或 MCP 语义的前提下：

1. 所有 production Editor `DrawCommand::Text` 字号相对当前基线增加 `5.0`。
2. 建立唯一的 Editor typography/compact-control metrics owner，避免字号与行高继续分散漂移。
3. 同步扩大 menu、toolbar、workspace tab/header、按钮、输入框、列表行、popup 和状态反馈区域，
   保证文字不裁切、不覆盖相邻控件，hit rect 与 paint rect 保持一致。
4. Launcher 和主 Editor 使用同一字号增量合同，但不修改 Runtime/AUI 字体。

## 2. 范围

允许修改：

- `rust/crates/editor_ui_renderer/src/`
- 直接受影响的 `editor_window_winit` 测试期望
- 本施工文档、完成记录和施工入口摘要

禁止修改：

- workspace topology、splitter/dock/drag 状态机
- Inspector Lock、Close Tab 或 Panel ownership 语义
- `editor_wgpu_renderer` 字形栅格实现
- Runtime/AUI、项目文件、MCP/Gateway
- Local CI 与 258 W1-W6 历史 authority

## 3. 实现合同

1. 新增 `EditorUiMetrics`，至少拥有 typography delta、常规/小号/标题字号、compact control、
   panel header、list row、popup row 和 shell bar 高度。
2. production Editor Text 不再直接使用旧字号数值；旧值通过 metrics 映射为 `old + 5.0`。
3. 文本容器高度不得小于其字号；常用交互控件高度不得小于新的 compact control 高度。
4. workspace tab paint、Tab hit rect、Panel chrome button 与 popup 均使用相同 header/row metrics。
5. 小 viewport 下允许内容减少或滚动，不允许负尺寸、文字跨出 paint/hit owner。

## 4. Gate

### R3-A Owner red test

- 枚举 production draw list 中的 Text，证明主界面旧 8-13px 字号会违反新合同。
- 验证 shell、Tab、Hierarchy row 和 Panel popup 的文字容器与交互高度。

### R3-B Implementation

- 引入 metrics owner。
- 迁移主 Shell、Panel chrome、Toolbar、Hierarchy、Inspector、Console、Viewport、Launcher 与其余
  production panels。

### R3-C Affected regression

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit workspace_panel_chrome
cargo test -p editor_window_winit workspace_splitter
```

只在编译期接口受影响时补跑对应直接消费者；不运行 Local CI。

### R3-D Production visual check

重建 `editor_host`，以 `--real-window` 启动，确认真实 WGPU 字体清晰可见、控件随字号扩大且无
明显裁切或重叠。该检查不消费 258 历史 authority。

## 5. 完成条件

- owner red test 先失败、实现后通过；
- 所有 production Editor 字号满足 `baseline + 5.0`；
- 受影响回归通过；
- production Editor 真实窗口可用；
- 完成记录生成，本文档归档。

## 6. 自审

- [x] 用户要求是视觉 remediation，不修改 258 正式架构方案
- [x] 字号和对应 UI 几何由同一 metrics owner 管理，不新增第二套 layout system
- [x] 测试范围贴近 owner 和直接消费者，不重跑 W1-W6 或 Local CI
- [x] 不修改 Runtime/AUI 字体，不扩大到项目内容
- [x] 单窗口预计远低于三小时

## 7. 完成证据

- 实现提交：`d493ff99e05506dea9f3bd7d687359dddd9cdba8`
- red test：`editor_typography_scale_raises_all_production_text_and_control_rows` 在旧实现上捕获
  8-11px production 字号，实现后通过。
- `cargo test -p editor_ui_renderer`：60 passed。
- `cargo test -p editor_window_winit workspace_panel_chrome`：2 passed。
- `cargo test -p editor_window_winit workspace_splitter`：5 passed。
- `cargo build -p editor_host --locked --features real-window,real-wgpu-surface`：passed。
- production binary SHA256：
  `7faf55b6c4e9ba6a004fce0f8e51d0fa6c98acc76b5ccb873c72fd1b0700b968`。
- production PID `9800` 正常响应；Launcher 与项目 workspace 均完成真实 WGPU 截图检查。
- 未运行 Local CI；本次窄视觉 remediation 不需要重跑 258 W1-W6 authority。
