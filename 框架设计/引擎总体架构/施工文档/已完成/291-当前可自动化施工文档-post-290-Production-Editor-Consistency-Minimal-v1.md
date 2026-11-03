# 291 当前可自动化施工文档：post-290 Production Editor Consistency Minimal v1

> 状态：已完成，待归档
> 日期：2026-08-15
> 上游：289/290 Editor GameView white-flash source repairs
> 授权：用户单独授权 production Editor 一致性更新

## 目标

从当前已验证源码构建 repository-external Editor candidate，byte-exact 备份唯一 production
`rust/target/debug/editor_host.exe`，经同目录 pending 与 SHA-256 复核后替换 production Editor。

## 唯一 Gate

1. 记录 HEAD、关键源码 hash、production before hash、进程与磁盘条件。
2. 在新的 `G:\AifeProductionRuns\291-post-290-*` root 和独立 `CARGO_TARGET_DIR` 中执行
   `cargo build -p editor_host --locked --features real-window,real-wgpu-surface`。
3. 校验 candidate 身份；确认 Editor 未运行后备份旧 production Editor。
4. candidate 复制为 production 同目录 pending，校验 hash 后替换；after hash 必须等于 candidate hash。
5. 任一步失败立即从 byte-exact backup 回滚并复核 hash。

## 范围与禁止

- 允许写入：本轮 repository-external run root、唯一 production Editor、施工/完成索引文档。
- 不修改产品源码、Tower Preview cache、真实配置、Player、MCP 或其它安装态二进制。
- 不启动 Editor，不运行 Play/input smoke、视觉矩阵或 Local CI。
- 290 已通过且源码输入未变化的 owner/consumer 测试直接复用，不机械重跑。

## 必要性审计与激活前复核

```text
confirmed gap:
289/290 只有源码修复证据，production Editor 仍为 A6B89322...C35CD。

minimum causal fix:
独立 candidate build -> hash -> backup -> pending hash -> exact replacement -> after hash。

files that must change:
无产品源码；仅 production editor_host.exe、run evidence 和施工索引。

tests that can fail before and pass after:
本 Gate 是 binary identity transaction，以 build verdict、candidate/pending/after SHA-256 和 backup hash 为证据。

explicitly deferred nearby enhancements:
Tower cache、真实配置、Editor smoke、Play/input smoke、视觉矩阵、Local CI、其它二进制。
```

激活前复核：施工槽无活动施工文档；production before hash 为
`A6B89322F2560E66ADF9AD31BD61994C9DE2E4CF525BECCE324CC586D0AC35CD`；Editor/Cargo/Rustc
相关进程为 0；G 盘可用约 113.7 GB；授权与唯一 Gate 一致。允许施工。

## 完成结果

```text
build: passed（dev profile 3m36s，仅既有 warnings）
candidate/pending/production after: F8856CE9A507E7A2E6D4DCA6E8D20B2957ECB507AACB414917281240D7420EA8
production before/backup/replace-backup: A6B89322F2560E66ADF9AD31BD61994C9DE2E4CF525BECCE324CC586D0AC35CD
run root: G:\AifeProductionRuns\291-post-290-20260815-142113
receipt: run root/291-completion-receipt.json
rollback required: false
```

未执行禁止项。完成记录见
`../../阶段完成记录/2026-08-15-post-290-Production-Editor-Consistency-Minimal-v1/00-总览.md`。
