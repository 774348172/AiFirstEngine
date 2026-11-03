# 250-F 当前可自动化施工文档：C-01 Golden Gate v1

> 状态：已生成、自审并激活；当前唯一施工文档  
> 对应方案：`250-AI-Primary-ProjectProduction-Dual-Path-v1方案.md` Phase 6  
> 验收协议：`审查目录/其它AI审查目录/51-P0-0.5-v7-人工协作2D战斗竞技场对比协议.md`  
> 前置完成：250-A/B/C/D/E  
> 生成日期：2026-07-15  
> 激活日期：2026-07-15

## 1. 施工目标

从人工已创建的真实空项目开始，只通过正式项目生产表面完成本引擎 C-01 A 通道：

```text
I:\EngineTest\AiFirstGame
  -> imported Codex ProjectCandidateEnvelope
  -> prepare
  -> validate
  -> explicit approval
  -> apply
  -> save / close / reopen
  -> Preview
  -> Windows Export
  -> external copied package no-arg run
  -> deterministic C-01 evidence
```

每个候选仍只承载一种 `ProjectPatch`、`ControlledSourcePatch` 或 `AssetImport` payload。多候选严格串行，每次 apply 后重新读取 project binding/base digest，不伪造跨 payload 原子性。

Phase 6 必须同时完成两件事：

1. 用通用 ProjectProduction 能力补齐真实 C-01 首次暴露的产品缺口。
2. 在真实项目中生成、保存、重开、预览和导出完整 C-01，并留下可复核报告。

## 2. 冻结输入与公平边界

真实目标：

```text
project_root = I:\EngineTest\AiFirstGame
engine_sdk_root = G:\gameEngin\rust
candidate_store_root = G:\gameEngin-targets\250f\candidates
engine_target_root = G:\gameEngin-targets\250f
external_export_root = I:\EngineTest\Exports\AiFirstGame-C01
evidence_root = G:\gameEngin-targets\250f\evidence
```

只允许从 `samples/complex_shooter_project/Assets/Images/` 复用以下 PNG 原始字节：

| 文件 | SHA-256 |
|---|---|
| `tex-starfield.png` | `f27a14785320e386f4c6b1f5bcf04f5e79ee1bfdfc06336a9ed3b356e67f6141` |
| `tex-player-ship.png` | `9657a0141f9224d656776f8e478062ea51a15ce8152c1cce59006677f41bb80c` |
| `tex-enemy-scout.png` | `7f7917a45bbdd352158e2a8a24411535d8306a8ec6d6e6eaf34c1e5e7307a803` |
| `tex-bullet.png` | `1b246eb588c49078dd242e5c3d478202a9b463cab05d67c3a94eeab87ce8fe17` |

禁止复制样例的 Scene、Prefab、Rule、AUI、RuntimeModule、BuildProfile、Cargo manifest/lockfile、测试或构建产物。允许把现有引擎 API 和 schema 当作只读接口参考；C-01 项目实现必须独立生成并归项目所有。

本阶段不操作 Unity、UE 项目，不执行 B 通道，不改写协议 51 中 2026-07-14 的历史 blocked 记录。

## 3. 已确认的 Phase 6 通用缺口

激活前代码复核确认以下三项是 C-01 必经缺口：

```text
1. ProjectPatch 可创建 Scene entity，但不能为 Scene/Prefab authoring entity 添加或删除通用 component。
2. Rule authoring 可创建/构建单个 RuleAsset，但没有正式项目级 rule-manifest 生成 surface。
3. Controlled SourcePatch 可验证 ProjectRust library，但 Preview/Export 仍只查找引擎 workspace 内预编译 player；没有从项目 RuntimeModule 生成可运行 Player 的通用 Dev Host/Artifact owner。
```

施工只补通用接口：

```text
Scene.AddComponent / Scene.RemoveComponent（或等价深命令）
Rule.BuildProjectManifest（或等价 deterministic manifest owner）
ProjectRuntimePlayerArtifactBuilder / Project Runtime Dev Host（或等价通用 owner）
```

禁止新增 `Player`、`Enemy`、`Bullet`、`Score`、`HP`、`Wave`、`Dash` 等 Engine Core operation/API。它们只允许存在于项目 Scene/Prefab/Rule/AUI/RuntimeModule 和 C-01 gate fixture/evidence 中。

## 4. C-01 项目要求

必须覆盖协议 51 的 C-R1 至 C-R9：

```text
1280x720 orthographic 2D；真实 starfield/player/enemy/bullet texture。
WASD 移动和 viewport clamp。
LeftShift dash：300 ms、3x speed、2 s cooldown，冷却期重触发可检查。
Space hold fire：6 shots/s。
可复用 enemy prefab；bullet/enemy、enemy/player collision。
HP=3、score +100、Game Over、R restart。
每波 >=5 enemy，波次确定性增强。
AUI HUD：HP、Score、Wave、Dash READY/COOLDOWN x.x、Game Over/Restart。
Save/close/reopen/Preview。
Windows external copied package，零参数真实窗口运行并正常退出。
canonical project digest、asset hash、domain validation、runtime transition、截图和 120+600 CPU frame sample。
```

## 5. Phase 6 Owner 与报告

在 `project_e2e_gate` 新增面向任意项目根的 C-01 owner，不复用写死的 complex-shooter wrapper：

```text
run_c01_golden_gate(C01GoldenGateRequest) -> C01GoldenGateReport
```

报告至少包含：

```text
schema_version / status / scenario_id
project_root / initial_project_digest / final_project_digest
candidate sequence：source/envelope/candidate/validation/approval/apply receipt digest
first_blocker / repairs / rollback receipts
frozen asset source/destination hashes
Scene/Input/AUI/Logic/Prefab/RuntimeModule validation
save/reopen/preview/export/process/screenshot evidence
runtime transition assertions
performance warmup_frames=120 / sample_frames=600 / mean/p95/p99 CPU frame ms
artifacts / diagnostics / next_actions
```

Runner 只能编排正式 `editor_core` owner；不得直接写真实项目文件。测试 fixture 可在隔离临时项目中构造基线，但真实 `I:\EngineTest\AiFirstGame` 的 C-01 mutation 必须全部有候选与 receipt。

## 6. Gate A：Baseline、Schema 与隔离环境

施工前预检：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-targets\250f'
cargo test -p editor_core project_candidate_entry
cargo test -p editor_core controlled_source_patch
cargo test -p editor_core project_asset_import
cargo test -p project_e2e_gate --lib
```

检查并记录：

```text
真实项目 readiness、project id、canonical digest 和 0-entity Scene。
四张冻结 PNG 的 exact SHA-256。
candidate/evidence/export roots 均不位于项目树和引擎源码树内。
外部 export root 不存在或可由 typed external output policy 安全替换。
```

Gate A 只允许生成报告目录和测试产物，不修改真实项目。

## 7. Gate B：ProjectPatch 通用 Authoring 完整度

施工：

```text
为 Scene authoring 增加通用 add/remove component command、transaction、undo/rollback evidence。
ProjectPatch schema/validator/applier/repair-scope/transaction 支持对应 operation。
Prefab 可从带自定义 component 的 Scene selection 正式创建，并保持 component 数据。
Rule authoring 增加 deterministic project runtime manifest 生成/验证命令；manifest 只从已保存、已验证 RuleAsset 生成。
所有新写入继续使用 ProjectWriteScope 和原子保存。
```

定向测试：

```powershell
cargo test -p editor_core scene_component_authoring
cargo test -p editor_core project_patch_scene_component
cargo test -p editor_core project_patch_prefab_component
cargo test -p editor_core project_rule_manifest
cargo test -p editor_core project_candidate_entry_project_patch
```

否定矩阵：missing entity、duplicate/missing component、invalid component id/fields、inverse rollback、manifest rule id/artifact mismatch、base/context drift、tampered candidate/receipt。

## 8. Gate C：ProjectRust Dev Host / Player Artifact

施工：

```text
建立通用 ProjectRuntime Player artifact builder；输入是已验证 ProjectRust manifest、RuntimeModule 和 trusted Engine SDK。
生成的 host source/manifest 位于隔离 build root，不写入项目 RuntimeModule，不要求用户维护 host boilerplate。
host 只调用项目 crate 的稳定 linked_set/module descriptor contract，并链接正式 runtime_cli/player host。
Preview 与 Export 从同一个 artifact identity/descriptor 输入消费，不再要求项目 package 预先属于 engine workspace。
构建输出、command、timeout、stdout/stderr、descriptor、hash 和 cleanup 进入结构化 report。
```

安全边界：不使用 dylib 热加载；不执行未通过 Controlled SourcePatch validation 的候选；不允许 project build.rs、.cargo config、额外依赖或输出逃逸；外层 timeout 大于内部 stage timeout 总和。

定向测试：

```powershell
cargo test -p editor_core project_runtime_player_artifact
cargo test -p editor_core project_rust_preview
cargo test -p editor_core project_rust_export
cargo test -p runtime_cli exported_player
```

## 9. Gate D：真实项目候选施工

候选按下列顺序逐个执行，每个步骤都必须：

```text
inspect binding -> envelope -> prepare -> validate -> approved_by=local-maintainer-c01
-> apply -> receipt -> re-inspect binding -> persist evidence
```

顺序：

```text
D1-D4：四个 AssetImport 候选，导入冻结 PNG、生成 asset/meta/DB/Graph provenance。
D5：ControlledSourcePatch 候选，建立 ProjectRust manifest/Cargo/runtime source/tests。
D6：ProjectPatch 候选，建立 Input action/binding。
D7：ProjectPatch 候选，建立 Scene entities/components、真实 sprite references 和 orthographic settings。
D8：ProjectPatch 候选，从 Scene selection 创建 enemy prefab，并在 Scene 中留下 prefab instance/reference evidence。
D9：ProjectPatch 候选，建立 Rule assets、artifact 和 project runtime rule manifest。
D10：ProjectPatch 候选，建立并保存 AUI HUD 文档与 binding。
```

若某候选失败：保留 first blocker；未 apply 的候选直接丢弃；已 apply 且必须撤销时只使用对应 receipt rollback。禁止直接修真实项目；修复必须生成下一候选并增加 repair count。

## 10. Gate E：Save、Close、Reopen 与 Preview

```text
Save All 后记录 project digest。
关闭原 EditorSession/进程语义，创建全新 Session 通过正式入口重开。
复核 Scene/Input/AUI/Rule/Prefab/asset references/ProjectRust readiness 与重开前一致。
通过 ProjectRuntimePackageAssembler 构建 Preview package。
通过 ProjectRust Dev Host 运行 Preview，并执行确定性输入/碰撞/波次/Game Over/Restart scenario。
保存 Preview screenshot；真实纹理像素和真实 glyph present 是 blocking evidence。
```

定向命令：

```powershell
cargo test -p project_e2e_gate c01_save_reopen
cargo test -p project_e2e_gate c01_runtime_transitions
cargo test -p project_e2e_gate c01_preview
```

## 11. Gate F：Windows External Export、Screenshot 与性能

```text
使用正式 DesktopExport/ReleasePackage owner 导出。
把最终 package 发布到 I:\EngineTest\Exports\AiFirstGame-C01，不留在项目树或引擎树。
从 external root 零参数启动真实 Game executable。
保存真实窗口截图和 process lifecycle report。
执行 120 帧 warm-up + 600 帧 CPU sample，输出 mean/p95/p99。
复核 Preview/Export 的 module descriptor、RuntimePackage、texture、HUD 和 gameplay evidence 一致。
```

真实窗口或截图失败不得被 headless 计数替代；必须记录 failed/blocked，而不是 partial 伪通过。

## 12. Gate G：受影响域与权威回归

先跑受影响域：

```powershell
$env:CARGO_TARGET_DIR='G:\gameEngin-targets\250f'
cargo test -p editor_core
cargo test -p project_e2e_gate --lib
cargo test -p runtime_cli
cargo test -p runtime_player_winit
cargo fmt --all -- --check
git diff --check
```

候选代码、manifest、lockfile、project digest 和 C-01 evidence 冻结后，权威矩阵各运行一次：

```powershell
cargo test --workspace
cargo test --workspace --all-features
```

当前主工作树继承用户与 249/250-A-E 的混合未提交变更，不能冒充 clean exact commit。Phase 6 使用独立 `G:\gameEngin-targets\250f`，完成记录必须报告实际 run 次数、耗时、首个失败、修复后累计失败集、artifact hash 和遗留风险。

## 13. 预计涉及文件

```text
rust/crates/editor_ui_model/src/...                         generic component/rule commands
rust/crates/editor_core/src/scene_editing.rs                component authoring transaction
rust/crates/editor_core/src/project_patch/{model,validator,applier,session,repair_scope}.rs
rust/crates/editor_core/src/rule_authoring.rs                project rule manifest owner
rust/crates/editor_core/src/project_player_artifact.rs       ProjectRust host builder
rust/crates/editor_core/src/editor_preview_package.rs        ProjectRust preview consumer
rust/crates/editor_core/src/desktop_export.rs                ProjectRust export consumer
rust/crates/project_e2e_gate/src/c01_golden_gate.rs          Phase 6 owner/report
rust/crates/project_e2e_gate/src/bin/...                     explicit C-01 runner mode/entry
rust/crates/project_e2e_gate/src/lib.rs                      exports
对应定向测试
```

真实项目中由候选创建的文件不列为引擎源码修改；它们必须在 C-01 report 的 changed-path inventory 中逐项列出。

## 14. 明确不做

```text
不复制 complex-shooter 项目实现，不把它改名后当成 C-01。
不直接写 I:\EngineTest\AiFirstGame 绕过候选链。
不新增 gameplay-specific Engine Core API、通用脚本语言或膨胀 Rule IR。
不伪造多 payload 原子事务，不批量跳过 approval。
不修改 Unity/UE，不开始 B 通道，不改写 V-01 或旧 Run 结论。
不做 Provider Registry、Agent Planner、热加载、在线热更或任意依赖执行。
不以 mock/headless draw count 替代真实 texture/glyph/window/screenshot。
```

## 15. 施工文档自审

```text
正式方案一致性：通过。本文只执行 250 Phase 6，并继承单 payload candidate、双 lowering、review/apply/rollback 真相。
协议一致性：通过。完整继承 51 的 C-R1..C-R9、冻结资源、公平限制和 A 通道边界。
前置复核：通过。250-A/B/C/D/E 均有完成记录；激活前 当前/为空、待执行/仅 README。
真实基线：通过。目标仍是 0-entity 空项目；四张 PNG hash 与协议一致。
缺口真实性：通过。component authoring、project rule manifest、ProjectRust Player artifact 均由当前代码直接证实，不凭历史记忆猜测。
深 Module：通过。新增 owner 分别隐藏 component transaction、manifest generation 和 ProjectRust host build；C-01 owner 只编排，不复制底层事务。
玩法边界：通过。所有 C-01 语义留在项目资产/源码与 gate evidence，不进入通用 Runtime/ECS/Renderer API。
范围：通过。只补 C-01 必经通用缺口；Unity/UE、B 通道、Provider/Planner 和热更排除。
测试：通过。包含定向、受影响域、default/all-features、真实项目、真实窗口、截图、external package 和性能证据。
失败纪律：通过。first blocker、repair count、receipt rollback、累计失败集和禁止直写均已明确。
混合工作树：已记录，不清理、不重置、不回滚既有修改，不声称 clean exact commit。
```

结论：250-F 可激活施工。本文是 `施工文档/当前/` 唯一非 README 文档；Gate A 通过前不得修改真实项目，Gate D 前不得直接生成 C-01 项目文件，Gate G 与真实 evidence 未通过前不得归档或声称 Phase 6 完成。

## 16. 2026-07-15 Gate G 执行记录

状态：Gate G 受影响域与两套 workspace 权威矩阵通过；250-F 暂不归档，Phase 6 暂不声明完成。

受影响域：

```text
cargo test -p editor_core
  first run: failed，460 passed / 6 failed / 2 ignored，154.5 s
  first blocker: ProjectPatch Gate B 文件快照合同落地后，四个旧 Scene 测试仍使用无 active project 的裸 Scene 夹具；
                 Input rollback history 未保存 apply 前文件快照；
                 candidate non-last 测试仍假设 Scene patch 不改变 project digest。
  repair: ProjectPatch history 增加 session-bound file snapshot companion；rollback 恢复精确字节；
          Create/Open/Recent Select 清空项目绑定 history 与 capability；
          Scene 测试迁移到正式项目内 Scene；candidate intervening patch 改为不写文件的 Asset validation。
  cumulative failure set: cargo test -p editor_core project_patch -> 79 passed / 0 failed
  rerun: passed，467 passed / 0 failed / 2 ignored，115.4 s

cargo test -p project_e2e_gate --lib
  passed，54 passed / 0 failed / 1 ignored，94.6 s

cargo test -p runtime_cli
  passed，unit/integration/doc tests 全绿，32.5 s

cargo test -p runtime_player_winit
  passed，17 passed / 0 failed，4.2 s

cargo fmt --all -- --check
  passed

git diff --check
  passed；仅输出既有 Windows LF/CRLF worktree warning
```

冻结候选后的权威矩阵：

```text
cargo test --workspace
  passed，authoritative run 1/1，331.3 s

cargo test --workspace --all-features
  passed，authoritative run 1/1，361.6 s
```

环境与证据边界：

```text
CARGO_TARGET_DIR=G:\gameEngin-targets\250f
CARGO_PROFILE_TEST_DEBUG=0
worktree=mixed_uncommitted；不得解释为 clean exact commit CI
structured Gate G report=G:\gameEngin-targets\250f\evidence\gate-g-regression-report.json
```

剩余阻断：

```text
G:\gameEngin-targets\250f\evidence\c01-golden-gate-report.json
  schemaVersion=c01-golden-gate-report.v2
  projectRoot=I:\EngineTest\AiFirstGame
  Preview/Export screenshot hash 相同，且没有 screenshotVisual；该报告属于已确认的旧假阳性，不能作为完成证据。

隔离临时项目的 v3 定向复验已证明真实 texture、player blue、enemy red 和 HUD glyph 可见，
但不能冒充 I:\EngineTest\AiFirstGame 的正式证据。
下一步必须提供 provider-independent validation-only Candidate Entry/re-entry：
对已经施工完成的真实项目只做 save/reopen/Preview/Export/外部包/截图 v3 重验，禁止重放 D1-D10 十个候选。
```

结论：Gate G 代码回归通过；真实项目 v3 evidence 仍未通过，因此 250-F 继续保留在 `施工文档/当前/`。

## 17. 2026-07-15 Validation-only Re-entry 与最终完成记录

新增 provider-independent `validate_existing_c01_project` 与 CLI `c01-validation-only`。该入口读取并验证既有 D1-D10 envelope/validation/approval/receipt 链、projectId、当前项目语义完整性和冻结资源 hash，只执行 Save、close/reopen、RuntimePackage、Preview、Windows Export、external no-arg run、截图与 v3 像素验收；不 prepare/apply/rollback candidate，不修改玩法内容。定向测试以全文件 hash 证明项目与 candidate store 在 validation prepare 前后不变。

真实项目首次 re-entry 暴露 ProjectRust Player artifact cache 未包含 Engine SDK 内容摘要，导致复用渲染修复前的旧 executable。通用修复将 `engine_runtime`、`engine_input`、`runtime_cli`、`runtime_player_winit` 与 workspace manifest/lock 的内容摘要加入 artifact identity/report；隔离 Dev Host 禁用 dev PDB 与 incremental；artifact 集成测试改用短路径、独立 identity 与 RAII cleanup，避免缓存泄漏。

真实权威证据：

```text
project=I:\EngineTest\AiFirstGame
report=G:\gameEngin-targets\250f\evidence\c01-golden-gate-report.json
report schema/status/mode=c01-golden-gate-report.v3 / passed / validation_only
candidate evidence=10/10，未重放
Preview=7837 colors / 10845 blue / 12862 red / 9714 HUD white
Export=6904 colors / 10845 blue / 6391 red / 9726 HUD white
performance=120 warmup + 600 sample / mean 17.0363 ms / p95 17.2959 ms / p99 18.5007 ms
report SHA-256=279B21B6EF70AC7A3DAF1321A74CB0ADFA36F7432E346B3833900997A0663B5C
preview PNG SHA-256=7FABD16C10573058212D923D356FB51190223A7F11AAB7F3CFE474A8835BDE62
export PNG SHA-256=0F4541DC299030FFD11400ABC439498CA4A8452A71F0E3221ECCEDD977E999AC
```

最终回归：

```text
cargo test -p editor_core：通过
cargo test -p project_e2e_gate --lib：54 passed / 0 failed / 1 ignored
cargo test -p runtime_cli：通过
cargo test -p runtime_player_winit：通过
cargo fmt --all -- --check：通过
git diff --check：通过，仅既有 LF/CRLF warning
cargo test --workspace：最终通过，296.9 s
cargo test --workspace --all-features：最终通过，264.2 s
```

all-features 首轮先暴露 `LNK1318 PDB LIMIT`，修复 PDB/incremental 后第二轮暴露专用 target 磁盘耗尽；清理约 43 GB 历史/测试 artifact 并修复测试隔离与清理后，精确失败集及两套完整矩阵均通过。当前是 mixed uncommitted worktree，不声称 clean exact-commit CI。

结论：Gate A-G、真实项目 Preview/Export、external package、v3 像素证据与最终回归全部通过；250-F / Phase 6 / 250 整体完成，可生成完成记录并归档。
