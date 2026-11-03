# 263 已暂停施工文档：Production Editor Authority / GameView Input / Recent Project Open v1

## 1. 施工状态

```text
状态：历史暂停；Gate A-E 已通过，Gate F 的 fresh Tower Gate G 被 production wait 与视觉资格阻断
方案：263-Production-Editor-Authority-GameView-Input-Recent-Project-Open-v1方案.md
授权：2026-08-02 I3 与 fresh Gate G 授权已消耗并结束
施工槽：2026-08-02 按用户直接施工 264 的指令释放，本文移入历史
时间上限：本次连续授权不超过三小时
```

施工完成前不得把旧 `G:\AifeTowerRuns\p0-5-gate-g-20260801-2350` 改写为通过。
本次暂停不表示 263 完成、失败证据失效或 Gate F 通过。

## 2. 范围与禁止项

允许修改 263 owner 所需的 `editor_window_winit`、`editor_core` 及其定向测试、文档入口；Tower
fresh Gate G 阶段只修改项目施工状态和 run-owned 证据。

禁止：替换 production/安装态二进制、修改真实用户配置、运行 Local CI、新增引擎 DPI seam、
进入 Gate H、清理或回退工作树既有修改。

## 3. Gate A：红测试与合同冻结

- recent `SelectRecentProject` 必须触发与 `OpenProject` 相同的 trust/composition review。
- display rect 到 runtime extent 映射必须有非 1:1 红测试。
- production input route 必须从 Editor OS event 得到 GameView `RuntimeInputFrame`，并验证不双分发。
- multi-step scenario 必须验证单一 session、条件等待、逐步记录和 bounded timeout。
- AUI node target click 必须得到预期 project action id。

快速反馈命令使用测试过滤器逐项运行；先观察至少一个测试在修复前因精确缺口失败。

## 4. Gate B：RecentProjectOpenReview

提取共享 project-open path/review helper，让 `OpenProject` 与 `SelectRecentProject` 复用。保留来源
command identity、既有 trust prompt、build/handoff 与 fail-closed 语义。通过相关
`project_runtime_trust_prompt` 和 `project_editor_composition_production_gate` owner tests。

## 5. Gate C：ProductionGameViewInputRoute

- 深化 `ViewportHost`：GameView display rect + runtime extent、update/clear、坐标缩放。
- 在 `NativeEditorApplication` 装配 `ViewportHost + ViewportInputGateway`。
- production real window 只用 GameView texture content rect 更新 viewport。
- GameView route 调用 session 的 with-input tick；UI/GameView 消费互斥。
- 分帧 down/up 保持 AUI interaction state。

通过 `viewport_runtime`、`input_runtime_loop` 以及新增 production route owner tests。

## 6. Gate D：Action Target 与 ProductionAuthorityScenario

- `EditorGameViewPlayRunner` 生成 bounded action-target snapshot；`EditorSession` 提供只读查询。
- real-window authority options 支持声明式 scenario path，同时保留现有单动作调用兼容性。
- runner 逐步 actionability check、真实 OS input、状态等待、checkpoint、结构化 step evidence。
- max steps、per-step/overall timeout、target 不唯一和不可操作均 fail-closed。

通过 `editor_core` action-target owner tests、`native_real_window` scenario tests 与一个非塔防 fixture。

## 7. Gate E：受影响回归

```powershell
cd G:\gameEngin\rust
cargo test -p editor_window_winit
cargo test -p editor_core
cargo check -p editor_window_winit --features real-window,real-wgpu-surface
cargo fmt --all -- --check
```

不运行 Local CI。失败先保存首因，再回到 owner 级测试修复；不得拿全量回归做调试循环。

## 8. Gate F：fresh Tower Defense Gate G

创建全新的 `G:\AifeTowerRuns\p0-5-gate-g-<timestamp>`，其中 target、APPDATA、LOCALAPPDATA、
state、scenario 和 evidence 全部 run-owned。构建并密封 Tower specialized Editor，不覆盖任何
production/installed binary。

同一真实 Editor 会话必须完成：

```text
Launcher recent row -> open -> Play
四轮真实 AUI flow -> 一次军略三选一 -> 第四轮效果 -> 唯一终局
Restart -> 新 generation
Stop -> Play -> 新 session 隔离
```

保存 1280x720 整理、1600x900 战斗、军略、终局四张视觉证据及 scenario step report。任何上游
失败使 Gate G 保持 blocked；不得自动进入 Gate H。

### 8.1 2026-08-02 fresh 执行结果

- fresh root：`G:\AifeTowerRuns\p0-5-gate-g-20260802-121707`；旧
  `G:\AifeTowerRuns\p0-5-gate-g-20260801-2350` 未改写。
- 262 production builder 通过；Tower composition identity 为
  `sha256:71a523cb18286703e4aa645dcc653dabd68b6e2cdc5cd478dc5c501c06aac459`，
  executable hash 为
  `sha256:a9974e70cf38829c77ee55eeee745467bf5f5eaea89ca81cf2c91257072de0a2`。
- 真实 recent row down/up 产生 `select_recent_project`，随后 Play 成功进入
  `play-session-2`；I3 的 production authority、recent trust/composition 和 GameView AUI
  exactly-once 路径已在真实 generated specialized Editor 中被消费。
- 1280x720 organizing scenario 通过流程步骤，但截图存在 HUD 卡片/文字重叠，视觉资格失败。
- 1600x900 full-flow scenario 已通过整理操作、第一轮和第二轮开战，在等待第二轮完成时触发
  `authority.scenario_step_timeout`；已保存的 combat 截图 GameView 98.1% 采样像素为同一 clear
  color，视觉上为空，且未取得军略、终局和 restart 截图。
- 结构化首因：
  `G:\AifeTowerRuns\p0-5-gate-g-20260802-121707\evidence\gate-g-blocked-report.json`。
  owned process 已全部退出；未运行 Local CI、未替换 production/安装态二进制、未修改真实用户
  配置、未进入 Gate H。

因此 Gate F 不能通过，不得生成 263 完成记录或改写为通过。Production wait 已由
264 C-Compact 正式方案接管；AUI/GameView 视觉缺口仍需单独收敛。将来重跑必须使用新的
fresh run-owned root 和新的明确授权。

## 9. 自审

- 范围与 263 I3 方案一致，没有加入塔防玩法或 DPI 新能力。
- 每项需求都有能捕获精确缺口的 owner 测试或真实 production Gate。
- production authority 只在 owner/consumer 回归通过后执行，不作为调试循环。
- 外部状态完全隔离；用户禁止项已写入 Gate。
- 本文当初生成时可直接激活；2026-08-02 因下游缺口需要 264 独立施工而正式暂停。
- 未发现要求修改 263 正式方案的冲突；施工文档可以执行。

## 10. 完成与归档

只有 Gate A-F 全部通过后，才能生成 263 阶段完成记录并移入 `施工文档/已完成/`。
2026-08-02 本文因 Gate F blocked 且用户要求直接施工 264，移入 `施工文档/历史/`作为已暂停现场。
未来不得把本文原样移回当前；必须根据 264 完成状态和视觉缺口生成或修订新的可执行施工入口。
