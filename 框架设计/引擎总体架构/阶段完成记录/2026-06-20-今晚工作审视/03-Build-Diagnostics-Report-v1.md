# Build Diagnostics Report v1 完成记录

## 定位

Build Diagnostics Report v1 是统一构建诊断索引。

它不是新的验证器，而是把各阶段报告里的问题收敛成 AI 和用户都能读懂的诊断列表。

它回答的问题是：

```text
当前构建有哪些 error / warning / info？
问题来自哪个阶段？
问题关联哪个 assetId / bundleId / platform？
可能的修复方向是什么？
```

## 代码产物

核心实现：

```text
scripts/build-diagnostics.cjs
```

测试：

```text
scripts/test-build-diagnostics.cjs
package.json -> test:builddiagnostics
```

Build Graph 接入：

```text
scripts/build-graph-core.cjs
runDiagnoseBuildStage
writeBuildGraphDataFiles
writeBuildGraphManifest
```

## 输出文件

导出时会生成：

```text
build-diagnostics-report.json
```

schema：

```text
build-diagnostics-report.v1
```

## 当前读取的输入

当前读取：

```text
Platform Build Plan
Asset Report
Asset Cook Report
Bundle Plan
Bundle Manifest
Bundle Pack Report
Size Report
```

## 当前诊断结构

每条 diagnostic 包含：

```text
severity
kind
message
source
suggestedFixes
```

summary 包含：

```text
total
error
warning
info
byKind
byStage
```

## 当前能识别的问题类型

当前已经能归纳：

```text
platform-build-error
platform-build-fallback
asset-source-missing
asset-source-not-current
asset-cook-blocked
asset-cook-planned
asset-cook-warning
bundle-dependency-cycle
bundle-plan-warning
bundle-build-warning
asset-not-copied
bundle-pack-blocked
bundle-pack-warning
largest-asset
largest-bundle
size warnings
```

## 重要边界

Build Diagnostics Report v1：

```text
不重新验证项目
不替代原始报告
不阻止构建
不自动修改项目
不生成最终 Patch Plan
```

当前是：

```text
reportOnly = true
```

## 对 AI 的价值

这个模块是后续 AI 自动修复的入口。

AI 后续不应该直接扫描一堆 JSON 乱猜，而应该先读：

```text
build-diagnostics-report.json
```

再根据：

```text
source.stage
source.assetId
source.bundleId
source.platform
suggestedFixes
```

生成可审查的修复候选。

下一步最自然的模块是：

```text
AI Build Repair Plan v1
```

它应该读取 Build Diagnostics Report，然后生成“修复候选”，仍然不直接改项目。

## 验证命令

```powershell
npm.cmd run test:builddiagnostics
```

关联回归：

```powershell
npm.cmd run test:buildgraphcore
npm.cmd run test:buildgraph
npm.cmd run test:webexport
npm.cmd run test:androidexport
npm.cmd run test:iosexport
```

