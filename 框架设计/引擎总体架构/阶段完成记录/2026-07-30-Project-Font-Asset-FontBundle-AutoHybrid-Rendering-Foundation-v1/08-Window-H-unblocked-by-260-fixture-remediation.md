# 261 Window H 阻塞解除：260 fixture remediation

日期：2026-07-30

状态：原 `project player artifact package_failed` 阻塞已解除；Window H 尚未完成

证据级别：dirty-worktree development evidence

## 1. 根因与修复

260 将 ProjectRuntimeModule 合同升级为 v2，并要求每个项目显式注册一个
ProjectRuntimeSession factory。`project_player_artifact` 的内嵌无状态项目 fixture 已更新
interface version，却漏掉显式 no-op session 注册；Preview 准备成功，Export child Player
在 `ProjectRuntimeBootstrap::bind` 时返回 `project_runtime.session_missing`，外层表现为
`package_failed`。

修复仅在测试 fixture source 中导入 `create_empty_project_runtime_session`，并在 UI producer
之前调用 `set_runtime_session_factory`。运行时缺失 factory 的 fail-closed 合同未修改，261
字体产品代码未修改。

## 2. 验证

```text
cargo test -p editor_core \
  project_runtime_player_artifact_project_rust_preview_project_rust_export_share_identity \
  --offline -- --nocapture
结果：1 passed，0 failed

cargo test -p editor_core --offline
结果：585 passed，0 failed，2 ignored
```

## 3. 当前边界

`07-Window-H-blocked.md` 保留为历史首次失败证据；其 260 fixture blocker 已由本记录解除。
Window H 后续 `runtime_player_winit --features real-window`、`project_e2e_gate`、
`editor_window_winit`、塔防/第二项目/legacy matrix 以及未授权的 production 视觉范围仍未完成。
261 当前文档继续占用施工槽，不能归档或标记完成。
