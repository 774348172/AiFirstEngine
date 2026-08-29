# 100-AUI：AI-first Runtime UI System 方案

## 当前归属说明：UiProjection

AUI Runtime Core 的结构真相仍是 `AuiDocument / AuiBlueprint / AuiTree / AuiDrawList`。AUI 进入 RuntimeRenderer 的跨域同步，从 `110-World-Projection-Adapter统一跨域同步规则.md` 起统一归属为：

```text
UiProjection
```

历史文档中的 `AuiRenderExtract / AuiRendererBridge` 只作为早期落地名保留。后续新增控件或 UI 绘制类型时，只新增 `UiProjectionAdapter` 或 AUI 内部 draw item，不新增独立 Bridge。

## 1. 系统定位

`AUI` 是本引擎的游戏运行时 UI 系统，全称建议定义为：

```text
AUI = AI-first Runtime UI System
```

它的目标不是只解决打飞机 HUD，而是成为所有类型游戏都能使用的正式 Runtime UI 系统：

```text
HUD
菜单
背包
技能栏
对话框
任务面板
设置界面
结算界面
商城界面
地图 / 小地图
多人房间 UI
复杂运营活动 UI
```

AUI 是 Runtime UI，不是编辑器 UI。

### 1.1 当前 authoring 口径修正

2026-07-06 后，AUI 可视化编辑入口以 `209-AUI-Scene-Unified-Authoring-Productization-v1方案.md` 为准：

```text
不新增独立 AUI Designer。
不新增 AUI 专用编辑模式。
Scene View / Hierarchy / Inspector 是统一 authoring surface。
2D 只是 Scene View 的视图 / 投影模式。
AUI Document 仍是 UI 结构真相。
AUI Node 不变成 Runtime ECS Entity。
AUI Node 只通过 AuiNodeAuthoringProxy 作为编辑器可选择对象出现。
当前可执行 render domain 只承认 ScreenOverlay / Modal。
跨 World 前后夹击只能在 AUI Canvas / AUI LayerGroup 粒度记录 authoring intent，并标注 runtime_supported=false。
```

本文旧段落中的 `AUI Designer` 只保留为历史泛称，不能再理解成独立 UI Designer 界面。

边界：

```text
AUI：玩家在游戏运行时看到和交互的 UI。
Editor UI：编辑器自身的 Hierarchy / Inspector / Console / Viewport / Toolbar。
Sprite2D：游戏世界中的 2D 可见对象。
```

AUI 可以复用底层 renderer / font / texture / input 基础设施，但不能和 Editor UI 混成一套业务模型。

AUI 从第一天分成两个正式域：

```text
AUI Runtime Core
  游戏运行时真正执行的 UI 系统。

AUI AI Authoring Pipeline
  AI 生成图片、拆分资源、生成 AuiBlueprint、验证和预览的生产流水线。
```

规则：

```text
AUI Runtime Core 不能被图片生成流水线污染。
AuiBlueprint / AuiDocument 仍然是 AUI 的结构真相。
AI 生成的效果图、HTML、截图、bbox、切图报告都只是 Authoring 输入和验证产物。
```

## 2. 为什么不能只叫 HUD

`HUD` 只覆盖：

```text
血条
分数
弹药
小地图
技能冷却
```

但完整游戏 UI 还需要：

```text
主菜单
设置
背包
商店
对话
任务
图鉴
角色养成
多人匹配
弹窗
复杂列表
```

因此正式系统命名应从第一天就是：

```text
AUI System
```

第一版实现可以是：

```text
AUI C-min
```

但架构边界必须按完整 UI 系统设计。

## 3. 其他 UI 系统对比

### 3.1 Unity UGUI

UGUI 是 Unity 官方 Runtime UI 系统。

核心结构：

```text
Canvas
RectTransform
Graphic
Image
Text / TextMeshPro
Button / Toggle / Slider
LayoutGroup
CanvasRenderer
EventSystem
GraphicRaycaster
```

特点：

```text
UI 基于 GameObject / Component。
RectTransform 负责屏幕空间布局。
Canvas 负责渲染批次、排序和 rebuild。
EventSystem 负责点击、拖拽、输入。
Prefab / Inspector / Scene 工作流成熟。
```

优点：

```text
用户心智成熟。
适合大多数游戏 UI。
Prefab 化能力强。
和编辑器工作流结合好。
```

缺点：

```text
Canvas rebuild 复杂且容易有性能坑。
复杂 LayoutGroup / ContentSizeFitter 容易造成隐式重算。
事件系统和渲染系统耦合较多。
AI 修改时容易改出层级很深、依赖隐蔽的 UI。
```

我们吸收：

```text
Canvas / Rect / Anchor / Image / Text / EventSystem 的用户心智。
UI Prefab 化。
屏幕空间 / 相机空间 / 世界空间 UI 的长期能力。
```

不照搬：

```text
复杂 Canvas rebuild 黑箱。
GameObject 级 UI 运行模型。
过度隐式的 LayoutGroup 链式重算。
```

### 3.2 NGUI

NGUI 是 Unity 早期常用第三方 UI 系统。

核心结构：

```text
UIRoot
UIPanel
UIWidget
UISprite
UILabel
UIButton
UICamera
UIAtlas
UIGrid / UITable
```

特点：

```text
Panel / Widget 心智简单。
Atlas / Sprite 管理清楚。
事件由 UICamera 处理。
比早期 Unity UI 更轻量直接。
```

优点：

```text
结构清楚。
Panel 控制裁剪、排序、draw call。
Widget 是直接可渲染单元。
对 2D 游戏 UI 很友好。
```

缺点：

```text
第三方插件路线，不是现代官方主线。
复杂 UI 工具链、字体、输入法、多平台适配不如现代系统。
长期维护风险高。
```

我们吸收：

```text
Panel / Widget 的简洁模型。
UI 渲染单元和裁剪区域清晰分离。
Atlas / Sprite 的资源组织思路。
```

不照搬：

```text
老式 Unity Transform 依赖。
插件式架构。
过度依赖 Atlas 的旧资源路线。
```

### 3.3 Unreal UMG / Slate

UE Runtime UI 常用 UMG，底层是 Slate。

核心结构：

```text
UUserWidget
Widget Tree
Canvas Panel
TextBlock / Image / Button
Binding
Animation
Slate Widget
Slate Application
```

特点：

```text
UMG 面向游戏 UI authoring。
Slate 面向底层 widget / editor UI。
蓝图工作流强。
复杂 UI 能力强。
```

优点：

```text
大项目能力强。
Designer / Graph / Binding / Animation 完整。
适合复杂菜单、运营界面、平台 UI。
```

缺点：

```text
体系重。
Slate / UMG 层级较多。
第一版照搬会明显过度设计。
AI 修改时如果缺少约束，容易生成复杂绑定和难查生命周期问题。
```

我们吸收：

```text
Widget Tree。
UI Blueprint / UI Asset 的长期方向。
动画和状态机作为独立层。
```

不照搬：

```text
完整 Slate 级底层 widget 框架。
复杂蓝图绑定。
编辑器 UI 和 Runtime UI 深度复用一套业务树。
```

### 3.4 Godot Control UI

Godot 使用 Control / CanvasLayer / Theme 构建 UI。

核心结构：

```text
Control
CanvasLayer
Label
TextureRect
Button
Container
Theme
Signal
```

特点：

```text
节点树心智简单。
Anchor / Offset 直观。
Container 负责布局。
Signal 负责交互回调。
```

优点：

```text
简单清晰。
适合中小型游戏快速构建 UI。
Anchor / Container 心智很好。
```

缺点：

```text
复杂项目约束弱。
大型 UI 的 patch / validation / trace 能力不是 AI-first 设计。
```

我们吸收：

```text
Anchor / Offset 简洁规则。
Control tree 心智。
Theme 统一风格。
```

### 3.5 Bevy UI

Bevy UI 基于 ECS 和 Style / Node。

核心结构：

```text
Node
Style
Text
Image
Interaction
UiCamera / Render phase
```

特点：

```text
ECS 友好。
数据驱动。
适合 Rust 架构。
```

优点：

```text
和 ECS 集成自然。
适合自动化测试和数据 diff。
```

缺点：

```text
复杂 UI authoring 心智弱于 Unity / UE。
如果直接暴露 CSS-like layout，会增加 AI patch 复杂度。
```

我们吸收：

```text
数据驱动。
ECS-friendly runtime。
headless layout / render report。
```

不照搬：

```text
第一版不做完整 CSS/Flexbox。
不把 UI 全部变成普通 Gameplay ECS 组件。
```

### 3.6 UnityAIUI / UnityCLI Web-to-UGUI 流水线

本地参考源码：

```text
<UNITY_UI_REFERENCE>\com.oathx.unitycli-master\com.oathx.unitycli-master
```

它不是 Runtime UI 系统，而是 Unity Editor 中的 AI UI 生产工具链。

核心流程：

```text
自然语言 / AI 生成 UI 效果
  -> HTML Web 原型
  -> extract-visual-ui.mjs 提取 visual-ui.json
  -> asset-manifest.json 描述独立切图资源
  -> ugui --dry-run 验证
  -> Unity Editor 主线程生成 UGUI prefab
  -> console / screenshot / save 验证
```

源码对应：

```text
unity-cli/scripts/extract-visual-ui/extract-visual-ui.mjs
Editor/Services/UnityCliWebVisualUguiPrefabService.cs
Editor/Commands/UguiUnityCLICommand.cs
unity-cli/references/asset-generation-workflow.md
unity-cli/references/ugui-component-composition.md
```

优点：

```text
非常适合 AI 生成 UI 的前期生存路线。
把效果图、结构化 JSON、资源 manifest、prefab 生成、dry-run、截图验证拆成清晰步骤。
禁止整屏图加透明热区冒充真实 UI。
强调按钮、文本、图标、列表项必须拆成可绑定、可替换、可验证的结构。
dry-run / issues / console / screenshot 对 AI 调试友好。
```

缺点：

```text
强依赖 Unity UGUI / RectTransform / prefab。
visual-ui.json 来自 HTML DOM，不适合作为我们 AUI 的长期真相。
布局第一版偏绝对像素和左上角 anchor，不足以支撑复杂多分辨率 UI。
它解决的是 Authoring / Prefab 生成，不解决 Runtime UI 生命周期、渲染、输入、绑定、Trace。
```

我们吸收：

```text
AI 生成图片 -> 资源拆分 -> 结构化 UI -> dry-run -> report -> preview -> runtime package 的流水线。
asset-manifest 的资源映射思想。
bbox review / visual diff / screenshot 验证思想。
禁止整屏截图 UI 的硬规则。
```

不照搬：

```text
不把 HTML / visual-ui.json 作为 AUI 真相。
不把 UGUI prefab 结构作为 AUI Runtime 结构。
不把绝对像素 rect 作为最终布局模型。
不让 Authoring 流水线决定 Runtime Core 架构。
```

## 4. AUI 总体目标

AUI 的长期目标：

```text
AI 能生成 UI。
AI 能修改 UI。
AI 能解释 UI 为什么显示成这样。
AI 能定位 UI bug。
复杂项目能长期维护。
运行时性能可控。
多平台输入和显示可扩展。
```

AUI 必须支持：

```text
屏幕空间 UI
相机空间 UI
世界空间 UI
多 Canvas / 多 Layer
Text / Image / Button / Toggle / Slider / List / ScrollView
Panel / Mask / Clip
Theme / Style
Animation / Transition
Focus / Navigation
Mouse / Touch / Gamepad
Data Binding
UI Prefab / UI Blueprint
Runtime Trace / Layout Report / Hit Test Report
```

第一版 C-min 不实现全部功能，但数据结构必须不把后续能力堵死。

## 5. AUI 架构分层

推荐结构：

```text
AUI AI Authoring Pipeline
  -> AUI Asset / Blueprint
  -> AUI Document
  -> AUI Runtime Tree
  -> AUI Layout Engine
  -> AUI Render Extract
  -> AUI Render Pass
  -> AUI Input / Hit Test
  -> AUI Binding / Command
  -> AUI Trace / Report
```

其中：

```text
AUI AI Authoring Pipeline 是生成入口，不是运行时真相。
AUI Asset / Blueprint 之后才进入正式 Runtime 链路。
```

### 5.0 AUI AI Authoring Pipeline

这是 AUI 的前期生存方向，用来让 AI 能快速生成可运行、可验证、可修改的游戏 UI。

目标：

```text
用户用自然语言描述 UI。
AI 生成 UI 效果图或参考图。
AI 将效果图拆成独立资源。
AI 生成 AuiBlueprint。
引擎 dry-run 验证。
引擎输出 preview / report。
确认后进入 Runtime Package。
```

标准流程：

```text
Natural Language
  -> AuiDesignBrief
  -> design-master.png
  -> AuiLayerPlan
  -> bbox review
  -> independent image assets
  -> AuiAssetManifest
  -> AuiBlueprint
  -> AuiValidationReport
  -> AuiPreview / AuiVisualDiffReport
  -> Runtime Package cook
```

关键规则：

```text
design-master.png 只是确认图，不能直接作为完整 UI 背景。
按钮、文本、图标、列表项、输入框、资源栏必须拆成独立 AuiNode。
普通文本、数字、价格、倒计时默认必须是 AUI Text，不烘焙进图片。
图片资源必须通过 AuiAssetManifest 映射到 AuiNode。
所有生成、导入、Patch 都必须支持 dry-run。
所有失败都必须输出可读 AuiValidationReport / AuiImportReport。
```

Authoring 产物：

```text
AuiDesignBrief
  ui_id
  target_resolution
  style_keywords
  required_screens
  interaction_notes

AuiLayerPlan
  node_id
  semantic_kind
  html_rect / draft_rect
  design_visual_bbox
  hit_rect
  asset_policy
  text_policy

AuiAssetManifest
  asset_id
  asset_ref
  used_by_nodes
  source_bbox
  sprite_border
  scale_policy
  import_policy

AuiGenerationReport
  generated_from
  prompt_summary
  asset_count
  node_count
  warnings
  rejected_shortcuts

AuiVisualDiffReport
  target_preview
  generated_preview
  diff_image
  mismatch_count
  review_notes
```

硬边界：

```text
HTML / Web Preview / visual extraction 只允许作为临时 Authoring 输入。
AuiBlueprint 是 AUI 的唯一结构真相。
AuiRect 是运行时布局真相。
design_visual_bbox 只用于资源裁切和视觉验证，不能成为第二套布局真相。
```

### 5.1 AUI Asset / Blueprint

AUI 界面是项目资源。

```text
AuiBlueprint
  schema_version
  blueprint_id
  authoring
  root_canvas
  nodes
  styles
  bindings
  assets
  validation_policy
```

规则：

```text
UI 是资源，不是散落代码。
AI 修改 UI 时优先修改 AuiBlueprint / AuiPatch。
项目逻辑不直接拼底层 draw command。
AuiBlueprint 可以记录 Authoring 来源，但 Runtime 不依赖 Authoring 中间文件。
```

`authoring` 是可选元数据：

```text
AuiAuthoringMeta
  source_kind
  source_artifacts
  design_resolution
  generation_report_ref
  asset_manifest_ref
```

规则：

```text
authoring 只帮助 AI 和编辑器追溯来源。
Runtime 加载 AuiBlueprint 时可以忽略 authoring。
不能因为 authoring 信息缺失导致运行时 UI 不能执行。
```

### 5.2 AUI Document

AUI Document 是编辑期和运行期共享的结构化 UI 描述。

```text
AuiDocument
  canvases
  node_tree
  style_refs
  asset_refs
  binding_refs
```

用途：

```text
Validation
Diff
AI Patch
Prefab / Blueprint
Build / Cook
Headless Test
```

### 5.3 AUI Runtime Tree

运行时加载 AUI Document 后形成 Runtime Tree。

```text
AuiRuntimeTree
  canvas_instances
  node_instances
  computed_state
  dirty_flags
```

规则：

```text
AUI Runtime Tree 是 UI 的运行时真相。
Gameplay ECS 不直接存储完整 UI 树。
AUI 可以读取项目数据快照，但不应该随意写 Gameplay ECS。
```

### 5.4 AUI Layout Engine

负责把 UI 结构计算成屏幕矩形。

第一版支持：

```text
anchor_min
anchor_max
offset_min
offset_max
pivot
size
position
z_order
```

长期支持：

```text
horizontal / vertical layout
grid
scroll content
safe area
localization text resize
responsive profile
```

规则：

```text
第一版不做完整 Flexbox。
第一版不做复杂 ContentSizeFitter。
布局必须能 headless 计算，并输出 LayoutReport。
```

### 5.5 AUI Render Extract

把 Runtime Tree 转为 UI 绘制命令。

```text
AuiRenderExtract
  AuiRuntimeTree
  AuiLayoutResult
  -> AuiDrawList
```

`AuiDrawList` 只包含可渲染命令：

```text
DrawRect
DrawImage
DrawText
DrawClipBegin
DrawClipEnd
```

规则：

```text
AUI 不进入 Sprite2D 排序。
AUI 有独立 UI render pass。
AUI draw order 由 Canvas layer / sorting_order / tree_order 决定。
```

### 5.6 AUI Input / Hit Test

负责 UI 命中和交互。

```text
PointerEvent
KeyboardEvent
GamepadNavigationEvent
HitTestResult
FocusState
NavigationState
```

规则：

```text
AUI 先处理 UI 命中。
被 UI 消费的输入不再进入 Gameplay Action。
未命中 UI 的输入继续走 InputMapping -> ActionSnapshot。
```

第一版只做：

```text
pointer hit test
click command
consume_input
```

长期再做：

```text
drag
scroll
focus navigation
gamepad navigation
text input / IME
multi-touch
```

### 5.7 AUI Binding / Command

Data Binding 是 AI-first UI 的重点，但不能做成黑箱脚本。

推荐第一版：

```text
AuiBinding
  binding_id
  target_node
  target_property
  source_kind
  source_path
  fallback_value
```

`source_kind` 第一版只允许：

```text
StaticValue
RuntimeSnapshot
ProjectUiState
```

规则：

```text
第一版不支持任意脚本 binding。
第一版不支持复杂表达式。
UI 只读项目数据快照。
UI 写入通过 AuiCommand / Project Command，不直接改 ECS。
```

这样 AI 可以清楚解释：

```text
这个 Text 为什么显示 120？
因为 node.score_text.text 绑定到 ProjectUiState.score。
```

### 5.8 AUI Trace / Report

AUI 必须可解释。

最小报告：

```text
AuiLayoutReport
  frame
  canvas_count
  node_count
  visible_node_count
  clipped_node_count
  overflow_count
  invalid_binding_count

AuiHitTestReport
  pointer
  hit_node
  consumed
  reason

AuiRenderReport
  draw_command_count
  text_count
  image_count
  batch_hint_count
```

规则：

```text
AI 默认读 report，不读底层 GPU command。
Report 字段保持 UI 通用，不出现 Health / Score / Ammo 等项目语义。
```

## 6. AUI 与其他系统的关系

### 6.1 AUI 与 Gameplay ECS

```text
Gameplay ECS：游戏对象和游戏状态。
AUI Runtime Tree：UI 对象和 UI 状态。
ProjectUiState：项目提供给 UI 的只读显示数据。
```

规则：

```text
AUI 不直接查询 Gameplay ECS。
AUI 通过 ProjectUiState / RuntimeSnapshot 读取显示数据。
AUI 交互通过 AuiCommand 发回项目层。
```

原因：

```text
避免 UI 和玩法强耦合。
避免 UI Binding 随项目组件膨胀。
方便 AI 定位 UI 数据来源。
```

### 6.2 AUI 与 Input Mapping

```text
Raw Input
  -> AUI HitTest
  -> consumed ? stop : InputMapping -> ActionSnapshot
```

规则：

```text
UI 输入优先于 Gameplay 输入。
AUI 只能消费发生在 UI 节点上的输入。
输入消费必须进入 AuiHitTestReport。
```

### 6.3 AUI 与 Renderer

```text
Game World Render Pass
Sprite2D Render Pass
AUI Render Pass
Present
```

规则：

```text
AUI 是独立 render domain。
Screen-space AUI 默认在 world/sprite 之后。
World-space / ScreenCamera AUI 只作为长期 CanvasMode 预留；209 C-min-r1 和当前 runtime 不能把它们暴露成可执行 authoring / render domain。
```

### 6.4 AUI 与 Asset Pipeline

AUI 依赖：

```text
Font
Texture
Sprite
Material preset
AuiBlueprint
AuiTheme
AuiAssetManifest
Localization table
```

规则：

```text
AUI 资源必须进入 Asset DB / Importer / Build Graph。
Build 时 cook AuiBlueprint / AuiTheme / Font / Texture 依赖。
AI 生成图片进入项目之前必须先成为受 Asset Pipeline 管理的资源。
AuiAssetManifest 负责记录图片资源与 AuiNode 的映射。
```

### 6.5 AUI 与 Editor UI

```text
Editor UI：编辑器自身工具。
AUI：游戏运行时 UI。
AUI Scene Unified Authoring：编辑器中的 AUI 可视化编辑入口，复用 Scene / Hierarchy / Inspector。
```

规则：

```text
Editor UI Renderer 不等于 AUI Runtime Renderer。
旧称 AUI Designer 只作历史泛称；当前不新增独立 Designer，AUI 可视化编辑必须走 209 Scene Unified Authoring。
Editor UI 可以预览 AUI，但不能成为 AUI 的运行时真相。
```

### 6.6 AUI 与 AI 图片生成

AUI 支持 AI 生成图片再生成 UI，但这是 Authoring Pipeline，不是 Runtime Core。

推荐路线：

```text
AI image generation
  -> design-master.png
  -> layer plan
  -> independent image assets
  -> AuiAssetManifest
  -> AuiBlueprint
```

规则：

```text
最终 UI 不能是一张整屏截图。
最终 UI 必须由 AuiNode + AuiText + AuiImage + AuiStyle + AuiBinding 组成。
图片生成只负责美术资源，不能替代 UI 结构。
如果图片中包含普通文案、数字、价格，默认必须拆成 AuiText。
如果确实是 Logo / 艺术字，必须在 AuiAssetManifest 标记 text_policy = bitmap_allowed。
```

## 7. AUI 标准数据结构

### 7.1 AuiCanvas

```text
AuiCanvas
  canvas_id
  mode
  layer
  sorting_order
  reference_resolution
  scale_mode
  root_node
```

`mode`：

```text
ScreenOverlay
ScreenCamera
WorldSpace
```

说明：

```text
ScreenCamera / WorldSpace 只是长期 schema 预留。
当前 209 C-min-r1 authoring 与 runtime present 不能把它们当作可执行 UI 层。
需要跨 World 前后夹击时，只能先在 Canvas / LayerGroup 粒度记录 VisualOrderIntent，并等待 RuntimeRenderer 多段 UI composition pass。
```

第一版只实现：

```text
ScreenOverlay
```

### 7.2 AuiNode

```text
AuiNode
  node_id
  name
  kind
  parent
  children
  rect
  visible
  interactable
  style_ref
  binding_refs
```

`kind` 长期支持：

```text
Panel
Image
Text
Button
Toggle
Slider
List
ScrollView
InputField
Custom
```

第一版只实现：

```text
Panel
Image
Text
```

### 7.3 AuiRect

```text
AuiRect
  anchor_min
  anchor_max
  offset_min
  offset_max
  pivot
  size
```

### 7.4 AuiStyle

```text
AuiStyle
  style_id
  color
  opacity
  padding
  margin
  font_ref
  font_size
  image_ref
  nine_slice
```

### 7.5 AuiCommand

```text
AuiCommand
  command_id
  source_node
  command_kind
  payload
```

第一版 `command_kind`：

```text
Click
```

长期：

```text
Click
ValueChanged
Submit
Cancel
FocusChanged
DragBegin
DragMove
DragEnd
Scroll
```

## 8. AI-first 规则

AUI 必须给 AI 提供稳定修改面。

AI 修改 UI 时优先生成：

```text
AuiPatch
```

而不是直接改运行时代码。

`AuiPatch` 类型：

```text
AddNode
RemoveNode
MoveNode
SetRect
SetStyle
SetText
SetImage
SetBinding
SetInteraction
```

Validation 必须检查：

```text
node_id 唯一
parent 存在
无循环父子关系
binding source 存在
asset_ref 存在
text 不为空时有 font fallback
image 不为空时有 texture / sprite fallback
screen overlay canvas 有 root
```

AI 默认解释字段：

```text
node_id
node_path
kind
rect
style
binding
visibility
interaction
```

AI 不应该默认解释：

```text
GPU buffer
batch key
atlas page
shader variant
```

### 8.1 AUI AI 生成与验证规则

AI 生成 AUI 时必须走结构化流程，不能直接产出不可解释的最终资源。

标准生成顺序：

```text
1. 生成 AuiDesignBrief
2. 生成或接收 design-master.png
3. 生成 AuiLayerPlan
4. 执行 bbox / asset review
5. 生成独立图片资源
6. 生成 AuiAssetManifest
7. 生成 AuiBlueprint
8. 执行 dry-run validation
9. 输出 AuiPreview / AuiReport
10. 用户确认后进入 Import / Cook
```

AI 允许做：

```text
生成 UI 效果图。
生成按钮、面板、图标、装饰等独立图片资源。
生成 AuiBlueprint。
生成 AuiPatch。
生成预览和报告。
```

AI 不允许做：

```text
用整屏图冒充 UI。
把普通文本和动态数字烘焙进按钮或面板图片。
跳过 dry-run 直接写入正式资源。
直接生成底层 DrawList 当作长期 UI 资产。
让 HTML / 截图 / bbox 成为运行时布局真相。
```

验证最小字段：

```text
AuiValidationReport
  ok
  error_count
  warning_count
  missing_asset_count
  invalid_node_count
  text_baked_warning_count
  full_screen_image_rejected
  report_items
```

第一版 `report_items`：

```text
severity
code
node_id
asset_id
message
suggested_fix
```

## 9. AUI C-min 第一版边界

第一版只做最小闭环：

```text
AuiDocument
AuiCanvas ScreenOverlay
AuiNode Panel / Image / Text
AuiRect anchor / offset
AuiLayoutEngine headless
AuiDrawList
AuiRenderReport
AuiHitTest pointer
AuiBinding StaticValue / ProjectUiState
RuntimeRenderer 接收 AUI draw list
AuiAssetManifest 最小结构
AuiValidationReport 最小结构
headless tests
```

第一版不做：

```text
Button 复杂状态
ScrollView
InputField
富文本
复杂布局系统
动画
主题继承完整系统
世界空间 UI
多语言排版
IME
AUI Scene Unified Authoring / 复杂 UI 可视化编辑
完整 AI 图片生成流水线
真实 bbox review UI
复杂 visual diff
真实 GPU 文本渲染
完整字体 atlas
```

但第一版的数据结构必须为这些能力预留位置。

## 10. 推荐施工顺序

```text
100 AUI 总方案
101 AUI C-min 数据模型 / Layout / DrawList
102 AUI Render Extract / RuntimeRenderer 接入
103 AUI Input HitTest / consume input
104 AUI Binding / ProjectUiState
105 AUI Importer / Runtime Package cook
106 AUI AI Authoring Pipeline C-min
209 AUI Scene Unified Authoring Productization v1（替代旧 AUI Designer 心智）
```

第一轮施工建议只做：

```text
101 AUI C-min 数据模型 / Layout / DrawList
```

原因：

```text
先稳定 UI 真相结构。
先让 AI 能生成 / 修改 / 验证 UI 文档。
先保证 headless layout report。
再接渲染、输入、绑定。
```

第二轮建议做：

```text
106 AUI AI Authoring Pipeline C-min
```

第一版只需要：

```text
AuiDesignBrief
AuiAssetManifest
AuiGenerationReport
AuiValidationReport
从独立图片资源 + AuiBlueprint 生成 AUI preview 的 headless 测试
拒绝整屏图 UI 的 validation
```

不需要第一版就做完整 imagegen、bbox review 交互页和高级 visual diff。

## 11. 结论

AUI 是本引擎面向所有游戏的正式 Runtime UI 系统，不是打飞机 HUD 的临时功能。

长期目标类似：

```text
Unity UGUI 的用户心智
+ NGUI 的 Panel / Widget 简洁性
+ UE UMG 的 Blueprint / Designer 能力
+ Godot Control 的 Anchor / Theme 清晰性
+ Bevy 的数据驱动和 headless test 能力
+ AI-first Patch / Validation / Trace
+ AI 图片生成到结构化 UI 的 Authoring Pipeline
```

第一版采用 C-min，不追求完整 UGUI 级能力，但从第一天按完整 UI 系统的边界设计。

最终路线：

```text
AUI Runtime Core 保持稳定、简单、可测试。
AUI AI Authoring Pipeline 负责让 AI 快速生成 UI、拆资源、验证、预览和转成 AuiBlueprint。
两者通过 AuiBlueprint / AuiAssetManifest / AuiReport 连接。
```
