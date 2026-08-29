# 302 AUI FontBundle MSDF Orientation + Physical-Pixel AutoHybrid B-min v1 方案

## 1. 文档状态

```text
系统编号：302
方案版本：v1
建立日期：2026-08-19
问题来源：Tower Windows Player 中 FPS 文本上下倒置，且大量中文 UI 文本仍不清晰
讨论方案：A 项目侧临时规避；B-min 修复既有 FontBundle 合同；C 完整字体能力扩张
用户选择：B-min
当前状态：正式方案已生成并自审；尚未生成施工文档，未授权施工
```

本文档只固化 261 FontBundle / AutoHybrid 已有合同的最小修复，以及 Tower 作为外部项目 consumer
的正确字体接线。本文档不构成源码修改、测试、构建、production Editor/Player 替换、Tower Preview
cache 重建、Windows dev 重新导出、真实配置修改或 Local CI 授权。

## 2. 一句话目的

让 MSDF 与 Bitmap 使用相同的图像方向约定，并让 Runtime AUI 按真实 target 物理像素选择字体变体；
Tower 同时使用真实 Regular 400 字体，使 720x1280 与 1080x1920 中的 FPS 和中文 UI 均方向正确、
采样清晰、布局不漂移。

## 3. 已确认现象与证据

### 3.1 FPS producer 正常，缺陷在字体 present

Tower FPS sampler 会按 0.25 秒窗口更新数字，`tower.performance.fps_text` 也能正确进入 UI snapshot。
FPS AUI 节点当前为：

```text
nodeId = performance-fps
fontSize = 36 logical px
rasterPolicy = autoHybrid
fontWeight = 400
```

因此“数值不更新”和“字符串生产错误”不是本次上下倒置的首因。真实 Player 截图与 atlas 抽样证明：

```text
Bitmap glyph F：方向正确
MSDF glyph F：上下倒置
Runtime Bitmap/MSDF：使用同一套 UV 约定
```

### 3.2 MSDF cook 缺少 Y-up 到 image-row Y-down 的转换

`rust/crates/editor_core/src/project_font_cook.rs` 的 `raster_msdf_variants` 将字体 outline 的 Y-up
坐标直接写入 image buffer 的 Y-down row 顺序。生成后的 RGBA8 glyph 在进入 atlas 前已经倒置。

`rust/crates/engine_runtime/src/runtime_renderer.rs` 对 Bitmap 与 MSDF 使用相同 quad UV 顺序；Bitmap
方向正确，因此 Runtime 不应增加 MSDF 专用 UV 翻转。方向修复 owner 必须位于 MSDF cook 输出规范化。

### 3.3 Runtime 未执行 261 的 physical-pixel AutoHybrid 合同

261 已正式规定：

```text
physical_px = authored_font_size * UI target scale * DPI scale * node effective scale
<= 32 physical px：优先 Hinting R8 Bitmap
> 32 physical px：选择 MSDF
Bitmap bucket scale 超出 [0.875, 1.125]：选择 MSDF
```

`editor_core::project_font_bundle::select_auto_hybrid` 已表达这套规则，但 production Runtime AUI
`build_text_glyph_plan_from_bundles` 仍只使用 authored logical `font_size`：

```text
font_size > 32 -> MSDF
否则 -> Bitmap
requested_pixel_size = authored font_size
```

Tower 的 reference canvas 为 1080x1920，Windows Player target 为 720x1280，canvas-to-target scale 为
`2/3`。例如 24 logical px 实际只有 16 target px；当前却请求 24px Bitmap，再把整张 UI 缩至 16px，
破坏 hinted bitmap 的 1:1 采样条件。

当前 Tower 可见文字约 282 个节点，中位 authored 字号为 24px；在 720 target 中中位物理字号约
16px。该缺口会系统性影响大部分中文 UI，而不是只影响 FPS。

### 3.4 Tower 字体粗细身份不真实

Tower AUI 普遍请求 `weight=400`，但：

```text
Assets/Fonts/font-face-tower-ui.json declared weight = 100
Assets/Fonts/font-family-tower-ui.json face weight = 100
source = NotoSansSC-VF.ttf
```

现有 cook 不应用 variable-font `wght` axis。将 metadata 从 100 改成 400 并不会把实际 raster 变成
Regular 400，只会制造错误身份。271 已提供静态 `NotoSansSC-Regular.ttf` 与 sealed Regular 400
FontBundle，可作为正确项目默认字体来源。

## 4. 既有架构与成熟实现参考

302 不建立新字体系统，继承以下正式方案：

```text
261：Project Font Asset / FontBundle v2 / Hinting R8 / MSDF / AutoHybrid
270：Project FontBundle 内容寻址 producer artifact cache
271：Engine Built-in Chinese Default FontPack，静态 Noto Sans SC Regular 400
273：GameView target 与 AUI input 共用 resolved presentation
295：Windows Player 持续逐帧竖屏 presentation
300：AUI snapshot conditional resolve 与 present cache
```

261/271 已研究并采纳成熟实现中的以下边界：

```text
FreeType glyph metrics：字体 outline/metrics 使用基线与 Y-up 坐标，raster image row 需要显式约定。
Bevy FontAtlasSet：atlas identity 必须包含字体、字号、variation/hinting/smoothing 等 raster 身份。
Godot Font/FontFile：字体源、fallback、hinting、MSDF 与 cache invalidation 分层管理。
```

可学习点是让 raster identity、物理字号和 cache identity 一致。不可照搬的是运行时动态字体 raster、
系统字体扫描或无界 atlas 增长；本引擎 Runtime 仍只读取确定性 RuntimePackage FontBundle。

302 不重复引入新的外部架构依赖，继续以 261/271 已固定的官方文档与源码参考为依据。

## 5. 方案比较与正式选择

### 5.1 方案 A：Tower 项目侧临时规避

```text
FPS 强制 Bitmap；继续提高字号/对比度；Tower 换 Regular 字体。
```

优点是改动最小。缺点是 MSDF atlas 仍然倒置，其他项目和 1080 大字号仍会复现；Runtime 仍违反
physical-pixel 合同。只能作为应急回退，不作为正式完成。

### 5.2 方案 B-min：修复既有合同

```text
修正 MSDF cooked row orientation；
Runtime glyph planning 消费共享 presentation 的 canvas-to-target scale；
按 target physical px 选择 Bitmap bucket 或 MSDF；
Tower 改用真实静态 Regular 400 字体；
补最小 owner/consumer/真实视觉证据。
```

优点是修复首因、复用既有 schema 和 Renderer，不引入第二套字体链。正式采用。

### 5.3 方案 C：完整 typography 扩张

```text
variable-font axis schema/cook、更多 hinting/oversampling/subpixel 控制、复杂 shaping/layout 扩张。
```

这些能力有长期价值，但与当前两个缺陷无直接必要关系，施工明显过量。本轮冻结。

## 6. 范围与所有权

### 6.1 引擎侧必须完成

```text
MSDF glyph cooked output 的垂直方向规范化；
Runtime AUI physical-pixel raster decision；
按 physical px 请求最接近的 Bitmap bucket；
glyph plan identity 纳入实际 raster decision/presentation identity；
Editor 与 Player 使用同一个 ResolvedGameViewPresentation 来源；
结构化选择摘要与定向回归。
```

这些目标均为 `engine-owned`。生成施工文档或修改源码前，需要用户单独明确授权引擎施工。

### 6.2 Tower 项目侧必须完成

```text
DefaultUi 不再声明实际不存在的 variable weight；
改用真实静态 Regular 400 FontBundle，优先复用 271 已存在的内置默认能力；
FPS 文本规范为 `FPS: --` / `FPS: 60`；
generator 与生成 AUI 保持单一 owner；
更新 FontBundleV2/Tower UI validator，移除历史 MSDF 方向规避断言。
```

Tower 文件仍只位于 `samples/tower_defense_project/**`，项目语义不得进入引擎 test fixture、diagnostic
或公共 API。

### 6.3 明确不做

```text
不新增或升级 AUI/FontBundle schema；
不实现 variable-font `wght` axis；
不更换全局 Bitmap nearestClamp / MSDF linearClamp sampler；
不增加 Runtime TTF parsing 或动态 glyph raster；
不建立第二套 glyph cache、字体 manager 或 renderer pass；
不调整 Tower 布局、玩法、输入、动画或 snapshot producer；
不把极小 Editor Fit 预览承诺为等同 720 target 的最终清晰度；
不自动运行 Local CI、production replacement、cache rebuild 或重新导出 Player。
```

## 7. MSDF 方向合同

FontBundle page 的统一约定为：

```text
page origin = top-left
row index increases downward
UV convention = BitmapR8 与 MsdfRgba8 完全相同
glyph bearing/advance = 字体 logical metrics，不因 row conversion 改变
```

`ProjectFontCookModule` 在每个 MSDF glyph variant 写入 `rgba8` 前执行且只执行一次 Y 方向规范化。
推荐最小实现是按完整 row 反序写出 RGBA8，不修改 Runtime quad UV；也允许等价的生成坐标变换，但必须
证明不会重复翻转、不会改变 sign correction、bearing、advance 或 atlas packing identity。

定向 owner 测试必须使用 `F/R/P` 等上下不对称 glyph，不能只用 `O/口/中` 等镜像后仍难以判断的字符。
测试比较语义区域或固定 fixture，而不是只断言 payload 非空。

修复会改变 MSDF page bytes、page hash、bundle digest 和所有包含该 bundle 的 RuntimePackage identity；
Bitmap page bytes 不得变化。

## 8. Physical-Pixel AutoHybrid 合同

### 8.1 唯一 scale 来源

glyph planning 不自行重复计算 GameView scale。调用方必须从 Renderer/Input 已共享的
`ResolvedGameViewPresentation` 派生每个 canvas 的 reference-to-target scale，并通过窄的
presentation context 交给 AUI glyph planner。

```text
target_physical_px = authored_font_size
                   * canvas_reference_to_target_scale
                   * existing_node_effective_scale
```

本轮选择的是实际渲染 target 像素，不把 Editor dock 中 target texture 的二次显示缩放冒充字体 raster
尺寸。720 Player 中 target 与窗口内容一致；极小 Editor Fit 的最终缩图清晰度属于独立 display-quality
问题。

### 8.2 Bitmap bucket 与布局分离

选择 Bitmap 时请求最接近 `target_physical_px` 的已 cook bucket：

```text
24 logical px * 2/3 target scale = 16 physical px
选择 16px Bitmap glyph
glyph local geometry scale = 24 / 16 = 1.5
presentation scale = 2/3
最终 atlas texel : target pixel = 1 : 1
```

layout advance、baseline、kerning 和换行继续使用 authored logical `font_size` 与 normalized metrics；
不能因为选择 16px bucket 把逻辑布局缩成 16px。

### 8.3 Render mode 决策

```text
rasterPolicy=bitmap：只选 Bitmap；缺失时 typed diagnostic，不静默改 MSDF。
rasterPolicy=msdf：只选 MSDF。
rasterPolicy=autoHybrid：复用 261 的 <=32 physical px 与 bitmap scale band 合同。
```

对于 AutoHybrid：

```text
physical_px <= 32 且最近 Bitmap bucket scale 在 [0.875, 1.125] -> BitmapR8
physical_px > 32 -> MsdfRgba8
最近 Bitmap bucket scale 超界、连续缩放或粗 outline -> MsdfRgba8
```

当前 AUI 没有独立 authored node transform schema 时，`existing_node_effective_scale` 为 1.0；302 不为
未来节点缩放预建新 schema。已有 control feedback 的瞬态 scale 不重新生产 FontBundle，也不在 hover
每帧切换 raster variant。

## 9. Present cache 与 identity

字体变体选择依赖 target presentation，不能只由 snapshot revision 决定。现有 AUI composite present
identity 必须把实际影响 glyph plan 的 presentation identity 纳入失效条件：

```text
snapshot unchanged + target/font generation unchanged -> reuse last present
target scale or font generation changed -> 使用 cached snapshot 重建 glyph plan/present 一次
ordinary clean frame -> 不重复 glyph planning
```

302 不修改 300 的 ProjectUiStateSnapshot production；presentation 变化只使文字 present 失效，不推进
Tower project UI visible revision，也不重新跨 ABI 生成全部 binding values。

## 10. Tower Regular 400 consumer

Tower 不得继续将 variable font 的默认/Thin raster 声明为 Regular 400。正式优先级为：

```text
1. 若 Tower 无不可替代的项目字体风格要求，移除自定义 DefaultUi override，使用 271 built-in
   `aife-default-zh-cn-common-v1` static Regular 400。
2. 若项目必须显式拥有字体资产，则引用 repo-owned static NotoSansSC-Regular.ttf，并准确声明
   family/style/weight/source hash；仍走 270 project font cache。
```

施工前必须先用项目 glyph coverage 检查确认 271 pack 覆盖 Tower 所有 required codepoint。缺字时不得
静默 fallback 或临时伪造 weight；应选择第 2 条显式 project Regular face，且不扩张到 glyph shard 系统。

FPS 文案统一为：

```text
未采样：FPS: --
已采样：FPS: 60
```

字号继续保持 36 logical px 与现有高对比底板。颜色、位置和对比度只按用户已确认 UI 视觉要求调整，
不能用强制 `bitmap` 长期掩盖 AutoHybrid 修复。

## 11. Diagnostics 与报告

遵守 Off / Summary / Trace。普通 Runtime Off 不写逐帧文件，也不构造完整 glyph 明细。

Summary 最多提供常量规模聚合：

```text
fontBundleId
targetPresentationIdentity
bitmapGlyphCount
msdfGlyphCount
physicalPixelRange
bitmapBucketRange
fallbackUsed
unsupportedGlyphCount
```

Trace 才允许每节点记录：

```text
nodeId / authoredPx / physicalPx / policy / chosenMode / bucket / reason
```

建议 typed diagnostics：

```text
aui.font.presentation_context_missing
aui.font.bitmap_bucket_outside_scale_band
aui.font.requested_raster_mode_missing
aui.font.face_weight_unavailable
font_cook.msdf_orientation_fixture_failed
```

普通 clean frame 不得新增 report 写盘、字符串详单或每 glyph allocation。

## 12. 最小验证合同

### 12.1 Engine owner

```text
MSDF asymmetric glyph orientation：F/R/P 方向正确，Bitmap 不变；
同输入双 cook：MSDF page byte-identical；
720 target：24 logical -> 16 Bitmap，36 logical -> 24 Bitmap；
1080 target：24 logical -> 24 Bitmap，36 logical -> MSDF；
bitmap bucket scale band 边界与 explicit bitmap/msdf policy；
layout metrics/kerning/wrap 在 Bitmap/MSDF 切换前后不变。
```

### 12.2 Shared consumers

```text
Editor 与 Player 从同一 ResolvedGameViewPresentation 得到相同 target raster decision；
presentation identity 不变时 clean frame glyph plan cache hit；
target 720 <-> 1080 变化时仅重建一次 present；
Bitmap/MSDF GPU batch 均有真实 glyph，非占位矩形。
```

### 12.3 Tower consumer

```text
所有 AUI requested weight 与 cooked face identity 一致；
required CJK/Latin/digit/punctuation coverage 无缺字；
FPS generator/runtime 文案为 `FPS: N`；
历史“多字符必须避开 MSDF”断言改为方向正确的正向能力断言；
generator drift check 与项目 FontBundleV2 consumer 通过。
```

### 12.4 最小真实视觉

施工完成后只要求与改动范围匹配的真实证据：

```text
普通 Editor：720x1280 target，FPS 与代表性中文可读；
Windows Player：720x1280 与 1080x1920 各一张稳定帧；
FPS 数字至少跨两个样本变化且方向保持正确；
同屏包含小字号 Bitmap 与大字号 MSDF；
无 glyph 倒置、缺字、baseline 跳变或布局漂移。
```

不自动要求完整 E2E、Local CI、全项目视觉矩阵、production transaction 或真实配置修改。若后续用户
单独授权 production 更新或重新导出，再执行对应一致性步骤。

## 13. 建议施工窗口

本文不是施工文档。后续极简施工文档最多拆为两个窗口：

```text
Window A / Gate A-B：
  MSDF row orientation owner repair + asymmetric fixture；
  physical-pixel glyph selection + shared presentation context +定向回归。

Window B / Gate C：
  Tower static Regular 400 consumer、FPS 文案/validator；
  最小 Editor/Player 视觉证据、完成记录和归档。
```

禁止把以下事项夹带进 302：

```text
variable-font axis、shaping、subpixel、glyph shard、MSDF 并行化；
字体 schema v3、Renderer rewrite、AUI layout rewrite；
production Editor/Player replacement；
Tower 完整视觉矩阵或 Local CI。
```

## 14. 风险与控制

### 风险 1：cook 与 Runtime 同时翻转

控制：只允许 cook 输出规范化；Runtime UV 与 Bitmap 保持一致，并用不对称 glyph 证明只翻转一次。

### 风险 2：物理字号修复引起布局变化

控制：raster bucket 只影响采样，advance/baseline/kerning/wrap 始终使用 logical metrics。

### 风险 3：target scale 变化导致 clean-frame 反复重建

控制：以稳定 presentation identity 失效一次；ordinary frame 禁止使用浮点噪声或窗口 timestamp 作为 identity。

### 风险 4：内置 Regular pack 缺少 Tower 字符

控制：施工前先做 required codepoint coverage；不满足时改用显式 static Regular project face，不扩大 271 glyph set。

### 风险 5：把极小 Editor dock 预览误当 target 字体缺陷

控制：资格证据以 target texture 与真实 Player 为准；极小 Fit display 若仍模糊，另立 display downsample
诊断，不继续修改 FontBundle。

## 15. 方案自审

### 15.1 是否直接解决两个已确认缺陷

是。MSDF cook 方向修复解决 FPS 倒置；physical-pixel variant selection 与真实 Regular 400 consumer
共同解决 720 target 的主要文字模糊。

### 15.2 是否推翻 261/271

否。302 让 production Runtime 真正执行 261 已有合同，并复用 271 已有静态 Regular 400 产物。

### 15.3 是否过量施工

否。方案明确排除 variable-font axis、schema 升级、sampler 全局更改、Renderer/AUI 重构、动态 raster、
glyph shard 与 Local CI。施工建议只有三个 Gate。

### 15.4 是否保持项目与引擎边界

是。方向和 physical-pixel selection 属于项目无关引擎能力；Tower 只负责字体选择、文案和项目 validator。
Tower 语义不进入引擎 Core。

### 15.5 是否具备可验证性

是。F/R/P 方向、720/1080 决策、bucket scale、metrics 稳定、Regular weight identity 和真实视觉均有能
捕获失败的定向证据；不依赖“看起来更清楚”的模糊判断。

### 15.6 权限与下一步

```text
方案结论：通过
正式方案：已生成并自审
施工文档：未生成
当前施工授权：无
引擎源码修改授权：需要用户后续明确给出
当前施工槽：空
下一步：用户明确要求后，单独生成并自审 302 极简施工文档
```
