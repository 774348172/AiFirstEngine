# 216-当前可自动化施工文档-AUI Runtime Navigation / Screen Flow / Text Entry Productization v1

## 1. 施工目标

按 `216-AUI-Runtime-Navigation-ScreenFlow-TextEntry-Productization-v1方案.md` 的 `C-full-guarded` 落地第一版真实链路：

```text
Submit / Cancel priority
Screen stack / default focus / focus restore C-min
Keyboard + gamepad normalized UI intent C-min
InputField single-line text editing C-min
RuntimeInputEvent TextInput / IME schema
runtime_player_winit TextInput / IME / modifiers 接入
runtime_player_winit summary evidence
project_e2e_gate complex shooter report
```

本施工必须吸收 `其它AI审查目录/34-216-AUI-Runtime-Navigation-ScreenFlow-TextEntry方案审查.md`：

```text
Gate A 修复 modifiers 链路。
Gate A/E 扩展 AuiBindingTarget::InputFieldText。
Gate A 扩展 authoring 新字段入口。
Gate C 明确 screen flow 走 canvas runtime visible。
Gate D gamepad v1 只做 schema + synthetic/headless input，不做完整设备生态。
Gate F 新增 winit Character / IME 转换。
Report 标注 ime_platform_coverage / focusable_derived_from_interactable。
```

## 2. 涉及文件

预计修改：

```text
rust/crates/engine_input/src/input_mapping.rs
rust/crates/engine_input/src/device_state.rs
rust/crates/engine_runtime/src/aui.rs
rust/crates/runtime_player_winit/src/lib.rs
rust/crates/editor_core/src/aui_authoring.rs
rust/crates/project_e2e_gate/src/*
框架设计/引擎总体架构/216-AUI-Runtime-Navigation-ScreenFlow-TextEntry-Productization-v1方案.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-07-AUI-Runtime-Navigation-ScreenFlow-TextEntry-Productization-v1/00-总览.md
```

禁止修改：

```text
不要新增 Logic Ownership Router / Architecture Guard。
不要把 AUI Node 改成 Runtime ECS Entity。
不要把 UI state machine 放入 IR。
不要把 Player / Enemy / Weapon / Equipment 等项目语义写进 engine_runtime。
不要回退当前工作区里与本施工无关的既有改动。
```

## 3. Gate A：schema / modifiers / binding / authoring / report

任务：

```text
RuntimeInputEvent 新增 TextInput / ImePreedit / ImeCommit / ImeCancel / GamepadButton / GamepadAxis2d C-min。
RuntimeInputFrame.modifiers 由 device_state 填充，不再恒空。
AuiCanvas 增加 visible / screen_id / default_focus_node_id / cancel_action_id / submit_action_id。
AuiNode 增加 focusable / placeholder / max_length / read_only / submit_behavior。
AuiBindingTarget 增加 InputFieldText。
AuiCommandKind / AuiActionEvent 增加 Submit / TextChanged / TextSubmitted / TextCancelled。
新增 AuiUiIntent / AuiInputMode / AuiScreenStackState / AuiInputFieldState / AuiRuntimeNavigationScreenFlowTextEntryProductizationReport。
editor_core::aui_authoring 支持新 schema_path C-min。
```

测试：

```powershell
cd rust
cargo test -p engine_input text_input
cargo test -p engine_runtime aui_navigation_screenflow_textentry_schema
cargo test -p editor_core aui_authoring
```

## 4. Gate B：Submit / Cancel priority

任务：

```text
KeyDown Enter / Space 归一为 Submit。
KeyDown Escape 归一为 Cancel。
Gamepad South/A 归一为 Submit；East/B 归一为 Cancel。
TextEditing -> Modal -> ScreenStack -> gameplay pass-through 优先级生效。
Submit 对 focused interactable node 生成 Submit command/action。
Cancel 在 TextEditing 中先生成 TextCancelled，不误 PopScreen。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_submit_cancel
```

## 5. Gate C：Screen stack / canvas runtime visible / focus restore

任务：

```text
AuiScreenStackState 支持 push / pop / replace C-min。
screen push 通过 AuiInteractionState 的 canvas visibility override 使目标 canvas visible。
screen pop 隐藏 top canvas，并恢复 previous focus。
default_focus_node_id 生效。
Cancel 在 ScreenStack 中生成 back / pop evidence。
layout / hit-test / present 遵守 canvas effective visible。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_screen_flow
```

## 6. Gate D：keyboard / gamepad navigation intent

任务：

```text
键盘方向键 / Tab 继续走 215 navigation。
Gamepad DPad / axis synthetic input 转 Move intent。
Move intent 复用 215 focus navigation 和 focus-visible auto-scroll。
Action prompt C-min 进入 report，不做商业级 action bar。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_gamepad_navigation
```

## 7. Gate E：InputField single-line text editing

任务：

```text
PointerDown / Submit 命中 InputField 进入 TextEditing。
维护 original_text / draft_text / caret_index / selection_anchor / selection_focus / composition。
支持 TextInput、Backspace、Delete、Left、Right、Home、End。
支持 SelectAll / ShiftLeft / ShiftRight C-min。
TextChanged / TextSubmitted / TextCancelled 生成 command/action/report evidence。
InputFieldText binding 可把 snapshot string 回流到 InputField text。
```

测试：

```powershell
cd rust
cargo test -p engine_runtime aui_input_field_text_edit
```

## 8. Gate F：IME schema and winit evidence

任务：

```text
runtime_player_winit 接入 WindowEvent::ModifiersChanged。
runtime_player_winit 接入 Character / Key::Character / WindowEvent::Ime C-min，转换为 TextInput / ImePreedit / ImeCommit / ImeCancel。
不把文本输入污染为普通 KeyDown uppercase。
AUI Runtime 在 TextEditing mode 消费 ImePreedit / ImeCommit / ImeCancel。
Report 输出 ime_preedit_count / ime_commit_count / ime_cancel_count / ime_platform_coverage。
```

测试：

```powershell
cd rust
cargo test -p engine_input ime
cargo test -p runtime_player_winit aui_text_ime
cargo test -p engine_runtime aui_ime
```

## 9. Gate G：runtime_player_winit / project_e2e_gate

任务：

```text
NativeAuiPresentSummary 输出 216 evidence：
  submit_count / cancel_count / screen_stack_push_count / screen_stack_pop_count
  text_edit_session_count / text_changed_count / text_submitted_count / text_cancelled_count
  ime_preedit_count / ime_commit_count / ime_cancel_count
  action_prompt_reported / ime_platform_coverage

project_e2e_gate 新增 complex-shooter-aui-runtime-navigation-screenflow-textentry-productization-report.json。
```

测试：

```powershell
cd rust
cargo test -p runtime_player_winit aui
cargo test -p project_e2e_gate aui_runtime_navigation_screenflow_textentry
cargo test -p project_e2e_gate
```

## 10. 整体回归

全部 Gate 通过后运行：

```powershell
cd rust
cargo fmt --check
cargo test -p engine_input
cargo test -p engine_runtime aui
cargo test -p runtime_player_winit
cargo test -p editor_core aui_authoring
cargo test -p project_e2e_gate
```

## 11. 完成后文档同步与归档

完成后必须：

```text
生成阶段完成记录：
  阶段完成记录/2026-07-07-AUI-Runtime-Navigation-ScreenFlow-TextEntry-Productization-v1/00-总览.md

更新入口：
  49-当前状态与下一步入口.md
  54-当前可自动化施工总入口.md
  施工文档/README.md
  阶段完成记录/README.md

移动施工文档：
  从 施工文档/当前/
  到 施工文档/已完成/
```

## 12. 施工文档自审

```text
范围没有扩大为完整 CommonUI / Slate / TextEdit 克隆。
范围没有跳过 34 号审查提出的高严重度前置。
每个 Gate 都有测试命令。
Screen stack 是 runtime transient state，不写回 AUI Document。
Text draft / composition 是 runtime transient state，不写回 AUI Document。
InputFieldText binding 只负责显示回流，不让 ProjectUiStateSnapshot 成为编辑 buffer。
Gamepad v1 只做 normalized intent C-min，不承诺完整设备生态。
IME v1 只做 schema + preedit/commit/cancel evidence，不承诺候选窗和全平台完整体验。
```
