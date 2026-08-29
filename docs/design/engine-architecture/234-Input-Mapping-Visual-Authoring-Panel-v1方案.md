# 234-Input Mapping Visual Authoring Panel v1 方案

> 状态：正式方案，用户已确认采用 `方案 B-min+`。  
> 确认日期：2026-07-10。  
> 所属路线：`227` 的 `P1-3 Input Mapping Visual Authoring Panel v1`。  
> 前置：`135 M5 Native Input System`、`136 M6 RuntimePackage InputMapping Resource Loading`、`143 M10 Input Mapping Authoring -> Runtime Productization` 已完成。  
> 本文只固定方案，不允许直接施工；下一步仍需方案审查/自审、施工文档、施工文档自审、分 Gate 施工与测试。

## 0. 用户确认结论

本系统正式采用：

```text
方案 B-min+:
  Unified Input Mapping Workspace
  + Existing InputMappingAsset Truth
  + Editor-only Working Copy
  + Stable Binding Identity
  + Conflict / Impact Diagnostics
  + Deterministic Input Preview
```

一句话含义：

```text
让用户不写 JSON，就能在现有编辑器 Workspace 中编辑 Context / Action / Binding，
并在保存、构建和运行前知道输入是否有效、是否冲突、最终会产生哪个 ActionSnapshot。
```

核心裁定：

```text
InputMappingAsset 仍是唯一项目资产真相。
不新增 Input Graph、ControlSchemeAsset 或第二套 Input Mapping 文档。
InputMappingEditorState 只是 editor-only working state，不进入项目资产或 RuntimePackage。
所有修改最终降低到 InputMappingEditCommand / ProjectPatch Input operation。
Runtime 继续只消费 RuntimePackage 内的 InputMappingAsset 和 ActionSnapshot。
```

## 1. 这个系统是干什么的

当前引擎已经具备：

```text
EditorCore 创建/修改/保存 InputMappingAsset
-> DesktopExportPipeline 收集项目 Input Mapping
-> RuntimePackage input manifest
-> Runtime Player 加载项目默认 mapping
-> InputResolver
-> ActionSnapshot
-> Project Rule / Runtime gameplay
```

但普通用户仍缺少真正可操作的可视化编辑面。现有能力主要停留在 model、command、service 和测试层，Native UI renderer 没有绘制完整 Input Mapping 编辑器。

P1-3 要补齐：

```text
选择 InputMappingAsset
-> 查看 Context / Action / Binding
-> 增删和编辑允许的字段
-> 监听键盘/鼠标或从设备目录选输入
-> 检查冲突和外部引用影响
-> 用确定性输入预览验证 ActionSnapshot
-> Save / Discard
-> Build & Run 使用保存后的项目 mapping
```

它不是：

```text
不是重做 Native Input System。
不是运行时 Rebinding 菜单。
不是完整多玩家设备系统。
不是通用输入图语言。
不是新建独立 OS 窗口或新的长期架构层。
```

## 2. 其它引擎源码对标

### 2.1 Unity Input System

官方资料：

```text
Input Action Assets
https://docs.unity3d.com/Packages/com.unity.inputsystem@1.14/manual/ActionAssets.html
```

本地源码：

```text
<LOCAL_TEST_ROOT>/AIPVtest/Library/PackageCache/com.unity.inputsystem@21a28c3a6c83/InputSystem/Actions/InputActionAsset.cs
<LOCAL_TEST_ROOT>/AIPVtest/Library/PackageCache/com.unity.inputsystem@21a28c3a6c83/InputSystem/Editor/UITKAssetEditor/InputActionsEditorWindow.cs
<LOCAL_TEST_ROOT>/AIPVtest/Library/PackageCache/com.unity.inputsystem@21a28c3a6c83/InputSystem/Editor/UITKAssetEditor/InputActionsEditorState.cs
<LOCAL_TEST_ROOT>/AIPVtest/Library/PackageCache/com.unity.inputsystem@21a28c3a6c83/InputSystem/Editor/UITKAssetEditor/Commands/Commands.cs
<LOCAL_TEST_ROOT>/AIPVtest/Library/PackageCache/com.unity.inputsystem@21a28c3a6c83/InputSystem/Editor/UITKAssetEditor/Views/InputActionsEditorView.cs
<LOCAL_TEST_ROOT>/AIPVtest/Library/PackageCache/com.unity.inputsystem@21a28c3a6c83/InputSystem/Editor/UITKAssetEditor/PackageResources/InputActionsEditor.uxml
```

关键实现：

```text
InputActionAsset 是单一资产真相。
InputActionsEditorWindow 创建 working copy，维护 dirty，并在 Save 时写回资产。
StateContainer + InputActionsEditorState 保存选择和编辑器状态。
Commands.cs 收敛 Add/Delete/Duplicate/Move/ApplyModifiedProperties/SaveAsset。
InputActionsEditor.uxml 使用 Action Maps | Actions + Bindings | Properties 三栏布局。
BindingPropertiesView 使用 InputControlPathEditor 编辑设备路径。
```

可学习：

```text
资产真相与 editor working copy 分离。
选择状态和资产数据分离。
UI 操作统一进入 command，而不是 widget 直接写文件。
Action / Binding / Properties 三栏适合复杂输入资产。
```

不照搬：

```text
不复制完整 Control Scheme、Composite Binding 和庞大的 Interaction/Processor 插件生态。
不新增独立 Input Actions OS window；本项目使用现有 Workspace domain。
```

### 2.2 Unreal Engine Enhanced Input

官方资料：

```text
Enhanced Input
https://dev.epicgames.com/documentation/en-us/unreal-engine/enhanced-input-in-unreal-engine
```

本地源码：

```text
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Plugins/EnhancedInput/Source/EnhancedInput/Public/InputAction.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Plugins/EnhancedInput/Source/EnhancedInput/Public/InputMappingContext.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Plugins/EnhancedInput/Source/EnhancedInput/Public/EnhancedActionKeyMapping.h
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Plugins/EnhancedInput/Source/EnhancedInput/Private/EnhancedInputSubsystemInterface.cpp
<UNREAL_ENGINE_SOURCE>/UnrealEngine-release/UnrealEngine-release/Engine/Plugins/EnhancedInput/Source/InputEditor/Private/ActionMappingDetails.cpp
```

关键实现：

```text
UInputAction 与 UInputMappingContext 是独立 DataAsset。
FEnhancedActionKeyMapping 保存 Action / Key / Triggers / Modifiers。
ActionMappingDetails 支持 add / clear / copy / paste / drag reorder。
QueryMapKeyInActiveContextSet 按 Context priority 检查同键冲突、遮挡和保留映射。
RebuildControlMappings 生成当前玩家实际生效的 mappings。
```

可学习：

```text
冲突不能只检查同一列表重复；必须考虑 Context 优先级和 consume 语义。
Action value type 与设备输入类型应做兼容性诊断。
Runtime 生效状态与 authoring 资产应有明确区分。
```

不照搬：

```text
不把 InputAction 和 InputMappingContext 拆成更多项目资产。
不在本轮复制 Player Mappable Profile、Chord、复杂 Trigger/Modifier UObject 体系。
```

### 2.3 Godot InputMap

官方资料：

```text
InputMap
https://docs.godotengine.org/en/stable/classes/class_inputmap.html
```

本地源码：

```text
<GODOT_SOURCE>/godot-master/godot-master/editor/settings/action_map_editor.cpp
<GODOT_SOURCE>/godot-master/godot-master/editor/settings/input_event_configuration_dialog.cpp
<GODOT_SOURCE>/godot-master/godot-master/editor/settings/event_listener_line_edit.cpp
<GODOT_SOURCE>/godot-master/godot-master/editor/settings/project_settings_editor.cpp
<GODOT_SOURCE>/godot-master/godot-master/core/input/input_map.cpp
```

关键实现：

```text
ActionMapEditor 使用 action tree，每个 action 下挂 input events。
支持 action add / rename / remove / reorder 和 event add / edit / remove。
EventListenerLineEdit 支持监听用户输入。
ProjectSettingsEditor 把 action 修改写入 ProjectSettings，并通过 queue_save 延迟保存。
```

可学习：

```text
监听输入是最直接的 binding 编辑体验。
简单 action tree 比通用节点图更适合第一版。
```

不照搬：

```text
Godot InputMap 缺少本项目已有的 Context priority / consume / Processor / Trigger 表达。
纯平铺 Action 列表不足以支撑长期复杂项目。
```

### 2.4 Bevy

本地源码：

```text
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_input/src
<BEVY_SOURCE>/bevy-main/bevy-main/crates/bevy_winit/src/converters.rs
```

关键判断：

```text
Bevy 官方核心主要提供 ButtonInput / keyboard / mouse / gamepad 等底层资源与事件。
高层 action mapping 和 authoring UI 通常由项目或插件承担。
```

可学习的是底层输入与高层 action 分离；不把 Bevy 作为本轮可视化编辑器的主要产品参考。

## 3. 本项目当前基线

### 3.1 已完成底座

`143 M10` 已完成：

```text
InputMappingAuthoringModel / Report / Command
InputMappingAuthoringService
Create / Load / Save / Validate
Add / Remove Action
Add / Remove Binding
Set Binding Device Path
Workspace / AssetBrowser summary
DesktopExport collection
RuntimePackage / Player project mapping verification
```

Runtime 数据结构已经具备：

```text
InputMappingAsset
InputActionDefinition
InputContextDefinition
InputBindingDefinition
InputActionValueType: Button / Axis1 / Axis2 / Pointer
InputTriggerPreset: Down / Pressed / Released / Hold / Tap
InputProcessorPreset: None / Deadzone / Normalize / Scale / Invert
```

### 3.2 当前真实缺口

```text
Native UI renderer 没有绘制 InputMappingAuthoringModel 产品面。
没有 selected context / action / binding editor state。
没有 working copy / dirty / Save / Discard 语义；现有编辑命令会直接写文件。
binding_index 不稳定，不适合 AI 长期 patch 和重排后的定位。
不能编辑 Context、Action value type、Trigger、Processor。
没有键盘/鼠标 binding capture。
没有结构化 Context conflict / type compatibility / impact diagnostics。
没有 deterministic InputResolver preview。
Report Panel 没有 Input Mapping authoring provider。
```

### 3.3 必须诚实处理的现状

```text
InputContextDefinition.enabled_by_default 当前没有进入 InputResolver 的 active context 过滤。
因此本轮不得把它作为可用开关暴露给用户。

Gamepad RuntimeInputEvent 和 resolver 已支持最小路径，
但正式支持目录当前只有 gamepad/South 与 gamepad/LeftStick，
Native Editor 尚没有真实 OS gamepad capture backend。
```

## 4. 方案回顾

### 方案 A：Godot-like Flat Table

```text
Action list
  -> Binding rows
  -> Device picker / keyboard-mouse capture
```

优点：施工小、上手快。  
缺点：Context、冲突治理、复杂项目维护和 AI 稳定定位不足。  
结论：不作为正式方案。

### 方案 B-min+：Unified Input Mapping Workspace

```text
Existing Workspace Input domain
  -> Context list
  -> Action + nested Binding tree
  -> Properties
  -> Diagnostics / Preview
```

优点：不新增持久化层，复用现有资产和命令链，同时能覆盖复杂项目所需的 Context、稳定定位、冲突诊断和测试证据。  
缺点：需要补 editor working copy、Native UI、部分 command 和 report。  
结论：正式采用。

### 方案 C：Full Input Authoring Suite

```text
Control Scheme
Platform Override Editor
Composite Binding
Dynamic Context
Runtime Rebinding
Player Device Ownership
Live Input Debugger
```

优点：长期上限最高。  
缺点：会把 P1-3 扩成多个系统并引入新的产品心智。  
结论：长期 deferred，只预留兼容接口。

## 5. 正式架构：B-min+

### 5.1 单一真相

项目资产真相继续是：

```text
Project/Input/*.json
  -> InputMappingAsset
```

构建与运行真相继续是：

```text
InputMappingAsset
  -> ProjectRuntimePackageAssembler / DesktopExportPipeline
  -> RuntimePackage input manifest
  -> Runtime Player default mapping
  -> InputResolver
  -> ActionSnapshot
```

禁止新增：

```text
InputMappingPanelAsset
InputGraphDocument
ControlSchemeAsset
第二套 editor-only mapping 文件
Widget 直接写 RuntimePackage
```

### 5.2 编辑器 Working Copy

新增或扩展 editor-only 状态：

```text
InputMappingEditorState
  selected_asset_path
  selected_context_id
  selected_action_id
  selected_binding_id
  source_hash
  draft_mapping
  dirty
  capture_session
  preview_result
```

规则：

```text
Working copy 不落盘为独立文件。
Working copy 不进入 RuntimePackage。
UI 编辑只修改 draft_mapping。
Save 前 validate，并检查 expected source_hash。
源文件被外部修改时拒绝覆盖，输出 stale source diagnostic。
Discard 重新加载 InputMappingAsset。
```

ProjectPatch / 自动化命令不需要打开 UI working copy，但必须复用同一 `InputMappingEditCommand` 和原子 commit 逻辑：

```text
load -> expected hash check -> apply edits -> validate -> atomic save -> report
```

### 5.3 Workspace 产品面

Input Mapping 编辑器进入现有中央 Workspace，不新增独立 Designer 或 OS window。

布局：

```text
Toolbar:
  asset selector | search | device filter | Validate | Preview | Save | Discard

Left:
  Contexts
  context_id / priority / consume_input

Center:
  Actions
    Action row
      Binding row
      Binding row

Right:
  Action Properties or Binding Properties
  value type / device path / trigger / processor

Bottom:
  Diagnostics / Conflict / Preview ActionSnapshot
```

打开入口：

```text
Authoring Workflow 选择 Input step
或
Asset Browser 双击 InputMappingAsset
```

### 5.4 稳定身份与兼容迁移

现有 `binding_index` 只允许作为 legacy/internal 兼容，不再作为 AI 和 UI 的长期定位主键。

正式增加稳定 binding identity：

```text
InputBindingDefinition.binding_id
```

兼容规则：

```text
正式规范版本升级为 input-mapping.v2。
loader 同时接受 input-mapping.v1 / input-mapping.v2。
旧 binding 缺少 binding_id 时，按 asset_id + context_id + action_id + device_path + occurrence 生成 deterministic migration id。
首次成功保存后写入持久化 binding_id。
新 authoring save 输出规范化 input-mapping.v2。
RuntimePackage build 在写入 package 前使用同一 normalization，不允许 editor/runtime 各自实现迁移。
Runtime 只把 binding_id 当诊断/source mapping 信息，不参与输入求值。
```

AI / UI 定位：

```text
asset_path
expected_mapping_hash
context_id
action_id
binding_id
field_path
```

### 5.5 Command 设计

保留现有命令：

```text
CreateDefaultInputMapping
SaveInputMapping
ValidateInputMapping
AddInputAction
RemoveInputAction
AddInputBinding
RemoveInputBinding
SetInputBindingDevicePath
```

本轮新增或扩展：

```text
OpenInputMapping
SelectInputContext
SelectInputAction
SelectInputBinding
DiscardInputMappingDraft

AddInputContext
RemoveInputContext
SetInputContextPriority
SetInputContextConsumeInput

SetInputActionValueType
SetInputBindingTrigger
SetInputBindingProcessor
BeginInputBindingCapture
CancelInputBindingCapture
CommitCapturedInputBinding

PreviewInputMapping
RefreshInputMappingDiagnostics
```

所有写命令必须降低到：

```text
InputMappingEditCommand
  -> InputMappingAuthoringService
  -> validate
  -> commit/report
```

默认不启用：

```text
RenameInputAction
RenameInputContext
DuplicateInputAction
MoveInputAction
MoveInputBinding
```

原因：现有项目可能在 Scene / Prefab / Rule / AUI action 中引用 action id。Rename/Remove 不能只改 InputMappingAsset；必须先有引用影响分析和明确确认。RemoveInputAction 本轮保留，但执行前必须输出 usage evidence，并在存在外部引用时要求显式确认或拒绝。

### 5.6 设备目录与 Capture

设备路径不允许用户依赖自由字符串猜测。产品面读取结构化 `InputControlCatalog`：

```text
Keyboard:
  keyboard/*

Mouse:
  mouse/Left
  mouse/Right
  mouse/Middle
  mouse/Position
  mouse/Wheel

Gamepad C-min:
  gamepad/South
  gamepad/LeftStick
```

键盘/鼠标：

```text
Begin Capture
-> EditorInputEvent KeyDown / PointerDown / Wheel / PointerMove
-> normalized device_path
-> conflict preview
-> Commit or Cancel
```

Capture 路由约束：

```text
只有 Input Workspace 获得焦点且 capture_session active 时才截获输入。
Capture event 必须先于 editor shortcut / GameView route 消费，不能同时触发编辑器命令或游戏输入。
Escape 默认 Cancel Capture，不绑定为输入；用户需要绑定 Escape 时必须通过设备目录显式选择。
PointerMove 只有目标 Action value type 为 Pointer 时才允许捕获为 mouse/Position。
失去窗口焦点时自动 Cancel Capture。
```

Gamepad：

```text
本轮允许从受支持目录选择并进行 synthetic preview。
真实物理手柄 capture 需要 Native Editor gamepad backend，明确 deferred。
未支持的 gamepad path 在 UI 中 disabled，并给出 capability diagnostic。
```

### 5.7 Context 边界

本轮可编辑：

```text
context_id
priority
consume_input
```

本轮不可编辑：

```text
enabled_by_default
```

原因：当前 Runtime resolver 没有 active context set，所有 mappings 都按 priority 参与求值。暴露一个不生效的 toggle 会制造假功能。

动态 Context activation、菜单/Gameplay context 切换和 player-local context stack 后续另开系统，不塞入 P1-3。

### 5.8 冲突与类型诊断

参考 UE `QueryMapKeyInActiveContextSet`，但只实现当前 schema 所需的确定性分析：

```text
UnknownAction
UnknownContext
UnsupportedDevicePath
DuplicateBindingInSameContext
HiddenByHigherPriorityConsumingContext
HidesLowerPriorityBinding
ActionValueTypeDeviceMismatch
BindingWithoutAction
ActionWithoutBinding
StaleSourceHash
ExternalActionReferenceImpact
RuntimeUnsupportedField
```

每条诊断必须包含：

```text
code
severity
asset_path
context_id?
action_id?
binding_id?
field_path?
human_explanation
suggested_fix
```

### 5.9 Deterministic Input Preview

Preview 复用正式 `InputResolver`，不另造简化求值器：

```text
Draft InputMappingAsset
  + captured/synthetic RuntimeInputFrame
  -> InputResolver::resolve
  -> ActionSnapshot
  -> InputMappingPreviewResult
```

Preview 必须显示：

```text
input event / device path
matched binding_id
resolved action_id
value type / phase / value
consumed or shadowed mapping
diagnostics
```

Preview 是 editor-only，不修改 World，不启动 RuntimePackage，不替代 Play Mode。

### 5.10 Save / Discard / 外部修改

Save 流程：

```text
draft validate
-> conflict / impact policy
-> expected source_hash check
-> normalized serialization
-> atomic file replace
-> AssetBrowser / Workspace refresh
-> report
```

不允许：

```text
每次 widget 值变化直接写文件。
无 hash 检查覆盖外部修改。
保存 invalid mapping 后继续显示 passed。
Save 直接写 RuntimePackage。
```

### 5.11 Report Panel

不新增新的报告面板，只向现有 `ReportRegistry / ReportPanelModel` 注册 Input authoring provider。

建议报告：

```text
InputMappingVisualAuthoringReport
  schema_version
  status
  asset_path
  source_hash
  dirty
  action_count
  context_count
  binding_count
  supported_device_paths
  conflict_count
  diagnostics
  changed_paths
  preview_summary
  next_actions
```

分档：

```text
Editor Off:
  不生成 authoring report。

Editor Summary（默认）:
  counts / status / top diagnostics / preview summary / next actions。

Editor Trace（Gate / debug）:
  完整 source mappings / conflicts / preview trace。

Runtime:
  不生成 Visual Authoring report；只保留运行所需 ActionSnapshot / compact diagnostics。
```

## 6. AI 编辑规则

AI 默认读取：

```text
InputMappingAuthoringModel
InputControlCatalog
InputMappingVisualAuthoringReport
Project references to action ids
```

AI 默认修改：

```text
ProjectPatch Input operations
或
InputMappingEditCommand list
```

必须带：

```text
asset_path
expected_mapping_hash
stable binding_id for binding edits
source / rationale
```

禁止 AI：

```text
按不稳定 binding_index 猜修改目标。
绕过 validator 直接写 JSON。
把项目 action id 做成 engine core API。
修改 RuntimePackage 内 cooked mapping 作为项目真相。
在未做影响分析时 rename/remove 被引用的 action。
```

## 7. 复杂打飞机最小验收

样例资产：

```text
samples/complex_shooter_project/Input/input.default.json
```

当前动作：

```text
action.move
action.fire
action.pointer
action.pause
```

最小用户流程：

```text
打开 complex shooter project
-> 进入 Input Workspace
-> 选择 input.default
-> 选择 action.fire
-> Begin Capture
-> 按下新的 keyboard key 或选择 mouse/Left / gamepad/South
-> conflict preview
-> PreviewInputMapping
-> ActionSnapshot 输出 action.fire=Pressed
-> Save
-> Build & Run
-> exported RuntimePackage 使用新 mapping
```

验收必须证明：

```text
Native Editor 真实显示 Context / Action / Binding / Properties。
用户可完成至少一次键盘或鼠标 capture。
至少一次 gamepad/South 或 gamepad/LeftStick catalog selection 能进入 synthetic preview。
invalid/conflicting binding 产生结构化 diagnostic。
binding edit 通过 stable binding_id 定位。
Save 后重新打开结果一致。
Build 后 RuntimePackage.default_input_mapping 使用保存后的项目资产。
RuntimeInputFrame 经正式 InputResolver 产生预期 ActionSnapshot。
未新增 Player / Fire / Move 等项目专用 engine API。
```

## 8. 推荐施工 Gate

后续施工文档建议按以下 Gate 拆分。

### Gate A：Schema / Stable Identity / Migration

```text
InputBindingDefinition binding_id。
v1 legacy migration。
expected mapping hash。
InputControlCatalog。
```

建议测试：

```powershell
cargo test -p engine_input input_mapping
cargo test -p editor_ui_model input_mapping
```

### Gate B：Editor Working Copy / Commands

```text
InputMappingEditorState。
selection / dirty / Save / Discard。
Context / Action value / Trigger / Processor edit commands。
ProjectPatch / headless atomic commit 复用。
```

建议测试：

```powershell
cargo test -p editor_core input_mapping_authoring
cargo test -p editor_core input_mapping_draft
```

### Gate C：Native Visual Panel / Capture

```text
Input Workspace mode。
Context list / Action tree / Properties / Diagnostics。
键盘和鼠标 capture。
Gamepad supported path catalog selection。
```

建议测试：

```powershell
cargo test -p editor_ui_renderer input_mapping
cargo test -p editor_input input_mapping
cargo test -p editor_window_winit input_mapping
```

### Gate D：Conflict / Preview / Report Panel

```text
Context priority/consume conflict analyzer。
Action value/device compatibility。
InputResolver deterministic preview。
Report Panel Input provider。
```

建议测试：

```powershell
cargo test -p engine_input input_mapping
cargo test -p editor_core input_mapping_conflict
cargo test -p editor_core report_panel
```

### Gate E：Complex Shooter E2E

```text
complex shooter mapping open/edit/capture/preview/save/reload。
DesktopExport / RuntimePackage / ActionSnapshot evidence。
complex-shooter-input-mapping-visual-authoring-report.json。
```

建议测试：

```powershell
cargo test -p project_e2e_gate input_mapping_visual_authoring
cargo test -p editor_core desktop_export_input_mapping
cargo test -p runtime_player_winit input_mapping
```

### Gate F：整体回归与文档归档

```powershell
cargo fmt --check
cargo test -p engine_input
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_ui_renderer
cargo test -p editor_input
cargo test -p editor_window_winit
cargo test -p runtime_player_winit
cargo test -p project_e2e_gate
```

## 9. 本轮不做

```text
完整 Control Scheme 编辑器。
Platform Override 可视化编辑。
Composite Binding 图编辑器。
Chord / arbitrary trigger graph。
动态 Runtime Context stack。
真实物理 gamepad device enumeration / capture backend。
Runtime rebinding AUI menu。
用户 key profile / cloud sync。
多玩家 input ownership / device pairing。
完整 live input debugger / timeline。
```

这些能力后续如成为复杂项目阻塞，应分别开系统，不回塞到 234。

## 10. 风险与治理

### 风险 A：为 UI 再造第二套 InputMapping 真相

治理：

```text
Working copy 只在 EditorSession 生命周期内存在。
保存后唯一真相仍是 InputMappingAsset。
RuntimePackage 仍由正式 build 链路生成。
```

### 风险 B：binding_index 导致 AI 修改错目标

治理：

```text
增加稳定 binding_id。
patch 同时校验 expected_mapping_hash。
legacy index 仅作兼容，不进入新产品面默认 command。
```

### 风险 C：暴露不生效字段

治理：

```text
enabled_by_default 不进入可编辑 UI。
Gamepad 只显示 Runtime 和 authoring 都支持的路径。
unsupported capability 必须 disabled + diagnostic。
```

### 风险 D：删除/重命名 Action 破坏项目引用

治理：

```text
删除前做 usage impact scan。
存在外部引用时要求显式确认或拒绝。
Rename 默认 disabled，直到有跨 Scene/Prefab/Rule/AUI 的原子引用更新。
RemoveInputContext 在仍有 binding 时必须拒绝，或在明确 impact preview 后级联删除；不得留下 UnknownContext binding。
```

### 风险 E：范围膨胀成完整 Unity/UE 输入套件

治理：

```text
本轮只做现有 schema 能真实运行的 Context / Action / Binding 产品面。
Control Scheme、动态 Context、runtime rebinding、多玩家、完整 gamepad backend 全部 deferred。
```

## 11. 自审

### 11.1 与用户选择一致

通过。

```text
采用 B-min+。
不采用 A 的平铺最小面板。
不直接施工 C 的完整 Input Suite。
```

### 11.2 AI 适配性

通过。

```text
稳定 binding_id + expected hash 避免错改。
所有修改走结构化 command / ProjectPatch。
diagnostic/report 包含 source mapping、human explanation 和 next action。
```

### 11.3 复杂项目与长期维护

通过。

```text
保留 Context / Action / Binding 正式结构。
增加 Context conflict 和外部引用影响分析。
不拆分更多资产，不增加运行时层。
```

### 11.4 效率

通过。

```text
Native panel 复用现有 Workspace、model、service 和 Runtime resolver。
Preview 不启动 RuntimePackage 或 World。
Runtime 热路径不生成 authoring report。
```

### 11.5 与 M10 是否重复

不重复。

```text
M10 完成资产、service、build 和 runtime 链路。
234 补的是 Native visual product surface、working copy、稳定定位、capture、conflict 和 preview。
```

### 11.6 结构复杂度

通过。

```text
没有新增持久化架构层。
InputMappingEditorState 只是编辑器临时状态，性质与 Scene/Rule 未保存工作状态相同。
唯一资产真相仍是 InputMappingAsset。
```

### 11.7 已知设计债务是否被掩盖

没有。

```text
enabled_by_default 当前不生效，明确不暴露。
真实 gamepad capture backend 未完成，明确 deferred。
Rename 跨域引用更新未完成，默认 disabled。
```

## 12. 结论

正式采用：

```text
B-min+: Unified Input Mapping Workspace
```

它把现有 M10 从“模型和命令已经能改”推进到：

```text
用户能在 Native Editor 中看懂并修改输入，
AI 能按稳定身份安全修改，
冲突和引用影响可审查，
Preview 能用正式 InputResolver 验证，
保存后继续沿现有 RuntimePackage -> ActionSnapshot 链路运行。
```

下一步应先进行方案审查或方案自审确认，再生成 234 当前施工文档；不得直接跳过施工文档开始改代码。

## 13. 参考

本项目：

```text
40-Input-System路线.md
98-Input-Mapping-Asset-C-min方案.md
135-M5-Native-Input-System-v1方案.md
136-M6-RuntimePackage-InputMapping-Resource-Loading-v1方案.md
143-M10-Input-Mapping-Authoring-Runtime-Productization-v1方案.md
191-Authoring-Walkthrough-Missing-Operations-Convergence-v1方案.md
207-ProjectPatch-All-Domain-Capability-v2方案.md
212-Report-Panel-Evidence-Panel-Productization-v1方案.md
220-Editor-GameView-Input-Focus-AUI-HitCandidate-RoutedDispatch-Productization-v1方案.md
227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md
阶段完成记录/2026-07-02-M10-Input-Mapping-Authoring-Runtime-Productization-v1/00-总览.md
```

外部官方资料：

```text
Unity Input Action Assets
https://docs.unity3d.com/Packages/com.unity.inputsystem@1.14/manual/ActionAssets.html

Unreal Engine Enhanced Input
https://dev.epicgames.com/documentation/en-us/unreal-engine/enhanced-input-in-unreal-engine

Godot InputMap
https://docs.godotengine.org/en/stable/classes/class_inputmap.html
```
