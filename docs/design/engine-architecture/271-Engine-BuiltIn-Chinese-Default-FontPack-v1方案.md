# 271 Engine Built-in Chinese Default FontPack v1 方案

## 1. 文档状态

```text
方案：C，分层内置中文字体包
用户确认：2026-08-04
方案状态：已确认，可据此生成独立引擎施工文档
当前施工状态：未生成施工文档，未授权施工
```

本方案只定义项目无关的引擎字体资源、构建产物和 RuntimePackage 装配合同，不修改塔防项目，
不把任何塔防文案、玩法或项目路径写入引擎。

## 2. 要解决的问题

261 已完成项目字体资产、`ProjectFontCookModule`、`CookedFontBundle v2`、Hinting R8、MSDF、
AutoHybrid、multi-page 和 Runtime 渲染链。270 已完成 Font producer 的内容寻址整包缓存，把相同
recipe 的 Tower Assembly 从冷路径约 85 秒降到热路径约 1.55 秒。

现有缺口是：一个没有自定义字体的普通中文项目仍没有正式的项目运行时默认中文 FontBundle。
如果项目为了显示中文而导入完整字体并首次 Cook，主要时间仍消耗在 MSDF raster；如果每个项目
各自保存完整字体，也会重复占用源资源和派生产物空间。

271 要提供：

```text
引擎随发行版提供一个已 Cook、已验证、内容寻址的默认中文 Core FontBundle。
普通项目无需导入字体、无需现场 raster，Assembler 直接把预制包装配进 RuntimePackage。
自定义字体继续走 270 整包缓存，不改变项目字体能力。
未来新增生僻字走 glyph shard cache；只有大量真实 miss 才使用有界 MSDF 并行生成。
```

## 3. 已确认输入与证据

用户提供的上游包：

```text
<local-font-archive>\Noto_Sans_SC.zip
archive size = 112760167 bytes
license = SIL Open Font License 1.1
reserved font name = Source
```

本方案选择其中单一静态 Regular face 作为构建源：

```text
entry = static/NotoSansSC-Regular.ttf
size = 10559284 bytes
sha256 = d45f67f0a7c0ca3f256950777ce6a61cc7ce5f9696d02900cbbaac25f8aa7d16
faceIndex = 0
weight = 400
style = normal
```

zip 内可变字体与仓库现有 Editor 字体不是相同 bytes：

```text
zip variable font:
  size = 17773248
  sha256 = e80613a35583f59b46dbf6cc2eb640f3db0bb0f53fa7f6fbaa7b09faf20e5172

existing Editor font:
  rust/resources/editor/fonts/NotoSansSC-VF.ttf
  size = 17773244
  sha256 = 763146584cf0710223441356b4395e279021b0806c196614377a7a0174ae074a
```

因此施工不得把 Desktop zip 路径当长期依赖，也不得把两个字体版本视为可互换。施工必须把选定
Regular face、OFL 和 provenance 作为 repo-owned、hash-pinned 的引擎开发资源导入。

## 4. 现有性能与容量基线

270 Tower 冷路径：

```text
RuntimePackage Assembly = 85270 ms
Font producer = 84134 ms
Bitmap raster = 1717 ms
MSDF raster = 79655 ms
```

相同 recipe 热路径：

```text
RuntimePackage Assembly = 1550 ms
Font producer produceDurationMs = 0
Bitmap/MSDF skipped = true
```

当前 Tower profile 约 139 个实际 codepoint，使用 16/24/32 Bitmap 和 64px MSDF，生成 556 个
glyph variants、2 个 512x512 R8 page 和 3 个 512x512 RGBA8 page。原始 page payload 约
3.5 MiB；270 JSON cache 因 typed JSON 表示约 9.7 MiB。

当前 `CookedFontBundle v2` page payload 是未压缩整页 bytes。以 512x512 page 粗估：

```text
约 1200-1500 个中文 Core codepoint：约 28-40 MiB raw bundle pages
GB2312 一级 3755 字：约 85-100 MiB raw bundle pages
一级加二级约 6763 字：可能超过 150 MiB raw bundle pages
```

这些是施工前预算基线，不是最终资格数据。正式施工必须用真实 cook report 重新给出 glyph、variant、
page、raw bytes、stored bytes 和 Runtime GPU bytes。

## 5. 成熟引擎参考

### 5.1 Bevy

Bevy `crates/bevy_text/src/font_atlas_set.rs` 使用：

```text
FontAtlasKey:
  font data id/index
  font size
  variation hash
  hinting
  smoothing

FontAtlasSet(HashMap<FontAtlasKey, Vec<FontAtlas>>)
```

它以 `GlyphCacheKey` 查询 glyph 是否存在，并按字体渲染身份维护一个或多个 atlas。可学习点是：
glyph cache 必须包含完整 raster identity，新增 glyph 应只影响对应 atlas/cache；不可照搬点是 Bevy
偏运行时动态 atlas，而本引擎正式 Runtime 只能读取 RuntimePackage，不在 frame 热路径解析字体或
生成 MSDF。

### 5.2 Godot

Godot `scene/resources/font.cpp` 中 `Font` 维护 fallback 和 cache invalidation，`FontFile` 暴露
MSDF、hinting、oversampling 等字体渲染设置；字体或 fallback 变化时使相关 RID/cache 失效。

可学习点是：完整字体源、fallback、glyph cache 和渲染配置分层；不可照搬点是系统字体 fallback
和运行时动态字体加载。本引擎要求 Preview、Play、Export 使用相同确定性 FontBundle digest，
Runtime 不扫描系统字体或引擎安装目录。

### 5.3 采纳结论

271 采纳成熟引擎的分层与 glyph identity 思路，但保留本引擎已有的离线 Cook 和
RuntimePackage-only Runtime：

```text
完整字体源：仅开发/构建侧
Core FontBundle：引擎发行资源，普通项目默认复用
Project FontBundle：项目显式字体，继续走 270
Glyph shard：未来按完整 raster identity 增量生成
Runtime：只加载最终 RuntimePackage 中的 FontBundle
```

## 6. 范围

### 6.1 v1 必须完成

```text
repo-owned、hash-pinned 的完整 Regular 字体开发源与 OFL/provenance
版本化默认中文 glyph set spec 与 resolved lock
离线、确定性的 Engine Built-in FontPack builder
预制中文 Core CookedFontBundle v2 与 sealed manifest
普通项目默认选择与 Assembler 注入
项目 DefaultUi 字体对内置默认字体的确定性覆盖
Preview / Play / Export digest parity
容量、冷/热 Assembly、真实中文视觉和许可证资格证据
结构化 report 与 fail-closed diagnostics
```

### 6.2 v1 明确不做

```text
不实现 glyph shard cache
不实现 MSDF 并行化
不实现 Runtime 动态 raster
不实现 Runtime 系统字体 fallback
不修改 CookedFontBundle v2 的基本 glyph/page identity
不默认预制 GB2312 3755 或 6763 全量字库
不生成或发布裁剪 TTF 作为唯一字体源
不增加塔防项目或其它 sample 的路径/文案特例
不把 Editor 267 字体栈直接暴露给 Project Runtime
```

glyph shard 和 MSDF 并行化必须在 v1 的真实 miss/容量/生产数据形成后，分别生成后续正式方案或
R1/R2 施工窗口；不得在 271 v1 施工中顺手扩张。

## 7. 总体架构

```text
Engine Development Font Source Pack
  NotoSansSC-Regular.ttf + OFL + provenance
             |
             v
EngineDefaultGlyphSetSpec.v1
             |
             v resolve/freeze
EngineDefaultGlyphSetLock.v1
             |
             v offline ProjectFontCook backend reuse
EngineBuiltInFontPackManifest.v1
  + CookedFontBundle v2 metadata
  + raw bitmap/MSDF pages
             |
             v validate/seal
Engine release resources
             |
             v ProjectRuntimePackageAssembler
RuntimePackageBuildInput.font_bundles
             |
             v existing RuntimePackageBuilder
Self-contained RuntimePackage
             |
             v existing RuntimeFontBundleLoader
RuntimeFontRegistry / AUI / UiProjection / Renderer
```

禁止新增 Runtime 字体源查找旁路。预制包进入运行时的唯一合法方式仍是：

```text
ProjectRuntimePackageAssembler
  -> RuntimePackageBuildInput
  -> RuntimePackageBuilder
  -> RuntimePackage
```

## 8. 字体源与许可证合同

### 8.1 完整源必须保留

Core FontBundle 只含常用 glyph。为了未来生成生僻字 shard，引擎开发/安装的受信构建资源中必须
保留完整 `NotoSansSC-Regular.ttf`。完整源不得自动写入项目目录或 RuntimePackage。

如果只保留裁剪后的 TTF，被删除的 glyph 无法恢复，glyph shard 设计将失去数据源。因此 v1
不把字体子集化作为性能主方案；真正减小普通项目成本的是预制 FontBundle，而不是缩小 source TTF。

### 8.2 provenance

引擎字体源至少记录：

```text
sourceId
upstreamArchiveName
archiveSha256
entryPath
fontBytesSha256
byteLength
faceIndex
family/style/weight
copyright
licenseId
licensePath
reservedNames
importedAtUtc（只进 provenance，不进 cook identity）
```

### 8.3 命名

内部 pack identity 使用项目无关名称：

```text
aife-default-zh-cn-common-v1
```

不得给派生字体使用 OFL 保留名称 `Source`。v1 不发布派生 TTF；若未来确需发布子集 TTF，必须：

```text
改写 name table 为非保留名称
保留 copyright 与 OFL
记录 subset recipe 和 source hash
继续保留完整字体作为 shard source
```

FontBundle 和引擎发行包仍携带可发现的 OFL 与 attribution，不以“只发布位图”规避许可记录。

## 9. 默认 glyph set 合同

### 9.1 spec 与 lock 分离

引擎不在 Rust 中硬编码上千字符。使用两个 schema-first 工程对象：

```text
EngineDefaultGlyphSetSpec.v1：声明来源、范围、优先级和预算
EngineDefaultGlyphSetLock.v1：保存解析后的排序 codepoint、来源映射和 digest
```

建议 spec：

```yaml
schemaVersion: engine-default-glyph-set-spec.v1
glyphSetId: aife-zh-cn-common-v1
locale: zh-CN
sources:
  - asciiPrintable
  - chineseFullwidthPunctuation
  - commonUiSymbols
  - engineZhCnCatalog
  - defaultProjectTemplates
  - pinnedFrequencyCorpus
mandatoryCodepoints:
  - U+FFFD
hanCodepointTarget:
  minimum: 1200
  maximum: 1500
budgets:
  maximumTotalCodepoints: 1800
  maximumRawBundleBytes: 41943040
```

`pinnedFrequencyCorpus` 必须具有 repo-owned provenance、稳定版本和可再分发许可。找不到合格来源时，
不得下载未知网络列表凑数；可以先用合法的引擎 Catalog、模板语料和明确可追溯字表生成较小候选，并让
资格 Gate 因未达到 minimum 而阻止正式发布。

### 9.2 固定组成

v1 至少包含：

```text
ASCII printable U+0020-U+007E
中文全角标点
数字、百分比、正负号、常用货币和 UI 符号
replacement glyph U+FFFD
引擎 zh-CN Catalog 的实际字符
默认项目模板的实际用户可见字符
经 provenance 固定的 1200-1500 高频汉字
```

不得包含：

```text
日志和 diagnostic 原始英文
schema key、asset id、hash、文件路径
任意 Rust 源字符串
Tower Defense 或其它 sample 专用文案
整个 CJK Unified Ideographs range
```

### 9.3 lock 的确定性

resolved lock 至少记录：

```text
schemaVersion
glyphSetId
specDigest
sortedCodepoints
codepointSourceTags
totalCodepointCount
hanCodepointCount
glyphSetDigest
```

相同 spec、Catalog、模板和 frequency corpus 必须产生 byte-identical lock。任何增删字都形成可审查
diff，并使 built-in pack recipe 失效。

## 10. Core FontBundle 配置

v1 复用 261 的正式 AutoHybrid backend：

```text
font face = Noto Sans SC Regular, faceIndex 0, weight 400
bitmap sizes = 16 / 24 / 32
bitmap format = R8Unorm
MSDF em size = 64
MSDF pixel range = 8
MSDF format = RGBA8Unorm
packing order = existing deterministic glyph/variant order
```

页面尺寸和 page budget 由施工资格 spike 在 512 与现有 texture-array 合同允许的候选中选择，必须
同时满足：

```text
所有同 render mode page 维度一致
raw bundle bytes <= 40 MiB
total codepoints <= 1800
AutoHybrid 的 Bitmap/MSDF variant 不缺失
Windows production WGPU texture-array limit 通过
```

不得为了缩包静默删除 MSDF、减少现有 16/24/32 Bitmap 变体，或让小字号请求回退到错误 render mode。
如果 1200 字 minimum 与 40 MiB 在现有 v2 合同下不能同时满足，施工必须停止并修订本方案，不得暗改
resolver 或冒充达标。

## 11. EngineBuiltInFontPackManifest

预制包使用单独的 engine-owned sealed manifest，不伪装成项目 `FontFaceAsset`：

```yaml
schemaVersion: engine-built-in-font-pack-manifest.v1
packId: aife-default-zh-cn-common-v1
locale: zh-CN
sourceIdentity:
  sourceId: noto-sans-sc-regular
  fontBytesSha256: sha256:d45f...
  faceIndex: 0
  licenseId: OFL-1.1
glyphSet:
  glyphSetId: aife-zh-cn-common-v1
  glyphSetDigest: sha256:...
recipe:
  cookedFontBundleSchemaVersion: cooked-font-bundle.v2
  producerRecipeVersion: ...
  rasterProfileDigest: sha256:...
artifact:
  fontBundleId: aife-default-zh-cn-common-v1
  bundleDigest: sha256:...
  metadataPath: ...
  pagePaths: [...]
budgets:
  codepointCount: ...
  glyphVariantCount: ...
  rawPageBytes: ...
```

manifest、metadata 和每个 page 都必须校验 schema、byte length 和 digest。缺失、corrupt、recipe
不兼容或超预算时，Assembler fail closed；不得静默退回 legacy C-min atlas。

## 12. 默认选择规则

Assembler 按现有 `FontAtlasProfileRole` 做项目无关决策：

```text
项目没有 FontAtlasProfileRole::DefaultUi：
  注入 built-in zh-CN Core FontBundle，并设为 RuntimePackage default UI bundle。

项目有且只有一个合法 project DefaultUi profile：
  该项目字体是 default UI bundle，继续走 270 整包缓存。
  不自动把 built-in pack 塞进 fallback，避免掩盖 required glyph 和风格差异。

项目只有 Additional profiles：
  built-in pack 仍是 default UI bundle；Additional project bundles 继续走 270。

项目有多个 DefaultUi、显式 project default 构建失败或 built-in pack 无效：
  fail closed，输出结构化 diagnostic。
```

未来如需显式禁用 built-in default，必须通过版本化 Project/Build Profile policy 另行设计；v1 不增加
隐藏环境变量、用户目录配置或 path convention。

## 13. 预制包复用与 RuntimePackage 自包含

“普通项目复用预制包”指复用生产结果，不是让 Runtime 在安装目录中查字体：

```text
Editor/Assembler：读取 engine-owned sealed built-in artifact
RuntimePackageBuilder：把 metadata/page payload 写入目标 RuntimePackage
Runtime：只读取目标 RuntimePackage
```

开发期可通过内容寻址文件共享、hardlink/reflink 或原子复制减少磁盘和装配成本，但这些只是 producer
实现细节。任何优化都必须满足：

```text
Preview、Play、Export bundle digest 相同
独立 Windows Player 脱离 Editor 安装目录仍可运行
删除全局 derived-data 后，已导出的 RuntimePackage 仍完整
Runtime 不读取完整 TTF
```

## 14. 与 261、267、270 的关系

### 14.1 继承 261

```text
复用 CookedFontBundle v2
复用 ProjectFontCookModule 内部 parser/raster/MSDF/packer backend
复用 RuntimePackageSourceFontBundle
复用 RuntimeFontBundleLoader/Registry
复用 AUI -> UiProjection -> RuntimeRenderer 字体链
不修改 AutoHybrid 选择语义
```

271 可以为 engine resource 构建提供一个离线 owner，但不能复制一套字体 cooker。

### 14.2 不混用 267

267 的 Noto Sans SC 是 Native Editor renderer 的独立字体栈。271 是 Project Runtime/AUI 的默认
FontBundle。两者可以来自同一字体家族，但资产身份、版本、缓存、renderer 和 RuntimePackage 边界独立。

禁止直接把 `rust/resources/editor/fonts/NotoSansSC-VF.ttf` 路径交给 Runtime。

### 14.3 继承 270

```text
project custom font -> 270 whole-bundle producer cache
built-in default font -> release-sealed built-in artifact
```

两个 namespace 不互相冒充：

```text
assembly-artifacts-v1/font-cook/...       project recipe cache
engine-built-in-font-packs-v1/...         engine release resource
```

项目显式字体变化只失效 270 recipe；引擎 Core glyph lock 或 recipe 变化只发布新的 built-in pack identity。

## 15. 未来 glyph shard cache 合同

glyph shard 不属于 v1 施工，但 v1 必须保留以下可扩展 identity：

```text
GlyphShardKey:
  source_font_digest
  face_index
  glyph_id
  codepoint_resolution_identity
  render_mode
  raster_variant_id
  raster_recipe_version
```

未来流程：

```text
项目可达文本
  -> Core FontBundle coverage check
  -> missing codepoints
  -> glyph shard cache lookup
  -> miss 时从完整 Regular source 生成 shard
  -> 确定性 merge/pack 成项目扩展 FontBundle generation
  -> 仍通过 RuntimePackage
```

v1 不得预先加入“空 shard manager”、假 provider seam 或 Runtime patch API。后续方案必须先用 v1
真实 miss 分布证明 shard 粒度、packing、merge 和 generation 切换的必要性。

## 16. 未来 MSDF 并行化合同

MSDF 并行化只处理大量真实 shard miss 或 built-in pack 离线重建，不参与普通项目命中预制包的路径。

后续实现必须满足：

```text
bounded worker count，不按 glyph 无界起任务
相同 glyph/recipe exactly-once produce
worker 输出按稳定 GlyphShardKey 排序后 merge
packer 输入顺序与串行结果相同
串行与并行 metadata/page bytes/digest 完全一致
取消、失败和 panic 能 join/清理，不发布半成品
```

没有足够大 miss 集合时，调度开销可能高于收益，应继续使用串行生成。

## 17. Report 与 diagnostics

遵守 Off / Summary / Trace。Assembler Summary 至少报告：

```text
fontSelection = builtInDefault | projectDefault | builtInPlusProjectAdditional
packId
bundleDigest
glyphSetDigest
codepointCount
glyphVariantCount
bitmapPageCount
msdfPageCount
rawPageBytes
selectionDurationMs
copyOrLinkDurationMs
projectFontCacheStatus（如适用）
```

建议 diagnostic：

```text
BuiltInFontPackManifestMissing
BuiltInFontPackManifestInvalid
BuiltInFontPackDigestMismatch
BuiltInFontPackRecipeIncompatible
BuiltInFontPackBudgetExceeded
BuiltInFontPackRequiredGlyphMissing
BuiltInFontPackLicenseMetadataMissing
DefaultUiFontSelectionAmbiguous
ProjectDefaultFontCookFailed
```

Runtime Off 不生成这些完整构建 report；Runtime 只保留 FontBundle load/present 必需的 compact 状态。

## 18. 性能与容量验收

v1 资格目标：

```text
built-in Core unique Han codepoints = 1200-1500
total codepoints <= 1800
raw page payload <= 40 MiB
普通无自定义字体项目 font raster duration = 0 ms
普通无自定义字体项目 font selection/injection 不触发 parser/MSDF dependency
第二次 Assembly 不依赖 270 project-font cache 才能命中 built-in pack
Preview / Play / Export bundle digest 完全一致
```

装配耗时阈值必须在施工 Gate A 先测量后冻结；方案不凭当前机器猜一个虚假毫秒门槛。最终施工文档
必须以相同 run root、同一项目副本、相同 build profile 对比 legacy、270 project font 和 built-in
default 三条路径。

## 19. 验收矩阵

### 19.1 source/provenance

```text
Regular face hash/size/faceIndex 精确匹配
OFL 和 copyright 可从发行包发现
Desktop zip 路径删除后 repo build 仍可重复
source corrupt/hash mismatch fail closed
```

### 19.2 glyph set

```text
spec -> lock 双生成 byte-identical
Catalog/模板/frequency source provenance 完整
U+FFFD、ASCII、中文标点和目标汉字覆盖
sample 专用文案未进入默认 lock
增删 source 字符正确改变 glyphSetDigest
```

### 19.3 pack

```text
同输入双 cook metadata/page byte-identical
16/24/32 Bitmap 和 64px MSDF 均可解析
glyph/page/byte budget 全部满足
corrupt metadata/page/digest 稳定拒绝
```

### 19.4 selection

```text
blank project -> built-in default
project Additional only -> built-in default + Additional
project DefaultUi -> project 270 cache path
multiple DefaultUi -> deterministic failure
project default cook failure 不静默回退 built-in
```

### 19.5 RuntimePackage

```text
Preview / Play / Export 同 digest
RuntimePackage 自包含 metadata/pages
Runtime dependency closure 不含 TTF parser、rasterizer、MSDF generator 或完整 TTF
脱离 Editor 安装目录的 Windows Player 正常显示中文
```

### 19.6 visual

```text
16/24/32px 中文 Bitmap screenshot
36/48/72px 中文 MSDF screenshot
中英文、数字、标点、replacement glyph 同屏
多 DPI/连续缩放不缺字、不改变 baseline
真实 production composition，不能用 debug overlay 冒充
```

### 19.7 project independence

至少使用：

```text
一个最小普通项目，无 Font Assets
一个只有 Additional font profile 的普通项目
一个自定义 DefaultUi font 的普通项目
Tower Defense 只作为外部 consumer，不修改其源文件
```

## 20. 建议施工窗口

本文不是施工文档。后续独立施工文档建议拆分：

```text
Window 1 / Gate A：source/provenance、容量 spike、frequency corpus 资格
Window 2 / Gate B：glyph set spec/lock schema、resolver、确定性与否定测试
Window 3 / Gate C：复用 261 backend 的离线 built-in pack builder、sealed manifest
Window 4 / Gate D：Assembler selection 与 RuntimePackageBuildInput 注入
Window 5 / Gate E：Preview/Play/Export、自定义 DefaultUi/Additional 项目矩阵
Window 6 / Gate F：性能、容量、真实 Windows/WGPU 视觉、许可证与文档闭环
```

施工文档必须把 glyph shard cache 和 MSDF 并行化列为 deferred，不得在 Window 1-6 中实现。

## 21. 风险与控制

### 风险 1：默认包过大

控制：`1200-1500 Han`、`<=1800 total`、`<=40 MiB raw page` 三重 hard gate。不得用估算替代真实
cook report。

### 风险 2：高频字表来源不清

控制：frequency corpus 必须 pin 版本、许可、hash 和 provenance。来源不合格时 fail closed，不把
未知网络字表写入正式资源。

### 风险 3：预制包破坏 RuntimePackage 真相

控制：预制 artifact 只能由 Assembler 注入 `RuntimePackageBuildInput`；Runtime 仍只读最终包。

### 风险 4：内置默认掩盖项目字体错误

控制：存在 project DefaultUi 时，失败即失败，不自动回退 built-in。

### 风险 5：裁剪字体阻断未来生僻字

控制：完整 Regular source 保留为受信开发资源；v1 不把 subset TTF 当唯一 source。

### 风险 6：默认包版本升级导致所有项目无意义失效

控制：packId、glyphSetDigest、recipeVersion 和 bundleDigest 分离。只有使用该 built-in pack 的
RuntimePackage identity 受影响；project custom 270 cache 不受影响。

## 22. 方案自审

### 22.1 是否满足用户确认的方案 C

是：

```text
预制默认中文 Core FontBundle：包含
普通项目默认复用：包含
自定义字体继续走 270：包含
完整字体保留以支持未来生僻字：包含
glyph shard cache：冻结长期合同，v1 deferred
MSDF 并行化：仅大批量真实 miss，v1 deferred
```

### 22.2 是否把普通条件硬写进 Rust

否。glyph 来源、目标数量、预算、source provenance 和 pack manifest 都是版本化数据合同。Rust 只实现
通用 schema validation、确定性 resolver、builder 和 Assembler selection。

### 22.3 是否复制字体 cooker

否。built-in builder 必须复用 261 `ProjectFontCookModule` 的内部 backend/recipe primitive，不能另建
第二套 parser/raster/MSDF/packer 真相。

### 22.4 是否改变 Runtime 输入

否。Runtime 仍只读取 RuntimePackage。完整字体源和 built-in release artifact 都不能成为 Runtime
旁路输入。

### 22.5 是否具备 AI 适配性

是。spec/lock/manifest/report 都是稳定 schema；字符变化形成明确 diff；source、glyph set、recipe、
artifact 和 RuntimePackage digest 可逐层解释；失败具有 next action，不要求 AI 猜日志或扫描 binary。

### 22.6 是否扩大到未验证优化

否。glyph shard 和 MSDF 并行化只保留 identity 与约束，明确 deferred。v1 先解决普通项目默认字体
首次 Cook 的最大真实问题。

### 22.7 自审结论

```text
方案结论：通过
与 261 / 267 / 270 冲突：无
需要修改塔防项目：否
允许生成施工文档：需要用户后续明确要求
当前允许施工：否
```
