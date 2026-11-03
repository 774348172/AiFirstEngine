# 333 Independent Tools + Shared Prepared Artifact Cache v1 施工文档

状态：已废弃（2026-09-12），停止施工，由 334 承接。

## 过量施工约束

只改 Compiler/Provider 内部准备复用与测试，不新增工具、Provider、Runtime 模块、Agent 状态或跨工具调用。保持每个工具独立完成调用。

## Gate A：调用链与缓存基线

盘点 `run`、`playtest`、`build` 的准备入口、缓存 owner 和身份字段，确认可复用边界。

## Gate B：独立复用实现

让各工具通过同一 Compiler cache lookup 独立命中或自行准备；不得等待其它 ToolResult。补命中、未命中和身份失效测试。

## Gate C：成本对照

比较同一 revision 下首次调用与后续独立调用的准备阶段次数和耗时；只记录差异，不改变工具 schema。

## Gate D：独立性回归

验证单独调用 `run`、`playtest`、`build` 均可完成；任意一个工具失败不污染其它工具；缓存损坏或身份漂移 fail-closed。

## 归档

Gate A-D 通过后生成完成记录并同步入口；若需要改变工具拓扑或公开 schema，暂停并修订正式方案。
