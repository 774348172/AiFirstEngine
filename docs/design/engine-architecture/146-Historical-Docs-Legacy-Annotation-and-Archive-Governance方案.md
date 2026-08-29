# 146 Historical Docs Legacy Annotation and Archive Governance 方案

## 1. 问题定义

TypeScript / Electron 原型层已经归档到 `legacy/typescript-prototype/`，但历史文档中仍大量出现：

```text
TypeScriptRuntimeBackend
React / TypeScript / Vite
Electron
npm.cmd
src/
scripts/
schemas/
```

这些内容作为历史记录是有价值的，但如果不加治理，AI 或人类工程师搜索时容易误判为当前正式入口。

## 2. 治理目标

```text
保留历史事实。
降低搜索噪声。
让当前入口和历史入口分区清晰。
不改 Rust 正式主线。
不把旧 TS/Electron 方案重新讨论成当前方案。
```

## 3. 其他引擎对比

Unity / Unreal / Godot / Bevy 都会保留历史文档、迁移记录或旧 API 说明，但通常会通过 `Deprecated`、`Legacy`、`Archive`、`Migration`、`Changelog` 等归口降低误用风险。

对我们的启发：

```text
历史记录可以保留，但必须带明确状态。
当前入口文档必须短、准、强约束。
历史目录不能和当前方案同层竞争。
```

## 4. 正式方案

采用“轻量标注 + 关键移动”的治理方式：

```text
1. 将容易误导的旧执行流水文档移动到 历史文档/。
2. 给历史文档 README 加明确规则。
3. 给其它 AI 审查目录加目录级 legacy notice。
4. 对仍有当前价值但含旧 TS/Electron 片段的文档加顶部状态说明。
5. 更新文档地图，说明历史文档只能作为参考，不能作为当前入口。
```

本轮不做：

```text
不逐行重写全部历史记录。
不删除历史阶段完成记录。
不删除其它 AI 审查目录。
不改 Rust 代码。
```

## 5. 自审

```text
Specification fit: 满足用户要求，治理历史文档标注和归档。
Rule fit: 不重开已退役的 TypeScript Runtime，不影响 Rust Native Runtime 唯一正式主线。
Textual consistency: 区分 current / legacy / external review / historical execution log。
Design fit: 降低搜索误导，保留可追溯性。
Implementation feasibility: 只移动和标注文档，可立即执行。
Practical reasonableness: 不做大规模历史重写，避免引入新的文档灾难。
```

结论：

```text
方案通过自审，可以生成施工文档并施工。
```

