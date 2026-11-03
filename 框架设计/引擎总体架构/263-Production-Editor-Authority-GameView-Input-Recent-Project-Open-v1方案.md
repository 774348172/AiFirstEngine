# 263 Production Editor Authority / GameView Input / Recent Project Open v1 方案

## 1. 状态与结论

```text
系统编号：263
方案：I3
状态：用户已确认，允许施工
日期：2026-08-02
上游：262 Trusted ProjectRust Editor Composition Artifact v1
下游：Tower Defense P0-5 Gate G fresh qualification
```

采用 I3：把“从 Launcher recent row 打开可信项目、在 production GameView 中输入、在同一真实
Editor 会话中执行有界多步验收”收敛为三个相邻但职责分离的 owner。263 是通用 Editor 能力，
不包含塔防玩法知识；塔防只通过稳定 AUI node/action identity 消费它。

## 2. 已证实缺口

1. `OpenProject` 会进入 ProjectRust trust/composition review，`SelectRecentProject` 却直接派发，
   recent row 没有复用同一安全入口。
2. production `RealNativeEditorApp` 只把 OS 输入交给 Editor UI；现有
   `ViewportHost + ViewportInputGateway` 只在 headless/测试路径装配，真实 GameView 无法形成
   `RuntimeInputFrame`。
3. real-window authority 只有单个 click/wheel/drag 后退出，不能在一个真实 session 中执行
   Open、Play、四轮 AUI、军略、终局、Restart 和 Stop/Play 隔离。

旧 Tower Defense Gate G run 仍为 `BLOCKED`，不得作为 263 或 fresh Gate G 的通过证据。

## 3. 目标链路

```text
Launcher recent row
  -> shared project-open review
  -> trust / composition match or prompt-build-handoff
  -> project opened by matching specialized Editor
  -> Play
  -> production GameView content rect
  -> OS pointer event
  -> ViewportInputGateway
  -> RuntimeInputFrame in runtime texture coordinates
  -> EditorSession active GameView tick
  -> AUI exactly-once action
  -> project RuntimeModule
```

同一链路必须可以由 bounded production authority scenario 观察、驱动和记录，但 authority 不得
成为 runtime 输入实现的第二条捷径。

## 4. Owner A：RecentProjectOpenReview

`OpenProject` 与 `SelectRecentProject` 只在来源 command identity 上不同；两者必须先提取项目路径，
再进入同一个 trust/composition review：

- 无 ProjectRust：正常打开。
- ProjectRust 且当前 specialized composition identity 匹配：正常打开。
- 普通 Editor 或 identity 不匹配：显示既有 trust prompt，批准后 build/handoff。
- 拒绝、构建失败、handoff 失败：保持 fail-closed，保留来源 command id 与结构化诊断。

不得为 recent row 增加信任旁路，也不得把失败结果硬编码为 `open_project`。

## 5. Owner B：ProductionGameViewInputRoute

`NativeEditorApplication` 持有并复用既有 `ViewportHost` 与 `ViewportInputGateway`。production
real window 每帧从 `ViewportTextureSlot` 注册 GameView 的实际内容 rect，而不是整个 panel rect；
没有有效 texture/frame 时清除 GameView viewport。

`ViewportHost` 同时记录显示 rect 与 runtime frame extent。输入坐标按以下合同转换：

```text
runtime_x = clamp(local_display_x / display_width  * runtime_width)
runtime_y = clamp(local_display_y / display_height * runtime_height)
```

PointerDown 命中 GameView 时先令 GameView 获得焦点，再将同一个事件路由；PointerDown 与
PointerUp 可以位于相邻 editor frames，但必须由 runtime interaction state 合成为一次合法 AUI
click。Editor UI 命中与 GameView 命中互斥，单个 OS 事件不得双分发。

路由得到的 `RuntimeInputFrame` 必须通过
`EditorSession::tick_active_game_view_runtime_descriptor_frame_with_input` 进入现有 session；不得直接
调用项目 action、伪造塔防命令或绕过 AUI。

## 6. Owner C：ProductionAuthorityScenario

新增 schema-first、声明式、bounded scenario，在单一真实 Editor process/session 中执行。最小步骤：

- Editor semantic widget click。
- GameView AUI node click。
- wait/assert Editor mode、command、runtime frame/action/session/generation。
- capture checkpoint。
- Stop / Play。

GameView AUI node click 使用 `EditorGameViewPlayRunner` 暴露的轻量只读 action-target snapshot：

```text
node_id
action_id
visible / interactable
computed_rect / effective_clip
reference_extent
```

snapshot 从当前 `AuiRuntimePresentOutput` 的 resolved document/layout 构建，不复制完整 AUI 文档、
全量 trace 或 project state。authority 将 runtime rect 经 viewport 映射为 OS client coordinate，
仍使用真实平台输入完成 down/up。

每一步必须记录：step id、target、actionability、OS down/up observation、before/after command、
runtime action、frame/session/generation、截图/hash、timeout 与诊断。等待使用状态条件自动重试，
不得用固定 sleep 表示成功。runner 必须具备 max steps、per-step timeout 和 overall timeout。

## 7. 安全与通用性

- recent row 不自动信任项目；继续复用 262 trust/composition policy。
- authority scenario 只驱动公开 production 行为，不注入项目私有状态。
- AUI target 查询是只读、轻量、按当前 frame 生命周期更新的调试/验收表面。
- scenario 文件只能从显式路径加载，解析失败或 target 不唯一时 fail-closed。
- 不修改 production/安装态二进制，不修改真实用户配置。
- fresh qualification 使用 run-owned target/state/evidence root。
- 263 不引入新的引擎 DPI seam，不扩展塔防玩法 API。

## 8. 验收

1. recent-row 与 open-dialog 的 ProjectRust 项目走同一 review，并保留来源 command identity。
2. production app 的真实 OS GameView click 生成按 texture extent 映射的 `RuntimeInputFrame`。
3. Editor UI 与 GameView 不双分发；分帧 down/up 产生且只产生一次 AUI action。
4. bounded scenario 在一个 session 内完成多动作、条件等待、checkpoint 与证据记录。
5. 通用第二项目证明实现不含 Tower Defense 特判。
6. owner tests、`editor_window_winit`、受影响的 `editor_core`、real-window feature check 与格式检查通过。
7. fresh Tower specialized Editor Gate G 完成四轮、三选一、终局、Restart、Stop/Play 隔离及
   1280x720、1600x900、军略、终局四张视觉证据。

## 9. 非目标

- 不替换 production/安装态 Editor。
- 不运行 Local CI。
- 不修改真实 recent-project store 或其它用户配置。
- 不自动进入 Tower Defense Gate H。
- 不建立录制回放系统、通用 UI 测试 DSL、远程控制服务或项目脚本执行器。

