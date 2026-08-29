# Native Editor Host 与 Schema-driven UI 路线

本文档定义编辑器长期技术路线。

当前确认结论：

```text
编辑器主线不再是 Electron / React Editor Shell。
编辑器主线改为 Native Rust Editor Host + Rust Editor Core + 自研 UI Schema / Command / Layout Model + 可替换 UI Backend。
采用 C-lite 长期路线：winit + wgpu + 自研 UI renderer 是正式 UI Backend v1，不是临时替换对象。
egui / eframe 不进入正式窗口主线，editor_ui_backend_egui 仅保留为 headless compatibility test adapter。
Electron / React 进入 legacy transition shell。
不再围绕 Electron 设计 Rust Runtime Bridge / Sidecar。
Rust Runtime 与编辑器优先采用 Native Editor Host 内部 crate / direct boundary。
Electron 只维护现有功能，不新增底层主线能力。
```

这不是要求立刻完整自研 Unity / Unreal 级 UI 框架。  
它要求现在就把最终方向切到 Native Editor Host，同时用最小 Native UI 后端验证编辑器底层能力。

## 为什么不再以 Electron 为主线

Electron / React 已经帮助当前项目快速验证了：

```text
AI 面板
Patch Plan 审批
Inspector
Hierarchy
Project / Asset 面板
Console
Runtime Trace
Build Report
Replay Debug
导出按钮和本地构建桥接
```

但它不适合作为长期主线：

```text
包体大。
Runtime / Renderer / Editor Host 边界容易被 Web Shell 反向锁死。
Rust Runtime 如果围绕 Electron sidecar 设计，会把临时壳变成正式架构约束。
Viewport / Graph / Inspector / Dock 等重型编辑器能力最终仍需要 native 级性能和控制力。
长期自研引擎要接近 Unity / Unreal 的工程形态，不能把桌面编辑器核心绑在浏览器壳上。
```

因此新规则是：

```text
Electron 可以继续存在。
Electron 不再是编辑器主线。
Electron 不再决定 Runtime Bridge、Renderer Preview、Asset Import、Build Report 的最终接口形态。
```

## 为什么不是立刻完整自研 UI

完整自研 UI 框架不是按钮库，而是一套编辑器平台：

```text
窗口系统
Dock / Tab / Layout
菜单 / 快捷键 / 命令系统
Inspector
Hierarchy
Project Browser
Graph Editor
Timeline
Profiler
Console
Undo / Redo
拖拽
文本输入和中文输入法 IME
高 DPI
多显示器
主题系统
插件 UI
```

现在直接重写全部 UI，会把主要精力从 Runtime、ECS、Asset DB、Renderer、AI Patch、Build Graph 上抽走。

当前已确认采用 C-lite 长期路线：

```text
第一版先做 Native Rust Editor Host MVP。
Rust Editor Core / UI Schema / Command / Layout Model 必须自研。
UI Backend v1 使用 winit + wgpu + 自研 UI renderer。
底座一步到位，功能慢慢加。
Dock / Graph / Inspector / Viewport Overlay 等关键能力逐步补齐，但不再先绑定到 egui / eframe。
Electron legacy shell 只维护现有功能，不继续长成第二套正式编辑器。
```

## 总体架构

长期结构：

```text
Native Rust Editor Host
  -> Editor Core
  -> Engine Core / Rust Runtime / Renderer / Asset DB / Build Graph

Editor Core
  -> Project Model
  -> Selection Model
  -> Command System
  -> Undo / Redo
  -> Patch Plan
  -> Validation
  -> Asset DB Service
  -> Build Service
  -> Runtime Service
  -> Inspector Schema
  -> Panel Layout Schema
  -> Graph Model

UI Backend
  -> winit + wgpu + self UI renderer backend v1
  -> custom Dock / Graph / Inspector / Viewport widgets
  -> optional other backend

Legacy Electron / React Shell
  -> existing panels and workflows only
  -> transition / maintenance
```

依赖方向：

```text
UI Backend -> Editor Core -> Engine Core
Native Editor Host -> Editor Core -> Engine Core
Legacy Electron Shell -> Editor Core / legacy services
```

禁止方向：

```text
Engine Core -> Electron
Engine Core -> React
Editor Core -> React Component
Editor Core -> DOM
Rust Runtime -> Electron main
Renderer -> Electron renderer
Project Logic -> Editor UI
Build Graph -> UI Component
```

## Native Editor Host

Native Editor Host 是正式编辑器宿主。

第一版负责：

```text
桌面窗口
菜单和命令入口
Dock / Panel 容器最小实现
加载 Editor Core
调用 Rust Runtime
调用 Build Graph / Asset DB / Renderer 服务
承载 Viewport surface
输出结构化 diagnostics
```

Viewport surface 的边界：

```text
Editor UI Renderer 负责画 Viewport 面板外壳、Toolbar、Tab、Overlay UI 和命中区域。
Runtime Renderer 负责画 Viewport 内的 Scene / Game 世界内容。
Scene Viewport 通过 Runtime Renderer 输出的 render target / texture 嵌入 Editor UI。
```

Native Editor real-window / Viewport Surface gate 已确认采用接近 Unreal 的结构：

```text
NativeEditorHost
  -> EditorWindowBackend
      -> OS Window / EventLoop / Surface lifecycle
  -> EditorCore
      -> UI Schema / Layout / Command / Transaction
  -> EditorUiRenderer
      -> 编辑器 UI
  -> ViewportHost
      -> Viewport 区域、尺寸、焦点、输入归属
  -> RuntimeRenderer
      -> Scene / Game 世界内容
```

第一版 gate 采用 C-min：真实 winit window、wgpu surface、resize / dpi / close / redraw、最小固定 panel layout、一个 Scene Viewport rect、Runtime Renderer clear / test triangle / test texture、输入分发、最小 SceneCamera state、SelectionOutline 占位、Gizmo disabled / placeholder state 和 RealWindowGateReport。

C-min 不是完整 Unity / Unreal 级 Scene Viewport；它只验证真实 Editor Viewport 闭环和长期边界。

所有 C-min 能力必须支持 headless 测试。真实 OS window / real wgpu surface 只能作为本机 smoke gate 和平台兼容 gate，不能成为唯一验证方式。WindowBackend、Surface、Input routing、ViewportHost、EditorUiRenderer、RuntimeRenderer、SceneCamera、SelectionOutline、Gizmo placeholder 和 RealWindowGateReport 都必须有 headless / mock / noop backend 等价入口。

规则：

```text
Editor UI Renderer 不实现 3D world renderer。
Runtime Renderer 不实现编辑器面板。
Scene View 使用 Runtime Renderer 的 EditorViewport mode。
Game View 使用 Runtime Renderer 的 GameView mode。
```

第一版不追求：

```text
完整 Unity / Unreal 级 UI 体验。
完整插件 UI。
完整自研文本编辑器。
完整 Graph Editor。
完整性能分析器。
```

Native Editor Host 的关键价值不是“换一个窗口壳”，而是：

```text
Runtime、Renderer、Asset、Build、Trace 不再绕 Electron 设计。
编辑器和引擎底层可以共享 Rust crate / direct API。
Viewport、Profiler、Graph 等重能力可以逐步 native 化。
```

## UI Backend

UI Backend 是可替换的渲染和控件承载层。

第一版建议：

```text
winit + wgpu + 自研 UI renderer 作为 Native Editor Host v1 的正式 UI Backend。
```

原因：

```text
窗口、输入、GPU 绘制底座从第一天就走最终方向。
第一版只实现 Rect / Text / Panel / Click / Selection 等最小能力。
可以验证 Command / Selection / Inspector Schema / Panel Layout。
不会把长期架构绑死在 Electron 或 egui。
```

长期规则：

```text
UI Backend 只渲染 Editor Core 提供的 schema / model / command。
UI Backend 不持有项目真相。
UI Backend 不直接修改 Project Schema。
UI Backend 不决定 Patch 是否合法。
UI Backend 可以被替换，但正式 v1 不再先绑定到 egui / eframe。
自研 UI renderer 不允许成为业务逻辑层。
editor_ui_backend_egui 只允许作为 headless compatibility test adapter。
```

后续逐步自研的关键 UI：

```text
Dock / Tab / Layout
Graph Editor
Inspector Field Widgets
Viewport Tool Overlay
Profiler / Trace View
Asset Browser
```

## Editor Core

Editor Core 是编辑器业务真相，必须独立于 UI 后端。

Editor Core 负责：

```text
Project Model
Selection Model
Command Registry
Undo / Redo
Patch 应用
Validation
Inspector 数据模型
Panel Layout 数据模型
Graph 数据模型
Asset DB 访问
Build Report 访问
Runtime / Replay 访问
```

Editor Core 必须遵守：

```text
不能 import React。
不能依赖 DOM。
不能依赖 Electron renderer。
不能直接调用任意 fs。
不能直接持有 GPU backend。
```

需要本地能力时，Editor Core 通过明确服务接口：

```text
FileService
BuildService
AssetImportService
RuntimeService
ReplayDebugService
RendererPreviewService
```

## Schema-driven UI

长期目标：

```text
Inspector 由 Component Schema / Project Schema 驱动。
Panel Layout 由 Panel Layout Schema 驱动。
Graph Editor 由 Graph Model / Graph Schema 驱动。
Command Palette 由 Command Registry 驱动。
菜单 / 快捷键由 Command Schema 驱动。
AI Patch Review 由 Patch Plan Schema 驱动。
Build Report 由 Report Schema 驱动。
```

这意味着：

```text
AI 生成或修改的是 schema / model / patch。
Native UI 渲染 schema。
Legacy React UI 也只能渲染 schema。
同一份 Editor Core 数据可以被不同 UI Backend 展示。
```

## Electron / React Legacy Shell 边界

Electron / React 当前定位：

```text
legacy transition shell
```

允许：

```text
维护现有编辑器功能。
修复现有 UI bug。
保留当前导出、报告、调试入口。
作为迁移期间的可运行编辑器。
读取和展示 Editor Core 已有数据。
```

不允许：

```text
新增底层主线能力。
围绕 Electron 设计 Rust Runtime Bridge。
把 RuntimeService 设计成 Electron sidecar 专属。
把 Renderer Preview 绑在 DOM / React 上。
把 Asset DB / Build Graph 的正式能力放进 Electron main。
让 React 组件直接成为项目数据真相。
让 AI 通过修改 React 文件来实现项目功能。
```

当前已有 Electron bridge 可以继续按 legacy 规则维护。  
新增本地能力默认应优先判断是否属于 Native Editor Host / Editor Core，而不是继续扩展 Electron bridge。

## Runtime 接入规则

旧路线：

```text
Electron / React
  -> RuntimeService
  -> sidecar / JSON-RPC
  -> Rust Runtime
```

新主线：

```text
Native Rust Editor Host
  -> Editor Core / RuntimeService
  -> engine_runtime crate / direct native boundary
  -> Rust Native Runtime
  -> viewport texture / RenderFrameReport / RuntimeTrace / FrameHash
  -> Native UI Backend 展示
```

Electron legacy shell 如需预览 Rust Runtime，只能作为过渡适配：

```text
Legacy Electron Shell
  -> compatibility bridge
  -> Native Runtime service
```

但该路径不能成为长期接口标准。

## 迁移路线

### 阶段 1：停止让 Electron 扩张成主线

目标：

```text
不删除 Electron。
不再新增 Electron 专属底层能力。
所有新能力先判断是否进入 Editor Core / Native Editor Host。
```

完成标准：

```text
文档口径统一。
新施工文档不再把 Electron bridge 作为默认下一步。
Rust Runtime 下一步不再默认 sidecar。
```

### 阶段 2：Editor Core 继续抽离

目标：

```text
Selection Model
Command Registry
Panel Layout Schema
Inspector Schema
Graph Model
Build / Runtime / Asset service boundary
```

完成标准：

```text
Legacy React UI 只是现有 View。
Native Editor Host 可以复用同一 Editor Core。
```

### 阶段 3：Native Editor Host MVP

目标：

```text
Rust desktop app 启动。
加载 Editor Core。
加载 Runtime Package。
调用 Rust Runtime。
显示最小 Hierarchy / Inspector / Console / Viewport shell。
```

第一版 UI Backend：

```text
winit + wgpu + 自研 UI renderer
```

完成标准：

```text
Native Editor Host 可以打开一个测试项目。
Native Editor Host 可以运行 Rust Runtime Golden Scenario。
Native Editor Host 可以显示 viewport shell / RenderFrameReport / RuntimeTrace。
Native Editor Host 不依赖 Electron。
```

### 阶段 4：关键编辑器 UI native 化

优先顺序：

```text
Viewport
Inspector
Hierarchy
Console / Trace
Project / Asset Browser
Graph Editor
Build Report
AI Panel
```

原则：

```text
先复用 schema-driven Editor Core。
再替换 UI Backend。
不要把 UI 迁移变成项目数据迁移。
```

### 阶段 5：Electron 退役

退役条件：

```text
Native Editor Host 覆盖主要编辑器工作流。
Rust Runtime Preview 已在 Native Host 中稳定。
Build / Asset / Trace / Patch / Validation 已走 Editor Core 服务。
Legacy Electron 不再承载唯一能力。
```

退役方式：

```text
冻结 Electron 新功能。
保留一段时间作为 legacy launcher / fallback。
删除 Electron-only runtime path。
最终移入历史或工具目录。
```

## 与 Unity / Unreal 的关系

学习 Unity / Unreal 的方向：

```text
编辑器是 native 级工程工具，不是普通网页。
编辑器需要 Command、Undo、Selection、Inspector、Viewport、Asset Browser、Profiler、Graph。
编辑器能力必须服务引擎工程，而不是散落在 UI 组件里。
```

不同点：

```text
Unity / Unreal 上层主要为人工操作设计。
本引擎上层必须同时为 AI 操作设计。
```

因此本项目的关键不是马上复制 Slate / UI Toolkit，而是建立：

```text
Native Editor Host
Schema-driven Editor Core
AI 可理解的 Command / Patch / Validation
可替换 UI Backend
逐步 native 化的 Viewport / Graph / Inspector
```

## 当前明确不做

当前不做：

```text
不立刻删除 Electron。
不立刻完整重写所有 React 面板。
不立刻自研完整 Slate / UI Toolkit。
不把 Runtime 放进 Electron。
不新增 Electron 专属底层能力。
不让 React 直接接管项目真相。
不让 AI 直接修改 UI 文件来实现项目功能。
```

## 当前必须开始做

从下一阶段开始：

```text
新编辑器底层能力默认进入 Native Editor Host / Editor Core。
新 Runtime 接入默认走 Native Editor Host direct boundary。
新 UI 能力默认 schema-driven。
Legacy Electron 只做维护和过渡。
涉及项目修改，必须通过 Patch / Command / Validation。
涉及本地文件能力，必须通过 FileService / BuildService 等受控服务。
```

## 总结

最终路线：

```text
短期：
  Electron / React 保留为 legacy transition shell。
  不再围绕 Electron 设计新底层能力。

中期：
  Native Rust Editor Host MVP。
  Editor Core schema-driven。
  UI Backend v1 使用 winit + wgpu + 自研 UI renderer。

长期：
  关键 UI 在自研 UI renderer 上逐步补齐。
  Rust Runtime / Renderer / Asset DB / Build Graph 与编辑器通过 native boundary 协作。
  Electron 退役或保留为 legacy fallback。
```

核心判断：

```text
主线必须 native。
Editor Core 必须 schema-driven。
UI Backend 必须可替换。
Electron 只能 legacy。
```
