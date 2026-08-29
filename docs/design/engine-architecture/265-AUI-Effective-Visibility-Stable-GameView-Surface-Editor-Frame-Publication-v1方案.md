# 265 AUI Effective Visibility / Stable GameView Surface / Editor Frame Publication v1 方案

## 1. 状态与结论

```text
系统编号：265
方案：C
状态：用户已确认，正式方案已生成并自审
日期：2026-08-03
上游：219 GameView GPU Sharing；260 Runtime Session；263 Production Authority；264 Production Wait
下游：Tower Defense P0-5 Gate G visual remediation
施工状态：未生成施工文档，不可施工
```

采用方案 C：

```text
AUI Effective Visibility
  + Stable GameView Surface
  + Compact Publication Receipt
  + 深 EditorFramePublicationModule
```

本方案同时修正两个独立但共同阻断真实 Editor Play 视觉验收的问题：AUI 父节点隐藏时后代
仍可能绘制或交互；GameView 把 runtime frame 编进纹理身份，导致 Editor draw list 与纹理
registry 在同一次 present 中引用不同帧。

核心合同：

```text
effectiveVisible = parent.effectiveVisible && localVisible

RuntimeContentIdentity = sessionId + frameIndex + frameHash
PublicationIdentity    = surfaceId + surfaceGeneration + publicationIndex
```

`visible=false` 只关闭绘制和交互，不折叠布局。GameView `surfaceId` 在一个 session/target
生命周期内稳定；redraw、present、wait 和 capture 都不能隐式推进 Runtime。

## 2. 真实阻断证据与本地首因

### 2.1 真实证据

Tower Defense P0-5 Gate G fresh run：

```text
<TOWER_RUN_ROOT>\p0-5-gate-g-20260802-121707
```

1280x720 组织阶段 HUD 重叠：

```text
<TOWER_RUN_ROOT>\p0-5-gate-g-20260802-121707\evidence\production-authority\tower-gate-g-organizing-1280\organizing-1280x720.png
```

1600x900 战斗阶段 GameView 蓝灰空白：

```text
<TOWER_RUN_ROOT>\p0-5-gate-g-20260802-121707\evidence\production-authority\tower-gate-g-full-flow-1600\combat-1600x900.png
```

264 已解决业务状态等待语义，但明确没有修复视觉缺口。263/P0-5 Gate G 仍保持 blocked。

### 2.2 AUI 首因

`rust/crates/engine_runtime/src/aui.rs` 当前把 computed visibility 直接设为本地
`node.visible`。隐藏祖先不会使后代有效隐藏；draw、hit-test、focus 和 navigation 也没有
共同的 effective visibility 真相。

这不是塔防布局参数问题。项目侧逐个隐藏后代会复制层级知识，也不能正确释放 modal、focus、
pressed、drag、scroll capture 或 IME 状态。

### 2.3 GameView 首因

1. `rust/crates/engine_runtime/src/runtime_renderer.rs` 用 `target::frame-N` 生成纹理 ID。
2. `rust/crates/editor_window_winit/src/application.rs` 的 present getter 在部分路径隐藏 Runtime tick。
3. `rust/crates/editor_window_winit/src/real_window.rs` 普通路径与 production authority 路径采用
   不同 publish/compose 顺序；后者可以先 compose 引用 N，再发布 N+1。
4. `rust/crates/editor_wgpu_renderer/src/viewport_texture.rs` 即使规格未变也创建 GPU texture；
   `real_wgpu.rs` 在 registry 找不到引用时绘制蓝灰 placeholder。

因此 1600x900 空白不是项目没有渲染，也不是字体、DPI 或 Production Wait 问题，而是 Editor
frame publication 的身份与顺序合同缺失。

## 3. 目标

```text
Runtime advance authority
  -> ProducedGameViewFrame
  -> EditorFramePublicationModule
       -> ensure stable surface
       -> submit runtime write
       -> issue publication receipt
       -> compose Editor draw list
       -> sample stable surface
       -> present Editor window
       -> optional exact capture
```

265 的目标：

1. AUI 层级隐藏对 draw 和全部交互路径语义一致。
2. 隐藏子树不残留 focus、capture、pressed、drag、IME 或 modal 状态。
3. GameView draw list 引用稳定 surface 身份，不引用逐帧纹理 ID。
4. surface backing 只在尺寸、格式、色彩空间或 device backing 变化时替换。
5. Runtime 内容身份和 Editor publication 身份分离且可关联。
6. Runtime advance 与 redraw/present/wait/capture 严格分离。
7. 普通帧只依赖同一 WGPU queue 的提交顺序，不增加 CPU fence/readback。
8. 精确截图绑定指定 publication，而不是未经证明的“最新帧”。
9. Off/Summary/Trace 和 typed diagnostics 支持 AI 定位失败，不污染热路径。
10. 通用项目和塔防使用同一公共 seam。

## 4. 非目标

- 修改塔防回合、军粮、军略、单位、战斗时长或任何 gameplay 规则。
- 新增塔防专用 AUI、GameView、截图或 production authority 分支。
- `display:none`、布局折叠、opacity 继承、遮挡剔除或 accessibility 全量重做。
- 多 GameView、跨 GPU/device 或外部进程纹理共享。
- 普通帧 CPU `queue.onSubmittedWorkDone`、GPU fence 或 texture readback。
- 新增 DPI seam；现有 logical/physical size 合同保持不变。
- 复用 Production Wait 的 `observationRevision` 作为视觉帧号。
- 修改 264 Observation Contract 或让像素成为 gameplay completion 真相。
- 重跑 Gate G、进入 Gate H、运行 Local CI 或替换 production/安装态二进制。

## 5. 成熟引擎源码参考

### 5.1 Godot

Godot 4.7.1 的 `CanvasItem` 分离本地和树上有效可见性，父级变化沿树传播；Control 隐藏时
同时处理 focus 和输入状态。`ViewportTexture` 对外保留稳定 proxy RID，底层 render target
可以变化；RendererViewport 保证子 viewport 先写、父 viewport 后采样。

学习 local/effective 分层、隐藏后的 interaction cleanup、稳定资源 handle 与 write-before-read。
不照搬 Godot 全套 notification、RID server 或 viewport tree。

### 5.2 Bevy

Bevy 0.19 使用 `Visibility / InheritedVisibility / ViewVisibility` 分离用户声明、层级继承和
逐 view 判定；布局折叠由 `Display::None` 独立表达。Render-to-texture 使用稳定
`Handle<Image>`，camera order/render graph 保证生产者先写、消费者后采样。

学习 visibility 与 layout display 分离、稳定 handle 与内容版本分离。不引入 Bevy ECS plugin
或通用 render graph 重构。

### 5.3 Unity

源码：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\PlayModeView\PlayModeView.cs
```

Unity 6000.6.0a7 的 `PlayModeView.ConfigureTargetTexture` 保留同一个 `m_TargetTexture` 对象；
尺寸变化时执行 `Release -> 更新 width/height -> Create`，不是每帧更换对象身份。

学习稳定 target object 与 resize backing lifecycle。不照搬 IMGUI/Repaint 或 C# 对象模型。

### 5.4 Unreal

源码：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Engine\Private\Slate\SceneViewport.cpp
```

Unreal 5.8 的 `FSceneViewport` 长期持有 `FSlateRenderTargetRHI`，render-thread backing 通过
`SetRHIRef` 更新，Slate 通过稳定 shader resource 采样 viewport 内容。

学习稳定 UI handle 和 owner-managed backing replacement。不复制 Unreal RHI/Slate 线程架构。

### 5.5 综合结论

```text
声明状态 != 有效状态
稳定资源身份 != 每帧内容身份
资源 backing 生命周期由 owner 封装
生产者写入必须先于 UI consumer 采样
```

## 6. 方案对比与选择

### 6.1 方案 A：项目补条件并调整一个 runner 的调用顺序

改动最少，但只修一个项目和一个 present 路径；普通 Editor、测试 getter、resize 和 capture
仍可能复现。拒绝。

### 6.2 方案 B：保留 frame texture ID，registry 缓存多帧纹理

可以暂时让 N 与 N+1 并存，但把身份错误变成 GPU 内存、淘汰和 stale draw list 问题，隐藏 tick
仍存在。拒绝。

### 6.3 方案 C：有效可见性 + 稳定 surface + 深 publication Module

一次覆盖所有项目和 Editor present caller；排序、generation、fallback、capture 和诊断集中在
一个 Module；普通帧无 CPU/GPU 同步成本；receipt 使 AI 可核对身份和顺序。需要跨 AUI、
Runtime/Editor frame contract、WGPU registry 和真实窗口施工，但长期复杂度最低。选择 C。

## 7. AUI Effective Visibility 合同

### 7.1 概念分层

```text
localVisible         作者、binding 或 runtime transient state 写入的本地声明
effectiveVisible     沿父子树计算后的有效可见性
layoutParticipation  v1 恒为参与布局；未来独立能力
```

```text
root.effectiveVisible  = root.localVisible
child.effectiveVisible = parent.effectiveVisible && child.localVisible
```

计算按确定性的 parent-before-child 顺序执行。缺父、循环或无效 hierarchy 仍由现有 document/
hydration validator 拒绝，visibility pass 不猜测修复。

### 7.2 `visible=false` 的精确定义

- 不产生 draw item。
- 不参与 pointer hit-test。
- 不能成为 keyboard/gamepad navigation candidate。
- 不能获得或保留 focus。
- 不能成为 action target 或 modal blocker。
- 仍参与 layout measurement 和 arrangement。

因此可见性切换不使兄弟节点 reflow。未来布局折叠必须单独设计
`display/layoutParticipation`，不得改变 265 的 `visible` 语义。

### 7.3 唯一消费真相

draw extraction、hit-test、focus、navigation、action target、modal blocking、scrollbar 和 input
field interaction 全部只读取 `effectiveVisible`，不得各自遍历 ancestor 或读取 local visible。
layout 不以 `effectiveVisible` 过滤。

## 8. Interaction State Reconciliation

子树从 effectively visible 变为 hidden 后，AUI Runtime 必须在处理下一批输入前：

- 清除隐藏子树内 focus，并按既有 default-focus 规则选择可见候选。
- 取消 pressed，不生成 click/action。
- 取消 drag，释放 pointer capture。
- 释放 scroll/thumb capture。
- 取消 IME preedit，结束 text-input ownership。
- 隐藏 modal root 时释放 modal capture，按剩余有效 modal stack 重算阻挡。
- navigation current item 隐藏时按既有确定性顺序恢复。

reconciliation 是 AUI interaction Module 内部实现，不增加项目 callback。它必须幂等，同一
computed visibility revision 重复调用不得生成重复 cancel/action。

## 9. 身份模型

### 9.1 Runtime 内容身份

```text
RuntimeContentIdentity {
  sessionId,
  frameIndex,
  frameHash
}
```

`frameIndex` 只由正式 Runtime advance 增加；`frameHash` 是验证和诊断事实，不充当 GPU key。

### 9.2 Publication 身份

```text
PublicationIdentity {
  surfaceId,
  surfaceGeneration,
  publicationIndex
}
```

- `surfaceId` 由 `(sessionId, targetId)` 确定性生成，在该生命周期内稳定。
- generation 只在 backing 规格或 device backing 成功替换时增加。
- publicationIndex 只在 Runtime 写入命令成功提交到 owner queue 后增加。

相同 Runtime 内容被再次 present 不产生新 publication；同一 surface 发布下一 Runtime 内容只
增加 publicationIndex。

### 9.3 Registry entry

Editor draw list 使用 `surfaceId`，不再使用 `target::frame-N`。registry entry 持有：

```text
surfaceId / sessionId / targetId / generation
extent / format / colorSpace / deviceIdentity
lastSuccessfulPublication
backing texture/view/sampler
```

consumer 不得缓存裸 texture/view 越过 generation。backing 替换、registry 更新和旧资源
retirement 由 surface owner 封装。

## 10. ProducedGameViewFrame 与 Receipt

### 10.1 ProducedGameViewFrame

Runtime advance authority 输出不可变 typed value，概念形状：

```rust
pub struct ProducedGameViewFrame {
    pub content: RuntimeContentIdentity,
    pub target_id: String,
    pub extent: PhysicalExtent,
    pub format: GameViewSurfaceFormat,
    pub color_space: ColorSpace,
    pub render_payload: GameViewRenderPayload,
}
```

`render_payload` 是受控 opaque 输入；Editor caller 不读取底层 command plan，也不能生成第二份
纹理身份。

### 10.2 Compact receipt

```rust
pub struct GameViewPublicationReceipt {
    pub content: RuntimeContentIdentity,
    pub publication: PublicationIdentity,
    pub extent: PhysicalExtent,
    pub format: GameViewSurfaceFormat,
    pub submit_serial: u64,
    pub status: PublicationStatus,
}
```

receipt 只表示 stable surface 写入已按 queue 顺序提交，不伪称 GPU 已完成。失败不签发成功
receipt，也不推进 publicationIndex。typed failure 至少包括：

```text
publication.session_mismatch
publication.invalid_extent
publication.unsupported_format
publication.surface_allocation_failed
publication.surface_replacement_failed
publication.runtime_write_submit_failed
publication.editor_compose_failed
publication.present_failed
publication.capture_readback_failed
publication.device_lost
```

## 11. 深 EditorFramePublicationModule

### 11.1 Seam 与 Interface

```rust
pub trait EditorFramePublicationModule {
    fn publish(
        &mut self,
        request: EditorFramePublicationRequest<'_>,
    ) -> EditorFramePublicationResult;

    fn last_good(&self, target: &GameViewTargetKey)
        -> Option<&GameViewPublicationReceipt>;
}
```

request 只含 optional newly produced frame、active session/target、Editor UI model input、window
surface facts、`Present | ExactCapture` mode 和 report level。它不接受排序 flag、texture ID、
generation、queue fence 或 fallback policy，这些复杂度属于 Module implementation。

### 11.2 内部依赖与真实 seam

Module 接受既有 owner：viewport texture registry、Runtime render submitter、Editor draw-list
composer、WGPU window renderer 和 optional capture adapter。真实 WGPU adapter 与 deterministic
test adapter 构成真实内部 seam，不扩张外部 Interface。

测试通过 Module 结果和 receipt 观察行为，不穿透私有 helper。若删除此 Module，surface
ensure/resize、generation、runtime write、receipt、compose order、present/capture、last-good 和
device-loss 知识会重新散落到多个 caller，因此它具有真实 depth，不是 pass-through。

## 12. 唯一顺序不变量

有新 `ProducedGameViewFrame` 时：

```text
1. validate active session/target and frame monotonicity
2. ensure or replace stable surface backing
3. encode and submit Runtime write to that backing
4. issue GameViewPublicationReceipt
5. compose Editor draw list referencing stable surfaceId
6. submit Editor UI sampling after Runtime write on the same queue
7. present Editor window
8. if requested, exact capture the receipt-bound publication
```

无新 Runtime frame 的 redraw：

```text
reuse last-good receipt/surface -> compose -> sample -> present
```

它不得重复 Runtime write、增加 publicationIndex 或调用 Runtime tick。普通窗口、production
authority 和视觉 gate 必须使用同一个 Module，不保留第二套顺序实现。

## 13. Runtime Advance 与 Present 分离

只有 running fixed-step scheduler、显式 `StepFrame` 和明确拥有 advance authority 的测试输入
可以推进 Runtime。

window redraw、frame getter、Editor compose、window present、Production Wait polling、capture、
resize、occlusion restore 和 report read 都不能推进 Runtime。

带隐藏 tick 的 present getter 必须替换为只读 last-produced/last-published 查询。调用者需要新帧
时显式经过 advance authority，得到 `ProducedGameViewFrame` 后再交给 publication Module。

## 14. Resize、Fallback 与生命周期

### 14.1 Resize/format change

extent、format、colorSpace、deviceIdentity 不变时复用 backing，generation 不变。变化时先创建并
验证新 backing，成功后原子替换 registry、generation +1，再在新 generation 提交本次写入；
旧 backing 按 WGPU 生命周期安全退休。创建失败不得先销毁 last-good，也不签发新 generation。

### 14.2 Last-good

- 暂无新 Runtime frame：正常重绘 last-good。
- resize replacement 失败：可按比例显示旧 surface，并输出 degraded diagnostic。
- 尚无成功 publication：显示明确 unavailable placeholder 和原因。
- Runtime submit 失败：保留 last-good，不推进 publicationIndex。
- compose/present 失败：publication receipt 可存在，但 present result 必须失败；两者不合并。

### 14.3 Stop/restart/device loss

Stop 使该 session surface 不再是 active GameView。Restart 生成新 sessionId/surfaceId，旧 receipt
不能满足新 session capture。device loss 使关联 generation 失效；恢复后同一 active surfaceId
使用新 deviceIdentity 和递增 generation，首个成功 publication 前不得伪造旧 GPU 资源有效。

## 15. WGPU Queue 与 Exact Capture

普通帧的 Runtime write 和 Editor sample 使用同一 WGPU queue，严格前者先提交。queue ordering
提供 write-before-read，不执行 CPU wait、map/readback 或 fence。若施工发现不能保证同一 queue，
必须暂停回填方案，不能静默增加跨 queue 假设。

Exact capture 绑定 `PublicationIdentity`，在目标 write 后编码 copy/readback，并仅为显式截图
等待完成。结果至少包含：

```text
sessionId / runtimeFrame / frameHash
surfaceId / surfaceGeneration / publicationIndex
capturedExtent / pixelDigest
```

capture 不推进 Runtime、不重新 publication，也不把未经核对的 registry 最新纹理当目标。

## 16. Production Wait / Observation Contract 边界

| 项 | 264 Production Wait | 265 Visual Publication |
|---|---|---|
| 真相 | post-commit project snapshot | stable GameView surface publication |
| 身份 | sessionId + runtimeFrame + path | surfaceId + generation + publicationIndex |
| 用途 | 判断业务状态到达 | 判断哪份视觉内容被发布/捕获 |
| 是否推进 Runtime | 否 | 否，只消费 ProducedGameViewFrame |
| 是否读取像素 | 否 | 仅 exact capture |

两者只通过 `sessionId + runtimeFrame` 关联。265 不新增、复用或重解释
`observationRevision`。Production authority 可以先等待业务状态，再捕获不早于该 runtimeFrame
的 publication，但不形成第三套时钟。

## 17. Report 与 Diagnostics

### 17.1 Off

只保留 active last-good receipt 和功能必需错误状态；不写 JSON、不存帧历史、不生成长字符串。

### 17.2 Summary

记录 active surface/generation、last runtimeFrame/publicationIndex、extent/format、new publication
或 redraw reuse/degraded fallback、present/capture status 和 diagnostic codes。

### 17.3 Trace

仅测试、gate、debug 或显式诊断记录 produced identity、allocation/replacement decision、submit
ordering、receipt、compose reference、present/capture result 和 reconciliation counts。Trace
仍有界，不保存 GPU payload、全量像素或无限历史。

## 18. 性能预算与复杂度护栏

- effective visibility 每次 computed update 为 O(node count)，不对每节点回溯 ancestors。
- reconciliation 只处理 interaction owners 和受影响 visibility facts。
- 规格不变时每个 active GameView 只保留一个 stable backing，不逐帧分配纹理。
- 普通 publication 为一次 Runtime write submit + 一次 Editor UI submit，无 CPU wait/readback。
- redraw reuse 为零 Runtime write、零 publicationIndex increment。
- exact capture 仅显式请求时承担 copy/map/readback。
- 不新增多个 public Surface/Publication/Capture coordinator。
- 不让 caller 传排序 flag、generation 或 texture ID。
- 不以多帧 GPU cache 修补 identity，不把 layout collapse 塞进 visible。

## 19. 通用项目矩阵与塔防验收

### 19.1 通用矩阵

| 场景 | 必须证明 |
|---|---|
| parent hidden | 后代不 draw/hit/focus，布局几何保持 |
| hidden focused input | focus/IME/capture 清理，无重复 action |
| stable-size frames | surfaceId/generation 稳定，publicationIndex 单调 |
| redraw without advance | runtimeFrame/publicationIndex 不变 |
| resize success | surfaceId 稳定，generation +1，新规格 present |
| resize failure | last-good 保留，typed degraded diagnostic |
| session restart | 新 surfaceId，旧 receipt 被拒绝 |
| exact capture | receipt 与 pixel evidence 一致 |
| second project | 无塔防概念也走同一 seam |

### 19.2 Tower Defense P0-5 Gate G

265 施工完成不自动等于 Gate G 通过。获得独立授权后，塔防仍需在 fresh run-owned root 和
generated specialized Editor 验证 1280x720、1600x900、四轮、军略三选一和终局。

必须证明 1280 HUD 不再受隐藏层级语义错误影响，1600 GameView 不再绘制 missing-texture
placeholder，每张截图绑定准确 receipt，264 wait 与 265 publication 以 sessionId/runtimeFrame
对齐，且没有修改 gameplay 结果迁就视觉测试。

## 20. Ownership 分类

| 范围 | Ownership | 当前状态 |
|---|---|---|
| `engine_runtime` AUI/runtime frame contract | engine-owned | 只写方案，未授权修改 |
| `editor_wgpu_renderer` surface/queue/capture | engine-owned | 只写方案，未授权修改 |
| `editor_window_winit` publication consumption | engine-owned | 只写方案，未授权修改 |
| 通用 engine/editor tests | engine-owned | 不修改、不运行 |
| `samples/tower_defense_project` | project-owned | 仅下游验收方，不修改 |
| production binary/真实配置 | external operational state | 未授权 |

具体文件、Gate 和测试命令必须由施工文档基于当时代码重新核对。

## 21. 风险与缓解

1. 旧项目可能错误依赖“隐藏父、显示子”的 bug。把本变化定义为 correctness fix，以层级矩阵和
   第二项目回归验证，不提供 legacy 双重语义。
2. stable surface 可能持有 stale backing。用 generation、deviceIdentity 和 owner-managed
   retirement 保证 consumer 不跨 generation 持有裸资源。
3. receipt 可能被误解为 GPU completion。普通 receipt 明确定义为 queue submission；只有 exact
   capture readback 证明像素可读。
4. 深 Module 可能变成巨型不可测实现。保持小外部 Interface，内部使用 WGPU/test adapter，
   测试只通过 Interface。新 Interface 测试覆盖后按 replace-don't-layer 删除重复浅顺序测试。
5. 剩余重叠可能是真实项目布局或 DPI 问题。265 只证明 effective visibility 和 publication；
   纯布局回项目修复，需要新 DPI seam 时必须单独讨论和授权。

## 22. 方案自审

### 22.1 范围

- [x] 引擎能力项目无关，没有塔防玩法进入 Core。
- [x] 塔防只作为下游真实验收方。
- [x] 未合并 264 Production Wait 与视觉 publication。
- [x] 未扩张到 DPI、布局折叠或多 viewport。
- [x] 本轮未修改代码、项目、production binary 或真实配置。

### 22.2 合同

- [x] local/effective visibility 和 layout participation 分开。
- [x] draw 与所有 interaction consumer 共用 effective visibility。
- [x] 隐藏子树 cleanup 已定义。
- [x] RuntimeContentIdentity 与 PublicationIdentity 分开。
- [x] surfaceId、generation、publicationIndex 推进条件唯一。
- [x] redraw/present/wait/capture 不推进 Runtime。
- [x] 普通帧同 queue order，exact capture 才 readback。
- [x] resize、last-good、stop/restart、device loss 有明确语义。

### 22.3 深 Module 与 AI 适配

- [x] 外部 Interface 小，caller 不承担排序、generation、fallback。
- [x] 删除 Module 会使复杂度扩散，具有真实 depth。
- [x] WGPU 与 deterministic test adapter 构成真实内部 seam。
- [x] typed identity、receipt、diagnostics 和 Summary/Trace 可供 AI 审查与修复。
- [x] Off 不产生常驻长报告或像素历史。

### 22.4 自审结论

方案 C 与 219、260、263、264 的 ownership 和生命周期一致，能够用一个深 Module 消除逐帧
纹理身份、隐藏 tick 和多 caller 顺序分叉，并以 effective visibility 修正 AUI 层级隐藏。
方案未扩大到 DPI、布局折叠或塔防 gameplay，正式方案自审通过。

## 23. 后续流程与授权边界

当前唯一可进行的下一步：

```text
根据 265 生成并自审独立引擎施工文档
```

仍需用户明确要求。施工文档必须重新核对 dirty worktree、代码基线、ownership、唯一施工槽、
分 Gate 测试和真实 Windows 授权窗口。

获得进一步授权前，不修改 Rust 或塔防文件，不创建/激活施工文档，不重跑 Gate G，不进入
Gate H，不运行 Local CI，不替换 production/安装态二进制，不修改真实用户配置。
