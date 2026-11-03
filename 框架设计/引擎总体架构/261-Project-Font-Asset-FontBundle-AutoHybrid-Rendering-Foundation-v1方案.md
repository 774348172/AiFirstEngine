# 261-Project Font Asset / FontBundle / AutoHybrid Rendering Foundation v1 方案

## 1. 文档状态

```text
系统编号：261
方案版本：v1
建立日期：2026-07-30
选题来源：塔防项目 P0-5“首个真实可玩界面”暴露的项目中文字体基础能力缺口
前置系统：208 Runtime Text Glyph Present C-min、210 RuntimeRenderer UI Composition、
          219 Editor GameView GPU Texture Sharing、236 Save/Reload/Rebuild Consistency
用户决定：采用 G2-C，不把字体作为塔防项目临时补丁；字体基础能力本轮一次形成长期合同
光栅策略：AutoHybrid，32 physical px 为默认分界
当前状态：正式方案已确认并完成方案自审
施工状态：Window A / Gate A 已完成；等待 Window B 单独授权
施工授权：Window A；2026-07-30 18:01 +08:00 至 21:01 +08:00
```

本文只定义通用引擎字体基础能力。塔防项目是首个需求方，但引擎资产、Interface、diagnostic、
report、fixture 和验收不得出现赵云、阿斗、军粮、轮次、单位或其它塔防专用语义。

## 2. 一句话目标

把现有单一 ASCII C-min FontAtlas 深化为项目可导入、可组合、可确定性 cook、可多页加载、
可由 AUI 选择、可在小字号保持 Hinting 清晰度并在大字号和缩放场景保持 MSDF 清晰度的
正式 FontBundle 基础设施，同时保持 RuntimePackage 是发布运行输入真相。

完整链路：

```text
FontFaceAsset
  -> FontFamilyAsset
  -> FontStackAsset
  -> FontAtlasProfileAsset
  -> ProjectFontCookModule
  -> CookedFontBundle v2
  -> RuntimePackage
  -> RuntimeFontRegistry
  -> AUI text style resolve / glyph planning
  -> Bitmap R8 or MSDF glyph draw
  -> UiProjection / RuntimeRenderer / Present
```

## 3. 已确认的产品决定

### 3.1 采用 G2-C

字体是通用引擎基础资产，不采用以下临时做法：

```text
不把真实中文字体硬编码为 font-main 的特殊分支。
不让项目作者手工维护完整 glyph 列表。
不继续把 8px ASCII atlas 当作正式字体质量。
不只补一张中文单页 atlas 后再延期 fallback、multi-page 和字体选择。
不让 Runtime 扫描系统字体或读取项目源 TTF/OTF。
不为塔防项目建立私有 FontLoader、私有 shader 或私有 renderer 旁路。
```

### 3.2 采用 AutoHybrid

默认规则：

```text
requested physical pixel size <= 32
且不要求连续大比例缩放
  -> Hinting R8 bitmap variant

requested physical pixel size > 32
或存在连续缩放、旋转、粗描边、显著 DPI 放大
  -> MSDF variant
```

项目级 `FontAtlasProfileAsset` 可显式选择：

```text
autoHybrid
hintedBitmap
msdf
```

不提供普通单通道 SDF 正式模式。它在小字号清晰度上不如 Hinting R8，在尖角和大比例缩放上
不如 MSDF；仅有较低纹理体积这一项优势，不足以抵消第三条 shader/cook 路径带来的永久复杂度。

### 3.3 Runtime 只消费 cooked 结果

```text
Editor / Build：
  读取项目字体源文件
  解析字体
  解析 family / stack / profile
  收集 glyph set
  rasterize / MSDF generate
  pack pages
  写入 RuntimePackage

Runtime：
  只加载 CookedFontBundle
  不读取 TTF / OTF / TTC
  不扫描 Windows / Linux / macOS 系统字体目录
  不运行 FontImporter
  不按首次遇到字符动态修改发布包
```

### 3.4 字体切换不得改变排版

Hinting R8 与 MSDF 是同一设计字体的不同 render variant。两种 variant 必须共享规范化的：

```text
glyph advance
bearing
ascent
descent
line gap
kerning adjustment
baseline
```

切换 render variant 只能改变采样和边缘重建，不能导致换行、字符位置或布局尺寸跳动。

## 4. 当前实现证据与缺口

### 4.1 已有能力

`rust/crates/engine_runtime/src/runtime_package.rs` 已有：

```text
RuntimeFontAtlasManifest
CookedFontAtlasAsset
CookedFontAtlasGlyph
R8 alpha bitmap
glyph UV / pixel rect / bearing / advance / page index
RuntimePackage font atlas load
```

`rust/crates/engine_runtime/src/aui.rs` 已有：

```text
AUI DrawText / overlay text item
RuntimeAuiFontAtlasRegistry
按 codepoint 查 cooked glyph
AuiTextGlyphPlan
glyph quad / rendered glyph evidence
```

`rust/crates/editor_wgpu_renderer/src/font_system.rs` 已证明：

```text
ab_glyph FontArc
outline_glyph
coverage -> R8 alpha bitmap
glyph atlas allocation
advance / bearing
真实 WGPU glyph texture
```

`ProjectRuntimePackageAssembler` 已是项目目录进入 RuntimePackageBuildInput 的唯一正式装配入口。

### 4.2 当前缺口

`rust/crates/editor_core/src/aui_font_atlas_cooker.rs` 当前：

```text
硬编码 font-main
硬编码 ui-default-cmin
硬编码 5x7 / 8px ASCII bitmap
collect_required_chars 只扫描 AUI 的 text 字段
normalize_char 把所有非 ASCII 字符转成 '?'
只检测项目字体描述存在状态，不解析真实 TTF / OTF
始终输出 engine_builtin_cooked_fallback
```

`rust/crates/engine_runtime/src/aui.rs` 当前：

```text
glyph 只按 codepoint 与单 atlas 查找
font scale 使用 requestedFontSize / 8.0
glyph draw item 不具备正式 font face / raster variant / atlas layer identity
```

`rust/crates/runtime_player_winit/src/lib.rs` 当前只上传和绑定默认单张 FontAtlas 纹理。

因此 261 不是重做 208 的 glyph-present 链路，而是替换 208 明确延期的完整 FontImporter、
fallback、multi-page、真实 CJK 和高质量 raster 能力。

## 5. 成熟引擎与工具研究

### 5.1 Unity TextMeshPro

参考：

```text
https://docs.unity.cn/Packages/com.unity.textmeshpro@3.2/manual/FontAssetsCreator.html
https://docs.unity.cn/Packages/com.unity.textmeshpro@3.2/manual/FontAssetsProperties.html
```

Unity Font Asset Creator 同时提供：

```text
SMOOTH / RASTER
SMOOTH_HINTED / RASTER_HINTED
SDF / SDFAA / SDF8 / SDF16 / SDF32
sampling point size
padding
character set
kerning pairs
fallback font assets
multi-atlas textures
```

可学习：

```text
字体源、FaceInfo、glyph table、atlas 和 material 是正式资产。
复杂或小字符需要更高采样质量。
fallback 和 multi-atlas 属于字体资产基础能力。
bitmap 与 distance field 是并存策略，不应强迫全部文本走同一种模式。
```

不照搬：

```text
不复制 TMP material hierarchy。
不复制 Unity Resources 路径规则。
不把动态 atlas 作为发布 Runtime 的默认真相。
```

### 5.2 Godot 4 FontFile / TextServer

参考：

```text
https://docs.godotengine.org/en/stable/classes/class_fontfile.html
https://docs.godotengine.org/en/stable/tutorials/ui/gui_using_fonts.html#msdf-font-rendering
https://github.com/godotengine/godot/blob/master/scene/resources/font.cpp
```

Godot `FontFile` 保存：

```text
font source data
size cache
texture pages
glyph texture index / UV
glyph advance / offset / size
kerning
fallback fonts
MSDF size / pixel range
```

Godot 官方明确说明：

```text
传统 grayscale raster 是默认路径。
MSDF 在巨大字号与缩放时保持清晰。
MSDF 基础成本更高，低端移动设备可能受影响。
MSDF 小字号因不能使用 Hinting，清晰度低于传统 raster。
自相交轮廓可能无法正确生成 MSDF。
```

这些证据直接支持 AutoHybrid，而不是纯 MSDF。

### 5.3 Unreal Slate / UMG

参考：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/font-asset-and-editor-in-unreal-engine
FSlateFontServices
FSlateFontCache
FShapedGlyphFontAtlasData
```

Unreal 的 Runtime Composite Font 路径以 Font Face、Hinting、fallback family 和按需 glyph cache
为核心；旧 Offline Font 使用预生成 atlas。

可学习：

```text
Face asset 与 family/composite font 分层。
Hinting 是 Face 的正式属性。
fallback 应按 family/style 匹配，不应只有全局 '?'。
glyph atlas 与 UI 业务分离。
```

不照搬：

```text
不引入 UObject / Slate font service hierarchy。
不把运行时 FreeType cache 作为发布真相。
```

### 5.4 Bevy

参考：

```text
https://github.com/bevyengine/bevy/blob/main/crates/bevy_text/src/font_atlas.rs
https://github.com/bevyengine/bevy/blob/main/crates/bevy_text/src/font_atlas_set.rs
```

Bevy `FontAtlasSet` 以 font data、font size、hinting、smoothing 等参数区分 atlas；glyph 加入失败时
可创建新 atlas page。

可学习：

```text
font + raster parameters 必须进入 glyph cache identity。
glyph metadata 必须明确指向具体 atlas image。
atlas 满后增加 page，而不是静默丢 glyph。
```

不照搬：

```text
不引入 Bevy RenderWorld / Assets 体系。
不在 Runtime 第一次遇到字符时修改 atlas。
```

### 5.5 msdfgen

参考：

```text
https://github.com/Chlumsky/msdfgen
```

关键事实：

```text
普通单通道 SDF 容易损失尖角。
MSDF 使用 RGB 三个距离通道恢复尖角。
fragment shader 采样后需要计算 RGB median。
MSDF 通道必须按 linear data 解释，不能使用 sRGB。
pixel range 必须进入生成和 shader 参数。
```

## 6. 方案比较与最终选择

### 6.1 G2-A：单字体单页 Hinting R8

优点：

```text
GPU 成本最低。
对固定小字号中文最清晰。
最接近当前 R8 实现。
```

问题：

```text
多字号重复 glyph，包体和显存随字号增长。
大比例缩放、动画和粗描边效果差。
不解决 family / stack / multi-page / per-node font。
```

结论：不足以作为长期基础。

### 6.2 G2-B：纯 MSDF

优点：

```text
一个 raster source 可覆盖大字号和连续缩放。
尖角、描边和特效优于普通 SDF。
减少多字号 bitmap 重复。
```

问题：

```text
小字号中文缺少 Hinting，笔画密集时清晰度下降。
RGBA8 page 比 R8 page 占用更高。
shader ALU 与带宽高于 bitmap。
低端移动设备基础成本更高。
```

结论：不作为全局唯一模式。

### 6.3 G2-C：完整 FontBundle + AutoHybrid

优点：

```text
小字号由 Hinting R8 保证清晰度。
大字号、缩放和描边由 MSDF 保证质量。
Face / Family / Stack / Profile / multi-page 一次形成长期合同。
项目作者只选择逻辑字体和样式，不接触 atlas page 或 shader。
```

代价：

```text
cooker、RuntimePackage、AUI、UiProjection 和 renderer 均需升级。
需要两种纹理格式和两条 text render pipeline。
施工必须分 Gate 关闭，不适合做单文件补丁。
```

最终选择：G2-C + AutoHybrid。

## 7. Module、Interface 与 seam

### 7.1 外部 seam

唯一正式 build/cook seam 仍位于 `ProjectRuntimePackageAssembler`。

外部调用形状：

```rust
pub struct ProjectFontCookRequest<'a> {
    pub project_root: &'a Path,
    pub asset_graph: &'a RuntimeAssetGraph,
    pub runtime_text_sources: &'a [ProjectTextSource],
    pub build_profile: &'a RuntimeBuildProfile,
}

pub struct ProjectFontCookOutput {
    pub bundle: RuntimePackageSourceFontBundle,
    pub report: ProjectFontCookReport,
}

pub struct ProjectFontCookModule;

impl ProjectFontCookModule {
    pub fn cook(
        request: ProjectFontCookRequest<'_>,
    ) -> Result<ProjectFontCookOutput, ProjectFontCookFailure>;
}
```

Assembler 不学习字体解析、fallback resolve、glyph raster、MSDF、packing 或 page 生成细节。

### 7.2 Module 深度

`ProjectFontCookModule` 隐藏：

```text
资产 schema 解析和引用解析
安全项目路径读取
font bytes hash
TTF / OTF / TTC face 解析
family/style/weight resolve
fallback stack resolve
项目文本收集
Unicode scalar 排序去重
glyph id resolve
Hinting bitmap raster
MSDF generation
deterministic page packing
metrics normalization
kerning extraction
cooked bundle 生成
diagnostic / report
```

删除该 Module 后，这些复杂度会重新扩散到 Assembler、AUI、Builder 和测试，因此该 Module
具有足够 Depth、Leverage 与 Locality。

### 7.3 不新增假 seam

首版只有一个生产字体 parser/raster implementation，不公开：

```text
FontBackend trait
Rasterizer provider port
AtlasPacker adapter registry
MSDF remote service
```

Implementation 内可以有私有函数和私有测试 seam，但公共 Interface 不暴露未来可能变化的
第三方库类型。

依赖分类：

```text
字体解析 / raster / MSDF / packing / hash：in-process dependency
项目字体文件：local-substitutable dependency
Runtime GPU upload：已有 RHI / render resource seam
不新增 remote dependency 或公共 Adapter
```

## 8. 项目字体资产合同

所有长期引用使用 `AssetRef` / stable asset id，不使用裸路径作为跨资产引用。

### 8.1 FontFaceAsset v2

一份物理字体 face：

```json
{
  "schemaVersion": "font-face-asset.v2",
  "assetId": "font-face-ui-regular",
  "source": {
    "kind": "projectFile",
    "assetRef": "asset-font-source-ui",
    "faceIndex": 0,
    "sourceSha256": "sha256:..."
  },
  "declared": {
    "family": "Project UI",
    "style": "normal",
    "weight": 400,
    "stretch": 100
  },
  "hinting": "fontDefault"
}
```

规则：

```text
支持 TTF、OTF、TTC/OTC faceIndex。
source 必须进入 Asset Graph 和 package dependency digest。
声明 metadata 与字体内部 metadata 不一致时 fail closed，不能悄悄换 face。
不允许 projectFile 指向项目根外或系统字体目录。
```

### 8.2 FontFamilyAsset v1

逻辑 family 负责 style/weight 到 face 的确定性映射：

```json
{
  "schemaVersion": "font-family-asset.v1",
  "assetId": "font-family-ui",
  "faces": [
    { "fontFace": "font-face-ui-regular", "style": "normal", "weight": 400 },
    { "fontFace": "font-face-ui-bold", "style": "normal", "weight": 700 }
  ],
  "missingStylePolicy": "nearestWeightSameStyle"
}
```

匹配顺序固定：

```text
exact style + exact weight
exact style + nearest weight
normal style + nearest weight
失败
```

同距离时使用较低 weight，再以 stable asset id 排序，禁止依赖 JSON map 顺序。

### 8.3 FontStackAsset v1

有序 fallback family：

```json
{
  "schemaVersion": "font-stack-asset.v1",
  "assetId": "font-stack-ui-default",
  "families": [
    "font-family-ui",
    "font-family-cjk-fallback"
  ],
  "missingGlyphPolicy": "error",
  "replacementCodepoint": "U+FFFD"
}
```

规则：

```text
family 顺序是正式语义，不排序。
每个 glyph 使用第一个包含该 glyph 的 resolved face。
fallback 命中不是错误，但必须报告来源。
missingGlyphPolicy=error 时 required glyph 缺失使 cook 失败。
replacement 只允许显式非 blocking profile；默认 UI 发布 profile 使用 error。
```

### 8.4 FontAtlasProfileAsset v1

定义 glyph source、raster policy 和 packing budget：

```json
{
  "schemaVersion": "font-atlas-profile-asset.v1",
  "assetId": "font-atlas-profile-ui-default",
  "role": "defaultUi",
  "fontStack": "font-stack-ui-default",
  "glyphSet": {
    "includeRuntimeTextSources": true,
    "unicodeRanges": ["U+0020-U+007E"],
    "literals": [],
    "locales": ["zh-CN"]
  },
  "raster": {
    "policy": "autoHybrid",
    "bitmapPixelSizes": [12, 14, 16, 18, 20, 24, 28, 32],
    "bitmapHinting": "fontDefault",
    "msdfEmSize": 64,
    "msdfPixelRange": 8
  },
  "packing": {
    "pageWidth": 2048,
    "pageHeight": 2048,
    "padding": 1,
    "maxBitmapPages": 16,
    "maxMsdfPages": 16
  }
}
```

`literals` 只用于真正无法进入项目文本资产的运行时生成字符，不要求策划复制全部中文文案。

## 9. glyph 集合与文本来源

### 9.1 正式收集范围

`includeRuntimeTextSources=true` 时收集：

```text
AUI text
AUI placeholder
binding fallback text
RuntimePackage source graph 中注册为用户可见文本的 Feature Asset 字段
本地化表的目标 locale 文本
FontAtlasProfile 显式 literals / Unicode ranges
```

不扫描：

```text
Design/
Build/
Library/
任意 Rust 源文件
日志
diagnostic 文本
资产 ID、hash、内部 schema key
系统字体目录
```

Gameplay Rust 中需要显示的中文必须来自正式 Project Asset / localization asset，不能依靠
扫描 `include_str!` 或源码字符串。

### 9.2 glyph identity

长期 cooked identity 不是单纯 codepoint：

```text
FontGlyphKey:
  font_face_id
  glyph_id
  raster_variant_id
```

另存：

```text
CodepointResolution:
  font_stack_id
  requested_style
  requested_weight
  codepoint
  resolved_font_face_id
  glyph_id
```

这样未来 shaping Module 可直接输出 glyph id，并继续消费 261 FontBundle，不需要推翻 atlas 合同。

### 9.3 确定性顺序

```text
profile 按 stable asset id 排序。
family face 按匹配规则和 stable asset id 决胜。
font stack 保留 authored order。
codepoint 按 Unicode scalar value 升序。
glyph key 按 font_face_id / glyph_id / raster_variant_id 排序。
packing 输入顺序固定，不依赖 HashMap 迭代顺序。
```

## 10. AutoHybrid 规则

### 10.1 physical pixel size

选择 render variant 使用实际 physical pixel size：

```text
physical_px =
  authored_font_size
  * UI scale
  * window DPI scale
  * node effective scale
```

不能只按 authored logical size 选择，否则高 DPI 和缩放节点会错误使用低分辨率 bitmap。

### 10.2 Bitmap 选择

```text
physical_px <= 32
且 effective scale 在稳定范围内
  -> 选择最接近的 Hinting R8 bitmap bucket

优先 exact bucket。
没有 exact bucket 时，允许在 [0.875, 1.125] 范围内缩放最近 bucket。
超出范围改用 MSDF，禁止继续放大低分辨率 bitmap。
```

Bitmap 支持 grayscale antialiasing 和 face 的 Hinting 策略。默认不启用 LCD subpixel，
避免不同显示器子像素排列和截图证据不稳定。

### 10.3 MSDF 选择

```text
physical_px > 32
或 bitmap bucket 缩放超界
或节点声明连续缩放用途
或 outline 超过 bitmap profile 能力
  -> MSDF
```

正式默认：

```text
msdfEmSize = 64
msdfPixelRange = 8
atlas format = RGBA8 linear
RGB 保存 multi-channel distance
A 保留为 1.0；v1 不启用 MTSDF
```

MSDF 生成失败、自相交轮廓不可恢复或 glyph 超出 profile 限制时必须给出结构化失败；
不能静默生成形状不同的 bitmap 并仍声称 MSDF 成功。项目可以显式把该 profile 改为
`hintedBitmap`。

### 10.4 render batching

Runtime 使用两组 texture array：

```text
BitmapGlyphPages: R8Unorm texture2DArray
MsdfGlyphPages: Rgba8Unorm texture2DArray
```

glyph draw vertex 至少携带：

```text
render_mode
atlas_layer
uv_rect
screen_rect
msdf_unit_range_or_variant_index
color
```

同一 AUI 顺序内按可合并的连续 run batching。不得为了减少 draw call 重排有遮挡关系的 UI。

## 11. CookedFontBundle v2

### 11.1 manifest

```text
RuntimeFontBundleManifest:
  schema_version
  default_ui_font_stack_id
  font_bundles[]
  source_digest
  glyph_set_digest
  cook_recipe_version
```

### 11.2 bundle

```text
CookedFontBundle:
  font_bundle_id
  font_stack_id
  faces[]
  families[]
  codepoint_resolutions[]
  glyph_variants[]
  kerning_adjustments[]
  bitmap_pages[]
  msdf_pages[]
  fallback_summary
  diagnostics_summary
```

### 11.3 face metrics

```text
CookedFontFaceMetrics:
  font_face_id
  units_per_em
  ascent
  descent
  line_gap
  underline_position
  underline_thickness
```

### 11.4 glyph variant

```text
CookedGlyphVariant:
  font_face_id
  glyph_id
  raster_variant_id
  render_mode
  nominal_pixel_size
  atlas_page_index
  atlas_array_layer
  uv_rect
  pixel_rect
  bearing_x
  bearing_y
  advance_x
  msdf_pixel_range
```

### 11.5 page

```text
CookedFontAtlasPage:
  page_id
  render_mode
  format
  width
  height
  byte_len
  content_sha256
  payload_path
```

所有 page 必须相同模式内维度一致，才能进入同一个 texture array。不同 render mode 使用不同 array。

## 12. AUI Interface

AUI 文本样式正式增加逻辑字体选择：

```json
{
  "fontStack": "font-stack-ui-default",
  "fontFamily": null,
  "fontWeight": 700,
  "fontStyle": "normal",
  "fontSize": 20,
  "pixelSnap": true
}
```

规则：

```text
fontStack 为空 -> 项目 defaultUi stack。
fontFamily 非空 -> 必须属于可达 FontBundle，作为首选 family；stack fallback 仍有效。
fontWeight / fontStyle 通过 FontFamilyAsset 确定性解析。
fontSize 是 logical px，variant 选择使用最终 physical px。
pixelSnap 只影响定位与 bitmap preference，不改变字体 metrics。
AUI 不暴露 atlas page、texture handle、glyph id 或 shader 参数。
```

AUI binding 仍只提供最终字符串，不读取字体资产。字体解析属于 text presentation，不进入
ProjectUiStateSnapshotProducer。

## 13. Runtime 与 UiProjection

### 13.1 RuntimeFontRegistry

Runtime load 后建立只读 registry：

```text
font stack -> family list
family + style + weight -> face
codepoint / glyph id -> cooked glyph variants
render mode + page -> texture array layer
kerning pair -> adjustment
```

非法 duplicate、missing default、越界 rect、错误 byte_len、未知 render mode 或 digest 不一致
必须在 package load 阶段失败，不能延迟到 draw。

### 13.2 glyph planning

```text
resolved text run
  -> resolve font stack / family / face
  -> codepoint mapping or shaped glyph id
  -> select raster variant from physical px
  -> apply normalized metrics / kerning
  -> create glyph draw items
  -> UiProjection
```

### 13.3 renderer ownership

```text
AUI layout/presenter：
  文本、字体逻辑选择、metrics、glyph positions

UiProjection：
  render-facing glyph items、render mode、array layer

RuntimeRenderer：
  pipeline selection、texture array binding、draw

RHI：
  texture/buffer/pipeline resource
```

禁止 AUI 或项目逻辑直接持有 GPU handle。

## 14. 热更新与 generation

261 的热更新是 cooked asset generation 替换，不是 Runtime 动态 raster：

```text
字体 bytes 改变
FontFace / Family / Stack / Profile 改变
可达文本新增 codepoint
locale glyph set 改变
cook recipe version 改变
  -> 只使受影响 FontBundle dependency digest 失效
  -> ProjectFontCookModule 重新 cook
  -> 生成新 FontBundle generation
  -> Play/Preview 在 frame boundary 原子替换 registry 和 texture arrays
```

仅修改文本但所有 glyph 已存在时，允许复用已有 FontBundle generation。

替换规则：

```text
新 generation 完整 load / validate / GPU prepare 成功前，旧 generation 保持 active。
切换在 frame boundary 完成。
旧 generation 在无 in-flight frame 引用后释放。
失败时保留旧 generation，并输出新 generation 的 cook/load diagnostic。
禁止半替换 metadata 与 page payload。
```

## 15. 确定性与 digest

输入 digest 至少覆盖：

```text
font source raw bytes
canonical FontFace / Family / Stack / Profile
resolved runtime text source identities and contents
sorted codepoint set
resolved glyph id set
parser/raster/MSDF/packer version
all raster parameters
page size / limits
cook recipe version
```

输出 digest 覆盖：

```text
canonical metadata bytes
all page payload bytes
page order
glyph variant order
codepoint resolution order
kerning order
```

相同输入连续 cook 两次必须满足：

```text
metadata byte-identical
page payload byte-identical
page content hash identical
FontBundle digest identical
RuntimePackage assembly input digest identical
```

不得把本机路径、时间戳、随机 seed、HashMap 顺序或系统字体版本写入确定性结果。

## 16. 失败策略与 diagnostics

### 16.1 build/cook 必须失败

```text
FontFace schema 非法
AssetRef 缺失或路径越界
source hash 不匹配
TTF / OTF / TTC 无法解析
faceIndex 越界
声明 family/style/weight 与字体 face 冲突
family 或 stack 引用循环
defaultUi stack 缺失或重复
required glyph 在完整 stack 中缺失
bitmap 或 MSDF raster 失败
MSDF shape 不可生成
单页 glyph 不可容纳
page 数超过 profile budget
重复 glyph key
metrics 非有限数
payload byte_len / hash 不一致
```

### 16.2 禁止静默成功

一旦项目使用正式 `font-face-asset.v2`：

```text
禁止回退系统字体。
禁止把中文替换为 '?' 后仍返回 passed。
禁止 atlas overflow 后丢弃 glyph。
禁止 MSDF 失败后悄悄声称使用 MSDF。
```

旧 `font-asset.v1 builtinFallback` 只能走 legacy compatibility adapter，并必须报告：

```text
legacy_mode=true
fallback_used=true
quality_gate_eligible=false
```

### 16.3 diagnostic 结构

```text
code
domain=font
stage=asset_resolve|parse|glyph_resolve|raster|pack|package|load|gpu_prepare|present
font_face_id
font_family_id
font_stack_id
font_atlas_profile_id
codepoint
glyph_id
render_mode
source_asset_ref
message
next_action
```

建议错误码：

```text
FontAssetParseFailed
FontSourcePathInvalid
FontSourceHashMismatch
FontFaceIndexOutOfRange
FontFamilyResolutionFailed
FontStackCycle
RequiredGlyphMissing
BitmapRasterFailed
MsdfGenerationFailed
FontAtlasPageOverflow
FontAtlasPageBudgetExceeded
CookedFontBundleInvalid
FontPageUploadFailed
FontRenderVariantUnavailable
```

## 17. report 分档

遵守 Off / Summary / Trace：

```text
Runtime Off：
  不生成字体 report 文件，只保留功能必需状态。

Runtime Summary：
  active bundle、glyph rendered/missing/fallback count、render mode count、compact diagnostics。

Runtime Trace：
  测试或显式诊断使用；包含 run resolve、variant selection、page/layer 和失败来源。

Editor Summary：
  cook status、bundle/profile、glyph/page count、fallback count、digest、next action。

Editor Trace：
  完整 source/glyph/packer evidence，可写结构化 report。
```

不得在正式 runtime 热路径每帧序列化完整 glyph trace。

## 18. 性能和资源预算

### 18.1 GPU page 成本

`2048 x 2048`：

```text
R8Unorm：约 4 MiB / layer
Rgba8Unorm MSDF：约 16 MiB / layer
```

MSDF 使用 RGBA8 是因为 wgpu 没有适合作为通用采样 texture 的 RGB8 格式。

### 18.2 shader 成本

Bitmap pipeline：

```text
一次 texture sample
alpha coverage
颜色混合
```

MSDF pipeline：

```text
一次 RGBA texture sample
RGB median
screen distance / smooth edge reconstruction
颜色混合
```

因此原始 fragment 成本 Bitmap 最低。AutoHybrid 只在大字号或缩放确有收益时选择 MSDF。

### 18.3 draw 与 binding

```text
同 render mode 的 pages 进入 texture2DArray。
page index 作为 array layer 随 glyph 传入。
不因每个 page 单独创建 draw call。
Bitmap/MSDF pipeline 切换形成必要 batch split。
fallback font 不应自动等于额外 draw call；只要 format/pipeline 相同，可继续使用同 texture array。
```

### 18.4 cooker

Font cook 不在 runtime frame 热路径。构建期允许较高成本，但必须：

```text
按 dependency digest 增量失效。
同一 source face 解析结果在一次 cook 内复用。
相同 glyph variant 不重复 raster。
MSDF generation 可并行，但最终 merge/pack 顺序必须确定。
```

## 19. 兼容与迁移

### 19.1 208 C-min

```text
208 的 AUI -> RuntimePackage -> glyph plan -> renderer 证据链继续保留。
261 替换 AuiFontAtlasCookerCmin 的正式生产职责。
208 C-min metadata/loader 作为 legacy read adapter 保留一段迁移期。
新 builder 对正式 font assets 只写 CookedFontBundle v2。
```

### 19.2 历史项目

Complex Shooter、Switch Puzzle 等旧项目可继续使用 legacy builtin fallback，但：

```text
不得通过 261 Font Quality Gate。
Editor 必须提示迁移到 FontFace / Family / Stack / Profile。
迁移由受控 Asset/Patch workflow 生成，用户不手写 cooked bundle。
```

### 19.3 塔防项目

塔防项目后续只作为正常引擎用户：

```text
导入有明确授权的中文字体源资产。
创建 UI family / default stack / atlas profile。
AUI 选择逻辑字体与样式。
文案继续位于项目 AUI、Feature Asset 或 localization asset。
```

塔防项目不得修改 261 引擎 schema、cooker、Runtime 或 renderer。

## 20. 与复杂文本系统的边界

261 本轮完成字体资产与 glyph rendering 基础，但不把其它系统伪装成字体子功能：

```text
IME / caret / composition：Text Input 系统
rich text markup / inline object：Rich Text Document 系统
Bidi paragraph resolve：Text Layout 系统
HarfBuzz 等复杂 script shaping：Text Shaping Module
emoji color layers / SVG glyph：Color Glyph / Inline Image 系统
```

261 必须为未来 shaping 保持稳定的 `font_face_id + glyph_id + raster_variant` 消费合同；
未来 shaping Module 接入时不得再次修改 FontFace、FontFamily、FontStack、page 或 glyph variant
的基本 identity。

本轮基础 kerning pair 属于简单 LTR text layout 的必要能力，不等同于完整 shaping。

## 21. 验收矩阵

### 21.1 Asset 与解析

```text
真实 TTF、OTF、TTC faceIndex 正向 fixture。
路径逃逸、缺失 source、hash mismatch、非法 faceIndex 否定 fixture。
family exact/nearest weight resolve。
stack fallback 顺序与 cycle 检查。
```

### 21.2 glyph 与 cooker

```text
中文、Latin、数字和标点进入真实 glyph set。
动态 UI 文本通过注册 Project Text Source 进入 glyph set。
required glyph 缺失稳定失败，不生成 '?' 成功产物。
同输入双 cook byte-for-byte identical。
输入文字、字体 bytes、profile 或 recipe 改变会改变正确 digest。
```

### 21.3 AutoHybrid

```text
<=32 physical px 选择 Hinting R8。
>32 physical px 选择 MSDF。
bitmap scale 超过 [0.875, 1.125] 时选择 MSDF。
不同 DPI 和 node scale 使用 physical px 决策。
Bitmap/MSDF 切换不改变 layout metrics 和换行。
普通单通道 SDF 不进入正式输出。
```

### 21.4 multi-page / GPU

```text
Bitmap 与 MSDF 各自可生成多 page。
page budget overflow 稳定失败。
Runtime load 验证 page format、size、byte_len 和 hash。
R8 与 RGBA8 texture array 正确建立。
glyph array layer 正确。
多页文本不因 page 切换缺字或改变 UI 顺序。
```

### 21.5 AUI

```text
default stack。
per-node stack / family / style / weight。
fallback face 命中可见并有 report。
中文 rendered == requested、missing == 0。
Editor Preview、Play、Export 使用同一 CookedFontBundle digest。
```

### 21.6 热更新

```text
已有 glyph 的文案变化不强制重新 cook。
新增 codepoint 只失效受影响 profile。
新 generation 未 prepare 成功前旧 generation 保持 active。
frame boundary 原子切换。
旧 GPU generation 在无引用后释放。
```

### 21.7 架构

```text
Runtime 不依赖 font parser / MSDF generator。
Runtime 不扫描系统字体或项目源目录。
项目逻辑不持有 glyph/page/GPU handle。
Assembler 只有一个 ProjectFontCookModule 调用 seam。
塔防语义不进入引擎 API、schema、fixture 或 report。
```

### 21.8 真实视觉证据

至少需要：

```text
小字号中文 Hinting R8 screenshot。
大字号中文 MSDF screenshot。
同一文本 Bitmap/MSDF metrics overlay 对齐证据。
缩放动画或多 DPI 下 MSDF 清晰证据。
多页 fallback 字体同屏证据。
真实 exported Windows Player 证据。
```

像素证据必须来自生产 composition，不得使用 debug overlay 或测试专用字体旁路冒充。

## 22. 建议施工窗口

本文不是施工文档。后续施工文档建议按以下窗口拆分，但必须另行生成、自审和激活：

```text
Window 1：FontFace / Family / Stack / Profile schema、Asset Graph、diagnostics
Window 2：ProjectFontCookModule、真实字体解析、glyph collection、Hinting R8
Window 3：CookedFontBundle v2、multi-page、Builder/Loader、legacy adapter
Window 4：MSDF generator、deterministic recipe、AutoHybrid variant selection
Window 5：AUI font style、RuntimeFontRegistry、metrics/kerning/glyph planning
Window 6：UiProjection、R8/MSDF texture arrays、两条 text pipeline
Window 7：hot reload generation、Editor Preview/Play/Export parity
Window 8：negative matrix、真实视觉证据、第二项目与完整回归
```

施工不得把 Window 1-3 完成描述为整个 261 完成。只有 Window 1-8、受影响回归、生产窗口证据、
完成记录和入口同步全部闭环后，261 才能标记施工完成。

## 23. 风险与控制

### 风险 1：范围较大

控制：

```text
保持一个深 ProjectFontCookModule Interface。
施工分窗口，但正式合同一次冻结。
不把 rich text / IME / Bidi / shaping implementation 混入 261。
```

### 风险 2：MSDF 第三方依赖与确定性

控制：

```text
固定依赖版本和 recipe version。
固定 edge coloring、em size、pixel range、quantization 和 pack 顺序。
建立 byte-identical 双 cook Gate。
```

### 风险 3：中文 glyph 数导致 page 膨胀

控制：

```text
只 cook RuntimePackage 可达文本、locale 和显式 range。
不默认烘焙完整 CJK Unified Ideographs。
结构化报告 glyph/page/bytes 和 budget。
budget 超限 fail closed，并给出拆 locale/profile 的 next action。
```

### 风险 4：两条 pipeline 造成布局差异

控制：

```text
metrics 与 raster variant 解耦。
布局只消费规范化字体 metrics。
视觉 Gate 对比 Bitmap/MSDF glyph origin、baseline、advance 和换行。
```

### 风险 5：legacy fallback 掩盖失败

控制：

```text
正式 font-face.v2 profile 绝不静默进入 legacy fallback。
legacy_mode 明确报告，不能通过 261 quality gate。
```

## 24. 方案自审

### 24.1 是否修改塔防项目专用逻辑

否。261 只定义项目无关字体能力。塔防项目仍通过公开 Project Asset、AUI 和 RuntimePackage
Interface 使用它。

### 24.2 是否推翻已有主线

否。

```text
继承 208 的 cooked RuntimePackage 方向。
继承 210 的 UI composition。
继承 236 的确定性 digest 与 save/rebuild consistency。
继承 AssetRef / Asset Graph 和 RHI resource discipline。
```

261 替换的是 208 明确标注为 C-min 与 deferred 的字体质量实现，不建立旁路。

### 24.3 Interface 是否过浅

否。Assembler 只学习一个 `ProjectFontCookModule::cook` Interface；字体解析、fallback、
raster、MSDF、packing、metrics、digest 和 report 都隐藏在 Module 内。

### 24.4 是否建立假 Adapter

否。字体解析/raster implementation 为内部依赖，不公开只有一个实现的 provider seam。
已有 RHI / Asset Graph 是真实 seam，继续复用。

### 24.5 是否解决“字体以后再补”

是。以下长期合同在 261 一次形成：

```text
Face / Family / Stack / Profile
style / weight
fallback
multi-size / multi-page
Bitmap / MSDF
glyph-id identity
metrics / kerning
AUI font selection
Runtime texture arrays
hot reload generation
deterministic cook / diagnostics / reports
```

未来 shaping、Bidi、IME、rich text 和 color emoji 属于独立文本系统；它们可以消费 261 的
font face / glyph id / FontBundle，不要求再次推翻字体基础资产合同。

### 24.6 是否具备可施工和可验收性

是。方案给出：

```text
明确资产 schema
明确 cook/load/runtime Interface
明确 AutoHybrid 阈值与格式
明确 failure matrix
明确性能预算
明确 hot reload generation
明确自动化和真实窗口证据
明确八个建议施工窗口
```

### 24.7 自审结论

```text
方案结论：通过
需要修改正式方案：无
允许生成施工文档：需要用户后续明确要求
当前允许施工：否
```
