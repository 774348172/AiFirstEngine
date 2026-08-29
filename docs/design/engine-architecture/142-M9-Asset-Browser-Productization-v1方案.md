# 142-M9 Asset Browser Productization v1 方案

## 1. 本文解决什么

本文定义 `M9 Asset Browser 产品化`。

它不是重新实现 Asset DB / Importer / RuntimeAssetIndex，也不是只美化现有 ProjectBrowser。

M9 要补齐的是 Unity Project Browser / UE Content Browser / Godot FileSystemDock 这一类编辑器资产中心：

```text
Project files / Asset DB / Importer records
  -> AssetBrowserIndex
  -> AssetBrowserModel
  -> Search / Filter / Selection / Preview
  -> AssetPicker / DragPayload / Open / Place / Assign
  -> EditorSession Transaction
  -> Scene / Prefab / Inspector / Build
```

采用：

```text
C-architecture
B-implementation
```

含义：

```text
长期边界按 Unity / UE 级资产中心设计。
第一版实现 Godot-like 简洁可用路径。
后续扩展缩略图缓存、复杂搜索、依赖图、批量操作时，不推翻 AssetBrowserModel / Command / Report。
```

## 2. 引擎边界

Asset Browser 是编辑器底座能力，只处理：

```text
asset
folder
path
kind
query
filter
selection
preview
pick
drag
command
diagnostic
report
```

不允许把以下项目侧概念做成 Asset Browser 内置 API：

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

项目侧资产语义通过：

```text
Project Schema
Project Asset Metadata
Prefab
Scene
Rule
AUI
Input Mapping
```

表达。

## 3. 其它引擎对应模块

### 3.1 Unity

Unity 对应模块：

```text
ProjectBrowser
ObjectSelector
AssetDatabase
SearchFilter
Selection
DragAndDrop
```

本地源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\ProjectBrowser\ProjectBrowser.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\ObjectSelector.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\Search\ObjectSelectorSearch.cs
```

可借鉴：

```text
ProjectBrowser 负责浏览、搜索、选择、拖拽。
ObjectSelector 负责 Inspector 字段选择资源。
SearchFilter 是可序列化的查询状态。
Selection 与 ProjectBrowser 交互，但操作仍走编辑器命令。
```

不照搬：

```text
Unity AssetDatabase 很多 native 黑盒，不适合 AI-first 调试。
我们的 AssetBrowserReport 必须结构化、可序列化、AI 可读。
```

### 3.2 Unreal Engine

UE 对应模块：

```text
ContentBrowser
ContentBrowserData
SAssetPicker
AssetView
AssetRegistry
AssetContextMenu
```

本地源码参考：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\ContentBrowser
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\ContentBrowserData
```

可借鉴：

```text
Content Browser 是完整资产中心。
AssetPicker 与主资产浏览器共用数据能力。
Asset Registry / ContentBrowserData 与 UI 分离。
右键菜单和批量操作不应该散落在 UI 代码里。
```

不照搬：

```text
UE Content Browser 很重，第一版不做完整依赖图、复杂菜单和高级搜索 DSL。
```

### 3.3 Godot

Godot 对应模块：

```text
FileSystemDock
EditorFileSystem
EditorResourcePreview
EditorFileDialog
```

本地源码参考：

```text
<GODOT_SOURCE>\godot-master\godot-master\editor\docks\filesystem_dock.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\editor_interface.cpp
```

可借鉴：

```text
目录树 + 文件列表 + 搜索 + 选择 + 预览 + 当前路径。
扫描状态和资源预览是 reportable 状态。
第一版心智简单，适合我们 C-architecture / B-implementation 的落地方式。
```

不照搬：

```text
Godot 以文件系统为中心。
我们必须把 Asset DB / Importer / AssetRef / RuntimePackage 也纳入同一资产视图。
```

### 3.4 Bevy

Bevy 没有成熟官方编辑器资产浏览器，但对应运行时资产基础是：

```text
AssetServer
AssetPath
Handle<T>
Assets<T>
AssetLoader
```

本地源码参考：

```text
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_asset\src\server\mod.rs
<BEVY_SOURCE>\bevy-main\bevy-main\crates\bevy_asset\src\handle.rs
```

可借鉴：

```text
AssetRef / Handle / AssetPath 的职责分离。
Asset load state 和 load failure 是一等诊断对象。
```

不照搬：

```text
Bevy 不提供 Unity / UE 级编辑器资产工作流。
```

## 4. 方案对比

### 4.1 方案 A：继续扩展 ProjectBrowser

优点：

```text
最快。
现有 UI 和命令改动少。
```

缺点：

```text
ProjectBrowser 会同时承担文件浏览、资产选择、拖拽、预览、报告、字段选择。
后期 Scene / Prefab / Rule / AUI / Input 都塞进一个旧模型，难维护。
AI 看到的仍是文件列表，不是资产语义。
```

不推荐作为长期路线。

### 4.2 方案 B：Asset Browser C-min

优点：

```text
新建 AssetBrowserModel / AssetCommand / AssetReport。
复用现有 ProjectBrowser 和 AssetPlacement，不重做底层。
第一版可控。
```

缺点：

```text
如果只按 B 做，后续完整 Content Browser 可能还要补 AssetPicker / DragPayload / Report slot。
```

可作为施工强度，但架构边界不够完整。

### 4.3 方案 C：完整 Content Browser

优点：

```text
最接近 Unity / UE。
长期复杂项目能力最强。
```

缺点：

```text
第一版过大。
缩略图缓存、依赖图、批量操作、高级搜索、版本控制会拖慢主线。
```

不建议第一版完整实现。

## 5. 推荐方案

采用：

```text
C-architecture / B-implementation
```

第一版必须建立长期核心层：

```text
AssetBrowserModel
AssetBrowserEntry
AssetKind
AssetQuery
AssetSelection
AssetPreviewDescriptor
AssetPickRequest / AssetPickResult
AssetDragPayload
AssetBrowserCommand
AssetBrowserReport
```

第一版只实现：

```text
项目资产树 / 列表
搜索 / 类型过滤
单选 / 多选模型
AssetRef 选择器模型
拖拽 payload 模型
打开资产命令
放入场景命令
Inspector 字段引用资产所需 pick result
资产状态报告
```

第一版不实现：

```text
完整缩略图 GPU 缓存
复杂标签系统
完整依赖图
批量重命名依赖修复
高级搜索 DSL
外部版本控制集成
资源商店 / 外部库
```

## 6. 核心数据结构

### 6.1 AssetKind

```text
AssetKind:
  Folder
  Scene
  Prefab
  Texture
  Sprite
  Material
  Rule
  Aui
  InputMapping
  Audio
  Unknown
```

规则：

```text
AssetKind 是编辑器资产分类，不是运行时组件类型。
未知资源必须显示为 Unknown，并进入 report，而不是静默丢弃。
```

### 6.2 AssetBrowserEntry

```text
AssetBrowserEntry:
  asset_id
  guid
  path
  label
  kind
  exists
  imported
  openable
  placeable
  selectable
  selected
  preview
```

规则：

```text
Entry 是视图对象，不是真相层。
Asset DB / Importer / Project file 是来源。
```

### 6.3 AssetQuery

```text
AssetQuery:
  search_text
  folder
  kinds[]
  include_missing
  include_unimported
```

规则：

```text
第一版只做简单 contains 搜索和 kind 过滤。
高级 DSL 后续再加，不进入 v1。
```

### 6.4 AssetSelection

```text
AssetSelection:
  selected_paths[]
  primary_path
  primary_asset_id
```

规则：

```text
选择状态属于编辑器 UI/session。
选择不直接修改资产。
修改必须走 AssetBrowserCommand。
```

### 6.5 AssetPreviewDescriptor

```text
AssetPreviewDescriptor:
  preview_kind
  text
  thumbnail_asset_id
  status
```

规则：

```text
第一版只提供 preview descriptor，不做真实 GPU 缩略图缓存。
Texture/Sprite 可以给 thumbnail_asset_id。
Rule/AUI/Input 可以给 text summary。
```

### 6.6 AssetPickRequest / AssetPickResult

```text
AssetPickRequest:
  request_id
  allowed_kinds[]
  target_path
  target_field_path
```

```text
AssetPickResult:
  request_id
  asset_ref
  accepted
  diagnostics[]
```

规则：

```text
它对应 Unity ObjectSelector / UE AssetPicker。
Inspector AssetRef 字段不得自己扫描项目文件。
```

### 6.7 AssetDragPayload

```text
AssetDragPayload:
  asset_refs[]
  source_panel
  allowed_drop_targets[]
```

规则：

```text
拖拽只携带结构化 payload。
落点决定转换成 PlaceAssetIntoScene / AssignAssetRef / OpenAsset 等命令。
```

### 6.8 AssetBrowserCommand

```text
AssetBrowserCommand:
  Select
  Open
  Pick
  PlaceIntoScene
  Refresh
```

规则：

```text
Asset Browser 不直接改 Scene / Prefab / Inspector。
命令必须进入 EditorSession / Transaction。
```

### 6.9 AssetBrowserReport

```text
AssetBrowserReport:
  schema_version
  asset_count
  folder_count
  selected_count
  missing_count
  unimported_count
  filtered_count
  diagnostics[]
```

诊断码：

```text
asset_missing
asset_unimported
asset_type_mismatch
asset_open_unsupported
asset_place_unsupported
asset_pick_rejected
asset_scan_failed
```

## 7. 标准流程

### 7.1 浏览流程

```text
ProjectSession
  -> Project files + Asset DB records
  -> AssetBrowserIndex
  -> AssetQuery apply
  -> AssetBrowserModel
  -> UI render
```

### 7.2 选择流程

```text
User select entry
  -> AssetBrowserCommand::Select
  -> EditorSession transaction
  -> AssetSelection update
  -> AssetBrowserModel rebuild
```

### 7.3 打开流程

```text
User open asset
  -> AssetBrowserCommand::Open
  -> EditorSession route
      Scene -> OpenSceneDocument
      Prefab -> future Prefab editor
      Rule -> future Rule editor
      AUI -> future AUI editor
      InputMapping -> future Input editor
      Unknown -> diagnostic
```

### 7.4 放入场景流程

```text
Drag asset to Scene / Place command
  -> AssetDragPayload / AssetBrowserCommand::PlaceIntoScene
  -> existing AssetPlacementResolver
  -> SceneEditCommand
  -> EditorSession transaction
```

### 7.5 Inspector 字段选资源流程

```text
Inspector AssetRef field
  -> AssetPickRequest
  -> AssetBrowserModel filtered by allowed_kinds
  -> AssetPickResult
  -> PropertyHandle / TransactionRouter
```

## 8. 与现有系统关系

### 8.1 ProjectBrowserModel

现有 `ProjectBrowserModel` 不删除。

第一版可以：

```text
保留 ProjectBrowserModel 作为 UI 兼容层。
新增 AssetBrowserModel 作为长期语义层。
ProjectBrowserModel 可以由 AssetBrowserModel 投影生成。
```

### 8.2 AssetPlacementResolver

继续复用：

```text
AssetPlacementResolver
AssetPlacementRequest
AssetPlacementReport
```

M9 只负责生成结构化 asset payload / command，不重新实现场景放置。

### 8.3 Inspector / M8

M8 已完成 `PropertyHandle / AssetFilter`。

M9 要补：

```text
AssetPickRequest / AssetPickResult
AssetRef picker model
```

后续 Inspector 的 AssetRef 字段通过 Asset Browser 选择资产。

### 8.4 Build / RuntimePackage

M9 不负责 cook 和 runtime load。

M9 只显示：

```text
imported / missing / unimported / build relevant status
```

真实打包仍由 RuntimePackageBuilder / DesktopExportPipeline 负责。

## 9. 第一版施工边界

必须完成：

```text
A1 AssetKind / AssetBrowserEntry / AssetBrowserModel / AssetQuery / AssetSelection
A2 AssetBrowserIndex 从项目目录和现有 ProjectBrowser 数据构建
A3 搜索 / kind 过滤 / missing-unimported report
A4 AssetPickRequest / AssetPickResult
A5 AssetDragPayload / PlaceIntoScene 命令适配
A6 EditorSession 最小接入：build_asset_browser_model / select / open / place
A7 ProjectBrowserModel 兼容投影
A8 AssetBrowserReport
A9 模块测试和整体回归
```

不做：

```text
真实 GPU 缩略图缓存
完整拖拽 UI 手势
复杂资产右键菜单
批量重命名依赖修复
完整 Asset dependency graph
```

## 10. 验收测试

必须有：

```text
AssetBrowserIndex scans project folders.
AssetQuery filters by text and kind.
AssetSelection tracks primary and multi selection.
AssetPickRequest rejects type mismatch.
AssetDragPayload can convert placeable asset to PlaceAssetIntoScene.
AssetBrowserReport counts missing / unimported / filtered assets.
EditorSession builds AssetBrowserModel after project open.
EditorSession select asset updates selection.
EditorSession place asset still routes through AssetPlacementResolver.
ProjectBrowserModel remains compatible.
No project gameplay terms appear in engine Asset Browser API.
```

整体回归：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

## 11. 方案自审

### 11.1 Specification fit

本文满足 M9 Asset Browser 产品化，不是零散 ProjectBrowser 美化，也不是重做 Asset DB / Importer。

### 11.2 Rule fit

本文遵守：

```text
先对齐 130 缺失能力清单。
不加入项目玩法 API。
对比 Unity / UE / Godot / Bevy。
优先 AI 友好、复杂项目、维护性、简单度、效率。
```

### 11.3 Textual consistency

术语一致：

```text
AssetBrowserModel 是编辑器资产视图。
AssetBrowserEntry 是视图条目。
AssetQuery 是过滤条件。
AssetSelection 是会话选择。
AssetBrowserCommand 是操作入口。
AssetBrowserReport 是诊断结果。
Asset DB / Importer / RuntimePackage 不是 M9 的真相层。
```

### 11.4 Design fit

方案符合长期路线：

```text
接近 Unity ProjectBrowser/ObjectSelector 和 UE ContentBrowser/AssetPicker。
第一版实现采用 Godot FileSystemDock 的简单心智。
AI 可读、可诊断、可回放。
```

### 11.5 Implementation feasibility

当前已有：

```text
ProjectBrowserModel
ProjectLauncher / ProjectSession
AssetPlacementResolver
AssetPipelineState / ProjectAssetRecord
Inspector AssetFilter
EditorSession transaction
```

可以增量实现。

### 11.6 Practical reasonableness

第一版只做资产浏览、选择、过滤、picker、payload、report，不做完整 Content Browser 高级功能，施工可控。

结论：

```text
方案通过自审，可以生成施工文档并开工。
```
