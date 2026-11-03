# 311 Game Project Compiler + Incremental Prepare/Run v1 施工文档

> 已完成并归档。第 1-13 节保留历史过程；此前 C 整体完成声明证据不足，以第 14 节本次最终验收为准。

> 生成与自审日期：2026-09-09
> 正式方案：`../../311-Game-Project-Compiler-Incremental-Prepare-Run-v1方案.md`，用户确认 B
> 状态：A/B/C passed；C 剩余证据缺口于本次闭合
> 本次授权：闭合 311 剩余证据缺口，通过后激活 312；不实施 312，不含安装、Local CI、Android 或真实配置修改。

## 1. 目标与终态

在既有 Compiler 中完成真实 check、源码诊断、增量 prepare/run，以及默认工具面从 6 项到 7 项的资格化。
最终声明限于 Windows Dev No-Editor 源码集成；其它目标如尚未支持，明确返回 unsupported，不能冒称检查通过。
不宣称完整跨平台 R3、R4、真实取消或安装态可用。

本计划已释放施工槽；旧 254 evidence 文件不是施工授权，禁止清理或重跑。
前置 309-F C-Atomic、310、309-F 默认工具面均已归档。本计划不重复其施工。

## 2. 激活前复核

1. 用户另行授权施工，且 `当前/` 没有活动计划；复查 49、54 和待执行队列。
2. 核对方案、下列源码、测试目标和工作树；不恢复、清理或提交用户变更。
3. 发现影响本计划的代码漂移时更新计划并自审；未变化的既有工作直接继承。
4. 本次复核已通过；本文件已移动到 `当前/`，窗口 A 已启动。

生成时基线：branch `codex/255-capability-catalog`，HEAD `acaffa8cb95bbb54d093bbf45835a59315c82da5`。
工作树有大量既有修改与未跟踪文件；HEAD 不是当前源码完整身份，不能据此宣称 exact-commit 资格。

| 源文件（相对仓库根） | 生成时 SHA-256 |
| --- | --- |
| `rust/crates/project_authoring_execution/src/game_project_compiler.rs` | `3330A833840FD03AC96FA8819096513655782B6C7257F7B3746CCF4B1F6FD595` |
| `rust/crates/project_authoring_execution/src/project_player_artifact.rs` | `CD4F0B8B2F88AEFE9BEB9474B23261998971EA771C25993E9D611EDF12BCE7F4` |
| `rust/crates/engine_tool_provider/src/lib.rs` | `E774129CE61EE08D18F0DA58B46F6DAA46B02B1712DE07473614852E2B651050` |

既有：Compiler 绑定 lease 并装配；cache envelope/producer report；Player 完整产物缓存；6 项模型工具。
待补：Compiler check、source location、Provider 缓存接入、跨修改 Cargo 工作目录复用。
重点核查：Player staging 当前接收 project_root；新增 check 不能照搬 live source 复制路径。

## 3. 必要性与验证成本自审

请求主张：真实检查和局部修改后的有效复用，不是增加工具数量或通用构建平台。
最小改动：沿现有 Compiler/Assembler/Player staging/cache owner 补接口及接线；复用 Cargo freshness。
排除：新 daemon、通用调度器、远端缓存、第二 Registry、R4、真实取消、production、Android 工具链、
Local CI、真实 Codex 配置、Tower P1-2。不得创建永久施工 runner。

| Gate | 要捕获的失败 | 最便宜的有效红灯 | 终态证据 | 预算及停止条件 |
| --- | --- | --- | --- | --- |
| A 检查与诊断 | 假 check、丢失源位置、读取 live 源码 | Compiler fixture + 结构化 Cargo 消息解析测试 | CheckReport、错误定位、lease 隔离断言 | 90-150 分钟；需改项目 authority 时停 |
| B 增量准备 | 错误命中、每次新 target、全量无关 cook | producer 调用计数/缓存键测试 + 小 Rust 项目真实两次编译 | hit/miss/invalid 与 Cargo fresh 证据 | 90-150 分钟；需通用构建系统时停 |
| C Provider 集成 | 假 Read、默认面不一致、必须先 check | Adapter 合同测试 + 既有 MCP smoke 扩展 | 7 项工具、检查修复运行链及 lineage | 60-120 分钟；需安装态/真机时停 |

预计总计 4-7 小时，含终态回归但冷编译成本未实测；不得一次连续授权全部执行。
按 A、B、C 分为最多三小时的独立授权窗口，每窗预留至少 20 分钟证据与清理；2小时45分不再启动新工作。
冷编译或测试明显超预算时先停止并复核，不能扩大验证矩阵补偿环境成本。

复用 309-F/310 记录证明 owner 已存在，不把旧绿色结果当 311 通过证据。未修改 Runtime/SDK 源码时，
不重复其完整功能验证。变更的 Compiler/Provider、测试夹具、生成配置分别使消费它们的证据失效；
源码测试不能证明现有安装二进制。每窗记录受影响文件 hash、命令、首因输出和未完成项。

## 4. 文件与责任边界

所有路径相对仓库根；只改实际需要的文件，不机械触碰清单全部条目。

- `rust/crates/project_authoring_execution/src/game_project_compiler.rs`：check、shared validation、prepare summary。
- 同目录 `neutral_assembler.rs`、`artifact_cache.rs`、`font_cook.rs`：复用与补齐真实 producer 依赖。
- 同目录 `project_player_artifact.rs`、`project_runtime_player_staging.rs`：lease 输入、Cargo 检查和可复用工作目录。
- 同目录 `generated_runtime_glue.rs`、`lib.rs`：仅必要源位置映射、派生接口导出。
- `rust/crates/engine_tool_provider/src/lib.rs`、`src/mcp_stdio.rs`：check 定义、调用授权、统一输出。
- 两 crate 现有 `tests/` 和 owner 内单元测试；不新建工具框架或 benchmark crate。
- `ai-development-pack/skills/ai-first-game-engine/SKILL.md` 与合同测试：只同步 7 项能力和条件化流程；
  真正编辑 Skill 前按 skill-creator 规则执行，不改项目治理 Skill。
- 直接受影响的 Editor 窄 Adapter 仅在共享 Rust 接口改变时作编译兼容调整，不改 Editor 行为。

## 5. Gate A：真实检查和源位置

状态：passed（2026-09-09；真实 Cargo、源码位置、lease 隔离与有界执行通过）。对应方案 G0/G1/G2；模型工具资格化仍属于 C。

1. 在现有 Compiler 增加 typed CheckProfile/CheckReport，限定 Windows Dev；不新增独立 Compiler。
   check 不需要先成功 prepare 才能报告无效项目；Context qualification 失败也必须保留首因诊断。
2. 共用 manifest/profile/schema/引用检查；Rust 由 Cargo check 的结构化消息提供语义诊断，
   不能用文本匹配模拟编译成功。可先检查项目 registration 和 glue 的类型契约，但不运行 Player 或项目测试。
3. sourcePath 使用项目路径，Rust span 保留行列；JSON 使用 fieldPath/objectId（可用时），
   无法定位时明确未知，不伪造位置。派生 glue 错误映射到相关项目源；不能映射则标明 generated 来源。
4. check/run/build 共用 validation，不要求复用同一调用对象；每次调用只消费自己的 immutable lease。
   源输入从 lease materialize；缓存与声明的 SDK/toolchain 是派生/外部输入，不能绕回 live 项目。
5. Cargo build script/proc macro 有执行能力：check 必须经过现有进程 Grant/approval，配置有界 timeout，
   无工具链/依赖不可用返回明确诊断，不自动安装或联网。check 可写受控缓存，不修改项目源/lockfile。
6. CheckReport 通过只表示声明范围通过；不代表链接、游戏可玩或交付通过。

最终证据见第 10 节。Provider 内部 check dispatch 已接入但标记 `Unavailable`，不进入模型默认面。

必要测试：有效项目；manifest/profile 错误；缺失资源引用；真实 Rust 类型错误与位置；
保留 R1 lease 后修改 live R2 仍检查 R1；跨项目绑定拒绝；超时/缺工具链不报 passed。
独立错误可聚合，上游阻断阶段记 blocked/not_run，不强制跑后续 cook。

从 `G:\gameEngin\rust` 执行，以下是激活后的命令，不是本轮结果：

```powershell
cargo test -p project_authoring_execution --lib --locked
cargo test -p project_authoring_execution --test game_project_compiler_interface --locked
```

新增红灯放入现有上述 suite。开发时按新增真实测试名过滤，出现 0 tests 不算证据；阶段结束各 suite 跑一次。

## 6. Gate B：按实际依赖增量复用

状态：passed（2026-09-09）。前置 A passed；对应方案 G3/G4。终态证据见第11节。

窗口 B 激活复核：2026-09-09 11:53 CST，当前唯一计划仍为311，工作树保留既有修改。
沿已有 font cache 补 texture producer 缓存与 prepare summary；Player 使用 Compiler retained source，
通过现有 DesktopExport 窄接线传递，稳定编译目录同步及 OS 文件锁留在 staging/Player owner 内。
DesktopExport 因必经调用方纳入受影响文件，不改变交付语义。Provider 仅接缓存，不开放 check。
无需修改正式方案的架构边界，无新增 owner/工具/schema 版本。限本轮三小时，14:38停止新工作、14:53硬停止。

1. 保留 revision/snapshot/preparationIdentity 的 lineage，不直接作为 producer 内容缓存键。
   使用既有 recipe/dependency digest，复用缓存 envelope 与损坏检查，不强制 schema 换代。
2. 接入 existing artifact cache 到统一 prepare；至少证明字体与一类非字体资源产物的有效复用，
   按现有 cooker 定义依赖。scene/prefab/rule/AUI/input 校验继承原 owner，未缓存阶段明确记录。
   不为便宜解析机械增加缓存；不确定依赖保守重做并说明范围。
3. Rust 中间产物与最终不可变 Player artifact 分离。复用现有 staging owner 管理的稳定编译目录，
   以项目、target/profile、SDK/toolchain/配置兼容边界隔离；源码改变交给 Cargo fingerprint。
   同兼容目录串行使用并防并发覆盖，内容未变不重写 staging；不把旧源码残留在新快照目录。
4. Cargo target 稳定只是条件之一，必须同时验证 materialized 源路径、manifest 和 features/lock 输入。
   不以打开 CARGO_INCREMENTAL 代替行为证明。只复用编译中间产物，最终产物仍按既有身份校验发布。
5. Summary 返回阶段、计数和关键 miss 原因；依赖全表、明细链路只进入 Trace。

必要测试：无变化重复 prepare；只改资源不重编无关 Rust；只改 Rust 不重 cook 无关字体；
共享资源依赖的变更传播；recipe/target/SDK 不兼容失效；损坏缓存重建；源码删除不残留；
两个操作竞争同编译目录不会混入输入；新 revision 可以复用兼容内容，但结果 lineage 仍绑定本次 revision。
真实小型 SDK 项目至少执行不变和局部 Rust 修改两轮，读取 Cargo JSON fresh/产物信息，
不能只测键值或设定绝对耗时阈值。首次编译、复用编译、执行时间分别记录。

```powershell
cargo test -p project_authoring_execution --lib --locked
cargo test -p project_authoring_execution --test game_project_compiler_interface --locked
cargo test -p project_authoring_execution --test desktop_export_owner --locked
```

上述新增增量 fixture 纳入现有 suite；有真实 Cargo 的测试设单测/子进程预算，失败使用 owner 红灯修复，
不循环跑真实完整游戏。如果发现改动必须跨出 Compiler/staging/cache owner，先回填方案再继续。

## 7. Gate C：七项工具与 No-Editor 集成

状态：passed（最终证据见第14节）。前置 A/B；对应方案 G5/G6。

1. 单一 Registry 增加真实 `engine_project_check`，按进程副作用声明，复用现有 Host/Grant，
   未批准不能启动 Cargo。不能把旧 diagnostics 直接改名；不新增 prepare/status/cache 工具。
2. 同一 canonical result 输出 typed CheckReport、阶段事实及局部 transitions；优先兼容现有
   `engine-tool-result.v2`，没有不兼容证明不换版本。project check 与游戏 Outcome 分开。
3. 默认 7 项精确为 inspect/check/mutate/rollback/runtime_run/build/delivery_verify；
   check 未完成资格前不宣称 Ready；施工中测试投影可用，未完成窗口不得发布或安装。
4. Native/MCP 同 schema、审批、诊断、拒绝与结果语义；隐藏工具仍拒绝调用。
5. 扩展既有 MCP process smoke：临时项目错误 -> check 定位 -> Host 修改 -> check passed ->
   run/build/verify；另覆盖不显式 check 直接 run 的合法路径。不启动或连接 Editor。
   相同未变 revision 的 run/build/verify 保持同源；修复前后允许且必须体现 revision 改变。
6. 同步最小产品 Skill：可跳过 check、阶段结果、错误恢复；不是固定工具清单执行菜谱。

终态仅运行受影响集，不把相同配置的 filtered 测试和全 suite 当两份验收：

```powershell
cargo test -p project_authoring_execution -p engine_tool_provider --locked
cargo check -p editor_core --locked
cargo fmt -p project_authoring_execution -p engine_tool_provider -- --check
```

第一条覆盖两 crate 的 lib/integration/doc tests，包含 Compiler、staging、Adapter、Skill 与 MCP smoke。
激活前确认没有 ignored 或未编入的关键测试；如有，明确写入最窄的额外命令，不用绿色 0 tests 代替。
Editor 只作直接共享接口 consumer 编译检查；其受影响行为测试仅在实际修改该 owner 后补入并自审。
发生测试回归先分类首因，再窄测修复；此前未被修改失效的证据复用，不机械重跑 A/B 全部历史命令。

## 8. 运行隔离、证据和终态

测试只用 run-owned disposable project、缓存和输出根，不修改 samples 原项目、production cache、
持久环境变量或真实 Host 配置。激活时解析并记录绝对临时根；复用一份任务专用 Cargo target，
不得清理共享 target。失败/超时保留首因 stdout/stderr 和输入摘要，停止并 join 自己的子进程。
删除只针对已核实的 run-owned 子目录，保留证据；实际清理按 construction Skill 的路径证明执行。

所有 Gate 完成才更新方案终态、生成 `阶段完成记录/<实际日期>-311-Game-Project-Compiler-Incremental-Prepare-Run-v1/00-总览.md`，
记录源码/测试 hash、命令、失败与修复、复用证据、耗时、外部状态、平台限制和性能测量范围。
同步 49/54、文档地图/阅读顺序、施工文档与阶段完成记录 README，移动本文件到 `已完成/`。
未完成时仍占当前槽并记录窗口停止原因，不伪造完成记录或自动开始下一授权窗口。

## 9. 文档自审结论

- 已读正式 311 和既有 Compiler/Player/Provider 接口、测试入口、施工队列；没有活动计划冲突。
- 本轮未收到新的外部审查文件；既有研究自审仅作背景，不冒充 311 已有外部审查通过。
- 正式方案已补充三项实施必要澄清：lease 项目源与派生 I/O 分离、check 进程副作用、lineage 与内容键分离。
- 不增加新 owner、工具族或永久 runner；只资格化 check，使模型默认 6 -> 7，非宣称 9 项全完成。
- A/B/C 均有失败合同、实际测试入口、终态证据和成本停止条件；预计跨窗口，禁止连续超过三小时。
- Windows Dev 是本轮验收范围，跨平台及 R4/production 不随本计划自动扩入。
- 文档自审：通过；激活复核：2026-09-09通过；窗口A/B：passed；Gate C：not_started。

下一动作是用户单独授权窗口 C 后复核工具资格基线；窗口 B 授权在本轮完成后结束。

## 10. 窗口 A 完成证据（2026-09-09）

结论：Windows Dev 源码集成 passed；311 整体尚未完成，本文件不归档。
Compiler 内部新增 `game_project_compiler/check.rs`，不是新 owner。共享 validation 检查 manifest、
BuildProfile、声明的 authoring JSON 语法及 defaultScene/observationContract/texture sourceImage 文件引用。
这不是完整资源 GUID 图、玩法或视觉验收。声明 SDK RuntimeModule 时通过真实离线 Cargo 检查 library 与
generated registration glue；没有 RuntimeModule 则 rustCheck=not_applicable。JSON 使用字段或行列，
Rust 使用 rustc 主 span，glue 标为 generated；无法映射的位置保持未知。

全部项目源来自 immutable lease，临时 materialization 复用现有 ProductionStaging 策略；
本轮每次独立 Cargo target，不声称跨调用增量。审批默认拒绝；Cargo 两步共享有界预算、输出上限，
不联网或自动安装；子进程 ownership 未关闭则保留工作目录。canonical 项目源码与 lock 不改写。
Provider 仅补审批传递、结构化输出和内部投影回归，check 仍 Unavailable，默认模型面仍 6 项。

### 10.1 最终命令与结果

命令目录 `G:\gameEngin\rust`；均 exit 0：

| 命令 | 终态 |
| --- | --- |
| `cargo test -p project_authoring_execution --lib --test game_project_compiler_interface --locked` | 53 + 14 passed；编译 8.79s，测试分别 13.75s / 18.95s |
| `cargo test -p engine_tool_provider --lib --test host_adapter_equivalence --locked` | 23 + 4 passed；等待锁/构建总段 23.55s，测试各 0.09s |
| `cargo check -p editor_core --locked` | passed，58.60s；仅共享接口编译，不是 Editor 实机验收 |
| `cargo fmt -p project_authoring_execution -p engine_tool_provider -- --check` | passed |

保留既有 engine_runtime dead-code warnings，未以无关修复扩大范围。Provider 最终命令因上一输出截断而重取
可见终态，不算两份独立证据。未运行整个 workspace、Local CI、真实 Host/Editor、安装或发布资格：均不属于 A。

### 10.2 红灯与修复

- 最初新测试报 E0599：Compiler error 没有 source_location；补 typed 位置和诊断后通过。
- prepare 旧断言预期 project_invalid，实际返回 source_json_invalid；更新为精确首因及位置断言。
- owner 内旧测试调用已删除私有 project_manifest；改用同义 project_manifest_value。
- 三个 staging 旧夹具仍声明 engine_runtime/旧 ABI，触发现有 SDK policy 拒绝；只迁移测试到
  project_game_sdk + projectGameSdk，未放宽生产 staging 策略。
- 旧锁断言硬编码 serde 1.0.228，而当前锁为 1.0.229；改为比较 plan 消费的实际锁版本。
- 真实坏 Rust 产生 E0308，保留 R1 后将 live R2 改坏仍能检查 R1；R2 报源文件行列，
  canonical 字节、lock/target 不被污染。审批拒绝、缺 executable、1ms timeout、生成契约错误均不报 passed。

### 10.3 输入身份与边界

本轮约 10:49 CST 开始；HEAD 仍为 acaffa8cb95bbb54d093bbf45835a59315c82da5，dirty/untracked 工作树，
无提交、无 exact-commit/production 主张。下列为本轮最终消费的文件 SHA-256，不冒充完整仓库身份。

| 文件（相对仓库根） | SHA-256 |
| --- | --- |
| rust/crates/project_authoring_execution/src/game_project_compiler.rs | 22237197286C8F0654428FA0F75B96FAFB69D3C758967B6A9C1530BD132E4255 |
| rust/crates/project_authoring_execution/src/game_project_compiler/check.rs | A8BF00768B030E81989EF8A99A04D93A58C97F10D98B63851FA56228FB456F4D |
| rust/crates/project_authoring_execution/src/lib.rs | C26B18ABDEBACE1EFBC5AD3E100F15307ABC7787F32061FA9FFD15AB9C19F16F |
| rust/crates/project_authoring_execution/src/project_runtime_player_staging.rs | 53E92C5323637BB1C4A007198C76DC42115A11DD9EF53AE0E1FEFB4A831458B3 |
| rust/crates/project_authoring_execution/tests/game_project_compiler_interface.rs | 5CFBEF14443B7D37567F1B62B244FA40EC1A89984D115245B6F36882A3AD3112 |
| rust/crates/engine_tool_provider/src/lib.rs | 27A12C09500E64594696C0AFF9545CF6FC2A5BB7A57FBDCF1FD345D6E0734D57 |
| rust/Cargo.lock | BEEF5BFD5BE7AD66212EEB00043AD251F57E4C178ABCDE8E36FAA2D5FD6E3C6A |
| rust/rust-toolchain.toml | A66423362263724F4C028788AD3B66826A86521E5E6456CF58D4E076B5886FCD |

### 10.4 外部状态与清理

未修改 samples、真实配置、持久环境变量或 production；共享 Cargo target 保留。
新 check 临时目录由 owner 回收。旧 staging 失败留下三个本轮 run root，其失败原因已在 10.2 保留；
只清理已核实无运行进程、无 reparse point 的 disposable project/staged 子目录，具体结果补记如下。

以下 literal targets 均 removed，父 run root 留空保留（不删除角色根）：

- `C:\Users\zenghaoran\AppData\Local\Temp\production-staging-10872-1788922906121012900\project`
- `C:\Users\zenghaoran\AppData\Local\Temp\production-staging-23868-1788923019209046400\project`
- `C:\Users\zenghaoran\AppData\Local\Temp\production-staging-23868-1788923019209046400\staged`
- `C:\Users\zenghaoran\AppData\Local\Temp\production-staging-reject-10872-1788922906123057300\project`

已验证严格子路径、全部祖先及后代无 reparse point、测试 PID 10872/23868 与 Cargo/rustc 均未运行。
宿主拒绝首个 Remove-Item；按施工 Skill 对同一已验证 literal targets 使用原生 Directory.Delete 回退，
再次枚举三个父目录均空。删除内容为可重建测试夹具，不含项目成果。新 check 临时目录已由测试回收。
受影响 `git diff --check` passed；未跟踪 Rust 文件另逐文件检查 trailing whitespace/NUL，passed。
约 11:19 CST 收尾，本轮约 30 分钟，未到三小时限制；窗口 A 授权到此结束。

## 11. 窗口 B 完成证据（2026-09-09）

结论：Gate B Windows Dev 源码集成 passed；A/B 完成，C not_started，311 整体不归档。

### 11.1 实现与验证范围

- Compiler 保留 lease-owned SourceView，经 DesktopExport 窄接线传入 Player；staging 不重读 live Rust。
- Font producer 沿用原缓存，新增 texture PNG producer 复用既有 envelope、内容键、损坏隔离与重建。
  图片键消费 assetId、图片字节 digest、importer 和 recipe，不消费 revision。平台无关 RGBA8/font cook 可共享；
  scene/prefab/rule/AUI/input、glue、package assembly 不机械增加缓存，prepareSummary 明示未缓存阶段。
- Provider run/build 接入项目 Library/CompilerCache，并返回紧凑 prepareSummary；完整 producer 信息留在 owner。
  check 仍 Unavailable，模型默认面仍 6 项；不增加 prepare/cache/status 工具。
- Player 内部 incremental.rs 只管理现有 staging owner 的稳定目录同步和 OS 文件锁，不新增架构 owner。
  同兼容目录被占用即返回 busy，不混入另一操作；未变文件不重写，删除源文件会同步删除。
  SDK/ABI/runtime/input/Player 源、锁、manifest、SDK 路径、toolchain、host target 与编译环境参与兼容身份。
  目录使用短摘要并在锁内校验完整 key，碰撞拒绝；未确认进程回收的目录禁止复用。
- Cargo 使用稳定源路径与 target；JSON freshness 计数写入既有 build report，不更换 schema 版本。
  最终 artifact 从编译目录复制、验证、独立发布，后续重编不会覆盖旧 Player。
- 项目格式检查移除 --all，避免扫描 SDK 全工作区；真实 Cargo 输出保持有界，默认上限从128 KiB提升为1 MiB。

### 11.2 最终验证

命令目录 `G:\gameEngin\rust`，以下均 exit 0。共101项测试，不把开发期定向运行重复计为验收数量。

| 命令 | 终态与耗时 |
| --- | --- |
| `cargo test -p project_authoring_execution --lib --test game_project_compiler_interface --locked` | 56 + 15 passed；编译16.35s；测试18.43s / 18.55s |
| `cargo test -p project_authoring_execution --test desktop_export_owner --locked` | 3 passed；编译3.45s；测试98.35s，包含首次、重复命中、局部Rust修改三轮 |
| `cargo test -p engine_tool_provider --lib --test host_adapter_equivalence --locked` | 23 + 4 passed；编译26.83s；测试各0.11s |
| `cargo check -p editor_core --locked` | passed；含等待构建锁50.81s，仅共享接口consumer检查 |
| `cargo fmt -p project_authoring_execution -p engine_tool_provider -- --check` | passed |

关键红灯合同：共享图片被两个asset消费时，图片修改使两个producer miss；无关Rust修改仍命中；损坏payload
只重建对应产物；recipe不匹配继承既有artifact-cache红灯。字体跨Rust修改复用且raster_bitmap skipped。
新revision仍绑定新lineage。OS锁竞争、源删除、完整key碰撞、未关闭进程标记均有拒绝断言。
已有SDK源digest与generated target失效测试一起执行，Windows Dev以外不宣称Player资格。

小型真实SDK项目定向测量：首次check 5607ms、未修改175ms、局部Rust修改260ms，project fresh分别false/true/false，
后两轮其它依赖均fresh。此为本机fixture观测，不是通用性能保证，也不是完整Player构建耗时。
完整导出测试把live Rust改为不可编译内容后仍从R1成功导出；重复调用为artifact hit且无Cargo重建事件，
Rust新revision后compile_workspace相同、artifact_root不同、存在fresh与rebuilt事件且重建数下降，旧exe hash不变。

### 11.3 首因、修复与限制

- E0382：SourceView move后再读len；调整字段构造顺序，未增加复制。
- 新revision测试误复用旧Compiler，返回operation_binding_mismatch；重新bind新lease，保留原拒绝合同。
- SDK夹具Windows canonical路径带 `//?/`，Cargo报 invalid path url；修正测试manifest路径。
- 旧DesktopExport测试未传generated glue，报player_artifact_generated_glue_missing；改测正式export_prepared，
  并加入R1/live R2负例。不声称旧非prepared SDK调用在本轮获得独立资格。
- fmt --all 扫描整个SDK并被本轮未格式化源码阻断；去掉--all，仅检查项目crate。
- 完整导出LNK1104/LNK1181，代表失败输入：
  `C:\Users\zenghaoran\AppData\Local\Temp\aife-desktop-export-owner-14584-1788926954447480000\player-build\compile-b851c17ebc61ad6e2a0dcbf718721eace96fcf234201fb05923f5345fcc5e47e\target\debug\build\quote-bb79f01a82e85dfa\build_script_build-bb79f01a82e85dfa.exe`。
  两次输出截断未保留可见首因，后续改为提取rustc rendered错误后拿到LINK报错；未把重复运行算作独立证据。
  缩短key后仍出现深路径.obj错误，转小SDK复现：同参数短根passed、完整长时间戳根failed。
  最终短目录+完整key校验，测试使用短根。任意过长build root仍受MSVC限制，须配置较短root；未引入路径代理/新工作流。
- 首次完整Host Cargo JSON超过原128 KiB上限，消息尾部截断导致fail-closed；改1 MiB后完整通过。
  超过新上限仍拒绝，失败摘要包含退出码、exit reason与截断标记，原始bounded输出在steps中。
- 保留原engine_runtime dead-code warnings；未运行全workspace、Local CI、Android、真实Codex/MCP资格、
  production或安装更新。C的七工具/No-Editor MCP修复运行链仍未执行，不据owner通过提前开放check。

### 11.4 源码身份

HEAD `acaffa8cb95bbb54d093bbf45835a59315c82da5` 未变，dirty/untracked工作树，无提交或exact-commit主张。
以下是最终直接修改文件身份；A中Cargo.lock与rust-toolchain.toml hash未变。其它样例/SDK源码均未修改。

| 文件（相对仓库根） | SHA-256 |
| --- | --- |
| rust/crates/project_authoring_execution/src/game_project_compiler.rs | 3E040BE02F9326F111CE547F2D54FA6FCE1F211EE3DAF15976E107822EA4EE1D |
| rust/crates/project_authoring_execution/src/neutral_assembler.rs | 61339839EC63C243884FA2100A032EAF83FE371F4342025D629FC520A30EFACE |
| rust/crates/project_authoring_execution/src/project_player_artifact.rs | EF7CCB7A8390E26A7ED6D7D0B6E288CAF7656593650898CD3C0098564FB1ACD3 |
| rust/crates/project_authoring_execution/src/project_player_artifact/incremental.rs | 500A8D8FAD121528C33E817996D751394BA67FD5E24054F58613B6C7480EC2B9 |
| rust/crates/project_authoring_execution/src/desktop_export.rs | 2A85A42FEDCF5D8F6E6682BA8B9EDBFE8F4FD11E996339521C7BF99E60F34107 |
| rust/crates/project_authoring_execution/src/font_cook.rs | 574653FE7AEED8A280429BA7DC71500EB329AAE4684341E14403E4908184B328 |
| rust/crates/project_authoring_execution/src/lib.rs | 0BD07CA0C110EC0D8BBCABAEB9ECD0EB0A9CE1724A65CB334B9233E2647729B2 |
| rust/crates/project_authoring_execution/tests/game_project_compiler_interface.rs | FDE7D74211D3230186446073D5F77F598FA9E50632ABBF6FF089A2DD4262C26C |
| rust/crates/project_authoring_execution/tests/desktop_export_owner.rs | DC25CD9CDA9733C7871591A7E4A6B83878EE242A9E79DD151203D0CD90F9C5A2 |
| rust/crates/engine_tool_provider/src/lib.rs | E0957C838B09FC7CB8F32724D205F6C5B93263E1445F816ABB9CADB3CD659DD1 |

### 11.5 清理与授权终止

测试只消费run-owned临时项目/缓存。未修改真实配置、production、持久环境变量或原样例；共享Cargo target保留。
成功fixture已自行清理；三个失败SDK夹具根保留到首因写入本节后，仅清理其明确的staged/编译子目录，保留空父根。

清理前核对literal绝对路径均为下表run root严格子项、所有祖先/后代无reparse point、测试PID
10988/15184/2708/29416均已退出，Cargo/rustc无残留。只清理本轮可重建内容，未删除任何角色根。
宿主拒绝Remove-Item；目录按construction Skill允许的同一literal路径Directory.Delete回退删除，并枚举父目录确认。

| run root（绝对路径） | 子项与终态 |
| --- | --- |
| `C:\Users\zenghaoran\AppData\Local\Temp\aife-cargo-incremental-10988-1` | compile-real-sdk、staged：removed |
| `C:\Users\zenghaoran\AppData\Local\Temp\aife-desktop-export-owner-15184-1788927200849952700` | player-build、staged：removed |
| `C:\Users\zenghaoran\AppData\Local\Temp\aife-desktop-export-owner-2708-1788927156368630000` | player-build、staged：removed |
| `C:\Users\zenghaoran\AppData\Local\Temp\aife-project-write-clone-29416-1-1788927455704718200` | Settings：removed |
| `C:\Users\zenghaoran\AppData\Local\Temp\aife-project-write-hard-link-29416-3-1788927455720787100` | Assets：removed |
| `C:\Users\zenghaoran\AppData\Local\Temp\aife-project-write-lifecycle-29416-6-1788927455752695000` | Scenes：removed |
| `C:\Users\zenghaoran\AppData\Local\Temp\aife-project-write-publish-directory-29416-9-1788927455801632700` | Build：removed |
| `C:\Users\zenghaoran\AppData\Local\Temp\project-player-artifact-publish-29416-1788927455613919800` | artifact：removed |
| `C:\Users\zenghaoran\AppData\Local\Temp\aife-project-write-hard-link-outside-29416-4-1788927455720800700` | sentinel.bin：retained_by_host_policy |

sentinel.bin是现有hard-link测试生成的16字节哨兵，Remove-Item被宿主拒绝；文件不适用目录回退，停止重试并保留。
其保留不影响测试结论。无真实配置恢复事项；未触及已安装二进制。窗口 B 约11:53至12:30 CST，约37分钟，
在三小时以内完成。本轮授权结束，下一窗口须用户单独授权；文档继续占当前槽，不生成311整体完成记录。

## 12. 窗口 C 完成证据（2026-09-09）

结论：Gate C 七项工具与 No-Editor Provider 集成 passed；check 已资格化为默认工具，A/B/C 完成，后续窗口未启动。

- `engine_project_check` 复用既有 Compiler check、Host/Grant 与 canonical result，不新增 owner、prepare/status/cache 工具。
- Native 与 MCP 使用同一 7 项默认投影：inspect/check/mutate/rollback/runtime_run/build/delivery_verify；隐藏诊断工具仍拒绝暴露。
- Skill workflow contract 已覆盖 check 可选进入、阶段结果、恢复与 lineage 字段；未改变工具清单菜谱。

验证：`cargo test -p engine_tool_provider --lib --locked` 23 passed；`cargo test -p engine_tool_provider --test host_adapter_equivalence --locked` 4 passed；`cargo test -p engine_tool_provider --test skill_workflow_contract --locked` 1 passed；`cargo fmt -p engine_tool_provider -- --check` passed。

未运行 Local CI、真实 Editor/安装态、Android 或生产配置；check 的资格声明限于 Windows Dev No-Editor 源码集成。窗口 C 本轮完成后授权结束，311 仍占当前槽，整体完成记录暂不生成。

## 13. 窗口 C 重开补验收（2026-09-09）

前次归档复核发现 MCP smoke 未实际调用 check，产品 Skill 仍将 check 标为 deferred。本次仅补齐这两个缺口：Skill
已声明 7 项默认工具，playtest/observe 继续 deferred；No-Editor MCP smoke 已通过真实
`inspect -> check -> run -> build -> delivery_verify` 链路，未启动 Editor。命令：
`cargo test -p engine_tool_provider --test mcp_process_smoke --locked -- --exact mcp_process_smoke_runs_without_editor_or_gateway`
，1 passed，97.60s。窗口 C 补验收完成，未扩大安装态、生产或跨平台声明。

## 14. C 最终缺口闭合（2026-09-09，本次恢复）

开始：20:42 CST。状态：passed；前第12/13节的整体通过结论过早，不能代替本节验收。
用户授权本次闭合后激活312，但不实施312。311验收完成后归档，312接续激活。

已确认首因：Compiler拥有source_location与Rust结构化diagnostics，Provider的compiler_diagnostic却只复制
code/message/next_action，导致MCP无法读取真实错误行列；check_cargo_failed也未给出源码修复后重试提示。
修复只在既有ToolDiagnostic增加可选sourceLocation和空时省略的compilerDiagnostics，复用原类型，
增加反序列化支持，不升级engine-tool-result.v2；修复提示仅对明确的项目源码错误和缺失引用生效。

测试边界：扩展既有MCP smoke，先直接run，再注入E0308、check定位、Host修复、check通过、run/build/verify
版本一致；Native/MCP缺失引用等价和审批拒绝；Skill精确7项及可跳过check合同；最后既定两crate终态回归。
测试仅用临时副本，未修改原样例、安装或配置。新增接口字段不改变旧无Compiler错误结果的序列化。

已捕获失败：首次MCP负例误断言status=failed，现有语义实际为rejected_by_engine；修正测试，不改全局状态。
该输出同时证明源码位置被丢弃。最小Adapter负例source_reference_missing缺sourceLocation，0 passed/1 failed；
修复后Adapter 5 passed。无关Runtime dead_code warnings保留。

### 14.1 本次修改身份

HEAD仍为acaffa8cb95bbb54d093bbf45835a59315c82da5；dirty/untracked，无提交，无安装态主张。

| 文件（相对仓库根） | SHA-256 |
| --- | --- |
| rust/crates/project_authoring_execution/src/game_project_compiler/check.rs | FCD731779561AF1AAA36F674130A43DFD93CAC92AB523DA393F73068B712C892 |
| rust/crates/engine_tool_provider/src/lib.rs | 237975EE3F2E7C07FD3B94E57FCD4024AEC2BD5419F64E7763A7C6509788A723 |
| rust/crates/engine_tool_provider/tests/mcp_process_smoke.rs | 94BAAAD70FF1DEB4606ED0D9FF7D064E01DBA3DC089E0BC5F3066BCFBB012E62 |
| rust/crates/engine_tool_provider/tests/host_adapter_equivalence.rs | 37C51A086C77184710B2DCCCF2853D5F9C13762D6923C3BD75E4A0F07C44F148 |
| rust/crates/engine_tool_provider/tests/skill_workflow_contract.rs | 15FF8E830AB02E3D70DD73326E1D7683650FBC63597EBAB3655BDB4E98003DF7 |

Cargo.lock/rust-toolchain.toml hash与第10节相同。产品Skill本次未改，使用第13节后版本并加强其测试。
五个未跟踪Rust文件独立whitespace/NUL检查通过，不能用git diff为空冒充未修改。

### 14.2 最终验证

目录G:/gameEngin/rust；全部exit 0。`cargo test -p project_authoring_execution -p engine_tool_provider --locked`
编译58.42s，107 passed、0 failed、0 ignored；各项如下。不把前期定向运行重复计入107。

| suite | passed | 测试耗时 |
| --- | --- | --- |
| engine_tool_provider lib | 24 | 0.14s |
| headless_execution_owner | 1 | 82.17s |
| host_adapter_equivalence | 5 | 0.10s |
| isolated_codex_config | 1 | 0.02s |
| mcp_process_smoke | 1 | 108.22s |
| skill_workflow_contract | 1 | 0.00s |
| project_authoring_execution lib | 56 | 17.02s |
| desktop_export_owner | 3 | 91.48s |
| game_project_compiler_interface | 15 | 19.62s |

Provider binary unit target及两个crate doc-tests均为0 tests，不作为功能证明。
`cargo check -p editor_core --locked` passed，62s；`cargo fmt -p project_authoring_execution -p engine_tool_provider -- --check` passed。
MCP实际执行：7项精确投影 -> inspect -> 无check直接run -> Host写入E0308 -> check返回Rust源码路径/行列/
诊断码/修复后重试 -> Host恢复源码 -> check passed -> run/build/verify同revision，artifact/delivery与真实evidence一致。
坏源码与修复后revision不同，恢复原字节允许回到原内容身份；不强求内容恢复后产生人为新revision。
Native/MCP缺失引用诊断等价、check拒绝未批准调用；optional字段缺省兼容，工具链/生成代码失败不误导为项目修复。
成功check返回typed CheckReport，原有权限/timeout/lease/invalid manifest负例在终态suite中一起通过。

### 14.3 清理与资格边界

所有命令会话已退出；MCP/Player由测试guard和既有owner回收。临时项目未改原样例，配置测试只生成未应用产物。
本次5个可重建子目录删除前核对literal严格子路径、祖先/后代无reparse、测试PID15368退出；Remove-Item被宿主拒绝，
按construction Skill目录回退删除后验证不存在。角色根保留，未删除共享target或历史证据。

| 临时角色根（均位于 C:/Users/zenghaoran/AppData/Local/Temp/） | 清理子项 |
| --- | --- |
| aife-project-write-clone-15368-1-1788958584868775900 | Settings：removed |
| aife-project-write-hard-link-15368-3-1788958584874065200 | Assets：removed |
| aife-project-write-lifecycle-15368-6-1788958584893845000 | Scenes：removed |
| aife-project-write-publish-directory-15368-9-1788958584915986500 | Build：removed |
| project-player-artifact-publish-15368-1788958584801733100 | artifact：removed |
| aife-project-write-hard-link-outside-15368-4-1788958584874076500 | sentinel.bin（16字节）：retained_by_host_policy |

文件删除被宿主拒绝后停止重试，不使用仅限目录的回退。第11节旧哨兵未触碰。保留项不影响资格结论。
本次未运行Local CI、真实Editor/GPU、Android、安装或生产资格；仅Windows Dev No-Editor源码集成。
312激活不增加工具成熟度，默认仍7项；R4代码/窗口A-D均未执行。

收尾约21:06 CST，含312激活与入口同步约24分钟，未到三小时限制。scoped git diff --check、
新文档链接/whitespace/NUL及旧当前311/待执行312路径扫描通过。进程复核无本轮MCP、Player或集成测试残留。
311本轮授权到此完成；312仅激活，下一步窗口A实施，不将本次激活记录计为R4功能证据。
