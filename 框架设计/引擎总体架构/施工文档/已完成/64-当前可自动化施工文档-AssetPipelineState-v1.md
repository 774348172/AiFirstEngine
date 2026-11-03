# 当前可自动化施工文档：AssetPipelineState v1

> 状态：已确认可施工。本文档只定义本轮自动化施工范围，不继续扩展 Asset Pipeline 架构规则。长期规则以 `25-Asset-DB-Importer-MVP.md` 和 `20-编辑器工程结构.md` 为准。

## 1. 施工目标

本轮目标是把已经确认的 Asset Pipeline State 规则落到代码里：

```text
AssetPipelineState
  -> AssetPipelineSnapshot
  -> AssetPipelineEvent
  -> AssetPipelineController
  -> ProjectDock / ConsoleDock / AI / Inspector 后续只读入口
```

本轮不是重新设计 Importer，也不是实现真实 OS file watcher、真实 Bundle 打包或平台 cook。

核心结果：

```text
Editor Core / Native Editor Host 方向有一个正式 AssetPipelineState 数据模型。
legacy React App 可以读取 Snapshot / ViewModel，但不能持有 AssetRegistry / reports 真相。
导入过程中只暴露 progress / busy，不暴露半成品 registry。
每次状态变化产生递增 stateVersion、只读 Snapshot 和轻量 Event。
```

## 2. 参考引擎对比

| 引擎 | 对应机制 | 可借鉴点 | 本轮取舍 |
|---|---|---|---|
| Unity | AssetDatabase + ProjectBrowser + import callbacks | AssetDatabase 是资源查询入口，ProjectBrowser 只读显示 | 学中心状态和 UI 只读，不复制 Unity 内部黑盒 |
| UE | IAssetRegistry / FAssetRegistryState + Content Browser data source + Asset events | Registry 状态集中、可查询、可追加，Content Browser 不持有真相 | 学 State + query + event，不复制 UObject / Package 复杂度 |
| Bevy | AssetServer / Assets<T> + AssetEvent + LoadState | 状态和事件分离，事件只通知变化 | 学 lightweight event，不做完整 event sourcing |
| Godot | EditorFileSystem / Import Dock / Output | 文件视图、导入设置、错误输出分离 | 学 UI 分离，不采用弱诊断能力 |

本项目最终路线：

```text
Unity-like 中心资源数据库
+ UE-like registry / report 查询边界
+ Bevy-like state / event 分离
+ AI-first Snapshot / Report 可解释结构
```

## 3. 本轮边界

### 本轮做

```text
新增 AssetPipelineState v1 类型。
新增 AssetPipelineSnapshot v1 类型。
新增 AssetPipelineEvent v1 类型。
新增 AssetPipelineController 纯函数 / 工厂函数。
接入现有 AssetPipelineDatabase / AssetRegistry / ImportReport / BatchImportReport / RegistryReport。
实现 report trimming，默认最近 50 条。
实现 active import read gate。
实现 stateVersion 递增。
实现 Snapshot 只读派生。
实现 Event 只做通知，不做真相。
为 AssetPipelineState v1 增加最小测试。
```

### 本轮不做

```text
真实 OS file watcher。
真实 Import Worker 线程。
Native Editor Host 真实窗口接入。
Inspector 深度资源设置 UI。
真实本地文件持久化读写。
Bundle 二进制打包。
平台 cook。
完整 Event Sourcing。
把 Snapshot 历史长期保存。
```

原因：

```text
本轮要固定状态所有权和读写边界。
如果同时做 watcher、worker、持久化 IO 和 UI 深度接入，调试面会过大。
State / Snapshot / Event 稳定后，再接真实 Editor Core 和 Native Host。
```

## 4. 新增文件

```text
src/engine/assetPipelineState.ts
scripts/test-asset-pipeline-state.cjs
```

更新：

```text
package.json
```

新增测试命令：

```json
{
  "test:assetpipelinestate": "node scripts/test-asset-pipeline-state.cjs"
}
```

## 5. 数据结构

### AssetPipelineStatus

```ts
type AssetPipelineStatus = "ready" | "importing" | "failed" | "rebuilding";
```

### AssetPipelineState

```ts
type AssetPipelineState = {
  schemaVersion: "asset-pipeline-state.v1";
  stateVersion: number;
  status: AssetPipelineStatus;
  database: AssetPipelineDatabase;
  registry: AssetRegistry;
  reports: {
    importReports: ImportReport[];
    batchReports: BatchImportReport[];
    registryReports: RegistryReport[];
    maxReports: number;
  };
  activeImport: AssetPipelineActiveImport;
  lastCompletedImport?: AssetPipelineCompletedImport;
  updatedAt: string;
};
```

### AssetPipelineActiveImport

```ts
type AssetPipelineActiveImport = {
  busy: boolean;
  taskId?: string;
  stage?: string;
  progress?: number;
  startedAt?: string;
  readable: false;
};
```

规则：

```text
activeImport.readable 第一版固定 false。
导入期间 UI / AI 只能读 activeImportPublic。
导入期间不能读半成品 registry。
```

### AssetPipelineSnapshot

```ts
type AssetPipelineSnapshot = {
  schemaVersion: "asset-pipeline-snapshot.v1";
  stateVersion: number;
  databaseVersion: number;
  registryVersion: number;
  readable: boolean;
  status: AssetPipelineStatus;
  summary: AssetPipelineSnapshotSummary;
  registry?: AssetRegistry;
  recentReports: {
    importReports: ImportReport[];
    batchReports: BatchImportReport[];
    registryReports: RegistryReport[];
  };
  activeImportPublic?: {
    busy: boolean;
    taskId?: string;
    stage?: string;
    progress?: number;
    startedAt?: string;
  };
};
```

规则：

```text
Snapshot 是只读派生值。
Snapshot 不允许反向写回 AssetPipelineState。
Snapshot 不保存完整历史。
active import 期间 Snapshot.readable=false，registry 不暴露。
```

### AssetPipelineEvent

```ts
type AssetPipelineEvent = {
  schemaVersion: "asset-pipeline-event.v1";
  stateVersion: number;
  kind:
    | "import_started"
    | "import_progress"
    | "import_completed"
    | "import_failed"
    | "registry_rebuilt"
    | "reports_trimmed";
  changedAssetGuids: string[];
  reportIds: string[];
  timestamp: string;
};
```

规则：

```text
Event 只做刷新通知。
Event 不用于重建完整 State。
Event 不是数据库。
Event 可以给 UI / AI 解释最近变化。
```

## 6. 控制器函数

### A1：createAssetPipelineState

输入：

```text
database
registry
now?
maxReports?
```

输出：

```text
AssetPipelineState
```

规则：

```text
schemaVersion 固定 asset-pipeline-state.v1。
stateVersion 初始为 1。
status 初始 ready。
reports 初始为空。
activeImport.busy=false。
```

最小测试：

```text
createAssetPipelineState creates ready state with version 1.
state contains database and registry references.
snapshot from initial state is readable.
```

### A2：getAssetPipelineSnapshot

输入：

```text
state
```

输出：

```text
AssetPipelineSnapshot
```

规则：

```text
ready / failed / rebuilding 状态下可读。
importing 状态下 readable=false。
importing 状态下不输出 registry。
recentReports 只输出已裁剪后的 reports。
summary 包含 totalAssets / current / modified / missing / failed / blocked / reportCount。
```

最小测试：

```text
snapshot exposes registry when readable.
snapshot hides registry during active import.
snapshot summary matches registry state indexes.
```

### A3：beginAssetPipelineImport

输入：

```text
state
taskId
stage?
now?
```

输出：

```text
{ state, event }
```

规则：

```text
status -> importing。
stateVersion + 1。
activeImport.busy=true。
activeImport.readable=false。
event.kind=import_started。
不修改 database / registry。
```

最小测试：

```text
begin import increments stateVersion.
begin import emits import_started event.
snapshot during import is not readable and hides registry.
```

### A4：updateAssetPipelineImportProgress

输入：

```text
state
stage?
progress?
now?
```

输出：

```text
{ state, event }
```

规则：

```text
只能在 importing 状态调用。
progress clamp 到 0..1。
stateVersion + 1。
event.kind=import_progress。
不修改 database / registry / reports。
```

最小测试：

```text
progress update increments version.
progress is clamped.
progress event does not change registry.
```

### A5：completeAssetPipelineImport

输入：

```text
state
database
registry
importReports?
batchReports?
registryReports?
changedAssetGuids?
now?
```

输出：

```text
{ state, events }
```

规则：

```text
status -> ready。
stateVersion + 1。
activeImport.busy=false。
写入新的 database / registry。
追加 reports。
reports 超过 maxReports 时裁剪最旧项。
生成 import_completed event。
如果发生裁剪，额外生成 reports_trimmed event。
```

最小测试：

```text
complete import commits database and registry.
complete import exposes readable snapshot.
complete import trims reports to maxReports.
complete import emits import_completed and optional reports_trimmed.
```

### A6：failAssetPipelineImport

输入：

```text
state
importReports?
registryReports?
changedAssetGuids?
now?
```

输出：

```text
{ state, events }
```

规则：

```text
status -> failed。
stateVersion + 1。
activeImport.busy=false。
不替换 database / registry，保留旧有效状态。
追加失败 reports。
生成 import_failed event。
```

最小测试：

```text
failed import keeps previous database and registry.
failed import exposes readable old registry.
failed import appends reports.
```

### A7：rebuildAssetPipelineRegistryState

输入：

```text
state
registry
registryReport?
changedAssetGuids?
now?
```

输出：

```text
{ state, event }
```

规则：

```text
status -> ready。
stateVersion + 1。
替换 registry。
可追加 registryReport。
event.kind=registry_rebuilt。
不修改 database。
```

最小测试：

```text
registry rebuild increments version.
registry rebuild does not modify database.
registry rebuild emits registry_rebuilt.
```

### A8：createAssetPipelinePanelSnapshotInput

目的：

```text
给现有 createAssetPipelinePanelViewModel 提供统一输入。
```

规则：

```text
如果 snapshot.readable=true，输出 registry + reports。
如果 snapshot.readable=false，只输出 busy / progress，不输出 registry。
不让 ProjectDock / ConsoleDock 直接接触 AssetPipelineState。
```

最小测试：

```text
readable snapshot can feed panel view model.
unreadable snapshot does not expose registry.
```

## 7. 测试文件

新增：

```text
scripts/test-asset-pipeline-state.cjs
```

测试组织：

```text
1. create state / initial snapshot。
2. begin import / progress / hidden registry。
3. complete import / reports trim / readable snapshot。
4. failed import / old registry preserved。
5. registry rebuild / database unchanged。
6. panel snapshot input read gate。
```

必须执行：

```powershell
npm.cmd run test:assetpipelinestate
npm.cmd run test:assetpipeline
npm.cmd run test:assetpipelineimporterregistry
npm.cmd run test:assetpipelineeditorbuild
npm.cmd run test:assetpipelineui
npm.cmd run build
```

## 8. 完成标准

```text
src/engine/assetPipelineState.ts 存在。
AssetPipelineState / Snapshot / Event 类型完整。
所有 controller 函数为纯函数或无外部 IO 函数。
active import 期间 Snapshot 不暴露 registry。
complete / fail / rebuild 都递增 stateVersion。
Event 不承载完整数据库。
reports 默认最多保留 50 条。
现有 Asset Pipeline 测试不退化。
文档不再把 ProjectDock / ConsoleDock 描述成资源真相持有者。
```

## 9. 失败处理

如果实现中发现现有 `AssetPipelineEditorState` 与 `AssetPipelineState` 重叠：

```text
不要删除 AssetPipelineEditorState。
先让 AssetPipelineEditorState 成为 Snapshot 派生的 legacy UI view state。
不得让它继续演变成第二套资源真相。
```

如果实现中发现需要真实持久化 IO：

```text
暂停 IO 实现。
本轮只保留 serialize / deserialize 纯函数或不实现持久化函数。
真实 Library/Reports/AssetPipeline 文件落盘另开施工文档。
```

如果实现中发现 UI 需要直接读 State：

```text
拒绝直接接入。
新增 Snapshot -> ViewModel 适配函数。
UI 只能读 Snapshot / ViewModel。
```

## 10. 本轮之后

本轮完成后，下一步优先级：

```text
1. Inspector 接入 Asset Pipeline importer setting / dependencies / referencers。
2. Native Editor Host 接入 AssetPipelineState Snapshot。
3. 真实 OS file watcher / Import Worker。
4. Library/Reports/AssetPipeline 持久化 IO。
```
