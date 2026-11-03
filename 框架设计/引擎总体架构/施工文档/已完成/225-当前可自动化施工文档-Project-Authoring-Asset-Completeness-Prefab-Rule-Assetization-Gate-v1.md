# 225-当前可自动化施工文档-Project Authoring Asset Completeness / Prefab-Rule Assetization Gate v1

## 1. 施工目标

本次施工落地 `225-Project Authoring Asset Completeness / Prefab-Rule Assetization Gate v1` 的 B-min：

```text
扫描复杂打飞机项目当前 authoring assets
  -> 生成 ProjectAuthoringAssetCompletenessReport
  -> 输出 Prefab / Rule 资产化 candidates
  -> 对 sample 执行 C-min 迁移
  -> 让 Prefab / Rule authoring gate 不再固定 partial
```

这不是新增 runtime 功能，而是把复杂打飞机 sample 补齐为当前引擎已经能理解、能编辑、能验证、能被 AI 修改的标准项目资产。

明确不做：

```text
不新增 runtime system。
不新增 Prefab runtime loader。
不新增 Rule VM / Rule Graph editor。
不新增 Feature Folder runtime layer。
不新增打飞机专用 API。
不做完整 Project Template 系统。
不做 Prefab Variant。
不做真实 LLM repair loop。
不做所有普通 Scene Entity 的全自动智能归类。
```

当前施工文档唯一性断言：

```text
施工前 `框架设计/引擎总体架构/施工文档/当前/` 为空。
本文件创建后，该目录只能保留本文件。
正式施工只按本文件执行。
```

## 2. 正式依据

正式方案：

```text
框架设计/引擎总体架构/225-Project-Authoring-Asset-Completeness-Prefab-Rule-Assetization-Gate-v1方案.md
```

外部审查：

```text
本轮用户未提供 225 外部 AI 审查文档。
因此施工自审只按正式 225 方案、项目 skill 流程和当前代码基线执行。
```

当前必须收敛的 sample 缺口：

```text
Prefab:
  samples/complex_shooter_project/Prefabs/ 有 3 个 PrefabAsset。
  samples/complex_shooter_project/Scenes/Main.scene.json 当前没有真实 engine.prefab_instance。

Rule:
  samples/complex_shooter_project/Rules/rule-manifest.json 有 runtime manifest。
  samples/complex_shooter_project/Rules/ 当前缺少对应 .rule.json authoring assets。
```

## 3. 涉及文件

预计新增或修改：

```text
rust/crates/project_e2e_gate/src/authoring_asset_completeness.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/project_e2e_gate/src/prefab_authoring.rs
rust/crates/project_e2e_gate/src/rule_authoring.rs
samples/complex_shooter_project/Scenes/Main.scene.json
samples/complex_shooter_project/Rules/player_move.rule.json
samples/complex_shooter_project/Rules/fire_bullet.rule.json
samples/complex_shooter_project/Rules/lifetime_cleanup.rule.json
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-08-Project-Authoring-Asset-Completeness-Prefab-Rule-Assetization-Gate-v1/00-总览.md
```

如实际施工发现某些文件不需要修改，以最小实现为准。

## 4. Gate A：现状锁定

任务：

```text
确认当前施工目录只有本文件。
确认 225 正式方案采用 B-min。
读取现有 Prefab / Rule authoring report 基线。
锁定 complex shooter sample 的 PrefabAsset、Scene entity、Rule manifest、Rule authoring asset 现状。
```

测试/命令：

```powershell
Get-ChildItem -Path "框架设计/引擎总体架构/施工文档/当前"
rg -n "prefab_instances_count|rule_authoring_assets_missing|ComplexShooterRuleAuthoringStatus|PrefabInstance" rust/crates/project_e2e_gate rust/crates/editor_core
```

通过标准：

```text
施工目录只有本文件。
Prefab / Rule 缺口可由现有 report 定位。
不需要修改 runtime。
```

## 5. Gate B：Completeness report

任务：

```text
新增 ProjectAuthoringAssetCompletenessReport。
扫描 PrefabAsset、Scene PrefabInstance、runtime spawn reference、Rule manifest、Rule authoring asset。
输出 prefab_summary / rule_summary / candidates / diagnostics / next_actions。
Report 分为 editor summary / editor trace，不新增 runtime report。
```

测试/命令：

```powershell
cargo test -p project_e2e_gate authoring_asset_completeness -- --nocapture
```

通过标准：

```text
report 能统计 PrefabAsset 数量。
report 能统计 Scene PrefabInstance 数量。
report 能识别 runtime spawn reference。
report 能列出缺失的 Rule authoring asset。
```

## 6. Gate C：Assetization candidates

任务：

```text
生成 Prefab candidates：
  convert_scene_entity_to_prefab_instance
  prefab_used_by_runtime_spawn_reference
  unused_prefab_asset

生成 Rule candidates：
  migrate_runtime_manifest_entry_to_authoring_rule

所有 candidate 必须携带：
  candidate_id
  domain
  source_path
  target_path
  confidence
  status: ready | blocked | warning
  apply_route
  diagnostics
```

测试/命令：

```powershell
cargo test -p project_e2e_gate authoring_asset_completeness -- --nocapture
```

通过标准：

```text
entity-enemy-a / entity-enemy-b 至少能生成 PrefabInstance 迁移候选。
rule.player-move / rule.fire-bullet / rule.lifetime-cleanup 能生成 Rule authoring asset 迁移候选。
entity-player 不被误判为 player_bullet PrefabInstance。
```

## 7. Gate D：C-min apply

任务：

```text
对 sample 执行最小资产化迁移。
Scene 中至少出现一个真实 engine.prefab_instance。
Rules/ 下出现 player_move.rule.json、fire_bullet.rule.json、lifetime_cleanup.rule.json。
Rule authoring asset 必须可由 RuleAuthoringService load / validate / build，或输出明确 diagnostic。
Prefab 迁移不改 runtime 语义，不新增打飞机专用 API。
```

测试/命令：

```powershell
cargo test -p editor_core prefab -- --nocapture
cargo test -p editor_core rule_authoring -- --nocapture
cargo test -p project_e2e_gate authoring_asset_completeness -- --nocapture
```

通过标准：

```text
sample Scene 有 engine.prefab_instance。
sample Rule authoring assets 存在且可读。
Completeness report 从 partial 缺口转为 passed 或明确无阻塞 warning。
```

## 8. Gate E：E2E gate 收敛

任务：

```text
project_e2e_gate 接入 complex-shooter-authoring-asset-completeness report。
更新 prefab_authoring / rule_authoring 对 sample 的旧 partial 断言。
断言 Prefab / Rule authoring partial 缺口收敛。
确保 RuntimePackage / complex shooter e2e 仍通过。
```

测试/命令：

```powershell
cargo test -p project_e2e_gate prefab_authoring -- --nocapture
cargo test -p project_e2e_gate rule_authoring -- --nocapture
cargo test -p project_e2e_gate -- --nocapture
```

通过标准：

```text
project_e2e_gate 默认测试通过。
Prefab report 不再因为 sample 0 Scene PrefabInstance 固定 partial。
Rule report 不再因为 sample 缺 .rule.json 固定 partial。
RuntimePackage / complex shooter e2e 不回退。
```

## 9. Gate F：回归 / 文档归档

任务：

```text
运行整体回归。
生成阶段完成记录。
更新入口文档。
施工文档归档。
```

测试/命令：

```powershell
cargo fmt --check
cargo test -p editor_core prefab -- --nocapture
cargo test -p editor_core rule_authoring -- --nocapture
cargo test -p project_e2e_gate -- --nocapture
```

文档同步：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-08-Project-Authoring-Asset-Completeness-Prefab-Rule-Assetization-Gate-v1/
```

通过标准：

```text
Gate A-F 全部完成。
默认 headless 测试通过。
施工文档从 当前/ 移动到 已完成/。
当前/ 目录为空。
```

## 10. 施工文档自审

范围自审：

```text
本施工文档只做 authoring asset completeness / migration gate。
没有新增 runtime system。
没有新增 Prefab runtime loader。
没有新增 Rule VM / Rule Graph editor。
没有新增 Feature Folder runtime layer。
没有新增打飞机专用 API。
没有做完整 Project Template / Prefab Variant / LLM repair loop。
```

外部审查自审：

```text
本轮没有提供 225 外部审查文档。
施工文档已记录“无外部审查输入”的判断。
如后续用户提供 225 审查文档，应先回到正式方案与施工文档吸收审查，再继续施工。
```

测试自审：

```text
每个 Gate 都有测试命令。
默认测试为 headless deterministic。
不要求真实 OS window / GPU smoke。
Gate F 包含 fmt、editor_core Prefab / Rule、project_e2e_gate。
```

方案一致性自审：

```text
施工目标与 225 正式方案一致。
Report / Candidate 是 AI 和测试真相，Candidate 不等于静默自动修改。
sample C-min 至少保留一个真实 Scene PrefabInstance。
Rule manifest 不再作为用户默认编辑真相，必须补 .rule.json authoring assets。
```

结论：

```text
本施工文档可执行。
下一步按 Gate A -> F 施工。
```
