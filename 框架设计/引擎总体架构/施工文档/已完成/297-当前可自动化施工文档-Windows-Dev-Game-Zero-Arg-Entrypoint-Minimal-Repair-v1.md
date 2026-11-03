# 297 Windows Dev Game Zero-Arg Entrypoint Minimal Repair v1

状态：已完成并归档；用户于 2026-08-18 授权小问题直接修复。

## 目标与边界

修复 Windows dev 导出的 `Game.exe` 无参数启动把 `desktop-package-manifest.v1` 误按
`release-package-manifest.v1` 解析并立即 exit 2 的问题。

只修改 `runtime_cli` packaged entrypoint owner 和对应定向测试。不修改 Tower、既有 schema、
DesktopExportPipeline、production Editor、旧导出包或真实配置；不运行 Local CI、不重新导出。

## 必要性与验证经济性

```text
confirmed failure: Game.exe 无参数启动在 0.27s 内以 release_manifest_invalid / exit 2 退出
minimum change: 按现有 schemaVersion 分流 desktop dev manifest，使用包内 data/runtime_package
excluded: schema v2、exporter 重构、release manifest 迁移、旧包修补、production 更新、完整 E2E
Gate A -> manifest contract mismatch -> one red owner test -> filtered owner regression + fmt
reused evidence: 当前与 295 旧包相同 exit 2；显式 windowed 已创建 AI First Engine Player 窗口
estimated cost: 10-20 分钟；若需要跨出 runtime_cli 或改变 schema，停止并进入方案讨论
```

## Gate A

1. 先新增 `desktop-package-manifest.v1` resolve 红测试并确认失败。
2. 在 `runtime_cli` packaged entrypoint 中按 schemaVersion 分流；release 路径保持原严格校验。
3. desktop dev 只接受 `target=windows/profile=dev`，忽略 manifest 中机器绑定的绝对路径，固定安全解析
   `data/runtime_package`。
4. 运行 filtered owner tests 与 `cargo fmt -p runtime_cli -- --check`。

结果：Gate A 已通过。owner、进程 consumer、format 与 real-window feature check 均通过；完成记录见
`../../阶段完成记录/2026-08-18-Windows-Dev-Game-Zero-Arg-Entrypoint-Minimal-Repair-v1/00-总览.md`。
