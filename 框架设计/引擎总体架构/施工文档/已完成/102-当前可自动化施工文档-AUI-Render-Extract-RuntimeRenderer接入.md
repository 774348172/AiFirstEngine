# 102-当前可自动化施工文档-AUI Render Extract / RuntimeRenderer 接入

## 1. 施工目标

按照 `102-AUI-Render-Extract-RuntimeRenderer接入方案.md`，实现 AUI DrawList 到 RuntimeRenderer 的第一版接入：

```text
AuiDrawList
  -> AuiRendererBridge
  -> AuiOverlayFrame
  -> RuntimeRenderer UI Render Pass
  -> RenderGraph
```

本阶段采用 UE-like DrawElement 路线，不让 AUI 直接生成底层 RHI 命令，也不让 AUI 变成 RenderProxy。

## 2. 明确不做

```text
不做真实 GPU UI 渲染。
不做真实字体 atlas。
不做复杂 Text shaping。
不做 Clip / Mask。
不做 UI batching。
不做 Button state。
不做 HitTest。
不做 WorldSpace UI。
不让 RuntimeRenderer 直接读取 AuiDocument。
不让 AUI 进入 Sprite2D sorting。
不让 AUI 变成 RenderProxy。
```

## 3. 施工文件

修改：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/render_graph.rs
```

文档归档：

```text
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/2026-06-29-AUI-Render-Extract-RuntimeRenderer接入/00-总览.md
```

## 4. 施工步骤

### Step 1：新增 AUI overlay bridge 类型

在 `aui.rs` 增加：

```text
AuiOverlayFrame
AuiOverlayDrawItem
AuiOverlayItemKind
AuiOverlaySortKey
AuiRendererBridge
```

规则：

```text
AuiRendererBridge 只读取 AuiDrawList。
AuiRendererBridge 不读取 RuntimeRenderer / RHI。
DrawRect -> Rect item
DrawImage -> Image item
DrawText -> TextPlaceholder item
```

最小测试：

```text
aui_renderer_bridge_converts_draw_list_to_overlay_frame
aui_renderer_bridge_keeps_overlay_tree_order
```

### Step 2：扩展 RenderGraph UI pass

在 `render_graph.rs` 增加：

```text
RenderPassKind::DrawUiOverlay
RenderPassCommand::DrawUiOverlay
```

字段：

```text
target
item_count
text_count
image_count
debug_label
```

规则：

```text
DrawUiOverlay 是第一版 headless / RenderGraph 级 pass 描述。
不表达真实 GPU command。
```

最小测试：

```text
render_graph_ui_overlay_command_references_target
```

### Step 3：RuntimeRenderer 接入 AUI overlay

在 `RuntimeRendererInput` 增加：

```text
aui_overlay: Option<&AuiOverlayFrame>
```

在 `RuntimeRenderer::build_graph` 中插入：

```text
Clear
World / Sprite draw passes
AUI Overlay pass
Present
```

规则：

```text
aui_overlay 为 None 时行为保持不变。
aui_overlay 为空 draw_items 时不插入 UI pass。
UI pass debug_source = RuntimeRenderer.auiOverlay。
RuntimeRenderFrameReport.draw_item_count 需要包含 UI item_count。
```

最小测试：

```text
runtime_renderer_inserts_aui_overlay_pass_before_present
runtime_renderer_without_aui_overlay_keeps_existing_passes
```

## 5. 验证命令

```powershell
cargo fmt --check
cargo check -p engine_runtime
cargo test -p engine_runtime aui --no-run
cargo test -p engine_runtime runtime_renderer --no-run
cargo test --workspace --no-run
```

说明：当前机器可能会阻止 Cargo 测试二进制实际执行，若 `cargo test` 运行出现 `os error 4551`，以 `--no-run` 编译级验证作为本机可用验证。

## 6. 完成标准

```text
规则文档存在。
施工文档归档到已完成。
AUI overlay bridge 编译通过。
RuntimeRenderer 可选接收 AuiOverlayFrame。
RenderGraph 能出现 DrawUiOverlay pass。
UI pass 位于 Present 前。
无 AUI overlay 时保持旧行为。
```
