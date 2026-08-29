# 231-Exported Windows Playable Golden Gate v1 方案

> 状态：正式方案，用户已确认采用 `B-min: Packaged Player Golden Gate + Optional Real Window Evidence`。  
> 校准日期：2026-07-09。  
> 所属路线：`227` 的 P0-4。  
> 前置：`228` 真实贴图、`229` 复杂打飞机真实玩法规则、`230` 真实 ProjectUiStateSnapshot Producer 已完成。  
> 本文只生成方案，不允许直接施工；施工前仍需审查/自审和施工文档。

## 0. 用户确认结论

本系统确认采用：

```text
B-min: Packaged Player Golden Gate + Optional Real Window Evidence
```

含义：

```text
Packaged Player Golden Gate:
  默认用真实导出的 Game.exe 作为验收对象，
  而不是只在 editor in-process 或 runtime crate 单测里验证。

Headless deterministic blocking:
  默认阻塞 gate 使用 headless-gate / deterministic player run，
  校验 RuntimePackage、真实贴图、真实玩法、真实 HUD、AUI glyph/render evidence。

Optional Real Window Evidence:
  真实 OS window / GPU screenshot 作为 local-only 或 feature-gated 证据。
  支持时记录 screenshot/pixel evidence；
  不支持时必须明确 feature-disabled / environment-blocked，
  不允许伪装成通过，也不作为默认 CI 阻塞项。
```

核心红线：

```text
P0-4 是最终验收系统，不是新 gameplay / AUI / renderer 功能系统。
不得把复杂打飞机玩法写入 engine core。
不得绕过 RuntimePackage 真相直接读取项目源目录或 EditorSession 内存。
不得用 debug overlay、sample producer、frame_index 分数或假 report 伪装可玩。
真实窗口/GPU 截图默认不阻塞 headless CI，但必须在 report 中诚实分档。
默认 blocking gate 必须使用导出进程契约；不得只用 in-process runtime 测试替代导出 Game.exe 验收。
```

## 1. 一句话说明

这个系统把复杂打飞机项目从：

```text
多个子系统各自能通过测试
```

推进到：

```text
导出的 Windows Game.exe 能被自动启动，
并用同一份 golden report 证明它真的可见、可运行、可玩、HUD 真实。
```

它在本引擎里的位置：

```text
Sample Project / Authoring Assets
  -> DesktopExportPipeline
  -> RuntimePackage
  -> staged Windows package / Game.exe
  -> exported Game.exe run-native-player --mode headless-gate
  -> child windowed-player-run-report.json
  -> ExportedWindowsPlayableGoldenGateReport
  -> Report Panel / project_e2e_gate evidence
```

它不是新的编辑器功能，也不是新的运行时模式；它是 P0 可玩闭环的最终强验收。

## 2. 为什么现在做它

`227` 的 P0 顺序是：

```text
P0-1 Real Texture Decode / GPU Texture Upload / Sprite Textured Present v1
P0-2 Complex Shooter Gameplay Rule Runtime Execution v1
P0-3 Project Rule Driven UiStateSnapshot Producer v1
P0-4 Exported Windows Playable Golden Gate v1
```

当前状态：

```text
228 已完成：
  PNG texture asset -> cooked texture payload -> RuntimePackage load
  -> Sprite2D real texture binding -> RealWgpuBackend texture upload/report。

229 已完成：
  RuntimePackage.rules -> ProjectLogicRunner -> EngineHostLoop/FrameLoop
  -> movement/fire/spawn/despawn/collision/score 真实运行。

230 已完成：
  HUD 的 score/hp/wave/enemy_count 来自真实 runtime World / Project Rule state，
  并按 active binding paths + dirty/cached 生成 ProjectUiStateSnapshot report。

历史导出 gate 已完成：
  Exported Windows Player Process Verification Gate v1 已能启动导出的 Game.exe。
  Exported Windows Real Window / Screenshot Verification Gate v1 已有 feature-gated 截图通道。
```

剩余缺口：

```text
已有证据分散在 228 / 229 / 230 / 168 / 170 / project_e2e_gate 中。
还没有一个 P0-4 golden gate 汇总证明：
  导出的 Game.exe 本身
  在 Windows package 目录下
  通过真实 RuntimePackage
  同时满足真实贴图、真实玩法、真实 HUD、AUI glyph/render、可选真实窗口证据。
```

所以 P0-4 不重新做贴图、规则或 UI producer，而是把它们汇合成一条最终验收链路。

## 3. 其它引擎对标

### Unity

对标：

```text
BuildPipeline.BuildPlayer
BuildReport
Unity Test Framework PlayMode tests in Player
Player log / test result artifact
```

可学习点：

```text
Build 成功不是最终答案，BuildReport 要暴露步骤、结果和错误。
Player 侧测试比 Editor 侧测试更接近真实发布产物。
构建产物、Player 启动、测试结果、日志/报告应形成一套可审查证据。
```

不照搬点：

```text
不引入 Unity 式 MonoBehaviour / Scene Object 运行时模型。
不让 AUI Binding 直接读取 ECS 或项目逻辑。
不让 Editor Play 直接等价于最终 Windows package gate。
```

参考：

```text
Unity BuildPipeline.BuildPlayer:
  https://docs.unity3d.com/ScriptReference/BuildPipeline.BuildPlayer.html
Unity BuildReport:
  https://docs.unity3d.com/ScriptReference/Build.Reporting.BuildReport.html
Unity Test Framework Player PlayMode tests:
  https://docs.unity3d.com/Packages/com.unity.test-framework@1.1/manual/workflow-run-playmode-test-standalone.html
UnityCsReference BuildPlayerWindow / BuildPlayerWindowBuildMethods:
  https://github.com/Unity-Technologies/UnityCsReference
```

### Unreal Engine

对标：

```text
AutomationTool
Gauntlet
packaged game launch / session validation / artifact collection
```

可学习点：

```text
验收对象应是 packaged game process，而不是只测编辑器内状态。
测试应能启动进程、驱动场景、采集 log/report/screenshot，并给出 pass/fail。
环境不满足时应分类为 skipped / environment blocked，而不是假通过。
```

不照搬点：

```text
不引入 UE 式 UObject / UMG / Blueprint runtime ownership。
不把 P0-4 做成庞大的自动化测试框架。
v1 只收敛复杂打飞机 Windows playable golden，不扩展成通用平台矩阵。
```

参考：

```text
Unreal Engine Gauntlet Automation Framework:
  https://dev.epicgames.com/documentation/unreal-engine/gauntlet-automation-framework-in-unreal-engine
Unreal Automation Tool / packaged test workflow:
  以官方文档和 Engine/Source/Programs/AutomationTool/Gauntlet 源码结构为参考，不照搬其大型框架。
```

## 4. 本项目当前代码基线

当前可复用入口：

```text
editor_core/src/desktop_export.rs
  DesktopExportPipeline::export
  已构建 RuntimePackage、stage Game.exe、写 desktop-export-report.json，
  并可通过 WindowedPlayerHost::run_headless_gate 跑 package 内 player gate。

runtime_cli/src/exported_player_verification.rs
  verify_exported_player_process
  已检查 exported package / Game.exe / package-manifest.json / data/runtime_package，
  并启动导出的 Game.exe run-native-player。

runtime_player_winit/src/lib.rs
  NativePlayerWindowRunRequest::headless_surface_gate
  NativePlayerWindowRunRequest::windowed
  run_headless_native_player_from_package
  run_windowed_native_player_from_package
  已有 AUI summary、screenshot report、real-window feature-gated smoke。

project_e2e_gate/src/gate.rs
  现有 complex shooter e2e 已串联 sample project -> DesktopExportPipeline
  -> RuntimePackage load -> headless player run，
  但报告还不是 P0-4 golden 汇总。
```

当前不足：

```text
没有 ExportedWindowsPlayableGoldenGateReport schema。
没有一个 gate 同时强断言 228 / 229 / 230 的关键证据。
导出 Game.exe 进程验证和 project_e2e_gate 证据还相对分散。
真实窗口截图仍是 optional/local-only，没有纳入 P0-4 summary 分档。
Report Panel 侧尚未有 P0-4 golden provider / evidence entry。
```

## 5. 备选方案

### 方案 A：Headless Packaged Golden Only

```text
导出 Game.exe 后，只跑 headless-gate。
强校验 package、manifest、frames、玩法、HUD、贴图、AUI glyph。
不处理真实 OS window / screenshot。
```

优点：

```text
CI 稳定。
实现最小。
不受 GPU / window environment 影响。
```

缺点：

```text
不能证明真实 OS window / screenshot 通道可用。
“Windows 可玩”证据少一层像素可信度。
```

### 方案 B：Hybrid Golden，默认 headless，真实窗口可选

```text
默认阻塞 gate：
  导出 Game.exe -> headless deterministic player -> P0 汇总断言。

可选证据：
  real-window feature / local GPU 环境可用时，
  追加 windowed screenshot smoke 和 screenshot summary。

不满足真实窗口条件：
  明确 report 为 feature-disabled / environment-blocked / skipped-local-only。
```

优点：

```text
符合项目 headless-first 规则。
能证明导出 exe 真能作为独立进程运行。
能汇总真实贴图、真实玩法、真实 HUD。
不让 CI 被窗口/GPU 环境拖垮。
保留长期真实窗口 golden 的接口。
```

缺点：

```text
v1 默认 blocking pass 仍不是强制真实 OS window pixel gate。
需要设计好 report 分档，避免 optional 被误读成核心通过条件。
```

### 方案 C：Full Real Window Mandatory

```text
导出后必须启动真实窗口、真实 GPU present、截图并做 pixel/golden 检查。
没有真实窗口环境则失败。
```

优点：

```text
可玩证明最强。
最接近最终用户实际运行。
```

缺点：

```text
CI / headless 环境不稳定。
GPU、驱动、窗口系统、截图差异会引入大量非玩法问题。
容易把 P0-4 变成平台环境治理，而不是复杂打飞机可玩闭环。
```

## 6. 选定方案：B-min

最终采用：

```text
B-min: Packaged Player Golden Gate + Optional Real Window Evidence
```

目标链路：

```text
Complex Shooter Sample Project
  -> DesktopExportPipeline::export
  -> Build/Windows/dev/Game.exe
  -> verify_exported_player_process
  -> Game.exe run-native-player --mode headless-gate --frames N
  -> child windowed-player-run-report.json
  -> ExportedWindowsPlayableGoldenGateReport
```

约束：

```text
v1 首选复用 runtime_cli::verify_exported_player_process。
如果施工时为了 project_e2e_gate 依赖边界选择直接 spawn exported Game.exe，
也必须遵守同一份 exported process contract：
  校验 package manifest / runtime_package / executable。
  从 exported package 目录启动 child process。
  child 通过 run-native-player 写 windowed-player-run-report.json。
  parent 读取 child report 并写 P0-4 golden report。
禁止只调用 run_headless_native_player_from_package 这类 in-process helper 后宣称 exported Game.exe 已验收。
```

阻塞断言：

```text
export:
  status == success
  package-manifest.json exists
  data/runtime_package/manifest.json exists
  Windows target 下 Game.exe exists
  非 Windows 自动化环境允许 platform-equivalent executable fixture，
  但 report 必须记录 target_os / actual_host_os / executable_name

process:
  exported Game.exe process spawn succeeds
  child exit_code == 0
  child report exists and parses
  frames_completed >= requested_frames

texture/render:
  real texture payload assembled/loaded/upload-ready evidence exists
  sprite texture binding fallback_count == 0 for complex shooter core sprites
  render/rhi command count > 0

gameplay:
  input script or deterministic action source is reported
  fire action or equivalent projectile spawn evidence exists
  movement/fire/collision/score evidence exists
  score changes through Project Rule runtime execution
  runtime spawn/despawn evidence exists when scenario requires it

HUD/AUI:
  AUI package document count > 0
  AUI draw item count > 0
  rendered_glyph_count > 0
  snapshot_source == project_producer
  producer_id == complex_shooter_runtime_ui_state
  HUD score_text matches runtime score_after
  hp/wave/enemy_count binding paths are produced or explicitly diagnosed
```

可选断言：

```text
real-window:
  if feature/environment available:
    run windowed screenshot smoke
    screenshot_status == captured
    screenshot path exists
    screenshot byte_size > 0
  else:
    real_window_status == feature_disabled | environment_blocked | local_only_skipped
    optional_real_window_blocking == false
```

## 7. Report 设计

新增或汇总报告：

```text
ExportedWindowsPlayableGoldenGateReport
schemaVersion: exported-windows-playable-golden-gate-report.v1
```

核心字段：

```text
status:
  passed | failed | partial

status 规则：
  所有 blocking evidence 通过时 status = passed。
  optional real-window skipped / feature_disabled / environment_blocked
  不得把 top-level status 降级为 partial。
  partial 只用于 blocking evidence 可定位但尚未满足、且施工文档明确允许的过渡状态；
  P0-4 施工完成时不应以 partial 作为最终完成状态。

package:
  exported_package_dir
  game_exe_path
  package_manifest_path
  runtime_package_path
  desktop_export_report_path

process:
  mode
  requested_frames
  child_exit_code
  child_present_status
  child_frames_completed
  child_report_path

goldenEvidence:
  texture_status
  gameplay_status
  hud_status
  aui_status
  render_status

textureEvidence:
  loaded_texture_count
  uploaded_texture_count
  sprite_texture_binding_ready
  fallback_count

gameplayEvidence:
  input_source
  input_script_id
  fire_action_observed
  projectile_spawn_observed
  movement_observed
  fire_observed
  collision_pair_count
  score_before
  score_after
  score_changed

hudEvidence:
  snapshot_source
  producer_id
  active_binding_paths
  produced_paths
  missing_paths
  score_text
  score_text_matches_score_after
  rendered_glyph_count

realWindowEvidence:
  status
  blocking
  screenshot_requested
  screenshot_status
  screenshot_path
  environment_diagnostic

reportMode:
  runtime_report_level: off | summary | trace
  editor_report_level: off | summary | trace

artifacts:
  report id/path pairs

diagnostics:
  severity
  code
  domain
  stage
  source_path
  message
  next_action
```

分档规则：

```text
runtime default:
  Off 或 compact Summary，不在 runtime hot path 常驻写重 JSON。

gate/debug:
  Summary 必须足够让 AI 判断 P0-4 为什么失败。
  Trace 仅用于测试、gate、debug 或用户显式诊断。

editor/report panel:
  Report Panel 只消费 Summary / Trace artifact，
  不把 runtime 热路径变成常驻 report 系统。
```

## 8. AI 调试路径

P0-4 report 必须让 AI 能按以下路径定位问题：

```text
Game.exe missing:
  看 package.game_exe_path / desktop export diagnostics。

RuntimePackage missing:
  看 runtime_package_path / manifest diagnostics。

进程启动失败:
  看 process spawn error / stdout_summary / stderr_summary。

能启动但没画面:
  看 render_status / rhi command count / present_status。

贴图不真实:
  看 textureEvidence fallback_count / loaded/uploaded count / source path。

玩法没跑:
  看 gameplayEvidence input_source / fire_action_observed / projectile_spawn_observed /
  movement/fire/collision/score。

分数变了但 HUD 不变:
  看 hudEvidence snapshot_source / produced_paths / score_text_matches_score_after。

HUD 有值但不可见:
  看 rendered_glyph_count / AUI draw item count / UI composition status。

真实窗口没截图:
  看 realWindowEvidence.status。
  如果是 feature_disabled 或 environment_blocked，不算默认 blocking failure。
```

## 9. 与已有系统关系

### 与 168/170 的关系

```text
168 已证明导出 Game.exe 进程级验证通道存在。
170 已证明真实窗口截图通道存在。
231 不重复实现这两个底座，而是把它们纳入 P0-4 golden summary。
```

### 与 228 的关系

```text
228 负责真实贴图 cook/load/upload/present。
231 只读取或汇总其 evidence，验证导出包里不再靠 fallback 色块伪装。
如果 child player report 缺少 228 所需字段，施工只能补轻量 texture summary，
不得重做一条平行 texture validation pipeline。
```

### 与 229 的关系

```text
229 负责复杂打飞机玩法规则 runtime execution。
231 只验证导出的 Game.exe 中 input/fire/projectile/movement/collision/score 仍真实发生。
如果复用 229 project_e2e 子报告，必须在 P0-4 report 中标明 source_report_path / source_domain。
```

### 与 230 的关系

```text
230 负责 HUD state 来自真实 Project Rule / ECS runtime state。
231 只验证导出 Game.exe 的 HUD score_text 与 runtime score_after 一致。
如果复用 230 project_e2e 子报告，必须在 P0-4 report 中标明 source_report_path / source_domain。
```

### 与后续 P1/P2 的关系

```text
P0-4 完成后，才能更有信心讨论：
  Editor Build And Run Productization v1
  Rule Graph / Card Authoring Productization v1
  Input Mapping Visual Authoring Panel v1
  Save / Reload / Rebuild Consistency Gate v1
  Release Package Polish / Metadata / Icon / Layout v1
```

## 10. 非目标

本轮不做：

```text
完整 installer / signing / store package。
强制真实 OS window pixel golden。
跨平台发布矩阵。
完整 GPU pixel diff baseline。
大型 Gauntlet/AutomationTool 克隆。
新的 gameplay feature。
新的 AUI 控件。
新的 RuntimePackage schema 大改。
真实 LLM provider / repair loop。
```

## 11. 预期施工边界

施工应优先新增或扩展：

```text
project_e2e_gate:
  新增 exported_windows_playable_golden gate/report/test。

runtime_cli:
  复用或轻微扩展 verify_exported_player_process，
  让 parent report 可暴露 P0-4 所需 child summary。

runtime_player_winit:
  不重写 player host。
  如 child report 缺少必要 summary，只补轻量 summary 字段。
  补字段必须服务 P0-4 summary，不把 runtime hot path 变成常驻 trace/report。

editor_core / report panel:
  如需要，只注册 P0-4 golden report provider/evidence entry。
```

施工不应：

```text
重写 DesktopExportPipeline。
让 Runtime 扫描项目源目录。
为复杂打飞机新增 engine core 专用 API。
把 optional real-window 失败当成默认 blocking failure。
```

## 12. 预期 Gate

施工文档应至少拆成：

```text
Gate A: Report schema / fixture
  建立 ExportedWindowsPlayableGoldenGateReport，
  覆盖 pass/fail/optional real-window skipped 序列化测试。

Gate B: Exported package + process verification 汇总
  复用 DesktopExportPipeline 和 verify_exported_player_process，
  证明导出 Game.exe 被真实启动。

Gate C: P0 evidence 汇总断言
  读取或汇总 228/229/230 的 texture/gameplay/HUD 关键证据。
  每条 evidence 必须标明来自 exported child report、existing project_e2e subreport、
  还是新补的 lightweight child summary。

Gate D: Optional real-window evidence 分档
  支持 captured / feature_disabled / environment_blocked / local_only_skipped，
  并证明默认不阻塞 headless gate。

Gate E: Report Panel / AI context 接入
  让统一 Report Panel 或 project_e2e_gate artifact 能看到 P0-4 summary。

Gate F: 整体回归
  跑 project_e2e_gate、runtime_cli、runtime_player_winit、editor_core 相关测试。
```

## 13. 自审

```text
是否符合 227 P0 顺序：
  是。P0-1/P0-2/P0-3 已完成，当前进入 P0-4。

是否避免新增复杂层：
  是。P0-4 是汇总 gate/report，不新增 runtime ownership 层。

是否符合 RuntimePackage 真相：
  是。验收对象是导出的 Game.exe 和 data/runtime_package。

是否符合 headless-first：
  是。默认阻塞 gate 是 deterministic headless packaged player；
  real-window/screenshot 是 optional/local-only evidence。

是否服务 AI 适配性：
  是。报告按 package/process/texture/gameplay/HUD/real-window 分域，
  并提供 domain/stage/source_path/next_action。

是否支持复杂项目维护：
  是。后续自走棋或更复杂项目可复用同类 golden report，
  只替换项目侧 evidence provider，不改 engine core。

是否控制效率：
  是。默认不强制真实窗口/GPU 截图，不在 runtime hot path 常驻重 trace。
```

## 13.1 方案自审记录（2026-07-09）

结论：

```text
方案方向正确，可以进入后续外部审查或施工文档生成。
本次自审不推翻 B-min，只补充 4 条施工约束，避免施工阶段走偏。
```

已发现并修正：

```text
SR-1 exported process contract 收紧：
  原文允许 "or equivalent exported process run"，措辞偏松。
  已改为首选 verify_exported_player_process；
  如 project_e2e_gate 直接 spawn，也必须遵守同一 exported process contract，
  不能用 in-process helper 替代导出 Game.exe 验收。

SR-2 Windows 目标与自动化宿主区分：
  P0-4 名义目标是 Windows Game.exe。
  但当前测试可能在非 Windows host 上使用 Game 等价 fixture。
  已要求 report 写 target_os / actual_host_os / executable_name，
  避免 AI 把非 Windows fixture 误读为真实 Windows release。

SR-3 输入证据补强：
  "可玩" 不能只看碰撞/计分，还要说明 fire/projectile 来自什么输入或确定性脚本。
  已增加 input_source / input_script_id / fire_action_observed / projectile_spawn_observed。

SR-4 optional real-window 状态规则：
  optional real-window skipped 不应让 top-level status 变 partial。
  已规定只有 blocking evidence 未满足时才允许 partial，
  P0-4 完成态不能以 partial 收尾。
```

无需修改：

```text
NU-1 headless-first 仍正确：
  项目 skill 明确真实 OS window / GPU screenshot 默认 local-only 或 optional，
  所以 B-min 不应升级为 C full real-window mandatory。

NU-2 Report Panel 接入保持轻量：
  231 只要求注册 summary/evidence，不新建另一套报告系统。

NU-3 不新增 engine core 玩法 API：
  Player / Enemy / Bullet / Score / Health 等仍只来自项目侧资产、Rule、AUI Binding 或项目报告。
```

后续施工文档必须继承：

```text
必须分 Gate 跑测试。
必须证明 exported Game.exe 进程被启动。
必须证明 228/229/230 evidence 来源清楚。
必须把 optional real-window 和 blocking headless gate 分开。
必须在施工文档自审中写明是否读取外部审查文档，以及是否需要修改本方案。
```

## 14. 结论

`Exported Windows Playable Golden Gate v1` 的正确 v1 形态是：

```text
用真实导出的 Game.exe 做最终验收对象；
默认以 headless deterministic packaged player 作为阻塞 gate；
强制证明真实贴图、真实玩法、真实 HUD、AUI glyph/render；
真实窗口/GPU screenshot 作为 optional/local-only evidence；
最终输出一个 AI 可审查、用户可理解、Report Panel 可注册的 golden report。
```

这个系统完成后，复杂打飞机 P0 主线才从“各系统分别能跑”收敛为“导出的 Windows 包真的可玩，并且有结构化证据证明”。
