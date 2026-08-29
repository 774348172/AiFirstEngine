# 185-M12 AUI HUD Authoring / Binding / Runtime Present v1 方案

## 1. 方案结论

本文定义 `130-复杂打飞机编辑到 Windows 可玩项目缺失能力当前基线` 中的：

```text
M12 | AUI HUD Authoring / Binding / Runtime Present
```

正式系统命名：

```text
AUI Runtime HUD Authoring / Binding / Present v1
```

推荐方案：

```text
方案 C-min：完整 AUI 产品链路的最小版
```

M12 不是重新创建一个新的 UI 系统，而是把已有 AUI 从“Runtime Core / Layout / DrawList / Interaction 的底座”推进到“复杂项目可制作、可绑定、可构建、可在导出 Player 显示”的产品化链路。

最终链路：

```text
AUI Document / HUD Asset
  -> Editor AUI Authoring
  -> Schema-driven structured edit
  -> AUI Binding
  -> Build RuntimePackage
  -> Runtime load
  -> Project UI State Snapshot
  -> AuiDocument resolve binding
  -> AuiLayout / AuiDrawList
  -> UiProjection / AuiOverlayFrame
  -> RuntimeRenderer UI Pass
  -> Windowed Player Present
  -> AUI Interaction Action
  -> Project Rule
```

## 2. Unity4.3 源码复查后的修正

参考文档：

```text
框架设计/Unity源码参考/Unity4.3.1f1-源码地图.md
框架设计/Unity源码参考/Unity4.3.1f1-参考源码文档.md
```

Unity4.3 关键源码入口：

```text
<UNITY_LEGACY_SOURCE>\Runtime\IMGUI
<UNITY_LEGACY_SOURCE>\Editor\Src\Application.cpp
<UNITY_LEGACY_SOURCE>\Editor\Src\Utility\SerializedProperty.cpp
<UNITY_LEGACY_SOURCE>\Editor\Src\Undo
<UNITY_LEGACY_SOURCE>\Runtime\GfxDevice
<UNITY_LEGACY_SOURCE>\Runtime\Camera
```

复查结论：

```text
M12 主方向不需要推翻。
Unity4.3 的 Runtime\IMGUI 不是现代 UGUI，也不是本项目 AUI 长期路线。
Unity4.3 对 M12 的价值是证明 UI 必须和 renderer / input / font / texture / editor edit chain 分层。
Unity4.3 的 SerializedProperty / Undo 链路提醒我们：AUI 编辑器不能让控件直接改 JSON。
```

因此 M12 需要补强两条规则：

```text
1. AUI Runtime UI 不走 Unity4.3 IMGUI 路线。
2. AUI Authoring 修改必须走结构化 command / transaction / schema path，不允许 UI 控件直接写文件。
```

## 3. 系统定位

AUI 是：

```text
AI-first Runtime UI System
```

AUI 不是：

```text
Editor UI
Sprite2D world object
Unity IMGUI clone
UGUI prefab clone
打飞机专用 HUD API
```

M12 做的是 AUI 的第一条真实产品链路：

```text
HUD Authoring
Binding
Runtime Present
```

第一版以复杂打飞机 HUD 验证，但引擎层不出现以下专用概念：

```text
Player
Enemy
Bullet
Health
Score
Wave
Weapon
Boss
Drop
```

这些概念只能由项目侧 Schema / Rule / Prefab / AUI Binding path 表达。

## 4. 当前已有基础

已完成基础：

```text
100-AUI-AI-First-Runtime-UI-System方案.md
102-AUI-Render-Extract-RuntimeRenderer接入方案.md
103-AUI-Interaction-System-C-min方案.md
```

代码层已有：

```text
engine_runtime::aui::AuiDocument
engine_runtime::aui::AuiCanvas
engine_runtime::aui::AuiNode
engine_runtime::aui::AuiNodeKind
engine_runtime::aui::AuiRect
engine_runtime::aui::AuiStyle
engine_runtime::aui::AuiBindingRef
engine_runtime::aui::AuiLayoutEngine
engine_runtime::aui::AuiDrawList
engine_runtime::aui::AuiRendererBridge
engine_runtime::aui::AuiOverlayFrame
engine_runtime::aui::AuiInteractionSystem
RuntimeRendererInput.aui_overlay
RenderPassKind::DrawUiOverlay
```

需要纠正的现状认知：

```text
AuiBindingRef 当前只存在 binding_id，不是完整 binding 描述。
AuiCommand 当前是 PointerDown / PointerUp / PointerMove / Hover / Click 这类指针级事件命令，不是业务级 action。
ProjectUiStateSnapshot / UiValue 当前不存在，是 M12 必须新增的核心类型。
AuiNodeKind 当前没有 ProgressBar，M12 Gate A 必须补齐。
AuiDrawCommand 当前只有 Rect / Image / Text，Button / ProgressBar 第一版必须用组合绘制，不新增专用 DrawCommand。
Runtime UI text present 当前存在字体链路风险，M12 必须明确真实 glyph 渲染策略，不能只统计 text_count。
```

当前缺口：

```text
缺 AUI 产品化 authoring。
缺 AUI 文档创建 / 编辑 / 保存。
缺 AUI binding 的正式 Project UI State 来源。
缺 AUI 文档进入 RuntimePackage 的完整 gate。
缺 Windowed Player 中 HUD 绑定变化可见的验收。
缺 AUI action -> Project Rule 的最小产品链。
```

## 5. 其他引擎对比

| 引擎 / 系统 | 对应模块 | 可借鉴点 | 不照搬点 |
|---|---|---|---|
| Unity UGUI | Canvas / RectTransform / Image / Text / Button / EventSystem | Canvas、Anchor、Image、Text、Button、Prefab/Inspector 工作流 | Canvas rebuild 黑箱、GameObject 级 UI runtime、复杂 LayoutGroup 隐式重算 |
| Unity4.3 IMGUI | Runtime/IMGUI、EditorGUI | 证明 text/style/clip/event/input 与渲染强相关；适合研究历史边界 | 不作为 Runtime UI 长期路线，不做 immediate-mode AUI |
| NGUI | UIRoot / UIPanel / UIWidget / UISprite / UILabel | Panel / Widget 简单清楚，适合 2D HUD 心智 | 老插件路线、强 Atlas 依赖、长期工具链不足 |
| UE UMG / Slate | Widget Tree / Binding / Slate Draw Elements | Widget Tree、Binding、DrawElement、UI Pass 分层 | 不照搬完整 Slate/UMG 重体系，不引入复杂 Blueprint binding |
| Godot Control | Control / CanvasLayer / Container / Theme / Signal | 简单节点树、Anchor、CanvasLayer、Signal 心智 | 不采用 Node/Signal 作为本项目 runtime 真相 |
| Bevy UI / Picking | UI Node / Layout / Extract / Render phase / Picking | 数据驱动、layout/render extract 分离、headless 可测 | 不暴露 Bevy schedule/render world 复杂度 |

我们的路线：

```text
Unity UGUI 的用户心智
+ NGUI 的 Panel / Widget 简洁
+ UE 的 DrawElement / UI Pass 分层
+ Godot Control 的简单节点树
+ Bevy 的数据驱动和 headless test
+ 本项目 AI-first 结构化文档真相层
```

## 6. 方案对比

### 6.1 方案 A：静态 HUD Overlay

只支持固定 Text / Image / Rect，不做 Binding，不做 Button action。

优点：

```text
实现最快。
可以显示简单 HUD。
```

缺点：

```text
不能表达分数、血量、暂停状态变化。
复杂项目不可维护。
后续仍要补 binding / authoring / runtime present。
```

结论：

```text
不选。
```

### 6.2 方案 B：Runtime Binding 优先，Editor Authoring 弱化

先做 AUI binding 和 Player 显示，编辑器只做 JSON / 列表 / 简单字段。

优点：

```text
更快进入 Player 验证。
Runtime 链路清晰。
```

缺点：

```text
编辑器体验弱。
用户仍不能像 Unity 一样制作 HUD。
后续会补一轮 AUI Authoring，容易形成两段路线。
```

结论：

```text
可作为保底，不作为正式推荐。
```

### 6.3 方案 C-min：完整产品链路最小版

从第一版就保留完整边界：

```text
AUI Asset
Editor Authoring
Structured Edit
Binding
RuntimePackage
Runtime Present
Interaction Action
Project Rule
Report / Test
```

但节点和能力只做最小集。

优点：

```text
最符合长期主义。
不会把 AUI 变成临时 HUD 叠层。
AI 可通过 AuiDocument / Binding / Report 理解 UI。
复杂项目后续可扩展到菜单、结算、背包、弹窗。
```

缺点：

```text
第一版施工量高于 A / B。
需要同时补 Editor、Build、Runtime、Report 的最小链路。
```

结论：

```text
推荐。
```

## 7. 推荐方案

采用：

```text
方案 C-min：完整 AUI 产品链路的最小版
```

选择理由：

| 指标 | 判断 |
|---|---|
| AI 友好 | AUI 文档、binding、action、report 都是结构化数据，AI 容易生成、修改、审查。 |
| 复杂项目维护 | Widget tree + binding + action 可以扩展到菜单、背包、结算、对话框。 |
| 效率 | Runtime 只消费已解析 AuiDocument 和 Project UI State Snapshot，生成 DrawList 后进入 UI Pass。 |
| 简单度 | 第一版只做 6 类节点和少量 binding，不做完整 UI 设计器。 |
| 长期主义 | 对齐 UGUI/UMG/Godot 的成熟心智，但不复制它们的历史复杂度。 |

## 8. 第一版能力边界

第一版节点：

```text
Canvas
Panel
Text
Image
Button
ProgressBar
```

已有 `Panel / Image / Text / Button` 可复用或补强。
`ProgressBar` 当前不是已有 `AuiNodeKind`，M12 v1 必须新增。
第一版不新增 Button / ProgressBar 专用 draw command：

```text
Button = background Rect + Text + hit region / action ref
ProgressBar = background Rect + fill Rect
```

这条规则的目的不是弱化 Button / ProgressBar，而是保持 renderer draw primitive 简单稳定。控件语义留在 AuiNode / Layout / Interaction 层，渲染层只消费 Rect / Image / Text 这类基础绘制。

第一版布局：

```text
ScreenOverlay Canvas
Anchor min / max
Offset min / max
Size
Z order / tree order
Visible
```

第一版不做：

```text
WorldSpace UI
CameraSpace UI
ScrollView
InputField
Toggle
Slider
RichText
IME
DragDrop
Focus / Capture
Animation timeline
复杂 LayoutGroup
Mask / Clip
```

## 9. AUI Document 规则

`AuiDocument` 是 AUI 真相层。

建议结构：

```text
AuiDocument
  document_id
  version
  canvases
  nodes
  bindings
  actions
  metadata
```

`AuiNode` 第一版字段：

```text
AuiNode
  node_id
  kind
  parent_id
  rect
  style
  text
  image_asset_ref
  progress_value
  visible
  interactable
  consume_input
  binding_refs
  action_refs
```

规则：

```text
AuiDocument 不保存运行时值。
AuiDocument 不直接引用 ECS entity。
AuiDocument 不保存项目专用语义。
AuiDocument 可以引用 AssetRef、binding path、action id。
```

## 10. Binding 规则

M12 第一版引入两个新核心类型：

```text
ProjectUiStateSnapshot
UiValue
```

定位：

```text
ProjectUiStateSnapshot 是 Project Rule 输出给 AUI 的只读 UI 状态快照。
它不是 ECS 世界本体。
它不是 AUI 文档真相。
它是 runtime 每帧或状态变化时供 AUI binding 读取的数据视图。
```

建议结构：

```text
ProjectUiStateSnapshot
  frame_index
  values: map<string, UiValue>

UiValue
  Bool
  Number
  String
  Color
  AssetRef
```

`ProjectUiStateSnapshot` 是 M12 新建类型，不是已有基础。它的存在价值是把项目逻辑输出给 UI 的运行时状态收敛成只读数据视图，避免 AUI 直接读取 ECS、Project Rule 或项目专用对象。

`AuiBindingRef` 必须从“只有 binding_id 的引用”扩展成完整 binding 描述：

```text
AuiBindingRef
  binding_id
  target_field: AuiBindingTarget
  path
  fallback: Option<AuiBindingValue>

AuiBindingTarget
  Text.text
  ProgressBar.value
  Panel.visible
  Image.visible
  Image.asset_ref

AuiBindingValue
  Bool
  Number
  String
  Color
  AssetRef
```

Binding 示例：

```text
Text.text <- bind("game.score_text")
ProgressBar.value <- bind("player.hp_ratio")
Panel.visible <- bind("game.paused")
Image.visible <- bind("warning.low_hp")
```

规则：

```text
Binding path 由项目侧定义含义。
引擎只验证 path 是否存在、类型是否匹配、fallback 是否明确。
AUI Binding 只读 ProjectUiStateSnapshot。
AUI Binding 不直接写 ECS。
AUI Binding 不调用 Project Rule。
AuiBindingRef 必须显式声明 target_field / path / fallback。
缺 fallback 或类型不匹配必须进入 AuiBindingReport / BuildReport / RuntimeFrameReport。
```

## 11. Action 规则

M12 必须把指针级命令和业务级 action 分开。

已有 `AuiCommand` 定位为指针级事件命令：

```text
AuiCommand
  PointerDown
  PointerUp
  PointerMove
  Hover
  Click
```

M12 新增 `AuiAction`，定位为业务级 UI 意图：

```text
AuiAction
  action_id
  node_id
  event
  payload
```

转换关系：

```text
PointerDown / PointerUp / PointerMove / Hover / Click -> AuiCommand
Click + node.action_ref -> AuiAction
AuiAction -> Project Rule input
```

示例：

```text
button.pause.on_click -> action("ui.pause")
button.resume.on_click -> action("ui.resume")
button.restart.on_click -> action("ui.restart")
```

规则：

```text
AUI 不知道 pause / restart 的业务含义。
AUI action 进入 Project Rule。
Project Rule 决定 action 对项目状态的影响。
AUI action 必须进入 trace / report。
AuiCommand 不允许承载项目业务语义。
AuiAction 不允许直接修改 ECS / Runtime World。
```

## 12. Editor Authoring 规则

M12 的 Editor Authoring 不是独立完整 UI Designer，但必须让用户能自然制作 HUD。

209 之后的口径修正：

```text
“自然制作 HUD” 不再指独立 AUI Designer。
后续可视化 authoring 以 209 AUI Scene Unified Authoring 为准。
Scene View / Hierarchy / Inspector 是统一编辑入口。
2D 只是 Scene View 的视图模式，不是 AUI 专用编辑模式。
AUI Node 不变成 Runtime ECS Entity，只通过 editor authoring proxy 被选择和检查。
```

第一版编辑器能力：

```text
在 Project / AUI domain 中创建 hud.aui.json。
打开 AUI 文档。
显示 AUI node tree。
选择 AUI node。
Inspector 编辑节点基础字段。
添加 Text / Image / Button / ProgressBar。
编辑 binding path。
编辑 button action id。
保存 AUI 文档。
生成 AUI authoring report。
```

Unity4.3 源码带来的强规则：

```text
编辑器控件不能直接写 JSON。
所有 AUI 修改必须通过 UiCommand / EditorSession / Transaction。
AUI 字段修改必须是 schema path 级结构化修改。
保存前必须 validate。
```

推荐链路：

```text
Editor UI
  -> UiCommandPayload::Aui(...)
  -> EditorSession
  -> AuiAuthoringService
  -> AuiTransaction
  -> AuiDocument
  -> Validate
  -> Save
  -> EditorUiModel refresh
  -> Report
```

当前 `UiCommandPayload::Aui(...)`、`AuiAuthoringService`、`AuiTransaction` 尚不存在，M12 施工必须把它们标记为新增能力。

第一版执行策略：

```text
先完成 headless AUI Authoring Service：
  create / open / edit / save / validate
  schema path transaction
  authoring report

再接 Native Editor UI：
  AUI domain list
  node tree
  inspector fields
  add node commands
```

这样做不是弱化编辑器，而是避免 Native Editor UI 尚未完全成熟时阻塞 Runtime Binding / Present 主链路。

## 13. Build / RuntimePackage 规则

Build 阶段：

```text
Project AUI documents
  -> validate
  -> collect AssetRef dependencies
  -> RuntimePackage AUI manifest
  -> cooked package
```

RuntimePackage 最小结构：

```text
RuntimePackage
  aui_manifest
    documents
      document_id
      path
      canvas_count
      node_count
      binding_count
      action_count
      asset_refs
```

规则：

```text
Runtime 不扫描项目源目录。
Runtime 只读取 RuntimePackage 中的 AUI manifest / document。
缺少 AUI 文档、缺少 asset、binding 类型不匹配必须进入 BuildReport / RuntimeReport。
```

## 14. Runtime Present 规则

运行时链路：

```text
RuntimePackage
  -> AuiDocument load
  -> ProjectUiStateSnapshot
  -> Binding resolve
  -> AuiLayoutEngine
  -> AuiDrawList
  -> UiProjectionAdapter
  -> AuiOverlayFrame
  -> RuntimeRenderer UI Pass
  -> Present
```

已有规则保持：

```text
AUI 不直接生成底层 RHI 命令。
AUI 不进入 RenderProxy。
AUI 不参与 Sprite2D sorting。
RuntimeRenderer 只接收 AuiOverlayFrame，不直接读取 AuiDocument。
ScreenOverlay AUI pass 插在 World / Sprite draw passes 之后、Present 之前。
```

M12 补充规则：

```text
AUI Binding resolve 发生在 Layout / DrawList 之前。
AuiOverlayFrame 只携带已解析后的 draw item。
Renderer 不读取 binding path。
Renderer 不读取 ProjectUiStateSnapshot。
```

字体 / 文本渲染规则：

```text
AUI 节点可以引用 font asset / font style，但 AUI 不直接持有 font atlas 内部细节。
AuiDrawCommand::DrawText 必须能在 RuntimeRenderer UI Pass 中生成真实 glyph 渲染结果。
如果当前 runtime path 没有可复用 FontSystem / glyph atlas，M12 必须补最小 runtime font binding。
HUD 文本验收必须看到真实字形，不能只用 text_count、占位矩形或日志统计代替。
font atlas page / glyph cache 仍属于 renderer/font backend 内部细节，AI 默认不读。
```

## 15. Interaction 规则

已有基础：

```text
RuntimeInputFrame
  -> AuiInteractionSystem
  -> AuiHitTestResult
  -> AuiInteractionResult
      consumed
      commands
      traces
```

M12 补充：

```text
AuiCommand::Click
  -> AuiAction
  -> Project Rule input
  -> Project state update
  -> next ProjectUiStateSnapshot
  -> next AUI binding resolve
```

输入消费规则：

```text
如果 AUI consumed=true，则该 pointer event 不进入 Gameplay InputMapping。
如果 AUI consumed=false，则继续进入 Gameplay InputMapping。
Button click 不直接修改 runtime world。
```

## 16. Report / Trace 规则

M12 必须提供：

```text
AuiAuthoringReport
AuiBindingReport
AuiRuntimePresentReport
AuiInteractionReport
```

最小字段：

```text
document_id
stage
status
node_count
binding_count
action_count
missing_assets
missing_bindings
type_mismatches
draw_item_count
consumed_input_count
emitted_action_count
diagnostics
```

AI 默认读取：

```text
AuiDocument summary
AuiBindingReport
AuiRuntimePresentReport
AuiInteractionReport
BuildReport AUI section
RuntimeFrameReport AUI section
```

AI 不默认读取：

```text
GPU buffer
font atlas page
shader variant
low-level RHI command
```

## 17. 最小验收场景

M12 完成后，必须支持一个通用 HUD 场景：

```text
hud.aui.json
  Canvas
    Panel top_left
      Text score_text bind("game.score_text")
    ProgressBar hp_bar bind("player.hp_ratio")
    Button pause_button action("ui.pause")
    Panel pause_overlay visible bind("game.paused")
```

验收链路：

```text
编辑器能创建 hud.aui.json。
编辑器能添加 Text / Image / Button / ProgressBar。
Inspector 能编辑 rect / text / image / binding / action。
保存后重新打开仍一致。
Build 后 RuntimePackage 包含 AUI document 和 manifest。
Player 启动后显示 HUD。
HUD Text 必须显示真实 glyph。
ProjectUiStateSnapshot 变化后 Text / ProgressBar / visible 更新。
点击 Button 后发出 AUI action。
AUI consumed pointer 后 Gameplay input 不触发。
导出 Windows exe 后 HUD 仍显示。
```

## 18. 与复杂打飞机的关系

复杂打飞机可使用 M12 表达：

```text
分数文本
血量条
暂停按钮
暂停面板
结算面板
警告提示
技能冷却文本
```

但这些都是项目侧组合：

```text
Project Rule 输出 game.score_text / player.hp_ratio / game.paused。
AUI 读取 binding path。
Button 输出 ui.pause / ui.resume / ui.restart。
Project Rule 处理 action。
```

引擎不内置：

```text
ScoreSystem
HealthSystem
ShooterHUD
PauseSystem
```

## 19. 可施工 Gate

后续施工文档应按以下 gate 拆分：

```text
Gate A：AUI v1 数据结构补齐
  Canvas / Panel / Text / Image / Button / ProgressBar
  AuiBindingRef target_field / path / fallback
  ProjectUiStateSnapshot / UiValue
  AuiAction
  ProgressBar node kind
  Button / ProgressBar composite draw rule

Gate B：AUI Authoring Service headless
  create/open/edit/save/validate
  schema path transaction
  authoring report

Gate C：Runtime Binding / Present
  ProjectUiStateSnapshot resolve
  DrawList / OverlayFrame / RuntimeRenderer UI Pass
  real text glyph present

Gate D：Build / RuntimePackage 接入
  AUI manifest
  dependency validation
  build report

Gate E：Interaction Action
  Button click -> AuiAction -> Project Rule input
  consumed input trace

Gate F：Editor UI 接入
  AUI domain list
  node tree
  inspector fields
  add node commands

Gate G：End-to-end 验收
  complex shooter HUD fixture
  exported Windows player HUD present
```

Gate 顺序原则：

```text
Runtime Binding / Present 必须早于真实 Editor UI 接入。
Editor UI 可以先用 headless authoring service 验证数据链路。
真实编辑器交互必须回接同一套 AuiAuthoringService / AuiTransaction，不能另开 JSON 快捷写入路径。
```

## 20. 不做内容

M12 v1 不做：

```text
独立完整 UI Designer
拖拽布局编辑
Canvas 多模式
WorldSpace UI
复杂富文本
IME
InputField
ScrollView
复杂动画
Mask / Clip
完整主题系统
UGUI 兼容层
NGUI 兼容层
Unity4.3 IMGUI 路线
```

## 21. 方案自审

### 21.1 是否合乎规格

符合。

本文直接对应 `130` 的 M12，不新增偏离缺口清单的新系统。

### 21.2 是否合乎已有规则

符合。

```text
引擎只提供底座能力。
不为复杂打飞机增加专用 API。
AUI 是 Runtime UI，不是 Editor UI。
AUI 不进入 RenderProxy。
AUI 通过 UiProjection 进入 RuntimeRenderer。
```

### 21.3 是否合乎 Unity4.3 源码复查

符合。

```text
吸收 Unity4.3 的分层、SerializedProperty / Undo 思路。
不照搬 Unity4.3 IMGUI。
不把 Unity4.3 旧 GUI 当作现代 Runtime UI 路线。
```

### 21.4 是否方便实现

基本符合。

已有 `engine_runtime::aui`、`RuntimeRendererInput.aui_overlay`、`AuiInteractionSystem`，M12 是产品化补链，不是从零开始。

风险在 Editor Authoring / Build / Runtime 需要跨 crate 串联，施工时必须按 gate 小步测试。
外部审查指出的 `AuiBindingRef`、`AuiAction`、`ProjectUiStateSnapshot`、`ProgressBar`、字体 present 和 Gate 顺序问题已纳入本文，不再允许施工文档按旧 Gate 顺序推进。

### 21.5 是否合理且能实现

合理。

第一版只做 6 类节点、只读 binding、通用 action、ScreenOverlay present，复杂度可控，同时长期边界没有堵死。

但“可控”不等于“占位通过”。M12 完成标准必须包含真实 binding resolve、真实 action emission、真实 Text glyph present、RuntimePackage AUI manifest 和导出 Windows Player HUD 可见性。

## 22. 最终结论

M12 采用：

```text
AUI Runtime HUD Authoring / Binding / Present v1
方案 C-min
```

Unity4.3 源码复查不改变 M12 方向，只强化：

```text
不走 IMGUI 长期路线。
Editor Authoring 必须结构化 command / transaction。
AUI Runtime Core 与 Editor UI / Sprite2D / Project Rule 保持边界。
后续可视化编辑必须复用 209 Scene Unified Authoring，不新增独立 Designer。
```

下一步可基于本文生成施工文档。
