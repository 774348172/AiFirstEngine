# 当前可自动化施工文档：Asset Pipeline ProjectDock / Console UI 接入

本文档只记录本轮可以直接施工的任务，不承载长期规则。长期规则见：

```text
25-Asset-DB-Importer-MVP.md
```

## 目标

把已经完成的 Asset Pipeline v1 Importer / Registry 闭环接入真实编辑器 UI：

```text
AssetRegistry -> ProjectDock
ImportReport / BatchImportReport / RegistryReport -> ConsoleDock
Console issue -> ProjectDock asset selection
```

本轮目标不是做完整资源编辑器，而是让用户和 AI 能在编辑器里看到真实资源状态、导入结果和 Registry 问题。

## 市场方案参考

```text
Unity：
ProjectBrowser 负责简洁资源浏览。
AssetDatabase / AssetImporter 是资源真相。
ConsoleWindow / Import Activity 展示导入错误。

Unreal Engine：
Content Browser 通过 ContentBrowserDataSubsystem / AssetRegistry 读取资源。
MessageLog / OutputLog 展示结构化问题。

Bevy：
AssetServer / AssetEvent / LoadState 提供运行时资源状态。
没有重编辑器 UI，但事件和状态边界清晰。

Godot：
FileSystem Dock 展示资源。
Import Dock / Output 展示导入设置和导入错误。
```

本轮采用：

```text
UE 数据源方式 + Unity 简洁 Project/Console 交互。
ProjectDock 读 Registry 派生 ViewModel。
ConsoleDock 读 Report 派生 Issue ViewModel。
```

## 当前已有输入

代码侧已经存在：

```text
src/engine/assetPipeline.ts
  AssetPipelineDatabase
  AssetRegistry
  AssetRegistryEntry
  ImportReport
  BatchImportReport
  RegistryReport
  getRegistryByGuid / getRegistryByType / getRegistryByState
  findMissingRegistryAssets / findFailedRegistryAssets / findModifiedRegistryAssets

src/engine/assetPipelineFileSync.ts
  applyFileChangeBatch

src/editor/project/ProjectDock.tsx
  当前主要读取 project.assets

src/editor/console/ConsoleDock.tsx
  当前主要显示 logs / AssetImportTask / Build / Trace / AI
```

## 施工边界

本轮必须做：

```text
1. 新增 Asset Pipeline UI ViewModel 层。
2. ProjectDock 接入 AssetRegistry 派生资源列表。
3. ConsoleDock 接入 ImportReport / BatchImportReport / RegistryReport 摘要。
4. Console issue 可以形成 jump target，并能驱动 ProjectDock 选中对应 asset。
5. 新增最小自动化测试。
6. package.json 增加独立测试脚本。
```

本轮不做：

```text
真实 OS file watcher。
真实 GPU 上传。
Bundle 二进制打包。
平台 cook。
完整 Inspector importer setting UI。
真实缩略图缓存。
资源自动修复执行。
ProjectDock 高级搜索语法。
虚拟滚动和大规模资源性能优化。
```

## 推荐新增文件

```text
src/editor/asset-pipeline/assetPipelineUiViewModel.ts
scripts/test-asset-pipeline-ui.cjs
```

如果现有目录结构不适合，可以放在更贴近现有代码的位置，但必须保持：

```text
ViewModel 不依赖 React。
ViewModel 不读取 DOM。
ViewModel 输入是 AssetRegistry / ImportReport / BatchImportReport / RegistryReport。
ViewModel 输出是 ProjectDock / ConsoleDock 可直接渲染的纯数据。
```

## 数据结构要求

第一版 `EditorAssetViewModel` 至少包含：

```text
assetGuid
assetId
name
type
sourcePath
folder
state
dependencies
referencers
lastImportReportId
badges
```

第一版 `EditorAssetIssueViewModel` 至少包含：

```text
issueId
severity
source
message
assetGuid?
assetId?
sourcePath?
jumpTarget?
suggestedAction?
```

第一版 `EditorAssetPipelinePanelViewModel` 至少包含：

```text
assets
issues
summary
registryVersion
lastBatchReportId?
```

## ProjectDock 施工规则

```text
ProjectDock 优先读取 AssetRegistry 派生 assets。
如果没有 registry，允许 fallback 到 project.assets，但 UI 必须显示 registry missing 状态。
folder / type / search / state filter 只作用于 ViewModel，不直接扫描项目文件。
资源卡片显示状态角标。
selectedAssetId 继续作为外部选择入口。
如果 Console jump target 提供 assetId / assetGuid，ProjectDock 必须能选中并滚动到对应资源。
```

第一版不要求完整右键菜单，不要求拖拽，不要求真实缩略图。

## ConsoleDock 施工规则

```text
ConsoleDock 增加 Asset Pipeline 报告摘要区域。
ConsoleDock 可以继续显示 AssetImportTask，但不能只显示任务。
ImportReport / BatchImportReport / RegistryReport 必须能形成结构化 issue。
issue 显示 severity / source / message / asset name or sourcePath。
issue 点击后调用 onJumpToIssue / onSelectAsset 一类回调，跳转 ProjectDock。
AI 修复入口只显示 report-only 修复候选，不自动执行。
```

第一版不要求新建独立 Console tab；可以在 Console / AI 面板内复用现有区域。

## App 接线规则

```text
App 层保存或派生当前 AssetRegistry / reports。
App 层把 registry / reports 传给 ProjectDock / ConsoleDock。
App 层负责处理 Console issue jump target。
ProjectDock / ConsoleDock 不直接调用 importer。
ProjectDock / ConsoleDock 不直接修改 AssetPipelineDatabase。
```

## 测试要求

必须新增：

```text
scripts/test-asset-pipeline-ui.cjs
```

并在 `package.json` 增加：

```text
"test:assetpipelineui": "node scripts/test-asset-pipeline-ui.cjs"
```

测试至少覆盖：

```text
1. AssetRegistry -> EditorAssetViewModel。
2. current / modified / missing / failed asset 生成正确状态和 badge。
3. folder / type / state / search filter 不改变原始 registry。
4. ImportReport / BatchImportReport / RegistryReport -> EditorAssetIssueViewModel。
5. issue 包含可跳转 jumpTarget。
6. Console issue jump target 可以映射到 ProjectDock selected asset。
7. registry missing 时生成明确 issue，而不是静默 fallback。
```

推荐运行：

```text
npm run test:assetpipelineui
npm run test:assetpipelineimporterregistry
npm run test:assetpipelineeditorbuild
npm run build
```

如果 `npm run build` 因现有无关问题失败，必须记录失败原因，不允许吞掉。

## 验收标准

```text
ProjectDock 不再只依赖 project.assets 展示资源。
ConsoleDock 能展示 Asset Pipeline 报告摘要。
报告问题能形成稳定 jump target。
ViewModel 是纯数据函数，可被 AI 和测试直接读取。
不引入第二套资源真相。
不引入新的自动修复执行路径。
所有新增测试通过。
```

## 完成后必须更新

施工完成后必须：

```text
1. 将本文档移动到 施工文档/已完成。
2. 新增 阶段完成记录/2026-xx-xx-Asset-Pipeline-ProjectDock-Console-UI接入/00-总览.md。
3. 更新 49-当前状态与下一步入口.md。
4. 更新 54-当前可自动化施工总入口.md。
5. 更新 18-长期主义实现路径.md。
6. 更新 README.md 和 00-阅读顺序.md。
7. 如果施工中新增长期规则，回填 25-Asset-DB-Importer-MVP.md。
```
