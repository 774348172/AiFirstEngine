# 242-当前可自动化施工文档-Project RuntimeModule / Generic Runtime Decoupling + Second Project Gate v1

> 状态：已完成并归档。  
> 正式方案：`../../242-Project-RuntimeModule-Generic-Runtime-Decoupling-Second-Project-Gate-v1方案.md`。  
> 方案选择：用户已确认方案 C，共享 `ProjectRuntimeBootstrap`、Editor/Dev 静态 linked set、导出 singleton wrapper。  
> 方案审查：242 已对照两份 5.6 审查、195/196、当前源码和 Unreal/Bevy/Unity/Cargo 官方资料完成审查并回填。  
> 执行规则：严格按 Gate A -> B -> C -> D -> E -> F 串行执行；Gate 失败先修当前 Gate，不跨 Gate 堆积问题。

## 1. 施工目标

关闭 CQ-01：

```text
engine_runtime / runtime_player_winit / runtime_cli / editor_core production
不再拥有 Complex Shooter 专用 rule、producer、binding、input fallback 或 validation 语义。

RuntimePackage v2 声明 required project module identity。
构建期静态链接 Adapter，启动时由共享 Bootstrap exact bind。
Editor GameView、headless/real-window Player、Desktop/Release 使用同一 bind receipt。
第二个语义不同项目通过真实 Editor/export/player process Gate。
```

完成必须证明：

```text
aife-project.v2 / runtime-package.v2 required schema 生效。
ProjectRuntimeModule 只有 descriptor/install 小 interface。
共享 Bootstrap 独占 descriptor、artifact、producer、input 与 diagnostics 装配。
Complex Shooter implementation 位于 project-owned Adapter。
Switch Puzzle 是第二个真实 Adapter。
错误 module/ABI/digest/artifact/input 在 hydration/首帧前 fail closed。
导出 Player 是 typed matching artifact，不再默认复制无身份 generic CLI。
default/all-features workspace 回归通过。
```

## 2. 施工前基线

```text
242 正式方案已由用户确认并完成方案审查。
施工文档/当前/ 在生成本文前为空。
240 中 CQ-01 讨论状态=已完成，施工状态=未施工。
239/241 与用户既有未提交改动必须保留，不 reset/checkout/revert。
```

当前关键证据：

```text
runtime_player_winit 固定五条 shooter rules 与两个 producer 创建点。
engine_runtime::aui 持有两个 ComplexShooter producer。
editor_gameview_play 固定 producer 且使用 EngineHostLoop::new。
runtime_package/builder/player/editor 存在 gameplay_default fallback。
RuntimePackageManifest.project 只有 name/version。
Desktop/Release 默认 player 是 target/debug/ai_engine_runtime_cli.exe。
```

## 3. 固定范围

### 3.1 必须交付

```text
aife-project.v2 typed runtime module build spec + migration。
runtime-package.v2 RuntimeProjectModuleRef。
ProjectRuntimeModuleDescriptor / ProjectRuntimeModule / private registration。
LinkedProjectRuntimeSet / ProjectRuntimeBootstrap / BoundProjectRuntime / bind receipt。
artifact-aware RuleModuleRegistry。
Complex Shooter project Adapter。
explicit project InputMapping，没有 production gameplay fallback。
Editor/Player/CLI shared bind chain。
typed ProjectPlayerArtifact / descriptor verification。
Switch Puzzle project Adapter/assets/player fixture。
Core source/dependency hygiene Gate。
```

### 3.2 禁止扩入

```text
不新增运行时 Logic Ownership Router / Architecture Guard。
不新增动态 VM、WASM gameplay、native dylib load/hot replace。
不重写 ECS、Renderer、AUI Runtime、Projection 或 RuntimePackage assembler。
不新增第二 rule runner、第二 export pipeline、第二 process runner。
不处理 INC-02、CQ-06、CQ-07、CQ-08。
不允许兼容失败回退 Complex Shooter module/input/producer。
```

## 4. 跨 Gate 固定合同

### 4.1 Project source

```text
aife-project.v2.runtimeModule =
  moduleId + interfaceVersion + cargoManifest + cargoPackage + playerBinary。

cargoManifest 必须是 241 ProjectRelativePath。
RuntimePackage、linked set、Adapter、Player artifact 从同一 typed spec 派生。
```

### 4.2 Runtime package

```text
runtime-package.v2.project =
  projectId + name + version + runtimeModule。

runtimeModule = moduleId + interfaceVersion + aotContentDigest。
v1 只返回 rebuild-required，不隐式补 module。
```

### 4.3 Module seam

```text
ProjectRuntimeModule::descriptor
ProjectRuntimeModule::install(private registration)

registration 只允许：
  register_rust_aot_rule(ruleId, artifactId, fn)
  set_ui_state_producer_factory(factory)
```

### 4.4 Bootstrap

```text
ProjectRuntimeBootstrap::bind(package, linked set)
  -> exact module match
  -> artifact-aware rule registration
  -> fresh producer
  -> package-owned default input
  -> ProjectLogicRunner
  -> BoundProjectRuntime + receipt
```

### 4.5 Input

```text
交互项目显式 InputMappingAsset。
无输入项目显式 input.none。
缺 input/default id -> project_runtime.default_input_missing。
```

### 4.6 Export

```text
ProjectPlayerArtifact = executable + embedded descriptor + source executable hash + build report。
stage 前 descriptor exact match。
stamp/publish 后 final executable 继续走既有 hash/PE contract。
```

## 5. Gate A - Schema / ProjectRuntimeModule Foundation

### 5.1 施工

```text
升级 ProjectManifest v2 并实现 runtimeModule typed spec/migration。
升级 RuntimePackageManifest v2 和 RuntimeProjectInfo。
builder/loader/assembler 写入 required descriptor。
实现 ProjectRuntimeModule module、registration、linked set、Bootstrap、diagnostics、receipt。
RuleModuleRegistry 注册并校验 ruleId + artifactId。
实现 explicit empty InputMapping constructor；v2 缺 input fail closed。
```

### 5.2 阻断测试

```powershell
cargo test -p engine_runtime project_runtime_module -- --nocapture
cargo test -p engine_runtime runtime_package
cargo test -p engine_runtime rule_registry
cargo test -p editor_core project_launcher
cargo test -p editor_core project_runtime_package_assembler
```

### 5.3 验收

```text
descriptor exact/missing/mismatch/duplicate matrix 通过。
rule artifact mismatch 不能只凭 ruleId 通过。
v1 rebuild-required 与 v2 round trip 通过。
Project manifest -> RuntimePackage -> Adapter descriptor 同源。
Bootstrap 失败不返回半成品。
```

## 6. Gate B - Complex Shooter Adapter Extraction

### 6.1 施工

```text
建立 project-owned Complex Shooter RuntimeModule crate。
迁移五条 Rust AOT rule 和 helpers。
迁移 ComplexShooterRuntimeUiStateProducer / sample producer / binding helpers。
迁移 shooter validation wrapper 和 input template/default。
project_e2e_gate 改为从 Adapter/Bootstrap 取 runner/producer。
engine_runtime/runtime_player_winit 删除 shooter public implementation。
```

### 6.2 阻断测试

```powershell
cargo test -p complex_shooter_project_runtime -- --nocapture
cargo test -p project_e2e_gate gameplay_rule_runtime -- --nocapture
cargo test -p project_e2e_gate project_rule_driven_ui_state -- --nocapture
cargo test -p project_e2e_gate complex_shooter -- --nocapture
```

### 6.3 验收

```text
Complex Shooter rule/component/UI 行为不回退。
Core/Player production 不再定义或创建具体 shooter module。
project Adapter 单向依赖 engine_runtime/engine_input。
engine_runtime/runtime_player_winit 不依赖 project Adapter。
```

## 7. Gate C - Shared Editor / Player Bootstrap

### 7.1 施工

```text
runtime_player_winit headless/real-window 接收 linked set 并调用 Bootstrap。
runtime_cli generic library 透传 linked set，不选择项目。
EditorRuntimePlayInstance 使用 Bootstrap binding，不固定 producer/EngineHostLoop::new。
删除 RuntimePackage/player/editor/authoring production gameplay_default fallback。
统一 bind receipt 和 diagnostics 映射。
```

### 7.2 阻断测试

```powershell
cargo test -p runtime_player_winit project_runtime -- --nocapture
cargo test -p runtime_cli project_runtime -- --nocapture
cargo test -p editor_core editor_gameview -- --nocapture
cargo test -p project_e2e_gate project_runtime_bootstrap -- --nocapture
```

### 7.3 验收

```text
Editor/headless/windowed 同 moduleId/interface/digest/producer/input/rule counts。
Editor GameView 真实执行项目 rule。
每个 bind 创建独立 producer state。
未知 module 给 rebuild/relaunch next action，不动态加载。
```

## 8. Gate D - Typed Project Player Artifact / Export

### 8.1 施工

```text
实现 ProjectPlayerArtifact / descriptor record / descriptor query。
project-specific singleton player wrapper 调用 generic runtime_cli。
Desktop/Release 请求 typed artifact，不默认复制无身份 CLI。
stage 前 exact descriptor verification。
报告记录 build/bind descriptor receipt，保持 237/239/241 contract。
```

### 8.2 阻断测试

```powershell
cargo test -p editor_core desktop_export -- --nocapture
cargo test -p editor_core release_package -- --nocapture
cargo test -p runtime_cli project_module_descriptor -- --nocapture
cargo test -p project_e2e_gate project_player_artifact -- --nocapture
cargo test -p project_e2e_gate release_package -- --nocapture
```

### 8.3 验收

```text
wrong executable/module/digest 在 stage 前失败。
Complex Shooter exported process 使用 singleton Adapter。
最终 PE/resource/hash verification 不回退。
外部 output 仍要求 ExplicitExportOutput。
```

## 9. Gate E - Switch Puzzle Second Project Full Gate

### 9.1 施工

```text
建立 Switch Puzzle project manifest/runtime module/rules/components/input/AUI/assets。
rule.toggle-switch / rule.evaluate-puzzle。
puzzle.switchState / puzzle.sessionState。
puzzle.moves_text / puzzle.solved。
action.toggle-switch -> keyboard/Enter。
Editor linked set 同时包含 Shooter/Puzzle；exported Puzzle Player 为 singleton。
实现 project-runtime-module-second-project-report.v1 validation-only evidence。
```

### 9.2 正向 Gate

```text
Enter -> action.toggle-switch -> rule invoked -> component changed
-> producer output -> AUI binding -> Editor GameView report。

Puzzle Desktop/Release export -> real child process -> matching receipt -> exit 0。
Shooter 既有完整 Gate 继续通过。
```

### 9.3 否定 Gate

```text
Shooter package + Puzzle Adapter。
Puzzle package + Shooter Adapter。
interface version mutation。
AOT digest mutation。
rule artifact mutation。
duplicate/missing module。
missing/default input mutation。
```

### 9.4 阻断测试

```powershell
cargo test -p switch_puzzle_project_runtime -- --nocapture
cargo test -p project_e2e_gate second_project_runtime -- --nocapture
cargo test -p project_e2e_gate project_runtime_module -- --nocapture
cargo test -p project_e2e_gate exported_player -- --nocapture
```

## 10. Gate F - Inventory / Full Regression / Closure

### 10.1 Source / dependency inventory

```text
复扫 engine_runtime/runtime_player_winit/runtime_cli/editor_core production：
  ComplexShooter* producer
  五条固定 rule id
  shooter component/binding/action
  gameplay_default production fallback
  generic CLI default export path

cargo tree 验证 Core/Player 不依赖 project Adapter。
所有命中分类为 generic fixture / project Adapter / migration leftover；production leftover 阻断。
```

### 10.2 整体回归

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
```

### 10.3 完成同步

```text
生成 阶段完成记录/2026-07-11-Project-RuntimeModule-Generic-Runtime-Decoupling-Second-Project-Gate-v1/00-总览.md。
更新 240：CQ-01 当前状态=已关闭，施工状态=已完成。
更新 00-文档地图、00-阅读顺序、49、54。
更新 施工文档/README、阶段完成记录/README。
把本文从 当前/ 移到 已完成/。
```

## 11. Diagnostic / report 固定集合

```text
project_runtime.project_manifest_v1_migration_required
project_runtime.package_v1_rebuild_required
project_runtime.module_ref_missing
project_runtime.module_not_linked
project_runtime.duplicate_linked_module_id
project_runtime.module_id_mismatch
project_runtime.interface_version_mismatch
project_runtime.aot_digest_mismatch
project_runtime.registration_failed
project_runtime.duplicate_rule
project_runtime.missing_linked_rule
project_runtime.rule_artifact_mismatch
project_runtime.unsupported_rule_module_kind
project_runtime.unsupported_rule_executor
project_runtime.ui_producer_missing
project_runtime.default_input_missing
project_runtime.player_artifact_mismatch
project_runtime.bind_failed

project-runtime-bind-receipt.v1
project-player-module-descriptor.v1
project-runtime-module-second-project-report.v1
```

## 12. 预计涉及文件

```text
rust/Cargo.toml
rust/Cargo.lock

rust/crates/engine_runtime/src/project_runtime_module.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/rule_registry.rs
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/lib.rs

rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/editor_preview_package.rs
rust/crates/editor_core/src/desktop_export.rs
rust/crates/editor_core/src/release_package.rs
rust/crates/editor_core/src/services/build_service.rs

rust/crates/runtime_player_winit/src/lib.rs
rust/crates/runtime_cli/src/lib.rs
rust/crates/runtime_cli/src/main.rs

rust/project_modules/complex_shooter/**
rust/project_modules/switch_puzzle/**
samples/complex_shooter_project/**
samples/switch_puzzle_project/**

rust/crates/project_e2e_gate/src/project_runtime_module.rs
rust/crates/project_e2e_gate/src/second_project_runtime.rs
rust/crates/project_e2e_gate/src/bin/*
```

列表外命中必须说明所有权与理由，不能静默扩范围。

## 13. 工作树规则

```text
保留所有既有用户/239/241 未提交改动。
不 reset/checkout/revert unrelated changes。
测试生成物只在正式 Gate 要求时更新。
实现与 242 冲突时先回填正式方案/本文，不改成更小的假完成。
不启动 INC-02 或其它施工。
```

## 14. 施工文档自审

### 14.1 与正式方案一致

通过。Gate A-F 逐项覆盖 242 的 schema、深 module、项目迁移、共享 host、typed artifact、第二项目和完成闭环，没有新增动态 module 系统。

### 14.2 审查输入处理

通过。CQ-01 必须修改进入 Gate A-E；第二项目/Core hygiene/真实进程成为阻断条件；199/217/218/229/237/239/241 只继承合同；其它 5.6 项明确不扩入。

### 14.3 是否形成深 module

通过。caller 只跨 Bootstrap；Adapter 只 descriptor/install；registry、artifact、producer、input、diagnostics 隐藏在 implementation。Shooter/Puzzle 两个 Adapter 使 seam 真实。

### 14.4 是否可能演化为 Router

已约束。Editor set 仅启动前 exact lookup；export singleton；禁止 alias/fallback/per-frame/per-rule routing。

### 14.5 测试是否覆盖完整目标

通过。每个 Gate 有定向命令；第二项目覆盖 Editor、Desktop/Release、真实 child process 与交叉否定矩阵；Gate F 执行双 workspace。

### 14.6 是否可以直接施工

可以。当前目录只有本文一份施工文档，正式方案已审查，自审无阻断。结论：`通过，开始 Gate A；当前 Gate 失败时停止并修复，不跨 Gate。`
