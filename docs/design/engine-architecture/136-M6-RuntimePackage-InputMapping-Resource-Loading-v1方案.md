# 136-M6 RuntimePackage InputMapping Resource Loading v1 方案

## 1. 系统定义

M6 是 RuntimePackage InputMapping Resource Loading -> Player Default Mapping。

它要解决的问题不是重新做输入系统，而是把项目里的 `InputMappingAsset` 从 RuntimePackage 正式加载到 Player，使真实桌面 Player 使用项目输入映射，而不是继续依赖引擎内置的 `gameplay_default()`。

目标链路：

```text
Project InputMappingAsset
  -> Build Pipeline
  -> RuntimePackage input manifest
  -> RuntimePackage Loader
  -> default InputMappingAsset
  -> Runtime Player / Windowed Player
  -> InputResolver
  -> ActionSnapshot
  -> EngineFrameInput
```

M6 的第一版只做一件事：项目默认 InputMapping 进入 Runtime Player。

第一版不做：

```text
Input Mapping 编辑器 UI
Runtime rebinding UI
Gamepad / Touch / IME
多玩家输入所有权
复杂 Trigger 图
运行时动态切换多个 Input Context
```

这些能力后续可以在同一套 manifest 结构上扩展，不能在 M6 内临时增加项目规则。

## 2. 已有规则继承

本方案继承：

```text
40-Input-System路线.md
74-Native-Editor-Viewport输入回流RuntimeFrame方案.md
98-Input-Mapping-Asset-C-min方案.md
106-Build-Runtime-Package-Completion-C-min方案.md
135-M5-Native-Input-System-v1方案.md
```

继续有效的规则：

```text
Input Mapping 属于项目资源。
RawInputEvent / RuntimeInputFrame 属于引擎输入层。
项目逻辑只读取 ActionSnapshot。
InputResolver 读取 InputMappingAsset，把 RuntimeInputFrame 转为 ActionSnapshot。
EngineHostLoop 每帧接收 EngineFrameInput。
引擎不为具体游戏类型增加输入规则。
```

M6 只补齐 RuntimePackage 到 Runtime Player 的资源加载闭环。

## 3. 当前代码缺口

当前实现已经具备：

```text
engine_input:
  InputMappingAsset
  InputResolver
  ActionSnapshot
  InputDeviceState

runtime_package_builder:
  RuntimePackageBuildInput.input_mappings
  写入 package/input/{mapping.id}.json

runtime_player_winit:
  RawInputEvent -> InputDeviceState -> RuntimeInputFrame -> InputResolver -> ActionSnapshot
```

当前缺口：

```text
RuntimePackageManifest 没有 input manifest 索引。
load_runtime_package 没有读取 package/input 下的 InputMappingAsset。
runtime_player_winit 仍然使用 InputMappingAsset::gameplay_default()。
Report 没有区分 project mapping 与 engine default fallback。
```

因此 M6 不是增加一个新输入系统，而是替换 Player 的默认 mapping 来源。

## 4. 参考引擎结论

### 4.1 Unreal Engine

UE Enhanced Input 的核心结构是：

```text
UInputAction
UInputMappingContext
UEnhancedInputLocalPlayerSubsystem
UEnhancedPlayerInput
```

关键源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\EnhancedInput\Source\EnhancedInput\Private\EnhancedInputSubsystems.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\EnhancedInput\Source\EnhancedInput\Private\InputMappingContext.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Plugins\EnhancedInput\Source\EnhancedInput\Public\EnhancedInputSubsystemInterface.h
```

UE 的路线是：

```text
InputAction / InputMappingContext 是项目资产。
运行时通过 Subsystem 添加 MappingContext。
默认 MappingContext 可以来自 DeveloperSettings。
运行时 PlayerInput 消费已经应用的 MappingContext。
```

对我们的启发：

```text
项目输入映射必须是项目资产。
运行时必须显式知道当前应用了哪些 mapping。
默认 mapping 可以存在，但不能隐藏在窗口层硬编码。
```

### 4.2 Unity

Unity 新 Input System 使用：

```text
InputActionAsset
InputActionMap
InputAction
PlayerInput
```

公开资料参考：

```text
https://docs.unity3d.com/Packages/com.unity.inputsystem@1.12/manual/Actions.html
```

UnityCsReference 中输入后端相关参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\Input
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\InputForUI
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Inspector\PlayerSettingsEditor\PlayerSettingsEditor.cs
```

Unity 的路线是：

```text
输入动作和映射作为项目资产或项目设置存在。
Player 运行时读取项目资产。
脚本层消费 action，而不是直接写 OS 输入。
```

对我们的启发：

```text
InputMappingAsset 应该随项目进入构建产物。
Player 不应该依赖引擎内置固定 mapping。
```

### 4.3 Godot

Godot 的输入映射是 `InputMap`。

关键源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\main\main.cpp
<GODOT_SOURCE>\godot-master\godot-master\core\input\input_map.cpp
```

Godot 启动时会从 ProjectSettings 的 `input/*` 加载项目输入：

```text
InputMap::load_from_project_settings()
```

对我们的启发：

```text
Godot 证明固定项目配置也能工作。
但纯约定路径会弱于 manifest，对 AI 追踪和复杂项目扩展不够友好。
```

### 4.4 Bevy

Bevy 核心输入层主要提供：

```text
ButtonInput<KeyCode>
ButtonInput<MouseButton>
KeyboardInput / MouseButtonInput event
InputPlugin
```

关键源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_input\src\lib.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_input\src\keyboard.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_input\src\mouse.rs
```

Bevy 的核心层偏底层，高层 Action Mapping 通常由项目或插件提供。

对我们的启发：

```text
底层输入状态和高层 action mapping 应该分离。
我们的 engine_input 已经完成这个分离。
M6 只需要把项目 mapping 装配进 runtime。
```

## 5. 方案选择

### 方案 A：继续硬编码 gameplay_default

```text
runtime_player_winit 继续使用 InputMappingAsset::gameplay_default()
```

优点：

```text
实现最简单。
测试方便。
```

缺点：

```text
不是项目真实输入。
用户修改 InputMappingAsset 不会影响 Player。
AI 无法根据项目资源判断运行时输入行为。
与 Unity / UE 的正式项目路线不一致。
```

结论：

```text
只允许作为 fallback / test fixture，不能作为正式 runtime 真相。
```

### 方案 B：固定路径 input/default.json

```text
RuntimePackage 约定读取 input/default.json。
没有该文件时 fallback 到 gameplay_default。
```

优点：

```text
实现简单。
接近 Godot 的项目配置读取方式。
```

缺点：

```text
隐藏约定较重。
不利于多个 mapping / 多平台 profile / 后续 context。
Report 不容易说明全部 mapping 状态。
AI 需要猜路径规则。
```

结论：

```text
适合小 demo，不适合长期路线。
```

### 方案 C：Manifest 索引 RuntimePackage InputMapping

```text
RuntimePackageManifest 显式记录 input manifest。
input manifest 显式记录 defaultMappingId 和 mapping 文件路径。
RuntimePackage Loader 读取并验证 InputMappingAsset。
Player 默认使用 package.default_input_mapping。
缺失或损坏时才显式 fallback。
```

优点：

```text
最接近 UE / Unity 的项目输入资产路线。
比 UE / Unity 更 AI 友好，因为 manifest 是可读真相层。
支持复杂项目后续扩展。
没有把输入规则写死在窗口层。
Report 可以直接解释当前输入来源。
```

缺点：

```text
比固定路径多一个 input manifest。
第一版需要补 RuntimePackage schema 和 loader。
```

结论：

```text
采用方案 C-min。
```

## 6. M6 正式规则

### 6.1 InputMapping 的真相层

M6 后，正式 Player 的 InputMapping 真相层是 RuntimePackage 内的 project InputMappingAsset。

```text
RuntimePackage input manifest
  -> InputMappingAsset
  -> RuntimePackage.default_input_mapping
```

`InputMappingAsset::gameplay_default()` 只能用于：

```text
测试 fixture
缺失项目 mapping 时的 fallback
临时 sample 构造
```

它不能再作为正式 Player 的默认真相。

### 6.2 RuntimePackage Manifest 结构

`RuntimePackageManifest` 增加 input 索引：

```json
{
  "input": {
    "path": "input/input-manifest.json",
    "defaultMappingId": "input.default",
    "mappingCount": 1
  }
}
```

第一版字段保持少而精：

```text
path: input manifest 相对 RuntimePackage 根目录路径
defaultMappingId: Player 默认使用的 mapping id
mappingCount: 构建和报告用摘要字段
```

第一版不在 RuntimePackageManifest 里直接列出所有 mapping，避免顶层 manifest 膨胀。

### 6.3 Input Manifest 结构

新增 `input/input-manifest.json`：

```json
{
  "schemaVersion": "runtime-input-manifest.v1",
  "defaultMappingId": "input.default",
  "mappings": [
    {
      "id": "input.default",
      "path": "input/input.default.json",
      "enabled": true
    }
  ]
}
```

第一版字段：

```text
schemaVersion: runtime-input-manifest.v1
defaultMappingId: 默认 mapping id
mappings[].id: mapping id
mappings[].path: mapping 文件路径
mappings[].enabled: 是否参与 runtime package
```

第一版只要求一个默认 mapping 可用。

允许打包多个 mapping，但 Player 第一版只激活 default mapping。

### 6.4 Build Pipeline 规则

Build Pipeline 必须：

```text
1. 收集项目 InputMappingAsset。
2. 写入 package/input/{id}.json。
3. 写入 package/input/input-manifest.json。
4. 在 RuntimePackageManifest.input 中记录 input manifest 摘要。
5. 验证 defaultMappingId 指向的 mapping 存在且 enabled。
6. 验证 default mapping 自身 validate 通过。
```

如果项目没有显式 InputMapping：

```text
第一版允许 Build 写入 engine default fallback mapping。
但必须在 report 中标记 source = engine-default-fallback。
```

长期规则：

```text
正式项目发布前应该拥有项目 InputMapping。
engine default fallback 只用于让空项目能跑起来，不能掩盖项目配置缺失。
```

### 6.5 Runtime Loader 规则

`load_runtime_package` 必须：

```text
1. 读取 RuntimePackageManifest.input。
2. 读取 input/input-manifest.json。
3. 读取所有 enabled InputMappingAsset。
4. 找到 defaultMappingId 对应的 mapping。
5. 调用 InputMappingAsset.validate。
6. 写入 RuntimePackage.default_input_mapping。
7. 产生 RuntimeDiagnostics。
```

加载失败处理：

```text
input manifest 缺失: warning + fallback
defaultMappingId 缺失: warning + fallback
default mapping 文件缺失: warning + fallback
default mapping validate 失败: error 或 warning，第一版建议 error
fallback mapping 自身失败: error，Player 输入不可用
```

第一版建议：

```text
开发 / 编辑器验证模式：缺失项目 mapping 为 warning。
正式导出模式：缺失项目 mapping 可提升为 error，由 Build Profile 决定。
```

M6 不新增复杂策略系统，只预留 severity 字段。

### 6.6 Runtime Player 规则

Runtime Player 选择 mapping 的优先级：

```text
1. RuntimePackage.default_input_mapping
2. engine_input::InputMappingAsset::gameplay_default fallback
```

Player 不允许直接扫描 `input/*.json` 猜测默认 mapping。

Player 不允许直接构造项目 mapping。

Player 只消费 RuntimePackage Loader 已经解析好的 mapping。

### 6.7 Report / Diagnostics 规则

NativeWindowHostReport / Runtime report 必须能说明：

```text
input.mappingSource:
  runtime-package
  engine-default-fallback

input.mappingId:
  input.default

input.mappingPath:
  input/input.default.json

input.mappingStatus:
  ok
  missing
  invalid
  fallback

input.diagnostics:
  code
  severity
  message
  path
```

Report 的目标不是记录每一帧输入细节，而是让用户和 AI 能快速判断：

```text
当前 Player 到底用了哪个输入映射。
项目 mapping 有没有进入包。
如果没进入，为什么 fallback。
```

### 6.8 Editor Authoring Workspace 规则

M6 第一版只要求 Editor Authoring Workspace 能提供摘要，不要求完整 Input Mapping Editor。

摘要字段：

```text
defaultMappingId
mappingCount
validationStatus
lastBuildInputMappingSource
```

这用于 Project Workspace / Build 面板显示当前输入映射状态。

完整 Input Mapping 编辑器属于后续系统，不在 M6 内展开。

## 7. 数据结构建议

### 7.1 RuntimePackageManifest

```rust
pub struct RuntimePackageManifest {
    pub schema_version: String,
    pub package_mode: String,
    pub project: RuntimeProjectInfo,
    pub active_scene_id: String,
    pub scenes: Vec<RuntimeSceneManifestEntry>,
    pub assets: RuntimeManifestAssetIndex,
    pub rules: RuntimeManifestRuleIndex,
    pub input: RuntimeManifestInputIndex,
    pub content_hash: Option<String>,
}

pub struct RuntimeManifestInputIndex {
    pub path: String,
    pub default_mapping_id: String,
    pub mapping_count: usize,
}
```

### 7.2 RuntimeInputManifest

```rust
pub const RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION: &str = "runtime-input-manifest.v1";

pub struct RuntimeInputManifest {
    pub schema_version: String,
    pub default_mapping_id: String,
    pub mappings: Vec<RuntimeInputMappingManifestEntry>,
}

pub struct RuntimeInputMappingManifestEntry {
    pub id: String,
    pub path: String,
    pub enabled: bool,
}
```

### 7.3 RuntimePackage

```rust
pub struct RuntimePackage {
    pub package_dir: PathBuf,
    pub manifest: RuntimePackageManifest,
    pub active_scene: RuntimeScene,
    pub assets: RuntimeAssetManifest,
    pub runtime_asset_index: RuntimeAssetIndex,
    pub runtime_asset_mount_table: RuntimePackageMountTable,
    pub rules: RuntimeRuleManifest,
    pub input_manifest: RuntimeInputManifest,
    pub input_mappings: Vec<InputMappingAsset>,
    pub default_input_mapping: Option<InputMappingAsset>,
}
```

第一版可以为了减少 clone 使用 owned value。后续如有性能需要，再改为 `Arc<InputMappingAsset>`。

## 8. 与其他引擎对比

| 项目 | UE | Unity | Godot | Bevy | 我们 M6 |
|---|---|---|---|---|---|
| 输入映射来源 | InputMappingContext 资产 / DeveloperSettings | InputActionAsset / Project-wide Actions | ProjectSettings input/* | 核心只给底层 input resource | RuntimePackage InputMappingAsset |
| 默认 mapping | 默认 MappingContext 可配置 | 项目 ActionAsset / PlayerInput | ProjectSettings 加载 | 项目或插件自管 | input manifest defaultMappingId |
| 运行时消费 | EnhancedPlayerInput | Input System / PlayerInput | InputMap 查询 | ButtonInput 资源 | InputResolver -> ActionSnapshot |
| AI 可读性 | 弱，资产链路复杂 | 中，资产可读但运行链路较黑盒 | 中，配置清楚 | 中，代码配置多 | 强，manifest + report 显式 |
| 复杂项目扩展 | 很强 | 强 | 中 | 依赖项目架构 | C-min 结构可扩展 |
| 第一版复杂度 | 已成熟 | 已成熟 | 简单 | 简单底层 | 中等但可控 |

结论：

```text
M6 采用 UE / Unity 的项目输入资产路线。
同时用 manifest/report 把运行时真相显式化，提升 AI 可读和可调试能力。
```

## 9. 测试要求

M6 施工必须至少覆盖：

### 9.1 项目 mapping 生效

```text
构建 RuntimePackage，包含 input.default。
Player 加载 RuntimePackage。
模拟 Space / KeyA 等输入。
ActionSnapshot 使用项目 mapping 输出 action。
Report mappingSource = runtime-package。
```

### 9.2 缺失项目 mapping fallback

```text
构建没有 input mapping 的 RuntimePackage。
Runtime Loader fallback 到 gameplay_default。
Report mappingSource = engine-default-fallback。
Diagnostics 包含 warning。
```

### 9.3 mapping 文件损坏

```text
input manifest 指向不存在或非法 JSON。
Runtime Loader 产生 diagnostics。
Player 不静默吞掉错误。
```

### 9.4 Player 不再硬编码默认 mapping

```text
构造一个与 gameplay_default 不同的 mapping。
Player 必须使用 RuntimePackage mapping。
如果仍输出 gameplay_default 行为，测试失败。
```

## 10. 边界确认

M6 完成后，输入相关系统边界是：

```text
M5:
  Native OS / Window input -> RuntimeInputFrame -> ActionSnapshot 的输入运行链路。

M6:
  RuntimePackage 中的项目 InputMappingAsset -> Player default mapping。

后续 Input Mapping Authoring:
  编辑器里创建、修改、验证 InputMappingAsset。

后续 Rebinding:
  运行时玩家改键、保存用户 profile。
```

不要把后续 Authoring / Rebinding / 多设备策略塞进 M6。

M6 的成功标准很简单：

```text
Player 默认输入映射来自 RuntimePackage 项目资源。
fallback 是显式、可报告、可诊断的异常路径。
AI 能从 package manifest 和 report 看懂输入来源。
```
