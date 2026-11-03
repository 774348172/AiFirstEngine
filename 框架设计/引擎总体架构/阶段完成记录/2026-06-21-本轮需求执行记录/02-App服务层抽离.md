# App 服务层抽离

## 需求目标

继续把 `App.tsx` 从“大入口文件”收敛成编辑器装配层，把 Patch / Validation / Build 相关能力抽到服务层。

目标不是改功能，而是降低长期维护成本：

```text
App.tsx 负责 UI 状态和面板装配。
Service 负责调用现有引擎能力、格式化结果、隔离桥接细节。
底层 engine / build / validation 规则不被 UI 直接缠住。
```

## 框架定位

该需求属于编辑器工程结构治理：

```text
Editor UI
Editor Service Layer
Runtime / Engine Capability
Build / Export Capability
Validation Capability
```

相关主文档：

```text
20-编辑器工程结构.md
施工文档/历史/34-当前剩余可直接实现清单与施工顺序.md
```

## 执行内容

本轮完成了三个服务门面：

```text
src/services/validationService.ts
src/services/patchService.ts
src/services/buildService.ts
```

### ValidationService

负责：

```text
封装 runProjectValidation。
格式化 validation errors / issues。
让 App.tsx 不再直接 import engine/projectValidation。
```

### PatchService

负责：

```text
封装 projectPatch / patchRollback 的服务入口。
保持现有 Patch Plan 语义不变。
为后续 AI Patch 审批和治理层接入预留清晰入口。
```

### BuildService

负责：

```text
封装 Windows export bridge。
定义 BuildReportSummary / BuildExportResult。
提供 createBuildReportSummary 纯函数。
为 Build Report Panel v2 最近导出摘要提供数据。
```

## 修改文件

```text
src/App.tsx
src/services/validationService.ts
src/services/patchService.ts
src/services/buildService.ts
框架设计/引擎总体架构/20-编辑器工程结构.md
框架设计/引擎总体架构/施工文档/历史/34-当前剩余可直接实现清单与施工顺序.md
```

## 验证方式

ValidationService 验证：

```powershell
npm.cmd run test:schema
npm.cmd run test:runtime
node scripts/validate-project.cjs .tmp/validation-service-starter-project.json
npm.cmd run build
npx.cmd playwright test ui-smoke
```

PatchService 验证：

```powershell
npm.cmd run test:patch
npm.cmd run test:rollback
npm.cmd run build
npx.cmd playwright test ui-smoke
```

BuildService 验证：

```powershell
npm.cmd run test:buildreportindex
npm.cmd run test:buildgraphcore
npm.cmd run build
npx.cmd playwright test ui-smoke
```

后续 BuildService 增强后追加：

```powershell
npm.cmd run test:buildservice
```

## 保留边界

本需求没有做：

```text
没有引入复杂全局 store。
没有改变 Patch Plan 语义。
没有改变 Validation Report 语义。
没有改变 Build Graph 输出结构。
没有改变 UI 行为。
没有接入真实 AI 自动修复。
```

## 当前结论

服务层抽离已经完成第一轮。

现在 `App.tsx` 的长期方向更清晰：

```text
UI 触发意图
  -> Service 编排能力
  -> Engine / Build / Validation 模块执行
  -> UI 展示结果
```

这符合后续 AI 生成、验证、修复、构建报告审查继续扩展的方向。

