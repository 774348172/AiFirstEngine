# 304-E1 Android x86_64 Emulator Dev Export Minimal v1

状态：已完成并归档。用户于 2026-08-22 明确授权；Gate A-B 源码与定向验证均通过。

## 目标

在保持默认 ARM64 手机导出完全兼容的前提下，新增显式、单 ABI 的 x86_64 Android Emulator debug
APK 出口，使 Android Studio Emulator 可以安装 Tower 包并通过 ADB/Logcat 调试。

## 必要性与验证经济性审计

```text
requested claim：引擎支持 x86_64 Android Emulator dev export
minimum change：RuntimePackage target allowlist + Android exporter typed ABI + CLI selector
must change：engine_runtime/runtime_package_builder.rs、editor_core/android_export.rs、export_android_dev.rs
excluded：multi-ABI、其它 ABI、AVD/system image 安装、Local CI、production、Tower 真实配置
Gate A -> target identity 错配 -> owner unit test -> 两个 Android target 可接受、未知 target 拒绝
Gate B -> APK/linker/Gradle ABI 错配 -> exporter unit test -> ARM64 默认与 x86_64 显式输出一致
estimated cost：源码与定向测试 30-60 分钟
stop condition：若需要 Renderer/Winit/GameActivity 重构或 RuntimePackage schema 变更，立即停止复核
```

## Gate A：RuntimePackage target

完成：增加 `android-x86_64-dev`；未知 target 保持 `UnsupportedTarget` fail-closed；未改变 package schema。

验证：RuntimePackage target owner test 1/1 passed。

## Gate B：Android exporter typed ABI

完成：typed ABI 同时驱动 RuntimePackage target、Rust/NDK target、Cargo env、JNI path、Gradle ABI filter、
manifest ABI、APK 单 ABI 校验与 output root。CLI 默认 ARM64，显式 `--abi x86_64` 选择模拟器。

验证：`android_export` owner tests 7/7、CLI tests 2/2、受影响格式检查与 exporter build 均通过。

## 完成状态

源码能力为 `sourceAvailable`。真实 x86_64 preflight 因 Rust `x86_64-linux-android` target 未安装而
`environmentBlocked / notBuilt / notRun`，且未创建 staging/output。按本施工范围未自动安装工具链、生成
APK 或启动模拟器。完成证据见
`../../阶段完成记录/2026-08-22-Android-x86_64-Emulator-Dev-Export-Minimal-v1/00-总览.md`。
