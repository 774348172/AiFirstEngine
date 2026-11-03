# 282-R1 Repair Window B / Gate R-E 至 R-G 完成记录

## 1. 结果与边界

```text
状态：R-A 至 R-G 已完成；eligible_to_request_gate_h
日期：2026-08-12
source identity：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留的 dirty worktree
fresh role root：C:\AifeProductionRuns\282-r1-20260812-135636
test harness SHA-256：9642F0F3674D751DDEE44693DE73552A5C9708A7934D7AFE2CCD3DC9FCA8DC37
未执行：Gate H、Local CI、production/installed binary replacement、真实 cache/config mutation
```

Window A 已完成 schema/lineage/resolved identity、path-affine cache v2、typed deadline 与 consumer
closure。Window B 只在上述 repository-external role root 中完成 fresh integration、qualification/promotion
与 Gate H re-entry preflight；真实 Tower 源、production Editor、真实 composition cache 和配置均未修改。

## 2. Gate R-E - Fresh A/B 与 Path-Affinity

run-owned Tower snapshot 排除了 `.aife`、Tests、Design、RuntimeModule/target 等派生/历史目录；仅 snapshot
Cargo manifest 的 Engine dependency 被重定位到受信 `G:/gameEngin/rust` SDK。qualification 需要的
`Library/AssetPipeline` cooked 输入随后补入 snapshot，真实 Tower 项目未修改。

```text
root A：success / cold；generate lock + release + descriptor；release 430838 ms
root B：success / cold；只复制 sealed lineage，无 generate-lock；release 1166976 ms，soft warning 后成功
copied-ct-negative：success / path_affine_miss；release 1130935 ms，soft warning 后成功
crossRootPortable：三者均 false
```

三者 resolved graph digest 均为
`sha256:d7150c7cabaeb067a1ebb69b73c1203820f878361779ea3a1b1ab1b7d564c0af`，resolved artifact key 均为
`sha256:e89ac77c6ed91984d119acf526327e5d0e9ae20ac5a96692097623e2b3af67e0`。只复制 lineage 的 B 没有旧
target affinity，因此正确状态为 `cold`；跨 root `path_affine_miss` 由独立 copied-ct negative 捕获。旧
intermediate 未形成 `same_root_hit`。

Window A 的 typed owner tests 已证明 shortened hard/cancel/terminal/cleanup 矩阵；本窗口两次真实 release
build 证明超 soft budget不会误杀。没有用真实构建等待 1200 秒 hard timeout。

## 3. Gate R-F - Qualification 与 Promotion

首次 qualification 在 R-E 全部成功后 fail-fast：snapshot 缺少 `Library/AssetPipeline`，Play preview 报
FontAssetDatabase 与 texture descriptor missing。补齐 run-owned cooked input 后只续跑 R-F，未重跑 R-E。

```text
qualification：passed；Play/Pause/Step/Resume/Stop 全部 committed
exact promotion：promoted
repeat promotion：exact_cache_hit
normal prepare：exact_cache / steps=0
resolved artifact key：qualification、promotion、normal loader 完全一致
```

## 4. 验证、失败与清理

```text
promotion owner filter：7 passed / 0 failed
qualification owner filter：2 passed / 0 failed
fresh R-F continuation：1 passed / 0 failed
cargo fmt --all -- --check：passed
scoped git diff --check：passed（仅既有 LF/CRLF 提示）
```

Window A 的 `editor_core` 677/2 ignored、winit composition 8/3 ignored 与 real-window check 未被本轮
test-harness-only 改动失效，按规则复用。初次 snapshot 误带 6.76 GB `RuntimeModule/target` 并触及 outer
120 秒上限；确认无进程、strict child 且祖先无 reparse 后只删除该 run-owned child。一次 exact filter 名
不完整实际运行 0 tests；另一次在 build 前因 snapshot 相对 Engine path 不受信 fail-fast。所有日志和 cleanup
record 均保留。最终无 Cargo/rustc/editor owned process遗留，整个 role root 与 evidence 保留。

## 5. Gate H Re-entry Preflight

```text
repair source/test identity：HEAD acaffa8c... + dirty worktree；harness SHA-256 9642F0F3...DC37
fresh A/B evidence、exact artifact、qualification seal：eligible
schema：BuildRequest/Report v3；Artifact/Descriptor/Qualification/Promotion v2
compatibility：resolved graph + stable inputs；target crossRootPortable=false
上次 production Editor只读基线：F17BEDB2...C786
上次真实 cache/config基线：14 entries；Codex 0F728C2D...8A0；recent 2ABE601F...68EE；trust 8DE8D14D...901B
```

结论仅为 `eligible_to_request_gate_h`。新的 Gate H 必须由用户单独授权，并重新获取当前 candidate/hash、
production/cache/config preflight、精确 mutation targets、备份和回滚 authority。282 父施工文档继续保留在
`施工文档/当前/`，不归档。
