# Input System MVP 总览

本轮完成 42 号施工清单中的 Input System MVP。

## 实现范围

```text
Input 数据模型
Input Resolver 纯函数
Golden Scenario / Replay 可用的 ActionSnapshot 数据层
```

## 修改文件

```text
src/input/inputModel.ts
src/input/inputResolver.ts
src/input/inputScenario.ts
scripts/test-input-model.cjs
scripts/test-input-resolver.cjs
scripts/test-input-scenario.cjs
package.json
```

## 验证

```powershell
npm.cmd run test:inputmodel
npm.cmd run test:inputresolver
npm.cmd run test:inputscenario
npm.cmd run build
```

结果：通过。`npm.cmd run build` 保留既有 Vite chunk size warning。

## 边界

```text
不接 UI。
不接 RuntimeBackend。
不接真实设备监听。
不修改 Project Schema。
不改变现有游戏行为。
```
