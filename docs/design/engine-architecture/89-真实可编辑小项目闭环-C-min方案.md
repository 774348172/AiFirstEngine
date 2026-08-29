# 89-真实可编辑小项目闭环 C-min 方案

## 定位

本方案解决的问题不是再新增一个底层模块，而是把已经完成的底层 C-min 能力串成一个用户可感知、AI 可验证的最小编辑器工作流。

当前已经有：

```text
SceneEditing v1 C-min
UiCommand -> EditorSession -> SceneEditTransaction
EditorUiModel / SelfUiRenderer / HitRegion
Native Editor Window C-min
PlaySessionController
Runtime Package / Runtime Asset Loader
Runtime Scene / Prefab 实例化
Rust ECS / FrameLoop / ProjectLogicRunner
RenderCommand / RenderSceneState / RuntimeRenderer / RenderThread
Console / RuntimeTrace
```

但这些能力目前更多是模块级可测。下一阶段需要验证它们能不能形成一个真实的小项目编辑闭环：

```text
打开编辑器
  -> 打开或创建一个小 Scene
  -> Hierarchy 看到 Entity
  -> Inspector 选择并修改 Transform
  -> 保存 Scene
  -> Play 当前 Scene
  -> Runtime / Viewport / Console / Trace 给出反馈
```

本方案采用 C-min：长期结构按 Unity / UE 级别的编辑器工作流设计，但第一版只实现最小闭环，不做完整生产级编辑器体验。

## 已有方案边界

本方案不重新定义以下系统：

```text
25-Asset-DB-Importer-MVP.md
70-Scene-Prefab-Entity-Runtime实例化方案.md
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
84-Editor-Play-Run-Session-System方案.md
85-Scene-Editing-v1-C-min方案.md
86-真实UI命令接入SceneEditing-C-min方案.md
88-真实NativeEditorWindow-EventLoop-UIDraw-C-min方案.md
```

已有规则继续有效：

```text
EditorSceneDocument 是编辑真相。
Runtime World 不是编辑真相。
Scene 修改必须走 SceneEditCommand -> SceneEditTransaction。
真实 UI 行为必须先转成 UiCommandPayload。
EditorSession 是 UI 进入 Editor Core 的统一入口。
Play / Run 由 PlaySessionController 编排。
Window / UI backend 不直接修改业务状态。
```

89 的新增职责是把这些系统组织成一个端到端用户工作流，而不是替代任何一个已完成模块。

## 2026-06-28 施工完成记录

```text
状态：已完成 C-min 施工。
已完成施工文档：施工文档/已完成/89-当前可自动化施工文档-真实可编辑小项目闭环-C-min.md
阶段完成记录：阶段完成记录/2026-06-28-真实可编辑小项目闭环-C-min/00-总览.md

代码落点：
rust/crates/editor_core/src/editable_project_loop.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_host/src/main.rs

已验证：
cargo fmt
cargo fmt --check
cargo test -p editor_core editable_project_loop
cargo test -p editor_core scene_edit
cargo test -p editor_core play_session
cargo test -p editor_host editable_project_loop
cargo test -p editor_core
cargo test -p editor_host
cargo test --workspace
```

## 其它引擎对比

### Unity

Unity 的最小编辑闭环是：

```text
Project Window
  -> Hierarchy
  -> Inspector
  -> SceneView / GameView
  -> Play
  -> Console
```

核心特点：

```text
资源在 Project 中浏览。
Scene 内对象在 Hierarchy 中管理。
选中对象后 Inspector 显示并编辑 Transform / Component。
SceneView 负责编辑视图。
GameView 负责运行视图。
Play 从当前编辑状态进入运行状态。
Console 提供错误反馈。
```

值得学习：

```text
用户心智简单。
Hierarchy / Inspector / SceneView 协作清晰。
编辑和运行是同一工作流的一部分。
```

不直接照搬：

```text
不复制 Unity 的 IMGUI / SerializedObject 历史体系。
不让 UI 层直接持有编辑对象真相。
不在第一版实现完整 SceneView / GameView 体验。
```

### Unreal Engine

UE 的最小编辑闭环是：

```text
Content Browser
  -> World Outliner
  -> Details Panel
  -> Level Viewport
  -> PIE / Standalone
  -> Output Log / Message Log
```

核心特点：

```text
World Outliner 管理关卡对象。
Details Panel 修改 Actor / Component。
所有编辑行为进入事务系统。
PIE 从当前关卡状态启动运行会话。
日志和消息系统用于定位问题。
```

值得学习：

```text
大型项目下编辑行为必须可追踪、可撤销、可保存。
运行会话和编辑器状态边界清楚。
工具很多，但底层入口保持稳定。
```

不直接照搬：

```text
不第一版实现完整 EditorMode / PlacementMode / Details customization。
不引入 UE 级复杂事务和对象反射系统。
不做完整 PIE World 复制。
```

### Godot

Godot 的最小闭环是：

```text
FileSystem
  -> Scene Tree
  -> Inspector
  -> Viewport
  -> Run Current Scene
  -> Output
```

值得学习：

```text
整体链路短。
小项目体验很直接。
Run Current Scene 对最小闭环非常友好。
```

不直接照搬：

```text
不采用 Node 作为我们项目的上层统一概念。
不让 Inspector 绕过 SceneEditTransaction 修改源数据。
```

### Bevy

Bevy 当前更偏 Runtime / ECS，不是完整 Unity/UE 式编辑器。

值得学习：

```text
ECS / Schedule / Asset / RenderApp 的数据分层清楚。
适合我们验证 Runtime 和渲染底层。
```

不适合作为 89 主参考：

```text
Bevy 不提供成熟的内置编辑器工作流。
89 重点是编辑器用户闭环，因此主要参考 Unity / UE / Godot。
```

## 方案对比

### 方案 A：继续补底层模块

继续做完整 ECS Scheduler、完整 RHI、完整 Asset Streaming、完整 IR AOT。

优点：

```text
底层能力更强。
长期技术债较少。
```

缺点：

```text
用户仍然不能完成真实编辑工作流。
AI 只能验证模块，不能验证用户意图是否被完整实现。
容易继续陷入无穷底层细节。
```

结论：

```text
不作为下一阶段主线。
```

### 方案 B：只做真实窗口交互

继续强化 Native Window、UI renderer、鼠标键盘事件、真实控件。

优点：

```text
能更像一个真实编辑器。
视觉和交互体验提升快。
```

缺点：

```text
如果不接 Scene / Play / Save 闭环，窗口只是壳。
容易把精力耗在控件和渲染细节上。
```

结论：

```text
不作为下一阶段主线。
```

### 方案 C：真实可编辑小项目闭环 C-min

把现有底层能力串成一个最小编辑工作流。

```text
EditorSession
  -> EditorUiModel
  -> Hierarchy / Inspector / Viewport / Console / RuntimeTrace
  -> UiCommand
  -> SceneEditTransaction
  -> SceneSavePipeline
  -> PlaySessionController
  -> Runtime / Render / Report
```

优点：

```text
最能验证整体架构是否真的可用。
最贴近 Unity / UE / Godot 的核心用户工作流。
最适合 AI：自然语言需求可以落到一个完整可验证流程。
能暴露跨系统边界问题。
不会引入大量新底层规则。
```

缺点：

```text
会同时触碰 EditorCore / UI Model / SceneEditing / PlaySession / Runtime Report。
需要严格控制第一版范围，避免膨胀成完整编辑器。
```

结论：

```text
推荐采用。
```

## 推荐方案

采用方案 C：真实可编辑小项目闭环 C-min。

第一版目标不是做完整编辑器，而是完成一个可自动化验证的小项目编辑流程：

```text
打开一个默认小项目 / Scene。
Hierarchy 能显示 Entity。
选择 Entity 后 Inspector 能显示 Transform。
Inspector 修改 Transform 必须生成 UiCommandPayload。
UiCommandPayload 进入 EditorSession。
EditorSession 转成 SceneEditCommand。
SceneEditTransaction 修改 EditorSceneDocument。
PreviewWorldSync 刷新预览数据。
SaveScene 走 SceneSavePipeline。
Play 当前 Scene 走 PlaySessionController。
Console / RuntimeTrace 输出结构化反馈。
Headless 测试覆盖完整链路。
```

## 第一版范围

必须实现：

```text
默认小项目 / 默认 Scene fixture。
EditorSession 打开默认 Scene。
Hierarchy 显示至少一个根 Entity。
Hierarchy 选择 Entity。
Inspector 显示被选中 Entity 的 Transform。
Inspector 修改 localPosition。
Scene dirty 状态变化。
Scene save 成功。
Undo / Redo 至少覆盖 Transform 修改。
Play 当前 Scene。
PlaySessionReport / Console / RuntimeTrace 最小反馈。
Headless end-to-end report。
```

建议第一版测试用例：

```text
editable_project_loop_opens_default_scene
editable_project_loop_selects_entity_and_updates_inspector
editable_project_loop_edits_transform_and_marks_dirty
editable_project_loop_save_clears_dirty
editable_project_loop_undo_redo_transform
editable_project_loop_play_current_scene_reports_result
editable_project_loop_report_is_ai_readable
```

明确不做：

```text
完整 ProjectDock 文件浏览。
真实资源拖拽到 Scene。
真实 Viewport picking。
真实 Gizmo。
完整 Inspector 控件体系。
Prefab Mode。
多选批量编辑。
复杂 Dock layout。
复杂菜单 / 快捷键。
真实字体和完整 UI renderer。
```

## 数据流规则

### 编辑流

```text
User / AI / Test
  -> UiCommandPayload
  -> EditorSession
  -> SceneEditCommand
  -> SceneEditTransaction
  -> EditorSceneDocument
  -> PreviewWorldSync
  -> EditorUiModel
```

规则：

```text
UI 不直接写 EditorSceneDocument。
AI 不直接写 Scene 文件。
Runtime 不反向修改 EditorSceneDocument。
Inspector 不直接写 Transform 字段。
Hierarchy 不直接修改 Entity 树。
所有修改必须可进入 transaction / dirty / undo / save。
```

### 运行流

```text
Toolbar Play / Test Play
  -> UiCommandPayload::Play
  -> EditorSession
  -> PlaySessionController
  -> DefaultGameRunOrchestrator
  -> Runtime Package / Scene / ECS / FrameLoop
  -> Report
  -> Console / RuntimeTrace / EditorUiModel
```

规则：

```text
PlaySession 属于 Editor 侧。
Runtime 不知道编辑器按钮。
第一版 Play 可以走 headless gate。
Windowed Play 可以继续保留 C-min diagnostic 或 smoke gate。
```

## Report 规则

新增建议：

```text
EditableProjectLoopReport
```

最小字段：

```text
schema_version
project_id
scene_id
opened_scene
selected_entity_id
inspector_entity_id
dirty_before_save
dirty_after_save
transform_edit_applied
undo_applied
redo_applied
play_started
play_finished
console_entry_count
runtime_trace_entry_count
diagnostics
```

规则：

```text
Report 是 AI 和测试的主要读取入口。
Report 不保存完整 Scene dump。
Report 只记录关键状态、计数、id、diagnostic。
如需定位细节，通过 transaction report / scene edit report / play session report 关联。
```

## AI 友好规则

```text
AI 只能生成 UiCommandPayload 或更高层的 Edit Plan，不能直接写 Scene 文件。
AI 修改必须能生成或更新测试。
AI 判断成功不靠 UI 像素，而靠 EditableProjectLoopReport。
每个编辑动作必须能追踪到 request_id / command_id / transaction_id。
失败必须进入 Console / Diagnostic / Report，不允许只 panic 或 stderr。
```

## 复杂项目适配规则

第一版只做一个小 Scene，但边界必须能扩展到复杂项目：

```text
SceneEditTransaction 可以扩展到多 Entity / 多 Component。
Inspector 后续可以由 Schema / Component metadata 生成。
ProjectDock 后续可以接 AssetRegistry 派生 ViewModel。
PlaySession 后续可以切换 headless / windowed / external process。
EditableProjectLoopReport 后续可以关联 BuildRunReport / RuntimeRunReport / RenderFrameReport。
```

避免第一版埋坑：

```text
不要把默认小项目写死成唯一项目模型。
不要让 Inspector 保存自己的 Transform 副本。
不要让 Play 直接读 UI 临时状态。
不要让测试绕过 EditorSession 主路径。
```

## 验收标准

满足以下条件才算 89 C-min 完成：

```text
可以从 EditorSession 打开默认可编辑 Scene。
可以通过 UiCommand 选择 Entity。
可以通过 UiCommand 修改 Transform。
修改会进入 SceneEditTransaction。
修改会刷新 EditorUiModel / Inspector / Viewport renderables。
保存会走 SceneSavePipeline。
Undo / Redo 能恢复 Transform。
Play 当前 Scene 能产生 PlaySessionReport。
Console / RuntimeTrace 能看到最小反馈。
EditableProjectLoopReport 可序列化。
默认 cargo test --workspace 不依赖真实窗口。
```

## 下一步施工建议

确认本方案后，生成施工文档：

```text
施工文档/当前/89-当前可自动化施工文档-真实可编辑小项目闭环-C-min.md
```

建议施工顺序：

```text
1. 新增 EditableProjectLoopReport。
2. 新增默认小项目 / 默认 Scene fixture helper。
3. 打通 open scene -> hierarchy -> inspector。
4. 打通 inspector transform edit -> transaction -> dirty -> preview sync。
5. 打通 save -> dirty clear。
6. 打通 undo / redo。
7. 打通 play current scene -> PlaySessionReport。
8. 补齐 headless end-to-end tests。
9. 同步阶段完成记录和入口文档。
```
