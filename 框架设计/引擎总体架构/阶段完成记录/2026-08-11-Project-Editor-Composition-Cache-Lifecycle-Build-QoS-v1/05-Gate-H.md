# 282-R1 新 Gate H 未通过记录

## 1. 结果

```text
状态：未通过；post-mutation normal loader exact hit 失败后已完整回滚
日期：2026-08-12
fresh root：C:\AifeProductionRuns\282-r1-gate-h-20260812-150654
source：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留 dirty worktree
candidate SHA-256：5007CDA6E32036D2F5FAD5A4CBD58B6A1AA9ED20E21E23DC905A66D42A5B9BC2
expected identity：sha256:4142b1e905ebeebcd174f3660a821bf72602af2ccb8c661de58921d22baca056
resolved graph：sha256:d7150c7cabaeb067a1ebb69b73c1203820f878361779ea3a1b1ab1b7d564c0af
artifact key：sha256:0b4ed21bbbadd92cea528bfe849e9c7cc999face98fc02588e8c339896a482aa
最终 production Editor：F17BEDB2A3892486DED074DE1E4FA1AEF9D4D14CC37E51E35B4155BEBD23C786（已恢复）
```

## 2. 已通过部分

candidate 在 run-owned target 以 `real-window,real-wgpu-surface` 构建，headless readiness 通过。Tower
run-owned snapshot 只把 RuntimeModule Engine dependency 接到受信任的 `G:/gameEngin/rust`，并保留
`Library/AssetPipeline` 三个普通 Play 必需索引。

candidate-bound cold composition build 使用单 job，在 600 秒 soft budget 后继续，并于 1,186,097 ms
自然完成，未触发 1,200 秒 hard deadline。qualification 完整通过 open/play/pause/step/resume/stop，seal v2
与 executable/descriptor/build/qualification hashes 一致。首次长路径 build 以 LNK1104 fail-closed，路径由
261 缩短至 246 以下后解决；该失败未触碰 production。

## 3. 首因

production Editor 已 byte-exact 备份并事务替换，`promote_exact` 也成功发布唯一 exact artifact。但是 normal
loader 没有消费已发布的 generated lock lineage，重新进入 `generate_composition_lock`；测试用不存在的 Cargo
路径因此得到 `spawn_failed`，证明不是 `exact_cache/steps=0`。source lineage 目录为
`b4ac81811feb139df2075217b5722ed6a040e55099c8d19b0caa154ba5e53e0a`，说明 Gate H 的 lineage promotion
key/owner 接线仍不闭合。不得用真实 Cargo warm-up 掩盖该 miss。

## 4. 回滚与不变量

失败后立即停止。新 exact entry 与 lineage 被移回 fresh root 的 `backups/rollback-retained` 保留证据，旧 Editor
由 byte-exact backup 恢复并 re-hash。回滚后：

```text
production Editor：F17BEDB2...C786
真实 cache：14 entries
真实 cache inventory：33FAC4E5...183B4
Codex config：8E2D3F3D...53F66
recent projects：2ABE601F...D68EE
trust tree：08255840...C594
Cargo/rustc/editor/generated Editor：0
```

未运行 Local CI，未修改真实配置，未替换 Player/MCP/其它安装态二进制，未清理无关 cache。普通 Editor
open/drag/resize/input smoke 因回滚停止条件未执行。

## 5. 下一步

282 保持当前且不得归档。下一步应单独讨论最小 R2 repair：让 exact cache normal loader 以同一 owner 读取或
事务发布正确的 generated lock lineage，并增加“source lineage -> production promotion -> normal steps=0”的
真实 destination red test。修复、受影响回归与重新资格化完成前，不得再次进入 Gate H。
