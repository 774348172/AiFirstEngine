# 280 Project-Level GameView Target / Editor Selector v1 施工文档

状态：已完成。2026-08-10 Gate A-D 与授权内受影响验证全部通过，准备归档。

正式方案：`280-Project-Level-GameView-Target-Editor-Selector-v1方案.md`

## 1. 施工范围

实现项目设置 -> ProjectSession -> EditorSession -> Play request 的默认 target 接线，并将 GameView
header 的静态比例文案替换为可操作 selector。Tower 项目声明 `720x1280 + Contain`。

禁止范围：Local CI、production/安装态二进制替换、真实配置、Player/BuildProfile/export、字体/MSDF
修复、Tower gameplay 或 AUI layout 修改。

## 2. 基线与冲突复核

- 280 编号未占用。
- `施工文档/当前/` 除 README/artifact 外没有其它活动施工文档。
- 工作区包含 260-279 保留的未提交基线；本施工必须基于当前文件增量修改，不回退任何既有改动。
- 273 已完成 target/render/display/input 能力；本施工只补产品接线。
- 279 production Editor 替换证据不因源码施工自动更新，本施工不得声称 production 已修复。

## 3. Gate A：设置合同与红测

涉及：

```text
rust/crates/engine_runtime/src/game_view_presentation.rs
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/services/project_service.rs
rust/crates/editor_core/src/tests/
```

任务：

1. 为 `GameViewTargetSpec` 提供 owner-level validation。
2. 增加 typed `ProjectSettingsDocument` 和可选 Editor Preview target。
3. ProjectLauncher 从 safe project scope 读取并验证 settings。
4. create/open/recent 三条入口采用项目 target；缺失字段保持 legacy default。
5. 先增加能捕获 Tower open 后仍为横屏 Stretch 的红测，再实现修复。

验证：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core project_settings_game_view_target -- --nocapture
cargo test -p editor_core toolbar_play_uses_the_session_game_view_target -- --nocapture
```

## 4. Gate B：GameView selector 与 typed command

涉及：

```text
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/viewport.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_ui_renderer/src/panels/viewport.rs
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_window_winit/src/command_system.rs
```

任务：

1. 增加 typed SetGameViewTarget command 与 target view model。
2. header 提供三个 preset 与 scale policy controls，使用 272 control style/pseudo-state。
3. selector 对窄 panel 不重叠；当前值可见；活动 Play 时禁用。
4. command 更新 current session，下一次 Play 使用新 target。

验证：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_ui_model game_view_target -- --nocapture
cargo test -p editor_ui_renderer game_view_target -- --nocapture
cargo test -p editor_input game_view_target -- --nocapture
cargo test -p editor_core game_view_target -- --nocapture
```

## 5. Gate C：Tower consumer

涉及：

```text
samples/tower_defense_project/Settings/project_settings.json
samples/tower_defense_project/AUI/battle-hud.aui.json（只读断言）
```

任务：Tower 声明 `720x1280 + Contain`，并以测试证明 AUI reference 仍为 `1080x1920`。

验证：复用 Gate A project-open seam，加 Tower fixture read；不重建 production cache。

## 6. Gate D：受影响回归与闭环

```powershell
cd G:\gameEngin\rust
cargo fmt --check
cargo test -p engine_runtime game_view_presentation -- --nocapture
cargo test -p editor_core project_settings_game_view_target -- --nocapture
cargo test -p editor_core game_view_target -- --nocapture
cargo test -p editor_ui_renderer game_view_target -- --nocapture
cargo test -p editor_input game_view_target -- --nocapture
```

只在 owner 定向测试揭示真实 consumer 缺口时增加受影响测试；不机械运行完整 workspace 或 Local CI。

完成后同步 `49`、`54`、施工文档 README、阶段完成记录 README，生成 280 完成记录并归档本文。

## 7. 施工文档自审

- 授权匹配：通过；用户明确要求开始该引擎侧产品接线。
- 唯一施工槽：通过。
- 方案一致：通过；严格采用方案 B。
- 红测：Gate A 明确要求先证明 ordinary project-open target 失败。
- 影响面：覆盖 runtime validator、core owner、UI model/renderer/input consumer 与 Tower project setting。
- 外部状态：不替换 production、不写真实配置、不运行 Local CI。
- 三小时限制：预计可在单一窗口完成；若到 2h45m 不开启新阶段。

## 8. 完成结果

- 红测先观察到 ordinary OpenProject 保持 `1280x720 + Stretch`，随后转绿为项目声明的
  `720x1280 + Contain`。
- `ProjectSettingsDocument`、target validation、open/recent adoption、session-scoped selector、typed
  input command 与 Play-active guard 已贯通。
- Tower project settings 已声明 `720x1280 + Contain`；3 个 AUI Canvas 仍保持 `1080x1920` reference。
- owner/consumer 定向测试、winit command registry、localization catalog、fmt 与 scoped diff check 通过。
- 未运行 Local CI，未替换 production/安装态二进制，未修改真实配置，未重建 Tower Preview cache。
