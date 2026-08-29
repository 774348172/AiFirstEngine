# 190-AUI RuntimePackage Document Hydration / Binding / Present v1 方案

## 1. 系统定义

本系统正式命名为：

```text
AUI RuntimePackage Document Hydration / Binding / Present v1
```

用户已选择：

```text
C-min 推荐方案：AUI Document Cook + Runtime Load + Binding + UiProjection Present
```

它解决的是 M12 中最关键的一段断链：

```text
RuntimePackage 里有 AUI manifest
  但 Runtime 没有真正加载 AUI document body
  也没有把 AUI document resolve / layout / draw list / overlay
  送进 RuntimeRenderer UI Pass
```

本方案不是 209 Scene Unified AUI Authoring，也不是完整 M12。它只把 “导出包里有 HUD 文件” 推进到：

```text
导出 RuntimePackage 内有 cooked AUI document
RuntimePackage loader 能加载 AUI document
Runtime 能 resolve binding 并生成 AuiOverlayFrame
RuntimeRenderer 能执行 DrawUiOverlay pass
Player report 能证明 HUD 进入 present 链路
```

## 2. 归属规则

本系统属于：

```text
185-M12 AUI HUD Authoring / Binding / Runtime Present v1
189-Project RuntimePackage Assembly Completeness v1
110-World Projection Adapter 统一跨域同步规则
```

严格归属：

```text
AUI authoring source -> RuntimePackage cooked AUI document = Build / Package stage
RuntimePackage cooked AUI document -> AuiDocument = RuntimePackage load stage
AuiDocument + ProjectUiStateSnapshot -> resolved AuiDocument = AUI Binding stage
resolved AuiDocument -> AuiLayout / AuiDrawList = AUI runtime layout stage
AuiDrawList -> AuiOverlayFrame = UiProjection stage
AuiOverlayFrame -> RuntimeRenderer UI Pass = Present stage
```

这里的 `Document Hydration` 是 AUI document 从 RuntimePackage 数据进入 Runtime AUI domain 的加载/归一化过程。它不是 ECS World Hydration，也不是新的 Bridge。

必须按 `110` 统一术语理解：

```text
AUI Render Extract / AuiRendererBridge = UiProjection
```

禁止新增：

```text
AuiPackageBridge
AuiRuntimeBridge
AuiPresentBridge
AuiDocumentBridge
```

如果代码中保留 `AuiRendererBridge` 历史类型名，文档和新逻辑必须解释为 `UiProjection` 的历史落地。

## 3. 当前真实基线

当前工作区已经具备的基础：

```text
engine_runtime::aui::AuiDocument
engine_runtime::aui::ProjectUiStateSnapshot
engine_runtime::aui::AuiBindingRef target_field / path / fallback
engine_runtime::aui::AuiAction
engine_runtime::aui::AuiNodeKind::ProgressBar
engine_runtime::aui::AuiRuntimeResolver::resolve_bindings
engine_runtime::aui::AuiLayoutEngine
engine_runtime::aui::AuiDrawList
engine_runtime::aui::AuiOverlayFrame
RuntimeRendererInput.aui_overlay
RenderPassKind::DrawUiOverlay
editor_core::AuiAuthoringService
editor_core::ProjectRuntimePackageAssembler
RuntimeAuiManifest
```

当前仍存在的断点：

```text
RuntimePackageBuilder 写出 aui/aui-manifest.json，但不写 AUI document body。
RuntimePackage loader 只加载 RuntimeAuiManifest，不加载 AuiDocument 列表。
RuntimeAuiManifestEntry.path 当前可能指向 authoring source path，例如 AUI/hud.aui.json，而不是 package 内 cooked document path。
复杂打飞机导出包当前只有 aui/aui-manifest.json，没有 aui/documents/hud-main.aui.json。
样例 hud.aui.json 当前是 authoring shape：root / nodeType / children，不是 runtime AuiDocument shape：canvases / nodes / binding_refs / action_refs。
RenderThread 当前向 RuntimeRenderer 传 aui_overlay: None。
RuntimeRenderer 已能插入 DrawUiOverlay pass，但主链路没有给它真实 AuiOverlayFrame。
```

因此本方案的目标不是发明 AUI，而是打通：

```text
authoring AUI file
  -> cooked runtime AuiDocument in RuntimePackage
  -> loaded Runtime AUI document
  -> resolved / layout / overlay
  -> RuntimeRenderer UI pass
```

## 4. 其它引擎参考结论

### Unity

Unity 的 UI / Sprite / Texture / Font 用户层保持简单，GPU 资源、字体 atlas、渲染绑定由 engine 内部处理。

本项目采用：

```text
AUI Document / AssetRef / Binding path 面向项目层。
字体 atlas / GPU texture / RHI command 不暴露给项目规则或 AI 默认读写。
```

不采用：

```text
Unity4.3 IMGUI immediate mode。
Canvas rebuild 黑盒。
MonoBehaviour 字段直接作为 UI binding 真相。
```

### Unreal Engine

UE 的 UMG / Slate 重点是：

```text
Widget tree
  -> Binding / Slate Draw Elements
  -> UI renderer
```

资源、字体和图片最终仍走 RenderResource / RHI 纪律。

本项目采用：

```text
AUI Tree / Binding / DrawList / UI Pass 分层。
UI font / image 最终仍走统一 RenderAsset / RHI 资源纪律。
```

不采用：

```text
完整 Slate / UMG 重体系。
Blueprint binding 黑箱。
```

### Godot

Godot Control / CanvasItem 证明 UI 可以是简单节点树，并通过统一 canvas renderer present。

本项目采用：

```text
ScreenOverlay Canvas + 简单 AUI node tree。
Canvas / Text / Image / Panel / Button / ProgressBar 作为第一版节点心智。
```

不采用：

```text
Godot Node / Signal 作为 Runtime 真相。
RID / RenderingServer API 暴露给项目层。
```

### Bevy

Bevy UI 的价值是：

```text
数据驱动 UI
layout / render extract 分离
headless 可测
```

本项目采用：

```text
AuiDocument -> Binding -> Layout -> DrawList -> UiProjection -> RuntimeRenderer
```

不采用：

```text
把 UI 直接做成 Bevy ECS entity 体系。
把 schedule/render world 复杂度暴露给项目或 AI。
```

## 5. C-min 推荐链路

本方案采用完整边界的最小真实路径：

```text
ProjectRuntimePackageAssembler
  -> collect AUI authoring documents
  -> AuiDocumentCooker
  -> RuntimePackageBuildInput.aui_documents
  -> RuntimePackageBuilder writes aui/documents/*.aui.json
  -> RuntimePackageBuilder writes aui/aui-manifest.json
  -> load_runtime_package loads RuntimeAuiManifest
  -> load_runtime_package loads cooked AuiDocument bodies
  -> Runtime AUI document registry
  -> ProjectUiStateSnapshot provider
  -> AuiRuntimeResolver::resolve_bindings
  -> AuiLayoutEngine
  -> AuiDrawList
  -> UiProjection / AuiOverlayFrame
  -> RenderFramePacket carries aui_overlay
  -> RuntimeRendererInput.aui_overlay
  -> DrawUiOverlay pass
  -> Present
```

第一版允许实现范围小，但每一步都必须是真实路径，不允许用 debug overlay、日志或源目录扫描替代。

## 6. RuntimePackage 规则

### 6.1 包内目录规则

RuntimePackage 中 AUI 文件必须采用包内路径：

```text
runtime_package/
  manifest.json
  aui/
    aui-manifest.json
    documents/
      hud-main.aui.json
```

禁止 Runtime 读取：

```text
samples/complex_shooter_project/AUI/hud.aui.json
ProjectRoot/AUI/*.aui.json
编辑器内存 Project Object
```

Runtime 只读取 RuntimePackage 内文件。

### 6.2 manifest path 规则

`RuntimeAuiManifestEntry.path` 必须指向 package-relative cooked document path：

```text
aui/documents/hud-main.aui.json
```

不应继续指向 source authoring path：

```text
AUI/hud.aui.json
```

如果需要保留 source path，只能作为 debug/sourceMap/report 字段，不能成为 Runtime 加载依赖。

### 6.3 document id 规则

每个 cooked AUI document 必须有稳定：

```text
document_id
schema_version
canvas_count
node_count
binding_count
action_count
asset_refs
content_hash
```

`document_id` 为空必须是 build error。

多个 document 同 id 必须是 build error，不能由 Runtime 后覆盖。

## 7. AUI Document Cook 规则

### 7.1 输入来源

输入只来自：

```text
ProjectRuntimePackageAssembler::assemble(project_root, build_profile)
  -> AUI/*.aui.json
```

Assembler 是项目目录进入 RuntimePackageBuildInput 的唯一入口。

禁止在 `desktop_export.rs` 中新增 AUI 专用扫描逻辑。

### 7.2 输出真相

Cook 后的输出必须是 runtime `AuiDocument` shape：

```text
AuiDocument
  document_id
  version / schema_version
  canvases
  nodes
  metadata
```

节点必须是：

```text
AuiNode
  node_id
  kind
  parent
  children
  rect
  visible
  interactable
  consume_input
  style
  text
  image
  progress_value
  binding_refs
  action_refs
```

Cooker 不输出 authoring-only `root/nodeType/children` 形态。

### 7.3 authoring shape 兼容规则

C-min 支持两种输入：

```text
canonical runtime-compatible AuiDocument shape
legacy/simple authoring shape: root / nodeType / children
```

兼容 legacy/simple authoring shape 只用于迁移当前样例项目，必须写入 cook report：

```text
source_shape = legacy_authoring_tree
normalized_to = runtime_aui_document
diagnostic_level = warning
```

新增项目和 Editor Authoring Service 保存的文档，应默认输出 canonical runtime-compatible shape。

### 7.4 node 映射规则

legacy/simple authoring shape 的最小映射：

```text
nodeType: canvas -> AuiCanvas + root Panel node
nodeType: text -> AuiNodeKind::Text
nodeType: image -> AuiNodeKind::Image
nodeType: image-row -> C-min 允许映射成多个 Image node 或一个 Image node + warning
nodeType: panel -> AuiNodeKind::Panel
nodeType: button -> AuiNodeKind::Button
nodeType: progress-bar -> AuiNodeKind::ProgressBar
```

未知 nodeType：

```text
build warning if skipped with fallback allowed
build error if it is required for HUD present
```

C-min 不为复杂控件新增 DrawCommand。Button / ProgressBar 仍用 Rect / Text / Image 组合绘制。

### 7.5 AssetRef 规则

AUI document 中引用的 image/font/texture asset 必须进入：

```text
RuntimePackageBuildInput.assets
RuntimeAssetIndex
RuntimeAuiManifestEntry.asset_refs
```

缺失 AssetRef 必须定位到：

```text
document_id
node_id
field
asset_id
source_path
stage
```

禁止只记录 “AUI asset missing” 这种不可定位错误。

## 8. Runtime Load / Document Hydration 规则

### 8.1 RuntimePackage 类型规则

`RuntimePackage` 必须能持有已加载 AUI documents。

建议结构：

```text
RuntimePackage
  aui_manifest: RuntimeAuiManifest
  aui_documents: RuntimeAuiDocumentRegistry

RuntimeAuiDocumentRegistry
  documents_by_id: BTreeMap<String, AuiDocument>
  load_report: RuntimeAuiDocumentLoadReport
```

具体代码结构可按 crate 现状调整，但必须满足：

```text
按 document_id 查找 document
能列出已加载 / 失败 / skipped document
load report 可序列化
```

### 8.2 loader 规则

`load_runtime_package` 加载 AUI 的顺序：

```text
read manifest.json
read aui/aui-manifest.json
validate RuntimeAuiManifest
for each document entry:
  read package_dir / entry.path
  deserialize AuiDocument
  validate document id / node tree / asset refs / binding refs
  insert into registry
```

如果 manifest 声明 document 但 body 缺失：

```text
RuntimeLoadResult failed
diagnostic path = aui.documents[index].path
diagnostic code = AuiDocumentMissing
```

Runtime 不允许因为 AUI document 缺失而静默使用空 HUD，除非 BuildProfile 明确把 AUI 标记为 optional，并且 RuntimeReport 写 warning。

### 8.3 validation 规则

Runtime 加载时至少验证：

```text
schema_version 合法
document_id 非空
document_id 与 manifest entry 匹配
canvas root_node 存在
node_id 唯一
parent / children 引用合法
binding target 与 node kind 兼容
action node 必须 interactable 或明确 warning
image asset_ref 能在 RuntimeAssetIndex 中 resolve
```

Runtime 只做最低限度校验。完整 authoring 错误应在 Build 阶段阻断。

## 9. Binding 规则

### 9.1 Binding 输入

Binding 输入必须是：

```text
AuiDocument
ProjectUiStateSnapshot
```

Binding 不读取：

```text
ECS World
Project Rule internals
RuntimePackage source files
Renderer state
```

### 9.2 ProjectUiStateSnapshot provider

C-min 引入明确的 snapshot provider 边界：

```text
ProjectUiStateSnapshotProvider
  -> ProjectUiStateSnapshot
```

第一版允许 provider 是：

```text
empty_default_snapshot
package_smoke_snapshot
test_snapshot
project_rule_snapshot
```

第一版默认优先采用：

```text
package_smoke_snapshot
```

原因：

```text
empty_default_snapshot 只能验证 fallback / missing path 路径，不能证明正常 binding resolve。
package_smoke_snapshot 可以预填 game.score_text / player.hp_ratio / game.paused 等通用测试值，
用于证明 AUI package/load/binding/layout/present 主链路真实可运行。
```

但 report 必须写明：

```text
snapshot_source
frame_index
value_count
```

如果使用 `empty_default_snapshot` 或 `test_snapshot`，不能声称 Project Rule -> AUI state 已完整接通。

### 9.3 resolve 规则

Binding resolve 必须发生在 layout / draw list 之前：

```text
AuiDocument
  -> AuiRuntimeResolver::resolve_bindings(ProjectUiStateSnapshot)
  -> resolved AuiDocument
  -> AuiLayoutEngine
```

缺 path：

```text
有 fallback -> 使用 fallback，并记录 warning
无 fallback -> 记录 error，document 可继续 layout，但 present report 必须 failed 或 partial
```

类型不匹配：

```text
记录 error
不做隐式转换
不猜项目语义
```

### 9.4 Binding target 规则

C-min 允许 target：

```text
Text.text
ProgressBar.value
Panel.visible
Image.visible
Image.asset_ref
```

不做：

```text
style binding
layout binding
font binding
animation binding
collection/list binding
```

## 10. UiProjection / Present 规则

### 10.1 生成 overlay 的位置

AUI overlay 必须在 Runtime frame/render submit 之前生成：

```text
RuntimePackage loaded AuiDocument
ProjectUiStateSnapshot
  -> resolved document
  -> layout
  -> draw list
  -> UiProjection / AuiOverlayFrame
  -> RenderFramePacket
```

RenderThread 不负责加载 AUI document，不负责 binding resolve，不负责访问 ProjectUiStateSnapshot。

RenderThread 只消费：

```text
RenderFramePacket.aui_overlay
```

然后传给：

```text
RuntimeRendererInput.aui_overlay
```

### 10.2 RuntimeRenderer 规则

RuntimeRenderer 只读取 `AuiOverlayFrame`。

禁止 RuntimeRenderer 读取：

```text
AuiDocument
binding path
ProjectUiStateSnapshot
Project Rule
source project AUI file
```

`DrawUiOverlay` pass 必须插入：

```text
World / Sprite draw passes 之后
Present 之前
```

如果没有 AUI overlay 或 overlay draw_items 为空，RuntimeRenderer 可以不创建 DrawUiOverlay pass，但 report 必须能说明原因：

```text
no_aui_documents
no_visible_nodes
binding_failed
layout_failed
draw_list_empty
```

### 10.3 字体 / glyph 规则

C-min 必须诚实区分：

```text
UI pass present
Text command present
real glyph present
```

如果当前 renderer path 只能统计 `text_count`，不能声明 “真实文字已显示”。

Full M12 / exported visual gate 要求：

```text
HUD Text 必须有真实 glyph evidence。
```

本方案 C-min 的完成报告必须包含：

```text
text_command_count
glyph_present: true | false
glyph_evidence
```

如果 `glyph_present=false`，系统最多通过 `AUI package/load/binding/ui-pass C-min`，不能通过 “visible HUD text” 验收。

## 11. Report / Trace 规则

本系统必须提供结构化报告，至少包含：

```text
AuiDocumentCookReport
RuntimeAuiDocumentLoadReport
AuiBindingReport
AuiRuntimePresentReport
```

### 11.1 AuiDocumentCookReport

字段：

```text
schema_version
status
source_path
package_path
document_id
source_shape
canvas_count
node_count
binding_count
action_count
asset_refs
diagnostics
```

### 11.2 RuntimeAuiDocumentLoadReport

字段：

```text
schema_version
status
package_path
manifest_document_count
loaded_document_count
failed_document_count
documents
diagnostics
```

每个 document：

```text
document_id
path
status
node_count
binding_count
action_count
asset_refs
```

### 11.3 AuiRuntimePresentReport

字段：

```text
schema_version
status
frame_index
document_id
snapshot_source
snapshot_value_count
binding_status
layout_status
draw_item_count
text_command_count
image_command_count
glyph_present
ui_pass_inserted
diagnostics
```

### 11.4 Report 归属

报告必须进入：

```text
BuildReport AUI section
RuntimePackage load diagnostics
RuntimeFrameReport / WindowedPlayer report AUI section
project_e2e_gate report AUI summary
```

AI 默认读这些报告，不读 GPU buffer、font atlas page、RHI command。

## 12. Complex Shooter 验收规则

复杂打飞机样例必须证明：

```text
samples/complex_shooter_project/AUI/hud.aui.json
  -> ProjectRuntimePackageAssembler
  -> cooked runtime AuiDocument
  -> runtime_package/aui/documents/hud-main.aui.json
  -> RuntimePackage load
  -> AuiDocument registry
  -> AuiOverlayFrame
  -> RuntimeRenderer DrawUiOverlay pass
  -> player report AUI present summary
```

验收不能只检查：

```text
AUI 文件数量
aui-manifest.json 存在
text_count 日志
debug overlay
```

最低验收必须检查：

```text
RuntimePackage 内存在 cooked AUI document body
manifest entry path 指向 package 内 cooked document
load_runtime_package 能读到 document registry
present report draw_item_count > 0
RuntimeRenderer report 包含 DrawUiOverlay pass
```

如果 text glyph 尚未真实渲染：

```text
report.status = partial
blocking_gap = runtime_text_glyph_present
```

不能把 partial 当作完整 M12 通过。

## 13. 第一版 C-min 范围

必须做：

```text
AuiDocumentCooker
ProjectRuntimePackageAssembler 接入 AUI document cook
RuntimePackageBuildInput 携带 cooked AUI documents 或等价结构
RuntimePackageBuilder 写出 aui/documents/*.aui.json
RuntimeAuiManifestEntry.path 改为 package-relative cooked path
load_runtime_package 加载 AuiDocument bodies
RuntimePackage 持有 AUI document registry
AUI binding -> layout -> draw list -> overlay frame 的 runtime presenter
RenderFramePacket 携带 aui_overlay
RenderThread 将 packet.aui_overlay 传给 RuntimeRendererInput
RuntimeRenderer / RuntimeFrameReport / Player report 输出 AUI present evidence
project_e2e_gate 检查 complex shooter AUI package/load/present 链路
```

### 13.1 代码落点规则

`AuiDocumentCooker` 属于 Editor / Build 侧能力，推荐落点：

```text
rust/crates/editor_core/src/aui_document_cooker.rs
```

允许在施工中选择放入 `editor_core/src/aui_authoring.rs` 附近，但必须满足：

```text
ProjectRuntimePackageAssembler 调用 AuiDocumentCooker。
AuiDocumentCooker 可以使用 engine_runtime::aui 的数据类型。
AuiDocumentCooker 不放进 RuntimePackage loader。
AuiDocumentCooker 不让 runtime 扫描项目源目录。
desktop_export.rs 不新增 AUI 专用扫描逻辑。
```

`RuntimeAuiDocumentRegistry` 和 `RuntimeAuiDocumentLoadReport` 属于 RuntimePackage load 侧能力，推荐落点：

```text
rust/crates/engine_runtime/src/runtime_package.rs
```

如果施工中为了文件体量拆分模块，可以放入 engine_runtime 的 runtime_package/aui 子模块或相邻模块，但必须满足：

```text
RuntimePackage 拥有 AUI document registry。
registry 按 document_id 查找 AuiDocument。
load report 可序列化并进入 RuntimePackage load diagnostics / RuntimeFrameReport。
engine_runtime 不依赖 editor_core。
```

`AuiRuntimePresenter` / `AuiRuntimePresentReport` 如需新增，属于 engine_runtime AUI / runtime frame 侧能力。它负责：

```text
AuiDocument + ProjectUiStateSnapshot
  -> resolved document
  -> layout
  -> draw list
  -> AuiOverlayFrame
  -> present report
```

它不属于 RenderThread。RenderThread 只转发 `RenderFramePacket.aui_overlay`。

允许 C-min 暂不做：

```text
完整 Native Editor AUI panel
209 Scene Unified AUI Authoring
WorldSpace UI
ScrollView / InputField 产品化
Project Rule 完整 UI State 编辑器
真实多字体排版
复杂 font atlas 管理
UI animation
Mask / Clip
```

不允许 C-min 做：

```text
Runtime 扫描项目源目录读取 AUI。
用 debug overlay 假装 HUD。
RuntimeRenderer 直接读 AuiDocument 或 binding path。
RenderThread 执行 binding resolve。
把 Player / Score / Health / Bullet 等玩法概念写入 AUI 引擎 API。
新建 AUI Bridge 系统。
只靠文件数量或 manifest 数量通过验收。
```

## 14. Gate 拆分

### Gate A：AUI Document Cook / Package Schema

目标：

```text
authoring AUI -> runtime AuiDocument
manifest path -> aui/documents/*.aui.json
```

验收：

```text
cargo test -p editor_core project_runtime_package_assembler
cargo test -p engine_runtime runtime_package_builder
```

施工前置检查：

```text
先读取 samples/complex_shooter_project/AUI/hud.aui.json。
确认当前 authoring shape 的字段形态。
再实现 legacy_authoring_tree -> runtime AuiDocument 的 cook 映射。
```

必须证明：

```text
package contains aui/documents/hud-main.aui.json
aui-manifest entry points to cooked package path
legacy sample hud can be normalized with warning
```

### Gate B：RuntimePackage AUI Document Load

目标：

```text
load_runtime_package loads AuiDocument registry
```

验收：

```text
cargo test -p engine_runtime runtime_package
```

必须证明：

```text
missing declared document body fails with diagnostic
document id mismatch fails with diagnostic
valid package loads AUI document by id
```

### Gate C：Binding / Layout / Overlay Presenter

目标：

```text
AuiDocument + ProjectUiStateSnapshot -> AuiOverlayFrame
```

验收：

```text
cargo test -p engine_runtime aui
```

必须证明：

```text
binding resolves text/progress/visible/image
missing binding with fallback records warning
missing binding without fallback records error
package_smoke_snapshot can resolve at least one normal text/progress/visible binding
overlay draw_item_count > 0 for sample HUD
```

### Gate D：RenderThread / RuntimeRenderer Present

目标：

```text
RenderFramePacket carries aui_overlay
RenderThread forwards overlay
RuntimeRenderer inserts DrawUiOverlay pass
```

验收：

```text
cargo test -p engine_runtime runtime_renderer
cargo test -p engine_runtime render_thread
```

必须证明：

```text
aui_overlay Some(...) creates DrawUiOverlay pass before Present
aui_overlay None keeps old behavior
render thread no longer drops packet AUI overlay
```

### Gate E：Complex Shooter E2E AUI Present Evidence

目标：

```text
complex shooter export/player report includes AUI package/load/present evidence
```

验收：

```text
cargo test -p project_e2e_gate
cargo test -p runtime_player_winit
```

必须证明：

```text
exported package has cooked AUI document body
load_runtime_package sees AUI document
headless player run report includes AUI present summary
DrawUiOverlay pass exists
```

如果真实 glyph 尚未接通，报告必须明确：

```text
glyph_present=false
status=partial
next_action=runtime_text_glyph_present
```

## 15. 与 185 / 188 / 189 的关系

### 与 185 的关系

185 是完整 M12 产品链路：

```text
Authoring / Binding / RuntimePackage / Runtime Present / Interaction Action / Project Rule
```

190 只切其中一段：

```text
RuntimePackage Document Hydration / Binding / Present
```

190 完成后，185 的 Editor Authoring 可按 209 Scene Unified Authoring 继续加严，完整 action -> Project Rule、真实 glyph visual gate 仍可继续加严。

### 与 188 的关系

188 要求复杂打飞机 HUD 必须通过 AUI，不允许 debug overlay。

190 为 188 提供 AUI 域的更深验收：

```text
不只检查 hud.aui.json 可解析
还检查 RuntimePackage 内 document body / Runtime load / UI pass present
```

### 与 189 的关系

189 定义 ProjectRuntimePackageAssembler 是唯一装配入口。

190 必须通过 Assembler 接入 AUI document cook，不允许 `desktop_export.rs` 或 runtime loader 另开项目扫描路径。

## 16. 自审

### 是否符合用户选择

符合。用户选择 C-min 推荐方案，本方案按完整边界的最小真实路径细化。

### 是否符合项目 skill

符合：

```text
RuntimePackage 是运行输入真相。
Runtime 不扫描源目录。
AUI 走 UiProjection。
不新增 Bridge。
不把打飞机玩法写进引擎 API。
报告必须结构化。
```

### 是否会过度扩大范围

可控。本方案不做 209 Scene Unified AUI Authoring、不做复杂字体系统、不做 UI 动画，只打通 package/load/bind/present。

### 主要风险

风险一：

```text
legacy sample AUI shape 与 runtime AuiDocument shape 不一致。
```

处理：

```text
C-min cook 支持 legacy_authoring_tree -> runtime AuiDocument，并写 warning。
```

风险二：

```text
text glyph present 可能仍未真实接通。
```

处理：

```text
report 必须区分 text command present 和 real glyph present。
不能把 partial 当完整通过。
```

风险三：

```text
RenderThread 可能被迫理解 AUI document。
```

处理：

```text
RenderThread 只转发 AuiOverlayFrame，不做 document load / binding / layout。
```

## 17. 最终结论

本方案采用：

```text
C-min：AUI Document Cook + Runtime Load + Binding + UiProjection Present
```

下一步应基于本文生成可自动化施工文档，而不是直接改代码。

施工文档必须包含：

```text
施工目标
涉及文件
Gate A-E 分阶段任务
每阶段测试命令
完成后文档同步动作
施工自审
```
