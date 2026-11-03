# 168-当前可自动化施工文档-Exported Windows Player Process Verification Gate v1

## 1. 施工目标

依据：

```text
167-Exported-Windows-Player-Process-Verification-Gate-v1方案.md
166-当前可自动化施工文档-Complex-Shooter-Real-Project-End-to-End-Gate-v1.md
133-M3-Native-Windowed-Player-Productization-v1方案.md
134-M4-Native-Player-WindowHost-RuntimePresentIntegration-v1方案.md
```

目标：

```text
建立一个以导出的 Game.exe 进程为最终通过依据的 exported player verification gate。
验证模式只影响 PlayerHost 外壳，不改变正常游戏运行时逻辑。
```

## 2. 涉及文件

预计修改：

```text
rust/crates/runtime_cli/Cargo.toml
rust/crates/runtime_cli/src/lib.rs
```

预计新增：

```text
rust/crates/runtime_cli/src/exported_player_verification.rs
rust/crates/runtime_cli/tests/exported_player_process_verification.rs
```

完成后更新：

```text
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-03-Exported-Windows-Player-Process-Verification-Gate-v1/00-总览.md
```

## 3. 分阶段施工

### A. runtime_cli 增加显式验证入口

任务：

```text
新增 verify-exported-player / --verify-exported-player 参数。
该入口复用 run-native-player 逻辑。
验证模式默认有限帧。
正常 run-native-player 默认行为不变。
```

测试：

```powershell
cargo test -p runtime_cli verify_exported_player_args
```

### B. 新增 exported player process verifier

任务：

```text
新增 exported-player-process-verification-report.v1。
实现 ExportedPlayerProcessVerificationRequest。
验证 package-manifest.json / Game.exe / data/runtime_package。
对外入口是 Game.exe verify-exported-player。
verifier 内部必须再 spawn 导出目录里的 Game.exe run-native-player --mode headless-gate|windowed。
等待进程结束，读取 child report。
父 report 记录 stdout/stderr 摘要、exit_code、child_report_path、status。
```

测试：

```powershell
cargo test -p runtime_cli exported_player_process_verification_report_serializes
cargo test -p runtime_cli exported_player_process_verification_reports_missing_game_exe
```

### C. 真实进程集成测试

任务：

```text
使用 cargo 为 runtime_cli 测试构建出的 ai_engine_runtime_cli.exe。
复制为临时 exported package 的 Game.exe。
准备 data/runtime_package。
调用 verifier 启动 Game.exe 进程。
确保 child report 由 Game.exe 写出。
确保 verifier report 通过。
```

测试：

```powershell
cargo test -p runtime_cli --test exported_player_process_verification
```

### D. 文档收口

任务：

```text
生成阶段完成记录。
将本施工文档移动到 施工文档/已完成。
更新 00 / 49 / 54 / 149 / README。
```

整体测试：

```powershell
cargo test -p runtime_cli
cargo test -p project_e2e_gate
cargo fmt --check
```

## 4. 施工自审

```text
Specification fit:
  施工目标明确以导出的 Game.exe 进程为最终通过依据。

Rule fit:
  不把测试逻辑放进正常运行路径；不新增项目玩法 API。

Textual consistency:
  A-D 阶段从 CLI 入口、process verifier、真实进程测试到文档收口闭合。

Design fit:
  PlayerHost 外壳控制验证模式，Runtime / Renderer / Input 共用正常路径。

Implementation feasibility:
  runtime_cli 已有 run-native-player、默认 package discovery、report 写出能力。

Practical reasonableness:
  第一版自动测试使用 headless-gate 真实进程路径，real-window 后续以同一入口扩展。
```

结论：

```text
本施工文档通过自审，可以开始施工。
```
