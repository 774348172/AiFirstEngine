# 273 GameView Presentation Target / AUI Input Coordinate Consistency v1 施工文档

状态：2026-08-05 Gate A-F 已完成，受影响回归与 source-built 真实视觉矩阵全绿；本文归档到
`施工文档/已完成/`。273 完成当时未自动恢复 272；272 后续取得独立授权并已完成归档。

正式方案：`273-GameView-Presentation-Target-AUI-Input-Coordinate-Consistency-v1方案.md`。

## 1. 施工目标

建立深 `GameViewPresentationModule`，用一份 typed、可逆、带 revision 的解析结果统一：

```text
AUI ReferenceSpace
Runtime TargetSpace
Editor GameView DisplaySpace
OS WindowSpace
```

Editor Play session 必须能显式选择 `1280x720`、`1080x1920` 与 `720x1280` target；AUI renderer、
GameView display、真实 pointer route、per-canvas AUI hit-test 与 production authority 必须消费同一
presentation truth。AUI 命中使用 ReferenceSpace，gameplay 输入继续使用 TargetSpace；`Contain`
gutter 输入 fail closed。

## 2. 当前基线与触发证据

Tower Defense P1-0 fresh Gate G blocked run：

```text
G:\AifeTowerRuns\p1-0-gate-g-20260805-164431
classification = real_editor_gameview_input_bridge
code = authority.runtime_action_not_observed
AUI canvas = 1080x1920
Runtime target = 1280x720
```

真实 OS PointerDown/PointerUp 已进入 Editor，但 RuntimeInputFrame 的 TargetSpace point 被直接用于
ReferenceSpace AUI layout hit-test，导致 action miss。本文修复通用引擎 coordinate contract；Tower
只作为后续外部消费者，施工中不得写入塔防语义或修改塔防项目内容。

当前工作树包含 260-272 多个系统的未提交真实基线。激活与施工均禁止 reset、checkout、覆盖或把现有
diff 假定为 272 独占改动。

## 3. 前置槽位切换与证据继承约束

272 已完成 Gate A-F 的实现、受影响回归、production-feature 视觉矩阵与真实 Windows/WGPU 截图；
只剩 `authority_input.target_not_foreground` 环境闭环。用户已明确授权在 Gate 边界暂停，272 已保全
证据、恢复条件与失效范围并移入历史。

正式切换前必须：

1. 在先行施工文档写明让位原因、已通过证据、未完成项与恢复条件。
2. 将先行施工文档移入 `施工文档/历史/`，释放唯一施工槽。
3. 同步 `49`、`54`、`施工文档/README.md`、`待执行/README.md` 与优先级入口，确认 `当前/` 为空。
4. 对本文重新读取代码、diff、测试 filter 与 owner，复核通过后才移动到 `当前/`。

273 与 272 的直接重叠文件至少包括：

```text
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/input_route.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
```

272 已有 style resolver、GPU 视觉矩阵和截图在相关输入/host 文件未变化时可保留。273 修改上述文件后，
272 的真实 OS click closure、input consumer 与 authority 路径证据失效；恢复 272 时必须按最终 diff
重跑受影响 input/authority closure，不机械重跑全部 style/GPU 视觉矩阵。

## 4. Owner 与预计文件

### 4.1 Presentation owner

```text
rust/crates/engine_runtime/src/game_view_presentation.rs（新增，或激活复核确认的等价 owner）
rust/crates/engine_runtime/src/lib.rs
```

公共类型至少包括：

```text
GameViewPresentationSpec
GameViewTargetExtent
GameViewScalePolicy::{Contain, Stretch}
CanvasReferenceFact
ResolvedGameViewPresentation
PresentationIdentity / PresentationRevision
PresentationPointMapping / PresentationDiagnostic
```

公开 interface 提供行为型 forward/inverse mapping；consumer 不读取 scale/offset 后自行重建公式。

### 4.2 Runtime 与 AUI consumer

```text
rust/crates/engine_runtime/src/aui.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
rust/crates/engine_runtime/src/engine_host_loop.rs
```

### 4.3 Editor Play、surface 与 input consumer

```text
rust/crates/editor_core/src/editor_gameview_play.rs
rust/crates/editor_core/src/play_session.rs
rust/crates/editor_window_winit/src/viewport.rs
rust/crates/editor_window_winit/src/input_route.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/editor_frame_publication.rs
rust/crates/editor_window_winit/src/real_window.rs（只在共享 authority adapter 所需时）
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/editor_window_winit/src/tests/input_runtime_loop.rs
rust/crates/project_e2e_gate/src/second_project_runtime.rs
```

路径只是当前导航。激活复核若发现 ownership 已移动，先修订本文，不复制第二个 owner。

## 5. 授权与禁止项

当前状态：激活后可施工；授权范围仅限本文 Gate A-F。

未来激活后，既有用户授权只覆盖 273 范围内引擎源码、owner/affected tests 与 run-owned source-built
Editor targeted smoke；仍明确禁止：

```text
运行 Local CI
替换 production/安装态 Editor 或 Player 二进制
修改真实用户配置、系统 DPI 或 production state
修改 Tower 项目以绕过通用能力缺口
复用或改写旧 blocked run 伪装 fresh pass
未经新的 fresh 授权重跑 Tower P1-0 Gate G 或进入 Gate H
增加 authority-only 坐标公式、直接 RuntimeInputFrame/action 注入
扩入 BuildProfile、exported Player window policy、设备模拟器或动态分辨率
```

## 6. 激活前复核

1. 完整读取 273 方案、本文、`49`、`54`、当前/待执行目录和先行施工让位记录。
2. 确认用户已明确批准切换施工槽并激活/施工 273，且 `当前/` 除 README 外为空。
3. 记录 `git status --short` 与预计文件 diff，逐项保留现有用户改动。
4. 复核 RuntimeInputFrame、AuiInteractionSystem、RuntimeRenderer、stable surface/publication、
   ViewportHost、Editor Play options 与 production authority 当前 owner。
5. 确认 package 与测试 filter 非零匹配；必要时按当前 owner suite 修订每 Gate 命令。
6. 明确先行施工的哪些证据继续有效、哪些因 273 触碰文件而失效，并写入让位记录。
7. 重新完成第 14 节自审，将本文移动到 `当前/273-当前可自动化施工文档-...md` 并同步 54。

2026-08-05 复核结果：上述 1-7 已完成。源码确认不存在既有等价 presentation owner；当前分裂点为
`EngineHostLoop` hardcoded target、RuntimeRenderer draw-bounds reference extent、ViewportHost 独立比例、
AUI TargetSpace 直接 hit-test 与 authority 手算。工作树高度脏但 owner 路径与 Gate 命令仍有效；施工将
保留全部现有改动，不执行 reset/checkout。

## 7. Gate A：Typed spec、纯变换 Module 与 owner tests

### 实现

1. 新增唯一 `GameViewPresentationModule`，校验 target/display/canvas facts 并返回 resolved value。
2. 实现 Reference↔Target、Target↔Display、Reference↔Display 的成对变换。
3. `Contain` 计算居中 content rect 与 gutters；边界统一为 left/top inclusive、right/bottom exclusive。
4. 支持 per-canvas reference extent；unknown/conflicting canvas fail closed。
5. identity 绑定 session/target、extent、policy、surface generation、canvas digest 与 revision。
6. Off 不生成字符串 trace；Summary/Trace 使用 typed diagnostics。

### Gate 测试

```powershell
cargo test --manifest-path rust/Cargo.toml -p engine_runtime game_view_presentation -- --nocapture
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

必须覆盖 identity、两种竖屏、非等比 contain、stretch、gutter、边界、round-trip、multiple canvas、
invalid/non-finite/capability exceeded 与 stale revision。

## 8. Gate B：AUI reference facts 与 Renderer projection

### 实现

1. AUI presenter 显式输出 ordered `canvasId + referenceExtent` facts。
2. RuntimeRenderer 按 canvas transform 投影 geometry、glyph、clip；删除 draw-item bounds 推断 extent。
3. selected target extent 进入 RenderTarget/RenderGraph/texture descriptor。
4. contain gutters 确定性 clear；旧 `1280x720 + Stretch` caller 通过 typed legacy spec 迁移。
5. 禁止将一个全局 canvas 比例套用到多个 reference extent。

### Gate 测试

```powershell
cargo test --manifest-path rust/Cargo.toml -p engine_runtime runtime_renderer -- --nocapture
cargo test --manifest-path rust/Cargo.toml -p engine_runtime aui -- --nocapture
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

## 9. Gate C：Editor Play target 与 stable surface/publication

### 实现

1. Editor Play/session options 显式携带 target extent 与 scale policy。
2. `1080x1920`、`720x1280` 真正创建对应 backing，不允许报告尺寸与 texture 尺寸分裂。
3. target change 复用 265 backing replacement/generation；display-only resize 不推进 surface generation。
4. receipt/publication 绑定 target extent、generation 与 presentation revision；旧 receipt fail closed。
5. target change 清理 hover/pressed/capture；第一帧新尺寸成功发布后才暴露 ready。

### Gate 测试

```powershell
cargo test --manifest-path rust/Cargo.toml -p editor_core editor_gameview_play -- --nocapture
cargo test --manifest-path rust/Cargo.toml -p editor_window_winit editor_frame_publication -- --nocapture
cargo test --manifest-path rust/Cargo.toml -p editor_window_winit game_view -- --nocapture
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

## 10. Gate D：Display content rect、input inverse 与 per-canvas hit-test

### 实现

1. ViewportHost 注册实际 display content rect，不把整个 panel rect 当 texture content。
2. OS logical pointer 先做 display→target；gutter/outside 不生成 RuntimeInputFrame。
3. AUI candidate 按 canvas 做 Target→Reference；gameplay fallback 保留原 TargetSpace event。
4. consumed event index 维持 exactly-once；AUI consumed 不泄漏 gameplay。
5. display/target/canvas revision 变化取消 capture，禁止 down/up 跨 revision 合成 action。
6. keyboard/text/IME 路径不进行坐标缩放。

### Gate 测试

```powershell
cargo test --manifest-path rust/Cargo.toml -p editor_window_winit input_runtime_loop -- --nocapture
cargo test --manifest-path rust/Cargo.toml -p editor_window_winit native_app -- --nocapture
cargo test --manifest-path rust/Cargo.toml -p engine_runtime aui_interaction -- --nocapture
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

## 11. Gate E：Authority 共用 mapping、第二项目与真实窗口 targeted smoke

### 实现与验证

1. authority 从 actionable ReferenceSpace rect 调用共享 forward mapping 得到 display/OS point。
2. report 记录 presentation identity/revision、canvas、reference/target/display point 与 rejection code；
   不读取或注入项目 action payload。
3. round-trip 必须通过普通 input route 返回同一 AUI node。
4. 使用 Switch Puzzle 或现有 second-project fixture 证明没有 Tower/project/action 特判。
5. 仅使用 source-built、run-owned 测试窗口做 targeted real-window smoke；前台权限失败分类为 environment，
   不降级为 PostMessage、直接 action 或 synthetic runtime frame。

### Gate 测试

```powershell
cargo test --manifest-path rust/Cargo.toml -p project_e2e_gate second_project -- --nocapture
cargo test --manifest-path rust/Cargo.toml -p editor_window_winit --features real-window production_authority -- --nocapture
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

真实窗口命令须在激活时根据现有 authority binary/options 写回本文；未确认 CLI 前不得猜参数执行。

已确认的 source-built 项目专用 Editor authority 入口：

```powershell
<run-owned-project-editor-composition.exe> --run-production-authority-scenario <run-owned-scenario.json>
```

该入口由 `ProjectEditorCompositionArtifact` 生成并显式链接项目 RuntimeModule；273 targeted smoke 使用
Switch Puzzle 的 run-owned 项目副本，不使用通用 `editor_ui_authority` 冒充项目组合，也不触碰 Tower Gate G。

## 12. Gate F：受影响回归、视觉矩阵与文档闭环

### 回归

```powershell
cargo test --manifest-path rust/Cargo.toml -p engine_runtime
cargo test --manifest-path rust/Cargo.toml -p editor_core
cargo test --manifest-path rust/Cargo.toml -p editor_window_winit
cargo test --manifest-path rust/Cargo.toml -p editor_window_winit --features real-window
cargo test --manifest-path rust/Cargo.toml -p project_e2e_gate
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

若 workspace 当前 suite 规模使整 crate 命令超出施工窗口，必须在激活时写明分片且覆盖所有受影响
owner/consumer；不得以单一 happy-path filter 代替 affected closure。

source-built visual matrix 至少覆盖：

```text
1280x720 legacy landscape
1080x1920 portrait target
720x1280 portrait target
landscape Editor panel 内 portrait contain + gutters
AUI geometry/glyph/clip 与 input round-trip
resize 后 generation/revision/capture behavior
```

完成后生成阶段完成记录、同步 `00`/`49`/`54`/`240`/施工入口并将本文归档。Gate F 不包含
Local CI、production replacement 或 Tower fresh Gate G。

## 13. 停止条件

任一项发生时停止当前 Gate，保全结构化证据，不跨 Gate 猜修：

```text
需要修改 production/安装态二进制或真实用户配置
需要修改 Tower 项目语义才能通过
发现第二个 presentation/coordinate owner
Renderer 与 input 无法消费同一 resolved identity/revision
旧稳定 surface generation 合同被破坏
测试 filter 零匹配或真实 owner 已漂移
先行施工未正式让位或当前槽不唯一
```

## 14. 文档自审

```text
正式方案一致性：通过，完整实现方案 C
深 Module：通过，正反变换、policy、identity、revision、diagnostics 收于唯一 owner
项目无关性：通过，无 Tower/node/action/projectId 语义
Gate 可验证性：通过，每 Gate 有实现边界与测试命令
现有 owner 保持：通过，AUI/Renderer/surface/input/session 生命周期不被夺取
前置冲突控制：通过，明确槽位转换、重叠文件和证据失效范围
施工时资格：是，用户已授权，先行施工已让位，激活复核通过
Local CI：禁止
production/安装态替换：禁止
Tower fresh Gate G：不包含，需施工完成后的独立 fresh 授权
```

自审结论：激活复核通过；本文移动到 `当前/` 并同步入口后可执行 Gate A-F。

## 15. 2026-08-05 施工进度与保全证据

最终结论：Gate A-F、owner/affected tests、source-built targeted real-window matrix 与受影响全量回归
均已通过。先前 G 盘容量/PDB 阻塞在释放磁盘空间后消失；剩余四个失败均确认为过期或非确定性测试
断言，并以 test-only 最小修复闭环。273 已生成阶段完成记录并归档。

新增源码能力：

```text
GameViewPresentation typed owner 与 Reference/Target/Display 正反变换
per-canvas AUI render/input projection
Editor Play session-scoped target extent + scale policy
Viewport display content rect、gutter reject、revision crossing reject
ProductionAuthorityScenario.gameViewTarget
GameViewAuiCoordinateEvidence：identity/revision/canvas/reference/target/display typed evidence
```

最新 owner/affected 结果：

```text
editor_core toolbar_play_uses_the_session_game_view_target：1 passed
editor_window_winit production_authority_scenario_accepts_single_session_flow：1 passed
editor_window_winit real-window production_authority_scenario_loads_declared_recent_project_store：1 passed
editor_window_winit default full：178 passed / 1 ignored
editor_window_winit real-window full：189 passed / 8 ignored
cargo fmt --all -- --check：passed
git diff --check（273 scope）：passed
```

source-built composition 与真实矩阵：

```text
run root：C:/Users/zenghaoran/AppData/Local/Temp/aife-273-switch-composition-20260805-r2
editor build identity：sha256:550c6a87c63c2d6f3821865760835dc4c0a87a5e2ba9a38ed88ffeca364a1f03
composition identity：sha256:16628be5599374e6d5256ee65715e3ecc1488d274c0a4fabbd3a8e486a027e59
binary sha256：sha256:765839088f508be1c4df5e94493945d33bb608aa8984f42699ee40edc92118a6
project：run-owned Switch Puzzle copy；headless lifecycle qualification passed

1280x720 + Stretch：passed；OS down/up；ui.toggle-switch；typed coordinates；RGBA capture
1080x1920 + Contain：passed；portrait content rect + gutters；OS down/up；typed coordinates；RGBA capture
720x1280 + Contain：passed；portrait content rect + gutters；OS down/up；typed coordinates；RGBA capture

matrix evidence root：
C:/Users/zenghaoran/AppData/Local/Temp/aife-273-visual-matrix-20260805/evidence
```

首个 legacy smoke 因 scenario 使用了底层 hit-region 派生 ID `editor/control/hit.toolbar.play` 而在 Play
actionability 等待 30 秒后失败；项目打开与 AuthoringWorkspace 已通过。读取正式 composition qualification
后改用稳定 retained widget `editor/shell/toolbar/play`，三个 fresh scenario 均通过。该失败未修改 production、
安装态、真实配置或 Tower 项目。

Gate F 最终结果：

```text
editor_core first rerun：632 passed / 2 failed / 2 ignored
- composition cache identity test 对已为 a 的 digest nibble 替换为 a，变体可能为空操作
- project launcher test 将 canonical long path 与调用方 8.3 short path 做逐字比较
两项均以 test-only 断言修复；owner reproduction 通过
editor_core final full：634 passed / 0 failed / 2 ignored

project_e2e_gate first full：64 passed / 2 failed / 2 ignored
- 两项旧测试仍要求 271 前 legacy font adapter / ui-default-cmin
- 271 正式合同已要求无项目 DefaultUi 时注入 aife-default-zh-cn-common-v1
两项均更新为现行 FontPack 合同；owner reproduction 通过
project_e2e_gate final full：lib 66 passed / 0 failed / 2 ignored；其余 integration 6 passed
cargo fmt --all -- --check：passed
git diff --check（273 scope + test-only closure）：passed

Local CI：按授权禁止，未运行
production/安装态 replacement：按授权禁止，未执行
Tower fresh Gate G/H：不在 273 授权内，未运行
```
