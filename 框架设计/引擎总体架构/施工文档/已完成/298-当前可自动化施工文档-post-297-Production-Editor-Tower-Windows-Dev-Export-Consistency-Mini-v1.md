# 298 post-297 Production Editor + Tower Windows Dev Export Consistency Mini v1

状态：已完成并归档；用户于 2026-08-18 明确授权更新 production Editor 并重新导出 Tower Windows dev 包。

## 目标与边界

把已由 297 owner/consumer 测试证明的 Windows dev `Game.exe` 无参数入口修复更新到 production
`editor_host.exe`，然后通过更新后的普通 Editor 重新导出一次 Tower Windows dev 包。

不修改产品源码、Tower 源配置或真实配置；不运行 Local CI、完整 E2E/视觉矩阵；不替换 MCP、安装态
Player 或其它二进制；不修补旧导出包。

## 必要性与验证经济性

```text
requested claim: 新 production Editor 可以导出可无参数启动的 Tower Windows dev Game.exe
minimum work: one Editor candidate build + readiness + byte-exact backup/replacement + one real export
reused evidence: 297 runtime_cli owner 24 passed、process consumer 5 passed、real-window check passed
Gate A -> production binary identity mismatch -> candidate readiness + candidate/after SHA-256 equality
Gate B -> new package availability -> completed export report + new Game.exe bounded zero-arg launch
excluded: source repair、schema/exporter redesign、Local CI、full E2E、旧包修改
stop condition: candidate/readiness/replacement/export 首次失败即保存证据并停止；replacement 失败立即回滚
```

## Gate A：Production Editor 一致性更新

1. 在新的 run-owned root 构建标准 debug `editor_host.exe` candidate。
2. 执行 candidate headless readiness。
3. 记录 production before hash，保留 byte-exact 备份后替换；after hash 必须等于 candidate hash。

结果：通过。production 从 `93AE6C0C...43B66` 更新为 `E27EE515...BF163`；两份旧二进制备份、
replacement receipt 和 candidate build/readiness 证据均保留。

## Gate B：Tower Windows dev 重新导出

1. 启动替换后的 ordinary production Editor 并打开真实 Tower。
2. 通过 production Editor Gateway 仅创建一次有效 `project.build_export` operation，轮询到 terminal。
3. 核验新 delivery 的 desktop export report、package manifest、RuntimePackage、Player report 与
   `Game.exe` 哈希。
4. 对新 `Game.exe` 做一次有界无参数启动；能持续存活并创建窗口后终止本轮 owned 进程。

结果：通过。operation `tool-op-ecbce750c22a3a28165eef1f` completed；新 `Game.exe` 为
`D8C3F5A8...BC025`，持续运行 8 秒并创建 `AI First Engine Player` 窗口。完成记录见
`../../阶段完成记录/2026-08-18-post-297-Production-Editor-Tower-Windows-Dev-Export-Consistency-Mini-v1/00-总览.md`。
