# 235-当前可自动化施工文档-Asset Browser Native Productization v1

> 状态：已完成归档。  
> 施工日期：2026-07-10。  
> 对应正式方案：`../../235-Asset-Browser-Native-Productization-v1方案.md`。  
> 采用方案：`B-min+: Unified Native Asset Browser`。  
> 前置完成：`142-M9-Asset-Browser-Productization-v1方案.md` 及其阶段完成记录。  
> 外部审查：未提供针对 235 的外部方案审查文档；已完成施工前方案自审并把 5 项修正写回正式方案。

## 0. 施工目标

把 142 已有 Asset Browser 数据/服务层接成真实 Native Editor 产品面：

```text
Project typed authoring assets / source files
  -> EditorSession-owned Cached AssetBrowserIndexSnapshot
  -> AssetBrowserModel
  -> Native folder tree / list-grid / search / preview
  -> Drag / Asset Picker / field-level AssetRef replacement
  -> Existing domain command / EditorSession transaction
  -> Save / Reload / RuntimePackage Build
```

本轮必须交付：

```text
结构化 AssetEntryKey / EditorAssetRef，并兼容读取 legacy AssetRef(String)。
覆盖复杂打飞机真实目录和类型的 typed asset catalog。
EditorSession-owned index snapshot/status/dirty lifecycle，彻底移除 frame compose 扫盘。
Native Asset Browser 目录树、搜索、过滤、列表/网格、滚动、多选、打开。
PNG visible/selected thumbnail decode、受限内存 cache 和真实 ImageTextureSlot present。
Scene drop、Inspector/AUI Asset Picker 和字段级同类型 AssetRef 替换。
authoring.asset_browser Report Panel provider，Editor Off/Summary/Trace。
复杂打飞机 Save/Reload/Build/RuntimePackage E2E 和 no-rescan/performance evidence。
```

## 1. 不得突破的边界

```text
项目 typed authoring asset 是编辑真相；AssetBrowserIndexSnapshot 只是可丢弃 editor cache。
RuntimePackage / RuntimeAssetIndex 仍是运行时真相；Runtime 不加载 AssetBrowserModel。
复用 142 的 AssetBrowserModel/Index/Service，不新建第二套 Asset Browser 或 Asset DB。
Native UI 直接消费 AssetBrowserModel；ProjectBrowserModel 只保留临时兼容投影。
EditorSession::build_ui_model(&self) 和 panel draw 禁止文件 IO。
新增命令使用稳定 AssetEntryKey / EditorAssetRef，不新增 path-only 长期身份。
Asset Picker 只生成候选和 lowered domain command，不直接写 JSON。
拖拽最终进入现有 Scene/AUI/Property command 和 transaction。
缩略图不得复用或伪装成 GameView ViewportTextureSlot。
正式 runtime 默认不生成 Asset Browser report。
不实现 OS file watcher、磁盘 thumbnail cache、Move/Rename/Delete/Reimport、Replace All、依赖图或 VCS。
不把复杂打飞机玩法名写入通用 engine API。
不清理或回退无关 dirty worktree。
```

## 2. 预计涉及文件

身份、模型和命令：

```text
rust/crates/editor_ui_model/src/asset_browser.rs
rust/crates/editor_ui_model/src/inspector.rs
rust/crates/editor_ui_model/src/model.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/lib.rs
```

索引、缓存、Picker、事务和报告：

```text
rust/crates/editor_core/src/asset_browser.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/property_editing.rs
rust/crates/editor_core/src/inspector_details.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/services/project_service.rs
rust/crates/editor_core/src/lib.rs
```

Native UI、输入和缩略图 present：

```text
rust/crates/editor_ui_renderer/src/draw_list.rs
rust/crates/editor_ui_renderer/src/panels/project_browser.rs
rust/crates/editor_ui_renderer/src/panels/mod.rs
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/focus.rs
rust/crates/editor_window_winit/src/command_system.rs
rust/crates/editor_wgpu_renderer/src/*
```

E2E、依赖和样例：

```text
rust/crates/project_e2e_gate/src/*
rust/crates/*/Cargo.toml（仅在现有依赖不足时）
rust/Cargo.lock（仅由依赖变化产生）
samples/complex_shooter_project/*（仅修复真实样例缺口时；测试修改优先使用临时复制）
```

允许根据现有模块边界增减文件，但不得为 235 新增长期第二资产架构层。

## 3. Gate A - 结构化身份与 Typed Asset Catalog

任务：

```text
1. 新增 AssetEntryRole 与可序列化 AssetEntryKey：AuthoringAsset / SourceFile / Folder。
2. 新增结构化 EditorAssetRef(asset_id, asset_type_id, guid?, sub_asset_id?)。
3. AssetBrowserEntry 同时携带稳定 entry_key、role、canonical path、identity/source status。
4. AssetSelection 新增 selected_entry_keys / primary_entry_key；selected_paths 保留 legacy projection。
5. AssetPickResult / AssetDragPayload 改用 EditorAssetRef；legacy AssetRef(String) 只作输入兼容。
6. 扩展 AssetKind/type mapping，覆盖 Scene/Prefab/Rule/AUI/Input/Texture/Sprite/Material/Font/Audio/BuildProfile/ProjectSettings。
7. 索引覆盖 Scenes/Prefabs/Rules/AUI/Input/Assets/BuildProfiles/Settings。
8. 排除 Build/Reports/dist/exports/release/target。
9. 通过 serde/现有 domain loader 读取 schema/id/type/source relation，不做字符串刮取。
10. Texture Asset 与 sourceImage 建立关系；SourceFile 不可直接转换为 EditorAssetRef。
11. 规范化 project-relative path，拒绝 traversal、root 外路径和逃逸链接。
```

兼容要求：

```text
现有 AssetBrowserModel serde 测试继续通过或有明确 v2 migration。
现有 ProjectBrowserModel projection 继续工作。
现有 PlaceAssetIntoScene legacy payload 继续可解析。
缺少 guid 的 authoring asset 使用 (type,id) 并产生 warning，不伪造 guid。
```

Gate 测试：

```powershell
cargo test -p editor_ui_model asset_browser
cargo test -p editor_core asset_browser
cargo test -p editor_core project_browser
```

通过条件：

```text
complex shooter 的 AUI、font、BuildProfile 和 texture source relation 均被识别。
生成目录不进入 catalog。
AuthoringAsset identity 来自 typed document，不来自文件名猜测。
EditorAssetRef round-trip 不丢失 type/guid/subAsset。
legacy string/path 输入兼容，但新增保存产物为结构化引用。
path safety 测试覆盖正常、traversal 和 root escape。
```

## 4. Gate B - EditorSession Cached Index 与 No-rescan

任务：

```text
1. EditorSession 持有 AssetBrowserSessionState。
2. 状态包含 snapshot、NotBuilt/Scanning/Ready/Stale/Failed、progress、pending refresh 和 UI state。
3. Open/Create Project 启动首次 scan；成功后原子提交 immutable snapshot。
4. Refresh Assets 和已知资产事务通过 &mut EditorSession mark_dirty/request_refresh。
5. build_ui_model(&self) 只对 snapshot 执行内存 query，不读文件系统。
6. NativeEditorApplication::frame 只 pump ready result，不执行递归扫描。
7. 搜索、过滤、选择、目录和视图模式变化只做 frontend refresh。
8. scan_generation 只在 backend snapshot 成功提交后增加。
9. scan 失败保留最后有效 snapshot，并输出 Stale/Failed diagnostic。
10. B-min+ 可以使用 worker 或有预算 editor task，但 UI 必须能展示 Scanning/progress。
```

Gate 测试：

```powershell
cargo test -p editor_core asset_browser
cargo test -p editor_core authoring_workspace
cargo test -p editor_window_winit asset_browser
```

通过条件：

```text
打开项目后 snapshot Ready 且 scan_generation=1。
连续 300 个 editor frame 不增加 scan_generation。
Search/Filter/Selection/ViewMode 不增加 scan_generation。
显式 Refresh 或资产事务 dirty 只产生一次新 generation。
测试 instrumentation 证明 build_ui_model 和 panel draw 没有文件 IO。
```

## 5. Gate C - Native Asset Browser 产品面

任务：

```text
1. EditorUiModel 正式暴露 AssetBrowserModel；Native renderer 不再以 ProjectBrowserModel 为主输入。
2. 在现有 Main Frame/Dock 内实现 toolbar、folder tree、asset body、preview/status 区。
3. toolbar 支持 Back/Forward/Up、breadcrumb、Search、Type Filter、List/Grid、Refresh。
4. 列表和网格支持稳定尺寸、滚动和任意数量条目，不再 take(5)。
5. 支持单选、Ctrl additive、Shift range、键盘方向、Enter Open 和 Escape Cancel Picker。
6. hit target 使用 AssetEntryKey 的稳定编码，不使用 visible index 作为命令目标。
7. Search text 输入进入 AssetBrowserSessionState，不污染 Inspector 文本编辑。
8. 双击/Enter 按类型路由到现有 Scene/Prefab/Rule/AUI/Input authoring。
9. 空结果、Scanning、Stale、missing、unimported 和 disabled action 有明确视觉状态。
```

Gate 测试：

```powershell
cargo test -p editor_ui_model asset_browser
cargo test -p editor_ui_renderer asset_browser
cargo test -p editor_input asset_browser
cargo test -p editor_window_winit asset_browser
```

通过条件：

```text
DrawList 包含完整 toolbar/tree/body/status 和稳定 hit regions。
滚动后可以选择第 5 项之外的资产。
搜索、过滤、列表/网格切换和键盘导航可通过真实输入路由完成。
Rule/AUI/Input 双击进入已有 authoring，不新增第二编辑窗口。
ProjectBrowserModel 只剩兼容投影，不驱动 Native panel。
```

## 6. Gate D - PNG Thumbnail / Editor Image Present

任务：

```text
1. 新增 AssetThumbnailService：request、decode queue、CPU payload、status、LRU。
2. request key 使用 stable source key + content hash + size。
3. 只请求 visible entries 和当前 selected preview；离屏条目不请求。
4. 默认预算：128 thumbnails、64 MiB RGBA payload、32 pending requests。
5. PNG decode 不在 panel draw 或 build_ui_model 中执行。
6. AssetBrowserModel 只携带 thumbnail_id/status/aspect_ratio，不携带 bytes/GPU handle。
7. 新增通用 DrawCommand::ImageTextureSlot。
8. editor_wgpu_renderer 新增 EditorImageTextureRegistry，负责 upload/resolve/LRU release。
9. Native application pump ready CPU payload，并在 GPU upload 后 request redraw。
10. thumbnail 失败显示 type icon，并输出 source/hash/stage diagnostic。
11. 不复用 GameView ViewportTextureSlot，不实现磁盘 cache。
```

Gate 测试：

```powershell
cargo test -p editor_core asset_thumbnail
cargo test -p editor_ui_renderer image_texture
cargo test -p editor_wgpu_renderer image_texture
cargo test -p editor_window_winit asset_thumbnail
```

通过条件：

```text
complex shooter PNG decode 后 alpha/non-background pixel 非空。
DrawList -> DrawPlan -> GPU command 含 ImageTextureSlot/upload evidence。
visible/selected request 集合正确，预算不被突破。
source hash 或 size 改变会失效；无变化 frame 不重复 decode/upload。
真实 OS window/WGPU screenshot smoke 可 local-only，默认 CI 不依赖人工目测。
```

## 7. Gate E - Drag/Drop、Asset Picker 与字段级替换

任务：

```text
1. 实现 pointer threshold、capture、hover target、drop validation 和 cancel 的 Asset drag state。
2. Scene drop 把 AuthoringAsset key 转为 EditorAssetRef，再复用 PlaceAssetIntoScene。
3. Inspector/AUI AssetRef 字段提供 picker icon，并生成完整 AssetPickRequest。
4. Picker Mode 复用同一个 snapshot/query/list/thumbnail，不新建第二 browser model。
5. AssetPickResult 保留 selected_entry_key 和结构化 EditorAssetRef。
6. Confirm 生成 AssetPickCommitPlan，再降低为 domain command；Cancel 不写入。
7. Scene SpriteRenderer2D.spriteRef 降低为结构化 SetSceneComponentField value。
8. AUI imageRef 降低为既有 AUI authoring command。
9. 其它字段只有 schema 声明 AssetRef + AssetFilter 时才允许 picker/drop。
10. 类型、存在性、identity 和 expected source revision/hash 全部验证。
11. 写入后刷新对应 document/Inspector/Asset Browser report。
```

Gate 测试：

```powershell
cargo test -p editor_core asset_browser
cargo test -p editor_core property_editing
cargo test -p editor_core aui_authoring
cargo test -p editor_input asset_browser
cargo test -p editor_window_winit asset_browser
```

通过条件：

```text
拖拽 Prefab 到 Scene 产生现有 transaction；cancel 不产生写入。
SourceFile 和不兼容类型不能 drop/pick 为 AssetRef。
Picker Preview/Cancel/Confirm 状态确定且可序列化。
Confirm 后保存结构化 id/type/guid/subAsset，不退化为 string。
外部修改导致 hash/revision 不一致时拒绝覆盖并给出 next action。
```

## 8. Gate F - Report Panel 与复杂打飞机 E2E

任务：

```text
1. 注册 authoring.asset_browser provider。
2. Editor report 实现 Off/Summary/Trace；Runtime 不生成 Asset Browser report。
3. 新增 asset-browser-native-productization-report.v1。
4. 在 complex shooter 临时副本执行 open/index/query/thumbnail/open/drag/pick/save/reload/build。
5. 用已有 texture asset 做同类型 SpriteRenderer2D.spriteRef 替换，不污染仓库样例源文件。
6. 验证 Cancel 不改文件，Confirm 后 source hash 改变且结构化引用保持。
7. 重新打开项目验证引用不丢失。
8. ProjectRuntimePackageAssembler/RuntimePackage/RuntimeAssetIndex 解析 replacement ref。
9. 记录 scan_generation、entry matrix、thumbnail pixels、drag/pick/transaction 和 package evidence。
10. 增加 path traversal/root escape 拒绝证据。
```

Gate 测试：

```powershell
cargo test -p editor_core report_panel
cargo test -p project_e2e_gate asset_browser_native_productization
cargo test -p project_e2e_gate complex_shooter
cargo test -p engine_runtime runtime_asset
```

通过条件：

```text
新 E2E 测试真实存在并运行，不能以 0 tests filtered 作为通过。
报告包含 index/query/identity/thumbnail/drag/picker/save/reload/package evidence。
300-frame no-rescan、非空 thumbnail pixel、预算和 path safety 全部通过。
Summary 不携带完整 trace；Trace 可解释 source/target/stage/diagnostic/next action。
RuntimePackage 使用 replacement AssetRef，Runtime 不依赖 editor cache。
```

## 9. Gate G - 整体回归、文档与归档

整体回归：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_ui_renderer
cargo test -p editor_input
cargo test -p editor_wgpu_renderer
cargo test -p editor_window_winit
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo test -p project_e2e_gate
```

测试规则：

```text
每个 Gate 先跑定向测试，通过后才能进入下一 Gate。
新增 filter 命令必须确认至少执行一个对应新测试。
失败先修当前 Gate，不跨 Gate 堆积问题。
只修 235 或本轮直接暴露的回归，不整理无关 dirty worktree。
真实 OS window/WGPU screenshot 可以 local-only；默认 CI 仍必须有确定性像素/plan evidence。
```

完成后：

```text
生成 阶段完成记录/2026-07-10-Asset-Browser-Native-Productization-v1/00-总览.md。
更新 49-当前状态与下一步入口.md。
更新 54-当前可自动化施工总入口.md。
更新 施工文档/README.md。
更新 阶段完成记录/README.md。
更新 227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md，将 P1-4 标为完成。
下一默认讨论系统切换到 P2-1 Save / Reload / Rebuild Consistency Gate v1。
把本施工文档从 当前/ 移到 已完成/。
```

## 10. 施工文档自审

审查结论：`通过，可以施工`。

审查记录：

```text
外部审查：未提供针对 235 的外部方案审查文档。
方案自审：已把结构化 EditorAssetRef、缓存所有权、资产矩阵、thumbnail 边界和强制验收写回 235。
前置复用：复用 142 AssetBrowserModel/Index/Service，不重复实现 Asset DB。
真相边界：项目资产、editor cache、RuntimePackage 三者职责明确，没有第二持久化资产真相。
结构复杂度：AssetBrowserSessionState 和 Thumbnail registry 都是现有模块内部状态，不新增长期架构层。
身份：新增链路使用 AssetEntryKey/EditorAssetRef，legacy string/path 只作兼容。
性能：明确禁止 build_ui_model/frame/draw 扫盘，并有 300-frame no-rescan gate。
渲染：thumbnail 使用通用 ImageTextureSlot，不滥用 GameView texture；有预算和非空像素证据。
写入：Picker/Drag 降低到既有 domain command/transaction，不直接改 JSON。
范围：不包含 watcher、磁盘 cache、Move/Rename/Delete/Reimport、Replace All 或完整 Content Browser。
报告：Editor Off/Summary/Trace，Runtime 默认不生成 Asset Browser report。
测试：每个 Gate 有定向测试，最后覆盖全量回归和 complex shooter E2E。
归档：完成记录、入口、227 状态和施工文档移动均已明确。
```

自审未发现阻塞施工的剩余冲突。正式施工必须从 Gate A 开始，当前 Gate 测试通过后才能进入下一 Gate。
