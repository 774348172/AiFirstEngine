# 301 allocation owner one-owner source repair

状态：源码修复完成；production 未更新，301 主 Gate 仍未完成。

## 改动

只修改 `rust/crates/engine_runtime/src/render_thread_worker.rs`：

- `InlineDeterministic` 已直接交付 immediate `RenderThreadFrameOutput`，不再向 `inline_reports` 放入第二份
  `output.clone()` / `report.clone()`。
- dedicated worker 的异常 fallback cache 保持原状。
- 新增 `inline_dispatcher_does_not_retain_immediate_submissions`，连续 256 帧锁定 immediate 可用且 owner
  不产生 duplicate retention。

没有修改 `EngineHostLoop`、公共 API/schema、renderer、GPU poll 或 production workflow。

## 红绿证据

同一 run-owned public-API harness、同一 487-item AUI composition：

| 阶段 | 300 帧 Private Bytes 净增长 | 结果 |
| --- | ---: | --- |
| 修复前 dispatcher no-poll | 61,534,208 | red，线性保留 |
| 修复后 dispatcher no-poll | 643,072 | passed，平台化 |
| 修复后 dispatcher poll | 479,232 | passed，平台化 |
| 修复后 EngineHostLoop consumer | 729,088 | passed，平台化 |

验证命令：

```powershell
& 'G:\AifeProductionRuns\post-300-allocation-diagnostic-20260819-073530\owner-harness\verify-dispatcher-repair.ps1'
```

`cargo check -p engine_runtime --locked` 与 `cargo fmt -p engine_runtime -- --check` 均通过。

仓库 owner test 的执行被既有 `render_thread.rs:552` test fixture 编译错误阻断：fixture 给
`RenderThreadFrameInput` 填入不存在的 `game_view_presentation` 字段（E0560）。该问题在本修复范围之外，未修改；
本次用同 owner public API 与真实 `EngineHostLoop` immediate consumer harness 提供替代绿证据。

结构化报告：
`G:\AifeProductionRuns\post-300-allocation-diagnostic-20260819-073530\evidence\allocation-owner-repair.v1.json`

## 外部状态

- production Editor 保持 `E27EE515...BF163`。
- Tower Preview cache 保持 93 files，manifest `6710E237...755D8`。
- Editor/Cargo/rustc/harness 残留进程为 0。
- 未运行 Local CI，未修改真实配置/trust store，未重试 301 production mutation。

下一步若要让普通 Editor 获得修复，需要新的 production consistency 授权；该权限不包含在本次 source repair 中。
