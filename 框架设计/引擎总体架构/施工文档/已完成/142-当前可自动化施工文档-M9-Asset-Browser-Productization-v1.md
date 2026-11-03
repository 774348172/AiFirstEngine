# 142-当前可自动化施工文档-M9 Asset Browser Productization v1

## 1. 施工目标

按 `142-M9-Asset-Browser-Productization-v1方案.md` 实现第一版：

```text
C-architecture
B-implementation
```

本次目标不是替换 Asset DB / Importer / RuntimePackage，也不是删除现有 ProjectBrowser。

本次要新增长期语义层：

```text
AssetBrowserModel
AssetBrowserEntry
AssetKind
AssetQuery
AssetSelection
AssetPreviewDescriptor
AssetPickRequest / AssetPickResult
AssetDragPayload
AssetBrowserReport
```

并最小接入 EditorSession。

## 2. 不做什么

本次不做：

```text
真实 GPU 缩略图缓存
完整拖拽 UI 手势
复杂右键菜单
高级搜索 DSL
完整依赖图
批量重命名依赖修复
版本控制集成
```

不得新增项目玩法 API：

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

## 3. 涉及文件

预计涉及：

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/asset_browser.rs
```

必要时只做小范围修改：

```text
rust/crates/editor_core/src/asset_placement.rs
```

## 4. 分阶段任务

### A0：现状审计

目标：

```text
确认 ProjectBrowserModel / AssetPlacementResolver / EditorSession 当前边界。
```

命令：

```powershell
rg -n "ProjectBrowserModel|AssetPlacementResolver|PlaceAssetIntoScene|SelectProjectBrowserEntry" rust/crates/editor_core/src rust/crates/editor_ui_model/src
```

### A1：UI Model 补齐 Asset Browser 数据结构

目标：

```text
在 editor_ui_model 中新增 AssetBrowserModel / Entry / Query / Selection / Preview / Pick / Drag / Report。
保留 ProjectBrowserModel。
```

测试：

```powershell
cargo test -p editor_ui_model asset_browser
```

### A2：editor_core 新增 AssetBrowserIndex / Builder

目标：

```text
新增 rust/crates/editor_core/src/asset_browser.rs。
从 ProjectSession project_root 扫描基础目录和文件。
生成 AssetBrowserModel。
```

测试：

```powershell
cargo test -p editor_core asset_browser_index
```

### A3：搜索 / 类型过滤 / Report

目标：

```text
AssetQuery 支持 search_text contains 和 kinds 过滤。
AssetBrowserReport 统计 asset_count / folder_count / selected_count / missing_count / unimported_count / filtered_count。
```

测试：

```powershell
cargo test -p editor_core asset_browser_query
cargo test -p editor_core asset_browser_report
```

### A4：AssetPickRequest / AssetPickResult

目标：

```text
支持 AssetRef 字段选择模型。
类型不匹配返回 structured diagnostic。
```

测试：

```powershell
cargo test -p editor_core asset_browser_pick
```

### A5：AssetDragPayload / PlaceIntoScene

目标：

```text
AssetDragPayload 能转换为现有 AssetPlacementRequest。
Scene 放置仍走 AssetPlacementResolver。
```

测试：

```powershell
cargo test -p editor_core asset_browser_drag
cargo test -p editor_core asset_placement
```

### A6：EditorSession 最小接入

目标：

```text
EditorSession 新增 build_asset_browser_model。
ProjectBrowserModel 由 AssetBrowserModel 兼容投影或保持一致。
打开项目后 AssetBrowser 能看到基础目录和 default scene。
选择资产能更新 selection。
```

测试：

```powershell
cargo test -p editor_core editor_session_asset_browser
cargo test -p editor_core project_browser
```

### A7：整体回归

命令：

```powershell
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

## 5. 验收标准

必须满足：

```text
1. AssetBrowserModel 可序列化。
2. AssetBrowserIndex 能扫描项目基础目录和文件。
3. AssetQuery 能按文本和 kind 过滤。
4. AssetSelection 能表达 primary 和多选。
5. AssetPickRequest 类型不匹配时返回 diagnostic。
6. AssetDragPayload 能生成 AssetPlacementRequest。
7. EditorSession 打开项目后能构建 AssetBrowserModel。
8. ProjectBrowserModel 不被破坏。
9. AssetBrowserReport 能统计关键字段。
10. 整体回归通过。
11. 新增 API 不出现项目玩法语义。
```

## 6. 完成后文档同步

完成后：

```text
1. 将本施工文档移动到 施工文档/已完成/。
2. 新增 阶段完成记录/2026-07-02-M9-Asset-Browser-Productization-v1/00-总览.md。
3. 更新 54-当前可自动化施工总入口.md。
4. 更新 施工文档/README.md。
5. 更新 00-文档地图.md / README.md。
```

## 7. 施工自审

### 7.1 Specification fit

施工文档对应 M9 Asset Browser 产品化，覆盖浏览、查询、选择、picker、drag payload、report、EditorSession 接入。

### 7.2 Rule fit

施工不重做 Asset DB / Importer / RuntimePackage，不新增项目玩法 API，遵守 130 缺失能力主线。

### 7.3 Textual consistency

施工阶段与正式方案一致：Asset Browser 是编辑器资产工作流层，底层继续复用 ProjectBrowser / AssetPlacement / EditorSession transaction。

### 7.4 Design fit

符合 C-architecture / B-implementation。长期边界完整，第一版施工可控。

### 7.5 Implementation feasibility

现有代码已有 ProjectBrowserModel、ProjectSession、AssetPlacementResolver、EditorSession，可增量施工。

### 7.6 Practical reasonableness

每阶段有测试命令，整体回归明确，不一次性做完整 UE Content Browser。

结论：

```text
施工文档通过自审，可以开工。
```
