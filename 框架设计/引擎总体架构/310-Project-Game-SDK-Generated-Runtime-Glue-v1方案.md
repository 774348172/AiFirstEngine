# 310 Project Game SDK + Generated Runtime Glue v1 方案

> 文档性质：权威架构 R2 正式子方案；方案 C，经最新结构收敛后由用户确认。  
> 上级 authority：`00-AI-First-Game-Engine-权威产品需求-v1.md`、`00-AI-First-Game-Engine-权威架构设计-v1.md`。  
> 前置完成：309-A 至 309-F 已完成并归档；Headless `EngineToolProvider`、`AuthoringProjectContext`、operation-bound snapshot lease 与 `GameProjectCompiler` 必要子集已经存在。  
> 当前状态：Gate A-D 已完成并归档，能力状态为 `source_integration_passed`。  
> 正式选择：方案 C，采用小型项目侧 Game SDK 与 Compiler 内部 Generated Runtime Glue；不建立独立 Glue Module、Host、Provider、Registry 或第二个 Compiler。  
> 设计日期：2026-09-02。

## 0. 一句话决定

让 AI 只编写普通 Rust 游戏逻辑和一处 typed project registration；现有
`GameProjectCompiler` 从 operation-owned immutable `SourceView` 自动生成并编译
`ProjectRuntimeAbi` Adapter，隐藏 descriptor、capability bits、session handles、FFI callbacks、
function table、AOT digest、Player/Editor/Android registration 和底层 SDK 定位。

对 AI 不新增 `generate_glue`、`register_module` 或固定跨工具菜谱。现有
`engine_project_run` / `engine_project_build` 等具体 Engine tools 在内部自动使用该能力。

## 1. 要解决的根本问题

现有底层基础是正确的，但项目侧 Interface 仍然过浅。

### 1.1 已有稳定基础

`rust/crates/project_runtime_abi` 已提供：

- versioned narrow C ABI；
- opaque handles；
- caller-owned byte buffers；
- `ProjectRuntimeApi` function table；
- capability bits；
- ABI schema 与 digest；
- 不依赖 Engine 或 Editor crate 的稳定二进制 seam。

`rust/crates/project_runtime_sdk` 已提供：

- stable serialized request/response types；
- JSON call helpers；
- panic containment；
- stateful output capacity floor；
- contract digest；
- WorldRead、deferred mutation、AUI、fixed update、observation 等 wire contract。

292 已证明稳定 Editor host 可以加载 native project module；309-F 已证明无 Editor 的真实
Codex Agent Tool Loop 可以完成 inspect、mutate、run、build 和 delivery verify。本文继承这些事实，
不重新建设 ABI、Provider、Context、Compiler 或 Player pipeline。

### 1.2 当前项目仍手写运行时胶水

三个真实样例的 `RuntimeModule/src/lib.rs` 当前规模为：

| 项目 | 主文件行数 | 手写 extern/entry 数量 | 重复基础设施 |
| --- | ---: | ---: | --- |
| Complex Shooter | 993 | 10 | descriptor、session、FFI、capability、API table、digest、WorldRead Adapter |
| Switch Puzzle | 849 | 10 | 同上 |
| Tower Defense | 485 | 10 | 同上；玩法已拆分，外层 ABI 胶水仍重复 |

三个项目共同手写：

- `descriptor`；
- `create_session` / `destroy_session` / `session_id`；
- `invoke_rule`；
- `handle_aui_actions`；
- `fixed_update`；
- `resolve_ui_state`；
- `observe`；
- `aife_project_runtime_entry_v1`；
- `ProjectRuntimeApi` 全量字段和 capability bitset；
- request decode、response encode、panic/buffer error mapping；
- session handle store 和 generation 校验；
- AOT source enumeration 和 digest；
- repository-relative `project_runtime_abi` / `project_runtime_sdk` Cargo paths。

项目 manifest、Rust 常量、Player 和测试还重复维护 module id、interface version、Cargo package、
player binary 与 descriptor identity。当前依赖“重复声明 + 测试防漂移”，而不是单一事实来源。

### 1.3 对 AI 的直接伤害

AI 为新增一段玩法逻辑，需要同时理解：

- 游戏语义；
- Rust 项目结构；
- unsafe C ABI；
- function pointer 和 buffer lifetime；
- session handle 正确性；
- manifest、Cargo、descriptor 与 Player 同步；
- Headless、Editor、Windows、Android 的不同组合路径。

这些知识不能提高游戏质量，只会扩大上下文、修改面和失败面。它们应成为深 Implementation，
而不是项目作者和 AI 必须学习的 Interface。

## 2. 正式方案 C 与结构收敛

### 2.1 不建设三个新的深 Module

整个长期架构仍然有三个不同变化原因的深 Module：

1. `EngineToolProvider`：面对 Host Agent，拥有 ToolDefinition、host call、operation 与 result projection；
2. `GameProjectCompiler`：面对项目 revision，拥有 prepare/run/build、增量生产和 artifact lineage；
3. `Project Game SDK`：面对项目代码，拥有稳定、低样板的游戏编程 Interface。

它们不是本方案新建的三层。前两者已经由 309-F 建立。本方案只新增第三者的正式项目侧
Interface，并在现有 Compiler Implementation 内增加 glue generation。

### 2.2 Generated Runtime Glue 不是独立 Module

Generated Runtime Glue 是实现 `ProjectRuntimeAbi` seam 的派生 Adapter artifact。它：

- 没有 AI 可见 Interface；
- 没有独立生命周期；
- 没有项目 authority；
- 没有 ToolDefinition；
- 没有独立 cache owner；
- 没有独立 host 或进程；
- 不接收自然语言目标；
- 不选择下一步工具；
- 不成为第二个 Compiler。

删除“独立 Glue Module”概念后，生成行为仍完整存在于 Compiler 内，因此独立 Module 不具备
删除测试所要求的深度，必须拒绝。

### 2.3 本方案实际新增量

```text
一个新增外部 Interface
  = AI / project-authored Rust 直接消费的 Project Game SDK

一个既有 Module 内部 Implementation 增量
  = GameProjectCompiler 中的 deterministic runtime glue generation

一个派生产物角色
  = 实现 ProjectRuntimeAbi 的 Generated Runtime Glue Adapter
```

## 3. 设计目标

本方案必须实现：

1. AI 写游戏时不直接依赖或导入 `project_runtime_abi`。
2. AI 不手写 `unsafe extern "C"` callbacks、`ProjectRuntimeApi`、descriptor、capability bitset、
   session handle store 或 AOT digest source list。
3. 项目侧 Interface 使用普通 Rust 类型、typed context 和少量显式 registration，默认行为低样板。
4. module/build identity、capability/handler registration 与派生 descriptor 各有唯一 owner。
5. Compiler 从 operation-bound immutable snapshot lease 生成 glue，不读取 Editor draft 或未经 lease
   绑定的 live source。
6. 同一 source revision、SDK contract、generator version、ABI contract 与 target profile 产生确定的
   generation identity 和可重建输出。
7. generated compile error 和 contract error 映射回 project-authored source path、symbol 和 declaration。
8. Headless run、Optional Editor Preview、Windows Player 和 Android Player 使用同一 project logic owner、
   SDK contract 与 ABI Adapter 语义。
9. 现有 concrete `engine_*` tools 自动消费该能力，Agent 不编排生成内部步骤。
10. Complex Shooter、Switch Puzzle、Tower Defense 三个不同复杂度项目不再维护重复 ABI pipeline。
11. 当前 gameplay、AUI、observation、RuntimePackage、Player 和 delivery 行为保持兼容，不借迁移重写玩法。
12. 项目局部源码修改只失效项目逻辑及其真实下游，不因 Host Adapter 或 Editor UI 变化重建。

## 4. 非目标与禁止扩大

本文不负责：

- 新建脚本语言、GDScript、Blueprint clone 或通用 Gameplay DSL；
- 生成玩法算法、敌人 AI、UI 设计或项目专用代码；
- 把 Shooter、Puzzle、Tower 的语义写入 Engine Core；
- 新建 `RuntimeGlueHost`、`GlueProvider`、`GlueRegistry`、`GlueDaemon` 或独立生成进程；
- 对 Agent 暴露 `engine_generate_glue`、`engine_register_module`、`engine_build_abi_table`；
- 建立第二个 Agent、Planner、Workflow 或固定 inspect -> generate -> register 菜谱；
- 完成权威架构 R3 的全部 incremental Compiler、全 target toolchain readiness 或公开 `check` 工具；
- 完成 Semantic Runtime Observation + Outcome Acceptance；
- 扩展 Renderer、Physics、Audio、Animation、Camera 或 Asset 能力范围；
- 重写现有 RuntimePackage、Assembler、Player、Desktop Export 或 Android exporter；
- 要求 Editor 存在，或恢复 Editor-hosted Gateway；
- 修改 production、真实 Codex 配置、安装态 Provider、Android 工具链或 Tower P1-2；
- 由本文直接授权代码施工、测试、Local CI、真实窗口或真实设备操作。

## 5. 成熟实现参考与采纳边界

### 5.1 Godot

Godot GDExtension 使用稳定 native ABI；`godot-cpp/binding_generator.py` 从
`extension_api.json` 生成 engine bindings。项目仍需 `InitObject`、initializer/terminator 和
`GDREGISTER_CLASS` 等手工注册。

采纳：稳定 ABI 与 generated bindings 分离。

拒绝：把 `register_types.cpp` 式手工注册负担留给项目，或扩大为 Godot Object/reflection surface。

参考：

- <https://docs.godotengine.org/en/stable/tutorials/scripting/cpp/gdextension_cpp_example.html>
- <https://github.com/godotengine/godot-cpp/blob/master/binding_generator.py>

### 5.2 Unity

Unity assembly definition 将脚本分成依赖明确的 compilation units；`EditorCompilation` 与
`AssemblyBuilder` 根据脚本、引用和编译选项派生构建计划。用户代码不手写 native function table。

采纳：依赖范围、内容失效与构建输入派生。

拒绝：Managed Runtime、Editor authority、reflection lifecycle 和 IL2CPP 架构。

参考：

- <https://docs.unity3d.com/6000.0/Documentation/Manual/assembly-definition-files.html>
- <https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/Scripting/ScriptCompilation/EditorCompilation.cs>

### 5.3 Unreal

Unreal Header Tool 从带标注声明生成 reflection/registration code，再由 Unreal Build Tool 编译。
生成输出支持 write-on-change、external dependency invalidation 和 generated-code drift verification。

采纳：一处权威声明、deterministic generation、write-on-change、source mapping 和 drift gate。

拒绝：自定义 C++ parser、大型 reflection system、宏 DSL 和 Editor-centric object model。

参考：

- <https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-header-tool-for-unreal-engine>
- <https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-build-tool-in-unreal-engine>

### 5.4 Bevy

Bevy 使用小型 `Plugin::build(&mut App)` Interface 和 derive macros；`App::add_plugins` 负责组合与
lifecycle，`#[derive(Component)]` 在编译期生成类型实现。

采纳：普通 Rust、typed registration、小型 Interface、compile-time diagnostics。

拒绝：要求项目手工注册所有底层 system、依赖不稳定 Rust dynamic ABI，或让项目链接完整 Engine internals。

参考：

- <https://github.com/bevyengine/bevy/blob/main/crates/bevy_app/src/plugin.rs>
- <https://github.com/bevyengine/bevy/blob/main/crates/bevy_ecs/macros/src/lib.rs>

### 5.5 综合结论

```text
Bevy式项目编程体验
+ Unreal式确定性生成和漂移验证
+ Unity式依赖范围与增量失效
+ Godot式稳定运行时ABI
= 本方案 C
```

不采纳任何成熟引擎的 Editor authority、大型反射系统或手工模块注册负担。

## 6. 正式目标拓扑

```text
User
  -> Host AI Agent / existing Agent Tool Loop
      |-- read_file / write_file / apply_patch / run_command
      |-- existing concrete engine_project_* / engine_delivery_* tools
      v
  existing EngineToolProvider
      v
  existing AuthoringProjectContext
      |-- refresh canonical source
      |-- operation-bound immutable snapshot lease
      v
  existing GameProjectCompiler
      |-- validate Project Game SDK declaration
      |-- generate derived Runtime Glue Adapter
      |-- compile project logic + generated glue
      |-- assemble RuntimePackage / stage Player
      v
  ProjectRuntimeAbi seam
      v
  Rust Native Runtime
      |-- Headless
      |-- Optional Editor Preview Adapter
      |-- Windows Player
      `-- Android Player
```

项目代码从另一个方向消费同一能力：

```text
AI-authored Rust gameplay
  -> Project Game SDK Interface
      -> typed ProjectGameDefinition / session handlers / safe contexts
          -> Compiler-generated ProjectRuntimeAbi Adapter
```

Project Game SDK 不是 Agent tool；EngineToolProvider 负责“直接调用引擎”，Project Game SDK负责
“直接写游戏代码”，GameProjectCompiler负责连接二者并隐藏派生复杂度。

## 7. Module、Interface、Adapter 与 owner

| 事实或行为 | 唯一 owner | 消费者 | 明确禁止 |
| --- | --- | --- | --- |
| Agent tool projection | existing `EngineToolProvider` | Host Adapter | SDK或Glue注册Agent工具 |
| canonical source/revision | existing `AuthoringProjectContext` | Compiler、Provider、Editor Adapter | Glue读取live project tree |
| project programming contract | `Project Game SDK` | AI-authored project code | 项目直接使用raw ABI |
| module/build identity | canonical project manifest | Compiler、registration validator | Rust常量重复维护同一身份 |
| gameplay capability/handler registration | project-authored typed registration | SDK validator、Glue generator | manifest和Rust各维护一份capability list |
| ABI/wire contract | existing `ProjectRuntimeAbi` / protocol SDK | generated Adapter、Runtime host | Agent或项目作者手写function table |
| generated glue content | existing `GameProjectCompiler` Implementation | Rust compiler、Player composition | 新建独立Glue owner/cache/tool |
| AOT/source/artifact identity | Compiler lineage owner | Provider、Runtime、delivery | 项目手写include_bytes source list |
| RuntimePackage assembly | existing Compiler internal Assembler | Runtime/Player | Glue复制Assembler |
| runtime semantic result | existing Runtime owners | run/observe/evidence | Compiler或Glue猜测游戏结果 |

## 8. Project Game SDK 外部 Interface

### 8.1 Interface 设计原则

项目侧只应学习：

1. 如何声明一个 project game definition；
2. 如何创建 project runtime session；
3. 如何实现需要的 typed handler；
4. 如何通过 safe context 读 World 和提交受控 intent；
5. 如何返回 structured result/diagnostic。

项目侧不应学习 ABI version negotiation、pointer、buffer、JSON wire format、handle store、function table、
dynamic library symbol、Player launcher 或平台注册。

### 8.2 单一 typed registration

每个 project RuntimeModule 必须有且只有一个约定的 Rust registration entry。概念形状如下，
具体 Rust 名称与泛型细节在施工文档冻结：

```rust
pub fn project_game() -> ProjectGameDefinition<ShooterSession> {
    ProjectGameDefinition::new(ShooterSession::create)
        .fixed_update(ShooterSession::fixed_update)
        .aui_actions(ShooterSession::handle_aui_actions)
        .ui_state(ShooterSession::resolve_ui_state)
        .observe(ShooterSession::observe)
        .rule("rule.player-move", ShooterSession::player_move)
}
```

这段示例冻结语义，不冻结最终语法：

- registration 必须是普通、可搜索、可类型检查的 Rust；
- handler presence 是 capability declaration 的唯一项目侧真相；
- `rule_id` 是 project-authored stable logical identity；
- rule artifact identity 由 Compiler 从 logical identity、source identity 与 contract 生成；
- module id、SDK requirement、Cargo build identity 继续来自 canonical project manifest；
- descriptor、capability bits、UI producer identity 和 AOT digest 全部派生；
- 不要求 proc-macro 才能理解 registration；可选 derive/attribute 只能减少局部样板，不能隐藏整个模块拓扑。

### 8.3 Session 与 handler 语义

v1 只覆盖现有 ABI 已具备并由三个样例真实使用的能力：

- session create/destroy；
- fixed update；
- project rule invocation；
- ordered AUI action batch；
- conditional UI state resolve；
- semantic observation publication；
- WorldRead；
- deferred mutation，包括现有 spawn/despawn contract。

未注册的 optional handler 由生成 Adapter 表现为 capability absent 或规范的 unhandled result，
不得要求项目手写 no-op callback。

### 8.4 Safe context

SDK context 至少提供受限 Interface：

```text
GameTime / frame identity
InputActionView
CollisionView
WorldRead
DeferredMutationWriter
UiBindingRequest / UiStateWriter
ObservationWriter
ProjectDiagnosticSink
EntityHandle { identity, generation }
```

禁止暴露：

- 裸 ECS world pointer；
- 裸 entity index；
- renderer/GPU handle；
- physics world internal object；
- Editor session、selection、draft 或 GameView；
- filesystem、network、process 或 arbitrary host callback；
- RuntimePackage mutable internals。

### 8.5 Project code dependency

project-authored RuntimeModule 只直接依赖 Project Game SDK 的稳定分发身份及其普通项目依赖。
它不得再通过 `../../../rust/crates/...` 直接定位 ABI/protocol crate。

SDK locator/materialization、ABI/protocol dependencies、generated crate manifest、features 与 target flags
由 Compiler拥有。物理上是否新增 crate、重组现有 `project_runtime_sdk` 模块或提供 facade，由施工设计在
不增加第二 owner 的前提下决定；本方案冻结的是外部 Interface 与依赖方向，不预先要求无收益的 crate 拆分。

## 9. Canonical declaration 与派生关系

### 9.1 Project manifest 拥有的事实

canonical `project.aife.json` 继续拥有：

- project identity；
- runtime module identity；
- project source kind；
- required Project Game SDK contract/version range；
- Cargo manifest/package 等 build locator；
- Player/build-facing logical identity；
- target-independent project settings。

不在 manifest 中重复保存 Rust handler path、function pointer、ABI capability bitset 或 generated digest。

### 9.2 Typed registration 拥有的事实

项目 Rust registration 拥有：

- session factory；
- enabled gameplay handlers；
- stable rule ids；
- handler到项目symbol的typed关联；
- handler-specific bounded configuration。

### 9.3 Compiler 派生的事实

Compiler派生：

- ABI capability bits；
- `ProjectRuntimeModuleDescriptor`；
- rule artifact ids；
- UI producer identity；
- AOT content digest；
- generated source map；
- generated crate/manifest/lock inputs；
- dynamic/static entry composition；
- target-specific Player registration；
- generation report 与 artifact lineage。

同一事实不得同时由 manifest、Rust常量、generated code 和 Player手工维护。

## 10. Generated Runtime Glue Adapter

### 10.1 生成内容

Compiler内部生成器至少产生：

- project registration binding；
- `ProjectRuntimeApi` table；
- ABI version/struct size/contract digest；
- `aife_project_runtime_entry_v1`；
- descriptor callback；
- session handle allocation、generation validation、destroy；
- typed request decode 与 response encode；
- panic containment 和 ABI status mapping；
- stateful output capacity-floor handling；
- WorldRead host Adapter；
- deferred mutation lowering；
- registered handler dispatch；
- conditional UI state与observation dispatch；
- static-link、native module、Player和Android需要的窄registration；
- project-source-to-generated-source map；
- generation manifest/report。

### 10.2 生成位置

生成物只能进入 Compiler-owned derived/cache/staging root。它：

- 不进入 canonical project source truth；
- 不由 AI 或用户手工修改；
- 不参与项目 mutation receipt；
- 不被 Editor Save 写回；
- 可以删除并从 source revision 确定性重建；
- 不因 mtime 单独失效；
- 不以 committed generated source 作为长期兼容真相。

### 10.3 确定性 identity

Generation identity 至少绑定：

```text
project canonical revision
+ selected runtime module source identity
+ Project Game SDK contract identity
+ ProjectRuntimeAbi / protocol contract identity
+ generator identity
+ target profile and relevant compile features
+ toolchain identity where artifact compatibility requires
```

相同输入必须产生相同 normalized generated content 和 generation digest。输出目录绝对路径、临时目录、
时间戳、Host session、Editor状态不得进入内容 identity。

### 10.4 Source map

每个 generated item 必须能够回溯到以下一种项目事实：

- project manifest field；
- typed registration call；
- handler symbol；
- rule id；
- SDK contract item；
- generator-owned synthetic item。

synthetic item失败时，诊断仍必须说明关联的项目 declaration 或明确标记为 Engine generator defect，
不能要求 AI 修改 generated file。

## 11. GameProjectCompiler 集成合同

### 11.1 Owner位置

生成器属于现有 `project_authoring_execution::GameProjectCompiler` 的内部 Implementation。允许用私有
子模块、内部 library 或 build helper组织，但不公开第二套 prepare/build Interface，不新增独立生命周期 owner。

### 11.2 Prepare顺序

概念流程：

```text
Context refresh
-> acquire operation-bound immutable snapshot lease
-> parse canonical project manifest
-> resolve Project Game SDK contract
-> validate typed registration/build inputs
-> compute generation dependency identity
-> cache hit validation or deterministic glue generation
-> compile project logic + glue Adapter
-> validate exported ABI/descriptor against manifest and registration
-> assemble RuntimePackageBuildInput
-> return PreparedRuntimePackage + diagnostics + lineage
```

Generator只消费 lease-owned `SourceView` 和 Compiler-owned SDK/toolchain inputs。不得在 generation 中
重新打开 live project root获取canonical source。

### 11.3 对现有Compiler Interface的影响

现有高层 Interface保持：

```text
prepare(ProjectSnapshotLease, TargetProfile) -> PreparedRuntimePackage
run(PreparedRuntimePackage, RunOptions) -> RuntimeExecutionReport
build(PreparedRuntimePackage, BuildRequest) -> BuildDeliveryReport
verify(DeliveryRef, VerifyRequest) -> DeliveryVerificationReport
```

本方案不新增公开 `generate` 步骤。完整 R3 可以以后增加正式 `inspect/check`，但本方案不得为了
glue generation预建第二套Compiler或Agent工具菜谱。

### 11.4 增量失效

至少区分：

- 仅玩法实现变化：重编项目逻辑及真实下游；
- registration变化：重生成dispatch/descriptor并重编真实下游；
- manifest build identity变化：重新解析并失效相应build node；
- SDK/ABI/generator contract变化：失效对应glue和下游artifact；
- Scene/AUI/Asset变化但RuntimeModule未变化：不得无条件重生成Rust glue；
- Host Adapter、MCP schema、Editor UI变化：不得失效项目RuntimeModule artifact。

## 12. Agent Tool Loop合同

### 12.1 AI继续直接写项目

AI可以使用普通文件工具编辑：

- project-authored Rust；
- typed registration；
- Scene、Prefab、AUI、Rule、Input、Asset metadata；
- project tests与fixtures。

下一次 Engine语义调用由Provider/Context refresh观察最新canonical bytes并绑定新revision。

### 12.2 Engine tools自动使用生成能力

现有具体工具继续是AI可见Interface，例如：

- `engine_project_inspect`；
- `engine_project_diagnostics`；
- `engine_project_run`；
- `engine_project_build`；
- `engine_delivery_verify`。

需要生成或重编时，工具在现有Provider -> Compiler调用内部自动完成。AI不得看到或维护：

- generation root；
- generated Cargo manifest；
- ABI entry symbol；
- function table；
- platform launcher source；
- generated source file edits。

### 12.3 不建立工具外菜谱

Provider结果可以返回修复类别和结构化诊断，但不得要求固定：

```text
先generate_glue
-> 再register_module
-> 再compile_adapter
-> 再run
```

AI仍自行决定何时读取、修改、运行、测试或构建。Engine隐藏自己的必要内部步骤。

### 12.4 SDK可发现性

本方案必须产出版本化、机器可读的最小SDK知识投影：

- contract identity；
- public symbol index；
- handler/context摘要；
- 最小registration示例；
- capability与限制；
- diagnostics reference。

它是未来 AI Development Pack 的输入，不在本方案中建设完整Skill、教程库或模型工作流。Provider可以在
project facts/diagnostics中返回SDK identity与reference，不能新增要求AI先调用的二级Catalog。

## 13. Consumer一致性

### 13.1 Headless

`engine_project_run/build`在Editor不存在时生成并消费同一Adapter，保持309-F已证明的No-Editor路径。

### 13.2 Optional Editor

Editor只消费Compiler准备的同一项目Runtime artifact或等价cache identity。Editor不得保留手工生成、
独立descriptor或第二套project logic registration。

### 13.3 Windows Player

现有Player staging/host template链接Compiler选择的generated Adapter。Windows包结构、RuntimePackage、
process verification和delivery identity由现有owner继续维护。

### 13.4 Android Player

Android launcher只消费同一generated Adapter的target-specific composition。Android exporter、Gradle、NDK、
ABI选择和APK交付仍属现有Android owner；本方案只消除项目侧手工runtime registration，不扩展工具链。

### 13.5 单一项目逻辑owner

Headless、Editor、Windows和Android不得分别生成或维护项目玩法实现。平台只允许拥有launcher/transport
Adapter，不允许拥有项目规则分支。

## 14. Version与兼容合同

以下identity必须显式区分：

- Project Game SDK contract version；
- ProjectRuntimeAbi version/digest；
- protocol/wire contract digest；
- glue generator identity；
- project source revision；
- target/toolchain artifact identity；
- RuntimePackage schema/version。

规则：

1. manifest声明兼容的Project Game SDK requirement，不直接声明raw ABI struct size。
2. Compiler解析SDK requirement并选择匹配的SDK/generator/ABI组合。
3. generated Adapter嵌入精确contract identities；Runtime加载时继续fail-closed验证。
4. SDK向后兼容变化不得无理由改变旧project semantic result。
5. incompatible SDK/ABI组合在prepare阶段失败，不能到Runtime启动后才崩溃。
6. generated content变化必须由至少一个声明的input identity变化解释。

## 15. Diagnostics与报告

Generation/compile诊断至少包含：

```text
code
stage
projectRevision
sdkContractIdentity
generatorIdentity
sourcePath
sourceSymbol or manifestField
generatedItemKind
reason
expected
actual
nextActionCategory
```

错误优先指向project-authored source：

```text
正确：RuntimeModule/src/weapons/homing.rs / ShooterSession::fixed_update
错误：Library/Generated/runtime_glue/src/lib.rs:281，要求AI自行猜测
```

报告按 `Off / Summary / Trace` 分档：

- 普通run/build只返回compact Summary与必要diagnostics/evidence refs；
- Trace可包含generation manifest、source map和依赖失效详情；
- 正式Runtime热路径不常驻生成长JSON或写调试文件。

## 16. 三个样例迁移策略

### 16.1 Complex Shooter

保留现有rule、WorldRead、deferred mutation、UI state与玩法结果；项目只保留typed gameplay handlers和
单一registration，删除手工ABI callbacks、API table、session store、descriptor和digest source list。

### 16.2 Switch Puzzle

保留stateful session、ordered AUI actions、conditional UI state与puzzle rules；用同一SDK证明有状态项目
不需要项目自行维护opaque ABI handle。

### 16.3 Tower Defense

保留已拆分的复杂项目结构、`TowerDefenseRuntimeSession`、AUI、observation与fixed update；只替换最外层
ABI/runtime facade重复胶水，不重写玩法Module或恢复Tower P1-2。

### 16.4 迁移规则

- 迁移是replace，不是长期Adapter叠加；
- 新SDK与generated path通过后删除样例旧ABI Implementation与重复测试；
- 允许有界兼容Adapter帮助一次迁移，但不得成为永久第二pipeline；
- 每个项目的行为差异必须留在项目侧，生成器不包含sample-specific条件分支。

## 17. 验收标准

### 17.1 Project authoring surface

三个样例均满足：

- project-authored source不直接依赖`project_runtime_abi`；
- project-authored source不定义`ProjectRuntimeApi`；
- project-authored source不定义`aife_project_runtime_entry_v1`；
- project-authored source不包含ABI buffer/pointer/extern callback；
- 只有一处typed project registration；
- module/build identity无重复Rust常量真相；
- 不手工枚举AOT digest source files。

### 17.2 Generated Adapter

- 同输入生成内容与digest稳定；
- cache删除后可精确重建；
- capability/descriptor与typed registration一致；
- session create/destroy与invalid handle fail-closed；
- panic、small buffer、invalid input保持现有ABI安全合同；
- source map覆盖全部generated callback/registration item；
- generated output不进入canonical project revision。

### 17.3 Consumer equivalence

- Headless、Optional Editor、Windows Player和Android source composition使用同一SDK contract与项目logic；
- Complex Shooter、Switch Puzzle、Tower Defense现有行为合同保持；
- run/build/delivery artifact lineage继续绑定同一project revision；
- Editor或Host Adapter变化不改变项目Runtime artifact identity；
- 旧手工pipeline与重复测试删除，只剩一条正式路径。

### 17.4 Agent体验

真实或等价Agent流程必须证明：

```text
inspect/read
-> 使用普通文件工具修改SDK项目代码
-> engine_project_run或build自动生成并编译glue
-> 失败诊断回到项目source
-> 成功返回revision/artifact/evidence
```

不得要求Editor、二级Host、glue tool或固定内部生成步骤。

## 18. 验证边界

后续施工文档必须按风险选择测试，但至少覆盖：

- Project Game SDK owner tests；
- generated content golden/structural tests；
- ABI/protocol safety regression；
- Compiler generation/cache/source-map tests；
- 三样例compile与behavior equivalence；
- Headless/Editor/Windows/Android source-composition consumer tests；
- 旧manual glue/static dependency retirement tests；
- Agent-visible diagnostics/result conformance。

本方案不预授权：

- Local CI；
- production或安装态替换；
- 真实Codex配置修改；
- Android工具链、APK或设备测试；
- 真实Editor窗口测试；
- Tower P1-2。

这些只有在施工文档明确范围且用户授权时才能执行。

## 19. 被否决方案

### 19.1 方案 A：Proc-macro-only SDK

优点是源码短、编译期类型检查强。否决其作为完整方案，因为宏展开会隐藏完整ABI与注册拓扑，
不能单独解决Player/Android/Cargo/source map和Compiler lineage，长期容易成为不可检查黑盒。

允许在方案 C 内使用极小derive/attribute减少局部样板，但不得让宏拥有第二套project identity或构建owner。

### 19.2 方案 B：Schema-first外部Generator

优点是deterministic、inspectable和language-neutral。否决其作为完整项目编程面，因为handler path和Rust
type容易在schema中重复，schema会逐渐变成第二种脚本语言。

方案 C只让manifest拥有build identity，让typed Rust registration拥有handler truth，Compiler负责派生。

### 19.3 三个新深Module串联

否决新增SDK Host、Glue Module和Compiler wrapper。现有Provider和Compiler已经拥有真实seam；Generated
Glue通过删除测试后只应是派生Adapter。新增中间Module只会产生第二Interface、第二owner和重复测试。

### 19.4 继续手写ABI模板

否决通过复制sample模板或AI生成样板解决问题。复制代码没有减少AI需要理解的Interface，也无法阻止
descriptor、capability和Player长期漂移。

## 20. 自审结论

### 20.1 与权威产品需求一致

- AI继续直接写真实代码和项目内容；
- concrete Engine tools继续与普通文件工具处于同一Agent Tool Loop；
- 默认完整路径无Editor；
- Engine Provider不是Agent；
- Tool projection没有代替SDK/Compiler深能力；
- Rust Native Runtime与RuntimePackage保持正式运行真相。

### 20.2 结构冗余自审

- 本方案没有新增三个深Module；
- Provider和Compiler是已存在的前置Module；
- 只新增一个项目侧外部Interface；
- Generated Runtime Glue降为Compiler内部派生Adapter；
- 不新增独立Host、Registry、cache owner、process或tool；
- 删除任一真正Module后，其复杂度会重新散落到多个caller，保留理由成立；
- 删除独立Glue Module概念不损失能力，因此已明确禁止。

### 20.3 owner自审

- manifest、typed registration、Compiler派生事实分工唯一；
- RuntimeModule、descriptor、Player和测试不再同时维护相同identity；
- generator只消费lease-owned SourceView；
- Generated output不是authoring truth；
- Headless、Editor、Windows、Android不建立第二project logic owner。

### 20.4 范围自审

- v1只覆盖现有ABI已经具备的真实capabilities；
- 不补做完整R3、Semantic Outcome、AI Development Pack或新Engine能力；
- 三样例只用于迁移和equivalence；
- 不修改production、真实配置、Android工具链或Tower P1-2；
- 施工已严格按不过量 Gate A-D 完成；未进入 production、Local CI、Android工具链或真实Codex新一轮验收。

自审结论：方案 C 已按冻结边界完成 source/integration 施工；Project Game SDK、Compiler-generated Adapter、
统一consumer与三个样例迁移均已有实现和证据，未引入第二Host、Provider、Registry或Compiler。

## 21. 当前状态与下一步

```text
方案选择：C（用户已确认）
正式方案：本文
方案自审：通过
施工文档：已完成并归档
当前施工授权：已结束
代码施工：Gate A-D完成
能力状态：source_integration_passed
完成记录：阶段完成记录/2026-09-02-310-Project-Game-SDK-Generated-Runtime-Glue-v1/00-总览.md
下一步：不自动激活新系统；后续需求重新研究、确认方案并获得施工授权
```
