# 308 post-306 Tower ARM64 APK Real-device Smoke v1 施工文档

状态：Gate A passed；Gate B `user-manual-acceptance passed`；308 已完成，待归档。

## 目标与范围

让 306 GL 多页字体修复进入一个新的 Tower ARM64 dev APK，校验单 ABI、签名、manifest、16 KB 与 hash；
随后只在一个真实 Android 设备上验证启动、竖屏、中文可读性、征兵 10→7、部署、出战和怪物移动。

继承 304 Android exporter/toolchain 与 306/307 字体证据。不修改源码、Tower、真实配置或 production
Editor/Player/MCP；不运行 Local CI、x86_64、Emulator、完整玩法矩阵或多设备矩阵。

## 必要性与验证经济性审计

```text
requested claim：post-306 ARM64 APK 在真实手机可启动、中文可读且关键输入/玩法链可用
minimum work：一次 fresh ARM64 export + 静态校验 + 一台真机的一次 smoke
Gate A -> artifact 未消费 306/ABI 或 16 KB 错配 -> APK ABI/signature/manifest/alignment/hash
Gate B -> Emulator 结论无法外推到 ARM64 真机 -> install/launch/screenshot/logcat + 4 个关键动作
reused evidence：306 owner red/green、307 x86_64 API 35/37 中文截图、304/305 锁定工具链
invalidation：engine/runtime/export input、APK identity 或目标设备安装态改变
estimated cost：export 5-15 分钟；真机 5-10 分钟；无 ADB 设备则 Gate B=deviceBlocked
```

## Gate A：fresh ARM64 export 与静态校验

1. 创建 run-owned root 与严格隔离的 fresh Tower 副本。
2. 复制既有 Gradle seed 到新的 run-owned Gradle home；激活 `android-304-v1` process-local 环境和短 TEMP。
3. 唯一执行一次 ARM64 默认 `export_android_dev <fresh-root>`。
4. 保留 APK/report/log，校验仅含 `lib/arm64-v8a/libmain.so`、v2 签名、portrait manifest、ZIP 16 KB、
   ELF `LOAD Align=0x4000` 与 SHA-256。

## Gate B：单台真机关键 smoke

1. ADB 只接受一台状态为 `device` 的非 Emulator 设备；否则记录 `deviceBlocked`，不伪造验收。
2. 安装 Gate A APK，cold launch，确认 top resumed、竖屏、进程存活且无 fatal/surface conflict。
3. 保存中文可读性截图并人工复核方向、完整字形和 atlas 碎片。
4. 真实点击征兵并观察军粮 10→7；部署、确认出战，观察怪物连续移动。
5. 保留 logcat、截图和报告；不扩大到完整十轮或性能矩阵。

Gate A 唯一 export 295.398 秒完成 8/8 stages；37,967,541-byte APK 的 SHA-256 为
`787A6CAC...23132`，单 ARM64 ABI、v2 签名、portrait、ZIP/ELF 16 KB 均通过。ADB 自动化阶段没有发现
设备；随后用户在真实手机上手动安装并验收同一 APK，确认启动、竖屏、中文可读、征兵 10→7、部署、出战
和怪物移动均正常，因此 Gate B 以 `user-manual-acceptance passed` 关闭。未采集 ADB、Logcat 或截图证据。

## 完成与清理

Gate A-B 均已通过；完成记录、入口同步与归档在本轮一次完成。fresh 项目、Gradle cache 和 TEMP 已按严格
cleanup proof 删除；静态 evidence 与 package manifest 保留，验收后 APK 已不在 Gate A 记录的原路径。

## 自审与激活前复核

```text
新方案：无；只消费 304/306 已完成能力
过量施工：无新 schema/runner/源码修改/完整矩阵
当前槽：无其它 active 施工文档；308 已激活
外部状态：只安装到用户授权的一台真机；当前 preflight 暂无 ADB device
```
