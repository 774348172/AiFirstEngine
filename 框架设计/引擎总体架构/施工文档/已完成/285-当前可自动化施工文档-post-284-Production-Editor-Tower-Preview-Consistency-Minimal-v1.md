# 285 当前可自动化施工文档：post-284 Production Editor / Tower Preview Consistency Minimal v1

> 状态：已完成并归档
> 日期：2026-08-14
> 上游：284 Editor GameView Normal-Frame Immediate AUI Input Dispatch v1
> 授权：备份并替换 production Editor；备份并仅重建 Tower Preview cache；打开 Tower

## 1. 目标

使用当前工作区中已经完成的 284 源码构建一个 repository-external Editor candidate，事务替换唯一
`rust/target/debug/editor_host.exe`；byte-exact 保留旧 Editor 和 Tower `scene-main` Preview cache；只重建该
Preview cache，并把 Tower 打开供用户检查。

本施工不新增功能、不修改产品源码，也不把 dirty worktree 声明为 clean exact-commit release qualification。

## 2. 唯一 Gate：production 一致性更新

1. 记录 HEAD、dirty 状态、production Editor path/hash、Tower cache path/inventory 和相关进程。
2. 在新的 `C:\AifeProductionRuns\285-post-284-*` root 下使用独立 `CARGO_TARGET_DIR` 构建：
   `cargo build -p editor_host --locked --features real-window,real-wgpu-surface`。
3. 构建成功后记录 candidate SHA-256；不额外运行 headless、窗口、输入或视觉 smoke。
4. 确认没有 Editor 进程占用后，byte-exact 备份 production Editor；以 pending copy + SHA-256 复核后替换，
   再验证 production hash 与 candidate 一致。
5. byte-preserving 备份 Tower
   `samples/tower_defense_project/.aife/editor-preview/windows-dev/scene-main`；只移出并重建这一 cache entry，
   不触碰其它项目、平台或场景 cache。
6. 打开 Tower 普通项目 Editor，确认进程存在且窗口响应后停止自动操作，保留窗口给用户。
7. 任一 production replacement 或 cache rebuild 步骤失败，立即关闭本轮 owned 进程，并从本轮备份恢复旧
   Editor 与旧 cache，复核 hash/inventory 后报告。

## 3. 范围与禁止

- 允许外部状态：唯一 production `editor_host.exe`、Tower `scene-main` Preview cache、本轮 run root。
- 仓库文档：本文、施工索引、完成记录和归档状态。
- 不修改引擎源码、Tower 源码、Tower 配置或真实用户配置。
- 不运行 Local CI、完整视觉矩阵、输入 smoke、普通 Play smoke 或其它无关测试。
- 不替换 Player、MCP 或其它 production/安装态二进制。
- 不清理其它 cache，不删除旧 Editor/cache 备份，不 reset/revert/checkout 用户改动。

## 4. 完成条件

- candidate 构建成功，production Editor SHA-256 与 candidate 相同。
- 旧 production Editor 与旧 Tower cache 的独立 byte-preserving 备份存在。
- 只生成新的 Tower `scene-main` Preview cache，且其它 Preview cache 未被改动。
- Tower Editor 进程已打开并响应；不声称 Play、输入或视觉行为通过。
- receipt、完成记录、49/54/施工索引同步，本文归档到 `施工文档/已完成/`。

## 5. 必要性审计与自审

```text
confirmed gap: 284 只完成 source-level 闭环，production Editor 仍是 282 后旧二进制。
minimum causal action: 重建并替换唯一 Editor，然后使 Tower 唯一 Preview entry 消费新 Editor。
source files that must change: 无产品源码；仅施工/完成索引文档。
evidence: candidate/production hash、Editor/cache 备份 inventory、新 cache inventory、Tower 响应进程。
deferred: Play/input smoke、视觉矩阵、Local CI、配置迁移、其它缓存和二进制。
```

自审结论：本施工没有把一次 production 一致性更新扩张为新 release qualification；没有重复 284 已通过的
源码测试；binary 与 cache mutation 都有明确、byte-preserving 的回滚点；当前施工槽和待执行队列均为空，
用户授权与本 Gate 完全一致，允许激活并施工。

## 6. 完成结果

production Editor 已从 `5007CDA6...B9BC2` 替换为 `3413FE62...53A93`；Tower `scene-main` cache 由正式
owner 重建为 `Success/Rebuilt/load success`；Tower 项目 Editor PID `13020` 保持打开。完成记录见
`../../阶段完成记录/2026-08-14-post-284-Production-Editor-Tower-Preview-Consistency-Minimal-v1/00-总览.md`。
