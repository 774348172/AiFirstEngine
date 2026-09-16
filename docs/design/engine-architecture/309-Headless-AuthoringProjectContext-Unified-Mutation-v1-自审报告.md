# 309 Headless AuthoringProjectContext + Unified Mutation v1 自审报告

> 审查对象：`309-Headless-AuthoringProjectContext-Unified-Mutation-v1方案.md`。  
> 审查日期：2026-08-31。  
> 审查结论：通过，可以生成分阶段施工文档；不允许用一份施工文档一次完成全部 Stage A-F。  
> 施工授权：无。

## 1. 结论

309 与权威产品需求及顶层架构一致，已经包含用户确认的关键边界：

```text
AI -> concrete engine_* tools -> Engine Tool Provider -> Engine Tool Kernel/domain owner
   -> internal AuthoringProjectContext
```

`AuthoringProjectContext` 只是协议无关的内部深 Module，不是 AI 可见工具、二级 Host、通用
`catalog/execute` 入口、后台 Agent 或必须连接的 daemon。方案 B 的嵌入式温状态实现没有改变这一点。

方案可以进入施工准备，但 309 是完整 R1 架构设计，范围包含读 authority、snapshot、mutation、recovery、
Editor Adapter 和旧 authority 退役。若一次施工会同时跨越过多 owner，测试和回滚范围不可控。因此正式施工
必须按能独立验收的纵切拆分，第一份只建设 Headless 只读 authority。

## 2. 权威需求一致性

| 检查项 | 结论 | 依据 |
| --- | --- | --- |
| AI 原生工具体验 | 通过 | Context 不暴露给 AI，未来由具体 `engine_*` 工具内部消费 |
| 默认无 Editor | 通过 | Headless Provider/CLI 可进程内嵌入 Context |
| 引擎适配 AI | 通过 | 普通文件写入后由下一次 Engine operation 内部 refresh |
| 单一项目真相 | 通过 | canonical bytes + revision policy 是 authority，watcher/cache/generation 仅作提示 |
| 高风险修改治理 | 通过 | expected revision、统一 lane、journal、receipt、rollback 已定义 |
| 不建立第二个 Agent | 通过 | Context 不拥有目标、计划、模型循环或工具选择 |
| Editor 保留但降级 | 通过 | Editor 只通过 Adapter 保存 draft 和提交 mutation |

## 3. 内部一致性检查

### 3.1 Interface 与 owner

`open/refresh/snapshot/commit/rollback/close` 保持小而深。Context 只拥有项目 identity、revision、source
inventory、snapshot 和事务协调；Compiler、Runtime、Host Adapter、Editor UI 与领域 schema 均在外部。

### 3.2 普通文件写入与结构化 mutation

方案没有把全部 AI 修改强制塞进一个自然语言 mutation 黑盒。普通代码和允许直接编辑的 Project Assets
继续使用文件工具；Scene/Prefab/AUI/Asset 等需要跨文件原子语义的修改才进入 typed mutation。

### 3.3 温状态与多进程

进程内 cache 只改善性能。跨进程正确性依赖 canonical bytes、统一 revision 算法、OS lock、CAS 和 journal，
所以没有因选择方案 B 形成多套 authority。

### 3.4 Snapshot 与并发

方案明确 snapshot 必须是不可变逻辑视图，长时间 Compiler 操作不能继续读取 live root，也不能长期占用
mutation lane。该约束足以阻止后续施工用“保存路径 + revision id”伪装 snapshot。

### 3.5 崩溃与未知写入

before、sealed after、可证明 partial apply 和 unknown bytes 已分开处理；未知外部字节不得被覆盖或吸收到
receipt。rollback 绑定 exact applied revision，满足 fail-closed 要求。

## 4. 发现但不要求修改方案的问题

以下内容属于施工分期，不是方案缺陷：

- 当前 `CandidateProjectRevisionStore`、`ProjectOpenPreparation` 和 manifest 类型仍在 `editor_core`；
- 当前工作树对相关 Rust 文件已有用户修改，激活前必须重新扫描，不能覆盖；
- immutable snapshot 的最终 content-addressed reuse 尚未实现；
- 跨进程 mutation、crash recovery、Editor Adapter 和 Gateway retirement 尚未实现；
- R5 的真实 Host Tool Registry 资格不属于 309-R1。

这些缺口已经被 309 的 Stage 与 G1-G9 覆盖，不需要为生成第一份施工文档扩大或改写正式方案。

## 5. 过量施工审查

第一份施工文档只允许覆盖：

```text
neutral crate boundary
canonical source inventory/revision
headless open/refresh
bounded immutable read snapshot
existing digest compatibility delegation
```

第一份施工明确禁止：

```text
commit / rollback / OS mutation lock / journal / crash recovery
Asset DB rebuild or raw asset registration
Compiler prepare / RuntimePackage / run / playtest / build
EditorProjectAdapter / EditorSession migration
Gateway generation retirement
Engine Tool Provider / MCP / Codex / OpenCode registration
production binary, real config, Local CI or release qualification
```

只有后续独立方案施工文档才可进入这些范围。完成第一份施工只能声明“309 Headless read authority
sourceAvailable”，不能声明 R1、309 或 AI 一等工具体验已经完成。

## 6. 最终判断

```text
方案一致性：通过
边界完整性：通过
AI 可见面约束：通过
并发/恢复方向：通过
施工可拆分性：通过
需要修改正式方案：否
允许生成施工文档：是，仅允许最小分期
允许代码施工：否，必须另行激活
```
