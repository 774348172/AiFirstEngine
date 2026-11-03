# 182-当前可自动化施工文档-M16-AI-Project-Patch-Entry-C-min

## 1. 施工目标

对应方案文档：

```text
框架设计/引擎总体架构/181-M16-AI-Project-Patch-Entry-C-min方案.md
```

本次施工目标是在现有 `UiCommand + EditorSession + CommandTransaction + AI proposal` 之上新增 `ProjectPatch` 意图层。

核心目标：

```text
Natural Language / Test
  -> ProjectPatchPlan / ProjectPatchDocument
  -> Validate
  -> Review / Diff
  -> PatchApplier
  -> UiCommandPayload sequence
  -> EditorSession / CommandTransaction / Domain Service
  -> PatchApplyReport
  -> PatchHistory / Revert
```

本次不是重做编辑器事务系统，不新增第二套执行器，不绕开已有命令框架。

## 2. 第一版边界

本次真实落地：

```text
Scene patch:
  CreateEntity
  DeleteEntity
  RenameEntity
  SetTransform
  SetComponentField
  PlaceAssetIntoScene

Input patch:
  CreateDefaultInputMapping
  AddInputAction
  AddInputBinding
  SetInputBindingDevicePath
```

本次不落地：

```text
真实 LLM 接入
Asset / Prefab / AUI / Rule patch 真实应用
完整 diff viewer
完整 rollback UI
跨文件 conflict merge
插件式 PatchOperation 扩展
```

`PlaceAssetIntoScene` 限制：

```text
只允许引用已有 AssetRef。
不允许导入新资源。
不允许修改 AssetDatabase。
不允许生成 Asset metadata。
```

## 3. 禁止事项

```text
禁止 AI / ProjectPatch 直接写 JSON 文件。
禁止 AI / ProjectPatch 直接修改 EditorSession 内部字段。
禁止 AI / ProjectPatch 直接修改 Runtime World。
禁止 PatchOperation handler 绕开 CommandTransaction。
禁止为复杂打飞机新增 Player / Enemy / Bullet / Health / Score / Weapon 等引擎专用 API。
禁止把 PatchOperation 退化成 UiCommandPayload 别名。
```

## 4. 施工 Gate

### Gate 1: project_patch 数据模型

新增 `editor_core/src/project_patch/`：

```text
mod.rs
model.rs
validator.rs
applier.rs
history.rs
```

数据结构：

```text
ProjectPatchDocument / ProjectPatchPlan
PatchOperation
ScenePatchOperation
InputPatchOperation
PatchValidationReport
PatchReviewModel
PatchApplyReport
PatchHistory
PatchOperationStatus
PatchRiskLevel
PatchCapability
```

测试：

```text
cargo test -p editor_core project_patch_model
```

### Gate 2: PatchValidator

只读验证：

```text
active project / scene / input context
operation count <= 32
domain 只允许 Scene / Input
entity exists / create can allocate id
component field path 非空
input action id unique
depends_on 存在
同 patch 不删除后继续修改同一 entity
同 patch 不重复创建 input action
禁止 gameplay-specific engine API
```

测试：

```text
cargo test -p editor_core project_patch_validator
```

### Gate 3: PatchApplier

职责：

```text
PatchOperation -> Vec<UiCommandPayload>
```

必须复用现有命令：

```text
Scene -> CreateSceneEntity / DeleteSceneEntity / RenameSceneEntity / SetSceneTransform / SetSceneComponentField / PlaceAssetIntoScene
Input -> CreateDefaultInputMapping / AddInputAction / AddInputBinding / SetInputBindingDevicePath
```

测试：

```text
cargo test -p editor_core project_patch_applier
```

### Gate 4: execute_patch_as_transaction

新增入口：

```text
EditorSession::execute_patch_as_transaction(ProjectPatchDocument) -> PatchApplyReport
```

规则：

```text
先 Validate。
Validate 失败不产生任何项目修改。
Validate 通过后 PatchApplier 展开 UiCommandPayload 序列。
通过现有 execute_ui_payload_as_editor_command 逐条执行。
若中途失败，执行 inverse_patch 回滚已提交操作。
若无法生成 inverse，Validate 阶段拒绝。
```

注意：

```text
本 Gate 不新建第二套 CommandTransaction。
第一版用 inverse operation 保证 patch 级原子语义。
```

测试：

```text
cargo test -p editor_core project_patch_transaction
```

### Gate 5: PatchHistory + Revert

记录：

```text
patch_id
applied_at
original_patch
inverse_patch
apply_report
```

Revert：

```text
inverse_patch -> Validate -> Apply
```

测试：

```text
cargo test -p editor_core project_patch_history
```

### Gate 6: AI Panel mock planner 接入

第一版不接真实 LLM。

改造方向：

```text
mock planner 生成 ProjectPatchPlan。
AI proposal 仍可使用现有 UI 入口。
接受 proposal 后进入 execute_patch_as_transaction。
```

为了避免本次过度修改 UI 模型，第一版可以保留 `AiProposedCommand` 对外形状，但内部必须通过 `ProjectPatch` 生成执行命令。

测试：

```text
cargo test -p editor_core ai_project_patch
```

### Gate 7: headless end-to-end

验收：

```text
1. 创建 scene entity patch 成功。
2. 添加 input action + binding patch 成功。
3. scene + input 组合 patch 成功。
4. missing entity patch 被拒绝，且不修改文档。
5. duplicate input action patch 被拒绝。
6. patch revert 能删除创建的 entity 和 input action/binding。
```

测试：

```text
cargo test -p editor_core project_patch
cargo test -p editor_core
```

## 5. 预期代码改动

```text
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/project_patch/validator.rs
rust/crates/editor_core/src/project_patch/applier.rs
rust/crates/editor_core/src/project_patch/history.rs
rust/crates/editor_core/src/tests/mod.rs
rust/crates/editor_core/src/tests/project_patch_tests.rs
```

原则：

```text
尽量不改 editor_ui_model。
尽量不改现有 command registry。
不重构无关系统。
```

## 6. 自审

合乎规格：

```text
对应 181-M16 方案，只做 C-min 第一版。
```

合乎规则：

```text
大系统施工，不把问题拆成无穷小点。
保持 AI 友好、复杂项目可维护、后期可扩展、规则简洁。
```

合乎方案文字本身：

```text
ProjectPatch 是意图层。
UiCommandPayload 是执行层。
EditorSession / CommandTransaction 是正式执行通道。
Scene / Input 是第一版真实应用边界。
```

方便实现：

```text
复用现有 SceneEditTransaction、InputMappingAuthoringService、execute_ui_payload_as_editor_command。
不要求本次重做 UI proposal 模型。
```

合理、能实现：

```text
先实现 headless ProjectPatch 闭环，再用 mock planner 接入 AI Panel。
每个 Gate 都有最小测试，失败可以局部定位。
```

结论：

```text
施工文档通过自审，可以开始施工。
```

## 7. 施工完成记录

完成时间：

```text
2026-07-03
```

实际完成：

```text
Gate 1: 已完成 project_patch 数据模型。
Gate 2: 已完成 Scene/Input PatchValidator。
Gate 3: 已完成 PatchApplier -> UiCommandPayload。
Gate 4: 已完成 EditorSession::execute_patch_as_transaction。
Gate 5: 已完成 PatchHistory + inverse Revert 最小闭环。
Gate 6: 已完成 AI Panel create mock planner 的 ProjectPatch 生成路径。
Gate 7: 已完成 headless ProjectPatch end-to-end 测试。
```

实际代码落点：

```text
rust/crates/editor_core/src/project_patch/mod.rs
rust/crates/editor_core/src/project_patch/model.rs
rust/crates/editor_core/src/project_patch/validator.rs
rust/crates/editor_core/src/project_patch/applier.rs
rust/crates/editor_core/src/project_patch/history.rs
rust/crates/editor_core/src/project_patch/session.rs
rust/crates/editor_core/src/services/ai_service.rs
rust/crates/editor_core/src/tests/project_patch_tests.rs
rust/crates/editor_core/src/tests/ai_service_tests.rs
```

测试结果：

```text
cargo test -p editor_core project_patch
  9 passed

cargo test -p editor_core ai_project_patch
  1 passed

cargo test -p editor_core
  201 passed
```

本次保留的现实边界：

```text
AI Panel 对外仍保留 AiProposedCommand / UiCommandPayload 形状，避免本次大面积改 UI 模型。
但 create mock planner 已经先生成 ProjectPatchDocument，再通过 PatchApplier 展开为 UiCommandPayload。

Input inverse 为了实现可回滚，ProjectPatch 内部增加 RemoveInputAction / RemoveInputBinding。
它们用于 inverse patch，不代表第一版 AI forward patch 要开放删除类 Input 修改。

CreateDefaultInputMapping / DeleteEntity 的 inverse 需要文件或子树 snapshot，C-min 当前会在 Validate/inverse 阶段拒绝这类不可逆 forward patch。
```

完成结论：

```text
M16 AI Project Patch Entry C-min 的核心 headless 链路已经落地。
第一版已经能表达、验证、展开、应用、记录、回滚 Scene/Input patch。
本次没有新增项目专用 gameplay API。
本次没有新建第二套编辑器事务系统。
```
