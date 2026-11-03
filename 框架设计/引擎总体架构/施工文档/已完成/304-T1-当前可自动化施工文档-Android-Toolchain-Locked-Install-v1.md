# 304-T1 当前可自动化施工文档：Android Toolchain Locked Install v1

## 状态与授权

```text
父方案：304 Android Native Player Dev APK Vertical Slice B v1
状态：已激活，施工中
授权：安装并锁定 Android 工具链
外部 owner root：G:\AifeToolchains\android-304-v1
Rust owner：stable-x86_64-pc-windows-msvc / rustc 1.96.0
```

本 Gate 只安装 JDK 17、Android command-line tools、platform/build-tools 35、NDK 28.1.13356709、Gradle
8.11.1 和 `aarch64-linux-android` Rust target。接受 license 仅限这些锁定 Android SDK 包。不得运行 APK
export、ADB、Local CI，不得替换 production Editor/Player/MCP，不得修改 Tower cache 或真实项目配置。

## 必要性与验证经济性

```text
requested claim：消除 304 preflight 的五个工具链 blocker，并锁定可复现调用环境。
minimum change：新建唯一外部 toolchain root；为当前 Rust toolchain 增加一个 target；生成 lock/env 文件。
excluded：Android Studio、模拟器、系统 PATH/JAVA_HOME/ANDROID_HOME 持久化、APK build、device smoke。
Gate T-A：下载身份/哈希 -> 官方 metadata/checksum 与下载文件 SHA-256。
Gate T-B：安装/SDK license -> 精确版本命令回读与文件存在性。
Gate T-C：锁定/消费 -> process-local env 下 304 CLI preflight ready=true。
estimated cost：4-8 GiB，30-90 分钟；超出 12 GiB 或两次同源下载失败即停止复核。
```

旧证据 `environmentBlocked/notBuilt/notRun` 只因工具链身份变化而失效；304 源码 owner tests、Tower profile
consumer 与 production 状态均不失效，不重跑。

## Gate T-A：解析并下载精确分发物

```text
JDK：Temurin 17 x64 Windows JDK，API 解析一次后冻结 exact version/url/checksum。
Android CLI：Google commandlinetools Windows exact revision，记录 URL/SHA-256。
Gradle：8.11.1 binary distribution，使用官方 SHA-256。
```

所有下载进入 `<root>\downloads`，逐项 hash 通过后才解压。

## Gate T-B：安装锁定组件

```text
JDK/Gradle/command-line tools 解压到版本目录；
使用 process-local JAVA_HOME/ANDROID_SDK_ROOT 安装 platforms;android-35、build-tools;35.0.0、
ndk;28.1.13356709；
只接受上述安装所需 licenses；
rustup target add aarch64-linux-android --toolchain stable-x86_64-pc-windows-msvc。
```

## Gate T-C：锁与最终 preflight

生成 `<root>\toolchain-lock.json` 与 `<root>\activate.ps1`。激活脚本只修改调用进程环境，不写用户/系统
环境变量。验证 Java、sdkmanager、Gradle、apksigner、NDK clang、Rust target 的 exact identity，并在相同
process-local 环境运行一次 304 Tower preflight；终态必须为 `ready=true`，仍不得进入 APK build。

## 自审

当前施工槽无其它正式文档，目标根不存在，41.23 GiB 可用空间满足估算。方案未扩大到 Android Studio、
emulator、ADB 或 APK qualification；验证只覆盖安装身份和 304 preflight。结论：通过，允许施工。

## 完成结果

2026-08-20 已在唯一外部 owner root `G:\AifeToolchains\android-304-v1` 完成安装和锁定：

```text
Temurin JDK：17.0.20+8
Android command-line tools：23.0 / archive 16111833
Android platform：35
Android build-tools：35.0.0
Android NDK：28.1.13356709 / r28b
Gradle：8.11.1
Rust toolchain：stable-x86_64-pc-windows-msvc / rustc 1.96.0
Rust target：aarch64-linux-android
```

下载归档的 SHA-256 全部复核通过；`toolchain-lock.json`、`activate.ps1` 与
`verification-report.json` 已写入工具链根。激活脚本只修改调用它的 PowerShell 进程，用户/系统级
`JAVA_HOME`、`ANDROID_HOME`、`ANDROID_SDK_ROOT`、`ANDROID_NDK_HOME` 与 `GRADLE_HOME` 均保持未设置。

304 consumer 源码使用的五类判定全部通过，`consumerProbeContract.ready=true`。现有
`export_android_dev` 没有 preflight-only 模式，ready 后会直接进入 APK 构建，因此为遵守本 Gate 禁止
APK export 的边界，没有调用该 CLI，也没有伪造导出成功。未运行 APK build、ADB、Local CI，未修改
Tower cache/真实配置，未替换 production Editor/Player/MCP。Gate T-A 至 T-C 完成，允许归档。
