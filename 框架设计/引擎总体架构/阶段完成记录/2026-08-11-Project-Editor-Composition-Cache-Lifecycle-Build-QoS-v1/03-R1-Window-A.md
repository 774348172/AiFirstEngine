# 282-R1 Repair Window A / Gate R-A 至 R-D 完成记录

## 1. 结果

```text
状态：Window A / Gate R-A 至 R-D 已完成
日期：2026-08-12
source identity：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留的 dirty worktree
授权边界：schema、generated lock lineage、path-affine cache、typed deadline、consumer closure
未进入：Window B / R-E 至 R-G、Gate H、fresh root、Local CI
未修改：production/installed binary、真实 Tower cache、真实配置
```

## 2. 实现闭环

BuildRequest/BuildReport 已升级到 v3，Artifact/Descriptor/Qualification/Promotion 升级到 v2；
`GeneratedCompositionLockLineage.v1` 将 raw lock seal 与 canonical resolved graph 分离，
`ProjectEditorCompositionResolvedIdentity.v1` 将 requested identity、lineage 与最终 artifact key 绑定。
旧 build report 只读；旧 artifact/qualification/promotion 不会用空默认升级为 v2 authority。

owner 现在维护 `locks/<lock-input-digest>/Cargo.lock + lineage.json`。有效 hit 才跳过
`cargo generate-lockfile --offline`；raw/graph tamper、incomplete entry 与 publish collision 均 fail closed。
generated Cargo manifest template digest 使用结构化 TOML，将 run-owned dependency absolute path 归一为稳定
dependency slot，同时继续跟踪 package/features；因此 staging root 不再污染 lineage/artifact key。

compilation cache v2 使用 `ct/<compatibility>/<target-anchor-prefix>`，`affinity.json` 同时绑定完整
canonical owner anchor 与最终 target root digest。只有三项全匹配才报告 `same_root_hit`；复制或 marker 缺失
报告 `path_affine_miss`，且 `crossRootPortable=false`。

typed deadline 为 generate-lock 60 秒、release soft 600 秒、release hard 1200 秒、descriptor query 30 秒。
soft budget 只告警；hard deadline 才终止。Completed/non-zero/Cancelled/Timeout/WaitFailed/SpawnFailed 保持不同
terminal，顶层 report 提升 output readers joined、root wait、process group release 与 owned cleanup evidence。
production request builder、launcher、qualification seal、promotion、normal loader 与 fixtures 已统一使用 resolved identity。

## 3. 验证与修复轨迹

R-A/R-B/R-C 定向测试通过：contract 1、lock lineage 2、resolved identity 1、lineage store 2、
compilation cache v2 1、deadline 1、process terminal 1。

首次 `editor_core` 全套暴露 4 个 fixture 失败：当前 v3 被旧断言当成错误 schema、promotion 仍期待 v1、
fake Cargo 未生成 lockfile 导致 QoS 测试未进入 build、promotion negative request 内部 resolved identity 不一致。
四项修复后定向重跑通过。第二次全套为 675 passed / 2 ignored / 1 failed，唯一失败是相同请求第二次
`Rebuilt` 而非 `Hit`。首因是 generated Cargo.toml 的 staging absolute path 进入 template digest；结构化归一后，
快速 path-independent red test 与真实 specialized Editor cache-hit 测试均通过。

最终证据：

```text
cargo test -p editor_core
  677 passed / 0 failed / 2 intentional ignored
cargo test -p editor_window_winit project_editor_composition_ -- --nocapture
  8 passed / 0 failed / 3 intentional ignored
cargo check -p editor_window_winit --features real-window
  passed
cargo fmt --all -- --check
  passed
scoped git diff --check
  passed
```

完整 `editor_core` 含 release artifact fixture，最终用临时日志承载以避免工具 stdout 管道 124 秒关闭；
测试内容未改变。当前工作树已有 runtime_cli 改动属于 282 前序基线，本 Window A 未修改 runtime_cli，故按施工
文档不追加 runtime_cli suite。

## 4. 外部状态与下一步

本窗口只使用测试临时目录与既有 `G:\Aife262W2` fixture owner；未创建 R1 fresh root，未运行 Local CI，
未启动普通/production Editor，未替换二进制，未修改真实 Tower cache、trust、recent projects 或配置。

282 继续保留在 `施工文档/当前/`，不得归档。下一步只能等待用户单独授权 Window B / Gate R-E 至 R-G；
Window B 仍不得触碰 production/真实 cache/config，也不得自动进入 Gate H。
