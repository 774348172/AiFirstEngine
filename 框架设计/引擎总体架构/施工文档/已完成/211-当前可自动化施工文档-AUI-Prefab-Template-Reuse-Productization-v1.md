# 211-当前可自动化施工文档-AUI Prefab / Template Reuse Productization v1

## 1. 施工目标

本轮实现 211 的 C-min：

```text
AUI subtree -> AuiTemplateAsset authoring asset
AuiTemplateAsset -> instantiate-by-expansion -> 真实 AUI Document nodes
实例化报告 -> node_id_remap / dependency warning / diagnostics
```

本轮不实现 linked instance、variant、override editor、apply-back、CommonUI screen flow、完整 Canvas batching，也不把 AUI Node 变成 Runtime ECS Entity。

## 2. 施工前审查结论

已读取：

```text
框架设计/引擎总体架构/211-AUI-Prefab-Template-Reuse-Productization-v1方案.md
其它AI审查目录/30-211-AUI-Prefab-Template-Reuse-Productization方案审查.md
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
施工文档/README.md
阶段完成记录/README.md
```

审查结论：

```text
54 与 施工文档/当前/ 均显示无当前施工文档，可以生成新的唯一施工文档。
30 号审查指出 211 原方案缺 schema/Gate/GUID/binding/action/schema 兼容/M7 关系。
正式 211 已回填这些缺口。
因此可以进入施工，但必须严格保持 C-min。
```

## 3. 涉及文件

预计修改：

```text
rust/crates/editor_core/src/aui_template.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/services/aui_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/manual_walkthrough.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/editor_core/src/tests/aui_template_tests.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/workspace.rs
rust/crates/editor_ui_model/src/workflow_command.rs
rust/crates/editor_ui_model/src/tests.rs
rust/crates/engine_runtime/src/aui.rs
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/aui_template_reuse.rs
rust/crates/project_e2e_gate/src/tests.rs
```

可能只读参考：

```text
rust/crates/editor_core/src/aui_authoring.rs
rust/crates/editor_core/src/services/prefab_service.rs
rust/crates/editor_core/src/prefab_workflow.rs
rust/crates/project_e2e_gate/src/aui_authoring.rs
rust/crates/project_e2e_gate/src/aui_scene_authoring.rs
```

## 4. Gate 任务

### Gate A：AuiTemplateAsset schema

任务：

```text
新增 AuiTemplateAsset / AuiTemplateRef / AuiTemplateInstantiateRequest / AuiTemplateInstantiateReport / AuiTemplateDiagnostic。
支持 JSON roundtrip。
asset_guid 使用 deterministic_path_hash 过渡，并在 report 中说明 guid_source / asset_db_integrated=false。
```

测试：

```powershell
cd rust
cargo test -p editor_core aui_template_schema_roundtrip
```

### Gate B：保存 AUI subtree 为模板

任务：

```text
从 AuiDocument 按 root_node_id 提取完整 subtree。
保持父子关系，收集 asset_refs / binding_refs / action_refs。
写出 AuiTemplateAsset JSON。
```

测试：

```powershell
cd rust
cargo test -p editor_core save_aui_subtree_as_template_collects_dependencies
```

### Gate C：实例化模板并 remap node_id

任务：

```text
读取 AuiTemplateAsset。
按 node_id_prefix / instance_id remap 所有节点。
插入目标 AUI Document parent_node_id / insertion_index。
保存目标 AUI Document。
```

测试：

```powershell
cd rust
cargo test -p editor_core instantiate_aui_template_remaps_node_ids
```

### Gate D：依赖报告与未参数化 warning

任务：

```text
实例化报告列出 copied_binding_refs / copied_action_refs / copied_asset_refs。
模板包含 binding/action 时必须输出 warning：未参数化，多个实例可能读同一路径或触发同一 action。
```

测试：

```powershell
cd rust
cargo test -p editor_core instantiate_aui_template_reports_copied_dependencies
```

### Gate E：AUI validation warning

任务：

```text
Image 节点带 text 字段输出 warning。
Text 节点带 image 字段输出 warning。
不改变 draw_list 行为，只增强 diagnostics。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_node_kind_field_mismatch_validation
```

### Gate F：e2e 重复 UI fixture

任务：

```text
在 project_e2e_gate 构造重复装备格 / 按钮 AUI fixture。
保存一次模板，实例化多次。
证明实例化后节点进入 layout / draw_list / composition/present report。
生成 complex-shooter-aui-template-reuse-productization-report.json。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate aui_template_reuse
```

### Gate G：整体回归与文档归档

任务：

```text
跑整体回归。
生成阶段完成记录。
更新 49 / 54 / 施工文档 README / 阶段完成记录 README。
把本施工文档从 当前/ 移动到 已完成/。
```

测试：

```powershell
cd rust
cargo fmt --check
cargo test -p editor_core aui_template
cargo test -p editor_ui_model
cargo test -p engine_runtime aui
cargo test -p project_e2e_gate aui_template_reuse
```

## 5. 验收标准

必须证明：

```text
保存任意 AUI subtree 为 AuiTemplateAsset。
AuiTemplateAsset 含 schema_version / asset_guid / template_id / source evidence。
实例化后目标 AUI Document 存在真实 AuiNode，不存在 runtime template instance。
node_id_remap 稳定且无 duplicate node id。
binding/action/asset 依赖被保留并进入 report。
binding/action 未参数化 warning 可见。
Image+text / Text+image validation warning 可见。
重复 UI fixture 能进入 AUI present 链路。
runtime / renderer 不读取 AuiTemplateAsset。
```

允许保留：

```text
deterministic_path_hash GUID 过渡。
没有 linked instance / apply-back / variant / nested template。
没有参数化 binding/action。
没有完整 UI batching / mask / rich text / IME。
```

不允许：

```text
新增运行时 AUI Template 层。
新增隐藏 Runtime Widget Instance。
把 path-only 当模板唯一身份。
绕过 AUI authoring service 直接乱写 AUI document。
把 M7 Scene Entity Prefab 数据模型直接套到 AUI。
```

## 6. 施工自审

```text
范围是否扩大：否，只做 C-min。
是否跳过 30 号审查：否，已回填正式 211。
是否会改变 runtime schema：否，AuiTemplateAsset 是 editor authoring asset。
是否新增运行时层：否，实例化结果是普通 AUI Document nodes。
是否有 Gate 测试：有，Gate A-G 均有命令。
是否需要联网：否，本轮是按已完成方案施工。
是否当前唯一施工文档：是，生成前 当前/ 为空。
```
