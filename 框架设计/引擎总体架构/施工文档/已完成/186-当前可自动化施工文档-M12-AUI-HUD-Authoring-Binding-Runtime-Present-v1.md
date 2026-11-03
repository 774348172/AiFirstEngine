# 186-当前可自动化施工文档-M12-AUI-HUD-Authoring-Binding-Runtime-Present-v1

## 1. 施工来源

来源方案：

```text
框架设计/引擎总体架构/185-M12-AUI-HUD-Authoring-Binding-Runtime-Present-v1方案.md
其它AI审查目录/12-185-M12-AUI-HUD方案审查.md
```

本施工文档只实现 M12 的第一版可验证产品链路：

```text
AUI Document
  -> Headless Authoring Service
  -> Binding Resolve
  -> Layout / DrawList / OverlayFrame
  -> RuntimePackage AUI manifest
  -> Interaction Click -> AuiAction
```

第一版先不做完整 Native Editor AUI 面板；209 之后后续可视化编辑统一按 Scene Unified AUI Authoring 理解。真实 Editor UI 接入排在 Runtime Binding / Present 后面，不能阻塞核心链路。

## 2. 施工原则

```text
不新增打飞机专用 API。
不让 AUI 直接读取 ECS / Project Rule / Runtime World。
不让 AUI 直接生成 RHI 命令。
不让 Button / ProgressBar 新增专用 DrawCommand。
不把 AuiCommand 当业务 action 使用。
不允许施工文档按 185 旧 Gate 顺序推进。
```

## 3. Gate A：AUI v1 数据结构补齐

目标：

```text
补齐 ProgressBar node kind。
扩展 AuiBindingRef 为 target_field / path / fallback。
新增 UiValue / AuiBindingValue / ProjectUiStateSnapshot。
新增 AuiAction。
为 AuiNode 增加 progress_value / action_refs。
保留 AuiDrawCommand 只有 Rect / Image / Text。
```

最小测试：

```text
aui_binding_ref_requires_target_path_fallback
aui_progress_bar_uses_composite_draw_commands
aui_action_is_separate_from_pointer_command
```

## 4. Gate B：AUI Authoring Service headless

目标：

```text
新增 AuiAuthoringService。
支持 create_document / add_node / set_node_field / save / load / validate。
新增 AuiTransaction / AuiAuthoringReport。
字段修改走 schema path，不直接散写 JSON。
```

第一版服务位置：

```text
rust/crates/editor_core/src/aui_authoring.rs
```

最小测试：

```text
aui_authoring_service_creates_saves_reopens_hud_document
aui_authoring_service_edits_binding_by_schema_path
```

## 5. Gate C：Runtime Binding / Present

目标：

```text
新增 AuiRuntimeResolver。
输入 AuiDocument + ProjectUiStateSnapshot。
输出已解析 AuiDocument / AuiBindingReport。
Text.text / ProgressBar.value / Panel.visible / Image.visible / Image.asset_ref 可绑定。
Layout / DrawList / OverlayFrame 能消费解析后的 AuiDocument。
ProgressBar 用背景 Rect + 填充 Rect 组合绘制。
Text 仍输出 DrawText；Runtime glyph present 由 renderer/font backend 负责，M12 需要在 report 中标记 text_present_required。
```

最小测试：

```text
aui_runtime_resolver_updates_text_progress_visible_and_image
aui_runtime_present_builds_overlay_with_real_text_items
```

## 6. Gate D：Build / RuntimePackage 接入

目标：

```text
新增结构化 RuntimeAuiManifest。
RuntimePackageManifest 增加可选 aui index。
RuntimePackageBuilder 写入 aui/aui-manifest.json。
RuntimePackageLoader 读取并校验 AUI manifest。
Build report / Runtime diagnostics 能报告 AUI manifest 问题。
```

最小测试：

```text
runtime_package_builder_writes_structured_aui_manifest
runtime_package_loader_reads_aui_manifest
```

## 7. Gate E：Interaction Action

目标：

```text
AuiInteractionSystem 继续输出 AuiCommand 指针级命令。
新增 AuiActionMapper，把 Click + node.action_refs 映射成 AuiAction。
AuiAction 进入 report，不能直接改 Runtime World。
```

最小测试：

```text
aui_click_command_maps_to_business_action
aui_consumed_input_still_blocks_gameplay_input
```

## 8. Gate G：End-to-end 自审测试

最小 HUD fixture：

```text
hud.aui.json
  Canvas
    Panel top_left
      Text score_text bind("game.score_text")
    ProgressBar hp_bar bind("player.hp_ratio")
    Button pause_button action("ui.pause")
    Panel pause_overlay visible bind("game.paused")
```

验收：

```text
Headless authoring 能创建该 HUD。
Binding resolve 后分数文本、血量条、暂停面板可变化。
DrawList 包含 Text 和 ProgressBar 的组合 Rect。
Click pause_button 能发出 ui.pause action。
RuntimePackage 包含 AUI manifest。
cargo test -p engine_runtime aui
cargo test -p editor_core aui
```

## 9. 本次不做

```text
209 Scene Unified AUI Authoring。
拖拽布局编辑。
WorldSpace UI。
InputField / IME / RichText / ScrollView。
Mask / Clip / 动画。
UGUI / NGUI 兼容层。
项目侧 ScoreSystem / HealthSystem / ShooterHUD。
```

## 10. 施工自审标准

施工完成后必须确认：

```text
185 外部审查指出的问题均已进入代码或明确留在后续真实 Editor UI Gate。
AuiBindingRef 不再只是 binding_id。
AuiCommand 和 AuiAction 没有混成一层。
ProjectUiStateSnapshot 是 AUI 读取运行时 UI 状态的唯一入口。
Button / ProgressBar 没有污染底层 DrawCommand。
RuntimePackage 写入结构化 AUI manifest。
测试覆盖 Gate A-E 的最小链路。
```
