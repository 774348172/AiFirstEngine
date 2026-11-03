# 296 post-295 Production Editor + Tower Windows Dev Export Consistency Mini v1

状态：已完成并归档。

## 目标与边界

把已由 295 定向测试与 fresh Player smoke 证明的源码更新到 production `editor_host.exe`，重新打开普通
Editor、打开 Tower，并只导出一次新的 Windows dev 包。

不修改产品源码或 Tower 配置；不运行 Local CI、完整 E2E/视觉矩阵或 295 测试；不替换 MCP、安装态
Player 或其它二进制。

## 必要性与验证经济性

```text
requested claim: 普通 production Editor 可以导出包含 295 的 Tower Windows dev 包
minimum change: one Editor candidate build + byte-exact backup/replacement + one real export
reused evidence: 295 owner tests、Tower RuntimeModule 103 passed、fresh real-input smoke
Gate A -> binary mismatch -> candidate readiness + candidate/after SHA-256 equality
Gate B -> ordinary export availability -> desktop export report + package manifest success
stop condition: candidate/readiness/export 首次失败时停止，不扩大到 Local CI 或新修复
```

## Gate A：Production Editor 一致性更新

- 在新的 run-owned root 构建标准 debug `editor_host.exe` candidate。
- 执行 candidate headless readiness。
- 记录 production before hash，保留 byte-exact 备份后替换；after hash 必须等于 candidate hash。
- 失败则恢复备份并停止。

结果：通过。production 从 `DA894B86...20B3` 更新为 `93AE6C0C...43B66`；candidate/after hash
完全一致，两份旧二进制备份与 replacement receipt 均保留。

## Gate B：Tower 打开与 Windows dev 导出

- 启动替换后的普通 production Editor 并打开 Tower。
- 仅执行一次 Windows dev export；现有 Tower `Build/Windows/dev` 若存在则先备份。
- 核验 `desktop-export-report.json`、`package-manifest.json`、`Game.exe` 与 Player report。
- 保持 Editor 打开供用户查看。

结果：通过。Tower Gateway project binding 正确；持久 MCP operation
`tool-op-65f18b2299f1e31b61b3d7c5` completed，delivery 中 export/runtime package success、Player exit 0。
Editor PID `19460` 保持打开，Cargo/rustc/Game 均已退出。

## 完成

完成记录：
`阶段完成记录/2026-08-18-post-295-Production-Editor-Tower-Windows-Dev-Export-Consistency-Mini-v1/00-总览.md`。
