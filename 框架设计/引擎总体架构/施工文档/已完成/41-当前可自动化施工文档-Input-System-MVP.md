# 当前可自动化施工文档：Input System MVP

> 状态提示：Input System MVP 已完成，本文档保留为历史施工依据。当前开工入口以 `49-当前状态与下一步入口.md` 为准。

本文档定义当前可以自动化实现的 Input System MVP。

它只实现纯数据层和测试，不接 UI，不接真实 Rust Runtime，不改变当前游戏运行行为。

## 施工前提

已经确认架构规则：

```text
40-Input-System路线.md
```

本次施工必须遵守：

```text
Action-first。
Processor / Trigger / Activation Rule 必须存在。
项目逻辑不直接面对 Raw Device Event。
Golden Scenario / Replay 记录 ActionSnapshot。
```

## 自动化任务 1：Input 数据模型

### 目标

新增 Input System 的 TypeScript 数据模型，用于后续 Rust Runtime 和编辑器共同参考。

### 建议新增文件

```text
src/input/inputModel.ts
```

### 允许实现

```text
InputActionDefinition
InputActionKind = button / axis1 / axis2 / pointer / gesture
InputBinding
InputContext
InputProcessorDefinition
InputTriggerDefinition
InputActionState
InputActionSnapshot
InputFrameSnapshot
RawInputEvent
```

### 不允许实现

```text
不接 App.tsx。
不接 RuntimeBackend。
不修改 Project Schema。
不修改现有游戏行为。
不加入真实键鼠监听。
```

### 测试要求

新增：

```text
scripts/test-input-model.cjs
```

测试覆盖：

```text
button action can be defined
axis1 / axis2 action can be defined
pointer action can be defined
gesture action can be defined
binding references existing action
context has priority
unknown action binding fails validation
```

执行：

```powershell
npm.cmd run test:inputmodel
npm.cmd run build
```

## 自动化任务 2：Input Resolver 纯函数

### 目标

实现 RawInputEvent -> ActionSnapshot 的纯函数。

### 建议新增文件

```text
src/input/inputResolver.ts
```

### 允许实现

```text
resolveInputFrame(definitions, contexts, previousSnapshot, rawEvents)
applyProcessor
evaluateTrigger
context priority
consume input
```

### 必须支持的 Processor

```text
deadzone
normalize
scale
invert
clamp
```

### 必须支持的 Trigger / Activation Rule

```text
press
hold
release
tap
doubleTap
threshold
chord
```

### 必须支持的 Action 类型

```text
Button
Axis1
Axis2
Pointer
Gesture
```

### 不允许实现

```text
不做真实设备监听。
不接浏览器 KeyboardEvent。
不接 Electron。
不接 Rust。
不接 UI。
不做自定义脚本 Trigger。
```

### 测试要求

新增：

```text
scripts/test-input-resolver.cjs
```

测试覆盖：

```text
keyboard WASD resolves to move Axis2
mouse left resolves to fire Button
gamepad-like raw event resolves to move Axis2
touch virtual joystick raw event resolves to move Axis2
doubleTap resolves to dash
hold resolves only after duration
release fires on release
threshold ignores below threshold
deadzone filters small axis
UI context consumes pointer and blocks gameplay fire
chord requires modifier action
```

执行：

```powershell
npm.cmd run test:inputresolver
npm.cmd run build
```

## 自动化任务 3：Golden Scenario ActionSnapshot

### 目标

验证 Input System 能服务 Golden Scenario / Replay，而不是只服务实时输入。

### 建议新增文件

```text
src/input/inputScenario.ts
scripts/test-input-scenario.cjs
```

### 允许实现

```text
createInputFrameSnapshot
normalizeInputFrameSnapshot
hashInputFrameSnapshot
validateInputScenarioFrames
```

### 测试覆盖

```text
same action snapshot produces same hash
different action value changes hash
raw device difference does not matter after same action snapshot
frame records active contexts
frame records trigger phase
frame records sources for debugging
```

执行：

```powershell
npm.cmd run test:inputscenario
npm.cmd run build
```

## 自动化任务 4：文档回填

实现完成后更新：

```text
40-Input-System路线.md
施工文档/已完成/41-当前可自动化施工文档-Input-System-MVP.md
阶段完成记录/YYYY-MM-DD-Input-System-MVP/
```

阶段记录至少包含：

```text
00-Input-System-MVP总览.md
01-Input数据模型.md
02-InputResolver纯函数.md
03-GoldenScenario输入快照.md
```

## 暂停条件

遇到以下情况必须暂停讨论：

```text
需要修改 Project Schema 核心结构。
需要修改 RuntimeBackend 接口。
需要接真实 Rust Runtime。
需要接真实设备监听。
需要改变当前编辑器 UI 行为。
需要让项目逻辑直接读 KeyCode / MouseButton。
需要新增自定义脚本 Trigger。
```

## 推荐执行顺序

```text
1. Input 数据模型
2. Input Resolver 纯函数
3. Golden Scenario ActionSnapshot
4. 文档回填
```

每完成一个任务必须先跑对应测试，再进入下一个任务。

## 当前建议立即实现

可以直接从任务 1 开始：

```text
Input 数据模型
```

原因：

```text
纯数据层。
不改变当前运行行为。
不接 UI。
不接 Rust Runtime。
能立刻写自动化测试。
符合已确认 Input System 架构规则。
```

## 当前实现记录

2026-06-22 已完成 Input System MVP 纯数据层：

```text
src/input/inputModel.ts
src/input/inputResolver.ts
src/input/inputScenario.ts
scripts/test-input-model.cjs
scripts/test-input-resolver.cjs
scripts/test-input-scenario.cjs
```

新增测试命令：

```powershell
npm.cmd run test:inputmodel
npm.cmd run test:inputresolver
npm.cmd run test:inputscenario
```

已验证：

```powershell
npm.cmd run test:inputmodel
npm.cmd run test:inputresolver
npm.cmd run test:inputscenario
npm.cmd run build
```

边界保持：

```text
未接 App.tsx。
未接 RuntimeBackend。
未修改 Project Schema。
未改变当前游戏运行行为。
未加入真实设备监听。
```

