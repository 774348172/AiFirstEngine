# 103-当前可自动化施工文档-AUI Interaction System C-min

## 1. 施工目标

按照 `../103-AUI-Interaction-System-C-min方案.md` 实现 AUI Runtime Core 第一版交互底座。

目标不是实现完整 UI 控件系统，而是跑通最小链路：

```text
RuntimeInputFrame
  -> AuiInteractionSystem
  -> HitTest
  -> AuiCommand / consumed / trace
  -> 未消费输入继续进入 InputMapping
```

## 2. 修改范围

### 2.1 engine_runtime::aui

文件：

```text
rust/crates/engine_runtime/src/aui.rs
```

新增类型：

```text
AuiPointer
AuiInteractionEventKind
AuiHitTestReason
AuiHitTestResult
AuiCommandKind
AuiCommand
AuiInteractionTrace
AuiInteractionResult
AuiInteractionSystem
```

新增能力：

```text
AuiInteractionSystem::hit_test(document, layout, x, y)
AuiInteractionSystem::process(document, layout, input_frame)
```

规则：

```text
只处理 RuntimeInputEvent 的 PointerDown / PointerUp / PointerMove。
不处理 keyboard / gamepad。
只命中 visible=true 且 interactable=true 的节点。
按 tree_order 倒序命中，后绘制节点优先。
PointerDown / PointerUp / PointerMove 命中后生成对应 AuiCommand。
PointerUp 命中同一节点时可以生成 Click。
第一版不做 pointer capture，只做同帧最小 click 判断。
```

### 2.2 input_mapping 关系

本阶段不改 `InputResolver` 主逻辑。

AUI 先产出：

```text
AuiInteractionResult.consumed
```

后续窗口层或 Runtime 主循环根据该字段决定是否把输入继续交给 `InputResolver`。

如需要本阶段新增 helper，必须放在 AUI 或独立通用函数里，不能让 `InputResolver` 依赖 AUI。

## 3. 测试要求

新增最小测试：

```text
aui_hit_test_picks_topmost_interactable_node
aui_hit_test_ignores_invisible_or_non_interactable_nodes
aui_interaction_consumes_pointer_down_on_button
aui_interaction_does_not_consume_pointer_outside_ui
aui_interaction_pointer_up_generates_click_when_pressed_same_node
aui_interaction_trace_records_consumed_reason
```

回归测试：

```text
cargo fmt --check
cargo check -p engine_runtime
cargo test -p engine_runtime aui
cargo test -p engine_runtime input_mapping
cargo test --workspace --no-run
```

## 4. 完成标准

```text
AUI 可以判断 pointer 是否命中 UI。
AUI 可以生成最小 AuiCommand。
AUI 可以标记 consumed。
AUI 可以输出 trace。
未命中 UI 的输入不会被 consumed。
不新增项目玩法 API。
不引入完整 focus / drag / navigation 规则。
```

## 5. 完成后整理

施工完成后：

```text
1. 将本文档移动到 施工文档/已完成。
2. 新增 阶段完成记录/2026-06-29-AUI-Interaction-System-C-min/00-总览.md。
3. 更新 施工文档/README.md。
```
