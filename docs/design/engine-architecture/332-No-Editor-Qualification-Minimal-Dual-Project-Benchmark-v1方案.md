# 332 No-Editor Qualification + Minimal Dual-project Benchmark v1

状态：用户已确认采用方案 A。
日期：2026-09-12

## 1. 目标

用两个已有项目验证当前架构是否能在 Editor 不存在、未启动、未连接时完成最小开发与交付闭环，并建立第一份内部基线。项目为 `complex_shooter_project` 与 `switch_puzzle_project`。

## 2. 统一闭环

```text
inspect → mutate → check → run → playtest → observe → build → delivery_verify → rollback
```

箭头表示可用能力关系，不规定 AI 必须按固定顺序调用全部工具。每个项目按相同目标和相同验证规则执行。

## 3. 记录指标

- Editor 是否完全未参与；
- 每阶段 status、diagnostics 和失败恢复；
- 工具调用数量与调用时间；
- check/run/playtest/build/verify 耗时；
- revision、artifact、delivery identity 一致性；
- 最终 Technical、Gameplay、Visual、Delivery outcome；
- rollback 是否成功；
- 是否发生人工干预。

## 4. 范围约束

只复用现有九项 Engine tools、现有项目、现有宿主和现有 RuntimePackage/Compiler。首轮不与 Unity、Unreal、Godot、TapTapMaker 做排名，不新增 Benchmark 平台、工具、Provider 或运行时模块。

## 5. 验收结论

分别给出两个项目的闭环状态、耗时、失败点和证据身份；明确哪些能力已验证、哪些仍未验证。只有两个项目都完成最小闭环，才能进入后续三任务或六路径对比讨论。
