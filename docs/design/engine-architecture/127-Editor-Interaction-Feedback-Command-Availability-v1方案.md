# 127-Editor Interaction Feedback / Command Availability v1 方案

## 问题定义

当前 Native Editor 已经有 `EditorUiModel -> SelfUiRenderer -> HitRegion -> EditorInputRouter -> UiCommand -> EditorSession -> CommandResult` 的链路，但交互反馈还不完整：

```text
按钮可见但是否可执行不够统一
禁用原因没有在输入层形成反馈
hover / pressed / disabled 视觉状态不足
命令失败、拒绝、禁用后的用户反馈和 AI 可读反馈不统一
```

本系统只处理编辑器基础交互与命令可用性，不引入项目侧规则。

## 参考引擎

### Unreal Engine

UE 使用 `FUICommandList / FUIAction / CanExecuteAction` 将命令、执行函数、可执行条件分离。Slate 控件负责显示和触发，命令系统负责判断能否执行。

参考源码：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Public\Framework\Commands\UICommandList.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Public\Framework\Commands\UIAction.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Public\Widgets\Input\SButton.h
```

### Unity

Unity 的 Toolbar、Menu、UI Toolkit 控件都有 enabled/disabled 和 command/validation 逻辑。成熟但分散，适合参考体验，不适合照搬结构。

参考源码：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\MenuItem.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Commands\CommandService.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\EditorToolbar\Controls\EditorToolbarButton.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\UIElements\Core\Controls\Button.cs
```

### Godot

Godot 的 `BaseButton / Button / MenuButton / Shortcut / EditorCommandPalette` 更直接，控件承担较多交互状态。优点是简单，缺点是 AI 难以统一解释命令不可用原因。

### Bevy

Bevy UI 的 `Interaction::Pressed / Hovered / None` 与 `InteractionDisabled` 提供清晰底层交互状态参考，但它不是完整编辑器命令系统。

参考源码：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_ui\src\focus.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_ui_widgets\src\button.rs
```

## 推荐路线：C-min

采用接近 UE 的命令可用性路线，但第一版只做最小闭环。

```text
EditorCommandRegistry
  -> CommandAvailability
  -> EditorInteractionState
  -> InputRouter / CommandDispatcher
  -> CommandFeedback / Console / StatusBar / AI Trace
```

## 核心规则

### 1. 命令可用性必须是数据

所有可点击控件必须携带：

```text
command_id
enabled
reason_disabled
```

第一版可以复用 `ToolbarCommand.reason_disabled`、`ProjectLauncherCommand.reason_disabled`，并把信息下沉到 `HitRegion`。

### 2. 禁用点击不得静默失败

点击 disabled hit region 时：

```text
不进入业务 UiCommand
生成 EditorCommandFeedback
写入 NativeEditorApplicationReport
可选写入 Console / WorkspaceReport
```

第一版至少在 report 中暴露。

### 3. hover / pressed 是编辑器输入状态

`NativeEditorApplication` 维护最小交互状态：

```text
hovered_hit_id
pressed_hit_id
last_feedback
```

`SelfUiRenderer` 根据状态绘制：

```text
normal
hovered
pressed
disabled
```

### 4. 执行结果必须形成反馈

命令执行后统一生成：

```text
EditorCommandFeedback {
  command_id
  status
  message
  reason
  source
}
```

第一版写入 `NativeEditorApplicationReport.last_feedback`，后续再接入 StatusBar / Command Palette / 更完整 AI Trace。

### 5. 第一版不做的内容

```text
不做复杂菜单展开
不做快捷键自定义
不做插件命令扩展
不做动画反馈
不做完整 Tooltip 延迟系统
不做项目侧特殊规则
```

## v1 覆盖范围

```text
Toolbar:
  Save / Undo / Redo / Open Runtime Package / Reload / Play / Pause / Step / Tick / Reset

Hierarchy:
  Create / Rename / Delete

ProjectBrowser:
  Select / Open

AI Panel:
  Submit / Accept / Reject

ProjectLauncher:
  Open / Create / Recent
```

## 验收标准

```text
1. disabled command 点击后不执行业务命令，但产生 feedback。
2. Play 没有 Runtime Package 时显示明确禁用原因。
3. Open Runtime Package 成功后 Play 自动变 enabled。
4. Toolbar hover / pressed / disabled 有可见区别。
5. NativeEditorApplicationReport 暴露 hovered_hit_id / pressed_hit_id / last_feedback。
6. editor_input / editor_window_winit / editor_ui_renderer 有最小测试覆盖。
```

