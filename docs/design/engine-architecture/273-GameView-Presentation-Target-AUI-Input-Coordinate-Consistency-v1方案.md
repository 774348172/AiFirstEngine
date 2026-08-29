# 273-GameView Presentation Target / AUI Input Coordinate Consistency v1 方案

## 1. 状态与结论

```text
讨论结论：用户确认方案 C
方案状态：正式方案已确认；Gate A-F 已完成
施工状态：已完成并归档；273-R1 720 GameView input bridge remediation 也已完成归档
引擎修改授权：已获得“允许修改引擎代码来补充竖屏 GameView target 与 AUI input 坐标一致性能力”
当前施工授权：273 Gate A-F 引擎源码、owner/affected tests 与 run-owned source-built targeted smoke
适用范围：Rust Runtime + Native Rust Editor Host 的 GameView target、AUI projection、input 与 authority
外部项目消费者：Tower Defense P1-0 竖屏 Editor Play；不得把塔防玩法语义写入引擎
```

采用方案 C：新增深 `GameViewPresentationModule`，以 typed `GameViewPresentationSpec` 和
`ResolvedGameViewPresentation` 统一 AUI ReferenceSpace、Runtime TargetSpace、Editor DisplaySpace
与 OS WindowSpace 之间的正反变换。Runtime renderer、GameView texture present、真实输入路由、
AUI hit-test、action-target authority 与诊断只消费这一份解析结果，不再各自维护缩放公式。

本方案同时补齐两个能力：

1. Editor Play session 可以显式选择真实竖屏 GameView render target，例如 `1080x1920`、
   `720x1280`，不再由 `EngineHostLoop` 固定使用 `1280x720`。
2. AUI interaction 按 canvas reference space 命中；gameplay 输入继续保持 runtime target space。
   同一个 OS pointer event 在 UI 与 gameplay 之间通过既有 consumed index 合同 exactly-once 分流。

## 2. 触发证据与问题定义

Tower Defense P1-0 fresh Gate G run：

```text
<TOWER_RUN_ROOT>\p1-0-gate-g-20260805-164431
```

结构化 blocked report：

```text
<TOWER_RUN_ROOT>\p1-0-gate-g-20260805-164431\evidence\gate-g-blocked-report.json
classification = real_editor_gameview_input_bridge
failedStep = recruit-one
nodeId = recruit-button
diagnosticCode = authority.runtime_action_not_observed
```

真实 Editor 已观察到 OS PointerDown / PointerUp，但 runtime action、AUI consumed event 和 gameplay
action 均为 0。该失败不是塔防 action map、AUI action identity、ProjectRuntimeSession 或 fixed update
问题，而是 presentation 与 input coordinate space 分裂。

当时四个事实为：

```text
AUI canvas reference extent = 1080 x 1920
Runtime GameView target extent = 1280 x 720
recruit-button center in ReferenceSpace ~= (451, 1779)
ViewportHost mapped RuntimeInputFrame point ~= (535, 667)
```

现有 `AuiInteractionSystem` 直接用 `(535,667)` 命中 `1080x1920` layout；真实按钮约位于
`x=376..526, y=1748..1810`，所以必然 miss。

现有链路：

```text
Authority AUI reference rect
  -> 按完整 GameView display rect 映射 OS logical point
  -> ViewportHost 按完整 display rect 映射 runtime texture pixel
  -> RuntimeInputFrame(TargetSpace)
  -> AuiInteractionSystem 直接命中 AUI layout(ReferenceSpace)
```

此外，RuntimeRenderer 当前从 draw-item 最大边界推断 `reference_extent`，并把该 extent 直接铺满
target NDC。内容未铺满 canvas、多个 canvas reference 不同或 target 与 reference 宽高比不同时，
该推断既不是 AUI Document 真相，也无法生成可靠的输入逆变换。

## 3. 与既有系统的关系

本方案深化而不替代以下已完成系统：

```text
219：Editor GameView shared GPU texture present
220：GameView focus / input routing / AUI consumed -> gameplay fallback
260：ProjectRuntimeSession AUI intent + FixedUpdate lifecycle
263：Production GameView real OS input route 与 action-target authority
265：Stable GameView surface / publication receipt / editor frame publication
```

保留的既有真相：

- 219/265 继续拥有稳定 surface、backing replacement、publication 与 WGPU queue 顺序。
- 220 继续拥有 focus、hover、capture、AUI consumed 与 gameplay fallback 顺序。
- 260 继续拥有 AUI business intent 到项目 session 的 exactly-once dispatch。
- 263 authority 继续只读查询 target 并发送真实 OS input，不直接注入 action。
- RuntimePackage 继续是项目运行内容真相；target extent 是 host/session presentation 选择，不写入
  AUI Document，不让 Runtime 扫描项目源目录。

本方案修正 220/263 中“display rect -> runtime texture pixel”只完成前半段、却没有继续完成
“runtime texture pixel -> AUI canvas reference point”的缺口。

## 4. 目标

v1 必须达到：

1. Editor GameView Play session 接受 typed target extent，支持至少 `1280x720`、`1080x1920`、
   `720x1280`。
2. target extent 真正进入 `RenderTarget::viewport_texture`、texture descriptor、stable surface backing
   与 publication receipt；禁止只改报告或截图尺寸。
3. AUI reference、runtime target、Editor display content rect 与 OS input 使用同一组解析后变换。
4. `Contain` 保持宽高比并产生确定的 content rect/gutters；gutters pointer 不进入 runtime。
5. `Stretch` 仅作为显式兼容策略，仍必须保持 projection 与 inverse input 一致。
6. AUI projection 使用 AUI presenter 输出的显式 canvas reference extent，不再从 draw item bounds 推断。
7. 多 canvas 按 `canvasId` 解析 reference transform；不得把一个全局比例静默套到所有 AUI node。
8. AUI 命中使用 ReferenceSpace，gameplay InputResolver 使用原始 TargetSpace；消费仍按同一 input event
   index 过滤，不复制 OS event。
9. authority 的 reference rect -> OS point 与真实 input 的 OS point -> reference rect 构成同一变换的正反路径。
10. resize、target change、display rect change、focus/capture 与 stable surface generation 具有明确生命周期。
11. Off / Summary / Trace diagnostics 能说明 extent、policy、content rect、gutters、generation、mapping
    与拒绝原因，正式 runtime 不默认生成长 trace 或写文件。
12. 默认 `1280x720` 现有调用方可兼容迁移，不要求一次修改所有项目资产。

## 5. 非目标

v1 不做：

```text
把 AUI referenceResolution 改成项目窗口尺寸
把 Tower P1-0 AUI 退回 1280x720
为 recruit-button、塔防节点或某个 scenario 写硬编码坐标
直接注入 td.recruit 或其它 authority action
完整多窗口/多设备模拟器、旋转传感器、安全区或刘海屏系统
动态分辨率、render scale、超采样、FSR/DLSS 或 3D camera viewport 重构
任意仿射/透视变换、旋转 GameView 或 world-space AUI picking
新增 DPI seam；继续复用现有 physical -> logical window contract
把 GameView target extent 存入 AUI Document
立即修改 BuildProfile schema 或 exported Player window policy
重做 219/265 stable surface、RHI、WGPU texture registry 或 frame publication
运行 Local CI、替换 production/安装态二进制、修改真实用户配置
自动重跑 Tower P1-0 fresh Gate G 或进入 Gate H
```

## 6. 成熟引擎源码结论

### 6.1 Unity GameView

参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\PlayModeView\PlayModeView.cs
```

关键实现：

```text
PlayModeView.targetSize
PlayModeView.RenderView
PlayModeView.ConfigureTargetTexture
```

`RenderView` 先以 `targetSize` 得到目标 width/height，再交给 `ConfigureTargetTexture`；尺寸变化时对
既有 `m_TargetTexture` 执行 `Release -> width/height -> Create`，而不是把 Editor panel 尺寸或 UI
reference 尺寸混成同一个值。

可学习点：GameView target extent 是显式 host 状态；稳定 target object 与 backing resize 分离。
不可照搬点：不复制 Unity IMGUI/Repaint、GameViewSize 全部设备模拟或 C# object lifecycle。

### 6.2 Godot Viewport

参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\scene\main\viewport.cpp
<GODOT_SOURCE>\godot-master\godot-master\doc\classes\Viewport.xml
<GODOT_SOURCE>\godot-master\godot-master\doc\classes\Window.xml
```

关键合同：

```text
Viewport.get_final_transform
Viewport.get_stretch_transform
Viewport.push_input(event, in_local_coords)
Window.content_scale_size / content_scale_mode / content_scale_aspect
```

Godot 明确区分 viewport coordinate 与 embedder coordinate；`push_input(..., false)` 会把 embedder
坐标转换到 viewport local，stretch 设置与 `get_stretch_transform` 使用同一 viewport truth。

可学习点：呈现变换和输入逆变换由同一 viewport owner 提供；调用方显式声明输入是否已经 local。
不可照搬点：不引入 Godot Viewport tree、notification、RID server 或完整 Window content-scale 面。

### 6.3 Unreal Slate / SceneViewport

参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\SlateCore\Public\Layout\Geometry.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Slate\SceneViewport.cpp
```

关键实现：

```text
FGeometry::AbsoluteToLocal
FGeometry::LocalToAbsolute
FSceneViewport::SetMouse
FSceneViewport::OnDrawViewport
FSceneViewport::ResizeViewport
```

`FGeometry` 用 accumulated render transform 及其 inverse 成对完成 absolute/local 转换；
`FSceneViewport` 将 viewport pixel 与 cached Slate geometry 明确归一化映射，并独立管理 viewport size。

可学习点：正反变换属于同一个 geometry owner；viewport size 与 widget geometry 分离。
不可照搬点：不复制 Slate WidgetPath、RHI/Slate 多线程对象体系或 Unreal viewport 全部功能。

### 6.4 综合结论

```text
设计/reference extent != render target extent != displayed content rect != OS window rect
target identity != backing size/generation
forward projection 与 inverse input 必须来自同一 transform owner
输入是否已在 local/reference space 必须由类型和 interface 表达，不能靠调用约定猜测
```

## 7. 核心架构决定

新增一个深 Module：

```text
AUI canvas reference facts + GameView target spec + Editor display slot
  -> GameViewPresentationModule.resolve(...)
  -> ResolvedGameViewPresentation
       - target content transform
       - display content transform
       - per-canvas reference transforms
       - inverse transforms / containment
       - compact diagnostics / identity
  -> RuntimeRenderer / ViewportHost / InputRoute / AUI Interaction / Authority
```

删除该 Module 后，contain arithmetic、gutter、rounding、outside rejection、canvas lookup、forward/inverse
与 diagnostics 会重新散落到至少五个调用方，因此它具有真实 depth、leverage 和 locality。

调用方不得直接读取 scale/offset 后再次拼装自己的公式；只允许通过解析结果的窄 interface 做映射。

## 8. 坐标空间

定义四个不同空间：

### 8.1 ReferenceSpace

AUI canvas 的逻辑布局空间，来源于 AUI Document `canvas.referenceResolution`，例如
`1080x1920`。`AuiLayout`、computed rect、effective clip 与 action target rect 均属于该空间。

ReferenceSpace 必须携带 `canvasId`；不同 canvas 可以有不同 reference extent。

### 8.2 TargetSpace

Runtime GameView render target 的 texture pixel 空间，例如 `720x1280`。Renderer output descriptor、
gameplay pointer、readback pixel 与 stable surface backing 属于该空间。

### 8.3 DisplaySpace

Native Editor logical coordinate 中 GameView texture 实际显示的 content rect。它不等于整个 panel rect；
`Contain` 时 panel 内 gutters 不属于 DisplaySpace content。

### 8.4 WindowSpace

OS client physical/logical input 与 screenshot 使用的空间。现有 winit DPI 转换先把 physical event 变成
Editor logical point，再进入本方案的 DisplaySpace mapping；本方案不新增第二套 DPI 换算。

## 9. Typed Interface

建议最小 interface：

```rust
pub struct GameViewExtent {
    pub width: u32,
    pub height: u32,
}

pub enum GameViewScalePolicy {
    Stretch,
    Contain,
}

pub struct GameViewPresentationSpec {
    pub target_extent: GameViewExtent,
    pub scale_policy: GameViewScalePolicy,
}

pub struct AuiCanvasReferenceSpace {
    pub canvas_id: String,
    pub reference_extent: GameViewExtent,
}

pub struct GameViewPresentationInput {
    pub spec: GameViewPresentationSpec,
    pub display_slot: PresentationRect,
    pub canvas_spaces: Vec<AuiCanvasReferenceSpace>,
}

pub struct ResolvedGameViewPresentation { /* opaque implementation */ }
```

解析结果只提供行为型 interface：

```rust
impl ResolvedGameViewPresentation {
    pub fn target_content_rect(&self) -> PresentationRect;
    pub fn display_content_rect(&self) -> PresentationRect;

    pub fn reference_to_target(
        &self,
        canvas_id: &str,
        point: PresentationPoint,
    ) -> Result<PresentationPoint, PresentationMapError>;

    pub fn target_to_reference(
        &self,
        canvas_id: &str,
        point: PresentationPoint,
    ) -> Result<Option<PresentationPoint>, PresentationMapError>;

    pub fn target_to_display(
        &self,
        point: PresentationPoint,
    ) -> Option<PresentationPoint>;

    pub fn display_to_target(
        &self,
        point: PresentationPoint,
    ) -> Option<PresentationPoint>;

    pub fn reference_to_display(
        &self,
        canvas_id: &str,
        point: PresentationPoint,
    ) -> Result<Option<PresentationPoint>, PresentationMapError>;

    pub fn display_to_reference(
        &self,
        canvas_id: &str,
        point: PresentationPoint,
    ) -> Result<Option<PresentationPoint>, PresentationMapError>;
}
```

`Option::None` 表示 point 位于 gutter/content rect 外；`Err` 表示 schema、canvas identity、extent 或
transform 无效。调用方不能把二者都降级为 clamp。

## 10. Transform 语义

### 10.1 Stretch

```text
scaleX = destination.width  / source.width
scaleY = destination.height / source.height
offset = destination.origin
```

允许非等比，但 projection 与 inverse input 必须使用同一 `scaleX/scaleY`。Stretch 不产生 gutters。

### 10.2 Contain

```text
scale = min(destination.width / source.width,
            destination.height / source.height)
contentWidth  = source.width  * scale
contentHeight = source.height * scale
offsetX = destination.x + (destination.width  - contentWidth)  / 2
offsetY = destination.y + (destination.height - contentHeight) / 2
```

剩余区域是 gutters。pointer down 在 gutter 中必须返回 outside，不得 clamp 到最近 AUI node；pointer
capture 已建立时按第 17 节处理。

### 10.3 边界与取整

- 变换内部使用有限 `f64` 或经过证明的稳定 `f32` 计算；输出到现有 Rust 类型时做显式转换。
- rect 使用 half-open `[min,max)` 语义，避免右/下边界同时命中 content 与 gutter。
- target pixel 只在提交 Renderer/RHI 时取整；reference/display hit-test 保持浮点。
- forward -> inverse 的误差预算由 owner test 固定，不允许调用方再次 round。
- width/height 为 0、NaN/Inf、overflow 或超过 backend texture capability 时 fail closed。

## 11. AUI Reference Truth

`AuiRuntimePresenter` 已拥有 resolved document 与 canvas reference resolution，因此它是 ReferenceSpace
唯一 owner。方案要求 present/composition 产物显式携带稳定、排序后的：

```text
canvasId
referenceExtent
stage membership / draw item canvasId
```

`RuntimeRenderer::input_reference_extent` 当前通过 draw item 最大边界推断 reference extent 的实现必须
退役。draw item bounds 只描述内容，不是 canvas size。

Renderer 按 draw item 的 `canvasId` 取得对应 reference transform。缺 canvas、重复冲突 canvas 或同一
canvas 在同一 present 中报告不同 extent 时产生 typed error；不得 fallback 到 target extent 后继续声称
input 一致。

## 12. GameView Target 选择

target extent 是 Editor Play host/session presentation 输入，不是 AUI layout 属性。

建议接入：

```text
EditorGameViewPlay start/options
  -> GameViewPresentationSpec
  -> EngineHostLoop session-owned render target selection
  -> RenderFramePacket.render_target
```

v1 兼容规则：

- 旧调用方未提供 spec 时，合成明确的 legacy spec：`1280x720 + Stretch`。
- 新竖屏 session/authority scenario 必须显式提供 `targetExtent + Contain`。
- 同一项目可以分别启动 `1080x1920` 与 `720x1280` session；不得把单一矩阵尺寸写死到项目 AUI。
- spec 必须进入 session identity/report；测试/authority 不得通过环境变量或隐藏全局覆盖。
- v1 不修改 BuildProfile；未来 Player/window policy 可复用本 Module，但必须另行讨论。

## 13. Runtime Renderer 集成

`EngineHostLoop` 不再在 output path 固定构造：

```text
RenderTarget::viewport_texture("viewport-main", 1280, 720)
```

而是消费 session-owned `GameViewPresentationSpec.target_extent`。结果必须贯穿：

```text
RenderTarget
RenderGraph view
ViewportTextureDescriptor
RuntimeRenderTargetSummary
GpuTextureDescriptor
ProducedGameViewFrame
GameViewPublicationReceipt
WGPU backing allocation/readback
```

AUI geometry/font projection 按 canvas reference -> target content transform 生成 NDC；不得继续默认把
reference rect 直接铺满 `[-1,1]`。`Contain` gutter 由 render pass 明确清成稳定背景色，不保留上一帧像素。

世界/Gameplay renderer 继续以 target extent 决定 camera aspect；本方案只统一 GameView presentation
与 screen AUI mapping，不重做 camera policy。

## 14. Editor Display 集成

Editor draw-list 在 GameView panel 的 texture slot 内按 target -> display transform 得到真实 content rect：

```text
panel texture slot
  -> target aspect + scale policy
  -> display content rect
  -> draw stable GameView surface
```

`ViewportHost` 注册该 content rect，而不是完整 panel/slot rect。focus、hover 与 pointer route 只对 content
rect 成立；gutter 仍属于 Editor UI，不进入 RuntimeInputFrame。

窗口 resize、dock resize 或 DPI 后 layout 变化只更新 display transform，不替换 runtime backing；target extent
变化才按 265 合同触发 surface backing resize/generation。

## 15. Input 与 AUI Interaction 集成

真实 pointer 路径：

```text
OS physical event
  -> existing winit DPI conversion
  -> Editor logical point
  -> ResolvedGameViewPresentation.display_to_target
  -> RuntimeInputFrame(TargetSpace)
  -> AuiInteractionSystem per-node canvas lookup
       -> target_to_reference(canvasId)
       -> AUI hit-test(ReferenceSpace)
  -> consumed_event_indices
  -> filter original RuntimeInputFrame(TargetSpace)
  -> gameplay InputResolver(TargetSpace)
```

关键规则：

1. 不把整个 RuntimeInputFrame 永久改写成某一个 AUI reference space。
2. AUI hit-test 根据 candidate node/computed node 的 `canvasId` 使用对应 inverse transform。
3. 同一 event index 即使针对多个 canvas 做候选映射，也只能被 AUI 消费一次、dispatch 一次。
4. gameplay 获得未消费的原 TargetSpace frame；world pick、移动、射击不接收 AUI reference 坐标。
5. wheel/drag pointer position 使用相同 mapping；keyboard/text/IME 不做坐标缩放。
6. canvas transform 不存在或失效时，该 canvas 不可交互并输出 diagnostic；不得退回 TargetSpace 直接命中。

## 16. Authority 与 Action Target

`GameViewAuiActionTarget` 保留业务无关的只读 facts：

```text
nodeId / actionId
canvasId
computedRect / effectiveClip
visible / interactable
presentationIdentity / revision
```

不再由 authority 使用 `referenceWidth/referenceHeight` 自己做完整 viewport 比例公式。

authority 路径改为：

```text
actionable rect center(ReferenceSpace, canvasId)
  -> ResolvedGameViewPresentation.reference_to_display
  -> existing Editor logical -> OS client input path
  -> real PointerDown / PointerUp
```

输入返回时必须经第 15 节逆路径命中相同 target。authority report 记录 transform identity/revision、
canvasId、reference point、target point、display point 与 outside/diagnostic，但不得记录或注入项目 action payload。

## 17. 生命周期、Resize 与 Capture

### 17.1 Session start

先校验 spec 与 backend capability，再创建 render target/surface。invalid extent 在 session start fail closed，
不能启动一个报告竖屏、实际仍为 1280x720 的 session。

### 17.2 Target extent change

target extent 或 format 变化：

```text
cancel active AUI/gameplay pointer capture
clear hover/pressed/drag transient state
resize stable surface backing
advance surface generation
resolve new presentation
publish first successful new-size frame
```

旧 receipt/transform 不得映射新 input。

### 17.3 Display rect change

只重新解析 target -> display transform 和 presentation revision；不 tick Runtime、不改 target backing、
不推进 surface generation。若 pointer capture 正在进行，为避免 down/up 使用不同 geometry，必须取消该 capture。

### 17.4 AUI canvas reference change

RuntimePackage/session 内 canvas reference facts 改变时清理对应 AUI transient state、推进 presentation revision，
并要求下一批 input 使用新 revision。不存在跨 revision 的 pressed/drag 继承。

## 18. Identity、Report 与 Diagnostics

compact identity 至少绑定：

```text
sessionId / targetId
targetExtent / format / surfaceGeneration
scalePolicy
displayContentRect logical facts
ordered canvasId + referenceExtent digest
presentationRevision
```

建议稳定 diagnostic code：

```text
game_view.presentation.target_extent_invalid
game_view.presentation.target_capability_exceeded
game_view.presentation.display_rect_invalid
game_view.presentation.canvas_missing
game_view.presentation.canvas_extent_conflict
game_view.presentation.point_outside_content
game_view.presentation.revision_stale
game_view.presentation.capture_cancelled_by_transform_change
game_view.presentation.inverse_non_finite
```

分档：

- `Off`：正式 Runtime 热路径不生成字符串 trace，仅保留功能必需 typed result。
- `Summary`：extent、policy、content/gutter rect、revision、outside/rejection counters。
- `Trace`：测试/authority 显式启用，记录单次 forward/inverse mapping 与 canvas identity。

不得每帧写 JSON、计算长 digest string 或保存完整 pointer history。

## 19. 兼容与迁移

### 19.1 现有 1280x720

未提供 spec 的现有 caller 使用合成 legacy `1280x720 + Stretch`，保持当前 target 和画面尺寸；但输入
仍改为消费共享 inverse transform，删除旧散落公式。

### 19.2 1080x1920

ReferenceSpace 与 TargetSpace 相同；reference/target 为 identity，target/display 由 contain 处理。

### 19.3 720x1280

ReferenceSpace `1080x1920` 到 TargetSpace `720x1280` 为等比 `2/3`；AUI input inverse 为 `3/2`。
Renderer 与 hit-test 必须在 round-trip 误差预算内命中同一 rect。

### 19.4 宽高比不同

Contain 产生 gutters；画面不拉伸，gutter pointer rejected。该用例是 Module owner test 和第二项目通用性
证明，不代表 Tower P1-0 重新纳入 landscape qualification。

## 20. 验收矩阵

### 20.1 Module owner

```text
1280x720 -> 1280x720 identity
1080x1920 -> 1080x1920 identity
1080x1920 -> 720x1280 equal-aspect scale + round-trip
1080x1920 -> 1280x720 contain + centered gutters
Stretch non-uniform forward/inverse consistency
left/top inclusive, right/bottom exclusive
gutter outside rejection
zero/overflow/non-finite/capability-exceeded fail closed
unknown canvasId / conflicting extent deterministic error
multiple canvasId with distinct reference extents map independently
```

### 20.2 Runtime renderer / surface

```text
selected target extent reaches RenderTarget/RenderGraph/descriptor/backing/receipt
AUI geometry and glyph projection use explicit canvas reference facts
draw-item bounds no longer infer canvas extent
contain gutters cleared deterministically
target resize advances generation; display resize does not
same spec/frame has stable presentation identity
```

### 20.3 Input / AUI / gameplay

```text
1080x1920 recruit-button equivalent fixture exactly-once consumed
720x1280 target maps to 1080x1920 AUI and exactly-once consumed
AUI consumed pointer does not leak into gameplay
AUI outside pointer reaches gameplay in TargetSpace
gutter pointer generates no RuntimeInputFrame
down/up across unchanged frames composes one click
resize/revision change cancels capture and produces no stuck pressed/action
keyboard/text/IME path does not receive coordinate scaling
```

### 20.4 Editor / authority

```text
GameView texture preserves target aspect inside landscape/portrait panel
ViewportHost registers actual content rect
authority reference -> display -> target -> reference round-trip hits same node
OS down/up observed and runtime AUI action observed
普通 Editor 与 production authority 消费同一 Module，无 authority-only transform
```

### 20.5 通用性

```text
既有 1280x720 AUI fixture 不回归
Switch Puzzle 或另一通用项目证明非 Tower 特判
源码不存在 projectId/tower/recruit-button/td.recruit 分支
```

### 20.6 后续真实资格

引擎施工与受影响回归完成后，Tower P1-0 Gate G 仍需新的 fresh run 授权，并在 run-owned root 中证明：

```text
真实 Editor Play 四轮流程
1080x1920 视觉矩阵
720x1280 视觉矩阵
真实 GameView OS input
Restart / Stop-Play isolation
```

旧 blocked run 只作首因证据，不得修改后伪装为通过。

## 21. 建议施工窗口

本文不是施工文档。273 待执行施工文档已按以下窗口生成；只有先行施工正式完成或让出唯一施工槽、
273 完成激活前复核且移入 `施工文档/当前/` 后才可执行：

```text
Window 1 / Gate A：GameViewPresentationSpec、纯 transform Module、owner tests、diagnostics
Window 2 / Gate B：AUI canvas reference facts、renderer projection、hardcoded target removal
Window 3 / Gate C：Editor Play target selection、stable surface resize/publication integration
Window 4 / Gate D：Viewport display content rect、input inverse、AUI per-canvas hit-test
Window 5 / Gate E：authority shared mapping、第二项目、real-window targeted smoke
Window 6 / Gate F：受影响 crate regression、文档闭环；不含 Local CI/production replacement
```

不得先在 authority 中补一个 reference ratio 再回填 Module；Gate A-D 是真实产品链路前置。

## 22. 预计涉及文件与 ownership

全部为 `engine-owned`，已获得引擎能力方向授权，但在施工文档激活前不可修改：

```text
rust/crates/engine_runtime/src/game_view_presentation.rs（建议新增）
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/play_session.rs（如 session options owner 需要）
rust/crates/editor_window_winit/src/viewport.rs
rust/crates/editor_window_winit/src/input_route.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/editor_frame_publication.rs（仅消费 extent/generation）
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/project_e2e_gate/src/project_editor_composition.rs（通用 consumer 证据，如适用）
```

Tower 项目在本引擎施工中保持只读；只有后续 fresh Gate G 独立授权才消费该能力。

## 23. 风险与控制

### 风险 1：Module 退化成共享数学工具，调用方仍自己拼公式

控制：解析结果提供行为型 forward/inverse interface；scale/offset 保持 opaque。源码 guard 搜索旧比例公式和
authority 手算路径，迁移后删除重复实现。

### 风险 2：AUI 与 gameplay 都被改成 reference space

控制：RuntimeInputFrame 明确属于 TargetSpace；AUI 仅在 candidate hit-test 内按 canvas 映射，filter 后的
原 frame 进入 InputResolver。

### 风险 3：多 canvas reference extent 混淆

控制：canvasId 是 transform key；present 显式输出 canvas facts；未知/冲突 identity fail closed，不选择
“第一个 canvas”或最大 draw bounds。

### 风险 4：Contain 只修输入，画面仍被 texture slot 拉伸

控制：同一 policy 同时进入 reference->target projection 与 target->display draw rect；视觉和 round-trip
测试必须同时证明。

### 风险 5：resize 破坏 265 stable surface

控制：target resize 复用 265 backing replacement/generation，display resize 不替换 backing；receipt 绑定
target extent/generation，旧 revision fail closed。

### 风险 6：为 Gate G 增加 authority 私有入口

控制：authority 只能消费普通 Editor Play session spec 和共享 transform；禁止 environment-only override、
直接 RuntimeInputFrame 注入或 action dispatch。

### 风险 7：范围扩张到 Player/BuildProfile/DPI

控制：v1 target spec 为 Editor Play host/session 输入；Player/BuildProfile 和新 DPI seam 明确 deferred。

## 24. 方案自审

### 24.1 是否满足用户确认的方案 C

是。方案同时覆盖显式竖屏 target、共享 presentation transform、Renderer、display、input、AUI hit-test
与 authority，不是 input-only 补偿。

### 24.2 是否为深 Module

是。调用方只提供 spec、display slot 和 canvas facts，并调用 forward/inverse 行为；contain/stretch、gutter、
rounding、identity、outside/error 与 diagnostics 收在单一实现内。

### 24.3 是否保持项目无关

是。interface 只识别 extent、policy、rect、canvasId 与 session/target identity，不识别塔防玩法、action id、
项目路径或 scenario step。

### 24.4 是否保持现有 owner

是。AUI Presenter 拥有 reference facts，EngineHostLoop/Renderer 拥有 target，Editor layout 拥有 display slot，
265 拥有 surface publication，220 拥有 input routing。本 Module 统一 geometry 语义，不夺取这些生命周期 owner。

### 24.5 是否兼容既有项目

是。旧调用方合成明确的 `1280x720 + Stretch`，然后迁移到共享 inverse；新竖屏 session 显式选择 contain。
方案不要求修改现有 AUI Document 或 BuildProfile。

### 24.6 是否可验证和 AI 友好

是。schema、canvas facts、presentation identity/revision、typed mapping result、diagnostics 与矩阵均确定；
AI 可从 report 判断错误发生在 reference、target、display、outside、stale revision 或 backend capability。

### 24.7 与当前施工槽是否冲突

用户已明确批准从 272 切换施工槽并激活/施工 273。272 当时的让位记录已保全；273 已完成 Gate A-F、
真实竖屏 matrix 与受影响回归，施工文档位于 `施工文档/已完成/`，当前施工槽为空。

### 24.8 自审结论

```text
方案结论：通过
用户选择：方案 C
正式方案：已生成
施工文档：已完成，位于已完成
当前允许施工：否，273 已归档
当前唯一施工：无
需要修改 Tower 项目：否
Local CI：未授权
production/安装态替换：未授权
fresh Gate G：本方案生成不自动授权重跑
```

## 25. 273-R1 720 GameView Input Bridge Remediation

2026-08-06 的 Tower Gate G R7 证明 720 GameView AUI target、OS down/up 与坐标 evidence 完整，
但 runtime action 未发布。source-built 结构化诊断确认 recruit 的最终 route 为
`UiConsumed / ui_hit / PointerUp`。首因不是 presentation mapping 或 AUI per-canvas consumer，
而是 toolbar overflow barrier 覆盖 GameView 时，application 把 gateway 的 `UiConsumed` 误当成
已经完成 UI activation 并提前返回，导致 barrier 没有真正关闭。

273-R1 保留 `UiConsumed` route evidence，但只允许 `RuntimeInputFrame` 终止 GameView routing；
Editor UI hit 继续下落到 retained UI activation。owner 回归与 input/AUI consumer filters 全绿，新的
source-built `720x1280 + Contain` 真实 OS scenario 已观察到 `td.recruit`，Play/Stop 均通过 toolbar
overflow。未修改 Tower 项目、真实配置、production/安装态二进制；未运行 Local CI，也未重跑 Tower
fresh Gate G 或进入 Gate H。
