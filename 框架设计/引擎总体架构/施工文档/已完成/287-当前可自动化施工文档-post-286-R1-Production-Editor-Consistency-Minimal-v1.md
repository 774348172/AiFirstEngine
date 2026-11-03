# 287 当前可自动化施工文档：post-286-R1 Production Editor Consistency Minimal v1

> 状态：已完成，待归档
> 日期：2026-08-14
> 上游：286-R1 GameView Contain / Camera2D / Texture Last-Good Minimal Repair v1
> 授权：单独执行 production Editor 一致性更新

## 目标

使用当前工作区中已完成并验证的 286-R1 源码构建 repository-external Editor candidate，记录其
SHA-256，byte-exact 备份唯一 production `rust/target/debug/editor_host.exe`，再通过 pending copy 与
SHA-256 复核事务替换 production Editor。

本施工不新增功能、不修改产品源码，也不把 dirty worktree candidate 声明为 clean exact-commit release
qualification。

## 唯一 Gate：production Editor 一致性更新

1. 记录 HEAD、dirty 状态、production Editor path/hash、相关进程与磁盘条件。
2. 在新的 `C:\AifeProductionRuns\287-post-286-r1-*` root 下使用独立 `CARGO_TARGET_DIR` 执行
   `cargo build -p editor_host --locked --features real-window,real-wgpu-surface`。
3. 记录 candidate SHA-256，并确认 candidate 是本轮 build 产物。
4. 确认没有 Editor 进程占用后，byte-exact 备份 production Editor 与 before hash。
5. 先复制 candidate 到 production 同目录的 run-owned pending 文件并复核其 SHA-256；一致后替换 production
   Editor，再验证 after hash 与 candidate 完全一致。
6. 任一步失败，立即从本轮备份恢复旧 production Editor，并复核 rollback hash。

## 范围与禁止

- 允许外部状态：唯一 production `editor_host.exe` 与本轮 repository-external run root。
- 仓库写入：本文、施工入口、完成记录与归档状态。
- 不修改引擎/Tower 产品源码，不修改真实配置。
- 不重建或改动 Tower Preview cache。
- 不启动 Editor，不运行 Play/input smoke、视觉矩阵或 Local CI。
- 不替换 Player、MCP 或其它 production/安装态二进制。
- 不清理历史备份，不 reset/revert/checkout 用户改动。

## 完成条件

- candidate 构建成功且 SHA-256 已记录。
- 旧 production Editor 的独立 byte-exact 备份存在且 hash 等于 before hash。
- pending hash、candidate hash 与 production after hash 完全一致。
- receipt、完成记录、49/54/施工索引同步，本文归档到 `施工文档/已完成/`。

## 必要性审计与激活前自审

```text
confirmed gap:
286-R1 只有 source-level evidence，production Editor 仍是旧二进制。

minimum causal fix:
构建 candidate -> hash -> 备份 production EXE -> pending replace -> after hash。

files that must change:
无产品源码；仅施工/完成索引文档与 production editor_host.exe。

tests that can fail before and pass after:
本 Gate 是 binary identity transaction；证据为 candidate/pending/production SHA-256 与 byte-exact backup，
不重复运行输入未变化的 286-R1 源码测试。

explicitly deferred nearby enhancements:
Tower cache、真实配置、Editor smoke、Play/input smoke、视觉矩阵、Local CI、其它二进制。
```

激活前复核结论：当前施工槽与待执行队列为空；production Editor before hash 为
`3413FE62B4D1FC8AC45D19E8E8E2C7E50E24DC12C6CEC8FEDFFE3EDB1EA53A93`；没有 Editor 相关进程；磁盘
条件满足。用户授权与唯一 Gate 完全一致，允许激活并施工。

## 完成结果

candidate、pending 与 production after SHA-256 均为
`A6B89322F2560E66ADF9AD31BD61994C9DE2E4CF525BECCE324CC586D0AC35CD`；旧 production 与 byte-exact backup
均为 `3413FE62B4D1FC8AC45D19E8E8E2C7E50E24DC12C6CEC8FEDFFE3EDB1EA53A93`。未执行禁止项。完成记录见
`../../阶段完成记录/2026-08-14-post-286-R1-Production-Editor-Consistency-Minimal-v1/00-总览.md`。
