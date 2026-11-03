# 282 Window B / Gate E-G 完成记录

## 1. 结果

```text
状态：Window B / Gate E-G 已完成
完成日期：2026-08-11
source identity：HEAD acaffa8cb95bbb54d093bbf45835a59315c82da5 + 保留的 dirty worktree
证据资格：dirty-worktree affected regression + repository-external disposable C2 integration
fresh role root：C:\AifeProductionRuns\282-c2-20260811-204604
未声明：exact-commit、Local CI、production/installed binary replacement、真实配置或真实用户 cache mutation
后续授权：无；Window C / Gate H 仍需单独授权
```

Window B 建立了 executable-bound qualification seal、typed exact promotion request/report、deep cache
promotion owner 与 thin typed admin adapter。source、destination、backup、staging、evidence 都保持显式
role-owned；normal Editor/Gateway、trust receipt 与 production updater 权限没有扩大。

## 2. Gate E - Qualification Seal / Schema

新增 schema：

```text
project-editor-composition-qualification-seal.v1
project-editor-composition-promotion-request.v1
project-editor-composition-promotion-report.v1
```

generated Editor 的 headless/real-window qualification 通过后，由 winit producer 写 report，再调用 core
`ProjectEditorCompositionArtifact::seal_qualification`。core owner 重读 actual executable、descriptor、build
report 与 qualification report，重算 SHA-256 并生成 seal；winit 不复制 hash/seal 算法。failed report 不生成
evidence 或 seal。历史 qualification report 没有 seal 时不能 promotion。

request 使用 `deny_unknown_fields`，要求 portable operation id、显式绝对路径、exact identity，且 source、
destination、backup 必须不同；不存在 search-latest、glob、shell、production replacement 或 config mutation
字段。

## 3. Gate F - Exact Promotion Owner

`ProjectEditorCompositionArtifact::promote_exact` 完成：

```text
authorized source canonical containment
symlink / junction / reparse rejection
identity / descriptor / executable / build report / qualification seal exact validation
destination-owned staging copy 后全量重读与重哈希
valid exact destination no-op hit
invalid destination operation backup
atomic publish + normal cache loader final verification
publish failure byte-exact rollback
rollback failure terminal failed + retained evidence
```

final cache entry 与 promotion staging 使用不同的 strict-child cleanup owner；不会再把 staging 误交给只接受
`cache/<key>` 的删除器。source identity 在 executable path 解析前验证，因此不同项目 candidate 稳定报告
`promotion_identity_mismatch`，不会退化为 executable missing。

Windows junction 矩阵使用 run-owned junction fixture，source/destination reparse root 均被拒绝。thin
`ProjectEditorCompositionCacheAdmin` 只读取 typed request、调用 owner、原子写 typed report。

## 4. Gate G - Disposable Integration

唯一 fresh root：

```text
C:\AifeProductionRuns\282-c2-20260811-204604
  source-qualified
  destination-cache
  backup
  evidence
```

Tower authored project 被复制到 fresh root；仅该副本的 RuntimeModule Engine dependency 被重定位到受信
`G:/gameEngin/rust` SDK，真实 Tower 配置未修改。当前 HEAD generated Editor candidate 最终事实：

```text
identity=sha256:8417ceec7a2875a586042b2528f457bb3eeb7edb89232dc55a534d445ba74dd4
sourceKind=controlled_build
cacheStatus=rebuilt
resolvedJobs=1
requested/effective priority=below_normal / below_normal
priorityApplied=true
final successful release build=109953 ms
executableHash=sha256:0b73b72548b104024b7a5318d6432c6725745e9869dd2d7a03da43ecbd301ce1
```

构建开始时可用内存只允许 `jobs=1`。前两次 attempt 各在 600 秒 owner timeout 后正确终止 owned tree、清理
staging 并保留 run-owned `ct`；第三次复用 compatibility compilation cache 后成功。失败不能改写为通过，
最终 source build report 只记录成功 attempt；前两次 typed timeout 已在执行日志中保留并在本记录披露。

qualification、promotion 与 normal loader：

```text
qualification seal/report：source-qualified/.../qualification/
qualification report digest=sha256:38c9be0c3325487971045ca02c5a4851748bb577876d58439d170e91dde96f5e
exact promotion=promoted
no-op repeat=exact_cache_hit
source/copied/final executable hash 全部为 0b73b725...01ce1
normal prepare status=success
normal prepare sourceKind=exact_cache
normal prepare cacheStatus=hit
normal prepare steps=0
```

normal prepare 的 `cargoExecutable` 被故意指向 fresh root 中不存在的文件；`steps=0` 证明 exact cache hit
没有启动 Cargo/rustc fallback。`evidence/negative-matrix` 保留 13 份 typed report，覆盖 identity、executable、
descriptor、build report、seal、outside-root、source/destination reparse、copy-after-verify mutation、publish
rollback、rollback failure retained evidence，以及 exact/no-op control。

## 5. 验证

```text
cargo test -p editor_core project_editor_composition_promotion_schema_: 2 passed
cargo test -p editor_window_winit project_editor_composition_qualification_seal_: 2 passed
cargo test -p editor_core project_editor_composition_promotion_: 7 passed
cargo test -p editor_core project_editor_composition_cache_admin_: 1 passed
fresh ignored Gate G integration: 1 passed
cargo test -p editor_core -- --test-threads=1: 671 passed / 2 intentional ignored
cargo test -p editor_window_winit: 191 passed / 3 intentional ignored
cargo check -p editor_window_winit --features real-window: passed
cargo fmt --all -- --check: passed
git diff --check（282 tracked scope）: passed
```

首次默认并行 `editor_core` 全量为 669 passed / 2 failed / 2 ignored：一个失败是新增 schema/export 尚未
rustfmt；另一个是两个内部真实 controlled-build tests 并发时 available memory 降到约 3 GiB，单 job build
在 600 秒超时。格式修正后前者通过；后者在系统恢复约 8.8 GiB available memory 后 382 秒通过。最终全量
用单测试线程消除两个 child-build 之间的人工资源竞争并全绿，不修改 QoS policy 或 timeout 来掩盖事实。

## 6. 边界与下一步

production Editor `G:\gameEngin\rust\target\debug\editor_host.exe` 仍为
`F17BEDB2A3892486DED074DE1E4FA1AEF9D4D14CC37E51E35B4155BEBD23C786`，与 281 baseline 相同。没有运行
Local CI，没有替换 production/installed binary，没有修改真实配置、真实用户 cache、trust receipt 或真实
Tower project。fresh root 中无 Cargo/rustc/generated Editor owned process 遗留，evidence 保留不清理。

282 仍保留在 `施工文档/当前/`，不归档、不自动进入 Gate H。下一步必须由用户单独授权
`282 Window C / Gate H`，并在激活前点名 production binary/cache 目标与备份/回滚边界。
