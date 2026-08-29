# 150-AI-first Editor Command Framework C-min 方案

## 1. 问题定义

本方案解决 `149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md` 中的三个关联问题：

```text
Q8  Editor UI 层级偏多，后续 UI 功能可能多层重复注册 command / hit / payload / handler。
Q12 editor_core 体量已经偏大，EditorSession 聚合过多领域状态与命令执行逻辑。
Q13 editor_window_winit 后续容易继续承载窗口、输入、UI、业务执行、present 编排等过多职责。
```

当前真实代码已经暴露出命令真相层分散的问题：

```text
rust/crates/editor_core/src/lib.rs                                  5100 lines
rust/crates/editor_core/src/lib.rs                                  execute_command 大 match
rust/crates/editor_core/src/lib.rs                                  command_id_for_payload
rust/crates/editor_input/src/lib.rs                                 command_id_for_payload
rust/crates/editor_core/src/authoring_workspace.rs                  workspace_command_id_for_payload
rust/crates/editor_core/src/property_editing.rs                     property_command_id_for_payload
```

这不是单纯“文件太长”的问题，而是命令定义、命令可用性、命令执行入口、测试命令构造、AI proposal 接受路径已经开始分散。继续按当前方式加 UI，会让每个新面板都重复注册一套 command id / payload / availability / handler，后期 AI 调试也会越来越难定位。

## 2. 现有确认规则

已有文档确认的规则继续有效：

```text
EditorUiModel 是 UI 状态真相。
SelfUiRenderer 只生成 DrawCommand / HitRegion。
editor_input 只做事件、HitRegion、快捷键到命令请求的转换，不执行业务。
EditorSession 是编辑器应用状态与服务协调入口，不应该长期充当所有命令定义和所有业务执行的大 match。
editor_wgpu_renderer 只画和 present，不理解业务。
editor_window_winit 只负责 OS window、事件循环、输入转发、surface/present 编排，不执行业务。
egui / eframe 不进入正式窗口主线，editor_ui_backend_egui 只保留为 headless 兼容/测试辅助。
```

相关文档：

```text
86-真实UI命令接入SceneEditing-C-min方案.md
88-真实NativeEditorWindow-EventLoop-UIDraw-C-min方案.md
111-Native-Editor-Real-UI-Present-方案B.md
114-Native-Editor-UI-RenderGraph-RHI-收敛方案.md
121-Native-Editor-Application-Shell方案.md
127-Editor-Interaction-Feedback-Command-Availability-v1方案.md
148-当前实现工程审查与功能摸底报告-2026-07-02.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
```

## 3. 其他引擎参考

### 3.1 Unreal Engine

UE 的编辑器命令路线最接近本项目要学习的方向。

源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Public\Framework\Commands\UICommandList.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Public\Framework\Commands\UIAction.h
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Private\Framework\Commands\UICommandList.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\Slate\Private\Framework\Commands\UIAction.cpp
```

UE 的关键结构：

```text
FUICommandInfo
  描述一个命令的身份、文字、快捷键等。

FUIAction
  包含 ExecuteAction / CanExecuteAction / CheckState / Visible / RepeatMode。

FUICommandList
  负责 MapAction、CanExecuteAction、TryExecuteAction、ProcessCommandBindings。
```

UE 的核心思想不是“按钮自己执行业务”，而是：

```text
Widget / Menu / Shortcut
  -> CommandInfo
  -> CommandList 查找 action
  -> CanExecute
  -> Execute
  -> Transaction / Tool / Editor domain
```

值得学习：

```text
命令身份和执行绑定分离。
可执行条件是命令系统的一等能力。
菜单、按钮、快捷键都能走同一套命令。
命令执行前可以统一做 CanExecute。
```

第一版不照搬：

```text
不照搬完整 Slate widget framework。
不照搬递归 command list / parent-child lookup。
不照搬完整 ToolMenus / EditorMode / UObject reflection。
不照搬复杂插件扩展机制。
```

### 3.2 Unity

Unity 没有一个完全等价 UE `FUICommandList` 的统一命令层。Unity 更像是：

```text
EditorWindow / UI Toolkit / IMGUI / MenuItem / Shortcut
  -> SerializedObject / SerializedProperty
  -> Undo / dirty / ApplyModifiedProperties
```

源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Undo\Undo.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\UIElements\Inspector\InspectorElement.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\UIElements\Controls\PropertyField.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\PackageManagerUI\Editor\UI\List\PackageSearchBar.cs
```

Unity 的 `ValidateCommand / ExecuteCommand`、`SerializedObject.ApplyModifiedProperties`、`Undo.RecordObject` 说明它也在做“显示层事件”和“实际数据修改/Undo”分离，只是命令分布更散。

值得学习：

```text
属性编辑统一走数据绑定、Undo、dirty。
Inspector 不应该绕开事务直接改数据。
```

不适合照搬：

```text
Unity 编辑器命令散在大量 EditorWindow / Inspector / UI Toolkit 控件中。
这对人类长期维护可行，但对 AI-first 调试不是最优。
```

### 3.3 Godot

Godot 更偏插件和控件驱动：

```text
EditorPlugin / Control / InputEvent / Shortcut
  -> 具体 editor plugin handler
  -> EditorUndoRedoManager
```

源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\editor
<GODOT_SOURCE>\godot-master\godot-master\scene\gui\popup_menu.h
<GODOT_SOURCE>\godot-master\godot-master\scene\gui\popup_menu.cpp
```

值得学习：

```text
编辑器扩展按领域插件组织。
UndoRedo 是真实修改的关键入口。
```

不适合照搬：

```text
命令和控件/插件耦合更强。
AI 想追踪一个按钮最后改了什么，需要跨多个插件和控件路径。
```

### 3.4 Bevy

Bevy 当前不是成熟 Unity/UE 式编辑器命令框架参考。它的 ECS schedule / event / state 设计有价值，但不能直接作为本系统的编辑器命令架构参考。

## 4. 方案对比

### 方案 A：只写文档规则，不改代码结构

做法：

```text
继续保留现有 UiCommandPayload / EditorSession::execute_command。
只在文档里规定新 UI 不要乱注册命令。
```

优点：

```text
最快。
不影响现有测试。
```

缺点：

```text
无法阻止 command_id_for_payload 继续复制。
无法阻止 EditorSession::execute_command 继续变大。
无法给 AI 一个唯一命令入口。
只是缓解，不解决。
```

结论：

```text
不推荐。
```

### 方案 B：轻量 CommandCatalog

做法：

```text
新增 CommandCatalog，集中 command_id / payload kind / availability。
EditorSession 继续执行主要业务。
```

优点：

```text
比 A 明显更好。
实现成本低。
能先解决部分重复 command id 问题。
```

缺点：

```text
容易变成现有系统旁边的补丁层。
执行、可用性、事务、AI trace 仍可能分散。
长期不够像正式编辑器命令框架。
```

结论：

```text
可作为过渡，但不符合当前“长期主义，不再补临时层”的要求。
```

### 方案 C：完整 UE Slate Command Framework

做法：

```text
实现完整 CommandInfo / CommandList / Action / Shortcut / Menu / Tool / Plugin 扩展框架。
```

优点：

```text
长期最强。
最接近 UE。
```

缺点：

```text
第一版过重。
会把 Q8 从“收敛命令真相层”扩大成“重写完整编辑器 UI 框架”。
当前 UI、Dock、Widget、Plugin 还没有成熟到需要完整 Slate 级复杂度。
```

结论：

```text
长期方向可参考，但第一版不直接做完整 C。
```

### 推荐：方案 C-min

做法：

```text
建立 AI-first Editor Command Framework C-min。
学习 UE 的 CommandInfo / Action / CanExecute / Execute 思路。
不做完整 Slate，不做完整插件命令系统。
先把命令身份、payload、availability、execute、feedback、trace 收敛成唯一正式通道。
```

结论：

```text
选 C-min。
```

## 5. C-min 正式设计

### 5.1 核心目标

```text
一个命令只有一个正式定义位置。
一个命令只有一个正式执行入口。
UI、快捷键、AI、测试都产生同一种 EditorCommandRequest。
命令是否可执行由命令框架统一判断。
命令执行结果统一形成 feedback / diagnostics / trace。
EditorSession 不再承载命令大字典，只做应用状态持有和服务协调。
```

### 5.2 核心数据结构

```text
EditorCommandId
  稳定字符串 id，例如 scene.create_entity / project.open / runtime.play。

EditorCommandDescriptor
  command_id
  title
  category
  owner_domain
  payload_kind
  default_shortcut
  availability_policy
  trace_level

EditorCommandPayload
  第一版可以复用/包装现有 UiCommandPayload。
  长期应迁移成 editor_core command payload，UI crate 不拥有业务 payload 真相。

EditorCommandRequest
  command_id
  source
  request_id
  payload

EditorCommandContext
  当前 Project / Selection / Scene / PlayState / BuildState / Focus / Dirty 等只读上下文。

EditorCommandAvailability
  enabled
  visible
  checked
  disabled_reason

EditorCommandAction
  can_execute(context, payload) -> EditorCommandAvailability
  execute(session/services, payload) -> EditorCommandResult

EditorCommandResult
  command_id
  status
  diagnostics
  feedback
  state_changes
  trace_events
  ui_model_revision

EditorCommandRegistry
  命令描述与 action 注册唯一来源。

EditorCommandExecutor
  validate -> execute -> transaction/report/feedback 的统一入口。
```

### 5.3 命令流

```text
UI / Shortcut / AI / Test
  -> EditorCommandRequest
  -> EditorCommandFramework
  -> lookup descriptor/action
  -> can_execute(context, payload)
  -> disabled: feedback + report，不执行业务
  -> enabled: execute action
  -> domain service / transaction
  -> EditorCommandResult
  -> EditorUiModel refresh / Console / Trace / Report
```

### 5.4 crate 边界

```text
editor_ui_model
  保留 UI 状态模型。
  可以暂时保留 UiCommandPayload 作为兼容类型。
  长期不拥有命令真相层。

editor_ui_renderer
  根据 EditorUiModel 生成 DrawList / HitRegion。
  HitRegion 只携带 command_id 和轻 payload hint，不执行业务。

editor_input
  OS/input/hit/shortcut -> EditorCommandRequest。
  不维护 command_id_for_payload 的第二份真相。

editor_core
  拥有 EditorCommandFramework。
  拥有命令 descriptor/action/executor。
  拥有领域 service 与 transaction。

editor_window_winit
  只做窗口事件循环、输入转发、present 编排。
  不判断业务可用性，不直接执行业务。

editor_wgpu_renderer
  只消费 DrawList / RenderGraph / RHI。
  不感知 command。
```

## 6. editor_core 拆分方案

### 6.1 当前判断

`editor_core/src/lib.rs` 当前约 5100 行，已经不适合作为长期主实现文件。问题不是 Rust 单文件不能编译，而是：

```text
EditorSession 状态字段过多。
execute_command match 覆盖多个领域。
scene command 在 AI proposal accept 路径再次重复。
command_id_for_payload 在多个 crate/模块重复。
测试 helper 绑定到 lib.rs。
新增功能倾向继续往 lib.rs 塞。
```

结论：

```text
需要拆，但不做无意义“大搬家”。
第一阶段只拆命令框架和会继续膨胀的执行路由。
```

### 6.2 推荐模块结构

第一阶段新增/收敛：

```text
rust/crates/editor_core/src/editor_command.rs
  EditorCommandId
  EditorCommandDescriptor
  EditorCommandRequest
  EditorCommandPayload 兼容包装
  EditorCommandAvailability
  EditorCommandResult

rust/crates/editor_core/src/editor_command_registry.rs
  builtin command descriptors
  duplicate id validation
  payload kind validation

rust/crates/editor_core/src/editor_command_executor.rs
  execute(request, session/services)
  can_execute(request, context)
  old UiCommand -> EditorCommandRequest adapter

rust/crates/editor_core/src/editor_session.rs
  EditorSession struct
  app state accessors
  build_ui_model
  high-level coordination

rust/crates/editor_core/src/editor_session_state.rs
  Project/session/runtime/scene/AI/build/play state grouping
```

后续按领域逐步 service 化：

```text
rust/crates/editor_core/src/editor_services/project_service.rs
rust/crates/editor_core/src/editor_services/scene_service.rs
rust/crates/editor_core/src/editor_services/asset_service.rs
rust/crates/editor_core/src/editor_services/input_mapping_service.rs
rust/crates/editor_core/src/editor_services/play_service.rs
rust/crates/editor_core/src/editor_services/build_service.rs
rust/crates/editor_core/src/editor_services/ai_service.rs
```

### 6.3 拆分顺序

第一步：

```text
建立 editor_command.rs / registry / executor。
把 command_id_for_payload 的唯一真相移到 editor_core command framework。
editor_input、authoring_workspace、property_editing 不再维护自己的 command id 映射。
```

第二步：

```text
把 EditorSession::execute_command 大 match 搬到 editor_command_executor。
EditorSession 保留 execute_command 兼容入口，但内部转发给 executor。
```

第三步：

```text
把 AI proposal accept 里的 scene command 重复 match 改成重新提交 EditorCommandRequest。
AI 不再单独维护一套“允许执行哪些命令”的局部执行器。
允许列表属于 command descriptor / availability policy。
```

第四步：

```text
拆 EditorSession state grouping。
不是为了少几行，而是为了让 project / scene / play / build / ai 状态边界可读。
```

第五步：

```text
按领域 service 化真实业务执行。
这一步可以渐进，不阻塞 C-min 第一版。
```

## 7. C-min 不做什么

第一版明确不做：

```text
不做完整 UE Slate。
不做完整 command list 树。
不做插件 command 扩展 API。
不做完整快捷键编辑器。
不做完整菜单系统。
不做完整 Tool Framework。
不把命令框架扩大成项目玩法规则系统。
```

这些不是放弃，而是避免第一版过重。长期可以在 C-min 上继续扩展，不需要推翻。

## 8. 命名规则

推荐使用领域前缀，降低冲突：

```text
project.open
project.create
project.refresh_recent
workspace.set_view_mode
scene.open
scene.save
scene.select_entity
scene.create_entity
scene.delete_entity
scene.rename_entity
scene.set_transform
scene.set_component_field
asset.select
asset.open
asset.place_into_scene
runtime.open_package
runtime.reload_package
runtime.play
runtime.pause
runtime.step_frame
runtime.reset
build.export_desktop
build.open_output
build.open_report
console.clear
ai.submit_prompt
ai.accept_proposal
ai.reject_proposal
```

旧的 snake_case command id 可以第一版兼容，但正式 descriptor 应使用点分层 id。兼容映射必须集中在 command framework，不能散在各 crate。

## 9. 测试门禁

C-min 施工后至少需要这些测试：

```text
command_registry_rejects_duplicate_ids
all_builtin_payloads_have_descriptor
legacy_ui_command_maps_to_editor_command_request
editor_input_does_not_define_business_command_truth
disabled_command_returns_feedback_without_business_execution
shortcut_click_ai_test_share_same_executor
ai_accept_proposal_reenters_command_framework
scene_create_entity_command_executes_through_framework
runtime_play_command_checks_availability_before_execute
editor_session_execute_command_keeps_backward_compatibility
```

回归：

```text
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_input
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit
```

## 10. 与其他引擎对比

| 项目 | UE | Unity | Godot | 我们 |
|---|---|---|---|---|
| 命令身份 | FUICommandInfo | MenuItem/Shortcut/事件名较分散 | Shortcut/Menu/Plugin 较分散 | EditorCommandDescriptor |
| 执行绑定 | FUIAction | EditorWindow/Inspector callback | EditorPlugin/Control handler | EditorCommandAction |
| 可执行判断 | CanExecuteAction | ValidateCommand/控件 enabled/自定义逻辑 | 控件/插件判断 | EditorCommandAvailability |
| Undo/Transaction | FScopedTransaction/GUndo | Undo/SerializedObject | EditorUndoRedoManager | EditorTransaction/Domain Service |
| UI 与业务分离 | 强 | 中等，较分散 | 中等，插件驱动 | 强，AI 可读 |
| AI 友好 | 弱 | 中等 | 中等 | 强 |
| 第一版复杂度 | 成熟系统，复杂 | 成熟系统，复杂 | 插件分散 | C-min 控制复杂度 |

本方案最像 UE 的命令框架思想，但不照搬 Slate。它比 Unity/Godot 更集中，更适合 AI 查找“一个按钮为什么不能点、点了以后改了什么、失败原因是什么”。

## 11. 为什么适合本项目

### AI 友好

```text
命令定义唯一。
可用性、执行、反馈、trace 可统一查询。
AI 不需要在 UI、input、session、service 多处猜测命令语义。
```

### 复杂项目支持

```text
复杂编辑器一定会有菜单、快捷键、按钮、AI command、测试 command。
如果没有统一命令层，复杂项目后期必然产生重复入口。
```

### 长期维护

```text
先收命令真相层，再逐步拆 service。
不会为了清理文件行数而制造大规模无收益重构。
```

### 简单度

```text
C-min 只做 command descriptor/action/availability/executor。
不做完整 Slate，不做插件系统，不做复杂菜单树。
```

### 效率

```text
编辑器命令查询和执行不是高频 runtime hot path。
集中框架带来的开销可以忽略，换来调试和维护收益。
```

## 12. 正式规则

```text
1. 新增编辑器命令必须先进入 EditorCommandDescriptor。
2. UI、快捷键、AI、测试不得直接执行业务，只能提交 EditorCommandRequest。
3. editor_input 不得维护业务 command id 真相。
4. editor_ui_renderer 不得执行业务，不得判断业务可用性。
5. editor_window_winit 不得执行业务。
6. EditorSession::execute_command 保留兼容，但必须转发到 EditorCommandExecutor。
7. 命令可用性必须由 EditorCommandAction::can_execute 或 descriptor policy 统一产生。
8. disabled command 必须返回可读 feedback，不允许静默失败。
9. AI proposal accept 必须重新进入 EditorCommandFramework，不得维护第二套执行 match。
10. editor_core 拆分优先拆命令框架和执行路由，再拆领域 service。
```

## 13. 方案自审

```text
Specification fit:
  本方案直接解决 Q8 UI 命令重复注册风险，同时回答 editor_core 是否过大以及如何拆分。

Rule fit:
  遵守 AI-first、长期主义、系统级讨论优先、引擎底座不引入项目玩法规则的既有规则。

Textual consistency:
  文中统一使用 EditorCommandFramework / Descriptor / Action / Request / Executor，不再把轻量 CommandCatalog 当最终方案。

Design fit:
  路线接近 UE 的命令框架思想，同时保留本项目 UI DrawList / RHI / EditorSession 边界，适合复杂编辑器长期演进。

Implementation feasibility:
  当前已有 UiCommandPayload / CommandResult / CommandStatus / EditorSession / editor_input，可通过兼容 adapter 渐进迁移，不需要推翻现有 UI。

Practical reasonableness:
  C-min 不做完整 Slate、插件系统和菜单树，复杂度可控；第一阶段拆 command 真相层，收益明确。
```

结论：

```text
方案通过自审。
正式采用 AI-first Editor Command Framework C-min。
下一步如进入施工，应先生成 150 对应施工文档，再按模块测试推进。
```
