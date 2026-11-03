# 261 Window H-R1 production staging 修复与 r10 视觉记录

日期：2026-07-31

## 结论

`ProjectRuntimePlayerArtifact` production staging 的通用缺口已修复并通过受影响闭包：

- SourcePatch v1 继续保持最小依赖策略，不接受普通项目第三方依赖或任意 path；
- 普通 ProjectRust production export 使用独立
  `project-runtime-player-production-staging.v1`；
- 源 `RuntimeModule/Cargo.toml` 保持 byte-exact；
- staging 生成独立 `RuntimeModuleBuild/Cargo.toml`，`lib.path` 指回原项目源码；
- trusted `engine_runtime` / `engine_input` 统一规范化为 canonical Engine SDK path；
- `serde` / `serde_json` 由受控 allowlist 和 lock identity 解析为精确版本；
- Host 只依赖 `RuntimeModuleBuild`，不再依靠 artifact root 与仓库的偶然相对布局；
- 原 `RuntimeModule/tests/*.rs` 由规范化 manifest 显式声明，tests-compile 不丢失；
- `.aife` Preview cache 不进入 production source copy/digest closure。

R1 已完成。261 Gate H 仍为 `incomplete`，不得归档 261：production multi-DPI/MSDF
清晰度截图和受控 fallback 同屏截图仍未取得。

## 实现

主要 owner：

```text
rust/crates/editor_core/src/project_runtime_player_staging.rs
rust/crates/editor_core/src/project_player_artifact.rs
```

正式方案已同步：

```text
框架设计/引擎总体架构/261-R1-ProjectRuntimePlayerArtifact-Production-Staging-v1方案.md
```

相对 trusted SDK 的 owner 红测使用真实塔防项目，把 staging 放到系统临时目录后执行
locked/offline Cargo metadata。该布局与仓库不共享相对父目录，旧实现不能通过。

## r10 production evidence

证据根：

```text
G:\gameEngin\rust\target\261-window-h-visual-20260731-0950
```

production artifact build root：

```text
C:\Users\zenghaoran\AppData\Local\Temp\aife-261-player-r10
```

该 root 位于项目与 trusted Engine SDK 外，并与 SDK 跨盘。证据副本：

```text
evidence/project-runtime-player-build-report-r10.json
evidence/desktop-export-report-r10.json
evidence/windowed-player-r10.json
evidence/exported-player-r10.png
export-r10/dev/Game.exe
```

关键 identity：

```text
HEAD=acaffa8cb95bbb54d093bbf45835a59315c82da5
projectSourceDigest=sha256:a447a29140f79816cde7c30f94ba7a7d02cf3e0d3701192f38ad9b40e96c6837
engineSdkDigest=sha256:59936ff4375e34a6ed992b0081f788aaa88760acaec7a4ceabd36ac3b6ee8a85
normalizedManifestDigest=sha256:42a53404363f3b423af1a24655c3c46e8d0a19e40cd817803fd0800f0414a086
normalizedDependencyDigest=sha256:5037498fbd0b2c98a5b8521ef524c86754c5292e0e7d6af0a6cebecfcc8ebd7f
trustedLockDigest=sha256:2860f8cb7d241225a1c8eb7fc09b18f7d42d9ab94b28c7a529bb33491fcb32ce
fontSha256=f70ecb32e5b312ba7bc724977352139a3f691566dc2491377be3828631c9fab2
fontBundleDigest=sha256:a5fc1c60a6aff8479158aa1e11f68f9dbfed672e6f92d8e6856826d2c13bf451
playerSha256=44eab9c6185c570698ad9f49ad3c33cd9b4c1881d32fb0b48ee6daedcbe472fe
screenshotSha256=750da25d1b196196a6cc7688282899bf644267858e16f6ec2e5a343b44a8d299
```

artifact report 证明：

```text
stagingPolicy=project-runtime-player-production-staging.v1
engine_runtime=canonical trusted Engine SDK
serde=1.0.229 crates.io locked
serde_json=1.0.151 crates.io locked
expected AOT descriptor == actual AOT descriptor
all build steps exit 0
```

真实 Windows Player：

```text
mode=windowed
framesCompleted=3
present=presented
screenshot=captured
pixelSize=1600x900
diagnostics=[]
ProjectRuntime bind=passed
producerId=tower_defense_ui_state
```

同屏截图包含同一字符串 `中界` 的 16px Bitmap R8 与 48px MSDF；FontBundle v2 有一张
Bitmap page 和两张 MSDF page，`qualityGateEligible=true`，`fallbackUsed=false`。

## 验证

```text
cargo fmt --check
passed

cargo test -p editor_core project_runtime_player_production_staging -- --nocapture
4 passed

cargo test -p editor_core project_runtime_player_artifact_project_rust_preview_project_rust_export_share_identity -- --nocapture
1 passed

cargo test -p editor_core
589 passed; 2 ignored

cargo test (tower RuntimeModule)
60 passed

cargo test (tower Tests/FontBundleV2)
1 passed

cargo test -p project_e2e_gate
65 passed; 1 ignored; integration binaries passed
```

首次并行全量 `editor_core` 中
`asset_thumbnail_decodes_png_and_preserves_non_empty_alpha_pixels` 因两秒等待预算耗尽失败；
单测立即通过，无并行负载的权威全量重跑 589/589 通过。它不属于 261/R1 修改面。

## 未完成视觉项

```text
production multi-DPI 或连续缩放下的 MSDF 清晰度证据：缺失
production controlled fallback 同屏证据：缺失
```

当前 public Player CLI 没有 DPI/scale 注入参数；塔防项目 font stack 只有一个真实 face，
`missingGlyphPolicy=error`。本窗口没有增加测试旁路、系统字体 fallback 或伪造截图。

## 外部状态

```text
production/安装态二进制：未替换
Local CI：未运行
项目 Build/：未作为 production 输出
外部 artifact root：为证据保留
owned child processes：全部退出
```

后续继续 Window H 时，必须使用普通项目公开字体资产 seam 建立第二个受控 face/fallback
fixture，并在真实 production Player 中获取 fallback 同屏证据；multi-DPI 需要真实 OS DPI
矩阵或另行确认的 production scale qualification seam。两项通过前不得关闭 Gate H。
