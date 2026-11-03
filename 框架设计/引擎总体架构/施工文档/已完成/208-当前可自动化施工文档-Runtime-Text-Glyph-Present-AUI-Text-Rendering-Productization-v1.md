# 208-当前可自动化施工文档：Runtime Text Glyph Present / AUI Text Rendering Productization v1

## 1. 施工目标

按 `208-Runtime-Text-Glyph-Present-AUI-Text-Rendering-Productization-v1方案.md` 的 **方案 C-min** 施工：

```text
Build / Cook 阶段生成最小 FontAtlas
RuntimePackage 写入 cooked FontAtlas
RuntimePackage load 加载 FontAtlas registry
AUI Presenter 用 cooked FontAtlas 生成 glyph quads
RuntimeRenderer / Player / project_e2e_gate 报告 glyph evidence
```

完成后，复杂打飞机 HUD 不能再只靠 `text_count` 证明文本进入 UI pass；必须能在 report 中看到：

```text
font_atlas_present=true
rendered_glyph_count > 0
glyph_present=true
text_pass_inserted=true
```

## 2. 方案与审查输入

正式方案：

```text
框架设计/引擎总体架构/208-Runtime-Text-Glyph-Present-AUI-Text-Rendering-Productization-v1方案.md
```

外部审查输入：

```text
未发现 208 专门审查文档。
已参考 12-185-M12-AUI-HUD方案审查.md 中“字体渲染最紧迫”结论。
已参考 15-190-AUI-RuntimePackage-Document-Hydration方案审查.md 中“glyph_present=false 必须诚实 partial”结论。
```

## 3. 涉及文件

预计涉及：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/render_graph.rs
rust/crates/engine_runtime/src/rhi_command_plan.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/project_e2e_gate/src/gate.rs
rust/crates/project_e2e_gate/src/report.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/aui_document_cooker.rs
```

可能新增：

```text
rust/crates/editor_core/src/aui_font_atlas_cooker.rs
```

文档同步：

```text
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-06-Runtime-Text-Glyph-Present-AUI-Text-Rendering-Productization-v1/00-总览.md
```

## 4. Gate A：FontAtlas C-min schema / report

任务：

```text
在 engine_runtime 中定义 cooked FontAtlas 最小数据结构。
RuntimePackage 持有 RuntimeAuiFontAtlasRegistry。
AuiRuntimePresentReport 增加 font atlas / glyph evidence 字段。
保持 serde schema-first，可序列化、可测试。
```

必须证明：

```text
默认空 registry 不会让 glyph_present=true。
report 能区分 font_atlas_present / rendered_glyph_count / fallback_used。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui
```

## 5. Gate B：Build-time FontAtlas cook

任务：

```text
新增 AuiFontAtlasCookerCmin。
从 AUI document 收集 ASCII printable + 静态文本字符。
项目 font-main.asset 是 placeholder 时使用 engine_builtin_ui_font_cmin 生成 cooked fallback atlas。
Cooker 输出 FontAtlas json + R8 alpha bytes 的内存结构，交给 RuntimePackageBuilder 写包。
```

必须证明：

```text
complex shooter 的 RuntimePackageBuildInput 含至少 1 个 cooked FontAtlas。
placeholder font 被标记为 font_asset_status=placeholder / fallback_used=true。
```

测试：

```powershell
cd rust
cargo test -p editor_core project_runtime_package_assembler
```

## 6. Gate C：RuntimePackage write/load

任务：

```text
RuntimePackageBuilder 写 fonts/*.fontatlas.json 和 fonts/*.fontatlas.r8。
load_runtime_package 加载 FontAtlas registry。
RuntimePackage load report 输出 font_atlas_count / default_ui_font_atlas_id / diagnostics。
```

必须证明：

```text
缺失 atlas json 或 bitmap 时报告 diagnostic。
有效 package 可按 id 加载 FontAtlas。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime runtime_package
cargo test -p engine_runtime runtime_package_builder
```

## 7. Gate D：AUI presenter glyph plan

任务：

```text
AuiRuntimePresenter 使用 RuntimeAuiFontAtlasRegistry 为 text item 生成 glyph quads。
glyph_present 只由 rendered_glyph_count > 0 和 font atlas evidence 决定。
旧的 glyph_present=false 硬编码必须移除。
```

必须证明：

```text
有 text item 且有 cooked atlas 时 rendered_glyph_count > 0。
有 text item 但无 atlas 时 glyph_present=false 且 diagnostic 明确。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui
```

## 8. Gate E：Renderer / Player / E2E evidence

任务：

```text
RuntimeRenderer report 或 render graph command 携带 glyph evidence。
runtime_player_winit NativeAuiPresentSummary 输出 font atlas / glyph 字段。
project_e2e_gate 使用 glyph evidence 判断，不再固定期待 glyph_present=false。
```

必须证明：

```text
complex shooter e2e 不再把 runtime_text_glyph_present 标为 blocking gap。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime runtime_renderer
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate
```

## 9. Gate F：文档同步与归档

任务：

```text
生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
施工完成后把本文档移动到 施工文档/已完成/。
```

整体回归：

```powershell
cd rust
cargo fmt --check
cargo test -p engine_runtime aui
cargo test -p engine_runtime runtime_package
cargo test -p engine_runtime runtime_package_builder
cargo test -p engine_runtime runtime_renderer
cargo test -p editor_core project_runtime_package_assembler
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate
```

## 10. 施工自审

范围自审：

```text
本施工只补 RuntimePackage cooked FontAtlas C-min，不做完整 FontImporter / CJK shaping / emoji / rich text / IME / 209 Scene Unified AUI Authoring。
```

架构自审：

```text
Runtime 不扫描项目源目录。
Runtime 不扫描系统字体路径作为项目字体真相。
Renderer 不读取 AUI binding path 或 ProjectUiStateSnapshot。
IR / Rule 不直接操作 glyph atlas。
```

测试自审：

```text
每个 Gate 都有 cargo test。
最终回归包含 engine_runtime / editor_core / runtime_player_winit / project_e2e_gate。
```

验收自审：

```text
不得用 text_count 或占位矩形伪装 glyph present。
placeholder font 只能报告 cooked fallback visible，不能宣称 project font fidelity 完成。
```
