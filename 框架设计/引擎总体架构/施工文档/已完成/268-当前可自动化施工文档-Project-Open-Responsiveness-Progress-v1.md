# 268 Project Open Responsiveness + Progress v1 施工文档

```text
状态：已完成，Gate A-F 于 2026-08-04 闭环
缺口来源：2026-08-03 Tower Defense recent-project 真实 AppHangB1
父约束：252 / 259 / 262 / 267；本次为已确认缺陷修复，不新增玩法或公共 AI Tool
日期：2026-08-03
施工方式：一次连续窗口完成 Gate A-F；激活前仍须复核当前代码与 dirty worktree
```

## 1. 施工目标

一次修复真实用户点击 ProjectRust 项目时的四个问题：

1. 项目摘要错误包含嵌套 Cargo `target` 等确定性生成目录；
2. 摘要实现把全项目文件与 canonical payload 同时聚合进内存；
3. 项目打开重活同步占用 Native Editor UI 线程，重复点击没有 exactly-once/busy 保护；
4. 首次 composition 构建和项目打开期间没有持续、可理解的进度反馈。

完成后的用户链路：

```text
点击最近项目
  -> 立即进入唯一 ProjectOpen operation 并显示中文阶段反馈
  -> 后台做 trust/cache/composition 或 project-open preparation
  -> UI 线程持续 pump/redraw，重复点击被禁用且不创建第二个 operation
  -> specialized Editor readiness 后短事务提交 prepared project
  -> 进入项目工作区

失败/取消
  -> 保留当前 Editor 和 Launcher
  -> 显示稳定中文摘要、diagnostic code 与 next action
  -> 不遗留 pending/busy 状态
```

## 2. 已确认根因与红证据

真实塔防项目：

```text
samples/tower_defense_project/Tests/FontBundleV2/target       约 6.09 GB
samples/tower_defense_project/Tests/PlayableInterface/target 约 5.50 GB
Tests 合计                                                    11,591,931,681 bytes / 9,080 files
```

调用链：

```text
SelectRecentProject
  -> ProjectService::select_recent_project
  -> reload_project_intent_workflow
  -> ProjectIntentWorkflow::open_project
  -> ProjectCandidateEntry::inspect_project_binding
  -> CandidateProjectRevisionStore::project_digest
  -> project_file_bytes + project_tree_digest
```

`candidate_project_revision.rs::is_excluded` 只排除项目根 `target` 与
`RuntimeModule/target`，没有识别其它项目拥有的 Cargo package/workspace `target`。随后
`project_file_bytes` 为每个文件保留 `Vec<u8>`，`project_tree_digest` 又构造第二份总 payload。
Windows Application log 在 `2026-08-03 20:54:04` 记录
`aife_project_editor_composition.exe / AppHangB1 / stopped responding and was closed`。

## 3. 授权与边界

用户已经确认要修复四项问题，并于 2026-08-03 明确开始施工。当前施工槽不存在其它活动施工
Markdown；254-R2 历史证据文件原样保留。本文已从 `待执行/` 移入 `当前/` 并完成激活复核。

施工允许：

- 修改通用引擎 `editor_core`、`editor_ui_model`、`editor_ui_renderer`、
  `editor_window_winit` owner/consumer 与对应测试；
- 新增通用、项目无关的 project-open preparation/activity 内部合同；
- 在 fresh run-owned state/target 下构建并启动源码 Editor，使用 Tower Defense 作真实 consumer；
- 使用小型嵌套 Cargo fixture 和稀疏/受控文件验证摘要范围与有界内存。

施工禁止：

- 删除、移动或修改塔防 `Tests/**/target`、`RuntimeModule/target` 来掩盖缺陷；
- 修改塔防玩法、资产、规则、RuntimeModule 或项目 manifest；
- 在引擎中写入 Tower Defense、FontBundleV2、PlayableInterface 的名称或路径特例；
- 改变 `ProjectRuntimeModule`、RuntimePackage、AUI、Gameplay Rule 公共合同；
- 运行 Local CI，替换 production/安装态二进制，修改真实用户配置；
- 恢复 263 Gate F/G/H 或把本修复宣称为塔防 P0-5 新资格。

## 4. 核心实现合同

### 4.1 ProjectDigestSourcePolicy

摘要仍覆盖项目拥有的全部可编辑源文件，而不是只扫描 Asset Browser roots。统一一个内部
`ProjectDigestSourcePolicy` 供 digest、candidate stage/copy/diff 使用，防止各路径排除规则漂移。

```text
始终排除的根级 engine/generated roots：
  Library / Build / target / .git / .aife / .aife-candidates

递归排除：
  位于已识别 Cargo package/workspace root 下、名称精确为 target 的目录

Cargo root 识别：
  当前目录存在 regular-file Cargo.toml；不跟随 symlink/junction/reparse point

不得排除：
  普通项目源目录、Tests 源码、Design/Features/Observations，或仅仅名字相似的文件
```

`RuntimeModule/target` 保留为 manifest 默认 RuntimeModule 的通用兼容规则；其它嵌套 crate 由 Cargo
marker 规则覆盖。策略变化必须有稳定测试证明：
修改 excluded 生成物不改变 digest；修改相邻源码必须改变 digest。既有 pending mutation/candidate
仍按 exact digest fail closed，不做旧摘要兼容 fallback。

### 4.2 Streaming canonical digest

保持当前 canonical byte protocol 的顺序与字段：

```text
schemaVersion NUL
relativePathLength + relativePath
fileLength + fileBytes
```

实现改为：先收集并排序受控的 `(relativePath, absolutePath, length)` metadata，再用固定上限 buffer
逐文件送入 `Sha256`。禁止保存 `Vec<(String, Vec<u8>)>`，禁止构造全项目 payload。读取期间若文件
长度、类型或可读性漂移则 fail closed，返回稳定 diagnostic；不接受部分摘要。

目标约束：摘要额外工作内存为 `O(file_count metadata + fixed IO buffer)`，不随项目总字节数线性增长。
测试使用可观察 reader/stat seam 断言最大单次 buffer 上限，不用不稳定的系统 RSS 阈值代替合同。

### 4.3 ProjectOpenPreparationModule

Native application 拦截 `OpenProject` / `SelectRecentProject` 后创建唯一 operation。后台 worker 只做
可隔离的只读准备：manifest/readiness、trust/composition identity、项目摘要、intent journal
装载与 binding 校验、default scene 预读。worker 返回 typed `PreparedProjectOpen` 或 typed failure；
UI 线程只执行有界 commit，安装 active session、recent entry、prepared intent/binding、scene 和异步
Asset Browser refresh。

不得把整个可变 `EditorSession`、Gateway transaction owner 或 renderer 移交后台线程。commit 前必须
复核 project/manifest identity；源发生漂移时拒绝过期 prepared result，并允许用户重新打开。任何后台
结果只消费一次，旧 operation id 的迟到结果不得覆盖新状态。

### 4.4 ProjectOpenActivityModel 与重复点击

增加通用 typed model，不用散落字符串或 Tower 特例：

```text
operationId
projectDisplayName
phase: trust_check | cache_check | staging | compiling | launching |
       waiting_readiness | reading_project | computing_digest |
       loading_workspace | completed | failed | cancelled
completedUnits / totalUnits（未知时 totalUnits=None，显示不确定进度）
elapsedMs
cancellable
diagnosticCode / nextAction（仅终态）
```

同一时刻只允许一个 project-open/composition operation：

- Launcher 的打开、新建、最近项目行和刷新在 busy 时 disabled；
- 重复点击不排队、不重启 worker、不重复写 trust receipt，返回同一 operation 的 busy feedback；
- application tick 持续 pump worker、更新 elapsed/progress、compose model 和 request redraw；
- 完成、失败、取消、worker disconnect、candidate readiness timeout 都必须清除 busy；
- 关闭窗口时有界取消并 join owned worker，不遗留后台进程。

### 4.5 可见进度

Launcher 使用已有 ordered painter 和 267 localization catalog 绘制单层 progress surface，不创建
嵌套 card。显示项目名、当前中文阶段、已耗时和进度条/不确定动画；不能显示虚假百分比。

composition preparer 增加内部 progress sink/channel，至少发布：cache check、staging、compiling、
launching、waiting readiness。Cargo 总任务数未知时使用 `totalUnits=None`；不为了百分比解析不稳定的
人类日志。项目打开 preparation 发布 reading project、computing digest、loading workspace。

所有阶段文案通过严格 Catalog key 提供 `zh-CN` 和 `en-US`；稳定 diagnostic code 保留，不把动态
路径直接拼进翻译 key。进度 surface 必须遮挡自身区域、文本不重叠，并通过 1280x720 与 1600x900
draw-list/真实窗口检查。

## 5. 预计涉及文件

激活前以当前代码复核为准，预计 owner/consumer：

```text
rust/crates/editor_core/src/candidate_project_revision.rs
rust/crates/editor_core/src/project_candidate_entry.rs
rust/crates/editor_core/src/project_intent_workflow/mod.rs
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/services/project_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/lib.rs
可能新增：rust/crates/editor_core/src/project_open_preparation.rs

rust/crates/editor_ui_model/src/launcher.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_ui_model/src/localization.rs 或 resources/locales/*

rust/crates/editor_ui_renderer/src/panels/launcher.rs
rust/crates/editor_ui_renderer/src/tests.rs

rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/headless_app.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/editor_window_winit/src/tests/project_editor_composition_production_gate.rs
```

如果实现需要修改 `engine_runtime`、公共 AI Tool schema、真实项目 manifest 或塔防代码，立即停止并
回到方案讨论；不能在施工中自行扩大范围。

## 6. Gate A - 红反馈与摘要域合同

先增加并运行能捕获真实缺陷的 owner tests：

- fixture 含 `Tests/Feature/Cargo.toml`、源码与 `Tests/Feature/target/...`；改变 target 大文件前后
  digest 必须相同，改变源码后必须不同；修复前应失败；
- `RuntimeModule/target` 与 root generated roots 仍保持排除；
- 普通名为 `target`、但不在 Cargo root 下的项目源目录不得被静默排除；
- link/junction/reparse point 继续 fail closed；
- streaming test 验证 canonical digest 与小 fixture 既有协议一致，且读取 chunk 有固定上限。

建议定向命令：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core candidate_project_revision -- --nocapture
cargo test -p editor_core project_candidate_entry -- --nocapture
```

Gate A 结束条件：至少一个嵌套 Cargo target 测试在旧实现上明确 red；修复后 owner tests 全绿，且没有
读取真实 11.59 GB target 作为测试前置。

## 7. Gate B - 流式摘要与 consumer closure

- 落地统一 source policy 和 streaming hasher；
- candidate project digest、stage、copy、changed-path diff 使用同一枚举策略；
- project intent、GoalMutation、Gateway candidate/rollback consumer 不出现摘要语义分叉；
- 对 excluded-only 变化保持 digest，source 变化保持 drift fail closed。

验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core candidate_project_revision -- --nocapture
cargo test -p editor_core project_intent_workflow -- --nocapture
cargo test -p editor_core goal_mutation -- --nocapture
cargo test -p editor_core ai_capability_tool_kernel -- --nocapture
```

## 8. Gate C - 异步项目打开与 exactly-once

- 建立 `ProjectOpenPreparationModule`、operation identity、worker lifecycle 与 prepared commit；
- 把摘要/intent load 等重活移出事件分发调用栈；
- 增加可阻塞 test adapter，证明 worker 未完成时 application tick/redraw 继续；
- 连续点击同一/另一项目只存在一个 worker，返回 busy feedback；
- 迟到、断连、取消、关闭窗口、prepared identity drift 都 failure closed 并释放 busy。

验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit project_open -- --nocapture
cargo test -p editor_window_winit native_app -- --nocapture
cargo test -p editor_core project_service -- --nocapture
```

## 9. Gate D - 进度模型、本地化与视觉

- typed activity model 覆盖 composition 与 project-open 全阶段；
- Launcher command/row availability 由同一 busy truth 派生；
- `zh-CN` 默认显示阶段、耗时、失败摘要，`en-US` 完整同键；
- 不确定任务不显示虚假百分比，确定任务进度单调不回退；
- draw list 检查 progress surface painter order、clip、文本 bounds 和 disabled hit targets。

验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_model launcher -- --nocapture
cargo test -p editor_ui_model localization -- --nocapture
cargo test -p editor_ui_renderer launcher -- --nocapture
cargo test -p editor_window_winit project_editor_composition -- --nocapture
```

## 10. Gate E - 真实 Tower Defense 开启资格

在 fresh run-owned target/state/evidence root 使用源码 specialized Editor，不修改真实用户配置、不替换
production/安装态二进制：

1. 保留真实 `Tests/**/target`，启动普通源码 Editor；
2. 点击 Tower Defense，确认首帧 activity 及时出现，composition build/cache 阶段持续更新；
3. 构建期间窗口保持 `Responding=True`，重复点击没有第二 worker/process；
4. specialized Editor readiness 后自动完成项目打开，不要求用户再次点击；
5. 记录从点击到 activity、composition、readiness、workspace 的时间和 operation id；
6. 检查 1280x720、1600x900 中文进度 surface，无文字重叠或后层穿透；
7. 关闭 owned Editor，保留 evidence；不删除塔防生成目录。

本 Gate 不是 Local CI，也不是 263/P0-5 Gate G/H。若真实 Cargo build 超出施工窗口，保留阶段证据并
停止，不用 cache 命中伪装首次构建通过。

## 11. Gate F - 受影响回归与文档闭环

按实际修改 closure 执行，不机械运行 workspace 全量：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit
cargo fmt --check
```

若共享 localization/model contract 影响 feature composition，再补对应 `--all-features` consumer；不得把
Local CI 或 production replacement 偷换进 Gate F。全部通过后：

- 生成 `阶段完成记录/2026-08-03-Project-Open-Responsiveness-Progress-v1/00-总览.md`；
- 同步 `00-文档地图.md`、`00-阅读顺序.md`、`49`、`54`、施工文档 README 与完成记录 README；
- 把本文档移至 `施工文档/已完成/268-当前可自动化施工文档-Project-Open-Responsiveness-Progress-v1.md`；
- 明确记录没有删除项目产物、没有修改真实配置、没有替换 production、没有运行 Local CI。

## 12. 停止条件

以下任一出现时停止当前 Gate，不跨 Gate 堆积：

- 无法用 agent-runnable owner test 捕获嵌套 Cargo target 摘要缺陷；
- 为实现异步打开必须并发共享可变 `EditorSession` 或绕过 transaction authority；
- prepared result 无法在 commit 前做 project/manifest identity 复核；
- composition progress 要求解析不稳定 Cargo 人类输出或伪造百分比；
- 需要删除塔防 target、修改塔防代码、替换 production 或修改真实配置才能过验；
- 当前 dirty worktree 与 owner 文件出现无法保留的冲突；
- 连续施工接近 2 小时 45 分钟仍未进入收尾。

## 13. 施工自审

- [x] 四项用户可见问题位于同一 project-open operation，未拆成互相不闭环的小修。
- [x] 根因由真实文件规模、调用链和 Windows AppHangB1 证明，不把 Asset Browser 当首因。
- [x] 摘要仍覆盖项目源，排除仅由通用 generated/Cargo 规则决定，没有 Tower 特例。
- [x] streaming 保持 canonical 顺序，并用有界 buffer 合同测试，不用 RSS 猜测。
- [x] 后台只负责准备，UI commit 保留现有 transaction/Gateway authority。
- [x] progress 是 typed、可本地化、可测试的产品状态，不是日志文本或假百分比。
- [x] 重复点击、迟到结果、失败、取消和 shutdown 均有 exactly-once/cleanup 条件。
- [x] 每项可执行需求都有 owner test，另有真实 Tower consumer 资格。
- [x] 激活前确认当前施工槽无其它活动施工文档；254-R2 历史证据原样保留，不计为活动施工。
- [x] 未授权 Local CI、production replacement、真实配置修改或 Tower P0-5 Gate。

自审结论：范围完整、没有扩大到玩法或公共 runtime，测试能捕获真实故障；激活前复核通过，按一个
连续窗口执行 Gate A-F。

## 14. 完成结果

- Gate A-B：统一 generated/Cargo source policy；任意 Cargo tree 的 `target` 均排除，普通源码
  `target` 仍参与摘要；摘要与 changed-path hash 改为 64 KiB 有界流式读取，canonical 协议不变。
- Gate C：项目读取与摘要进入唯一 owned worker；重复点击返回 `editor.project_open.busy`；关闭应用
  会取消并 join worker；prepared commit 前复核 canonical root、projectId 与 manifest digest，漂移失败关闭。
- Gate D：Launcher 使用 typed activity 显示中文/英文阶段、不确定进度条与耗时；打开期间所有入口和
  recent row 共用 busy truth；1280x720、1600x900 draw-list bounds/disabled target 测试通过。
- Gate E：两个 fresh run-owned 根完成真实 Windows/Tower Defense 资格。第一轮发现 generated Editor
  二次 trust，修复为经 readiness 确认且 composition identity 精确匹配的一次性自动打开；第二轮
  generated Editor 无二次确认直接进入塔防工作区，进程保持 Responding。
- Gate F：`editor_core`、`editor_ui_model`、`editor_ui_renderer`、`editor_window_winit` 全量 owner
  suites 与 `cargo fmt --check` 通过；最终 window suite 为 172 passed / 1 ignored。

真实证据：

```text
.artifacts/268/fresh-real-20260803-1/composition-progress-live.png
.artifacts/268/fresh-real-20260803-2/specialized-auto-open-fixed.png
.artifacts/268/fresh-real-20260803-2/specialized-workspace-clean.png
```

边界：未删除或修改 Tower `Tests/**/target`、`RuntimeModule/target`；未修改 Tower 玩法、资产、
RuntimeModule 或 manifest；未修改真实用户配置；未替换 production/安装态二进制；未运行 Local CI；
未恢复 263/P0-5 Gate G/H。
