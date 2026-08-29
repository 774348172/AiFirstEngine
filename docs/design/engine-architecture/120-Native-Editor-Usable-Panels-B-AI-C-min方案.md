# 120-Native Editor Usable Panels B+AI C-min 方案

## 1. 定位

本文确认 Native Editor 第一版可用面板路线：

```text
Native Editor Usable Panels B+AI C-min
```

它不是重新设计 Native Editor 架构，也不是完整 Unity / Unreal 级编辑器。它是在既有规则之上，把 Native Editor 从“能打开 / 能画粗框架”推进到“能操作、能编辑、能诊断”的第一版。

继承规则：

```text
37-Editor-Core与可迁移UI路线.md
47-Native-Editor-Host-BC路线.md
105-Editor-Authoring-System-C-min方案.md
111-Native-Editor-Real-UI-Present-方案B.md
113-Native-Editor-FontSystem-v1方案.md
114-Native-Editor-UI-RenderGraph-RHI-收敛方案.md
119-复杂打飞机编辑到桌面打包缺失功能清单.md
```

核心目标：

```text
Toolbar / Hierarchy / Inspector / Viewport Shell / Console / RuntimeTrace / AI Panel
都必须通过 EditorUiModel / UiDrawList / HitRegion / UiCommand / CommandTransaction 工作。
```

## 2. 成熟引擎对比

### Unity

Unity 的最小编辑闭环依赖：

```text
Hierarchy
Inspector
SceneView
Console
Toolbar / Play Controls
Undo / Dirty / Save
```

GameObject / Component 修改不会由 UI 控件随意写入，而是经由 Editor 序列化、Undo、Dirty、Save 体系。

### Unreal Engine

UE 的对应结构是：

```text
World Outliner
Details Panel
Level Viewport
Output Log
Toolbar
Transaction
```

Details / Outliner 是核心编辑面板，所有修改需要进入 Transaction，避免 UI 状态变成业务真相。

### Godot

Godot 的短链路是：

```text
SceneTree
Inspector
Viewport
FileSystem
Output
UndoRedo
```

Godot 的优点是短、直接、易懂。本项目第一版可用面板应学习这种短链路。

### 本项目取舍

本项目采用：

```text
Unity / Godot 的简单编辑心智
+ UE 的 CommandTransaction 边界
+ AI-first 的结构化 UiModel / UiCommand / Diagnostics
```

不采用：

```text
完整 Slate Widget 体系
完整 UI Toolkit
复杂 Dock / Tab / 多窗口
UI 直接持有项目真相
AI 直接写文件或绕过 EditorSession
```

## 3. 面板范围

第一版包含：

```text
Toolbar
Hierarchy 深度可用
Inspector 深度可用
Viewport Shell
Console
RuntimeTrace
AI Panel
```

第一版不包含：

```text
完整 Dock / Tab 拖拽
复杂菜单 / 快捷键系统
完整 Asset Browser
完整 Build Panel
Prefab Mode
复杂 Scene Gizmo
复杂 Viewport picking
复杂 TextInput / IME
复杂 Inspector 自定义控件
```

## 4. 标准链路

所有面板操作必须走同一条链：

```text
EditorUiModel
  -> SelfUiRenderer
  -> UiDrawList
  -> HitRegion
  -> EditorInputRouter
  -> UiCommand
  -> EditorSession
  -> CommandTransaction
  -> StateChange / Diagnostics / Console
  -> EditorUiModel rebuild
```

AI Panel 也不例外：

```text
AI Panel
  -> AiEditorRequest
  -> AiPanelResponse
  -> proposed UiCommand list
  -> user confirm / auto-allowed gate
  -> EditorSession execute
```

## 5. Hierarchy 深度可用规则

第一版必须支持：

```text
显示 Scene Entity Tree
显示选中状态
点击选择 Entity
创建空 Entity
删除 Entity
重命名 Entity
显示父子层级
Undo / Redo
Console / Diagnostics 反馈
```

第一版不做：

```text
拖拽改父子层级
多选
搜索过滤
右键复杂菜单
Prefab override
```

命令入口：

```text
SelectSceneEntity
CreateSceneEntity
DeleteSceneEntity
RenameSceneEntity
UndoSceneEdit
RedoSceneEdit
```

## 6. Inspector 深度可用规则

第一版必须支持：

```text
显示 selected entity
显示 Transform
编辑 localPosition / localRotation / localScale
显示基础组件字段
编辑 bool / number / string / Vec3 / AssetRef / Json 基础字段
字段修改走 SetSceneComponentField
Transform 修改走 SetSceneTransform
Undo / Redo
失败进入 Console / Diagnostics
```

第一版不做：

```text
复杂数组编辑器
复杂对象嵌套编辑器
自定义组件编辑器插件
高级资源选择器
动画曲线控件
颜色选择器
```

## 7. AI Panel 规则

AI Panel 第一版只做三类事：

```text
解释当前状态
生成编辑计划
执行受控命令
```

AI Panel 输入：

```text
user_text
selected_entity_id
current_scene_summary
inspector_summary
console_summary
runtime_trace_summary
allowed_command_schema
```

AI Panel 输出：

```text
AiPanelResponse
  explanation
  proposed_commands[]
  risk_summary
  requires_confirmation
  diagnostics[]
```

允许生成的命令：

```text
SelectSceneEntity
CreateSceneEntity
DeleteSceneEntity
RenameSceneEntity
SetSceneTransform
SetSceneComponentField
SaveSceneDocument
UndoSceneEdit
RedoSceneEdit
PlaceAssetIntoScene
```

禁止：

```text
直接写文件
直接改 Runtime World
直接改 ECS storage
直接绕过 EditorSession
直接生成项目专用引擎 API
直接执行 build/package
直接修改 renderer/backend 代码
```

AI Panel 第一版可以用本地 deterministic mock planner，不接真实模型。重点是先打通结构：

```text
自然语言
  -> 可解释计划
  -> 结构化命令
  -> EditorSession
  -> Diagnostics / Console
```

## 8. 其它面板规则

### Toolbar

第一版支持：

```text
Open Scene
Save Scene
Undo
Redo
Play
Pause
Step
Tick
Clear Console
```

### Viewport Shell

第一版只显示和轻交互：

```text
scene id
frame
selected entity
renderable count
viewport placeholder / texture slot
```

不做真实 gizmo 和复杂 picking。

### Console

Console 是所有面板反馈中心：

```text
Command committed
Command rejected
Command failed
AI plan warning
Scene dirty state
Save result
Runtime/package error
```

### RuntimeTrace

第一版支持：

```text
显示 trace entries
点击 trace entry
如果 trace 带 entity_id，后续可联动选择 entity
```

## 9. 验收标准

必须有 headless 测试证明：

```text
EditorUiModel 包含 ai_panel
SelfUiRenderer 输出 AI Panel / Inspector Field / Hierarchy Action hit regions
EditorInputRouter 能把命中转成 UiCommand
EditorSession 能执行 RenameSceneEntity
Inspector editable command 能修改 Transform
AI Panel mock request 能生成 proposed command
确认 AI command 后能通过 EditorSession 修改 scene
Console 记录 AI / command 反馈
```

real-window smoke 只作为本机验证，不作为唯一验收方式。

## 10. 核心结论

```text
Native Editor 第一版应该先变成可用工具。
Hierarchy / Inspector 是深度可用核心。
AI Panel 可以进入第一版，但只能作为结构化命令和解释入口。
所有修改必须走 EditorSession / CommandTransaction。
```

