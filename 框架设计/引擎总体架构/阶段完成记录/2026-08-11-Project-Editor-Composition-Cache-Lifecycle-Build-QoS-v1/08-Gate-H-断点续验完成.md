# 282 Gate H 断点续验完成记录

## 结果

```text
状态：通过
日期：2026-08-13
复用 run root：C:\AifeProductionRuns\282-r1-gate-h-20260812-150654
resume evidence：C:\AifeProductionRuns\282-r1-gate-h-20260812-150654\resume-20260812
production Editor：5007CDA6E32036D2F5FAD5A4CBD58B6A1AA9ED20E21E23DC905A66D42A5B9BC2
artifact key：0b4ed21bbbadd92cea528bfe849e9c7cc999face98fc02588e8c339896a482aa
lineage key：b4ac81811feb139df2075217b5722ed6a040e55099c8d19b0caa154ba5e53e0a
```

本次没有重建 candidate、冷 composition、qualification 或 seal。旧 Gate H 的 candidate、artifact executable、
descriptor、build report、qualification report/seal 与 generated-lock lineage 均重新计算 SHA-256，并与 retained
evidence 一致；R2-M 只改变 `#[cfg(test)]` harness，不改变这些产品输入，因此复用资格成立。

## 断点续验

production Editor 原件 `F17BEDB2...C786` 已 byte-exact 备份。首次续验中 `promote_exact` 成功，但只发布
exact cache entry，normal loader 因真实 lineage 缺失进入 generate-lock；该次 mutation 立即完整回滚，Editor、
cache 与 lineage 恢复到 mutation 前状态。

source 与旧 rollback evidence 中的 lineage 两份逐字节一致。随后把 exact cache entry 与对应 sealed lineage
作为旧 qualified artifact 的一组输入恢复，再运行修复后的同一 Gate H tail harness：

```text
project_editor_composition_282_gate_h_promote_and_normal_hit
1 passed; 0 failed; 198 filtered out
sourceKind=exact_cache
cacheStatus=hit
steps=[]
```

不存在的 `must-not-start-cargo.exe` 未被启动，证明 normal loader 没有进入编译链。

## 普通 Editor Smoke

exact cache 中的真实项目 Editor 使用 run-owned Tower snapshot、recent store 与 workspace state 执行 7 步场景：

```text
open recent -> workspace ready -> Play -> runtime active
-> click recruit-button / observe td.recruit -> Stop -> runtime stopped
```

结果为 `passed`，耗时 `1,364 ms`，diagnostics 为空。输入报告证明 720x1280 Contain target、GameView
坐标转换、PointerUp route 与 `td.recruit` action 均被真实观察。

## 最终状态

- production Editor：`5007CDA6...B9BC2`；旧二进制备份保留。
- exact artifact executable：`4C3351F6...FCB6`；真实 cache 命中。
- lineage `Cargo.lock`：`1CFCCC1E...7A3D`；`lineage.json`：`08EF2A98...1618`。
- owned Editor/Cargo/rustc process：0。
- 未运行 Local CI，未修改真实配置，未替换 Player/MCP/其它安装态二进制，未运行完整视觉矩阵。

282 Gate A-H 全部完成，施工文档可以归档。
