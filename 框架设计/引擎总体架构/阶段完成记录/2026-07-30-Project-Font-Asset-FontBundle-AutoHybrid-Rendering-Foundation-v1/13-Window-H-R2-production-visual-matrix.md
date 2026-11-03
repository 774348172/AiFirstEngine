# 261 Window H-R2 production 视觉矩阵完成记录

日期：2026-07-31

授权窗口：2026-07-31 14:56 +08:00 至 17:56 +08:00；17:41 后不得开启新阶段。

## 结论

Window H-R2 已完成，Gate H 的剩余 production 视觉矩阵通过：

- 第二个受控 FontFace 通过普通项目字体资产 seam 进入塔防项目；
- `AifeQualificationCFF-Regular.otf` 作为 Latin 主 face，原 Noto Sans SC TTF 作为中文
  fallback face；
- FontBundle manifest 为 `fallbackUsed=true`、`qualityGateEligible=true`；
- `A中界V` 同屏混排真实消费两个 face，`rendered == requested`、`missing == 0`；
- production bundle 为 1 张 Bitmap page 与 4 张 MSDF page，同屏 multi-page/fallback 非空；
- 同文 32px Bitmap/MSDF 保持相同 codepoint、起点和 advance，基线行距稳定；
- 真实 Windows Player 在单显示器 96 DPI 环境中，以公开 AUI seam 同屏呈现
  36/48/72px MSDF 连续缩放样本，三个区域均非空；
- 真实 Player 完成 3/3 frames、`present=presented`、1600x900 screenshot、diagnostics 为空。

当前桌面只暴露一个 96 DPI monitor，因此没有伪造 multi-monitor DPI。261 原施工合同允许
“连续缩放或多 DPI”；本轮使用 production continuous-scaling 分支，不增加引擎 DPI/scale
qualification seam。

## 项目改动

全部 H-R2 功能改动位于普通项目公开 seam：

```text
samples/tower_defense_project/Assets/Fonts/
samples/tower_defense_project/AUI/battle-hud.aui.json
samples/tower_defense_project/Library/AssetPipeline/asset-database.json
samples/tower_defense_project/Tests/FontBundleV2/tests/font_bundle_v2.rs
```

新增 OTF SHA-256：

```text
b89d882e3082ac1f03c337af4e1ba3250384e9b4e645907a0e6f8e606ec3bf93
```

许可证为仓库自有 qualification fixture 的 MIT 条款；项目内保留许可证副本。没有修改引擎
源码，没有使用系统字体、debug overlay 或测试专用 renderer 旁路。

## r11 production evidence

证据根：

```text
G:\gameEngin\rust\target\261-window-h-visual-20260731-0950
```

关键文件：

```text
evidence/frozen-candidate-r11.json
evidence/project-source-manifest-r11.json
evidence/project-runtime-player-build-report-r11.json
evidence/desktop-export-report-r11.json
evidence/font-bundle-manifest-r11.json
evidence/windowed-player-r11.json
evidence/font-production-visual-qualification-r11.json
evidence/exported-player-r11.png
export-r11/dev/Game.exe
```

关键 identity：

```text
HEAD=acaffa8cb95bbb54d093bbf45835a59315c82da5
projectSourceManifest=sha256:7d0947788721aa09703e1edbafadfa771c38aa24e1c787912f67c578d3646e31
projectRuntimeSourceDigest=sha256:b143cd59156c7ba92683e3018e13168d9e3e921660f7baafe754b882f8ac3c17
engineSdkDigest=sha256:59936ff4375e34a6ed992b0081f788aaa88760acaec7a4ceabd36ac3b6ee8a85
fontBundleDigest=sha256:4ab047c8b0de1f5ef42f8456fc194d2678e04bd7a938712a25fd9e84d8f05d66
playerSha256=4389f9cde4c8e19f8e185ad50e33a8497e3776a0e4d15527c5cf5ff715206b95
screenshotSha256=ee2c42044d38e87b27ad84e780fc46b77cf635b8950b7805e67cfed78ac4021f
qualificationReportSha256=bece589f921678bfd0235d050fe8b92cb6476f33f74dc69a50291410bafde5d6
```

## 验证

```text
cargo fmt --check
passed

cargo test -p editor_core project_runtime_player_production_staging -- --nocapture
4 passed

cargo test --test font_bundle_v2 -- --nocapture
1 passed

production DesktopExportPipeline r11
success

export-r11/dev/Game.exe run-native-player --mode windowed --frames 3 --screenshot
3/3 frames; presented; screenshot captured; diagnostics=[]
```

r10 已通过且输入未变化的 `editor_core` 全量 589/589、塔防 RuntimeModule 60/60、
`project_e2e_gate` 65/65、非塔防第二项目 v2 seam 和 legacy 专项矩阵继续有效，未机械重跑。
H-R2 项目输入变化只失效并重跑了 FontBundleV2 consumer、production staging owner 和
production export/Player。

## 外部状态与禁止项

```text
Local CI：未运行
production/安装态二进制：未替换
引擎 DPI seam：未新增
系统 DPI / monitor 配置：未修改
owned child processes：全部退出
外部 artifact root：作为 production evidence 保留
```

## Gate H 判断

Gate H passed。Window A-H、negative matrix、consumer closure、第二项目、legacy、塔防普通
用户路径和 production 视觉证据均已闭合，允许生成 261 总完成记录并归档施工文档。
