# 305 Android x86_64 Player GLES Policy + Android 17/16KB B-min-R1 v1 方案

## 1. 文档状态

```text
系统编号：305
方案版本：v1
建立日期：2026-08-25
问题来源：合格 Tower x86_64 APK 在 Android 17 beta / API 37 / 16 KB AVD 中启动后立即返回桌面
讨论方案：A 升级 WGPU/Vulkan；B Android Vulkan + GLES fallback；C 两阶段显式重建 backend
用户选择：方案 B-min，真实 Gate B 否证后改选 B-min-R1
当前状态：B-min-R1 已完成并归档；Android x86_64 固定 GL，其余目标保持 PRIMARY
```

本文档是 304 Android Native Player 纵切的窄兼容扩展。它只解决 Android Player 在 Vulkan adapter
不可用、但 OpenGL ES 可用的 Android 环境中无法启动的问题，并把实际选中的图形 backend 写入既有启动
诊断。本文档不构成源码修改、APK 重导出、模拟器安装、Local CI 或 production 二进制替换授权。

## 2. 一句话目的

让 Android x86_64 Player 只请求 OpenGL ES，使符合 16 KB 合同的 x86_64 APK 能在 Android 17 beta /
API 37 / 16 KB 模拟器中避开不可用 Vulkan 和双 backend native-window 争用；Android ARM64 与所有非 Android
x86_64 目标继续使用原 `PRIMARY`，不改变 Windows Player、Editor 或 Tower 项目逻辑。

## 3. 已确认事实

### 3.1 可稳定复现的失败

失败设备：

```text
AVD：Pixel_7
Android：17.0 beta / API 37
ABI：x86_64
系统镜像：google_apis_playstore_ps16k
page size：16 KB
GPU：host
```

安装 APK：

```text
<ANDROID_RUN_ROOT>\304-E3-presentation-fix-20260824-210800\artifacts\TowerDefense-x86_64-debug.apk
SHA-256：D279A8140D9C39AC4E3E604CA9949E5719B250D77FCDCCE784DA4C10A06D8996
ABI：仅 lib/x86_64/libmain.so
```

设备内 APK SHA-256 与上述保留 artifact 完全一致。GameActivity、`libmain.so` 和 native window 均成功
创建；没有 Java exception、native fatal signal 或 crash buffer。随后 Player 在 adapter 请求阶段返回，
GameActivity 正常销毁，因此用户看到的是“闪一下回桌面”，不是 Android 进程崩溃。

既有 `aife-startup-diagnostic.json` 给出首因：

```text
surface.request_adapter_failed:
No suitable graphics adapter found;
noop not requested,
vulkan found no adapters,
metal support not compiled in,
dx12 support not compiled in,
gl not requested,
webgpu support not compiled in
```

### 3.2 API 35 对照组已通过

相同 APK 在以下组合中已完成 304-E3：

```text
AVD：Aife_Tower_API35_x86_64
Android：API 35
ABI：x86_64
GPU：host
结果：启动、竖屏、gutter、征兵、部署、出战、怪物移动全部 passed
```

因此 RuntimePackage、项目 RuntimeModule、GameActivity、AUI、输入、玩法和 APK 主体不是本次失败 owner。

### 3.3 16 KB 打包合同已经满足

对当前 APK 的静态检查结果：

```text
zipalign -c -P 16 -v 4：Verification successful
libmain.so ELF LOAD p_align：0x4000
```

`0x4000` 即 16 KB。当前失败不是 ZIP 对齐或 ELF segment 对齐错误，不应通过重写 exporter、升级 Gradle、
修改 NDK 或重建 RuntimePackage 来修复。

### 3.4 当前源码缺口

`rust/crates/runtime_player_winit/src/lib.rs` 的 `RealWindowHost::new` 当前创建 WGPU instance 时固定使用：

```rust
backends: wgpu::Backends::PRIMARY
```

WGPU 26 中 `PRIMARY` 不包含 `GL`。当前依赖默认 feature 已包含 `gles`，但 Android Player 从未请求该
backend；诊断中的 `gl not requested` 与源码完全一致。

## 4. 成熟实现依据与取舍

### 4.1 Android 16 KB 合同

Android 官方 16 KB page-size 指南把 native library 的 ELF LOAD alignment 与 APK 中未压缩 native
library 的 ZIP alignment 作为核心检查项。本项目现有 APK 已分别通过 `llvm-readelf` 的 `0x4000` 和
`zipalign -P 16`，所以本方案只保留这两项作为静态防回归，不重复建设新的 page-size schema。

参考：`https://developer.android.com/guide/practices/page-sizes`

### 4.2 WGPU backend 模型

WGPU 26 把 Vulkan 与 OpenGL/OpenGL ES 作为独立 backend；`InstanceDescriptor.backends` 决定允许枚举的
backend。`GlBackendOptions` 明确支持 OpenGL ES 3.x。当前错误已证明 GLES feature 存在但未被请求，最小
修复应扩大 Android instance 的允许 backend 集，而不是升级整个 WGPU 依赖族。

参考：

```text
https://docs.rs/wgpu/26.0.1/wgpu/struct.InstanceDescriptor.html
https://docs.rs/wgpu-types/26.0.0/wgpu_types/struct.GlBackendOptions.html
```

### 4.3 Unity / Godot / Bevy 的可学习边界

304 已确认成熟引擎通常把 Android 图形 backend、工具链/package 和项目玩法分开管理。可学习点是平台
Player 选择可用 renderer，并把实际选择纳入诊断；不可照搬点是为了一个 adapter 枚举缺口引入完整
PlayerSettings、渲染质量层、插件系统或多 renderer 配置 UI。

本方案仍使用 304 的 GameActivity + Winit + WGPU 主线，不建立 Tower 专用 Android 壳，也不恢复 legacy
TypeScript exporter。

## 5. 方案比较与正式选择

### 5.1 方案 A：升级 WGPU/Vulkan

升级 WGPU、ash 或 Vulkan loader，继续保持 Vulkan-only。当前诊断没有证明 WGPU 26 本身存在缺陷，也没有
证明 Android 17 beta 镜像能够提供可用 Vulkan adapter。该方案会扩大 Cargo lock、renderer 和多平台
回归范围，冻结，不施工。

### 5.2 原方案 B-min：Android Vulkan + GLES fallback（已被真实 Gate B 否证）

Android Player 的 WGPU instance 允许：

```text
VULKAN | GL
```

`request_adapter` 仍只执行一次，并继续绑定真实 surface。WGPU 在允许集合内按现有 power preference 与
adapter device type 规则选择可用 adapter；本方案不声明严格的 Vulkan-first 顺序。Vulkan 不可用而
OpenGL ES 可用时可选择 GLES。Windows 与其它非 Android real-window 路径继续使用 `PRIMARY`。

真实 API 37/16 KB Gate 已证明，单个 multi-backend instance 会使 Vulkan 与 GLES 争用同一个 Android
native window，最终触发 `EGL_BAD_ALLOC` 与 `Surface::configure: Invalid surface`。因此该方案不再可施工。

### 5.3 方案 B-min-R1：Android x86_64 固定 GLES（正式选择）

编译目标策略：

```text
target_os=android && target_arch=x86_64：GL
其它所有目标：PRIMARY
```

x86_64 Android APK 是当前 Emulator dev export；它不同时请求 Vulkan，因此不会发生同一 native window 的
双 backend 连接。ARM64 Android 真机包继续走既有 `PRIMARY`，Windows、Editor 和其它平台也保持不变。
本方案不增加运行时设备探测、项目配置、ABI schema 或两阶段 surface lifecycle。

### 5.4 方案 C：显式 Vulkan 失败后重建 GLES instance/surface

先创建 Vulkan-only instance，失败后销毁并用 GL-only instance 重建 surface。它能保证严格 Vulkan-first
并输出更明确的 fallback 阶段，但增加两个 instance/surface 生命周期、错误合并和资源清理分支。当前 WGPU
已能在一个允许集合中完成 adapter 选择，没有证据要求该复杂度，冻结。

## 6. 正式架构与 owner

### 6.1 唯一行为 owner

图形 backend 允许集合继续由 `runtime_player_winit::real_window::RealWindowHost` 所有。不得把 backend
选择放入 Tower、BuildProfile、RuntimePackage、AUI、GameView target 或项目 RuntimeModule。

平台规则：

```text
Android x86_64 real-window Player：GL
Android ARM64 real-window Player：PRIMARY（保持现状）
Windows real-window Player：PRIMARY（保持现状）
Editor WGPU：不在本方案范围内
Headless Player：不在本方案范围内
```

首版不提供项目侧“强制 Vulkan/GL”开关。该选择属于引擎平台兼容策略，不应让普通项目配置承担驱动差异。

### 6.2 Adapter 选择语义

保持现有一次性链路：

```text
create window
-> create WGPU instance with platform backend set
-> create surface
-> request_adapter(compatible_surface, HighPerformance, no forced fallback)
-> request_device
-> surface config
-> normal Player session
```

不得增加“失败后静默改用 noop/headless renderer”。如果 Vulkan 和 GLES 都不可用，仍生成
`real_window_environment_blocked` 并退出，不能用空白画面伪装成功。

### 6.3 启动诊断

复用现有 `android-startup-diagnostic.v1` 与 `NativeWindowHostReport`，不新增 schema。至少保证：

```text
失败：保留 request_adapter/request_device 的原始 bounded message；
成功：terminal report 可识别实际 backend（Vulkan 或 Gl）；
普通帧：不写逐帧 backend/report；
诊断文件：仍只在启动失败或 terminal summary 边界更新。
```

若既有 `NativeWindowHostReport` 已有可承载 backend 的字段，直接复用；只有缺少该字段且无法从现有 summary
读取时，才允许给 compact terminal report 增加一个非 schema-breaking 的现有字段映射。不得新建 Android
graphics diagnostic family。

### 6.4 16 KB 合同

305 不修改 exporter，但将下列检查作为 Android 17/16K qualification 的前置静态合同：

```text
zipalign -c -P 16 -v 4 <apk>
llvm-readelf -l libmain.so：所有 LOAD p_align >= 0x4000
APK 单 ABI 与目标 AVD ABI 一致
```

若静态合同失败，应归类为 exporter/package failure；若静态合同通过但 adapter 失败，应归类为 Player graphics
compatibility failure。两类问题不得互相替代。

## 7. 最小涉及文件

预期源码范围：

```text
engine-owned：
  rust/crates/runtime_player_winit/src/lib.rs

仅在现有 compact diagnostic 无法证明 backend 时允许：
  rust/crates/runtime_player_android/src/lib.rs

验证/文档：
  305 对应极简施工文档
  一份 run-owned Android 17/16K smoke report
  一份阶段完成记录与必要入口同步
```

明确不修改：

```text
samples/tower_defense_project/**
rust/crates/engine_runtime/**
rust/crates/editor_*/**
Android exporter / Gradle template / NDK lock
Windows Player / MCP / production Editor
真实项目配置
```

如果实施时必须修改上述排除项，说明 B-min 的因果假设不成立，必须停止并回到方案复核。

## 8. 最小验证合同

### 8.1 Owner 级

```text
Android x86_64 backend policy 仅包含 GL；
Android ARM64 与非 Android backend policy 保持 PRIMARY；
Vulkan + GL 都不可用时仍 fail-closed；
既有 Runtime Player 定向测试通过；
Android target cross-check 通过。
```

由于真实 adapter 可用性由 Emulator 图形栈决定，host 单元测试不能替代真实 AVD。施工文档不得为了伪造
GPU adapter 建立大型 mock renderer；平台策略用最小 owner test 锁定，真实红绿结论由第 8.2 节给出。

### 8.2 唯一 Android 17 beta / 16K 红绿链路

使用新的 run-owned root，消费包含 305 的 fresh x86_64 APK：

```text
AVD：Pixel_7 / API 37 beta / x86_64 / 16 KB / host GPU
安装前：校验 APK hash、单 ABI、签名、manifest、zipalign -P 16、ELF 0x4000
启动：GameActivity 在 8 秒观察窗后仍为 top resumed activity
渲染：至少出现一张非空、非纯白 Tower 首帧
诊断：fatal=0，framesCompleted > 0，实际 backend=Gl 或等价可审查证据
最小输入：一次征兵 10 -> 7，证明 render/input/session 已真正进入普通帧
```

本链路就是本缺陷的 red-capable terminal test。旧 APK 已在同一 AVD 稳定产生
`vulkan found no adapters, gl not requested`，因此不需要另建完整 E2E runner。

### 8.3 API 35 防回归

复用同一个新 APK，在既有 `Aife_Tower_API35_x86_64` / host GPU 上只运行：

```text
Open -> 首帧 -> 进程存活 -> Stop
```

不重复 304-E3 的完整 gutter、部署、出战、怪物移动矩阵，因为 305 不修改 presentation、input 或 gameplay。

### 8.4 不要求的验证

```text
不运行 Local CI；
不运行完整 Tower E3/E4；
不运行 Windows/Editor 视觉矩阵；
不替换 production Editor/Player/MCP；
不要求 ARM64 真机重测；
不要求 Android 17 多 GPU、多厂商或所有 beta 镜像矩阵。
```

## 9. 失败边界

```text
GL backend 未编入 Android artifact：修 runtime_player_winit dependency feature，不升级 WGPU；
GL adapter 仍不可用：保留完整 request_adapter message，停止并复核，不进入 renderer 重构；
adapter 成功但 request_device 失败：只诊断 required limits/features，不修改 Tower；
device 成功但 surface config/present 失败：将 owner 收敛到 surface format/present mode，再单独讨论；
16 KB 静态检查失败：归类 exporter/package，不以 graphics fallback 掩盖；
API 35 回归失败：停止交付，不扩大到完整 E3，先定位 backend policy 或 artifact identity。
```

任何失败都不得自动触发 WGPU 大版本升级、Gradle/NDK 重装、RuntimePackage 重建体系、完整 Android 发布系统
或多平台 renderer 重构。

## 10. 明确不做

```text
不升级 WGPU、winit、ash、NDK、Gradle 或 Android Gradle Plugin；
不实现项目可配置 graphics API；
不实现 Vulkan-only 强制模式或运行时设置 UI；
不实现两阶段 instance/surface 重建；
不实现 noop/headless fallback；
不修改 Tower gameplay、AUI、字体、输入或 presentation；
不修改 Editor、Windows Player、ARM64 默认导出或 production 安装态；
不把 Android 17 beta 扩张成所有未来 Android/GPU 的发布资格声明；
不运行 Local CI、完整 E2E 或全设备矩阵。
```

## 11. 施工建议

已完成的极简施工只有一个施工窗口：

```text
Gate A：
  锁定 Android/非 Android backend policy；
  实施 Android x86_64 GL-only，其余目标 PRIMARY；
  复用/补足 compact backend diagnostic；
  owner test + Android cross-check。

Gate B：
  新 fresh root 导出一次 x86_64 APK；
  16 KB/ABI/signature/manifest/hash 静态校验；
  Android 17 beta/16K Open/首帧/征兵 smoke；
  API 35 Open/首帧/存活回归；
  完成记录与归档。
```

若施工文档把本修复扩张为多个窗口、WGPU 升级、完整 E3/E4、production replacement 或跨平台矩阵，应在
自审时删除这些内容，而不是保留为“更保险”的 Gate。

## 12. 方案自审

### 12.1 是否命中已确认首因

是。启动诊断明确给出 `vulkan found no adapters, gl not requested`；B-min 唯一行为变化就是让 Android
instance 请求已编译的 GL backend。

### 12.2 是否把 16 KB 与图形问题混淆

否。APK 已通过 ZIP 16 KB 对齐和 ELF `0x4000` 对齐。方案保留静态合同，但不修改 exporter。

### 12.3 是否保持 Tower 外部项目边界

是。Tower 只作为真实 consumer，不修改项目文件；平台 backend policy 完全位于引擎 Android Player owner。

### 12.4 是否过量施工

否。方案不升级依赖、不新增 schema、不建立第二套 surface lifecycle、不运行完整历史矩阵。实现预期只修改
`runtime_player_winit` 一个 owner；`runtime_player_android` 仅在既有诊断无法证明 backend 时允许最小映射。

### 12.5 验证是否经济且能捕获真实失败

是。Android 17 beta/16K 同设备、同启动路径已经稳定红；修复后以同路径的进程存活、真实首帧、backend
证据和一次征兵形成绿色终点。API 35 只做启动防回归，不重复 304-E3 已证明的玩法矩阵。

### 12.6 权限与状态

```text
方案结论：通过
正式方案：已生成并自审
施工文档：已完成并归档
当前施工授权：无
引擎源码修改授权：仅限同一 runtime_player_winit owner
APK 重导出/安装/模拟器 smoke：已授权，范围以修订后的 305 施工文档为准
完成结果：fresh API 37/16 KB 红绿与 API 35 最小回归均 passed；下一步等待用户指定
```
