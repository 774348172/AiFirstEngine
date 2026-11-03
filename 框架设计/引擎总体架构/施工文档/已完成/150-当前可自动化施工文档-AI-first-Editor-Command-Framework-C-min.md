# 150-当前可自动化施工文档-AI-first Editor Command Framework C-min

## 1. 施工目标

根据正式方案：

```text
../150-AI-first-Editor-Command-Framework-C-min方案.md
```

本轮施工只完成 C-min 第一阶段：

```text
建立 editor_core 内的 EditorCommandFramework C-min。
集中 command id / descriptor / request / availability / executor。
保留 EditorSession::execute_command 兼容入口，但内部转发到 executor。
让 AI proposal accept 重新进入统一命令执行通道。
减少 editor_core / editor_input / authoring_workspace / property_editing 中 command id 映射重复。
```

本轮不做：

```text
不做完整 UE Slate Command Framework。
不做插件命令系统。
不做完整菜单/快捷键编辑器。
不做完整 service 化大拆分。
不改变 UI 视觉设计。
不改变 Runtime / Build / RHI 主流程。
```

## 2. 涉及文件

预计新增：

```text
rust/crates/editor_core/src/editor_command.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/editor_command_executor.rs
```

预计修改：

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/authoring_workspace.rs
rust/crates/editor_core/src/property_editing.rs
rust/crates/editor_input/src/lib.rs
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/149-当前实现审查问题拆解与逐项解决队列-2026-07-02.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-02-AI-first-Editor-Command-Framework-C-min/00-总览.md
```

## 3. 阶段任务

### 阶段 A：建立命令数据模型与注册表

新增 `editor_command.rs`：

```text
EditorCommandId
EditorCommandDescriptor
EditorCommandCategory
EditorCommandOwnerDomain
EditorCommandPayloadKind
EditorCommandPayload
EditorCommandRequest
EditorCommandAvailability
EditorCommandContext
```

新增 `editor_command_registry.rs`：

```text
EditorCommandRegistry
builtin_editor_command_registry()
command_id_for_payload(payload)
legacy_command_id_for_payload(payload)
descriptor_for_payload(payload)
validate_unique_builtin_command_ids()
```

验收：

```powershell
cargo test -p editor_core command_registry
```

### 阶段 B：建立 executor，并保留兼容入口

新增 `editor_command_executor.rs`：

```text
execute_editor_command(session, request)
ui_command_to_editor_command_request(command)
```

修改 `EditorSession::execute_command`：

```text
旧 public API 保留。
内部转换为 EditorCommandRequest。
再进入 editor_command_executor。
```

验收：

```powershell
cargo test -p editor_core editor_session_execute_command
cargo test -p editor_core command_executor
```

### 阶段 C：收敛重复 command id 映射

修改：

```text
editor_core/src/lib.rs
editor_core/src/authoring_workspace.rs
editor_core/src/property_editing.rs
editor_input/src/lib.rs
```

规则：

```text
command id 真相集中在 editor_core::editor_command_registry。
editor_input 不再维护完整业务 command id 映射。
authoring_workspace / property_editing 不再维护局部 command id 映射。
如受 crate 依赖方向限制，editor_ui_model 可承载纯数据 helper，但不得在多个 crate 重复。
```

如果发现 `editor_input -> editor_core` 会造成依赖倒置，则采用临时但合规的边界：

```text
把纯 payload -> command id helper 放入 editor_ui_model。
editor_core registry 使用同一 helper 生成 descriptor。
editor_input 使用同一 helper 构造 UiCommand。
不得保留第二份 match。
```

验收：

```powershell
cargo test -p editor_input
cargo test -p editor_core command_id
```

### 阶段 D：AI proposal accept 重进统一命令通道

修改 `EditorSession::accept_ai_proposed_command`：

```text
保留 proposal review state 更新。
业务执行不再维护 scene command 子 match。
改为把 proposal payload 重新提交到 EditorCommandExecutor。
descriptor/availability 决定是否允许执行。
```

验收：

```powershell
cargo test -p editor_core ai_accept
cargo test -p editor_core ai_panel
```

### 阶段 E：整体回归与文档归档

执行：

```powershell
cargo fmt --check
cargo test -p editor_core
cargo test -p editor_input
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit
```

完成后：

```text
施工文档/当前/150-...md -> 施工文档/已完成/150-...md
更新 54 当前可施工入口
更新 49 当前状态
更新 149 Q8/Q12/Q13 状态为 已施工/已验证
新增阶段完成记录
```

## 4. 风险与处理

### 4.1 crate 依赖方向风险

`editor_input` 当前依赖 `editor_ui_model`，不应反向依赖 `editor_core`。如果要共享 payload -> command id，优先放到 `editor_ui_model` 作为纯数据 helper。

### 4.2 改动过大风险

本轮不移动 `EditorSession` struct，不做完整 `editor_session.rs` 拆分。先收命令真相层，避免大搬家引入无关风险。

### 4.3 旧测试兼容风险

`command_for_test(payload)` 保留，但内部必须使用统一 command id helper。

### 4.4 旧 snake_case id 兼容

第一版保留旧 snake_case id，不强制迁移成 dot id。正式 descriptor 可以记录 canonical id 与 legacy id 的关系，实际 UI 兼容仍使用现有 id，避免大范围 UI fixture 改动。

## 5. 施工自审

```text
Specification fit:
  覆盖 150 方案中的 C-min 第一阶段，不扩大到完整 Slate/插件/菜单系统。

Rule fit:
  遵守当前文档规则：正式方案先行、施工文档先行、模块测试和整体回归。

Textual consistency:
  文档中的数据模型、注册表、executor、兼容入口和测试阶段一致。

Design fit:
  命令真相层集中，符合 AI-first 和长期维护目标；不引入项目玩法规则。

Implementation feasibility:
  当前已有 UiCommandPayload / EditorSession / CommandResult，可通过兼容 adapter 渐进迁移。

Practical reasonableness:
  只做高收益的命令路由收敛，不做大规模文件搬家，风险可控。
```

结论：

```text
施工文档通过自审，可以开始施工。
```
