# Golden Scenario 输入快照

## 实现

新增 `src/input/inputScenario.ts`。

提供：

```text
createInputFrameSnapshot
normalizeInputFrameSnapshot
hashInputFrameSnapshot
validateInputScenarioFrames
```

## 规则

Golden Scenario / Replay 记录 ActionSnapshot，而不是 raw device event。

这让同一动作可以来自不同设备，同时保持测试和 AI 证据稳定。

## 验证

```powershell
npm.cmd run test:inputscenario
```

覆盖：

```text
same action snapshot produces same hash
different action value changes hash
raw device difference does not matter after same action snapshot
frame records active contexts
frame records trigger phase
frame records sources for debugging
```
