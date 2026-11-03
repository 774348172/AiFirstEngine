# 128-当前可自动化施工文档：Playable Windows Export Vertical Slice v1

## 施工目标

实现第一版 Windows 可玩导出纵向切片：

```text
Saved Editor Project
  -> RuntimePackageBuilder
  -> staged Windows package
  -> player gate
  -> reports
```

## 涉及文件

预计新增：

```text
rust/crates/editor_core/src/desktop_export.rs
```

预计修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/engine_runtime/src/windowed_player.rs
rust/crates/runtime_cli/src/lib.rs
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
```

## Gate 1：Editor Saved Project -> RuntimePackageBuilder

任务：

```text
读取 project.aife.json
读取 default scene
把 EditorSceneDocument 转为 RuntimeScene
构造 RuntimePackageBuildInput
调用 RuntimePackageBuilder::build
```

测试：

```powershell
cargo test -p editor_core desktop_export_builds_runtime_package_from_saved_project
```

## Gate 2：Windows Package Stage

任务：

```text
创建 Build/Windows/dev/
创建 data/runtime_package/
创建 data/assets/
创建 reports/
写 package-manifest.json
写 desktop-export-report.json
```

测试：

```powershell
cargo test -p editor_core desktop_export_stages_windows_package_layout
```

## Gate 3：Player Gate

任务：

```text
从 staged data/runtime_package 运行 WindowedPlayerHost headless gate
生成 windowed-player-run-report.json
将结果汇总到 desktop-export-report.json
```

测试：

```powershell
cargo test -p editor_core desktop_export_runs_player_gate_from_staged_package
```

## Gate 4：CLI / 回归

任务：

```text
保证 runtime_cli / editor_core / engine_runtime 回归通过
必要时补最小 CLI 或 helper，不做完整 UI
```

测试：

```powershell
cargo fmt --check
cargo test -p editor_core desktop_export
cargo test -p engine_runtime windowed_player
cargo test -p runtime_cli
```

## 完成后文档同步

完成后：

```text
施工文档/当前/128... 移入 施工文档/已完成/
新增 阶段完成记录/2026-07-01-Playable-Windows-Export-Vertical-Slice-v1/00-总览.md
更新 49-当前状态与下一步入口.md
更新 54-当前可自动化施工总入口.md
```

