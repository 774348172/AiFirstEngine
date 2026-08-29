# Rust Native Runtime MVP 与 TypeScript Runtime 退役规则

## 当前归属说明：Projection 术语

本文中如果出现以下历史名称：

```text
RenderExtract
RenderAssetBridge / Render Asset Bridge
Physics2DBridge
RuntimeScene Hydration
AuiRenderExtract / AuiRendererBridge
SpriteRenderer2D ECS-to-RenderProxy Bridge
```

统一按 `110-World-Projection-Adapter统一跨域同步规则.md` 理解为：

```text
RenderProjection
AssetProjection
Physics2DProjection
HydrationProjection
UiProjection
RenderProjectionAdapter<SpriteRenderer2D>
```

这些名称可以作为历史实现名保留，但不再作为新增架构概念扩展。后续新增类型只新增对应 `ProjectionAdapter`，不新增独立 Bridge。

本文档确认下一阶段 runtime 路线。

核心结论：

```text
下一阶段进入 Rust Native Runtime MVP。
Rust Native Runtime 是唯一正式 runtime。
第一版只做最小 runtime 内核。
TypeScript Runtime 只是早期编辑器测试验证用 prototype。
Rust Runtime MVP 跑通后，TypeScript Runtime 进入退役和删除流程。
不建立长期 TypeScript Runtime vs Rust Runtime 等价测试。
Rust Runtime 的验证标准改为 Golden Scenario Test。
编辑器接入主线改为 Native Rust Editor Host direct boundary。
Electron / React 只作为 legacy transition shell，不再作为 Runtime Bridge 主线。
```

## 为什么不保留 TypeScript Runtime

如果继续保留 TypeScript Runtime，会形成两套 runtime：

```text
TypeScript Runtime
Rust Native Runtime
```

这会带来长期风险：

```text
两套系统行为会慢慢分叉。
AI 和用户不知道哪个 runtime 才是真相。
编辑器流程可能继续依赖 TypeScript 旧行为。
Rust Runtime 会被 prototype 的历史行为绑架。
维护成本翻倍。
Bug 定位会先问“是哪套 runtime 的问题”，增加复杂度。
```

因此 TypeScript Runtime 不能成为长期 backend，也不能成为 Rust Runtime 的标准答案。

正式标准必须来自：

```text
Project Schema
Scene Data
Component Schema
Canonical Rule IR
FrameLoop Spec
Expected Trace
Expected RenderFrameReport
Expected State Hash
Golden Scenario Test
```

## TypeScript Runtime 的定位

TypeScript Runtime 当前定位：

```text
早期编辑器 prototype。
早期 Schema / Patch / Trace / Replay / UI 流程验证工具。
Rust Runtime 出现前的临时运行路径。
```

从本文档确认后，TypeScript Runtime 冻结为 transition code。

允许：

```text
修复阻塞当前编辑器启动的 bug。
补充迁移所需的 fixture。
保留已有测试作为历史参考。
辅助生成 Golden Scenario 的初始样例，但不能作为标准答案。
```

不允许：

```text
继续新增正式玩法能力。
继续接入新的长期 runtime system。
继续作为最终导出 runtime。
继续作为 Rust Runtime 对照 backend。
继续扩展成第二套正式 ECS / FrameLoop / Renderer。
```

## Rust Native Runtime MVP 的定位

Rust Runtime 是唯一正式 runtime。

第一版目标不是完整游戏引擎，而是跑通最小正式内核：

```text
Project Schema -> Rust Runtime -> ECS -> FrameLoop -> RuntimeTrace / RenderFrameReport / FrameHash -> Golden Scenario Test
```

第一版必须包含：

```text
Rust crate: engine_runtime
Project Schema loader
Scene loader
Entity allocator
Component storage
Transform component
Renderable marker / simple render data
Fixed FrameLoop
Command queue
RuntimeTrace
FrameHash
RenderFrameReport / SnapshotView output
Golden Scenario runner
```

## Runtime 加载与转换边界

Project Schema Loader 是运行时项目数据入口，不是项目逻辑编译器。

正式规则：

```text
Project Schema Loader 负责读取、校验、转换项目结构数据。
Project Schema Loader 不负责把 DSL / IR / Schema 转换成 Rust 代码。
Project Schema Loader 不负责运行 AI 生成流程。
Project Schema Loader 不负责现场编译项目逻辑。
```

项目逻辑规则走独立链路：

```text
Feature Spec / DSL / Graph
  -> Canonical Rule IR
  -> 开发 / 验证 / 热更覆盖：IR Interpreter
  -> 发布构建：Rust AOT / compiled rule module
```

重型转换必须发生在编辑器、Build Graph 或热更包生成阶段：

```text
DSL -> IR
IR validation
IR -> Rust AOT
Rust codegen / compile
Scene cook
Asset cook
Bundle pack
```

运行时只允许做轻量工作：

```text
读取 cooked project data
校验 schemaVersion / manifest / hash
加载 cooked scene data
创建 ECS World
加载已编译 rule module
加载必要资源引用和 bundle manifest
启动 FrameLoop
输出 RuntimeTrace / RenderFrameReport / FrameHash
```

禁止：

```text
运行时启动时全量解析 DSL 并生成 IR。
运行时启动时把 IR 现场转换成 Rust。
运行时启动时调用 Rust 编译器生成平台代码。
运行时每次进入场景都全量重编项目规则。
运行时把编辑器内部对象当成正式项目输入。
```

模式区分：

```text
Editor / Debug Mode:
  可以读取可读 JSON / Schema，方便 AI 解释、调试和 Golden Scenario 验证。

Release Mode:
  应读取 cooked binary / compact data、manifest 和已编译 rule module，避免启动卡顿。
```

## Runtime Package 输入规则

Rust Native Runtime MVP 第一版不直接读取编辑器内部 Project Object。

第一版输入采用：

```text
Debug Readable Runtime Package
```

它是 Runtime 可读取的包格式，不是编辑器内存对象，也不是最终 Release binary cooked data。

推荐结构：

```text
runtime-package/
  manifest.json
  scenes/
    scene-main.json
  assets/
    asset-manifest.json
  rules/
    rule-manifest.json
  golden/
    scenario-*.json
```

第一版 Runtime Package 可以使用 JSON，原因：

```text
方便 Golden Scenario 验证。
方便 AI 和用户阅读构建输入。
方便 diagnostics 标注 path。
方便第一版 Rust Runtime 快速验证 Schema -> ECS -> FrameLoop。
不把 Runtime 绑定到编辑器内部对象。
```

Runtime Package v1 的正式定位：

```text
Runtime Package v1 = Normalized Project Runtime View
```

它不是复杂 Cook 后的第二套项目数据，也不是字段级 sourceMap 系统。  
第一版应通过减少转换链路来降低编辑器和 Runtime 行为不一致的风险。

第一版只允许做三类处理：

```text
Normalize:
  排序、补默认值、稳定 hash、统一必要字段形态。

Strip:
  删除 editor-only 数据，例如面板状态、选中状态、AI 对话、Inspector 展开状态。

Validate:
  检查 scene / entity / component / assetRef / manifest 是否合法。
```

第一版不做：

```text
复杂 Prefab 展开。
复杂 Component lowering。
字段级 sourceMap。
复杂 provenance 系统。
复杂 cooked binary。
运行时 rule compile。
```

编辑器预览规则：

```text
Editor Play / Preview 必须走 Runtime Package。
编辑器不应直接把内存 Project Object 交给 Runtime Preview。
导出运行和编辑器预览应尽量使用同一种 Runtime Package 输入。
```

查错规则：

```text
第一版依赖稳定 id 和对象级 Build Report 查问题。
runtime scene scene-main 来自 project scene scene-main。
runtime entity entity-player 来自 project entity entity-player。
runtime asset asset-ship 来自 project asset asset-ship。
```

第一版不要求字段级 sourceMap，原因：

```text
规则复杂度过高。
Runtime Package 与 Project Schema 保持同构后，大多数字段可直接通过稳定 id 和路径查回。
第一版更需要验证主链路，而不是提前建立重型追踪系统。
```

Runtime Package v1 字段规范化规则：

```text
大部分字段尽量继承 Project Schema 命名。
Transform 例外，Runtime Package v1 提前采用运行时语义字段。
```

Transform 规则：

```text
Project Schema:
  transform.position
  transform.rotation
  transform.scale

Runtime Package v1:
  transform.localPosition
  transform.localRotation
  transform.localScale
```

原因：

```text
Runtime / ECS / Hierarchy 内部需要明确 local transform。
world transform / matrix 由 Runtime 计算和缓存，不写入 Runtime Package v1。
提前使用 local* 可以避免后续接入父子层级、动画、物理和渲染抽取时再迁移字段语义。
```

转换边界：

```text
Project transform.position -> Runtime Package transform.localPosition
Project transform.rotation -> Runtime Package transform.localRotation
Project transform.scale -> Runtime Package transform.localScale
```

这个转换属于 Normalize，不属于复杂 Component lowering。

Renderable / mesh 规则：

```text
Runtime Package Entity 渲染字段固定使用 mesh.assetRef / mesh.materialRef。
RenderExtract 负责从 mesh.assetRef / mesh.materialRef 解析为 RenderCommand payload 或 RenderSceneState 所需的渲染资源句柄。
RenderFrameReport 只输出 AI / Debug 可读的 resolved / fallback / missing 摘要。
Runtime Package v1 不写入 meshRef / materialRef 这类渲染后端句柄。
```

禁止：

```text
Runtime Package v1 同时混用 mesh.assetRef 和 mesh.meshRef。
Runtime Package v1 写入 worldPosition / worldRotation / worldScale。
Runtime Package v1 写入 worldMatrix / localMatrix。
```

长期规则：

```text
Editor / Debug Mode 可以读取 Debug Readable Runtime Package。
Release Mode 应读取 cooked binary / compact data。
二者必须共享同一套语义模型和 manifest 规则。
```

第一版 Runtime Package 必须包含：

```text
manifest.json
scene json
entity list
Transform component data
Renderable component data
Hierarchy data
asset ref manifest
empty rule manifest
golden scenario input / expected
```

第一版 Runtime Package 不包含：

```text
真实资源二进制
真实 shader
真实 audio
真实 rule module
真实热更 patch
真实平台 signing 信息
```

正式链路：

```text
Project Schema
  -> Build Graph / Runtime Package Export
  -> Debug Readable Runtime Package
  -> Rust Project Schema Loader
  -> Scene Loader
  -> ECS World
  -> FrameLoop
```

禁止：

```text
Rust Runtime 直接读取 React / Electron 内存对象。
Rust Runtime 直接依赖 src/App.tsx 的数据结构。
Rust Runtime 把编辑器临时 UI 状态当成项目输入。
```

## Runtime Snapshot 保存边界

Rust Runtime MVP 必须支持 Snapshot 输出，但 Snapshot 是纯数据只读视图，不等于默认持久化存档。

核心规则：

```text
Runtime 每帧可以生成 Snapshot。
Runtime 每帧必须能生成 FrameHash。
Runtime 默认不保存完整 Snapshot 历史。
RenderFrameReport 可以每帧临时输出给调试 / AI / 编辑器摘要使用，不默认进入历史。
DebugSnapshot 只在验证失败、Runtime 报错、AI Patch 行为变化、用户录制时保存。
Full State Snapshot 只允许由 checkpoint / rollback / deep debug / Golden Scenario 显式触发。
```

Snapshot 分层：

```text
SnapshotView
  只作为旧 MVP / Debug 视图，包含渲染和编辑器显示所需摘要，例如 Transform、Renderable、Camera、Light、UI render data。

RenderFrameReport
  只保存 AI / Debug 可读的本帧渲染摘要、降级、变化计数和 trace 引用，不作为渲染主流程输入。

FrameHash
  只保存稳定 hash，用于 Replay、Golden Scenario、AI Patch review 判断行为是否变化。

DebugSnapshot
  保存局部 ECS / Component / Trace 证据，用于 AI 查 Bug 和用户审查。

Full State Snapshot
  保存完整或接近完整 Runtime 状态，成本高，禁止默认逐帧保存。
```

用途：

```text
编辑器 Game / Scene 视图显示当前运行状态。
Renderer 消费 RenderCommand / RenderSceneState 绘制当前帧。
Golden Scenario 根据 Trace / RenderFrameReport / FrameHash 验证 runtime 正确性。
Replay 根据 FrameHash 定位分歧帧，必要时展开 DebugSnapshot。
AI 根据 Snapshot + RuntimeTrace 解释为什么某个结果发生或没有发生。
```

禁止：

```text
默认逐帧保存完整 ECS World。
默认逐帧保存所有 Entity / Component。
把完整 Snapshot 历史写入 Project Data。
把 Replay Debug Package 当成项目数据长期保存。
```

第一版不做：

```text
真实 GPU 渲染
真实资源 Cook
真实物理
真实 Audio
真实 UI
完整 IR AOT
完整热更
完整平台打包
复杂玩法系统
```

## 不做 TypeScript vs Rust 等价测试

不再把 TypeScript Runtime 与 Rust Runtime 的等价测试作为长期要求。

原因：

```text
TypeScript Runtime 是 prototype，不是规格。
Rust Runtime 不应该对齐 prototype 的历史行为。
对齐 prototype 会把临时实现固化成长期约束。
```

因此废弃这类目标：

```text
TypeScript backend vs Rust backend equivalence
TypeScript runtime 作为所有后续 runtime 的对照 backend
Rust Runtime 必须输出与 TypeScript backend 等价的结果
编辑器长期可选择 TypeScript backend 或 Rust backend
```

## Golden Scenario Test

Rust Runtime 的通过标准是 Golden Scenario Test。

Golden Scenario Test 的含义：

```text
用标准场景和明确期望验证 runtime 是否符合引擎规格。
不是验证 Rust 是否等于 TypeScript。
```

每个 scenario 包含：

```text
scenarioId
project schema input
scene input
input frames
expected entity count
expected component values
expected events
expected runtime trace records
expected render snapshot
expected frame hash
expected diagnostics
```

第一批 Golden Scenario：

```text
1. empty_scene_load
2. single_entity_transform
3. renderable_snapshot
4. fixed_tick_10_frames
5. command_queue_create_destroy
6. projectile_lifetime_cleanup
7. simple_movement_rule
8. runtime_trace_output
```

通过标准：

```text
Rust Runtime 加载 scenario。
Rust Runtime 按 input frames 运行。
输出和 expected 完全一致。
diagnostics 可解释失败原因。
trace 能定位到 system / rule / component path。
```

## TypeScript Runtime 退役路线

退役分阶段执行。

### 阶段 0：冻结

```text
不再新增 TypeScript Runtime 正式能力。
不再把新玩法迁移到 TypeScript Runtime。
不再用 TypeScript Runtime 定义 runtime 正确性。
```

### 阶段 1：Rust Runtime MVP 跑通空场景

```text
Project Schema loader
Scene loader
Entity allocator
Transform component
fixed tick
```

### 阶段 2：Rust Runtime 输出可观察结果

```text
RenderFrameReport
RuntimeTrace
FrameHash
diagnostics
```

### 阶段 3：Rust Runtime 跑通 Golden Scenario Test

```text
至少通过第一批 Golden Scenario。
失败能输出结构化 diagnostics。
```

### 阶段 4：编辑器 Preview 切到 Rust Runtime

```text
Native Editor Host 调用 Rust Runtime。
RuntimeService 通过 native boundary / crate boundary 访问 Rust Runtime。
Native UI Backend 显示 RenderFrameReport / RuntimeTrace / FrameHash。
Legacy React / Electron 如果保留预览，只能作为 compatibility bridge。
Viewport 不再依赖 TypeScript Runtime。
```

主线不再是：

```text
Electron sidecar + JSON/RPC
```

可选兼容路径仅用于 legacy shell：

```text
Electron compatibility bridge
sidecar / IPC fallback
```

### 阶段 5：删除 TypeScript Runtime 运行路径

```text
删除 TypeScript Runtime Backend 的编辑器入口。
删除 TypeScript Runtime 作为 runtime backend 的选择项。
保留必要 scenario fixture / schema fixture / report fixture。
```

### 阶段 6：清理旧测试和文档

```text
把 TypeScript Runtime 相关测试迁移为 Golden Scenario Test。
把仍有参考价值的旧文档移入历史文档。
确保 README / 施工文档不再把 TypeScript Runtime 作为长期 backend。
```

## 编辑器接入原则

编辑器主线改为 Native Rust Editor Host。

Runtime Preview 主线必须走：

```text
Native Rust Editor Host
  -> RuntimeService
  -> engine_runtime crate / native boundary
  -> Rust Native Runtime
  -> RenderFrameReport / RuntimeTrace / StateHash
  -> Native UI Backend 展示
```

Electron / React 定位：

```text
legacy transition shell
```

legacy shell 如需显示 Rust Runtime 结果，只能走兼容适配：

```text
Legacy Electron Shell
  -> compatibility bridge
  -> RuntimeService / Rust Runtime
```

禁止：

```text
React 直接调用 runtime 内部函数。
Electron main 直接承载 runtime 逻辑。
TypeScript Runtime 继续作为 Preview 真相。
围绕 Electron sidecar 设计正式 Runtime Bridge。
```

## 和 IR Interpreter / Rust AOT 的关系

本文档废弃的是：

```text
TypeScript Runtime vs Rust Runtime 等价测试
```

不废弃：

```text
受限 RuleSlot / Canonical Rule IR 的语义验证。
受限验证执行路径与 Rust AOT 的规则语义一致性验证。
```

区别：

```text
TypeScript Runtime 是 prototype，不是标准。
Gameplay Rule Asset / Contract-bound RuleSlot 是用户和 AI 面向的规则资产边界。
Canonical Rule IR 是受限 RuleSlot 的内部规范语义和构建输入，可以作为受限验证执行路径和 AOT 的共同规格。
```

因此后续仍允许：

```text
同一份 Canonical Rule IR 在解释执行和 Rust AOT 下保持语义一致。
```

但 Rust Runtime 本身不对齐 TypeScript Runtime。

## 当前确认后的结论

```text
下一阶段进入 Rust Native Runtime MVP。
Rust Native Runtime 是唯一正式 runtime。
TypeScript Runtime 冻结为 transition code。
Rust Runtime MVP 通过后删除 TypeScript Runtime 运行路径。
不做 TypeScript Runtime vs Rust Runtime 长期等价测试。
用 Golden Scenario Test 定义 Rust Runtime 正确性。
编辑器 Preview 最终通过 Native Editor Host 调 Rust Runtime。
Electron / React 仅作为 legacy transition shell。
```
