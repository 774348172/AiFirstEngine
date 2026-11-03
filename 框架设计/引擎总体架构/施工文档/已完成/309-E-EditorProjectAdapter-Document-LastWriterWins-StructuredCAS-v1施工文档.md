# 309-E EditorProjectAdapter：完整文档 Last-Writer-Wins 与结构化 CAS v1 施工文档

> 文档性质：施工文档。  
> 当前状态：E0-E5 已完成；2026-09-01 归档。  
> 上级方案：`../../309-E-EditorProjectAdapter-Document-LastWriterWins-StructuredCAS-v1方案.md`。  
> 上级总方案：`../../309-Headless-AuthoringProjectContext-Unified-Mutation-v1方案.md`。  
> 前置状态：309-A、309-B、309-C、309-D 已完成并归档。  
> 生成日期：2026-08-31。  
> 用户授权范围：2026-08-31 用户明确要求直接按本文开始施工。  
> 激活基线：branch `codex/255-capability-catalog`，HEAD `acaffa8cb95bbb54d093bbf45835a59315c82da5`；继承现有 dirty worktree，不重置用户变更。

## 1. 施工目标与允许声明

本施工只实现 309-E 的最小可验证纵切：把现有 Editor Scene 的打开、快照读取、完整单文件 Save/Save As
接入同一个 `AuthoringProjectContext`，并保留结构化增量修改的 expected-revision CAS。

施工完成后最多可以声明：

```text
Editor Scene draft 通过 EditorProjectAdapter 从 Context snapshot 打开；
完整单文件 Save/Save As 按最后完成有效原子写入者覆盖；
clean Save 不写盘；schema/序列化/原子替换失败保留 dirty；
结构化 ProjectMutation 仍经过 309-C expected-revision CAS；
Context refresh 后 Editor 观察到实际 canonical bytes 与 revision。
```

不得声明整个 Editor 已经完成 Headless 化、AI 工具已接入、所有文件 writer 已统一、生产可用、跨平台发布
可用，或 309-E 之外的 Stage F/G 已完成。

## 2. 必要性与验证经济性审计

```text
requested claim：生成309-E施工文档，未来以最小纵切验证Editor与Context统一保存语义
confirmed gap：EditorSceneDocument/SceneSavePipeline仍直接读取或写入文件，未通过Context的document写入边界；完整文档保存仍可能误用旧revision冲突语义
minimum causal change：新增一个EditorProjectAdapter document读写边界，并只迁移Scene open、Save、Save As与直接回归测试
必须修改：editor_core的session/project_launcher/scene_editing document与scene service相关调用；必要的authoring_project_context document save接口；直接owner tests
明确排除：AUI/Prefab/Rule/Input/Asset/Build迁移，Gateway/Provider/Compiler/Runtime接入，Editor UI重做，自动merge/rebase/reload、版本历史、daemon、release与安装态
E0 -> 施工范围或当前代码基线漂移 -> 文档/源码清单检查 -> 不进入代码修改
E1 -> Editor仍绕过Context读取canonical文件或把Editor revision当authority -> adapter/open红测试 -> snapshot与revision一致性测试
E2 -> 完整Save错误拒绝外部drift或非原子覆盖 -> 双写入者红测试 -> 最后完成有效原子写入者字节相等性
E3 -> clean Save覆盖外部修改、失败清除dirty或equal-byte改变mtime -> clean/failure红测试 -> bytes、mtime、dirty与revision断言
E4 -> Scene完整Save意外降低结构化mutation的CAS保证 -> mutation边界红测试 -> stale mutation仍drift失败
E5 -> Save/Save As路径、schema、原子性或refresh回归 -> 定向editor_core与owner suite -> 最小受影响闭包
复用证据：309-A/B/C/D的Context handle、canonical revision、ProjectWriteScope原子写、journal/receipt/recovery owner与现有scene save digest tests
证据失效条件：Context document接口、SceneSavePipeline调用图、ProjectWriteScope写入语义、309-D recovery顺序、现有测试命名或依赖方向发生变化
预计成本：5个Gate；1个authoring_project_context边界与editor_core Scene纵切；仅定向测试，约2至3小时
停止条件：需要迁移第二个writer、修改公共Gateway/Provider合同、引入新crate/daemon/merge历史、触碰真实项目或预计超出3小时；先回到方案复核
```

## 3. 冻结产品语义

### 3.1 完整单文件 Save

完整 Save 先由 Scene 领域模块完成 schema validation 与稳定序列化，再调用 Context 的
`save_document(handle, request)`。该操作不要求调用方提供 project revision 作为准入条件。

1. schema 或序列化失败：不写入，返回结构化诊断，draft 保持 dirty。
2. draft clean：返回 `clean_save_no_write`，不覆盖磁盘上的外部新内容。
3. 序列化字节与目标当前字节相同：返回 `document_unchanged`，不改变 mtime 或 revision。
4. 字节不同：使用单文件原子替换；不比较 draft base revision，不拒绝外部 drift。
5. 目标不存在：显式 Save/Save As 创建完整文件。
6. 写入成功后清除 dirty，refresh Context，并发布实际 before/after digest 与 revision。
7. 写入失败保持 dirty；不得伪报成功。

“最后写入者”按实际完成有效原子替换的顺序定义，不按开始编辑时间定义。单文件 Save 不创建多文件
mutation journal，也不承担结构化 rollback 义务。

### 3.2 结构化增量修改

依赖旧状态、跨多个路径或需要 receipt/rollback 的操作继续构造 `ProjectMutation`，绑定 expected revision
并进入 309-C mutation lane。两个相同 base revision 的增量提交仍然只能有一个 CAS 成功；不得为了实现 LWW
而把 mutation 降级成无条件覆盖。

### 3.3 普通文件工具

`write_file`/`apply_patch` 仍是正式一等入口，不在本施工中迁移或加锁。下一次 Engine 语义操作通过 Context
refresh 读取当前事实；无法解析的最后写入内容应发布 invalid qualification，不自动恢复旧内容。

## 4. 施工边界与实际文件

### 4.1 允许修改的文件

```text
rust/crates/authoring_project_context/src/context.rs
rust/crates/authoring_project_context/src/lib.rs
rust/crates/authoring_project_context/src/diagnostic.rs（仅必要的document save诊断）
rust/crates/authoring_project_context/src/document.rs（若现有模块位置需要新增领域无关接口）
rust/crates/authoring_project_context/src/tests.rs（document save owner tests）
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/scene_editing/document.rs
rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/services/scene_service.rs
rust/crates/editor_core/src/tests/scene_save_digest_contract_tests.rs
```

以上文件仅在实际调用图证明必要时修改。若 Context 已有合适模块，禁止为了命名一致新增重复抽象。

### 4.2 明确禁止修改

```text
Gateway、Provider、Compiler、Runtime、RuntimePackage、Build/export、AUI/Prefab/Rule/Input/Asset writer、
项目样例、生产Editor、安装态配置、release脚本、Tower P1-2及其任何施工文档。
```

本文已经按用户授权移动到 `施工文档/当前/`。源码修改与验证只能按 E1-E5 顺序执行，不得越过上述边界。

## 5. Gate E0：激活前范围与合同复核

E0 只做只读复核；未来激活前必须重新执行，确保施工文档仍与代码基线一致：

- 309-E 正式方案顶部状态与实际施工终态一致（已完成并归档）；
- 309-A/B/C/D 的 owner、Context handle、revision、atomic writer、recovery边界仍存在；
- `EditorSceneDocument`、`SceneSavePipeline`、`ProjectSession` 的调用图未被用户变更破坏；
- `施工文档/当前/` 中没有其它施工授权冲突，Tower P1-2 仍为 `stopped_by_user`；
- 允许文件清单、测试命令和停止条件仍然有效。

复核命令（激活前才可执行）：

```powershell
Set-Location G:\gameEngin
rg -n "EditorSceneDocument|SceneSavePipeline|AuthoringProjectContext|ProjectMutation|save_document" rust\crates\editor_core rust\crates\authoring_project_context
git status --short
```

E0 不通过时停止，不进入任何代码 Gate，并先修订本施工文档或上级方案。

## 6. Gate E1：Editor 通过 Context 打开与绑定 draft

先增加能证明当前绕过路径的红测试，再实现最小 adapter：

- Scene open 从 Context handle/snapshot 取得 canonical bytes；
- Scene domain module 负责解析，Editor 只保存 local draft、path 与 dirty/undo 状态；
- Editor revision 只作为 local draft revision，不被当成 project authority；
- open/refresh 后观察到的 Context revision 与 Headless snapshot 相同；
- Editor 不把内存对象直接提供给 Runtime、Compiler、Build 或其他 authority。

建议命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core --lib editor_project_adapter_open
cargo test -p editor_core --lib scene_snapshot_binding
cargo test -p authoring_project_context --lib snapshot_revision
```

如果现有测试命名不同，应在施工记录中记录实际过滤器，不得为了通过命令新增无意义别名。

## 7. Gate E2：Scene 完整文档 LWW Save/Save As

覆盖单文件保存的核心行为：

- 两个独立 adapter/fixture 先后保存不同完整 Scene，最终 bytes 严格等于最后完成有效原子替换者；
- Save 不读取或比较 draft base revision，不返回 `stale_draft_rejected`、`merge_required` 或 `rebase_required`；
- Save As 对 project root 内新旧目标分别执行创建与覆盖；
- root 外路径、非canonical source path、超大文件和非法 domain/schema identity 在写入前失败；
- 每次成功保存后 refresh，返回 path、before/after digest、observed revision 与状态；
- 单文件写入只允许看到完整旧文件或完整新文件，不留下半个序列化文档。

建议命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core --lib scene_document_lww
cargo test -p editor_core --lib scene_save_as
cargo test -p authoring_project_context --lib document_save
```

测试只能使用测试自有临时 project fixture 与受控 writer，不启动真实 Editor/Gateway，不修改仓库真实项目。

## 8. Gate E3：clean-save、equal-byte 与失败保留

必须覆盖：

- local draft clean 且 AI/外部工具已改写磁盘：Save 返回 `clean_save_no_write`，外部 bytes、mtime、revision 不变；
- local draft dirty 但序列化后 bytes 相同：返回 `document_unchanged`，不写盘；
- dirty Save schema validation、序列化、path validation、atomic replace 任一失败：bytes 不变，dirty 仍为 true；
- 目标缺失时显式 Save（包括当前 draft clean 的显式 Save）可重建；只有目标存在且 draft clean 时才返回
  `clean_save_no_write`，不得把 clean Save 隐式改成对现有目标的 Force Save；
- 成功 Save 后 dirty 清除；失败和诊断不会清除 undo/draft 状态。

保留并扩展现有定向测试：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core --lib scene_save_digest_contract_tests
cargo test -p editor_core --lib scene_save_clean
cargo test -p editor_core --lib scene_save_failure_retains_dirty
```

## 9. Gate E4：结构化 CAS 边界不回退

使用 `authoring_project_context` owner fixture 验证：

- 两个结构化 mutation 从同一 base revision 提交时最多一个成功，另一个返回 typed drift；
- Scene 完整文档 Save 成功不会把待提交 mutation 的 expected revision 自动改写为当前 revision；
- 完整 Save 不创建结构化 receipt/rollback，也不绕过已有 recovery_required 阻断；
- 309-C/D 既有 journal、receipt、rollback 和 recovery 语义保持通过；
- 本施工不迁移其它 mutation consumer，也不添加新的二级 authority。

建议命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib mutation_commit_
cargo test -p authoring_project_context --lib mutation_rollback_
cargo test -p authoring_project_context --lib recovery_
cargo test -p editor_core --lib scene_save_does_not_bypass_mutation_cas
```

## 10. Gate E5：最小受影响闭包与终态验证

只有 E1-E4 通过后执行：

- `editor_core` Scene save/open 直接回归通过；
- `authoring_project_context` owner suite、格式和类型检查通过；
- 变更文件无越界 writer、无第二 lock/journal/revision authority；
- 文档同步只更新 309-E 状态、施工入口和完成记录，不能顺带激活 Stage F/G；
- 施工结果记录实际文件、红/绿首因、测试输出、未覆盖项和停止条件。

建议命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p authoring_project_context --lib
cargo test -p editor_core --lib scene_save
cargo fmt --check -p authoring_project_context -p editor_core
cargo check -p authoring_project_context -p editor_core
Set-Location G:\gameEngin
git diff --check
```

不运行完整 workspace、Local CI、生产 Editor、真实 Gateway、安装态配置、release binary 或设备/移动端导出。

## 11. 失败、回滚与归档规则

- 任一 Gate 失败时保留失败证据，停止后续 Gate；不通过删除测试、放宽断言或改写正式语义来转绿。
- 若源码施工尚未产生有效提交，回滚只允许撤销本施工明确修改的文件，并保留用户既有变更。
- 若已产生有效 commit/receipt 证据，按 309-C/D 既有 exact-revision rollback，不用文件覆盖伪装回滚。
- 完成后写 `阶段完成记录/2026-09-01-309-E-EditorProjectAdapter-Document-LastWriterWins-StructuredCAS-v1/00-总览.md`，再把本文件移动到 `施工文档/已完成/`。
- 归档前同步 `待执行/README.md`、文档地图、阅读顺序、`49-当前状态与下一步入口.md`、`54-当前可自动化施工总入口.md`；不得自动激活下一阶段。
- 如果只完成部分 Gate，文件留在 `施工文档/当前/` 或按项目规则记录为失败施工，不得宣称 309-E 完成。

## 12. 激活记录与自审

```text
状态：已完成；E0-E5全部通过，允许归档。
E0结果：正式方案、309-A至D前置、当前施工槽、调用图、测试owner和停止条件均匹配。
工作树：authoring_project_context及editor_core相关文件包含既有施工改动；本施工继承并保留，不reset/revert。
范围审计：通过；只覆盖Editor Scene纵切、单文件LWW、clean-save和结构化CAS边界。
深模块审计：通过；Adapter/Context隐藏path、atomic write、refresh和report细节，调用方不编排journal或lock。
语义审计：通过；完整文档Save不CAS，结构化增量继续CAS；不引入merge/rebase/reload/旧草稿恢复。
不过量审计：通过；不迁移第二writer，不碰Gateway/Provider/Compiler/Runtime/Build，不新增daemon/crate。
验证经济性：通过；定向owner与editor_core测试足以验证本纵切，完整workspace与生产验证排除。
边界审计：通过；不恢复Tower P1-2，不覆盖当前目录既有历史evidence，不改变其它施工授权。
激活授权：2026-08-31用户明确要求直接施工；本文已移动到当前目录，允许按Gate修改源码和运行定向测试。
```

## 13. 施工结果与 Gate 证据

```text
E1：通过。ProjectSession持有共享EmbeddedAuthoringProjectContext与ProjectHandle；生产项目Scene open先refresh，
    再从Context snapshot取得canonical bytes，最后交给Scene领域解析。裸Scene测试夹具保留受限fallback。
E2：通过。Scene Save/Save As进入Context document lane；完整文档不比较draft base revision，双Editor最终字节
    等于最后完成有效原子替换者；路径、source policy、大小与identity在写入前校验。
E3：通过。现有目标上的clean Save不写盘；dirty equal-byte不改变mtime；失败保持dirty；显式Save可重建
    缺失目标；Scene领域拒绝未知editor-scene-document schemaVersion。
E4：通过。结构化ProjectMutation继续使用expected-revision CAS；document save不创建mutation receipt/rollback，
    不弱化309-C/D recovery与drift合同。
E5：通过。Context owner 45/45、Scene save过滤器21/21、Scene digest合同10/10、schema定向1/1、
    Scene service 32/32、project launcher 16/16、ControlledSourcePatch 15 passed/1 ignored、
    ProjectAssetImport 13/13通过；fmt与affected cargo check通过。
```

首次新增 schema 定向测试与 Scene Save 合同测试并发链接同一 Windows test binary 时，schema 任务出现
`LNK1104` 文件占用；Scene Save 10/10 完成后串行重跑 schema 测试 1/1 通过，归类为验证并发环境冲突，
不是产品断言失败。保留的 3 个 `engine_runtime` dead-code warning 为既有 warning，本施工未新增 warning。

本施工没有启动真实 Editor/Gateway、没有运行完整 workspace/Local CI/release build、没有修改 production
二进制或真实项目；这些环境不属于 `sourceAvailable` 声明。Stage F、其它 writer 与 Tower P1-2 均未激活。
