# 121-Native Editor Application Shell 方案

## 1. 结论

本项目确认选择：

```text
方案 C：一步设计完整 Unity / UE 式编辑器框架
```

这里的“完整”指：

```text
完整定义编辑器应用壳、服务边界、数据流、命令流、布局流、输入流、事务流。
```

不代表第一版一次实现所有高级功能：

```text
完整 Dock 拖拽
完整菜单系统
完整快捷键编辑器
完整 Inspector 控件库
完整 SceneView Gizmo
完整 Asset Browser
完整插件市场
```

第一版施工必须按完整框架切入，不能继续在静态窗口上零散补按钮。

## 2. 当前底层能否支持方案 C

判断：

```text
可以支持启动方案 C。
不能支持直接堆完整编辑器功能。
```

当前已经具备：

```text
EditorSession
EditorUiModel
SelfUiRenderer
UiDrawList
HitRegion
EditorInputRouter
UiCommand
CommandTransaction / SceneEditTransaction
editor_wgpu_renderer
Real WGPU surface present
FontSystem glyph atlas
headless tests
```

当前关键缺口：

```text
真实窗口只持有静态 EditorUiModel，不持有 EditorSession。
真实窗口事件只处理 Close / Resize / Redraw。
CursorMoved / MouseInput / KeyboardInput / TextInput / IME 没有进入编辑器主链路。
没有 Application Shell 级焦点、capture、hover、active panel。
没有 CommandRegistry / ShortcutMap。
没有 PanelRegistry / PanelLifecycle。
没有 DockLayoutManager。
没有 EditorServiceRegistry。
没有统一 invalidation / rebuild / request_redraw 流。
没有真实 TextInput editing state。
```

所以现在的问题不是“Hierarchy / Inspector 不够丰富”，而是：

```text
缺 NativeEditorApplicationShell。
```

## 3. 成熟引擎对比

### Unreal Engine

UE 对应结构：

```text
FSlateApplication
FMainFrameModule
FGlobalTabmanager / FTabManager
FUICommandList
FScopedTransaction / GUndo
PropertyEditor / DetailsView
LevelEditor / SLevelViewport
```

UE 特点：

```text
应用壳边界非常强。
输入、焦点、窗口、绘制、命令、布局、事务都有中心系统。
适合复杂大型编辑器。
缺点是历史复杂度高，Slate / UObject / Module 体系较重。
```

### Unity

Unity 对应结构：

```text
EditorApplication
ContainerWindow
GUIView
HostView
EditorWindow
SceneView
InspectorWindow
Undo / SerializedObject / SerializedProperty
```

Unity 特点：

```text
用户心智更直接。
Hierarchy / Inspector / SceneView / Project / Console 是清晰工作台。
缺点是公开源码只覆盖 C# 层，底层原生窗口和 UI 渲染细节不可完全参考。
```

### Godot

Godot 对应结构：

```text
EditorNode
EditorDock / EditorDockManager
EditorInspector
EditorUndoRedoManager
DisplayServer / Window / Viewport
```

Godot 特点：

```text
结构短，整体性强。
适合学习“不要把编辑器搞成过重体系”。
缺点是复杂商业生产管线能力弱于 UE。
```

### Bevy

Bevy 对应结构：

```text
App runner
winit event loop
Schedule
ECS world
```

Bevy 特点：

```text
窗口 / 事件 / schedule / ECS 分层清晰。
但 Bevy 不是完整 Unity/UE 式编辑器参考。
```

## 4. 我们的完整框架

目标结构：

```text
NativeEditorApplication
  -> PlatformWindowLayer(winit)
  -> EditorMainFrame
  -> DockLayoutManager
  -> PanelRegistry
  -> EditorFocusInputSystem
  -> EditorCommandSystem
  -> EditorTransactionService
  -> EditorSelectionService
  -> EditorServiceRegistry
  -> EditorUiModelComposer
  -> SelfUiRenderer / UiRenderBackend
  -> EditorSession / EditorCore
  -> RuntimePreview / PlaySession
```

核心数据流：

```text
WindowEvent
  -> NativeEditorApplication.handle_event
  -> EditorFocusInputSystem
  -> hit_test / focus / capture / text_input
  -> EditorCommand
  -> EditorCommandSystem.dispatch
  -> EditorSession.execute_command / EditorService
  -> EditorTransactionService
  -> state changed
  -> invalidate affected panels
  -> EditorUiModelComposer rebuild
  -> SelfUiRenderer build UiDrawList
  -> UiRenderBackend present
```

## 5. 核心模块职责

### NativeEditorApplication

职责：

```text
拥有 EditorSession。
拥有 EditorMainFrame。
拥有 Focus / Input / Command / Transaction / Selection / Layout / Services。
接收 winit WindowEvent。
控制 redraw / rebuild / present。
输出 diagnostics / report。
```

禁止：

```text
不写项目玩法规则。
不直接改 Scene 数据。
不把 wgpu / winit 类型暴露给 EditorCore。
```

### PlatformWindowLayer

职责：

```text
创建窗口。
转换平台事件。
管理 surface resize。
只输出 PlatformWindowEvent / EditorInputEvent。
```

长期规则：

```text
winit 是第一版平台窗口后端。
EditorCore 不依赖 winit。
EditorUiModel 不依赖 wgpu。
```

### EditorMainFrame

职责：

```text
定义主窗口结构。
管理 Toolbar / MainMenu / DockRoot / StatusBar。
引用 DockLayoutManager 生成可见面板布局。
```

第一版可以固定布局：

```text
Toolbar 顶部
Hierarchy 左侧
Viewport 中央
Inspector 右侧
Console / RuntimeTrace 底部
AI Panel 右下或底部 tab
```

但类型设计必须按 Layout Tree：

```text
DockRoot
DockSplit
DockStack
PanelSlot
```

### PanelRegistry

职责：

```text
注册 panel_id。
定义 panel title / default area / lifecycle / model builder / command handler。
```

第一版内置 panel：

```text
Toolbar
Hierarchy
Inspector
Viewport
Console
RuntimeTrace
AI Panel
ProjectDock
```

规则：

```text
Panel 不互相直接调用。
Panel 只读 EditorUiModel / Services snapshot。
Panel 输出 EditorCommand。
```

### DockLayoutManager

职责：

```text
持有布局树。
根据窗口尺寸计算 panel rect。
保存/恢复布局。
处理布局缺失和版本不匹配。
```

第一版：

```text
固定布局 + layout schema + headless layout test。
```

后续：

```text
拖拽 docking。
tab reorder。
floating window。
layout profile。
```

### EditorFocusInputSystem

职责：

```text
cursor position
hovered region
active panel
keyboard focus
mouse capture
drag state
text editing state
IME composition state
```

规则：

```text
平台事件先进 FocusInputSystem。
FocusInputSystem 决定事件是 UI 消费、SceneView 工具消费、还是 GameView Runtime 输入。
```

### EditorCommandSystem

职责：

```text
注册 command_id。
保存 command metadata。
处理 can_execute。
处理 shortcut。
分发 command。
记录 command trace。
```

来源统一：

```text
Toolbar click
Menu item
Shortcut
Panel action
Inspector field edit
AI proposed command accept
```

都进入：

```text
EditorCommandSystem -> EditorSession / EditorService
```

### EditorTransactionService

职责：

```text
把可撤销编辑包进 CommandTransaction / SceneEditTransaction。
提供 undo / redo。
提供 command result / diagnostics。
```

规则：

```text
121 不新建第二套 Undo。
121 只把现有 Transaction 能力提升为 Application Shell service。
```

### EditorUiModelComposer

职责：

```text
从 EditorSession / services / panel states 组合 EditorUiModel。
集中处理 model revision / dirty panels / diagnostics。
```

规则：

```text
EditorUiModel 是 UI 真相。
UiDrawList 是渲染产物，不是业务真相。
```

### EditorServiceRegistry

职责：

```text
集中管理 ProjectService / AssetService / BuildService / RuntimeService / AiService / ReportService。
```

规则：

```text
Service 只提供引擎底座能力。
不为具体游戏项目增加敌人、子弹、血量等专用 API。
```

## 6. AI 友好规则

AI 不直接操作窗口、面板或底层状态。

AI 只能生成：

```text
EditorCommand
CommandPatch
PanelIntent
DiagnosticQuery
```

AI 输入应包含：

```text
current selection
visible panels
focused panel
available commands
schema summary
recent diagnostics
recent command trace
```

AI 输出必须可解释：

```text
proposed_commands
risk_summary
requires_confirmation
expected_effect
rollback_hint
```

这样 AI 修改的是“编辑器命令层”，不是“某个按钮的 callback”。

## 7. 为什么不是继续 120 的路线

120 的价值：

```text
证明 EditorUiModel / DrawList / HitRegion / UiCommand / EditorSession 链路可行。
```

120 的上限：

```text
它仍然更像可用面板补强。
真实窗口没有 Application Shell。
继续补单个控件会让焦点、输入、事务、布局状态散落到多个 crate。
```

121 要做的是把 120 收进正式应用壳：

```text
120: panel usable
121: editor application owns panels
```

## 8. 第一版施工边界

虽然选择方案 C，但第一版施工仍必须可测试、可回归。

第一版必须完成：

```text
新增 NativeEditorApplication 类型。
NativeEditorApplication 持有 EditorSession，不再只持有静态 EditorUiModel。
建立 EditorMainFrame / PanelRegistry / DockLayoutManager 基础类型。
建立 EditorFocusInputSystem 基础类型。
建立 EditorCommandSystem 基础类型。
建立 EditorTransactionService wrapper。
真实窗口事件进入 NativeEditorApplication。
Mouse click 能经过 focus/hit_test/command/session/rebuild/present。
Keyboard shortcut 能进入 CommandSystem。
Text input 状态有正式归属。
UiModel rebuild / request_redraw 有统一路径。
所有关键路径有 headless test。
```

第一版不必须完成：

```text
拖拽 Docking。
复杂多窗口。
完整菜单编辑器。
完整文本编辑控件。
完整 Inspector 控件库。
真实 AI 模型调用。
```

## 9. 验收测试

必须有 headless 测试：

```text
NativeEditorApplication can create default shell。
PanelRegistry contains required panels。
DockLayoutManager computes stable rects。
Mouse click hierarchy item -> SelectSceneEntity -> EditorSession -> UiModel rebuild。
Toolbar Save/Undo/Redo -> CommandSystem -> TransactionService。
Keyboard shortcut Ctrl+Z -> UndoSceneEdit。
Focused text field receives TextInput without leaking to Runtime GameView。
GameView focused key input routes to RuntimeInputFrame。
AI proposed command accept routes through CommandSystem。
Redraw invalidation increments model revision and draw list revision。
```

真实窗口测试：

```text
cargo check real-window / real-wgpu-surface。
real-window smoke 可本机手动验证，不作为唯一验收。
```

## 10. 和 UE / Unity 的差异

| 项目 | UE | Unity | 我们 |
|---|---|---|---|
| 应用壳 | FSlateApplication + MainFrame | EditorApplication + ContainerWindow | NativeEditorApplication |
| 面板容器 | FTabManager / SDockTab | HostView / EditorWindow | PanelRegistry + DockLayoutManager |
| 输入路由 | WidgetPath tunnel/bubble | GUIView / UIElements event | FocusInputSystem + HitRegion |
| 命令 | FUICommandList | MenuItem / Shortcut / EditorWindow callback | EditorCommandSystem |
| Undo | FScopedTransaction / GUndo | Undo / SerializedObject | EditorTransactionService |
| Inspector | PropertyEditor / Details | SerializedProperty / Inspector | Schema-driven Inspector |
| AI 友好 | 弱 | 中等 | 强，命令和状态结构化 |
| 复杂项目能力 | 很强 | 强 | 目标强 |
| 第一版复杂度 | 已成熟 | 已成熟 | 我们最高，但可控 |

## 11. 核心规则

```text
NativeEditorApplication 是编辑器最高层应用壳。
真实窗口不直接持有静态 EditorUiModel 作为长期路线。
真实窗口事件必须进入 NativeEditorApplication。
Panel 不互相直接调用。
Panel 只输出 EditorCommand。
所有可撤销修改必须经过 EditorTransactionService。
EditorCore 不依赖 winit / wgpu。
EditorUiModel 不依赖 winit / wgpu。
UiDrawList 不是业务状态真相。
AI 不绕过 EditorCommandSystem。
引擎只提供底座能力，不为具体游戏项目增加专用规则。
```

## 12. 下一步

下一步不是继续讨论某个具体按钮，而是生成施工文档：

```text
121-当前可自动化施工文档-Native-Editor-Application-Shell-方案C.md
```

施工顺序应按：

```text
Application Shell 类型
PanelRegistry / Layout
FocusInput
CommandSystem
TransactionService wrapper
真实窗口接入
Headless tests
```
