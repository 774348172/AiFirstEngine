# Evidence Index v1 数据层

## 需求目标

把 runtime trace、build report、asset repair candidate、validation report 等 AI 查错证据统一成一个只读索引。

目标不是合并报告，而是提供一个入口：

```text
当前项目有哪些证据？
证据来自哪里？
证据类型是什么？
是否 report-only？
是否需要用户确认？
AI 或用户应该从哪里继续查？
```

## 框架定位

该需求属于验证、追踪和 AI 查错基础设施：

```text
Runtime Trace
Build Report
Asset Repair Candidate
Validation Report
Evidence Index
AI Debug / Review Entry
```

相关主文档：

```text
11-测试与验证系统.md
26-Runtime-Replay-MVP.md
30-Build-Graph-Core-v2.md
施工文档/历史/34-当前剩余可直接实现清单与施工顺序.md
```

## 执行方式

新增只读索引模块：

```text
src/evidence/evidenceIndex.ts
```

它可以索引：

```text
RuntimeTraceReport
BuildReportIndex-like
AssetRegistrationPlan
AssetImportRepairPlan
ValidationReport-like
```

索引输出只保存摘要和引用，不改原报告。

## 修改文件

```text
package.json
src/evidence/evidenceIndex.ts
scripts/test-evidence-index.cjs
框架设计/引擎总体架构/施工文档/历史/34-当前剩余可直接实现清单与施工顺序.md
```

## 验证方式

新增测试命令：

```powershell
npm.cmd run test:evidenceindex
```

本轮执行：

```powershell
npm.cmd run test:evidenceindex
npm.cmd run test:runtime
npm.cmd run test:buildreportindex
npm.cmd run build
```

最终回归也再次执行：

```powershell
npm.cmd run test:evidenceindex
```

验证点：

```text
Evidence Index 能索引 runtime / build / asset / validation 证据。
索引保持只读。
索引不会扫描文件系统。
索引不会合并、重写或删除原报告。
索引不会触发自动修复。
```

## 保留边界

本需求没有做：

```text
不接 UI。
不扫描本地目录。
不读取历史构建 JSON。
不替代原始报告。
不自动修复。
不生成 Patch Plan。
```

## 当前结论

Evidence Index v1 已经建立了 AI 查错的统一数据入口。

这对后续非常重要：

```text
AI 不需要盲目猜问题。
AI 可以先找证据。
用户也能知道某个问题来自 runtime、build、asset 还是 validation。
```

但当前它仍是数据层，不是完整 Debug UI。

