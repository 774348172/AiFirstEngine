# 307 post-306 Tower x86_64 APK Chinese Smoke v1 施工文档

状态：已完成并归档；Gate A-B passed。

## 目标与范围

让 306 的 GL 多页字体修复进入一个新的 Tower x86_64 dev APK，并只验证两档 Emulator 的启动存活、
GL backend、无 fatal/surface conflict 与中文可读性截图。

不修改源码、Tower、真实配置或 production Editor/Player/MCP；不运行 Local CI、完整 304-E3/E4、玩法输入矩阵、
ARM64 export 或真机验收。复用已锁定 `android-304-v1` 工具链、305 Gradle seed 与现有两个 AVD。

## 必要性与验证经济性审计

```text
requested claim：306 修复进入 fresh x86_64 APK，API 35/API 37 中文可读
minimum work：一次 fresh export；同一 APK 在两 AVD 各一次启动和截图
Gate A -> artifact 未消费 306/ABI 错配 -> export report + APK hash/ABI/manifest
Gate B -> GL 设备文字仍损坏 -> API 35/API 37 截图 + logcat + top-resumed
reused evidence：306 owner red/green；305 工具链、Gradle seed、AVD 与 presentation/input 证据
invalidation：engine/runtime/export input 或 APK identity 改变
estimated cost：export 5-15 分钟；两 AVD 10-20 分钟；新 owner 失败则保留首因并停止
```

## Gate A：fresh x86_64 export

1. 创建 `G:\AifeAndroidRuns\307-*` 与严格子目录 fresh Tower 副本。
2. 排除 Build、AndroidExport、AiCapability delivery 与 Cargo target；保留项目 source/cooked 输入。
3. 复制 305 Gradle seed 到新的 run-owned Gradle home，并在 process-local 环境激活锁定工具链。
4. 唯一执行一次 `export_android_dev <fresh-root> --abi x86_64`。
5. 保留 APK、export/package manifest、SHA-256；校验 APK 只含 `lib/x86_64/libmain.so`、签名和 manifest。

首次 export 在 `rustNative` 后因锁定 JDK 使用含 `~` 的宿主 `TEMP/TMP` 创建 AF_UNIX selector pipe 而失败：
`Invalid argument: connect`。同一 JDK 在 run-owned 无 8.3 路径的短 TEMP 中连续 100 次 selector probe 通过。
允许清理首次失败的 owned staging，并仅以 process-local `TEMP/TMP=<runRoot>/temp` 重试一次 exporter；不改
系统环境、toolchain lock、Gradle 工程或产品源码。

## Gate B：API 35/API 37 中文截图 smoke

对 `Aife_Tower_API35_x86_64` 与 `Pixel_7` 顺序执行：启动 owned Emulator、安装同一 APK、cold launch、等待
8 秒、确认 top resumed、截屏、保存 logcat，检查 fatal/surface conflict 与 GL backend。截图人工核对主界面
标题、资源栏、按钮和棋盘中文不是 atlas 碎片且方向正确。每档完成后停止本轮 owned Emulator。

## 完成与归档

生成 run-owned 总报告与阶段完成记录；同步 49/54/两个 README，归档本文档。fresh 项目副本在保留 APK、
manifest、日志和截图后按严格 cleanup proof 删除。

完成结果：43,670,918-byte x86_64 APK（SHA-256 `22C78C00...CD119`）通过 ABI、签名、manifest 与 hash
校验；API 35 与 API 37/16 KB 中文截图均可读、正序且无 atlas 碎片。fresh 项目、run-owned Gradle cache、
TEMP 和 owned Emulator 已清理，artifact/evidence/logs/screenshots 已保留。

## 自审与激活前复核

```text
方案：继承 306 已确认 B-min，无新设计
范围：一次 artifact + 两档文字 smoke，无 gameplay/production 扩张
过量施工：无新 runner/schema/transaction/完整矩阵
当前槽：空；307 已激活
外部状态：只安装到 owned Emulator；顺序执行并停止 owned 进程
```
