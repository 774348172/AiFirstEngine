# 303 post-302 Production Editor / Tower Preview Consistency Minimal v1

## 状态与边界

```text
用户授权：2026-08-19
状态：唯一 Gate 已完成，待归档
目标：让普通 production Editor 与真实 Tower Preview cache 消费已完成的 302 source
允许：fresh candidate build、production Editor/cache 精确备份与回滚、仅 Tower scene-main cache 重建、720/1080 显示检查
禁止：Local CI、完整 E2E/视觉矩阵、Player/MCP/其它安装态二进制、真实配置、Windows export、其它 cache
```

## 唯一 Gate

1. 在 `G:\AifeProductionRuns\post-302-consistency-20260819-191926` 记录 source、production Editor、Tower cache 与进程 preflight。
2. 用 fresh run-owned `CARGO_TARGET_DIR` 构建标准 debug Editor candidate：
   `cargo build -p editor_host --locked --features real-window,real-wgpu-surface`。
3. candidate 无参 readiness 必须在 30 秒内 exit 0；记录 dirty source identity 与 candidate SHA-256，不声明 clean/exact-commit qualification。
4. byte-exact 备份 production Editor 与完整 Tower `scene-main` cache；校验备份 hash/tree digest。
5. 只替换 production Editor，并且 candidate/production-after SHA-256 必须相同；只移出并重建 Tower `scene-main` cache。
6. 打开真实 Tower，在 720x1280 与 1080x1920 target 检查 `FPS: N`、中文文字方向/清晰度与整体竖屏显示。
7. 任一 mutation 后失败立即恢复旧 Editor 与旧 cache；成功则保留备份和证据、打开 Tower，并归档本记录。

## 验证经济性

```text
requested claim: ordinary production Editor contains 302 and Tower presents corrected text at two portrait targets
minimum work: one Editor build + one exact Editor/cache transaction + two bounded visual captures
excluded: product-source changes, Player/export, business matrix, Local CI, unrelated caches/configuration
red-capable evidence: candidate/after hash, backup/cache manifests, rebuilt cache manifest, 720/1080 captures
stop condition: readiness/build failure, hash mismatch, cache rebuild failure, or rollback failure
```

激活前复核：无其它活动施工文档；Editor/Cargo/rustc 为 0；production Editor 与 Tower cache 可读取；302 source closure 已归档。范围与本次授权一致，自审通过。

## 执行结果

```text
run root: G:\AifeProductionRuns\post-302-consistency-20260819-191926
candidate build: passed, 230.8s
candidate / production-after SHA-256: BF3C2302EEA4D0FC0C0718ECED73FC767ADFF91EA8AED4AFBF0F54663C93F58A
production-before / backup SHA-256: 7FDF496ED411480BAA041D26D2C700D9FB895027ADF3C62C0DB643C01D306A9C
old cache backup: 93 files / 20,106,165 bytes / tree 68226CE5...9C77B33
rebuilt cache: 93 files / 20,099,101 bytes / manifest F687C1EF...C63DC68
ordinary Editor Open / Play / Stop: passed
720x1280: passed; FPS: N 正序，中文正序、无缺字；截图 SHA-256 3BCDF604...669AD3
1080x1920: passed; frame target exact，FPS: N 正序，中文正序、无缺字；截图 SHA-256 4D30E8B8...5DAE10
normal-hit 1080 Play: 2.268s, Cargo/rustc spawn 0
rollback: not required; backups retained
terminal residue before final reopen: Editor/Cargo/rustc 0
```

未运行 Local CI、完整 E2E/视觉矩阵；未修改真实配置，未替换 Player/MCP/其它二进制，未重建其它 cache。
