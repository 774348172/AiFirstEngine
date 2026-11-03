# 375 本地 WebAssembly 试玩与游戏展厅交付

状态：`local_web_delivery_verified`。2026-10-25，用户授权十小时窗口内完成 G1–G6；Native 仍是正式主线，Web 为本机桌面 WebGPU、单线程 2D 验证交付。没有公网发布、安装替换或游戏 V0.0.5 升级。

## 交付与使用

本机入口：`http://127.0.0.1:8751/`。点击「熄灯时刻」进入真实引擎，支持鼠标点击、数字 1–9、R 重置、N 下一局、暂停/继续、重新开始及全屏。首局 1、5、9 三步可解，共四局。网页游戏 HUD 为引擎 AUI，玩法是项目 Rust 模块。

独立交付：[目录与操作说明](../../../../../../05-Build/375-web/deliveries/origin-play-20261025-final07/README.md)，[双击启动](../../../../../../05-Build/375-web/deliveries/origin-play-20261025-final07/Start-Showcase.cmd)，[完整文件指纹](../../../../../../05-Build/375-web/deliveries/origin-play-20261025-final07/delivery-manifest.json)。通过实际 cmd 启动脚本启动，Node.js 使用本机已有安装；不依赖引擎源码或 Cargo 环境。PowerShell 入口为可选方式。

交付保留 loopback 服务 PID **16216**，工具会话 39241；根目录为 `05-Build/375-web/deliveries/origin-play-20261025-final07/site`。旧工作服务 50980 和旧交付服务 47072 已按命令行/root 身份核对停止。终端启动可 Ctrl+C 关闭；停止当前后台服务时先确认 PID 16216 的 `CommandLine` 包含本次 `origin-play-20261025-final07/server.mjs` 和端口 8751，再执行 `Stop-Process -Id 16216`。PID 只适用于此次启动，后续启动以实际进程为准。

最终浏览器保留本路 tab 17；本路旧 tab 11/12/14/15/16 已关闭，用户 Craft 4/5/7/8 保留。临时 viewport override 已恢复，退出试玩后 iframe 为零。构建、测试进程已结束；交付服务及其启动父进程保留。

## 资源实际位置与运行链路

`site/games/lights-07/package/` 是场景、AUI、字体、纹理等发布资源；`site/games/lights-07/wasm/` 是静态链接的引擎和该项目逻辑。页面点击后从同一 HTTP 网站下载，先校验文件长度、SHA256 与项目/模块/包身份，再交给原 RuntimePackage 解码器；在访客浏览器执行 WASM，并通过 WebGPU 呈现。

本地服务从上述磁盘目录读取，localhost 只供本机。公开上线时可整体部署 `site/` 到 HTTPS 服务器或对象存储/CDN，浏览器从该服务传输并可能缓存文件。首版使用同源目录；分离 CDN 的跨域/路径与发布配置未验收。展示页和资源可以在同一站点托管，不能只复制 HTML 代替完整资源目录。原生目录 source 的 canonicalize/符号链接防护保留；memory source 没有另一套包解析器。

## 实现与接入

- Rust 平台时钟、单线程任务、wgpu WASM 边界适配；新增 `runtime_player_web`，消费原 bootstrap、world、EngineHostLoop、RealWgpuBackend 与 AUI。
- 不可变 memory RuntimePackageSource 注入现有包/场景/纹理/字体/theme 消费者；交付清单拒绝坏路径、重复、缺失、长度和哈希不符。
- 原 ProjectRuntimeProductionStaging → ProjectRuntimePackageAssembler → RuntimePackageBuilder 和编译器生成静态 ABI glue；普通 `samples/web_lights_project` 经该链路导出。
- 展厅提供实际画面封面、搜索/分类、同源 iframe、会话消息校验、错误重试和发布说明；JS 仅负责平台展示、下载、输入与生命周期。

隔离快照 `01-Engine/worktrees/375-web`，独立 target `05-Build/375-web/cargo-target`。已逐文件备份、核对后接入 main；原生另一条路的诊断修复和未提交修改保留。接入清单、首个换行断言原因及补丁备份见 [integration-manifest](../../../../.artifacts/375/web-runtime-20261025-061020/integration-manifest.json)。未进行 Git 索引写入，原空 `.git/index.lock` 保留。

## 证据与资格

证据根：`.artifacts/375/web-runtime-20261025-061020`。工具链 Rust 1.96.0、wasm32-unknown-unknown、wasm-bindgen 0.2.125。快照开发证据与主线组合证据分别保留，不声称 clean exact commit 或全 workspace 资格。

| Gate | 终态与主要证据 |
|---|---|
| G1 平台编译 | core、原 wgpu 后端、browser host 的 WASM 检查通过；`g1-*.log`、`g3-player-check-*.log` |
| G2 原包语义 | source 4、交付负例 1、native 包 owner 15、原消费者 55、native/memory 同包语义与纹理 5；缺失/坏数据走原解码与诊断 |
| G3 正式导出 | Web profile/目的地 owner 2；真实项目 Rust 单元 3；第七版主线 release 导出通过，`g3-main-full-export-07.log` |
| G4 实际 Player | 可见中文 glyph/正确色彩、鼠标、键盘过关、下一局、暂停稳定、重启、缩放和前台全屏；`g4-*.png/json/txt` |
| G5 平台与失败流 | 搜索空结果/命中、分类、发布指南、卡片进入、退出/重开；404、坏 WASM、项目混用、坏 package 拒绝，恢复后重试可玩；`g5-*` |
| G6 集成与独立交付 | main 包 source 5 和包 owner 15 重新执行通过，native scoped Clippy、四个原生直接消费者 check、WASM scoped Clippy、52 个 Rust 文件定向 fmt；43 磁盘文件及 39 HTTP 文件校验通过 |

共 **85 项去重定向测试**，重跑不重复累计，[测试索引](../../../../.artifacts/375/web-runtime-20261025-061020/g6-test-evidence-index.json)列出名称和日志哈希。20 项 main 组合测试和实际命令/feature/时间见 [组合结果](../../../../.artifacts/375/web-runtime-20261025-061020/g6-main-composition-results.json)。未变 owner 的旧开发证据按其来源保留，不冒充不同环境；原告警保留，21 项新 needless-borrow 已消除，未声称全仓库无告警。

第七版固定身份：WASM **13,823,189 bytes**，SHA256 `88de0895a5ffc6e23d304aaeb44d267d79fe4fc338f369dcbce435d0a4455d26`；资源包 **21 文件 / 4,865,757 bytes**，内容 hash `sha256:a0f8885a9e6bee6390cdada22b91ead0ef8fcc20259182285858d24cda00a873`。30 个导出收据文件验证，1194 项输入从 prepare 到发布无漂移，产物扫描无本机绝对路径泄漏。[导出审计](../../../../.artifacts/375/web-runtime-20261025-061020/g3-main-lights-07-export-audit.json)、[独立 HTTP 交付审计](../../../../.artifacts/375/web-runtime-20261025-061020/g6-final07-delivery-verification.json)保存完整身份。项目 sourceDigest 与引擎输入清单分别记录，不能混作同一个身份。

## 首错及修复

- 首次 hydration fixture 漏登场景资产，以及原 Windows 包 AUI/aui 大小写：修正 fixture、复用原无歧义 collision key，保留路径与重名拒绝。
- 全中文源字体 Cook 超 MSDF 页预算：按真实 UI 字符生成 Noto SC 子集，未扩大预算；157 码点/163 glyph，字库 29,700 bytes，OFL 许可随交付。
- 404 后并行下载仍改写错误界面：失败递增 generation，后续 reader 停止过期进度；真实负例验证修复。
- 第六版重启出现异步 `closure invoked recursively or after being dropped`：vendored wgpu 26.0.1 的 `set_device_lost_callback` 注册一次性 closure 后立即 drop。仅在 WebGPU owner 回补上游 `closure.forget()` 的 JS 所有权转移，保留版本和 374 native RenderBundle 补丁。[首错](../../../../.artifacts/375/web-runtime-20261025-061020/g6-device-close-first-cause.json)、[修改与备份](../../../../.artifacts/375/web-runtime-20261025-061020/g6-device-lost-callback-repair.json)保留。新 WASM lint/fmt、重新导出、实际连续三次重启及关闭重开通过，[最终控制台](../../../../.artifacts/375/web-runtime-20261025-061020/g6-final07-console-clean.json)为零错误。第六版保留首错身份，未用于最终交付。
- 后台标签全屏曾观察到黑屏；前台全屏有实际画面，此项没有改 renderer。IAB 标签切换未可靠触发 visibility 策略，**自动后台暂停未获资格**；显式暂停已验证帧/tick 稳定。没有注入内部世界状态或伪造事件补过验收。

实际浏览器截图：[最终过关画面](../../../../.artifacts/375/web-runtime-20261025-061020/g4-final07-playable.png)、[最终展厅](../../../../.artifacts/375/web-runtime-20261025-061020/g5-final07-showcase.png)。启动耗时仅是本机一次观察，不作公网性能承诺；没有正式性能、显存峰值或零内存增长资格。

## 收尾边界

本版不授其它浏览器、手机、WebGL、3D、Spine、音频或全部既有游戏兼容性；既有游戏继续 V0.0.5。没有运行完整 workspace/全平台/正式性能矩阵，因为不属于此单线程 2D 本地交付的声明。

375 文档移入已完成并释放 B；**374 保持待续 partial，必须重新激活才施工**。371 A 路继续按其自身当前登记推进，本记录不关闭 371/S1、不改变其预算或旧资格身份。只保留最终服务，其余本路后台已结束。
