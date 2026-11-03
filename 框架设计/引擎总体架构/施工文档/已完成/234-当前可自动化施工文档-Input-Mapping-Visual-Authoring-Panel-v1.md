# 234-当前可自动化施工文档-Input Mapping Visual Authoring Panel v1

> 状态：已完成归档。  
> 施工日期：2026-07-10。  
> 对应正式方案：`../../234-Input-Mapping-Visual-Authoring-Panel-v1方案.md`。  
> 采用方案：`B-min+: Unified Input Mapping Workspace`。  
> 外部审查：未提供针对 234 的外部方案审查文档；正式方案无需修改。

## 0. 施工目标

把现有 Input Mapping 的 schema、authoring service 和 runtime resolver 串成用户可操作的编辑器产品面：

```text
InputMappingAsset 唯一项目真相
  -> InputMappingEditorState working copy
  -> Existing Workspace Input Mapping panel
  -> InputMappingEditCommand
  -> validate / conflict / impact
  -> source hash guarded atomic save
  -> RuntimePackage
  -> InputResolver / ActionSnapshot
```

本轮必须交付：

```text
input-mapping.v2 与稳定 binding_id；兼容读取 v1 并确定性迁移。
结构化 InputControlCatalog。
Context / Action / Binding / Trigger / Processor 可视化编辑模型和命令。
editor-only working copy、选择、dirty、Save / Discard、source hash 冲突保护。
现有 Workspace 中的 Native Input Mapping 产品面和稳定 hit target。
键盘/鼠标 capture；gamepad/South、gamepad/LeftStick 目录选择和 synthetic preview。
Context 冲突、类型兼容、引用影响诊断。
复用真实 InputResolver 的 deterministic preview。
现有 Report Panel provider，Editor Off / Summary / Trace 分档。
复杂打飞机样例从编辑到 RuntimePackage / runtime resolve 的自动化证据。
```

## 1. 不得突破的边界

```text
InputMappingAsset 仍是唯一持久化项目真相。
InputMappingEditorState 不落盘为第二资产，不进入 RuntimePackage。
UI widget 不直接写文件；修改统一进入 InputMappingEditCommand / service。
RuntimePackage 不是编辑真相，Runtime 只消费 package 中的 mapping。
binding_index 仅保留 legacy/internal 兼容，不再作为 UI/AI 长期身份。
enabled_by_default 当前 resolver 不消费，本轮不得暴露为可用编辑项。
Preview 必须调用正式 InputResolver，不另造简化求值器。
Runtime 默认不生成 Visual Authoring report。
不新增 Input Graph、ControlSchemeAsset、动态 context stack、runtime rebinding、多人设备所有权。
不实现真实物理手柄 capture；只做受支持目录选择和 synthetic preview。
不新增独立 OS window，产品面必须进入现有 Workspace。
不把复杂打飞机 action id 写进通用 engine core API。
```

## 2. 预计涉及文件

核心 schema/runtime：

```text
rust/crates/engine_input/src/input_mapping.rs
rust/crates/engine_input/src/lib.rs
```

编辑器模型、命令和服务：

```text
rust/crates/editor_ui_model/src/input_mapping.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/lib.rs
rust/crates/editor_core/src/input_mapping_authoring.rs
rust/crates/editor_core/src/services/project_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/authoring_workspace.rs
```

原生 UI / 输入路由：

```text
rust/crates/editor_ui_renderer/src/panels/workspace.rs
rust/crates/editor_ui_renderer/src/lib.rs
rust/crates/editor_input/src/lib.rs
rust/crates/editor_window_winit/src/command_system.rs
rust/crates/editor_window_winit/src/input_route.rs
```

诊断、报告、E2E 与样例：

```text
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/project_patch/*
rust/crates/project_e2e_gate/src/*
samples/complex_shooter_project/Input/input.default.json
```

允许根据现有模块边界增减文件，但不得为 234 新建独立长期架构层。

## 3. Gate A - Schema v2、稳定身份与设备目录

任务：

```text
1. 将正式规范升级为 input-mapping.v2。
2. 为 InputBindingDefinition 增加持久化 binding_id。
3. loader 接受 v1/v2；v1 缺失 id 时按稳定输入生成 deterministic migration id。
4. normalization / save 输出 v2；RuntimePackage 使用同一 normalization。
5. 保持现有 v1 JSON 和 ProjectPatch binding_index 操作可兼容。
6. 增加结构化 InputControlCatalog，仅发布 resolver 真实支持的路径。
```

Gate 测试：

```powershell
cargo test -p engine_input input_mapping
cargo test -p editor_ui_model input_mapping
```

通过条件：

```text
旧 v1 样例可加载并产生跨重复加载稳定的 binding_id。
新保存产物为 input-mapping.v2 并持久化 binding_id。
v2 round-trip 不改变身份。
Runtime 输入求值结果不因 binding_id 改变。
设备目录与 RuntimeInputEvent / resolver 支持路径一致。
```

## 4. Gate B - Working Copy 与完整编辑命令

任务：

```text
1. 新增 editor-only InputMappingEditorState：选择、draft、source_hash、dirty、capture、preview。
2. Open 从项目资产建立 working copy；普通 UI edit 只改 draft，不立即写盘。
3. Save 执行 validate -> expected hash -> normalize -> atomic commit -> refresh。
4. Discard 从磁盘重新加载并清空 dirty/capture/preview。
5. 补 Context、Action value type、Binding trigger/processor 的编辑命令。
6. 绑定编辑以 binding_id 为主；legacy binding_index 继续可解析。
7. ProjectPatch 继续走 load -> hash guard -> apply -> validate -> commit/report，不依赖打开 UI。
```

Gate 测试：

```powershell
cargo test -p editor_ui_model input_mapping
cargo test -p editor_core input_mapping
```

通过条件：

```text
draft edit 不修改源文件。
Save 后源文件和 model 同步，dirty 清零。
Discard 恢复磁盘内容。
外部修改导致 source hash 不一致时拒绝覆盖并给出结构化 diagnostic。
所有本轮允许字段都可通过 command/service 修改并验证。
```

## 5. Gate C - Native Workspace 与键鼠 Capture

任务：

```text
1. 在现有 Workspace Input domain 绘制 toolbar / Context / Action+Binding / Properties / Diagnostics+Preview。
2. 使用稳定、可测试的 hit target 和 command payload。
3. Asset Browser / workflow 能打开指定 InputMappingAsset。
4. 支持键盘/鼠标 capture，生成规范化 device_path。
5. capture active 时先于快捷键和 GameView 路由消费输入。
6. Escape 取消、窗口失焦取消；PointerMove 只允许绑定到 Pointer action。
7. gamepad 本轮只展示受支持目录选择，不宣称物理 capture。
```

Gate 测试：

```powershell
cargo test -p editor_ui_renderer input_mapping
cargo test -p editor_input input_mapping
cargo test -p editor_window_winit input_mapping
```

通过条件：

```text
用户可在 Workspace 完成选择、增删、属性编辑、capture、Save、Discard、Preview。
所有命令有稳定 hit target 证据。
capture 不同时触发 editor shortcut 或 GameView runtime input。
布局没有新增独立窗口或第二编辑真相。
```

## 6. Gate D - Diagnostics、Preview 与 Report Panel

任务：

```text
1. 增加结构化 conflict/type/reference impact diagnostics。
2. Context 冲突分析考虑 priority 与 consume_input。
3. RemoveAction 前输出引用影响；存在引用时拒绝或要求显式确认。
4. Draft + synthetic RuntimeInputFrame 调用正式 InputResolver 生成 preview。
5. Preview 给出 matched binding_id、action、phase/value、consume/shadow 和 diagnostics。
6. 向现有 ReportRegistry 注册 Input Mapping authoring provider。
7. Editor report 实现 Off / Summary / Trace；Runtime 不生成 Visual Authoring report。
```

Gate 测试：

```powershell
cargo test -p editor_core input_mapping
cargo test -p editor_core report_panel
cargo test -p engine_input input_mapping
```

通过条件：

```text
同 Context 重复、跨 Context 遮挡、类型不匹配和不支持路径都有稳定 code/source/fix。
Preview 与直接调用 InputResolver 的 ActionSnapshot 一致。
Summary 不携带完整 trace；Trace 可供 Gate 诊断；Off 不生成报告。
```

## 7. Gate E - 复杂打飞机 E2E

任务：

```text
1. 将 complex shooter mapping 规范化为 v2 并持久化 binding_id。
2. 自动化模拟打开 mapping、修改 binding/trigger、预览、Save、重新加载。
3. 证明 build / RuntimePackage 收集保存后的 mapping，而非 editor draft。
4. Runtime Player 加载 package 后用真实 resolver 解析 action.move / fire / pointer / pause。
5. 输出 complex-shooter-input-mapping-visual-authoring-report.v1 证据。
```

Gate 测试：

```powershell
cargo test -p project_e2e_gate input_mapping_visual_authoring
cargo test -p runtime_player_winit input_mapping
```

通过条件：

```text
样例完整链路通过，报告包含 asset/hash/counts/changed paths/preview/runtime evidence。
未保存 draft 不进入 RuntimePackage。
保存后 mapping 在 RuntimePackage 和 Runtime resolve 中生效。
```

## 8. Gate F - 整体回归、文档与归档

整体测试：

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

测试失败时只修复与 234 或本轮暴露的直接回归，不清理无关 dirty worktree。

完成后：

```text
生成 阶段完成记录/2026-07-10-Input-Mapping-Visual-Authoring-Panel-v1/00-总览.md。
更新 49-当前状态与下一步入口.md。
更新 54-当前可自动化施工总入口.md。
更新 施工文档/README.md。
更新 阶段完成记录/README.md。
更新 227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md，将 P1-3 标为完成。
下一默认讨论系统切换到 P1-4 Asset Browser Native Productization v1。
把本施工文档从 当前/ 移到 已完成/。
```

## 9. 施工文档自审

审查结论：`通过，可以施工`。

审查记录：

```text
外部审查：未提供针对 234 的外部方案审查文档。
正式方案：B-min+ 不变，无需修改。
范围：只补 Input Mapping Visual Authoring 闭环，没有扩成完整 Input Suite。
真相：InputMappingAsset / RuntimePackage 边界清晰，没有第二持久化编辑真相。
兼容：明确 v1 loader、deterministic binding_id migration 和 binding_index legacy 兼容。
交互：明确 working copy、Save/Discard、source hash、键鼠 capture 与焦点消费。
运行：Preview 复用真实 InputResolver；未伪造 enabled_by_default 或物理 gamepad capture。
报告：Editor Off/Summary/Trace 分档，Runtime 默认不生成 Visual Authoring report。
测试：每个 Gate 有针对性测试，最后有完整 workspace regression。
归档：完成记录、入口、227 状态和施工文档移动均有明确要求。
```

自审未发现会阻塞施工的冲突。正式施工必须从 Gate A 开始，当前 Gate 测试通过后才能进入下一 Gate。
