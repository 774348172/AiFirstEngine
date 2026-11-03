# 当前可自动化施工文档：Rust Native Runtime MVP A0 / A1

> 状态提示：Rust Native Runtime A0 / A1 已完成并归档到阶段完成记录。本文档保留为历史施工依据，当前入口看 `49-当前状态与下一步入口.md`。
> 架构提示：本文档中的“第一版单线程”只记录当时的历史 MVP 做法。当前正式架构已改为第一版即按多线程线程域、并行 ECS 调度、并行 RenderExtract 设计；不得继续按本文档的单线程假设开工。
> 渲染提示：本文档中的 RenderSnapshot 只代表当时 MVP 的兼容输出。当前正式 Game -> Render 主线为 RenderExtract -> RenderCommand -> RenderSceneState，AI / Debug 默认读取 RenderFrameReport。

本文档从现有框架文档中筛选“已经确认方向、可以自动化施工、且不需要继续讨论大方向”的下一批任务。

当前结论：

```text
下一批最适合自动化施工的是 Rust Native Runtime MVP 的最小底座。
但当前本机 cargo / rustc 不可用，因此必须先做 Rust 工具链环境门禁。
Runtime Package v1 已经具备作为 Rust Runtime 第一版输入的条件。
TypeScript Runtime 不再扩展为正式 runtime。
```

## 来源文档

本施工文档依据：

```text
17-Runtime-FrameLoop.md
18-长期主义实现路径.md
21-Runtime-Core-Boundary.md
38-Rust-Native-Runtime-MVP与TypeScript退役规则.md
39-Rust-Runtime-ECS路线.md
施工文档/已完成/43-当前可自动化施工文档-Runtime-Package-v1.md
```

已经确认的关键规则：

```text
Rust Native Runtime 是唯一正式 runtime。
TypeScript Runtime 是 transition code，不能继续膨胀。
Rust Runtime 第一版读取 Runtime Package，而不是编辑器内存 Project Object。
ECS 走自研外壳 + 最小自研存储。
第一版 ECS 单线程。
第一版只做 Runtime Core，不做真实渲染、不做真实资源加载、不做 IR AOT。
Rust Runtime 正确性用 Golden Scenario Test 验证，不做 TypeScript Runtime 等价测试。
```

## 当前环境发现

当前机器执行结果：

```powershell
cargo --version
rustc --version
```

结果：

```text
cargo 不可用。
rustc 不可用。
```

因此 Rust 代码施工前必须先完成环境门禁。

## 筛选标准

可以自动化实现的任务必须满足：

```text
已有明确架构规则。
不改变 Project Schema 核心结构。
不改变 RuntimeBackend 接口。
不接编辑器 Rust Bridge。
不接真实 Native RHI / WgpuBackend。
不接真实 IR -> Rust AOT。
不接真实 Asset Cook / Binary Cooked Data。
不接真实 Android / iOS SDK / 签名。
不让 TypeScript Runtime 改读 Runtime Package。
每一步都有自动化测试。
```

必须暂停的情况：

```text
需要修改 Project Schema 核心字段。
需要修改 Runtime Package v1 字段语义。
需要让 Rust Runtime 直接读取 React / Electron 内存对象。
需要让编辑器 Preview 立刻切 Rust Runtime。
需要引入 hecs / bevy_ecs 作为正式项目 API。
需要做多线程 ECS 调度。
需要实现真实渲染、真实音频、真实物理、真实资源二进制加载。
需要让 IR 直接读写 ECS World。
需要把 TypeScript Runtime 继续作为正式 runtime 对照。
```

## 总施工顺序

推荐分两段：

```text
A0：Rust Runtime 工具链与 workspace 门禁
A1：Rust Runtime 最小纯内核
```

A0 完成后才进入 A1。  
A1 完成后再讨论 Native Editor Host MVP，不在本文档中实现。

## A0：Rust Runtime 工具链与 Workspace 门禁

### 目标

建立 Rust Runtime 的最小工程入口，但不实现复杂 runtime。

目标是让项目具备：

```text
rust/
  Cargo.toml
  crates/
    engine_runtime/
      Cargo.toml
      src/lib.rs
```

并能执行：

```powershell
cargo test
```

### 前置条件

必须先安装 Rust toolchain。

最低要求：

```text
rustc 可用。
cargo 可用。
stable toolchain 即可。
```

建议验证命令：

```powershell
rustc --version
cargo --version
```

### 允许实现

```text
新增 Rust workspace。
新增 engine_runtime crate。
新增最小 lib.rs。
新增 cargo test smoke。
新增 Node 检查脚本检测 cargo 是否可用。
新增 npm script 调用该检查脚本。
```

建议新增：

```text
rust/Cargo.toml
rust/crates/engine_runtime/Cargo.toml
rust/crates/engine_runtime/src/lib.rs
scripts/check-rust-toolchain.cjs
scripts/test-rust-runtime-workspace.cjs
```

建议 package.json 新增：

```json
"test:rusttoolchain": "node scripts/check-rust-toolchain.cjs",
"test:rustruntime": "node scripts/test-rust-runtime-workspace.cjs"
```

### 不允许实现

```text
不接 Electron。
不接 RuntimeService。
不接 sidecar。
不读取真实项目 exports。
不读取 TypeScript runtime。
不实现 ECS 复杂调度。
```

### 测试要求

```powershell
npm.cmd run test:rusttoolchain
npm.cmd run test:rustruntime
```

如果 cargo 不存在：

```text
test:rusttoolchain 必须失败并给出清晰说明。
不要继续执行 A1。
```

如果 cargo 存在：

```text
test:rustruntime 必须执行 cargo test。
```

### 完成标准

```text
Rust workspace 存在。
engine_runtime crate 可 cargo test。
Node 脚本能在 cargo 缺失时清晰失败。
不影响 npm.cmd run build。
```

## A1：Rust Runtime 最小纯内核

### 目标

实现不接编辑器、不接真实渲染的 Rust Runtime Core。

第一版只验证：

```text
Runtime Package JSON
  -> Scene Loader
  -> ECS World
  -> FrameLoop
  -> RenderSnapshot
  -> RuntimeTrace
  -> FrameHash
```

### 建议 Rust 模块

```text
rust/crates/engine_runtime/src/
  lib.rs
  ids.rs
  math.rs
  runtime_package.rs
  world.rs
  components.rs
  scene_loader.rs
  frame_loop.rs
  render_snapshot.rs
  runtime_trace.rs
  frame_hash.rs
  golden.rs
```

### 第一版数据结构

#### EntityId

```text
EntityId = string wrapper
```

原因：

```text
Runtime Package v1 已经使用稳定 id。
第一版不需要内部 numeric id 映射。
后续性能优化时可以增加内部 id 表，但不改变上层输入。
```

#### Transform

必须使用 Runtime Package 语义：

```text
localPosition
localRotation
localScale
```

禁止写入：

```text
worldPosition
worldRotation
worldScale
localMatrix
worldMatrix
```

#### Renderable

Runtime Package 输入字段：

```text
mesh.assetRef
mesh.materialRef
```

RenderSnapshot 输出字段：

```text
meshRef
materialRef
```

#### World

第一版 World 只需要：

```text
entities
transforms
renderables
hierarchy
alive state
command queue
```

#### CommandQueue

第一版只需要：

```text
spawn_entity
destroy_entity
flush
```

但 A1 可以先只实现 `destroy_entity` 和 `flush`，如果 Golden Scenario 不需要 spawn。

### FrameLoop 第一版

第一版单线程。

可见阶段：

```text
FixedUpdate
Update
LateUpdate
Render
```

内部最小流程：

```text
FrameBegin
FixedUpdate
Update
LateUpdate
RenderExtract
FrameEnd
```

A1 不做：

```text
真实物理。
真实项目规则。
真实 IR Interpreter。
真实热更 Apply。
真实多线程 Job System。
```

### RuntimeTrace 第一版

每帧至少输出：

```text
engine.frame_loop begin
engine.scene_loader load_scene
engine.render_extract end
```

每条 trace 建议字段：

```text
frame
systemId
phase
message
entityCount?
```

### RenderSnapshot 第一版

建议结构：

```text
sceneId
frame
renderables[]
```

每个 renderable：

```text
entityId
localPosition
localRotation
localScale
meshRef
materialRef
visible
layer
```

注意：

```text
RenderSnapshot 是只读输出。
RenderSnapshot 不修改 ECS。
RenderSnapshot 不保存历史。
```

### FrameHash 第一版

FrameHash 使用稳定排序后的纯数据 hash。

输入建议：

```text
frame
sceneId
entity ids
transform local values
renderable refs
alive state
```

禁止：

```text
把时间戳放入 FrameHash。
把随机数放入 FrameHash。
把日志顺序不稳定字段放入 FrameHash。
```

### Runtime Package Loader 第一版

读取：

```text
runtime-package/manifest.json
runtime-package/scenes/<scene-id>.json
runtime-package/assets/asset-manifest.json
runtime-package/rules/rule-manifest.json
```

只校验：

```text
schemaVersion
activeSceneId
scene exists
entity transform exists
mesh assetRef exists in asset manifest when primitive=model
rules.mode == none
```

不做：

```text
Prefab 展开。
Component lowering。
字段级 sourceMap。
Asset binary loading。
Rule compile。
```

### Golden Scenario Runner 第一版

第一批只跑 4 个低风险场景：

```text
empty_scene_load
single_entity_transform
renderable_snapshot
fixed_tick_10_frames
```

暂不跑：

```text
command_queue_create_destroy
projectile_lifetime_cleanup
simple_movement_rule
runtime_trace_output
```

原因：

```text
后四个会引入 command queue / gameplay rule / IR trace 边界。
A1 先验证 Runtime Package -> ECS -> Snapshot 主链路。
```

### 建议测试

Rust 侧：

```text
cargo test -p engine_runtime runtime_package_loader_loads_empty_scene
cargo test -p engine_runtime scene_loader_creates_entity_and_transform
cargo test -p engine_runtime render_extract_maps_mesh_asset_ref_to_mesh_ref
cargo test -p engine_runtime fixed_tick_produces_stable_frame_hash
cargo test -p engine_runtime invalid_runtime_package_reports_diagnostics
```

Node 侧：

```text
scripts/test-rust-runtime-golden.cjs
```

职责：

```text
先用现有 Runtime Package v1 exporter 生成 fixture。
再调用 cargo test 或 cargo run runner。
验证输出 JSON。
```

### package.json 建议新增

```json
"test:rustruntimecore": "node scripts/test-rust-runtime-core.cjs",
"test:rustruntimegolden": "node scripts/test-rust-runtime-golden.cjs"
```

### A1 必须执行

```powershell
npm.cmd run test:runtimepackage
npm.cmd run test:runtimepackagebuilder
npm.cmd run test:runtimepackageexport
npm.cmd run test:runtimepackagegolden
npm.cmd run test:rustruntimecore
npm.cmd run test:rustruntimegolden
npm.cmd run build
```

如果 Rust toolchain 不存在：

```text
只允许完成 A0 文档和环境门禁脚本。
不要伪造 Rust Runtime 测试通过。
```

## A1 完成标准

A1 完成后必须满足：

```text
Rust Runtime 能读取 Runtime Package v1。
Rust Runtime 能创建最小 ECS World。
Rust Runtime 能加载空场景。
Rust Runtime 能加载单 Entity transform。
Rust Runtime 能从 mesh.assetRef 输出 RenderSnapshot.meshRef。
Rust Runtime 能 fixed tick 10 frames 并输出稳定 FrameHash。
Rust Runtime 能输出 RuntimeTrace。
失败时能输出结构化 diagnostics。
不依赖 TypeScript Runtime。
不接 Electron。
不接编辑器 UI。
不接真实渲染。
```

## A2：暂不自动化实现

以下任务不进入本文档施工：

```text
Native Editor Host MVP。
Native Host 直接调用 Rust Runtime。
编辑器 Preview 切到 Native Host / Rust Runtime。
TypeScript Runtime 删除。
IR Interpreter 接入 Rust Runtime。
IR -> Rust AOT。
真实 Asset binary loading。
真实 Renderer / RHI。
真实 Audio / Physics。
多线程 ECS 调度。
完整 command queue create/destroy golden。
```

原因：

```text
它们会改变正式 runtime 与编辑器、IR、资源、渲染之间的边界。
必须在 A1 通过后再单独讨论施工文档。
```

## 推荐开工顺序

```text
1. 安装 Rust toolchain。
2. 执行 A0：Rust toolchain gate + workspace smoke。
3. 执行 A1-1：Runtime Package Rust 数据模型和 loader。
4. 执行 A1-2：World / Entity / Transform / Renderable 最小 ECS。
5. 执行 A1-3：Scene Loader。
6. 执行 A1-4：RenderSnapshot / RuntimeTrace / FrameHash。
7. 执行 A1-5：Golden Scenario 前 4 个场景。
8. 更新 18 / 38 / 39 / 43 文档状态。
9. 写入阶段完成记录。
```

## 最小回归命令

每个小任务完成后至少执行：

```powershell
npm.cmd run test:runtimepackage
npm.cmd run test:runtimepackagebuilder
npm.cmd run test:runtimepackageexport
npm.cmd run build
```

涉及 Rust 后执行：

```powershell
npm.cmd run test:rusttoolchain
npm.cmd run test:rustruntime
```

涉及 Golden 后执行：

```powershell
npm.cmd run test:runtimepackagegolden
npm.cmd run test:rustruntimegolden
```

## 当前结论

当前可以自动化施工：

```text
A0：Rust Runtime 工具链与 workspace 门禁。
A1：Rust Runtime 最小纯内核，限定在 Runtime Package Loader / ECS / FrameLoop / Snapshot / Trace / FrameHash / Golden 前 4 个场景。
```

当前不应该自动化施工：

```text
Native Editor Host MVP。
TypeScript Runtime 删除。
IR -> Rust AOT。
真实渲染 / 真实资源加载 / 真实平台打包。
```

