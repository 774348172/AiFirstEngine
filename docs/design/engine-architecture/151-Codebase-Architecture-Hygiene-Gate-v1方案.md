# 151-Codebase Architecture Hygiene Gate v1 方案

## 1. 问题定义

本方案解决 `149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md` 中新增的代码结构治理问题：

```text
Q16 多个 Rust 文件体量过大，AI 检索和人工维护成本开始上升。
Q17 EditorSession 仍是事实上的编辑器聚合根，领域状态和 UI model composition 继续集中。
Q18 editor_window_winit 仍把 app shell / dock / input route / viewport / present / 真实窗口测试放在同一入口文件。
Q19 UI model / UI renderer / WGPU renderer 都是单文件 crate，后续面板和渲染能力继续增加会恶化。
Q20 测试保护网充足但高度集中在大文件内部，重构时定位成本高。
```

当前代码还不能判断为已经不可维护，但已经出现明显的大入口文件和职责堆叠风险：

```text
editor_core/src/lib.rs                       5018 行
editor_window_winit/src/lib.rs               4174 行
editor_core/src/scene_editing.rs             2272 行
editor_ui_renderer/src/lib.rs                2098 行
editor_ui_model/src/lib.rs                   2098 行
editor_wgpu_renderer/src/lib.rs              1865 行
```

本系统目标不是新增功能，而是建立代码结构健康门禁，让后续新增功能不会继续堆进少数 `lib.rs`。

## 2. 设计目标

本方案采用用户确认的完整方案 C：

```text
一次性确认长期最优目标架构。
按阶段小步迁移。
每迁移一块就测试一块。
不通过行为变更来掩盖结构治理。
不因为谨慎而只做表面轻拆。
```

最终目标：

```text
lib.rs 只作为 crate public facade。
每个 crate 内部按领域拆分文件和目录。
EditorSession 保留外部兼容 facade，但内部委托给 domain service。
Native editor shell 内部按 shell / dock / input / viewport / present / dialog / report 分区。
UI model / UI renderer / WGPU renderer 按模型、面板、draw list、hit test、font、surface 等职责分区。
engine_runtime 从平铺 pub mod 逐步转为 domain grouping 文档和稳定 facade。
测试随模块迁移，数量不减少，语义不改变。
```

## 3. 现有规则继承

继续继承以下已确认规则：

```text
EditorUiModel 是 UI 状态真相。
SelfUiRenderer 只生成 DrawCommand / HitRegion。
editor_input 只做输入路由，不执行业务。
EditorSession 是编辑器应用状态与服务协调入口，但不应该长期成为所有领域实现的大文件。
EditorCommandRegistry / EditorCommandExecutor 是命令真相和执行入口。
editor_wgpu_renderer 只负责 GPU draw plan / present / diagnostics，不理解业务。
editor_window_winit 只负责 OS window / event loop / input forwarding / present orchestration。
egui 不进入正式 UI backend 主线。
```

相关文档：

```text
121-Native-Editor-Application-Shell方案.md
127-Editor-Interaction-Feedback-Command-Availability-v1方案.md
149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
150-AI-first-Editor-Command-Framework-C-min方案.md
阶段完成记录/2026-07-02-AI-first-Editor-Command-Framework-C-min/00-总览.md
```

## 4. 其他引擎参考

### 4.1 Unreal Engine

UE 有大量大系统，但编辑器复杂度通过模块边界分散：

```text
Slate / UICommandList / UIAction 负责 UI 命令与可执行性。
LevelEditor 负责关卡编辑器工作区和主编辑器操作。
AssetTools / ContentBrowser / DetailsView / MessageLog 等分别承担独立编辑器领域。
Widget 不直接承载全部业务，通常通过 command / transaction / editor domain 执行。
```

对我们的启发：

```text
可以保留统一 facade，但真实实现必须进入领域模块。
命令、窗口、面板、渲染、资产、场景、事务不能无限堆在一个入口文件。
```

### 4.2 Unity

Unity Editor 的典型组织方式：

```text
EditorWindow / HostView / GUIView 负责窗口和宿主。
Inspector / SerializedObject / SerializedProperty 负责属性编辑。
UI Toolkit / VisualElement 负责 UI 结构。
Undo / dirty / ApplyModifiedProperties 负责编辑事务。
```

对我们的启发：

```text
用户看见的是统一编辑器，但内部职责不能混在一起。
窗口宿主、UI 数据、属性系统、事务系统应分层。
```

### 4.3 Bevy

Bevy 的核心启发不是编辑器，而是模块扩展方式：

```text
Plugin / Schedule / System 让功能按领域接入。
crate 可以有大入口，但功能扩展通过清晰 system/plugin 边界组织。
```

对我们的启发：

```text
engine_runtime 可以保持 crate 统一，但应该有 domain grouping 和稳定 facade。
不要让所有运行时模块在 lib.rs 里长期无分组平铺。
```

## 5. 目标结构

### 5.1 editor_core

目标结构：

```text
editor_core/src/lib.rs
editor_core/src/session.rs
editor_core/src/session_facade.rs
editor_core/src/transaction.rs
editor_core/src/command/
  mod.rs
  model.rs
  registry.rs
  executor.rs
editor_core/src/services/
  mod.rs
  project_service.rs
  scene_service.rs
  asset_service.rs
  prefab_service.rs
  property_service.rs
  build_service.rs
  play_service.rs
  ai_service.rs
editor_core/src/ui_model_composer/
  mod.rs
  toolbar.rs
  launcher.rs
  project_browser.rs
  workspace.rs
  hierarchy.rs
  inspector.rs
  viewport.rs
  console.rs
  runtime_trace.rs
  ai_panel.rs
```

规则：

```text
EditorSession 保留为 public facade。
EditorSession 不再新增大块领域实现。
新增编辑器领域能力必须落入 services 或 ui_model_composer。
命令入口继续走 command executor。
```

### 5.2 editor_window_winit

目标结构：

```text
editor_window_winit/src/lib.rs
editor_window_winit/src/shell/
  mod.rs
  application.rs
  report.rs
editor_window_winit/src/dock/
  mod.rs
  panel_registry.rs
  layout.rs
editor_window_winit/src/input/
  mod.rs
  route.rs
  shortcut.rs
  runtime_event.rs
editor_window_winit/src/viewport/
  mod.rs
  host.rs
  scene_route.rs
  game_route.rs
editor_window_winit/src/present/
  mod.rs
  ui_present.rs
  runtime_present.rs
editor_window_winit/src/window/
  mod.rs
  headless.rs
  real.rs
  dialog.rs
```

规则：

```text
lib.rs 只 re-export public API。
NativeEditorApplication 放入 shell/application.rs。
PanelRegistry / DockLayoutManager 放入 dock。
input route 和 runtime input conversion 放入 input。
真实窗口和 headless window 分离。
```

### 5.3 editor_ui_model

目标结构：

```text
editor_ui_model/src/lib.rs
editor_ui_model/src/model.rs
editor_ui_model/src/command.rs
editor_ui_model/src/launcher.rs
editor_ui_model/src/workspace.rs
editor_ui_model/src/panels.rs
editor_ui_model/src/hierarchy.rs
editor_ui_model/src/inspector.rs
editor_ui_model/src/viewport.rs
editor_ui_model/src/console.rs
editor_ui_model/src/ai_panel.rs
editor_ui_model/src/build_export.rs
editor_ui_model/src/diagnostics.rs
editor_ui_model/src/asset_browser.rs
editor_ui_model/src/input_mapping.rs
```

规则：

```text
UI model 只放数据结构和纯 helper。
payload -> command id helper 继续保留在 command.rs。
不能引入 editor_core 依赖。
```

### 5.4 editor_ui_renderer

目标结构：

```text
editor_ui_renderer/src/lib.rs
editor_ui_renderer/src/draw_list.rs
editor_ui_renderer/src/layout.rs
editor_ui_renderer/src/hit_test.rs
editor_ui_renderer/src/theme.rs
editor_ui_renderer/src/panels/
  mod.rs
  launcher.rs
  toolbar.rs
  hierarchy.rs
  inspector.rs
  viewport.rs
  console.rs
  ai_panel.rs
  project_browser.rs
  build_export.rs
```

规则：

```text
SelfUiRenderer 保留为 public facade。
具体 panel 绘制进入 panels。
hit test 独立，不与绘制逻辑混在一起。
不执行业务命令。
```

### 5.5 editor_wgpu_renderer

目标结构：

```text
editor_wgpu_renderer/src/lib.rs
editor_wgpu_renderer/src/render_graph.rs
editor_wgpu_renderer/src/draw_plan.rs
editor_wgpu_renderer/src/font_system.rs
editor_wgpu_renderer/src/texture_atlas.rs
editor_wgpu_renderer/src/headless.rs
editor_wgpu_renderer/src/real_wgpu.rs
editor_wgpu_renderer/src/surface.rs
editor_wgpu_renderer/src/diagnostics.rs
```

规则：

```text
HeadlessUiGpuRenderer 和 RealWgpuUiRenderer 可保留 public API。
WGPU 细节不能泄漏到 editor_core / editor_ui_model。
FontSystem / atlas / surface / draw plan 分离。
```

### 5.6 engine_runtime

目标结构先以文档和 facade 分组为主，避免一次性移动 60 多个模块：

```text
engine_runtime/src/lib.rs
engine_runtime/src/domain/
  mod.rs
  ecs.rs
  frame_loop.rs
  render.rs
  asset.rs
  input.rs
  physics.rs
  package.rs
  validation.rs
```

第一版规则：

```text
先建立 domain grouping facade，不强制移动全部 runtime 文件。
新增 runtime 模块必须归属到一个 domain。
后续再按 domain 渐进移动文件。
```

## 6. 施工分期

### 阶段 A：建立治理骨架和门禁

目标：

```text
新增结构健康检查脚本或测试。
记录当前文件体量基线。
建立 architecture hygiene report。
不迁移业务行为。
```

验收：

```text
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_window_winit
```

### 阶段 B：拆 editor_window_winit

目标：

```text
优先拆 Native Editor shell，因为真实窗口和输入问题会直接影响用户体验。
保持 public API 不变。
```

验收：

```text
cargo test -p editor_window_winit
cargo test -p editor_host
```

### 阶段 C：拆 editor_ui_model

目标：

```text
把纯数据模型按领域拆分。
保持序列化结构和 public type 不变。
```

验收：

```text
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_input
```

### 阶段 D：拆 editor_ui_renderer

目标：

```text
按 panel / draw_list / hit_test / theme 拆 renderer。
保持 DrawList / HitRegion 输出不变。
```

验收：

```text
cargo test -p editor_ui_renderer
cargo test -p editor_input
cargo test -p editor_window_winit
```

### 阶段 E：拆 editor_wgpu_renderer

目标：

```text
按 render graph / draw plan / font / atlas / surface / diagnostics 拆 WGPU UI renderer。
```

验收：

```text
cargo test -p editor_wgpu_renderer
cargo test -p editor_window_winit
```

### 阶段 F：拆 editor_core / EditorSession service

目标：

```text
保持 EditorSession facade。
逐步把 project / scene / build / play / ai / ui model composer 迁入领域 service。
不改变命令入口。
```

验收：

```text
cargo test -p editor_core
cargo test -p editor_window_winit
```

### 阶段 G：engine_runtime domain grouping

目标：

```text
先建立 runtime domain grouping facade。
只在低风险模块上做文件移动。
```

验收：

```text
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

### 阶段 H：测试迁移与整体回归

目标：

```text
把大文件内部测试逐步迁移到对应模块。
保留测试数量和语义。
生成最终 hygiene report。
```

验收：

```text
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_ui_renderer
cargo test -p editor_wgpu_renderer
cargo test -p editor_input
cargo test -p editor_window_winit
cargo test -p engine_runtime
cargo test -p runtime_player_winit
```

## 7. 不做什么

```text
不新增游戏功能。
不改变用户可见编辑器行为。
不改变 Runtime 数据真相层。
不把项目侧 gameplay 概念引入引擎底座。
不删除测试。
不为了降行数而制造无意义 wrapper。
不一次性移动所有 runtime 文件。
```

## 8. 验收指标

第一版目标指标：

```text
editor_window_winit/src/lib.rs 降到 facade 级别。
editor_ui_model / editor_ui_renderer / editor_wgpu_renderer 不再是单文件 crate。
editor_core/src/lib.rs 不再继续新增领域实现。
新增功能有明确 domain 归属。
现有测试数量不减少。
整体回归通过。
149 Q16-Q20 状态更新为已施工/已验证或阶段性已验证。
```

## 9. 方案自审

```text
Specification fit:
  满足用户选择完整方案 C 的要求：确认长期最优结构，不只做轻量规则。

Rule fit:
  继承已有 EditorCommandFramework、Native Editor Shell、UI/RHI 分层规则，不新增项目玩法 API。

Textual consistency:
  问题、目标结构、施工阶段和验收命令一致，明确哪些阶段移动文件，哪些阶段只建 facade。

Design fit:
  强化 AI-first、复杂项目维护和长期可修改性，避免功能继续堆进大文件。

Implementation feasibility:
  通过小阶段迁移和每阶段测试降低风险；保留 facade 和 public API 兼容，避免一次性破坏调用链。

Practical reasonableness:
  完整目标不妥协，但施工不大爆炸；先建立门禁，再逐步拆核心高风险文件。
```

结论：

```text
本方案通过自审，可以生成施工文档并按阶段开始施工。
```
