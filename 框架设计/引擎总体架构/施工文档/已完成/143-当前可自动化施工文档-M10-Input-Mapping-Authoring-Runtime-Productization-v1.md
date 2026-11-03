# 143-当前可自动化施工文档-M10 Input Mapping Authoring Runtime Productization v1

## 1. 施工目标

按 `143-M10-Input-Mapping-Authoring-Runtime-Productization-v1方案.md` 实现第一版：

```text
Editor Input Mapping Authoring
  -> Project InputMappingAsset
  -> Validate / Save
  -> Workspace / AssetBrowser summary
  -> DesktopExportPipeline input collection
  -> RuntimePackage default input mapping
  -> Player / ActionSnapshot verification
```

本次不重做 M5 / M6 输入底层，不新增项目玩法 API。

## 2. 不做什么

本次不做：

```text
Gamepad
Touch
IME text input
Runtime rebinding UI
Complex trigger graph
Multi-player input ownership
Native UI 完整 Input Mapping 面板
```

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

## 3. 涉及文件

预计涉及：

```text
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/input_mapping_authoring.rs
rust/crates/editor_core/src/desktop_export.rs
```

必要时只做小范围修改：

```text
rust/crates/engine_input/src/input_mapping.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/runtime_player_winit/src/lib.rs
```

## 4. 分阶段施工

### Gate A：UI Model 与命令

目标：

```text
新增 InputMappingAuthoringModel / InputMappingActionSummary / InputMappingBindingSummary / InputMappingAuthoringReport。
新增 UiCommandPayload 输入映射编辑命令。
命令保持结构化、AI 可读。
```

测试：

```powershell
cargo test -p editor_ui_model input_mapping_authoring
```

### Gate B：EditorCore InputMappingAuthoringService

目标：

```text
新增 editor_core/src/input_mapping_authoring.rs。
支持 create default / load / save / validate / add action / remove action / add binding / remove binding / set binding device path。
所有编辑都基于 engine_input::InputMappingAsset。
```

测试：

```powershell
cargo test -p editor_core input_mapping_authoring
```

### Gate C：DesktopExport / RuntimePackage 输入资产接入

目标：

```text
DesktopExportPipeline 扫描 ProjectRoot/Input/*.json。
解析并校验 InputMappingAsset。
写入 RuntimePackageBuildInput.input_mappings。
导出后的 RuntimePackage 使用项目 mapping，而不是静默使用 engine default fallback。
```

测试：

```powershell
cargo test -p editor_core desktop_export_input_mapping
cargo test -p engine_runtime runtime_package_loader_reads_project_default_input_mapping
cargo test -p runtime_player_winit input_mapping
```

### Gate D：EditorSession / Workspace / AssetBrowser 集成

目标：

```text
EditorSession 接入输入映射编辑命令。
Workspace Input domain 显示 count / default / validation status。
AssetBrowser 能识别 Input/input.default.json 为 InputMapping。
```

测试：

```powershell
cargo test -p editor_core editor_session_input_mapping
cargo test -p editor_core project_browser
cargo test -p editor_core asset_browser
```

### Gate E：整体回归

目标：

```text
确认 M10 没破坏编辑器、Runtime、Player、输入底层。
```

测试：

```powershell
cargo fmt --check
cargo test -p engine_input
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

## 5. 验收标准

必须满足：

```text
1. InputMapping authoring model 可序列化。
2. EditorCore 可创建默认 InputMappingAsset。
3. EditorCore 可添加 action 和 binding。
4. EditorCore 可保存和重新读取 mapping。
5. validate 能报告 unknown action / context / unsupported device path。
6. Workspace Input domain 能显示 mapping 数量和验证状态。
7. DesktopExportPipeline 能把 ProjectRoot/Input 下的 mapping 写入 RuntimePackage。
8. RuntimePackage loader 能读到项目 default_input_mapping。
9. WindowedPlayer 使用 RuntimePackage mapping 优先于 engine default fallback。
10. 不新增项目玩法 API。
11. 整体回归通过。
```

## 6. 完成后文档同步

完成后：

```text
1. 将本施工文档移动到 施工文档/已完成/。
2. 新增 阶段完成记录/2026-07-02-M10-Input-Mapping-Authoring-Runtime-Productization-v1/00-总览.md。
3. 更新 54-当前可自动化施工总入口.md。
4. 更新 施工文档/README.md。
5. 更新 00-文档地图.md / README.md。
```

## 7. 施工自审

### 7.1 Specification fit

施工文档覆盖 M10 第一版完整产品链路：编辑、验证、保存、构建、运行验证。

### 7.2 Rule fit

施工不重做 M5/M6，不新增玩法 API，只补输入资产创作与产品化接入。

### 7.3 Textual consistency

Gate A-D 对应方案文档中的 UI model、Editor service、DesktopExport、EditorSession/Workspace 集成。

### 7.4 Design fit

施工按系统 gate 推进，不拆成无穷小点，符合大系统优先规则。

### 7.5 Implementation feasibility

现有代码已有 InputMappingAsset、RuntimePackage input manifest、DesktopExportPipeline、Workspace domain，可增量实现。

### 7.6 Practical reasonableness

每个 gate 都有明确测试命令，最终有整体回归，第一版范围可控。

结论：

```text
施工文档通过自审，可以开工。
```
