# 当前可自动化实施需求清单：Rust 已就绪

> 状态提示：本文档对应的 Rust Runtime A0 / A1 自动化实施已完成。当前入口以 `49-当前状态与下一步入口.md` 为准。
> 架构提示：本文档中的“第一版单线程”只记录当时的历史 MVP 做法。当前正式架构已改为第一版即按多线程线程域、并行 ECS 调度、并行 RenderExtract 设计；不得继续按本文档的单线程假设开工。
> 渲染提示：本文档中的 RenderSnapshot 只代表当时 MVP 的兼容输出。当前正式 Game -> Render 主线为 RenderExtract -> RenderCommand -> RenderSceneState，AI / Debug 默认读取 RenderFrameReport。

本文档根据当前代码库、框架文档和本机工具链状态，重新筛选“现在可以由自动化直接实施、不需要用户继续介入决策”的需求。

当前最新状态：

```text
Rust 工具链已安装。
cargo 1.96.0 可用。
rustc 1.96.0 可用。
rust/ workspace 尚不存在。
Runtime Package v1 已完成并通过测试。
```

因此，当前最适合自动化实施的是：

```text
Rust Native Runtime MVP A0 / A1
```

## 依据文档

```text
17-Runtime-FrameLoop.md
18-长期主义实现路径.md
21-Runtime-Core-Boundary.md
38-Rust-Native-Runtime-MVP与TypeScript退役规则.md
39-Rust-Runtime-ECS路线.md
施工文档/已完成/43-当前可自动化施工文档-Runtime-Package-v1.md
施工文档/已完成/44-当前可自动化施工文档-Rust-Native-Runtime-MVP-A0-A1.md
阶段完成记录/2026-06-22-Runtime-Package-v1/00-总览.md
```

## 当前可以自动化实施的需求

### 需求 1：Rust Runtime A0 - Workspace 与工具链门禁

目标：

```text
建立 Rust Runtime 最小工程入口。
确认 cargo / rustc 可在项目脚本中稳定调用。
让后续 Rust Runtime Core 有正式 crate 落点。
```

允许新增：

```text
rust/Cargo.toml
rust/crates/engine_runtime/Cargo.toml
rust/crates/engine_runtime/src/lib.rs
scripts/check-rust-toolchain.cjs
scripts/test-rust-runtime-workspace.cjs
```

允许修改：

```text
package.json
18-长期主义实现路径.md
施工文档/已完成/44-当前可自动化施工文档-Rust-Native-Runtime-MVP-A0-A1.md
阶段完成记录/<日期>-Rust-Runtime-A0/
```

建议 npm script：

```json
"test:rusttoolchain": "node scripts/check-rust-toolchain.cjs",
"test:rustruntime": "node scripts/test-rust-runtime-workspace.cjs"
```

必须测试：

```powershell
npm.cmd run test:rusttoolchain
npm.cmd run test:rustruntime
npm.cmd run build
```

完成标准：

```text
Rust workspace 存在。
engine_runtime crate 存在。
cargo test 能通过。
脚本能输出 rustc / cargo 版本。
不接 Electron。
不接 RuntimeService。
不接 TypeScript Runtime。
```

### 需求 2：Rust Runtime A1-1 - Runtime Package Rust 数据模型与 Loader

目标：

```text
Rust 能读取 Runtime Package v1 debug-readable JSON。
Rust 能解析 manifest / scene / asset-manifest / rule-manifest。
Rust 能做第一版结构校验。
```

允许新增 Rust 模块：

```text
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/diagnostics.rs
```

允许新增 Node 测试：

```text
scripts/test-rust-runtime-package-loader.cjs
```

允许修改：

```text
package.json
```

建议 npm script：

```json
"test:rustruntimepackage": "node scripts/test-rust-runtime-package-loader.cjs"
```

第一版必须读取：

```text
runtime-package/manifest.json
runtime-package/scenes/<scene-id>.json
runtime-package/assets/asset-manifest.json
runtime-package/rules/rule-manifest.json
```

第一版必须验证：

```text
schemaVersion == runtime-package.v1
activeSceneId 存在
scene 文件存在
entity transform.localPosition / localRotation / localScale 存在
model mesh.assetRef 在 asset manifest 中存在
rules.mode == none
```

第一版不做：

```text
Prefab 展开
Component lowering
字段级 sourceMap
真实资源二进制加载
Rule compile
IR AOT
```

必须测试：

```powershell
npm.cmd run test:runtimepackage
npm.cmd run test:runtimepackagebuilder
npm.cmd run test:runtimepackageexport
npm.cmd run test:rustruntimepackage
npm.cmd run build
```

完成标准：

```text
Rust loader 可以加载由现有 runtime-package-export.cjs 输出的 package。
非法 schema / missing scene / missing transform / missing assetRef 能返回结构化 diagnostics。
不依赖 TypeScript Runtime。
```

### 需求 3：Rust Runtime A1-2 - 最小 ECS World

目标：

```text
建立 Rust Runtime 自研 ECS 外壳第一版。
只做 Entity / Transform / Renderable / Hierarchy / Alive 状态。
```

允许新增：

```text
rust/crates/engine_runtime/src/ids.rs
rust/crates/engine_runtime/src/math.rs
rust/crates/engine_runtime/src/components.rs
rust/crates/engine_runtime/src/world.rs
rust/crates/engine_runtime/src/scene_loader.rs
```

第一版 World 只包含：

```text
EntityId = string wrapper
Transform
Renderable
Hierarchy parentId
alive state
```

第一版允许：

```text
spawn/load entity from scene
query all alive renderables
query transform by entity id
```

第一版不做：

```text
archetype storage
parallel schedule
change detection
用户手写 Rust System API
复杂 command queue
```

必须测试：

```powershell
npm.cmd run test:rustruntime
cargo test -p engine_runtime scene_loader_creates_entity_and_transform
cargo test -p engine_runtime world_queries_alive_renderables
npm.cmd run build
```

完成标准：

```text
Runtime Package scene 能转成 ECS World。
Transform 使用 localPosition / localRotation / localScale。
Renderable 从 mesh.assetRef / mesh.materialRef 得到。
不暴露 hecs / bevy_ecs API。
```

### 需求 4：Rust Runtime A1-3 - FrameLoop / RuntimeTrace / RenderSnapshot / FrameHash

目标：

```text
实现第一版单线程 FrameLoop。
输出 RenderSnapshot、RuntimeTrace、FrameHash。
```

允许新增：

```text
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/render_snapshot.rs
rust/crates/engine_runtime/src/runtime_trace.rs
rust/crates/engine_runtime/src/frame_hash.rs
```

第一版 FrameLoop：

```text
FrameBegin
FixedUpdate
Update
LateUpdate
RenderExtract
FrameEnd
```

第一版 RuntimeTrace 至少包含：

```text
engine.frame_loop begin
engine.scene_loader load_scene
engine.render_extract end
```

第一版 RenderSnapshot：

```text
sceneId
frame
renderables[]
  entityId
  localPosition
  localRotation
  localScale
  meshRef
  materialRef
  visible
  layer
```

第一版 FrameHash：

```text
基于稳定排序的 sceneId / frame / entity id / transform / renderable refs / alive state。
不包含时间戳。
不包含随机数。
```

必须测试：

```powershell
cargo test -p engine_runtime render_extract_maps_mesh_asset_ref_to_mesh_ref
cargo test -p engine_runtime fixed_tick_produces_stable_frame_hash
cargo test -p engine_runtime runtime_trace_records_frame_phases
npm.cmd run build
```

完成标准：

```text
Rust Runtime 可以 tick 固定帧。
同输入产生稳定 FrameHash。
RenderSnapshot 不修改 ECS。
RuntimeTrace 可解释基本阶段。
```

### 需求 5：Rust Runtime A1-4 - Golden Scenario 前 4 个场景

目标：

```text
用 Golden Scenario 验证 Rust Runtime，而不是对齐 TypeScript Runtime。
```

第一版只跑：

```text
empty_scene_load
single_entity_transform
renderable_snapshot
fixed_tick_10_frames
```

允许新增：

```text
rust/crates/engine_runtime/src/golden.rs
scripts/test-rust-runtime-golden.cjs
```

建议 npm script：

```json
"test:rustruntimegolden": "node scripts/test-rust-runtime-golden.cjs"
```

测试流程：

```text
Node 生成 Runtime Package fixture。
Rust Runtime 加载 fixture。
Rust Runtime 输出 snapshot / trace / hash。
Node 或 Rust 断言 expected。
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
这些会引入 command queue / IR / gameplay rule 边界。
A1 只验证 Runtime Package -> ECS -> FrameLoop -> Snapshot 主链路。
```

必须测试：

```powershell
npm.cmd run test:runtimepackagegolden
npm.cmd run test:rustruntimegolden
npm.cmd run build
```

完成标准：

```text
前 4 个 Golden Scenario 在 Rust Runtime 下通过。
失败时能输出 diagnostics。
不需要 TypeScript Runtime 作为 oracle。
```

## 当前不应自动化实施的需求

以下任务仍然需要等 A1 通过后再讨论：

```text
Native Editor Host MVP。
Native Host 直接调用 Rust Runtime。
编辑器 Preview 切到 Native Host / Rust Runtime。
TypeScript Runtime 删除。
IR Interpreter 接入 Rust Runtime。
IR -> Rust AOT。
真实 Asset binary loading。
真实 Renderer / RHI / WgpuBackend。
真实 Audio / Physics。
多线程 ECS 调度。
完整 command queue create/destroy golden。
真实 Android Gradle / iOS xcodebuild / 签名。
真实 AI Provider 接入。
```

原因：

```text
这些会改变底层 runtime、编辑器桥接、IR、资源、渲染或平台边界。
如果在 A1 之前做，会把原型结构过早固化。
```

## 推荐实施顺序

```text
1. Rust Runtime A0 - Workspace 与工具链门禁
2. Rust Runtime A1-1 - Runtime Package Rust 数据模型与 Loader
3. Rust Runtime A1-2 - 最小 ECS World
4. Rust Runtime A1-3 - FrameLoop / RuntimeTrace / RenderSnapshot / FrameHash
5. Rust Runtime A1-4 - Golden Scenario 前 4 个场景
6. 文档回填和阶段完成记录
```

## 每步必须遵守的纪律

```text
一次只做一个子需求。
每个子需求新增自动化测试。
先跑本子需求测试，再跑必要回归。
不把 TypeScript Runtime 当成 Rust Runtime 的正确性标准。
不引入第三方 ECS 作为项目 API。
不接编辑器桥接。
不接真实渲染和真实资源二进制。
```

## 最小回归命令

每个 Rust 子需求完成后至少执行：

```powershell
npm.cmd run test:runtimepackage
npm.cmd run test:runtimepackagebuilder
npm.cmd run test:runtimepackageexport
npm.cmd run test:rustruntime
npm.cmd run build
```

A1-4 完成后执行：

```powershell
npm.cmd run test:runtimepackagegolden
npm.cmd run test:rustruntimegolden
npm.cmd run build
```

## 当前结论

现在不需要继续讨论，可以直接开工的需求是：

```text
Rust Runtime A0。
Rust Runtime A1-1。
Rust Runtime A1-2。
Rust Runtime A1-3。
Rust Runtime A1-4。
```

这些需求全部属于 Rust Native Runtime MVP 的最小闭环，不碰编辑器桥接、不碰真实渲染、不碰 IR AOT。

