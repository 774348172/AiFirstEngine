# 217-当前可自动化施工文档：Editor Play / RuntimePackage Preview Productization v1

## 1. 施工目标

按 `217-Editor-Play-RuntimePackage-Preview-Productization-v1方案.md` 的 `B-min-cache` 落地第一版真实链路：

```text
Editor Play 自动准备 Preview RuntimePackage
Preview RuntimePackage cache manifest
dirty domain fingerprint
cache hit / missing / stale / rebuilt / failed report
PlaySessionReport 链接 PreviewPackageReport
Toolbar Play 在项目已打开时可用，不再要求用户手动 Open RuntimePackage
Report Panel / Console 输出 preview package 证据
project_e2e_gate 增加 editor Play cache gate
```

本施工必须吸收 `其它AI审查目录/35-217-Editor-Play-RuntimePackage-Preview方案审查.md`：

```text
明确 runner 策略：
  217 v1 只接现有 PlaySession HeadlessGate。
  不实现 WindowedUserRun / Embedded GameView。
  不把 runtime_player_winit 强行接入 editor_core。

明确 windowed deferred：
  真实 windowed/GameView preview 后续单独讨论。
  本轮不把 headless gate 冒充为真实窗口预览。

Gate E 必须新增 editor Play e2e 接入：
  通过 EditorSession::start_play_session 触发 preview package + headless Play。
  不只验证 DesktopExport / native player。

Report 必须记录 deferred：
  windowed_game_view_play_runner_deferred=true。
  aui_input_snapshot_runner_unification_deferred=true。
```

## 2. 涉及文件

预计新增：

```text
rust/crates/editor_core/src/editor_preview_package.rs
```

预计修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/services/mod.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_core/src/play_session.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/tests/play_service_tests.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/project_e2e_gate/src/*
框架设计/引擎总体架构/217-Editor-Play-RuntimePackage-Preview-Productization-v1方案.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-07-Editor-Play-RuntimePackage-Preview-Productization-v1/00-总览.md
```

禁止修改：

```text
不要让 Runtime 读取项目源目录。
不要绕过 ProjectRuntimePackageAssembler。
不要让 Play 默认执行 DesktopExportPipeline。
不要实现或假装实现 WindowedUserRun / Embedded GameView。
不要新增 runtime 架构层。
不要改动与本施工无关的既有工作区改动。
```

## 3. Gate A：schema / report / PlaySession link

任务：

```text
新增 EditorPreviewPackageRequest。
新增 EditorPreviewPackageFingerprint。
新增 EditorPreviewPackageCacheManifest。
新增 EditorPlayPreviewPackageReport。
新增 EditorPreviewPackageStageReport / Diagnostic / Status / CacheStatus / DirtyDomain。
PlaySessionReport 增加 preview_package_report_path / preview_cache_status / preview_dirty_domains / preview_prepare_duration_ms。
EditorSession 保存 last_editor_preview_package_report。
Report Panel 预留 preview package provider。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_preview_package_schema
```

## 4. Gate B：fingerprint / cache lookup

任务：

```text
实现稳定 source fingerprint。
fingerprint 覆盖 Project / BuildProfile / Scene / Prefab / Asset / Rule / Aui / Input / FontAtlas / EngineSchema。
实现 preview-cache-manifest.json read/write。
实现 cache hit / missing / stale 判断。
cache hit 时输出 build_runtime_package skipped stage。
fingerprint/check_cache stage 输出 duration_ms。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_preview_package_cache
```

## 5. Gate C：prepare preview package

任务：

```text
cache missing / stale 时调用 ProjectRuntimePackageAssembler。
调用 RuntimePackageBuilder 写 .aife/editor-preview/<profile>/<active_scene>/runtime_package。
调用 RuntimePackageLoader 验证输出。
写 editor-play-preview-package-report.json。
构建失败时 status=failed，PlaySession 不启动。
不调用 DesktopExportPipeline，不 stage player executable。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_preview_package_prepare
cargo test -p engine_runtime runtime_package_builder
```

## 6. Gate D：Play service integration

任务：

```text
EditorSession::start_play_session：
  如果已有手动打开 RuntimePackage 且没有 active_project_session，可沿用 debug/manual 路径。
  如果项目已打开，先调用 EditorPreviewPackageService::prepare。
  使用 prepared runtime_package_path 构造 PlaySessionRequest::headless_gate。
  PreviewPackageReport 失败时不启动 PlaySession。

Toolbar：
  项目已打开时 Play enabled。
  无项目且无 RuntimePackage 时 Play disabled。
  运行中仍 disabled。

Console：
  输出 preview cache status / dirty domains / duration / report path。
```

测试：

```powershell
cd rust
cargo test -p editor_core play_service
cargo test -p editor_core editor_session_play
```

## 7. Gate E：editor Play e2e cache gate

任务：

```text
project_e2e_gate 新增 editor_play_preview_runtime_package_cache gate。
通过 EditorSession 打开 samples/complex_shooter_project。
第一次 Play 生成 Preview RuntimePackage。
第二次 Play cache hit，并证明 build_runtime_package stage skipped=true。
修改 test fixture 中的 Scene 或复制临时项目后修改 Scene，第三次 Play cache stale，dirtyDomains 包含 Scene。
本 Gate 只验证 editor Play HeadlessGate，不验证 runtime_player_winit windowed/GameView。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate editor_play_preview_runtime_package_cache
```

## 8. Gate F：Report Panel / UI model / timing

任务：

```text
Report Panel 显示 Editor Preview Package report。
Workspace Play domain summary 包含 preview cache status。
duration_ms 分 stage 输出。
deferred_flags 包含 windowed_game_view_play_runner_deferred 和 aui_input_snapshot_runner_unification_deferred。
```

测试：

```powershell
cd rust
cargo test -p editor_core report_panel
cargo test -p editor_core ui_model
```

## 9. 整体回归

全部 Gate 完成后运行：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_core editor_preview_package
cargo test -p editor_core play_service
cargo test -p project_e2e_gate editor_play_preview_runtime_package_cache
cargo test -p project_e2e_gate
```

如整体回归暴露与本施工无关的既有失败，必须在最终报告中说明；不得为通过测试回退无关改动。

## 10. 施工文档自审

范围自审：

```text
本施工只做 Preview RuntimePackage cache + Editor Play HeadlessGate 接入。
本施工不做 WindowedUserRun。
本施工不做 Embedded GameView。
本施工不把 runtime_player_winit 加成 editor_core 依赖。
本施工不绕过 RuntimePackage 真相。
```

审查吸收自审：

```text
35 号审查的高风险 runner 问题已通过明确 deferred 收敛。
35 号审查的 editor Play e2e 缺口已进入 Gate E。
35 号审查的 fingerprint duration / autosave 失败 / cache 清理低风险项进入 report/deferred。
```

测试自审：

```text
每个 Gate 都有 cargo test 命令。
cache hit 必须有 skipped=true 证据。
构建失败不启动 PlaySession。
整体回归包含 editor_core 和 project_e2e_gate。
```

## 11. 完成后文档同步与归档

完成后必须：

```text
生成阶段完成记录：
  框架设计/引擎总体架构/阶段完成记录/2026-07-07-Editor-Play-RuntimePackage-Preview-Productization-v1/00-总览.md

更新入口：
  框架设计/引擎总体架构/49-当前状态与下一步入口.md
  框架设计/引擎总体架构/54-当前可自动化施工总入口.md
  框架设计/引擎总体架构/施工文档/README.md
  框架设计/引擎总体架构/阶段完成记录/README.md

归档施工文档：
  从 施工文档/当前/
  移到 施工文档/已完成/
```
