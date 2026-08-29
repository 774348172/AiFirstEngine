# 257 Native Editor Dark Theme + Workspace Docking v1 方案

## 0. 文档状态

```text
系统编号：257
方案版本：v1 方案 B
方案状态：施工完成并归档；D0-D7、真实窗口、受影响域与 exact-commit Local CI 全部通过
确认日期：2026-07-26
正式 Module：EditorWorkspaceDockingModule
视觉模型：EditorTheme（Module 内部消费的 Editor-only 主题值，不新增公共生命周期 Module）
前置系统：121 Native Editor Application Shell、248 Native Editor UI Interaction Reachability / Layout Convergence
```

本方案只定义 Native Editor 自身的工作区布局和视觉主题，不修改项目 AUI、Runtime Player、RuntimePackage 或导出游戏 UI。

## 1. 一句话目的

让 Native Editor 采用克制的黑灰色专业工具界面，并让 Hierarchy、Viewport、Inspector、Asset Browser、Console、AI Panel 等编辑器面板能够像 Unity 一样在主窗口内自由拉伸、组成 Tab、跨区域拖动、关闭恢复和持久化布局。

## 2. 用户问题

当前 Native Editor 已经可以显示并操作主要编辑面板，但工作区仍是固定布局。用户不能按实际任务调整面板宽高，也不能把常用面板移动到更顺手的位置。

当前视觉已经使用部分深色常量，但仍存在：

```text
颜色语义过粗；
局部面板直接写 rgba；
面板、字段、选中态和层级表面的明暗关系不统一；
Launcher 与 Workspace 的视觉密度不完全一致；
缺少一份稳定的 Editor-only 主题模型。
```

该问题不是给每个面板增加独立拖拽回调。若每个面板分别维护拖拽、命中、焦点、尺寸和持久化，会产生大量浅 Interface 和重复状态。

## 3. 当前实现基线

### 3.1 已完成能力

248-A/B/C 已完成并归档：

```text
Editor-only retained WidgetTree；
Taffy layout adapter；
single geometry；
clip / extract / pick；
focus / pointer capture 基础；
逻辑坐标与 Windows DPI 边界；
生产面板 retained cutover；
真实窗口、OS 输入和 actual RGBA 验收。
```

因此 257 不重新建设 WidgetTree、不引入第二套 UI 框架，也不恢复手写 HitRegion。

### 3.2 当前布局事实

当前代码同时保留两个不同历史来源的布局对象：

```text
editor_window_winit::DockLayoutManager
  -> 拥有 DockLayoutNode / PanelRegistry
  -> 当前仍以 fixed_default / compute_fixed_rects 为主
  -> 未驱动生产 renderer 的实际面板区域

editor_ui_renderer::EditorLayout::resolve
  -> 当前生产渲染使用
  -> 按固定比例计算 left / center / right / bottom
  -> 不接受用户布局状态
```

248 已保证实际绘制与命中共享 WidgetTree geometry，但没有把固定工作区升级成可变 Dock 生命周期。257 必须替换上述固定布局来源，不能在它们上面再叠第三套拖拽矩形。

### 3.3 当前主题事实

`editor_ui_renderer::theme` 已有：

```text
ROOT / MENU / TOOLBAR
PANEL / PANEL_DARK / PANEL_LIGHT
TAB / TAB_ACTIVE
FIELD / BORDER
TEXT / TEXT_MUTED
ACCENT / WARNING / ERROR
```

这是可复用基础，但不是完整主题合同。部分 Launcher、Project Browser、overlay 和状态表面仍直接使用 `UiColor::rgba(...)`。

## 4. 成熟编辑器对标

### 4.1 Unity

本地源码：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/GUI/SplitView.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/GUI/DockArea.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/HostView.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/GUI/PaneDragTab.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/GUI/WindowLayout.cs
```

关键链路：

```text
SplitView
  -> 维护分割树和子 View 尺寸

DockArea
  -> 承载一组 EditorWindow Tab
  -> DragOver 计算 drop zone
  -> PerformDrop 提交移动

HostView
  -> 承载当前激活 EditorWindow 内容

WindowLayout
  -> 保存、加载和恢复工作区布局
```

采用点：

```text
Split 与 Tab Stack 是工作区结构，不属于业务面板；
拖拽先形成 preview/drop info，再原子提交；
布局保存由工作区 owner 负责；
面板内容与停靠位置解耦。
```

不照搬点：

```text
不照搬 Unity 的 ScriptableObject/GUIView 历史对象模型；
不保留 IMGUI 与 retained UI 双轨；
第一版不实现跨原生窗口和多显示器布局；
不复制 Unity 完整 WindowLayout 版本兼容历史。
```

### 4.2 Godot

本地源码：

```text
<GODOT_SOURCE>/godot-master/godot-master/editor/docks/editor_dock_manager.h
<GODOT_SOURCE>/godot-master/godot-master/editor/docks/editor_dock_manager.cpp
<GODOT_SOURCE>/godot-master/godot-master/scene/gui/split_container.cpp
```

关键链路：

```text
EditorDockManager
  -> 管理 dock 注册、移动、关闭和菜单
DockSplitContainer
  -> 管理可拖拽 split
DockTabContainer
  -> 管理同区域 Tab
save_docks_to_config / load_docks_from_config
  -> 保存和恢复用户布局
```

采用点：

```text
Dock 管理集中；
业务 Dock 不实现布局算法；
关闭的 Dock 可从菜单恢复；
损坏或过期布局可以回到默认状态。
```

不照搬点：

```text
不复制 Godot 固定 slot 数量和节点对象体系；
不把项目资源路径写入 Editor workspace layout；
不让 Dock manager 成为 Editor 业务状态 owner。
```

### 4.3 对标结论

成熟编辑器的共同点不是某一种 UI 框架，而是：

```text
一个工作区布局 owner；
Split Tree + Tab Stack；
统一 drag/drop preview 与 commit；
统一焦点、命中和 pointer capture；
面板描述与面板内容分离；
版本化布局持久化和默认恢复。
```

## 5. 方案比较与正式选择

### 5.1 方案 A：主题 + 固定区域 Splitter

只调整颜色，并允许左右、底部固定区域拉伸。

优点：

```text
改动较小；
短期很快改善可用性。
```

缺点：

```text
不能移动面板；
不能组成或拆分 Tab；
固定区域假设会继续渗入 renderer；
后续完整 Dock 仍需再次替换。
```

不采用。

### 5.2 方案 B：目标级深 Workspace Docking Module

建立一个 `EditorWorkspaceDockingModule`，统一拥有工作区树、交互状态、布局约束和持久化。所有业务面板只注册描述和内容。

优点：

```text
满足主窗口内 Unity-like 工作区；
Interface 小，Implementation 集中；
不要求每个面板学习拖拽和布局；
可以直接复用 248 retained WidgetTree；
为未来浮动窗口保留结构扩展点。
```

正式采用。

### 5.3 方案 C：立即完成原生浮动窗口与多显示器

在 B 基础上同时实现 Tab 拖出主窗口、多个 winit window、多个 GPU surface、跨窗口拖拽和 Per-Monitor DPI。

能力最完整，但当前会扩大到 window/surface/present 生命周期，不适合“先真实使用编辑器制作小游戏”的阶段。

本轮不采用。其能力保持 deferred，必须由真实使用反馈证明需要后再单独讨论。

## 6. 范围

### 6.1 必须完成

```text
Native Editor 全局黑灰主题；
EditorTheme 语义 token；
所有当前生产 workspace panel 接入统一 Dock；
水平和垂直 Splitter 拖拽；
Tab 激活和同 Stack 排序；
Tab 跨 Stack 移动；
中心/左/右/上/下 drop preview；
拖放后创建、复用或清理 Split/Stack；
面板关闭、菜单恢复；
默认布局重置；
布局持久化、读取验证和安全回退；
最小尺寸与窄窗口约束；
真实 DPI 下正确 drag threshold、preview 和 resize；
稳定 PanelId / LayoutNodeId，供 AI、测试和诊断定位。
```

### 6.2 固定 Shell 表面

以下表面消费统一主题，但不作为普通 Dock Panel：

```text
主菜单；
全局工具栏；
状态栏；
窗口级 overlay；
Launcher 顶层导航。
```

### 6.3 非目标

```text
原生浮动窗口；
跨显示器拖拽；
多个 winit window / WGPU surface；
自定义用户主题编辑器；
在线主题市场；
任意 CSS/stylesheet 语言；
动画 Dock；
项目 AUI Theme；
Runtime HUD；
插件二进制 Widget ABI；
AI 专用布局菜谱；
按游戏项目保存业务数据。
```

## 7. 深 Module 与 Interface

正式 Module：

```text
EditorWorkspaceDockingModule
```

概念上的外部 Interface 收敛为：

```text
register_panels(panel descriptors)
restore_or_default(layout source, available panels)
update(workspace intent, viewport facts) -> WorkspaceUpdate
snapshot(viewport facts) -> WorkspaceSnapshot
persist_if_dirty(layout sink)
```

这里的 `workspace intent` 是受限的 Editor UI-local action，不是 AI 或用户必须遵守的工具外流程：

```text
ResizeSplit
ActivateTab
ReorderTab
BeginPanelDrag
UpdatePanelDrag
CommitPanelDrop
CancelPanelDrag
ClosePanel
ShowPanel
ResetLayout
```

具体 Rust Interface 可在施工文档中根据依赖方向进一步收敛，但不得把每个内部步骤公开成跨 crate 生命周期协议。

### 7.1 删除测试

若删除该 Module，下列复杂性会重新散落到 shell、renderer 和所有业务 panel：

```text
split ratio clamp；
drop zone hit testing；
tab move/reorder；
empty stack cleanup；
minimum size propagation；
drag preview；
focus/capture cleanup；
layout schema migration；
persist/restore/fallback。
```

因此它是深 Module，不是对 WidgetTree 的透传包装。

## 8. 状态归属

| 状态 | 唯一 owner |
|---|---|
| 业务显示值、selection、document、diagnostics | `EditorUiModel` / `EditorSession` |
| Panel 注册信息 | `EditorWorkspaceDockingModule` 内的 registry |
| Split/Stack/Tab 布局 | `EditorWorkspaceDockingModule` |
| active tab / closed panel | `EditorWorkspaceDockingModule` |
| split resize / panel drag 瞬时状态 | `EditorWorkspaceDockingModule` |
| Widget geometry / clip / pick path | 248 `EditorWidgetTree` |
| OS pointer / keyboard / DPI facts | `editor_window_winit` |
| 项目 authoring mutation | `UiCommand` / `EditorSession` |
| Editor 主题 token | `EditorTheme` |
| 最终 GPU DrawList | `editor_ui_renderer` / `editor_wgpu_renderer` |

禁止事项：

```text
Panel 不保存自己的 dock rect；
Renderer 不维护第二份 split ratio；
Window host 不复制 layout tree；
持久化文件不是运行时第二真相；
WidgetTree 不取得 Editor 业务状态；
Dock action 不绕过 UiCommand 修改项目。
```

## 9. 工作区数据模型

### 9.1 PanelDescriptor

```text
PanelDescriptor {
  panel_id
  title
  icon_id
  minimum_size
  preferred_size
  closable
  default_placement
}
```

规则：

```text
panel_id 稳定且全局唯一；
title/icon 可以变化，不参与持久化 identity；
minimum_size 是约束，不是固定尺寸；
default_placement 只用于首次布局和 reset；
业务 panel 不知道当前 placement。
```

### 9.2 Layout Tree

```text
WorkspaceRoot {
  body: DockNode
}

DockNode =
  Split {
    node_id
    axis
    ratio
    first
    second
  }
  | Stack {
    node_id
    tabs
    active_panel_id
  }
```

Toolbar/StatusBar 等 Shell chrome 不进入该树。

### 9.3 WorkspaceSnapshot

Module 向 retained composer 提供只读 snapshot：

```text
WorkspaceSnapshot {
  layout_revision
  root
  resolved_node_rects
  resolved_panel_rects
  active_tabs
  splitters
  drag_preview
  diagnostics
}
```

Snapshot 是投影，不是可变 owner。Renderer 和 Widget composer 不能回写其内部字段。

## 10. 布局不变量

每次 restore、resize、drop、close 和 viewport resize 后都必须满足：

```text
每个可见 panel 只在一个 Stack 中出现一次；
每个 Stack 至少包含一个 panel；
active_panel_id 必须属于该 Stack；
Split 必须有两个有效子节点；
ratio 为有限数并处于合法范围；
父子关系无环、无孤儿；
所有 panel_id 必须在 registry 中存在；
同一 panel 不得同时处于 closed 和 visible；
resolved rect 不出现负宽高；
minimum size 约束尽最大可能成立；
无法满足全部 preferred size 时优先保证 center workspace 可达；
布局无效时 fail closed 到默认布局，不带病运行。
```

空 Stack 在一次 drop/close transaction 内立即清理；相邻同轴 Split 可按实现需要规范化，但规范化不得改变用户可见顺序。

## 11. Splitter Resize

Splitter 是 WidgetTree 中真实、稳定、可命中的 Editor widget：

```text
pointer down
  -> acquire pointer capture
pointer move
  -> Module 计算受 minimum size 约束的新 ratio
  -> 发布新 layout revision
pointer up
  -> commit ratio
  -> 标记 persistence dirty
Escape / capture lost
  -> cancel 或按最后合法 ratio 收敛
```

规则：

```text
命中区域可以比视觉分隔线稍宽；
视觉分隔线保持 1px 或 DPI 等价细线；
拖动不触发项目 mutation；
拖动过程中不重建 Editor 业务 Model；
resize 只 invalidates 受影响 layout subtree；
窗口缩放后 ratio 保持语义，像素 rect 重新解析。
```

## 12. Tab Drag / Dock

### 12.1 状态机

```text
Idle
  -> Armed(panel_id, source_stack, pointer_origin)
  -> Dragging(panel_id, source_stack, preview_target)
  -> Commit
  -> Idle

Armed / Dragging
  -> Cancel
  -> Idle
```

拖拽阈值使用逻辑像素并经过 DPI 换算。单击 Tab 只激活，不误触拖动。

### 12.2 Drop Target

Stack 的候选目标：

```text
Center
  -> 加入目标 Stack，成为 Tab

Left / Right / Top / Bottom
  -> 在目标位置创建 Split + 新 Stack
```

Drop preview 必须来自与 commit 相同的 target resolution，不允许视觉预览与最终落点使用两套算法。

### 12.3 原子提交

一次 drop transaction 必须完成：

```text
验证 source/target 仍存在；
从 source 移除 panel；
创建或选择 target Stack；
插入 panel 并激活；
清理空 Stack 和退化 Split；
恢复或转移 focus；
发布一个 layout revision；
标记 persistence dirty。
```

任何验证失败都保持原布局并返回 typed diagnostic，不允许出现面板丢失或重复。

## 13. Close / Show / Reset

### 13.1 Close

Closable panel 关闭后从 layout tree 移除并进入 `closed_panels`。不可关闭的核心 panel 不显示关闭命令。

### 13.2 Show

Window/Panel 菜单列出已注册 panel。恢复时优先：

```text
仍存在的 last valid stack；
descriptor.default_placement；
默认 center-compatible fallback。
```

### 13.3 Reset

Reset Layout 使用版本内置默认布局替换当前 workspace。它只修改 Editor 用户布局，不修改项目、selection 或已打开文档。

## 14. 布局持久化

### 14.1 归属

布局是 Editor 用户偏好，不是游戏项目资产：

```text
不写入项目 root；
不进入 Asset DB；
不进入 RuntimePackage；
不影响项目 digest；
不要求 AI mutation Grant。
```

### 14.2 Schema

```text
EditorWorkspaceLayout.v1 {
  schema_version
  layout_id
  root
  closed_panels
  last_known_panels
}
```

不持久化：

```text
像素 rect；
pointer position；
drag state；
focus capture；
Widget address/index；
业务 Model；
项目绝对路径。
```

### 14.3 Restore

读取顺序：

```text
parse
  -> schema version check
  -> structural validation
  -> reconcile against current PanelRegistry
  -> clamp ratios / repair safe omissions
  -> use

unrecoverable
  -> preserve diagnostic
  -> load built-in default
```

未知旧 panel 可以丢弃；新 panel 按 default placement 补入。禁止因布局文件损坏让 Editor 无法启动。

写入必须使用宿主现有安全持久化原语或等价原子替换，避免半写文件成为下一次启动真相。

## 15. EditorTheme

### 15.1 定位

`EditorTheme` 是 Editor renderer 消费的不可变视觉值，不是第二个公共生命周期 Module。第一版只提供一个内置主题：

```text
DarkNeutral
```

不提供用户主题编辑、热切换或项目级覆盖。

### 15.2 语义 Token

至少覆盖：

```text
surface.root
surface.chrome
surface.toolbar
surface.panel
surface.panel_raised
surface.panel_recessed
surface.viewport
surface.field
surface.popup

border.subtle
border.normal
border.focused

text.primary
text.secondary
text.disabled

selection.active
selection.inactive
accent.primary

status.warning
status.error
status.success

overlay.scrim
overlay.drop_preview
```

颜色使用语义，而不是按某个面板命名。`hierarchy_background`、`console_tab_blue` 等 panel-specific token 不进入公共主题。

### 15.3 视觉方向

参考用户确认的 Godot 黑灰视觉：

```text
近黑 application chrome；
深灰主 panel；
稍亮 active tab / selected row；
低对比细边框；
浅灰主文字与中灰辅助文字；
蓝色只用于 active/focus/selection/drop preview；
warning/error 保留必要辨识度；
紧凑 toolbar；
少圆角、无渐变、无装饰性光晕。
```

主题收敛必须覆盖 Launcher、Workspace、Menu、Toolbar、Panel、Field、Tab、Popup、Modal 和状态反馈，不能只换主工作区背景。

## 16. 与 Retained WidgetTree 集成

正式链路：

```text
EditorUiModel
  + EditorWorkspaceDockingModule snapshot
  + EditorTheme
  -> EditorWidgetTree compose/reconcile
  -> Taffy + Dock resolved constraints
  -> single geometry / clip / pick
  -> UiDrawList
  -> WGPU
```

Dock Module 决定工作区结构和区域约束；WidgetTree 仍是最终 Widget geometry、clip 和 pick 真相。

禁止链路：

```text
Dock Module -> 手写最终 DrawList
Dock Module -> 手写 HitRegion
Renderer -> 反向修改 Dock tree
Panel -> 直接修改 split ratio
editor_ui_renderer -> editor_window_winit 反向依赖
```

施工时必须根据当前 crate 依赖选择 seam。允许 Module 的内部实现分散到现有 crate 的私有模块，但外部 Interface 和状态 owner 只有一个；不得仅为名称新建无复用证据的 crate。

## 17. 焦点、命中和 DPI

### 17.1 焦点

```text
激活 Tab 后，focus 恢复到该 panel 最近合法 focus target；
关闭 active panel 后，切换到同 Stack 的相邻 Tab；
移动 focused panel 后，focus 随 panel 转移；
被删除 Widget 的 focus/capture 必须清理；
drag preview 不取得业务输入焦点。
```

### 17.2 命中

Splitter、Tab、close button 和 drop zone 都必须是 WidgetTree 派生的真实交互对象。Z-order、clip 和 modal scope 继续遵守 248 合同。

### 17.3 DPI

必须区分：

```text
logical workspace coordinates；
physical OS pointer coordinates；
physical framebuffer pixels。
```

Splitter 厚度、Tab 高度、drag threshold、preview rect 和最小尺寸均以逻辑尺寸定义，由既有 DPI adapter 转换。

## 18. 面板覆盖范围

第一版覆盖当前生产 workspace 中全部可见业务面板：

```text
Hierarchy；
Game / Scene Viewport shell；
Inspector；
Asset / Project Browser；
Console；
Runtime Trace；
Authoring Workflow；
Input Mapping；
Build & Export；
AI Panel；
Project Intent；
Report。
```

具体 PanelRegistry 以施工时生产 panel manifest 为准。不得只迁移 3 个示例 panel 后保留其它 panel 固定坐标。

Launcher、Menu、Toolbar、StatusBar 和 Modal 消费主题但按第 6.2 节保持 Shell 归属。

## 19. AI 适配

该 Module 不限制 AI 的游戏制作能力，也不要求 AI 操作固定布局流程。

AI 可以通过稳定结构观察：

```text
panel_id；
visible / closed；
stack_id；
active；
logical_rect；
minimum_size；
layout_revision；
drag/resize availability；
diagnostics。
```

AI 是否移动面板、以什么顺序使用工具、是否恢复默认布局，均由目标和上下文决定。方案不提供 `nextAction`、固定步骤或工作区菜谱。

Editor layout action 只影响用户工作环境；AI 修改项目仍使用现有 typed tools、UiCommand、ProjectPatch 和 mutation guard，不能用 Dock action 绕过项目审计。

## 20. Diagnostics 与报告

普通 Editor 默认只保留轻量结果，不每帧序列化完整布局。

```text
Off
  -> 正常交互和必要错误提示

Summary
  -> schema fallback、missing panel、invalid ratio、drop rejection、minimum-size pressure

Trace
  -> layout tree、resolved rect、drag state transition、drop target、revision lineage
```

建议结构化诊断：

```text
invalid_layout_schema
layout_cycle
unknown_panel
duplicate_panel
empty_stack
invalid_active_tab
invalid_split_ratio
minimum_size_unsatisfied
drag_source_missing
drop_target_stale
drop_rejected
layout_persist_failed
layout_restore_fallback
```

报告只用于测试、诊断或用户显式请求，不形成常驻重型 telemetry。

## 21. 验证边界

本方案只规定结果，不建立固定测试菜谱或新的长期 Runner。

### 21.1 定向合同

至少证明：

```text
layout tree invariant；
split ratio clamp；
minimum size propagation；
tab activate/reorder；
center/edge drop；
atomic drop failure；
empty stack cleanup；
close/show/reset；
schema restore/reconcile/fallback；
theme token completeness；
无 panel 内 dock rect owner；
无生产固定 EditorLayout 第二真相。
```

### 21.2 交互验证

至少覆盖：

```text
真实 pointer splitter drag；
Tab click 与 drag threshold 区分；
Tab 跨 Stack drop；
drop preview 与 commit 同源；
Escape/capture lost 取消；
窗口 resize 后布局稳定；
focus 转移；
modal/input 不穿透。
```

### 21.3 尺寸与 DPI

复用 248 的代表性尺寸与 DPI 分层证据，不机械复制全部旧 Gate：

```text
确定性 logical layout / DPI adapter contract；
受影响真实窗口 smoke；
至少一个 actual RGBA 截图证明黑灰主题和 Dock preview；
窄窗口下无 incoherent overlap，关键面板仍可恢复。
```

真实 150%/200% Windows 环境不可用时必须如实记录环境边界，不用 metadata 冒充真实证据。

### 21.4 受影响域回归

覆盖 WidgetTree compose/reconcile、renderer、window input route、panel manifest 和 layout persistence 的真实 consumer。项目 Runtime、AUI 和 Export 未修改时不重复运行其全量产品验收。

## 22. 后续施工切片建议

本节只给未来施工文档提供边界，不构成施工授权：

```text
D0  baseline / ownership red contracts
D1  EditorTheme semantic token convergence
D2  Workspace tree + invariant + restore/default
D3  production fixed layout cutover + Splitter resize
D4  Tab Stack + drag preview + atomic dock
D5  close/show/reset + persistence
D6  production panel coverage + focus/DPI/real-window acceptance
D7  affected-domain regression + document/archive
```

施工文档应继续按失败捕获能力、风险和成本拆分，而不是把上述编号机械变成必须重复运行的测试流程。任何单次施工不得超过当前公共规则的三小时上限；预计超出时必须继续切小。

## 23. 验收标准

只有以下全部成立，257 才能标记完成：

1. Native Editor 的 Launcher、Shell 和 Workspace 统一消费 `DarkNeutral` 语义主题。
2. 生产 panel 不再散落可由主题表达的装饰性 `rgba`。
3. `EditorWorkspaceDockingModule` 是唯一可变工作区布局 owner。
4. 当前生产固定 `EditorLayout::resolve` 退出 workspace 布局真相。
5. 旧 `DockLayoutManager` 被迁移、替换或删除，不再形成 dormant duplicate contract。
6. 所有生产 workspace panel 通过稳定 PanelId 注册。
7. 用户可自由水平/垂直拉伸主要区域。
8. 用户可激活、排序和跨区域拖动 Tab。
9. center/edge drop preview 与最终 commit 一致。
10. close/show/reset 和布局持久化可用。
11. 损坏或过期布局安全回退到默认布局。
12. panel、split、stack 不出现重复、丢失、孤儿或无效 active tab。
13. 最小尺寸、窄窗口和 DPI 约束不产生失控重叠。
14. focus、pointer capture、modal 和业务输入路由不回退。
15. Dock action 不修改项目、不进入项目 digest、不要求 mutation Grant。
16. Runtime Player、项目 AUI、RuntimePackage 和 Export 行为不受影响。
17. 定向合同、受影响域回归和必要真实窗口证据通过。
18. 原生浮动窗口、多显示器和跨窗口拖拽仍保持 deferred。

## 24. 风险与控制

### 24.1 再次形成双布局真相

风险：保留固定 renderer layout，同时新增 Dock snapshot。

控制：施工必须以替换方式退出固定 workspace resolver；测试断言生产 panel rect 来自唯一 snapshot。

### 24.2 Module Interface 膨胀成菜谱

风险：把 drag lifecycle 的每个内部步骤变成跨 crate 公共协议。

控制：外部只提交 UI-local intent 并读取 snapshot/update result；hit testing、preview、tree rewrite 和 cleanup 保持 Implementation 私有。

### 24.3 每个 panel 获得布局逻辑

风险：面板自行保存 rect、drop target 或 persistence key。

控制：Panel 只提供 descriptor/content；布局完全由 Module 负责。

### 24.4 布局损坏阻止 Editor 启动

风险：持久化 schema 过期或文件半写。

控制：版本化、原子写、结构验证、registry reconcile 和 built-in default fallback。

### 24.5 拖拽与业务输入冲突

风险：Tab drag、Viewport camera drag、Asset drag 共享 pointer 状态时互相穿透。

控制：复用统一 pointer capture 和 WidgetPath；drag owner 显式互斥；capture lost 必须收敛。

### 24.6 视觉换肤扩大为通用 Theme 平台

风险：为了一个内置黑灰主题增加用户 stylesheet、插件 skin 和 Runtime Theme。

控制：第一版只有 Editor-only `DarkNeutral` 不可变值；没有主题编辑器和项目覆盖。

### 24.7 一次施工范围过大

风险：主题、布局、拖拽、持久化和真实窗口验证同时修改导致难以定位。

控制：未来施工文档按第 22 节拆分，定向修复不使用全量真实窗口矩阵作为调试循环。

## 25. 方案自审

### 25.1 是否满足用户确认的方案 B

通过。方案包含统一黑灰主题、主窗口内自由 Splitter resize、Tab 拖动、Dock preview、布局保存和恢复；原生浮动窗口明确 deferred。

### 25.2 是否与 248 冲突

不冲突。248 的 retained WidgetTree、single geometry、clip、pick、focus 和 DPI 合同继续有效。257 只为其提供可变 workspace structure 和主题输入，不引入第二套 WidgetTree 或手写 HitRegion。

### 25.3 是否真正消除重复布局 owner

通过。方案明确要求当前固定 `EditorLayout::resolve` 退出生产 workspace 真相，并迁移、替换或删除未驱动 renderer 的旧 `DockLayoutManager`；最终只保留一个 `EditorWorkspaceDockingModule` owner。

### 25.4 是否过度限制 AI

不限制。AI 可观察稳定 PanelId、布局和诊断，也可以自主决定是否调整布局。方案没有规定制作游戏的工具顺序、下一步或固定工作流。

### 25.5 是否形成工具外菜谱

没有。Splitter 和 Tab 的步骤属于 Module 内部交互实现；调用者只提交目标级 UI-local intent。项目制作流程仍由用户和 AI 自主决定。

### 25.6 是否过度设计

已控制。第一版不做原生浮动窗口、多显示器、主题编辑器、插件 Widget ABI、动画和通用 stylesheet。复杂性集中在一个确有多个消费者的深 Module 内。

### 25.7 是否错误增加第二个公共 Module

没有。`EditorTheme` 是不可变视觉模型，不拥有独立 lifecycle、session 或 command protocol；正式公共 Module 只有 `EditorWorkspaceDockingModule`。

### 25.8 是否覆盖真实用户使用

通过。验收要求所有生产 workspace panel 覆盖、真实 splitter/tab drag、持久化恢复、窄窗口、DPI 和 actual RGBA 证据，不以三个示例 panel 或纯 headless geometry 冒充完成。

### 25.9 是否扩大到 Runtime/AUI

没有。主题和 Dock 全部为 Editor-only；不修改 Runtime Player、项目 AUI、RuntimePackage、Build 或 Export。

### 25.10 自审结论

```text
方案规格：通过
用户目标：通过
深 Module：通过
单一状态 owner：通过
AI 自由度：通过
248 兼容性：通过
范围控制：通过
施工可拆分性：通过
```

## 26. 最终结论

257 正式采用方案 B：

```text
EditorUiModel
  + EditorWorkspaceDockingModule
  + EditorTheme::DarkNeutral
  -> retained EditorWidgetTree
  -> single geometry / clip / pick
  -> UiDrawList
  -> WGPU
```

它提供 Unity-like 的主窗口内自由工作区，但不把 Dock 复杂性推给业务 panel，也不把内部生命周期
变成 AI 菜谱。对应施工文档已经完成、自审并归档；D0-D7、真实窗口与最终 Local CI 证据见
阶段完成记录。
