# 231-当前可自动化施工文档-Exported Windows Playable Golden Gate v1

> 状态：当前施工文档。  
> 方案来源：`框架设计/引擎总体架构/231-Exported-Windows-Playable-Golden-Gate-v1方案.md`。  
> 外部审查：已搜索 `其它AI审查目录/` 与 `4.8AI审查目录/`，未发现 231 专项审查文档；本轮按正式方案自审结论施工。  
> 施工日期：2026-07-09。  
> 当前施工目录同一时间只能保留这一份施工文档。

## 1. 施工目标

把 228 / 229 / 230 已完成的真实贴图、真实玩法规则、真实 HUD state 证据，收敛成一个 P0-4 golden gate：

```text
Complex Shooter Sample Project
  -> DesktopExportPipeline::export
  -> exported package / Game.exe
  -> verify_exported_player_process
  -> child windowed-player-run-report.json
  -> ExportedWindowsPlayableGoldenGateReport
```

本轮必须证明：

```text
导出产物存在 package-manifest.json / data/runtime_package / executable。
blocking gate 使用 exported process contract，不用 in-process helper 冒充。
导出的 player child process 能完成 headless deterministic run。
P0-4 report 汇总 texture / gameplay / HUD / AUI glyph evidence。
optional real-window evidence 被诚实分档，但 skipped 不降低 top-level passed。
P0-4 完成态不能以 partial 收尾。
```

## 2. 涉及文件

预计修改：

```text
rust/crates/project_e2e_gate/Cargo.toml
rust/crates/project_e2e_gate/src/exported_windows_playable_golden.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-09-Exported-Windows-Playable-Golden-Gate-v1/00-总览.md
```

可能只读或轻微扩展：

```text
rust/crates/runtime_cli/src/exported_player_verification.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/project_e2e_gate/src/real_texture_present.rs
rust/crates/project_e2e_gate/src/gameplay_rule_runtime.rs
rust/crates/project_e2e_gate/src/project_rule_driven_ui_state.rs
rust/crates/editor_core/src/desktop_export.rs
```

禁止修改方向：

```text
不把 Player / Enemy / Bullet / Score 写入 engine core API。
不重写 DesktopExportPipeline。
不绕过 RuntimePackage 读取项目源目录。
不新增大型自动化框架或完整 GPU pixel diff。
不把 optional real-window 变成默认 CI blocking。
```

## 3. Gate A：Report schema / fixture

任务：

```text
新增 ExportedWindowsPlayableGoldenGateReport。
新增 package / process / goldenEvidence / textureEvidence / gameplayEvidence / hudEvidence / realWindowEvidence / reportMode / artifacts / diagnostics 字段。
字段必须包含 target_os / actual_host_os / executable_name。
字段必须包含 input_source / input_script_id / fire_action_observed / projectile_spawn_observed。
序列化测试覆盖 passed 与 optional real-window skipped。
```

测试：

```powershell
cargo test -p project_e2e_gate exported_windows_playable_golden_report
```

## 4. Gate B：Exported package + exported process verification

任务：

```text
project_e2e_gate 新增 P0-4 runner。
runner 先通过 DesktopExportPipeline 导出 complex shooter sample。
runner 必须调用 runtime_cli::verify_exported_player_process，或严格复用同一 exported process contract。
校验 executable / package-manifest.json / data/runtime_package。
校验 child exit_code == 0、child report exists、frames_completed >= requested_frames。
P0-4 report 写入 exported_package_dir / game_exe_path / child_report_path。
```

测试：

```powershell
cargo test -p runtime_cli exported_player_process_verification
cargo test -p project_e2e_gate exported_windows_playable_golden_process
```

## 5. Gate C：P0 evidence 汇总断言

任务：

```text
汇总 228 real_texture_present report 关键证据。
汇总 229 gameplay_rule_runtime report 关键证据。
汇总 230 project_rule_driven_ui_state report 关键证据。
每条 evidence 标明 source_domain / source_report_path。
断言 texture_status / gameplay_status / hud_status / aui_status / render_status 都通过。
断言 score_text_matches_score_after。
断言 rendered_glyph_count > 0。
```

测试：

```powershell
cargo test -p project_e2e_gate real_texture_present_report_covers_228_gate
cargo test -p project_e2e_gate complex_shooter_gameplay_rule_runtime_execution_report_covers_229_gate
cargo test -p project_e2e_gate complex_shooter_project_rule_driven_ui_state_report_covers_230_gate
cargo test -p project_e2e_gate exported_windows_playable_golden_evidence
```

## 6. Gate D：Optional real-window evidence 分档

任务：

```text
realWindowEvidence.status 支持 captured / feature_disabled / environment_blocked / local_only_skipped。
realWindowEvidence.blocking 默认 false。
optional real-window skipped 不得把 top-level status 降级为 partial。
若启用真实窗口或截图，只追加 screenshot summary，不影响默认 headless gate。
```

测试：

```powershell
cargo test -p project_e2e_gate exported_windows_playable_golden_optional_real_window
```

## 7. Gate E：Report Panel / AI context 最小接入

任务：

```text
让 project_e2e_gate artifact 能输出 complex-shooter-exported-windows-playable-golden-gate-report.json。
如果现有 unified Report Panel provider 可注册 project_e2e artifact，则补最小 provider/evidence entry。
如果当前 Report Panel 只需要读取 project_e2e artifact，则不新增面板结构，只在 report artifacts 中声明。
```

测试：

```powershell
cargo test -p project_e2e_gate exported_windows_playable_golden
cargo test -p project_e2e_gate unified_report_panel
```

## 8. Gate F：整体回归与文档归档

任务：

```text
运行格式化与相关 crate 回归。
生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
把本施工文档从 当前/ 移动到 已完成/。
```

测试：

```powershell
cargo fmt --check
cargo test -p runtime_cli exported_player_process_verification
cargo test -p project_e2e_gate exported_windows_playable_golden
cargo test -p project_e2e_gate real_texture_present_report_covers_228_gate
cargo test -p project_e2e_gate complex_shooter_gameplay_rule_runtime_execution_report_covers_229_gate
cargo test -p project_e2e_gate complex_shooter_project_rule_driven_ui_state_report_covers_230_gate
cargo test -p project_e2e_gate
```

## 9. 施工文档自审

```text
是否符合 231 正式方案：
  是。采用 B-min：Packaged Player Golden Gate + Optional Real Window Evidence。

是否存在外部 231 审查文档：
  未发现。已搜索 其它AI审查目录/ 与 4.8AI审查目录/，施工按正式方案自审约束执行。

是否扩大范围：
  否。不做 installer/signing/store package，不做 mandatory real-window，不做大型 Gauntlet 克隆。

是否遵守 exported process contract：
  是。Gate B 明确要求 verify_exported_player_process 或同一契约，禁止 in-process helper 冒充。

是否保护 RuntimePackage 真相：
  是。验证对象是导出 package、executable、package manifest、data/runtime_package 与 child report。

是否保护项目逻辑边界：
  是。P0-4 只汇总 evidence，不把复杂打飞机玩法写入 engine core API。

是否处理 runtime/editor report 分档：
  是。reportMode 保留 runtime/editor Off/Summary/Trace；默认 runtime 不常驻重 trace。

是否分 Gate 测试：
  是。Gate A-F 都有明确测试命令，施工中必须逐 Gate 运行。

是否允许最终 partial：
  否。optional real-window skipped 不降级；P0-4 完成态必须 passed，否则继续修复 blocking evidence。
```
