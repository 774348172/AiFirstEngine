# 310 Project Game SDK + Generated Runtime Glue v1 施工文档

> 文档性质：已确认方案 C 的已完成施工文档。  
> 当前状态：Gate A-D 已通过；已归档到 `已完成/`。  
> 正式方案：`../../310-Project-Game-SDK-Generated-Runtime-Glue-v1方案.md`。  
> 正式方案 SHA-256：`CE9CB3CA8677E47B67F3E2A097C63A761DD17E7C3DEBBD6245AD5E6F19B68505`。  
> 前置状态：309-A 至 309-F 已完成并归档；当前施工槽为空；Tower P1-2 为 `stopped_by_user`。  
> 生成基线：branch `codex/255-capability-catalog`，HEAD `acaffa8cb95bbb54d093bbf45835a59315c82da5`；工作区已有大量用户变更，后续不得重置或覆盖。  
> 生成日期：2026-09-02。  
> 施工授权：2026-09-02 用户已授权按本文施工 Gate A-D；仍禁止 production、真实 Codex 配置、Android 工具链、Local CI 和 Tower P1-2。

## 1. 施工目标与允许声明

本文只完成一个替换结果：让 AI / 项目作者只面对一个普通 Rust `Project Game SDK` Interface，
由现有 `GameProjectCompiler` 在内部确定性生成 `ProjectRuntimeAbi` Adapter，并让 Headless、Optional Editor、
Windows Player 与 Android source composition 消费同一项目逻辑产物。

全部 Gate 通过后最多允许声明：

```text
项目 RuntimeModule 只直接依赖 Project Game SDK，不再直接依赖 project_runtime_abi/project_runtime_sdk；
每个项目只有一个 typed Rust registration，handler presence 是 capability truth；
ABI entry、descriptor、session handle、FFI、capability bits、AOT digest 和平台注册由 Compiler 派生；
Generated Runtime Glue 位于 Compiler-owned derived root，不进入 canonical project truth；
现有 engine_project_run/build 在需要时自动生成和编译 Glue，不要求 AI 调用 generate_glue；
Headless、Optional Editor、Windows 与 Android source composition 不再维护四套项目注册逻辑；
Complex Shooter、Switch Puzzle、Tower Defense 已删除手工 ABI glue 并保持已覆盖的玩法合同。
```

不得宣称完整 R3 Compiler、完整 AI Development Pack、Semantic Outcome/Acceptance、新 gameplay 能力、
全平台发布、真实 Android APK/设备、production 安装态或“已经优于 Unity/UE/Godot”已完成。

## 2. 必要性与验证经济性审计

```text
requested claim：让AI像写普通代码一样只使用Project Game SDK，Engine在既有run/build内部自动完成底层ABI接线
confirmed gap：三个样例各自手写ProjectRuntimeApi、extern callbacks、descriptor、session store、capabilities与AOT source list；Player/Editor/Android仍直接寻找项目ABI entry
minimum causal change：新增一个项目侧SDK；在现有Compiler内增加私有确定性Glue生成；回接现有四类consumer；迁移三个真实样例并删除旧手工路径
必须修改：workspace与新SDK、Compiler生成/缓存/诊断和staging、Player/Editor/Android source composition、三个样例RuntimeModule与直接受影响fixture/tests
明确排除：第二Compiler、独立Glue crate/Host/Provider/Registry/daemon、公开generate工具、完整R3、完整AI知识包、新引擎能力、production与真实外部环境
复用证据：309-A/B lease-owned SourceView与revision；309-F Compiler/Assembler/Player/export唯一owner；现有ABI安全、native loader、三个样例behavior tests
证据失效条件：正式310方案或ProjectRuntimeAbi合同改变；GameProjectCompiler/consumer调用图漂移；相关样例玩法代码改变；测试fixture或toolchain identity改变
预计成本：Gate A 0.5-1日，Gate B 1-2日，Gate C 1-2日，Gate D 1-2日；总计约4-7个工程日，不含冷编译等待
总停止条件：需要重写RuntimePackage/export/runtime、创建第二生命周期owner、长期双pipeline、扩展ABI能力、修改真实配置/production/Android工具链或恢复Tower P1-2时立即停止复核
```

| Gate | 风险 / 失败合同 | 最便宜可打红测试 | 终态证据 | 预计成本与停止点 |
| --- | --- | --- | --- | --- |
| A | SDK 只是浅 facade，或向项目泄漏 ABI/Engine/Editor | SDK owner compile tests + dependency/static guard | typed registration、缺省 capability、safe context tests 全绿 | 0.5-1 日；若必须新增 proc-macro/Host，停止 |
| B | Glue 非确定、读取 live root、形成第二 owner，或错误只指向 generated file | generator owner golden/identity/source-map tests + 一个最小 fixture compile/load | 相同输入同 digest，删除 cache 可重建，ABI safety 保持 | 1-2 日；若需独立 Glue crate/daemon，停止 |
| C | Player、Editor、Windows、Android 继续各自接旧 entry | 既有 consumer source-composition tests | 四类 consumer 都链接同一 generated Adapter identity | 1-2 日；若需重写导出/平台 pipeline，停止 |
| D | 样例迁移损失玩法，或旧手工 ABI 路径残留 | 三样例现有窄 behavior tests + static retirement guard + 一条既有 Headless tool path | 三样例 compile/behavior 通过，旧实现与重复测试删除 | 1-2 日；若暴露新玩法能力缺口，记录后 deferred |

验证经济规则：后一个命令若在相同输入、profile 和 feature 下完整包含前一个结果，不重复运行前一个；
只有实际修改会使所消费 identity 失效的证据才重跑。任何 Gate 实际成本超过估计上界或单次授权接近三小时时，
先保存首因与已完成证据并停止，不通过新增抽象、Runner 或更宽测试掩盖成本。

## 3. 冻结实现形状

### 3.1 只新增一个项目侧 Module

允许新增一个 workspace crate：

```text
project_game_sdk
  - ProjectGameDefinition<S>
  - ProjectGameSession / typed handler contracts
  - GameTime / InputActionView / CollisionView
  - WorldRead / DeferredMutationWriter
  - UiBindingRequest / UiStateWriter / ObservationWriter
  - ProjectDiagnosticSink / EntityHandle
  - versioned machine-readable symbol metadata
```

它可以依赖并窄 re-export `project_runtime_sdk` 中已经稳定的安全数据类型，但不得公开
`project_runtime_abi` 的 pointer、buffer、function table、host callback 或 raw handle；不得依赖
`engine_*`、`editor_*`、`project_authoring_execution` 或 `engine_tool_provider`。

v1 不新增 proc-macro crate。普通 Rust builder/trait 已足以提供类型检查和可搜索性；以后只有出现真实重复且
不隐藏 owner 时，才能另案讨论局部 derive/attribute。

### 3.2 冻结项目 registration 约定

项目 crate root 提供且只提供一个普通 Rust entry：

```rust
pub fn project_game() -> project_game_sdk::ProjectGameDefinition<GameSession> {
    project_game_sdk::ProjectGameDefinition::new(GameSession::create)
        .fixed_update(GameSession::fixed_update)
        .aui_actions(GameSession::handle_aui_actions)
        .ui_state(GameSession::resolve_ui_state)
        .observe(GameSession::observe)
        .rule("rule.player-move", GameSession::player_move)
}
```

上述冻结公开形状与 owner，不强制每个 optional handler 出现。未注册 handler 必须由 SDK/生成 Adapter 表现为
capability absent 或规范 unhandled；项目不能手写 no-op ABI callback。

项目 RuntimeModule 变为普通 Rust gameplay library。最终 `cdylib`、static-link、Player/Android entry 由
Compiler-owned generated wrapper 拥有，不能要求项目 Cargo manifest 继续把 ABI entry 当作项目职责。

### 3.3 Glue 只是 Compiler 私有 Adapter

在 `project_authoring_execution` 内新增私有生成实现，不从 crate root 暴露第二套 prepare/build Interface：

```text
GameProjectCompiler::prepare
  -> 从operation-bound lease取得SourceView
  -> 解析manifest中的module/build/SDK requirement
  -> 计算project source + SDK + ABI + generator + target identity
  -> cache hit校验或生成wrapper source/manifest/source map/report
  -> 编译project gameplay library + generated Adapter
  -> 继续既有RuntimePackageBuildInput / Player / build owner
```

生成 wrapper 才允许直接依赖 `project_runtime_abi`、`project_runtime_sdk` 和项目 gameplay crate。它负责：

```text
ProjectRuntimeApi与aife_project_runtime_entry_v1
descriptor/capability派生
session handle create/destroy/generation validation
typed request decode/response encode
panic/status/small-buffer safety
WorldRead与deferred mutation Adapter
AOT digest constant与static/native/platform registration
```

生成目录、临时绝对路径、时间戳、Editor session 不得进入 normalized content 或 digest。
generated bytes 不进入 canonical revision、mutation receipt 或 Editor Save。

### 3.4 不改变现有 AI 工具面

本施工不新增 `generate_glue`、`register_module`、Catalog、Agent Planner 或固定工具菜谱。
既有 `engine_project_run` / `engine_project_build` 通过 Provider -> `GameProjectCompiler` 自动触发生成。
AI 继续用普通文件工具修改项目 Rust、Scene、AUI、Rule 与测试。

## 4. 允许文件与最小改动面

### 4.1 允许新增

```text
rust/crates/project_game_sdk/Cargo.toml
rust/crates/project_game_sdk/src/**
rust/crates/project_game_sdk/tests/**（仅owner/contract确有需要时）
rust/crates/project_authoring_execution/src/generated_runtime_glue.rs（名称可在激活复核时按现有目录风格调整一次）
rust/crates/project_authoring_execution/tests/**（仅现有内联测试不能表达跨crate fixture时）
```

不得新增独立 generated-glue crate 到 workspace。Compiler 可以在 derived/staging root 生成临时 Cargo package，
但该产物不是仓库 Module、canonical source 或长期 checked-in crate。

### 4.2 允许修改

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/project_authoring_execution/Cargo.toml
rust/crates/project_authoring_execution/src/lib.rs（只接私有module或必要类型re-export）
rust/crates/project_authoring_execution/src/project_schema.rs
rust/crates/project_authoring_execution/src/game_project_compiler.rs
rust/crates/project_authoring_execution/src/project_runtime_player_staging.rs
rust/crates/project_authoring_execution/src/project_player_artifact.rs
rust/crates/project_authoring_execution/src/desktop_export.rs（仅consumer identity需要）
rust/crates/editor_core/Cargo.toml
rust/crates/editor_core/src/project_runtime_native_module.rs
rust/crates/editor_core/src/project_runtime_preparation.rs（仅identity/Adapter回接）
rust/crates/editor_core/src/project_editor_composition_artifact.rs
rust/crates/editor_core/src/android_export.rs（仅source composition，不运行工具链）
rust/crates/editor_core/src/lib.rs（仅re-export调整）
rust/crates/editor_window_winit/Cargo.toml（仅production Editor consumer回接Compiler产物）
rust/crates/editor_window_winit/src/project_editor_composition_production.rs（仅native module preparation消费generated glue）
rust/crates/editor_window_winit/src/project_editor_composition_qualification.rs（仅旧composition fixture补显式无glue语义）
rust/crates/project_e2e_gate/**（仅现有consumer/retirement测试迁移）
rust/crates/runtime_player_winit/Cargo.toml（仅删除旧样例runtime dev依赖）
rust/crates/runtime_player_winit/src/lib.rs（仅删除或改写直接消费旧样例ABI的重复测试fixture）
rust/project_modules/complex_shooter/**（仅退役旧手工sample ABI wrapper）
rust/project_modules/switch_puzzle/**（仅退役旧手工sample ABI wrapper）
rust/project_players/complex_shooter/**（仅退役旧静态sample Player pipeline）
rust/project_players/switch_puzzle/**（仅退役旧静态sample Player pipeline）
rust/fixtures/project_runtime_native_module_minimal/**（迁移现有唯一compile/load fixture，不新增第二fixture）
samples/complex_shooter_project/project.aife.json
samples/complex_shooter_project/RuntimeModule/Cargo.toml
samples/complex_shooter_project/RuntimeModule/Cargo.lock
samples/complex_shooter_project/RuntimeModule/src/**
samples/switch_puzzle_project/project.aife.json
samples/switch_puzzle_project/RuntimeModule/Cargo.toml
samples/switch_puzzle_project/RuntimeModule/Cargo.lock
samples/switch_puzzle_project/RuntimeModule/src/**
samples/tower_defense_project/project.aife.json
samples/tower_defense_project/RuntimeModule/Cargo.toml
samples/tower_defense_project/RuntimeModule/Cargo.lock
samples/tower_defense_project/RuntimeModule/src/**
samples/tower_defense_project/Tests/PlayableInterface/**
直接消费runtimeModule manifest合同的既有test fixtures（只补SDK requirement或迁移旧fixture）
README.md与框架设计入口（只在完成时同步真实状态）
```

实际施工只能修改由打红测试证明必要的上述文件。清单不是批量重构授权；若某文件无需变化，应保持不动。

### 4.3 明确禁止

```text
不新增第二Compiler、Glue Host、Glue Provider、Registry、daemon、进程或cache owner
不新增公开engine_* Tool，不修改Agent策略或Codex Skill
不重写ProjectRuntimeAbi、RuntimePackage、Renderer、ECS、AUI、physics、audio、animation或export pipeline
不扩展现有ABI capability；发现缺口只记录后续方案
不建立长期legacy/manual-glue兼容pipeline
不迁移实验目录、历史repair副本或无关fixture
不运行Local CI、完整workspace矩阵、真实Editor窗口、真实Codex任务、production替换
不运行Gradle/NDK/APK/ADB/Emulator/设备测试，不修改Android工具链
不恢复或修改Tower P1-2；Tower样例只迁移RuntimeModule最外层glue并运行既有窄行为测试
```

## 5. Gate A：Project Game SDK Interface

### 5.1 施工任务

1. 激活前先确认本文是 `当前/` 唯一非 README 施工文档，并重算正式方案 hash。
2. 重新扫描现有 ABI/SDK 和三个样例使用面，冻结 v1 真正使用的 handler/context 最小集合。
3. 新增 `project_game_sdk`，实现 typed definition、session/handler、安全 context 和缺省 capability。
4. 提供机器可读但不另建 Tool 的最小 symbol metadata：contract identity、公开 symbol index、handler/context 摘要、最小 registration 示例和限制。
5. 增加依赖静态断言：SDK 不依赖 Engine/Editor/Provider，项目面对的公开 surface 不包含 raw ABI 类型。

### 5.2 风险与测试

必须先有能打红的 owner tests：

- 注册 handler 后 capability 可由 definition 唯一派生；未注册 handler 缺省 absent。
- duplicate stable rule id、无效 registration 或越界 handle 在类型/校验 seam 失败。
- safe context 只提供 WorldRead 和 deferred mutation，不暴露裸 world/entity index/host pointer。
- symbol metadata 由同一公开 Interface 派生，不复制第二份 capability truth。
- Cargo dependency/static scan 拒绝 Engine、Editor 和直接 `project_runtime_abi` 泄漏。

终态命令（在 `G:\gameEngin\rust`）：

```powershell
cargo fmt --all -- --check
cargo test -p project_game_sdk --locked
cargo check -p project_game_sdk --locked
```

若 `cargo fmt --all -- --check` 因无关用户文件失败，只记录无关路径并对本 Gate 修改的 Rust 文件运行
`rustfmt --check`；不得格式化或修复无关文件。

### 5.3 Gate A 完成条件

- 只有一个项目侧 SDK Interface；没有 proc-macro/Host/Provider/第二 registry。
- registration 普通、可搜索、可类型检查。
- 公开依赖方向和 safe context 通过 owner tests。
- 不进入样例批量迁移，不修改 consumer pipeline。

## 6. Gate B：Compiler-Owned Generated Runtime Glue

### 6.1 施工任务

1. 在 `GameProjectCompiler` 内接入一个私有 generator；不得新增公开 `generate` 方法。
2. 从 lease-owned `SourceView` 读取 manifest、Cargo source 和 registration约定，不得重新打开 live project root。
3. 在现有 project manifest runtimeModule 中加入 Project Game SDK requirement；不新增第二 manifest/schema 文件，不保存 handler path、capability bit 或 generated digest。
4. 用 canonical source identity、SDK/ABI/generator/target/toolchain必要身份计算 generation key。
5. 生成 wrapper source、Cargo manifest、source map 和 Summary/Trace report；普通路径不常驻长 Trace。
6. 让 Compiler-owned staging能够物化 generated wrapper，并迁移既有
   `project_runtime_native_module_minimal` fixture实际compile/load ABI entry；生产Player/Editor/Android切换留到Gate C。
7. 保持现有 ABI panic containment、invalid handle、small-buffer 和 contract digest fail-closed合同。

不得用正则或字符串猜 Rust 语义。v1 通过固定 crate-root `project_game()` convention 建立 binding；项目 Rust
编译错误保留 rustc 原始 project source span，generator synthetic 错误明确标记 Engine defect。若必须理解更多
Rust 结构，先停止复核，不能在本 Gate 偷建 schema language 或 source parser。

### 6.2 风险与测试

新增/迁移以下最小 owner tests：

- 同一 normalized SourceView + contracts + target 生成 byte-identical content 与 digest。
- 输出目录、mtime、时间戳、Editor state 改变不改变 generation identity。
- registration/source/SDK/ABI/generator/target 的真实变化只失效相应 Glue 和下游。
- Scene/AUI/Asset 变化且 RuntimeModule source identity 不变时不重生成 Rust Glue。
- cache 删除后从相同输入精确重建；generated bytes 不进入 canonical project revision。
- refresh 到 R2 后，持有 R1 lease 的 operation 仍只生成 R1 Adapter。
- source map覆盖每个generated callback/registration item；synthetic defect不要求AI编辑generated file。
- 最小 fixture 编译并加载 entry，保留现有 invalid handle/panic/small-buffer safety。

终态命令（在 `G:\gameEngin\rust`）：

```powershell
cargo test -p project_authoring_execution --locked generated_runtime_glue
cargo test -p project_authoring_execution --locked project_runtime_player_staging
cargo check -p project_authoring_execution --locked
```

过滤命令必须实际执行非零数量测试；否则先修正测试名，不能把 `0 tests` 当作通过。最小 fixture compile/load
由上述 owner test 自己使用隔离临时目录；不新增通用施工 Runner。

### 6.3 Gate B 完成条件

- generator 是 Compiler Implementation，不是新 Module Interface 或 lifecycle owner。
- `prepare` 的既有高层 Interface保持，generation lineage 进入其既有 prepared result/diagnostics。
- 一个真实最小项目已证明 generated wrapper 可编译并符合当前 ABI安全合同。
- 未切换生产Player/Editor/Android consumer，未触碰Android工具链、production或三个样例玩法实现。

## 7. Gate C：现有 Consumer 统一回接

### 7.1 施工任务

按现有 owner 逐项替换 source composition，不改行为：

```text
Headless/Windows Player -> generated wrapper entry
Optional Editor native module/composition -> 同一generation identity的wrapper artifact
Android launcher source -> 同一generated wrapper target composition
Desktop export -> 继续既有Player/export owner，只传递新artifact identity
```

允许保留 ABI loader 作为 Runtime host seam；要删除的是“项目手写 ABI”和“consumer各自拼项目 entry”，
不是删除底层稳定 ABI。本 Gate 不重写动态库加载、Player packaging、Desktop export、Android Gradle/NDK。

### 7.2 风险与测试

- Headless、Editor、Windows 和 Android source-composition tests 都断言消费同一 SDK contract、project source identity 与 generation digest。
- Editor cache key继续区分 ABI/SDK/generator/project source/target，Editor-only UI变化不失效项目runtime artifact。
- Player template和Android launcher不再直接假设canonical project crate自己定义ABI entry。
- 既有 loader contract digest、symbol、panic与session destroy安全测试保留。
- dependency/static test证明没有第二个 generator、项目logic registry或manual descriptor owner。

终态命令（在 `G:\gameEngin\rust`）：

```powershell
cargo test -p project_authoring_execution --locked project_player_artifact
cargo test -p editor_core --locked project_runtime_native_module
cargo test -p editor_core --locked project_editor_composition_artifact
cargo test -p editor_core --locked android_export
cargo check -p editor_core --locked
```

这些是 source/integration composition 证据，不授权真实 Editor、Windows delivery、Gradle、APK 或设备执行。

### 7.3 Gate C 完成条件

- 四类 consumer只保留平台 launcher/transport差异，不拥有项目玩法或descriptor分支。
- Optional Editor与No-Editor路径消费同一 Compiler产物身份。
- 没有为了兼容旧样例保留长期第二 pipeline；样例迁移在 Gate D 同一施工范围内完成后删除临时 Adapter。

## 8. Gate D：三样例迁移、旧路径退役与最小 Agent 回归

### 8.1 施工任务

1. 依次迁移 Complex Shooter、Switch Puzzle、Tower Defense 的 Cargo依赖和crate-root `project_game()`。
2. 保留各自 gameplay session、rule、WorldRead、deferred mutation、AUI、UI state 与 observation Implementation。
3. 删除手工 `ProjectRuntimeApi`、extern callbacks、session store、descriptor、capability bits和AOT source list。
4. 将直接调用旧ABI entry的样例测试改为通过typed definition或既有Compiler/Provider seam验证同一行为。
5. 删除过渡 Adapter与重复测试，只剩Project Game SDK -> Compiler generated Adapter一条正式pipeline。
6. 复用既有 `engine_tool_provider` run/build owner test中的一个项目fixture，证明AI可见
   `engine_project_run` 或 `engine_project_build` 会自动经过generation；不得新增真实Codex任务或第二工具流程。
7. 从 workspace、`project_e2e_gate` 与 `runtime_player_winit` 测试依赖中退役旧静态 sample Module/Player；
   consumer需要真实项目runtime时，统一经 `GameProjectCompiler` 生成并加载，纯Player边界测试改用项目无关fixture。

Tower只改 RuntimeModule最外层胶水和直接测试，禁止修改英雄、玩法数据、AUI设计或恢复P1-2。

### 8.2 风险与测试

静态 retirement 必须对三个样例同时证明：

```text
无直接 project_runtime_abi 依赖
无直接 project_runtime_sdk 依赖（安全类型只经project_game_sdk）
无 ProjectRuntimeApi 定义
无 aife_project_runtime_entry_v1 定义
无项目手写 extern ABI callbacks / raw buffers / pointer glue
无手工 descriptor/capability bit/AOT source enumeration
每个项目只有一个 project_game typed registration
```

终态命令：

```powershell
Set-Location G:\gameEngin
rg -n -g "*.rs" -g "Cargo.toml" "project_runtime_abi|project_runtime_sdk|ProjectRuntimeApi|aife_project_runtime_entry_v1|extern \"C\"" samples/complex_shooter_project/RuntimeModule samples/switch_puzzle_project/RuntimeModule samples/tower_defense_project/RuntimeModule

Set-Location G:\gameEngin\samples\complex_shooter_project\RuntimeModule
cargo test --locked

Set-Location G:\gameEngin\samples\switch_puzzle_project\RuntimeModule
cargo test --locked

Set-Location G:\gameEngin\samples\tower_defense_project\RuntimeModule
cargo test --locked

Set-Location G:\gameEngin\samples\tower_defense_project\Tests\PlayableInterface
cargo test --locked

Set-Location G:\gameEngin\rust
cargo test -p engine_tool_provider --locked project_game_sdk_generated_glue
cargo test -p project_e2e_gate --locked project_runtime_native_module_consumers
cargo check --workspace --locked
```

第一条 `rg` 的预期结果为零；若保留文字仅出现在明确的 negative/static test fixture 中，测试必须限定fixture路径，
不能让项目生产源码出现旧符号。`cargo check --workspace` 是共享 Rust contract 改动后的唯一宽编译闭包；
不追加 `cargo test --workspace`、all-features、Local CI 或真实平台矩阵。

如三个样例 unit/behavior tests与最小generated fixture仍不能证明实际Compiler composition，才允许只补一次现有
Headless `engine_project_run` 测试；不得顺带build delivery、真实MCP、Editor窗口或多样例E2E矩阵。

### 8.3 Gate D 完成条件

- 三个样例的已覆盖 gameplay结果未改变。
- 旧手工 ABI pipeline、临时兼容 Adapter和重复测试已删除。
- 一个既有AI可见 engine tool consumer已证明generation自动发生，结果仍返回revision/artifact/evidence。
- `cargo check --workspace --locked`通过，且未把这个结果扩大解释为production或平台发布资格。

## 9. Gate顺序、失败与证据复用

必须按 A -> B -> C -> D 顺序施工。当前 Gate失败时先保存首个可复现断言、stdout/stderr、相关source/test identity，
只在该owner内修复；不得跨Gate堆积临时兼容代码。

```text
Gate A变化会使B-D中消费SDK contract的证据失效。
Gate B变化只重跑generator/ABI fixture与真实消费其artifact的C-D证据。
Gate C仅Adapter/source-composition变化，不重跑未消费这些文件的SDK owner tests。
Gate D项目玩法文件变化，只重跑对应样例behavior与消费该样例的最小tool test。
测试代码变化只失效对应test/harness证据，不自动失效未变产品artifact。
```

每个连续施工授权最长三小时。预计无法在剩余时间内完成当前 Gate、证据写入和清理时，不启动新的昂贵命令；
记录 `stopped_by_time_limit`，后续需新的用户授权。不得因时间限制把单一 owner拆成更多永久Gate。

## 10. 完成、文档同步与归档

只有 Gate A-D全部通过后才允许：

1. 生成 `阶段完成记录/2026-09-xx-Project-Game-SDK-Generated-Runtime-Glue-v1/00-总览.md`。
2. 记录实际修改、product/test identity、每条命令结果、复用证据、失败与修复、未运行项和残留风险。
3. 更新正式310方案的施工终态，不改变已冻结架构边界。
4. 更新 `README.md`、`00-文档地图.md`、`00-阅读顺序.md`、`49`、`54`、施工文档README和阶段记录README。
5. 将本文从 `待执行/` 激活后所在的 `当前/` 移入 `已完成/`，清空当前施工槽。

任一 Gate未通过时不得写“已完成”、不得生成完成记录、不得归档到 `已完成/`。若正式方案需要改变，先停止源码施工，
修订方案和本文并重新自审。

## 11. 施工文档自审

### 11.1 与正式310方案一致

- 只新增一个项目侧 Project Game SDK Interface。
- Generated Runtime Glue 是现有 Compiler 的私有派生 Adapter，不是独立 Module。
- Provider、Compiler、RuntimePackage、Player/export owner全部复用，不新增二级Host或AI工具。
- manifest、typed registration、Compiler derived facts各有唯一owner。
- Headless、Optional Editor、Windows、Android只统一source composition，不扩平台能力。

### 11.2 不过量审计

- 四个 Gate各自对应一个不可合并的真实风险，没有为文档完整性增加空 Gate。
- 只有一个最小compile/load fixture；不新增Runner、golden项目矩阵或production qualification。
- 最宽验证只有一次 `cargo check --workspace --locked`，因为新增共享SDK并迁移多个workspace consumer；
  不运行workspace test、all-features、Local CI或真实环境。
- v1只覆盖三个样例已经真实使用的ABI能力；新能力缺口deferred，不阻塞替换。
- Android只做source composition；Tower只做runtime glue迁移。
- 不保留长期兼容pipeline，不批量迁移实验/历史项目。

### 11.3 可执行性与停止条件

- 每个Gate都有最便宜可打红test、明确terminal evidence、估计成本和停止条件。
- 激活前必须重核方案hash、代码基线、dirty worktree冲突、测试名和fixture。
- 过滤测试必须非零执行；命令名因实现命名调整时，先修本文再施工。
- 任何需要第二owner、ABI扩容、RuntimePackage/export重写或真实外部环境的发现都会停止并回到方案审查。

自审结论：本文覆盖正式310方案所需的最小完整替换链路，没有把未来R3、AI Development Pack、
Semantic Outcome、新Engine能力或发布资格带入当前范围；可以进入待执行队列，但在用户明确授权并完成激活前复核前
保持 `不可施工`。

## 12. 当前状态与下一步

```text
方案：310 Project Game SDK + Generated Runtime Glue v1，方案C
施工文档：本文，已完成归档
队列：已完成
当前施工槽：空
当前施工授权：已完成并结束
代码修改：Gate A-D已完成
测试/Gate：Gate A-D passed；workspace locked check、格式和静态retirement通过
完成记录：阶段完成记录/2026-09-02-310-Project-Game-SDK-Generated-Runtime-Glue-v1/00-总览.md
下一步：不自动激活新系统
```
