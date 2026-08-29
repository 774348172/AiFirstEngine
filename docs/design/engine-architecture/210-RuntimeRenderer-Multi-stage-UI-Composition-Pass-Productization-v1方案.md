# 210-RuntimeRenderer Multi-stage UI Composition Pass Productization v1 方案

## 1. 系统定义

本系统正式命名为：

```text
RuntimeRenderer Multi-stage UI Composition Pass Productization v1
```

一句话：

```text
让运行时 renderer 不再只有一个固定在世界之后的 AUI Overlay pass，
而是能按明确 stage 渲染 AUI Canvas / LayerGroup：
  UI Background / BeforeWorld
  World / Sprite
  UI Foreground / ScreenOverlay
  UI Modal
```

它解决的是 209 留下的真实运行时缺口：

```text
209 已经允许用户在 Scene / Hierarchy 里表达：
  某个 UI 层在场景物体后面
  另一个 UI 层在场景物体前面

但当前 RuntimeRenderer 只有：
  World / Sprite
  -> Draw AUI Overlay
  -> Present

所以 209 只能把跨 World 排序标成 runtime_supported=false。
210 要把 Canvas / LayerGroup 粒度的跨 World 排序变成真实 runtime pass order。
```

注意：

```text
210 不是让 AUI Node 变成 Runtime ECS Entity。
210 不是做独立 AUI Designer。
210 不是做完整 WorldSpace UI / UI Camera / Depth-aware UI。
210 只把已经 layout / binding / projection 后的 AUI draw frame 分 stage 交给 RuntimeRenderer。
```

## 1.1 29 号审查吸收结论

根据：

```text
其它AI审查目录/29-210-RuntimeRenderer-Multi-stage-UI-Composition-Pass方案审查.md
```

本文吸收以下修订，作为后续施工硬规则：

```text
1. BeforeWorld 是新增 AUI composition stage，不等于 AuiCanvasMode::WorldSpace。
2. stage 真相采用 schema-first：新增 AuiCanvas.composition_stage 字段。
3. AuiLayout / AuiDrawList / UiProjection 必须按 canvas composition_stage 分桶。
4. AuiOverlaySortKey 必须真正接入 AuiCanvas.layer / AuiCanvas.sorting_order，不能继续全置 0。
5. editor_ui_model 的 SceneVisualOrderRenderSpace 必须补 BeforeWorld 变体。
6. RuntimeRenderFrameReport 必须输出 stage presence / skip evidence，不能只靠 pass_count 猜。
7. Modal 第一版只代表 rendering composition stage，不承诺 input blocking / focus trap。
```

采用 `AuiCanvas.composition_stage` 的原因：

```text
它随 AUI Document / RuntimePackage 自然流转。
它对用户和 AI 都是显式字段，可审查、可 patch、可 diff。
它避免用 layer / sorting_order 阈值偷藏 stage 语义。
```

## 2. 其它引擎对标

### 2.1 Unity UGUI

Unity UGUI 的成熟心智是：

```text
Canvas / Graphic / RectTransform
  -> CanvasUpdateRegistry delayed rebuild
  -> Graphic mesh / CanvasRenderer
  -> Canvas render mode / sorting order
  -> EventSystem / GraphicRaycaster
```

本项目已扫描源码参考：

```text
框架设计/Unity源码参考/UGUI-Canvas-EventSystem-Render源码参考.md

<UNITY_UI_REFERENCE>\com.unity.ugui
  Runtime/UGUI/UI/Core/CanvasUpdateRegistry.cs
  Runtime/UGUI/UI/Core/Graphic.cs
  Runtime/UGUI/UI/Core/GraphicRaycaster.cs
  Runtime/UGUI/EventSystem/EventSystem.cs
```

可学习点：

```text
Canvas 是 UI 渲染、排序、rebuild、raycast 的重要边界。
Layout / Graphic rebuild / Raycast / Render 是分阶段链路。
复杂 UI 不靠每个子节点和世界物体任意混排。
```

不照搬：

```text
不把 AUI Node 改成 GameObject / Runtime ECS Entity。
不照搬 CanvasRenderer 黑箱。
不让 AUI Document 或 Binding 直接接触 renderer / GPU。
```

联网参考：

```text
Unity UGUI Canvas Manual:
https://docs.unity3d.com/Packages/com.unity.ugui@2.0/manual/UICanvas.html
```

### 2.2 Unity SRP / CommandBuffer

本项目已有源码参考：

```text
框架设计/Unity源码参考/SRP-CommandBuffer-RendererBackend源码参考.md

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master
  Runtime/Export/RenderPipeline/RenderPipeline.cs
  Runtime/Export/RenderPipeline/ScriptableRenderContext.cs
  Runtime/Export/Graphics/RenderingCommandBuffer.cs
```

可学习点：

```text
Renderer 用稳定入口组织一帧渲染。
CommandBuffer / context 是延迟提交，不是项目逻辑直接调用 GPU。
pass 顺序应由 Renderer / RenderGraph 表达，不能散落在项目规则里。
```

### 2.3 Unreal UMG / Slate / RDG / RHI

本项目已有源码参考：

```text
框架设计/UE源码参考/RDG-RHI-RendererBackend源码参考.md

<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release
  Engine/Source/Runtime/RenderCore/Public/RenderGraph.h
  Engine/Source/Runtime/RenderCore/Public/RenderGraphBuilder.h
  Engine/Source/Runtime/RHI/Public/RHICommandList.h
  Engine/Source/Runtime/RHI/Public/DynamicRHI.h
```

可学习点：

```text
Widget / Slate 先生成 DrawElement。
Renderer 再组织 pass / RHI command。
RenderCommand / RDG / RHI 要分层，项目逻辑不碰 GPU command。
```

不照搬：

```text
不把 UE 完整 RDG / RHI 复杂度搬进 C-min。
不让 AI 直接生成 RDG lambda / RHI command。
```

### 2.4 Godot CanvasLayer / RenderingServer

本项目已有源码参考：

```text
框架设计/Godot源码参考/06-RenderingServer-RenderingDevice-DisplayServer源码参考.md

servers/rendering/rendering_server.h
servers/rendering/rendering_device.h
servers/display/display_server.h
```

可学习点：

```text
Canvas / RenderingServer / RenderingDevice 分界清楚。
项目对象不直接持 GPU 对象。
UI 层级和渲染服务之间通过服务边界和 handle 连接。
```

联网参考：

```text
Godot CanvasLayer:
https://docs.godotengine.org/en/stable/classes/class_canvaslayer.html
```

### 2.5 Bevy UI

Bevy UI 的 ZIndex / GlobalZIndex 证明 UI 自身需要稳定的层级排序语义。

联网参考：

```text
Bevy UI ZIndex:
https://docs.rs/bevy_ui/latest/bevy_ui/struct.ZIndex.html
```

本项目只学习：

```text
UI ordering 要结构化、可测试、可报告。
```

不照搬：

```text
不让用户手写 Rust UI tree。
不把 AUI 真相改成 ECS UI entity。
```

## 3. 本项目当前基线

当前正式 AUI runtime 链路：

```text
AUI Document
  -> RuntimePackage
  -> ProjectUiStateSnapshot
  -> Binding Resolve
  -> AuiLayout / AuiDrawList
  -> UiProjection / AuiOverlayFrame
  -> RuntimeRenderer UI Pass
  -> Present
```

当前关键代码事实：

```text
rust/crates/engine_runtime/src/aui.rs
  AuiCanvasMode:
    ScreenOverlay
    ScreenCamera
    WorldSpace

  AuiCanvas:
    canvas_id
    mode
    composition_stage
    layer
    sorting_order

  AuiOverlayFrame:
    frame_index
    draw_items
    report
    glyph_plan

  AuiRuntimePresenter:
    生成 AuiOverlayFrame 和 glyph_plan

rust/crates/engine_runtime/src/runtime_renderer.rs
  RuntimeRendererInput.aui_overlay: Option<&AuiOverlayFrame>
  build_graph 当前只在 present 前插入 draw-aui-overlay

rust/crates/engine_runtime/src/render_graph.rs
  RenderPassKind::DrawUiOverlay
  RenderPassCommand::DrawUiOverlay

rust/crates/engine_runtime/src/rhi_command_plan.rs
  RhiDrawKind::UiOverlay
  RhiDrawPayload::UiOverlay
```

当前 209 authoring 事实：

```text
rust/crates/editor_ui_model/src/aui_scene_authoring.rs
  SceneVisualOrderAuthoringModel
  VisualOrderKey
  VisualOrderIntent
  runtime_supported / deferred_to_runtime_composition_gate

rust/crates/project_e2e_gate/src/aui_scene_authoring.rs
  runtime_composition_gap_count >= 1
  next_required_runtime_gate = RuntimeRenderer Multi-stage UI Composition Pass
```

当前缺口：

```text
RuntimeRenderer 只有一个 ScreenOverlay pass。
AuiOverlayFrame 是扁平 draw item frame。
RenderGraph / RHI report 不能表达 UI stage。
209 跨 World 排序只能 authoring intent，不能真实运行。
复杂打飞机 HUD 无法表达“背景 UI 在世界后，HUD UI 在世界前，弹窗在最前”的 runtime pass evidence。
AuiLayoutEngine::layout 当前跳过非 ScreenOverlay canvas；施工必须避免把 BeforeWorld 误接到 WorldSpace。
AuiRendererBridge::build_overlay_frame 当前把 canvas_layer / canvas_sorting_order 全置 0；施工必须接入真实 canvas 字段。
editor_ui_model::SceneVisualOrderRenderSpace 当前无 BeforeWorld；施工必须补齐 authoring 侧枚举。
```

## 4. 与 102 / 209 的关系

102 的历史结论是：

```text
AuiDrawList
  -> AuiRendererBridge
  -> AuiOverlayFrame
  -> RuntimeRenderer UI Render Pass
```

这个结论仍然保留，但要升级：

```text
AuiOverlayFrame 是 C-min 兼容入口。
AuiCompositionFrame 是新的多 stage frame。
旧 AuiOverlayFrame 默认映射到 ScreenOverlay stage。
```

209 的结论是：

```text
AUI Node 不变成 Runtime ECS Entity。
跨 World 前后夹击的安全粒度是 AUI Canvas / AUI LayerGroup。
单个 AUI Node 不能直接拖到 Scene Entity 前后。
```

210 必须遵守：

```text
只兑现 Canvas / LayerGroup 粒度的 stage composition。
不兑现单个 AUI Node 与 Scene Entity 任意混排。
BeforeWorld 是 screen-space composition stage，不是 WorldSpace UI。
```

## 5. 方案选项

### 5.1 方案 A：继续单 Overlay Pass，只改 Report

做法：

```text
保留当前 draw-aui-overlay。
209 的跨 World intent 继续 runtime_supported=false。
只把 diagnostics 和 next_action 写清楚。
```

优点：

```text
施工最小。
风险最低。
```

缺点：

```text
没有解决 209 的真实运行时缺口。
复杂打飞机 HUD 仍只能整体盖在世界上。
Hierarchy 里表达的 UI 前后关系仍然不能在 Player 中兑现。
```

结论：

```text
不推荐。它只是维持现状。
```

### 5.2 方案 B：完整 WorldSpace / Depth-aware UI Composition

做法：

```text
一次性支持：
  ScreenOverlay
  ScreenCamera
  WorldSpace
  UI Camera
  depth test
  stencil / mask
  per-node world interleave
  input hit-test 与 world picking 全量融合
```

优点：

```text
能力最强。
长期可覆盖 3D 名牌、血条、世界内交互面板等高级 UI。
```

缺点：

```text
范围过大。
会把 renderer、input、layout、camera、depth、mask、hit-test 全部绑在一起。
很容易把 AUI Node 推向 Runtime ECS Entity 心智。
施工和测试成本高，不适合作为 209 的直接后续。
```

结论：

```text
不作为本轮方案。
后续可单列 WorldSpace UI / UI Camera / Depth-aware UI Productization。
```

### 5.3 方案 C-min：Fixed Stage AUI Composition Frame

做法：

```text
新增可审查的 AuiCompositionFrame。
RuntimeRenderer 按固定 stage 插入 UI pass：
  Draw AUI BeforeWorld
  Draw World / Sprite
  Draw AUI ScreenOverlay
  Draw AUI Modal

stage 只服务 AUI Canvas / AUI LayerGroup 粒度。
旧 AuiOverlayFrame 兼容映射到 ScreenOverlay。
RenderGraph / RHI / runtime report 输出 stage evidence。
209 中 Canvas / LayerGroup 级跨 World 排序可从 runtime_supported=false 推进到 runtime_supported=true。
```

优点：

```text
直接解决 209 的最关键运行时缺口。
不新增 UI 真相层，不把 AUI Node 变成 Entity。
AI 可以通过 stage / pass / report 判断问题。
复杂打飞机项目足够使用：背景 UI、世界、HUD、弹窗四段已经覆盖主需求。
施工范围可控。
```

缺点：

```text
不支持完整 WorldSpace UI。
不支持单个 AUI Node 与 Scene Entity 任意穿插。
需要更新 AUI frame / RenderGraph / RHI / 209 report / project_e2e_gate。
```

结论：

```text
推荐采用。
```

## 6. 推荐方案

采用：

```text
方案 C-min：Fixed Stage AUI Composition Frame
```

正式链路：

```text
RuntimePackage AUI documents
  -> AuiRuntimePresenter
  -> AuiDrawList
  -> AuiCompositionFrame
      stage: BeforeWorld | ScreenOverlay | Modal
      boundary: Canvas | LayerGroup
      draw_items
      glyph_plan
      report
  -> RuntimeRenderer
      Clear
      Draw AUI BeforeWorld
      Draw World / Mesh
      Draw Sprite2D
      Draw AUI ScreenOverlay
      Draw AUI Modal
      Present
  -> RenderGraph / RHI Command Plan / RuntimeRenderFrameReport
```

### 6.1 Stage 定义

第一版只支持：

```text
AuiCompositionStage::BeforeWorld
  屏幕空间 UI，绘制在 World / Sprite 前。
  用于背景框、场景后面的 UI 装饰、伪 2D 背板。

AuiCompositionStage::ScreenOverlay
  屏幕空间 UI，绘制在 World / Sprite 后。
  用于 HUD、血条、技能按钮、分数、资源栏。

AuiCompositionStage::Modal
  屏幕空间 UI，绘制在 ScreenOverlay 后。
  用于弹窗、遮罩、暂停菜单、确认框。
```

第一版不把 `World` 当成 AUI stage。World 是 renderer 的中间固定区段。

stage 存储位置：

```text
AuiCanvas.composition_stage: AuiCompositionStage
```

默认值：

```text
AuiCompositionStage::ScreenOverlay
```

兼容规则：

```text
旧 AUI document 没有 composition_stage 时反序列化为 ScreenOverlay。
RuntimePackage / cooker 不手写 stage 映射；字段随 AuiDocument 自然流转。
BeforeWorld 不使用 AuiCanvasMode::WorldSpace 表达。
WorldSpace / ScreenCamera 仍 deferred。
```

### 6.2 视觉排序规则

支持：

```text
AUI Canvas / AUI LayerGroup 在 BeforeWorld / ScreenOverlay / Modal stage 之间移动。
同一 stage 内按 layer / sorting_order / tree_order 排序。
同一 Canvas / LayerGroup 内 AUI Node 按 tree_order 排序。
```

排序接线规则：

```text
AuiOverlaySortKey.canvas_layer 必须来自 AuiCanvas.layer。
AuiOverlaySortKey.canvas_sorting_order 必须来自 AuiCanvas.sorting_order。
AuiOverlaySortKey.tree_order 保持同 canvas / subtree 内稳定遍历顺序。
composition builder 必须按 stage -> layer -> sorting_order -> tree_order 输出稳定顺序。
```

拒绝：

```text
单个 AUI Node 直接移动到 Scene Entity 前后。
单个 AUI Node 跨 World stage。
任意 AUI Node 与任意 Scene Entity 逐对象混排。
```

提示：

```text
如果用户想让一个节点在世界后、另一个节点在世界前，
必须先显式 Extract To AUI LayerGroup / Canvas，
再移动对应 LayerGroup / Canvas 的 composition stage。
```

示例：

```text
Scene
  bg-ui-layer                         [AUI LayerGroup, stage=BeforeWorld]
    bg-frame                          [AUI Node]
  World
    PlayerShip                        [Scene Entity]
    Enemy                             [Scene Entity]
  main-hud                            [AUI Canvas, stage=ScreenOverlay]
    hp-bar                            [AUI Node]
    skill-buttons                     [AUI Node]
  pause-modal                         [AUI Canvas, stage=Modal]
    mask                              [AUI Node]
    panel                             [AUI Node]
```

### 6.3 数据结构建议

新增运行时 frame 数据，不新增运行时架构层：

```text
AuiCompositionFrame
  frame_index
  stages: Vec<AuiCompositionStageFrame>
  report: AuiCompositionReport
  glyph_plan: Option<AuiTextGlyphPlan>

AuiCompositionStageFrame
  stage: AuiCompositionStage
  draw_items: Vec<AuiOverlayDrawItem>
  item_count
  text_count
  image_count
  glyph_count
  canvas_count
  layer_group_count
  debug_label

AuiCompositionStage
  BeforeWorld
  ScreenOverlay
  Modal

AuiCompositionReport
  schema_version
  frame_index
  stage_count
  before_world_item_count
  screen_overlay_item_count
  modal_item_count
  unsupported_stage_count
  rejected_node_interleave_count
  glyph_present
  diagnostics
```

AuiCanvas schema：

```text
AuiCanvas
  canvas_id
  mode
  composition_stage: AuiCompositionStage
  layer
  sorting_order
  reference_resolution
  scale_mode
  root_node
```

layout / extract 规则：

```text
AuiLayoutEngine 仍只支持 screen-space layout。
mode == ScreenOverlay 的 canvas 可进入 BeforeWorld / ScreenOverlay / Modal stage。
mode == ScreenCamera 或 WorldSpace 的 canvas 仍不进入 C-min composition，并输出 deferred diagnostic。
不能因为 BeforeWorld 叫 world 前，就把 WorldSpace canvas layout 进来。
```

兼容规则：

```text
AuiOverlayFrame 暂时保留。
AuiOverlayFrame -> AuiCompositionFrame 的默认映射是 ScreenOverlay。
RuntimeRenderer 可先同时接受 aui_overlay / aui_composition，施工完成后再逐步收敛命名。
```

### 6.4 RenderGraph / RHI 表达

RenderGraph 新增或扩展：

```text
RenderPassKind::DrawUiComposition
RenderPassCommand::DrawUiComposition
  target
  stage
  item_count
  text_count
  image_count
  glyph_count
  font_atlas_id
  text_pass_inserted
  debug_label
```

RHI 新增或扩展：

```text
RhiDrawKind::UiComposition
RhiDrawPayload::UiComposition
  stage
  item_count
  text_count
  image_count
  glyph_count
  font_atlas_id
  pipeline_key
```

兼容规则：

```text
旧 DrawUiOverlay 可保留一个施工周期。
旧 DrawUiOverlay 等价于 DrawUiComposition(stage=ScreenOverlay)。
测试必须覆盖旧路径兼容和新路径 stage pass order。
```

### 6.5 RuntimeRenderer Pass 顺序

固定顺序：

```text
clear-main
draw-aui-before-world              optional
draw-* mesh/world                  existing
draw-sprite2d-*                    existing
draw-aui-screen-overlay            optional
draw-aui-modal                     optional
present-main
```

规则：

```text
RuntimeRenderer 不读取 AuiDocument。
RuntimeRenderer 不读取 Binding path。
RuntimeRenderer 不读取 ProjectUiStateSnapshot。
RuntimeRenderer 只读取 AuiCompositionFrame / AuiOverlayFrame。
RuntimeRenderer 只决定 pass order，不修改 AUI draw item。
```

RuntimeRenderFrameReport 必须新增 stage evidence：

```text
ui_composition_stage_count
ui_before_world_item_count
ui_screen_overlay_item_count
ui_modal_item_count
ui_before_world_pass_present
ui_screen_overlay_pass_present
ui_modal_pass_present
ui_before_world_skipped
ui_screen_overlay_skipped
ui_modal_skipped
```

空 stage 仍不生成 draw pass，但必须在 report 中说明 skipped。

### 6.6 209 Report 收敛

210 完成后，209 的 report 需要从：

```text
visual_order_runtime_supported=false
deferred_to_runtime_composition_gate=true
```

升级为：

```text
Canvas / LayerGroup stage ordering:
  visual_order_runtime_supported=true
  deferred_to_runtime_composition_gate=false

Single AUI Node cross World:
  rejected
  diagnostics += extract_to_aui_layer_group_or_canvas
```

仍然 deferred：

```text
WorldSpace UI
ScreenCamera UI
Depth-aware UI
UI Camera
per-node world interleave
```

### 6.7 对复杂打飞机项目的意义

复杂打飞机最小可用 UI composition：

```text
BeforeWorld:
  背景 UI 装饰
  关卡边框
  不参与世界深度的背板

World / Sprite:
  飞机
  子弹
  敌人
  爆炸 sprite

ScreenOverlay:
  血条
  分数
  能量条
  技能按钮
  当前武器

Modal:
  暂停菜单
  胜利 / 失败弹窗
  设置面板
```

这已经能让“导出后屏幕上真的有 HUD”进一步变成：

```text
导出后 HUD 与世界有可验证的前后合成关系。
```

## 7. 本阶段不做

明确不做：

```text
AUI Node 变 Runtime ECS Entity。
独立 AUI Designer。
完整 WorldSpace UI。
UI Camera。
Depth test / stencil / mask 全套。
任意 AUI Node 与 Scene Entity 逐对象混排。
ScrollView / InputField / IME。
完整 CJK shaping / rich text。
真实 GPU UI batching 优化。
复杂透明排序。
编辑器拖拽创建控件。
打飞机专用 HUD API。
项目逻辑直接生成 RenderGraph / RHI command。
```

## 8. 可施工 Gate

### Gate A：AUI Composition Frame Schema

目标：

```text
新增 AuiCompositionStage / AuiCompositionFrame / AuiCompositionStageFrame / AuiCompositionReport。
新增 AuiCanvas.composition_stage，默认 ScreenOverlay。
明确 BeforeWorld 是 screen-space composition stage，不等于 WorldSpace。
AuiOverlayFrame 可兼容映射到 ScreenOverlay stage。
AuiLayout / composition builder 按 canvas composition_stage 分桶。
AuiOverlaySortKey 接入 canvas.layer / canvas.sorting_order。
现有 AuiRuntimePresenter glyph_plan 不回退。
```

测试：

```powershell
cargo test -p engine_runtime aui
```

### Gate B：RenderGraph / RHI UI Composition Command

目标：

```text
RenderGraph 能表达 DrawUiComposition(stage)。
RHI command plan 能保留 stage payload。
旧 DrawUiOverlay 兼容为 ScreenOverlay。
```

测试：

```powershell
cargo test -p engine_runtime render_graph
cargo test -p engine_runtime rhi_command_plan
```

### Gate C：RuntimeRenderer Multi-stage Pass Order

目标：

```text
RuntimeRenderer 按 BeforeWorld -> World/Sprite -> ScreenOverlay -> Modal -> Present 生成 pass。
空 stage 不生成空 pass。
RuntimeRenderFrameReport 输出 ui_composition_stage_count / stage item counts。
RuntimeRenderFrameReport 输出 stage pass present / skipped evidence。
```

测试：

```powershell
cargo test -p engine_runtime runtime_renderer
```

### Gate D：RenderThread / EngineHostLoop / Player Forwarding

目标：

```text
RenderThreadFrameInput / RenderFramePacket / EngineFrameInput 支持 AuiCompositionFrame。
runtime_player_winit 可把 presenter 输出传到 RuntimeRenderer。
旧 aui_overlay 路径继续通过兼容映射工作。
```

测试：

```powershell
cargo test -p engine_runtime render_thread
cargo test -p runtime_player_winit aui
```

### Gate E：209 Authoring Report Runtime Gap 收敛

目标：

```text
Canvas / LayerGroup 级 BeforeWorld / ScreenOverlay / Modal stage ordering 标记 runtime_supported=true。
SceneVisualOrderRenderSpace 新增 BeforeWorld 变体和 VisualOrderKey::before_world 构造器。
单 AUI Node 跨 World 仍 rejected。
AuthoringAiContext / project_e2e_gate runtime_composition_gap_count 根据真实能力更新。
```

测试：

```powershell
cargo test -p editor_ui_model aui_scene
cargo test -p editor_core aui_scene
cargo test -p project_e2e_gate aui_scene
```

### Gate F：复杂打飞机 E2E Evidence

目标：

```text
complex shooter e2e report 能证明：
  before_world pass 存在或被明确跳过
  screen_overlay pass 存在
  modal pass 可被样例触发或以 empty optional 说明
  pass order 正确
  glyph_present 不回退
```

测试：

```powershell
cargo test -p project_e2e_gate
```

### Gate G：整体回归与文档同步

目标：

```text
同步 49 / 54 / 施工文档 README / 阶段完成记录 README。
如果 102 中单 overlay 描述仍被引用，要标注它是兼容旧路径。
```

测试：

```powershell
cargo fmt --check
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p project_e2e_gate
```

## 9. 验收标准

必须证明：

```text
RenderGraph pass 顺序可审查。
RHI command payload 保留 UI stage。
AuiOverlayFrame 旧路径不坏。
AuiCompositionFrame 新路径可表达 BeforeWorld / ScreenOverlay / Modal。
BeforeWorld 有真实 draw item 分桶测试，不是空 pass 伪造。
AuiOverlaySortKey 真实反映 canvas.layer / canvas.sorting_order。
RuntimeRenderer 不读取 AuiDocument / Binding / ProjectUiStateSnapshot。
AUI Node 未变成 Runtime ECS Entity。
209 Canvas / LayerGroup 级 runtime gap 被收敛。
单个 AUI Node 跨 World 仍被拒绝。
complex shooter e2e 能输出 UI composition pass evidence。
glyph_present / font_atlas_id / text_pass_inserted 不回退。
```

允许保留：

```text
WorldSpace UI deferred。
UI Camera deferred。
Depth-aware UI deferred。
复杂 mask / stencil deferred。
ScrollView / InputField deferred。
真实 GPU batching 优化 deferred。
```

不允许：

```text
用 debug overlay 假装 AUI pass。
用单 overlay pass 假装支持 BeforeWorld。
用 WorldSpace canvas 假装支持 BeforeWorld。
用 layer / sorting_order 阈值偷藏 stage 语义。
让 Scene Entity 和 AUI Node 直接进入同一个排序数组并逐对象混排。
让 RuntimeRenderer 回读 AUI Document。
让 Project Rule 或 IR 直接生成 RenderGraph / RHI command。
为了测试通过伪造 stage count。
```

## 10. 方案自审

### 是否解决 209 最关键问题

解决。

```text
209 中“一个 UI 层在场景后，一个 UI 层在场景前”的需求，
在 210 中通过 Canvas / LayerGroup stage composition 兑现。
```

### 是否层数太多

没有。

```text
AuiCompositionFrame 是一帧渲染数据，不是新的运行时系统层。
它不保存工程真相，不进入 AUI Document，不成为 ECS。
它只是替代“单 AuiOverlayFrame 扁平列表”的更真实 frame payload。
```

### 是否符合 AI-first

符合。

```text
stage、pass order、item count、glyph evidence、diagnostics 都结构化。
AI 可以根据 report 判断：
  是 AUI 文档问题
  是 stage 选择问题
  是 RuntimeRenderer pass order 问题
  是 RHI payload 问题
```

### 是否支撑复杂打飞机

支撑。

```text
复杂打飞机需要的背景 UI、世界 sprite、HUD、弹窗四段已经覆盖。
不需要本轮上完整 WorldSpace UI。
```

### 主要风险

风险一：

```text
把 stage composition 误解成任意 node/entity 混排。
```

处理：

```text
文档、report、测试都断言只支持 Canvas / LayerGroup 粒度。
```

风险二：

```text
AuiOverlayFrame 和 AuiCompositionFrame 兼容期命名混乱。
```

处理：

```text
施工文档必须明确迁移顺序：
  先兼容映射
  再改 RuntimeRenderer
  最后更新 report 和入口文档
```

风险三：

```text
Modal 只是更高 overlay，第一版没有真实输入焦点 / 阻挡。
```

处理：

```text
210 只负责 rendering composition。
Modal input blocking / focus trap 后续归 AUI Interaction / Input System 单列。
```

风险四：

```text
只新增 pass，但 layout/extract 没有按 stage 分桶，导致 BeforeWorld 永远空。
```

处理：

```text
Gate A 必须新增 composition builder 测试，断言 BeforeWorld / ScreenOverlay / Modal 都能产生 draw item。
```

风险五：

```text
canvas.layer / sorting_order 字段存在但未接线，导致 stage 内排序仍只靠 tree_order。
```

处理：

```text
Gate A 必须测试 sort_key 来自 canvas.layer / sorting_order。
```

## 11. 最终结论

正式采用：

```text
RuntimeRenderer Multi-stage UI Composition Pass Productization v1
方案 C-min：Fixed Stage AUI Composition Frame
```

下一步：

```text
如果用户确认该方案，
按本文生成自动化施工文档并自审，
再进入施工与测试。
```

施工优先级：

```text
1. AuiCompositionFrame schema 和旧 AuiOverlayFrame 兼容映射。
2. AuiCanvas.composition_stage、layout/extract 分桶、sort_key 接线。
3. RenderGraph / RHI stage command。
4. RuntimeRenderer pass order 与 stage evidence。
5. RenderThread / runtime_player_winit 转发。
6. 209 report / project_e2e_gate runtime gap 收敛。
```
