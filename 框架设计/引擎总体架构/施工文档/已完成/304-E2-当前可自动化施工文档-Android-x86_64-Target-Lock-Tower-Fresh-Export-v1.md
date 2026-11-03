# 304-E2 Android x86_64 Target Lock + Tower Fresh Export v1

状态：Gate A-B 已完成；x86_64 APK、证据保留与 run-owned cleanup 已通过并归档。

## 目标与边界

在既有 `G:\AifeToolchains\android-304-v1` 中安装并记录 x86_64 Android Rust target，再从新的
run-owned Tower 副本执行一次合格的 `--abi x86_64` dev export。项目 RuntimeModule 使用仓库层级相对 Cargo
依赖，因此合格项目根位于 `G:\gameEngin\samples\__304_e2_*`；外部 run root 保存证据和最终 artifact。

不修改引擎源码、真实 Tower 配置/cache/output、production Editor/Player/MCP、持久环境变量或 ARM64 产物；
不创建 AVD、不运行 APK/ADB/Local CI、不进入设备交互验证。

## 必要性与验证经济性审计

```text
requested claim：安装并锁定 x86_64 Rust target，生成一次 fresh Tower x86_64 dev APK
minimum change：rustup target add + toolchain lock/activation/verification metadata + one run-owned export
Gate A -> target/lock 漂移 -> rustup installed readback + lock parse/hash + process-local activation probe
Gate B -> ABI/artifact 错配 -> exporter report + package manifest + APK entries/signature/hash verification
reused evidence：304-E1 source/CLI tests；只有 E1 source hash 改变才失效
estimated cost：10-30 分钟；若下载/构建超过有界等待或出现产品源码缺口则停止并保留首因
```

## Gate A：安装与锁定

1. 备份现有 `toolchain-lock.json`、`activate.ps1`、`verification-report.json` 到 run-owned evidence。
2. 为 `stable-x86_64-pc-windows-msvc` 安装 `x86_64-linux-android`。
3. 保留既有 ARM64 `target` 字段，新增 emulator target 记录；activation 同时检查 x86_64 API 26 linker。
4. 真实 readback Rust target、NDK linker、lock JSON 与 process-local activation，并记录新 hash。

## Gate B：fresh Tower x86_64 export

1. 复制真实 Tower 的 272 个权威输入到 run-owned samples 项目；只包含三份必需 AssetPipeline identity，排除
   其它 `Library`、`.aife`、`Build`、RuntimeModule target 与 Tests 生成物。
2. 校验源与副本逐文件 SHA-256 一致。
3. 使用已验证 E1 exporter 和 process-local locked environment 执行一次 `--abi x86_64`。
4. 校验 success report、独立 output、package ABI、唯一 `lib/x86_64/libmain.so`、APK 签名与 SHA-256。
5. 保留 APK、manifest、report、输入清单与验证结果；检查构建子进程退出。

## 完成

Gate A-B 通过后写一次完成记录，更新入口并归档本文档。若合格路径 Gate B 失败，不修改产品源码，不自动重试或扩展。

2026-08-23 单独授权的 cache-seeded retry 已完成：旧 304-T2 Gradle cache 经全量 SHA-256 清单校验后，
3981 个非 volatile 文件复制到新 run-owned cache 并逐文件一致；fresh Tower 仍使用 272 个 byte-exact 输入。
唯一一次 `--abi x86_64` export 在 306.162 秒内完成 8/8 stages，最终 APK 只含
`lib/x86_64/libmain.so`，签名、Android manifest、package/runtime manifests 与 hash 校验通过。

最终 APK 保留于
`G:\AifeAndroidRuns\304-E2-retry-20260823-125651\artifacts\TowerDefense-x86_64-debug.apk`，
SHA-256 为 `24C562691C2D8809FE6EFCB6AFCADD4D62F1C7DB35C7BC7ADDC86AF8D6A6A93D`。
旧失败项目与本轮 fresh 副本合计清理 5,916,279,035 bytes；旧失败 run evidence 与本轮 run root 均保留。

## Gate B harness 复核记录

首次调用使用外部 `G:\AifeAndroidRuns\304-E2-*\project`，在 `rustNative` 前因 Tower RuntimeModule 的
`../../../rust` 相对依赖解析到不存在的 `G:\AifeAndroidRuns\rust` 而失败；preflight 全绿、完成到 launcher，
没有 output/APK。该根不满足既有 path-affine project composition，属于 ineligible harness，不是产品失败。
后续只允许按 304-T2 已验证的 `samples/__304_e2_*` 层级创建一次 replacement fresh project；若该调用失败即停止。

## 最终结果

Gate A passed：`x86_64-linux-android` 已安装到 `stable-x86_64-pc-windows-msvc`，rustlib 145,284,914
bytes；NDK clang readback 为 `x86_64-unknown-linux-android26`。新 lock SHA-256 为
`734ABDF4...D453D`，activation SHA-256 为 `08263F2F...BFCC`，前后元数据均已备份。

Gate B passed：cache seed、fresh input、export、ABI、签名、manifest、hash、artifact retention 与 cleanup
全部通过。package manifest 的 `nativeLibrarySha256` 正确绑定 Cargo/JNI 未 strip 库；APK entry 另绑定
Gradle stripped 库哈希，二者不混作同一身份。未运行 APK/ADB/AVD、Local CI，未修改引擎源码、真实 Tower、
持久环境变量或 production/安装态二进制。
