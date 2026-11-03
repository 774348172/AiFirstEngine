# 当前可自动化实施需求清单：Rust Runtime MVP A0 / A1

> 状态提示：Rust Runtime MVP A0 / A1 已完成并归档到阶段完成记录。本文档保留为历史施工依据，当前入口看 `49-当前状态与下一步入口.md`。
> 架构提示：本文档中的“第一版单线程”只记录当时的历史 MVP 做法。当前正式架构已改为第一版即按多线程线程域、并行 ECS 调度、并行 RenderExtract 设计；不得继续按本文档的单线程假设开工。
> 渲染提示：本文档中的 RenderSnapshot 只代表当时 MVP 的兼容输出。当前正式 Game -> Render 主线为 RenderExtract -> RenderCommand -> RenderSceneState，AI / Debug 默认读取 RenderFrameReport。

本文档根据当前代码库、已确认架构文档、已完成阶段记录重新筛选“现在可以由自动化直接实施、不需要继续等待用户决策”的需求。

当前结论：

```text
可以直接实施：Rust Native Runtime MVP A0 / A1。
不建议继续扩展 TypeScript Runtime。
不建议现在接 legacy Electron sidecar、真实渲染、真实资源二进制加载、IR AOT 或平台签名。
```

## 当前项目状态

已完成或已存在：

```text
Runtime Package v1 TypeScript 数据模型、生成器、导出脚本和测试。
Golden Scenario 数据层。
Input System MVP 数据层。
Editor Core 最小数据层。
Build Report 受控读取与 Evidence Summary。
Rust toolchain 已安装，cargo / rustc 可用。
```

当前缺口：

```text
rust/ workspace 尚不存在。
Rust Native Runtime 尚未落地。
Rust Runtime 还不能读取 Runtime Package。
Rust Runtime 还没有 ECS World / FrameLoop / RenderSnapshot / RuntimeTrace / FrameHash。
```

因此下一轮自动化施工应该集中在 Rust Runtime 最小闭环，而不是继续补 TypeScript 原型。

## 自动化实施原则

```text
一项需求完成后必须先跑最小测试，再进入下一项。
Rust Native Runtime 是正式 runtime 方向。
TypeScript Runtime 只作为历史 prototype，不新增正式能力。
Rust Runtime 的正确性用 Golden Scenario Test 验证，不做长期 TS Runtime 对齐测试。
第一版只做 Runtime Core，不接编辑器桥接。
第一版只读 Debug Readable Runtime Package JSON，不做 cooked binary。
第一版单线程，不做复杂 ECS 调度。
```

## 需求 1：A0 Rust Workspace 与工具链门禁

目标：

```text
建立 Rust Runtime 的最小工程入口。
保证 npm 脚本能稳定检查 cargo / rustc。
保证 engine_runtime crate 可以执行 cargo test。
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
```

建议新增 npm script：

```json
"test:rusttoolchain": "node scripts/check-rust-toolchain.cjs",
"test:rustruntime": "node scripts/test-rust-runtime-workspace.cjs"
```

最小测试：

```powershell
npm.cmd run test:rusttoolchain
npm.cmd run test:rustruntime
npm.cmd run build
```

完成标准：

```text
rust/ workspace 存在。
engine_runtime crate 存在。
cargo test 能通过。
工具链缺失时脚本能给出清晰错误。
不接 Electron。
不接 RuntimeService。
不读 TypeScript Runtime。
```

## 需求 2：A1-1 Runtime Package Rust 数据模型与 Loader

目标：

```text
Rust Runtime 能读取 Runtime Package v1 的 debug-readable JSON。
Rust Runtime 能解析 manifest / scene / asset-manifest / rule-manifest。
Rust Runtime 能返回结构化 diagnostics，而不是 panic。
```

允许新增：

```text
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/diagnostics.rs
scripts/test-rust-runtime-package-loader.cjs
```

允许修改：

```text
rust/crates/engine_runtime/Cargo.toml
rust/crates/engine_runtime/src/lib.rs
package.json
```

建议 Rust 依赖：

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

第一版读取：

```text
runtime-package/manifest.json
runtime-package/scenes/<scene-id>.json
runtime-package/assets/asset-manifest.json
runtime-package/rules/rule-manifest.json
```

第一版验证：

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

最小测试：

```powershell
npm.cmd run test:runtimepackage
npm.cmd run test:runtimepackagebuilder
npm.cmd run test:runtimepackageexport
npm.cmd run test:rustruntimepackage
npm.cmd run build
```

完成标准：

```text
Rust loader 能加载现有 runtime-package-export.cjs 输出的 package。
非法 schema / missing scene / missing transform / missing assetRef 能返回 diagnostics。
不依赖 TypeScript Runtime。
```

## 需求 3：A1-2 最小 ECS World

目标：

```text
建立 Rust Runtime 自研 ECS 外壳第一版。
只实现 Runtime Package -> ECS World 所需的最小组件。
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
Transform = localPosition / localRotation / localScale
Renderable = meshRef / materialRef / visible / layer
Hierarchy = parentId
alive state
```

第一版允许：

```text
从 Runtime Scene load entity
查询 alive renderables
按 entity id 查询 transform
```

第一版不做：

```text
archetype storage
parallel schedule
change detection
用户手写 Rust System API
复杂 command queue
```

最小测试：

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

## 需求 4：A1-3 FrameLoop / RuntimeTrace / RenderSnapshot / FrameHash

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
基于稳定排序后的 sceneId / frame / entity id / transform / renderable refs / alive state。
不包含时间戳。
不包含随机数。
不包含日志顺序这类不稳定信息。
```

最小测试：

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
RuntimeTrace 能解释基本阶段。
```

## 需求 5：A1-4 Golden Scenario 前 4 个场景

目标：

```text
用 Golden Scenario 验证 Rust Runtime，而不是对齐 TypeScript Runtime。
```

允许新增：

```text
rust/crates/engine_runtime/src/golden.rs
scripts/test-rust-runtime-golden.cjs
```

建议新增 npm script：

```json
"test:rustruntimegolden": "node scripts/test-rust-runtime-golden.cjs"
```

第一版只跑 4 个场景：

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
这些场景会提前引入 command queue / gameplay rule / IR trace 边界。
A1 只验证 Runtime Package -> ECS -> FrameLoop -> Snapshot 主链路。
```

最小测试：

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

## 需求 6：阶段文档回填

目标：

```text
把 Rust Runtime A0 / A1 的实际完成情况写入阶段完成记录。
更新长期主义实现路径中的阶段 7 状态。
```

允许新增：

```text
框架设计/引擎总体架构/阶段完成记录/<日期>-Rust-Runtime-A0-A1/00-总览.md
```

允许修改：

```text
18-长期主义实现路径.md
38-Rust-Native-Runtime-MVP与TypeScript退役规则.md
39-Rust-Runtime-ECS路线.md
施工文档/已完成/44-当前可自动化施工文档-Rust-Native-Runtime-MVP-A0-A1.md
```

完成标准：

```text
记录实际新增文件。
记录实际测试命令。
记录未完成边界。
明确下一步是否进入 Native Editor Host MVP。
```

## 当前不应自动化实施

以下需求当前不进入自动化施工：

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
真实 Android Gradle / iOS xcodebuild / signing。
真实 AI Provider 接入。
```

原因：

```text
这些会改变 runtime、编辑器桥接、IR、资源、渲染或平台边界。
它们应该等 A1 主链路通过后，再分别生成单独施工文档。
```

## 推荐执行顺序

```text
1. A0 Rust Workspace 与工具链门禁
2. A1-1 Runtime Package Rust 数据模型与 Loader
3. A1-2 最小 ECS World
4. A1-3 FrameLoop / RuntimeTrace / RenderSnapshot / FrameHash
5. A1-4 Golden Scenario 前 4 个场景
6. 阶段文档回填
```

## 最小总回归

A0 / A1 全部完成后执行：

```powershell
npm.cmd run test:runtimepackage
npm.cmd run test:runtimepackagebuilder
npm.cmd run test:runtimepackageexport
npm.cmd run test:runtimepackagegolden
npm.cmd run test:rusttoolchain
npm.cmd run test:rustruntime
npm.cmd run test:rustruntimepackage
npm.cmd run test:rustruntimegolden
npm.cmd run build
```

如果中途任何一步失败：

```text
先修复当前步骤。
不跳到下一步。
不扩大施工范围。
不临时绕过测试。
```

## 当前判断

现在可以直接自动化实施的需求只有一条主线：

```text
Rust Native Runtime MVP A0 / A1。
```

这条主线包含 5 个子需求和 1 个文档回填步骤。它正好是当前长期主义路线的下一块地基：先让正式 Rust Runtime 能读 Runtime Package、建立最小 ECS、跑 FrameLoop、吐 Snapshot / Trace / Hash，再谈编辑器桥接、IR、渲染和平台导出。

## 2026-06-22 执行状态

已按本文档完成源码实现：

```text
A0 Rust Workspace 与工具链门禁。
A1-1 Runtime Package Rust 数据模型与 Loader。
A1-2 最小 ECS World。
A1-3 FrameLoop / RuntimeTrace / RenderSnapshot / FrameHash。
A1-4 Golden Scenario 前 4 个场景。
```

已通过：

```powershell
npm.cmd run test:runtimepackage
npm.cmd run test:runtimepackagebuilder
npm.cmd run test:runtimepackageexport
npm.cmd run test:runtimepackagegolden
npm.cmd run build
```

Rust native 环境处理结果：

```text
本机默认 MSVC Rust toolchain 缺 link.exe / Windows SDK。
Visual Studio Build Tools 静默安装返回 1602。
GNU Rust toolchain 曾下载中断后缺 rustc manifest。
已清理并重装 stable-x86_64-pc-windows-gnu --profile minimal。
当前 Rust 测试脚本在无 MSVC link.exe 时自动使用 GNU toolchain。
```

详细执行记录见：

```text
阶段完成记录/2026-06-22-Rust-Runtime-A0-A1/00-总览.md
```

