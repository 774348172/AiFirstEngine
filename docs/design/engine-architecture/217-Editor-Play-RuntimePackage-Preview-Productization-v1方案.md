# 217-Editor Play / RuntimePackage Preview Productization v1 方案

采用方案：

```text
Preview RuntimePackage Cache + Dirty Domain Detection + Incremental Reuse + Structured Timing Report
```

简称：

```text
B-min-cache
```

## 1. 这个系统是干什么的

一句话：

```text
让用户在编辑器里点击 Play 时，不再手动打开 RuntimePackage，也不每次完整 Export，而是用 RuntimePackage 真相生成或复用一个编辑器预览包，并用结构化报告解释等待时间、脏域和失败原因。
```

它解决的问题：

```text
当前 Play 必须先 Open Runtime Package。
复杂打飞机项目里 Play 是高频操作，不能每次都走完整 Desktop Export / 全量 RuntimePackage 构建。
RuntimePackage 又必须继续作为运行输入真相，不能为了快而让 Runtime 直接读编辑器内存或项目源目录。
```

它在其它成熟引擎中的对标：

```text
Unity：
  Editor Play Mode / GameView。
  Play 不是 Build Player。
  Build And Run 属于 Build Pipeline，和高频 Editor Play 分开。

Unreal：
  Play In Editor / Standalone Game。
  UI 发 FRequestPlaySessionParams，EditorEngine 延迟启动 PIE / NewProcess。
  PIE 不是每次完整打包。

Godot：
  EditorRunBar / EditorRun。
  F5/F6 运行项目或当前场景，必要时保存和 build hook，然后启动运行进程。
  GameView/EmbeddedProcess 只是显示和调试消费者。
```

它在本引擎主线中的作用：

```text
Editor Play/Preview 仍走 RuntimePackage 或等价运行产物。
Runtime 仍只加载 RuntimePackage，不扫描项目源目录。
Editor 负责把项目当前可保存内容准备成 Preview RuntimePackage。
PlaySessionController 仍负责 Play 会话，不负责内容装配细节。
PreviewPackageService 负责预览包缓存、脏域检测、构建准备和耗时报告。
```

## 2. 为什么不能每次 Play 全量构建

Play 是高频反馈操作：

```text
改一个按钮位置。
改一条规则。
改一张 HUD 图片。
改一个敌人出生点。
都可能立刻点击 Play 验证。
```

如果每次点击 Play 都完整执行：

```text
ProjectRuntimePackageAssembler
  -> RuntimePackageBuilder
  -> copy runtime assets
  -> Desktop package stage
  -> player executable stage
  -> validation
  -> launch
```

问题会非常明显：

```text
小改动也等待完整构建。
用户无法形成 Unity/Godot 那种高频 Play 体验。
复杂项目资源越多，Play 越慢。
AI 也无法判断到底慢在 scene assemble、asset cook、package write、load validate 还是 launch。
```

因此本方案的关键结论是：

```text
Play 不能等同 Export。
Play 可以自动准备 RuntimePackage，但必须缓存优先、脏域优先、报告优先。
```

## 3. 外部源码参考

### 3.1 Unity

本机源码参考：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\EditorApplication.bindings.cs
  EditorApplication.EnterPlaymode()
  EditorApplication.isPlaying
  EditorApplication.isPlayingOrWillChangePlaymode

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\EditorApplication.cs
  PlayModeStateChange
  playModeStateChanged
  Internal_EnterPlayModePreStart

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\GameView\GameView.cs
  playModeStateChanged listener
  target size / focus / render target handling

<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPlayerWindow.cs
  BuildAndRun button / Build Pipeline branch
```

官方参考：

```text
https://docs.unity3d.com/Manual/configurable-enter-play-mode.html
```

可学习点：

```text
Editor Play 和 BuildAndRun 是不同入口。
GameView 只负责显示、焦点、尺寸和运行输出，不负责构建编排。
Unity 提供 Enter Play Mode 配置来减少 Domain Reload / Scene Reload 的等待，说明高频 Play 的等待时间是核心体验问题。
```

不照搬：

```text
不照搬 Unity 深度一体化的 native PlayMode。
不让 Runtime 直接读 Editor 内存对象。
不把关闭 reload 的隐式状态作为 AI-first 主线。
```

### 3.2 Unreal

本项目已有源码参考：

```text
框架设计/UE源码参考/EditorPlaySession-PIE-Standalone源码参考.md
```

关键调用链：

```text
Kismet2/DebuggerCommands.cpp
  -> GUnrealEd->RequestPlaySession(SessionParams)

PlayLevel.cpp
  -> UEditorEngine::RequestPlaySession
  -> StartQueuedPlaySessionRequest
  -> StartPlayInEditorSession
  -> StartPlayInNewProcessSession
```

可学习点：

```text
UI 只发 PlaySessionRequest / StopRequest。
启动和停止延迟到稳定同步点。
InProcess PIE、NewProcess、Launcher 复用会话请求模型。
自动化测试直接构造 Request，不模拟 UI 点击。
```

不照搬：

```text
不做完整 PIE World 复制。
不做 GWorld 切换。
不做 Blueprint debugger references 转移。
不把第一版 Play 做成编辑器和 runtime 深耦合体系。
```

### 3.3 Godot

本项目已有源码参考：

```text
框架设计/Godot源码参考/11-EditorRun-GameView-PlaySession源码参考.md
```

关键调用链：

```text
EditorRunBar
  -> play_main_scene / play_current_scene / stop_playing
  -> _run_scene
  -> try_autosave
  -> call_build
  -> EditorRun::run
  -> OS::create_instance
  -> Runtime process
  -> GameView / EmbeddedProcess / Debugger
```

可学习点：

```text
RunBar / Controller 负责保存、build hook、启动、停止和 UI 状态。
EditorRun 负责运行参数和进程 pid。
GameView 监听 play/stop，只做显示和调试。
```

不照搬：

```text
不做 Godot EmbeddedProcess 平台嵌入作为 v1 必需项。
不做多实例运行。
不做完整 debugger protocol。
```

## 4. 本项目当前基线

已完成基础：

```text
84-Editor-Play-Run-Session-System方案.md
  PlaySessionRequest / PlaySessionController / PlaySessionReport C-min 已完成。

106-Build-Runtime-Package-Completion-C-min方案.md
  RuntimePackageBuildRequest / RuntimePackageBuilder / ValidationReport / DiffReport / BuildReport 已完成。

189-Project-RuntimePackage-Assembly-Completeness-v1方案.md
  ProjectRuntimePackageAssembler 是项目目录进入 RuntimePackageBuildInput 的正式入口。

190 AUI RuntimePackage Document Hydration / Binding / Present
199 ProjectUiStateSnapshot
208 Runtime Text Glyph Present
210 RuntimeRenderer Multi-stage UI Composition Pass
213-216 AUI runtime interaction/navigation/text entry
```

当前代码基线：

```text
rust/crates/editor_core/src/services/play_service.rs
  start_play_session 当前要求 self.runtime_package_path 存在。
  没有 RuntimePackage 时返回 editor.play_session.runtime_package_required。

rust/crates/editor_core/src/ui_model_composer.rs
  Toolbar Play 当前在没有 active_runtime_package 时 disabled。

rust/crates/editor_core/src/play_session.rs
  PlaySessionRequest 持有 runtime_package_path。
  PlaySessionController 负责 queue_start / drain_queued_with_runtime。

rust/crates/editor_core/src/project_runtime_package_assembler.rs
  ProjectRuntimePackageAssembler::assemble 从项目目录读取 Project / Scene / Prefab / Asset / Rule / AUI / Input。
  输出 RuntimePackageBuildInput 和 ProjectRuntimePackageAssemblyReport。

rust/crates/editor_core/src/desktop_export.rs
  DesktopExportPipeline 已经调用 Assembler -> RuntimePackageBuilder -> WindowedPlayer headless gate。
  它还负责 desktop package stage / player executable stage，不适合作为高频 Play 的默认入口。

rust/crates/engine_runtime/src/runtime_package_builder.rs
  RuntimePackageBuilder::build 输出 build-runtime-package-report.v1。
  已有 validation report / diff report / previousPackageManifest 原语。
  当前 stage 粒度仍偏粗，没有 Editor Preview cache manager。
```

当前真实缺口：

```text
Play 不能从 open project / editable scene 直接准备预览运行包。
没有 Preview RuntimePackage cache。
没有 dirty domain fingerprint。
没有 cache hit / stale / rebuild / last good report。
PlaySessionReport 没有链接 preview package report。
Toolbar 无法表达 Preparing Preview Package / Cache Hit / Build Failed。
Report Panel 没有独立展示 Editor Play Preview Package 的证据。
```

## 4.1 吸收 35 号审查后的修正

`其它AI审查目录/35-217-Editor-Play-RuntimePackage-Preview方案审查.md` 判断 B-min-cache 方向正确，但指出施工前必须澄清 Play runner 范围。

审查指出的关键问题：

```text
DefaultGameRunOrchestrator 当前是 headless end-to-end gate，不集成 runtime_player_winit 的完整 AUI present / input / ComplexShooterSampleUiStateProducer 链路。
PlaySession::WindowedUserRun 当前仍是 not_implemented_in_c_min。
project_e2e_gate 当前验证 native player，不验证 editor Play。
```

本方案吸收后的 v1 边界：

```text
217 v1 只产品化 Editor Play 的 Preview RuntimePackage 准备、缓存、dirty domain、结构化耗时报告，以及现有 PlaySession HeadlessGate 接入。
217 v1 不声称完成 windowed GameView / 嵌入式运行窗口 / runtime_player_winit 完整可视化 Play runner。
```

原因：

```text
Preview RuntimePackage cache 是当前最直接阻塞高频 Play 的问题。
windowed/GameView runner 会牵涉 runtime_player_winit、窗口生命周期、AUI/input/snapshot producer 统一和 editor viewport 托管，应该作为后续独立系统讨论。
本轮不能用 headless gate 冒充真实窗口预览，也不能把 runtime_player_winit 强行塞进 editor_core 造成 crate 耦合。
```

因此本轮采用的 runner 策略是：

```text
Preview package prepare：
  必做。

PlaySession HeadlessGate：
  必做，使用 prepared RuntimePackage 运行现有 DefaultGameRunOrchestrator。

WindowedUserRun / Embedded GameView：
  明确 deferred。

AUI/input/snapshot producer 统一到 editor Play runner：
  明确 deferred 到后续 Editor Windowed/GameView Play Runner Productization。
```

施工文档必须把这个边界写入自审：

```text
不能把 217 v1 说成完整 Unity GameView / UE PIE 替代品。
不能让验收标准要求真实窗口呈现。
必须新增 editor Play e2e 接入，至少验证 EditorSession::start_play_session 通过 PreviewPackageService 生成/复用 RuntimePackage 并启动 HeadlessGate。
```

## 5. 可选方案

### 方案 A：保持当前手动 Open RuntimePackage

内容：

```text
用户先 Build / Export RuntimePackage。
用户再 Open Runtime Package。
点击 Play 只运行已打开包。
```

优点：

```text
改动最小。
当前代码已经支持。
运行真相清楚。
```

缺点：

```text
用户体验差。
AI 也无法自动验证“当前编辑内容”。
复杂打飞机项目每次编辑后都要手动准备包。
不符合成熟编辑器 Play 体验。
```

结论：

```text
不采用，只作为 fallback / debug 入口保留。
```

### 方案 B：每次 Play 自动全量 RuntimePackage 构建

内容：

```text
点击 Play。
立即调用 ProjectRuntimePackageAssembler。
立即调用 RuntimePackageBuilder。
每次重写完整 RuntimePackage。
再启动 PlaySession。
```

优点：

```text
实现简单。
每次都能保证包来自当前项目磁盘内容。
报告链路容易接。
```

缺点：

```text
Play 高频操作会被构建时间拖慢。
复杂项目资源越多，Play 越不可用。
无法区分无变更、轻微变更和全量变更。
如果未来 asset cook / Rust AOT / font atlas / audio processing 变重，这个方案会迅速退化。
```

结论：

```text
不采用裸方案 B。
它只能作为 B-min-cache 里的 cache miss fallback。
```

### 方案 C：Preview RuntimePackage Cache

内容：

```text
点击 Play。
先生成或读取 source fingerprint。
检查 Editor Preview RuntimePackage cache。
cache hit 直接运行 cached package。
cache stale 才 assemble/build。
构建完成后更新 cache manifest。
失败时输出结构化报告，不静默运行旧包。
```

优点：

```text
保持 RuntimePackage 运行真相。
高频 Play 的常见路径是 cache hit。
脏域明确，AI 能知道到底是 Scene / AUI / Rule / Input / Asset / BuildProfile 变了。
可以逐步扩展真正的 domain-level incremental cook。
和当前 RuntimePackageBuilder / DiffReport / ReportPanel 主线一致。
```

缺点：

```text
需要新增 PreviewPackageService。
需要维护 fingerprint / cache manifest。
第一版 cache stale 时可能仍调用完整 RuntimePackageBuilder，不应假装已经有完美增量 cook。
```

结论：

```text
采用。
```

## 6. 正式采用：B-min-cache

正式命名：

```text
Editor Play / RuntimePackage Preview Productization v1
```

采用：

```text
B-min-cache：
  RuntimePackage 仍是真相。
  Play 使用 Preview RuntimePackage cache。
  Dirty domain 检测决定是否复用、重建或失败。
  所有耗时和决策进入结构化 report。
```

核心链路：

```text
Toolbar Play
  -> PlaySessionStartRequest
  -> EditorPreviewPackageService::prepare
  -> Fingerprint Project Sources
  -> Preview Cache lookup
  -> CacheHit:
       use cached RuntimePackage path
  -> CacheMiss / Stale:
       optional autosave editor documents
       ProjectRuntimePackageAssembler::assemble
       RuntimePackageBuilder::build
       RuntimePackageLoader validate
       write preview cache manifest
  -> PlaySessionRequest { runtime_package_path }
  -> PlaySessionController
  -> DefaultGameRunOrchestrator HeadlessGate
  -> PlaySessionReport + PreviewPackageReport
  -> Console / Report Panel / RuntimeTrace
```

## 7. 核心规则

### 7.1 RuntimePackage 真相不变

规则：

```text
Runtime 不读取项目源目录。
Runtime 不读取 EditorSession 内存。
Runtime 不扫描 Asset DB。
Runtime 只加载 Preview RuntimePackage 或正式 Export RuntimePackage。
```

Preview RuntimePackage 和正式 Export RuntimePackage 的关系：

```text
Preview RuntimePackage：
  位于 editor preview cache。
  面向高频 Play。
  不 stage desktop package。
  不 copy player executable。
  默认 debug readable json。

Export RuntimePackage：
  位于 Build/Windows/... 或正式 output。
  面向发布 / 可交付 Windows 包。
  进入 DesktopExportPipeline。
  可以 stage player executable / reports / package manifest。
```

共同点：

```text
都必须通过 ProjectRuntimePackageAssembler -> RuntimePackageBuilder。
都必须能被 RuntimePackageLoader 读取。
都必须输出 validation / diff / build report。
```

### 7.2 Play 不等于 Export

禁止：

```text
点击 Play 默认执行 DesktopExportPipeline。
点击 Play 默认复制 player executable。
点击 Play 默认 cargo build / Rust AOT compile。
点击 Play 默认完整平台发布。
```

允许：

```text
Play 在 cache miss / stale 时调用 ProjectRuntimePackageAssembler。
Play 在 cache miss / stale 时调用 RuntimePackageBuilder。
Play 使用 current dev profile / preview profile。
Play 写入 editor preview cache 目录。
```

### 7.3 dirty domain 是 Play 准备的核心判断

第一版 dirty domain：

```text
Project
BuildProfile
Scene
Prefab
Asset
Rule
Aui
Input
FontAtlas
EngineSchema
```

fingerprint 输入：

```text
project.aife.json
BuildProfiles/*.json
Scenes/**/*.json
Prefabs/**/*.json
Assets/** metadata / asset records
Rules/**/*.json
AUI/**/*.json
Input/**/*.json
RuntimePackage schema versions
AUI / Input / Rule / Scene schema versions
active_scene_id
build_profile
engine_runtime package format version
```

规则：

```text
fingerprint 稳定、可序列化、可报告。
fingerprint 不依赖文件枚举随机顺序。
只改 selection / viewport / inspector foldout 不应让 runtime package stale。
EngineSchema 变化必须强制 stale。
无法读取的文件必须进入 diagnostic，不能默默当作无变化。
```

### 7.4 cache 状态

Preview cache status：

```text
None：
  没有可用 cache。

Hit：
  fingerprint 完全一致，可以直接运行 cached RuntimePackage。

Stale：
  有 cache，但 source fingerprint 或 schema fingerprint 不一致。

Rebuilt：
  stale 后完成重建。

Failed：
  准备包失败。

LastGoodAvailable：
  当前构建失败，但存在上一次成功包。
```

默认策略：

```text
Hit：直接 Play。
None / Stale：准备新 Preview RuntimePackage。
Failed：默认不自动运行 last good，避免用户误以为当前修改已生效。
LastGoodAvailable：只提供显式 Run Last Good 后续入口，本轮可先 report deferred。
```

### 7.5 unsaved editor document 策略

当前主线要求：

```text
ProjectRuntimePackageAssembler 从项目目录读取 saved project。
RuntimePackage 是运行输入真相。
```

因此 v1 不新增第二套 in-memory runtime export bridge。

本轮策略：

```text
Play 前如果存在 dirty authoring document：
  优先走已有 Save command / scene save / aui save / rule save 能力，形成可报告的 autosave stage。
  如果该文档不能安全保存，PreviewPackageService 返回 failed diagnostic。
  不从 EditorSession 内存直接拼 RuntimePackage 绕过 ProjectRuntimePackageAssembler。
```

说明：

```text
这和 Unity 的“不保存也能 Play”体验不同，但更符合本项目 RuntimePackage 真相和 AI-first 可审查主线。
后续如果要支持 unsaved preview snapshot，必须先把它设计成正式 PreviewSourceSnapshot schema，并仍进入 ProjectRuntimePackageAssembler 等价入口；不能做临时桥。
```

### 7.6 增量的真实边界

B-min-cache 的“增量”分两级：

```text
v1 必做：
  cache hit 时完全跳过 RuntimePackageBuilder。
  cache stale 时报告 dirty domains。
  cache stale 时复用 previousPackageManifest 生成 diff report。
  不 stage desktop player。

v1 不承诺：
  每个 dirty domain 都能只写局部文件。
  asset cook 已经完全增量。
  font atlas / texture / audio 都有独立 incremental cooker。
```

禁止：

```text
报告里写 partial rebuild，但实际没有跳过对应阶段。
为了看起来快而让 Runtime 读取旧内容。
```

### 7.7 Play runner 边界

217 v1 的 Play runner 范围：

```text
采用现有 PlaySessionMode::HeadlessGate。
PlaySessionRequest 使用 PreviewPackageService 准备出的 runtime_package_path。
DefaultGameRunOrchestrator 继续作为 headless deterministic gate。
PlaySessionReport 必须链接 PreviewPackageReport。
```

217 v1 不做：

```text
WindowedUserRun 实现。
嵌入式 GameView。
runtime_player_winit 进入 editor_core 依赖。
完整 AUI present / input / snapshot producer 在 editor Play runner 中统一。
```

后续应单独讨论：

```text
Editor Windowed/GameView Play Runner Productization v1
  -> 选择 runtime_player_winit 新进程 / editor host window / embedded view 的正式路线。
  -> 统一 AUI present / input / ComplexShooterSampleUiStateProducer。
  -> 处理 Stop / process lifetime / viewport embedding / real window screenshot。
```

## 8. 预览包目录

建议目录：

```text
<project_root>/.aife/editor-preview/<profile>/<active_scene_id>/
  preview-cache-manifest.json
  runtime_package/
    manifest.json
    scenes/
    prefabs/
    assets/
    rules/
    aui/
    fonts/
    input/
    reports/
      runtime-package-validation-report.json
      runtime-package-diff-report.json
      build-runtime-package-report.json
      editor-play-preview-package-report.json
```

规则：

```text
.aife/editor-preview 是编辑器生成缓存，不是用户手写资产。
Preview cache 可以被清理，不能作为长期引用来源。
RuntimePackage 内仍保持正式 package 结构。
正式 Export 不读取 .aife/editor-preview 作为输入。
```

如果项目不允许写 `.aife`：

```text
可 fallback 到 target/editor_preview/<project_id>/<profile>/<active_scene_id>/。
fallback 必须进入 report。
```

## 9. 新增结构

### 9.1 EditorPreviewPackageRequest

```text
EditorPreviewPackageRequest
  schema_version
  project_root
  active_scene_id
  build_profile
  requested_by
  allow_autosave
  allow_last_good
  force_rebuild
  frame_limit
```

规则：

```text
默认 allow_autosave=true。
默认 allow_last_good=false。
默认 force_rebuild=false。
```

### 9.2 EditorPreviewPackageFingerprint

```text
EditorPreviewPackageFingerprint
  schema_version
  project_id
  active_scene_id
  build_profile
  engine_schema_hash
  project_hash
  build_profile_hash
  scene_hash
  prefab_hash
  asset_hash
  rule_hash
  aui_hash
  input_hash
  font_atlas_seed_hash
  combined_hash
```

### 9.3 EditorPreviewPackageCacheManifest

```text
EditorPreviewPackageCacheManifest
  schema_version
  project_root
  active_scene_id
  build_profile
  cache_key
  fingerprint
  runtime_package_dir
  build_report_path
  validation_report_path
  diff_report_path
  last_success_at
  last_success_package_hash
```

### 9.4 EditorPlayPreviewPackageReport

schema：

```text
editor-play-preview-package-report.v1
```

字段：

```text
schema_version
status
project_root
active_scene_id
build_profile
cache_dir
runtime_package_dir
cache_status
cache_key
previous_cache_key
dirty_domains
source_fingerprint
previous_fingerprint
autosave_summary
stage_reports
runtime_package_build_report_path
runtime_package_validation_report_path
runtime_package_diff_report_path
runtime_package_load_status
play_session_id
play_session_report_schema
duration_total_ms
diagnostics
next_actions
deferred_flags
```

stage report：

```text
EditorPlayPreviewPackageStageReport
  stage_id
  status
  duration_ms
  skipped
  cache_status
  dirty_domains
  diagnostics
```

建议 stage：

```text
fingerprint_sources
check_cache
autosave_dirty_documents
assemble_project_runtime_package_input
build_runtime_package
load_validate_runtime_package
prepare_play_session_request
launch_play_session
```

### 9.5 PlaySessionReport 扩展

PlaySessionReport 保持会话报告职责，但需要能链接 preview package report：

```text
PlaySessionReport
  preview_package_report_path
  preview_cache_status
  preview_dirty_domains
  preview_prepare_duration_ms
```

规则：

```text
PlaySessionReport 不吞掉 PreviewPackageReport。
PreviewPackageReport 不吞掉 RuntimePackageBuildReport。
Report Panel 需要能分别定位 preview / package / play 三层失败。
```

## 10. Toolbar / UI 行为

当前行为：

```text
没有 active RuntimePackage 时 Play disabled。
```

217 后目标行为：

```text
项目已打开：
  Play enabled。
  点击 Play 会准备 preview package。

没有项目打开：
  Play disabled。

准备中：
  Toolbar 状态显示 Preparing / Building / Launching。

cache hit：
  Console 输出 concise summary。

cache stale：
  Console 输出 dirty domains 和重建耗时。

失败：
  Console 输出失败 layer、diagnostic code、report path、next action。
```

用户可见状态保持少：

```text
No Project
Ready
Preparing
Playing
Failed
```

AI / Report 看到完整状态：

```text
cache_status
dirty_domains
stage_reports
duration_ms
diagnostics
```

## 11. 耗时目标

这些是体验目标，不是跨机器硬性测试阈值：

```text
cache hit：
  目标 100-300ms 级准备成本。
  超过 500ms 应进入 warning diagnostic。

small stale：
  Scene / AUI / Rule / Input 小改动。
  目标 0.3-2s。
  超过 2s 应报告最慢 stage。

asset stale：
  涉及 font atlas / texture / audio / large asset。
  可以更慢，但必须报告 domain 和 stage。

engine schema / build profile stale：
  可触发 full rebuild。
  必须明确说明不是普通 Play cache hit。
```

测试不能依赖固定墙钟：

```text
CI / 自动化测试主要断言 cache status、stage skipped、dirty domains 和 report 完整性。
duration_ms 只要求存在且非负。
本地性能 smoke 可以作为 optional，不作为 v1 CI 阻塞。
```

## 12. 复杂打飞机项目例子

### 12.1 只改 HUD 文本位置

```text
dirty_domains = [Aui]
cache_status = Stale
autosave_dirty_documents = aui document saved
assemble/build preview package
Play 使用新 RuntimePackage
Report 显示 AUI dirty 和耗时
```

### 12.2 连续点击 Play，内容没改

```text
dirty_domains = []
cache_status = Hit
RuntimePackageBuilder skipped
直接启动 PlaySession
```

### 12.3 改敌人 Prefab

```text
dirty_domains = [Prefab, Asset?]
cache_status = Stale
RuntimePackageBuilder rebuilds preview package
DiffReport 显示 prefab/package modified
```

### 12.4 改一张大图

```text
dirty_domains = [Asset]
cache_status = Stale
asset stage 可能耗时较长
Report 必须显示 asset domain 和 copy/cook/build stage duration
```

### 12.5 构建失败

```text
cache_status = Failed
PlaySession 不启动
Report 指向 Assembly / RuntimePackageBuilder / Loader 哪一层失败
last_good_package 可报告，但默认不运行
```

## 13. AI-first 报告

报告必须能回答：

```text
这次 Play 有没有真的构建包？
如果没构建，是因为 cache hit 还是错误跳过？
哪些 domain 变脏？
慢在哪个 stage？
运行的是哪个 RuntimePackage path？
RuntimePackageBuilder / Loader 是否通过？
PlaySession 是否启动？
失败时下一步该修哪个源文件或哪个 domain？
```

核心 diagnostic code：

```text
editor.preview_package.cache_hit
editor.preview_package.cache_stale
editor.preview_package.cache_missing
editor.preview_package.autosave_failed
editor.preview_package.assembly_failed
editor.preview_package.runtime_package_build_failed
editor.preview_package.runtime_package_load_failed
editor.preview_package.last_good_available
editor.preview_package.launch_skipped
```

deferred flags：

```text
true_incremental_asset_cook_deferred=true
async_background_preview_build_deferred=true
run_last_good_button_deferred=true
unsaved_memory_snapshot_preview_deferred=true
embedded_game_view_deferred=true
multi_instance_play_deferred=true
remote_device_play_deferred=true
rust_aot_hot_compile_on_play_deferred=true
windowed_game_view_play_runner_deferred=true
aui_input_snapshot_runner_unification_deferred=true
```

## 14. 拟施工 Gate

### Gate A：schema / report

目标：

```text
新增 EditorPreviewPackageRequest。
新增 EditorPreviewPackageFingerprint。
新增 EditorPreviewPackageCacheManifest。
新增 EditorPlayPreviewPackageReport。
PlaySessionReport 增加 preview report link。
Report Panel provider 预留 preview package report 入口。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_preview_package_schema
```

### Gate B：fingerprint / cache lookup

目标：

```text
实现稳定 source fingerprint。
实现 Preview cache manifest read/write。
实现 cache hit / missing / stale 判断。
不启动 RuntimePackageBuilder 也能输出 cache decision report。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_preview_package_cache
```

### Gate C：prepare preview package

目标：

```text
cache miss / stale 时调用 ProjectRuntimePackageAssembler。
调用 RuntimePackageBuilder 输出 preview runtime_package。
调用 RuntimePackageLoader 验证。
写 editor-play-preview-package-report.json。
失败时不启动 PlaySession。
```

测试：

```powershell
cd rust
cargo test -p editor_core editor_preview_package_prepare
cargo test -p engine_runtime runtime_package_builder
```

### Gate D：Play service integration

目标：

```text
EditorSession::start_play_session 不再要求用户手动 Open RuntimePackage。
项目已打开时，Play 先 prepare preview package。
PlaySessionRequest 使用 prepared runtime_package_path。
PlaySession 仍使用 HeadlessGate，不冒充 windowed GameView。
Toolbar Play enabled 条件从 has_package 改为 project_open && !running。
保留 Open Runtime Package 作为 debug/manual 入口。
```

测试：

```powershell
cd rust
cargo test -p editor_core play_service
cargo test -p editor_core editor_session_play
```

### Gate E：cache hit / dirty rebuild e2e

目标：

```text
新增 editor Play e2e 接入，直接通过 EditorSession 打开 sample project 并触发 Play。
第一次 Play 构建 preview package。
第二次 Play 在无变更时 cache hit，并跳过 RuntimePackageBuilder。
修改 Scene / AUI / Input / Rule fixture 后 cache stale，并报告 dirty domain。
本 Gate 验证 Editor Play HeadlessGate，不验证 runtime_player_winit windowed/GameView。
```

测试：

```powershell
cd rust
cargo test -p project_e2e_gate editor_play_preview_runtime_package_cache
```

### Gate F：Report Panel / Console / timing

目标：

```text
Report Panel 能看到 PreviewPackageReport。
Console 输出 cache status / dirty domains / duration / report path。
duration_ms 分 stage 输出。
```

测试：

```powershell
cd rust
cargo test -p editor_core report_panel
cargo test -p editor_core ui_model
```

## 15. 验收标准

必须满足：

```text
打开项目后，即使没有手动 Open RuntimePackage，Play 也能准备 Preview RuntimePackage。
无变更连续 Play，第二次报告 cacheStatus=hit。
cache hit 时 RuntimePackageBuilder stage 必须 skipped=true。
修改 Scene 后 Play 报 dirtyDomains 包含 Scene。
修改 AUI 后 Play 报 dirtyDomains 包含 Aui。
RuntimePackage 构建失败时 PlaySession 不启动。
PreviewPackageReport 链接 RuntimePackageBuildReport / ValidationReport / DiffReport。
PlaySessionReport 链接 PreviewPackageReport。
Toolbar 不再把“没有手动打开 RuntimePackage”作为项目已打开时的 Play 禁用理由。
DesktopExportPipeline 不进入默认 Play 路径。
217 v1 明确保持 headless Play gate；真实 windowed/GameView 预览不在本轮验收。
```

不允许用以下方式冒充完成：

```text
每次 Play 都无条件全量构建，但报告写 cache。
cache hit 时仍实际调用 RuntimePackageBuilder。
构建失败后静默运行 last good package。
Runtime 直接读取项目源目录。
Editor Play 直接读取 EditorSession 内存对象绕过 RuntimePackage。
点击 Play 默认执行完整 Windows Export。
为了快而跳过 RuntimePackageLoader validation。
用 HeadlessGate 冒充已经完成真实窗口预览。
```

## 16. 对用户和 AI 的心智

用户心智：

```text
我点 Play，就是运行当前项目的预览包。
如果什么都没改，它应该很快。
如果改了东西，它会准备一下，并告诉我慢在哪。
如果构建失败，它不会偷偷运行旧内容。
```

AI 心智：

```text
改运行内容：改项目资产 / Scene / AUI / Rule / Input。
判断 Play 是否用了当前内容：看 PreviewPackageReport 的 fingerprint / dirty domains / cache status。
判断失败位置：看 autosave / assembly / package build / loader / launch stage。
不要让 Runtime 直接读项目源目录。
不要绕过 ProjectRuntimePackageAssembler。
```

## 17. 自审

### 17.1 是否增加过多结构

```text
没有增加新的运行时架构层。
新增的是 Editor 侧 PreviewPackageService 和结构化 report/cache manifest。
RuntimePackage / PlaySession / RuntimePackageBuilder 主线保持不变。
35 号审查指出的 windowed/GameView runner 缺口被明确 deferred，没有在本轮方案中冒充完成。
```

### 17.2 是否会让 Play 太慢

```text
方案目标就是避免每次 Play 全量构建。
cache hit 是第一优先级。
cache stale 才构建。
耗时必须分 stage 报告，后续才能针对最慢 domain 做真正增量优化。
```

### 17.3 是否和 RuntimePackage 真相冲突

```text
不冲突。
Preview Package 仍是 RuntimePackage。
Runtime 仍只加载 package。
差异只在 Editor 侧 cache 和准备策略。
```

### 17.4 是否适合复杂打飞机项目

```text
适合。
复杂打飞机会频繁改 Scene、Prefab、AUI、Input、Rule、Asset。
本方案能让这些改动进入 RuntimePackage，同时避免无变更 Play 重复构建。
```

### 17.5 是否符合 AI-first

```text
符合。
所有关键判断都有 schema/report：
cache status、dirty domains、fingerprint、stage duration、diagnostic、next action。
AI 不需要猜 Play 慢在哪里，也不需要猜运行的是不是旧包。
```

## 18. 结论

```text
217 采用 B-min-cache。
下一步如果进入施工，应先读取可能存在的其它 AI 审查文档，再生成自动化施工文档。
施工文档必须按 Gate A-F 拆分，并在每个 Gate 后运行对应测试。
施工文档必须写明：217 v1 完成 Preview RuntimePackage cache + Editor Play HeadlessGate 接入；windowed/GameView runner 后续单独讨论。
```
