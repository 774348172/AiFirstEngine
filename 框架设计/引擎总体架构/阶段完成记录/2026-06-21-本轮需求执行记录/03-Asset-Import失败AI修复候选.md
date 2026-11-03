# Asset Import 失败 AI 修复候选

## 需求目标

资源导入失败后，不只把错误留在控制台，而是生成 AI 可审查的修复候选。

目标链路：

```text
AssetImportTask failed
  -> AssetImportDiagnostic
  -> asset-import-repair-plan.v1
  -> 用户确认
  -> 后续 Patch Plan 或人工处理
```

本轮只实现数据层，不接 UI，不自动修复。

## 框架定位

该需求属于资源系统和 AI 修复链路的交叉点：

```text
Asset Importer
Asset Diagnostic
AI Repair Candidate
User Approval
Patch / Repair Flow
```

相关主文档：

```text
25-Asset-DB-Importer-MVP.md
13-AI资源生产管线.md
施工文档/历史/34-当前剩余可直接实现清单与施工顺序.md
```

## 执行方式

新增数据层能力：

```text
failed AssetImportTask
  -> createAssetImportDiagnostic
  -> createAssetImportRepairPlan
```

生成的 repair candidate 必须满足：

```text
reportOnly: true
requiresUserApproval: true
canAutoApply: false
```

也就是说，这一层只把问题整理成 AI 和用户都能看懂的候选，不自动改项目。

## 修改文件

```text
package.json
src/asset-generation/assetImportRepairPlan.ts
scripts/test-asset-import-repair-plan.cjs
框架设计/引擎总体架构/25-Asset-DB-Importer-MVP.md
框架设计/引擎总体架构/施工文档/历史/34-当前剩余可直接实现清单与施工顺序.md
```

## 验证方式

新增测试命令：

```powershell
npm.cmd run test:assetimportrepair
```

本轮执行：

```powershell
npm.cmd run test:assetlock
npm.cmd run test:assetimportrepair
npm.cmd run build
```

验证点：

```text
failed import 可以生成 diagnostic。
diagnostic 可以生成 repair candidate。
candidate 是 report-only。
candidate 需要用户确认。
candidate 不修改项目文件。
```

## 保留边界

本需求没有做：

```text
不自动重新导入资源。
不自动删除 meta。
不自动移动文件。
不自动替换资源引用。
不自动写入 Asset DB。
不接 UI。
```

## 当前结论

该需求完成了“导入失败可解释化”的第一步。

它让资源导入错误进入统一 AI 修复流程，但仍保持安全边界：

```text
AI 可以提出候选。
引擎可以展示证据。
用户必须确认。
系统不能自动破坏资源文件。
```

