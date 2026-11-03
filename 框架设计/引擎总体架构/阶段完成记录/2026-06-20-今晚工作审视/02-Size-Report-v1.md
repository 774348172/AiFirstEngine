# Size Report v1 完成记录

## 定位

Size Report v1 是 Build Graph 的体积审计阶段。

它回答的问题是：

```text
当前项目资源源文件有多大？
哪些资源最大？
哪个 Bundle 最大？
不同资源类型各占多少？
哪些资源没有进入包？
哪些资源只是计划转码，当前大小仍是源文件估算？
```

它不是最终包体大小计算器。

## 代码产物

核心实现：

```text
scripts/size-report.cjs
```

测试：

```text
scripts/test-size-report.cjs
package.json -> test:sizereport
```

Build Graph 接入：

```text
scripts/build-graph-core.cjs
runAnalyzeSizeStage
writeBuildGraphDataFiles
writeBuildGraphManifest
```

## 输出文件

导出时会生成：

```text
size-report.json
```

schema：

```text
size-report.v1
```

## 当前能力

当前已经支持读取：

```text
Asset Database
Asset Cook Report
Bundle Manifest
Bundle Pack Report
Platform Build Plan
```

当前报告内容：

```text
totalSourceBytes
bundledSourceBytes
largestAssets
largestAssetId
largestAssetBytes
largestBundleId
largestBundleBytes
assetsByType
sourceBytesByType
buildStates
cookModes
AI-readable warnings
```

## 重要边界

当前 Size Report v1：

```text
只使用源文件大小和 Bundle Build 输出大小
不代表真实 Cook 后大小
不代表压缩后大小
不代表最终安装包大小
不代表差分热更包大小
不阻止构建
```

当前是：

```text
reportOnly = true
```

后续体积预算 gate 必须基于这个报告扩展，而不是在平台脚本里临时判断。

## 对 AI 的价值

这个模块让 AI 能解释“为什么包体大”：

```text
最大资源是谁
最大 Bundle 是谁
哪类资源占比最高
哪些资源没有被复制
哪些资源需要真实压缩 / 转码后才有准确体积
```

这对用户最关心的“为什么 exe / 包体有 200MB”很关键，因为 AI 不应该只说 Electron 大、资源大，而应该能拿到结构化证据。

## 验证命令

```powershell
npm.cmd run test:sizereport
```

关联回归：

```powershell
npm.cmd run test:bundlepack
npm.cmd run test:buildgraphcore
npm.cmd run test:platformbuild
```

