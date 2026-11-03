# 146 当前可自动化施工文档：Historical Docs Legacy Annotation and Archive Governance

## 1. 施工目标

治理 TypeScript / Electron 原型层退役后的历史文档噪声，让当前入口与历史记录清晰分离。

## 2. Gate A：移动旧执行流水文档

操作：

```text
移动 框架设计/当前项目改造计划.md
到 框架设计/引擎总体架构/历史文档/当前项目改造计划-TS原型期历史记录.md
并添加顶部 Legacy Notice。
```

## 3. Gate B：目录级标注

操作：

```text
更新 历史文档/README.md。
新增 其它AI审查目录/README.md。
给 其它AI审查目录 下现有审查报告添加顶部 Legacy Notice。
```

## 4. Gate C：当前价值文档标注

操作：

```text
给 框架设计/渲染技术路线/README.md 添加顶部状态说明：
顶部长期渲染路线仍可参考；
文中 React / Electron / npm / TS 原型相关内容视为历史记录。
```

## 5. Gate D：入口文档同步

操作：

```text
更新 00-文档地图.md 的历史文档使用规则。
更新 49-当前状态与下一步入口.md，记录历史文档治理已完成。
更新 54 与施工 README。
```

## 6. Gate E：验证

测试：

```powershell
rg -n "当前项目改造计划" 框架设计 -g "*.md"
rg -n "React \\+ TypeScript|Electron|npm\\.cmd|TypeScriptRuntimeBackend" 框架设计 其它AI审查目录 -g "*.md"
cargo fmt --check
cargo test -p editor_core
```

验收：

```text
旧 TS/Electron 命中必须位于 legacy notice、历史文档、其它 AI 审查或阶段记录上下文中。
Rust 正式主线测试通过。
```

## 7. 施工文档自审

```text
Specification fit: 覆盖历史文档标注和归档治理。
Rule fit: 不重新启用 TypeScript Runtime，不修改正式 Rust 逻辑。
Textual consistency: Gate 顺序从移动、标注、入口同步到验证。
Design fit: 降低 AI 搜索误导，保留历史证据。
Implementation feasibility: 仅文档移动和标注，可执行。
Practical reasonableness: 不逐行清洗所有历史记录，避免过度治理。
```

结论：

```text
施工文档通过自审，可以施工。
```

