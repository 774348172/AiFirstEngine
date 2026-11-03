# 334 Internal Prepared Artifact Cache — No New Tool v1 施工文档

状态：源码内部复用工作已归档；2026-09-13 更正遗留的“待执行”状态并补录证据边界。对应[正式方案](../../334-Internal-Prepared-Artifact-Cache-No-New-Tool-v1方案.md)，见[记录更正](../../阶段完成记录/2026-09-13-334-Internal-Prepared-Artifact-Cache-Record-Correction-v1/00-总览.md)。本次不重开缓存施工或计时复测。

归档不等于取得新的计时资格：Gate A 为 `baseline_captured`，B/D 为 `passed`，C 原始记录为 `evidence_reused`。后续真实连续调用复测为 `ineligible_for_fresh_host_retest`；54.3 秒仅是此前宿主测量参考，不能称为 334 新增优化量。下列 Gate 保留原施工意图，实际结果以各 Gate 记录和上述更正为准。

## 施工边界

只调整 Engine/Compiler 内部 prepared artifact 复用和证据，不增加 AI 工具、Provider、Skill 步骤、公开 schema 或工具间调用依赖。

## Gate A：Cache Identity Baseline

盘点现有 RuntimePackage 与 Player artifact cache 的身份字段、缓存 root、命中校验和失效分支，确认可复用边界。

证据：`.artifacts/334-gate-a-identity.json`。

## Gate B：Internal Reuse

让 `run`、`playtest`、`build` 在各自独立调用时查询同一内部缓存；命中时跳过可复用准备，未命中时自行完成。Prepared Artifact 不暴露给 AI。

验证：命中、未命中、身份漂移和损坏产物测试；九项工具定义数量不变。

证据：`.artifacts/334-gate-b-reuse.json`。

## Gate C：Time Comparison

原计划：在同一 projectRevision 下比较首次调用与后续独立调用的 tests compile、Host build 和总耗时，确认缓存命中带来实际节省。

实际：原始 Gate C 采用 `evidence_reused`，引用既有 332 rebuilt 计时和已验证的缓存命中分支；没有执行本 Gate 的新同条件计时对照。2026-09-12 定向连续复测未取得资格，详见 `.artifacts/334-targeted-continuous-retest-20260912.json`。不得把复用记录改写为新计时 `passed`。

证据：`.artifacts/334-gate-c-timing.json`。

## Gate D：Tool Independence

验证单独调用 `run`、`playtest`、`build` 不需要其它工具结果；缓存失效不污染其它工具；工具面和 Skill 工作流不增加。

证据：`.artifacts/334-gate-d-independence.json`。

## 停止条件

若需要新增 AI 工具、公开 Prepared Artifact 参数、工具间强制等待、固定调用顺序或修改 Runtime/Project 语义，停止并先修订正式方案。

## 归档条件

原计划以 Gate A-D 和受影响 owner 回归通过作为归档条件。现存证据支持内部复用与工具独立性归档，计时声明收窄为历史参考；Gate C 不补写为新测试通过。2026-09-13 仅补齐记录与状态，不倒填历史测试或重新执行任何 Gate。
