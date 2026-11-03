# 282 Window A / Gate A-D 完成记录

## 1. 结果

```text
状态：Window A / Gate A-D 已完成
完成日期：2026-08-11
source identity：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留的 dirty worktree
证据资格：dirty-worktree affected integration + repository-external fresh Windows qualification
fresh role root：C:\AifeProductionRuns\282-c1-20260811-174607
未声明：exact-commit、Local CI、production/installed binary replacement、真实配置或真实用户 cache
后续授权：无；Window B / Gate E-G 与 Window C / Gate H 仍需分别授权
```

Window A 在既有 composition artifact 与 `runtime_cli` bounded child process owner 上完成了 Build
Request/Report v2、v1 只读兼容、typed QoS policy/system facts/decision、Windows 可用内存采集、
BelowNormal priority、全 composition build step cancellation、worker Drop/join、bounded progress 和
10 Hz redraw deadline。262 exact identity/trust/artifact seal 与 269 compatibility cache 均未放宽。

## 2. QoS 与进程合同

默认 policy：

```text
maxJobs=4
reservedLogicalProcessors=4
reservedMemory=3 GiB
estimatedMemoryPerJob=1 GiB
minJobs=1
```

`resolvedJobs` 是 CPU cap、memory cap 与 maxJobs 的最小值，下限 1；facts 缺失时走保守 fallback，
不回退到 CPU-wide 默认。Windows Cargo root 在 suspended 状态绑定 Job Object、施加 BelowNormal priority
后 resume；generate-lockfile、release build 和 descriptor query 共用同一 cancellable owner。Cargo 参数保持
`--release --locked --offline`，只增加唯一 `--jobs <resolvedJobs>`。

本机 12 个逻辑处理器，但 cold build 时可用内存约 6.72 GiB；扣除 3 GiB reserve 后 memory cap 为 3，
因此真实结果 `resolvedJobs=3`。`maxJobs=4` 是上限，不得把本次结果强制或伪报为 4。

## 3. fresh Windows qualification

唯一 role root：

```text
C:\AifeProductionRuns\282-c1-20260811-174607
```

该 root 独立持有 evidence/logs/state/cache/target/temp/project/handoff。source-built Editor candidate 位于：

```text
C:\AifeProductionRuns\282-c1-20260811-174607\target\editor-candidate\debug\editor_host.exe
```

cold controlled build 与 real generated Editor handoff：

```text
report：handoff\evidence-r3\282-c1-real-process-report.json
status：passed
sourceKind：controlled_build
cacheStatus：rebuilt
resolvedJobs：3
requested/effective priority：below_normal / below_normal
priorityApplied：true
command：build --release --locked --offline --jobs 3
candidate terminated/joined：true
application worker dropped/joined：true
```

响应性证据 `evidence\responsiveness.json`：

```text
sampledDuringNestedCargo=true
Editor Responding=true
message samples=60；timeout=0；P95=0.4599 ms
resize samples=20；P95=22.5584 ms
title-bar drag 后仍响应
nested Cargo priority=BelowNormal
最多 3 个 rustc，与 resolvedJobs 一致
```

compiling cancellation 证据：

```text
report：handoff\evidence-r5-cancel\282-c1-cancel-report.json
cancelIssuedAtPhase=compiling
cancelJoinElapsedMs=70
applicationWorkerDroppedAndJoined=true
failure diagnostic=project_editor_composition.cancelled
cancellationRequested=true
processTreeTerminated=true
Job Object root wait/group release/output readers join=true
```

内层 BuildReport 的 `workerJoined=false` 是正确事实：artifact owner 返回时 application worker 尚未由
外层 lifecycle join；外层 qualification report 单独证明 Drop 后 worker join，未伪造内层字段。

## 4. 验证

最终有效结果：

```text
cargo test -p runtime_cli
  23 unit + 8 bounded-process integration + 4 exported-player integration passed
cargo test -p editor_core
  662 passed / 1 harness-format failure / 2 ignored
定向重跑唯一失败项
  1 passed；修复后等价累计 663 passed / 2 ignored
cargo test -p editor_ui_model
  66 passed
cargo test -p editor_ui_renderer
  76 passed
cargo test -p editor_window_winit
  189 passed / 2 ignored
cargo check -p editor_window_winit --features real-window
  passed
cargo fmt --all -- --check
  passed
git diff --check（282 tracked scope）
  passed
```

`editor_core` 首次完整回归的唯一失败来自新增
`project_editor_composition_production_gate.rs` 一处未 rustfmt，仓库内 artifact test 的格式 preflight 正确
fail closed。只修正 test harness 排版后，原失败测试通过；产品源码与其余 662 个测试输入未变化，按施工
规则复用这些有效证据，没有机械重跑完整 suite。更早一次 1 秒 outer timeout 导致 Windows BrokenPipe，
属于无效 harness attempt，不作为产品失败或通过证据。

## 5. 失败分类与边界

普通 Editor folder picker 自动化多次未能可靠填写目录，正确报告
`project_open.manifest_unreadable`。该问题属于 Windows picker 自动化路径/字段语义，不是 composition build
失败；这些 attempts 保留在 fresh root，但不计入 Gate 成功。最终资格由仓库既有 real-process handoff owner
完成，没有用猜测坐标扩大 claim。

未修改真实用户 composition cache、production/installed 二进制、真实配置、Tower 项目源内容；未运行
Local CI，未进入 Gate E-H。source-built普通 Editor 已通过 WM_CLOSE 正常退出，run-owned Cargo/rustc/
composition/candidate 无遗留。fresh role root 作为证据保留，不做递归清理。

## 6. 下一步

282 施工文档保持在 `施工文档/当前/`，系统尚未整体完成。继续施工前必须由用户单独授权
`282 Window B / Gate E-G`；Window B 完成也不自动授权 `Window C / Gate H`。
