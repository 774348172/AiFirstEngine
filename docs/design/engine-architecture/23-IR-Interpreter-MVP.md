# IR Interpreter MVP

本文档记录当前阶段落地的 IR Interpreter MVP。

## 定位

IR Interpreter 是开发期、验证期、热更覆盖用的解释执行后端。

IR Interpreter 不再作为 TypeScript Runtime 的长期能力扩张。正式 Rust Runtime 中，IR Interpreter 必须通过 ProjectLogicRunner / LogicExecutor 接入，详见：

```text
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
```

当前 MVP 执行：

```text
Function Rule
State / Lifecycle Rule 的 frame_update 最小执行
```

暂不执行：

```text
runtime state migration
native calls
```

这个边界是有意的。解释器可以验证持续规则的最小执行语义，但仍不直接读写 ECS。

## 当前实现

新增代码：

```text
src/ir/ruleInterpreter.ts
scripts/test-ir-interpreter.cjs
scripts/test-ir-scenario.cjs
```

新增命令：

```powershell
npm.cmd run test:interpreter
npm.cmd run test:scenario
```

解释器入口：

```ts
interpretFunctionRule(rule, { input })
interpretStateRule(rule, { input, runtime })
```

RuntimeBackend trace smoke 接入：

```ts
backend.runIrRuleForTrace(rule, { input, runtime })
backend.getRuntimeTrace().irRules
```

这不是正式 gameplay 调度，只用于验证 Runtime Trace 能承载 IR rule 执行记录。

编辑器 Runtime Trace 面板当前会展示：

```text
IR ruleId
trace label / op
IR node path
featureId
requirementIds，作为 hover title
```

真实 runtime system 内部 IR trace 当前通过：

```text
FrameContext.irTrace
  -> current prototype RuntimeBackend.getRuntimeTrace().irRules
  -> Runtime Trace UI
```

返回：

```text
ok
output
runtime
events
commands
trace
errors
ops
```

错误结构：

```text
kind = ir-interpreter
ruleId
path
message
sourceMap
```

## 支持的表达式

当前支持：

```text
const
get / input / output
var
add / sub / mul / div
min / max
eq / neq
lt / lte / gt / gte
and / or / not
```

当前拒绝：

```text
call_native
```

原因：

```text
native call 白名单还未设计。
过早开放 native call 会让解释器变成隐藏脚本入口。
```

## 支持的语句

当前支持：

```text
let
set_var
set_output
emit_event
emit_command
if
for_each，受 maxItems / budget 限制
trace
```

Function Rule 当前拒绝：

```text
write_runtime
```

原因：

```text
Function Rule 不应该修改 runtime state。
State / Lifecycle Rule 可以通过 write_runtime 产生 runtime patch。
```

## 当前测试用例

已覆盖：

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
scenario: direction move -> hit -> damage output
scenario: fatal damage emits DeathCandidate
scenario trace keeps source map for each rule
```

## 当前边界

已经完成：

```text
Function Rule 可以解释执行。
State / Lifecycle Rule 可以执行 frame_update MVP。
解释器会先跑 IR validator。
解释器错误带 ruleId / path / sourceMap。
解释器支持 output / event / command / trace。
解释器支持 budget maxOps / maxEmits。
解释器返回 runtime state，不直接写 ECS。
Scenario test 可以串联 Function Rule 和 State / Lifecycle Rule。
Spin3D prototype system 已经受控接入 Function Rule。
Spin3D prototype system 内部 IR trace 已进入 RuntimeTraceReport.irRules。
GameScore prototype system 已经受控接入 Function Rule。
GameScore prototype system 内部 IR trace 已进入 RuntimeTraceReport.irRules。
```

暂未完成：

```text
Runtime state preservation。
Lowered Execution IR。
Interpreter vs Rust AOT equivalence test。
native call 白名单。
IR rule 自动接入每帧 gameplay system。
更多 prototype systems 接入 IR Interpreter。
IR trace source map 点击跳转。
```

## 下一步

阶段 6 已通过两个真实 runtime system 收敛验证。

详细结论见：

```text
24-阶段6收敛-真实RuntimeSystem接入IR.md
```

Asset DB / Importer MVP 已完成第一轮收敛。当前 IR Interpreter 的后续主线不再继续扩大 TypeScript runtime，而是进入 ProjectLogicRunner / LogicExecutor / Rust ECS 正式接入。

详见：

```text
31-Project-Logic-Runner-IR-RustAOT-ECS方案.md
```
