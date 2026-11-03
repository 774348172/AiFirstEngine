# 301 allocation failure owner 最小诊断

状态：owner 已定位；尚未修复，不改变 301 的未完成状态。

## 结论

分配压力 owner 是 `engine_runtime::render_thread_worker::RenderCommandDispatcher::inline_reports`，
不是 Tower 战斗、AUI 字体 atlas、launcher frame 或 WGPU present 本身。

`RenderCommandDispatcher::submit_frame_output()` 的 inline 分支在返回完整 immediate output 的同时，又把
`output.clone()` 和 `report.clone()` 放入 `inline_reports`。缓存只会在
`poll_submission_output(ticket)` 中删除。`EngineHostLoop` 的两个 immediate consumer 直接返回，不 poll
该 ticket，因此每帧永久保留一份完整 `RenderThreadFrameOutput`。Tower 的 487 个 AUI draw item 会放大每帧
retained output，最终任意后续分配都可能成为失败申请者；错误中的 1 MiB 与 glyph atlas allocation 大小吻合，
但 glyph atlas 只是可能的终局申请点，不是持续增长 owner。

## 有界 A/B 证据

run-owned harness：
`G:\AifeProductionRuns\post-300-allocation-diagnostic-20260819-073530\owner-harness`

相同 487-item AUI composition，唯一变量为 submit 后是否 poll 当前 ticket：

| 样本 | 帧数 | 起始 Private Bytes | 终止 Private Bytes | 净增长 | 结果 |
| --- | ---: | ---: | ---: | ---: | --- |
| no poll | 600 | 2,084,864 | 124,411,904 | 122,327,040 | 线性增长，约 203,878 bytes/frame |
| poll every frame | 600 | 2,068,480 | 3,342,336 | 1,273,856 | 早期分配后平台化 |
| no poll repeat | 300 | 2,084,864 | 63,463,424 | 61,378,560 | 再现约 204,595 bytes/frame |
| poll repeat | 300 | 2,080,768 | 2,568,192 | 487,424 | 平台化 |

真实 Editor Play 样本约 48 秒净增 499.69 MiB，而 idle 对照不增长；这与 Tower 真实 composition 比微型
fixture 包含更多 texture、glyph、report 和 runtime payload 相符。此前 CPU `UiGpuDrawPlan`、真实 launcher
DrawList WGPU present、241 RHI command WGPU present 与 `NativeEditorApplication::frame()` 均已平台化，
支持排除邻近 owner。

结构化证据：
`G:\AifeProductionRuns\post-300-allocation-diagnostic-20260819-073530\evidence\allocation-owner-diagnostic.v1.json`

可重复的单命令 red-capable verifier：

```powershell
& 'G:\AifeProductionRuns\post-300-allocation-diagnostic-20260819-073530\owner-harness\verify-dispatcher-owner.ps1'
```

本次实际结果为 `owner_located`；300 帧 no-poll 净增 61,497,344 bytes，poll 对照净增 495,616 bytes，
增长分离为 124.08 倍。

## 源码位置

- `render_thread_worker.rs:480`：`inline_reports` owner。
- `render_thread_worker.rs:545`：每帧 push 完整 cloned output/report。
- `render_thread_worker.rs:577-587`：只有 poll 才 remove cached submission。
- `engine_host_loop.rs:479-480`、`:750-751`：immediate 路径直接 return。

## 最小修复边界

后续若单独授权修复，只需让 inline-immediate 路径不留下第二份 cached output，或在 immediate consumer 返回前
retire 对应 ticket。无需新增 schema、全局缓存、GPU poll、渲染器重构或 production 工作流。修复前应先把本
A/B 变成 owner-level red test；修复授权不自动包含 production Editor 替换或 301 重试。

本次未修改产品源码、未运行 Local CI、未替换 production Editor、未重建真实 Tower Preview cache、未修改
真实配置或 trust store，也未重试 301 production mutation。诊断进程残留为 0。
