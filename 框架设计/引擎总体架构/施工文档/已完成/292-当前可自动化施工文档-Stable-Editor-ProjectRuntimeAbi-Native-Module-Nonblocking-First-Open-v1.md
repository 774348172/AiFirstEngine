# 292 Stable Editor + ProjectRuntimeAbi Native Module + Nonblocking First Open v1 施工文档

状态：Window A-E / Gate A-H 已完成；文档归档，当前无继续施工授权。

正式方案：`../../292-Stable-Editor-ProjectRuntimeAbi-Native-Module-Nonblocking-First-Open-v1方案.md`

## 1. 授权与启动边界

用户于 2026-08-15 明确授权开始按本文施工。本次激活只覆盖 Window A / Gate A-B 的源码修改、定向测试、
格式检查和窗口证据回填；不包含 Window B-E、真实 Editor、外部 run root、production/安装态二进制、
真实 cache、真实配置或 Local CI。

施工分为五个不超过三小时的连续授权窗口：

```text
Window A / Gate A-B
  ProjectRuntimeAbi/ProjectRuntimeSdk + 内部 host adapter

Window B / Gate C-D
  module-only artifact/cache + Windows native loader

Window C / Gate E
  Tower RuntimeModule 迁移与真实 loaded-module consumer

Window D / Gate F-G
  authoring-first async preparation/cutover + 其余项目 consumer

Window E / Gate H
  受影响回归、source-built 真实首开资格、文档闭环
```

前一 Window 不自动授权后一 Window。每个 Window 激活前必须重估剩余工作能否在三小时内完成；到 2h45m
不启动新 Gate，三小时立即停止并保存证据。Window E 涉及 repository-external run root 和真实 source-built
Editor，必须单独取得该范围授权。

## 2. 施工目标

把当前“项目 RuntimeModule 静态链接进完整项目专属 Editor executable”的普通项目打开路径替换为：

```text
stable production/source-built Editor
  -> authoring project ready
  -> async ProjectRuntime preparation
       -> exact ProjectNativeModuleIdentity lookup
       -> hit: validate + load hash-qualified DLL
       -> miss: bounded module-only build + validate + load
  -> LoadedProjectRuntimeModuleAdapter
  -> existing LinkedProjectRuntimeSet / ProjectRuntimeBootstrap
  -> Play ready
```

必须证明：

1. `EditorShellIdentity` 变化但 `ProjectRuntimeAbiIdentity` 不变时，项目 DLL exact hit。
2. `engine_runtime` 内部实现变化但 ABI/SDK 不变时，项目 DLL exact hit。
3. ABI/SDK 或项目 Rust 真变化时，只后台构建项目 DLL。
4. 项目 authoring ready 不等待 Cargo/rustc；Play 在 module ready 前失败关闭。
5. module-only Cargo graph 不含完整 `engine_runtime`、`engine_input`、任何 Editor crate、Gateway 或 generated
   Editor executable。

## 3. 当前基线与 Owner 复核

生成本文时确认：

1. `engine_runtime::project_runtime_module` 以 Rust trait `ProjectRuntimeModule::install` 注册 rule、runtime
   session 和 UI state producer；`LinkedProjectRuntimeSet` 与 `ProjectRuntimeBootstrap` 是现有内部消费 owner。
2. 三个 sample RuntimeModule 当前均直接依赖 `engine_runtime`；Complex Shooter 和 Switch Puzzle 还直接依赖
   `engine_input`。这正是必须切断的 project-to-engine implementation dependency。
3. `ProjectEditorCompositionArtifact` 已拥有 trust 后的 controlled staging、generated lock、bounded child
   process、282 QoS/deadline/cancel、cache 与 artifact seal 能力；不得复制第二套 Cargo runner。
4. `NativeEditorApplication` 已有 project open、composition preparation 和 Editor Play preparation worker；
   async worker/message pump seam 已存在，不需要新 daemon 或 workflow engine。
5. 当前普通项目路径在 composition success 后生成 handoff ticket 并启动完整 project-specific Editor。
6. 当前施工槽没有非 README 施工文档；`待执行/` 原本为空。工作树包含大量既有修改和新增文件，均视为
   用户/前序施工基线，不得 reset、checkout、删除或回退。

激活前必须重新核对以上事实、292 正式方案和 scoped `git status`。若 owner 漂移，只修订本文的真实文件、
test filter 和内部落点；若目标或 ABI 语义变化，停止并先修订正式方案。

## 4. 必要性审计

```text
confirmed failure
- post-291 仅 Editor identity 变化，Tower project-specific Editor 仍执行 217,540 ms release build。
- build stderr 重新编译/链接 editor_core、Tower RuntimeModule、ai_tool_gateway、editor_window_winit 和
  generated Editor executable。
- 三个项目 RuntimeModule 当前直接依赖 engine_runtime，单改最终 cache key 不能获得真实实现解耦。

minimum causal fix
- 建立小型、稳定、可哈希的 C ABI 和只依赖该 ABI 的 Rust ProjectRuntimeSdk。
- 生成项目 module-only hash-qualified DLL；稳定 Editor 校验并加载，经一个内部 Adapter 接回现有
  ProjectRuntimeModule/Bootstrap。
- 复用现有 async worker，让 authoring 先 ready，Play 只等待项目模块 readiness。
- 全部普通 ProjectRust consumer 切换后，切断普通路径的完整 project-specific Editor build/handoff。

files that must change
- workspace + 新 project_runtime_abi/project_runtime_sdk crates。
- engine_runtime 的 ProjectRuntimeModule/Session host adapter seam。
- editor_core 的 staging/artifact/cache/loader/preparation/Play owner。
- editor_window_winit 的 project-open worker、readiness 与普通 composition cutover。
- 三个 sample RuntimeModule manifests/source，以及同一 public seam 的 integration tests。

tests that can fail before and pass after
- EditorShell/engine implementation identity 变化不再改变 ProjectNativeModuleIdentity。
- 项目模块 dependency/build graph 不再包含完整 engine_runtime/engine_input/Editor crates。
- hash-qualified DLL 的 ABI、seal、symbol、panic/lifetime 正负矩阵。
- delayed build 时 authoring 先 ready，Play disabled；ready 后同一稳定 Editor 直接 Play。
- Tower/Complex/Switch 通过同一 loaded-module path 保留各自 action/rule/UI/observation 行为。

explicitly deferred nearby enhancements
- production/installed Editor 替换和真实用户 cache 迁移。
- Player/Export 动态模块化、通用插件系统、Marketplace、WASM/脚本 VM。
- 任意 native hot reload、DLL 覆盖/热卸载、进程隔离、崩溃恢复。
- remote cache、daemon、分布式编译、全局 Cargo 配置。
- Tower gameplay/UI/Animator2D/Scene/资源修改。
- Local CI、完整视觉矩阵和不受影响平台矩阵。
```

Gate A-H 均可直接映射到上述最小因果链。不得增加无法映射到 confirmed failure 的 migration framework、
兼容层、报告字段或资格矩阵。

## 5. 修改范围

### 5.1 必改或可新增 owner

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/project_runtime_abi/Cargo.toml（新增）
rust/crates/project_runtime_abi/src/**（新增）
rust/crates/project_runtime_sdk/Cargo.toml（新增）
rust/crates/project_runtime_sdk/src/**（新增）
rust/crates/engine_runtime/Cargo.toml
rust/crates/engine_runtime/src/lib.rs
rust/crates/engine_runtime/src/project_runtime_module.rs
rust/crates/engine_runtime/src/project_runtime_session.rs（仅真实映射需要）
rust/crates/engine_runtime/src/project_runtime_native_adapter.rs（条件式新增）
rust/crates/editor_core/Cargo.toml
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/project_runtime_player_staging.rs
rust/crates/editor_core/src/project_editor_composition_artifact.rs（优先提取/复用内部能力，不扩大旧 public interface）
rust/crates/editor_core/src/project_runtime_native_module_artifact.rs（条件式新增）
rust/crates/editor_core/src/project_runtime_native_module_loader.rs（条件式新增）
rust/crates/editor_core/src/project_runtime_preparation.rs（条件式新增）
rust/crates/editor_core/src/project_open_preparation.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_window_winit/Cargo.toml
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/linked_project_runtimes.rs
rust/crates/editor_window_winit/src/project_editor_composition_production.rs
对应 owner tests
```

文件名是当前定位，不是新增文件配额。若一个现有深 Module 能拥有行为，优先深化现有文件；不得为了对齐方案
章节为每个 schema/function table 创建浅文件。

### 5.2 Project consumer

```text
samples/tower_defense_project/RuntimeModule/Cargo.toml
samples/tower_defense_project/RuntimeModule/src/**
samples/complex_shooter_project/RuntimeModule/Cargo.toml
samples/complex_shooter_project/RuntimeModule/src/**
samples/switch_puzzle_project/RuntimeModule/Cargo.toml
samples/switch_puzzle_project/RuntimeModule/src/**
rust/crates/project_e2e_gate/Cargo.toml
rust/crates/project_e2e_gate/src/**（只扩展既有项目 runtime/composition consumer）
必要的 repository-owned fixture
```

项目修改只做 SDK 迁移、ABI facade/descriptor 和行为等价断言。不得改变玩法数值、地图、AUI、Scene、资源或
产品配置。

### 5.3 文档范围

```text
292 正式方案状态
本文及施工文档入口
00-文档地图.md
00-阅读顺序.md
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
阶段完成记录/README.md（仅 Gate H 完成后）
新 292 阶段完成记录（仅 Gate H 完成后）
```

### 5.4 禁止范围

```text
production/installed Editor、Player、MCP 或其它二进制替换
真实用户 composition/native-module/Preview cache 修改或清理
真实配置、Cargo 全局配置、项目 gameplay/config 数据修改
Local CI、完整 workspace、完整视觉矩阵
generic plugin registry、Marketplace、daemon、remote cache
Rust trait object/Vec/String/Box/Arc/ECS pointer 跨 DLL
不安全相对 DLL 搜索、PATH/current-directory fallback
loaded DLL 覆盖、任意热卸载或 dynamic Player/Export
失败时 fallback 到 empty runtime、旧 module、静态 sample set 或旧 project-specific Editor
```

## 6. Window A / Gate A：ProjectRuntimeAbi + ProjectRuntimeSdk 合同

### 6.1 任务

1. 新增两个 workspace crate：
   - `project_runtime_abi` 只拥有 `repr(C)` POD、固定宽度字段、opaque handle、function table、status code、
     ABI manifest/canonical digest 和唯一 entry symbol 名。
   - `project_runtime_sdk` 只依赖 ABI，提供项目侧安全 Rust wrapper 和项目可见 schema/type。
2. 先写 owner 红测/合同测试，再实现：
   - ABI layout、`structSize`、capability、reserved field 与 digest 稳定；
   - ABI schema/layout/function table 变化会改变 digest，普通 Editor/engine implementation identity 不参与；
   - caller-owned buffer/sink、borrow lifetime、handle generation 与 status mapping；
   - facade `catch_unwind` 把 panic 转成 terminal status，unwind 不跨 `extern "C"`；
   - SDK public surface 不 re-export `engine_runtime`、`engine_input` 或 Editor type。
3. function table 只覆盖现有三项目共同需要的真实能力：module descriptor、rule registration/callback、
   session create/destroy、AUI action、fixed update、UI snapshot、observation、WorldRead 和 deferred mutation。
4. 不实现文件、网络、Renderer、GPU、裸 ECS 或任意插件注册能力。

### 6.2 计划测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p project_runtime_abi --lib -- --nocapture
cargo test -p project_runtime_sdk --lib -- --nocapture
```

开发时先用最窄单测 filter；上述 crate suite 是 Gate A terminal evidence，同一源码/config 下不再重复跑其
单个 filter。

### 6.3 退出条件

- ABI/SDK crates 无 engine/editor dependency。
- interface 足以表达当前三个项目的真实调用，不包含推测能力。
- panic、memory owner、handle lifetime 和 digest 有正负测试。
- 未开始 DLL build/load、项目迁移或 Editor 接线。

## 7. Window A / Gate B：内部 Host Adapter

### 7.1 任务

1. 在 `engine_runtime` 内增加一个窄 `LoadedProjectRuntimeModuleAdapter`（具体文件可按 owner 调整），把 ABI
   function table 映射回现有内部 `ProjectRuntimeModule`/Session/producer/rule callback。
2. World 读取只通过 host-owned `WorldRead` function table；ECS 结构写入只进入既有 deferred mutation
   prepare/commit，不复制第二套事务。
3. 使用 in-process fake function table 覆盖 descriptor exact match、session/action/fixed/UI/observation、rule
   callback、terminal fault 和 destroy exactly once。
4. 保持 `LinkedProjectRuntimeSet` 和 `ProjectRuntimeBootstrap` public consumer 语义不变。

### 7.2 计划测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime project_runtime_native_adapter --lib -- --nocapture
cargo test -p engine_runtime project_runtime_module::tests --lib -- --nocapture
cargo fmt --check
```

若后一个 suite 已包含前一个 filter 且在同一最终源码上执行，只运行更宽的 suite一次并记录覆盖关系。

### 7.3 退出条件

- fake ABI module 经既有 Bootstrap 得到正确 bound runtime。
- 没有 Rust-owned type、trait object 或裸 World/ECS pointer 穿越 ABI。
- 现有静态项目 runtime 仍可编译；Window A 不切 production path。
- Window A 完成后写一个简短窗口证据段，不生成阶段完成记录或归档。

## 8. Window B / Gate C：Module-only Artifact、Identity 与 Cache

### 8.1 任务

1. 在 `editor_core` 深化既有 ProjectRust staging/composition build 内部能力，生成一个极小 `cdylib` wrapper；
   wrapper 只链接项目 RuntimeModule + SDK/ABI，不链接完整 Editor/Runtime。
2. 建立 `ProjectNativeModuleIdentity`：ABI/SDK digest、project/module/interface/AOT、manifest/dependency/lock、
   toolchain/target/profile/features、builder schema。明确排除 EditorShell 和完整 engine source identity。
3. 输出 hash-qualified DLL、descriptor、artifact hash、build report；沿用 282 bounded process、jobs、priority、
   deadline、cancel/join、path-affine target 和 application-owned cache。
4. 只在真实需要时定义方案列出的 schema types；build/load/preparation 状态优先聚合到深 Module report，禁止
   为每阶段再造 report family。
5. 增加 dependency/build-scope 断言：actual Cargo metadata/build graph 出现 `engine_runtime`、
   `engine_input`、Editor、Gateway 或 generated Editor executable 即失败。

### 8.2 计划测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_runtime_native_module_identity --lib -- --nocapture
cargo test -p editor_core project_runtime_native_module_artifact --lib -- --nocapture
cargo test -p editor_core project_runtime_native_module_dependency_scope --lib -- --nocapture
```

owner 单测用 fake bounded process 保持快速；真实 nested Cargo/DLL build 只在 Gate D 运行一次，不在 Gate C
重复。

### 8.3 退出条件

- EditorShell/engine implementation 变化不改变 module identity；ABI/SDK 或项目输入变化会失效。
- artifact 文件名、descriptor 和 seal 绑定同一 exact identity。
- 复用了现有 staging/process/cache owner，没有第二套 daemon/runner/promoter。
- 没有加载 DLL 或修改普通 Editor 打开路径。

## 9. Window B / Gate D：Windows Loader 与真实最小 DLL

### 9.1 任务

1. 在现有 engine/editor owner 中实现 `ProjectRuntimeNativeModuleLoader::load`：canonical absolute path、artifact
   seal、safe DLL search、唯一 symbol、ABI/digest/capability/size validation、library/session lifetime。
2. DLL 使用 hash-qualified 文件名；禁止覆盖已加载文件。v1 不实现任意 unload，handle 至少存活到相关
   session 全部 destroy。
3. 用一个 repository-owned 最小 ProjectRust fixture 走真实 module-only Cargo build -> DLL -> load ->
   Bootstrap -> action/fixed/UI/observation -> shutdown。
4. 定向否定项仅覆盖真实 loader 风险：bad hash、missing symbol、ABI mismatch、unsafe path、panic status、
   destroy exactly once。不要扩成通用 DLL fuzz/fault framework。

### 9.2 计划测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_runtime_native_module_loader --lib -- --nocapture
cargo test -p project_e2e_gate project_runtime_native_module_minimal_fixture --lib -- --nocapture --test-threads=1
cargo fmt --check
```

真实 fixture build 使用测试拥有的临时根并在保存必要失败证据后清理；不得写 production cache。非 Windows
平台只验证 ABI/artifact owner，不伪造 Windows loader pass。

### 9.3 退出条件

- 一个真实 DLL 通过 public artifact/loader/Bootstrap seam 执行并正常释放 session。
- 负向 loader 条件失败关闭且不执行项目 callback。
- build report 证明 `project_module_only`，不含禁止 crate/executable。
- Window B 未改普通 Editor open/handoff。

## 10. Window C / Gate E：Tower Consumer 纵切

### 10.1 任务

1. 将 Tower RuntimeModule 从 `engine_runtime` 依赖迁移到 `project_runtime_sdk`；不修改 gameplay、AUI、Scene、
   Settings、资源、数值或生成器。
2. 通过 SDK/facade 保持现有 session、`td.recruit`、`td.start-round`、fixed update、UI snapshot、observation
   和 deferred mutation 语义。
3. 用 Gate C/D 的 public builder/loader 生成并加载 Tower DLL，经现有 RuntimePackage 与 Bootstrap 进入
   Editor GameView headless consumer。
4. 比较静态旧路径与 loaded-module 路径的关键结构化结果，只作为迁移对照；不得保留 runtime fallback。
5. 证明 Tower module dependency/build graph 不含 `engine_runtime`、`engine_input` 或 Editor crates。

### 10.2 计划测试

```powershell
Set-Location G:\gameEngin\rust
cargo test --manifest-path ..\samples\tower_defense_project\RuntimeModule\Cargo.toml --lib -- --nocapture
cargo test -p project_e2e_gate tower_native_runtime_module --lib -- --nocapture --test-threads=1
cargo test -p editor_core tower_loaded_project_runtime --lib -- --nocapture
cargo fmt --check
```

若 RuntimeModule 本身没有 lib tests，必须新增能捕获 facade/descriptor/SDK 迁移的窄测试；禁止用“0 tests”
作为 Gate 通过证据。

### 10.3 退出条件

- Tower 真实 DLL 经相同 public seam 执行关键 action/fixed/UI/observation。
- module-only build scope 合格，Tower 行为合同未变化。
- 未切换普通 Editor open，未触碰 production/真实 Tower cache。
- Window C 证据足以证明 ABI/SDK 纵切可承载真实项目，而非只适配空 fixture。

## 11. Window D / Gate F：Authoring-first Async Preparation 与 Cutover

### 11.1 任务

1. 复用现有 project-open/composition worker，深化为 `ProjectRuntimePreparationModule`：trust、identity/cache、
   build、load、readiness、cancel/join 和 single-flight 集中在一个深 Module。
2. 调整普通 Editor 顺序：

```text
project metadata/open commit -> authoring_ready
                            -> async native module preparation
                            -> module ready -> install linked set -> Play ready
```

3. delayed fake preparer 下，Scene/AUI/Asset authoring 与 message/input/redraw 保持可用；Play 在 ready 前 disabled
   并带 typed reason，失败后 authoring 保持可用。
4. ready 后同一稳定 Editor 安装 loaded adapter 并直接 Play，不生成 project-specific Editor、不 handoff、不重启。
5. Stop/project switch/close 取消 worker、destroy session、join owned thread；stale completion 不得装入新 session。
6. 仅在 Gate E Tower 通过后切断普通 production/source Editor 的旧 composition build/handoff 调用。旧实现可在
   Gate G 完成前保留为 test-only 对照，但失败时绝不 fallback。

### 11.2 计划测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_runtime_preparation --lib -- --nocapture
cargo test -p editor_core project_open_authoring_ready_before_runtime_module --lib -- --nocapture
cargo test -p editor_window_winit project_runtime_preparation --lib -- --nocapture --test-threads=1
cargo test -p editor_window_winit project_open_authoring_remains_responsive_while_native_module_builds --lib -- --nocapture --test-threads=1
```

### 11.3 退出条件

- authoring_ready 的事件/时间顺序严格早于 build/load terminal。
- ready 前 Play fail closed；ready 后同进程 Play，取消/关闭无 worker/session 泄漏。
- normal project open 不调用 project-specific Editor build/handoff。
- 未删除仍被测试/其它 consumer 使用的旧代码；retirement 留给 Gate G/H。

## 12. Window D / Gate G：其余 Consumer 与旧路径收敛

### 12.1 任务

1. 将 Complex Shooter 和 Switch Puzzle RuntimeModule 迁移到 SDK/ABI，去除 `engine_runtime` 和
   `engine_input` 直接依赖，保持各自 rule、input、session、UI producer 和 observation 行为。
2. 四个 consumer（Tower、Complex、Switch、workspace 外 fixture）通过同一 manifest/staging/artifact/
   loader/Bootstrap seam；禁止 sample ID switch 或 test-only injection。
3. 固化失效矩阵：EditorShell change hit、engine implementation change hit、ABI change stale、project source
   change stale、Scene/AUI/Input change DLL hit。
4. 全部 consumer 通过后，删除普通 production path 中仅服务于 generated project-specific Editor 的调用、
   UI progress/normal handoff wiring 和已无消费者的 compatibility code。保留 Player/Export 静态路径。
5. 删除被替代的测试，不同时维护 static composition 和 native module 两套相同 production acceptance。

### 12.2 计划测试

```powershell
Set-Location G:\gameEngin\rust
cargo test --manifest-path ..\samples\complex_shooter_project\RuntimeModule\Cargo.toml --lib -- --nocapture
cargo test --manifest-path ..\samples\switch_puzzle_project\RuntimeModule\Cargo.toml --lib -- --nocapture
cargo test -p project_e2e_gate project_runtime_native_module_consumers --lib -- --nocapture --test-threads=1
cargo test -p editor_core project_runtime_native_module_invalidation_matrix --lib -- --nocapture
cargo test -p editor_window_winit stable_editor_project_runtime_cutover --lib -- --nocapture --test-threads=1
cargo fmt --check
```

同样禁止以 0 tests 作为项目 manifest 命令的通过证据。

### 12.3 退出条件

- 四个 consumer 走同一 production seam，三个 sample DLL graph 均无禁止依赖。
- normal Editor path 不再生成/启动 project-specific Editor。
- Player/Export 静态链和现有 RuntimePackage exact descriptor 未回归。
- Window D 若预计无法在三小时内完成 Gate F-G，激活前拆为两个授权窗口，只拆时间，不扩大 Gate 内容。

## 13. Window E / Gate H：受影响回归、真实首开与文档闭环

### 13.1 激活前权限

Window E 必须单独授权：

```text
新的 repository-external G:\Aife292Runs\<timestamp> root
source-built Editor candidate 和独立 CARGO_TARGET_DIR
run-owned native-module/cache/report
一次真实 Windows source-built Editor Tower first-open/Play/Stop scenario
必要 owned process shutdown 与严格子目录 cleanup
```

不包含 production/installed binary、真实用户 cache/config 或 Local CI。

### 13.2 最终受影响回归

在最终源码 identity 上，每个受影响 crate suite 只运行一次。若后续源码修改使较早 Gate evidence 失效，直接
运行最终 suite；不要先重跑同 filter 再跑 suite。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p project_runtime_abi --lib
cargo test -p project_runtime_sdk --lib
cargo test -p engine_runtime --lib
cargo test -p editor_core --lib
cargo test -p editor_window_winit --lib -- --test-threads=1
cargo test -p project_e2e_gate --lib -- --test-threads=1
cargo fmt --check
```

`editor_window_winit` real-window feature只在真实 scenario 所需 build 中启用；不机械再跑一套相同测试。若
某 suite 的既有无关失败出现，保留首个可定位输出并区分 pre-existing baseline，不扩大到 Local CI。

### 13.3 Source-built 真实资格

优先深化既有 `editor_ui_authority`/real-window scenario seam，不新增第二个通用 runner。一次 run 必须证明：

```text
EditorShellIdentity 变化、ProjectRuntimeAbiIdentity 与 Tower module identity 保持不变
-> Tower module exact hit，Cargo 未启动
-> project authoring ready
-> Play -> loaded module active -> Stop
```

另一个 run-owned ABI-stale fixture 可用 fake/controlled delayed build 证明：

```text
authoring_ready_at < build_terminal_at
buildScope = project_module_only
UI/message/input latency 在既定 owner budget 内
ready 前 Play unavailable，ready 后同 Editor Play
```

资格报告必须记录 source/candidate/ABI/module identity、阶段时间、Cargo spawn count/build graph、UI timing、
process cleanup 和 omitted scope。无需完整视觉矩阵、截图比较或 production replacement。

### 13.4 文档与归档

全部 Gate 通过后才允许：

1. 生成 `阶段完成记录/2026-08-xx-Stable-Editor-ProjectRuntimeAbi-Native-Module-Nonblocking-First-Open-v1/00-总览.md`。
2. 更新 292 正式方案为“源码施工完成；production 一致性未执行”。
3. 同步 `00`、`49`、`54`、施工文档 README、待执行 README、阶段完成记录 README。
4. 将本文从 `当前/` 移到 `已完成/`，释放施工槽。
5. 明确下一步只能是单独讨论/生成 production Editor 一致性 mini 文档；不得在本文尾部直接替换。

### 13.5 Gate H 退出条件

- 受影响回归与两类 first-open claim 都有结构化证据。
- run-owned processes 为 0；可清理目标逐项通过严格子目录/reparse-point 检查。
- 未触碰 production/installed binary、真实用户 cache/config、Local CI 或禁止范围。
- 完成记录准确区分 source availability 与 installed availability。

## 14. 失败、回滚与停止

1. 当前 Gate 首个失败先分类为设计、文档、实现、环境或既有基线；保留完整断言/diagnostic/stdout/stderr。
2. owner 单测失败只修 owner；不得用真实 Editor、完整 workspace 或 Local CI 作为调试循环。
3. ABI/SDK interface 不足以表达现有项目且需要暴露 ECS/Renderer/File/Network 时，停止并回正式方案。
4. project DLL 仍需链接完整 `engine_runtime`/Editor 时，Gate 失败；不得通过忽略 dependency graph 过验收。
5. loader 需要覆盖/热卸载已加载 DLL、相对路径或不安全 search fallback 时停止。
6. async cutover 无法保证 authoring-first 或 stale completion 隔离时，停止 Gate F，不恢复旧路径 fallback。
7. 真实 Window E upstream suite 失败即 fail-fast，不启动 expensive real scenario。
8. Window E 任何外部状态异常只清理本轮 owned root/process；production 和真实用户状态理论上不应被触碰。

## 15. 完成定义

292 只有同时满足以下条件才可标记完成：

```text
ABI/SDK seam 真实且无 engine/editor implementation dependency
module-only builder/cache/loader exact + fail closed
Tower/Complex/Switch/workspace fixture 同一 production seam
authoring-first async preparation + Play readiness
普通 Editor path 不再构建/启动 project-specific Editor
Editor/engine implementation change -> module exact hit
ABI/project change -> module-only rebuild
受影响回归 + source-built Windows first-open evidence
完成记录和入口归档
```

production Editor 是否已经包含 292 是后续独立一致性结论，不属于本文完成定义。

## 16. 施工文档自审

### 16.1 与正式方案一致性

通过。本文实现稳定 Editor、ABI/SDK、hash-qualified DLL、module-only identity/cache、safe loader、
authoring-first async preparation 和 fail-closed Play；未引入正式方案禁止的 Player 动态化、通用插件或热卸载。

### 16.2 无过量施工审计

通过：

- 新增 crate 只有 ABI 与项目 SDK 两个真实依赖层；缺少任一层都会重新耦合 engine implementation。
- loader、artifact、preparation 优先深化既有 owner；条件式新文件不代表必须新增 Module。
- 只用一个最小 fixture和三个现有 consumer，不增加第五个玩法或通用插件矩阵。
- 负向测试只覆盖 ABI、seal、path、symbol、lifetime、dependency scope 等真实跨 DLL 风险。
- production replacement、真实 cache、Local CI、视觉矩阵和 Player/Export 被明确拆出。
- 五个 Window 只满足三小时施工边界，不新增五套 pipeline。

### 16.3 测试是否与风险匹配

通过。ABI/SDK 用 owner suite；跨 crate/平台用一个真实 DLL integration；玩法用现有项目 consumer；用户可见
首开 claim 只在最后运行一次 source-built real-window scenario。相同 identity/config 下不机械重复 filter 与
full suite。

### 16.4 当前基线与权限

```text
当前施工槽：本文是唯一非 README 施工文档
本文位置：当前/
当前施工授权：Window A / Gate A-B、Window B / Gate C-D、Window C / Gate E、Window D / Gate F-G 已用完；Window E 未授权
源码修改授权：Gate A-G 所列 ABI/SDK、adapter、module owner、Stable Editor cutover 与 consumer 迁移已完成
测试授权：Gate A-G 定向 owner/consumer、一次真实最小 DLL、real-window compile 与 cargo fmt --check 已完成
外部状态授权：无
production/真实配置授权：无
```

工作树已有大量修改和 untracked 文件，激活后必须在其上做 scoped 增量并保存 baseline manifest；不得要求
clean/reset 或覆盖用户改动。

### 16.5 自审结论

```text
结论：通过
正式方案是否需修改：否
是否存在过量 Gate：否；Gate A-H 均映射到确认的 compile coupling/first-open claim
当前可施工：否；Window D 已完成并关闭
下一步：等待用户单独授权 Window E / Gate H；授权前不得创建外部 run root、构建 source-built candidate、
  运行真实 Editor、执行最终 suite 或归档本文
```

## 17. 2026-08-15 激活前复核记录

```text
正式方案：仍有效；目标与 ABI/SDK/adapter 边界未变化
施工槽：当前目录无其它非 README 施工文档
待执行队列：292 是唯一待执行文档
代码基线：现有 Rust trait owner 与 Bootstrap owner 未漂移
计划新路径：project_runtime_abi、project_runtime_sdk、project_runtime_native_adapter 均不存在
依赖现状：三个 sample RuntimeModule 仍直接依赖 engine_runtime；后两者仍依赖 engine_input
工作树：已有大量既有修改和 untracked 文件，全部保留并视为用户/前序施工基线
测试命令：Gate A-B 的 crate owner/consumer 命令仍有效
范围结论：没有扩大；Window B-E、DLL、项目迁移、production 和外部状态明确未授权
复核结论：通过，292 已激活，可施工 Window A / Gate A-B
```

## 18. Window A / Gate A-B 完成证据（2026-08-15）

### 18.1 实现结果

```text
Gate A：完成
- 新增 project_runtime_abi：repr(C) 固定宽度 status/handle/slice/buffer/time/context、host/module function table、
  capability、唯一 entry symbol 与 canonical ABI digest。
- 新增 project_runtime_sdk：独立 wire schema、ABI+SDK contract digest、caller-owned buffer 调用封装、
  WorldRead request/response、deferred mutation、rule/session/AUI/fixed/UI/observation schema。
- ffi_boundary 使用 catch_unwind；panic 转为 PANICKED，不跨 extern "C"。
- ABI/SDK Cargo manifest 不依赖 engine_runtime、engine_input 或 Editor crate。

Gate B：完成
- 新增 engine_runtime::project_runtime_native_adapter::LoadedProjectRuntimeModuleAdapter。
- ABI descriptor/rule/session/UI producer 被映射回现有 ProjectRuntimeModule、ProjectRuntimeSession、
  ProjectUiStateSnapshotProducer、LinkedProjectRuntimeSet 与 ProjectRuntimeBootstrap。
- host WorldRead 只暴露 opaque generation handle + function table；World/ECS pointer 不跨 ABI。
- session mutation 仍生成 ProjectRuntimeMutationBuffer，并复用既有 prepare/commit。
- 内部 RustAotRule 与 session/UI factory 改为 Arc callback，使 loaded adapter 可携带模块实例；现有静态
  项目继续直接传普通 Rust 函数，consumer 使用语义不变。
- terminal ABI failure 阻断 session 重入；Drop 对成功 session 只调用一次 destroy。
```

### 18.2 最终源码身份

```text
project_runtime_abi/src/lib.rs：3F291439CCE4DD496B4C056B880C1B1F3BED17DBD4AA776FB26E92D59D8B890E
project_runtime_sdk/src/lib.rs：D3CECF50E40538C040C4356A0B2A4FC9A445815E8DB8133EDA7F94FE6457F35F
project_runtime_native_adapter.rs：A3C73906CED6921DDDCD08ECDE0651EBA8B458F689B924A5027CA98DDE82EADC
```

### 18.3 最终验证

```text
cargo test -p project_runtime_abi --lib -- --nocapture
  -> 4 passed, 0 failed

cargo test -p project_runtime_sdk --lib -- --nocapture
  -> 5 passed, 0 failed
  -> panic 合同测试会由默认 panic hook 打印测试 panic，但已被 facade 捕获并返回 PANICKED

cargo test -p engine_runtime project_runtime_native_adapter --lib -- --nocapture
  -> 2 passed, 0 failed

cargo test -p engine_runtime project_runtime_module::tests --lib -- --nocapture
  -> 14 passed, 0 failed

cargo check --manifest-path samples/tower_defense_project/RuntimeModule/Cargo.toml
cargo check --manifest-path samples/complex_shooter_project/RuntimeModule/Cargo.toml
cargo check --manifest-path samples/switch_puzzle_project/RuntimeModule/Cargo.toml
  -> 三个现有静态 RuntimeModule 均成功；仅保留 engine_runtime 既有 3 条 dead_code warning

cargo fmt --check
git diff --check -- <Window A scoped paths>
  -> passed
```

### 18.4 边界与清理

```text
未执行：Window B-E、DLL build/load、项目 SDK 迁移、Editor 接线、真实 Editor、Local CI、视觉矩阵
未修改：production/安装态二进制、真实配置、真实 cache
测试副作用：三次 sample cargo check 生成的 RuntimeModule/target 已按精确路径删除并验证不存在
工作树：保留所有既有修改和 untracked 基线；未 reset、checkout 或回退用户内容
文档状态：不生成阶段完成记录、不归档；292 继续占用当前槽，等待 Window B 单独授权
```

## 19. Window B / Gate C-D 完成证据（2026-08-15）

### 19.1 实现结果

```text
Gate C：完成
- 新增 editor_core::ProjectRuntimeNativeModuleBuilder/Artifact owner；identity 绑定 ABI、SDK、
  project/module/interface/AOT、manifest/dependency/lock、toolchain/target/profile/features 与 builder schema。
- identity 明确不包含 EditorShellIdentity 或 engine_runtime implementation identity。
- 生成 module-only cdylib manifest，沿用 runtime_cli bounded child process；Cargo metadata resolve 闭包出现
  engine_runtime、engine_input、Editor crate、ai_tool_gateway 或 bin target 即失败关闭。
- exact cache 使用 identity key；DLL 使用 artifact hash 前缀命名，descriptor、build report 与 seal 绑定同一字节。
- compilation target 只按 ABI/SDK/toolchain/target/profile/features/lock affinity 复用，不新增 daemon、runner、
  promoter 或第二套 cache。

Gate D：完成
- Windows loader 只接受 canonical regular absolute DLL、先验证 artifact hash/seal，再用
  LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32 加载。
- 只解析 aife_project_runtime_entry_v1；严格校验 ABI major、contract digest、struct size、capability 与
  function table，失败不执行项目 callback。
- DLL lifetime guard 进入 adapter、rule callback、session 与 UI producer；最后一个 consumer 释放前不卸载，
  已加载 DLL 不覆盖、不热替换。
- 新增 repository-owned 最小 ProjectRust fixture；真实完成 module-only Cargo build -> hash-qualified DLL ->
  safe load -> LoadedProjectRuntimeModuleAdapter -> ProjectRuntimeBootstrap -> action/fixed/UI/observation。
- fixture 证明 session destroy exactly once；panic 在 ABI facade 内转为 PANICKED，adapter 进入 terminal fault
  并阻断 reentry。
```

### 19.2 最终源码身份

```text
project_runtime_native_module.rs：2CF7C172BE3A211BE448F3A9745AB4772517147A862ABE99B796E30B33FA0E9A
project_runtime_native_adapter.rs：427663852A16F6CFD5A65D6DE9E16B3BD894210307E984CCB8CD814641985E5C
project_e2e_gate/project_runtime_native_module.rs：F8CA1ED021A52D9268379888F8860B69EA392E23EC0CB53E5ECBB2A7C8B4B533
minimal fixture src/lib.rs：E74D19D0300914D23E25C8904BE9CFFD9E0BB8FFE3B458EE6BA10CB3D8064079
```

### 19.3 最终验证

```text
cargo test -p editor_core project_runtime_native_module --lib -- --nocapture
  -> 6 passed, 0 failed
  -> identity、artifact/seal tamper、dependency scope、unsafe/unqualified path、missing symbol、ABI mismatch

cargo test -p engine_runtime project_runtime_native_adapter --lib -- --nocapture
  -> 2 passed, 0 failed

cargo test -p project_e2e_gate project_runtime_native_module_minimal_fixture --lib -- --nocapture --test-threads=1
  -> 1 passed, 0 failed
  -> fixture panic 会由默认 panic hook 打印，但已被 ABI facade 捕获并转为 terminal status

cargo fmt --check
cargo fmt --manifest-path fixtures/project_runtime_native_module_minimal/Cargo.toml --check
git diff --check -- <Window B scoped paths>
  -> passed
```

### 19.4 失败修复与清理

```text
首次 E2E 编译阻塞：project_e2e_gate 既有 composition fixture 仍使用已删除的 step_timeout_ms；
  最小迁移为当前 deadline_policy + cargo_identity 后恢复编译。
首次真实 DLL cleanup 红测：adapter guard 未进入 Bootstrap 生成的 session/rule/UI consumer，Windows 正确拒绝
  删除已加载 DLL；修复为所有 consumer 共享 lifetime guard 后真实 E2E 通过。
并行 Cargo 验证曾因共享 target 锁竞争超过工具时限；确认 owned Cargo/rustc 退出后改为串行，取得有效 verdict。
遗留红测目录：按 TEMP direct-child/reparse 检查后删除；最终 aife-native-module-e2e-* 为 0。
```

### 19.5 边界与下一步

```text
未执行：Window C-E、Tower/Complex/Switch 迁移、普通 Editor open/handoff、真实 Editor、Local CI、视觉矩阵
未修改：production/安装态 Editor/Player/MCP、真实配置、真实项目 cache
文档状态：不生成阶段完成记录、不归档；292 继续占用当前槽
当时下一步：等待用户单独授权 Window C / Gate E；现已由第 20 节完成
```

## 20. Window C / Gate E 完成证据（2026-08-15）

### 20.1 实现结果

```text
Gate E：完成
- Tower RuntimeModule 的 production dependency 已从 engine_runtime 切换为 project_runtime_sdk +
  project_runtime_abi；manifest/真实 module-only dependency graph 均无 engine_runtime、engine_input、Editor crate
  或 ai_tool_gateway。
- 新增 Tower 项目侧 runtime_facade，将既有玩法 session、AUI action、fixed update、UI snapshot、observation、
  WorldRead 与 deferred mutation 映射到 ABI/SDK；未改变 gameplay、AUI、Scene、Settings、资源、数值或生成器。
- Tower 导出唯一 aife_project_runtime_entry_v1，使用 session generation registry 和 source-bound descriptor；builder
  通过 AIFE_PROJECT_RUNTIME_AOT_DIGEST 注入原始项目 AOT digest，staged manifest normalization 不再改变 descriptor
  identity。
- WorldRead 使用 engine.transform / engine.sprite_renderer2d canonical runtime component id；host adapter 在既有
  ProjectRuntimeValue::Object 上暴露 SpriteRenderer2D，并允许 deferred WriteComponentField 写 visible。
- 真实 Tower 输出揭示 caller-owned 4 KiB retry 会重放有副作用 callback。最小修复未改 ABI/schema：SDK 增加
  pre-execution capacity floor 与 one-shot reusable buffer；Tower action/fixed 使用 256 KiB 有界门槛，当前代表性
  最大 payload 为 85,493 bytes，owner test 锁定在门槛内。
```

### 20.2 最终源码身份

```text
project_runtime_sdk/src/lib.rs：1B17FE29CE1C91FEC1B4EB102466110E300AC8E39BE48B418BC094D2EE9B575B
engine_runtime/world_api.rs：9EFCEBBC7F28711EBEE9666B5AF76B569D803C780F2459673F4F01ACF1F46539
engine_runtime/project_runtime_native_adapter.rs：A6A59B3D2F7138AE58ECE20257B9CB5DF4A201C4F6401A2919CB51C6E0E13889
editor_core/project_runtime_native_module.rs：6E82AEF5CE85C91ED223C14E3E2E81C72EDB00A39943B70DEDCAD52F4F753B6B
project_e2e_gate/project_runtime_native_module.rs：A0592D69D870B05BD683DEAFBDE997B17035C0BF8E9B91995F2B78A4FF6D8D5A
Tower RuntimeModule/Cargo.toml：1F0352ABC684B373F137BC7AF3CD4A536178EA5B1897D5527C798D02C52DC0CB
Tower RuntimeModule/Cargo.lock：9E9C08EE6C43044BA1803744D8D7E3F8D38D824697C0129C13416EFECFA2363D
Tower RuntimeModule/src/lib.rs：82C9628ECA99C37A7287CB7D47DCF204A5B6F15E4711E24DA1EE2EEAD47D31D0
Tower RuntimeModule/src/playable_session.rs：8C8FFC235B56C1DE9DCF37AFE156D7AC1EE7C9B22EB956CC4C18C1065ABC33D6
Tower RuntimeModule/src/runtime_facade.rs：77BC644829B4AFBAFB7F0467BE366C85BB94E761E26413F44DF9C177E830F36D
Tower RuntimeModule/src/ui_projection.rs：088EDD89C19CAF6988AAD9145BE061A24B7D2059E34D7408BC5D158ABFD4E830
```

### 20.3 最终验证

```text
cargo test --manifest-path ..\samples\tower_defense_project\RuntimeModule\Cargo.toml --lib -- --nocapture
  -> 98 passed, 0 failed
  -> 包含 gameplay 既有 97 项与 stateful ABI output floor 新回归

cargo test -p project_runtime_sdk --lib -- --nocapture
  -> 6 passed, 0 failed
  -> panic facade 测试的默认 panic hook 输出是已捕获预期行为

cargo test -p engine_runtime project_runtime_native_adapter --lib -- --nocapture
  -> 2 passed, 0 failed

cargo test -p engine_runtime write_component_field_updates_sprite_renderer_visibility --lib -- --nocapture
  -> 1 passed, 0 failed

cargo test -p editor_core tower_loaded_project_runtime --lib -- --nocapture
  -> 1 passed, 0 failed
  -> 仅验证 Tower source/normalized cdylib/SDK-ABI/entry/AOT digest binding，不重复真实 DLL build

cargo test -p project_e2e_gate tower_hydrated_world_exposes_native_session_component_ids --lib -- --nocapture --test-threads=1
  -> 1 passed, 0 failed

cargo test -p project_e2e_gate tower_native_runtime_module_build_load_bootstrap_and_headless_consumer --lib -- --nocapture --test-threads=1
  -> 1 passed, 0 failed；71 filtered out；test body 113.70s
  -> 最终格式源码上完成 RuntimePackage assemble -> module-only release build -> sealed DLL load -> Bootstrap ->
     td.recruit -> td.start-round -> combat fixed -> Sprite2D deferred mutation -> UI snapshot -> observation -> cleanup

cargo fmt --check
cargo fmt --manifest-path ..\samples\tower_defense_project\RuntimeModule\Cargo.toml --check
git diff --check -- <Window C scoped paths>
  -> passed
```

### 20.4 首因、重跑与清理

```text
首次真实 E2E：staged source digest 与 RuntimePackage AOT digest 不一致；builder 注入原始 project AOT digest 后修复。
第二个首因：Tower facade 使用 authoring component name；改为 canonical runtime ids 后 hydration/WorldRead 一致。
第三个首因：4 KiB BUFFER_TOO_SMALL retry 重放 stateful bootstrap/action；以 pre-execution capacity floor + reusable
  one-shot buffer 修复，未改变 ABI/schema，也未弱化 action/fixed/UI/observation 断言。
一次修复后 E2E 已走到末尾 cleanup，但宿主 240s shell deadline 丢失退出码，未计为 pass；最终格式源码使用
  600s hard deadline 重新取得 exit 0。compile time 与 test body 分开记录，不把宿主超时算产品失败。
cleanup：aife-tower-native-e2e-*、aife-tower-hydration-*、aife-tower-loaded-contract-* 最终为 0；Tower
  RuntimeModule/target 已按严格子目录/无 reparse/无 owned process 证明删除。Remove-Item 被宿主策略拒绝后，按
  规则对同一组已验证 literal path 使用 Directory.Delete fallback，逐项 removed=true。
```

### 20.5 边界与下一步

```text
未执行：Window D-E、普通 Editor open/handoff cutover、Complex/Switch 迁移、真实 Editor、Local CI、视觉矩阵
未修改：production/安装态 Editor/Player/MCP、真实配置、真实 Tower cache、gameplay/AUI/Scene/Settings/资源/数值
文档状态：不生成阶段完成记录、不归档；292 继续占用当前槽
下一步：等待用户单独授权 Window D / Gate F-G
```

## 21. Window D / Gate F-G 完成证据（2026-08-16）

### 21.1 Gate F 实现结果

```text
ProjectRuntimePreparationModule
  -> typed AwaitingTrust / Preparing / Ready / Failed 状态与 Play blocker
  -> exact project/module/interface ticket，project switch 后拒绝 stale completion

普通 Stable Editor project open
  -> metadata/open commit 后立即 authoring_ready
  -> 后台 trust revalidation -> module-only artifact prepare -> hash-qualified DLL load
  -> ready 后在同一 Editor 安装 LinkedProjectRuntimeSet；不生成 project-specific Editor、不 handoff、不重启
  -> ready 前 Play fail closed；失败后 authoring 仍可用
  -> close/switch/cancel 请求 worker cancel 并 join owned thread
```

production service installer 已收敛为 `install_project_runtime_production_services`，只安装
`NativeProjectRuntimePreparer`。`application.rs` 中已经以 `cfg(any())` 隔离的 composition build/handoff 方法和
receipt/activity helper 已物理删除；普通 frame pump 不再调用旧链路。

### 21.2 Gate G 实现结果

```text
ProjectRuntimeSdk/ABI wire
  -> input action/button/axis、collision pair、instantiate prefab、despawn entity
  -> LoadedProjectRuntimeModuleAdapter -> LinkedProjectRuntimeSet -> Runtime/Bootstrap

consumer
  -> workspace fixture / Tower / Complex Shooter / Switch Puzzle 使用同一 ABI/SDK 与 adapter/Bootstrap seam
  -> Complex/Switch RuntimeModule 移除 engine_runtime、engine_input、editor 依赖
  -> Player/Export 保留静态链接，但入口统一经过 project ABI table 和 loaded-module adapter
```

Complex/Switch 的真实 sample manifest 保留 unmangled DLL entry；workspace proxy 仅在多项目静态测试二进制中启用
`static-link` feature，避免相同 ABI entry symbol 冲突。proxy lib path 直接指向真实 sample source，AOT digest 输入不再
来自复制品。

`project_runtime_aot_digest` 已成为 SDK canonical owner，Tower、Complex、Switch 和 engine assembly 使用相同 canonical
JSON key ordering。production native module identity 直接读取 project manifest、RuntimeModule Cargo manifest/lock/Rust
source、ABI/SDK digest 和 toolchain identity，不再借用 `ProjectEditorCompositionIdentity`，因此 EditorShell/engine
implementation-only 变化不会使项目 DLL stale。

旧 `project_editor_composition_production_gate` acceptance 已删除并从 test module 移除；普通 production installer 中的
composition preparer、candidate process/workspace/exit adapter 已删除。历史 282 资格化仍使用的 identity helper 仅保留在
`cfg(test)`，没有恢复为 normal open fallback。显式 legacy candidate seam 若由历史测试调用，首次 present 后也改走标准
`OpenProject` command，不再调用专用 verified handoff open 方法。

### 21.3 最终定向验证

```text
cargo test -p project_runtime_sdk --lib -- --nocapture
  -> 6 passed, 0 failed

cargo test -p engine_runtime project_runtime_native_adapter --lib -- --nocapture
  -> 2 passed, 0 failed

cargo check -p complex_shooter_player -p switch_puzzle_player
  -> passed

cargo test -p editor_core project_runtime_preparation --lib -- --nocapture
  -> fail-closed 与 stale completion 2 passed
  -> authoring-first 首次因 BuiltInEmpty 测试错误地 unwrap ProjectRust-only Play blocker 而失败；见 21.4

cargo test -p editor_core project_open_authoring_ready_before_runtime_module --lib -- --nocapture
  -> 1 passed, 0 failed（最终测试源码）

cargo test -p editor_window_winit project_runtime_preparation_cancel_joins_owned_worker --lib -- --nocapture --test-threads=1
  -> 1 passed, 0 failed

cargo test -p editor_window_winit stable_editor_project_runtime_cutover --lib -- --nocapture --test-threads=1
  -> 1 passed, 0 failed

cargo check -p editor_window_winit --features real-window
  -> passed；只编译真实 production composition，未打开窗口

cargo test --manifest-path ..\samples\complex_shooter_project\RuntimeModule\Cargo.toml --lib -- --nocapture
  -> 3 passed, 0 failed

cargo test --manifest-path ..\samples\switch_puzzle_project\RuntimeModule\Cargo.toml --lib -- --nocapture
  -> 3 passed, 0 failed

cargo test -p project_e2e_gate project_runtime_native_module_consumers --lib -- --nocapture --test-threads=1
  -> 1 passed, 0 failed；72 filtered out；test body 13.87s

cargo test -p editor_core project_runtime_native_module_invalidation_matrix --lib -- --nocapture
  -> 1 passed, 0 failed

cargo test -p project_e2e_gate project_runtime_session_project_module_negative_matrix --lib -- --nocapture --test-threads=1
  -> 1 passed, 0 failed（在 consumer/source 未再变化时复用本 Window 前段有效证据）

cargo fmt --check
git diff --check
  -> passed；diff check 仅报告工作树既有 LF/CRLF 提示，无 whitespace error

静态扫描
  -> 三个 sample RuntimeModule 与 Complex/Switch workspace proxy 均无 engine_runtime/engine_input/editor 依赖
  -> 普通 Editor 中旧 preparer/handoff/receipt/adapter symbol 扫描为 0
```

### 21.4 失败、重跑与清理

```text
1. legacy cleanup 后首次 no-run compile 准确暴露旧 production preparer trait import；删除无消费者 adapter，修复 native
   identity owner 后通过。
2. 两次定向编译 shell deadline 在 test verdict 前到期；首组 rustc 完成增量产物，第二组等待同一 target lock 的本轮
   Cargo 子进程被终止。随后同一 target 1.4s 取得 no-run exit 0。归类为 compile cost，不是产品失败。
3. real-window compile 暴露首次 present 后仍调用已删除的专用 handoff open；改走标准 OpenProject command 后通过，
   没有恢复旧方法或 fallback。
4. authoring-first 测试使用 BuiltInEmpty 项目却读取 ProjectRust-only Play blocker；改为直接断言 authoring session 与
   exact Preparing ticket。第二次只因 Windows 删除仍由 session 持有的临时目录失败；删除前显式 drop session 后单项通过。
   产品状态机未因这两个测试问题修改。
5. cancel worker 测试首次 shell deadline 在 verdict 前到期且无残留 Cargo/Rustc；同一 target 重跑 2.5s 通过。
6. 两个失败测试遗留的系统 TEMP 直属、非 reparse `aife-authoring-first-*` 目录已逐项验证。Remove-Item 被宿主策略
   拒绝后，对相同 literal path 使用 Directory.Delete fallback，最终均 removed；本轮 Cargo/Rustc 进程为 0。
```

### 21.5 边界与下一步

```text
未执行：Window E / Gate H、完整 affected crate suites、repository-external run root、source-built Editor candidate、
  真实 Windows first-open/Play/Stop、Local CI、production/安装态 replacement、真实用户 cache/config、视觉矩阵
未修改：production/安装态 Editor/Player/MCP、真实配置、真实 Tower/native-module cache
文档状态：不生成阶段完成记录、不更新 00/49/54/README 索引、不归档；292 继续占用当前槽
下一步：等待用户单独授权 Window E / Gate H
```

## 22. Window E / Gate H 完成证据（2026-08-16）

### 22.1 最终受影响回归

Gate H 使用 `G:\Aife292Runs\20260816-143623` 的独立 `CARGO_TARGET_DIR`。最终通过结果：

```text
project_runtime_abi: 4 passed
project_runtime_sdk: 6 passed
engine_runtime: 563 passed
editor_core: 693 passed; 2 ignored
editor_window_winit: 189 passed; 5 ignored
project_e2e_gate: 71 passed; 2 ignored
```

`editor_core` 前序尝试暴露测试并发/fixture side effect 后改为串行并取得全绿；`project_e2e_gate` 首次完整
suite 在宿主 deadline 前未形成 verdict，保留日志后在同一 target 一次重跑取得全绿。后续仅修改
`project_e2e_gate` Tower fixture 的 run-owned artifact retention 与 production identity 计算，不修改产品源码、
candidate 或 ABI/SDK。按用户明确范围只重跑该单项 Tower E2E：`1 passed; 72 filtered out`，未再运行完整 E2E。

controlled delayed-build claim 由最终 `editor_window_winit` suite 中
`stable_editor_project_runtime_cutover_project_open_authoring_remains_responsive_while_native_module_builds`
证明：build 被测试 owner 阻塞期间 authoring 已 ready，连续推进 8 帧，Play 以
`project_runtime.preparation_pending` fail-closed；释放 build 后同一 Editor 中 Play 变为 enabled。

### 22.2 Retained exact artifact 与真实 source-built 场景

单项 E2E 新增 opt-in `AIFE_E2E_TOWER_NATIVE_RETAIN_ROOT`；未设置时仍删除临时 root，设置时要求绝对且
不存在的 run-owned root并输出 retained artifact 路径。本轮 retained artifact identity 为：

```text
identity: sha256:65029b9650706621ebc15ba5c02f4245e040a5bf617df01feb3438abecf81303
DLL: sha256:201bbe5e1345adfe81df44faabd810323fc5ee51c30bd6b8fdba19838588777c
descriptor: sha256:2a26d6b93ae50ccd3f5e34dc47420e3c09d15b532ba3efa15e084a8d8497b7a4
build report: sha256:9199c43daf2851569949f2343669cdb8ad37bdaf84acda672b1af2459cf015be
ABI: sha256:7751ec69594ec86ab955041bbe9479a00618141f76396d418a054941074ff3c6
SDK: sha256:23183117f29f8fdb5e7985ef33c7b91e27e7d5cf59073f66fb33392c9887f565
```

artifact 已复制到 fresh root
`G:\Aife292Runs\gate-h-final-20260816-190006\normal-cache` 并复核 descriptor、seal 与文件哈希。真实场景
复用未重建的 candidate
`G:\Aife292Runs\20260816-143623\cargo-target\debug\editor_ui_authority.exe`，SHA-256 为
`9baa24f64e10c86487a18a6a47b79b6fdd2a76e41b8f75dc064ed62784d6cd12`。

Tower Open / trust approve / Play / runtime active / frame / Stop 共 8/8 steps passed，production authority
耗时 9093 ms，runtime 到 frame 27；Cargo spawn 为 0，compilation cache 文件为 0，normal cache 始终只有
该 exact key，退出后 Cargo/rustc/Editor 进程均为 0。结构化报告：
`G:\Aife292Runs\gate-h-final-20260816-190006\evidence\gate-h-summary.json`。

第一次 retained 尝试因 E2E 使用非 production manifest/dependency/toolchain identity 而触发 normal build，
已明确标记为不接受；失败 root 保留用于首因追溯。修复只让测试调用 production trust inspection 与真实
`rustc --version --verbose` identity，没有修改 loader/cache production owner。

外层 authority shell 因 candidate 未包含 capture surface 且 wrapper 检查本 Gate 未申请的全局 widget
reachability 而 exit 1；其中 production authority 场景自身为 `passed 8/8`。Gate H 不要求 screenshot、capture
或完整 reachability，因此该外层诊断不纳入 Gate 声明，也不被隐藏为成功。

### 22.3 边界、清理与完成判断

```text
未运行：Local CI、完整视觉矩阵、第二次完整 E2E
未修改：production/installed Editor、Player、MCP、其它安装态二进制、真实配置、真实用户 cache
candidate：复用既有 source-built candidate，未为 final scenario 重建
external state：仅保留 run-owned roots/evidence/artifact；owned processes = 0
installed availability：未执行；production Editor 尚不声明包含 292
Gate H：passed；292 source construction complete
```

292 完成记录已生成，入口已同步，本文归档到 `施工文档/已完成/`。下一步只能单独讨论/生成
post-292 Production Editor Consistency Mini，不得从本文授权直接替换 production Editor。
