# 345 AUI 完整文本整形与 GlyphRun 字体方案 C v1

## 1. 文档状态

```text
系统编号：345
方案版本：v1
用户决定：2026-09-15 选择方案 C
目标：为 AUI 与项目运行时 UI 增加完整文本 shaping，避免逐字符排版反复修补
前置系统：261 Project FontBundle、302 MSDF/AutoHybrid、340 AUI 字体渲染保真修复
范围：Project AUI runtime text；Editor FontSystem 不在本轮迁移
状态：正式方案已确认，等待施工文档与独立施工授权
```

## 2. 为什么需要本方案

当前 AUI 已有 FontFace、FontFamily、FontStack、FontBundle、Bitmap/MSDF、bearing、advance、kerning 和 glyph atlas，但排版路径仍是：遍历 Unicode 字符、逐个 resolve glyph、累加 advance、生成 quad。它缺少 shaping、glyph cluster、统一基线、fallback run 和行布局，因此中文标点、数字/中文混排、按钮垂直居中及大字号会反复出现局部修补。

截图中的问题不能归结为单一字体文件。必须把“文字整形与排版”和“字形栅格与 atlas 绘制”分开。

## 3. 外部实现研究

### Godot

Godot 4 的 `TextServer` 负责字体查找、shaping、glyph run、方向、换行和字形位置；`Font`/`FontFile` 负责字体资源与纹理化，RenderingServer 消费已布局的结果。参考：Godot 官方 `FontFile`、`TextServer` 类文档及源码 `scene/resources/font.cpp`、`servers/text_server.cpp`、`servers/text/text_server_adv.cpp`。可学习点是 TextServer 与绘制分层、按 run 返回 advance/offset/cluster；不能照搬其 Variant API 和 Godot 专用对象生命周期。

### Unity

Unity TextCore 的 `FontEngine` 负责字体 face、glyph metrics、glyph rect 和 atlas；TextGenerator/TextHandle 负责文本生成、line metrics、kerning、fallback 与布局缓存。参考 UnityCsReference `Modules/TextCoreTextEngine/Managed/FontAsset.cs`、`FontEngine.cs`、`TextGenerator.cs`。可学习点是字体资源度量与最终排版解耦、按 face/size 缓存；不能照搬 Unity 的 managed object 与包资源格式。

### Unreal Engine

UE Slate 的 `FSlateFontCache`、`FCompositeFont` 和 shaped text/layout cache 负责字体 fallback、glyph cache、atlas 与布局结果；Slate draw elements 只消费已解析的文本布局。参考 `Engine/Source/Runtime/SlateCore/Public/Fonts/SlateFontCache.h`、`Private/Fonts/SlateFontCache.cpp`、`FCompositeFont`。可学习点是 composite font、fallback run 与缓存身份分离；不能把 Slate 的 UObject/Slate widget 状态带入 AUI。

### 本项目现状

现有 `rust/crates/project_authoring_execution/src/font_cook.rs` 已生成 bitmap/MSDF glyph、bearing、advance 和 kerning；`rust/crates/engine_runtime/src/font_bundle.rs` 已提供 resolve/fallback/kerning；`rust/crates/engine_runtime/src/aui.rs` 当前同时负责 resolve、排字和 quad 生成。`rust/crates/editor_wgpu_renderer/src/font_system.rs` 是 Editor 独立路径，本方案不迁移它。

## 4. 方案设计

目标链路：

```text
AUI Text item
  -> FontShaper
  -> GlyphRun[]
  -> LineLayout
  -> GlyphQuadPlan
  -> UiProjection / Renderer
```

### GlyphRun 合同

```text
font_face_id
glyph_id
codepoint（可选诊断）
cluster_start / cluster_end
offset_x / offset_y（font units）
advance_x / advance_y（font units）
fallback_used
direction
```

shaping 库只负责把 UTF-8/Unicode 文本变成 GlyphRun；它不读 ECS、AUI binding 或 Renderer。

### 行布局合同

`LineLayout` 统一使用 font units 到 logical UI pixels 的单一换算，显式保存 ascender、descender、line_gap、baseline、line width 和 clipping。Bitmap/MSDF 只在 glyph quad 生成阶段转换到 atlas 像素。按钮、状态栏、暂停面板均使用同一 baseline/line-height 规则。

### 字体与 fallback

FontStack 先按 family/style/weight/script 选择 face，再以整段 run 为单位 shaping；禁止一个字符一个字符静默切换字体。缺字、fallback、混合 face、unsupported cluster 进入结构化 Summary/Trace 报告。

### 栅格与 atlas

继续复用 261/302 的 FontBundle cook、Bitmap/MSDF 和 RuntimePackage。保留 atlas padding、半像素 UV 内缩、采样边界修正；不把 atlas 采样问题塞进 shaping 层。

## 5. HarfBuzz 接入边界

首选 Rust HarfBuzz binding（HarfBuzz + FreeType/现有字体 face 数据）作为 shaping provider，封装为项目无关的 `engine_runtime::text_shaping`。RuntimePackage 的 `RuntimePackageSourceFontBundle` 携带 `font_face_sources[]`，每项包含 `font_face_id`、`face_index`、`source_digest` 和 `bytes`；loader 在加载时重新计算 SHA-256，digest 不一致即拒绝 bundle。接口只接收已校验字体字节/face identity、Unicode 文本、语言、脚本、方向和 features，输出确定性 GlyphRun。

第一阶段支持 Latin、数字、CJK、常用标点和 kerning；接口预留 Arabic、Indic、emoji、vertical writing，不在首个施工 Gate 扩展。HarfBuzz 版本、feature flags、字体 bytes digest 必须进入产物身份，避免不同机器 shaping 漂移。

## 6. 运行时与 AUI 边界

- AUI binding 只提供最终字符串与 TextStyle；不直接调用 shaping。
- `FontShaper` 不依赖 ECS、Project Rule、Window 或 Renderer。
- Renderer 只消费 `GlyphQuadPlan`，不重新计算 baseline、advance 或 kerning。
- RuntimePackage 继续保存已 cook 字体资源和 shaping 配置；Runtime 不扫描项目源目录。
- Editor FontSystem 继续独立运行，未来若迁移必须另开方案，不在 345 偷渡。

## 7. 施工 Gates

### Gate A：依赖与最小 shaping provider

- 锁定 HarfBuzz/字体解析依赖、许可证与版本。
- 新增 `GlyphRun` schema 与 provider owner 测试。
- 用 `中文，句号。引号“测试” 01:23 Agpy` 验证 cluster、advance、direction 和 deterministic digest。

### Gate B：AUI 排版迁移

- 将 `aui.rs` 当前逐字符排字迁移到 Shaper → LineLayout → QuadPlan。
- 接入 ascender/descender/line_gap、统一 baseline、fallback run、按钮垂直居中。
- 保留旧 GlyphPlan 字段，兼容现有 RuntimePackage；禁止 Renderer 再做第二次度量换算。

### Gate C：atlas 与真实窗口

- 验证 Bitmap/MSDF、atlas padding/UV、14/18/24/32/48/64px。
- 第一房普通、断电、暂停、失败、通关截图；检查中文标点、数字、英文、按钮四组文本。
- 1280×720、最大化窗口与可用的 1600×1000 入口分别记录，不用重采样图冒充窗口验收。

### Gate D：跨项目与交付

- 运行现有 FontBundle、AUI、RuntimePackage、项目字体和第二项目消费者回归。
- 独立 Windows dev 交付、源码/字体/配置 digest、缺字/裁切/串 atlas Summary。
- 旧项目若不能消费 GlyphRun，保留兼容适配层并记录迁移状态，不静默改变其视觉合同。

## 8. 验证经济性与排除项

最小因果变更是新增 shaping/line-layout owner 并让 AUI 消费其结果；不修改 Core ECS、Renderer 采样器、AUI binding 语义、玩法、场景或项目资源。首个 Gate 只需 provider 单测和固定文本 GlyphRun 对照；真实窗口放在迁移完成后。

排除：字体编辑器、完整富文本、emoji 彩色字体、文本输入法重构、Editor FontSystem 迁移、动态字体下载、全平台 release 矩阵。

复用 261/302/340 的有效字体 cook、atlas、MSDF、fallback 和窗口证据；新增 shaping 依赖、GlyphRun schema、AUI 排字代码或字体 profile 时，只有消费这些身份的证据失效。

## 9. 风险与回滚

- shaping 依赖构建失败：保留旧逐字符适配器作为明确 fallback，Gate A 不进入 AUI 迁移。
- shaping 结果与现有 kerning 不一致：以 provider GlyphRun 为唯一 advance 来源，禁止叠加旧 kerning。
- fallback face 基线不一致：在 LineLayout 统一 ascender/descender，保留 face-level offset 诊断。
- atlas 串字：回滚到旧 quad plan，仅修 atlas/采样 owner，不修改 shaping 数据。
- 任何 schema 不兼容：通过 RuntimePackage 版本门禁拒绝加载，不静默降级为错误字形。

## 10. 完成标准

方案完成必须证明：固定文本的 GlyphRun 可重复；中文/数字/英文/标点共享正确 baseline；Bitmap/MSDF 与 fallback 不产生上下漂移；按钮文字垂直居中；14–64px 无裁切、缺字和串 atlas；AUI Renderer 只消费最终 quad；现有项目和第二项目消费者回归通过。

本方案只定义方向与合同。用户已确认方案 C；生成施工文档、激活施工和修改正式架构代码需另行进入施工流程。
