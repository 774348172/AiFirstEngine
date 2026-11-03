# 282 Window C / Gate H 未通过记录

## 1. 结果

```text
状态：Gate H 未通过；在 production mutation 前安全停止
日期：2026-08-11
source identity：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留的 dirty worktree
run root：C:\AifeProductionRuns\282-c3-20260811-225420
production Editor candidate SHA-256：AA8EB114B44B3022ACD94ED1F541E2FF78E059A542573388F398D2FE39355628
Tower expected exact identity：sha256:e76e62c9c4d04bb018d713ccb8f79e1cced22e4d8db0cc83780ff45cc6484ade
production replacement：未开始
real cache promotion：未开始
Local CI：未运行
```

candidate 在 repository-external target 以 `real-window,real-wgpu-surface` 构建成功，headless readiness
通过。run-owned ordinary Editor 能打开 Tower、显示并批准 run-owned trust prompt，真实窗口全程
`Responding=true`。

## 2. 首个失败与受控续跑

新 candidate 对应的 Tower exact build 在
`source-qualified/project-editor-compositions` 中执行，identity 始终为 `e76e62c9...4ade`：

```text
attempt 1：jobs=2；build_composition_release=600015 ms；timeout；staging removed
attempt 2：复用同 root compatibility ct；jobs=1；600004 ms；timeout；staging removed
```

两次均在高层 `editor_core` release rustc 完成前触及 owner 的固定 600 秒 step timeout；没有 schema、
source 或 linker error。timeout owner 请求终止并 join 进程树，检查时 Cargo/rustc 均为 0。

Gate G 与 Gate H 的 269 compatibility key 同为 `554a84eec277b25aed63bca5df81f79d`。将 Gate G
1,915,700,778 bytes / 2,581 files 的 intermediate cache 精确复制到 run-owned `source-qualified-r2`
后，Cargo 因 target root 绝对路径变化重编 path-sensitive 产物；该尝试在 161381 ms 主动取消并证明
`cancellationRequested=true`、`processTreeTerminated=true`。随后只读复用 Gate G 原 ct 路径仍因当前
generated lock dependency resolution 漂移补编依赖，在进入 `editor_core` 后已无足够 deadline；于
412129 ms fail-fast 取消。没有第三次盲目 timeout 重跑。

## 3. Production 与配置状态

Gate H 没有获得 qualified exact source artifact，因此严格停止在事务第 2 步之前：

```text
production Editor before/after：
  G:\gameEngin\rust\target\debug\editor_host.exe
  F17BEDB2A3892486DED074DE1E4FA1AEF9D4D14CC37E51E35B4155BEBD23C786
real composition cache：14 entries；latest mtime 2026-08-11T12:29:35.7735232+08:00
Codex config digest：0F728C2D69BBD603F2872A973A7A317258A32586896BBA75A2D3CAF02AF1A8A0
real recent-project digest：2ABE601FFA8B494B3E9203612B639EDEEC4F0BF0C3A7B29435FC89C3FA9D68EE
real trust tree digest：8DE8D14DA9DF72D4C39B30F93DEE7A17A81DAE96CB0B28584D3F4D341D2C901B
```

未创建 production binary/cache backup，因为事务化 replacement/promotion 从未开始；因此无需回滚。
Player、MCP、其它 installed binary 和真实配置均未触碰。最终无 Cargo/rustc/editor/generated Editor
owned process 遗留。

Gate G run root 新增了本次 run-owned trust receipt 与一份 cancelled build report，旧 exact artifact、
qualification report/seal 未覆盖。c3 run root、两个 timeout report、两个 cancellation report、复制的
1.784 GiB intermediate cache、candidate 和截图均保留，不做清理。

## 4. 结论与下一步

282 不得归档或声明 production consistency closure。下一步先形成最小 repair 方案，至少解决：

1. generated composition dependency resolution 必须稳定并进入 compatibility identity/lock lineage；
2. compatibility cache 的可移植性声明必须与 target-root 绝对路径事实一致；
3. `editor_core` release build 的 bounded timeout 必须能区分真实 hang 与本机正常但超过 600 秒的优化编译。

在 repair 完成并重新资格化前，不得直接重跑 Gate H，也不得替换 production Editor 或提升真实 cache。
