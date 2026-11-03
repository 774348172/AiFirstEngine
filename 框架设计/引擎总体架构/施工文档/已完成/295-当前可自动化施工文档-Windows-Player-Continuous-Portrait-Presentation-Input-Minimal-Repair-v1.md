# 295 Windows Player Continuous Portrait Presentation/Input Minimal Repair v1

状态：已完成并归档；用户于 2026-08-18 明确授权。

## 目标与边界

修复导出 Windows Player 在首个 `RedrawRequested` 内跑完全部帧并退出、无法接收真实输入的问题；让
Player 消费项目 Windows build profile 的竖屏 target，并让渲染和 AUI 输入消费同一个
`ResolvedGameViewPresentation`。

只修改 RuntimePackage/BuildProfile 的可选 target 传递、`runtime_player_winit` 持续逐帧会话、Renderer
presentation consumer 和 Tower Windows dev profile。不修改 Tower 玩法、不运行 Local CI、不替换
production Editor、不修改安装态配置。

## 必要性与验证经济性自审

```text
confirmed failure:
  first redraw runs all frames and exits; raw window coordinates bypass presentation inverse
minimum causal change:
  persistent player session + one redraw/one frame + existing target/presentation owner consumption
excluded:
  new renderer architecture, mobile export, production promotion, broad visual matrix
Gate A -> event/input/presentation contract -> runtime_player_winit + engine_runtime directed tests
Gate B -> Tower consumer -> one fresh Windows dev export and recruit/deploy/start/movement smoke
estimated cost:
  one affected build plus one real smoke; stop if work expands beyond these owners
```

## Gate A：Owner 修复

- [x] `BuildProfile.gameViewTarget` 可选传入 RuntimePackage project runtime facts，缺失保持 legacy target。
- [x] Windows Player 初始化一次 RuntimePackage/World/Host/GPU 资源；每次 redraw 只推进一帧。
- [x] 每帧解析一份 presentation；AUI target-space interaction 与 Renderer 共享该实例。
- [x] 定向测试覆盖多帧调度、720x1280 inverse hit、BuildProfile/RuntimePackage target 传递和 renderer consumer。

## Gate B：Tower Consumer

- [x] Tower Windows dev profile 指定 `720x1280 + contain`。
- [x] 新 run-owned root 导出 Windows dev package。
- [x] 真实点击征兵 `10 -> 7`、部署、出战；观察到怪物生成并连续移动。
- [x] 保留 package manifest、运行报告和关键截图；本轮 owned 进程已清理。

## 完成结果

fresh root：

```text
G:\AifeTowerRuns\tower-windows-player-295-complete-20260818-141000
```

最终 Windows Player 客户区与 target 均为 `720x1280`，`contain` 为 `scale=1 / offset=0,0`。真实交互
完成征兵、部署、出战和怪物移动；最终 report 为 `framesCompleted=82`、`presented`、`input/logic/rhi=ok`、
`diagnostics=[]`、`exitCode=0`。详细证据见：

```text
阶段完成记录/2026-08-18-Windows-Player-Continuous-Portrait-Presentation-Input-Minimal-Repair-v1/00-总览.md
```
