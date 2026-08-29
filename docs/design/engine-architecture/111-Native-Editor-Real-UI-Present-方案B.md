# 111-Native Editor Real UI Present 方案B

## 问题

当前 Native Editor real-window 已经能创建 winit 窗口，并且能从 `EditorUiModel` 生成 `UiDrawList`。但真实窗口路径仍然没有把 UI 绘制命令提交到 GPU surface，所以用户看到的是空窗口。

本方案只解决一件事：

```text
SelfUiRenderer DrawList
  -> editor_wgpu_renderer
  -> WGPU Surface
  -> Present
```

它不重新讨论 Editor Core、Scene Editing、Runtime Viewport、AUI、RenderGraph，也不把项目逻辑塞进编辑器窗口层。

## 其他引擎对比

### Unreal Engine

UE 的编辑器 UI 路线接近：

```text
FSlateApplication / SWindow
  -> Slate Widget
  -> Slate Draw Elements
  -> SlateRenderer / SlateRHIRenderer
  -> RHI Present
```

参考源码入口：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Private\Framework\Application\SlateApplication.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\SlateRHIRenderer\Private\SlateRHIRenderer.cpp
```

可借鉴点：

```text
窗口 / 输入 / Widget 状态 / 绘制后端分层。
UI 绘制后端消费结构化 draw element，不直接拥有编辑器业务真相。
渲染后端负责 GPU 资源、pipeline、present。
```

不照搬：

```text
完整 Slate Widget 对象体系。
完整 Slate batching / font atlas / clipping / invalidation。
完整 RHI 后端。
```

### Unity

Unity 编辑器路线接近：

```text
ContainerWindow / GUIView / EditorWindow
  -> IMGUI / UI Toolkit Panel
  -> Native backend repaint
  -> Present
```

参考源码入口：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\GUIView.cs
```

可借鉴点：

```text
EditorWindow / GUIView 不等于底层 GPU renderer。
Repaint 由宿主调度。
UI 状态和底层渲染不是一层。
```

不照搬：

```text
IMGUI 即时模式历史包袱。
UI Toolkit / IMGUI 双体系。
```

### Bevy

Bevy 提供了 Rust + winit + wgpu 的工程参考：

```text
bevy_winit
  -> Window / event loop
bevy_render
  -> Extract / Render world / Surface present
```

可借鉴点：

```text
winit window 与 wgpu surface 生命周期清晰分离。
主世界数据和 render 执行分离。
```

不照搬：

```text
Bevy 双 World RenderApp。
Bevy ECS schedule。
```

### Godot

Godot 路线接近：

```text
Control / CanvasItem
  -> draw commands
  -> RenderingServer
  -> DisplayServer present
```

可借鉴点：

```text
UI 节点生成绘制命令。
RenderingServer 负责真正渲染。
```

不照搬：

```text
Godot Node / Control 体系。
RenderingServer 全量架构。
```

## 方案选择

采用方案 B：

```text
editor_window_winit
  -> OS Window / EventLoop / Surface lifecycle

editor_ui_renderer
  -> EditorUiModel -> UiDrawList / HitRegion

editor_wgpu_renderer
  -> UiDrawList -> GPU draw plan / WGPU commands / Surface present
```

它最像 UE 的 SlateRenderer 分层，也比 Unity 更适合我们，因为我们需要 AI 能读懂中间产物。

## 边界规则

### editor_window_winit

负责：

```text
winit EventLoop
OS Window
resize / close / redraw
surface 生命周期持有和调用
把 WindowEvent 转成 EditorInputEvent
调用 Editor Core / Input / UI Renderer / WGPU Renderer 的顺序编排
```

禁止：

```text
直接实现 UI GPU 绘制细节。
直接保存业务 UI 真相。
直接修改 Runtime Package / Project。
```

### editor_ui_renderer

负责：

```text
EditorUiModel -> UiDrawList
EditorUiModel -> HitRegion
```

禁止：

```text
依赖 winit。
依赖 wgpu。
提交 GPU 命令。
```

### editor_wgpu_renderer

负责：

```text
消费 UiDrawList。
生成 UiGpuDrawPlan。
维护 UI GPU pipeline / buffer / shader。
把 Rect / ViewportTextureSlot placeholder 绘制到 WGPU Surface。
输出 RealUiPresentReport。
```

禁止：

```text
读取 EditorSession。
执行 UiCommand。
调用 engine_runtime。
修改项目数据。
解释业务语义。
```

## 第一版 C-min 范围

必须实现：

```text
新增 editor_wgpu_renderer crate。
UiGpuDrawPlan 从 editor_window_winit 迁出。
HeadlessUiGpuRenderer 使用同一份 UiDrawList 生成 deterministic report。
RealUiPresentReport 可序列化，AI 可读。
DrawCommand::Rect 进入可绘制批次。
DrawCommand::ViewportTextureSlot 第一版绘制为 placeholder rect。
DrawCommand::Text 第一版不渲染，计入 skipped_text_count，不算错误。
真实窗口 redraw 时调用 editor_wgpu_renderer。
真实窗口不再把“生成 draw list”伪装成 presented。
```

第一版不实现：

```text
真实文字渲染。
字体 atlas。
图片纹理。
复杂 clipping。
复杂 batching。
9-slice。
真实 viewport texture sampling。
多窗口。
完整 Dock / Panel 控件。
```

## Report 规则

后续补充规则：

```text
112-Native-Editor-Text-Rendering-C-min方案.md
```

112 已确认：`DrawCommand::Text` 第一版使用内置 ASCII debug font 转为 rect glyphs，不再默认全部 skipped。

`RealUiPresentReport` 最小字段：

```text
schema_version
backend
surface_width
surface_height
draw_command_count
rect_count
viewport_slot_count
skipped_text_count
submitted_batch_count
presented
present_status
diagnostics[]
```

diagnostics 最小字段：

```text
severity
code
message
source_stage
```

规则：

```text
Text skipped 是 C-min 能力边界，不是错误。
空 surface 是错误。
没有 Rect / ViewportTextureSlot 但有 DrawList 时是 warning。
真实 present 失败必须进入 diagnostics，不能只打 stderr。
```

## 和当前 88 文档的关系

`88-真实NativeEditorWindow-EventLoop-UIDraw-C-min方案.md` 是真实窗口 gate 文档。

本文件是 88 后续的窄化补充规则：

```text
88 负责 Window / EventLoop / UIDraw gate。
111 负责 DrawList -> WGPU Surface 的真实 present。
```

后续凡是讨论“窗口为什么空”“UI 是否真正画到 GPU”“SelfUiRenderer DrawList 如何 present”，以本文档为准。

## 推荐施工顺序

```text
1. 新增 editor_wgpu_renderer crate。
2. 迁出 UiGpuDrawPlan。
3. 新增 HeadlessUiGpuRenderer 和 RealUiPresentReport。
4. 为 UiDrawList -> UiGpuDrawPlan / Report 补测试。
5. editor_window_winit 依赖 editor_wgpu_renderer。
6. HeadlessNativeEditorWindowApp 使用 HeadlessUiGpuRenderer。
7. real-window 路径创建 WGPU renderer 并在 RedrawRequested 调用 present。
8. editor_host real-window 输出真实 present report。
9. 更新文档索引和完成记录。
```

## 完成标准

```text
cargo test -p editor_wgpu_renderer 通过。
cargo test -p editor_window_winit native_editor_window_headless 通过。
cargo test -p editor_host real_window 通过。
cargo test --workspace 通过。
cargo check -p editor_window_winit --features real-window 通过，或因本机 OS policy 被明确标记为 environment_blocked。
cargo run -p editor_host --features real-window -- --real-window 打开的窗口不再是纯空白；至少能看到 clear color / panel rect / viewport placeholder。
```
