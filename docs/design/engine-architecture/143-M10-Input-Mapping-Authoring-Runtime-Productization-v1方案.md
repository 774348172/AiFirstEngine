# M10 Input Mapping Authoring -> Runtime Productization v1 方案

## 1. 系统定义

M10 是输入映射从编辑器创作到 Runtime Player 生效的产品化闭环。

它不是重新做输入底层，也不是重做 RuntimePackage input manifest。M5 / M6 已经确认并落地的主链继续有效：

```text
Platform Window Event
  -> RawInputEvent / InputDeviceState
  -> RuntimeInputFrame
  -> InputMappingAsset
  -> InputResolver
  -> ActionSnapshot
  -> EngineFrameInput
  -> ProjectLogicRunner
```

M10 要补齐的是：

```text
Editor Input Mapping Authoring
  -> Project InputMappingAsset
  -> Validate / Report
  -> Save
  -> Asset Browser / Workspace summary
  -> DesktopExportPipeline
  -> RuntimePackage input manifest
  -> Native Player default mapping
  -> ActionSnapshot
```

第一版只支持 Windows 键盘和鼠标输入映射的编辑、保存、构建和运行验证。

## 2. 已有规则继承

本方案继承以下文档：

```text
40-Input-System路线.md
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
98-Input-Mapping-Asset-C-min方案.md
135-M5-Native-Input-System-v1方案.md
136-M6-RuntimePackage-InputMapping-Resource-Loading-v1方案.md
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
```

继续有效的规则：

```text
Input Mapping 是项目资产。
项目逻辑只读 ActionSnapshot。
项目逻辑不能直接读 keyboard / mouse / RawInputEvent / WindowEvent。
Runtime Player 优先使用 RuntimePackage.default_input_mapping。
engine default mapping 只能作为 fallback 或测试 fixture。
Report 必须能说明 mapping 来源和诊断。
```

## 3. 引擎侧 / 项目侧边界

引擎侧只提供通用能力：

```text
InputMappingAsset
InputActionDefinition
InputContextDefinition
InputBindingDefinition
InputMapping validation
InputMapping authoring command
InputMapping report
RuntimePackage build input collection
ActionSnapshot runtime verification
```

项目侧定义具体语义：

```text
action.move
action.fire
action.pointer
以及任何项目自定义 action id
```

这些 action id 只是字符串数据，不成为引擎 API。

禁止新增项目玩法 API：

```text
Player
Enemy
Bullet
Health
Damage
Score
Wave
Weapon
Boss
Drop
```

## 4. 其他引擎做法

| 引擎 | 对应系统 | 做法 | 对我们的启发 |
|---|---|---|---|
| Unity | InputActionAsset / ActionMap / Binding / PlayerInput | 输入动作和绑定是项目资产，运行时脚本消费 action | 输入必须资产化，编辑器能修改，Runtime 消费 action |
| UE | Enhanced Input: InputAction / InputMappingContext / EnhancedPlayerInput | action 与 mapping context 是资产，运行时 subsystem 激活 context | 输入上下文要显式，不让 gameplay 读平台事件 |
| Godot | InputMap / ProjectSettings input | 项目设置里保存 action 到 event 的映射，运行时查询 action | 第一版心智可以简单清晰，适合 C-min |
| Bevy | ButtonInput / Input events | 核心偏底层，action mapping 常由项目或插件提供 | 底层输入状态与高层 action mapping 要分离 |

我们的路线：

```text
采用 Unity / UE 的 Action-first 长期路线。
采用 Godot-like 简单 InputMap 心智作为第一版复杂度边界。
保留 Bevy 式底层清晰性，但不暴露 raw input 给项目逻辑。
额外用 manifest / report 提升 AI 可读性。
```

## 5. 方案对比

| 方案 | 做法 | 优点 | 缺点 | 结论 |
|---|---|---|---|---|
| A | 继续只用 engine default mapping | 最快 | 用户和 AI 不能编辑项目输入；Player 行为不是项目真相 | 不选 |
| B | 只做一个 JSON 文件编辑器 | 简单 | 容易变成孤立面板，不接 Workspace / Build / Report | 不选 |
| C-min | 完整产品链路，第一版只做键鼠输入映射 | 长期边界正确，AI 可读，复杂度可控 | 需要新增 editor service 和测试 gate | 选择 |
| D | 复制 Unity/UE 完整复杂输入系统 | 能力强 | 第一版规则过多，维护成本高 | 不选 |

## 6. 正式方案

采用 C-min：

```text
InputMappingAuthoringModel
InputMappingAuthoringService
InputMappingCommand
InputMappingAuthoringReport
EditorSession command integration
Workspace Input domain summary
DesktopExportPipeline input mapping collection
RuntimePackage / Player verification tests
```

### 6.1 第一版支持范围

支持：

```text
创建默认 InputMappingAsset
读取项目 InputMappingAsset
保存 InputMappingAsset
添加 / 删除 action
添加 / 删除 binding
修改 binding device_path
验证 unknown action / context / device path
Workspace Input domain 显示 count / default mapping / validation status
Build 时收集 Input/*.json 到 RuntimePackageBuildInput.input_mappings
导出后 RuntimePackage 使用项目 mapping
```

限制：

```text
只支持 keyboard/*、mouse/Left、mouse/Right、mouse/Middle、mouse/Position、mouse/Wheel
只支持 Button / Axis1 / Axis2 / Pointer
只支持 Down / Pressed / Released / Hold / Tap preset
不做 runtime rebinding UI
不做 gamepad / touch / IME
不做复杂 trigger graph
不做多玩家输入 ownership
```

### 6.2 文件约定

第一版项目输入目录：

```text
Input/
  input.default.json
```

文件内容直接使用 `engine_input::InputMappingAsset` JSON。

后续如果需要多个 mapping，可继续扩展为：

```text
Input/
  input.default.json
  input.editor.json
  input.mobile.json
```

但 RuntimePackage 的真相仍由 build 阶段写入：

```text
runtime_package/input/input-manifest.json
runtime_package/input/{mapping_id}.json
manifest.json.input
```

### 6.3 Editor UI Model

新增编辑器可读模型：

```text
InputMappingAuthoringModel
  project_root
  selected_path
  mapping_id
  action_count
  binding_count
  context_count
  validation_status
  diagnostics
  commands
```

新增命令：

```text
CreateDefaultInputMapping { path }
SaveInputMapping { path }
AddInputAction { path, action_id, value_type }
RemoveInputAction { path, action_id }
AddInputBinding { path, context_id, action_id, device_path }
RemoveInputBinding { path, binding_index }
SetInputBindingDevicePath { path, binding_index, device_path }
ValidateInputMapping { path }
```

第一版不要求 Native UI 面板完整可交互，但命令和模型必须稳定，供后续真实面板消费。

### 6.4 Editor Core Service

新增：

```text
editor_core::input_mapping_authoring
```

职责：

```text
load mapping from project path
create default mapping
apply command
validate mapping
save mapping
build authoring report
scan project input mappings
collect runtime package source json
```

不允许：

```text
读取 OS input event
执行 InputResolver runtime resolve
内置项目玩法 action 语义
绕开 engine_input::InputMappingAsset
```

### 6.5 DesktopExport 接入

`DesktopExportPipeline` 构建 RuntimePackageBuildInput 时必须：

```text
扫描 ProjectRoot/Input/*.json
解析 InputMappingAsset
校验 schema 和 validate report
把合法 mapping 写入 RuntimePackageBuildInput.input_mappings
如果没有项目 mapping，保留现有 fallback，但 report 必须能诊断
```

第一版接受 `Input/*.json` 和 `Input/*.input.json`。
`*.input-mapping.json` 继续作为 AssetBrowser 类型识别兼容，但正式写入建议用 `input.default.json`。

### 6.6 Workspace / Report

Workspace Input domain summary：

```text
Input item_count={n} default={id|none} validation={ok|warning|error|missing}
```

Report 重点不是记录每帧输入，而是回答：

```text
项目里有没有 InputMappingAsset
默认 mapping 是哪个
mapping 是否有效
导出包是否使用了项目 mapping
如果 fallback，为什么 fallback
```

## 7. 最小验收场景

### 场景 A：创建并保存默认 mapping

```text
CreateProject
CreateDefaultInputMapping path=Input/input.default.json
SaveInputMapping
AssetBrowser sees Input/input.default.json as InputMapping
Workspace Input domain item_count=1 validation=ok
```

### 场景 B：编辑 binding 后运行验证

```text
AddInputAction action.test Button
AddInputBinding action.test keyboard/T
ValidateInputMapping ok
RuntimeInputFrame KeyDown T
InputResolver outputs action.test
```

### 场景 C：导出后 Player 使用项目 mapping

```text
Project Input/input.default.json maps keyboard/T -> action.test
DesktopExportPipeline builds RuntimePackage
load_runtime_package reads default_input_mapping=input.default
WindowedPlayer headless input uses package mapping
Report mapping source is runtime-package
```

## 8. 测试要求

模块测试：

```powershell
cargo test -p editor_ui_model input_mapping_authoring
cargo test -p editor_core input_mapping_authoring
cargo test -p editor_core editor_session_input_mapping
cargo test -p editor_core desktop_export_input_mapping
cargo test -p engine_runtime runtime_package_loader_reads_project_default_input_mapping
cargo test -p runtime_player_winit input_mapping
```

整体回归：

```powershell
cargo fmt --check
cargo test -p engine_input
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

## 9. 方案自审

### 9.1 Specification fit

本方案满足“选择一个大系统继续推进复杂打飞机闭环”的要求，选定 M10，并覆盖编辑器输入资产创作到 Runtime Player 生效的完整链路。

### 9.2 Rule fit

方案继承 M5 / M6 / 98，不重复实现输入底层，不把项目玩法术语做成引擎 API。

### 9.3 Textual consistency

文档中 M10 边界一致：Editor authoring / validation / build input collection / runtime verification。Raw input、Runtime resolver、Window event 均不属于本次重做范围。

### 9.4 Design fit

符合 AI-first：InputMappingAsset、AuthoringModel、Report 都是结构化数据。符合复杂项目：后续多 mapping、多平台、多设备可以在同一资产与 manifest 体系扩展。

### 9.5 Implementation feasibility

现有代码已有 `engine_input::InputMappingAsset`、RuntimePackage input manifest、RuntimePackage loader、Runtime Player package mapping 优先级。M10 可以增量落地。

### 9.6 Practical reasonableness

第一版只支持键鼠、少量 action value 和 trigger preset，避免复制 Unity/UE 完整输入系统。测试 gate 清晰，可分模块施工。

结论：

```text
方案通过自审，可以进入施工文档。
```
