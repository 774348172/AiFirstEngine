# 336 Engine Runtime DLL 可见试玩与语义执行最小接入 v1 施工文档

状态：2026-09-13 最小施工完成并归档；Gate A/B/C passed，Gate D 安装独立预检及三档测量 passed，当前宿主旧连接重载为明确 deferred。该边界由第 5 节预先约定，不冒称 host_activated。335 保持归档，254 文件仅为历史证据。
正式方案：`../../336-Engine-Runtime-DLL-可见试玩与语义执行最小接入-v1方案.md`。
起始时间：2026-09-13 13:02（本地），连续施工上限 16:02，15:47 后只收尾。

## 1. 必要性与验证经济性自审

确认失败：正式包 windowed 请求走 headless；semantic worker 绕过 DLL。最小修改为已有执行请求的 C ABI 传输、DLL 内有限分发、两条调用方回接及真实窗口构建 feature。复用 runtime_player_winit / semantic outcome / bounded process / 335 交付身份，不新增工具或运行 owner。

保留 dirty worktree；源码、测试、原样项目、DLL/Host、安装和配置身份分开记录。只对受影响 crate、入口和真实组合验证，禁止 workspace/Editor/Android/Local CI。未改变的 335 固定布局、旧 ABI 布局及进程取消证据复用；发现具体相关回归才补跑。

| Gate | 要捕获的失败 | 最便宜验证 | 终态证据与成本 |
|---|---|---|---|
| A | mode/输入/捕获丢失、坏请求、旧 ABI 静默回退 | 共享执行请求与 DLL owner 定向单元、旧行为红证据 | 请求/分发回归，约 10–25 分钟 |
| B | 普通/semantic 入口仍在 Host 执行、报告假成功 | runtime_cli 定向/受影响 suite、DLL 执行者和失败传播测试 | 日志及 DLL 构建身份，约 10–25 分钟 |
| C | 原项目观测/输入/画面在 DLL 链失败 | 一份正式原样项目交付，windowed verify + 既有 16 断言/4 捕获 + observe | 独占 `.artifacts/336-*`，原包/报告/截图/摘要，约 10–30 分钟 |
| D | 安装未升级、项目变化误重编引擎、暖缓存冒认冷编译 | 既有安装/CLI及真实可用宿主入口；三种构建各一次 | 安装/连接层次、分段耗时、完成记录，约 10–30 分钟 |

预计 40–110 分钟。任一昂贵分支超过估计先看实际首因，不用新 Runner 或重复全测试补偿。文档、代码和安装修改均在本轮用户已确认范围；框架边界改变才重新讨论。

## 2. Gate A：有界 DLL 执行合同

- 修改 `rust/crates/engine_runtime_host/src/lib.rs` / Cargo.toml、既有双方依赖 crate 中的共享内部传输定义；补新入口、能力检查及有界载荷解析，保留旧 headless。
- mode、GameView、input、screenshot、report level、frame budget、performance 和 scenario/capture 目录沿用现有类型。DLL 复用已有函数加载模块并执行；报告包含真正 DLL owner。
- 最小回归覆盖普通与semantic的mode一致性、参数保存、坏JSON/超界、无模块、报告失败。旧版只headless不得冒充新增能力。

命令按最终文件选择：`cargo test -p engine_runtime_host --locked --lib`；共享请求所在 crate 只跑新增定向用例。一个 Cargo 调度者，Agent 不同时争用 target。

## 3. Gate B：现有入口接入

- 修改 `runtime_cli/src/lib.rs`（必要时移出已有 loader 为独立源码文件）、`semantic_playtest.rs`；新正式包模式和 semantic 走 DLL，非交付测试入口保留。
- parent 继续持有超时/进程树/报告读取与证据验证；不新增调度器。DLL 缺失、符号/能力不匹配、坏报告和非零执行不能变成成功或静态 fallback。
- 报告映射不得再调用静态 Runtime。`engine_runtime/src/windowed_player.rs` 只公开既有空报告构造器，CLI 回归使用不存在的包证明报告转换不触发第二次执行。
- 确认 ProjectPlayerArtifact 消费本轮已构建且支持窗口的 Engine DLL，避免旧文件仅凭存在被当作新版；只做必要能力检查，不重构缓存。保持项目生成胶水/模板与现有公开导出路径。
- 运行 `cargo test -p runtime_cli --locked`，必要 real-window 对应入口单独验证；`cargo build -p engine_runtime_host --locked` 生成正式 Windows 窗口 DLL。不机械重复已覆盖 check。

## 4. Gate C：原样项目可见试玩

- 使用当前正式 CLI/Compiler/Provider，原样项目输入复制到本轮专属根，保持 source bytes。复用已有入口/测试辅助方式，不新建永久 Runner。
- build 不隐式试玩；成功后先留存完整交付，再使用既有 Game.exe/runtime_cli `verify-exported-player --mode windowed`（输入/截图）及项目默认 playtest/observe。当前 typed delivery_verify 只接受 headless，不为本次窗口验收扩大其 schema。
- 验收现有 16 断言、4 捕获、实际 1800 tick场景；报告证明 Engine DLL 实际执行。视觉检查捕获内容；不把自动输入回放称为人工手感审核。
- 若原样项目暴露实现/夹具兼容缺陷，记录首因并在已有 owner 修复，禁止换成 test.frame、改断言或手改派生包绕过。测试文档旧数字可修正，不改变项目验收内容。

## 5. Gate D：宿主更新、耗时和归档

- 新 Provider 编译/预检通过后，复用 325 的单目标构建、备份旧 EXE、同路径可恢复替换。仓库没有当前 Provider install/activate CLI，codex_config 仅生成未应用片段；不调用历史 Editor/Gateway 安装命令。保留旧配置/二进制，最小修改，不终止整个用户宿主或无关进程。
- 用可用真实 typed 连接确认实际版本；若客户端不支持自动重连，完成其它独立工作并精确记录该外部限制，安装态通过不冒称当前会话通过。
- 初次原样项目构建/无修改重复/专属副本代码小改各一次分段测量，检查 Engine DLL SHA 未因项目修改变化。原项目不作试验写入。记录缓存条件，可能的不可比性和仍存在的 Host 编译依赖；只报告有证据的秒数。
- 汇总相关回归、原样交付、captured evidence、source/binary/安装身份；同步 49/54/地图/阅读顺序/README/完成记录，全部必要 Gate 达标后归档本文件。未具备当前宿主观察则单列终态边界，不能伪造通过。

## 6. 施工文档自审

2026-09-13 主审通过：与已确认最小方案一致，用户已授权顺序完整；非新系统讨论，无需再选方案。失败分支只覆盖新增 DLL 接入风险，现有原样场景承担集成验收，不增加测试平台。真实窗口/DLL构建属于本次交付声明必要范围；安装只有在原样项目通过后执行。历史规则不要求重跑全工作区或先安装后调试。预计成本在三小时内。

独立复核通过，已澄清实际安装采用 325 同路径替换方法；windowed verify 使用现有 CLI，保持 typed 九工具 schema。当前连接重载若不可自动完成须明确 deferred，不冒认安装盘上字节等于进程字节。

## 7. 执行记录

Gate A/B：共享 4 MiB 有界请求、DLL 分发、普通/semantic caller、纯报告映射完成。独立复核发现的未知 schema 绕过及缓存旧 DLL 命中已在原 owner 修正并补回归，未新增工具、crate 或缓存架构。

基线红证据：`.artifacts/336-baseline-windowed-request-report.json`，旧 335 包请求 windowed/1 帧，返回 exit0 但实际 mode=headless-gate，已复现模式静默降级。stdout 保留同名 `.log`。

回归终态：CLI 60 项、共享请求 2 项、DLL 7 项、缓存 DLL 资格 1 项，共 70 项去重通过。首次 CLI suite 的 3 个旧 release 进程回归因错误分流失败；先定向修复再完成最终 suite，未削弱断言。格式及相关 diff-check 通过。

Gate C：原样输入副本完成单次正式交付，完整 59 文件复制后 windowed verify 120 帧、真实截图通过。项目既有场景 1800 tick、16 断言、4 捕获全部通过，observe 再验通过；普通/semantic 报告分别记录 native_host.engine_runtime.execution_owner / engine_runtime.execution_owner。四张捕获均人工检查过输出图像；不是人工操控手感或性能结论。

Gate D：Provider 目标构建 24.42 秒；按 325 同路径可恢复替换，安装盘上 SHA 与已验候选一致，独立 initialize/tools9/原项目 inspect 通过。config SHA 未变；当前 typed inspect 仍返回旧 PID 15964，未终止用户旧连接，host_connection_reload_deferred。三档真实 build 分别 77.223 / 19.055 / 26.329 秒，小改后只重编 3 个产物、137 fresh，Host build 3.498 秒，Engine DLL 字节在三份包中一致。

证据经济性收尾：首轮完整 build 响应及总耗时已保留，但 Player 逐阶段报告被后续缓存 hit 覆盖，不能声称首轮内部时间分布；已修正本轮证据脚本及时保存外部报告，第三档完整分段报告保留。不为了恢复该非功能明细删除缓存或重跑冷编译；这是明确的证据豁免，非性能收益证明。一次性 DLL build 5.49 秒为依赖已热状态，不能当作引擎全冷成本。

本轮误复制的临时 target 副本清理被执行前自动策略拒绝，保留为 retained_by_host_policy；原项目缓存未改。全部本轮运行进程已退出，既有宿主 Provider 保留。完成记录：`../../阶段完成记录/2026-09-13-336-Engine-DLL-Visible-Semantic-Execution-v1/00-总览.md`。
