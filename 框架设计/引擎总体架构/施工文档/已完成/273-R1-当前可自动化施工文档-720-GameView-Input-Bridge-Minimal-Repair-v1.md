# 273-R1 720 GameView Input Bridge Minimal Repair v1 施工文档

## 1. 状态与授权

```text
上游方案：273-GameView-Presentation-Target-AUI-Input-Coordinate-Consistency-v1方案.md
文档状态：已完成，待归档
授权：用户明确授权引擎侧 720 GameView input bridge 最小诊断与修复
触发证据：G:\AifeTowerRuns\p1-0-gate-g-20260806-004658-273-272-r7\evidence\gate-g-blocked-report.json
当前阻断：720 GameView AUI node actionable、OS down/up 与坐标证据完整，但 runtime action 未观察到
```

禁止：Tower gameplay 特判、直接注入 `td.recruit`、修改 Tower 项目语义、Local CI、production/安装态
二进制替换、真实配置修改、Tower fresh Gate G 或 Gate H。

## 2. 范围与所有权

- `rust/crates/editor_window_winit/**`：engine-owned，允许最小修复真实 Editor display/input route。
- `rust/crates/engine_runtime/src/game_view_presentation.rs`：engine-owned；仅当 owner red test 证明通用映射错误时修改。
- `rust/crates/engine_runtime/src/aui.rs`：engine-owned；仅当 per-canvas Target→Reference consumer red test 证明错误时修改。
- 273 方案、施工入口、完成记录：只同步 R1 结论。
- `samples/tower_defense_project/**`：外部项目，只读；只引用既有失败证据。

## 3. Gate A：秒级红测与最小诊断

在真实 call-site seam 冻结 R7 几何：physical `720x1280`、display content rect
`(116.99999,224.83333,237.00002,421.33334)`、target `720x1280`、reference `1080x1920`、
reference point `(451,1779)`。测试必须证明 Editor input route 产生正确 TargetSpace pointer，并且不会被
Editor hit region 误消费；运行指定 filter，先观察红测。

根据红测生成 3-5 个可证伪假设，只测试能区分 owner 的最小变量。不得用 fresh Gate G 作诊断循环。

## 4. Gate B：最小修复与 owner/consumer 验证

只修改红测命中的 owner；保持 273 的 typed presentation、gutter reject、revision/capture 与 exactly-once
合同。先使新增 red test 转绿，再运行：

```powershell
cargo test --manifest-path rust/Cargo.toml -p editor_window_winit input_runtime_loop -- --nocapture
cargo test --manifest-path rust/Cargo.toml -p editor_window_winit native_app -- --nocapture
cargo test --manifest-path rust/Cargo.toml -p engine_runtime aui_interaction -- --nocapture
```

若修改共享 presentation owner，再运行其完整 owner filter 与直接 consumer；不机械运行完整 workspace。

## 5. Gate C：source-built targeted smoke 与收口

仅在 owner/consumer 全绿后，使用 repository-external、run-owned source-built 最小项目 fixture 验证
`720x1280 + Contain` 真实 OS pointer down/up 与 AUI action。不得替换安装态二进制或读取真实配置。

通过后运行 `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` 与 scoped `git diff --check`，更新
273 方案/完成记录/入口，生成 R1 完成记录并归档本文。Tower fresh Gate G 仍需单独授权。

## 6. 自审

```text
既有方案一致性：是，修复 273 已声明的 720 正反 mapping/input 一致性，不新增系统
最小复现：必须先红后绿
项目无关性：禁止 Tower node/action 特判
验证范围：owner + direct consumer + run-owned targeted smoke
外部状态：不触碰 production/安装态、真实配置、Tower Gate G/H
施工时资格：用户已授权；引擎当前槽原为空，本文为唯一当前施工文档
```

## 7. 执行结果

Gate A 以真实 source-built 720 scenario 稳定复现 `UiConsumed / ui_hit / PointerUp`。加强后的
toolbar overflow owner test 在 GameView viewport 覆盖条件下先红，证明 barrier PointerUp 被
GameView gateway 提前截断。Gate B 最小修复只调整 `NativeEditorApplication` 的下落控制：保留
`UiConsumed` evidence，但只有 `RuntimeInputFrame` 视为 GameView 已实际消费。owner 与 direct
consumer filters 分别为 `18/18`、`55/55`、`6/6` passed。

Gate C run root：

```text
G:\AifeTowerRuns\273-r1-720-input-repair-smoke-20260806-091647
editor build identity：sha256:caa8adc59764debc2f0bb1da23147441099fe267bfef411d6674b868e39c89c0
composition identity：sha256:98496340fd0064882629946d85a2d683eb23da8f789ccb91b2a1c38547d016f3
binary sha256：sha256:85ee5b75a1257e8e190fc97f29caa0a565b43bc48e6be563eaf3a463c1ae7b5d
authority：10/10 steps passed；recruit runtimeActionId=td.recruit
```

未运行 Local CI，未替换 production/安装态二进制，未修改真实配置或 Tower 项目；Tower fresh
Gate G 与 Gate H 均未执行。
