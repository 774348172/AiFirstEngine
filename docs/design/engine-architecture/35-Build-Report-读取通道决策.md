# Build Report 读取通道决策

本文档用于确认 Build Report Panel v2 的读取通道。

当前文档适用范围：

```text
本文档只约束 legacy Electron / React shell 下的 Build Report 读取通道。
Native Editor Host 主线应通过 Native FileService / BuildService 读取受控报告。
Electron bridge 不是长期主线，只是 legacy transition shell 的过渡能力。
```

当前已经确认：

```text
第一版采用方案 B：最近一次导出的 buildReportSummary。
下一版采用受控历史报告浏览：Electron bridge 只读读取当前项目 exports 下的白名单报告 JSON。
```

核心规则：

```text
需要支持历史构建报告浏览。
必须受控读取。
不能开放通用 readFile。
Build Report Panel 读取 Evidence Index / BuildReportSummary。
AI 只能基于用户选中的报告生成修复候选。
```

## 当前状态

当前编辑器已有：

```text
BuildReportPanel 轻量 UI
BuildReportIndex 数据层
Build Graph 输出多份 JSON 报告
Windows 导出返回 outputDir / executable / manifest / report 路径
```

当前缺口：

```text
BuildReportPanel 只能展示路径。
Renderer 前端不能直接读取本地 JSON 文件。
Electron preload 当前只暴露 exportWindowsGame / exportReplayDebugArchive。
BuildService 当前只是 Windows 导出的服务门面。
```

相关代码：

```text
electron/main.cjs
electron/preload.cjs
src/services/buildService.ts
src/editor/build/BuildReportPanel.tsx
scripts/build-graph.cjs
scripts/build-report-index.cjs
```

重要事实：

```text
scripts/build-graph.cjs 的 exportWindowsGameBuild 已经返回 buildReport 对象。
electron/main.cjs 当前只回传 outputDir / executable / manifest / report，丢弃了 buildReport。
```

## 为什么不能直接在 UI 里读文件

不能让 BuildReportPanel 直接读取本地 JSON。

原因：

```text
Renderer 运行在隔离环境，不能直接访问 Node fs。
临时打开 nodeIntegration 会破坏 Electron 安全边界。
在 UI 中拼路径和读本地文件会让 UI 变成文件系统执行器。
这会违反“Build 面板只读观察，BuildService / Electron bridge 管本地能力”的边界。
```

因此 Build Report Panel v2 暂停是正确的。

## 目标

Build Report Panel v2 应该做到：

```text
展示真实构建报告摘要。
展示 diagnostics summary。
展示 size / size budget summary。
展示 ai repair candidate summary。
缺失报告显示 warning。
只读，不自动修复。
不改变 Build Graph 输出结构。
不让 UI 直接读本地文件。
```

## 方案对比

### 方案 A：Electron bridge 只读读取报告 JSON

路线：

```text
BuildReportPanel
  -> BuildService
  -> window.gameExporter.readBuildReport(manifestPath)
  -> ipcMain
  -> scripts/build-report-index.cjs / fs.readFile
  -> BuildReportIndex JSON
```

优点：

```text
能读取任意历史导出目录。
适合用户手动选择 manifest 后查看旧报告。
复用 BuildReportIndex。
长期能力完整。
```

缺点：

```text
需要新增 Electron bridge。
需要定义路径安全规则，防止任意文件读取。
需要确认允许读取的目录范围，如 workspace exports / 当前 build outputDir。
涉及本地文件系统安全边界，不能算纯 UI 小改。
```

适用场景：

```text
后续正式 Build Report Browser。
查看历史构建。
打开外部导出目录。
```

### 方案 B：BuildService 持有最近一次导出报告摘要

路线：

```text
exportWindowsGameBuild
  -> 返回 buildReport / buildReportSummary
electron/main.cjs
  -> 回传 latestBuildReportSummary
App.tsx
  -> latestBuildExport
BuildReportPanel
  -> 只读展示 latestBuildExport.summary
```

优点：

```text
不需要新增文件读取能力。
不需要 Renderer 读本地 JSON。
实现最小，风险低。
只展示最近一次导出，符合当前 BuildReportPanel 的轻量定位。
可以利用 exportWindowsGameBuild 已经返回 buildReport 的事实。
```

缺点：

```text
不能查看历史构建。
如果用户重启编辑器，内存态摘要会消失。
只能展示导出时回传的摘要，不是通用报告浏览器。
```

适用场景：

```text
Build Report Panel v2 的第一版。
只服务最近一次导出。
不改变本地文件系统边界。
```

### 方案 C：Build Graph 额外生成前端可消费 summary

路线：

```text
Build Graph
  -> build-report-summary.json
  -> manifest 引用 summary
BuildService / Electron
  -> 读取或返回 summary
BuildReportPanel
  -> 展示 summary
```

优点：

```text
报告摘要成为正式构建产物。
可被编辑器、AI、外部工具复用。
摘要结构稳定，避免 UI 理解所有底层报告。
```

缺点：

```text
需要新增 Build Graph 阶段或 manifest 字段。
会改变 Build Graph 输出结构。
需要重新确认报告 schema。
当前会超过“轻量 UI v2”的范围。
```

适用场景：

```text
Build Graph 报告系统正式化。
后续 AI 自动查错统一入口。
与 Evidence Index 深度结合。
```

## 推荐决策

推荐采用分阶段路线：

```text
第一步：方案 B
第二步：受控方案 A
第三步：方案 C
```

### 第一版采用方案 B

理由：

```text
最符合当前 BuildReportPanel 的轻量定位。
不新增 Renderer 文件读取权限。
不改变 Build Graph 输出结构。
不需要安全路径策略。
可以直接利用 exportWindowsGameBuild 已有 buildReport 返回值。
```

第一版允许做：

```text
electron/main.cjs 回传 buildReportSummary。
src/services/buildService.ts 定义 BuildReportSummary 类型。
src/editor/build/BuildReportPanel.tsx 只读展示 summary。
tests/ui-smoke.spec.ts 覆盖 Build tab summary 文案。
```

第一版不做：

```text
不读取历史构建。
不新增 readFile bridge。
不允许用户输入任意 manifest 路径。
不自动修复。
不改变 Build Graph 阶段。
```

### 第二版采用受控方案 A

已确认采用受控历史报告浏览。

目标：

```text
用户可以查看历史构建报告。
AI 可以基于用户选中的历史报告分析问题。
编辑器不能变成任意本地文件读取器。
AI 不能自动扫描磁盘。
读取报告不触发修复。
```

正式路线：

```text
BuildReportPanel
  -> BuildService
  -> window.gameExporter.readBuildReport(request)
  -> electron preload
  -> ipcMain
  -> BuildReportReader
  -> allowed reports JSON
  -> BuildReportSummary / EvidenceSummary
  -> BuildReportPanel
```

边界：

```text
Renderer 不直接读本地文件。
BuildService 不暴露任意文件读取。
Electron bridge 只提供 readBuildReport。
readBuildReport 只读当前项目 exports 下的构建报告。
返回结构化 summary / evidence，不返回任意文件内容。
```

## 受控历史报告浏览规则

### 路径范围

只允许读取当前项目导出目录下的报告。

允许：

```text
<projectRoot>/exports/**
当前 BuildService 记录过的 latest outputDir
Build Manifest 引用的同目录报告
```

不允许：

```text
用户输入任意磁盘路径
读取项目根目录外的文件
读取系统目录
读取用户 home 目录下任意文件
读取 node_modules
读取源码目录下任意 JSON
跟随符号链接跳出 exports
```

路径校验规则：

```text
所有输入路径必须 resolve 成绝对路径。
resolve 后必须仍在 projectRoot/exports 下。
manifest 引用的报告路径必须再次 resolve 并校验。
禁止 .. 路径逃逸。
禁止符号链接逃逸。
路径校验失败时返回 blocking diagnostic，不读取文件。
```

### 文件白名单

只允许读取 Build Graph 产出的报告文件。

第一版白名单：

```text
build-manifest.json
build-report-index.json
build-diagnostics-report.json
ai-build-repair-plan.json
size-report.json
size-budget-report.json
bundle-pack-report.json
evidence-index.json
```

后续新增报告必须满足：

```text
由 Build Graph stage 生成。
被 build-manifest.json 或 build-report-index.json 引用。
有 schemaVersion。
有明确用途。
加入白名单文档。
有测试覆盖。
```

不允许读取：

```text
任意 .json
package.json
tsconfig.json
用户自定义未注册报告
AI 自己拼出来的路径
```

### 文件大小与解析安全

每个报告文件必须设置读取上限。

建议第一版：

```text
单个 JSON 最大 5 MB。
单次读取最多 16 个报告文件。
超过上限返回 warning / blocking issue。
```

解析规则：

```text
只按 JSON 解析。
不执行报告中的任何脚本或表达式。
必须校验 schemaVersion。
未知 schemaVersion 返回 unsupported report。
字段缺失时降级展示 warning，不崩溃。
```

### BuildReportReader 职责

新增读取层应独立于 UI。

职责：

```text
校验请求。
校验路径。
读取 manifest。
读取白名单报告。
校验 schemaVersion。
生成 BuildReportSummary。
生成 EvidenceSummary。
返回 diagnostics / warnings。
```

不负责：

```text
自动修复。
自动应用 Patch。
自动删除构建产物。
扫描整个磁盘。
解释任意未知 JSON。
直接驱动 UI。
```

### BuildService 职责

BuildService 负责连接 UI 和 Electron bridge。

职责：

```text
发起 readBuildReport 请求。
接收 BuildReportSummary / EvidenceSummary。
把读取失败转换成 UI 可展示错误。
为 AI 面板提供当前用户选中的 evidence。
```

不负责：

```text
直接 fs.readFile。
路径安全底层判断。
自动修复构建问题。
自动扫描历史构建。
```

### BuildReportPanel 职责

BuildReportPanel 是只读观察面板。

允许：

```text
展示最近一次构建摘要。
展示历史构建摘要。
展示 diagnostics / size / repair candidate 摘要。
展示缺失报告 warning。
让用户选择某份报告作为 AI evidence。
```

不允许：

```text
直接读取本地文件。
直接应用 AI repair candidate。
直接修改项目。
直接写 Build Graph。
直接删除或移动构建产物。
```

## AI 使用历史构建报告的权限规则

AI 可以使用历史构建报告，但必须满足：

```text
用户已经打开或选择该报告。
报告来自受控 readBuildReport 通道。
报告进入 EvidenceSummary。
AI 只读取 summary / evidence，不读取任意本地文件。
```

AI 可以做：

```text
解释构建失败原因。
总结包体问题。
根据 diagnostics 生成 repair candidate。
根据 ai-build-repair-plan 生成 ProjectPatchPlan 草案。
提示用户需要人工决策的问题。
```

AI 不可以做：

```text
自动扫描 exports 以外的文件。
自动读取用户电脑上的其它 JSON。
自动应用修复。
自动删除资源。
自动改 Build Graph。
绕过用户确认。
```

AI 修复流程：

```text
用户选择历史报告
  -> BuildService 读取受控 Evidence
  -> AI 分析 Evidence
  -> AI 生成 Repair Candidate 或 Patch Plan 草案
  -> ValidationService 验证
  -> 用户确认
  -> PatchService 应用
```

## 受控方案 A 的实现路线

### 阶段 A1：读取最近一次导出目录的历史报告

目标：

```text
不允许用户输入任意路径。
只读取 latestBuildExport.outputDir 下的报告。
```

实现：

```text
electron/preload.cjs 暴露 readBuildReport。
electron/main.cjs 实现 ipcMain handler。
BuildReportReader 校验 outputDir 在 projectRoot/exports 下。
BuildReportReader 读取白名单文件。
BuildService 返回 BuildReportSummary / EvidenceSummary。
BuildReportPanel 展示历史详情。
```

测试：

```powershell
npm.cmd run test:buildservice
npm.cmd run test:buildreportindex
npm.cmd run test:buildgraphcore
npm.cmd run build
npx.cmd playwright test ui-smoke
```

新增测试应覆盖：

```text
允许读取 exports 下的白名单报告。
拒绝 exports 外路径。
拒绝非白名单文件。
拒绝路径逃逸。
缺失报告不崩溃。
未知 schemaVersion 有 warning。
```

### 阶段 A2：构建历史列表

目标：

```text
用户可以在当前项目 exports 下选择历史构建。
```

实现规则：

```text
只扫描 projectRoot/exports 的一层或受控层级。
只识别包含 build-manifest.json 的目录。
不扫描整个项目。
不扫描系统目录。
构建历史列表只显示 buildId / target / platform / time / status。
```

### 阶段 A3：Evidence Index 统一入口

目标：

```text
Build Report Panel 和 AI Debug 都读取同一 EvidenceSummary。
```

实现规则：

```text
优先读取 evidence-index.json。
如果不存在，使用 build-report-index / diagnostics / repair-plan 派生 EvidenceSummary。
EvidenceSummary 是只读派生结果。
```

### 阶段 A4：AI 修复候选接入

目标：

```text
用户可以从历史构建报告中选择 AI repair candidate。
```

规则：

```text
候选默认 reportOnly。
生成 ProjectPatchPlan 前必须用户确认。
PatchPlan 必须再次通过 ValidationService。
不能从报告直接修改项目。
```

## 与方案 C 的关系

方案 C 不立即做，但作为长期方向保留：

```text
Build Graph 生成正式 build-report-summary.json。
Build Manifest 引用 summary。
Evidence Index 成为 AI 和 UI 的统一证据入口。
```

方案 A 和方案 C 不冲突：

```text
方案 A 解决“怎么安全读取历史报告”。
方案 C 解决“历史报告摘要应该由谁生成、如何稳定复用”。
```

## 当前确认后的结论

```text
需要做历史构建报告浏览。
legacy shell 必须通过受控 Electron bridge。
Native Editor Host 必须通过 Native FileService / BuildService。
不允许通用 readFile。
只允许读取当前项目 exports 下的白名单 Build Graph 报告。
BuildReportPanel 只读展示 summary / evidence。
AI 只能基于用户选中的报告生成修复候选。
所有修复必须走 Patch Plan / Validation / 用户确认。
```

### 第三版再做方案 C

前提：

```text
Build Report Summary schema 已经稳定。
Evidence Index 与 Build Graph 的关系已经确认。
需要让外部工具和 AI 也消费同一个 summary。
```

## BuildReportSummary v1 建议结构

第一版只做摘要，不搬运完整报告：

```text
schemaVersion: build-report-summary.v1
buildId
target
platform
status
outputDir
executable?
manifest?
report?
stages:
  total
  passed
  failed
  failedStage?
diagnostics:
  total?
  error?
  warning?
  info?
size:
  totalSourceBytes?
  bundledSourceBytes?
  sourceEstimate?: true
sizeBudget:
  issueCount?
  blocking?
  warning?
aiRepair:
  total?
  requiresUserApproval?
  canAutoApply?
warnings[]
```

数据来源：

```text
buildReport.stages
buildReport.status
buildReport.failedStage
buildReport.outputDir
buildReport.executable
buildReport.manifest
```

如果没有 diagnostics / size 细节：

```text
字段留空。
UI 显示“summary not available yet”。
不假装读取了完整 JSON。
```

## 测试要求

第一版方案 B 实现后必须跑：

```powershell
npm.cmd run test:buildreportindex
npm.cmd run test:buildgraphcore
npm.cmd run build
npx.cmd playwright test ui-smoke
```

如果新增 summary 纯函数，增加：

```powershell
npm.cmd run test:buildservice
```

测试必须覆盖：

```text
export result can carry buildReportSummary
summary is read-only
BuildReportPanel renders summary without reading local files
missing summary does not crash
```

## 当前结论

```text
Build Report Panel v2 方案 B 第一版已完成。
当前只展示最近一次导出的 buildReportSummary。
如果要查看历史构建报告，需要单独确认方案 A 的 Electron bridge 和路径安全策略。
如果要做方案 A 或 C，必须另开架构确认。
```

## 方案 B 实现记录

实现范围：

```text
electron/main.cjs 回传 buildReportSummary。
src/services/buildService.ts 定义 BuildReportSummary / BuildExportResult。
src/services/buildService.ts 提供 createBuildReportSummary 纯函数。
src/editor/build/BuildReportPanel.tsx 只读展示 summary。
不新增 readFile bridge。
不读取本地 JSON 文件。
不改变 Build Graph 阶段。
```

验证：

```powershell
npm.cmd run test:buildservice
npm.cmd run test:buildreportindex
npm.cmd run test:buildgraphcore
npm.cmd run build
npx.cmd playwright test ui-smoke
```
