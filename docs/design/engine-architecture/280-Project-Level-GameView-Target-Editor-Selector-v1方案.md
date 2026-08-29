# 280 Project-Level GameView Target / Editor Selector v1 方案

状态：方案 B 已由用户于 2026-08-10 确认，并授权开始施工。

## 1. 问题

273 已完成 ReferenceSpace、Runtime TargetSpace、Editor DisplaySpace 与 input inverse 的统一，
但普通 Editor 打开项目后仍由 `EditorSession::default()` 得到 legacy `1280x720 + Stretch`。
只有 production authority scenario 能显式注入竖屏 target。Tower AUI 使用 `1080x1920`
reference resolution，因此普通 Play 会产生非等比拉伸和严重文字压扁。

## 2. 目标

建立项目无关、schema-first 的普通 Editor 产品链：

```text
Settings/project_settings.json
  -> ProjectLauncher typed load/validation
  -> ProjectSession preferred Editor Preview target
  -> EditorSession current GameView target
  -> existing PlaySessionRequest / GameViewPresentationModule
  -> Renderer / display / input inverse
```

同时把 GameView header 中的静态 `16:9 Landscape` 文案替换为真实可操作 target selector。

## 3. 已确认方案 B

### 3.1 项目默认值

`aife-project-settings.v1` 以可选、向后兼容字段扩展：

```json
{
  "schemaVersion": "aife-project-settings.v1",
  "projectName": "Tower Defense",
  "editorPreview": {
    "gameViewTarget": {
      "extent": { "width": 720, "height": 1280 },
      "scalePolicy": "contain"
    }
  }
}
```

缺少 `editorPreview.gameViewTarget` 的旧项目继续得到明确 legacy target，不隐式读取 AUI Canvas，
不按项目名称或项目 ID 硬编码。

### 3.2 会话覆盖

GameView header 提供 `1280x720`、`1080x1920`、`720x1280` 预设与 `Contain/Stretch`
策略选择。选择操作只覆盖当前 EditorSession，不自动写项目文件。Play/Preparing/Stopping 期间
控制禁用；新 target 只对下一次 Play 生效。

### 3.3 边界

- AUI `reference_resolution` 仍属于 Canvas layout space，不被修改为 target extent。
- 复用 273 `GameViewTargetSpec` 与 presentation/input mapping，不新增 Tower 专用 runtime 分支。
- 本系统只拥有 Editor Preview target；Player window、BuildProfile、Windows export 与移动设备策略延期。
- 不修改 production/安装态二进制、不修改真实用户配置、不运行 Local CI。

## 4. 错误策略

- settings 文件缺失、schema 不匹配、target 为零或超过 capability 时，项目打开失败并返回 typed diagnostic；
  不静默回退后继续声称已采用项目 target。
- settings 字段缺失是兼容输入，使用 legacy default。
- UI command 再次通过同一 target validator；Play 活动期间拒绝修改。

## 5. Owner 与 Consumer

```text
engine_runtime/game_view_presentation.rs
  GameViewTargetSpec validation owner

editor_core/project_launcher.rs
  ProjectSettingsDocument load/validation owner

editor_core/services/project_service.rs
  project-open -> EditorSession target adoption owner

editor_ui_model / editor_ui_renderer / editor_input
  target view model、selector present 与 typed command

editor_core/services/play_service.rs
  session override 与 existing Play request consumer

samples/tower_defense_project/Settings/project_settings.json
  project-owned preferred target consumer
```

## 6. 验收

1. Tower 普通 project open 后 current target 为 `720x1280 + Contain`。
2. 缺少可选字段的旧项目仍为 `1280x720 + Stretch`。
3. 非法 project target 在 open seam 得到稳定诊断。
4. selector 显示真实 current target，并可在未 Play 时切换 preset/policy。
5. Toolbar Play 使用选定 target；活动 Play 期间不能产生 target identity 漂移。
6. Tower source settings、AUI reference resolution 与 GameView target 三者边界明确。

## 7. 方案自审

- 与 273 一致：是，只补普通产品入口，不重做 presentation module。
- 项目无关：是，Core 只识别 extent/policy，不识别 Tower。
- schema-first：是，项目设置、UI command 和 diagnostics 均为 typed contract。
- 向后兼容：是，新增字段可选；缺失保持 legacy behavior。
- 范围控制：通过；Player/Build/export、字体策略和 production replacement 均不在本方案。

