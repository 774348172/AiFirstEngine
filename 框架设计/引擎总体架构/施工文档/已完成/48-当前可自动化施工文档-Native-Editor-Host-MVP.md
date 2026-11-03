# 当前可自动化施工文档：Native Editor Host MVP

本文档定义 Native Editor Host C-lite 长期路线的第一版可施工范围。

来源文档：

```text
18-长期主义实现路径.md
37-Editor-Core与可迁移UI路线.md
38-Rust-Native-Runtime-MVP与TypeScript退役规则.md
47-Native-Editor-Host-BC路线.md
```

## 当前结论

```text
可以开始 Native Editor Host MVP。
采用 C-lite 长期路线。
Rust Editor Core / UI Model 是长期资产。
winit + wgpu + 自研 UI renderer 是正式 UI Backend v1。
egui / eframe 不进入正式窗口主线。
editor_ui_backend_egui 仅保留为 headless compatibility test adapter。
Native Host 第一版直接调用 engine_runtime crate。
Electron / React 不再扩展为主线。
```

第一版面板范围固定为：

```text
Toolbar
Hierarchy
Inspector
Viewport Shell
Console
RuntimeTrace
```

第一版布局固定为：

```text
Toolbar
------------------------------------------------
Hierarchy | Viewport Shell | Inspector
------------------------------------------------
Console / RuntimeTrace
```

第一版不做复杂 Dock。固定布局足够验证 Native Host + Editor Core + Runtime direct call 主链路。

Editor Core v1 边界：

```text
只管理 Runtime Package。
不直接管理完整 Project。
不接 Asset DB。
不接 Build Graph。
不接 AI Patch。
Inspector 只读。
```

UiCommand 执行规则：

```text
采用 Transaction-first Command System。
所有 UiCommand 必须进入 CommandTransaction。
第一版做事务生命周期、诊断、state change 摘要和 undo_policy。
第一版不做完整 Undo / Redo 栈。
第一版不允许 partial_success。
```

第一版数据流：

```text
Self UI Renderer
  -> UiCommand
  -> Editor Core
  -> engine_runtime crate
  -> viewport texture / RenderFrameReport / RuntimeTrace / FrameHash
  -> Editor Core ViewModel
  -> Self UI Renderer / wgpu 渲染
```

## 施工原则

必须遵守：

```text
一项一测。
先建 crate 和数据边界，再做 UI。
Editor Core 不依赖任何 UI backend。
UI Model 不依赖 winit / wgpu / self renderer / egui。
self UI renderer 只渲染 UI Model。
editor_ui_backend_egui 只作为 headless compatibility test adapter。
Runtime 调用走 engine_runtime crate。
第一版只读取 Runtime Package。
不接 Electron。
不接 TypeScript Runtime。
不实现真实 Native RHI。
不删除 Electron legacy shell。
```

必须暂停的情况：

```text
需要修改 Runtime Package v1 核心语义。
需要修改 engine_runtime 公共数据结构的大方向。
需要让 Native Host 读取 Electron / React 内存 Project。
需要把 Editor Core 写成任何 UI backend 专属。
需要新增真实 Asset Importer。
需要新增真实 Build Graph UI 全量迁移。
需要引入复杂 Dock / Graph / Text Editor。
```

## 建议新增 crate

```text
rust/crates/editor_core/
rust/crates/editor_ui_model/
rust/crates/editor_ui_renderer/
rust/crates/editor_window_winit/
rust/crates/editor_ui_backend_egui/  # compatibility test adapter only
rust/crates/editor_host/
```

如果为了第一版减少工程复杂度，可以先：

```text
editor_core
editor_ui_model
editor_host
```

但代码模块必须按 backend 可替换方向组织。

## A0：Workspace 与 crate 骨架

### 目标

建立 Native Editor Host 的 Rust 工程入口。

新增：

```text
rust/crates/editor_core/Cargo.toml
rust/crates/editor_core/src/lib.rs
rust/crates/editor_ui_model/Cargo.toml
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_host/Cargo.toml
rust/crates/editor_host/src/main.rs
scripts/test-native-editor-workspace.cjs
```

修改：

```text
rust/Cargo.toml
package.json
```

建议 npm script：

```json
"test:nativeeditor": "node scripts/test-native-editor-workspace.cjs"
```

测试：

```powershell
npm.cmd run test:rusttoolchain
npm.cmd run test:nativeeditor
npm.cmd run build
```

完成标准：

```text
editor_core cargo test 通过。
editor_ui_model cargo test 通过。
editor_host cargo check 通过。
不打开窗口也能跑 CI smoke。
```

## A1：Editor Core 最小 Session

### 目标

Editor Core 能读取 Runtime Package manifest，并建立最小编辑器会话。

实现：

```text
EditorSession
RuntimePackageSession
RuntimeController
SelectionState
EditorCommandRegistry
CommandRequest
CommandTransaction
CommandResult
StateChangeSummary
UndoPolicy
DiagnosticsStore
ConsoleStore
RuntimeTraceStore
PanelLayoutState
ViewModelBuilder
EditorDiagnostic
```

CommandTransaction v1 字段：

```text
transaction_id
request_id
command_id
source
payload
status
read_set[]
write_set[]
diagnostics[]
state_changes[]
undo_policy
```

CommandTransaction.status：

```text
pending
validated
committed
rejected
failed
```

UndoPolicy v1：

```text
none
snapshot_ready
future_undoable
```

CommandResult v1 字段：

```text
transaction_id
request_id
command_id
status
diagnostics[]
console_entries[]
state_changes[]
ui_model_revision
```

EditorDiagnostic v1 字段：

```text
severity
code
message
source
command_id?
request_id?
path?
entity_id?
trace_entry_id?
suggested_action?
```

第一版命令：

```text
open_runtime_package
reload_runtime_package
select_entity
tick_runtime
tick_one_frame
play
pause
step_frame
reset_runtime
clear_console
select_trace_entry
```

测试：

```powershell
cargo test -p editor_core
npm.cmd run test:nativeeditor
```

完成标准：

```text
Editor Core 可以创建 session。
Editor Core 可以记录当前 runtime package。
Editor Core 可以选择 entity id。
Editor Core 可以返回结构化 diagnostics。
Editor Core 可以从 Runtime Package 派生只读 ViewModel。
Editor Core 不直接读取完整 Project。
Editor Core 不接 Asset DB / Build Graph / AI Patch。
Editor Core 不依赖任何 UI backend。
所有 UiCommand 都会生成 CommandTransaction。
命令成功会返回 committed CommandResult。
命令失败会返回 failed / rejected CommandResult，并写入 diagnostics。
StateChangeSummary 只做摘要，不保存完整反向补丁。
```

最小测试：

```text
select_entity 生成 transaction。
select_entity 成功后 status = committed。
select_entity 的 state_changes 包含 selection.selected_entity_id。
选择不存在 Entity 返回 failed / rejected，并带 editor.entity.not_found diagnostic。
tick_one_frame 生成 transaction，read_set / write_set 至少标记 runtime.frame。
未加载 Runtime Package 时 tick_one_frame 返回 editor.runtime_package.not_loaded。
clear_console 的 undo_policy = snapshot_ready。
第一版任何命令都不能返回 partial_success。
```

## A2：UI Model v1

### 目标

定义 UI Backend 可渲染的数据协议。

UI Model v1 采用：

```text
Typed ViewModel + Command Intent
```

规则：

```text
Editor Core 输出类型化 ViewModel。
UI Backend 只渲染 ViewModel。
所有交互都变成 UiCommand。
UiCommand 是 UI Backend 唯一交互出口。
```

实现：

```text
EditorUiModel
PanelDescriptor
PanelLayoutModel
PanelRegion
UiCommand
ToolbarModel
ToolbarCommand
HierarchyModel
HierarchyNode
InspectorModel
InspectorSection
InspectorField
ViewportModel
RenderableSummary
ConsoleModel
ConsoleEntry
RuntimeTraceModel
RuntimeTraceEntryView
```

EditorUiModel 字段：

```text
frame
active_runtime_package
panels
toolbar
hierarchy
inspector
viewport
console
runtime_trace
diagnostics
```

PanelLayoutModel 第一版固定：

```text
top: toolbar
left: hierarchy
center: viewport
right: inspector
bottom: console / runtime_trace
```

InspectorField 第一版值类型：

```text
String
Bool
Number
Vec3
AssetRef
EntityRef
Json
```

Inspector 第一版字段来源：

```text
Transform
Renderable
Hierarchy
Metadata
```

UiCommand payload 第一版：

```text
OpenRuntimePackage { path }
ReloadRuntimePackage
SelectEntity { entity_id }
TickOneFrame
Play
Pause
StepFrame
ResetRuntime
ClearConsole
SelectTraceEntry { entry_id }
```

测试：

```powershell
cargo test -p editor_ui_model
npm.cmd run test:nativeeditor
```

完成标准：

```text
UI Model 可从 EditorSession 派生。
UI Model 可序列化为 JSON 用于调试。
UI Model 不依赖 winit / wgpu / self renderer / egui。
UI Model 字段命名对 AI 可读。
UI Backend 可以只根据 EditorUiModel 渲染六个 MVP 面板。
UI Backend 的交互只能产生 UiCommand。
InspectorField 第一版 readonly=true。
```

最小测试用例：

```text
初始 Runtime Package 状态可以生成 HierarchyModel。
select_entity(entity-gun) 后 InspectorModel.selected_entity_id 更新。
InspectorModel.readonly == true。
tick_one_frame 后 ViewportModel.frame 增加。
tick_one_frame 后 RuntimeTraceModel 出现 engine.frame_loop 记录。
UiCommand 不允许直接调用 engine_runtime。
```

## A3：Runtime 直连

### 目标

Native Editor Host / Editor Core 能直接调用 `engine_runtime`。

实现：

```text
load runtime package
load active scene
tick one frame
read RenderFrameReport / viewport summary
read RuntimeTrace
read FrameHash
```

测试：

```powershell
npm.cmd run test:rustruntime
npm.cmd run test:rustruntimepackage
npm.cmd run test:rustruntimegolden
cargo test -p editor_core native_editor_can_tick_runtime_package
```

完成标准：

```text
不通过 TypeScript Runtime。
不通过 Electron。
同一个 Runtime Package 能被 engine_runtime 加载并 tick。
Editor Core 能拿到 RenderFrameReport / RuntimeTrace / FrameHash。
```

## A4：Headless UI Backend Adapter v1（兼容测试）

### 目标

实现最小 headless compatibility test adapter。
真实窗口和正式 UI Backend 从 A7 开始，走 winit + wgpu + 自研 UI renderer。

建议依赖：

```text
none
editor_ui_model
editor_core
```

第一版面板：

```text
Hierarchy panel
Inspector panel
Console panel
RuntimeTrace panel
Viewport shell panel
Toolbar
```

面板职责：

```text
Toolbar:
  Open Runtime Package / Reload / Play / Pause / Step Frame / Tick 1 Frame / Reset Runtime。

Hierarchy:
  显示当前 runtime scene entity 树。
  显示 entityId / name / parent / children / alive state。
  点击选择 Entity。

Inspector:
  只读显示选中 Entity 的 Transform / Renderable / Hierarchy / metadata。

Viewport Shell:
  显示 sceneId / frame / frameHash / renderable count / selected entity summary。
  显示 renderables table 或 simple 2D placeholder。

Console:
  显示 Editor diagnostics / Runtime diagnostics / Command result / Package load result。

RuntimeTrace:
  显示 frame / phase / systemId / message / entityCount / diagnostic level。
  如果 trace 带 entityId，点击 trace 可以定位 Entity。
```

规则：

```text
headless backend 只读取 UI Model。
headless backend 操作只发 UiCommand。
UiCommand 交给 Editor Core 执行。
headless backend 不直接修改 Runtime Package。
headless backend 不直接调用 engine_runtime。
editor_ui_backend_egui 仅作为 compatibility test adapter，不进入正式窗口主线。
```

测试：

```powershell
cargo check -p editor_host
cargo test -p editor_core
cargo test -p editor_ui_model
npm.cmd run test:nativeeditor
```

如果增加 headless UI smoke：

```powershell
npm.cmd run test:nativeeditorsmoke
```

完成标准：

```text
editor_host 可以启动窗口。
窗口能显示基础布局。
窗口能显示 Runtime Package / Scene / Entity 摘要。
窗口能显示 RuntimeTrace。
窗口能触发 tick_runtime command。
Toolbar 可以打开 Runtime Package。
Hierarchy 点击 Entity 后 Inspector 更新。
Viewport Shell 可以显示 frame / frameHash / renderable count。
Console 可以显示加载、tick、错误日志。
```

## A5：Viewport Shell

### 目标

第一版 Viewport 只展示 RenderSnapshot 摘要，不做真实 3D 渲染。

长期规则：

```text
真实 Scene Viewport 不由 Editor UI Renderer 自己画 3D 世界。
Runtime Renderer 使用 EditorCamera / EditorViewport mode 渲染世界内容。
Runtime Renderer 输出 viewport render target / texture。
Editor UI Renderer 把该 texture 合成进 Scene 面板。
Editor UI Renderer 继续负责面板外壳、工具栏、Overlay UI 和输入命中。
```

显示：

```text
sceneId
frame
frameHash
renderable count
entityId
meshRef
materialRef
localPosition
```

不做：

```text
真实 wgpu 3D 绘制。
相机控制。
Transform gizmo。
选中描边。
材质预览。
```

完成标准：

```text
Viewport shell 能证明 Native Host 已经接到 Runtime 输出。
真实渲染留给 Renderer / RHI 后续阶段。
后续真实 Scene Viewport 必须复用 Runtime Renderer，而不是新增第二套 3D 渲染器。
```

## A6：文档回填

完成后更新：

```text
18-长期主义实现路径.md
20-编辑器工程结构.md
37-Editor-Core与可迁移UI路线.md
47-Native-Editor-Host-BC路线.md
阶段完成记录/YYYY-MM-DD-Native-Editor-Host-MVP/
```

记录：

```text
新增 crate。
新增测试命令。
Native Host 当前能做什么。
仍未做什么。
Electron legacy shell 当前状态。
```

## A7：Native Window + winit / wgpu Skeleton

目标：

```text
建立真实 Native Editor Window。
使用 winit 管理窗口生命周期和输入事件。
使用 wgpu 建立 surface / device / queue / swapchain 配置。
不接 egui / eframe。
不实现复杂 Dock / Graph / Text Editor。
```

当前后续施工方案：

```text
采用方案 B：窗口壳 + 事件协议 + DrawList 渲染后端。
editor_window_winit 只负责 OS window / event loop / wgpu surface。
editor_input 只负责 WindowEvent / EditorInputEvent 到 UiCommand。
editor_core 只负责业务命令执行和 EditorUiModel 输出。
editor_ui_renderer 只负责 EditorUiModel 到 UiDrawList。
后续新增 editor_wgpu_renderer，把 UiDrawList 转成 GPU commands 并 present。
```

自动化第一版边界：

```text
默认测试路径先实现 editor_window_winit skeleton。
skeleton 必须固定 winit window attributes、wgpu surface/device/queue/swapchain plan。
真实 winit / wgpu 依赖通过 real-window feature 接入。
默认 CI / 本机自动化测试不强制创建 OS 窗口，避免 GUI 环境和图形工具链阻塞纯数据链路。
```

完成标准：

```text
默认门禁：
editor_window_winit 可以通过 headless skeleton 测试。
window attribute plan 可以稳定描述 title / size / resizable。
wgpu surface plan 可以稳定描述 surface / device / queue / swapchain 边界。
editor_host 可以引用 Native Window skeleton。

真实窗口门禁：
开启 editor_window_winit/real-window feature 后，editor_host 可以打开真实窗口。
窗口可以创建 wgpu surface。
窗口 resize 不崩溃。
窗口事件可以进入 Input Router。
窗口关闭可以正常退出。
```

## A8：Self UI Renderer Rect / Text MVP

目标：

```text
建立自研 UI renderer 的最小绘制能力。
第一版只画背景、panel rect、label text、selected state、简单列表。
renderer 只消费 EditorUiModel，不保存编辑器业务状态。
```

完成标准：

```text
Toolbar / Hierarchy / Inspector / Console / RuntimeTrace / Viewport Shell 可以以最小样式显示。
renderer 不直接调用 Editor Core。
renderer 不直接修改 Runtime Package。
renderer 不直接调用 engine_runtime。
```

## A9：Input Event Routing MVP

目标：

```text
winit event -> Editor Input Router -> UiCommand。
点击 Hierarchy Entity 可以产生 SelectEntity。
Toolbar button 可以产生 TickOneFrame / Play / Pause / ResetRuntime。
```

完成标准：

```text
输入命中测试由 UI renderer / Input Router 完成。
业务命令仍由 Editor Core 执行。
所有交互都能生成 CommandTransaction / Diagnostics / StateChangeSummary。
```

## 第一版不做

```text
不删除 Electron。
不迁移 AI 面板。
不做 AI 对话。
不做 Patch Review。
不迁移完整 Build Report Panel。
不迁移完整 Asset Browser。
不做完整 Dock。
不做 Graph Editor。
不做字段编辑。
不做真实 3D Viewport。
不做相机控制。
不做 Transform gizmo。
不做菜单 / 快捷键系统。
不做真实 Native RHI。
不做 IR -> Rust AOT。
不做真实资源导入。
不做移动端打包。
```

## 总回归命令

A0-A5 完成后执行：

```powershell
npm.cmd run test:rusttoolchain
npm.cmd run test:rustruntime
npm.cmd run test:rustruntimepackage
npm.cmd run test:rustruntimegolden
npm.cmd run test:nativeeditor
npm.cmd run build
```

如果任何一步失败：

```text
先修复当前阶段。
不跳到下一阶段。
不临时绕过测试。
不把任何 UI backend 专属代码写进 Editor Core。
```

## 当前推荐执行顺序

```text
1. A0 Workspace 与 crate 骨架
2. A1 Editor Core 最小 Session
3. A2 UI Model v1
4. A3 Runtime 直连
5. A4 Headless UI Backend Adapter v1（兼容测试）
6. A5 Viewport Shell
7. A6 文档回填
8. A7 Native Window + winit / wgpu Skeleton
9. A8 Self UI Renderer Rect / Text MVP
10. A9 Input Event Routing MVP
```

## 2026-06-22 执行状态

已完成：

```text
A0 Workspace 与 crate 骨架。
A1 Editor Core 最小 Session。
A2 UI Model v1。
A3 Runtime 直连。
A4 Headless egui-backend adapter v1。
A5 Viewport Shell 数据模型。
```

新增 crate：

```text
rust/crates/editor_ui_model
rust/crates/editor_core
rust/crates/editor_ui_backend_egui  # compatibility test adapter only
rust/crates/editor_host
```

新增测试脚本：

```text
scripts/test-native-editor-workspace.cjs
package.json -> test:nativeeditor
```

当前 `editor_ui_backend_egui` 状态：

```text
已实现 headless adapter 和单元测试。
不会接真实 egui / eframe 窗口。
路线调整后，它只作为 compatibility test adapter。
正式窗口主线改为 winit + wgpu + 自研 UI renderer。
```

已通过：

```powershell
npm.cmd run test:nativeeditor
npm.cmd run test:rustruntime
npm.cmd run test:rustruntimepackage
npm.cmd run test:rustruntimegolden
npm.cmd run build
```

当前边界：

```text
Native Host 目前是最小 CLI host，不打开真实窗口。
Editor Core 只管理 Runtime Package，不管理完整 Project。
Inspector 只读。
Viewport Shell 只输出 RenderSnapshot 摘要模型。
editor_ui_backend_egui 当前只做 headless compatibility test adapter，不做真实绘制。
真实绘制后续由 editor_ui_renderer / editor_window_winit 承担。
Electron legacy shell 未删除。
```

## 2026-06-22 A7-A9 自动化施工追加状态

本轮按当前施工文档继续执行 A7 / A8 / A9，并在最后做文档同步。

新增 crate：

```text
rust/crates/editor_ui_renderer
rust/crates/editor_input
rust/crates/editor_window_winit
```

新增 / 扩展测试入口：

```text
scripts/test-native-editor-workspace.cjs
scripts/test-native-editor-real-window.cjs
package.json -> test:nativeeditor
package.json -> test:nativeeditor:realwindow
```

A8 Self UI Renderer Rect / Text MVP 已完成默认自动化闭环：

```text
editor_ui_renderer 可以从 EditorUiModel 生成 UiDrawList。
UiDrawList 包含 DrawCommand、ViewportTextureSlot、HitRegion。
renderer 只消费 EditorUiModel，不调用 Editor Core。
renderer 不修改 Runtime Package。
renderer 不调用 engine_runtime。
已覆盖 Toolbar / Hierarchy / Inspector / Console / Viewport Shell 的最小绘制数据。
```

A9 Input Event Routing MVP 已完成默认自动化闭环：

```text
editor_input 提供 EditorInputRouter。
PointerDown 命中 Toolbar HitRegion 可以生成 TickOneFrame / Play / Pause / ResetRuntime 等 UiCommand。
PointerDown 命中 Hierarchy Entity 可以生成 SelectEntity。
Console / Viewport 当前只做命中，不生成业务命令。
业务执行仍由 Editor Core 负责。
```

A7 Native Window + winit / wgpu Skeleton 已完成 headless skeleton：

```text
editor_window_winit 提供 NativeEditorWindowConfig。
editor_window_winit 提供 NativeWindowAttributePlan。
editor_window_winit 提供 WgpuSurfacePlan。
默认测试路径不编译真实 winit / wgpu 依赖。
真实 winit / wgpu 依赖通过 real-window feature 接入。
```

本机验证记录：

```powershell
npm.cmd run test:nativeeditor
```

结果：

```text
通过。
editor_ui_model / editor_core / editor_ui_backend_egui / editor_ui_renderer / editor_input / editor_window_winit / editor_host 单元测试全部通过。
editor_host check 通过。
```

真实 winit / wgpu 编译门禁记录：

```text
曾尝试默认编译真实 winit / wgpu 依赖。
当前 Windows GNU Rust 链路缺少 dlltool.exe，导致 libloading 编译失败。
因此真实窗口依赖已改为 real-window feature。
这不是路线回退；正式 UI Backend v1 仍是 winit + wgpu + 自研 UI renderer。
后续装齐 Windows GUI Rust 工具链后，再执行 real-window feature 门禁。
```

真实窗口 feature 门禁命令：

```powershell
npm.cmd run test:nativeeditor:realwindow
```

当前边界更新：

```text
Native Host 仍是 CLI smoke，不打开真实窗口。
Self UI Renderer 已有 DrawList / HitRegion 数据层。
Input Router 已有从 HitRegion 到 UiCommand 的最小链路。
Window crate 已固定 winit / wgpu skeleton 和 feature-gated 真实图形依赖。
真实 OS window、真实 wgpu surface、resize event loop、swapchain present 尚未完成。
Electron legacy shell 未删除。
```
