# 309-E EditorProjectAdapter：完整文档 Last-Writer-Wins 与结构化 CAS v1 方案

> 文档性质：309 子方案；不授权源码施工。  
> 上级方案：`309-Headless-AuthoringProjectContext-Unified-Mutation-v1方案.md`。  
> 前置阶段：309-A、309-B、309-C、309-D 已完成并归档。  
> 当前状态：Stage E 已于 2026-09-01 完成 Scene 最小纵切并归档；只声明 `sourceAvailable`。  
> 设计日期：2026-08-31。

## 0. 一句话决定

`EditorProjectAdapter` 只把可选 Editor 接入同一个 `AuthoringProjectContext`。对于一个完整的、可验证的
单文件文档，显式保存采用 **last-writer-wins**：最后完成有效原子写入的内容成为当前 canonical 内容；不做
冲突弹窗、自动 merge、rebase 或旧草稿恢复。对于依赖旧状态的结构化增量修改，继续使用 309-C 的
expected-revision CAS、journal、receipt 和 rollback。

## 1. 要解决的问题

当前 Editor 有 `ProjectSession`、`EditorSceneDocument` 和 `ProjectWriteScope`，Headless AI 则可通过普通
文件工具或 Engine mutation 修改同一项目。两者都可能保留内存状态，导致 Editor 保存时不知道 AI 是否已经
写过同一个文件。

本方案不把这个问题解释成“必须保护每一次草稿不被覆盖”。用户确认的产品语义是普通文件开发语义：

```text
Editor/AI 产生完整文档内容
-> 执行一次有效 Save
-> 该写入按实际完成顺序覆盖此前内容
-> 后一次有效 Save 成为最新内容
```

因此 309-E 的职责是统一保存入口、保证单文件写入完整、刷新 Context 观察事实，而不是替用户仲裁哪个草稿
更正确。

## 2. 非目标与边界

本文不负责：

- 把 Editor 变成 AI 默认入口；
- 建立新的 Editor/Gateway/Engine Agent；
- 自动合并两个完整文档；
- 恢复或阻止用户明确要求的后写入；
- 把普通文件写入强制转换为 `project.mutate(goal)`；
- 迁移所有 AUI、Prefab、Rule、Input、Build 和 Asset writer；
- 修改 Rust Runtime、RuntimePackage、Compiler 或 Host Tool Registry。

Editor 仍是可选验证 Projection。`AuthoringProjectContext` 仍是项目 identity、revision、source inventory
和结构化 mutation 的唯一 owner；Editor memory、dirty flag 和 UI revision 都不是项目 authority。

## 3. 术语与写入分类

### 3.1 完整文档保存

完整文档保存是对一个 canonical 文档路径生成完整、可解析、可验证的最终字节，并以单文件原子替换写入。
典型对象包括：

- 一个 Scene document；
- 一个 AUI document；
- 一个独立配置、规则或输入映射 document；
- Save As 产生的单一新 document。

文档内容必须由领域 Module 完成 schema validation 和稳定序列化。Context 不理解 Scene 或 AUI 语义，只负责
路径能力、原子替换和刷新。

### 3.2 结构化增量修改

结构化增量修改依赖“我读到的旧状态”，或者同时操作多个路径，例如：

- 删除、移动、重命名稳定身份对象；
- 批量引用替换；
- Asset import、meta/GUID 创建和资源替换；
- schema migration；
- 需要跨文件原子性、影响分析、receipt 或 rollback 的操作。

这些操作继续生成 `ProjectMutation`，绑定 expected revision 和 expected before state，并进入 309-C 统一
mutation lane。它们不是完整文档 Save，不能改成无条件覆盖。

## 4. 正式拓扑

```text
Optional Editor
  -> EditorProjectAdapter
      -> AuthoringProjectContext Interface
          -> EmbeddedAuthoringProjectContext
              -> canonical project storage

Headless AI / Engine Tool
  -> HeadlessProjectProvider
      -> same AuthoringProjectContext Interface
```

Adapter 的 Interface 应保持小而深，隐藏 path containment、serialization handoff、clean-save 判断、原子替换、
Context refresh、结构化 mutation lowering、receipt/report 投影和生命周期细节。调用方不应自己编排 lock、
journal、recovery 或 revision 计算。

## 5. EditorProjectAdapter 职责

### 5.1 打开

1. 通过 Context open/refresh 取得当前 canonical revision。
2. 通过 Context snapshot 读取目标文档字节。
3. 由领域 Module 解析为 `EditorSceneDocument` 或相应 Editor draft。
4. 记录 `scene_path` 和本地 `draftRevision`；该 revision 只用于 UI dirty/undo，不是 project revision。

打开流程不得直接以 Editor 内存对象作为 Headless、Compiler、Runtime 或 Build 输入。

### 5.2 编辑

编辑操作继续在本地 draft 上完成，并由现有 Undo/dirty 机制提供体验。只改变 selection、viewport、preview
world 或 UI 状态的操作不产生 canonical 写入。

`dirty` 只回答“本地 draft 是否发生了编辑”，不回答“项目是否被其他进程修改”。watcher 只能更新需要刷新
的提示，不能推进 canonical revision。

### 5.3 完整文档 Save

```text
local draft
  -> domain validation
  -> stable serialization
  -> EditorProjectAdapter.save_document
  -> Context single-document atomic replace
  -> Context refresh / report
```

保存规则：

1. 草稿验证失败：不写入，返回结构化诊断，保持 dirty。
2. 当前文档目标存在且本地 draft 未变化：`clean_save_no_write`，不覆盖外部新内容。
3. 序列化字节与目标当前字节相同：不写入，不改变 mtime。
4. 序列化字节不同：执行单文件原子替换；不比较 draft base revision，不拒绝外部 drift。
5. 目标文件缺失：显式 Save 重新创建该完整文档；这是一次有效写入。
6. 写入成功后清除本地 dirty，并 refresh Context 发布实际 canonical revision。
7. 写入失败保持 dirty；不得把失败报告成成功或清除草稿。

“最后写入者”指最后完成有效原子替换的写入者，而不是最先开始编辑者。两个 Engine 参与者同时保存时，
以 canonical storage 上实际完成的写入顺序为准；不能以调用开始时间推断胜负。

### 5.4 Save As

Save As 对目标路径执行同样的完整文档替换规则：

- 新目标不存在时创建；
- 目标已存在时由本次显式 Save As 覆盖；
- 目标必须在 project root 内且满足 canonical source policy；
- 不把 Save As 伪装成引用迁移或跨文件 mutation；
- 成功后 draft 的当前路径切换到目标路径。

如果 Save As 同时要求重写引用、创建多个 meta 或移动稳定身份对象，则拆成“完整文档保存 + 结构化 mutation”，
后者按 CAS 规则执行。

## 6. Context 写入语义

为避免 Adapter 重新实现 309-C，Context 对外保留一个小的领域无关写入 Interface，概念上可表示为：

```text
save_document(handle, DocumentWriteRequest) -> DocumentWriteReport
commit(handle, ProjectMutation) -> MutationReceipt
```

`DocumentWriteRequest` 至少包含 canonical relative path、validated bytes、domain/schema identity 和
write intent。它不包含用户自然语言目标，也不要求调用方提供 project revision 作为准入条件。

`save_document` 的 Implementation 必须：

1. 验证 handle、root binding、relative path、source policy 和文件大小限制；
2. 确认 bytes 已由领域 owner 验证；
3. 获取 project-scoped 短写入协调；
4. 读取目标当前字节并执行 equal-byte no-write 判断；
5. 用现有 `ProjectWriteScope` 的原子替换语义写入单文件；
6. 释放写入协调；
7. refresh canonical inventory，返回 before/after digest、changed path、revision 和状态。

该操作不执行 expected-revision CAS、不产生结构化 mutation rollback 义务，也不触发 merge/rebase/reload。
单文件原子替换已经保证崩溃结果是旧文件或新文件，不会产生半个 JSON 文档。多文件完整替换不属于本方案
的“完整文档”类别，必须使用 ProjectMutation。

## 7. 与 309-C/D 的关系

### 7.1 结构化 mutation 保持原语义

`ProjectMutation` 继续执行：

```text
expected revision
-> expected before state
-> OS mutation lane
-> journal
-> multi-file apply
-> post verification
-> receipt / rollback
```

两个 AI session 从同一 revision 准备相同类型的增量操作时，只有一个能 CAS 成功；另一个必须重新读取并由
Host AI 决定是否重新生成。这里的 CAS 是防止错误增量叠加，不是 Editor Save 的冲突仲裁器。

### 7.2 Crash recovery 的适用范围

309-D 的未完成 journal、before/after 判定、receipt 补发和 recovery_required 继续只约束结构化 mutation。
单文件完整文档 Save 使用原子替换，不创建多文件 journal；进程崩溃后下次 refresh 读取 storage 中完整的旧或新
文件即可。

### 7.3 外部普通文件工具

`write_file`/`apply_patch` 仍然是正式的一等入口。它们按文件系统的最后有效写入结果生效；下一次 Engine
语义操作自动 refresh。Context 不声称能够锁住不遵守 Engine lock 的外部进程，也不把正常的后写入提升为冲突。

如果外部写入导致文档无法解析，refresh 发布 invalid revision，后续 check/run/build 按既有 qualification 规则
拒绝；这不是恢复旧版本，而是如实报告当前最后写入结果。

## 8. 典型时序

### 8.1 Editor 与 AI 先后保存

```text
Editor 打开 A
AI 保存 B                 -> canonical = B
Editor 修改并保存 A1       -> canonical = A1
AI 再保存 B2              -> canonical = B2
```

每次完整保存都按实际写入完成顺序生效；没有“旧草稿保护”或“冲突解决”步骤。

### 8.2 结构化修改与完整保存交错

```text
AI 准备 rename mutation，base = R1
Editor 完整保存 Scene      -> canonical revision = R2
AI commit rename           -> CAS 发现 R1 != R2，返回 drift
```

AI 的 rename 不能盲目套在新文件上，因为它依赖旧状态。Editor 的完整保存仍然成功，不会被 AI 的待提交 mutation
反向阻塞。

### 8.3 clean Save

```text
Editor draft clean，AI 已改写磁盘
Editor 点击 Save           -> clean_save_no_write
```

没有本地 draft 变化就没有“最后一次有效保存”。如果未来需要无视 dirty 强制写入，应新增明确的 Force Save
产品语义，不在 309-E 隐式加入。

## 9. 诊断与报告

完整文档 Save 至少返回：

```text
status: saved | unchanged | failed
path
domain
beforeDigest (可选)
afterDigest (成功时)
observedRevision (成功 refresh 后)
dirtyAfter
diagnostics
```

必须区分：

- `clean_save_no_write`：本地无变化，不是冲突；
- `document_unchanged`：序列化字节相同，不是冲突；
- `document_saved`：本次有效写入成为当前内容；
- `document_invalid`：当前最后写入内容不符合 schema；
- `document_write_failed`：本次写入未完成，草稿仍 dirty。

不新增 `source_level_conflict`、`merge_required`、`rebase_required` 或 `stale_draft_rejected` 作为完整文档
保存的默认结果。

## 10. 不变量

1. canonical project storage 是唯一项目真相；Editor draft 只是 local projection。
2. 完整单文件 Save 不执行 expected-revision CAS；最后完成有效写入者覆盖此前内容。
3. clean Save 不产生写入；无本地变化不构成一次有效保存。
4. 结构化增量修改继续使用 expected-revision CAS，不得悄悄降级为覆盖。
5. 完整文档保存不得跨多个文件伪装成单文件操作。
6. 单文件保存使用原子替换，崩溃不会留下半个文档。
7. Save 成功后必须 refresh；EditorSession revision、Gateway generation 和 watcher event 不得成为 authority。
8. 不自动 merge、rebase、reload、恢复旧草稿或猜测用户意图。
9. Editor、Headless Provider 和普通文件工具最终都从 canonical bytes 得到当前事实。
10. Context 不成为 AI 可见工具、二级 Host 或 Agent。

## 11. 资格 Gate（未来施工文档必须覆盖）

### E1：Editor 接入同一 Context

Editor open/refresh/snapshot 与 Headless 读取同一 revision；移除 Scene 打开流程对独立 `fs::read` 的依赖。

### E2：完整文档 last-writer-wins

两个真实 Adapter 或测试进程依次保存不同完整文档，最终字节严格等于最后完成的有效写入；无 drift rejection、
merge 或 rebase 状态。

### E3：clean-save contract

clean Save 不改变 bytes、mtime 或 revision；目标缺失时显式 Save 可重建文档；dirty Save 失败保持 dirty。

### E4：结构化 CAS 不回退

两个结构化 mutation 从同一 base 并发提交时仅一个成功；完整文档 Save 不改变 CAS 的 drift 语义。

### E5：原子性与刷新

强制终止发生在单文件替换前后时，重新 open/refresh 只能看到完整旧文档或完整新文档；invalid 最后写入必须明确
发布 invalid qualification，而不是静默恢复旧文件。

### E6：边界与回归

Save As 的 path containment、canonical policy、schema validation、Scene/AUI 现有 clean-save 测试和 Context
309-A 至 309-D 回归均保持通过。不得借 309-E 迁移 Provider、Compiler、Gateway 或所有旧 writer。

## 12. 施工拆分建议

本方案不授权施工。后续施工文档应至少拆成：

1. `EditorProjectAdapter` 的 Context handle、open、snapshot 和 local draft binding；
2. Scene 单文件 Save/Save As 的 LWW lowering 与 clean-save 迁移；
3. 结构化 mutation consumer 的边界核对与 E4/E5 回归。

每一份施工文档必须列出实际文件、测试命令、回滚范围和“不得扩大到哪些 writer”。在 Stage E 完成前，不能
宣称整个 Editor 已经迁移，也不能宣称 309 或 AI-first Tool Provider 已完成。

## 13. 方案选择理由

与“所有 Editor Save 都拒绝 drift”的方案相比，本方案：

- 更接近 Codex 使用 `write_file` 的直觉；
- 不引入用户不要求的冲突、合并和恢复工作流；
- 保留完整文档的高表达性，不把一个文档拆成大量 CRUD mutation；
- 让 CAS 只承担它真正擅长的增量一致性，而不是阻止正常覆盖；
- 仍保留统一 Context 的 path safety、atomic write、refresh 和结构化报告；
- 不削弱 309-C/D 对多文件 mutation 的正确性和崩溃恢复保证。

代价是：完整文档的中间修改可能被后一次完整保存覆盖，调用方必须接受 last-writer-wins 作为产品语义；
Engine 不承诺替用户找回被覆盖的草稿。若产品未来需要历史找回，应另立版本历史或显式 checkpoint 方案，
不能偷偷把 309-E 变成冲突仲裁系统。
