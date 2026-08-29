# Build Graph Core v2

本文档记录当前 Build Graph v2 的代码落点和边界。

## 目标

Build Graph Core 的目标不是增加一个新的打包系统，而是把所有平台共同需要的构建阶段收敛到一个可信实现里。

核心原则：

```text
公共构建阶段只能实现一次。
平台脚本只处理平台特有包装。
所有阶段必须输出结构化 report。
Manifest 必须能解释最终包来自哪些规则。
```

## 当前代码入口

```text
scripts/build-graph-core.cjs
scripts/test-build-graph-core.cjs
npm.cmd run test:buildgraphcore
```

当前 schema：

```text
build-graph-core.v1
```

## 公共阶段

Build Graph Core 当前提供：

```text
createBuildGraphContext
createBuildGraphPaths
createBuildGraphReport
runBuildGraphStage
runResolveBuildProfileStage
runValidateProjectStage
runBuildAssetDatabaseStage
runPlanBundlesStage
runCookAssetsStage
runBuildBundlesStage
runPackBundlesStage
runAnalyzeSizeStage
runDiagnoseBuildStage
runPlanAiRepairStage
initializeBuildGraphCache
flushBuildGraphCache
writeBuildGraphDataFiles
writeBuildGraphManifest
```

当前标准阶段顺序：

```text
resolve-build-profile
validate-project
validate-inputs，平台脚本提供
build-asset-database
plan-bundles
cook-assets
prepare-platform-output，平台脚本提供
copy-platform-runtime-dependencies，平台脚本提供
build-bundles
pack-bundles
analyze-size
diagnose-build
plan-ai-repair
cook-project-data
write-manifest
package/sign/verify，后续平台工具链提供
```

## 平台边界

Build Graph Core 负责：

```text
Project Schema 归一化
Platform Build Plan 校验
Project Validation
Asset Database
Asset Report
Bundle Plan
Asset Cook Plan
Asset Cook Report
Bundle Build
Bundle Pack Plan
Bundle Pack Report
Size Report
Build Diagnostics Report
AI Build Repair Plan
game-project.json
asset-database.json
asset-report.json
asset-cook-plan.json
asset-cook-report.json
bundle-plan.json
bundle-manifest.json
bundle-pack-plan.json
bundle-pack-report.json
size-report.json
build-diagnostics-report.json
ai-build-repair-plan.json
platform-build-plan.json
package-manifest.json
build-manifest.json
Build Report stage 记录
Build Cache Report stage 指纹和 hit/miss
```

平台脚本负责：

```text
Web runtime 目录和 Three.js 依赖复制
Android Gradle 工程骨架
iOS Xcode 工程骨架
Windows Electron / native runtime 目录
平台工具链 gate
平台签名 / package / verify
```

## 当前接入状态

已接入：

```text
Windows Export
Web Export MVP
Android Export Skeleton
iOS Export Skeleton
```

## 测试规则

Build Graph Core 必须有独立测试：

```powershell
npm.cmd run test:buildgraphcore
```

平台导出必须继续跑各自测试：

```powershell
npm.cmd run test:bundlepack
npm.cmd run test:sizereport
npm.cmd run test:builddiagnostics
npm.cmd run test:aibuildrepair
npm.cmd run test:webexport
npm.cmd run test:androidexport
npm.cmd run test:iosexport
npm.cmd run test:buildgraph
```

## Cache Report v1

当前 Build Graph Core 已接入观测型缓存报告：

```text
scripts/build-graph-cache.cjs
build-cache.json
build-cache-report.json
build-graph-cache.v1
build-graph-cache-report.v1
```

Cache Report v1 记录：

```text
stageId
fingerprint
previousFingerprint
status: hit / miss
reason: fingerprint-match / fingerprint-changed / new-stage
resultSummary
summary.hit
summary.miss
summary.changed
summary.new
```

当前规则：

```text
Cache Report v1 只做解释和验证。
Build Graph 仍然执行所有 stage。
build-manifest.json 必须引用 build-cache-report.json。
平台导出包必须携带 build-cache-report.json。
首次同输出目录构建应为 miss。
同输入重复构建应为 hit。
```

后续真实增量构建必须先满足：

```text
stage 输出文件可复用
cache miss 能明确说明原因
cache hit 能验证输出仍存在
平台 profile / cook profile / runtime template 变化会正确失效
失败构建不能污染可复用 cache
```

## Asset Cook Report v1

Build Graph Core 当前已接入 Asset Cook Report v1：

```text
scripts/asset-cook.cjs
asset-cook-plan.json
asset-cook-report.json
asset-cook-plan.v1
asset-cook-report.v1
```

当前阶段顺序：

```text
build-asset-database
plan-bundles
cook-assets
build-bundles
pack-bundles
analyze-size
diagnose-build
```

规则：

```text
cook-assets 读取 Asset Database 和 Platform Build Plan。
cook-assets 输出平台资源处理计划和报告。
Bundle Build v1 仍复制源文件。
Asset Cook v1 不做真实转码。
build-manifest.json 必须引用 asset-cook-report.json。
平台导出包必须携带 asset-cook-report.json。
```

当前平台策略：

```text
desktop-raw: texture passthrough
basisu-web: texture planned-convert -> basisu
astc-mobile: texture planned-convert -> astc
model/audio/material/scene/prefab/script: 当前 mostly passthrough，报告未来优化点
```

## Bundle Pack Report v1

Build Graph Core 当前已接入 Bundle Pack Report v1：

```text
scripts/bundle-pack.cjs
bundle-pack-plan.json
bundle-pack-report.json
bundle-pack-plan.v1
bundle-pack-report.v1
```

规则：

```text
pack-bundles 读取 Bundle Manifest、Asset Cook Report 和 Platform Build Plan。
pack-bundles 输出平台包布局计划和报告。
Bundle Pack v1 只做 report-only。
Bundle Pack v1 不压缩、不加密、不归档、不生成热更包。
build-manifest.json 必须引用 bundle-pack-plan.json 和 bundle-pack-report.json。
平台导出包必须携带 bundle-pack-report.json。
```

当前平台输出类型：

```text
windows: portable-resource-directory
web: static-resource-directory
android: android-assets-directory
ios: ios-resources-directory
```

当前固定策略：

```text
packMode: loose-directory
compression: none
encryption: none
hotUpdateReady: false
```

后续真实 Bundle Pack 必须复用该 plan/report 边界，再逐步增加 zip / pak / obb / compression / encryption / delta hot update。

## Size Report v1

Build Graph Core 当前已接入 Size Report v1：

```text
scripts/size-report.cjs
size-report.json
size-report.v1
```

规则：

```text
analyze-size 读取 Asset Database、Asset Cook Report、Bundle Manifest、Bundle Pack Report 和 Platform Build Plan。
analyze-size 输出体积审计报告。
Size Report v1 只做 report-only。
Size Report v1 不直接做预算失败 gate。
Size Report v1 不重新扫描文件系统。
Size Report v1 不代表真实 Cook / 压缩 / 热更差分后的最终包体。
build-manifest.json 必须引用 size-report.json。
平台导出包必须携带 size-report.json。
```

当前报告内容：

```text
totalSourceBytes
bundledSourceBytes
largestAssets
largestAssetId / largestAssetBytes
largestBundleId / largestBundleBytes
assetsByType
sourceBytesByType
buildStates
cookModes
AI-readable warnings
```

当前 Size Budget Report v1 已作为 Size Report 的派生报告接入：

```text
scripts/size-budget-report.cjs
size-budget-report.json
size-budget-report.v1
```

规则：

```text
Size Budget Report v1 基于 size-report.json，不重新扫描文件系统。
默认非 strict 只产生 warning，不阻止构建。
strict profile 可产生 blocking issue。
报告必须标明 source-estimate / packed-estimate / real-packed-size。
当前 v1 使用 source-estimate，不假装知道真实 Cook / 压缩后大小。
Build Diagnostics Report 读取 size-budget-report.json 并生成预算诊断。
AI Build Repair Plan 基于预算诊断生成 report-only 候选。
build-manifest.json 必须引用 size-budget-report.json。
平台导出包必须携带 size-budget-report.json。
```

## Build Diagnostics Report v1

Build Graph Core 当前已接入 Build Diagnostics Report v1：

```text
scripts/build-diagnostics.cjs
build-diagnostics-report.json
build-diagnostics-report.v1
```

规则：

```text
diagnose-build 读取 Platform Build Plan、Asset Report、Asset Cook Report、Bundle Plan、Bundle Manifest、Bundle Pack Report 和 Size Report。
diagnose-build 输出统一诊断索引。
Build Diagnostics Report v1 只做 report-only。
Build Diagnostics Report v1 不重新验证项目。
Build Diagnostics Report v1 不替代各阶段原始报告。
Build Diagnostics Report v1 不做构建失败 gate。
Build Diagnostics Report v1 不自动修改项目。
build-manifest.json 必须引用 build-diagnostics-report.json。
平台导出包必须携带 build-diagnostics-report.json。
```

每条诊断必须包含：

```text
severity
kind
message
source
suggestedFixes
```

后续 AI 修复入口应优先读取 Build Diagnostics Report，再根据 source.stage / assetId / bundleId 追到原始报告。

## AI Build Repair Plan v1

Build Graph Core 当前已接入 AI Build Repair Plan v1：

```text
scripts/ai-build-repair-plan.cjs
ai-build-repair-plan.json
ai-build-repair-plan.v1
```

规则：

```text
plan-ai-repair 读取 Build Diagnostics Report。
plan-ai-repair 输出 AI 修复候选计划。
AI Build Repair Plan v1 只做 report-only。
AI Build Repair Plan v1 不修改项目。
AI Build Repair Plan v1 不生成最终 ProjectPatchPlan。
AI Build Repair Plan v1 不自动删除、替换或恢复资源。
AI Build Repair Plan v1 固定 canAutoApply=false。
build-manifest.json 必须引用 ai-build-repair-plan.json。
平台导出包必须携带 ai-build-repair-plan.json。
```

每个候选必须包含：

```text
candidateId
diagnosticKind
severity
action
requiresUserApproval
canAutoApply
source
reason
expectedEffect
blockedReason
suggestedPatchIntent
diagnosticMessage
```

后续真正的 AI 修复流程必须从 `ai-build-repair-plan.json` 里选择候选，再生成可审查的 ProjectPatchPlan 或任务计划。

## 后续扩展

后续能力必须扩展 Build Graph Core 或其明确 stage，而不是复制平台脚本逻辑：

```text
Asset Cook
Bundle Pack compression / archive / delta package
incremental cache
rule compile
source map
size budget gate
diagnostics-to-ai-repair entry
platform verify
signing report
hot update package
```

如果某个平台需要特殊行为，优先通过平台 stage hook 表达；只有真正平台相关的工作才能留在平台脚本中。
