# 265 已完成施工文档：AUI Effective Visibility / Stable GameView Surface / Editor Frame Publication v1

## 1. 施工状态

```text
方案：265-AUI-Effective-Visibility-Stable-GameView-Surface-Editor-Frame-Publication-v1方案.md
方案选择：C
文档状态：已生成、已自审、已通过激活前复核
施工状态：Gate A-E 已通过，已完成并归档
授权：2026-08-03 用户要求根据 265 生成、自审施工文档并直接开始施工
时间上限：本次连续施工不超过三小时
```

当前施工槽和待执行队列在激活前均为空。本文是 `施工文档/当前/` 唯一非 README 施工文档。

## 2. 施工目标

完成 265 方案定义的项目无关链路：

```text
AUI local visibility
  -> inherited effective visibility
  -> draw/interaction single truth
  -> hidden-subtree state reconciliation

explicit Runtime advance
  -> Produced GameView frame identity
  -> stable session/target surface
  -> publication receipt
  -> Editor compose/sample/present
  -> optional exact readback
```

本施工只完成引擎 owner/consumer 源码和自动化验证。Tower Defense P0-5 Gate G 是完成后的独立
真实消费资格，不在本次授权内。

## 3. 授权与禁止项

允许：

- 修改 265 明确涉及的 engine-owned Rust source 和 tests。
- 新增一个深 `EditorFramePublicationModule` 及其小 Interface。
- 修改已有 GameView frame/viewport registry typed contract。
- 运行 owner 级定向测试、受影响 crate 回归、real-WGPU feature 测试/check 和 format check。
- 同步施工状态、完成记录和文档入口。

禁止：

- 修改 `samples/tower_defense_project` gameplay、AUI 或项目资产。
- 重跑 Tower Defense P0-5 Gate G 或进入 Gate H。
- 运行 Local CI。
- 替换 production/安装态 Editor 或 Player。
- 修改真实用户配置、recent-project store 或 stable MCP install。
- 新增 DPI seam、layout collapse、通用 RenderGraph 重构、多 viewport 或跨 device sharing。
- 清理、回退或覆盖工作树中的既有修改。

## 4. 代码基线与 Ownership

工作树包含 260-264 及相关历史施工的现存未提交变更。本施工在当前字节基线上增量编辑，不以
Git HEAD 覆盖这些文件。

| Owner | 预期文件 |
|---|---|
| AUI computed/interaction | `engine_runtime/src/aui.rs` |
| Runtime GameView descriptor | `engine_runtime/src/runtime_renderer.rs` |
| Editor GameView typed consumer | `editor_core/src/editor_gameview_play.rs`, `session.rs`, `ui_model_composer.rs` |
| Stable surface registry | `editor_wgpu_renderer/src/viewport_texture.rs`, `tests.rs` |
| Deep publication Module | `editor_window_winit/src/editor_frame_publication.rs`, `lib.rs` |
| Native/authority consumers | `editor_window_winit/src/application.rs`, `real_window.rs`, owner tests |

实际 owner 若已在邻近深 Module 中，深化现有 owner；不得为了匹配表格新增 pass-through。

## 5. Gate A：AUI Effective Visibility

### A1. Computed state

- 将 computed node 的语义明确为 effective visibility。
- parent-before-child 计算 `parent.effective && local`。
- layout 继续为隐藏节点计算几何，不增加 layout collapse。
- draw、hit-test、focus、navigation、action 和 modal 只消费 computed effective visibility。

### A2. Reconciliation

在下一批输入处理前，清理落在有效隐藏子树中的：focus、pressed、drag、scroll capture、
text input/IME owner、modal capture 和 navigation state。清理幂等，不产生 click/action。

### A3. Red-capable tests

必须覆盖：隐藏父/可见子不 draw/hit/focus；几何仍存在；focused input/drag/scroll/modal 在隐藏后
被清理；重复处理无重复 action。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime aui_effective_visibility -- --nocapture
```

2026-08-03 Gate A 结果：

```text
2 passed；0 failed；502 filtered out
hidden parent 保留 child layout rect，同时阻止 draw/hit
focus/pressed/drag/scroll capture/IME/modal reconciliation 一次完成且重复调用幂等
```

## 6. Gate B：Runtime 内容与稳定 Surface 身份

### B1. 内容身份

GameView frame 增加或收敛 `sessionId/frameIndex/frameHash` typed identity。Runtime renderer 的
viewport descriptor 不再把 `frame-N` 编入 texture identity。

### B2. Stable surface key

surface identity 由 active `(sessionId,targetId)` 确定性生成。相同 session/target 的连续帧必须
得到同一 key；restart/session switch 必须不同。

### B3. 显式 advance

present getter 改为只读 last-produced frame；Runtime tick 只通过显式 scheduler/Step/test advance。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime runtime_renderer -- --nocapture
cargo test -p editor_core game_view -- --nocapture
```

## 7. Gate C：Registry、Generation 与 Receipt

### C1. Stable backing lifecycle

`EditorViewportTextureRegistry` 在规格不变时复用 GPU backing，不增加 generation 或 lifecycle
allocation event；尺寸/格式/device backing 变化才创建并原子替换，失败保留 last-good。

### C2. Publication receipt

增加 compact typed receipt，至少绑定：content identity、surfaceId、generation、publicationIndex、
submit serial、extent/format/status。只有成功提交 Runtime write 才增加 publicationIndex；redraw reuse
不增加。

### C3. Exact readback

readback 校验请求 receipt 与 registry 当前 session/surface/generation/publication/frame 一致。普通帧
不等待 GPU；只有显式 readback 执行 copy/map。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_wgpu_renderer viewport_texture --features real-wgpu -- --nocapture
```

## 8. Gate D：深 EditorFramePublicationModule 与 Consumer Cutover

### D1. Interface

Module 外部只暴露 publish 和 last-good 查询；caller 不传 texture ID、generation、顺序 flag、fence
或 fallback policy。deterministic adapter 测试从 Interface 验证顺序和结果。

### D2. 唯一顺序

```text
ensure surface
-> submit Runtime write
-> issue receipt
-> compose draw list using stable surfaceId
-> sample/present
-> optional exact readback
```

普通 real window、production authority 和 preview evidence 路径统一消费此 Module 或其唯一 owner，
删除重复 publish/compose 顺序。没有新 frame 的 redraw 复用 last-good，不能 tick Runtime。

### D3. Tests

必须覆盖 stable frames、redraw reuse、resize generation、failed replacement last-good、session switch、
receipt monotonicity、exact capture identity、compose-before-publish 防回归和 getter-no-tick。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit editor_frame_publication -- --nocapture
cargo test -p editor_window_winit viewport_runtime -- --nocapture
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
```

## 9. Gate E：受影响回归与闭环

### Gate B-D 施工结果

```text
Runtime descriptor：连续 frame 使用同一 stable surfaceId，session switch 更换 surfaceId
Registry：规格不变复用 backing；resize 才推进 generation；成功 submit 才推进 publicationIndex
Receipt：绑定 content/surface/generation/publication/submit/extent/format/status
Publication Module：redraw 复用 last-good，不执行 Runtime submit；frame regression/invalid receipt fail closed
Window consumer：present getter 只读；普通窗口与 production authority 均为 publish -> compose -> present
Exact capture：receipt 当前性与 readback generation/publication/frameHash/submitSerial 完整比对
```

前序 Gate 已覆盖的同配置定向测试不机械重跑，最终以受影响 suite 替代：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p engine_runtime
cargo test -p editor_core
cargo test -p editor_wgpu_renderer --features real-wgpu
cargo test -p editor_window_winit --features real-window,real-wgpu-surface
cargo fmt --all --check
```

若 feature suite 已完整执行同 crate default tests，不再额外运行同一 crate default suite。任一失败先
保存首个 actionable diagnostic，在 owner 级复现和修复后只续跑失效证据。

完成后：

- 更新 265 施工结果和残留风险。
- 生成阶段完成记录。
- 更新 `00/49/54/施工文档 README/阶段完成记录 README`。
- 将本文从 `当前/` 移到 `已完成/`。
- 保持 Tower Gate G blocked，明确下一步需要独立授权。

2026-08-03 Gate E 结果：

```text
engine_runtime：504 passed
editor_core serial：616 passed，2 ignored
editor_wgpu_renderer + real-wgpu：21 passed
editor_window_winit + real-window,real-wgpu-surface：175 passed，10 ignored
cargo fmt --all --check：passed
```

`editor_core` 首轮并发 suite 为 615 passed、1 failed、2 ignored；唯一失败是 fixture copy 遇到
Windows OS error 32。owner 单测立即通过，最终以 `--test-threads=1` 完整通过，未修改产品逻辑或
无关测试阈值。Local CI、真实 Gate G/Gate H、production replacement 与真实配置修改均未执行。

## 10. 验收标准

1. AUI hidden ancestor 的后代没有 draw/hit/focus/action，但 layout rect 保留。
2. 隐藏发生后所有相关 interaction transient state 被幂等清理。
3. 连续 runtime frames 使用稳定 surfaceId；规格不变不重建 GPU texture。
4. generation/publicationIndex 只按方案规定条件推进。
5. redraw/present/wait/capture 和 frame getter 不推进 Runtime。
6. Editor draw list 的 texture reference 与 registry stable surface 一致。
7. receipt 不伪称 GPU completion；exact readback 验证精确身份。
8. 普通窗口和 production authority 不再维护不同顺序实现。
9. 受影响 suite 和 format check 通过。
10. 没有塔防玩法、DPI 或 construction governance 进入 runtime Interface。

## 11. 施工文档自审

### 范围

- [x] 只实现 265 已确认方案，没有扩张到 Tower gameplay、DPI 或 layout collapse。
- [x] Tower Defense 只保留下游 Gate G 身份，本施工不修改项目。
- [x] production replacement、真实配置、Local CI 和真实视觉矩阵未获授权并明确禁止。

### Interface 与测试

- [x] 深 Module Interface 小，排序/generation/fallback 留在 implementation。
- [x] owner 和跨 crate consumer 均有 red-capable tests。
- [x] 普通帧与 exact readback 的成本/证据等级分开。
- [x] 最终回归覆盖共享 contract 的受影响 closure，不要求无关 workspace suite。

### 基线与激活

- [x] 当前施工槽、待执行队列和 265 正式方案已重新核对。
- [x] dirty worktree 被视为既有用户/历史修改，不回退、不清理。
- [x] 没有需要先修订 265 正式方案的冲突。
- [x] 本文可在当前空槽直接激活，用户已明确要求立即施工。

自审结论：施工文档与 265 一致，验证范围与授权相称，激活前复核通过。

## 12. 2026-08-03 完成后 consumer 修正

Tower P0-5 fresh Gate G builder 发现 `project_e2e_gate/src/second_project_runtime.rs` 的
Switch Puzzle consumer 把 `effective_visible` 错误地当作 resolved `AuiNode` 字段读取。用户已单独
授权修复；实现改为读取 `present.layout.computed_nodes` 中对应节点的
`AuiComputedNode.effective_visible`，没有修改 265 Interface 或扩大施工范围。

定向 consumer 回归 1/1 通过，`cargo fmt -p project_e2e_gate -- --check` 通过。Tower Gate G、
Gate H、Local CI、production replacement 与真实配置修改均未执行。
