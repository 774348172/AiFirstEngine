# 300 ProjectUiStateSnapshot Session-Bound Conditional Resolve v1 方案

## 1. 文档状态

```text
系统编号：300
方案版本：v1
建立日期：2026-08-18
问题来源：299 已消除 Windows Player clean-frame present 全量重建，但真实 Tower 仍在 cache 判断前每帧跨 ABI 生产 768 个 binding values，约 50-59ms/frame
讨论方案：A 独立 peek revision；B-min producer 前置 revision；完整 B session-bound 单次 conditional resolve；C mutation/world 全局 dirty push
用户选择：完整 B
正式名称：ProjectUiStateSnapshot Session-Bound Conditional Resolve v1
当前状态：正式方案已生成并自审；尚未生成施工文档，未授权施工
```

本文档只固化用户确认的完整 B 方案，不构成源码修改、测试、构建、Tower RuntimeModule 重建、
production Editor/Player 替换、Tower Preview cache 重建、真实配置修改或 Local CI 授权。

## 2. 一句话目的

让项目 UI snapshot producer 与真实 Play Session 共享同一状态和 native handle，并通过一次原子
`resolve(previousIdentity)` 在昂贵 World read、active binding 遍历和值序列化之前返回 `Reuse`；只有
项目可见 UI 状态真正变化时才生成新 snapshot。

## 3. 已确认问题与性能证据

299 完成记录：

```text
阶段完成记录/2026-08-18-Windows-Player-AUI-Clean-Frame-Cache-Minimal-Repair-v1/00-总览.md
```

真实 Tower 证据：

```text
旧基线 update.meanMs：59.13ms
299 后 update.meanMs：约 59.18-61.24ms
无 AUI manifest 同包对照：1.03ms

299 present cache 30 帧：
  rebuild = 2
  hit = 28
  presentation revision = 2

clean frame cache 判断前：
  768-path ABI snapshot production = 50-59ms/frame
  AUI input = 约 2ms
  host tick = 约 0.7-1.8ms
```

因此 299 已证明：

```text
present cache 本身可以命中；
性能残留不在 layout/draw/glyph present；
残留发生在 host 取得可比较 values 之前；
必须把 clean 判断移动到 ProjectUiStateSnapshot production 之前。
```

## 4. 当前所有权缺口

### 4.1 Core Interface 只能无条件生产

当前 `engine_runtime::aui::ProjectUiStateSnapshotProducer` 只有：

```rust
fn producer_id(&self) -> &str;
fn produce(&mut self, context: ProjectUiStateProducerContext<'_>)
    -> ProjectUiStateSnapshotOutput;
```

调用方无法在 `produce()` 前表达自己已经持有哪一版 snapshot，也无法得到 `Reuse` 结果。

### 4.2 Native producer 没有绑定真实 session

当前 `LoadedProjectRuntimeSession` 与 `LoadedProjectUiStateProducer` 由两个彼此独立的 factory 创建。
Native producer 调 `produce_ui_state` 时传入 `ProjectRuntimeOpaqueHandle::NULL`，而 Tower 的：

```text
match/runtime state
selection / project feedback
projection generation
FPS sampler
session generation
```

都属于 `TowerDefenseRuntimeSession`。因此单独给无 session producer 增加 revision，无法可靠覆盖真实
session-local UI 状态，也无法解决 FPS/选择态等后续缺口。

### 4.3 Editor 与 Player 都在错误位置比较

当前普通 Editor GameView 和 Windows Player 都执行：

```text
producer.produce(all active paths)
-> 收到全部 values
-> 与上一帧 values 比较
-> 决定 present hit/rebuild
```

这只能避免第二段 present 重建，不能避免第一段 snapshot production。只修 Player 还会让 Editor 保留
同一缺陷，因此完整 B 必须同时修正共享 Interface 与两个真实 consumer。

## 5. 既有方案继承

300 不推翻既有系统：

```text
199：ProjectUiStateSnapshotProducer 是项目 ReadModel producer seam，AUI Binding 只读 snapshot。
230：producer 必须 active-binding-driven、dirty/cached，并由 Play Session 生命周期持有。
260：项目 action、fixed update 与 ProjectRuntimeSession 的 exactly-once 生命周期。
265/286-R1：last-valid presentation 与失败时保留上一张正常帧。
284：AUI 输入、反馈与业务 action 在普通帧即时处理，不等待 fixed tick。
292：稳定 Editor 通过 ProjectRuntimeAbi 加载项目 native module，合同变化触发项目模块重建。
294：AUI/presentation identity 使用 revision，fixed simulation 与 presentation 分离。
299：Windows Player 已有 session-local present cache 与 take/store 所有权转移。
```

300 只补齐 199/230/294/299 没有真正实现的 producer 前置 clean 合同，并修正 292 native Adapter
中 gameplay session 与 UI producer 的实例所有权。

## 6. 成熟引擎对标

### 6.1 Unreal Slate / UMG

Epic 官方 Invalidation 文档说明：Invalidation Box 缓存子树 geometry，只在 widget 被标记 invalidated
后重绘；Global Invalidation 把同一策略提升到窗口级。对应 Slate 源码责任集中在：

```text
SWidget::Invalidate(EInvalidateWidgetReason)
FSlateInvalidationRoot
SInvalidationPanel / UInvalidationBox
```

可学习点：昂贵 paint/geometry 之前由 owner 判断失效，clean 时复用缓存。

不可照搬点：300 不建立 per-node invalidation tree，不把 ProjectUiStateSnapshot 变成 Slate widget tree。

参考：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/invalidation-in-slate-and-umg-for-unreal-engine
```

### 6.2 Godot CanvasItem

Godot `CanvasItem::queue_redraw()` 只登记一次 idle redraw，之后通过 `NOTIFICATION_DRAW` 请求绘制；
CanvasItem 不需要每帧重画。源码 owner 位于：

```text
scene/main/canvas_item.cpp
CanvasItem::queue_redraw()
CanvasItem::_notification(NOTIFICATION_DRAW)
```

可学习点：状态 owner 先记录 redraw demand，绘制路径只消费 demand。

不可照搬点：AUI snapshot 是项目 ReadModel，不是 CanvasItem；300 不把项目数据 mutation 改成节点回调。

参考：

```text
https://docs.godotengine.org/en/stable/classes/class_canvasitem.html
```

### 6.3 Unity UI Toolkit

Unity UI Toolkit 在 `VisualElement.IncrementVersion(VersionChangeType)` 把变化上报 Panel，再由
`VisualTreeUpdater` 针对 version/change type 更新受影响阶段。相关公开源码位置：

```text
Modules/UIElements/Core/VisualElement.cs
Modules/UIElements/Core/Panel.cs
Modules/UIElements/Core/VisualTreeUpdater.cs
```

可学习点：revision 由真正拥有变化的对象推进，而不是 renderer 每帧重算内容 hash。

不可照搬点：300 不建立完整 VisualElement change graph，不把 AUI node 变成 runtime ECS entity。

## 7. 方案比较与正式选择

### 7.1 方案 A：`peek_revision()` + `produce()`

```text
优点：表面改动小。
缺点：dirty frame 两次 ABI call；probe/produce 之间有状态漂移窗口；caller 必须学习两阶段协议；
      仍未解决 producer 与 session 分裂。
结论：不采用。
```

### 7.2 方案 B-min：在现有 producer 上增加 visible revision

```text
优点：可以把比较移动到 values production 之前。
缺点：当前 native producer 使用 NULL session；revision 无法可靠覆盖 session-local FPS、selection、
      feedback 与 lifecycle epoch；Editor/Player 仍可能各自实现不同判断。
结论：单独采用不足；其小 Interface 思路并入完整 B。
```

### 7.3 完整 B：Session-bound 单次 Conditional Resolve

```text
session + producer 原子创建并共享 session lease；
一次 resolve 内完成 identity 判断和必要 snapshot production；
active binding set 只在首次/变化时跨 ABI；
Editor/Player 共用 conditional snapshot/present consumer；
Tower 在项目 session 内维护 ui_visible_revision。
```

优点：修正真实 owner，消除 clean-frame 50-59ms 路径，并同时覆盖 Editor、Player、FPS 与项目反馈。

代价：需要同步修改 Core Interface、ABI/SDK wire contract、Native Adapter、两个 consumer 与 Tower
RuntimeModule，并按 292 规则重建受影响项目 native module。

结论：正式采用。

### 7.4 方案 C：World/mutation 全局 dirty push 或节点级依赖图

```text
优点：理论上可做更细粒度更新。
缺点：World 任意变化会被怪物 Transform 高频 mutation 污染；需要新 signal/dependency graph；
      扩大到 ECS/AUI 全局架构，明显超过已确认首因。
结论：不采用。
```

## 8. 正式深 Module 与 seam

300 深化现有 `ProjectUiStateSnapshotProducer` Module，不新增第二套 UI runtime：

```text
小 Interface：producer_id + resolve

Implementation 隐藏：
  session lease / native handle
  session-local visible revision
  active binding set registry
  ABI request/output 编码
  World read 与 binding value production
  report mode 与 compact diagnostics
  snapshot/present cache coordination
```

Interface 的 leverage：一次实现供普通 Editor、Windows Player、headless gate 与未来 consumer 共同使用。

Interface 的 locality：项目可见 revision 只由项目 session 推进；ABI 编码只在 Native Adapter；
present composite identity 只在 AUI present owner。

依赖分类：

```text
Core producer / session / cache：in-process。
ProjectRuntimeAbi：remote-but-owned 式进程内动态库 seam，由 Native Adapter 适配。
Tower RuntimeModule：外部用户项目 consumer，不反向依赖 Editor/Player。
```

## 9. Core Interface

建议正式类型：

```rust
pub struct ProjectUiStateIdentity {
    pub producer_epoch: u64,
    pub visible_revision: u64,
    pub binding_set_identity: ProjectUiBindingSetIdentity,
}

pub enum ProjectUiBindingSetRef<'a> {
    Known(ProjectUiBindingSetIdentity),
    Replace {
        identity: ProjectUiBindingSetIdentity,
        sorted_deduplicated_paths: &'a [String],
    },
}

pub struct ProjectUiStateResolveContext<'a> {
    pub frame_index: u64,
    pub time: TimeContext,
    pub package: &'a RuntimePackage,
    pub world: &'a World,
    pub binding_set: ProjectUiBindingSetRef<'a>,
    pub previous_identity: Option<&'a ProjectUiStateIdentity>,
    pub report_mode: ProjectUiStateReportMode,
}

pub enum ProjectUiStateResolve {
    Reuse {
        identity: ProjectUiStateIdentity,
    },
    Replace {
        identity: ProjectUiStateIdentity,
        output: ProjectUiStateSnapshotOutput,
    },
    Uncacheable {
        output: ProjectUiStateSnapshotOutput,
    },
}

pub trait ProjectUiStateSnapshotProducer {
    fn producer_id(&self) -> &str;

    fn resolve(
        &mut self,
        context: ProjectUiStateResolveContext<'_>,
    ) -> Result<ProjectUiStateResolve, ProjectUiStateResolveError>;
}
```

v1 普通 production producer 必须返回 `Reuse` 或 `Replace`。`Uncacheable` 只允许 built-in empty、
test fixture 或显式 compatibility diagnostic 使用；Tower production normal-loader 不得长期依赖它。

## 10. Identity 合同

`ProjectUiStateIdentity` 是 session-scoped opaque content identity；caller 只比较，不自行推导。

### 10.1 `producer_epoch`

下列事件必须产生新 epoch：

```text
Play Session 创建/重建；
project native module/session handle 重建；
project runtime reset；
producer terminal fault 后重新建立；
```

identity 不得跨 Play Session、Editor/Player process 或不同 native module artifact 复用。

### 10.2 `visible_revision`

只有任意 active binding 可见值可能变化时才单调递增。允许保守 false positive，禁止 false negative。

以下事实不得自动推进项目 visible revision：

```text
frame_index；
普通 fixed tick count；
怪物 Transform / Animator2D 普通移动；
report timestamp / trace；
hover / pressed 等 AUI control feedback；
font/texture/target resource revision。
```

### 10.3 `binding_set_identity`

active binding paths 必须：

```text
trim validation；
stable sort；
deduplicate；
以 canonical bytes 生成确定性 digest；
```

binding set 变化强制 `Replace`。frame 普通 clean path 只跨 ABI 传固定宽度 identity，不再序列化
768 个 path string。

## 11. Resolve 不变量

```text
R1. Reuse 必须在枚举 active paths、读取完整 component、生成 ProjectRuntimeValue、构造 snapshot values
    和序列化 values payload 之前返回。

R2. 相同 ProjectUiStateIdentity 必须对应完全相同的 active binding visible values。

R3. 同一个 resolve call 内完成 revision 判断与必要 production；禁止 caller 编排 probe -> produce。

R4. previous identity 缺失、epoch 不匹配、binding set 不匹配、revision 回退或 producer 不确定时，
    禁止 Reuse；必须 Replace、Uncacheable 或 typed fault。

R5. resolve error 不推进 caller cache；last-valid present 可以继续显示，但不得记录为新正常 hit。

R6. Off/Summary clean output 必须为常量大小；不得先构造 Trace paths/values 再丢弃。

R7. dirty frame 最多生产一次 snapshot；一次 ordinary frame 最多调用一次正常 resolve。
```

## 12. Session Bundle 与 native handle 所有权

当前独立 `runtime_session_factory + ui_state_producer_factory` 改为 bind-local session bundle factory：

```text
ProjectRuntimeSessionBundle
  gameplay_session: ProjectRuntimeSession Adapter
  ui_state_producer: ProjectUiStateSnapshotProducer Adapter
  shared native_session_lease
```

Native Adapter 的两个角色共享同一 `NativeProjectRuntimeSessionLease`：

```text
lease 唯一拥有 ProjectRuntimeOpaqueHandle；
create_session 只执行一次；
handle_aui_actions / fixed_update / observe / resolve_ui_state 使用同一 handle；
最后一个 bundle owner drop 时 destroy_session exactly-once；
缺失、重复或 NULL lease 在 bind 阶段 typed fail；
```

不得使用 process-global/static map 按创建顺序猜测 session/producer 配对，也不得继续把 `NULL session`
作为 production UI producer 的合法状态。

## 13. ProjectRuntimeAbi / SDK wire contract

### 13.1 正式 cutover

现有 `produce_ui_state` 语义正式切换为 `resolve_ui_state`，wire contract 使用：

```text
ProjectRuntimeUiStateResolveRequest
  frame/time
  previousIdentity?
  bindingSet:
    known { digest }
    replace { digest, activeBindingPaths[] }
  reportMode

ProjectRuntimeUiStateResolveOutput
  reuse { identity }
  replace { identity, producerId, values, compactSummary? }
  uncacheable { producerId, values, compactDiagnostic? }
```

clean `reuse` response 不含 values map、active path list、source path list或 snapshot payload。

### 13.2 版本与迁移

```text
更新 ProjectRuntime ABI/SDK contract schema 与 digest；
按兼容规则更新 ABI minor 或等价 contract version；
native module API、loader validation 与 module build/cache identity 必须消费新 contract digest；
RuntimeProjectModuleRef 继续只保存 module/interface/AOT identity，不为 300 增加重复 contract 字段；
旧项目 native module 不得被新 host 静默解释为 conditional producer；
292 module builder/cache 只重建 contract identity 真正失配的项目 module；
repo 内正常项目模块直接迁移新 schema，不长期保留双 production protocol。
```

可以保留一个短期 source-level compatibility Adapter，让旧 in-process fixture 每次返回 `Uncacheable`；
它不得进入 Tower production normal-loader，也不得成为长期 migration layer。

### 13.3 Active binding set 注册

```text
首次 session/frame：Replace(binding set digest + paths)。
相同 document 后续帧：Known(binding set digest)。
AUI document/visible binding set 变化：Replace(new digest + paths)。
module 报 binding set unknown：caller 最多重发一次 Replace；禁止普通帧循环重试。
```

## 14. Tower project consumer

Tower 是首个真实 consumer，但项目语义不得进入引擎 Interface。

### 14.1 Session-local UI identity

`TowerDefenseRuntimeSession` 新增项目侧：

```text
ui_producer_epoch
ui_visible_revision
last_visible_projection
active_binding_set cache
FPS displayed revision
```

不得直接复用现有 `projection_generation` 作为 UI visible revision，除非测试证明它只在真实可见值变化
时递增。当前 implementation 允许 fixed update 内部 generation 前进而最终 projection mutation 为空，
直接复用会制造每帧 false dirty。

### 14.2 Revision 推进

```text
tower.matchView 实际可见 projection 变化并产生 replacement：revision + 1。
selection / project feedback / phase / resource number 等 active binding 值变化：revision + 1。
FPS sampler 每帧只做 O(1) accumulate；每 250ms 采样后显示文本实际变化：revision + 1。
active binding set 不含 FPS path：不做 FPS UI sampling work。
怪物 Transform、Sprite Animator 普通移动：不推进 UI revision。
无可见变化：revision 不动。
```

### 14.3 Dirty production

dirty resolve 优先从 session-owned `last_visible_projection` 生产 values；不得为了同一份项目 read model
再次经 ABI host callbacks 全量 query/read `tower.matchView`。必须用等价测试证明它与最终提交到 World 的
projection 内容一致。

Tower 文件仍只出现于：

```text
samples/tower_defense_project/RuntimeModule/**
```

引擎 schema、diagnostic code 与 test fixture 禁止出现 `tower.*`、兵种、怪物、军粮等项目专用语义。

## 15. Editor / Player 共用 consumer

现有 Editor 与 Player 的 late compare 逻辑收敛为一个 AUI conditional present owner/helper：

```text
resolve project snapshot once
  Reuse  -> 保留 cached snapshot
  Replace -> replace cached snapshot
  Error   -> 保留 last-valid present + compact diagnostic

compose host present identity
  unchanged -> take/store reuse last present
  changed   -> 用 cached/new snapshot 重建 present 一次
```

真实 consumer 至少包括：

```text
editor_core::editor_gameview_play 普通 Editor GameView；
runtime_player_winit real-window persistent session；
runtime_player_winit headless gate；
runtime_player_winit compatibility window loop。
```

不得分别复制 binding-set registry、identity validation 或 error fallback。consumer 差异只保留在窗口、
输入和 publication Adapter 内。

## 16. Composite present identity

项目 snapshot identity 只拥有项目 binding values。外层 AUI present owner 组合：

```text
AuiCompositePresentIdentity
  document_revision
  project_ui_state_identity
  target_presentation_revision
  font_texture_resource_generation
  aui_control_feedback_revision
```

规则：

```text
snapshot Reuse + host factor 全部相同：直接复用 present。
snapshot Reuse + hover/pressed/target/resource 变化：使用 cached snapshot 重建 present，不重新生产项目 values。
snapshot Replace：替换 cached snapshot，并按 host factor 重建 present 一次。
```

这保证 284 的即时 hover/pressed/click feedback 不等待 fixed tick，也不误推进项目 UI revision。

## 17. 普通帧顺序

```text
1. Pump platform input/events。
2. 使用 last-valid action targets 解析即时 AUI hit/feedback/action。
3. Project Runtime Session 处理 action；必要时推进 project UI revision。
4. 执行 0..N fixed simulation steps；聚合项目 mutation/visible revision。
5. canonicalize/复用 active binding set identity。
6. 调用一次 ProjectUiStateSnapshotProducer::resolve。
7. Replace 时更新 cached snapshot；Reuse 时零 snapshot production。
8. 组合 host present identity；最多重建/提交一次 AUI presentation。
9. Render/present 一次；发布 compact Summary。
```

首帧没有 last-valid action targets 时先执行 initial Replace；不恢复 pre-hydration 多帧重复 present，也不让
业务 action 重放。

## 18. 错误与 fail-closed

最小 typed error/diagnostic：

```text
project_ui_state.session_lease_missing
project_ui_state.binding_set_unknown
project_ui_state.identity_epoch_mismatch
project_ui_state.revision_regressed
project_ui_state.reuse_without_baseline
project_ui_state.resolve_contract_fault
```

行为：

```text
identity 不确定：不得 Reuse。
binding set unknown：最多一次 Replace registration retry。
recoverable resolve failure：保留 last-valid present，下一帧清 baseline 后重试。
terminal session/ABI fault：停止该 project runtime session，输出 compact next action。
diagnostic/report failure：不得改变 authoritative gameplay state。
```

## 19. 性能合同

### 19.1 Clean frame

```text
conditional ABI resolve call：1 次常量大小 request/response。
World query：0。
component read：0。
active path string serialization：0。
ProjectRuntimeValue production：0。
snapshot values payload：0 bytes。
Trace path/detail construction：0。
复杂度：O(1)。
```

建议 owner 门槛：clean resolve p95 小于 0.5ms；最终门槛由施工基线机实测固定，不用 sleep/帧率节流
伪造结果。

### 19.2 Dirty frame

```text
conditional ABI resolve：1 次。
snapshot production：最多 1 次。
复杂度：O(active binding paths)。
present rebuild：一次 ordinary frame 最多 1 次。
```

### 19.3 Binding set 变化

```text
canonicalize/digest：只在 document/visible binding set 变化时 O(N)。
paths 跨 ABI：每个新 binding set 注册一次。
普通 clean frame：只传 digest/identity。
```

## 20. 预期验证合同

本节只定义未来施工必须证明的结果，不授权现在运行测试。

### 20.1 Owner red-capable tests

```text
相同 identity -> Reuse，producer body/value counter 不增加。
visible value change -> revision 前进且 Replace 一次。
binding set change -> Replace；旧 identity 禁止命中。
epoch/session change -> 旧 identity 禁止复用。
revision regression/reuse without baseline -> typed fail closed。
clean Off/Summary -> 0 path/value/Trace production。
```

### 20.2 Native Adapter tests

```text
gameplay session 与 producer 使用同一非 NULL handle。
destroy_session exactly-once。
clean resolve 0 host world query / component read。
active binding set 首次发送 paths，后续只发送 digest。
ABI/SDK contract mismatch 在 load/bind 阶段失败，不误用旧 payload。
```

### 20.3 Consumer tests

```text
Editor 与 Player 通过同一 helper 得到相同 Reuse/Replace 语义。
snapshot clean + hover/pressed dirty 仍即时 present，producer 不重建 values。
snapshot dirty 一帧只 Replace/present 一次。
last-valid present 在 recoverable producer failure 下保留。
```

### 20.4 Tower vertical slice

```text
initial frame Replace；连续 clean frame Reuse。
征兵/部署/出战/选择/反馈改变可见值时 Replace。
怪物连续移动不让 project UI snapshot 每帧 dirty。
FPS 只在 250ms sampled display value 实际变化时 Replace。
30 秒 source-linked Tower smoke 记录：
  cleanResolveCount
  replaceCount
  worldQueryCountOnClean = 0
  componentReadCountOnClean = 0
  producedValueCountOnClean = 0
  update mean/p95
```

施工开发循环只需要 owner test、Native Adapter direct consumer 和一次有界 source-linked Tower smoke。
Local CI、production replacement、真实 cache、完整 E2E/视觉矩阵不自动成为源码修复前置条件。

## 21. 文件与所有权范围

预计 engine-owned：

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/project_runtime_module.rs
rust/crates/engine_runtime/src/project_runtime_native_adapter.rs
rust/crates/project_runtime_abi/src/lib.rs
rust/crates/project_runtime_sdk/src/lib.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/runtime_player_winit/src/lib.rs
```

预计 project-owned：

```text
samples/tower_defense_project/RuntimeModule/src/lib.rs
samples/tower_defense_project/RuntimeModule/src/playable_session.rs
samples/tower_defense_project/RuntimeModule/src/ui_projection.rs
```

按真实引用闭包可能更新 repo 内其它 ProjectRuntimeModule fixture/sample，但只能做新合同所需的机械迁移与
对应 owner tests，不得顺手修改 gameplay、Editor UI、Renderer 或 build/export 功能。

## 22. 明确不做

```text
不做固定帧率节流或降低 AUI 更新频率。
不做 Tower component/path 的引擎特判。
不做 World 全局 revision 作为 UI identity。
不做 per-node reactive dependency graph、signal bus 或完整 retained UI 重构。
不做 AUI node 局部 patch；v1 dirty 时允许完整 snapshot/present rebuild 一次。
不修改战斗、物理、怪物移动 fixed tick 语义。
不修改字体、纹理、GameView target/input presentation。
不生成新的常驻 Trace/report 系统。
不在本方案阶段运行测试、构建 candidate、替换 production 二进制或修改真实配置/cache。
```

## 23. 风险与控制

### 23.1 False negative 导致 UI 过期

```text
控制：same identity => same visible values 不变量；项目 session 在 mutation locality 推进 revision；
      identity 不确定时 fail closed 到 Replace，不允许可疑 Reuse。
```

### 23.2 False positive 仍频繁重建

```text
控制：Tower 不直接复用每 tick 前进的 projection_generation；怪物 Transform/Animator 不进入
      ui_visible_revision；smoke 必须记录 dirty reason 与 clean/replace counts。
```

### 23.3 ABI migration 再触发项目冷编译

```text
控制：合同 digest 只变化一次；292 cache identity 精确失效；受影响项目 module 一次重建后正常 hit；
      不新建第二套 ABI、长期 compatibility layer 或 project-specific Editor。
```

### 23.4 Editor/Player 再次漂移

```text
控制：共享 conditional present owner/helper；两个 consumer 只保留平台 Adapter 差异；同一 owner contract
      测试同时覆盖 Editor 与 Player。
```

### 23.5 FPS sampler 自身要求每帧时间输入

```text
控制：resolve clean path允许一次 O(1) session-local sampler accumulate；只有 displayed FPS 文本实际变化
      才推进 revision；不遍历 bindings、不读 World、不生成 values。
```

## 24. 方案自审

### 24.1 是否一次性解决已确认根因

```text
是。clean 判断移动到 768-path ABI production 之前；session-local revision、FPS、selection、feedback、
Editor 与 Player consumer 均在同一合同内闭环，不留下 NULL session 或 late compare。
```

### 24.2 是否过量施工

```text
否。完整 B 比 B-min 多出的 session bundle/lease 是修正真实 owner 所必需；Editor/Player 共用 helper 是
避免同一缺陷重复实现所必需。方案明确拒绝 World dirty graph、节点级 reactive UI、固定节流、完整 E2E、
Local CI 与 production workflow。
```

### 24.3 Interface 是否足够深

```text
是。caller 只学习 producer_id + resolve 和 Reuse/Replace/Uncacheable；session lease、binding-set registry、
ABI encoding、revision folding 与 diagnostics 全部隐藏在 Implementation/Adapter 内。
```

### 24.4 是否遵守引擎/Tower 所有权

```text
是。引擎只提供项目无关 conditional snapshot/identity/session bundle 能力；Tower 只在项目 RuntimeModule
维护自己的 visible projection/revision。项目专用 binding path 与玩法语义不进入引擎。
```

### 24.5 是否保持输入即时性

```text
是。hover/pressed/click feedback 继续由 284 普通帧即时阶段处理；它只改变外层 AUI feedback revision，
不等待 fixed tick，也不要求重新生产项目 snapshot。
```

### 24.6 是否有最小但能捕获失败的验证

```text
是。owner counter 直接捕获 clean frame 是否仍生产 values；Native Adapter 直接证明同 session handle；
Editor/Player consumer test 防止漂移；一次 30 秒 Tower smoke 证明真实 50-59ms 路径消失。没有机械追加
Local CI、完整视觉矩阵或 production replacement。
```

### 24.7 外部审查状态

```text
本轮没有与 300 对应的新其它 AI 审查文档。既有 199/230 审查结论已由正式 dirty/cached、active binding、
project-owned producer 与 report 分档规则吸收；无需重开历史审查。
```

## 25. 正式结论

正式采用：

```text
ProjectUiStateSnapshot Session-Bound Conditional Resolve v1

= bind-local ProjectRuntimeSessionBundle / shared native session lease
+ single atomic resolve(previous identity)
+ Reuse before World read / value production
+ one-time active binding set registration
+ Tower project-owned ui_visible_revision
+ shared Editor/Player conditional present consumer
+ host-side composite present identity
```

下一步只能在用户单独要求后生成并自审 300 极简施工文档。当前不得修改源码、运行 Gate、重建 Tower
RuntimeModule、更新 production Editor/Player、修改真实 cache/config 或运行 Local CI。
