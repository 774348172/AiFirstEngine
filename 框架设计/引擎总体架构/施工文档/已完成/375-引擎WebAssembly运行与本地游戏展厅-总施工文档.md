# 375 引擎 WebAssembly 运行与本地游戏展厅总施工

状态：G1–G6 已完成，`local_web_delivery_verified`，2026-10-25。方案：[375](../../375-引擎WebAssembly运行与本地游戏展厅-v1方案.md)，[双自审](../../375-A-方案与施工文档自审.md)已通过。用户明确“网页版接替B槽，保留374待续状态”。374 未完成，待执行保留全部成果，未经新激活不得施工。375 归档后释放 B，不自动恢复 374。[完成记录](../../阶段完成记录/2026-10-25-375-WebAssembly-Local-Showcase/00-本地Web试玩交付.md)。

## 授权和施工边界

十小时自 2026-10-25 06:10:20 UTC 起，16:10:20 硬截止，15:55:20 停止新增。优先于公共技能默认三小时。主线当前 dirty 源码只作为快照来源；独立目录 01-Engine/worktrees/375-web，独立 05-Build/375-web/cargo-target。证据主线 .artifacts/375/web-runtime-20261025-061020。不安装、不升级固定 V0.0.5、不发布公网、不改 371 正式预算或 GPU 几何优化。

共享修改 owner：375 拥有新增 browser player、memory package source、Web 导出、展厅及其必要 cfg 适配；371 拥有几何/性能/原生 Player 在途工作。需改公共 loader、assembler、SDK/帧循环时仅在快照实施，记录基线哈希及差异，接入逐文件串行核对，无条件覆盖禁止。当前 A 新十小时授权保持，GPU 性能测量错峰，本路不运行正式性能基准。

## 必要性与验证经济性审计

请求：网页点击即可真实运行本引擎和 SDK 游戏，资源 HTTP 下发。

最低改动：WASM 平台依赖/时钟适配、只读 package source 接入现有 loader、已有生成器静态 glue、browser player 和静态导出/展厅。文件集中在 runtime_core/content/render_native、authoring_execution 的直接 owner 及新 web crate/前端。构建时不扫描整个工作区联接。

排除：第二引擎、手写派生包、通用脚本、云串流、手机全平台资格、全量引擎回归、生产安装。重复证据仅在实际消费源码/feature 改变时失效。现有 native 证据只作未变功能参考，不冒充 Web 结果。

| Gate | 风险/失败契约 | 最便宜可红验证 | 最终证据/成本上限 |
|---|---|---|---|
| G1 基线与平台编译 | 当前 Rust core/backend 无法编译 wasm | wasm32 cargo check 直接受影响 crate；记录首错 | 源快照清单、工具版本、check 日志；90min 后审视架构 |
| G2 包 source | 内存读包绕过原校验、路径逃逸或损坏 | source owner 的路径/缺失/坏 JSON/hash 定向测试，native 对照 | 同包双 source 语义、受影响 consumer tests；90min |
| G3 项目导出 | 玩法由 JS 替代或另造 assembler | 普通项目中立装配→生成静态 glue→wasm 构建，坏导出清单测试 | 可移动导出目录、包/逻辑 hash；90min |
| G4 实际浏览器 Player | tick 有数但无真实渲染/输入/AUI | HTTP 页面点击、键盘、截图可见 glyph 和项目状态；resize/暂停/重启 | 浏览器观察、页面状态与截图；120min |
| G5 展厅与错误流 | 卡片不可玩、iframe 错配、下载失败静默 | 搜索、卡片、全屏/退出/重试；坏路径/hash 拒绝 | 真实展厅交互和缺失资源失败证据；60min |
| G6 受影响集成与收尾 | native 回归或静态目录依赖源码 | 定向 cargo tests/check/fmt/clippy；源码外副本 HTTP 再玩 | 源码/产物指纹、阶段记录和入口同步；90min |

估计是探索边界而非承诺时长；上游失败先定位，不盲目重跑大构建。预算剩余用于实现缺口与验证，不靠堆叠全量测试消耗授权。

## 执行命令与材料

当前进程加载 07-Tools/Enter-EngineEnvironment.ps1 后立即覆盖 CARGO_TARGET_DIR 至 375 并进入快照 rust。rustup target add wasm32-unknown-unknown；必要的 wasm-bindgen CLI 版本由 Cargo.lock 确定。只改进程环境。

G1：cargo check --locked --target wasm32-unknown-unknown -p engine_runtime_core；随后 -p engine_render_native --features real-wgpu。
G2：cargo test --locked -p engine_content --test <实际 source owner test>；core 按当前 lib test=false 约束使用现有 foundation integration owner 过滤，不新建覆盖整个引擎的临时 runner。
G3：仓库 Web export 命令、cargo build --locked --target wasm32-unknown-unknown -p runtime_player_web；wasm-bindgen --target web。导出入口随实现写回本节实际命令。
G4/G5：仓库 loopback server，实际浏览器交互，显式 test mode 可导出紧凑状态；不通过内部 JS 改游戏世界过验收。
G6：cargo fmt --check 对修改 owner；cargo clippy --locked -p <受影响 owner> 对选定 feature；native cargo check/test 直接 consumer。实际命令及豁免写 evidence，不声称未执行全矩阵。

实际执行补充（均使用 375 独立 target；开发 profile 为 dev-fast）：

- G2：`cargo test --locked --profile dev-fast -p engine_content --lib runtime_package_source::tests`；同 crate `web_package::tests`；`cargo test --locked --profile dev-fast -p engine_runtime --test runtime_package_source`（最终5项）；`--lib runtime_package::tests::`（15项）。
- G3：`cargo test --locked --profile dev-fast -p project_authoring_execution --lib web_`；项目 `RuntimeModule/Cargo.toml` 的 Rust tests（3项）。实际项目模块入口以样例 README/工程文件为准。
- 最终导出：`web/Export-WebGame.ps1 -ProjectRoot samples/web_lights_project -BuildRoot <375>/exports/lights-07-build -OutputRoot <375>/site/games/lights-07 -WasmBindgen <375工具目录>/wasm-bindgen.exe -CargoTargetDir <375>/cargo-target -Profile release`，jobs=1。精确绝对路径和输出在 `g3-main-full-export-07.log`，未在编译期间改 consumed source。
- G6 main：`cargo clippy --locked --profile dev-fast --lib -p engine_content -p engine_runtime_core -p engine_render_native -p engine_runtime -p project_authoring_execution --features engine_runtime/real-wgpu`；`cargo check --locked --profile dev-fast -p editor_core -p editor_window_winit -p project_e2e_gate -p engine_tool_provider`；`cargo clippy --locked --profile dev-fast --target wasm32-unknown-unknown --lib -p runtime_player_web`。完整运行结果在 `g6-main-composition-results.json`；最后 callback 改动另续 `g6-main-wasm-callback-clippy-03.log`。
- 定向 `rustfmt --edition 2021 --config skip_children=true --check` 主线51文件，vendor WebGPU 文件另1；JS module syntax、PowerShell parser，以及实际 cmd 启动/HTTP 测试通过。没有机械扩大到全 workspace。

## 归档和退出

所有 Gate 完成才将此文档移已完成并释放 B；未完成时保留明确 partial/下一点。更新 49/54、本目录 README、阶段记录 README 和 375 阶段记录，保持 A 行在途内容。清理只针对本路临时进程和目录，保留导出、失败日志、源码及374。网页 localhost 服务如交付保留，明确 PID/端口和停止方法；其余施工后台在截止前结束。

## 当前进展

### G6 有界回调修复，2026-10-25 08:42 UTC

最终 lights-06 重启后的控制台出现 `closure invoked recursively or after being dropped`，首错保存在 `g6-device-close-first-cause.json`。wgpu 26.0.1 的 WebGPU `set_device_lost_callback` 把一次性 Rust closure 借给 `device.lost.then()` 后立即释放。仅回补该 WebGPU owner 的 JS 生命周期转移，采用当前 wgpu 上游同处 `closure.forget()` 的做法，不升级 wgpu、不修改 native backend。保留原 vendor 374 补丁和版本，登记第二补丁来源。

最便宜的可红证据是已经出现首错的真实 WASM/Promise 组合；native owner 测试不能模拟该 JS 生命周期，不另建全引擎 runner。修复后做 wasm Clippy、WebGPU 文件 fmt、一次新版本导出、真实浏览器多次重启/关闭/重开及控制台错误核对；原生结果按未变 cfg 输入复用。旧06保留首错身份，不作为最终交付。此处为实现所有权修复，无需改变正式架构。

- G0：方案→方案自审→施工文档→施工自审完成；已激活 B 槽，374 待续保留。
- G1：core、原 wgpu 后端、新 browser host 的 wasm32 检查通过，最终正式项目 release 已导出。
- G2：4项 source owner、1项交付清单负例、15项 native包 owner、55项既有消费者、5项native/memory包/纹理语义通过。首错及修复保留；main组合重新执行包source5、包owner15。
- G3：普通 `web_lights_project` 经既有 ProductionStaging 冻结、中立 assembler、RuntimePackageBuilder、编译器生成静态 glue 导出；Web owner2、项目Rust玩法3项通过。最终 lights-07，21资源/4,865,757 bytes，WASM13,823,189 bytes；1194输入从prepare到发布无漂移，导出30收据文件校验通过。
- G4：最终中文AUI、正确颜色、鼠标步数1、键盘3步过关、下一局、显式暂停稳定、重新开始、缩放及前台全屏通过。IAB自动后台visibility策略不授资格；未伪造事件补过。
- G5：搜索/分类/指南、卡片、退出移除iframe/重开新会话通过；真实404、坏WASM、错项目、坏package清楚拒绝并可恢复重试。错误进度竞态已修复。
- G6：85项去重定向测试；main受影响组合、native/WASM scoped lint、52文件定向fmt通过。第六版设备关闭异步回调首错经单owner上游回补后，第七版连续3次重启及退出重开通过，最终控制台错误0。43交付文件磁盘校验及39站点文件实际HTTP逐一核对通过。
- 最终独立交付：`05-Build/375-web/deliveries/origin-play-20261025-final07`。实际cmd启动，服务127.0.0.1:8751，PID16216；旧50980/47072已停。保留最终服务和tab17，其余本路构建/测试后台结束，临时viewport已恢复。具体身份、失败、复用、豁免和停止方法见完成记录。
