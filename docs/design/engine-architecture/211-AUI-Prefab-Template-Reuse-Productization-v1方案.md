# 211-AUI Prefab / Template Reuse Productization v1 方案

## 1. 系统是什么

本系统正式命名为：

```text
AUI Prefab / Template Reuse Productization v1
```

一句话：

```text
让用户和 AI 可以像使用 Unity Prefab 一样复用复杂 UI 组合，但 AUI 的工程真相仍然是 AUI Document subtree，而不是 Runtime ECS Entity 或 GameObject Component 容器。
```

它不是预设模板库。用户不应该只能从引擎预置的 HUD / Button / InventoryCell 模板里选；用户应该能把任意 AUI 节点子树组合成可复用资产，并在多个界面里插入、校验和报告。

定位规则：

```text
211 不是第二套 Prefab 系统。
211 是通用 Project Prefab / Template 体系里的 AUI domain part。
用户体验可以像 Prefab，自由组合；工程真相仍按 domain 分区。
C-min 先做 AUI subtree template save / instantiate-by-expansion / report，不承诺完整 linked instance、variant、反向 apply。
```

## 2. 与现有系统关系

本方案接在以下系统之后：

```text
203-Prefab-Authoring-Productization-v1方案.md
204-AUI-Document-Authoring-Productization-v1方案.md
209-AUI-Scene-Unified-Authoring-Productization-v1方案.md
210-RuntimeRenderer-Multi-stage-UI-Composition-Pass-Productization-v1方案.md
```

203 负责通用 Scene / Prefab authoring 心智。211 只固定 AUI 复用对象在 AUI Document 中的表达方式，并把 AUI subtree 接入同一个 Project Prefab / Template 用户心智。

如果未来做“一个 Prefab 同时包含 Scene Entity、AUI subtree、Rule、Asset 引用”的统一项目 prefab，用户体验可以继续像 Unity Prefab，但内部必须保持 domain truth 分区：

```text
Scene / Entity part -> Scene / Prefab data
AUI part -> AUI Document subtree
Rule part -> Rule Asset / RuleSlot
Asset part -> AssetRef / Asset DB
```

## 3. 核心设计规则：复杂控件用 AUI subtree

复杂 AUI 控件不靠一个 `AuiNode` 同时塞 `text` / `image` / `behavior` 来表达。

标准表达是：

```text
Container node: Panel / Button
  Image child node
  Text child node
  Panel / ProgressBar child node
  Interaction / Action refs on semantic container
```

示例，一个装备格按钮：

```text
equip_slot_button        kind=Button, action_refs=[open_equipment_detail]
  equip_slot_bg          kind=Image, image=slot_frame
  equip_icon             kind=Image, image={binding: equipment.icon}
  equip_name             kind=Text,  text={binding: equipment.name}
  equip_level            kind=Text,  text={binding: equipment.level}
  cooldown_mask          kind=ProgressBar, progress_value={binding: equipment.cooldown}
```

固定规则：

```text
Text 节点负责文字绘制。
Image 节点负责图片绘制。
Panel / Button 节点负责容器、背景、交互语义和 action_refs。
复杂视觉组合通过 children 表达，不通过单节点多组件黑箱表达。
```

当前 `AuiNode` 结构上允许 `text` 和 `image` 字段同时存在，但 `AuiLayoutEngine::extract_draw_list` 按 `AuiNodeKind` 展开。后续 validation 应对不匹配组合给出 warning：

```text
Image 节点带 text 字段：warning，文字不会按通用规则绘制。
Text 节点带 image 字段：warning，图片不会按通用规则绘制。
复杂控件需要拆成 AUI subtree。
```

## 3.1 C-min 数据模型

C-min 不新增运行时 UI 层，也不把 AUI template 做成独立 Runtime Entity。它只增加可审查的 authoring asset、展开命令和报告。

正式 schema：

```text
AuiTemplateAsset
  schema_version: "aui-template-asset.v1"
  asset_guid: string
  template_id: string
  display_name: string
  source_document_path: string
  source_document_id: string
  root_node_id: string
  nodes: Vec<AuiNode>
  asset_refs: Vec<AuiTemplateDependencyRef>
  binding_refs: Vec<AuiTemplateDependencyRef>
  action_refs: Vec<AuiTemplateDependencyRef>
  metadata: AuiTemplateMetadata

AuiTemplateDependencyRef
  node_id: string
  field_path: string
  value: string

AuiTemplateMetadata
  created_by: string
  created_at_unix_ms: u64
  source_node_count: usize
  notes: Vec<string>
```

模板引用：

```text
AuiTemplateRef
  asset_guid: string
  template_id: string
  asset_path: string
```

插入时使用展开策略：

```text
AuiTemplateInstantiateRequest
  template_ref: AuiTemplateRef
  target_document_path: string
  parent_node_id: string
  insertion_index: Option<usize>
  instance_id: string
  node_id_prefix: string

AuiTemplateInstantiateReport
  template_ref: AuiTemplateRef
  instance_id: string
  target_document_path: string
  parent_node_id: string
  inserted_node_count: usize
  node_id_remap: Vec<(source_node_id, inserted_node_id)>
  asset_ref_count: usize
  binding_ref_count: usize
  action_ref_count: usize
  copied_binding_refs: Vec<AuiTemplateDependencyRef>
  copied_action_refs: Vec<AuiTemplateDependencyRef>
  copied_asset_refs: Vec<AuiTemplateDependencyRef>
  override_supported: false
  linked_instance_supported: false
  runtime_instance_supported: false
  diagnostics: Vec<AuiTemplateDiagnostic>

AuiTemplateDiagnostic
  severity: info | warning | error
  code: string
  node_id: Option<string>
  message: string
  suggested_action: string
```

关键规则：

```text
插入结果是真实 AUI Document nodes，不是运行时 instance object。
node_id 必须 remap，不能把 template 内 node_id 原样复制到目标文档导致冲突。
binding_refs / action_refs / asset_refs 默认保留，但必须进入 report，方便用户和 AI 检查模板依赖。
C-min 不在 AuiDocument / AuiNode 里写入 template instance 字段；template 来源只存在于 authoring report / command report。
如需长期 linked instance / override / apply-to-template，后续再单独产品化。
```

## 3.2 30 号审查采纳结论

`其它AI审查目录/30-211-AUI-Prefab-Template-Reuse-Productization方案审查.md` 的结论是：211 方向正确，但原文不足以直接施工，必须补全 schema、数据结构、Gate、测试、验收，并处理 binding/action、GUID、schema 兼容、M7 Prefab 关系和 e2e 重复 UI evidence。

本方案采纳如下：

```text
1. C-min 使用 AuiTemplateAsset + instantiate-by-expansion + report，不增加运行时 widget / instance 层。
2. 模板身份使用 asset_guid + template_id；asset_path 只作为定位和报告证据，不作为长期唯一身份。
3. 当前不修改 AuiDocument schema；旧 AUI Document 无需迁移。
4. RuntimePackage / Runtime / Renderer 不读取 AuiTemplateAsset；只有实例化命令把模板展开进 AUI Document 后，runtime 才看到普通 AuiNode。
5. M7 / 203 Prefab 只能借鉴四件套心智和 service/report 风格，不能直接复用 Scene Entity 专用数据模型。
6. 复杂打飞机当前 HUD 节点少，211 的 e2e 需要构造重复装备格 / 按钮 UI fixture 来证明复用价值。
```

施工前判断：

```text
只有本节补齐后，才允许生成 211 施工文档。
施工文档必须保持 C-min，不允许扩展到 linked instance、variant、override editor、CommonUI screen flow 或完整 Canvas batching。
```

## 3.3 Binding / Action 复制语义

C-min 不新增 binding 参数化语言，不引入 `{slot_index}` / `{instance_prefix}` 这类 placeholder，也不让模板拥有隐藏运行时上下文。

第一版规则：

```text
实例化时 binding_refs 原样复制到新节点。
实例化时 action_refs 原样复制到新节点。
实例化报告必须列出 copied_binding_refs / copied_action_refs。
只要模板包含 binding/action，实例化报告必须生成 warning，提示这些引用未参数化，多个实例可能读取同一路径或触发同一 action。
AI / 用户随后通过已有 AUI authoring command 修改真实节点上的 binding/action。
```

原因：

```text
这避免为了模板复用过早发明一门 binding 参数化语言。
问题不会被隐藏：report 会把所有复制出的 binding/action 依赖列出来。
多个装备格都显示同一 equipment.icon 这类问题可以被测试和 AI report 定位。
```

后续如果要支持真正的参数化模板，应另开方案，并且必须同时定义：

```text
template parameters
binding path rewrite rule
action payload parameter rule
validation / preview / migration report
```

## 3.4 GUID / 引用与 schema 兼容

C-min 模板资产必须携带稳定身份：

```text
asset_guid + template_id 是模板身份。
asset_path 是定位和人类可读证据。
report 中不得只记录 path-only template reference。
```

当前如果 Asset DB / meta 集成不足，命令可以用 deterministic stable guid 作为过渡：

```text
asset_guid = stable_hash("aui-template" + canonical_template_asset_path + template_id)
```

但报告必须诚实说明：

```text
guid_source: deterministic_path_hash
asset_db_integrated: false
```

schema 兼容策略：

```text
AuiTemplateAsset 是独立 authoring asset。
AuiDocument schema 不因 211 C-min 改版。
旧 AUI Document 不需要 migration。
RuntimePackageBuilder / AuiDocumentCooker 不直接读取 template asset。
模板实例化之后保存的是普通 AUI Document nodes，所以 RuntimePackage / runtime load / layout / draw_list 链路不需要知道 template。
```

## 4. 与 Unity UGUI 的对应关系

Unity UGUI 的运行心智：

```text
GameObject + Text/Image/Button Component
  -> 属性变化 SetVerticesDirty
  -> CanvasUpdateRegistry 注册 rebuild
  -> Graphic.Rebuild(PreRender)
  -> UpdateGeometry
  -> OnPopulateMesh(VertexHelper)
  -> VertexHelper.FillMesh(workerMesh)
  -> canvasRenderer.SetMesh(workerMesh)
  -> CanvasRenderer 渲染
```

我们的 AUI 不照搬 GameObject + Component。对应链路是：

```text
AUI Document / AuiNode subtree
  -> AUI Document patch 或 ProjectUiStateSnapshot binding 变化
  -> Binding Resolve
  -> AuiLayoutEngine::layout
  -> AuiLayoutEngine::extract_draw_list
  -> DrawRect / DrawImage / DrawText
  -> AuiOverlayDrawItem / AuiCompositionFrame
  -> RuntimeRenderer DrawUiComposition pass
  -> RHI UiComposition / Wgpu backend
  -> Present
```

对应关系：

```text
Unity GameObject hierarchy
≈ AUI Document node tree

Unity Graphic Component
≈ AuiNodeKind + AuiNode data + draw extraction rule

Unity OnPopulateMesh generated quad
≈ DrawRect / DrawImage / DrawText after layout

Unity CanvasRenderer
≈ RuntimeRenderer UI composition pass + RHI UiComposition
```

这是概念对应，不是能力等价。当前 `RHI UiComposition` 已能证明 AUI composition pass 进入渲染链路，但还不是 Unity CanvasRenderer 级的完整 mesh batching / material batching / mask / text shaping 系统。

差异：

```text
Unity 的 rebuild / mesh / CanvasRenderer 更成熟，但对 AI 来说较黑箱。
AUI 的 DrawList / OverlayFrame / Report 更显式，更适合 AI patch、诊断和回放。
AUI 目前还不是完整 Canvas batching 系统，复杂 batching、mask、rich text、scroll/input 仍需后续产品化。
```

## 4.1 与 UE Slate / UMG / CommonUI 的对应关系

UE 的 UI 栈可以拆成三层理解：

```text
Slate
  C++ widget / paint / draw element / renderer 内核。

UMG
  WidgetBlueprint / UWidget / WidgetTree 可视化游戏 UI 层。

CommonUI
  建在 UMG 之上的游戏 UI 产品层，补激活栈、输入动作、手柄/键鼠提示、多平台导航。
```

源码对比后的判断：

```text
UE Slate 的 SWidget::Paint / OnPaint / FSlateDrawElement
≈ AUI 的 layout / extract_draw_list / AuiDrawCommand。

UE UMG 的 UWidgetTree / UWidget / TakeWidget / RebuildWidget
≈ AUI Document authoring / AuiNode subtree / RuntimePackage hydration。

UE CommonUI 的 ActivatableWidget / ButtonBase / ActionRouter
≈ AUI 后续需要的 screen flow / focus navigation / input action productization。
```

但这些只是职责对标，不是结构照搬。211 的结论是：

```text
不复制 UE 的 Slate -> UMG -> CommonUI 三层运行时结构。
不新增 AUI Runtime Widget 对象层来模拟 UWidget / SWidget。
不把 AUI Template 变成 CommonUI 风格的运行时 widget instance。

可以学习 Slate 的显式 draw element 思路。
可以学习 UMG 的 WidgetTree authoring / transaction / designer 体验。
可以学习 CommonUI 的可复用游戏 UI 控件心智、激活栈、输入动作提示和导航约束。

C-min 仍只做 AUI Document subtree template save / instantiate-by-expansion / report。
CommonUI 级 screen flow / focus navigation / input action 以后单独产品化，不塞进 211。
```

这次源码对比强化了 211 的核心边界：AUI 的优势不是比 UMG/UGUI 更成熟，而是让复杂 UI 复用保持 schema-first、可审查、可报告、AI 可修改。模板实例化后的工程真相必须仍然是 AUI Document nodes，而不是隐藏在运行时 widget instance 里。

## 5. 当前 AUI 对复杂 UI 的支持判断

现在已经有的对应能力：

```text
AuiDocument / AuiNode 数据真相。
AuiNodeKind: Panel / Image / Text / Button / ProgressBar 等基础类型。
Binding Resolve 读取 ProjectUiStateSnapshot。
AuiLayoutEngine 计算 rect。
extract_draw_list 生成 DrawRect / DrawImage / DrawText。
AuiOverlayFrame / AuiCompositionFrame 保存 item、stage、sort_key、glyph_plan。
RuntimeRenderer 能插入 DrawUiComposition pass。
Runtime text glyph C-min 已能生成 glyph quad 和 report evidence。
209 让 AUI Node 进入 Scene 统一选择 / Inspector authoring。
210 让 AUI Canvas / LayerGroup 可以按 stage 进入 RuntimeRenderer。
```

还缺的能力：

```text
没有 Unity CanvasUpdateRegistry 等价的 dirty registry / rebuild cache。
没有成熟 UI batching / atlas batch key / material batch。
Image 还缺 sliced / tiled / filled sprite 的完整产品化。
Text 还缺 CJK shaping / rich text / fallback chain / line wrap 的完整产品化。
Mask / Clip / ScrollView / InputField / IME / focus navigation 还未完整产品化。
CommonUI 级 screen activation stack / input action prompt / gamepad navigation 还未产品化。
AUI subtree template save / instantiate report 尚未产品化。
linked instance / override / variant 尚未产品化。
不匹配字段组合的 validation warning 还未固定。
```

结论：

```text
AUI 现在的架构可以承载复杂 UI，但还没有达到 Unity UGUI 的完整成熟度。
211 的第一目标不是补齐所有渲染细节，而是先把复杂 UI 的复用真相固定为 AUI subtree prefab/template，让用户和 AI 不再复制粘贴大量节点，也不把 AUI Node 误建模成 GameObject Component。
```

## 6. C-min 边界

C-min 做：

```text
保存任意 AUI subtree 为 AuiTemplateAsset。
从 AuiTemplateAsset 向 AUI Document 插入展开后的节点子树。
生成 node_id_remap，避免目标文档 node_id 冲突。
保留 binding_refs / action_refs / asset_refs，并在 report 中列出。
输出 AuiTemplateInstantiateReport，说明来源、插入数量、依赖和诊断。
用户和 AI 继续通过 AUI authoring command 修改插入后的真实 AUI nodes。
```

C-min 不做：

```text
不把 AUI Node 改成 Runtime ECS Entity。
不把 AUI prefab 做成 GameObject Component 模型。
不做完整跨 domain unified prefab。
不做 linked instance 长期同步。
不做 apply instance change back to template。
不做 template variant / nested template。
不做 override diff editor。
不做完整 Canvas batching / mask / rich text / IME。
不做 CommonUI 级 screen stack / action router / focus navigation。
不把引擎预设模板当成唯一复用方式。
```

## 6.1 Authoring Command 边界

C-min 只需要最小命令：

```text
SaveAuiSubtreeAsTemplate {
  document_path
  root_node_id
  template_asset_path
  template_id
  display_name
}

InstantiateAuiTemplate {
  template_ref
  target_document_path
  parent_node_id
  insertion_index
  instance_id
  node_id_prefix
}

ValidateAuiTemplate {
  template_ref
}
```

这些命令必须走现有 AUI authoring / ProjectPatch / report 边界；禁止直接 `fs::write` 绕过 AUI document service。

命令报告必须包含：

```text
operation
status
template_ref / template_asset_path
target_document_path
inserted_node_count
node_id_remap
dependencies
diagnostics
```

## 6.2 可施工 Gate

```text
Gate A：AuiTemplateAsset schema + JSON roundtrip。
  目标：新增独立 authoring 数据结构，能序列化 / 反序列化，包含 asset_guid + template_id。
  测试：cargo test -p editor_core aui_template_schema_roundtrip

Gate B：从 AUI Document 提取 root_node_id 子树，生成 AuiTemplateAsset。
  目标：按 AuiDocument node tree 提取完整 subtree，并收集 asset_refs / binding_refs / action_refs。
  测试：cargo test -p editor_core save_aui_subtree_as_template_collects_dependencies

Gate C：实例化模板到目标 AUI Document，完成 node_id_remap。
  目标：插入展开后的真实 AuiNode，保持父子关系，避免 node_id 冲突。
  测试：cargo test -p editor_core instantiate_aui_template_remaps_node_ids

Gate D：保留 binding_refs / action_refs / asset_refs，并输出 AuiTemplateInstantiateReport。
  目标：复制依赖但报告未参数化 warning，AI / 用户可读。
  测试：cargo test -p editor_core instantiate_aui_template_reports_copied_dependencies

Gate E：validation warning 覆盖 Image+text / Text+image 等不匹配字段组合。
  目标：不改变渲染语义，只在 authoring validation/report 中给出清晰诊断。
  测试：cargo test -p engine_runtime aui_node_kind_field_mismatch_validation

Gate F：AUI present smoke：实例化后的 subtree 能进入 layout / draw_list / composition report。
  目标：重复 UI fixture 实例化后仍是普通 AUI Document nodes，可进入现有 AUI present 链路。
  测试：cargo test -p project_e2e_gate aui_template_reuse

Gate G：入口同步与归档。
  目标：完成阶段记录，更新 49 / 54 / 施工文档 README / 阶段完成记录 README，并归档施工文档。
  测试：人工核对当前施工目录为空，cargo fmt --check 通过。
```

建议测试：

```powershell
cargo test -p editor_core aui_template
cargo test -p engine_runtime aui
cargo test -p project_e2e_gate aui_template_reuse
cargo fmt --check
```

## 6.3 验收标准

必须证明：

```text
用户 / AI 可以把任意 AUI subtree 保存成 AuiTemplateAsset。
AuiTemplateAsset 有 schema_version、asset_guid、template_id 和 source evidence。
实例化会把 template subtree 展开成目标 AUI Document 的真实 nodes。
node_id_remap 稳定输出，目标文档不会产生 duplicate node id。
binding/action/asset 依赖会被保留并写入 report。
包含 binding/action 的模板实例化会输出未参数化 warning。
Image+text / Text+image 这类单节点误用会输出 validation warning。
实例化后的重复 UI fixture 能进入 layout / draw_list / composition 或 e2e report。
Runtime / Renderer 不读取 AuiTemplateAsset，也不存在隐藏 runtime template instance。
```

允许保留：

```text
没有 linked instance。
没有 apply instance back to template。
没有 template variant / nested template。
没有参数化 binding/action。
没有完整 Canvas batching / mask / rich text / IME。
Asset DB GUID 集成可以是 deterministic_path_hash 过渡，但必须在 report 中说明。
```

不允许：

```text
把 AUI Node 改成 Runtime ECS Entity。
把 AUI Template 做成运行时 widget instance。
把 path-only 当作模板唯一身份。
让 runtime / renderer 直接读 AuiTemplateAsset。
绕过 AUI authoring service 直接写 AUI document。
为了复用模板新增一层 Logic Ownership Router / Architecture Guard。
```

## 7. AI 和用户可读性规则

AI 默认读取：

```text
AUI Document subtree
AUI template/prefab source id
AUI template instantiate report
node_id_remap
AuiDrawList / AuiOverlayFrame / AuiCompositionFrame report
```

用户默认看到：

```text
Scene 统一 Hierarchy 中的 AUI subtree。
Inspector 中的 AUI Node 字段。
Template 来源、插入报告和 remap 诊断。
自然语言错误解释。
```

AI / 用户不需要理解 UE 式多层运行时对象。默认解释口径是：

```text
模板是什么：一段可复用的 AUI Document subtree。
实例化做了什么：把 subtree 展开成目标 AUI Document 里的真实节点。
为什么能渲染：真实节点继续走 binding / layout / draw_list / composition。
为什么不是隐藏对象：没有运行时 prefab instance，所以 AI 和用户都能直接看到并修改节点。
```

错误示例：

```text
规则：EquipSlotButton
节点：equip_icon
问题：Image 节点包含 text 字段，但 Image 节点只生成 DrawImage。
建议：把文字拆到 Text 子节点，例如 equip_name。
```
