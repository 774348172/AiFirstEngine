# 当前可自动化施工文档：Asset Pipeline v1 接入编辑器 / 文件同步 / Build Pipeline

本文档是下一轮可直接自动化施工入口。

长期规则以 `25-Asset-DB-Importer-MVP.md` 为准。本文档不重新设计 Asset Pipeline，只把已经完成的 `Asset Pipeline v1` 数据层接入到编辑器、文件同步和 Build Pipeline，形成更接近真实项目的资源闭环。

## 0. 施工定位

本轮目标：

```text
Asset Pipeline v1 数据层
  -> 编辑器导入任务入口
  -> 外部文件变更批处理模型
  -> Project / Asset DB 同步入口
  -> Build Pipeline 资源清单与依赖门禁
  -> Console / AI Debug 可读报告
```

本轮不是：

```text
真实 OS file watcher。
真实 GPU 上传。
真实贴图压缩 / 模型骨骼 / shader 编译。
真实 Bundle 二进制打包。
真实 Android / iOS 打包。
真实 AI Provider 资源生成。
真实 Native Editor OS window。
```

第一版允许用纯函数、内存数据和脚本测试模拟文件变更；但接口必须按长期路线设计，后续可以替换为真实 file watcher、真实 Import Worker 和真实 Build Worker。

## 1. 市场方案对比

| 引擎 | 对应机制 | 优点 | 本轮借鉴 | 本轮不照搬 |
|---|---|---|---|---|
| Unity | AssetDatabase / .meta / Importer / Refresh / OnPostprocessAllAssets / BuildPipeline | 导入、刷新、构建接入成熟，编辑器体验清晰 | 文件变化进入统一导入流程；导入完成后 Project 面板和构建读取同一套资源状态 | 不照搬黑盒 AssetDatabase 行为，必须输出 AI 可读 ImportReport / DependencyReport |
| UE | AssetRegistry / UFactory / Reimport / DDC / Cook | 大项目资源索引、依赖和 cook 管线强 | Build Pipeline 只读 Asset Registry / Artifact / Dependency，不直接扫描散文件 | 不实现完整 Cook、DDC 分布式缓存和复杂 Editor UI |
| Bevy | AssetServer / AssetProcessor / processed assets / dependency hash | Rust 化、hash 驱动、依赖变化重处理清晰 | 用 hash / dependencyHash 决定资源是否需要重新导入和重新构建 | 不把运行时 Handle 写回项目源数据 |
| Godot | .import / imported cache / export presets | 简洁直接 | 第一版保持导入边界简单 | 不采用弱诊断和弱依赖图路线 |

本项目取向：

```text
Unity-like 编辑器导入体验
+ UE-like registry / cook 边界
+ Bevy-like hash / processed asset 思路
+ AI-first structured reports
```

## 2. 本轮施工原则

```text
Asset Pipeline 是资源进入项目的唯一正式入口。
编辑器、文件同步、Build Pipeline 都读取同一个 AssetPipelineDatabase。
用户和 AI 不直接改底层 Asset DB。
外部文件变更必须先进入 FileChangeBatch，再生成 ImportPlan / ImportTransaction。
导入期间必须持有导入锁，普通 Project 修改和 AI Patch 不能并发写入。
Build Pipeline 不重新发明资源扫描逻辑，只消费 Asset Pipeline 的产物。
失败只生成报告，不自动删除文件、不自动改引用、不自动重导入。
每个阶段必须有最小测试。
```

## 3. 建议新增 / 修改文件

新增：

```text
src/engine/assetPipelineEditorBridge.ts
src/engine/assetPipelineFileSync.ts
src/engine/assetPipelineBuildBridge.ts
scripts/test-asset-pipeline-editor-build.cjs
```

可按需要修改：

```text
src/engine/assetPipeline.ts
src/engine/assetDatabase.ts
src/services/assetImportLockService.ts
src/services/buildService.ts
src/editor/project/ProjectDock.tsx
src/editor/console/ConsoleDock.tsx
src/asset-generation/assetImportRepairPlan.ts
package.json
```

不应修改：

```text
真实 Rust Runtime ECS。
真实 Renderer / RHI / RDG。
真实 Native Editor real-window。
真实平台打包器。
```

## 4. 数据结构边界

### FileChangeBatch v1

用于表示外部文件或编辑器导入产生的一批变化。

```ts
type FileChangeBatch = {
  schemaVersion: "asset-file-change-batch.v1";
  batchId: string;
  source: "manual_import" | "external_scan" | "ai_generated" | "build_preflight";
  changes: Array<{
    kind: "added" | "modified" | "deleted" | "moved";
    path: string;
    previousPath?: string;
    hash?: string;
    size?: number;
    mtimeMs?: number;
  }>;
};
```

规则：

```text
只记录文件事实，不做项目语义判断。
moved 优先保持原 guid；无法确认时生成 conflict report。
deleted 不物理删除项目引用，只把资源标记为 missing，并输出 affectedReferences。
```

### AssetPipelineEditorState v1

用于编辑器只读展示当前导入状态。

```ts
type AssetPipelineEditorState = {
  schemaVersion: "asset-pipeline-editor-state.v1";
  locked: boolean;
  activeTransactionId?: string;
  recentReports: ImportReport[];
  recentIssues: ImportDiagnostic[];
};
```

规则：

```text
Project 面板只展示状态，不直接修 Asset DB。
Console / AI Debug 读取 report，不自动执行修复。
```

### BuildAssetManifest v1

Build Pipeline 消费的最小资源清单。

```ts
type BuildAssetManifest = {
  schemaVersion: "build-asset-manifest.v1";
  target: string;
  platform: string;
  assets: Array<{
    assetId: string;
    guid: string;
    type: string;
    artifactId?: string;
    sourcePath: string;
    state: "current" | "modified" | "missing" | "failed";
    requiredByRuntime: boolean;
  }>;
  dependencies: DependencyRecord[];
  issues: ImportDiagnostic[];
};
```

规则：

```text
Build Graph 只读 BuildAssetManifest。
missing / failed 的 runtime required 资源必须让构建失败。
modified 资源可以触发 reimport plan，但不能静默使用旧 artifact。
editor-only 资源不阻塞 runtime build，除非被 runtime dependency 引用。
```

## 5. 自动化施工阶段

### A0：施工前基线

运行：

```powershell
npm.cmd run test:assetdb
npm.cmd run test:assetscan
npm.cmd run test:assetlock
npm.cmd run test:assetimportrepair
npm.cmd run test:assetpipeline
npm.cmd run test:buildgraph
npm.cmd run test:buildservice
```

验收：

```text
全部通过；如果已有失败，必须记录为施工前失败，不能混入本轮变更。
```

### A1：FileChangeBatch -> ImportPlan / ImportTransaction

实现：

```text
新增 assetPipelineFileSync。
把 added / modified / deleted / moved 转成可执行 import plan。
复用 Asset Pipeline v1 的 ImportTransaction。
deleted 生成 missing 状态和 report，不删除引用。
moved 在 guid 可确认时保持身份，否则生成 conflict。
```

最小测试：

```text
added texture 生成 import transaction。
modified texture 生成 reimport transaction。
deleted runtime texture 生成 missing issue 和 affected asset。
moved asset 保持 guid。
unknown moved source 生成 conflict report。
```

### A2：编辑器导入桥接

实现：

```text
新增 assetPipelineEditorBridge。
提供 beginEditorImport / applyEditorImportResult / createEditorAssetPipelineState。
导入期间接入 AssetImportLock。
ProjectDock 可读取 asset state / recent issue 的纯数据。
ConsoleDock 可读取 recent ImportReport。
AI Debug 只读取 failed report 和 repair candidate。
```

第一版 UI 不需要大改，只要能通过数据层证明：

```text
导入锁能阻止普通 project mutation。
导入任务自身可以受控 bypass 写入结果。
Project / Console / AI Debug 读取同一份 report。
```

最小测试：

```text
import running 时普通修改被拒绝。
import transaction commit 后锁释放。
failed import 进入 recentReports。
failed import 能生成 report-only repair candidate。
```

### A3：Build Pipeline 资源清单接入

实现：

```text
新增 assetPipelineBuildBridge。
从 AssetPipelineDatabase 生成 BuildAssetManifest。
BuildService / Build Graph 测试中接入 manifest preflight。
runtime required missing / failed 资源使构建失败。
artifact hit / miss / invalidated 进入 BuildReport。
dependency cycle 按 usage 区分 error / warning。
```

最小测试：

```text
所有资源 current 且 artifact valid -> build preflight passed。
runtime texture missing -> build preflight failed。
editor-only asset missing -> build preflight warning。
sourceHash changed -> artifact invalidated / reimport required。
dependencyHash changed -> affected asset rebuild required。
```

### A4：端到端 Golden Scenario

构造一个最小项目：

```text
Texture: player_ship.png
Material: player_ship.mat 引用 Texture
Prefab: player_ship.prefab 引用 Material
Scene: main.scene 引用 Prefab
Logic: shooter_rule.dsl-ir
```

执行流程：

```text
FileChangeBatch added
  -> ImportTransaction
  -> ImportResult
  -> ArtifactRecord
  -> DependencyGraph
  -> EditorState
  -> BuildAssetManifest
  -> Build preflight report
```

验收：

```text
导入成功。
DependencyGraph 能查到 Texture -> Material -> Prefab -> Scene 的受影响链。
BuildAssetManifest 包含 runtime required assets。
Build preflight passed。
修改 Texture hash 后，Material / Prefab / Scene 的 affected chain 可被报告。
```

### A5：文档与入口同步

施工完成后必须：

```text
把本文档从 施工文档/当前 移动到 施工文档/已完成。
新增 阶段完成记录/2026-06-24-Asset-Pipeline-Editor-Build接入/00-总览.md。
更新 49-当前状态与下一步入口.md。
更新 54-当前可自动化施工总入口.md。
更新 18-长期主义实现路径.md。
更新 施工文档/README.md。
更新 阶段完成记录/README.md。
如果新增长期规则，回填 25-Asset-DB-Importer-MVP.md。
```

## 6. 必须新增测试命令

package.json 中新增：

```json
{
  "test:assetpipelineeditorbuild": "node scripts/test-asset-pipeline-editor-build.cjs"
}
```

测试脚本必须覆盖：

```text
A1 FileChangeBatch。
A2 EditorBridge + ImportLock。
A3 BuildAssetManifest / preflight。
A4 Golden Scenario。
```

## 7. 最终验收命令

本轮完成前至少运行：

```powershell
npm.cmd run test:assetpipeline
npm.cmd run test:assetpipelineeditorbuild
npm.cmd run test:assetdb
npm.cmd run test:assetscan
npm.cmd run test:assetlock
npm.cmd run test:assetimportrepair
npm.cmd run test:buildgraph
npm.cmd run test:buildservice
npm.cmd run build
```

可选烟测：

```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

## 8. 完成标准

```text
Asset Pipeline v1 不再只是孤立数据层。
编辑器导入任务能通过 Asset Pipeline v1 产生状态和报告。
外部文件变化能进入统一 FileChangeBatch / ImportTransaction。
Build Pipeline 能读取 AssetPipelineDatabase / ArtifactStore / DependencyGraph。
Build preflight 能阻止 missing / failed runtime asset。
Console / AI Debug 有同一份结构化 report 可读。
所有新增能力都有最小自动化测试。
没有引入真实 GPU / 真实窗口 / 真实平台打包临时方案。
```

## 9. 风险与处理

```text
风险：把 FileChangeBatch 做成真实 watcher。
处理：第一版只做纯数据模型和脚本模拟，真实 watcher 另开施工文档。

风险：Build Pipeline 绕过 Asset Pipeline 继续扫描散文件。
处理：BuildAssetManifest 必须从 AssetPipelineDatabase 生成。

风险：UI 为了展示状态直接修改 Asset DB。
处理：Project / Console / AI Debug 只能读报告；修复必须另走 Patch Plan。

风险：资源缺失时自动修引用。
处理：本轮只 report-only，不自动修。
```
