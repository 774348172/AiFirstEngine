# 283 Tower Enemy Scale + GameView Report Hotpath Minimal Repair v1

状态：已完成，用户已于 2026-08-13 授权项目侧尺寸修复与引擎侧逐帧报告写盘最小修复一起执行。

## 目标与边界

- Tower 敌人 Sprite2D 四边形尺寸不超过一个地图格，生成器与场景保持一致。
- 普通 Editor GameView 连续 tick、逐帧 GPU present 与暂停自动 tick 只更新内存报告，不再每帧序列化并原子写盘。
- Start、Pause、Resume、Step、Stop 和启动失败终态继续落盘。
- 不实现渲染插值，不修改报告 schema，不运行 Local CI，不替换 production 二进制，不修改真实配置。

## 必要性审计

```text
confirmed failure
- Tower 敌人 scale=0.9，四边形直径 180px，大于 129px 格子。
- 普通 Editor GameView 实测约 11-15 FPS；连续 tick 当前每帧 pretty-serialize 并 atomic write 完整报告。

minimum causal fix
- 将项目生成尺寸收敛到 0.62，并让验证器执行尺寸合同。
- 连续 tick、逐帧 GPU present 与暂停自动 tick 保留 report_path 但不调用磁盘 writer；生命周期边界仍写盘。

files that must change
- samples/tower_defense_project/Features/EnemySpritePresentation/**
- samples/tower_defense_project/Scenes/Main.scene.json
- rust/crates/editor_core/src/editor_gameview_play.rs

tests that can fail before and pass after the fix
- node samples/tower_defense_project/Features/EnemySpritePresentation/tests/validate-enemy-sprite-presentation.cjs
- cargo test -p editor_core editor_gameview_play::tests::editor_gameview_continuous_tick_keeps_report_in_memory --lib -- --exact
- cargo test -p editor_core editor_gameview --lib

explicitly deferred nearby enhancements
- render interpolation / accumulator
- report mode schema or Report Panel redesign
- production Editor promotion and complete visual matrix
```

## Gate A - Red-capable contracts

1. Add the Tower sprite size assertion.
2. Add a GameView regression proving continuous tick does not change the report file.
3. Run both focused checks against the old behavior and retain the failures in command output.

## Gate B - Minimal repair

1. Update Tower generator and generated scene to scale `0.62`.
2. Keep normal tick report in memory; retain lifecycle-boundary disk writes.
3. Run the two focused checks and the affected `editor_gameview` owner tests.

## Completion

Archive this document only after Gate B passes. Record measured source-level results and explicitly state that production Editor FPS remains unqualified until a later authorized production update/smoke.

## 完成结果

- Tower 敌人 `localScale` 已由 `0.9` 收敛为 `0.62`，40 个生成槽位与场景一致；尺寸合同新增
  `localScale=0.62` 与 `maxQuadToCellRatio=1`，验证器确认 200px quad 不超过 129px 地图格。
- 普通 Editor GameView 的连续 tick、逐帧 GPU present 与暂停自动 tick 不再写盘；Start、显式
  Pause/Resume/Step、Stop 与启动失败仍保留边界报告。
- 新增回归在旧行为下可捕获 normal tick / GPU present 改写报告文件，修复后通过；既有
  `editor_gameview` owner tests 14/14、Tower validator、generator drift check 与 `cargo fmt --all -- --check`
  均通过。
- fresh source-linked release candidate 在真实 1080x1920 WGPU Editor scenario 中，从开战 frame 55 到
  结算 frame 695 用时 10.187 秒，等效约 62.8 FPS；此前约 11-15 FPS 的主瓶颈确认为逐帧报告序列化与
  原子写盘。因此 render interpolation 继续 deferred。
- evidence 位于 `G:\AifeTowerRuns\283-report-hotpath-20260813\evidence\tower-283-report-hotpath-1080`。
- 未运行 Local CI，未替换 production/安装态二进制，未修改真实配置或 production cache；因此
  production Editor 一致性更新仍需后续单独授权。
