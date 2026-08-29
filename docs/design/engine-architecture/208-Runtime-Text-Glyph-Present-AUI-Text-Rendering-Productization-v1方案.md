# 208-Runtime Text Glyph Present / AUI Text Rendering Productization v1 方案

## 1. 这个系统是干什么的

一句话：

```text
把 AUI 文本从“有 DrawText 命令”推进到“RuntimePackage 里有可加载的 FontAtlas，运行时能生成真实 glyph quad，并在报告里证明文字可见”。
```

它解决当前明确缺口：

```text
190 / 199 / 204 已经证明 AUI document/package/binding/overlay/UI pass 存在；
但 glyph_present 仍为 false。
```

完成后链路应变成：

```text
AUI Document
  -> Build / Cook 收集文本与字体依赖
  -> RuntimePackage 写入 cooked FontAtlas + glyph metrics
  -> RuntimePackage load FontAtlas registry
  -> AUI Binding / Layout / AuiOverlayFrame
  -> Runtime 使用 cooked FontAtlas 生成 glyph quads
  -> RuntimeRenderer UI text pass evidence
  -> AuiRuntimePresentReport.glyph_present=true
```

本系统不负责：

```text
UI 业务逻辑
209 Scene Unified AUI Authoring
富文本编辑器
InputField / IME
完整多语言排版系统
```

## 2. 为什么选择 C-min

本轮选择：

```text
方案 C-min：RuntimePackage Pre-cooked FontAtlas Asset
```

核心理由：

```text
RuntimePackage 是运行输入真相。
运行时不应该扫描系统字体，也不应该靠本机环境临时生成项目字体结果。
复杂打飞机第一版只需要 HUD 文本可见，不需要一次做完整 TextCore / Slate / TextServer。
```

C-min 不是完整字体系统，而是一个最小真实字体资产链路：

```text
构建期：
  项目字体资产 / 引擎内置默认字体
  -> 最小 glyph 集
  -> atlas alpha bitmap
  -> glyph metrics json
  -> RuntimePackage

运行期：
  只加载 RuntimePackage 里的 FontAtlas
  只根据 glyph metrics 生成 glyph quad
  不做运行时字体扫描
  不做运行时 TTF import
```

## 3. 其它引擎对标

### 3.1 Unity

对标：

```text
Font Asset / TextCore / TextMeshPro Font Asset
```

参考：

```text
Unity Manual: Introduction to font assets
https://docs.unity3d.com/Manual/UIE-font-asset.html
本项目参考文档：
框架设计/Unity源码参考/Runtime-Render-Asset-Production-Binding源码参考.md
```

可学习点：

```text
用户层表达 Text / Font / Style。
字体资产承担 atlas texture / glyph 数据。
项目脚本默认不碰 GPU texture / atlas 细节。
```

不照搬：

```text
不一次引入完整 TextCore / TMP / SDF / 动态 atlas / fallback chain。
不接受 native 黑盒；本项目必须输出 AI 可读 report。
```

### 3.2 Unreal Engine

对标：

```text
Slate text rendering:
FSlateDrawElement::MakeText
  -> FSlateFontServices
  -> FSlateFontCache
  -> font atlas
  -> Slate RHI renderer
```

参考：

```text
UE API: FSlateFontServices
https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/SlateCore/FSlateFontServices
UE API: FSlateFontCache
https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/SlateCore/FSlateFontCache
UE API: FShapedGlyphFontAtlasData
https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/SlateCore/FShapedGlyphFontAtlasData
本项目参考文档：
框架设计/UE源码参考/Runtime-Render-Asset-Production-Binding源码参考.md
```

可学习点：

```text
Font cache / font services 独立于 UI 业务。
glyph atlas 最终进入 RHI texture 资源纪律。
字体渲染证据不应该混入 gameplay API。
```

不照搬：

```text
不照搬完整 Slate、FreeType/HarfBuzz shaped text、复杂 render-thread font cache。
```

### 3.3 Godot

对标：

```text
CanvasItem.draw_string / Font / TextServer / RenderingServer
```

参考：

```text
Godot source: scene/main/canvas_item.cpp
https://github.com/godotengine/godot/blob/master/scene/main/canvas_item.cpp
本项目参考文档：
框架设计/Godot源码参考/11-Runtime-Render-Asset-Production-Binding源码参考.md
```

可学习点：

```text
CanvasItem 只表达 draw_string / draw_char。
Font / TextServer 处理文本。
底层通过 texture / render server 进入渲染。
```

不照搬：

```text
不暴露 RID / RenderingServer handle 给项目 AI。
不在第一版做完整 TextServer。
```

### 3.4 Bevy

对标：

```text
bevy_text FontAtlas / FontAtlasSet
Font asset
  -> glyph raster
  -> FontAtlas image
  -> render asset prepare
  -> GPU image
```

参考：

```text
Bevy source: crates/bevy_text/src/font_atlas.rs
https://github.com/bevyengine/bevy/blob/main/crates/bevy_text/src/font_atlas.rs
Bevy source: crates/bevy_text/src/font_atlas_set.rs
https://github.com/bevyengine/bevy/blob/main/crates/bevy_text/src/font_atlas_set.rs
本项目参考文档：
框架设计/Bevy源码参考/14-Runtime-Render-Asset-Production-Binding.md
```

可学习点：

```text
FontAtlas 持有 glyph_to_atlas_index / texture atlas layout / texture handle。
atlas 最终仍是 Image / render asset。
```

不照搬：

```text
不引入完整 Bevy RenderWorld / ExtractSchedule / PrepareAssets。
只吸收 source asset -> FontAtlas -> render-facing asset 的边界。
```

## 4. 本项目当前基线

已具备：

```text
rust/crates/engine_runtime/src/aui.rs
  AuiDrawCommand::DrawText { text, color, font_size }
  AuiOverlayDrawItem { item_kind=Text, text, font_size }
  AuiRendererBridge::build_overlay_frame(...)
  AuiRuntimePresentReport.glyph_present

rust/crates/engine_runtime/src/runtime_renderer.rs
  DrawUiOverlay pass 已插入 render graph
  当前只记录 item_count / text_count / image_count

rust/crates/engine_runtime/src/render_asset_production.rs
  RuntimeRenderAssetKind::FontAtlas
  FontAtlasDescriptor
  FontAtlasRenderAsset
  RenderBindingKind::FontAtlas

rust/crates/editor_wgpu_renderer/src/font_system.rs
  EditorFontSystem 已能用 ab_glyph 生成 CPU atlas / glyph quads
  real-wgpu 路径已有 R8 glyph atlas texture 思路
```

当前断点：

```text
rust/crates/engine_runtime/src/aui.rs
  AuiRuntimePresenter::present_with_snapshot_output(...)
  glyph_present = false

rust/crates/engine_runtime/src/runtime_renderer.rs
  DrawUiOverlay 只证明 UI pass 有 aggregate command
  没有 glyph atlas / glyph quad / text pass evidence

samples/complex_shooter_project/Assets/font-main.asset
  当前是 placeholder，不是真 font bytes
```

## 5. 方案对比

### 方案 A：运行时临时 glyph planner

```text
AuiOverlayFrame text item
  -> RuntimeAuiFontSystem
  -> runtime rasterize
  -> glyph atlas / glyph quads
```

优点：

```text
施工最短。
能快速让 glyph_present=true。
```

问题：

```text
容易滑向运行时扫描系统字体。
RuntimePackage 不再是字体结果真相。
和发布包 / 热更新资源治理不够一致。
```

结论：

```text
不作为本轮最终选择。
```

### 方案 B：抽共享 FontSystem crate

```text
editor_wgpu_renderer::EditorFontSystem
  -> shared font system crate
  -> editor + runtime 共用
```

优点：

```text
长期代码复用更好。
编辑器与运行时字体行为更容易统一。
```

问题：

```text
触碰 editor_wgpu_renderer / engine_runtime / RHI 边界。
现有 EditorFontSystem 有系统字体路径逻辑，不能直接成为 RuntimePackage 真相。
为当前 glyph_present 缺口改动偏大。
```

结论：

```text
后续重构候选，不作为本轮选择。
```

### 方案 C：RuntimePackage 预 cook FontAtlas

```text
Font source asset / engine default font
  -> build-time font cook
  -> FontAtlas asset + glyph metrics
  -> RuntimePackage
  -> Runtime load atlas and draw glyph quads
```

优点：

```text
包内容确定，符合 RuntimePackage 真相原则。
最接近 Unity Font Asset / Bevy FontAtlas / UE Slate atlas 的资产化方向。
后续资源热更新更自然：替换 FontAtlas 资产，而不是替换 runtime 逻辑。
```

问题：

```text
完整方案会牵出 font importer、font schema、atlas packing、fallback chain、多语言 shaping。
```

结论：

```text
采用 C-min：只做复杂打飞机 HUD 需要的最小 cooked FontAtlas。
```

## 6. 正式推荐方案：C-min

采用：

```text
RuntimePackage Pre-cooked FontAtlas Asset C-min
```

一句话规则：

```text
运行时不生成字体资产；运行时只消费 RuntimePackage 中已经 cook 好的 FontAtlas。
```

C-min 的最小链路：

```text
AUI documents + default font policy
  -> collect required glyph set
  -> AuiFontAtlasCookerCmin
  -> RuntimePackage fonts/*.fontatlas.json
  -> RuntimePackage fonts/*.fontatlas.r8
  -> RuntimeAuiFontAtlasRegistry
  -> AuiTextGlyphPlanner
  -> glyph quads
  -> RuntimeRenderer text evidence
```

为什么它适合当前复杂打飞机：

```text
复杂打飞机 HUD 主要需要分数、血量、弹药、波次、提示文本等 ScreenOverlay 文本。
第一版可以用 ASCII / Latin / digits / punctuation 覆盖这些 HUD。
如果项目字体资产仍是 placeholder，构建期使用 engine default font 生成包内 cooked fallback atlas，并在 report 中标记。
这样导出的包里仍然有真实 FontAtlas，不依赖运行机器的系统字体。
```

## 7. C-min 范围

必须支持：

```text
单默认 UI 字体。
ASCII printable glyphs: 0x20..0x7E。
从 AUI 文档静态文本中额外收集出现过的字符。
绑定文本中的数字和基础符号。
单 atlas page。
R8 alpha atlas 或等价 alpha bitmap。
glyph metrics: uv / size / bearing / advance。
LTR 单行基础排布。
rect 宽度裁剪。
AuiRuntimePresentReport 输出真实 glyph evidence。
```

允许使用 build-time fallback：

```text
项目 font asset 缺失 / placeholder / 无法解析时：
  构建期使用 engine_builtin_ui_font_cmin 生成 FontAtlas。
  RuntimePackage 内仍写入 cooked FontAtlas。
  report 标记 font_source_kind=engine_builtin_cooked_fallback。
  不能宣称 project font fidelity 已完成。
```

暂不支持：

```text
CJK shaping。
emoji。
富文本。
TextInput / IME。
多字体 fallback chain。
多 atlas page。
atlas eviction。
动态运行时新增 glyph。
SDF / MSDF。
复杂 kerning / HarfBuzz shaping。
```

## 8. 数据结构预留

C-min 只填最小字段，但 schema 要给后续留位置。

### 8.1 FontAtlasCookInput

```text
FontAtlasCookInput:
  font_asset_id
  font_asset_ref
  font_source_kind
  requested_ranges
  requested_chars
  default_font_policy
  source_hash
```

预留：

```text
font_family
font_weight
font_style
locale
shaping_engine
fallback_chain[]
```

### 8.2 CookedFontAtlasAsset

```text
CookedFontAtlasAsset:
  schema_version
  font_atlas_id
  font_asset_id
  font_source_kind
  font_asset_status
  atlas_image_path
  atlas_format
  atlas_width
  atlas_height
  atlas_generation
  glyphs[]
  fallback_used
  diagnostics[]
```

`glyphs[]` 第一版字段：

```text
codepoint
glyph_id
uv_rect
pixel_rect
bearing_x
bearing_y
advance
page_index
```

预留：

```text
cluster_index
variation_key
font_face_index
fallback_font_asset_id
msdf_range
```

### 8.3 RuntimeAuiFontAtlasRegistry

```text
RuntimeAuiFontAtlasRegistry:
  atlases_by_id
  default_ui_font_atlas_id
  load_report
```

预留：

```text
atlases_by_font_variant_key
fallback_chain_index
unicode_range_index
```

### 8.4 AuiTextGlyphPlan

```text
AuiTextGlyphPlan:
  frame_index
  font_atlas_id
  text_item_count
  requested_glyph_count
  rendered_glyph_count
  unsupported_glyph_count
  clipped_glyph_count
  glyph_quads[]
  glyph_plan_hash
```

预留：

```text
glyph_runs[]
line_count
shaped_run_count
fallback_run_count
```

## 9. Report 规则

`AuiRuntimePresentReport` 扩展：

```text
glyph_present
font_atlas_present
font_atlas_id
font_source_kind
font_asset_id
font_asset_status
font_fallback_used
requested_glyph_count
rendered_glyph_count
unsupported_glyph_count
clipped_glyph_count
glyph_atlas_width
glyph_atlas_height
glyph_atlas_generation
text_pass_inserted
glyph_plan_hash
```

`RuntimePackageLoadReport` 扩展：

```text
font_atlas_count
font_atlas_load_status
default_ui_font_atlas_id
font_atlas_diagnostics[]
```

`BuildReport` / `ProjectRuntimePackageAssembler` 扩展：

```text
font_cook_count
font_cook_fallback_count
font_cook_diagnostics[]
cooked_font_atlas_paths[]
```

状态规则：

```text
text_command_count > 0 && font_atlas_present=false:
  glyph_present=false
  diagnostic=aui_text.font_atlas_missing

text_command_count > 0 && rendered_glyph_count == 0:
  glyph_present=false
  diagnostic=aui_text.glyph_plan_empty

rendered_glyph_count > 0 && font_source_kind=engine_builtin_cooked_fallback:
  glyph_present=true
  font_fallback_used=true
  diagnostic=aui_text.font_cooked_fallback_used

project font asset placeholder:
  font_asset_status=placeholder
  fallback_used=true
  glyph_present=true only means visible default glyphs are present
```

## 10. Runtime / Render 边界

允许：

```text
editor_core / build 侧实现 AuiFontAtlasCookerCmin。
RuntimePackageBuilder 写入 fonts/*.fontatlas.json 和 fonts/*.fontatlas.r8。
engine_runtime 加载 RuntimeAuiFontAtlasRegistry。
AuiRuntimePresenter / AuiTextGlyphPlanner 使用 registry 生成 glyph quads。
RuntimeRenderer report 写入 text pass / glyph count / atlas evidence。
RuntimeRenderAssetProduction 的 FontAtlas skeleton 用于表达 render-facing FontAtlas。
```

不允许：

```text
Runtime 扫描项目源目录找字体。
Runtime 扫描系统字体路径作为项目字体真相。
RuntimeRenderer 读取 AUI binding path / ProjectUiStateSnapshot。
RenderThread 做 binding resolve。
项目侧 IR / Rule 直接操作 glyph atlas。
用 debug overlay 或纯矩形假装 HUD 文本。
```

## 11. 施工 Gate 建议

Gate A：FontAtlas C-min schema / report

```text
定义 CookedFontAtlasAsset / RuntimeAuiFontAtlasRegistry / glyph metrics。
扩展 AuiRuntimePresentReport / package load report / build report 字段。
测试：cargo test -p engine_runtime aui
```

Gate B：Build-time FontAtlas cook

```text
新增 AuiFontAtlasCookerCmin。
从 AUI 文档收集字符集。
项目字体为 placeholder 时使用 engine_builtin_ui_font_cmin 生成 cooked fallback atlas。
写入 RuntimePackage build input。
测试：cargo test -p editor_core project_runtime_package_assembler
```

Gate C：RuntimePackage write/load

```text
RuntimePackageBuilder 写 fonts/*.fontatlas.json / fonts/*.fontatlas.r8。
load_runtime_package 加载 FontAtlas registry。
测试：
cargo test -p engine_runtime runtime_package
cargo test -p engine_runtime runtime_package_builder
```

Gate D：AUI presenter glyph plan

```text
AuiRuntimePresenter 用 cooked FontAtlas registry 为 text items 生成 glyph quads。
glyph_present 只由 rendered_glyph_count > 0 与 font atlas evidence 决定。
测试：cargo test -p engine_runtime aui
```

Gate E：Renderer / Player / E2E evidence

```text
RuntimeRenderer DrawUiOverlay 或 DrawUiTextGlyphs command 携带 glyph_count / atlas evidence。
runtime_player_winit 和 project_e2e_gate 改用 glyph evidence 判断。
测试：
cargo test -p engine_runtime runtime_renderer
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate
```

Gate F：文档同步

```text
更新 49 / 54 / 阶段完成记录 / 施工文档归档。
```

## 12. 后续接口预留

C-min 后续可扩展为完整字体系统，但不在本阶段施工：

```text
FontImporter:
  ttf / otf / collection import

FontCookBackend:
  ab_glyph
  freetype
  msdf
  harfbuzz_shaping

FontVariantKey:
  family / weight / style / size / locale / smoothing

TextShapingPlan:
  glyph_runs / clusters / bidi / fallback runs

FontFallbackChain:
  primary font
  CJK font
  emoji font
  project override font

FontAtlasPaging:
  multi page
  eviction
  runtime patchable atlas
```

预留原则：

```text
后续扩展只能填充 schema 预留字段或新增版本字段。
不能让 AUI Document 直接持有 atlas path / GPU handle。
不能让项目 Rule 操作字体底层资源。
```

## 13. 自审

是否符合 RuntimePackage 真相：

```text
符合。字体结果由构建期 cook 进入 RuntimePackage，运行时只加载包内 FontAtlas。
```

是否新增用户心智层：

```text
否。用户仍编辑 AUI Document 的 text/style/font ref；FontAtlas 是构建产物。
```

是否满足复杂打飞机基本需求：

```text
满足。复杂打飞机 HUD 的数字、英文、基础符号和静态提示文本可由 C-min glyph 集覆盖。
```

是否伪装验收：

```text
否。方案区分 font_asset_status、font_source_kind、fallback_used、rendered_glyph_count、glyph_present。
placeholder font 只能通过 cooked fallback 证明可见，不能证明项目字体 fidelity。
```

是否和 190 / 195 / 196 冲突：

```text
否。208 补的是 190 留下的 runtime_text_glyph_present 缺口。
IR 不参与字体渲染。
AUI Runtime Core 仍由 Rust 负责。
```

## 14. 最终结论

本系统采用：

```text
Runtime Text Glyph Present / AUI Text Rendering Productization v1
方案 C-min：RuntimePackage Pre-cooked FontAtlas Asset
```

最终目标：

```text
复杂打飞机导出包中存在 cooked FontAtlas；
Runtime load 能加载 FontAtlas registry；
AUI text item 能生成真实 glyph quad；
RuntimeRenderer report 能证明 text pass 与 glyph evidence；
AuiRuntimePresentReport.glyph_present=true。
```

这条路线比 A-min 更适合当前主线，因为它把字体结果放回 RuntimePackage，后续做资源热更新、字体替换、多语言扩展时不会推翻基础链路。
