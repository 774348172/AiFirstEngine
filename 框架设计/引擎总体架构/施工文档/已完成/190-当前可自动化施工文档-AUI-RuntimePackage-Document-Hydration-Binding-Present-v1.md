# 190-当前可自动化施工文档-AUI RuntimePackage Document Hydration / Binding / Present v1

## 1. 施工目标

基于：

```text
190-AUI-RuntimePackage-Document-Hydration-Binding-Present-v1方案.md
```

完成 C-min：

```text
AUI authoring document
  -> AuiDocumentCooker
  -> RuntimePackage aui/documents/*.aui.json
  -> load_runtime_package AUI document registry
  -> package_smoke_snapshot binding
  -> AuiLayout / AuiDrawList / AuiOverlayFrame
  -> RenderFramePacket.aui_overlay
  -> RuntimeRenderer DrawUiOverlay pass
  -> complex shooter player report AUI present evidence
```

本轮不是完整 M12，不做 209 Scene Unified AUI Authoring、不做真实 glyph atlas、不做 Project Rule -> UI state 完整闭环。

## 2. 必须遵守的边界

```text
Runtime 不扫描项目源目录。
RuntimePackage 是运行输入真相。
ProjectRuntimePackageAssembler 是项目目录进入 RuntimePackageBuildInput 的唯一入口。
AUI 走 UiProjection，不新增 Bridge。
RenderThread 只转发 AuiOverlayFrame，不加载 AUI document，不做 binding/layout。
RuntimeRenderer 只读 AuiOverlayFrame，不读 AuiDocument / binding path / ProjectUiStateSnapshot。
HUD 不能用 debug overlay 假装。
glyph_present=false 时只能通过 C-min partial，不能声明真实 HUD text visible。
```

## 3. 涉及文件

预计修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/aui_document_cooker.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/editor_core/src/tests/project_runtime_package_assembler_tests.rs 或现有相关测试文件

rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/render_thread.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/engine_runtime/src/windowed_player.rs

rust/crates/project_e2e_gate/src/assembly.rs
rust/crates/project_e2e_gate/src/gate.rs
rust/crates/project_e2e_gate/src/report.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/runtime_player_winit/src/lib.rs
```

按实际代码依赖可少改，但不得绕过本施工目标。

预计验证读取：

```text
samples/complex_shooter_project/AUI/hud.aui.json
samples/complex_shooter_project/Build/Windows/dev/data/runtime_package/
```

## 4. Gate A：AUI Document Cook / Package Schema

### 4.1 前置检查

施工前读取：

```text
samples/complex_shooter_project/AUI/hud.aui.json
```

确认当前形态是：

```text
schemaVersion
documentId
root
nodeType
children
```

### 4.2 实现任务

新增：

```text
rust/crates/editor_core/src/aui_document_cooker.rs
```

实现：

```text
AuiDocumentCooker
AuiDocumentCookRequest
AuiDocumentCookReport
AuiDocumentCookDiagnostic
```

职责：

```text
读取 serde_json::Value authoring document。
识别 canonical runtime AuiDocument shape 或 legacy_authoring_tree。
legacy_authoring_tree -> engine_runtime::aui::AuiDocument。
生成 package path：aui/documents/{document_id}.aui.json。
生成 cook report，legacy shape 必须 warning。
收集 asset refs。
```

最小 nodeType 映射：

```text
canvas -> screen overlay canvas + root panel
text -> Text
image -> Image
image-row -> 多个 Image node，或者一个 Image node + warning
panel -> Panel
button -> Button
progress-bar -> ProgressBar
```

更新：

```text
RuntimePackageBuildInput
```

增加 cooked AUI documents 的结构，例如：

```text
pub aui_documents: Vec<RuntimePackageSourceJson>
```

更新：

```text
ProjectRuntimePackageAssembler::assemble
collect_aui_manifest
```

让它通过 `AuiDocumentCooker` 输出：

```text
RuntimePackageBuildInput.aui_documents
RuntimePackageBuildInput.aui_manifest
RuntimeAuiManifestEntry.path = aui/documents/{document_id}.aui.json
```

更新：

```text
RuntimePackageBuilder::write_runtime_package
```

写出：

```text
aui/documents/*.aui.json
aui/aui-manifest.json
```

### 4.3 Gate A 测试

运行：

```powershell
cd rust
cargo test -p editor_core project_runtime_package_assembler
cargo test -p engine_runtime runtime_package_builder
```

必须证明：

```text
package contains aui/documents/hud-main.aui.json
aui-manifest entry path points to aui/documents/hud-main.aui.json
legacy sample hud normalized with warning
RuntimePackageBuilder writes document body
```

## 5. Gate B：RuntimePackage AUI Document Load

### 5.1 实现任务

更新：

```text
rust/crates/engine_runtime/src/runtime_package.rs
```

新增或等价实现：

```text
RuntimeAuiDocumentRegistry
RuntimeAuiDocumentLoadReport
RuntimeAuiDocumentLoadStatus
RuntimeAuiDocumentLoadEntry
```

更新：

```text
RuntimePackage
```

持有：

```text
aui_documents
```

更新：

```text
load_runtime_package
```

加载顺序：

```text
load manifest
load aui manifest
validate aui manifest
read each package_dir / entry.path
deserialize AuiDocument
validate document_id matches entry
validate basic node/canvas shape
insert registry
```

缺失 declared document body 必须 failed diagnostic，不得静默空 HUD。

### 5.2 Gate B 测试

运行：

```powershell
cd rust
cargo test -p engine_runtime runtime_package
```

必须证明：

```text
valid package loads AUI document by id
missing declared AUI document body fails
document id mismatch fails
runtime package no longer only loads manifest
```

## 6. Gate C：Binding / Layout / Overlay Presenter

### 6.1 实现任务

在 `engine_runtime::aui` 中新增或等价实现：

```text
AuiRuntimePresenter
AuiRuntimePresentReport
AuiRuntimePresentStatus
AuiSnapshotSource
```

职责：

```text
select AuiDocument by document_id
build package_smoke_snapshot
resolve bindings
layout
extract draw list
build AuiOverlayFrame
produce AuiRuntimePresentReport
```

第一版默认 snapshot：

```text
package_smoke_snapshot
```

预填示例：

```text
game.score_text = "SCORE 000000"
player.hp_ratio = 1.0
game.paused = false
```

report 必须含：

```text
snapshot_source
snapshot_value_count
binding_status
layout_status
draw_item_count
text_command_count
image_command_count
glyph_present=false
ui_pass_inserted
diagnostics
```

### 6.2 Gate C 测试

运行：

```powershell
cd rust
cargo test -p engine_runtime aui
```

必须证明：

```text
binding resolves text/progress/visible/image
package_smoke_snapshot resolves at least one normal binding
fallback warning path works
missing binding without fallback reports error
overlay draw_item_count > 0
glyph_present=false is reported honestly
```

## 7. Gate D：RenderThread / RuntimeRenderer Present

### 7.1 实现任务

更新：

```text
rust/crates/engine_runtime/src/render_thread.rs
```

让：

```text
RenderThreadFrameInput
RenderFramePacket
```

携带：

```text
aui_overlay: Option<AuiOverlayFrame>
```

更新 `execute_packet`：

```text
RuntimeRendererInput { aui_overlay: packet.aui_overlay.as_ref(), ... }
```

更新调用点：

```text
engine_host_loop.rs
render_thread_worker.rs tests / helpers
```

默认无 AUI 时保持 None。

### 7.2 Gate D 测试

运行：

```powershell
cd rust
cargo test -p engine_runtime runtime_renderer
cargo test -p engine_runtime render_thread
```

必须证明：

```text
aui_overlay Some inserts DrawUiOverlay before Present
aui_overlay None keeps old behavior
RenderThread no longer drops packet AUI overlay
```

## 8. Gate E：Complex Shooter E2E AUI Present Evidence

### 8.1 实现任务

更新：

```text
project_e2e_gate
runtime_player_winit / windowed_player report
```

把 AUI package/load/present evidence 写入结构化报告：

```text
aui_package_document_count
aui_loaded_document_count
aui_draw_item_count
aui_text_command_count
aui_ui_pass_inserted
aui_glyph_present
aui_status
aui_next_actions
```

如果 glyph 尚未真实接通：

```text
aui_status = partial
aui_glyph_present = false
aui_next_actions contains runtime_text_glyph_present
```

### 8.2 Gate E 测试

运行：

```powershell
cd rust
cargo test -p project_e2e_gate
cargo test -p runtime_player_winit
```

必须证明：

```text
complex shooter exported package has aui/documents/hud-main.aui.json
load_runtime_package sees AUI document registry
headless player report includes AUI present summary
DrawUiOverlay pass exists
```

## 9. 最终回归测试

施工全部完成后运行：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_core project_runtime_package_assembler
cargo test -p engine_runtime aui
cargo test -p engine_runtime runtime_package
cargo test -p engine_runtime runtime_package_builder
cargo test -p engine_runtime runtime_renderer
cargo test -p engine_runtime render_thread
cargo test -p project_e2e_gate
cargo test -p runtime_player_winit
```

如某个 filter 没有匹配测试，必须改用对应 crate 的实际 test 名或整 crate 测试，并在阶段记录说明。

## 10. 完成后文档同步

完成后必须：

```text
生成阶段完成记录：
  阶段完成记录/2026-07-04-AUI-RuntimePackage-Document-Hydration-Binding-Present-v1/00-总览.md

把本施工文档移动到：
  施工文档/已完成/190-当前可自动化施工文档-AUI-RuntimePackage-Document-Hydration-Binding-Present-v1.md

更新：
  49-当前状态与下一步入口.md
  54-当前可自动化施工总入口.md
  施工文档/README.md
  阶段完成记录/README.md
```

更新摘要必须说明：

```text
AUI document body now packaged
RuntimePackage loader now loads AUI documents
RenderThread forwards AUI overlay
Complex shooter AUI present evidence status
glyph_present 是否仍为 false
```

## 11. 施工自审

施工前检查：

```text
是否从 190 正式方案生成：是。
是否一次只施工一份文档：是。
是否避免 Runtime 扫源目录：是。
是否避免新增 Bridge：是。
是否避免把打飞机玩法写入引擎 API：是。
是否有 Gate A-E 测试命令：是。
是否允许 glyph false 但不假装完整 HUD text visible：是。
```

如果施工中发现必须修改 190 规则，先回填 190，再继续施工。
