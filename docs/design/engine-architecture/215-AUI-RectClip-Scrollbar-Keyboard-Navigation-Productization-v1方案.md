# 215-AUI RectClip / Scrollbar / Keyboard Navigation Productization v1 方案

## 1. 这个系统是干什么的

本系统补齐 214 之后复杂 AUI 滚动控件还缺的三件事：

```text
RectClip：ScrollView / List 的内容必须真的被矩形裁剪，不能只是 layout 位置移动。
Scrollbar：滚动区域必须能显示基础滚动条，并支持拖拽 thumb 改变 scroll offset。
Keyboard Navigation：键盘方向键 / Tab 移动焦点时，焦点项必须可见，必要时自动滚入视野。
```

它在其它引擎里的大致对标：

```text
Unity UGUI：RectMask2D / MaskableGraphic / ClipperRegistry + ScrollRect + Scrollbar + Selectable Navigation。
Unreal Slate / UMG：Slate clipping + SScrollBox + focus/navigation。
Godot：Control clipping + ScrollContainer + follow_focus。
```

它在本引擎主线中的作用：

```text
AUI Document 仍是 UI 结构真相。
AUI Runtime Core 负责 clip / scrollbar / focus navigation 机制。
Renderer 只消费已解析的 AuiOverlayFrame / AuiCompositionFrame，不读 AUI Document。
Project Rule / IR 不处理 clip、scrollbar、focus navigation。
```

## 2. 当前为什么需要它

214 已完成：

```text
Modal input blocking。
Focus trap。
ScrollView / List wheel scroll。
ScrollView / List drag scroll。
layout_with_scroll_offsets 把 scroll offset 应用到 computed rect。
```

但 214 明确没有做：

```text
renderer clip pass。
Scrollbar visual。
完整 keyboard / gamepad navigation。
Mask / Clip / Scrollbar rendering。
```

所以当前状态是：

```text
滚动内容位置会动，但超出 viewport 的内容还没有成为 renderer 可执行裁剪。
用户看不到滚动条，也不能拖动滚动条。
键盘焦点可以在 modal 内循环，但还不是完整菜单/列表导航。
焦点移动到不可见列表项时，ScrollView 不会自动滚动到该项。
```

复杂打飞机项目中的装备列表、设置面板、暂停菜单、关卡列表都需要这个能力。

## 3. 外部源码参考

### 3.1 Unity UGUI

本项目已有 UGUI 源码参考：

```text
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UI/Core/RectMask2D.cs
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UI/Core/MaskableGraphic.cs
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UI/Core/ClipperRegistry.cs
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UI/Core/ScrollRect.cs
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UI/Core/Scrollbar.cs
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UI/Core/Selectable.cs
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UI/Core/Navigation.cs
框架设计/Unity源码参考/UGUI-Canvas-EventSystem-Render源码参考.md
```

关键实现点：

```text
RectMask2D.PerformClipping：
  计算多个 RectMask2D 的 compound clip rect。
  对 IClippable / MaskableGraphic 调 SetClipRect。
  对 MaskableGraphic 调 Cull。

ScrollRect.OnBeginDrag / OnDrag / OnEndDrag：
  记录 pointer start / content start。
  根据 pointer delta 更新 content anchored position。
  LateUpdate 同步 inertia / elastic / scrollbar。

Scrollbar：
  作为 Selectable，可拖拽 thumb。
  会根据 axis 调整 navigation 行为。

Selectable.FindSelectable：
  根据方向、候选 rect 位置、interactable 和 navigation mode 找下一焦点。
  支持 Automatic / Horizontal / Vertical / Explicit / None。
```

可学习点：

```text
Clip 是 UI runtime / renderer 基础能力，不应由项目规则或 IR 计算。
ScrollView 必须同时处理 offset、clip、hit-test、scrollbar 和 focus visibility。
Keyboard navigation 需要结构化 Navigation 数据，而不是写死 Tab 顺序。
```

不可照搬点：

```text
不照搬 Unity GameObject / Component / CanvasRenderer 结构。
不照搬 stencil Mask 作为第一版。
不照搬完整 ScrollRect inertia / elasticity / nested scroll。
不把 Scrollbar 做成独立运行时 Entity。
```

### 3.2 Unreal / Godot

参考结论：

```text
UE Slate 把 clipping 当成低层 draw element / geometry 约束，SScrollBox 负责滚动容器行为。
Godot ScrollContainer 默认提供滚动容器和 scrollbar，并支持焦点进入不可见区域时跟随滚动。
```

对本项目的吸收方式：

```text
只吸收“clip / scroll / focus visibility 是同一条 UI framework 链路”的结论。
不引入 Slate Widget 层，不引入 Godot Control 节点继承体系。
```

## 4. 本项目已有基线

相关已完成系统：

```text
199-AUI-ProjectUiStateSnapshot-Producer-v1方案.md
209-AUI-Scene-Unified-Authoring-Productization-v1方案.md
210-RuntimeRenderer-Multi-stage-UI-Composition-Pass-Productization-v1方案.md
211-AUI-Prefab-Template-Reuse-Productization-v1方案.md
213-AUI-Runtime-Interaction-Input-Consumption-Action-Dispatch-Productization-v1方案.md
214-AUI-Complex-Controls-Modal-Focus-Scroll-Productization-v1方案.md
```

当前代码基线：

```text
rust/crates/engine_runtime/src/aui.rs
  AuiNodeKind 已有 List / ScrollView。
  AuiInteractionState 已有 focus / active_modal_root / scroll_offsets。
  AuiScrollState 已有 offset_y / max_offset_y。
  AuiLayoutEngine::layout_with_scroll_offsets 已能应用 scroll offset。
  AuiLayoutReport 已有 clipped_node_count / scroll_offset_applied。
  AuiDrawCommand 目前只有 DrawRect / DrawImage / DrawText。
  AuiOverlayDrawItem 目前没有 per-item effective_clip_rect。

rust/crates/runtime_player_winit/src/lib.rs
  已输出 modal / focus / scroll / consumed input evidence。

rust/crates/project_e2e_gate/src/aui_complex_controls.rs
  已覆盖 modal blocking / focus trap / wheel scroll / drag scroll / Scene AUI hit-test。
```

## 5. 方案选择

### 方案 A：RectClip-only C-min

内容：

```text
只做 effective_clip_rect。
ScrollView / List 子节点超过 viewport 后被裁剪。
hit-test 尊重 clip。
```

优点：

```text
范围最小。
能立刻修掉“滚动内容露出 viewport”的核心问题。
```

缺点：

```text
没有 scrollbar，用户仍不直观看到列表可滚。
键盘焦点仍不能自动滚入视野。
复杂菜单体验不完整。
```

### 方案 B：RectClip + Scrollbar + Keyboard Navigation B-min

内容：

```text
RectClip stack / effective_clip_rect。
ScrollView / List 子节点裁剪。
hit-test 尊重 clip。
基础 vertical scrollbar 显示。
scrollbar thumb drag 改变 scroll offset。
Tab / Shift+Tab / ArrowUp / ArrowDown 导航焦点。
焦点项不可见时自动滚入 ScrollView viewport。
结构化 productization report 和 complex shooter e2e report。
```

优点：

```text
正好补齐 214 留下的真实缺口。
不新增架构层。
仍保持 AUI Document 为结构真相。
对复杂打飞机菜单/装备/关卡列表足够有用。
AI 可通过 report 判断 clip、scrollbar、navigation 是否真实工作。
```

缺点：

```text
比方案 A 多改 interaction / layout / draw extraction / report。
仍不是完整商业级 ScrollRect / CommonUI。
```

### 方案 C：完整复杂 UI 容器系统

内容：

```text
stencil mask。
nested scroll。
inertia / elastic。
virtualized list。
完整 gamepad navigation。
InputField / IME。
rich text。
theme / scrollbar template。
```

优点：

```text
能力完整。
更接近 Unity / UE / Godot 成熟 UI。
```

缺点：

```text
范围过大。
会把 215 变成大而散的控件库项目。
测试面过宽，不利于 AI 稳定施工。
```

## 6. 采用方案

采用：

```text
方案 B：RectClip + Scrollbar + Keyboard Navigation B-min。
```

本方案的核心原则：

```text
只补 214 后复杂滚动 UI 的可用闭环。
不新增运行时 Widget 层。
不让 AUI Node 变成 Scene Entity。
不让 IR 处理 UI framework 机制。
不做完整 InputField / IME / virtualized list。
```

## 7. 设计细化

### 7.1 RectClip

新增或扩展：

```text
AuiClipPolicy：
  None
  Rect

AuiNode：
  clip_policy: AuiClipPolicy，serde default=None。

AuiComputedNode：
  effective_clip_rect: Option<AuiComputedRect>。
  clipped_by_node: Option<String>。

AuiOverlayDrawItem：
  effective_clip_rect: Option<AuiComputedRect>。
```

规则：

```text
ScrollView / List 默认在 runtime 视为 Rect clip root。
显式 clip_policy=Rect 的 Panel 也可作为 clip root。
子节点 effective_clip_rect 是父 clip 与当前 clip root rect 的交集。
完全落在 clip rect 外的 draw item 可以被 cull，并进入 report。
部分相交的 draw item 保留，带 effective_clip_rect 进入 overlay。
hit-test 必须跳过 clip rect 外的区域。
```

第一版只做矩形裁剪：

```text
不做 stencil mask。
不做圆角 mask。
不做任意 polygon clip。
不做 shader-level soft mask。
```

### 7.2 Scrollbar

新增或扩展：

```text
AuiScrollbarPolicy：
  None
  Auto
  Always

AuiScrollbarAxis：
  Vertical

AuiScrollbarMetrics：
  scroll_node_id
  track_rect
  thumb_rect
  offset_y
  max_offset_y
  viewport_height
  content_height
  visible
```

规则：

```text
B-min 只做 vertical scrollbar。
ScrollView / List 默认 scrollbar_policy=Auto。
content_height <= viewport_height 时 scrollbar visible=false。
scrollbar track / thumb 是 runtime framework chrome，不是 Scene Entity。
draw extraction 可以生成 synthetic DrawRect / overlay item，但必须带 source scroll_node_id。
thumb drag 优先于 content drag hit-test。
拖拽 thumb 更新 AuiScrollState.offset_y。
```

本轮不做：

```text
horizontal scrollbar。
scrollbar template / skin。
鼠标 hover fade。
滚动条惯性。
nested scrollbar arbitration。
```

### 7.3 Keyboard Navigation

新增或扩展：

```text
AuiNavigationMode：
  None
  Auto
  Vertical
  Horizontal
  Explicit

AuiNavigationRef：
  mode
  up
  down
  left
  right
  next
  previous
```

规则：

```text
interactable=true 的 visible node 可成为 focusable。
Modal active 时 navigation scope 仍限制在 focus_scope_root subtree 内。
Tab / Shift+Tab 默认按 tree order 移动 next / previous。
ArrowUp / ArrowDown 在 Vertical / Auto 中按 computed rect 选择候选。
ArrowLeft / ArrowRight 在 Horizontal / Auto 中按 computed rect 选择候选。
Explicit 模式优先使用 up/down/left/right/next/previous。
Navigation 不直接触发业务 action，只改变 focus，Submit/Cancel 后续系统再做。
```

B-min 支持：

```text
Tab / Shift+Tab。
ArrowUp / ArrowDown。
ArrowLeft / ArrowRight 可有 schema 和 report，但测试只要求基础左右候选。
```

不做：

```text
完整 gamepad navigation。
Submit action。
screen flow。
input rebinding UI。
```

### 7.4 Focus Visible Auto-scroll

规则：

```text
当 focused_node 位于 ScrollView / List subtree 内，且 computed rect 超出 effective_clip_rect：
  如果节点在 viewport 上方，把 offset_y 减小到节点可见。
  如果节点在 viewport 下方，把 offset_y 增大到节点可见。
  offset_y 必须 clamp 到 0..max_offset_y。
```

状态归属：

```text
focus / scroll offset 仍在 AuiInteractionState。
AUI Document 不保存运行时 focus 和 scroll offset。
```

报告：

```text
focus_visible_scroll_count。
focused_node_before。
focused_node_after。
scroll_node_adjusted。
scroll_offset_before / after。
```

## 8. Runtime 链路

目标链路：

```text
AUI Document
  -> Binding Resolve
  -> AuiLayoutEngine::layout_with_scroll_offsets
  -> effective_clip_rect / scrollbar metrics
  -> AuiDrawList / AuiOverlayFrame with clip
  -> AuiInteractionSystem hit-test with clip
  -> filter_consumed_events
  -> RuntimeRenderer UI Pass
  -> productization report
```

Renderer 规则：

```text
Renderer 只能接收 effective_clip_rect，不计算 AUI hierarchy。
Headless renderer 可以先把 clip 写入 RHI command / report，不要求真实 GPU scissor 截图。
真实 GPU scissor / stencil 是后续 renderer 施工，不阻塞本方案 B-min 的 headless gate。
```

## 9. AI-first 报告

新增报告：

```text
aui-rectclip-scrollbar-navigation-productization-report.v1
complex-shooter-aui-rectclip-scrollbar-navigation-productization-report.v1
```

核心字段：

```text
schema_version
status
clip_root_count
effective_clip_item_count
culled_draw_item_count
hit_test_clip_rejected_count
scrollbar_visible_count
scrollbar_thumb_drag_count
scrollbar_offset_change_count
keyboard_navigation_event_count
focus_move_count
focus_visible_scroll_count
focused_node_before
focused_node_after
deferred_flags
diagnostics
next_actions
```

必须保留 deferred flags：

```text
stencil_mask_deferred=true
nested_scroll_deferred=true
inertia_elastic_deferred=true
virtualized_list_deferred=true
input_field_ime_deferred=true
full_gamepad_navigation_deferred=true
```

## 10. 施工边界

本轮做：

```text
RectClip / effective_clip_rect。
ScrollView / List clip root。
hit-test clip rejection。
vertical scrollbar visual + thumb drag。
keyboard navigation B-min。
focus visible auto-scroll。
runtime_player_winit summary evidence。
project_e2e_gate complex shooter report。
```

本轮不做：

```text
Stencil Mask。
Soft Mask。
Rounded Clip。
Horizontal scrollbar。
Nested scroll arbitration。
Inertia / elastic。
Virtualized list。
InputField / IME。
Rich text。
完整 gamepad navigation。
Submit action / screen flow。
Scrollbar template / theme editor。
真实 GPU screenshot pixel clip gate。
```

## 11. 拟施工 Gate

Gate A：schema / report / clip 基础

```text
新增 AuiClipPolicy / AuiScrollbarPolicy / AuiNavigationMode / AuiNavigationRef。
扩展 AuiNode / AuiComputedNode / AuiOverlayDrawItem。
新增 productization report 字段。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_clip
```

Gate B：layout effective clip / draw cull

```text
layout 计算 effective_clip_rect。
draw extraction 携带 clip rect。
完全不可见 item 被 cull 并计入 report。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_clip
cargo test -p engine_runtime aui
```

Gate C：hit-test clip rejection

```text
hit-test 跳过 clip 外内容。
pointer 在 viewport 内但命中被裁剪子节点时不能产生 action。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_hit_test
```

Gate D：vertical scrollbar visual + thumb drag

```text
生成 scrollbar metrics。
生成 track / thumb overlay item。
thumb drag 更新 scroll offset。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_scrollbar
```

Gate E：keyboard navigation + focus visible auto-scroll

```text
Tab / Shift+Tab / ArrowUp / ArrowDown 改变 focused_node。
焦点移动到不可见 item 时自动调整 scroll offset。
Modal scope 内继续保持 focus trap。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_navigation
```

Gate F：runtime_player_winit / project_e2e_gate

```text
NativeAuiPresentSummary 输出 clip / scrollbar / navigation evidence。
project_e2e_gate 新增 complex shooter rectclip-scrollbar-navigation report。
```

测试：

```powershell
cd rust
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate aui_rectclip_scrollbar_navigation
cargo test -p project_e2e_gate
```

## 12. 验收标准

必须满足：

```text
ScrollView/List 子节点超出 viewport 后不再被 hit-test 命中。
Overlay draw item 携带 effective_clip_rect 或被 cull。
report 能证明 clip root、culled item、hit-test clip rejection。
Scrollbar thumb visible 且能通过 drag 改变 offset_y。
Keyboard navigation 能移动焦点。
焦点移动到不可见列表项时 scroll offset 自动调整。
Modal focus trap 与 keyboard navigation 不冲突。
```

不能用以下方式冒充完成：

```text
只统计 clipped_node_count，但 renderer / overlay item 没有 clip 证据。
只画 scrollbar 矩形，但拖拽不改变 scroll offset。
只改变 focused_node，但不可见项不自动滚入视野。
hit-test 仍能命中 clip 外内容。
```

## 13. 自审结论

```text
本方案没有新增架构层。
本方案没有把 AUI Node 改成 Runtime ECS Entity。
本方案没有把复杂 UI framework 机制交给 IR。
本方案只扩展现有 Rust AUI Runtime Core。
本方案是 214 的直接后续，不重复做 Modal / basic scroll。
```

风险：

```text
engine_runtime/src/aui.rs 已经较大，施工时如果新增代码过多，应优先拆到 aui_* 子模块，但不改变 public API 心智。
真实 GPU scissor 可能需要 RuntimeRenderer / RHI 后续单独强化；B-min 先以 headless overlay/report 证明。
Keyboard navigation 自动候选算法第一版不要追求 Unity 级完整，先保证 deterministic。
```

## 14. 结论

```text
采用方案 B-min：RectClip + Scrollbar + Keyboard Navigation。
下一步如要施工，应根据本方案生成当前自动化施工文档，并先自审施工范围。
```
