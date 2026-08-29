# Current Status Notice

本文档中早期 `npm.cmd`、Electron runtime、TypeScript 原型导出相关内容属于历史迁移记录。当前正式构建/运行主线以 Rust Native Runtime、RuntimePackage、DesktopExportPipeline、Native Player 相关当前方案和阶段记录为准。

热更新当前实现基线见 `201-热更新方案收敛与当前实现基线.md`。本文中的 Hot Update Package 是长期构建输出边界；当前 Bundle Pack v1 / RuntimePackageBuilder 不生成真实热更补丁包。

# Build-Export-Pipeline

## 当前归属说明：Build Graph 与 Projection

本文档中历史出现的 `RenderExtract / RenderAssetBridge / Render Asset Bridge`，从 `110-World-Projection-Adapter统一跨域同步规则.md` 起按以下方式理解：

```text
RenderExtract = RenderProjection
RenderAssetBridge = AssetProjection
```

Build / Export Pipeline 只负责生成 RuntimePackage、Bundle、Asset Manifest、Build Report 等构建产物。运行时从资源引用到渲染资源绑定的同步，属于 `AssetProjection`；从 ECS World 到 RenderSceneState 的同步，属于 `RenderProjection`。

## Build / Export Pipeline

Build / Export Pipeline 不只是导出按钮，而是一套可验证、可解释、可追踪、可缓存、可回滚的构建系统。

正式方案：

```text
AI-native Build Graph + Export Orchestrator
```

核心目标：

```text
AI 能理解构建流程
用户能看懂构建错误
构建结果可复现
出错可定位
资源和逻辑可增量构建
多平台可统一管理
```

### 总体流程

```text
User Build Intent
  -> AI Build Plan
  -> Build Profile Resolution
  -> Build Graph Generation
  -> Preflight Validation
  -> Logic Compile
  -> Native Runtime Build / Select
  -> Asset Cook
  -> Bundle Plan
  -> Bundle Build
  -> Package
  -> Sign
  -> Verify
  -> Export Report
```

其中 Bundle Plan 是 Build Graph 的自动产物，不是用户手写的资源包清单。

正式规则：

```text
AssetSet 是语义集合
Bundle 是构建产物
Bundle 分包由 Build Graph 自动生成
用户只配置策略，不手动管理底层包
```

构建系统根据以下输入生成 Bundle Plan：

```text
Asset Graph
AssetSet
Bundle Policy
Hot Update Policy
Build Profile
Platform Profile
Cook Profile
Quality Profile
资源依赖
资源体积
资源热更频率
首包 / 远程下载 / 常驻策略
```

用户和 AI 负责表达“想要什么策略”。  
Build Graph 负责计算“最终怎么分包”。

示例用户输入：

```text
帮我导出安卓版本，包体尽量小，但是保留战斗特效。
```

AI 生成 Build Plan：

```text
目标平台：Android
Build Profile：android_release
Quality Profile：mobile_high_60fps
Texture：ASTC 6x6
Mesh：生成 LOD
VFX：中等预算
Update Mode：Delta
签名：android_release_key
验证：运行资源审计和场景测试
```

### Build Graph

Build Graph 是构建系统核心。  
每个构建步骤是一个节点：

```text
resolve_project
validate_project
build_asset_database
compile_dsl_to_ir
validate_ir_semantics
build_ir_interpreter_package
compile_ir_to_rust_aot
validate_aot_equivalence
select_native_runtime
cook_assets
plan_bundles
build_bundles
package_platform
sign_package
verify_package
generate_report
```

## 构建期转换与运行时加载边界

Build Graph 承担重型转换，Runtime 只承担轻量加载。

正式规则：

```text
DSL -> IR 属于编辑器 / Build Graph 阶段。
IR validation 属于编辑器 / Build Graph 阶段。
IR -> Rust AOT 属于 Build Graph / 热更包生成阶段。
Rust codegen / compile 属于 Build Graph / 平台构建阶段。
Scene cook 属于 Build Graph 阶段。
Asset cook 属于 Build Graph 阶段。
Bundle pack 属于 Build Graph 阶段。
```

运行时不做：

```text
不在启动时全量解析 DSL。
不在启动时把 IR 现场转换成 Rust。
不在启动时调用 Rust 编译器。
不在切场景时全量重编项目规则。
不把编辑器对象当成正式 Runtime 输入。
```

运行时只做：

```text
读取 cooked project data。
校验 manifest / schemaVersion / hash。
加载 cooked scene data。
创建 ECS World。
加载已编译 rule module。
加载 bundle manifest 和必要资源引用。
启动 FrameLoop。
输出 RuntimeTrace / RenderFrameReport / FrameHash。
```

这条规则的目标：

```text
避免发布版本启动卡顿。
避免手机端运行时承担编译成本。
避免 Runtime 被编辑器 / AI 生成链路污染。
让 AI 查 Bug 时可以区分“构建期转换错误”和“运行时执行错误”。
```

## Runtime Package 生成规则

Rust Native Runtime MVP 第一版读取 Runtime Package，而不是编辑器内部 Project Object。

Build Graph 负责从 Project Schema 生成 Runtime Package：

```text
Project Schema
  -> validate_project
  -> cook_project_data
  -> write_runtime_package
  -> Runtime Package
```

第一版 Runtime Package 采用：

```text
Debug Readable Runtime Package
```

推荐结构：

```text
runtime-package/
  manifest.json
  scenes/
    scene-main.json
  assets/
    asset-manifest.json
  rules/
    rule-manifest.json
  golden/
    scenario-*.json
```

第一版规则：

```text
Runtime Package 可以是 JSON。
Runtime Package 是构建产物。
Runtime Package 不等于编辑器项目原始对象。
Runtime Package 不包含编辑器 UI 状态。
Runtime Package 必须有 manifest 和 schemaVersion。
Runtime Package 必须能被 Golden Scenario Test 直接消费。
```

Runtime Package v1 的生成原则：

```text
Runtime Package v1 = Normalized Project Runtime View
```

Build Graph 第一版只做：

```text
Normalize:
  排序、补默认值、稳定 hash、统一必要字段形态。

Strip:
  删除 editor-only 数据，例如 UI 面板状态、选择状态、AI 对话历史、Inspector 展开状态。

Validate:
  检查 scene / entity / component / assetRef / manifest 是否合法。
```

Build Graph 第一版不做：

```text
复杂 Prefab 展开。
复杂 Component lowering。
字段级 sourceMap。
复杂 provenance 系统。
运行时 rule compile。
```

说明：

```text
这里的“不做复杂 cooked binary”仅表示 Debug Readable Runtime Package 阶段不强制二进制紧凑格式。
正式 Runtime 资源加载仍必须由 Build / Cook 阶段生成 RuntimeAssetIndex、cooked_asset_table、bundle_table 和 dependency_table。
Release Mode 后续可以把同一套结构写成 cooked binary / compact package。
```

编辑器预览和导出一致性规则：

```text
Editor Play / Preview 必须先生成 Runtime Package。
Runtime Preview 和导出运行应尽量读取同一种 Runtime Package 输入。
不允许编辑器预览直接把内存 Project Object 交给 Runtime。
```

查错规则：

```text
第一版通过稳定 id 和对象级 Build Report 查问题。
不建立字段级 sourceMap 作为 MVP 必需能力。
如果 Runtime Package 和 Project Schema 保持同构，AI 可以通过 id / path 直接回查原始对象。
```

字段规范化规则：

```text
大部分字段尽量继承 Project Schema 命名。
Transform 例外，Runtime Package v1 提前采用运行时语义字段。
```

Transform Normalize：

```text
Project transform.position -> Runtime Package transform.localPosition
Project transform.rotation -> Runtime Package transform.localRotation
Project transform.scale -> Runtime Package transform.localScale
```

规则：

```text
Runtime Package v1 不写入 worldPosition / worldRotation / worldScale。
Runtime Package v1 不写入 localMatrix / worldMatrix。
world transform / matrix 由 Runtime 计算和缓存。
```

Renderable / mesh 规则：

```text
Runtime Package Entity 渲染字段固定使用 mesh.assetRef / mesh.materialRef。
RenderExtract 负责从 mesh.assetRef / materialRef 解析为 RenderCommand payload 或 RenderSceneState 所需的渲染资源句柄。
RenderFrameReport 只记录 AI / Debug 可读的解析摘要，例如 resolved / fallback / missing。
Runtime Package 不写入 meshRef / materialRef 这类渲染后端句柄。
```

Scene / Prefab Runtime Package 规则：

```text
Runtime Package 必须保留 Scene / Prefab 的 Entity Tree、Component Data、AssetRef、Prefab Instance 和 Overrides。
Runtime Package 必须保留稳定 sourceEntityId。
Runtime Package 不保存 runtimeEntityId。
runtimeEntityId 只由 Rust Runtime SceneInstantiator 在加载 Scene / Prefab 时分配。
Build Graph 不生成 GameObject / Actor 中间层。
```

运行时实例化边界：

```text
SceneInstantiator 从 Runtime Package 读取 Scene / Prefab 数据。
SceneInstantiator 负责 sourceEntityId -> runtimeEntityId 映射、Component 写入、Hierarchy 建立、Prefab 展开、Override 应用、EntityRef 修复。
SceneInstantiator 只校验 AssetRef 能通过 RuntimeAssetIndex 解析，不强制同步加载全部资源。
资源 preload / release 由项目侧 Loading Rule / Scene Lifecycle 控制。
```

长期规则：

```text
Editor / Debug Mode 可以生成 readable JSON package。
Release Mode 应生成 cooked binary / compact package。
两种包必须共享同一套 manifest、schemaVersion、hash 和 diagnostics 规则。
```

当前代码中的 Windows / Web / Android / iOS 导出已经共用 Build Graph Core，并落地 `resolve-build-profile`、`build-asset-database`、`plan-bundles`、`cook-assets`、`build-bundles`、`pack-bundles`、`analyze-size`、`diagnose-build`、`plan-ai-repair` 和 `write-manifest`：

```text
project.assets
  -> scanProjectAssetFiles
  -> AssetDatabase
  -> Asset Report
  -> Bundle Plan
  -> Asset Cook Report
  -> Bundle Build
  -> Bundle Manifest
  -> Bundle Pack Report
  -> Size Report
  -> Size Budget Report
  -> Build Diagnostics Report
  -> AI Build Repair Plan
  -> Platform Build Plan
  -> Package Manifest
  -> build-manifest.json 引用 asset-database.json / asset-report.json / asset-cook-report.json / bundle-plan.json / bundle-manifest.json / bundle-pack-report.json / size-report.json / size-budget-report.json / build-diagnostics-report.json / ai-build-repair-plan.json / platform-build-plan.json / package-manifest.json
```

当前 Asset Cook v1 和 Bundle Pack v1 都是报告 / 计划优先的最小闭环。  
它负责让导出产物携带资源元数据、资源状态、资源依赖图、解释型分包计划、平台 Cook 计划、朴素 Bundle Manifest、Bundle Pack Report、Size Report、Size Budget Report、Build Diagnostics Report 和 AI Build Repair Plan，方便后续真实转码、压缩包、热更、包体预算和 AI 查错继续接入。

资源文件扫描规则：

```text
Build Graph 不递归扫描整个项目目录。
Build Graph 只扫描 project.assets.source 明确引用的源文件。
generated / virtual source 不扫描，记录为 skipped。
缺失文件不直接让 Asset DB 构建失败，而是记录为 missing 状态。
越过项目根目录的 source 被记录为 outside-project-root。
目录 source 被记录为 not-file。
```

原因：

```text
大项目可能一次拉取大量资源，递归全盘扫描会不可控。
AI 查错需要知道“这次同步涉及哪些明确资产”，而不是面对整棵目录树。
缺失文件属于资源状态，是否阻止构建由后续 Build Profile / Platform Profile / Asset Policy 决定。
```

当前 Bundle Plan v1 规则：

```text
schemaVersion: bundle-plan.v1
target: 当前构建目标
policy: auto / scene-first / commonBundle
bundles: startup / scene-* / shared / loose
dependencyGraph: Bundle 到 Bundle 的依赖图
warnings: 可解释构建警告
```

Bundle Plan v1 只负责规划和解释，不负责：

```text
压缩资源
转换平台格式
生成真实 bundle 文件
下载远程热更包
运行时 mount / unmount
```

这些能力属于后续 Cook / Bundle Pack / Runtime Asset Loading 阶段。

当前 Bundle Build v1 规则：

```text
输入：bundle-plan.json + asset-database.json
输出：bundle-manifest.json + bundles/<bundleId>/bundle.json + copied source files
格式：朴素目录结构，不压缩，不加密，不做平台 Cook
缺失资源：记录 warning 和 buildState，不直接崩溃
目标：让 Runtime Loader 和 AI Debug 能读取可解释资源包结构
```

当前 Bundle Pack v1 规则：

```text
scripts/bundle-pack.cjs
bundle-pack-plan.json
bundle-pack-report.json
schemaVersion: bundle-pack-plan.v1
schemaVersion: bundle-pack-report.v1
```

Bundle Pack v1 是“包布局计划与报告”，不是正式压缩包生成器。

它负责：

```text
读取 Bundle Manifest
读取 Asset Cook Report
读取 Platform Build Plan
为每个 Bundle 生成 packAction / packMode / outputKind
统计 bundle 文件数、资源数、source bytes、planned cook conversions
报告缺失资源、未复制资源、计划转码但当前仍使用源文件的风险
让 AI 和用户知道当前导出包里的 Bundle 如何进入平台包体
```

当前策略：

```text
packMode: loose-directory
compression: none
encryption: none
reportOnly: true
hotUpdateReady: false
```

正式边界：

```text
Bundle Pack v1 不生成 zip / pak / obb。
Bundle Pack v1 不压缩资源。
Bundle Pack v1 不加密资源。
Bundle Pack v1 不重写 Bundle Build 输出。
Bundle Pack v1 不生成热更补丁包。
build-manifest.json 必须引用 bundle-pack-plan.json 和 bundle-pack-report.json。
Windows / Web / Android / iOS 导出都必须携带 bundle-pack-report.json。
```

真实 Bundle Pack 接入后必须复用 `bundle-pack-plan.json`，不能另起一套分包 / 压缩 / 热更规则。

当前 Size Report v1 规则：

```text
scripts/size-report.cjs
size-report.json
schemaVersion: size-report.v1
```

Size Report v1 是“体积审计报告”，不是预算失败 gate。

它负责：

```text
读取 Asset Database
读取 Asset Cook Report
读取 Bundle Manifest
读取 Bundle Pack Report
读取 Platform Build Plan
统计 totalSourceBytes / bundledSourceBytes
统计每个 Asset 的 sourceBytes / bundleId / cookMode / buildState
统计每个 Bundle 的 sourceBytes / fileCount / assetCount / plannedCookConversions
输出 largestAssets / largestBundle
输出 AI 可读 warnings
```

当前边界：

```text
Size Report v1 使用源文件大小和 Bundle Build 输出大小。
Size Report v1 不代表真实 ASTC / Basis / LOD / 音频转码后的最终大小。
Size Report v1 不代表 zip / pak / obb 压缩后的最终包体大小。
Size Report v1 不阻止构建。
Size Report v1 不自己扫描文件系统。
build-manifest.json 必须引用 size-report.json。
Windows / Web / Android / iOS 导出都必须携带 size-report.json。
```

当前 Size Budget Report v1 规则：

```text
scripts/size-budget-report.cjs
size-budget-report.json
schemaVersion: size-budget-report.v1
```

Size Budget Report v1 是基于 Size Report 的预算审查报告。

它负责：

```text
读取 Size Report
读取 Platform Build Plan / Build Profile 中的可选 sizeBudget 配置
输出 budgets / issues / summary
区分 source-estimate / packed-estimate / real-packed-size
非 strict profile 只产生 warning
strict profile 可产生 blocking issue
```

当前边界：

```text
Size Budget Report v1 不重新统计文件。
Size Budget Report v1 不自动压缩资源。
Size Budget Report v1 不自动降低质量。
当前 v1 使用 source-estimate，不代表真实 Cook / 压缩 / 热更差分后的最终包体。
build-manifest.json 必须引用 size-budget-report.json。
Windows / Web / Android / iOS 导出都必须携带 size-budget-report.json。
```

当前 Build Diagnostics Report v1 规则：

```text
scripts/build-diagnostics.cjs
build-diagnostics-report.json
schemaVersion: build-diagnostics-report.v1
```

Build Diagnostics Report v1 是“统一诊断索引”，不是新的验证器。

它负责：

```text
读取 Platform Build Plan
读取 Asset Report
读取 Asset Cook Report
读取 Bundle Plan
读取 Bundle Manifest
读取 Bundle Pack Report
读取 Size Report
把各阶段 warning / blocked / missing / largest item 归一化为 diagnostics[]
每条 diagnostic 必须包含 severity / kind / message / source / suggestedFixes
输出 summary.total / error / warning / info / byKind / byStage
```

当前边界：

```text
Build Diagnostics v1 不重新验证项目。
Build Diagnostics v1 不替代各阶段原始报告。
Build Diagnostics v1 不阻止构建。
Build Diagnostics v1 不自动修改项目。
build-manifest.json 必须引用 build-diagnostics-report.json。
Windows / Web / Android / iOS 导出都必须携带 build-diagnostics-report.json。
```

后续 AI 修复入口应优先读取 `build-diagnostics-report.json` 获取问题摘要，再按 source 追到原始报告细节。

当前 AI Build Repair Plan v1 规则：

```text
scripts/ai-build-repair-plan.cjs
ai-build-repair-plan.json
ai-build-repair-plan.v1
```

AI Build Repair Plan v1 是“AI 修复候选计划”，不是自动修复器。

它负责：

```text
读取 Build Diagnostics Report
把 diagnostics[] 转成 candidates[]
按 diagnosticKind / severity / action / source 建立可审查候选
标记 requiresUserApproval
固定 canAutoApply = false
解释 reason / expectedEffect / blockedReason / suggestedPatchIntent
```

边界：

```text
AI Build Repair Plan v1 不修改项目。
AI Build Repair Plan v1 不生成最终 ProjectPatchPlan。
AI Build Repair Plan v1 不删除资源。
AI Build Repair Plan v1 不替换资源。
AI Build Repair Plan v1 不重新扫描文件系统。
build-manifest.json 必须引用 ai-build-repair-plan.json。
Windows / Web / Android / iOS 导出都必须携带 ai-build-repair-plan.json。
```

当前 Asset Cook v1 规则：

```text
scripts/asset-cook.cjs
asset-cook-plan.json
asset-cook-report.json
schemaVersion: asset-cook-plan.v1
schemaVersion: asset-cook-report.v1
```

Asset Cook v1 是“平台资源处理计划与报告”，不是正式转码器。

它负责：

```text
读取 Asset Database
读取 Platform Build Plan
根据 assetCookFormat 生成每个资源的 cookMode / targetFormat / outputKind
输出可解释 warnings
让 AI 和用户知道某个资源在目标平台下未来应该如何处理
```

当前 cookMode：

```text
passthrough = 当前 MVP 保留源格式
planned-convert = 未来需要平台转码，当前只记录计划
planned-optimize = 未来需要优化，当前只记录计划
blocked = 源资源缺失或不可用
```

当前平台规则：

```text
Windows desktop-raw:
  texture -> passthrough，保留 png/jpg/webp 等源格式

Web basisu-web:
  texture -> planned-convert，目标 basisu

Android / iOS astc-mobile:
  texture -> planned-convert，目标 astc
  model -> passthrough，但报告未来 mobile mesh optimization warning
```

正式边界：

```text
Asset Cook v1 不改变源文件。
Asset Cook v1 不生成真实 ASTC / Basis / mesh LOD。
Bundle Build v1 仍复制源文件。
build-manifest.json 必须引用 asset-cook-report.json。
Windows / Web / Android / iOS 导出都必须携带 asset-cook-report.json。
```

真实转码器接入后必须复用 `asset-cook-plan.json`，不能另起一套平台规则。

当前 Build Graph Core v1：

```text
scripts/build-graph-core.cjs
npm.cmd run test:buildgraphcore
```

Build Graph Core 是跨平台导出的公共构建核心。  
平台导出脚本不应重复实现项目验证、Asset DB、Bundle Plan、Bundle Build、数据文件写入和 Manifest 写入。

核心职责：

```text
createBuildGraphContext
createBuildGraphReport
resolve-build-profile
validate-project
build-asset-database
plan-bundles
cook-assets
build-bundles
pack-bundles
analyze-size
diagnose-build
plan-ai-repair
cook-project-data
write-manifest
```

平台脚本职责：

```text
validate platform-specific inputs
prepare platform project/runtime directory
copy platform runtime dependencies
write platform-specific manifest extension
run/sign/package platform output when the platform toolchain exists
```

正式规则：

```text
公共构建阶段只能有一个实现来源：Build Graph Core。
Web / Android / iOS / Windows 导出必须逐步收敛到 Build Graph Core。
平台脚本只能扩展平台特有步骤，不能复制公共 Build Graph 逻辑。
构建报告必须继续保留 stage id，让 AI 和用户能定位失败阶段。
```

当前已迁移：

```text
Windows Export / Electron runtime copy -> Build Graph Core
Web Export -> Build Graph Core
Android Export Skeleton -> Build Graph Core
iOS Export Skeleton -> Build Graph Core
```

当前 Build Graph MVP 阶段顺序：

```text
resolve-build-profile
validate-project
validate-inputs
build-asset-database
plan-bundles
copy-electron-runtime
prepare-app-directory
copy-runtime-dependencies
build-bundles
cook-project-data
write-manifest
rename-executable
```

其中：

```text
plan-bundles = 计算怎么分包
cook-assets = 生成平台资源处理计划和报告
build-bundles = 生成可加载的最小 bundle 目录和 bundle-manifest.json
pack-bundles = 生成平台包布局计划和 bundle-pack-report.json
analyze-size = 生成 size-report.json，解释资源和 Bundle 体积
diagnose-build = 生成 build-diagnostics-report.json，统一聚合构建风险
plan-ai-repair = 根据 build-diagnostics-report.json 生成 ai-build-repair-plan.json，提供 AI 修复候选
resolve-build-profile = 根据 Platform Profile 解析 Build Profile 并生成可解释 fallback
write-manifest = 写 build-manifest.json / platform-build-plan.json / package-manifest.json
```

后续真正的 Asset Cook / Bundle Pack 应扩展 `cook-assets` / `pack-bundles` 阶段，不应改变“Plan / Build / Pack 分离”的边界。

当前 Build Graph Cache Report v1：

```text
scripts/build-graph-cache.cjs
build-cache.json
build-cache-report.json
schemaVersion: build-graph-cache.v1
schemaVersion: build-graph-cache-report.v1
```

当前缓存是“可解释缓存报告”，不是“跳过执行缓存”。

规则：

```text
每个公共 Build Graph stage 生成稳定输入指纹。
Build Graph 对比上一次同输出目录的 stage 指纹。
指纹一致记录 cache hit。
指纹变化或首次出现记录 cache miss。
build-manifest.json 必须引用 build-cache-report.json。
Windows / Web / Android / iOS 导出都必须携带 build-cache-report.json。
```

当前纳入 cache report 的公共阶段：

```text
resolve-build-profile
validate-project
build-asset-database
plan-bundles
cook-assets
build-bundles
pack-bundles
analyze-size
diagnose-build
plan-ai-repair
cook-project-data
```

当前不做：

```text
不跳过 stage 执行
不复用上一次输出文件
不宣称已经完成真实增量构建
不把 cache hit 当作正确性证明
```

原因：

```text
先建立可解释输入指纹和 cache report。
让 AI 能回答“为什么这个阶段重跑 / 为什么这个阶段没变化”。
等输出复用、失效策略、平台差异和错误恢复稳定后，再启用真正的 stage skip。
```

当前 Windows Package Manifest v1：

```text
platform-build-plan.json
package-manifest.json
```

规则：

```text
Windows 导出必须写出 Platform Build Plan。
Windows 导出必须写出 Package Manifest。
build-manifest.json 必须引用这两个文件。
Package Manifest 必须记录 platform / packageKind / runtime / ruleTarget / assetCookFormat / fallbacks。
这让 AI 和用户能从最终导出包反查“这个包是按什么平台规则生成的”。
```

当前 Web Export MVP：

```text
scripts/export-game-web.cjs
npm.cmd run export:web <project.json>
npm.cmd run test:webexport
```

Web 输出静态目录：

```text
index.html
ai-first-runtime.js
player.js
game-project.json
asset-database.json
asset-report.json
bundle-plan.json
bundle-manifest.json
platform-build-plan.json
package-manifest.json
bundles/
three/
```

规则：

```text
Web Export 必须执行 Platform Build Plan。
Web Export 必须写 Package Manifest。
Web Export 使用 packageKind=web-static。
Web Export 使用 runtime=web。
Web Export 当前 ruleTarget=web-ir。
Web Export 当前 assetCookFormat=basisu-web。
Web Export 复用 Runtime Asset Loader 和 Bundle Manifest。
Web Export 不复制 Electron main.cjs / preload.cjs。
```

当前 Android Export Skeleton：

```text
scripts/export-game-android.cjs
npm.cmd run export:android <project.json>
npm.cmd run test:androidexport
```

Android 输出工程骨架：

```text
android-project/
  settings.gradle
  build.gradle
  gradle.properties
  app/build.gradle
  app/src/main/AndroidManifest.xml
  app/src/main/java/<package>/MainActivity.kt
  app/src/main/res/values/styles.xml
  app/src/main/assets/ai-first/
    build-manifest.json
    package-manifest.json
    platform-build-plan.json
    game-project.json
    bundle-manifest.json
    bundles/
```

规则：

```text
Android Export Skeleton 必须执行 Platform Build Plan。
Android Export Skeleton 必须写 Package Manifest。
Android Export Skeleton 使用 packageKind=aab。
Android Export Skeleton 使用 runtime=native-rust。
Android Export Skeleton 当前 ruleTarget=rust-aot。
Android Export Skeleton 当前 assetCookFormat=astc-mobile。
Android Export Skeleton 不运行 Gradle。
Android Export Skeleton 不伪造 APK / AAB。
build-manifest.json 和 package-manifest.json 必须记录 gradleBuildStatus=not-run。
```

当前 Android Build Gate：

```text
scripts/android-build-gate.cjs
npm.cmd run android:buildgate <android-project-dir> [output-report.json]
npm.cmd run test:androidbuildgate
```

Android Build Gate 输出：

```text
android-build-gate-report.json
```

规则：

```text
Android Build Gate 只做环境与工程结构检查。
Android Build Gate 不运行 Gradle。
Android Build Gate 不生成 APK / AAB。
缺少 Android SDK 是 blocking。
缺少 NDK 当前是 warning，真实 Rust Native 编译阶段可升级为 blocking。
缺少 Gradle wrapper 当前是 warning，CI/一键构建阶段可升级为 blocking。
报告必须包含 canRunGradle / blockingIssues / warnings / nextActions。
```

当前 iOS Export Skeleton：

```text
scripts/export-game-ios.cjs
npm.cmd run export:ios <project.json>
npm.cmd run test:iosexport
```

iOS 输出工程骨架：

```text
ios-project/
  <AppName>.xcodeproj/project.pbxproj
  <AppName>/Info.plist
  <AppName>/AppDelegate.swift
  <AppName>/ViewController.swift
  <AppName>/Resources/ai-first/
    build-manifest.json
    package-manifest.json
    platform-build-plan.json
    game-project.json
    bundle-manifest.json
    bundles/
```

规则：

```text
iOS Export Skeleton 必须执行 Platform Build Plan。
iOS Export Skeleton 必须写 Package Manifest。
iOS Export Skeleton 使用 packageKind=ipa。
iOS Export Skeleton 使用 runtime=native-rust。
iOS Export Skeleton 当前 ruleTarget=rust-aot。
iOS Export Skeleton 当前 assetCookFormat=astc-mobile。
iOS Export Skeleton 不运行 xcodebuild。
iOS Export Skeleton 不伪造 IPA。
build-manifest.json 和 package-manifest.json 必须记录 xcodeBuildStatus=not-run。
```

当前 iOS Build Gate：

```text
scripts/ios-build-gate.cjs
npm.cmd run ios:buildgate <ios-project-dir> [output-report.json]
npm.cmd run test:iosbuildgate
```

iOS Build Gate 输出：

```text
ios-build-gate-report.json
```

规则：

```text
iOS Build Gate 只做环境与工程结构检查。
iOS Build Gate 不运行 xcodebuild。
iOS Build Gate 不生成 IPA。
非 macOS host 是 blocking。
缺少 xcodebuild 是 blocking。
缺少 Apple Development Team 当前是 warning，签名 / archive export 阶段可升级为 blocking。
报告必须包含 canRunXcodebuild / blockingIssues / warnings / nextActions。
```

当前 Runtime Asset Loading v1：

```text
导出 runtime 启动时读取 build-manifest.json
根据 build-manifest.bundleManifest 读取 bundle-manifest.json
创建 RuntimeAssetLoader
默认 mount startup Bundle
resolveAsset 只解析已 mount 且 buildState=copied 的资源
```

Runtime 资源加载接口规则：

```text
RuntimeAssetLoader 必须支持同步加载和异步加载两类入口。
异步加载是运行时默认推荐路径。
同步加载只用于启动、编辑器、加载界面、小型必需资源和测试；热路径同步加载必须产生 warning/report。
分阶段加载不属于 RuntimeAssetLoader 的固定底层模式，而属于项目侧 Loading Rule / Scene Lifecycle 编排。
```

RuntimeAssetIndex / cooked asset 生成规则：

```text
Build / Cook 阶段必须生成 RuntimeAssetIndex。
RuntimeAssetIndex 是 Runtime 资源加载的唯一索引真相。
Runtime 不读取完整编辑器 Asset DB。
AssetRef 必须通过 RuntimeAssetIndex 解析到 cookedAssetId / bundleId / loader_kind。
依赖必须在 Build 阶段展开为 dependency_table。
Runtime 阶段只校验和执行 dependency_table，不重新推导业务依赖。
```

Runtime Package 资源加载相关产物：

```text
runtime_asset_index
asset_set_table
bundle_table
cooked_asset_table
dependency_table
type_loader_table
diagnostics_source_map
```

Build Graph 必须保证：

```text
每个 RuntimeAssetIndex entry 都有 guid / assetId / type / cookedAssetId / bundleId / loader。
每个 cookedAssetId 都能在 cooked_asset_table 中找到。
每个 bundleId 都能在 bundle_table 中找到。
每个 dependency 都能解析到有效 RuntimeAssetIndex entry。
循环依赖必须在 Build 阶段报错或显式标记为允许。
Debug 包可以保留 sourceMap。
Release 包可以裁剪或压缩 sourceMap，但不能让 Runtime 解析依赖 sourceMap。
```

这一步只建立运行时资源边界，不代表渲染器已经会加载 glTF / Texture / Audio。  
具体资源类型的解码、实例化和 GPU 上传属于后续 Runtime Asset Type Loader / Render Asset Bridge 阶段。

当前 Render Asset Bridge v1：

```text
RenderExtract 读取 mesh.assetRef
RuntimeAssetLoader 解析 assetRef -> bundle URL
RenderAssetBridge 生成 resolved / fallback / none 绑定状态
桌面 runtime 当前仍用 primitive 显示，并记录可解释降级状态
```

当前 Render Asset Bridge 已扩展为多资源绑定：

```text
mesh.assetRef / mesh.assetId -> model binding
mesh.materialRef -> material binding
mesh.textureRef -> texture binding
```

这意味着 Build Graph 必须把 Scene / Prefab 中 Mesh 引用的 model / material / texture 都纳入 Asset Dependency Graph 和 Bundle Plan。  
用户和 AI 仍然不手写最终 Bundle；Build Graph 根据 Asset Graph 自动把这些依赖资源带入构建产物。

后续 Runtime Asset Type Loader 应继续沿用这个边界：

```text
model loader 读取 resolved URL -> glTF / mesh runtime resource
texture loader 读取 resolved URL -> GPU texture
material loader 组合 texture / shader / params -> material instance
render backend 消费 runtime resource，不直接理解 Asset DB / Bundle Plan
```

当前 Runtime Asset Type Loader v1：

```text
RenderAssetBridge.resolved binding
  -> RuntimeAssetTypeLoader.loadFromBinding
  -> typed runtime resource cache
  -> report ready / cached / failed
```

当前 Type Loader 使用 `url-only` 模式，只建立类型化资源缓存，不执行真实解码。  
后续 model / texture / material / audio 解码器应挂在 Type Loader 后面，而不是绕过 Bundle Manifest 或 Render Asset Bridge。

当前 Model Decoder v1：

```text
RuntimeAssetTypeLoader(model ready)
  -> ModelDecoder.decode
  -> metadata-only decoded model cache
  -> report ready / cached / failed
```

当前 Model Decoder 只识别 `.glb` / `.gltf` / unknown 格式，不实例化真实 Three.js Mesh。  
真实 GLTFLoader / GPU 上传 / 动画绑定属于后续 Model Runtime Backend，必须继续沿用 Model Decoder 的 cache / report / fallback 边界。

当前 Texture Decoder v1：

```text
RuntimeAssetTypeLoader(texture ready)
  -> TextureDecoder.decode
  -> metadata-only decoded texture cache
  -> report ready / cached / failed
```

当前 Texture Decoder 只识别 `.png` / `.jpg` / `.jpeg` / `.webp` / unknown 格式，不创建 GPU Texture。  
真实 GPU Texture 创建、mipmap、sampler、平台压缩格式加载属于后续 Render Backend / Texture Runtime Backend。

当前 Material Decoder v1：

```text
RuntimeAssetTypeLoader(material ready)
  -> MaterialDecoder.decode
  -> metadata-only decoded material cache
  -> report ready / cached / failed
```

当前 Material Decoder 只识别 `.json` / generated / unknown 来源，不解析完整 Material Graph，也不创建真实 Material Instance。  
真实 Material Graph、Shader IR、Pipeline 绑定属于后续 Material System / Render Backend。

当前 Material Runtime Backend v1：

```text
MaterialDecoder ready material
TextureDecoder ready texture
  -> game-runtime/material-runtime-backend.js
  -> Three.js MeshStandardMaterial / TextureLoader
  -> primitive mesh material
```

它让导出的桌面 runtime 中的 `mesh.materialRef / mesh.textureRef` 开始真实影响 primitive mesh 的材质显示。  
它仍然只是当前桌面 runtime 的 Three.js 适配层，不是最终 Native RHI 材质系统。

正式规则：

```text
Build Graph 复制 game-runtime 模板时会携带 material-runtime-backend.js。
Material Runtime Backend 必须在 Type Loader / Decoder 之后运行。
Material Runtime Backend 不读取 Asset DB / Bundle Plan。
Material Runtime Backend 只消费 decoded runtime resource。
未来 Native RHI 后端必须实现同样的 createMaterial / unload / report 契约。
```

当前 Audio Decoder v1：

```text
RuntimeAssetTypeLoader(audio ready)
  -> AudioDecoder.decode
  -> metadata-only decoded audio cache
  -> report ready / cached / failed
```

当前 Audio Decoder 只识别 `.mp3` / `.wav` / `.ogg` / `.m4a` / unknown 格式，不创建 AudioBuffer，也不播放声音。  
真实 AudioContext、空间音频、混音、流式播放和平台音频 Cook 属于后续 Audio Runtime Backend。

当前 Audio Runtime Backend v1：

```text
AudioDecoder ready audio
  -> game-runtime/audio-runtime-backend.js
  -> HTMLAudioElement cache
  -> optional Audio Component autoplay
```

它让导出的桌面 runtime 中的 `AudioComponent.clipRef` 开始进入真实音频 runtime 链路。  
它仍然只是当前桌面 runtime 的最小适配层，不是最终 Native Audio / Mixer 系统。

正式规则：

```text
Build Graph 复制 game-runtime 模板时会携带 audio-runtime-backend.js。
Audio Runtime Backend 必须在 Type Loader / Decoder 之后运行。
Audio Runtime Backend 不读取 Asset DB / Bundle Plan。
Audio Runtime Backend 只消费 decoded runtime resource 和 Audio Component snapshot。
未来 Native runtime 必须实现同样的 loadAudio / applySource / unload / report 契约。
```

当前 Model Runtime Backend v1：

```text
ModelDecoder ready model
  -> game-runtime/model-runtime-backend.js
  -> Three.js GLTFLoader
  -> glTF scene cache
  -> player.js clone scene for renderable
```

Build Graph 当前会复制桌面模型运行后端需要的最小 ESM 依赖：

```text
three/build/three.module.js
three/examples/jsm/loaders/GLTFLoader.js
three/examples/jsm/utils/BufferGeometryUtils.js
three/examples/jsm/utils/SkeletonUtils.js
```

这只是当前桌面 runtime 的模型后端。  
未来 Native RHI / Rust 渲染后端不能复用 Three.js，但必须实现同样的 Model Runtime Backend 契约和 report / fallback 语义。

每个节点必须记录：

```text
input files
input hash
output files
output hash
dependencies
cache status
warnings
errors
duration
responsible system
fix suggestions
```

构建不能是黑盒。  
任何失败都必须能定位到具体 Build Node、输入、输出和影响对象。

### 构建输入

Build Pipeline 读取：

```text
Project Manifest
Feature Specs
DSL / Graph
Project System IR
Project Rule Backend Manifest
Asset Database
Asset Graph
AssetSlot / AssetSet
Bundle Policy
Version Domains
Build Profile
Platform Profile
Quality Profile
Hot Update Policy
Module Manifest
Signing Profile
```

Build Pipeline 不读取用户手写的最终 Bundle 清单作为真相。  
最终 Bundle 结果必须由 Build Graph 根据 Asset Graph 和策略生成。

允许项目提供：

```text
Bundle Policy
AssetSet
Preload / Release Policy
Hot Update Policy
首包 / 远程下载策略
```

不允许项目把底层 Bundle 当作长期手工维护对象。  
原因是底层 Bundle 依赖、平台变体、热更差异、共享资源抽取和体积优化都应该由构建系统统一计算，否则后期会变成不可维护的手工分包工程。

### 构建输出

每次构建必须输出：

```text
Platform Package
Content Package
Hot Update Package，可选
Build Report
Validation Report
Asset Report
Bundle Report
Size Report
Performance Budget Report
Source / License Report
Rollback Manifest
```

当前 `asset-report.json` 必须包含 `referenceReport`：

```text
schemaVersion: asset-reference-report.v1
references: 所有资源引用记录
referencedBy: 按 assetId 聚合的反向引用索引
```

Build Graph 生成 Asset Report 时必须传入当前 Project，让资源依赖图和反向引用报告来自同一份项目数据。  
导出的游戏包必须携带这份报告，方便用户和 AI 在资源缺失、删除、替换、热更和体积分析时定位具体 Scene / Prefab / Entity / Component 字段。

资源删除 / 替换进入构建前必须先生成 Asset Impact Report。  
Impact Report 不一定写入最终运行时包，但必须作为编辑器、AI Patch Plan、构建验证和审计报告的输入。  
如果 Impact Report 存在 high risk 项，默认不能自动应用，必须由用户确认或由 AI 生成修复方案后重新验证。

当前 Patch Plan 支持资源操作：

```text
replaceAsset(assetId, replacementAssetId, approvedImpact, removeOriginal?)
deleteAsset(assetId)
```

Build / Export 前必须满足：

```text
replaceAsset 已通过同类型验证
replaceAsset 影响已有引用时已确认 approvedImpact
deleteAsset 目标资源没有任何引用
项目 Asset Graph 无缺失引用
Asset Report / Asset Impact Report 可以解释修改影响
```

构建系统不负责替用户修正这些问题。  
构建系统只报告明确失败原因；AI 可以基于报告生成新的 Patch Plan，再重新验证和构建。

平台输出示例：

```text
Windows:
  Rust Native Runtime EXE
  Rust AOT rule module
  IR rule package，可选，用于热更覆盖 / debug trace
  asset bundles
  installer / portable package

Android:
  Android shell
  Rust libengine.so
  Rust AOT rule module
  IR rule package，可选，用于热更覆盖 / debug trace
  cooked asset bundles
  APK / AAB
  signing

iOS:
  iOS shell
  Rust xcframework / static lib
  Rust AOT rule module
  受控 IR rule package，可选
  cooked asset bundles
  Xcode signing
  IPA

Web:
  Web runtime
  IR / Web backend rule package
  web cooked assets
  web manifest
```

### 构建错误分类

构建错误必须结构化，至少分为：

```text
Project Error：项目配置错误
Logic Error：DSL / IR / Interpreter / Rust AOT 错误
Asset Error：资源错误
Bundle Error：分包依赖错误
Platform Error：平台配置错误
Signing Error：签名错误
Policy Error：热更 / 权限 / 审批错误
Performance Error：包体 / 内存 / 性能预算超标
```

用户看到的错误不应是底层日志，而应该是可理解的工程问题：

```text
资源错误：冲刺技能缺少移动端拖尾资源。
影响功能：player_dash
建议修复：生成移动端低配拖尾，或关闭移动端拖尾。
```

### Source Trace

每个构建错误必须能追踪回：

```text
Feature Spec
DSL node
IR node
AssetSlot
AssetSet
Scene
Prefab
Build Profile
Bundle Policy
Version Domain
```

例如：

```text
错误：Android 包体超过预算 38MB

来源：
- Feature: season_01_units
- AssetSet: season_01_character_models
- Asset: boss_dragon_model
- Cook Profile: android_mobile
- Bundle: season_01_characters
```

### AI 修复建议

错误报告应为结构化数据，便于 AI 生成修复方案。

示例：

```json
{
  "errorType": "AssetBudgetExceeded",
  "platform": "android",
  "assetId": "boss_dragon_model",
  "budget": "12MB",
  "actual": "38MB",
  "source": {
    "featureId": "season_01_units",
    "assetSet": "season_01_character_models"
  },
  "suggestedFixes": [
    "generate_lod",
    "reduce_texture_size",
    "split_optional_bundle",
    "disable_mobile_variant"
  ]
}
```

AI 可以生成：

```text
方案 A：生成 LOD，预计减少 14MB
方案 B：贴图从 2048 降到 1024，预计减少 18MB
方案 C：Boss 资源改为进入 Boss 关卡前下载
```

用户选择后，AI 生成 Patch Plan，再由引擎验证和应用。

### Build Debug View

编辑器应提供 Build Debug View，而不是要求用户阅读日志文件。

视图包括：

```text
构建进度图
Build Graph 节点状态
错误列表
影响功能
影响资源
影响平台
建议修复
一键生成修复 Patch
重新验证按钮
构建报告
包体分析
```

目标用户即使没有编程基础或只有少量编程基础，也能看到：

```text
哪里错了
为什么错
影响什么功能
有哪些修复方案
修复后是否通过验证
```

### 增量构建和缓存

构建系统必须支持增量构建：

```text
DSL 没变 -> 不重新生成 IR
IR 没变 -> 不重新生成 IR Interpreter Package / Rust AOT Rule
资源源文件没变 -> 不重新 Cook
Bundle 依赖没变 -> 不重新打包
Native Runtime 没变 -> 复用 Runtime Template
```

构建缓存：

```text
IR Cache
IR Interpreter Package Cache
Rust AOT Rule Cache
Cooked Asset Cache
Bundle Cache
Native Runtime Cache
Package Cache
```

当前已实现的缓存基础：

```text
Build Graph Cache Report v1
Asset Cook Report v1
stage input fingerprint
hit / miss / changed / new summary
build-manifest.json -> build-cache-report.json
```

后续真实增量构建必须建立在该报告之上，不能绕过 Build Graph Core 直接按平台脚本判断缓存。

并行构建：

```text
资源 Cook 并行
平台变体并行
Bundle 构建并行
测试并行
```

### 多平台 Profile

平台差异由以下 Profile 统一管理：

```text
Platform Profile
Build Profile
Cook Profile
Signing Profile
Quality Profile
Hot Update Profile
```

统一原则：

```text
Project Logic Truth = Canonical Rule IR
Project Logic Debug / Hotfix = IR Interpreter
Project Logic Release = Rust AOT
Native Runtime = platform-specific
Assets = platform cooked
Build Profile = platform-specific
```

### 权责边界

Build / Export Pipeline 属于引擎机制。

引擎负责：

```text
Build Graph
Validation
Logic Compile
Asset Cook
Bundle Plan Generation
Bundle Build
Package
Signing Integration
Report
Cache
Incremental Build
Export Orchestration
```

项目负责：

```text
Build Profile
Platform Profile
Bundle Policy
Version Domains
Quality Profile
Hot Update Policy
Signing Profile 引用
```

AI 负责：

```text
Build Plan
错误解释
修复建议
Patch Plan
优化建议
Bundle Policy 建议
构建报告摘要
```

用户负责：

```text
选择平台
审批发布
选择修复方案
配置签名密钥
确认热更 / 回滚
确认高风险降级或删除
```

最终原则：

```text
Build Graph 是给引擎执行的。
Build Report 是给用户和 AI 查 Bug 的。
```

### Build / Run Package Orchestrator v1

Build / Run Package Orchestrator v1 是 Build / Export Pipeline 的本地运行闭环。

正式规则：

```text
采用 UE-like BuildCookRun 的 C-min 路线。
第一版只做 dev-desktop staged run folder 和 Rust Runtime 启动编排。
Runtime 必须只读取 staged runtime-package 和 staged cooked-assets。
Runtime 不读取编辑器内存 Project Object。
Build / Run 只生成 Runtime Package、cooked assets、launch command 和 BuildRunReport。
ProjectLogicRunner 在 Runtime 内执行项目规则，不在 Build / Run Orchestrator 内执行。
```

第一版流程：

```text
BuildRunRequest
  -> ResolveBuildProfile
  -> GenerateBuildPlan
  -> PreflightValidate
  -> WriteRuntimePackage
  -> CookAssetsMin
  -> SelectRuntimeExecutable
  -> StageRunFolder
  -> LaunchRuntime
  -> WriteBuildRunReport
```

第一版输出：

```text
dist/dev-desktop/
  runtime/
  runtime-package/
  cooked-assets/
  reports/
  logs/
```

第一版非目标：

```text
不做真实 Android / iOS 打包。
不做签名、安装器、Store package。
不做真实热更包。
不做 zip / pak / obb。
不做真实资源转码。
不做 IR -> Rust AOT codegen。
不引入 TypeScript Runtime 作为正式 fallback。
```

详细规则见：

```text
72-Build-Run-Package-Orchestrator-v1方案.md
```

### 关联规则索引

Build / Export Pipeline 只定义构建、导出、分包、签名、验证和报告流程。  
以下规则不在本文重复展开：

```text
逻辑系统边界 / IR / Rust AOT / ECS -> 05-逻辑系统边界-DSL-IR-RustAOT-ECS.md
Build / Run 最小可运行包 -> 72-Build-Run-Package-Orchestrator-v1方案.md
资源、AssetSet、Bundle、热更资源 -> 06-资源系统架构.md
热更新能力边界 -> 09-热更新能力边界.md
测试与验证矩阵 -> 11-测试与验证系统.md
团队协作和版本冲突 -> 12-团队协作与版本控制.md
Scene / Entity / Component / Prefab -> 15-Scene-Entity-Component-Prefab数据模型.md
```


