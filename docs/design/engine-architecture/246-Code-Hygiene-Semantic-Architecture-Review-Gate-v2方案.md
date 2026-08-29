# 246-Code Hygiene Semantic Architecture Review Gate v2 方案

> 状态：已完成；Gate A-G、真实 Provider artifact、EngineStrict 与 authoritative exact-commit Local CI 全部通过，CQ-07 已关闭。
> 方案日期：2026-07-12。  
> 选题来源：`240-5.6审查剩余问题讨论与施工优先级.md` Priority 6 / CQ-07。  
> 继承：`151-Codebase-Architecture-Hygiene-Gate-v1方案.md`、151 Stage A-K 完成记录、`245-Reproducible-Toolchain-CI-Lint-Budget-Gate-v1方案.md`。  
> 用户确认：选择方案 B，采用“确定性结构检查 + AI 语义审查 + 可审查阻断”；引擎侧彻底执行，项目侧默认只给建议。  
> 并行边界：245 已完成并归档；246 已完成并归档，不重开或替代 245。
> 自审修订：已补入可信 AI Review Artifact、机器可读架构策略、finding 本地证据验证、项目源码授权、分批历史审查、报告 v2 与模型评测合同。

## 1. 这个系统解决什么问题

151 已完成一次代码结构治理，但当前 `code_hygiene.report.v1` 仍然只统计文件数、行数和热点并输出 recommendation。只要扫描成功，Gate 就成功；后续代码可以继续把新职责堆进大文件而不被阻断。

246 不把“文件超过固定行数”直接等同于“架构错误”。它建立两类互补证据：

```text
确定性检查
  -> 判断依赖方向、domain 归属、facade 回流、规模变化和 exception 是否合规

AI 语义审查
  -> 判断职责是否混杂、代码是否放错位置、拆分是否只是 wrapper、
     是否重复已有能力、public API 是否泄漏实现、后续 AI 是否容易安全修改
```

最终目标不是让数字变好看，而是阻止引擎代码的结构继续退化，并让每个阻断结论都有具体证据、处置方式和复审记录。

## 2. 治理对象与强制边界

### 2.1 引擎侧：强制执行

以下仓库内代码使用 `EngineStrict` profile：

```text
Engine Core
Runtime
Editor
Build / Export
Quality / AI infrastructure
其它正式 Rust workspace crates
Cargo manifests / build scripts / workflow / engine schema and policy files
```

引擎侧规则：

```text
Fast Gate 的确定性错误必须阻断。
Merge Gate 必须完成 AI 语义审查。
未处置的 high / critical finding 必须阻断。
不得通过关闭 AI、修改 baseline 或删除 finding 来绕过。
例外必须有 owner、reason、review_by 和明确处置计划。
```

### 2.2 第一方验收项目：建议为主，不冒充引擎 Core

复杂打飞机、Switch Puzzle 等第一方项目用于验收引擎能力，但其玩法 Rust Module 和项目资产不使用引擎内部的绝对阈值。默认生成建议；只有该项目自己的 profile 显式升级某条规则时才阻断。

### 2.3 用户项目侧：默认只建议

项目侧统一使用 `ProjectAdvisory` profile：

```text
Rust Project Framework / Project Module：输出代码与结构建议。
Scene / Prefab / AUI / Rule / Feature Asset：继续走各自 schema、引用和验证合同。
不因 AI finding 阻断 Build、Play、Export 或 RuntimePackage 生成。
用户可以显式开启 ProjectStrict，但引擎不能替用户自动开启。
```

引擎规则与项目规则复用同一检查器和同一 report schema，只通过 profile 决定严重度和阻断策略；不新增运行时架构层。

### 2.4 EngineStrict Scope Manifest

引擎侧“彻底执行”必须有显式范围，不能靠扫描器猜目录。机器可读 `architecture-policy.v1` 至少声明：

```text
engine_strict_include
project_advisory_include
generated/vendor/build_output exclusions
crate/path -> domain / owner
allowed dependency directions
facade_only subjects
review authorities
```

Rust 文件参与规模、symbol 和语义治理；`Cargo.toml`、build script、workflow、schema/config/policy 文件参与依赖、权限、构建和架构入口治理。项目资产继续由各自 Validator 负责，不把行数规则错误套到 Scene/AUI/Prefab/Rule。

## 3. 不进入 Runtime

246 全部属于工程治理：

```text
local quality command
pre-merge review
CI
AI Patch validation
Report Panel evidence
```

它不链接进 Runtime Player，不在游戏帧循环中运行，不读取运行时 ECS，不影响发布包体和游戏运行效率。

## 4. 当前基线与问题

151 Stage K 曾记录：

```text
215 Rust files
70,805 lines
18 files > 1,000 lines
0 files > 2,000 lines
0 files > 4,000 lines
```

2026-07-12 讨论阶段只读统计约为：

```text
348 Rust files
165,348 lines
44 files > 1,000 lines
10 files > 2,000 lines
1 file > 4,000 lines
engine_runtime/src/aui.rs approximately 7,119 lines
```

行数增长本身不能证明设计错误，但它证明 151 的一次性治理没有形成持续防回归合同。现有集成测试只验证文件数和总行数不低于历史下界，也无法捕获恶化。

## 5. 成熟引擎源码参考

### 5.1 Bevy

参考：

```text
https://github.com/bevyengine/bevy/blob/main/tools/ci/src/ci.rs
https://github.com/bevyengine/bevy/blob/main/.github/workflows/ci.yml
https://github.com/bevyengine/bevy/blob/main/Cargo.toml
```

Bevy 使用仓库内 Rust CI runner 统一 fmt、Clippy、test、compile 和 docs，workspace lint 集中声明，并在 CI 中使用 `-D warnings`。它主要依赖 crate/module 边界和严格 CI，不使用一个适用于所有源码文件的统一行数上限。

采用点：复用 245 的深 `QualityGateRunner`，让 workflow 保持薄；结构合同优先于纯行数。

### 5.2 Godot

参考：

```text
https://github.com/godotengine/godot/blob/master/.github/workflows/static_checks.yml
https://github.com/godotengine/godot/blob/master/.pre-commit-config.yaml
https://github.com/godotengine/godot/blob/master/misc/scripts/file_format.py
modules/*/SCsub
```

Godot 对变更文件执行阻断式静态检查，并通过 module 注册、构建描述、ownership 和格式合同维护边界，而不是只按文件行数判断质量。

采用点：优先检查 changed scope、模块归属和 ownership；历史债务不等于所有新工作停摆。

### 5.3 Unity UGUI

本地源码参考：

```text
<UNITY_UI_REFERENCE>/com.unity.ugui/Runtime/UGUI/UnityEngine.UI.asmdef
<UNITY_UI_REFERENCE>/com.unity.ugui/Editor/UGUI/UnityEditor.UI.asmdef
对应 Runtime / Editor test asmdef
```

UGUI 通过 assembly definition 明确 Runtime、Editor 和 Test 依赖边界。采用点是检查依赖和职责归属，不为了降低行数制造只转发调用的空模块。

## 6. 方案比较与正式选择

### 6.1 方案 A：AI 只建议

AI 生成报告但永不阻断。实现稳定，但长期会重新退化成无人处理的 warning。

### 6.2 方案 B：AI 语义审查 + 可审查阻断

正式采用。

```text
确定性 Fast Gate 提供可复现底线。
AI Review 理解职责和上下文。
finding 必须结构化并引用证据。
引擎 Merge Gate 对 high / critical finding fail closed。
项目侧默认 advisory，不阻断用户工作流。
```

### 6.3 方案 C：AI 直接 Pass / Fail

不采用。模型、上下文、Provider 和 prompt 变化会使结论难以复现，而且一句 Pass/Fail 不足以支撑修复和审计。

## 7. 检查档位

```text
Fast
  只运行确定性检查；用于频繁本地开发。

Review
  运行确定性检查并生成 AI finding；用于开发中审阅和 Report Panel。

Merge
  引擎侧完整执行；确定性违规和未处置 high/critical AI finding 阻断。

Runtime
  不存在该档位；正式 Runtime 不运行治理检查。
```

`ProjectAdvisory` 在任何档位都不能因为 AI finding 阻断项目 Build/Play/Export。确定性的安全、schema 或构建错误仍由原有项目验证器负责，不能被本方案降级。

### 7.1 Merge 的两段可信执行

普通 PR CI 保持无 secret、只读和可复现；真实 AI 不直接塞进 245 的 public workflow：

```text
Public Fast CI
  -> QualityGateRunner deterministic stages
  -> 不持有 Provider credential

Trusted Architecture Review
  -> 在受信本机或受保护 CI environment 中调用 Provider
  -> 生成 architecture-review-artifact.v1
  -> artifact 绑定 exact base/head commit、policy、model、prompt、schema 和 context digest

Merge Quality Gate
  -> 验证 artifact provenance / freshness / digest / evidence disposition
  -> 与确定性结果汇总为 quality-gate-report.v2
```

可信 Review 可以由同一 workflow 的受保护 job、独立 required check 或本地受信 reviewer 执行，但规则真相和 artifact validator 必须在仓库内。Provider credential 只进入受保护环境；fork/untrusted PR 不得到 secret。

本项目当前是本地 Git，不要求 remote 或远端 required check。正式权威路径为：

```text
本地受信 Provider Review
  -> 绑定 exact local base/head commit 的 architecture-review-artifact.v1
  -> 本地 evidence/provenance/disposition verifier
  -> exact-commit Local CI / QualityGateRunner
  -> quality-gate-report.v2
```

GitHub Actions 只保留为可选 Adapter；没有 remote 不阻塞 246。没有真实 Provider credential 或没有与 exact local commit 匹配的可信 artifact 仍然阻塞 246 完成，不能用 scripted fixture 冒充真实激活。

### 7.2 验证成本与环境边界

```text
定向测试：每个 Gate 的 schema、parser、matrix、transport、artifact verifier；用于快速收敛。
受影响域回归：code_hygiene、quality_gate、architecture_review、editor_core LLM/report、project_e2e_gate。
最终权威回归：候选冻结后运行一次 default/all-features、真实受信 Provider Review 和 exact-commit Local CI。
```

施工必须在隔离 clean worktree 或等价 clean 基线执行，显式固定 Rust toolchain、Cargo.lock、`CARGO_TARGET_DIR`/artifact 输出根、Windows CRLF/LF 读取、子进程环境传播和 cache key。预检不能替代最终权威回归；权威回归失败后先在等价环境收敛累计失败集，再允许重跑。

## 8. 确定性结构检查

确定性检查只处理可稳定复现的事实：

```text
禁止的 crate/domain 依赖方向。
新模块缺少 domain 归属。
facade 在已拆出职责后重新接收领域实现。
历史热点继续显著增长。
新增大文件或跨越风险阈值。
baseline/exception 缺字段、过期、路径失效或被自动放宽。
已消除的债务重新出现。
```

### 8.0 机器可读架构策略真相

确定性检查的规则来源固定为版本控制的 `rust/quality/architecture-policy.v1.toml`，不得把 domain 和 ownership 继续散落到 Rust `match`、workflow 或 prompt：

```text
schema_version
profiles
domains[]：id、owners、include/exclude globs
dependency_rules[]：from、to、decision、rule_id
facade_subjects[]：path/symbol、allowed_responsibilities
size_risk_bands：仅作为风险信号
review_authorities[]
generated/vendor exclusions
```

策略变更本身属于 `EngineStrict`，必须显示 policy diff、影响范围和批准者；不能和普通功能 Patch 一起静默放宽。Rust symbol/impl/import 使用 `syn` 等结构化解析，crate 依赖使用 Cargo metadata；禁止用不稳定正则表达式冒充语义分析。

行数只作为风险信号：

```text
单独超过 1,000 行不直接证明失败。
大文件 + 明显增长 + 新增异质职责 + 无有效处置，才形成强阻断候选。
全局硬天花板只用于发现极端聚集，仍需提供具体结构证据。
禁止为了过阈值检查制造无意义 wrapper、include 文件或纯 re-export 拆分。
```

### 8.1 旧引擎代码先全量建账

246 首次启用时必须对全部 EngineStrict scope 完成确定性 inventory；AI 语义审查按风险分批完成，不能因为一次扫描成本过大就只治理启用日期之后的新文件。扫描结果形成受版本控制的 `historical architecture debt ledger` 和 `review coverage ledger`：

```text
全部现有引擎代码
  -> 一次性完成文件、symbol、impl、public API 和依赖图 inventory
  -> 按确定性风险排序 AI review batches
  -> 已审查部分形成 AI findings 和人工 disposition
  -> 未审查部分明确记录 coverage_pending
  -> owner / review_by / remediation plan
  -> historical debt ledger + review coverage ledger
```

启动规则：

```text
确定性 inventory 未完成：EngineStrict 不得宣称启用完成。
changed scope 命中 coverage_pending：本次 Merge 前必须先完成该 scope 的 AI review。
未触碰 coverage_pending：不阻断无关变更，但必须保留在定期批量审查队列。
相同 content/context/policy/model/prompt digest 命中缓存：不得重复调用 Provider。
任何静默截断或漏扫都必须记录 partial/not_evaluated，不能记为 passed。
```

初始建账不是自动批准全部旧问题。每条历史 finding 必须经过复核后进入以下状态之一：

```text
accepted_historical_debt
  证据成立，允许在期限内存在，但不得恶化。

remediation_required
  必须在复审日期前拆分或修复。

confirmed_critical
  涉及安全、数据真相、禁止依赖或大范围修改风险；不能作为普通长期债务保留。

rejected_finding
  AI 证据不足或判断错误；记录处置理由，不能直接删除审计轨迹。
```

### 8.2 “触碰”按语义影响范围定义

旧代码触碰式治理不能只判断文件是否出现在 Git diff。以下任一情况都算触碰历史 finding 的 subject：

```text
直接修改 finding 对应 symbol、impl block 或职责代码。
修改其 public API、数据合同、错误语义或生命周期。
新增或改变进入/离开该 subject 的调用边、依赖边或所有权关系。
把新职责加入同一 facade/service/module。
移动、重命名或拆分代码后，语义 fingerprint 仍能追踪到原 subject。
修改测试并实质改变该 subject 的行为覆盖或验收合同。
```

以下情况默认不算语义触碰，但必须由 canonical diff 分类证明：

```text
仅 rustfmt/空白/换行变化。
仅注释拼写且不改变规则或契约。
内容和依赖不变的机械路径移动。
完全不进入该 subject dependency neighborhood 的无关修改。
```

工具无法确定是否触碰时不得静默判定为未触碰；引擎 Merge Gate 必须升级为 Review finding 或人工确认。

### 8.3 旧代码触碰后的决策

对每个被触碰的历史 finding，Gate 比较修改前后的结构证据：

```text
没有新增职责、没有扩大依赖、没有提高风险
  -> 可以通过；原历史债务继续保留，review_by 不自动延期。

修复局部 bug，但无法在本次合理拆完整个热点
  -> 可以通过；必须证明修改局限于原职责，不能扩大 allowance。

减少职责、缩小 public surface、消除依赖或完成真实拆分
  -> 通过；降低或删除对应 baseline，改进结果不可逆回滚。

继续向历史热点增加异质职责、扩大依赖或提高风险
  -> 阻断；不能用“这是旧文件”作为理由。

产生新的 high/critical finding
  -> 按新代码规则立即阻断，不得登记成历史债务。
```

“本次必须顺手清完全部旧问题”不是默认要求。触碰式治理要求本次变更不恶化，并在合理范围内收敛；具体巨型热点继续通过独立拆分施工逐项清债。

### 8.4 无关修改不被历史债务全局阻断

历史 finding 必须绑定稳定的 `scope/path/symbol/dependency neighborhood`。一个未到期的历史 finding 与本次 changed scope 没有交集时：

```text
finding 继续出现在 debt summary 中；
不把无关 Merge 判为失败；
不重置 review_by；
不减少 owner 的治理责任；
定期全量审查仍可发现它自然恶化、路径失效或证据漂移。
```

这样既治理旧代码，又不会因为 `aui.rs` 等现有热点让整个仓库所有领域停止开发。

### 8.5 到期、严重问题与逐步清债

```text
到达 review_by 仍未处理：阻断该 finding 所属 domain/subject 的后续功能增长。
继续修复该 finding 的变更：允许进入专用 remediation 路径，但仍需测试和 AI 复核。
confirmed critical：立即形成明确处置任务；不得通过普通延期长期保留。
已解决 finding：从 active debt 删除并记录 resolved evidence；后续重新出现按新问题阻断。
```

除 changed-scope Merge 审查外，必须提供定期全量审查。全量审查负责发现未被直接修改但因依赖变化而受影响的历史 subject，并输出按风险排序的清债队列；它不把所有未到期债务自动变成全局阻断。

## 9. AI 语义审查

### 9.1 审查输入

AI 不得只查看孤立 class、struct 或单个文件。最小上下文包为：

```text
changed diff
base_commit / head_commit / dirty_patch_digest
目标文件与相邻模块摘要
public API / impl blocks / imports
crate 与 domain 依赖关系
相关架构文档摘录和稳定规则 ID
历史 baseline、未过期 exception、既有 finding
确定性检查证据
```

输入必须有大小预算。超出预算时按 changed symbol 和 dependency neighborhood 分片，并在报告中标记覆盖范围；禁止静默截断后声称完整审查。

`base_commit` 必须是 Merge policy 明确接受的基点；本地脏工作树使用已记录 base commit + canonical patch digest。无法建立可信 base/head 时 fail closed 为 `change_scope_unresolved`。generated/vendor/build output 依据 `architecture-policy.v1` 排除，不能靠临时命令参数跳过。

### 9.2 AI 判断内容

```text
一个 struct/class/module 是否混合多个独立变化原因。
新增代码是否放错 crate、domain 或 service。
拆分是否只有 wrapper/re-export，没有形成真实职责边界。
是否重复实现已有 helper、Projection、Validator、Provider 或 Report。
public API 是否泄漏内部状态或造成错误依赖方向。
测试是否被删除、弱化或与实现过度耦合。
命名、文档和实际职责是否冲突。
后续 AI 是否能从稳定入口定位、修改和验证该职责。
```

### 9.3 结构化 finding

AI 不能只返回自然语言 Pass/Fail。每条 finding 至少包含：

```text
finding_id
subject_kind / subject_path / symbol
issue_type
severity: info | warning | high | critical
confidence
summary
evidence[]: path + symbol/line anchor + observed fact
architecture_rule_ids[]
suggested_destination_or_action
scope_coverage
provider/model/prompt/context digest
```

缺少具体 evidence 或规则依据的 finding 自动降为 `warning`，不能阻断。

### 9.4 Finding 本地证据验证与处置状态机

AI 输出只是 candidate，不能直接把 `passed` 改成 false。Provider 返回后必须先通过本地验证：

```text
strict JSON Schema parse
finding count / response bytes / enum limits
workspace-relative path normalization
subject symbol 与 AST fingerprint 存在
dependency edge 可由 Cargo metadata / structured index 复算
architecture_rule_id 存在且适用于该 subject
evidence context hash 与 exact commit 匹配
```

状态机：

```text
candidate
  -> evidence_verified
  -> review_required
  -> confirmed | rejected | exception | resolved
```

规则：

```text
AI 建议 severity/confidence，不拥有最终批准权。
本地证据无法复算：标记 provider_invalid_evidence，不能作为 high/critical 阻断依据。
verified high/critical：先阻断为 review_required，直到有效 disposition。
rejected 必须保留 reviewer 和 reason，防止无痕删除。
line number 只用于显示，不参与稳定 identity；identity 使用 path + symbol/AST/context fingerprint。
源码注释、字符串和文档中的指令全部按不可信数据处理，不能改写 system policy 或要求工具执行操作。
```

## 10. 可审查阻断合同

### 10.1 引擎侧

```text
info/warning：记录，不阻断。
新 verified high：进入 review_required；必须修复、由授权 reviewer 驳回，或提交有效 exception 后才能合入。
新 verified critical：默认必须修复；仅 architecture-policy 声明的 review authority 批准限时 exception 才可暂时放行。
未到期且未被触碰的 historical high：保留 debt，不阻断无关变更。
被触碰的 historical high：必须证明不恶化；新增职责或风险提高则阻断。
到期 historical high：阻断所属 subject/domain 的继续增长，修复路径仍可进入。
confirmed critical：不能降为普通历史债务，必须立即进入处置流程。
同一 finding 重跑时通过稳定 subject/evidence fingerprint 对齐。
AI 不能自行批准 exception、修改 baseline 或删除历史 finding。
```

`approved_by` 必须匹配 `architecture-policy.v1` 中 subject/domain 对应 authority。普通 domain owner 可以处置 high；critical exception 至少需要该 domain authority 和 engine maintainer 的显式批准。单人仓库也必须保留两个逻辑角色和理由字段，不能使用未定义的“安全负责人”。

### 10.2 项目侧

```text
所有 AI finding 默认 advisory。
报告明确写出 recommended action，但 passed 不因 AI finding 变为 false。
只有项目显式选择 ProjectStrict 后，才使用该项目自己的阻断 policy。
引擎升级不得偷偷把已有项目切换为 ProjectStrict。
```

### 10.3 Provider 不可用

```text
Fast：不需要 Provider。
Review：记录 not_evaluated，可以继续开发。
EngineStrict Merge：不得把 not_evaluated 冒充 passed；必须取得有效结果或走显式人工应急处置。
ProjectAdvisory：记录 not_evaluated，不阻断 Build/Play/Export。
```

应急处置必须记录 reviewer、原因、影响范围和到期时间；不能成为永久跳过 AI 的开关。

### 10.4 项目侧外部 Provider 授权

`ProjectAdvisory` 表示结果不阻断，不代表允许默认外发项目源码：

```text
外部 Provider 默认 Off。
用户必须显式同意 Provider、发送范围和数据类别。
调用前可查看 scope manifest；未授权文件不得进入 context。
授权记录绑定项目、Provider 和 policy version，可撤销、可过期。
撤销后停止新请求，不影响 Build/Play/Export。
本地离线 Provider 使用独立授权策略，但仍受路径范围和报告脱敏约束。
```

引擎自身源码也只能在受信 Review 环境按 repository policy 发送；不得把 fork/untrusted PR 中的任意路径扩入上下文。

## 11. Provider 与安全边界

246 复用现有 LLM transport/controller 的生命周期、取消、限流和 credential 原则，但 `quality_gate` 不得反向依赖 `editor_core`。新增的是面向治理的窄 Adapter / Artifact interface，例如：

```rust
pub trait ArchitectureReviewProvider {
    fn review(&self, request: ArchitectureReviewRequest)
        -> ArchitectureReviewOutcome;
}

pub trait ArchitectureReviewArtifactVerifier {
    fn verify(&self, artifact: &ArchitectureReviewArtifact,
              expected: &ArchitectureReviewExpectation)
        -> ArchitectureReviewVerification;
}
```

实现边界：真实 Provider Adapter 可以复用或下沉通用 transport；`QualityGateRunner` 只读取并验证 artifact，不链接 EditorSession、AI Panel 或项目 Patch service。不得形成 `quality_gate -> editor_core` 依赖。

规则：

```text
Provider 只读审查上下文，不能直接写代码或修改 ledger。
源码注释、字符串和文档全部视为不可信数据，不能覆盖系统审查规则。
不发送 secret、credential、绝对用户路径和未授权文件。
真实 Provider 不进入默认 deterministic 单元测试。
测试使用 scripted provider 和固定 fixture。
模型、prompt、schema 和 context 都要带 digest，保证报告可追踪。
请求设置 token/byte/cost/timeout/rate budget；超预算明确失败或分批，不静默缩减覆盖。
artifact 缓存 key 至少包含 commit/content/policy/model/prompt/schema/context digest。
Trace 不保存完整源码、原始 Provider response 或 credential；只保存脱敏证据摘要和 digest。
```

## 12. Report 与现有 Quality Gate 集成

246 不新增平行 Quality Gate 系统。它扩展 245 已有 `QualityGateRunner`，但由于新增 blocking semantics 和字段，正式报告升级为 `quality-gate-report.v2`：

```text
code_hygiene.report.v2
  -> deterministic observations / baseline reconciliation

architecture-review-report.v1
  -> AI findings / coverage / provider evidence / dispositions

QualityGateRunner
  -> 根据 EngineStrict 或 ProjectAdvisory 汇总最终 decision

quality-gate-report.v2
  -> hygiene + architecture_review summary
```

兼容规则：

```text
保留 quality-gate-report.v1 reader，用于读取 245 历史 artifact。
246 新 runner 只写 v2，不把新增必填字段伪装成 v1。
CI summary/Report Panel 根据 schema_version 分派 reader。
v2 未知必填语义 fail closed；未知可选 display 字段可忽略。
```

Report Panel 只消费 Summary/Trace 产物，不形成第二套治理真相。正式 Runtime 默认不生成这些报告。

## 13. Baseline 与 Exception

baseline 和 exception 都必须版本控制、人工审查，不允许工具原地自动放宽。

246 需要的仓库治理资产为：

```text
rust/quality/architecture-policy.v1.toml
rust/quality/architecture-debt-ledger.v1.json
rust/quality/architecture-review-coverage.v1.json
target/quality-gate/architecture-review-artifact.v1.json（生成物，不作为可手改真相）
```

历史债务 ledger 除通用 exception 字段外，还必须记录：

```text
finding_fingerprint
subject_fingerprint
baseline_evidence_digest
accepted_severity
disposition
dependency_neighborhood
introduced_or_observed_at
last_reviewed_at
resolved_evidence（仅 resolved 记录）
```

例外至少包含：

```text
id
scope/path/symbol
reason
owner
review_by
allowed_growth_or_finding_ids
decomposition_or_remediation_plan
approved_by
```

以下情况失败：

```text
exception 过期。
路径或 symbol 已消失但记录未删除。
实际增长超过 allowance。
finding evidence 已变化却沿用旧批准。
没有 owner、reason、review_by 或处置计划。
把已改善的新基线重新扩大到历史值。
重命名或移动代码后通过路径变化逃掉原 finding。
把新 finding 伪装成初始历史债务。
触碰历史 subject 后没有生成 before/after 证据。
```

## 14. 施工边界

246 后续施工只建立防回归合同，不在同一施工文档内拆完所有热点：

```text
升级 code_hygiene schema 和 baseline/exception reconciliation。
增加 EngineStrict / ProjectAdvisory profile。
建立 changed scope 和结构证据包。
建立 architecture-policy.v1、作用域清单、domain/owner/dependency/facade 规则。
建立旧代码全量 inventory、historical debt ledger 和初始人工复核流程。
建立 symbol/dependency-neighborhood 触碰判定与 rename/move fingerprint 追踪。
建立 trusted review artifact 生成与本地 evidence/provenance verifier。
建立项目侧 external Provider 显式授权和 scope preview 合同。
建立 AI review request/outcome schema、缓存/成本预算与 scripted provider tests。
接入现有 QualityGateRunner/report。
升级并兼容读取 quality-gate-report.v2/v1。
建立 blocking/advisory negative matrix。
```

`aui.rs`、`runtime_player_winit/src/lib.rs`、`report_panel.rs` 等具体热点按风险另开小型拆分方案和施工文档，不能借 CQ-07 一次性迁移整个 workspace。

## 15. 不做什么

```text
不重开或修改已归档的 245/CQ-08。
不新增 Runtime 架构层。
不让 LLM 进入游戏帧循环。
不对用户项目默认强制 EngineStrict。
不把固定行数作为唯一质量真相。
不让 AI 无证据直接 Pass/Fail。
不让 AI 自动修改 baseline、exception 或源代码。
不让 quality_gate 依赖 editor_core。
不在无授权情况下把项目源码发送到外部 Provider。
不把真实 Provider credential 注入 fork/untrusted PR workflow。
不为了降低行数制造假模块。
不在本方案中一次拆完全部历史热点。
```

## 16. 验收合同

后续施工至少证明：

```text
EngineStrict：确定性结构违规会阻断。
EngineStrict：未处置 high/critical AI finding 会阻断。
EngineStrict：无 evidence 的高严重度 finding 会降级且给出诊断。
EngineStrict：Provider 未评估不能冒充 Merge passed。
EngineStrict：public CI 无 secret，Merge 通过受信 artifact 完成 AI evidence。
EngineStrict：artifact 的 commit/policy/model/prompt/context digest 不匹配会阻断。
EngineStrict：AI 引用不存在的 path/symbol/dependency/rule 不能成为阻断 finding。
EngineStrict：architecture-policy 放宽会显示影响并要求授权审批。
EngineStrict：未触碰、未到期的历史债务不会阻断无关 domain 变更。
EngineStrict：触碰历史 subject 但不恶化的局部修复可以通过且不会延期债务。
EngineStrict：向历史热点新增异质职责或扩大依赖会阻断。
EngineStrict：历史 subject 的 rename/move 不能逃避 finding。
EngineStrict：到期债务阻断所属 subject/domain 的功能增长，但允许专用修复变更。
EngineStrict：已解决债务不能被 baseline 自动放宽后重新引入。
ProjectAdvisory：相同 finding 只建议，不阻断 Build/Play/Export。
ProjectAdvisory：未授权外部 Provider 时不发送项目源码，且不影响 Build/Play/Export。
ProjectStrict：只有显式启用后才应用项目自己的阻断 policy。
exception 的新增、过期、失效、超预算和旧 evidence 复用均有 negative tests。
scripted provider 下报告完全可复现。
真实 Provider 不进入默认 workspace 回归。
quality-gate-report.v1 历史 artifact 可读取，246 新产物使用 v2。
QualityGateRunner 仍是唯一入口，没有平行 CI/Report 系统。
Runtime/Player 依赖图和性能不受影响。
```

模型和 prompt 不能只测管线。必须建立固定 architecture review eval corpus，至少覆盖：

```text
wrong-domain dependency
mixed responsibilities
fake wrapper split
cohesive large module（负例，防止机械误报）
rename/move finding continuity
prompt injection in source/comment/string
hallucinated symbol/rule/dependency evidence
partial context / timeout / rate limit / budget exceeded
```

Provider/model/prompt/policy 升级前必须运行同一 eval corpus，报告 precision-oriented blocking regression、finding stability 和 coverage。真实 Provider eval 可以是受信、显式运行的 release Gate；默认单元测试继续使用 scripted/recorded fixtures。

## 17. 方案自审

### 17.1 是否仍然简单粗暴地按行数治理

否。行数只负责触发风险审查；依赖、domain、职责、diff 和 exception 共同决定结果，AI 补充语义判断。

### 17.2 AI 是否可以凭感觉阻断

不可以。阻断 finding 必须提供代码 evidence、规则 ID、覆盖范围和可执行处置；缺失证据自动降级。

### 17.3 是否错误治理用户项目

没有。引擎使用 `EngineStrict`；项目默认 `ProjectAdvisory`，只有用户显式选择才进入 `ProjectStrict`。

### 17.4 是否增加运行时开销或架构层

没有。所有检查都在工程治理路径，复用现有 QualityGateRunner、Report 和 LLM transport，不进入 Runtime。

### 17.5 是否干扰 245

不干扰。245 已完成并归档；246 已完成独立治理施工，也不重开 245。

### 17.6 是否符合 AI-first 优先级

符合。确定性证据保证可验证，AI 语义判断避免机械规则，结构化 finding 让 AI 和用户都能定位、修复和复审。

### 17.7 是否真正治理 246 之前的旧引擎代码

是。首次启用时全量建账；未触碰旧债不会阻断无关开发，被触碰 subject 必须提供 before/after 证据并禁止恶化，到期债务按 subject/domain 阻断继续增长，已解决债务不可重新放宽。

### 17.8 AI Merge Gate 是否有可执行载体

是。public CI 保持无 secret；受信 Review 生成绑定 exact commit 和全部配置 digest 的 artifact；QualityGateRunner 只验证 artifact 和 disposition，不反向依赖 editor_core。

### 17.9 AI 是否可能用幻觉证据随机阻断

已控制。AI finding 先经过 strict schema 和本地 path/symbol/dependency/rule 复算，再进入 review_required；AI 只建议严重度，授权 reviewer 决定 disposition。

### 17.10 项目侧是否可能默认外发源码

不会。ProjectAdvisory 外部 Provider 默认 Off，必须显式授权 Provider、路径范围和数据类别；没有授权不阻断项目工作流。

### 17.11 报告版本是否诚实

是。245 历史产物继续按 v1 读取；246 新增必填语义写入 `quality-gate-report.v2`，不在 v1 名称下静默改变合同。

## 18. 结论

CQ-07 v2 正式采用：

```text
方案 B：AI 语义审查 + 可审查阻断

EngineStrict
  = 确定性结构 Gate
  + trusted AI semantic review artifact
  + local evidence verification
  + high/critical disposition contract

ProjectAdvisory
  = 同一证据与建议
  + 默认不阻断项目 Build/Play/Export
```

246 已完成。最终 exact commit `3d6db9a7c859aea6c95c0b32f24f6b42c4ad5911` 取得真实 `gpt-5.5` trusted Provider artifact；首批按风险审查 `aui.rs` 与 `asset_browser.rs`，outcome=complete、findings=0。EngineStrict 12/12 stages passed、artifact verified、diagnostics=0；authoritative Local CI `local-3d6db9a7c859-1783860976` passed，source/isolated clean、cleanup removed，quality report digest 为 `sha256:3e222c30d28f0d47e7ca7707192cf1a2d23078912b52da8647857f6911f4e031`。CQ-07 已关闭；未覆盖旧代码继续以 committed `coverage_pending` 表达，不伪装成全量 AI reviewed。
