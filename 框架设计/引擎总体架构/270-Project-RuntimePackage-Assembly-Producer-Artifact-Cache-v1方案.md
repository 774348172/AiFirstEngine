# 270 Project RuntimePackage Assembly Producer Artifact Cache v1 方案

## 1. 文档状态

```text
状态：方案已确认
采用方案：B，类型化 Assembly Producer + 内容寻址 Artifact Cache
首个落地 Producer：FontCookProducer
文档类型：独立引擎方案文档
后续步骤：基于本方案生成并自审独立引擎施工文档
```

本方案处理 269 完成后仍存在的 Editor Play 冷重建瓶颈，不修改塔防项目玩法、资产语义、
RuntimeModule 或项目声明。塔防项目只是外部真实资格样本，所有正式合同必须保持项目无关。

## 2. 问题与真实证据

269 的 fresh Tower Defense Editor Play 报告为：

```text
durationTotalMs                           93,976 ms
fingerprint_sources                       3,746 ms
assemble_project_runtime_package_input   89,172 ms
build_runtime_package                       524 ms
copy_runtime_assets                         389 ms
load_validate_runtime_package                92 ms
build_project_runtime_player_artifact         0 ms / skipped
```

证据路径：

```text
.artifacts/269/tower-preview-copy/editor-play-preview-package-report.json
```

269 已消除两个旧主因：

```text
Cargo target 不再进入 RuntimeModule source fingerprint。
in-process Editor GameView 不再构建独立 Player artifact。
```

剩余时间已经集中在 `ProjectRuntimePackageAssembler::assemble`。当前调用链在收集 AUI 后直接调用
`ProjectFontCookModule::cook_for_runtime_package`。字体 Cook 会依次完成 glyph resolve、Bitmap 多字号
raster、MSDF raster、kerning、atlas pack，最后才计算 `dependency_digest`。因此 digest 只能证明两次
输出依赖相同，不能在昂贵工作之前命中并复用产物。

塔防字体 profile 进一步放大该问题：

```text
policy = autoHybrid
bitmapPixelSizes = [16, 24, 32]
msdfEmSize = 64
includeRuntimeTextSources = true
locale = zh-CN
```

现有 assembly report 只有整体 stage 时间，没有 producer/substage 时间。因此字体是当前最强根因，
但施工前仍必须用细分计时完成实证，不能用推测替代报告。

## 3. 与既有系统的关系

### 3.1 217 Preview RuntimePackage Cache

217 的整包缓存继续保留：

```text
项目所有相关输入完全未变化
  -> Preview RuntimePackage cache hit
  -> 整个 assembly/build 跳过
```

它不能处理：

```text
只修改 Scene、Rule、Input 或不改变 glyph 集合的 AUI 文本
  -> 整包 fingerprint stale
  -> 当前仍全量执行所有 assembly producer
```

270 不替换 217，而是在整包 stale 后提供第二级派生产物复用。

### 3.2 261 FontBundle

261 已冻结以下原则：

```text
Font Cook 按 dependency digest 增量失效。
相同 glyph variant 不重复 raster。
MSDF generation 可以并行，但最终 merge/pack 必须确定。
Runtime 只加载 CookedFontBundle，不读取字体源文件。
```

270 将“dependency digest 增量失效”从报告语义补成真实的 cache lookup、artifact reuse 和确定性失效。

### 3.3 Build / Export Pipeline

Build Graph 现有 cache report 主要负责可解释指纹，并不普遍跳过 stage。270 不绕过 Build Graph Core，
也不直接把 Preview RuntimePackage 当作 Export 输入；它提供的是可被 Preview 和正式 Build/Export
共同调用的项目无关派生产物缓存能力。

## 4. 目标与非目标

### 4.1 v1 目标

```text
把 RuntimePackage assembly 从单体过程划分为可报告的类型化 Producer。
建立稳定、内容寻址、可校验、可清理的 Producer Artifact Cache。
先让 FontCookProducer 在昂贵 raster 前完成 recipe key lookup。
部分项目内容变化但字体依赖不变时，直接复用 sealed FontBundle artifact。
cache hit/miss/失效/损坏/重建原因全部进入结构化 report。
保持 RuntimePackage 是 Runtime 唯一运行输入真相。
```

### 4.2 v1 非目标

```text
不一次重构成完整通用 Build Graph / Asset Graph 调度器。
不承诺第一次 Font Cook 立即从 80-90 秒降到数秒。
不在 v1 实现 glyph shard、单字形 atlas patch 或跨版本 atlas 局部修改。
不在 v1 为 Texture、Prefab、AUI 全部启用真实 artifact reuse。
不修改字体视觉规格、塔防字体 profile 或减少中文字形来伪造性能提升。
不让 Runtime 读取 project source、cache manifest、TTF/OTF/TTC。
不把项目玩法条件或塔防语义写入 Rust 引擎代码。
```

## 5. 外部成熟方案对照

成熟引擎虽然术语不同，但共同结构一致：

```text
Unity Asset Import Pipeline / ArtifactDB：
  importer version + source/dependency hash -> Library derived artifact

Unreal Derived Data Cache：
  content/recipe key -> local/shared derived data

Godot import cache：
  importer/remap + source/options identity -> .godot/imported output

Bevy processed asset pipeline：
  source asset + processor settings/dependencies -> processed asset
```

270 吸收：

```text
缓存派生产物，不改变源资产真相。
key 同时覆盖内容、依赖、recipe/schema/toolchain 身份。
cache miss 只影响性能，不影响正确性。
缓存可以删除并由源输入确定性重建。
```

270 不照搬：

```text
不引入 Unity GUID/meta 全套模型。
不引入 Unreal 全局 DDC 服务层和网络共享协议。
不把 Godot import remap 暴露给 Runtime。
不为第一期建立过宽的异步 asset processor framework。
```

## 6. 正式采用的架构

### 6.1 两级缓存

```text
Level 1：Preview RuntimePackage Cache（217）
  identity = 完整 preview source fingerprint
  hit = 整包复用，跳过 assembly/build

Level 2：Assembly Producer Artifact Cache（270）
  identity = producer recipe key
  hit = 在整包 stale 后复用未受影响的昂贵派生产物
```

两级缓存不得混为一个状态。报告必须能区分：

```text
preview_package_cache = hit/stale/missing
producer_cache = hit/miss/invalid/corrupt/disabled
```

### 6.2 AssemblyInputSnapshot

Assembler 首先建立廉价、规范化、只读的 `AssemblyInputSnapshot`：

```text
project identity
build profile identity
active scene identity
normalized manifest references
normalized source descriptors
source content digests
schema/toolchain identities
```

Snapshot 负责提供 producer 计算 key 所需的输入，不携带 Runtime 对象或 GPU handle，也不把 Editor
selection、viewport、foldout 等非运行状态纳入身份。

Snapshot 只允许读取项目声明授权的输入。生成目录、Cargo target、Preview 输出和 producer cache 本身
不得反向进入 source identity。

### 6.3 类型化 Producer

正式逻辑形状：

```text
AssemblyInputSnapshot
  -> Producer.prepare_recipe(snapshot)
  -> ProducerRecipe { producer_id, recipe_version, dependency_digest }
  -> ArtifactCache.lookup(recipe_key)
       hit:
         validate envelope/payload digest
         deserialize typed artifact
       miss:
         Producer.produce(snapshot, recipe)
         validate typed artifact
         atomic publish
  -> merge typed outputs into RuntimePackageBuildInput
```

v1 不要求公开一个允许任意插件动态注册的复杂 trait graph。可以先建立受控的引擎内部 producer
catalog，但每个 producer 必须拥有独立 recipe、typed artifact 和 report，不得继续把全部工作藏在一个
无法观测的 assembly stage 中。

### 6.4 v1 Producer 范围

```text
FontCookProducer：
  正式启用 artifact lookup/reuse/publish。

Scene / Prefab / Input / Rule / AUI / Texture：
  建立 producer/substage timing 与结构化 report 边界。
  v1 可以仍执行现有实现，不得谎报 cache hit。
```

后续只有真实 producer identity、失效合同和资格样本明确后，才逐个启用 artifact reuse。

## 7. FontCookProducer 合同

### 7.1 廉价 recipe 阶段

必须把目前位于 raster 之后的依赖计算拆到昂贵工作之前。recipe 准备允许：

```text
解析字体资产声明。
读取并 hash 字体源文件。
收集/规范化 AUI 和声明 literal 的实际可达 codepoint。
解析 FontFamily / FontStack fallback reachability。
收集影响 kerning 输出的规范化 pair 集合。
读取 raster/packing profile。
```

recipe 阶段不允许：

```text
Bitmap raster。
MSDF raster。
atlas page 构建。
完整 FontBundle payload 生成。
```

### 7.2 Recipe Key

```text
font-cook-recipe-key = hash(
  producer_id,
  producer_recipe_version,
  font_cook_schema_version,
  canonical FontFace/Family/Stack/Profile declarations,
  ordered source font content digests,
  normalized reachable codepoint set,
  normalized required kerning pair set,
  bitmap pixel sizes and hinting policy,
  MSDF em size, pixel range and policy,
  atlas packing dimensions/padding/page budgets,
  locale/fallback resolution inputs,
  relevant engine/toolchain recipe identity
)
```

规则：

```text
使用内容 hash，不使用 mtime 作为正确性身份。
路径只以规范化 project-relative logical identity 参与；绝对路径不得导致相同项目副本无意义 miss。
集合先排序去重，再 canonical encode。
recipe version 由 producer 明确维护，算法语义变化必须升级。
与字体输出无关的 Scene/Rule/Input 变化不得使 key 失效。
文本顺序变化但 codepoint/kerning 需求不变时应允许复用。
新增 codepoint、替换字体内容或修改 raster/packing 参数必须 miss。
```

### 7.3 Typed Artifact

Font artifact 至少包含：

```text
artifact envelope
  schema_version
  producer_id
  producer_recipe_version
  recipe_key
  dependency_digest
  output_digest
  created_by_engine_identity
  payload_digest

typed payload
  RuntimePackageSourceFontBundle
  optional legacy atlas adapter payload（仅兼容期）
  deterministic FontCook report summary
```

cache hit 后必须验证 envelope、payload digest 和 typed payload schema，再交给 Assembler。不得把未经验证的
缓存字节直接写入 RuntimePackage。

## 8. 缓存所有权、路径与生命周期

### 8.1 所有权

Producer cache 是引擎管理的 generated derived data：

```text
可以删除。
不能作为项目源资产引用目标。
不能进入 source fingerprint。
不能提交为项目设计真相。
删除后必须能从授权源输入确定性重建。
Runtime 不知道其存在。
```

### 8.2 Cache Root

正式服务接受显式、受控的 cache root，不在深层模块读取真实用户配置：

```text
Editor/Build caller
  -> resolve engine-managed derived-data root
  -> AssemblyArtifactCache::open(root)
  -> Assembler request
```

测试和资格运行必须使用 run-owned cache root。production 可以使用 application-owned derived-data root；
不得默认写入安装态二进制目录，也不得污染项目 Assets/RuntimeModule/Rules/AUI。

内容寻址目录建议形状：

```text
<derived-data-root>/assembly-artifacts/v1/
  font-cook/<recipe-key-prefix>/<recipe-key>/
    artifact-envelope.json
    payload.bin
    producer-report.json
```

目录形状属于实现细节，正式身份是 schema + recipe key，而不是路径字符串。

### 8.3 并发与原子发布

```text
同 key 并发 miss 可以重复计算，但只允许原子 publish 完整 artifact。
先写同 root 临时目录，完成 payload digest 和 typed validation 后 rename/commit。
reader 不得观察半写入 artifact。
竞争 winner 已发布相同 output digest 时，loser 丢弃临时输出并复用 winner。
同 recipe key 产生不同 output digest 必须报 deterministic violation，不能静默覆盖。
```

v1 可以采用 per-key lock 减少重复工作，但锁不是正确性基础；进程崩溃后的 stale lock/temp 必须可恢复。

### 8.4 损坏、版本与清理

```text
envelope 无法解析 -> corrupt，隔离或删除后重建。
payload digest 不匹配 -> corrupt，隔离或删除后重建。
schema/recipe/toolchain identity 不兼容 -> invalid，cache miss 后重建。
artifact 缺失 -> miss。
cache root 不可写 -> 明确 diagnostic；允许无缓存生产，但不得谎报 hit。
```

清理策略不参与 v1 正确性。v1 至少提供显式清理能力和可枚举大小/最后访问证据；自动 LRU/配额可以在
真实磁盘数据出现后单独设计。

## 9. 可观测性合同

### 9.1 Assembly Report v2

在保持旧 reader 可兼容的前提下，assembly report 增加：

```text
producerReports[]
  producerId
  producerRecipeVersion
  status
  durationMs
  recipeDurationMs
  lookupDurationMs
  produceDurationMs
  validateDurationMs
  publishDurationMs
  cacheStatus
  inputDigest / recipeKey
  outputDigest
  missReason
  artifactPath（可选、仅 Editor/Build 证据）
  diagnostics[]
```

FontCookProducer 还要输出不含项目私密文本内容的 substage 统计：

```text
collect_text_requirements
resolve_font_assets
resolve_glyphs
collect_metrics
raster_bitmap
raster_msdf
collect_kerning
pack_atlas
build_font_bundle
```

### 9.2 Cache Status

```text
Hit
Miss
Invalid
Corrupt
Disabled
PublishRaceReused
Produced
Failed
```

`cacheStatus=Hit` 必须同时满足：

```text
没有执行昂贵 produce。
artifact envelope/payload 验证通过。
typed artifact 成功反序列化。
report 带有 recipe key 和 output digest。
```

### 9.3 Miss Reason

至少支持：

```text
artifact_missing
producer_recipe_changed
source_font_changed
glyph_requirements_changed
kerning_requirements_changed
font_profile_changed
schema_or_toolchain_changed
artifact_corrupt
cache_unavailable
```

如果实现无法可靠细分输入差异，可以先报告 `recipe_key_changed` 并附 previous/current component digests，
不能猜测一个错误的具体原因。

## 10. 正确性与安全边界

```text
RuntimePackage 仍是 Runtime 唯一输入真相。
Producer cache 只在 Editor/Build assembly 侧存在。
cache artifact 不是可信源；每次 hit 都验证 envelope 和 payload digest。
缓存命中不得放宽 ProjectRuntime trust、composition seal 或 source authorization。
缓存 key 不记录绝对项目路径、用户名或原始文本正文。
诊断不得打印字体二进制内容。
正式 Export 与 Preview 可以共享 producer artifact，但各自仍构建并验证自己的 RuntimePackage。
```

## 11. AI 适配度

该方案适合 AI-first 工作流，因为 AI 可以从报告直接回答：

```text
这次 Play 是整包 cache stale，还是 producer cache miss？
哪一个 producer 慢？
字体为什么失效？
是新增 glyph、字体源变化、profile 变化，还是 recipe version 变化？
缓存是否损坏并已重建？
运行包使用的 FontBundle output digest 是什么？
```

AI patch 常见的复制项目、重排 JSON、批量改文件和保留 mtime 不会破坏正确性，因为身份基于 canonical
content/dependency digest，而不是时间戳和调用者猜测。

控制设计成本的 C-Compact 原则同样适用：

```text
v1 只冻结一个通用 producer artifact envelope、一个 cache service、一个 report shape。
只让 FontCookProducer 真正复用。
其它 producer 先报告，不提前设计所有未来依赖图。
只有真实性能证据出现后才新增 producer 或细化 shard。
```

## 12. 性能目标与资格口径

性能数字是本机真实样本目标，不作为跨机器固定 CI 墙钟断言：

```text
Tower fresh / empty derived cache：
  必须完成 FontCook miss、产物发布和完整 substage report。
  冷耗时先作为基线，不以减少 glyph/profile 冒充优化。

Tower 第二次、完整项目未变化：
  仍由 217 Preview Package Cache hit，跳过 assembly。

Tower 修改 Scene/Rule/Input，但字体 recipe 未变化：
  Preview Package Cache stale。
  FontCookProducer cache hit。
  raster_bitmap 与 raster_msdf 不执行。
  assembly 目标从约 89 秒降到数秒级。

Tower 修改 AUI 文本但 glyph/kerning 需求集合未变化：
  FontCookProducer cache hit。

Tower 新增一个可解析中文字形：
  FontCookProducer cache miss 并确定性重建。

Tower 修改 font profile / font source：
  FontCookProducer cache miss 并确定性重建。
```

自动化测试主要断言 cache decision、stage skipped/produced、digests、失效原因和输出一致性；墙钟只要求
报告存在且非负。独立 performance qualification 才比较真实耗时。

## 13. 验收矩阵

### 13.1 Determinism

```text
相同输入、空缓存下两次独立 produce：output digest 相同。
相同输入、第二次 cache hit：typed payload 与第一次 byte-identical。
项目复制到不同绝对路径：recipe key 相同。
仅 JSON 无语义重排：recipe key 相同。
```

### 13.2 Invalidation

```text
Scene only change -> Font hit。
Rule only change -> Font hit。
Input only change -> Font hit。
AUI layout only change -> Font hit。
AUI text reorder with same requirements -> Font hit。
new glyph -> Font miss。
font bytes change -> Font miss。
raster/packing profile change -> Font miss。
producer recipe version change -> Font miss。
```

### 13.3 Recovery

```text
missing artifact -> produce/publish success。
truncated envelope -> corrupt diagnostic + rebuild。
payload digest mismatch -> corrupt diagnostic + rebuild。
unwritable cache root -> explicit diagnostic + uncached produce or controlled failure。
concurrent same-key build -> no partial artifact, final digest unique。
```

### 13.4 End-to-end

```text
Tower Defense 真实 production profile。
至少一个第二项目使用不同字体/profile 或无 FontBundle，证明项目无关。
Preview 与正式 RuntimePackage 对相同 recipe 使用相同 FontBundle output digest。
RuntimePackage Loader 在删除 producer cache 后仍可加载既有已构建包。
```

## 14. 分阶段落地建议

后续施工文档应按以下 Gate 拆分，但具体命令、文件清单和授权窗口由施工文档自审后冻结：

```text
Gate A：assembly/font substage timing 与 269 基线复现
Gate B：artifact envelope、recipe key、cache service schema
Gate C：FontCook recipe/produce 分离，昂贵工作前 lookup
Gate D：atomic publish、corruption recovery、concurrency
Gate E：Assembly Report v2 与 Editor Play report handoff
Gate F：Tower + second-project invalidation/determinism matrix
Gate G：fresh production performance qualification 与文档闭环
```

Gate A 必须先证明字体各 substage 的真实耗时。若证据否定字体是主因，施工必须暂停并修订方案，不能把
Font cache 做完后再解释 89 秒来自别处。

## 15. 备选方案与拒绝理由

### 15.1 方案 A：只做 Font Cook 专用缓存

拒绝作为正式架构。它能快速缓解当前样本，但会让 cache identity、损坏恢复、报告和路径策略成为字体
特例；Texture/AUI/Prefab 后续还会重复建设。

允许吸收的部分：v1 只让 FontCookProducer 真正复用，以控制首期范围。

### 15.2 方案 C：一次完成完整 Build/Asset Graph 增量化

暂不采用。当前只有字体拥有明确的 89 秒级证据，提前设计所有 producer 的依赖 DAG、远程缓存、调度、
逐资产 shard 和淘汰策略会显著提高设计与施工成本。

### 15.3 只并行化 MSDF

不采用为第一修复。并行化只能缩短 cache miss，不能消除 Scene/Rule/Input 变化后重复生成相同字形的
浪费。它可以在 Producer Cache 生效并获得冷 cook substage 数据后独立优化。

### 15.4 整个 RuntimePackageBuildInput 内存缓存

不采用。进程重启后失效，身份不透明，难以跨 Preview/Build 复用，也无法对损坏、schema、toolchain 和
单个 producer 失效给出可解释证据。

### 15.5 mtime / 目录时间戳缓存

禁止。复制项目、AI patch、版本控制还原、时钟变化和保留时间戳都会造成错误 hit 或无意义 miss。

## 16. 风险与控制

### 风险 1：recipe 准备本身重新变慢

控制：recipe 只做规范化、hash 和 requirement collection；单独报告 `recipeDurationMs`，禁止 raster。

### 风险 2：key 漏依赖导致错误 hit

控制：用显式 component digest、recipe version、mutation matrix 和冷/热输出 byte equality 验证。

### 风险 3：cache 数据损坏影响构建正确性

控制：envelope + payload digest + typed validation；损坏只导致重建，不允许进入 RuntimePackage。

### 风险 4：抽象过宽导致施工失控

控制：v1 一个 cache service、一个 artifact envelope、一个真实缓存 producer；其它 producer 只加报告边界。

### 风险 5：缓存无限增长

控制：v1 可枚举、可清理、可统计；自动配额在取得真实容量数据后单独设计，不能阻塞正确性主线。

## 17. 完成定义

270 只有同时满足以下条件才算完成：

```text
269 的 89,172ms assembly 有 producer/substage 级实证解释。
Font recipe key 在 Bitmap/MSDF raster 前完成。
相同 recipe 的第二次 assembly 真正跳过昂贵 Font produce。
Scene/Rule/Input/无新 glyph 的 AUI 变化均能复用 Font artifact。
新增 glyph、字体源/profile/recipe 变化均准确失效。
缓存损坏和并发写入不产生错误 RuntimePackage。
Preview 与 Build/Export 没有建立两套不一致 Font cache 真相。
Tower 与第二项目矩阵通过。
Runtime 仍只依赖 RuntimePackage。
报告能让人和 AI 解释 hit/miss、耗时、原因和 output digest。
```

## 18. 结论

```text
270 正式采用方案 B。
以类型化 Assembly Producer + 内容寻址 Artifact Cache 作为长期结构。
v1 先实现 FontCookProducer 的真实复用，其它 producer 先建立计时和报告边界。
现有 Preview RuntimePackage Cache 保持为第一级整包缓存。
冷 Font Cook 的并行化和 glyph shard 增量化延后到真实 substage 数据出现之后。
下一步是生成并自审 270 独立引擎施工文档，不在本方案阶段直接修改代码。
```
