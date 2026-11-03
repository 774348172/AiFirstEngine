# Legacy Notice

本文档是 TypeScript / Electron 原型期的历史执行流水记录。

当前正式主线以 Rust Native Runtime / Native Editor Host 为准。本文中的 `src/`、`TypeScriptRuntimeBackend`、`npm.cmd`、`Electron`、`Vite` 等内容只作为历史迁移证据，不是当前施工入口，也不是当前架构规则。

# 当前项目改造计划

本文档记录当前 `AI Game Dev Environment` 代码库如何逐步改造成设计文档中的 AI-first engine 最小实现。

## 2026-06-20 执行更新：阶段 6 补齐：Spin3D System 内部 IR Trace 接入 RuntimeTraceReport

### 长期主义复核

本次目标是让真实 runtime system 内部执行的 IR rule 进入 RuntimeTraceReport，而不是只靠手动 trace smoke。

判断依据：

```text
AI 查错需要看到真实 system 内部执行了哪个 IR rule。
RuntimeTraceReport 应同时承载 ECS system trace 和 IR rule trace。
IR trace 不应依赖全局变量或 UI，应该通过 FrameContext 传递。
```

### 已完成代码改造

修改：

```text
src/engine/frameLoop.ts
  FrameContext 新增 irTrace。
  runFrame 每帧开始清空 context.irTrace。

src/engine/runtime.ts
  RuntimeSession.context 初始化 irTrace。
  新增 getRuntimeIrTrace(states)。

src/engine/systems.ts
  Spin3D 执行 interpretFunctionRule 后，把 result.trace 追加到 context.irTrace。

src/runtime-backends/typescript/TypeScriptRuntimeBackend.ts
  getRuntimeTrace 合并手动 irTrace 和 runtime context irTrace。

scripts/test-runtime-backend.cjs
  starter project tick 后验证 irRules 包含 prototype.spin3d.rotation。
```

文档更新：

```text
框架设计/引擎总体架构/21-Runtime-Core-Boundary.md
框架设计/引擎总体架构/23-IR-Interpreter-MVP.md
```

### 当前边界

已经完成：

```text
真实 runtime system 内部 IR trace 可以进入 RuntimeTraceReport.irRules。
Runtime Trace UI 可以显示 Spin3D 的 prototype.spin3d.rotation rule。
IR trace 通过 FrameContext 传递，不依赖全局状态。
```

暂未完成：

```text
IR trace source map 点击跳转。
更多 prototype systems 接入 IR。
Runtime replay。
Rust AOT 等价测试。
```

### 验证

已执行：

```powershell
npm.cmd run test:runtime
npm.cmd run build
```

结果：

```text
Runtime backend tests passed
tsc passed
editor build passed
runtime bundle build passed
```

### 下一阶段建议

下一步建议：

```text
进入 Rust Runtime MVP 前，先做一次阶段 6 收敛审查。
```

审查重点：

```text
是否还需要接第二个 prototype system。
当前解释器是否足够支撑 Rust Runtime 对照。
是否该开始阶段 7 Rust Runtime MVP。
```

## 2026-06-20 执行更新：阶段 6 补齐：Spin3D Prototype System 受控接入 IR Interpreter

### 长期主义复核

本次目标是选择一个足够小的 prototype system，把真实 runtime system 的一段规则迁到 IR Interpreter。

选择 Spin3D 的原因：

```text
规则小：rotation.y = rotation.y + speed * deltaTime。
不涉及实体生成。
不涉及碰撞。
不涉及资源加载。
不涉及 Native Module。
可以验证 Runtime system 组装输入、IR 解释执行、Runtime 写回 Component 的最小路径。
```

### 已完成代码改造

修改：

```text
src/engine/systems.ts
  Spin3D 不再直接计算 rotation.y。
  新增 prototype.spin3d.rotation Canonical Rule IR。
  spinSystem 组装 input，调用 interpretFunctionRule，再把 output.rotationY 写回 Transform。

scripts/test-runtime-backend.cjs
  starter project 的 Coin tick 后 rotation.y 增量验证。
```

### 当前边界

已经完成：

```text
第一个 prototype gameplay system 受控接入 IR Interpreter。
IR 不直接读写 ECS。
Runtime system 负责查询 ECS、组装 input、执行 IR、写回 Transform。
保持原 Spin3D 行为。
```

暂未完成：

```text
更多 prototype systems 接入 IR。
IR rule 热更新。
Rust AOT 等价测试。
```

### 验证

已执行：

```powershell
npm.cmd run test:runtime
npm.cmd run build
```

结果：

```text
Runtime backend tests passed
tsc passed
editor build passed
runtime bundle build passed
```

观察：

```text
game-runtime/ai-first-runtime.js 从约 18KB 增至约 31KB。
原因是导出 runtime bundle 现在包含 IR Interpreter 及 Spin3D IR rule。
这是预期的阶段性成本，后续 Rust Runtime / AOT 后会重新评估发布包体。
```

### 下一阶段建议

下一步建议：

```text
把 Spin3D IR rule trace 接入 RuntimeTraceReport.irRules
```

原因：

```text
当前 system 行为已经走 IR。
但 system 内部 IR trace 还没有进入 Runtime Trace UI。
为了 AI 查错完整闭环，应让真实 runtime system 内的 IR rule 也能被 Trace 面板看到。
```

## 2026-06-20 执行更新：阶段 6 补齐：IR Interpreter Scenario Test

### 长期主义复核

本次目标是验证 IR 不是只能跑单个规则，而是能承接多个规则协作的小型真实流程。

判断依据：

```text
复杂项目不是一个规则单独运行。
AI 友好要求跨规则 trace 仍能回到 featureId / requirementIds。
在进入 Rust Runtime 前，先用 scenario test 暴露 IR 表达和调试链路问题。
```

### 已完成代码改造

新增：

```text
scripts/test-ir-scenario.cjs
  direction move -> hit -> damage output。
  fatal damage -> DeathCandidate event。
  scenario trace 保留每条规则的 sourceMap。
```

修改：

```text
package.json
  新增 npm.cmd run test:scenario。
```

文档更新：

```text
框架设计/引擎总体架构/23-IR-Interpreter-MVP.md
框架设计/引擎总体架构/18-长期主义实现路径.md
```

### 当前边界

已经完成：

```text
Scenario test 串联 State / Lifecycle Rule 和 Function Rule。
Scenario test 验证 command 输出可以驱动下一步状态。
Scenario test 验证 damage output 可以修改模拟世界。
Scenario test 验证 fatal damage 可以发出 DeathCandidate。
Scenario trace 能追到每条 rule 的 sourceMap。
```

暂未完成：

```text
正式 runtime gameplay system 接入 IR。
Scenario runner 产品化。
Runtime replay。
Rust AOT 等价测试。
```

### 验证

已执行：

```powershell
npm.cmd run test:scenario
npm.cmd run build
```

结果：

```text
IR scenario tests passed
tsc passed
editor build passed
runtime bundle build passed
```

### 下一阶段建议

下一步建议：

```text
选择一个 prototype system 的小规则受控接入 IR Interpreter
```

推荐候选：

```text
GameScore normalization
Spin3D rotation
Projectile lifetime cleanup decision
```

原因：

```text
这些规则足够小，能验证 runtime 接入 IR 的真实路径。
但它们不会立刻牵扯复杂碰撞、生成实体、资源加载或 Native Module。
```

## 2026-06-20 执行更新：阶段 6 补齐：RuntimeBackend Trace 接入 IR Rule

### 长期主义复核

本次目标是让 IR Interpreter 不只存在于单元测试里，而是能被 RuntimeBackend trace report 承载。

判断依据：

```text
AI 友好要求运行时问题能追到 ruleId / sourceMap。
复杂项目维护要求 runtime trace 同时看到 ECS system trace 和 IR rule trace。
当前不应把 IR rule 自动塞进每帧 gameplay system，避免调度边界混乱。
所以先提供显式 trace smoke 通道。
```

### 已完成代码改造

修改：

```text
src/runtime/RuntimeBackend.ts
  RuntimeTraceReport 新增 irRules。

src/runtime-backends/typescript/TypeScriptRuntimeBackend.ts
  新增 irTrace。
  新增 runIrRuleForTrace(rule, input)。
  getRuntimeTrace 返回 irRules。

scripts/test-runtime-backend.cjs
  新增 backend runtime trace contains IR rule entries 测试。
```

文档更新：

```text
框架设计/引擎总体架构/21-Runtime-Core-Boundary.md
框架设计/引擎总体架构/23-IR-Interpreter-MVP.md
```

### 当前边界

已经完成：

```text
RuntimeTraceReport 可以承载 IR rule trace。
TypeScriptRuntimeBackend 可以显式执行 IR rule 并记录 trace。
IR trace 包含 ruleId / path / op / label / sourceMap。
```

暂未完成：

```text
IR rule 自动接入每帧 gameplay system。
Runtime Trace UI 展示 irRules。
Runtime replay。
Interpreter vs Rust AOT equivalence test。
```

### 验证

已执行：

```powershell
npm.cmd run test:runtime
npm.cmd run build
```

结果：

```text
Runtime backend tests passed
tsc passed
editor build passed
runtime bundle build passed
```

### 下一阶段建议

下一步可以进入二选一：

```text
更稳：Runtime Trace UI 展示 irRules
更长期：Rust Runtime MVP
```

如果优先 AI 查错闭环，建议先做 Runtime Trace UI 展示 irRules。

## 2026-06-20 执行更新：阶段 6 补齐：Runtime Trace UI 展示 IR Rule

### 长期主义复核

本次目标是把 backend 中的 IR rule trace 显示到编辑器 Runtime Trace 面板。

判断依据：

```text
AI 查错闭环不能只停在 backend 数据结构。
用户和 AI 都需要在编辑器里看到 ruleId / sourceMap。
当前只做只读展示，不做跳转和复杂调试器。
```

### 已完成代码改造

修改：

```text
src/App.tsx
  RuntimeTraceView 新增 irRules。
  createRuntimeTraceView 通过 runIrRuleForTrace 生成 IR trace smoke。
  RuntimeTracePanel 新增 IR Rules 区域。
  Summary 显示 IR rules 数量。

src/styles.css
  Runtime Trace summary 从 5 列调整为 6 列。
```

文档更新：

```text
框架设计/引擎总体架构/21-Runtime-Core-Boundary.md
框架设计/引擎总体架构/23-IR-Interpreter-MVP.md
```

### 当前边界

已经完成：

```text
Runtime Trace UI 能展示 IR ruleId。
Runtime Trace UI 能展示 trace label / op。
Runtime Trace UI 能展示 IR node path。
Runtime Trace UI 能展示 featureId。
requirementIds 通过 title 暴露。
```

暂未完成：

```text
IR trace source map 点击跳转。
IR rule 自动接入每帧 gameplay system。
Runtime replay。
```

### 验证

已执行：

```powershell
npm.cmd run build
npm.cmd run test:runtime
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
Runtime backend tests passed
```

### 下一阶段建议

下一步建议进入：

```text
Interpreter Scenario Test
```

原因：

```text
当前已有 Function Rule、State/Lifecycle frame_update、Runtime trace、UI 展示。
但还缺一个更贴近真实游戏流程的 scenario test，把多个规则输入输出串起来验证。
这会比立刻进入 Rust Runtime 更能提前暴露 IR 规则表达和调试链路问题。
```

## 2026-06-20 执行更新：长期主义阶段 6 完成：IR Interpreter Function + State/Lifecycle frame_update MVP

### 长期主义复核

本阶段目标是让 Canonical Rule IR 开始具备真实解释执行能力。

判断依据：

```text
优先级 1：AI 友好，所以解释器错误必须带 ruleId / path / sourceMap。
优先级 2：复杂项目可维护，所以先执行 Function Rule，不把 State / Lifecycle 一次做重。
优先级 3：效率与长期路线，所以解释器作为开发 / 验证 / 热更覆盖后端，不替代未来 Rust AOT。
优先级 4：规则简洁，所以 native call / runtime write 暂不开放。
```

### 已完成代码改造

新增：

```text
src/ir/ruleInterpreter.ts
  interpretFunctionRule。
  interpretStateRule。
  支持 input / output / event / command / trace。
  支持 State / Lifecycle frame_update runtime state 输入和输出。
  支持结构化 interpreter error。
  支持 maxOps / maxEmits budget。

scripts/test-ir-interpreter.cjs
  IR Interpreter MVP 测试。

框架设计/引擎总体架构/23-IR-Interpreter-MVP.md
  IR Interpreter MVP 文档。
```

修改：

```text
package.json
  新增 npm.cmd run test:interpreter。

框架设计/引擎总体架构/README.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/18-长期主义实现路径.md
  接入 IR Interpreter MVP 文档并更新阶段状态。
```

### 当前测试覆盖

新增解释器测试：

```text
damage rule interpreter result equals hand-written expected result
event output is emitted
trace links to rule source map
missing input reports source map error
maxOps budget violation reports structured error
lifecycle rule returns unsupported error
direction move frame_update updates runtime state
direction move frame_update emits SetEntityPosition command
state rule requires frame_update hook
```

### 当前边界

已经完成：

```text
Function Rule 可以解释执行。
State / Lifecycle Rule 可以执行 frame_update MVP。
解释器会先跑 IR validator。
解释器错误带 ruleId / path / sourceMap。
解释器支持 output / event / command / trace。
解释器支持 budget maxOps / maxEmits。
解释器返回 runtime state，不直接写 ECS。
```

暂未完成：

```text
Runtime state preservation。
RuntimeBackend trace 接入。
Lowered Execution IR。
Interpreter vs Rust AOT equivalence test。
native call 白名单。
```

### 验证

已执行：

```powershell
npm.cmd run test:interpreter
npm.cmd run build
```

结果：

```text
IR Interpreter MVP tests passed
tsc passed
editor build passed
runtime bundle build passed
```

### 下一阶段建议

建议下一步先做 RuntimeBackend trace 接入，而不是马上进入 Rust Runtime：

```text
RuntimeBackend trace 接入 IR rule
```

原因：

```text
解释器已经能执行 Function Rule 和 State / Lifecycle frame_update。
但 Runtime Trace 还看不到 IR rule 执行记录。
先把 trace 接上，可以更符合 AI 查错和复杂项目维护目标。
```

## 2026-06-20 执行更新：长期主义阶段 5 完成：Canonical Rule IR v1 最小闭环

### 长期主义复核

本阶段目标是固定项目规则真相层，而不是直接实现解释器或把现有玩法系统全部迁移。

判断依据：

```text
Project Schema v1 已完成。
Runtime Core Boundary v1 已完成。
下一步必须先固定 IR v1，否则解释器 / Rust AOT / AI 生成规则都会缺少共同契约。
第一版 IR 要可验证、可追踪、可测试，不追求完整语言能力。
```

### 已完成代码改造

新增：

```text
src/ir/canonicalRuleIr.ts
  CanonicalRuleIr 类型。
  Function / State / Lifecycle Rule 类型。
  RuleExpression / RuleStatement 类型。
  validateCanonicalRuleIr。
  createRuleCoverageReport。
  simulateRuleTrace。

schemas/canonical-rule-ir.schema.json
  Canonical Rule IR v1 JSON Schema。

scripts/test-canonical-rule-ir.cjs
  Canonical Rule IR v1 测试。

框架设计/引擎总体架构/22-Canonical-Rule-IR-v1.md
  Canonical Rule IR v1 设计文档。
```

修改：

```text
package.json
  新增 npm.cmd run test:ir。

框架设计/引擎总体架构/README.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/18-长期主义实现路径.md
  接入 Canonical Rule IR v1 文档并更新阶段状态。
```

### 当前测试覆盖

新增 IR 测试：

```text
damage calculation function rule
direction move lifecycle rule
frame_update movement rule
invalid component read fails
invalid component write fails
missing source map fails
IR can explain which Feature Spec item it implements
function rule cannot declare lifecycle
state rule requires lifecycle
```

### 当前边界

已经完成：

```text
IR v1 有 TypeScript 类型。
IR v1 有 validator。
IR v1 有机器可读 JSON Schema。
IR v1 能表达伤害计算 Function Rule。
IR v1 能表达方向移动 Lifecycle Rule / frame_update。
IR v1 能输出 requirement coverage。
IR v1 能生成最小 trace report。
```

暂未完成：

```text
IR Interpreter。
Lowered Execution IR。
IR -> Rust AOT。
RuntimeBackend 接入 IR。
编辑器 Graph / Inspector 显示 IR。
完整 native call 白名单。
完整类型推导。
```

这些属于阶段 6 之后。

### 验证

已执行：

```powershell
npm.cmd run test:ir
npm.cmd run test:schema
npm.cmd run test:runtime
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
npm.cmd run build
```

结果：

```text
Canonical Rule IR v1 tests passed
Project Schema v1 tests passed
Runtime backend tests passed
project validation passed
tsc passed
editor build passed
runtime bundle build passed
```

### 下一阶段建议

进入长期主义阶段 6：

```text
IR Interpreter MVP
```

建议第一版只解释执行 Function Rule：

```text
damage calculation function rule
input -> output
trace
source map error
```

等 Function Rule 跑通后，再扩展 State / Lifecycle Rule。

## 2026-06-19 执行更新：长期主义阶段 3/4 完成：Runtime Core Boundary 与 TypeScript Runtime Backend v1

### 长期主义复核

本阶段目标是把现有 TypeScript runtime 从编辑器直接调用的散落函数，收束成正式 RuntimeBackend。

判断依据：

```text
当前还不应直接进入 Rust Runtime / IR Interpreter。
但 Runtime Boundary 已经具备明确设计，可以先固化。
TypeScript runtime 必须成为稳定 prototype backend，用于后续对照 Rust backend。
编辑器和验证层不应继续直接依赖 tickScene / createRuntimeState。
```

### 已完成代码改造

新增：

```text
src/runtime/RuntimeBackend.ts
  定义 RuntimeBackend / RuntimeLoadResult / RuntimeTickResult / RuntimeTraceReport / RuntimeValidationReport。

src/runtime-backends/typescript/TypeScriptRuntimeBackend.ts
  包装现有 TypeScript ECS / FrameLoop / Systems。
  支持 loadProject / loadScene / tick / extractRenderSnapshot / getRuntimeTrace / validateRuntime。

src/services/runtimeService.ts
  编辑器和验证层使用的 runtime service 最小入口。

scripts/test-runtime-backend.cjs
  Runtime backend smoke tests。

框架设计/引擎总体架构/21-Runtime-Core-Boundary.md
  Runtime Core Boundary / TypeScript Runtime Backend v1 文档。
```

修改：

```text
src/engine/projectValidation.ts
  runRuntimeSmoke 改为通过 RuntimeBackend 执行。

src/App.tsx
  Runtime Trace 面板改为通过 RuntimeBackend 获取 systems / trace。
  Viewport 播放、单步和 RenderSnapshot 提取改为通过 RuntimeBackend。
  不再直接调用 createRuntimeState / tickScene / extractRenderSnapshot。

package.json
  新增 npm.cmd run test:runtime。

框架设计/引擎总体架构/README.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/18-长期主义实现路径.md
  接入 Runtime Core Boundary 文档并更新阶段状态。
```

### 当前测试覆盖

新增 runtime backend 测试：

```text
backend can load starter project
backend can load shooter project
backend tick returns scene state
backend extractRenderSnapshot returns renderables
backend runtime trace contains system entries
backend validation reports systems / trace / schemas
backend fails on invalid scene
shooter tick can spawn projectile
```

### 当前边界

已经完成：

```text
RuntimeBackend v1。
TypeScriptRuntimeBackend v1。
RuntimeService 最小入口。
Validation runtime smoke 走 backend。
Editor Viewport / Trace 走 backend。
```

暂未完成：

```text
RustRuntimeBackend。
Runtime backend 选择 UI。
Runtime sidecar / native addon 桥接。
IR Interpreter 接入 runtime backend。
Runtime Trace Replay。
```

这些需要等 IR v1 / Rust runtime / replay 方案稳定后再实现。

### 验证

已执行：

```powershell
npm.cmd run test:schema
npm.cmd run test:runtime
npm.cmd run build
```

结果：

```text
Project Schema v1 tests passed
Runtime backend tests passed
tsc passed
editor build passed
runtime bundle build passed
```

### 下一阶段建议

进入长期主义阶段 5：

```text
Canonical Rule IR v1
```

原因：

```text
Project Schema v1 已经稳定。
Runtime Boundary v1 已经稳定。
下一步如果继续进入底层，应先定义项目规则真相层 IR，而不是继续堆 TypeScript gameplay systems。
```

## 2026-06-19 执行更新：长期主义阶段 1 完成：Project Schema v1 最小闭环

### 长期主义复核

本阶段目标是固定当前项目数据真相，作为后续 Runtime Boundary、IR、Asset DB、Build Graph 的稳定输入。

判断依据：

```text
不能继续只依赖 TypeScript types 和手写 validation。
AI-first engine 需要机器可读、可测试、可版本化的 schema。
第一版必须简单，不能一上来拆 Scene 文件、Asset DB、Bundle 和热更。
```

### 已完成代码改造

新增：

```text
src/schema/projectSchema.ts
  Project / Scene / Entity / Asset / Patch Plan / Validation Report v1 validator。
  输出 kind / path / message。
  schemaVersion 可选兼容旧数据；如果声明则必须匹配 v1。

scripts/project-schema.cjs
  Node 脚本可用的同规则 validator。

schemas/project.schema.json
schemas/scene.schema.json
schemas/entity.schema.json
schemas/asset.schema.json
schemas/patch-plan.schema.json
schemas/validation-report.schema.json

scripts/test-project-schema.cjs
  Project Schema v1 测试用例。

框架设计/引擎总体架构/19-Project-Schema-v1.md
  Project Schema v1 正式设计文档。
```

修改：

```text
src/engine/projectValidation.ts
  validateProjectData 接入 validateGameProjectSchema。

scripts/validation.cjs
  Node validation 接入 scripts/project-schema.cjs。

src/engine/types.ts
  GameProject 增加 schemaVersion?: "project.v1"。

src/data/starterProject.ts
  starterProject / shooterProject 增加 schemaVersion: "project.v1"。

package.json
  新增 npm.cmd run test:schema。

框架设计/引擎总体架构/README.md
框架设计/引擎总体架构/00-阅读顺序.md
框架设计/引擎总体架构/18-长期主义实现路径.md
  接入 Project Schema v1。
```

### 当前测试覆盖

已新增 schema 测试：

```text
valid starter-like project passes
valid shooter-like project passes
missing project.name fails
invalid activeSceneId fails
invalid scene.gravity fails
invalid entity.transform.position.x fails
asset ref mismatch fails
scene asset points to missing scene fails
valid patch plan passes
patch plan unknown operation fails
valid validation report passes
validation report missing issue path fails
```

### 当前边界

已经完成：

```text
Project Schema v1 最小正式契约。
TS validation 和 Node validation 均接入 schema validator。
JSON Schema 文件作为文档和后续工具入口。
schemaVersion 已进入模板项目。
```

暂未完成：

```text
schemaVersion 尚未强制所有对象必须存在。
Scene / Entity 尚未拆成独立文件。
Asset Registry / Asset DB 尚未实现。
Component Schema 尚未与 JSON Schema 完全统一。
Patch Rollback 尚未实现。
Validation Report source / jump 尚未完全标准化为独立 report 对象。
```

### 验证

已执行：

```powershell
npm.cmd run test:schema
npm.cmd run build
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```text
Project Schema v1 tests passed
tsc passed
editor build passed
runtime bundle build passed
project validation passed
runtime smoke validation passed
```

额外坏例子验证：

```text
project.name
project.activeSceneId
project.scenes.scene-bad.gravity
project.scenes.scene-bad.background
project.scenes.scene-bad.entities.player-bad.name
project.scenes.scene-bad.entities.player-bad.transform.position.x
project.assets.scene-asset-bad.ref
project.assets.scene-asset-bad
runtime
```

均能输出结构化 kind / path / message。

### 下一阶段建议

进入长期主义阶段 2：

```text
编辑器模块化与服务层
```

原因：

```text
Project Schema v1 已经给后续模块提供稳定输入。
当前 App.tsx 过大，继续加 Runtime Boundary / IR / Asset DB 会增加维护风险。
下一步应先拆 editor UI 和 service 层，保持行为不变，只降低工程耦合。
```

## 2026-06-19 执行更新：阶段 27 完成：Inspector Field Registry 最小化

### 长期主义复核

阶段 27 的目标是把字段级高亮规则从 JSX 中收束到一个最小 registry。

判断依据：

```text
阶段 25 已经做了字段级高亮。
如果继续在各个 Inspector JSX 中散落 path 判断，后续字段越多越难维护。
本阶段不扩大字段覆盖范围，只把已有稳定字段登记到 registry。
```

### 已完成代码改造

修改：

```text
src/App.tsx
  新增 InspectorFieldTarget。
  新增 inspectorFieldRegistry。
  新增 fieldClass / axisFieldClass / inputClass。
  新增 isRegisteredInspectorFieldFocused。
  新增 fieldSuffix。

当前 registry 覆盖：
  project:
    name
    version
    activeSceneId
    engineMode

  scene:
    name
    gravity
    background
    skyColor

  entity:
    name
    transform.position.x/y/z
    transform.rotation.x/y/z
    transform.scale.x/y/z
```

### 当前边界

已经完成：

```text
字段级高亮不再完全散落在 JSX 判断中。
新增字段高亮需要先进入 registry。
不做模糊匹配，不做自动猜测。
```

暂未完成：

```text
Mesh / Physics / Camera / Light / Script 字段级高亮还未登记。
Runtime Trace 不支持 replay / breakpoint / timeline。
Patch History 不支持反向 Patch / rollback。
```

这些不是当前最小改造闭环的阻塞项。尤其 rollback 需要反向 Patch、资源引用、Scene/Prefab 一致性规则，必须单独设计后再实现。

### 验证

已执行：

```powershell
npm.cmd run build
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
project validation passed
runtime smoke validation passed
```

### 当前批次结论

截至阶段 27，当前改造计划中“错误定位与可维护性闭环”的最小实现已经完成：

```text
Patch History
Patch Operation trace
Validation source map
Project / Scene / Entity / Asset / Runtime issue jump
Runtime Trace read-only panel
字段级高亮最小 registry
Asset 跳转滚动
```

后续如果继续改造，应进入更大主题：

```text
Patch Rollback / 反向 Patch
Runtime Replay / Breakpoint / Timeline
完整 Inspector Field Registry + Schema Renderer
资源导入与 Bundle 构建闭环
```

## 2026-06-19 执行更新：阶段 26 完成：Asset 跳转滚动最小化

### 长期主义复核

阶段 26 的目标是补齐 Asset issue 的可见定位体验。

判断依据：

```text
阶段 21 已经能让 Asset issue 跳到 Project 并高亮资源。
如果资源数量较多，只有高亮不够，目标资源可能仍在滚动区域外。
本阶段只做 selectedAssetId -> asset card scrollIntoView，不改变 Asset 数据模型。
```

### 已完成代码改造

修改：

```text
src/App.tsx
  ProjectDock 为 asset card 建立 refs。
  selectedAssetId 变化时自动切换到资源所在 folder：
    script -> Scripts
    prefab -> Prefabs
    scene -> Scenes
    其他资源 -> Assets

  selectedAssetId / folder / visibleAssets 变化后：
    requestAnimationFrame
    assetCard.scrollIntoView({ block: "nearest", inline: "nearest" })
```

### 当前边界

已经完成：

```text
Asset validation issue 可以跳到 Project。
Asset validation issue 可以高亮目标资源。
Asset validation issue 可以自动滚动到目标资源。
```

暂未完成：

```text
Runtime Trace 不支持 replay / breakpoint / timeline。
Patch History 不支持反向 Patch / rollback。
Mesh / Physics / Camera / Light / Script 字段级高亮需要后续 Inspector Field Registry。
```

### 验证

已执行：

```powershell
npm.cmd run build
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
project validation passed
runtime smoke validation passed
```

### 下一阶段建议

阶段 27 建议进入 `Inspector Field Registry 最小设计/落地` 或 `Patch Rollback 设计`：

```text
如果继续增强错误定位，应先做 Inspector Field Registry，避免 JSX 中散落 path 判断。
如果继续增强审计闭环，应设计反向 Patch / rollback，但这会涉及资源引用和 Scene/Prefab 一致性，不能仓促实现。
```

## 2026-06-19 执行更新：阶段 25 完成：字段级 Validation 高亮最小化

### 长期主义复核

阶段 25 的目标是让 validation issue 从“跳到面板”推进到“高亮字段”。

判断依据：

```text
字段级定位对 AI-first engine 的可维护性非常关键。
但第一版不能伪装成完整 schema renderer。
因此只支持 path 能稳定映射到现有 UI 的字段。
```

### 已完成代码改造

修改：

```text
src/App.tsx
  新增 validationFocusPath。
  jumpToValidationIssue(issue) 会保存 focus path。

  Project Settings 支持字段高亮：
    project.name
    project.version
    project.activeSceneId
    project.engineMode

  Scene Settings 支持字段高亮：
    project.scenes.<sceneId>.name
    project.scenes.<sceneId>.gravity
    project.scenes.<sceneId>.background
    project.scenes.<sceneId>.skyColor

  Entity Inspector 支持字段高亮：
    project.scenes.<sceneId>.entities.<entityId>.name
    project.scenes.<sceneId>.entities.<entityId>.transform.position.x/y/z
    project.scenes.<sceneId>.entities.<entityId>.transform.rotation.x/y/z
    project.scenes.<sceneId>.entities.<entityId>.transform.scale.x/y/z

src/styles.css
  新增 validation-focused-field。
  新增 validation-focused-input。
```

### 当前边界

已经完成：

```text
稳定字段可以从 validation issue 直接定位到 UI 控件。
字段高亮基于明确 path，不做模糊匹配。
```

暂未完成：

```text
Mesh / Physics / Camera / Light / Script 字段级高亮。
Asset 面板滚动到目标资源。
Runtime Trace 不支持 replay / breakpoint / timeline。
Patch History 不支持反向 Patch / rollback。
```

后续如果要继续扩展字段级高亮，应优先引入 Inspector Field Registry，而不是在 JSX 里无限手写 path 判断。

### 验证

已执行：

```powershell
npm.cmd run build
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
project validation passed
runtime smoke validation passed
```

### 下一阶段建议

阶段 26 建议进入 `Asset 跳转滚动最小化`：

```text
当前 Asset issue 已经能跳到 Project 并高亮资源。
如果资源很多，目标资源可能不在可视区域。
下一步应在 ProjectDock 中为 asset card 建立 ref，并在 selectedAssetId 变化时 scrollIntoView。
```

## 2026-06-19 执行更新：阶段 24 完成：History Operation 高亮最小化

### 长期主义复核

阶段 24 的目标是让 validation issue 与 Patch Operation 的弱关联真正可用。

判断依据：

```text
阶段 20 已经保存 relatedOperationIndex。
如果 UI 只显示 related operation #N，用户仍需要人工查找。
本阶段只做当前展开 history entry 内的 operation 高亮，不做反向 Patch，不做跨记录搜索。
```

### 已完成代码改造

修改：

```text
src/App.tsx
  PatchHistoryPanel 新增 highlightedOperation。
  点击 validation source 的 Jump 时：
    如果 issue 有 relatedOperationIndex，先展开对应 history entry 并高亮 operation。
    再执行原有 Jump 行为。

  对于没有 editor jump target、但有关联 operation 的 issue：
    显示 Operation 按钮。
    点击只高亮对应 operation。

src/styles.css
  patch-operation-list span.highlighted 增加高亮样式。
  patch-jump-button.secondary 增加次级按钮样式。
```

### 当前边界

已经完成：

```text
Validation issue 可以把用户带到相关 Patch Operation。
没有明确 editor jump target 的 issue 仍然可以定位到 operation。
```

暂未完成：

```text
Asset 面板滚动到目标资源。
Inspector 字段级高亮。
Runtime Trace 不支持 replay / breakpoint / timeline。
Patch History 不支持反向 Patch / rollback。
```

### 验证

已执行：

```powershell
npm.cmd run build
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
project validation passed
runtime smoke validation passed
```

### 下一阶段建议

阶段 25 建议进入 `字段级 Validation 高亮最小化`：

```text
当前 issue 能跳到 Entity / Asset / Scene / Project / Runtime。
但跳到 Inspector 后，还不能高亮具体字段。
下一步只支持 project.name、project.version、project.activeSceneId、scene.name、scene.gravity、scene.background、scene.skyColor、entity.name 和 transform 字段。
不支持所有组件字段，避免一次性把 Inspector 变成复杂 schema renderer。
```

## 2026-06-19 执行更新：阶段 23 完成：Runtime Trace Issue 跳转最小化

### 长期主义复核

阶段 23 的目标是让 runtime validation issue 有明确的诊断落点。

判断依据：

```text
runtime issue 不应该伪跳到 Entity / Scene。
Runtime 的正确落点应该是系统列表、最近一次 smoke tick trace、runtime issue 列表。
第一版只做只读诊断面板，不做 replay、断点、逐帧调试器，避免调试系统过早膨胀。
```

### 已完成代码改造

修改：

```text
src/App.tsx
  新增 ConsoleTab = game / console / ai / json / trace。
  Console tab 状态提升到 App 层。
  jumpToValidationIssue(issue) 增加 kind=runtime：
    -> setConsoleTab(trace)
    -> show Console

  ConsoleDock 增加 Trace tab。
  新增 createRuntimeTraceView(project)。
  新增 RuntimeTracePanel。

  RuntimeTracePanel 展示：
    active scene name
    runtime system count
    trace entry count
    component schema count
    runtime validation issues
    system manifest
    last smoke tick trace

src/styles.css
  新增 runtime-trace-panel。
  新增 runtime-trace-summary。
  新增 runtime-trace-section。
```

### 当前边界

已经完成：

```text
Runtime validation issue 点击 Jump 会进入 Console / Trace。
Trace 面板使用独立 smoke tick 生成诊断快照，不 drain 正在运行的 Game 预览 session。
Runtime issue 不再只写 Console log。
```

暂未完成：

```text
Asset 面板滚动到目标资源。
点击 related operation 高亮 operation。
Inspector 字段级高亮。
Runtime Trace 不支持 replay / breakpoint / timeline。
```

这些继续作为后续阶段处理。Runtime replay 和 breakpoint 属于完整调试器，不放进当前最小闭环。

### 验证

已执行：

```powershell
npm.cmd run build
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
project validation passed
runtime smoke validation passed
```

### 下一阶段建议

阶段 24 建议进入 `History Operation 高亮最小化`：

```text
当前 validation issue 已经能显示 relatedOperationIndex。
下一步点击 related operation 或 Jump 时，应在 Patch History 展开区域高亮对应 operation。
第一版只做当前展开 entry 内的 operation 高亮，不做跨历史记录搜索，也不做反向 Patch。
```

## 2026-06-19 执行更新：阶段 22 完成：Project Settings / Scene Settings 最小化

### 长期主义复核

阶段 22 的目标是补上 project / scene validation issue 的明确编辑器落点。

判断依据：

```text
阶段 21 已经能把 entity / asset issue 跳到 Inspector / Project。
project / scene issue 如果只写 Console log，会让 AI-first 错误定位闭环断在项目级配置。
本阶段只补最小 Settings surface，不引入完整 Settings 系统，避免过早复杂化。
```

### 已完成代码改造

修改：

```text
src/App.tsx
  新增 InspectorMode:
    entity
    scene-settings
    project-settings

  Inspector 增加 Entity / Scene / Project 三个最小 tab。

  Project Settings 支持编辑：
    project.name
    project.version
    project.activeSceneId
    project.engineMode 只读显示

  Scene Settings 支持编辑：
    scene.name
    scene.gravity
    scene.background
    scene.skyColor

  jumpToValidationIssue(issue) 增加：
    kind=project -> 打开 Inspector / Project Settings
    kind=scene -> 打开 Inspector / Scene Settings
    kind=entity -> 仍跳 Entity Inspector / Hierarchy
    kind=asset -> 仍跳 Project 并高亮 Asset

  canJumpToValidationIssue(issue) 增加 project / scene。

src/styles.css
  新增 inspector-mode-tabs。
  新增 settings-component。
```

### 当前跳转规则

```text
kind=project
  -> setInspectorMode(project-settings)
  -> show Inspector

kind=scene + path 包含 scenes.<sceneId>
  -> 如 sceneId 存在，切换 activeSceneId
  -> setInspectorMode(scene-settings)
  -> show Inspector

kind=entity + path 包含 entities.<entityId>
  -> setSelectedEntityId(entityId)
  -> setInspectorMode(entity)
  -> show Inspector / Hierarchy

kind=asset + path 包含 assets.<assetId>
  -> setSelectedAssetId(assetId)
  -> show Project
```

### 当前边界

已经完成：

```text
Project validation issue 有明确 UI 落点。
Scene validation issue 有明确 UI 落点。
Settings 面板只承载已有数据模型字段，不新增新的项目配置模型。
```

暂未完成：

```text
Runtime issue 跳转到 Runtime Trace。
Asset 面板滚动到目标资源。
点击 related operation 高亮 operation。
Inspector 字段级高亮。
```

这些继续作为后续阶段处理。字段级高亮必须建立在 path -> UI field 的稳定映射上，不能靠字符串猜测。

### 验证

已执行：

```powershell
npm.cmd run build
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
project validation passed
runtime smoke validation passed
```

### 下一阶段建议

阶段 23 建议进入 `Runtime Trace Issue 跳转最小化`：

```text
当前 runtime validation issue 仍不可定位。
下一步应增加 Runtime Trace 面板或在 Console/Game 中增加 Trace tab。
Runtime issue 点击 Jump 时，应跳到最近一次 runtime validation / tick trace，而不是伪造 entity 或 scene 定位。
第一版只展示系统 trace、schemas、runtime errors/issues，不做逐帧 replay。
```

## 2026-06-19 执行更新：阶段 21 完成：Validation Issue 跳转 Inspector / Asset 最小化

### 长期主义复核

阶段 21 的目标是让结构化 validation issue 不只可读，还能驱动编辑器定位。

判断依据：

```text
AI-first engine 的错误可维护性，最终要落到用户能快速定位问题对象。
阶段 19 已经有 kind / path。
阶段 20 已经有 related operation。
阶段 21 先把最明确的 entity / asset 跳转做起来。
scene / runtime 暂不跳转，避免伪定位。
```

### 已完成代码改造

修改：

```text
src/App.tsx
  新增 selectedAssetId。
  新增 jumpToValidationIssue(issue)。
  entity issue 会解析 path 中 entities 后的 entity id。
  asset issue 会解析 path 中 assets 后的 asset id。
  entity 跳转会选中 Entity，并打开 Inspector / Hierarchy。
  asset 跳转会选中 Asset，并打开 Project 面板。
  无法跳转的 issue 会写入 Console log。
  PatchHistoryPanel 的 Validation Sources 增加 Jump 按钮。
  ProjectDock 支持 selectedAssetId / onSelectAsset。

src/styles.css
  Project asset card 增加 selected 高亮。
  Validation Sources 增加 Jump 按钮样式。
```

当前跳转规则：

```text
kind=entity + path 包含 entities.<entityId>
  -> setSelectedEntityId(entityId)
  -> show Inspector / Hierarchy

kind=asset + path 包含 assets.<assetId>
  -> setSelectedAssetId(assetId)
  -> show Project

kind=scene / runtime / project
  -> 暂不跳转，只写 log
```

### 当前边界

已经完成：

```text
Entity validation issue 可以跳到 Inspector。
Asset validation issue 可以跳到 Project 并高亮资源。
不可跳转 issue 不会伪装成可跳转。
```

暂未完成：

```text
Scene issue 跳转到 Scene 设置。
Runtime issue 跳转到 Runtime Trace。
Project issue 跳转到 Project Settings。
Asset 面板滚动到目标资源。
点击 related operation 高亮 operation。
Inspector 字段级高亮。
```

这些属于后续阶段，尤其字段级高亮需要更细的 path -> Inspector field 映射。

### 验证

已执行：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行：

```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": [],
    "issues": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "issues": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

### 下一阶段建议

阶段 22 建议进入 `Project Settings / Scene Settings 最小化`：

```text
当前 Inspector 主要面向 Entity。
为了让 project / scene validation issue 也能定位，需要最小 Project Settings / Scene Settings 面板。
第一版只显示和编辑 name、version、activeScene、scene gravity/background/skyColor。
```

## 2026-06-19 执行更新：阶段 20 完成：Validation Issue 与 Patch Operation 弱关联最小化

### 长期主义复核

阶段 20 的目标是让 validation issue 在安全条件下关联到 Patch Operation。

判断依据：

```text
阶段 19 已经让 project/runtime validation issue 有 kind / path。
阶段 17 已经让 history 有 operation target。
只有当 issue path 中的 scene/entity/asset id 与 operation target 明确匹配时，才建立 relatedOperationIndex。
不根据自然语言 message 猜测，不做模糊相似度匹配。
宁可少关联，也不能错误归因。
```

### 已完成代码改造

修改：

```text
src/engine/types.ts
  ProjectPatchHistoryValidationIssue 增加 relatedOperationIndex。

src/engine/patchHistory.ts
  createPatchHistoryEntry 会在写入 validationIssues 时尝试关联 operation。
  patch validation issue 已有 operationIndex 时直接使用。
  project validation issue 只从 path 中提取 scene/entity/asset id。
  operation target 按 / 和 . 拆分为 token。
  id 明确匹配时写入 relatedOperationIndex。
  找不到明确匹配时不关联。

src/App.tsx
  PatchHistoryPanel 的 Validation Sources 显示 related operation #N。
```

当前弱关联规则：

```text
patch validation issue:
  operationIndex -> relatedOperationIndex

project validation issue:
  kind=scene  -> path 中 scenes 后面的 scene id
  kind=entity -> path 中 entities 后面的 entity id
  kind=asset  -> path 中 assets 后面的 asset id

operation target:
  用 / 和 . 切成 token

只有 issue token 与 target token 完全相等时关联。
```

### 当前边界

已经完成：

```text
Validation Issue 可以保守关联到 Patch Operation。
History 展开时可以看到 related operation。
未明确匹配时不会显示 related operation。
```

暂未完成：

```text
基于路径层级的更强关联。
字段级 diff。
operation 级回滚。
点击 related operation 高亮对应 operation。
AI 修复策略按 related operation 排序。
runtime issue 到 operation 的关联。
```

这些属于后续阶段。runtime issue 暂时不做关联，因为 runtime 错误往往来自执行结果，不一定能安全映射到单个 Patch Operation。

### 验证

已执行：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行：

```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": [],
    "issues": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "issues": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

### 下一阶段建议

阶段 21 建议进入 `Validation Issue 跳转 Inspector / Asset 最小化`：

```text
让 project issue path 可以驱动编辑器选择目标。
entity path 跳转到对应 Entity Inspector。
asset path 跳转到 Project 面板对应资源。
第一版只做 entity / asset，scene 和 runtime 暂不跳转。
```

## 2026-06-19 执行更新：阶段 19 完成：Project Validation Source Map 最小化

### 长期主义复核

阶段 19 的目标是让 project validation / runtime smoke 错误先具备结构化路径，而不是强行反推到 Patch Operation。

判断依据：

```text
Project validation 发生在 Patch 应用后的候选项目上。
这类错误不一定能安全地直接归因到某个 operation。
第一版应先输出 kind / path / message，让错误本身可定位。
后续再用 Patch operation target 和 validation path 做弱关联。
这样可以避免引擎增加一套猜测式归因规则。
```

### 已完成代码改造

修改：

```text
src/engine/projectValidation.ts
  ValidationSection 增加 issues。
  新增 ProjectValidationIssue。
  validateProjectData 输出 project / scene / entity / asset issue。
  runRuntimeSmoke 输出 runtime issue。
  errors 字符串数组继续保留，兼容已有调用。

scripts/validation.cjs
  命令行 / Build Graph 使用的验证器同步输出 issues。
  validate-project.cjs 输出现在包含 project.issues / runtime.issues。

src/ai/aiRepair.ts
  AiErrorItem 增加 kind / path。
  createAiErrorReport 从 project.project.issues / project.runtime.issues 生成结构化错误。

src/engine/types.ts
  ProjectPatchHistoryValidationIssue 增加 kind / path。

src/engine/patchHistory.ts
  normalizePatchHistory 支持 kind / path。

src/App.tsx
  project validation failed / runtime smoke failed 写入 validationIssues。
  AI 当前 validation summary 显示前两个 project/runtime source。
  PatchHistoryPanel 的 Validation Sources 显示 kind / path / message。
```

Project / Runtime issue 字段：

```text
kind: project | scene | entity | asset | runtime
path
message
```

### 当前边界

已经完成：

```text
Project validation 错误可定位到项目路径。
Runtime smoke 错误可定位到 runtime 路径。
AI Error Report 可以携带 kind / path。
Patch History 可以保存并展示 kind / path。
命令行验证输出也包含 issues。
```

暂未完成：

```text
project/runtime validation issue 到 Patch operation 的弱关联。
字段级 Component diff。
基于 path 的自动修复策略。
错误路径点击跳转到 Inspector / Asset。
Build Report UI 展示 issues。
```

这些属于后续阶段。尤其 operation 弱关联应基于明确 target/path 规则，而不是字符串猜测。

### 验证

已执行：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行：

```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": [],
    "issues": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "issues": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

### 下一阶段建议

阶段 20 建议进入 `Validation Issue 与 Patch Operation 弱关联最小化`：

```text
利用 Patch operation target 和 validation issue path 做保守弱关联。
只在 target/path 明确匹配时关联，不做猜测。
History 展开时显示 Related Operation。
AI 修复时可优先查看相关 operation。
```

## 2026-06-19 执行更新：阶段 18 完成：Patch Validation Error Source Map 最小化

### 长期主义复核

阶段 18 的目标是让 Patch validation 错误尽量关联到具体 Patch Operation。

判断依据：

```text
AI-first engine 需要让错误可解释、可定位、可修复。
如果 validation failed 只返回字符串，AI 和用户仍然不知道是哪一步 Patch 出错。
阶段 17 已经让历史记录包含 operation trace，阶段 18 继续把 validation error 绑定到 operation index / type / target。
第一版只处理 patch validation，不处理 project validation / runtime smoke 的 operation 反向定位。
```

### 已完成代码改造

修改：

```text
src/engine/projectPatch.ts
  ProjectPatchValidation 增加 issues。
  新增 ProjectPatchValidationIssue。
  validateProjectPatchPlan 内部改为收集结构化 issue。
  plan 级错误不带 operationIndex。
  operation 级错误带 operationIndex、operationType、target。
  errors 字符串数组继续保留，兼容旧调用。

src/ai/aiRepair.ts
  AiErrorItem 增加 operationIndex、operationType、target。
  createAiErrorReport 从 patch.issues 生成 patch-validation 错误。

src/engine/types.ts
  ProjectPatchHistoryEntry 增加 validationIssues。
  新增 ProjectPatchHistoryValidationIssue。

src/engine/patchHistory.ts
  createPatchHistoryEntry 支持写入 validationIssues。
  normalizePatchHistory 支持读取 validationIssues，并兼容旧历史。

src/App.tsx
  validation_failed history 写入 patch validation issues。
  AI 当前 validation summary 显示前两个 operation source。
  PatchHistoryPanel 展开记录时显示 Validation Sources。

src/styles.css
  Validation Sources 复用 operation list 样式。
```

结构化错误字段：

```text
message
operationIndex
operationType
target
```

### 当前边界

已经完成：

```text
Patch validation 错误可以定位到 operation index / type / target。
AI Error Report 可以携带 operation source。
Patch History 可以保存 validationIssues。
History 展开时可以看到 Validation Sources。
旧 errors 字符串数组仍然存在，兼容已有逻辑。
```

暂未完成：

```text
Project validation 错误反向定位到 operation。
Runtime smoke 错误反向定位到 operation。
字段级 Component source map。
自然语言需求到 operation 的映射。
validation issue 到自动修复策略的精确绑定。
```

这些属于后续阶段。尤其 project/runtime validation 的错误反向定位需要候选项目、Patch 前后状态和验证器路径一起设计。

### 验证

已执行：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行：

```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

### 下一阶段建议

阶段 19 建议进入 `Project Validation Source Map 最小化`：

```text
让 project validation 错误尽量关联到 Patch 后的资源路径或 Entity / Asset。
第一版不强行反推 operation，只先让 project validation 输出 path / kind / message。
后续再用 Patch operation target 与 validation path 做弱关联。
```

## 2026-06-19 执行更新：阶段 17 完成：Patch Source Map / Operation 级追踪最小化

### 长期主义复核

阶段 17 的目标是让 Patch History 不只记录“结果”，还记录“由哪些操作造成这个结果”。

判断依据：

```text
复杂项目里只知道某个 Patch approved / rejected 不够。
AI 和用户需要追到具体 operation：createEntity、updateEntity、addScript、removeScript、replaceProject 等。
第一版只做 operation 级，不做字段级 diff。
字段级 diff 会牵涉 Component Schema、Entity 深层比较、AssetRef 变化和 source map，应该单独设计。
```

### 已完成代码改造

修改：

```text
src/engine/types.ts
  ProjectPatchHistoryEntry 增加 operations。
  新增 ProjectPatchHistoryOperation。

src/engine/patchHistory.ts
  createPatchHistoryEntry 会把 ProjectPatchDiff.items 写入 history.operations。
  normalizePatchHistory 支持读取旧历史记录。
  旧记录没有 operations 时自动补空数组。
  新历史记录保存 operation、target、summary、risk。

src/App.tsx
  PatchHistoryPanel 展开单条历史时显示 Operations 列表。
  没有 operation trace 的旧历史会显示 Operation trace unavailable。

src/styles.css
  增加 patch-operation-list 样式。
```

Operation 级追踪字段：

```text
operation
target
summary
risk
```

### 当前边界

已经完成：

```text
每条新 Patch History 都保存 operation 级追踪。
用户展开历史记录时可以看到每一步 operation。
旧历史记录兼容，不会因为缺少 operations 被丢弃。
```

暂未完成：

```text
字段级 diff。
Component 字段 source map。
自然语言需求到 operation 的映射。
operation 到具体验证错误的反向定位。
按 operation 级回滚。
```

这些属于后续阶段，尤其字段级 diff 必须建立在更细的 Patch Operation 和 Component Schema 渲染之上。

### 验证

已执行：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行：

```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

### 下一阶段建议

阶段 18 建议进入 `Patch Validation Error Source Map 最小化`：

```text
让 patch validation / project validation 错误尽量关联到 patch operation。
第一版可以只在 validateProjectPatchPlan 中记录 operation index 和 operation type。
AI 面板显示错误来自哪一步 operation。
暂不做字段级定位，先做到 operation 级错误定位。
```

## 2026-06-19 执行更新：阶段 16 完成：Patch History 面板 / 审计查看最小化

### 长期主义复核

阶段 16 的目标是把阶段 15 的项目级 Patch History 从“能记录”推进到“能查看、能过滤、能展开”。

判断依据：

```text
AI-first engine 的可维护性不只来自 Patch 验证，也来自用户和 AI 能回看修改历史。
只显示最近 4 条摘要不足以支撑复杂项目调试。
第一版先做查看、过滤、展开，不做回滚。
回滚会涉及反向 Patch、资源引用、Scene / Prefab / Asset 一致性，必须单独设计，不能偷塞进审计查看阶段。
```

### 已完成代码改造

修改：

```text
src/App.tsx
  新增 PatchHistoryFilter。
  PatchHistorySummary 替换为 PatchHistoryPanel。
  PatchHistoryPanel 支持查看完整 patchHistory。
  PatchHistoryPanel 支持按 status 过滤。
  PatchHistoryPanel 支持展开单条记录。
  展开内容包含 patchId、time、operationCount、reason、summary、validationSummary。
  ConsoleDock / BottomPanel 都使用 PatchHistoryPanel。

src/styles.css
  增加 Patch History Panel、过滤按钮、记录行、状态标签、展开详情样式。
```

当前支持的过滤：

```text
all
applied
approved
rejected
validation_failed
repair_pending
```

### 当前边界

已经完成：

```text
用户可以查看完整 Patch History。
用户可以按状态过滤历史。
用户可以展开单条历史查看原因和验证摘要。
Patch History Panel 只读 Project，不修改 Project。
```

暂未完成：

```text
按历史记录回滚。
历史记录搜索。
历史记录导出。
按用户身份过滤。
团队审批审计。
Diff 字段级展开。
Patch source map 展开。
```

这些属于后续阶段，尤其回滚必须先设计反向 Patch 和资源一致性策略。

### 验证

已执行：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行：

```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

### 下一阶段建议

阶段 17 建议进入 `Patch Source Map / Operation 级追踪最小化`：

```text
让 Patch History 能追到每个 operation。
每个 operation 至少记录 type、target、risk、summary。
AI / 用户排查问题时可以知道是哪一步 createEntity / updateEntity / addScript 导致变化。
暂不做字段级 diff，先做到 operation 级。
```

## 2026-06-19 执行更新：阶段 15 完成：Patch History 最小化

### 长期主义复核

阶段 15 的目标是让 AI 修改从“当前 UI 状态”进入“项目级可回溯记录”。

判断依据：

```text
AI-first engine 不能只知道当前最后一次 AI 结果。
用户批准、用户拒绝、验证失败、修复待审批都应该成为项目可追踪事件。
Patch History 是后续 AI 调试、用户回溯、团队审计、版本协作的基础。
第一版只做项目级最小历史，不提前引入复杂权限、多人协作、持久化审计服务器。
```

### 已完成代码改造

修改：

```text
src/engine/types.ts
  GameProject 增加 patchHistory。
  新增 ProjectPatchHistoryStatus。
  新增 ProjectPatchHistoryEntry。

src/engine/patchHistory.ts
  新增 createPatchHistoryEntry。
  新增 appendPatchHistory。
  新增 normalizePatchHistory。
  Patch History 最多保留最近 100 条。

src/App.tsx
  loadProject / cloneProject / normalizeProject 接入 patchHistory 标准化。
  AI 自动应用 Patch 时写入 applied history。
  用户批准普通 Patch 时写入 approved history。
  用户拒绝普通 Patch 时写入 rejected history。
  用户批准 Repair Patch 时写入 approved history。
  用户拒绝 Repair Patch 时写入 rejected history。
  Patch validation failed / project validation failed 写入 validation_failed history。
  Repair pending 写入 repair_pending history。
  AI 面板显示最近 4 条 Patch History。

src/data/starterProject.ts
  starterProject / shooterProject 增加 patchHistory: []。

src/styles.css
  增加 Patch History 摘要样式。
```

历史记录字段：

```text
id
patchId
title
status: applied | approved | rejected | validation_failed | repair_pending
summary
operationCount
risks: low / medium / high
reason
validationSummary
createdAt
```

### 当前边界

已经完成：

```text
Patch History 存在于 GameProject 根数据。
导入旧项目时会自动补空历史。
保存 / 导出项目 JSON 时会携带 patchHistory。
AI 面板能读取并展示最近历史。
```

暂未完成：

```text
完整 Patch History 面板。
按历史记录回滚。
历史记录 diff 展开。
历史记录搜索 / 过滤。
团队审计和用户身份。
replaceProject 保留旧项目历史的合并策略。
```

第一版选择不处理完整审计系统，是为了避免把审批主干和团队协作提前混在一起。

### 验证

第一次并行构建时遇到 Vite 偶发输出路径错误：

```text
[vite:build-html] emitted chunks and assets must be strings that are neither absolute nor relative paths
```

单独重跑后通过。该问题此前也出现过，判断为当前 Vite 构建偶发问题，不是本阶段 TypeScript 或 Patch History 改造错误。

最终已执行：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行：

```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

### 下一阶段建议

阶段 16 建议进入 `Patch History 面板 / 审计查看最小化`：

```text
在 AI 面板或独立 History 面板中展示完整 Patch History。
支持按 status 过滤。
支持展开单条 history 查看 validationSummary 和风险摘要。
暂不做回滚，先把“看得清楚”做好。
```

## 2026-06-19 执行更新：阶段 14 完成：Patch 审批拒绝路径最小化

### 长期主义复核

阶段 14 继续沿着 AI-first engine 的主干做，而不是增加临时 UI 行为。

本阶段的核心判断：

```text
如果 AI Patch 可以等待批准，就必须也可以被明确拒绝。
拒绝不能只是把 UI pending 清掉，否则后续 AI、用户和调试系统都无法知道“为什么没有应用”。
拒绝也不能修改 Project，审批层只负责授权结果，不负责偷偷修项目。
第一版只做内存中的拒绝事件，不提前引入团队审批、权限矩阵或持久化审计系统。
```

### 已完成代码改造

修改：

```text
src/ai/mockAi.ts
  AiResult 增加 rejections。
  新增 AiPatchRejection。

src/App.tsx
  新增 rejectAiPatch(reason)。
  新增 rejectAiRepair(reason)。
  拒绝 pendingPatch 时清除 pendingPatch，记录 rejection，不修改 Project。
  拒绝 pendingRepair 时清除 pendingRepair，记录 rejection，不修改 Project。
  Console log 会记录用户拒绝事件。
  AiPatchDiffSummary 增加普通 Patch / Repair Patch 的拒绝按钮。
  AiPatchDiffSummary 显示最近一次 rejection 摘要。
  ConsoleDock / BottomPanel 同步接入拒绝回调，避免旧面板签名断层。
```

拒绝事件结构：

```text
kind: patch | repair
reason: string
rejectedAt: ISO time
diff: ProjectPatchDiff
```

### 当前边界

已经完成：

```text
用户可以拒绝普通 pendingPatch。
用户可以拒绝 pendingRepair。
拒绝不会执行 applyProjectPatchPlan。
拒绝不会污染 Project。
拒绝会留在当前 AiResult.rejections 中，供 UI / AI 后续查看。
```

暂未完成：

```text
拒绝原因由用户输入。
拒绝记录持久化。
审批历史面板。
团队审批。
按 Patch 类型建立“同类命令全部放过 / 全部拒绝”规则。
字段级 Diff 和 source map。
```

这些属于后续阶段。当前阶段只建立审批闭环的最低必要能力：批准和拒绝都存在，且都可追踪。

### 验证

已执行：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行：

```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

### 下一阶段建议

阶段 15 建议进入 `审批记录持久化 / Patch History 最小化`：

```text
把已批准、已拒绝、验证失败的 Patch 结果写入项目级 Patch History。
Patch History 第一版只记录内存或 Project metadata，不做团队协作。
每条记录应包含 patch id、title、diff、status、validation summary、time。
这会为后续 AI 调试、用户回溯、团队审计打基础。
```

## 2026-06-19 执行更新：阶段 13 完成：用户授权模式最小化

### 长期主义复核

本轮继续改造前重新审视了当前计划：该计划仍然符合长期主义。

判断依据：

```text
当前改造没有围绕“打飞机模板能跑”做临时补丁。
它优先消除旧项目中的结构性风险：双 Runtime、AI 直改 Project、缺少 Patch 审查、缺少验证闭环、导出流程不可解释。
每一阶段都在补 AI-first engine 的长期主干能力：Project Patch、Validation、Repair、Diff、Approval。
阶段 13 没有引入复杂权限矩阵，而是先把“AI 修改是否自动提交”做成清晰的全局授权入口。
这为后续“放过一次 / 同类型全部放过 / 团队审批 / 审批记录持久化”留下扩展点，但不提前制造隐藏规则。
```

### 已完成代码改造

新增最小授权模式：

```text
AiAuthorizationMode = auto | ask
auto：普通 AI Patch 在验证通过后自动应用。
ask：普通 AI Patch 在验证通过后进入 pendingPatch，等待用户批准。
Repair Patch：继续默认必须用户批准，不受 auto 模式绕过。
```

修改：

```text
src/ai/mockAi.ts
  AiResult 增加 pendingPatch。

src/App.tsx
  增加 aiAuthorizationMode 状态。
  runAi 在 ask 模式下不直接 commit，而是生成 pendingPatch。
  新增 approveAiPatch(patchPlan, reason)。
  approveAiPatch 会重新 validate patch、apply 到候选项目、runProjectValidation，通过后才 commit。
  ConsoleDock / BottomPanel 增加授权模式控件和普通 Patch 批准入口。
  AiPatchDiffSummary 同时显示 pendingPatch 和 pendingRepair。

src/styles.css
  增加 AI 授权分段控件样式。
  调整 AI 面板布局，避免授权行挤压输入和结果区域。
```

### 当前边界

已经完成：

```text
普通 AI Patch 可配置自动应用或询问确认。
询问确认模式下，Patch 已经完成验证，但不会污染 Project。
用户批准时会重新验证，避免基于过期 Project 状态盲目应用。
Repair Patch 仍然必须显式批准。
```

暂未完成：

```text
放过一次。
同类型命令全部放过。
拒绝 Patch 按钮。
审批记录持久化。
团队协作审批。
字段级 Diff。
Patch Operation source map。
```

这些能力属于后续阶段；当前阶段只建立最小、可验证、可扩展的授权主干。

### 验证

已执行：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行：

```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：

```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

### 下一阶段建议

阶段 14 建议进入 `Patch 审批结果与拒绝路径最小化`：

```text
增加拒绝 pendingPatch / pendingRepair。
拒绝时保留 diff、reason、validation summary 到 AI 结果，而不修改 Project。
Console log 记录拒绝事件。
为后续审批记录持久化和团队协作做入口。
```

## 2026-06-19 状态更新

### 阶段 2 完成：RuntimeWorld 与编辑器 Scene 分离

完成内容：

```text
编辑器 Play 模式不再把 runtime scene 写回 project.scenes。
已移除 updateRuntimeScene / onRuntimeScene 写回路径。
SceneCanvas 在进入 Play 时从编辑器 Scene clone 出 RuntimeScene。
停止 Play 时恢复显示编辑器 Scene。
Step Preview 也从编辑器 Scene clone 出 RuntimeScene 后执行单帧，不污染 Project。
```

长期主义判断：

```text
编辑器 Scene 是项目资产数据。
RuntimeScene / RuntimeWorld 是 Play 时运行数据。
运行时变化默认不能污染编辑器资产。
后续如果需要“保存运行结果”，必须做成显式命令，而不是 Play 模式隐式写回。
```

验证：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
```

### 阶段 1 完成：清理 Runtime 双轨

本轮已完成阶段 1：清理 Runtime 双轨。

完成内容：

```text
game-runtime/player.js 已删除本地 gameplay runtime。
player.js 不再包含本地 tickScene / applyScripts / collision / projectile hit 逻辑。
导出播放器只负责加载项目、输入、HUD 和 Three.js 渲染。
所有 gameplay runtime 均来自 game-runtime/ai-first-runtime.js。
game-runtime/ai-first-runtime.js 由 src/engine/runtimeBundle.ts 构建而来。
编辑器预览和导出播放器共用 src/engine 下的 ECS Runtime。
```

长期主义判断：

```text
这一步不是为了让打飞机模板短期能跑，而是为了消灭编辑器和导出播放器的双轨逻辑。
从现在开始，Runtime 行为必须只有一个来源：src/engine。
任何 gameplay 行为如果只写在 player.js、App.tsx 或导出模板里，都视为架构倒退。
```

验证：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
```

当前原则：

```text
不为了短期模板产出牺牲底层架构。
优先验证完整运行架构是否成立。
先做最小闭环，但闭环必须站在长期架构上。
```

## 当前代码现状

当前项目主要由以下部分组成：

```text
src/App.tsx
  React 编辑器 UI
  Three.js Scene / Game 预览
  Inspector / Project / AI 面板

src/engine
  当前已开始改造为 ECS Runtime

src/data/starterProject.ts
  内置示例项目和打飞机模板

src/ai/mockAi.ts
  mock AI 指令，直接生成或修改项目数据

game-runtime
  导出后的 Electron 游戏播放器

electron
  编辑器 Electron 外壳和导出 IPC

scripts/export-game-win.cjs
  命令行导出 Windows 游戏
```

当前已经具备：

```text
基础编辑器界面
Scene / Entity / Component 雏形
Three.js 渲染
简单 Play 模式
打飞机模板
Windows EXE 导出
```

当前主要问题：

```text
项目数据模型仍然偏原型
AI 仍是 mock，且直接修改 Project
Runtime 刚开始 ECS 化，还不是完整 ECS
编辑器渲染仍直接读 Scene，不是 RenderSnapshot
game-runtime/player.js 仍残留旧 runtime 代码
Build / Export 还不是正式 Build Graph
没有 Component Schema / Rule Graph / IR
没有 Asset Graph / Bundle / Hot Update 实现
```

## 已完成改造

### 1. 主编辑器 Runtime 切到最小 ECS

新增：

```text
src/engine/ecs.ts
src/engine/frameLoop.ts
src/engine/systems.ts
```

作用：

```text
ECS World
Component Store
Runtime Command
FrameLoop
System Registry
Scene <-> World 转换
```

### 2. runtime.ts 改为架构入口

`src/engine/runtime.ts` 已从旧的脚本 if-chain 变成兼容入口：

```text
createRuntimeState(scene)
tickScene(scene, states, input, dt)
```

内部执行：

```text
Scene -> EcsWorld
runFrame(world, systems, context)
EcsWorld -> Scene
```

这样 UI 仍然能调用旧函数名，但底层已经走 ECS。

### 3. 导出播放器接入共享 Runtime

新增：

```text
src/engine/runtimeBundle.ts
vite.runtime.config.ts
game-runtime/ai-first-runtime.js
```

`game-runtime/index.html` 现在加载：

```html
<script src="./ai-first-runtime.js"></script>
<script src="./player.js"></script>
```

`game-runtime/player.js` 主循环现在调用：

```text
window.AiFirstRuntime.createRuntimeState
window.AiFirstRuntime.tickScene
```

编辑器预览和导出播放器开始共享同一套 Runtime 逻辑。

### 4. 构建验证

已执行：

```powershell
npm.cmd run build
```

结果：

```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

## 目标架构

最小目标架构：

```text
AI / Editor
  -> Project Model
  -> Scene / Entity / Component / Asset
  -> Runtime Build
  -> ECS World
  -> FrameLoop
  -> Systems
  -> RenderExtract
  -> Renderer
  -> Build / Export
```

用户 / AI 可见 Runtime 阶段：

```text
FixedUpdate
Update
LateUpdate
Render
```

引擎内部阶段：

```text
FrameBegin
HotUpdateApply
DeferredCommandApply
RenderExtract
RendererFeatureBuilder
RDG
RHI
FrameEnd
```

短期仍使用 Three.js Renderer，但架构上必须预留：

```text
RenderExtract
RendererFeatureBuilder
RDG
RHI
Backend
```

## 关键边界

### ECS 写入

当前正式规则：

```text
ECS 默认允许项目规则直接写 Component。
引擎只处理读写安全、并行安全、结构变化安全。
业务顺序和业务正确性由项目层负责。
CommandBuffer 主要用于结构变化和跨线程延迟提交。
```

### 逻辑与渲染

正式目标：

```text
逻辑层可以读写 ECS。
渲染层不能直接读写 Gameplay ECS。
渲染层只能读取 RenderSnapshot。
RenderExtract 是逻辑到渲染的唯一桥。
```

当前状态：

```text
编辑器和 game-runtime 仍直接渲染 Scene 数据。
RenderSnapshot 尚未实现。
```

### AI 修改

目标：

```text
AI 不直接乱改 Project。
AI 生成 Patch Plan。
引擎验证 Patch Plan。
验证通过后应用到 Project Model。
```

当前状态：

```text
mockAi.ts 仍直接返回完整 Project。
```

## 后续改造顺序

### 阶段 1：清理 Runtime 双轨

目标：

```text
彻底移除 game-runtime/player.js 内残留的旧 tickScene / applyScripts / collision 逻辑。
player.js 只保留渲染、输入、加载、HUD。
所有 gameplay runtime 只来自 ai-first-runtime.js。
```

验证：

```text
npm.cmd run build
导出播放器仍可运行打飞机模板
player.js 中不再存在本地 tickScene / applyScripts
```

### 阶段 2：RuntimeWorld 与编辑器 Scene 分离

目标：

```text
Play 模式创建 RuntimeWorld。
停止 Play 后不污染编辑器 Scene。
只有用户明确保存运行结果时才写回 Scene。
```

当前问题：

```text
编辑器播放时仍周期性把 runtime scene 同步回 project。
```

验证：

```text
按 Play 后实体移动。
按 Stop 后编辑器 Scene 恢复播放前状态。
```

### 阶段 3：RenderExtract / RenderSnapshot

目标：

```text
Renderer 不直接读 ECS / Scene。
Runtime 每帧生成 RenderSnapshot。
ThreeRenderer 只消费 RenderSnapshot。
```

最小 RenderSnapshot：

```text
camera
lights
renderables
background
skyColor
hudData
```

验证：

```text
Scene/Game 视图仍正常显示。
渲染代码不直接访问 gameplay scripts。
```

### 阶段 4：SystemRegistry 正式化

目标：

```text
当前 systems.ts 中硬编码系统注册改成显式 registry。
系统声明 id / phase / update。
后续支持 AI 生成项目系统。
```

第一版不需要完整 reads/writes 暴露给用户。

验证：

```text
能列出当前项目所有 runtime systems。
能禁用某个 system。
能输出每帧 system trace。
```

### 阶段 5：Project Model / Patch Plan

目标：

```text
mockAi.ts 不直接返回 Project。
AI 输出 AiPatchPlan。
PatchPlan 经 validatePatch 后 applyPatch。
```

最小 PatchPlan：

```text
createEntity
deleteEntity
updateComponent
addScript
removeScript
setActiveScene
```

验证：

```text
AI 生成打飞机模板时输出 patch list。
用户能看到 patch 摘要。
应用后 Project 正确变化。
```

### 阶段 6：Component Schema

目标：

```text
Transform / Mesh / Physics / Camera / Light / Script 不只是 TypeScript 类型。
它们必须有运行时 Schema。
Inspector / AI / Validation 共用 Schema。
```

验证：

```text
Inspector 字段来自 Schema。
AI Patch 修改字段时走 Schema 校验。
非法字段被阻止。
```

### 阶段 7：Scene / Prefab / Asset 正式化

目标：

```text
Scene 是资源。
Prefab 是资源。
Entity 可以来自 Prefab Instance。
AssetRef 替代直接 source 字符串。
```

验证：

```text
拖入 Prefab 创建实例。
修改 Prefab 可影响实例。
Scene 保存 Entity / Component / Prefab Instance / Override。
```

### 阶段 8：Build Graph

目标：

```text
export-game-win.cjs 从复制脚本升级为 Build Graph 执行器。
输出 Build Report。
```

最小 Build Graph：

```text
validate project
build runtime bundle
cook project data
copy runtime template
write manifest
write build-report.json
```

验证：

```text
导出成功时生成 build-report.json。
导出失败时报告具体阶段。
```

## 暂停点

当前已完成到：

```text
主编辑器 runtime ECS 化
导出播放器主循环接入共享 ECS runtime
构建通过
```

接下来暂停继续实现，优先审查本改造计划。

建议下一步讨论：

```text
阶段 1：是否先清理 game-runtime/player.js 旧 runtime 残留
阶段 2：是否先做 RuntimeWorld 与编辑器 Scene 分离
阶段 3：RenderSnapshot 的最小字段应该有哪些
```
# 当前项目改造计划

## 2026-06-19 执行更新：阶段 12 完成，Patch Diff / 用户批准机制最小化

### 长期主义复核

本阶段没有直接做完整权限系统，而是先让 Patch Plan 具备可审查 Diff 摘要，并让 Repair Patch 不再静默应用。

当前规则：
```text
普通 AI Patch 仍沿用当前自动应用流程。
所有 Patch 都生成操作级 Diff 摘要。
Post-apply validation 失败后生成的 Repair Patch 进入 pendingRepair。
Repair Patch 必须用户点击批准后才会应用。
批准时会先应用原 Patch，再应用 Repair Patch，再执行 runProjectValidation。
验证通过才 commit。
```

这符合长期主义：
```text
AI 修改必须逐步可审查。
修复 Patch 风险更高，不应该静默应用。
Diff 摘要应属于 Patch 层能力，而不是 UI 临时字符串。
后续“放过一次 / 同类型全部放过”可以建立在这个审批入口上。
```

### 已完成代码改造

修改：
```text
src/engine/projectPatch.ts
```

新增：
```text
ProjectPatchDiffItem
ProjectPatchDiff
summarizeProjectPatchPlan(plan)
```

Diff 当前字段：
```text
operation
target
summary
risk
```

当前支持操作摘要：
```text
replaceProject
createEntity
deleteEntity
updateEntity
addScript
removeScript
setActiveScene
```

风险级别：
```text
low
medium
high
```

修改：
```text
src/ai/mockAi.ts
```

新增到 AiResult：
```text
diff
pendingRepair
```

pendingRepair 当前包含：
```text
basePatchPlan
patchPlan
diff
reason
```

修改：
```text
src/App.tsx
```

新增：
```text
AiPatchDiffSummary
approveAiRepair(basePatchPlan, repairPatchPlan, reason)
```

行为变化：
```text
AI 面板显示 Patch Diff 摘要。
AI 面板显示 Repair pending。
用户点击“批准修复”后才应用 Repair Patch。
批准修复时会验证 base patch、应用 base patch、验证 repair patch、应用 repair patch、再 runProjectValidation。
验证失败不会提交。
```

顺手修复：
```text
BottomPanel 旧 Project 资源筛选逻辑也改为按 asset.type 识别 Prefabs / Scenes。
```

### 当前边界

已经完成：
```text
Patch 操作级 Diff 摘要。
Repair Patch 待审批。
Repair Patch 用户批准后再应用。
Repair Patch 应用后再验证。
AI 面板显示 diff / repair pending / repair reason。
```

暂未完成：
```text
普通 Patch 的用户批准模式。
放过一次 / 同类型全部放过。
Diff 的字段级对比。
Patch Operation source map。
用户拒绝 Repair Patch 的显式按钮。
审批记录持久化。
团队协作审批。
```

这些属于后续阶段；当前阶段先建立可审查 Patch 的最小 UI 和数据结构。

### 验证

已执行项目构建：
```powershell
npm.cmd run build
```

结果：
```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行命令行项目验证烟测：
```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：
```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

已检索确认：
```text
summarizeProjectPatchPlan 已接入 AiResult.diff。
pendingRepair 已接入 runAi。
approveAiRepair 已接入 ConsoleDock / AI 面板。
```

### 下一阶段建议

阶段 13 应进入 `用户授权模式最小化`：
```text
提供两种模式：自动应用 / 询问确认。
Repair Patch 默认询问。
普通 Patch 可配置自动应用或询问。
第一版只需要全局设置，不急着做复杂权限策略。
后续扩展“放过一次 / 同类型全部放过”。
```

## 2026-06-19 执行更新：阶段 11 完成，AI 错误反馈 / 自动修复循环最小化

### 长期主义复核

本阶段没有把 AI 修复做成“万能自动改项目”，而是先建立结构化错误包和保守修复入口。

当前闭环：
```text
AI 生成 Patch Plan。
引擎执行 Patch Validation。
Patch 通过后应用到候选 Project。
引擎执行 Project Validation + Runtime Smoke。
失败时生成 AiErrorReport。
AI / mock repair 根据 AiErrorReport 生成修复 Patch。
修复 Patch 再次 validate。
修复后 Project 再次 runProjectValidation。
全部通过才 commit。
```

这符合长期主义：
```text
AI 不能在没有把握时继续乱修。
错误必须结构化，便于 AI 和用户理解。
自动修复必须是保守的、可验证的。
无法安全修复时必须暴露错误，而不是掩盖错误。
```

### 已完成代码改造

新增：
```text
src/ai/aiRepair.ts
```

提供：
```text
AiErrorSource
AiErrorItem
AiErrorReport
AiRepairResult
createAiErrorReport(patch, project?)
createMockRepairPatch(project, report)
```

当前错误来源：
```text
patch-validation
project-validation
runtime-smoke
```

当前错误字段：
```text
source
message
severity
repairable
```

当前 mock repair 支持：
```text
activeSceneId 指向不存在 Scene 时，生成 setActiveScene 到第一个可用 Scene 的修复 Patch。
```

明确不支持：
```text
复杂资源引用修复。
复杂 Component 修复。
逻辑系统修复。
自动猜测用户意图。
循环多轮自动修复。
```

已改造：
```text
src/ai/mockAi.ts
  AiResult.validation 增加 errorReport / repair。

src/App.tsx
  runAi 在 patch validation 失败时生成 AiErrorReport。
  runAi 在 post-apply validation 失败时生成 AiErrorReport。
  runAi 会尝试一次 createMockRepairPatch。
  修复 Patch 必须再次 validate。
  修复后的 Project 必须再次 runProjectValidation。
  成功才 commit。
  失败则保留当前 Project，并写 Console log。
  AI 面板展示错误摘要和 repair 结果。
```

### 当前边界

已经完成：
```text
结构化 AI 错误包。
保守 mock repair 入口。
generate -> validate -> repair -> validate 最小闭环。
验证失败不污染当前 Project。
AI 面板能看到 error summary / repair reason。
```

暂未完成：
```text
真实大模型修复。
多轮修复循环。
错误 source map 到具体 Patch Operation。
错误树 UI。
用户批准 repair Patch。
修复前后 diff 展示。
复杂错误分类和策略库。
```

这些属于后续阶段；当前阶段先建立 AI 可消费的错误标准结构和一次性安全修复通道。

### 验证

已执行项目构建：
```powershell
npm.cmd run build
```

结果：
```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行命令行项目验证烟测：
```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：
```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

已检索确认：
```text
AiErrorReport / createAiErrorReport / createMockRepairPatch 已接入 runAi。
AI 面板会显示 errorReport summary 和 repair reason。
```

### 下一阶段建议

阶段 12 应进入 `Patch Diff / 用户批准机制最小化`：
```text
AI Patch 和 Repair Patch 不应该总是静默应用。
需要最小 Diff 摘要，让用户知道会创建 / 删除 / 修改什么。
第一版可以继续默认自动应用普通 Patch，但 repair Patch 应该进入可审查状态。
长期目标是用户可选择“放过一次 / 同类型全部放过”。
```

## 2026-06-19 执行更新：阶段 10 完成，Patch Plan 应用后自动验证 / AI 修改闭环落地

### 长期主义复核

本阶段把 AI Patch 从“验证通过后直接提交”升级为“两段验证闭环”。

当前 AI 修改流程：
```text
AI 生成 Patch Plan。
引擎执行 validateProjectPatchPlan。
Patch 验证通过后，先应用到候选 Project。
引擎对候选 Project 执行 runProjectValidation。
Project Validation + Runtime Smoke 全部通过后才 commit。
失败时不修改当前 Project，并把结构化错误写入 Console / AI 面板。
```

这符合长期主义：
```text
AI 不能只负责生成。
引擎必须负责验证和阻止坏修改。
用户不需要理解底层代码，也能知道 AI 修改是否通过验证。
Runtime Smoke 让 AI 修改后的项目至少能创建运行状态、tick 一帧、生成 RenderSnapshot。
```

### 已完成代码改造

新增：
```text
src/engine/projectValidation.ts
```

提供：
```text
runProjectValidation(project)
validateProjectData(project)
runRuntimeSmoke(project)
```

浏览器侧 Runtime Smoke 当前检查：
```text
createRuntimeState
tickScene
extractRenderSnapshot
listRuntimeSystems
getRuntimeSystemTrace
listComponentSchemas
```

已改造：
```text
src/ai/mockAi.ts
  AiResult 增加 validation 字段。

src/App.tsx
  runAi 不再只做 patch validation。
  runAi 会先应用到 candidateProject。
  candidateProject 通过 runProjectValidation 后才 commit。
  验证失败会保留当前 Project，不提交坏结果。
  AI 面板增加 AiValidationSummary。
  Console 会输出 patch rejected 或 post-apply validation failed 的结构化错误。
```

### 当前边界

已经完成：
```text
AI Patch 生成后验证。
Patch 应用后项目验证。
Runtime Smoke 集成到编辑器内 AI 修改流程。
验证通过才提交 Project。
验证失败阻止提交。
AI 面板展示验证结果摘要。
```

暂未完成：
```text
AI 自动根据错误进行二次修复。
AI 面板展示完整错误树。
每个 Patch Operation 的 source map / trace。
Patch 应用后的测试用例自动生成。
和命令行 validation.cjs 的验证逻辑完全去重。
```

当前浏览器侧 `projectValidation.ts` 与 Node 侧 `scripts/validation.cjs` 有一部分重复逻辑。

这是当前阶段可接受的临时状态：
```text
浏览器侧需要直接调用 TS runtime。
Node 侧需要加载构建后的 runtime bundle。
两者验证目标一致，但运行环境不同。
后续可以抽象共享验证规范，减少重复实现。
```

### 验证

已执行项目构建：
```powershell
npm.cmd run build
```

结果：
```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行命令行项目验证烟测：
```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：
```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

已检索确认：
```text
App.tsx 不再存在 commitProject(() => result.project) 旧路径。
AI 提交流程走 candidateProject -> runProjectValidation -> commit。
```

### 下一阶段建议

阶段 11 应进入 `AI 错误反馈 / 自动修复循环最小化`：
```text
AI Patch 验证失败时生成结构化错误包。
AI 可以读取错误包并生成修复 Patch Plan。
第一版不需要真正接大模型，可以先让 mock AI / UI 能显示并回放错误。
目标是形成 generate -> validate -> repair -> validate 的最小闭环。
```

## 2026-06-19 执行更新：阶段 9 完成，测试系统最小化落地

### 长期主义复核

本阶段没有做帧同步专用 Replay / Determinism Test，而是按之前确认的方向做通用测试系统最小闭环。

当前测试系统目标：
```text
验证 Project 数据是否合法。
验证 Runtime 能创建状态、tick 一帧、生成 RenderSnapshot。
验证 Runtime 能暴露 SystemRegistry trace。
验证 Runtime 能暴露 Component Schema。
验证 Build Graph 能在构建前执行 Project + Runtime smoke。
验证 Build Report 结构可被 AI 和用户读取。
```

这符合长期主义：
```text
测试系统先服务 AI 修改和项目可维护性。
第一版不绑定某一种游戏类型。
第一版不绑定帧同步。
测试结果必须结构化，方便 AI 定位错误。
```

### 已完成代码改造

新增：
```text
scripts/validation.cjs
scripts/validate-project.cjs
```

`scripts/validation.cjs` 提供：
```text
normalizeProjectAssets(project)
validateProjectData(project)
runRuntimeSmoke(root, project)
validateBuildReport(report)
runProjectValidation(root, project)
```

`runRuntimeSmoke` 当前检查：
```text
game-runtime/ai-first-runtime.js 能被加载。
window.AiFirstRuntime 能注册。
createRuntimeState 能创建运行状态。
tickScene 能执行一帧。
extractRenderSnapshot 能生成渲染快照。
listRuntimeSystems 能列出系统。
getRuntimeSystemTrace 能拿到 trace。
listComponentSchemas 能列出组件 Schema。
```

命令行入口：
```text
node scripts/validate-project.cjs <project.json>
```

package.json 新增：
```text
npm.cmd run validate:project -- <project.json>
```

已接入：
```text
scripts/build-graph.cjs
  validate-project 阶段改为调用 runProjectValidation。
  Build Graph 导出前会执行 Project Validation + Runtime Smoke。
  build-report.json 的 validate-project stage result 会记录 runtimeSystems / runtimeTraceEntries / componentSchemas。
```

### 当前边界

已经完成：
```text
Project Validation
Runtime Smoke
Build Report Validation 函数
Build Graph 构建前验证接入
命令行项目验证入口
```

暂未完成：
```text
Patch Plan 每次应用后自动触发验证。
编辑器 UI 展示验证结果。
Build Graph 构建后自动启动 exe 做端到端验证。
Playwright UI 测试和新验证系统整合。
Determinism / Replay 专用测试。
性能测试。
移动平台 smoke test。
```

这些属于后续阶段；当前阶段先建立 AI 可用的最小结构化验证闭环。

### 验证

已执行项目构建：
```powershell
npm.cmd run build
```

结果：
```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行项目验证烟测：
```powershell
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：
```json
{
  "ok": true,
  "project": {
    "ok": true,
    "errors": []
  },
  "runtime": {
    "ok": true,
    "errors": [],
    "systems": 12,
    "trace": 12,
    "schemas": 6
  }
}
```

已执行 Windows 导出烟测：
```powershell
node scripts\export-game-win.cjs $env:TEMP\ai-first-validation-smoke.json
```

结果：
```text
G:\gameEngin\exports\Validation-Smoke-win\Validation-Smoke.exe
```

已检查：
```text
G:\gameEngin\exports\Validation-Smoke-win\build-report.json
G:\gameEngin\exports\Validation-Smoke-win\resources\app\build-manifest.json
```

验证结论：
```text
build-report.json status = passed
failedStage = null
validate-project stage result 包含 runtimeSystems = 12 / runtimeTraceEntries = 12 / componentSchemas = 6
```

### 下一阶段建议

阶段 10 应进入 `Patch Plan 应用后自动验证 / AI 修改闭环`：
```text
AI Patch 生成后 validatePatch。
Patch 应用后 runProjectValidation。
验证失败时不隐藏错误，返回结构化错误给 AI 面板和 Console。
AI 可以根据错误进行下一轮修复。
```

## 2026-06-19 执行更新：阶段 8 完成，Build Graph 最小执行器落地

### 长期主义复核

本阶段把 Windows 导出从“复制目录 + 写项目 JSON”的脚本，升级为共享 Build Graph 执行器。

这符合长期主义：
```text
导出流程必须是可解释、可验证、可报告的。
AI 和用户需要知道构建失败在哪个阶段，而不是只看到一个错误字符串。
命令行导出和编辑器按钮必须走同一套构建管线。
后续 PC / Android / iOS / Web 多平台导出应复用 Build Graph 思路。
```

### 已完成代码改造

新增：
```text
scripts/build-graph.cjs
```

提供：
```text
exportWindowsGameBuild({ root, project })
normalizeProjectAssets(project)
sanitizeName(value)
```

Build Graph 当前阶段：
```text
validate-project
validate-inputs
copy-electron-runtime
prepare-app-directory
copy-runtime-dependencies
cook-project-data
write-manifest
rename-executable
```

每个阶段记录：
```text
id
name
status
startedAt
endedAt
durationMs
error
result
```

输出文件：
```text
resources/app/game-project.json
resources/app/package.json
resources/app/build-manifest.json
build-report.json
```

已改造：
```text
scripts/export-game-win.cjs
  改为调用 scripts/build-graph.cjs。
  保留原命令行用法：node scripts/export-game-win.cjs <project.json>。
  stdout 仍输出 exe 路径。
  增加 UTF-8 BOM JSON 兼容。

electron/main.cjs
  删除重复导出逻辑。
  编辑器导出按钮也调用同一个 exportWindowsGameBuild。
  IPC 返回 outputDir / executable / manifest / report。
```

### 当前边界

已经完成：
```text
CLI 导出和 Electron 编辑器导出共享同一套 Build Graph。
成功导出会生成 build-manifest.json。
成功或失败都会尽量写 build-report.json。
失败时 report.failedStage 指向失败阶段。
导出前会 normalizeProjectAssets。
```

暂未完成：
```text
Build Graph 还没有独立节点文件格式。
还没有资源 cook / Bundle / AssetSet 真实分包。
还没有 Android / iOS / Web 后端。
还没有构建产物自动运行验证。
还没有 UI 面板展示 build-report。
```

这些属于后续阶段，当前阶段目标是先让构建流程从“不可解释脚本”变成“有阶段和报告的最小管线”。

### 验证

已执行项目构建：
```powershell
npm.cmd run build
```

结果：
```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

已执行 Windows 导出烟测：
```powershell
node scripts\export-game-win.cjs $env:TEMP\ai-first-build-smoke.json
```

结果：
```text
G:\gameEngin\exports\Build-Graph-Smoke-win\Build-Graph-Smoke.exe
```

已检查：
```text
G:\gameEngin\exports\Build-Graph-Smoke-win\build-report.json
G:\gameEngin\exports\Build-Graph-Smoke-win\resources\app\build-manifest.json
```

验证结论：
```text
build-report.json status = passed
failedStage = null
8 个 Build Graph stage 全部 passed
build-manifest.json 包含 buildId / target / appName / projectName / projectVersion / activeSceneId / sceneCount / assetCount
```

### 下一阶段建议

阶段 9 应进入 `测试系统最小化`：
```text
每个 AI Patch 至少应能触发基础验证。
Build Graph 应支持构建前验证和构建后 smoke test。
Runtime 应支持最小场景加载 / tick / render snapshot 的自动检查。
测试系统第一版不要绑定帧同步，应做成通用 Validation / Runtime Smoke / Build Smoke。
```

## 2026-06-19 执行更新：阶段 7 最小完成，Scene / Prefab / Asset 正式化起步

### 长期主义复核

本阶段没有直接实现完整资源导入器、Bundle 分包或 Prefab 编辑器，而是先把项目数据模型从“字符串 source + 临时 assetId”推进到正式的 AssetRef / Prefab / Scene Asset 边界。

判断理由：
```text
AssetRef 是后续资源依赖、热更、Build Graph、Bundle 生成的基础。
Scene 必须逐步成为一等资源，而不是只存在于 project.scenes 数组里。
Prefab 必须逐步成为一等资源，Entity 需要能引用 prefab instance。
当前先建立数据结构、规范化和校验，不急着做完整编辑体验。
```

### 已完成代码改造

修改：
```text
src/engine/types.ts
```

新增类型：
```text
AssetType
AssetRef
PrefabInstance
SceneAssetData
PrefabAssetData
```

字段变化：
```text
MeshComponent.assetRef?: AssetRef<"model">
Entity.prefabInstance?: PrefabInstance
Asset.type: AssetType
Asset.ref?: AssetRef
Asset.data?: SceneAssetData | PrefabAssetData
```

新增：
```text
src/engine/assetModel.ts
```

提供：
```text
assetRef(id, type)
createSceneAsset(id, name, sceneId, source)
createPrefabAsset(id, name, root, source)
normalizeAsset(asset)
normalizeProjectAssets(project)
resolveAsset(project, ref)
validateProjectAssets(project)
```

已接入：
```text
src/data/starterProject.ts
  starterProject / shooterProject 的 scene 资源改为 createSceneAsset。

src/engine/projectPatch.ts
  Patch Plan 验证和应用会 normalizeProjectAssets。
  replaceProject / createEntity / updateEntity 过程中会间接校验资源引用。
  validateProjectShape 会调用 validateProjectAssets。

src/App.tsx
  loadProject / cloneProject / normalizeProject 会 normalizeProjectAssets。
  GLTF / GLB 导入时同时写入 mesh.assetRef 和旧 mesh.assetId。
  渲染模型时优先读取 mesh.assetRef.id，旧 assetId 作为兼容兜底。
  Project 面板中 Scenes / Prefabs 文件夹按 asset.type 筛选，不再靠名字包含 scene / prefab。

scripts/export-game-win.cjs
  写入 game-project.json 前补齐 asset.ref，保证导出项目也进入新 AssetRef 形态。

src/engine/componentSchema.ts
  Mesh Schema 增加 assetRef 字段校验。
```

### 当前边界

已经完成：
```text
Scene 可以作为 scene asset 表达。
Prefab 可以作为 prefab asset 表达。
Entity 可以携带 prefabInstance 引用。
Mesh model 资源可以通过 AssetRef 引用。
旧 assetId / source 仍然兼容。
项目加载、克隆、导出会规范化资产 ref。
Patch 验证会检查 scene asset、prefab asset、mesh assetRef、prefabInstance 引用。
```

暂未完成：
```text
Prefab 实例化流程。
Prefab Override 编辑。
Scene / Prefab 独立保存文件。
AssetRef 替代所有旧 assetId。
AssetSet / Bundle / Build Graph 自动分包。
资源导入器和资源数据库。
```

这些不属于本阶段阻塞项，原因是：
```text
本阶段目标是让数据模型具备长期边界。
Prefab 编辑和 Bundle 属于后续更大阶段，必须建立在 AssetRef 和资源校验之上。
```

### 验证

已执行：
```powershell
npm.cmd run build
```

验证记录：
```text
第一次与并行检索同时执行时，Vite build-html 偶发报出 index.html 输出路径异常。
单独重跑 npm.cmd run build 后通过。
```

最终结果：
```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

### 下一阶段建议

阶段 8 应进入 `Build Graph 最小执行器`：
```text
export-game-win.cjs 从复制脚本升级为 Build Graph 执行器。
第一版 Build Graph 包含 validate project、build runtime bundle、cook project data、copy runtime template、write manifest、write build-report.json。
导出失败时必须报告具体失败阶段。
```

## 2026-06-19 执行更新：阶段 6 最小完成，Component Schema 落地

### 长期主义复核

本阶段没有直接把 Inspector 重写成复杂的 schema renderer，而是先建立运行时可读的组件 Schema，并接入 AI Patch 验证和组件默认值。

这个选择符合长期主义：
```text
Schema 先成为统一规则源。
Patch Validation 先用 Schema 阻止非法组件。
Inspector 先复用 Schema 默认值。
后续再逐步让 Inspector 字段渲染完全来自 Schema。
```

### 已完成代码改造

新增：
```text
src/engine/componentSchema.ts
```

提供：
```text
ComponentKind
ComponentFieldSchema
ComponentSchema
componentSchemas
getComponentSchema(kind)
listComponentSchemas()
createDefaultComponent(kind)
validateEntityComponents(entity)
```

第一版覆盖组件：
```text
Transform
Mesh
Physics
Camera
Light
Scripts
```

Schema 当前包含：
```text
字段路径
字段 label
字段类型
必填规则
默认值
select options
number min / max
基础组件校验
```

已接入：
```text
src/engine/projectPatch.ts
  validateEntityShape 改为调用 validateEntityComponents。
  createEntity / updateEntity / replaceProject 都会触发组件 Schema 校验。

src/App.tsx
  Add Mesh / Physics / Camera / Light 的默认组件来自 createDefaultComponent。

src/engine/runtimeBundle.ts
  window.AiFirstRuntime 增加 listComponentSchemas。
```

### 当前边界

已经完成：
```text
AI Patch Validation 可以用 Schema 校验组件合法性。
Inspector 新增组件默认值开始来自 Schema。
Runtime bundle 可以暴露组件 Schema 元数据。
```

暂未完成：
```text
Inspector 字段还没有完全由 Schema 渲染。
Script 参数还没有按具体脚本 Schema 校验。
Schema 还没有接入导入项目 / 资源导入流程。
Schema 还没有覆盖 AssetRef / Prefab / Scene Resource。
```

这些不属于本阶段阻塞项，原因是：
```text
当前阶段目标是建立统一规则源。
完全 schema-driven Inspector 会大幅改动 UI，应该在 Schema 稳定后单独推进。
脚本参数 Schema 需要后续和 Logic Rule / System Registry / DSL 继续对齐。
```

### 验证

已执行：
```powershell
npm.cmd run build
```

结果：
```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

### 下一阶段建议

阶段 7 应进入 `Scene / Prefab / Asset 正式化`：
```text
Scene 是资源。
Prefab 是资源。
Entity 可以来自 Prefab Instance。
AssetRef 替代直接 source 字符串。
先做最小数据模型和验证，不急着做完整资源导入器。
```

## 2026-06-19 执行更新：阶段 5 最小完成，Project Patch Plan 管线落地

### 长期主义复核

本阶段把 AI 修改从“直接返回完整 Project”改成“生成 Patch Plan，经过验证后应用”。

这符合之前确认的 AI-first 引擎原则：
```text
AI 不直接改 Project。
AI 生成可审查的 Patch Plan。
引擎验证 Patch Plan。
验证通过后由引擎应用到 Project Model。
验证失败时保留原项目，并把错误暴露给用户 / AI。
```

### 已完成代码改造

新增：
```text
src/engine/projectPatch.ts
```

提供：
```text
ProjectPatchPlan
ProjectPatchOperation
validateProjectPatchPlan(project, plan)
applyProjectPatchPlan(project, plan)
```

第一版支持操作：
```text
replaceProject
createEntity
deleteEntity
updateEntity
addScript
removeScript
setActiveScene
```

已改造：
```text
src/ai/mockAi.ts
  runAiCommand 不再只返回完整 Project。
  AI mock 现在生成 patchPlan。
  patchPlan 先 validate，再 apply。

src/App.tsx
  runAi 不再信任 result.project 作为提交来源。
  UI 收到 AI 结果后重新 validateProjectPatchPlan。
  验证通过后 applyProjectPatchPlan。
  验证失败只写 Console log，不改 Project。
  AI 面板显示 patch operations 数量。
```

### 当前边界

当前保留 `AiResult.project`：
```text
这是临时兼容字段，方便现有 UI 和调试查看结果。
权威提交路径已经变成 patchPlan -> validate -> apply。
后续可以删除 AiResult.project。
```

当前保留 `replaceProject`：
```text
它用于模板级命令，例如“生成打飞机模板”。
它不是长期最终形态。
长期应该把模板替换拆成 Scene / Entity / Asset / Script / Setting 等更细粒度 patch。
但现在先让模板级生成也进入验证和应用管线，避免继续保留 AI 直接改 Project 的旁路。
```

验证器当前能力：
```text
检查 patch plan 基础字段。
检查目标 scene / entity / script 是否存在。
检查 createEntity 重复 id。
检查 updateEntity 不允许改 entity id。
检查 replaceProject 的项目基础结构。
按操作顺序模拟项目状态，后续操作基于前序操作结果验证。
```

### 验证

已执行：
```powershell
npm.cmd run build
```

结果：
```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

### 下一阶段建议

阶段 6 应进入 `Component Schema 最小化`：
```text
Transform / Mesh / Physics / Camera / Light / Script 不能只靠 TypeScript 类型。
需要运行时 Schema，供 Inspector、AI Patch Validation、导入资源校验共用。
第一版不要做复杂 Schema 系统，先做字段定义、默认值、基础校验、Inspector 元数据。
```

## 2026-06-19 执行更新：阶段 4 完成，SystemRegistry 最小正式化

### 长期主义复核

本阶段没有引入复杂的 reads/writes 调度规则，也没有把系统依赖交给引擎自动推断。

当前策略：
```text
系统执行顺序仍然由显式注册顺序决定。
引擎提供系统注册、启停和 trace。
业务顺序和业务正确性仍由项目规则负责。
SystemRegistry 是调试和治理边界，不是新的隐藏规则调度器。
```

这样做符合之前确认的原则：
```text
引擎层只处理运行安全、结构边界、可观察性。
项目层负责业务流程。
AI 可以查看系统列表和每帧 trace，但不需要理解复杂底层调度黑箱。
```

### 已完成代码改造

新增：
```text
src/engine/systemRegistry.ts
```

提供：
```text
SystemRegistry
SystemManifest
SystemTraceEntry
createSystemRegistry(systems)
```

已改造：
```text
src/engine/systems.ts
  增加 createDefaultSystemRegistry()。

src/engine/frameLoop.ts
  runFrame 从接收 EcsSystem[] 改为接收 SystemRegistry。
  每帧 beginFrame。
  每个被执行的 system 记录 phase / systemId / durationMs。

src/engine/runtime.ts
  RuntimeSession 持有自己的 SystemRegistry。
  增加 listRuntimeSystems(states)。
  增加 setRuntimeSystemEnabled(states, systemId, enabled)。
  增加 getRuntimeSystemTrace(states)。

src/engine/runtimeBundle.ts
  window.AiFirstRuntime 暴露系统列表、启停和 trace API。
```

### 当前边界

当前 SystemRegistry 只做三件事：
```text
列出系统。
启用 / 禁用系统。
记录每帧执行 trace。
```

明确不做：
```text
不自动推断业务依赖。
不自动重排系统。
不暴露复杂 reads/writes 规则给用户。
不把项目规则冲突伪装成引擎可以自动解决的问题。
```

### 验证

已执行：
```powershell
npm.cmd run build
```

结果：
```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

### 下一阶段建议

阶段 5 应进入 `Project Model / Patch Plan`：
```text
mockAi.ts 不再直接返回完整 Project。
AI 输出 AiPatchPlan。
PatchPlan 经过 validatePatch 后再 applyPatch 到 Project。
第一版只支持 createEntity / deleteEntity / updateComponent / addScript / removeScript / setActiveScene。
```

## 2026-06-19 执行更新：阶段 3 完成，RenderExtract / RenderSnapshot 落地

### 长期主义复核

本轮继续改造前，先重新审视了当前改造计划：这个计划仍然符合长期主义。

判断理由：
```text
它不是围绕“打飞机模板能跑”来补丁式改造。
它优先消除编辑器预览、导出 runtime、渲染入口之间的架构双轨。
它把旧项目逐步切到稳定边界：ECS Runtime、RuntimeWorld、FrameLoop、RenderExtract、Renderer。
每一步都是未来 Rust Runtime / Render Graph / RHI / 多平台导出可以继续承接的边界。
```

当前阶段的关键原则：
```text
Gameplay Scene / ECS World 是逻辑世界。
RenderSnapshot 是渲染世界输入。
Renderer 不直接消费 Gameplay Scene。
RenderExtract 是逻辑世界到渲染世界的唯一桥。
```

### 已完成代码改造

新增：
```text
src/engine/renderSnapshot.ts
```

提供：
```text
RenderSnapshot
RenderCamera
RenderLight
Renderable
RenderHud
extractRenderSnapshot(scene)
```

已改造：
```text
src/App.tsx
  编辑器 Scene/Game 视图不再把 Scene 直接传给 Three 渲染函数。
  每帧先 extractRenderSnapshot(sceneRef.current)，再把 RenderSnapshot 交给 renderThreeScene。

src/engine/runtimeBundle.ts
  window.AiFirstRuntime 增加 extractRenderSnapshot。

game-runtime/player.js
  导出后的播放器不再直接遍历 sceneData.entities 渲染。
  每帧调用 Runtime.extractRenderSnapshot(sceneData)。
  HUD 数据来自 snapshot.hud，不再由 player.js 自己扫描 GameScore 脚本。

src/engine/frameLoop.ts
  移除临时 RenderSnapshot 类型，避免错误地把 RenderSnapshot 定义成 Scene entities 的别名。
```

### 当前过渡边界

当前 `extractRenderSnapshot(scene)` 仍然从 Scene 提取，这是为了兼容当前 TypeScript 最小 runtime。

长期目标不是 Scene -> Snapshot，而是：
```text
ECS World / Render World
  -> RenderExtract
  -> RenderSnapshot
  -> Renderer Feature Builder
  -> RDG
  -> RHI
  -> Backend
```

当前快照里暂时保留 `scriptTags`：
```text
scriptTags 只用于当前模板的临时外观选择，例如飞机和子弹 mesh。
它不能成为渲染层读取玩法逻辑的正式规则。
后续应替换为 renderKind / rendererFeature / materialPreset 等明确渲染数据。
```

### 验证

已执行：
```powershell
npm.cmd run build
```

结果：
```text
tsc passed
editor build passed
runtime bundle build passed
game-runtime/ai-first-runtime.js generated
```

### 下一阶段建议

阶段 4 应进入 `SystemRegistry 正式化`：
```text
把当前 systems.ts 的硬编码系统集合改成显式 registry。
系统有 id / phase / enabled / update。
先不暴露复杂 reads/writes 规则给用户。
引擎只记录系统执行 trace，便于 AI 和用户排查一帧内发生了什么。
```
