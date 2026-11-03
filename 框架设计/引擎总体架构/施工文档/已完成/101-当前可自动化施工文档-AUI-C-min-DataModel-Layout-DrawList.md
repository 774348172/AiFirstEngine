# 101-当前可自动化施工文档-AUI C-min 数据模型 / Layout / DrawList

## 1. 施工目标

按照 `100-AUI-AI-First-Runtime-UI-System方案.md`，实现 AUI Runtime Core 的第一版最小闭环：

```text
AuiDocument
AuiCanvas ScreenOverlay
AuiNode Panel / Image / Text
AuiRect anchor / offset
AuiLayoutEngine headless
AuiDrawList
AuiRenderReport
AuiAssetManifest 最小结构
AuiValidationReport 最小结构
headless tests
```

本阶段只做 Runtime Core 的数据、布局、绘制列表和验证底座。

## 2. 明确不做

```text
不接真实 GPU 渲染。
不接 RuntimeRenderer。
不接 Input / HitTest。
不接 ProjectUiState Binding。
不做 Button / Toggle / Slider / ScrollView / InputField。
不做真实字体 atlas。
不做完整 AUI AI Authoring Pipeline。
不做 imagegen、bbox review UI、visual diff UI。
不让 HTML / 截图 / bbox 成为运行时布局真相。
```

## 3. 施工文件

优先新增：

```text
rust/crates/engine_runtime/src/aui.rs
```

修改：

```text
rust/crates/engine_runtime/src/lib.rs
```

文档归档：

```text
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/2026-06-29-AUI-C-min-DataModel-Layout-DrawList/00-总览.md
```

## 4. 数据结构

新增 AUI 基础类型：

```text
AuiDocument
AuiCanvas
AuiNode
AuiNodeKind
AuiRect
AuiStyle
AuiBindingRef
AuiAssetRef
AuiAssetManifest
AuiAssetManifestEntry
AuiValidationReport
AuiValidationItem
```

新增 layout / draw 类型：

```text
AuiLayoutEngine
AuiLayoutResult
AuiComputedNode
AuiDrawList
AuiDrawCommand
AuiRenderReport
```

## 5. 核心规则

```text
AuiDocument 是 AUI Runtime Core 第一版输入真相。
AuiBlueprint / Authoring Pipeline 后续可以生成 AuiDocument，但本阶段不实现转换器。
AuiRect 使用 anchor_min / anchor_max / offset_min / offset_max / pivot / size。
ScreenOverlay canvas 使用 reference_resolution 作为 layout viewport。
Panel 生成 DrawRect。
Image 生成 DrawImage。
Text 生成 DrawText。
不可见 node 不生成 draw command，但计入 node_count。
资源缺失不 panic，进入 AuiValidationReport。
整屏图片 UI 默认被 validation 拒绝，防止整屏截图加透明热区。
```

## 6. 最小测试

必须新增测试：

```text
aui_layout_resolves_anchor_offsets_for_screen_overlay
aui_draw_list_keeps_tree_order_for_panel_image_text
aui_validation_reports_missing_image_asset
aui_validation_rejects_full_screen_single_image_ui
aui_asset_manifest_maps_asset_to_node
```

## 7. 验证命令

```powershell
cargo fmt --check
cargo test -p engine_runtime aui
cargo test -p engine_runtime
cargo test --workspace --no-run
```

## 8. 完成标准

```text
Rust 编译通过。
AUI 最小测试通过。
AuiDocument 能 headless 计算 layout。
AuiDocument 能生成 DrawList。
AuiValidationReport 能报告缺失资源和整屏图 UI。
不影响现有 runtime / renderer / input 测试。
施工文档归档到 已完成。
阶段完成记录写入。
```
