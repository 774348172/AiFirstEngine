# 173-Native Editor Real Window Interaction Smoke / Screenshot Gate v1 方案

## 1. 系统是什么

Native Editor Real Window Interaction Smoke / Screenshot Gate v1 是编辑器真实窗口交互验收系统。

它不是替代 Native Editor Real Interaction Validation Gate v1，也不是重新做一套 UI 测试系统。

它负责验证：

```text
NativeEditorInteractionScenario
  -> NativeEditorApplication
  -> winit real window boundary
  -> WGPU surface present
  -> controlled window event bridge
  -> HitRegion / UiCommand / EditorSession
  -> DrawList / present report / screenshot evidence
  -> AI-readable report
```

通俗说，它回答的问题是：

```text
编辑器窗口是否真的创建出来了。
WGPU surface 是否真的 present 过。
按钮是否能通过真实窗口层的输入路径触发。
触发后 UI model / command / feedback 是否真的变化。
验收报告是否能告诉 AI 和开发者哪里断了。
```

## 2. 系统边界

本系统属于编辑器真实窗口验收层。

它只做：

```text
真实窗口 smoke。
真实 surface present 证据。
真实编辑器输入路径的受控注入。
复用已有 NativeEditorInteractionScenario。
输出 screenshot evidence metadata / report。
```

它不做：

```text
不做完整 OS 自动化驱动。
不做 Windows 全局鼠标移动。
不做 golden image pixel diff。
不做复杂拖拽、多窗口、弹窗、IME 全量验收。
不替代 headless deterministic interaction gate。
```

长期路线中，OS-level click / screenshot / golden diff 会作为外层验收接入，但不能替换本系统的稳定 scenario/report。

## 3. 其它引擎怎么运作

### 3.1 Unreal Engine

UE 编辑器真实交互以 Slate 为核心：

```text
OS / platform window
  -> FSlateApplication
  -> Widget path / hit testing
  -> SWidget::OnMouseButtonDown / OnKeyDown
  -> Editor command / transaction
  -> Slate renderer
  -> automation screenshot / screenshot comparison
```

对应源码参考：

```text
Engine/Source/Runtime/SlateCore/Public/Application/SlateApplicationBase.h
Engine/Source/Runtime/SlateCore/Public/Widgets/SWidget.h
Engine/Source/Runtime/SlateCore/Private/Widgets/SWidget.cpp
Engine/Source/Developer/ScreenShotComparisonTools
```

UE 的重点不是只测某个业务函数，而是让事件进入 Slate 应用层，再通过 widget / command / renderer / screenshot 形成闭环。

### 3.2 Unity

Unity 编辑器真实窗口以 ContainerWindow / GUIView / EditorWindow 为核心：

```text
Native editor container window
  -> GUIView / HostView
  -> IMGUI / UI Toolkit event dispatch
  -> EditorWindow / serialized object / command
  -> repaint
  -> screenshot / profiler capture / test runner
```

对应源码参考：

```text
Editor/Mono/ContainerWindow.cs
Editor/Mono/WindowBackendManager.cs
Editor/Mono/EditorGUI.cs
Editor/Mono/SceneView/SceneView.cs
Runtime/Export/PlayerLoop/PlayerLoop.bindings.cs
```

Unity 的真实编辑器验收会围绕 EditorWindow、GUIView 事件、repaint、play/edit mode 变化做测试。

### 3.3 Bevy

Bevy 更接近 Rust 原生路线：

```text
WinitPlugin
  -> window events
  -> app schedule
  -> render graph
  -> screenshot plugin / GPU readback
```

对应源码参考：

```text
crates/bevy_app/src/app.rs
crates/bevy_app/src/schedule_runner.rs
crates/bevy_render/src/view/window/screenshot.rs
```

Bevy 清晰区分 headless runner 和 windowed runner，这一点适合我们保留 headless interaction gate 与 real-window smoke gate 两层。

### 3.4 Godot

Godot 以 DisplayServer / Window / Viewport / Control 为核心：

```text
DisplayServer platform event
  -> Window callback
  -> Viewport input
  -> Control hit / focus
  -> rendering server / viewport capture
```

对应源码参考：

```text
scene/main/window.h
scene/main/window.cpp
platform/*/display_server_*.cpp
```

Godot 的经验是把平台窗口和引擎 UI 控件分层，平台只产生事件，业务由 Viewport / Control 系统处理。

## 4. 方案对比

### 方案 A：继续只用 headless interaction gate

优点：

```text
最快。
最稳定。
CI 友好。
```

缺点：

```text
不能证明真实窗口能 present。
不能证明真实窗口坐标/DPI/surface 路径没问题。
不能回答“按钮在打开的编辑器里为什么点不了”。
```

结论：不够。

### 方案 B：真实窗口 + 受控 window event bridge

优点：

```text
真实创建窗口。
真实创建 WGPU surface。
真实 present。
输入不直接绕过 UI 架构，仍进入 EditorInputEvent / HitRegion / UiCommand。
可自动退出，可测试，可稳定回归。
```

缺点：

```text
不是 OS 全局鼠标点击。
不能覆盖窗口遮挡、系统焦点、真实鼠标移动等问题。
```

结论：适合作为 v1 主体。

### 方案 C：完整 OS-level 自动化

优点：

```text
最接近真人点击。
能覆盖真实系统焦点、窗口遮挡、DPI、OS 事件注入问题。
```

缺点：

```text
第一版不稳定。
对测试机器、权限、窗口焦点、系统缩放、杀毒/安全策略敏感。
会拖慢当前编辑器基础闭环。
```

结论：长期需要，但不适合作为第一层默认 gate。

## 5. 推荐方案：B-C-min

确认采用 B-C-min：

```text
第一版采用真实窗口 + 真实 WGPU present + 受控 window event bridge。
复用 NativeEditorInteractionScenario / Report。
生成 RealWindowInteractionSmokeReport。
截图第一版记录 evidence metadata；如果 real-wgpu screenshot readback 已可用，则接入截图 artifact。
OS-level click / golden image diff 作为长期 C 路线保留。
```

## 6. 第一版数据结构

```text
RealWindowInteractionSmokeScenario
  scenario: NativeEditorInteractionScenario
  max_frames: u32
  width: u32
  height: u32
  require_present: bool
  require_screenshot_evidence: bool

RealWindowInteractionSmokeReport
  schema_version
  status
  backend
  window_created
  surface_created
  surface_configured
  present_status
  frame_count
  draw_command_count
  hit_region_count
  interaction_report
  screenshot
  diagnostics

RealWindowScreenshotEvidence
  kind
  width
  height
  frame_index
  artifact_path
  rgba_hash
```

## 7. 执行流程

```text
create NativeEditorApplication
create real-window smoke host
frame once to build DrawList
if real-window feature:
  create winit window / WGPU renderer in bounded smoke runner
else:
  produce feature_not_enabled report
for each NativeEditorInteractionStep:
  find HitRegion from latest DrawList
  synthesize controlled window event bridge input
  app.handle_input_event(...)
  frame/present
collect interaction report
collect present evidence
collect screenshot evidence metadata
exit automatically
```

## 8. 成功标准

第一版通过标准：

```text
report 可序列化。
feature 未启用时返回明确 skipped / feature_not_enabled，不假装成功。
headless-compatible smoke runner 能验证 scenario -> present evidence -> report。
真实窗口 feature 编译通过。
默认测试不依赖 OS window。
ignored/local-only 测试可以在本机手动跑真实窗口。
```

## 9. 自审

### 9.1 规格符合

符合用户要求：不是只停留在讨论，生成方案后可施工；系统讨论前已解释系统是什么，并对比 UE / Unity / Bevy / Godot。

### 9.2 规则符合

遵守长期主义规则：真实窗口层不绕过编辑器架构，不建立第二套 UI 测试系统。

### 9.3 文本一致

方案中明确 B-C-min：v1 是真实窗口与受控事件桥，长期保留 OS-level automation。

### 9.4 设计符合

符合 AI 友好：报告结构化，scenario 可读，可定位 command / hit / present / screenshot 断点。

符合复杂项目：复杂场景可以继续扩展 scenario，而不是增加零散测试函数。

### 9.5 实现可行

当前已有：

```text
NativeEditorInteractionScenario / Runner
NativeEditorApplication
RealWgpuUiRenderer
RealNativeEditorWindowReport
HeadlessSurfaceBackend
HeadlessNativeEditorWindowApp
```

第一版可通过连接层实现。

### 9.6 合理性

不直接上 OS-level 自动化是合理的。否则会把一个编辑器架构问题变成不稳定的系统自动化问题。

