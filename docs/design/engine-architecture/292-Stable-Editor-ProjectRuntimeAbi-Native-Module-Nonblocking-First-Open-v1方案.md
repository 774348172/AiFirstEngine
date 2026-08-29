# 292 Stable Editor + ProjectRuntimeAbi Native Module + Nonblocking First Open v1 方案

## 1. 文档状态

```text
系统编号：292
方案版本：v1
建立日期：2026-08-15
缺口来源：post-291 production Editor 首次打开 Tower 触发完整项目专属 Editor release build
正式选择：修订方案 C（稳定 Editor + 版本化项目原生模块 + 非阻塞打开/后台预热）
当前状态：正式方案已确认，方案自审通过；源码施工完成
施工状态：Window A-E / Gate A-H 已完成并归档；production Editor 一致性未执行
```

本文是 262/269/282 后续的独立架构深化，不是 282 Gate H repair，也不修改 Tower gameplay。
它解决的不是“引擎不变时第二次打开快”，而是：引擎经常修改时，项目首次打开仍不因重新链接完整
Editor 而阻塞。

## 2. 一句话目标

```text
引擎实现可以频繁变化
  -> production Editor 先直接打开项目并保持可交互
  -> 只按稳定 ProjectRuntimeAbi 判断项目原生模块是否失效
  -> ABI 未变时复用既有项目模块
  -> ABI 真变化时只在后台重建项目模块，不重编完整 Editor
  -> 模块 ready 后才允许 Play
```

## 3. 用户需求的正式解释

“引擎改动后首次打开也快”分成两个必须同时成立的结果：

1. **Editor 首次可交互不等待 Cargo/rustc。** 项目浏览、Scene/AUI/资源编辑和报告查看先进入普通
   production Editor。
2. **失效范围与真实接口变化一致。** Editor UI、窗口、渲染器或 Runtime 内部实现变化，不得使
   ProjectRuntimeModule artifact 失效；只有 ProjectRuntimeAbi、项目 Rust、项目 manifest/lock、目标或
   toolchain 等真实输入变化才允许重建项目模块。

本文不承诺 ABI 破坏后“完全不编译”。那在没有兼容 artifact 时不可能成立。本文保证该编译：

```text
不位于项目打开关键路径
+ 只构建项目原生模块
+ 不编译/链接 editor_core、editor_window_winit、ai_tool_gateway 或 generated Editor executable
```

## 4. 当前问题与真实证据

### 4.1 当前静态组合

262 当前链路是：

```text
production Editor
  -> 读取项目 RuntimeModule 声明
  -> 生成 project-specific Editor crate
  -> 静态链接 editor_core + editor_window_winit + ai_tool_gateway
     + engine_runtime + Tower RuntimeModule
  -> release build 完整 Editor executable
  -> handoff 到项目专属 Editor
```

该模型保证了 Rust trait 不跨 DLL，但代价是项目模块与完整 Editor 实现形成同一个最终 artifact。

### 4.2 本次首次打开证据

post-291 production Editor identity 更新为：

```text
F8856CE9A507E7A2E6D4DCA6E8D20B2957ECB507AACB414917281240D7420EA8
```

Tower 模块 digest、manifest、dependency graph 与 toolchain 没有变化，但 composition identity 因完整
`editorBuildIdentity` 变化而失效。真实 build report：

```text
cacheStatus：rebuilt
compilationCacheAffinity：same_root_hit
generate-lock：1,281 ms
release build：217,540 ms
descriptor query：606 ms
```

release stderr 明确重新编译/链接：

```text
editor_core
tower_defense_project_runtime
ai_tool_gateway
editor_window_winit
generated project Editor executable
```

所以项目大小不是首因。首因是完整 Editor implementation identity 直接进入了项目最终 composition
artifact，并且该 artifact 本身就是另一个 Editor executable。

### 4.3 现有文档已经承认该缺口

282 第 6.2 节明确将以下能力延期为后续独立方案：

```text
把“Editor 整个可执行文件哈希”替换为 ABI/capability digest，
减少与 composition ABI 无关的 invalidation。
```

仅替换 identity 仍不够：旧静态项目专属 Editor 内含旧 Editor 实现，不能让新 production Editor 直接
消费。因此本方案必须同时改变 Editor 与项目 RuntimeModule 的装配形态。

## 5. 成熟实现研究

### 5.1 Godot GDExtension

Godot 通过专门定义的 C interface、extension API 描述和 `.gdextension` 文件，在不重新编译引擎的
情况下加载原生共享库。可学习点是：

```text
稳定 C ABI
+ 版本/能力协商
+ 原生共享库独立 artifact
+ 引擎与扩展各自拥有内存和生命周期
```

不能照搬 Godot 的完整对象/反射注册面。本引擎只需要 ProjectRuntimeModule 的窄接口，不建设通用
插件系统。

参考：<https://docs.godotengine.org/en/stable/engine_details/engine_api/gdextension/what_is_gdextension.html>

### 5.2 Unreal Modules / Live Coding

Unreal 将 Engine、Project 和 Plugin 代码按 module 管理，Live Coding 可只重建并 patch 变化模块；官方
同时指出预加载更多 module 会增加启动时间。可学习点是按模块确定失效范围并把编译从完整 Editor 重建中
拆开，不能照搬对象 reinstancing、Live++ patch 或 Unreal 的宏/反射体系。

参考：

- <https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-modules>
- <https://dev.epicgames.com/documentation/en-us/unreal-engine/using-live-coding-to-recompile-unreal-engine-applications-at-runtime>

### 5.3 Unity Assembly Definition

Unity Assembly Definition 通过显式依赖图减少项目代码重编范围。它证明“依赖 identity 应绑定接口与
依赖图，而不是整个 Editor executable”是正确方向；但托管程序集加载、Domain Reload 与 Rust native
ABI 不同，不能直接复用。

参考：<https://docs.unity3d.com/Manual/assembly-definition-files.html>

### 5.4 Rust ABI 约束

Rust 默认 ABI、trait object layout、`Vec`、`String` 和 enum layout 不提供跨 toolchain/crate graph 的
稳定合同。因此项目 DLL 不能直接导出或接收 `dyn ProjectRuntimeModule`、`dyn ProjectRuntimeSession`、
ECS 引用或任何 Rust-owned collection。

参考：<https://doc.rust-lang.org/reference/abi.html>

## 6. 方案比较与正式选择

### 6.1 方案 A：保留静态项目专属 Editor，只改 cache identity

```text
优点：代码改动最小。
缺点：复用旧 artifact 就会运行旧 Editor 实现；使用新 Editor 又必须重新链接完整 executable。
结论：拒绝。它不能同时保证实现一致性和首次打开速度。
```

### 6.2 方案 B：稳定 Editor + 项目原生 DLL，但打开前同步构建

```text
优点：重建范围从完整 Editor 缩小到项目模块。
缺点：cold/ABI-break build 仍阻塞首次打开，用户仍看到长时间等待。
结论：不完整。可作为内部装配基础，不能单独满足用户需求。
```

### 6.3 修订方案 C：稳定 Editor + 版本化项目原生模块 + 非阻塞打开/后台预热

```text
优点：
- Editor 实现变化与项目模块失效解耦；
- 项目打开不等待编译；
- ABI 未变时直接验证并加载已有模块；
- ABI 真变化时只后台重建项目模块；
- 延续 Rust 底层规则 + Schema 上层规则；
- 保留 exact identity、trust、artifact seal 和失败关闭。

代价：
- 必须正式建立很小的稳定 native ABI；
- 必须处理内存所有权、panic、DLL 搜索、加载生命周期和 Windows 文件占用；
- ABI 破坏时 Play readiness 仍需等待项目模块构建。

正式选择：修订方案 C。
```

## 7. 目标与非目标

### 7.1 v1 目标

1. 普通 production Editor 成为稳定宿主，不再为每个项目生成并启动完整专属 Editor executable。
2. 建立版本化、可生成、可哈希的 `ProjectRuntimeAbi`，以及只依赖该 ABI 的稳定
   `ProjectRuntimeSdk`，作为 Editor 与项目原生模块之间唯一 native seam 和项目侧 Rust 编程面。
3. Tower、Complex Shooter、Switch Puzzle 和 workspace 外 fixture 经同一 ABI/loader 路径工作，不允许
   项目名或玩法硬编码。
4. 项目模块构建输出独立 hash-qualified DLL、descriptor、build report 和 load report。
5. 模块 cache identity 不直接包含完整 production Editor executable hash。
6. Editor 打开项目后先进入可交互 authoring；模块准备在后台进行，只有 Play/运行观察依赖 readiness。
7. ABI 未变化的引擎更新必须复用原项目 DLL；ABI 变化时只重建项目 DLL。
8. 复用 262 trust/dependency/staging、282 QoS/cancel/cache lifecycle，不另造第二套 Cargo 管理器。
9. 保持 RuntimePackage module descriptor、ProjectRuntimeBootstrap exact bind 和现有内部 Rust trait
   consumer 语义。

### 7.2 v1 非目标

- 不建设通用 Editor 插件系统、Marketplace、脚本 VM、WASM runtime 或任意第三方 DLL loader。
- 不支持跨 DLL 传 Rust trait object 或 Rust-owned 内存。
- 不实现任意时刻卸载/覆盖已加载 DLL，也不承诺 native hot reload 无状态切换。
- 不修改 Player/Export 的静态装配；Player 动态模块化作为后续独立评估。
- 不修改 Tower gameplay、AUI、Sprite2D、Animator2D、Scene 或真实项目配置。
- 不放宽 ProjectRust trust、Cargo dependency allowlist、locked/offline 或路径 containment。
- 不建设 daemon、远程 cache、分布式编译或全局 Cargo 配置。
- 不把 prewarm 当正确性的前提；未预热时仍必须安全地非阻塞打开。
- 方案文档不授权源码施工、Local CI、production/installed binary 替换、真实 cache 写入或真实配置修改。

## 8. 正式架构

```text
Stable production Editor executable
  -> Open project authoring immediately
  -> ProjectRuntimePreparationModule.prepare_async(request)
       -> existing trust + controlled staging
       -> ProjectNativeModuleIdentity resolve
       -> exact module cache lookup
          -> hit: seal/ABI validation
          -> miss: bounded background module-only build
       -> ProjectRuntimeNativeModuleLoader.load(qualified artifact)
       -> LoadedProjectRuntimeModuleAdapter
            implements internal Rust ProjectRuntimeModule trait
       -> existing LinkedProjectRuntimeSet singleton
       -> existing ProjectRuntimeBootstrap::bind(RuntimePackage, set)
       -> Play ready
```

项目侧：

```text
Tower RuntimeModule Rust implementation
  -> stable ProjectRuntimeSdk
  -> generated/owned ABI facade
  -> aife_project_runtime_entry_v1
  -> hash-qualified tower_runtime_<artifact-prefix>.dll
```

Editor 内部现有 `ProjectRuntimeModule` / `ProjectRuntimeSession` trait 可以保留，但只能由引擎侧
`LoadedProjectRuntimeModuleAdapter` 实现。trait 本身绝不跨 DLL。

## 9. 稳定 ProjectRuntimeAbi seam

### 9.1 ProjectRuntimeSdk 依赖规则

只把最终 artifact 改成 DLL 还不够。若项目 crate 继续依赖完整 `engine_runtime`，任何 Runtime 内部源码变化
仍会让 Cargo 重新编译项目模块。正式依赖方向必须改为：

```text
Project RuntimeModule
  -> project_runtime_sdk（安全 Rust wrapper、项目可见 schema/type）
       -> project_runtime_abi（repr(C) POD、function table、ABI manifest）

Stable Editor
  -> engine_runtime internal implementation
  -> project_runtime_abi
  -> host adapter
```

规则：

- `project_runtime_sdk` 可以静态编译进项目 DLL，因为它不跨二进制暴露 Rust ABI。
- SDK 只提供 WorldRead、deferred mutation、AUI action、UI state、observation 等项目所需窄能力。
- 项目 DLL 的 Cargo graph 不得包含完整 `engine_runtime`、`editor_core` 或任何 Editor crate。
- engine runtime 内部类型由 host adapter 转换，不能泄漏到 SDK public surface。
- ABI/SDK surface 真变化时必须更新 canonical digest；普通 Runtime implementation 修改不得改写该 digest。

这条依赖规则是“Runtime 内部实现变化但 ABI 未变时复用项目 DLL”成立的必要条件，不得在施工时省略。

### 9.2 唯一导出入口

项目 DLL 只暴露一个稳定符号：

```c
AifeStatus aife_project_runtime_entry_v1(
    const AifeProjectRuntimeHostV1* host,
    AifeProjectRuntimeModuleV1* out_module
);
```

`host` 和 `out_module` 均包含：

```text
abiMajor / abiMinor
structSize
capabilityBits
function table
reserved fields
```

v1 采用 exact `abiMajor + abiDigest` 资格化；`structSize/capabilityBits` 用于明确诊断和未来受控兼容，
不在 v1 静默接受未知组合。

### 9.3 ABI-safe 数据规则

跨 seam 只允许：

```text
固定宽度整数/浮点
显式 repr(C) POD
pointer + length 的只借用字节/UTF-8 slice（仅调用期间有效）
opaque u64 handle + generation
caller-owned output sink / buffer
显式 status code
```

禁止：

```text
Rust reference / trait object / Box / Arc / Vec / String
Rust enum 默认布局
panic/unwind 穿越 ABI
World/ECS 裸指针或 entity index
GPU handle、Renderer 内部对象、文件/网络能力
```

### 9.4 最小 function table

接口按现有真实消费能力收敛，不暴露完整引擎：

```text
module_descriptor
create_session / destroy_session
register_rule callbacks
handle_aui_actions
fixed_update
produce_ui_state
observe
```

World 读取和 mutation 输出继续遵守现有项目规则：项目只能通过 host-owned `WorldRead` 函数表与
deferred mutation sink 工作。AUI action、observation 与 UI state 通过版本化 packet/sink 传递，不让项目
模块持有调用后的宿主指针。

### 9.5 内存、panic 与线程合同

```text
谁分配谁释放；不得跨模块 free 对方内存。
借用 slice 只在一次调用内有效。
每个导出 facade 使用 catch_unwind，panic 转为 terminal status/diagnostic。
ABI profile 必须允许 facade 捕获 unwind；不得让 panic 穿越 extern "C"。
v1 callback 仅在创建它的 runtime thread 调用，项目不得缓存 thread-local host pointer。
session destroy exactly once；destroy 后所有 handle 失效。
```

`catch_unwind` 不能捕获进程 abort、访问违规或任意 native 崩溃。受信 ProjectRust 与当前静态进程内
执行具有相同的 native trust 风险；进程隔离不是本文目标。

## 10. Identity 与失效范围

### 10.1 三个独立 identity

```text
EditorShellIdentity
  = production Editor executable/source/features identity

ProjectRuntimeAbiIdentity
  = ABI schema/version/layout/function-table/capability contract digest
  + stable ProjectRuntimeSdk public contract digest

ProjectNativeModuleIdentity
  = ProjectRuntimeAbiIdentity
  + project/module/interface/AOT digest
  + normalized manifest/dependency/lock digest
  + toolchain/target/profile/features
  + native module build schema/tool version
```

`ProjectNativeModuleIdentity` 明确不包含完整 `EditorShellIdentity`。它也不能使用一个人工维护但无法审查的
版本字符串代替 ABI 真相；ABI header/schema 与生成结果必须产生 canonical digest。

完整 `engine_runtime` source digest 同样不得进入该 identity。项目只绑定稳定 ABI/SDK；host adapter 对当前
engine implementation 负责。

### 10.2 变更分类

| 变更 | Editor | 项目 DLL | 首次打开 | Play readiness |
|---|---|---|---|---|
| Editor UI/窗口/渲染实现 | 重建 | 复用 | 立即 | 立即 |
| Runtime 内部实现，ABI 不变 | 重建 | 复用 | 立即 | 立即 |
| ProjectRuntimeAbi/schema 破坏 | 重建 | 后台重建 | 立即 | 构建后 ready |
| 项目 Rust/manifest/lock 变化 | 不变 | 后台重建 | 立即 | 构建后 ready |
| Scene/AUI/资源/Input 数据变化 | 不变 | 复用 | 立即 | 只重建 RuntimePackage |

任何分类不确定时必须按 ABI/module stale 失败关闭，不能回退旧模块进入 Play。

## 11. Artifact、缓存与 Windows 加载

项目原生模块 artifact 至少包含：

```text
project-runtime-native-module-descriptor.v1.json
project-runtime-native-module-build-report.v1.json
bin/<module-id-safe>_<artifact-hash-prefix>.dll
artifact seal / file hash
```

descriptor 至少记录：

```text
projectId
moduleId
logicalInterfaceVersion
aotContentDigest
projectRuntimeAbiVersion
projectRuntimeAbiDigest
normalized manifest/dependency/lock digests
toolchain / target / profile / features
artifactHash
createdAt
```

Windows 规则：

- DLL 文件名必须 hash-qualified；绝不覆盖正在加载的文件。
- 只从 application-owned、已 canonicalize 且通过 seal 的绝对路径加载。
- 使用安全 DLL search policy，禁止当前目录/PATH 注入未声明依赖。
- v1 已加载 library handle 至少存活到相关 session 全部销毁；不做任意热卸载。
- 项目模块新 generation 采用新文件名。旧 generation 由 cache owner 在无活跃进程时回收。
- loader 失败不影响 authoring，但必须禁用 Play 并给出 typed nextAction。

## 12. 非阻塞打开与后台预热

### 12.1 普通打开

```text
T0  启动稳定 production Editor
T1  解析项目最小 authoring metadata
T2  显示项目并允许普通编辑
T3  后台执行 trust/cache/ABI preparation
T4a exact hit -> load -> Play ready
T4b miss/stale -> bounded module-only build -> seal -> load -> Play ready
T4c failed -> authoring 保持可用，Play unavailable + typed diagnostic
```

关键合同：`T2` 不等待 `T3/T4`，也不等待 Cargo/rustc 子进程。

### 12.2 预热

预热只作为降低 ABI-break 后 Play 等待的优化：

- Editor 更新产生 ABI compatibility manifest。
- ABI 未变时不调度任何项目重建。
- ABI 变化时，只对已有有效 trust receipt 的最近项目安排低优先级 module-only prewarm。
- prewarm 使用 282 的 jobs/priority/cancel/deadline 和 application-owned cache。
- 用户主动打开项目优先于后台 prewarm；同 identity 合并为一个 owner task，不重复构建。
- prewarm 失败不阻止 Editor 启动，也不修改项目源、真实配置或 production binary。

即使 prewarm 完全没有运行，普通打开仍必须满足非阻塞合同。

## 13. 深 Module 与 seam

### 13.1 ProjectRuntimePreparationModule

外部 interface：

```rust
prepare_async(request, control, progress)
    -> ProjectRuntimePreparationHandle
```

该深 Module 隐藏：trust、staging、identity、cache lookup、282 build QoS、seal、single-flight、取消和
readiness publication。Editor caller 不能自行拼接十几个状态，也不能同步等待完整 build 才打开项目。

### 13.2 ProjectRuntimeNativeModuleLoader

外部 interface：

```rust
load(qualified_artifact)
    -> LoadedProjectRuntimeModuleAdapter | ProjectRuntimeLoadDiagnostic
```

该深 Module 隐藏：安全路径、DLL search、symbol resolve、ABI negotiation、handle lifetime、session destroy
和错误映射。测试通过同一 interface 使用 fake native library adapter；公共调用者不接触平台 handle。

### 13.3 不新增的 seam

```text
不新增 generic plugin registry
不新增第二套 RuntimePackage assembler
不新增第二套 Cargo runner/cache promoter
不新增玩法级 ABI
不把每个 ABI function 拆成公共浅 Module
```

## 14. Schema 与报告

新增/演进 schema 控制在以下最小集合：

```text
project-runtime-abi-manifest.v1
project-runtime-native-module-artifact.v1
project-runtime-native-module-build-report.v1
project-runtime-native-module-load-report.v1
project-runtime-preparation-report.v1
```

preparation report 至少能回答：

```text
authoringReadyAt
runtimeStatus = not_required | lookup | building | loading | ready | failed | cancelled
cacheStatus = exact_hit | stale | rebuilt | failed
editorShellIdentity
projectRuntimeAbiIdentity
projectNativeModuleIdentity
invalidationReason
buildScope
stage durations
artifact/load diagnostics
playReadyAt
```

`buildScope` 必须能证明 `project_module_only`。若实际 Cargo graph 包含 `editor_core`、
`editor_window_winit`、`ai_tool_gateway`、完整 `engine_runtime` 或 generated Editor executable，资格 Gate
必须失败。

报告继续遵守 Off/Summary/Trace：普通运行不每帧写盘，build/load 生命周期只在状态变化或 terminal 时写入。

## 15. Trust、安全与失败关闭

沿用 262 的 ProjectRust trust receipt，但 trust identity 从完整 Editor build 改为：

```text
projectCanonicalRootIdentity
runtimeModule source/manifest/dependency identity
ProjectRuntimeAbiIdentity
decision/source/time
```

EditorShell implementation 变化且 ABI 未变，不得让 trust receipt stale。项目代码或依赖变化仍必须 stale。

必须失败关闭：

```text
缺失/未知 ABI version 或 digest
structSize/capability mismatch
descriptor/AOT/artifact hash mismatch
导出符号缺失
DLL 路径/依赖搜索不安全
项目未信任或 trust stale
module build/load/panic/session fault
RuntimePackage 请求与 loaded module identity 不一致
```

失败时 authoring 可继续；Play、runtime observation 和任何项目 native callback 不得执行。禁止 fallback 到
empty runtime、旧 module、样例静态 set 或旧项目专属 Editor。

## 16. 迁移与 cutover

迁移必须是替换式，不长期并存两套 production 真相：

```text
M1 建立 ProjectRuntimeAbi schema/header/generated facade 与 ABI fixture
M2 让现有 ProjectRust staging 生成独立 DLL artifact/cache/report
M3 loader + LoadedProjectRuntimeModuleAdapter 接回现有 Bootstrap/Play
M4 Editor project open 改为 authoring-first + async preparation + Play readiness
M5 三项目和 workspace 外项目资格通过后，切断普通路径的专属 Editor build/handoff
M6 删除或归档仅服务于 generated project-specific Editor executable 的 production 路径
```

cutover 前允许静态 composition 仅作为测试对照/回滚证据；不得在 dynamic load 失败时自动 fallback，否则
新 ABI 缺口会被旧路径掩盖并永久维持双重复杂度。

Player/Export 的现有静态 ProjectRuntimePlayerArtifact 不随 M5 删除。

## 17. 验证方向

后续施工文档应按风险最小化选择 Gate，不机械复制完整矩阵。最低必须覆盖：

### 17.1 ABI owner

```text
layout/digest 稳定
未知 ABI / structSize / capability / symbol 失败
内存 owner、destroy exactly once、panic containment
safe DLL path/search 与 artifact seal
```

### 17.2 失效矩阵

```text
只改 Editor UI implementation -> 同一项目 DLL exact hit
只改 Runtime implementation、ABI 不变 -> exact hit
改 ABI schema -> 项目 DLL stale/module-only rebuild
改 Tower RuntimeModule -> Tower DLL stale/module-only rebuild
只改 Scene/AUI/Input -> DLL hit，RuntimePackage 按既有规则变化
```

### 17.3 非阻塞 first-open

一个 agent-runnable timing harness 必须证明：

```text
authoring_ready 发生在任何 Cargo build terminal 之前
UI/message pump 在 background build 期间保持可响应
Play 在 module ready 前 fail closed，ready 后无需重启完整 Editor
cancel/close 后 owned process 与 preparation worker 全部 join
```

### 17.4 真实 consumer

```text
Complex Shooter
Switch Puzzle
Tower Defense
workspace 外普通 ProjectRust fixture
```

四者必须通过同一 public manifest/staging/artifact/loader/Bootstrap seam。Tower 只作为 consumer，不进入
引擎接口或模板。

### 17.5 构建范围证据

ABI-break cold qualification 必须记录实际 rustc/Cargo graph，并证明没有重新编译或链接：

```text
engine_runtime（完整内部实现 crate）
editor_core
editor_window_winit
ai_tool_gateway
editor_host/generated project Editor executable
```

数值预算由施工前基线确定；方案层不伪造一个与机器无关的秒数。结构性验收是 Editor 打开不等待编译，
且不可避免的编译被缩小为项目模块。

## 18. 预计涉及范围

以下是后续施工定位范围，不是当前修改授权：

```text
可能新增 rust/crates/project_runtime_abi/
可能新增 rust/crates/project_runtime_sdk/
rust/crates/engine_runtime/src/project_runtime_module.rs
rust/crates/engine_runtime/src/project_runtime_session.rs
rust/crates/editor_core/src/project_runtime_player_staging.rs
rust/crates/editor_core/src/project_editor_composition_artifact.rs
rust/crates/editor_core/src/project_open_preparation.rs
rust/crates/editor_core/src/services/play_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/linked_project_runtimes.rs
rust/crates/editor_window_winit/src/project_editor_composition_production.rs
三个 sample RuntimeModule Cargo/lib facade
对应 owner/consumer/Windows loader/timing tests
```

施工前必须重新定位 owner。优先深化既有 staging、bounded child process、cache 和 preparation worker，
不得按此清单机械新增文件。

## 19. 与既有方案关系

### 19.1 242 / 262

保留 RuntimePackage required module descriptor、ProjectRuntimeBootstrap exact bind、单项目 singleton 和
进程内 GameView。替代的是 262 的“项目模块静态链接进完整专属 Editor executable”装配手段，以及由此产生
的 project-specific Editor handoff。

这属于 262 已明确延期的稳定 native extension ABI 深化，因此必须以独立 292 方案实施，不能伪装为
262/282 小 repair。

### 19.2 269 / 282

复用 269 compilation compatibility 思路和 282 jobs/priority/cancel/deadline、exact artifact seal、事务发布、
cache ownership。292 改变的是最终 artifact 粒度和 identity：从完整 Editor executable 收敛为项目 DLL。

292 不允许忽略 identity 提高 hit rate，而是用真实 `ProjectRuntimeAbiIdentity` 取代无关的完整
`EditorShellIdentity` 耦合。

### 19.3 RuntimePackage / Rust + Schema 规则

项目复杂规则仍由 Rust RuntimeModule 实现；Scene、Prefab、AUI、Gameplay Rule Asset、Input 和其它上层
对象仍是 schema-first。ABI 只是加载/调用 Rust 模块的窄工程合同，不成为新的玩法 DSL，也不允许项目绕过
RuntimePackage、WorldRead、deferred mutation、UiProjection 或其它既有 owner。

### 19.4 Player / Export

v1 不修改已发布 Player/Export 静态装配，避免同时承担动态分发、签名、打包布局和平台加载差异。Editor
资格稳定后，可单独讨论 Player 是否复用同一 ABI artifact；不得在 292 施工中顺手扩大。

## 20. 风险与控制

1. **ABI 面失控。** 只映射现有 ProjectRuntimeModule/Session 的真实消费，新增能力需显式 schema/version
   评审；不暴露 ECS/Renderer/File/Network。
2. **Windows DLL 占用。** hash-qualified 文件 + generation handle；不覆盖已加载 DLL，不把热卸载纳入 v1。
3. **native 崩溃。** facade 捕获 Rust unwind，但 abort/访问违规仍可能终止 Editor；沿用受信 native code
   模型，未来进程隔离单独讨论。
4. **后台构建再次卡 UI。** 强制复用 282 QoS/redraw/cancel，并以真实 message/input timing gate 捕获。
5. **双路径长期共存。** qualification 后切断静态项目专属 Editor 普通入口；失败不 fallback。
6. **ABI 版本忘记更新。** ABI manifest/header/generated binding 的 canonical digest 是事实来源，由测试
   对布局和导出表变化自动判定，不依赖人工字符串。
7. **SDK 偷带完整 Runtime。** 以 dependency deny test 固化项目 DLL 只能依赖 ABI/SDK；一旦 Cargo graph
   出现完整 `engine_runtime` 或 Editor crate，资格立即失败。

## 21. 回滚与停止条件

### 21.1 可回滚单位

```text
ABI schema/generated facade
module artifact builder/cache
native loader/adapter
async preparation/readiness
production cutover/旧 composition retirement
```

### 21.2 必须停止并回到方案讨论

```text
需要跨 DLL 传 Rust trait object、ECS 裸指针或 Rust-owned collection；
必须放宽任意 Cargo dependency/build.rs/path/git 才能构建；
必须让 untrusted/stale 项目 native code 先加载再确认；
无法保证项目打开与 Cargo build 解耦；
module-only build 仍依赖/链接完整 Editor crates；
需要同时改 Player/Export、通用插件系统或 Tower gameplay；
无法证明 DLL 绝对路径、依赖搜索和 artifact seal；
需要动态卸载/覆盖正在加载的 DLL 才能完成 v1。
```

## 22. 方案自审

### 22.1 是否真正满足“引擎改动后首次打开也快”

满足。ABI 未变时项目 DLL identity 不变；ABI 变化时 Editor authoring 先打开，后台只构建项目 DLL。方案
不再依赖“引擎没改”或“刚好预热成功”。

### 22.2 是否只是放宽 hash

否。静态专属 Editor artifact 被稳定 Editor + 独立项目原生模块替代；identity 与 artifact 粒度同步改变，
不会拿旧 Editor executable 冒充新实现。

### 22.3 是否违反 Rust ABI 事实

否。默认 Rust ABI 和 trait object 不跨 DLL；seam 使用显式 C ABI、POD、opaque handle、function table 和
owner-controlled memory。项目侧 Rust 通过静态编入 DLL 的稳定 SDK 使用该 seam，不直接依赖宿主内部类型。

### 22.4 是否破坏 Rust 底层规则 + Schema 上层规则

否。项目规则实现仍在 Rust RuntimeModule；Schema 继续是项目数据和生成合同真相。新增 ABI 只负责装载与
调用，不成为第三套玩法规则层。

### 22.5 是否过量施工

没有。方案明确不做通用插件、任意 hot reload、动态 Player、进程隔离、daemon/remote cache 或 Tower
特例。施工时应先形成一个项目 DLL + stable Editor + nonblocking open 的最小真实纵切，再扩展资格项目。

### 22.6 是否保留正确性与安全性

保留并收紧。262 trust、dependency policy、RuntimePackage descriptor，282 exact seal/QoS/cancel 均继续
有效；ABI mismatch、load failure 和 stale artifact 只影响 Play readiness，不允许 fallback 执行错误模块。

### 22.7 自审结论

```text
结论：通过
必须修改正式方案：无
当前可施工：否；Gate A-H 已完成并归档
原因：292 源码与 source-built 资格闭环已完成；production Editor 一致性属于后续独立 mini
```

## 23. 下一步

292 不再保留后续施工 Gate。下一步只能单独讨论：

```text
post-292 Production Editor Consistency Mini
  -> 先确认 installed availability 的用户目标与最小边界
  -> 单独生成方案/施工文档并取得 production replacement 授权
  -> 不复用 292 Gate H 授权自动替换任何安装态二进制
```

292 完成时未替换 production/installed Editor、Player、MCP 或其它二进制，未修改真实配置或真实用户缓存。
