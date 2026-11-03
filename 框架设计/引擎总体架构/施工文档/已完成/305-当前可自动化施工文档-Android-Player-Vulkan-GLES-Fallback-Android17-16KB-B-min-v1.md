# 305 Android x86_64 Player GLES Policy + Android 17/16KB B-min-R1 v1 施工文档

状态：B-min-R1 Gate A-R1/B-R1 已完成，文档待归档。

## 目标与边界

修复已确认的 Android Player adapter/surface 缺口：Android x86_64 WGPU instance 只允许 `GL`，Android
ARM64 与其它平台保持 `PRIMARY`。复用既有启动诊断记录实际选中的 backend，并用一份 fresh Tower
x86_64 APK 完成 Android 17 beta/API 37/16 KB 红绿验收与 API 35 最小防回归。

唯一主要源码 owner 是 `rust/crates/runtime_player_winit/src/lib.rs`。只有既有 compact terminal report
无法消费现有诊断时才允许最小修改 `rust/crates/runtime_player_android/src/lib.rs`。Tower 仅作为外部 consumer，
不修改项目文件、真实配置或真实缓存；不升级 WGPU/winit/NDK/Gradle，不改 Editor/Windows/ARM64，不运行
Local CI，不替换 production Editor/Player/MCP，不重跑完整 304-E3/E4。

## 必要性与验证经济性审计

```text
requested claim：让合格 x86_64/16 KB Tower APK 在 API 37 ps16k AVD 上能通过 GLES adapter 启动
confirmed failure：request_adapter 报 vulkan found no adapters, gl not requested
minimum causal change：runtime_player_winit 仅对 Android x86_64 选择 GL，其余目标保持 PRIMARY
minimum files：runtime_player_winit/src/lib.rs；android compact report 仅在不能复用 diagnostics 时才改
excluded/deferred：依赖升级、两阶段 surface 重建、项目 graphics 配置、完整设备矩阵、production 替换
Gate A -> Android 未请求 GL/非 Android 行为漂移 -> policy owner test + Android target check -> source evidence
Gate B -> 真实 API 37 仍无 adapter/16 KB artifact 错配 -> fresh APK + 同 AVD red-capable smoke -> APK/日志/截图/report
reused evidence：304-E3 的 Tower presentation/input 与 API 35 完整玩法证据；源码或 artifact 输入改变才失效
estimated cost：Gate A 5-15 分钟；Gate B 15-40 分钟；export/AVD 超出有界预算或进入新 owner 即停止复核
```

不新增 schema、migration、runner、transaction 或完整矩阵。每个 Gate 只有一项因果风险和能将其打红的最低成本证据。

## Gate A：Android backend policy 与启动诊断

1. 在 `runtime_player_winit` 中建立可测试的最小 backend policy：Android 为 `VULKAN | GL`，非 Android 为
   `PRIMARY`；真实 instance 只调用一次 `request_adapter`，不声明严格 Vulkan-first。
2. adapter 成功后把实际 backend 作为一条启动级 Info diagnostic 写入既有 `NativeWindowHostReport`；Android
   compact terminal report 继续消费既有 diagnostics，不新增 report family，不逐帧写盘。
3. 增加 owner test，锁定 Android 与非 Android policy；保持 Vulkan/GLES 都不可用时的 fail-closed 行为。
4. 执行：

```powershell
cd G:\gameEngin\rust
cargo fmt --check -p runtime_player_winit -p runtime_player_android
cargo test -p runtime_player_winit --features real-window player_wgpu_backends
cargo test -p runtime_player_android compact_terminal_report
# 使用 G:\AifeToolchains\android-304-v1 的 process-local activation
cargo check -p runtime_player_android --target x86_64-linux-android
```

若 Android target 中 GL feature 未编入，才允许修正 `runtime_player_winit` 自身 feature wiring；不得升级依赖。

## Gate B：fresh APK 与两档 AVD 最小验收

1. 在 `G:\AifeAndroidRuns\305-*` 新建 run-owned root；复用锁定 Android 工具链与已验证 Gradle cache，创建
   fresh Tower 项目副本并执行一次 `export_android_dev <fresh-tower-root> --abi x86_64`。
2. 保留 APK、export report、package manifest 与 SHA-256；校验签名、manifest、仅有
   `lib/x86_64/libmain.so`、`zipalign -c -P 16 -v 4` 通过、ELF LOAD `p_align >= 0x4000`。
3. 在 `Pixel_7` / API 37 beta / x86_64 / `google_apis_playstore_ps16k` / host GPU 安装并启动：8 秒后仍为
   top resumed、首帧非空非纯白、fatal=0、`framesCompleted > 0`、backend 为 `Gl` 或等价证据；真实点击一次
   征兵并确认 10 -> 7。
4. 同一 APK 在 `Aife_Tower_API35_x86_64` 只做 Open、首帧、8 秒存活、Stop；不重复部署/出战/怪物矩阵。
5. 保存 logcat、截图、静态检查、设备/package identity 与总结 report；终止本轮 owned emulator/app 进程，
   不删除用户已有 AVD。

若 API 37 的 adapter 成功但失败点进入 request_device/surface present，新 owner 不在本文档授权内：保留首因并停止，
不得扩张 renderer 或工具链施工。

## 2026-08-26 Gate 结果

Gate A passed：Android policy 已实现为 `VULKAN | GL`，非 Android 保持 `PRIMARY`；实际 backend 复用既有
Info diagnostic。owner test、`runtime_player_android` 4 tests、格式检查和 x86_64 Android target check
通过；target check 明确编译了 WGPU `vulkan` 与 `gles`。

Gate B failed：唯一 fresh export 8/8 stages passed，APK 静态合同全绿；APK SHA-256 为
`76AB89E53B7C3AD85340C4963FC3B6D9D430BC9E1AD32AC4B8CCFC376BF295AA`。同一 APK 安装到
`Pixel_7` API 37/16 KB 后，日志证明 GLES 已被请求并打开 `libGLESv2_emulation.so`，但 Vulkan 与 GL
在同一 multi-backend instance 中先后连接同一个 Android native window：

```text
BufferQueueProducer: connect: already connected
eglCreateWindowSurface: EGL_BAD_ALLOC
wgpu Surface::configure: Invalid surface
```

这推翻了 B-min “单 instance 的 `VULKAN | GL` 可安全形成 fallback”的施工前提。API 35 回归按 fail-fast
未运行；不得把当前 APK或 Gate B 标记为合格，也不得未经方案修订改成 GL-only 或两阶段 instance/surface。

证据保留在 `G:\AifeAndroidRuns\305-bmin-20260826-000929`。本轮 owned AVD 已关闭，fresh Tower 副本已在
严格 cleanup proof 后删除约 3.27 GB；Local CI、production replacement、Tower 修改均未执行。

## 完成与归档

Gate A-B 通过后一次性写阶段完成记录，更新 `49`、`54`、施工文档 README、阶段完成记录 README，并把本文档
移动到 `施工文档/已完成/`。明确记录未运行 Local CI、未修改 Tower/真实配置、未替换 production 二进制。

## B-min-R1 修订 Gate

### Gate A-R1：x86_64 GL-only policy

1. 将 owner policy 收敛为 `Android && x86_64 -> GL`，其余组合 `PRIMARY`；仍只创建一个 instance/surface。
2. 更新 owner test，覆盖 Android x86_64、Android ARM64、非 Android x86_64 三种组合。
3. 运行受影响格式、定向 owner test、`runtime_player_android` tests 与 x86_64 Android target check。

### Gate B-R1：fresh artifact 与最小真实回归

1. 新 run-owned root 中执行一次 fresh x86_64 APK export，不复用原 B-min APK资格结论。
2. 重做 ABI、签名、manifest、hash、ZIP 16 KB 与 ELF `0x4000` 静态合同。
3. API 37/16 KB 验证 Open、非空首帧、8 秒存活、GLES backend 证据与征兵 10 -> 7。
4. 同一 APK 在 API 35 只做 Open、首帧、8 秒存活与 Stop。
5. 失败继续按首因 fail-fast；不进入两 instance lifecycle、依赖升级或完整 E3/E4。

## 施工文档自审

```text
正式方案一致性：已修订；B-min-R1 采用 Android x86_64 GL-only，旧 B-min 仅作失败证据
范围：通过；一个主要 source owner，Tower 只作 consumer
必要性：通过；直接命中 gl not requested，不升级依赖或重建 surface lifecycle
验证经济性：通过；一个真实红绿 AVD + 一个最小 API 35 启动回归，不重跑完整 304-E3/E4
诊断：通过；复用既有 report diagnostics，不新增 schema/family/逐帧 report
外部状态：通过；仅 run-owned export/evidence 与已有 AVD 的安装 smoke，不改真实配置或 production 二进制
激活前复核：2026-08-26 已检查正式方案、唯一活动槽、重叠源码 diff、测试入口与禁止项；通过
```

## B-min-R1 完成结果

Gate A-R1 passed：`runtime_player_winit` 的 backend policy 已收敛为 `Android && x86_64 -> GL`，Android
ARM64 与其它目标继续使用 `PRIMARY`；仍只创建一个 WGPU instance/surface。以下受影响验证均通过：

```text
cargo fmt --check -p runtime_player_winit -p runtime_player_android
cargo test -p runtime_player_winit --features real-window player_wgpu_backends_uses_gl_only_for_android_x86_64
cargo test -p runtime_player_android
cargo check -p runtime_player_android --target x86_64-linux-android
```

Gate B-R1 passed：fresh export 8/8 stages passed。新 APK 仅含 `lib/x86_64/libmain.so`，签名、manifest、
SHA-256、`zipalign -P 16` 与四个 ELF LOAD `0x4000` 对齐全部通过。API 37/16 KB AVD 冷启动后 8 秒仍为
top resumed，首帧真实非空，GLES 被加载，无 Vulkan adapter 请求、surface conflict 或 fatal；真实点击征兵
后界面变化并出现预备卡。相同 APK 在 API 35 完成 Open、首帧、8 秒存活与 Stop。

```text
run root：G:\AifeAndroidRuns\305-bmin-r1-20260826-100804
APK：G:\AifeAndroidRuns\305-bmin-r1-20260826-100804\artifacts\TowerDefense-x86_64-debug.apk
APK bytes：43,669,250
APK SHA-256：899F7665C88AA51857513B6A1E9462002980694EA9B725AE0206F82A0AAD63C0
completion report：G:\AifeAndroidRuns\305-bmin-r1-20260826-100804\305-bmin-r1-report.json
```

本轮 owned API 37/API 35 Emulator 已停止，fresh Tower 副本已按严格 cleanup proof 删除；run root、APK、
Gradle cache、日志、截图和报告保留。未运行 Local CI，未修改 Tower/真实配置，未替换 production
Editor/Player/MCP，未重跑完整 304-E3/E4，也未重建或真机验证 ARM64 APK。
