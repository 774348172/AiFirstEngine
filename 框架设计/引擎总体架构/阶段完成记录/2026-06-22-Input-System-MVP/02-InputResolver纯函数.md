# Input Resolver 纯函数

## 实现

新增 `src/input/inputResolver.ts`。

输入流：

```text
InputDefinitions
previous InputFrameSnapshot
RawInputEvent[]
frame / dt
  -> InputFrameSnapshot
```

已支持：

```text
deadzone / normalize / scale / invert / clamp
press / hold / release / tap / doubleTap / threshold / chord
context priority / consume
button / axis1 / axis2 / pointer / gesture
```

## 验证

```powershell
npm.cmd run test:inputresolver
```

覆盖：

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
