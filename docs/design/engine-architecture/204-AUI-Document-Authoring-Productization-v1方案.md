# 204-AUI Document Authoring Productization v1 方案

## 1. 系统定义

本系统正式命名为：

```text
AUI Document Authoring Productization v1
```

采用用户已确认的：

```text
B-min：AUI Authoring Service 产品化
```

一句话：

```text
让用户和 AI 能通过正式编辑器命令创建、打开、编辑、验证、保存、预览 AUI Document，
而不是手写 JSON，或只依赖 Build 阶段 AuiDocumentCooker 兜底归一化。
```

它解决的是 190 / 199 完成后仍然存在的 authoring 缺口：

```text
AUI Document 已经能进入 RuntimePackage，也能由 ProjectUiStateSnapshot 驱动 Binding / Present；
但用户和 AI 还没有一套正式、可验证、可报告的 AUI 文档编辑入口。
```

本系统不是完整 Native AUI Designer，也不是拖拽式 UI Builder。它先把已有 headless `AuiAuthoringService` 产品化成正式 authoring domain。

209 之后的口径修正：

```text
204 是 AUI command / service / report 的 authoring 基础。
后续可视化编辑入口不再走独立 Native AUI Designer。
后续可视化编辑统一由 209 AUI Scene Unified Authoring 承担。
Scene View / Hierarchy / Inspector 是统一编辑表面。
PreviewAuiOverlay 仍只是 preview/report，不是独立 Designer。
```

## 2. 在本引擎主线中的作用

当前 AUI 正式链路是：

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

204 补的是链路最前端：

```text
Editor / AI / Manual Walkthrough
  -> AUI Authoring Command
  -> AuiAuthoringService
  -> AuiTransaction / AuiAuthoringReport
  -> saved canonical AuiDocument
  -> existing RuntimePackage / Present chain
```

完成后，复杂打飞机项目不只是“有 `AUI/hud.aui.json`”，而是可以通过正式命令完成：

```text
CreateAuiDocument
OpenAuiDocument
AddAuiNode
SetAuiNodeField
SetAuiBindingPath
SetAuiActionRef
ValidateAuiDocument
SaveAuiDocument
PreviewAuiOverlay
```

## 3. 其它引擎 / 工具对标

### Unity UI Toolkit / UI Builder

Unity 用 UI Builder 编辑 UXML / VisualTreeAsset，再在运行时加载实例化。可借鉴：

```text
UI 文档是 authoring truth。
编辑器控件不应直接乱改运行态对象。
保存的 UI 资产进入后续 runtime 加载链路。
```

不照搬：

```text
不照搬 Unity 的 UXML / USS 格式。
不采用 MonoBehaviour 字段作为 AUI Binding 真相。
不把用户编辑入口变成隐式对象生命周期。
```

### Unreal UMG / Widget Blueprint

UE 的 UMG 以 WidgetTree / Widget Blueprint 表达 UI 资产。本地 UE 源码参考显示，UMG 修改 WidgetTree 时走 transaction / Modify / Dirty。

可借鉴：

```text
UI authoring 命令入口和真实事务分离。
Widget tree 修改必须进入 undo / dirty / save 边界。
复杂 UI 资产不能靠文本旁路长期维护。
```

不照搬：

```text
不引入 UObject / Slate / Blueprint 全体系。
不把 AUI 做成 UE UMG clone。
```

### Godot Control / PackedScene

Godot UI 使用 Control node tree，场景 / 子树保存为 PackedScene。

可借鉴：

```text
UI 可以是可保存、可实例化、可验证的节点树资产。
authoring data 与运行时对象分离。
```

不照搬：

```text
不采用 Godot Node / Signal 作为本项目 AUI 真相。
```

### Bevy UI

Bevy UI 证明 UI 可以作为结构化数据，layout / interaction / render 分阶段处理。

可借鉴：

```text
UI 数据、布局、交互、渲染分离。
headless deterministic test 有价值。
```

不照搬：

```text
不把 AUI 文档编辑变成用户手写 Rust UI tree。
```

## 4. 当前本项目基线

已具备：

```text
rust/crates/editor_core/src/aui_authoring.rs
  AuiAuthoringService
  AuiTransaction
  AuiAuthoringReport
  create_document / open / add_node / set_node_field / validate / save / report

rust/crates/editor_core/src/aui_document_cooker.rs
  AuiDocumentCooker
  legacy_authoring_tree -> runtime AuiDocument

rust/crates/engine_runtime/src/aui.rs
  AuiDocument / AuiNode / AuiBindingRef / AuiActionRef
  ProjectUiStateSnapshot
  AuiRuntimePresenter
  AuiLayoutEngine / AuiDrawList / AuiOverlayFrame

samples/complex_shooter_project/AUI/hud.aui.json
  当前仍是 legacy/simple authoring tree 形态
```

当前缺口：

```text
UiCommandPayload 没有 AUI authoring 命令。
EditorSession 没有 AUI command 执行入口。
WorkflowCommandResolver / EditorCommandRegistry 没有 AUI command descriptor。
ManualWalkthrough 中 AUI 操作大多仍是 MissingCommand 或 FocusDomainPanel。
AuthoringAiContext 没有 AUI authoring summary。
project_e2e_gate 没有 AUI authoring productization report。
AuiAuthoringService 仍偏 headless 工具，没有成为正式产品化 authoring domain。
```

## 5. 方案选项

### 方案 A：继续 JSON + Cook

做法：

```text
继续让用户 / AI 修改 AUI/*.aui.json；
Build 阶段由 AuiDocumentCooker 归一化。
```

优点：

```text
最快。
不改 EditorSession / UiCommandPayload。
```

缺点：

```text
AI 只能猜 JSON。
用户难以稳定修改 binding / action / node field。
没有正式 transaction / report。
后续 ProjectPatch AUI domain 没有可复用执行出口。
```

结论：

```text
不采用。
```

### 方案 B-min：AUI Authoring Service 产品化

做法：

```text
复用已有 AuiAuthoringService。
新增正式 AUI UiCommandPayload。
EditorSession 通过 AUI service 执行 create/open/edit/validate/save/preview。
输出 AuiAuthoringReport / AuiDocumentAuthoringProductizationReport。
ManualWalkthrough / AuthoringAiContext / project_e2e_gate 能看见 AUI authoring 状态。
```

优点：

```text
AI 适配性最高：命令和报告结构化。
复杂项目可维护：binding / action / node field 都有正式路径。
施工量可控：不先做完整 Designer。
后续 AUI ProjectPatch v2 可复用同一 authoring service。
```

缺点：

```text
第一版不是拖拽式 UI Builder。
Native Editor 里只完成命令 / report / summary 级产品化，完整可视化编辑器后续再做。
```

结论：

```text
采用。
```

### 方案 C：Native AUI Mini Designer（历史备选，209 后不采用）

做法：

```text
做 AUI 面板、节点树、Inspector、预览画布、添加控件按钮。
```

优点：

```text
用户体验更接近 Unity UI Builder / UE UMG Designer。
```

缺点：

```text
牵扯 Native Editor UI、hit region、selection、preview panel、Inspector 交互。
施工面大，容易延迟正式 authoring command 和 report。
```

结论：

```text
不采用为后续主线。
209 已把后续可视化编辑收敛为 Scene Unified Authoring，不再新增独立 AUI Designer。
```

## 6. 正式推荐方案

采用：

```text
B-min：AUI Authoring Service 产品化
```

选择理由按本项目优先级：

### 6.1 AI 适配性

通过。

```text
AI 修改 AUI 时生成 UiCommandPayload / AuiAuthoringCommand，而不是直接改 JSON。
每次修改都有 transaction / diagnostics / report。
binding path / action id / node field 都能被结构化审查。
```

### 6.2 复杂项目适配与可维护

通过。

```text
复杂打飞机 HUD、自走棋商店 / 装备 / 背包 UI 都可以落在 AuiDocument。
复杂 UI 逻辑仍按 195 / 199：Rust Project Framework + Project Assets；
AUI Document 只保存 UI 结构、style、binding path、action ref。
```

### 6.3 效率

通过。

```text
复用已有 AuiAuthoringService 和 190 / 199 runtime 链路。
不把第一版拖进完整 Designer。
headless gate 可自动化验证。
```

## 7. 本轮必须做的能力

### 7.1 UiCommandPayload

新增正式 AUI command：

```text
CreateAuiDocument {
  path,
  document_id,
  width,
  height
}

OpenAuiDocument {
  path
}

AddAuiNode {
  path,
  parent_node_id,
  node_id,
  kind,
  name,
  rect
}

SetAuiNodeField {
  path,
  node_id,
  schema_path,
  value
}

SetAuiBindingPath {
  path,
  node_id,
  target_field,
  binding_id,
  binding_path,
  fallback
}

SetAuiActionRef {
  path,
  node_id,
  event,
  action_id,
  payload
}

ValidateAuiDocument {
  path
}

SaveAuiDocument {
  path
}

PreviewAuiOverlay {
  path
}
```

具体字段类型可按 Rust 类型现状调整，但 command 粒度必须覆盖上面这些用户操作。

### 7.2 EditorSession 执行入口

新增 AUI service 层，建议落点：

```text
rust/crates/editor_core/src/services/aui_service.rs
```

职责：

```text
加载 / 创建 AuiDocument。
调用 AuiAuthoringService 执行 add / edit / validate / save。
把结果写入 CommandTransaction。
刷新 workspace / authoring summary。
输出 EditorDiagnostic。
```

规则：

```text
Editor UI / AI 不能直接 fs::write AUI 文档。
AUI 修改必须进入 EditorSession command / transaction。
保存前必须 validate 或在 report 中说明 validation status。
```

### 7.3 AuiAuthoringService 补强

当前已有 service 可以保留，但需要产品化补齐：

```text
支持 action_refs 编辑。
支持 binding path 单独编辑，而不只 push bindingRefs。
支持 node kind / rect / style 等最小字段编辑。
report 增加 source_path / saved_path / active_document summary。
```

不要求本轮做完整 schema editor。

### 7.4 Workflow / Registry / Manual Walkthrough

需要把 AUI 操作从缺口推进到正式可识别状态：

```text
EditorCommandRegistry 增加 AUI command descriptor。
WorkflowCommandResolver 识别 AUI commands。
ManualWalkthrough AUI operation status 从 MissingCommand 推进到 ExecutableCommand 或 ExecutableCommandNeedsContext。
AuthoringAiContext 能暴露 AUI authoring summary。
```

### 7.5 project_e2e_gate

新增复杂打飞机样例的 AUI authoring productization report：

```text
complex-shooter-aui-authoring-productization-report.json
```

至少证明：

```text
能创建一个 test AUI document。
能打开 samples/complex_shooter_project/AUI/hud.aui.json。
能添加节点或编辑节点字段。
能设置 binding path。
能设置 action ref。
能 validate。
能 save 到临时 / fixture output。
能 preview 生成 AuiOverlayFrame 或诚实报告缺口。
ManualWalkthrough AUI coverage 不再把所有 AUI 操作标为 missing。
```

## 8. Preview 规则

`PreviewAuiOverlay` 第一版不是完整可视化 Designer。

允许：

```text
AuiDocument + test / package smoke ProjectUiStateSnapshot
  -> AuiRuntimePresenter
  -> AuiOverlayFrame
  -> Preview report
```

必须报告：

```text
document_id
snapshot_source
draw_item_count
text_command_count
image_command_count
glyph_present
diagnostics
```

如果真实 glyph 仍未接通：

```text
glyph_present=false
status=partial
next_action=runtime_text_glyph_present
```

禁止：

```text
用 debug overlay 假装 AUI preview。
Preview 阶段直接读 ECS。
Preview 阶段绕过 AuiRuntimePresenter。
```

## 9. 数据与边界规则

必须遵守：

```text
AUI Document 不保存运行时值。
AUI Document 不直接引用 ECS entity。
AUI Binding 只读 ProjectUiStateSnapshot。
AUI action 是业务级 UI 意图，进入 Project Rule / Project Module，不直接改 Runtime World。
Renderer 只读 AuiOverlayFrame，不读 AuiDocument / binding path / ProjectUiStateSnapshot。
```

本轮不新增：

```text
Logic Ownership Router。
新的运行时架构层。
AUI Bridge。
打飞机专用 HUD API。
独立完整 Native UI Designer。
ProjectPatch AUI v2。
```

## 10. 与 185 / 190 / 199 / 202 / 203 的关系

### 与 185

185 是完整 M12 AUI 产品链路。204 只补其中的 Editor / AI authoring productization。

### 与 209

209 是后续 AUI 可视化编辑入口。204 输出的 command / service / report 必须被 209 复用，不能绕开 EditorSession / AuiAuthoringService 另造 Designer 写入路径。

```text
204 = AUI Document 的结构化编辑命令底座。
209 = AUI 在 Scene / Hierarchy / Inspector 中的统一可视化 authoring surface。
```

### 与 190

190 已打通：

```text
AUI Document -> RuntimePackage -> Runtime load -> Binding -> Present
```

204 不能推翻 190，只能让 AUI Document 的来源更正式、更可编辑。

### 与 199

199 已定义 ProjectUiStateSnapshot Producer。204 只编辑 AUI 文档中的 binding path / action ref，不负责 producer 复杂聚合逻辑。

### 与 202

202 的 ProjectPatch v1 当前只真实支持 Scene/Input，AUI patch 仍 unsupported。204 完成后，后续 AUI ProjectPatch v2 可以复用 AUI authoring service。

### 与 203

203 已完成 Prefab authoring productization。204 应学习其模式：

```text
UiCommandPayload
EditorSession service
Authoring report
ManualWalkthrough coverage
project_e2e_gate report
阶段完成记录
```

但不照搬 Prefab Stage 的复杂 instance / override 机制。

## 11. 可施工 Gate

### Gate A：AUI Command Surface

目标：

```text
新增 AUI UiCommandPayload / command_id / descriptor。
ManualWalkthrough AUI 操作能识别 expected command。
```

测试：

```powershell
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core editor_command_registry
```

### Gate B：EditorSession AUI Service

目标：

```text
EditorSession 支持 create / open / add_node / set_field / set_binding / set_action / validate / save。
```

测试：

```powershell
cargo test -p editor_core aui_authoring
```

### Gate C：Preview / Report

目标：

```text
新增 AUI authoring productization report。
PreviewAuiOverlay 走 AuiRuntimePresenter 或诚实 partial。
```

测试：

```powershell
cargo test -p editor_core aui
cargo test -p engine_runtime aui
```

### Gate D：Workflow / AI Context / Manual Walkthrough

目标：

```text
Workflow / ManualWalkthrough / AuthoringAiContext 能报告 AUI authoring 产品化状态。
```

测试：

```powershell
cargo test -p editor_ui_model workflow_command
cargo test -p editor_ui_model manual_walkthrough
cargo test -p editor_core authoring_workflow
cargo test -p editor_core manual_walkthrough
```

### Gate E：Complex Shooter E2E

目标：

```text
project_e2e_gate 生成 complex-shooter-aui-authoring-productization-report.json。
```

测试：

```powershell
cargo test -p project_e2e_gate aui
cargo test -p project_e2e_gate manual_walkthrough
```

### Gate F：整体回归与文档同步

目标：

```text
确认 AUI runtime / package / existing authoring domains 未回退。
```

测试：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime aui
cargo test -p project_e2e_gate
```

## 12. 第一版验收标准

必须证明：

```text
AUI commands 出现在 UiCommandPayload 和 command registry。
EditorSession 能通过 command 修改 AUI 文档。
AUI 修改产生 transaction / diagnostics / report。
ManualWalkthrough 中 AUI create/open/add/edit/binding/action/validate/save/preview 不再全部 missing。
complex shooter e2e 能生成 AUI authoring productization report。
保存后的 AUI document 能继续被 AuiDocumentCooker / RuntimePackage 链路消费。
```

允许保留：

```text
Scene Unified AUI Authoring 未完成。
真实 glyph present 仍按 190 / 后续 runtime_text_glyph_present 处理。
ProjectPatch AUI v2 未完成。
复杂 ScrollView / InputField / DragDrop 未完成。
```

不允许：

```text
直接文本拼 JSON。
绕过 EditorSession 写 AUI 文件。
为了让 walkthrough pass 伪造空 path / 空 node_id command。
把 Preview 当成真实 windowed UI Designer 验收。
```

## 13. 方案自审

### 是否符合用户选择

符合。用户确认采用 B-min，本方案固定为 AUI Authoring Service 产品化。

### 是否符合项目 skill

符合：

```text
schema-first。
结构化 command / report。
不新增玩法专用 API。
不绕过 RuntimePackage / AUI 正式链路。
先方案，后施工文档，再施工测试。
```

### 是否范围过大

可控。本方案不做完整 Designer、不做 Native Editor 复杂拖拽、不做 ProjectPatch AUI v2。

### 是否足够支撑复杂打飞机

足够支撑下一步：

```text
复杂打飞机 HUD 的 document / binding / action 能通过正式命令编辑。
运行时 present 继续复用 190 / 199。
```

### 主要风险

风险一：

```text
现有 AuiAuthoringService 对字段编辑支持较窄。
```

处理：

```text
本轮只补最小 node field / binding / action，不扩成完整 schema editor。
```

风险二：

```text
Preview 被误认为完整可视化 Designer。
```

处理：

```text
Preview 只输出 AuiOverlayFrame / report；真实可视化 authoring 走 209 Scene Unified Authoring。
```

风险三：

```text
AUI ProjectPatch v2 诱惑很大。
```

处理：

```text
本轮只做 command/service/report。ProjectPatch AUI v2 等 authoring domain 产品化后再做。
```

## 14. 最终结论

正式采用：

```text
AUI Document Authoring Productization v1
B-min：AUI Authoring Service 产品化
```

209 之后，本方案的后续可视化方向固定为：

```text
AUI Scene Unified Authoring Productization v1
不新增独立 Native AUI Designer。
```

下一步：

```text
如果没有外部 AI 审查文档，则基于本文生成可自动化施工文档并自审；
如有审查文档，先读取审查结论，再决定是否修订本文。
```
