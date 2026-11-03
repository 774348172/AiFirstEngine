# 121-当前可自动化施工文档-Native Editor Application Shell 方案C

## 1. 目标

本次施工把 Native Editor 从“真实窗口展示静态 EditorUiModel”推进到：

```text
NativeEditorApplication
  owns EditorSession
  owns MainFrame / PanelRegistry / DockLayout / FocusInput / CommandSystem / TransactionService
  routes input -> command -> EditorSession -> rebuild UiModel -> DrawList -> present
```

本次不是补某个单独按钮，也不是完整实现所有编辑器功能，而是先把完整 Unity / UE 式编辑器应用壳骨架落地。

## 2. 对应规则

```text
121-Native-Editor-Application-Shell方案.md
120-Native-Editor-Usable-Panels-B-AI-C-min方案.md
111-Native-Editor-Real-UI-Present-方案B.md
113-Native-Editor-FontSystem-v1方案.md
114-Native-Editor-UI-RenderGraph-RHI-收敛方案.md
```

## 3. 施工边界

必须做：

```text
新增 NativeEditorApplication 类型。
NativeEditorApplication 持有 EditorSession。
新增 EditorMainFrame / PanelRegistry / DockLayoutManager。
新增 EditorFocusInputSystem。
新增 EditorCommandSystem。
新增 EditorTransactionService wrapper。
新增 EditorUiModelComposer。
真实窗口从静态 EditorUiModel 改为持有 NativeEditorApplication。
WindowEvent 的鼠标 / 键盘事件进入 NativeEditorApplication。
Headless 测试覆盖完整链路。
```

不做：

```text
完整 Dock 拖拽。
复杂多窗口。
完整菜单编辑器。
完整 Inspector 控件库。
真实 AI 模型调用。
项目侧玩法规则。
```

## 4. 代码施工顺序

### A1：依赖调整

文件：

```text
rust/crates/editor_window_winit/Cargo.toml
```

动作：

```text
增加 editor_core 依赖。
```

验证：

```powershell
cargo check -p editor_window_winit
```

### A2：新增 Application Shell 类型

文件：

```text
rust/crates/editor_window_winit/src/lib.rs
```

新增：

```text
NativeEditorApplication
EditorMainFrame
PanelRegistry
EditorPanelDescriptor
DockLayoutManager
DockLayoutNode
EditorFocusInputSystem
EditorCommandSystem
EditorTransactionService
EditorUiModelComposer
NativeEditorApplicationReport
```

验证：

```powershell
cargo test -p editor_window_winit native_editor_application
```

### A3：Headless 应用壳闭环

能力：

```text
NativeEditorApplication::frame
NativeEditorApplication::handle_input_event
NativeEditorApplication::handle_shortcut
NativeEditorApplication::latest_model
NativeEditorApplication::latest_draw_list
```

验证：

```text
点击 Hierarchy -> command -> EditorSession -> UiModel revision 增加。
Ctrl+Z -> UndoSceneEdit。
AI proposal accept -> CommandSystem -> EditorSession。
TextInput 在 focused text field 时被 UI 消费，不流向 Runtime。
GameView focused key input 仍可走 RuntimeInputFrame。
```

### A4：真实窗口接入

改造：

```text
RealNativeEditorApp
  from model: Option<EditorUiModel>
  to app: NativeEditorApplication
```

事件接入：

```text
CursorMoved -> EditorInputEvent::PointerMove
MouseInput Pressed -> PointerDown
MouseInput Released -> PointerUp
KeyboardInput Pressed -> KeyDown
KeyboardInput Released -> KeyUp
RedrawRequested -> app.frame() -> present latest draw_list
```

保留兼容：

```text
run_real_native_editor_window_with_model(model)
```

它只作为测试/兼容入口，用 model 初始化 shell 的 latest model，不作为长期主入口。

### A5：测试与格式化

必须执行：

```powershell
cargo fmt --check
cargo test -p editor_window_winit native_editor_application
cargo test -p editor_window_winit
cargo test -p editor_host
```

如 real-window feature 可编译：

```powershell
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

## 5. 验收标准

本次施工完成后，以下结论必须成立：

```text
真实编辑器窗口长期入口是 NativeEditorApplication。
窗口事件不再停留在静态展示层。
PanelRegistry / DockLayout / FocusInput / CommandSystem / TransactionService 都有正式类型。
所有 UI 修改仍经过 EditorSession / Transaction。
EditorCore 不依赖 winit / wgpu。
UiDrawList 仍只是渲染产物，不成为业务真相。
```

