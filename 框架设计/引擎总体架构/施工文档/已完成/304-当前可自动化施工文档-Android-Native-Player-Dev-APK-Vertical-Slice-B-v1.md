# 304 当前可自动化施工文档：Android Native Player Dev APK Vertical Slice B v1

## 1. 状态与授权

```text
系统编号：304
正式方案：../../304-Android-Native-Player-Dev-APK-Vertical-Slice-B-v1方案.md
状态：源码 Gate A-C 已完成并归档；真实 APK/设备 Gate 环境阻塞
授权：生成极简施工文档并直接施工
```

目标是在 Rust 正式主线建立通用 `arm64-v8a` Android debug APK 纵切，并让 Tower 仅作为普通消费者接入。
本授权不包含安装 JDK/SDK/NDK/Gradle/Rust target、接受许可证、ADB 设备操作、Local CI、真实配置修改，
也不替换 production Editor/Player/MCP。

## 2. 必要性与验证经济性自审

只施工以下必要 owner：Android BuildProfile、共享 Player 的 lifecycle/touch 契约、通用 Android exporter
source contract、Tower `android.dev.json`。优先纯函数和 host-side 定向测试；缺少外部工具链时 preflight 必须
fail-fast，不能用重复构建或模拟成功代替真实 APK。

明确不做：AAB、release signing、multi-ABI、iOS、商店发布、完整设备矩阵、legacy exporter 恢复、Renderer/
AUI/RuntimePackage 大重构、完整 E2E、Local CI、production 更新和自动安装工具链。若实现要求突破这些边界，
立即停止并回到方案复核。

## 3. Gate A：Profile 与 Host 契约

```text
允许 build-profile.v1 target=android，其他未知 target 继续 fail-closed；
显式 profile path 决定 source mapping，不再写死 windows.dev.json；
增加 primary touch Started/Moved/Ended/Cancelled 到现有 RawInputEvent 的确定性映射；
增加 Android suspended/resumed host 状态合同，确保 suspend 停帧、resume 重建 surface 而不重建 gameplay session。
```

验证：只运行被修改 crate 的精确 owner tests。

## 4. Gate B：Android Exporter Source Contract

```text
新增窄 AndroidDevExportPipeline 与结构化 request/report/manifest；
先探测 JDK 17、SDK 35、NDK r28b、Gradle wrapper 与 aarch64-linux-android target；
生成 GameActivity launcher/Gradle staging 合同，build-time 链接选定 project RuntimeModule；
生成并校验 RuntimePackage asset manifest，目标为 digest-addressed app internal materialization；
只有 toolchain 完整时才执行 native cross-build、assembleDebug、APK verify 与 atomic publish。
```

验证：host-side deterministic tests 覆盖 preflight、launcher/manifest/staging 和错误边界。本机工具链缺失时只记录
一次 environment-blocked 结果，不安装、不循环重试、不声称 APK 已生成。

## 5. Gate C：Tower Consumer 与终态

```text
新增 samples/tower_defense_project/BuildProfiles/android.dev.json；
用显式 profile 执行 Tower assembler consumer test；
执行一次有界 Android toolchain preflight；
记录源码 Gate 与 package/device qualification 的独立状态；
同步索引、完成记录并归档。
```

真实 APK、安装、触摸与 background/resume smoke 只有工具链和设备授权同时存在时才运行；否则保留为明确后续入口。

## 6. 完成条件

```text
Gate A-C 的 host-side owner tests 通过；
Tower Android profile 能被正式 assembler 消费；
exporter 缺失工具链时给出稳定、可行动且无副作用的诊断；
若工具链不完整，packageStatus 不得为 success，deviceQualification 必须为 notRun；
施工记录准确说明 APK 是否真实生成。
```

## 7. 激活前复核与自审结论

2026-08-20 复核：正式方案有效；当前施工槽和待执行队列为空；相关代码仍为 Windows-only 基线；Tower
RuntimeModule ABI 可复用；工作树存在大量既有变更，施工将使用窄 patch 保留它们。三道 Gate 足以证明本轮
最小纵切，没有重复 qualification 或扩大发布系统范围。结论：通过，允许直接施工。

## 8. 完成结果

```text
Gate A：通过。Android dev profile/source mapping、primary touch、suspend/resume owner tests 通过。
Gate B：通过。通用 exporter、GameActivity launcher、asset materialization、Gradle/ARM64/APK/signing 合同完成。
Gate C：Tower android.dev consumer 通过；最终 preflight 为 environmentBlocked。
packageStatus：notBuilt
deviceQualification：notRun
```

阻塞项为 JDK 17、SDK/build-tools 35、NDK r28b、Gradle bootstrap 与 Rust Android target 均未安装。本轮没有
创建 Tower Android output/staging，没有运行 Local CI，没有修改真实配置，没有替换 production 二进制，也
没有执行 ADB。源码施工已按授权闭环；真实 APK build/device smoke 必须在工具链获得单独授权后另开有界 Gate。
