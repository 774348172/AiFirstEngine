# Runtime FrameLoop

## 当前归属说明：Projection Stage

本文档中 `RenderExtract / Hydration / Physics2D sync / AUI render extract` 等内部跨域同步阶段，统一按 `110-World-Projection-Adapter统一跨域同步规则.md` 理解：

```text
Hydration = HydrationProjection
RenderExtract = RenderProjection
Physics2D sync = Physics2DProjection
AUI render extract = UiProjection
```

FrameLoop 只负责安排这些 Projection Stage 的顺序和同步点，不为具体类型新增独立 Bridge。具体类型扩展必须进入对应 `ProjectionAdapter`。

本文档定义运行时一帧的正式结构。

项目逻辑进入 FrameLoop 的正式入口是 ProjectLogicRunner，详见：

```text
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
```

目标：

```text
用户和 AI 看到的阶段足够少。
复杂项目有清晰扩展点。
逻辑和渲染强隔离。
多线程和调度细节尽量隐藏在引擎内部。
```

## Runtime Asset Loading Apply Point

资源加载和帧循环的正式关系：

```text
Asset IO / decode / GPU upload 可以异步执行。
异步加载完成后，加载结果必须进入 Runtime Asset Apply Point。
Runtime Asset Apply Point 由引擎管理，保证资源缓存、引用计数、RuntimeAssetHandle 和诊断状态一致。
项目侧可以决定何时发起 load_async、何时等待、何时 instantiate、何时 activate、何时 release。
项目侧不能绕过 Runtime Asset Apply Point 直接修改底层资源缓存。
```

同步加载规则：

```text
load_sync 会阻塞当前调用上下文。
load_sync 允许用于启动期、编辑器、加载界面、小型必需资源和测试。
load_sync 如果发生在运行时热路径，必须进入 RuntimeTrace / Console / Build Report warning。
```

异步加载规则：

```text
load_async 返回 AssetLoadRequest。
AssetLoadRequest 可以被 poll / await / cancel。
FrameLoop 不强制项目必须采用固定分阶段加载。
分阶段加载属于 Project Loading Rule / Scene Lifecycle。
```

示例：

```text
Update:
  Project Rule 请求 load_async(BattleSceneAssetSet)

Asset Runtime:
  异步执行 IO / decode / GPU upload
  在 Runtime Asset Apply Point 提交 ready / failed / cancelled 状态

Project Loading Rule:
  观察 AssetLoadRequest.status
  Ready 后执行 instantiate / activate / release old scene
```

## 核心结论

用户 / AI 可见层：

```text
FixedUpdate
Update
LateUpdate
Render
```

引擎内部层：

```text
FrameBegin
HotUpdateApply
DeferredCommandApply
RenderExtract
RendererFeatureBuilder
RDG
RHI
FrameEnd
```

正式主流程：

```text
FrameBegin
  -> HotUpdateApply
  -> DeferredCommandApply

FixedUpdate
  -> ProjectLogicRunner.fixed_update
  -> physics
  -> deterministic systems, optional

Update
  -> ProjectLogicRunner.frame_update
  -> component writes
  -> spawn / destroy requests

LateUpdate
  -> camera follow
  -> animation finalization
  -> UI data sync
  -> gameplay post-processing

RenderExtract
  -> ECS dirty visible state -> RenderCommand / RenderFrameReport

Render
  -> Renderer Feature Builder
  -> RenderSceneState
  -> RDG compile
  -> RHI execute

FrameEnd
  -> present
  -> cleanup
  -> profiler trace
```

## Bevy Frame / Schedule 参考边界

Bevy 的 Main Schedule、SubApp、RenderApp、Extract 和并行 executor 证明了 Rust ECS 可以用清晰的调度边界支撑复杂项目。它对本项目的参考点是：

```text
Main World 和 Render World 分离。
Extract 阶段把 gameplay world 的可见状态提取到 render side。
System 读写声明可以驱动并行调度。
Deferred Commands 用于结构变化安全点。
```

本项目不照搬 Bevy 的用户可见调度 API：

```text
不把 SystemSet / before / after / ambiguous_with 暴露给普通用户和 AI。
用户 / AI 仍只看到 FixedUpdate / Update / LateUpdate / Render 这几个少量阶段。
业务顺序属于项目规则；引擎调度只保证内存安全、结构变化安全和可确定执行。
```

编辑器 Scene Viewport 使用同一条 Runtime Render 主链路，但输出目标不同：

```text
Game View
  -> Runtime Renderer 使用 GameCamera
  -> 输出到 swapchain / game surface

Scene View
  -> Runtime Renderer 使用 EditorCamera
  -> 启用 EditorViewport Render Mode
  -> 可追加 grid / gizmo / selection outline / debug draw 等 editor-only pass
  -> 输出到 viewport render target / texture
  -> Editor UI Renderer 将该 texture 合成到 Scene 面板中
```

边界：

```text
Runtime FrameLoop / Render 阶段负责生成世界内容的渲染结果。
Editor UI Frame 负责面板、按钮、布局、overlay UI 和输入命中。
Editor UI Renderer 不直接读取 Gameplay ECS 来画世界。
Runtime Renderer 不直接绘制 Hierarchy / Inspector / Console / Toolbar。
```

## 设计来源

本方案的用户心智接近 Unity：

```text
FixedUpdate
Update
LateUpdate
Rendering
```

本方案的底层隔离参考 Unreal：

```text
Game Thread / Render Thread separation
Render side reads copied or extracted render data
Game state and render state do not freely mutate each other
```

本项目不直接暴露完整 PlayerLoop、Tick Group 或 Task Graph 给普通用户和 AI。

## EngineHostLoop 最高层心跳源

正式规则：

```text
EngineHostLoop 是最高层心跳源。
EditorFrame / RuntimeFrame / RenderFrame 都由 EngineHostLoop 调度。
OS Window Event 只提供事件输入，不拥有引擎主循环。
Runtime FrameLoop 不直接拥有编辑器 UI。
Editor UI 不直接驱动 Runtime，只发 Play / Pause / Step / Command。
```

EngineHostLoop 的标准结构：

```text
EngineHostLoop
  -> PumpPlatformEvents
  -> DispatchInputEvents
  -> EditorFrame，if editor mode
  -> RuntimeFrame，if playing / simulating / stepping / exported runtime / headless server
  -> RenderExtract，if runtime advanced and render surface exists
  -> RenderFrame，if any surface needs redraw
  -> Present，if render surface exists
  -> Sleep / FramePacing
```

不同运行模式：

```text
Editor Idle:
  PumpPlatformEvents
  DispatchInputEvents
  EditorFrame
  RenderFrame(editor UI only)
  Present
  FramePacing

Editor Play:
  PumpPlatformEvents
  DispatchInputEvents
  EditorFrame
  RuntimeFrame
  RenderExtract
  RenderFrame(runtime viewport + editor UI)
  Present
  FramePacing

Editor Pause:
  PumpPlatformEvents
  DispatchInputEvents
  EditorFrame
  RenderFrame(editor UI only or last runtime snapshot)
  Present
  FramePacing

Editor Step:
  PumpPlatformEvents
  DispatchInputEvents
  EditorFrame
  RuntimeFrame once
  RenderExtract
  RenderFrame
  Present
  FramePacing

Exported Game:
  PumpPlatformEvents
  DispatchInputEvents
  RuntimeFrame
  RenderExtract
  RenderFrame
  Present
  FramePacing

Headless Server:
  RuntimeFrame
  FramePacing / fixed tick pacing
```

对比 Unreal / Unity：

```text
Unreal:
  最高层是 FEngineLoop::Tick。
  平台事件、Slate、Game、Render 都在 EngineLoop 框架下调度。
  优点是引擎拥有主循环，缺点是历史复杂度高。

Unity:
  用户看到 PlayerLoop / Editor Loop / Play Mode。
  用户心智简单，但内部同步和渲染线程细节较黑箱。

本项目:
  采用 EngineHostLoop 作为最高层心跳源。
  保留 EditorFrame / RuntimeFrame / RenderFrame 的显式分离。
  比 Unreal 更轻，比 Unity 更可追踪，更适合 AI 定位问题。
```

已建立最小调度验证：

```powershell
npm.cmd run test:enginehostloop
```

该测试覆盖：

```text
Editor Idle
Editor Play
Editor Pause
Editor Step
Exported Game
Headless Server
```

## 内部线程域与长期路线

FrameLoop 的正式架构从第一版开始就采用多线程线程域设计。

长期正式线程域：

```text
Main / Editor Thread
Runtime / Game Thread
Render Thread
Worker Pool
IO Thread
```

第一版实现规则：

```text
Main / Editor Thread、Runtime / Game Thread、Render Thread、Worker Pool、IO Thread 是第一版正式架构边界。
Runtime / Game / Render / IO 不允许通过单线程共享状态互相穿透。
Render Thread 第一版就拥有 RenderSceneState。
Worker Pool 第一版就作为调度器存在。
IO Thread 第一版就通过消息 / TaskResult 把结果交回 owner 线程域。
```

允许的测试配置：

```text
Headless / Golden Test / CI 可以设置 worker_count=1。
worker_count=1 只是确定性测试配置，不是单线程架构路线。
worker_count=1 必须走同一套调度器、命令队列、Trace 和同步点。
禁止新增单线程专用 API、单线程专用状态共享、Runtime 直接读写 RenderSceneState。
```

### Main / Editor Thread

职责：

```text
OS Window Event
Editor UI Frame
Input Routing
Panel / Toolbar / Inspector / Console / RuntimeTrace ViewModel
UiCommand 生成
```

边界：

```text
拥有窗口和编辑器 UI 状态。
不直接修改 Runtime ECS。
不直接调用 Render Backend 绘制世界。
只通过分领域 Request / Command 与其它域通信，例如 SceneLifecyclePlan、RuntimeSpawnRequest、AssetLoadRequest、EditorCommand、UiCommand。
不建立统一万能 RuntimeCommand。
```

### Runtime / Game Thread

职责：

```text
ECS World ownership
FixedUpdate / Update / LateUpdate
Project Rule System
Deferred Command Apply
HotUpdate Apply Point
RuntimeTrace / FrameHash
RenderExtract
```

边界：

```text
Runtime / Game Thread 是 ECS World 的唯一写入 owner。
项目规则可以在允许阶段读写 ECS Component。
结构变化通过 Deferred Command Apply 在安全点提交。
只在 RenderExtract 产出渲染增量命令、渲染侧输入和调试报告。
```

### Render Thread

职责：

```text
消费 RenderCommand / RenderSceneState / RenderFrameReport
Renderer Feature Builder
RDG / RenderGraph build
RHI Command Plan
RenderReport
GPU submit / present，后续版本
```

边界：

```text
不直接读写 Gameplay ECS。
不执行项目玩法规则。
不修改 Runtime World。
只消费渲染命令、渲染侧状态和渲染资源句柄。
```

### Worker Pool

职责：

```text
asset import preparation
async load preparation
build tasks
AI non-runtime task
后续 animation / physics / visibility / pathfinding job
```

边界：

```text
Worker Pool 不拥有核心状态。
Worker Task 只能处理输入副本或明确授权的只读数据。
任务完成后通过结果消息提交给 owner 线程域。
```

### IO Thread

职责：

```text
file read
package read
download
decompress
hash / signature verify
mount preparation
```

边界：

```text
IO Thread 不让资源在当前帧任意中途生效。
资源加载结果必须进入 Asset Runtime 的 Apply Point。
热更包 mount 必须走 FrameBegin / SceneLoad / SafePause 等安全点。
```

### 线程域设计原则

```text
谁拥有状态，谁负责写入。
跨线程域只能传 Command / Snapshot / Report / TaskResult。
Render 只能读 RenderCommand / RenderSceneState / Report，不读 ECS。
Editor 只能发命令，不直接改 Runtime World。
IO 只准备数据，不直接改变当前运行状态。
Worker 只计算，不拥有长期状态。
```

这条规则的目的，是避免临时单线程实现反过来塑造接口。后续可以扩展 Worker Pool 能力和任务数量，但不能从单线程状态共享迁移到多线程隔离。

## 用户 / AI 可见阶段

### FixedUpdate

用于固定步长逻辑：

```text
物理
帧同步核心逻辑
需要固定 delta 的移动 / 碰撞
确定性模拟，可选
```

AI 选择 FixedUpdate 的典型条件：

```text
用户明确要求固定帧率模拟
逻辑依赖物理步长
逻辑需要确定性或回放
```

### Update

用于普通玩法逻辑：

```text
玩家输入响应
技能释放
AI 行为
子弹移动
血量变化
道具拾取
普通 ECS Component 写入
```

默认项目规则优先放在 Update。

### LateUpdate

用于逻辑之后、渲染抽取之前的收尾：

```text
相机跟随
动画最终修正
UI 数据同步
表现层状态整理
依赖 Update 结果的后处理逻辑
```

LateUpdate 可以读写 ECS，但不应再启动大型玩法结算。

### Render

用于渲染表现：

```text
Renderer Feature Builder
RDG
RHI
Backend
```

Render 阶段不允许修改 Gameplay ECS 状态。

## 逻辑与渲染边界

正式规则：

```text
逻辑层可以读写 ECS。
渲染层不能直接读写 ECS。
渲染层只能读取 RenderCommand / RenderSceneState / 渲染资源句柄。
RenderCommand 由 RenderExtract 从 ECS dirty state 抽取。
RenderSceneState 由 Render Thread 长期维护。
```

原因：

```text
避免渲染流程隐藏修改 gameplay 状态
降低 AI 查 Bug 难度
支持 Game / Render 多线程隔离
支持 RDG / RHI 独立执行
```

错误示例：

```text
Render pass 中修改 enemy.Health
Material callback 中触发 DestroyEntity
Visibility culling 中改变 gameplay state
```

正确示例：

```text
Update 中修改 enemy.Health
LateUpdate 中同步 UI 可见数据
RenderExtract 中复制 Transform / Renderer / Material / Light 数据
Render 中只消费 RenderCommand / RenderSceneState
```

## UE-like Game 到 Render 同步路线

正式长期路线参考 Unreal 的 Game Thread -> Render Thread 隔离方式，但增加 AI 可读的结构化报告。

核心判断：

```text
不采用完整场景 RenderSnapshot 双缓冲作为长期底层战略。
采用增量 RenderCommand + Render-side SceneState。
RenderSnapshot 在架构上标记为 Deprecated / Transition Only。
RenderSnapshot 不能成为大型场景的底层同步模型。
新渲染能力禁止继续依赖 RenderSnapshot。
```

长期正式链路：

```text
Game / ECS / Scene
  -> RenderExtract
  -> RenderCommand Queue
  -> Render Thread
  -> RenderSceneState / RenderProxy
  -> Renderer Feature Builder
  -> RDG
  -> RHI
  -> Backend / GPU
```

与 Unreal 的对应关系：

```text
Unreal:
  UPrimitiveComponent
    -> FPrimitiveSceneProxy / FPrimitiveSceneInfo
    -> ENQUEUE_RENDER_COMMAND
    -> FScene
    -> FSceneRenderer
    -> RDG
    -> RHI

本项目:
  Entity + Render Component
    -> RenderProxy / RenderSceneItem
    -> RenderCommand
    -> RenderSceneState
    -> Renderer Feature Builder
    -> RDG
    -> RHI
```

RenderCommand 是变化命令，不是完整场景副本：

```text
CreateRenderProxy
DestroyRenderProxy
UpdateTransform
UpdateMesh
UpdateMaterial
UpdateLight
UpdateCamera
UpdateVisibility
UpdateSkinningData
UpdateInstanceData
```

性能规则：

```text
如果一帧只有 300 个对象变化，只提交 300 个对象的渲染变化。
不能因为 AI 调试方便，每帧复制完整可渲染世界。
完整场景级双缓冲会按场景总量付费，增量命令按变化量付费。
```

RenderProxy 规则：

```text
每个可渲染 Entity 在渲染侧拥有稳定 RenderProxyId。
RenderProxy 是 Render Thread / RenderSceneState 的长期对象。
项目逻辑不能直接操作 RenderProxy。
项目逻辑只能修改 ECS Component，由 RenderExtract 生成 RenderCommand。
```

AI 友好规则：

```text
AI 不直接生成底层 RenderCommand。
AI 生成 RenderIntent / Visual Patch / Preset / Material Graph / Quality Policy。
引擎验证后由 Renderer Feature Builder / RenderExtract 生成底层命令。
每条 RenderCommand 必须可追溯 source_entity / source_component / source_system / source_ai_patch / reason / frame_index。
```

RenderFrameReport 规则：

```text
RenderFrameReport 是 AI 和调试读取的摘要报告。
它记录本帧渲染变化、降级、警告、成本和 trace。
它不是完整渲染世界的数据副本。
它不能替代 RenderSceneState。
```

允许局部双缓冲的数据：

```text
ViewUniform
CameraData
FrameConstants
LightList
VisibleList
SkinningPalette
ParticleBuffer
UI DrawList
```

禁止作为长期底层战略双缓冲的数据：

```text
完整 Scene
完整 ECS World
完整 RenderProxy 世界
完整 Asset 状态
```

## Deferred Command Apply

ECS 默认允许项目规则直接写 Component。详见：

```text
16-ECS写入与项目规则边界.md
```

Deferred Command 主要用于结构变化：

```text
SpawnEntity
DestroyEntity
AddComponent
RemoveComponent
InstantiatePrefab
跨线程延迟提交
```

Apply Point 默认位于：

```text
FrameBegin
Update 结束后的安全点，可选
LateUpdate 结束后的安全点，可选
```

第一版 Runtime MVP 可以只实现：

```text
FrameBegin Apply
UpdateEnd Apply
```

## HotUpdateApply

热更不能在任意指令中间生效。

热更默认 Apply Point：

```text
FrameBegin
SceneLoad
SafePause
```

正在执行中的 Rule / State 不被中途替换。

## RenderExtract

RenderExtract 是逻辑和渲染之间的唯一桥：

```text
ECS World
  -> dirty visible state
  -> RenderCommand / RenderFrameReport
  -> RenderSceneState
  -> RDG
  -> RHI
```

RenderExtract 负责：

```text
扫描 dirty visible state
生成增量 RenderCommand
更新或提交给 RenderSceneState
生成 AI / Debug 可读的 RenderFrameReport
```

RenderCommand / RenderFrameReport 可以包含：

```text
Transform 变化
MeshRenderer / SpriteRenderer 变化
MaterialRef 变化
Light 变化
Camera 变化
Animation final pose 变化
Visibility data 变化
UI render data 变化
source_entity / source_component / source_system / source_ai_patch / reason / frame_index
```

RenderExtract 不包含：

```text
Health 业务规则
Inventory 业务规则
Skill cooldown 业务规则
AI decision state，除非渲染表现需要只读副本
完整 ECS World
完整 RenderProxy 世界
```

## Snapshot 规则

Snapshot 是 Runtime 对外导出的纯数据只读视图，不等于默认持久化存档。

正式规则：

```text
Runtime 每帧必须能够生成轻量 Snapshot / Hash。
Runtime 默认不保存完整 Snapshot 历史。
RenderSnapshot 已标记为 Deprecated / Transition Only。
后续正式渲染闭环不再要求每帧生成 RenderSnapshot。
当前已经存在的代码如果仍输出 RenderSnapshot，只能视为旧 MVP 兼容输出，不能作为新功能依赖。
FrameHash 可以每帧保存，用于 Replay / Golden Scenario / AI Patch 验证。
DebugSnapshot 只在测试失败、Runtime 报错、AI Patch 行为变化、用户录制时生成或保存。
Full State Snapshot 只在 checkpoint、回滚、深度调试、Golden Scenario 小场景验证时显式触发。
```

Snapshot 分级：

```text
RenderSnapshot
  本帧渲染和编辑器显示需要的只读数据。
  默认每帧生成，临时消费，不保存历史。

FrameHash
  本帧关键状态的稳定 hash。
  默认可以保存，体积极小，用于判断第几帧开始变化。

DebugSnapshot
  面向 AI 查 Bug / 用户审查的局部状态证据。
  只在验证、报错、Replay Debug、用户录制时保存。

Full State Snapshot
  完整或接近完整的 ECS 状态检查点。
  成本高，禁止默认逐帧保存。
```

用途：

```text
编辑器显示当前运行状态
渲染系统消费 RenderSnapshot
Golden Scenario 对比 expected
Replay / 回滚 / 热更 checkpoint
AI 根据 Snapshot + Trace 解释运行结果和定位 Bug
```

禁止：

```text
默认每帧保存完整 ECS World
默认每帧保存所有 Entity / Component
把完整 Snapshot 历史塞进 Project Data
把 Replay Debug Package 当成项目数据长期膨胀
```

## 多线程定位

多线程不是用户可见阶段。

正式定位：

```text
FrameLoop 是用户 / AI 心智。
Job System 是引擎内部执行器。
```

引擎可以在阶段内部并行：

```text
FixedUpdate 内部并行 physics jobs
Update 内部并行无冲突 ECS systems
LateUpdate 内部并行动画和 UI 数据准备
RenderExtract 并行抽取可见对象
Render 内部 RDG / RHI 按平台能力执行
```

用户和 AI 默认不直接选择线程。

## AI 生成规则

AI 只需要选择少量阶段：

```text
FixedUpdate
Update
LateUpdate
Render
```

默认判断：

```text
玩法逻辑 -> Update
固定步长 / 物理 / 帧同步核心 -> FixedUpdate
相机 / UI 同步 / 表现收尾 -> LateUpdate
渲染表现 -> Render
```

AI 不直接维护：

```text
完整 Task Graph
Worker Thread 分配
底层 sync point
RDG pass scheduling
RHI command ordering
```

## 最小 Job System 与 UE TaskGraph 边界

本项目学习 Unreal 的 Game Thread / Render Thread / Worker 并行思想，但第一版不实现完整 UE TaskGraph。

原因：

```text
UE TaskGraph 是全引擎通用任务调度底座。
它服务 UObject、ActorComponent、Animation、Physics、Renderer、Slate、Async Loading 等大量系统。
它包含 NamedThread、任务依赖、任务事件、不同线程队列、已有上下文复用等复杂能力。
直接照搬会让第一版 Runtime 过早面对调度器复杂度，而不是先验证 ECS / RenderExtract / RenderCommand 边界。
AI 查 bug 时也会先陷入任务竞争、调度依赖和线程时序，而不是业务规则本身。
```

UE 源码对应关系：

```text
UE FTaskGraphInterface / TGraphTask
  -> 本项目 Minimal Job System

UE ParallelFor / ParallelForWithExistingTaskContext
  -> 本项目 WorkerPool + parallel_for

UE UWorld::ComponentsThatNeedEndOfFrameUpdate
  -> 本项目 RenderDirtyTracker lists

UE UActorComponent::DoDeferredRenderUpdates_Concurrent
  -> 本项目 RenderExtract job

UE ENQUEUE_RENDER_COMMAND
  -> 本项目 RenderCommandQueue
```

本项目第一版只实现 Minimal Job System：

```text
ThreadDomain
Job
JobHandle
WorkerPool
PhaseScheduler
RenderExtractScheduler
```

### ThreadDomain

表示任务归属线程域：

```text
MainEditor
RuntimeGame
Render
Worker
IO
```

规则：

```text
状态只能由 owner ThreadDomain 写入。
跨 ThreadDomain 只能传 Command / TaskResult / Report。
Worker 不拥有长期状态。
Render 不读 ECS。
Runtime 不直接写 RenderSceneState。
```

### Job / JobHandle

Job 是内部执行单元，JobHandle 是等待或依赖句柄。

第一版只需要：

```text
job_id
name
domain
phase
input range
trace span
dependency handles
```

禁止：

```text
把 Job 暴露给普通项目逻辑。
让 AI 直接生成底层 Job Graph。
让业务顺序依赖隐式线程调度。
```

### WorkerPool

WorkerPool 负责并行计算，不负责业务规则解释。

第一版用途：

```text
ECS 无冲突 system 并行执行。
RenderExtract 按 dirty 分块并行提取。
Asset / IO 后处理任务。
Golden Test 中使用 worker_count=1 走同一执行路径。
```

### PhaseScheduler

PhaseScheduler 负责执行 FrameLoop 中的阶段：

```text
FixedUpdate
Update
LateUpdate
RenderExtract
Render
```

规则：

```text
用户和 AI 只看到阶段。
PhaseScheduler 内部根据 reads / writes / dirty ranges 决定是否并行。
项目业务顺序由项目规则显式表达，不由线程调度隐式决定。
```

### RenderExtractScheduler

RenderExtractScheduler 是第一版最重要的并行落地点。

职责：

```text
读取 RenderDirtyTracker。
按 dirty type / component type / entity range 分块。
并行生成 RenderCommand。
稳定排序后提交 RenderCommandQueue。
生成 RenderFrameReport。
```

规则：

```text
RenderExtract 可以并行。
RenderCommand 合并和排序必须确定性。
worker_count=1 和 worker_count>1 的输出语义必须一致。
```

### 不实现完整 UE TaskGraph 的正式规则

```text
第一版不实现完整 UE TaskGraph。
第一版不暴露通用任务图给项目层。
第一版不支持用户手写任意跨阶段 job dependency。
第一版只支持引擎内部固定阶段内的可控并行。
后续如果扩展通用 Job Graph，也必须保持 FrameLoop / ECS / RenderExtract 的用户心智不变。
```

## MVP 版本

最小 Runtime MVP 只需要实现：

```text
FrameBegin
FixedUpdate
Update
LateUpdate
RenderExtract
Render
FrameEnd
```

第一版必须走多线程架构执行路径：

```text
System 读写检测
Deferred Command
RenderExtract
RenderCommandQueue
Profiler Timeline
```

允许测试环境把 worker_count 设为 1，但仍然必须通过同一套调度器和命令队列执行。

## 正式边界

```text
FixedUpdate / Update / LateUpdate / Render = 用户和 AI 可见主阶段
FrameBegin / FrameEnd = 引擎生命周期阶段
HotUpdateApply = 引擎安全切换点
DeferredCommandApply = ECS 结构变化安全点
RenderExtract = 逻辑到渲染的唯一桥
Render = 渲染执行，不修改 Gameplay ECS
Job System = 内部执行器，不作为用户主心智
```
