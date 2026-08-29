# 304 Android Native Player Dev APK Vertical Slice B v1 方案

## 1. 文档状态

```text
系统编号：304
方案版本：v1
建立日期：2026-08-19
问题来源：Tower 已可导出并运行 Windows dev package，但正式 Rust 主线不能生成可安装 Android 包
讨论方案：A Tower 项目专属 Android 壳；B 通用 Android dev APK 纵切；C 完整 Android 发布系统
用户选择：方案 B
当前状态：源码纵切已完成并归档；真实 APK/设备 qualification 因工具链缺失未运行
```

本文档定义 AI First Game Engine Rust 正式主线的第一条真实 Android dev APK 纵切。它只负责让普通
项目通过公共 BuildProfile / RuntimePackage / Project RuntimeModule 能力生成、安装并运行一个 ARM64
debug APK。304 极简施工已完成源码 owner 与 host-side Gate；本文档仍不构成 Android 工具链安装、Local
CI、production Editor/Player 替换、真实配置修改、设备安装或发布签名授权。

施工结果见：

```text
施工文档/已完成/304-当前可自动化施工文档-Android-Native-Player-Dev-APK-Vertical-Slice-B-v1.md
阶段完成记录/2026-08-20-Android-Native-Player-Dev-APK-Vertical-Slice-B-v1/00-总览.md
```

## 2. 一句话目的

让 Tower 作为普通外部项目，通过通用引擎导出链生成一个可安装的 `arm64-v8a` Android debug APK；APK
启动后使用与 Windows Player 相同的 RuntimePackage、项目 Rust RuntimeModule、WGPU Renderer、竖屏
GameView presentation、AUI 与 gameplay 逻辑，并能通过真实触摸完成最小游戏操作。

## 3. 当前基线与已确认缺口

### 3.1 正式导出 owner 只有 Windows

`rust/crates/editor_core/src/desktop_export.rs` 当前只有：

```text
DesktopExportTarget::Windows
DesktopExportRequest::windows_dev
desktop-package-manifest.v1
```

`ProjectRuntimePackageAssembler` 的 BuildProfile 校验只接受 `target=windows`，默认读取和 source mapping 也
写死 `BuildProfiles/windows.dev.json`。因此新增一个 Android JSON 文件并不能进入正式装配链。

### 3.2 Android/iOS skeleton 不属于当前正式 Rust 主线

仓库中旧 Android/iOS export skeleton 与 build gate 只存在于：

```text
legacy/typescript-prototype/scripts/**
```

它们属于已退役 TypeScript prototype，只能提供历史需求参考，不能恢复为正式 Android exporter，也不能
以生成 Gradle 文本但不生成 APK 的 skeleton 冒充完成。

### 3.3 Windows DLL loader 不能作为 Android RuntimeModule 路径

`ProjectRuntimeNativeModuleLoader` 的 production owner 使用 `LoadLibraryExW`；非 Windows 分支明确返回
`project_runtime.native_module_platform_unsupported`。Android 不应移植 Windows DLL cache/promotion/loader。

现有 `engine_runtime::linked_project_runtime_set_from_api` 已提供更适合移动端的正式接线：Android Player
在构建期链接项目 RuntimeModule，并把项目导出的 `ProjectRuntimeApi` 注册为
`LinkedProjectRuntimeSet`。RuntimePackage 仍按 module id / interface / digest 执行 exact bind。

### 3.4 Winit/WGPU 主体可复用，但 Android host contract 尚未接通

`runtime_player_winit` 已有持续逐帧 Windowed Player、WGPU surface、RuntimePackage session、AUI 与
`ResolvedGameViewPresentation`。当前 Android 缺口是：

```text
EventLoopBuilderExtAndroid::with_android_app 未接入；
没有 GameActivity android_main 入口；
没有 suspended/resumed surface lifecycle；
没有 WindowEvent::Touch 到 RawInputEvent/AUI pointer 的映射；
RuntimePackage 只接受普通文件系统路径，不能直接读取 APK assets；
真实窗口诊断仍包含 Windows-only 文案与假设。
```

### 3.5 Tower consumer 与本机工具链状态

Tower 当前只有 `BuildProfiles/windows.dev.json`。项目 `RuntimeModule` 已通过 v2 ABI 导出
`aife_project_runtime_entry_v1`，适合作为 Android launcher 的构建期依赖，不需要重写玩法。

2026-08-19 context scan 中，本机没有 JDK、Android SDK/NDK、ADB、Gradle 或 `aarch64-linux-android`
Rust target。这是施工环境前置，不是产品代码失败；正式施工不得静默下载或安装这些外部工具。

## 4. 成熟实现参考与取舍

### 4.1 Unity

Unity Android Build Profile 可以直接生成 APK/AAB，也可以导出 Gradle project；debug signing 用于设备
测试，custom signing 用于发布。可学习点是 BuildProfile、Gradle、签名与设备运行分层；不照搬 Unity
完整 PlayerSettings、IL2CPP、商店与插件生态。

参考：`https://docs.unity.cn/6000.0/Documentation/Manual/android-BuildProcess.html`

### 4.2 Godot

Godot 明确探测 JDK、SDK、Build Tools、NDK 与 CMake，再通过 Gradle 生成 APK/AAB；设备安装与 Play
Store signing 是独立阶段。可学习点是 toolchain preflight、结构化 blocking issue 和真实包输出；不照搬
Godot 的模块、PCK 或 EditorSetting 格式。

参考：`https://docs.godotengine.org/en/stable/tutorials/export/exporting_for_android.html`

### 4.3 Android GameActivity

Android 官方推荐新游戏使用 GameActivity，而不是为 NativeActivity 继续扩张自定义 glue。GameActivity
负责把生命周期、触摸、按键和文本输入传给原生游戏线程，并以 SurfaceView 承载渲染。

参考：`https://developer.android.com/games/agdk/game-activity`

### 4.4 Bevy / Winit

Bevy mobile example 使用 Rust `cdylib`、`android-game-activity` 与 Gradle Android app；native example
通过 `arm64-v8a` ABI filter 组装应用。Winit 0.30.13 已提供
`EventLoopBuilderExtAndroid::with_android_app` 与 `android-game-activity` feature。

参考：

```text
https://github.com/bevyengine/bevy/blob/main/examples/mobile/Cargo.toml
https://github.com/bevyengine/bevy/tree/main/examples/mobile/android_example_native
```

Bevy 的旧 `cargo-apk` 示例已标记 deprecated。本方案不采用 cargo-apk 作为正式 owner；Rust `.so`
由受控 NDK cross-build 产生，再交给 Gradle wrapper 组包。

## 5. 方案比较与正式选择

### 5.1 方案 A：Tower 项目专属 Android 壳

把 Gradle、Activity、JNI/entry 与 engine crate glue 直接放进 Tower。它能较快做出一次性 APK，但会让
Tower 承担引擎平台层职责，其他项目无法复用，也违反外部项目边界。冻结，不施工。

### 5.2 方案 B：通用 Android dev APK 纵切

```text
Project + android.dev BuildProfile
  -> ProjectRuntimePackageAssembler
  -> RuntimePackage
  -> generated Android launcher + linked Project RuntimeModule
  -> aarch64-linux-android native library
  -> Gradle assembleDebug
  -> verified debug APK
  -> optional ADB install/launch/touch qualification
```

这是本次正式采用方案。它建立真实可复用能力，但把第一版严格限制在 ARM64 debug APK。

### 5.3 方案 C：完整 Android 发布系统

同时完成 AAB、正式 keystore、multi-ABI、Play Store metadata、asset delivery、增量安装、崩溃符号和完整
设备矩阵。长期有价值，但不是“先让 Tower 在手机运行”的必要条件。本轮冻结。

## 6. 架构边界与 owner

### 6.1 ProjectRuntimePackageAssembler 继续是唯一项目装配入口

Android exporter 必须显式传入 `BuildProfiles/android.dev.json`，并复用现有 assembler/cook/bundle/font/
AUI/rule/input 产物。禁止为 Android 再造 Scene、AUI、Font、Texture 或 Rule 导出桥。

BuildProfile v1 在本轮只扩展已存在的 `target` 判定以接受 `android`，不新增 Android release 字段：

```json
{
  "schemaVersion": "build-profile.v1",
  "profile": "dev",
  "target": "android",
  "runtimePackageMode": "debug-readable",
  "frameLimit": 3,
  "headlessSurfaceGate": false,
  "realWindowSmoke": "optional",
  "gameViewTarget": {
    "extent": { "width": 720, "height": 1280 },
    "scalePolicy": "contain"
  }
}
```

`applicationId`、versionCode、release signing 等可发布身份不塞入 v1。dev application id 由 project id
确定性生成并写入 manifest；未来 release profile 再引入显式、可审查的 Android application identity。

### 6.2 Android exporter 是 Desktop exporter 的同级 owner

新增窄的 `AndroidDevExportPipeline`，共享 RuntimePackage producer 和通用 process ownership，但不把
`DesktopExportPipeline` 大规模改名或重写。它只负责：

```text
profile/platform validation；
toolchain preflight；
RuntimePackage build；
generated launcher / Gradle staging；
Rust ARM64 native build；
Gradle assembleDebug；
APK structural verification；
atomic publish 与结构化 report。
```

Android exporter 不负责下载 SDK、接受许可证、创建发布证书、上传商店或修改系统环境变量。

### 6.3 Android Player 使用薄入口和共享 Runtime

新增项目无关 `runtime_player_android` host library，负责 AndroidApp、GameActivity lifecycle、APK asset
materialization 与启动参数。真正的 World、fixed simulation、RuntimePackage binding、AUI、Renderer、WGPU
surface 和 GameView presentation 继续复用 `engine_runtime` / `runtime_player_winit`。

每次导出在 run-owned staging 中生成一个极薄 launcher crate：

```text
crate-type = cdylib
depends on runtime_player_android
depends on the selected project RuntimeModule source package
android_main(AndroidApp)
  -> project aife_project_runtime_entry_v1()
  -> linked_project_runtime_set_from_api(api)
  -> runtime_player_android::run(...)
```

项目 RuntimeModule 是 build-time composition，不通过 APK 可写目录动态下载或 `dlopen`。这既复用现有
ABI exact bind，也避免把 292 Windows DLL promotion/cache 机制错误搬到 Android。

### 6.4 Android 工具链基线

首版冻结单一可复现组合：

```text
JDK：17
compileSdk / targetSdk：35
minSdk：26
NDK：r28b 系列
Rust target：aarch64-linux-android
ABI：arm64-v8a
package kind：debug APK
Android host：GameActivity
```

具体 patch 版本、Gradle wrapper 与 Android Gradle Plugin 版本在施工文档激活前按本机可安装的官方兼容
组合固定，并写入 lock/report。不得使用“latest”作为 build identity。

Toolchain probe 必须先于高成本构建，缺少任一 owner 时 fail-fast，并提供缺失项和安装动作；不得在引擎
进程中自动安装软件或接受 Android SDK license。

## 7. RuntimePackage 在 APK 中的合同

Gradle assets 内保存 RuntimePackage 的原始目录与一份 export asset manifest：

```text
app/src/main/assets/aife/runtime-package/**
app/src/main/assets/aife/runtime-package-asset-manifest.json
```

Android assets 不是普通文件系统路径。启动时 `runtime_player_android` 使用 Android AssetManager，把声明的
文件按 manifest 复制到应用私有目录中的 digest-addressed root：

```text
<internalData>/aife/runtime-packages/<runtimePackageDigest>/
```

规则：

```text
manifest/hash 完全匹配 -> 复用已 materialize root；
缺失或 hash 不一致 -> 写入 sibling staging，逐文件校验后原子 publish；
失败 -> 不启动 Runtime，不读取项目源目录，不使用旧错配 package；
成功 -> 交给现有 RuntimePackageLoader，Runtime 不感知 APK AssetManager。
```

该桥只解决 Android package resource 到现有 RuntimePackage path 的边界，不建立第二套 RuntimePackage
schema、Android 专用 scene loader 或运行时 source scan。

## 8. GameActivity、Surface 与帧循环

### 8.1 EventLoop 接线

Android launcher 从 GameActivity 获得 `AndroidApp`，通过 Winit
`EventLoopBuilderExtAndroid::with_android_app` 创建 event loop。Windows 仍使用现有
`with_any_thread(true)` 路径，两个平台不共享错误的 event-loop 构造假设。

### 8.2 Lifecycle

```text
resumed：创建/恢复 Window 与 WGPU surface，按当前 physical extent resolve presentation；
suspended：停止 redraw，释放 surface/window-dependent resource，不推进 simulation；
再次 resumed：重建 surface，复用已加载 RuntimePackage/World session；
destroy/exit：关闭 session，释放 GPU 与 project module，生成一次 terminal summary。
```

首版不实现后台持续战斗、通知、保存恢复、低内存重载或进程被杀后的完整 checkpoint。

### 8.3 普通帧语义

继承 284/294/295/300：

```text
每次 visible redraw 最多一次 AUI/render/presentation；
fixed simulation 可 0..N catch-up；
AUI 点击反馈与业务 action 在普通帧即时处理，不等待 fixed tick；
clean UI frame 复用 present cache，不每帧全量 resolve；
Runtime 默认 Off，不写逐帧 JSON/report。
```

## 9. 触摸、竖屏与 AUI 输入合同

首版把 Winit `WindowEvent::Touch` 的 primary contact 映射到现有 pointer contract：

```text
Started   -> PointerMoved + PointerPressed
Moved     -> PointerMoved
Ended     -> PointerMoved + PointerReleased
Cancelled -> PointerCancelled / 清理 pressed state
```

触摸坐标保持 surface physical pixel，不预先按 DPI 或 reference canvas 缩放；World/AUI hit test 必须继续
通过共享 `ResolvedGameViewPresentation` 的 contain rect 转成 target-local 坐标。Render content rect 与
input rect 必须来自同一 presentation identity。

首版只承诺 single-primary-touch gameplay。multi-touch gesture、虚拟键盘、IME candidate UI、gamepad、
传感器和系统 back navigation 另立后续能力。Tower 当前核心操作不依赖这些能力。

Android manifest 锁定 portrait；720x1280 是 logical GameView target，不要求设备物理屏幕恰好等于该尺寸。
异形屏、安全区和系统栏先由 edge-to-edge=false 的可用 content surface 隔离，不在 v1 自建 safe-area schema。

## 10. 输出、身份与报告

默认输出布局：

```text
<authorized output>/Android/dev/
  TowerDefense-debug.apk
  package-manifest.json
  reports/android-dev-export-report.json
  reports/android-device-smoke-report.json   # 仅运行设备 Gate 时存在
```

`android-dev-package-manifest.v1` 至少记录：

```text
project id / project digest
profile id / target / ABI / SDK baseline
application id / version / signing kind / signing certificate fingerprint
RuntimePackage manifest digest
Project RuntimeModule id / interface / contract digest / AOT content digest
native library path / SHA-256
APK path / SHA-256
export report relative path
```

`android-dev-export-report.v1` 只记录阶段聚合与首个阻塞诊断：

```text
profile -> toolchain -> runtimePackage -> launcher -> rustNative -> gradle -> apkVerify -> publish
```

普通 Android Runtime 保持 Off；只有 fatal startup diagnostic 和一次 terminal summary 可以持久化。禁止把
触摸、Animator、AUI、World 或 render command 详单逐帧写盘。

APK build 与 device qualification 分开声明：

```text
packageStatus = success/failed
deviceQualification = notRun/passed/failed
```

没有连接设备时允许声明“APK 已构建并结构校验”，但不得声称“已在手机运行”。

## 11. Tower consumer

Tower 项目侧只允许增加普通项目资产与验证：

```text
BuildProfiles/android.dev.json；
必要的项目 application display metadata（若公共 project schema 已有对应字段）；
项目级 Android dev export consumer test / smoke scenario；
完成后保留 run-owned APK manifest 与运行报告。
```

不得在 Tower 中提交 Gradle wrapper、Activity、JNI glue、engine fork、硬编码 SDK path 或引擎内部 fixture。
Tower gameplay、UI、字体、动画和 Windows BuildProfile 不因 Android 导出而改变。

Tower 真实 device smoke 的最小行为合同：

```text
APK 安装并创建可见 portrait surface；
首帧出现战报主界面，无白色 fallback、倒字或缺字；
触摸征兵使数量 10 -> 7；
触摸部署和出战生效；
怪物连续移动，AUI 点击反馈即时；
返回桌面再回到应用后 surface 可恢复且不崩溃。
```

## 12. 最小验证合同

### 12.1 Owner-level red-capable tests

```text
BuildProfile target=android 被正式接受，unsupported target 仍 fail-closed；
assembler 显式消费 android.dev.json，source mapping 不再写死 windows.dev；
generated launcher 只链接选定 project RuntimeModule；
RuntimePackage asset manifest 缺失/错 hash 时拒绝启动；
Touch Started/Moved/Ended/Cancelled 映射与 pressed cleanup；
suspend/resume 只重建 surface，不重新创建 gameplay session；
Android report Off 不产生逐帧文件。
```

### 12.2 Cross-build / package integration

```text
aarch64-linux-android project launcher cross-build；
Gradle assembleDebug exit 0；
APK 内含唯一 arm64-v8a native library、GameActivity 与完整 RuntimePackage assets；
APK/package manifest/native library/RuntimePackage/module identities 互相一致；
同输入重复导出生成相同 RuntimePackage/module identity，签名与 Gradle 非确定性字段单独记录。
```

### 12.3 一次真实 Android qualification

当且仅当用户单独授权设备安装/运行且 ADB 只有一个明确目标设备时执行：

```text
adb install；
启动 Activity 并确认进程/可见 surface；
执行 Tower 最小触摸序列；
background/resume 一次；
保存一份 screenshot/logcat bounded evidence；
停止本次 app，确认没有 engine-owned host process 残留。
```

不要求 Local CI、完整 E2E、全 workspace、所有 Android API、模拟器+真机双矩阵或多设备并行。施工文档按
实际修改 owner 选择最便宜的 red-capable test，不得机械重复同一配置的 suite。

## 13. 失败边界与回滚

```text
toolchain 缺失：在 staging 前 fail-fast，不修改项目与 output；
Rust cross-build 失败：保留 run-owned compiler evidence，不运行 Gradle；
Gradle 失败：不 publish APK，保留首个 actionable failure；
APK verify 失败：不声明 package success；
ADB install/launch 失败：package success 可保留，device qualification 单独 failed；
项目/引擎 source 变化：只失效消费该 identity 的证据，不自动重跑全部历史验证。
```

Android build 只写用户明确授权的 output 与 run-owned staging。它不替换 production Editor、Windows
Player、MCP 或其它安装态二进制，也不修改系统 `JAVA_HOME` / `ANDROID_HOME`。外部工具链安装、SDK
license、USB debugging 和设备 app 卸载均需要后续明确授权。

## 14. 明确不做

```text
不恢复 legacy TypeScript Android/iOS exporter；
不生成空 Gradle skeleton 冒充 APK；
不实现 iOS；
不实现 release AAB、Play Store、正式 keystore 或上传；
除第 18 节单独授权的 x86_64 emulator debug artifact 外，不实现其它 ABI、multi-ABI APK、
App Bundle split 或 Play Asset Delivery；
不实现热更新下载代码或从可写目录加载任意 project native library；
不改写 RuntimePackage schema、Renderer、AUI、ECS、Tower gameplay 或 Windows exporter；
不新增平台无关大一统 Export Framework 重构；
不自动安装 JDK/SDK/NDK/cargo 工具；
不默认运行 Local CI、完整 E2E 或全设备矩阵。
```

## 15. 建议后续施工窗口

本文不是施工文档。后续极简施工文档建议最多三个窗口：

```text
Window A：
  Android profile/assembler owner、GameActivity entry、surface lifecycle、touch mapping；
  owner-level deterministic tests。

Window B：
  AndroidDevExportPipeline、toolchain probe、generated launcher、NDK/Gradle、APK manifest/verification；
  一次 ARM64 debug APK build。

Window C：
  Tower android.dev consumer；
  用户单独授权后的一次 ADB install/launch/touch/background-resume qualification；
  完成记录与归档。
```

实际施工文档必须先做必要性/验证经济性审计。若 Window A 证明现有 Winit/WGPU owner 需要超出上述窄
接线的大规模重构，应停止并回到方案复核，不能把 v1 扩张为完整移动平台。

## 16. 风险与控制

### 风险 1：把 Windows DLL lifecycle 搬到 Android

控制：Android launcher 在 build time 链接 project RuntimeModule，只通过现有 API adapter 建立 linked set。

### 风险 2：Android suspend 后 surface 已失效仍继续 present

控制：suspended 明确停止 redraw 并释放 surface；resumed 重新 resolve physical extent 与 presentation。

### 风险 3：触摸坐标重复缩放

控制：Winit touch 保持 surface physical pixel，Render/Input 只在共享 presentation owner 中转换一次。

### 风险 4：APK assets 被当成普通路径

控制：唯一 AssetManager materialization owner 按 manifest/hash 原子发布到 app internal data；Loader 不猜路径。

### 风险 5：工具链安装与产品实现混为一体

控制：probe 只读；安装、license 与环境变量属于独立用户授权。版本固定进入报告，不使用 latest。

### 风险 6：首版演变为完整发布平台

控制：默认只接受 ARM64 debug APK；第 18 节只增加显式、互斥的 x86_64 emulator debug artifact。
AAB、release signing、multi-ABI 和 store 明确 deferred。

## 17. 方案自审

### 17.1 是否符合用户选择的方案 B

是。它建立通用引擎 Android dev APK 能力，并使用 Tower 作为普通 consumer；没有采用 Tower-only 壳，
也没有提前建设完整 Android release pipeline。

### 17.2 是否保持 RuntimePackage 与项目规则边界

是。`ProjectRuntimePackageAssembler` 仍是唯一项目装配入口；Android 只增加平台 host、资产 materialize
和 package owner。Tower 语义不进入引擎 Core，Android glue 不进入 Tower。

### 17.3 是否复用已有能力而非重复建设

是。方案复用 `LinkedProjectRuntimeSet`、ProjectRuntimeApi adapter、Windowed Player session、WGPU、AUI、
ResolvedGameViewPresentation、BuildProfile 与 RuntimePackage，不建立 Android 专用 Runtime 或玩法桥。

### 17.4 是否过量施工

否。方案冻结 AAB、release signing、multi-ABI、iOS、store、safe-area schema、virtual keyboard 和完整设备
矩阵；第 18 节只增加单 ABI x86_64 emulator debug 出口，不要求大一统 Export Framework 重构或 Local CI。

### 17.5 是否具备真实终点

是。完成终点不是生成 skeleton，而是产生 hash/manifest 可校验的 debug APK；设备 qualification 在单独
授权后证明可安装、可启动、可触摸、可恢复。

### 17.6 权限与下一步

```text
方案结论：通过
正式方案：已生成并自审
施工文档：304-E1 已完成并归档
当前施工授权：无
引擎源码修改授权：仅限第 18 节 x86_64 emulator dev export 最小扩展
外部工具安装/SDK license/设备安装授权：需要后续分别明确给出
下一步：单独授权安装并锁定 Rust x86_64-linux-android target；随后运行一次 fresh x86_64 APK export
```

## 18. 2026-08-22 x86_64 Android Emulator dev export 最小扩展

### 18.1 授权与目标

用户已明确授权引擎侧最小 `x86_64 Android Emulator dev export` 支持。该扩展只为 Windows 上的
Android Studio Emulator 提供可安装、可由 ADB/Logcat 诊断的单 ABI debug APK，不改变手机 ARM64
默认出口。

### 18.2 合同

```text
AndroidDevAbi::Arm64V8a
  rustTarget = aarch64-linux-android
  runtimePackageTarget = android-arm64-dev
  jniAbi = arm64-v8a
  output = Build/Android/dev

AndroidDevAbi::X86_64
  rustTarget = x86_64-linux-android
  runtimePackageTarget = android-x86_64-dev
  jniAbi = x86_64
  output = Build/Android/emulator-x86_64
```

`export_android_dev <project-root>` 保持 ARM64 默认兼容；只有显式
`export_android_dev <project-root> --abi x86_64` 才生成模拟器包。每个 APK 仍只含一个 ABI，package
manifest、RuntimePackage target、Cargo target、NDK linker、JNI 目录、Gradle filter 与 APK verify 必须
共享同一 typed ABI identity。

### 18.3 明确排除

```text
不生成同时包含 arm64-v8a 与 x86_64 的 multi-ABI APK；
不增加 armeabi-v7a/x86、release/AAB/store；
不修改 Tower gameplay、AUI、Renderer、RuntimePackage schema 或 production Editor；
不自动安装 Rust target、Android system image 或创建 AVD；
不运行 Local CI、完整 E2E、真机或模拟器交互矩阵。
```

### 18.4 最小验证

```text
RuntimePackageBuilder 接受 android-x86_64-dev，未知 target 继续 fail-closed；
Android exporter owner test 覆盖两个 ABI 的 target/linker/JNI/Gradle/APK identity；
默认 CLI 仍解析为 arm64-v8a，--abi x86_64 解析为显式模拟器 request；
cargo fmt + engine_runtime/editor_core 受影响定向测试；
已安装对应 Rust target 时才执行真实 cross-check/export，否则报告环境前置缺失。
```

该扩展不推翻 304 的 ARM64 手机纵切，只解除原 deferred 列表中的一个独立模拟器调试出口。

### 18.5 施工结果

2026-08-22，304-E1 源码施工与定向验证已完成。默认 ARM64 行为保持不变，显式
`--abi x86_64` 会绑定 `android-x86_64-dev`、`x86_64-linux-android`、NDK x86_64 linker、
`jniLibs/x86_64`、Gradle x86_64 filter、单 ABI APK 校验和独立输出目录。

当前能力状态为 `sourceAvailable`。2026-08-22 的 304-E2 Gate A 已将 Rust
`x86_64-linux-android` target 安装并追加锁定到 `android-304-v1`。随后的合格 fresh Tower export
preflight 全绿并完成 `rustNative`，但隔离 Gradle cache 从 Google Maven 下载 AGP/bundletool/protos 时
`Read timed out`，因此仍为 `environmentBlocked / notBuilt / notRun`，没有创建 APK output。不得将 target
安装或 native build 通过表述为已生成 x86_64 APK；下一次 export 需要新的明确 retry 授权。
