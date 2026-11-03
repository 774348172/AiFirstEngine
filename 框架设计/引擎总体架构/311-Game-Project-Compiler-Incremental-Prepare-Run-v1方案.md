# 311 Game Project Compiler + Incremental Prepare/Run v1

> 文档类型：正式架构子方案
> 方案状态：已完成并归档；窗口 A/B/C 全部通过
> 方案选择：B（检查与增量准备闭环）
> 施工完成复核：2026-09-09，最终以施工第14节为准
> 适用范围：R3 Game Project Compiler、`engine_project_check`、增量 prepare/run

## 1. 结论

R3 不新建通用构建平台，也不把 `prepare` 暴露成 Agent 工具。复用现有
`GameProjectCompiler`、`ProjectRuntimePackageAssembler`、`ProjectAssemblyArtifactCache`
和 Cargo fingerprint，补齐真实的项目检查、源码级诊断和可解释的增量准备。

模型默认 Engine 工具面在 R3 资格化后增加一个工具：

```text
engine_project_check
```

现有 `engine_runtime_run`、`engine_project_build` 继续作为外部决策点；prepare、glue、cook、
RuntimePackage assembly、staging 和 worker 管理仍是 Compiler 内部阶段。

## 2. 目标与非目标

目标：

- 修改后可独立检查项目格式、资源引用、目标配置和项目 Rust 编译输入；
- 诊断必须能回到源文件、对象或字段，并给出可执行的局部下一步；
- 无 Editor 完成 `inspect -> check -> run/build`；
- 未变化的有效产物可复用，变化只传播到受影响依赖；
- 缓存命中、失效、损坏和保守重建均有结构化结果；
- check、run、build 共用同一份快照、校验和准备 owner。

非目标：

- 不实现 R4 Semantic Outcome、playtest、observe、capture 或 replay；
- 不实现远端/分布式缓存、常驻构建 daemon 或新的任务调度器；
- 不替代 Cargo 实现 Rust 依赖 fingerprint；
- 不建立第二个 Compiler、Assembler、Provider、Registry 或 Agent Planner；
- 不把完整项目计划、自然语言理解或玩法判断放入 Compiler。

## 3. 当前基线

`rust/crates/project_authoring_execution/src/game_project_compiler.rs` 已具备：

- operation-bound `ProjectSnapshotLease` 到 immutable `CompilerSourceView`；
- manifest、Build Profile、项目身份和 revision 校验；
- generated Runtime Glue；
- `ProjectRuntimePackageAssembler` 调用；
- `prepare_with_artifact_cache` 接口；
- 无 Editor 的 run/build/verify 链路。

施工前缺口（历史基线，A/B已补齐源码能力）：

- 没有独立的 Compiler `check` 接口和 `engine_project_check` 真实 owner；
- Compiler 错误虽有 code/stage/next_action，但缺少统一的 source/object/field location；
- Provider 的 run/build 主要调用无缓存 prepare；
- Player staging 将 Cargo target 置于 staging，并设置 `CARGO_INCREMENTAL=0`，不能宣称跨修改
  Rust 编译已有完整增量闭环；
- 缓存已有 envelope、dependency digest、producer report，但尚未形成统一的 prepare summary。

窗口 A 已完成真实 check、源码诊断与lease隔离；B 已完成字体/图片缓存、prepareSummary和稳定Cargo目录复用。
窗口 C 的最终恢复验收已完成：check Ready，默认面7项；MCP错误定位、修复重检及直接run已验证，终态107项通过。前述缺口列表为施工前基线。

## 4. 核心接口

### 4.1 Compiler

```text
inspect(ProjectSnapshot, Scope) -> ProjectFacts
check(ProjectSnapshot, CheckProfile) -> CheckReport
prepare(ProjectSnapshot, TargetProfile) -> PreparedRuntimePackage
```

三个接口必须接受同一 operation-bound snapshot，不读取 live filesystem，不依赖 Editor draft。
`prepare` 是内部接口，不注册为工具。

这里的限制针对项目语义源输入；允许读取声明的 SDK/toolchain 和缓存、写入受控派生目录。
Rust 检查与编译 staging 必须来自 lease-owned bytes，不得在校验 revision 后重新复制 live 项目源码。
check 不修改项目源、不运行游戏或测试，但 Cargo/build script/proc macro 可能执行代码和写派生缓存，
因此必须按真实进程副作用经过现有 Host approval/Engine Grant，不能声明为纯 Read。

### 4.2 CheckReport

```text
schemaVersion
projectIdentity
projectRevision
qualification
diagnostics[]
dependencySummary
cacheReadiness
recommendedLocalTransitions[]
```

每条诊断至少包含：`code`、`stage`、`severity`、`message`、`sourcePath`（若适用）、
`objectId/fieldPath`（若适用）和 `nextAction`。结构化字段是真相，文本只是展示投影。

### 4.3 PrepareSummary

prepare 结果补充：

```text
preparationIdentity
sourceIdentity
targetProfile
cacheStatus
producerReports[]
invalidatedDependencies[]
reusedDependencies[]
diagnostics[]
```

`sourceIdentity`、目标 profile、Compiler/producer recipe、SDK/toolchain identity 和依赖 digest
共同决定有效性；revision 用于 lineage 和 drift，不得单独导致全量失效。

现有 `sourceIdentity/preparationIdentity` 包含 snapshot/revision，只保留为调用与产物追溯身份；
producer 复用键必须使用其实际输入的内容 digest，不能直接沿用全项目 sourceIdentity。
Summary 仅返回阶段、计数与关键失效原因；完整依赖明细按需进入 Trace，避免常驻大报告。

## 5. 增量规则

采用三层失效：

1. **项目语义输入**：manifest、scene、prefab、rule、AUI、input 和 asset references 的
   内容 digest；
2. **producer 依赖**：每个 cooker/生成器的 recipe version、dependency digest 和 output digest；
3. **Rust 项目编译**：继续由 Cargo fingerprint、lockfile、features、profile、target 和
   依赖输出决定 freshness。

规则：

- 内容未变且产物校验通过：复用；
- 内容或 recipe 变更：仅失效其反向依赖闭包；
- 缓存 envelope、payload 或产物校验失败：标记 invalid/corrupt 后重建；
- 无法证明依赖关系：保守失效，不允许错误命中；
- 不同项目、revision、target、SDK 或 toolchain 不得共享不兼容产物。

R3 不自行推导完整 Rust 依赖图；它只负责为 Cargo 提供稳定、隔离、可复现的输入，并将 Cargo
结果映射为 Compiler 诊断和 prepare summary。

324修正：最终Player产物仍绑定完整SDK源码digest，但可变Cargo编译工作区不使用该源码digest选目录；同一SDK路径、项目与兼容配置下，局部引擎源码变化由Cargo失效反向依赖。编译缓存复用不代表最终产物复用，旧产物不可误命中。开发/优化配置各自使用稳定缓存，不为每轮调参创建新的target。

## 6. Agent 工作流

```text
Host 文件工具或 engine_project_mutate
    -> engine_project_check（需要时）
    -> engine_runtime_run / engine_project_build
```

`check` 可跳过。`run/build` 在内部执行必要检查和 prepare。失败结果返回当前阶段、诊断和
局部合法转移，不要求 Agent 手动调用 refresh、snapshot、prepare、assemble 或 cache lookup。

Compiler 不拥有目标级计划；Skill 只说明 check 的进入/跳过条件、常见诊断和恢复方式。

## 7. 方案选项与取舍记录

已比较：

- A：只补 check，最小但不能解决重复准备；
- B：检查与增量准备闭环，复用现有 owner；
- C：建设通用构建平台，范围过大且引入新 authority。

用户已确认采用 B。选择理由是覆盖 R3 目标，同时保持 Compiler、Assembler、Cargo 和现有
缓存的职责边界不变。

## 8. 外部实现借鉴

- Godot `EditorFileSystem` 以 source digest、import settings/version 和 destination digest
  判断 reimport；本项目只采纳内容/配置/产物一致性思想，不引入 Editor-centered authority。
- Cargo `fingerprint` 追踪编译单元、profile、features、target、依赖 fingerprint 和 lock/config；
  本项目复用 Cargo，不再实现第二套 Rust freshness 算法。

## 9. 资格 Gate

- G0：接口、identity、diagnostic schema 与权威架构及 309-F/310 一致；
- G1：check 能对无效 manifest、profile、缺失引用和项目 Rust 编译输入给出稳定诊断；
- G2：check/run/build 使用同一 snapshot 和 Compiler owner；
- G3：无变更重复 prepare 可命中有效缓存，局部变更不会错误复用；
- G4：缓存损坏、recipe 变化和依赖不确定性会 fail-closed 并可重建；
- G5：No-Editor check/run/build 集成证据保持 revision、artifact 和 evidence lineage 一致；
- G6：只在 G1-G5 通过后将 `engine_project_check` 投影到模型默认面。

每个 Gate 必须绑定 owner-level red-capable test；不默认要求 Local CI、production、Android
或 R4 视觉验证。

## 10. 施工边界

施工文档只允许涉及：Compiler check owner、诊断位置字段、prepare/cache 接入、Provider
check 投影、受影响测试和必要文档同步。若需要新 daemon、远端缓存、第二依赖图或 R4
Outcome，必须暂停并重新讨论方案。

本方案已完成A/B/C并归档。B的MSVC长路径限制见第11节；C最终修复和回归证据见第14节。

施工归档：`施工文档/已完成/311-Game-Project-Compiler-Incremental-Prepare-Run-v1施工文档.md`；当前施工不再为311。
2026-09-09 自审补充仅澄清 source/派生输入、check 副作用和缓存键含义，不新增 owner 或工具。
