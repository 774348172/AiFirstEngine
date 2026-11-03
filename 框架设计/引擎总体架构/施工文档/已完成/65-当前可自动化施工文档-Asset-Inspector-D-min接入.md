# 当前可自动化施工文档：Asset Inspector D-min 接入

> 状态：已确认可施工。本文档只定义本轮自动化施工范围，不继续扩展 Asset Pipeline 架构规则。长期规则以 `25-Asset-DB-Importer-MVP.md` 和 `20-编辑器工程结构.md` 为准。

## 1. 施工目标

本轮目标是把已经确认的 Asset Editor / Asset Inspector D-min 规则接入当前编辑器：

```text
ProjectDock selected asset
  -> EditorSelectionState asset target
  -> AssetPipelineSnapshot
  -> AssetInspectorViewModel
  -> Inspector asset mode render
  -> AssetActionPanel low-risk direct command / high-risk request placeholder
```

本轮不是实现完整资源编辑器，也不是实现真实 reimport / repair / importer setting 修改。

核心结果：

```text
用户在 ProjectDock 选择资源后，Inspector 可以显示这个资源的 identity / importer / dependencies / referencers / reports / actions。
Asset Inspector 只读取 AssetPipelineSnapshot / AssetInspectorViewModel。
Asset Inspector 不直接写 Asset DB / AssetRegistry。
高风险动作只生成结构化 request placeholder，不执行真实写入。
```

## 2. 参考引擎对比

| 引擎 | 对应机制 | 可借鉴点 | 本轮取舍 |
|---|---|---|---|
| Unity | ProjectBrowser + Selection.objects + Inspector + AssetImporterEditor | 选资源后 Inspector 直接显示资源和 Importer，交互简单 | 学选择到 Inspector 的直观链路，不复制黑盒 AssetDatabase 行为 |
| UE | Content Browser + Details Panel / Asset Editor + AssetRegistry + FAssetEditorToolkit | Content Browser 负责选择，Details / Asset Editor 显示属性，Reimport 交给工具系统 | 学数据源和执行边界，不做完整 UObject / Toolkit |
| Bevy | AssetServer / Assets<T> / AssetEvent / LoadState | 状态和事件分离，资源状态可查询 | 学只读状态和事件分离，不把 Bevy runtime asset 当编辑器 Inspector |
| Godot | FileSystem Dock + Inspector / Import Dock | 资源浏览和导入面板分开，第一版较轻 | 学轻量 UI，不采用弱依赖诊断路线 |

本项目路线：

```text
Unity-like 简洁选择体验
+ UE-like 资源真相和执行边界
+ Bevy-like Snapshot / Event 分离
+ AI-first AssetInspectorViewModel / Request / Plan / Transaction
```

## 3. 本轮边界

### 本轮做

```text
新增 AssetInspectorViewModel v1 纯函数。
扩展 Inspector mode，支持 asset target。
ProjectDock 选中资源后能驱动 Inspector 显示 asset mode。
Inspector asset mode 显示 identity / importer / dependencies / referencers / reports / actions。
低风险动作支持 direct callback placeholder：Locate / Copy guid / Open report。
高风险动作只生成 request placeholder：Request Reimport / Create AI Repair Candidate。
新增最小测试，覆盖 ViewModel、空态、缺失依赖、报告摘要、动作分类。
更新 package.json 测试命令。
```

### 本轮不做

```text
真实 Importer setting 修改。
Apply / Revert。
真实 Reimport 执行。
真实 AI Repair 执行。
多资源同时编辑。
完整资源预览器。
独立 Asset Editor 窗口。
材质 / 模型 / 动画专用编辑器。
真实 OS file watcher。
真实文件系统扫描触发。
```

原因：

```text
本轮固定 Asset Inspector 的只读展示和动作边界。
如果同时做真实 reimport、repair、setting 修改和预览器，调试面过大，容易破坏 Asset DB 真相边界。
```

## 4. 新增 / 修改文件

建议新增：

```text
src/editor/asset-pipeline/assetInspectorViewModel.ts
scripts/test-asset-inspector-view-model.cjs
```

建议修改：

```text
src/editor/inspector/Inspector.tsx
src/editor/project/ProjectDock.tsx
src/editor/asset-pipeline/assetPipelineUiViewModel.ts
src/engine/assetPipelineState.ts
package.json
```

如果实现时发现现有 App selection 状态在 `src/App.tsx`，允许做最小接线，但不能把 Asset DB 真相搬进 App。

## 5. 数据结构

### AssetInspectorViewModel

```ts
type AssetInspectorViewModel = {
  schemaVersion: "asset-inspector-view-model.v1";
  selectedAssetId: string | null;
  emptyReason?: "no-selection" | "asset-not-found" | "asset-pipeline-busy";
  identity?: AssetInspectorIdentityViewModel;
  importer?: AssetInspectorImporterViewModel;
  dependencies: AssetInspectorDependencyViewModel[];
  missingDependencies: AssetInspectorDependencyViewModel[];
  referencers: AssetInspectorReferencerViewModel[];
  reports: AssetInspectorReportViewModel[];
  actions: AssetInspectorActionViewModel[];
};
```

### identity

```ts
type AssetInspectorIdentityViewModel = {
  assetId: string;
  guid: string;
  type: string;
  sourcePath: string;
  displayName: string;
  state: string;
};
```

### importer

```ts
type AssetInspectorImporterViewModel = {
  importerId: string;
  importerVersion: string;
  sourceHash?: string;
  settingsHash?: string;
  artifactId?: string;
};
```

### actions

```ts
type AssetInspectorActionViewModel = {
  actionId: string;
  label: string;
  risk: "low" | "high";
  commandKind:
    | "locate-in-project"
    | "copy-guid"
    | "open-report"
    | "request-reimport"
    | "create-ai-repair-candidate";
  enabled: boolean;
  disabledReason?: string;
};
```

## 6. ViewModel 生成规则

```text
createAssetInspectorViewModel(snapshot, selectedAssetId)
```

规则：

```text
selectedAssetId 为空 -> emptyReason = no-selection。
snapshot.status 为 importing / rebuilding -> emptyReason = asset-pipeline-busy，并返回最小 locked action。
找不到 asset -> emptyReason = asset-not-found。
找到 asset -> 填充 identity / importer / dependencies / referencers / reports / actions。
dependencies 只展示直接依赖。
referencers 只展示直接反向引用。
reports 只展示与 selectedAssetId 相关的最近报告摘要。
actions 必须区分 low / high risk。
```

禁止：

```text
ViewModel 生成函数不读取文件系统。
ViewModel 生成函数不调用 Importer。
ViewModel 生成函数不修改 AssetPipelineState。
ViewModel 生成函数不读取 ProjectDock UI 内部状态。
```

## 7. Inspector UI 规则

第一版显示为简洁分组：

```text
Asset
Importer
Dependencies
Referencers
Reports
Actions
```

UI 行为：

```text
没有选择资源时显示空态。
资源不存在时显示 asset-not-found。
导入占用态显示 locked message。
依赖缺失必须在 Dependencies 中标记。
报告 issue 点击可以触发 open-report callback placeholder。
Locate in ProjectDock / Copy guid 属于 low-risk direct command。
Request Reimport / Create AI Repair Candidate 属于 high-risk request placeholder。
```

## 8. 动作边界

低风险动作：

```text
Locate in ProjectDock
Copy guid
Open report
Switch Inspector tab
```

允许直接调用 UI callback。

高风险动作：

```text
Request Reimport
Create AI Repair Candidate
Importer setting 修改
Delete / Move / Rename / ReplaceRef
Batch operation
```

本轮只做前两个高风险动作的 request placeholder：

```text
AssetActionRequest
  requestId
  actionKind
  targetAssetId
  source = "asset-inspector"
  reason
```

不执行真实 ImportTransaction / Patch Transaction。

## 9. 测试要求

新增：

```text
scripts/test-asset-inspector-view-model.cjs
```

新增 package script：

```json
{
  "test:assetinspector": "node scripts/test-asset-inspector-view-model.cjs"
}
```

最小测试用例：

```text
1. no-selection 返回 emptyReason = no-selection。
2. asset-not-found 返回 emptyReason = asset-not-found。
3. 正常 asset 返回 identity / importer / actions。
4. missing dependency 被归入 missingDependencies。
5. referencers 只显示 directReferencers。
6. reports 只显示相关 asset issue。
7. low-risk / high-risk actions 分类正确。
8. importing 状态下显示 asset-pipeline-busy，不暴露半成品详情。
```

回归命令：

```powershell
npm.cmd run test:assetinspector
npm.cmd run test:assetpipelineui
npm.cmd run test:assetpipelinestate
npm.cmd run test:assetpipeline
npm.cmd run build
```

## 10. 验收标准

```text
AssetInspectorViewModel 是纯函数，测试可独立运行。
Inspector 支持 asset mode。
ProjectDock 选择资源能让 Inspector 进入 asset mode。
Asset Inspector 不直接写 Asset DB / AssetRegistry。
高风险动作不执行真实资源写入，只生成 request placeholder。
所有新增测试通过。
build 通过。
```

## 11. 完成后文档同步

完成施工后必须：

```text
1. 将本文档移动到 施工文档/已完成。
2. 新增 阶段完成记录/2026-xx-xx-Asset-Inspector-D-min接入/00-总览.md。
3. 更新 49-当前状态与下一步入口.md。
4. 更新 54-当前可自动化施工总入口.md。
5. 更新 18-长期主义实现路径.md。
6. 更新 00-阅读顺序.md。
7. 如果实现中改变长期规则，回填 25-Asset-DB-Importer-MVP.md / 20-编辑器工程结构.md。
```
