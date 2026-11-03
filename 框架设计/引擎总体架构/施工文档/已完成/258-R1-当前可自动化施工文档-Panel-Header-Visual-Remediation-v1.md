# 258-R1 Panel Header Visual Remediation v1 施工文档

状态：已完成并归档

基线：`f981381280513fe28fa1c410fd1ac1b4da313c30`

用户授权：执行一次 258-R1 Panel Header Visual Remediation。

## 1. 目标

修复用户在 production Editor 中确认的两个视觉缺陷：

1. Stack Tab header 与 Panel body 在同一位置重复绘制标题，形成名称重影；
2. 右上角 Lock / More 使用 `"l"/"L"` 与 `"..."` 文本和大色块，不符合 Unity 风格的小型
   锁图标与竖向三点。

## 2. 范围

允许修改：

```text
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_ui_renderer/src/panels/mod.rs
rust/crates/editor_ui_renderer/src/tests.rs
```

允许在完成时更新本施工文档、258 完成记录和四份入口文档。

禁止修改 workspace topology、拖拽/dock 行为、Inspector Lock 语义、MCP、项目数据和 258
W1-W6 evidence。禁止重跑 W1-W6 或 Local CI。

## 3. R1-A 红测试

- active Stack header 的 Panel title 只能有一个 header draw command；
- chrome 不得再生成 `"l"`、`"L"` 或 `"..."` 文本图标；
- Lock / More 仍保留稳定且相邻的 24px 命中区；
- More 视觉必须是竖向三点；Lock 视觉使用 renderer-native geometry，不依赖字体 glyph。

命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer panel_header_visual_remediation -- --nocapture
```

测试必须先失败，再进入实现。

## 4. R1-B 实现与回归

- 让 Stack Tab header 成为标题唯一 owner；
- Panel body 仍负责背景、边框和现有内容 offset，不再绘制重复 header/title；
- chrome 保留 24px hit target，视觉图标缩小居中；
- idle 状态不绘制整块按钮底色，active 状态保留轻量反馈。

回归：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit workspace_panel_chrome -- --nocapture
cargo fmt --all -- --check
```

## 5. Production replacement

受影响回归通过后，停止本次启动的 PID `32884`，构建：

```powershell
cargo build -p editor_host --features real-window
```

随后以 `--real-window` 重新启动同一路径，并确认新 PID 持续存活。不得停止其它 Editor 进程。

## 6. 完成与自审

- [x] 属于实现 remediation，不修改 258 正式方案
- [x] 测试贴近 `editor_ui_renderer` owner，不扩大到 Local CI
- [x] 保留交互命中区和 Inspector Lock 语义
- [x] production replacement 仅针对用户当前启动的 PID 32884
- [x] 预计可在单次三小时窗口内完成
- [x] 完成后归档本文档并恢复当前施工槽为空

## 7. 执行结果

```text
红测试：title count 2 != 1
定向修复测试：passed
editor_ui_renderer：58 passed
editor_window_winit workspace_panel_chrome：2 passed
format：passed
实现提交：3aea824fef6bdd926552a24ff622c16640b200f3
原 production PID：32884（仅停止该 PID）
新 production PID：4644
新 EXE sha256：2781b8ca2fc5534c8d97becccbf3429bea478d247b82d1e95cbd743c932e76a4
W1-W6 rerun：false
Local CI：not run, not required
```
