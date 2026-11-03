# Build Report Panel v2 方案 B

## 需求目标

让 Build Report Panel 不只显示导出路径，而是能展示最近一次导出的真实构建摘要。

本轮采用方案 B：

```text
BuildService 持有最近一次导出报告摘要。
BuildReportPanel 展示 latestBuildExport.buildReportSummary。
不读取本地 JSON。
不浏览历史构建。
不新增任意 readFile bridge。
```

## 框架定位

该需求属于 Build / Export Pipeline 与编辑器 UI 的只读连接：

```text
Build Graph
  -> exportWindowsGameBuild
  -> Electron main
  -> BuildService
  -> App state
  -> BuildReportPanel
```

相关主文档：

```text
07-Build-Export-Pipeline.md
20-编辑器工程结构.md
30-Build-Graph-Core-v2.md
35-Build-Report-读取通道决策.md
```

## 为什么选择方案 B

之前 Build Report Panel v2 想直接读取报告 JSON，但这个动作会碰到 Electron 安全边界：

```text
Renderer 不能直接读本地文件。
不能为了 UI 方便打开 nodeIntegration。
不能新增任意 fs.readFile。
不能让 UI 变成本地文件执行器。
```

因此本轮采用更稳的方案 B：

```text
导出时 Build Graph 已经产生 buildReport。
Electron main 从导出结果中提取 buildReportSummary。
前端只展示这次导出返回的摘要。
```

这避免了历史报告读取和路径安全策略问题。

## 执行方式

实现内容：

```text
electron/main.cjs 回传 buildReportSummary。
src/services/buildService.ts 定义 BuildReportSummary / BuildExportResult。
src/services/buildService.ts 提供 createBuildReportSummary 纯函数。
src/editor/build/BuildReportPanel.tsx 只读展示 summary。
src/editor/console/ConsoleDock.tsx 适配导出摘要显示。
src/App.tsx 保存 latestBuildExport。
```

## 修改文件

```text
package.json
electron/main.cjs
src/services/buildService.ts
src/editor/build/BuildReportPanel.tsx
src/editor/console/ConsoleDock.tsx
src/App.tsx
scripts/test-build-service.cjs
框架设计/引擎总体架构/20-编辑器工程结构.md
框架设计/引擎总体架构/施工文档/历史/34-当前剩余可直接实现清单与施工顺序.md
框架设计/引擎总体架构/35-Build-Report-读取通道决策.md
```

## 验证方式

新增测试命令：

```powershell
npm.cmd run test:buildservice
```

本轮执行：

```powershell
npm.cmd run test:buildservice
npm.cmd run test:buildreportindex
npm.cmd run test:buildgraphcore
npm.cmd run build
npx.cmd playwright test ui-smoke
```

验证点：

```text
export result 可以携带 buildReportSummary。
BuildReportPanel 可以展示 summary。
缺失 summary 不会崩溃。
UI 不读取本地 JSON 文件。
没有新增任意文件读取 bridge。
```

## 保留边界

本需求没有做：

```text
不支持历史构建报告浏览。
不读取本地 build-report-index.json。
不新增 Electron readFile bridge。
不改变 Build Graph 阶段。
不自动应用 AI repair candidate。
```

## 当前结论

Build Report Panel v2 的第一版已经完成。

它现在能看最近一次导出的摘要，但如果要查看历史构建报告，需要进入下一轮架构确认：

```text
方案 A：Electron bridge 只读读取历史 Build Report JSON。
必须定义路径 allowlist 和文件名 allowlist。
不能开放任意 fs.readFile。
```

