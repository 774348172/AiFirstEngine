# 177-Editor Visual Regression / Golden Image Gate v1 方案

## 1. 系统是什么

`Editor Visual Regression / Golden Image Gate v1` 是编辑器 UI 的视觉回归验收门。

它回答的问题是：

```text
编辑器这次改完以后，整体界面是否还和我们确认过的基线一致？
Project Launcher、Authoring Workspace、Hierarchy、Inspector、Workflow、Console、AI Panel 等关键界面是否被画丢、错位、裁切或结构性改变？
如果改变了，这个改变是明确更新 baseline 的结果，还是无意回归？
```

它不是新的 UI 框架，不是新的交互系统，也不是项目玩法测试。它只负责给编辑器 UI 提供可重复、可审查、可归档的视觉证据。

## 2. 为什么现在需要

当前已经具备：

```text
Native Editor Real Interaction Validation Gate v1
Native Editor Real Window Interaction Smoke / Screenshot Gate v1
Exported Windows Real Window / Screenshot Verification Gate v1
Complex Shooter Authoring Workflow v1
```

这些系统能证明：

```text
窗口能创建
输入能路由
按钮能触发命令
真实窗口 smoke 能产出截图 metadata
编辑器 authoring workflow 能进入结构化状态
```

但它们还不能稳定证明：

```text
UI 视觉没有退化
按钮仍在正确位置
文本仍然被绘制
关键面板仍然可见
布局变化是否符合预期
```

所以 177 的职责是补齐“视觉基线比较”这一层。

## 3. 其它引擎参考

### 3.1 Unreal Engine

参考路径：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Developer/ScreenShotComparisonTools
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Developer/AutomationController
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Source/Runtime/Engine/Private/GameViewportClient.cpp
```

UE 的思路是：

```text
Automation Test
  -> 截图
  -> Screenshot Comparison
  -> 生成差异报告和 artifact
```

关键启发：

```text
截图比较是测试/自动化层，不污染正常运行时。
baseline 更新是显式动作，不应该静默覆盖。
报告必须包含差异原因和 artifact，而不只是 true/false。
```

### 3.2 Unity

参考路径：

```text
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Editor/Mono/SceneView/SceneView.cs
<UNITY_CS_REFERENCE>/UnityCsReference-master/UnityCsReference-master/Modules/DeviceSimulatorEditor/SimulatorWindow.cs
```

Unity 编辑器侧通常通过：

```text
EditorWindow / GUIView / UI Toolkit repaint
  -> 测试 Runner 或工具捕获窗口
  -> ImageAssert / Golden image 比较
```

关键启发：

```text
编辑器窗口本身仍然走正常 repaint。
视觉比较属于外部验证层。
真实窗口截图有价值，但容易受平台、字体、GPU、DPI 影响。
```

### 3.3 Godot

参考路径：

```text
<GODOT_SOURCE>/godot-master/godot-master/tests/test_main.cpp
<GODOT_SOURCE>/godot-master/godot-master/tests/scene/test_viewport.cpp
```

Godot 的测试更强调：

```text
Mock DisplayServer / headless 测试
Viewport 输出验证
场景结构与渲染结果分层验证
```

关键启发：

```text
第一版不应强依赖真实 OS window。
headless 结构化渲染证据更稳定。
真实截图可以作为 local-only / artifact 层逐步增强。
```

## 4. 方案对比

### 4.1 方案 A：只保留真实窗口截图 smoke

做法：

```text
继续使用 173 的真实窗口 screenshot metadata。
不做 baseline 比较。
```

优点：

```text
成本最低。
不增加新结构。
```

缺点：

```text
只能证明“有截图”，不能证明“截图对”。
无法发现 UI 被画丢、错位、文字缺失等回归。
不满足 golden image gate 的目标。
```

结论：不采用。

### 4.2 方案 B：真实 GPU 截图 + 像素级 diff 作为默认 gate

做法：

```text
默认创建真实窗口。
读取 GPU framebuffer。
和 PNG baseline 做像素 diff。
```

优点：

```text
最接近用户真实看到的画面。
长期可以作为本地验收或 release gate。
```

缺点：

```text
不同 GPU、DPI、字体、抗锯齿会导致不稳定。
会拖慢默认测试。
容易把测试系统变成正常编辑器运行时的负担。
第一版会把注意力拉进平台截图细节。
```

结论：不作为第一版默认 gate。后续可作为 local-only evidence layer。

### 4.3 方案 C-min：结构化 DrawList/GPU Plan baseline + 可选渲染 artifact

做法：

```text
NativeEditorApplication
  -> EditorUiModel
  -> UiDrawList
  -> UiGpuDrawPlan
  -> VisualEvidence
  -> GoldenBaseline compare
  -> VisualRegressionReport
```

第一版比较三层证据：

```text
DrawList 结构摘要
UiGpuDrawPlan 稳定摘要
稳定 fingerprint/hash
```

渲染 PNG / 真实窗口截图暂时作为后续扩展，不作为默认 CI blocker。

优点：

```text
稳定、快、可 headless。
不影响正常编辑器运行时。
能捕获 UI 结构、文本 glyph、矩形、hit region、viewport slot 的关键变化。
AI 和报告系统可读。
baseline 更新必须显式执行。
```

缺点：

```text
第一版不是严格真实像素 diff。
对字体抗锯齿、GPU shader、最终 framebuffer 问题覆盖不完整。
```

结论：采用方案 C-min。

## 5. 正式规则

### 5.1 分层规则

```text
editor_window_winit
  负责 visual regression scenario / runner / report / baseline compare。

editor_ui_renderer
  继续只负责 UiDrawList。

editor_wgpu_renderer
  继续负责 UiDrawList -> UiGpuDrawPlan / RHI draw plan / present report。

NativeEditorApplication
  只提供 frame 后的 model / draw_list，不知道 golden baseline。
```

禁止：

```text
禁止把 golden baseline 逻辑塞进 SelfUiRenderer。
禁止让正常编辑器运行时自动读写 baseline。
禁止测试失败时自动覆盖 baseline。
禁止把项目 gameplay 规则写进 visual regression scenario。
```

### 5.2 Evidence 规则

第一版 evidence 必须包含：

```text
scenario_id
surface_width / surface_height
mode
model_revision
draw_command_count
hit_region_count
rect_count
text_command_count
rendered_glyph_count
viewport_slot_count
font_backend
font_loaded
structural_hash
```

可选扩展字段：

```text
artifact_path
png_hash
real_window_screenshot_hash
```

第一版可以不写 PNG 文件，但 report 必须为后续 artifact 留边界。

### 5.3 Baseline 规则

baseline 是明确的结构：

```text
EditorVisualRegressionBaseline
  scenario_id
  surface_width
  surface_height
  structural_hash
  draw_command_count
  hit_region_count
  rect_count
  text_command_count
  rendered_glyph_count
  viewport_slot_count
```

规则：

```text
baseline 缺失时，状态为 BaselineMissing。
baseline 不匹配时，状态为 Failed，并输出 diagnostics。
baseline 匹配时，状态为 Passed。
baseline 更新必须由显式命令或显式测试 fixture 完成。
```

### 5.4 Scenario 规则

第一版推荐场景：

```text
ProjectLauncher default page
AuthoringWorkspace default layout
Hierarchy + Inspector selected entity
Workflow / Build / Console / AI visible state
```

第一版实现时可以先落地最小场景：

```text
ProjectLauncher default page
AuthoringWorkspace after one frame
```

后续随着编辑器真实 authoring loop 成熟，再补充更多场景。

### 5.5 性能规则

```text
Visual regression 只在测试/验证命令下运行。
正常编辑器 frame 不做 baseline 比较。
正常编辑器 frame 不计算额外 heavy diff。
默认 gate 走 headless deterministic path。
真实窗口 screenshot diff 只能作为 local-only 或 release gate 扩展。
```

## 6. 第一版做什么

```text
新增 EditorVisualRegressionScenario。
新增 EditorVisualRegressionBaseline。
新增 EditorVisualRegressionEvidence。
新增 EditorVisualRegressionReport。
新增 EditorVisualRegressionRunner。
基于 NativeEditorApplication + UiGpuDrawPlan 生成 evidence。
实现 baseline missing / passed / failed 三种状态。
补充序列化测试、匹配测试、不匹配测试、缺 baseline 测试、真实 app 证据测试。
```

## 7. 第一版不做什么

```text
不做跨 GPU 精确像素比较。
不默认创建真实 OS window。
不做动画/video regression。
不做复杂 perceptual diff。
不自动更新 baseline。
不把 visual regression 结果作为用户项目可玩性的唯一证明。
```

## 8. 和现有验证系统的关系

```text
165 Complex Shooter E2E Gate
  验证复杂项目链路能跑通。

169 Exported Windows Real Window Screenshot Gate
  验证导出 Windows player 能产生真实窗口截图证据。

171 Native Editor Real Interaction Validation Gate
  验证编辑器输入 -> 命令 -> session -> model -> draw_list。

173 Native Editor Real Window Interaction Smoke / Screenshot Gate
  验证编辑器真实窗口 smoke 和截图 metadata。

177 Editor Visual Regression / Golden Image Gate
  验证编辑器 UI 结构化视觉输出没有无意回归。
```

177 不是替代前几个 gate，而是在它们之上补齐视觉稳定性。

## 9. 自审

### 9.1 是否合乎规格

结论：通过。

理由：

```text
用户要求做 177，并确认方案。
方案聚焦 editor visual regression，不扩散到测试系统大重构。
方案明确默认不影响正常运行时性能。
```

### 9.2 是否合乎规则

结论：通过。

理由：

```text
保留方案 -> 自审 -> 施工文档 -> 自审 -> 施工 -> 测试 -> 归档流程。
讨论粒度是编辑器视觉回归大系统，不是单个按钮修补。
对比了 UE / Unity / Godot 的路线。
```

### 9.3 是否合乎长期设计

结论：通过。

理由：

```text
采用稳定 evidence / baseline / report 三层结构。
不把 WGPU 或真实窗口截图直接暴露成 editor core 的长期真相。
后续可接入真实 PNG / GPU readback / artifact compare，而不推翻第一版。
```

### 9.4 是否方便实现

结论：通过。

理由：

```text
现有 NativeEditorApplication 已提供 latest_draw_list。
现有 UiGpuDrawPlan 已提供稳定结构化 GPU draw plan。
editor_window_winit 已有 interaction gate / real window smoke gate 的 report 风格可复用。
```

### 9.5 是否合理且能实现

结论：通过。

理由：

```text
第一版不依赖真实 GPU readback，所以默认测试稳定。
baseline 比较从结构化字段开始，能快速覆盖当前 UI 退化风险。
不会阻断后续更真实的 golden PNG 方案。
```

## 10. 最终结论

采用方案 C-min：

```text
Headless deterministic visual evidence
  + structured UiGpuDrawPlan fingerprint
  + explicit golden baseline compare
  + serializable report
```

下一步生成施工文档并按阶段施工。
