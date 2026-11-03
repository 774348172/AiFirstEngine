# Build Graph Core 接入状态

## 定位

Build Graph Core 是当前导出链路的公共构建骨架。

它的长期目标是：

```text
公共构建阶段只实现一次。
平台脚本只保留平台特有包装。
每个阶段输出结构化 report。
Manifest 能解释包从哪里来。
AI 修复能根据 report 定位问题。
```

## 今晚新增接入的阶段

今晚新增或补齐：

```text
pack-bundles
analyze-size
diagnose-build
```

对应函数：

```text
runPackBundlesStage
runAnalyzeSizeStage
runDiagnoseBuildStage
```

对应输出：

```text
bundle-pack-plan.json
bundle-pack-report.json
size-report.json
build-diagnostics-report.json
```

这些输出已经进入：

```text
build-manifest.json
cook-project-data stage
Windows/Web/Android/iOS 导出产物数据目录
```

## 当前公共阶段顺序

当前 Build Graph Core 公共阶段：

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
cook-project-data
write-manifest
```

平台脚本在公共阶段前后插入自己的阶段。

## 当前平台接入状态

已接入：

```text
Windows Export
Web Export MVP
Android Export Skeleton
iOS Export Skeleton
```

平台脚本仍然负责：

```text
Windows runtime 复制 / exe 命名
Web runtime 复制
Android Gradle 工程骨架
iOS Xcode 工程骨架
平台 build gate
```

Build Graph Core 负责：

```text
Project Validation
Asset Database
Asset Report
Asset Cook Plan / Report
Bundle Plan
Bundle Manifest
Bundle Pack Plan / Report
Size Report
Build Diagnostics Report
Platform Build Plan
Package Manifest
Build Manifest
Build Cache Report
```

## 当前仍是 MVP 的部分

以下阶段已经有结构，但还不是真实生产能力：

```text
Asset Cook v1：只计划，不真实转码
Bundle Pack v1：只报告，不压缩归档
Size Report v1：只审计，不预算阻断
Build Diagnostics v1：只诊断，不自动修复
Build Cache Report v1：只解释 hit/miss，不跳过执行
Android / iOS：只有工程骨架和环境 gate，不是真实签名发布
```

## 为什么这仍然符合长期主义

这不是“偷懒做报告”，而是长期主义里很重要的一步。

如果直接做真实打包，构建问题会变成黑盒：

```text
为什么资源没进去？
为什么包体变大？
为什么 Android 可以而 iOS 不行？
为什么 AI 修复改错了？
```

现在先有 report，可以让后续每个真实能力都挂到已有边界上：

```text
真实 Cook -> 扩展 asset-cook-report
真实 Pack -> 扩展 bundle-pack-report
体积预算 -> 读取 size-report
AI 修复 -> 读取 build-diagnostics-report
热更包 -> 扩展 bundle pack / package manifest
增量构建 -> 扩展 build-cache-report
```

## 当前文档已更新

已同步更新：

```text
框架设计/引擎总体架构/07-Build-Export-Pipeline.md
框架设计/引擎总体架构/18-长期主义实现路径.md
框架设计/引擎总体架构/30-Build-Graph-Core-v2.md
```

