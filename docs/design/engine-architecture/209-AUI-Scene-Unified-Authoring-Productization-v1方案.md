# 209-AUI Scene Unified Authoring Productization v1 方案

## 1. 系统定义

本系统正式命名为：

```text
AUI Scene Unified Authoring Productization v1
```

采用用户确认并经 28 号审查修订后的：

```text
方案 C-min-r1：Truthful Scene Unified AUI Authoring
```

一句话：

```text
AUI 不开独立 Designer 界面，也不新增 AUI 专用编辑模式；
AUI 作为 Scene 统一编辑链路中的可选择、可检查、可修改对象，
复用 Scene View / Hierarchy / Inspector / 2D View / Command / Transaction / Report。
```

这次修正来自用户提供的 Unity Scene 操作视频。视频中的关键事实是：

```text
Unity 没有打开单独 UI Designer。
用户仍在 Scene tab 中编辑。
2D 按钮只是 Scene View 的观察 / 投影视图模式。
UIRoot / RootCanvas / EventSystem / UI 子对象仍出现在 Hierarchy。
点选对象后，Inspector 显示该对象组件。
UI 编辑复用 Scene 的选择、变换工具、Hierarchy、Inspector 和 Undo/Dirty 链路。
```

因此本项目 209 的正确方向不是“单独 AUI Designer”，也不是“Scene 里的 AUI 专用编辑模式”，而是：

```text
Scene View 是唯一主编辑表面。
2D 只是 Scene View 的视图模式。
AUI Document / AUI Node 通过 editor authoring proxy 接入 Scene 的统一选择与 Inspector。
真实修改仍写回 AUI Document。
```

## 2. 为什么要改掉独立 Designer 心智

之前的 209 草案把 AUI Designer 设计成：

```text
左侧 AUI node tree
中间 AUI canvas preview
右侧 AUI inspector
```

这个方向的问题是：

```text
它新增了一个和 Scene 并列的大编辑界面。
用户需要理解 Scene 和 AUI Designer 两套入口。
Scene 里看到的游戏画面和 HUD 编辑被拆开。
后续还容易出现两套选择、两套 Inspector、两套 hit-test、两套编辑报告。
```

Unity 视频证明更简单的用户心智是：

```text
所有可编辑内容都在 Scene / Hierarchy / Inspector 中统一工作。
2D / 3D 只是视角切换，不是业务编辑模式。
UI 对象与场景对象共享选择和 Inspector 心智。
```

本项目应学习这个心智，但不照搬 Unity 的 GameObject / RectTransform 真相层。

## 3. 在本引擎中的作用

当前 AUI 正式链路仍是：

```text
AUI Document
  -> RuntimePackage
  -> ProjectUiStateSnapshot
  -> Binding Resolve
  -> AuiLayout / AuiDrawList
  -> UiProjection / AuiOverlayFrame
  -> RuntimeRenderer UI Pass
  -> Present
```

209 补的是编辑器统一 authoring 入口：

```text
Scene View / Hierarchy / Inspector
  -> AuiNodeAuthoringProxy
  -> WorkspaceSelectionTarget::AuiNode
  -> existing AUI UiCommandPayload
  -> EditorSession AUI service
  -> AuiAuthoringService
  -> saved canonical AuiDocument
  -> existing RuntimePackage / Present chain
```

关键边界：

```text
AuiNodeAuthoringProxy 是编辑器 view model，不是 Runtime Entity。
AUI Node 不变成 Runtime ECS Entity。
AUI Node 在编辑器中可以作为 Scene authoring selectable object 出现在 Scene / Hierarchy / Inspector。
AUI Document 仍是 UI 结构真相。
Scene View 只提供统一显示、选择、拖拽排序和编辑入口。
Hierarchy 默认显示视觉合成顺序，不默认显示底层存储分桶。
```

### 3.1 审查后修订结论：前后夹击问题

根据 `其它AI审查目录/28-209-AUI-Scene-Unified-Authoring方案审查.md`，209 需要收敛成：

```text
方案 C-min-r1：Truthful Scene Unified AUI Authoring
```

核心修正：

```text
Hierarchy 可以给用户视觉合成顺序心智，但不能谎报 runtime 已支持的渲染能力。
UI 与 World 前后夹击的最小安全粒度是 AUI Canvas / AUI LayerGroup，不是任意单个 AUI Node。
单个 AUI Node 如果要跨到 Scene Entity 前后，必须先显式抽成 AUI LayerGroup / AUI Canvas。
C-min-r1 不自动包裹 node，不隐式改 AUI Document 结构。
```

一个 UI 节点在场景后、另一个 UI 节点在场景前，应表达为：

```text
Scene
  bg-ui-layer                         [AUI Canvas / LayerGroup]
    bg-frame                          [AUI Node]
  World
    Character                         [Scene Entity]
  hud-ui-layer                        [AUI Canvas / LayerGroup]
    hp-bar                            [AUI Node]
    skill-buttons                     [AUI Node]
```

不能表达为：

```text
one-canvas
  bg-frame                            [AUI Node behind World]
  Character                           [Scene Entity]
  hp-bar                              [AUI Node before World]
```

原因：

```text
一个 Canvas / LayerGroup 是 layout、batch、hit-test、render composition 的边界。
任意 node 级跨 World 混排会破坏 UI batching、透明排序和命中测试边界。
Unity / UE 实际也把这种能力约束在 Canvas / Widget pass 粒度，而不是普通 node 粒度。
```

C-min-r1 的真相边界：

```text
当前 runtime 只有 ScreenOverlay AUI pass，固定在 World / Sprite 后。
C-min-r1 可以建立视觉顺序 authoring model，但跨 World 前后夹击必须标注 runtime_supported=false。
真正运行时兑现前后夹击，需要后续 RuntimeRenderer 多段 UI composition pass。
```

AUI LayerGroup 定义：

```text
AUI LayerGroup 是 AUI Document 内显式声明的 UI subtree composition boundary。
它不是 Runtime ECS Entity。
它不是编辑器自动偷偷插入的父节点。
它用于让一个 AUI subtree 在 authoring / future runtime composition 中作为一个可排序单元。
C-min-r1 只允许用户通过显式 Extract To AUI LayerGroup / Canvas 创建它。
```

## 4. 其它引擎 / 工具对标

### 4.1 Unity Scene / Canvas / RectTransform

对标心智：

```text
Scene tab 是主编辑表面。
2D 是 Scene View 的视图切换。
Canvas / UI child 出现在 Hierarchy。
点选 UI 后 Inspector 显示 RectTransform / Image / Text / Button 等组件。
```

本项目已有源码参考锚点：

```text
框架设计/Unity源码参考/README.md
Editor/Mono/SceneView/SceneView.cs
Runtime/Transform/ScriptBindings/RectTransform.bindings.cs
Modules/UIElements/Core/VisualElement.cs
Unity4.3.1f1-参考源码文档.md
```

可学习点：

```text
Scene View / Hierarchy / Inspector 统一用户心智。
2D 是 view projection，不是 UI 编辑模式。
UI 编辑不需要独立大界面。
编辑动作必须进入序列化、Undo、Dirty、保存链路。
```

不照搬：

```text
不把 AUI Node 变成 GameObject。
不引入 RectTransform 作为本项目真相层。
不把 AUI Binding 放进 MonoBehaviour 字段。
不引入 Unity 多套 UI 历史复杂度。
```

### 4.2 Unreal UMG / Widget Blueprint

可学习：

```text
UMG 修改 WidgetTree 时进入 transaction / dirty / save。
复杂 UI 工具不能绕过正式编辑事务。
```

不照搬：

```text
不做独立 UMG clone。
不引入 UObject / Slate / Blueprint 全体系。
```

### 4.3 Godot Control / Editor

可学习：

```text
场景树、Inspector、Canvas 编辑工具可以统一服务 UI 与普通节点。
Inspector 是属性编辑 surface，不是业务逻辑中心。
```

不照搬：

```text
不把 Godot Node / Signal 作为 AUI 真相。
```

### 4.4 Bevy UI

可学习：

```text
UI layout / computed rect / hit-test / render extraction 分阶段。
结构化 report 有利于 headless deterministic gate。
```

不照搬：

```text
不让用户手写 Rust UI tree。
不直接编辑运行时 ECS World。
```

## 5. 本项目当前基线

已完成：

```text
190-AUI-RuntimePackage-Document-Hydration-Binding-Present-v1
  AUI Document 已能进入 RuntimePackage 并运行时 present。

199-AUI-ProjectUiStateSnapshot-Producer-v1
  Binding 只读 ProjectUiStateSnapshot。

204-AUI-Document-Authoring-Productization-v1
  AUI command / AuiAuthoringService / EditorSession service / preview report 已存在。

207-ProjectPatch-All-Domain-Capability-v2
  ProjectPatch 已覆盖 AUI domain。

208-Runtime-Text-Glyph-Present-AUI-Text-Rendering-Productization-v1
  RuntimePackage cooked FontAtlas / glyph_present evidence 已完成。
```

当前代码事实：

```text
rust/crates/editor_ui_model/src/workspace.rs
  WorkspaceSelectionTarget 当前有 Entity / Asset / Prefab / Rule / AuiDocument 等。
  还没有 AuiNode selection。

rust/crates/editor_ui_model/src/command.rs
  AUI UiCommandPayload 已存在。

rust/crates/editor_core/src/services/aui_service.rs
  create/open/add_node/set_field/set_binding/set_action/validate/save/preview 已存在。

rust/crates/editor_core/src/ui_model_composer.rs
  Inspector 当前主要根据 Scene Entity / Rule / Asset 等 selection 生成。
  还没有 AUI Node Inspector。

rust/crates/editor_ui_renderer/src/renderer.rs
  ViewportTextureSlot / HitTarget::Viewport 已存在。
  还没有 Scene View AUI authoring overlay hit regions。
```

当前缺口：

```text
Scene View 不能显示可选择的 AUI authoring overlay。
Hierarchy 不能以 Scene 统一心智展示 AUI Document / AUI Node。
WorkspaceSelectionTarget 没有 AuiNode。
Inspector 不能根据 AuiNode selection 显示 AUI 字段。
Scene View pointer hit-test 不能在 AUI proxy 与 Scene Entity 之间统一分派。
没有 AUI Scene Unified Authoring report。
AuthoringAiContext 还不能说明 AUI 现在走 Scene 统一编辑链路。
```

## 6. 方案选项

### 6.1 方案 A：独立 AUI Designer

做法：

```text
新增一个和 Scene 并列的 AUI Designer 工作区。
```

优点：

```text
概念上容易单独实现。
不会立刻碰 Scene View hit-test。
```

缺点：

```text
新增一套编辑界面和用户心智。
与 Unity 视频中的工作方式不一致。
后续可能出现 Scene 和 AUI Designer 两套选择 / Inspector / 预览。
```

结论：

```text
不采用。
```

### 6.2 方案 B：Scene 中新增 AUI 专用编辑模式

做法：

```text
在 Scene View 中加入 AUI Edit Mode。
AUI Edit Mode 开启时只编辑 AUI。
```

优点：

```text
比独立界面少一层。
能在 Scene 中点选 AUI。
```

缺点：

```text
仍然把 2D / AUI / Scene 选择混成专用模式。
用户需要理解“现在是不是 AUI 编辑模式”。
不符合视频中 2D 只是视图模式的事实。
```

结论：

```text
不采用。
```

### 6.3 方案 C-min-r1：Truthful Scene Unified AUI Authoring

做法：

```text
不新增独立界面。
不新增 AUI 专用编辑模式。
Scene View 统一显示 Scene + AUI authoring overlay。
Hierarchy 默认按视觉合成顺序统一展示 Scene Entity + AUI Canvas / AUI LayerGroup。
同一 AUI Canvas 内再展示 AUI Node 树。
Inspector 统一根据 selection 显示 Entity 或 AUI Node。
Hierarchy 拖拽排序生成 VisualOrderKey / sibling_order 修改命令。
跨 World 前后夹击只允许在 AUI Canvas / LayerGroup 粒度表达。
当前 runtime 不支持的跨域排序必须在 report / AI context 中标注 runtime_supported=false。
2D 只作为 Scene View 投影 / 观察模式。
```

优点：

```text
最接近 Unity 视频的心智。
用户只理解 Scene / Hierarchy / Inspector。
用户可以通过拖拽直观改变前后层级关系。
AI 也只需要读取统一 WorkspaceSelection / Inspector / Command / Report。
不会让 authoring 承诺超过 runtime 现状，避免“能拖不能看”的假象。
不新增运行时层，不新增 UI 真相。
```

缺点：

```text
需要把 AUI authoring proxy 接入 Scene View hit-test。
需要扩展 WorkspaceSelectionTarget 和 Inspector composer。
需要新增视觉顺序 authoring model，避免 Hierarchy 只显示工程分桶。
Scene Entity 空间拾取当前缺失，需要单列前置 Gate。
跨域统一排序基础设施当前是 0->1，需要单列 Gate。
任意单个 AUI Node 与 Scene Entity 穿插排序，第一版拒绝并给出结构化提示；用户需显式抽成 AUI LayerGroup / Canvas。
```

结论：

```text
采用。
```

## 7. 正式推荐方案：C-min-r1

采用：

```text
方案 C-min-r1：Truthful Scene Unified AUI Authoring
```

C-min-r1 链路：

```text
Open Project / Scene
  -> collect AUI documents
  -> build AuiNodeAuthoringProxy from AuiDocument + AuiLayout
  -> build SceneVisualOrderAuthoringModel from Scene Entity + AUI Canvas / LayerGroup
  -> annotate visual order runtime support truth
  -> Scene View draws AUI authoring overlay
  -> Hierarchy exposes visual composition order entries
  -> pointer selects AuiNode or Scene Entity through one hit-test path
  -> hierarchy drag changes VisualOrderKey or AUI sibling order
  -> unsupported cross-domain reorder emits structured diagnostic
  -> WorkspaceSelectionTarget::AuiNode
  -> Inspector displays AUI node fields
  -> existing AUI command/service saves document
  -> Validate / Preview / Report
  -> complex shooter e2e unified authoring report
```

## 8. 必做能力

### 8.1 Scene View 2D 是视图模式

新增或明确：

```text
SceneViewProjection:
  Perspective
  Orthographic2D
```

规则：

```text
2D 只改变 Scene View 投影、网格、相机观察方式。
2D 不等于 AUI 编辑模式。
2D 不决定 selection domain。
```

### 8.2 AuiNodeAuthoringProxy

新增编辑器侧 proxy：

```text
AuiNodeAuthoringProxy:
  document_path
  document_id
  node_id
  parent_node_id
  name
  kind
  source_rect: AuiRect
  rect: AuiComputedRect
  visible
  interactable
  binding_count
  action_count
  selectable
  diagnostics
```

规则：

```text
Proxy 来自 AUI Document + AuiLayoutResult。
source_rect 来自 AuiNode.rect，是作者输入的 anchor / offset / pivot 数据。
rect 来自 AuiComputedNode.rect，是 layout 后用于显示和 hit-test 的屏幕矩形。
Proxy 只用于 Scene View / Hierarchy / Inspector。
Proxy 不进入 RuntimePackage。
Proxy 不写入 Scene 文件。
Proxy 不能成为 ECS Entity。
```

### 8.3 统一 Selection

扩展 selection：

```text
WorkspaceSelectionTarget::AuiNode {
  document_path,
  document_id,
  node_id
}
```

规则：

```text
Scene Entity selection 和 AUI Node selection 共用 WorkspaceSelectionSummary。
Inspector 只根据当前 selection 决定展示内容。
AI context 也读取同一 selection 真相。
```

### 8.4 Scene View 统一 Hit-Test

Scene View pointer 命中顺序 C-min-r1：

```text
Editor gizmo / active handle
  -> AUI authoring proxy selectable region
  -> Scene entity selectable region
  -> empty scene
```

C-min-r1 不做复杂选择过滤，但 report 必须说明：

```text
hit_target_kind
hit_document_path
hit_node_id
hit_entity_id
selection_changed
```

后续可扩展：

```text
layer visibility
selection lock
UI overlay visibility
multi-select
pick-through
```

### 8.5 Hierarchy 视觉顺序统一展示与拖拽排序

Hierarchy 不应默认展示成工程分桶：

```text
Scene
  AUI
    [Before World] background-ui.aui
    [After World] main-hud.aui
    [Modal] login-popup.aui
  World
    Character
    Props
```

这种展示是工程分桶，不是创作者看到画面的方式。正式方向是：

```text
Hierarchy 默认显示视觉合成顺序。
用户在 Hierarchy 上下拖拽，就等价于改变最终画面前后关系。
底层存储仍保持 Scene Document / AUI Document 各自真相。
runtime 当前不能兑现的跨域前后关系必须被清楚标注，不能假装可运行。
```

Hierarchy C-min-r1 推荐展示：

```text
Scene
  Main Camera                         [Scene Entity]
  background-ui                       [AUI Canvas, runtime_supported=false until composition pass]
    bg-image                          [AUI Node]
    bg-vfx-frame                      [AUI Node]
  IslandBackground                    [Scene Entity]
  Character                           [Scene Entity]
  nameplate-ui                        [AUI LayerGroup, runtime_supported=false until composition pass]
    hp-bar                            [AUI Node]
    name-text                         [AUI Node]
  Props                               [Scene Entity]
  main-hud                            [AUI Canvas, ScreenOverlay]
    top-currency-bar                  [AUI Node]
    bottom-buttons                    [AUI Node]
  login-popup                         [AUI Canvas / Modal]
    mask                              [AUI Node]
    panel                             [AUI Node]
    confirm-button                    [AUI Node]
```

新增 editor authoring view model：

```text
SceneVisualOrderAuthoringEntry:
  entry_id
  display_name
  target_kind: SceneEntity | AuiCanvas | AuiLayerGroup | AuiNode
  target_ref
  parent_entry_id
  visual_order_key
  visual_order_intent
  runtime_supported
  runtime_support_reason
  can_reorder
  reorder_scope
  diagnostics

VisualOrderKey:
  render_space: ScreenOverlay | Modal
  layer
  order
  local_order

VisualOrderIntent:
  relation: None | Before | After
  target_kind: SceneEntity | AuiCanvas | AuiLayerGroup | AuiNode
  target_ref
  reason
```

v1 规则：

```text
ScreenOverlay 表示当前 RuntimeRenderer 已支持的 AUI overlay pass，固定在 World / Sprite 后。
Modal 表示同属 overlay 之后的更高 UI 层，可在 authoring/report 中区分，runtime 可先映射到同一 overlay pass 的更高 order。
ScreenCamera / WorldSpaceUi / Debug / BeforeWorld 等 render_space 不进入 C-min-r1 的可执行枚举。
需要这些能力时，必须等后续 RuntimeRenderer 多段 UI composition pass / WorldSpace UI 方案。
```

拖拽规则：

```text
拖动 Scene Entity 到另一个 Scene Entity 前后
  -> 走已有 Scene sibling / render sorting 修改命令。

拖动 AUI Canvas / LayerGroup 在 ScreenOverlay / Modal UI 域内排序
  -> 修改该 canvas / layer group 的 VisualOrderKey / canvas.layer / canvas.sorting_order。
  -> runtime_supported=true。

拖动 AUI Canvas / LayerGroup 到 Scene Entity 前后，形成 World 前后夹击
  -> C-min-r1 允许记录 VisualOrderIntent。
  -> runtime_supported=false。
  -> report 增加 deferred_to_runtime_composition_gate diagnostic。
  -> Editor 必须提示当前 Player 不会按该跨域顺序渲染。

拖动 AUI Node 在同一 AUI Canvas / LayerGroup 内上下移动
  -> 修改 AUI children sibling order / tree_order。
  -> runtime_supported=true。

拖动单个 AUI Node 到 Scene Entity 前后
  -> C-min-r1 拒绝该拖拽。
  -> 不自动生成 LayerGroup。
  -> 输出 structured hint：先显式 Extract To AUI LayerGroup / Canvas，再移动该 LayerGroup。
```

规则：

```text
Hierarchy entry 是 visual authoring view，不等于底层存储树。
Scene Entity 仍写 Scene Document。
AUI Canvas / LayerGroup / Node 仍写 AUI Document。
点击 AUI node entry 只改变 WorkspaceSelectionTarget::AuiNode。
拖拽排序必须生成结构化 command，不允许直接改 renderer draw item。
report 必须说明拖拽前后 visual_order_key、目标对象和落点是否合法。
report 必须说明 visual_order_intent。
report 必须说明 runtime_supported / runtime_support_reason。
不允许一次拖拽隐式注入新的 AUI parent node 或 LayerGroup。
```

注意：

```text
当前 runtime 已有 ScreenOverlay AUI pass，默认在 World / Sprite 后。
视觉顺序 authoring 是 209 必须建立的编辑心智。
真实运行时 World 前后夹击，需要 RuntimeRenderer / RenderGraph 后续支持多段 UI composition pass。
C-min-r1 不能伪装已经完成任意 node 级 runtime 混排。
```

### 8.6 Inspector 统一编辑

AuiNode Inspector C-min-r1 字段：

```text
document_path
node_id
name
kind
text
visible
interactable
consumeInput
rect
style.color
style.text_color
style.font_size
binding path
action ref
```

编辑路径：

```text
Inspector field edit
  -> UiCommandPayload::SetAuiNodeField / SetAuiBindingPath / SetAuiActionRef
  -> EditorSession AUI service
  -> AuiAuthoringService
  -> save canonical AuiDocument
  -> refresh Scene View proxies / Inspector
```

禁止：

```text
Inspector 直接 fs::write AUI JSON。
Inspector 直接改 runtime World。
Inspector 直接改 renderer draw command。
```

### 8.7 Report

新增：

```text
AuiSceneUnifiedAuthoringReport:
  schema_version
  status
  scene_path
  aui_document_count
  proxy_count
  selectable_proxy_count
  selected_target_kind
  selected_document_path
  selected_node_id
  visual_order_entry_count
  selected_visual_order_key
  selected_visual_order_intent
  visual_order_runtime_supported
  visual_order_runtime_support_reason
  deferred_to_runtime_composition_gate
  reorder_supported
  last_reorder_status
  inspector_field_count
  hit_test_status
  command_roundtrip_ok
  validation_ok
  glyph_present
  diagnostics
  next_actions
```

报告必须进入：

```text
AuthoringAiContext
ManualWalkthroughCoverageReport
project_e2e_gate complex shooter report
```

AuthoringAiContext 必须额外暴露：

```text
visual_order_runtime_supported
visual_order_runtime_support_reason
runtime_composition_gap_count
next_required_runtime_gate
```

## 9. C-min-r1 不做

本阶段明确不做：

```text
独立 AUI Designer 界面。
AUI 专用编辑模式。
拖拽创建控件。
拖拽移动 / resize。
Rect handle。
复杂对齐辅助线。
复杂多 Canvas 生命周期 / 模板 / 批量管理 UI。
world-space UI 编辑。
任意 AUI Node 与 Scene Entity 直接混排的运行时渲染。
跨 World 前后夹击的真实 RuntimeRenderer 多段 UI composition pass。
拖拽单个 AUI Node 到 Scene Entity 前后时自动生成 LayerGroup。
隐式改写 AUI Document 层级结构。
复杂 ScrollView / InputField / IME。
UI animation / transition。
Theme editor。
真实 GPU screenshot visual diff。
完整 FontImporter / CJK shaping / emoji / rich text。
打飞机专用 HUD API。
新的运行时 AUI 层。
```

## 10. 数据与边界规则

必须遵守：

```text
AUI Document 是唯一 UI 结构真相。
AUI Node 不变成 Runtime ECS Entity。
AUI Node 可以作为编辑器 Scene authoring selectable object 出现。
AuiNodeAuthoringProxy 只是编辑器 view model。
SceneVisualOrderAuthoringEntry 只是编辑器 visual authoring view model。
Hierarchy 默认显示视觉合成顺序，不默认显示底层存储分桶。
Hierarchy 拖拽排序只修改 VisualOrderKey / Scene sibling / AUI sibling，不直接改 renderer draw item。
VisualOrderKey 在 C-min-r1 只声明 ScreenOverlay / Modal 可执行域。
跨 World 前后夹击只能作为 authoring intent，必须标注 runtime_supported=false。
单个 AUI Node 不能直接跨到 Scene Entity 前后；必须先显式抽成 AUI LayerGroup / Canvas。
2D 只是 Scene View 视图模式。
Scene View / Hierarchy / Inspector 只是统一 authoring surface。
AUI Binding 只读 ProjectUiStateSnapshot。
AUI action 是业务级 UI 意图，进入 Project Rule / Project Module。
Renderer 只读 AuiOverlayFrame，不读 authoring proxy。
```

## 11. 与已有系统关系

### 11.1 与 204

204 提供 AUI command/service/report。

209 只把这些能力接入 Scene 统一编辑入口，不新增 AUI 修改后门。

### 11.2 与 207

207 的 ProjectPatch AUI capability 仍可用。

AI 可以：

```text
读取 Scene unified selection / Inspector / report
  -> 生成 ProjectPatch 或 UiCommandPayload
  -> 走既有 validate / apply / transaction
```

### 11.3 与 208

208 已完成 glyph present。

209 不应再把 `runtime_text_glyph_present` 作为 blocking gap。
如果编辑器预览局部拿不到字体资产，应报告：

```text
scene_aui_preview_font_atlas_unavailable
```

### 11.4 与 Scene Editing

209 扩展的是 Scene authoring surface，不改变 Scene 文件真相：

```text
Scene Entity 仍写 Scene Document。
AUI Node 仍写 AUI Document。
统一的是编辑器选择、Inspector 与视觉顺序 authoring view，不是底层存储。
VisualOrderKey 是 authoring / composition 语义，不能让 AUI Node 变成 ECS Entity。
```

### 11.5 与后续 RuntimeRenderer 多段 UI composition pass

209 C-min-r1 只负责建立真实、可审查的 authoring 入口，不负责在本阶段完成跨 World 前后夹击的 runtime 渲染。

直接后续必须单列：

```text
RuntimeRenderer Multi-stage UI Composition Pass
```

目标：

```text
把当前单一 AUI ScreenOverlay pass 扩展为至少：
  AUI Background / BeforeWorld pass
  World / Sprite pass
  AUI Foreground / ScreenOverlay pass
  AUI Modal pass
```

在该后续完成前：

```text
VisualOrderKey 跨 World 排序只能作为 authoring intent。
AuiSceneUnifiedAuthoringReport 必须输出 deferred_to_runtime_composition_gate。
AuthoringAiContext 必须让 AI 知道 visual_order_runtime_supported=false。
```

### 11.6 与后续 AUI Prefab / Template Reuse

复杂 UI 项目不能长期靠复制粘贴 AUI Node 树维护。

209 不实现 AUI Prefab / Template，但必须把它作为直接后续方案，而不是埋在“不做”清单里：

```text
AUI Prefab / Template Reuse Productization v1
```

它负责：

```text
可复用 UI item cell。
列表 / 商店 / 背包 / 角色卡片模板。
Prefab override / variant / instance diagnostics。
AI 可安全批量修改模板而不是散改几百个节点。
```

`211-AUI-Prefab-Template-Reuse-Productization-v1方案.md` 固定后续规则：复杂 AUI 控件用 AUI Document subtree 表达，而不是把一个 `AuiNode` 当作 Unity GameObject Component 容器。209 的 `AuiNodeAuthoringProxy` 只负责选择和编辑这些 subtree 中的节点；复用、实例、override 和 template diagnostics 进入 211。

## 12. 可施工 Gate

### Gate A：Model / Selection Schema

目标：

```text
新增 AuiNodeAuthoringProxy。
新增 SceneVisualOrderAuthoringEntry / VisualOrderKey / VisualOrderIntent。
新增 WorkspaceSelectionTarget::AuiNode。
新增 AuiSceneUnifiedAuthoringReport schema。
VisualOrderKey C-min-r1 只允许 ScreenOverlay / Modal executable domain。
Report schema 包含 visual_order_runtime_supported / deferred_to_runtime_composition_gate。
```

测试：

```powershell
cargo test -p editor_ui_model aui_scene
cargo test -p editor_core aui_scene
```

### Gate B：Scene View AUI Overlay Model

目标：

```text
从 AUI Document + AuiLayoutResult 生成 Scene View overlay proxies。
支持 Perspective / Orthographic2D 视图模式下的 preview metadata。
```

测试：

```powershell
cargo test -p editor_core aui_scene
cargo test -p engine_runtime aui
```

### Gate C0：Hierarchy Visual Order Model Foundation

目标：

```text
建立 SceneVisualOrderAuthoringModel。
当前 Scene Entity / AUI Canvas / AUI LayerGroup / AUI Node 都能生成 VisualOrderAuthoringEntry。
Hierarchy 渲染不再依赖底层容器迭代序，必须有稳定 sort key。
工程分桶只允许作为调试视图，不作为默认 authoring 视图。
```

测试：

```powershell
cargo test -p editor_ui_model hierarchy_visual_order
cargo test -p editor_core hierarchy_visual_order
```

### Gate C1：Hierarchy Selection / Reorder Integration

目标：

```text
Hierarchy 默认按视觉合成顺序展示 Scene Entity + AUI Canvas / LayerGroup / Node authoring entries。
点击 AUI hierarchy entry 可设置 AuiNode selection。
UI 域内拖拽排序可生成 VisualOrderKey / AUI sibling order 修改命令。
拖动 AUI Canvas / LayerGroup 到 Scene Entity 前后时只记录 VisualOrderIntent，并标 runtime_supported=false。
拖动单个 AUI Node 到 Scene Entity 前后必须拒绝，并输出 Extract To AUI LayerGroup / Canvas 的结构化提示。
report 可说明 reorder_supported / last_reorder_status / selected_visual_order_key / selected_visual_order_intent / runtime_supported。
```

测试：

```powershell
cargo test -p editor_ui_renderer hierarchy
cargo test -p editor_core authoring_workflow
```

### Gate D0：Scene Entity 空间拾取前置

目标：

```text
补齐 Scene View 坐标到 Scene Entity 的 authoring pick path。
Viewport pointer 不再只表示 focus viewport。
Scene Entity pick report 能说明 pointer、candidate_count、selected_entity_id、diagnostics。
```

测试：

```powershell
cargo test -p editor_input scene_pick
cargo test -p editor_core scene_pick
```

### Gate D：Scene View Hit-Test Integration

目标：

```text
Scene View hit-test 能选择 AUI node proxy。
hit-test 与 Hierarchy visual order 使用同一套 visual order source。
未命中 AUI 时保留 Scene entity selection 路径。
AUI overlay hit region rebuild 条件明确。
Gizmo / AUI / Scene Entity hit-test 注册序稳定。
```

测试：

```powershell
cargo test -p editor_ui_renderer aui_scene
cargo test -p editor_input aui_scene
```

### Gate E：Inspector Command Roundtrip

目标：

```text
选中 AuiNode 后 Inspector 显示 AUI 字段。
编辑字段走已有 AUI UiCommandPayload / AuiAuthoringService。
保存后刷新 proxy / Inspector。
```

测试：

```powershell
cargo test -p editor_core aui_authoring
cargo test -p editor_core aui_scene
```

### Gate F：Manual Walkthrough / AI Context / E2E

目标：

```text
ManualWalkthrough 能报告 AUI Scene unified authoring。
AuthoringAiContext 能说明 AUI 走 Scene 统一编辑链路。
AuthoringAiContext 能说明 visual_order_runtime_supported / runtime_composition_gap_count。
project_e2e_gate 生成 complex-shooter-aui-scene-unified-authoring-report.json。
```

测试：

```powershell
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core manual_walkthrough
cargo test -p project_e2e_gate aui_scene
```

### Gate G：整体回归与文档同步

目标：

```text
AUI runtime / package / authoring / project patch 既有能力不回退。
入口文档和完成记录按真实施工结果同步。
```

测试：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_core
cargo test -p engine_runtime aui
cargo test -p project_e2e_gate
```

## 13. 第一版验收标准

必须证明：

```text
没有新增独立 AUI Designer 界面。
2D 只作为 Scene View 视图模式存在。
AUI document / node 能进入 Scene 统一 authoring model。
Hierarchy 默认按视觉合成顺序看到 Scene Entity + AUI Canvas / LayerGroup / Node authoring entry。
Hierarchy UI 域内拖拽排序能生成结构化 reorder command / report。
Hierarchy 跨 World 拖拽排序必须输出 runtime_supported=false / deferred_to_runtime_composition_gate。
单个 AUI Node 拖到 Scene Entity 前后必须被拒绝，并提示先 Extract To AUI LayerGroup / Canvas。
Scene View 能点选 AUI node proxy。
Scene Entity 空间拾取有明确 report；不能假装已有 entity pick path。
WorkspaceSelectionTarget::AuiNode 能进入 Inspector。
Inspector 能通过已有 AUI command 修改 AUI node 字段。
保存后的 AUI Document 仍能被 AuiDocumentCooker / RuntimePackage 消费。
complex shooter e2e 生成 AUI Scene unified authoring report。
runtime_text_glyph_present 不再作为 blocking gap。
```

允许保留：

```text
拖拽创建控件未完成。
拖拽移动 / resize 未完成。
Rect handle 未完成。
复杂多 Canvas 生命周期 / 模板管理未完成。
任意单个 AUI Node 与 Scene Entity 的真实运行时混排未完成。
跨 World 前后夹击的真实 runtime 渲染未完成，但必须有 runtime_supported=false 报告。
真实 GPU screenshot visual diff 未完成。
```

不允许：

```text
用 debug overlay 假装 AUI authoring overlay。
AUI Node 变成 Runtime ECS Entity。
AUI authoring proxy 写入 RuntimePackage。
SceneVisualOrderAuthoringEntry 写入 RuntimePackage。
Inspector 绕过 EditorSession / AuiAuthoringService。
为了 walkthrough pass 伪造空 path / 空 node_id。
把 2D 误写成 AUI 专用编辑模式。
把工程分桶 `[Before World] / [After World] / [Modal]` 当作默认 Hierarchy 用户体验。
让 VisualOrderKey 暴露 ScreenCamera / WorldSpaceUi / Debug / BeforeWorld 等 runtime 尚未支持的可执行枚举。
跨 World 排序 report 缺少 runtime_supported / deferred_to_runtime_composition_gate。
拖拽单个 AUI Node 到 Scene Entity 前后时自动注入 LayerGroup。
```

## 14. 方案自审

### 是否符合用户视频和修正意见

符合。

```text
视频中的 Unity 工作方式是 Scene / Hierarchy / Inspector 统一编辑。
本文明确取消独立 Designer 和 AUI 专用编辑模式。
本文进一步要求 Hierarchy 默认显示视觉合成顺序，而不是 AUI / World 工程分桶。
用户可以通过拖拽改变 AUI Canvas / LayerGroup 与 Scene Entity 的 authoring 前后关系。
跨 World 前后夹击在 runtime 未支持时必须诚实标注。
```

### 是否增加不必要层级

没有。

```text
AuiNodeAuthoringProxy 是编辑器 view model，不是运行时层。
SceneVisualOrderAuthoringEntry 是编辑器 visual authoring view model，不是运行时层。
AuiSceneUnifiedAuthoringReport 是验证报告，不是业务层。
真实修改继续走已有 AUI command / EditorSession / AuiAuthoringService。
runtime 多段 UI composition pass 不塞进 209，本方案只暴露缺口和 authoring intent。
```

### 是否符合 AI-first

符合。

```text
selection、proxy、hit-test、Inspector fields、diagnostics、report 均结构化。
visual_order_key、reorder_supported、runtime_supported、last_reorder_status 可被 AI 读取和审查。
AI 可读取统一 Scene authoring context 定位 AUI bug。
AI 修改仍生成 ProjectPatch 或 UiCommandPayload。
```

### 是否支撑复杂打飞机

支撑。

```text
复杂打飞机 HUD 可以在 Scene View 中与游戏画面一起查看、点选和编辑。
复杂 UI 的 Canvas / LayerGroup 可以在 Hierarchy 中和场景对象按视觉顺序排列。
如果用户做跨 World 前后夹击，报告会诚实提示当前 runtime 尚未兑现。
用户不需要离开 Scene 主编辑界面。
```

### 主要风险

风险一：

```text
Scene View hit-test 与 AUI overlay hit-test 容易互相干扰。
```

处理：

```text
C-min-r1 先做明确命中顺序和结构化 hit-test report，后续再做 layer/filter/lock。
Gate D0 先补 Scene Entity 空间拾取，避免统一 hit-test 无地基。
```

风险二：

```text
误把 AuiNodeAuthoringProxy 当作 Scene Entity。
```

处理：

```text
文档和测试必须断言 proxy 不写入 Scene、RuntimePackage 或 ECS。
```

风险三：

```text
Inspector 编辑路径可能绕过已有 AUI service。
```

处理：

```text
所有修改必须映射到已有 AUI UiCommandPayload。
```

风险四：

```text
Hierarchy 退回 AUI / World 工程分桶，导致创作者无法直观看到或拖拽调整前后层级。
```

处理：

```text
默认 Hierarchy 必须使用 SceneVisualOrderAuthoringEntry。
工程分桶只允许作为高级过滤 / 调试视图，不作为默认 authoring 视图。
拖拽排序必须输出结构化 reorder report。
```

风险五：

```text
误以为 C-min-r1 已经支持任意 AUI Node 与 Scene Entity 的真实运行时混排。
```

处理：

```text
C-min-r1 只保证视觉顺序 authoring model、Canvas / LayerGroup 级排序和结构化报告。
真实运行时任意节点级混排必须在后续 RuntimeRenderer / RenderGraph composition gate 中验证。
```

风险六：

```text
VisualOrderKey 暴露超过 runtime 支持的 render_space，导致 authoring 谎报。
```

处理：

```text
C-min-r1 只暴露 ScreenOverlay / Modal 可执行枚举。
其它能力必须通过 runtime_supported=false 和 deferred_to_runtime_composition_gate 诊断表达。
```

风险七：

```text
拖拽单个 AUI Node 到 Scene Entity 前后时，编辑器隐式插入 LayerGroup，污染 AUI Document 真相。
```

处理：

```text
C-min-r1 明确拒绝该拖拽。
只输出结构化提示，让用户显式执行 Extract To AUI LayerGroup / Canvas。
```

## 15. 最终结论

正式采用：

```text
AUI Scene Unified Authoring Productization v1
方案 C-min-r1：Truthful Scene Unified AUI Authoring
```

下一步：

```text
基于本文生成可自动化施工文档并自审。
施工必须严格按 Gate A / B / C0 / C1 / D0 / D / E / F / G 执行，一边施工一边测试。
209 之后优先讨论 RuntimeRenderer Multi-stage UI Composition Pass。
再讨论 AUI Prefab / Template Reuse Productization v1。
```
