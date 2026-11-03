# 257-R1 Production Binary + Splitter Cursor Feedback Remediation v1 施工文档

## 0. 状态

```text
状态：已完成并归档
授权：2026-07-27 用户明确要求先解决旧生产二进制与 Splitter cursor 缺口
开始：2026-07-27T09:36:59.0629449+08:00
硬停止：2026-07-27T12:36:59.0629449+08:00
基线：d75e88947ab01748cd840268c6707a01405a0ace
```

## 1. 目标与边界

只完成两项：

1. 当前生产启动不得继续使用早于 257 feature commit 的 `editor_host.exe`。
2. 鼠标悬停或拖拽 Workspace Splitter 时，真实 winit Window 显示对应的水平或垂直 resize cursor；
   离开 Splitter 后恢复默认 cursor。

不实现浮动窗口、跨窗口拖拽、多显示器、拖 panel 内容区或新的 Dock 生命周期。

## 2. Owner

```text
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/tests/workspace_splitter.rs
rust/crates/editor_window_winit/src/lib.rs（仅在需要导出 cursor value 时）
```

## 3. 施工

### R1 红测试

增加 application owner 测试：

- Horizontal Splitter hover 返回 column-resize；
- Vertical Splitter hover 返回 row-resize；
- 非 Splitter hover 返回 default；
- active resize 在指针离开原 hit rect 后仍保持对应 resize cursor。

### R2 实现

在 application 层以 typed cursor value 表达当前 workspace cursor，不把 winit 类型泄漏到 headless
owner。real-window adapter 把 typed value 映射为 winit cursor，并在 PointerMove、PointerDown、
PointerUp、FocusLost 后同步真实 Window。

### R3 验证与生产替换

```powershell
cargo test -p editor_window_winit --lib --locked workspace_splitter -- --nocapture
cargo check -p editor_window_winit --locked --features real-window,real-wgpu-surface
cargo fmt --all -- --check
cargo clippy -p editor_window_winit --all-targets --locked --features real-window,real-wgpu-surface -- -D warnings
```

终止范围只允许 PID `21488`，且必须再次确认 executable path 精确为
`G:\gameEngin\rust\target\debug\editor_host.exe`。随后重建当前 production feature composition，
验证 EXE 时间与 257 源码身份，并启动新的可见窗口。

## 4. 自审

```text
正式方案：无需修改；257 已要求真实 Splitter drag，本修复补齐遗漏的真实 cursor feedback
范围：不扩展 257 v1 Dock 能力
测试：owner red-capable，真实 winit composition 通过 feature build 验证
外部状态：只替换本任务启动的旧 PID 21488
Local CI：不运行；本修复不主张新的 release-wide qualification
三小时限制：预计小于一小时
```

## 5. 完成

```text
代码提交：b04eef0
owner red test：先因缺少 WorkspacePointerCursor / workspace_pointer_cursor 编译失败
workspace_splitter owner regression：5/5 passed
real-window + real-wgpu-surface check：passed
fmt：passed
本 crate Clippy --no-deps -D warnings：passed
全依赖 Clippy -D warnings：被 engine_input / editor_ui_model 既有 lint 债务阻断，与本次改动无关
旧进程：PID 21488，精确路径核验后仅终止该进程
新 EXE：G:\gameEngin\rust\target\debug\editor_host.exe
新 EXE sha256：af01ef3d6305d0a3986c8518b1b616ac47e6e589e40826798550eba7829f4163
新 EXE LastWriteTime：2026-07-27 09:53:06 +08:00
新进程：PID 12368，--real-window，启动后持续存活
```

本 remediation 只解决旧 production binary 与 Splitter cursor feedback，没有扩展 257 v1
Dock 能力。49、54、施工文档 README 和 257 完成记录已同步，本文件移入 `已完成/`。
