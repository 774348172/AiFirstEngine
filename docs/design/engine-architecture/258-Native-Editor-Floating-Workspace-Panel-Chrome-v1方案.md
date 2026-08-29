# 258 Native Editor Floating Workspace + Panel Chrome v1 方案

## 0. 文档状态

```text
系统编号：258
方案版本：v1 方案 B
方案状态：用户已确认；正式方案已冻结；W1 已完成，等待 W2 单独授权
确认日期：2026-07-27
正式 Module：EditorWorkspaceDockingModule（在现有小 Interface 后深化）
内部 Adapter：NativeWorkspaceWindowHost（不形成公共生命周期 Interface）
```

258 是独立新系统，不是 `257-R2`，也不重新打开已经完成的 257。

## 1. 一句话目的

让 Native Editor 具备 Unity 风格的细 Splitter、统一 Panel 右上角 chrome，以及可以在主窗口、
浮动原生窗口之间自由拖出、组合和重新停靠的真实工作区。

## 2. 与 257 的隔离合同

257 永久保持以下终态：

```text
257 方案：完成并归档
257 施工文档：完成并归档
257 Local CI / 完成记录：只作历史证据
257-R1 production binary / cursor remediation：完成并归档
```

258 只继承当前生产代码已经存在的事实：

```text
retained EditorWidgetTree；
EditorWorkspaceDockingModule；
主窗口内 Split / Stack / Tab / drop transaction；
EditorTheme::DarkNeutral；
winit + WGPU 真实窗口与 DPI Adapter。
```

禁止形成以下耦合：

```text
不得修改 257 的方案状态或完成结论；
不得要求重跑 257 D0-D7；
不得复用 257 的施工授权、role root、Local CI run 或 binary seal；
不得把 258 命名为 257 remediation、257-R2 或 257 continuation；
258 的失败不得回写成 257 失败；
258 必须拥有独立施工文档、测试身份、提交和完成记录。
```

这是一种代码基线复用，不是施工生命周期耦合。

## 3. 用户问题

### 3.1 Splitter 视觉边线过宽

当前 `split_rect` 使用约 `7` 个逻辑像素的 Splitter 命中区域，renderer 又把整个命中矩形画成
边线，导致视觉上出现宽灰条。

正确合同是：

```text
视觉 hairline：约 1 个物理像素；
可交互 hit region：约 7-10 个逻辑像素；
hit region 透明；
visual rect 与 hit rect 共享同一中心线；
hover / active drag 使用水平或垂直 resize cursor。
```

### 3.2 Tab 不能拖出主窗口

当前工作区只有一个 root，真实窗口 host 只有一个 winit Window 和一个 WGPU surface。
主窗口内部 Dock 已实现，但原生浮动窗口、跨窗口 drop resolution 和每窗口 DPI 生命周期尚未实现。

### 3.3 Panel header 缺少 Unity 风格 chrome

当前 Stack header 只绘制 Tab，没有右侧固定控制区、lock 状态或 anchored popup。

每个可见 Stack header 需要保留两个固定按钮位置：

```text
Lock；
More（竖向三点）。
```

More 菜单 v1 只包含 `Close Tab`。

## 4. 成熟编辑器对标

### 4.1 Unity

官方文档：

```text
https://docs.unity3d.com/Manual/CustomizingYourWorkspace.html
https://docs.unity3d.com/ScriptReference/EditorWindow.html
```

本地源码：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/GUI/SplitView.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/GUI/DockArea.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/HostView.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/GUI/PaneDragTab.cs
```

关键做法：

```text
SplitView 用宽 hit rect 处理 resize，不要求视觉边线同宽；
DockArea 管理 Tab、drop zone 和 Close Tab；
HostView 在 active pane 右上角安排 generic menu 和可选 lock button；
PaneDragTab 使用临时原生拖拽窗口表达被拖出的 Tab；
EditorWindow 可以处于 docked 或 floating 状态；
一次 drop 原子改变 Split / DockArea / active pane。
```

采用点：

```text
命中和视觉分离；
右侧 chrome 属于 Panel host，不属于业务内容；
拖拽代理与最终浮动窗口分离；
浮动窗口可以包含多个 Tab；
drop preview 与 commit 使用同一 target resolution。
```

不照搬点：

```text
不复制 Unity 的 ScriptableObject / GUIView / IMGUI 历史对象模型；
不让每个 panel 实现自己的窗口生命周期；
不公开面向 AI 的 create-window / attach-surface 步骤菜谱；
不复制 Unity generic menu 的全部历史命令。
```

### 4.2 Godot

参考：

```text
<GODOT_SOURCE>/godot-master/godot-master/editor/docks/editor_dock_manager.h
<GODOT_SOURCE>/godot-master/godot-master/editor/docks/editor_dock_manager.cpp
<GODOT_SOURCE>/godot-master/godot-master/scene/gui/split_container.cpp
```

采用集中 Dock owner、业务 panel 与 placement 解耦、布局保存与无效状态安全恢复。不照搬固定
Dock slot、Scene Node 所有权或 Godot 专用窗口体系。

## 5. 方案比较

### 5.1 方案 A：视觉修补 + 主窗口内伪浮动层

优点是改动小；缺点是没有真实 OS Window、不能自然跨窗口和跨显示器，也会形成第二套 overlay
布局。不能满足用户目标，不采用。

### 5.2 方案 B：深化现有 Workspace Docking Module

`EditorWorkspaceDockingModule` 继续拥有全部逻辑工作区状态；winit 内部 Adapter 只负责把窗口计划
落实为真实 OS Window / WGPU surface。

优点：

```text
一个逻辑工作区 owner；
不增加公共生命周期菜谱；
业务 panel 不感知窗口、surface 或拖拽算法；
主窗口与浮动窗口共享同一套 Split / Stack / Tab 不变量；
headless owner 测试与真实 OS composition 可以分层验证。
```

正式采用。

### 5.3 方案 C：建立多个公共 Window / Chrome Module

把窗口、Dock、Panel chrome 分成多个公共 Module。虽然表面职责清晰，但会公开 create、attach、
activate、detach、destroy 等生命周期顺序，Interface 接近 Implementation，增加调用方和测试负担。
不采用。

## 6. 深 Module 与内部 Adapter

唯一公共状态 owner 继续是：

```text
EditorWorkspaceDockingModule
```

概念 Interface 保持小：

```text
register_panels(panel descriptors)
restore_or_default(layout source, available panels, display facts)
update(workspace intent, window facts) -> WorkspaceUpdate
snapshot(workspace window id, viewport facts) -> WorkspaceWindowSnapshot
window_plan() -> WorkspaceWindowPlan
persist_if_dirty(layout sink)
```

`NativeWorkspaceWindowHost` 是 `editor_window_winit` 内部 Adapter：

```text
reconcile(WorkspaceWindowPlan)
  -> 创建、更新或销毁真实 winit Window；
  -> 为每个窗口维护 WGPU surface / renderer；
  -> 把 WindowId、screen rect、DPI、focus 和 close facts送回 application。
```

它不形成公共 Module，不被 AI 调用，也不公开 create-window 菜谱。调用者只提交一个目标级
`WorkspaceWindowPlan`。

删除测试：

```text
若删除 EditorWorkspaceDockingModule，多窗口 topology、drop target、Tab transaction、空窗口清理、
Panel chrome、lock、Close Tab、布局恢复和持久化会重新散落到 application、renderer 和每个 panel。
```

因此深化后的 Module 仍具有足够 Depth。

## 7. 工作区拓扑

```text
WorkspaceTopology {
  main_window: WorkspaceWindowRoot
  floating_windows: Map<WorkspaceWindowId, WorkspaceWindowRoot>
}

WorkspaceWindowRoot {
  workspace_window_id
  root: DockNode
  placement
  display_hint
}

DockNode =
  Split { node_id, axis, ratio, first, second }
  | Stack { node_id, tabs, active_panel_id }
```

规则：

```text
WorkspaceWindowId 是稳定布局身份；
winit WindowId、surface 和 native handle 只存在于 Adapter 运行态；
一个 panel 在整个 WorkspaceTopology 中最多出现一次；
浮动窗口至少包含一个有效 Stack；
最后一个 Tab 被关闭或重新停靠后，空浮动窗口自动销毁；
主窗口不能因最后一个业务 Tab 被移走而失去固定 Shell；
业务 panel 不知道自己位于主窗口还是浮动窗口。
```

## 8. Unity 风格 Splitter

`WorkspaceSplitter` 输出两类 geometry：

```text
hit_rect
visual_rect
```

合同：

```text
hit_rect 使用逻辑像素，默认宽度约 7；
visual_rect 使用 EditorTheme hairline，并对齐物理像素；
WidgetTree 的 Splitter widget 拥有 hit_rect；
视觉 child / paint 只使用 visual_rect；
透明 hit rect 不产生宽色带；
Pointer capture、minimum size clamp、Escape cancel 和 cursor 语义保持一致。
```

不得用减小 hit rect 的方式让边线看起来更细，否则会降低可用性。

## 9. Panel Chrome

### 9.1 布局

每个可见 Stack header 由以下区域组成：

```text
Tab viewport | optional tab scroller | Lock | More
```

右侧控制区尺寸稳定，Tab 很多时只能裁剪或滚动 Tab viewport，不能覆盖按钮。按钮使用 Editor
图标资源和 tooltip，不在业务 panel 中手写 SVG 或字符图形。

### 9.2 Lock

Lock 表示“固定当前 panel 的上下文”，不是“禁止移动窗口”。

```text
PanelDescriptor.context_lock = supported | unsupported
PanelPresentationState.context_locked = true | false
```

规则：

```text
每个 Stack header 都保留 Lock 按钮位置；
active panel 支持 context lock 时按钮可用；
不具备上下文跟随语义的 panel 显示 disabled lock 和解释 tooltip；
Inspector v1 必须支持锁定当前 authoring/runtime selection；
锁定不复制项目数据，不阻止项目 mutation，不冻结整个 EditorUiModel；
切换 Tab 后显示新 active panel 自己的 lock 状态；
关闭 panel 会清理 session-only lock state。
```

不得让通用 Dock Module缓存任意业务 PanelModel。具体 panel 只负责解释自己的 context anchor；
Dock Module 只拥有 lock capability 与 presentation state。

### 9.3 More 与 Close Tab

More 打开锚定在按钮下方的 retained popup。v1 菜单只有：

```text
Close Tab
```

规则：

```text
closable panel：命令可用；
不可关闭 panel：显示 disabled Close Tab；
点击外部、Escape、窗口失焦或 Tab 被移动时关闭 popup；
popup 使用 WidgetTree geometry / pick / modal scope；
Close Tab 复用 Module 现有 ClosePanel transaction；
关闭只影响 Editor view，不修改项目，也不丢失项目数据。
```

## 10. Tab 拖出与重新停靠

### 10.1 状态机

```text
Idle
  -> Armed
  -> DraggingDocked
  -> DraggingNativeProxy
  -> CommitDocked | CommitFloating | Cancel
  -> Idle
```

### 10.2 拖拽代理

Tab 离开所有 workspace window 的有效 Dock target 后，内部 Adapter 创建轻量原生 drag proxy，
显示 Tab 标题并跟随 screen-space pointer。

该 proxy：

```text
不是正式 WorkspaceWindowRoot；
不进入布局持久化；
不拥有业务 PanelModel；
cancel 或 commit 后必须销毁；
只为跨原生窗口的连续拖拽和视觉反馈服务。
```

### 10.3 Drop resolution

Module 使用所有活动 workspace window 的 screen rect、DPI 和 Stack snapshot 解析唯一 drop target：

```text
Stack Center -> 加入目标 Stack；
Stack Edge -> 创建 Split + Stack；
无目标 -> 创建 floating WorkspaceWindowRoot；
浮动窗口目标 -> 与主窗口使用相同规则；
无效或过期 target -> 原布局不变并返回 typed diagnostic。
```

preview 和 commit 必须消费同一个 resolved target token，禁止两套算法。

### 10.4 原子提交

一次 commit 完成：

```text
验证 source panel 与 source window；
验证 resolved target token；
从 source topology 移除 panel；
插入目标 Stack 或创建 floating root；
清理退化 Split、空 Stack 和空 floating root；
转移 active tab / focus；
发布一个 layout revision；
生成一个 WorkspaceWindowPlan；
标记 persistence dirty。
```

任一步失败都保持旧 topology，不允许 panel 丢失、重复或成为无 owner 状态。

## 11. 原生窗口生命周期

每个正式 workspace window 独立拥有：

```text
winit Window；
WGPU surface / renderer；
logical viewport；
physical framebuffer；
scale factor；
redraw / focus / cursor state。
```

所有窗口共享：

```text
一个 NativeEditorApplication；
一个 EditorSession；
一个 EditorUiModel 业务真相；
一个 EditorWorkspaceDockingModule；
一个 Gateway / project context。
```

窗口关闭规则：

```text
关闭主窗口 -> 走现有 Editor shutdown；
关闭浮动窗口 -> closable tabs 进入 closed_panels；
浮动窗口中的不可关闭 panel -> 原子 rehome 到主窗口 fallback Stack；
关闭过程中不得丢失项目 mutation、未保存项目状态或 gateway lifecycle。
```

## 12. DPI、多显示器与坐标

真实浮动窗口必须处理：

```text
window-local logical coordinates；
screen-space physical coordinates；
per-window scale factor；
ScaleFactorChanged；
显示器断开后的 placement fallback。
```

布局持久化保存稳定 logical placement 和 display hint，不保存 winit WindowId、surface handle 或
裸平台坐标身份。恢复时如果目标显示器不存在或窗口完全不可见，clamp 到主显示器可见工作区。

Windows production composition 是 v1 必须能力。macOS/Linux 原生窗口资格和平台专用 chrome
不是本方案 v1 的完成条件。

## 13. 布局持久化

新增：

```text
EditorWorkspaceLayout.v2 {
  schema_version
  main_root
  floating_roots
  closed_panels
  panel_presentation
}
```

迁移：

```text
v1.root -> v2.main_root；
v1 不存在 floating_roots 时使用空集合；
未知或损坏 floating root 单独丢弃并产生 diagnostic；
不能因一个浮动窗口记录损坏而阻止 Editor 启动；
写入继续使用原子替换。
```

Panel context lock 默认是 session-only；除非某个 panel 的明确产品语义要求，否则不跨 Editor
重启持久化具体 context anchor。

## 14. 状态归属

| 状态 | 唯一 owner |
|---|---|
| 多窗口 WorkspaceTopology | `EditorWorkspaceDockingModule` |
| Split / Stack / Tab / active / closed | `EditorWorkspaceDockingModule` |
| Panel chrome presentation state | `EditorWorkspaceDockingModule` |
| 业务 selection / document / diagnostics | `EditorSession` / `EditorUiModel` |
| Inspector pinned context anchor | Inspector/application 现有业务 owner |
| Widget geometry / clip / pick | 每窗口 retained `EditorWidgetTree` |
| OS Window / surface / DPI facts | 内部 `NativeWorkspaceWindowHost` Adapter |
| drag proxy native handle | 内部 Adapter，瞬时 |
| Editor 用户布局文件 | `EditorWorkspaceLayout.v2` 持久化 Adapter |

禁止：

```text
业务 panel 创建或销毁 OS Window；
renderer 修改 WorkspaceTopology；
window host 复制 Dock tree；
Dock Module 缓存任意业务 PanelModel；
浮动窗口创建第二个 EditorSession、Gateway 或 project context；
项目文件保存 Editor 窗口布局。
```

## 15. AI 适配与工具外自由

258 不增加 AI-facing 工具步骤或固定流程。AI 和测试可以观察：

```text
workspace_window_id；
main / floating；
panel_id / stack_id / active；
logical rect / screen rect / scale factor；
context_lock_supported / context_locked；
close availability；
drag/drop availability；
layout_revision；
typed diagnostics。
```

AI 可以按任务自由选择是否移动、锁定、关闭或恢复 panel。引擎只提供目标级 intent 和原子
transaction，不要求 AI 执行 create-window、attach-surface、move-tab、cleanup-window 菜谱。

## 16. 失败与恢复

必须 fail closed：

```text
OS Window 创建失败 -> panel 保持 source Stack；
surface 创建失败 -> 新窗口不进入 topology；
drop target stale -> 保持原布局；
drag proxy 失败 -> 继续显示 retained preview 或 cancel，不丢 panel；
浮动窗口异常关闭 -> 根据 topology rehome / close contract 收敛；
布局文件损坏 -> 保留 diagnostic，恢复安全默认布局；
焦点或 pointer capture 丢失 -> cancel 当前 drag；
显示器断开 -> 把不可见窗口恢复到可见工作区。
```

## 17. 验证原则

施工文档必须按风险拆分，不把所有能力塞入一次高成本 Gate。

最低验证面：

```text
Splitter hit_rect / visual_rect owner red test；
Panel chrome geometry、Lock capability、popup 与 Close Tab owner test；
WorkspaceTopology 多窗口不变量与原子 transaction test；
v1 -> v2 layout migration / corrupt fallback test；
每窗口 WidgetTree geometry / pick / DPI test；
真实 Windows create / drag proxy / floating / re-dock / close process smoke；
受影响 crate 定向回归；
最终 production composition 与用户可见验收。
```

真实 OS window smoke 只证明真实 composition，不替代 deterministic owner tests。不得机械重复
同配置上层已覆盖的测试，也不得因单个 UI 问题自动要求 release-wide exact-commit Local CI。

## 18. 明确非目标

```text
修改项目 AUI 或 Runtime HUD；
插件二进制 Window ABI；
任意脚本自定义原生 window chrome；
复制 Unity 全部 generic menu；
把 Lock 解释为冻结整个项目或禁止 layout mutation；
每个 panel 单独实现拖拽；
独立 EditorSession / Gateway per floating window；
macOS/Linux production qualification；
在线布局同步；
动画 Dock；
重新施工或重跑 257。
```

## 19. 完成定义

258 完成必须同时满足：

```text
Splitter 视觉 hairline 与宽命中区分离；
主窗口和浮动原生窗口之间可双向拖拽 Tab；
浮动窗口可组成多个 Tab；
每个 Stack header 有稳定 Lock / More chrome；
More 菜单包含正确可用性状态的 Close Tab；
Inspector context lock 真实生效；
多窗口共享同一 EditorSession / project context；
布局 v2 可保存、恢复、迁移和安全回退；
真实 Windows production Editor 验收通过；
独立 258 完成记录与归档形成；
257 保持原完成态且其证据未被复用或重跑。
```

## 20. 下一步

`258` 施工文档已生成、自审并完成激活前复核，当前施工切片为：

```text
S1 Splitter visual/hit separation；
S2 Panel chrome / Lock / Close Tab；
S3 WorkspaceTopology v2 与持久化迁移；
S4 NativeWorkspaceWindowHost 多窗口 composition；
S5 drag proxy / floating / re-dock；
S6 真实 Windows 验收与归档。
```

这些是施工切片，不是对 AI 或用户公开的操作菜谱。W1 / S1 已完成并提交为 `9cc9e5b`；
当前下一步是用户单独授权 W2 / S2，未授权前不得继续施工。
