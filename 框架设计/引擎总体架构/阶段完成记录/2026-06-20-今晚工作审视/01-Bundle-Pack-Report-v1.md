# Bundle Pack Report v1 完成记录

## 定位

Bundle Pack Report v1 是 Build Graph 里的包布局解释阶段。

它回答的问题是：

```text
当前 Bundle 准备如何进入不同平台的包？
每个 Bundle 里面有多少资源？
哪些资源没有成功复制？
哪些资源只是计划 Cook 但还没有真实转码？
当前包布局是否只是 loose directory？
```

它不是最终压缩打包器。

## 代码产物

核心实现：

```text
scripts/bundle-pack.cjs
```

测试：

```text
scripts/test-bundle-pack.cjs
package.json -> test:bundlepack
```

Build Graph 接入：

```text
scripts/build-graph-core.cjs
runPackBundlesStage
writeBuildGraphDataFiles
writeBuildGraphManifest
```

## 输出文件

导出时会生成：

```text
bundle-pack-plan.json
bundle-pack-report.json
```

schema：

```text
bundle-pack-plan.v1
bundle-pack-report.v1
```

## 当前能力

当前已经支持：

```text
读取 Bundle Manifest
读取 Asset Cook Report
读取 Platform Build Plan
按平台生成包布局策略
统计 bundle 文件数量
统计 source bytes
识别 planned cook conversion
识别 missing / not-copied asset
生成 AI 可读 warnings
```

平台输出类型：

```text
windows -> portable-resource-directory
web -> static-resource-directory
android -> android-assets-directory
ios -> ios-resources-directory
```

当前固定策略：

```text
packMode = loose-directory
compression = none
encryption = none
hotUpdateReady = false
reportOnly = true
```

## 没有完成的能力

当前明确没有做：

```text
不创建 zip / pak / obb
不压缩
不加密
不生成热更包
不重写 Bundle 文件
不做真实平台安装包结构
```

这些后续必须在当前 plan/report 边界上继续扩展，不能绕开 Build Graph 另写一套平台逻辑。

## 对 AI 的价值

这个模块让 AI 能看懂：

```text
哪个 Bundle 有问题
哪个资源没有进入包
哪个平台只是 loose-directory
哪个资源需要真实 Cook 阶段
当前包体大小只是源文件估算
```

后续 AI 修复入口可以根据它生成：

```text
恢复缺失资源
替换资源
移除未使用资源
调整 Bundle 策略
开启真实压缩 / 归档阶段
```

但当前模块不会自动修复，只报告。

## 验证命令

```powershell
npm.cmd run test:bundlepack
```

关联回归：

```powershell
npm.cmd run test:buildgraphcore
npm.cmd run test:webexport
npm.cmd run test:androidexport
npm.cmd run test:iosexport
```

