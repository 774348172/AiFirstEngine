# Canonical Rule IR v1

本文档记录当前阶段落地的 Canonical Rule IR v1。

## 当前修正：195 / 196 优先

本文是早期 IR 技术骨架记录。按 `195` / `196` 的当前规则：

```text
Gameplay Rule Asset / Contract-bound RuleSlot 是用户和 AI 面向的规则资产边界。
Canonical Rule IR 是受限 RuleSlot 的内部规范语义和构建输入。
它不是项目规则总真相层，也不是 Lua / Blueprint 式脚本语言。
```

## 定位

Canonical Rule IR 是受限项目规则片段的规范化中间表示。

它不是：

```text
不是自由脚本语言。
不是完整解释器。
不是 Rust AOT 产物。
不是用户直接手写代码的替代语言。
不是完整项目逻辑的默认承载层。
```

它是：

```text
AI / Rule Graph / DSL 生成受限 RuleSlot 后的稳定中间表示。
Validation / Trace / Test / Interpreter / Rust AOT 的共同输入。
受限规则片段是否可执行、可追踪、可回滚、可维护的核心契约。
```

## 当前 v1 覆盖

新增代码：

```text
src/ir/canonicalRuleIr.ts
schemas/canonical-rule-ir.schema.json
scripts/test-canonical-rule-ir.cjs
```

新增命令：

```powershell
npm.cmd run test:ir
```

当前 v1 支持：

```text
Function Rule
State Rule
Lifecycle Rule
frame_update 生命周期声明
read component / runtime / world / input / output
write runtime / output
emit event / command
budget
source map
trace spec
requirement coverage report
最小 simulate trace
```

## 标准结构

最小结构：

```json
{
  "schemaVersion": "canonical-rule-ir.v1",
  "ruleId": "combat.damage.calculate",
  "version": "1.0.0",
  "kind": "function",
  "deterministic": true,
  "inputs": [],
  "outputs": [],
  "reads": [],
  "writes": [],
  "emits": [],
  "budget": {
    "maxOps": 64,
    "maxNativeCalls": 0,
    "maxLoopItems": 0,
    "maxEmits": 1
  },
  "sourceMap": {
    "featureId": "FEATURE_COMBAT_DAMAGE",
    "requirementIds": ["REQ_DAMAGE_FORMULA"]
  },
  "trace": {
    "label": "damage.calculate",
    "fields": ["damage"]
  },
  "body": []
}
```

State / Lifecycle Rule 必须额外声明：

```text
lifecycle.hooks
lifecycle.runtimeState，可选
```

Function Rule 不允许声明 lifecycle。

## 当前测试用例

已覆盖：

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

## 设计边界

当前阶段只做 IR v1 结构和验证，不做完整执行。

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

## 为什么不直接接 runtime

当前 TypeScript runtime 仍然是 prototype backend。

如果在 IR v1 还未稳定时直接接 runtime，会出现两个问题：

```text
解释器行为和 IR schema 一起变化，Bug 难定位。
AI 会过早依赖临时执行细节，而不是稳定规则契约。
```

所以当前顺序是：

```text
先固定 IR v1 结构和验证。
再实现 IR Interpreter MVP。
再接 RuntimeBackend。
最后进入 Rust AOT / Rust Runtime 对照。
```
