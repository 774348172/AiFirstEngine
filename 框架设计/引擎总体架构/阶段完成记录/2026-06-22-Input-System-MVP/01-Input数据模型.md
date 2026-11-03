# Input 数据模型

## 实现

新增 `src/input/inputModel.ts`。

第一版包含：

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

## 验证

```powershell
npm.cmd run test:inputmodel
```

覆盖：

```text
button / axis1 / axis2 / pointer / gesture action 定义
binding 引用已有 action
context priority
unknown action binding 失败
```
