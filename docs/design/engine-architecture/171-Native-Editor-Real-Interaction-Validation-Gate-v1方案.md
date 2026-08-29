# 171-Native Editor Real Interaction Validation Gate v1 方案

## 1. 问题是什么

当前编辑器已经有 Application Shell、Workspace、Inspector、Command Framework、UI renderer、WGPU present 等基础能力。

但还缺一个系统级门禁证明：

```text
真实 Native Editor Window / 等价可回放事件
  -> UI hit test
  -> focus / hover / pressed / disabled
  -> EditorCommandRequest / UiCommand
  -> EditorSession 执行业务
  -> EditorUiModel 刷新
  -> DrawList / report 形成证据
```

这个系统不是新增某个面板，也不是给某个按钮单独补点击逻辑。它的目标是建立长期规则：以后编辑器真实交互是否可用，要通过统一 scenario gate 验收。

## 2. 其他引擎怎么做

Unreal Engine：

```text
FSlateApplication
  -> SWidget hit / focus / FReply
  -> FUICommandList / FUICommandInfo
  -> LevelEditor / Details / ContentBrowser / Toolkit
```

源码参考：

```text
Engine/Source/Runtime/Slate
Engine/Source/Runtime/SlateCore
Engine/Source/Editor/UnrealEd
Engine/Source/Editor/LevelEditor
```

Unity：

```text
GUIView / HostView / EditorWindow
  -> UI Toolkit EventDispatcher
  -> PointerDownEvent / ClickEvent / IMGUIContainer
  -> Inspector / SceneView / ProjectBrowser
```

源码参考：

```text
UnityCsReference/Modules/UIElements/Core/EventDispatcher.cs
UnityCsReference/Modules/UIElements/Core/ClickDetector.cs
UnityCsReference/Modules/UIElements/Core/IMGUIContainer.cs
UnityCsReference/Editor/Mono/SceneView/SceneView.cs
UnityCsReference/Editor/Mono/UIElements/Inspector
```

Bevy：

```text
winit event
  -> WindowEvent / ButtonInput / MouseButtonInput
  -> UI interaction / picking
```

Godot：

```text
EditorNode
  -> Control / gui_input
  -> SceneTreeDock / Inspector / FileSystemDock / Export
```

共同结论：

```text
成熟引擎都不是按钮直接执行业务。
真实路线都是 Window Event -> UI Tree / Hit / Focus -> Command / Action -> Editor State -> Redraw / Report。
```

## 3. 方案对比

| 方案 | 做法 | 优点 | 缺点 | 判断 |
|---|---|---|---|---|
| A | 继续补单个按钮点击 | 快 | 会继续出现某些按钮能点、某些按钮不能点的问题 | 不选 |
| B | 只做几条 headless click smoke | 能证明部分路径 | 缺少长期交互验收结构，后续仍会散 | 不够 |
| C-min | 建立 Native Editor Interaction Scenario Gate | 统一 event/hit/command/state/report，符合 Unity/UE 路线 | 第一版要补 scenario/report 结构 | 推荐 |

## 4. 推荐方案：C-min

系统名：

```text
Native Editor Real Interaction Validation Gate v1
```

核心数据流：

```text
NativeEditorInteractionScenario
  -> InteractionStep
  -> NativeEditorApplication
  -> EditorInputEvent
  -> HitTarget / HitRegion
  -> UiCommand / EditorCommandRequest
  -> EditorCommandSystem / EditorSession
  -> EditorUiModel
  -> SelfUiRenderer DrawList
  -> NativeEditorInteractionReport
```

第一版必须覆盖：

```text
Project Launcher: click create/open/recent can enter workspace.
Hierarchy: click entity changes selection and refreshes Inspector.
Inspector: click field, edit value, commit through transaction.
Asset Browser: click asset row changes selection / Inspector asset mode.
Build/Run: click build/run command writes command/report state.
AI Panel: click accept/reject command routes through command system.
```

## 5. 架构规则

```text
1. 真实交互验收必须从 Native Window Event 或等价可回放 EditorInputEvent 开始。
2. 不允许直接调用 EditorSession 方法冒充按钮点击。
3. UI hit test 必须产出稳定 HitTarget / HitRegion。
4. HitTarget 只能转换成 UiCommand / EditorCommandRequest，不能直接执行业务。
5. EditorCommandSystem / EditorSession 是业务执行入口。
6. 每个 scenario 必须输出 AI-readable report。
7. 每个 step 至少记录 input、hit、command、state、diagnostic。
8. 每个 scenario 至少验证 command、state、visual summary 三类结果。
9. 真实窗口不可用时报告 environment_blocked，不伪装 passed。
10. headless deterministic replay 必须保留，用于默认自动化测试。
```

## 6. 第一版实现边界

第一版做：

```text
NativeEditorInteractionScenario 数据结构。
NativeEditorInteractionStep 数据结构。
NativeEditorInteractionReport 数据结构。
Headless deterministic runner。
从 hit_region_id 生成 PointerDown 事件。
执行后记录 command_id/status、mode、selection、model_revision、draw/hit counts。
覆盖 Project Launcher / Hierarchy / Inspector / Build / AI 的最小 scenario 测试。
```

第一版不做：

```text
真实 OS 自动点击注入。
完整 IME 真实系统输入。
拖拽、框选、多窗口焦点。
截图 pixel diff。
复杂菜单、弹窗、停靠布局编辑。
```

这些后续必须继续接入同一 scenario gate，不能另起零散测试体系。

## 7. 为什么适合我们

AI 友好：

```text
失败报告能定位到 event / hit / command / state / draw 哪一层。
```

复杂项目维护：

```text
无论面板越来越多，新增交互都必须进入同一 scenario gate。
```

长期路线：

```text
方向接近 UE Slate Application + CommandList，也接近 Unity UI Toolkit EventDispatcher + EditorWindow。
```

简单度：

```text
第一版不做真实 OS 自动点击，只做等价可回放 EditorInputEvent。
这不会污染正常编辑器运行路径。
```

效率：

```text
只在测试/验证模式运行，不影响普通编辑器交互性能。
```

## 8. 方案自审

```text
Specification fit:
  满足用户确认的 C-min：做编辑器真实交互验收，不再补零散按钮。

Rule fit:
  遵守现有 121/122/123/127/150 规则，不绕过 EditorCommandSystem / EditorSession。

Textual consistency:
  数据流从 input 到 report 闭合，headless replay 和真实窗口 gate 边界清晰。

Design fit:
  符合 AI-first、复杂项目、长期维护、简单度、效率优先级。

Implementation feasibility:
  现有 NativeEditorApplication 已具备 frame、handle_input_event、dispatch_command、latest_draw_list、report。

Practical reasonableness:
  第一版覆盖核心创作链路，不扩展到拖拽、多窗口和 pixel diff，边界合理。
```

结论：

```text
本方案通过自审，可以生成施工文档并开始施工。
```
