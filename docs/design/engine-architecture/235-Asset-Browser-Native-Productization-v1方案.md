# 235-Asset Browser Native Productization v1 方案

> 状态：正式方案。  
> 方案日期：2026-07-10。  
> 采用方案：`B-min+: Unified Native Asset Browser`。  
> 前置方案：`142-M9-Asset-Browser-Productization-v1方案.md`。  
> 目标：让用户和 AI 不修改 JSON，也能在 Native Editor 中真实浏览、预览、拖拽、选择和替换项目资产引用。

## 1. 这个系统是干什么的

直白地说，本系统把当前“代码能扫描资产”推进为“用户真的能在编辑器里使用资产”。

完成后，用户可以：

```text
浏览项目资产目录。
搜索并按类型过滤资产。
看到图片资产的真实缩略图和选中预览。
打开 Scene / Prefab / Rule / AUI / Input Mapping。
把 Prefab / Texture / Sprite 等资产拖入 Scene。
从 Inspector / AUI AssetRef 字段打开同一个资产选择器。
选择另一个同类型资产，替换当前字段引用。
保存、重开和 Build 后继续得到相同结果。
```

本系统在其它引擎中的对标是：

```text
Unity Project Browser + ObjectSelector
Unreal Content Browser + Asset Picker
Godot FileSystemDock + EditorFileSystem + EditorResourcePreview
```

它在本引擎中的作用是：

```text
Project Authoring Assets
  -> Cached AssetBrowserIndex
  -> AssetBrowserModel
  -> Native Asset Browser / Asset Picker
  -> EditorSession Transaction
  -> Scene / Prefab / AUI / Rule / Input authoring
  -> Save / RuntimePackage Build
```

## 2. 与 142 的关系

`142-M9-Asset-Browser-Productization-v1方案.md` 已完成数据与服务层第一版：

```text
AssetBrowserModel
AssetBrowserEntry
AssetQuery
AssetSelection
AssetPickRequest / AssetPickResult
AssetDragPayload
AssetBrowserCommand
AssetBrowserReport
AssetBrowserIndex / AssetBrowserService
```

235 不重做这些类型，也不新增第二套资产浏览架构。

235 只收敛 142 完成记录中明确留下的产品化缺口：

```text
Native Editor 尚未直接消费 AssetBrowserModel。
当前 UI 仍使用 ProjectBrowserModel 兼容投影。
当前面板最多显示少量文本行。
AssetPick / DragPayload 尚未形成真实交互。
缩略图仍是描述符，没有真实 GPU present。
AssetBrowserReport 尚未进入统一 Report Panel。
```

长期关系：

```text
AssetBrowserModel = 正式编辑器资产视图模型。
ProjectBrowserModel = 临时兼容投影，不再作为 Native UI 主输入。
项目资产文件 / typed authoring document = 唯一资产真相。
AssetBrowserIndexSnapshot = editor-only 可丢弃缓存，不是第二真相。
RuntimePackage / RuntimeAssetIndex = 运行时真相，不是编辑真相。
```

## 3. 当前实现问题

当前 Rust 基线存在以下真实问题：

```text
EditorUiModel 只暴露 project_browser，没有暴露完整 AssetBrowserModel。
Native ProjectBrowser 面板最多绘制 5 个 entry。
Editor frame 每次 compose UI model 时都会执行 AssetBrowserIndex::build。
AssetBrowserIndex::build 会递归扫描已知项目目录。
asset_id 主要由 path 派生，guid 没有真实接入。
imported 状态基本等于文件是否存在，不代表真实 importer 状态。
AssetBrowserCommand 没有完整降低为 Native hit target / UiCommandPayload。
Inspector 的 AssetRefPicker 只有编辑器种类定义，没有真实 picker 交互。
DrawCommand 没有通用 editor image/thumbnail present 能力。
```

如果只把当前列表画大，会得到一个“外观看起来像 Asset Browser、内部仍每帧扫盘”的系统，不能适配复杂项目。

## 4. 其它引擎源码参考

### 4.1 Unity

本地源码：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\ProjectBrowser\ProjectBrowser.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\ObjectSelector.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\AssetDatabase\Editor\ScriptBindings\AssetDatabase.bindings.cs
```

关键实现：

```text
ProjectBrowser.Init() 创建 SearchFilter、folder tree、ObjectListArea 和持久 UI state。
ObjectListArea 支持 selection、multi-select、drag、rename、keyboard navigation 和 grid size。
ProjectBrowser.InitListArea() 使用 SearchFilter 查询 AssetDatabase，并初始化选择。
ProjectBrowser.OnGUI() 组合 toolbar、folder tree、list/grid、breadcrumb、search 和 context command。
ObjectSelector.SharedShow() 接收 requiredTypes、allowed ids 和 selection callback，复用过滤资产视图。
AssetDatabase 提供 GUID/path/type/find/dependency 等资产数据入口。
```

可学习：

```text
主浏览器和字段 Asset Picker 共享资产查询能力。
UI state、selection、search filter 与资产真相分离。
AssetRef 字段通过类型过滤选择资产，而不是让用户手写字符串。
```

不照搬：

```text
不复制 IMGUI / UIElements 双轨历史结构。
不依赖 Unity native AssetDatabase 黑盒。
不把 Selection 全局隐式状态作为 AI 修改协议。
```

在线参考：

```text
https://docs.unity3d.com/6000.0/Documentation/Manual/ProjectView.html
https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/ProjectBrowser/ProjectBrowser.cs
```

### 4.2 Unreal Engine

本地源码：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\ContentBrowser\Private\SContentBrowser.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\ContentBrowser\Private\SAssetView.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\ContentBrowser\Private\SAssetPicker.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\ContentBrowserData
```

关键实现：

```text
SContentBrowser 组合 sources、history、search、filters 和 AssetView。
SAssetView 订阅 ContentBrowserData 的 item updated/refreshed/discovery complete 事件。
RequestSlowFullListRefresh 重新读取 backend source items。
RequestQuickFrontendListRefresh 只执行 frontend filter/list refresh。
SAssetView 对刷新和过滤做分帧预算，不在 Slate 绘制时同步全量扫盘。
SAssetPicker 复用 SAssetView，并通过 FAssetPickerConfig 注入类型、选择、拖拽和回调。
```

可学习：

```text
后端索引刷新和前端过滤刷新必须分开。
Asset Picker 与主 Content Browser 应共享同一查询和视图能力。
大列表刷新必须有预算、状态和 telemetry/report。
```

不照搬：

```text
本轮不复制 UE 的多数据源、Collections、Plugin Content 和完整高级搜索系统。
本轮不复制 UE 的大型 context menu / extender 生态。
```

在线参考：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/content-browser-interface-in-unreal-engine
```

### 4.3 Godot

本地源码：

```text
<GODOT_SOURCE>\godot-master\godot-master\editor\docks\filesystem_dock.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\file_system\editor_file_system.cpp
<GODOT_SOURCE>\godot-master\godot-master\editor\file_system\editor_file_system.h
<GODOT_SOURCE>\godot-master\godot-master\editor\inspector\editor_resource_preview.cpp
```

关键实现：

```text
EditorFileSystemDirectory 缓存 path、UID、type、dependencies 和 import validity。
EditorFileSystem 扫描和增量变化后发出 filesystem_changed/resources_reimported。
FileSystemDock 只负责 folder tree、file list、search、selection 和 drag/drop。
EditorResourcePreview 使用 queue + cache + worker thread 异步生成缩略图。
缩略图缓存使用 path/content/modification 信息判断失效。
```

可学习：

```text
第一版 UI 心智可以保持目录树 + 资产列表 + 搜索 + 预览。
索引、Dock 和 Preview 必须分工，但它们仍属于同一个资产浏览系统，不是三个资产真相层。
缩略图必须支持 Pending / Ready / Failed，而不是假设同步立即完成。
```

不照搬：

```text
不采用纯文件系统作为长期资产身份。
不把 .import cache 当成项目真相。
```

在线参考：

```text
https://github.com/godotengine/godot/blob/master/editor/docks/filesystem_dock.cpp
```

## 5. 选定方案

采用：

```text
B-min+: Unified Native Asset Browser
```

核心原则：

```text
复用 142，不新增第二 Asset Browser。
项目资产是真相，索引只是 editor cache。
Native UI 直接消费 AssetBrowserModel，不再以 ProjectBrowserModel 为主模型。
所有资产操作进入 EditorSession command / transaction。
选择和命令优先使用稳定 asset identity，不把 path 当长期身份。
索引更新和 UI 过滤分离，禁止每帧扫描硬盘。
主浏览器和 AssetRef Picker 复用同一个模型与服务。
本轮实现复杂打飞机所需真实闭环，不扩成完整 UE Content Browser。
```

## 6. 资产身份规则

### 6.1 Entry Role

Asset Browser 必须区分：

```text
Folder
AuthoringAsset
SourceFile
```

含义：

```text
AuthoringAsset：Scene、Prefab、Rule、AUI、Input、Texture Asset 等可被项目引用的结构化资产。
SourceFile：PNG、WAV 等导入源文件，可预览和定位，但不能伪装成已经注册的 AssetRef。
Folder：只用于导航。
```

复杂打飞机中的：

```text
Assets/tex-player-ship.asset = AuthoringAsset，asset_id=tex-player-ship。
Assets/Images/tex-player-ship.png = SourceFile，是前者的 sourceImage。
```

### 6.2 Stable Asset Key

长期命令身份：

```text
优先：guid
次选：(asset_type, asset_id)
path：只用于定位、展示和 source mapping
```

规则：

```text
AuthoringAsset 的 asset_id 必须从结构化文档读取，不能从文件名猜测。
SourceFile 没有 authoring asset identity 时，asset_id 必须为空。
缺少 guid 时允许本轮使用稳定 (type,id)，同时输出 asset_identity_guid_missing。
不允许把 path hash 或文件名偷偷伪装成真实 guid。
selection / pick / drag / command 应携带 stable key，并保留 source path 供 report 定位。
legacy path-based selection 只保留兼容，不作为新增 UI/AI 命令主身份。
```

正式身份结构：

```text
AssetEntryKey:
  AuthoringAsset:
    asset_id
    asset_type_id
    guid?
  SourceFile:
    canonical_project_relative_path
    content_hash?
  Folder:
    canonical_project_relative_path

EditorAssetRef:
  asset_id
  asset_type_id
  guid?
  sub_asset_id?
```

规则：

```text
AssetEntryKey 用于浏览器 selection、hit target、drag 和 source entry 定位。
只有 AuthoringAsset key 可以转换为 EditorAssetRef。
EditorAssetRef 是 InspectorValue / PropertyValue / PickResult 的正式 AssetRef 表达。
asset_type_id 使用稳定小写规范值，例如 texture / prefab / font，不使用 UI display label。
AssetKind 只负责编辑器显示与过滤，不能替代 asset_type_id。
legacy AssetRef(String) 只允许作为兼容输入；235 新保存产物必须输出结构化 EditorAssetRef。
legacy selected_paths 只作兼容投影；新增命令使用 selected_entry_keys / primary_entry_key。
```

本轮不强制为整个旧项目批量生成 meta/guid；完整 GUID/meta 迁移仍属于 Asset DB/Importer 专项。

### 6.3 v1 资产目录与类型矩阵

B-min+ 必须覆盖当前复杂打飞机项目实际使用的目录：

```text
Scenes/          -> scene
Prefabs/         -> prefab
Rules/           -> rule
AUI/             -> aui
Input/           -> input_mapping
Assets/*.asset   -> texture / sprite / material / font / audio
Assets/**        -> image_source / audio_source 等 SourceFile
BuildProfiles/   -> build_profile
Settings/        -> project_settings
```

明确排除生成目录：

```text
Build/
Reports/
dist/
exports/
release/
target/
```

解析规则：

```text
必须使用 serde/现有 domain loader 读取 schemaVersion、assetId、type 和 source relation。
禁止通过字符串查找或只看扩展名猜 AuthoringAsset identity。
无法识别或 schema 无效的文件仍显示为 Unknown/SourceFile，并输出结构化 diagnostic。
Texture Asset 的 sourceImage 必须关联到对应 SourceFile，不能把两者当成两个可引用 texture。
索引路径必须规范化为 project-relative forward-slash path。
canonical path 越出 project root、包含非法 traversal 或通过链接逃逸 root 时必须拒绝并报告。
```

## 7. Cached AssetBrowserIndex

### 7.1 缓存结构

在现有 `AssetBrowserIndex` 上增加 editor session cache 语义：

```text
AssetBrowserIndexSnapshot:
  project_root
  revision
  entries
  source_fingerprint
  scan_generation
  refreshed_at
  dirty_reasons[]
  diagnostics[]
```

它不是持久化工程资产，也不进入 RuntimePackage。

缓存和 UI 状态由 `EditorSession` 明确持有：

```text
AssetBrowserSessionState:
  index_snapshot
  index_status: NotBuilt | Scanning | Ready | Stale | Failed
  index_progress
  pending_refresh
  ui_state
```

生命周期规则：

```text
Open/Create Project 和资产事务通过 &mut EditorSession 启动、提交或标记 index refresh。
EditorSession::build_ui_model(&self) 只能读取 snapshot 并做内存 query，禁止文件 IO。
NativeEditorApplication::frame 只能 pump 已完成的 index result，不得直接递归扫描。
backend scan 在 worker 或有预算的 editor task 中执行；结果以不可变 snapshot 提交。
首次扫描期间 UI 显示 Scanning/progress，旧 snapshot 可用时继续显示并标记 Stale。
```

### 7.2 刷新时机

B-min+ 只在以下时机重建或更新索引：

```text
打开/创建项目。
用户执行 Refresh Assets。
已知资产事务提交后。
保存/生成/导入会创建或删除资产条目的命令后。
测试显式标记 index dirty 后。
```

禁止：

```text
NativeEditorApplication::frame 每帧递归扫描项目目录。
AssetBrowser panel 绘制时直接读文件系统。
搜索文本变化时重新读取后端资产。
```

搜索、目录切换和类型过滤只对现有 snapshot 做 frontend query。

性能约束：

```text
连续 300 个无资产变化 editor frame 不得增加 scan_generation。
Search/Filter/Selection/ViewMode 变化不得触发 backend scan。
同一个 dirty generation 只能提交一次 refresh result。
后台扫描失败保留最后一份有效 snapshot，并将状态切为 Stale/Failed。
```

### 7.3 为方案 C 预留

方案 C 后续可以把 OS file watcher / importer event 降低为：

```text
mark_dirty(reason)
apply_delta(changed_paths)
或 request_full_refresh()
```

不会改变 AssetBrowserModel、Native UI 或 Asset Picker 契约。

## 8. Native Asset Browser UI

本轮使用现有 Editor Main Frame / Dock，不新建独立 OS 窗口。

布局：

```text
Toolbar:
  Back / Forward / Up
  Breadcrumb
  Search
  Type Filter
  List/Grid toggle
  Refresh

Body:
  Left: Folder tree
  Center: Asset list/grid
  Right or lower detail: selected asset preview + identity/status

Status:
  visible count / selected count / index revision / diagnostics
```

必须支持：

```text
目录导航和 breadcrumb。
搜索文本和 AssetKind filter。
列表/网格两种视图。
滚动，不再限制最多 5 条。
单选；Ctrl additive 与 Shift range 多选。
键盘方向键、Enter Open、Escape Cancel Picker。
双击打开 openable asset。
稳定 hit target id，不能用当前可见 index 作为长期命令身份。
空结果、missing、unimported、preview failed 等明确状态。
```

UI state 属于 EditorSession：

```text
current_folder
history
query
view_mode
thumbnail_size
scroll_offset
selection
picker_state
drag_state
```

这些状态不进入项目资产，不进入 RuntimePackage。

## 9. 真实缩略图与预览

B-min+ 只实现复杂打飞机当前需要的最小真实图片预览：

```text
PNG Texture/SourceFile 真实缩略图。
列表中仅请求 visible items。
选中 Texture 显示较大真实预览。
其它类型使用稳定 type icon + text summary。
```

缩略图状态：

```text
NotRequested
Pending
Ready
Failed
```

缓存键：

```text
stable asset/source key + content/source hash + requested size
```

实现约束：

```text
AssetThumbnailService 负责请求、PNG decode、CPU payload 和 LRU 状态。
EditorImageTextureRegistry 由 editor_wgpu_renderer 持有 GPU texture/upload 生命周期。
AssetBrowserModel 只携带 thumbnail_id/status/aspect_ratio，不携带 RGBA bytes 或 GPU handle。
DrawCommand::ImageTextureSlot 是通用编辑器图片命令，不复用 GameView ViewportTextureSlot。
Native editor application 只转发 ready upload 并请求 redraw，不在 panel draw 中 decode。
使用有上限的 editor-only memory cache：默认最多 128 个 thumbnail、64 MiB RGBA payload、32 个 pending request。
超限按 LRU 回收；选中预览优先于离屏 grid thumbnail。
只在 source/hash/size 变化后失效。
PNG decode 不在 panel draw 中执行。
不得把 Asset thumbnail 假装成 GameView ViewportTextureSlot。
失败必须保留 type icon，并输出 preview diagnostic。
```

本轮不做磁盘缩略图缓存、模型旋转预览、材质球、音频波形；方案 C 可以在相同 async contract 上增加。

## 10. 打开、拖拽和放置

### 10.1 Open

```text
Scene -> OpenSceneDocument
Prefab -> Open Prefab authoring
Rule -> Open Rule authoring
AUI -> Open AUI authoring
InputMapping -> Open Input Mapping authoring
Texture/SourceFile -> select + preview
Unknown -> structured diagnostic
```

所有打开动作继续由 EditorSession 路由，Asset Browser 不直接持有各 domain editor。

### 10.2 Drag

真实拖拽链路：

```text
PointerDown on asset
  -> movement threshold
  -> AssetDragPayload(stable asset refs)
  -> pointer capture
  -> hover drop target validation
  -> PointerUp
  -> target converts payload to domain command
```

本轮 drop target：

```text
Scene View：Prefab/Texture/Sprite/Material -> existing PlaceAssetIntoScene。
Inspector AssetRef field：compatible asset -> typed property command。
AUI image AssetRef field：Texture/Sprite -> AUI authoring command。
```

规则：

```text
drag payload 只携带结构化 AssetReference，不携带文件对象或裸 OS path。
drop target 决定允许类型和最终命令。
drag cancel 不修改任何项目文件。
一次 drop 只产生一组可审计 transaction。
```

本轮不实现把资产拖到 Project Folder 进行移动/复制。

## 11. Asset Picker 与字段级替换

### 11.1 Picker 入口

Inspector / AUI 中标记为 AssetRef 的字段提供 picker 图标。

点击后进入同一 Asset Browser 的 Picker Mode：

```text
AssetPickRequest:
  request_id
  target_document_path
  target_object_id
  target_field_path
  allowed_asset_types
  current_asset_ref
  expected_source_revision/hash
```

Picker 输出和提交计划必须保留完整结构化引用：

```text
AssetPickResult:
  request_id
  selected_entry_key
  editor_asset_ref
  diagnostics[]

AssetPickCommitPlan:
  target_document_path
  target_object_id
  target_field_path
  old_editor_asset_ref
  new_editor_asset_ref
  expected_source_revision/hash
  lowered_domain_command
```

Picker Mode 复用：

```text
Cached AssetBrowserIndexSnapshot
AssetQuery
Asset list/grid
Thumbnail preview
AssetPickResult
```

### 11.2 本轮“替换资产”的定义

本轮替换是字段级 AssetRef assignment：

```text
选中具体 owner document / object / field
  -> 打开类型过滤 Asset Picker
  -> 选择 replacement asset
  -> validate type/existence/identity
  -> preview old ref -> new ref
  -> confirm
  -> existing domain authoring command / transaction
  -> save
```

例如：

```text
Scenes/Main.scene.json
entity-player
SpriteRenderer2D.spriteRef
tex-player-ship -> tex-player-ship-alt
```

或：

```text
AUI/hud.aui.json
life-icons.imageRef
tex-player-ship -> tex-life-icon
```

规则：

```text
必须同类型或满足字段 AssetFilter。
必须验证 replacement asset 可解析。
必须携带 expected revision/hash，拒绝覆盖外部修改。
选择和 preview 不改项目；Confirm 后才进入 transaction。
Cancel 不产生写入。
写入后必须刷新受影响 document model 和 asset report。
ConfirmAssetPick 本身不直接写 JSON，只生成并执行 lowered_domain_command。
Scene SpriteRenderer2D.spriteRef 必须降低为结构化 SetSceneComponentField value。
AUI imageRef 必须降低为既有 AUI authoring command。
其它字段只有在 schema 声明 AssetRef + AssetFilter 时才允许进入 Picker。
禁止把结构化 EditorAssetRef 再压回裸字符串后保存。
```

### 11.3 本轮不做的替换

```text
全项目 Replace All References。
物理覆盖原始 PNG/WAV 文件。
移动/重命名/删除资产。
删除旧资产。
跨类型替换。
批量引用迁移。
```

这些动作需要完整 Asset Reference Graph / Impact Report / approvedImpact，属于方案 C 或独立高风险资源治理施工。

## 12. 命令与 AI 适配

Native UI 不直接调用 `std::fs` 修改项目。

本轮结构化命令至少包含：

```text
SetAssetBrowserFolder
SetAssetBrowserSearch
SetAssetBrowserKindFilter
SetAssetBrowserViewMode
SelectAssetBrowserEntry
OpenAssetBrowserEntry
RefreshAssetBrowserIndex
BeginAssetPick
ConfirmAssetPick
CancelAssetPick
BeginAssetDrag / CancelAssetDrag
```

长期命令规则：

```text
asset target 使用 stable asset key。
UI path/index 只能作为 source hint，不能作为唯一目标。
项目写入最终降低到既有 Scene/Prefab/AUI/Rule/Input/Property command。
不新增 Asset Browser 专用文件写入器。
```

AI 可以：

```text
读取 AssetBrowserModel/Report。
按 id/type/query 查找候选资产。
生成 Begin/Confirm Pick 或现有 domain property patch。
通过 source path、target field、old/new AssetRef 和 diagnostics 审查结果。
```

自然语言搜索词不是真相；最终命令必须解析为稳定 asset identity。

## 13. Report Panel

注册统一 provider：

```text
provider_id: authoring.asset_browser
schema: asset-browser-native-productization-report.v1
```

Editor 分档：

```text
Off：不生成诊断报告，只保留功能必需状态。
Summary：index revision、asset/folder/visible/selected counts、cache/preview 状态、错误计数。
Trace：scan reason、source paths、identity resolution、query、thumbnail queue、pick/drag/replace evidence。
```

Runtime：

```text
不加载 AssetBrowserModel。
不生成 Asset Browser report。
只消费 RuntimePackage / RuntimeAssetIndex。
```

报告至少能区分：

```text
asset_identity_missing
asset_identity_guid_missing
asset_source_missing
asset_unimported
asset_type_mismatch
asset_pick_rejected
asset_preview_pending
asset_preview_failed
asset_index_stale
asset_index_refresh_failed
asset_external_change_conflict
asset_drop_unsupported
```

## 14. 复杂打飞机验收链路

必须在临时复制的 complex shooter project 上证明：

```text
1. 打开项目后完成一次 backend scan，连续 300 个 editor frame 不增加 scan_generation。
2. Search/Filter/Selection/ViewMode 变化不增加 scan_generation。
3. Asset Browser 能识别 Scene/Prefab/Rule/AUI/Input/Texture/Font/BuildProfile 和 PNG SourceFile。
4. Build/Reports/dist/exports/release/target 不进入项目资产结果。
5. 搜索 tex-player-ship 能定位 AuthoringAsset 与关联 source image。
6. Native panel 显示真实 PNG 缩略图和选中预览。
7. thumbnail decode 的 alpha/non-background pixel 证据必须非空；draw plan 必须包含 ImageTextureSlot。
8. thumbnail request 只覆盖 visible/selected entries，且 cache/pending 不超过预算。
9. 双击 Rule/AUI/Input 可进入对应现有 authoring 产品面。
10. 拖拽 prefab-enemy-scout 到 Scene 仍走 PlaceAssetIntoScene transaction。
11. SpriteRenderer2D.spriteRef 打开类型过滤 picker。
12. 选择兼容 Texture 后 Preview/Confirm 写入结构化 EditorAssetRef；Cancel 不修改。
13. 保存、关闭、重开后 asset_id/type/guid/subAsset 语义保持不变。
14. Build 后 RuntimePackage/RuntimeAssetIndex 使用新 AssetRef。
15. 不兼容类型、缺失资源和外部修改均被结构化拒绝。
16. project-root 外路径、非法 traversal 和逃逸链接必须被拒绝。
```

真实 OS window / WGPU thumbnail screenshot 可以作为 local-only smoke；默认 CI 必须至少验证 PNG decode 非空像素、ImageTextureSlot、GPU upload plan 和缓存状态，不能只检查 thumbnail_count。

输出：

```text
complex-shooter-asset-browser-native-productization-report.v1
```

报告必须包含：

```text
project_root
index_revision / scan_generation
entry counts by role/kind
query and selected stable key
thumbnail evidence
drag/drop evidence
picker old/new ref evidence
save/reload evidence
RuntimePackage resolve evidence
diagnostics
```

## 15. 方案 C 兼容契约

为了保证后续从 B-min+ 升级到 C 不重写，本轮必须固定：

```text
1. Stable identity：选择/命令不用 path-only。
2. Cached snapshot：UI 不直接扫描硬盘。
3. Dirty API：索引支持 mark_dirty/rebuild，后续 watcher 可接入同一入口。
4. Backend/frontend refresh 分离：索引刷新和 query/filter 刷新不是同一操作。
5. Async preview state：NotRequested/Pending/Ready/Failed。
6. Transaction route：所有写入统一经过 EditorSession/domain command。
7. Shared picker：主浏览器与 AssetRef Picker 共用模型和查询。
8. Stable reports：扫描、缩略图和操作有结构化 evidence。
```

方案 C 后续主要新增：

```text
OS file watcher / importer event / incremental apply_delta。
磁盘 thumbnail cache 和更多 preview generator。
Move / Rename / Delete / Reimport / Batch operations。
Asset Reference Graph / Impact Report / Replace All References。
高级搜索、标签、Collections、版本控制状态。
```

这些新增能力不应推翻 235 的 Native panel、AssetBrowserModel、Picker、DragPayload 和 transaction 主链。

## 16. 本轮明确不做

```text
完整 UE Content Browser。
第二套 Asset DB。
运行时 Asset Browser。
每帧文件系统扫描。
OS file watcher。
磁盘缩略图缓存。
完整 Importer 设置编辑器。
模型/材质实时 3D preview。
音频波形和播放控制。
资产移动、重命名、删除、复制。
全项目引用图和批量替换。
资源商店、远程库、版本控制集成。
把 Player/Enemy/Bullet 等玩法概念写入通用 Asset API。
```

## 17. 预期涉及模块

正式施工预计主要涉及：

```text
rust/crates/editor_ui_model/src/asset_browser.rs
rust/crates/editor_ui_model/src/model.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_core/src/asset_browser.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/property_editing.rs
rust/crates/editor_core/src/inspector_details.rs
rust/crates/editor_ui_renderer/src/panels/project_browser.rs
rust/crates/editor_ui_renderer/src/draw_list.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/focus.rs
rust/crates/editor_wgpu_renderer/src/*
rust/crates/project_e2e_gate/src/*
```

允许施工时根据现有模块边界调整文件，但不得为了 235 新建长期第二资产架构层。

## 18. 风险与控制

### 风险 1：把缓存误做成第二真相

控制：snapshot 可随时丢弃并从项目资产重建，不落盘为新的工程资产格式。

### 风险 2：为了缩略图扩张成完整资源编辑器

控制：本轮只做 visible/selected PNG、受限内存 cache 和通用 image slot。

### 风险 3：Asset Picker 绕过 domain authoring

控制：PickResult 只产生候选 AssetRef，最终写入必须降低到现有 domain/property command。

### 风险 4：路径移动导致引用失效

控制：AuthoringAsset 使用 guid 或 `(type,id)`；path 只作 source location。完整 move/rename 仍 deferred。

### 风险 5：没有 watcher 导致外部修改不即时出现

控制：提供显式 Refresh、已知事务 dirty 和 stale diagnostic；方案 C 再接 watcher。

## 19. 方案自审

2026-07-10 施工前自审已吸收以下修正：

```text
AssetRef 从 String 收敛为结构化 EditorAssetRef，并保留 legacy input compatibility。
AssetBrowserSessionState / index status / &mut refresh ownership 已明确。
AUI/Font/BuildProfile/Settings 与生成目录排除矩阵已明确。
AssetThumbnailService -> EditorImageTextureRegistry -> ImageTextureSlot 边界和预算已明确。
300-frame no-rescan、非空像素、预算和 path safety 已进入强制验收。
```

```text
是否重复 142：
  否。142 的 Model/Index/Service 全部复用，235 只补 Native 产品面、缓存语义、真实 preview 和交互闭环。

是否新增第二资产真相：
  否。项目 typed assets 是编辑真相，RuntimePackage 是运行真相，index snapshot 只是 editor cache。

是否增加多余架构层：
  否。缓存、UI state 和 thumbnail registry 都是现有 Asset Browser/Editor Renderer 内部状态。

是否满足 AI 适配：
  是。稳定 identity、结构化 query/command/report、expected hash、明确 diagnostics。

是否适配复杂项目：
  是。禁止每帧扫描，后端刷新和前端过滤分离，并预留增量 watcher 接口。

是否满足复杂打飞机：
  是。能浏览真实贴图、拖入 Prefab、通过 Picker 替换 Scene/AUI AssetRef，并验证 Save/Reload/Build。

是否能升级方案 C：
  是。C 主要新增 watcher、磁盘 cache、高风险资产操作和引用图，不推翻 B-min+ 主干。

是否扩成完整 Content Browser：
  否。Move/Rename/Delete/Reimport/Replace All/Collections/VCS 均明确 deferred。

是否修改 runtime 热路径：
  否。Asset Browser 是 editor-only，Runtime 继续只消费 RuntimePackage/RuntimeAssetIndex。
```

## 20. 结论

正式采用：

```text
B-min+: Unified Native Asset Browser
```

本轮真正关闭的缺口是：

```text
用户和 AI 不再通过 JSON 或简陋 5 行文件列表使用资产。
Native Editor 可以真实浏览、搜索、预览、打开、拖拽和字段级替换 AssetRef。
Asset Browser 不再每帧扫描硬盘。
后续升级 Full Content Browser 不需要重写核心模型与交互主链。
```
