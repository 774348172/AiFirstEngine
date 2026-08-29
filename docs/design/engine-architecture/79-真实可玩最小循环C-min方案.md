# 真实可玩最小循环 C-min 方案

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

本文档定义真实可玩最小循环的长期架构边界。

C-min 的含义：

```text
采用最终生产级 runtime 架构边界。
第一版只实现最小功能。
不为了快而绕过 EngineHostLoop / Runtime Game Thread / Render Thread / RenderCommand / RDG / RHI。
```

## 设计问题

当前已经完成：

```text
Runtime Package v1。
Runtime 资源加载系统 v1。
Scene / Prefab / Entity Runtime 实例化 v1。
Rust ECS Storage v1。
ProjectLogicRunner / LogicExecutor / Rust ECS 接入 v1。
RenderCommand / RenderSceneState v1。
RuntimeRenderer / RDG / RHI / HeadlessRhiBackend C-min。
真实 Rust Runtime CLI / Process Spawn C-min。
真实 Wgpu Surface / GPU Texture Lifetime / RenderThread C-min。
真实 Windowed Runtime / Viewport Present C-min。
OS File Watcher / Import Worker C-min。
GPU Upload C-min。
Bundle Binary Pack C-min。
最小游戏端到端 headless gate。
```

但这些能力仍然需要被收敛成一个默认可玩的连续窗口循环：

```text
Native Window
  -> Input Pump
  -> InputActionSnapshot
  -> EngineHostLoop
  -> Runtime / Game Thread
  -> ECS World
  -> ProjectLogicRunner
  -> RenderExtract
  -> RenderCommandQueue
  -> Render Thread
  -> RenderSceneState
  -> RuntimeRenderer
  -> RDG
  -> RHI
  -> Surface Present
  -> FrameReport / Trace
```

## 其它引擎参考

### Unreal Engine

UE 以 `FEngineLoop::Tick` 作为最高层心跳源。

典型流程：

```text
Platform Pump Messages
  -> Game Thread Tick
  -> World / Actor / Component Tick
  -> ENQUEUE_RENDER_COMMAND
  -> Render Thread
  -> RHI
  -> Present
```

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Launch\Private\LaunchEngineLoop.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\RenderCore\Private\RenderingThread.cpp
```

对我们的启发：

```text
最高心跳源必须归引擎。
Game Thread 和 Render Thread 必须隔离。
Game -> Render 通过命令或可控同步边界，不共享渲染状态。
```

### Unity

Unity 通过 PlayerLoop 把运行时阶段组织成用户可理解的结构。

典型用户心智：

```text
Input / EarlyUpdate
  -> Update
  -> LateUpdate
  -> Render
  -> Present
```

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Runtime\Export\PlayerLoop\PlayerLoop.bindings.cs
```

对我们的启发：

```text
用户和 AI 应该理解一帧的顺序。
不要把业务顺序藏进复杂 scheduler。
```

### Bevy

Bevy 使用 winit runner 驱动 App update。
渲染侧通过 ExtractSchedule 从 Main World 抽取数据到 RenderApp。

典型流程：

```text
winit EventLoop
  -> App::update
  -> Main Schedule
  -> ExtractSchedule
  -> RenderApp
  -> wgpu present
```

源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_winit\src\state.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_render\src\extract_plugin.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_render\src\lib.rs
```

对我们的启发：

```text
Rust / ECS 项目适合 Main World -> Render World / Render State 的抽取模型。
headless 和 windowed 应该走同一条 app/update/extract/render 结构。
```

### Godot

Godot 以 MainLoop / SceneTree iteration 推动节点处理、物理和渲染服务。

对我们的启发：

```text
引擎主循环需要稳定、集中、可观测。
但我们不采用节点脚本主线，而采用 Rust ECS + ProjectLogicRunner。
```

## 方案对比

| 方案 | 内容 | 优点 | 问题 |
|---|---|---|---|
| A：headless 为主，真实窗口只 smoke | 默认测试还是 headless，真实窗口可选 | 简单 | 不能证明产品真的可玩 |
| B：真实可玩循环 + headless gate | 真实窗口连续 loop 是主线，headless 做自动化等价门禁 | 平衡 | 仍可能保留部分以后正规化的临时边界 |
| C-full：完整生产 runtime | 多窗口、完整 GPU pool、完整跨线程队列、完整平台发布一次做完 | 长期最完整 | 第一版过大，容易拖死施工 |
| C-min：最终边界 + 最小功能 | 第一版就采用最终架构边界，每个模块只做最小闭环 | 最符合长期主义 | 需要严格控制功能范围 |

## 最终选择

采用 C-min。

规则：

```text
C-min 不是做完整生产级功能。
C-min 是第一版就采用最终生产级架构边界。
C-min 的每个模块只实现最小功能。
```

## C-min 标准链路

```text
Native Window
  -> EngineHostLoop
  -> Input Pump
  -> InputActionSnapshot
  -> Runtime / Game Thread
  -> ECS World
  -> ProjectLogicRunner
  -> RenderExtract
  -> RenderCommandQueue
  -> Render Thread
  -> RenderSceneState
  -> RuntimeRenderer
  -> RDG / RHI
  -> Surface Present
  -> FrameReport / Trace
```

## 第一版功能范围

第一版必须支持：

```text
一个真实窗口。
一个 Runtime Viewport。
一个 Runtime Scene。
连续运行 N 帧。
最小 InputActionSnapshot。
一个可移动 Entity。
ProjectLogicRunner 根据输入修改 Transform。
RenderExtract 生成 Transform 相关 RenderCommand。
RenderThread 消费 RenderCommand 并更新 RenderSceneState。
RuntimeRenderer 通过 RDG / RHI 输出到 Surface。
每帧生成结构化 report。
headless gate 走同一条逻辑链路。
```

第一版不支持：

```text
多窗口。
多 viewport。
完整编辑器 Play Mode。
完整 GPU Resource Pool。
完整跨线程 RenderCommand 队列优化。
完整材质 / shader / pipeline cache。
完整场景切换。
完整热更。
完整资源 streaming。
真实 Android / iOS / Web 发布。
复杂 RenderGraph 优化。
```

## 线程与所有权规则

```text
EngineHostLoop 是最高心跳源。
OS window event 只提供事件，不拥有引擎主循环。
Input Pump 只生成输入事件和 InputActionSnapshot。
Runtime / Game Thread 是 ECS World 唯一写入 owner。
项目逻辑只通过 ProjectLogicRunner / LogicExecutor 进入 ECS。
RenderExtract 是 Game -> Render 的唯一边界。
Render Thread 拥有 RenderSceneState。
RuntimeRenderer 不直接读取 ECS。
RHI / Backend 不理解项目 ECS 和项目规则。
```

跨线程域只允许传：

```text
Command
Snapshot
Report
TaskResult
Handle
```

禁止：

```text
Runtime 直接写 RenderSceneState。
RenderThread 直接读写 ECS World。
RuntimeRenderer 绕过 RDG / RHI 直接 present。
为了真实窗口单独造一条不同于 headless 的逻辑链路。
```

## Headless 测试规则

真实窗口能力必须有 headless 等价门禁。

规则：

```text
headless gate 不等于假 runtime。
headless gate 必须走同一套 EngineHostLoop / InputActionSnapshot / RuntimeFrame / RenderExtract / RenderThread / RuntimeRenderer 边界。
headless backend 可以替换真实 Surface，但不能替换主流程。
真实窗口 smoke 可以 ignored，但默认 CI gate 必须 headless 可跑。
```

## Report / Trace 规则

每次连续运行必须输出结构化报告：

```text
WindowedContinuousRuntimeReport:
  ok
  frame_count
  input
  runtime
  logic
  ecs
  render_extract
  render_thread
  renderer
  rdg
  rhi
  present
  diagnostics[]
```

最小诊断字段：

```text
system
stage
severity
code
message
frame_index
entity_id?
asset_id?
command_id?
```

报告用于：

```text
AI 查错。
用户理解失败位置。
自动化测试断言。
未来 RuntimeTrace 面板展示。
```

报告不用于：

```text
驱动项目逻辑。
替代 ECS 状态。
替代 RenderSceneState。
```

## 第一版验收标准

```text
1. 能打开真实窗口并连续运行 N 帧。
2. headless gate 能连续运行同样的 N 帧。
3. 输入事件能进入 InputActionSnapshot。
4. ProjectLogicRunner 能根据输入修改一个 Entity 的 Transform。
5. RenderExtract 能产生对应 RenderCommand。
6. RenderThread 能更新 RenderSceneState。
7. RuntimeRenderer 能提交到 Surface 或 HeadlessSurfaceBackend。
8. 每帧 report 能说明 input / logic / ecs / render / present 的状态。
9. resize / surface lost / acquire failure / present failure 有结构化 diagnostics。
10. 默认测试不依赖真实 GPU / 真实窗口。
```

## 与现有文档关系

本方案不替代以下文档，而是把它们串成真实可玩连续循环：

```text
17-Runtime-FrameLoop.md
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
40-Input-System路线.md
50-RenderCommand-RenderSceneState方案.md
51-RenderDirtyTracker-RenderExtract-RenderCommand闭环方案.md
59-真实WgpuBackend-RDG-RHI最小门禁方案.md
68-Runtime资源加载系统方案.md
70-Scene-Prefab-Entity-Runtime实例化方案.md
71-Runtime-Viewport-Rendering-System方案.md
72-Build-Run-Package-Orchestrator-v1方案.md
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
75-真实RustRuntimeCLI-ProcessSpawn方案.md
```

后续施工文档：

```text
施工文档/当前/79-当前可自动化施工文档-真实可玩最小循环C-min.md
```
