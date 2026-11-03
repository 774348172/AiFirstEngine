# 133-当前可自动化施工文档-M3 Native Windowed Player Productization v1

## 施工目标

按照 `133-M3-Native-Windowed-Player-Productization-v1方案.md` 实施第一版 Native Windowed Player 产品化入口。

本阶段只做长期正确的产品化契约：

```text
RuntimePackage
  -> native player entry
  -> package discovery
  -> headless gate / windowed host handoff
  -> structured report
```

不把 OS window 创建逻辑放入 `engine_runtime`。

## 涉及文件

```text
rust/crates/runtime_cli/src/lib.rs
rust/crates/runtime_cli/Cargo.toml
框架设计/引擎总体架构/133-M3-Native-Windowed-Player-Productization-v1方案.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-01-M3-Native-Windowed-Player-Productization-v1/00-总览.md
```

## Gate A：Native Player 路径发现

实现：

```text
run-native-player
--package <path> 可选
--report <path> 可选
--frames <N> 可选
--headless-gate 可选
--mode headless-gate|windowed 可选
```

package 发现顺序：

```text
1. --package
2. current_exe_dir/data/runtime_package
3. current_dir/data/runtime_package
```

report 发现顺序：

```text
1. --report
2. current_exe_dir/reports/windowed-player-run-report.json
3. current_dir/reports/windowed-player-run-report.json
```

测试：

```powershell
cargo test -p runtime_cli native_player_resolves_explicit_package
cargo test -p runtime_cli native_player_resolves_default_cwd_package
```

## Gate B：Headless 自动化入口

实现：

```text
run-native-player --headless-gate
```

内部调用：

```text
WindowedPlayerHost::run_headless_gate(WindowedPlayerRunRequest::headless_gate(package))
```

测试：

```powershell
cargo test -p runtime_cli native_player_headless_gate_writes_report
```

## Gate C：Windowed Host Handoff

实现：

```text
run-native-player --mode windowed
```

第一版允许返回：

```text
native_window_host_required
```

但必须写入结构化 report，不能只打印普通错误。

测试：

```powershell
cargo test -p runtime_cli native_player_windowed_mode_reports_native_host_required
```

## Gate D：失败路径

实现：

```text
package 缺失
frames 为 0
report 写入失败
```

至少 package 缺失必须产出 report，方便编辑器和 AI 诊断。

测试：

```powershell
cargo test -p runtime_cli native_player_missing_package_writes_report
```

## Gate E：回归测试

运行：

```powershell
cargo fmt --check
cargo test -p runtime_cli
cargo test -p engine_runtime windowed_player
cargo test -p engine_runtime default_game_run
cargo test -p editor_core desktop_export
```

如果时间允许，补充：

```powershell
cargo test -p engine_runtime
cargo test -p editor_core
cargo test -p editor_host
```

## 完成后文档动作

```text
施工文档/当前/133-当前可自动化施工文档-M3-Native-Windowed-Player-Productization-v1.md
  -> 施工文档/已完成/133-当前可自动化施工文档-M3-Native-Windowed-Player-Productization-v1.md

新增：
阶段完成记录/2026-07-01-M3-Native-Windowed-Player-Productization-v1/00-总览.md

更新：
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
施工文档/README.md
```
